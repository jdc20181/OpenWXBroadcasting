//! Forecaster desk: forecast projects, saved severe-weather analysis sessions, a timestamped
//! notebook, forecast decision history, weather event bookmarks and quick calculations.
//! Saved as desk.json next to the settings.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Project {
    pub name: String,
    /// "YYYY-MM-DD" or free text ("Friday 5 PM").
    pub due: String,
    /// "Planned", "Working" or "Done".
    pub status: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalysisSession {
    pub name: String,
    pub saved: i64,
    pub map: crate::map::scene::MapScene,
    /// Warnings selected / in coverage when saved (ids), for context.
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Entry {
    pub time: i64,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Bookmark {
    pub time: i64,
    pub label: String,
    /// Radar frame shown when bookmarked.
    pub radar_time: Option<i64>,
    pub view: crate::config::GeoView,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Desk {
    pub projects: Vec<Project>,
    pub sessions: Vec<AnalysisSession>,
    pub notebook: Vec<Entry>,
    pub decisions: Vec<Entry>,
    pub bookmarks: Vec<Bookmark>,
}

fn path() -> std::path::PathBuf {
    crate::config::config_dir().join("desk.json")
}

pub fn load() -> Desk {
    std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save(desk: &Desk) {
    if let Ok(json) = serde_json::to_string_pretty(desk) {
        let _ = std::fs::write(path(), json);
    }
}

/// Add a decision to the history (also called when a forecast is published).
pub fn log_decision(text: &str) {
    let mut d = load();
    d.decisions.push(Entry { time: chrono::Utc::now().timestamp(), text: text.to_string() });
    if d.decisions.len() > 500 {
        d.decisions.remove(0);
    }
    save(&d);
}

/// Quick meteorological calculations. Examples:
/// `hi 95 60` heat index (°F, %), `wc 10 20` wind chill (°F, mph), `td 80 65` dew point (°F, %),
/// `rh 80 65` humidity (°F, dew point °F), `c 72` / `f 22` temperature, `kt 40` / `mph 35` wind,
/// `mm 1.5` / `in 30` rain, `mb 29.92` / `inhg 1013` pressure.
pub fn calculate(input: &str) -> String {
    let parts: Vec<&str> = input.split_whitespace().collect();
    let Some(cmd) = parts.first().map(|c| c.to_lowercase()) else { return String::new() };
    let n = |i: usize| parts.get(i).and_then(|v| v.trim_end_matches(['°', '%', 'f', 'F']).parse::<f64>().ok());
    match (cmd.as_str(), n(1), n(2)) {
        ("hi", Some(t), Some(rh)) => {
            let hi = -42.379 + 2.049_015_23 * t + 10.143_331_27 * rh - 0.224_755_41 * t * rh - 0.006_837_83 * t * t - 0.054_817_17 * rh * rh + 0.001_228_74 * t * t * rh + 0.000_852_82 * t * rh * rh - 0.000_001_99 * t * t * rh * rh;
            let hi = if t < 80.0 { 0.5 * (t + 61.0 + (t - 68.0) * 1.2 + rh * 0.094) } else { hi };
            format!("Heat index {hi:.0}°F")
        }
        ("wc", Some(t), Some(v)) => {
            if t > 50.0 || v < 3.0 {
                format!("Wind chill {t:.0}°F (no chill above 50°F or under 3 mph)")
            } else {
                let wc = 35.74 + 0.6215 * t - 35.75 * v.powf(0.16) + 0.4275 * t * v.powf(0.16);
                format!("Wind chill {wc:.0}°F")
            }
        }
        ("td", Some(t), Some(rh)) => {
            let tc = (t - 32.0) * 5.0 / 9.0;
            let g = (rh.max(1.0) / 100.0).ln() + 17.62 * tc / (243.12 + tc);
            let td = 243.12 * g / (17.62 - g);
            format!("Dew point {:.0}°F", td * 9.0 / 5.0 + 32.0)
        }
        ("rh", Some(t), Some(td)) => {
            let (tc, dc) = ((t - 32.0) * 5.0 / 9.0, (td - 32.0) * 5.0 / 9.0);
            let e = |x: f64| (17.62 * x / (243.12 + x)).exp();
            format!("Humidity {:.0}%", (e(dc) / e(tc) * 100.0).min(100.0))
        }
        ("c", Some(f), _) => format!("{f:.1}°F = {:.1}°C", (f - 32.0) * 5.0 / 9.0),
        ("f", Some(c), _) => format!("{c:.1}°C = {:.1}°F", c * 9.0 / 5.0 + 32.0),
        ("kt", Some(mph), _) => format!("{mph:.0} mph = {:.0} kt", mph * 0.868_976),
        ("mph", Some(kt), _) => format!("{kt:.0} kt = {:.0} mph", kt / 0.868_976),
        ("mm", Some(inch), _) => format!("{inch:.2} in = {:.1} mm", inch * 25.4),
        ("in", Some(mm), _) => format!("{mm:.1} mm = {:.2} in", mm / 25.4),
        ("mb", Some(inhg), _) => format!("{inhg:.2} inHg = {:.1} mb", inhg * 33.8639),
        ("inhg", Some(mb), _) => format!("{mb:.1} mb = {:.2} inHg", mb / 33.8639),
        _ => "Try: hi 95 60 • wc 10 20 • td 80 65 • rh 80 65 • c 72 • f 22 • kt 40 • mph 35 • mm 1.5 • in 30 • mb 29.92 • inhg 1013".to_string(),
    }
}
