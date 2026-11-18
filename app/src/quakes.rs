//! Earthquakes from the USGS Earthquake Hazards Program real-time GeoJSON feeds (keyless).
//! NOAA does not publish earthquake catalogs; USGS is the federal source of record, and NWS
//! itself points to it. Two feeds are merged by event id: magnitude 2.5+ for the past month
//! (US and world) and every located quake for the past week (catches small local ones).

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

const FEED: &str = "https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Quake {
    pub id: String,
    pub mag: f32,
    /// USGS place text ("5 km NE of Mount Carmel, Illinois").
    pub place: String,
    /// Unix seconds.
    pub time: i64,
    pub lat: f64,
    pub lon: f64,
    pub depth_km: f32,
    /// "Did You Feel It?" reports.
    #[serde(default)]
    pub felt: Option<u32>,
    /// PAGER alert level: "green", "yellow", "orange", "red" or empty.
    #[serde(default)]
    pub alert: String,
    #[serde(default)]
    pub tsunami: bool,
}

impl Quake {
    pub fn age_secs(&self, now: i64) -> i64 {
        (now - self.time).max(0)
    }

    /// Distance from a point in miles.
    pub fn miles_from(&self, lat: f64, lon: f64) -> f64 {
        crate::map::geo::haversine_km(lat, lon, self.lat, self.lon) / 1.609
    }
}

/// USGS / news-style magnitude class.
pub fn class(mag: f32) -> &'static str {
    match mag {
        m if m >= 8.0 => "Great",
        m if m >= 7.0 => "Major",
        m if m >= 6.0 => "Strong",
        m if m >= 5.0 => "Moderate",
        m if m >= 4.0 => "Light",
        m if m >= 3.0 => "Minor",
        _ => "Very minor",
    }
}

/// What people near the epicenter usually notice, in plain words.
pub fn felt_words(mag: f32) -> &'static str {
    match mag {
        m if m >= 7.0 => "Serious damage over a wide area",
        m if m >= 6.0 => "Damage likely near the center",
        m if m >= 5.0 => "Felt widely; damage possible to weaker buildings",
        m if m >= 4.0 => "Felt by most nearby; rattling, rarely damage",
        m if m >= 3.0 => "Often felt nearby; no damage",
        _ => "Usually not felt",
    }
}

/// Marker colour by age (the USGS map convention): past hour, day, week, older.
pub fn age_rgb(age_secs: i64) -> (u8, u8, u8) {
    match age_secs {
        a if a < 3600 => (232, 32, 32),
        a if a < 86_400 => (255, 140, 0),
        a if a < 7 * 86_400 => (255, 214, 0),
        _ => (205, 214, 226),
    }
}

/// Marker radius in pixels at `scale` 1: grows with magnitude (energy-like, but kept readable).
pub fn radius(mag: f32) -> f32 {
    (3.0 + 2.2 * 1.5f32.powf(mag.clamp(0.0, 9.0) - 1.0)).min(60.0)
}

pub fn age_label(age_secs: i64) -> String {
    match age_secs {
        a if a < 3600 => format!("{} min ago", (a / 60).max(1)),
        a if a < 86_400 => format!("{} h ago", a / 3600),
        a => format!("{} days ago", a / 86_400),
    }
}

async fn feed(name: &str) -> Result<Vec<Quake>> {
    let v = crate::net::get_json(&format!("{FEED}/{name}.geojson")).await?;
    let feats = v.get("features").and_then(|f| f.as_array()).ok_or_else(|| anyhow!("USGS {name}: no features"))?;
    Ok(feats.iter().filter_map(parse_feature).collect())
}

fn parse_feature(f: &serde_json::Value) -> Option<Quake> {
    let p = f.get("properties")?;
    let c = f.pointer("/geometry/coordinates")?.as_array()?;
    let kind = p.get("type").and_then(|t| t.as_str()).unwrap_or("earthquake");
    if kind != "earthquake" {
        // Quarry blasts and explosions are in the same feed.
        return None;
    }
    Some(Quake {
        id: f.get("id")?.as_str()?.to_string(),
        mag: p.get("mag")?.as_f64()? as f32,
        place: p.get("place").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        time: p.get("time")?.as_i64()? / 1000,
        lon: c.first()?.as_f64()?,
        lat: c.get(1)?.as_f64()?,
        depth_km: c.get(2).and_then(|d| d.as_f64()).unwrap_or(0.0) as f32,
        felt: p.get("felt").and_then(|x| x.as_u64()).map(|n| n as u32),
        alert: p.get("alert").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        tsunami: p.get("tsunami").and_then(|x| x.as_i64()).unwrap_or(0) == 1,
    })
}

/// Every quake from the two feeds, newest first.
pub async fn fetch() -> Result<Vec<Quake>> {
    let (month, week) = futures_util::join!(feed("2.5_month"), feed("all_week"));
    let mut out = month?;
    if let Ok(week) = week {
        let seen: std::collections::HashSet<String> = out.iter().map(|q| q.id.clone()).collect();
        out.extend(week.into_iter().filter(|q| !seen.contains(&q.id)));
    }
    out.sort_by(|a, b| b.time.cmp(&a.time));
    Ok(out)
}

/// Quakes in the last `days` of at least `min_mag`, within `miles` of a point (None = anywhere).
pub fn select<'a>(quakes: &'a [Quake], now: i64, days: u32, min_mag: f32, near: Option<(f64, f64, f64)>) -> Vec<&'a Quake> {
    quakes.iter()
        .filter(|q| q.age_secs(now) <= days as i64 * 86_400 && q.mag >= min_mag)
        .filter(|q| near.is_none_or(|(lat, lon, miles)| q.miles_from(lat, lon) <= miles))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_usgs_feature_and_skips_blasts() {
        let quake: serde_json::Value = serde_json::from_str(r#"{"type":"Feature","properties":{"mag":3.1,"place":"5 km NE of Mount Carmel, Illinois","time":1763300000000,"felt":42,"alert":null,"tsunami":0,"type":"earthquake"},"geometry":{"type":"Point","coordinates":[-87.7,38.43,11.2]},"id":"nm60000000"}"#).unwrap();
        let q = parse_feature(&quake).unwrap();
        assert_eq!((q.mag, q.time, q.felt, q.depth_km), (3.1, 1_763_300_000, Some(42), 11.2));
        let blast: serde_json::Value = serde_json::from_str(r#"{"properties":{"mag":1.9,"time":1,"type":"quarry blast"},"geometry":{"coordinates":[-87,38,0]},"id":"x"}"#).unwrap();
        assert!(parse_feature(&blast).is_none());
    }

    #[test]
    fn classes_and_markers_grow_with_magnitude() {
        assert_eq!(class(4.2), "Light");
        assert_eq!(class(7.1), "Major");
        assert!(radius(6.0) > radius(4.0) && radius(4.0) > radius(2.0));
        assert!(radius(9.5) <= 60.0);
        assert_eq!(age_rgb(60), (232, 32, 32));
    }

    #[test]
    fn selects_by_days_magnitude_and_distance() {
        let now = 10 * 86_400;
        let mk = |id: &str, mag: f32, days_ago: i64, lat: f64| Quake { id: id.into(), mag, time: now - days_ago * 86_400, lat, lon: -87.0, ..Default::default() };
        let all = vec![mk("near", 2.0, 1, 38.4), mk("far", 5.0, 1, 10.0), mk("old", 4.0, 9, 38.4)];
        let near: Vec<_> = select(&all, now, 7, 0.0, Some((38.4, -87.0, 300.0))).iter().map(|q| q.id.clone()).collect();
        assert_eq!(near, vec!["near"]);
        assert_eq!(select(&all, now, 30, 4.5, None).len(), 1);
    }
}
