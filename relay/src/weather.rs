//! `/api/weather/overview` (current conditions + NWS forecast for a point) and
//! `/api/weather/spc-outlooks` (SPC days 1-8, normalised for the map). Both read NOAA/NWS directly.

use std::collections::HashMap;

use axum::{
    extract::{Query, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    fetch::{url, ApiError, ApiResult, FetchOptions, DAY, LIVE},
    AppState,
};

/// NWS gridpoint for a point (the points metadata changes rarely: kept a day).
pub async fn grid_point(state: &AppState, lat: f64, lon: f64) -> ApiResult<(String, i64, i64)> {
    let meta = points(state, lat, lon).await?;
    let p = &meta["properties"];
    let grid = p["gridId"].as_str().unwrap_or_default().to_owned();
    let (x, y) = (p["gridX"].as_i64().unwrap_or(-1), p["gridY"].as_i64().unwrap_or(-1));
    if grid.len() != 3 || !grid.chars().all(|c| c.is_ascii_uppercase()) || x < 0 || y < 0 {
        return Err(ApiError::new(502, "NWS points response has no forecast grid"));
    }
    Ok((grid, x, y))
}

pub async fn points(state: &AppState, lat: f64, lon: f64) -> ApiResult<Value> {
    let target = url(&format!("https://api.weather.gov/points/{lat:.4},{lon:.4}"));
    state.fetch.get(target, DAY, FetchOptions::default()).await.map_err(|e| {
        if e.upstream == Some(404) { ApiError::new(404, "NWS forecast coverage is unavailable for this point") } else { e }
    })?.json()
}

fn value_of(p: &Value, key: &str) -> Option<(f64, String)> {
    let v = &p[key];
    Some((v["value"].as_f64()?, v["unitCode"].as_str().unwrap_or_default().to_owned()))
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn to_f(p: &Value, key: &str) -> Option<f64> {
    let (v, unit) = value_of(p, key)?;
    Some(round1(if unit.ends_with("degF") { v } else { v * 9.0 / 5.0 + 32.0 }))
}

fn to_mph(p: &Value, key: &str) -> Option<f64> {
    let (v, unit) = value_of(p, key)?;
    let mph = if unit.ends_with("km_h-1") { v / 1.609_344 } else if unit.ends_with("m_s-1") { v * 2.236_936 } else if unit.ends_with("kt") { v * 1.150_779 } else { v };
    Some(round1(mph))
}

pub fn cardinal(deg: f64) -> String {
    const NAMES: [&str; 16] = ["N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW", "NW", "NNW"];
    NAMES[(((deg % 360.0) + 360.0) % 360.0 / 22.5).round() as usize % 16].to_owned()
}

/// NWS latest observation -> the app's `Current` shape.
pub fn current_from_observation(station: &Value, observation: &Value) -> Value {
    let p = &observation["properties"];
    let id = station["properties"]["stationIdentifier"].as_str().map(str::to_owned);
    let name = station["properties"]["name"].as_str().map(str::to_owned);
    let direction = value_of(p, "windDirection").map(|v| v.0);
    let pressure = value_of(p, "barometricPressure").map(|(v, _)| (v / 3386.389 * 100.0).round() / 100.0);
    let visibility = value_of(p, "visibility").map(|(v, _)| round1(v / 1609.344));
    json!({
        "station": id,
        "stationLabel": name,
        "stationIdentifier": id,
        "timestamp": p["timestamp"],
        "textDescription": p["textDescription"].as_str().map(str::trim).filter(|s| !s.is_empty()),
        "temperatureF": to_f(p, "temperature"),
        "dewpointF": to_f(p, "dewpoint"),
        "windSpeedMph": to_mph(p, "windSpeed"),
        "windGustMph": to_mph(p, "windGust"),
        "windDirection": direction,
        "windDirectionCardinal": direction.map(cardinal),
        "relativeHumidity": value_of(p, "relativeHumidity").map(|(v, _)| v.round()),
        "barometricPressureInHg": pressure,
        "visibilityMiles": visibility,
        "heatIndexF": to_f(p, "heatIndex"),
        "windChillF": to_f(p, "windChill"),
    })
}

/// NWS forecast periods -> the app's `Period` list.
pub fn periods(forecast: &Value) -> Vec<Value> {
    forecast["properties"]["periods"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            json!({
                "name": p["name"],
                "startTime": p["startTime"],
                "endTime": p["endTime"],
                "isDaytime": p["isDaytime"].as_bool().unwrap_or(true),
                "temperature": p["temperature"],
                "temperatureUnit": p["temperatureUnit"],
                "shortForecast": p["shortForecast"].as_str().unwrap_or_default(),
                "detailedForecast": p["detailedForecast"].as_str().unwrap_or_default(),
                "windSpeed": p["windSpeed"],
                "windDirection": p["windDirection"],
                "precipitationProbability": p["probabilityOfPrecipitation"]["value"],
            })
        })
        .collect()
}

pub async fn overview(State(state): State<AppState>, Query(q): Query<HashMap<String, String>>) -> ApiResult<Json<Value>> {
    let (home_lat, home_lon) = state.config.home().unwrap_or((39.8283, -98.5795));
    let lat = match q.get("lat") { Some(_) => crate::broadcaster::coordinate(q.get("lat"), "latitude", 90.0)?, None => home_lat };
    let lon = match q.get("lon") { Some(_) => crate::broadcaster::coordinate(q.get("lon"), "longitude", 180.0)?, None => home_lon };
    let meta = points(&state, lat, lon).await?;
    let p = &meta["properties"];
    let place = &p["relativeLocation"]["properties"];

    let forecast = match p["forecast"].as_str().and_then(|u| url::Url::parse(u).ok()).filter(|u| u.host_str() == Some("api.weather.gov")) {
        Some(u) => state.fetch.get(u, LIVE, FetchOptions::default()).await.and_then(|e| e.json()).map(|f| periods(&f)).unwrap_or_default(),
        None => Vec::new(),
    };

    // Nearest observing stations: the first with a temperature in its latest report.
    let mut current = json!({});
    if let Some(list) = p["observationStations"].as_str().and_then(|u| url::Url::parse(u).ok()).filter(|u| u.host_str() == Some("api.weather.gov")) {
        if let Ok(stations) = state.fetch.get(list, DAY, FetchOptions::default()).await.and_then(|e| e.json()) {
            for station in stations["features"].as_array().into_iter().flatten().take(3) {
                let Some(id) = station["properties"]["stationIdentifier"].as_str() else { continue };
                let latest = url(&format!("https://api.weather.gov/stations/{id}/observations/latest"));
                if let Ok(obs) = state.fetch.get(latest, LIVE, FetchOptions::default()).await.and_then(|e| e.json()) {
                    let c = current_from_observation(station, &obs);
                    let has_temp = !c["temperatureF"].is_null();
                    if has_temp || current.as_object().is_some_and(|o| o.is_empty()) {
                        current = c;
                    }
                    if has_temp {
                        break;
                    }
                }
            }
        }
    }

    let label = state.config.coverage_label.trim();
    Ok(Json(json!({
        "location": {
            "lat": lat,
            "lon": lon,
            "coverageLabel": if label.is_empty() { Value::Null } else { json!(label) },
            "nearestCity": place["city"],
            "nearestState": place["state"],
        },
        "current": current,
        "forecast": forecast,
    })))
}

// ------------------------------------------------------------------------------------------
// SPC outlooks
// ------------------------------------------------------------------------------------------

const SPC_BASE: &str = "https://mapservices.weather.noaa.gov/vector/rest/services/outlooks/SPC_wx_outlks/MapServer";

/// (day, key, kind, hazard, layer id, label)
pub const SPC_LAYERS: &[(u8, &str, &str, &str, u32, &str)] = &[
    (1, "categorical", "categorical", "severe", 1, "Risk levels"),
    (1, "tornado", "probability", "tornado", 3, "Tornado chance"),
    (1, "hail", "probability", "hail", 5, "Large hail chance"),
    (1, "wind", "probability", "wind", 7, "Damaging wind chance"),
    (2, "categorical", "categorical", "severe", 9, "Risk levels"),
    (2, "tornado", "probability", "tornado", 11, "Tornado chance"),
    (2, "hail", "probability", "hail", 13, "Large hail chance"),
    (2, "wind", "probability", "wind", 15, "Damaging wind chance"),
    (3, "categorical", "categorical", "severe", 17, "Risk levels"),
    (3, "severe", "probability", "severe", 19, "Severe chance"),
    (4, "severe", "probability", "severe", 21, "Severe chance"),
    (5, "severe", "probability", "severe", 22, "Severe chance"),
    (6, "severe", "probability", "severe", 23, "Severe chance"),
    (7, "severe", "probability", "severe", 24, "Severe chance"),
    (8, "severe", "probability", "severe", 25, "Severe chance"),
];

/// (code, label, rank, colour, fill opacity)
const CATEGORICAL: &[(&str, &str, i64, &str, f64)] = &[
    ("TSTM", "Thunderstorms", 0, "#3b82f6", 0.12),
    ("MRGL", "Marginal", 1, "#22c55e", 0.16),
    ("SLGT", "Slight", 2, "#eab308", 0.16),
    ("ENH", "Enhanced", 3, "#f97316", 0.18),
    ("MDT", "Moderate", 4, "#ef4444", 0.2),
    ("HIGH", "High", 5, "#ec4899", 0.22),
];

fn prop<'a>(props: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let map = props.as_object()?;
    keys.iter().find_map(|k| map.get(*k).or_else(|| map.iter().find(|(pk, _)| pk.eq_ignore_ascii_case(k)).map(|(_, v)| v))).filter(|v| !v.is_null())
}

fn text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.trim().to_owned(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn probability_percent(props: &Value) -> Option<i64> {
    for key in [&["LABEL2", "label2"][..], &["LABEL", "label"], &["DN", "dn"], &["VALUE", "value"], &["PERCENT", "percent"]] {
        let t = text(prop(props, key));
        let digits: String = t.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() && digits.len() <= 3 {
            return digits.parse().ok();
        }
    }
    None
}

fn probability_style(percent: Option<i64>, hazard: &str) -> (&'static str, f64, i64) {
    let p = percent.unwrap_or(0);
    if hazard == "tornado" {
        return match p { 15.. => ("#ef4444", 0.2, 4), 10.. => ("#f97316", 0.18, 3), 5.. => ("#eab308", 0.16, 2), _ => ("#38bdf8", 0.14, 1) };
    }
    match p { 45.. => ("#dc2626", 0.2, 5), 30.. => ("#f97316", 0.18, 4), 15.. => ("#eab308", 0.16, 3), 5.. => ("#22c55e", 0.14, 2), _ => ("#38bdf8", 0.12, 1) }
}

/// One SPC feature in the shape the app reads (label, shortLabel, rank, style, geometry).
pub fn normalize_feature(feature: &Value, day: u8, key: &str, kind: &str, hazard: &str) -> Option<Value> {
    let geometry = feature.get("geometry").filter(|g| !g.is_null())?;
    let props = &feature["properties"];
    let (label, rank, colour, opacity, percent) = if kind == "categorical" {
        // SPC's numeric DN (2 TSTM, 3 MRGL, 4 SLGT, 5 ENH, 6 MDT, 8 HIGH), else a code in a label.
        let from_dn = match prop(props, &["DN", "dn"]).and_then(Value::as_i64) {
            Some(2) => Some("TSTM"), Some(3) => Some("MRGL"), Some(4) => Some("SLGT"), Some(5) => Some("ENH"), Some(6) => Some("MDT"), Some(8) => Some("HIGH"), _ => None,
        };
        let code: String = from_dn.map(str::to_owned).or_else(|| {
            [&["DN", "dn"][..], &["LABEL", "label"], &["LABEL2", "label2"]]
                .iter()
                .map(|k| text(prop(props, k)).to_ascii_uppercase().chars().filter(char::is_ascii_uppercase).collect::<String>())
                .find(|c| CATEGORICAL.iter().any(|row| row.0 == c))
        }).unwrap_or_default();
        match CATEGORICAL.iter().find(|c| c.0 == code) {
            Some(&(_, label, rank, colour, opacity)) => (label.to_owned(), rank, colour, opacity, None),
            None => {
                let label = text(prop(props, &["LABEL2", "label2"]));
                let label = if label.is_empty() { text(prop(props, &["LABEL", "label"])) } else { label };
                (if label.is_empty() { "Risk area".into() } else { label }, 0, "#38bdf8", 0.14, None)
            }
        }
    } else {
        let percent = probability_percent(props);
        let (colour, opacity, rank) = probability_style(percent, hazard);
        let label = match percent {
            Some(p) => format!("{p}%"),
            None => {
                let l = text(prop(props, &["LABEL2", "label2"]));
                if l.is_empty() { "Probability area".into() } else { l }
            }
        };
        (label, if percent.is_some() { rank } else { 0 }, colour, opacity, percent)
    };
    let stroke = text(prop(props, &["stroke"]));
    let fill = text(prop(props, &["fill"]));
    let fill_opacity = prop(props, &["fill-opacity", "fill_opacity"]).and_then(Value::as_f64).unwrap_or(opacity);
    Some(json!({
        "id": feature.get("id").cloned().unwrap_or_else(|| json!(format!("{day}-{key}-{label}"))),
        "geometry": geometry,
        "label": label,
        "shortLabel": label,
        "rank": rank,
        "day": day,
        "key": key,
        "kind": kind,
        "hazard": hazard,
        "percent": percent,
        "style": {
            "stroke": if stroke.is_empty() { colour.to_owned() } else { stroke },
            "fill": if fill.is_empty() { colour.to_owned() } else { fill },
            "fillOpacity": fill_opacity,
            "strokeOpacity": 0.95,
            "weight": 2,
        },
    }))
}

fn point_in_ring(x: f64, y: f64, ring: &[Value]) -> bool {
    let pts: Vec<(f64, f64)> = ring.iter().filter_map(|p| Some((p.get(0)?.as_f64()?, p.get(1)?.as_f64()?))).collect();
    let mut inside = false;
    let mut j = pts.len().wrapping_sub(1);
    for i in 0..pts.len() {
        let ((xi, yi), (xj, yj)) = (pts[i], pts[j]);
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn geometry_contains(geometry: &Value, lon: f64, lat: f64) -> bool {
    let polygon = |rings: &Value| rings.as_array().is_some_and(|r| r.iter().fold(false, |acc, ring| acc ^ ring.as_array().is_some_and(|ring| point_in_ring(lon, lat, ring))));
    match geometry["type"].as_str() {
        Some("Polygon") => polygon(&geometry["coordinates"]),
        Some("MultiPolygon") => geometry["coordinates"].as_array().is_some_and(|ps| ps.iter().any(polygon)),
        _ => false,
    }
}

pub async fn spc_outlooks(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let jobs = SPC_LAYERS.iter().map(|&(day, key, kind, hazard, layer, label)| {
        let state = state.clone();
        async move {
            let mut target = url(&format!("{SPC_BASE}/{layer}/query"));
            target.query_pairs_mut().extend_pairs([("f", "geojson"), ("outFields", "*"), ("outSR", "4326"), ("returnGeometry", "true"), ("where", "1=1")]);
            let payload = state.fetch.get(target, LIVE, FetchOptions { max_bytes: crate::fetch::MAX_LARGE_RESPONSE, ..Default::default() }).await.and_then(|e| e.json());
            let features: Vec<Value> = match &payload {
                Ok(p) => p["features"].as_array().into_iter().flatten().filter_map(|f| normalize_feature(f, day, key, kind, hazard)).collect(),
                Err(_) => Vec::new(),
            };
            (day, json!({ "day": day, "key": key, "kind": kind, "hazard": hazard, "layerId": layer, "label": label, "features": features }), payload.is_ok())
        }
    });
    let layers = futures_util::future::join_all(jobs).await;
    if layers.iter().all(|l| !l.2) {
        return Err(ApiError::new(503, "SPC outlooks data is unavailable right now.").retry(60));
    }
    let home = state.config.home();
    let days: Vec<Value> = (1..=8u8)
        .map(|day| {
            let set: Vec<&Value> = layers.iter().filter(|l| l.0 == day).map(|l| &l.1).collect();
            let categorical = set.iter().find(|l| l["kind"] == "categorical").cloned().cloned();
            let probabilities: Vec<Value> = set.iter().filter(|l| l["kind"] == "probability").map(|l| (*l).clone()).collect();
            let top = |layer: &Value| -> Option<Value> {
                let (lat, lon) = home?;
                layer["features"].as_array()?.iter().filter(|f| geometry_contains(&f["geometry"], lon, lat)).max_by_key(|f| f["rank"].as_i64().unwrap_or(0)).cloned()
            };
            let cat_here = categorical.as_ref().and_then(top);
            let headline = match &cat_here {
                Some(f) => format!("{} risk at the coverage point", f["shortLabel"].as_str().unwrap_or("Some")),
                None if home.is_none() => "Set the station's home point to see the local risk".into(),
                None => "No SPC area over the coverage point right now".into(),
            };
            json!({
                "day": day,
                "label": format!("Day {day}"),
                "categorical": categorical,
                "probabilities": probabilities,
                "summary": { "headline": headline, "hasCoverage": cat_here.is_some() },
            })
        })
        .collect();
    Ok(Json(json!({
        "coverage": { "label": state.config.coverage_label, "lat": home.map(|h| h.0), "lon": home.map(|h| h.1) },
        "fetchedAt": chrono::Utc::now().to_rfc3339(),
        "days": days,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_converts_units() {
        let station = json!({ "properties": { "stationIdentifier": "KHNB", "name": "Huntingburg Airport" } });
        let obs = json!({ "properties": {
            "timestamp": "2026-11-17T15:00:00+00:00", "textDescription": "Cloudy",
            "temperature": { "value": 10.0, "unitCode": "wmoUnit:degC" },
            "windSpeed": { "value": 16.09344, "unitCode": "wmoUnit:km_h-1" },
            "windDirection": { "value": 225.0, "unitCode": "wmoUnit:degree_(angle)" },
            "barometricPressure": { "value": 101_591.67, "unitCode": "wmoUnit:Pa" },
            "visibility": { "value": 16_093.44, "unitCode": "wmoUnit:m" },
            "heatIndex": { "value": null, "unitCode": "wmoUnit:degC" }
        }});
        let c = current_from_observation(&station, &obs);
        assert_eq!(c["temperatureF"], 50.0);
        assert_eq!(c["windSpeedMph"], 10.0);
        assert_eq!(c["windDirectionCardinal"], "SW");
        assert_eq!(c["barometricPressureInHg"], 30.0);
        assert_eq!(c["visibilityMiles"], 10.0);
        assert!(c["heatIndexF"].is_null());
        assert_eq!(c["stationIdentifier"], "KHNB");
    }

    #[test]
    fn spc_categorical_and_probability() {
        let cat = json!({ "properties": { "dn": 4, "label": "SLGT", "label2": "Slight Risk" }, "geometry": { "type": "Polygon", "coordinates": [[[-90.0, 35.0], [-80.0, 35.0], [-80.0, 42.0], [-90.0, 42.0], [-90.0, 35.0]]] } });
        let f = normalize_feature(&cat, 1, "categorical", "categorical", "severe").unwrap();
        assert_eq!(f["label"], "Slight");
        assert_eq!(f["rank"], 2);
        assert!(geometry_contains(&f["geometry"], -86.9, 38.4));
        assert!(!geometry_contains(&f["geometry"], -95.0, 38.4));
        let prob = json!({ "properties": { "label": "0.10", "label2": "10% Tornado Risk" }, "geometry": cat["geometry"] });
        let p = normalize_feature(&prob, 1, "tornado", "probability", "tornado").unwrap();
        assert_eq!(p["label"], "10%");
        assert_eq!(p["rank"], 3);
    }
}
