//! Bounded town-point NWS high/low and probability-of-precipitation maps.
use super::{
    board::{self, Palette, Panel, ReportPoint, Scene},
    observed::Coverage,
    reference::{frame, label, social_frame, MISSING},
};
use crate::{
    config::Settings,
    gfx::{
        bind::Bindings,
        model::{AnimKind, Fill, GraphicDoc, Layer},
        text::Align,
    },
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    HighLow,
    Precipitation,
}
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    kind: Kind,
    day: u8,
    night: bool,
    coverage: Coverage,
    format: u8,
}
include!("town_forecast_maps_specs.rs");

fn coverage_key(coverage: Coverage) -> &'static str {
    match coverage {
        Coverage::County => "county",
        Coverage::Region => "region",
        Coverage::State => "state",
        Coverage::Multi => "multi",
    }
}
fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::HighLow => "highlow",
        Kind::Precipitation => "precip",
    }
}
fn day_key(day: u8, night: bool) -> &'static str {
    if night {
        "tonight"
    } else {
        match day {
            0 => "today",
            1 => "tomorrow",
            2 => "day3",
            3 => "day4",
            4 => "day5",
            5 => "day6",
            _ => "day7",
        }
    }
}
fn period_label(day: u8, night: bool) -> &'static str {
    if night {
        "Tonight"
    } else {
        match day {
            0 => "Today",
            1 => "Tomorrow",
            2 => "Day 3",
            3 => "Day 4",
            4 => "Day 5",
            5 => "Day 6",
            _ => "Day 7",
        }
    }
}
fn prefix(spec: &Spec) -> String {
    format!(
        "set.townmaps.{}.{}.{}",
        kind_key(spec.kind),
        day_key(spec.day, spec.night),
        coverage_key(spec.coverage)
    )
}
fn chance_bucket(value: f32) -> usize {
    if value <= 0.0 {
        0
    } else if value < 20.0 {
        1
    } else if value < 40.0 {
        2
    } else if value < 60.0 {
        3
    } else {
        4
    }
}
struct CoverageInfo {
    label: String,
    scope: board::Scope,
}
#[derive(Default)]
struct AreaCache {
    values: HashMap<(u64, u64), Arc<HashMap<&'static str, CoverageInfo>>>,
    order: VecDeque<(u64, u64)>,
}
fn area_cache() -> &'static Mutex<AreaCache> {
    static CACHE: OnceLock<Mutex<AreaCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(AreaCache::default()))
}
fn coverage_info(home: crate::config::GeoView) -> Option<Arc<HashMap<&'static str, CoverageInfo>>> {
    let key = (home.lon.to_bits(), home.lat.to_bits());
    if let Ok(cache) = area_cache().lock() {
        if let Some(value) = cache.values.get(&key) {
            return Some(Arc::clone(value));
        }
    }
    // Cache only immutable, strictly selected geography. Forecast values are
    // collected and freshness-checked on every bind below.
    let areas = super::city_forecast::town_forecast_map_areas(home)?
        .into_iter()
        .map(|area| {
            (
                coverage_key(area.coverage),
                CoverageInfo {
                    label: area.label,
                    scope: area.scope,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    if areas.len() != 3 {
        return None;
    }
    let areas = Arc::new(areas);
    if let Ok(mut cache) = area_cache().lock() {
        cache.values.insert(key, Arc::clone(&areas));
        cache.order.retain(|entry| *entry != key);
        cache.order.push_back(key);
        while cache.order.len() > 8 {
            if let Some(expired) = cache.order.pop_front() {
                cache.values.remove(&expired);
            }
        }
    }
    Some(areas)
}
fn bands(kind: Kind) -> Vec<board::Band> {
    let rows: &[(&str, &str)] = match kind {
        Kind::HighLow => &[("NWS town forecast values • °F", "#64B5D9")],
        Kind::Precipitation => &[
            ("0%", "#667E92"),
            ("1–19%", "#01B414"),
            ("20–39%", "#074407"),
            ("40–59%", "#FEED07"),
            ("60–100%", "#BF0C08"),
        ],
    };
    rows.iter()
        .map(|(label, color)| board::Band {
            label: (*label).into(),
            color: (*color).into(),
        })
        .collect()
}
fn point_label(
    kind: Kind,
    day: u8,
    night: bool,
    values: &super::social_forecast::CityMapDay,
) -> Option<(String, f32, usize)> {
    match kind {
        Kind::HighLow => {
            if values.high.is_none() && values.low.is_none() {
                return None;
            }
            let high = values
                .high
                .as_ref()
                .map(|(v, _)| v.trim_end_matches('F').to_owned())
                .unwrap_or_else(|| {
                    if day == 0 && !values.has_daytime {
                        "withheld"
                    } else {
                        "unavailable"
                    }
                    .into()
                });
            let low = values
                .low
                .as_ref()
                .map(|(v, _)| v.trim_end_matches('F').to_owned())
                .unwrap_or_else(|| "unavailable".into());
            Some((format!("H {high} / L {low}"), 0., 0))
        }
        Kind::Precipitation => {
            let chance = if night {
                values.nighttime_pop
            } else {
                values.daytime_pop
            }?;
            let bucket = chance_bucket(chance);
            let _ = day;
            Some((format!("{chance:.0}% • {}", values.date), chance, bucket))
        }
    }
}
/// The value column of the town list. The date leads only when the towns' local dates differ
/// (otherwise the panel's valid line already carries it), so the value fits on one line.
fn summary_value(
    kind: Kind,
    day: u8,
    night: bool,
    values: Option<&super::social_forecast::CityMapDay>,
    show_date: bool,
) -> String {
    let Some(values) = values else {
        return MISSING.into();
    };
    let date = if show_date {
        format!("{} • ", compact_date(&values.date))
    } else {
        String::new()
    };
    match kind {
        Kind::HighLow => {
            let high = if day == 0 && !values.has_daytime {
                "withheld".to_owned()
            } else {
                values
                    .high
                    .as_ref()
                    .map(|(v, _)| v.clone())
                    .unwrap_or_else(|| "unavailable".into())
            };
            let low = values
                .low
                .as_ref()
                .map(|(v, _)| v.clone())
                .unwrap_or_else(|| "unavailable".into());
            if values.high.is_none() && values.low.is_none() {
                MISSING.into()
            } else {
                format!("{date}H {high} • L {low}")
            }
        }
        Kind::Precipitation => {
            let value = if night {
                values.nighttime_pop
            } else {
                values.daytime_pop
            };
            value
                .map(|v| format!("{date}PoP {v:.0}%"))
                .unwrap_or_else(|| MISSING.into())
        }
    }
}

/// Map markers for the town points. Every marker carries the town's list number. At state
/// scale towns closer than about a county width share one marker ("Towns 1–6") instead of
/// stacking callouts that hide each other; the list beside the map gives each town's value.
fn town_markers(
    towns: Vec<(usize, ReportPoint)>,
    coverage: Coverage,
    precipitation: bool,
) -> Vec<ReportPoint> {
    let numbered = |n: usize, mut p: ReportPoint| {
        p.label = format!("{n} • {}", p.label);
        p
    };
    if coverage != Coverage::State {
        return towns.into_iter().map(|(n, p)| numbered(n, p)).collect();
    }
    const NEAR_DEG: f64 = 0.3;
    let mut groups: Vec<Vec<(usize, ReportPoint)>> = Vec::new();
    for (n, p) in towns {
        match groups.iter_mut().find(|g| {
            g.iter()
                .any(|(_, q)| (q.lon - p.lon).abs() < NEAR_DEG && (q.lat - p.lat).abs() < NEAR_DEG)
        }) {
            Some(g) => g.push((n, p)),
            None => groups.push(vec![(n, p)]),
        }
    }
    groups
        .into_iter()
        .map(|mut g| {
            if g.len() == 1 {
                let (n, p) = g.remove(0);
                return numbered(n, p);
            }
            let count = g.len() as f64;
            let lon = g.iter().map(|(_, p)| p.lon).sum::<f64>() / count;
            let lat = g.iter().map(|(_, p)| p.lat).sum::<f64>() / count;
            let numbers: Vec<usize> = g.iter().map(|(n, _)| *n).collect();
            let contiguous = numbers.windows(2).all(|w| w[1] == w[0] + 1);
            let label = if contiguous {
                format!("Towns {}–{}", numbers[0], numbers[numbers.len() - 1])
            } else {
                format!(
                    "Towns {}",
                    numbers.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")
                )
            };
            // Precipitation keeps the group's highest chance (its colour bucket and value).
            let top = g
                .iter()
                .max_by(|a, b| {
                    a.1.value
                        .unwrap_or(0.)
                        .partial_cmp(&b.1.value.unwrap_or(0.))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(_, p)| (p.category, p.value))
                .unwrap_or((0, None));
            ReportPoint {
                id: format!("towns-{}-{lon:.4}-{lat:.4}", numbers.iter().map(|n| n.to_string()).collect::<Vec<_>>().join("_")),
                lon,
                lat,
                label,
                category: top.0,
                value: if precipitation { top.1 } else { None },
            }
        })
        .collect()
}
fn compact_date(date: &str) -> &str {
    date.split_once(' ').map(|(_, date)| date).unwrap_or(date)
}
fn scene(
    area: &CoverageInfo,
    coverage: Coverage,
    kind: Kind,
    day: u8,
    night: bool,
    rows: Option<&[super::city_forecast::TownForecastMapRow]>,
) -> Scene {
    let title = match kind {
        Kind::HighLow => "Town high and low points",
        Kind::Precipitation => "Town precipitation chances",
    };
    let mut dates = rows
        .into_iter()
        .flatten()
        .take(6)
        .filter_map(|row| row.values.as_ref().map(|v| v.date.as_str()))
        .collect::<Vec<_>>();
    dates.sort_unstable();
    dates.dedup();
    let mut towns = Vec::new();
    let mut summary = Vec::new();
    if let Some(rows) = rows {
        for (i, row) in rows.iter().take(6).enumerate() {
            let value = row.values.as_ref();
            // Numbered like the markers on the map.
            summary.push((
                format!("{}. {}", i + 1, row.name),
                summary_value(kind, day, night, value, dates.len() > 1),
            ));
            if let Some(values) = value {
                if let Some((point_text, numeric, bucket)) = point_label(kind, day, night, values) {
                    towns.push((
                        i + 1,
                        ReportPoint {
                            id: format!("town-{i}-{}-{:.4}-{:.4}", day, row.lon, row.lat),
                            lon: row.lon,
                            lat: row.lat,
                            label: point_text,
                            category: bucket,
                            value: (kind == Kind::Precipitation).then_some(numeric),
                        },
                    ));
                }
            }
        }
    }
    let points = town_markers(towns, coverage, kind == Kind::Precipitation);
    let valid = match dates.as_slice() {
        [] => period_label(day, night).to_owned(),
        [date] => (*date).to_owned(),
        _ => "Local dates by town".into(),
    };
    let issue = rows
        .into_iter()
        .flatten()
        .find_map(|row| row.values.as_ref().map(|v| v.issue.as_str()))
        .unwrap_or("");
    let detail = if rows.is_some() && !issue.is_empty() {
        format!("Up to six selected town points • Own NWS local periods • {issue}")
    } else if rows.is_some() {
        "Up to six selected town points • Each town uses its own NWS local period".into()
    } else {
        "Selected-town forecasts are not available yet".into()
    };
    Scene {
        reviewed: rows.is_some(),
        scope: area.scope.clone(),
        palette: Palette::Category,
        legend_title: match kind {
            Kind::HighLow => "NWS 12-hour point periods • temperature in °F".into(),
            Kind::Precipitation => "Probability of measurable precipitation by town".into(),
        },
        legend: bands(kind),
        panels: vec![Panel {
            title: format!("{} • {title}", area.label),
            valid,
            detail,
            points: rows.and_then(|_| (!points.is_empty()).then_some(points)),
            summary: rows.map(|_| summary),
            summary_title: "Selected towns • local values".into(),
            ..Default::default()
        }],
    }
}

pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let areas = coverage_info(settings.home);
    let mut unavailable_scope = board::Scope::default();
    unavailable_scope.states.clear();
    unavailable_scope.county = None;
    unavailable_scope.bbox = None;
    let unavailable_area = CoverageInfo {
        label: "Selected area unavailable".into(),
        scope: unavailable_scope,
    };
    let mut seen = std::collections::HashSet::<String>::new();
    for spec in SPECS {
        if spec.format != 0 {
            continue;
        }
        let semantic_key = format!(
            "{}:{}:{}:{}",
            kind_key(spec.kind),
            coverage_key(spec.coverage),
            spec.day,
            spec.night
        );
        if !seen.insert(semantic_key) {
            continue;
        }
        let rows = super::city_forecast::town_forecast_map_rows(
            settings.home,
            spec.coverage,
            spec.day,
            now,
        );
        let rows = rows.as_deref();
        let area = areas
            .as_ref()
            .and_then(|areas| areas.get(coverage_key(spec.coverage)))
            .unwrap_or(&unavailable_area);
        let payload = serde_json::to_string(&scene(area, spec.coverage, spec.kind, spec.day, spec.night, rows))
            .unwrap_or_default();
        let p = prefix(spec);
        b.set(&format!("{p}.payload"), &payload);
        b.set(
            &format!("{p}.period"),
            &format!(
                "{} • NWS local time at each town",
                period_label(spec.day, spec.night)
            ),
        );
        b.set(&format!("{p}.scope"), &area.label);
        b.set(
            &format!("{p}.source"),
            "Source: NOAA / National Weather Service • Verified town point forecast periods",
        );
        b.set(
            &format!("{p}.note"),
            if spec.kind == Kind::Precipitation {
                "Probability of precipitation (not rain-only) • Up to six selected towns • Local conditions vary"
            } else {
                "Up to six selected towns • Not a complete area forecast"
            },
        );
    }
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = *SPECS.iter().find(|spec| spec.key == key)?;
    let p = prefix(&spec);
    let title = match spec.kind {
        Kind::HighLow => "Town Highs and Lows",
        Kind::Precipitation => "Precipitation Chance",
    };
    let subtitle = format!("{{{{{p}.period}}}} • {{{{{p}.scope}}}}", p = p);
    let source = format!("{{{{{p}.source}}}}", p = p);
    let mut doc = if spec.format == 3 {
        GraphicDoc::new(title, "Forecast", None)
    } else if spec.format == 0 {
        frame(title, "Forecast", &subtitle, &source)
    } else {
        social_frame(title, "Forecast", &subtitle, &source, spec.format == 1)
    };
    if spec.format == 3 {
        let (x, y, w, h) = (46., 360., 1008., 692.);
        doc.layers.push(
            Layer::rect(
                "Town forecast overlay plate",
                x,
                y,
                w,
                h,
                Fill::solid("$panel"),
                20.,
            )
            .with_anim(AnimKind::Fade, 0.1, 0.35),
        );
        doc.layers.push(
            Layer::rect(
                "Town forecast overlay accent",
                x,
                y,
                10.,
                h,
                Fill::solid("$primary"),
                5.,
            )
            .with_anim(AnimKind::Wipe, 0.1, 0.35),
        );
        // The existing board scene carries the actual selected towns, markers,
        // own values, legend, local date and unavailable/partial states.
        doc.layers.push(
            Layer::risk_map(
                "Town forecast overlay map and values",
                64.,
                392.,
                972.,
                590.,
                &format!("set:board:single:{{{{{p}.payload}}}}", p = p),
            )
            .with_anim(AnimKind::Fade, 0.16, 0.4),
        );
        label(
            &mut doc,
            "Forecast overlay window and scope",
            (74., 984., 770., 23.),
            &format!("{{{{{p}.period}}}} • {{{{{p}.scope}}}}", p = p),
            16.,
            "$text",
            Align::Left,
        );
        label(
            &mut doc,
            "Forecast overlay source",
            (74., 1007., 940., 20.),
            &format!("{{{{{p}.source}}}}", p = p),
            15.,
            "$muted",
            Align::Left,
        );
        label(
            &mut doc,
            "Forecast overlay caveat",
            (74., 1027., 940., 25.),
            &format!("{{{{{p}.note}}}}", p = p),
            15.,
            "$muted",
            Align::Left,
        );
        label(
            &mut doc,
            "Forecast overlay station",
            (810., 365., 220., 18.),
            "{{station}}",
            16.,
            "$muted",
            Align::Right,
        );
        doc.template = Some(key.to_owned());
        return Some(doc);
    }
    let (x, y, w, h, note_box, note_size) = match spec.format {
        1 => (54., 258., 972., 1230., (54., 1520., 972., 90.), 25.),
        2 => (54., 214., 972., 630., (54., 860., 972., 82.), 21.),
        _ => (70., 198., 1780., 720., (80., 922., 1760., 45.), 24.),
    };
    doc.layers.push(
        Layer::risk_map(
            "Selected-town NWS point map",
            x,
            y,
            w,
            h,
            &format!("set:board:single:{{{{{p}.payload}}}}", p = p),
        )
        .with_anim(AnimKind::Fade, 0.18, 0.55),
    );
    label(
        &mut doc,
        "Map scope and completeness",
        note_box,
        &format!("{{{{{p}.note}}}}", p = p),
        note_size,
        "$muted",
        Align::Left,
    );
    // The test harness invokes `sample` using this exact template key.
    // Keep it set here as well as in the registry so module-level sample
    // fixtures exercise the same payload path as registry renders.
    doc.template = Some(key.to_owned());
    Some(doc)
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(spec) = SPECS
        .iter()
        .find(|spec| Some(spec.key) == doc.template.as_deref())
    else {
        return;
    };
    let mut sample_scope = board::Scope {
        states: vec!["18".into()],
        county: (spec.coverage == Coverage::County).then(|| "18019".into()),
        bbox: None,
    };
    if spec.coverage == Coverage::Region {
        sample_scope.bbox = Some([-86.0, 38.15, -85.4, 38.7]);
    }
    let sample_label = match spec.coverage {
        Coverage::County => "Clark County, IN • SAMPLE ONLY",
        Coverage::Region => "Selected home-region counties • SAMPLE ONLY",
        Coverage::State => "Indiana • SAMPLE ONLY",
        Coverage::Multi => "Selected multi-state counties • SAMPLE ONLY",
    };
    let sample_date = [
        "Sat Nov 14",
        "Sun Nov 15",
        "Mon Nov 16",
        "Tue Nov 17",
        "Wed Nov 18",
        "Thu Nov 19",
        "Fri Nov 20",
        "Sat Nov 21",
    ][usize::from(spec.day.min(7))];
    let towns = [
        ("Jeffersonville, IN", -85.7372, 38.2776),
        ("Clarksville, IN", -85.7594, 38.2967),
        ("Sellersburg, IN", -85.7544, 38.3981),
        ("Utica, IN", -85.6544, 38.3331),
        ("Charlestown, IN", -85.6705, 38.4531),
        ("Henryville, IN", -85.7677, 38.5417),
    ];
    let rows = towns
        .iter()
        .enumerate()
        .map(|(i, (name, lon, lat))| {
            let withheld = i == 1 && spec.day == 0 && !spec.night;
            let pop_missing = i == 4;
            let chance = [0., 15., 35., 55., 75., 95.][i];
            let values = super::social_forecast::CityMapDay {
                date: sample_date.into(),
                high: (!withheld).then(|| (format!("{}°F", 71 - i as i32), (71 - i as i32) as f32)),
                low: Some((format!("{}°F", 49 - i as i32), (49 - i as i32) as f32)),
                daytime_pop: (!pop_missing).then_some(chance),
                nighttime_pop: (!pop_missing).then_some(chance),
                has_daytime: !withheld,
                issue: "SAMPLE ONLY".into(),
            };
            super::city_forecast::TownForecastMapRow {
                name: (*name).into(),
                lon: *lon,
                lat: *lat,
                values: Some(values),
            }
        })
        .collect::<Vec<_>>();
    let mut sample_scene = scene(
        &CoverageInfo {
            label: sample_label.into(),
            scope: sample_scope,
        },
        spec.coverage,
        spec.kind,
        spec.day,
        spec.night,
        Some(&rows),
    );
    sample_scene.legend_title = "SAMPLE ONLY • Synthetic values".into();
    if let Some(panel) = sample_scene.panels.first_mut() {
        panel.title = sample_label.into();
        panel.valid = format!(
            "SAMPLE ONLY • {} • local dates by town",
            period_label(spec.day, spec.night)
        );
        panel.detail = "Synthetic sample • Not for broadcast".into();
        panel.summary_title = "SAMPLE ONLY • Selected towns".into();
    }
    let p = prefix(spec);
    let payload = serde_json::to_string(&sample_scene).unwrap_or_default();
    for (suffix, value) in [
        ("payload", payload),
        (
            "source",
            "SAMPLE ONLY • Synthetic data • Not for broadcast".into(),
        ),
        (
            "period",
            format!("SAMPLE ONLY • {}", period_label(spec.day, spec.night)),
        ),
        ("scope", format!("SAMPLE ONLY • {}", sample_label)),
        (
            "note",
            "SAMPLE ONLY • Synthetic values; no live NWS data".into(),
        ),
    ] {
        doc.data_overrides.push((format!("{p}.{suffix}"), value));
    }
}

#[cfg(test)]
mod tests {
    use super::{chance_bucket, scene, CoverageInfo, Kind};
    use crate::gfx::sets::{board, social_forecast::CityMapDay};

    #[test]
    fn precipitation_buckets_preserve_zero_and_boundaries() {
        assert_eq!(chance_bucket(0.), 0);
        assert_eq!(chance_bucket(1.), 1);
        assert_eq!(chance_bucket(20.), 2);
        assert_eq!(chance_bucket(40.), 3);
        assert_eq!(chance_bucket(60.), 4);
    }

    #[test]
    fn missing_temperature_is_labeled_unavailable() {
        let values = CityMapDay {
            date: "Sat Nov 14".into(),
            high: None,
            low: Some(("-12°F".into(), -12.)),
            daytime_pop: None,
            nighttime_pop: None,
            has_daytime: true,
            issue: String::new(),
        };
        assert_eq!(
            super::point_label(Kind::HighLow, 1, false, &values)
                .unwrap()
                .0,
            "H unavailable / L -12°"
        );
    }

    fn area() -> CoverageInfo {
        CoverageInfo {
            label: "Clark County, IN".into(),
            scope: board::Scope {
                states: vec!["18".into()],
                county: Some("18019".into()),
                bbox: None,
            },
        }
    }

    #[test]
    fn partial_negative_high_low_summary_passes_board_parser() {
        let row = super::super::city_forecast::TownForecastMapRow {
            name: "Jeffersonville, IN".into(),
            lon: -85.7372,
            lat: 38.2776,
            values: Some(CityMapDay {
                date: "Sat Nov 14".into(),
                high: None,
                low: Some(("-12°F".into(), -12.)),
                daytime_pop: None,
                nighttime_pop: None,
                has_daytime: true,
                issue: "Updated 06:00 UTC".into(),
            }),
        };
        let map = scene(&area(), super::Coverage::County, Kind::HighLow, 1, false, Some(&[row]));
        let summary = map.panels[0].summary.as_ref().unwrap();
        assert!(summary[0].1.contains("-12°F"));
        assert!(summary[0].1.chars().count() <= 40);
        let json = serde_json::to_string(&map).unwrap();
        assert!(board::parse(&json, board::Layout::Single).is_some());
    }

    #[test]
    fn missing_precipitation_values_do_not_create_points_or_claim_zero() {
        let row = super::super::city_forecast::TownForecastMapRow {
            name: "Jeffersonville, IN".into(),
            lon: -85.7372,
            lat: 38.2776,
            values: Some(CityMapDay {
                date: "Sat Nov 14".into(),
                high: None,
                low: None,
                daytime_pop: None,
                nighttime_pop: Some(0.),
                has_daytime: false,
                issue: String::new(),
            }),
        };
        let map = scene(&area(), super::Coverage::County, Kind::Precipitation, 0, false, Some(&[row]));
        assert!(map.panels[0].points.is_none());
        assert_eq!(map.panels[0].summary.as_ref().unwrap()[0].1, super::MISSING);
        let json = serde_json::to_string(&map).unwrap();
        let parsed = board::parse(&json, board::Layout::Single).unwrap();
        assert!(parsed.panels[0].points.is_none());
        assert!(!parsed.panels[0].available());

        let nighttime = CityMapDay {
            date: "Sat Nov 14".into(),
            high: None,
            low: None,
            daytime_pop: None,
            nighttime_pop: Some(0.),
            has_daytime: false,
            issue: String::new(),
        };
        assert_eq!(
            super::point_label(Kind::Precipitation, 0, true, &nighttime)
                .unwrap()
                .0,
            "0% • Sat Nov 14"
        );
    }

    #[test]
    fn sample_marks_all_tokens_and_uses_six_real_town_points() {
        let spec = super::SPECS
            .iter()
            .find(|s| {
                s.kind == Kind::Precipitation
                    && s.night
                    && s.coverage == super::Coverage::County
                    && s.format == 0
            })
            .unwrap();
        let mut doc = super::build(spec.key).unwrap();
        super::sample(&mut doc);
        let prefix = super::prefix(spec);
        for suffix in ["source", "period", "scope", "note"] {
            let value = doc
                .data_overrides
                .iter()
                .find(|(key, _)| key == &format!("{prefix}.{suffix}"))
                .map(|(_, value)| value)
                .unwrap();
            assert!(value.contains("SAMPLE ONLY"));
        }
        let payload = doc
            .data_overrides
            .iter()
            .find(|(key, _)| key == &format!("{prefix}.payload"))
            .map(|(_, value)| value)
            .unwrap();
        let parsed = board::parse(payload, board::Layout::Single).unwrap();
        let panel = &parsed.panels[0];
        let points = panel.points.as_ref().unwrap();
        assert_eq!(points.len(), 5); // one of six towns intentionally has no PoP
        assert!(points.iter().any(|p| p.value == Some(0.)));
        assert!(panel
            .summary
            .as_ref()
            .unwrap()
            .iter()
            .any(|(_, value)| value == super::MISSING));
        assert!(panel.title.contains("SAMPLE ONLY"));
    }

    /// State maps group towns a few miles apart into one numbered marker instead of stacking
    /// callouts that hide each other; county maps keep one numbered marker per town.
    #[test]
    fn state_scale_groups_crowded_towns_and_numbers_every_marker() {
        let town = |name: &str, lon: f64, lat: f64, hi: &str| super::super::city_forecast::TownForecastMapRow {
            name: name.into(),
            lon,
            lat,
            values: Some(CityMapDay {
                date: "Wed Nov 18".into(),
                high: Some((format!("{hi}°F"), 70.)),
                low: Some(("48°F".into(), 48.)),
                daytime_pop: None,
                nighttime_pop: None,
                has_daytime: true,
                issue: String::new(),
            }),
        };
        let rows = [
            town("Jeffersonville, IN", -85.7372, 38.2776, "71"),
            town("Clarksville, IN", -85.7600, 38.2967, "70"),
            town("Sellersburg, IN", -85.7550, 38.3981, "69"),
            town("Evansville, IN", -87.5711, 37.9716, "72"),
        ];
        let state = scene(&area(), super::Coverage::State, Kind::HighLow, 4, false, Some(&rows));
        let labels: Vec<_> = state.panels[0].points.as_ref().unwrap().iter().map(|p| p.label.clone()).collect();
        assert_eq!(labels, vec!["Towns 1–3".to_string(), "4 • H 72° / L 48°".to_string()]);
        let county = scene(&area(), super::Coverage::County, Kind::HighLow, 4, false, Some(&rows));
        assert_eq!(county.panels[0].points.as_ref().unwrap().len(), 4);
        let summary = state.panels[0].summary.as_ref().unwrap();
        assert_eq!(summary[0], ("1. Jeffersonville, IN".to_string(), "H 71°F • L 48°F".to_string()));
        let json = serde_json::to_string(&state).unwrap();
        assert!(board::parse(&json, board::Layout::Single).is_some());
    }

    #[test]
    fn sample_high_low_covers_six_towns_in_clark_county_scope() {
        let spec = super::SPECS
            .iter()
            .find(|s| {
                s.kind == Kind::HighLow
                    && s.day == 1
                    && s.coverage == super::Coverage::County
                    && s.format == 0
            })
            .unwrap();
        let mut doc = super::build(spec.key).unwrap();
        super::sample(&mut doc);
        let prefix = super::prefix(spec);
        let payload = doc
            .data_overrides
            .iter()
            .find(|(key, _)| key == &format!("{prefix}.payload"))
            .map(|(_, value)| value)
            .unwrap();
        let parsed = board::parse(payload, board::Layout::Single).unwrap();
        let panel = &parsed.panels[0];
        assert_eq!(panel.points.as_ref().unwrap().len(), 6);
        assert_eq!(parsed.scope.county.as_deref(), Some("18019"));
        assert!(panel
            .summary
            .as_ref()
            .unwrap()
            .iter()
            .any(|(name, _)| name == "1. Jeffersonville, IN"));
    }

    #[test]
    fn overlay_variants_reuse_the_existing_town_semantics_and_payload_prefix() {
        let overlays: Vec<_> = super::SPECS
            .iter()
            .filter(|spec| spec.format == 3)
            .collect();
        assert_eq!(overlays.len(), 45);
        for overlay in overlays {
            let source = super::SPECS
                .iter()
                .find(|candidate| {
                    candidate.format == 0
                        && candidate.kind == overlay.kind
                        && candidate.day == overlay.day
                        && candidate.night == overlay.night
                        && candidate.coverage == overlay.coverage
                })
                .expect("existing native town-map semantic source");
            assert_eq!(super::prefix(overlay), super::prefix(source));
            let doc = super::build(overlay.key).expect("overlay builder");
            assert_eq!(doc.template.as_deref(), Some(overlay.key));
            assert!(
                doc.background.is_none(),
                "overlay outside the card stays transparent"
            );
            let card = doc
                .layers
                .iter()
                .find(|layer| layer.name == "Town forecast overlay plate")
                .expect("bounded card");
            assert!(
                card.x > 0. && card.y > 0. && card.x + card.w < 1920. && card.y + card.h < 1080.
            );
            let map = doc
                .layers
                .iter()
                .find(|layer| layer.name == "Town forecast overlay map and values")
                .expect("actual board map");
            assert!(
                matches!(&map.kind, crate::gfx::model::LayerKind::RiskMap { spec } if spec.contains(&format!("set.townmaps.{}.{}.{}", super::kind_key(overlay.kind), super::day_key(overlay.day, overlay.night), super::coverage_key(overlay.coverage))))
            );
            let mut sample_doc = doc;
            super::sample(&mut sample_doc);
            let prefix = super::prefix(overlay);
            let values: std::collections::HashMap<_, _> =
                sample_doc.data_overrides.iter().cloned().collect();
            assert!(values
                .get(&format!("{prefix}.source"))
                .is_some_and(|value| value.contains("SAMPLE ONLY")));
            let payload = values
                .get(&format!("{prefix}.payload"))
                .expect("sample forecast scene");
            let sample_scene =
                board::parse(payload, board::Layout::Single).expect("sample board scene");
            assert!(sample_scene.panels[0]
                .summary
                .as_ref()
                .is_some_and(|rows| rows.len() == 6));
        }
    }
}
