//! Native portrait and square layouts for supplied NWS point and County representative-point forecasts.
//! Values and valid-date labels come unchanged from social_forecast::daily_values.

use super::reference::social_frame;
use crate::{
    config::Settings,
    gfx::{
        bind::Bindings,
        model::{AnimKind, GraphicDoc, Layer, LayerKind},
        templates::{panel, text, DISPLAY},
        text::{Align, Weight},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Portrait,
    Square,
}

#[derive(Clone, Copy, Debug)]
struct Spec {
    key: &'static str,
    days: u8,
    format: Format,
    county: bool,
}

const SPECS: &[Spec] = &[
    Spec {
        key: "set_seven_day__portrait",
        days: 7,
        format: Format::Portrait,
        county: false,
    },
    Spec {
        key: "set_seven_day__square",
        days: 7,
        format: Format::Square,
        county: false,
    },
    Spec {
        key: "set_three_day__portrait",
        days: 3,
        format: Format::Portrait,
        county: false,
    },
    Spec {
        key: "set_three_day__square",
        days: 3,
        format: Format::Square,
        county: false,
    },
    Spec {
        key: "set_seven_day__county_portrait",
        days: 7,
        format: Format::Portrait,
        county: true,
    },
    Spec {
        key: "set_seven_day__county_square",
        days: 7,
        format: Format::Square,
        county: true,
    },
    Spec {
        key: "set_three_day__county_portrait",
        days: 3,
        format: Format::Portrait,
        county: true,
    },
    Spec {
        key: "set_three_day__county_square",
        days: 3,
        format: Format::Square,
        county: true,
    },
];

fn namespace(county: bool) -> &'static str {
    if county {
        "set.multiday.county"
    } else {
        "set.multiday"
    }
}

fn day_prefix(day: u8, county: bool) -> String {
    format!("{}.day{day}", namespace(county))
}

fn set_day(b: &mut Bindings, day: u8, fields: &[(&'static str, String)], county: bool) {
    let prefix = day_prefix(day, county);
    for (field, value) in fields {
        b.set(&format!("{prefix}.{field}"), value);
    }
    let field = |name: &str| {
        fields
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
            .unwrap_or("")
    };
    let icon = super::weather_word::icon_for(
        field("condition"),
        field("note").starts_with("Remaining nighttime")
            || (county && field("note").starts_with("Remaining night")),
    );
    b.set(
        &format!("{prefix}.icon"),
        if icon == "none" { "none" } else { icon },
    );
    let missing = |name: &str| {
        let value = field(name);
        value.trim().is_empty() || value == super::reference::MISSING
    };
    let unavailable =
        missing("condition") && missing("daytemp") && missing("nighttemp") && missing("pop");
    b.set(
        &format!("{prefix}.status"),
        if unavailable {
            "Forecast not available yet"
        } else {
            ""
        },
    );
}

fn text_layer(
    name: &str,
    rect: (f32, f32, f32, f32),
    value: &str,
    size: f32,
    color: &str,
    align: Align,
) -> Layer {
    let mut layer = text(name, rect, value, DISPLAY, size, Weight::Bold, color, align);
    if let LayerKind::Text {
        wrap,
        auto_fit,
        vcenter,
        ..
    } = &mut layer.kind
    {
        *wrap = true;
        *auto_fit = true;
        *vcenter = false;
    }
    layer
}

fn day_card(
    doc: &mut GraphicDoc,
    day: u8,
    rect: (f32, f32, f32, f32),
    compact: bool,
    county: bool,
) {
    let (x, y, w, h) = rect;
    let p = day_prefix(day, county);
    doc.layers.push(
        panel(&format!("Forecast day {day} panel"), x, y, w, h).with_anim(
            AnimKind::Rise,
            0.2 + day as f32 * 0.04,
            0.4,
        ),
    );
    if compact {
        doc.layers.push(Layer::risk_map(
            &format!("Forecast day {day} Remix icon"),
            x + 12.,
            y + 11.,
            20.,
            20.,
            &format!("set:cityforecast-icon:{{{{{p}.icon}}}}"),
        ));
        doc.layers.push(text_layer(
            &format!("Forecast day {day} period"),
            (x + 38., y + 7., w - 62., 28.),
            &format!("{{{{{p}.period}}}}"),
            21.,
            "$primary",
            Align::Left,
        ));
        doc.layers.push(text_layer(
            &format!("Forecast day {day} condition"),
            (x + 12., y + 37., w - 24., 40.),
            &format!("{{{{{p}.condition}}}}"),
            17.,
            "$text",
            Align::Left,
        ));
        doc.layers.push(text_layer(
            &format!("Forecast day {day} daytime temperature"),
            (x + 12., y + 78., w - 24., 22.),
            &format!("{{{{{p}.daylabel}}}}  {{{{{p}.daytemp}}}}"),
            20.,
            "$text",
            Align::Left,
        ));
        doc.layers.push(text_layer(
            &format!("Forecast day {day} nighttime temperature"),
            (x + 12., y + 101., w - 24., 22.),
            &format!("NIGHTTIME  {{{{{p}.nighttemp}}}}"),
            20.,
            "$muted",
            Align::Left,
        ));
        doc.layers.push(text_layer(
            &format!("Forecast day {day} precipitation"),
            (x + 12., y + 125., w - 24., 23.),
            &format!("{{{{{p}.poplabel}}}}  {{{{{p}.pop}}}}"),
            18.,
            "$primary",
            Align::Left,
        ));
        doc.layers.push(text_layer(
            &format!("Forecast day {day} status"),
            (x + 12., y + 150., w - 24., 19.),
            &format!("{{{{{p}.status}}}}"),
            14.,
            "$muted",
            Align::Left,
        ));
        return;
    }

    let wide = w > 600.;
    let (
        pad,
        period_h,
        condition_y,
        condition_h,
        label_y,
        temp_y,
        temp_h,
        night_label_y,
        night_y,
        pop_y,
        status_y,
    ) = if wide {
        (24., 44., 62., 112., 185., 214., 66., 280., 309., 378., 420.)
    } else {
        (18., 38., 54., 74., 132., 160., 42., 206., 234., 280., 319.)
    };
    let period_size = if wide { 34. } else { 26. };
    let condition_size = if wide { 42. } else { 25. };
    let value_size = if wide { 58. } else { 35. };
    doc.layers.push(Layer::risk_map(
        &format!("Forecast day {day} Remix icon"),
        x + pad,
        y + 13.,
        34.,
        34.,
        &format!("set:cityforecast-icon:{{{{{p}.icon}}}}"),
    ));
    doc.layers.push(text_layer(
        &format!("Forecast day {day} period"),
        (x + pad + 44., y + 12., w - pad * 2. - 44., period_h),
        &format!("{{{{{p}.period}}}}"),
        period_size,
        "$primary",
        Align::Left,
    ));
    doc.layers.push(text_layer(
        &format!("Forecast day {day} condition"),
        (x + pad, y + condition_y, w - pad * 2., condition_h),
        &format!("{{{{{p}.condition}}}}"),
        condition_size,
        "$text",
        Align::Left,
    ));
    doc.layers.push(text_layer(
        &format!("Forecast day {day} daytime label"),
        (x + pad, y + label_y, w - pad * 2., 26.),
        &format!("{{{{{p}.daylabel}}}}"),
        if wide { 24. } else { 19. },
        "$muted",
        Align::Left,
    ));
    doc.layers.push(text_layer(
        &format!("Forecast day {day} daytime temperature"),
        (x + pad, y + temp_y, w - pad * 2., temp_h),
        &format!("{{{{{p}.daytemp}}}}"),
        value_size,
        "$text",
        Align::Left,
    ));
    doc.layers.push(text_layer(
        &format!("Forecast day {day} nighttime label"),
        (x + pad, y + night_label_y, w - pad * 2., 24.),
        "NIGHTTIME",
        if wide { 23. } else { 18. },
        "$muted",
        Align::Left,
    ));
    doc.layers.push(text_layer(
        &format!("Forecast day {day} nighttime temperature"),
        (
            x + pad,
            y + night_y,
            w - pad * 2.,
            if wide { 58. } else { 39. },
        ),
        &format!("{{{{{p}.nighttemp}}}}"),
        if wide { 48. } else { 31. },
        "$text",
        Align::Left,
    ));
    doc.layers.push(text_layer(
        &format!("Forecast day {day} precipitation"),
        (
            x + pad,
            y + pop_y,
            w - pad * 2.,
            if wide { 38. } else { 34. },
        ),
        &format!("{{{{{p}.poplabel}}}}  {{{{{p}.pop}}}}"),
        if wide { 28. } else { 21. },
        "$primary",
        Align::Left,
    ));
    doc.layers.push(text_layer(
        &format!("Forecast day {day} status"),
        (
            x + pad,
            y + status_y,
            w - pad * 2.,
            if wide { 21. } else { 19. },
        ),
        &format!("{{{{{p}.status}}}}"),
        if wide { 19. } else { 15. },
        "$muted",
        Align::Left,
    ));
    // Keep an explicitly readable panel even when a day has no issued period.
    let _ = h;
}

fn layout(spec: Spec) -> (f32, f32, f32, f32, usize, f32, bool) {
    match (spec.days, spec.format) {
        (7, Format::Portrait) => (54., 228., 474., 345., 2, 20., false),
        (7, Format::Square) => (54., 184., 474., 175., 2, 14., true),
        (3, Format::Portrait) => (54., 228., 972., 445., 1, 22., false),
        (3, Format::Square) => (54., 184., 474., 350., 2, 16., false),
        _ => unreachable!("only supported forecast horizons have catalog rows"),
    }
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = *SPECS.iter().find(|s| s.key == key)?;
    let horizon = if spec.days == 7 {
        "7-Day Forecast"
    } else {
        "3-Day Forecast"
    };
    let title = if spec.county {
        format!("County {horizon}")
    } else {
        horizon.to_owned()
    };
    let (subtitle, source) = if spec.county {
        (
            "{{set.multiday.county.place}}",
            "{{set.multiday.county.source}}",
        )
    } else {
        ("{{set.multiday.place}}", "{{set.multiday.source}}")
    };
    let portrait = spec.format == Format::Portrait;
    let mut doc = social_frame(&title, "Forecast", subtitle, source, portrait);
    let (x0, mut y0, card_w, card_h, columns, gap_y, compact) = layout(spec);
    if spec.county {
        let note_y = if portrait { 219. } else { 180. };
        doc.layers.push(text_layer(
            "County representative-point disclosure",
            (54., note_y, 972., 24.),
            "{{set.multiday.county.note}}",
            18.,
            "$muted",
            Align::Center,
        ));
        y0 += 28.;
    }
    let gap_x = if columns == 2 { 24. } else { 0. };
    for day in 0..spec.days as usize {
        let col = day % columns;
        let row = day / columns;
        let x = x0 + col as f32 * (card_w + gap_x);
        let y = y0 + row as f32 * (card_h + gap_y);
        day_card(
            &mut doc,
            day as u8,
            (x, y, card_w, card_h),
            compact,
            spec.county,
        );
    }
    doc.template = Some(key.into());
    Some(doc)
}

/// Reuses the existing checked NWS point and County representative-point daily values.
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    let point_days: Vec<_> = (0..7)
        .map(|day| super::social_forecast::daily_values(settings.home, false, day, now))
        .collect();
    if let Some(first) = point_days.first() {
        for (field, key) in [
            ("source", "source"),
            ("place", "place"),
            ("issued", "updated"),
            ("note", "note"),
        ] {
            let value = first
                .iter()
                .find(|(name, _)| *name == field)
                .map(|(_, value)| value.as_str())
                .unwrap_or("");
            b.set(&format!("set.multiday.{key}"), value);
        }
    }
    for (day, fields) in point_days.iter().enumerate() {
        set_day(b, day as u8, fields, false);
    }

    let county_days: Vec<_> = (0..7)
        .map(|day| super::social_forecast::daily_values(settings.home, true, day, now))
        .collect();
    if let Some(first) = county_days.first() {
        for (field, key) in [
            ("source", "source"),
            ("place", "place"),
            ("issued", "updated"),
            ("note", "note"),
        ] {
            let value = first
                .iter()
                .find(|(name, _)| *name == field)
                .map(|(_, value)| value.as_str())
                .unwrap_or("");
            b.set(&format!("set.multiday.county.{key}"), value);
        }
    }
    for (day, fields) in county_days.iter().enumerate() {
        set_day(b, day as u8, fields, true);
    }
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(spec) = SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
    else {
        return;
    };
    let anchor = chrono::NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
    let ns = namespace(spec.county);
    let (place, source, note) = if spec.county {
        (
            "SAMPLE • County representative point",
            "SAMPLE ONLY • Synthetic NWS-style County representative-point forecast",
            "SAMPLE ONLY • Representative point • County conditions vary • Synthetic periods",
        )
    } else {
        (
            "SAMPLE • Jeffersonville home point",
            "SAMPLE ONLY • Synthetic NWS-style point forecast",
            "SAMPLE ONLY • Synthetic daily periods",
        )
    };
    doc.data_overrides.extend([
        (format!("{ns}.place"), place.into()),
        (format!("{ns}.source"), source.into()),
        (
            format!("{ns}.updated"),
            "SAMPLE • Synthetic issue time".into(),
        ),
        (format!("{ns}.note"), note.into()),
    ]);
    for day in 0..spec.days {
        let prefix = day_prefix(day, spec.county);
        let date = anchor + chrono::Duration::days(day as i64);
        let period_name = match day {
            0 => "Today",
            1 => "Tomorrow",
            2 => "Day 3",
            _ => "Day",
        };
        let period = if day <= 2 {
            format!("SAMPLE • {period_name} • {}", date.format("%a %b %d, %Y"))
        } else {
            format!("SAMPLE • Day {} • {}", day + 1, date.format("%a %b %d, %Y"))
        };
        let (daylabel, daytemp, nighttemp, condition, poplabel, pop) = if day == 0 {
            (
                "DAYTIME PERIOD ENDED",
                "",
                "SAMPLE • 41°F",
                "SAMPLE • Clear overnight",
                "Nighttime precip. chance",
                "SAMPLE • 20%",
            )
        } else {
            (
                "DAYTIME",
                "SAMPLE • synthetic high",
                "SAMPLE • synthetic low",
                "SAMPLE • Partly sunny with a chance of showers",
                "Daytime precip. chance",
                "SAMPLE • 30%",
            )
        };
        let values = [
            ("period", period),
            ("daylabel", daylabel.to_owned()),
            (
                "daytemp",
                if day == 0 {
                    daytemp.to_owned()
                } else {
                    format!("SAMPLE • {}°F", 72 + day as i32)
                },
            ),
            (
                "nighttemp",
                if day == 0 {
                    nighttemp.to_owned()
                } else {
                    format!("SAMPLE • {}°F", 49 + day as i32)
                },
            ),
            ("condition", condition.to_owned()),
            ("poplabel", poplabel.to_owned()),
            ("pop", pop.to_owned()),
            ("status", "SAMPLE ONLY".to_owned()),
            (
                "note",
                if spec.county && day == 0 {
                    "Remaining night • Representative point • County conditions vary • SAMPLE ONLY"
                        .to_owned()
                } else if spec.county {
                    "Representative point • County conditions vary • SAMPLE ONLY".to_owned()
                } else if day == 0 {
                    "Remaining nighttime forecast • SAMPLE ONLY".to_owned()
                } else {
                    "SAMPLE ONLY • Synthetic daily periods".to_owned()
                },
            ),
        ];
        let condition = values
            .iter()
            .find(|(field, _)| *field == "condition")
            .unwrap()
            .1
            .as_str();
        let note = values
            .iter()
            .find(|(field, _)| *field == "note")
            .unwrap()
            .1
            .as_str();
        let icon = super::weather_word::icon_for(
            condition,
            note.starts_with("Remaining nighttime")
                || (spec.county && note.starts_with("Remaining night")),
        );
        for (field, value) in values {
            doc.data_overrides
                .push((format!("{prefix}.{field}"), value));
        }
        doc.data_overrides.push((
            format!("{prefix}.icon"),
            if icon == "none" {
                "none".to_owned()
            } else {
                icon.to_owned()
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn eight_catalog_layouts_have_native_dimensions_and_bounded_layers() {
        let mut keys = HashSet::new();
        assert_eq!(SPECS.len(), 8);
        for spec in SPECS {
            assert!(spec.key.starts_with("set_"));
            assert!(keys.insert(spec.key));
            let doc = build(spec.key).unwrap();
            assert_eq!(doc.width, 1080);
            assert_eq!(
                doc.height,
                if spec.format == Format::Portrait {
                    1920
                } else {
                    1080
                }
            );
            assert!(
                doc.layers.iter().all(|layer| {
                    layer.x.is_finite()
                        && layer.y.is_finite()
                        && layer.w.is_finite()
                        && layer.h.is_finite()
                        && layer.x >= 0.
                        && layer.y >= 0.
                        && layer.w >= 0.
                        && layer.h >= 0.
                        && layer.x + layer.w <= doc.width as f32
                        && layer.y + layer.h <= doc.height as f32
                }),
                "{} has an out-of-canvas layer",
                spec.key
            );
            assert!(doc.layers.len() < 80);
        }
    }

    #[test]
    fn county_variants_keep_the_existing_point_tokens_and_add_only_county_tokens() {
        assert_eq!(namespace(false), "set.multiday");
        assert_eq!(day_prefix(0, false), "set.multiday.day0");
        assert_eq!(namespace(true), "set.multiday.county");
        assert_eq!(day_prefix(0, true), "set.multiday.county.day0");

        for spec in SPECS {
            let doc = build(spec.key).unwrap();
            let text = doc
                .layers
                .iter()
                .filter_map(|layer| match &layer.kind {
                    LayerKind::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            for day in 0..spec.days {
                let p = day_prefix(day, spec.county);
                for field in [
                    "period",
                    "condition",
                    "daylabel",
                    "daytemp",
                    "nighttemp",
                    "poplabel",
                    "pop",
                ] {
                    assert!(
                        text.contains(&format!("{{{{{p}.{field}}}}}")),
                        "{} missing {field} day {day}",
                        spec.key
                    );
                }
            }
            if spec.county {
                assert!(text.contains("{{set.multiday.county.place}}"));
                assert!(text.contains("{{set.multiday.county.source}}"));
                assert!(text.contains("{{set.multiday.county.note}}"));
                assert!(!text.contains("{{set.multiday.place}}"));
                assert!(!text.contains("{{set.multiday.source}}"));
            } else {
                assert!(text.contains("{{set.multiday.place}}"));
                assert!(text.contains("{{set.multiday.source}}"));
                assert!(!text.contains("set.multiday.county"));
            }
        }
    }

    #[test]
    fn county_note_is_visible_and_sample_discloses_representative_point_scope() {
        for key in [
            "set_seven_day__county_portrait",
            "set_three_day__county_square",
        ] {
            let mut doc = build(key).unwrap();
            let text = doc
                .layers
                .iter()
                .filter_map(|layer| match &layer.kind {
                    LayerKind::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("{{set.multiday.county.note}}"));
            sample(&mut doc);
            assert!(doc
                .data_overrides
                .iter()
                .any(|(k, v)| k == "set.multiday.county.place"
                    && v.contains("County representative point")));
            assert!(doc
                .data_overrides
                .iter()
                .any(|(k, v)| k == "set.multiday.county.source" && v.contains("SAMPLE ONLY")));
            assert!(doc
                .data_overrides
                .iter()
                .any(|(k, v)| k == "set.multiday.county.note"
                    && v.contains("Representative point")
                    && v.contains("County conditions vary")
                    && v.contains("SAMPLE ONLY")));
            assert!(doc
                .data_overrides
                .iter()
                .all(|(k, _)| !k.starts_with("set.multiday.day")));
        }

        let mut point_doc = build("set_seven_day__portrait").unwrap();
        sample(&mut point_doc);
        assert!(point_doc
            .data_overrides
            .iter()
            .any(|(k, v)| k == "set.multiday.place" && v == "SAMPLE • Jeffersonville home point"));
    }

    #[test]
    fn point_and_county_values_are_isolated_and_missing_county_values_stay_missing() {
        let county_note = "Representative point • County conditions vary • Temperatures in °F";
        let fields = [
            ("condition", "Mostly cloudy".to_owned()),
            ("daytemp", "72°F".to_owned()),
            ("nighttemp", "51°F".to_owned()),
            ("pop", "20%".to_owned()),
            ("period", "Today • Wed Nov 18".to_owned()),
            ("place", "County representative-point forecast".to_owned()),
            (
                "source",
                "Source: NOAA / NWS • County representative-point forecast".to_owned(),
            ),
            ("note", county_note.to_owned()),
        ];
        let mut b = Bindings::default();
        set_day(&mut b, 0, &fields, false);
        set_day(&mut b, 0, &fields, true);
        assert_eq!(b.get("set.multiday.day0.daytemp"), Some("72°F"));
        assert_eq!(b.get("set.multiday.county.day0.daytemp"), Some("72°F"));
        assert_eq!(b.get("set.multiday.day0.note"), Some(county_note));
        assert_eq!(b.get("set.multiday.county.day0.note"), Some(county_note));

        let missing = [
            ("condition", super::super::reference::MISSING.to_owned()),
            ("daytemp", String::new()),
            ("nighttemp", super::super::reference::MISSING.to_owned()),
            ("pop", String::new()),
            ("note", county_note.to_owned()),
        ];
        let mut unavailable = Bindings::default();
        set_day(&mut unavailable, 0, &missing, true);
        assert_eq!(
            unavailable.get("set.multiday.county.day0.status"),
            Some("Forecast not available yet")
        );
        assert_eq!(unavailable.get("set.multiday.county.day0.pop"), Some(""));
        assert_eq!(
            unavailable.get("set.multiday.county.day0.icon"),
            Some("none")
        );
        assert_eq!(unavailable.get("set.multiday.day0.status"), None);
        assert!(!unavailable
            .values()
            .values()
            .any(|v| v == "0%" || v == "0°F"));
    }

    #[test]
    fn missing_values_are_withheld_and_never_replaced_with_zeroes() {
        let fields = [
            ("condition", super::super::reference::MISSING.to_owned()),
            ("daytemp", String::new()),
            ("nighttemp", super::super::reference::MISSING.to_owned()),
            ("pop", String::new()),
        ];
        let mut b = Bindings::default();
        set_day(&mut b, 0, &fields, false);
        assert_eq!(
            b.get("set.multiday.day0.status"),
            Some("Forecast not available yet")
        );
        assert_eq!(b.get("set.multiday.day0.pop"), Some(""));
        assert_eq!(b.get("set.multiday.day0.icon"), Some("none"));
        assert!(!b.values().values().any(|v| v == "0%" || v == "0°F"));
    }

    #[test]
    fn established_icon_mapping_suppresses_clear_sun_at_night_and_missing() {
        assert_eq!(
            super::super::weather_word::icon_for(super::super::reference::MISSING, false),
            "none"
        );
        assert_eq!(super::super::weather_word::icon_for("Clear", true), "none");
        assert_eq!(
            super::super::weather_word::icon_for("Mostly cloudy", true),
            "cloud"
        );
    }

    #[test]
    fn sample_has_explicit_synthetic_period_dates_and_remaining_night_case() {
        let mut doc = build("set_seven_day__portrait").unwrap();
        sample(&mut doc);
        assert!(doc
            .data_overrides
            .iter()
            .any(|(k, v)| k == "set.multiday.source" && v.contains("SAMPLE ONLY")));
        assert!(doc
            .data_overrides
            .iter()
            .any(|(k, v)| k == "set.multiday.day0.daylabel" && v == "DAYTIME PERIOD ENDED"));
        assert!(doc
            .data_overrides
            .iter()
            .any(|(k, v)| k == "set.multiday.day6.period"
                && v.contains("SAMPLE")
                && v.contains("2026")));
        assert!(doc
            .data_overrides
            .iter()
            .any(|(k, v)| k == "set.multiday.day0.icon" && v == "none"));
        assert!(doc
            .data_overrides
            .iter()
            .any(|(k, v)| k == "set.multiday.day1.icon" && v == "rain"));
        assert!(doc.data_overrides.iter().all(|(k, v)| {
            (k.ends_with(".icon")
                && [
                    "none", "suncloud", "cloud", "rain", "snow", "thunder", "hail", "wind",
                ]
                .contains(&v.as_str()))
                || v.is_empty()
                || v.contains("SAMPLE")
                || v == "DAYTIME PERIOD ENDED"
                || v == "DAYTIME"
                || v == "NIGHTTIME"
                || v == "Daytime precip. chance"
                || v == "Nighttime precip. chance"
                || v == "SAMPLE ONLY"
        }));
    }
}
