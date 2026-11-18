//! Native line charts backed by a fresh, contiguous NWS hourly point forecast.
use super::{
    hourly_forecast::{self, Hour, Snapshot},
    reference::{frame, label, social_frame},
};
use crate::{
    config::Settings,
    gfx::{
        bind::Bindings,
        chart::ChartKind,
        model::{AnimKind, GraphicDoc, Layer, LayerKind},
        templates::panel,
        text::Align,
    },
};
use chrono::DateTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Metric {
    Temperature,
    Wind,
    Dewpoint,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Full,
    Portrait,
    Square,
}
#[allow(dead_code)] // catalog ids and base keys are carried for exact catalog integration.
#[derive(Clone, Copy)]
struct Spec {
    id: &'static str,
    key: &'static str,
    old_key: &'static str,
    metric: Metric,
    hours: usize,
    format: Format,
}
include!("hourly_charts_specs.rs");

impl Metric {
    fn title(self) -> &'static str {
        match self {
            Self::Temperature => "Temperature trend",
            Self::Wind => "Wind timeline",
            Self::Dewpoint => "Dew point timeline",
        }
    }
    fn unit(self) -> &'static str {
        match self {
            Self::Temperature | Self::Dewpoint => "°F",
            Self::Wind => "mph",
        }
    }
    fn color(self) -> &'static str {
        match self {
            Self::Temperature => "#FF9A55",
            Self::Wind => "#57C7FF",
            Self::Dewpoint => "#55D6BE",
        }
    }
    fn value(self, h: &Hour) -> Option<f64> {
        match self {
            Self::Temperature => h.temperature_f,
            Self::Wind => h.wind_mph,
            Self::Dewpoint => h.dewpoint_f,
        }
    }
}
fn token(s: Spec, field: &str) -> String {
    format!("set.hourly_chart.{}.{}", s.key, field)
}
fn fmt_local(t: &DateTime<chrono::FixedOffset>) -> String {
    t.format("%a %b %-d %H:%M %:z").to_string()
}
fn selected(s: &Snapshot, hours: usize, now: i64, metric: Metric) -> Option<Vec<(&Hour, f64)>> {
    let first = s.hours.iter().position(|h| h.end.timestamp() > now)?;
    let range = s.hours.get(first..first.checked_add(hours)?)?;
    if range.len() != hours
        || range.iter().any(|h| Metric::value(metric, h).is_none())
        || range
            .windows(2)
            .any(|w| w[1].start.timestamp() != w[0].end.timestamp())
    {
        return None;
    }
    Some(
        range
            .iter()
            .map(|h| (h, metric.value(h).unwrap()))
            .collect(),
    )
}
fn inline_series(spec: Spec, points: &[(&Hour, f64)]) -> String {
    // Match the shared chart's thinning rule, but supply only well-spaced interior
    // tick labels. Every hourly value remains plotted; endpoints are stated in Valid.
    let width = if spec.format == Format::Full {
        1744_f32
    } else {
        968_f32
    };
    let text_size = 34_f32;
    let plot_width = width - text_size * 3.4;
    let every = ((points.len() as f32 * text_size * 3.2) / plot_width)
        .ceil()
        .max(1.) as usize;
    let stride = (points.len().div_ceil(4)).div_ceil(every) * every;
    points
        .iter()
        .enumerate()
        .map(|(i, (h, value))| {
            let labeled = i >= every && i + every < points.len() && (i - every) % stride == 0;
            let label = if labeled {
                h.start.format("%I%p %z").to_string()
            } else {
                String::new()
            };
            format!("{label}:{value:.1}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn bind_one(b: &mut Bindings, spec: Spec, snap: Option<&Snapshot>, settings: &Settings, now: i64) {
    let base = format!("set.hourly_chart.{}", spec.key);
    let mut series = String::new();
    let mut valid = "Data not available yet • No complete hourly forecast window".to_string();
    let mut source = "Source: NOAA / NWS forecastHourly • Home point".to_string();
    let mut note =
        "Data not available yet • No gaps are bridged and missing values are not zero.".to_string();
    if let Some(snapshot) = snap {
        source = format!(
            "Source: NOAA / NWS forecastHourly • {} • Issued {} UTC",
            snapshot.place,
            DateTime::from_timestamp(snapshot.issued, 0)
                .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "issue time unavailable".into())
        );
        if let Some(points) = selected(snapshot, spec.hours, now, spec.metric) {
            let first = points.first().map(|p| p.0.start.clone());
            let last = points.last().map(|p| p.0.end.clone());
            if let (Some(first), Some(last)) = (first, last) {
                valid = format!(
                    "{} forecast hours • Valid {} through {} • {}",
                    spec.hours,
                    fmt_local(&first),
                    fmt_local(&last),
                    snapshot.timezone
                );
                series = inline_series(spec, &points);
                note = match spec.metric {
                    Metric::Temperature => "NWS hourly temperature • °F • Hourly timestamps retain the source UTC offset.".into(),
                    Metric::Dewpoint => "NWS hourly dew point • °F • Hourly timestamps retain the source UTC offset.".into(),
                    Metric::Wind => "NWS hourly wind speed • mph • Only source-published single values are plotted; no range midpoint or gust is inferred.".into(),
                };
            }
        }
    }
    // The station settings are checked again here so a snapshot for another point cannot bind.
    let snapshot_matches =
        snap.is_some_and(|v| v.home == [settings.home.lon.to_bits(), settings.home.lat.to_bits()]);
    if !snapshot_matches {
        series.clear();
        valid = "Data not available yet • No current NWS forecast for this point".into();
        note =
            "Data not available yet • No gaps are bridged and missing values are not zero.".into();
    }
    b.set(&format!("{base}.series"), &series);
    b.set(&format!("{base}.valid"), &valid);
    b.set(&format!("{base}.subtitle"), "NWS hourly point forecast");
    b.set(&format!("{base}.source"), &source);
    b.set(&format!("{base}.note"), &note);
}

pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let snapshot = hourly_forecast::current(settings.home);
    for spec in SPECS {
        bind_one(b, *spec, snapshot.as_deref(), settings, now);
    }
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = SPECS.iter().find(|s| s.key == key).copied()?;
    let heading = if spec.format == Format::Full {
        match spec.metric {
            Metric::Temperature => "Temperature",
            Metric::Wind => "Wind speed",
            Metric::Dewpoint => "Dew point",
        }
    } else {
        spec.metric.title()
    };
    let title = format!("{} • Next {} hours", heading, spec.hours);
    let source = format!("{{{{{}}}}}", token(spec, "source"));
    let valid = format!("{{{{{}}}}}", token(spec, "valid"));
    let subtitle = format!("{{{{{}}}}}", token(spec, "subtitle"));
    let mut doc = match spec.format {
        Format::Full => frame(&title, "Forecast", &subtitle, &source),
        Format::Portrait => social_frame(&title, "Forecast", &subtitle, &source, true),
        Format::Square => social_frame(&title, "Forecast", &subtitle, &source, false),
    };
    let (box_, note_box) = match spec.format {
        Format::Full => ((88., 275., 1744., 540.), (90., 896., 1738., 60.)),
        Format::Portrait => ((56., 300., 968., 1110.), (58., 1550., 964., 110.)),
        Format::Square => ((56., 255., 968., 520.), (58., 875., 964., 70.)),
    };
    if spec.format == Format::Full {
        doc.layers.push(
            panel("Hourly chart panel", 64., 205., 1792., 770.).with_anim(
                AnimKind::Fade,
                0.15,
                0.4,
            ),
        );
    }
    let valid_box = match spec.format {
        Format::Full => (90., 830., 1738., 58.),
        Format::Portrait => (58., 1425., 964., 110.),
        Format::Square => (58., 788., 964., 80.),
    };
    label(
        &mut doc,
        "Hourly forecast valid period",
        valid_box,
        &valid,
        if spec.format == Format::Portrait {
            25.
        } else {
            21.
        },
        "$text",
        Align::Left,
    );
    label(
        &mut doc,
        "Hourly chart unit",
        (box_.0 + 8., box_.1 - 38., box_.2 - 16., 30.),
        &format!("{} • {}", spec.metric.title(), spec.metric.unit()),
        22.,
        "$muted",
        Align::Left,
    );
    let series = format!("{{{{{}}}}}", token(spec, "series"));
    doc.layers.push(
        Layer::new(
            "Hourly forecast chart",
            box_.0,
            box_.1,
            box_.2,
            box_.3,
            LayerKind::Chart {
                chart: ChartKind::Line,
                series,
                series2: String::new(),
                color: spec.metric.color().into(),
                color2: "#FFFFFF".into(),
                text_color: "$text".into(),
                show_values: false,
                min: None,
                max: None,
                unit: String::new(),
                highlight: String::new(),
                note: String::new(),
            },
        )
        .with_anim(AnimKind::Fade, 0.25, 0.5),
    );
    label(
        &mut doc,
        "Hourly source and coverage note",
        note_box,
        &format!("{{{{{}}}}}", token(spec, "note")),
        if spec.format == Format::Portrait {
            25.
        } else {
            21.
        },
        "$muted",
        Align::Left,
    );
    doc.template = Some(key.into());
    Some(doc)
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(spec) = SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
        .copied()
    else {
        return;
    };
    let offset = chrono::FixedOffset::east_opt(-5 * 3600).unwrap();
    let now = DateTime::parse_from_rfc3339("2026-11-14T18:00:00Z")
        .unwrap()
        .timestamp();
    let mut points = Vec::new();
    for i in 0..spec.hours {
        let start = DateTime::from_timestamp(now + i as i64 * 3600, 0)
            .unwrap()
            .with_timezone(&offset);
        let end = DateTime::from_timestamp(now + (i as i64 + 1) * 3600, 0)
            .unwrap()
            .with_timezone(&offset);
        let value = match spec.metric {
            Metric::Temperature => 61. + (i as f64 * 0.7).sin() * 8.,
            Metric::Dewpoint => 49. + (i as f64 * 0.4).sin() * 4.,
            Metric::Wind => 5. + (i % 7) as f64,
        };
        points.push((
            Hour {
                start,
                end,
                temperature_f: None,
                dewpoint_f: None,
                wind_mph: None,
            },
            value,
        ));
    }
    let prefix = format!("set.hourly_chart.{}", spec.key);
    for (field, value) in [
        ("subtitle", format!("SAMPLE • {} hourly forecast periods", spec.hours)),
        (
            "series",
            inline_series(spec, &points.iter().map(|(h, v)| (h, *v)).collect::<Vec<_>>()),
        ),
        (
            "valid",
            format!(
                "SAMPLE • {} forecast hours • Valid {} through {} • America/Indiana/Indianapolis",
                spec.hours, fmt_local(&points.first().unwrap().0.start), fmt_local(&points.last().unwrap().0.end)
            ),
        ),
        (
            "source",
            "SAMPLE ONLY • Synthetic NWS point 38.278, -85.738 • Issued 2026-11-14 18:00 UTC • Not for broadcast".into(),
        ),
        (
            "note",
            format!(
                "SAMPLE • Synthetic {} series • Not a live forecast",
                spec.metric.title()
            ),
        ),
    ] {
        doc.data_overrides
            .push((format!("{prefix}.{field}"), value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_registry_has_34_new_rows_and_keeps_existing_base_keys() {
        assert_eq!(SPECS.len(), 34);

        assert_eq!(
            SPECS
                .iter()
                .filter(|s| s.metric == Metric::Temperature)
                .count(),
            11
        );
        assert_eq!(
            SPECS.iter().filter(|s| s.metric == Metric::Wind).count(),
            11
        );
        assert_eq!(
            SPECS
                .iter()
                .filter(|s| s.metric == Metric::Dewpoint)
                .count(),
            12
        );
        assert!(SPECS.iter().any(|s| s.old_key == "hourly_temp_chart"));
        assert!(SPECS.iter().any(|s| s.old_key == "wind_timeline"));
        assert!(SPECS.iter().all(|s| build(s.key).is_some()));
    }
    #[test]
    fn incomplete_or_noncontiguous_windows_are_withheld() {
        let start = DateTime::parse_from_rfc3339("2026-11-14T12:00:00-05:00").unwrap();
        let make = |offset: i64, temp| Hour {
            start: start + chrono::Duration::hours(offset),
            end: start + chrono::Duration::hours(offset + 1),
            temperature_f: temp,
            dewpoint_f: Some(45.),
            wind_mph: Some(5.),
        };
        let snapshot = Snapshot {
            home: [0, 0],
            generation: 1,
            checked: start.timestamp(),
            issued: start.timestamp(),
            place: "Home point".into(),
            timezone: "America/Indiana/Indianapolis".into(),
            hours: vec![
                make(0, Some(60.)),
                make(1, Some(61.)),
                make(2, None),
                make(3, Some(63.)),
            ],
        };
        let now = start.timestamp() - 1;
        assert!(selected(&snapshot, 4, now, Metric::Temperature).is_none());
        assert!(selected(&snapshot, 5, now, Metric::Dewpoint).is_none());
        let mut gap = snapshot.clone();
        gap.hours[1].start += chrono::Duration::minutes(15);
        assert!(selected(&gap, 3, now, Metric::Dewpoint).is_none());
    }
}
