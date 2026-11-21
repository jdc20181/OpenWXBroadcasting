//! NOAA RAP county-centre surface maps. No non-NOAA fallback is used.
use super::{
    board::{self, Band, Layout, Panel, Scene, Scope},
    model_feed::{self, Bounds, Field, Frame},
    observed::{self, Coverage},
};
use crate::{
    config::{GeoView, Settings},
    gfx::{
        bind::Bindings,
        model::{AnimKind, GraphicDoc, Layer},
        text::Align,
    },
};
use chrono::{DateTime, Utc};
use futures_util::{StreamExt, stream};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Metric {
    Temperature,
    Dewpoint,
    Humidity,
    Wind,
    Gust,
    Cloud,
    Pressure,
    Cape,
}
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    field: Metric,
    title: &'static str,
    coverage: Coverage,
    layout: Layout,
    portrait: bool,
}
include!("model_map_specs.rs");
// Loop rows reuse the same four already-loaded RAP frames as the timeline graphics.  These
// are presentation variants only: no additional refresh, request, or snapshot state is added.
macro_rules! loop_specs_for_fields {
    ($(($field:ident, $slug:literal, $title:literal)),* $(,)?) => {
        &[
            $(
                Spec { key: concat!("set_model_rap__", $slug, "_loop_region"), field: Metric::$field, title: $title, coverage: Coverage::Region, layout: Layout::Single, portrait: false },
                Spec { key: concat!("set_model_rap__", $slug, "_loop_state"), field: Metric::$field, title: $title, coverage: Coverage::State, layout: Layout::Single, portrait: false },
                Spec { key: concat!("set_model_rap__", $slug, "_loop_multi"), field: Metric::$field, title: $title, coverage: Coverage::Multi, layout: Layout::Single, portrait: false },
                Spec { key: concat!("set_model_rap__", $slug, "_loop_region_portrait"), field: Metric::$field, title: $title, coverage: Coverage::Region, layout: Layout::Single, portrait: true },
                Spec { key: concat!("set_model_rap__", $slug, "_loop_state_portrait"), field: Metric::$field, title: $title, coverage: Coverage::State, layout: Layout::Single, portrait: true },
                Spec { key: concat!("set_model_rap__", $slug, "_loop_multi_portrait"), field: Metric::$field, title: $title, coverage: Coverage::Multi, layout: Layout::Single, portrait: true },
            )*
        ]
    };
}
const LOOP_SPECS: &[Spec] = loop_specs_for_fields![
    (Temperature, "temperature", "2 m temperature"),
    (Dewpoint, "dewpoint", "2 m dew point"),
    (Humidity, "humidity", "Relative humidity 2 m"),
    (Wind, "wind", "10 m wind"),
    (Gust, "gust", "Wind gusts"),
    (Cloud, "cloud", "Total cloud cover"),
    (Pressure, "pressure", "Mean sea-level pressure"),
    (Cape, "cape", "CAPE"),
];
// Standard RAP cycles end at +21 h (NCEP product inventory).
const LEADS: [u32; 4] = [6, 12, 18, 21];
const LOOP_SECS: f32 = 8.0;
const FRAME_SECS: f32 = LOOP_SECS / 4.0;
const MAX_AGE: i64 = 20 * 60;

struct Site {
    fips: String,
    lon: f64,
    lat: f64,
}
struct ScopeData {
    scope: Scope,
    sites: Vec<Site>,
    frames: BTreeMap<u32, Frame>,
}
struct Snapshot {
    home: (u64, u64),
    checked: i64,
    run: Option<i64>,
    scopes: BTreeMap<&'static str, ScopeData>,
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
fn home_key(h: GeoView) -> (u64, u64) {
    (h.lon.to_bits(), h.lat.to_bits())
}
fn cov_key(c: Coverage) -> &'static str {
    match c {
        Coverage::Region => "region",
        Coverage::State => "state",
        Coverage::Multi => "multi",
        Coverage::County => "county",
    }
}
fn cov_label(c: Coverage) -> &'static str {
    match c {
        Coverage::Region => "Home region",
        Coverage::State => "Home state",
        Coverage::Multi => "Selected states",
        Coverage::County => "Home county",
    }
}

fn scope_job(settings: &Settings, cov: Coverage) -> Option<(Scope, Vec<Site>, Bounds)> {
    let scope = observed::report_scope(settings, cov);
    let all = crate::map::boundaries::counties_named()?;
    let sites: Vec<_> = board::selected(&all, &scope)
        .into_iter()
        .filter_map(|c| {
            let (lon, lat) = board::centre(c);
            (board::inside(c, lon, lat) && lon.is_finite() && lat.is_finite()).then(|| Site {
                fips: c.fips.clone(),
                lon,
                lat,
            })
        })
        .collect();
    if sites.is_empty() {
        return None;
    }
    let (west, south, east, north) = if let Some([w, s, e, n]) = scope.bbox {
        (w, s, e, n)
    } else {
        (
            sites.iter().map(|x| x.lon).fold(f64::INFINITY, f64::min) - 0.75,
            sites.iter().map(|x| x.lat).fold(f64::INFINITY, f64::min) - 0.75,
            sites
                .iter()
                .map(|x| x.lon)
                .fold(f64::NEG_INFINITY, f64::max)
                + 0.75,
            sites
                .iter()
                .map(|x| x.lat)
                .fold(f64::NEG_INFINITY, f64::max)
                + 0.75,
        )
    };
    if ![west, south, east, north].iter().all(|v| v.is_finite())
        || west >= east
        || south >= north
        || east - west > 20.
        || north - south > 15.
    {
        return None;
    }
    Some((
        scope,
        sites,
        Bounds {
            west,
            south,
            east,
            north,
        },
    ))
}

pub(super) async fn refresh(home: GeoView) {
    let ticket = match store().lock() {
        Ok(mut s) => {
            s.generation += 1;
            s.generation
        }
        Err(_) => return,
    };
    let mut settings = Settings::default();
    settings.home = home;
    let mut jobs = HashMap::new();
    for cov in [Coverage::Region, Coverage::State, Coverage::Multi] {
        if let Some(v) = scope_job(&settings, cov) {
            jobs.insert(cov_key(cov), v);
        }
    }
    let run = model_feed::latest_run().await.ok();
    let mut fetched: HashMap<(String, u32), Frame> = HashMap::new();
    if let Some(run) = run {
        let mut requests = Vec::new();
        for (key, (_, sites, bounds)) in &jobs {
            for lead in LEADS {
                requests.push((
                    (*key).to_string(),
                    lead,
                    *bounds,
                    sites
                        .iter()
                        .map(|s| model_feed::Site {
                            id: s.fips.clone(),
                            lon: s.lon,
                            lat: s.lat,
                        })
                        .collect::<Vec<_>>(),
                ));
            }
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
        let mut pending = stream::iter(requests.into_iter().map(
            |(key, lead, bounds, sites)| async move {
                (key, lead, model_feed::fetch(run, lead, bounds, sites).await)
            },
        ))
        .buffer_unordered(2);
        loop {
            match tokio::time::timeout_at(deadline, pending.next()).await {
                Ok(Some((key, lead, Ok(frame)))) => {
                    fetched.insert((key, lead), frame);
                }
                Ok(Some((_, _, Err(_)))) => {}
                Ok(None) | Err(_) => break,
            }
        }
    }
    let scopes = jobs
        .into_iter()
        .map(|(key, (scope, sites, _))| {
            let frames = LEADS
                .into_iter()
                .filter_map(|lead| fetched.remove(&(key.to_string(), lead)).map(|f| (lead, f)))
                .collect();
            (
                key,
                ScopeData {
                    scope,
                    sites,
                    frames,
                },
            )
        })
        .collect();
    let snap = Arc::new(Snapshot {
        home: home_key(home),
        checked: Utc::now().timestamp(),
        run,
        scopes,
    });
    if let Ok(mut s) = store().lock() {
        if s.generation == ticket {
            s.current = Some(snap)
        }
    }
}

fn bands(m: Metric) -> (&'static str, Vec<Band>) {
    let (title, pairs) = match m {
        Metric::Temperature => (
            "2 m temperature • °F",
            [
                ("Below 20°F", "#3855B8"),
                ("20 to <32°F", "#387DD0"),
                ("32 to <45°F", "#28B8D2"),
                ("45 to <60°F", "#37BE83"),
                ("60 to <75°F", "#D6DB45"),
                ("75 to <90°F", "#F5A332"),
                ("90 to <105°F", "#EB5538"),
                ("105°F and above", "#B52643"),
            ],
        ),
        Metric::Dewpoint => (
            "2 m dew point • °F",
            [
                ("Below 20°F", "#6475A8"),
                ("20 to <32°F", "#447BC2"),
                ("32 to <45°F", "#32A9CC"),
                ("45 to <55°F", "#35B995"),
                ("55 to <65°F", "#B5CA4C"),
                ("65 to <70°F", "#E0A739"),
                ("70 to <75°F", "#D67535"),
                ("75°F and above", "#B83B45"),
            ],
        ),
        Metric::Humidity => (
            "2 m relative humidity • %",
            [
                ("Below 20%", "#C56D39"),
                ("20 to <30%", "#DDAA3B"),
                ("30 to <40%", "#D1CD49"),
                ("40 to <50%", "#83BE5B"),
                ("50 to <60%", "#43B99B"),
                ("60 to <70%", "#35AFC9"),
                ("70 to <80%", "#467FC7"),
                ("80% and above", "#5D60B7"),
            ],
        ),
        Metric::Wind => (
            "10 m wind speed • mph",
            [
                ("Below 5 mph", "#3855B8"),
                ("5 to <10 mph", "#387DD0"),
                ("10 to <15 mph", "#28B8D2"),
                ("15 to <20 mph", "#37BE83"),
                ("20 to <30 mph", "#D6DB45"),
                ("30 to <40 mph", "#F5A332"),
                ("40 to <55 mph", "#EB5538"),
                ("55 mph and above", "#B52643"),
            ],
        ),
        Metric::Gust => (
            "Surface wind gust • mph",
            [
                ("Below 15 mph", "#3855B8"),
                ("15 to <25 mph", "#387DD0"),
                ("25 to <35 mph", "#28B8D2"),
                ("35 to <45 mph", "#37BE83"),
                ("45 to <55 mph", "#D6DB45"),
                ("55 to <65 mph", "#F5A332"),
                ("65 to <80 mph", "#EB5538"),
                ("80 mph and above", "#B52643"),
            ],
        ),
        Metric::Cloud => (
            "Total cloud cover • %",
            [
                ("Below 10%", "#D9B744"),
                ("10 to <25%", "#B5C65A"),
                ("25 to <40%", "#82BA72"),
                ("40 to <55%", "#5FAE91"),
                ("55 to <70%", "#528FAC"),
                ("70 to <85%", "#5B78B4"),
                ("85 to <95%", "#66708E"),
                ("95% and above", "#8B91A3"),
            ],
        ),
        Metric::Pressure => (
            "Mean sea level pressure • hPa",
            [
                ("Below 980 hPa", "#3855B8"),
                ("980 to <990 hPa", "#387DD0"),
                ("990 to <1000 hPa", "#28B8D2"),
                ("1000 to <1010 hPa", "#37BE83"),
                ("1010 to <1020 hPa", "#D6DB45"),
                ("1020 to <1030 hPa", "#F5A332"),
                ("1030 to <1040 hPa", "#EB5538"),
                ("1040 hPa and above", "#B52643"),
            ],
        ),
        Metric::Cape => (
            "CAPE • J/kg",
            [
                ("Below 100 J/kg", "#3855B8"),
                ("100 to <500 J/kg", "#387DD0"),
                ("500 to <1000 J/kg", "#28B8D2"),
                ("1000 to <1500 J/kg", "#37BE83"),
                ("1500 to <2000 J/kg", "#D6DB45"),
                ("2000 to <3000 J/kg", "#F5A332"),
                ("3000 to <4000 J/kg", "#EB5538"),
                ("4000 J/kg and above", "#B52643"),
            ],
        ),
    };
    (
        title,
        pairs
            .into_iter()
            .map(|(label, color)| Band {
                label: label.into(),
                color: color.into(),
            })
            .collect(),
    )
}
fn metric_label(m: Metric) -> &'static str {
    match m {
        Metric::Temperature => "2 m temperature",
        Metric::Dewpoint => "2 m dew point",
        Metric::Humidity => "2 m relative humidity",
        Metric::Wind => "10 m wind speed",
        Metric::Gust => "Surface wind gust",
        Metric::Cloud => "Total cloud cover",
        Metric::Pressure => "Mean sea level pressure",
        Metric::Cape => "CAPE",
    }
}
fn value(frame: &Frame, m: Metric, id: &str) -> Option<f32> {
    let v = match m {
        Metric::Temperature => frame
            .sample(Field::Temperature, id)
            .map(|x| (x - 273.15) * 1.8 + 32.),
        Metric::Dewpoint => frame
            .sample(Field::Dewpoint, id)
            .map(|x| (x - 273.15) * 1.8 + 32.),
        Metric::Humidity => frame.sample(Field::Humidity, id),
        Metric::Wind => frame.wind_speed_ms(id).map(|x| x * 2.2369363),
        Metric::Gust => frame.sample(Field::Gust, id).map(|x| x * 2.2369363),
        Metric::Cloud => frame.sample(Field::Cloud, id),
        Metric::Pressure => frame.sample(Field::Pressure, id).map(hpa_from_pa),
        Metric::Cape => frame.sample(Field::Cape, id).map(cape_jkg),
    }?;
    v.is_finite().then_some(v)
}
fn hpa_from_pa(pa: f32) -> f32 {
    pa / 100.
}
fn cape_jkg(value: f32) -> f32 {
    value
}
fn band(m: Metric, v: f32) -> usize {
    let limits: &[f32] = match m {
        Metric::Temperature => &[20., 32., 45., 60., 75., 90., 105.],
        Metric::Dewpoint => &[20., 32., 45., 55., 65., 70., 75.],
        Metric::Humidity => &[20., 30., 40., 50., 60., 70., 80.],
        Metric::Wind => &[5., 10., 15., 20., 30., 40., 55.],
        Metric::Gust => &[15., 25., 35., 45., 55., 65., 80.],
        Metric::Cloud => &[10., 25., 40., 55., 70., 85., 95.],
        Metric::Pressure => &[980., 990., 1000., 1010., 1020., 1030., 1040.],
        Metric::Cape => &[100., 500., 1000., 1500., 2000., 3000., 4000.],
    };
    limits.iter().position(|x| v < *x).unwrap_or(7)
}
fn time(e: i64) -> String {
    DateTime::<Utc>::from_timestamp(e, 0)
        .map(|d| d.format("%a %b %d, %H:%M UTC").to_string())
        .unwrap_or_else(|| "Valid time not available yet".into())
}
fn one_panel(scope: &ScopeData, m: Metric, lead: u32, run: Option<i64>) -> Panel {
    let Some(f) = scope.frames.get(&lead) else {
        return Panel {
            title: format!("{} • RAP", metric_label(m)),
            headline: "Data not available yet".into(),
            detail: "Native RAP field unavailable; no model substitution used".into(),
            ..Default::default()
        };
    };
    if run != Some(f.run) || f.valid != f.run + i64::from(f.lead) * 3600 {
        return Panel {
            title: metric_label(m).into(),
            headline: "Data not available yet".into(),
            detail: "RAP run/valid-time verification failed".into(),
            ..Default::default()
        };
    }
    let counties: BTreeMap<_, _> = scope
        .sites
        .iter()
        .filter_map(|s| value(f, m, &s.fips).map(|v| (s.fips.clone(), band(m, v))))
        .collect();
    Panel {
        title: format!("{} • RAP", metric_label(m)),
        valid: time(f.valid),
        detail: format!("Forecast hour +{} • County-centre samples", f.lead),
        counties: (!counties.is_empty()).then_some(counties),
        ..Default::default()
    }
}
fn scene(spec: Spec, snap: Option<&Snapshot>) -> Scene {
    let (legend_title, legend) = bands(spec.field);
    let mut s = Scene {
        reviewed: true,
        scope: Scope::default(),
        palette: board::Palette::Category,
        legend_title: legend_title.into(),
        legend,
        panels: Vec::new(),
    };
    if let Some(data) = snap.and_then(|v| v.scopes.get(cov_key(spec.coverage))) {
        s.scope = data.scope.clone();
        let leads = if spec.layout == Layout::Timeline {
            LEADS.to_vec()
        } else {
            vec![6]
        };
        s.panels = leads
            .into_iter()
            .map(|l| one_panel(data, spec.field, l, snap.and_then(|v| v.run)))
            .collect();
    } else {
        s.panels = (0..spec.layout.panels())
            .map(|i| Panel {
                title: if spec.layout == Layout::Timeline {
                    format!("Frame {} • {}", i + 1, metric_label(spec.field))
                } else {
                    metric_label(spec.field).into()
                },
                headline: "Data not available yet".into(),
                detail: "NOAA RAP data unavailable; no other model substituted".into(),
                ..Default::default()
            })
            .collect();
    }
    s
}
fn prefix(s: Spec) -> String {
    let field = match s.field {
        Metric::Temperature => "temperature",
        Metric::Dewpoint => "dewpoint",
        Metric::Humidity => "humidity",
        Metric::Wind => "wind",
        Metric::Gust => "gust",
        Metric::Cloud => "cloud",
        Metric::Pressure => "pressure",
        Metric::Cape => "cape",
    };
    format!(
        "rap.{field}.{}.{}.{}",
        if s.layout == Layout::Timeline {
            "timeline"
        } else {
            "single"
        },
        cov_key(s.coverage),
        if s.portrait { "portrait" } else { "wide" }
    )
}

fn loop_prefix(s: Spec) -> String {
    let field = match s.field {
        Metric::Temperature => "temperature",
        Metric::Dewpoint => "dewpoint",
        Metric::Humidity => "humidity",
        Metric::Wind => "wind",
        Metric::Gust => "gust",
        Metric::Cloud => "cloud",
        Metric::Pressure => "pressure",
        Metric::Cape => "cape",
    };
    // Broadcast and portrait variants share the exact same supplied frame payloads.
    format!("rap.{field}.loop.{}", cov_key(s.coverage))
}

fn frame_scene(spec: Spec, snap: Option<&Snapshot>, lead: u32) -> Scene {
    let (legend_title, legend) = bands(spec.field);
    let mut scene = Scene {
        reviewed: true,
        scope: Scope::default(),
        palette: board::Palette::Category,
        legend_title: legend_title.into(),
        legend,
        panels: Vec::with_capacity(1),
    };
    if let Some((snapshot, data)) =
        snap.and_then(|v| v.scopes.get(cov_key(spec.coverage)).map(|d| (v, d)))
    {
        scene.scope = data.scope.clone();
        if data.frames.contains_key(&lead) {
            let mut panel = one_panel(data, spec.field, lead, snapshot.run);
            if !panel.available() && panel.headline.is_empty() {
                panel.headline = "Data not available yet".into();
                panel.detail =
                    "No valid RAP county-centre samples at this lead; no model substituted".into();
            }
            // one_panel retains the source frame's valid time when run/lead checks pass. A
            // failed verification deliberately keeps its valid-time label empty.
            scene.panels.push(panel);
        }
    }
    if scene.panels.is_empty() {
        scene.panels.push(Panel {
            title: format!("{} • RAP", metric_label(spec.field)),
            valid: String::new(),
            headline: "Data not available yet".into(),
            detail: "NOAA RAP data unavailable; no other model substituted".into(),
            ..Default::default()
        });
    }
    scene
}

fn opacity_keys(rect: (f32, f32, f32, f32), frame: usize) -> Vec<crate::gfx::model::Keyframe> {
    use crate::gfx::model::Keyframe;
    let (x, y, _, _) = rect;
    let key = |t, active| Keyframe {
        t, x, y, scale: 1.0, rotation: 0.0,
        opacity: if active { 1.0 } else { 0.0 },
        easing: "linear".into(),
    };
    let mut keys = vec![key(0.0, frame == 0)];
    // Paired keys cut instantly. The renderer holds the preceding frame at the
    // exact boundary and selects the next frame immediately afterward.
    for boundary in 1..4 {
        let time = boundary as f32 * FRAME_SECS;
        keys.push(key(time, frame == boundary - 1));
        keys.push(key(time, frame == boundary));
    }
    keys.push(key(LOOP_SECS, frame == 3));
    keys.push(key(LOOP_SECS, frame == 0));
    keys
}

pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = Utc::now().timestamp();
    let snap = store()
        .lock()
        .ok()
        .and_then(|s| s.current.clone())
        .filter(|s| {
            s.home == home_key(settings.home)
                && (0..=MAX_AGE).contains(&(now - s.checked))
                && s.run.is_some_and(|r| (0..=24 * 3600).contains(&(now - r)))
        });
    let scopes: BTreeMap<_, _> = [Coverage::Region, Coverage::State, Coverage::Multi]
        .into_iter()
        .map(|c| (cov_key(c), observed::report_scope(settings, c)))
        .collect();
    for spec in SPECS {
        let p = prefix(*spec);
        let mut sc = scene(*spec, snap.as_deref());
        if snap
            .as_ref()
            .and_then(|v| v.scopes.get(cov_key(spec.coverage)))
            .is_none()
        {
            sc.scope = scopes[cov_key(spec.coverage)].clone();
        }
        let any = sc.panels.iter().any(Panel::available);
        let subtitle = snap
            .as_ref()
            .and_then(|s| s.run)
            .map(|r| format!("NOAA RAP run {} • {}", time(r), cov_label(spec.coverage)))
            .unwrap_or_else(|| "NOAA RAP run unavailable".into());
        let source = if any {
            "Source: NOAA / National Weather Service RAP"
        } else {
            "Source: NOAA / National Weather Service RAP • Data not available yet"
        };
        let note = if any {
            "County-centre model samples • Conditions vary • Forecast guidance, not an official warning"
        } else {
            "RAP fields unavailable from relay • No alternate model substituted"
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
    for spec in LOOP_SPECS.iter().filter(|s| !s.portrait) {
        let p = loop_prefix(*spec);
        let mut any = false;
        for lead in LEADS {
            let mut sc = frame_scene(*spec, snap.as_deref(), lead);
            if snap
                .as_ref()
                .and_then(|v| v.scopes.get(cov_key(spec.coverage)))
                .is_none()
            {
                sc.scope = scopes[cov_key(spec.coverage)].clone();
            }
            any |= sc.panels.iter().any(Panel::available);
            b.set(
                &format!("set.{p}.payload_{lead}"),
                &serde_json::to_string(&sc).unwrap_or_default(),
            );
        }
        let subtitle = snap
            .as_ref()
            .and_then(|s| s.run)
            .map(|r| format!("NOAA RAP run {} • {}", time(r), cov_label(spec.coverage)))
            .unwrap_or_else(|| "NOAA RAP run unavailable".into());
        let source = if any {
            "Source: NOAA / National Weather Service RAP"
        } else {
            "Source: NOAA / National Weather Service RAP • Data not available yet"
        };
        let note = if any {
            "County-centre model samples • Conditions vary • Forecast guidance, not an official warning"
        } else {
            "RAP fields unavailable from relay • No alternate model substituted"
        };
        b.set(&format!("set.{p}.subtitle"), &subtitle);
        b.set(&format!("set.{p}.source"), source);
        b.set(&format!("set.{p}.note"), note);
    }
}

fn build_loop(s: Spec) -> GraphicDoc {
    let p = loop_prefix(s);
    let title = if s.field == Metric::Pressure {
        "RAP Sea-level pressure".to_string()
    } else {
        format!("RAP {}", s.title)
    };
    let mut d = if s.portrait {
        super::reference::social_frame(
            &title,
            "Model output maps",
            &format!("{{{{set.{p}.subtitle}}}}"),
            &format!("{{{{set.{p}.source}}}}"),
            true,
        )
    } else {
        super::comparison::template(&title, "Model output maps", &p, Layout::Single)
    };
    d.duration = 0.0;
    let rect = if s.portrait {
        (54.0, 242.0, 972.0, 1400.0)
    } else {
        (70.0, 205.0, 1780.0, 715.0)
    };
    if s.portrait {
        super::reference::label(
            &mut d,
            "Map context",
            (54.0, 1656.0, 972.0, 68.0),
            &format!("{{{{set.{p}.note}}}}"),
            26.0,
            "$muted",
            Align::Left,
        );
    }
    d.layers
        .retain(|l| !matches!(&l.kind, crate::gfx::model::LayerKind::RiskMap { .. }));
    for (i, lead) in LEADS.iter().enumerate() {
        let mut layer = Layer::risk_map(
            &format!("RAP +{lead} h frame"),
            rect.0,
            rect.1,
            rect.2,
            rect.3,
            &format!("set:board:single:{{{{set.{p}.payload_{lead}}}}}"),
        )
        .with_anim(AnimKind::None, 0.0, 0.0);
        layer.loop_mode = "loop".into();
        layer.loop_secs = LOOP_SECS;
        layer.keys = opacity_keys(rect, i);
        d.layers.push(layer);
    }
    d
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    if let Some(spec) = LOOP_SPECS.iter().find(|s| s.key == key) {
        return Some(build_loop(*spec));
    }
    let s = *SPECS.iter().find(|v| v.key == key)?;
    let p = prefix(s);
    let title = if s.field == Metric::Pressure {
        "RAP Sea-level pressure".to_string()
    } else {
        format!("RAP {}", s.title)
    };
    if !s.portrait {
        return Some(super::comparison::template(
            &title,
            "Model output maps",
            &p,
            s.layout,
        ));
    }
    let mut d = super::reference::social_frame(
        &title,
        "Model output maps",
        &format!("{{{{set.{p}.subtitle}}}}"),
        &format!("{{{{set.{p}.source}}}}"),
        true,
    );
    d.layers.push(
        Layer::risk_map(
            "RAP model map board",
            54.,
            242.,
            972.,
            1400.,
            &format!("set:board:{}:{{{{set.{p}.payload}}}}", s.layout.key()),
        )
        .with_anim(AnimKind::Fade, 0.2, 0.6),
    );
    super::reference::label(
        &mut d,
        "Model context",
        (54., 1656., 972., 68.),
        &format!("{{{{set.{p}.note}}}}"),
        26.,
        "$muted",
        Align::Left,
    );
    Some(d)
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    if let Some(spec) = LOOP_SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
    {
        let settings = Settings::default();
        let scope = observed::report_scope(&settings, spec.coverage);
        let all = crate::map::boundaries::counties_named();
        let selected = all
            .as_ref()
            .map(|v| board::selected(v, &scope))
            .unwrap_or_default();
        let (legend_title, legend) = bands(spec.field);
        let p = loop_prefix(*spec);
        for (i, lead) in LEADS.iter().enumerate() {
            let counties = selected
                .iter()
                .enumerate()
                .map(|(j, c)| (c.fips.clone(), (j + i) % legend.len().max(1)))
                .collect();
            let sc = Scene {
                reviewed: true,
                scope: scope.clone(),
                palette: board::Palette::Category,
                legend_title: format!("SAMPLE • {legend_title}"),
                legend: legend.clone(),
                panels: vec![Panel {
                    title: format!("{} • RAP", metric_label(spec.field)),
                    valid: format!("SAMPLE • Forecast hour +{lead}"),
                    detail: "SAMPLE • Synthetic county-centre categories • Not a forecast".into(),
                    counties: Some(counties),
                    ..Default::default()
                }],
            };
            doc.data_overrides.push((
                format!("set.{p}.payload_{lead}"),
                serde_json::to_string(&sc).unwrap(),
            ));
        }
        for (k, v) in [
            (
                "subtitle",
                "SAMPLE • RAP-style synthetic loop • Not a forecast",
            ),
            ("source", "SAMPLE ONLY • Not for broadcast"),
            (
                "note",
                "SAMPLE • Synthetic county-centre categories • Not a forecast",
            ),
        ] {
            doc.data_overrides.push((format!("set.{p}.{k}"), v.into()));
        }
        return;
    }
    let Some(spec) = SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
    else {
        return;
    };
    let settings = Settings::default();
    let scope = observed::report_scope(&settings, spec.coverage);
    let all = crate::map::boundaries::counties_named();
    let selected = all
        .as_ref()
        .map(|v| board::selected(v, &scope))
        .unwrap_or_default();
    let mut panels = Vec::new();
    let leads = if spec.layout == Layout::Timeline {
        LEADS.to_vec()
    } else {
        vec![6]
    };
    for (i, lead) in leads.into_iter().enumerate() {
        let counties = selected
            .iter()
            .enumerate()
            .map(|(j, c)| (c.fips.clone(), (j + i) % 8))
            .collect();
        panels.push(Panel {
            title: format!("{} • RAP", spec.title),
            valid: format!("SAMPLE • Forecast hour +{lead}"),
            detail: "SAMPLE • Synthetic county-centre categories • Not a forecast".into(),
            counties: Some(counties),
            ..Default::default()
        });
    }
    let (legend_title, legend) = bands(spec.field);
    let scene = Scene {
        reviewed: true,
        scope,
        palette: board::Palette::Category,
        legend_title: format!("SAMPLE • {legend_title}"),
        legend,
        panels,
    };
    let p = prefix(*spec);
    for (k, v) in [
        ("payload", serde_json::to_string(&scene).unwrap()),
        (
            "subtitle",
            "SAMPLE • NOAA RAP-style synthetic sample • Not a forecast".into(),
        ),
        ("source", "SAMPLE ONLY • Not for broadcast".into()),
        (
            "note",
            "SAMPLE • Synthetic county-centre categories • Not a forecast".into(),
        ),
    ] {
        doc.data_overrides.push((format!("set.{p}.{k}"), v));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_frames_never_become_zero_value_forecasts() {
        let sc = scene(SPECS[0], None);
        assert!(sc.panels.iter().all(|p| !p.available()));
        assert!(
            sc.panels
                .iter()
                .all(|p| p.headline == "Data not available yet")
        );
        assert_eq!(LEADS, [6, 12, 18, 21]);
        assert_eq!(SPECS.len(), 96);
        let keys: std::collections::BTreeSet<_> = SPECS.iter().map(|s| s.key).collect();
        assert_eq!(keys.len(), SPECS.len());
        assert_eq!(LOOP_SPECS.len(), 48);
        let loop_keys: std::collections::BTreeSet<_> = LOOP_SPECS.iter().map(|s| s.key).collect();
        assert_eq!(loop_keys.len(), LOOP_SPECS.len());
    }

    #[test]
    fn loop_keyframes_are_hard_cuts_with_one_opaque_frame_and_closed_seam() {
        let rect = (70.0, 205.0, 1780.0, 715.0);
        let make = |i| {
            let mut layer = Layer::risk_map(
                "frame",
                rect.0,
                rect.1,
                rect.2,
                rect.3,
                "set:board:single:{}",
            );
            layer.loop_mode = "loop".into();
            layer.loop_secs = LOOP_SECS;
            layer.keys = opacity_keys(rect, i);
            layer
        };
        let frames: Vec<_> = (0..4).map(make).collect();
        let checks = [
            (0.0, 0),
            (1.25, 0),
            (2.0, 0),
            (2.001, 1),
            (4.0, 1),
            (4.001, 2),
            (6.0, 2),
            (6.001, 3),
            (7.999, 3),
            (8.0, 0),
            (8.001, 0),
            (15.999, 3),
            (16.0, 0),
        ];
        let mut doc = build(LOOP_SPECS[0].key).expect("loop test template");
        doc.layers = frames.clone();
        for (time, expected) in checks {
            let opacities: Vec<_> = frames
                .iter()
                .enumerate()
                .map(|(_, layer)| {
                    let t = doc.layer_time(layer, time);
                    layer.sample_keys(t).unwrap().4
                })
                .collect();
            assert!(
                opacities.iter().all(|v| *v == 0.0 || *v == 1.0),
                "no alpha blending at {time}s: {opacities:?}"
            );
            assert_eq!(
                opacities.iter().sum::<f32>(),
                1.0,
                "exactly one visible frame at {time}s"
            );
            let active: Vec<_> = opacities
                .iter()
                .enumerate()
                .filter_map(|(i, v)| (*v == 1.0).then_some(i))
                .collect();
            assert_eq!(active, vec![expected], "phase at {time}s");
        }
        for layer in &frames {
            assert!(layer.keys.iter().all(|k| k.x == rect.0 && k.y == rect.1));
        }
    }

    #[test]
    fn loop_builder_replays_only_the_map_frames_not_station_chrome() {
        let doc = build(LOOP_SPECS[0].key).expect("loop template");
        let maps: Vec<_> = doc
            .layers
            .iter()
            .filter(|layer| matches!(&layer.kind, crate::gfx::model::LayerKind::RiskMap { .. }))
            .collect();
        assert_eq!(maps.len(), 4);
        assert!(
            maps.iter()
                .all(|layer| layer.loop_mode == "loop" && layer.loop_secs == LOOP_SECS)
        );
        assert!(
            doc.layers
                .iter()
                .filter(|layer| !matches!(
                    &layer.kind,
                    crate::gfx::model::LayerKind::RiskMap { .. }
                ))
                .all(|layer| layer.loop_mode.is_empty())
        );
        assert!(
            maps.iter()
                .all(|layer| layer.anim_in.kind == AnimKind::None)
        );
    }

    #[test]
    fn loop_phase_payloads_remain_frame_specific_and_unavailable_is_calm() {
        let missing = LOOP_SPECS[0];
        for lead in LEADS {
            let sc = frame_scene(missing, None, lead);
            assert_eq!(sc.panels.len(), 1);
            assert!(!sc.panels[0].available());
            assert_eq!(sc.panels[0].headline, "Data not available yet");
            assert!(sc.panels[0].counties.is_none());
        }
        assert_eq!(LOOP_SECS, 8.0);
        assert_eq!(LEADS, [6, 12, 18, 21]);
    }

    #[test]
    fn qa_export_remaining_loop_phases_both_looks() {
        let Ok(dir) = std::env::var("CWX_QA_NEW_DIR") else {
            return;
        };
        let selected: Option<std::collections::BTreeSet<String>> =
            std::env::var("CWX_QA_SET_KEYS").ok().map(|v| {
                v.split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect()
            });
        let out = std::path::PathBuf::from(dir).join("rap-loops-phases");
        std::fs::create_dir_all(&out).unwrap();
        let settings = Settings::default();
        let data = crate::data::WxSnapshot::default();
        for spec in LOOP_SPECS
            .iter()
            .filter(|s| selected.as_ref().is_none_or(|keys| keys.contains(s.key)))
        {
            let Some(mut doc) = build(spec.key) else {
                panic!("missing loop template {}", spec.key)
            };
            doc.template = Some(spec.key.to_string());
            sample(&mut doc);
            for look in ["modern", "classic"] {
                let mut themed = doc.clone();
                crate::gfx::look::apply(&mut themed, look);
                for (phase, t) in [(1, 10_003.0), (2, 10_005.0), (3, 10_007.0)] {
                    let image = crate::program::render_graphic_mode(
                        &themed,
                        &settings,
                        &data,
                        t,
                        themed.width,
                        themed.height,
                        None,
                        crate::gfx::check::PreviewData::Sample,
                    );
                    let path = out.join(format!("{}-{}-phase-{}.png", spec.key, look, phase));
                    image.save_png(path).unwrap();
                }
            }
        }
    }

    #[test]
    fn pressure_and_cape_units_and_half_open_boundaries_are_explicit() {
        assert!((hpa_from_pa(101_325.0) - 1013.25).abs() < 0.001);
        assert_eq!(cape_jkg(1250.0), 1250.0);
        assert_eq!(bands(Metric::Pressure).0, "Mean sea level pressure • hPa");
        assert_eq!(bands(Metric::Cape).0, "CAPE • J/kg");
        for (metric, limits) in [
            (
                Metric::Pressure,
                [980.0, 990.0, 1000.0, 1010.0, 1020.0, 1030.0, 1040.0],
            ),
            (
                Metric::Cape,
                [100.0, 500.0, 1000.0, 1500.0, 2000.0, 3000.0, 4000.0],
            ),
        ] {
            for (index, boundary) in limits.into_iter().enumerate() {
                assert_eq!(
                    band(metric, boundary - 0.01),
                    index,
                    "lower side of {boundary}"
                );
                assert_eq!(
                    band(metric, boundary),
                    index + 1,
                    "upper-open edge {boundary}"
                );
            }
        }
    }
}
