//! Radar and satellite composite tiles, storm cells and GLM lightning from the configured
//! RainViewer-compatible upstream (`radar_upstream`, LibreWXR by default).
//!
//! `/api/radar?path=public/weather-maps.json` frame index, `/api/radar?path=v2/radar/...` tiles,
//! `/api/radar/storm-cells` cells enriched relative to a point, `/api/public/lightning.json`
//! strikes around the station's home point.

use std::collections::HashMap;

use axum::{
    extract::{Query, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};
use regex::Regex;
use serde_json::{json, Value};

use crate::{
    fetch::{ApiError, ApiResult, FetchOptions, DAY, LIVE},
    AppState,
};

const KM_PER_MILE: f64 = 1.609_344;
const LIGHTNING_MINUTES: u32 = 15;
const COMPASS: [&str; 16] = [
    "North", "North North East", "North East", "East North East", "East", "East South East", "South East", "South South East",
    "South", "South South West", "South West", "West South West", "West", "West North West", "North West", "North North West",
];

fn upstream(state: &AppState, path_and_query: &str) -> ApiResult<url::Url> {
    url::Url::parse(&format!("{}/{}", state.config.radar_upstream.trim_end_matches('/'), path_and_query.trim_start_matches('/')))
        .map_err(|_| ApiError::new(500, "radar_upstream is not a valid address"))
}

pub async fn radar(State(state): State<AppState>, Query(q): Query<HashMap<String, String>>) -> ApiResult<Response> {
    let path = q.get("path").cloned().unwrap_or_default();
    if path.is_empty() {
        return Err(ApiError::bad_request("Missing path query parameter."));
    }
    if path.contains("..") {
        return Err(ApiError::bad_request("Invalid proxy path."));
    }
    if path == "public/weather-maps.json" || Regex::new(r"^public/[\w./-]+$").unwrap().is_match(&path) {
        let entry = state.fetch.get(upstream(&state, &path)?, LIVE, FetchOptions { timeout: Some(std::time::Duration::from_secs(10)), ..Default::default() }).await
            .map_err(|e| if e.upstream == Some(404) { ApiError::new(404, "Not found") } else { ApiError::new(503, "Radar index temporarily unavailable.").retry(30) })?;
        return Ok(entry.into_response());
    }
    if !Regex::new(r"^v2/(radar|satellite)/[\w./-]+\.(webp|png)$").unwrap().is_match(&path) {
        return Err(ApiError::bad_request("Invalid proxy path."));
    }
    let mut extra = Vec::new();
    if let Some(arrows) = q.get("arrows").filter(|a| *a == "light" || *a == "dark") {
        extra.push(format!("arrows={arrows}"));
    }
    if q.get("lightning").is_some_and(|l| l == "bolts") {
        extra.push("lightning=bolts".into());
    }
    let target = if extra.is_empty() { path.clone() } else { format!("{path}?{}", extra.join("&")) };
    // A timestamped frame never changes once published.
    let entry = state.fetch.get(upstream(&state, &target)?, DAY, FetchOptions { timeout: Some(std::time::Duration::from_secs(15)), ..Default::default() }).await
        .map_err(|e| if e.upstream == Some(404) { ApiError::new(404, "Tile not found") } else { e })?;
    let mut response = entry.into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=300, must-revalidate"));
    Ok(response)
}

fn rad(d: f64) -> f64 {
    d.to_radians()
}

pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let a = (rad(lat2 - lat1) / 2.0).sin().powi(2) + rad(lat1).cos() * rad(lat2).cos() * (rad(lon2 - lon1) / 2.0).sin().powi(2);
    6371.0 * 2.0 * a.sqrt().asin()
}

pub fn bearing_deg(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let y = rad(lon2 - lon1).sin() * rad(lat2).cos();
    let x = rad(lat1).cos() * rad(lat2).sin() - rad(lat1).sin() * rad(lat2).cos() * rad(lon2 - lon1).cos();
    (y.atan2(x).to_degrees() + 360.0) % 360.0
}

fn compass(deg: f64) -> &'static str {
    COMPASS[((((deg % 360.0) + 360.0) % 360.0) / 22.5).round() as usize % 16]
}

/// Reflectivity -> plain-language intensity (hail wording is an estimate from dBZ alone).
pub fn describe_intensity(dbz: f64) -> Value {
    let (level, label, hail) = if !dbz.is_finite() {
        (0, "Unknown", None)
    } else if dbz >= 63.0 {
        (6, "Extreme", Some("large hail possible"))
    } else if dbz >= 57.0 {
        (5, "Intense", Some("hail likely"))
    } else if dbz >= 52.0 {
        (4, "Very heavy", Some("small hail possible"))
    } else if dbz >= 47.0 {
        (3, "Very heavy", None)
    } else if dbz >= 40.0 {
        (2, "Heavy", None)
    } else if dbz >= 30.0 {
        (1, "Moderate", None)
    } else {
        (0, "Light", None)
    };
    json!({ "level": level, "label": label, "hail": hail })
}

fn bbox_around(lat: f64, lon: f64, radius_km: f64) -> String {
    let d_lat = radius_km / 111.0;
    let d_lon = radius_km / (111.0 * rad(lat).cos().max(0.2));
    [lon - d_lon, lat - d_lat, lon + d_lon, lat + d_lat].map(|n| format!("{n:.3}")).join(",")
}

/// Where a cell's straight-line track passes closest to the reference point (None when it is
/// stationary, moving away or passing wide).
fn closest_approach(cell: &Value, ref_lat: f64, ref_lon: f64, cell_radius_km: f64) -> Value {
    let (Some(speed), Some(heading), Some(lat), Some(lon)) = (cell["motion_speed_kmh"].as_f64(), cell["motion_heading_deg"].as_f64(), cell["lat"].as_f64(), cell["lon"].as_f64()) else {
        return Value::Null;
    };
    if speed < 3.0 {
        return Value::Null;
    }
    let km_per_deg_lon = 111.32 * rad((lat + ref_lat) / 2.0).cos();
    let (rx, ry) = ((ref_lon - lon) * km_per_deg_lon, (ref_lat - lat) * 110.57);
    let (vx, vy) = (speed * rad(heading).sin(), speed * rad(heading).cos());
    let t = (rx * vx + ry * vy) / (speed * speed);
    if t <= 0.0 || t > 3.0 {
        return Value::Null;
    }
    let miss = (rx - vx * t).hypot(ry - vy * t);
    if miss > (cell_radius_km + 5.0).max(8.0) {
        return Value::Null;
    }
    json!({ "etaMinutes": (t * 60.0).round(), "missKm": (miss * 10.0).round() / 10.0 })
}

fn strikes(value: &Value) -> Vec<(f64, f64, i64)> {
    value["features"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let c = f["geometry"]["coordinates"].as_array()?;
            let ts = chrono::DateTime::parse_from_rfc3339(f["properties"]["utc"].as_str()?).ok()?.timestamp_millis();
            Some((c.get(1)?.as_f64()?, c.first()?.as_f64()?, ts))
        })
        .collect()
}

async fn lightning_near(state: &AppState, lat: f64, lon: f64, radius_km: f64, minutes: u32) -> ApiResult<Value> {
    let path = format!("v2/lightning?bbox={}&minutes={minutes}&limit=5000", bbox_around(lat, lon, radius_km));
    state.fetch.get(upstream(state, &path)?, LIVE, FetchOptions::default()).await?.json()
}

pub async fn storm_cells(State(state): State<AppState>, Query(q): Query<HashMap<String, String>>) -> ApiResult<Response> {
    let num = |k: &str| q.get(k).and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite());
    let (Some(lat), Some(lon)) = (num("lat"), num("lon")) else { return Err(ApiError::new(422, "lat and lon are required.")) };
    if lat.abs() > 90.0 || lon.abs() > 180.0 {
        return Err(ApiError::new(422, "lat and lon are required."));
    }
    let (ref_lat, ref_lon) = (num("refLat").unwrap_or(lat), num("refLon").unwrap_or(lon));
    let radius = num("radius").unwrap_or(250.0).clamp(25.0, 600.0);
    // Round so nearby views share one upstream request.
    let (q_lat, q_lon, q_radius) = ((lat * 2.0).round() / 2.0, (lon * 2.0).round() / 2.0, (radius / 50.0).ceil() * 50.0);
    let cells_path = format!("v2/storm-cells?lat={q_lat}&lon={q_lon}&radius_km={q_radius}&format=json");
    let (cells, lightning) = tokio::join!(
        async { state.fetch.get(upstream(&state, &cells_path)?, LIVE, FetchOptions::default()).await?.json() },
        lightning_near(&state, q_lat, q_lon, q_radius, LIGHTNING_MINUTES),
    );
    let cells = cells.map_err(|_| ApiError::new(503, "Storm cell data temporarily unavailable.").retry(60))?;
    let strikes = lightning.map(|l| strikes(&l)).unwrap_or_default();
    let now = chrono::Utc::now().timestamp_millis();

    let mut out: Vec<Value> = cells["cells"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let (clat, clon) = (c["lat"].as_f64()?, c["lon"].as_f64()?);
            let area = c["area_km2"].as_f64().unwrap_or(0.0);
            let cell_radius = (area / std::f64::consts::PI).sqrt();
            let distance = haversine_km(ref_lat, ref_lon, clat, clon);
            let max_dbz = c["max_dbz"].as_f64().unwrap_or(f64::NAN);
            let moving = c["motion_speed_kmh"].as_f64().zip(c["motion_heading_deg"].as_f64()).filter(|(s, _)| *s >= 3.0);
            let nearby: Vec<_> = strikes.iter().filter(|s| haversine_km(clat, clon, s.0, s.1) <= cell_radius + 10.0).collect();
            let latest = nearby.iter().map(|s| s.2).max();
            Some(json!({
                "id": format!("{clat:.3},{clon:.3}"),
                "lat": clat,
                "lon": clon,
                "maxDbz": (max_dbz * 10.0).round() / 10.0,
                "areaKm2": (area * 10.0).round() / 10.0,
                "areaSqMi": (area / (KM_PER_MILE * KM_PER_MILE) * 10.0).round() / 10.0,
                "intensity": describe_intensity(max_dbz),
                "distanceKm": (distance * 10.0).round() / 10.0,
                "distanceMi": (distance / KM_PER_MILE * 10.0).round() / 10.0,
                "direction": compass(bearing_deg(ref_lat, ref_lon, clat, clon)),
                "motion": moving.map(|(speed, heading)| json!({
                    "speedKmh": speed.round(),
                    "speedMph": (speed / KM_PER_MILE).round(),
                    "headingDeg": heading.round(),
                    "toward": compass(heading),
                    "from": compass(heading + 180.0),
                })),
                "approach": closest_approach(c, ref_lat, ref_lon, cell_radius),
                "lightning": {
                    "strikes": nearby.len(),
                    "minutes": LIGHTNING_MINUTES,
                    "lastStrikeMinutesAgo": latest.map(|t| ((now - t) / 60_000).max(0)),
                },
                "tags": [],
            }))
        })
        .collect();
    out.sort_by(|a, b| a["distanceKm"].as_f64().unwrap_or(0.0).total_cmp(&b["distanceKm"].as_f64().unwrap_or(0.0)));
    let watch_next = out.iter().filter(|c| !c["approach"].is_null()).min_by_key(|c| c["approach"]["etaMinutes"].as_f64().unwrap_or(1e9) as i64).map(|c| c["id"].clone());
    let fastest = out.iter().filter(|c| !c["motion"].is_null()).max_by_key(|c| c["motion"]["speedKmh"].as_f64().unwrap_or(0.0) as i64)
        .filter(|c| c["motion"]["speedKmh"].as_f64().unwrap_or(0.0) >= 30.0).map(|c| c["id"].clone());
    for cell in &mut out {
        let mut tags = Vec::new();
        if watch_next.as_ref() == Some(&cell["id"]) {
            tags.push("watch-next");
        }
        if fastest.as_ref() == Some(&cell["id"]) {
            tags.push("fastest");
        }
        cell["tags"] = json!(tags);
    }
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=60")],
        Json(json!({ "generatedAt": cells["generated_at"], "reference": { "lat": ref_lat, "lon": ref_lon }, "lightningMinutes": LIGHTNING_MINUTES, "cells": out })),
    )
        .into_response())
}

/// GLM strikes around the station's home point as GeoJSON with `validtime` (ms), the shape the
/// app's lightning layer reads.
pub async fn lightning(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let Some((lat, lon)) = state.config.home() else {
        return Ok(Json(json!({ "type": "FeatureCollection", "features": [], "note": "Set the station's home point to receive lightning" })));
    };
    let data = lightning_near(&state, lat, lon, 600.0, 30).await?;
    let features: Vec<Value> = strikes(&data)
        .into_iter()
        .map(|(slat, slon, ts)| json!({ "type": "Feature", "geometry": { "type": "Point", "coordinates": [slon, slat] }, "properties": { "validtime": ts } }))
        .collect();
    Ok(Json(json!({ "type": "FeatureCollection", "features": features })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intensity_wording() {
        assert_eq!(describe_intensity(65.0)["label"], "Extreme");
        assert_eq!(describe_intensity(42.0)["level"], 2);
        assert!(describe_intensity(35.0)["hail"].is_null());
    }

    #[test]
    fn approach_math() {
        // Cell 20 km west of the point moving east at 40 km/h: arrives in ~30 minutes.
        let cell = json!({ "lat": 38.4, "lon": -86.93 - 20.0 / (111.32 * 38.4f64.to_radians().cos()), "motion_speed_kmh": 40.0, "motion_heading_deg": 90.0 });
        let a = closest_approach(&cell, 38.4, -86.93, 5.0);
        assert_eq!(a["etaMinutes"], 30.0);
        let away = json!({ "lat": 38.4, "lon": -87.2, "motion_speed_kmh": 40.0, "motion_heading_deg": 270.0 });
        assert!(closest_approach(&away, 38.4, -86.93, 5.0).is_null());
    }

    #[test]
    fn compass_names() {
        assert_eq!(compass(0.0), "North");
        assert_eq!(compass(225.0), "South West");
        assert_eq!(compass(359.0), "North");
    }
}
