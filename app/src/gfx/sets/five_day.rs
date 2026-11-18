//! Five exact NWS calendar days; one native strip builder, two source scopes.
use super::reference::{frame, label, lowerthird_frame, social_frame, MISSING};
use crate::{
    config::Settings,
    gfx::{
        bind::Bindings,
        model::{AnimKind, Fill, GraphicDoc, Layer},
        text::Align,
    },
};

#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    county: bool,
    format: u8,
}
const SPECS: &[Spec] = &[
    Spec {
        key: "set_five_day__point",
        county: false,
        format: 0,
    },
    Spec {
        key: "set_five_day__county",
        county: true,
        format: 0,
    },
    Spec {
        key: "set_five_day__point_portrait",
        county: false,
        format: 1,
    },
    Spec {
        key: "set_five_day__county_portrait",
        county: true,
        format: 1,
    },
    Spec {
        key: "set_five_day__point_square",
        county: false,
        format: 2,
    },
    Spec {
        key: "set_five_day__county_square",
        county: true,
        format: 2,
    },
    Spec {
        key: "set_five_day__point_lowerthird",
        county: false,
        format: 3,
    },
    Spec {
        key: "set_five_day__county_lowerthird",
        county: true,
        format: 3,
    },
];
fn prefix(county: bool) -> &'static str {
    if county {
        "set.fiveday.county"
    } else {
        "set.fiveday.point"
    }
}
fn binding(p: &str, field: &str) -> String {
    format!("{{{{{p}.{field}}}}}")
}
fn temperature(value: &str) -> &str {
    if value == MISSING {
        ""
    } else {
        value
    }
}

pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    for county in [false, true] {
        let p = prefix(county);
        for day in 0..5u8 {
            let values = super::social_forecast::daily_values(settings.home, county, day, now);
            let temp = |field| {
                values
                    .iter()
                    .find(|(k, _)| *k == field)
                    .map(|(_, v)| temperature(v))
                    .filter(|v| !v.is_empty())
            };
            let pair = [
                temp("daytemp").map(|v| format!("Day {v}")),
                temp("nighttemp").map(|v| format!("Night {v}")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" • ");
            b.set(&format!("{p}.{day}.pair"), &pair);
            for (field, value) in &values {
                let value = if matches!(*field, "daytemp" | "nighttemp") {
                    temperature(value)
                } else {
                    value
                };
                b.set(&format!("{p}.{day}.{field}"), value);
            }
            if day == 0 {
                for field in ["source", "place", "issued"] {
                    if let Some((_, value)) = values.iter().find(|(k, _)| *k == field) {
                        b.set(&format!("{p}.{field}"), value);
                    }
                }
            }
        }
        b.set(&format!("{p}.note"), if county {
            "NWS county representative point • Local conditions vary • Today shows remaining periods"
        } else {
            "NWS home-point forecast • Local conditions vary • Today shows remaining periods"
        });
    }
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|s| s.key == key)?;
    let p = prefix(s.county);
    let subtitle = binding(p, "place");
    let source = binding(p, "source");
    let title = "Five Day Forecast";
    let mut d = match s.format {
        1 | 2 => social_frame(title, "Forecast", &subtitle, &source, s.format == 1),
        3 => lowerthird_frame(title, "Forecast", &subtitle, &source),
        _ => frame(title, "Forecast", &subtitle, &source),
    };
    for day in 0..5 {
        let (x, y, w, h) = match s.format {
            1 => (54., 280. + day as f32 * 265., 972., 240.),
            2 => (54., 218. + day as f32 * 128., 972., 116.),
            3 => (70. + day as f32 * 358., 870., 342., 120.),
            _ => (70. + day as f32 * 358., 260., 342., 580.),
        };
        let plate = if s.format == 3 {
            Layer::rect(&format!("Five day strip plate {day}"), x, y, w, h, Fill::solid("$panel"), 6.)
        } else {
            crate::gfx::templates::panel(&format!("Five day panel {day}"), x, y, w, h)
        };
        d.layers.push(
            plate.with_anim(
                AnimKind::Rise,
                0.1 + day as f32 * 0.04,
                0.35,
            ),
        );
        let f = |name: &str| binding(p, &format!("{day}.{name}"));
        match s.format {
            1 | 2 => {
                let portrait = s.format == 1;
                label(
                    &mut d,
                    &format!("Five day date {day}"),
                    (x + 20., y + 10., 216., h - 20.),
                    &f("period"),
                    if portrait { 36. } else { 27. },
                    "$text",
                    Align::Left,
                );
                label(
                    &mut d,
                    &format!("Five day condition {day}"),
                    (x + 256., y + 12., 386., h - 24.),
                    &f("condition"),
                    if portrait { 36. } else { 26. },
                    "$text",
                    Align::Left,
                );
                label(
                    &mut d,
                    &format!("Five day daytime label {day}"),
                    (x + 672., y + 10., 135., if portrait { 48. } else { 30. }),
                    &f("daylabel"),
                    if portrait { 19. } else { 14. },
                    "$muted",
                    Align::Center,
                );
                label(
                    &mut d,
                    &format!("Five day high {day}"),
                    (
                        x + 662.,
                        y + if portrait { 65. } else { 39. },
                        150.,
                        if portrait { 110. } else { 66. },
                    ),
                    &f("daytemp"),
                    if portrait { 64. } else { 45. },
                    "$text",
                    Align::Center,
                );
                label(
                    &mut d,
                    &format!("Five day night label {day}"),
                    (x + 820., y + 10., 132., if portrait { 48. } else { 30. }),
                    "NIGHTTIME",
                    if portrait { 19. } else { 14. },
                    "$muted",
                    Align::Center,
                );
                label(
                    &mut d,
                    &format!("Five day low {day}"),
                    (
                        x + 812.,
                        y + if portrait { 65. } else { 39. },
                        140.,
                        if portrait { 110. } else { 66. },
                    ),
                    &f("nighttemp"),
                    if portrait { 57. } else { 41. },
                    "$text",
                    Align::Center,
                );
            }
            3 => {
                label(
                    &mut d,
                    &format!("Five day date {day}"),
                    (x + 12., y + 3., w - 24., 30.),
                    &f("period"),
                    19.,
                    "$primary",
                    Align::Center,
                );
                label(
                    &mut d,
                    &format!("Five day temperatures {day}"),
                    (x + 12., y + 35., w - 24., 34.),
                    &f("pair"),
                    26.,
                    "$text",
                    Align::Center,
                );
                label(
                    &mut d,
                    &format!("Five day condition {day}"),
                    (x + 12., y + 73., w - 24., 42.),
                    &f("condition"),
                    19.,
                    "$text",
                    Align::Center,
                );
            }
            _ => {
                label(
                    &mut d,
                    &format!("Five day date {day}"),
                    (x + 18., y + 20., w - 36., 88.),
                    &f("period"),
                    35.,
                    "$primary",
                    Align::Center,
                );
                label(
                    &mut d,
                    &format!("Five day daytime label {day}"),
                    (x + 18., y + 125., w - 36., 60.),
                    &f("daylabel"),
                    26.,
                    "$muted",
                    Align::Center,
                );
                label(
                    &mut d,
                    &format!("Five day high {day}"),
                    (x + 18., y + 190., w - 36., 115.),
                    &f("daytemp"),
                    88.,
                    "$text",
                    Align::Center,
                );
                label(
                    &mut d,
                    &format!("Five day night label {day}"),
                    (x + 18., y + 318., w - 36., 38.),
                    "NIGHTTIME",
                    23.,
                    "$muted",
                    Align::Center,
                );
                label(
                    &mut d,
                    &format!("Five day low {day}"),
                    (x + 18., y + 358., w - 36., 78.),
                    &f("nighttemp"),
                    58.,
                    "$text",
                    Align::Center,
                );
                label(
                    &mut d,
                    &format!("Five day condition {day}"),
                    (x + 18., y + 452., w - 36., 108.),
                    &f("condition"),
                    31.,
                    "$text",
                    Align::Center,
                );
            }
        }
    }
    if s.format != 3 {
        let (y, width, size) = match s.format {
            1 => (1630., 972., 27.),
            2 => (876., 972., 22.),
            _ => (870., 1780., 27.),
        };
        let x = if s.format == 0 { 70. } else { 54. };
        label(
            &mut d,
            "Five day note",
            (x, y, width, 64.),
            &binding(p, "note"),
            size,
            "$muted",
            Align::Left,
        );
        label(
            &mut d,
            "Five day issue",
            (x, y + 67., width, 30.),
            &binding(p, "issued"),
            size - 4.,
            "$muted",
            Align::Left,
        );
    }
    Some(d)
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(s) = SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
    else {
        return;
    };
    let p = prefix(s.county);
    let today = chrono::Utc::now().date_naive();
    for day in 0..5 {
        let date = today + chrono::Duration::days(day);
        for (field, value) in [
            ("period", format!("SAMPLE • {}", date.format("%a %b %d"))),
            (
                "condition",
                [
                    "Partly cloudy",
                    "Chance of showers",
                    "Mostly sunny",
                    "Rain and thunderstorms possible",
                    "Mostly cloudy",
                ][day as usize]
                    .into(),
            ),
            (
                "daylabel",
                if day == 0 {
                    "DAYTIME PERIOD ENDED"
                } else {
                    "DAYTIME"
                }
                .into(),
            ),
            (
                "daytemp",
                if day == 0 {
                    String::new()
                } else {
                    format!("{}°F", 70 + day)
                },
            ),
            ("nighttemp", format!("{}°F", 48 + day)),
            (
                "pair",
                if day == 0 {
                    "Night 48°F".into()
                } else {
                    format!("Day {}°F • Night {}°F", 70 + day, 48 + day)
                },
            ),
        ] {
            doc.data_overrides
                .push((format!("{p}.{day}.{field}"), value));
        }
    }
    for (field, value) in [
        (
            "place",
            "Jeffersonville, Indiana • SAMPLE representative point",
        ),
        ("source", "SAMPLE ONLY • Not for broadcast"),
        (
            "note",
            "SAMPLE • Five NWS-style calendar days • Daytime-ended state exercised",
        ),
        ("issued", "SAMPLE • Synthetic forecast periods"),
    ] {
        doc.data_overrides
            .push((format!("{p}.{field}"), value.into()));
    }
}
