//! NWS Damage Assessment Toolkit (DAT) tornado tracks for "Tornadoes This Year" and the
//! latest confirmed-survey card. Relay provider `dat` (services.dat.noaa.gov, Damage Lines
//! layer 1 only). Only NWS quality-controlled (`qc = 'Y'`) tracks with an EF rating
//! (EF0–EF5 or EFU) count; thunderstorm-wind lines are excluded. One record per DAT global ID;
//! a track counts for the home state when its survey start or end point lies inside it.
use crate::{
    config::{GeoView, Settings},
    gfx::bind::Bindings,
    map::boundaries::County,
};
use chrono::{Datelike, TimeZone};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

const LAYER: &str = "https://services.dat.noaa.gov/arcgis/rest/services/nws_damageassessmenttoolkit/DamageViewer/FeatureServer/1/query";
const PAGE: usize = 2000;
const MAX_PAGES: usize = 10;
const MAX_SNAPSHOT_AGE_SECS: i64 = 2 * 60 * 60;

fn snapshot_is_fresh(fetched_at: i64, now: i64) -> bool {
    now.checked_sub(fetched_at)
        .is_some_and(|age| (0..=MAX_SNAPSHOT_AGE_SECS).contains(&age))
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Track {
    pub id: String,
    pub name: String,
    pub start: i64,
    pub lat: f64,
    pub lon: f64,
    pub ef: Option<u8>,
    pub wind: Option<u16>,
    pub edited: Option<i64>,
}

#[derive(Deserialize)]
struct Attrs {
    globalid: Option<String>,
    event_id: Option<String>,
    starttime: Option<i64>,
    startlat: Option<f64>,
    startlon: Option<f64>,
    endlat: Option<f64>,
    endlon: Option<f64>,
    efscale: Option<String>,
    qc: Option<String>,
    maxwind: Option<i64>,
    last_edited_date: Option<i64>,
}
#[derive(Deserialize)]
struct Feature {
    attributes: Attrs,
}
#[derive(Deserialize)]
struct Page {
    features: Option<Vec<Feature>>,
    #[serde(default, rename = "exceededTransferLimit")]
    more: bool,
    error: Option<serde_json::Value>,
}

/// EF0–EF5 → Some(n); EFU → None (rated unknown, still a confirmed tornado). Anything else
/// (TSTM/Wind, blank) is not a tornado and returns Err.
fn rating(raw: &str) -> Result<Option<u8>, ()> {
    let s = raw.trim().to_ascii_uppercase();
    match s.strip_prefix("EF") {
        Some("U") => Ok(None),
        Some(n) => n
            .parse::<u8>()
            .ok()
            .filter(|v| *v <= 5)
            .map(Some)
            .ok_or(()),
        None => Err(()),
    }
}

fn inside(counties: &[&County], lat: f64, lon: f64) -> bool {
    counties.iter().any(|c| {
        c.rings.iter().fold(false, |inside, ring| {
            inside ^ crate::gfx::winter_map::inside_pub(ring, lon, lat)
        })
    })
}

/// Decode one query page into `out`. Returns whether more pages follow, or None on a
/// service error or malformed page (fail closed).
fn decode(
    raw: &str,
    in_state: impl Fn(f64, f64) -> bool,
    out: &mut BTreeMap<String, Track>,
) -> Option<bool> {
    let page: Page = serde_json::from_str(raw).ok()?;
    if page.error.is_some() {
        return None;
    }
    for f in page.features? {
        let a = f.attributes;
        if a.qc.as_deref().map(str::trim) != Some("Y") {
            continue;
        }
        let Ok(ef) = rating(a.efscale.as_deref().unwrap_or("")) else {
            continue;
        };
        let (Some(id), Some(start)) = (a.globalid, a.starttime) else {
            continue;
        };
        let ends = [(a.startlat, a.startlon), (a.endlat, a.endlon)];
        let Some((lat, lon)) = ends.into_iter().find_map(|(la, lo)| {
            let (la, lo) = (la?, lo?);
            (la.is_finite() && lo.is_finite() && in_state(la, lo)).then_some((la, lo))
        }) else {
            continue;
        };
        let t = Track {
            id: id.trim().to_string(),
            name: a.event_id.unwrap_or_default().trim().to_string(),
            start,
            lat,
            lon,
            ef,
            wind: a
                .maxwind
                .and_then(|w| u16::try_from(w).ok())
                .filter(|w| *w > 0),
            edited: a.last_edited_date,
        };
        // Keep the latest revision of a repeated global ID.
        match out.get(&t.id) {
            Some(prior) if prior.edited >= t.edited => {}
            _ => {
                out.insert(t.id.clone(), t);
            }
        }
    }
    Some(page.more)
}

type Year = (String, i32, i64, Arc<Vec<Track>>);

#[derive(Default)]
struct Store {
    generation: u64,
    attempted: Option<(i64, String)>,
    year: Option<Year>,
}
fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Store::default()))
}

fn bbox(counties: &[&County]) -> Option<(f64, f64, f64, f64)> {
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(lon, lat) in counties.iter().flat_map(|c| c.rings.iter().flatten()) {
        b = (b.0.min(lon), b.1.min(lat), b.2.max(lon), b.3.max(lat));
    }
    (b.0 < b.2).then_some(b)
}

fn query_url(b: (f64, f64, f64, f64), year: i32, offset: usize) -> String {
    let mut u = reqwest::Url::parse(LAYER).expect("static DAT url");
    u.query_pairs_mut()
        .append_pair(
            "where",
            &format!(
                "qc = 'Y' AND starttime >= TIMESTAMP '{year}-01-01 00:00:00' AND starttime < TIMESTAMP '{}-01-01 00:00:00'",
                year + 1
            ),
        )
        .append_pair(
            "geometry",
            &format!("{:.3},{:.3},{:.3},{:.3}", b.0, b.1, b.2, b.3),
        )
        .append_pair("geometryType", "esriGeometryEnvelope")
        .append_pair("inSR", "4326")
        .append_pair("spatialRel", "esriSpatialRelIntersects")
        .append_pair(
            "outFields",
            "globalid,event_id,starttime,startlat,startlon,endlat,endlon,efscale,qc,maxwind,last_edited_date",
        )
        .append_pair("returnGeometry", "false")
        .append_pair("orderByFields", "objectid")
        .append_pair("resultOffset", &offset.to_string())
        .append_pair("resultRecordCount", &PAGE.to_string())
        .append_pair("f", "json");
    u.into()
}

/// State FIPS of the county holding this point.
fn state_at(lat: f64, lon: f64) -> Option<String> {
    let all = crate::map::boundaries::counties_named()?;
    let found = all
        .iter()
        .find(|c| inside(&[c], lat, lon))
        .map(|c| c.state.clone());
    found
}

pub(super) async fn refresh(home: GeoView) {
    let Some(fips) = state_at(home.lat, home.lon) else {
        return;
    };
    let now = chrono::Utc::now();
    let ticket = {
        let Ok(mut s) = store().lock() else {
            return;
        };
        if s.attempted
            .as_ref()
            .is_some_and(|(at, st)| *st == fips && (0..1800).contains(&(now.timestamp() - at)))
        {
            return;
        }
        s.generation = s.generation.wrapping_add(1);
        s.attempted = Some((now.timestamp(), fips.clone()));
        s.generation
    };
    let Some(all) = crate::map::boundaries::counties_named() else {
        return;
    };
    let counties: Vec<&County> = all.iter().filter(|c| c.state == fips).collect();
    let Some(b) = bbox(&counties) else {
        return;
    };
    let year = chrono::Local::now().year();
    let mut tracks = BTreeMap::new();
    let mut complete = false;
    for page in 0..MAX_PAGES {
        let url = query_url(b, year, page * PAGE);
        let Some(bytes) = super::report_feed::get_bytes(&url, Duration::from_secs(40)).await
        else {
            break;
        };
        let Some(more) = std::str::from_utf8(&bytes)
            .ok()
            .and_then(|raw| decode(raw, |la, lo| inside(&counties, la, lo), &mut tracks))
        else {
            break;
        };
        if !more {
            complete = true;
            break;
        }
    }
    if let Ok(mut s) = store().lock() {
        if s.generation == ticket && complete {
            let list = tracks.into_values().collect();
            s.year = Some((fips, year, now.timestamp(), Arc::new(list)));
        }
    }
}

fn local_date(ms: i64) -> String {
    chrono::Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|t| t.format("%b %-d, %Y").to_string())
        .unwrap_or_else(|| super::reference::MISSING.into())
}

/// Bind a fetched year into the Tornadoes This Year and Confirmed Tornado prefixes. Runs
/// after `reference::bind`, so without data the reference placeholders stand; document data
/// overrides still win over both.
fn put(b: &mut Bindings, fips: &str, state_name: &str, year: i32, tracks: &[Track], fetched: i64) {
    #[derive(serde::Serialize)]
    struct Ev<'a> {
        id: &'a str,
        lon: f64,
        lat: f64,
        ef: Option<u8>,
    }
    let events: Vec<Ev> = tracks
        .iter()
        .map(|t| Ev {
            id: &t.id,
            lon: t.lon,
            lat: t.lat,
            ef: t.ef,
        })
        .collect();
    let mut payload = serde_json::json!({
        "state": fips,
        "reviewed": true,
        "events": events,
    });
    if let Some(avg) = fips
        .parse::<u8>()
        .ok()
        .and_then(super::tornado_history::annual_average)
    {
        payload["annual_average"] = serde_json::json!((avg * 10.).round() / 10.);
        payload["baseline"] = serde_json::json!("1991–2020 average (SPC)");
    }
    b.set("set.tornado.scope", state_name);
    b.set("set.tornado.payload", &payload.to_string());
    let latest = tracks.iter().max_by_key(|t| (t.start, t.id.as_str()));
    let survey_events: Vec<Ev> = latest.into_iter().map(|t| Ev {
        id: &t.id, lon: t.lon, lat: t.lat, ef: t.ef,
    }).collect();
    b.set("set.survey.payload", &serde_json::json!({
        "state": fips, "reviewed": true, "events": survey_events,
    }).to_string());
    b.set(
        "set.tornado.period",
        &format!("{year} • Year to date • {} surveyed", tracks.len()),
    );
    let checked = chrono::Local
        .timestamp_opt(fetched, 0)
        .single()
        .map(|t| t.format(" • Checked %b %-d %-I:%M %p").to_string())
        .unwrap_or_default();
    b.set(
        "set.tornado.source",
        &format!("Source: NWS Damage Assessment Toolkit • QC-reviewed surveys{checked}"),
    );
    b.set(
        "set.tornado.note",
        "Preliminary NWS surveys; ratings can change. Tracks counted where they start or end in the state.",
    );
    if let Some(t) = latest {
        b.set("set.survey.status", "Confirmed tornado");
        b.set(
            "set.survey.location",
            if t.name.is_empty() { state_name } else { &t.name },
        );
        b.set(
            "set.survey.rating",
            &t.ef
                .map(|e| format!("EF{e}"))
                .unwrap_or_else(|| "EF-Unknown".into()),
        );
        b.set(
            "set.survey.wind",
            &t.wind
                .map(|w| format!("{w} mph"))
                .unwrap_or_else(|| super::reference::MISSING.into()),
        );
        b.set("set.survey.date", &local_date(t.start));
        b.set(
            "set.survey.note",
            "Source: NWS Damage Assessment Toolkit. Preliminary; ratings and winds may change after review.",
        );
    }
}

pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let Some(fips) = state_at(settings.home.lat, settings.home.lon) else {
        return;
    };
    let Some((st, year, fetched, tracks)) = store().lock().ok().and_then(|s| s.year.clone())
    else {
        return;
    };
    if st != fips || year != chrono::Local::now().year()
        || !snapshot_is_fresh(fetched, chrono::Utc::now().timestamp()) {
        return;
    }
    let name = crate::winter::STATES
        .iter()
        .find(|s| s.0 == fips)
        .map(|s| s.2)
        .unwrap_or("State");
    put(b, &fips, name, year, &tracks, fetched);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dat_snapshot_freshness_rejects_stale_and_future_fetches() {
        assert!(snapshot_is_fresh(10_000, 10_000));
        assert!(snapshot_is_fresh(10_000, 10_000 + MAX_SNAPSHOT_AGE_SECS));
        assert!(!snapshot_is_fresh(10_000, 10_001 + MAX_SNAPSHOT_AGE_SECS));
        assert!(!snapshot_is_fresh(10_001, 10_000));
    }

    #[test]
    fn keeps_only_qc_tornado_tracks_in_state_and_latest_revision() {
        let in_state =
            |la: f64, lo: f64| (38.0..=40.0).contains(&la) && (-87.0..=-85.0).contains(&lo);
        let raw = r#"{"features":[
          {"attributes":{"globalid":"{A}","event_id":"Inside EF1","starttime":1781223780000,"startlat":39.0,"startlon":-86.0,"efscale":"EF1","qc":"Y","maxwind":90,"last_edited_date":1}},
          {"attributes":{"globalid":"{A}","event_id":"Inside EF2 revised","starttime":1781223780000,"startlat":39.0,"startlon":-86.0,"efscale":"EF2","qc":"Y","maxwind":115,"last_edited_date":5}},
          {"attributes":{"globalid":"{B}","event_id":"Ends inside","starttime":1781223790000,"startlat":41.0,"startlon":-86.0,"endlat":39.5,"endlon":-86.5,"efscale":"EFU","qc":"Y"}},
          {"attributes":{"globalid":"{C}","event_id":"Wind","starttime":1781223790000,"startlat":39.0,"startlon":-86.0,"efscale":"TSTM/Wind","qc":"Y"}},
          {"attributes":{"globalid":"{D}","event_id":"Unreviewed","starttime":1781223790000,"startlat":39.0,"startlon":-86.0,"efscale":"EF0","qc":"N"}},
          {"attributes":{"globalid":"{E}","event_id":"Outside","starttime":1781223790000,"startlat":42.0,"startlon":-86.0,"endlat":42.1,"endlon":-86.0,"efscale":"EF0","qc":"Y"}}
        ],"exceededTransferLimit":true}"#;
        let mut out = BTreeMap::new();
        assert_eq!(decode(raw, in_state, &mut out), Some(true));
        assert_eq!(out.len(), 2);
        assert_eq!(out["{A}"].ef, Some(2));
        assert_eq!(out["{A}"].wind, Some(115));
        assert_eq!(out["{B}"].ef, None);
        assert_eq!((out["{B}"].lat, out["{B}"].lon), (39.5, -86.5));
        assert_eq!(decode(r#"{"error":{"code":400}}"#, in_state, &mut out), None);
    }

    #[test]
    fn bound_payload_is_a_reviewed_tornado_payload() {
        let track = |id: &str, name: &str, start, ef, wind| Track {
            id: id.into(),
            name: name.into(),
            start,
            lat: 39.,
            lon: -86.,
            ef,
            wind,
            edited: None,
        };
        let tracks = vec![
            track("{A}", "Paxton", 1_781_223_780_000, Some(1), Some(90)),
            track("{B}", "Later", 1_786_475_580_000, None, None),
        ];
        let mut b = Bindings::default();
        put(&mut b, "18", "Indiana", 2026, &tracks, 1_786_000_000);
        let p = super::super::tornado::parse(b.get("set.tornado.payload").unwrap()).unwrap();
        assert!(p.reviewed);
        assert_eq!(p.events.unwrap().len(), 2);
        let survey = super::super::tornado::parse(b.get("set.survey.payload").unwrap()).unwrap();
        let survey_events = survey.events.unwrap();
        assert_eq!(survey_events.len(), 1);
        assert_eq!(survey_events[0].id, "{B}");
        assert_eq!(b.get("set.survey.location"), Some("Later"));
        assert_eq!(b.get("set.survey.rating"), Some("EF-Unknown"));
        assert!(b
            .get("set.tornado.source")
            .unwrap()
            .contains("Damage Assessment Toolkit"));
    }
}
