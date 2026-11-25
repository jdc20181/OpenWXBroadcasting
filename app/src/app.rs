//! Application state and the background loops (data polling, radar animation, frame pump).

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use gpui_kit::*;

use crate::{
    config::{GeoView, LibraryPaths, Settings},
    data::{DataHealth, FeedStatus, WxSnapshot},
    gfx::model::GraphicDoc,
    library::{self, MediaItem},
    map::{
        engine::MapEngine,
        scene::{MapScene, RadarMode},
        style::{build_style, radar_frames},
    },
    net,
    program::{ActiveTransition, BusFrame, Compositor, ProgramSource, ProgramState},
    record::Recording,
    show::{SceneKind, Show, Transition},
    updater::{self, UpdateState},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Workspace {
    Map,
    Severe,
    Winter,
    Graphics,
    Show,
    Data,
    Media,
}

impl Workspace {
    pub const ALL: [Workspace; 7] = [
        Workspace::Map,
        Workspace::Severe,
        Workspace::Winter,
        Workspace::Graphics,
        Workspace::Show,
        Workspace::Data,
        Workspace::Media,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Workspace::Map => "Map Studio",
            Workspace::Severe => "Severe Center",
            Workspace::Winter => "Winter Studio",
            Workspace::Graphics => "Graphics",
            Workspace::Show => "Show",
            Workspace::Data => "Data Discovery",
            Workspace::Media => "Media",
        }
    }

    /// Compact tab label for narrow windows.
    pub fn short_label(self) -> &'static str {
        match self {
            Workspace::Map => "Map",
            Workspace::Severe => "Severe",
            Workspace::Winter => "Winter",
            Workspace::Graphics => "Graphics",
            Workspace::Show => "Show",
            Workspace::Data => "Data",
            Workspace::Media => "Media",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modal {
    None,
    /// First-run setup (station, coverage, backend).
    Setup,
    Settings,
    Sponsors,
    About,
    Changelog,
    Updates,
    /// "Ready for air?" check.
    Readiness,
    /// Closings and warming / cooling centers.
    Community,
    /// The station's forecast edits.
    ForecastEditor,
    /// Data sources: licenses, lineage, quality, compliance.
    DataSources,
    /// Forecaster desk: projects, sessions, notebook, decisions, bookmarks, calculations.
    Desk,
    /// Historical events and case studies.
    History,
    /// Viewer submissions: moderation, questions, polls.
    Viewers,
    /// Publishing targets, queue, schedules and public endpoints.
    Publishing,
    /// Emergency operations dashboard.
    Eoc,
    /// Workflow automation designer.
    Workflows,
    /// Science lab: verification, analysis and research tools.
    Lab,
    /// Map theme designer (custom basemaps).
    Themes,
    /// Report a bug: description and up to three screenshots, saved locally.
    BugReport,
}

/// What `AppState::open_request` asks the shell to open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenRequest {
    Modal(Modal),
    /// Settings on the page with this label ("Audio", "Integrations", …).
    Settings(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub id: u64,
    pub text: String,
    pub kind: ToastKind,
    pub at: Instant,
}

#[derive(Clone, Copy, Debug)]
struct FlyAnim {
    from: GeoView,
    to: GeoView,
    started: Instant,
    seconds: f32,
}

/// What Program is showing, for the UI.
#[derive(Clone, Debug, PartialEq)]
pub enum OnAir {
    LiveMap,
    Graphic(String),
}

#[path = "app_ext.rs"]
mod ext;
#[path = "app_ops2.rs"]
mod ops2;

pub struct AppState {
    pub settings: Settings,
    pub data: Arc<WxSnapshot>,
    pub health: DataHealth,
    pub refreshing: bool,

    pub live: MapScene,
    pub radar_frames: Vec<i64>,
    pub radar_index: usize,
    pub playing: bool,
    last_frame_switch: Instant,
    fly: Option<FlyAnim>,
    /// A camera move the operator interrupted (grabbed the map): its target, for "Resume".
    pub fly_interrupted: Option<GeoView>,
    last_style: String,
    /// Bumped by Refresh radar so "latest" tile URLs change and skip the tile cache.
    refresh_nonce: i64,
    /// Refresh radar was clicked: rebuild the renderers once the new data has arrived.
    reload_after_refresh: bool,

    /// Program's map engine. With Preview / Take on, the staged map has its own engines.
    pub engine: MapEngine,
    /// Engines for comparison panels 2-4 (empty in the single-map layout).
    panel_engines: Vec<MapEngine>,
    /// Preview / Take: `live` is the staged scene Map Studio edits; this holds what is on air.
    pub map_preview: Option<MapPreview>,
    /// Map Studio work tabs (browser-style). Only the visible tab is drawn: the on-air map has
    /// Program's engines and the visible work tab borrows the one preview engine set.
    pub work_tabs: Vec<WorkTab>,
    /// Last airport-observation fetch: when, and the half-degree box it covered.
    metar_at: Option<(Instant, [i32; 4])>,
    metar_loading: bool,
    /// The map overlay before an automatic call-out lower third replaced it.
    callout_prev_overlay: Option<Option<String>>,
    warning_check_at: Instant,
    /// Severe mode has seen a coverage warning since it was switched on.
    severe_saw_warning: bool,
    /// The show soundtrack now playing (file and volume), kept in step with the show by `tick`.
    show_bed: Option<(std::path::PathBuf, u32)>,
    /// Which work tab Map Studio shows; None = the On air tab (edits go straight to air).
    pub active_tab: Option<usize>,
    tab_serial: u64,
    /// Map engine and compositor paused because nothing needs frames.
    map_idle: bool,
    /// Lightning was inside the safety radius at the last check (for the "lightning nearby" alert).
    lightning_was_near: bool,
    /// Practice warnings (Severe Center > Rehearse). Kept across data refreshes until cleared.
    pub test_alerts: Vec<crate::data::Alert>,
    panel_styles: Vec<String>,
    pub compositor: Compositor,
    /// Set by any state change; the frame tick re-renders the UI when it is set.
    ui_dirty: bool,

    pub workspace: Workspace,
    pub modal: Modal,
    /// A bug-report screenshot to take on the next frame: Some(true) starts a new report,
    /// Some(false) adds a screenshot to the open one (the shell captures, then opens the form).
    pub bug_capture: Option<bool>,
    /// A window to open from outside the shell (Find anything): the shell opens it on the next
    /// update the same way its own buttons do (Settings loads first, then shows the page).
    pub open_request: Option<OpenRequest>,

    pub graphics: Vec<GraphicDoc>,
    pub shows: Vec<Show>,
    pub media: Vec<MediaItem>,
    pub exports: Vec<MediaItem>,

    pub show: Show,
    pub show_pos: Option<usize>,
    pub show_running: bool,
    scene_started: Instant,
    /// Show clock frozen this many seconds into the scene (Show tab transport pause).
    pub scene_paused: Option<f32>,
    /// Map Studio's own map from before a show scene took the map (going to a map scene replaces
    /// the live map). It comes back when the user returns to Map Studio with the show stopped.
    pub studio_map_before_show: Option<MapScene>,
    /// The workspace on the last tick (to notice the user coming back to Map Studio).
    last_workspace: Workspace,
    /// Recordings being finished in the background (encoder drain, sound mix).
    finishing_recordings: Vec<std::sync::mpsc::Receiver<anyhow::Result<std::path::PathBuf>>>,
    scene_forecast_wait_until: Option<Instant>,
    pending_clip: Option<PendingClip>,
    clip_duration: Option<f32>,
    replay_export: Option<ReplayExport>,

    pub on_air: OnAir,
    pub black: bool,
    pub recording: Option<Recording>,
    pub update: UpdateState,
    pub toasts: Vec<Toast>,
    toast_serial: u64,
    pub output_window: Option<AnyWindowHandle>,
    last_clock: i64,
    /// Futurecast / data loop playing.
    pub data_playing: bool,
    last_tick: Instant,
    grid_loading: bool,
    model_loading: bool,
    pub site_replay_loading: bool,
    model_retry: [Instant; 3],
    pub model_error: [Option<String>; 3],
    grid_view_changed: Instant,
    lightning_loading: bool,
    lightning_at: Instant,
    /// Graphic to open in Graphics Studio (set when one is created elsewhere).
    pub pending_graphic: Option<String>,
    /// Ask the main window to open the program output window.
    pub want_output: bool,
    /// Ask the main window to close the program output window.
    pub want_close_output: bool,
    /// Rendering the show to video: recording stops when the show ends.
    pub exporting_show: bool,
    /// Show export: start recording once the first scene is actually on air (not before, or
    /// the video opens on whatever was on air last, e.g. the sign-off).
    pub export_record_pending: bool,
    pub severe_mode: SevereMode,
    /// Mode to switch to on the next frame (set where no window context is available).
    pub pending_mode: Option<SevereMode>,
    saved_scene: Option<MapScene>,
    seen_alerts: std::collections::HashSet<String>,
    alerts_primed: bool,
    pub event_log: Vec<LogEntry>,
    /// Broadcast operations state (control server, session, monitoring, sponsors).
    pub ext: ext::Ext,
    pub ops2: ops2::Ops2,
}

/// Preview / Take. Program keeps `program` (rendered by `AppState::engine`) while Map Studio
/// edits `AppState::live`, rendered by this preview's own engines. Take swaps the two engine
/// sets, so the staged map goes to air already drawn.
struct PanelCtx<'a> {
    data: &'a WxSnapshot,
    api_base: &'a str,
    refresh: i64,
    time: Option<i64>,
    earliest: Option<i64>,
}

/// Start/stop the comparison-panel engines for a scene's layout and keep their styles current.
/// Panel 1 is the main engine; panels 2-4 get their own MapLibre instances.
fn sync_panel_set(ctx: &PanelCtx, scene: &MapScene, engines: &mut Vec<MapEngine>, styles: &mut Vec<String>) {
    let layout = scene.layout;
    let wanted = layout.count() - 1;
    let size = layout.panel_size();
    let style_for = |i: usize| {
        let t = if scene.panel_spec(i).product == crate::map::scene::PanelProduct::RadarEarlier {
            ctx.earliest.or(ctx.time)
        } else {
            ctx.time
        };
        build_style(&scene.panel_scene(i), ctx.data.radar.as_ref(), t, ctx.api_base, ctx.refresh)
    };
    if engines.len() != wanted || !engines.iter().all(|e| e.size() == size) {
        // Dropping an engine closes its command channel, which ends its thread.
        engines.clear();
        styles.clear();
        for i in 1..=wanted {
            let style = style_for(i);
            styles.push(style.to_string());
            engines.push(MapEngine::start_sized(style, scene.view, size.0, size.1));
        }
        return;
    }
    for i in 1..=wanted {
        let style = style_for(i);
        let key = style.to_string();
        if styles.get(i - 1) != Some(&key) {
            engines[i - 1].set_style(style);
            styles[i - 1] = key;
        }
    }
}

/// Which copy of a graphic a show scene plays.
pub enum SceneGraphic {
    /// The scene's own graphic, from the library.
    Library(GraphicDoc),
    /// The scene's graphic is gone; this library graphic has the same name and category.
    Relinked(GraphicDoc),
    /// Nothing in the library matches: the copy saved in the show.
    Saved(GraphicDoc),
}

/// A Map Studio work tab: a map being prepared while another is on air.
#[derive(Clone)]
pub struct WorkTab {
    pub id: u64,
    pub scene: MapScene,
}

impl WorkTab {
    /// Tab caption: the map's on-air title ("Live Doppler", "Snowfall Forecast", …).
    pub fn title(&self) -> String {
        self.scene.banner_text().trim().to_string()
    }
}

/// Most work tabs open at once (each keeps only a saved map; one preview renderer is shared).
pub const MAX_WORK_TABS: usize = 4;

pub struct MapPreview {
    pub program: MapScene,
    /// The on-air forecast loop keeps playing while the operator stages.
    program_playing: bool,
    engine: MapEngine,
    panel_engines: Vec<MapEngine>,
    panel_styles: Vec<String>,
    last_style: String,
}

struct ReplayExport {
    format: crate::config::RecordFormat,
    recording: Option<Recording>,
    ready_at: Instant,
    view: crate::config::GeoView,
    frames: Vec<i64>,
    previous: (usize, bool),
}

struct PendingClip {
    show_id: String,
    scene_id: String,
    title: String,
    transition: Transition,
    soundtrack: Option<std::path::PathBuf>,
    receiver: std::sync::mpsc::Receiver<anyhow::Result<crate::clip::Clip>>,
}

impl AppState {
    pub fn new() -> Self {
        let mut settings = Settings::load();
        settings.api_base = crate::backend::effective_base(&settings);
        crate::map::winter_radar::set_enabled(settings.snow_radar_repaint);
        crate::audio::set_presentation_mix(settings.presentation_muted, settings.graphic_volume);
        net::set_api_base(&settings.api_base);
        let live = MapScene::from_settings(&settings);
        let style = build_style(&live, None, None, &settings.api_base, 0);
        let engine = MapEngine::start(style.clone(), live.view);
        let compositor = Compositor::start(
            engine.clone(),
            ProgramState::new(Arc::new(settings.clone()), live.clone()),
        );
        let shows = library::list_shows();
        let show = shows.first().cloned().unwrap_or_else(|| Show::new("Evening Weather"));

        let mut app = Self {
            settings,
            data: Arc::new(WxSnapshot::default()),
            health: DataHealth::default(),
            refreshing: false,
            live,
            radar_frames: Vec::new(),
            radar_index: 0,
            playing: false,
            last_frame_switch: Instant::now(),
            fly: None,
            fly_interrupted: None,
            last_style: style.to_string(),
            refresh_nonce: 0,
            reload_after_refresh: false,
            engine,
            panel_engines: Vec::new(),
            map_preview: None,
            work_tabs: Vec::new(),
            show_bed: None,
            metar_at: None,
            metar_loading: false,
            callout_prev_overlay: None,
            warning_check_at: Instant::now(),
            severe_saw_warning: false,
            active_tab: None,
            tab_serial: 0,
            map_idle: false,
            test_alerts: Vec::new(),
            lightning_was_near: false,
            panel_styles: Vec::new(),
            compositor,
            ui_dirty: true,
            workspace: Workspace::Map,
            modal: Modal::None,
            bug_capture: None,
            open_request: None,
            graphics: library::list_graphics(),
            shows,
            media: library::list_media(),
            exports: library::list_exports(),
            show,
            show_pos: None,
            show_running: false,
            scene_started: Instant::now(),
            scene_paused: None,
            studio_map_before_show: None,
            last_workspace: Workspace::Map,
            finishing_recordings: Vec::new(),
            scene_forecast_wait_until: None,
            pending_clip: None,
            clip_duration: None,
            replay_export: None,
            on_air: OnAir::LiveMap,
            black: false,
            recording: None,
            update: UpdateState::Idle,
            toasts: Vec::new(),
            toast_serial: 0,
            output_window: None,
            last_clock: 0,
            data_playing: false,
            last_tick: Instant::now(),
            grid_loading: false,
            model_loading: false,
            site_replay_loading: false,
            model_retry: [Instant::now(); 3],
            model_error: [None, None, None],
            grid_view_changed: Instant::now(),
            lightning_loading: false,
            lightning_at: Instant::now() - Duration::from_secs(600),
            pending_graphic: None,
            want_output: false,
            want_close_output: false,
            exporting_show: false,
            export_record_pending: false,
            severe_mode: SevereMode::Off,
            pending_mode: None,
            saved_scene: None,
            seen_alerts: std::collections::HashSet::new(),
            alerts_primed: false,
            event_log: Vec::new(),
            ext: ext::Ext::new(),
            ops2: ops2::Ops2::new(),
        };
        app.push_sponsor();
        app.relink_show_graphics();
        app
    }

    // ---- Notifications -----------------------------------------------------------------------

    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        let text = text.into();
        crate::devlog::push(
            if kind == ToastKind::Error { crate::devlog::Level::Warn } else { crate::devlog::Level::Info },
            "toast",
            text.clone(),
        );
        self.toast_serial += 1;
        self.toasts.push(Toast {
            id: self.toast_serial,
            text,
            kind,
            at: Instant::now(),
        });
        if self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
        self.ui_dirty = true;
    }

    pub fn dismiss_toast(&mut self, id: u64) {
        self.toasts.retain(|t| t.id != id);
    }

    // ---- Map ---------------------------------------------------------------------------------

    pub fn current_radar_time(&self) -> Option<i64> {
        if let Some(time) = self.live.history_time { return Some(time); }
        if self.radar_frames.is_empty() {
            return None;
        }
        let index = match self.live.radar.mode {
            _ if self.live.is_radar_replay() => self.radar_index,
            RadarMode::Latest => self
                .data
                .radar
                .as_ref()
                .map(|r| r.radar.past.len().saturating_sub(1))
                .unwrap_or(0),
            RadarMode::Loop => self.radar_index,
        };
        self.radar_frames
            .get(index.min(self.radar_frames.len() - 1))
            .copied()
    }

    /// Rebuild the style (only sent to the engine when it changed) and refresh the compositor.
    pub fn sync_map(&mut self) {
        self.radar_frames = radar_frames(&self.live, self.data.radar.as_ref());
        if self.radar_index >= self.radar_frames.len() {
            self.radar_index = self.radar_frames.len().saturating_sub(1);
        }
        let time = self.current_radar_time();
        // A running show always plays straight to air: staging pauses until it stops.
        if self.show_running {
            if let Some(preview) = self.map_preview.as_mut() {
                preview.program = self.live.clone();
            }
        }
        let program_scene = self.map_preview.as_ref().map_or(&self.live, |p| &p.program).clone();
        let style = build_style(&program_scene, self.data.radar.as_ref(), time, &self.settings.api_base, self.refresh_key());
        let key = style.to_string();
        if key != self.last_style {
            self.engine.set_style(style);
            self.last_style = key;
        }
        let refresh = self.refresh_key();
        let ctx = PanelCtx { data: &self.data, api_base: &self.settings.api_base, refresh, time, earliest: self.radar_frames.first().copied() };
        sync_panel_set(&ctx, &program_scene, &mut self.panel_engines, &mut self.panel_styles);
        let staging = !self.show_running;
        let preview_bus = match self.map_preview.as_mut().filter(|_| staging) {
            Some(preview) => {
                let style = build_style(&self.live, self.data.radar.as_ref(), time, &self.settings.api_base, refresh);
                let key = style.to_string();
                if key != preview.last_style {
                    preview.engine.set_style(style);
                    preview.last_style = key;
                }
                sync_panel_set(&ctx, &self.live, &mut preview.panel_engines, &mut preview.panel_styles);
                Some(crate::program::PreviewBus {
                    scene: self.live.clone(),
                    primary: preview.engine.clone(),
                    panels: preview.panel_engines.clone(),
                })
            }
            None => None,
        };
        let live = program_scene;
        let panels = self.panel_engines.clone();
        let primary = self.engine.clone();
        let slots = self.live.overlay_slots.each_ref().map(|id| id.as_ref().and_then(|id| {
            self.graphics.iter().find(|graphic| graphic.id == *id && graphic.is_overlay()).cloned()
        }));
        self.compositor.update(|p| {
            p.live_map = live;
            p.radar_time = time;
            p.panel_engines = panels;
            p.primary_engine = Some(primary);
            p.preview = preview_bus;
            for (slot, doc) in p.map_overlay_slots.iter_mut().zip(slots) {
                let started = slot.as_ref().filter(|(old, _)| doc.as_ref().is_some_and(|new| new.id == old.id))
                    .map_or_else(Instant::now, |(_, time)| *time);
                *slot = doc.map(|doc| (doc, started));
            }
        });
        self.ui_dirty = true;
    }

    pub fn panel_engine_status(&self) -> Vec<crate::map::engine::EngineStatus> {
        self.panel_engines.iter().map(|e| e.status()).collect()
    }

    pub fn set_view(&mut self, view: GeoView) {
        if view != self.live.view {
            self.grid_view_changed = Instant::now();
        }
        self.live.view = view;
        if let Some(preview) = self.map_preview.as_ref().filter(|_| !self.show_running) {
            preview.engine.set_camera(view);
            for engine in &preview.panel_engines {
                engine.set_camera(view);
            }
            self.compositor.update(|p| if let Some(bus) = p.preview.as_mut() { bus.scene.view = view });
        } else {
            if let Some(preview) = self.map_preview.as_mut() {
                preview.program.view = view;
            }
            self.engine.set_camera(view);
            for engine in &self.panel_engines {
                engine.set_camera(view);
            }
            self.compositor.update(|p| p.live_map.view = view);
        }
        self.ui_dirty = true;
    }

    /// The engine whose frames Map Studio shows (for mapping clicks to places).
    pub fn editor_engine(&self) -> &MapEngine {
        self.map_preview.as_ref().map_or(&self.engine, |p| &p.engine)
    }

    /// Turn Preview / Take on or off. Turning it off puts the staged map on air.
    pub fn set_map_preview(&mut self, on: bool) {
        if on == self.map_preview.is_some() {
            return;
        }
        if on {
            let style = build_style(&self.live, self.data.radar.as_ref(), self.current_radar_time(), &self.settings.api_base, self.refresh_key());
            let engine = MapEngine::start(style.clone(), self.live.view);
            engine.set_paused(self.map_idle);
            self.map_preview = Some(MapPreview {
                program: self.live.clone(),
                program_playing: self.data_playing,
                engine,
                panel_engines: Vec::new(),
                panel_styles: Vec::new(),
                last_style: style.to_string(),
            });
            self.sync_map();
        } else {
            let pending = self.preview_pending();
            self.commit_preview(Transition::Cut);
            self.map_preview = None;
            self.compositor.update(|p| p.preview = None);
            self.sync_map();
            if pending && matches!(self.on_air, OnAir::LiveMap) {
                self.toast(ToastKind::Info, "Live edit: the staged map is now on air");
            }
        }
        self.ui_dirty = true;
    }

    /// Open a work tab starting from the map on air and switch to it.
    pub fn open_work_tab(&mut self) {
        if self.work_tabs.len() >= MAX_WORK_TABS {
            self.toast(ToastKind::Info, format!("Up to {MAX_WORK_TABS} work tabs. Close one to open another."));
            return;
        }
        let on_air = self.map_preview.as_ref().map_or_else(|| self.live.clone(), |p| p.program.clone());
        self.tab_serial += 1;
        self.work_tabs.push(WorkTab { id: self.tab_serial, scene: on_air });
        self.switch_tab(Some(self.work_tabs.len() - 1));
    }

    /// Show a work tab (Some) or the On air tab (None). Leaving a work tab keeps its map.
    pub fn switch_tab(&mut self, to: Option<usize>) {
        if to == self.active_tab || to.is_some_and(|i| i >= self.work_tabs.len()) {
            return;
        }
        if let Some(i) = self.active_tab {
            if let Some(tab) = self.work_tabs.get_mut(i) {
                tab.scene = self.live.clone();
            }
        }
        match to {
            Some(i) => {
                if self.map_preview.is_none() {
                    self.set_map_preview(true);
                }
                self.active_tab = Some(i);
                let scene = self.work_tabs[i].scene.clone();
                let view = scene.view;
                self.live = scene;
                self.fly = None;
                self.set_view(view);
                self.sync_map();
            }
            None => {
                self.active_tab = None;
                self.leave_preview();
            }
        }
        self.ui_dirty = true;
    }

    pub fn close_work_tab(&mut self, index: usize) {
        if index >= self.work_tabs.len() {
            return;
        }
        if self.active_tab == Some(index) {
            self.switch_tab(None);
        }
        self.work_tabs.remove(index);
        if let Some(active) = self.active_tab.filter(|a| *a > index) {
            self.active_tab = Some(active - 1);
        }
        self.ui_dirty = true;
    }

    /// Back to editing the on-air map, discarding nothing: the staged map stays in its tab.
    fn leave_preview(&mut self) {
        let Some(preview) = self.map_preview.take() else { return };
        let view = preview.program.view;
        self.live = preview.program;
        self.compositor.update(|p| p.preview = None);
        self.fly = None;
        self.set_view(view);
        self.sync_map();
    }

    /// Put the visible work tab on air.
    pub fn send_tab_to_air(&mut self) {
        if self.active_tab.is_some() {
            self.take_map_preview(Transition::Fade);
        } else {
            // On the on-air tab: put the map back on Program when a graphic (or a Winter /
            // Graphics Studio take) is showing there (CWXB-106: the button did nothing, and only
            // Severe Center > Radar to Program brought the map back).
            self.take_live_map(Transition::Fade);
            self.show_pos = None;
        }
    }

    /// The staged map differs from what is on air.
    pub fn preview_pending(&self) -> bool {
        self.map_preview.as_ref().is_some_and(|p| p.program != self.live)
    }

    /// Discard staged changes: Map Studio goes back to what is on air.
    pub fn revert_map_preview(&mut self) {
        let Some(program) = self.map_preview.as_ref().map(|p| p.program.clone()) else { return };
        let view = program.view;
        self.live = program;
        self.fly = None;
        self.set_view(view);
        self.sync_map();
    }

    /// Put the staged map on Program. The preview engines already drew it, so the two engine
    /// sets swap places instead of Program reloading tiles.
    fn commit_preview(&mut self, transition: Transition) {
        if self.show_running {
            return;
        }
        let Some(preview) = self.map_preview.as_mut() else { return };
        if preview.program == self.live {
            return;
        }
        preview.program = self.live.clone();
        preview.program_playing = self.data_playing;
        std::mem::swap(&mut self.engine, &mut preview.engine);
        std::mem::swap(&mut self.panel_engines, &mut preview.panel_engines);
        std::mem::swap(&mut self.panel_styles, &mut preview.panel_styles);
        std::mem::swap(&mut self.last_style, &mut preview.last_style);
        let view = self.live.view;
        self.engine.set_camera(view);
        preview.engine.set_camera(view);
        for engine in self.panel_engines.iter().chain(&preview.panel_engines) {
            engine.set_camera(view);
        }
        if matches!(self.on_air, OnAir::LiveMap) {
            let t = self.start_transition(transition);
            self.compositor.update(|p| p.transition = t);
        }
        self.sync_map();
    }

    /// Take: the staged map goes on air (and the map becomes Program's source).
    pub fn take_map_preview(&mut self, transition: Transition) {
        if matches!(self.on_air, OnAir::LiveMap) {
            self.commit_preview(transition);
            self.mark_dirty();
        } else {
            self.take_live_map(transition);
        }
    }

    /// Ask for a UI refresh on the next frame tick.
    pub fn mark_dirty(&mut self) {
        self.ui_dirty = true;
    }

    /// Changes on every data refresh; part of "latest" tile URLs (NEXRAD site, GOES, MRMS) so
    /// they are fetched again instead of served from the tile cache.
    pub fn refresh_key(&self) -> i64 {
        self.data.updated_at.map(|t| t.timestamp() / 60).unwrap_or(0) + self.refresh_nonce * 1_000_000_000
    }

    /// Refresh radar (button or F5): fetch the data again, rebuild every map renderer so each
    /// tile is requested again, and give "latest" products new tile URLs. Fixes a blank or stuck
    /// map without restarting the app.
    pub fn refresh_radar(&mut self, cx: &mut Context<Self>) {
        self.refresh_nonce += 1;
        // Rebuild after the new radar frame list is in, so the new renderer starts from the
        // newest frames (rebuilding first re-requested the old ones).
        self.reload_after_refresh = true;
        self.refresh_data(cx);
        log::info!("refresh radar: data requested again; renderers rebuild when it arrives");
        self.toast(ToastKind::Info, "Refreshing radar and map");
    }

    /// The second half of Refresh radar, once the data has arrived.
    fn reload_renderers(&mut self) {
        // Comparison panels are simply restarted (dropping one ends its thread).
        self.panel_styles.clear();
        self.panel_engines.clear();
        self.last_style.clear();
        if let Some(preview) = self.map_preview.as_mut() {
            preview.panel_styles.clear();
            preview.panel_engines.clear();
            preview.last_style.clear();
        }
        // Send the new style first so the rebuilt renderer starts from it.
        self.sync_map();
        self.engine.reload();
        if let Some(preview) = &self.map_preview {
            preview.engine.reload();
        }
        let newest = self.data.radar.as_ref().and_then(|r| r.radar.past.last()).map(|f| f.time);
        let text = match newest.and_then(|t| chrono::DateTime::from_timestamp(t, 0)) {
            Some(t) => format!("Radar refreshed: newest frame {}", t.with_timezone(&chrono::Local).format("%-I:%M %p")),
            None => "Radar refreshed, but no radar frames came back. Check Data health".to_string(),
        };
        log::info!("refresh radar: renderers rebuilt ({text})");
        self.toast(ToastKind::Info, text);
    }

    /// Call out an alert on air (TV "blip"), or clear it.
    pub fn select_alert(&mut self, id: Option<String>) {
        if let (Some(id), OnAir::LiveMap) = (&id, &self.on_air) {
            if !self.black {
                let id = id.clone();
                self.mark_aired(&id);
            }
        }
        let previous = self.live.selected_alert.clone();
        self.live.selected_alert = id.clone();
        self.compositor.update(|p| p.blip_started = Instant::now());
        self.sync_map();
        if !self.settings.callout_lower_third || previous == id {
            return;
        }
        match id {
            Some(id) => {
                if self.callout_prev_overlay.is_none() {
                    self.callout_prev_overlay = Some(self.live.overlay_graphic.clone());
                }
                self.callout_lower_third(&id);
            }
            None => {
                if let Some(overlay) = self.callout_prev_overlay.take() {
                    self.set_map_overlay(overlay);
                }
            }
        }
    }

    /// The called-out warning's lower third on the map. One library graphic per warning type
    /// is reused, so repeated call-outs don't fill the library.
    fn callout_lower_third(&mut self, id: &str) {
        let Some(alert) = self.find_alert(id) else { return };
        let mut doc = crate::gfx::templates::warning_lower_third(&alert);
        if let Some(existing) = self.graphics.iter().find(|g| g.name == doc.name && g.category == doc.category) {
            doc.id = existing.id.clone();
        }
        if let Some(saved) = self.save_graphic(doc) {
            self.set_map_overlay(Some(saved.id));
        }
    }

    /// Every 15 s: clear a call-out whose warning has ended, and leave severe mode once no
    /// coverage warnings remain (restoring the map that was up before).
    pub fn warning_end_check(&mut self, cx: &mut Context<Self>) {
        if !self.settings.return_after_warning || self.warning_check_at.elapsed() < Duration::from_secs(15) {
            return;
        }
        self.warning_check_at = Instant::now();
        if let Some(id) = self.live.selected_alert.clone() {
            if self.find_alert(&id).is_none() {
                self.select_alert(None);
                self.toast(ToastKind::Info, "The called-out warning ended. Call-out cleared.");
            }
        }
        if self.severe_mode == SevereMode::Off {
            self.severe_saw_warning = false;
            return;
        }
        if !self.coverage_warnings().is_empty() {
            self.severe_saw_warning = true;
        } else if self.severe_saw_warning {
            self.severe_saw_warning = false;
            self.set_severe_mode(SevereMode::Off, cx);
            self.toast(ToastKind::Info, "No warnings left in the coverage area. Back to normal coverage.");
        }
    }

    /// Switch to Graphics Studio with this graphic open.
    /// Start a bug report: the shell captures the app window, then opens the report form.
    pub fn report_bug(&mut self) {
        self.modal = Modal::None;
        self.bug_capture = Some(true);
    }

    pub fn open_graphic(&mut self, id: String) {
        self.pending_graphic = Some(id);
        self.workspace = Workspace::Graphics;
        self.ui_dirty = true;
    }

    fn wants_grid(&self) -> bool {
        let d = &self.live.data;
        (d.field.is_some() && d.field != Some(crate::map::scene::DataField::Futurecast)) || d.wind_particles || d.isobars || d.wind_barbs || self.live.impact_callouts || self.live.snow_band || self.live.urban_flood
    }

    /// Fetch the model grid when a data layer needs it: none yet, the view moved off it, or it
    /// is older than an hour. Waits for the camera to settle first.
    pub fn maybe_fetch_grid(&mut self, cx: &mut Context<Self>) {
        if self.grid_loading || !self.wants_grid() || self.fly.is_some() {
            return;
        }
        if self.grid_view_changed.elapsed() < Duration::from_millis(900) {
            return;
        }
        let view = self.live.view;
        let model = self.live.data.grid_model;
        let field = self.live.data.field;
        let extended = self.settings.severe_grid || field.is_some_and(crate::map::scene::DataField::needs_upper_air);
        let air = self.settings.air_quality || field.is_some_and(crate::map::scene::DataField::needs_air);
        let fresh = self.data.grid.as_ref().map(|g| {
            g.covers(view) && chrono::Utc::now().timestamp() - g.fetched_at < 60 * 60
                && g.model == crate::grid::effective_model(model, view, extended) && (g.extended || !extended) && (g.air || !air)
        });
        if fresh == Some(true) {
            return;
        }
        // Don't hammer the API after a failure: wait 1 minute, or 10 after a rate limit (429).
        if !self.health.grid.ok {
            let wait = if self.health.grid.message.contains("429") { 600 } else { 60 };
            if self.health.grid.at.map(|t| (chrono::Local::now() - t).num_seconds() < wait).unwrap_or(false) {
                return;
            }
        }
        self.grid_loading = true;
        self.toast(ToastKind::Info, format!("Loading {} model data for this view…", crate::grid::effective_model(model, view, extended).label()));
        cx.spawn(async move |this, cx| {
            let result = net::spawn(crate::grid::fetch(view, extended, air, model)).await;
            let _ = this.update(cx, |state, cx| {
                state.grid_loading = false;
                let now = chrono::Local::now();
                match result {
                    Ok(Ok(grid)) => {
                        let mut data = (*state.data).clone();
                        let hours = grid.hours();
                        let raw = Arc::new(grid);
                        state.ext.raw_grid = Some(raw.clone());
                        data.grid = Some(if state.ext.edits.published.adjustments.is_empty() {
                            raw
                        } else {
                            Arc::new(raw.adjusted(&state.ext.edits.published))
                        });
                        data.forecast_edits = state.ext.edits.published.clone();
                        state.data = Arc::new(data);
                        if state.scene_forecast_wait_until.is_some()
                            && state.live.data.field != Some(crate::map::scene::DataField::Futurecast)
                            && crate::program::forecast_ready(&state.live, &state.data)
                        {
                            state.scene_forecast_wait_until = None;
                            state.scene_started = Instant::now();
                        }
                        let snapshot = state.data.clone();
                        state.compositor.update(|p| p.data = snapshot);
                        state.health.grid = FeedStatus {
                            ok: true,
                            message: format!("{hours} model hours"),
                            at: Some(now),
                        };
                    }
                    Ok(Err(e)) => {
                        state.health.grid = FeedStatus { ok: false, message: e.to_string(), at: Some(now) };
                        let text = if e.to_string().contains("429") {
                            format!("{} data paused: the provider asked us to wait (429). See Developer Logs.", model.label())
                        } else {
                            format!("Model data failed: {e}")
                        };
                        state.toast(ToastKind::Error, text);
                    }
                    Err(e) => {
                        state.health.grid = FeedStatus { ok: false, message: e.to_string(), at: Some(now) };
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Load only the NOAA sources used by the active map and its comparison panels.
    pub fn maybe_fetch_futurecast(&mut self, cx: &mut Context<Self>) {
        if self.model_loading || self.fly.is_some() || self.grid_view_changed.elapsed() < Duration::from_millis(900) {
            return;
        }
        let hours = self.live.data.loop_hours.clamp(1, crate::futurecast::MAX_LEAD);
        let view = self.live.view;
        let Some(source) = self.live.model_products().map(|product| product.source()).find(|&source| {
            let i = source.index();
            if Instant::now() < self.model_retry[i] { return false; }
            let pin = self.live.data.model_pins[i];
            !self.data.model(source).is_some_and(|model| {
                model.region.covers(view) && model.hours >= hours && pin.is_none_or(|pin| pin == model.pin())
                    && (pin.is_some() || chrono::Utc::now().timestamp() - model.fetched_at < 30 * 60)
            })
        }) else { return };
        let i = source.index();
        let region = match crate::futurecast::Region::for_view(view).and_then(|region| {
            region.validate(source)?;
            Ok(region)
        }) {
            Ok(region) => region,
            Err(error) => {
                self.model_error[i] = Some(error.to_string());
                self.model_retry[i] = Instant::now() + Duration::from_secs(5);
                cx.notify();
                return;
            }
        };
        self.model_loading = true;
        self.model_error[i] = None;
        let base = self.settings.api_base.clone();
        let pin = self.live.data.model_pins[i];
        cx.spawn(async move |this, cx| {
            let request_base = base.clone();
            let result = net::spawn(async move {
                crate::futurecast::fetch(&request_base, region, hours, source, pin).await
            }).await;
            let _ = this.update(cx, |state, cx| {
                state.model_loading = false;
                if state.settings.api_base != base {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Ok(model)) => {
                        Arc::make_mut(&mut state.data).models[i] = Some(Arc::new(model));
                        if state.scene_forecast_wait_until.is_some() && crate::program::forecast_ready(&state.live, &state.data) {
                            state.scene_forecast_wait_until = None;
                            state.scene_started = Instant::now();
                        }
                        state.compositor.update(|p| p.data = state.data.clone());
                    }
                    error => {
                        let message = match error {
                            Ok(Err(e)) => e.to_string(),
                            Err(e) => e.to_string(),
                            Ok(Ok(_)) => unreachable!(),
                        };
                        state.model_error[i] = Some(format!("{} unavailable: {message}", source.label()));
                        state.model_retry[i] = Instant::now() + Duration::from_secs(60);
                    }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    /// Lightning refreshes every 30 s while the layer is on.
    pub fn maybe_fetch_lightning(&mut self, cx: &mut Context<Self>) {
        if self.lightning_loading || !self.live.lightning || self.lightning_at.elapsed() < Duration::from_secs(30) {
            return;
        }
        self.lightning_loading = true;
        self.lightning_at = Instant::now();
        let base = self.settings.api_base.clone();
        cx.spawn(async move |this, cx| {
            let result = net::spawn(net::lightning(base)).await;
            let _ = this.update(cx, |state, cx| {
                state.lightning_loading = false;
                let now = chrono::Local::now();
                match result {
                    Ok(Ok(strikes)) => {
                        state.health.lightning = FeedStatus {
                            ok: true,
                            message: format!("{} strikes", strikes.len()),
                            at: Some(now),
                        };
                        let mut data = (*state.data).clone();
                        data.lightning = strikes;
                        state.data = Arc::new(data);
                        let snapshot = state.data.clone();
                        state.compositor.update(|p| p.data = snapshot);
                        state.check_lightning_near();
                    }
                    Ok(Err(e)) => state.health.lightning = FeedStatus { ok: false, message: e.to_string(), at: Some(now) },
                    Err(e) => state.health.lightning = FeedStatus { ok: false, message: e.to_string(), at: Some(now) },
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Send the staged forecast hour to whichever bus shows `live`: the preview while staging,
    /// otherwise Program.
    fn set_shown_hour(&mut self, hour: f32) {
        if self.map_preview.is_some() && !self.show_running {
            self.compositor.update(|p| if let Some(bus) = p.preview.as_mut() { bus.scene.data.hour = hour });
        } else {
            if let Some(preview) = self.map_preview.as_mut() {
                preview.program.data.hour = hour;
            }
            self.compositor.update(|p| p.live_map.data.hour = hour);
        }
    }

    /// Set the data/Futurecast hour (scrubbing); stops playback.
    pub fn set_data_hour(&mut self, hour: f32) {
        self.data_playing = false;
        let max = crate::program::forecast_span(&self.live, &self.data) as f32;
        self.live.data.hour = hour.clamp(0.0, max);
        let h = self.live.data.hour;
        self.set_shown_hour(h);
        self.ui_dirty = true;
    }

    /// Render the whole show to a video file: start recording, run the show from the top, and
    /// stop when it ends. Manual scenes (duration 0) get 8 seconds.
    pub fn export_show_video(&mut self) {
        if self.show.scenes.is_empty() {
            self.toast(ToastKind::Error, "Add scenes to the show before exporting it");
            return;
        }
        if self.recording.is_some() {
            self.toast(ToastKind::Error, "Stop the current recording first");
            return;
        }
        self.exporting_show = true;
        self.switch_tab(None);
        self.show_running = true;
        self.show_pos = None;
        // Take the first scene on a cut (no fade from whatever was on air), and start recording
        // only once it is on air: see `export_record_tick`.
        let first_transition = self.show.scenes[0].transition;
        self.show.scenes[0].transition = crate::show::Transition::Cut;
        self.next_scene();
        self.show.scenes[0].transition = first_transition;
        self.export_record_pending = true;
        self.toast(ToastKind::Info, format!("Exporting \"{}\" to video - it plays through once", self.show.name));
    }

    /// Show export: start the recording when the first scene is really on air (its clip
    /// decoded and its forecast loaded), so the video never opens on a stale frame.
    pub fn export_record_tick(&mut self) {
        if !self.export_record_pending {
            return;
        }
        if !self.exporting_show || !self.show_running {
            self.export_record_pending = false;
            return;
        }
        let first_on_air = self.show_pos == Some(0) && self.pending_clip.is_none() && self.scene_forecast_wait_until.is_none();
        // One compositor frame after the take, so Program already shows the scene.
        if first_on_air && self.scene_started.elapsed() >= Duration::from_millis(60) {
            self.export_record_pending = false;
            self.toggle_recording();
            if self.recording.is_none() {
                self.exporting_show = false;
                self.show_running = false;
                return;
            }
            // Restart the scene clock so the first scene gets its full length on the video.
            self.scene_started = Instant::now();
        }
    }

    /// Save the show (scenes, map views and embedded graphics) to a .cwxshow file.
    /// The file dialog for exporting the show (run it off the UI thread: `ui::pick_off_thread`).
    pub fn export_show_dialog(&self) -> rfd::FileDialog {
        let name = sanitize(&self.show.name);
        rfd::FileDialog::new()
            .set_title("Export show")
            .set_directory(LibraryPaths::get().exports)
            .set_file_name(format!("{name}.cwxshow"))
            .add_filter("OpenWX show", &["cwxshow", "json"])
    }

    pub fn export_show_file(&mut self, path: std::path::PathBuf) {
        match serde_json::to_string_pretty(&self.show).map_err(anyhow::Error::from).and_then(|json| Ok(std::fs::write(&path, json)?)) {
            Ok(()) => self.toast(ToastKind::Success, format!("Show exported to {}", path.display())),
            Err(e) => self.toast(ToastKind::Error, format!("Export failed: {e}")),
        }
    }

    /// Import a .cwxshow file as a new show.
    pub fn import_show_file(&mut self, path: std::path::PathBuf) -> Option<Show> {
        let parsed = std::fs::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|text| Ok(serde_json::from_str::<Show>(&text)?));
        match parsed {
            Ok(mut show) => {
                show.id = crate::gfx::model::new_id();
                // Bring embedded graphics into the library so scenes can be edited.
                for scene in &show.scenes {
                    if let SceneKind::Graphic { doc, .. } = &scene.kind {
                        if !self.graphics.iter().any(|g| g.id == doc.id) {
                            self.save_graphic(doc.clone());
                        }
                    }
                }
                self.toast(ToastKind::Success, format!("Imported \"{}\"", show.name));
                Some(show)
            }
            Err(e) => {
                self.toast(ToastKind::Error, format!("Import failed: {e}"));
                None
            }
        }
    }

    pub fn fly_to(&mut self, to: GeoView, seconds: f32) {
        self.fly_interrupted = None;
        let seconds = seconds * self.settings.camera_speed.clamp(0.3, 3.0);
        if seconds <= 0.05 {
            self.fly = None;
            self.set_view(to);
            return;
        }
        self.fly = Some(FlyAnim {
            from: self.live.view,
            to,
            started: Instant::now(),
            seconds,
        });
    }

    /// The operator grabbed the map mid-move: stop where it is and remember the target.
    /// The camera is flying or the map hasn't caught up with the latest pan / zoom.
    pub fn camera_moving(&self) -> bool {
        self.fly.is_some() || self.engine.latest().is_some_and(|frame| frame.view != self.live.view)
    }

    pub fn interrupt_fly(&mut self) {
        if let Some(fly) = self.fly.take() {
            self.fly_interrupted = Some(fly.to);
        }
    }

    /// Continue an interrupted move from wherever the map is now.
    pub fn resume_fly(&mut self) {
        if let Some(to) = self.fly_interrupted.take() {
            self.fly_to(to, 1.2);
        }
    }

    pub fn update_live(&mut self, f: impl FnOnce(&mut MapScene)) {
        let before_view = self.live.view;
        f(&mut self.live);
        if self.live.view != before_view {
            let view = self.live.view;
            self.set_view(view);
        }
        self.sync_map();
    }

    pub fn start_radar_replay(&mut self, hours: u32) {
        self.data_playing = false;
        self.playing = true;
        self.radar_index = 0;
        self.last_frame_switch = Instant::now();
        self.update_live(|map| {
            map.radar_replay = Some(crate::history::RadarReplay::recent(hours, chrono::Utc::now().timestamp()));
            map.site_replay = None;
            map.history_time = None;
            map.radar.enabled = true;
            map.radar.mode = RadarMode::Loop;
            map.radar.source = crate::map::scene::RadarSource::Composite;
            map.satellite.enabled = false;
            map.warnings = false;
            map.data.field = None;
            map.precip_total = None;
            map.layout = crate::map::scene::PanelLayout::Single;
        });
    }

    pub fn start_site_radar_replay(&mut self, cx: &mut Context<Self>) {
        if self.site_replay_loading { return; }
        self.site_replay_loading = true;
        let view = self.live.view;
        let previous = self.live.clone();
        let site = self.live.radar.resolved_site(view.lat, view.lon);
        let base = self.settings.api_base.clone();
        cx.spawn(async move |this, cx| {
            let result = net::spawn(async move { crate::history::site_replay(&base, site).await }).await;
            let _ = this.update(cx, |state, cx| {
                state.site_replay_loading = false;
                match result {
                    Ok(Ok(replay)) if state.workspace == Workspace::Map && state.live.view == previous.view
                        && state.live.radar == previous.radar && state.live.data.field == previous.data.field
                        && state.live.radar_replay == previous.radar_replay && state.live.site_replay == previous.site_replay => {
                        state.start_radar_replay(3);
                        state.update_live(|map| {
                            map.radar_replay = None;
                            map.radar.source = crate::map::scene::RadarSource::Site;
                            map.radar.site = replay.site.clone();
                            map.radar.product = crate::map::scene::SiteProduct::StormRelative;
                            map.site_replay = Some(replay);
                        });
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => state.toast(ToastKind::Error, format!("Velocity replay: {error}")),
                    Err(error) => state.toast(ToastKind::Error, format!("Velocity replay: {error}")),
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    pub fn is_exporting_replay(&self) -> bool { self.replay_export.is_some() }

    pub fn export_radar_replay(&mut self, format: crate::config::RecordFormat) {
        if self.replay_export.is_some() || !self.live.is_radar_replay() || self.radar_frames.len() < 2 {
            return;
        }
        self.replay_export = Some(ReplayExport {
            format, recording: None, ready_at: Instant::now() + Duration::from_millis(800),
            view: self.live.view, frames: self.radar_frames.clone(),
            previous: (self.radar_index, self.playing),
        });
        self.radar_index = 0;
        self.playing = false;
        self.sync_map();
        self.toast(ToastKind::Info, "Recording one radar replay cycle to Media → Exports");
    }

    fn poll_radar_export(&mut self) -> bool {
        let Some(mut export) = self.replay_export.take() else { return false };
        let invalid = self.live.view != export.view || self.radar_frames != export.frames || !self.live.radar.enabled
            || self.live.data.field.is_some() || self.live.radar.mode != RadarMode::Loop
            || (export.recording.is_some() && !self.playing);
        let finished = export.recording.is_some() && self.radar_index + 1 == self.radar_frames.len()
            && self.last_frame_switch.elapsed() >= Duration::from_millis(self.settings.loop_dwell_ms);
        if invalid || finished {
            if let Some(recording) = export.recording {
                let path = recording.path.clone();
                let result = recording.stop(&self.compositor);
                if invalid {
                    let _ = std::fs::remove_file(path);
                } else {
                    match result {
                        Ok(path) => {
                            self.exports = library::list_exports();
                            self.toast(ToastKind::Success, format!("Replay saved to {}", path.display()));
                        }
                        Err(error) => self.toast(ToastKind::Error, format!("Replay export failed: {error}")),
                    }
                }
            }
            if invalid {
                self.toast(ToastKind::Info, "Replay export cancelled because the map or playback changed");
            } else {
                (self.radar_index, self.playing) = export.previous;
                self.last_frame_switch = Instant::now();
                self.sync_map();
            }
            return true;
        }
        if export.recording.is_none() && Instant::now() >= export.ready_at {
            match Recording::start_map(&self.compositor, export.format, library::exports_dir()) {
                Ok(recording) => {
                    export.recording = Some(recording);
                    self.last_frame_switch = Instant::now();
                    self.playing = true;
                }
                Err(error) => {
                    (self.radar_index, self.playing) = export.previous;
                    self.sync_map();
                    self.toast(ToastKind::Error, format!("Replay export failed: {error}"));
                    return true;
                }
            }
        }
        self.replay_export = Some(export);
        false
    }

    pub fn step_radar(&mut self, delta: i32) {
        if self.radar_frames.is_empty() {
            return;
        }
        self.live.radar.mode = RadarMode::Loop;
        let len = self.radar_frames.len() as i32;
        self.radar_index = ((self.radar_index as i32 + delta).rem_euclid(len)) as usize;
        self.last_frame_switch = Instant::now();
        self.sync_map();
    }

    pub fn set_map_overlay(&mut self, id: Option<String>) {
        let doc = id
            .as_ref()
            .and_then(|id| self.graphics.iter().find(|g| &g.id == id).cloned());
        self.live.overlay_graphic = doc.as_ref().map(|d| d.id.clone());
        self.compositor.update(|p| {
            let now = Instant::now();
            if let Some(old) = p.map_overlay.take() {
                p.map_overlay_out = Some((old, now));
            }
            if let Some(doc) = doc {
                p.map_overlay = Some(doc);
                p.map_overlay_started = now;
            }
        });
        self.sync_map();
    }

    pub fn set_map_overlay_slot(&mut self, slot: usize, id: Option<String>) {
        if slot == 0 { self.set_map_overlay(id); return; }
        if let Some(current) = self.live.overlay_slots.get_mut(slot - 1) {
            *current = id.filter(|id| self.graphics.iter().any(|doc| doc.id == *id && doc.is_overlay()));
            self.sync_map();
        }
    }

    // ---- Program -----------------------------------------------------------------------------

    fn start_transition(&self, kind: Transition) -> Option<ActiveTransition> {
        match kind {
            Transition::Cut | Transition::Fly => None,
            other => Some(ActiveTransition {
                kind: other,
                started: Instant::now(),
                seconds: other.seconds(),
            }),
        }
    }

    pub fn take_live_map(&mut self, transition: Transition) {
        self.pending_clip = None;
        self.clip_duration = None;
        // Taking the map takes what is staged (OBS Studio Mode semantics).
        if !matches!(self.on_air, OnAir::LiveMap) {
            self.commit_preview(Transition::Cut);
        } else if self.preview_pending() {
            self.commit_preview(transition);
            return;
        }
        let t = self.start_transition(transition);
        self.compositor.update(|p| {
            p.transition = t;
            p.source = ProgramSource::LiveMap;
            p.source_started = Instant::now();
        });
        self.on_air = OnAir::LiveMap;
        self.resume_live_loop();
        crate::audio::stop(crate::audio::Bus::Graphic);
        crate::broadcast_ops::fire_webhook(&self.settings.tally_webhook, "program", "Live map");
    }

    /// The live radar loop never rests paused on an old frame when it is (or becomes) the
    /// on-air map: a paused loop starts again from the newest frame. Replays, history and the
    /// "Latest" mode are left alone.
    pub fn resume_live_loop(&mut self) {
        let live = &self.live;
        if self.playing
            || !live.radar.enabled
            || live.radar.mode != RadarMode::Loop
            || live.data.field.is_some()
            || live.is_radar_replay()
            || live.history_time.is_some()
        {
            return;
        }
        self.playing = true;
        self.radar_index = self.radar_frames.len().saturating_sub(1);
        self.last_frame_switch = Instant::now();
        self.sync_map();
    }

    pub fn take_graphic(&mut self, doc: GraphicDoc, transition: Transition) {
        self.pending_clip = None;
        self.clip_duration = None;
        let t = self.start_transition(transition);
        self.on_air = OnAir::Graphic(doc.name.clone());
        crate::broadcast_ops::fire_webhook(&self.settings.tally_webhook, "program", &doc.name);
        if doc.audio.trim().is_empty() {
            crate::audio::stop(crate::audio::Bus::Graphic);
        } else {
            crate::audio::play(crate::config::LibraryPaths::get().media.join(doc.audio.trim()), crate::audio::Bus::Graphic, 1.0, self.settings.audio_offset_ms as u64);
        }
        self.compositor.update(|p| {
            // A retake replays the entrance. Fading from the settled frame of the
            // same graphic overlays it with the animated copy at a different scale.
            let retake = matches!(&p.source, ProgramSource::Graphic(active) if active.id == doc.id);
            p.transition = if retake { None } else { t };
            p.graphic_state.clear();
            p.hidden_layers.clear();
            p.paused_at = None;
            p.source = ProgramSource::Graphic(doc);
            p.source_started = Instant::now();
        });
    }

    pub fn set_black(&mut self, black: bool) {
        self.black = black;
        self.compositor.update(|p| {
            p.black = black;
            p.black_since = Instant::now();
        });
    }

    // ---- Shows -------------------------------------------------------------------------------

    pub fn go_to_scene(&mut self, index: usize) {
        let Some(scene) = self.show.scenes.get(index).cloned() else {
            return;
        };
        self.scene_forecast_wait_until = None;
        self.pending_clip = None;
        self.clip_duration = None;
        let soundtrack = scene.soundtrack.as_deref().and_then(|name| {
            let file = std::path::Path::new(name);
            if file.file_name() == Some(file.as_os_str()) && library::is_audio(file) {
                let path = LibraryPaths::get().media.join(file);
                if path.is_file() {
                    return Some(path);
                }
            }
            self.toast(ToastKind::Error, format!("Soundtrack unavailable: {name}"));
            None
        });
        match scene.kind {
            SceneKind::Map { map } => {
                if map.data.play_on_take && map.data.field.is_some()
                    && !crate::program::forecast_ready(&map, &self.data)
                {
                    self.scene_forecast_wait_until = Some(Instant::now() + Duration::from_secs(45));
                }
                let target = map.view;
                let current = self.live.view;
                let was_replaying = self.live.is_radar_replay();
                if self.studio_map_before_show.is_none() {
                    self.studio_map_before_show = Some(self.live.clone());
                }
                self.live = map;
                if self.live.radar_replay.is_some() || self.live.site_replay.is_some() {
                    self.playing = self.live.site_replay.as_ref().is_none_or(|replay| replay.scans.len() > 1);
                    self.radar_index = 0;
                    self.last_frame_switch = Instant::now();
                } else if was_replaying || self.live.history_time.is_some() {
                    self.playing = false;
                    self.radar_index = radar_frames(&self.live, self.data.radar.as_ref()).len().saturating_sub(1);
                }
                self.data_playing = self.live.data.play_on_take;
                if self.data_playing {
                    self.live.data.hour = 0.0;
                }
                self.live.view = current;
                if scene.transition == Transition::Fly {
                    self.fly_to(target, Transition::Fly.seconds());
                } else {
                    self.fly = None;
                    self.set_view(target);
                }
                let overlay = self.live.overlay_graphic.clone();
                self.set_map_overlay(overlay);
                self.sync_map();
                self.take_live_map(if scene.transition == Transition::Fly {
                    Transition::Cut
                } else {
                    scene.transition
                });
                if let Some(path) = soundtrack {
                    crate::audio::play(path, crate::audio::Bus::Graphic, 1.0, self.settings.audio_offset_ms as u64);
                }
            }
            SceneKind::Graphic { graphic_id, doc } => {
                let mut doc = match self.scene_graphic(&graphic_id, &doc) {
                    SceneGraphic::Library(doc) => doc,
                    SceneGraphic::Relinked(doc) => {
                        self.relink_show_graphics();
                        doc
                    }
                    SceneGraphic::Saved(doc) => {
                        self.toast(ToastKind::Error, format!("\"{}\" is no longer in the graphics library. Playing the copy saved in this show; relink it in Show Builder.", doc.name));
                        doc
                    }
                };
                if self.approval_blocked(&doc) {
                    return;
                }
                if let Some(path) = soundtrack {
                    doc.audio = path.file_name().expect("library soundtrack name").to_string_lossy().into_owned();
                }
                self.take_graphic(doc, scene.transition);
            }
            SceneKind::Clip { media } => {
                let (sender, receiver) = std::sync::mpsc::sync_channel(1);
                if let Err(error) = std::thread::Builder::new().name("show-clip".into()).spawn(move || {
                    let _ = sender.send(crate::clip::Clip::load(&media));
                }) {
                    self.toast(ToastKind::Error, format!("Could not start clip decoder: {error}"));
                    return;
                }
                self.pending_clip = Some(PendingClip {
                    show_id: self.show.id.clone(), scene_id: scene.id, title: scene.title,
                    transition: scene.transition, soundtrack, receiver,
                });
            }
        }
        self.show_pos = Some(index);
        self.scene_started = Instant::now();
        self.scene_paused = None;
    }

    pub fn next_scene(&mut self) {
        let len = self.show.scenes.len();
        if len == 0 {
            return;
        }
        let next = match self.show_pos {
            None => 0,
            Some(i) if i + 1 < len => i + 1,
            Some(_) if self.show.loop_show && !self.exporting_show => 0,
            Some(_) => {
                self.show_running = false;
                return;
            }
        };
        self.go_to_scene(next);
    }

    pub fn prev_scene(&mut self) {
        if let Some(i) = self.show_pos {
            if i > 0 {
                self.go_to_scene(i - 1);
            }
        }
    }

    pub fn add_show_clip(&mut self, path: std::path::PathBuf) {
        match library::import_media(&[path]) {
            Ok(files) => {
                if let Some(media) = files.into_iter().next() {
                    self.show.scenes.push(crate::show::Scene::clip(media));
                    self.media = library::list_media();
                    self.save_show();
                }
            }
            Err(error) => self.toast(ToastKind::Error, format!("Could not import clip: {error}")),
        }
    }

    fn poll_show_clip(&mut self) -> bool {
        use std::sync::mpsc::TryRecvError;
        let Some(pending) = &self.pending_clip else { return false };
        if pending.show_id != self.show.id || self.show_pos.and_then(|index| self.show.scenes.get(index))
            .is_none_or(|scene| scene.id != pending.scene_id)
        {
            self.pending_clip = None;
            return false;
        }
        let result = match pending.receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err(anyhow::anyhow!("Clip decoder stopped")),
        };
        let pending = self.pending_clip.take().expect("clip request");
        self.scene_started = Instant::now();
        match result {
            Ok(clip) => {
                self.clip_duration = Some(clip.seconds());
                let transition = self.start_transition(pending.transition);
                let audio = clip.audio.clone();
                let clip = Arc::new(clip);
                self.compositor.update(|program| {
                    program.source = ProgramSource::Clip(clip);
                    program.transition = transition;
                    program.source_started = Instant::now();
                });
                crate::audio::stop(crate::audio::Bus::Graphic);
                if let Some(path) = pending.soundtrack {
                    crate::audio::play(path, crate::audio::Bus::Graphic, 1.0, self.settings.audio_offset_ms as u64);
                } else if let Some(audio) = audio {
                    crate::audio::play_clip(audio, self.settings.audio_offset_ms as u64);
                }
                self.on_air = OnAir::Graphic(pending.title);
            }
            Err(error) => {
                self.clip_duration = Some(1.0);
                self.toast(ToastKind::Error, format!("Clip unavailable: {error}"));
            }
        }
        true
    }

    pub fn scene_elapsed(&self) -> f32 {
        if let Some(p) = self.scene_paused {
            return p;
        }
        self.scene_started.elapsed().as_secs_f32()
    }

    /// Put the scene clock offset seconds in (scrub bar). Keeps a pause in place.
    pub fn set_scene_offset(&mut self, offset: f32) {
        let offset = offset.max(0.0);
        self.scene_started = Instant::now().checked_sub(Duration::from_secs_f32(offset)).unwrap_or_else(Instant::now);
        if self.scene_paused.is_some() {
            self.scene_paused = Some(offset);
        }
    }

    /// Coming back to Map Studio with the show stopped (not paused): put the user's own map back.
    /// Only on the way in, so a show that ends while Map Studio is open doesn't change air.
    fn restore_studio_map(&mut self) {
        let entered = self.workspace == Workspace::Map && self.last_workspace != Workspace::Map;
        self.last_workspace = self.workspace;
        if !entered || self.show_running || self.scene_paused.is_some() || self.exporting_show {
            return;
        }
        let Some(map) = self.studio_map_before_show.take() else { return };
        let view = map.view;
        self.live = map;
        self.fly = None;
        self.set_view(view);
        let overlay = self.live.overlay_graphic.clone();
        self.set_map_overlay(overlay);
        self.sync_map();
        self.toast(ToastKind::Info, "Map Studio is back on your map from before the show");
    }

    /// Pause the running show where it is (the scene stays on air).
    pub fn pause_show(&mut self) {
        self.scene_paused = Some(self.scene_elapsed());
        self.show_running = false;
    }

    /// Resume a paused show from the same point, or start it from the first scene.
    pub fn resume_show(&mut self) {
        if let Some(p) = self.scene_paused.take() {
            self.set_scene_offset(p);
        }
        self.show_running = true;
        if self.show_pos.is_none() {
            self.next_scene();
        }
    }

    // ---- Per-frame tick ----------------------------------------------------------------------

    /// Called ~30 times a second. Returns true when the UI should re-render.
    /// Whether anything needs map or Program frames right now.
    fn frames_needed(&self) -> bool {
        matches!(self.workspace, Workspace::Map | Workspace::Severe | Workspace::Show)
            || self.output_window.is_some()
            || self.recording.is_some()
            || self.replay_export.is_some()
            || self.show_running
    }

    /// Airport observations for the map view while station plots are on (5-minute refresh).
    pub fn maybe_fetch_metars(&mut self, cx: &mut Context<Self>) {
        let wanted = self.live.metar_plots || self.map_preview.as_ref().is_some_and(|p| p.program.metar_plots);
        if !wanted || self.metar_loading {
            return;
        }
        let vp = crate::map::geo::Viewport::new(self.live.view, crate::config::FRAME_W, crate::config::FRAME_H);
        let (north, west) = vp.to_geo(0.0, 0.0);
        let (south, east) = vp.to_geo(crate::config::FRAME_W as f64, crate::config::FRAME_H as f64);
        // Pad the box a little and snap it, so small pans reuse the last fetch.
        let snap = |v: f64, up: bool| if up { (v * 2.0).ceil() as i32 } else { (v * 2.0).floor() as i32 };
        let lat_pad = ((north - south) * 0.15).max(0.25);
        let lon_pad = ((east - west) * 0.15).max(0.25);
        let key = [snap(south - lat_pad, false), snap(west - lon_pad, false), snap(north + lat_pad, true), snap(east + lon_pad, true)];
        let fresh = self.metar_at.is_some_and(|(at, last)| {
            at.elapsed() < Duration::from_secs(300)
                && last[0] <= key[0] && last[1] <= key[1] && last[2] >= key[2] && last[3] >= key[3]
        });
        if fresh {
            return;
        }
        // Station plots only make sense regionally; the relay refuses very large boxes.
        if key[2] - key[0] > 40 || key[3] - key[1] > 60 {
            return;
        }
        self.metar_loading = true;
        self.metar_at = Some((Instant::now(), key));
        let [s, w, n, e] = key.map(|v| v as f64 / 2.0);
        cx.spawn(async move |this, cx| {
            let result = net::spawn(net::metars(s, w, n, e)).await;
            let _ = this.update(cx, |state, cx| {
                state.metar_loading = false;
                match result {
                    Ok(Ok(metars)) => {
                        let mut data = (*state.data).clone();
                        data.metars = metars;
                        state.data = Arc::new(data);
                        let snapshot = state.data.clone();
                        state.compositor.update(|p| p.data = snapshot);
                        cx.notify();
                    }
                    Ok(Err(e)) => log::warn!("airport observations: {e}"),
                    Err(e) => log::warn!("airport observations task: {e}"),
                }
            });
        }).detach();
    }

    /// Start, change or stop the show soundtrack to match the show that is running.
    fn sync_show_bed(&mut self) {
        let wanted = self.show.soundtrack.as_deref().filter(|_| self.show_running).map(|name| {
            (LibraryPaths::get().media.join(name), (self.show.soundtrack_volume.clamp(0.0, 1.5) * 100.0).round() as u32)
        });
        if wanted == self.show_bed {
            return;
        }
        crate::audio::stop(crate::audio::Bus::Show);
        self.show_bed = None;
        if let Some((path, volume)) = wanted {
            if path.is_file() {
                crate::audio::play_loop(path.clone(), crate::audio::Bus::Show, volume as f32 / 100.0);
            } else {
                self.toast(ToastKind::Error, format!("Show soundtrack \"{}\" is missing from Media. The show runs without it.", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()));
            }
            // Remember it either way so a missing file warns once, not every tick.
            self.show_bed = Some((path, volume));
        }
    }

    pub fn tick(&mut self) -> bool {
        self.restore_studio_map();
        self.sync_show_bed();
        // Pause the map engine and compositor while nothing on screen uses them.
        let idle = !self.frames_needed();
        if idle != self.map_idle {
            self.map_idle = idle;
            self.compositor.update(|p| p.idle = idle);
            self.engine.set_paused(idle);
            for e in &self.panel_engines {
                e.set_paused(idle);
            }
            if let Some(preview) = &self.map_preview {
                preview.engine.set_paused(idle);
                for e in &preview.panel_engines {
                    e.set_paused(idle);
                }
            }
            log::info!("map rendering {}", if idle { "paused (no view needs it)" } else { "resumed" });
        }

        // The title-bar clock and REC timer redraw once a second.
        let second = chrono::Utc::now().timestamp();
        let mut changed = second != self.last_clock || std::mem::take(&mut self.ui_dirty);
        self.last_clock = second;

        if let Some(fly) = self.fly {
            let t = (fly.started.elapsed().as_secs_f32() / fly.seconds).min(1.0) as f64;
            let view = if t >= 1.0 {
                fly.to
            } else {
                crate::map::geo::fly_interpolate_with(fly.from, fly.to, t, &self.settings.camera_easing, self.settings.camera_dip)
            };
            self.set_view(view);
            if t >= 1.0 {
                self.fly = None;
            }
            changed = true;
        }

        changed |= self.poll_radar_export();
        if self.playing
            && self.live.radar.enabled
            && self.live.radar.mode == RadarMode::Loop
            && self.radar_frames.len() > 1
        {
            let last = self.radar_index + 1 >= self.radar_frames.len();
            let wait = if last {
                self.settings.loop_dwell_ms
            } else {
                self.settings.loop_frame_ms
            };
            if self.last_frame_switch.elapsed() >= Duration::from_millis(wait) {
                self.radar_index = (self.radar_index + 1) % self.radar_frames.len();
                self.last_frame_switch = Instant::now();
                self.sync_map();
                changed = true;
            }
        }

        // Futurecast / data loop: ~4 model hours per second, short hold at the end.
        let dt = self.last_tick.elapsed().as_secs_f32().min(0.2);
        self.last_tick = Instant::now();
        if self.data_playing && self.live.data.field.is_some_and(|field| field.fixed_window().is_none()) {
            let end = crate::program::forecast_span(&self.live, &self.data).max(1) as f32;
            let model_pending = !crate::program::forecast_ready(&self.live, &self.data);
            let rate = end / 7.0;
            let mut hour = if model_pending { 0.0 } else { self.live.data.hour + dt * rate };
            if hour > end + rate * 0.5 {
                hour = 0.0;
            }
            self.live.data.hour = hour;
            let shown = hour.min(end);
            self.set_shown_hour(shown);
            changed = true;
        }
        // While staging, the on-air forecast loop keeps its own clock.
        if let Some(preview) = self.map_preview.as_mut().filter(|p| p.program_playing && !self.show_running) {
            let program = &mut preview.program;
            if program.data.field.is_some_and(|field| field.fixed_window().is_none()) {
                let end = crate::program::forecast_span(program, &self.data).max(1) as f32;
                let rate = end / 7.0;
                let mut hour = if crate::program::forecast_ready(program, &self.data) { program.data.hour + dt * rate } else { 0.0 };
                if hour > end + rate * 0.5 {
                    hour = 0.0;
                }
                program.data.hour = hour;
                let shown = hour.min(end);
                self.compositor.update(|p| p.live_map.data.hour = shown);
                changed = true;
            }
        }

        changed |= self.poll_show_clip();
        if self.show_running {
            if let Some(i) = self.show_pos {
                let duration = self.show.scenes.get(i).map_or(0.0, |scene| {
                    if scene.duration > 0.0 {
                        scene.duration
                    } else if matches!(scene.kind, SceneKind::Clip { .. }) {
                        self.clip_duration.unwrap_or(0.0)
                    } else if self.exporting_show {
                        8.0
                    } else {
                        0.0
                    }
                });
                let forecast_failed = if self.live.data.field == Some(crate::map::scene::DataField::Futurecast) {
                    self.model_error[self.live.data.model_product.source().index()].is_some()
                } else {
                    self.health.grid.at.is_some_and(|at| !self.health.grid.ok
                        && (chrono::Local::now() - at).num_seconds() < 60)
                };
                let waiting = self.scene_forecast_wait_until.is_some_and(|until| Instant::now() < until) && !forecast_failed;
                if self.pending_clip.is_none() && !waiting && duration > 0.0 && self.scene_elapsed() >= duration {
                    self.next_scene();
                    changed = true;
                }
            }
        }

        // A show export ends when the show stops running.
        if self.exporting_show && !self.show_running {
            self.exporting_show = false;
            if self.recording.is_some() {
                self.toggle_recording();
            }
            changed = true;
        }

        // Recordings finishing in the background.
        let mut finished = Vec::new();
        self.finishing_recordings.retain(|rx| match rx.try_recv() {
            Ok(result) => {
                finished.push(result);
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                finished.push(Err(anyhow::anyhow!("the recording stopped unexpectedly")));
                false
            }
        });
        for result in finished {
            match result {
                Ok(path) => {
                    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    self.toast(ToastKind::Success, format!("Recording saved: {name}"));
                    self.exports = library::list_exports();
                }
                Err(e) => self.toast(ToastKind::Error, format!("Recording failed: {e}")),
            }
            changed = true;
        }

        let before = self.toasts.len();
        self.toasts
            .retain(|t| t.at.elapsed() < Duration::from_secs(if t.kind == ToastKind::Error { 9 } else { 5 }));
        changed |= before != self.toasts.len();

        // Clear finished transitions so Program can share the Map bus frame again.
        let transition_done = self
            .compositor
            .read(|p| {
                p.transition
                    .as_ref()
                    .map(|t| t.started.elapsed().as_secs_f32() > t.seconds + 0.1)
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if transition_done {
            self.compositor.update(|p| p.transition = None);
        }

        changed
    }

    // ---- Data --------------------------------------------------------------------------------

    pub fn refresh_data(&mut self, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        let base = self.settings.api_base.clone();
        let home = self.settings.home;
        let area = self.settings.alert_area.clone();
        cx.spawn(async move |this, cx| {
            let radar = net::spawn(net::radar_index(base.clone()));
            let overview = net::spawn(net::overview(base.clone(), home.lat, home.lon));
            let local = net::spawn(net::alerts_for_point(home.lat, home.lon));
            let map_alerts = net::spawn(net::alerts_for_map(area));
            let cells = net::spawn(net::storm_cells(base.clone(), home.lat, home.lon, 400.0));
            let spc = net::spawn(net::spc(base.clone()));
            let reports = net::spawn(net::storm_reports());
            let roads = net::spawn(net::road_events(base));
            let mesoscale = net::spawn(net::mesoscale());
            let (radar, overview, local, map_alerts, cells, spc) =
                futures_util::join!(radar, overview, local, map_alerts, cells, spc);
            let (reports, roads, mesoscale) = futures_util::join!(reports, roads, mesoscale);

            fn flatten<T>(r: Result<anyhow::Result<T>, tokio::task::JoinError>) -> anyhow::Result<T> {
                match r {
                    Ok(inner) => inner,
                    Err(e) => Err(anyhow::anyhow!("task failed: {e}")),
                }
            }
            let radar = flatten(radar);
            let overview = flatten(overview);
            let local = flatten(local);
            let map_alerts = flatten(map_alerts);
            let cells = flatten(cells);
            let spc = flatten(spc);
            let reports = flatten(reports);
            let roads = flatten(roads);
            let mesoscale = flatten(mesoscale);

            let _ = this.update(cx, |state, cx| {
                state.apply_extras(reports, roads, mesoscale);
                state.apply_data(radar, overview, local, map_alerts, cells, spc);
                state.detect_warning_changes();
                state.refresh_tracks_from_alerts();
                state.refreshing = false;
                if std::mem::take(&mut state.reload_after_refresh) {
                    state.reload_renderers();
                }
                cx.notify();
            });
        })
        .detach();
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_data(
        &mut self,
        radar: anyhow::Result<crate::data::RadarIndex>,
        overview: anyhow::Result<crate::data::Overview>,
        local: anyhow::Result<Vec<crate::data::Alert>>,
        map_alerts: anyhow::Result<Vec<crate::data::Alert>>,
        cells: anyhow::Result<Vec<crate::data::StormCell>>,
        spc: anyhow::Result<Vec<crate::data::SpcDay>>,
    ) {
        let now = chrono::Local::now();
        fn feed_status<T>(r: &anyhow::Result<T>, ok_text: String, now: chrono::DateTime<chrono::Local>) -> FeedStatus {
            match r {
                Ok(_) => FeedStatus {
                    ok: true,
                    message: ok_text,
                    at: Some(now),
                },
                Err(e) => FeedStatus {
                    ok: false,
                    message: e.to_string(),
                    at: Some(now),
                },
            }
        }
        let mut data = (*self.data).clone();

        self.health.radar = feed_status(
            &radar,
            radar
                .as_ref()
                .map(|r| format!("{} frames", r.radar.past.len()))
                .unwrap_or_default(),
            now,
        );
        if let Ok(radar) = radar {
            data.radar = Some(radar);
        }
        self.health.overview = feed_status(&overview, "Conditions + forecast".into(), now);
        if let Ok(overview) = overview {
            data.overview = Some(overview);
        }
        let had_local = data.local_alerts.len();
        self.health.alerts = feed_status(
            &local,
            local
                .as_ref()
                .map(|a| format!("{} local alerts", a.len()))
                .unwrap_or_default(),
            now,
        );
        if let Ok(local) = local {
            if local.len() > had_local {
                if let Some(top) = local.first() {
                    self.toast(ToastKind::Error, format!("New alert: {}", top.event));
                }
            }
            data.local_alerts = local;
        }
        if let Ok(map_alerts) = map_alerts {
            data.map_alerts = map_alerts;
        }
        // Practice warnings stay until cleared.
        for test in &self.test_alerts {
            if !data.map_alerts.iter().any(|a| a.id == test.id) {
                data.map_alerts.insert(0, test.clone());
            }
            if !data.local_alerts.iter().any(|a| a.id == test.id) {
                data.local_alerts.insert(0, test.clone());
            }
        }
        self.health.cells = feed_status(
            &cells,
            cells
                .as_ref()
                .map(|c| format!("{} cells", c.len()))
                .unwrap_or_default(),
            now,
        );
        if let Ok(cells) = cells {
            // Keep a 2-hour history per cell for growth / decay trends and dBZ history charts.
            let t = chrono::Utc::now().timestamp();
            let cutoff = t - 2 * 3600;
            for c in &cells {
                let h = data.cell_history.entry(c.id.clone()).or_default();
                if h.last().map(|p| t - p.0 >= 60).unwrap_or(true) {
                    h.push((t, c.max_dbz as f32));
                }
                let a = data.cell_area_history.entry(c.id.clone()).or_default();
                if a.last().map(|p| t - p.0 >= 60).unwrap_or(true) {
                    a.push((t, c.area_sq_mi as f32));
                }
            }
            data.cell_history.retain(|_, v| {
                v.retain(|p| p.0 >= cutoff);
                !v.is_empty()
            });
            data.cell_area_history.retain(|_, v| {
                v.retain(|p| p.0 >= cutoff);
                !v.is_empty()
            });
            data.cells = cells;
            data.analysis = crate::cells::update(&data.cells, &data.lightning);
            for e in data.analysis.events.iter().filter(|e| chrono::Utc::now().timestamp() - e.time < 5) {
                log::warn!("storm analysis: {}", e.text);
            }
        }
        self.health.spc = feed_status(&spc, "Day 1-3".into(), now);
        if let Ok(spc) = spc {
            data.spc = spc;
        }
        data.updated_at = Some(now);

        let was_on_latest = self.radar_index + 1 >= self.radar_frames.len();
        self.data = Arc::new(data);
        let data = self.data.clone();
        self.compositor.update(|p| p.data = data);
        self.sync_map();
        if was_on_latest {
            self.radar_index = self.radar_frames.len().saturating_sub(1);
            self.sync_map();
        }
    }

    // ---- Library -----------------------------------------------------------------------------

    pub fn reload_library(&mut self) {
        self.graphics = library::list_graphics();
        self.shows = library::list_shows();
        self.media = library::list_media();
        self.exports = library::list_exports();
    }

    /// The graphic a show scene plays. Scenes keep a copy of their graphic, but the library
    /// version wins. When the scene's own graphic is gone from the library, a single library
    /// graphic with the same name and category takes its place; only when there is none does
    /// the show fall back to its saved copy (which may be out of date).
    pub fn scene_graphic(&self, graphic_id: &str, saved: &GraphicDoc) -> SceneGraphic {
        if let Some(doc) = self.graphics.iter().find(|g| g.id == graphic_id) {
            return SceneGraphic::Library(doc.clone());
        }
        let mut same_name = self.graphics.iter().filter(|g| {
            g.name.trim().eq_ignore_ascii_case(saved.name.trim()) && g.category == saved.category
        });
        match (same_name.next(), same_name.next()) {
            (Some(doc), None) => SceneGraphic::Relinked(doc.clone()),
            _ => SceneGraphic::Saved(saved.clone()),
        }
    }

    /// Point scenes whose graphic left the library at the same-name library graphic, and keep
    /// every saved copy in step with the library. Returns how many scenes were relinked.
    pub fn relink_show_graphics(&mut self) -> usize {
        let mut relinked = 0;
        let mut changed = false;
        let resolved: Vec<Option<SceneGraphic>> = self.show.scenes.iter().map(|scene| match &scene.kind {
            SceneKind::Graphic { graphic_id, doc } => Some(self.scene_graphic(graphic_id, doc)),
            _ => None,
        }).collect();
        for (scene, resolved) in self.show.scenes.iter_mut().zip(resolved) {
            let SceneKind::Graphic { graphic_id, doc } = &mut scene.kind else { continue };
            match resolved {
                Some(SceneGraphic::Relinked(library)) => {
                    log::info!("show scene \"{}\": graphic {} is gone; relinked to library graphic {} ({})", scene.title, graphic_id, library.id, library.name);
                    *graphic_id = library.id.clone();
                    *doc = library;
                    relinked += 1;
                    changed = true;
                }
                Some(SceneGraphic::Library(library)) if *doc != library => {
                    *doc = library;
                    changed = true;
                }
                _ => {}
            }
        }
        if changed && !self.show.scenes.is_empty() && self.shows.iter().any(|s| s.id == self.show.id) {
            let mut show = self.show.clone();
            if library::save_show(&mut show).is_ok() {
                if let Some(existing) = self.shows.iter_mut().find(|s| s.id == show.id) {
                    *existing = show.clone();
                }
                self.show = show;
            }
        }
        relinked
    }

    pub fn save_graphic(&mut self, mut doc: GraphicDoc) -> Option<GraphicDoc> {
        match library::save_graphic(&mut doc) {
            Ok(_) => {
                if let Some(existing) = self.graphics.iter_mut().find(|g| g.id == doc.id) {
                    *existing = doc.clone();
                } else {
                    self.graphics.insert(0, doc.clone());
                }
                // Keep on-air copies current.
                let updated = doc.clone();
                self.compositor.update(|p| {
                    if let Some(overlay) = &mut p.map_overlay {
                        if overlay.id == updated.id {
                            *overlay = updated.clone();
                        }
                    }
                    for (overlay, _) in p.map_overlay_slots.iter_mut().flatten() {
                        if overlay.id == updated.id { *overlay = updated.clone(); }
                    }
                    if let ProgramSource::Graphic(on_air) = &mut p.source {
                        if on_air.id == updated.id {
                            *on_air = updated.clone();
                        }
                    }
                });
                Some(doc)
            }
            Err(e) => {
                self.toast(ToastKind::Error, format!("Could not save graphic: {e}"));
                None
            }
        }
    }

    pub fn delete_graphic(&mut self, id: &str) {
        if let Err(e) = library::delete_graphic(id) {
            self.toast(ToastKind::Error, format!("Could not delete: {e}"));
        }
        self.graphics.retain(|g| g.id != id);
        if self.live.overlay_graphic.as_deref() == Some(id) {
            self.set_map_overlay(None);
        }
        for slot in &mut self.live.overlay_slots {
            if slot.as_deref() == Some(id) { *slot = None; }
        }
        self.sync_map();
    }

    pub fn add_map_to_show(&mut self, animated: bool) {
        let replay = self.live.radar_replay.filter(|_| self.live.data.field.is_none());
        let site_replay = self.live.site_replay.as_ref().filter(|_| self.live.data.field.is_none());
        if animated && self.live.data.field.is_some_and(|field| field.fixed_window().is_some()) {
            self.toast(ToastKind::Info, "This outlook covers a fixed window. Add it as a still.");
            return;
        }
        if animated && replay.is_none() && site_replay.is_none() && !crate::program::forecast_ready(&self.live, &self.data) {
            self.toast(ToastKind::Info, "Wait for the forecast to finish loading before adding a loop");
            return;
        }
        let mut map = self.live.clone();
        map.data.play_on_take = animated && replay.is_none() && site_replay.is_none();
        let pins = std::array::from_fn(|i| {
            let source = crate::futurecast::Source::ALL[i];
            map.data.model_pins[i].or_else(|| map.model_products().any(|p| p.source() == source)
                .then(|| self.data.model(source).map(|model| model.pin())).flatten())
        });
        map.data.model_pins = pins;
        if !animated && replay.is_some() {
            map.radar_replay = None;
            map.history_time = self.current_radar_time();
        }
        if let Some(replay) = map.site_replay.as_mut().filter(|_| !animated) {
            replay.scans = self.current_radar_time().into_iter().collect();
        }
        let title = if let Some(site) = site_replay.filter(|_| animated) {
            format!("K{} velocity replay", site.site)
        } else if let Some(replay) = replay.filter(|_| animated) {
            format!("Observed radar · {}h replay", replay.hours)
        } else if animated {
            let field = if map.data.field == Some(crate::map::scene::DataField::Futurecast) {
                format!("{} {}", map.data.model_product.source().label(), map.data.model_product.label())
            } else {
                map.data.field.expect("checked forecast").label().to_owned()
            };
            format!("{field} · {}h loop", crate::program::forecast_span(&map, &self.data))
        } else if map.banner_text().trim().is_empty() {
            format!("Map {}", self.show.scenes.len() + 1)
        } else {
            map.banner_text().into_owned()
        };
        let mut scene = crate::show::Scene::map(&title, map);
        if animated && (replay.is_some() || site_replay.is_some()) {
            let count = site_replay.map_or_else(|| replay.map_or(0, |r| r.frames().len()), |r| r.scans.len());
            let intervals = count.saturating_sub(1) as f32;
            let millis = intervals * self.settings.loop_frame_ms as f32 + self.settings.loop_dwell_ms as f32;
            scene.duration = (millis / 1000.0 + scene.transition.seconds()).max(scene.duration);
        }
        self.show.scenes.push(scene);
        self.save_show();
    }

    pub fn save_show(&mut self) {
        let mut show = self.show.clone();
        match library::save_show(&mut show) {
            Ok(_) => {
                self.show = show.clone();
                if let Some(existing) = self.shows.iter_mut().find(|s| s.id == show.id) {
                    *existing = show;
                } else {
                    self.shows.insert(0, show);
                }
                self.toast(ToastKind::Success, "Show saved");
            }
            Err(e) => self.toast(ToastKind::Error, format!("Could not save show: {e}")),
        }
    }

    // ---- Settings ----------------------------------------------------------------------------

    pub fn apply_settings(&mut self, mut settings: Settings) {
        settings.api_base = crate::backend::effective_base(&settings);
        crate::gfx::look::set_default(&settings.graphics_look);
        if settings.presentation_muted != self.settings.presentation_muted
            || settings.graphic_volume != self.settings.graphic_volume
        {
            crate::audio::set_presentation_mix(settings.presentation_muted, settings.graphic_volume);
        }
        let api_changed = settings.api_base != self.settings.api_base;
        // Changed map defaults apply to the live map right away.
        let old = self.settings.clone();
        if settings.basemap != old.basemap {
            self.live.basemap = settings.basemap;
        }
        if settings.radar_palette != old.radar_palette {
            self.live.radar.palette = settings.radar_palette.clone();
        }
        crate::map::winter_radar::set_enabled(settings.snow_radar_repaint);
        if (settings.radar_opacity - old.radar_opacity).abs() > f32::EPSILON {
            self.live.radar.opacity = settings.radar_opacity;
        }
        if settings.show_logo_bug != old.show_logo_bug {
            self.live.show_logo = settings.show_logo_bug;
        }
        if settings.show_clock_bug != old.show_clock_bug {
            self.live.show_clock = settings.show_clock_bug;
        }
        if settings.show_legend != old.show_legend {
            self.live.show_legend = settings.show_legend;
        }
        if settings.show_timestamp != old.show_timestamp {
            self.live.show_timestamp = settings.show_timestamp;
        }
        self.settings = settings;
        net::set_api_base(&self.settings.api_base);
        if api_changed {
            self.model_retry = [Instant::now(); 3];
            self.model_error = [None, None, None];
            Arc::make_mut(&mut self.data).models = [None, None, None];
            self.compositor.update(|p| p.data = self.data.clone());
        }
        if let Err(e) = self.settings.save() {
            self.toast(ToastKind::Error, format!("Could not save settings: {e}"));
        } else {
            self.toast(ToastKind::Success, "Settings saved");
        }
        let arc = Arc::new(self.settings.clone());
        self.compositor.update(|p| p.settings = arc);
        self.push_sponsor();
        if api_changed {
            self.last_style.clear();
        }
        self.sync_map();
    }

    // ---- Recording ---------------------------------------------------------------------------

    pub fn toggle_recording(&mut self) {
        if let Some(recording) = self.recording.take() {
            // Finished off the UI thread: a long show used to freeze the app here, and closing
            // it while frozen left a broken MP4.
            self.finishing_recordings.push(recording.stop_in_background(&self.compositor));
            self.toast(ToastKind::Info, "Finishing the video… keep the app open until it says it's saved");
        } else {
            match Recording::start(
                &self.compositor,
                self.settings.record_format,
                self.settings.record_fps,
                LibraryPaths::get().exports,
            ) {
                Ok(recording) => {
                    self.recording = Some(recording);
                    self.toast(ToastKind::Info, "Recording program output");
                }
                Err(e) => self.toast(ToastKind::Error, format!("Could not start recording: {e}")),
            }
        }
    }

    // ---- Updates -----------------------------------------------------------------------------

    pub fn check_updates(&mut self, manual: bool, cx: &mut Context<Self>) {
        if matches!(self.update, UpdateState::Checking | UpdateState::Downloading { .. }) {
            return;
        }
        self.update = UpdateState::Checking;
        let feed = self.settings.update_feed_for_channel();
        let beta = self.settings.update_beta();
        cx.spawn(async move |this, cx| {
            let result = net::spawn(updater::check(feed, beta)).await;
            let _ = this.update(cx, |state, cx| {
                state.update = match result {
                    Ok(Ok(Some(manifest))) => {
                        let skipped = state.settings.skipped_version.as_deref() == Some(manifest.label().as_str());
                        if !skipped || manual {
                            state.toast(
                                ToastKind::Info,
                                format!("Update available: version {}", manifest.label()),
                            );
                        }
                        UpdateState::Available(manifest)
                    }
                    Ok(Ok(None)) => {
                        if manual {
                            state.toast(ToastKind::Success, "You're on the latest version");
                        }
                        UpdateState::UpToDate {
                            checked_at: chrono::Local::now(),
                        }
                    }
                    Ok(Err(e)) => {
                        if manual {
                            state.toast(ToastKind::Error, format!("Update check failed: {e}"));
                        }
                        UpdateState::Failed(e.to_string())
                    }
                    Err(e) => UpdateState::Failed(e.to_string()),
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub fn download_update(&mut self, cx: &mut Context<Self>) {
        let UpdateState::Available(manifest) = self.update.clone() else {
            return;
        };
        if !updater::SELF_INSTALL {
            let page = if manifest.release_page.is_empty() { format!("{}/releases", updater::DEFAULT_FEED) } else { manifest.release_page.clone() };
            let _ = open::that(&page);
            self.toast(ToastKind::Info, format!("Version {} is on the download page that just opened", manifest.label()));
            return;
        }
        let version = manifest.label();
        self.update = UpdateState::Downloading {
            version: version.clone(),
            downloaded: 0,
            total: None,
        };
        let progress = Arc::new(std::sync::Mutex::new((0u64, None::<u64>)));
        let sink = progress.clone();
        let task = net::spawn(updater::download(manifest, move |done, total| {
            if let Ok(mut p) = sink.lock() {
                *p = (done, total);
            }
        }));
        cx.spawn(async move |this, cx| {
            // Report progress while the download runs.
            let mut task = task;
            loop {
                let finished = futures_util::future::select(
                    &mut task,
                    Box::pin(cx.background_executor().timer(Duration::from_millis(250))),
                )
                .await;
                match finished {
                    futures_util::future::Either::Left((result, _)) => {
                        let _ = this.update(cx, |state, cx| {
                            state.update = match result {
                                Ok(Ok(installer)) => {
                                    state.toast(ToastKind::Success, "Update downloaded and verified");
                                    UpdateState::Ready {
                                        version: version.clone(),
                                        installer,
                                    }
                                }
                                Ok(Err(e)) => {
                                    state.toast(ToastKind::Error, format!("Update failed: {e}"));
                                    UpdateState::Failed(e.to_string())
                                }
                                Err(e) => UpdateState::Failed(e.to_string()),
                            };
                            cx.notify();
                        });
                        break;
                    }
                    futures_util::future::Either::Right(_) => {
                        let (done, total) = progress.lock().map(|p| *p).unwrap_or((0, None));
                        let _ = this.update(cx, |state, cx| {
                            state.update = UpdateState::Downloading {
                                version: version.clone(),
                                downloaded: done,
                                total,
                            };
                            cx.notify();
                        });
                    }
                }
            }
        })
        .detach();
    }

    pub fn install_update(&mut self) {
        if let UpdateState::Ready { installer, .. } = &self.update {
            if let Some(recording) = self.recording.take() {
                let _ = recording.stop(&self.compositor);
            }
            if let Err(e) = updater::install_and_exit(installer) {
                self.toast(ToastKind::Error, format!("Could not start the installer: {e}"));
            }
        }
    }
}

/// A file-name-safe version of a title.
fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "Show".into()
    } else {
        cleaned
    }
}

/// GPU images of the latest bus frames, refreshed by the frame pump.
#[derive(Default)]
pub struct FrameStore {
    pub map: Option<Arc<RenderImage>>,
    pub program: Option<Arc<RenderImage>>,
    map_serial: u64,
    program_serial: u64,
}

fn to_image(frame: &BusFrame) -> Option<Arc<RenderImage>> {
    let buffer = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra.as_ref().clone())?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

impl FrameStore {
    /// Pull new frames from the compositor. Returns true when anything changed.
    pub fn pull(&mut self, compositor: &Compositor) -> bool {
        let mut changed = false;
        let (map, program) = compositor.frames();
        if let Some(frame) = &map {
            if frame.serial != self.map_serial {
                self.map_serial = frame.serial;
                self.map = to_image(frame);
                changed = true;
            }
        }
        if let Some(frame) = &program {
            if frame.serial != self.program_serial {
                self.program_serial = frame.serial;
                // Program usually *is* the map frame: share the image.
                self.program = if map.as_ref().map(|m| m.serial) == Some(frame.serial) {
                    self.map.clone()
                } else {
                    to_image(frame)
                };
                changed = true;
            }
        }
        changed
    }
}

pub struct GlobalApp {
    pub state: Entity<AppState>,
    pub frames: Entity<FrameStore>,
}

impl Global for GlobalApp {}

pub fn app_entity(cx: &App) -> Entity<AppState> {
    cx.global::<GlobalApp>().state.clone()
}

pub fn frames_entity(cx: &App) -> Entity<FrameStore> {
    cx.global::<GlobalApp>().frames.clone()
}

/// Create the global state and start the background loops.
pub fn init(cx: &mut App) {
    let state = cx.new(|_| AppState::new());
    let frames = cx.new(|_| FrameStore::default());
    cx.set_global(GlobalApp {
        state: state.clone(),
        frames: frames.clone(),
    });

    // National town list (storm ETAs, towns in warnings): cached after the first download.
    let _ = net::spawn(async {
        if let Err(e) = crate::places::ensure_loaded().await {
            log::warn!("national place list unavailable, using the built-in list: {e}");
        }
    });
    // County / state outlines: cached after the first download.
    let _ = net::spawn(async {
        if let Err(e) = crate::map::boundaries::ensure_loaded().await {
            log::warn!("county outlines unavailable: {e}");
        }
    });

    // Initial data + periodic refresh.
    state.update(cx, |s, cx| {
        s.refresh_data(cx);
        if s.settings.auto_check_updates {
            s.check_updates(false, cx);
        }
    });
    let weak_state = state.downgrade();
    cx.spawn(async move |cx| loop {
        let seconds = weak_state
            .read_with(cx, |s, _| s.settings.poll_seconds.max(30))
            .unwrap_or(120);
        cx.background_executor().timer(Duration::from_secs(seconds)).await;
        if weak_state.update(cx, |s, cx| s.refresh_data(cx)).is_err() {
            break;
        }
    })
    .detach();

    // 30 fps: animation tick + frame pump.
    let weak_state = state.downgrade();
    let weak_frames = frames.downgrade();
    cx.spawn(async move |cx| loop {
        cx.background_executor().timer(Duration::from_millis(33)).await;
        let Ok(compositor) = weak_state.update(cx, |s, cx| {
            s.maybe_fetch_futurecast(cx);
            s.maybe_fetch_grid(cx);
            s.maybe_fetch_lightning(cx);
            if let Some(mode) = s.pending_mode.take() {
                s.set_severe_mode(mode, cx);
            }
            s.ext_tick(cx);
            if s.tick() {
                cx.notify();
            }
            s.compositor.clone()
        }) else {
            break;
        };
        if weak_frames
            .update(cx, |f, cx| {
                if f.pull(&compositor) {
                    cx.notify();
                }
            })
            .is_err()
        {
            break;
        }
    })
    .detach();
}

// ---- Severe Weather Center ------------------------------------------------------------------

/// One-click broadcast modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SevereMode {
    Off,
    Severe,
    TornadoEmergency,
    FlashFloodEmergency,
}

impl SevereMode {
    pub fn label(self) -> &'static str {
        match self {
            SevereMode::Off => "Normal",
            SevereMode::Severe => "Severe Weather Mode",
            SevereMode::TornadoEmergency => "Tornado Emergency Mode",
            SevereMode::FlashFloodEmergency => "Flash Flood Emergency Mode",
        }
    }
}

/// A line in the live event log.
#[derive(Clone, Debug)]
pub struct LogEntry {
    pub at: chrono::DateTime<chrono::Local>,
    pub text: String,
    /// 0 info, 1 notable, 2 critical.
    pub level: u8,
}

impl AppState {
    pub fn log(&mut self, level: u8, text: impl Into<String>) {
        self.event_log.insert(
            0,
            LogEntry {
                at: chrono::Local::now(),
                text: text.into(),
                level,
            },
        );
        self.event_log.truncate(300);
        self.ui_dirty = true;
    }

    /// Short-fused warnings in the coverage area, most dangerous and soonest-expiring first.
    pub fn coverage_warnings(&self) -> Vec<crate::data::Alert> {
        let home = self.settings.home;
        let radius_km = self.settings.coverage_miles.max(5.0) * crate::map::geo::KM_PER_MILE;
        let mut list: Vec<crate::data::Alert> = self
            .data
            .map_alerts
            .iter()
            .chain(self.data.local_alerts.iter())
            .filter(|a| a.is_short_fused())
            .filter(|a| {
                let near = |lat: f64, lon: f64, km: f64| {
                    a.contains(lat, lon)
                        || a.centroid()
                            .map(|(la, lo)| crate::map::geo::haversine_km(lat, lon, la, lo) <= km)
                            .unwrap_or(false)
                };
                near(home.lat, home.lon, radius_km)
                    || self
                        .settings
                        .alert_zones
                        .iter()
                        .any(|z| near(z.lat, z.lon, z.miles.max(1.0) * crate::map::geo::KM_PER_MILE))
            })
            .cloned()
            .collect();
        list.sort_by_key(|a| a.id.clone());
        list.dedup_by(|a, b| a.id == b.id);
        list.sort_by(|a, b| {
            let threat = |x: &crate::data::Alert| if x.threat_tag().is_some() { 0 } else { 1 };
            threat(a)
                .cmp(&threat(b))
                .then(a.priority().cmp(&b.priority()))
                .then(a.minutes_left().unwrap_or(i64::MAX).cmp(&b.minutes_left().unwrap_or(i64::MAX)))
        });
        list
    }

    pub fn find_alert(&self, id: &str) -> Option<crate::data::Alert> {
        self.data
            .map_alerts
            .iter()
            .chain(self.data.local_alerts.iter())
            .find(|a| a.id == id)
            .cloned()
    }

    /// After each refresh: new warnings in coverage (sound, toast, log, optional auto mode),
    /// and warnings that expired or were cancelled.
    /// A practice Tornado Warning southwest of home, moving toward it, for rehearsing coverage.
    /// Everything in it is marked TEST; it never leaves the app.
    pub fn start_test_warning(&mut self) {
        use crate::map::geo::destination;
        let home = self.settings.home;
        let n = self.test_alerts.len() + 1;
        // Storm 14 km southwest of home, moving from 225 degrees (toward the northeast) at 30 kt.
        let (slat, slon) = destination(home.lat, home.lon, 225.0, 14.0);
        let ahead = |b: f64, km: f64| destination(slat, slon, b, km);
        let corners = [ahead(135.0, 6.0), ahead(315.0, 6.0), ahead(10.0, 34.0), ahead(80.0, 34.0)];
        let mut ring: Vec<(f64, f64)> = corners.iter().map(|(la, lo)| (*lo, *la)).collect();
        ring.push(ring[0]);
        let now = chrono::Utc::now();
        let expires = now + chrono::Duration::minutes(45);
        let alert = crate::data::Alert {
            id: format!("TEST-{}-{n}", now.timestamp()),
            event: "Tornado Warning".into(),
            headline: "TEST - Tornado Warning (practice, not a real warning)".into(),
            severity: "Extreme".into(),
            urgency: "Immediate".into(),
            area_desc: format!("TEST AREA near {}", self.settings.home_label),
            sender: "OpenWX rehearsal (TEST)".into(),
            sent: Some(now.to_rfc3339()),
            expires: Some(expires.to_rfc3339()),
            description: "THIS IS A TEST. A practice tornado warning for rehearsing severe weather coverage. \
At this time a radar-indicated tornado was located near the test area, moving northeast at 35 mph. \
HAZARD...Tornado. SOURCE...Radar indicated rotation. IMPACT...Practice only.".into(),
            instruction: "TEST ONLY. Practice your sheltering message here.".into(),
            polygons: vec![ring],
            motion_text: Some(format!(
                "{}...storm...225DEG...30KT...{:.2},{:.2}",
                now.format("%H%MZ"),
                slat,
                slon
            )),
            message_type: "Alert".into(),
            fips: Vec::new(),
            county_based: false,
        };
        self.test_alerts.push(alert.clone());
        let mut data = (*self.data).clone();
        data.map_alerts.insert(0, alert.clone());
        data.local_alerts.insert(0, alert);
        self.data = Arc::new(data);
        let snapshot = self.data.clone();
        self.compositor.update(|p| p.data = snapshot);
        self.toast(ToastKind::Info, "Practice Tornado Warning added (marked TEST). Clear it with Clear practice warnings.");
    }

    /// Remove every practice warning.
    pub fn clear_test_warnings(&mut self) {
        if self.test_alerts.is_empty() {
            return;
        }
        let ids: Vec<String> = self.test_alerts.drain(..).map(|a| a.id).collect();
        let mut data = (*self.data).clone();
        data.map_alerts.retain(|a| !ids.contains(&a.id));
        data.local_alerts.retain(|a| !ids.contains(&a.id));
        self.data = Arc::new(data);
        let snapshot = self.data.clone();
        self.compositor.update(|p| p.data = snapshot);
        if self.live.selected_alert.as_ref().map(|id| ids.contains(id)).unwrap_or(false) {
            self.select_alert(None);
        }
        // Their graphics (named "PRACTICE · …") come off Program, the map and the library.
        let practice: Vec<GraphicDoc> = self.graphics.iter().filter(|g| g.name.starts_with("PRACTICE")).cloned().collect();
        if matches!(&self.on_air, OnAir::Graphic(name) if name.starts_with("PRACTICE")) {
            self.take_live_map(Transition::Fade);
        }
        let practice_ids: Vec<&str> = practice.iter().map(|g| g.id.as_str()).collect();
        if self.live.overlay_graphic.as_deref().is_some_and(|id| practice_ids.contains(&id)) {
            self.set_map_overlay(None);
        }
        for (slot, id) in self.live.overlay_slots.clone().into_iter().enumerate() {
            if id.as_deref().is_some_and(|id| practice_ids.contains(&id)) {
                self.set_map_overlay_slot(slot, None);
            }
        }
        for g in &practice {
            self.delete_graphic(&g.id);
        }
        self.toast(ToastKind::Info, "Practice warnings cleared, and their graphics taken off air");
    }

    pub fn detect_warning_changes(&mut self) {
        let current = self.coverage_warnings();
        let ids: std::collections::HashSet<String> = current.iter().map(|a| a.id.clone()).collect();
        if !self.alerts_primed {
            // First load: everything already active is "known", nothing beeps.
            self.alerts_primed = true;
            for a in &current {
                self.log(1, format!("Active: {} - {}", a.event, a.area_desc));
            }
            self.seen_alerts = ids;
            return;
        }
        let mut urgent = false;
        let mut any_new = false;
        let fresh: Vec<crate::data::Alert> = current.iter().filter(|a| !self.seen_alerts.contains(&a.id)).cloned().collect();
        if !fresh.is_empty() {
            let today = chrono::Local::now().date_naive();
            let mut data = (*self.data).clone();
            data.warning_history.retain(|(t, _, _)| {
                chrono::DateTime::from_timestamp(*t, 0).map(|d| d.with_timezone(&chrono::Local).date_naive() == today).unwrap_or(false)
            });
            for a in &fresh {
                if !a.id.starts_with("TEST-") && !a.message_type.eq_ignore_ascii_case("Update") {
                    let towns = a.towns();
                    let area = if towns.is_empty() { a.area_desc.clone() } else { towns.into_iter().take(4).collect::<Vec<_>>().join(", ") };
                    data.warning_history.push((chrono::Utc::now().timestamp(), a.event.clone(), area));
                }
            }
            self.data = Arc::new(data);
            let snapshot = self.data.clone();
            self.compositor.update(|p| p.data = snapshot);
        }
        for a in fresh.iter() {
            any_new = true;
            crate::ops::archive_alert(a);
            let tag = a.threat_tag().map(|t| format!(" • {t}")).unwrap_or_default();
            let towns = a.towns();
            let towns = if towns.is_empty() { a.area_desc.clone() } else { towns.join(", ") };
            let is_update = a.message_type.eq_ignore_ascii_case("Update");
            let verb = if is_update { "Updated" } else { "NEW" };
            self.log(2, format!("{verb}: {}{tag} - {towns}", a.event));
            self.toast(ToastKind::Error, format!("{verb} {}{tag} for {towns}", a.event));
            if a.event.to_lowercase().contains("tornado") || a.threat_tag().is_some() {
                urgent = true;
            }
        }
        // Automated zoom: the most dangerous new warning gets the map.
        if self.settings.auto_zoom_threat && !self.black {
            let worst = fresh
                .iter()
                .filter(|a| a.is_warning() && !a.message_type.eq_ignore_ascii_case("Update"))
                .filter(|a| a.event.to_lowercase().contains("tornado") || a.threat_tag().is_some())
                .min_by_key(|a| a.priority())
                .map(|a| (a.id.clone(), a.event.clone()));
            if let Some((id, event)) = worst {
                self.zoom_to_alert(&id);
                self.select_alert(Some(id));
                self.log(2, format!("Auto zoom: {event}"));
            }
        }
        for id in self.seen_alerts.clone().difference(&ids) {
            self.log(0, "A warning in the coverage area expired or was cancelled".to_string());
            if self.live.selected_alert.as_deref() == Some(id.as_str()) {
                self.select_alert(None);
            }
        }
        self.seen_alerts = ids;
        if any_new {
            // Escalation: tornado warnings and PDS / emergency tags sound even in quiet hours.
            let quiet = self.in_quiet_hours() && !(urgent && self.settings.quiet_allow_urgent);
            if self.settings.alert_sounds && !quiet {
                crate::obs::play_alert_sound(urgent);
            } else if quiet {
                self.log(0, "Quiet hours: warning sound muted".to_string());
            }
            let wants_mode = !self.settings.auto_severe_tornado_only || urgent;
            if self.settings.auto_severe_mode && wants_mode && self.severe_mode == SevereMode::Off {
                let mode = if current.iter().any(|a| a.threat_tag() == Some("TORNADO EMERGENCY")) {
                    SevereMode::TornadoEmergency
                } else {
                    SevereMode::Severe
                };
                self.pending_mode = Some(mode);
            }
        }
    }

    /// Inside the quiet-hours window (Settings > Severe weather)?
    pub fn in_quiet_hours(&self) -> bool {
        if !self.settings.quiet_hours {
            return false;
        }
        use chrono::Timelike as _;
        let h = chrono::Local::now().hour();
        let (start, end) = (self.settings.quiet_start % 24, self.settings.quiet_end % 24);
        if start == end {
            false
        } else if start < end {
            h >= start && h < end
        } else {
            h >= start || h < end
        }
    }

    /// Watch a spot for warnings (map details card > Watch this spot).
    pub fn add_alert_zone(&mut self, name: String, lat: f64, lon: f64) {
        let mut settings = self.settings.clone();
        if settings
            .alert_zones
            .iter()
            .any(|z| crate::map::geo::haversine_km(z.lat, z.lon, lat, lon) < 1.0)
        {
            self.toast(ToastKind::Info, format!("{name} is already watched"));
            return;
        }
        settings.alert_zones.push(crate::config::AlertZone { name: name.clone(), lat, lon, miles: 10.0 });
        self.apply_settings(settings);
        self.toast(ToastKind::Success, format!("Watching {name} for warnings (10 mi). Change it in Settings > Weather & maps > Severe weather"));
    }

    /// Frame the map on a warning polygon.
    pub fn zoom_to_alert(&mut self, id: &str) {
        let Some(alert) = self.find_alert(id) else { return };
        let (mut n, mut s, mut w, mut e) = (f64::MIN, f64::MAX, f64::MAX, f64::MIN);
        for ring in &alert.polygons {
            for (lon, lat) in ring {
                n = n.max(*lat);
                s = s.min(*lat);
                w = w.min(*lon);
                e = e.max(*lon);
            }
        }
        if n < s {
            return;
        }
        let center = GeoView {
            lat: (n + s) / 2.0,
            lon: (w + e) / 2.0,
            zoom: 6.0,
        };
        // Largest zoom where the polygon fills at most ~55% of the frame.
        let mut zoom = 5.0;
        for step in 0..60 {
            let z = 5.0 + step as f64 * 0.1;
            let vp = crate::map::geo::Viewport::new(GeoView { zoom: z, ..center }, crate::config::FRAME_W, crate::config::FRAME_H);
            let (x0, y0) = vp.to_screen(n, w);
            let (x1, y1) = vp.to_screen(s, e);
            if (x1 - x0).abs() > crate::config::FRAME_W as f32 * 0.55 || (y1 - y0).abs() > crate::config::FRAME_H as f32 * 0.55 {
                break;
            }
            zoom = z;
        }
        self.fly_to(GeoView { zoom, ..center }, 1.4);
    }

    /// Storm track from the warning's NWS storm motion. An existing track is kept as an extra.
    pub fn track_alert(&mut self, id: &str) {
        let Some(alert) = self.find_alert(id) else { return };
        let Some(m) = alert.motion() else {
            self.toast(ToastKind::Error, "This warning has no storm motion - draw the track with S or F");
            return;
        };
        let label = format!("{} STORM", alert.short_code());
        self.update_live(|l| {
            if let Some(old) = l.storm_track.take() {
                l.extra_tracks.push(old);
            }
            l.storm_track = Some(crate::map::scene::StormTrack {
                lat: m.lat,
                lon: m.lon,
                heading_deg: m.heading_deg,
                speed_mph: m.speed_mph.round(),
                label,
                color: alert.color().to_string(),
                ..Default::default()
            });
            l.extra_tracks.truncate(5);
        });
        self.log(1, format!("Tracking {}: {} at {:.0} mph", alert.event, crate::map::geo::compass(m.heading_deg), m.speed_mph));
    }

    /// Re-aim every storm track that came from a warning when that warning's motion updates.
    /// After each data refresh: keep every storm cone on its storm. The cone's storm is looked
    /// for where it should be by now (its last position moved along its motion); the nearest
    /// moving storm cell there gives the new position, heading and speed, or else a warning's
    /// motion line projected to now. Arrival times follow from the new position.
    pub fn refresh_tracks_from_alerts(&mut self) {
        use crate::map::geo::{destination, haversine_km};
        let alerts: Vec<crate::data::Alert> = self.coverage_warnings();
        let now = chrono::Utc::now().timestamp();
        let mut changed = false;
        let mut live = self.live.clone();
        for track in live.storm_track.iter_mut().chain(live.extra_tracks.iter_mut()) {
            if track.is_freeform() || !track.follow {
                continue;
            }
            let hours = if track.placed_at > 0 { ((now - track.placed_at) as f64 / 3600.0).clamp(0.0, 2.0) } else { 0.0 };
            let (plat, plon) = destination(track.lat, track.lon, track.heading_deg, track.speed_mph * 1.609_344 * hours);
            let cell = self
                .data
                .cells
                .iter()
                .filter(|c| c.motion.is_some() && c.intensity.level >= 2)
                .map(|c| (haversine_km(plat, plon, c.lat, c.lon), c))
                .filter(|(km, _)| *km < 25.0)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, c)| c);
            let next = if let Some(c) = cell {
                let m = c.motion.as_ref().expect("filtered on motion");
                Some((c.lat, c.lon, m.heading_deg, m.speed_mph.max(5.0)))
            } else {
                alerts.iter().find_map(|a| {
                    let m = a.motion()?;
                    let sent = a.sent.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())?;
                    let h = ((now - sent.timestamp()) as f64 / 3600.0).clamp(0.0, 2.0);
                    let (lat, lon) = destination(m.lat, m.lon, m.heading_deg, m.speed_mph * 1.609_344 * h);
                    (haversine_km(plat, plon, lat, lon) < 30.0).then_some((lat, lon, m.heading_deg, m.speed_mph))
                })
            };
            if let Some((lat, lon, heading, mph)) = next {
                if haversine_km(lat, lon, track.lat, track.lon) > 0.2 || (heading - track.heading_deg).abs() > 1.0 {
                    track.lat = lat;
                    track.lon = lon;
                    track.heading_deg = heading;
                    track.speed_mph = mph.round();
                    changed = true;
                }
                track.placed_at = now;
            }
        }
        if changed {
            self.update_live(|l| *l = live);
            self.log(0, "Storm cones moved with their storms");
        }
    }

    /// Generated graphic for a warning: lower third on the map, or a full-screen take.
    pub fn warning_graphic(&mut self, id: &str, full_screen: bool) {
        let Some(alert) = self.find_alert(id) else { return };
        if full_screen {
            self.mark_aired(id);
        }
        let doc = if full_screen {
            crate::gfx::templates::warning_fullscreen(&alert)
        } else {
            crate::gfx::templates::warning_lower_third(&alert)
        };
        if let Some(saved) = self.save_graphic(doc) {
            if full_screen {
                self.take_graphic(saved, Transition::Fade);
                self.toast(ToastKind::Info, format!("{} full-screen is on Program", alert.event));
            } else {
                self.set_map_overlay(Some(saved.id));
                self.toast(ToastKind::Info, format!("{} lower third is on the map", alert.event));
            }
        }
    }

    /// Put a safety card on Program.
    pub fn safety_graphic(&mut self, hazard: &str) {
        let doc = crate::gfx::templates::safety_card(hazard);
        if let Some(saved) = self.save_graphic(doc) {
            self.take_graphic(saved, Transition::Fade);
        }
    }

    /// The Alert Crawl overlay from the library, created on first use.
    fn crawl_overlay_id(&mut self) -> Option<String> {
        if let Some(g) = self.graphics.iter().find(|g| g.name == "Alert Crawl") {
            return Some(g.id.clone());
        }
        let doc = crate::gfx::templates::build("alert_crawl")?;
        self.save_graphic(doc).map(|d| d.id)
    }

    /// One-click broadcast modes. The current map is saved and restored by Normal.
    pub fn set_severe_mode(&mut self, mode: SevereMode, cx: &mut Context<Self>) {
        if mode == self.severe_mode {
            return;
        }
        if mode == SevereMode::Off {
            if let Some(saved) = self.saved_scene.take() {
                let view = saved.view;
                self.update_live(|l| *l = saved);
                self.fly_to(view, 1.4);
                let overlay = self.live.overlay_graphic.clone();
                self.set_map_overlay(overlay);
            }
            self.severe_mode = SevereMode::Off;
            self.take_live_map(Transition::Fade);
            self.log(1, "Back to normal coverage");
            let scene = self.settings.obs_normal_scene.clone();
            self.obs_switch(scene, cx);
            return;
        }
        if self.severe_mode == SevereMode::Off {
            self.saved_scene = Some(self.live.clone());
        }
        self.severe_mode = mode;
        let warnings = self.coverage_warnings();
        let target = match mode {
            SevereMode::TornadoEmergency => warnings
                .iter()
                .find(|a| a.event.to_lowercase().contains("tornado"))
                .or(warnings.first())
                .cloned(),
            SevereMode::FlashFloodEmergency => warnings
                .iter()
                .find(|a| a.event.to_lowercase().contains("flood"))
                .or(warnings.first())
                .cloned(),
            _ => warnings.first().cloned(),
        };
        let crawl = self.crawl_overlay_id();
        let home = self.settings.home;
        let severe_rings = self.settings.severe_range_rings;
        self.update_live(|l| {
            l.radar.enabled = true;
            l.radar.mode = crate::map::scene::RadarMode::Loop;
            l.warnings = true;
            l.warnings_fill = true;
            l.warning_countdowns = true;
            l.alert_kinds.warnings = true;
            l.lightning = true;
            l.storm_cells = true;
            l.data.field = None;
            l.show_title = true;
            l.show_legend = true;
            l.overlay_graphic = crawl.clone();
            match mode {
                SevereMode::Severe => {
                    l.layout = crate::map::scene::PanelLayout::Single;
                    l.radar.source = crate::map::scene::RadarSource::Composite;
                    l.title = "SEVERE WEATHER COVERAGE".into();
                }
                SevereMode::TornadoEmergency => {
                    // Reflectivity beside storm-relative velocity from the nearest radar.
                    l.title = "TORNADO EMERGENCY".into();
                    l.radar.source = crate::map::scene::RadarSource::Composite;
                    l.layout = crate::map::scene::PanelLayout::Dual;
                    while l.panels.len() < 3 {
                        let next = crate::map::scene::default_panels()[l.panels.len()].clone();
                        l.panels.push(next);
                    }
                    l.panels[0].product = crate::map::scene::PanelProduct::StormRelative;
                }
                SevereMode::FlashFloodEmergency => {
                    l.title = "FLASH FLOOD EMERGENCY".into();
                    l.layout = crate::map::scene::PanelLayout::Single;
                    l.radar.source = crate::map::scene::RadarSource::Composite;
                    l.precip_total = Some(crate::map::scene::PrecipWindow::Hours24);
                }
                SevereMode::Off => {}
            }
            if severe_rings && l.range_rings.is_none() {
                l.range_rings = Some(crate::map::scene::RangeRings {
                    lat: home.lat,
                    lon: home.lon,
                    ..Default::default()
                });
            }
        });
        self.set_map_overlay(crawl);
        self.take_live_map(Transition::Cut);
        if let Some(alert) = target {
            self.zoom_to_alert(&alert.id);
            self.select_alert(Some(alert.id.clone()));
            if alert.motion().is_some() && mode != SevereMode::FlashFloodEmergency {
                self.track_alert(&alert.id);
            }
        } else {
            self.fly_to(GeoView { zoom: 7.8, ..home }, 1.4);
        }
        self.log(2, format!("{} on", mode.label()));
        self.toast(ToastKind::Error, format!("{} - map, crawl and call-out are set", mode.label()));
        let scene = self.settings.obs_severe_scene.clone();
        self.obs_switch(scene, cx);
    }

    /// Switch OBS's program scene (when enabled in Settings).
    pub fn obs_switch(&mut self, scene: String, cx: &mut Context<Self>) {
        // Hardware switchers (ATEM etc. through Companion) follow Severe Mode too.
        if scene == self.settings.obs_severe_scene && !self.settings.companion_severe.trim().is_empty() {
            let loc = self.settings.companion_severe.clone();
            self.switcher_press(&loc, cx);
        }
        if !self.settings.obs_enabled || scene.trim().is_empty() {
            return;
        }
        let url = self.settings.obs_url.clone();
        let password = self.settings.obs_password.clone();
        let studio = self.settings.obs_studio_mode;
        cx.spawn(async move |this, cx| {
            let result = net::spawn(async move {
                if studio {
                    crate::switcher::obs_preview(url.clone(), password.clone(), scene).await?;
                    crate::switcher::obs_transition(url, password).await
                } else {
                    crate::obs::switch_scene(url, password, scene).await
                }
            }).await;
            let _ = this.update(cx, |s, cx| {
                match result {
                    Ok(Ok(msg)) => s.log(0, msg),
                    Ok(Err(e)) => {
                        s.toast(ToastKind::Error, e.to_string());
                        s.log(1, format!("OBS: {e}"));
                    }
                    Err(e) => s.toast(ToastKind::Error, e.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn apply_extras(
        &mut self,
        reports: anyhow::Result<Vec<crate::data::StormReport>>,
        roads: anyhow::Result<Vec<crate::data::RoadEvent>>,
        mesoscale: anyhow::Result<Vec<crate::data::MesoDiscussion>>,
    ) {
        let now = chrono::Local::now();
        let mut data = (*self.data).clone();
        match reports {
            Ok(r) => {
                self.health.reports = FeedStatus { ok: true, message: format!("{} reports (24 h)", r.len()), at: Some(now) };
                data.reports = r;
            }
            Err(e) => self.health.reports = FeedStatus { ok: false, message: e.to_string(), at: Some(now) },
        }
        match roads {
            Ok(r) => {
                self.health.roads = FeedStatus {
                    ok: true,
                    message: format!("{} INDOT events", r.len()),
                    at: Some(now),
                };
                data.roads = r;
            }
            Err(e) => self.health.roads = FeedStatus { ok: false, message: e.to_string(), at: Some(now) },
        }
        match mesoscale {
            Ok(m) => {
                self.health.mesoscale = FeedStatus {
                    ok: true,
                    message: format!("{} active mesoscale discussion(s)", m.len()),
                    at: Some(now),
                };
                data.mesoscale = m;
            }
            Err(e) => self.health.mesoscale = FeedStatus { ok: false, message: e.to_string(), at: Some(now) },
        }
        self.data = Arc::new(data);
    }

    /// Lightning near home right now (closest strike, all-clear countdown).
    pub fn lightning_near(&self) -> Option<crate::data::LightningNear> {
        let home = self.settings.home;
        crate::data::lightning_near(&self.data.lightning, home.lat, home.lon)
    }

    /// Notify once when lightning first comes within the safety radius, and once at the all clear.
    fn check_lightning_near(&mut self) {
        let near = self.lightning_near().filter(|n| n.all_clear_in > 0);
        match (self.lightning_was_near, near) {
            (false, Some(n)) => {
                self.lightning_was_near = true;
                log::warn!("lightning near home: {}", n.summary());
                self.toast(ToastKind::Error, format!("{} of {}", n.summary(), self.settings.home_label));
            }
            (true, None) => {
                self.lightning_was_near = false;
                log::info!("lightning all clear near home");
                self.toast(
                    ToastKind::Success,
                    format!(
                        "Lightning all clear: no strikes within {:.0} mi for {} min",
                        crate::data::LIGHTNING_RADIUS_MI,
                        crate::data::LIGHTNING_ALL_CLEAR_MIN
                    ),
                );
            }
            _ => {}
        }
    }

    /// Minutes since the newest radar frame (stale-data warning).
    pub fn radar_age_minutes(&self) -> Option<i64> {
        self.data
            .radar
            .as_ref()
            .and_then(|r| r.latest_past())
            .map(|f| (chrono::Utc::now().timestamp() - f.time) / 60)
    }
}
