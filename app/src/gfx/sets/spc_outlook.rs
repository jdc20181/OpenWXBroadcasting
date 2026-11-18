//! Graphics-local maps for official SPC Day 1/2 tornado, hail, and wind probabilities.
use super::{
    arcgis_geometry::{self, Rings},
    board::{self, Band, Layout, Scene},
    observed::{self, Coverage},
    report_feed,
};
use crate::{
    config::{GeoView, Settings},
    gfx::{bind::Bindings, model::GraphicDoc},
};
use futures_util::{StreamExt, stream};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Hazard {
    Tornado,
    Hail,
    Wind,
}
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    hazard: Hazard,
    day: u8,
    coverage: Coverage,
    national: bool,
    format: u8,
}
include!("spc_outlook_specs.rs");

struct Area {
    band: usize,
    rings: Rings,
}
struct Product {
    issue: i64,
    start: i64,
    end: i64,
    areas: Vec<Area>,
    values: BTreeMap<String, usize>,
}
struct Snapshot {
    home: [u64; 2],
    checked: i64,
    products: BTreeMap<(u8, Hazard), Product>,
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

fn compact_time(v: &Value) -> Option<i64> {
    let s = v.as_str()?;
    if s.len() != 12 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    chrono::NaiveDateTime::parse_from_str(s, "%Y%m%d%H%M")
        .ok()
        .map(|d| d.and_utc().timestamp())
}
fn legend(h: Hazard, day: u8) -> Vec<(i64, &'static str, &'static str)> {
    // Exact DN classes and labels from the SPC renderer, with crisp saturated category swatches.
    match h {
        Hazard::Tornado => vec![
            (2, "2%", "#1A731D"),
            (5, "5%", "#7F3F27"),
            (10, "10%", "#FFC800"),
            (15, "15%", "#FF0000"),
            (30, "30%", "#FF00FF"),
            (45, "45%", "#912CEE"),
            (60, "60%", "#104E8B"),
        ],
        Hazard::Hail => vec![
            (5, "5%", "#7F3F27"),
            (15, "15%", "#FFC800"),
            (30, "30%", "#FF0000"),
            (45, "45%", "#FF00FF"),
            (60, "60%", "#912CEE"),
        ],
        Hazard::Wind if day == 1 => vec![
            (5, "5%", "#7F3F27"),
            (15, "15%", "#FFC800"),
            (30, "30%", "#FF0000"),
            (45, "45%", "#FF00FF"),
            (60, "60%", "#912CEE"),
            (75, "75%", "#104E8B"),
            (90, "90%", "#00FFFF"),
        ],
        Hazard::Wind => vec![
            (5, "5%", "#7F3F27"),
            (15, "15%", "#FFC800"),
            (30, "30%", "#FF0000"),
            (45, "45%", "#FF00FF"),
            (60, "60%", "#912CEE"),
            (75, "75%", "#104E8B"),
            (90, "90%", "#00FFFF"),
        ],
    }
}
fn layer(day: u8, h: Hazard) -> Option<u8> {
    Some(match (day, h) {
        (1, Hazard::Tornado) => 3,
        (1, Hazard::Hail) => 5,
        (1, Hazard::Wind) => 7,
        (2, Hazard::Tornado) => 11,
        (2, Hazard::Hail) => 13,
        (2, Hazard::Wind) => 15,
        _ => return None,
    })
}
fn layer_url(day: u8, h: Hazard) -> Option<String> {
    let id = layer(day, h)?;
    Some(format!(
        "https://mapservices.weather.noaa.gov/vector/rest/services/outlooks/SPC_wx_outlks/MapServer/{id}/query?where=1%3D1&outFields=objectid,dn,valid,expire,idp_filedate,issue&returnGeometry=true&outSR=4326&f=json"
    ))
}
fn parse(value: &Value, day: u8, h: Hazard, now: i64) -> Option<Product> {
    if ![1, 2].contains(&day)
        || value.get("exceededTransferLimit").and_then(Value::as_bool) == Some(true)
        || value.get("spatialReference")?.get("wkid")?.as_i64()? != 4326
    {
        return None;
    }
    let features = value.get("features")?.as_array()?;
    if features.is_empty() || features.len() > 500 {
        return None;
    }
    let allowed: BTreeMap<i64, usize> = legend(h, day)
        .iter()
        .enumerate()
        .map(|(i, (dn, _, _))| (*dn, i))
        .collect();
    let mut dates = None;
    let mut issue_text: Option<String> = None;
    let mut ids = BTreeSet::new();
    let mut vertices = 0usize;
    let mut areas = Vec::new();
    for f in features {
        let a = f.get("attributes")?;
        let id = a.get("objectid")?.as_i64()?;
        if id < 1 || !ids.insert(id) {
            return None;
        }
        // ArcGIS esriFieldTypeDate values are epoch milliseconds. Never reinterpret
        // a seconds-valued payload as a valid issue cycle.
        let issue_ms = a.get("idp_filedate")?.as_i64()?;
        if !(1_000_000_000_000..=4_102_444_800_000).contains(&issue_ms) {
            return None;
        }
        let issue = issue_ms / 1000;
        let start = compact_time(a.get("valid")?)?;
        let end = compact_time(a.get("expire")?)?;
        let duration = end.checked_sub(start)?;
        if !(0..=30 * 3600).contains(&(now - issue))
            || end <= now
            || (day == 1 && !(1..=24 * 3600).contains(&duration))
            || (day == 2 && duration != 24 * 3600)
            || !(0..=72 * 3600).contains(&(end - now))
        {
            return None;
        }
        let issue_date = chrono::DateTime::from_timestamp(issue, 0)?.date_naive();
        let start_date = chrono::DateTime::from_timestamp(start, 0)?.date_naive();
        if (start_date - issue_date).num_days() != i64::from(day - 1) {
            return None;
        }
        let issue_text_now = a.get("issue")?.as_str()?.trim();
        if issue_text_now.is_empty()
            || issue_text_now.len() > 32
            || issue_text
                .as_deref()
                .is_some_and(|old| old != issue_text_now)
        {
            return None;
        }
        issue_text = Some(issue_text_now.to_string());
        let dn = a.get("dn")?.as_i64()?;
        let band = *allowed.get(&dn)?;
        let d = (issue, start, end);
        if dates.is_some_and(|old| old != d) {
            return None;
        }
        dates = Some(d);
        areas.push(Area {
            band,
            rings: arcgis_geometry::rings(f.get("geometry")?, &mut vertices)?,
        });
    }
    let (issue, start, end) = dates?;
    Some(Product {
        issue,
        start,
        end,
        areas,
        values: BTreeMap::new(),
    })
}
fn at(p: &Product, x: f64, y: f64) -> Option<usize> {
    let mut found = None;
    for a in &p.areas {
        if arcgis_geometry::contains(&a.rings, x, y)? {
            found = Some(found.map_or(a.band, |v: usize| v.max(a.band)));
        }
    }
    found
}
fn sample_county_centres(p: &mut Product, sites: &BTreeMap<String, (f64, f64)>) {
    p.values = sites
        .iter()
        .filter_map(|(id, &(x, y))| at(p, x, y).map(|v| (id.clone(), v)))
        .collect();
    p.areas.clear();
}
fn national_scope() -> board::Scope {
    board::Scope {
        states: Vec::new(),
        county: None,
        bbox: Some([-125., 24., -66., 50.]),
    }
}
fn scope(settings: &Settings, s: Spec) -> board::Scope {
    if s.national {
        national_scope()
    } else {
        observed::report_scope(settings, s.coverage)
    }
}
fn cov_key(c: Coverage) -> &'static str {
    match c {
        Coverage::County => "county",
        Coverage::Region => "region",
        Coverage::State => "state",
        Coverage::Multi => "multi",
    }
}
async fn fetch_product(
    day: u8,
    h: Hazard,
    sites: Arc<BTreeMap<String, (f64, f64)>>,
) -> (u8, Hazard, Option<Product>) {
    let Some(url) = layer_url(day, h) else {
        return (day, h, None);
    };
    let parsed = report_feed::get_json(&url)
        .await
        .and_then(|v| parse(&v, day, h, chrono::Utc::now().timestamp()))
        .map(|mut p| {
            sample_county_centres(&mut p, &sites);
            p
        });
    (day, h, parsed)
}
async fn fetch(home: GeoView) -> Option<Snapshot> {
    let all = crate::map::boundaries::counties_named()?;
    let mut settings = Settings::default();
    settings.home = home;
    let mut sites = BTreeMap::new();
    for s in [
        observed::report_scope(&settings, Coverage::Region),
        observed::report_scope(&settings, Coverage::State),
        observed::report_scope(&settings, Coverage::Multi),
        national_scope(),
    ] {
        for c in board::selected(&all, &s) {
            let (x, y) = board::centre(c);
            if board::inside(c, x, y) {
                sites.insert(c.fips.clone(), (x, y));
            }
        }
    }
    if sites.is_empty() {
        return None;
    }
    let sites = Arc::new(sites);
    let mut pending = stream::iter([
        (1, Hazard::Tornado),
        (1, Hazard::Hail),
        (1, Hazard::Wind),
        (2, Hazard::Tornado),
        (2, Hazard::Hail),
        (2, Hazard::Wind),
    ])
    .map(|(day, h)| {
        let sites = Arc::clone(&sites);
        async move { fetch_product(day, h, sites).await }
    })
    .buffer_unordered(2);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let mut products = BTreeMap::new();
    loop {
        match tokio::time::timeout_at(deadline, pending.next()).await {
            Ok(Some((day, h, Some(p)))) => {
                products.insert((day, h), p);
            }
            Ok(Some((_day, _h, None))) => {}
            Ok(None) | Err(_) => break,
        }
    }
    Some(Snapshot {
        home: home_key(home),
        checked: chrono::Utc::now().timestamp(),
        products,
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
            s.current = result.map(Arc::new)
        }
    }
}
fn hazard_title(h: Hazard) -> &'static str {
    match h {
        Hazard::Tornado => "Tornado",
        Hazard::Hail => "Hail",
        Hazard::Wind => "Wind",
    }
}
fn prefix(s: Spec) -> String {
    format!(
        "spcprob.{}.{}.{}",
        hazard_title(s.hazard).to_ascii_lowercase(),
        s.day,
        if s.national {
            "national"
        } else {
            cov_key(s.coverage)
        }
    )
}
fn scene(settings: &Settings, s: Spec, p: Option<&Product>) -> Scene {
    let mut out = board::empty(Layout::Single);
    out.scope = scope(settings, s);
    let bands = legend(s.hazard, s.day);
    out.legend_title = format!("SPC Day {} {} probability", s.day, hazard_title(s.hazard));
    out.legend = bands
        .iter()
        .map(|(_, l, c)| Band {
            label: (*l).into(),
            color: (*c).into(),
        })
        .collect();
    let panel = &mut out.panels[0];
    panel.title = format!("SPC Day {} • {} probability", s.day, hazard_title(s.hazard));
    panel.detail="Probability within 25 miles of a point • County-centre samples • Intensity guidance not shown".into();
    if s.national {
        panel.title = format!(
            "SPC Day {} • Contiguous U.S. {} probability",
            s.day,
            hazard_title(s.hazard)
        );
        panel.detail="Contiguous U.S. • Probability within 25 miles of a point • Intensity guidance not shown".into();
    }
    if let Some(p) = p {
        let stamp = |t| {
            chrono::DateTime::from_timestamp(t, 0)
                .map(|d| d.format("%b %d %H:%M UTC").to_string())
                .unwrap_or_default()
        };
        panel.valid = format!("{}–{}", stamp(p.start), stamp(p.end));
        let Some(all) = crate::map::boundaries::counties_named() else {
            return out;
        };
        let mut values = BTreeMap::new();
        for c in board::selected(&all, &out.scope) {
            if let Some(&v) = p.values.get(&c.fips) {
                values.insert(c.fips.clone(), v);
            }
        }
        panel.counties = Some(values);
        out.reviewed = true;
    }
    out
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let snap = store().lock().ok().and_then(|s| {
        s.current
            .as_ref()
            .filter(|v| {
                v.home == home_key(settings.home) && (0..=20 * 60).contains(&(now - v.checked))
            })
            .cloned()
    });
    for s in SPECS.iter().filter(|s| s.format == 0) {
        let p = prefix(*s);
        let product = snap
            .as_ref()
            .and_then(|v| v.products.get(&(s.day, s.hazard)))
            .filter(|v| v.end > now && (0..=30 * 3600).contains(&(now - v.issue)));
        let sc = scene(settings, *s, product);
        let source = "Source: NOAA / NWS Storm Prediction Center";
        let subtitle = product
            .and_then(|v| chrono::DateTime::from_timestamp(v.issue, 0))
            .map(|t| {
                format!(
                    "SPC file {} • Valid period verified from feature attributes",
                    t.format("%b %d %H:%M UTC")
                )
            })
            .unwrap_or_else(|| "SPC probability outlook unavailable".into());
        let note = if product.is_some() {
            "Probability within 25 miles of a point • County-centre samples • Intensity guidance not shown"
        } else {
            "Data unavailable or validity could not be confirmed • No other product substituted"
        };
        for (k, v) in [
            ("payload", serde_json::to_string(&sc).unwrap_or_default()),
            ("subtitle", subtitle),
            ("source", source.into()),
            ("note", note.into()),
        ] {
            b.set(&format!("set.{p}.{k}"), &v)
        }
    }
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|v| v.key == key)?;
    let p = prefix(s);
    let title = format!("SPC Day {} {} probability", s.day, hazard_title(s.hazard));
    Some(match s.format {
        1 => super::comparison::portrait_template(&title, "SPC outlooks", &p),
        2 => super::comparison::square_template(&title, "SPC outlooks", &p),
        3 => super::product_overlay::template(&title, "SPC outlooks", &p),
        _ => super::comparison::template(&title, "SPC outlooks", &p, Layout::Single),
    })
}
#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(s) = SPECS
        .iter()
        .find(|v| Some(v.key) == doc.template.as_deref())
    else {
        return;
    };
    let settings = Settings::default();
    let mut sc = scene(&settings, *s, None);
    let Some(all) = crate::map::boundaries::counties_named() else {
        return;
    };
    let mut vals = BTreeMap::new();
    for c in board::selected(&all, &sc.scope) {
        let (x, _y) = board::centre(c);
        let band = (((x - settings.home.lon) / 5.).floor() as i32).rem_euclid(5) as usize;
        if band > 0 {
            vals.insert(c.fips.clone(), band.min(sc.legend.len() - 1));
        }
    }
    sc.panels[0].counties = Some(vals);
    sc.panels[0].valid = "SAMPLE • Synthetic SPC-style categories • Not an outlook".into();
    sc.panels[0].detail = "SAMPLE • Synthetic county-centre categories • Not for broadcast".into();
    sc.panels[0].title = format!(
        "SAMPLE • SPC Day {} {} probability",
        s.day,
        hazard_title(s.hazard)
    );
    sc.reviewed = true;
    let p = prefix(*s);
    for (k, v) in [
        ("payload", serde_json::to_string(&sc).unwrap()),
        ("subtitle", "SAMPLE • Not a forecast".into()),
        ("source", "SAMPLE ONLY • Not for broadcast".into()),
        (
            "note",
            "SAMPLE • Synthetic county-centre categories • Not for broadcast".into(),
        ),
    ] {
        doc.data_overrides.push((format!("set.{p}.{k}"), v));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn time(s: &str) -> i64 {
        chrono::NaiveDateTime::parse_from_str(s, "%Y%m%d%H%M")
            .unwrap()
            .and_utc()
            .timestamp()
    }
    fn feature(id: i64, dn: i64, valid: &str, expire: &str, issue: &str, file_ms: i64) -> Value {
        json!({
            "attributes": {"objectid":id,"dn":dn,"valid":valid,"expire":expire,"issue":issue,"idp_filedate":file_ms},
            "geometry": {"rings":[[[-100.0,30.0],[-90.0,30.0],[-90.0,40.0],[-100.0,40.0],[-100.0,30.0]]],"spatialReference":{"wkid":4326}}
        })
    }
    fn fixture(day: u8, hazard: Hazard, start: &str, expire: &str, issue: &str, dn: i64) -> Value {
        let file_ms = time(issue) * 1000;
        let start_fmt = chrono::DateTime::from_timestamp(time(start), 0)
            .unwrap()
            .format("%Y%m%d%H%M")
            .to_string();
        let expire_fmt = chrono::DateTime::from_timestamp(time(expire), 0)
            .unwrap()
            .format("%Y%m%d%H%M")
            .to_string();
        let issue_text = chrono::DateTime::from_timestamp(time(issue), 0)
            .unwrap()
            .format("%Y-%m-%d %H:%MZ")
            .to_string();
        let layer = super::layer(day, hazard).unwrap();
        json!({"spatialReference":{"wkid":4326},"features":[feature(i64::from(layer)+1,dn,&start_fmt,&expire_fmt,&issue_text,file_ms)]})
    }
    fn compact(s: &str) -> String {
        chrono::DateTime::from_timestamp(time(s), 0)
            .unwrap()
            .format("%Y%m%d%H%M")
            .to_string()
    }

    #[test]
    fn accepts_day1_short_updated_windows_and_day2_exact_day() {
        let d1_01 = fixture(
            1,
            Hazard::Tornado,
            "202611141200",
            "202611142300",
            "202611140100",
            2,
        );
        assert!(parse(&d1_01, 1, Hazard::Tornado, time("202611141300")).is_some()); // 11 h
        let d1_20 = fixture(
            1,
            Hazard::Wind,
            "202611141200",
            "202611150400",
            "202611142000",
            5,
        );
        assert!(parse(&d1_20, 1, Hazard::Wind, time("202611142100")).is_some()); // 16 h
        let d2 = fixture(
            2,
            Hazard::Hail,
            "202611151200",
            "202611161200",
            "202611141700",
            5,
        );
        assert!(parse(&d2, 2, Hazard::Hail, time("202611150800")).is_some()); // exactly 24 h
        let mut short_d2 = d2.clone();
        short_d2["features"][0]["attributes"]["expire"] = json!(compact("202611160000"));
        assert!(parse(&short_d2, 2, Hazard::Hail, time("202611150800")).is_none());
    }

    #[test]
    fn rejects_stale_expired_wrong_day_wrong_units_and_unknown_dn() {
        let d2 = fixture(
            2,
            Hazard::Tornado,
            "202611151200",
            "202611161200",
            "202611141700",
            2,
        );
        assert!(parse(&d2, 2, Hazard::Tornado, time("202611160100")).is_none()); // stale issue
        assert!(parse(&d2, 2, Hazard::Tornado, time("202611171300")).is_none()); // expired product
        assert!(parse(&d2, 3, Hazard::Tornado, time("202611150800")).is_none());
        let mut seconds = d2.clone();
        seconds["features"][0]["attributes"]["idp_filedate"] = json!(time("202611141700"));
        assert!(parse(&seconds, 2, Hazard::Tornado, time("202611150800")).is_none());
        let mut unknown = d2.clone();
        unknown["features"][0]["attributes"]["dn"] = json!(99);
        assert!(parse(&unknown, 2, Hazard::Tornado, time("202611150800")).is_none());
        let mut wrong_sr = d2.clone();
        wrong_sr["spatialReference"]["wkid"] = json!(3857);
        assert!(parse(&wrong_sr, 2, Hazard::Tornado, time("202611150800")).is_none());
    }

    #[test]
    fn rejects_conflicting_issue_cycles_and_unbounded_issue_text() {
        let mut conflict = fixture(
            1,
            Hazard::Hail,
            "202611141200",
            "202611142300",
            "202611140100",
            5,
        );
        let mut second = conflict["features"][0].clone();
        second["attributes"]["objectid"] = json!(99);
        second["attributes"]["issue"] = json!("2026-11-14 06:00Z");
        conflict["features"].as_array_mut().unwrap().push(second);
        assert!(parse(&conflict, 1, Hazard::Hail, time("202611141300")).is_none());
        let mut long_issue = fixture(
            1,
            Hazard::Hail,
            "202611141200",
            "202611142300",
            "202611140100",
            5,
        );
        long_issue["features"][0]["attributes"]["issue"] = json!("i".repeat(33));
        assert!(parse(&long_issue, 1, Hazard::Hail, time("202611141300")).is_none());
        let mut conflicting_filedate = fixture(
            1,
            Hazard::Hail,
            "202611141200",
            "202611142300",
            "202611140100",
            5,
        );
        let mut second = conflicting_filedate["features"][0].clone();
        second["attributes"]["objectid"] = json!(100);
        second["attributes"]["idp_filedate"] = json!(time("202611140600") * 1000);
        conflicting_filedate["features"]
            .as_array_mut()
            .unwrap()
            .push(second);
        assert!(parse(&conflicting_filedate, 1, Hazard::Hail, time("202611141300")).is_none());
    }

    #[test]
    fn holes_boundaries_and_nested_probability_classes_are_conservative() {
        let mut p = parse(
            &fixture(
                1,
                Hazard::Tornado,
                "202611141200",
                "202611142300",
                "202611140100",
                2,
            ),
            1,
            Hazard::Tornado,
            time("202611141300"),
        )
        .unwrap();
        p.areas[0].rings.push(vec![
            (-97., 33.),
            (-93., 33.),
            (-93., 37.),
            (-97., 37.),
            (-97., 33.),
        ]);
        p.areas.push(Area {
            band: 3,
            rings: vec![vec![
                (-96., 34.),
                (-94., 34.),
                (-94., 36.),
                (-96., 36.),
                (-96., 34.),
            ]],
        });
        assert_eq!(at(&p, -105., 32.), None); // Outside all published contours
        assert_eq!(at(&p, -100., 35.), None); // boundary is ambiguous
        assert_eq!(at(&p, -95., 35.), Some(3)); // nested higher category takes precedence
        assert_eq!(at(&p, -98., 35.), Some(0));
    }

    #[test]
    fn rejects_feature_and_vertex_limit_overruns() {
        let mut many = fixture(
            1,
            Hazard::Tornado,
            "202611141200",
            "202611142300",
            "202611140100",
            2,
        );
        let base = many["features"][0].clone();
        for id in 1000..=1500 {
            let mut f = base.clone();
            f["attributes"]["objectid"] = json!(id);
            many["features"].as_array_mut().unwrap().push(f);
        }
        assert!(parse(&many, 1, Hazard::Tornado, time("202611141300")).is_none());
        let geom = fixture(
            1,
            Hazard::Tornado,
            "202611141200",
            "202611142300",
            "202611140100",
            2,
        )["features"][0]["geometry"]
            .clone();
        let mut vertices = 199_998;
        assert!(arcgis_geometry::rings(&geom, &mut vertices).is_none());
    }
}
