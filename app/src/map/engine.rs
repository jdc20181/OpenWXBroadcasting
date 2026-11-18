//! The map engine: maplibre-rs running off-screen on its own thread.
//!
//! The UI sends camera and style changes; the engine renders at broadcast resolution and
//! publishes each frame together with the camera it was rendered with, so the compositor can
//! draw geo overlays that line up with that exact frame.

use std::{
    sync::{
        mpsc::{self, Receiver, Sender, TryRecvError},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use maplibre::{
    headless::offscreen::{CameraOptions, OffscreenMap},
    style::Style,
};

use crate::config::{tile_cache_dir, GeoView, FRAME_H, FRAME_W};

pub enum MapCommand {
    Camera(GeoView),
    Style(serde_json::Value),
    /// Stop rendering (nothing on screen needs the map) or resume.
    Pause(bool),
    /// Throw away the renderer and every tile it holds, and start again from the current style
    /// and camera (Refresh radar).
    Reload,
}

/// A rendered map frame (RGBA, opaque) and the camera it shows.
#[derive(Clone)]
pub struct MapFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
    pub view: GeoView,
    pub serial: u64,
}

#[derive(Clone, Debug, Default)]
pub struct EngineStatus {
    pub ready: bool,
    pub error: Option<String>,
    pub fps: f32,
}

#[derive(Clone)]
pub struct MapEngine {
    tx: Sender<MapCommand>,
    frame: Arc<Mutex<Option<MapFrame>>>,
    status: Arc<Mutex<EngineStatus>>,
    size: (u32, u32),
}

impl MapEngine {
    /// The main broadcast-resolution map.
    pub fn start(style: serde_json::Value, view: GeoView) -> Self {
        Self::start_sized(style, view, FRAME_W, FRAME_H)
    }

    /// A map rendered at `width` x `height` (comparison panels).
    pub fn start_sized(style: serde_json::Value, view: GeoView, width: u32, height: u32) -> Self {
        maplibre::raster::set_tile_filter(crate::map::winter_radar::filter);
        let (tx, rx) = mpsc::channel();
        let frame = Arc::new(Mutex::new(None));
        let status = Arc::new(Mutex::new(EngineStatus::default()));

        let thread_frame = frame.clone();
        let thread_status = status.clone();
        std::thread::Builder::new()
            .name("maplibre-engine".into())
            .spawn(move || {
                let status = thread_status.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(style, view, width, height, rx, thread_frame, thread_status)
                }));
                if result.is_err() {
                    // Surface the crash in the UI (badge + status bar) instead of a silent 0 fps.
                    set_status(&status, |s| {
                        s.ready = false;
                        s.error = Some("map renderer stopped - see log".into());
                    });
                }
            })
            .expect("spawn map engine thread");

        Self {
            tx,
            frame,
            status,
            size: (width, height),
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn set_camera(&self, view: GeoView) {
        let _ = self.tx.send(MapCommand::Camera(view));
    }

    pub fn set_style(&self, style: serde_json::Value) {
        let _ = self.tx.send(MapCommand::Style(style));
    }

    /// Pause rendering while no view, output or recording needs the map (saves CPU/GPU).
    pub fn set_paused(&self, paused: bool) {
        let _ = self.tx.send(MapCommand::Pause(paused));
    }

    /// Rebuild the renderer from scratch: every tile is requested again.
    pub fn reload(&self) {
        let _ = self.tx.send(MapCommand::Reload);
    }

    /// The most recent frame (cheap: the pixel buffer is shared).
    pub fn latest(&self) -> Option<MapFrame> {
        self.frame.lock().ok()?.clone()
    }

    pub fn status(&self) -> EngineStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

fn set_status(status: &Arc<Mutex<EngineStatus>>, update: impl FnOnce(&mut EngineStatus)) {
    if let Ok(mut s) = status.lock() {
        update(&mut s);
    }
}

fn camera(view: GeoView) -> CameraOptions {
    CameraOptions {
        latitude: view.lat,
        longitude: view.lon,
        zoom: view.zoom,
    }
}

fn run(
    style: serde_json::Value,
    view: GeoView,
    width: u32,
    height: u32,
    rx: Receiver<MapCommand>,
    frame_slot: Arc<Mutex<Option<MapFrame>>>,
    status: Arc<Mutex<EngineStatus>>,
) {
    // maplibre-rs schedules tile requests on Tokio; the map itself (Rc-based) stays on this thread.
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .thread_name("maplibre-io")
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            set_status(&status, |s| s.error = Some(format!("map runtime: {e}")));
            return;
        }
    };

    runtime.block_on(async move {
        // Kept so Reload can rebuild the renderer with the style currently shown.
        let mut style_json = style.clone();
        let style: Style = match serde_json::from_value(style) {
            Ok(style) => style,
            Err(e) => {
                set_status(&status, |s| s.error = Some(format!("map style: {e}")));
                return;
            }
        };

        let cache = tile_cache_dir();
        let _ = std::fs::create_dir_all(&cache);
        let mut map = match OffscreenMap::new(
            style,
            width,
            height,
            Some(cache.to_string_lossy().into_owned()),
        )
        .await
        {
            Ok(map) => map,
            Err(e) => {
                set_status(&status, |s| s.error = Some(format!("map renderer: {e}")));
                log::error!("map renderer failed: {e:?}");
                return;
            }
        };

        let mut sink = map.frame_sink();
        let mut current_view = view;
        map.jump_to(camera(current_view));
        set_status(&status, |s| {
            s.ready = true;
            s.error = None;
        });

        let mut last_activity = Instant::now();
        let mut frames = 0u32;
        let mut fps_window = Instant::now();
        let mut paused = false;

        loop {
            // Apply everything queued since the last frame; only the newest camera matters.
            let mut changed = false;
            loop {
                match rx.try_recv() {
                    Ok(MapCommand::Camera(view)) => {
                        current_view = view;
                        changed = true;
                    }
                    Ok(MapCommand::Style(value)) => match serde_json::from_value::<Style>(value.clone()) {
                        Ok(style) => {
                            style_json = value;
                            map.set_style(style);
                            changed = true;
                        }
                        Err(e) => log::warn!("rejected map style: {e}"),
                    },
                    Ok(MapCommand::Reload) => {
                        let rebuilt = match serde_json::from_value::<Style>(style_json.clone()) {
                            Ok(style) => {
                                OffscreenMap::new(style, width, height, Some(cache.to_string_lossy().into_owned()))
                                    .await
                                    .map_err(|e| format!("{e:?}"))
                            }
                            Err(e) => Err(e.to_string()),
                        };
                        match rebuilt {
                            Ok(fresh) => {
                                map = fresh;
                                sink = map.frame_sink();
                                log::info!("map reloaded: renderer rebuilt, all tiles requested again");
                                changed = true;
                            }
                            Err(e) => log::error!("map reload failed, keeping the old renderer: {e}"),
                        }
                    }
                    Ok(MapCommand::Pause(p)) => {
                        if paused && !p {
                            changed = true;
                        }
                        paused = p;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            if changed {
                map.jump_to(camera(current_view));
                last_activity = Instant::now();
            }
            if paused {
                set_status(&status, |s| s.fps = 0.0);
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue;
            }

            if let Err(e) = map.render() {
                log::warn!("map render: {e:?}");
            }

            if let Some(frame) = sink.take() {
                let mut pixels = frame.pixels;
                if frame.bgra {
                    for px in pixels.chunks_exact_mut(4) {
                        px.swap(0, 2);
                    }
                }
                for px in pixels.chunks_exact_mut(4) {
                    px[3] = 255;
                }
                if let Ok(mut slot) = frame_slot.lock() {
                    // An identical picture keeps its frame (and serial) so the compositor can
                    // skip redrawing a map that has not changed.
                    let same = slot.as_ref().is_some_and(|old| {
                        old.view == current_view && old.width == frame.width && old.height == frame.height && *old.rgba == pixels
                    });
                    if same {
                        frames += 1;
                        drop(slot);
                        let elapsed = fps_window.elapsed();
                        if elapsed >= Duration::from_secs(1) {
                            set_status(&status, |s| s.fps = frames as f32 / elapsed.as_secs_f32());
                            frames = 0;
                            fps_window = Instant::now();
                        }
                        tokio::time::sleep(if last_activity.elapsed() < Duration::from_secs(4) { Duration::from_millis(30) } else { Duration::from_millis(120) }).await;
                        continue;
                    }
                    *slot = Some(MapFrame {
                        width: frame.width,
                        height: frame.height,
                        rgba: Arc::new(pixels),
                        view: current_view,
                        serial: frame.serial,
                    });
                }
                frames += 1;
            }

            let elapsed = fps_window.elapsed();
            if elapsed >= Duration::from_secs(1) {
                let fps = frames as f32 / elapsed.as_secs_f32();
                set_status(&status, |s| s.fps = fps);
                frames = 0;
                fps_window = Instant::now();
            }

            // Full rate while the camera/style is changing or tiles are streaming in after a
            // change; a slower steady rate otherwise (tiles can still arrive later).
            let busy = last_activity.elapsed() < Duration::from_secs(4);
            tokio::time::sleep(if busy {
                Duration::from_millis(30)
            } else {
                Duration::from_millis(120)
            })
            .await;
        }
    });
}
