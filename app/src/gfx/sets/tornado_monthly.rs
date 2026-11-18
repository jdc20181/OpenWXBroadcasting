//! Monthly tornado climatology and complete previous-year comparison; shared SPC archive.
use crate::{
    config::Settings,
    gfx::{
        bind::Bindings,
        chart::ChartKind,
        model::{AnimKind, GraphicDoc, Layer, LayerKind},
        text::Align,
    },
};
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Climatology,
    LastYear,
}
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    kind: Kind,
    national: bool,
    format: Option<bool>,
}
const SPECS: &[Spec] = &[
    Spec {
        key: "set_tornado_monthly_climatology__state",
        kind: Kind::Climatology,
        national: false,
        format: None,
    },
    Spec {
        key: "set_tornado_monthly_climatology__national",
        kind: Kind::Climatology,
        national: true,
        format: None,
    },
    Spec {
        key: "set_tornado_monthly_climatology__state_portrait",
        kind: Kind::Climatology,
        national: false,
        format: Some(true),
    },
    Spec {
        key: "set_tornado_monthly_climatology__national_portrait",
        kind: Kind::Climatology,
        national: true,
        format: Some(true),
    },
    Spec {
        key: "set_tornado_monthly_climatology__state_square",
        kind: Kind::Climatology,
        national: false,
        format: Some(false),
    },
    Spec {
        key: "set_tornado_monthly_climatology__national_square",
        kind: Kind::Climatology,
        national: true,
        format: Some(false),
    },
    Spec {
        key: "set_tornado_count_average__lastyear_state",
        kind: Kind::LastYear,
        national: false,
        format: None,
    },
    Spec {
        key: "set_tornado_count_average__lastyear_national",
        kind: Kind::LastYear,
        national: true,
        format: None,
    },
    Spec {
        key: "set_tornado_count_average__lastyear_state_portrait",
        kind: Kind::LastYear,
        national: false,
        format: Some(true),
    },
    Spec {
        key: "set_tornado_count_average__lastyear_national_portrait",
        kind: Kind::LastYear,
        national: true,
        format: Some(true),
    },
    Spec {
        key: "set_tornado_count_average__lastyear_state_square",
        kind: Kind::LastYear,
        national: false,
        format: Some(false),
    },
    Spec {
        key: "set_tornado_count_average__lastyear_national_square",
        kind: Kind::LastYear,
        national: true,
        format: Some(false),
    },
];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
fn prefix(s: Spec) -> String {
    format!(
        "set.tornadomonth.{}.{}",
        if s.kind == Kind::Climatology {
            "climo"
        } else {
            "lastyear"
        },
        if s.national { "national" } else { "state" }
    )
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|s| s.key == key)?;
    let p = prefix(s);
    let token = |f: &str| format!("{{{{{p}.{f}}}}}");
    let title = if s.kind == Kind::Climatology {
        "Tornado Climatology"
    } else {
        "Tornado Count vs Average"
    };
    let mut d = match s.format {
        None => super::reference::frame(title, "Tornado", &token("period"), &token("source")),
        Some(portrait) => super::reference::social_frame(
            title,
            "Tornado",
            &token("period"),
            &token("source"),
            portrait,
        ),
    };
    let portrait = s.format == Some(true);
    let social = s.format.is_some();
    let left = if social { 54. } else { 80. };
    let width = if social { 972. } else { 1760. };
    let label = super::reference::label;
    label(
        &mut d,
        "Archive scope",
        (left, if social { 238. } else { 195. }, width, 64.),
        &token("scope"),
        if social { 34. } else { 38. },
        "$text",
        Align::Left,
    );
    let headings = if s.kind == Kind::Climatology {
        ["ANNUAL AVERAGE", "PEAK MONTH", "MONTHLY PEAK"]
    } else {
        ["LAST-YEAR COUNT", "1991–2020 AVERAGE", "DIFFERENCE"]
    };
    let fields = if s.kind == Kind::Climatology {
        ["mean", "peakmonth", "peak"]
    } else {
        ["count", "mean", "delta"]
    };
    for i in 0..3 {
        let (x, y, w, h) = if portrait {
            (left, 340. + i as f32 * 174., width, 154.)
        } else {
            let w = (width - 36.) / 3.;
            (
                left + i as f32 * (w + 18.),
                if social { 330. } else { 280. },
                w,
                150.,
            )
        };
        d.layers.push(
            crate::gfx::templates::panel(&format!("Summary {i}"), x, y, w, h).with_anim(
                AnimKind::Rise,
                0.2 + i as f32 * 0.08,
                0.4,
            ),
        );
        label(
            &mut d,
            &format!("Summary heading {i}"),
            (x + 16., y + 12., w - 32., 40.),
            headings[i],
            if social && !portrait { 21. } else { 27. },
            "$muted",
            Align::Left,
        );
        label(
            &mut d,
            &format!("Summary value {i}"),
            (x + 16., y + 60., w - 32., 72.),
            &token(fields[i]),
            if portrait { 52. } else { 44. },
            "$text",
            Align::Left,
        );
    }
    let (chart_y, chart_h, legend_y, note_y) = if portrait {
        (930., 600., 1570., 1675.)
    } else if social {
        (510., 340., 860., 922.)
    } else {
        (485., 380., 875., 930.)
    };
    d.layers.push(
        Layer::new(
            "Monthly archive chart",
            left,
            chart_y,
            width,
            chart_h,
            LayerKind::Chart {
                chart: if s.kind == Kind::Climatology {
                    ChartKind::Bar
                } else {
                    ChartKind::Line
                },
                series: token("series"),
                series2: token("average"),
                color: "$primary".into(),
                color2: "#FFCC66".into(),
                text_color: "$text".into(),
                show_values: false,
                min: Some(0.),
                max: None,
                unit: String::new(),
                highlight: String::new(),
                note: String::new(),
            },
        )
        .with_anim(AnimKind::Fade, 0.3, 0.5),
    );
    label(
        &mut d,
        "Primary legend",
        (left, legend_y, width * 0.52, 50.),
        &token("legend"),
        if social { 26. } else { 29. },
        "$primary",
        Align::Left,
    );
    if s.kind == Kind::LastYear {
        label(
            &mut d,
            "Average legend",
            (left + width * 0.52, legend_y, width * 0.48, 50.),
            "1991–2020 monthly average",
            if social { 23. } else { 29. },
            "#FFCC66",
            Align::Right,
        );
    }
    label(
        &mut d,
        "Archive caveat",
        (left, note_y, width, if portrait { 62. } else { 44. }),
        &token("note"),
        if social { 23. } else { 25. },
        "$muted",
        Align::Left,
    );
    Some(d)
}
fn series(values: &[f64; 12]) -> String {
    MONTHS
        .iter()
        .zip(values)
        .map(|(m, v)| format!("{m}:{v:.4}"))
        .collect::<Vec<_>>()
        .join(",")
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    use chrono::Datelike;
    let year = chrono::Utc::now().year() - 1;
    for s in SPECS.iter().filter(|s| s.format.is_none()) {
        let p = prefix(*s);
        let mut put = |field: &str, value: &str| b.set(&format!("{p}.{field}"), value);
        put(
            "period",
            &if s.kind == Kind::Climatology {
                "Computed average • 1991–2020".into()
            } else {
                format!("{year} • Complete calendar year")
            },
        );
        put(
            "scope",
            &super::tornado_history::scope_label(settings, s.national),
        );
        put(
            "source",
            "Source: NWS Storm Prediction Center • Archive-derived counts",
        );
        put("series", "");
        put("average", "");
        for f in ["count", "mean", "delta", "peakmonth", "peak"] {
            put(f, super::reference::MISSING);
        }
        put(
            "legend",
            if s.kind == Kind::Climatology {
                "Monthly average • Tornadoes/year"
            } else {
                "Last-year monthly tornado count"
            },
        );
        put(
            "note",
            "Complete SPC archive not available yet. Counts and averages are withheld.",
        );
        let Some((counts, average)) = super::tornado_history::monthly(
            settings,
            s.national,
            if s.kind == Kind::LastYear {
                Some(year)
            } else {
                None
            },
        ) else {
            continue;
        };
        put(
            "mean",
            &format!("{:.1} tornadoes", average.iter().sum::<f64>()),
        );
        put(
            "note",
            "Computed 1991–2020 calendar-month averages • Zero-event years included",
        );
        if s.kind == Kind::Climatology {
            put("series", &series(&average));
            let index = (0..12)
                .max_by(|a, c| average[*a].total_cmp(&average[*c]).then_with(|| c.cmp(a)))
                .unwrap();
            put("peakmonth", MONTHS[index]);
            put("peak", &format!("{:.1} tornadoes/year", average[index]));
        } else {
            put("series", &series(&counts));
            put("average", &series(&average));
            let total = counts.iter().sum::<f64>();
            put("count", &format!("{total:.0} tornadoes"));
            put(
                "delta",
                &format!("{:+.1} tornadoes", total - average.iter().sum::<f64>()),
            );
            put("legend", &format!("{year} monthly tornado count"));
        }
    }
}
#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(s) = SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
    else {
        return;
    };
    let p = prefix(*s);
    let counts = [1., 0., 4., 8., 12., 6., 5., 2., 1., 0., 0., 1.];
    let average = [0.4, 0.7, 2., 4., 6., 5., 3., 2., 1., 0.5, 0.4, 0.3];
    for (f, v) in [
        ("period", "SAMPLE • Complete calendar years"),
        ("scope", "SAMPLE • Archive-derived statistics"),
        ("source", "SAMPLE ONLY • Not for broadcast"),
        ("count", "40 tornadoes"),
        ("mean", "25.3 tornadoes"),
        ("delta", "+14.7 tornadoes"),
        ("peakmonth", "May"),
        ("peak", "6.0 tornadoes/year"),
        (
            "legend",
            if s.kind == Kind::Climatology {
                "SAMPLE • Monthly average • Tornadoes/year"
            } else {
                "SAMPLE • Monthly tornado count"
            },
        ),
        (
            "note",
            "SAMPLE • Computed monthly averages • Zero-event years included",
        ),
    ] {
        doc.data_overrides.push((format!("{p}.{f}"), v.into()));
    }
    doc.data_overrides.push((
        format!("{p}.series"),
        series(if s.kind == Kind::Climatology {
            &average
        } else {
            &counts
        }),
    ));
    doc.data_overrides.push((
        format!("{p}.average"),
        if s.kind == Kind::Climatology {
            String::new()
        } else {
            series(&average)
        },
    ));
}
