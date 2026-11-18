//! Weather data models. Shapes match the Dubois County Weather site API (`/api/...`) and the
//! public NWS API. Every field is optional/defaulted so an upstream change never breaks a show.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------------------------
// Radar index (LibreWXR weather-maps.json via the site's /api/radar proxy)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RadarFrame {
    pub time: i64,
    #[serde(default)]
    pub path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RadarFrames {
    #[serde(default)]
    pub past: Vec<RadarFrame>,
    #[serde(default)]
    pub nowcast: Vec<RadarFrame>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SatelliteFrames {
    #[serde(default)]
    pub infrared: Vec<RadarFrame>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RadarIndex {
    #[serde(default)]
    pub generated: i64,
    #[serde(default)]
    pub radar: RadarFrames,
    #[serde(default)]
    pub satellite: SatelliteFrames,
}

impl RadarIndex {
    pub fn latest_past(&self) -> Option<&RadarFrame> {
        self.radar.past.last()
    }
}

/// LibreWXR palettes, exactly as the site offers them (frontend/src/lib/radarv2.js).
pub const RADAR_PALETTES: &[(&str, &str)] = &[
    ("1", "Classic Radar"),
    ("2", "Universal Blue"),
    ("3", "TITAN"),
    ("4", "Modern HD"),
    ("5", "Meteored"),
    ("6", "NEXRAD Level III"),
    ("7", "Rainbow"),
    ("8", "Dark Sky"),
    ("255", "Raw Grayscale"),
];

pub fn palette_label(code: &str) -> &'static str {
    RADAR_PALETTES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, l)| *l)
        .unwrap_or("Modern HD")
}

/// Legend stops per palette as [dBZ, hex], from the site's radar-legend.js (generated from
/// LibreWXR's color_table.csv, sampled every 5 dBZ).
pub fn legend_stops(code: &str) -> &'static [(i32, &'static str)] {
    match code {
        "1" => &[(10,"#dfdfdf"),(15,"#9bea8f"),(20,"#58ff42"),(25,"#47c278"),(30,"#4793f9"),(35,"#0c59ff"),(40,"#6153c1"),(45,"#ff93a3"),(50,"#ff3f35"),(55,"#c20511"),(60,"#ffeb0a"),(65,"#ff9811"),(70,"#a84c06"),(75,"#dda8ff")],
        "2" => &[(5,"#cec087"),(10,"#88ddee"),(15,"#0099cc"),(20,"#0077aa"),(25,"#005588"),(30,"#ffee00"),(35,"#ffaa00"),(40,"#ff7700"),(45,"#ff4400"),(50,"#ee0000"),(55,"#990000"),(60,"#ffaaff"),(65,"#ff77ff"),(70,"#ff44ff"),(75,"#ff00ff")],
        "3" => &[(5,"#007e00"),(10,"#087fdb"),(15,"#1c47e8"),(20,"#6e0dc6"),(25,"#c80f86"),(30,"#c06487"),(35,"#d2883b"),(40,"#fac431"),(45,"#fefb02"),(50,"#fe9a58"),(55,"#fe5f05"),(60,"#fd341c"),(65,"#bebebe"),(70,"#d3d3d3"),(75,"#d3d3d3")],
        "5" => &[(10,"#3ffefc"),(15,"#1790f9"),(20,"#0000f3"),(25,"#41ff50"),(30,"#32c23d"),(35,"#4b8339"),(40,"#ffff50"),(45,"#fdbb3c"),(50,"#df5e0b"),(55,"#fd8788"),(60,"#e90000"),(65,"#a80000"),(70,"#ef5bee"),(75,"#924cbe")],
        "6" => &[(5,"#00efe7"),(10,"#009cf7"),(15,"#0000f7"),(20,"#00ff00"),(25,"#03b703"),(30,"#087305"),(35,"#ffff00"),(40,"#ecce00"),(45,"#fe9300"),(50,"#ff0000"),(55,"#bd0000"),(60,"#bd0000"),(65,"#fe00fe"),(70,"#9c52c6"),(75,"#fefefe")],
        "7" => &[(10,"#009f9f"),(15,"#008c4b"),(20,"#00d319"),(25,"#21fd22"),(30,"#fffd1b"),(35,"#ffd400"),(40,"#ffab00"),(45,"#ff6e00"),(50,"#f01002"),(55,"#d00523"),(60,"#e400b1"),(65,"#ff33ff"),(70,"#ff66ff"),(75,"#ff99ff")],
        "8" => &[(5,"#005eb6"),(10,"#005eb6"),(15,"#005eb6"),(20,"#005eb6"),(25,"#2458af"),(30,"#8e4b9b"),(35,"#fc5370"),(40,"#ffb76e"),(45,"#fffd05"),(50,"#fffd05"),(55,"#fffd05"),(60,"#fffd05"),(65,"#fffd05"),(70,"#fffd05"),(75,"#fffd05")],
        "255" => &[(5,"#4a4a4a"),(10,"#545454"),(15,"#5e5e5e"),(20,"#686868"),(25,"#727272"),(30,"#7c7c7c"),(35,"#878787"),(40,"#919191"),(45,"#9b9b9b"),(50,"#a5a5a5"),(55,"#afafaf"),(60,"#b9b9b9"),(65,"#c3c3c3"),(70,"#cdcdcd"),(75,"#d7d7d7")],
        _ => &[(10,"#01b714"),(15,"#088915"),(20,"#11651a"),(25,"#064307"),(30,"#ffee07"),(35,"#f8bb08"),(40,"#f38b08"),(45,"#f07108"),(50,"#ea5e09"),(55,"#df370a"),(60,"#d3100c"),(65,"#c00d09"),(70,"#b80c08"),(75,"#b80c08")],
    }
}

// ---------------------------------------------------------------------------------------------
// Conditions + forecast (/api/weather/overview)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Current {
    pub station: Option<String>,
    pub station_label: Option<String>,
    pub station_identifier: Option<String>,
    pub timestamp: Option<String>,
    pub text_description: Option<String>,
    pub temperature_f: Option<f64>,
    pub dewpoint_f: Option<f64>,
    pub wind_speed_mph: Option<f64>,
    pub wind_gust_mph: Option<f64>,
    pub wind_direction: Option<f64>,
    pub wind_direction_cardinal: Option<String>,
    pub relative_humidity: Option<f64>,
    pub barometric_pressure_in_hg: Option<f64>,
    pub visibility_miles: Option<f64>,
    pub heat_index_f: Option<f64>,
    pub wind_chill_f: Option<f64>,
}

impl Current {
    pub fn feels_like_f(&self) -> Option<f64> {
        self.heat_index_f
            .or(self.wind_chill_f)
            .or(self.temperature_f)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Period {
    pub name: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub is_daytime: bool,
    pub temperature: Option<f64>,
    pub temperature_unit: Option<String>,
    pub short_forecast: String,
    pub detailed_forecast: String,
    pub wind_speed: Option<String>,
    pub wind_direction: Option<String>,
    pub precipitation_probability: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct OverviewLocation {
    pub lat: f64,
    pub lon: f64,
    pub coverage_label: Option<String>,
    pub nearest_city: Option<String>,
    pub nearest_state: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Overview {
    pub location: OverviewLocation,
    pub current: Current,
    pub forecast: Vec<Period>,
}

/// A day of the 7-day: the daytime period paired with the following night.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DaySummary {
    pub name: String,
    pub short_name: String,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub short_forecast: String,
    pub precip: Option<f64>,
    pub is_daytime: bool,
}

impl Overview {
    pub fn days(&self) -> Vec<DaySummary> {
        let mut days = Vec::new();
        let periods = &self.forecast;
        let mut i = 0;
        while i < periods.len() {
            let p = &periods[i];
            if p.is_daytime {
                let night = periods.get(i + 1).filter(|n| !n.is_daytime);
                days.push(DaySummary {
                    name: p.name.clone(),
                    short_name: short_day(&p.name),
                    high: p.temperature,
                    low: night.and_then(|n| n.temperature),
                    short_forecast: p.short_forecast.clone(),
                    precip: max_opt(
                        p.precipitation_probability,
                        night.and_then(|n| n.precipitation_probability),
                    ),
                    is_daytime: true,
                });
                i += if night.is_some() { 2 } else { 1 };
            } else {
                days.push(DaySummary {
                    name: p.name.clone(),
                    short_name: short_day(&p.name),
                    high: None,
                    low: p.temperature,
                    short_forecast: p.short_forecast.clone(),
                    precip: p.precipitation_probability,
                    is_daytime: false,
                });
                i += 1;
            }
        }
        days
    }
}

fn max_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

fn short_day(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.starts_with("tonight") || lower.starts_with("overnight") {
        return "TONIGHT".into();
    }
    if lower.starts_with("today") || lower.starts_with("this afternoon") {
        return "TODAY".into();
    }
    name.split_whitespace()
        .next()
        .map(|w| w.chars().take(3).collect::<String>().to_uppercase())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// NWS alerts (api.weather.gov/alerts/active) - polygons for the map + local list
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Alert {
    pub id: String,
    pub event: String,
    pub headline: String,
    pub severity: String,
    pub urgency: String,
    pub area_desc: String,
    pub sender: String,
    pub sent: Option<String>,
    pub expires: Option<String>,
    pub description: String,
    pub instruction: String,
    /// Outer rings of the warning polygon(s) as (lon, lat).
    pub polygons: Vec<Vec<(f64, f64)>>,
    /// NWS eventMotionDescription (storm position + motion), when given.
    #[serde(default)]
    pub motion_text: Option<String>,
    /// Alert, Update or Cancel.
    #[serde(default)]
    pub message_type: String,
    /// County FIPS codes (5 digits) from the alert's SAME geocodes. Watches and most advisories
    /// come without a polygon; the map draws them from these counties.
    #[serde(default)]
    pub fips: Vec<String>,
    /// The polygons were built from county outlines (not an NWS storm-based polygon).
    #[serde(default)]
    pub county_based: bool,
}

impl Alert {
    /// NWS-standard display color for the event (hazard map colors).
    pub fn color(&self) -> &'static str {
        alert_color(&self.event)
    }

    /// Ordering for lists: warnings first, then watches, then the rest.
    pub fn priority(&self) -> u8 {
        let e = self.event.to_lowercase();
        if e.contains("tornado warning") {
            0
        } else if e.contains("severe thunderstorm warning") || e.contains("flash flood warning") {
            1
        } else if e.contains("warning") {
            2
        } else if e.contains("watch") {
            3
        } else if e.contains("advisory") {
            4
        } else {
            5
        }
    }

    pub fn from_feature(feature: &serde_json::Value) -> Option<Alert> {
        let props = feature.get("properties")?;
        let s = |key: &str| {
            props
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };
        let mut polygons = Vec::new();
        if let Some(geometry) = feature.get("geometry").filter(|g| !g.is_null()) {
            collect_rings(geometry, &mut polygons);
        }
        Some(Alert {
            id: s("id"),
            event: s("event"),
            headline: s("headline"),
            severity: s("severity"),
            urgency: s("urgency"),
            area_desc: s("areaDesc"),
            sender: s("senderName"),
            sent: props.get("sent").and_then(|v| v.as_str()).map(String::from),
            expires: props
                .get("ends")
                .and_then(|v| v.as_str())
                .or_else(|| props.get("expires").and_then(|v| v.as_str()))
                .map(String::from),
            description: s("description"),
            instruction: s("instruction"),
            polygons,
            motion_text: props
                .pointer("/parameters/eventMotionDescription/0")
                .and_then(|v| v.as_str())
                .map(String::from),
            message_type: s("messageType"),
            fips: props
                .pointer("/geocode/SAME")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|c| c.as_str())
                .filter(|c| c.len() == 6)
                .map(|c| c[1..].to_string())
                .collect(),
            county_based: false,
        })
    }
}

/// Outer rings of a GeoJSON Polygon/MultiPolygon as (lon, lat) lists.
pub fn collect_rings(geometry: &serde_json::Value, out: &mut Vec<Vec<(f64, f64)>>) {
    let kind = geometry.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let coords = geometry.get("coordinates");
    let ring = |ring: &serde_json::Value| -> Vec<(f64, f64)> {
        ring.as_array()
            .map(|pts| {
                pts.iter()
                    .filter_map(|p| {
                        let p = p.as_array()?;
                        Some((p.first()?.as_f64()?, p.get(1)?.as_f64()?))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    match (kind, coords) {
        ("Polygon", Some(c)) => {
            if let Some(outer) = c.as_array().and_then(|r| r.first()) {
                out.push(ring(outer));
            }
        }
        ("MultiPolygon", Some(c)) => {
            for poly in c.as_array().into_iter().flatten() {
                if let Some(outer) = poly.as_array().and_then(|r| r.first()) {
                    out.push(ring(outer));
                }
            }
        }
        ("GeometryCollection", _) => {
            for g in geometry
                .get("geometries")
                .and_then(|g| g.as_array())
                .into_iter()
                .flatten()
            {
                collect_rings(g, out);
            }
        }
        _ => {}
    }
}

/// NWS hazard map colors (weather.gov/help-map) for the common broadcast events.
/// US time zone abbreviation for a local time (EDT, CST...), from the UTC offset and whether
/// daylight time is in effect. Falls back to "UTC±h".
pub fn tz_abbrev(t: chrono::DateTime<chrono::Local>) -> String {
    use chrono::{Datelike, TimeZone};
    let offset_h = t.offset().local_minus_utc() / 3600;
    let year = t.year();
    let jan = chrono::Local.with_ymd_and_hms(year, 1, 15, 12, 0, 0).single().map(|d| d.offset().local_minus_utc() / 3600);
    let jul = chrono::Local.with_ymd_and_hms(year, 7, 15, 12, 0, 0).single().map(|d| d.offset().local_minus_utc() / 3600);
    let dst = matches!((jan, jul), (Some(j), Some(s)) if j != s && offset_h == s);
    let standard = if dst { offset_h - 1 } else { offset_h };
    let zone = match standard {
        -5 => "E",
        -6 => "C",
        -7 => "M",
        -8 => "P",
        -9 => "AK",
        -10 => "H",
        _ => return format!("UTC{offset_h:+}"),
    };
    format!("{zone}{}T", if dst { "D" } else { "S" })
}

/// "4:45 PM EDT", "tomorrow at 11 AM EDT", "Sat at 11 AM EDT", "Nov 14 at 11 AM EST".
pub fn when_words(t: chrono::DateTime<chrono::Utc>, clock_24h: bool) -> String {
    use chrono::Timelike;
    let local = t.with_timezone(&chrono::Local);
    let time = if clock_24h {
        local.format("%H:%M").to_string()
    } else if local.minute() == 0 {
        local.format("%-I %p").to_string()
    } else {
        local.format("%-I:%M %p").to_string()
    };
    let tz = tz_abbrev(local);
    let today = chrono::Local::now().date_naive();
    let days = (local.date_naive() - today).num_days();
    match days {
        0 => format!("{time} {tz}"),
        1 => format!("tomorrow at {time} {tz}"),
        -1 => format!("yesterday at {time} {tz}"),
        2..=6 => format!("{} at {time} {tz}", local.format("%a")),
        _ => format!("{} at {time} {tz}", local.format("%b %-d")),
    }
}

/// "45 min", "2 hr 15 min", "3 days".
pub fn duration_words(minutes: i64) -> String {
    match minutes {
        m if m < 90 => format!("{m} min"),
        m if m < 48 * 60 => {
            let (h, rest) = (m / 60, m % 60);
            if rest == 0 { format!("{h} hr") } else { format!("{h} hr {rest} min") }
        }
        m => format!("{} days", m / (24 * 60)),
    }
}

pub fn alert_color(event: &str) -> &'static str {
    let e = event.to_lowercase();
    match () {
        _ if e.contains("tornado warning") => "#FF0000",
        _ if e.contains("severe thunderstorm warning") => "#FFA500",
        _ if e.contains("flash flood warning") => "#8B0000",
        _ if e.contains("flood warning") => "#00FF00",
        _ if e.contains("tornado watch") => "#FFFF00",
        _ if e.contains("severe thunderstorm watch") => "#DB7093",
        _ if e.contains("flash flood watch") || e.contains("flood watch") => "#2E8B57",
        _ if e.contains("winter storm warning") => "#FF69B4",
        _ if e.contains("blizzard warning") => "#FF4500",
        _ if e.contains("ice storm warning") => "#8B008B",
        _ if e.contains("winter storm watch") => "#4682B4",
        _ if e.contains("winter weather advisory") => "#7B68EE",
        _ if e.contains("special weather statement") => "#FFE4B5",
        _ if e.contains("heat advisory") => "#FF7F50",
        _ if e.contains("excessive heat") || e.contains("extreme heat") => "#C71585",
        _ if e.contains("wind advisory") => "#D2B48C",
        _ if e.contains("high wind") => "#DAA520",
        _ if e.contains("dense fog") => "#708090",
        _ if e.contains("frost") || e.contains("freeze") => "#6495ED",
        _ if e.contains("red flag") => "#FF1493",
        _ if e.contains("advisory") => "#40E0D0",
        _ if e.contains("watch") => "#FFD700",
        _ if e.contains("warning") => "#FF4040",
        _ => "#B0C4DE",
    }
}

// ---------------------------------------------------------------------------------------------
// Storm cells (/api/radar/storm-cells)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CellIntensity {
    pub level: u8,
    pub label: String,
    pub hail: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CellMotion {
    pub speed_kmh: f64,
    pub speed_mph: f64,
    pub heading_deg: f64,
    pub toward: String,
    pub from: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct CellLightning {
    pub strikes: u32,
    pub minutes: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct StormCell {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    pub max_dbz: f64,
    pub area_sq_mi: f64,
    pub intensity: CellIntensity,
    pub distance_mi: f64,
    pub direction: String,
    pub motion: Option<CellMotion>,
    pub lightning: CellLightning,
    pub tags: Vec<String>,
}

impl StormCell {
    /// A calm, plain description for viewers ("Heavy rain", "Strong storm"): no hype words.
    pub fn calm_label(&self) -> String {
        let lightning = self.lightning.strikes > 0;
        let hail = self.intensity.hail.as_deref().is_some_and(|h| !h.is_empty());
        match self.intensity.level {
            0 | 1 => "Light rain".into(),
            2 => if lightning { "Rain with lightning".into() } else { "Moderate rain".into() },
            3 => if lightning { "Thunderstorm".into() } else { "Heavy rain".into() },
            _ if hail => "Strong storm, hail possible".into(),
            _ => "Strong storm".into(),
        }
    }

    /// The on-air name for this cell: calm wording, or the feed's own intensity label.
    pub fn on_air_label(&self, calm: bool) -> String {
        if calm {
            self.calm_label()
        } else {
            format!("{} storm", self.intensity.label)
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct StormCells {
    pub cells: Vec<StormCell>,
}

// ---------------------------------------------------------------------------------------------
// SPC outlooks (/api/weather/spc-outlooks)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpcArea {
    pub label: String,
    pub rank: i32,
    pub stroke: String,
    pub fill: String,
    pub fill_opacity: f32,
    pub rings: Vec<Vec<(f64, f64)>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpcDay {
    pub day: u8,
    pub label: String,
    pub categorical: Vec<SpcArea>,
    /// Hazard probabilities for this day.
    pub probabilities: Vec<SpcHazard>,
}

/// Polygons of one SPC layer (categorical risk or one hazard's probabilities), lowest rank first.
fn parse_spc_areas(layer: Option<&serde_json::Value>) -> Vec<SpcArea> {
    let mut areas = Vec::new();
    let features = layer.and_then(|c| c.get("features")).and_then(|f| f.as_array());
    for feature in features.into_iter().flatten() {
        let style = feature.get("style");
        let mut rings = Vec::new();
        if let Some(geometry) = feature.get("geometry") {
            collect_rings(geometry, &mut rings);
        }
        areas.push(SpcArea {
            label: feature
                .get("shortLabel")
                .or_else(|| feature.get("label"))
                .and_then(|l| l.as_str())
                .unwrap_or("Risk")
                .to_string(),
            rank: feature.get("rank").and_then(|r| r.as_i64()).unwrap_or(0) as i32,
            stroke: style
                .and_then(|s| s.get("stroke"))
                .and_then(|s| s.as_str())
                .unwrap_or("#55BB55")
                .to_string(),
            fill: style
                .and_then(|s| s.get("fill"))
                .and_then(|s| s.as_str())
                .unwrap_or("#C1E9C1")
                .to_string(),
            fill_opacity: style
                .and_then(|s| s.get("fillOpacity"))
                .and_then(|s| s.as_f64())
                .unwrap_or(0.2) as f32,
            rings,
        });
    }
    areas.sort_by_key(|a| a.rank);
    areas
}

pub fn parse_spc(value: &serde_json::Value) -> Vec<SpcDay> {
    let mut out = Vec::new();
    for day in value.get("days").and_then(|d| d.as_array()).into_iter().flatten() {
        let day_num = day
            .get("day")
            .and_then(|d| d.as_str().and_then(|s| s.parse().ok()).or(d.as_u64().map(|n| n as u8)))
            .unwrap_or(1);
        let label = day
            .get("label")
            .and_then(|l| l.as_str())
            .unwrap_or("Day")
            .to_string();
        let categorical = parse_spc_areas(day.get("categorical"));
        // Tornado / hail / wind (days 1-2) or combined severe (day 3+) probabilities.
        let probabilities = day
            .get("probabilities")
            .and_then(|p| p.as_array())
            .into_iter()
            .flatten()
            .map(|layer| SpcHazard {
                key: layer.get("key").and_then(|k| k.as_str()).unwrap_or("severe").to_string(),
                label: layer.get("label").and_then(|k| k.as_str()).unwrap_or("Severe chance").to_string(),
                areas: parse_spc_areas(Some(layer)),
            })
            .collect();
        out.push(SpcDay {
            day: day_num,
            label,
            categorical,
            probabilities,
        });
    }
    out
}

/// One SPC probability layer ("tornado", "hail", "wind" or "severe").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpcHazard {
    pub key: String,
    pub label: String,
    pub areas: Vec<SpcArea>,
}

impl SpcDay {
    /// Areas for the chosen layer: "categorical" (or empty) for risk levels, else a hazard key.
    pub fn areas(&self, layer: &str) -> &[SpcArea] {
        if layer.is_empty() || layer == "categorical" {
            return &self.categorical;
        }
        self.probabilities
            .iter()
            .find(|h| h.key == layer)
            .map(|h| h.areas.as_slice())
            .unwrap_or(&[])
    }
}

// ---------------------------------------------------------------------------------------------
// SPC mesoscale discussions (NOAA map services)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MesoDiscussion {
    /// "MD 1234" or similar.
    pub name: String,
    /// Link to the full discussion on spc.noaa.gov, when given.
    pub url: Option<String>,
    pub rings: Vec<Vec<(f64, f64)>>,
}

pub fn parse_mesoscale(value: &serde_json::Value) -> Vec<MesoDiscussion> {
    value
        .get("features")
        .and_then(|f| f.as_array())
        .into_iter()
        .flatten()
        .filter_map(|feature| {
            let props = feature.get("properties")?;
            let name = props.get("name").and_then(|n| n.as_str()).unwrap_or("").trim().to_string();
            // The service answers with a tiny "NoArea" square when no discussion is active.
            if name.is_empty() || name.eq_ignore_ascii_case("NoArea") {
                return None;
            }
            let url = props
                .get("popupinfo")
                .and_then(|p| p.as_str())
                .and_then(|p| p.split(['"', ' ', '<', '>']).find(|t| t.starts_with("http")))
                .map(str::to_string);
            let mut rings = Vec::new();
            collect_rings(feature.get("geometry")?, &mut rings);
            Some(MesoDiscussion { name, url, rings })
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Snapshot shared with the compositor and graphics bindings
// ---------------------------------------------------------------------------------------------

/// One airport observation (METAR) for station plots.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metar {
    pub id: String,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub temp_f: Option<f32>,
    pub dew_f: Option<f32>,
    /// Direction the wind blows from, degrees (None when variable or calm).
    pub wind_dir: Option<f32>,
    pub wind_kt: Option<f32>,
    pub gust_kt: Option<f32>,
    pub visibility: String,
    /// VFR, MVFR, IFR or LIFR.
    pub flight_category: String,
    pub obs_time: i64,
}

/// AviationWeather METAR JSON (through the relay) into station plots.
pub fn parse_metars(value: &serde_json::Value) -> Vec<Metar> {
    let c_to_f = |c: f64| (c * 1.8 + 32.0) as f32;
    value.as_array().into_iter().flatten().filter_map(|m| {
        let num = |key: &str| m.get(key).and_then(|v| v.as_f64());
        Some(Metar {
            id: m.get("icaoId")?.as_str()?.to_string(),
            name: m.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
            lat: num("lat")?,
            lon: num("lon")?,
            temp_f: num("temp").map(c_to_f),
            dew_f: num("dewp").map(c_to_f),
            wind_dir: num("wdir").map(|d| d as f32),
            wind_kt: num("wspd").map(|v| v as f32),
            gust_kt: num("wgst").map(|v| v as f32),
            visibility: m.get("visib").map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).unwrap_or_default(),
            flight_category: m.get("fltCat").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
            obs_time: m.get("obsTime").and_then(|v| v.as_i64()).unwrap_or(0),
        })
    }).collect()
}

#[derive(Clone, Debug, Default)]
pub struct WxSnapshot {
    pub radar: Option<RadarIndex>,
    /// Airport observations over the map (Layers > Airport obs).
    pub metars: Vec<Metar>,
    pub overview: Option<Overview>,
    pub local_alerts: Vec<Alert>,
    pub map_alerts: Vec<Alert>,
    pub cells: Vec<StormCell>,
    pub spc: Vec<SpcDay>,
    /// Active SPC mesoscale discussions.
    pub mesoscale: Vec<MesoDiscussion>,
    /// Lightning strikes (newest feed pull), Unix ms times.
    pub lightning: Vec<Strike>,
    /// NWS Local Storm Reports (last 24 h).
    pub reports: Vec<StormReport>,
    /// INDOT traffic events.
    pub roads: Vec<RoadEvent>,
    /// Gridded model data for data layers / Futurecast.
    pub grid: Option<std::sync::Arc<crate::grid::WxGrid>>,
    pub models: [Option<std::sync::Arc<crate::futurecast::ModelLoop>>; 3],
    pub updated_at: Option<chrono::DateTime<chrono::Local>>,
    /// River gauges near home (NWPS), worst first.
    pub rivers: Vec<crate::feeds::RiverGauge>,
    /// WPC rainfall forecast contours: Day 1, Days 1-3, Days 1-7.
    pub qpf: [Vec<SpcArea>; 3],
    /// WPC Excessive Rainfall Outlook, Days 1-3.
    pub ero: [Vec<SpcArea>; 3],
    /// Community board: closings and warming / cooling centers.
    pub community: crate::community::Community,
    /// The forecaster's published edits (day overrides, regions; area adjustments are already
    /// in `grid`).
    pub forecast_edits: crate::forecast_edit::EditSet,
    /// Warnings issued today for the coverage area: (unix time, event, areas), newest last.
    pub warning_history: Vec<(i64, String, String)>,
    /// Normals, records, 16-day, model comparison, aurora, airports.
    pub climate: crate::climate::Climate,
    /// Hydrograph of the worst river gauge near home.
    pub hydrograph: Option<crate::feeds::Hydrograph>,
    /// Storm cell history: id -> (unix time, max dBZ) samples, last 2 hours.
    pub cell_history: std::collections::HashMap<String, Vec<(i64, f32)>>,
    /// Storm cell area history: id -> (unix time, sq mi).
    pub cell_area_history: std::collections::HashMap<String, Vec<(i64, f32)>>,
    /// Storm tracks, splits / merges, lightning jumps and clusters, MCS (cells.rs).
    pub analysis: crate::cells::Analysis,
    /// Tropical, fire, drought and climate history (hazards.rs).
    pub hazards: crate::hazards::Hazards,
    /// Earthquakes (USGS), newest first: M2.5+ for the past month plus every quake this week.
    pub quakes: Vec<crate::quakes::Quake>,
    /// Viewer submissions (portal), newest first.
    pub viewer: Vec<crate::portal::Submission>,
    /// Emergency operations: shelters, zones, resources, closures (eoc.rs).
    pub eoc: crate::eoc::Eoc,
    /// Power outages by county and utility (outages.rs).
    pub outages: crate::outages::Outages,
    /// NWS hourly forecast at home (graphics fall back to it when the model grid doesn't
    /// cover home).
    pub home_hourly: Vec<crate::homecast::Hour>,
    /// Storm nowcast: motion spread, threats, initiation, arrival chances (nowcast.rs).
    pub nowcast: crate::nowcast::Nowcast,
    /// A loaded historical event (Event history).
    pub history_event: Option<std::sync::Arc<crate::history::Event>>,
    /// Coverage towns (home, watched spots, forecast regions): each one's NWS 7-day, for the
    /// Regional Outlook graphic.
    pub coverage: Option<crate::social_graphics::Regional>,
}

impl WxSnapshot {
    pub fn model(&self, source: crate::futurecast::Source) -> Option<&crate::futurecast::ModelLoop> {
        self.models[source.index()].as_deref()
    }
}

#[derive(Clone, Debug, Default)]
pub struct FeedStatus {
    pub ok: bool,
    pub message: String,
    pub at: Option<chrono::DateTime<chrono::Local>>,
}

/// Health of each data feed, shown in the status bar and Data Discovery.
#[derive(Clone, Debug, Default)]
pub struct DataHealth {
    pub radar: FeedStatus,
    pub overview: FeedStatus,
    pub alerts: FeedStatus,
    pub cells: FeedStatus,
    pub spc: FeedStatus,
    pub mesoscale: FeedStatus,
    pub hydro: FeedStatus,
    pub climate: FeedStatus,
    pub lightning: FeedStatus,
    pub reports: FeedStatus,
    pub roads: FeedStatus,
    pub grid: FeedStatus,
}

// ---------------------------------------------------------------------------------------------
// Lightning (site feed: /api/public/lightning.json, Blitzortung relay)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    pub lat: f64,
    pub lon: f64,
    /// Unix milliseconds.
    pub time_ms: i64,
}

/// Lightning safety radius around home (NWS: if thunder roars, go indoors; ~10 miles).
pub const LIGHTNING_RADIUS_MI: f64 = 10.0;
/// Minutes without a strike inside the radius before the all clear (NWS 30-minute rule).
pub const LIGHTNING_ALL_CLEAR_MIN: i64 = 30;

/// Lightning near home: the closest recent strike and the all-clear countdown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightningNear {
    /// Closest strike inside the radius in the last 30 minutes, miles.
    pub nearest_mi: f64,
    /// Minutes since the newest strike inside the radius.
    pub minutes_ago: i64,
    /// Minutes until the all clear (0 = all clear now).
    pub all_clear_in: i64,
    /// Strikes inside the radius in the last 30 minutes.
    pub count: usize,
}

impl LightningNear {
    pub fn summary(&self) -> String {
        let ago = if self.minutes_ago <= 0 { "just now".to_string() } else { format!("{} min ago", self.minutes_ago) };
        format!("Lightning {:.0} mi away, last strike {ago}", self.nearest_mi)
    }

    pub fn all_clear_text(&self) -> String {
        if self.all_clear_in <= 0 {
            "All clear".into()
        } else {
            format!("All clear in {} min", self.all_clear_in)
        }
    }
}

/// None when no strike fell within [`LIGHTNING_RADIUS_MI`] of home in the last 30 minutes.
pub fn lightning_near(strikes: &[Strike], home_lat: f64, home_lon: f64) -> Option<LightningNear> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    let window_ms = LIGHTNING_ALL_CLEAR_MIN * 60_000;
    let mut nearest = f64::MAX;
    let mut newest = i64::MIN;
    let mut count = 0;
    for s in strikes {
        if now_ms - s.time_ms > window_ms {
            continue;
        }
        let mi = crate::map::geo::haversine_km(home_lat, home_lon, s.lat, s.lon) * 0.621371;
        if mi <= LIGHTNING_RADIUS_MI {
            count += 1;
            nearest = nearest.min(mi);
            newest = newest.max(s.time_ms);
        }
    }
    if count == 0 {
        return None;
    }
    let minutes_ago = ((now_ms - newest) / 60_000).max(0);
    Some(LightningNear {
        nearest_mi: nearest,
        minutes_ago,
        all_clear_in: (LIGHTNING_ALL_CLEAR_MIN - minutes_ago).max(0),
        count,
    })
}

pub fn parse_lightning(value: &serde_json::Value) -> Vec<Strike> {
    value
        .get("features")
        .and_then(|f| f.as_array())
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let coords = f.get("geometry")?.get("coordinates")?.as_array()?;
            let lon = coords.first()?.as_f64()?;
            let lat = coords.get(1)?.as_f64()?;
            let time_ms = f.get("properties")?.get("validtime")?.as_i64()?;
            Some(Strike { lat, lon, time_ms })
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// NEXRAD sites (WSR-88D) for single-site products. Ids without the leading K.
// ---------------------------------------------------------------------------------------------

pub const NEXRAD_SITES: &[(&str, &str, f64, f64)] = &[
    ("VWX", "Evansville IN", 38.260, -87.725),
    ("LVX", "Louisville KY", 37.975, -85.944),
    ("IND", "Indianapolis IN", 39.708, -86.280),
    ("PAH", "Paducah KY", 37.068, -88.772),
    ("HPX", "Fort Campbell KY", 36.737, -87.285),
    ("ILX", "Lincoln IL", 40.150, -89.337),
    ("LSX", "St. Louis MO", 38.699, -90.683),
    ("ILN", "Wilmington OH", 39.420, -83.822),
    ("JKL", "Jackson KY", 37.591, -83.313),
    ("OHX", "Nashville TN", 36.247, -86.563),
    ("IWX", "Northern Indiana", 41.359, -85.700),
    ("LOT", "Chicago IL", 41.604, -88.085),
    ("GRR", "Grand Rapids MI", 42.894, -85.545),
    ("DTX", "Detroit MI", 42.700, -83.472),
    ("CLE", "Cleveland OH", 41.413, -81.860),
    ("PBZ", "Pittsburgh PA", 40.532, -80.218),
    ("RLX", "Charleston WV", 38.311, -81.723),
    ("MRX", "Knoxville TN", 36.168, -83.402),
    ("NQA", "Memphis TN", 35.345, -89.873),
    ("LZK", "Little Rock AR", 34.836, -92.262),
    ("SGF", "Springfield MO", 37.235, -93.400),
    ("EAX", "Kansas City MO", 38.810, -94.264),
    ("DVN", "Quad Cities IA", 41.612, -90.581),
    ("DMX", "Des Moines IA", 41.731, -93.723),
    ("MKX", "Milwaukee WI", 42.968, -88.551),
    ("HTX", "Huntsville AL", 34.931, -86.084),
    ("BMX", "Birmingham AL", 33.172, -86.770),
    ("GWX", "Columbus AFB MS", 33.897, -88.329),
    ("FFC", "Atlanta GA", 33.364, -84.566),
    ("GSP", "Greer SC", 34.883, -82.220),
    ("LWX", "Sterling VA", 38.976, -77.487),
    ("OKX", "New York NY", 40.866, -72.864),
    ("JAN", "Jackson MS", 32.318, -90.080),
    ("LIX", "New Orleans LA", 30.337, -89.825),
    ("SHV", "Shreveport LA", 32.451, -93.841),
    ("HGX", "Houston TX", 29.472, -95.079),
    ("EWX", "Austin/San Antonio TX", 29.704, -98.029),
    ("FWS", "Dallas/Fort Worth TX", 32.573, -97.303),
    ("TLX", "Oklahoma City OK", 35.333, -97.278),
    ("INX", "Tulsa OK", 36.175, -95.564),
    ("ICT", "Wichita KS", 37.654, -97.443),
    ("OAX", "Omaha NE", 41.320, -96.367),
    ("MPX", "Minneapolis MN", 44.849, -93.565),
    ("FTG", "Denver CO", 39.787, -104.546),
    ("TLH", "Tallahassee FL", 30.398, -84.329),
    ("AMX", "Miami FL", 25.611, -80.413),
    ("TBW", "Tampa Bay FL", 27.705, -82.402),
    ("MLB", "Melbourne FL", 28.113, -80.654),
    ("JAX", "Jacksonville FL", 30.485, -81.702),
    ("RAX", "Raleigh NC", 35.665, -78.490),
    ("FCX", "Roanoke VA", 37.024, -80.274),
    ("DIX", "Philadelphia PA", 39.947, -74.411),
    ("BOX", "Boston MA", 41.956, -71.137),
    // The rest of the lower-48 network, from api.weather.gov/radar/stations (WSR-88D), 2026-11-14.
    ("ABR", "Aberdeen SD", 45.456, -98.413),
    ("ABX", "Albuquerque NM", 35.150, -106.824),
    ("AKQ", "Norfolk VA", 36.984, -77.008),
    ("AMA", "Amarillo TX", 35.233, -101.709),
    ("APX", "Gaylord MI", 44.906, -84.720),
    ("ARX", "La Crosse WI", 43.823, -91.191),
    ("ATX", "Seattle WA", 48.195, -122.496),
    ("BBX", "Beale AFB CA", 39.496, -121.632),
    ("BGM", "Binghamton NY", 42.200, -75.985),
    ("BHX", "Eureka CA", 40.498, -124.292),
    ("BIS", "Bismarck ND", 46.771, -100.760),
    ("BLX", "Billings MT", 45.854, -108.607),
    ("BRO", "Brownsville TX", 25.916, -97.419),
    ("BUF", "Buffalo NY", 42.949, -78.737),
    ("BYX", "Key West FL", 24.597, -81.703),
    ("CAE", "Columbia SC", 33.949, -81.119),
    ("CBW", "Caribou ME", 46.039, -67.806),
    ("CBX", "Boise ID", 43.490, -116.236),
    ("CCX", "State College PA", 40.923, -78.004),
    ("CLX", "Charleston SC", 32.656, -81.042),
    ("CRP", "Corpus Christi TX", 27.784, -97.511),
    ("CXX", "Burlington VT", 44.511, -73.166),
    ("CYS", "Cheyenne WY", 41.152, -104.806),
    ("DAX", "Sacramento CA", 38.501, -121.678),
    ("DDC", "Dodge City KS", 37.761, -99.969),
    ("DFX", "Laughlin AFB TX", 29.272, -100.280),
    ("DGX", "Jackson/Brandon MS", 32.280, -89.984),
    ("DLH", "Duluth MN", 46.837, -92.210),
    ("DOX", "Dover AFB DE", 38.826, -75.440),
    ("DYX", "Dyess AFB TX", 32.538, -99.254),
    ("EMX", "Tucson AZ", 31.894, -110.630),
    ("ENX", "Albany NY", 42.587, -74.064),
    ("EOX", "Fort Rucker AL", 31.461, -85.459),
    ("EPZ", "El Paso TX", 31.873, -106.698),
    ("ESX", "Las Vegas NV", 35.701, -114.891),
    ("EVX", "Eglin AFB FL", 30.565, -85.922),
    ("EYX", "Edwards AFB CA", 35.098, -117.561),
    ("FDR", "Altus AFB OK", 34.362, -98.977),
    ("FDX", "Cannon AFB NM", 34.634, -103.619),
    ("FSD", "Sioux Falls SD", 43.588, -96.729),
    ("FSX", "Flagstaff AZ", 34.574, -111.198),
    ("GGW", "Glasgow MT", 48.206, -106.625),
    ("GJX", "Grand Junction CO", 39.062, -108.214),
    ("GLD", "Goodland KS", 39.367, -101.700),
    ("GRB", "Green Bay WI", 44.499, -88.111),
    ("GRK", "Fort Hood TX", 30.722, -97.383),
    ("GYX", "Portland ME", 43.891, -70.256),
    ("HDC", "Hammond LA", 30.520, -90.407),
    ("HDX", "Holloman AFB NM", 33.077, -106.120),
    ("HNX", "San Joaquin Valley CA", 36.314, -119.632),
    ("ICX", "Cedar City UT", 37.591, -112.862),
    ("IWA", "Phoenix AZ", 33.289, -111.670),
    ("JGX", "Robins AFB GA", 32.675, -83.351),
    ("LBB", "Lubbock TX", 33.654, -101.814),
    ("LCH", "Lake Charles LA", 30.125, -93.216),
    ("LGX", "Langley Hill WA", 47.117, -124.107),
    ("LNX", "North Platte NE", 41.958, -100.576),
    ("LRX", "Elko NV", 40.740, -116.803),
    ("LTX", "Wilmington NC", 33.989, -78.429),
    ("MAF", "Midland/Odessa TX", 31.943, -102.189),
    ("MAX", "Medford OR", 42.081, -122.717),
    ("MBX", "Minot AFB ND", 48.392, -100.864),
    ("MHX", "Morehead City NC", 34.776, -76.876),
    ("MOB", "Mobile AL", 30.679, -88.240),
    ("MQT", "Marquette MI", 46.531, -87.548),
    ("MSX", "Missoula MT", 47.041, -113.986),
    ("MTX", "Salt Lake City UT", 41.263, -112.448),
    ("MUX", "San Francisco CA", 37.155, -121.898),
    ("MVX", "Fargo/Grand Forks ND", 47.528, -97.325),
    ("MXX", "Maxwell AFB AL", 32.537, -85.790),
    ("NKX", "San Diego CA", 32.919, -117.042),
    ("OTX", "Spokane WA", 47.681, -117.626),
    ("PDT", "Pendleton OR", 45.691, -118.853),
    ("POE", "Fort Polk LA", 31.155, -92.976),
    ("PUX", "Pueblo CO", 38.459, -104.181),
    ("RGX", "Reno NV", 39.754, -119.462),
    ("RIW", "Riverton WY", 43.066, -108.477),
    ("RTX", "Portland OR", 45.715, -122.965),
    ("SFX", "Pocatello ID", 43.106, -112.686),
    ("SJT", "San Angelo TX", 31.371, -100.492),
    ("SOX", "Santa Ana Mountains CA", 33.818, -117.636),
    ("SRX", "Fort Smith AR", 35.290, -94.362),
    ("TFX", "Great Falls MT", 47.460, -111.385),
    ("TWX", "Topeka KS", 38.997, -96.232),
    ("TYX", "Fort Drum NY", 43.756, -75.680),
    ("UDX", "Rapid City SD", 44.125, -102.830),
    ("UEX", "Grand Island NE", 40.321, -98.442),
    ("VAX", "Moody AFB GA", 30.890, -83.002),
    ("VBX", "Vandenberg AFB CA", 34.839, -120.398),
    ("VNX", "Vance AFB OK", 36.741, -98.127),
    ("VTX", "Los Angeles CA", 34.412, -119.179),
    ("YUX", "Yuma AZ", 32.495, -114.657),
];

/// The NEXRAD site nearest to a point: (id, name, lat, lon).
/// Height of the 0.5° radar beam centre above the radar over a point, in feet, with the site id
/// and distance in miles (standard 4/3 earth-radius refraction model).
pub fn beam_height(lat: f64, lon: f64) -> (&'static str, f64, f64) {
    let (id, _, slat, slon) = nearest_site(lat, lon);
    let r_km = crate::map::geo::haversine_km(slat, slon, lat, lon);
    let re: f64 = 6371.0 * 4.0 / 3.0;
    let elev = 0.5f64.to_radians();
    let h_km = (r_km.powi(2) + re.powi(2) + 2.0 * r_km * re * elev.sin()).sqrt() - re;
    (id, h_km * 3280.84, r_km * 0.621371)
}

pub fn nearest_site(lat: f64, lon: f64) -> (&'static str, &'static str, f64, f64) {
    NEXRAD_SITES
        .iter()
        .copied()
        .min_by(|a, b| {
            let da = (a.2 - lat).powi(2) + ((a.3 - lon) * (lat.to_radians().cos())).powi(2);
            let db = (b.2 - lat).powi(2) + ((b.3 - lon) * (lat.to_radians().cos())).powi(2);
            da.total_cmp(&db)
        })
        .unwrap_or(NEXRAD_SITES[0])
}

impl Alert {
    /// A practice warning from Severe Center > Rehearse (never a real NWS alert).
    pub fn is_practice(&self) -> bool {
        self.id.starts_with("TEST-")
    }

    /// Parsed expiry time.
    pub fn expires_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.expires
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|t| t.with_timezone(&chrono::Utc))
    }

    /// Minutes until expiry (negative once expired).
    pub fn minutes_left(&self) -> Option<i64> {
        self.expires_at().map(|t| (t - chrono::Utc::now()).num_minutes())
    }

    /// When it ends, in words: "until 4:45 PM EDT", "until tomorrow at 11 AM EDT",
    /// "until Sat at 11 AM EDT", or "expired 4:45 PM".
    pub fn expires_label(&self, clock_24h: bool) -> Option<String> {
        let t = self.expires_at()?;
        let words = when_words(t, clock_24h);
        Some(if t < chrono::Utc::now() { format!("expired {words}") } else { format!("until {words}") })
    }

    /// Time left, scaled to its size: "45 min", "2 hr 15 min", "3 days".
    pub fn time_left_label(&self) -> Option<String> {
        self.minutes_left().filter(|m| *m >= 0).map(duration_words)
    }

    /// "Particularly Dangerous Situation" / "Tornado Emergency" / destructive wording.
    pub fn threat_tag(&self) -> Option<&'static str> {
        let text = format!("{} {} {}", self.headline, self.description, self.instruction).to_uppercase();
        if text.contains("TORNADO EMERGENCY") {
            Some("TORNADO EMERGENCY")
        } else if text.contains("FLASH FLOOD EMERGENCY") {
            Some("FLASH FLOOD EMERGENCY")
        } else if text.contains("PARTICULARLY DANGEROUS SITUATION") {
            Some("PARTICULARLY DANGEROUS SITUATION")
        } else if text.contains("DESTRUCTIVE STORM") || text.contains("DAMAGE THREAT...DESTRUCTIVE") {
            // Only the NWS destructive-thunderstorm wording; "destructive waves" (High Surf) is not a storm tag.
            Some("DESTRUCTIVE STORM")
        } else if text.contains("TORNADO...OBSERVED") || text.contains("TORNADO DAMAGE THREAT...CONSIDERABLE") {
            Some("CONFIRMED TORNADO")
        } else {
            None
        }
    }

    /// Short code for on-map labels: TOR, SVR, FFW, …
    pub fn short_code(&self) -> String {
        let e = self.event.to_lowercase();
        let code = if e.contains("tornado warning") {
            "TOR"
        } else if e.contains("severe thunderstorm warning") {
            "SVR"
        } else if e.contains("flash flood warning") {
            "FFW"
        } else if e.contains("extreme wind") {
            "EWW"
        } else if e.contains("special marine") {
            "SMW"
        } else if e.contains("snow squall") {
            "SQW"
        } else if e.contains("tornado watch") {
            "TOA"
        } else if e.contains("severe thunderstorm watch") {
            "SVA"
        } else {
            return self.event.split_whitespace().filter_map(|w| w.chars().next()).collect::<String>().to_uppercase();
        };
        code.to_string()
    }

    pub fn is_warning(&self) -> bool {
        self.event.to_lowercase().contains("warning")
    }

    /// Whether a (lat, lon) point is inside any of the alert's polygons.
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        self.polygons.iter().any(|ring| point_in_ring(ring, lon, lat))
    }

    /// Area-weighted-ish centre (mean of the first ring's vertices), as (lat, lon).
    pub fn centroid(&self) -> Option<(f64, f64)> {
        let ring = self.polygons.iter().max_by_key(|r| r.len())?;
        if ring.is_empty() {
            return None;
        }
        let n = ring.len() as f64;
        let lon = ring.iter().map(|p| p.0).sum::<f64>() / n;
        let lat = ring.iter().map(|p| p.1).sum::<f64>() / n;
        Some((lat, lon))
    }
}

/// Even-odd point-in-polygon on (lon, lat) rings.
pub fn point_in_ring(ring: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut inside = false;
    let mut j = ring.len().wrapping_sub(1);
    for i in 0..ring.len() {
        let (xi, yi) = ring[i];
        let (xj, yj) = ring[j];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi + f64::EPSILON) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// ---------------------------------------------------------------------------------------------
// Alert classes, storm motion from NWS warnings
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertClass {
    Warning,
    Watch,
    Advisory,
    Statement,
}

/// Storm motion from a warning's `eventMotionDescription`: position, heading toward, speed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StormMotion {
    pub lat: f64,
    pub lon: f64,
    /// Direction the storm is moving TOWARD, degrees.
    pub heading_deg: f64,
    pub speed_mph: f64,
}

impl Alert {
    pub fn class(&self) -> AlertClass {
        let e = self.event.to_lowercase();
        if e.contains("warning") || e.contains("emergency") {
            AlertClass::Warning
        } else if e.contains("watch") {
            AlertClass::Watch
        } else if e.contains("advisory") {
            AlertClass::Advisory
        } else {
            AlertClass::Statement
        }
    }

    /// Short-fused convective / flash-flood warnings: the ones that get countdowns, sounds and
    /// the command-center queue.
    pub fn is_short_fused(&self) -> bool {
        let e = self.event.to_lowercase();
        [
            "tornado warning",
            "severe thunderstorm warning",
            "flash flood warning",
            "extreme wind warning",
            "snow squall warning",
            "special marine warning",
            "dust storm warning",
        ]
        .iter()
        .any(|k| e.contains(k))
    }

    /// Storm position and motion NWS puts in the warning ("...storm...245DEG...30KT...38.1,-87.2").
    pub fn motion(&self) -> Option<StormMotion> {
        motion_from_text(self.motion_text.as_deref()?)
    }

    /// Towns inside the warning (largest first, nearest the storm first among equals).
    pub fn towns(&self) -> Vec<String> {
        self.places_inside().into_iter().map(|p| p.name).collect()
    }

    /// Places inside the warning polygon(s), largest first.
    pub fn places_inside(&self) -> Vec<crate::places::Place> {
        let (mut n, mut s, mut w, mut e) = (f64::MIN, f64::MAX, f64::MAX, f64::MIN);
        for ring in &self.polygons {
            for (lon, lat) in ring {
                n = n.max(*lat);
                s = s.min(*lat);
                w = w.min(*lon);
                e = e.max(*lon);
            }
        }
        if n < s {
            return Vec::new();
        }
        crate::places::in_box(s, n, w, e)
            .into_iter()
            .filter(|p| self.contains(p.lat, p.lon))
            .collect()
    }

    /// Rough population inside the warning (towns of 1,000+ people only).
    pub fn population(&self) -> u64 {
        self.places_inside().iter().map(|p| p.population as u64).sum()
    }
}

pub fn motion_from_text(text: &str) -> Option<StormMotion> {
    let upper = text.to_uppercase();
    let parts: Vec<&str> = upper.split("...").collect();
    let deg = parts.iter().find_map(|p| p.trim().strip_suffix("DEG").and_then(|v| v.trim().parse::<f64>().ok()))?;
    let kt = parts.iter().find_map(|p| p.trim().strip_suffix("KT").and_then(|v| v.trim().parse::<f64>().ok()))?;
    let pos = parts.last()?.split_whitespace().next()?;
    let (lat, lon) = pos.split_once(',')?;
    Some(StormMotion {
        lat: lat.trim().parse().ok()?,
        lon: lon.trim().parse().ok()?,
        // NWS gives the direction the storm moves FROM.
        heading_deg: (deg + 180.0) % 360.0,
        speed_mph: kt * 1.15078,
    })
}

// ---------------------------------------------------------------------------------------------
// NWS Local Storm Reports (Iowa Environmental Mesonet GeoJSON)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StormReport {
    pub lat: f64,
    pub lon: f64,
    /// IEM type code: T tornado, H hail, G/D/N wind, F/E flood, S snow, …
    pub kind: String,
    pub text: String,
    pub magnitude: String,
    pub city: String,
    /// "Vanderburgh, IN" (county and state), when the report gives them.
    pub county: String,
    pub source: String,
    pub remark: String,
    /// Unix seconds.
    pub time: i64,
}

impl StormReport {
    /// Short marker letter and colour (RGB).
    pub fn marker(&self) -> (&'static str, [u8; 3]) {
        match self.kind.as_str() {
            "T" | "C" => ("T", [230, 30, 40]),
            "H" => ("H", [40, 200, 80]),
            "G" | "D" | "M" | "N" | "W" => ("W", [40, 120, 240]),
            "F" | "E" | "R" => ("F", [30, 200, 220]),
            "S" | "Z" | "I" | "B" => ("S", [200, 210, 255]),
            "L" => ("L", [255, 220, 40]),
            _ => ("•", [200, 200, 200]),
        }
    }
}

/// Report counts for the last `hours`: per marker letter (T, H, W, F, S, L, •) and the busiest
/// counties, most reports first.
pub fn report_counts(reports: &[StormReport], hours: u32, hidden: &str) -> (Vec<(&'static str, usize)>, Vec<(String, usize)>) {
    let cutoff = chrono::Utc::now().timestamp() - hours.max(1) as i64 * 3600;
    let mut by_kind: Vec<(&'static str, usize)> = Vec::new();
    let mut by_county: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for r in reports.iter().filter(|r| r.time >= cutoff && !hidden.contains(r.marker().0)) {
        let letter = r.marker().0;
        match by_kind.iter_mut().find(|(k, _)| *k == letter) {
            Some((_, n)) => *n += 1,
            None => by_kind.push((letter, 1)),
        }
        if !r.county.is_empty() {
            *by_county.entry(r.county.clone()).or_default() += 1;
        }
    }
    by_kind.sort_by(|a, b| b.1.cmp(&a.1));
    let mut counties: Vec<(String, usize)> = by_county.into_iter().collect();
    counties.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    (by_kind, counties)
}

pub fn parse_lsr(value: &serde_json::Value) -> Vec<StormReport> {
    value
        .get("features")
        .and_then(|f| f.as_array())
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let p = f.get("properties")?;
            let s = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
            let time = chrono::DateTime::parse_from_rfc3339(&s("valid").replace("Z", "+00:00"))
                .map(|t| t.timestamp())
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(&s("valid"), "%Y-%m-%dT%H:%M:%SZ").map(|t| t.and_utc().timestamp()))
                .unwrap_or(0);
            Some(StormReport {
                lat: p.get("lat")?.as_f64()?,
                lon: p.get("lon")?.as_f64()?,
                kind: s("type"),
                text: s("typetext"),
                magnitude: p
                    .get("magnitude")
                    .map(|m| m.as_str().map(String::from).unwrap_or_else(|| m.to_string()))
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
                city: s("city"),
                county: match (s("county"), s("st")) {
                    (c, st) if !c.is_empty() && !st.is_empty() => format!("{c}, {st}"),
                    (c, _) => c,
                },
                source: s("source"),
                remark: s("remark"),
                time,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// INDOT traffic events (DCWeather API /api/indot/events)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct RoadEvent {
    pub kind: String,
    pub label: String,
    pub color: String,
    pub headline: String,
    pub route: String,
    pub lat: f64,
    pub lon: f64,
}

impl RoadEvent {
    /// Events that matter on a weather broadcast (not routine roadwork / restrictions).
    pub fn is_hazard(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "closure" | "incident" | "obstruction" | "traffic-condition" | "warning" | "weather" | "mobile-situation"
        )
    }

    pub fn rgb(&self) -> [u8; 3] {
        match self.color.as_str() {
            "red" => [230, 40, 40],
            "orange" => [245, 140, 30],
            "yellow" => [240, 210, 40],
            "purple" => [170, 80, 220],
            "blue" => [60, 140, 240],
            _ => [170, 170, 170],
        }
    }
}

pub fn parse_road_events(value: &serde_json::Value) -> Vec<RoadEvent> {
    value
        .get("events")
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let s = |k: &str| e.get(k).and_then(|v| v.as_str()).unwrap_or_default().to_string();
            Some(RoadEvent {
                kind: s("type"),
                label: s("typeLabel"),
                color: s("typeColor"),
                headline: s("headline"),
                route: s("route"),
                lat: e.get("lat")?.as_f64()?,
                lon: e.get("lon")?.as_f64()?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_site_covers_the_west() {
        // Eugene OR used to snap to Denver (FTG) because the table stopped at the Rockies.
        assert_eq!(nearest_site(44.052, -123.087).0, "RTX");
        assert_eq!(nearest_site(40.76, -111.89).0, "MTX");
        // Home area is unchanged.
        assert_eq!(nearest_site(38.39, -86.93).0, "VWX");
        let (_, _, mi) = beam_height(44.052, -123.087);
        assert!(mi < 150.0, "Eugene should be well within a site's range, got {mi} mi");
    }

    #[test]
    fn destructive_tag_only_for_storms() {
        let surf = Alert {
            event: "High Surf Warning".into(),
            description: "Large breaking waves of 20 to 25 feet. Destructive waves may damage docks.".into(),
            ..Default::default()
        };
        assert_eq!(surf.threat_tag(), None);
        let svr = Alert {
            event: "Severe Thunderstorm Warning".into(),
            description: "THUNDERSTORM DAMAGE THREAT...DESTRUCTIVE".into(),
            ..Default::default()
        };
        assert_eq!(svr.threat_tag(), Some("DESTRUCTIVE STORM"));
    }
}
