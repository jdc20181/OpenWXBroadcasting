//! "Ready for air?" check: one look at everything that can go wrong on air (stale radar, a
//! stopped map, failed feeds, no output window, practice warnings left on, recent errors).

use gpui_kit::*;

use crate::{
    app::AppState,
    data::FeedStatus,
    devlog::{self, Level},
    ui::{c, col, muted, row, AMBER, BORDER, GREEN, RED, TEXT},
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    Problem,
    Check,
    Good,
}

pub struct Item {
    pub verdict: Verdict,
    pub name: &'static str,
    pub detail: String,
}

fn item(verdict: Verdict, name: &'static str, detail: impl Into<String>) -> Item {
    Item { verdict, name, detail: detail.into() }
}

/// A feed that should have answered in the last `fresh_min` minutes.
fn feed(name: &'static str, status: &FeedStatus, fresh_min: i64, required: bool) -> Item {
    let age = status.at.map(|t| (chrono::Local::now() - t).num_minutes());
    match (status.ok, age) {
        (true, Some(m)) if m <= fresh_min => item(Verdict::Good, name, format!("Updated {}", ago(m))),
        (true, Some(m)) => item(Verdict::Check, name, format!("Last update {}", ago(m))),
        (false, _) if !status.message.is_empty() => item(
            if required { Verdict::Problem } else { Verdict::Check },
            name,
            status.message.clone(),
        ),
        // An optional feed that never ran is switched off (Data feeds shows "off" too).
        (false, None) if !required => item(Verdict::Good, name, "Off"),
        _ => item(if required { Verdict::Problem } else { Verdict::Check }, name, "No data yet"),
    }
}

fn ago(minutes: i64) -> String {
    match minutes {
        m if m <= 0 => "just now".into(),
        1 => "1 minute ago".into(),
        m if m < 60 => format!("{m} minutes ago"),
        m => format!("{} hr {} min ago", m / 60, m % 60),
    }
}

pub fn checks(state: &AppState) -> Vec<Item> {
    let mut out = Vec::new();

    out.push(match state.radar_age_minutes() {
        Some(m) if m <= 10 => item(Verdict::Good, "Radar", format!("Newest frame {}", ago(m))),
        Some(m) if m <= 15 => item(Verdict::Check, "Radar", format!("Newest frame {} (getting old)", ago(m))),
        Some(m) => item(Verdict::Problem, "Radar", format!("Newest frame {}. Try Refresh radar (F5)", ago(m))),
        None => item(Verdict::Problem, "Radar", "No radar frames. Try Refresh radar (F5)"),
    });

    let engine = state.engine.status();
    out.push(if let Some(e) = engine.error {
        item(Verdict::Problem, "Map", format!("{e}. Try Refresh radar (F5)"))
    } else if engine.ready {
        item(Verdict::Good, "Map", "Renderer running")
    } else {
        item(Verdict::Check, "Map", "Renderer still starting")
    });

    out.push(feed("Warnings feed", &state.health.alerts, 10, true));
    out.push(feed("Current conditions", &state.health.overview, 20, true));
    out.push(feed("Storm cells", &state.health.cells, 15, false));
    out.push(feed("Lightning", &state.health.lightning, 15, false));
    out.push(feed("SPC outlooks", &state.health.spc, 90, false));
    out.push(feed("Model data", &state.health.grid, 90, false));

    if let Some(l) = state.lightning_near().filter(|l| l.all_clear_in > 0) {
        out.push(item(Verdict::Check, "Lightning", format!("{}. {}", l.summary(), l.all_clear_text())));
    }

    out.push(if state.test_alerts.is_empty() {
        item(Verdict::Good, "Practice warnings", "None on")
    } else {
        item(
            Verdict::Problem,
            "Practice warnings",
            format!("{} practice warning(s) still on. Clear them in Severe Center", state.test_alerts.len()),
        )
    });

    out.push(if state.output_window.is_some() {
        item(Verdict::Good, "Output window", "Open")
    } else {
        item(Verdict::Check, "Output window", "Closed. Press F10 to open it")
    });

    if state.settings.obs_enabled {
        let s = &state.settings;
        out.push(if s.obs_normal_scene.trim().is_empty() || s.obs_severe_scene.trim().is_empty() {
            item(Verdict::Check, "OBS", "Scene switching is on but a scene name is empty (Settings > Integrations > Broadcast control)")
        } else {
            item(
                Verdict::Good,
                "OBS",
                format!("Switches between \"{}\" and \"{}\"", s.obs_normal_scene, s.obs_severe_scene),
            )
        });
    }

    if !state.settings.sponsor_name.trim().is_empty() && state.settings.sponsor_logo_media.is_none() {
        out.push(item(Verdict::Check, "Sponsor", "Sponsor name set but no logo (Settings > Branding & sponsors)"));
    }

    let since = chrono::Local::now() - chrono::Duration::minutes(10);
    let recent: Vec<_> = devlog::snapshot()
        .into_iter()
        .filter(|e| e.level == Level::Error && e.at >= since)
        .collect();
    out.push(match recent.last() {
        None => item(Verdict::Good, "Errors", "None in the last 10 minutes"),
        Some(last) => item(
            Verdict::Check,
            "Errors",
            format!("{} in the last 10 minutes. Latest: {}", recent.len(), last.message),
        ),
    });

    out.sort_by_key(|i| i.verdict);
    out
}

/// System health: rendering, memory, stream, switcher, data quality and integrity.
pub fn system(state: &AppState) -> Vec<Item> {
    let mut out = Vec::new();
    let stats = state.compositor.stats();
    let target = if state.settings.low_resource { 15.0 } else { 30.0 };
    out.push(if stats.fps <= 0.0 {
        item(Verdict::Good, "Renderer", "Paused (nothing on screen needs frames)")
    } else if stats.fps >= target * 0.9 {
        item(Verdict::Good, "Renderer", format!("{:.0} fps, {:.1} ms per frame (worst {:.0} ms)", stats.fps, stats.render_ms, stats.worst_ms))
    } else {
        item(Verdict::Check, "Renderer", format!("{:.0} of {target:.0} fps, {:.1} ms per frame. Try Low-resource mode", stats.fps, stats.render_ms))
    });
    if let Some(mb) = state.ext.memory_mb {
        let limit = state.settings.memory_limit_mb.max(500) as f64;
        out.push(item(if mb > limit { Verdict::Check } else { Verdict::Good }, "Memory", format!("{mb:.0} MB of {limit:.0} MB")));
    }
    if state.settings.obs_enabled {
        out.push(match &state.ops2.obs {
            None => item(Verdict::Check, "OBS stream", "Not connected to OBS"),
            Some(h) if !h.streaming => item(Verdict::Good, "OBS stream", format!("Connected, not streaming ({:.0} fps, CPU {:.0}%)", h.fps, h.cpu)),
            Some(h) => {
                let dropped = if h.total_frames > 0 { h.skipped_frames as f64 / h.total_frames as f64 * 100.0 } else { 0.0 };
                let bad = h.congestion > 0.2 || dropped > 1.0 || (h.bitrate_kbps > 0.0 && h.bitrate_kbps < 500.0);
                item(
                    if bad { Verdict::Problem } else { Verdict::Good },
                    "OBS stream",
                    format!(
                        "Live {} • {:.0} kbps • {dropped:.1}% dropped • congestion {:.0}% • render lag {} • encoder lag {}{}",
                        h.stream_timecode,
                        h.bitrate_kbps,
                        h.congestion * 100.0,
                        h.render_skipped,
                        h.output_skipped,
                        h.audio_sync_ms.map(|ms| format!(" • audio offset {ms:+.0} ms")).unwrap_or_default()
                    ),
                )
            }
        });
        if let Some(peak) = state.ops2.audio_peak {
            out.push(item(if peak < -55.0 { Verdict::Problem } else { Verdict::Good }, "Audio", format!("Peak {peak:.0} dB")));
        }
    }
    if state.settings.obs_enabled || state.has_switcher() {
        out.push(match &state.ext.buses {
            Some((pgm, pvw)) => item(Verdict::Good, "Switcher", format!("Connected. Program: {pgm} • Preview: {pvw}")),
            None if state.settings.obs_enabled => item(Verdict::Check, "Switcher", "No answer from OBS"),
            None => item(Verdict::Good, "Switcher", "Companion / TriCaster set up (checked when pressed)"),
        });
    }
    for s in crate::sources::SOURCES {
        if let Some((q, st)) = crate::sources::quality(s) {
            out.push(item(
                if q >= 80 { Verdict::Good } else if q >= 50 { Verdict::Check } else { Verdict::Problem },
                s.name,
                format!(
                    "Quality {q}/100 • {} requests, {} failed, {} ms average{}",
                    st.requests,
                    st.failures,
                    st.avg_ms(),
                    if st.last_error.is_empty() { String::new() } else { format!(" • last error: {}", st.last_error) }
                ),
            ));
        }
    }
    if !state.data.analysis.lightning_quality.is_empty() {
        out.push(item(Verdict::Check, "Lightning coverage", state.data.analysis.lightning_quality.clone()));
    }
    let offline: Vec<String> = state
        .settings
        .webcams
        .iter()
        .filter(|c| crate::travel::cam_status(&c.name).last_ok.map(|t| chrono::Utc::now().timestamp() - t > 900).unwrap_or(true))
        .map(|c| c.name.clone())
        .collect();
    if !state.settings.webcams.is_empty() {
        out.push(if offline.is_empty() { item(Verdict::Good, "Cameras", format!("{} cameras updating", state.settings.webcams.len())) } else { item(Verdict::Check, "Cameras", format!("Not updating: {}", offline.join(", "))) });
    }
    let m = crate::audio::meter();
    if m.active {
        out.push(item(
            if m.peak_db > -1.0 || m.short_lufs > -12.0 { Verdict::Problem } else if m.short_lufs < -36.0 { Verdict::Check } else { Verdict::Good },
            "Audio input",
            format!("Peak {:.0} dBFS • loudness {:.0} LUFS (aim for about -24 to -16)", m.peak_db, m.short_lufs),
        ));
    }
    let problems = crate::sources::integrity(&state.data);
    out.push(if problems.is_empty() {
        item(Verdict::Good, "Data integrity", "All checks passed")
    } else {
        item(Verdict::Check, "Data integrity", problems.join(" • "))
    });
    if !state.ops2.cap_problems.is_empty() {
        out.push(item(Verdict::Check, "CAP messages", format!("{} message(s) failed validation (Settings > Weather & maps > CAP alerts)", state.ops2.cap_problems.len())));
    }
    let incidents = crate::broadcast_ops::incidents();
    let today = chrono::Utc::now().timestamp() - 86_400;
    let recent = incidents.iter().filter(|i| i.time >= today).count();
    out.push(item(if recent == 0 { Verdict::Good } else { Verdict::Check }, "Incidents", format!("{recent} in the last 24 hours")));
    out.sort_by_key(|i| i.verdict);
    out
}

/// Overall answer for the header and the status-bar button.
pub fn summary(items: &[Item]) -> (Verdict, String) {
    let problems = items.iter().filter(|i| i.verdict == Verdict::Problem).count();
    let checks = items.iter().filter(|i| i.verdict == Verdict::Check).count();
    match (problems, checks) {
        (0, 0) => (Verdict::Good, "Ready for air".into()),
        (0, n) => (Verdict::Check, format!("Ready, {n} thing(s) to look at")),
        (n, _) => (Verdict::Problem, format!("Not ready: {n} problem(s)")),
    }
}

pub fn colour(v: Verdict) -> u32 {
    match v {
        Verdict::Good => GREEN,
        Verdict::Check => AMBER,
        Verdict::Problem => RED,
    }
}

pub fn render(items: &[Item]) -> Div {
    let (verdict, text) = summary(items);
    col()
        .gap(px(2.))
        .child(
            row()
                .gap(px(10.))
                .pb(px(12.))
                .child(div().size(px(12.)).rounded(px(6.)).bg(c(colour(verdict))))
                .child(div().text_size(px(18.)).text_color(c(TEXT)).child(text)),
        )
        .children(items.iter().map(|i| {
            row()
                .gap(px(12.))
                .py(px(8.))
                .border_t_1()
                .border_color(c(BORDER))
                .child(div().size(px(9.)).rounded(px(5.)).bg(c(colour(i.verdict))))
                .child(div().w(px(160.)).text_size(px(14.)).text_color(c(TEXT)).child(i.name))
                .child(div().flex_1().min_w_0().child(muted(i.detail.clone())))
        }))
}
