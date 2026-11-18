//! SPC's published mesoscale discussion polygons, with explicit service-check time.
//! No watch probabilities, warning status or product expiry times are inferred.
use super::{
    board::{self, Band, Layout, Scene, Zone},
    comparison,
    observed::{self, Coverage},
};
use crate::{
    config::Settings,
    data::MesoDiscussion,
    gfx::{bind::Bindings, model::GraphicDoc},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};

pub(super) const SPECS: &[(&str, Coverage)] = &[
    ("set_mesoscale_discussions__region", Coverage::Region),
    ("set_mesoscale_discussions__state", Coverage::State),
    ("set_mesoscale_discussions__multi", Coverage::Multi),
    (
        "set_mesoscale_discussions__region_portrait",
        Coverage::Region,
    ),
    ("set_mesoscale_discussions__state_portrait", Coverage::State),
    ("set_mesoscale_discussions__multi_portrait", Coverage::Multi),
    ("set_mesoscale_discussions__region_square", Coverage::Region),
    ("set_mesoscale_discussions__state_square", Coverage::State),
    ("set_mesoscale_discussions__multi_square", Coverage::Multi),
];
fn prefix(key: &str) -> String {
    format!(
        "meso.{}",
        key.trim_start_matches("set_mesoscale_discussions__")
            .trim_end_matches("_portrait")
            .trim_end_matches("_square")
    )
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    SPECS.iter().find(|s| s.0 == key)?;
    let p = prefix(key);
    Some(if key.ends_with("_portrait") {
        comparison::portrait_template("Mesoscale Discussions", "Severe Weather", &p)
    } else if key.ends_with("_square") {
        comparison::square_template("Mesoscale Discussions", "Severe Weather", &p)
    } else {
        comparison::template(
            "Mesoscale Discussions",
            "Severe Weather",
            &p,
            Layout::Single,
        )
    })
}
struct Snapshot {
    items: Vec<MesoDiscussion>,
    checked: i64,
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
fn fresh(s: &Snapshot, now: i64) -> bool {
    (0..=20 * 60).contains(&(now - s.checked))
}
const URL:&str="https://mapservices.weather.noaa.gov/vector/rest/services/outlooks/spc_mesoscale_discussion/MapServer/0/query?where=1%3D1&outFields=*&f=geojson";
fn decode(value: &serde_json::Value, checked: i64) -> Option<Snapshot> {
    if value.get("type")?.as_str()? != "FeatureCollection"
        || value.get("exceededTransferLimit").and_then(|v| v.as_bool()) == Some(true)
    {
        return None;
    }
    let features = value.get("features")?.as_array()?;
    if features.len() > 2000 {
        return None;
    }
    for f in features {
        let name = f.get("properties")?.get("name")?.as_str()?;
        if name.eq_ignore_ascii_case("NoArea") {
            continue;
        }
        if name.trim().is_empty()
            || name.len() > 48
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b" #.-".contains(&b))
        {
            return None;
        }
        // The board draws solid exterior rings. Reject holes rather than filling them.
        let g = f.get("geometry")?;
        let c = g.get("coordinates")?.as_array()?;
        let valid_ring = |v: &serde_json::Value| {
            let Some(r) = v.as_array() else {
                return false;
            };
            (4..=4096).contains(&r.len())
                && r.first() == r.last()
                && r.iter().all(|v| {
                    let Some(p) = v.as_array() else {
                        return false;
                    };
                    p.len() >= 2
                        && p[0]
                            .as_f64()
                            .is_some_and(|x| x.is_finite() && (-180.0..=180.0).contains(&x))
                        && p[1]
                            .as_f64()
                            .is_some_and(|y| y.is_finite() && (-85.0..=85.0).contains(&y))
                })
        };
        let valid = match g.get("type")?.as_str()? {
            "Polygon" => c.len() == 1 && valid_ring(&c[0]),
            "MultiPolygon" => {
                !c.is_empty()
                    && c.len() <= 128
                    && c.iter().all(|p| {
                        p.as_array()
                            .is_some_and(|a| a.len() == 1 && valid_ring(&a[0]))
                    })
            }
            _ => false,
        };
        if !valid {
            return None;
        }
    }
    let mut unique = BTreeMap::new();
    for d in crate::data::parse_mesoscale(value) {
        if d.rings.is_empty()
            || d.rings.iter().any(|r| {
                r.len() < 4
                    || r.len() > 4096
                    || r.iter().any(|&(x, y)| {
                        !x.is_finite()
                            || !y.is_finite()
                            || !(-180.0..=180.0).contains(&x)
                            || !(-85.0..=85.0).contains(&y)
                    })
            })
        {
            return None;
        }
        if let Some(old) = unique.insert(d.name.clone(), d.clone()) {
            if old.rings != d.rings {
                return None;
            }
        }
    }
    Some(Snapshot {
        items: unique.into_values().collect(),
        checked,
    })
}
pub(super) async fn refresh() {
    let ticket = match store().lock() {
        Ok(mut s) => {
            s.generation += 1;
            s.generation
        }
        Err(_) => return,
    };
    let result = super::report_feed::get_json(URL)
        .await
        .and_then(|v| decode(&v, chrono::Utc::now().timestamp()));
    if let Ok(mut s) = store().lock() {
        if s.generation == ticket {
            s.current = result.map(Arc::new);
        }
    }
}
fn cross(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}
fn segments(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    if a.0.min(b.0) > c.0.max(d.0)
        || c.0.min(d.0) > a.0.max(b.0)
        || a.1.min(b.1) > c.1.max(d.1)
        || c.1.min(d.1) > a.1.max(b.1)
    {
        return false;
    }
    cross(a, b, c) * cross(a, b, d) <= 0. && cross(c, d, a) * cross(c, d, b) <= 0.
}
fn touches(a: &[(f64, f64)], b: &[(f64, f64)]) -> bool {
    let bbox = |r: &[(f64, f64)]| {
        r.iter().fold(
            [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
            |b, &(x, y)| [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)],
        )
    };
    let x = bbox(a);
    let y = bbox(b);
    if x[0] > y[2] || y[0] > x[2] || x[1] > y[3] || y[1] > x[3] {
        return false;
    }
    a.iter()
        .any(|&(x, y)| crate::gfx::winter_map::inside_pub(b, x, y))
        || b.iter()
            .any(|&(x, y)| crate::gfx::winter_map::inside_pub(a, x, y))
        || (0..a.len()).any(|i| {
            (0..b.len()).any(|j| segments(a[i], a[(i + 1) % a.len()], b[j], b[(j + 1) % b.len()]))
        })
}
fn scene(settings: &Settings, coverage: Coverage, snapshot: Option<&Snapshot>) -> Scene {
    let mut scene = board::empty(Layout::Single);
    scene.scope = observed::report_scope(settings, coverage);
    scene.legend_title = "SPC published discussion polygons".into();
    scene.legend = vec![Band {
        label: "Discussion area".into(),
        color: "#A900E6".into(),
    }];
    let p = &mut scene.panels[0];
    p.title = settings.home_label.clone();
    p.valid = "Service data not available yet".into();
    p.detail = "Read the SPC discussion for hazards, timing and reasoning".into();
    let Some(s) = snapshot else {
        return scene;
    };
    p.valid = chrono::DateTime::from_timestamp(s.checked, 0)
        .map(|t| t.format("Service checked %b %-d, %Y %H:%M UTC").to_string())
        .unwrap_or_default();
    let Some(all) = crate::map::boundaries::counties_named() else {
        return scene;
    };
    let counties = board::selected(&all, &scene.scope);
    for d in &s.items {
        for ring in &d.rings {
            if !counties
                .iter()
                .any(|c| c.rings.iter().any(|r| touches(ring, r)))
            {
                continue;
            }
            let at = counties
                .iter()
                .map(|c| board::centre(c))
                .find(|&(x, y)| crate::gfx::winter_map::inside_pub(ring, x, y));
            p.zones.get_or_insert_with(Vec::new).push(Zone {
                category: 0,
                ring: ring.clone(),
                label: d.name.clone(),
                label_at: at,
            });
            scene.reviewed = true;
        }
    }
    if let Some(zones) = &p.zones {
        let ids: std::collections::BTreeSet<_> = zones.iter().map(|z| z.label.clone()).collect();
        p.summary = Some(
            ids.into_iter()
                .take(7)
                .map(|id| (id, "SPC".into()))
                .collect(),
        );
        p.summary_title = "Published IDs • Up to 7 shown".into();
    }
    scene
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let current = store()
        .lock()
        .ok()
        .and_then(|s| s.current.as_ref().filter(|s| fresh(s, now)).cloned());
    for &(key, coverage) in &SPECS[..3] {
        let p = prefix(key);
        let scene = scene(settings, coverage, current.as_deref());
        b.set(
            &format!("set.{p}.payload"),
            &serde_json::to_string(&scene).unwrap_or_default(),
        );
        b.set(&format!("set.{p}.subtitle"), &settings.home_label);
        b.set(
            &format!("set.{p}.source"),
            "Source: NWS Storm Prediction Center • NOAA map service",
        );
        b.set(&format!("set.{p}.note"),"Discussion areas describe short-term weather concerns. Check current NWS watches and warnings separately.");
    }
}
#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some((key, coverage)) = SPECS.iter().find(|s| Some(s.0) == doc.template.as_deref()) else {
        return;
    };
    let snapshot = Snapshot {
        checked: 1794592800,
        items: vec![MesoDiscussion {
            name: "MD 1234 • SAMPLE".into(),
            rings: vec![vec![
                (-88., 37.8),
                (-86.3, 37.8),
                (-85.7, 39.4),
                (-87.5, 39.6),
                (-88., 37.8),
            ]],
            url: None,
        }],
    };
    let mut settings = Settings::default();
    settings.home_label = "SAMPLE • Dubois County, IN".into();
    let s = scene(&settings, *coverage, Some(&snapshot));
    let p = prefix(key);
    doc.data_overrides.extend([
        (
            format!("set.{p}.payload"),
            serde_json::to_string(&s).unwrap(),
        ),
        (
            format!("set.{p}.subtitle"),
            "SAMPLE • Published-area layout".into(),
        ),
        (
            format!("set.{p}.source"),
            "SAMPLE ONLY • Not for broadcast".into(),
        ),
    ]);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intersections_include_thin_crossings_and_exclude_distant_areas() {
        let a = [(-2., -0.1), (2., -0.1), (2., 0.1), (-2., 0.1)];
        let b = [(-0.1, -2.), (0.1, -2.), (0.1, 2.), (-0.1, 2.)];
        assert!(touches(&a, &b));
        assert!(!touches(&a, &[(5., 5.), (6., 5.), (6., 6.), (5., 6.)]));
    }
    #[test]
    fn stale_service_data_and_clock_reversal_are_unavailable() {
        let s = Snapshot {
            checked: 10000,
            items: Vec::new(),
        };
        assert!(fresh(&s, 11000));
        assert!(!fresh(&s, 11201));
        assert!(!fresh(&s, 9999));
    }
    #[test]
    fn noarea_is_empty_and_holes_or_conflicting_ids_are_rejected() {
        let empty = serde_json::json!({"type":"FeatureCollection","features":[{"properties":{"name":"NoArea"}}]});
        assert!(decode(&empty, 10000).unwrap().items.is_empty());
        let f = |x: f64| serde_json::json!({"properties":{"name":"MD 1234"},"geometry":{"type":"Polygon","coordinates":[[[x,38.],[-86.,38.],[-86.,39.],[x,39.],[x,38.]]]}});
        let v = serde_json::json!({"type":"FeatureCollection","features":[f(-87.)]});
        assert_eq!(decode(&v, 10000).unwrap().items.len(), 1);
        let v = serde_json::json!({"type":"FeatureCollection","features":[f(-87.),f(-88.)]});
        assert!(decode(&v, 10000).is_none());
        let mut malformed = f(-87.);
        malformed["geometry"]["coordinates"][0][1] = serde_json::json!(["bad", 38.]);
        assert!(decode(
            &serde_json::json!({"type":"FeatureCollection","features":[malformed]}),
            10000
        )
        .is_none());
        let mut f = f(-87.);
        f["geometry"]["coordinates"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!([[0., 0.], [1., 0.], [0., 1.], [0., 0.]]));
        assert!(decode(
            &serde_json::json!({"type":"FeatureCollection","features":[f]}),
            10000
        )
        .is_none());
    }
}
