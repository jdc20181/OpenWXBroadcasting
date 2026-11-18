//! Published experimental NWS HeatRisk categories, sampled at actual county centres.
use super::{
    board::{self, Band, Layout, ReportPoint, Scene},
    observed::{self, Coverage},
};
use crate::{
    config::{GeoView, Settings},
    gfx::{bind::Bindings, model::GraphicDoc},
};
use chrono::NaiveDate;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, OnceLock},
};
const BASE: &str =
    "https://mapservices.weather.noaa.gov/experimental/rest/services/NWS_HeatRisk/ImageServer";
const CATEGORIES: [(&str, &str); 5] = [
    ("0 • Little / no risk", "#E8F9E7"),
    ("1 • Minor", "#F4F257"),
    ("2 • Moderate", "#F69632"),
    ("3 • Major", "#E22F33"),
    ("4 • Extreme", "#7A0E7F"),
];
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    day: u8,
    coverage: Coverage,
    format: u8,
}
const SPECS: &[Spec] = &[
    Spec {
        key: "set_heat_risk__today_county_overlay",
        day: 0,
        coverage: Coverage::County,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__today_region_overlay",
        day: 0,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__today_state_overlay",
        day: 0,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__today_multi_overlay",
        day: 0,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__tomorrow_county_overlay",
        day: 1,
        coverage: Coverage::County,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__tomorrow_region_overlay",
        day: 1,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__tomorrow_state_overlay",
        day: 1,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__tomorrow_multi_overlay",
        day: 1,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day3_county_overlay",
        day: 2,
        coverage: Coverage::County,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day3_region_overlay",
        day: 2,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day3_state_overlay",
        day: 2,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day3_multi_overlay",
        day: 2,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day4_county_overlay",
        day: 3,
        coverage: Coverage::County,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day4_region_overlay",
        day: 3,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day4_state_overlay",
        day: 3,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day4_multi_overlay",
        day: 3,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day5_county_overlay",
        day: 4,
        coverage: Coverage::County,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day5_region_overlay",
        day: 4,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day5_state_overlay",
        day: 4,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day5_multi_overlay",
        day: 4,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day6_county_overlay",
        day: 5,
        coverage: Coverage::County,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day6_region_overlay",
        day: 5,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day6_state_overlay",
        day: 5,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day6_multi_overlay",
        day: 5,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day7_county_overlay",
        day: 6,
        coverage: Coverage::County,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day7_region_overlay",
        day: 6,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day7_state_overlay",
        day: 6,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__day7_multi_overlay",
        day: 6,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_heat_risk__today_county",
        day: 0,
        coverage: Coverage::County,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__today_region",
        day: 0,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__today_state",
        day: 0,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__today_multi",
        day: 0,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__tomorrow_county",
        day: 1,
        coverage: Coverage::County,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__tomorrow_region",
        day: 1,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__tomorrow_state",
        day: 1,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__tomorrow_multi",
        day: 1,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day3_county",
        day: 2,
        coverage: Coverage::County,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day3_region",
        day: 2,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day3_state",
        day: 2,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day3_multi",
        day: 2,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day4_county",
        day: 3,
        coverage: Coverage::County,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day4_region",
        day: 3,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day4_state",
        day: 3,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day4_multi",
        day: 3,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day5_county",
        day: 4,
        coverage: Coverage::County,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day5_region",
        day: 4,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day5_state",
        day: 4,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day5_multi",
        day: 4,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day6_county",
        day: 5,
        coverage: Coverage::County,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day6_region",
        day: 5,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day6_state",
        day: 5,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day6_multi",
        day: 5,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day7_county",
        day: 6,
        coverage: Coverage::County,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day7_region",
        day: 6,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day7_state",
        day: 6,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__day7_multi",
        day: 6,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_heat_risk__today_county_portrait",
        day: 0,
        coverage: Coverage::County,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__today_region_portrait",
        day: 0,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__today_state_portrait",
        day: 0,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__today_multi_portrait",
        day: 0,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__tomorrow_county_portrait",
        day: 1,
        coverage: Coverage::County,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__tomorrow_region_portrait",
        day: 1,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__tomorrow_state_portrait",
        day: 1,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__tomorrow_multi_portrait",
        day: 1,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day3_county_portrait",
        day: 2,
        coverage: Coverage::County,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day3_region_portrait",
        day: 2,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day3_state_portrait",
        day: 2,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day3_multi_portrait",
        day: 2,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day4_county_portrait",
        day: 3,
        coverage: Coverage::County,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day4_region_portrait",
        day: 3,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day4_state_portrait",
        day: 3,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day4_multi_portrait",
        day: 3,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day5_county_portrait",
        day: 4,
        coverage: Coverage::County,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day5_region_portrait",
        day: 4,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day5_state_portrait",
        day: 4,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day5_multi_portrait",
        day: 4,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day6_county_portrait",
        day: 5,
        coverage: Coverage::County,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day6_region_portrait",
        day: 5,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day6_state_portrait",
        day: 5,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day6_multi_portrait",
        day: 5,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day7_county_portrait",
        day: 6,
        coverage: Coverage::County,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day7_region_portrait",
        day: 6,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day7_state_portrait",
        day: 6,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__day7_multi_portrait",
        day: 6,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_heat_risk__today_county_square",
        day: 0,
        coverage: Coverage::County,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__today_region_square",
        day: 0,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__today_state_square",
        day: 0,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__today_multi_square",
        day: 0,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__tomorrow_county_square",
        day: 1,
        coverage: Coverage::County,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__tomorrow_region_square",
        day: 1,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__tomorrow_state_square",
        day: 1,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__tomorrow_multi_square",
        day: 1,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day3_county_square",
        day: 2,
        coverage: Coverage::County,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day3_region_square",
        day: 2,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day3_state_square",
        day: 2,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day3_multi_square",
        day: 2,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day4_county_square",
        day: 3,
        coverage: Coverage::County,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day4_region_square",
        day: 3,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day4_state_square",
        day: 3,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day4_multi_square",
        day: 3,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day5_county_square",
        day: 4,
        coverage: Coverage::County,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day5_region_square",
        day: 4,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day5_state_square",
        day: 4,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day5_multi_square",
        day: 4,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day6_county_square",
        day: 5,
        coverage: Coverage::County,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day6_region_square",
        day: 5,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day6_state_square",
        day: 5,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day6_multi_square",
        day: 5,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day7_county_square",
        day: 6,
        coverage: Coverage::County,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day7_region_square",
        day: 6,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day7_state_square",
        day: 6,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_heat_risk__day7_multi_square",
        day: 6,
        coverage: Coverage::Multi,
        format: 2,
    },
];
#[derive(Clone, Debug)]
struct Site {
    id: String,
    lon: f64,
    lat: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Raster {
    id: i64,
    issued: i64,
    ingested: i64,
    valid: NaiveDate,
}
struct Day {
    raster: Raster,
    values: BTreeMap<String, usize>,
}
struct Snapshot {
    home: [u64; 2],
    checked: i64,
    base: NaiveDate,
    days: BTreeMap<NaiveDate, Day>,
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
fn epoch(v: &Value) -> Option<i64> {
    let ms = v.as_i64()?;
    (ms > 0).then_some(ms / 1000)
}
fn catalog(v: &Value, today: NaiveDate, now: i64) -> Option<BTreeMap<NaiveDate, Raster>> {
    if v.get("exceededTransferLimit").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let a = v.get("features")?.as_array()?;
    if a.len() > 1000 {
        return None;
    }
    let mut days: BTreeMap<NaiveDate, Raster> = BTreeMap::new();
    let mut seen = BTreeMap::new();
    for f in a {
        let p = f.get("attributes")?;
        if p.get("category")?.as_i64()? != 1 {
            continue;
        }
        let id = p.get("objectid")?.as_i64()?;
        if !(1..=i32::MAX as i64).contains(&id) {
            return None;
        }
        let issued = epoch(p.get("idp_filedate")?)?;
        let ingested = epoch(p.get("idp_ingestdate")?)?;
        let valid =
            chrono::DateTime::from_timestamp(epoch(p.get("idp_validtime")?)?, 0)?.date_naive();
        let r = Raster {
            id,
            issued,
            ingested,
            valid,
        };
        if let Some(old) = seen.insert(id, r) {
            if old != r {
                return None;
            }
        }
        if !(0..=36 * 3600).contains(&(now - issued))
            || ingested > now
            || ingested < issued
            || !(0..=6).contains(&(valid - today).num_days())
        {
            continue;
        }
        match days.get(&valid) {
            Some(old) if (old.issued, old.ingested) > (issued, ingested) => {}
            Some(old) if (old.issued, old.ingested) == (issued, ingested) && old.id != id => {
                return None
            }
            _ => {
                days.insert(valid, r);
            }
        }
    }
    Some(days)
}
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
fn samples_url(r: Raster, sites: &[Site]) -> String {
    let geometry = serde_json::json!({"points":sites.iter().map(|s|[s.lon,s.lat]).collect::<Vec<_>>(),"spatialReference":{"wkid":4326}});
    let rule = serde_json::json!({"mosaicMethod":"esriMosaicLockRaster","lockRasterIds":[r.id],"mosaicOperation":"MT_FIRST"});
    format!("{BASE}/getSamples?geometry={}&geometryType=esriGeometryMultipoint&mosaicRule={}&returnFirstValueOnly=true&interpolation=RSP_NearestNeighbor&outFields=objectid,idp_filedate,idp_ingestdate,idp_validtime&f=json",encode(&geometry.to_string()),encode(&rule.to_string()))
}
fn samples(v: &Value, r: Raster, sites: &[Site]) -> Option<BTreeMap<String, usize>> {
    let a = v.get("samples")?.as_array()?;
    if a.len() > sites.len() {
        return None;
    }
    let mut seen = BTreeSet::new();
    let mut origin = None;
    let mut values = BTreeMap::new();
    for p in a {
        let i = p.get("locationId")?.as_u64()? as usize;
        if !seen.insert(i) || p.get("rasterId")?.as_i64()? != r.id {
            return None;
        }
        // Catalog IDs can be reused when a service refreshes during a batch.
        // Require returned issue/valid metadata as well as the locked raster ID.
        let attrs = p.get("attributes")?;
        if attrs.get("objectid")?.as_i64()? != r.id
            || epoch(attrs.get("idp_filedate")?)? != r.issued
            || epoch(attrs.get("idp_ingestdate")?)? != r.ingested
            || chrono::DateTime::from_timestamp(epoch(attrs.get("idp_validtime")?)?, 0)?
                .date_naive()
                != r.valid
        {
            return None;
        }
        let loc = p.get("location")?;
        let mut x = loc.get("x")?.as_f64()?;
        let mut y = loc.get("y")?.as_f64()?;
        let sr = loc
            .get("spatialReference")?
            .get("latestWkid")
            .or_else(|| loc.get("spatialReference")?.get("wkid"))?
            .as_i64()?;
        if sr == 3857 || sr == 102100 {
            x = (x / 6378137.).to_degrees();
            y = (2. * (y / 6378137.).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees();
        } else if sr != 4326 {
            return None;
        }
        if !x.is_finite() || !y.is_finite() || i > sites.len() {
            return None;
        }
        // ArcGIS examples number locations from one; some services use zero.
        // Resolve the base from the returned coordinates, never silently shift values.
        let candidates: Vec<_> = [Some((0usize, i)), i.checked_sub(1).map(|j| (1usize, j))]
            .into_iter()
            .flatten()
            .filter(|(base, _)| origin.is_none_or(|o| o == *base))
            .filter(|(_, j)| {
                sites
                    .get(*j)
                    .is_some_and(|s| (x - s.lon).abs() <= 0.0001 && (y - s.lat).abs() <= 0.0001)
            })
            .collect();
        if candidates.len() != 1 {
            return None;
        }
        let (base, j) = candidates[0];
        origin = Some(base);
        let s = &sites[j];
        let value = p.get("value")?;
        if value.is_null()
            || value
                .as_str()
                .is_some_and(|s| s.trim().eq_ignore_ascii_case("nodata"))
        {
            continue;
        }
        let x = if let Some(s) = value.as_str() {
            s.trim().parse::<f64>().ok()?
        } else {
            value.as_f64()?
        };
        if !x.is_finite() || !(0.0..=4.0).contains(&x) || x.fract() != 0. {
            return None;
        }
        values.insert(s.id.clone(), x as usize);
    }
    Some(values)
}
fn sites(settings: &Settings) -> Option<Vec<Site>> {
    let all = crate::map::boundaries::counties_named()?;
    let mut points = BTreeMap::new();
    for coverage in [
        Coverage::County,
        Coverage::Region,
        Coverage::State,
        Coverage::Multi,
    ] {
        let scope = observed::report_scope(settings, coverage);
        for c in board::selected(&all, &scope) {
            let (lon, lat) = board::centre(c);
            if !lon.is_finite() || !lat.is_finite() {
                return None;
            }
            if !board::inside(c, lon, lat) {
                continue;
            }
            points.insert(
                c.fips.clone(),
                Site {
                    id: c.fips.clone(),
                    lon,
                    lat,
                },
            );
        }
    }
    if points.is_empty() || points.len() > 2048 {
        return None;
    }
    Some(points.into_values().collect())
}
async fn fetch(h: GeoView) -> Option<Snapshot> {
    let started = chrono::Utc::now().timestamp();
    let today = super::social_forecast::home_date(h, started)?;
    let mut settings = Settings::default();
    settings.home = h;
    let sites = sites(&settings)?;
    let records=catalog(&super::report_feed::get_json(&format!("{BASE}/query?where=category%3D1&outFields=objectid,category,idp_filedate,idp_ingestdate,idp_validtime&returnGeometry=false&f=json")).await?,today,started)?;
    let mut days: BTreeMap<_, _> = records
        .values()
        .map(|r| {
            (
                r.valid,
                Day {
                    raster: *r,
                    values: BTreeMap::new(),
                },
            )
        })
        .collect();
    let mut failed = BTreeSet::new();
    let mut jobs = Vec::new();
    for r in records.into_values() {
        for chunk in sites.chunks(64) {
            jobs.push((r, chunk.to_vec()));
        }
    }
    // At most four bounded requests in flight. A failed chunk withholds its whole day.
    for group in jobs.chunks(4) {
        let mut tasks = tokio::task::JoinSet::new();
        for (r, sites) in group.iter().cloned() {
            tasks.spawn(async move {
                let v = super::report_feed::get_json(&samples_url(r, &sites))
                    .await
                    .and_then(|v| samples(&v, r, &sites));
                (r.valid, v)
            });
        }
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok((day, Some(values))) => {
                    days.get_mut(&day)?.values.extend(values);
                }
                Ok((day, None)) => {
                    failed.insert(day);
                }
                Err(_) => return None,
            }
        }
    }
    days.retain(|day, v| !failed.contains(day) && !v.values.is_empty());
    let checked = chrono::Utc::now().timestamp();
    if super::social_forecast::home_date(h, checked) != Some(today) {
        return None;
    }
    Some(Snapshot {
        home: home_key(h),
        checked,
        base: today,
        days,
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
    let result = tokio::time::timeout(std::time::Duration::from_secs(120), fetch(h))
        .await
        .ok()
        .flatten();
    if let Ok(mut s) = store().lock() {
        if s.generation == ticket {
            s.current = result.map(Arc::new);
        }
    }
}
fn prefix(s: Spec) -> String {
    format!(
        "heatrisk.{}.{}",
        s.day,
        match s.coverage {
            Coverage::County => "county",
            Coverage::Region => "region",
            Coverage::State => "state",
            Coverage::Multi => "multi",
        }
    )
}
fn county_name(c: &crate::map::boundaries::County) -> String {
    let state = crate::winter::STATES
        .iter()
        .find(|s| s.0 == c.state)
        .map(|s| s.1)
        .unwrap_or("US");
    format!("{}, {state}", c.name)
}
fn scene(settings: &Settings, s: Spec, day: Option<&Day>) -> Scene {
    let mut scene = board::empty(Layout::Single);
    scene.scope = observed::report_scope(settings, s.coverage);
    scene.legend_title =
        "Experimental HeatRisk • County-centre samples • Blank = unavailable".into();
    scene.legend = CATEGORIES
        .iter()
        .map(|(label, color)| Band {
            label: (*label).into(),
            color: (*color).into(),
        })
        .collect();
    let p = &mut scene.panels[0];
    p.title = "Experimental NWS HeatRisk".into();
    p.detail = "24-hour heat-impact index • Local risk varies within counties".into();
    let Some(day) = day else {
        return scene;
    };
    p.valid = format!("Valid {}", day.raster.valid.format("%b %d, %Y"));
    let Some(all) = crate::map::boundaries::counties_named() else {
        return scene;
    };
    let counties = board::selected(&all, &scene.scope);
    let values: BTreeMap<_, _> = counties
        .iter()
        .filter_map(|c| day.values.get(&c.fips).map(|i| (c.fips.clone(), *i)))
        .collect();
    if values.is_empty() {
        return scene;
    }
    let mut ranked: Vec<_> = counties
        .iter()
        .filter_map(|c| values.get(&c.fips).map(|i| (*c, *i)))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));
    p.summary_title = "Highest sampled county centres • Up to 7 shown".into();
    p.summary = Some(
        ranked
            .iter()
            .take(7)
            .map(|(c, i)| (county_name(c), CATEGORIES[*i].0.into()))
            .collect(),
    );
    p.points = Some(
        ranked
            .iter()
            .take(7)
            .map(|(c, i)| {
                let (lon, lat) = board::centre(c);
                ReportPoint {
                    id: c.fips.clone(),
                    lon,
                    lat,
                    label: format!("{} • {i}", county_name(c)),
                    category: *i,
                    value: Some(*i as f32),
                }
            })
            .collect(),
    );
    p.counties = Some(values);
    scene.reviewed = true;
    scene
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let today = super::social_forecast::home_date(settings.home, now);
    let snapshot = store().lock().ok().and_then(|s| {
        s.current
            .as_ref()
            .filter(|s| {
                s.home == home_key(settings.home)
                    && today == Some(s.base)
                    && (0..=1200).contains(&(now - s.checked))
            })
            .cloned()
    });
    for s in SPECS.iter().filter(|s| s.format == 0) {
        let day = today
            .and_then(|d| d.checked_add_signed(chrono::Duration::days(s.day as i64)))
            .and_then(|d| snapshot.as_ref()?.days.get(&d))
            .filter(|d| (0..=36 * 3600).contains(&(now - d.raster.issued)));
        let scene = scene(settings, *s, day);
        let p = prefix(*s);
        b.set(
            &format!("set.{p}.payload"),
            &serde_json::to_string(&scene).unwrap_or_default(),
        );
        b.set(
            &format!("set.{p}.subtitle"),
            &day.map(|d| {
                format!(
                    "Issued {} UTC",
                    chrono::DateTime::from_timestamp(d.raster.issued, 0)
                        .unwrap()
                        .format("%b %d %H:%M")
                )
            })
            .unwrap_or_else(|| "Published daily forecast".into()),
        );
        b.set(
            &format!("set.{p}.source"),
            "Source: NOAA / NWS WPC • Experimental HeatRisk",
        );
        b.set(
            &format!("set.{p}.note"),
            "Published categories sampled at county centres • Not an official heat alert",
        );
    }
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|s| s.key == key)?;
    let p = prefix(s);
    Some(match s.format {
        3 => super::product_overlay::template("Experimental NWS HeatRisk", "Heat", &p),
        1 => super::comparison::portrait_template("NWS HeatRisk", "Heat", &p),
        2 => super::comparison::square_template("NWS HeatRisk", "Heat", &p),
        _ => super::comparison::template("NWS HeatRisk", "Heat", &p, Layout::Single),
    })
}
#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(s) = SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
    else {
        return;
    };
    let settings = Settings::default();
    let mut d = Day {
        raster: Raster {
            id: 1,
            issued: chrono::Utc::now().timestamp(),
            ingested: chrono::Utc::now().timestamp(),
            valid: chrono::Local::now().date_naive() + chrono::Duration::days(s.day as i64),
        },
        values: BTreeMap::new(),
    };
    if let Some(all) = crate::map::boundaries::counties_named() {
        for (i, c) in board::selected(&all, &observed::report_scope(&settings, s.coverage))
            .iter()
            .enumerate()
        {
            let (lon, lat) = board::centre(c);
            if board::inside(c, lon, lat) {
                d.values.insert(c.fips.clone(), (i + s.day as usize) % 5);
            }
        }
    }
    let mut scene = scene(&settings, *s, Some(&d));
    scene.panels[0].valid = format!("SAMPLE • {}", d.raster.valid.format("%b %d, %Y"));
    let p = prefix(*s);
    for (k, v) in [
        ("payload", serde_json::to_string(&scene).unwrap()),
        ("source", "SAMPLE ONLY • Not for broadcast".into()),
        ("subtitle", "SAMPLE • Published categories".into()),
        (
            "note",
            "SAMPLE • Synthetic county-centre categories • Not an official heat alert".into(),
        ),
    ] {
        doc.data_overrides.push((format!("set.{p}.{k}"), v));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn annotate(v: &mut Value, r: Raster) {
        for p in v["samples"].as_array_mut().unwrap() {
            p["attributes"] = serde_json::json!({"objectid":r.id,"idp_filedate":r.issued*1000,"idp_ingestdate":r.ingested*1000,"idp_validtime":r.valid.and_hms_opt(12,0,0).unwrap().and_utc().timestamp_millis()});
        }
    }
    fn raster() -> Raster {
        Raster {
            id: 7,
            issued: 1794571200,
            ingested: 1794571200,
            valid: NaiveDate::from_ymd_opt(2026, 11, 13).unwrap(),
        }
    }
    #[test]
    fn exact_categories_require_matching_raster_and_location() {
        let r = raster();
        let sites = vec![Site {
            id: "18037".into(),
            lon: -86.9,
            lat: 38.4,
        }];
        let mut v = json!({"samples":[{"locationId":1,"rasterId":7,"location":{"x":-86.9,"y":38.4,"spatialReference":{"wkid":4326}},"value":"0"}]});
        annotate(&mut v, r);
        assert_eq!(samples(&v, r, &sites).unwrap()["18037"], 0);
        v["samples"][0]["value"] = json!("NoData");
        assert!(samples(&v, r, &sites).unwrap().is_empty());
        v["samples"][0]["value"] = json!("2.5");
        assert!(samples(&v, r, &sites).is_none());
        v["samples"][0]["value"] = json!(2);
        v["samples"][0]["rasterId"] = json!(8);
        assert!(samples(&v, r, &sites).is_none());
    }
    #[test]
    fn catalog_never_substitutes_another_valid_day_or_conflicting_issue() {
        let r = raster();
        let f = json!({"attributes":{"objectid":7,"category":1,"idp_filedate":r.issued*1000,"idp_ingestdate":r.ingested*1000,"idp_validtime":1794571200000i64}});
        let v = json!({"features":[f.clone()]});
        let d = catalog(&v, r.valid, r.issued + 60).unwrap();
        assert_eq!(d.len(), 1);
        assert!(d.get(&(r.valid + chrono::Duration::days(1))).is_none());
        assert!(catalog(&v, r.valid, r.issued + 36 * 3600 + 1)
            .unwrap()
            .is_empty());
        let mut other = f.clone();
        other["attributes"]["objectid"] = json!(8);
        assert!(catalog(&json!({"features":[f,other]}), r.valid, r.issued + 60).is_none());
    }
    #[test]
    fn sample_origin_coordinates_and_locked_requests_cannot_shift_counties() {
        let r = raster();
        let sites = vec![
            Site {
                id: "first".into(),
                lon: -86.9,
                lat: 38.4,
            },
            Site {
                id: "second".into(),
                lon: -86.8,
                lat: 38.5,
            },
        ];
        let mut v = serde_json::json!({"samples":[{"locationId":0,"rasterId":7,"location":{"x":-86.9,"y":38.4,"spatialReference":{"wkid":4326}},"value":"3"},{"locationId":1,"rasterId":7,"location":{"x":-86.8,"y":38.5,"spatialReference":{"wkid":4326}},"value":"1"}]});
        annotate(&mut v, r);
        let parsed = samples(&v, r, &sites).unwrap();
        assert_eq!(parsed["first"], 3);
        assert_eq!(parsed["second"], 1);
        v["samples"][0]["locationId"] = serde_json::json!(1);
        v["samples"][1]["locationId"] = serde_json::json!(2);
        assert_eq!(samples(&v, r, &sites).unwrap(), parsed);
        v["samples"][0]["attributes"]["idp_validtime"] = serde_json::json!(1794657600000i64);
        assert!(samples(&v, r, &sites).is_none());
        annotate(&mut v, r);

        v["samples"][1]["location"]["x"] = serde_json::json!(-80.);
        assert!(samples(&v, r, &sites).is_none());
        let url = samples_url(r, &sites);
        assert!(url.starts_with(BASE));
        assert!(url.contains("RSP_NearestNeighbor"));
        assert!(url.contains("lockRasterIds"));
        assert_eq!(
            epoch(&serde_json::json!(1794571200123i64)),
            Some(1794571200)
        );
    }
}
