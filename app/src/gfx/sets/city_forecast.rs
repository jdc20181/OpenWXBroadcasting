//! NWS forecasts for a bounded list of real towns inside the selected area.
use super::{
    arcgis_geometry::{self, Rings},
    board,
    observed::{self, Coverage},
    reference::{frame, social_frame},
};
use crate::{
    config::{GeoView, Settings},
    gfx::{
        bind::Bindings,
        model::{AnimKind, GraphicDoc, Layer, LayerKind},
        templates::{BODY, DISPLAY, panel, text, wrapped},
        text::{Align, Weight},
    },
    map::boundaries::County,
    places::Place,
};
use futures_util::{StreamExt, stream};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tiny_skia::{Mask, Pixmap, PixmapPaint, Transform};

#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    day: u8,
    coverage: Coverage,
    format: u8,
}
include!("city_forecast_specs.rs");

const MAX_TOWNS: usize = 6;
const MAX_AGE: i64 = 20 * 60;
const MISSING_DATA: &str = "Data not available yet";

#[derive(Clone, Debug)]
struct Town {
    name: String,
    state: String,
    point: GeoView,
}
impl Town {
    fn label(&self) -> String {
        format!("{}, {}", self.name, self.state)
    }
    fn key(&self) -> (u64, u64) {
        (self.point.lon.to_bits(), self.point.lat.to_bits())
    }
}
struct TownResult {
    town: Town,
    forecast: Option<Arc<super::social_forecast::CitySnapshot>>,
}
struct Snapshot {
    home: (u64, u64),
    checked: i64,
    scopes: BTreeMap<&'static str, Vec<TownResult>>,
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
fn home_key(home: GeoView) -> (u64, u64) {
    (home.lon.to_bits(), home.lat.to_bits())
}
fn coverage_key(c: Coverage) -> &'static str {
    match c {
        Coverage::County => "county",
        Coverage::Region => "region",
        Coverage::State => "state",
        Coverage::Multi => "multi",
    }
}
fn place_state_matches(place: &Place, county_state: &str) -> bool {
    let Some((_, postal, _)) = crate::winter::STATES
        .iter()
        .find(|(fips, _, _)| *fips == county_state)
    else {
        return false;
    };
    place.state.is_empty() || place.state.eq_ignore_ascii_case(postal)
}
#[derive(Clone, Copy)]
struct CountyShape<'a> {
    state: &'a str,
    rings: &'a Rings,
    bounds: [f64; 4],
}
fn bounds(rings: &Rings) -> Option<[f64; 4]> {
    let points = rings
        .iter()
        .flatten()
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .collect::<Vec<_>>();
    if points.is_empty() {
        return None;
    }
    Some([
        points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min),
        points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min),
        points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max),
        points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max),
    ])
}
fn selected_counties<'a>(all: &'a [County], scope: &board::Scope) -> Vec<CountyShape<'a>> {
    board::selected(all, scope)
        .into_iter()
        .filter_map(|c| {
            Some(CountyShape {
                state: &c.state,
                rings: &c.rings,
                bounds: bounds(&c.rings)?,
            })
        })
        .collect()
}
fn unique_home_county<'a>(home: GeoView, all: &'a [County]) -> Option<&'a County> {
    let mut found = None;
    for county in all {
        if arcgis_geometry::contains(&county.rings, home.lon, home.lat) == Some(true) {
            if found.is_some() {
                return None;
            }
            found = Some(county);
        }
    }
    found
}
fn scope_for_home(home: GeoView, coverage: Coverage, all: &[County]) -> Option<board::Scope> {
    let county = unique_home_county(home, all)?;
    let mut settings = Settings::default();
    settings.home = home;
    let scope = observed::report_scope(&settings, coverage);
    match coverage {
        Coverage::County if scope.county.as_deref() != Some(county.fips.as_str()) => None,
        Coverage::State if !scope.states.iter().any(|s| s == &county.state) => None,
        _ => Some(scope),
    }
}
fn town_inside(
    place: &Place,
    scope: &board::Scope,
    counties: &[CountyShape<'_>],
) -> Option<String> {
    if !place.lat.is_finite()
        || !place.lon.is_finite()
        || !(-90.0..=90.0).contains(&place.lat)
        || !(-180.0..=180.0).contains(&place.lon)
    {
        return None;
    }
    if scope.bbox.is_some_and(|[w, s, e, n]| {
        place.lon < w || place.lon > e || place.lat < s || place.lat > n
    }) {
        return None;
    }
    counties.iter().find_map(|county| {
        let [w, s, e, n] = county.bounds;
        (place.lon >= w
            && place.lon <= e
            && place.lat >= s
            && place.lat <= n
            && place_state_matches(place, county.state)
            && arcgis_geometry::contains(county.rings, place.lon, place.lat) == Some(true))
        .then(|| {
            crate::winter::STATES
                .iter()
                .find(|(fips, _, _)| *fips == county.state)
                .map(|(_, postal, _)| (*postal).to_owned())
        })
        .flatten()
    })
}
fn rank_and_limit(mut towns: Vec<(u32, Town)>, limit: usize) -> Vec<Town> {
    towns.sort_by(|(ap, a), (bp, b)| {
        bp.cmp(ap)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.state.cmp(&b.state))
            .then_with(|| a.point.lat.total_cmp(&b.point.lat))
            .then_with(|| a.point.lon.total_cmp(&b.point.lon))
    });
    let mut seen = std::collections::HashSet::new();
    towns
        .into_iter()
        .filter_map(|(_, town)| seen.insert(town.key()).then_some(town))
        .take(limit)
        .collect()
}
fn candidates(home: GeoView, coverage: Coverage) -> Vec<Town> {
    let Some(all_counties) = crate::map::boundaries::counties_named() else {
        return Vec::new();
    };
    let Some(scope) = scope_for_home(home, coverage, &all_counties) else {
        return Vec::new();
    };
    let counties = selected_counties(&all_counties, &scope);
    if counties.is_empty() {
        return Vec::new();
    }
    let mut towns = Vec::new();
    if let Some([w, s, e, n]) = scope.bbox {
        for place in crate::places::in_box(s, n, w, e) {
            let Some(state) = town_inside(&place, &scope, &counties) else {
                continue;
            };
            if place.name.trim().is_empty() || place.name.chars().count() > 70 {
                continue;
            }
            towns.push((
                place.population,
                Town {
                    name: place.name.trim().to_owned(),
                    state,
                    point: GeoView {
                        lon: place.lon,
                        lat: place.lat,
                        zoom: home.zoom,
                    },
                },
            ));
        }
    } else {
        let places = crate::places::all();
        for place in places.iter() {
            let Some(state) = town_inside(place, &scope, &counties) else {
                continue;
            };
            if place.name.trim().is_empty() || place.name.chars().count() > 70 {
                continue;
            }
            towns.push((
                place.population,
                Town {
                    name: place.name.trim().to_owned(),
                    state,
                    point: GeoView {
                        lon: place.lon,
                        lat: place.lat,
                        zoom: home.zoom,
                    },
                },
            ));
        }
    }
    rank_and_limit(towns, MAX_TOWNS)
}
fn coverage_label(home: GeoView, coverage: Coverage) -> String {
    // Immutable county identity/names only: no weather is cached here.
    type Entry = ((u64, u64), String, String);
    static LABELS: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
    let cache = LABELS.get_or_init(Default::default);
    let key = home_key(home);
    let label = |county: &str, state: &str| match coverage {
        Coverage::County => county.to_owned(),
        Coverage::Region => "Home region • selected county geography".into(),
        Coverage::State => state.to_owned(),
        Coverage::Multi => "Selected states".into(),
    };
    if let Some(value) = cache.lock().ok().and_then(|items| {
        items.iter().find(|(k, _, _)| *k == key).map(|(_, c, s)| label(c, s))
    }) { return value; }
    let Some(all) = crate::map::boundaries::counties_named() else {
        return "Selected area unavailable".into();
    };
    let Some(county) = unique_home_county(home, &all) else {
        return "Selected area unavailable".into();
    };
    let Some((_, postal, state)) = crate::winter::STATES.iter().find(|(f, _, _)| *f == county.state) else {
        return "Selected area unavailable".into();
    };
    let county = format!("{} County, {}", county.name, postal);
    let value = label(&county, state);
    if let Ok(mut items) = cache.lock() {
        if items.len() >= 8 { items.remove(0); }
        items.push((key, county, (*state).to_owned()));
    }
    value
}
pub(super) async fn refresh(home: GeoView) {
    let ticket = match store().lock() {
        Ok(mut s) => {
            s.generation += 1;
            s.generation
        }
        Err(_) => return,
    };
    let mut scopes = BTreeMap::new();
    let mut unique = BTreeMap::<(u64, u64), Town>::new();
    for coverage in [Coverage::County, Coverage::Region, Coverage::State] {
        let towns = candidates(home, coverage);
        for town in &towns {
            unique.entry(town.key()).or_insert_with(|| town.clone());
        }
        scopes.insert(coverage_key(coverage), towns);
    }
    let jobs = unique.into_values().collect::<Vec<_>>();
    let mut fetched =
        HashMap::<(u64, u64), Option<Arc<super::social_forecast::CitySnapshot>>>::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let mut pending = stream::iter(jobs.into_iter().map(|town| async move {
        let result = super::social_forecast::fetch_city(town.point).await;
        (town.key(), result)
    }))
    .buffer_unordered(4);
    loop {
        match tokio::time::timeout_at(deadline, pending.next()).await {
            Ok(Some((key, result))) => {
                fetched.insert(key, result.map(Arc::new));
            }
            Ok(None) | Err(_) => break,
        }
    }
    let scopes = scopes
        .into_iter()
        .map(|(key, towns)| {
            let rows = towns
                .into_iter()
                .map(|town| {
                    let forecast = fetched.get(&town.key()).cloned().flatten();
                    TownResult { town, forecast }
                })
                .collect();
            (key, rows)
        })
        .collect();
    let snapshot = Arc::new(Snapshot {
        home: home_key(home),
        checked: chrono::Utc::now().timestamp(),
        scopes,
    });
    if let Ok(mut s) = store().lock() {
        if s.generation == ticket {
            s.current = Some(snapshot);
        }
    }
}

fn day_rows(
    snapshot: Option<&Arc<Snapshot>>,
    home: GeoView,
    coverage: Coverage,
    day: u8,
    now: i64,
) -> Vec<(Town, Option<super::social_forecast::CityDay>)> {
    let Some(snapshot) =
        snapshot.filter(|s| s.home == home_key(home) && (0..=MAX_AGE).contains(&(now - s.checked)))
    else {
        return Vec::new();
    };
    snapshot
        .scopes
        .get(coverage_key(coverage))
        .into_iter()
        .flatten()
        .map(|row| {
            (
                row.town.clone(),
                row.forecast
                    .as_ref()
                    .and_then(|f| super::social_forecast::city_day(f, day, now)),
            )
        })
        .collect()
}
fn period_label(day: u8) -> &'static str {
    match day {
        0 => "Today",
        1 => "Tomorrow",
        _ => "Day 3",
    }
}
fn icon_key(condition: &str) -> &'static str {
    let c = condition.to_ascii_lowercase();
    if c.contains("thunder") || c.contains("lightning") {
        "thunder"
    } else if c.contains("hail") {
        "hail"
    } else if c.contains("snow")
        || c.contains("sleet")
        || c.contains("freezing")
        || c.contains("ice")
    {
        "snow"
    } else if c.contains("rain") || c.contains("shower") || c.contains("drizzle") {
        "rain"
    } else if c.contains("wind") {
        "wind"
    } else if c.contains("cloud") || c.contains("overcast") || c.contains("fog") {
        "cloud"
    } else if c.contains("sun") || c.contains("clear") || c.contains("fair") {
        "suncloud"
    } else {
        "none"
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Cells {
    name: String,
    condition: String,
    high: String,
    low: String,
    icon: &'static str,
}
fn cells(town: &Town, value: Option<&super::social_forecast::CityDay>) -> Cells {
    let Some(value) = value else {
        return Cells {
            name: town.label(),
            condition: MISSING_DATA.into(),
            high: MISSING_DATA.into(),
            low: MISSING_DATA.into(),
            icon: "none",
        };
    };
    Cells {
        name: town.label(),
        icon: {
            let icon = icon_key(&value.condition);
            if value.remaining_night && icon == "suncloud" {
                if value.condition.to_ascii_lowercase().contains("cloud") { "cloud" } else { "none" }
            } else { icon }
        },
        condition: format!("{} • {}", value.condition, value.date),
        high: value.high.clone(),
        low: value.low.clone(),
    }
}
fn binding_prefix(spec: &Spec) -> String {
    format!(
        "set.cityforecast.{}.{}",
        coverage_key(spec.coverage),
        spec.day
    )
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let snapshot = store().lock().ok().and_then(|s| s.current.clone());
    for spec in SPECS.iter().filter(|s| s.format == 0) {
        let rows = day_rows(
            snapshot.as_ref(),
            settings.home,
            spec.coverage,
            spec.day,
            now,
        );
        let some_data = rows.iter().any(|(_, value)| value.is_some());
        let prefix = binding_prefix(spec);
        let scope = coverage_label(settings.home, spec.coverage);
        b.set(&format!("{prefix}.scope"), &scope);
        b.set(
            &format!("{prefix}.period"),
            &format!("{} • NWS local time at each town", period_label(spec.day)),
        );
        b.set(
            &format!("{prefix}.source"),
            if some_data {
                "Source: NOAA / National Weather Service • Listed town point forecasts"
            } else {
                "Source: NOAA / National Weather Service • Awaiting town forecasts"
            },
        );
        let remaining = rows
            .iter()
            .any(|(_, v)| v.as_ref().is_some_and(|d| d.remaining_night));
        let note = if rows.is_empty() {
            "Selected-town forecasts are not available yet • No area-wide conditions inferred"
        } else if remaining {
            "Up to six selected towns • Remaining night where daytime ended; daytime highs withheld"
        } else {
            "Up to six selected towns • Not a complete area forecast"
        };
        b.set(&format!("{prefix}.note"), note);
        let issue = rows
            .iter()
            .find_map(|(_, v)| v.as_ref().map(|d| d.issue.as_str()))
            .unwrap_or("");
        b.set(&format!("{prefix}.issue"), issue);
        for i in 0..MAX_TOWNS {
            let row = rows.get(i).map(|(town, value)| cells(town, value.as_ref()));
            for (field, value) in [
                ("name", row.as_ref().map(|c| c.name.as_str()).unwrap_or("")),
                (
                    "condition",
                    row.as_ref().map(|c| c.condition.as_str()).unwrap_or(""),
                ),
                ("high", row.as_ref().map(|c| c.high.as_str()).unwrap_or("")),
                ("low", row.as_ref().map(|c| c.low.as_str()).unwrap_or("")),
            ] {
                b.set(&format!("{prefix}.town.{i}.{field}"), value);
            }
            b.set(
                &format!("{prefix}.town.{i}.icon"),
                row.as_ref().map(|c| c.icon).unwrap_or("none"),
            );
        }
    }
}
fn token(spec: &Spec, i: usize, field: &str) -> String {
    format!("{{{{{}.town.{i}.{field}}}}}", binding_prefix(spec))
}
fn text_layer(
    name: &str,
    rect: (f32, f32, f32, f32),
    value: &str,
    size: f32,
    font: &str,
    color: &str,
    align: Align,
) -> Layer {
    let mut layer = wrapped(text(
        name,
        rect,
        value,
        font,
        size,
        Weight::SemiBold,
        color,
        align,
    ));
    if let LayerKind::Text { auto_fit, .. } = &mut layer.kind {
        *auto_fit = true;
    }
    layer.with_anim(AnimKind::Rise, 0.22, 0.38)
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = *SPECS.iter().find(|s| s.key == key)?;
    let prefix = binding_prefix(&spec);
    let period = format!("{{{{{prefix}.period}}}}");
    let source = format!("{{{{{prefix}.source}}}}");
    let coverage = format!("{{{{{prefix}.scope}}}}");
    let note = format!("{{{{{prefix}.note}}}}");
    let issue = format!("{{{{{prefix}.issue}}}}");
    let portrait = spec.format == 1;
    let square = spec.format == 2;
    let mut doc = if spec.format == 0 {
        frame("Area Cities Forecast", "Forecast", &period, &source)
    } else {
        social_frame(
            "Area Cities Forecast",
            "Forecast",
            &period,
            &source,
            portrait,
        )
    };
    let (x, top, row_h, widths, city_size, condition_size, temp_size) = if portrait {
        (
            45.,
            320.,
            220.,
            [290., 78., 310., 135., 135.],
            29.,
            25.,
            32.,
        )
    } else if square {
        (
            44.,
            278.,
            105.,
            [210., 74., 330., 152., 152.],
            23.,
            20.,
            24.,
        )
    } else {
        (
            80.,
            265.,
            108.,
            [330., 100., 800., 190., 190.],
            32.,
            29.,
            38.,
        )
    };
    let table_w: f32 = widths.iter().sum();
    doc.layers.push(text_layer(
        "Area scope",
        (x, top - 88., table_w, 34.),
        &coverage,
        if portrait { 24. } else { 21. },
        BODY,
        "$muted",
        Align::Left,
    ));
    let header_y = top - 47.;
    for (i, (title, width)) in [
        ("TOWN", widths[0]),
        ("", widths[1]),
        ("FORECAST", widths[2]),
        ("HIGH", widths[3]),
        ("LOW", widths[4]),
    ]
    .into_iter()
    .enumerate()
    {
        if !title.is_empty() {
            doc.layers.push(text_layer(
                &format!("Column heading {i}"),
                (x + widths[..i].iter().sum::<f32>(), header_y, width, 40.),
                title,
                if portrait { 23. } else { 21. },
                DISPLAY,
                "$muted",
                if i == 0 { Align::Left } else { Align::Center },
            ));
        }
    }
    for i in 0..MAX_TOWNS {
        let y = top + i as f32 * row_h;
        doc.layers.push(
            panel(&format!("Town row {i}"), x, y, table_w, row_h - 8.).with_anim(
                AnimKind::Fade,
                0.12 + i as f32 * 0.04,
                0.35,
            ),
        );
        let starts = [
            x,
            x + widths[0],
            x + widths[0] + widths[1],
            x + widths[0] + widths[1] + widths[2],
            x + widths[0] + widths[1] + widths[2] + widths[3],
        ];
        doc.layers.push(text_layer(
            &format!("Town name {i}"),
            (starts[0] + 12., y + 10., widths[0] - 20., row_h - 20.),
            &token(&spec, i, "name"),
            city_size,
            DISPLAY,
            "$text",
            Align::Left,
        ));
        let icon = Layer::risk_map(
            &format!("Town weather icon {i}"),
            starts[1] + 8.,
            y + 10.,
            widths[1] - 16.,
            row_h - 20.,
            &format!("set:cityforecast-icon:{}", token(&spec, i, "icon")),
        );
        doc.layers
            .push(icon.with_anim(AnimKind::Pop, 0.2 + i as f32 * 0.04, 0.38));
        doc.layers.push(text_layer(
            &format!("Town condition {i}"),
            (starts[2] + 8., y + 8., widths[2] - 16., row_h - 16.),
            &token(&spec, i, "condition"),
            condition_size,
            BODY,
            "$text",
            Align::Left,
        ));
        doc.layers.push(text_layer(
            &format!("Town high {i}"),
            (starts[3] + 4., y + 8., widths[3] - 8., row_h - 16.),
            &token(&spec, i, "high"),
            temp_size,
            DISPLAY,
            "$text",
            Align::Center,
        ));
        doc.layers.push(text_layer(
            &format!("Town low {i}"),
            (starts[4] + 4., y + 8., widths[4] - 8., row_h - 16.),
            &token(&spec, i, "low"),
            temp_size,
            DISPLAY,
            "$text",
            Align::Center,
        ));
    }
    let note_y = top + MAX_TOWNS as f32 * row_h + 1.;
    doc.layers.push(text_layer(
        "Coverage note",
        (x, note_y, table_w, 38.),
        &note,
        if portrait { 22. } else { 20. },
        BODY,
        "$muted",
        Align::Left,
    ));
    doc.layers.push(text_layer(
        "Forecast issue",
        (x, note_y + 38., table_w, 32.),
        &issue,
        18.,
        BODY,
        "$muted",
        Align::Left,
    ));
    Some(doc)
}
fn svg_for(key: &str) -> Option<&'static str> {
    Some(match key {
        "suncloud" => include_str!("../../../assets/icons/remix/sun-cloudy-line.svg"),
        "cloud" => include_str!("../../../assets/icons/remix/cloud-line.svg"),
        "rain" => include_str!("../../../assets/icons/remix/rainy-line.svg"),
        "snow" => include_str!("../../../assets/icons/remix/snowy-line.svg"),
        "thunder" => include_str!("../../../assets/icons/remix/thunderstorms-line.svg"),
        "hail" => include_str!("../../../assets/icons/remix/hail-line.svg"),
        "wind" => include_str!("../../../assets/icons/remix/windy-line.svg"),
        _ => return None,
    })
}
pub(super) fn draw_map(
    pm: &mut Pixmap,
    local: Transform,
    mask: Option<&Mask>,
    w: f32,
    h: f32,
    spec: &str,
    opacity: f32,
) -> bool {
    let Some(key) = spec.strip_prefix("set:cityforecast-icon:") else {
        return false;
    };
    let Some(svg) = svg_for(key) else { return true };
    let svg = svg.replace("currentColor", "#FFFFFF");
    let Some(tree) = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).ok()
    else {
        return true;
    };
    let size = w.min(h).max(1.);
    let side = size.ceil() as u32;
    let Some(mut raster) = resvg::tiny_skia::Pixmap::new(side, side) else {
        return true;
    };
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(
            size / tree.size().width(),
            size / tree.size().height(),
        ),
        &mut raster.as_mut(),
    );
    let Some(icon) = tiny_skia::IntSize::from_wh(side, side)
        .and_then(|dimensions| Pixmap::from_vec(raster.take(), dimensions)) else { return true };
    pm.draw_pixmap(
        ((w - size) / 2.) as i32,
        ((h - size) / 2.) as i32,
        icon.as_ref(),
        &PixmapPaint {
            opacity: opacity.clamp(0., 1.),
            ..Default::default()
        },
        local,
        mask,
    );
    true
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(spec) = SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
    else {
        return;
    };
    let prefix = binding_prefix(spec);
    let sample = [
        (
            "Jeffersonville, IN",
            "Partly cloudy",
            "74°F",
            "52°F",
            "suncloud",
        ),
        ("Jasper, IN", "Chance of showers", "71°F", "49°F", "rain"),
        ("New Albany, IN", "Mostly sunny", "76°F", "54°F", "suncloud"),
        ("Huntingburg, IN", "Cloudy", "69°F", "48°F", "cloud"),
        ("Tell City, IN", "Rain likely", "68°F", "50°F", "rain"),
        ("English, IN", "A few snow showers", "38°F", "31°F", "snow"),
    ];
    for (i, row) in sample.iter().enumerate() {
        for (field, value) in [
            ("name", row.0),
            ("condition", row.1),
            ("high", row.2),
            ("low", row.3),
            ("icon", row.4),
        ] {
            doc.data_overrides
                .push((format!("{prefix}.town.{i}.{field}"), value.into()));
        }
    }
    doc.data_overrides.push((
        format!("{prefix}.scope"),
        match spec.coverage {
            Coverage::County => "Dubois County, IN • SAMPLE",
            Coverage::Region => "Home region • SAMPLE",
            Coverage::State => "Indiana • SAMPLE",
            Coverage::Multi => "Selected states • SAMPLE",
        }
        .into(),
    ));
    doc.data_overrides.push((
        format!("{prefix}.period"),
        format!("SAMPLE • {} • Local NWS periods", period_label(spec.day)),
    ));
    doc.data_overrides.push((
        format!("{prefix}.source"),
        "SAMPLE ONLY • Synthetic town forecasts • Not for broadcast".into(),
    ));
    doc.data_overrides.push((
        format!("{prefix}.note"),
        "SAMPLE • Six selected towns • Not a complete area forecast".into(),
    ));
    doc.data_overrides.push((
        format!("{prefix}.issue"),
        "SAMPLE • Issued Nov 14 12:00 UTC".into(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn town(name: &str, state: &str, population: u32, lon: f64, lat: f64) -> (u32, Town) {
        (
            population,
            Town {
                name: name.into(),
                state: state.into(),
                point: GeoView { lon, lat, zoom: 1. },
            },
        )
    }

    #[test]
    fn largest_town_selection_is_stable_and_deduplicates_coordinates() {
        let values = vec![
            town("Jasper", "IN", 30000, -86.9, 38.4),
            town("Jeffersonville", "IN", 50000, -85.7, 38.3),
            town("Jasper duplicate", "IN", 1000, -86.9, 38.4),
            town("Huntingburg", "IN", 6000, -86.9, 38.3),
        ];
        let ranked = rank_and_limit(values, MAX_TOWNS);
        assert_eq!(
            ranked.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            vec!["Jeffersonville", "Jasper", "Huntingburg"]
        );
    }

    #[test]
    fn town_selection_rejects_boundaries_outside_points_and_wrong_state() {
        let square: Rings = vec![vec![(0., 0.), (4., 0.), (4., 4.), (0., 4.), (0., 0.)]];
        let county = CountyShape {
            state: "18",
            rings: &square,
            bounds: [0., 0., 4., 4.],
        };
        let scope = board::Scope {
            states: vec!["18".into()],
            county: Some("18001".into()),
            bbox: None,
        };
        let selected = [county];
        let inside = Place {
            name: "Town".into(),
            state: "IN".into(),
            lat: 2.,
            lon: 2.,
            population: 5000,
        };
        let border = Place {
            lon: 0.,
            ..inside.clone()
        };
        let outside = Place {
            lon: 5.,
            ..inside.clone()
        };
        let wrong_state = Place {
            state: "KY".into(),
            ..inside.clone()
        };
        assert_eq!(
            town_inside(&inside, &scope, &selected).as_deref(),
            Some("IN")
        );
        assert!(town_inside(&border, &scope, &selected).is_none());
        assert!(town_inside(&outside, &scope, &selected).is_none());
        assert!(town_inside(&wrong_state, &scope, &selected).is_none());
    }

    #[test]
    fn missing_city_snapshot_never_fabricates_weather_or_an_icon() {
        let town = Town {
            name: "Jeffersonville".into(),
            state: "IN".into(),
            point: GeoView {
                lon: -85.7,
                lat: 38.3,
                zoom: 1.,
            },
        };
        let missing = cells(&town, None);
        assert_eq!(missing.name, "Jeffersonville, IN");
        assert_eq!(missing.high, MISSING_DATA);
        assert_eq!(missing.low, MISSING_DATA);
        assert_eq!(missing.condition, MISSING_DATA);
        assert_eq!(missing.icon, "none");
    }

    #[test]
    fn city_family_registers_only_unique_area_city_keys() {
        let keys: std::collections::HashSet<_> = SPECS.iter().map(|s| s.key).collect();
        assert_eq!(SPECS.len(), 27);
        assert_eq!(keys.len(), 27);
        assert!(SPECS.iter().all(|s| s.day <= 2
            && matches!(
                s.coverage,
                Coverage::County | Coverage::Region | Coverage::State
            )));
    }

    #[test]
    fn remaining_night_withholds_daytime_sun_glyph_and_high() {
        let (_, town) = town("Jeffersonville", "IN", 50000, -85.7, 38.3);
        let mut value = super::super::social_forecast::CityDay {
            high: String::new(), low: "51°F".into(), condition: "Mostly clear".into(),
            issue: "NWS source time".into(), date: "Sat Nov 14".into(), remaining_night: true,
        };
        let night = cells(&town, Some(&value));
        assert_eq!(night.icon, "none");
        assert!(night.high.is_empty());
        assert_eq!(night.low, "51°F");
        value.condition = "Partly cloudy".into();
        assert_eq!(cells(&town, Some(&value)).icon, "cloud");
        value.remaining_night = false;
        value.condition = "Mostly clear".into();
        assert_eq!(cells(&town, Some(&value)).icon, "suncloud");
        value.condition = "Unclassified weather phrase".into();
        assert_eq!(cells(&town, Some(&value)).icon, "none");
    }
}

include!("town_forecast_maps_city_api.rs");
