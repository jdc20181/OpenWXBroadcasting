//! Strict NWS `forecastHourly` reader for point-series graphics. In-memory only.
use crate::config::GeoView;
use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Debug)]
pub(super) struct Hour {
    pub start: DateTime<FixedOffset>,
    pub end: DateTime<FixedOffset>,
    pub temperature_f: Option<f64>,
    pub dewpoint_f: Option<f64>,
    pub wind_mph: Option<f64>,
}
#[derive(Clone, Debug)]
pub(super) struct Snapshot {
    pub home: [u64; 2],
    pub generation: u64,
    pub checked: i64,
    pub issued: i64,
    pub place: String,
    pub timezone: String,
    pub hours: Vec<Hour>,
}
#[derive(Default)]
struct Store {
    generation: u64,
    current: Option<Arc<Snapshot>>,
}
fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Store::default()))
}
fn home_key(h: GeoView) -> [u64; 2] {
    [h.lon.to_bits(), h.lat.to_bits()]
}
fn clean(s: &str, max: usize) -> Option<String> {
    let s = s.trim();
    (!s.is_empty() && s.len() <= max && !s.chars().any(char::is_control)).then(|| s.to_owned())
}
fn rfc3339(v: &Value) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(v.as_str()?).ok()
}
fn valid_grid_url(v: &Value) -> Option<String> {
    let raw = v.as_str()?;
    let rest = raw.strip_prefix("https://api.weather.gov/gridpoints/")?;
    let parts: Vec<_> = rest.split('/').collect();
    if parts.len() != 4
        || parts[0].len() != 3
        || !parts[0].bytes().all(|b| b.is_ascii_uppercase())
        || parts[2] != "forecast"
        || parts[3] != "hourly"
    {
        return None;
    }
    let (x, y) = parts[1].split_once(',')?;
    for n in [x, y] {
        if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) || n.parse::<u16>().ok()? > 2000 {
            return None;
        }
    }
    // Rebuild only the exact endpoint path; unexpected query/fragment suffixes fail closed.
    Some(format!(
        "https://api.weather.gov/gridpoints/{}/{}/forecast/hourly",
        parts[0], parts[1]
    ))
}
fn temperature_period(p: &Value) -> Option<f64> {
    let t = p.get("temperature")?;
    if t.is_null() {
        return None;
    }
    let (n, unit) = if t.is_object() {
        (t.get("value")?.as_f64()?, t.get("unitCode")?.as_str()?)
    } else {
        (t.as_f64()?, p.get("temperatureUnit")?.as_str()?)
    };
    let f = match unit {
        "F" | "wmoUnit:degF" | "degF" => n,
        "C" | "wmoUnit:degC" | "degC" => n * 1.8 + 32.,
        _ => return None,
    };
    (f.is_finite() && (-150.0..=150.0).contains(&f)).then_some(f)
}
fn wind_mph(v: &Value) -> Option<f64> {
    // NWS publishes ranges as strings such as "5 to 10 mph". Do not replace them
    // with an invented midpoint or gust value; this chart accepts only one exact mph value.
    let raw = v.as_str()?.trim();
    let number = raw.strip_suffix(" mph")?;
    if number.is_empty()
        || !number.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        || number.matches('.').count() > 1
    {
        return None;
    }
    let value = number.parse::<f64>().ok()?;
    (value.is_finite() && (0.0..=150.0).contains(&value)).then_some(value)
}
fn dewpoint(v: &Value) -> Option<f64> {
    if v.is_null() {
        return None;
    }
    let number = v.get("value")?.as_f64()?;
    let unit = v.get("unitCode")?.as_str()?;
    let f = match unit {
        "wmoUnit:degF" | "degF" => number,
        "wmoUnit:degC" | "degC" => number * 1.8 + 32.,
        _ => return None,
    };
    (f.is_finite() && (-150.0..=150.0).contains(&f)).then_some(f)
}
fn decode(
    v: &Value,
    home: GeoView,
    timezone: String,
    now: i64,
    generation: u64,
) -> Option<Snapshot> {
    let properties = v.get("properties")?;
    let issued = rfc3339(properties.get("updateTime")?)?.timestamp();
    if !(0..=12 * 3600).contains(&(now - issued)) {
        return None;
    }
    let raw = properties.get("periods")?.as_array()?;
    if raw.is_empty() || raw.len() > 192 {
        return None;
    }
    let mut hours = Vec::with_capacity(raw.len());
    for period in raw {
        let start = rfc3339(period.get("startTime")?)?;
        let end = rfc3339(period.get("endTime")?)?;
        let duration = end.timestamp() - start.timestamp();
        if duration != 3600
            || start.timestamp() < now - 3600
            || end.timestamp() <= now
            || start.timestamp() > now + 8 * 86400
            || end.timestamp() > now + 8 * 86400 + 3600
        {
            return None;
        }
        hours.push(Hour {
            start,
            end,
            temperature_f: period
                .get("temperature")
                .and_then(|_| temperature_period(period)),
            dewpoint_f: period.get("dewpoint").and_then(dewpoint),
            wind_mph: period.get("windSpeed").and_then(wind_mph),
        });
    }
    if hours
        .windows(2)
        .any(|w| w[1].start.timestamp() < w[0].end.timestamp())
    {
        return None;
    }
    let near = properties
        .get("gridId")
        .and_then(Value::as_str)
        .unwrap_or("");
    let place = if near.is_empty() {
        format!("Home point {:.3}, {:.3}", home.lat, home.lon)
    } else {
        format!(
            "Home point {:.3}, {:.3} • NWS grid {}",
            home.lat, home.lon, near
        )
    };
    Some(Snapshot {
        home: home_key(home),
        generation,
        checked: now,
        issued,
        place,
        timezone,
        hours,
    })
}
async fn fetch(home: GeoView, generation: u64) -> Option<Snapshot> {
    if !home.lat.is_finite()
        || !home.lon.is_finite()
        || !(-90.0..=90.0).contains(&home.lat)
        || !(-180.0..=180.0).contains(&home.lon)
    {
        return None;
    }
    let point_url = format!(
        "https://api.weather.gov/points/{:.4},{:.4}",
        home.lat, home.lon
    );
    let point = super::report_feed::get_json(&point_url).await?;
    if point.get("geometry")?.get("type")?.as_str()? != "Point" {
        return None;
    }
    let coordinates = point.get("geometry")?.get("coordinates")?.as_array()?;
    if coordinates.len() != 2
        || (coordinates[0].as_f64()? - home.lon).abs() > 0.02
        || (coordinates[1].as_f64()? - home.lat).abs() > 0.02
    {
        return None;
    }
    let properties = point.get("properties")?;
    let timezone = clean(properties.get("timeZone")?.as_str()?, 64)?;
    let raw_url = properties.get("forecastHourly")?;
    let forecast_url = valid_grid_url(raw_url)?;
    let forecast = super::report_feed::get_json(&forecast_url).await?;
    decode(
        &forecast,
        home,
        timezone,
        chrono::Utc::now().timestamp(),
        generation,
    )
}

/// Refresh replaces the prior snapshot at the start; failed latest fetches remain unavailable.
pub(super) async fn refresh(home: GeoView) {
    let generation = match store().lock() {
        Ok(mut state) => {
            state.generation = state.generation.wrapping_add(1);
            state.current = None;
            state.generation
        }
        Err(_) => return,
    };
    let result = tokio::time::timeout(std::time::Duration::from_secs(60), fetch(home, generation))
        .await
        .ok()
        .flatten()
        .map(Arc::new);
    if let Ok(mut state) = store().lock() {
        if state.generation == generation {
            state.current = result;
        }
    }
}
pub(super) fn current(home: GeoView) -> Option<Arc<Snapshot>> {
    let now = chrono::Utc::now().timestamp();
    let state = store().lock().ok()?;
    let snapshot = state.current.as_ref()?;
    if snapshot.home != home_key(home)
        || snapshot.generation != state.generation
        || !(0..=1200).contains(&(now - snapshot.checked))
        || !(0..=12 * 3600).contains(&(now - snapshot.issued))
    {
        return None;
    }
    Some(Arc::clone(snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hour(start: &str, end: &str, temp: Value, dew: Value, wind: &str) -> Value {
        json!({
            "startTime":start,"endTime":end,"temperature":temp,"temperatureUnit":"F",
            "dewpoint":dew,"windSpeed":wind
        })
    }
    fn response(update: &str, periods: Vec<Value>) -> Value {
        json!({"properties":{"updateTime":update,"gridId":"LMK","periods":periods}})
    }
    #[test]
    fn hourly_parser_keeps_dst_offsets_and_converts_source_units() {
        let now = DateTime::parse_from_rfc3339("2026-11-01T04:30:00Z")
            .unwrap()
            .timestamp();
        let v = response(
            "2026-11-01T04:00:00Z",
            vec![
                hour(
                    "2026-11-01T01:00:00-04:00",
                    "2026-11-01T01:00:00-05:00",
                    json!(20),
                    json!({"value":10.0,"unitCode":"wmoUnit:degC"}),
                    "5 mph",
                ),
                hour(
                    "2026-11-01T01:00:00-05:00",
                    "2026-11-01T02:00:00-05:00",
                    json!(21),
                    json!({"value":50.0,"unitCode":"wmoUnit:degF"}),
                    "7 mph",
                ),
            ],
        );
        let h = GeoView {
            lat: 38.3,
            lon: -86.9,
            zoom: 7.,
        };
        let snapshot = decode(&v, h, "America/Indiana/Indianapolis".into(), now, 4).unwrap();
        assert_eq!(snapshot.hours.len(), 2);
        assert_eq!(
            snapshot.hours[0].start.offset().local_minus_utc(),
            -4 * 3600
        );
        assert_eq!(
            snapshot.hours[1].start.offset().local_minus_utc(),
            -5 * 3600
        );
        assert!((snapshot.hours[0].temperature_f.unwrap() - 20.).abs() < 0.01);
        assert!((snapshot.hours[0].dewpoint_f.unwrap() - 50.).abs() < 0.01);
        assert_eq!(snapshot.hours[0].wind_mph, Some(5.));
    }
    #[test]
    fn malformed_stale_or_future_intervals_are_withheld() {
        let now = DateTime::parse_from_rfc3339("2026-11-14T18:30:00Z")
            .unwrap()
            .timestamp();
        let h = GeoView {
            lat: 38.3,
            lon: -86.9,
            zoom: 7.,
        };
        let good = hour(
            "2026-11-14T13:00:00-05:00",
            "2026-11-14T14:00:00-05:00",
            json!(60),
            json!({"value":50.0,"unitCode":"wmoUnit:degF"}),
            "5 mph",
        );
        assert!(
            decode(
                &response("2026-11-14T18:00:00Z", vec![good.clone()]),
                h,
                "America/Indiana/Indianapolis".into(),
                now,
                1
            )
            .is_some()
        );
        assert!(
            decode(
                &response("2026-11-14T06:00:00Z", vec![good.clone()]),
                h,
                "America/Indiana/Indianapolis".into(),
                now,
                1
            )
            .is_none()
        );
        assert!(
            decode(
                &response("2026-11-14T19:00:00Z", vec![good.clone()]),
                h,
                "America/Indiana/Indianapolis".into(),
                now,
                1
            )
            .is_none()
        );
        let bad = hour(
            "2026-11-14T13:00:00-05:00",
            "2026-11-14T15:00:00-05:00",
            json!(60),
            json!({"value":50.0,"unitCode":"wmoUnit:degF"}),
            "5 mph",
        );
        assert!(
            decode(
                &response("2026-11-14T18:00:00Z", vec![bad]),
                h,
                "America/Indiana/Indianapolis".into(),
                now,
                1
            )
            .is_none()
        );
    }
    #[test]
    fn unsupported_wind_ranges_and_untrusted_grid_urls_are_not_reinterpreted() {
        assert_eq!(wind_mph(&json!("7 mph")), Some(7.));
        assert_eq!(wind_mph(&json!("5 to 10 mph")), None);
        assert_eq!(wind_mph(&json!("10 kt")), None);
        assert!(
            valid_grid_url(&json!(
                "https://api.weather.gov/gridpoints/LMK/77,85/forecast/hourly"
            ))
            .is_some()
        );
        assert!(
            valid_grid_url(&json!(
                "https://api.weather.gov/gridpoints/LMK/77,85/forecast/hourly?x=1"
            ))
            .is_none()
        );
        assert!(
            valid_grid_url(&json!(
                "https://evil.example/gridpoints/LMK/77,85/forecast/hourly"
            ))
            .is_none()
        );
        assert!(
            valid_grid_url(&json!(
                "https://api.weather.gov/gridpoints/LMK/77,85/forecast"
            ))
            .is_none()
        );
    }
}
