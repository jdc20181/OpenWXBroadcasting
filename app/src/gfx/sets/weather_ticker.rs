//! NWS station temperatures and explicitly located NWS forecasts in a transparent crawl.
use super::{
    arcgis_geometry,
    observed::{self, Coverage},
    reference::{lowerthird_frame, MISSING},
};
use crate::{
    config::{GeoView, Settings},
    gfx::{
        bind::Bindings,
        model::{AnimKind, Fill, GraphicDoc, Layer, LayerKind},
    },
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, OnceLock},
};
#[derive(Clone)]
struct Station {
    id: String,
    name: String,
    lon: f64,
    lat: f64,
    county: bool,
}
#[derive(Clone)]
struct Observation {
    station: Station,
    time: i64,
    temp: f64,
}
struct Snapshot {
    home: [u64; 2],
    checked: i64,
    observations: Vec<Observation>,
}
#[derive(Default)]
struct Store {
    generation: u64,
    current: Option<Arc<Snapshot>>,
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
    (!s.is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control))
        .then(|| s.into())
}
fn point(v: &Value) -> Option<(f64, f64)> {
    if v.get("type")?.as_str()? != "Point" {
        return None;
    }
    let xy = v.get("coordinates")?.as_array()?;
    if xy.len() != 2 {
        return None;
    }
    let (x, y) = (xy[0].as_f64()?, xy[1].as_f64()?);
    (x.is_finite() && y.is_finite() && (-180.0..=180.).contains(&x) && (-90.0..=90.).contains(&y))
        .then_some((x, y))
}
fn stations_url(v: &Value) -> Option<String> {
    let tail = v
        .as_str()?
        .strip_prefix("https://api.weather.gov/gridpoints/")?;
    let parts: Vec<_> = tail.split('/').collect();
    if parts.len() != 3
        || parts[0].len() != 3
        || !parts[0].bytes().all(|b| b.is_ascii_uppercase())
        || parts[2] != "stations"
    {
        return None;
    }
    let (x, y) = parts[1].split_once(',')?;
    if ![x, y].iter().all(|s| {
        !s.is_empty()
            && s.bytes().all(|b| b.is_ascii_digit())
            && s.parse::<u16>().is_ok_and(|n| n <= 2000)
    }) {
        return None;
    }
    Some(format!(
        "https://api.weather.gov/gridpoints/{}/{}/stations",
        parts[0], parts[1]
    ))
}
fn station_id(v: &Value) -> Option<String> {
    let id = v.as_str()?;
    (3..=6).contains(&id.len()).then_some(())?;
    id.bytes()
        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        .then(|| id.into())
}
fn observation(v: &Value, station: Station, now: i64) -> Option<Observation> {
    let (x, y) = point(v.get("geometry")?)?;
    if (x - station.lon).abs() > 0.02 || (y - station.lat).abs() > 0.02 {
        return None;
    }
    let p = v.get("properties")?;
    if p.get("station")?.as_str()? != format!("https://api.weather.gov/stations/{}", station.id) {
        return None;
    }
    let time = chrono::DateTime::parse_from_rfc3339(p.get("timestamp")?.as_str()?)
        .ok()?
        .timestamp();
    if !(0..=5400).contains(&(now - time)) {
        return None;
    }
    let t = p.get("temperature")?;
    // MADIS V = passed all three QC levels. Preliminary/questioned/rejected data is withheld.
    if t.get("qualityControl")?.as_str()? != "V" {
        return None;
    }
    let n = t.get("value")?.as_f64()?;
    let temp = match t.get("unitCode")?.as_str()? {
        "wmoUnit:degC" => n * 1.8 + 32.,
        "wmoUnit:degF" => n,
        _ => return None,
    };
    if !temp.is_finite() || !(-150.0..=150.).contains(&temp) {
        return None;
    }
    Some(Observation {
        station,
        time,
        temp,
    })
}
async fn fetch(home: GeoView) -> Option<Snapshot> {
    if !home.lon.is_finite()
        || !home.lat.is_finite()
        || !(-180.0..=180.).contains(&home.lon)
        || !(-90.0..=90.).contains(&home.lat)
    {
        return None;
    }
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(50);
    let metadata = tokio::time::timeout_at(
        deadline,
        super::report_feed::get_json(&format!(
            "https://api.weather.gov/points/{:.4},{:.4}",
            home.lat, home.lon
        )),
    )
    .await
    .ok()??;
    let (x, y) = point(metadata.get("geometry")?)?;
    if (x - home.lon).abs() > 0.02 || (y - home.lat).abs() > 0.02 {
        return None;
    }
    let url = stations_url(metadata.get("properties")?.get("observationStations")?)?;
    let list = tokio::time::timeout_at(deadline, super::report_feed::get_json(&url))
        .await
        .ok()??;
    let features = list.get("features")?.as_array()?;
    if features.len() > 500 {
        return None;
    }
    // Discovery is a nearby-station list, not a complete county/region measurement network.
    let all = crate::map::boundaries::counties_named()?;
    let counties: Vec<_> = all
        .iter()
        .filter(|c| arcgis_geometry::contains(&c.rings, home.lon, home.lat) == Some(true))
        .collect();
    let county = if counties.len() == 1 {
        Some(counties[0])
    } else {
        None
    };
    let mut settings = Settings::default();
    settings.home = home;
    let scope = observed::report_scope(&settings, Coverage::Region);
    let [west, south, east, north] = scope.bbox?;
    let mut stations = BTreeMap::new();
    let mut conflicts = BTreeSet::new();
    for f in features {
        let Some(p) = f.get("properties") else {
            continue;
        };
        let (Some(id), Some(name), Some((lon, lat))) = (
            p.get("stationIdentifier").and_then(station_id),
            p.get("name").and_then(|v| clean(v, 80)),
            f.get("geometry").and_then(point),
        ) else {
            continue;
        };
        if !(west..=east).contains(&lon) || !(south..=north).contains(&lat) {
            continue;
        }
        let in_county =
            county.is_some_and(|c| arcgis_geometry::contains(&c.rings, lon, lat) == Some(true));
        if let Some(old) = stations.get(&id) {
            let old: &Station = old;
            if old.lon != lon || old.lat != lat || old.name != name {
                conflicts.insert(id.clone());
            }
        }
        stations.insert(
            id.clone(),
            Station {
                id,
                name,
                lon,
                lat,
                county: in_county,
            },
        );
    }
    let mut stations: Vec<_> = stations
        .into_values()
        .filter(|s| !conflicts.contains(&s.id))
        .collect();
    stations.sort_by(|a, b| {
        b.county
            .cmp(&a.county)
            .then_with(|| {
                let d = |s: &Station| {
                    (s.lon - home.lon).powi(2) * home.lat.to_radians().cos().powi(2)
                        + (s.lat - home.lat).powi(2)
                };
                d(a).total_cmp(&d(b))
            })
            .then_with(|| a.id.cmp(&b.id))
    });
    stations.truncate(8);
    let mut pending = stations.into_iter();
    let mut tasks = tokio::task::JoinSet::new();
    let launch = |tasks: &mut tokio::task::JoinSet<Option<Observation>>, station: Station| {
        tasks.spawn(async move {
            let value = super::report_feed::get_json(&format!(
                "https://api.weather.gov/stations/{}/observations/latest",
                station.id
            ))
            .await?;
            observation(&value, station, chrono::Utc::now().timestamp())
        });
    };
    for station in pending.by_ref().take(4) {
        launch(&mut tasks, station);
    }
    let mut observations = Vec::new();
    while !tasks.is_empty() {
        let Ok(Some(result)) = tokio::time::timeout_at(deadline, tasks.join_next()).await else {
            break;
        };
        if let Ok(Some(obs)) = result {
            observations.push(obs);
        }
        if let Some(station) = pending.next() {
            launch(&mut tasks, station);
        }
    }
    tasks.abort_all();
    observations.sort_by(|a, b| a.station.name.cmp(&b.station.name));
    Some(Snapshot {
        home: home_key(home),
        checked: chrono::Utc::now().timestamp(),
        observations,
    })
}
pub(super) async fn refresh(home: GeoView) {
    let ticket = match store().lock() {
        Ok(mut s) => {
            s.generation += 1;
            s.generation
        }
        Err(_) => return,
    };
    let result = fetch(home).await;
    if let Ok(mut s) = store().lock() {
        if s.generation == ticket {
            s.current = result.map(Arc::new);
        }
    }
}
fn compose(b: &Bindings, observations: &[Observation], county: bool, now: i64) -> String {
    let temps: Vec<_> = observations
        .iter()
        .filter(|o| (!county || o.station.county) && (0..=5400).contains(&(now - o.time)))
        .map(|o| {
            format!(
                "{} ({}) {:.0}°F • Observed {} UTC",
                o.station.name,
                o.station.id,
                o.temp,
                chrono::DateTime::from_timestamp(o.time, 0)
                    .unwrap()
                    .format("%b %d %H:%M")
            )
        })
        .collect();
    let mut parts = vec![if temps.is_empty() {
        "NWS observations: Data not available yet".into()
    } else {
        format!(
            "Available NWS station temperatures: {}",
            temps.join("  |  ")
        )
    }];
    let prefix = if county {
        "set.countyforecast.0"
    } else {
        "set.socialforecast.0"
    };
    let get = |f: &str| {
        b.get(&format!("{prefix}.{f}"))
            .unwrap_or(MISSING)
            .to_owned()
    };
    let condition = get("condition");
    let heading = if county {
        "County representative-point forecast"
    } else {
        "Home-point forecast"
    };
    if condition == MISSING || condition.trim().is_empty() {
        parts.push(format!("{heading}: {MISSING}"));
    } else {
        let mut forecast = vec![format!(
            "{heading} • {} • {} • {condition}",
            get("place"),
            get("period")
        )];
        for (label, field) in [
            (get("daylabel"), "daytemp"),
            ("Nighttime".into(), "nighttemp"),
            ("Wind".into(), "wind"),
            (get("poplabel"), "pop"),
        ] {
            let value = get(field);
            if !value.trim().is_empty() {
                forecast.push(format!("{label}: {value}"));
            }
        }
        parts.push(forecast.join(" • "));
    }
    parts.push("Available stations only • Local conditions vary • Source: NOAA / NWS".into());
    parts.join("    |    ")
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    b.set(
        "set.weatherticker.source",
        "Source: NOAA / National Weather Service",
    );
    let now = chrono::Utc::now().timestamp();
    let snapshot = store().lock().ok().and_then(|s| {
        s.current
            .as_ref()
            .filter(|s| {
                s.home == home_key(settings.home) && (0..=1200).contains(&(now - s.checked))
            })
            .cloned()
    });
    let obs = snapshot
        .as_ref()
        .map(|s| s.observations.as_slice())
        .unwrap_or(&[]);
    for county in [false, true] {
        let prefix = if county { "county" } else { "region" };
        let text = compose(b, obs, county, now);
        b.set(&format!("set.weatherticker.{prefix}.text"), &text);
    }
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let county = match key {
        "set_weather_ticker__county" => true,
        "set_weather_ticker__region" => false,
        _ => return None,
    };
    let mut doc = lowerthird_frame(
        "Local Weather",
        "Weather ticker",
        if county {
            "Home county • Representative-point forecast"
        } else {
            "Regional stations • Home-point forecast"
        },
        "{{set.weatherticker.source}}",
    );
    for l in &mut doc.layers {
        let (x, y, w, h, font) = match l.name.as_str() {
            "Lower third Header" => (0., 870., 1920., 60., 0.),
            "Lower third Header accent" => (0., 930., 1920., 4., 0.),
            "Lower third Title" => (80., 880., 500., 42., 31.),
            "Lower third Header right" => (620., 885., 1220., 34., 24.),
            _ => continue,
        };
        (l.x, l.y, l.w, l.h) = (x, y, w, h);
        if let LayerKind::Text { size, .. } = &mut l.kind {
            *size = font;
        }
    }
    doc.layers.push(
        Layer::rect(
            "Weather crawl panel",
            0.,
            934.,
            1920.,
            62.,
            Fill::solid("$panel"),
            0.,
        )
        .with_anim(AnimKind::Fade, 0., 0.2),
    );
    doc.layers.push(
        Layer::new(
            "Weather crawl",
            80.,
            939.,
            1760.,
            52.,
            LayerKind::Ticker {
                text: format!(
                    "{{{{set.weatherticker.{}.text}}}}",
                    if county { "county" } else { "region" }
                ),
                font: "$body".into(),
                size: 40.,
                color: "$text".into(),
                speed: 96.,
            },
        )
        .with_anim(AnimKind::Fade, 0.1, 0.2),
    );
    doc.duration = 0.;
    Some(doc)
}
#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let county = match doc.template.as_deref() {
        Some("set_weather_ticker__county") => true,
        Some("set_weather_ticker__region") => false,
        _ => return,
    };
    let text="SAMPLE ONLY • Not for broadcast    |    Available NWS-style station temperatures: Jeffersonville Sample Station 63°F • Observed Nov 14 15:00 UTC    |    Forecast at a representative point: Tonight • Partly cloudy • Nighttime 51°F • Southwest 10 to 15 mph • Nighttime precipitation chance 40%    |    SAMPLE • Synthetic observations and forecast • Local conditions vary";
    doc.data_overrides.push((
        "set.weatherticker.source".into(),
        "SAMPLE ONLY • Not for broadcast".into(),
    ));
    doc.data_overrides.push((
        format!(
            "set.weatherticker.{}.text",
            if county { "county" } else { "region" }
        ),
        text.into(),
    ));
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn station() -> Station {
        Station {
            id: "KHNB".into(),
            name: "Huntingburg".into(),
            lon: -86.95,
            lat: 38.25,
            county: true,
        }
    }
    fn fixture() -> Value {
        json!({"geometry":{"type":"Point","coordinates":[-86.95,38.25]},"properties":{"station":"https://api.weather.gov/stations/KHNB","timestamp":"2026-11-14T15:00:00Z","temperature":{"value":20.,"unitCode":"wmoUnit:degC","qualityControl":"V"}}})
    }
    #[test]
    fn nws_observations_require_matching_station_location_fresh_time_units_and_verified_qc() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-14T15:20:00Z")
            .unwrap()
            .timestamp();
        assert_eq!(observation(&fixture(), station(), now).unwrap().temp, 68.);
        for qc in ["Z", "S", "X", "Q", ""] {
            let mut v = fixture();
            v["properties"]["temperature"]["qualityControl"] = json!(qc);
            assert!(observation(&v, station(), now).is_none());
        }
        let mut v = fixture();
        v["properties"]["temperature"]["value"] = Value::Null;
        assert!(observation(&v, station(), now).is_none());
        let mut v = fixture();
        v["properties"]["temperature"]["unitCode"] = json!("unknown");
        assert!(observation(&v, station(), now).is_none());
        let mut v = fixture();
        v["geometry"]["coordinates"][0] = json!(-90.);
        assert!(observation(&v, station(), now).is_none());
        let mut v = fixture();
        v["properties"]["station"] = json!("https://api.weather.gov/stations/KOTHER");
        assert!(observation(&v, station(), now).is_none());
        assert!(observation(&fixture(), station(), now + 7200).is_none());
        assert!(observation(&fixture(), station(), now - 3600).is_none());
        assert!(stations_url(&json!(
            "https://api.weather.gov/gridpoints/LMK/59,81/stations"
        ))
        .is_some());
        assert!(
            stations_url(&json!("https://example.com/gridpoints/LMK/59,81/stations")).is_none()
        );
        assert!(station_id(&json!("KHNB/evil")).is_none());
    }
    #[test]
    fn crawl_preserves_missing_data_and_moves_without_covering_the_map() {
        let b = Bindings::default();
        let text = compose(&b, &[], true, 0);
        assert!(text.contains(MISSING));
        assert!(!text.contains("0°F"));
        assert_crawl_moves(10., 14.);
    }
    #[test]
    fn crawl_continues_after_one_hour() {
        assert_crawl_moves(3601., 3605.);
    }
    fn assert_crawl_moves(first: f32, second: f32) {
        for key in ["set_weather_ticker__county", "set_weather_ticker__region"] {
            for look in ["modern", "classic"] {
                let mut doc = super::build(key).unwrap();
                doc.template = Some(key.into());
                sample(&mut doc);
                crate::gfx::look::apply(&mut doc, look);
                let settings = Settings::default();
                let data = crate::data::WxSnapshot::default();
                let render = |t| {
                    crate::program::render_graphic_mode(
                        &doc,
                        &settings,
                        &data,
                        t,
                        1920,
                        1080,
                        None,
                        crate::gfx::check::PreviewData::Sample,
                    )
                };
                let a = render(first);
                let c = render(second);
                assert_eq!(a.pixel(100, 100).unwrap().alpha(), 0);
                assert!(a.data() != c.data(), "Crawl must actually move");
            }
        }
    }
}
