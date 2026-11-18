//! Producer-editable show packaging. Manual values stay clearly unconfigured until entered.
use super::reference::{frame, label, social_frame};
use crate::gfx::{
    bind::Bindings,
    model::{AnimKind, Fill, GraphicDoc, Layer, LayerKind},
    text::Align,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    EndCard,
    Schedule,
    Credits,
}

#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    kind: Kind,
    format: u8, // 0 landscape, 1 portrait, 2 square
}

const SPECS: &[Spec] = &[
    Spec {
        key: "set_show_end_card",
        kind: Kind::EndCard,
        format: 0,
    },
    Spec {
        key: "set_show_weekly_schedule",
        kind: Kind::Schedule,
        format: 0,
    },
    Spec {
        key: "set_show_weekly_schedule__portrait",
        kind: Kind::Schedule,
        format: 1,
    },
    Spec {
        key: "set_show_weekly_schedule__square",
        kind: Kind::Schedule,
        format: 2,
    },
    Spec {
        key: "set_show_data_credits",
        kind: Kind::Credits,
        format: 0,
    },
    Spec {
        key: "set_show_data_credits__portrait",
        kind: Kind::Credits,
        format: 1,
    },
    Spec {
        key: "set_show_data_credits__square",
        kind: Kind::Credits,
        format: 2,
    },
];

fn token(field: &str) -> String {
    format!("{{{{set.packaging.{field}}}}}")
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

fn card(doc: &mut GraphicDoc, name: &str, x: f32, y: f32, w: f32, h: f32) {
    doc.layers.push(
        Layer::rect(name, x, y, w, h, Fill::solid("$panel"), 18.).with_anim(
            AnimKind::Rise,
            0.12,
            0.45,
        ),
    );
    doc.layers.push(
        Layer::rect(
            &format!("{name} accent"),
            x,
            y,
            7.,
            h,
            Fill::solid("$primary"),
            0.,
        )
        .with_anim(AnimKind::Grow, 0.22, 0.4),
    );
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = *SPECS.iter().find(|s| s.key == key)?;
    let title = match spec.kind {
        Kind::EndCard => "Thanks for watching",
        Kind::Schedule => "Weekly show schedule",
        Kind::Credits => "Data sources & credits",
    };
    let header_right = match spec.kind {
        Kind::EndCard => "Your local weather".to_string(),
        Kind::Schedule => token("schedule.status"),
        Kind::Credits => token("credits.status"),
    };
    let category = "Station, branding and show";
    let source = if spec.kind == Kind::EndCard {
        "Station production"
    } else {
        "Producer-editable • Confirm before broadcast"
    };
    let mut doc = match spec.format {
        1 => social_frame(title, category, &header_right, source, true),
        2 => social_frame(title, category, &header_right, source, false),
        _ => frame(title, category, &header_right, source),
    };
    doc.template = Some(key.to_string());
    match spec.kind {
        Kind::EndCard => end_card(&mut doc),
        Kind::Schedule => schedule(&mut doc, spec.format),
        Kind::Credits => credits(&mut doc, spec.format),
    }
    for layer in &mut doc.layers {
        if let LayerKind::Text { text, auto_fit, wrap, .. } = &mut layer.kind {
            if text.contains("{{set.packaging.") {
                *auto_fit = true;
                *wrap = true;
            }
        }
    }
    Some(doc)
}

fn end_card(doc: &mut GraphicDoc) {
    fitted_label(
        doc,
        "End card station wordmark",
        (80., 300., 850., 150.),
        "{{station}}",
        88.,
        "$text",
        Align::Left,
    );
    fitted_label(
        doc,
        "End card tagline",
        (84., 462., 820., 110.),
        &token("endcard.tagline"),
        38.,
        "$muted",
        Align::Left,
    );
    fitted_label(
        doc,
        "End card next update note",
        (84., 625., 820., 130.),
        &token("endcard.note"),
        30.,
        "$primary",
        Align::Left,
    );

    // These are empty reserved zones for YouTube's own end-screen elements, not fake controls.
    card(doc, "Next video placement zone", 1060., 252., 760., 504.);
    fitted_label(
        doc,
        "Next video zone label",
        (1090., 274., 700., 60.),
        "NEXT VIDEO • 16:9 PLACEMENT AREA",
        30.,
        "$text",
        Align::Center,
    );
    doc.layers.push(
        Layer::rect(
            "Next video empty surface",
            1110.,
            350.,
            660.,
            371.25,
            Fill::solid("$secondary"),
            12.,
        )
        .with_anim(AnimKind::Fade, 0.2, 0.4),
    );
    fitted_label(
        doc,
        "Next video empty surface note",
        (1140., 482., 600., 82.),
        "Leave clear for the platform end-screen video element",
        27.,
        "$muted",
        Align::Center,
    );

    card(doc, "Subscribe placement zone", 1190., 760., 500., 200.);
    fitted_label(
        doc,
        "Subscribe zone label",
        (1220., 772., 440., 44.),
        "SUBSCRIBE ELEMENT AREA",
        25.,
        "$text",
        Align::Center,
    );
    doc.layers.push(Layer::new(
        "Subscribe empty circle",
        1376.,
        824.,
        128.,
        128.,
        LayerKind::Ellipse {
            fill: Fill::solid("$secondary"),
            stroke: None,
        },
    ));
}

fn schedule(doc: &mut GraphicDoc, format: u8) {
    let days = [
        "MONDAY",
        "TUESDAY",
        "WEDNESDAY",
        "THURSDAY",
        "FRIDAY",
        "SATURDAY",
        "SUNDAY",
    ];
    let (x0, y0, w, h, gap_x, gap_y, cols, day_size, title_size, time_size) = match format {
        1 => (54., 248., 972., 190., 0., 20., 1, 26., 32., 25.),
        2 => (54., 188., 476., 174., 20., 14., 2, 22., 24., 21.),
        _ => (80., 194., 850., 176., 20., 13., 2, 24., 29., 23.),
    };
    for (i, day) in days.iter().enumerate() {
        let row = i / cols;
        let col = i % cols;
        let x = x0 + col as f32 * (w + gap_x);
        let y = y0 + row as f32 * (h + gap_y);
        card(doc, &format!("Schedule day {i} card"), x, y, w, h);
        fitted_label(
            doc,
            &format!("Schedule day {i} label"),
            (x + 25., y + 12., w - 50., 36.),
            day,
            day_size,
            "$primary",
            Align::Left,
        );
        fitted_label(
            doc,
            &format!("Schedule day {i} show title"),
            (x + 25., y + 52., w - 50., 57.),
            &token(&format!("schedule.{i}.title")),
            title_size,
            "$text",
            Align::Left,
        );
        fitted_label(
            doc,
            &format!("Schedule day {i} airtime"),
            (x + 25., y + 119., w - 50., 40.),
            &token(&format!("schedule.{i}.time")),
            time_size,
            "$muted",
            Align::Left,
        );
    }
}

fn credits(doc: &mut GraphicDoc, format: u8) {
    let (x0, y0, w, h, gap_x, gap_y, cols, name_size, detail_size) = match format {
        1 => (54., 260., 972., 325., 0., 24., 1, 35., 27.),
        2 => (54., 206., 476., 325., 20., 24., 2, 28., 23.),
        _ => (80., 202., 850., 330., 20., 22., 2, 32., 27.),
    };
    for i in 0..4 {
        let row = i / cols;
        let col = i % cols;
        let x = x0 + col as f32 * (w + gap_x);
        let y = y0 + row as f32 * (h + gap_y);
        card(doc, &format!("Credit source {i} card"), x, y, w, h);
        fitted_label(
            doc,
            &format!("Credit source {i} eyebrow"),
            (x + 25., y + 20., w - 50., 42.),
            &format!("SOURCE {}", i + 1),
            22.,
            "$primary",
            Align::Left,
        );
        fitted_label(
            doc,
            &format!("Credit source {i} agency"),
            (x + 25., y + 78., w - 50., 76.),
            &token(&format!("credits.{i}.agency")),
            name_size,
            "$text",
            Align::Left,
        );
        fitted_label(
            doc,
            &format!("Credit source {i} attribution"),
            (x + 25., y + 172., w - 50., h - 196.),
            &token(&format!("credits.{i}.detail")),
            detail_size,
            "$muted",
            Align::Left,
        );
    }
}

pub(super) fn bind(b: &mut Bindings, _settings: &crate::config::Settings) {
    b.set("set.packaging.schedule.status", "SCHEDULE NOT SET");
    b.set("set.packaging.credits.status", "CREDITS NOT ENTERED");
    b.set(
        "set.packaging.endcard.tagline",
        "Local forecasts • Community weather",
    );
    b.set(
        "set.packaging.endcard.note",
        "Add a producer message or next-update time",
    );
    for i in 0..7 {
        b.set(
            &format!("set.packaging.schedule.{i}.title"),
            "Show title not entered",
        );
        b.set(
            &format!("set.packaging.schedule.{i}.time"),
            "Airtime not entered",
        );
    }
    for i in 0..4 {
        b.set(
            &format!("set.packaging.credits.{i}.agency"),
            "Source / agency not entered",
        );
        b.set(
            &format!("set.packaging.credits.{i}.detail"),
            "Add the exact product and attribution before broadcast",
        );
    }
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(spec) = doc
        .template
        .as_deref()
        .and_then(|k| SPECS.iter().find(|s| s.key == k))
    else {
        return;
    };
    match spec.kind {
        Kind::EndCard => {
            doc.data_overrides.push((
                "set.packaging.endcard.tagline".into(),
                "SAMPLE • Your local forecasts and community weather".into(),
            ));
            doc.data_overrides.push((
                "set.packaging.endcard.note".into(),
                "SAMPLE ONLY • Add your next update message".into(),
            ));
        }
        Kind::Schedule => {
            doc.data_overrides
                .push(("set.packaging.schedule.status".into(), "SAMPLE ONLY".into()));
            let entries = [
                ("Local forecast", "SAMPLE • 7:00 AM"),
                ("Community weather update", "SAMPLE • 10:00 AM"),
                ("Forecast discussion", "SAMPLE • 1:00 PM"),
                ("Evening outlook", "SAMPLE • 5:00 PM"),
                ("Weekend planner", "SAMPLE • 6:00 PM"),
                ("Local weather recap", "SAMPLE • 8:00 AM"),
                ("Week-ahead preview", "SAMPLE • 9:00 AM"),
            ];
            for (i, (title, time)) in entries.into_iter().enumerate() {
                doc.data_overrides.push((
                    format!("set.packaging.schedule.{i}.title"),
                    format!("SAMPLE • {title}"),
                ));
                doc.data_overrides
                    .push((format!("set.packaging.schedule.{i}.time"), time.into()));
            }
        }
        Kind::Credits => {
            doc.data_overrides
                .push(("set.packaging.credits.status".into(), "SAMPLE ONLY".into()));
            for i in 0..4 {
                doc.data_overrides.push((
                    format!("set.packaging.credits.{i}.agency"),
                    format!("SAMPLE • Agency / provider {}", i + 1),
                ));
                doc.data_overrides.push((
                    format!("set.packaging.credits.{i}.detail"),
                    "SAMPLE ONLY • Example product / attribution; verify before use".into(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_packaging_variants_have_bounded_fit_text_and_manual_defaults() {
        let mut seen = std::collections::HashSet::new();
        for spec in SPECS {
            assert!(seen.insert(spec.key));
            let doc = build(spec.key).unwrap();
            assert_eq!(doc.template.as_deref(), Some(spec.key));
            assert_eq!(
                (doc.width, doc.height),
                match spec.format {
                    1 => (1080, 1920),
                    2 => (1080, 1080),
                    _ => (1920, 1080),
                }
            );
            assert!(
                doc.layers.iter().all(|l| {
                    l.x >= 0.
                        && l.y >= 0.
                        && l.x + l.w <= doc.width as f32
                        && l.y + l.h <= doc.height as f32
                }),
                "out of bounds layer in {}",
                spec.key
            );
            for layer in &doc.layers {
                if let LayerKind::Text {
                    text,
                    auto_fit,
                    wrap,
                    ..
                } = &layer.kind
                {
                    if text.contains("{{set.packaging.") {
                        assert!(
                            *auto_fit && *wrap,
                            "manual text should fit/wrap: {} in {}",
                            layer.name,
                            spec.key
                        );
                    }
                }
            }
        }
        assert_eq!(seen.len(), 7);
    }

    #[test]
    fn end_card_keeps_platform_placement_zones_clear_and_correctly_proportioned() {
        let doc = build("set_show_end_card").unwrap();
        let video = doc
            .layers
            .iter()
            .find(|layer| layer.name == "Next video empty surface")
            .unwrap();
        assert!((video.w / video.h - 16. / 9.).abs() < 0.002);
        let subscribe = doc
            .layers
            .iter()
            .find(|layer| layer.name == "Subscribe empty circle")
            .unwrap();
        assert!((subscribe.w - subscribe.h).abs() < f32::EPSILON);
        assert!(
            doc.layers
                .iter()
                .any(|layer| layer.name == "End card station wordmark")
        );
    }

    #[test]
    fn defaults_and_sample_overrides_never_claim_a_real_schedule_or_credit() {
        let mut b = Bindings::default();
        bind(&mut b, &crate::config::Settings::default());
        for spec in SPECS {
            let mut doc = build(spec.key).unwrap();
            sample(&mut doc);
            let joined = doc
                .data_overrides
                .iter()
                .map(|(_, v)| v.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            match spec.kind {
                Kind::Schedule => {
                    assert!(joined.contains("SAMPLE ONLY") && joined.contains("SAMPLE • 7:00 AM"))
                }
                Kind::Credits => {
                    assert!(joined.contains("SAMPLE ONLY") && joined.contains("verify before use"))
                }
                Kind::EndCard => assert!(joined.contains("SAMPLE ONLY")),
            }
        }
        assert_eq!(
            b.get("set.packaging.schedule.0.time"),
            Some("Airtime not entered")
        );
        assert_eq!(
            b.get("set.packaging.credits.0.agency"),
            Some("Source / agency not entered")
        );
    }
}
