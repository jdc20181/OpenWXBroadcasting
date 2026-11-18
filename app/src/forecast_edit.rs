//! Forecaster edits: the station's own forecast on top of the model and NWS data.
//!
//! - Day overrides (high, low, sky, rain chance) feed every `{{dayN.*}}` graphic.
//! - Area adjustments change the model grid inside a circle or drawn polygon for a time window
//!   (with optional ramps in and out), so maps and town values show the edited forecast.
//! - Custom regions get their own values (school districts, county halves, lake areas).
//! - Edits are made in a draft, checked for consistency, compared with what is on air, and
//!   published; every publish is kept as a version, and drafts can be saved as scenarios.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DayOverride {
    /// Forecast day index (0 = today).
    pub index: usize,
    pub hi: Option<f32>,
    pub lo: Option<f32>,
    pub condition: Option<String>,
    pub pop: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Shape {
    Circle { lat: f64, lon: f64, radius_mi: f64 },
    /// (lat, lon) vertices.
    Polygon(Vec<(f64, f64)>),
}

impl Default for Shape {
    fn default() -> Self {
        Shape::Circle { lat: 0.0, lon: 0.0, radius_mi: 10.0 }
    }
}

impl Shape {
    /// 1 inside, 0 outside; circles feather over their outer 30%.
    pub fn weight(&self, lat: f64, lon: f64) -> f32 {
        match self {
            Shape::Circle { lat: clat, lon: clon, radius_mi } => {
                let d = crate::map::geo::haversine_km(*clat, *clon, lat, lon) * 0.621371;
                let r = radius_mi.max(0.5);
                if d <= r * 0.7 {
                    1.0
                } else if d >= r {
                    0.0
                } else {
                    (1.0 - (d - r * 0.7) / (r * 0.3)) as f32
                }
            }
            Shape::Polygon(pts) => {
                let ring: Vec<(f64, f64)> = pts.iter().map(|(la, lo)| (*lo, *la)).collect();
                if crate::data::point_in_ring(&ring, lon, lat) { 1.0 } else { 0.0 }
            }
        }
    }

    pub fn center(&self) -> (f64, f64) {
        match self {
            Shape::Circle { lat, lon, .. } => (*lat, *lon),
            Shape::Polygon(pts) if !pts.is_empty() => {
                let n = pts.len() as f64;
                (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n)
            }
            Shape::Polygon(_) => (0.0, 0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum EditVar {
    #[default]
    Temp,
    Dew,
    Wind,
    Gust,
    Cloud,
    /// Precipitation rate scale in percent (+50 = 50% wetter).
    Precip,
}

impl EditVar {
    pub const ALL: [EditVar; 6] = [EditVar::Temp, EditVar::Dew, EditVar::Wind, EditVar::Gust, EditVar::Cloud, EditVar::Precip];

    pub fn label(self) -> &'static str {
        match self {
            EditVar::Temp => "Temperature °",
            EditVar::Dew => "Dew point °",
            EditVar::Wind => "Wind mph",
            EditVar::Gust => "Gusts mph",
            EditVar::Cloud => "Cloud cover %",
            EditVar::Precip => "Precipitation %",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Adjustment {
    pub name: String,
    pub var: EditVar,
    pub delta: f32,
    pub shape: Shape,
    /// Hours from now the change applies (from..to).
    pub from_hour: f32,
    pub to_hour: f32,
    /// Ease in and out over 3 hours instead of switching on and off.
    pub ramp: bool,
}

impl Adjustment {
    /// Time weight for an hour offset from now.
    pub fn time_weight(&self, hour: f32) -> f32 {
        if hour < self.from_hour || hour > self.to_hour {
            return 0.0;
        }
        if !self.ramp {
            return 1.0;
        }
        let edge = 3.0f32.min((self.to_hour - self.from_hour) / 2.0).max(0.01);
        ((hour - self.from_hour) / edge).min((self.to_hour - hour) / edge).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Region {
    pub name: String,
    pub shape: Shape,
    /// Added to the model high / low for the region.
    pub hi_delta: f32,
    pub lo_delta: f32,
    /// Station wording for the region ("Lake-effect snow showers likely").
    pub note: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EditSet {
    pub days: Vec<DayOverride>,
    pub adjustments: Vec<Adjustment>,
    pub regions: Vec<Region>,
}

impl EditSet {
    pub fn day(&self, index: usize) -> Option<&DayOverride> {
        self.days.iter().find(|d| d.index == index)
    }

    pub fn day_mut(&mut self, index: usize) -> &mut DayOverride {
        if let Some(i) = self.days.iter().position(|d| d.index == index) {
            return &mut self.days[i];
        }
        self.days.push(DayOverride { index, ..Default::default() });
        self.days.last_mut().expect("just pushed")
    }

    pub fn is_empty(&self) -> bool {
        self.days.iter().all(|d| d.hi.is_none() && d.lo.is_none() && d.condition.is_none() && d.pop.is_none())
            && self.adjustments.is_empty()
            && self.regions.is_empty()
    }

    /// Differences from another set, in plain words (draft vs on air).
    pub fn diff(&self, other: &EditSet) -> Vec<String> {
        let mut out = Vec::new();
        for i in 0..7 {
            let (a, b) = (self.day(i).cloned().unwrap_or_default(), other.day(i).cloned().unwrap_or_default());
            let f = |v: Option<f32>| v.map(|x| format!("{x:.0}")).unwrap_or_else(|| "model".into());
            if a.hi != b.hi {
                out.push(format!("Day {i} high: {} → {}", f(b.hi), f(a.hi)));
            }
            if a.lo != b.lo {
                out.push(format!("Day {i} low: {} → {}", f(b.lo), f(a.lo)));
            }
            if a.pop != b.pop {
                out.push(format!("Day {i} rain chance: {} → {}", f(b.pop), f(a.pop)));
            }
            if a.condition != b.condition {
                out.push(format!(
                    "Day {i} sky: {} → {}",
                    b.condition.unwrap_or_else(|| "model".into()),
                    a.condition.unwrap_or_else(|| "model".into())
                ));
            }
        }
        if self.adjustments != other.adjustments {
            out.push(format!("Area adjustments: {} on air → {} in draft", other.adjustments.len(), self.adjustments.len()));
        }
        if self.regions != other.regions {
            out.push(format!("Regions: {} on air → {} in draft", other.regions.len(), self.regions.len()));
        }
        out
    }

    /// Things that look wrong: high below low, rain chance vs sky wording, snow with warm highs,
    /// big departures from the model.
    pub fn check(&self, model_days: &[crate::data::DaySummary]) -> Vec<String> {
        let mut out = Vec::new();
        for d in &self.days {
            let model = model_days.get(d.index);
            let hi = d.hi.or(model.and_then(|m| m.high).map(|v| v as f32));
            let lo = d.lo.or(model.and_then(|m| m.low).map(|v| v as f32));
            let cond = d.condition.clone().or(model.map(|m| m.short_forecast.clone())).unwrap_or_default().to_lowercase();
            let pop = d.pop.or(model.and_then(|m| m.precip).map(|v| v as f32)).unwrap_or(0.0);
            if let (Some(h), Some(l)) = (hi, lo) {
                if h < l {
                    out.push(format!("Day {}: high {h:.0} is below low {l:.0}", d.index));
                }
            }
            if pop >= 60.0 && (cond.contains("sunny") || cond.contains("clear")) {
                out.push(format!("Day {}: {pop:.0}% rain chance but the sky says \"{cond}\"", d.index));
            }
            if pop <= 10.0 && (cond.contains("rain") || cond.contains("storm") || cond.contains("snow")) {
                out.push(format!("Day {}: only {pop:.0}% rain chance but the sky says \"{cond}\"", d.index));
            }
            if cond.contains("snow") && hi.map(|h| h >= 45.0).unwrap_or(false) {
                out.push(format!("Day {}: snow with a high of {:.0}°", d.index, hi.unwrap_or(0.0)));
            }
            if let (Some(h), Some(mh)) = (d.hi, model.and_then(|m| m.high)) {
                if (h - mh as f32).abs() >= 10.0 {
                    out.push(format!("Day {}: high {h:.0} is {:.0}° from the NWS forecast", d.index, (h - mh as f32).abs()));
                }
            }
        }
        for a in &self.adjustments {
            if a.to_hour <= a.from_hour {
                out.push(format!("\"{}\": the time window ends before it starts", a.name));
            }
        }
        out
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ForecastEdits {
    pub draft: EditSet,
    pub published: EditSet,
    pub scenarios: Vec<(String, EditSet)>,
    /// (unix time, what changed)
    pub history: Vec<(i64, String)>,
}

fn dir() -> std::path::PathBuf {
    crate::config::LibraryPaths::get().root.join("Forecast edits")
}

pub fn load() -> ForecastEdits {
    std::fs::read_to_string(dir().join("current.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(e: &ForecastEdits) {
    let d = dir();
    let _ = std::fs::create_dir_all(&d);
    if let Ok(json) = serde_json::to_string_pretty(e) {
        let _ = std::fs::write(d.join("current.json"), json);
    }
}

impl ForecastEdits {
    pub fn log(&mut self, what: impl Into<String>) {
        self.history.push((chrono::Utc::now().timestamp(), what.into()));
        if self.history.len() > 500 {
            self.history.remove(0);
        }
    }

    /// Put the draft on air and keep a copy as a version.
    pub fn publish(&mut self) {
        let changes = self.draft.diff(&self.published);
        self.published = self.draft.clone();
        self.log(format!("Published ({} change(s))", changes.len()));
        let d = dir().join("versions");
        let _ = std::fs::create_dir_all(&d);
        if let Ok(json) = serde_json::to_string_pretty(&self.published) {
            let _ = std::fs::write(d.join(format!("{}.json", chrono::Local::now().format("%Y-%m-%d_%H%M%S"))), json);
        }
    }

    /// Published versions, newest first.
    pub fn versions() -> Vec<(String, std::path::PathBuf)> {
        let mut v: Vec<(String, std::path::PathBuf)> = std::fs::read_dir(dir().join("versions"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| Some((e.path().file_stem()?.to_string_lossy().to_string(), e.path())))
            .collect();
        v.sort_by(|a, b| b.0.cmp(&a.0));
        v
    }

    pub fn load_version(path: &std::path::Path) -> Option<EditSet> {
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }
}
