//! Producer-authored story cards. Copy is manual and defaults to visibly unentered.
use super::reference::{frame, social_frame};
use crate::config::Settings;
use crate::gfx::{
    bind::Bindings,
    model::{Fill, GraphicDoc, Layer, LayerKind},
    text::{Align, Weight},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Breaking,
    QuoteTip,
    Quiz,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Full,
    Portrait,
    Square,
}

#[derive(Clone, Copy, Debug)]
struct Spec {
    key: &'static str,
    kind: Kind,
    format: Format,
}

const SPECS: &[Spec] = &[
    Spec {
        key: "set_breaking_card__portrait",
        kind: Kind::Breaking,
        format: Format::Portrait,
    },
    Spec {
        key: "set_breaking_card__square",
        kind: Kind::Breaking,
        format: Format::Square,
    },
    Spec {
        key: "set_quote_tip_card__full",
        kind: Kind::QuoteTip,
        format: Format::Full,
    },
    Spec {
        key: "set_quote_tip_card__portrait",
        kind: Kind::QuoteTip,
        format: Format::Portrait,
    },
    Spec {
        key: "set_quote_tip_card__square",
        kind: Kind::QuoteTip,
        format: Format::Square,
    },
    Spec {
        key: "set_weather_quiz__portrait",
        kind: Kind::Quiz,
        format: Format::Portrait,
    },
    Spec {
        key: "set_weather_quiz__square",
        kind: Kind::Quiz,
        format: Format::Square,
    },
];

const QUIZ_QUESTION: &str = "Question";
const QUIZ_ANSWER: &str = "Answer";

fn prefix(kind: Kind) -> &'static str {
    match kind {
        Kind::Breaking => "set.breakingcard",
        Kind::QuoteTip => "set.quotetip",
        Kind::Quiz => "set.weatherquiz",
    }
}

fn token(kind: Kind, field: &str) -> String {
    format!("{{{{{}}}}}", binding_key(kind, field))
}

fn binding_key(kind: Kind, field: &str) -> String {
    format!("{}.{field}", prefix(kind))
}

fn panel_rect(format: Format) -> (f32, f32, f32, f32) {
    match format {
        Format::Full => (78., 218., 1764., 700.),
        Format::Portrait => (54., 278., 972., 1360.),
        Format::Square => (54., 214., 972., 730.),
    }
}

fn panel(doc: &mut GraphicDoc, format: Format, name: &str) {
    let (x, y, w, h) = panel_rect(format);
    doc.layers
        .push(Layer::rect(name, x, y, w, h, Fill::solid("$panel"), 18.));
    doc.layers.push(Layer::rect(
        &format!("{name} accent"),
        x,
        y,
        9.,
        h,
        Fill::solid("$primary"),
        0.,
    ));
}

fn fit_text(
    doc: &mut GraphicDoc,
    name: &str,
    rect: (f32, f32, f32, f32),
    value: &str,
    size: f32,
    color: &str,
    weight: Weight,
    states: &[&str],
) {
    let mut layer = Layer::text(
        name,
        rect.0,
        rect.1,
        rect.2,
        rect.3,
        value,
        "Inter",
        size,
        weight,
        color,
        Align::Left,
    );
    if let LayerKind::Text { auto_fit, wrap, .. } = &mut layer.kind {
        *auto_fit = true;
        *wrap = true;
    }
    layer.states = states.iter().map(|state| (*state).to_owned()).collect();
    doc.layers.push(layer);
}

fn layout_breaking(doc: &mut GraphicDoc, spec: Spec) {
    panel(doc, spec.format, "Breaking update card");
    let (x, y, w, _) = panel_rect(spec.format);
    let pad = if spec.format == Format::Full {
        58.
    } else {
        46.
    };
    fit_text(
        doc,
        "Breaking update label",
        (x + pad, y + 38., w - pad * 2., 54.),
        "BREAKING UPDATE",
        if spec.format == Format::Square {
            25.
        } else {
            28.
        },
        "$primary",
        Weight::Bold,
        &[],
    );
    fit_text(
        doc,
        "Breaking headline",
        (x + pad, y + 104., w - pad * 2., 208.),
        &token(Kind::Breaking, "headline"),
        if spec.format == Format::Square {
            58.
        } else {
            66.
        },
        "$text",
        Weight::Bold,
        &[],
    );
    fit_text(
        doc,
        "Breaking details label",
        (x + pad, y + 338., w - pad * 2., 44.),
        "DETAILS",
        25.,
        "$muted",
        Weight::Bold,
        &[],
    );
    fit_text(
        doc,
        "Breaking details",
        (x + pad, y + 386., w - pad * 2., 150.),
        &token(Kind::Breaking, "detail"),
        if spec.format == Format::Square {
            31.
        } else {
            36.
        },
        "$text",
        Weight::Regular,
        &[],
    );
    fit_text(
        doc,
        "Breaking period and location",
        (x + pad, y + 570., w - pad * 2., 74.),
        &format!(
            "{{{{{}.period}}}} • {{{{{}.location}}}}",
            prefix(Kind::Breaking),
            prefix(Kind::Breaking)
        ),
        26.,
        "$muted",
        Weight::SemiBold,
        &[],
    );
}

fn layout_quote(doc: &mut GraphicDoc, spec: Spec) {
    panel(doc, spec.format, "Quote and tip card");
    let (x, y, w, _) = panel_rect(spec.format);
    let pad = if spec.format == Format::Full {
        60.
    } else {
        46.
    };
    fit_text(
        doc,
        "Quote or tip label",
        (x + pad, y + 40., w - pad * 2., 52.),
        "QUOTE / TIP",
        27.,
        "$primary",
        Weight::Bold,
        &[],
    );
    fit_text(
        doc,
        "Quote or tip copy",
        (
            x + pad,
            y + 118.,
            w - pad * 2.,
            if spec.format == Format::Full {
                300.
            } else {
                460.
            },
        ),
        &token(Kind::QuoteTip, "copy"),
        match spec.format {
            Format::Full => 68.,
            Format::Portrait => 61.,
            Format::Square => 54.,
        },
        "$text",
        Weight::Bold,
        &[],
    );
    let (attribution_y, context_y, context_h) = match spec.format {
        Format::Full => (y + 448., y + 548., 90.),
        Format::Portrait => (y + 640., y + 740., 90.),
        Format::Square => (y + 500., y + 600., 100.),
    };
    fit_text(
        doc,
        "Quote or tip attribution",
        (x + pad, attribution_y, w - pad * 2., 82.),
        &token(Kind::QuoteTip, "attribution"),
        31.,
        "$primary",
        Weight::SemiBold,
        &[],
    );
    fit_text(
        doc,
        "Quote or tip context",
        (x + pad, context_y, w - pad * 2., context_h),
        &token(Kind::QuoteTip, "context"),
        25.,
        "$muted",
        Weight::Regular,
        &[],
    );
}

fn layout_quiz(doc: &mut GraphicDoc, spec: Spec) {
    panel(doc, spec.format, "Weather quiz card");
    let (x, y, w, _) = panel_rect(spec.format);
    let pad = 46.;
    let question_states = [QUIZ_QUESTION, QUIZ_ANSWER];
    let (question_label_y, question_y, question_h, answer_label_y, answer_y, answer_h) =
        if spec.format == Format::Square {
            (y + 34., y + 98., 224., y + 366., y + 430., 210.)
        } else {
            (y + 40., y + 112., 345., y + 522., y + 596., 240.)
        };
    fit_text(
        doc,
        "Quiz question label",
        (x + pad, question_label_y, w - pad * 2., 54.),
        "QUESTION",
        27.,
        "$primary",
        Weight::Bold,
        &question_states,
    );
    fit_text(
        doc,
        "Quiz question",
        (x + pad, question_y, w - pad * 2., question_h),
        &token(Kind::Quiz, "question"),
        if spec.format == Format::Square {
            49.
        } else {
            64.
        },
        "$text",
        Weight::Bold,
        &question_states,
    );
    fit_text(
        doc,
        "Quiz answer label",
        (x + pad, answer_label_y, w - pad * 2., 56.),
        "ANSWER",
        27.,
        "$primary",
        Weight::Bold,
        &[QUIZ_ANSWER],
    );
    fit_text(
        doc,
        "Quiz answer",
        (x + pad, answer_y, w - pad * 2., answer_h),
        &token(Kind::Quiz, "answer"),
        if spec.format == Format::Square {
            40.
        } else {
            50.
        },
        "$text",
        Weight::SemiBold,
        &[QUIZ_ANSWER],
    );
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = *SPECS.iter().find(|spec| spec.key == key)?;
    let (title, subtitle) = match spec.kind {
        Kind::Breaking => (
            "Breaking Update",
            "Producer-authored update • Confirm details before air",
        ),
        Kind::QuoteTip => ("Quote / Tip", "Producer-entered quote or tip"),
        Kind::Quiz => ("Weather Quiz", "Producer-entered question"),
    };
    let source = "{{set.storycards.source}}";
    let category = "Station production";
    let mut doc = match spec.format {
        Format::Full => frame(title, category, subtitle, source),
        Format::Portrait => social_frame(title, category, subtitle, source, true),
        Format::Square => social_frame(title, category, subtitle, source, false),
    };
    doc.template = Some(key.into());
    match spec.kind {
        Kind::Breaking => layout_breaking(&mut doc, spec),
        Kind::QuoteTip => layout_quote(&mut doc, spec),
        Kind::Quiz => {
            doc.states = vec![QUIZ_QUESTION.into(), QUIZ_ANSWER.into()];
            layout_quiz(&mut doc, spec);
        }
    }
    Some(doc)
}

pub(super) fn bind(bindings: &mut Bindings, _settings: &Settings) {
    for (key, value) in [
        ("set.breakingcard.headline", "Update copy not entered yet"),
        (
            "set.breakingcard.detail",
            "Producer-entered details will appear here after review.",
        ),
        ("set.breakingcard.period", "Update time not entered"),
        ("set.breakingcard.location", "Location not entered"),
        ("set.quotetip.copy", "Quote or tip copy not entered yet"),
        ("set.quotetip.attribution", "Attribution not entered"),
        ("set.quotetip.context", "Source or context not entered"),
        ("set.weatherquiz.question", "Question not entered yet"),
        ("set.weatherquiz.answer", "Answer not entered yet"),
        (
            "set.storycards.source",
            "Station production • Producer-entered copy",
        ),
    ] {
        bindings.set(key, value);
    }
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(spec) = doc
        .template
        .as_deref()
        .and_then(|key| SPECS.iter().find(|spec| spec.key == key))
    else {
        return;
    };
    let fields: &[(&str, &str)] = match spec.kind {
        Kind::Breaking => &[
            (
                "headline",
                "SAMPLE • Producer update headline with a longer phrase to demonstrate wrapping on narrow and square layouts",
            ),
            (
                "detail",
                "SAMPLE ONLY • Replace this long example with reviewed producer copy. This fixture makes no claim about current conditions and is not an active warning.",
            ),
            ("period", "SAMPLE • Update time not entered"),
            ("location", "SAMPLE • Location not entered"),
        ],
        Kind::QuoteTip => &[
            (
                "copy",
                "SAMPLE • Enter approved quote or tip copy here. This longer synthetic line checks that a multi-line producer message remains inside the card at native portrait and square sizes.",
            ),
            (
                "attribution",
                "SAMPLE • Producer attribution line for a longer named source",
            ),
            (
                "context",
                "SAMPLE • Producer-entered context or source details go here; verify exact wording before use.",
            ),
        ],
        Kind::Quiz => &[
            (
                "question",
                "SAMPLE • Which producer-entered detail should viewers look for in this longer example scenario?",
            ),
            (
                "answer",
                "SAMPLE ONLY • Replace this sample response with a reviewed producer-entered answer before use.",
            ),
        ],
    };
    for (field, value) in fields {
        doc.data_overrides
            .push((binding_key(spec.kind, field), (*value).to_owned()));
    }
    doc.data_overrides.push((
        "set.storycards.source".into(),
        "SAMPLE ONLY • Synthetic producer-authored layout fixture • Not for broadcast".into(),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx::model::LayerKind;
    use std::collections::HashSet;

    fn text_layers(doc: &GraphicDoc) -> Vec<&Layer> {
        doc.layers
            .iter()
            .filter(|layer| layer.visible && matches!(&layer.kind, LayerKind::Text { .. }))
            .collect()
    }

    #[test]
    fn seven_specs_are_unique_native_layouts_with_fitted_text() {
        let mut keys = HashSet::new();
        assert_eq!(SPECS.len(), 7);
        for spec in SPECS {
            assert!(keys.insert(spec.key));
            let doc = build(spec.key).expect("known producer story card");
            assert_eq!(doc.template.as_deref(), Some(spec.key));
            assert_eq!(
                (doc.width, doc.height),
                match spec.format {
                    Format::Full => (1920, 1080),
                    Format::Portrait => (1080, 1920),
                    Format::Square => (1080, 1080),
                }
            );
            assert!(doc
                .layers
                .iter()
                .any(|layer| layer.name == "Header" || layer.name == "Social Header"));
            assert!(doc
                .layers
                .iter()
                .any(|layer| layer.name == "Footer" || layer.name == "Social Footer"));
            for layer in doc.layers.iter().filter(|layer| {
                matches!(
                    layer.name.as_str(),
                    "Breaking update card" | "Quote and tip card" | "Weather quiz card"
                )
            }) {
                assert!(layer.x >= 0. && layer.y >= 0.);
                assert!(layer.x + layer.w <= doc.width as f32);
                assert!(layer.y + layer.h <= doc.height as f32);
            }
            for layer in text_layers(&doc).into_iter().filter(|layer| {
                layer.name.starts_with("Breaking")
                    || layer.name.starts_with("Quote")
                    || layer.name.starts_with("Quiz")
            }) {
                let LayerKind::Text { auto_fit, wrap, .. } = &layer.kind else {
                    unreachable!();
                };
                assert!(*auto_fit && *wrap, "{} should fit and wrap", layer.name);
                assert!(layer.x >= 0. && layer.y >= 0.);
                assert!(layer.x + layer.w <= doc.width as f32);
                assert!(layer.y + layer.h <= doc.height as f32);
            }
        }
    }

    #[test]
    fn manual_defaults_are_calm_and_do_not_claim_weather_or_safety_facts() {
        let mut bindings = Bindings::default();
        bind(&mut bindings, &Settings::default());
        for (key, expected) in [
            (
                "{{set.breakingcard.headline}}",
                "Update copy not entered yet",
            ),
            (
                "{{set.breakingcard.detail}}",
                "Producer-entered details will appear here after review.",
            ),
            ("{{set.quotetip.copy}}", "Quote or tip copy not entered yet"),
            ("{{set.weatherquiz.question}}", "Question not entered yet"),
            ("{{set.weatherquiz.answer}}", "Answer not entered yet"),
        ] {
            assert_eq!(bindings.apply(key), expected);
        }
        let source = bindings.apply("{{set.storycards.source}}");
        assert!(source.contains("Station production"));
        assert!(source.contains("Producer-entered copy"));
        assert!(!source.contains("NWS"));
    }

    #[test]
    fn quiz_answer_is_isolated_until_the_answer_state_and_keeps_question_context() {
        for key in ["set_weather_quiz__portrait", "set_weather_quiz__square"] {
            let doc = build(key).unwrap();
            assert_eq!(
                doc.states,
                vec![QUIZ_QUESTION.to_string(), QUIZ_ANSWER.to_string()]
            );
            let question = doc.in_state(QUIZ_QUESTION, &[]);
            let answer = doc.in_state(QUIZ_ANSWER, &[]);
            let question_layers = text_layers(&question);
            let answer_layers = text_layers(&answer);
            assert!(question_layers
                .iter()
                .any(|layer| layer.name == "Quiz question"));
            assert!(!question_layers
                .iter()
                .any(|layer| layer.name == "Quiz answer"));
            assert!(answer_layers
                .iter()
                .any(|layer| layer.name == "Quiz question"));
            assert!(answer_layers
                .iter()
                .any(|layer| layer.name == "Quiz answer"));
            for state_doc in [&question, &answer] {
                for chrome in ["Social Header", "Social Footer"] {
                    assert!(state_doc.layers.iter().any(|layer| layer.name == chrome));
                }
            }
        }
    }

    #[test]
    fn synthetic_samples_are_explicit_and_cover_each_manual_kind() {
        for key in SPECS.iter().map(|spec| spec.key) {
            let mut doc = build(key).unwrap();
            sample(&mut doc);
            assert!(doc
                .data_overrides
                .iter()
                .all(|(_, value)| value.contains("SAMPLE")));
            assert!(doc.data_overrides.iter().any(|(key, value)| {
                key == "set.storycards.source" && value.contains("Not for broadcast")
            }));
        }
        let quiz = SPECS.iter().find(|spec| spec.kind == Kind::Quiz).unwrap();
        let mut quiz_doc = build(quiz.key).unwrap();
        sample(&mut quiz_doc);
        assert!(quiz_doc.data_overrides.iter().any(|(key, value)| {
            key == "set.weatherquiz.question" && value.starts_with("SAMPLE •")
        }));
        assert!(quiz_doc.data_overrides.iter().any(|(key, value)| {
            key == "set.weatherquiz.answer" && value.starts_with("SAMPLE ONLY •")
        }));
    }
}
