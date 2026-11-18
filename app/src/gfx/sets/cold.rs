//! NOAA GFS thermal maps. Shared scope/window/layout parameters; no extrapolation.
use super::{
    board::{self, Band, Layout, ReportPoint, Scene},
    comparison,
    observed::{self, Coverage},
};
use crate::{
    config::Settings,
    data::WxSnapshot,
    gfx::{bind::Bindings, model::GraphicDoc},
    grid::{Var, WxGrid},
};
use chrono::{Local, TimeZone};

pub(super) const SPECS: &[(&str, u8, Coverage)] = &[
    ("set_wind_chill__today_county_overlay", 0, Coverage::County),
    ("set_wind_chill__today_region_overlay", 0, Coverage::Region),
    ("set_wind_chill__today_state_overlay", 0, Coverage::State),
    (
        "set_wind_chill__tomorrow_county_overlay",
        1,
        Coverage::County,
    ),
    (
        "set_wind_chill__tomorrow_region_overlay",
        1,
        Coverage::Region,
    ),
    ("set_wind_chill__tomorrow_state_overlay", 1, Coverage::State),
    ("set_wind_chill__day3_county_overlay", 2, Coverage::County),
    ("set_wind_chill__day3_region_overlay", 2, Coverage::Region),
    ("set_wind_chill__day3_state_overlay", 2, Coverage::State),
    (
        "set_morning_lows__today_county_overlay",
        0,
        Coverage::County,
    ),
    (
        "set_morning_lows__today_region_overlay",
        0,
        Coverage::Region,
    ),
    ("set_morning_lows__today_state_overlay", 0, Coverage::State),
    (
        "set_morning_lows__tomorrow_county_overlay",
        1,
        Coverage::County,
    ),
    (
        "set_morning_lows__tomorrow_region_overlay",
        1,
        Coverage::Region,
    ),
    (
        "set_morning_lows__tomorrow_state_overlay",
        1,
        Coverage::State,
    ),
    ("set_morning_lows__day3_county_overlay", 2, Coverage::County),
    ("set_morning_lows__day3_region_overlay", 2, Coverage::Region),
    ("set_morning_lows__day3_state_overlay", 2, Coverage::State),
    ("set_heat_index__today_county_overlay", 0, Coverage::County),
    ("set_heat_index__today_region_overlay", 0, Coverage::Region),
    ("set_heat_index__today_state_overlay", 0, Coverage::State),
    (
        "set_heat_index__tomorrow_county_overlay",
        1,
        Coverage::County,
    ),
    (
        "set_heat_index__tomorrow_region_overlay",
        1,
        Coverage::Region,
    ),
    ("set_heat_index__tomorrow_state_overlay", 1, Coverage::State),
    ("set_heat_index__day3_county_overlay", 2, Coverage::County),
    ("set_heat_index__day3_region_overlay", 2, Coverage::Region),
    ("set_heat_index__day3_state_overlay", 2, Coverage::State),
    ("set_heat_index__today_county", 0, Coverage::County),
    ("set_heat_index__today_region", 0, Coverage::Region),
    ("set_heat_index__today_state", 0, Coverage::State),
    ("set_heat_index__tomorrow_county", 1, Coverage::County),
    ("set_heat_index__tomorrow_region", 1, Coverage::Region),
    ("set_heat_index__tomorrow_state", 1, Coverage::State),
    ("set_heat_index__day3_county", 2, Coverage::County),
    ("set_heat_index__day3_region", 2, Coverage::Region),
    ("set_heat_index__day3_state", 2, Coverage::State),
    ("set_heat_index__today_county_portrait", 0, Coverage::County),
    ("set_heat_index__today_region_portrait", 0, Coverage::Region),
    ("set_heat_index__today_state_portrait", 0, Coverage::State),
    (
        "set_heat_index__tomorrow_county_portrait",
        1,
        Coverage::County,
    ),
    (
        "set_heat_index__tomorrow_region_portrait",
        1,
        Coverage::Region,
    ),
    (
        "set_heat_index__tomorrow_state_portrait",
        1,
        Coverage::State,
    ),
    ("set_heat_index__day3_county_portrait", 2, Coverage::County),
    ("set_heat_index__day3_region_portrait", 2, Coverage::Region),
    ("set_heat_index__day3_state_portrait", 2, Coverage::State),
    ("set_heat_index__today_county_square", 0, Coverage::County),
    ("set_heat_index__today_region_square", 0, Coverage::Region),
    ("set_heat_index__today_state_square", 0, Coverage::State),
    (
        "set_heat_index__tomorrow_county_square",
        1,
        Coverage::County,
    ),
    (
        "set_heat_index__tomorrow_region_square",
        1,
        Coverage::Region,
    ),
    ("set_heat_index__tomorrow_state_square", 1, Coverage::State),
    ("set_heat_index__day3_county_square", 2, Coverage::County),
    ("set_heat_index__day3_region_square", 2, Coverage::Region),
    ("set_heat_index__day3_state_square", 2, Coverage::State),
    ("set_morning_lows__today_county_square", 0, Coverage::County),
    ("set_morning_lows__today_region_square", 0, Coverage::Region),
    ("set_morning_lows__today_state_square", 0, Coverage::State),
    (
        "set_morning_lows__tomorrow_county_square",
        1,
        Coverage::County,
    ),
    (
        "set_morning_lows__tomorrow_region_square",
        1,
        Coverage::Region,
    ),
    (
        "set_morning_lows__tomorrow_state_square",
        1,
        Coverage::State,
    ),
    ("set_morning_lows__day3_county_square", 2, Coverage::County),
    ("set_morning_lows__day3_region_square", 2, Coverage::Region),
    ("set_morning_lows__day3_state_square", 2, Coverage::State),
    ("set_wind_chill__today_county_square", 0, Coverage::County),
    ("set_wind_chill__today_region_square", 0, Coverage::Region),
    ("set_wind_chill__today_state_square", 0, Coverage::State),
    (
        "set_wind_chill__tomorrow_county_square",
        1,
        Coverage::County,
    ),
    (
        "set_wind_chill__tomorrow_region_square",
        1,
        Coverage::Region,
    ),
    ("set_wind_chill__tomorrow_state_square", 1, Coverage::State),
    ("set_wind_chill__day3_county_square", 2, Coverage::County),
    ("set_wind_chill__day3_region_square", 2, Coverage::Region),
    ("set_wind_chill__day3_state_square", 2, Coverage::State),
    (
        "set_morning_lows__today_county_portrait",
        0,
        Coverage::County,
    ),
    (
        "set_morning_lows__today_region_portrait",
        0,
        Coverage::Region,
    ),
    ("set_morning_lows__today_state_portrait", 0, Coverage::State),
    (
        "set_morning_lows__tomorrow_county_portrait",
        1,
        Coverage::County,
    ),
    (
        "set_morning_lows__tomorrow_region_portrait",
        1,
        Coverage::Region,
    ),
    (
        "set_morning_lows__tomorrow_state_portrait",
        1,
        Coverage::State,
    ),
    (
        "set_morning_lows__day3_county_portrait",
        2,
        Coverage::County,
    ),
    (
        "set_morning_lows__day3_region_portrait",
        2,
        Coverage::Region,
    ),
    ("set_morning_lows__day3_state_portrait", 2, Coverage::State),
    ("set_wind_chill__today_county_portrait", 0, Coverage::County),
    ("set_wind_chill__today_region_portrait", 0, Coverage::Region),
    ("set_wind_chill__today_state_portrait", 0, Coverage::State),
    (
        "set_wind_chill__tomorrow_county_portrait",
        1,
        Coverage::County,
    ),
    (
        "set_wind_chill__tomorrow_region_portrait",
        1,
        Coverage::Region,
    ),
    (
        "set_wind_chill__tomorrow_state_portrait",
        1,
        Coverage::State,
    ),
    ("set_wind_chill__day3_county_portrait", 2, Coverage::County),
    ("set_wind_chill__day3_region_portrait", 2, Coverage::Region),
    ("set_wind_chill__day3_state_portrait", 2, Coverage::State),
    ("set_morning_lows__today_county", 0, Coverage::County),
    ("set_morning_lows__today_region", 0, Coverage::Region),
    ("set_morning_lows__today_state", 0, Coverage::State),
    ("set_morning_lows__tomorrow_county", 1, Coverage::County),
    ("set_morning_lows__tomorrow_region", 1, Coverage::Region),
    ("set_morning_lows__tomorrow_state", 1, Coverage::State),
    ("set_morning_lows__day3_county", 2, Coverage::County),
    ("set_morning_lows__day3_region", 2, Coverage::Region),
    ("set_morning_lows__day3_state", 2, Coverage::State),
    ("set_wind_chill__today_county", 0, Coverage::County),
    ("set_wind_chill__today_region", 0, Coverage::Region),
    ("set_wind_chill__today_state", 0, Coverage::State),
    ("set_wind_chill__tomorrow_county", 1, Coverage::County),
    ("set_wind_chill__tomorrow_region", 1, Coverage::Region),
    ("set_wind_chill__tomorrow_state", 1, Coverage::State),
    ("set_wind_chill__day3_county", 2, Coverage::County),
    ("set_wind_chill__day3_region", 2, Coverage::Region),
    ("set_wind_chill__day3_state", 2, Coverage::State),
];
#[derive(Clone, Copy, PartialEq, Eq)]
enum Metric {
    WindChill,
    MorningLow,
    HeatIndex,
}
impl Metric {
    fn from_key(key: &str) -> Self {
        if key.starts_with("set_heat_index") {
            Self::HeatIndex
        } else if key.starts_with("set_morning_lows") {
            Self::MorningLow
        } else {
            Self::WindChill
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::WindChill => "Wind Chill Forecast",
            Self::MorningLow => "Morning Low Forecast",
            Self::HeatIndex => "Peak Heat Index Forecast",
        }
    }
    fn panel(self) -> &'static str {
        match self {
            Self::WindChill => "Lowest forecast wind chill",
            Self::MorningLow => "Lowest forecast morning temperature",
            Self::HeatIndex => "Highest applicable hourly heat index",
        }
    }
}
fn prefix(key: &str) -> String {
    key.strip_suffix("_portrait")
        .or_else(|| key.strip_suffix("_square"))
        .or_else(|| key.strip_suffix("_overlay"))
        .unwrap_or(key)
        .replace("set_wind_chill__", "chill.")
        .replace("set_morning_lows__", "morning.")
        .replace("set_heat_index__", "heatindex.")
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    SPECS.iter().find(|s| s.0 == key)?;
    if key.ends_with("_overlay") {
        return Some(super::product_overlay::template(
            Metric::from_key(key).title(),
            if Metric::from_key(key) == Metric::HeatIndex {
                "Heat"
            } else {
                "Winter"
            },
            &prefix(key),
        ));
    }
    if key.ends_with("_portrait") {
        return Some(comparison::portrait_template(
            Metric::from_key(key).title(),
            if Metric::from_key(key) == Metric::HeatIndex {
                "Heat"
            } else {
                "Winter"
            },
            &prefix(key),
        ));
    }
    if key.ends_with("_square") {
        return Some(comparison::square_template(
            Metric::from_key(key).title(),
            if Metric::from_key(key) == Metric::HeatIndex {
                "Heat"
            } else {
                "Winter"
            },
            &prefix(key),
        ));
    }
    Some(comparison::template(
        Metric::from_key(key).title(),
        if Metric::from_key(key) == Metric::HeatIndex {
            "Heat"
        } else {
            "Winter"
        },
        &prefix(key),
        Layout::Single,
    ))
}
// NWS formula: https://www.weather.gov/safety/cold-faqs . °F and sustained mph.
// Outside its defined domain there is no wind-chill value, rather than an air-temperature substitute.
fn wind_chill(t: f32, v: f32) -> Option<f32> {
    if !t.is_finite()
        || !v.is_finite()
        || !(-150. ..=50.).contains(&t)
        || !(3. ..=250.).contains(&v)
        || v == 3.
    {
        return None;
    }
    let power = v.powf(0.16);
    Some(35.74 + 0.6215 * t - 35.75 * power + 0.4275 * t * power)
}
// NWS State College publishes the WPC algorithm and both humidity adjustments:
// https://www.weather.gov/ctp/heat . Conservative supported subset: T 80–112F,
// finite RH 0–100%, resulting HI 80–140F. Never substitute air temperature.
fn heat_index(t: f32, rh: f32) -> Option<f32> {
    let (t, rh) = (t as f64, rh as f64);
    if !t.is_finite()
        || !rh.is_finite()
        || !(80. ..=112.).contains(&t)
        || !(0. ..=100.).contains(&rh)
    {
        return None;
    }
    let simple = (0.5 * (t + 61. + (t - 68.) * 1.2 + rh * 0.094) + t) * 0.5;
    if simple < 80. {
        return None;
    }
    let mut hi = -42.379 + 2.04901523 * t + 10.14333127 * rh
        - 0.22475541 * t * rh
        - 0.00683783 * t * t
        - 0.05481717 * rh * rh
        + 0.00122874 * t * t * rh
        + 0.00085282 * t * rh * rh
        - 0.00000199 * t * t * rh * rh;
    if rh < 13. && (80. ..=112.).contains(&t) {
        hi -= ((13. - rh) / 4.) * ((17. - (t - 95.).abs()) / 17.).sqrt();
    } else if rh > 85. && (80. ..=87.).contains(&t) {
        hi += ((rh - 85.) / 10.) * ((87. - t) / 5.);
    }
    (hi.is_finite() && (80. ..=140.).contains(&hi)).then_some(hi as f32)
}
fn extreme(metric: Metric, prior: Option<f32>, value: f32) -> f32 {
    prior
        .map(|old| {
            if metric == Metric::HeatIndex {
                old.max(value)
            } else {
                old.min(value)
            }
        })
        .unwrap_or(value)
}
fn window(day: u8, now: chrono::DateTime<Local>, metric: Metric) -> Option<(i64, i64)> {
    let date = now.date_naive() + chrono::Duration::days(day as i64);
    let start = Local
        .from_local_datetime(&date.and_hms_opt(0, 0, 0)?)
        .single()?
        .timestamp();
    let end_date = if metric == Metric::MorningLow {
        date
    } else {
        date + chrono::Duration::days(1)
    };
    let end = Local
        .from_local_datetime(&end_date.and_hms_opt(
            if metric == Metric::MorningLow { 12 } else { 0 },
            0,
            0,
        )?)
        .single()?
        .timestamp();
    let start = if day == 0 && metric != Metric::MorningLow {
        ((now.timestamp() + 3599) / 3600) * 3600
    } else {
        start
    };
    (start < end).then_some((start, end))
}
fn usable(grid: &WxGrid, start: i64, end: i64) -> bool {
    grid.model == crate::grid::GridModel::Gfs
        && grid.model.is_noaa()
        && grid.nx >= 2
        && grid.ny >= 2
        && grid.north > grid.south
        && grid.east > grid.west
        && grid.times.len() >= 2
        && grid.times.windows(2).all(|p| p[1] - p[0] == 3600)
        && grid.times[0] <= start
        && *grid.times.last().unwrap() >= end - 3600
}
fn base(scope: board::Scope, metric: Metric) -> Scene {
    let mut scene = board::empty(Layout::Single);
    scene.scope = scope;
    scene.legend_title = if metric == Metric::HeatIndex {
        "Heat index • °F • County-centre fill / town point values".into()
    } else {
        format!("{} • °F • Rounded county-centre values", metric.panel())
    };
    scene.legend = if metric == Metric::HeatIndex {
        [
            ("80 to 89", "#FFE066"),
            ("90 to 99", "#FFBA42"),
            ("100 to 104", "#FF8C32"),
            ("105 to 109", "#F65336"),
            ("110 to 114", "#D92F59"),
            ("115 to 119", "#BD258C"),
            ("120 to 140", "#8738CF"),
        ]
    } else {
        [
            ("Below -20", "#AB63E8"),
            ("-20 to -11", "#7951DB"),
            ("-10 to -1", "#466AE0"),
            ("0 to 9", "#419DDC"),
            ("10 to 19", "#58C3DC"),
            ("20 to 31", "#7BD6CB"),
            ("32+", "#B8E0C1"),
        ]
    }
    .into_iter()
    .map(|(label, color)| Band {
        label: label.into(),
        color: color.into(),
    })
    .collect();
    scene.panels[0].title = metric.panel().into();
    scene.panels[0].summary = Some(Vec::new());
    scene.panels[0].summary_title = if metric == Metric::HeatIndex {
        "Highest sampled towns • Up to 7 shown"
    } else {
        "Coldest available county centres"
    }
    .into();
    scene
}
fn category(value: f32, metric: Metric) -> usize {
    (if metric == Metric::HeatIndex {
        [90., 100., 105., 110., 115., 120.]
    } else {
        [-20., -10., 0., 10., 20., 32.]
    })
    .iter()
    .filter(|n| value >= **n)
    .count()
}
fn thermal_at(
    grid: &WxGrid,
    start: i64,
    end: i64,
    lon: f64,
    lat: f64,
    metric: Metric,
) -> Option<f32> {
    if lon < grid.west || lon > grid.east || lat < grid.south || lat > grid.north {
        return None;
    }
    let mut result = None;
    for stamp in (start..end).step_by(3600) {
        let tf = (stamp - grid.times[0]) as f32 / 3600.;
        let t = grid
            .sample(Var::Temp, tf, lat, lon)
            .filter(|v| v.is_finite() && (-150. ..=150.).contains(v))?;
        let value = match metric {
            Metric::MorningLow => Some(t),
            Metric::HeatIndex => heat_index(
                t,
                grid.sample(Var::Rh, tf, lat, lon)
                    .filter(|v| v.is_finite() && (0. ..=100.).contains(v))?,
            ),
            Metric::WindChill => wind_chill(
                t,
                grid.sample(Var::Wind, tf, lat, lon)
                    .filter(|v| v.is_finite() && (0. ..=250.).contains(v))?,
            ),
        };
        if let Some(value) = value {
            result = Some(extreme(metric, result, value));
        }
    }
    result.map(f32::round)
}
// Geography only: reuse the existing place cache; never fetch another location provider.
fn sampled_towns(scope: &board::Scope) -> Vec<crate::places::Place> {
    let Some(all) = crate::map::boundaries::counties_named() else {
        return Vec::new();
    };
    let counties = board::selected(&all, scope);
    let mut bbox = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for c in &counties {
        for &(x, y) in c.rings.iter().flatten() {
            bbox[0] = bbox[0].min(x);
            bbox[1] = bbox[1].min(y);
            bbox[2] = bbox[2].max(x);
            bbox[3] = bbox[3].max(y);
        }
    }
    let mut towns: Vec<_> = crate::places::in_box(bbox[1], bbox[3], bbox[0], bbox[2])
        .into_iter()
        .filter_map(|mut town| {
            let c = counties
                .iter()
                .find(|c| board::inside(c, town.lon, town.lat))?;
            town.state = crate::winter::state_code(&c.state).into();
            Some(town)
        })
        .collect();
    towns.sort_by(|a, b| {
        b.population
            .cmp(&a.population)
            .then(a.name.cmp(&b.name))
            .then(a.lon.total_cmp(&b.lon))
            .then(a.lat.total_cmp(&b.lat))
    });
    towns.dedup_by(|a, b| a.name == b.name && a.lon == b.lon && a.lat == b.lat);
    towns.truncate(64);
    towns
}
fn town_point(town: &crate::places::Place, regional: bool, value: f32) -> ReportPoint {
    let name = if regional {
        format!("{}, {}", town.name, town.state)
    } else {
        town.name.clone()
    };
    ReportPoint {
        id: format!(
            "town:{}:{}:{}",
            town.name,
            town.lon.to_bits(),
            town.lat.to_bits()
        ),
        lon: town.lon,
        lat: town.lat,
        label: format!("{name} • {value:.0}°F"),
        category: category(value, Metric::HeatIndex),
        value: None,
    }
}
fn forecast(
    data: &WxSnapshot,
    scope: board::Scope,
    day: u8,
    now: chrono::DateTime<Local>,
    metric: Metric,
) -> Scene {
    let mut scene = base(scope, metric);
    let Some((start, end)) = window(day, now, metric) else {
        return scene;
    };
    let p = &mut scene.panels[0];
    let local = |t| {
        chrono::DateTime::from_timestamp(t, 0)
            .unwrap()
            .with_timezone(&Local)
    };
    p.valid = format!(
        "{} to {}",
        local(start).format("%b %-d %H:%M %Z"),
        local(end).format("%b %-d %H:%M %Z")
    );
    p.detail = if metric == Metric::MorningLow {
        "NOAA GFS • Lowest hourly temperature from midnight to noon"
    } else if metric == Metric::HeatIndex {
        "NOAA GFS • Town point values; county fill uses county centres"
    } else {
        "NOAA GFS • Lowest hourly wind chill in the stated window"
    }
    .into();
    let Some(grid) = data.grid.as_ref().filter(|g| usable(g, start, end)) else {
        return scene;
    };
    let Some(all) = crate::map::boundaries::counties_named() else {
        return scene;
    };
    let mut values = std::collections::BTreeMap::new();
    let mut labels = Vec::new();
    for c in board::selected(&all, &scene.scope) {
        let (lon, lat) = board::centre(c);
        if let Some(value) = thermal_at(grid, start, end, lon, lat, metric) {
            let i = category(value, metric);
            values.insert(c.fips.clone(), i);
            labels.push((
                value,
                ReportPoint {
                    id: c.fips.clone(),
                    lon,
                    lat,
                    label: format!("{value:.0}°F"),
                    category: i,
                    value: None,
                },
            ));
        }
    }
    if metric == Metric::HeatIndex {
        labels = sampled_towns(&scene.scope)
            .iter()
            .filter_map(|town| {
                let value = thermal_at(grid, start, end, town.lon, town.lat, metric)?;
                Some((value, town_point(town, scene.scope.bbox.is_some(), value)))
            })
            .collect();
    }
    if !values.is_empty() || !labels.is_empty() {
        labels.sort_by(|a, b| {
            if metric == Metric::HeatIndex {
                b.0.total_cmp(&a.0)
            } else {
                a.0.total_cmp(&b.0)
            }
        });
        p.summary = Some(
            labels
                .iter()
                .take(7)
                .map(|(value, r)| {
                    if metric == Metric::HeatIndex {
                        return (
                            r.label.split(" • ").next().unwrap_or(&r.label).to_string(),
                            format!("{value:.0}°F"),
                        );
                    }
                    (
                        all.iter()
                            .find(|c| c.fips == r.id)
                            .map(|c| {
                                if scene.scope.bbox.is_some() {
                                    format!("{}, {}", c.name, crate::winter::state_code(&c.state))
                                } else {
                                    c.name.clone()
                                }
                            })
                            .unwrap_or_else(|| r.id.clone()),
                        r.label.clone(),
                    )
                })
                .collect(),
        );
        p.counties = Some(values);
        p.points = Some(labels.into_iter().take(24).map(|(_, p)| p).collect());
        scene.reviewed = true;
    }
    scene
}
pub(super) fn bind(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    let now = Local::now();
    for &(key, day, coverage) in SPECS {
        if key.ends_with("_portrait") || key.ends_with("_square") || key.ends_with("_overlay") {
            continue;
        }
        let metric = Metric::from_key(key);
        let prefix = prefix(key);
        let scene = forecast(
            data,
            observed::report_scope(settings, coverage),
            day,
            now,
            metric,
        );
        b.set(
            &format!("set.{prefix}.payload"),
            &serde_json::to_string(&scene).unwrap_or_default(),
        );
        b.set(
            &format!("set.{prefix}.source"),
            if metric == Metric::MorningLow {
                "Source: NOAA GFS • Temperature forecast"
            } else if metric == Metric::HeatIndex {
                "Source: NOAA GFS • NWS heat index calculation"
            } else {
                "Source: NOAA GFS • NWS wind chill formula"
            },
        );
        b.set(
            &format!("set.{prefix}.subtitle"),
            &format!(
                "{} • {}",
                settings.home_label,
                match day {
                    0 if metric != Metric::MorningLow => "Remaining today",
                    0 => "Today morning",
                    1 => "Tomorrow",
                    _ => "Day 3",
                }
            ),
        );
        b.set(&format!("set.{prefix}.note"),if metric==Metric::MorningLow {"Forecast midnight–noon minimum • Complete model coverage required • Forecast values do not replace observed morning lows."}else if metric==Metric::HeatIndex {"Shade / light wind • Supported heat index 80–140°F from air temperatures 80–112°F • Other values withheld."}else{"Defined only at 50°F or colder with wind above 3 mph • Unfilled areas are unavailable or outside that range."});
    }
}
#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(&(key, day, coverage)) = SPECS.iter().find(|s| Some(s.0) == doc.template.as_deref())
    else {
        return;
    };
    let metric = Metric::from_key(key);
    let mut scene = base(
        observed::report_scope(&Settings::default(), coverage),
        metric,
    );
    if let Some(all) = crate::map::boundaries::counties_named() {
        let counties = board::selected(&all, &scene.scope);
        let mut ranked: Vec<_> = counties
            .iter()
            .enumerate()
            .map(|(i, c)| {
                (
                    if metric == Metric::HeatIndex {
                        [85, 95, 102, 107, 112, 117, 125][i % 7]
                    } else {
                        -25 + (i % 7) as i32 * 10
                    },
                    *c,
                )
            })
            .collect();
        ranked.sort_by(|a, b| {
            (if metric == Metric::HeatIndex {
                b.0.cmp(&a.0)
            } else {
                a.0.cmp(&b.0)
            })
            .then(a.1.name.cmp(&b.1.name))
        });
        scene.panels[0].summary = Some(
            ranked
                .iter()
                .take(7)
                .map(|(value, county)| {
                    (
                        if scene.scope.bbox.is_some() {
                            format!(
                                "{}, {}",
                                county.name,
                                crate::winter::state_code(&county.state)
                            )
                        } else {
                            county.name.clone()
                        },
                        format!("{value}°F"),
                    )
                })
                .collect(),
        );
        scene.panels[0].points = Some(
            ranked
                .iter()
                .take(24)
                .map(|(value, c)| {
                    let (lon, lat) = board::centre(c);
                    ReportPoint {
                        id: c.fips.clone(),
                        lon,
                        lat,
                        label: format!("{value}°F"),
                        category: category(*value as f32, metric),
                        value: None,
                    }
                })
                .collect(),
        );
        scene.panels[0].counties = Some(
            board::selected(&all, &scene.scope)
                .iter()
                .enumerate()
                .map(|(i, c)| (c.fips.clone(), i % 7))
                .collect(),
        );
        scene.reviewed = true;
    }
    if metric == Metric::HeatIndex {
        let mut points: Vec<_> = sampled_towns(&scene.scope)
            .iter()
            .enumerate()
            .map(|(i, town)| {
                let value = [85., 95., 102., 107., 112., 117., 125.][i % 7];
                (value, town_point(town, scene.scope.bbox.is_some(), value))
            })
            .collect();
        points.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.label.cmp(&b.1.label)));
        scene.panels[0].summary = Some(
            points
                .iter()
                .take(7)
                .map(|(v, p)| {
                    (
                        p.label.split(" • ").next().unwrap().into(),
                        format!("{v:.0}°F"),
                    )
                })
                .collect(),
        );
        scene.panels[0].points = Some(points.into_iter().take(24).map(|(_, p)| p).collect());
    }
    scene.panels[0].valid = if metric == Metric::MorningLow {
        format!("SAMPLE • Nov {} 00:00 to 12:00 EST", 13 + day)
    } else if day == 0 {
        "SAMPLE • Nov 13 18:00 to Nov 14 00:00 EST".into()
    } else {
        format!(
            "SAMPLE • Nov {} 00:00 to Nov {} 00:00 EST",
            13 + day,
            14 + day
        )
    };
    scene.panels[0].detail = "SAMPLE synthetic forecast categories • Not operational".into();
    let prefix = prefix(key);
    doc.data_overrides.extend([
        (
            format!("set.{prefix}.payload"),
            serde_json::to_string(&scene).unwrap(),
        ),
        (
            format!("set.{prefix}.source"),
            "SAMPLE ONLY • Not for broadcast".into(),
        ),
        (
            format!("set.{prefix}.subtitle"),
            format!(
                "SAMPLE • {} • Thermal forecast",
                match day {
                    0 if metric != Metric::MorningLow => "Remaining today",
                    0 => "Today morning",
                    1 => "Tomorrow",
                    _ => "Day 3",
                }
            ),
        ),
    ]);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn heat_points_preserve_town_coordinates_and_are_not_county_centres() {
        let town = crate::places::Place {
            name: "Jeffersonville".into(),
            state: "IN".into(),
            lon: -85.738,
            lat: 38.278,
            population: 49000,
        };
        let point = town_point(&town, true, 106.);
        assert_eq!((point.lon, point.lat), (town.lon, town.lat));
        assert_eq!(point.label, "Jeffersonville, IN • 106°F");
        let hourly = [(90., 70.), (100., 40.)];
        let max = hourly
            .into_iter()
            .filter_map(|(t, rh)| heat_index(t, rh))
            .fold(0., f32::max);
        assert!((max - 109.3).abs() < 0.1); // Combining independent daily maxima would produce a different value.
    }
    #[test]
    fn heat_index_matches_chart_and_both_humidity_adjustments() {
        assert!((heat_index(90., 70.).unwrap() - 106.).abs() < 0.2);
        assert!((heat_index(100., 40.).unwrap() - 109.).abs() < 0.5);
        assert!((heat_index(100., 10.).unwrap() - 94.1).abs() < 0.3);
        assert!((heat_index(82., 90.).unwrap() - 92.0).abs() < 0.5);
        for (t, rh) in [
            (79., 90.),
            (113., 40.),
            (90., -1.),
            (90., 101.),
            (f32::NAN, 60.),
            (90., f32::NAN),
            (112., 100.),
        ] {
            assert!(heat_index(t, rh).is_none());
        }
    }
    #[test]
    fn heat_peaks_select_maximum_and_share_complete_day_windows() {
        assert_eq!(extreme(Metric::HeatIndex, Some(100.), 110.), 110.);
        assert_eq!(extreme(Metric::WindChill, Some(-10.), -5.), -10.);
        let now = Local
            .with_ymd_and_hms(2026, 7, 13, 14, 20, 0)
            .single()
            .unwrap();
        assert_eq!(
            window(0, now, Metric::HeatIndex),
            window(0, now, Metric::WindChill)
        );
        assert_eq!(
            window(1, now, Metric::HeatIndex),
            window(1, now, Metric::WindChill)
        );
    }
    #[test]
    fn formula_domain_and_day_windows_are_explicit() {
        assert!((wind_chill(0., 10.).unwrap() + 15.9).abs() < 0.2);
        for (t, v) in [
            (51., 10.),
            (20., 3.),
            (20., 0.),
            (f32::NAN, 10.),
            (0., f32::NAN),
        ] {
            assert!(wind_chill(t, v).is_none());
        }
        let now = Local
            .with_ymd_and_hms(2026, 11, 13, 14, 20, 0)
            .single()
            .unwrap();
        let (start, end) = window(0, now, Metric::WindChill).unwrap();
        assert_eq!(start % 3600, 0);
        assert!(start > now.timestamp() && end > start);
        let (next_start, next_end) = window(1, now, Metric::WindChill).unwrap();
        assert_eq!(end, next_start);
        assert!(next_end > next_start);
        let (morning_start, morning_end) = window(1, now, Metric::MorningLow).unwrap();
        assert_eq!(morning_start, next_start);
        assert_eq!(morning_end - morning_start, 12 * 3600);
    }
}
