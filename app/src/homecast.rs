//! Hourly NWS forecast at the home point, for graphics. Hour-by-hour, day-part, workweek and
//! outdoor-planner graphics used the Map Studio model grid, which only covers the area the map
//! is looking at (and only loads when a forecast map is open), so they came up empty. This feed
//! is independent of the map: the DCWeather relay's `nws/forecast?view=hourly` route, or
//! api.weather.gov directly when the relay doesn't have the route yet.

use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::DateTime;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hour {
    /// Unix start time.
    pub t: i64,
    /// °F.
    pub temp: f32,
    /// Chance of precipitation, %.
    pub pop: f32,
    /// Sustained wind (upper end of a range), mph.
    pub wind_mph: f32,
    pub short: String,
    pub is_day: bool,
}

pub async fn fetch(lat: f64, lon: f64) -> Result<Vec<Hour>> {
    let path = format!("nws/forecast?lat={lat:.4}&lon={lon:.4}&view=hourly");
    let json = match crate::net::broadcaster_route(&path, Duration::from_secs(20)).await {
        Ok(response) => response.json::<serde_json::Value>().await?,
        Err(e) => {
            log::debug!("home hourly: relay route unavailable ({e}); asking api.weather.gov");
            direct(lat, lon).await?
        }
    };
    let hours = parse(&json);
    if hours.is_empty() {
        return Err(anyhow!("NWS hourly forecast had no periods"));
    }
    Ok(hours)
}

async fn direct(lat: f64, lon: f64) -> Result<serde_json::Value> {
    let point = crate::net::get_json(&format!("https://api.weather.gov/points/{lat:.4},{lon:.4}")).await?;
    let url = point
        .pointer("/properties/forecastHourly")
        .and_then(|u| u.as_str())
        .ok_or_else(|| anyhow!("NWS has no hourly forecast for this point"))?
        .to_string();
    crate::net::get_json(&url).await
}

/// "10 mph", "5 to 15 mph" -> the larger number.
fn wind(text: &str) -> f32 {
    text.split(|c: char| !c.is_ascii_digit() && c != '.').filter_map(|w| w.parse::<f32>().ok()).fold(0.0, f32::max)
}

pub fn parse(json: &serde_json::Value) -> Vec<Hour> {
    let Some(periods) = json.pointer("/properties/periods").and_then(|p| p.as_array()) else {
        return Vec::new();
    };
    periods
        .iter()
        .filter_map(|p| {
            let t = DateTime::parse_from_rfc3339(p.get("startTime")?.as_str()?).ok()?.timestamp();
            let mut temp = p.get("temperature")?.as_f64()? as f32;
            if p.get("temperatureUnit").and_then(|u| u.as_str()) == Some("C") {
                temp = temp * 9.0 / 5.0 + 32.0;
            }
            Some(Hour {
                t,
                temp,
                pop: p.pointer("/probabilityOfPrecipitation/value").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                wind_mph: p.get("windSpeed").and_then(|w| w.as_str()).map(wind).unwrap_or(0.0),
                short: p.get("shortForecast").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                is_day: p.get("isDaytime").and_then(|d| d.as_bool()).unwrap_or(true),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nws_hourly_periods() {
        let json = serde_json::json!({"properties": {"periods": [
            {"startTime": "2026-11-11T14:00:00-05:00", "temperature": 61, "temperatureUnit": "F",
             "probabilityOfPrecipitation": {"value": 40}, "windSpeed": "10 to 15 mph",
             "shortForecast": "Chance Showers And Thunderstorms", "isDaytime": true}
        ]}});
        let h = parse(&json);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].temp, 61.0);
        assert_eq!(h[0].pop, 40.0);
        assert_eq!(h[0].wind_mph, 15.0);
        assert!(h[0].is_day);
    }
}
