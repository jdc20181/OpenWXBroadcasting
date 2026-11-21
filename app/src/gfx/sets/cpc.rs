//! CPC period-category outlooks sampled at county centres; probabilities are not amounts.
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
/// `Spec::first` for the CPC monthly outlook (next calendar month).
const MONTH: i64 = 30;
/// `Spec::first` for the CPC 3-month outlook, lead 1 (the next three months, e.g. DJF).
const SEASON: i64 = 90;
fn long_range(first: i64) -> bool {
    first >= MONTH
}
/// Days an issued product stays current: 6–10/8–14 day outlooks are daily, the monthly and
/// 3-month outlooks are issued once a month (third Thursday).
fn max_age_days(first: i64) -> i64 {
    if long_range(first) { 45 } else { 2 }
}
fn period_label(first: i64) -> &'static str {
    match first {
        6 => "6–10 day",
        8 => "8–14 day",
        MONTH => "monthly",
        _ => "3-month",
    }
}
/// Valid period from the long-range services' text: "DJF 2026-2027" (3-month) or "Dec 2026".
fn valid_period(text: &str) -> Option<(NaiveDate, NaiveDate)> {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const SEASONS: [&str; 12] = ["JFM", "FMA", "MAM", "AMJ", "MJJ", "JJA", "JAS", "ASO", "SON", "OND", "NDJ", "DJF"];
    let (name, years) = text.trim().split_once(' ')?;
    let year: i32 = years.split('-').next()?.trim().parse().ok()?;
    let (month, length) = if let Some(i) = MONTHS.iter().position(|m| *m == name) {
        (i as u32 + 1, 1)
    } else {
        (SEASONS.iter().position(|s| *s == name)? as u32 + 1, 3)
    };
    let start = NaiveDate::from_ymd_opt(year, month, 1)?;
    let end = start.checked_add_months(chrono::Months::new(length))?.pred_opt()?;
    Some((start, end))
}
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    precip: bool,
    first: i64,
    coverage: Coverage,
    format: u8,
}
const SPECS: &[Spec] = &[
    Spec {
        key: "set_cpc_temperature__6day_region_overlay",
        precip: false,
        first: 6,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_cpc_temperature__6day_state_overlay",
        precip: false,
        first: 6,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_cpc_temperature__6day_multi_overlay",
        precip: false,
        first: 6,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_cpc_temperature__8day_region_overlay",
        precip: false,
        first: 8,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_cpc_temperature__8day_state_overlay",
        precip: false,
        first: 8,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_cpc_temperature__8day_multi_overlay",
        precip: false,
        first: 8,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_cpc_precipitation__6day_region_overlay",
        precip: true,
        first: 6,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_cpc_precipitation__6day_state_overlay",
        precip: true,
        first: 6,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_cpc_precipitation__6day_multi_overlay",
        precip: true,
        first: 6,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_cpc_precipitation__8day_region_overlay",
        precip: true,
        first: 8,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_cpc_precipitation__8day_state_overlay",
        precip: true,
        first: 8,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_cpc_precipitation__8day_multi_overlay",
        precip: true,
        first: 8,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_cpc_temperature__6day_region",
        precip: false,
        first: 6,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_cpc_temperature__6day_state",
        precip: false,
        first: 6,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_cpc_temperature__6day_multi",
        precip: false,
        first: 6,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_cpc_temperature__6day_region_portrait",
        precip: false,
        first: 6,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_cpc_temperature__6day_state_portrait",
        precip: false,
        first: 6,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_cpc_temperature__6day_multi_portrait",
        precip: false,
        first: 6,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_cpc_temperature__6day_region_square",
        precip: false,
        first: 6,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_cpc_temperature__6day_state_square",
        precip: false,
        first: 6,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_cpc_temperature__6day_multi_square",
        precip: false,
        first: 6,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_cpc_temperature__8day_region",
        precip: false,
        first: 8,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_cpc_temperature__8day_state",
        precip: false,
        first: 8,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_cpc_temperature__8day_multi",
        precip: false,
        first: 8,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_cpc_temperature__8day_region_portrait",
        precip: false,
        first: 8,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_cpc_temperature__8day_state_portrait",
        precip: false,
        first: 8,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_cpc_temperature__8day_multi_portrait",
        precip: false,
        first: 8,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_cpc_temperature__8day_region_square",
        precip: false,
        first: 8,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_cpc_temperature__8day_state_square",
        precip: false,
        first: 8,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_cpc_temperature__8day_multi_square",
        precip: false,
        first: 8,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_cpc_precipitation__6day_region",
        precip: true,
        first: 6,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_cpc_precipitation__6day_state",
        precip: true,
        first: 6,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_cpc_precipitation__6day_multi",
        precip: true,
        first: 6,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_cpc_precipitation__6day_region_portrait",
        precip: true,
        first: 6,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_cpc_precipitation__6day_state_portrait",
        precip: true,
        first: 6,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_cpc_precipitation__6day_multi_portrait",
        precip: true,
        first: 6,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_cpc_precipitation__6day_region_square",
        precip: true,
        first: 6,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_cpc_precipitation__6day_state_square",
        precip: true,
        first: 6,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_cpc_precipitation__6day_multi_square",
        precip: true,
        first: 6,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_cpc_precipitation__8day_region",
        precip: true,
        first: 8,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_cpc_precipitation__8day_state",
        precip: true,
        first: 8,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_cpc_precipitation__8day_multi",
        precip: true,
        first: 8,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_cpc_precipitation__8day_region_portrait",
        precip: true,
        first: 8,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_cpc_precipitation__8day_state_portrait",
        precip: true,
        first: 8,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_cpc_precipitation__8day_multi_portrait",
        precip: true,
        first: 8,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_cpc_precipitation__8day_region_square",
        precip: true,
        first: 8,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_cpc_precipitation__8day_state_square",
        precip: true,
        first: 8,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_cpc_precipitation__8day_multi_square",
        precip: true,
        first: 8,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_cpc_temperature__month_region",
        precip: false,
        first: MONTH,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_cpc_temperature__month_region_portrait",
        precip: false,
        first: MONTH,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_cpc_temperature__month_region_square",
        precip: false,
        first: MONTH,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_cpc_temperature__month_region_overlay",
        precip: false,
        first: MONTH,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_cpc_temperature__month_state",
        precip: false,
        first: MONTH,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_cpc_temperature__month_state_portrait",
        precip: false,
        first: MONTH,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_cpc_temperature__month_state_square",
        precip: false,
        first: MONTH,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_cpc_temperature__month_state_overlay",
        precip: false,
        first: MONTH,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_cpc_temperature__month_multi",
        precip: false,
        first: MONTH,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_cpc_temperature__month_multi_portrait",
        precip: false,
        first: MONTH,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_cpc_temperature__month_multi_square",
        precip: false,
        first: MONTH,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_cpc_temperature__month_multi_overlay",
        precip: false,
        first: MONTH,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_cpc_temperature__season_region",
        precip: false,
        first: SEASON,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_cpc_temperature__season_region_portrait",
        precip: false,
        first: SEASON,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_cpc_temperature__season_region_square",
        precip: false,
        first: SEASON,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_cpc_temperature__season_region_overlay",
        precip: false,
        first: SEASON,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_cpc_temperature__season_state",
        precip: false,
        first: SEASON,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_cpc_temperature__season_state_portrait",
        precip: false,
        first: SEASON,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_cpc_temperature__season_state_square",
        precip: false,
        first: SEASON,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_cpc_temperature__season_state_overlay",
        precip: false,
        first: SEASON,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_cpc_temperature__season_multi",
        precip: false,
        first: SEASON,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_cpc_temperature__season_multi_portrait",
        precip: false,
        first: SEASON,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_cpc_temperature__season_multi_square",
        precip: false,
        first: SEASON,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_cpc_temperature__season_multi_overlay",
        precip: false,
        first: SEASON,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_cpc_precipitation__month_region",
        precip: true,
        first: MONTH,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_cpc_precipitation__month_region_portrait",
        precip: true,
        first: MONTH,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_cpc_precipitation__month_region_square",
        precip: true,
        first: MONTH,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_cpc_precipitation__month_region_overlay",
        precip: true,
        first: MONTH,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_cpc_precipitation__month_state",
        precip: true,
        first: MONTH,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_cpc_precipitation__month_state_portrait",
        precip: true,
        first: MONTH,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_cpc_precipitation__month_state_square",
        precip: true,
        first: MONTH,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_cpc_precipitation__month_state_overlay",
        precip: true,
        first: MONTH,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_cpc_precipitation__month_multi",
        precip: true,
        first: MONTH,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_cpc_precipitation__month_multi_portrait",
        precip: true,
        first: MONTH,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_cpc_precipitation__month_multi_square",
        precip: true,
        first: MONTH,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_cpc_precipitation__month_multi_overlay",
        precip: true,
        first: MONTH,
        coverage: Coverage::Multi,
        format: 3,
    },
    Spec {
        key: "set_cpc_precipitation__season_region",
        precip: true,
        first: SEASON,
        coverage: Coverage::Region,
        format: 0,
    },
    Spec {
        key: "set_cpc_precipitation__season_region_portrait",
        precip: true,
        first: SEASON,
        coverage: Coverage::Region,
        format: 1,
    },
    Spec {
        key: "set_cpc_precipitation__season_region_square",
        precip: true,
        first: SEASON,
        coverage: Coverage::Region,
        format: 2,
    },
    Spec {
        key: "set_cpc_precipitation__season_region_overlay",
        precip: true,
        first: SEASON,
        coverage: Coverage::Region,
        format: 3,
    },
    Spec {
        key: "set_cpc_precipitation__season_state",
        precip: true,
        first: SEASON,
        coverage: Coverage::State,
        format: 0,
    },
    Spec {
        key: "set_cpc_precipitation__season_state_portrait",
        precip: true,
        first: SEASON,
        coverage: Coverage::State,
        format: 1,
    },
    Spec {
        key: "set_cpc_precipitation__season_state_square",
        precip: true,
        first: SEASON,
        coverage: Coverage::State,
        format: 2,
    },
    Spec {
        key: "set_cpc_precipitation__season_state_overlay",
        precip: true,
        first: SEASON,
        coverage: Coverage::State,
        format: 3,
    },
    Spec {
        key: "set_cpc_precipitation__season_multi",
        precip: true,
        first: SEASON,
        coverage: Coverage::Multi,
        format: 0,
    },
    Spec {
        key: "set_cpc_precipitation__season_multi_portrait",
        precip: true,
        first: SEASON,
        coverage: Coverage::Multi,
        format: 1,
    },
    Spec {
        key: "set_cpc_precipitation__season_multi_square",
        precip: true,
        first: SEASON,
        coverage: Coverage::Multi,
        format: 2,
    },
    Spec {
        key: "set_cpc_precipitation__season_multi_overlay",
        precip: true,
        first: SEASON,
        coverage: Coverage::Multi,
        format: 3,
    },
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Category {
    kind: usize,
    probability: u8,
}
impl Category {
    fn label(self) -> String {
        if self.kind == 1 {
            "Near normal".into()
        } else if self.kind == 3 {
            "Equal chances".into()
        } else {
            format!(
                "{} {}%",
                ["Below", "Near", "Above"][self.kind],
                self.probability
            )
        }
    }
}
struct Area {
    category: Category,
    rings: Vec<Vec<(f64, f64)>>,
}
struct Product {
    issued: NaiveDate,
    start: NaiveDate,
    end: NaiveDate,
    areas: Vec<Area>,
    values: BTreeMap<String, Category>,
}
struct Snapshot {
    home: [u64; 2],
    checked: i64,
    products: BTreeMap<(bool, i64), Product>,
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
fn date(v: &Value) -> Option<NaiveDate> {
    let ms = v.as_i64()?;
    let d = chrono::DateTime::from_timestamp_millis(ms)?;
    (ms > 0).then(|| d.date_naive())
}
fn category(p: &Value, long: bool) -> Option<Category> {
    let n = p.get("prob")?.as_f64()?;
    if !n.is_finite() || !(0.0..=100.0).contains(&n) || n.fract() != 0. {
        return None;
    }
    let kind = match p.get("cat")?.as_str()? {
        "Below" => 0,
        "Normal" => 1,
        "Above" => 2,
        "EC" => 3,
        _ => return None,
    };
    let n = n as u8;
    if long {
        // Monthly and 3-month outlooks publish 33-90 % categories, near normal included.
        return ((kind == 3 && n == 33) || (kind != 3 && (33..=90).contains(&n))).then_some(Category { kind, probability: n });
    }
    if (kind == 3 && n != 33)
        || (kind == 1 && n != 36)
        || ((kind == 0 || kind == 2) && ![33, 40, 50, 60, 70, 80, 90].contains(&n))
    {
        return None;
    }
    Some(Category {
        kind,
        probability: n,
    })
}
fn parse(v: &Value, first: i64, now: i64) -> Option<Product> {
    if v.get("exceededTransferLimit").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    if v.get("spatialReference")?.get("wkid")?.as_i64()? != 4326 {
        return None;
    }
    let features = v.get("features")?.as_array()?;
    if features.is_empty() || features.len() > 500 {
        return None;
    }
    let today = chrono::DateTime::from_timestamp(now, 0)?.date_naive();
    let mut dates = None;
    let mut areas = Vec::new();
    let mut ids = BTreeSet::new();
    let mut vertices = 0;
    for f in features {
        let p = f.get("attributes")?;
        let id = p.get("objectid")?.as_i64()?;
        if id < 1 || !ids.insert(id) {
            return None;
        }
        let issue = date(p.get("fcst_date")?)?;
        let (start, end) = if long_range(first) {
            valid_period(p.get("valid_seas")?.as_str()?)?
        } else {
            (date(p.get("start_date")?)?, date(p.get("end_date")?)?)
        };
        let fits = if long_range(first) {
            start > issue && (start - issue).num_days() <= 75
        } else {
            (start - issue).num_days() == first && (end - start).num_days() == if first == 6 { 4 } else { 6 }
        };
        if !(0..=max_age_days(first)).contains(&(today - issue).num_days()) || !fits {
            return None;
        }
        let d = (issue, start, end);
        if dates.is_some_and(|old| old != d) {
            return None;
        };
        dates = Some(d);
        let category = category(p, long_range(first))?;
        let input = f.get("geometry")?.get("rings")?.as_array()?;
        // Monthly and 3-month shapes carry many island and coastline rings (958 seen).
        if input.is_empty() || input.len() > if long_range(first) { 4096 } else { 512 } {
            return None;
        }
        let mut rings = Vec::new();
        for r in input {
            let points = r.as_array()?;
            if points.len() < 4 || points.len() > 100000 {
                return None;
            }
            vertices += points.len();
            if vertices > 200000 {
                return None;
            }
            let mut ring = Vec::new();
            for x in points {
                let a = x.as_array()?;
                if a.len() != 2 {
                    return None;
                }
                let lon = a[0].as_f64()?;
                let lat = a[1].as_f64()?;
                if !lon.is_finite()
                    || !lat.is_finite()
                    || !(-180.0..=180.0).contains(&lon)
                    || !(-90.0..=90.0).contains(&lat)
                {
                    return None;
                }
                ring.push((lon, lat));
            }
            if ring.first() != ring.last() {
                return None;
            }
            let area: f64 = ring
                .windows(2)
                .map(|p| p[0].0 * p[1].1 - p[1].0 * p[0].1)
                .sum();
            if area.abs() < 1e-12 {
                return None;
            }
            rings.push(ring);
        }
        areas.push(Area { category, rings });
    }
    let (issued, start, end) = dates?;
    Some(Product {
        issued,
        start,
        end,
        areas,
        values: BTreeMap::new(),
    })
}
// ArcGIS rings use the even-odd rule: preserve interior holes and separate islands.
// A centre exactly on a category boundary is ambiguous and is withheld.
fn contains(a: &Area, x: f64, y: f64) -> Option<bool> {
    let mut inside = false;
    for r in &a.rings {
        for pair in r.windows(2) {
            let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
            let cross = (x - ax) * (by - ay) - (y - ay) * (bx - ax);
            if cross.abs() < 1e-10
                && x >= ax.min(bx) - 1e-10
                && x <= ax.max(bx) + 1e-10
                && y >= ay.min(by) - 1e-10
                && y <= ay.max(by) + 1e-10
            {
                return None;
            }
            if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
                inside = !inside
            }
        }
    }
    Some(inside)
}
fn at(product: &Product, x: f64, y: f64) -> Option<Category> {
    let mut found = None;
    for a in &product.areas {
        if contains(a, x, y)? {
            if found.is_some_and(|old| old != a.category) {
                return None;
            }
            found = Some(a.category)
        }
    }
    found
}
fn query_url(precip: bool, first: i64, bbox: [f64; 4]) -> String {
    let service = match (first, precip) {
        (6, _) => "cpc_6_10_day_outlk",
        (8, _) => "cpc_8_14_day_outlk",
        (MONTH, false) => "cpc_mthly_temp_outlk",
        (MONTH, true) => "cpc_mthly_precip_outlk",
        (_, false) => "cpc_sea_temp_outlk",
        (_, true) => "cpc_sea_precip_outlk",
    };
    // 6–10/8–14: layer 0 temperature, 1 precipitation. Monthly: one layer. 3-month: layer 0 is
    // lead 1 (the next three months).
    let layer = if precip && !long_range(first) { 1 } else { 0 };
    let dates = if long_range(first) { "valid_seas" } else { "start_date,end_date" };
    format!("https://mapservices.weather.noaa.gov/vector/rest/services/outlooks/{service}/MapServer/{layer}/query?where=1%3D1&outFields=objectid,fcst_date,{dates},cat,prob&geometry={},{},{},{}&geometryType=esriGeometryEnvelope&inSR=4326&spatialRel=esriSpatialRelIntersects&returnGeometry=true&outSR=4326&f=json",bbox[0],bbox[1],bbox[2],bbox[3])
}
async fn fetch(h: GeoView) -> Option<Snapshot> {
    let mut settings = Settings::default();
    settings.home = h;
    let all = crate::map::boundaries::counties_named()?;
    let mut bbox = [180f64, 90f64, -180f64, -90f64];
    let mut sites = BTreeMap::new();
    for scope in [Coverage::Region, Coverage::State, Coverage::Multi] {
        for c in board::selected(&all, &observed::report_scope(&settings, scope)) {
            let (x, y) = board::centre(c);
            if board::inside(c, x, y) {
                sites.insert(c.fips.clone(), (x, y));
            }
            for ring in &c.rings {
                for &(x, y) in ring {
                    bbox[0] = bbox[0].min(x);
                    bbox[1] = bbox[1].min(y);
                    bbox[2] = bbox[2].max(x);
                    bbox[3] = bbox[3].max(y);
                }
            }
        }
    }
    if bbox[0] >= bbox[2] || bbox[1] >= bbox[3] || bbox.iter().any(|x| !x.is_finite()) {
        return None;
    }
    let mut tasks = tokio::task::JoinSet::new();
    let sites = Arc::new(sites);
    for precip in [false, true] {
        for first in [6, 8, MONTH, SEASON] {
            let sites = Arc::clone(&sites);
            tasks.spawn(async move {
                let result = super::report_feed::get_json(&query_url(precip, first, bbox))
                    .await
                    .and_then(|v| parse(&v, first, chrono::Utc::now().timestamp()))
                    .map(|mut product| {
                        sample_sites(&mut product, &sites);
                        product.areas.clear();
                        product
                    });
                ((precip, first), result)
            });
        }
    }
    let mut products = BTreeMap::new();
    while let Some(result) = tasks.join_next().await {
        let (key, value) = result.ok()?;
        if let Some(v) = value {
            products.insert(key, v);
        }
    }
    Some(Snapshot {
        home: home_key(h),
        checked: chrono::Utc::now().timestamp(),
        products,
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
    let result = tokio::time::timeout(std::time::Duration::from_secs(50), fetch(h))
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
        "cpc.{}.{}.{}",
        if s.precip { "precip" } else { "temp" },
        s.first,
        match s.coverage {
            Coverage::County => "county",
            Coverage::Region => "region",
            Coverage::State => "state",
            Coverage::Multi => "multi",
        }
    )
}
fn sample_sites(product: &mut Product, sites: &BTreeMap<String, (f64, f64)>) {
    product.values = sites
        .iter()
        .filter_map(|(id, &(x, y))| at(product, x, y).map(|value| (id.clone(), value)))
        .collect();
}
fn scene(settings: &Settings, s: Spec, product: Option<&Product>) -> Scene {
    let mut scene = board::empty(Layout::Single);
    scene.scope = observed::report_scope(settings, s.coverage);
    scene.legend_title = "Favoured category • County-centre samples • Blank = unavailable".into();
    let colors = if s.precip {
        ["#B97532", "#A0A0A0", "#338B58", "#FFFFFF"]
    } else {
        ["#389FDC", "#A0A0A0", "#DA5731", "#FFFFFF"]
    };
    scene.legend = [
        "Below normal",
        "Near normal",
        "Above normal",
        "Equal chances",
    ]
    .iter()
    .zip(colors)
    .map(|(label, color)| Band {
        label: (*label).into(),
        color: color.into(),
    })
    .collect();
    let p = &mut scene.panels[0];
    p.title = format!(
        "CPC {} {} outlook",
        period_label(s.first),
        if s.precip {
            "precipitation"
        } else {
            "temperature"
        }
    );
    p.detail = if s.precip {
        "Period-total category probability • Not a rain chance or rainfall amount"
    } else {
        "Period-average category probability • Not a temperature departure"
    }
    .into();
    let Some(product) = product else { return scene };
    p.valid = format!(
        "{}–{}",
        product.start.format("%b %d, %Y"),
        product.end.format("%b %d, %Y")
    );
    let Some(all) = crate::map::boundaries::counties_named() else {
        return scene;
    };
    let mut rows = Vec::new();
    let mut values = BTreeMap::new();
    for c in board::selected(&all, &scene.scope) {
        if let Some(&value) = product.values.get(&c.fips) {
            values.insert(c.fips.clone(), value.kind);
            rows.push((c, value));
        }
    }
    if values.is_empty() {
        return scene;
    }
    rows.sort_by(|a, b| {
        b.1.probability
            .cmp(&a.1.probability)
            .then_with(|| a.0.name.cmp(&b.0.name))
    });
    let name = |c: &crate::map::boundaries::County| {
        format!(
            "{}, {}",
            c.name,
            crate::winter::STATES
                .iter()
                .find(|s| s.0 == c.state)
                .map(|s| s.1)
                .unwrap_or("US")
        )
    };
    p.summary_title = "Selected centres • Highest category probabilities".into();
    p.summary = Some(
        rows.iter()
            .take(7)
            .map(|(c, v)| (name(c), v.label()))
            .collect(),
    );
    p.points = Some(
        rows.iter()
            .take(7)
            .map(|(c, v)| {
                let (lon, lat) = board::centre(c);
                ReportPoint {
                    id: c.fips.clone(),
                    lon,
                    lat,
                    label: format!("{} • {}", name(c), v.label()),
                    category: v.kind,
                    value: None,
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
    let today = chrono::Utc::now().date_naive();
    let snapshot = store().lock().ok().and_then(|s| {
        s.current
            .as_ref()
            .filter(|s| {
                s.home == home_key(settings.home) && (0..=1200).contains(&(now - s.checked))
            })
            .cloned()
    });
    for s in SPECS.iter().filter(|s| s.format == 0) {
        let product = snapshot
            .as_ref()
            .and_then(|v| v.products.get(&(s.precip, s.first)))
            .filter(|p| (0..=max_age_days(s.first)).contains(&(today - p.issued).num_days()));
        let p = prefix(*s);
        let scene = scene(settings, *s, product);
        for (key,value) in [("payload",serde_json::to_string(&scene).unwrap_or_default()),("subtitle",product.map(|p|format!("CPC issued {}",p.issued.format("%b %d, %Y"))).unwrap_or_else(||"Published period outlook".into())),("source","Source: NOAA / NWS Climate Prediction Center".into()),("note","County-centre samples • Equal chances gives no favoured category • Local conditions vary".into())] {b.set(&format!("set.{p}.{key}"),&value);}
    }
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|s| s.key == key)?;
    let p = prefix(s);
    let title = if s.precip {
        "CPC precipitation"
    } else {
        "CPC temperature"
    };
    Some(match s.format {
        3 => super::product_overlay::template(title, "Climate", &p),
        1 => super::comparison::portrait_template(title, "Climate", &p),
        2 => super::comparison::square_template(title, "Climate", &p),
        _ => super::comparison::template(title, "Climate", &p, Layout::Single),
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
    let now = chrono::Utc::now().date_naive();
    let settings = Settings::default();
    let mut product = Product {
        issued: now,
        start: now + chrono::Duration::days(s.first),
        end: now + chrono::Duration::days(match s.first { 6 => 10, 8 => 14, MONTH => 60, _ => 180 }),
        areas: Vec::new(),
        values: BTreeMap::new(),
    };
    // Synthetic non-overlapping stripes exercise categories and probability labels.
    for i in 0..36 {
        let x = -126. + i as f64 * 2.;
        product.areas.push(Area {
            category: Category {
                kind: i % 4,
                probability: if i % 4 == 1 {
                    36
                } else if i % 4 == 3 {
                    33
                } else {
                    [33, 40, 50, 60, 70, 80][i % 6]
                },
            },
            rings: vec![vec![
                (x, 20.),
                (x + 2., 20.),
                (x + 2., 55.),
                (x, 55.),
                (x, 20.),
            ]],
        });
    }
    if let Some(all) = crate::map::boundaries::counties_named() {
        let sites = board::selected(&all, &observed::report_scope(&settings, s.coverage))
            .iter()
            .filter_map(|c| {
                let (x, y) = board::centre(c);
                board::inside(c, x, y).then(|| (c.fips.clone(), (x, y)))
            })
            .collect();
        sample_sites(&mut product, &sites);
    }
    let mut scene = scene(&settings, *s, Some(&product));
    scene.panels[0].valid = format!(
        "SAMPLE • {}–{}",
        product.start.format("%b %d"),
        product.end.format("%b %d, %Y")
    );
    let p = prefix(*s);
    for (k, v) in [
        ("payload", serde_json::to_string(&scene).unwrap()),
        ("subtitle", "SAMPLE • Period category outlook".into()),
        ("source", "SAMPLE ONLY • Not for broadcast".into()),
        (
            "note",
            "SAMPLE • Synthetic categories and probabilities • Not a forecast".into(),
        ),
    ] {
        doc.data_overrides.push((format!("set.{p}.{k}"), v));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn payload() -> Value {
        json!({"spatialReference":{"wkid":4326},"features":[{"attributes":{"objectid":1,"fcst_date":1794528000000i64,"start_date":1795046400000i64,"end_date":1795392000000i64,"cat":"Above","prob":50},"geometry":{"rings":[[[-90.,30.],[-80.,30.],[-80.,45.],[-90.,45.],[-90.,30.]]]}}]})
    }
    #[test]
    fn explicit_period_dates_units_and_geometry_are_required() {
        let now = 1794571200;
        let v = payload();
        assert!(parse(&v, 6, now).is_some());
        assert!(parse(&v, 8, now).is_none());
        assert!(parse(&v, 6, now + 3 * 86400).is_none());
        let mut bad = v.clone();
        bad["exceededTransferLimit"] = json!(true);
        assert!(parse(&bad, 6, now).is_none());
        bad = v.clone();
        bad["features"][0]["attributes"]["prob"] = json!(0);
        assert!(parse(&bad, 6, now).is_none());
        for probability in [-1.0, 33.5, 100.0, 289.0, 1e20] {
            bad["features"][0]["attributes"]["prob"] = json!(probability);
            assert!(parse(&bad, 6, now).is_none());
        }
        bad["features"][0]["attributes"]["prob"] = json!(36);
        assert!(parse(&bad, 6, now).is_none());
        assert_eq!(
            Category {
                kind: 1,
                probability: 36
            }
            .label(),
            "Near normal"
        );
        bad = v.clone();
        let mut second = bad["features"][0].clone();
        second["attributes"]["objectid"] = json!(2);
        second["attributes"]["fcst_date"] = json!(1794441600000i64);
        bad["features"].as_array_mut().unwrap().push(second);
        assert!(parse(&bad, 6, now).is_none());
        bad = v.clone();
        bad["features"][0]["geometry"]["rings"][0][1][0] = json!(200);
        assert!(parse(&bad, 6, now).is_none());
        let u = query_url(true, 8, [-90., 30., -80., 45.]);
        assert!(u.contains("cpc_8_14_day_outlk/MapServer/1/query"));
        assert!(u.contains("outSR=4326"));
    }
    #[test]
    fn holes_edges_and_conflicting_category_overlaps_are_withheld() {
        let mut p = parse(&payload(), 6, 1794571200).unwrap();
        assert_eq!(at(&p, -85., 38.).unwrap().label(), "Above 50%");
        p.areas[0].rings.push(vec![
            (-86., 37.),
            (-84., 37.),
            (-84., 39.),
            (-86., 39.),
            (-86., 37.),
        ]);
        assert!(at(&p, -85., 38.).is_none());
        assert!(at(&p, -86., 38.).is_none());
        assert!(at(&p, -89., 38.).is_some());
        p.areas.push(Area {
            category: Category {
                kind: 0,
                probability: 40,
            },
            rings: vec![vec![
                (-90., 30.),
                (-80., 30.),
                (-80., 45.),
                (-90., 45.),
                (-90., 30.),
            ]],
        });
        assert!(at(&p, -89., 38.).is_none());
    }
    #[test]
    fn sampled_values_survive_geometry_release_without_shifting_counties() {
        let mut p = parse(&payload(), 6, 1794571200).unwrap();
        let sites = BTreeMap::from([
            ("inside".into(), (-85., 38.)),
            ("outside".into(), (-75., 38.)),
            ("boundary".into(), (-90., 38.)),
        ]);
        sample_sites(&mut p, &sites);
        assert_eq!(p.values.len(), 1);
        assert_eq!(p.values["inside"].label(), "Above 50%");
        p.areas.clear();
        assert_eq!(p.values["inside"].label(), "Above 50%");
    }
    #[test]
    fn long_range_periods_come_from_the_season_text() {
        let d = |y, m, day| NaiveDate::from_ymd_opt(y, m, day).unwrap();
        assert_eq!(valid_period("DJF 2026-2027"), Some((d(2026, 12, 1), d(2027, 2, 28))));
        assert_eq!(valid_period("JFM 2027"), Some((d(2027, 1, 1), d(2027, 3, 31))));
        assert_eq!(valid_period("Dec 2026"), Some((d(2026, 12, 1), d(2026, 12, 31))));
        assert_eq!(valid_period("XYZ 2026"), None);
    }
    /// Live: this winter's CPC 3-month (lead 1) and monthly outlooks parse with their real dates.
    #[test]
    #[ignore = "network"]
    fn live_long_range_outlooks_parse() {
        let bbox = [-92.0, 36.0, -84.0, 42.0];
        let now = chrono::Utc::now().timestamp();
        for (precip, first) in [(false, SEASON), (true, SEASON), (false, MONTH), (true, MONTH)] {
            let v = crate::net::runtime().block_on(super::super::report_feed::get_json(&query_url(precip, first, bbox))).expect("NOAA answered");
            let p = parse(&v, first, now).unwrap_or_else(|| panic!("parse failed for precip={precip} first={first}"));
            eprintln!("precip={precip} first={first}: issued {} valid {}–{} areas {}", p.issued, p.start, p.end, p.areas.len());
            assert!(p.end > p.start && !p.areas.is_empty());
        }
    }
}
