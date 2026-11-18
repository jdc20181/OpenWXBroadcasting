//! More broadcast operations on the app state (child module of `app`): warning acknowledgment,
//! assignment, delivery tracking, escalation and audit; workstation sync; CAP feeds; the timed
//! command queue; OBS stream / audio health and incidents; output auto-reopen; captions; and a
//! last-good data cache so graphics never go blank after a restart during an outage.

use std::time::{Duration, Instant};

use gpui_kit::Context;

use super::{AppState, ToastKind};
use crate::broadcast_ops as bo;

pub struct Ops2 {
    pub status: bo::WarningStatus,
    status_synced: Instant,
    escalated_at: Instant,
    cap_at: Option<Instant>,
    cap_loading: bool,
    /// CAP alerts from the feed / imported files, kept across NWS refreshes.
    pub cap_alerts: Vec<crate::data::Alert>,
    /// Validation problems of the last CAP messages (id, problems).
    pub cap_problems: Vec<(String, Vec<String>)>,
    schedule_at: Instant,
    obs_at: Instant,
    obs_loading: bool,
    pub obs: Option<bo::ObsHealth>,
    obs_last_bytes: Option<(u64, Instant)>,
    obs_last_skipped: u64,
    silent_since: Option<Instant>,
    audio_at: Instant,
    audio_loading: bool,
    pub audio_peak: Option<f32>,
    snapshot_saved: Instant,
    restored_cache: bool,
    output_was_needed: bool,
    pub caption: String,
    started_listeners: bool,
    pub rehearsing: bool,
}

impl Ops2 {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            status: Default::default(),
            status_synced: now,
            escalated_at: now,
            cap_at: None,
            cap_loading: false,
            cap_alerts: Vec::new(),
            cap_problems: Vec::new(),
            schedule_at: now,
            obs_at: now,
            obs_loading: false,
            obs: None,
            obs_last_bytes: None,
            obs_last_skipped: 0,
            silent_since: None,
            audio_at: now,
            audio_loading: false,
            audio_peak: None,
            snapshot_saved: now,
            restored_cache: false,
            output_was_needed: false,
            caption: String::new(),
            started_listeners: false,
            rehearsing: false,
        }
    }
}

impl Default for Ops2 {
    fn default() -> Self {
        Self::new()
    }
}

/// Last-good overview and alerts, saved so graphics have data right after a restart.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct DataCache {
    overview: Option<crate::data::Overview>,
    saved: i64,
}

fn cache_path() -> std::path::PathBuf {
    crate::config::config_dir().join("last-good-data.json")
}

impl AppState {
    pub fn ops2_tick(&mut self, cx: &mut Context<Self>) {
        if !self.ops2.started_listeners {
            self.ops2.started_listeners = true;
            self.ops2.status = bo::load_status(&self.settings.sync_folder);
            self.start_control_listeners();
            let days = self.settings.retention_days;
            crate::jobs::submit(crate::jobs::Priority::Low, "retention", move || {
                bo::apply_retention(days);
            });
        }
        if !self.ops2.restored_cache {
            self.ops2.restored_cache = true;
            self.restore_data_cache();
        }
        if self.ops2.status_synced.elapsed() >= Duration::from_secs(10) {
            self.ops2.status_synced = Instant::now();
            let folder = self.settings.sync_folder.clone();
            bo::save_status(&folder, &mut self.ops2.status);
        }
        self.escalate_unacknowledged();
        self.maybe_fetch_cap(cx);
        self.run_schedule();
        self.poll_obs_health(cx);
        self.watch_output();
        if self.ops2.snapshot_saved.elapsed() >= Duration::from_secs(300) {
            self.ops2.snapshot_saved = Instant::now();
            self.save_data_cache();
        }
    }

    fn start_control_listeners(&mut self) {
        // OSC, MOS and GPI-over-MIDI feed the same command channel as the control server.
        let (tx, rx) = std::sync::mpsc::channel();
        if self.settings.osc_enabled {
            bo::start_osc(self.settings.osc_port, tx.clone());
        }
        if self.settings.mos_enabled {
            bo::start_mos(self.settings.mos_port, self.settings.mos_id.clone(), tx.clone());
        }
        self.ext.loopback = Some(tx);
        self.ext.extra_rx = Some(rx);
    }

    // ---- Warnings: ack, assign, aired, escalation, audit ------------------------------------

    pub fn acknowledge_alert(&mut self, id: &str) {
        let who = self.settings.operator_name.clone();
        self.ops2.status.acks.insert(id.to_string(), (chrono::Utc::now().timestamp(), who.clone()));
        let name = self.find_alert(id).map(|a| format!("{} ({})", a.event, a.area_desc)).unwrap_or_else(|| id.to_string());
        bo::audit(&format!("ACKNOWLEDGED {name}"), &who);
        self.log(1, format!("Acknowledged: {name}"));
        let folder = self.settings.sync_folder.clone();
        bo::save_status(&folder, &mut self.ops2.status);
    }

    pub fn assign_alert(&mut self, id: &str, operator: &str) {
        self.ops2.status.assigned.insert(id.to_string(), operator.to_string());
        bo::audit(&format!("ASSIGNED {id} to {operator}"), &self.settings.operator_name);
        let folder = self.settings.sync_folder.clone();
        bo::save_status(&folder, &mut self.ops2.status);
    }

    /// Record that a warning went on air (call-out, graphic, break-in): delivery tracking.
    pub fn mark_aired(&mut self, id: &str) {
        if id.starts_with("TEST-") || self.ops2.status.aired.contains_key(id) {
            return;
        }
        self.ops2.status.aired.insert(id.to_string(), chrono::Utc::now().timestamp());
        let name = self.find_alert(id).map(|a| a.event).unwrap_or_default();
        bo::audit(&format!("ON AIR {name} ({id})"), &self.settings.operator_name);
        bo::fire_webhook(&self.settings.gpo_webhook, "warning_on_air", &name);
    }

    pub fn is_acknowledged(&self, id: &str) -> Option<(i64, String)> {
        self.ops2.status.acks.get(id).cloned()
    }

    /// Unacknowledged tornado warnings / emergencies sound again every 2 minutes.
    fn escalate_unacknowledged(&mut self) {
        if self.ops2.escalated_at.elapsed() < Duration::from_secs(120) {
            return;
        }
        self.ops2.escalated_at = Instant::now();
        let pending: Vec<crate::data::Alert> = self
            .coverage_warnings()
            .into_iter()
            .filter(|a| (a.event.to_lowercase().contains("tornado") || a.threat_tag().is_some()) && !self.ops2.status.acks.contains_key(&a.id))
            .collect();
        if pending.is_empty() {
            return;
        }
        if self.settings.alert_sounds && !(self.in_quiet_hours() && !self.settings.quiet_allow_urgent) {
            crate::obs::play_alert_sound(true);
        }
        let names = pending.iter().map(|a| a.event.clone()).collect::<Vec<_>>().join(", ");
        self.toast(ToastKind::Error, format!("Not yet acknowledged: {names}"));
        bo::notify("Warning not acknowledged", &names, &self.settings.notify_webhook);
    }

    // ---- CAP feed -------------------------------------------------------------------------

    fn maybe_fetch_cap(&mut self, cx: &mut Context<Self>) {
        let url = self.settings.cap_feed_url.trim().to_string();
        if url.is_empty() || self.ops2.cap_loading {
            return;
        }
        if self.ops2.cap_at.map(|t| t.elapsed() < Duration::from_secs(120)).unwrap_or(false) {
            return;
        }
        self.ops2.cap_loading = true;
        self.ops2.cap_at = Some(Instant::now());
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(crate::cap::fetch_feed(url)).await;
            let _ = this.update(cx, |s, cx| {
                s.ops2.cap_loading = false;
                if let Ok(Ok(messages)) = result {
                    s.apply_cap(messages);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Import CAP messages (feed or file): validate, check the sender, apply Alert / Update / Cancel.
    pub fn apply_cap(&mut self, messages: Vec<crate::cap::CapMessage>) {
        let lang = if self.settings.cap_language.is_empty() { "en".to_string() } else { self.settings.cap_language.clone() };
        for m in messages {
            let problems = crate::cap::validate(&m);
            if !problems.is_empty() {
                log::warn!("CAP {} has problems: {}", m.identifier, problems.join("; "));
                self.ops2.cap_problems.retain(|(id, _)| *id != m.identifier);
                self.ops2.cap_problems.push((m.identifier.clone(), problems));
            }
            if !crate::cap::trusted_sender(&m.sender, &self.settings.trusted_senders) {
                log::warn!("CAP {} from an unverified sender ({}) was not shown", m.identifier, m.sender);
                bo::audit(&format!("CAP REJECTED unverified sender {} ({})", m.sender, m.identifier), &self.settings.operator_name);
                continue;
            }
            if m.status != "Actual" {
                continue;
            }
            // Updates and cancels replace or remove what they reference.
            for r in &m.references {
                self.ops2.cap_alerts.retain(|a| a.id != format!("CAP-{r}"));
            }
            if m.msg_type == "Cancel" {
                bo::audit(&format!("CAP CANCEL {}", m.identifier), &self.settings.operator_name);
                continue;
            }
            if let Some(alert) = crate::cap::to_alert(&m, &lang) {
                self.ops2.cap_alerts.retain(|a| a.id != alert.id);
                crate::ops::archive_alert(&alert);
                self.ops2.cap_alerts.push(alert);
            }
        }
        // Drop expired ones and merge into the map / local lists.
        let now = chrono::Utc::now();
        self.ops2.cap_alerts.retain(|a| a.expires.as_deref().and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok()).map(|t| t > now).unwrap_or(true));
        let mut data = (*self.data).clone();
        data.map_alerts.retain(|a| !a.id.starts_with("CAP-"));
        data.map_alerts.extend(self.ops2.cap_alerts.iter().cloned());
        self.data = std::sync::Arc::new(data);
        let snapshot = self.data.clone();
        self.compositor.update(|p| p.data = snapshot);
    }

    pub fn import_cap_file(&mut self, path: std::path::PathBuf) {
        match std::fs::read_to_string(&path).map_err(anyhow::Error::from).and_then(|t| crate::cap::parse(&t)) {
            Ok(m) => {
                let problems = crate::cap::validate(&m);
                let id = m.identifier.clone();
                self.apply_cap(vec![m]);
                self.toast(
                    if problems.is_empty() { ToastKind::Success } else { ToastKind::Info },
                    if problems.is_empty() { format!("CAP message {id} imported") } else { format!("CAP message {id} imported with {} problem(s): {}", problems.len(), problems.join("; ")) },
                );
            }
            Err(e) => self.toast(ToastKind::Error, format!("Not a CAP message: {e}")),
        }
    }

    // ---- Timed command queue --------------------------------------------------------------

    fn run_schedule(&mut self) {
        if self.ops2.schedule_at.elapsed() < Duration::from_millis(250) || self.settings.schedule.is_empty() {
            return;
        }
        self.ops2.schedule_at = Instant::now();
        let now = chrono::Local::now();
        let hm = now.format("%H:%M").to_string();
        let hms = now.format("%H:%M:%S").to_string();
        let today = now.format("%Y-%m-%d").to_string();
        let mut due = Vec::new();
        let mut settings = self.settings.clone();
        for item in settings.schedule.iter_mut() {
            // "HH:MM:SS" fires on that second (time-synchronized playback); "HH:MM" within the minute.
            let hit = if item.at.len() == 8 { item.at <= hms && item.at[..5] == hms[..5] } else { item.at == hm };
            if hit && item.last_run != today {
                item.last_run = today.clone();
                due.push(item.clone());
            }
        }
        if due.is_empty() {
            return;
        }
        settings.schedule.retain(|i| i.daily || i.last_run.is_empty());
        self.settings = settings;
        let _ = self.settings.save();
        for item in due {
            bo::log_control("schedule", &format!("{} {} {}", item.at, item.action, item.arg));
            if let Some(cmd) = bo::action_command(&item.action, &item.arg) {
                if let Some(tx) = &self.ext.loopback {
                    let _ = tx.send(cmd);
                }
            }
        }
    }

    // ---- OBS stream / audio health, incidents ----------------------------------------------

    fn poll_obs_health(&mut self, cx: &mut Context<Self>) {
        if !self.settings.obs_enabled || self.ops2.obs_loading || self.ops2.obs_at.elapsed() < Duration::from_secs(5) {
            return;
        }
        self.ops2.obs_loading = true;
        self.ops2.obs_at = Instant::now();
        let (url, pw, mic) = (self.settings.obs_url.clone(), self.settings.obs_password.clone(), self.settings.obs_mic_input.clone());
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(bo::obs_health(url, pw, mic)).await;
            let _ = this.update(cx, |s, cx| {
                s.ops2.obs_loading = false;
                let Ok(Ok((mut h, bytes))) = result else {
                    if s.ops2.obs.take().is_some() {
                        bo::log_incident("OBS connection lost", "OBS stopped answering the WebSocket");
                    }
                    return;
                };
                if let Some((prev, at)) = s.ops2.obs_last_bytes {
                    let secs = at.elapsed().as_secs_f64().max(0.5);
                    h.bitrate_kbps = (bytes.saturating_sub(prev)) as f64 * 8.0 / 1000.0 / secs;
                }
                s.ops2.obs_last_bytes = Some((bytes, Instant::now()));
                // Dropped frames: more than 1% new skipped frames since the last poll.
                let new_skipped = h.skipped_frames.saturating_sub(s.ops2.obs_last_skipped);
                if h.streaming && new_skipped > 30 {
                    bo::log_incident("Dropped frames", &format!("{new_skipped} frames skipped in 5 s (congestion {:.0}%)", h.congestion * 100.0));
                }
                s.ops2.obs_last_skipped = h.skipped_frames;
                if h.streaming && h.bitrate_kbps > 0.0 && h.bitrate_kbps < 500.0 {
                    bo::log_incident("Low bitrate", &format!("{:.0} kbps", h.bitrate_kbps));
                }
                crate::shows::set_live(h.streaming);
                s.ops2.obs = Some(h);
                cx.notify();
            });
        })
        .detach();
        // Audio silence: sample OBS meters for 3 s every 15 s while streaming.
        let streaming = self.ops2.obs.as_ref().map(|h| h.streaming).unwrap_or(false);
        if streaming && !self.ops2.audio_loading && self.ops2.audio_at.elapsed() >= Duration::from_secs(15) {
            self.ops2.audio_loading = true;
            self.ops2.audio_at = Instant::now();
            let (url, pw) = (self.settings.obs_url.clone(), self.settings.obs_password.clone());
            cx.spawn(async move |this, cx| {
                let peak = crate::net::spawn(bo::obs_audio_peak(url, pw, 3)).await.ok().and_then(|r| r.ok()).flatten();
                let _ = this.update(cx, |s, cx| {
                    s.ops2.audio_loading = false;
                    s.ops2.audio_peak = peak;
                    let silent = peak.map(|p| p < -55.0).unwrap_or(false);
                    match (silent, s.ops2.silent_since) {
                        (true, None) => s.ops2.silent_since = Some(Instant::now()),
                        (true, Some(t)) if t.elapsed() >= Duration::from_secs(30) => {
                            bo::log_incident("Audio silence", "No audio above -55 dB for 30 s while streaming");
                            bo::notify("Broadcast audio is silent", "No audio for 30 seconds", &s.settings.notify_webhook);
                            s.toast(ToastKind::Error, "Broadcast audio has been silent for 30 seconds");
                            s.ops2.silent_since = Some(Instant::now());
                        }
                        (false, _) => s.ops2.silent_since = None,
                        _ => {}
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// Reopen the output window if it closes while a show runs or a recording is on.
    fn watch_output(&mut self) {
        let needed = self.show_running || self.recording.is_some();
        if needed && self.output_window.is_none() && self.ops2.output_was_needed && !self.want_output {
            bo::log_incident("Output window closed", "Reopened automatically during a show / recording");
            self.want_output = true;
            self.toast(ToastKind::Info, "The output window closed during the show; it was reopened");
        }
        self.ops2.output_was_needed = needed && self.output_window.is_some();
    }

    // ---- Continuity rehearsal ------------------------------------------------------------

    /// Practise an outage: backup scene, incident, notification, then back after 20 seconds.
    pub fn rehearse_outage(&mut self) {
        self.ops2.rehearsing = true;
        self.set_backup_scene(true);
        bo::log_incident("Rehearsal", "Continuity rehearsal started (backup scene on air)");
        bo::notify("Continuity rehearsal", "Backup scene is on air for 20 seconds", &self.settings.notify_webhook);
        self.toast(ToastKind::Info, "Rehearsal: backup scene on air for 20 seconds, then back to the map");
    }

    pub fn rehearsal_tick(&mut self) {
        // Called from the backup-scene timer in ext_tick.
        if self.ops2.rehearsing && self.ext.backup_on && self.ext.backup_since.map(|t| t.elapsed() >= Duration::from_secs(20)).unwrap_or(false) {
            self.ops2.rehearsing = false;
            self.set_backup_scene(false);
            bo::log_incident("Rehearsal", "Continuity rehearsal finished");
        }
    }

    // ---- Hardware switchers -----------------------------------------------------------------

    /// Press a Companion button ("page/row/column").
    pub fn switcher_press(&mut self, location: &str, cx: &mut Context<Self>) {
        let base = self.settings.companion_url.clone();
        let loc = location.to_string();
        bo::log_control("switcher", &format!("Companion {loc}"));
        cx.spawn(async move |this, cx| {
            let result = crate::net::spawn(bo::companion_press(base, loc)).await;
            let _ = this.update(cx, |s, cx| {
                match result {
                    Ok(Ok(m)) => log::info!("{m}"),
                    Ok(Err(e)) => s.toast(ToastKind::Error, format!("Switcher: {e}")),
                    Err(e) => s.toast(ToastKind::Error, format!("Switcher: {e}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Cut or auto transition through the configured controller, or use OBS's selected transition.
    pub fn switcher_transition(&mut self, auto: bool, cx: &mut Context<Self>) {
        let loc = if auto { self.settings.companion_auto.clone() } else { self.settings.companion_cut.clone() };
        if !loc.trim().is_empty() {
            self.switcher_press(&loc, cx);
            return;
        }
        if !self.settings.tricaster_url.trim().is_empty() {
            let base = self.settings.tricaster_url.clone();
            let shortcut = if auto { "main_auto" } else { "main_take" }.to_string();
            bo::log_control("switcher", &format!("TriCaster {shortcut}"));
            cx.spawn(async move |this, cx| {
                let result = crate::net::spawn(bo::tricaster(base, shortcut)).await;
                let _ = this.update(cx, |s, cx| {
                    if let Ok(Err(e)) = &result {
                        s.toast(ToastKind::Error, format!("TriCaster: {e}"));
                    }
                    cx.notify();
                });
            })
            .detach();
            return;
        }
        if self.settings.obs_enabled && self.settings.obs_studio_mode {
            self.obs_transition(cx);
            return;
        }
        self.toast(ToastKind::Info, "Set up a switcher in Settings → Integrations");
    }

    pub fn has_switcher(&self) -> bool {
        self.has_external_switcher() || (self.settings.obs_enabled && self.settings.obs_studio_mode)
    }

    pub fn has_external_switcher(&self) -> bool {
        let s = &self.settings;
        !s.companion_cut.trim().is_empty() || !s.companion_auto.trim().is_empty() || !s.tricaster_url.trim().is_empty()
    }

    // ---- Captions --------------------------------------------------------------------------

    pub fn set_caption(&mut self, text: &str) {
        let fixed = bo::correct_caption(text.trim());
        self.ops2.caption = fixed.clone();
        let (pos, size, on) = (self.settings.caption_position.clone(), self.settings.caption_size, self.settings.captions_enabled);
        self.compositor.update(|p| {
            p.caption = if on && !fixed.is_empty() { Some((fixed, pos == "top", size)) } else { None };
        });
    }

    // ---- Last-good data cache ---------------------------------------------------------------

    fn save_data_cache(&self) {
        if let Some(ov) = self.data.overview.clone() {
            let cache = DataCache { overview: Some(ov), saved: chrono::Utc::now().timestamp() };
            std::thread::spawn(move || {
                if let Ok(json) = serde_json::to_string(&cache) {
                    let _ = std::fs::write(cache_path(), json);
                }
            });
        }
    }

    fn restore_data_cache(&mut self) {
        if self.data.overview.is_some() {
            return;
        }
        let Some(cache) = std::fs::read_to_string(cache_path()).ok().and_then(|t| serde_json::from_str::<DataCache>(&t).ok()) else { return };
        if let Some(ov) = cache.overview {
            let mut data = (*self.data).clone();
            data.overview = Some(ov);
            self.data = std::sync::Arc::new(data);
            let snapshot = self.data.clone();
            self.compositor.update(|p| p.data = snapshot);
            log::info!("showing the last saved forecast until fresh data arrives");
        }
    }
}
