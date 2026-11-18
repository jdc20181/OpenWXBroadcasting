//! NOAA feeds that have no DCWeather route: river gauges and hydrographs (NWPS), WPC rainfall
//! forecasts (QPF Day 1 / 1-3 / 1-7) and the WPC Excessive Rainfall Outlook (Days 1-3).
//! All are keyless public services; areas are drawn like SPC outlook areas.

use anyhow::Result;

use crate::data::{collect_rings, SpcArea};

const NOAA: &str = "https://mapservices.weather.noaa.gov";

// ---------------------------------------------------------------------------------------------
// Rivers
// ---------------------------------------------------------------------------------------------

/// Gauges rising fast: (location, feet per hour), from consecutive readings.
pub fn rapid_rises(gauges: &[RiverGauge]) -> Vec<(String, f64)> {
    static PREV: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (i64, f64)>>> = std::sync::OnceLock::new();
    let prev = PREV.get_or_init(Default::default);
    prev.lock().map(|mut map| rapid_rises_at(gauges, chrono::Utc::now().timestamp(), &mut map)).unwrap_or_default()
}

fn rapid_rises_at(gauges: &[RiverGauge], now: i64, map: &mut std::collections::HashMap<String, (i64, f64)>) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    for g in gauges {
        let Some(stage) = g.observed.filter(|s| s.is_finite()) else { continue };
        let stage = if g.units.eq_ignore_ascii_case("m") { stage * 3.28084 } else { stage };
        match map.get(&g.lid).copied() {
            Some((t0, s0)) if now > t0 && now - t0 < 720 => {
                // Retain the baseline across frequent polls until 12 minutes have elapsed.
                // Resetting it on every poll prevented any rise detection at five-minute cadence.
                let _ = s0;
                continue;
            }
            Some((t0, s0)) if now > t0 && now - t0 <= 7200 => {
                let rate = (stage - s0) / ((now - t0) as f64 / 3600.0);
                if rate >= 1.0 && near_flood(g, stage, rate) {
                    out.push((format!("{} ({})", g.location, g.waterbody), rate));
                }
            }
            _ => (),
        }
        map.insert(g.lid.clone(), (now, stage));
    }
    map.retain(|_, (t, _)| now - *t <= 7200);
    out
}

/// Whether a rising gauge is worth an alert: already at action stage or worse (now or
/// forecast), or close enough that three more hours at this rate reach action / flood stage.
/// Gauges with no stages on file (often regulated sites below dams) only alert when the NWS
/// category says so.
fn near_flood(g: &RiverGauge, stage_ft: f64, rate: f64) -> bool {
    if RiverGauge::severity(&g.status) >= 1 || RiverGauge::severity(&g.forecast_status) >= 1 {
        return true;
    }
    let feet = |v: Option<f64>| v.map(|v| if g.units.eq_ignore_ascii_case("m") { v * 3.28084 } else { v });
    match feet(g.action).or(feet(g.flood)) {
        Some(threshold) => stage_ft + rate * 3.0 >= threshold,
        None => false,
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RiverGauge {
    /// NWS location id, e.g. "JSPI3".
    pub lid: String,
    pub location: String,
    pub waterbody: String,
    pub state: String,
    pub lat: f64,
    pub lon: f64,
    /// no_flooding, action, minor, moderate, major, low_threshold, obs_not_current…
    pub status: String,
    pub observed: Option<f64>,
    pub forecast: Option<f64>,
    /// Forecast category (same words as `status`) when a forecast exists.
    pub forecast_status: String,
    pub action: Option<f64>,
    pub flood: Option<f64>,
    pub moderate: Option<f64>,
    pub major: Option<f64>,
    pub units: String,
    pub url: String,
}

impl RiverGauge {
    /// Plain words for a status code.
    pub fn status_words(code: &str) -> &'static str {
        match code {
            "major" => "Major flooding",
            "moderate" => "Moderate flooding",
            "minor" => "Minor flooding",
            "action" => "Near flood stage",
            "no_flooding" => "Below flood stage",
            "low_threshold" => "Low water",
            _ => "No current data",
        }
    }

    /// Marker colour (RGB) for a status code, NWS flood-category colours.
    pub fn status_rgb(code: &str) -> [u8; 3] {
        match code {
            "major" => [204, 51, 255],
            "moderate" => [255, 0, 0],
            "minor" => [255, 153, 0],
            "action" => [255, 255, 0],
            "no_flooding" => [0, 204, 0],
            _ => [150, 150, 150],
        }
    }

    /// 0 none … 4 major, for sorting the worst first.
    pub fn severity(code: &str) -> u8 {
        match code {
            "major" => 4,
            "moderate" => 3,
            "minor" => 2,
            "action" => 1,
            _ => 0,
        }
    }

    pub fn worst_status(&self) -> &str {
        if Self::severity(&self.forecast_status) > Self::severity(&self.status) {
            &self.forecast_status
        } else {
            &self.status
        }
    }

    pub fn summary(&self) -> String {
        let unit = if self.units.is_empty() { "ft" } else { &self.units };
        let obs = self.observed.map(|v| format!("{v:.1} {unit}")).unwrap_or_else(|| "no reading".into());
        let flood = self.flood.map(|v| format!(", flood stage {v:.0} {unit}")).unwrap_or_default();
        let crest = self.forecast.map(|v| format!(", forecast {v:.1} {unit}")).unwrap_or_default();
        format!("{} at {}: {obs}{flood}{crest}", self.waterbody, self.location)
    }
}

fn num(v: Option<&serde_json::Value>) -> Option<f64> {
    let v = v?;
    let n = v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))?;
    (n > -900.0).then_some(n)
}

fn text(v: Option<&serde_json::Value>) -> String {
    v.and_then(|x| x.as_str()).unwrap_or_default().trim().to_string()
}

/// River gauges in a box (observed stage + full-period forecast), worst first.
pub async fn rivers(south: f64, north: f64, west: f64, east: f64) -> Result<Vec<RiverGauge>> {
    let q = |layer: u32| {
        format!(
            "{NOAA}/eventdriven/rest/services/water/riv_gauges/MapServer/{layer}/query?where=1%3D1&outFields=*&returnGeometry=false&f=json&geometry={west},{south},{east},{north}&geometryType=esriGeometryEnvelope&inSR=4326&spatialRel=esriSpatialRelIntersects&resultRecordCount=400"
        )
    };
    let observed = crate::net::get_json(&q(0)).await?;
    let forecast = crate::net::get_json(&q(15)).await.unwrap_or_default();
    let mut out: Vec<RiverGauge> = observed
        .get("features")
        .and_then(|f| f.as_array())
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let a = f.get("attributes")?;
            Some(RiverGauge {
                lid: text(a.get("gaugelid")),
                location: text(a.get("location")),
                waterbody: text(a.get("waterbody")),
                state: text(a.get("state")),
                lat: num(a.get("latitude"))?,
                lon: num(a.get("longitude"))?,
                status: text(a.get("status")),
                observed: num(a.get("observed")),
                forecast: None,
                forecast_status: String::new(),
                action: num(a.get("action")),
                flood: num(a.get("flood")),
                moderate: num(a.get("moderate")),
                major: num(a.get("major")),
                units: text(a.get("units")),
                url: text(a.get("url")),
            })
        })
        .collect();
    for f in forecast.get("features").and_then(|f| f.as_array()).into_iter().flatten() {
        let Some(a) = f.get("attributes") else { continue };
        let lid = text(a.get("gaugelid"));
        if let Some(g) = out.iter_mut().find(|g| g.lid == lid) {
            g.forecast = num(a.get("forecast"));
            g.forecast_status = text(a.get("status"));
        }
    }
    out.sort_by(|a, b| {
        RiverGauge::severity(b.worst_status())
            .cmp(&RiverGauge::severity(a.worst_status()))
            .then(a.location.cmp(&b.location))
    });
    Ok(out)
}

/// One hydrograph point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StagePoint {
    /// Unix seconds.
    pub time: i64,
    pub stage: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hydrograph {
    pub lid: String,
    pub observed: Vec<StagePoint>,
    pub forecast: Vec<StagePoint>,
}

impl Hydrograph {
    /// Highest forecast stage and its time (the crest).
    pub fn crest(&self) -> Option<StagePoint> {
        self.forecast.iter().copied().max_by(|a, b| a.stage.total_cmp(&b.stage))
    }
}

/// Observed and forecast stage series for a gauge (NWPS API).
pub async fn hydrograph(lid: &str) -> Result<Hydrograph> {
    let url = format!("https://api.water.noaa.gov/nwps/v1/gauges/{}/stageflow", lid.to_lowercase());
    let v = crate::net::get_json(&url).await?;
    let series = |key: &str| -> Vec<StagePoint> {
        v.get(key)
            .and_then(|o| o.get("data"))
            .and_then(|d| d.as_array())
            .into_iter()
            .flatten()
            .filter_map(|p| {
                let t = chrono::DateTime::parse_from_rfc3339(p.get("validTime")?.as_str()?).ok()?.timestamp();
                let s = p.get("primary")?.as_f64()?;
                (s > -900.0).then_some(StagePoint { time: t, stage: s })
            })
            .collect()
    };
    let mut observed = series("observed");
    // Last 72 hours of observations are plenty for a broadcast hydrograph.
    let cutoff = chrono::Utc::now().timestamp() - 72 * 3600;
    observed.retain(|p| p.time >= cutoff);
    Ok(Hydrograph { lid: lid.to_string(), observed, forecast: series("forecast") })
}

// ---------------------------------------------------------------------------------------------
// WPC rainfall forecast (QPF) and Excessive Rainfall Outlook
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum QpfPeriod {
    #[default]
    Day1,
    Days1to3,
    Days1to7,
}

impl QpfPeriod {
    pub const ALL: [QpfPeriod; 3] = [QpfPeriod::Day1, QpfPeriod::Days1to3, QpfPeriod::Days1to7];

    fn layer(self) -> u32 {
        match self {
            QpfPeriod::Day1 => 1,
            QpfPeriod::Days1to3 => 9,
            QpfPeriod::Days1to7 => 11,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            QpfPeriod::Day1 => "Day 1",
            QpfPeriod::Days1to3 => "3-day",
            QpfPeriod::Days1to7 => "7-day",
        }
    }
}

/// WPC rainfall colours by inches (close to the WPC product).
fn qpf_color(inches: f64) -> &'static str {
    match inches {
        x if x >= 10.0 => "#FF00FF",
        x if x >= 7.0 => "#C000C0",
        x if x >= 5.0 => "#8B0000",
        x if x >= 4.0 => "#CC0000",
        x if x >= 3.0 => "#FF3300",
        x if x >= 2.5 => "#FF6600",
        x if x >= 2.0 => "#FF9900",
        x if x >= 1.5 => "#FFCC00",
        x if x >= 1.0 => "#FFFF00",
        x if x >= 0.75 => "#66CC00",
        x if x >= 0.5 => "#009900",
        x if x >= 0.25 => "#00CC66",
        x if x >= 0.1 => "#00CCFF",
        _ => "#99CCFF",
    }
}

async fn query_areas(service: &str, layer: u32, fields: &str) -> Result<Vec<serde_json::Value>> {
    let url = format!("{NOAA}/vector/rest/services/{service}/MapServer/{layer}/query?where=1%3D1&outFields={fields}&returnGeometry=true&f=geojson");
    let v = crate::net::get_json(&url).await?;
    Ok(v.get("features").and_then(|f| f.as_array()).cloned().unwrap_or_default())
}

/// WPC rainfall forecast contours for a period, lightest first.
pub async fn qpf(period: QpfPeriod) -> Result<Vec<SpcArea>> {
    let days = match period {
        QpfPeriod::Day1 => 1,
        QpfPeriod::Days1to3 => 3,
        QpfPeriod::Days1to7 => 7,
    };
    let path = format!("outlooks/qpf?days={days}");
    let legacy = format!("{NOAA}/vector/rest/services/precip/wpc_qpf/MapServer/{}/query?where=1%3D1&outFields=qpf&returnGeometry=true&f=geojson", period.layer());
    let value: serde_json::Value = crate::net::broadcaster_product(&path, &legacy, std::time::Duration::from_secs(35)).await?.json().await?;
    let features = value.get("features").and_then(|f| f.as_array()).ok_or_else(|| anyhow::anyhow!("QPF feed returned no GeoJSON features"))?;
    let mut areas: Vec<SpcArea> = features
        .iter()
        .filter_map(|f| {
            let inches = num(f.get("properties")?.get("qpf"))?;
            let mut rings = Vec::new();
            collect_rings(f.get("geometry")?, &mut rings);
            let color = qpf_color(inches);
            Some(SpcArea {
                label: if inches < 1.0 { format!("{inches:.2}\"").replacen("0.", ".", 1) } else { format!("{inches:.1}\"") },
                rank: (inches * 100.0) as i32,
                stroke: color.into(),
                fill: color.into(),
                fill_opacity: 0.45,
                rings,
            })
        })
        .collect();
    areas.sort_by_key(|a| a.rank);
    Ok(areas)
}

/// Excessive Rainfall Outlook areas for Day 1-3, lowest risk first.
pub async fn excessive_rain(day: u8) -> Result<Vec<SpcArea>> {
    let layer = day.clamp(1, 3) as u32 - 1;
    let mut areas: Vec<SpcArea> = query_areas("hazards/wpc_precip_hazards", layer, "outlook,dn")
        .await?
        .iter()
        .filter_map(|f| {
            let p = f.get("properties")?;
            let dn = num(p.get("dn")).unwrap_or(1.0) as i32;
            let outlook = text(p.get("outlook"));
            let (color, short) = match dn {
                4 => ("#FF00FF", "High"),
                3 => ("#FF0000", "Moderate"),
                2 => ("#FFFF00", "Slight"),
                _ => ("#00B050", "Marginal"),
            };
            let mut rings = Vec::new();
            collect_rings(f.get("geometry")?, &mut rings);
            Some(SpcArea {
                label: if outlook.is_empty() { short.to_string() } else { short.to_string() },
                rank: dn,
                stroke: color.into(),
                fill: color.into(),
                fill_opacity: 0.3,
                rings,
            })
        })
        .collect();
    areas.sort_by_key(|a| a.rank);
    Ok(areas)
}

/// Highest-ranked area containing a point (for "your rainfall" / "your risk" bindings).
pub fn area_at<'a>(areas: &'a [SpcArea], lat: f64, lon: f64) -> Option<&'a SpcArea> {
    areas
        .iter()
        .filter(|a| a.rings.iter().any(|r| crate::data::point_in_ring(r, lon, lat)))
        .max_by_key(|a| a.rank)
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[test]
    fn rapid_rise_survives_five_minute_polling() {
        let mut history = std::collections::HashMap::new();
        // Action stage 14 ft: 11 ft rising 4 ft/hr gets there within three hours.
        let mut gauge = RiverGauge { lid: "test".into(), observed: Some(10.), units: "ft".into(), action: Some(14.), ..Default::default() };
        assert!(rapid_rises_at(&[gauge.clone()], 0, &mut history).is_empty());
        gauge.observed = Some(10.5);
        assert!(rapid_rises_at(&[gauge.clone()], 300, &mut history).is_empty());
        gauge.observed = Some(10.75);
        assert!(rapid_rises_at(&[gauge.clone()], 600, &mut history).is_empty());
        gauge.observed = Some(11.);
        let rises = rapid_rises_at(&[gauge], 900, &mut history);
        assert_eq!(rises.len(), 1); assert!((rises[0].1 - 4.).abs() < 1e-9);
    }

    /// A fast rise far below flood stage, or at a gauge with no stages on file (regulated
    /// sites below dams), is not an alert (CWXB-71).
    #[test]
    fn rapid_rise_needs_to_near_flood_stage() {
        for action in [Some(40.), None] {
            let mut history = std::collections::HashMap::new();
            let mut gauge = RiverGauge { lid: "dam".into(), observed: Some(10.), units: "ft".into(), action, status: "no_flooding".into(), ..Default::default() };
            assert!(rapid_rises_at(&[gauge.clone()], 0, &mut history).is_empty());
            gauge.observed = Some(11.);
            assert!(rapid_rises_at(&[gauge], 900, &mut history).is_empty(), "action {action:?}");
        }
    }
}
