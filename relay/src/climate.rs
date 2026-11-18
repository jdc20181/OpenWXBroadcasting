//! `/api/weatherbroadcaster/climate/normals`: NOAA NCEI 1991-2020 normals for the nearest station
//! with a full year of daily temperature normals, plus monthly precipitation and freeze dates.

use std::collections::HashMap;

use axum::{
    extract::{Query, State},
    http::header,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

use crate::{
    broadcaster::coordinate,
    fetch::{url, ApiError, ApiResult, FetchOptions, DAY},
    AppState,
};

const NORMALS_TTL: std::time::Duration = std::time::Duration::from_secs(30 * 86_400);

#[derive(Clone, Debug)]
struct Station {
    id: String,
    name: String,
    lat: f64,
    lon: f64,
    distance_km: f64,
}

impl Station {
    fn json(&self) -> Value {
        json!({ "id": self.id, "name": self.name, "lat": self.lat, "lon": self.lon, "distanceKm": (self.distance_km * 10.0).round() / 10.0 })
    }
}

pub fn distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = std::f64::consts::PI / 180.0;
    let a = ((lat2 - lat1) * r / 2.0).sin().powi(2) + (lat1 * r).cos() * (lat2 * r).cos() * ((lon2 - lon1) * r / 2.0).sin().powi(2);
    12_742.0 * a.sqrt().asin()
}

/// NCEI flags missing values as -7777/-8888/-9999.
fn normal_value(value: Option<&Value>) -> Option<f64> {
    let text = match value? {
        Value::String(s) => s.trim().to_owned(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    let n: f64 = text.parse().ok()?;
    (!text.is_empty() && n.is_finite() && n > -7000.0).then_some(n)
}

fn ncei(dataset: &str, station: &str, types: &str) -> url::Url {
    let mut u = url("https://www.ncei.noaa.gov/access/services/data/v1");
    u.query_pairs_mut().extend_pairs([
        ("dataset", dataset),
        ("stations", station),
        ("dataTypes", types),
        ("startDate", "0001-01-01"),
        ("endDate", "9996-12-31"),
        ("format", "json"),
    ]);
    u
}

async fn stations(state: &AppState, lat: f64, lon: f64) -> ApiResult<Vec<Station>> {
    for pad in [0.5, 1.5] {
        let mut u = url("https://www.ncei.noaa.gov/access/services/search/v1/data");
        let bbox = [lat + pad, lon - pad, lat - pad, lon + pad].map(|v| format!("{v:.2}")).join(",");
        u.query_pairs_mut().extend_pairs([("dataset", "normals-daily-1991-2020"), ("dataTypes", "DLY-TMAX-NORMAL"), ("limit", "100"), ("bbox", &bbox)]);
        let data = state.fetch.get(u, NORMALS_TTL, FetchOptions::default()).await?.json()?;
        let mut found: Vec<Station> = data["results"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| {
                let station = r["stations"].get(0)?;
                let id = station["id"].as_str()?.to_owned();
                let coords = r["location"]["coordinates"].as_array()?;
                let (slon, slat) = (coords.first()?.as_f64()?, coords.get(1)?.as_f64()?);
                Some(Station { name: station["name"].as_str().unwrap_or(&id).to_owned(), id, lat: slat, lon: slon, distance_km: distance_km(lat, lon, slat, slon) })
            })
            .collect();
        if !found.is_empty() {
            found.sort_by(|a, b| a.distance_km.total_cmp(&b.distance_km));
            found.truncate(5);
            return Ok(found);
        }
    }
    Err(ApiError::new(404, "No NOAA normals station within about 150 km"))
}

async fn rows(state: &AppState, dataset: &str, id: &str, types: &str) -> Vec<Value> {
    match state.fetch.get(ncei(dataset, id, types), NORMALS_TTL, FetchOptions::default()).await.and_then(|e| e.json()) {
        Ok(Value::Array(rows)) => rows,
        _ => Vec::new(),
    }
}

fn month_day(value: Option<&Value>) -> Option<String> {
    let s = value?.as_str()?.trim();
    (s.len() == 5 && s.as_bytes()[2] == b'/' && s.chars().filter(|c| c.is_ascii_digit()).count() == 4).then(|| s.to_owned())
}

pub async fn normals(State(state): State<AppState>, Query(q): Query<HashMap<String, String>>) -> ApiResult<Response> {
    let lat = coordinate(q.get("lat"), "latitude", 90.0)?;
    let lon = coordinate(q.get("lon"), "longitude", 180.0)?;
    let candidates = stations(&state, lat, lon).await?;
    for station in &candidates {
        let daily = rows(&state, "normals-daily-1991-2020", &station.id, "DLY-TMAX-NORMAL,DLY-TMIN-NORMAL").await;
        let days: Vec<Value> = daily
            .iter()
            .filter_map(|row| {
                let date = row["DATE"].as_str()?;
                let (hi, lo) = (normal_value(row.get("DLY-TMAX-NORMAL"))?, normal_value(row.get("DLY-TMIN-NORMAL"))?);
                (date.len() == 5 && date.as_bytes()[2] == b'-').then(|| json!({ "date": date, "hi": hi, "lo": lo }))
            })
            .collect();
        if days.len() < 365 {
            continue; // the next-nearest station
        }
        let annual = rows(&state, "normals-annualseasonal-1991-2020", &station.id, "ANN-TMIN-PRBLST-T32FP50,ANN-TMIN-PRBFST-T32FP50,ANN-TMIN-PRBGSL-T32FP50").await;
        // Many cooperative stations lack precipitation normals: the nearest with all 12 months.
        let mut monthly: Option<Vec<f64>> = None;
        let mut precip_station: Option<&Station> = None;
        let order = std::iter::once(station).chain(candidates.iter().filter(|s| s.id != station.id));
        for candidate in order {
            let mut months = [None; 12];
            for row in rows(&state, "normals-monthly-1991-2020", &candidate.id, "MLY-PRCP-NORMAL").await {
                if let Some(m) = row["DATE"].as_str().and_then(|d| d.trim().parse::<usize>().ok()).filter(|m| (1..=12).contains(m)) {
                    months[m - 1] = normal_value(row.get("MLY-PRCP-NORMAL"));
                }
            }
            if months.iter().all(Option::is_some) {
                monthly = Some(months.iter().map(|m| m.unwrap_or_default()).collect());
                precip_station = Some(candidate);
                break;
            }
        }
        let ann = annual.first().cloned().unwrap_or(Value::Null);
        let body = json!({
            "source": "NOAA NCEI 1991-2020 Climate Normals",
            "period": "1991-2020",
            "station": station.json(),
            "daily": days,
            "monthlyPrecip": monthly.map(|m| json!(m)).unwrap_or_else(|| json!([null, null, null, null, null, null, null, null, null, null, null, null])),
            "precipStation": precip_station.map(Station::json),
            "lastFreeze": month_day(ann.get("ANN-TMIN-PRBLST-T32FP50")),
            "firstFreeze": month_day(ann.get("ANN-TMIN-PRBFST-T32FP50")),
            "growingDays": normal_value(ann.get("ANN-TMIN-PRBGSL-T32FP50")),
        });
        let _ = DAY;
        return Ok(([(header::CACHE_CONTROL, "public, max-age=86400")], Json(body)).into_response());
    }
    Err(ApiError::new(404, "No nearby NOAA station has complete daily normals"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_normals_are_none() {
        assert_eq!(normal_value(Some(&json!("  45.2 "))), Some(45.2));
        assert_eq!(normal_value(Some(&json!("-7777"))), None);
        assert_eq!(normal_value(Some(&json!(""))), None);
        assert_eq!(month_day(Some(&json!(" 04/12 "))), Some("04/12".into()));
        assert_eq!(month_day(Some(&json!("-7777"))), None);
    }

    #[test]
    fn distance_is_haversine() {
        let d = distance_km(38.39, -86.93, 38.03, -87.53);
        assert!((60.0..70.0).contains(&d), "{d}");
    }
}
