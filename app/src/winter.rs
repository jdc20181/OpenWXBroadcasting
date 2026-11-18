//! Winter Weather Studio data: saved winter maps (snow totals, ice, storm potential) for one
//! state, with an editable legend (bands), model snowfall/ice fetched from Open-Meteo for any
//! model, and county overrides painted by hand. Graphics draw a map through a `winter:<id>`
//! risk-map layer, so edits here show up in every graphic that uses it.

use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock, RwLock},
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::grid::GridModel;

/// US states and DC: (county FIPS prefix, postal code, name).
pub const STATES: &[(&str, &str, &str)] = &[
    ("01", "AL", "Alabama"), ("04", "AZ", "Arizona"), ("05", "AR", "Arkansas"), ("06", "CA", "California"),
    ("08", "CO", "Colorado"), ("09", "CT", "Connecticut"), ("10", "DE", "Delaware"), ("11", "DC", "District of Columbia"),
    ("12", "FL", "Florida"), ("13", "GA", "Georgia"), ("16", "ID", "Idaho"), ("17", "IL", "Illinois"),
    ("18", "IN", "Indiana"), ("19", "IA", "Iowa"), ("20", "KS", "Kansas"), ("21", "KY", "Kentucky"),
    ("22", "LA", "Louisiana"), ("23", "ME", "Maine"), ("24", "MD", "Maryland"), ("25", "MA", "Massachusetts"),
    ("26", "MI", "Michigan"), ("27", "MN", "Minnesota"), ("28", "MS", "Mississippi"), ("29", "MO", "Missouri"),
    ("30", "MT", "Montana"), ("31", "NE", "Nebraska"), ("32", "NV", "Nevada"), ("33", "NH", "New Hampshire"),
    ("34", "NJ", "New Jersey"), ("35", "NM", "New Mexico"), ("36", "NY", "New York"), ("37", "NC", "North Carolina"),
    ("38", "ND", "North Dakota"), ("39", "OH", "Ohio"), ("40", "OK", "Oklahoma"), ("41", "OR", "Oregon"),
    ("42", "PA", "Pennsylvania"), ("44", "RI", "Rhode Island"), ("45", "SC", "South Carolina"), ("46", "SD", "South Dakota"),
    ("47", "TN", "Tennessee"), ("48", "TX", "Texas"), ("49", "UT", "Utah"), ("50", "VT", "Vermont"),
    ("51", "VA", "Virginia"), ("53", "WA", "Washington"), ("54", "WV", "West Virginia"), ("55", "WI", "Wisconsin"),
    ("56", "WY", "Wyoming"),
];

pub fn state_name(fips: &str) -> &'static str {
    STATES.iter().find(|s| s.0 == fips).map(|s| s.2).unwrap_or("Indiana")
}

pub fn state_code(fips: &str) -> &'static str {
    STATES.iter().find(|s| s.0 == fips).map(|s| s.1).unwrap_or("IN")
}

/// What a winter map shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WinterKind {
    #[default]
    Snow,
    Ice,
    Potential,
    /// Snowfall bands with freezing-rain glaze hatched on top (CWXB-83 item 5).
    SnowIce,
}

impl WinterKind {
    pub const ALL: [WinterKind; 4] = [WinterKind::Snow, WinterKind::Ice, WinterKind::SnowIce, WinterKind::Potential];

    pub fn label(self) -> &'static str {
        match self {
            Self::Snow => "Snowfall forecast",
            Self::Ice => "Ice forecast",
            Self::Potential => "Snow potential (model amounts)",
            Self::SnowIce => "Snow and ice forecast",
        }
    }

    /// The model field filling the map: snowfall (in) or freezing-rain liquid (in).
    pub fn uses_ice(self) -> bool {
        self == Self::Ice
    }

    /// Ice glaze is read as well and hatched over the snow bands.
    pub fn with_ice(self) -> bool {
        self == Self::SnowIce
    }

    /// Default legend, highest first. Snow bands use the TWC snow colours (LibreWXR palette 4)
    /// the snow-total maps use at each band's starting amount (checked by a test); TWC has no
    /// ice colours, so ice keeps its pinks.
    pub fn default_bands(self) -> Vec<Band> {
        let b = |label: &str, color: &str, min: f32| Band { label: label.into(), color: color.into(), min };
        match self {
            Self::Snow | Self::SnowIce => vec![
                b("8\"+", "#1F7FFF", 8.0),
                b("5-8\"", "#4FAFFF", 5.0),
                b("3-5\"", "#6FDFFF", 3.0),
                b("1-3\"", "#9FFFFF", 1.0),
                b("Up to 1\"", "#CFFFFF", 0.1),
            ],
            Self::Ice => vec![
                b("Over 0.25\"", "#C2185B", 0.25),
                b("0.10-0.25\"", "#E5679A", 0.10),
                b("Glaze", "#F6B8D0", 0.01),
            ],
            Self::Potential => vec![
                b("Heavy (6\"+)", "#3F9FFF", 6.0),
                b("Moderate (3-6\")", "#6FDFFF", 3.0),
                b("Light (1-3\")", "#9FFFFF", 1.0),
            ],
        }
    }

    fn default_title(self) -> &'static str {
        match self {
            Self::Snow => "Snowfall Forecast",
            Self::Ice => "Ice Forecast",
            Self::Potential => "Snow Potential",
            Self::SnowIce => "Snow and Ice Forecast",
        }
    }
}

/// Glaze colour for ice hatching on snow-and-ice maps (and its legend row).
pub const ICE_HATCH: &str = "#E5679A";
/// Ice at or above this (inches) is hatched.
pub const ICE_HATCH_MIN: f32 = 0.01;

/// A hand-drawn area on the map ("Flakes possible", "Accumulation possible"): a closed shape
/// in lon/lat, filled in its colour with its label in the middle (CWXB-83 item 8).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Zone {
    pub label: String,
    pub color: String,
    /// Corners in lon/lat; drawn as a smooth closed curve through them.
    pub points: Vec<(f64, f64)>,
    /// Still being drawn (shown with its corners, not closed for good yet).
    pub open: bool,
}

/// Only the counties within `km` of a point (the home area), instead of whole states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Focus {
    pub lon: f64,
    pub lat: f64,
    pub km: f64,
}

/// Distance in km between two lon/lat points.
pub fn km_between(a: (f64, f64), b: (f64, f64)) -> f64 {
    let k = ((a.1 + b.1) / 2.0).to_radians().cos();
    (((a.0 - b.0) * k).powi(2) + (a.1 - b.1).powi(2)).sqrt() * 111.2
}

/// State FIPS prefixes in a map's `state` ("18" or "18,21,17").
pub fn state_list(state: &str) -> Vec<String> {
    let v: Vec<String> = state.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    if v.is_empty() { vec!["18".into()] } else { v }
}

/// "Indiana" or "IN, KY and IL" for a map's states.
pub fn region_name(state: &str) -> String {
    let list = state_list(state);
    if list.len() == 1 {
        return state_name(&list[0]).to_string();
    }
    let codes: Vec<&str> = list.iter().map(|s| state_code(s)).collect();
    format!("{} and {}", codes[..codes.len() - 1].join(", "), codes[codes.len() - 1])
}

/// Postal codes of a map's states ("IN, KY, IL"), for the state box.
pub fn region_codes(state: &str) -> String {
    state_list(state).iter().map(|s| state_code(s)).collect::<Vec<_>>().join(", ")
}

/// Parse the state box ("IN", "Indiana", "IN, KY, IL") into FIPS prefixes joined by commas.
pub fn parse_states(text: &str) -> Option<String> {
    let find = |part: &str| STATES.iter().find(|s| s.1 == part.to_uppercase() || s.2.eq_ignore_ascii_case(part)).map(|s| s.0);
    let mut out: Vec<&str> = Vec::new();
    for part in text.split([',', ';', '/']).map(str::trim).filter(|p| !p.is_empty()) {
        // "New York" is one state; "IN KY" is two.
        let found: Vec<&str> = match find(part) {
            Some(f) => vec![f],
            None => part.split_whitespace().map(find).collect::<Option<Vec<_>>>()?,
        };
        for f in found {
            if !out.contains(&f) {
                out.push(f);
            }
        }
    }
    (!out.is_empty()).then(|| out.join(","))
}

/// Is a county (state FIPS, centre) part of a map region?
pub fn in_region(state: &str, focus: Option<Focus>, county_state: &str, centre: (f64, f64)) -> bool {
    state_list(state).iter().any(|s| s == county_state) && focus.is_none_or(|f| km_between(centre, (f.lon, f.lat)) <= f.km)
}

/// One legend row: values at or above `min` (and below the next band up) get this colour.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Band {
    pub label: String,
    pub color: String,
    pub min: f32,
}

/// Model values on a regular lat/lon grid over the state (row 0 = north).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WinterGrid {
    pub model: GridModel,
    /// The window: from `start` hours after the model was read, for `hours` hours.
    pub start: u32,
    pub hours: u32,
    pub fetched_at: i64,
    pub north: f64,
    pub south: f64,
    pub west: f64,
    pub east: f64,
    pub nx: usize,
    pub ny: usize,
    pub values: Vec<f32>,
    /// Freezing-rain glaze (in) on the same nodes, for snow-and-ice maps (else empty).
    pub ice: Vec<f32>,
    /// Set when the values are not one model's (e.g. "Average of GFS, NAM and ECMWF").
    pub note: String,
}

impl WinterGrid {
    /// Bilinear value at a point (None outside the grid).
    pub fn sample(&self, lon: f64, lat: f64) -> Option<f32> {
        self.sample_in(&self.values, lon, lat)
    }

    /// Bilinear ice glaze at a point (None without ice values).
    pub fn sample_ice(&self, lon: f64, lat: f64) -> Option<f32> {
        self.sample_in(&self.ice, lon, lat)
    }

    fn sample_in(&self, values: &[f32], lon: f64, lat: f64) -> Option<f32> {
        if self.nx < 2 || self.ny < 2 || values.len() != self.nx * self.ny {
            return None;
        }
        let gx = (lon - self.west) / (self.east - self.west) * (self.nx - 1) as f64;
        let gy = (self.north - lat) / (self.north - self.south) * (self.ny - 1) as f64;
        if !(0.0..=(self.nx - 1) as f64).contains(&gx) || !(0.0..=(self.ny - 1) as f64).contains(&gy) {
            return None;
        }
        let (x0, y0) = ((gx.floor() as usize).min(self.nx - 2), (gy.floor() as usize).min(self.ny - 2));
        let (fx, fy) = ((gx - x0 as f64) as f32, (gy - y0 as f64) as f32);
        let v = |x: usize, y: usize| values[y * self.nx + x];
        let top = v(x0, y0) * (1.0 - fx) + v(x0 + 1, y0) * fx;
        let bottom = v(x0, y0 + 1) * (1.0 - fx) + v(x0 + 1, y0 + 1) * fx;
        Some(top * (1.0 - fy) + bottom * fy)
    }

    /// The mean of several model grids for the same window (CWXB-83 item 3 "consensus"), on
    /// the first grid's nodes; each node averages the models that cover it.
    pub fn average(grids: &[&WinterGrid]) -> Option<WinterGrid> {
        let first = *grids.first()?;
        let node = |j: usize, i: usize| {
            (first.west + (first.east - first.west) * i as f64 / (first.nx - 1).max(1) as f64, first.north - (first.north - first.south) * j as f64 / (first.ny - 1).max(1) as f64)
        };
        let mean = |pick: &dyn Fn(&WinterGrid, f64, f64) -> Option<f32>| -> Vec<f32> {
            (0..first.ny)
                .flat_map(|j| (0..first.nx).map(move |i| (j, i)))
                .map(|(j, i)| {
                    let (lon, lat) = node(j, i);
                    let vals: Vec<f32> = grids.iter().filter_map(|g| pick(g, lon, lat)).collect();
                    if vals.is_empty() { 0.0 } else { vals.iter().sum::<f32>() / vals.len() as f32 }
                })
                .collect()
        };
        let values = mean(&|g, lon, lat| g.sample(lon, lat));
        let ice = if grids.iter().all(|g| !g.ice.is_empty()) { mean(&|g, lon, lat| g.sample_ice(lon, lat)) } else { Vec::new() };
        let names: Vec<&str> = grids.iter().map(|g| g.model.label()).collect();
        let note = match names.len() {
            1 => format!("Model: {}", names[0]),
            n => format!("Average of {} and {}", names[..n - 1].join(", "), names[n - 1]),
        };
        Some(WinterGrid { values, ice, note, fetched_at: grids.iter().map(|g| g.fetched_at).min().unwrap_or(first.fetched_at), ..first.clone() })
    }
}

/// A saved winter map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WinterMap {
    pub id: String,
    pub kind: WinterKind,
    /// State FIPS prefix ("18" = Indiana).
    pub state: String,
    pub title: String,
    pub subtitle: String,
    /// Legend heading, e.g. "Through Friday 11/13".
    pub period: String,
    pub bands: Vec<Band>,
    pub model: GridModel,
    /// The window starts this many hours from now (0 = now; "Tonight", "Tomorrow"… in the
    /// studio set it to the next such time).
    pub start: u32,
    /// Forecast window length, hours.
    pub hours: u32,
    /// Smooth model contours (off: each county takes the value at its centre).
    pub smooth: bool,
    /// Town placards with each town's total.
    pub blips: bool,
    /// Placards show the band name ("3-5\"") instead of the model number.
    pub blip_bands: bool,
    /// How many towns get a placard.
    pub towns: u32,
    /// Graphic look, "modern" or "classic" (new maps take the station's default look).
    pub look: String,
    /// Hand-set county values (FIPS → value in the map's unit), over the model.
    pub overrides: BTreeMap<String, f32>,
    pub grid: Option<WinterGrid>,
    /// The map this one updates ("First call" → "Update"), by id; empty for an original.
    /// The county list then shows each county's previous amount and the change.
    pub based_on: String,
    /// Hand-drawn areas on top of the fill ("Flakes possible").
    pub zones: Vec<Zone>,
    /// Only the home area (counties within this distance of home), not whole states.
    pub focus: Option<Focus>,
}

impl Default for WinterMap {
    fn default() -> Self {
        Self::new(WinterKind::Snow, "18")
    }
}

impl WinterMap {
    pub fn new(kind: WinterKind, state: &str) -> Self {
        let hours = 48;
        Self {
            id: crate::gfx::model::new_id(),
            kind,
            state: state.into(),
            title: kind.default_title().into(),
            subtitle: String::new(),
            period: period_text(hours),
            bands: kind.default_bands(),
            // NOAA GFS: read directly, no Open-Meteo.
            model: GridModel::Gfs,
            start: 0,
            hours,
            smooth: true,
            blips: true,
            blip_bands: kind == WinterKind::Potential,
            towns: 10,
            look: crate::gfx::look::default(),
            overrides: BTreeMap::new(),
            grid: None,
            based_on: String::new(),
            zones: Vec::new(),
            focus: None,
        }
    }

    /// Ice glaze at a point on a snow-and-ice map (None on other kinds or without ice).
    pub fn ice_at(&self, lon: f64, lat: f64) -> Option<f32> {
        if !self.kind.with_ice() {
            return None;
        }
        self.grid.as_ref().and_then(|g| g.sample_ice(lon, lat))
    }

    /// The map's area name: "Indiana", "IN, KY and IL", or "Indiana (home area)".
    pub fn area_name(&self) -> String {
        let name = region_name(&self.state);
        match self.focus {
            Some(f) => format!("{name}, {} mi around home", (f.km / 1.609).round()),
            None => name,
        }
    }

    /// A copy to update, keeping this map as it was (CWXB-83 "first call" → "updated").
    pub fn make_update(&self) -> WinterMap {
        let mut m = self.clone();
        m.id = crate::gfx::model::new_id();
        m.based_on = self.id.clone();
        m.subtitle = format!("Updated {}", chrono::Local::now().format("%a %-I %p"));
        m
    }

    /// The map's value at a point: a painted county wins, else the model.
    pub fn value(&self, fips: Option<&str>, lon: f64, lat: f64) -> Option<f32> {
        if let Some(v) = fips.and_then(|f| self.overrides.get(f)) {
            return Some(*v);
        }
        self.grid.as_ref().and_then(|g| g.sample(lon, lat))
    }

    /// The band a value falls in (bands are kept highest first).
    pub fn band_for(&self, v: f32) -> Option<&Band> {
        self.bands.iter().filter(|b| v >= b.min).max_by(|a, b| a.min.total_cmp(&b.min))
    }

    /// A calm note when the model has loaded and nothing reaches the lowest band anywhere
    /// (CWXB-84: an ice map with no ice showed a full legend over a grey state).
    pub fn empty_note(&self) -> Option<&'static str> {
        let grid = self.grid.as_ref().filter(|g| !g.values.is_empty())?;
        let lowest = self.bands.iter().map(|b| b.min).fold(f32::INFINITY, f32::min);
        let any = grid.values.iter().chain(self.overrides.values()).any(|v| v.is_finite() && *v >= lowest);
        let ice = self.kind.with_ice() && grid.ice.iter().any(|v| *v >= ICE_HATCH_MIN);
        if any || ice {
            return None;
        }
        Some(match self.kind {
            WinterKind::Ice => "No icing expected in this window.",
            WinterKind::SnowIce => "No accumulating snow or ice expected in this window.",
            _ => "No accumulating snow expected in this window.",
        })
    }

    /// Placard text for a value.
    pub fn blip_text(&self, v: f32) -> Option<String> {
        if self.blip_bands {
            return self.band_for(v).map(|b| b.label.clone());
        }
        self.band_for(v)?;
        Some(match self.kind {
            WinterKind::Ice => format!("{v:.2}\""),
            _ if v < 1.0 => "<1\"".to_string(),
            _ => format!("{:.0}\"", v.round()),
        })
    }

    /// Bottom-line credit.
    pub fn source_text(&self) -> String {
        let span = |start: u32, hours: u32| if start == 0 { format!("next {hours} h") } else { format!("{start}-{} h ahead", start + hours) };
        match &self.grid {
            Some(g) if !g.note.is_empty() => format!("{}, {}", g.note, span(g.start, g.hours)),
            Some(g) if g.model.is_noaa() => format!("Model: NOAA {}, {}", g.model.label(), span(g.start, g.hours)),
            Some(g) => format!("Model: {} via Open-Meteo, {}", g.model.label(), span(g.start, g.hours)),
            // Not loaded yet: credit the model that is loading, not a bare "Forecast".
            None if self.model.is_noaa() => format!("Model: NOAA {}, {}", self.model.label(), span(self.start, self.hours)),
            None => format!("Model: {} via Open-Meteo, {}", self.model.label(), span(self.start, self.hours)),
        }
    }

    /// County list for the website and social posts (CWXB-83): "County,Value,Band" rows for
    /// the map's state, highest first, value at each county's centre (or its painted value).
    /// `counties`: (name, state FIPS, county FIPS, (lon, lat) centre).
    /// With `previous` (the map this one updates), two more columns: Previous and Change.
    pub fn county_csv<'a>(&self, counties: impl IntoIterator<Item = (&'a str, &'a str, &'a str, (f64, f64))>, previous: Option<&WinterMap>) -> String {
        let unit = if self.kind.uses_ice() { "in ice" } else { "in snow" };
        let multi = state_list(&self.state).len() > 1;
        let mut rows: Vec<(String, f32, Option<f32>, Option<f32>)> = counties
            .into_iter()
            .filter(|c| in_region(&self.state, self.focus, c.1, c.3))
            .filter_map(|(name, st, fips, (lon, lat))| {
                // Several states: "Dubois, IN" so same-named counties stay apart.
                let name = if multi { format!("{name}, {}", state_code(st)) } else { name.to_string() };
                Some((name, self.value(Some(fips), lon, lat)?, previous.and_then(|p| p.value(Some(fips), lon, lat)), self.ice_at(lon, lat)))
            })
            .collect();
        rows.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let quote = |s: &str| if s.contains([',', '"']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
        let num = |v: f32| if self.kind.uses_ice() { format!("{v:.2}") } else { format!("{v:.1}") };
        let ice_col = self.kind.with_ice();
        let mut out = format!("County,Value ({unit}),Band{}{}\n", if ice_col { ",Ice (in)" } else { "" }, if previous.is_some() { ",Previous,Change" } else { "" });
        for (name, v, before, ice) in rows {
            let band = self.band_for(v).map(|b| b.label.clone()).unwrap_or_else(|| "Below lowest band".into());
            out += &format!("{},{},{}", quote(&name), num(v), quote(&band));
            if ice_col {
                out += &format!(",{}", ice.map(|i| format!("{:.2}", i.max(0.0))).unwrap_or_default());
            }
            if previous.is_some() {
                match before {
                    Some(b) => out += &format!(",{},{}{}", num(b), if v >= b { "+" } else { "" }, num(v - b)),
                    None => out += ",,",
                }
            }
            out.push('\n');
        }
        out
    }

    /// The finished zones as a Graphics Studio "Forecast zones" graphic (Codex's
    /// `set_forecast_zones` sets, docs/codex/SET-DATA.md): (template key, data overrides).
    /// `reviewed` must come from the forecaster: unreviewed zones are not drawn by that graphic.
    /// `day`: days from today the window starts (0 today, 1 tomorrow, 2+ day 3).
    pub fn zones_graphic(&self, reviewed: bool, day: i64, bbox: Option<(f64, f64, f64, f64)>) -> Option<(String, Vec<(String, String)>)> {
        let zones: Vec<&Zone> = self.zones.iter().filter(|z| !z.open && z.points.len() >= 3).collect();
        if zones.is_empty() {
            return None;
        }
        let period = match day {
            ..=0 => "today",
            1 => "tomorrow",
            _ => "day3",
        };
        // One state, whole: the state view; several states or the home area: the regional view.
        let regional = self.focus.is_some() || state_list(&self.state).len() > 1;
        let scope = if regional { "region" } else { "state" };
        let key = format!("set_forecast_zones__{period}_{scope}");
        let prefix = format!("set.zones.{period}.{scope}");
        // Legend: one category per distinct label, in drawing order.
        let mut legend: Vec<(String, String)> = Vec::new();
        for z in &zones {
            let label = if z.label.trim().is_empty() { "Zone".to_string() } else { z.label.trim().chars().take(60).collect() };
            if !legend.iter().any(|(l, _)| *l == label) {
                legend.push((label, z.color.clone()));
            }
        }
        let rings: Vec<serde_json::Value> = zones
            .iter()
            .map(|z| {
                let label: String = if z.label.trim().is_empty() { "Zone".to_string() } else { z.label.trim().chars().take(60).collect() };
                let category = legend.iter().position(|(l, _)| *l == label).unwrap_or(0);
                serde_json::json!({ "category": category, "ring": z.points, "label": label, "label_at": null })
            })
            .collect();
        let payload = serde_json::json!({
            "reviewed": reviewed,
            "scope": {
                "states": state_list(&self.state),
                "county": null,
                "bbox": if regional { bbox.map(|(w, s, e, n)| vec![w, s, e, n]) } else { None },
            },
            "palette": "category",
            "legend_title": "Forecast zones",
            "legend": legend.iter().map(|(label, color)| serde_json::json!({ "label": label, "color": color })).collect::<Vec<_>>(),
            "panels": [{ "title": self.title, "valid": self.period, "zones": rings }],
        });
        let overrides = vec![
            (format!("{prefix}.payload"), payload.to_string()),
            (format!("{prefix}.source"), "Source: Forecaster analysis".to_string()),
            (format!("{prefix}.subtitle"), format!("{} • {}", self.area_name(), self.period)),
        ];
        Some((key, overrides))
    }

    /// Keep bands highest first.
    pub fn sort_bands(&mut self) {
        self.bands.sort_by(|a, b| b.min.total_cmp(&a.min));
    }
}

/// "Through Saturday night 11/14" for a window of `hours` from now (CWXB-84: a window
/// ending just after midnight is the night before, not the next day).
pub fn period_text(hours: u32) -> String {
    period_from(chrono::Local::now().naive_local(), hours)
}

fn period_from(now: chrono::NaiveDateTime, hours: u32) -> String {
    let (day, part) = day_part(now + chrono::Duration::hours(hours as i64));
    format!("Through {}{part} {}", day.format("%A"), day.format("%-m/%-d"))
}

/// The day and part of day a time belongs to (after midnight is still the night before).
fn day_part(t: chrono::NaiveDateTime) -> (chrono::NaiveDate, &'static str) {
    use chrono::Timelike;
    match t.hour() {
        0..=5 => (t.date() - chrono::Duration::days(1), " night"),
        6..=11 => (t.date(), " morning"),
        12..=17 => (t.date(), " afternoon"),
        _ => (t.date(), " evening"),
    }
}

/// Legend heading for a window starting `start` hours from now: "Through Saturday night
/// 11/14" when it starts now, else "Saturday night through Monday morning".
pub fn window_text(start: u32, hours: u32) -> String {
    window_from(chrono::Local::now().naive_local(), start, hours)
}

fn window_from(now: chrono::NaiveDateTime, start: u32, hours: u32) -> String {
    if start == 0 {
        return period_from(now, hours);
    }
    let from = now + chrono::Duration::hours(start as i64);
    let (d0, p0) = day_part(from);
    let (d1, p1) = day_part(from + chrono::Duration::hours(hours as i64));
    format!("{}{p0} through {}{p1}", d0.format("%A"), d1.format("%A"))
}

/// Start choices for the studio: (label, hours from `now` to that time). "Tonight" is 7 PM,
/// "Tomorrow" 7 AM, "Tomorrow night" 7 PM, "Day 3" 7 AM the day after tomorrow. Times already
/// passed are left out (after 7 PM, "Tonight" is now).
pub fn start_choices() -> Vec<(&'static str, u32)> {
    start_choices_from(chrono::Local::now().naive_local())
}

fn start_choices_from(now: chrono::NaiveDateTime) -> Vec<(&'static str, u32)> {
    let at = |days: i64, h: u32| (now.date() + chrono::Duration::days(days)).and_hms_opt(h, 0, 0).unwrap();
    let mut out = vec![("Now", 0)];
    for (label, t) in [("Tonight", at(0, 19)), ("Tomorrow", at(1, 7)), ("Tomorrow night", at(1, 19)), ("Day 3", at(2, 7))] {
        let h = (t - now).num_minutes();
        if h > 30 {
            out.push((label, ((h + 30) / 60) as u32));
        }
    }
    out
}

#[cfg(test)]
mod period_tests {
    #[test]
    fn window_names_the_real_end() {
        let at = |d: u32, h: u32| chrono::NaiveDate::from_ymd_opt(2026, 11, d).unwrap().and_hms_opt(h, 25, 0).unwrap();
        // Friday 12:25 AM + 48 h ends 12:25 AM Sunday: that's Saturday night.
        assert_eq!(super::period_from(at(13, 0), 48), "Through Saturday night 11/14");
        assert_eq!(super::period_from(at(13, 9), 24), "Through Saturday morning 11/14");
        assert_eq!(super::period_from(at(13, 20), 72), "Through Monday evening 11/16");
    }

    #[test]
    fn windows_that_start_later() {
        let at = |d: u32, h: u32| chrono::NaiveDate::from_ymd_opt(2026, 11, d).unwrap().and_hms_opt(h, 0, 0).unwrap();
        // Friday 10 PM: tonight already started, so the first later start is Saturday 7 AM.
        let choices = super::start_choices_from(at(13, 22));
        assert_eq!(choices, vec![("Now", 0), ("Tomorrow", 9), ("Tomorrow night", 21), ("Day 3", 33)]);
        // Saturday 7 PM for 36 h ends Monday 7 AM.
        assert_eq!(super::window_from(at(13, 22), 21, 36), "Saturday evening through Monday morning");
        assert_eq!(super::window_from(at(13, 22), 0, 24), "Through Saturday evening 11/14");
        // Friday 9 AM: Tonight is 10 h away.
        assert_eq!(super::start_choices_from(at(13, 9))[1], ("Tonight", 10));
    }
}

// ---------------------------------------------------------------------------------------------
// Store (winter-maps.json in the config folder)
// ---------------------------------------------------------------------------------------------

fn store() -> &'static RwLock<Arc<Vec<WinterMap>>> {
    static S: OnceLock<RwLock<Arc<Vec<WinterMap>>>> = OnceLock::new();
    S.get_or_init(|| {
        let maps = std::fs::read_to_string(path()).ok().and_then(|s| serde_json::from_str::<Vec<WinterMap>>(&s).ok()).unwrap_or_default();
        RwLock::new(Arc::new(maps))
    })
}

fn path() -> std::path::PathBuf {
    crate::config::config_dir().join("winter-maps.json")
}

pub fn all() -> Arc<Vec<WinterMap>> {
    store().read().map(|s| s.clone()).unwrap_or_default()
}

/// The legend of the most recent map of a kind, so a new map keeps the bands the forecaster
/// set up last time (CWXB-83 item 12).
pub fn last_bands(kind: WinterKind) -> Option<Vec<Band>> {
    all().iter().find(|m| m.kind == kind && !m.bands.is_empty()).map(|m| m.bands.clone())
}

pub fn get(id: &str) -> Option<WinterMap> {
    all().iter().find(|m| m.id == id).cloned()
}

/// Save (insert or replace) a map, and write the file.
pub fn save(map: &WinterMap) {
    let Ok(mut s) = store().write() else { return };
    let mut maps = (**s).clone();
    match maps.iter_mut().find(|m| m.id == map.id) {
        Some(slot) => *slot = map.clone(),
        None => maps.insert(0, map.clone()),
    }
    *s = Arc::new(maps.clone());
    write(&maps);
}

pub fn delete(id: &str) {
    let Ok(mut s) = store().write() else { return };
    let maps: Vec<WinterMap> = s.iter().filter(|m| m.id != id).cloned().collect();
    *s = Arc::new(maps.clone());
    write(&maps);
}

fn write(maps: &[WinterMap]) {
    if cfg!(test) {
        return;
    }
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(maps) {
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &p);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Model fetch
// ---------------------------------------------------------------------------------------------

/// A county's centre (mean of its outline points).
pub fn county_centre(rings: &[Vec<(f64, f64)>]) -> (f64, f64) {
    let n = rings.iter().map(|r| r.len()).sum::<usize>().max(1) as f64;
    let (sx, sy) = rings.iter().flatten().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    (sx / n, sy / n)
}

/// The lon/lat box of a map region (one or more states, optionally only the home area).
pub fn region_bbox(state: &str, focus: Option<Focus>) -> Option<(f64, f64, f64, f64)> {
    let counties = crate::map::boundaries::counties_named()?;
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for c in counties.iter().filter(|c| in_region(state, focus, &c.state, county_centre(&c.rings))) {
        for (lon, lat) in c.rings.iter().flatten() {
            b = (b.0.min(*lon), b.1.min(*lat), b.2.max(*lon), b.3.max(*lat));
        }
    }
    (b.0 < b.2).then_some(b)
}

/// Snowfall (10:1, inches) or freezing-rain totals for the next `hours` over the state. GFS is
/// read from NOAA's GRIB files on a dense grid; the other models come from Open-Meteo.
pub async fn fetch(state: String, focus: Option<Focus>, kind: WinterKind, model: GridModel, start: u32, hours: u32) -> Result<WinterGrid> {
    // On a first run the county outlines are still downloading when Winter Studio opens: wait
    // for them (up to 2 minutes) instead of failing until "Reload model" is pressed.
    let mut bbox = region_bbox(&state, focus);
    for _ in 0..60 {
        if bbox.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        bbox = region_bbox(&state, focus);
    }
    let (west, south, east, north) = bbox.ok_or_else(|| anyhow!("county outlines are not downloaded yet (check the connection, then Reload model)"))?;
    let (west, south, east, north) = (west - 0.15, south - 0.15, east + 0.15, north + 0.15);
    let k = ((south + north) / 2.0).to_radians().cos();
    let aspect = ((east - west) * k / (north - south)).clamp(0.3, 4.0);
    // The window must end within the model's reach: a later start shortens it.
    let reach = model.max_hours().max(6);
    let start = start.min(reach.saturating_sub(6));
    let hours = hours.clamp(6, (reach - start).max(6));
    if model.is_noaa() && crate::grid::noaa_fits(model, (north, south, west, east)) {
        match fetch_noaa((north, south, west, east), aspect, kind, model, start, hours).await {
            Ok(grid) => return Ok(grid),
            // HRRR from NOAA needs the updated relay; until then HRRR uses its previous path.
            Err(e) if model == GridModel::Hrrr => log::warn!("NOAA HRRR winter grid: {e:#}; using the previous HRRR source"),
            Err(e) => return Err(e),
        }
    }
    let target = 180.0_f64;
    let ny = ((target / aspect).sqrt().round() as usize).clamp(6, 24);
    let nx = ((target / ny as f64).round() as usize).clamp(6, 30);
    let mut points = Vec::with_capacity(nx * ny);
    for j in 0..ny {
        for i in 0..nx {
            points.push((north - (north - south) * j as f64 / (ny - 1) as f64, west + (east - west) * i as f64 / (nx - 1) as f64));
        }
    }
    let hourly = if kind.uses_ice() { "precipitation,weather_code" } else if kind.with_ice() { "snowfall,precipitation,weather_code" } else { "snowfall" };
    let total = start + hours;
    let mut tasks = Vec::new();
    for chunk in points.chunks(100) {
        let lats = chunk.iter().map(|p| format!("{:.3}", p.0)).collect::<Vec<_>>().join(",");
        let lons = chunk.iter().map(|p| format!("{:.3}", p.1)).collect::<Vec<_>>().join(",");
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={lats}&longitude={lons}&hourly={hourly}&forecast_hours={total}&precipitation_unit=inch&timeformat=unixtime&timezone=GMT{}",
            model.api_id_pub().map(|id| format!("&models={id}")).unwrap_or_default()
        );
        tasks.push(crate::net::spawn(async move {
            let response = crate::net::get_logged(&url, std::time::Duration::from_secs(40)).await?;
            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                return Err(anyhow!("Open-Meteo daily limit reached (429), try again later"));
            }
            Ok::<serde_json::Value, anyhow::Error>(response.json().await?)
        }));
    }
    let mut locations = Vec::new();
    for t in tasks {
        match t.await.map_err(|e| anyhow!("winter fetch: {e}"))?? {
            serde_json::Value::Array(items) => locations.extend(items),
            other => locations.push(other),
        }
    }
    if locations.len() != points.len() {
        return Err(anyhow!("Open-Meteo returned {} of {} points", locations.len(), points.len()));
    }
    // Hours before the window's start are dropped.
    let nums = |v: &serde_json::Value, key: &str| -> Vec<f32> {
        v["hourly"][key].as_array().map(|a| a.iter().skip(start as usize).map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()).unwrap_or_default()
    };
    // Freezing rain / drizzle hours (WMO 56, 57, 66, 67): their liquid becomes ice.
    let ice_of = |loc: &serde_json::Value| -> f32 {
        let p = nums(loc, "precipitation");
        let w = nums(loc, "weather_code");
        p.iter().zip(w.iter()).filter(|(_, c)| matches!(**c as i32, 56 | 57 | 66 | 67)).map(|(p, _)| *p).sum()
    };
    let values = locations
        .iter()
        .map(|loc| {
            if kind.uses_ice() {
                ice_of(loc)
            } else {
                // Open-Meteo's snowfall is at 7:1 (inches with precipitation_unit=inch); scale to
                // the 10:1 the NOAA path and broadcast totals use.
                nums(loc, "snowfall").iter().sum::<f32>() * 10.0 / 7.0
            }
        })
        .collect();
    let ice = if kind.with_ice() { locations.iter().map(ice_of).collect() } else { Vec::new() };
    Ok(WinterGrid { model, start, hours, fetched_at: chrono::Utc::now().timestamp(), north, south, west, east, nx, ny, values, ice, note: if model.is_noaa() { format!("Model: {} via Open-Meteo", model.label()) } else { String::new() } })
}

/// The NOAA path of [`fetch`]: a dense GFS grid over the state (NOAA has no per-point cost).
async fn fetch_noaa(bounds: (f64, f64, f64, f64), aspect: f64, kind: WinterKind, model: GridModel, start: u32, hours: u32) -> Result<WinterGrid> {
    use crate::grid::Var;
    let (north, south, west, east) = bounds;
    let target = 700.0_f64;
    let ny = ((target / aspect).sqrt().round() as usize).clamp(8, 40);
    let nx = ((target / ny as f64).round() as usize).clamp(8, 50);
    let grid = crate::grid::fetch_box(bounds, nx, ny, model, start + hours + 2).await?;
    let last = (grid.hours() - 1) as f32;
    let now = (grid.time_index(0.0) + start as f32).min(last);
    let end = (now + hours as f32).min(last);
    let mut values = Vec::with_capacity(nx * ny);
    let mut ice = Vec::new();
    // Liquid in hours typed freezing rain (WMO 66 / 67) becomes ice.
    let ice_at = |j: usize, i: usize| -> f32 {
        let first = now.ceil() as usize;
        (first..=end.floor() as usize)
            .filter(|&t| matches!(grid.node(Var::Code, t as f32, j, i) as i32, 56 | 57 | 66 | 67))
            .map(|t| grid.node(Var::Precip, t as f32, j, i))
            .sum()
    };
    for j in 0..ny {
        for i in 0..nx {
            values.push(if kind.uses_ice() { ice_at(j, i) } else { (grid.node(Var::Snow10Acc, end, j, i) - grid.node(Var::Snow10Acc, now, j, i)).max(0.0) });
            if kind.with_ice() {
                ice.push(ice_at(j, i));
            }
        }
    }
    Ok(WinterGrid { model, start, hours, fetched_at: chrono::Utc::now().timestamp(), north, south, west, east, nx, ny, values, ice, note: String::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Default snow and potential bands are the snow-total map's TWC colour at their amount.
    #[test]
    fn default_bands_use_twc_snow_colours() {
        for kind in [WinterKind::Snow, WinterKind::Potential] {
            for band in kind.default_bands() {
                let [r, g, b] = crate::grid::snow_total_rgb(band.min);
                assert_eq!(band.color, format!("#{r:02X}{g:02X}{b:02X}"), "{}", band.label);
            }
        }
    }

    #[test]
    fn regions_and_states() {
        assert_eq!(parse_states("IN").as_deref(), Some("18"));
        assert_eq!(parse_states("in, ky, IL").as_deref(), Some("18,21,17"));
        assert_eq!(parse_states("IN KY").as_deref(), Some("18,21"));
        assert_eq!(parse_states("New York").as_deref(), Some("36"));
        assert_eq!(parse_states("Indiana, Ohio").as_deref(), Some("18,39"));
        assert_eq!(parse_states("IN, XX"), None);
        assert_eq!(parse_states(" "), None);
        assert_eq!(region_name("18"), "Indiana");
        assert_eq!(region_name("18,21,17"), "IN, KY and IL");
        assert_eq!(region_codes("18,21"), "IN, KY");
        // Jasper, IN to Evansville is about 70 km; Indianapolis is about 165 km.
        let home = Focus { lon: -86.93, lat: 38.39, km: 97.0 };
        assert!(in_region("18,21", Some(home), "18", (-87.56, 37.97)));
        assert!(!in_region("18,21", Some(home), "18", (-86.16, 39.77)));
        assert!(!in_region("18", None, "21", (-87.0, 38.0)));
    }

    #[test]
    fn zones_become_a_forecast_zones_graphic() {
        let mut m = WinterMap::new(WinterKind::Potential, "18");
        assert!(m.zones_graphic(true, 0, None).is_none());
        let ring = vec![(-87.0, 38.0), (-86.0, 38.0), (-86.5, 39.0)];
        m.zones = vec![
            Zone { label: "Flakes possible".into(), color: "#9FD8FF".into(), points: ring.clone(), open: false },
            Zone { label: "Accumulation possible".into(), color: "#3F8FFF".into(), points: ring.clone(), open: false },
            Zone { label: "Flakes possible".into(), color: "#9FD8FF".into(), points: ring.clone(), open: false },
            // Still being drawn: left out.
            Zone { label: "Heavy".into(), color: "#000".into(), points: ring.clone(), open: true },
        ];
        let (key, ov) = m.zones_graphic(true, 1, None).unwrap();
        assert_eq!(key, "set_forecast_zones__tomorrow_state");
        assert_eq!(ov[0].0, "set.zones.tomorrow.state.payload");
        let p: serde_json::Value = serde_json::from_str(&ov[0].1).unwrap();
        assert_eq!(p["reviewed"], true);
        assert_eq!(p["legend"].as_array().unwrap().len(), 2);
        assert_eq!(p["panels"][0]["zones"].as_array().unwrap().len(), 3);
        assert_eq!(p["panels"][0]["zones"][2]["category"], 0);
        assert_eq!(p["scope"]["bbox"], serde_json::Value::Null);
        // The home area is a regional view with its box.
        m.focus = Some(Focus { lon: -86.9, lat: 38.4, km: 97.0 });
        let (key, ov) = m.zones_graphic(false, 0, Some((-88.0, 37.5, -86.0, 39.0))).unwrap();
        assert_eq!(key, "set_forecast_zones__today_region");
        let p: serde_json::Value = serde_json::from_str(&ov[0].1).unwrap();
        assert_eq!(p["reviewed"], false);
        assert_eq!(p["scope"]["bbox"][0], -88.0);
        assert!(crate::gfx::templates::build(&key).is_some());
    }

    #[test]
    fn average_of_models() {
        let a = WinterGrid { model: GridModel::Gfs, nx: 2, ny: 2, north: 1.0, south: 0.0, west: 0.0, east: 1.0, values: vec![2.0, 2.0, 4.0, 4.0], ..Default::default() };
        let b = WinterGrid { model: GridModel::Nam, values: vec![4.0, 4.0, 8.0, 8.0], ..a.clone() };
        let avg = WinterGrid::average(&[&a, &b]).unwrap();
        assert_eq!(avg.values, vec![3.0, 3.0, 6.0, 6.0]);
        assert_eq!(avg.note, "Average of GFS and NAM");
        let mut m = WinterMap::new(WinterKind::Snow, "18");
        m.grid = Some(avg);
        assert!(m.source_text().starts_with("Average of GFS and NAM, "), "{}", m.source_text());
    }

    #[test]
    fn snow_and_ice() {
        let mut m = WinterMap::new(WinterKind::SnowIce, "18");
        assert_eq!(m.bands, WinterKind::Snow.default_bands());
        // No snow but some glaze: not "nothing expected".
        m.grid = Some(WinterGrid { nx: 2, ny: 2, north: 1.0, south: 0.0, west: 0.0, east: 1.0, values: vec![0.0; 4], ice: vec![0.0, 0.0, 0.1, 0.1], ..Default::default() });
        assert_eq!(m.empty_note(), None);
        // Row 0 is the north edge: halfway down is halfway between 0 and 0.1.
        assert!((m.ice_at(0.5, 0.5).unwrap() - 0.05).abs() < 1e-6);
        let csv = m.county_csv([("Dubois", "18", "18037", (0.5, 0.0))], None);
        assert!(csv.starts_with("County,Value (in snow),Band,Ice (in)\n"), "{csv}");
        assert!(csv.contains("Dubois,0.0,Below lowest band,0.10"), "{csv}");
        m.grid.as_mut().unwrap().ice = vec![0.0; 4];
        assert_eq!(m.empty_note(), Some("No accumulating snow or ice expected in this window."));
        // A plain snow map never reports ice.
        let s = WinterMap { kind: WinterKind::Snow, ..m.clone() };
        assert_eq!(s.ice_at(0.5, 0.5), None);
    }

    #[test]
    fn bands_and_grid() {
        let mut m = WinterMap::new(WinterKind::Snow, "18");
        assert_eq!(m.band_for(4.0).map(|b| b.label.as_str()), Some("3-5\""));
        assert_eq!(m.band_for(0.0), None);
        assert_eq!(m.blip_text(4.4).as_deref(), Some("4\""));
        m.grid = Some(WinterGrid { nx: 2, ny: 2, north: 1.0, south: 0.0, west: 0.0, east: 1.0, values: vec![0.0, 2.0, 4.0, 6.0], ..Default::default() });
        assert_eq!(m.value(None, 0.5, 0.5), Some(3.0));
        m.overrides.insert("18037".into(), 9.0);
        assert_eq!(m.value(Some("18037"), 0.5, 0.5), Some(9.0));
        // County list: highest first, painted value wins, other states left out.
        let csv = m.county_csv([("Dubois", "18", "18037", (0.5, 0.5)), ("Pike", "18", "18125", (0.5, 0.5)), ("Daviess", "21", "21059", (0.5, 0.5))], None);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "County,Value (in snow),Band");
        assert!(lines[1].starts_with("Dubois,9.0,"), "{csv}");
        // Band labels with an inch mark are quoted CSV: "3-5""".
        assert_eq!(lines[2], "Pike,3.0,\"3-5\"\"\"", "{csv}");
        assert_eq!(lines.len(), 3);
        // An update lists the previous amount and the change.
        let mut update = m.make_update();
        assert_eq!(update.based_on, m.id);
        assert_ne!(update.id, m.id);
        update.overrides.insert("18037".into(), 7.5);
        let csv = update.county_csv([("Dubois", "18", "18037", (0.5, 0.5))], Some(&m));
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "County,Value (in snow),Band,Previous,Change");
        assert!(lines[1].ends_with(",9.0,-1.5"), "{csv}");
    }
}
