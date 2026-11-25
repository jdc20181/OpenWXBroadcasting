//! Broadcast operations on the app state (a child module of `app`, so it can use its private
//! fields): control-server commands, session restore, memory watch, output monitoring, sponsor
//! rotation and impressions, scheduled graphic exports, warning archive and diagnostics.

use std::time::{Duration, Instant};

use gpui_kit::Context;

use super::{AppState, OnAir, SevereMode, ToastKind};
use crate::{ops, show::Transition};

pub struct Ext {
    /// Forecast editor state (draft, published, scenarios, history).
    pub edits: crate::forecast_edit::ForecastEdits,
    /// Paint mode for forecast adjustments: (variable, amount, radius miles).
    pub paint: Option<(crate::forecast_edit::EditVar, f32, f64)>,
    /// The model grid before forecaster adjustments.
    pub raw_grid: Option<std::sync::Arc<crate::grid::WxGrid>>,
    ops_rx: Option<std::sync::mpsc::Receiver<ops::Command>>,
    midi_rx: Option<std::sync::mpsc::Receiver<ops::Command>>,
    /// OSC / MOS / scheduled commands.
    pub extra_rx: Option<std::sync::mpsc::Receiver<ops::Command>>,
    /// Sender into `extra_rx`, used by the timed command queue.
    pub loopback: Option<std::sync::mpsc::Sender<ops::Command>>,
    /// When the backup scene went on air (for the outage rehearsal).
    pub backup_since: Option<Instant>,
    pub midi_devices: Vec<String>,
    /// OBS buses (program, preview), polled every 5 s.
    pub buses: Option<(String, String)>,
    buses_at: Instant,
    buses_loading: bool,
    started: bool,
    session_saved: Instant,
    mem_checked: Instant,
    mem_warned: bool,
    pub memory_mb: Option<f64>,
    output_checked: Instant,
    black_warned: bool,
    late_seen: u64,
    last_late_warning: Option<Instant>,
    pub backup_on: bool,
    sponsor_switched: Instant,
    pub sponsor_index: usize,
    /// Seconds each sponsor has been on the Program output today, keyed by name.
    impressions: std::collections::HashMap<String, f64>,
    impressions_flushed: Instant,
    last_impression_tick: Instant,
    exported_at: Instant,
    hydro_at: Option<Instant>,
    hydro_loading: bool,
    climate_at: Option<(Instant, (f64, f64, bool))>,
    climate_loading: bool,
    models_warned_day: Option<chrono::NaiveDate>,
    /// Camera path playing: (stops, next stop index, when to start it).
    camera_play: Option<(Vec<crate::map::scene::CameraKey>, usize, Instant)>,
    last_show_pos: Option<usize>,
    follow_at: Instant,
    /// Scenario shown on air for a demonstration (None = the published forecast).
    pub demo_scenario: Option<String>,
    channels_synced: Instant,
    failover_at: Instant,
    cams_at: Option<Instant>,
    tour: Option<Tour>,
    /// Automated broadcast state: (segment index, next change, interrupted by warning id,
    /// last station ID, cycle count).
    pub autocast: Option<Autocast>,
    /// Background work per app tick, ms (smoothed): Forecaster desk > performance.
    pub tick_ms: f32,
    adaptive_at: Instant,
    sounding_at: Option<Instant>,
    hazards_at: Option<Instant>,
    quakes_at: Option<Instant>,
    audio_key: String,
    viewer_at: Instant,
    eoc_version: u64,
    workflow_at: Instant,
    verify_day: Option<chrono::NaiveDate>,
    outages_at: Option<Instant>,
    home_hourly_at: Option<(Instant, (f64, f64))>,
    /// Last coverage-towns forecast fetch and the places it was for.
    coverage_at: Option<(Instant, String)>,
    nowcast_at: Instant,
    rain_alerted: Option<Instant>,
    flood_checked: Instant,
    publish_at: Instant,
    public_at: Option<Instant>,
    history_at: Option<Instant>,
    history_retry: Duration,
    ensemble_at: Option<Instant>,
    /// 0 = full quality, 1-2 = reduced radar resolution.
    pub quality_level: u8,
    /// Consecutive 5 s checks over / well under the frame budget (hysteresis).
    pub quality_over: u8,
    pub quality_under: u8,
}

pub struct Autocast {
    index: usize,
    next_at: Instant,
    /// Warning on air for an interruption, and when to resume.
    interrupted: Option<(String, Instant)>,
    last_id: Instant,
    pub cycles: u32,
    seen_warnings: std::collections::HashSet<String>,
}

/// One stop of an automated tour.
#[derive(Clone)]
pub enum TourStop {
    Warning(String),
    Place(f64, f64),
}

pub struct Tour {
    stops: Vec<TourStop>,
    index: usize,
    next_at: Instant,
    hold: f32,
}

impl Ext {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            edits: crate::forecast_edit::load(),
            raw_grid: None,
            paint: None,
            ops_rx: None,
            midi_rx: None,
            extra_rx: None,
            loopback: None,
            backup_since: None,
            midi_devices: Vec::new(),
            buses: None,
            buses_at: now,
            buses_loading: false,
            started: false,
            session_saved: now,
            mem_checked: now,
            mem_warned: false,
            memory_mb: None,
            output_checked: now,
            black_warned: false,
            late_seen: 0,
            last_late_warning: None,
            backup_on: false,
            sponsor_switched: now,
            sponsor_index: 0,
            impressions: Default::default(),
            impressions_flushed: now,
            last_impression_tick: now,
            exported_at: now,
            hydro_at: None,
            hydro_loading: false,
            climate_at: None,
            climate_loading: false,
            models_warned_day: None,
            camera_play: None,
            last_show_pos: None,
            follow_at: now,
            demo_scenario: None,
            channels_synced: now,
            failover_at: now,
            cams_at: None,
            tour: None,
            autocast: None,
            tick_ms: 0.0,
            adaptive_at: now,
            sounding_at: None,
            hazards_at: None,
            quakes_at: None,
            audio_key: "unset".into(),
            viewer_at: now,
            eoc_version: 0,
            workflow_at: now,
            verify_day: None,
            outages_at: None,
            home_hourly_at: None,
            coverage_at: None,
            nowcast_at: now,
            rain_alerted: None,
            flood_checked: now,
            publish_at: now,
            public_at: None,
            history_at: None,
            history_retry: Duration::from_secs(6 * 3600),
            ensemble_at: None,
            quality_level: 0,
            quality_over: 0,
            quality_under: 0,
        }
    }
}

impl Default for Ext {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Called with the 30 fps tick.
    pub fn ext_tick(&mut self, cx: &mut Context<Self>) {
        let tick_started = Instant::now();
        self.ext_tick_inner(cx);
        let ms = tick_started.elapsed().as_secs_f32() * 1000.0;
        self.ext.tick_ms = self.ext.tick_ms * 0.95 + ms * 0.05;
    }

    /// Show the radar frame nearest a time (event bookmarks).
    pub fn jump_to_radar_time(&mut self, t: i64) {
        if let Some((i, _)) = self.radar_frames.iter().enumerate().min_by_key(|(_, f)| (**f - t).abs()) {
            self.playing = false;
            let delta = i as i32 - self.radar_index as i32;
            self.step_radar(delta);
        }
    }

    fn ext_tick_inner(&mut self, cx: &mut Context<Self>) {
        if !self.ext.started {
            self.ext.started = true;
            self.ext_start();
        }
        self.poll_ops(cx);

        if self.ext.session_saved.elapsed() >= Duration::from_secs(30) {
            self.ext.session_saved = Instant::now();
            self.save_session();
        }
        if self.ext.mem_checked.elapsed() >= Duration::from_secs(15) {
            self.ext.mem_checked = Instant::now();
            self.check_memory();
        }
        if self.ext.output_checked.elapsed() >= Duration::from_secs(2) {
            self.ext.output_checked = Instant::now();
            self.check_output();
        }
        self.maybe_fetch_hydro(cx);
        self.maybe_fetch_climate(cx);
        self.maybe_fetch_home_hourly(cx);
        self.maybe_fetch_coverage(cx);
        // Mobile app bridge: newest YouTube uploads for /public/shows.json.
        if crate::shows::due(&self.settings.youtube_channel_id) {
            let _ = crate::net::spawn(crate::shows::refresh(self.settings.youtube_channel_id.clone()));
        }
        self.export_record_tick();
        self.maybe_fetch_metars(cx);
        self.warning_end_check(cx);
        self.camera_tick();
        self.rotate_sponsor();
        self.count_impressions();
        self.run_scheduled_exports();
        self.rehearsal_tick();
        self.autocast_tick(cx);
        self.ops2_tick(cx);
        if self.ext.channels_synced.elapsed() >= Duration::from_secs(2) {
            self.ext.channels_synced = Instant::now();
            crate::channels::update_data(self.data.clone(), std::sync::Arc::new(self.settings.clone()));
            crate::gfx::model::set_registry(&self.graphics);
        }
        self.failover_tick();
        // Publishing: scheduled / conditional publishing, then work the queue off the UI thread.
        if self.ext.publish_at.elapsed() >= Duration::from_secs(5) {
            self.ext.publish_at = Instant::now();
            if !self.settings.publish_schedules.is_empty() {
                let warn = !self.coverage_warnings().is_empty();
                self.settings.publish_schedules = crate::publish::run_schedules(&self.settings, &self.data, &self.graphics, warn);
            }
            if crate::publish::pending() > 0 {
                let (s, d, g) = (std::sync::Arc::new(self.settings.clone()), self.data.clone(), std::sync::Arc::new(self.graphics.clone()));
                crate::net::spawn(crate::publish::work(s, d, g));
            }
        }
        // Public endpoints: share the newest data, graphics and radar picture with the portal.
        if self.settings.portal_enabled && self.settings.public_endpoints && self.ext.public_at.map(|t| t.elapsed() >= Duration::from_secs(30)).unwrap_or(true) {
            self.ext.public_at = Some(Instant::now());
            let radar_jpg = self.compositor.onair_map_frame().and_then(|f| crate::ops::encode_frame(&f, true, Some(1280))).unwrap_or_default();
            let version = self.data.updated_at.map(|t| t.timestamp() as u64).unwrap_or(0);
            if let Ok(mut p) = crate::publish::public().write() {
                *p = Some(crate::publish::Public { data: self.data.clone(), settings: std::sync::Arc::new(self.settings.clone()), graphics: std::sync::Arc::new(self.graphics.clone()), radar_jpg, version });
            }
        }
        self.adaptive_tick();
        self.maybe_fetch_hazards(cx);
        self.maybe_fetch_quakes(cx);
        if self.ext.flood_checked.elapsed() >= Duration::from_secs(300) {
            self.ext.flood_checked = Instant::now();
            self.flood_watch();
        }
        // Forecast verification: record today's forecasts and fetch observations once a day
        // (after the forecast has loaded); weekly report on Mondays.
        let today = chrono::Local::now().date_naive();
        if self.ext.verify_day != Some(today) && self.data.overview.is_some() && self.ext.viewer_at.elapsed() >= Duration::from_secs(5) {
            self.ext.verify_day = Some(today);
            let places = crate::verify::places(&self.settings, &self.data.forecast_edits);
            let station = places.first().map(|(name, _, _)| crate::verify::station_records(&self.data, name)).unwrap_or_default();
            let station_name = self.settings.station_name.clone();
            crate::net::spawn(async move {
                match crate::verify::daily_update(places, station).await {
                    Ok((f, o)) => log::info!("verification: {f} forecasts recorded, {o} observations"),
                    Err(e) => log::warn!("verification: {e}"),
                }
                if crate::verify::weekly_due() {
                    if let Err(e) = crate::verify::export_report(30, &station_name) {
                        log::warn!("verification report: {e}");
                    }
                }
            });
        }
        // Storm nowcast every 30 s while storms are tracked (or to clear it when they're gone).
        if self.ext.nowcast_at.elapsed() >= Duration::from_secs(30) && (!self.data.cells.is_empty() || !self.data.nowcast.cells.is_empty()) {
            self.ext.nowcast_at = Instant::now();
            let n = crate::nowcast::compute(&self.data, (self.settings.home.lat, self.settings.home.lon));
            if n != self.data.nowcast {
                let mut data = (*self.data).clone();
                data.nowcast = n;
                self.data = std::sync::Arc::new(data);
                let snapshot = self.data.clone();
                self.compositor.update(|p| p.data = snapshot);
            }
        }
        // Power outages every 10 minutes while utilities are configured.
        if !self.settings.outage_sources.is_empty() && self.ext.outages_at.map(|t| t.elapsed() >= Duration::from_secs(600)).unwrap_or(true) {
            self.ext.outages_at = Some(Instant::now());
            crate::outages::set_state_filter(&self.settings.outage_state);
            let sources = self.settings.outage_sources.clone();
            let prev = self.data.outages.facilities.clone();
            let home = self.settings.home;
            let d = self.settings.coverage_miles.max(40.0) / 69.0;
            let bbox = (home.lat - d, home.lon - d * 1.3, home.lat + d, home.lon + d * 1.3);
            cx.spawn(async move |this, cx| {
                let o = crate::net::spawn(crate::outages::fetch(sources, prev, bbox)).await;
                let _ = this.update(cx, |s, cx| {
                    let Ok(o) = o else { return };
                    for p in &o.problems {
                        log::warn!("outages: {p}");
                    }
                    let mut data = (*s.data).clone();
                    data.outages = o;
                    s.data = std::sync::Arc::new(data);
                    let snapshot = s.data.clone();
                    s.compositor.update(|p| p.data = snapshot);
                    cx.notify();
                });
            })
            .detach();
        }
        // Workflow automation: triggers, branches and actions (carried out like control commands).
        if self.ext.workflow_at.elapsed() >= Duration::from_secs(2) {
            self.ext.workflow_at = Instant::now();
            if crate::workflow::busy() {
                let b = crate::gfx::bind::Bindings::build(&self.data, &self.settings, None, &self.settings.radar_palette);
                let ctx = crate::workflow::Ctx {
                    values: b.values().clone(),
                    warnings: self.coverage_warnings().into_iter().map(|a| (a.id, a.event)).collect(),
                    show_running: self.show_running,
                    severe: !matches!(self.severe_mode, crate::app::SevereMode::Off),
                    graphic_on_air: matches!(self.on_air, crate::app::OnAir::Graphic(_)),
                };
                for cmd in crate::workflow::tick(&ctx, &self.graphics, &self.settings) {
                    self.handle_command(cmd, cx);
                }
            }
        }
        // Emergency operations: timeline entries for new warnings / closures, and the map copy.
        if self.ext.viewer_at.elapsed() >= Duration::from_secs(10) {
            crate::eoc::watch(&self.coverage_warnings(), &self.data.roads);
            let v = crate::eoc::version();
            if v != self.ext.eoc_version {
                self.ext.eoc_version = v;
                let mut data = (*self.data).clone();
                data.eoc = crate::eoc::get();
                self.data = std::sync::Arc::new(data);
                let snapshot = self.data.clone();
                self.compositor.update(|p| p.data = snapshot);
            }
        }
        if self.ext.viewer_at.elapsed() >= Duration::from_secs(10) {
            self.ext.viewer_at = Instant::now();
            let list = crate::portal::list();
            if list != self.data.viewer {
                let mut data = (*self.data).clone();
                data.viewer = list;
                self.data = std::sync::Arc::new(data);
                let snapshot = self.data.clone();
                self.compositor.update(|p| p.data = snapshot);
            }
        }
        let audio_key = format!("{}|{}|{}|{}|{}", self.settings.audio_program_device, self.settings.audio_alert_device, self.settings.audio_input_device, self.settings.audio_input_on, self.settings.duck_threshold_db);
        if audio_key != self.ext.audio_key {
            self.ext.audio_key = audio_key;
            crate::audio::set_devices(&self.settings.audio_program_device, &self.settings.audio_alert_device);
            let duck = (self.settings.duck_threshold_db < 0.0).then_some(self.settings.duck_threshold_db);
            crate::audio::start_input(self.settings.audio_input_on.then_some(self.settings.audio_input_device.as_str()), duck);
        }
        if self.ext.ensemble_at.map(|t| t.elapsed() >= Duration::from_secs(3 * 3600)).unwrap_or(true) {
            self.ext.ensemble_at = Some(Instant::now());
            let home = self.settings.home;
            let d = self.settings.coverage_miles.max(40.0) / 69.0;
            let mut towns = crate::places::in_box(home.lat - d, home.lat + d, home.lon - d * 1.3, home.lon + d * 1.3);
            towns.sort_by(|a, b| b.population.cmp(&a.population));
            let mut points = vec![(if self.settings.home_label.is_empty() { "Home".to_string() } else { self.settings.home_label.clone() }, home.lat, home.lon)];
            points.extend(towns.into_iter().take(7).map(|t| (t.name, t.lat, t.lon)));
            crate::net::spawn(async move {
                match crate::ensemble::fetch(points).await {
                    Ok(e) => crate::ensemble::set_latest(e),
                    Err(e) => log::warn!("ensemble: {e}"),
                }
            });
        }
        if self.ext.sounding_at.map(|t| t.elapsed() >= Duration::from_secs(1800)).unwrap_or(true) {
            self.ext.sounding_at = Some(Instant::now());
            let home = self.settings.home;
            crate::net::spawn(async move {
                match crate::sounding::fetch(home.lat, home.lon).await {
                    Ok(s) => crate::sounding::set_latest(s),
                    Err(e) => log::warn!("sounding: {e}"),
                }
            });
        }
        if self.ext.cams_at.map(|t| t.elapsed() >= Duration::from_secs(60)).unwrap_or(true) {
            self.ext.cams_at = Some(Instant::now());
            crate::travel::refresh_cameras(self.settings.webcams.clone());
        }
    }

    /// Adaptive quality: every 5 s, compare frame time and radar server speed with the budget.
    fn adaptive_tick(&mut self) {
        if self.ext.adaptive_at.elapsed() < Duration::from_secs(5) {
            return;
        }
        self.ext.adaptive_at = Instant::now();
        let (zoom, frames) = if self.settings.adaptive_quality {
            let stats = self.compositor.stats();
            let budget = if self.settings.low_resource { 66.0 } else { 33.0 };
            // Zooming and panning cost a few slow frames on their own; changing the radar
            // resolution then swaps every radar layer (a visible jump). So only react to load
            // that lasts (3 checks = 15 s over, 6 checks = 30 s well under), and never while the
            // camera is moving.
            let moving = self.camera_moving();
            let ext = &mut self.ext;
            if moving {
                ext.quality_over = 0;
                ext.quality_under = 0;
            } else if stats.fps > 0.0 && stats.render_ms > budget * 0.9 {
                ext.quality_under = 0;
                ext.quality_over = ext.quality_over.saturating_add(1);
                if ext.quality_over >= 3 {
                    ext.quality_over = 0;
                    ext.quality_level = (ext.quality_level + 1).min(2);
                }
            } else if stats.render_ms < budget * 0.55 && ext.quality_level > 0 {
                ext.quality_over = 0;
                ext.quality_under = ext.quality_under.saturating_add(1);
                if ext.quality_under >= 6 {
                    ext.quality_under = 0;
                    ext.quality_level -= 1;
                }
            } else {
                ext.quality_over = 0;
                ext.quality_under = 0;
            }
            let level = &mut self.ext.quality_level;
            let slow = crate::broadcast_ops::source_stats()
                .iter()
                .find(|(h, _)| h.contains("dcweather"))
                .map(|(_, s)| s.avg_ms() > 2500 || (s.requests >= 10 && s.failures * 4 > s.requests))
                .unwrap_or(false);
            let zoom = match *level {
                0 => None,
                1 => Some(10),
                _ => Some(8),
            };
            (zoom, slow.then_some(6))
        } else {
            (None, None)
        };
        if self.live.radar_maxzoom_cap != zoom || self.live.frame_cap != frames {
            log::info!("adaptive quality: radar zoom cap {zoom:?}, loop frames cap {frames:?}");
            self.update_live(|l| {
                l.radar_maxzoom_cap = zoom;
                l.frame_cap = frames;
            });
        }
    }

    /// With "require approval" on, unapproved graphics never reach air.
    pub fn approval_blocked(&mut self, doc: &crate::gfx::model::GraphicDoc) -> bool {
        if self.settings.require_approval && doc.approved_by.trim().is_empty() {
            self.toast(ToastKind::Error, format!("\"{}\" is not approved for air yet (Graphics Studio > Approve)", doc.name));
            log::warn!("blocked unapproved graphic {}", doc.name);
            true
        } else {
            false
        }
    }

    /// Automation failover during a show: a map scene with stale radar is skipped, and a Program
    /// that goes black on its own falls back to the backup scene.
    fn failover_tick(&mut self) {
        if !self.settings.failover_enabled || !self.show_running || self.ext.failover_at.elapsed() < Duration::from_secs(3) {
            return;
        }
        self.ext.failover_at = Instant::now();
        let stale = self.radar_age_minutes().map(|m| m > 20).unwrap_or(true);
        if stale && matches!(self.on_air, OnAir::LiveMap) && self.show.scenes.len() > 1 && !self.ext.backup_on {
            crate::broadcast_ops::log_incident("Failover", "Radar is stale during the show: skipped to the next scene");
            self.toast(ToastKind::Error, "Radar is stale: the show skipped ahead (automation failover)");
            self.next_scene();
        }
        let stats = self.compositor.stats();
        if stats.black_for >= 5.0 && !self.black && !self.ext.backup_on {
            crate::broadcast_ops::log_incident("Failover", "Program went black on its own: backup scene put on air");
            self.set_backup_scene(true);
        }
    }

    fn ext_start(&mut self) {
        crate::gfx::look::set_default(&self.settings.graphics_look);
        // Restore the last live map; say so only after an unexpected close.
        if let Some(session) = ops::load_session() {
            if let Some(live) = session.live.clone() {
                let view = live.view;
                self.live = live;
                self.set_view(view);
                self.sync_map();
            }
            if let Some(id) = &session.show_id {
                if let Some(show) = self.shows.iter().find(|s| &s.id == id).cloned() {
                    self.show = show;
                    self.relink_show_graphics();
                    self.show_pos = session.show_pos.filter(|p| *p < self.show.scenes.len());
                }
            }
            if !session.clean_exit && session.saved > 0 {
                let when = chrono::DateTime::from_timestamp(session.saved, 0)
                    .map(|t| t.with_timezone(&chrono::Local).format("%-I:%M %p").to_string())
                    .unwrap_or_default();
                log::warn!("previous session did not close cleanly; restored the map and show from {when}");
                self.toast(ToastKind::Info, format!("OpenWX closed unexpectedly. Your map and show from {when} are restored"));
            }
        }
        self.save_session();

        if self.settings.control_server {
            let port = if self.settings.control_port == 0 { ops::DEFAULT_PORT } else { self.settings.control_port };
            self.ext.ops_rx = Some(ops::start_server(port, self.compositor.clone()));
        }

        if self.settings.midi_enabled {
            let (rx, names) = crate::switcher::start_midi();
            self.ext.midi_rx = Some(rx);
            self.ext.midi_devices = names;
        }

        if self.settings.portal_enabled {
            crate::portal::start(self.settings.portal_port, self.settings.station_name.clone());
        }

        // Generated sky backgrounds for {{sky_image}} (written once).
        std::thread::spawn(crate::gfx::backgrounds::ensure);

        // Keep the tile cache in bounds (off the UI thread).
        let cap = self.settings.tile_cache_mb.max(200);
        crate::jobs::submit(crate::jobs::Priority::Low, "trim tile cache", move || {
            ops::trim_tile_cache(cap);
        });
    }

    pub fn save_session(&self) {
        let session = ops::Session {
            live: Some(self.live.clone()),
            show_id: Some(self.show.id.clone()),
            show_pos: self.show_pos,
            severe_mode: self.severe_mode.label().to_string(),
            saved: chrono::Utc::now().timestamp(),
            clean_exit: false,
        };
        std::thread::spawn(move || ops::save_session(&session));
    }

    // ---- Control server --------------------------------------------------------------------

    fn find_graphic(&self, key: &str) -> Option<crate::gfx::model::GraphicDoc> {
        let k = key.trim().to_lowercase();
        self.graphics
            .iter()
            .find(|g| g.id == key || g.name.to_lowercase() == k)
            .or_else(|| self.graphics.iter().find(|g| g.name.to_lowercase().contains(&k)))
            .cloned()
    }

    fn poll_ops(&mut self, cx: &mut Context<Self>) {
        let mut commands = Vec::new();
        for rx in [&self.ext.ops_rx, &self.ext.midi_rx, &self.ext.extra_rx].into_iter().flatten() {
            while let Ok(cmd) = rx.try_recv() {
                commands.push(cmd);
            }
        }
        self.poll_buses(cx);
        for cmd in commands {
            self.handle_command(cmd, cx);
            cx.notify();
        }
    }

    fn handle_command(&mut self, cmd: ops::Command, cx: &mut Context<Self>) {
        use ops::Command::*;
        match cmd {
            TakeScene(i) => {
                log::info!("control: take scene {}", i + 1);
                self.go_to_scene(i);
            }
            Next => self.next_scene(),
            Prev => self.prev_scene(),
            Live => self.take_live_map(Transition::Fade),
            Black(on) => self.set_black(on),
            Backup(on) => self.set_backup_scene(on),
            Refresh => self.refresh_radar(cx),
            Caption(text) => self.set_caption(&text),
            Interact(action) => self.graphic_action(&action, "remote"),
            PlayAudio(name) => {
                let path = crate::config::LibraryPaths::get().media.join(name.trim());
                crate::audio::play(path, crate::audio::Bus::Cue, 1.0, self.settings.audio_offset_ms as u64);
            }
            Cue(name) => {
                // Seek the on-air graphic so the marker is "now".
                let found = self.compositor.read(|p| match &p.source {
                    crate::program::ProgramSource::Graphic(doc) => doc.markers.iter().find(|(_, n)| n.eq_ignore_ascii_case(&name)).map(|(t, _)| *t),
                    _ => None,
                }).flatten();
                match found {
                    Some(t) => self.compositor.update(|p| p.source_started = std::time::Instant::now() - std::time::Duration::from_secs_f32(t.max(0.0))),
                    None => log::warn!("control: no cue point \"{name}\" on the graphic on air"),
                }
            }
            ChannelTake(n, name) => match (self.find_graphic(&name), crate::channels::get(n)) {
                (Some(doc), Some(ch)) => {
                    if !self.approval_blocked(&doc) {
                        log::info!("control: channel {n} takes {}", doc.name);
                        ch.take(doc);
                    }
                }
                _ => log::warn!("control: channel {n} or graphic {name} not found"),
            },
            ChannelClear(n) => {
                if let Some(ch) = crate::channels::get(n) {
                    ch.clear();
                }
            }
            TakeSynced(n, name) => {
                if let (Some(doc), Some(ch)) = (self.find_graphic(&name), crate::channels::get(n)) {
                    if !self.approval_blocked(&doc) {
                        // Program and the channel start on the same instant: animations match.
                        self.take_graphic(doc.clone(), Transition::Cut);
                        let started = std::time::Instant::now();
                        self.compositor.update(|p| p.source_started = started);
                        ch.take_at(doc, started);
                    }
                }
            }
            ListGraphics(reply) => {
                let list: Vec<serde_json::Value> = self
                    .graphics
                    .iter()
                    .map(|g| serde_json::json!({ "name": g.name, "category": g.category, "approved": !g.approved_by.is_empty() || !self.settings.require_approval }))
                    .collect();
                let _ = reply.send(serde_json::Value::Array(list).to_string());
            }
            Rundown(reply) => {
                let scenes: Vec<serde_json::Value> = self
                    .show
                    .scenes
                    .iter()
                    .map(|s| serde_json::json!({ "title": s.title, "kind": s.kind_label(), "duration": s.duration }))
                    .collect();
                let _ = reply.send(serde_json::json!({ "show": self.show.name, "on_air": self.show_pos, "scenes": scenes }).to_string());
            }
            Approve(name) => {
                if let Some(mut doc) = self.find_graphic(&name) {
                    doc.approved_by = format!("{} (producer page)", self.settings.operator_name);
                    crate::broadcast_ops::audit(&format!("APPROVED graphic {}", doc.name), &self.settings.operator_name);
                    let _ = self.save_graphic(doc);
                }
            }
            Acknowledge(id) => {
                let id = if id.is_empty() {
                    self.coverage_warnings().into_iter().find(|a| self.is_acknowledged(&a.id).is_none()).map(|a| a.id).unwrap_or_default()
                } else {
                    id
                };
                if !id.is_empty() {
                    self.acknowledge_alert(&id);
                }
            }
            Graphic(name) => match self.find_graphic(&name).filter(|d| !self.approval_blocked(d)) {
                Some(doc) => {
                    log::info!("control: take graphic {}", doc.name);
                    self.take_graphic(doc, Transition::Fade);
                }
                None => log::warn!("control: no graphic named {name}"),
            },
            Severe(mode) => {
                let m = match mode.to_lowercase().as_str() {
                    "off" | "normal" => SevereMode::Off,
                    "tornado" | "tornado_emergency" => SevereMode::TornadoEmergency,
                    "flood" | "flash_flood" | "ffe" => SevereMode::FlashFloodEmergency,
                    _ => SevereMode::Severe,
                };
                self.set_severe_mode(m, cx);
            }
            RenderGraphic(name, reply) => {
                let png = self.find_graphic(&name).filter(|doc| match crate::sources::blocked_source(&self.settings, doc) {
                    Some(src) => {
                        log::warn!("control server: \"{}\" uses {src}, which is not cleared for redistribution", doc.name);
                        false
                    }
                    None => true,
                }).and_then(|doc| {
                    let pm = crate::program::render_graphic(
                        &doc,
                        &self.settings,
                        &self.data,
                        30.0,
                        crate::config::FRAME_W,
                        crate::config::FRAME_H,
                        None,
                    );
                    ops::pixmap_png(&pm)
                });
                let _ = reply.send(png);
            }
            Status(reply) => {
                let stats = self.compositor.stats();
                let on_air = match &self.on_air {
                    OnAir::LiveMap => "Live map".to_string(),
                    OnAir::Graphic(name) => name.clone(),
                };
                let body = serde_json::json!({
                    "on_air": on_air,
                    "black": self.black,
                    "backup_scene": self.ext.backup_on,
                    "show": self.show.name,
                    "scene": self.show_pos.map(|p| p + 1),
                    "scenes": self.show.scenes.len(),
                    "severe_mode": self.severe_mode.label(),
                    "local_alerts": self.data.local_alerts.len(),
                    "radar_age_minutes": self.radar_age_minutes(),
                    "recording": self.recording.is_some(),
                    "fps": stats.fps,
                    "render_ms": stats.render_ms,
                    "memory_mb": self.ext.memory_mb,
                });
                let _ = reply.send(body.to_string());
            }
        }
    }

    // ---- Switchers ------------------------------------------------------------------------

    fn poll_buses(&mut self, cx: &mut Context<Self>) {
        if !self.settings.obs_enabled || self.ext.buses_loading || self.ext.buses_at.elapsed() < Duration::from_secs(5) {
            return;
        }
        self.ext.buses_loading = true;
        self.ext.buses_at = Instant::now();
        let (url, password) = (self.settings.obs_url.clone(), self.settings.obs_password.clone());
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(crate::switcher::obs_buses(url, password)).await;
            let _ = this.update(cx, |state, cx| {
                state.ext.buses_loading = false;
                state.ext.buses = match result {
                    Ok(Ok(b)) => Some(b),
                    _ => None,
                };
                cx.notify();
            });
        })
        .detach();
    }

    /// OBS studio mode: transition preview to program.
    pub fn obs_transition(&mut self, cx: &mut Context<Self>) {
        let (url, pw) = (self.settings.obs_url.clone(), self.settings.obs_password.clone());
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(crate::switcher::obs_transition(url, pw)).await;
            let _ = this.update(cx, |s, cx| {
                if let Ok(Err(e)) | Err(e) = result.map_err(|e| anyhow::anyhow!("{e}")) {
                    s.toast(ToastKind::Error, format!("OBS: {e}"));
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn obs_save_replay(&mut self, cx: &mut Context<Self>) {
        let (url, password) = (self.settings.obs_url.clone(), self.settings.obs_password.clone());
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(crate::switcher::obs_save_replay(url, password)).await;
            let _ = this.update(cx, |state, cx| {
                match result {
                    Ok(Ok(())) => state.toast(ToastKind::Info, "OBS is saving the replay to its recording folder"),
                    Ok(Err(error)) => state.toast(ToastKind::Error, format!("OBS replay: {error}")),
                    Err(error) => state.toast(ToastKind::Error, format!("OBS replay: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ---- Emergency backup scene ------------------------------------------------------------

    /// Put the "please stay with us" backup card on air, or return to the live map.
    pub fn set_backup_scene(&mut self, on: bool) {
        self.ext.backup_on = on;
        self.ext.backup_since = on.then(Instant::now);
        if on {
            crate::broadcast_ops::log_incident("Backup scene", "Backup scene put on air");
        }
        if on {
            if let Some(doc) = crate::gfx::templates::build("backup") {
                log::warn!("emergency backup scene on air");
                self.take_graphic(doc, Transition::Cut);
            }
        } else {
            log::info!("emergency backup scene off");
            self.take_live_map(Transition::Fade);
        }
    }

    // ---- Monitoring ------------------------------------------------------------------------

    fn check_memory(&mut self) {
        let Some(mb) = ops::memory_mb() else { return };
        self.ext.memory_mb = Some(mb);
        let limit = self.settings.memory_limit_mb.max(500) as f64;
        if mb > limit && !self.ext.mem_warned {
            self.ext.mem_warned = true;
            log::warn!("memory pressure: {mb:.0} MB in use (limit {limit:.0} MB); freeing map tiles");
            // Rebuilding the renderer frees every tile texture and decoded tile.
            self.engine.reload();
            self.toast(
                ToastKind::Error,
                format!("OpenWX is using {mb:.0} MB of memory. Map tiles were freed; turn on Low-resource mode in Settings if this repeats"),
            );
        } else if mb < limit * 0.8 {
            self.ext.mem_warned = false;
        }
    }

    fn check_output(&mut self) {
        let stats = self.compositor.stats();
        let on_air_somewhere = self.output_window.is_some() || self.recording.is_some();
        if stats.black_for >= 3.0 && on_air_somewhere && !self.black {
            if !self.ext.black_warned {
                self.ext.black_warned = true;
                log::error!("Program output has been black for {:.0} s without Black being on", stats.black_for);
                self.toast(ToastKind::Error, "Program output is black. Check the map (F5 refreshes it) or take another scene");
            }
        } else {
            self.ext.black_warned = false;
        }
        let late_frames = stats.late_frames.saturating_sub(self.ext.late_seen);
        let budget_ms = if self.settings.low_resource { 66.7 } else { 33.3 };
        if stats.render_ms <= budget_ms {
            self.ext.late_seen = stats.late_frames;
        } else if late_frames > 60 && self.ext.last_late_warning.is_none_or(|at| at.elapsed() >= Duration::from_secs(60)) {
            log::warn!(
                "compositor running slow: {} late frames, {:.1} ms average, {:.1} ms worst, {:.0} fps",
                late_frames,
                stats.render_ms,
                stats.worst_ms,
                stats.fps
            );
            self.ext.late_seen = stats.late_frames;
            self.ext.last_late_warning = Some(Instant::now());
        }
    }

    // ---- Sponsors ----------------------------------------------------------------------------

    /// All sponsors in rotation order: the main one first, then the extra list.
    pub fn sponsor_lineup(&self) -> Vec<crate::config::Sponsor> {
        let mut list = Vec::new();
        if !self.settings.sponsor_name.trim().is_empty() {
            list.push(crate::config::Sponsor {
                name: self.settings.sponsor_name.clone(),
                tagline: self.settings.sponsor_tagline.clone(),
                logo_media: self.settings.sponsor_logo_media.clone(),
            });
        }
        list.extend(self.settings.sponsors.iter().filter(|s| !s.name.trim().is_empty()).cloned());
        list
    }

    /// The sponsor on air now (rotation applied).
    pub fn current_sponsor(&self) -> Option<crate::config::Sponsor> {
        let list = self.sponsor_lineup();
        if list.is_empty() {
            return None;
        }
        Some(list[self.ext.sponsor_index % list.len()].clone())
    }

    fn rotate_sponsor(&mut self) {
        let minutes = self.settings.sponsor_rotate_minutes;
        if minutes == 0 || self.ext.sponsor_switched.elapsed() < Duration::from_secs(minutes as u64 * 60) {
            return;
        }
        let count = self.sponsor_lineup().len();
        if count < 2 {
            return;
        }
        self.ext.sponsor_switched = Instant::now();
        self.ext.sponsor_index = (self.ext.sponsor_index + 1) % count;
        self.push_sponsor();
    }

    /// Send the current sponsor to the compositor so {{sponsor}} bindings follow the rotation.
    pub fn push_sponsor(&mut self) {
        let Some(sponsor) = self.current_sponsor() else { return };
        log::info!("sponsor rotation: {}", sponsor.name);
        let mut settings = (*self.compositor.read(|p| p.settings.clone()).unwrap_or_else(|| std::sync::Arc::new(self.settings.clone()))).clone();
        settings.sponsor_name = sponsor.name;
        settings.sponsor_tagline = sponsor.tagline;
        settings.sponsor_logo_media = sponsor.logo_media;
        let settings = std::sync::Arc::new(settings);
        self.compositor.update(|p| p.settings = settings);
    }

    /// Seconds a sponsor element is on the Program output, written to a CSV once a minute.
    fn count_impressions(&mut self) {
        let dt = self.ext.last_impression_tick.elapsed().as_secs_f64();
        self.ext.last_impression_tick = Instant::now();
        if self.black || dt > 1.0 {
            return;
        }
        let on_program = match &self.on_air {
            OnAir::Graphic(name) => name.to_lowercase().contains("sponsor"),
            OnAir::LiveMap => self.live.show_sponsor || self.live.overlay_graphic.iter()
                .chain(self.live.overlay_slots.iter().flatten())
                .filter_map(|id| self.graphics.iter().find(|g| &g.id == id))
                .any(|g| g.name.to_lowercase().contains("sponsor") || g.template.as_deref().is_some_and(|name| name.starts_with("sponsor"))),
        };
        let watched = self.output_window.is_some() || self.recording.is_some();
        if on_program && watched {
            if let Some(s) = self.current_sponsor() {
                *self.ext.impressions.entry(s.name).or_default() += dt;
            }
        }
        if self.ext.impressions_flushed.elapsed() >= Duration::from_secs(60) {
            self.ext.impressions_flushed = Instant::now();
            let batch = std::mem::take(&mut self.ext.impressions);
            if !batch.is_empty() {
                std::thread::spawn(move || write_impressions(batch));
            }
        }
    }

    // ---- Scheduled exports -----------------------------------------------------------------

    fn run_scheduled_exports(&mut self) {
        let minutes = self.settings.scheduled_export_minutes;
        if minutes == 0 || self.settings.scheduled_exports.is_empty() {
            return;
        }
        if self.ext.exported_at.elapsed() < Duration::from_secs(minutes as u64 * 60) {
            return;
        }
        self.ext.exported_at = Instant::now();
        self.export_scheduled_now();
    }

    /// Save the Program map (with overlays) as a PNG in Exports.
    pub fn save_map_snapshot(&mut self) {
        match self.compositor.map_frame() {
            Some(frame) => {
                let mut rgba = frame.bgra.as_ref().clone();
                for px in rgba.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
                let size = tiny_skia::IntSize::from_wh(frame.width, frame.height);
                let pm = size.and_then(|s| tiny_skia::Pixmap::from_vec(rgba, s));
                match pm.map(|pm| crate::library::export_png(&pm, "Map snapshot", None)) {
                    Some(Ok(path)) => {
                        self.exports = crate::library::list_exports();
                        self.toast(ToastKind::Success, format!("Map saved to {}", path.display()));
                    }
                    Some(Err(e)) => self.toast(ToastKind::Error, format!("Could not save the map: {e}")),
                    None => {}
                }
            }
            None => self.toast(ToastKind::Info, "The map has not drawn a frame yet"),
        }
    }

    /// Render every scheduled graphic to `Exports/Scheduled/<name>.png` (overwritten each time,
    /// so websites and OBS image sources always find the latest).
    pub fn export_scheduled_now(&mut self) {
        let dir = crate::config::LibraryPaths::get().exports.join("Scheduled");
        let _ = std::fs::create_dir_all(&dir);
        let mut done = 0;
        for id in self.settings.scheduled_exports.clone() {
            let Some(doc) = self.graphics.iter().find(|g| g.id == id).cloned() else { continue };
            if let Some(src) = crate::sources::blocked_source(&self.settings, &doc) {
                log::warn!("scheduled export skipped \"{}\": uses {src}, which is not cleared for redistribution", doc.name);
                continue;
            }
            let pm = crate::program::render_graphic(&doc, &self.settings, &self.data, 30.0, crate::config::FRAME_W, crate::config::FRAME_H, None);
            if let Some(png) = ops::pixmap_png(&pm) {
                let safe: String = doc.name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
                if std::fs::write(dir.join(format!("{}.png", safe.trim())), png).is_ok() {
                    done += 1;
                }
            }
        }
        // The map too, as Map.png.
        if let Some(frame) = self.compositor.onair_map_frame().filter(|_| self.settings.scheduled_map_snapshot) {
            let mut rgba = frame.bgra.as_ref().clone();
            for px in rgba.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
            if let Some(img) = image::RgbaImage::from_raw(frame.width, frame.height, rgba) {
                if img.save(dir.join("Map.png")).is_ok() {
                    done += 1;
                }
            }
        }
        log::info!("scheduled export: {done} image(s) written to {}", dir.display());
    }

    // ---- Forecast editor ---------------------------------------------------------------------

    /// Save the editor state (after any draft change).
    pub fn edits_changed(&mut self, what: &str) {
        self.ext.edits.log(what.to_string());
        crate::forecast_edit::save(&self.ext.edits);
    }

    /// Put the draft on air: day overrides, regions and the adjusted model grid.
    pub fn publish_forecast(&mut self) {
        let issues = self.ext.edits.draft.check(&self.data.overview.as_ref().map(|o| o.days()).unwrap_or_default());
        self.ext.edits.publish();
        crate::forecast_edit::save(&self.ext.edits);
        crate::desk::log_decision(&format!(
            "Published the forecast ({} day override(s), {} adjustment(s), {} region(s))",
            self.ext.edits.published.days.len(),
            self.ext.edits.published.adjustments.len(),
            self.ext.edits.published.regions.len()
        ));
        self.apply_published_edits();
        self.toast(
            ToastKind::Success,
            if issues.is_empty() { "Forecast published to every graphic and map".to_string() } else { format!("Forecast published ({} warning(s) in the checks)", issues.len()) },
        );
    }

    /// Scenario demonstration: put a saved scenario (or None = the published forecast) on air in
    /// every map and graphic, without publishing it. "What if the front is 3 hours slower?"
    pub fn demo_scenario(&mut self, index: Option<usize>) {
        let set = match index.and_then(|i| self.ext.edits.scenarios.get(i).cloned()) {
            Some((name, set)) => {
                self.ext.demo_scenario = Some(name.clone());
                self.toast(ToastKind::Info, format!("On air: scenario \"{name}\" (not published)"));
                set
            }
            None => {
                self.ext.demo_scenario = None;
                self.ext.edits.published.clone()
            }
        };
        let mut data = (*self.data).clone();
        data.forecast_edits = set.clone();
        if let Some(raw) = &self.ext.raw_grid {
            data.grid = Some(std::sync::Arc::new(raw.adjusted(&set)));
        }
        self.data = std::sync::Arc::new(data);
        let snapshot = self.data.clone();
        self.compositor.update(|p| p.data = snapshot);
    }

    pub fn apply_published_edits(&mut self) {
        self.ext.demo_scenario = None;
        let mut data = (*self.data).clone();
        data.forecast_edits = self.ext.edits.published.clone();
        if let Some(raw) = &self.ext.raw_grid {
            data.grid = Some(std::sync::Arc::new(raw.adjusted(&self.ext.edits.published)));
        }
        self.data = std::sync::Arc::new(data);
        let snapshot = self.data.clone();
        self.compositor.update(|p| p.data = snapshot);
    }

    /// Paint an adjustment at a spot (Map Studio > Paint forecast).
    pub fn paint_adjustment(&mut self, lat: f64, lon: f64, var: crate::forecast_edit::EditVar, delta: f32, radius_mi: f64) {
        self.ext.edits.draft.adjustments.push(crate::forecast_edit::Adjustment {
            name: format!("{} {:+.0} near {:.2}, {:.2}", var.label(), delta, lat, lon),
            var,
            delta,
            shape: crate::forecast_edit::Shape::Circle { lat, lon, radius_mi },
            from_hour: 0.0,
            to_hour: 48.0,
            ramp: true,
        });
        self.edits_changed("Painted an area adjustment");
    }

    /// Turn the last drawn Area on the map into an adjustment.
    pub fn adjustment_from_drawn_area(&mut self, var: crate::forecast_edit::EditVar, delta: f32) -> bool {
        let poly = self
            .live
            .annotations
            .iter()
            .rev()
            .find(|a| matches!(a.kind, crate::map::scene::AnnotationKind::Polygon) && a.points.len() >= 3)
            .map(|a| a.points.clone());
        let Some(points) = poly else {
            self.toast(ToastKind::Info, "Draw an Area on the map first (Draw > Area)");
            return false;
        };
        self.ext.edits.draft.adjustments.push(crate::forecast_edit::Adjustment {
            name: format!("{} {:+.0} in drawn area", var.label(), delta),
            var,
            delta,
            shape: crate::forecast_edit::Shape::Polygon(points),
            from_hour: 0.0,
            to_hour: 48.0,
            ramp: true,
        });
        self.edits_changed("Adjustment from a drawn area");
        true
    }

    /// Make a region from the last drawn Area.
    pub fn region_from_drawn_area(&mut self, name: String) -> bool {
        let poly = self
            .live
            .annotations
            .iter()
            .rev()
            .find(|a| matches!(a.kind, crate::map::scene::AnnotationKind::Polygon) && a.points.len() >= 3)
            .map(|a| a.points.clone());
        let Some(points) = poly else {
            self.toast(ToastKind::Info, "Draw an Area on the map first (Draw > Area)");
            return false;
        };
        self.ext.edits.draft.regions.push(crate::forecast_edit::Region {
            name,
            shape: crate::forecast_edit::Shape::Polygon(points),
            ..Default::default()
        });
        self.edits_changed("Added a region");
        true
    }

    // ---- Camera: recorded paths, framing, follow storm -------------------------------------

    pub fn record_camera_key(&mut self) {
        let view = self.live.view;
        self.update_live(|l| l.camera_path.push(crate::map::scene::CameraKey { view, fly: 2.5, hold: 2.0 }));
        let n = self.live.camera_path.len();
        self.toast(ToastKind::Info, format!("Camera stop {n} recorded"));
    }

    /// Automated warning presentation: zoom to and call out each coverage warning in turn
    /// (most dangerous first), holding each for `hold` seconds; then back to coverage.
    pub fn warning_tour(&mut self, hold: f32) {
        let mut warnings: Vec<crate::data::Alert> = self.coverage_warnings().into_iter().filter(|a| a.is_warning()).collect();
        warnings.sort_by_key(|a| a.priority());
        if warnings.is_empty() {
            self.toast(ToastKind::Info, "No warnings in the coverage area to present");
            return;
        }
        let ids: Vec<String> = warnings.iter().map(|a| a.id.clone()).collect();
        self.toast(ToastKind::Info, format!("Presenting {} warning(s), {hold:.0} s each", ids.len()));
        self.ext.tour = Some(Tour { stops: ids.into_iter().map(TourStop::Warning).collect(), index: 0, next_at: Instant::now(), hold });
    }

    /// Automated regional sequence: fly town to town (forecast regions first, else the largest
    /// towns in coverage) with temperatures plotted, then home.
    pub fn regional_tour(&mut self, hold: f32) {
        let mut stops: Vec<TourStop> = self
            .data
            .forecast_edits
            .regions
            .iter()
            .map(|r| {
                let (lat, lon) = r.shape.center();
                TourStop::Place(lat, lon)
            })
            .collect();
        if stops.is_empty() {
            let home = self.settings.home;
            let d = self.settings.coverage_miles.max(40.0) / 69.0;
            let mut towns = crate::places::in_box(home.lat - d, home.lat + d, home.lon - d * 1.3, home.lon + d * 1.3);
            towns.sort_by(|a, b| b.population.cmp(&a.population));
            stops = towns.into_iter().take(5).map(|t| TourStop::Place(t.lat, t.lon)).collect();
        }
        if stops.is_empty() {
            return;
        }
        self.update_live(|l| {
            if l.data.field.is_none() {
                l.data.field = Some(crate::map::scene::DataField::Temperature);
            }
            l.data.city_plots = true;
        });
        self.ext.tour = Some(Tour { stops, index: 0, next_at: Instant::now(), hold });
    }

    /// Automated storm path: the strongest moving storm gets a track with arrival times, the
    /// storm-track layer, and a camera move that frames its path.
    pub fn auto_storm_path(&mut self) {
        let best = self
            .data
            .cells
            .iter()
            .filter(|c| c.motion.as_ref().map(|m| m.speed_mph >= 5.0).unwrap_or(false))
            .max_by(|a, b| a.max_dbz.total_cmp(&b.max_dbz))
            .cloned();
        let Some(cell) = best else {
            self.toast(ToastKind::Info, "No moving storms to track right now");
            return;
        };
        let m = cell.motion.clone().unwrap_or_default();
        self.update_live(|l| {
            let mut track = l.storm_track.clone().unwrap_or_default();
            track.lat = cell.lat;
            track.lon = cell.lon;
            track.heading_deg = m.heading_deg;
            track.speed_mph = m.speed_mph;
            track.path.clear();
            l.storm_track = Some(track);
            l.storm_tracks = true;
            l.storm_cells = true;
        });
        // Frame the storm and where it will be in an hour.
        let km = m.speed_mph * 1.609_344;
        let (lat2, lon2) = crate::map::geo::destination(cell.lat, cell.lon, m.heading_deg, km);
        let mid = crate::config::GeoView { lat: (cell.lat + lat2) / 2.0, lon: (cell.lon + lon2) / 2.0, zoom: if m.speed_mph > 40.0 { 7.6 } else { 8.4 } };
        self.fly_to(mid, 2.0);
        self.toast(ToastKind::Success, format!("Tracking the strongest storm: {:.0} dBZ moving {} at {:.0} mph", cell.max_dbz, m.toward, m.speed_mph));
    }

    /// Flood monitoring each data refresh: rapid river rises and heavy-rain exceedance in coverage,
    /// plus basin averages for loaded watershed polygons.
    pub fn flood_watch(&mut self) {
        // Rapid rises only matter in the coverage area and near flood stage: the river feed
        // covers ~175 miles around home for the map, and a dam release 200 miles away raised a
        // red alert (CWXB-71).
        let home = self.settings.home;
        let miles = self.settings.coverage_miles.max(10.0);
        let near: Vec<crate::feeds::RiverGauge> = self
            .data
            .rivers
            .iter()
            .filter(|g| crate::map::geo::haversine_km(home.lat, home.lon, g.lat, g.lon) / crate::map::geo::KM_PER_MILE <= miles)
            .cloned()
            .collect();
        for (name, rate) in crate::feeds::rapid_rises(&near) {
            self.toast(ToastKind::Info, format!("Rapid rise: {name} up {rate:.1} ft/hr"));
            crate::broadcast_ops::log_incident("Rapid river rise", &format!("{name}: {rate:.1} ft/hr"));
        }
        if let Some(grid) = self.data.grid.as_deref() {
            let home = self.settings.home;
            let d = self.settings.coverage_miles / 69.0;
            let tf = grid.time_index(0.0);
            let mut worst = 0.0f32;
            for k in 0..9 {
                let (dy, dx) = (((k / 3) as f64 - 1.0) * d, ((k % 3) as f64 - 1.0) * d);
                worst = worst.max(grid.value(crate::map::scene::DataField::RainRateExceed, tf, home.lat + dy, home.lon + dx).unwrap_or(0.0));
            }
            if worst >= 1.0 && self.ext.rain_alerted.map(|t| t.elapsed() > Duration::from_secs(3600)).unwrap_or(true) {
                self.ext.rain_alerted = Some(Instant::now());
                self.toast(ToastKind::Error, format!("Rain rates over {worst:.1}\"/hr in the coverage area: flash flooding possible"));
            }
        }
    }

    /// Earthquakes (USGS), every 5 minutes: the map layer, Map Studio list and quake graphics.
    fn maybe_fetch_quakes(&mut self, cx: &mut Context<Self>) {
        if self.ext.quakes_at.is_some_and(|t| t.elapsed() < Duration::from_secs(5 * 60)) {
            return;
        }
        self.ext.quakes_at = Some(Instant::now());
        cx.spawn(async move |this, cx| {
            let Ok(result) = crate::net::spawn(crate::quakes::fetch()).await else { return };
            let _ = this.update(cx, |s, cx| match result {
                Ok(quakes) => {
                    let mut data = (*s.data).clone();
                    data.quakes = quakes;
                    s.data = std::sync::Arc::new(data);
                    let snapshot = s.data.clone();
                    s.compositor.update(|p| p.data = snapshot);
                    cx.notify();
                }
                Err(e) => log::warn!("earthquakes (USGS): {e:#}"),
            });
        })
        .detach();
    }

    fn maybe_fetch_hazards(&mut self, cx: &mut Context<Self>) {
        let home = self.settings.home;
        if self.ext.hazards_at.map(|t| t.elapsed() >= Duration::from_secs(15 * 60)).unwrap_or(true) {
            self.ext.hazards_at = Some(Instant::now());
            let d = 6.0;
            cx.spawn(async move |this, cx| {
                // Catalog sets (gfx::sets) fetch their own data on their own task: there are many
                // of them now, and waiting for all of them held drought, fire and tropical maps
                // back until every set had finished (drought never showed when one was slow).
                let sets_home = home.clone();
                crate::net::spawn(async move {
                    crate::gfx::sets::refresh(sets_home.clone()).await;
                    crate::gfx::history_set::refresh(sets_home).await;
                });
                let task = crate::net::spawn(async move {
                    let (tropical, fire, wild, drought, month, season) = futures_util::join!(
                        crate::hazards::tropical(),
                        crate::hazards::fire_outlook(),
                        crate::hazards::wildfires(home.lat - d, home.lat + d, home.lon - d * 1.3, home.lon + d * 1.3),
                        crate::hazards::drought(),
                        crate::hazards::drought_outlook(1),
                        crate::hazards::drought_outlook(4),
                    );
                    let outlooks = (month.inspect_err(|e| log::warn!("drought outlook (month): {e:#}")).ok(), season.inspect_err(|e| log::warn!("drought outlook (season): {e:#}")).ok());
                    let (tropical, fire, wild) = (tropical.unwrap_or_default(), fire.unwrap_or_default(), wild.unwrap_or_default());
                    if let Err(e) = &drought {
                        log::warn!("drought monitor: {e:#}");
                    }
                    // Spaghetti: try the first few Atlantic storm numbers this year when a storm is active.
                    let mut models = Vec::new();
                    if !tropical.points.is_empty() {
                        for n in (1..=30).rev() {
                            if let Ok(m) = crate::hazards::atcf_models(n).await {
                                if !m.is_empty() {
                                    models = m;
                                    break;
                                }
                            }
                        }
                    }
                    (tropical, fire, wild, drought, outlooks, models)
                });
                let Ok((mut tropical, fire, wild, drought, outlooks, models)) = task.await else { return };
                tropical.models = models;
                let _ = this.update(cx, |s, cx| {
                    let mut data = (*s.data).clone();
                    data.hazards.tropical = tropical;
                    data.hazards.fire_outlook = fire;
                    data.hazards.wildfires = wild;
                    if let Ok(drought) = drought {
                        data.hazards.drought = drought;
                    }
                    if let Some((shapes, target, issued)) = outlooks.0 {
                        data.hazards.drought_month = shapes;
                        data.hazards.drought_month_target = target;
                        data.hazards.drought_outlook_issued = issued;
                    }
                    if let Some((shapes, target, _)) = outlooks.1 {
                        data.hazards.drought_season = shapes;
                        data.hazards.drought_season_target = target;
                    }
                    s.data = std::sync::Arc::new(data);
                    let snapshot = s.data.clone();
                    s.compositor.update(|p| p.data = snapshot);
                    cx.notify();
                });
            })
            .detach();
        }
        if self.settings.fetch_legacy_climate_history
            && self.ext.history_at.is_none_or(|t| t.elapsed() >= self.ext.history_retry)
            && self.data.climate.normals.is_some()
        {
            self.ext.history_at = Some(Instant::now());
            self.ext.history_retry = Duration::from_secs(6 * 3600);
            let normals = self.data.climate.normals.clone();
            cx.spawn(async move |this, cx| {
                let result = crate::net::spawn(crate::hazards::history(home.lat, home.lon, normals)).await;
                let _ = this.update(cx, |s, cx| {
                    if s.settings.home.lat != home.lat || s.settings.home.lon != home.lon || !s.settings.fetch_legacy_climate_history {
                        return;
                    }
                    s.ext.history_at = Some(Instant::now());
                    match result {
                        Ok(Ok(h)) => {
                            let mut data = (*s.data).clone();
                            data.hazards.history = h;
                            s.data = std::sync::Arc::new(data);
                            let snapshot = s.data.clone();
                            s.compositor.update(|p| p.data = snapshot);
                            cx.notify();
                        }
                        Ok(Err(error)) => {
                            let wait = error.downcast_ref::<crate::net::RateLimit>()
                                .map(|limit| limit.retry_after).unwrap_or_default();
                            s.ext.history_retry = wait.max(Duration::from_secs(5 * 60));
                        }
                        Err(_) => s.ext.history_retry = Duration::from_secs(5 * 60),
                    }
                });
            })
            .detach();
        }
    }

    /// Uncertainty cone: widen the storm track's cone by how much the nearest tracked storm's
    /// heading has wobbled over the last hour (10-45 degrees).
    pub fn cone_from_history(&mut self) {
        let Some(track) = self.live.storm_track.clone() else {
            self.toast(ToastKind::Info, "Place a storm track first (Severe tab or Track the strongest storm)");
            return;
        };
        let best = self
            .data
            .analysis
            .tracks
            .iter()
            .filter(|t| !t.ended && t.points.len() >= 3)
            .min_by(|a, b| {
                let d = |t: &crate::cells::Track| t.points.last().map(|p| crate::map::geo::haversine_km(p.lat, p.lon, track.lat, track.lon)).unwrap_or(f64::MAX);
                d(a).total_cmp(&d(b))
            })
            .cloned();
        let Some(t) = best else {
            self.toast(ToastKind::Info, "No tracked storm history near the track yet");
            return;
        };
        let headings: Vec<f64> = t.points.windows(2).map(|w| crate::map::geo::bearing_deg(w[0].lat, w[0].lon, w[1].lat, w[1].lon)).collect();
        let mean = headings.iter().sum::<f64>() / headings.len() as f64;
        let sd = (headings.iter().map(|h| ((h - mean + 540.0) % 360.0 - 180.0).powi(2)).sum::<f64>() / headings.len() as f64).sqrt();
        let cone = (sd * 2.0).clamp(10.0, 45.0);
        self.update_live(|l| {
            if let Some(tr) = l.storm_track.as_mut() {
                tr.cone_deg = cone;
            }
        });
        self.toast(ToastKind::Success, format!("Cone set to {cone:.0}° from storm {}'s last hour", t.id));
    }

    /// Run a graphic interaction: hotspot click / touch, presenter remote or control API.
    /// Every interaction is logged (Developer Logs folder, interactions-YYYY-MM-DD.log).
    pub fn graphic_action(&mut self, action: &str, source: &str) {
        let line = format!("{} {source}: {action}", chrono::Local::now().format("%H:%M:%S"));
        let path = crate::devlog::path().with_file_name(format!("interactions-{}.log", chrono::Local::now().format("%Y-%m-%d")));
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            use std::io::Write;
            let _ = writeln!(f, "{line}");
        }
        log::info!("interaction: {line}");
        let (kind, arg) = action.split_once(':').map(|(a, b)| (a.trim(), b.trim().to_string())).unwrap_or((action.trim(), String::new()));
        match kind {
            "state" => self.compositor.update(|p| p.graphic_state = arg),
            "toggle" => self.compositor.update(|p| {
                if let Some(i) = p.hidden_layers.iter().position(|n| *n == arg) {
                    p.hidden_layers.remove(i);
                } else {
                    p.hidden_layers.push(arg);
                }
            }),
            "graphic" => {
                if let Some(doc) = self.graphics.iter().find(|g| g.name.eq_ignore_ascii_case(&arg)).cloned() {
                    self.compositor.update(|p| {
                        p.graphic_state.clear();
                        p.hidden_layers.clear();
                    });
                    self.take_graphic(doc, crate::show::Transition::Fade);
                }
            }
            "pause" => self.compositor.update(|p| {
                if p.paused_at.is_none() {
                    p.paused_at = Some(p.source_started.elapsed().as_secs_f32());
                }
            }),
            "play" | "resume" => self.compositor.update(|p| {
                if let Some(t) = p.paused_at.take() {
                    p.source_started = std::time::Instant::now() - Duration::from_secs_f32(t);
                }
            }),
            "next" => self.next_scene(),
            _ => log::warn!("unknown interaction {action}"),
        }
    }

    /// A click or touch on the output at frame pixel (x, y): run the hotspot under it.
    pub fn output_click(&mut self, x: f32, y: f32) {
        let hit = self.compositor.read(|p| match &p.source {
            crate::program::ProgramSource::Graphic(doc) => {
                let sx = doc.width as f32 / crate::config::FRAME_W as f32;
                let sy = doc.height as f32 / crate::config::FRAME_H as f32;
                doc.hotspot_at(x * sx, y * sy, &p.graphic_state)
            }
            _ => None,
        }).flatten();
        if let Some((layer, action)) = hit {
            self.graphic_action(&action, &format!("touch on \"{layer}\""));
        }
    }

    /// Start or stop the automated broadcast (continuous weather channel).
    pub fn toggle_autocast(&mut self) {
        if self.ext.autocast.take().is_some() {
            self.compositor.update(|p| p.ticker = None);
            self.toast(ToastKind::Info, "Automated broadcast stopped");
            return;
        }
        let seen = self.coverage_warnings().into_iter().map(|a| a.id).collect();
        self.ext.autocast = Some(Autocast { index: 0, next_at: Instant::now(), interrupted: None, last_id: Instant::now(), cycles: 0, seen_warnings: seen });
        self.toast(ToastKind::Success, "Automated broadcast running");
    }

    pub fn autocast_running(&self) -> bool {
        self.ext.autocast.is_some()
    }

    fn autocast_tick(&mut self, cx: &mut Context<Self>) {
        let Some(mut ac) = self.ext.autocast.take() else { return };
        let now = Instant::now();
        // Ticker with current conditions and the warnings in effect.
        if self.settings.autocast_ticker {
            let b = crate::gfx::bind::Bindings::build(&self.data, &self.settings, None, &self.settings.radar_palette);
            let warnings: Vec<String> = self.coverage_warnings().iter().map(|a| a.event.clone()).collect();
            let text = format!(
                "{}  •  Now {} {}  •  Today {} / {}  •  {}{}",
                self.settings.station_name,
                b.get("temp").unwrap_or(""),
                b.get("condition").unwrap_or(""),
                b.get("day0.hi").unwrap_or(""),
                b.get("day0.lo").unwrap_or(""),
                b.get("day0.condition").unwrap_or(""),
                if warnings.is_empty() { String::new() } else { format!("  •  IN EFFECT: {}", warnings.join(", ")) }
            );
            self.compositor.update(|p| p.ticker = Some(text));
        } else {
            self.compositor.update(|p| p.ticker = None);
        }
        // Automated severe interruption: a new warning in coverage breaks in for 45 s.
        let fresh = self.coverage_warnings().into_iter().filter(|a| a.is_warning()).find(|a| !ac.seen_warnings.contains(&a.id));
        if let Some(w) = fresh {
            ac.seen_warnings.insert(w.id.clone());
            self.warning_graphic(&w.id, true);
            self.speak_alert(&w.id);
            ac.interrupted = Some((w.id.clone(), now + Duration::from_secs(45)));
            log::warn!("automated broadcast interrupted for {}", w.event);
        }
        if let Some((_, until)) = &ac.interrupted {
            if now < *until {
                self.ext.autocast = Some(ac);
                return;
            }
            ac.interrupted = None;
            ac.next_at = now;
        }
        // Automatic recovery: if Program went black or the output closed, carry on.
        if self.black {
            self.set_black(false);
        }
        if now >= ac.next_at {
            // Scheduled station identification.
            let id_due = self.settings.autocast_id_minutes > 0 && ac.last_id.elapsed() >= Duration::from_secs(self.settings.autocast_id_minutes as u64 * 60);
            let seconds = self.settings.autocast_seconds.max(5.0);
            if id_due {
                ac.last_id = now;
                if let Some(doc) = crate::gfx::templates::build("station_id") {
                    self.take_graphic(doc, crate::show::Transition::Fade);
                }
                ac.next_at = now + Duration::from_secs(8);
            } else {
                let segments = if self.settings.autocast_segments.is_empty() { vec!["current".to_string(), "radar".into()] } else { self.settings.autocast_segments.clone() };
                let key = segments[ac.index % segments.len()].clone();
                ac.index += 1;
                if ac.index % segments.len() == 0 {
                    ac.cycles += 1;
                    if self.settings.autocast_bulletin {
                        self.speak_forecast();
                    }
                }
                match key.as_str() {
                    "radar" | "regional" => {
                        // Radar loop insertion or a regional temperature map.
                        let home = self.settings.home;
                        self.update_live(|l| {
                            l.radar.enabled = true;
                            l.data.field = if key == "regional" { Some(crate::map::scene::DataField::Temperature) } else { None };
                            l.data.city_plots = key == "regional";
                        });
                        self.fly_to(crate::config::GeoView { zoom: if key == "regional" { 7.0 } else { 7.8 }, ..home }, 1.5);
                        self.playing = true;
                        self.take_live_map(crate::show::Transition::Fade);
                    }
                    other => {
                        // Graphic segments (local forecast insertion uses the home location).
                        let doc = self.graphics.iter().find(|g| g.template.as_deref() == Some(other)).cloned().or_else(|| crate::gfx::templates::build(other));
                        if let Some(doc) = doc {
                            self.take_graphic(doc, crate::show::Transition::Fade);
                        }
                    }
                }
                ac.next_at = now + Duration::from_secs_f32(seconds);
            }
        }
        let _ = cx;
        self.ext.autocast = Some(ac);
    }

    fn tour_tick(&mut self) {
        let Some(mut tour) = self.ext.tour.take() else { return };
        if Instant::now() < tour.next_at {
            self.ext.tour = Some(tour);
            return;
        }
        match tour.stops.get(tour.index).cloned() {
            Some(TourStop::Warning(id)) => {
                self.zoom_to_alert(&id);
                self.select_alert(Some(id));
            }
            Some(TourStop::Place(lat, lon)) => {
                let zoom = 9.0;
                self.fly_to(crate::config::GeoView { lat, lon, zoom }, 1.8);
            }
            None => {
                // Done: back to the coverage area.
                self.select_alert(None);
                self.frame_coverage();
                return;
            }
        }
        tour.index += 1;
        tour.next_at = Instant::now() + Duration::from_secs_f32(tour.hold.max(3.0) + 1.8);
        self.ext.tour = Some(tour);
    }

    pub fn stop_tour(&mut self) {
        self.ext.tour = None;
    }

    pub fn tour_running(&self) -> bool {
        self.ext.tour.is_some()
    }

    pub fn play_camera_path(&mut self) {
        let path = self.live.camera_path.clone();
        if path.is_empty() {
            self.toast(ToastKind::Info, "Record some camera stops first");
            return;
        }
        self.ext.camera_play = Some((path, 0, Instant::now()));
    }

    pub fn stop_camera_path(&mut self) {
        self.ext.camera_play = None;
    }

    /// Frame every place given (warnings, watched spots…) in one view.
    pub fn frame_all(&mut self, points: Vec<(f64, f64)>) {
        if let Some(view) = crate::autoshow::frame_points(&points) {
            self.fly_to(view, 2.0);
        }
    }

    /// Frame home, the watched spots and every warning in the coverage area.
    pub fn frame_coverage(&mut self) {
        let mut pts = vec![(self.settings.home.lat, self.settings.home.lon)];
        pts.extend(self.settings.alert_zones.iter().map(|z| (z.lat, z.lon)));
        for a in self.coverage_warnings() {
            for ring in &a.polygons {
                pts.extend(ring.iter().map(|(lon, lat)| (*lat, *lon)));
            }
        }
        self.frame_all(pts);
    }

    fn camera_tick(&mut self) {
        self.tour_tick();
        // A show's map scene with a camera path plays it when taken.
        if self.show_pos != self.ext.last_show_pos {
            self.ext.last_show_pos = self.show_pos;
            if let Some(crate::show::SceneKind::Map { map }) = self.show_pos.and_then(|i| self.show.scenes.get(i)).map(|s| s.kind.clone()) {
                if !map.camera_path.is_empty() {
                    self.ext.camera_play = Some((map.camera_path.clone(), 0, Instant::now()));
                }
            }
        }
        if let Some((path, i, at)) = self.ext.camera_play.clone() {
            if Instant::now() >= at {
                match path.get(i) {
                    Some(key) => {
                        self.fly_to(key.view, key.fly);
                        self.ext.camera_play = Some((path.clone(), i + 1, Instant::now() + Duration::from_secs_f32(key.fly + key.hold)));
                    }
                    None => self.ext.camera_play = None,
                }
            }
        }
        // Follow storm: every 3 s, recentre on the storm if it drifted from the centre.
        if self.live.follow_storm && self.ext.follow_at.elapsed() >= Duration::from_secs(3) {
            self.ext.follow_at = Instant::now();
            // A called-out alert without storm motion (a watch, an advisory) stays in view: only
            // with nothing called out does the camera follow the strongest cell.
            let selected = self.live.selected_alert.clone().and_then(|id| self.find_alert(&id));
            let alert_without_motion = selected.as_ref().is_some_and(|a| a.motion().is_none());
            let target = selected
                .and_then(|a| {
                    let m = a.motion()?;
                    // Project the storm position from the warning's issue time to now.
                    let sent = a.sent.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())?;
                    let hours = (chrono::Utc::now().timestamp() - sent.timestamp()) as f64 / 3600.0;
                    let km = m.speed_mph * hours.clamp(0.0, 2.0) * 1.609_344;
                    Some(crate::map::geo::destination(m.lat, m.lon, m.heading_deg, km))
                })
                .or_else(|| {
                    if alert_without_motion {
                        return None;
                    }
                    self.data
                        .cells
                        .iter()
                        .max_by(|a, b| a.max_dbz.total_cmp(&b.max_dbz))
                        .map(|c| (c.lat, c.lon))
                });
            if let Some((lat, lon)) = target {
                let v = self.live.view;
                if crate::map::geo::haversine_km(v.lat, v.lon, lat, lon) > 3.0 && self.fly.is_none() {
                    self.fly_to(crate::config::GeoView { lat, lon, zoom: v.zoom }, 2.5);
                }
            }
        }
    }

    // ---- Rivers, rainfall forecast, excessive rain, community board -------------------------

    fn maybe_fetch_hydro(&mut self, cx: &mut Context<Self>) {
        let due = self.ext.hydro_at.map(|t| t.elapsed() >= Duration::from_secs(15 * 60)).unwrap_or(true);
        if !due || self.ext.hydro_loading {
            return;
        }
        self.ext.hydro_loading = true;
        self.ext.hydro_at = Some(Instant::now());
        let home = self.settings.home;
        cx.spawn(async move |this, cx| {
            use crate::feeds::{excessive_rain, qpf, rivers, QpfPeriod};
            let r = crate::net::spawn(rivers(home.lat - 2.5, home.lat + 2.5, home.lon - 3.2, home.lon + 3.2));
            let q1 = crate::net::spawn(qpf(QpfPeriod::Day1));
            let q3 = crate::net::spawn(qpf(QpfPeriod::Days1to3));
            let q7 = crate::net::spawn(qpf(QpfPeriod::Days1to7));
            let e1 = crate::net::spawn(excessive_rain(1));
            let e2 = crate::net::spawn(excessive_rain(2));
            let e3 = crate::net::spawn(excessive_rain(3));
            let (r, q1, q3, q7, e1, e2, e3) = futures_util::join!(r, q1, q3, q7, e1, e2, e3);
            let community = crate::community::load();
            let worst = match &r {
                Ok(Ok(g)) => g.first().map(|g| g.lid.clone()),
                _ => None,
            };
            let hydro = match worst {
                Some(lid) => crate::net::spawn(async move { crate::feeds::hydrograph(&lid).await }).await.ok().and_then(|r| r.ok()),
                None => None,
            };
            let _ = this.update(cx, |s, cx| {
                s.ext.hydro_loading = false;
                let mut data = (*s.data).clone();
                let mut problems = Vec::new();
                let mut take = |name: &str, res: Result<anyhow::Result<Vec<crate::data::SpcArea>>, tokio::task::JoinError>, slot: &mut Vec<crate::data::SpcArea>| match res {
                    Ok(Ok(v)) => *slot = v,
                    Ok(Err(e)) => problems.push(format!("{name}: {e}")),
                    Err(e) => problems.push(format!("{name}: {e}")),
                };
                take("rain day 1", q1, &mut data.qpf[0]);
                take("rain 3-day", q3, &mut data.qpf[1]);
                take("rain 7-day", q7, &mut data.qpf[2]);
                take("excessive rain day 1", e1, &mut data.ero[0]);
                take("excessive rain day 2", e2, &mut data.ero[1]);
                take("excessive rain day 3", e3, &mut data.ero[2]);
                match r {
                    Ok(Ok(g)) => data.rivers = g,
                    Ok(Err(e)) => problems.push(format!("rivers: {e}")),
                    Err(e) => problems.push(format!("rivers: {e}")),
                }
                data.community = community;
                if hydro.is_some() {
                    data.hydrograph = hydro;
                }
                let now = chrono::Local::now();
                s.health.hydro = crate::data::FeedStatus {
                    ok: problems.is_empty(),
                    message: if problems.is_empty() {
                        format!("{} river gauges, rainfall and excessive rain outlooks", data.rivers.len())
                    } else {
                        problems.join("; ")
                    },
                    at: Some(now),
                };
                for p in &problems {
                    log::debug!("hydro feed: {p}");
                }
                s.data = std::sync::Arc::new(data);
                let snapshot = s.data.clone();
                s.compositor.update(|p| p.data = snapshot);
                cx.notify();
            });
        })
        .detach();
    }

    /// Every coverage town's NWS 7-day (home, watched spots, forecast regions) every 30 minutes,
    /// for the Regional Outlook graphic. Skipped while home is the only place.
    fn maybe_fetch_coverage(&mut self, cx: &mut Context<Self>) {
        let places = crate::social_graphics::coverage_places(&self.settings, &self.data.forecast_edits.regions);
        if places.len() < 2 {
            return;
        }
        let key = places.iter().map(|p| format!("{}@{:.3},{:.3}", if p.fips.is_empty() { &p.name } else { &p.fips }, p.lat, p.lon)).collect::<Vec<_>>().join("|");
        let due = self.ext.coverage_at.as_ref().is_none_or(|(t, last)| *last != key || t.elapsed() >= Duration::from_secs(30 * 60));
        if !due {
            return;
        }
        self.ext.coverage_at = Some((Instant::now(), key.clone()));
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(crate::social_graphics::coverage_forecast(places)).await;
            let _ = this.update(cx, |s, cx| match result {
                Ok(Ok(r)) => {
                    let mut data = (*s.data).clone();
                    data.coverage = Some(r);
                    s.data = std::sync::Arc::new(data);
                    let snapshot = s.data.clone();
                    s.compositor.update(|p| p.data = snapshot);
                    cx.notify();
                }
                Ok(Err(e)) => {
                    log::warn!("coverage forecasts: {e}");
                    // Try again in 5 minutes rather than 30.
                    s.ext.coverage_at = Some((Instant::now() - Duration::from_secs(25 * 60), key));
                }
                Err(_) => {}
            });
        })
        .detach();
    }

    /// NWS hourly forecast at home every 30 minutes (graphics' hour-by-hour and planners).
    fn maybe_fetch_home_hourly(&mut self, cx: &mut Context<Self>) {
        let home = self.settings.home;
        let key = (home.lat, home.lon);
        let due = self.ext.home_hourly_at.is_none_or(|(t, last)| last != key || t.elapsed() >= Duration::from_secs(30 * 60));
        if !due {
            return;
        }
        self.ext.home_hourly_at = Some((Instant::now(), key));
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(crate::homecast::fetch(home.lat, home.lon)).await;
            let _ = this.update(cx, |s, cx| {
                match result {
                    Ok(Ok(hours)) => {
                        let mut data = (*s.data).clone();
                        data.home_hourly = hours;
                        s.data = std::sync::Arc::new(data);
                        let snapshot = s.data.clone();
                        s.compositor.update(|p| p.data = snapshot);
                        cx.notify();
                    }
                    Ok(Err(e)) => {
                        log::warn!("home hourly forecast: {e}");
                        // Try again in 5 minutes rather than 30.
                        s.ext.home_hourly_at = Some((Instant::now() - Duration::from_secs(25 * 60), key));
                    }
                    Err(_) => {}
                }
            });
        })
        .detach();
    }

    fn maybe_fetch_climate(&mut self, cx: &mut Context<Self>) {
        let home = self.settings.home;
        let refresh_archive = self.settings.fetch_legacy_climate_history;
        let key = (home.lat, home.lon, refresh_archive);
        let due = self.ext.climate_at.is_none_or(|(t, last)| last != key || t.elapsed() >= Duration::from_secs(3 * 3600));
        if !due || self.ext.climate_loading {
            return;
        }
        self.ext.climate_loading = true;
        self.ext.climate_at = Some((Instant::now(), key));
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(crate::climate::fetch_all(home.lat, home.lon, refresh_archive)).await;
            let _ = this.update(cx, |s, cx| {
                s.ext.climate_loading = false;
                let current = s.settings.home;
                if key != (current.lat, current.lon, s.settings.fetch_legacy_climate_history) {
                    return;
                }
                let now = chrono::Local::now();
                let Ok((climate, problems)) = result else { return };
                for p in &problems {
                    log::debug!("climate feed: {p}");
                }
                s.health.climate = crate::data::FeedStatus {
                    ok: problems.is_empty(),
                    message: if problems.is_empty() {
                        match &climate.normals {
                            Some(n) if n.is_noaa() => format!("NOAA normals ({}), 16-day, models, aurora, airports", crate::climate::station_label(&n.station)),
                            Some(_) => "Normals (reanalysis cache), 16-day, models, aurora, airports".into(),
                            None => "16-day, models, aurora, airports".into(),
                        }
                    } else { problems.join("; ") },
                    at: Some(now),
                };
                let mut data = (*s.data).clone();
                data.climate = climate;
                s.data = std::sync::Arc::new(data);
                let snapshot = s.data.clone();
                s.compositor.update(|p| p.data = snapshot);
                s.check_model_discrepancy();
                cx.notify();
            });
        })
        .detach();
    }

    /// Once a day: say so when the models disagree on tomorrow by a lot.
    fn check_model_discrepancy(&mut self) {
        let tomorrow = chrono::Local::now().date_naive().succ_opt();
        if self.ext.models_warned_day == tomorrow {
            return;
        }
        let picks: Vec<(&str, f32, f32)> = self
            .data
            .climate
            .models
            .iter()
            .filter_map(|m| m.days.iter().find(|d| Some(d.0) == tomorrow).map(|d| (m.name, d.1, d.2)))
            .collect();
        if picks.len() < 2 {
            return;
        }
        let hi_spread = picks.iter().map(|p| p.1).fold(f32::MIN, f32::max) - picks.iter().map(|p| p.1).fold(f32::MAX, f32::min);
        let wet = picks.iter().filter(|p| p.2 >= 0.1).count();
        let rain_split = wet > 0 && wet < picks.len();
        if hi_spread >= 8.0 || rain_split {
            self.ext.models_warned_day = tomorrow;
            let detail = picks.iter().map(|(n, h, p)| format!("{n} {h:.0}° / {p:.2}\"")).collect::<Vec<_>>().join(", ");
            log::warn!("models disagree for tomorrow: {detail}");
            self.toast(ToastKind::Info, format!("Models disagree about tomorrow ({detail}). Lower confidence in the forecast."));
        }
    }

    /// Reload the community board (closings, centers) after it is edited.
    pub fn reload_community(&mut self) {
        let mut data = (*self.data).clone();
        data.community = crate::community::load();
        self.data = std::sync::Arc::new(data);
        let snapshot = self.data.clone();
        self.compositor.update(|p| p.data = snapshot);
    }

    /// Emergency interruption: put the warning's full-screen graphic on air now and, if a show
    /// is running, insert it as the next scene so the rundown resumes after it.
    pub fn break_in(&mut self, id: &str) {
        let Some(alert) = self.find_alert(id) else { return };
        self.mark_aired(id);
        let doc = crate::gfx::templates::warning_fullscreen(&alert);
        let Some(saved) = self.save_graphic(doc) else { return };
        if self.show_running {
            let at = self.show_pos.map(|p| p + 1).unwrap_or(0).min(self.show.scenes.len());
            let mut scene = crate::show::Scene::graphic(&saved);
            scene.title = format!("BREAK-IN: {}", alert.event);
            self.show.scenes.insert(at, scene);
            self.go_to_scene(at);
        } else {
            self.take_graphic(saved, Transition::Cut);
        }
        log::warn!("break-in: {} on air", alert.event);
        self.toast(ToastKind::Error, format!("Breaking in: {}", alert.event));
    }

    /// Strongest storm cells with their trend over the last 20 minutes:
    /// (summary, trend words, rising?).
    pub fn cell_trends(&self) -> Vec<(String, String, Option<bool>)> {
        let now = chrono::Utc::now().timestamp();
        let mut cells: Vec<&crate::data::StormCell> = self.data.cells.iter().collect();
        cells.sort_by(|a, b| b.max_dbz.total_cmp(&a.max_dbz));
        cells
            .into_iter()
            .take(6)
            .map(|c| {
                let summary = format!("{} {:.0} dBZ, {:.0} mi {}", c.on_air_label(self.settings.calm_storm_labels), c.max_dbz, c.distance_mi, c.direction);
                let hist = self.data.cell_history.get(&c.id);
                let area = self.data.cell_area_history.get(&c.id);
                let past = |h: Option<&Vec<(i64, f32)>>| h.and_then(|v| v.iter().find(|p| now - p.0 <= 25 * 60).copied());
                let trend = match (past(hist), past(area)) {
                    (Some((_, d0)), a0) => {
                        let dd = c.max_dbz as f32 - d0;
                        let da = a0.map(|(_, a)| c.area_sq_mi as f32 - a).unwrap_or(0.0);
                        if dd >= 3.0 || da >= 15.0 {
                            (format!("Strengthening (+{dd:.0} dBZ{})", if da > 0.0 { format!(", +{da:.0} sq mi") } else { String::new() }), Some(true))
                        } else if dd <= -3.0 || da <= -15.0 {
                            (format!("Weakening ({dd:.0} dBZ)"), Some(false))
                        } else {
                            ("Steady".to_string(), None)
                        }
                    }
                    _ => ("New".to_string(), None),
                };
                (summary, trend.0, trend.1)
            })
            .collect()
    }

    // ---- Diagnostics, archive, speech ------------------------------------------------------

    pub fn export_diagnostics(&mut self) {
        match ops::export_diagnostics(&self.settings) {
            Ok(path) => {
                self.toast(ToastKind::Success, format!("Diagnostics saved to {}", path.display()));
                let _ = open::that(path.parent().unwrap_or(&path));
            }
            Err(e) => self.toast(ToastKind::Error, format!("Could not save diagnostics: {e}")),
        }
    }

    /// Read an alert aloud (Windows speech).
    pub fn speak_alert(&mut self, id: &str) {
        let Some(a) = self.find_alert(id) else { return };
        let towns = a.towns();
        let text = format!(
            "{}. {}. {}. {}",
            a.event,
            if towns.is_empty() { a.area_desc.clone() } else { format!("Including {}", towns.join(", ")) },
            a.expires_label(self.settings.clock_24h).unwrap_or_default(),
            a.instruction
        );
        ops::speak(&text);
    }

    /// Read the forecast aloud (bulletin).
    pub fn speak_forecast(&mut self) {
        let Some(ov) = self.data.overview.as_ref() else {
            self.toast(ToastKind::Info, "No forecast loaded yet");
            return;
        };
        let mut text = format!("{} weather for {}. ", self.settings.station_name, self.settings.home_label);
        if let Some(t) = ov.current.temperature_f {
            text.push_str(&format!("Right now it is {t:.0} degrees. "));
        }
        for p in ov.forecast.iter().take(3) {
            text.push_str(&format!("{}: {} ", p.name, p.detailed_forecast));
        }
        ops::speak(&text);
    }
}

fn write_impressions(batch: std::collections::HashMap<String, f64>) {
    use std::io::Write as _;
    let path = crate::config::LibraryPaths::get().root.join("Sponsor impressions.csv");
    let new = !path.exists();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        if new {
            let _ = writeln!(f, "time,sponsor,seconds on air");
        }
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M");
        for (name, secs) in batch {
            let _ = writeln!(f, "{now},\"{}\",{secs:.0}", name.replace('"', "'"));
        }
    }
}
