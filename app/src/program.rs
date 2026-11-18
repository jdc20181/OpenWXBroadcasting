//! The compositor: builds the Map bus (map + overlays) and the Program bus (what goes to air)
//! at broadcast resolution, 30 frames per second, on its own thread.

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{SyncSender, TrySendError},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant},
};

use tiny_skia::{Color, FillRule, Mask, PathBuilder, Pixmap, PixmapPaint, Rect, Transform};

use crate::{
    config::{LibraryPaths, Settings, FRAME_H, FRAME_W},
    data::WxSnapshot,
    gfx::{
        bind::Bindings,
        model::GraphicDoc,
        render::{load_image_pixmap, GraphicsRenderer, RenderCtx},
    },
    map::{
        engine::{MapEngine, MapFrame},
        geo::Viewport,
        layers as overlays_layers, overlays,
        scene::{MapScene, PanelLayout},
    },
    show::Transition,
};

pub const FPS: u32 = 30;

#[derive(Clone)]
pub enum ProgramSource {
    /// Program follows the live map (Map Studio).
    LiveMap,
    /// A graphic is on air.
    Graphic(GraphicDoc),
    Clip(Arc<crate::clip::Clip>),
}

/// Which part of the map bus to draw. With a call-out pulse or an overlay graphic on the map,
/// the still map underneath (radar, warnings, counties, labels, on-air chrome) is drawn once per
/// change (`Base`) and only the moving pieces are drawn over a copy of it each frame (`Moving`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum BusStage {
    All,
    Base,
    Moving,
}

/// Everything the map bus picture depends on, to skip redrawing a map that has not changed.
#[derive(PartialEq)]
struct MapBusKey {
    scene: MapScene,
    data: usize,
    settings: usize,
    radar_time: Option<i64>,
    engine_frames: Vec<u64>,
    /// Wall-clock second: the on-air clock and time-stamps move once a second.
    second: i64,
    /// Something on the map moves every frame (wind flow, overlay graphics, call-out pulse).
    animated: bool,
}

impl MapBusKey {
    /// Moving parts of the still map itself (not the call-out pulse or overlay graphics), which
    /// force the whole map to be redrawn every frame.
    fn base_moves(state: &ProgramState) -> bool {
        let scene = &state.live_map;
        scene.data.wind_particles || scene.lightning_anim || scene.annotations.iter().any(|a| a.motion.is_some())
    }

    /// The key for the still map under the moving pieces.
    fn of_base(state: &ProgramState, primary: &MapEngine) -> Self {
        let mut key = Self::of(state, primary);
        key.animated = Self::base_moves(state);
        key
    }

    fn of(state: &ProgramState, primary: &MapEngine) -> Self {
        let scene = &state.live_map;
        let animated = scene.data.wind_particles
            || scene.selected_alert.is_some()
            || state.map_overlay.is_some()
            || state.map_overlay_slots.iter().any(Option::is_some)
            || state.map_overlay_out.is_some()
            || state.caption.is_some()
            || state.ticker.is_some();
        Self {
            scene: scene.clone(),
            data: Arc::as_ptr(&state.data) as usize,
            settings: Arc::as_ptr(&state.settings) as usize,
            radar_time: state.radar_time,
            engine_frames: std::iter::once(primary).chain(state.panel_engines.iter())
                .map(|engine| engine.latest().map_or(0, |frame| frame.serial)).collect(),
            second: chrono::Utc::now().timestamp(),
            animated,
        }
    }
}

/// A staged map with its own renderers. Map Studio edits and shows this scene; Program keeps
/// showing `ProgramState::live_map` until the operator takes it.
#[derive(Clone)]
pub struct PreviewBus {
    pub scene: MapScene,
    pub primary: MapEngine,
    pub panels: Vec<MapEngine>,
}

#[derive(Clone)]
pub struct ActiveTransition {
    pub kind: Transition,
    pub started: Instant,
    pub seconds: f32,
}

#[derive(Clone)]
pub struct ProgramState {
    pub live_map: MapScene,
    /// Overlay graphic on the map (resolved from `live_map.overlay_graphic`).
    pub map_overlay: Option<GraphicDoc>,
    pub map_overlay_slots: [Option<(GraphicDoc, Instant)>; 2],
    pub map_overlay_started: Instant,
    pub map_overlay_out: Option<(GraphicDoc, Instant)>,
    pub source: ProgramSource,
    pub source_started: Instant,
    pub transition: Option<ActiveTransition>,
    pub black: bool,
    pub black_since: Instant,
    pub radar_time: Option<i64>,
    pub data: Arc<WxSnapshot>,
    pub settings: Arc<Settings>,
    /// Engines for comparison panels 2-4 in multi-panel layouts.
    pub panel_engines: Vec<MapEngine>,
    /// Program's primary map engine once Preview/Take has swapped engines (None: the
    /// compositor's original engine).
    pub primary_engine: Option<MapEngine>,
    /// Staged map shown in Map Studio while Program holds `live_map` (Preview / Take).
    pub preview: Option<PreviewBus>,
    /// When the current alert call-out (blip) started, for its animation.
    pub blip_started: Instant,
    /// Nothing needs frames (no map view on screen, no output, no recording): skip rendering.
    pub idle: bool,
    /// Live caption on the program output: (text, at the top, size multiplier).
    pub caption: Option<(String, bool, f32)>,
    /// Automated-broadcast ticker crawling along the bottom.
    pub ticker: Option<String>,
    /// Interactive graphics: current state, layers toggled off, and a paused animation time.
    pub graphic_state: String,
    pub hidden_layers: Vec<String>,
    pub paused_at: Option<f32>,
}

impl ProgramState {
    pub fn new(settings: Arc<Settings>, live_map: MapScene) -> Self {
        let now = Instant::now();
        Self {
            live_map,
            map_overlay: None,
            map_overlay_slots: [None, None],
            map_overlay_started: now,
            map_overlay_out: None,
            source: ProgramSource::LiveMap,
            source_started: now,
            transition: None,
            black: false,
            black_since: now,
            radar_time: None,
            data: Arc::new(WxSnapshot::default()),
            settings,
            panel_engines: Vec::new(),
            primary_engine: None,
            preview: None,
            blip_started: now,
            idle: false,
            caption: None,
            ticker: None,
            graphic_state: String::new(),
            hidden_layers: Vec::new(),
            paused_at: None,
        }
    }
}

/// A finished frame in BGRA (GPUI's image byte order), shared without copying.
#[derive(Clone)]
pub struct BusFrame {
    pub serial: u64,
    pub width: u32,
    pub height: u32,
    pub bgra: Arc<Vec<u8>>,
}

/// Output quality numbers (Ready for air?, status bar, Developer Logs).
#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    /// Average time to compose one frame, ms (last second).
    pub render_ms: f32,
    /// Slowest frame in the last second, ms.
    pub worst_ms: f32,
    /// Frames per second actually produced.
    pub fps: f32,
    /// Frames that took longer than the frame budget since start.
    pub late_frames: u64,
    /// Frames the recorder could not take (encoder busy) since start.
    pub recorder_drops: u64,
    /// Seconds the Program output has been solid black without Black being on (0 = fine).
    pub black_for: f32,
}

/// Straight RGBA frame for the recorder.
pub struct RecordFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone)]
pub struct Compositor {
    state: Arc<RwLock<ProgramState>>,
    frames: Arc<Mutex<(Option<BusFrame>, Option<BusFrame>)>>,
    /// Program's own map bus. Same as the map slot of `frames` unless a staged preview is up.
    onair_map: Arc<Mutex<Option<BusFrame>>>,
    recorder: Arc<Mutex<Option<SyncSender<RecordFrame>>>>,
    map_recorder: Arc<Mutex<Option<SyncSender<RecordFrame>>>>,
    serial: Arc<AtomicU64>,
    pub engine: MapEngine,
    pub stats: Arc<Mutex<FrameStats>>,
}

struct FrozenMap {
    primary: MapFrame,
    panels: Vec<MapFrame>,
}

impl Compositor {
    pub fn start(engine: MapEngine, initial: ProgramState) -> Self {
        let compositor = Self {
            state: Arc::new(RwLock::new(initial)),
            frames: Arc::new(Mutex::new((None, None))),
            onair_map: Arc::new(Mutex::new(None)),
            recorder: Arc::new(Mutex::new(None)),
            map_recorder: Arc::new(Mutex::new(None)),
            serial: Arc::new(AtomicU64::new(0)),
            engine,
            stats: Arc::new(Mutex::new(FrameStats::default())),
        };
        let worker = compositor.clone();
        std::thread::Builder::new()
            .name("compositor".into())
            .spawn(move || worker.run())
            .expect("spawn compositor");
        compositor
    }

    pub fn update(&self, f: impl FnOnce(&mut ProgramState)) {
        if let Ok(mut state) = self.state.write() {
            f(&mut state);
        }
    }

    pub fn read<R>(&self, f: impl FnOnce(&ProgramState) -> R) -> Option<R> {
        self.state.read().ok().map(|s| f(&s))
    }

    /// The map Map Studio shows (the staged preview while Preview / Take holds Program).
    pub fn map_frame(&self) -> Option<BusFrame> {
        self.frames.lock().ok()?.0.clone()
    }

    /// The map as it is on air, for public feeds and scheduled snapshots.
    pub fn onair_map_frame(&self) -> Option<BusFrame> {
        self.onair_map.lock().ok()?.clone()
    }

    pub fn export_map_motion(&self, gif: bool) -> anyhow::Result<PathBuf> {
        let mut state = self.read(Clone::clone).ok_or_else(|| anyhow::anyhow!("Map state unavailable"))?;
        // Export what Map Studio shows: the staged preview when one exists.
        let (primary_engine, panel_engines) = match state.preview.take() {
            Some(preview) => {
                state.live_map = preview.scene;
                (preview.primary, preview.panels)
            }
            None => (state.primary_engine.clone().unwrap_or_else(|| self.engine.clone()), state.panel_engines.clone()),
        };
        let engines = (&primary_engine, panel_engines.as_slice());
        let animated = state.live_map.data.field.is_some_and(|field| field.fixed_window().is_none());
        let span = forecast_span(&state.live_map, &state.data);
        let still = self.map_frame().and_then(|frame| {
            let mut rgba = frame.bgra.as_ref().clone();
            for pixel in rgba.chunks_exact_mut(4) { pixel.swap(0, 2); }
            Pixmap::from_vec(rgba, tiny_skia::IntSize::from_wh(frame.width, frame.height)?)
        }).ok_or_else(|| anyhow::anyhow!("The map has not drawn a frame yet"))?;
        let base = if animated {
            let scene = &state.live_map;
            let ready = |frame: &MapFrame, width, height| frame.view == scene.view && frame.width == width && frame.height == height;
            let primary = primary_engine.latest().filter(|frame| ready(frame, FRAME_W, FRAME_H))
                .ok_or_else(|| anyhow::anyhow!("Wait for the map to finish drawing"))?;
            let mut panels = Vec::new();
            let (width, height) = scene.layout.panel_size();
            for i in 1..scene.layout.count() {
                let panel = scene.panel_scene(i);
                if panel.data.field.is_some() {
                    anyhow::ensure!(forecast_ready(&panel, &state.data), "Wait for the comparison forecast to finish loading");
                }
                let frame = panel_engines.get(i - 1).and_then(|engine| engine.latest())
                    .filter(|frame| ready(frame, width, height))
                    .ok_or_else(|| anyhow::anyhow!("Wait for the comparison map to finish drawing"))?;
                panels.push(frame);
            }
            anyhow::ensure!(forecast_ready(scene, &state.data), "Wait for the forecast to finish loading");
            Some(FrozenMap { primary, panels })
        } else { None };
        let dir = crate::library::exports_dir();
        let name = if animated { "Forecast" } else { "Map" };
        let path = dir.join(format!("{name}_{}.{}", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%3f"), if gif { "gif" } else { "mp4" }));
        let media_dir = LibraryPaths::get().media;
        let logo = state.settings.brand.logo_media.as_ref().and_then(|name| load_image_pixmap(&media_dir.join(name)));
        let sponsor_logo = state.settings.sponsor_logo_media.as_ref().and_then(|name| load_image_pixmap(&media_dir.join(name)));
        let bindings = Bindings::build(&state.data, &state.settings, state.radar_time, &state.live_map.radar.palette);
        let mut renderer = GraphicsRenderer::new();
        let mut particles = overlays_layers::WindParticles::default();
        let mut cached = None::<(i64, Pixmap)>;
        let render = |seconds: f32| {
            let Some(base) = &base else { return still.clone() };
            let hour = ((seconds / 7.0).min(1.0) * span as f32).floor();
            let time = if state.live_map.data.field == Some(crate::map::scene::DataField::Futurecast) {
                forecast_model(&state.live_map, &state.data)
                    .and_then(|model| model.frame(hour, span, state.live_map.data.model_product))
                    .map_or(i64::MIN, |frame| frame.time)
            } else {
                hour as i64
            };
            if let Some((_, frame)) = cached.as_ref().filter(|(key, _)| *key == time) {
                return frame.clone();
            }
            state.live_map.data.hour = hour;
            let mut frame = still.clone();
            self.render_map_bus(&mut frame, &state, &bindings, &mut renderer, &media_dir, logo.as_ref(), sponsor_logo.as_ref(), &mut particles, Some(base), engines, BusStage::All);
            cached = Some((time, frame.clone()));
            frame
        };
        let result = if gif {
            crate::record::export_map_gif(render, if animated { 8.0 } else { 0.0 }, path.clone())
        } else {
            crate::record::export_animation_mp4(render, if animated { 8.0 } else { 3.0 }, 24, path.clone())
        };
        if result.is_err() { let _ = std::fs::remove_file(path); }
        result
    }

    pub fn program_frame(&self) -> Option<BusFrame> {
        self.frames.lock().ok()?.1.clone()
    }

    pub(crate) fn frames(&self) -> (Option<BusFrame>, Option<BusFrame>) {
        self.frames.lock().map(|frames| frames.clone()).unwrap_or_default()
    }

    pub fn set_recorder(&self, tx: Option<SyncSender<RecordFrame>>) {
        if let Ok(mut slot) = self.recorder.lock() {
            *slot = tx;
        }
    }

    pub fn set_map_recorder(&self, tx: Option<SyncSender<RecordFrame>>) {
        if let Ok(mut slot) = self.map_recorder.lock() { *slot = tx; }
    }

    pub fn stats(&self) -> FrameStats {
        self.stats.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn is_recording(&self) -> bool {
        self.recorder.lock().map(|r| r.is_some()).unwrap_or(false)
    }

    fn run(self) {
        let media_dir = LibraryPaths::get().media;
        let mut renderer = GraphicsRenderer::new();
        let mut last_program: Option<Arc<Pixmap>> = None;
        let mut last_program_frame: Option<BusFrame> = None;
        let mut last_map: Option<Arc<Pixmap>> = None;
        let mut last_map_frame: Option<BusFrame> = None;
        let mut last_map_key: Option<MapBusKey> = None;
        let mut last_base: Option<(MapBusKey, Arc<Pixmap>)> = None;
        let mut transition_from: Option<Arc<Pixmap>> = None;
        let mut transition_marker: Option<Instant> = None;
        let mut logo_cache: Option<(String, Option<Pixmap>)> = None;
        let mut sponsor_cache: Option<(String, Option<Pixmap>)> = None;
        let mut particles = overlays_layers::WindParticles::default();
        let mut preview_particles = overlays_layers::WindParticles::default();
        let mut last_preview: Option<BusFrame> = None;
        // The last picture each recorder was given: an unchanged frame isn't sent again (the
        // encoder stretches the previous one to its real duration), so a still map or graphic
        // costs the encoder nothing and the compositor keeps its frame rate.
        let mut last_recorded: [Option<Arc<Pixmap>>; 2] = [None, None];
        let mut clip_cache: Option<(Arc<crate::clip::Clip>, usize, Arc<Pixmap>)> = None;
        let frame_time = Duration::from_secs_f64(1.0 / FPS as f64);
        let mut window_start = Instant::now();
        let mut window_frames = 0u32;
        let mut window_ms = 0f32;
        let mut window_worst = 0f32;
        let mut black_since: Option<Instant> = None;
        let mut frame_no = 0u64;

        loop {
            let started = Instant::now();
            frame_no += 1;
            if self.state.read().map(|s| s.idle).unwrap_or(false) {
                std::thread::sleep(Duration::from_millis(250));
                continue;
            }
            let Some(state) = self.state.read().ok().map(|s| s.clone()) else {
                std::thread::sleep(frame_time);
                continue;
            };

            crate::gfx::set_text_scale(state.settings.onair_text_scale);
            let bindings = Bindings::build(
                &state.data,
                &state.settings,
                state.radar_time,
                &state.live_map.radar.palette,
            );

            // Station logo (media library) for the on-air bug.
            let logo_name = state.settings.brand.logo_media.clone();
            let cached_name = logo_cache.as_ref().map(|(name, _)| name.clone());
            if cached_name != logo_name {
                logo_cache = logo_name
                    .as_ref()
                    .map(|name| (name.clone(), load_image_pixmap(&media_dir.join(name))));
            }
            let logo = logo_cache.as_ref().and_then(|(_, pixmap)| pixmap.as_ref());

            let sponsor_name = &state.settings.sponsor_logo_media;
            if sponsor_cache.as_ref().map(|(name, _)| name) != sponsor_name.as_ref() {
                sponsor_cache = sponsor_name.as_ref().map(|name| (name.clone(), load_image_pixmap(&media_dir.join(name))));
            }
            let sponsor_logo = sponsor_cache.as_ref().and_then(|(_, pixmap)| pixmap.as_ref());

            // --- Map bus ---------------------------------------------------------------
            // Workload scheduling: while a full-screen graphic covers Program, the map bus only
            // refreshes every sixth frame (it still feeds Map Studio and is ready to cut back to).
            let map_hidden = (matches!(&state.source, ProgramSource::Graphic(d) if !d.is_overlay())
                || matches!(&state.source, ProgramSource::Clip(_))) && state.transition.is_none()
                && self.map_recorder.lock().is_ok_and(|slot| slot.is_none());
            let program_engine = state.primary_engine.clone().unwrap_or_else(|| self.engine.clone());
            // A still map (paused loop, no moving overlays) only needs drawing when one of its
            // inputs changes, or once a second for the on-air clock.
            let map_key = MapBusKey::of(&state, &program_engine);
            let unchanged = !map_key.animated && last_map_key.as_ref() == Some(&map_key);
            last_map_key = Some(map_key);
            let reuse_map = last_map.is_some() && (unchanged || (map_hidden && frame_no % 6 != 0));
            let map_pm = match (&last_map, reuse_map) {
                (Some(cached), true) => cached.clone(),
                _ => {
                    let split = state.live_map.layout == PanelLayout::Single && !MapBusKey::base_moves(&state);
                    let pm = if split {
                        // Still map from cache (redrawn only when it changes), moving pieces on top.
                        let base_key = MapBusKey::of_base(&state, &program_engine);
                        let base = match &last_base {
                            Some((key, base)) if *key == base_key => base.clone(),
                            _ => {
                                let mut base = Pixmap::new(FRAME_W, FRAME_H).expect("frame pixmap");
                                self.render_map_bus(&mut base, &state, &bindings, &mut renderer, &media_dir, logo, sponsor_logo, &mut particles, None, (&program_engine, &state.panel_engines), BusStage::Base);
                                let base = Arc::new(base);
                                last_base = Some((base_key, base.clone()));
                                base
                            }
                        };
                        let mut pm = base.as_ref().clone();
                        self.render_map_bus(&mut pm, &state, &bindings, &mut renderer, &media_dir, logo, sponsor_logo, &mut particles, None, (&program_engine, &state.panel_engines), BusStage::Moving);
                        pm
                    } else {
                        last_base = None;
                        let mut pm = Pixmap::new(FRAME_W, FRAME_H).expect("frame pixmap");
                        self.render_map_bus(&mut pm, &state, &bindings, &mut renderer, &media_dir, logo, sponsor_logo, &mut particles, None, (&program_engine, &state.panel_engines), BusStage::All);
                        pm
                    };
                    let pm = Arc::new(pm);
                    last_map = Some(pm.clone());
                    pm
                }
            };

            // --- Preview bus (Preview / Take) ------------------------------------------
            // The staged map only feeds Map Studio, so it refreshes at half rate.
            let preview_frame = match &state.preview {
                None => {
                    last_preview = None;
                    None
                }
                Some(preview) => {
                    if last_preview.is_none() || frame_no % 2 == 0 {
                        let mut staged = state.clone();
                        staged.live_map = preview.scene.clone();
                        staged.preview = None;
                        let mut pm = Pixmap::new(FRAME_W, FRAME_H).expect("frame pixmap");
                        self.render_map_bus(&mut pm, &staged, &bindings, &mut renderer, &media_dir, logo, sponsor_logo, &mut preview_particles, None, (&preview.primary, &preview.panels), BusStage::All);
                        last_preview = Some(self.publish(&pm));
                    }
                    last_preview.clone()
                }
            };

            // --- Program bus -------------------------------------------------------------
            if !matches!(&state.source, ProgramSource::Clip(_)) { clip_cache = None; }
            let mut program_pm = match &state.source {
                ProgramSource::LiveMap => map_pm.clone(),
                ProgramSource::Clip(clip) => {
                    let index = clip.frame_index(state.source_started.elapsed().as_secs_f32());
                    if let Some((_, _, frame)) = clip_cache.as_ref().filter(|(cached, i, _)| Arc::ptr_eq(cached, clip) && *i == index) {
                        frame.clone()
                    } else {
                        let mut frame = Pixmap::new(FRAME_W, FRAME_H).expect("frame pixmap");
                        frame.fill(Color::BLACK);
                        if let Err(error) = clip.draw(index, &mut frame) { log::warn!("Clip frame: {error}"); }
                        let frame = Arc::new(frame);
                        clip_cache = Some((clip.clone(), index, frame.clone()));
                        frame
                    }
                },
                ProgramSource::Graphic(doc) => {
                    let resolved_doc = if doc.inherits.is_some() { Some(doc.resolved()) } else { None };
                    let doc = resolved_doc.as_ref().unwrap_or(doc);
                    let stated = (!doc.states.is_empty() || !state.hidden_layers.is_empty()).then(|| doc.in_state(&state.graphic_state, &state.hidden_layers));
                    let doc = stated.as_ref().unwrap_or(doc);
                    let mut pm = if doc.is_overlay() {
                        // An overlay on its own goes over the live map.
                        map_pm.as_ref().clone()
                    } else {
                        let mut pm = Pixmap::new(FRAME_W, FRAME_H).expect("frame pixmap");
                        pm.fill(Color::BLACK);
                        pm
                    };
                    // A graphic made for another location gets its own bindings.
                    let placed = doc.place.as_ref().map(|p| {
                        Bindings::build(&state.data, &p.apply(&state.settings), state.radar_time, &state.live_map.radar.palette)
                    });
                    let ctx = RenderCtx {
                        bindings: placed.as_ref().unwrap_or(&bindings),
                        brand: &state.settings.brand,
                        time: state.paused_at.unwrap_or_else(|| state.source_started.elapsed().as_secs_f32()),
                        out_time: None,
                        media_dir: &media_dir,
                    };
                    renderer.render(&mut pm, doc, &ctx);
                    Arc::new(pm)
                }
            };

            // Transitions between program sources.
            if let Some(t) = &state.transition {
                if transition_marker != Some(t.started) {
                    transition_marker = Some(t.started);
                    transition_from = last_program.clone();
                }
                let p = (t.started.elapsed().as_secs_f32() / t.seconds.max(0.01)).clamp(0.0, 1.0);
                if let (Some(from), true) = (&transition_from, p < 1.0) {
                    match t.kind {
                        Transition::Fade => {
                            let paint = PixmapPaint {
                                opacity: 1.0 - p,
                                ..PixmapPaint::default()
                            };
                            Arc::make_mut(&mut program_pm).draw_pixmap(0, 0, from.as_ref().as_ref(), &paint, Transform::identity(), None);
                        }
                        Transition::Wipe => {
                            // The new source wipes in from the left.
                            let edge = FRAME_W as f32 * crate::gfx::ease_out(p);
                            if let (Some(mut mask), Some(rect)) = (
                                Mask::new(FRAME_W, FRAME_H),
                                Rect::from_xywh(edge, 0.0, FRAME_W as f32 - edge + 1.0, FRAME_H as f32),
                            ) {
                                let path = PathBuilder::from_rect(rect);
                                mask.fill_path(&path, FillRule::Winding, false, Transform::identity());
                                Arc::make_mut(&mut program_pm).draw_pixmap(0, 0, from.as_ref().as_ref(), &PixmapPaint::default(), Transform::identity(), Some(&mask));
                            }
                        }
                        Transition::Cut | Transition::Fly => {}
                    }
                }
            }

            if let Some(text) = &state.ticker {
                draw_ticker(Arc::make_mut(&mut program_pm), renderer.glyphs(), text, frame_no, &state.settings.brand);
            }
            // Live captions sit over everything except the fade to black.
            if let Some((text, top, size)) = &state.caption {
                draw_caption(Arc::make_mut(&mut program_pm), renderer.glyphs(), text, *top, *size, &state.settings.brand.font_body);
            }
            // A practice warning can never pass for a real one on air.
            if state.data.local_alerts.iter().chain(state.data.map_alerts.iter()).any(|a| a.is_practice()) {
                draw_practice_banner(Arc::make_mut(&mut program_pm), renderer.glyphs(), &state.settings.brand.font_display);
            }

            // Fade to / from black.
            let black_p = (state.black_since.elapsed().as_secs_f32() / 0.5).clamp(0.0, 1.0);
            let black_alpha = if state.black { black_p } else { 1.0 - black_p };
            if black_alpha > 0.0 {
                if let Some(rect) = Rect::from_xywh(0.0, 0.0, FRAME_W as f32, FRAME_H as f32) {
                    let mut paint = tiny_skia::Paint::default();
                    paint.set_color(Color::from_rgba(0.0, 0.0, 0.0, black_alpha).unwrap_or(Color::BLACK));
                    let program_pm = Arc::make_mut(&mut program_pm);
                    program_pm.fill_rect(rect, &paint, Transform::identity(), None);
                    if black_alpha >= 1.0 {
                        program_pm.fill(Color::BLACK);
                    }
                }
            }

            // Publish.
            if !reuse_map || last_map_frame.is_none() {
                last_map_frame = Some(self.publish(&map_pm));
            }
            let map_frame = last_map_frame.clone().expect("published map frame");
            let program_frame = if Arc::ptr_eq(&program_pm, &map_pm) {
                map_frame.clone()
            } else if last_program.as_ref().is_some_and(|last| Arc::ptr_eq(last, &program_pm)) {
                last_program_frame.clone().unwrap_or_else(|| self.publish(&program_pm))
            } else {
                self.publish(&program_pm)
            };
            last_program_frame = Some(program_frame.clone());
            if let Ok(mut frames) = self.frames.lock() {
                *frames = (Some(preview_frame.unwrap_or_else(|| map_frame.clone())), Some(program_frame));
            }
            if let Ok(mut slot) = self.onair_map.lock() {
                *slot = Some(map_frame);
            }

            // Separate recorders keep a replay export off Program.
            for (n, (slot, image)) in [(&self.recorder, &program_pm), (&self.map_recorder, &map_pm)].into_iter().enumerate() {
            if let Ok(mut recorder) = slot.lock() {
                if recorder.is_none() {
                    last_recorded[n] = None;
                    continue;
                }
                let unchanged = last_recorded[n].as_ref().is_some_and(|last| Arc::ptr_eq(last, image))
                    // Re-send a still picture once a second so the encoder never waits long.
                    && frame_no % (FPS as u64) != 0;
                if unchanged {
                    continue;
                }
                last_recorded[n] = Some(image.clone());
                if let Some(tx) = recorder.as_ref() {
                    let frame = RecordFrame {
                        width: FRAME_W,
                        height: FRAME_H,
                        rgba: image.data().to_vec(),
                    };
                    match tx.try_send(frame) {
                        Err(TrySendError::Disconnected(_)) => *recorder = None,
                        Err(TrySendError::Full(_)) => {
                            if let Ok(mut st) = self.stats.lock() {
                                st.recorder_drops += 1;
                            }
                        }
                        Ok(()) => {}
                    }
                }
            }

            }

            // Black-frame detection: sample the Program output twice a second.
            if frame_no % 15 == 0 {
                let data = program_pm.data();
                let mut sum = 0u64;
                let mut n = 0u64;
                let mut i = 0;
                while i + 3 < data.len() {
                    sum += data[i] as u64 + data[i + 1] as u64 + data[i + 2] as u64;
                    n += 3;
                    i += 4 * 997;
                }
                let mean = if n > 0 { sum / n } else { 0 };
                if mean <= 2 && !state.black {
                    black_since.get_or_insert_with(Instant::now);
                } else {
                    black_since = None;
                }
            }

            last_program = Some(program_pm);

            let spent = started.elapsed();
            let ms = spent.as_secs_f32() * 1000.0;
            window_frames += 1;
            window_ms += ms;
            window_worst = window_worst.max(ms);
            if window_start.elapsed() >= Duration::from_secs(1) {
                let secs = window_start.elapsed().as_secs_f32();
                if let Ok(mut st) = self.stats.lock() {
                    st.render_ms = window_ms / window_frames.max(1) as f32;
                    st.worst_ms = window_worst;
                    st.fps = window_frames as f32 / secs;
                    st.black_for = black_since.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0);
                }
                window_start = Instant::now();
                window_frames = 0;
                window_ms = 0.0;
                window_worst = 0.0;
            }
            // Low-resource mode composes at half rate.
            let budget = if state.settings.low_resource { frame_time * 2 } else { frame_time };
            if spent > budget {
                if let Ok(mut st) = self.stats.lock() {
                    st.late_frames += 1;
                }
            }
            if spent < budget {
                std::thread::sleep(budget - spent);
            }
        }
    }

    fn publish(&self, pm: &Pixmap) -> BusFrame {
        // Opaque frames: premultiplied == straight; swap to BGRA for GPUI.
        let mut bgra = pm.data().to_vec();
        for px in bgra.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        BusFrame {
            serial: self.serial.fetch_add(1, Ordering::Relaxed) + 1,
            width: pm.width(),
            height: pm.height(),
            bgra: Arc::new(bgra),
        }
    }

    fn render_map_bus(
        &self,
        pm: &mut Pixmap,
        state: &ProgramState,
        bindings: &Bindings,
        renderer: &mut GraphicsRenderer,
        media_dir: &PathBuf,
        logo: Option<&Pixmap>,
        sponsor_logo: Option<&Pixmap>,
        particles: &mut overlays_layers::WindParticles,
        frozen_base: Option<&FrozenMap>,
        (primary_engine, panel_engines): (&MapEngine, &[MapEngine]),
        stage: BusStage,
    ) {
        let scene = &state.live_map;
        if stage == BusStage::Moving {
            // Only the call-out pulse and the overlay graphics, over the cached still map.
            let view = primary_engine.latest().filter(|f| f.width == FRAME_W && f.height == FRAME_H).map_or(scene.view, |f| f.view);
            let vp = Viewport::new(view, FRAME_W, FRAME_H);
            draw_alert_pulse(pm, renderer.glyphs(), &vp, scene, state);
            self.render_map_overlays(pm, state, bindings, renderer, media_dir);
            return;
        }
        let settings = &state.settings;
        let background = Color::from_rgba8(11, 15, 24, 255);
        pm.fill(background);
        let glyphs = renderer.glyphs();
        let layout = scene.layout;

        if layout == PanelLayout::Single {
            // The map frame and the camera it was rendered with.
            let view = match frozen_base.map(|map| map.primary.clone()).or_else(|| primary_engine.latest()) {
                Some(frame) if frame.width == FRAME_W && frame.height == FRAME_H => {
                    pm.data_mut().copy_from_slice(&frame.rgba);
                    frame.view
                }
                _ => scene.view,
            };
            let vp = Viewport::new(view, FRAME_W, FRAME_H);
            let data_time = draw_data_under(pm, glyphs, &vp, scene, state, particles, None);
            overlays::draw_scene_layers(pm, glyphs, &vp, scene, &state.data, &settings.brand, state.radar_time);
            draw_data_over(pm, glyphs, &vp, scene, state, stage != BusStage::Base);
            overlays::draw_raised_area(pm, &vp, scene, state.radar_time);
            // A lower third on the map (warning call-out, overlay graphic) takes the bottom of
            // the frame: lift the legend, time box and clock above it instead of under it.
            match overlay_bottom_lift(state) {
                Some(lift) => {
                    let mut lifted = scene.clone();
                    for key in ["legend", "time", "clock"] {
                        let (dx, dy) = lifted.onair_offsets.get(key).copied().unwrap_or((0.0, 0.0));
                        lifted.onair_offsets.insert(key.to_string(), (dx, dy - lift));
                    }
                    overlays::draw_onair(pm, glyphs, &lifted, settings, state.radar_time, data_time, logo, sponsor_logo);
                }
                None => overlays::draw_onair(pm, glyphs, scene, settings, state.radar_time, data_time, logo, sponsor_logo),
            }
        } else {
            let (pw, ph) = layout.panel_size();
            let time_label = state.radar_time.and_then(|t| chrono::DateTime::from_timestamp(t, 0)).map(|t| {
                let local = t.with_timezone(&chrono::Local);
                if settings.clock_24h {
                    local.format("%H:%M").to_string()
                } else {
                    local.format("%-I:%M %p").to_string()
                }
            });
            let mut primary_valid = None;
            for i in 0..layout.count() {
                let panel_scene = scene.panel_scene(i);
                let Some(mut sub) = Pixmap::new(pw, ph) else { continue };
                sub.fill(background);
                let view = if i == 0 {
                    // Panel 1 is the centre of the main broadcast-size map at the same zoom.
                    match frozen_base.map(|map| map.primary.clone()).or_else(|| primary_engine.latest()) {
                        Some(frame) if frame.width == FRAME_W && frame.height == FRAME_H => {
                            copy_center(&frame.rgba, FRAME_W, FRAME_H, &mut sub);
                            frame.view
                        }
                        _ => scene.view,
                    }
                } else {
                    match frozen_base.and_then(|map| map.panels.get(i - 1).cloned()).or_else(|| panel_engines.get(i - 1).and_then(|e| e.latest())) {
                        Some(frame) if frame.width == pw && frame.height == ph => {
                            sub.data_mut().copy_from_slice(&frame.rgba);
                            frame.view
                        }
                        _ => scene.view,
                    }
                };
                let vp = Viewport::new(view, pw, ph);
                let data_time = if i == 0 || panel_scene.data.field.is_some() {
                    draw_data_under(&mut sub, glyphs, &vp, &panel_scene, state, particles, primary_valid)
                } else {
                    None
                };
                overlays::draw_scene_layers(&mut sub, glyphs, &vp, &panel_scene, &state.data, &settings.brand, state.radar_time);
                if i == 0 {
                    if scene.data.field == Some(crate::map::scene::DataField::Futurecast) { primary_valid = data_time; }
                    draw_data_over(&mut sub, glyphs, &vp, &panel_scene, state, true);
                }
                overlays::draw_raised_area(&mut sub, &vp, &panel_scene, state.radar_time);
                let mut title = scene.panel_title(i);
                if panel_scene.data.field == Some(crate::map::scene::DataField::Futurecast) {
                    if let Some(run) = forecast_model(&panel_scene, &state.data)
                        .and_then(|model| chrono::DateTime::from_timestamp(model.run, 0)) {
                        title.push_str(&format!(" · {}", run.format("%HZ")));
                    }
                    if data_time.is_none() { title.push_str(" · NO FRAME"); }
                }
                let data_label = data_time.and_then(|t| chrono::DateTime::from_timestamp(t, 0)).map(|t| {
                    t.with_timezone(&chrono::Local).format(if settings.clock_24h { "%a %H:%M" } else { "%a %-I %p" }).to_string()
                });
                let time = if data_label.is_some() {
                    data_label.as_deref()
                } else if panel_scene.radar.enabled && panel_scene.radar.source == crate::map::scene::RadarSource::Composite {
                    time_label.as_deref()
                } else {
                    None
                };
                if layout == PanelLayout::Wipe {
                    // Slider: panel 2 shows right of the divider; a small label sits on each side.
                    let x = (scene.wipe.clamp(0.02, 0.98) * FRAME_W as f32).round();
                    if i == 0 {
                        pm.draw_pixmap(0, 0, sub.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
                    } else if let (Some(mut mask), Some(rect)) = (Mask::new(FRAME_W, FRAME_H), Rect::from_xywh(x, 0.0, FRAME_W as f32 - x, FRAME_H as f32)) {
                        mask.fill_path(&PathBuilder::from_rect(rect), FillRule::Winding, false, Transform::identity());
                        pm.draw_pixmap(0, 0, sub.as_ref(), &PixmapPaint::default(), Transform::identity(), Some(&mask));
                        let mut line = tiny_skia::Paint::default();
                        line.set_color(Color::WHITE);
                        if let Some(r) = Rect::from_xywh(x - 3.0, 0.0, 6.0, FRAME_H as f32) {
                            pm.fill_rect(r, &line, Transform::identity(), None);
                        }
                        let st = crate::map::overlays::style(&settings.brand.font_display, crate::gfx::text::Weight::Bold, 30.0, Color::WHITE);
                        let left = scene.panel_title(0);
                        let lw = crate::gfx::text::measure(&left, &st);
                        crate::map::overlays::halo_text(pm, glyphs, &left, (x - lw - 24.0).max(20.0), 70.0, &st);
                        crate::map::overlays::halo_text(pm, glyphs, &title, x + 24.0, 70.0, &st);
                    }
                    continue;
                }
                overlays::draw_panel_header(&mut sub, glyphs, &title, time, &settings.brand);
                let (ox, oy) = layout.panel_origin(i);
                pm.draw_pixmap(ox as i32, oy as i32, sub.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
            }
            // Panel dividers.
            let mut divider = tiny_skia::Paint::default();
            divider.set_color(Color::from_rgba8(5, 7, 11, 255));
            if let Some(rect) = Rect::from_xywh(pw as f32 - 2.0, 0.0, 4.0, FRAME_H as f32) {
                pm.fill_rect(rect, &divider, Transform::identity(), None);
            }
            if layout == PanelLayout::Quad {
                if let Some(rect) = Rect::from_xywh(0.0, ph as f32 - 2.0, FRAME_W as f32, 4.0) {
                    pm.fill_rect(rect, &divider, Transform::identity(), None);
                }
            }
            // Station bug and clock stay; banner/legend/time move into the panel headers.
            let mut chrome = scene.clone();
            chrome.show_title = false;
            chrome.show_legend = false;
            chrome.show_timestamp = false;
            overlays::draw_onair(pm, glyphs, &chrome, settings, state.radar_time, None, logo, sponsor_logo);
        }

        if stage != BusStage::Base {
            self.render_map_overlays(pm, state, bindings, renderer, media_dir);
        }
    }

    /// Overlay graphics (lower third, crawl, bug…) on top of the map.
    fn render_map_overlays(&self, pm: &mut Pixmap, state: &ProgramState, bindings: &Bindings, renderer: &mut GraphicsRenderer, media_dir: &PathBuf) {
        let scene = &state.live_map;
        let settings = &state.settings;
        let primary = state.map_overlay.as_ref().map(|doc| (doc, state.map_overlay_started));
        let slots = state.map_overlay_slots.iter().map(|slot| slot.as_ref().map(|(doc, started)| (doc, *started)));
        for (key, overlay) in MapScene::OVERLAY_POSITIONS.into_iter().zip(std::iter::once(primary).chain(slots)) {
            let Some((doc, started)) = overlay else { continue };
            let ctx = RenderCtx {
                bindings,
                brand: &settings.brand,
                time: started.elapsed().as_secs_f32(),
                out_time: None,
                media_dir,
            };
            let (dx, dy) = scene.onair_offsets.get(key).copied().unwrap_or_default();
            if dx.abs() < 0.5 && dy.abs() < 0.5 {
                renderer.render(pm, doc, &ctx);
            } else if let Some(mut layer) = Pixmap::new(FRAME_W, FRAME_H) {
                // Dragged with the Arrange tool: render apart and place at the offset.
                renderer.render(&mut layer, doc, &ctx);
                pm.draw_pixmap(dx as i32, dy as i32, layer.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
            }
        }
        if let Some((doc, since)) = &state.map_overlay_out {
            let out = since.elapsed().as_secs_f32();
            if out < 1.5 {
                let ctx = RenderCtx {
                    bindings,
                    brand: &settings.brand,
                    time: crate::gfx::render::SETTLED,
                    out_time: Some(out),
                    media_dir,
                };
                renderer.render(pm, doc, &ctx);
            }
        }
    }
}

/// Data layers that sit under the warnings and telestration: the shaded field, isobars and
/// wind particles. Returns the valid time of the data shown.
pub fn forecast_model<'a>(scene: &MapScene, snapshot: &'a WxSnapshot) -> Option<&'a crate::futurecast::ModelLoop> {
    let source = scene.data.model_product.source();
    let pin = scene.data.model_pins[source.index()];
    snapshot.model(source).filter(|model| {
        model.region.covers(scene.view) && model.hours >= scene.data.loop_hours && pin.is_none_or(|pin| pin == model.pin())
    })
}

pub fn forecast_ready(scene: &MapScene, snapshot: &WxSnapshot) -> bool {
    match scene.data.field {
        Some(crate::map::scene::DataField::Futurecast) => forecast_model(scene, snapshot).is_some_and(|model| {
            model.frame(0.0, scene.data.loop_hours, scene.data.model_product).is_some()
        }),
        Some(field) => snapshot.grid.as_ref().is_some_and(|grid| {
            grid.covers(scene.view) && (field.fixed_window().is_some() || grid.available_hours(Some(field)) > 0)
        }),
        None => false,
    }
}

pub fn forecast_span(scene: &MapScene, snapshot: &WxSnapshot) -> u32 {
    if scene.data.field.is_some_and(|field| field.fixed_window().is_some()) { return 0; }
    let requested = scene.data.loop_hours;
    if scene.data.field == Some(crate::map::scene::DataField::Futurecast) {
        forecast_model(scene, snapshot)
            .map_or(0, |model| model.timeline_hours(requested, scene.data.model_product).last().unwrap_or(0))
    } else {
        snapshot.grid.as_ref().filter(|grid| grid.covers(scene.view))
            .map_or(requested.min(crate::grid::MAX_FORECAST_HOURS), |grid| requested.min(grid.available_hours(scene.data.field)))
    }
}

/// How far (frame px) to lift the bottom on-air elements so they clear an overlay graphic's
/// lower third: None when no overlay reaches into the bottom of the frame.
fn overlay_bottom_lift(state: &ProgramState) -> Option<f32> {
    let docs = state.map_overlay.iter().chain(state.map_overlay_slots.iter().flatten().map(|(doc, _)| doc));
    let mut top: Option<f32> = None;
    for doc in docs {
        let scale = FRAME_H as f32 / doc.height.max(1) as f32;
        for layer in doc.layers.iter().filter(|l| l.visible && l.h > 0.0) {
            let (y, bottom, width) = (layer.y * scale, (layer.y + layer.h) * scale, layer.w * scale);
            // Wide bars in the bottom third (a lower third), not small corner bugs.
            if y > FRAME_H as f32 * 0.55 && bottom > FRAME_H as f32 * 0.8 && width > FRAME_W as f32 * 0.3 {
                top = Some(top.map_or(y, |t: f32| t.min(y)));
            }
        }
    }
    // The legend's bottom sits 40 px above the frame edge; keep a 14 px gap above the bar.
    top.map(|t| ((FRAME_H as f32 - 40.0) - (t - 14.0)).max(0.0)).filter(|lift| *lift > 0.0)
}

fn draw_data_under(
    pm: &mut Pixmap,
    glyphs: &mut crate::gfx::text::GlyphCache,
    vp: &Viewport,
    scene: &MapScene,
    state: &ProgramState,
    particles: &mut overlays_layers::WindParticles,
    shared_valid: Option<i64>,
) -> Option<i64> {
    let data = &scene.data;
    if data.field.is_none() && !data.wind_particles && !data.isobars && !data.wind_barbs {
        return None;
    }
    let model_mode = data.field == Some(crate::map::scene::DataField::Futurecast);
    let model_time = if model_mode {
        forecast_model(scene, &state.data).filter(|model| model.region.covers(vp.view)).and_then(|model| {
            let frame = match shared_valid {
                Some(time) => model.exact_frame(time).filter(|frame| frame.has_product(data.model_product))?,
                None => model.frame(data.hour, data.loop_hours, data.model_product)?,
            };
            overlays_layers::render_futurecast(pm, vp, frame, data.model_product, data.opacity);
            Some(frame.time)
        })
    } else {
        None
    };
    let Some(grid) = state.data.grid.as_ref().filter(|grid| grid.covers(vp.view)) else { return model_time };
    let tf = grid.time_index(data.hour);
    if let Some(field) = data.field.filter(|_| !model_mode) {
        overlays_layers::render_field(pm, vp, grid, field, tf, data.opacity);
    }
    if data.isobars || data.field == Some(crate::map::scene::DataField::Pressure) {
        overlays_layers::draw_isobars(pm, glyphs, vp, grid, tf, &state.settings.brand);
    }
    if data.wind_particles && !state.settings.low_resource {
        particles.step_and_draw(pm, vp, grid, tf);
    }
    if data.wind_barbs {
        overlays_layers::draw_wind_barbs(pm, vp, grid, tf);
    }
    if model_mode { model_time } else { data.field.filter(|field| field.fixed_window().is_none()).map(|_| grid.time_at(tf)) }
}

/// Data layers drawn on top: city value plots and the alert call-out.
fn draw_data_over(pm: &mut Pixmap, glyphs: &mut crate::gfx::text::GlyphCache, vp: &Viewport, scene: &MapScene, state: &ProgramState, pulse: bool) {
    if let (true, Some(grid)) = (scene.snow_band, state.data.grid.as_ref()) {
        crate::map::extras::draw_snow_band(pm, glyphs, vp, grid, &state.settings.brand);
    }
    if let (true, Some(grid)) = (scene.urban_flood, state.data.grid.as_ref()) {
        crate::map::extras::draw_urban_flood(pm, glyphs, vp, grid, &state.settings.brand);
    }
    if !scene.ens_map.is_empty() {
        crate::map::extras::draw_ensemble_towns(pm, glyphs, vp, &scene.ens_map, &state.settings.brand);
    }
    if scene.cameras {
        crate::map::extras::draw_cameras(pm, glyphs, vp, &state.settings.webcams, &state.settings.brand);
    }
    if scene.metar_plots {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        crate::map::overlays::view_key(vp).hash(&mut h);
        for m in &state.data.metars {
            (&m.id, m.obs_time).hash(&mut h);
        }
        crate::map::overlays::cached_layer("metars", h.finish(), pm, |sheet| {
            crate::map::extras::draw_metar_plots(sheet, glyphs, vp, &state.data.metars, &state.settings.brand)
        });
    }
    if let (true, Some(grid)) = (scene.impact_callouts, state.data.grid.as_ref()) {
        let tf = grid.time_index(scene.data.hour);
        overlays_layers::draw_impact_callouts(pm, glyphs, vp, grid, tf, &state.settings.brand);
    }
    if let (Some(field), true, Some(grid)) = (scene.data.field.filter(|f| *f != crate::map::scene::DataField::Futurecast), scene.data.city_plots, state.data.grid.as_ref()) {
        let tf = grid.time_index(scene.data.hour);
        // Keep plates out from under the title, legend, logo, time box and clock.
        let reserved: Vec<(f32, f32, f32, f32)> = crate::map::overlays::onair_boxes(scene, vp.width as f32, vp.height as f32, true, true)
            .into_iter()
            .map(|(_, r)| r)
            .collect();
        overlays_layers::draw_city_plots(pm, glyphs, vp, grid, field, tf, &state.settings.brand, &reserved);
    }
    if let Some(id) = &scene.selected_alert {
        let alert = state
            .data
            .map_alerts
            .iter()
            .chain(state.data.local_alerts.iter())
            .find(|a| &a.id == id)
            // A called-out alert follows the alert switches: turning Watches off hides a
            // called-out watch too.
            .filter(|a| scene.warnings && scene.alert_kinds.allows(a));
        if let (Some(alert), true) = (alert, scene.warning_towns) {
            crate::map::extras::draw_warning_towns(pm, glyphs, vp, alert, &state.settings.brand);
        }
        if let (Some(_), true) = (alert, pulse) {
            draw_alert_pulse(pm, glyphs, vp, scene, state);
        }
    }
}

/// The called-out alert's pulsing marker.
fn draw_alert_pulse(pm: &mut Pixmap, glyphs: &mut crate::gfx::text::GlyphCache, vp: &Viewport, scene: &MapScene, state: &ProgramState) {
    let Some(id) = &scene.selected_alert else { return };
    let Some(alert) = state.data.map_alerts.iter().chain(state.data.local_alerts.iter()).find(|a| &a.id == id) else { return };
    if !(scene.warnings && scene.alert_kinds.allows(alert)) {
        return;
    }
    overlays_layers::draw_alert_blip(pm, glyphs, vp, alert, state.blip_started.elapsed().as_secs_f32(), &state.settings.brand, state.settings.clock_24h);
}

/// Conditions ticker: a dark band along the bottom with the text crawling right to left.
fn draw_ticker(pm: &mut Pixmap, glyphs: &mut crate::gfx::text::GlyphCache, text: &str, frame: u64, brand: &crate::config::Brand) {
    use crate::gfx::text::{draw_line, measure, TextStyle, Weight};
    let (w, h) = (FRAME_W as f32, FRAME_H as f32);
    let band = 64.0;
    if let Some(rect) = Rect::from_xywh(0.0, h - band, w, band) {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color(crate::gfx::with_alpha(crate::gfx::parse_color("$panel", brand), 0.92));
        pm.fill_rect(rect, &paint, Transform::identity(), None);
    }
    let style = TextStyle { family: brand.font_body.clone(), weight: Weight::SemiBold, size: 34.0, color: crate::gfx::parse_color("$text", brand), letter_spacing: 0.0 };
    let tw = measure(text, &style) + w * 0.3;
    let x = w - ((frame as f32 * 4.0) % (tw + w));
    draw_line(pm, glyphs, text, x, h - band / 2.0 + 12.0, &style, 1.0, None);
}

/// "PRACTICE · NOT A REAL WARNING" across the top of Program while a practice warning exists.
fn draw_practice_banner(pm: &mut Pixmap, glyphs: &mut crate::gfx::text::GlyphCache, family: &str) {
    use crate::gfx::text::{draw_line, measure, TextStyle, Weight};
    // Kept to 40 px so it stays above graphic titles (a full-screen warning title starts near y = 45).
    let (w, band) = (FRAME_W as f32, 40.0);
    if let Some(rect) = Rect::from_xywh(0.0, 0.0, w, band) {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color(Color::from_rgba8(255, 214, 0, 255));
        pm.fill_rect(rect, &paint, Transform::identity(), None);
    }
    let style = TextStyle { family: family.to_string(), weight: Weight::Black, size: 28.0, color: Color::from_rgba8(12, 12, 12, 255), letter_spacing: 3.0 };
    let text = "PRACTICE  ·  NOT A REAL WARNING  ·  REHEARSAL ONLY";
    let tw = measure(text, &style);
    draw_line(pm, glyphs, text, (w - tw) / 2.0, band / 2.0 + 10.0, &style, 1.0, None);
}

/// Caption box: up to two lines, white on a dark band, inside the title-safe area.
fn draw_caption(pm: &mut Pixmap, glyphs: &mut crate::gfx::text::GlyphCache, text: &str, top: bool, size: f32, family: &str) {
    use crate::gfx::text::{draw_line, line_metrics, wrap, TextStyle, Weight};
    let style = TextStyle {
        family: family.to_string(),
        weight: Weight::SemiBold,
        size: 40.0 * size.clamp(0.5, 2.0),
        color: Color::WHITE,
        letter_spacing: 0.0,
    };
    let (w, h) = (FRAME_W as f32, FRAME_H as f32);
    let max_w = w * 0.7;
    let mut lines = wrap(text, &style, max_w);
    // Roll-up style: only the newest two lines stay on screen.
    if lines.len() > 2 {
        lines.drain(..lines.len() - 2);
    }
    if lines.is_empty() {
        return;
    }
    let (ascent, descent) = line_metrics(&style);
    let line_h = (ascent + descent) * 1.2;
    let pad = 16.0;
    let box_h = line_h * lines.len() as f32 + pad * 2.0;
    let widest = lines.iter().map(|l| crate::gfx::text::measure(l, &style)).fold(0.0, f32::max);
    let box_w = widest + pad * 3.0;
    let x = (w - box_w) / 2.0;
    let y = if top { h * 0.07 } else { h * 0.93 - box_h - h * 0.12 };
    if let Some(rect) = Rect::from_xywh(x, y, box_w, box_h) {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color(Color::from_rgba8(0, 0, 0, 200));
        pm.fill_rect(rect, &paint, Transform::identity(), None);
    }
    for (i, line) in lines.iter().enumerate() {
        let lw = crate::gfx::text::measure(line, &style);
        let baseline = y + pad + line_h * i as f32 + ascent;
        draw_line(pm, glyphs, line, (w - lw) / 2.0, baseline, &style, 1.0, None);
    }
}

/// Copy the centre `dst`-sized region of an RGBA frame into `dst`.
fn copy_center(src: &[u8], src_w: u32, src_h: u32, dst: &mut Pixmap) {
    let (dw, dh) = (dst.width().min(src_w), dst.height().min(src_h));
    let x0 = ((src_w - dw) / 2) as usize;
    let y0 = ((src_h - dh) / 2) as usize;
    let dst_stride = dst.width() as usize * 4;
    let src_stride = src_w as usize * 4;
    let data = dst.data_mut();
    for row in 0..dh as usize {
        let s = (y0 + row) * src_stride + x0 * 4;
        let d = row * dst_stride;
        data[d..d + dw as usize * 4].copy_from_slice(&src[s..s + dw as usize * 4]);
    }
}

/// One-off render of a graphic (editor preview, thumbnails, PNG export).
pub fn render_graphic(
    doc: &GraphicDoc,
    settings: &Settings,
    data: &WxSnapshot,
    time: f32,
    width: u32,
    height: u32,
    background: Option<&Pixmap>,
) -> Pixmap {
    render_graphic_mode(doc, settings, data, time, width, height, background, crate::gfx::check::PreviewData::Live)
}

/// [`render_graphic`] with test data (Graphics Studio > Preview with).
#[allow(clippy::too_many_arguments)]
pub fn render_graphic_mode(
    doc: &GraphicDoc,
    settings: &Settings,
    data: &WxSnapshot,
    time: f32,
    width: u32,
    height: u32,
    background: Option<&Pixmap>,
    mode: crate::gfx::check::PreviewData,
) -> Pixmap {
    thread_local! {
        static RENDERER: std::cell::RefCell<GraphicsRenderer> = std::cell::RefCell::new(GraphicsRenderer::new());
    }
    let media_dir = LibraryPaths::get().media;
    let placed = doc.place.as_ref().map(|p| p.apply(settings));
    let settings = placed.as_ref().unwrap_or(settings);
    let resolved;
    let doc = if doc.inherits.is_some() {
        resolved = doc.resolved();
        &resolved
    } else {
        doc
    };
    let live = Bindings::build(data, settings, data.radar.as_ref().and_then(|r| r.latest_past()).map(|f| f.time), &settings.radar_palette);
    let mut bindings = crate::gfx::check::preview_bindings(&live, mode);
    // Template data source configuration.
    for (key, value) in &doc.data_overrides {
        let v = bindings.apply(value);
        bindings.set(key.trim().trim_start_matches("{{").trim_end_matches("}}"), &v);
    }
    let mut pm = Pixmap::new(width.max(1), height.max(1)).expect("pixmap");
    if let Some(bg) = background {
        let sx = width as f32 / bg.width() as f32;
        let sy = height as f32 / bg.height() as f32;
        pm.draw_pixmap(0, 0, bg.as_ref(), &PixmapPaint::default(), Transform::from_scale(sx, sy), None);
    }
    RENDERER.with(|r| {
        let mut renderer = r.borrow_mut();
        renderer.clear_media();
        let ctx = RenderCtx {
            bindings: &bindings,
            brand: &settings.brand,
            time,
            out_time: None,
            media_dir: &media_dir,
        };
        renderer.render(&mut pm, doc, &ctx);
    });
    pm
}

/// Checkerboard used behind transparent overlays in the editor.
pub fn checkerboard(width: u32, height: u32) -> Pixmap {
    let mut pm = Pixmap::new(width, height).expect("pixmap");
    let data = pm.data_mut();
    for y in 0..height {
        for x in 0..width {
            let dark = ((x / 32) + (y / 32)) % 2 == 0;
            let v = if dark { 38 } else { 52 };
            let i = ((y * width + x) * 4) as usize;
            data[i] = v;
            data[i + 1] = v;
            data[i + 2] = v + 6;
            data[i + 3] = 255;
        }
    }
    pm
}
