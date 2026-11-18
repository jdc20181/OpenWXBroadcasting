//! Weather reads for the CLI and the MCP tools. They run OpenWX Relay in-process (no server or app
//! needed) with the station's contact and home point from its settings.

use anyhow::{anyhow, Result};
use axum::{body::Body, http::Request};
use serde_json::{json, Value};

use openwx_relay::{AppState, RelayConfig};

pub fn relay(settings: &Value) -> Result<AppState> {
    let home = settings["setup_complete"].as_bool().unwrap_or(false).then(|| (settings["home"]["lat"].as_f64(), settings["home"]["lon"].as_f64()));
    let (home_lat, home_lon) = home.unwrap_or((None, None));
    let config = RelayConfig {
        contact: settings["contact"].as_str().unwrap_or_default().to_owned(),
        station: settings["station_name"].as_str().unwrap_or_default().to_owned(),
        home_lat,
        home_lon,
        coverage_label: settings["coverage"]["name"].as_str().or(settings["home_label"].as_str()).unwrap_or_default().to_owned(),
        ..Default::default()
    };
    AppState::new(config)
}

/// GET a relay path in-process and parse the JSON answer.
pub async fn get(relay: &AppState, path_and_query: &str) -> Result<Value> {
    let request = Request::get(path_and_query).body(Body::empty())?;
    let response = openwx_relay::oneshot(relay.clone(), request).await;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 64 << 20).await?;
    let value: Value = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(&bytes[..bytes.len().min(400)]) }));
    if !status.is_success() {
        return Err(anyhow!("{}: {}", status.as_u16(), value["error"].as_str().unwrap_or("request failed")));
    }
    Ok(value)
}

fn point(settings: &Value, lat: Option<f64>, lon: Option<f64>) -> Result<(f64, f64)> {
    match (lat, lon) {
        (Some(a), Some(b)) => Ok((a, b)),
        _ if settings["setup_complete"].as_bool() == Some(true) => Ok((settings["home"]["lat"].as_f64().unwrap_or(0.0), settings["home"]["lon"].as_f64().unwrap_or(0.0))),
        _ => Err(anyhow!("no point given and the station has no home yet (run `openwx setup`)")),
    }
}

/// Current conditions and the 7-day forecast, trimmed for reading.
pub async fn overview(relay: &AppState, settings: &Value, lat: Option<f64>, lon: Option<f64>) -> Result<Value> {
    let (lat, lon) = point(settings, lat, lon)?;
    let v = get(relay, &format!("/api/weather/overview?lat={lat:.4}&lon={lon:.4}")).await?;
    let periods: Vec<Value> = v["forecast"]
        .as_array()
        .into_iter()
        .flatten()
        .take(14)
        .map(|p| json!({ "name": p["name"], "temperature": p["temperature"], "short": p["shortForecast"], "detail": p["detailedForecast"], "precipChance": p["precipitationProbability"], "wind": format!("{} {}", p["windDirection"].as_str().unwrap_or(""), p["windSpeed"].as_str().unwrap_or("")).trim().to_owned() }))
        .collect();
    Ok(json!({ "location": v["location"], "current": v["current"], "forecast": periods, "source": "NOAA/National Weather Service" }))
}

/// Active NWS alerts for a state ("IN") or the station's states.
pub async fn alerts(relay: &AppState, settings: &Value, area: Option<&str>) -> Result<Value> {
    let area = match area {
        Some(a) if !a.trim().is_empty() => a.trim().to_ascii_uppercase(),
        _ => settings["alert_area"].as_str().filter(|a| !a.is_empty()).map(str::to_owned).ok_or_else(|| anyhow!("give an area (state code like IN) or run `openwx setup`"))?,
    };
    let resource = format!("/alerts/active?area={}", area.split(',').map(str::trim).collect::<Vec<_>>().join(","));
    let mut path = url::Url::parse("http://relay/api/weatherbroadcaster/feed")?;
    path.query_pairs_mut().append_pair("provider", "nws").append_pair("resource", &resource);
    let v = get(relay, &format!("{}?{}", path.path(), path.query().unwrap_or(""))).await?;
    let counties: Vec<String> = settings["coverage"]["counties"].as_array().into_iter().flatten().filter_map(|c| c["name"].as_str().map(|n| n.replace(" County", "").replace(" Parish", ""))).collect();
    let alerts: Vec<Value> = v["features"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| {
            let p = &f["properties"];
            let areas = p["areaDesc"].as_str().unwrap_or_default();
            let in_coverage = counties.iter().any(|c| areas.split(';').any(|a| a.trim().eq_ignore_ascii_case(c)));
            json!({ "event": p["event"], "headline": p["headline"], "severity": p["severity"], "areas": areas, "inCoverage": in_coverage, "onset": p["onset"], "expires": p["expires"], "instruction": p["instruction"], "sender": p["senderName"] })
        })
        .collect();
    Ok(json!({ "area": area, "count": alerts.len(), "alerts": alerts, "source": "NOAA/National Weather Service" }))
}

/// SPC outlook summary for days 1-8 (or one day).
pub async fn spc(relay: &AppState, day: Option<u64>) -> Result<Value> {
    let v = get(relay, "/api/weather/spc-outlooks").await?;
    let days: Vec<Value> = v["days"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| day.is_none_or(|n| d["day"].as_u64() == Some(n)))
        .map(|d| {
            let risks: Vec<Value> = d["categorical"]["features"].as_array().into_iter().flatten().map(|f| f["label"].clone()).collect();
            let probs: Vec<Value> = d["probabilities"].as_array().into_iter().flatten().filter(|p| p["features"].as_array().is_some_and(|f| !f.is_empty()))
                .map(|p| json!({ "hazard": p["label"], "levels": p["features"].as_array().map(|f| f.iter().map(|x| x["label"].clone()).collect::<Vec<_>>()) }))
                .collect();
            json!({ "day": d["day"], "riskLevelsShown": risks, "probabilities": probs, "atHome": d["summary"]["headline"] })
        })
        .collect();
    Ok(json!({ "days": days, "source": "NOAA Storm Prediction Center" }))
}

pub async fn check(relay: &AppState) -> Result<Value> {
    get(relay, "/api/openwx/check").await
}
