//! Forecast crest cards with compatible published past crests. NOAA NWPS only.
use super::reference::{frame, label, lowerthird_frame, social_frame, MISSING};
use crate::{
    config::{GeoView, Settings},
    gfx::{
        bind::Bindings,
        model::{AnimKind, GraphicDoc},
        text::Align,
    },
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};

pub(super) const KEYS: [&str; 4] = [
    "set_river_crest",
    "set_river_crest__portrait",
    "set_river_crest__square",
    "set_river_crest__lowerthird",
];
#[derive(Clone, Debug, PartialEq)]
struct Past {
    time: i64,
    stage: f64,
    quality: String,
}
#[derive(Clone, Debug)]
struct Peak {
    stage: f64,
    first: i64,
    last: i64,
}
struct Snapshot {
    home: [u64; 2],
    checked: i64,
    issued: i64,
    gauge: String,
    distance: f64,
    peak: Option<Peak>,
    past: Vec<Past>,
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
fn fresh(s: &Snapshot, h: GeoView, now: i64) -> bool {
    s.home == home_key(h)
        && (0..=1200).contains(&(now - s.checked))
        && (0..=86400).contains(&(now - s.issued))
        && s.peak.as_ref().is_none_or(|p| p.first >= now)
}
fn valid_home(h: GeoView) -> bool {
    h.lon.is_finite()
        && h.lat.is_finite()
        && (-178.0..=-60.0).contains(&h.lon)
        && (15.0..=72.0).contains(&h.lat)
}
fn distance(h: GeoView, lon: f64, lat: f64) -> Option<f64> {
    if !valid_home(h)
        || !lon.is_finite()
        || !lat.is_finite()
        || !(-180.0..=180.0).contains(&lon)
        || !(-90.0..=90.0).contains(&lat)
    {
        return None;
    }
    let a = ((lat - h.lat).to_radians() / 2.).sin().powi(2)
        + h.lat.to_radians().cos()
            * lat.to_radians().cos()
            * ((lon - h.lon).to_radians() / 2.).sin().powi(2);
    Some(3958.7613 * 2. * a.clamp(0., 1.).sqrt().asin())
}
fn text(v: &Value, max: usize) -> Option<String> {
    let s = v.as_str()?.trim();
    (!s.is_empty() && s.len() <= max && !s.chars().any(char::is_control)).then(|| s.to_owned())
}
fn lid(v: &Value) -> Option<String> {
    let s = text(v, 15)?;
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
fn feet(v: &Value) -> bool {
    v.as_str().is_some_and(|s| {
        matches!(
            s.trim().to_ascii_lowercase().as_str(),
            "ft" | "feet" | "foot"
        )
    })
}
fn stage(v: &Value) -> Option<f64> {
    v.as_f64()
        .filter(|x| x.is_finite() && (-100.0..=3000.0).contains(x))
}
fn closest(v: &Value, h: GeoView) -> Option<String> {
    let a = v.get("gauges")?.as_array()?;
    if a.len() > 2000 {
        return None;
    }
    let mut candidates = BTreeMap::new();
    for g in a {
        let id = lid(g.get("lid")?)?;
        let lon = g.get("longitude")?.as_f64()?;
        let lat = g.get("latitude")?.as_f64()?;
        let d = distance(h, lon, lat)?;
        if let Some(old) = candidates.insert(id, (lon, lat, d)) {
            if old != (lon, lat, d) {
                return None;
            }
        }
    }
    candidates
        .into_iter()
        .filter(|(_, p)| p.2 <= 60.)
        .min_by(|a, b| a.1 .2.total_cmp(&b.1 .2).then_with(|| a.0.cmp(&b.0)))
        .map(|p| p.0)
}
/// A sampled, interior global peak; endpoints and separated equal peaks are ambiguous.
fn peak(points: &[(i64, f64)], now: i64) -> Option<Peak> {
    if points.len() < 3 {
        return None;
    }
    let high = points.iter().map(|p| p.1).max_by(f64::total_cmp)?;
    let indices: Vec<_> = points
        .iter()
        .enumerate()
        .filter(|(_, p)| p.1 == high)
        .map(|(i, _)| i)
        .collect();
    let first = *indices.first()?;
    let last = *indices.last()?;
    if first == 0
        || last + 1 == points.len()
        || indices.len() != last - first + 1
        || points[first].0 < now
    {
        return None;
    }
    Some(Peak {
        stage: high,
        first: points[first].0,
        last: points[last].0,
    })
}
fn history(meta: &Value, now: i64) -> Option<Vec<Past>> {
    let c = meta.get("flood")?.get("crests")?;
    let mut records: BTreeMap<i64, Past> = BTreeMap::new();
    for field in ["historic", "recent"] {
        let Some(a) = c.get(field).and_then(Value::as_array) else {
            continue;
        };
        if a.len() > 2000 {
            return None;
        }
        for p in a {
            // An absent datum flag is not evidence of compatibility.
            if p.get("olddatum").and_then(Value::as_bool) != Some(false) {
                continue;
            }
            let (Some(time), Some(stage)) = (
                p.get("occurredTime").and_then(stamp),
                p.get("stage").and_then(stage),
            ) else {
                continue;
            };
            if time >= now || time < -5364662400 {
                continue;
            }
            let quality = match p
                .get("preliminary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
                .as_str()
            {
                "true" | "yes" | "y" | "1" | "p" => "Preliminary",
                "false" | "no" | "n" | "0" => "Published",
                _ => "Status unspecified",
            }
            .to_owned();
            let record = Past {
                time,
                stage,
                quality,
            };
            if let Some(old) = records.get(&time) {
                if old != &record {
                    return None;
                }
            }
            records.insert(time, record);
        }
    }
    let mut rows: Vec<_> = records.into_values().collect();
    rows.sort_by(|a, b| {
        b.stage
            .total_cmp(&a.stage)
            .then_with(|| b.time.cmp(&a.time))
    });
    rows.truncate(4);
    Some(rows)
}
fn decode(meta: &Value, f: &Value, id: &str, h: GeoView, now: i64) -> Option<Snapshot> {
    if lid(meta.get("lid")?)? != id {
        return None;
    }
    let distance = distance(
        h,
        meta.get("longitude")?.as_f64()?,
        meta.get("latitude")?.as_f64()?,
    )?;
    if distance > 60.
        || !feet(meta.get("flood")?.get("stageUnits")?)
        || !feet(f.get("primaryUnits")?)
    {
        return None;
    }
    let code = f.get("pedts")?.as_str()?;
    if !code.starts_with("HG") || meta.get("pedts")?.get("forecast")?.as_str()? != code {
        return None;
    }
    let issued = stamp(f.get("issuedTime")?)?;
    if !(0..=86400).contains(&(now - issued)) {
        return None;
    }
    let a = f.get("data")?.as_array()?;
    if !(3..=2000).contains(&a.len()) {
        return None;
    }
    let mut series = BTreeMap::new();
    for p in a {
        let t = stamp(p.get("validTime")?)?;
        let x = stage(p.get("primary")?)?;
        if !(issued - 86400..=now + 14 * 86400).contains(&t) {
            return None;
        }
        if let Some(old) = series.insert(t, x) {
            if old != x {
                return None;
            }
        }
    }
    // Include one point before now so an imminent peak can be bracketed.
    let points: Vec<_> = series
        .into_iter()
        .filter(|p| p.0 >= now - 6 * 3600)
        .collect();
    if points.len() < 3
        || points[0].0 > now + 6 * 3600
        || points
            .windows(2)
            .any(|w| !(1..=6 * 3600).contains(&(w[1].0 - w[0].0)))
    {
        return None;
    }
    let name = meta
        .get("description")
        .and_then(|v| text(v, 140))
        .or_else(|| meta.get("name").and_then(|v| text(v, 100)))?;
    Some(Snapshot {
        home: home_key(h),
        checked: now,
        issued,
        gauge: format!("{name} • {id}"),
        distance,
        peak: peak(&points, now),
        past: history(meta, now).unwrap_or_default(),
    })
}
async fn fetch(h: GeoView) -> Option<Snapshot> {
    if !valid_home(h) {
        return None;
    }
    let dx = 1.1 / h.lat.to_radians().cos();
    if h.lon - dx < -180. || h.lon + dx > 180. {
        return None;
    }
    let url=format!("https://api.water.noaa.gov/nwps/v1/gauges?bbox.xmin={:.5}&bbox.ymin={:.5}&bbox.xmax={:.5}&bbox.ymax={:.5}&srid=EPSG_4326",h.lon-dx,h.lat-1.1,h.lon+dx,h.lat+1.1);
    let id = closest(&super::report_feed::get_json(&url).await?, h)?;
    let url = format!("https://api.water.noaa.gov/nwps/v1/gauges/{id}");
    let m = super::report_feed::get_json(&url).await?;
    let f = super::report_feed::get_json(&format!("{url}/stageflow/forecast")).await?;
    decode(&m, &f, &id, h, chrono::Utc::now().timestamp())
}
pub(super) async fn refresh(h: GeoView) {
    let ticket = match store().lock() {
        Ok(mut s) => {
            s.generation += 1;
            s.generation
        }
        Err(_) => return,
    };
    let result = fetch(h).await;
    if let Ok(mut s) = store().lock() {
        if s.generation == ticket {
            s.current = result.map(Arc::new);
        }
    }
}
fn values(s: Option<&Snapshot>) -> BTreeMap<String, String> {
    let mut v = BTreeMap::new();
    for f in [
        "height",
        "time",
        "gauge",
        "issued",
        "nearby",
        "status",
        "note",
        "history_note",
    ] {
        v.insert(f.to_string(), String::new());
    }
    v.insert(
        "source".into(),
        "Source: NOAA / NWS • National Water Prediction Service".into(),
    );
    v.insert("gauge".into(), "Location not available yet".into());
    v.insert("status".into(), MISSING.into());
    v.insert("issued".into(), "Forecast not available yet".into());
    v.insert(
        "history_note".into(),
        "Compatible published past crests not available yet".into(),
    );
    v.insert(
        "note".into(),
        "Gauge stage in feet • Not water depth or inundation".into(),
    );
    for i in 0..4 {
        for f in ["date", "height", "quality"] {
            v.insert(format!("past{i}_{f}"), String::new());
        }
    }
    let Some(s) = s else {
        return v;
    };
    v.insert("gauge".into(), s.gauge.clone());
    v.insert(
        "nearby".into(),
        format!("Nearby gauge • {:.1} mi from home", s.distance),
    );
    v.insert(
        "issued".into(),
        format!("Issued {}", date(s.issued, "%b %d • %H:%M UTC")),
    );
    if let Some(p) = &s.peak {
        v.insert("status".into(), "Forecast crest near".into());
        v.insert("height".into(), format!("{:.1} ft", p.stage));
        v.insert(
            "time".into(),
            if p.first == p.last {
                date(p.first, "%a %b %d • %H:%M UTC")
            } else {
                format!(
                    "{}–{}",
                    date(p.first, "%b %d %H:%M"),
                    date(p.last, "%b %d %H:%M UTC")
                )
            },
        );
    }
    if !s.past.is_empty() {
        v.insert(
            "history_note".into(),
            "Selected high crests • Old-datum entries excluded".into(),
        );
        for (i, p) in s.past.iter().enumerate() {
            v.insert(format!("past{i}_date"), date(p.time, "%b %d, %Y"));
            v.insert(format!("past{i}_height"), format!("{:.1} ft", p.stage));
            v.insert(format!("past{i}_quality"), p.quality.clone());
        }
    }
    v
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let current = store().lock().ok().and_then(|s| {
        s.current
            .as_ref()
            .filter(|s| fresh(s, settings.home, now))
            .cloned()
    });
    for (k, v) in values(current.as_deref()) {
        b.set(&format!("set.rivercrest.{k}"), &v);
    }
}
fn token(f: &str) -> String {
    format!("{{{{set.rivercrest.{f}}}}}")
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let format = KEYS.iter().position(|k| *k == key)?;
    let title = "River Crest Forecast";
    let mut d = match format {
        1 => social_frame(title, "Flooding", &token("issued"), &token("source"), true),
        2 => social_frame(title, "Flooding", &token("issued"), &token("source"), false),
        3 => lowerthird_frame(title, "Flooding", &token("issued"), &token("source")),
        _ => frame(title, "Flooding", &token("issued"), &token("source")),
    };
    if format == 3 {
        d.layers.push(
            crate::gfx::model::Layer::rect(
                "River crest content band",
                0.,
                865.,
                1920.,
                131.,
                crate::gfx::model::Fill::solid("$panel"),
                0.,
            )
            .with_anim(AnimKind::Rise, 0.15, 0.4),
        );
    }
    let mut put = |name: &str, box_, value: &str, size, color| {
        label(&mut d, name, box_, value, size, color, Align::Left)
    };
    if format == 3 {
        // Every layer stays within the branded lower-third strip.
        put(
            "Gauge",
            (80., 872., 1140., 42.),
            &format!("Nearby gauge • {}", token("gauge")),
            29.,
            "$text",
        );
        put(
            "Crest status",
            (80., 920., 300., 56.),
            &token("status"),
            27.,
            "$muted",
        );
        put(
            "Crest height",
            (390., 920., 240., 56.),
            &token("height"),
            44.,
            "$text",
        );
        put(
            "Crest time",
            (640., 920., 590., 56.),
            &token("time"),
            28.,
            "$text",
        );
        put(
            "Past crest heading",
            (1260., 872., 580., 32.),
            "SELECTED PAST CRESTS • ft",
            24.,
            "$muted",
        );
        for i in 0..2 {
            put(
                &format!("Past {i} date"),
                (1260., 912. + i as f32 * 36., 250., 34.),
                &token(&format!("past{i}_date")),
                25.,
                "$text",
            );
            put(
                &format!("Past {i} quality"),
                (1515., 912. + i as f32 * 36., 185., 34.),
                &token(&format!("past{i}_quality")),
                19.,
                "$muted",
            );
            put(
                &format!("Past {i} height"),
                (1710., 912. + i as f32 * 36., 130., 34.),
                &token(&format!("past{i}_height")),
                27.,
                "$text",
            );
        }
    } else {
        let (x, w, gauge_y, near_y, cx, cy, cw, ch, hx, hy, hw, step, rows, note_y) = match format {
            1 => (
                54., 972., 245., 320., 54., 410., 972., 400., 54., 980., 972., 135., 4, 1670.,
            ),
            2 => (
                54., 972., 210., 285., 54., 350., 972., 220., 54., 600., 972., 76., 3, 930.,
            ),
            _ => (
                80., 1760., 190., 258., 80., 350., 730., 500., 870., 350., 970., 118., 4, 930.,
            ),
        };
        put(
            "Gauge",
            (x, gauge_y, w, 70.),
            &token("gauge"),
            if format == 0 { 42. } else { 35. },
            "$text",
        );
        put(
            "Nearby gauge distance",
            (x, near_y, w, 44.),
            &token("nearby"),
            27.,
            "$muted",
        );
        drop(put);
        d.layers.push(
            crate::gfx::templates::panel("Forecast crest panel", cx, cy, cw, ch).with_anim(
                AnimKind::Rise,
                0.15,
                0.4,
            ),
        );
        let mut put = |name: &str, box_, value: &str, size, color| {
            label(&mut d, name, box_, value, size, color, Align::Left)
        };
        put(
            "Crest status",
            (cx + 24., cy + 18., cw - 48., 70.),
            &token("status"),
            if format == 2 { 32. } else { 40. },
            "$muted",
        );
        put(
            "Crest height",
            (
                cx + 24.,
                cy + if format == 2 { 78. } else { 115. },
                cw - 48.,
                if format == 2 { 70. } else { 150. },
            ),
            &token("height"),
            if format == 2 { 70. } else { 124. },
            "$text",
        );
        put(
            "Crest time",
            (
                cx + 24.,
                cy + ch - if format == 2 { 65. } else { 105. },
                cw - 48.,
                if format == 2 { 50. } else { 82. },
            ),
            &token("time"),
            if format == 2 { 28. } else { 35. },
            "$text",
        );
        put(
            "Past crest heading",
            (hx, hy, hw, 52.),
            "SELECTED PAST CRESTS",
            if format == 2 { 28. } else { 34. },
            "$text",
        );
        put(
            "Past crest method",
            (hx, hy + 52., hw, 52.),
            &token("history_note"),
            if format == 2 { 20. } else { 25. },
            "$muted",
        );
        for i in 0..rows {
            let y = hy + 112. + i as f32 * step;
            put(
                &format!("Past {i} date"),
                (hx, y, hw * 0.62, step * 0.48),
                &token(&format!("past{i}_date")),
                if format == 2 { 27. } else { 36. },
                "$text",
            );
            put(
                &format!("Past {i} height"),
                (hx + hw * 0.67, y, hw * 0.33, step * 0.48),
                &token(&format!("past{i}_height")),
                if format == 2 { 30. } else { 42. },
                "$text",
            );
            put(
                &format!("Past {i} quality"),
                (hx, y + step * 0.48, hw, step * 0.38),
                &token(&format!("past{i}_quality")),
                if format == 2 { 20. } else { 25. },
                "$muted",
            );
        }
        put(
            "Stage meaning",
            (x, note_y, w, 40.),
            &token("note"),
            if format == 2 { 24. } else { 28. },
            "$muted",
        );
    }
    Some(d)
}
#[cfg(test)]
pub(super) fn sample(d: &mut GraphicDoc) {
    if !KEYS.iter().any(|k| Some(*k) == d.template.as_deref()) {
        return;
    }
    let now = 1794571200;
    let s = Snapshot {
        home: [0, 0],
        checked: now,
        issued: now,
        gauge: "White River near Jeffersonville • SAMPLE".into(),
        distance: 12.3,
        peak: Some(Peak {
            stage: 26.4,
            first: now + 86400,
            last: now + 86400,
        }),
        past: vec![
            Past {
                time: 1483228800,
                stage: 34.2,
                quality: "Published".into(),
            },
            Past {
                time: 1420070400,
                stage: 31.8,
                quality: "Preliminary".into(),
            },
            Past {
                time: 1262304000,
                stage: 30.1,
                quality: "Status unspecified".into(),
            },
            Past {
                time: 946684800,
                stage: 28.9,
                quality: "Published".into(),
            },
        ],
    };
    let mut v = values(Some(&s));
    v.insert("source".into(), "SAMPLE ONLY • Not for broadcast".into());
    v.insert(
        "note".into(),
        "SAMPLE • Synthetic stages • Gauge stage is not water depth".into(),
    );
    for (k, v) in v {
        d.data_overrides.push((format!("set.rivercrest.{k}"), v));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn home() -> GeoView {
        GeoView {
            lon: -86.9,
            lat: 38.4,
            zoom: 8.,
        }
    }
    fn fixture(now: i64) -> (Value, Value) {
        let meta = json!({"lid":"TEST1","description":"River near Jeffersonville","longitude":-86.9,"latitude":38.4,"pedts":{"forecast":"HGIFE"},"flood":{"stageUnits":"ft","crests":{"historic":[{"occurredTime":"2000-01-01T00:00:00Z","stage":30.,"olddatum":false,"preliminary":"false"}],"recent":[]}}});
        let forecast = json!({"pedts":"HGIFE","primaryUnits":"ft","issuedTime":date(now-3600,"%Y-%m-%dT%H:%M:%SZ"),"data":[{"validTime":date(now,"%Y-%m-%dT%H:%M:%SZ"),"primary":20.},{"validTime":date(now+3600,"%Y-%m-%dT%H:%M:%SZ"),"primary":25.},{"validTime":date(now+7200,"%Y-%m-%dT%H:%M:%SZ"),"primary":22.}]});
        (meta, forecast)
    }
    #[test]
    fn only_interior_contiguous_forecast_peak_has_a_crest_time() {
        let p = |values: &[f64]| {
            values
                .iter()
                .enumerate()
                .map(|(i, x)| (1000 + i as i64 * 3600, *x))
                .collect::<Vec<_>>()
        };
        assert!(peak(&p(&[1., 2., 3.]), 1000).is_none());
        assert!(peak(&p(&[3., 2., 1.]), 1000).is_none());
        assert!(peak(&p(&[1., 3., 2., 3., 1.]), 1000).is_none());
        let x = peak(&p(&[1., 3., 3., 1.]), 1000).unwrap();
        assert_eq!((x.first, x.last, x.stage), (4600, 8200, 3.));
        assert!(peak(&p(&[1., 3., 1.]), 5000).is_none());
    }
    #[test]
    fn nwps_contract_requires_fresh_stage_and_compatible_history() {
        let now = 1794571200;
        let (mut m, mut f) = fixture(now);
        let s = decode(&m, &f, "TEST1", home(), now).unwrap();
        assert_eq!(s.peak.unwrap().stage, 25.);
        assert_eq!(s.past.len(), 1);
        m["flood"]["crests"]["historic"][0]["olddatum"] = json!(true);
        assert!(decode(&m, &f, "TEST1", home(), now)
            .unwrap()
            .past
            .is_empty());
        f["primaryUnits"] = json!("kcfs");
        assert!(decode(&m, &f, "TEST1", home(), now).is_none());
        let (_, mut f) = fixture(now);
        f["issuedTime"] = json!(date(now - 86401, "%Y-%m-%dT%H:%M:%SZ"));
        assert!(decode(&m, &f, "TEST1", home(), now).is_none());
        let (_, mut f) = fixture(now);
        f["data"][1]["primary"] = json!(-999.);
        assert!(decode(&m, &f, "TEST1", home(), now).is_none());
        let (_, mut f) = fixture(now);
        f["data"][2] = f["data"][1].clone();
        f["data"][2]["primary"] = json!(23.);
        assert!(decode(&m, &f, "TEST1", home(), now).is_none());
    }
    #[test]
    fn live_snapshot_never_crosses_home_or_clock_or_peak() {
        let now = 1794571200;
        let (m, f) = fixture(now);
        let s = decode(&m, &f, "TEST1", home(), now).unwrap();
        assert!(fresh(&s, home(), now + 60));
        assert!(!fresh(&s, home(), now - 1));
        assert!(!fresh(&s, home(), now + 1201));
        let mut h = home();
        h.lon += 0.01;
        assert!(!fresh(&s, h, now));
        let v = values(None);
        assert_eq!(v["height"], "");
        assert_eq!(v["past0_height"], "");
        assert_eq!(v["status"], MISSING);
        let gauges = json!({"gauges":[{"lid":"FAR1","latitude":39.,"longitude":-86.9},{"lid":"NEAR1","latitude":38.4,"longitude":-86.9}]});
        assert_eq!(closest(&gauges, home()).as_deref(), Some("NEAR1"));
    }
    #[test]
    fn historical_duplicates_and_sampling_gaps_fail_closed() {
        let now = 1794571200;
        let (mut m, mut f) = fixture(now);
        let past = m["flood"]["crests"]["historic"][0].clone();
        m["flood"]["crests"]["recent"] = json!([past]);
        assert_eq!(history(&m, now).unwrap().len(), 1);
        m["flood"]["crests"]["recent"][0]["stage"] = json!(31.);
        assert!(history(&m, now).is_none());
        assert!(decode(&m, &f, "TEST1", home(), now)
            .unwrap()
            .past
            .is_empty());
        f["data"][2]["validTime"] = json!(date(now + 8 * 3600, "%Y-%m-%dT%H:%M:%SZ"));
        assert!(decode(&m, &f, "TEST1", home(), now).is_none());
    }
}
