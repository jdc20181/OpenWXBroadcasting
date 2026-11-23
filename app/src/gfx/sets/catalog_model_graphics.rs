//! Parameterized model presentation, independent of data collection.
//! Live inputs are reviewed graphic scenes. Unset input is unavailable; demos are test-only.
use super::{
    board::{self, Band, Layout, Palette, Panel, Scene, Scope},
    observed::{self, Coverage},
    reference,
};
use crate::{
    config::Settings,
    gfx::{
        bind::Bindings,
        model::{AnimKind, GraphicDoc, Keyframe, Layer, LayerKind},
        text::{Align, GlyphCache},
    },
};
use serde::{Deserialize, Serialize};
use tiny_skia::{Mask, Pixmap, Transform};

#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    model: &'static str,
    metric: &'static str,
    view: &'static str,
    scope: &'static str,
    portrait: bool,
}
include!("catalog_model_graphics_specs.rs");
#[derive(Clone, Copy)]
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
include!("catalog_model_graphics_palette.rs");
include!("catalog_model_graphics_fields.rs");
include!("catalog_model_graphics_diagnostics.rs");

#[derive(Serialize, Deserialize)]
struct Input {
    model: String,
    metric: String,
    scope: String,
    scene: Scene,
}

fn layout(s: Spec) -> Layout {
    if s.view == "4-panel timeline" {
        Layout::Timeline
    } else {
        Layout::Single
    }
}
fn is_loop(s: Spec) -> bool {
    s.view == "Animated loop"
}
fn prefix(s: Spec) -> String {
    format!(
        "set.catalogmodel.{}",
        s.key
            .strip_suffix("_portrait")
            .or_else(|| s.key.strip_suffix("_full"))
            .unwrap_or(s.key)
    )
}
fn token(s: Spec, field: &str) -> String {
    format!("{{{{{}.{field}}}}}", prefix(s))
}
fn geo_scope(s: Spec, settings: &Settings) -> Scope {
    if s.scope == "National" {
        return Scope {
            states: vec![],
            county: None,
            bbox: Some([-125., 24., -66., 50.]),
        };
    }
    if settings.coverage.states.is_empty()
        && settings.coverage.counties.is_empty()
        && settings.coverage.home_fips.is_empty()
    {
        return observed::report_scope(
            settings,
            match s.scope {
                "State" => Coverage::State,
                "Multi-state" => Coverage::Multi,
                _ => Coverage::Region,
            },
        );
    }
    let all = crate::map::boundaries::counties_named();
    let state_id = |code: &str| -> Option<String> {
        crate::winter::STATES
            .iter()
            .find(|(id, abbr, _)| *id == code || abbr.eq_ignore_ascii_case(code))
            .map(|(id, _, _)| (*id).to_string())
            .or_else(|| {
                all.as_ref()?
                    .iter()
                    .find(|c| c.state == code)
                    .map(|c| c.state.clone())
            })
    };
    let mut states: Vec<String> = settings
        .coverage
        .states
        .iter()
        .filter_map(|v| state_id(v))
        .collect();
    for c in &settings.coverage.counties {
        if let Some(id) = c.fips.get(..2).and_then(state_id) {
            states.push(id);
        }
    }
    states.sort();
    states.dedup();
    if s.scope == "State" {
        let home = all
            .as_ref()
            .and_then(|a| a.iter().find(|c| c.fips == settings.coverage.home_fips))
            .map(|c| c.state.clone());
        if let Some(id) = home.or_else(|| states.first().cloned()) {
            return Scope {
                states: vec![id],
                county: None,
                bbox: None,
            };
        }
    }
    if s.scope == "Multi-state" && states.len() >= 2 {
        return Scope {
            states,
            county: None,
            bbox: None,
        };
    }
    if s.scope == "Region" {
        let counties: Vec<_> = all
            .as_ref()
            .into_iter()
            .flat_map(|a| a.iter())
            .filter(|c| settings.coverage.counties.iter().any(|v| v.fips == c.fips))
            .collect();
        let mut bounds = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for (x, y) in counties.iter().flat_map(|c| c.rings.iter().flatten()) {
            bounds[0] = bounds[0].min(*x);
            bounds[1] = bounds[1].min(*y);
            bounds[2] = bounds[2].max(*x);
            bounds[3] = bounds[3].max(*y);
        }
        if bounds.iter().all(|v| v.is_finite()) {
            return Scope {
                states: vec![],
                county: None,
                bbox: Some([
                    bounds[0] - 0.2,
                    bounds[1] - 0.2,
                    bounds[2] + 0.2,
                    bounds[3] + 0.2,
                ]),
            };
        }
    }
    observed::report_scope(
        settings,
        match s.scope {
            "State" => Coverage::State,
            "Multi-state" => Coverage::Multi,
            _ => Coverage::Region,
        },
    )
}
fn extended_style(metric: &str) -> Option<(String, Palette, Vec<Band>)> {
    // Presentation categories only. Upstream supplies categories and valid periods;
    // no weather conversion, probabilities or precipitation types are calculated here.
    let rain = |dbz| crate::twc::rain(dbz).expect("fixed TWC table sample");
    let snow = |dbz| crate::twc::snow(dbz).expect("fixed TWC table sample");
    let (title, labels, colors): (&str, Vec<&str>, Vec<[u8; 3]>) = match metric {
        "Precipitable water" => (
            "Precipitable water • in",
            vec![
                "Below 0.25 in",
                "0.25 to <0.5 in",
                "0.5 to <0.75 in",
                "0.75 to <1 in",
                "1 to <1.5 in",
                "1.5 to <2 in",
                "2 to <2.5 in",
                "2.5 in and above",
            ],
            crate::grid::RAIN_TOTAL_STOPS
                .iter()
                .map(|(_, c)| *c)
                .collect(),
        ),
        "250 mb jet stream" => (
            "250 mb wind speed • kt",
            vec![
                "Below 30 kt",
                "30 to <50 kt",
                "50 to <70 kt",
                "70 to <90 kt",
                "90 to <110 kt",
                "110 to <130 kt",
                "130 to <150 kt",
                "150 kt and above",
            ],
            vec![
                [56, 85, 184],
                [56, 125, 208],
                [40, 184, 210],
                [55, 190, 131],
                [214, 219, 69],
                [245, 163, 50],
                [235, 85, 56],
                [181, 38, 67],
            ],
        ),
        "Simulated radar" => (
            "Simulated reflectivity • dBZ • Model guidance",
            vec![
                "Below 10 dBZ",
                "10 to <20 dBZ",
                "20 to <30 dBZ",
                "30 to <40 dBZ",
                "40 to <50 dBZ",
                "50 to <60 dBZ",
                "60 to <70 dBZ",
                "70 dBZ and above",
            ],
            vec![
                [77, 87, 104],
                rain(10.),
                rain(20.),
                rain(30.),
                rain(40.),
                rain(50.),
                rain(60.),
                rain(70.),
            ],
        ),
        "Precipitation type" => (
            "Supplied precipitation type • Category",
            vec![
                "No precipitation",
                "Rain",
                "Snow",
                "Mixed precipitation",
                "Freezing rain",
                "Sleet",
            ],
            vec![
                [77, 87, 104],
                rain(20.),
                snow(20.),
                rain(31.),
                rain(66.),
                snow(75.),
            ],
        ),
        "Probability of 1 in rain" | "Probability of 1 in snow" => (
            if metric == "Probability of 1 in rain" {
                "Probability of 1 in rain • % • Supplied period"
            } else {
                "Probability of 1 in snow • % • Supplied period"
            },
            vec![
                "Below 10%",
                "10 to <20%",
                "20 to <30%",
                "30 to <40%",
                "40 to <50%",
                "50 to <60%",
                "60 to <80%",
                "80% and above",
            ],
            crate::grid::RAIN_TOTAL_STOPS
                .iter()
                .map(|(_, c)| *c)
                .collect(),
        ),
        _ => return None,
    };
    assert_eq!(labels.len(), colors.len());
    Some((
        title.into(),
        Palette::Category,
        labels
            .into_iter()
            .zip(colors)
            .map(|(label, c)| Band {
                label: label.into(),
                color: format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]),
            })
            .collect(),
    ))
}

fn style(metric: &str) -> (String, Palette, Vec<Band>) {
    if let Some(style) = diagnostic_style(metric) {
        return style;
    }
    if let Some(style) = supplied_field_style(metric) {
        return style;
    }
    if let Some(style) = extended_style(metric) {
        return style;
    }
    match metric {
        "Total precipitation" => {
            return (
                "Total precipitation • in".into(),
                Palette::RainTotal,
                vec![],
            )
        }
        "Snowfall 10:1" => {
            return (
                "Snowfall 10:1 • in • Supplied snowfall values".into(),
                Palette::SnowTotal,
                vec![],
            )
        }
        "Snowfall Kuchera" => {
            return (
                "Snowfall Kuchera • in • Supplied snowfall values".into(),
                Palette::SnowTotal,
                vec![],
            )
        }
        "Freezing rain accumulation" => {
            let labels = [
                "<0.05 in",
                "0.05–<0.1 in",
                "0.1–<0.25 in",
                "0.25–<0.5 in",
                "0.5–<0.75 in",
                "0.75–<1 in",
                "1–<1.5 in",
                "1.5+ in",
            ];
            let bands = labels
                .iter()
                .zip(crate::grid::RAIN_TOTAL_STOPS)
                .map(|(label, (_, c))| Band {
                    label: (*label).into(),
                    color: format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]),
                })
                .collect();
            return (
                "Freezing rain accumulation • in".into(),
                Palette::Category,
                bands,
            );
        }
        _ => {}
    }
    let m = match metric {
        "2 m temperature" => Metric::Temperature,
        "Dew point" => Metric::Dewpoint,
        "Relative humidity 2 m" => Metric::Humidity,
        "10 m wind" => Metric::Wind,
        "Wind gusts" => Metric::Gust,
        "Total cloud cover" => Metric::Cloud,
        "Mean sea level pressure" => Metric::Pressure,
        "CAPE" => Metric::Cape,
        _ => unreachable!("unregistered model metric"),
    };
    let (title, bands) = categorical_bands(m);
    (title.into(), Palette::Category, bands)
}
fn empty_input(s: Spec, settings: &Settings, frame: usize) -> Input {
    empty_input_scoped(s, geo_scope(s, settings), frame)
}
fn empty_input_scoped(s: Spec, scope: Scope, frame: usize) -> Input {
    let mut scene = board::empty(layout(s));
    scene.scope = scope;
    let (title, palette, legend) = style(s.metric);
    scene.palette = palette;
    scene.legend_title = title;
    scene.legend = legend;
    let panel_count = scene.panels.len();
    for (i, p) in scene.panels.iter_mut().enumerate() {
        p.title = format!("{} • {}", s.model, s.metric);
        p.valid = if is_loop(s) {
            format!("Frame {} • Forecast time unavailable", frame + 1)
        } else if panel_count > 1 {
            format!("Frame {} • Forecast time unavailable", i + 1)
        } else {
            "Forecast time unavailable".into()
        };
        p.detail = reference::MISSING.into();
    }
    Input {
        model: s.model.into(),
        metric: s.metric.into(),
        scope: s.scope.into(),
        scene,
    }
}
fn scope_matches(s: Spec, scope: &Scope) -> bool {
    if scope.county.is_some() {
        return false;
    }
    match s.scope {
        "National" => scope.bbox == Some([-125., 24., -66., 50.]) && scope.states.is_empty(),
        "State" => scope.bbox.is_none() && scope.states.len() == 1,
        "Multi-state" => scope.bbox.map_or_else(
            || (2..=8).contains(&scope.states.len()),
            |b| (8.0..=20.0).contains(&(b[2] - b[0])) && (4.0..=15.0).contains(&(b[3] - b[1])),
        ),
        _ => scope
            .bbox
            .is_some_and(|b| b[2] - b[0] <= 20. && b[3] - b[1] <= 15.),
    }
}
type DefaultPairs = std::sync::Arc<Vec<(String, String)>>;
fn default_pairs(settings: &Settings) -> DefaultPairs {
    // Cache pure unavailable presentation strings, never weather values. The
    // serialized geographic scopes include every setting used by these defaults.
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::VecDeque<(String, DefaultPairs)>>,
    > = std::sync::OnceLock::new();
    let scopes: Vec<_> = ["Region", "State", "Multi-state", "National"]
        .into_iter()
        .map(|scope| (scope, geo_scope(Spec { scope, ..SPECS[0] }, settings)))
        .collect();
    let signature = serde_json::to_string(&scopes).unwrap();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some((_, pairs)) = cache.iter().find(|(key, _)| key == &signature) {
        return pairs.clone();
    }
    let mut pairs = Vec::new();
    let mut prefixes = std::collections::HashSet::new();
    for &s in SPECS {
        let p = prefix(s);
        if !prefixes.insert(p.clone()) {
            continue;
        }
        for (field, value) in [
            (
                "subtitle",
                format!("{} • {} • Run unavailable", s.scope, s.view),
            ),
            ("source", "Approved model data not available yet".into()),
            (
                "note",
                "Model guidance • Values remain unavailable until approved input is supplied"
                    .into(),
            ),
        ] {
            pairs.push((format!("{p}.{field}"), value));
        }
        let scope = &scopes.iter().find(|(name, _)| *name == s.scope).unwrap().1;
        for frame in 0..if is_loop(s) { 4 } else { 1 } {
            let field = if is_loop(s) {
                format!("payload_{frame}")
            } else {
                "payload".into()
            };
            pairs.push((
                format!("{p}.{field}"),
                serde_json::to_string(&empty_input_scoped(s, scope.clone(), frame)).unwrap(),
            ));
        }
    }
    let pairs = std::sync::Arc::new(pairs);
    cache.push_back((signature, pairs.clone()));
    while cache.len() > 4 {
        cache.pop_front();
    }
    pairs
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    for (key, value) in default_pairs(settings).iter() {
        if b.get(key).is_none() {
            b.set(key, value);
        }
    }
}
fn opacity_keys(rect: (f32, f32, f32, f32), frame: usize) -> Vec<Keyframe> {
    let key = |t, active| Keyframe {
        t,
        x: rect.0,
        y: rect.1,
        scale: 1.,
        rotation: 0.,
        opacity: if active { 1. } else { 0. },
        easing: "linear".into(),
    };
    let mut keys = vec![key(0., frame == 0)];
    for boundary in 1..4 {
        let t = boundary as f32 * 2.;
        keys.push(key(t, frame == boundary - 1));
        keys.push(key(t, frame == boundary));
    }
    keys.push(key(8., frame == 3));
    keys.push(key(8., frame == 0));
    keys
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|s| s.key == key)?;
    // Keep the complete metric on every map panel; use a concise station heading
    // so the shared Classic subtitle has its own space.
    let heading = if s.portrait {
        s.metric
    } else {
        match s.metric {
            "2 m temperature" => "Temperature",
            "Relative humidity 2 m" => "Humidity",
            "Mean sea level pressure" => "MSLP",
            "Total cloud cover" => "Cloud cover",
            "Total precipitation" => "Rain total",
            "Snowfall Kuchera" => "Kuchera snow",
            "Freezing rain accumulation" => "Ice total",
            "Ground (soil) temperature" => "Soil temp",
            "Apparent temperature" => "Apparent temp",
            "850 mb temperature" => "850 mb temp",
            "Hourly precipitation" => "Hourly rain",
            "Precipitation mean" => "Mean rain",
            "Ice accumulation" => "Ice total",
            "Probability of 0.25 in ice" => "0.25 in ice %",
            "Probability of 1 in rain per hour" => "1 in/hr rain %",
            "Probability of 40 dBZ" => "40 dBZ %",
            "Probability of 6 in snow" => "6 in snow %",
            "Probability of precipitation" => "Precip chance",
            "Probability of thunder" => "Thunder %",
            "Snowfall probability" => "Snow chance",
            "Temperature change 24 h" => "24h change",
            "Temperature anomaly" => "Temp anom",
            "500 mb height anomaly" => "Height anom",
            "700 mb vertical motion" => "700 mb omega",
            "Precipitable water" => "PWAT",
            "250 mb jet stream" => "250 mb jet",
            "Precipitation type" => "Precip type",
            "Probability of 1 in rain" => "1 in rain %",
            "Probability of 1 in snow" => "1 in snow %",
            other => other,
        }
    };
    let title = format!("{} • {}", s.model, heading);
    let mut d = if s.portrait {
        reference::social_frame(
            &title,
            "Model output maps",
            &token(s, "subtitle"),
            &token(s, "source"),
            true,
        )
    } else {
        reference::frame(
            &title,
            "Model output maps",
            &token(s, "subtitle"),
            &token(s, "source"),
        )
    };
    let rect = if s.portrait {
        (54., 242., 972., 1400.)
    } else {
        (70., 205., 1780., 715.)
    };
    for frame in 0..if is_loop(s) { 4 } else { 1 } {
        let field = if is_loop(s) {
            format!("payload_{frame}")
        } else {
            "payload".into()
        };
        let mut layer = Layer::risk_map(
            &format!("Model field frame {frame}"),
            rect.0,
            rect.1,
            rect.2,
            rect.3,
            &format!("set:catalogmodel:{}:{}", s.key, token(s, &field)),
        )
        .with_anim(AnimKind::Fade, 0.2, 0.5);
        if is_loop(s) {
            layer.anim_in.kind = AnimKind::None;
            layer.loop_mode = "loop".into();
            layer.loop_secs = 8.;
            layer.keys = opacity_keys(rect, frame);
        }
        d.layers.push(layer);
    }
    reference::label(
        &mut d,
        "Model guidance context",
        if s.portrait {
            (54., 1656., 972., 68.)
        } else {
            (80., 936., 1760., 44.)
        },
        &token(s, "note"),
        if s.portrait { 26. } else { 27. },
        "$muted",
        Align::Left,
    );
    for l in &mut d.layers {
        // The Classic title and run caption share one band. Keep separate boxes
        // for the new diagnostic headings even with a wider station font.
        if !s.portrait && diagnostic_style(s.metric).is_some() && l.name == "Title" {
            l.w = 980.;
        }
        if let LayerKind::Text { auto_fit, wrap, .. } = &mut l.kind {
            *auto_fit = true;
            *wrap = true;
        }
    }
    if is_loop(s) {
        d.duration = 0.;
    }
    d.template = Some(s.key.into());
    Some(d)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    pm: &mut Pixmap,
    g: &mut GlyphCache,
    local: Transform,
    mask: Option<&Mask>,
    w: f32,
    h: f32,
    spec: &str,
    opacity: f32,
    font: &str,
) -> bool {
    let Some(rest) = spec.strip_prefix("set:catalogmodel:") else {
        return false;
    };
    let Some((key, raw)) = rest.split_once(':') else {
        return true;
    };
    let Some(&s) = SPECS.iter().find(|s| s.key == key) else {
        return true;
    };
    let valid = if raw.len() <= 1_000_000 {
        serde_json::from_str::<Input>(raw).ok()
    } else {
        None
    };
    let mut sc = valid
        .filter(|p| {
            p.model == s.model
                && p.metric == s.metric
                && p.scope == s.scope
                && scope_matches(s, &p.scene.scope)
        })
        .map(|p| p.scene)
        .unwrap_or_else(|| board::empty(layout(s)));
    let (title, palette, legend) = style(s.metric);
    sc.legend_title = title;
    sc.palette = palette;
    sc.legend = legend;
    let board_spec = format!(
        "set:board:{}:{}",
        layout(s).key(),
        serde_json::to_string(&sc).unwrap()
    );
    board::draw(pm, g, local, mask, w, h, &board_spec, opacity, font)
}

#[cfg(test)]
pub(super) fn sample(d: &mut GraphicDoc) {
    sample_with_settings(d, &Settings::default());
}
#[cfg(test)]
fn sample_with_settings(d: &mut GraphicDoc, settings: &Settings) {
    let Some(&s) = SPECS.iter().find(|s| Some(s.key) == d.template.as_deref()) else {
        return;
    };
    let all = crate::map::boundaries::counties_named().expect("existing geographic cache");
    for frame in 0..if is_loop(s) { 4 } else { 1 } {
        let mut p = empty_input(s, settings, frame);
        p.scene.reviewed = true;
        p.scene.legend_title = format!("SAMPLE • {}", p.scene.legend_title);
        let n = board::bands(&p.scene).len();
        let selected = board::selected(&all, &p.scene.scope);
        for (i, panel) in p.scene.panels.iter_mut().enumerate() {
            let phase = if is_loop(s) { frame } else { i };
            let lead = if s.model == "RAP" {
                [6, 12, 18, 21][phase]
            } else {
                [6, 12, 18, 24][phase]
            };
            let valid = match lead {
                6 => "Nov 13 06Z",
                12 => "Nov 13 12Z",
                18 => "Nov 13 18Z",
                21 => "Nov 13 21Z",
                _ => "Nov 14 00Z",
            };
            panel.valid = format!("SAMPLE • Valid {valid} (+{lead}h)");
            panel.detail = if matches!(s.metric, "Temperature anomaly" | "500 mb height anomaly") {
                "SAMPLE ONLY • Departure from synthetic reference • Not a climate baseline".into()
            } else if s.metric == "Temperature change 24 h" {
                "SAMPLE ONLY • Supplied 24-hour change • Synthetic categories".into()
            } else if s.metric == "700 mb vertical motion" {
                "SAMPLE ONLY • Supplied pressure-coordinate omega • Synthetic categories".into()
            } else if matches!(s.metric, "Daily high" | "Daily low") {
                let (start, end) = [("Nov 13", "Nov 14"), ("Nov 14", "Nov 15"), ("Nov 15", "Nov 16"), ("Nov 16", "Nov 17")][phase];
                panel.valid = format!("SAMPLE • Day {start} 00Z to {end} 00Z");
                format!("SAMPLE ONLY • Supplied daily period • Synthetic categories")
            } else if s.metric == "Ground (soil) temperature" {
                "SAMPLE ONLY • Depth 0–10 cm • Synthetic categories • Not a forecast".into()
            } else if matches!(s.metric, "Hourly precipitation" | "Probability of 1 in rain per hour") {
                format!("SAMPLE ONLY • One hour ending {valid} • Synthetic categories")
            } else if s.metric == "Snowfall probability" {
                format!("SAMPLE ONLY • At least 1 in snow • Nov 13 00Z to {valid}")
            } else if s.metric == "Probability of precipitation" {
                format!("SAMPLE ONLY • At least 0.01 in • Nov 13 00Z to {valid}")
            } else if s.metric == "Probability of thunder" {
                format!("SAMPLE ONLY • Thunder occurrence • Nov 13 00Z to {valid}")
            } else if matches!(s.metric, "Probability of 0.25 in ice" | "Probability of 40 dBZ" | "Probability of 6 in snow") {
                format!("SAMPLE ONLY • Period Nov 13 00Z to {valid} • Synthetic categories")
            } else if matches!(s.metric, "Precipitation mean" | "Ice accumulation" | "Snowfall") {
                format!("SAMPLE ONLY • Nov 13 00Z to {valid} • Supplied method • Synthetic categories")
            } else if matches!(
                s.metric,
                "Probability of 1 in rain" | "Probability of 1 in snow"
            ) {
                format!("SAMPLE ONLY • Accum Nov 13 00Z to {valid} • Synthetic categories • Not a forecast")
            } else {
                "SAMPLE ONLY • Synthetic county categories • Not a forecast".into()
            };
            panel.counties = Some(
                selected
                    .iter()
                    .enumerate()
                    .map(|(j, c)| (c.fips.clone(), (j + phase) % n))
                    .collect(),
            );
        }
        let field = if is_loop(s) {
            format!("payload_{frame}")
        } else {
            "payload".into()
        };
        d.data_overrides.push((
            format!("{}.{field}", prefix(s)),
            serde_json::to_string(&p).unwrap(),
        ));
    }
    for (field, value) in [
        (
            "subtitle",
            format!("SAMPLE • {} • Run Nov 13, 2026 00Z", s.scope),
        ),
        (
            "source",
            "SAMPLE ONLY • Synthetic model fields • Not for broadcast".into(),
        ),
        (
            "note",
            "SAMPLE ONLY • Synthetic county-centre categories • Not a forecast".into(),
        ),
    ] {
        d.data_overrides
            .push((format!("{}.{field}", prefix(s)), value));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_legends_keep_signed_units_and_do_not_alias_temperature() {
        for (metric, unit) in [("Temperature change 24 h", "°F"), ("Temperature anomaly", "°F"),
            ("500 mb height anomaly", " m"), ("700 mb vertical motion", "Pa/s"), ("Visibility", "mi")] {
            let (title, palette, bands) = style(metric);
            assert_eq!(palette, Palette::Category);
            assert!(title.contains(unit));
            assert_eq!(bands.len(), 8);
            assert!(bands.iter().all(|b| b.label.contains(unit)));
            assert_eq!(bands.iter().map(|b| &b.label).collect::<std::collections::HashSet<_>>().len(), 8);
            assert_ne!(title, style("2 m temperature").0);
        }
        assert!(style("700 mb vertical motion").0.contains("Negative = rising"));
        assert!(style("500 mb height anomaly").0.contains("Supplied baseline"));
        assert!(style("Visibility").2[0].label.contains("0.25 mi"));
        for &s in SPECS.iter().filter(|s| !s.portrait && diagnostic_style(s.metric).is_some()) {
            let d = build(s.key).unwrap();
            let title = d.layers.iter().find(|l| l.name == "Title").unwrap();
            let caption = d.layers.iter().find(|l| l.name == "Header right").unwrap();
            assert!(title.x + title.w <= caption.x);
        }
        for metric in ["Temperature change 24 h", "Temperature anomaly", "500 mb height anomaly", "700 mb vertical motion"] {
            assert!(style(metric).2[0].label.contains('-'));
        }
    }
    #[test]
    fn extended_field_legends_preserve_units_and_twc_precipitation_colors() {
        for metric in [
            "Precipitable water",
            "250 mb jet stream",
            "Simulated radar",
            "Precipitation type",
            "Probability of 1 in rain",
            "Probability of 1 in snow",
        ] {
            let (title, palette, bands) = style(metric);
            assert_eq!(palette, Palette::Category);
            assert!(!title.is_empty() && (6..=8).contains(&bands.len()));
            assert_eq!(
                bands
                    .iter()
                    .map(|b| &b.label)
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                bands.len()
            );
            let mut scene = board::empty(Layout::Single);
            scene.legend_title = title;
            scene.legend = bands;
            assert!(
                board::parse(&serde_json::to_string(&scene).unwrap(), Layout::Single).is_some()
            );
        }
        let radar = style("Simulated radar").2;
        for (band, dbz) in radar
            .iter()
            .skip(1)
            .zip([10., 20., 30., 40., 50., 60., 70.])
        {
            let c = crate::twc::rain(dbz).unwrap();
            assert_eq!(band.color, format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]));
        }
        assert!(style("Probability of 1 in snow")
            .0
            .contains("Supplied period"));
        assert!(style("250 mb jet stream").0.contains("kt"));
    }
    #[test]
    fn variants_are_unique_native_and_inputs_are_missing_by_default() {
        let mut keys = std::collections::HashSet::new();
        assert_eq!(SPECS.len(), 2552);
        for &s in SPECS {
            assert!(keys.insert(s.key));
            let d = build(s.key).unwrap();
            assert_eq!(
                (d.width, d.height),
                if s.portrait {
                    (1080, 1920)
                } else {
                    (1920, 1080)
                }
            );
            assert!(d.layers.iter().all(|l| l.x >= 0.
                && l.y >= 0.
                && l.x + l.w <= d.width as f32
                && l.y + l.h <= d.height as f32));
            let p = empty_input(s, &Settings::default(), 0);
            assert!(!p.scene.reviewed && p.scene.panels.iter().all(|p| !p.available()));
            assert!(board::parse(&serde_json::to_string(&p.scene).unwrap(), layout(s)).is_some());
            assert!(scope_matches(s, &p.scene.scope));
        }
    }
    #[test]
    fn loops_hold_and_only_one_opaque_map_frame_is_selected() {
        for &s in SPECS.iter().filter(|s| is_loop(**s)) {
            let d = build(s.key).unwrap();
            assert_eq!(d.duration, 0.);
            let layers: Vec<_> = d
                .layers
                .iter()
                .filter(|l| matches!(l.kind, LayerKind::RiskMap { .. }))
                .collect();
            assert_eq!(layers.len(), 4);
            for t in [0.1, 2.1, 4.1, 6.1, 7.99] {
                let mut opaque = 0;
                for l in &layers {
                    let mut value = l.keys[0].opacity;
                    for k in &l.keys {
                        if k.t <= t {
                            value = k.opacity;
                        }
                    }
                    opaque += usize::from(value == 1.);
                    assert!(value == 0. || value == 1.);
                }
                assert_eq!(opaque, 1);
            }
        }
    }
    #[test]
    fn precipitation_palettes_are_twc_and_producer_inputs_survive_defaults() {
        assert_eq!(style("Total precipitation").1, Palette::RainTotal);
        assert_eq!(style("Snowfall Kuchera").1, Palette::SnowTotal);
        let s = SPECS[0];
        let mut b = Bindings::default();
        let k = format!("{}.source", prefix(s));
        b.set(&k, "Supplied approved source");
        bind(&mut b, &Settings::default());
        assert_eq!(b.get(&k), Some("Supplied approved source"));
        let national = *SPECS.iter().find(|s| s.scope == "National").unwrap();
        assert!(!scope_matches(national, &Scope::default()));
    }
    fn alternate_coverage() -> Settings {
        let mut settings = Settings::default();
        settings.coverage.states = vec!["KY".into(), "TN".into()];
        settings.coverage.home_fips = "21067".into();
        settings.coverage.counties = ["21067", "21113"]
            .into_iter()
            .map(|fips| crate::config::CoverageCounty {
                fips: fips.into(),
                ..Default::default()
            })
            .collect();
        settings
    }
    #[test]
    fn configured_coverage_drives_state_region_multi_and_cache_invalidation() {
        let settings = alternate_coverage();
        let spec = |scope| Spec { scope, ..SPECS[0] };
        assert_eq!(geo_scope(spec("State"), &settings).states, vec!["21"]);
        assert_eq!(
            geo_scope(spec("Multi-state"), &settings).states,
            vec!["21", "47"]
        );
        let region = geo_scope(spec("Region"), &settings);
        assert!(region.county.is_none());
        let [w, south, e, north] = region.bbox.unwrap();
        let all = crate::map::boundaries::counties_named().unwrap();
        for fips in ["21067", "21113"] {
            let county = all
                .iter()
                .find(|c| c.fips == fips)
                .expect("existing Kentucky geography");
            assert!(county
                .rings
                .iter()
                .flatten()
                .all(|(x, y)| *x >= w && *x <= e && *y >= south && *y <= north));
        }
        let mut original = Bindings::default();
        bind(&mut original, &Settings::default());
        let mut changed = Bindings::default();
        bind(&mut changed, &settings);
        let s = *SPECS
            .iter()
            .find(|s| s.scope == "State" && !s.portrait && !is_loop(**s))
            .unwrap();
        let token = format!("{}.payload", prefix(s));
        assert_ne!(original.get(&token), changed.get(&token));
        let parsed: Input = serde_json::from_str(changed.get(&token).unwrap()).unwrap();
        assert_eq!(parsed.scene.scope.states, vec!["21"]);
    }
    #[test]
    fn qa_configured_coverage() {
        let Some(root) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) else {
            return;
        };
        let out = root.join("model-configured-coverage");
        std::fs::create_dir_all(&out).unwrap();
        let settings = alternate_coverage();
        for &s in SPECS.iter().filter(|s| {
            s.model == "RRFS" && s.metric == "2 m temperature" && s.view == "Single time"
        }) {
            for look in ["modern", "classic"] {
                for state in ["sample", "unavailable"] {
                    let mut doc = crate::gfx::templates::build_in(s.key, look).unwrap();
                    if state == "sample" {
                        sample_with_settings(&mut doc, &settings);
                    }
                    let pm = crate::program::render_graphic_mode(
                        &doc,
                        &settings,
                        &crate::data::WxSnapshot::default(),
                        10_000.,
                        doc.width,
                        doc.height,
                        None,
                        crate::gfx::check::PreviewData::Sample,
                    );
                    pm.save_png(out.join(format!("{}-{look}-{state}.png", s.key)))
                        .unwrap();
                }
            }
        }
    }
    #[test]
    fn cached_unavailable_defaults_follow_viewport_changes_and_preserve_exact_inputs() {
        for offset in [0., 0.1, 0.2, 0.3, 0.4, 0.] {
            let mut settings = Settings::default();
            settings.home.lon += offset;
            let mut b = Bindings::default();
            bind(&mut b, &settings);
            for &s in SPECS {
                for field in ["subtitle", "source", "note"] {
                    assert!(
                        b.get(&format!("{}.{field}", prefix(s))).is_some(),
                        "{} missing {field}",
                        s.key
                    );
                }
                for frame in 0..if is_loop(s) { 4 } else { 1 } {
                    let field = if is_loop(s) {
                        format!("payload_{frame}")
                    } else {
                        "payload".into()
                    };
                    let expected =
                        serde_json::to_string(&empty_input(s, &settings, frame)).unwrap();
                    assert_eq!(
                        b.get(&format!("{}.{field}", prefix(s))),
                        Some(expected.as_str())
                    );
                }
            }
        }
    }
    #[test]
    fn qa_loop_phases() {
        let Ok(dir) = std::env::var("CWX_QA_NEW_DIR") else {
            return;
        };
        let selected = std::env::var("CWX_QA_SET_KEYS").unwrap_or_default();
        let out = std::path::PathBuf::from(dir).join("bulk-model-phases");
        std::fs::create_dir_all(&out).unwrap();
        for s in SPECS
            .iter()
            .filter(|s| is_loop(**s) && selected.split(',').any(|k| k == s.key))
        {
            for look in ["modern", "classic"] {
                let mut d = crate::gfx::templates::build_in(s.key, look).unwrap();
                sample(&mut d);
                for (phase, t) in [(1, 10003.), (2, 10005.), (3, 10007.)] {
                    let im = crate::program::render_graphic_mode(
                        &d,
                        &Settings::default(),
                        &crate::data::WxSnapshot::default(),
                        t,
                        d.width,
                        d.height,
                        None,
                        crate::gfx::check::PreviewData::Sample,
                    );
                    im.save_png(out.join(format!("{}-{look}-phase-{phase}.png", s.key)))
                        .unwrap();
                }
            }
        }
    }
}
