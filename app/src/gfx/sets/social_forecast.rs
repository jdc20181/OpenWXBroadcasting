//! Purpose-built NWS point forecast cards. Source discovery and live data stay in sets.
use super::reference::{frame, label, social_frame, MISSING};
use crate::{
    config::{GeoView, Settings},
    gfx::{
        bind::Bindings,
        model::{AnimKind, GraphicDoc},
        text::Align,
    },
};
use chrono::{DateTime, FixedOffset};
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    day: u8,
    format: u8,
    county: bool,
}
const SPECS: &[Spec] = &[
    Spec {
        key: "set_social_forecast__today_point",
        day: 0,
        county: false,
        format: 0,
    },
    Spec {
        key: "set_social_forecast__tomorrow_point",
        day: 1,
        county: false,
        format: 0,
    },
    Spec {
        key: "set_social_forecast__day3_point",
        day: 2,
        county: false,
        format: 0,
    },
    Spec {
        key: "set_social_forecast__today_point_portrait",
        day: 0,
        county: false,
        format: 1,
    },
    Spec {
        key: "set_social_forecast__tomorrow_point_portrait",
        day: 1,
        county: false,
        format: 1,
    },
    Spec {
        key: "set_social_forecast__day3_point_portrait",
        day: 2,
        county: false,
        format: 1,
    },
    Spec {
        key: "set_social_forecast__today_point_square",
        day: 0,
        county: false,
        format: 2,
    },
    Spec {
        key: "set_social_forecast__tomorrow_point_square",
        day: 1,
        county: false,
        format: 2,
    },
    Spec {
        key: "set_social_forecast__day3_point_square",
        day: 2,
        county: false,
        format: 2,
    },
    Spec {
        key: "set_social_forecast__today_county",
        day: 0,
        format: 0,
        county: true,
    },
    Spec {
        key: "set_social_forecast__tomorrow_county",
        day: 1,
        format: 0,
        county: true,
    },
    Spec {
        key: "set_social_forecast__day3_county",
        day: 2,
        format: 0,
        county: true,
    },
    Spec {
        key: "set_social_forecast__today_county_portrait",
        day: 0,
        format: 1,
        county: true,
    },
    Spec {
        key: "set_social_forecast__tomorrow_county_portrait",
        day: 1,
        format: 1,
        county: true,
    },
    Spec {
        key: "set_social_forecast__day3_county_portrait",
        day: 2,
        format: 1,
        county: true,
    },
    Spec {
        key: "set_social_forecast__today_county_square",
        day: 0,
        format: 2,
        county: true,
    },
    Spec {
        key: "set_social_forecast__tomorrow_county_square",
        day: 1,
        format: 2,
        county: true,
    },
    Spec {
        key: "set_social_forecast__day3_county_square",
        day: 2,
        format: 2,
        county: true,
    },
];
#[derive(Clone, Debug)]
struct Period {
    start: DateTime<FixedOffset>,
    end: i64,
    day: bool,
    temp: Option<f64>,
    short: String,
    wind: String,
    pop: Option<f64>,
}
struct Snapshot {
    home: [u64; 2],
    checked: i64,
    issued: i64,
    place: String,
    periods: Vec<Period>,
}

/// Opaque validated NWS snapshot for independent town-point graphics.
pub(super) struct CitySnapshot(Snapshot);

#[derive(Clone, Debug)]
pub(super) struct CityDay {
    pub high: String,
    pub low: String,
    pub condition: String,
    pub issue: String,
    pub date: String,
    pub remaining_night: bool,
}
#[derive(Default)]
struct Store {
    generation: u64,
    current: Option<Arc<Snapshot>>,
    county: Option<Arc<CountySnapshot>>,
}
struct CountySnapshot {
    origin: [u64; 2],
    place: String,
    forecast: Snapshot,
}
fn county_point(rings: &super::arcgis_geometry::Rings, home: (f64, f64)) -> Option<(f64, f64)> {
    if super::arcgis_geometry::contains(rings, home.0, home.1) != Some(true) {
        return None;
    }
    let n = rings.iter().map(Vec::len).sum::<usize>();
    if n == 0 {
        return None;
    }
    let total = rings
        .iter()
        .flatten()
        .fold((0., 0.), |v, p| (v.0 + p.0, v.1 + p.1));
    let centre = (total.0 / n as f64, total.1 / n as f64);
    (super::arcgis_geometry::contains(rings, centre.0, centre.1) == Some(true)).then_some(centre)
}
fn county_location(home: GeoView) -> Option<(GeoView, String)> {
    let all = crate::map::boundaries::counties_named()?;
    let mut found = None;
    for county in all.iter() {
        if super::arcgis_geometry::contains(&county.rings, home.lon, home.lat) == Some(true) {
            if found.is_some() {
                return None;
            }
            let (lon, lat) = county_point(&county.rings, (home.lon, home.lat))?;
            let state = crate::winter::STATES
                .iter()
                .find(|s| s.0 == county.state)
                .map(|s| s.1)?;
            found = Some((
                GeoView {
                    lon,
                    lat,
                    zoom: home.zoom,
                },
                format!("{}, {state} • County representative point", county.name),
            ));
        }
    }
    found
}
async fn fetch_county(home: GeoView) -> Option<CountySnapshot> {
    let (point, place) = county_location(home)?;
    Some(CountySnapshot {
        origin: home_key(home),
        place,
        forecast: fetch(point).await?,
    })
}
fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Store::default()))
}
fn home_key(h: GeoView) -> [u64; 2] {
    [h.lon.to_bits(), h.lat.to_bits()]
}
fn clean(v: &Value, max: usize) -> Option<String> {
    let s = v.as_str()?.trim();
    (!s.is_empty() && s.len() <= max && !s.chars().any(char::is_control)).then(|| s.to_owned())
}
fn time(v: &Value) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(v.as_str()?).ok()
}
fn forecast_url(v: &Value) -> Option<String> {
    let tail = v
        .as_str()?
        .strip_prefix("https://api.weather.gov/gridpoints/")?;
    let p: Vec<_> = tail.split('/').collect();
    if p.len() != 3
        || p[0].len() != 3
        || !p[0].bytes().all(|b| b.is_ascii_uppercase())
        || p[2] != "forecast"
    {
        return None;
    }
    let (x, y) = p[1].split_once(',')?;
    if ![x, y].iter().all(|s| {
        !s.is_empty()
            && s.bytes().all(|b| b.is_ascii_digit())
            && s.parse::<u16>().is_ok_and(|n| n <= 2000)
    }) {
        return None;
    }
    Some(format!(
        "https://api.weather.gov/gridpoints/{}/{}/forecast",
        p[0], p[1]
    ))
}
fn temp(p: &Value) -> Option<f64> {
    let t = p.get("temperature")?;
    let (v, u) = if t.is_object() {
        (t.get("value")?.as_f64()?, t.get("unitCode")?.as_str()?)
    } else {
        (t.as_f64()?, p.get("temperatureUnit")?.as_str()?)
    };
    let f = match u {
        "F" | "wmoUnit:degF" => v,
        "C" | "wmoUnit:degC" => v * 1.8 + 32.,
        _ => return None,
    };
    (f.is_finite() && (-150.0..=150.0).contains(&f)).then_some(f)
}
fn wind(p: &Value) -> String {
    let Some(w) = p.get("windSpeed") else {
        return String::new();
    };
    let speed = if let Some(s) = clean(w, 60) {
        s
    } else if let (Some(x), Some(u)) = (
        w.get("value").and_then(Value::as_f64),
        w.get("unitCode").and_then(Value::as_str),
    ) {
        if !x.is_finite() || !(0.0..=500.0).contains(&x) {
            return String::new();
        }
        let unit = match u {
            "wmoUnit:km_h-1" => "km/h",
            "wmoUnit:m_s-1" => "m/s",
            "wmoUnit:mi_h-1" => "mph",
            _ => return String::new(),
        };
        format!("{x:.0} {unit}")
    } else {
        return String::new();
    };
    format!(
        "{} {speed}",
        p.get("windDirection")
            .and_then(|v| clean(v, 12))
            .unwrap_or_default()
    )
    .trim()
    .to_owned()
}
fn decode(v: &Value, h: GeoView, place: String, now: i64) -> Option<Snapshot> {
    let p = v.get("properties")?;
    let issued = time(p.get("updateTime")?)?.timestamp();
    if !(0..=86400).contains(&(now - issued)) {
        return None;
    }
    let a = p.get("periods")?.as_array()?;
    if a.is_empty() || a.len() > 40 {
        return None;
    }
    let mut periods = Vec::new();
    for p in a {
        let start = time(p.get("startTime")?)?;
        let end = time(p.get("endTime")?)?.timestamp();
        if !(1..=16 * 3600).contains(&(end - start.timestamp()))
            || !(issued - 86400..=now + 10 * 86400).contains(&start.timestamp())
        {
            return None;
        }
        let day = p.get("isDaytime")?.as_bool()?;
        let pop = p
            .get("probabilityOfPrecipitation")
            .filter(|p| p.get("unitCode").and_then(Value::as_str) == Some("wmoUnit:percent"))
            .and_then(|p| p.get("value"))
            .and_then(Value::as_f64)
            .filter(|x| x.is_finite() && (0.0..=100.0).contains(x));
        periods.push(Period {
            start,
            end,
            day,
            temp: temp(p),
            short: p
                .get("shortForecast")
                .and_then(|v| clean(v, 180))
                .unwrap_or_default(),
            wind: wind(p),
            pop,
        });
    }
    periods.sort_by_key(|p| p.start.timestamp());
    if periods
        .windows(2)
        .any(|w| w[1].start.timestamp() != w[0].end)
    {
        return None;
    }
    if !periods
        .iter()
        .any(|p| p.end > now && p.start.timestamp() <= now + 18 * 3600)
    {
        return None;
    }
    Some(Snapshot {
        home: home_key(h),
        checked: now,
        issued,
        place,
        periods,
    })
}
async fn fetch(h: GeoView) -> Option<Snapshot> {
    if !h.lat.is_finite()
        || !h.lon.is_finite()
        || !(-90.0..=90.0).contains(&h.lat)
        || !(-180.0..=180.0).contains(&h.lon)
    {
        return None;
    }
    let point = super::report_feed::get_json(&format!(
        "https://api.weather.gov/points/{:.4},{:.4}",
        h.lat, h.lon
    ))
    .await?;
    if point.get("geometry")?.get("type")?.as_str()? != "Point" {
        return None;
    }
    let c = point.get("geometry")?.get("coordinates")?.as_array()?;
    if c.len() != 2
        || (c[0].as_f64()? - h.lon).abs() > 0.02
        || (c[1].as_f64()? - h.lat).abs() > 0.02
    {
        return None;
    }
    let p = point.get("properties")?;
    let url = forecast_url(p.get("forecast")?)?;
    let near = p.get("relativeLocation").and_then(|v| v.get("properties"));
    let place = near
        .and_then(|p| {
            Some(format!(
                "Near {}, {}",
                clean(p.get("city")?, 80)?,
                clean(p.get("state")?, 8)?
            ))
        })
        .unwrap_or_else(|| format!("Home point • {:.3}, {:.3}", h.lat, h.lon));
    decode(
        &super::report_feed::get_json(&url).await?,
        h,
        place,
        chrono::Utc::now().timestamp(),
    )
}
/// Fetch one independently verified NWS town-point forecast through the existing logged relay.
pub(super) async fn fetch_city(h: GeoView) -> Option<CitySnapshot> {
    fetch(h).await.map(CitySnapshot)
}

/// Read the selected local NWS day/night pair without exposing the parser's snapshot internals.
pub(super) fn city_day(s: &CitySnapshot, day: u8, now: i64) -> Option<CityDay> {
    if day > 2 || !(0..=1200).contains(&(now - s.0.checked))
        || !(0..=86400).contains(&(now - s.0.issued))
    {
        return None;
    }
    let (d, n) = selected(&s.0, day, now)?;
    let period = d.or(n)?;
    Some(CityDay {
        high: d.and_then(|p| p.temp).map(|v| format!("{v:.0}°F"))
            .unwrap_or_else(|| if day == 0 && d.is_none() { String::new() } else { MISSING.into() }),
        low: n.and_then(|p| p.temp).map(|v| format!("{v:.0}°F"))
            .unwrap_or_else(|| MISSING.into()),
        condition: if period.short.is_empty() { MISSING.into() } else { period.short.clone() },
        issue: chrono::DateTime::from_timestamp(s.0.issued, 0)
            .map(|t| format!("Updated {} UTC", t.format("%b %d %H:%M")))
            .unwrap_or_default(),
        date: period.start.format("%a %b %d").to_string(),
        remaining_night: day == 0 && d.is_none(),
    })
}
pub(super) async fn refresh(h: GeoView) {
    let ticket = match store().lock() {
        Ok(mut s) => {
            s.generation += 1;
            s.generation
        }
        Err(_) => return,
    };
    let bound = std::time::Duration::from_secs(50);
    let (result, county) = tokio::join!(
        tokio::time::timeout(bound, fetch(h)),
        tokio::time::timeout(bound, fetch_county(h))
    );
    if let Ok(mut s) = store().lock() {
        if s.generation == ticket {
            s.current = result.ok().flatten().map(Arc::new);
            s.county = county.ok().flatten().map(Arc::new);
        }
    }
}
fn selected(s: &Snapshot, day: u8, now: i64) -> Option<(Option<&Period>, Option<&Period>)> {
    let first = s.periods.iter().find(|p| p.end > now)?;
    let target = chrono::DateTime::from_timestamp(now, 0)?
        .with_timezone(first.start.offset())
        .date_naive()
        .checked_add_signed(chrono::Duration::days(day as i64))?;
    let daytime = s
        .periods
        .iter()
        .find(|p| p.day && p.end > now && p.start.date_naive() == target);
    let night = s.periods.iter().find(|p| {
        !p.day
            && p.end > now
            && p.start.date_naive() == target
            && daytime.is_none_or(|d| p.start.timestamp() >= d.end)
    });
    (daytime.is_some() || night.is_some()).then_some((daytime, night))
}
fn day_label(day: u8) -> String {
    match day {
        0 => "Today".into(),
        1 => "Tomorrow".into(),
        2 => "Day 3".into(),
        _ => format!("Day {}", u16::from(day) + 1),
    }
}
fn values(s: Option<&Snapshot>, day: u8, now: i64) -> Vec<(&'static str, String)> {
    let mut v = vec![
        (
            "source",
            "Source: NOAA / National Weather Service • Point forecast".into(),
        ),
        ("place", "Home point forecast".into()),
        ("period", day_label(day)),
        ("issued", String::new()),
        ("condition", MISSING.into()),
        ("daytemp", String::new()),
        ("daylabel", "DAYTIME".into()),
        ("nighttemp", String::new()),
        ("wind", String::new()),
        ("pop", String::new()),
        ("poplabel", "Precipitation chance".into()),
        ("note", "NWS day/night periods • Temperatures in °F".into()),
    ];
    let mut put = |k, value: String| {
        v.iter_mut().find(|(f, _)| *f == k).unwrap().1 = value;
    };
    let Some(s) = s else {
        return v;
    };
    let Some((d, n)) = selected(s, day, now) else {
        return v;
    };
    let p = d.or(n).unwrap();
    put("place", s.place.clone());
    put(
        "period",
        format!("{} • {}", day_label(day), p.start.format("%a %b %d")),
    );
    put(
        "issued",
        format!(
            "Updated {} UTC",
            chrono::DateTime::from_timestamp(s.issued, 0)
                .unwrap()
                .format("%b %d %H:%M")
        ),
    );
    put(
        "condition",
        if p.short.is_empty() {
            MISSING.into()
        } else {
            p.short.clone()
        },
    );
    put(
        "daytemp",
        d.and_then(|p| p.temp)
            .map(|x| format!("{x:.0}°F"))
            .unwrap_or_else(|| {
                if day == 0 && d.is_none() {
                    String::new()
                } else {
                    MISSING.into()
                }
            }),
    );
    if day == 0 && d.is_none() {
        put("daylabel", "DAYTIME PERIOD ENDED".into());
    }
    put(
        "nighttemp",
        n.and_then(|p| p.temp)
            .map(|x| format!("{x:.0}°F"))
            .unwrap_or_else(|| MISSING.into()),
    );
    put(
        "wind",
        if p.wind.is_empty() {
            MISSING.into()
        } else {
            p.wind.clone()
        },
    );
    put(
        "pop",
        p.pop
            .map(|x| format!("{x:.0}%"))
            .unwrap_or_else(|| MISSING.into()),
    );
    put(
        "poplabel",
        if p.day {
            "Daytime precip. chance"
        } else {
            "Nighttime precip. chance"
        }
        .into(),
    );
    put(
        "note",
        if d.is_none() {
            "Remaining nighttime forecast • Daytime temperature withheld"
        } else {
            "NWS day/night forecast periods • Temperatures in °F"
        }
        .into(),
    );
    v
}
fn values_on_date(
    s: Option<&Snapshot>,
    target: Option<chrono::NaiveDate>,
    name: &str,
    now: i64,
) -> Vec<(&'static str, String)> {
    let day = s.and_then(|s| {
        let p = s
            .periods
            .iter()
            .find(|p| p.end > now && p.start.timestamp() <= now)?;
        let today = chrono::DateTime::from_timestamp(now, 0)?
            .with_timezone(p.start.offset())
            .date_naive();
        let days = (target? - today).num_days();
        (0..=6).contains(&days).then_some(days as u8)
    });
    let mut result = values(if day.is_some() { s } else { None }, day.unwrap_or(0), now);
    for (field, value) in &mut result {
        if *field == "period" {
            *value = target
                .map(|d| format!("{name} • {}", d.format("%a %b %d, %Y")))
                .unwrap_or_else(|| "Published forecast date not available yet".into());
        }
        if *field == "note" {
            *value = if day.is_none() {
                "NWS forecast not yet available for the named date"
            } else if value.starts_with("Remaining nighttime") {
                "Remaining nighttime period • NWS home-point forecast"
            } else {
                "NWS home-point forecast • Valid only for the named date • °F"
            }
            .into();
        }
    }
    result
}
pub(super) fn named_date_values(
    home: GeoView,
    target: Option<chrono::NaiveDate>,
    name: &str,
    now: i64,
) -> Vec<(&'static str, String)> {
    let source = current(home, now);
    values_on_date(source.as_deref(), target, name, now)
}
fn current(home: GeoView, now: i64) -> Option<Arc<Snapshot>> {
    store().lock().ok().and_then(|s| {
        s.current
            .as_ref()
            .filter(|s| {
                s.home == home_key(home)
                    && (0..=1200).contains(&(now - s.checked))
                    && (0..=86400).contains(&(now - s.issued))
            })
            .cloned()
    })
}
/// Current home calendar date from an official NWS period's encoded UTC offset.
pub(super) fn home_date(home: GeoView, now: i64) -> Option<chrono::NaiveDate> {
    let s = current(home, now)?;
    let p = s.periods.iter().find(|p| p.end > now)?;
    Some(
        chrono::DateTime::from_timestamp(now, 0)?
            .with_timezone(p.start.offset())
            .date_naive(),
    )
}
pub(super) fn daily_values(home: GeoView, county: bool, day: u8, now: i64) -> Vec<(&'static str, String)> {
    let point_snapshot = if county { None } else { current(home, now) };
    let county_snapshot = if county {
        store().lock().ok().and_then(|s| {
            s.county.as_ref().filter(|snapshot| {
                snapshot.origin == home_key(home)
                    && (0..=1200).contains(&(now - snapshot.forecast.checked))
                    && (0..=86400).contains(&(now - snapshot.forecast.issued))
            }).cloned()
        })
    } else { None };
    let mut fields = if county {
        values(county_snapshot.as_ref().map(|s| &s.forecast), day, now)
    } else {
        values(point_snapshot.as_deref(), day, now)
    };
    if county {
        let place = county_snapshot.as_ref().map(|s| s.place.as_str())
            .unwrap_or("County representative-point forecast");
        for (key, value) in &mut fields {
            if *key == "source" {
                *value = "Source: NOAA / NWS • County representative-point forecast".into();
            } else if *key == "place" {
                *value = place.to_owned();
            } else if *key == "note" {
                *value = if value.starts_with("Remaining nighttime") {
                    "Remaining night • Representative point • County conditions vary".into()
                } else {
                    "Representative point • County conditions vary • Temperatures in °F".into()
                };
            }
        }
    }
    fields
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let s = current(settings.home, now);
    let county = store().lock().ok().and_then(|s| {
        s.county
            .as_ref()
            .filter(|s| {
                s.origin == home_key(settings.home)
                    && (0..=1200).contains(&(now - s.forecast.checked))
                    && (0..=86400).contains(&(now - s.forecast.issued))
            })
            .cloned()
    });
    for day in 0..3 {
        for (k, v) in values(s.as_deref(), day, now) {
            b.set(&format!("set.socialforecast.{day}.{k}"), &v);
        }
        for (k, mut v) in values(county.as_ref().map(|s| &s.forecast), day, now) {
            if k == "source" {
                v = "Source: NOAA / NWS • County representative-point forecast".into();
            }
            if k == "place" {
                v = county
                    .as_ref()
                    .map(|s| s.place.clone())
                    .unwrap_or_else(|| "County representative-point forecast".into());
            }
            if k == "note" {
                v = if v.starts_with("Remaining nighttime") {
                    "Remaining night • Representative point • County conditions vary"
                } else {
                    "Representative point • County conditions vary • Temperatures in °F"
                }
                .into();
            }
            b.set(&format!("set.countyforecast.{day}.{k}"), &v);
        }
    }
}
fn token(s: Spec, f: &str) -> String {
    format!(
        "{{{{set.{}.{}.{f}}}}}",
        if s.county {
            "countyforecast"
        } else {
            "socialforecast"
        },
        s.day
    )
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|s| s.key == key)?;
    let t = |f| token(s, f);
    let title = if s.county {
        "County Forecast"
    } else {
        "Your Local Forecast"
    };
    let mut d = if s.format == 0 {
        frame(title, "Forecast", &t("period"), &t("source"))
    } else {
        social_frame(title, "Forecast", &t("period"), &t("source"), s.format == 1)
    };
    let (x, w, place_y, condition_y, card_y, card_h, details_y, note_y) = match s.format {
        1 => (54., 972., 270., 380., 680., 360., 1110., 1660.),
        2 => (54., 972., 210., 300., 440., 230., 715., 920.),
        _ => (80., 1760., 210., 310., 480., 250., 755., 925.),
    };
    label(
        &mut d,
        "Forecast place",
        (x, place_y, w, 65.),
        &t("place"),
        if s.format == 0 { 43. } else { 36. },
        "$text",
        Align::Left,
    );
    label(
        &mut d,
        "Forecast condition",
        (x, condition_y, w, if s.format == 1 { 220. } else { 110. }),
        &t("condition"),
        if s.format == 1 {
            66.
        } else if s.format == 2 {
            43.
        } else {
            64.
        },
        "$text",
        Align::Left,
    );
    for i in 0..2 {
        let cw = (w - 24.) / 2.;
        let cx = x + i as f32 * (cw + 24.);
        d.layers.push(
            crate::gfx::templates::panel(&format!("Temperature panel {i}"), cx, card_y, cw, card_h)
                .with_anim(AnimKind::Rise, 0.2 + i as f32 * 0.1, 0.4),
        );
        label(
            &mut d,
            &format!("Temperature heading {i}"),
            (cx + 20., card_y + 12., cw - 40., 52.),
            &if i == 0 {
                t("daylabel")
            } else {
                "NIGHTTIME".into()
            },
            if s.format == 2 { 25. } else { 31. },
            "$muted",
            Align::Left,
        );
        label(
            &mut d,
            &format!("Temperature value {i}"),
            (cx + 20., card_y + 80., cw - 40., card_h - 100.),
            &t(if i == 0 { "daytemp" } else { "nighttemp" }),
            if s.format == 1 { 92. } else { 86. },
            "$text",
            Align::Left,
        );
    }
    let gap = if s.format == 1 { 170. } else { 0. };
    let detail_w = if s.format == 1 { w } else { w * 0.49 };
    label(
        &mut d,
        "Wind heading",
        (x, details_y, detail_w, 42.),
        "WIND",
        26.,
        "$muted",
        Align::Left,
    );
    label(
        &mut d,
        "Wind value",
        (
            x,
            details_y + 50.,
            detail_w,
            if s.format == 1 { 90. } else { 65. },
        ),
        &t("wind"),
        if s.format == 2 { 28. } else { 40. },
        "$text",
        Align::Left,
    );
    let px = if s.format == 1 { x } else { x + w * 0.51 };
    label(
        &mut d,
        "Precipitation heading",
        (px, details_y + gap, detail_w, 42.),
        &t("poplabel"),
        26.,
        "$muted",
        Align::Left,
    );
    label(
        &mut d,
        "Precipitation value",
        (
            px,
            details_y + gap + 50.,
            detail_w,
            if s.format == 1 { 90. } else { 65. },
        ),
        &t("pop"),
        if s.format == 2 { 32. } else { 45. },
        "$text",
        Align::Left,
    );
    label(
        &mut d,
        "Forecast issue",
        (
            x,
            if s.format == 1 {
                1510.
            } else if s.format == 2 {
                865.
            } else {
                880.
            },
            w,
            40.,
        ),
        &t("issued"),
        24.,
        "$muted",
        Align::Left,
    );
    label(
        &mut d,
        "Forecast scope note",
        (x, note_y, w, 40.),
        &t("note"),
        if s.format == 2 { 21. } else { 25. },
        "$muted",
        Align::Left,
    );
    Some(d)
}
#[cfg(test)]
pub(super) fn sample(d: &mut GraphicDoc) {
    let Some(s) = SPECS.iter().find(|s| Some(s.key) == d.template.as_deref()) else {
        return;
    };
    let namespace = if s.county {
        "countyforecast"
    } else {
        "socialforecast"
    };
    for (k, v) in [
        ("source", "SAMPLE ONLY • Not for broadcast"),
        ("place", "Near Jeffersonville, IN • SAMPLE"),
        ("condition", "Chance of showers, then partly cloudy"),
        ("daytemp", "74°F"),
        ("nighttemp", "51°F"),
        ("wind", "Southwest 10 to 15 mph"),
        ("pop", "40%"),
        ("poplabel", "Daytime precip. chance"),
        ("issued", "SAMPLE • Updated Nov 13 12:00 UTC"),
        ("note", "SAMPLE • Synthetic NWS-style day/night periods"),
    ] {
        d.data_overrides
            .push((format!("set.{namespace}.{}.{k}", s.day), v.into()));
    }
    if s.day == 0 {
        for (k, v) in [
            ("daytemp", ""),
            ("daylabel", "DAYTIME PERIOD ENDED"),
            ("condition", "Partly cloudy overnight"),
            ("poplabel", "Nighttime precip. chance"),
            (
                "note",
                "SAMPLE • Remaining nighttime forecast • Daytime temperature withheld",
            ),
            ("period", "Today • Fri Nov 13"),
        ] {
            d.data_overrides
                .push((format!("set.{namespace}.{}.{k}", s.day), v.into()));
        }
    }
    if s.county {
        let day = chrono::NaiveDate::from_ymd_opt(2026, 11, 14).unwrap()
            + chrono::Duration::days(s.day as i64);
        d.data_overrides.push((
            format!("set.{namespace}.{}.period", s.day),
            format!(
                "SAMPLE • {} • {}",
                ["Today", "Tomorrow", "Day 3"][s.day as usize],
                day.format("%a %b %d")
            ),
        ));
        d.data_overrides.push((
            format!("set.{namespace}.{}.issued", s.day),
            "SAMPLE • Updated Nov 14 12:00 UTC".into(),
        ));
        for (k, v) in [
            ("place", "Clark, IN • County representative point • SAMPLE"),
            (
                "note",
                if s.day == 0 {
                    "SAMPLE • Remaining night at representative point • County conditions vary"
                } else {
                    "SAMPLE • Representative point • County conditions vary • Temperatures in °F"
                },
            ),
        ] {
            d.data_overrides
                .push((format!("set.{namespace}.{}.{k}", s.day), v.into()));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn named_dates_select_the_published_calendar_day_and_withhold_far_future_weather() {
        let now = 1794574800;
        let s = decode(&fixture(), home(), "Home point".into(), now).unwrap();
        let tomorrow = chrono::NaiveDate::from_ymd_opt(2026, 11, 14).unwrap();
        let value = values_on_date(Some(&s), Some(tomorrow), "Named event", now);
        assert!(value.contains(&("daytemp", "70°F".into())));
        assert!(value
            .iter()
            .any(|(k, v)| *k == "period" && v.contains("Nov 14, 2026")));
        let far = values_on_date(
            Some(&s),
            Some(tomorrow + chrono::Duration::days(30)),
            "Named event",
            now,
        );
        assert!(far.contains(&("condition", MISSING.into())));
        assert!(far.contains(&("daytemp", String::new())));
        assert!(far.contains(&("nighttemp", String::new())));
        assert!(
            values_on_date(None, None, "Named event", now).contains(&("condition", MISSING.into()))
        );
    }
    #[test]
    fn county_representative_point_requires_home_and_sample_inside_actual_rings() {
        let square = vec![vec![(0., 0.), (4., 0.), (4., 4.), (0., 4.), (0., 0.)]];
        let centre = county_point(&square, (2., 2.)).unwrap();
        assert_eq!(
            super::super::arcgis_geometry::contains(&square, centre.0, centre.1),
            Some(true)
        );
        assert!(county_point(&square, (5., 2.)).is_none());
        assert!(county_point(&square, (0., 2.)).is_none());
        let concave = vec![vec![
            (0., 0.),
            (4., 0.),
            (4., 4.),
            (3., 4.),
            (3., 1.),
            (1., 1.),
            (1., 4.),
            (0., 4.),
            (0., 0.),
        ]];
        assert!(
            county_point(&concave, (0.5, 2.)).is_none(),
            "Never move a sample outside a concave county"
        );
        let mut hole = square.clone();
        hole.push(vec![(1., 1.), (3., 1.), (3., 3.), (1., 3.), (1., 1.)]);
        assert!(county_point(&hole, (2., 2.)).is_none());
    }
    fn home() -> GeoView {
        GeoView {
            lon: -86.,
            lat: 38.,
            zoom: 8.,
        }
    }
    fn fixture() -> Value {
        json!({"properties":{"updateTime":"2026-11-13T12:00:00Z","periods":[{"startTime":"2026-11-13T06:00:00-05:00","endTime":"2026-11-13T18:00:00-05:00","isDaytime":true,"temperature":20.,"temperatureUnit":"C","shortForecast":"Partly cloudy","windSpeed":"5 mph","windDirection":"SW","probabilityOfPrecipitation":{"unitCode":"wmoUnit:percent","value":null}},{"startTime":"2026-11-13T18:00:00-05:00","endTime":"2026-11-14T06:00:00-05:00","isDaytime":false,"temperature":45.,"temperatureUnit":"F","shortForecast":"Clear"},{"startTime":"2026-11-14T06:00:00-05:00","endTime":"2026-11-14T18:00:00-05:00","isDaytime":true,"temperature":70.,"temperatureUnit":"F","shortForecast":"Sunny"}]}})
    }
    #[test]
    fn nws_source_url_and_temperature_units_are_explicit() {
        assert!(forecast_url(&json!(
            "https://api.weather.gov/gridpoints/LMK/77,85/forecast"
        ))
        .is_some());
        for s in [
            "https://evil.example/gridpoints/LMK/77,85/forecast",
            "https://api.weather.gov/gridpoints/LMK/77,85/forecast?url=evil",
            "https://api.weather.gov/gridpoints/LMK/77,85/forecast/hourly",
        ] {
            assert!(forecast_url(&json!(s)).is_none());
        }
        assert_eq!(
            temp(&json!({"temperature":{"value":20.,"unitCode":"wmoUnit:degC"}})),
            Some(68.)
        );
        assert!(temp(&json!({"temperature":999.,"temperatureUnit":"F"})).is_none());
    }
    #[test]
    fn period_dates_preserve_today_tonight_and_missing_chances() {
        let now = 1794574800;
        let s = decode(&fixture(), home(), "Home point".into(), now).unwrap();
        let v = values(Some(&s), 0, now);
        assert!(v.contains(&("daytemp", "68°F".into())));
        assert!(v.contains(&("pop", MISSING.into())));
        let evening = now + 12 * 3600;
        let v = values(Some(&s), 0, evening);
        assert!(v.contains(&("daytemp", String::new())));
        assert!(v.contains(&("daylabel", "DAYTIME PERIOD ENDED".into())));
        assert!(v.contains(&("nighttemp", "45°F".into())));
        assert!(selected(&s, 2, now).is_none());
    }
    #[test]
    fn stale_or_conflicting_forecast_periods_are_withheld() {
        let now = 1794574800;
        assert!(decode(&fixture(), home(), "Home".into(), now + 86401).is_none());
        let mut v = fixture();
        v["properties"]["periods"][1]["startTime"] = json!("2026-11-13T17:00:00-05:00");
        assert!(decode(&v, home(), "Home".into(), now).is_none());
    }
}




include!("town_forecast_maps_social_api.rs");
