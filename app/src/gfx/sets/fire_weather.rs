//! Official SPC Day 1/2 wind and low-humidity fire outlooks, not fire warnings.
use super::{
    arcgis_geometry::{self, Rings},
    board::{self, Band, Layout, Scene},
    observed::{self, Coverage},
};
use crate::{
    config::{GeoView, Settings},
    gfx::{bind::Bindings, model::GraphicDoc},
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, OnceLock},
};
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    day: u8,
    coverage: Coverage,
    national: bool,
    format: u8,
}
include!("fire_weather_specs.rs");
struct Area {
    category: usize,
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
    products: BTreeMap<u8, Product>,
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
fn compact_time(v: &Value) -> Option<i64> {
    let s = v.as_str()?;
    if s.len() != 12 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    chrono::NaiveDateTime::parse_from_str(s, "%Y%m%d%H%M")
        .ok()
        .map(|d| d.and_utc().timestamp())
}
fn parse(value: &Value, day: u8, now: i64) -> Option<Product> {
    if ![1, 2].contains(&day)
        || value.get("exceededTransferLimit").and_then(Value::as_bool) == Some(true)
        || value.get("spatialReference")?.get("wkid")?.as_i64()? != 4326
    {
        return None;
    }
    let features = value.get("features")?.as_array()?;
    // An empty feature collection carries no issue/valid time: it cannot establish a clear outlook.
    if features.is_empty() || features.len() > 500 {
        return None;
    }
    let mut dates = None;
    let mut ids = BTreeSet::new();
    let mut vertices = 0;
    let mut areas = Vec::new();
    for f in features {
        let p = f.get("attributes")?;
        let id = p.get("objectid")?.as_i64()?;
        if id < 1 || !ids.insert(id) {
            return None;
        }
        let issue =
            chrono::DateTime::from_timestamp_millis(p.get("idp_filedate")?.as_i64()?)?.timestamp();
        let start = compact_time(p.get("valid")?)?;
        let end = compact_time(p.get("expire")?)?;
        if !(0..=30 * 3600).contains(&(now - issue))
            || end <= now
            || !(3600..=86400).contains(&(end - start))
            || (day == 2 && end - start != 86400)
            || !(0..=72 * 3600).contains(&(end - now))
        {
            return None;
        }
        let issue_date = chrono::DateTime::from_timestamp(issue, 0)?.date_naive();
        let start_date = chrono::DateTime::from_timestamp(start, 0)?.date_naive();
        if (start_date - issue_date).num_days() != i64::from(day - 1) {
            return None;
        }
        let d = (issue, start, end);
        if dates.is_some_and(|old| old != d) {
            return None;
        }
        dates = Some(d);
        let category = match p.get("dn")?.as_i64()? {
            5 => 0,
            8 => 1,
            10 => 2,
            _ => return None,
        };
        areas.push(Area {
            category,
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
fn at(product: &Product, x: f64, y: f64) -> Option<usize> {
    let mut found = None;
    for area in &product.areas {
        if arcgis_geometry::contains(&area.rings, x, y)? {
            found = Some(found.map_or(area.category, |v: usize| v.max(area.category)));
        }
    }
    found
}
fn sample_sites(product: &mut Product, sites: &BTreeMap<String, (f64, f64)>) {
    product.values = sites
        .iter()
        .filter_map(|(id, &(x, y))| at(product, x, y).map(|v| (id.clone(), v)))
        .collect();
    product.areas.clear();
}
fn url(day: u8) -> String {
    let layer = if day == 1 { 1 } else { 4 };
    format!("https://mapservices.weather.noaa.gov/vector/rest/services/fire_weather/SPC_firewx/MapServer/{layer}/query?where=1%3D1&outFields=objectid,dn,valid,expire,idp_filedate&returnGeometry=true&outSR=4326&f=json")
}
async fn fetch(home: GeoView) -> Option<Snapshot> {
    let all = crate::map::boundaries::counties_named()?;
    let mut settings = Settings::default();
    settings.home = home;
    let mut sites = BTreeMap::new();
    for scope in [
        observed::report_scope(&settings, Coverage::Region),
        observed::report_scope(&settings, Coverage::State),
        observed::report_scope(&settings, Coverage::Multi),
        national_scope(),
    ] {
        for c in board::selected(&all, &scope) {
            let (x, y) = board::centre(c);
            if board::inside(c, x, y) {
                sites.insert(c.fips.clone(), (x, y));
            }
        }
    }
    let sites = Arc::new(sites);
    let mut tasks = tokio::task::JoinSet::new();
    for day in [1, 2] {
        let sites = Arc::clone(&sites);
        tasks.spawn(async move {
            let product = super::report_feed::get_json(&url(day))
                .await
                .and_then(|v| parse(&v, day, chrono::Utc::now().timestamp()))
                .map(|mut p| {
                    sample_sites(&mut p, &sites);
                    p
                });
            (day, product)
        });
    }
    let mut products = BTreeMap::new();
    while let Some(result) = tasks.join_next().await {
        let (day, product) = result.ok()?;
        if let Some(p) = product {
            products.insert(day, p);
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
    let result = tokio::time::timeout(std::time::Duration::from_secs(50), fetch(home))
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
        "fireweather.{}.{}",
        s.day,
        if s.national {
            "national"
        } else {
            match s.coverage {
                Coverage::County => "county",
                Coverage::Region => "region",
                Coverage::State => "state",
                Coverage::Multi => "multi",
            }
        }
    )
}
fn national_scope() -> board::Scope {
    // SPC's fire outlook is for the contiguous U.S.; do not imply AK/HI coverage.
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
fn scene(settings: &Settings, s: Spec, product: Option<&Product>) -> Scene {
    let mut scene = board::empty(Layout::Single);
    scene.scope = scope(settings, s);
    scene.legend_title =
        "SPC wind / low humidity • County-centre categories • Blank = unclassified or unavailable"
            .into();
    scene.legend = [
        ("Elevated", "#E69800"),
        ("Critical", "#FF0000"),
        ("Extremely critical", "#E600A9"),
    ]
    .iter()
    .map(|(l, c)| Band {
        label: (*l).into(),
        color: (*c).into(),
    })
    .collect();
    let p = &mut scene.panels[0];
    p.title = format!("SPC Day {} • Fire weather outlook", s.day);
    p.detail = "Wind / low humidity categories • Not a fire warning or a burn ban".into();
    if s.national {
        p.title = format!("SPC Day {} • Contiguous U.S.", s.day);
        p.detail =
            "Contiguous U.S. only • Wind / low humidity • Not a fire warning or burn ban".into();
    }
    let Some(product) = product else {
        return scene;
    };
    let stamp = |t| {
        chrono::DateTime::from_timestamp(t, 0)
            .unwrap()
            .format("%b %d %H:%M UTC")
            .to_string()
    };
    p.valid = format!("{}–{}", stamp(product.start), stamp(product.end));
    let Some(all) = crate::map::boundaries::counties_named() else {
        return scene;
    };
    let mut values = BTreeMap::new();
    let mut rows = Vec::new();
    for c in board::selected(&all, &scene.scope) {
        if let Some(&v) = product.values.get(&c.fips) {
            values.insert(c.fips.clone(), v);
            rows.push((c, v));
        }
    }
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));
    p.summary_title = "Selected county centres • Highest categories".into();
    p.summary = Some(
        rows.iter()
            .take(7)
            .map(|(c, v)| {
                let state = crate::winter::STATES
                    .iter()
                    .find(|s| s.0 == c.state)
                    .map(|s| s.1)
                    .unwrap_or("US");
                (
                    format!("{}, {state}", c.name),
                    scene.legend[*v].label.clone(),
                )
            })
            .collect(),
    );
    // A valid national product can contain no classified centre in the selected area.
    // Retain neutral outlines; do not turn an uncovered point into a safe/zero category.
    p.counties = Some(values);
    scene.reviewed = true;
    scene
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
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
            .and_then(|snp| snp.products.get(&s.day))
            .filter(|p| p.end > now && (0..=30 * 3600).contains(&(now - p.issue)));
        let prefix = prefix(*s);
        let scene = scene(settings, *s, product);
        let subtitle = product
            .and_then(|p| chrono::DateTime::from_timestamp(p.issue, 0))
            .map(|t| format!("SPC file {}", t.format("%b %d %H:%M UTC")))
            .unwrap_or_else(|| "Published SPC wind / low humidity outlook".into());
        for (field, value) in [
            ("payload", serde_json::to_string(&scene).unwrap_or_default()),
            ("subtitle", subtitle),
            (
                "source",
                "Source: NOAA / NWS Storm Prediction Center".into(),
            ),
            (
                "note",
                "County-centre samples • Local conditions vary • Blank does not mean no fire risk"
                    .into(),
            ),
        ] {
            b.set(&format!("set.{prefix}.{field}"), &value);
        }
    }
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|s| s.key == key)?;
    let prefix = prefix(s);
    Some(match s.format {
        1 => super::comparison::portrait_template("Fire Weather Outlook", "Fire weather", &prefix),
        2 => super::comparison::square_template("Fire Weather Outlook", "Fire weather", &prefix),
        _ => super::comparison::template(
            "Fire Weather Outlook",
            "Fire weather",
            &prefix,
            Layout::Single,
        ),
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
    let now = chrono::Utc::now().timestamp();
    let start = chrono::Utc::now()
        .date_naive()
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp()
        + i64::from(s.day - 1) * 86400;
    let mut product = Product {
        issue: now,
        start,
        end: start + 86400,
        areas: Vec::new(),
        values: BTreeMap::new(),
    };
    let settings = Settings::default();
    if let Some(all) = crate::map::boundaries::counties_named() {
        for c in board::selected(&all, &scope(&settings, *s)) {
            let (lon, lat) = board::centre(c);
            let width = if s.national { 7. } else { 0.7 };
            let band = (((lon - settings.home.lon) / width).floor() as i32).rem_euclid(4);
            if band != 0 && board::inside(c, lon, lat) {
                product.values.insert(c.fips.clone(), (band - 1) as usize);
            }
        }
    }
    let mut scene = scene(&settings, *s, Some(&product));
    scene.panels[0].valid = format!("SAMPLE • {}", scene.panels[0].valid);
    for (field, value) in [
        ("payload", serde_json::to_string(&scene).unwrap()),
        ("subtitle", "SAMPLE • Wind / low humidity categories".into()),
        ("source", "SAMPLE ONLY • Not for broadcast".into()),
        (
            "note",
            "SAMPLE • Synthetic county-centre categories • Not a forecast".into(),
        ),
    ] {
        doc.data_overrides
            .push((format!("set.{}.{field}", prefix(*s)), value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn time(s: &str) -> i64 {
        compact_time(&json!(s)).unwrap()
    }
    fn fixture(day: u8) -> Value {
        json!({"spatialReference":{"wkid":4326},"features":[{"attributes":{"objectid":1,"dn":5,"idp_filedate":time("202611141000")*1000,"valid":if day==1 {"202611141200"} else {"202611151200"},"expire":if day==1 {"202611151200"} else {"202611161200"}},"geometry":{"rings":[[[-90.,30.],[-80.,30.],[-80.,45.],[-90.,45.],[-90.,30.]]]}}]})
    }
    #[test]
    fn source_contract_rejects_mixed_cycles_bad_categories_dates_and_partial_responses() {
        let now = time("202611141300");
        for day in [1, 2] {
            let v = fixture(day);
            assert!(parse(&v, day, now).is_some());
            for field in ["dn", "valid", "expire", "idp_filedate"] {
                let mut bad = v.clone();
                bad["features"][0]["attributes"][field] = json!("unknown");
                assert!(parse(&bad, day, now).is_none());
            }
            let mut bad = v.clone();
            bad["exceededTransferLimit"] = json!(true);
            assert!(parse(&bad, day, now).is_none());
            let mut bad = v.clone();
            bad["spatialReference"]["wkid"] = json!(3857);
            assert!(parse(&bad, day, now).is_none());
            let mut bad = v.clone();
            bad["features"][0]["attributes"]["dn"] = json!(6);
            assert!(parse(&bad, day, now).is_none());
            let mut bad = v.clone();
            let mut extra = bad["features"][0].clone();
            extra["attributes"]["objectid"] = json!(2);
            extra["attributes"]["idp_filedate"] = json!((time("202611141000") - 60) * 1000);
            bad["features"].as_array_mut().unwrap().push(extra);
            assert!(parse(&bad, day, now).is_none());
            assert!(parse(&v, day, now - 6 * 3600).is_none());
            assert!(parse(&v, day, now + 3 * 86400).is_none());
        }
        let mut update = fixture(1);
        update["features"][0]["attributes"]["valid"] = json!("202611141700");
        update["features"][0]["attributes"]["idp_filedate"] = json!(time("202611141700") * 1000);
        assert!(parse(&update, 1, time("202611141800")).is_some());
        assert!(compact_time(&json!("202602301200")).is_none());
        assert!(compact_time(&json!("202611141200Z")).is_none());
        assert!(url(1).contains("/MapServer/1/query?"));
        assert!(url(2).contains("/MapServer/4/query?"));
    }
    #[test]
    fn nested_categories_preserve_holes_withhold_edges_and_sample_once() {
        let now = time("202611141300");
        let mut p = parse(&fixture(1), 1, now).unwrap();
        p.areas.push(Area {
            category: 2,
            rings: vec![
                vec![
                    (-88., 32.),
                    (-82., 32.),
                    (-82., 42.),
                    (-88., 42.),
                    (-88., 32.),
                ],
                vec![
                    (-86., 35.),
                    (-84., 35.),
                    (-84., 39.),
                    (-86., 39.),
                    (-86., 35.),
                ],
            ],
        });
        assert_eq!(at(&p, -87., 34.), Some(2));
        assert_eq!(at(&p, -85., 37.), Some(0));
        assert_eq!(at(&p, -90., 35.), None);
        assert_eq!(at(&p, -95., 35.), None);
        let sites = BTreeMap::from([
            ("18001".into(), (-87., 34.)),
            ("18003".into(), (-85., 37.)),
            ("18005".into(), (-95., 35.)),
        ]);
        sample_sites(&mut p, &sites);
        assert!(p.areas.is_empty());
        assert_eq!(p.values.get("18001"), Some(&2));
        assert_eq!(p.values.get("18003"), Some(&0));
        assert!(!p.values.contains_key("18005"));
        let mut count = 0;
        assert!(
            arcgis_geometry::rings(&json!({"rings":[[[0,0],[1,0],[1,1],[0,1]]]}), &mut count)
                .is_none()
        );
        let mut count = 200000;
        assert!(
            arcgis_geometry::rings(&fixture(1)["features"][0]["geometry"], &mut count).is_none()
        );
    }
}
