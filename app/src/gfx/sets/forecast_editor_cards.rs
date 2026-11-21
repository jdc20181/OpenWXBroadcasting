//! Producer-entered daily forecast cards. These templates do not invent weather data.
use super::reference::{frame, label, lowerthird_frame, social_frame};
use crate::config::Settings;
use crate::gfx::{
    bind::Bindings,
    model::{AnimKind, Fill, GraphicDoc, Layer, LayerKind},
    text::Align,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Landscape,
    Portrait,
    Square,
    LowerThird,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    Point,
    County,
}

#[derive(Clone, Copy, Debug)]
struct Spec {
    key: &'static str,
    format: Format,
    scope: Scope,
}

const SPECS: &[Spec] = &[
    Spec {
        key: "set_forecast_editor__county",
        format: Format::Landscape,
        scope: Scope::County,
    },
    Spec {
        key: "set_forecast_editor__portrait",
        format: Format::Portrait,
        scope: Scope::Point,
    },
    Spec {
        key: "set_forecast_editor__county_portrait",
        format: Format::Portrait,
        scope: Scope::County,
    },
    Spec {
        key: "set_forecast_editor__square",
        format: Format::Square,
        scope: Scope::Point,
    },
    Spec {
        key: "set_forecast_editor__county_square",
        format: Format::Square,
        scope: Scope::County,
    },
    Spec {
        key: "set_forecast_editor__county_lowerthird",
        format: Format::LowerThird,
        scope: Scope::County,
    },
];

impl Scope {
    fn field_name(self) -> &'static str {
        match self {
            Self::Point => "point",
            Self::County => "county",
        }
    }
}

fn token(scope: Scope, field: &str) -> String {
    format!("{{{{set.forecasteditor.{}.{field}}}}}", scope.field_name())
}

fn fitted_label(
    doc: &mut GraphicDoc,
    name: &str,
    rect: (f32, f32, f32, f32),
    value: &str,
    size: f32,
    color: &str,
    align: Align,
) {
    label(doc, name, rect, value, size, color, align);
    if let Some(Layer {
        kind: LayerKind::Text { auto_fit, wrap, .. },
        ..
    }) = doc.layers.last_mut()
    {
        *auto_fit = true;
        *wrap = true;
    }
}

fn card(doc: &mut GraphicDoc, name: &str, rect: (f32, f32, f32, f32)) {
    doc.layers.push(
        Layer::rect(
            name,
            rect.0,
            rect.1,
            rect.2,
            rect.3,
            Fill::solid("$panel"),
            16.,
        )
        .with_anim(AnimKind::Rise, 0.14, 0.44),
    );
    doc.layers.push(
        Layer::rect(
            &format!("{name} accent"),
            rect.0,
            rect.1,
            8.,
            rect.3,
            Fill::solid("$primary"),
            0.,
        )
        .with_anim(AnimKind::Grow, 0.22, 0.4),
    );
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = *SPECS.iter().find(|s| s.key == key)?;
    let title = "Local Forecast";
    let subtitle = token(spec.scope, "period");
    let source = token(spec.scope, "source");
    let category = "Local forecast • Manual copy";
    let mut doc = match spec.format {
        Format::Portrait => social_frame(title, category, &subtitle, &source, true),
        Format::Square => social_frame(title, category, &subtitle, &source, false),
        Format::LowerThird => lowerthird_frame(title, category, &subtitle, &source),
        Format::Landscape => frame(title, category, &subtitle, &source),
    };
    doc.template = Some(key.to_string());
    let location_label = match spec.scope {
        Scope::Point => "Home point / area",
        Scope::County => "Home county",
    };

    match spec.format {
        Format::Landscape => {
            card(&mut doc, "Manual forecast card", (78., 218., 1764., 706.));
            fitted_label(
                &mut doc,
                "Forecast context",
                (132., 254., 1640., 58.),
                location_label,
                32.,
                "$primary",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast location",
                (132., 314., 1640., 68.),
                &token(spec.scope, "location"),
                40.,
                "$text",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast headline label",
                (132., 412., 1640., 44.),
                "FORECAST OUTLOOK",
                27.,
                "$muted",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast headline",
                (132., 458., 1640., 130.),
                &token(spec.scope, "headline"),
                61.,
                "$text",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast detail label",
                (132., 622., 1640., 42.),
                "FORECAST DETAIL",
                26.,
                "$muted",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast detail",
                (132., 666., 1640., 178.),
                &token(spec.scope, "detail"),
                36.,
                "$text",
                Align::Left,
            );
        }
        Format::Portrait => {
            card(&mut doc, "Manual forecast card", (54., 278., 972., 1360.));
            fitted_label(
                &mut doc,
                "Forecast context",
                (102., 324., 876., 70.),
                location_label,
                33.,
                "$primary",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast location",
                (102., 402., 876., 108.),
                &token(spec.scope, "location"),
                43.,
                "$text",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast headline label",
                (102., 560., 876., 55.),
                "FORECAST OUTLOOK",
                28.,
                "$muted",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast headline",
                (102., 620., 876., 252.),
                &token(spec.scope, "headline"),
                54.,
                "$text",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast detail label",
                (102., 938., 876., 50.),
                "FORECAST DETAIL",
                27.,
                "$muted",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast detail",
                (102., 994., 876., 322.),
                &token(spec.scope, "detail"),
                36.,
                "$text",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast period",
                (102., 1446., 876., 106.),
                &token(spec.scope, "period"),
                34.,
                "$primary",
                Align::Left,
            );
        }
        Format::Square => {
            card(&mut doc, "Manual forecast card", (54., 214., 972., 704.));
            fitted_label(
                &mut doc,
                "Forecast context",
                (98., 244., 884., 42.),
                location_label,
                23.,
                "$primary",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast location",
                (98., 286., 884., 58.),
                &token(spec.scope, "location"),
                28.,
                "$text",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast headline label",
                (98., 366., 884., 36.),
                "FORECAST OUTLOOK",
                20.,
                "$muted",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast headline",
                (98., 402., 884., 112.),
                &token(spec.scope, "headline"),
                38.,
                "$text",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast detail label",
                (98., 538., 884., 34.),
                "FORECAST DETAIL",
                19.,
                "$muted",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast detail",
                (98., 572., 884., 172.),
                &token(spec.scope, "detail"),
                26.,
                "$text",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast period",
                (98., 776., 884., 94.),
                &token(spec.scope, "period"),
                24.,
                "$primary",
                Align::Left,
            );
        }
        Format::LowerThird => {
            // Transparent broadcast band: the surrounding live picture remains visible.
            card(
                &mut doc,
                "Manual forecast lower-third band",
                (60., 866., 1800., 126.),
            );
            fitted_label(
                &mut doc,
                "Forecast context",
                (96., 870., 465., 38.),
                location_label,
                24.,
                "$primary",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast location",
                (568., 870., 570., 38.),
                &token(spec.scope, "location"),
                24.,
                "$text",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast period",
                (1150., 870., 660., 38.),
                &token(spec.scope, "period"),
                24.,
                "$muted",
                Align::Right,
            );
            fitted_label(
                &mut doc,
                "Forecast headline",
                (96., 910., 850., 68.),
                &token(spec.scope, "headline"),
                33.,
                "$text",
                Align::Left,
            );
            fitted_label(
                &mut doc,
                "Forecast detail",
                (970., 910., 830., 68.),
                &token(spec.scope, "detail"),
                25.,
                "$text",
                Align::Left,
            );
        }
    }
    for layer in &mut doc.layers {
        if let LayerKind::Text {
            text,
            auto_fit,
            wrap,
            ..
        } = &mut layer.kind
        {
            if text.contains("{{set.forecasteditor.") {
                *auto_fit = true;
                *wrap = true;
            }
        }
    }
    Some(doc)
}

pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let home = if settings.home_label.trim().is_empty() {
        "Home area not entered"
    } else {
        settings.home_label.as_str()
    };
    for (scope, location) in [(Scope::Point, home), (Scope::County, "County not entered")] {
        for (field, value) in [
            ("period", "Forecast period not entered"),
            ("location", location),
            ("headline", "Forecast headline not entered yet"),
            (
                "detail",
                "Producer-entered forecast details will appear here after review.",
            ),
            ("source", "Manual forecast editor • Producer-entered copy"),
        ] {
            b.set(
                &format!("set.forecasteditor.{}.{field}", scope.field_name()),
                value,
            );
        }
    }
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(spec) = doc
        .template
        .as_deref()
        .and_then(|key| SPECS.iter().find(|s| s.key == key))
    else {
        return;
    };
    let location = match spec.scope {
        Scope::Point => "SAMPLE • Jeffersonville home point",
        Scope::County => "SAMPLE • Clark County, Indiana",
    };
    for (field, value) in [
        ("period", "SAMPLE • Monday daytime forecast"),
        ("location", location),
        ("headline", "SAMPLE • A brighter afternoon for Jeffersonville"),
        ("detail", "SAMPLE ONLY • Synthetic producer copy for layout review. Confirm every detail before broadcast."),
        ("source", "SAMPLE ONLY • Synthetic manual producer copy • Not for broadcast"),
    ] {
        doc.data_overrides.push((
            format!("set.forecasteditor.{}.{field}", spec.scope.field_name()),
            value.into(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn text_values(doc: &GraphicDoc) -> impl Iterator<Item = (&str, &str)> {
        doc.layers.iter().filter_map(|layer| match &layer.kind {
            LayerKind::Text {
                text,
                auto_fit,
                wrap,
                ..
            } if *auto_fit && *wrap => Some((layer.name.as_str(), text.as_str())),
            _ => None,
        })
    }

    fn all_text_values(doc: &GraphicDoc) -> impl Iterator<Item = &str> {
        doc.layers.iter().filter_map(|layer| match &layer.kind {
            LayerKind::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
    }

    #[test]
    fn all_six_rows_have_bounded_native_layouts_and_manual_copy_fields() {
        let mut seen = HashSet::new();
        for spec in SPECS {
            assert!(seen.insert(spec.key));
            let doc = build(spec.key).unwrap();
            let expected = match spec.format {
                Format::Landscape | Format::LowerThird => (1920, 1080),
                Format::Portrait => (1080, 1920),
                Format::Square => (1080, 1080),
            };
            assert_eq!((doc.width, doc.height), expected, "{}", spec.key);
            assert_eq!(doc.template.as_deref(), Some(spec.key));
            assert!(
                doc.layers.iter().all(|layer| layer.x >= 0.
                    && layer.y >= 0.
                    && layer.x + layer.w <= doc.width as f32
                    && layer.y + layer.h <= doc.height as f32),
                "out of bounds in {}",
                spec.key
            );
            for field in ["period", "location", "headline", "detail"] {
                let expected = token(spec.scope, field);
                assert!(
                    text_values(&doc).any(|(_, value)| value == expected.as_str()),
                    "{} binds {field}",
                    spec.key
                );
            }
            let source_token = token(spec.scope, "source");
            assert!(
                all_text_values(&doc).any(|value| value == source_token.as_str()),
                "{} carries manual producer source",
                spec.key
            );
        }
        assert_eq!(seen.len(), 6);
    }

    #[test]
    fn unavailable_defaults_do_not_invent_a_forecast_or_nws_credit() {
        let settings = Settings::default();
        let mut bindings = Bindings::default();
        bind(&mut bindings, &settings);
        assert_eq!(
            bindings.apply("{{set.forecasteditor.point.headline}}"),
            "Forecast headline not entered yet"
        );
        assert!(bindings
            .apply("{{set.forecasteditor.point.detail}}")
            .contains("after review"));
        assert!(bindings
            .apply("{{set.forecasteditor.point.source}}")
            .contains("Manual forecast editor"));
        assert!(!bindings
            .apply("{{set.forecasteditor.point.source}}")
            .contains("NWS"));
        assert_eq!(
            bindings.apply("{{set.forecasteditor.county.location}}"),
            "County not entered"
        );
        assert_eq!(
            bindings.apply("{{set.forecasteditor.point.location}}"),
            settings.home_label
        );
        assert_ne!(
            bindings.apply("{{set.forecasteditor.point.location}}"),
            bindings.apply("{{set.forecasteditor.county.location}}")
        );
    }

    #[test]
    fn synthetic_sample_is_explicit_and_handles_long_location_copy() {
        let mut doc = build("set_forecast_editor__county_portrait").unwrap();
        sample(&mut doc);
        assert!(doc
            .data_overrides
            .iter()
            .any(|(key, value)| key == "set.forecasteditor.county.location"
                && value.contains("Clark County")));
        assert!(doc
            .data_overrides
            .iter()
            .all(|(_, value)| value.contains("SAMPLE")));
        assert!(doc
            .layers
            .iter()
            .any(|layer| layer.name == "Forecast headline"));
    }

    #[test]
    fn county_lowerthird_preserves_transparency_and_contains_the_brand_frame() {
        let doc = build("set_forecast_editor__county_lowerthird").unwrap();
        assert!(doc.background.is_none());
        assert!(doc
            .layers
            .iter()
            .any(|layer| layer.name == "Lower third Footer"));
        assert!(doc
            .layers
            .iter()
            .any(|layer| layer.name == "Manual forecast lower-third band"));
    }
}
