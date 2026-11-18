//! Current NWPS gauge flood categories and site-published impact statements.
//! Maps contain gauge point observations only; they never color counties uniformly.
use super::{
    board::{self, Band, Layout, ReportPoint, Scene, Scope},
    observed::Coverage,
};
use crate::{
    config::{GeoView, Settings},
    gfx::{
        bind::Bindings,
        model::{AnimKind, Fill, GraphicDoc, Layer},
        text::Align,
    },
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Broadcast,
    Portrait,
    Square,
    Overlay,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoverageKind {
    County,
    Region,
    State,
}
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    coverage: CoverageKind,
    format: Format,
}
const MAPS: [Spec; 12] = [
    Spec {
        key: "set_nwps_flood_map__county",
        coverage: CoverageKind::County,
        format: Format::Broadcast,
    },
    Spec {
        key: "set_nwps_flood_map__county_portrait",
        coverage: CoverageKind::County,
        format: Format::Portrait,
    },
    Spec {
        key: "set_nwps_flood_map__county_square",
        coverage: CoverageKind::County,
        format: Format::Square,
    },
    Spec {
        key: "set_nwps_flood_map__county_overlay",
        coverage: CoverageKind::County,
        format: Format::Overlay,
    },
    Spec {
        key: "set_nwps_flood_map__region",
        coverage: CoverageKind::Region,
        format: Format::Broadcast,
    },
    Spec {
        key: "set_nwps_flood_map__region_portrait",
        coverage: CoverageKind::Region,
        format: Format::Portrait,
    },
    Spec {
        key: "set_nwps_flood_map__region_square",
        coverage: CoverageKind::Region,
        format: Format::Square,
    },
    Spec {
        key: "set_nwps_flood_map__region_overlay",
        coverage: CoverageKind::Region,
        format: Format::Overlay,
    },
    Spec {
        key: "set_nwps_flood_map__state",
        coverage: CoverageKind::State,
        format: Format::Broadcast,
    },
    Spec {
        key: "set_nwps_flood_map__state_portrait",
        coverage: CoverageKind::State,
        format: Format::Portrait,
    },
    Spec {
        key: "set_nwps_flood_map__state_square",
        coverage: CoverageKind::State,
        format: Format::Square,
    },
    Spec {
        key: "set_nwps_flood_map__state_overlay",
        coverage: CoverageKind::State,
        format: Format::Overlay,
    },
];
const IMPACTS: [(&str, Format); 3] = [
    ("set_nwps_flood_impacts", Format::Broadcast),
    ("set_nwps_flood_impacts__portrait", Format::Portrait),
    ("set_nwps_flood_impacts__square", Format::Square),
];
pub(super) const KEYS: [&str; 15] = [
    "set_nwps_flood_map__county",
    "set_nwps_flood_map__county_portrait",
    "set_nwps_flood_map__county_square",
    "set_nwps_flood_map__county_overlay",
    "set_nwps_flood_map__region",
    "set_nwps_flood_map__region_portrait",
    "set_nwps_flood_map__region_square",
    "set_nwps_flood_map__region_overlay",
    "set_nwps_flood_map__state",
    "set_nwps_flood_map__state_portrait",
    "set_nwps_flood_map__state_square",
    "set_nwps_flood_map__state_overlay",
    "set_nwps_flood_impacts",
    "set_nwps_flood_impacts__portrait",
    "set_nwps_flood_impacts__square",
];

#[derive(Clone, Debug)]
struct Gauge {
    id: String,
    name: String,
    lon: f64,
    lat: f64,
    observed_at: i64,
    category: usize,
}
#[derive(Clone, Debug)]
struct Impact {
    stage: f64,
    text: String,
}
#[derive(Clone, Debug)]
struct ImpactSite {
    id: String,
    name: String,
    county: String,
    state: String,
    distance_mi: f64,
    fetched_at: i64,
    stage_unit: String,
    datum: String,
    impacts: Vec<Impact>,
    source_entry_count: usize,
}
struct Snapshot {
    home: [u64; 2],
    checked: i64,
    gauges: Vec<Gauge>,
    impact_site: Option<ImpactSite>,
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
fn valid_home(h: GeoView) -> bool {
    h.lon.is_finite()
        && h.lat.is_finite()
        && (-178.0..=-60.0).contains(&h.lon)
        && (15.0..=72.0).contains(&h.lat)
}
fn safe_text(v: &Value, limit: usize) -> Option<String> {
    let s = v.as_str()?.trim();
    (!s.is_empty() && s.chars().count() <= limit && !s.chars().any(char::is_control))
        .then(|| s.to_owned())
}
fn gauge_id(v: &Value) -> Option<String> {
    let s = safe_text(v, 15)?;
    s.bytes()
        .all(|b| b.is_ascii_alphanumeric())
        .then(|| s.to_ascii_uppercase())
}
fn stamp(v: &Value) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(v.as_str()?)
        .ok()
        .map(|d| d.timestamp())
}
fn date(t: i64, fmt: &str) -> String {
    chrono::DateTime::from_timestamp(t, 0)
        .map(|d| d.format(fmt).to_string())
        .unwrap_or_default()
}
fn category(v: &Value) -> Option<usize> {
    match v.as_str()?.trim().to_ascii_lowercase().as_str() {
        "action" => Some(0),
        "minor" => Some(1),
        "moderate" => Some(2),
        "major" => Some(3),
        _ => None,
    }
}
fn unit(v: &Value) -> Option<&'static str> {
    match v.as_str()?.trim().to_ascii_lowercase().as_str() {
        "ft" | "feet" | "foot" => Some("ft"),
        "m" | "meter" | "meters" | "metre" | "metres" => Some("m"),
        _ => None,
    }
}
fn number(v: &Value, min: f64, max: f64) -> Option<f64> {
    v.as_f64()
        .filter(|x| x.is_finite() && (min..=max).contains(x))
}
fn distance(home: GeoView, lon: f64, lat: f64) -> Option<f64> {
    if !valid_home(home)
        || !lon.is_finite()
        || !lat.is_finite()
        || !(-180.0..=180.0).contains(&lon)
        || !(-85.0..=85.0).contains(&lat)
    {
        return None;
    }
    let a = ((lat - home.lat).to_radians() / 2.).sin().powi(2)
        + home.lat.to_radians().cos()
            * lat.to_radians().cos()
            * ((lon - home.lon).to_radians() / 2.).sin().powi(2);
    Some(3958.7613 * 2. * a.clamp(0., 1.).sqrt().asin())
}

fn decode_gauges(value: &Value, now: i64) -> Option<Vec<Gauge>> {
    let items = value.get("gauges")?.as_array()?;
    if items.len() > 12_000 {
        return None;
    }
    let mut out = Vec::new();
    let mut ids = HashSet::new();
    for g in items {
        let Some(id) = g.get("lid").and_then(gauge_id) else {
            continue;
        };
        let Some(lon) = g.get("longitude").and_then(Value::as_f64) else {
            continue;
        };
        let Some(lat) = g.get("latitude").and_then(Value::as_f64) else {
            continue;
        };
        if !lon.is_finite()
            || !lat.is_finite()
            || !(-180.0..=180.0).contains(&lon)
            || !(-85.0..=85.0).contains(&lat)
        {
            continue;
        }
        // The gauge-list contract exposes current observed status separately from forecast status.
        let Some(obs) = g.pointer("/status/observed") else {
            continue;
        };
        let Some(observed_at) = obs.get("validTime").and_then(stamp) else {
            continue;
        };
        if !(0..=7200).contains(&(now - observed_at)) {
            continue;
        }
        // Current flood categories require a numeric stage and recognized stage units; unknown
        // units/categories stay unplotted rather than being treated as safe or converted.
        let Some(primary) = obs.get("primary") else {
            continue;
        };
        let Some(primary_unit) = obs.get("primaryUnit") else {
            continue;
        };
        if number(primary, -100.0, 3000.0).is_none() || unit(primary_unit).is_none() {
            continue;
        }
        let Some(name) = g.get("name").and_then(|v| safe_text(v, 90)) else {
            continue;
        };
        let Some(category) = obs.get("floodCategory").and_then(category) else {
            continue;
        };
        if !ids.insert(id.clone()) {
            return None;
        }
        out.push(Gauge {
            id,
            name,
            lon,
            lat,
            observed_at,
            category,
        });
    }
    Some(out)
}

fn stage_unit_matches(unit_value: &Value) -> Option<&'static str> {
    unit(unit_value)
}
fn decode_impacts(
    meta: &Value,
    expected_id: &str,
    expected_lon: f64,
    expected_lat: f64,
    home: GeoView,
    fetched_at: i64,
) -> Option<ImpactSite> {
    if gauge_id(meta.get("lid")?)?.as_str() != expected_id {
        return None;
    }
    let lon = number(meta.get("longitude")?, -180.0, 180.0)?;
    let lat = number(meta.get("latitude")?, -85.0, 85.0)?;
    if (lon - expected_lon).abs() > 0.001 || (lat - expected_lat).abs() > 0.001 {
        return None;
    }
    let distance_mi = distance(home, lon, lat)?;
    if distance_mi > 15.0 {
        return None;
    }
    if meta.pointer("/inService/enabled").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let stage_unit = stage_unit_matches(meta.pointer("/flood/stageUnits")?)?;
    // Keep the gauge's own stage reference. List any supplied vertical datum labels, but never
    // assume one of several alternatives is the reference for these thresholds or convert them.
    let datum_values = meta
        .pointer("/datums/vertical/value")
        .and_then(Value::as_array);
    let mut datums = datum_values
        .into_iter()
        .flatten()
        .filter_map(|v| {
            v.get("label")
                .and_then(|x| safe_text(x, 60))
                .or_else(|| v.get("abbrev").and_then(|x| safe_text(x, 30)))
        })
        .collect::<Vec<_>>();
    datums.sort();
    datums.dedup();
    let more_datums = datums.len() > 3;
    datums.truncate(3);
    let raw = meta.pointer("/flood/impacts")?.as_array()?;
    if raw.is_empty() || raw.len() > 256 {
        return None;
    }
    let mut impacts = Vec::new();
    for entry in raw {
        let Some(stage) = entry.get("stage").and_then(|v| number(v, -100.0, 3000.0)) else {
            continue;
        };
        let Some(text) = entry.get("statement").and_then(|v| safe_text(v, 170)) else {
            continue;
        };
        impacts.push(Impact { stage, text });
    }
    if impacts.is_empty() {
        return None;
    }
    impacts.sort_by(|a, b| a.stage.total_cmp(&b.stage));
    impacts.dedup_by(|a, b| a.stage == b.stage && a.text == b.text);
    let id = gauge_id(meta.get("lid")?)?;
    let name = meta
        .get("description")
        .and_then(|v| safe_text(v, 100))
        .or_else(|| meta.get("name").and_then(|v| safe_text(v, 80)))?;
    let county = meta
        .get("county")
        .and_then(|v| safe_text(v, 60))
        .unwrap_or_else(|| "County not provided".into());
    let state = meta
        .pointer("/state/abbreviation")
        .and_then(|v| safe_text(v, 2))?;
    Some(ImpactSite {
        id,
        name,
        county,
        state,
        distance_mi,
        fetched_at,
        stage_unit: stage_unit.into(),
        datum: if datums.is_empty() {
            "Gauge-specific stage reference; no vertical datum listed".into()
        } else {
            format!(
                "Gauge-specific stage reference • listed vertical datum{}: {}{}",
                if datums.len() == 1 { "" } else { "s" },
                datums.join(", "),
                if more_datums { ", …" } else { "" }
            )
        },
        impacts,
        source_entry_count: raw.len(),
    })
}

fn home_county<'a>(
    all: &'a [crate::map::boundaries::County],
    home: GeoView,
) -> Option<&'a crate::map::boundaries::County> {
    all.iter().find(|c| board::inside(c, home.lon, home.lat))
}
fn scope(
    settings: &Settings,
    kind: CoverageKind,
    all: &[crate::map::boundaries::County],
) -> Option<Scope> {
    match kind {
        CoverageKind::County => {
            let c = home_county(all, settings.home)?;
            Some(Scope {
                states: vec![c.state.clone()],
                county: Some(c.fips.clone()),
                bbox: None,
            })
        }
        CoverageKind::Region => Some(super::observed::report_scope(settings, Coverage::Region)),
        CoverageKind::State => {
            let c = home_county(all, settings.home)?;
            Some(Scope {
                states: vec![c.state.clone()],
                county: None,
                bbox: None,
            })
        }
    }
}
fn in_scope(g: &Gauge, s: &Scope, all: &[crate::map::boundaries::County]) -> bool {
    if let Some([w, south, e, n]) = s.bbox {
        return (w..=e).contains(&g.lon) && (south..=n).contains(&g.lat);
    }
    all.iter().any(|c| {
        s.county.as_ref().is_none_or(|f| f == &c.fips)
            && (s.county.is_some() || s.states.iter().any(|state| state == &c.state))
            && board::inside(c, g.lon, g.lat)
    })
}
fn coverage_label(
    settings: &Settings,
    coverage: CoverageKind,
    all: &[crate::map::boundaries::County],
) -> Option<String> {
    let county = home_county(all, settings.home)?;
    let state = crate::winter::state_name(&county.state);
    let label = match coverage {
        CoverageKind::County => format!("{} County • {}", county.name, state),
        CoverageKind::Region => format!("Local region • {} County, {}", county.name, state),
        CoverageKind::State => state.to_string(),
    };
    // Board scene titles have an 85-character limit. Keep scope honest for long
    // county/census-area names without rejecting an otherwise valid map.
    Some(if label.chars().count() <= 50 {
        label
    } else {
        match coverage {
            CoverageKind::County => format!("Selected home county • {state}"),
            CoverageKind::Region => format!("Selected home region • {state}"),
            CoverageKind::State => state.to_owned(),
        }
    })
}
fn request_bounds(home: GeoView, all: &[crate::map::boundaries::County]) -> Option<[f64; 4]> {
    let home_county = home_county(all, home)?;
    let state = home_county.state.as_str();
    let mut bounds = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for c in all.iter().filter(|c| c.state.as_str() == state) {
        for &(lon, lat) in c.rings.iter().flatten() {
            if !lon.is_finite() || !lat.is_finite() {
                return None;
            }
            bounds[0] = bounds[0].min(lon);
            bounds[1] = bounds[1].min(lat);
            bounds[2] = bounds[2].max(lon);
            bounds[3] = bounds[3].max(lat);
        }
    }
    // Include the independent regional window if it extends beyond the home state's boundary.
    let (w, south, e, north) = (
        home.lon - 2.0,
        home.lat - 1.2,
        home.lon + 2.0,
        home.lat + 1.2,
    );
    bounds[0] = bounds[0].min(w);
    bounds[1] = bounds[1].min(south);
    bounds[2] = bounds[2].max(e);
    bounds[3] = bounds[3].max(north);
    if bounds[0] >= bounds[2]
        || bounds[1] >= bounds[3]
        || bounds[2] - bounds[0] > 55.
        || bounds[3] - bounds[1] > 45.
    {
        return None;
    }
    Some(bounds)
}
fn nearest_in_home_county<'a>(
    gauges: &'a [Gauge],
    home: GeoView,
    all: &[crate::map::boundaries::County],
) -> Option<&'a Gauge> {
    let c = home_county(all, home)?;
    gauges
        .iter()
        .filter(|g| board::inside(c, g.lon, g.lat))
        .filter_map(|g| distance(home, g.lon, g.lat).map(|d| (g, d)))
        .filter(|(_, d)| *d <= 15.0)
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.id.cmp(&b.0.id)))
        .map(|(g, _)| g)
}

async fn fetch(home: GeoView, now: i64) -> Option<Snapshot> {
    if !valid_home(home) {
        return None;
    }
    let all = crate::map::boundaries::counties_named()?;
    let bounds = request_bounds(home, &all)?;
    let url = format!(
        "https://api.water.noaa.gov/nwps/v1/gauges?bbox.xmin={:.5}&bbox.ymin={:.5}&bbox.xmax={:.5}&bbox.ymax={:.5}&srid=EPSG_4326",
        bounds[0], bounds[1], bounds[2], bounds[3]
    );
    // The list read has its own 40-second bound. Keep optional detail time within a
    // 55-second total refresh budget so it cannot discard an already decoded list.
    let deadline = Instant::now() + Duration::from_secs(55);
    let raw = tokio::time::timeout_at(
        deadline,
        super::report_feed::get_json(&url),
    )
    .await
    .ok()??;
    let gauges = decode_gauges(&raw, now)?;
    let impact_site = if let Some(g) = nearest_in_home_county(&gauges, home, &all) {
        let url = format!("https://api.water.noaa.gov/nwps/v1/gauges/{}", g.id);
        let remaining = deadline.saturating_duration_since(Instant::now());
        let detail_budget = remaining.min(Duration::from_secs(8));
        if detail_budget.is_zero() {
            None
        } else {
            tokio::time::timeout(detail_budget, super::report_feed::get_json(&url))
                .await
                .ok()
                .flatten()
                .and_then(|m| {
                    decode_impacts(
                        &m,
                        &g.id,
                        g.lon,
                        g.lat,
                        home,
                        chrono::Utc::now().timestamp(),
                    )
                })
        }
    } else {
        None
    };
    Some(Snapshot {
        home: home_key(home),
        checked: now,
        gauges,
        impact_site,
    })
}
pub(super) async fn refresh(home: GeoView) {
    let ticket = match store().lock() {
        Ok(mut s) => {
            s.generation += 1;
            s.current = None;
            s.generation
        }
        Err(_) => return,
    };
    let result = fetch(home, chrono::Utc::now().timestamp()).await;
    if let Ok(mut s) = store().lock() {
        if s.generation == ticket {
            s.current = result.map(Arc::new);
        }
    }
}
fn current(settings: &Settings, now: i64) -> Option<Arc<Snapshot>> {
    store()
        .lock()
        .ok()?
        .current
        .as_ref()
        .filter(|s| s.home == home_key(settings.home) && (0..=1200).contains(&(now - s.checked)))
        .cloned()
}

const CATEGORY_LABELS: [&str; 4] = ["Action", "Minor", "Moderate", "Major"];
const CATEGORY_COLORS: [&str; 4] = ["#55D5F9", "#FFE34A", "#FF982F", "#F34545"];
fn bands() -> Vec<Band> {
    CATEGORY_LABELS
        .iter()
        .zip(CATEGORY_COLORS)
        .map(|(label, color)| Band {
            label: (*label).into(),
            color: color.into(),
        })
        .collect()
}
fn scene(settings: &Settings, spec: Spec, snap: Option<&Snapshot>, now: i64) -> Scene {
    let mut s = board::empty(Layout::Single);
    let Some(all) = crate::map::boundaries::counties_named() else {
        s.scope.states.clear();
        s.panels[0].detail = "County boundary data unavailable".into();
        return s;
    };
    let Some(scope) = scope(settings, spec.coverage, &all) else {
        s.scope.states.clear();
        s.panels[0].detail = "Home location is outside supported county boundaries".into();
        return s;
    };
    s.scope = scope;
    s.legend = bands();
    s.legend_title = "Observed NWPS flood category at gauge sites".into();
    let label =
        coverage_label(settings, spec.coverage, &all).unwrap_or_else(|| "Selected area".into());
    let p = &mut s.panels[0];
    p.title = format!("River Gauge Flood Categories • {label}");
    p.detail = "Gauge-site observations only • Unmarked locations are not assessed".into();
    let Some(snapshot) = snap
        .filter(|v| v.home == home_key(settings.home) && (0..=1200).contains(&(now - v.checked)))
    else {
        p.detail = "NWPS observation feed unavailable • Unmarked locations are not assessed".into();
        return s;
    };
    let mut rows: Vec<_> = snapshot
        .gauges
        .iter()
        .filter(|g| (0..=7200).contains(&(now - g.observed_at)) && in_scope(g, &s.scope, &all))
        .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    let Some(min_t) = rows.iter().map(|g| g.observed_at).min() else {
        p.detail =
            "No current categorized gauge observations in this view • Not an all-clear".into();
        return s;
    };
    let max_t = rows.iter().map(|g| g.observed_at).max().unwrap_or(min_t);
    p.valid = format!(
        "Observed {}–{} UTC • each up to 2 h old",
        date(min_t, "%Y-%m-%d %H:%M"),
        date(max_t, "%Y-%m-%d %H:%M")
    );
    p.detail = format!(
        "{} gauge sites with current categorized observations • Unmarked locations are not assessed",
        rows.len()
    );
    p.summary_title = "Named gauges • observed category".into();
    p.summary = Some(
        rows.iter()
            .take(7)
            .map(|g| (g.name.clone(), CATEGORY_LABELS[g.category].to_string()))
            .collect(),
    );
    p.points = Some(
        rows.iter()
            .map(|g| ReportPoint {
                id: g.id.clone(),
                lon: g.lon,
                lat: g.lat,
                label: format!(
                    "{} • {}",
                    g.name.chars().take(58).collect::<String>(),
                    CATEGORY_LABELS[g.category]
                ),
                category: g.category,
                value: None,
            })
            .collect(),
    );
    s.reviewed = true;
    s
}
fn prefix(c: CoverageKind) -> &'static str {
    match c {
        CoverageKind::County => "nwpsflood.county",
        CoverageKind::Region => "nwpsflood.region",
        CoverageKind::State => "nwpsflood.state",
    }
}
fn bind_map(
    b: &mut Bindings,
    settings: &Settings,
    coverage: CoverageKind,
    snap: Option<&Snapshot>,
    now: i64,
) {
    let spec = Spec {
        key: "",
        coverage,
        format: Format::Broadcast,
    };
    let scene = scene(settings, spec, snap, now);
    let prefix = prefix(coverage);
    let subtitle = crate::map::boundaries::counties_named()
        .as_deref()
        .and_then(|all| {
            // The Classic header shares one line with the title: the region view says
            // "Dubois County area" there (the panel title keeps "Local region • ...").
            if coverage == CoverageKind::Region {
                return home_county(all, settings.home).map(|c| format!("{} County area", c.name));
            }
            coverage_label(settings, coverage, all)
        })
        // The title already says gauges; a shorter line keeps clear of it in the Classic header.
        .map(|v| v.to_string())
        .unwrap_or_else(|| "Gauge observations • area unavailable".into());
    let note = "Observed NWPS category is valid only at each named gauge • Unmarked places are not assessed; this map is not an area-wide all-clear";
    for (field, value) in [
        ("payload", serde_json::to_string(&scene).unwrap_or_default()),
        ("subtitle", subtitle),
        (
            "source",
            "Source: NOAA / NWS National Water Prediction Service • observed gauge status".into(),
        ),
        ("note", note.into()),
    ] {
        b.set(&format!("set.{prefix}.{field}"), &value);
    }
}
/// Impact fields plus the two joined lines the template shows ("datum • unit", "note • fetched"),
/// joined here so an empty part leaves no stray bullet.
fn impact_values(site: Option<&ImpactSite>, now: i64) -> BTreeMap<String, String> {
    let mut v = impact_values_raw(site, now);
    let join = |a: &str, b: &str| [a, b].iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" • ");
    let datum_line = join(&v["datum"], &v["unit"]);
    let note_line = join(&v["note"], &v["fetched"]);
    v.insert("datum_line".into(), datum_line);
    v.insert("note_line".into(), note_line);
    v
}
fn impact_values_raw(site: Option<&ImpactSite>, now: i64) -> BTreeMap<String, String> {
    let mut v = BTreeMap::new();
    for key in [
        "site", "nearby", "fetched", "datum", "unit", "source", "note", "status",
    ] {
        v.insert(key.into(), String::new());
    }
    for i in 0..6 {
        for key in ["stage", "text"] {
            v.insert(format!("row{i}_{key}"), String::new());
        }
    }
    v.insert("site".into(), "Named gauge impacts unavailable".into());
    v.insert(
        "status".into(),
        "Site-specific published impact statements".into(),
    );
    v.insert(
        "source".into(),
        "Source: NOAA / NWS National Water Prediction Service gauge metadata".into(),
    );
    v.insert(
        "note".into(),
        "Impact statements are tied to the named gauge; they do not predict flooding at home"
            .into(),
    );
    let Some(s) = site.filter(|s| (0..=1200).contains(&(now - s.fetched_at))) else {
        return v;
    };
    v.insert("note".into(), format!("Showing {} selected statements from {} source entries • Named gauge only; not a home forecast", s.impacts.len().min(6), s.source_entry_count));
    v.insert("site".into(), format!("{} • {}", s.name, s.id));
    v.insert(
        "nearby".into(),
        format!(
            "{}, {} • {:.1} mi from station home",
            s.county, s.state, s.distance_mi
        ),
    );
    v.insert(
        "fetched".into(),
        format!(
            "Gauge metadata retrieved {} UTC",
            date(s.fetched_at, "%Y-%m-%d %H:%M")
        ),
    );
    v.insert("datum".into(), s.datum.clone());
    v.insert(
        "unit".into(),
        format!(
            "Impact stages • {} • published/effective date not supplied by API",
            s.stage_unit
        ),
    );
    for (i, r) in s.impacts.iter().take(6).enumerate() {
        v.insert(
            format!("row{i}_stage"),
            format!("{:.1} {}", r.stage, s.stage_unit),
        );
        v.insert(format!("row{i}_text"), r.text.clone());
    }
    v
}
fn bind_impacts(b: &mut Bindings, settings: &Settings, snap: Option<&Snapshot>, now: i64) {
    let site = snap
        .filter(|v| v.home == home_key(settings.home) && (0..=1200).contains(&(now - v.checked)))
        .and_then(|v| v.impact_site.as_ref());
    for (k, v) in impact_values(site, now) {
        b.set(&format!("set.nwpsimpact.{k}"), &v);
    }
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let snap = current(settings, now);
    for c in [
        CoverageKind::County,
        CoverageKind::Region,
        CoverageKind::State,
    ] {
        bind_map(b, settings, c, snap.as_deref(), now);
    }
    bind_impacts(b, settings, snap.as_deref(), now);
}
fn token(prefix: &str, field: &str) -> String {
    format!("{{{{set.{prefix}.{field}}}}}")
}
fn map_template(title: &str, prefix: &str, format: Format) -> GraphicDoc {
    match format {
        Format::Broadcast => super::comparison::template(title, "Flooding", prefix, Layout::Single),
        Format::Portrait => super::comparison::portrait_template(title, "Flooding", prefix),
        Format::Square => super::comparison::square_template(title, "Flooding", prefix),
        Format::Overlay => overlay_template(title, prefix),
    }
}
fn overlay_template(title: &str, prefix: &str) -> GraphicDoc {
    let mut d = GraphicDoc::new(title, "Flooding", None);
    d.layers.push(
        Layer::rect(
            "Gauge map overlay card",
            58.,
            675.,
            1180.,
            354.,
            Fill::solid("$panel"),
            20.,
        )
        .with_anim(AnimKind::Fade, 0.1, 0.35),
    );
    d.layers.push(
        Layer::rect(
            "Gauge overlay accent",
            58.,
            675.,
            10.,
            354.,
            Fill::solid("$primary"),
            5.,
        )
        .with_anim(AnimKind::Wipe, 0.1, 0.35),
    );
    d.layers.push(
        Layer::risk_map(
            "Gauge point map",
            82.,
            698.,
            1132.,
            240.,
            &format!("set:nwpsgauges:{}", token(prefix, "payload")),
        )
        .with_anim(AnimKind::Fade, 0.18, 0.4),
    );
    super::reference::label(
        &mut d,
        "Gauge overlay subtitle",
        (86., 939., 1120., 25.),
        &token(prefix, "subtitle"),
        19.,
        "#C7D4E0",
        Align::Left,
    );
    super::reference::label(
        &mut d,
        "Gauge overlay source",
        (86., 964., 1120., 24.),
        &token(prefix, "source"),
        17.,
        "#D5E1EC",
        Align::Left,
    );
    super::reference::label(
        &mut d,
        "Gauge overlay caveat",
        (86., 988., 1120., 31.),
        &token(prefix, "note"),
        16.,
        "#B9C9D8",
        Align::Left,
    );
    super::reference::label(
        &mut d,
        "Gauge overlay station",
        (1000., 682., 210., 22.),
        "{{station}}",
        16.,
        "$muted",
        Align::Right,
    );
    d
}
fn impact_template(format: Format) -> GraphicDoc {
    let title = "NWPS Gauge Impact Statements";
    let mut d = match format {
        Format::Portrait => super::reference::social_frame(
            title,
            "Flooding",
            &token("nwpsimpact", "fetched"),
            &token("nwpsimpact", "source"),
            true,
        ),
        Format::Square => super::reference::social_frame(
            title,
            "Flooding",
            &token("nwpsimpact", "fetched"),
            &token("nwpsimpact", "source"),
            false,
        ),
        _ => super::reference::frame(
            title,
            "Flooding",
            &token("nwpsimpact", "fetched"),
            &token("nwpsimpact", "source"),
        ),
    };
    let (x, w, top, step, size) = match format {
        Format::Portrait => (54., 972., 410., 170., 30.),
        Format::Square => (54., 972., 360., 84., 23.),
        _ => (80., 1760., 350., 88., 28.),
    };
    super::reference::label(
        &mut d,
        "Named gauge",
        (x, top - 170., w, 70.),
        &token("nwpsimpact", "site"),
        if format == Format::Broadcast {
            48.
        } else {
            35.
        },
        "$text",
        Align::Left,
    );
    super::reference::label(
        &mut d,
        "Gauge location",
        (x, top - 95., w, 42.),
        &token("nwpsimpact", "nearby"),
        26.,
        "$muted",
        Align::Left,
    );
    super::reference::label(
        &mut d,
        "Gauge vertical datum",
        (x, top - 50., w, 35.),
        &token("nwpsimpact", "datum_line"),
        22.,
        "$muted",
        Align::Left,
    );
    d.layers.push(
        crate::gfx::templates::panel("Gauge impact statements", x, top, w, step * 6. + 24.)
            .with_anim(AnimKind::Rise, 0.15, 0.4),
    );
    for i in 0..6 {
        let y = top + 16. + i as f32 * step;
        super::reference::label(
            &mut d,
            &format!("Impact threshold {i}"),
            (x + 22., y, 190., step - 12.),
            &token("nwpsimpact", &format!("row{i}_stage")),
            if format == Format::Broadcast {
                33.
            } else {
                28.
            },
            "$primary",
            Align::Left,
        );
        super::reference::label(
            &mut d,
            &format!("Impact description {i}"),
            (x + 220., y, w - 245., step - 12.),
            &token("nwpsimpact", &format!("row{i}_text")),
            size,
            "$text",
            Align::Left,
        );
    }
    let note_y = top + step * 6. + 45.;
    super::reference::label(
        &mut d,
        "Gauge impact note",
        (x, note_y, w, 54.),
        &token("nwpsimpact", "note_line"),
        22.,
        "$muted",
        Align::Left,
    );
    d
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = MAPS.iter().find(|s| s.key == key);
    if let Some(s) = spec {
        let prefix = prefix(s.coverage);
        let mut doc = map_template("River Gauge Flood Categories", prefix, s.format);
        doc.template = Some(key.to_owned());
        return Some(doc);
    }
    let format = IMPACTS.iter().find(|(k, _)| *k == key)?.1;
    let mut doc = impact_template(format);
    doc.template = Some(key.to_owned());
    Some(doc)
}

/// Draw transparent point-only gauge overlay. Parent registers this in draw_map dispatcher.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_overlay(
    pm: &mut tiny_skia::Pixmap,
    glyphs: &mut crate::gfx::text::GlyphCache,
    local: tiny_skia::Transform,
    mask: Option<&tiny_skia::Mask>,
    w: f32,
    h: f32,
    spec: &str,
    opacity: f32,
    font: &str,
) -> bool {
    let Some(raw) = spec.strip_prefix("set:nwpsgauges:") else {
        return false;
    };
    let Some(scene) = board::parse(raw, Layout::Single) else {
        return true;
    };
    let (dw, dh) = (
        (w * local.sx.abs()).round().clamp(1., 4096.) as u32,
        (h * local.sy.abs()).round().clamp(1., 4096.) as u32,
    );
    let Some(mut layer) = tiny_skia::Pixmap::new(dw, dh) else {
        return true;
    };
    let Some(panel) = scene.panels.first() else {
        return true;
    };
    let all = crate::map::boundaries::counties_named();
    let counties = all
        .as_ref()
        .map(|all| board::selected(all, &scene.scope))
        .unwrap_or_default();
    let mut bounds = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for c in &counties {
        for &(x, y) in c.rings.iter().flatten() {
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x);
            bounds[3] = bounds[3].max(y);
        }
    }
    if let Some(b) = scene.scope.bbox {
        bounds = [
            bounds[0].min(b[0]),
            bounds[1].min(b[1]),
            bounds[2].max(b[2]),
            bounds[3].max(b[3]),
        ];
    }
    if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
        return true;
    }
    let width = dw as f32;
    let height = dh as f32;
    let mapw = width * 0.64;
    let top = height * 0.17;
    let maph = height * 0.68;
    let mid = ((bounds[1] + bounds[3]) / 2.)
        .to_radians()
        .cos()
        .abs()
        .max(0.15);
    let gw = (bounds[2] - bounds[0]) * mid;
    let gh = bounds[3] - bounds[1];
    if gw <= 0. || gh <= 0. {
        return true;
    }
    let scale = ((mapw - 16.) / (gw as f32)).min((maph - 16.) / (gh as f32));
    let ox = (mapw - gw as f32 * scale) / 2.;
    let oy = top + (maph - gh as f32 * scale) / 2.;
    let project = |lon: f64, lat: f64| {
        (
            (ox + ((lon - bounds[0]) * mid) as f32 * scale),
            (oy + ((bounds[3] - lat) as f32 * scale)),
        )
    };
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(180, 205, 225, 210);
    paint.anti_alias = true;
    let stroke = tiny_skia::Stroke {
        width: 0.65,
        ..Default::default()
    };
    for c in &counties {
        for r in &c.rings {
            let mut path = tiny_skia::PathBuilder::new();
            for (i, &(lon, lat)) in r.iter().enumerate() {
                let (x, y) = project(lon, lat);
                if i == 0 {
                    path.move_to(x, y)
                } else {
                    path.line_to(x, y)
                }
            }
            if let Some(p) = path.finish() {
                layer.stroke_path(&p, &paint, &stroke, tiny_skia::Transform::identity(), None);
            }
        }
    }
    let points = panel.points.as_deref().unwrap_or_default();
    let legend = board::bands(&scene);
    for p in points {
        let (x, y) = project(p.lon, p.lat);
        let color = legend
            .get(p.category)
            .map(|b| crate::gfx::parse_hex(&b.color).unwrap_or(tiny_skia::Color::WHITE))
            .unwrap_or(tiny_skia::Color::WHITE);
        let mut dot = tiny_skia::Paint::default();
        dot.set_color(color);
        dot.anti_alias = true;
        let radius = (4.2 * width / 1132.).clamp(2., 7.);
        if let Some(circle) = tiny_skia::PathBuilder::from_circle(x, y, radius) {
            layer.fill_path(
                &circle,
                &dot,
                tiny_skia::FillRule::Winding,
                tiny_skia::Transform::identity(),
                None,
            );
        }
    }
    let scale_text = (width / 1132.).clamp(0.45, 1.5);
    let map_portion = width * 0.64;
    let mut draw = |pm: &mut tiny_skia::Pixmap,
                    value: &str,
                    rect: (f32, f32, f32, f32),
                    size: f32,
                    color: &str| {
        crate::gfx::text::draw_text_box(
            pm,
            glyphs,
            value,
            rect,
            &crate::gfx::text::TextStyle {
                family: font.to_string(),
                weight: crate::gfx::text::Weight::SemiBold,
                size: size * scale_text,
                color: crate::gfx::parse_hex(color).unwrap_or(tiny_skia::Color::WHITE),
                letter_spacing: 0.,
            },
            Align::Left,
            true,
            true,
            true,
            false,
            1.,
        )
    };
    draw(
        &mut layer,
        &panel.title,
        (
            8. * scale_text,
            2. * scale_text,
            map_portion - 16.,
            26. * scale_text,
        ),
        21.,
        "#F4F8FC",
    );
    draw(
        &mut layer,
        &panel.valid,
        (
            8. * scale_text,
            27. * scale_text,
            map_portion - 16.,
            24. * scale_text,
        ),
        15.,
        "#B9C9D8",
    );
    draw(
        &mut layer,
        "GAUGE CATEGORIES",
        (
            map_portion + 12.,
            4. * scale_text,
            width - map_portion - 16.,
            24. * scale_text,
        ),
        16.,
        "#F4F8FC",
    );
    for (i, band) in scene.legend.iter().enumerate() {
        let y = (32. + i as f32 * 29.) * scale_text;
        let c = crate::gfx::parse_hex(&band.color).unwrap_or(tiny_skia::Color::WHITE);
        if let Some(r) = tiny_skia::Rect::from_xywh(
            map_portion + 12.,
            y + 4.,
            13. * scale_text,
            13. * scale_text,
        ) {
            let mut p = tiny_skia::Paint::default();
            p.set_color(c);
            layer.fill_rect(r, &p, tiny_skia::Transform::identity(), None);
        }
        draw(
            &mut layer,
            &band.label,
            (
                map_portion + 31.,
                y,
                width - map_portion - 37.,
                24. * scale_text,
            ),
            15.,
            "#D5E1EC",
        );
    }
    let mut names = points
        .iter()
        .take(3)
        .map(|p| format!("{} • {}", p.id, p.label))
        .collect::<Vec<_>>();
    if names.is_empty() {
        names.push("No fresh categorized gauge sites".into());
    }
    draw(
        &mut layer,
        "Named gauge sites",
        (
            map_portion + 12.,
            157. * scale_text,
            width - map_portion - 16.,
            20. * scale_text,
        ),
        14.,
        "#B9C9D8",
    );
    for (i, n) in names.iter().enumerate() {
        draw(
            &mut layer,
            n,
            (
                map_portion + 12.,
                177. * scale_text + i as f32 * 17. * scale_text,
                width - map_portion - 18.,
                18. * scale_text,
            ),
            12.,
            "#F2F6FA",
        );
    }
    draw(
        &mut layer,
        "Unmarked locations are not assessed • Not an all-clear",
        (
            8. * scale_text,
            height - 21. * scale_text,
            map_portion - 16.,
            19. * scale_text,
        ),
        12.,
        "#B9C9D8",
    );
    pm.draw_pixmap(
        0,
        0,
        layer.as_ref(),
        &tiny_skia::PixmapPaint {
            opacity: opacity.clamp(0., 1.),
            ..Default::default()
        },
        tiny_skia::Transform::from_translate(local.tx, local.ty),
        mask,
    );
    true
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let key = doc.template.as_deref().unwrap_or("");
    let settings = Settings::default();
    let now = chrono::Utc::now().timestamp();
    if let Some(spec) = MAPS.iter().find(|s| s.key == key) {
        let mut gauges = Vec::new();
        if let Some(all) = crate::map::boundaries::counties_named() {
            if let Some(scope) = scope(&settings, spec.coverage, &all) {
                let mut candidates = board::selected(&all, &scope);
                candidates.retain(|c| {
                    let (x, y) = board::centre(c);
                    board::inside(c, x, y)
                });
                for (i, c) in candidates.iter().take(12).enumerate() {
                    let (lon, lat) = board::centre(c);
                    gauges.push(Gauge {
                        id: format!("SAMPLE{i:02}"),
                        name: format!("Sample {} gauge", c.name),
                        lon,
                        lat,
                        observed_at: now - 900,
                        category: i % CATEGORY_LABELS.len(),
                    });
                }
            }
        }
        let fake = Snapshot {
            home: home_key(settings.home),
            checked: now,
            gauges,
            impact_site: None,
        };
        let mut scene = scene(&settings, *spec, Some(&fake), now);
        scene.panels[0].valid = format!("SAMPLE ONLY • {}", scene.panels[0].valid);
        let prefix = prefix(spec.coverage);
        for (field, value) in [
            ("payload", serde_json::to_string(&scene).unwrap_or_default()),
            ("subtitle", "SAMPLE • synthetic gauge sites".into()),
            ("source", "SAMPLE ONLY • Not for broadcast".into()),
            (
                "note",
                "SAMPLE • Synthetic positions and flood categories • Not observations or forecast"
                    .into(),
            ),
        ] {
            doc.data_overrides
                .push((format!("set.{prefix}.{field}"), value));
        }
        return;
    }
    if IMPACTS.iter().any(|(k, _)| *k == key) {
        let site = ImpactSite {
            id: "SAMPLE".into(),
            name: "Sample River gauge near Town".into(),
            county: "Sample County".into(),
            state: "IN".into(),
            distance_mi: 0.0,
            fetched_at: now,
            stage_unit: "ft".into(),
            datum: "SAMPLE datum".into(),
            source_entry_count: 3,
            impacts: vec![
                Impact {
                    stage: 12.0,
                    text: "Sample low road near gauge may flood.".into(),
                },
                Impact {
                    stage: 17.5,
                    text: "Sample park access can be affected.".into(),
                },
                Impact {
                    stage: 22.0,
                    text: "Sample low structures near gauge may be affected.".into(),
                },
            ],
        };
        for (field, value) in impact_values(Some(&site), now) {
            doc.data_overrides.push((
                format!("set.nwpsimpact.{field}"),
                match field.as_str() {
                    "source" | "note" => "SAMPLE ONLY • Not for broadcast".into(),
                    "fetched" => "SAMPLE ONLY • Synthetic metadata time".into(),
                    "datum" => "SAMPLE • Synthetic gauge datum".into(),
                    _ => value,
                },
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_fresh_observed_categories_with_known_units_plot() {
        let now = 1_800_000_000;
        let good = json!({"gauges":[{"lid":"ABCD","name":"River at Town","longitude":-86.8,"latitude":38.4,"inService":{"enabled":true},"status":{"observed":{"primary":12.3,"primaryUnit":"ft","floodCategory":"Moderate","validTime":date(now-3600,"%Y-%m-%dT%H:%M:%SZ")},"forecast":{"primary":50.0,"primaryUnit":"ft","floodCategory":"Major","validTime":date(now+86400,"%Y-%m-%dT%H:%M:%SZ")}}}]});
        let rows = decode_gauges(&good, now).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].category, 2);
        // The official list schema does not expose inService; list observations are
        // qualified by their observed timestamp/category/unit, while detail cards require it.
        let mut without_service_field = good.clone();
        without_service_field["gauges"][0]
            .as_object_mut()
            .unwrap()
            .remove("inService");
        assert_eq!(decode_gauges(&without_service_field, now).unwrap().len(), 1);
        let mut no_flooding = good.clone();
        no_flooding["gauges"][0]["status"]["observed"]["floodCategory"] = json!("no_flooding");
        assert!(decode_gauges(&no_flooding, now).unwrap().is_empty());
        let mut stale = good.clone();
        stale["gauges"][0]["status"]["observed"]["validTime"] =
            json!(date(now - 7201, "%Y-%m-%dT%H:%M:%SZ"));
        assert!(decode_gauges(&stale, now).unwrap().is_empty());
        let mut future = good.clone();
        future["gauges"][0]["status"]["observed"]["floodCategory"] = json!("Major");
        future["gauges"][0]["status"]["observed"]["validTime"] =
            json!(date(now + 86400, "%Y-%m-%dT%H:%M:%SZ"));
        assert!(decode_gauges(&future, now).unwrap().is_empty());
        let mut unknown = good.clone();
        unknown["gauges"][0]["status"]["observed"]["floodCategory"] = json!("Record");
        assert!(decode_gauges(&unknown, now).unwrap().is_empty());
        let mut badunit = good.clone();
        badunit["gauges"][0]["status"]["observed"]["primaryUnit"] = json!("kcfs");
        assert!(decode_gauges(&badunit, now).unwrap().is_empty());
    }
    #[test]
    fn impacts_require_named_site_ft_unit_vertical_datum_and_safe_statement() {
        let meta = json!({"lid":"ABCD","name":"Town","description":"River near Town","longitude":-86.8,"latitude":38.4,"county":"Dubois","state":{"abbreviation":"IN"},"inService":{"enabled":true},"flood":{"stageUnits":"ft","impacts":[{"stage":20.0,"statement":"Park road floods."},{"stage":25.0,"statement":"Low homes may be affected."}]},"datums":{"vertical":{"value":[{"label":"NAVD88"}]}}});
        let home = GeoView {
            lon: -86.8,
            lat: 38.4,
            zoom: 7.,
        };
        let site = decode_impacts(&meta, "ABCD", -86.8, 38.4, home, 1_800_000_000).unwrap();
        assert_eq!(site.impacts.len(), 2);
        assert_eq!(
            site.datum,
            "Gauge-specific stage reference • listed vertical datum: NAVD88"
        );
        let mut many = meta.clone();
        many["flood"]["impacts"] = json!((0..8).map(|i| json!({"stage": 20.0 + f64::from(i), "statement": if i == 7 { "x".repeat(171) } else { format!("Published statement {i}") }})).collect::<Vec<_>>());
        let selected = decode_impacts(&many, "ABCD", -86.8, 38.4, home, 1_800_000_000).unwrap();
        assert_eq!(selected.source_entry_count, 8);
        assert_eq!(selected.impacts.len(), 7);
        let values = impact_values(Some(&selected), 1_800_000_000);
        assert!(values["note"].contains("6 selected statements from 8 source entries"));
        let stale_values = impact_values(Some(&selected), 1_800_001_201);
        assert!(stale_values["row0_stage"].is_empty());
        assert!(stale_values["row0_text"].is_empty());
        let future_values = impact_values(Some(&selected), 1_799_999_999);
        assert!(future_values["row0_stage"].is_empty());
        let mut no_datum = meta.clone();
        no_datum["datums"]["vertical"]["value"] = json!([]);
        let no_datum_site =
            decode_impacts(&no_datum, "ABCD", -86.8, 38.4, home, 1_800_000_000).unwrap();
        assert!(no_datum_site.datum.contains("no vertical datum listed"));
        let mut wrong_unit = meta.clone();
        wrong_unit["flood"]["stageUnits"] = json!("kcfs");
        assert!(decode_impacts(&wrong_unit, "ABCD", -86.8, 38.4, home, 1_800_000_000).is_none());
        let mut mismatched = meta.clone();
        mismatched["lid"] = json!("OTHR");
        assert!(decode_impacts(&mismatched, "ABCD", -86.8, 38.4, home, 1_800_000_000).is_none());
    }
    #[test]
    fn source_fixture_is_explicitly_not_live() {
        let mut doc = super::build("set_nwps_flood_map__county").unwrap();
        doc.template = Some("set_nwps_flood_map__county".into());
        sample(&mut doc);
        assert!(doc
            .data_overrides
            .iter()
            .any(|(_, v)| v.contains("SAMPLE ONLY")));
        assert!(doc
            .data_overrides
            .iter()
            .any(|(_, v)| v.contains("Not for broadcast")));
    }

    #[test]
    fn every_catalog_key_builds_with_its_own_identity_and_bounded_sample_categories() {
        let mut identities = HashSet::new();
        for key in KEYS {
            let mut doc = build(key).expect("catalog key should build");
            assert_eq!(doc.template.as_deref(), Some(key));
            assert!(identities.insert(doc.template.clone().unwrap()));
            sample(&mut doc);
            if let Some((_, payload)) = doc
                .data_overrides
                .iter()
                .find(|(k, _)| k.ends_with(".payload"))
            {
                let scene = board::parse(payload, Layout::Single).expect("sample scene");
                for point in scene.panels.iter().flat_map(|p| p.points.iter().flatten()) {
                    assert!(point.category < CATEGORY_LABELS.len());
                }
            }
        }
    }
}
