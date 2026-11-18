//! WPC quantitative precipitation polygons: deterministic amounts, not rain probability.
use super::{
    board::Scope,
    observed::{self, Coverage as LocalCoverage},
    qpf_canvas::{Payload, Polygon},
};
use crate::{
    config::{GeoView, Settings},
    gfx::{
        bind::Bindings,
        model::{AnimKind, Fill, GraphicDoc, Layer},
        text::Align,
    },
};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Coverage {
    Region,
    State,
    Multi,
    National,
}
impl Coverage {
    fn key(self) -> &'static str {
        match self {
            Self::Region => "region",
            Self::State => "state",
            Self::Multi => "multi",
            Self::National => "national",
        }
    }
    fn scope(self, settings: &Settings) -> Scope {
        match self {
            Self::Region => observed::report_scope(settings, LocalCoverage::Region),
            Self::State => observed::report_scope(settings, LocalCoverage::State),
            Self::Multi => observed::report_scope(settings, LocalCoverage::Multi),
            Self::National => Scope {
                states: CONUS_FIPS.iter().map(|x| (*x).to_string()).collect(),
                county: None,
                bbox: None,
            },
        }
    }
}
#[derive(Clone, Copy)]
struct Period {
    id: u8,
    key: &'static str,
    title: &'static str,
    product: &'static str,
    hours: i64,
    offset: i64,
}
const PERIODS: [Period; 7] = [
    Period {
        id: 1,
        key: "day1",
        title: "Day 1",
        product: "24-hour QPF",
        hours: 24,
        offset: 0,
    },
    Period {
        id: 2,
        key: "day2",
        title: "Day 2",
        product: "24-hour QPF",
        hours: 24,
        offset: 24,
    },
    Period {
        id: 3,
        key: "day3",
        title: "Day 3",
        product: "24-hour QPF",
        hours: 24,
        offset: 48,
    },
    Period {
        id: 4,
        key: "days4_5",
        title: "Days 4–5",
        product: "48-hour QPF",
        hours: 48,
        offset: 72,
    },
    Period {
        id: 9,
        key: "days1_3",
        title: "Days 1–3 cumulative",
        product: "72-hour QPF",
        hours: 72,
        offset: 0,
    },
    Period {
        id: 10,
        key: "days1_5",
        title: "Days 1–5 cumulative",
        product: "120-hour QPF",
        hours: 120,
        offset: 0,
    },
    Period {
        id: 11,
        key: "days1_7",
        title: "Days 1–7 cumulative",
        product: "168-hour QPF",
        hours: 168,
        offset: 0,
    },
];
// 48 states plus DC: National rows are explicitly contiguous-U.S. (CONUS).
const CONUS_FIPS: [&str; 49] = [
    "01", "04", "05", "06", "08", "09", "10", "11", "12", "13", "16", "17", "18", "19", "20", "21",
    "22", "23", "24", "25", "26", "27", "28", "29", "30", "31", "32", "33", "34", "35", "36", "37",
    "38", "39", "40", "41", "42", "44", "45", "46", "47", "48", "49", "50", "51", "53", "54", "55",
    "56",
];
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    period: u8,
    coverage: Coverage,
    format: u8,
}
include!("qpf_specs.rs");

#[derive(Clone)]
pub(super) struct Product {
    layer: u8,
    issue: i64,
    start: i64,
    end: i64,
    checked: i64,
    valid: String,
    polygons: Arc<Vec<Polygon>>,
}
impl Product {
    fn fresh(&self, now: i64) -> bool {
        (0..=1200).contains(&(now - self.checked))
            && (0..=30 * 3600).contains(&(now - self.issue))
            && self.start < self.end
            && self.end > now
    }
}
#[derive(Clone)]
struct Snapshot {
    home: [u64; 2],
    generation: u64,
    checked: i64,
    products: BTreeMap<u8, Arc<Product>>,
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
fn period(id: u8) -> Option<Period> {
    PERIODS.iter().copied().find(|p| p.id == id)
}
fn spec(key: &str) -> Option<Spec> {
    SPECS.iter().copied().find(|s| s.key == key)
}
fn prefix(s: Spec) -> String {
    format!(
        "wpcqpf.{}.{}",
        period(s.period).expect("registered QPF period").key,
        s.coverage.key()
    )
}

fn parse_endpoint(raw: &str) -> Option<i64> {
    let mut p = raw.split_whitespace();
    let time = p.next()?.strip_suffix('Z')?;
    let date = p.next()?;
    if p.next().is_some() || time.len() != 2 || !time.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let date = NaiveDate::parse_from_str(date, "%m/%d/%y").ok()?;
    let hour = time.parse::<u32>().ok()?;
    let time = NaiveTime::from_hms_opt(hour, 0, 0)?;
    Some(NaiveDateTime::new(date, time).and_utc().timestamp())
}
fn parse_valid(raw: &str) -> Option<(i64, i64)> {
    let (a, b) = raw.split_once(" - ")?;
    Some((parse_endpoint(a)?, parse_endpoint(b)?))
}
fn parse_naive_utc(raw: &str) -> Option<i64> {
    Some(
        NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
            .ok()?
            .and_utc()
            .timestamp(),
    )
}
fn parse(value: &Value, p: Period, now: i64) -> Option<Product> {
    if value.get("exceededTransferLimit").and_then(Value::as_bool) == Some(true)
        || value.get("spatialReference")?.get("wkid")?.as_i64()? != 4326
    {
        return None;
    }
    let fs = value.get("features")?.as_array()?;
    if fs.is_empty() || fs.len() > 2000 {
        return None;
    }
    let mut ids = BTreeSet::new();
    let mut identity = None;
    let mut polygons = Vec::new();
    let mut vertices = 0usize;
    for f in fs {
        let a = f.get("attributes")?;
        let id = a.get("objectid")?.as_i64()?;
        if id < 1 || !ids.insert(id) {
            return None;
        }
        let name = a.get("product")?.as_str()?;
        if !name.eq_ignore_ascii_case(p.product) {
            return None;
        }
        let valid = a.get("valid_time")?.as_str()?.trim();
        let (start, end) = parse_valid(valid)?;
        let issue = parse_naive_utc(a.get("issue_time")?.as_str()?)?;
        let start_field = parse_naive_utc(a.get("start_time")?.as_str()?)?;
        let end_field = parse_naive_utc(a.get("end_time")?.as_str()?)?;
        if start_field != start
            || end_field != end
            || end - start != p.hours * 3600
            || !(0..=30 * 3600).contains(&(now - issue))
            || end <= now
            || end > now + 8 * 86400
        {
            return None;
        }
        let unit = a.get("units")?.as_str()?;
        if !unit.eq_ignore_ascii_case("inches") {
            return None;
        }
        let qpf = a.get("qpf")?.as_f64()? as f32;
        if !qpf.is_finite() || qpf < 0. || qpf > 100. {
            return None;
        }
        let cycle_start = start - p.offset * 3600;
        if !(now - 30 * 3600..=now + 24 * 3600).contains(&cycle_start) {
            return None;
        }
        let ident = (issue, start, end, cycle_start, valid.to_string());
        if identity.as_ref().is_some_and(|old| old != &ident) {
            return None;
        }
        identity = Some(ident);

        let input = f.get("geometry")?.get("rings")?.as_array()?;
        if input.is_empty() || input.len() > 4096 {
            return None;
        }
        let mut rings = Vec::with_capacity(input.len());
        for r in input {
            let points = r.as_array()?;
            if points.len() < 4 || points.len() > 100_000 {
                return None;
            }
            vertices = vertices.checked_add(points.len())?;
            if vertices > 500_000 {
                return None;
            }
            let mut ring = Vec::with_capacity(points.len());
            for pt in points {
                let xy = pt.as_array()?;
                if xy.len() != 2 {
                    return None;
                }
                let lon = xy[0].as_f64()?;
                let lat = xy[1].as_f64()?;
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
                .map(|v| v[0].0 * v[1].1 - v[1].0 * v[0].1)
                .sum();
            if area.abs() < 1e-10 {
                return None;
            }
            rings.push(ring);
        }
        polygons.push(Polygon { qpf, rings });
    }
    let (issue, start, end, _cycle_start, valid) = identity?;
    polygons.sort_by(|a, b| a.qpf.total_cmp(&b.qpf));
    Some(Product {
        layer: p.id,
        issue,
        start,
        end,
        checked: now,
        valid,
        polygons: Arc::new(polygons),
    })
}
fn url(layer: u8) -> String {
    format!(
        "https://mapservices.weather.noaa.gov/vector/rest/services/precip/wpc_qpf/MapServer/{layer}/query?where=1%3D1&outFields=objectid,product,valid_time,qpf,units,issue_time,start_time,end_time&returnGeometry=true&outSR=4326&f=json"
    )
}
async fn get_period(p: Period, now: i64) -> Option<Product> {
    let v = super::report_feed::get_json(&url(p.id)).await?;
    parse(&v, p, now)
}
async fn fetch(home: [u64; 2], generation: u64) -> Option<Snapshot> {
    let now = chrono::Utc::now().timestamp();
    let mut products = BTreeMap::new();
    let mut pending: VecDeque<Period> = PERIODS.into_iter().collect();
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..2 {
        if let Some(p) = pending.pop_front() {
            set.spawn(async move { (p.id, get_period(p, now).await) });
        }
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        tokio::select! {
            _=tokio::time::sleep_until(deadline)=>break,
            item=set.join_next(),if !set.is_empty()=>{
                if let Some(Ok((id,Some(p))))=item { products.insert(id,p); }
                if let Some(p)=pending.pop_front(){set.spawn(async move{(p.id,get_period(p,now).await)});}
            }
            else=>break,
        }
    }
    set.abort_all();
    if products.is_empty() {
        return None;
    }
    let products = products
        .into_iter()
        .map(|(id, product)| (id, Arc::new(product)))
        .collect();
    Some(Snapshot {
        home,
        generation,
        checked: now,
        products,
    })
}
pub(super) async fn refresh(home_view: GeoView) {
    let ticket = if let Ok(mut s) = store().lock() {
        s.generation = s.generation.wrapping_add(1);
        s.current = None;
        s.generation
    } else {
        return;
    };
    let home = home_key(home_view);
    let result = tokio::time::timeout(Duration::from_secs(61), fetch(home, ticket))
        .await
        .ok()
        .flatten();
    if let Ok(mut s) = store().lock() {
        if s.generation == ticket {
            s.current = result.map(Arc::new);
        }
    }
}
fn scope(s: Spec, settings: &Settings) -> Scope {
    s.coverage.scope(settings)
}
fn canvas_payload(
    settings: &Settings,
    s: Spec,
    generation: u64,
    product: Option<&Product>,
    sample: bool,
) -> Payload {
    let p = period(s.period).expect("registered QPF period");
    let title = if s.coverage == Coverage::National {
        format!("WPC {} QPF • CONUS", p.title)
    } else {
        format!("WPC {} QPF", p.title)
    };
    let product = product.filter(|p| p.layer == s.period);
    Payload {
        scope: scope(s, settings),
        title,
        valid: product.map(|p| p.valid.clone()).unwrap_or_default(),
        issue: product
            .and_then(|p| chrono::DateTime::from_timestamp(p.issue, 0))
            .map(|d| d.format("%Y-%m-%d %H:%MZ").to_string())
            .unwrap_or_default(),
        home: home_key(settings.home),
        generation,
        layer: s.period,
        sample,
        sample_polygons: None,
    }
}
pub(super) fn live_polygons(
    home: [u64; 2],
    generation: u64,
    layer: u8,
) -> Option<Arc<Vec<Polygon>>> {
    let now = chrono::Utc::now().timestamp();
    let guard = store().lock().ok()?;
    if guard.generation != generation {
        return None;
    }
    let snapshot = guard.current.as_ref()?;
    if snapshot.home != home
        || snapshot.generation != generation
        || !(0..=1200).contains(&(now - snapshot.checked))
    {
        return None;
    }
    let product = snapshot.products.get(&layer)?;
    product.fresh(now).then(|| Arc::clone(&product.polygons))
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let snapshot = store().lock().ok().and_then(|s| {
        s.current
            .as_ref()
            .filter(|v| {
                v.home == home_key(settings.home)
                    && s.generation == v.generation
                    && (0..=1200).contains(&(now - v.checked))
            })
            .cloned()
    });
    let generation = snapshot.as_ref().map(|s| s.generation).unwrap_or(0);
    for forecast_period in PERIODS {
        for coverage in [
            Coverage::Region,
            Coverage::State,
            Coverage::Multi,
            Coverage::National,
        ] {
            let Some(spec) = SPECS
                .iter()
                .find(|s| s.period == forecast_period.id && s.coverage == coverage)
                .copied()
            else {
                continue;
            };
            let product = snapshot
                .as_ref()
                .and_then(|x| x.products.get(&spec.period))
                .filter(|p| p.fresh(now));
            let payload =
                canvas_payload(settings, spec, generation, product.map(Arc::as_ref), false);
            let p = prefix(spec);
            let subtitle = period(spec.period).map(|p| p.title).unwrap_or("WPC QPF");
            for (k,v) in [
            ("payload",serde_json::to_string(&payload).unwrap_or_default()),
            ("subtitle",subtitle.to_string()),
            ("source","Source: NOAA / National Weather Service Weather Prediction Center".to_string()),
            ("note","Unshaded: below 0.01 in or outside mapped contours • Not an all-clear. QPF is liquid-equivalent forecast guidance.".to_string()),
        ] { b.set(&format!("set.{p}.{k}"),&v); }
        }
    }
}
fn canvas_layer(p: &str, x: f32, y: f32, w: f32, h: f32, name: &str) -> Layer {
    Layer::risk_map(
        name,
        x,
        y,
        w,
        h,
        &format!("set:qpfcanvas:{{{{set.{p}.payload}}}}"),
    )
}
fn template(s: Spec) -> GraphicDoc {
    let p = prefix(s);
    let period = period(s.period).expect("valid QPF spec");
    let title = if s.coverage == Coverage::National {
        format!("WPC {} QPF • CONUS", period.title)
    } else {
        format!("WPC {} QPF", period.title)
    };
    match s.format {
        3 => {
            let mut d = GraphicDoc::new(&title, "Precipitation", None);
            d.layers.push(
                Layer::rect(
                    "QPF overlay card",
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
                    "QPF overlay accent",
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
                canvas_layer(&p, 82., 698., 1132., 240., "WPC QPF polygon overlay").with_anim(
                    AnimKind::Fade,
                    0.18,
                    0.4,
                ),
            );
            super::reference::label(
                &mut d,
                "QPF overlay subtitle",
                (86., 939., 1120., 24.),
                &format!("{{{{set.{p}.subtitle}}}}"),
                19.,
                "#C7D4E0",
                Align::Left,
            );
            super::reference::label(
                &mut d,
                "QPF overlay source",
                (86., 964., 1120., 22.),
                &format!("{{{{set.{p}.source}}}}"),
                17.,
                "#D5E1EC",
                Align::Left,
            );
            super::reference::label(
                &mut d,
                "QPF overlay caveat",
                (86., 988., 1120., 31.),
                &format!("{{{{set.{p}.note}}}}"),
                15.,
                "#B9C9D8",
                Align::Left,
            );
            super::reference::label(
                &mut d,
                "Station branding",
                (914., 677., 296., 20.),
                "{{station}}",
                15.,
                "$text",
                Align::Right,
            );
            d
        }
        1 => {
            let mut d = super::reference::social_frame(
                &title,
                "Precipitation",
                &format!("{{{{set.{p}.subtitle}}}}"),
                &format!("{{{{set.{p}.source}}}}"),
                true,
            );
            d.layers.push(
                canvas_layer(&p, 54., 242., 972., 1400., "Portrait WPC QPF").with_anim(
                    AnimKind::Fade,
                    0.2,
                    0.6,
                ),
            );
            super::reference::label(
                &mut d,
                "QPF map context",
                (54., 1656., 972., 68.),
                &format!("{{{{set.{p}.note}}}}"),
                24.,
                "$muted",
                Align::Left,
            );
            d
        }
        2 => {
            let mut d = super::reference::social_frame(
                &title,
                "Precipitation",
                &format!("{{{{set.{p}.subtitle}}}}"),
                &format!("{{{{set.{p}.source}}}}"),
                false,
            );
            d.layers.push(
                canvas_layer(&p, 54., 242., 972., 650., "Square WPC QPF").with_anim(
                    AnimKind::Fade,
                    0.2,
                    0.6,
                ),
            );
            super::reference::label(
                &mut d,
                "QPF map context",
                (54., 902., 972., 64.),
                &format!("{{{{set.{p}.note}}}}"),
                22.,
                "$muted",
                Align::Left,
            );
            d
        }
        _ => {
            let mut d = super::reference::frame(
                &title,
                "Precipitation",
                &format!("{{{{set.{p}.subtitle}}}}"),
                &format!("{{{{set.{p}.source}}}}"),
            );
            d.layers.push(
                canvas_layer(&p, 70., 205., 1780., 715., "Broadcast WPC QPF").with_anim(
                    AnimKind::Fade,
                    0.2,
                    0.6,
                ),
            );
            super::reference::label(
                &mut d,
                "QPF map context",
                (80., 925., 1760., 42.),
                &format!("{{{{set.{p}.note}}}}"),
                23.,
                "$muted",
                Align::Left,
            );
            d
        }
    }
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = spec(key)?;
    Some(template(s))
}
#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(s) = doc.template.as_deref().and_then(spec) else {
        return;
    };
    let p = period(s.period).unwrap();
    let settings = Settings::default();
    let now = chrono::Utc::now();
    let valid = format!(
        "{} - {}",
        now.format("%HZ %m/%d/%y"),
        (now + chrono::Duration::hours(p.hours)).format("%HZ %m/%d/%y")
    );
    let mut polygons: Vec<Polygon> = (0..36)
        .map(|i| {
            let x = -126. + i as f64 * 1.8;
            Polygon {
                qpf: crate::grid::RAIN_TOTAL_STOPS[i % 8].0,
                rings: vec![vec![
                    (x, 25.),
                    (x + 3., 25.),
                    (x + 3., 49.),
                    (x, 49.),
                    (x, 25.),
                ]],
            }
        })
        .collect();
    polygons.sort_by(|a, b| a.qpf.total_cmp(&b.qpf));
    let mut payload = canvas_payload(&settings, s, 0, None, true);
    payload.valid = format!("SAMPLE • {valid}");
    payload.issue = "SAMPLE • synthetic cycle".into();
    payload.sample_polygons = Some(polygons);
    let pfx = prefix(s);
    for (k, v) in [
        ("payload", serde_json::to_string(&payload).unwrap()),
        ("subtitle", format!("SAMPLE • {}", p.title)),
        ("source", "SAMPLE ONLY • Not for broadcast".to_string()),
        (
            "note",
            "SAMPLE • Synthetic polygon geometry and values • Not a forecast".to_string(),
        ),
    ] {
        doc.data_overrides.push((format!("set.{pfx}.{k}"), v));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture(p: Period, now: i64) -> Value {
        let start = now - now.rem_euclid(86400) + 86400 + p.offset * 3600;
        let end = start + p.hours * 3600;
        let fmt = |t: i64| chrono::DateTime::from_timestamp(t, 0).unwrap();
        let valid = format!(
            "{} - {}",
            fmt(start).format("%HZ %m/%d/%y"),
            fmt(end).format("%HZ %m/%d/%y")
        );
        let na = |t: i64| fmt(t).format("%Y-%m-%d %H:%M:%S").to_string();
        json!({"spatialReference":{"wkid":4326},"features":[{"attributes":{"objectid":1,"product":p.product,"valid_time":valid,"qpf":0.5,"units":"Inches","issue_time":na(now),"start_time":na(start),"end_time":na(end)},"geometry":{"rings":[[[-90.,30.],[-80.,30.],[-80.,40.],[-90.,40.],[-90.,30.]], [[-88.,32.],[-88.,35.],[-85.,35.],[-85.,32.],[-88.,32.]]]}}]})
    }
    #[test]
    fn period_units_utc_cycle_and_geometry_are_strict() {
        let now = 1_794_571_200;
        let sample_range = parse_valid("00Z 03/02/26 - 00Z 03/05/26").unwrap();
        assert_eq!(sample_range.1 - sample_range.0, 72 * 3600);
        let p = period(9).unwrap();
        assert!(parse(&fixture(p, now), p, now).is_some());
        let mut overlapping = fixture(p, now);
        let mut lower = overlapping["features"][0].clone();
        lower["attributes"]["objectid"] = json!(2);
        lower["attributes"]["qpf"] = json!(0.01);
        overlapping["features"].as_array_mut().unwrap().push(lower);
        let ordered = parse(&overlapping, p, now).unwrap();
        assert_eq!(ordered.polygons[0].qpf, 0.01);
        assert_eq!(ordered.polygons[1].qpf, 0.5);
        let mut bad = fixture(p, now);
        bad["features"][0]["attributes"]["units"] = json!("mm");
        assert!(parse(&bad, p, now).is_none());
        bad = fixture(p, now);
        bad["exceededTransferLimit"] = json!(true);
        assert!(parse(&bad, p, now).is_none());
        bad = fixture(p, now);
        bad["features"][0]["attributes"]["issue_time"] = json!(
            chrono::DateTime::from_timestamp(now + 301, 0)
                .unwrap()
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        );
        assert!(parse(&bad, p, now).is_none());
        bad = fixture(p, now);
        bad["features"][0]["attributes"]["valid_time"] = json!("00 03/02/26 - 00Z 03/05/26");
        assert!(parse(&bad, p, now).is_none());
        bad = fixture(p, now);
        bad["features"][0]["geometry"]["rings"][0][1][0] = json!(200.);
        assert!(parse(&bad, p, now).is_none());
        bad = fixture(p, now);
        bad["features"][0]["geometry"]["rings"][1][0] =
            bad["features"][0]["geometry"]["rings"][1][1].clone();
        assert!(parse(&bad, p, now).is_none());
        let p1 = period(1).unwrap();
        assert!(parse(&fixture(p, now), p1, now).is_none());
        assert_eq!(p.hours, 72);
        assert_eq!(
            parse(&fixture(p, now), p, now).unwrap().polygons[0]
                .rings
                .len(),
            2
        );
        let p1 = period(1).unwrap();
        let p2 = period(2).unwrap();
        let a = parse(&fixture(p1, now), p1, now).unwrap();
        let mut shifted = fixture(p2, now);
        let start = now - now.rem_euclid(86400) + 86400 + p2.offset * 3600 + 3600;
        let end = start + p2.hours * 3600;
        let fmt = |t: i64| chrono::DateTime::from_timestamp(t, 0).unwrap();
        shifted["features"][0]["attributes"]["valid_time"] = json!(format!(
            "{} - {}",
            fmt(start).format("%HZ %m/%d/%y"),
            fmt(end).format("%HZ %m/%d/%y")
        ));
        shifted["features"][0]["attributes"]["start_time"] =
            json!(fmt(start).format("%Y-%m-%d %H:%M:%S").to_string());
        shifted["features"][0]["attributes"]["end_time"] =
            json!(fmt(end).format("%Y-%m-%d %H:%M:%S").to_string());
        let b = parse(&shifted, p2, now).unwrap();
        assert_ne!(a.start, b.start);
        let independent: BTreeMap<_, _> = [(a.layer, a), (b.layer, b)].into_iter().collect();
        assert_eq!(independent.len(), 2);
        assert_ne!(independent[&p1.id].start, independent[&p2.id].start);

        let durations = PERIODS.map(|p| (p.id, p.hours));
        assert_eq!(
            durations,
            [
                (1, 24),
                (2, 24),
                (3, 24),
                (4, 48),
                (9, 72),
                (10, 120),
                (11, 168)
            ]
        );

        let mut recent = parse(&fixture(p, now), p, now).unwrap();
        assert!(recent.fresh(now + 1200));
        assert!(!recent.fresh(now + 1201));
        recent.checked = now + 30 * 3600;
        assert!(!recent.fresh(now + 30 * 3600 + 1));
        recent.checked = now;
        recent.issue = now + 301;
        assert!(!recent.fresh(now));
        recent.issue = now;
        recent.end = now;
        assert!(!recent.fresh(now));
    }
}
