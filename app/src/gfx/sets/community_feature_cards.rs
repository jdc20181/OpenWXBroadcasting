//! Manual-only community birthday/shout-out and pet forecast presentation cards.
//! Copy and local media are supplied by producers via GraphicDoc data_overrides.

use super::reference::{frame, label, lowerthird_frame, social_frame};
use crate::gfx::{
    bind::Bindings,
    model::{AnimKind, Fill, GraphicDoc, ImageFit, Layer, LayerKind},
    templates::{panel, text, DISPLAY},
    text::{Align, Weight},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Birthday,
    PetForecast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Wide,
    Portrait,
    Square,
    LowerThird,
}

#[derive(Clone, Copy, Debug)]
struct Spec {
    key: &'static str,
    kind: Kind,
    format: Format,
}

const SPECS: &[Spec] = &[
    Spec {
        key: "set_community_birthday",
        kind: Kind::Birthday,
        format: Format::Wide,
    },
    Spec {
        key: "set_community_birthday__portrait",
        kind: Kind::Birthday,
        format: Format::Portrait,
    },
    Spec {
        key: "set_community_birthday__square",
        kind: Kind::Birthday,
        format: Format::Square,
    },
    Spec {
        key: "set_community_birthday__lowerthird",
        kind: Kind::Birthday,
        format: Format::LowerThird,
    },
    Spec {
        key: "set_community_pet_forecast",
        kind: Kind::PetForecast,
        format: Format::Wide,
    },
    Spec {
        key: "set_community_pet_forecast__portrait",
        kind: Kind::PetForecast,
        format: Format::Portrait,
    },
    Spec {
        key: "set_community_pet_forecast__square",
        kind: Kind::PetForecast,
        format: Format::Square,
    },
];

fn txt(
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
        *vcenter = true;
    }
    layer
}

fn named_frame(title: &str, subtitle: &str, format: Format) -> GraphicDoc {
    match format {
        Format::Wide => frame(title, "Community", subtitle, "Manual • Station production"),
        Format::Portrait => social_frame(
            title,
            "Community",
            subtitle,
            "Manual • Station production",
            true,
        ),
        Format::Square => social_frame(
            title,
            "Community",
            subtitle,
            "Manual • Station production",
            false,
        ),
        Format::LowerThird => {
            lowerthird_frame(title, "Community", subtitle, "Manual • Station production")
        }
    }
}

fn birthday(format: Format) -> GraphicDoc {
    let mut doc = named_frame("Community Shout-out", "Producer supplied message", format);
    match format {
        Format::Wide => {
            doc.layers
                .push(panel("Birthday greeting panel", 100., 220., 1720., 610.));
            label(
                &mut doc,
                "Community member",
                (150., 300., 1620., 150.),
                "{{set.community.birthday.name}}",
                84.,
                "$text",
                Align::Center,
            );
            label(
                &mut doc,
                "Shout-out message",
                (190., 488., 1540., 230.),
                "{{set.community.birthday.message}}",
                56.,
                "$primary",
                Align::Center,
            );
            label(
                &mut doc,
                "Occasion",
                (180., 750., 1560., 60.),
                "{{set.community.birthday.occasion}}",
                32.,
                "$muted",
                Align::Center,
            );
        }
        Format::Portrait => {
            doc.layers
                .push(panel("Birthday greeting panel", 54., 350., 972., 930.));
            label(
                &mut doc,
                "Community member",
                (100., 480., 880., 190.),
                "{{set.community.birthday.name}}",
                68.,
                "$text",
                Align::Center,
            );
            label(
                &mut doc,
                "Shout-out message",
                (110., 730., 860., 360.),
                "{{set.community.birthday.message}}",
                52.,
                "$primary",
                Align::Center,
            );
            label(
                &mut doc,
                "Occasion",
                (112., 1138., 856., 82.),
                "{{set.community.birthday.occasion}}",
                32.,
                "$muted",
                Align::Center,
            );
        }
        Format::Square => {
            doc.layers
                .push(panel("Birthday greeting panel", 54., 220., 972., 590.));
            label(
                &mut doc,
                "Community member",
                (100., 310., 880., 140.),
                "{{set.community.birthday.name}}",
                58.,
                "$text",
                Align::Center,
            );
            label(
                &mut doc,
                "Shout-out message",
                (112., 500., 856., 210.),
                "{{set.community.birthday.message}}",
                42.,
                "$primary",
                Align::Center,
            );
            label(
                &mut doc,
                "Occasion",
                (112., 730., 856., 54.),
                "{{set.community.birthday.occasion}}",
                28.,
                "$muted",
                Align::Center,
            );
        }
        Format::LowerThird => {
            doc.layers
                .push(Layer::rect("Shout-out band", 80., 870., 1760., 118., Fill::solid("$panel"), 22.));
            doc.layers.push(txt(
                "Community member",
                (104., 884., 585., 86.),
                "{{set.community.birthday.name}}",
                36.,
                "$text",
                Align::Left,
            ));
            doc.layers.push(txt(
                "Shout-out message",
                (710., 880., 1100., 94.),
                "{{set.community.birthday.message}}",
                30.,
                "$primary",
                Align::Right,
            ));
        }
    }
    doc
}

fn photo(doc: &mut GraphicDoc, media: &str, well: (f32, f32, f32, f32)) {
    doc.layers
        .push(panel("Pet photo well", well.0, well.1, well.2, well.3));
    doc.layers.push(
        Layer::new(
            "Producer local pet photo",
            well.0 + 12.,
            well.1 + 12.,
            well.2 - 24.,
            well.3 - 24.,
            LayerKind::Image {
                media: media.into(),
                fit: ImageFit::Contain,
            },
        )
        .with_anim(AnimKind::Fade, 0.22, 0.45),
    );
    doc.layers.push(txt(
        "Pet photo prompt",
        (well.0 + 28., well.1 + 22., well.2 - 56., 72.),
        "{{set.community.pet.photo_note}}",
        26.,
        "$muted",
        Align::Center,
    ));
}

fn pet_forecast(format: Format) -> GraphicDoc {
    let mut doc = named_frame(
        "Pet of the Week",
        "Producer supplied photo and forecast copy",
        format,
    );
    match format {
        Format::Wide => {
            photo(
                &mut doc,
                "{{set.community.pet.photo}}",
                (82., 205., 770., 610.),
            );
            label(
                &mut doc,
                "Pet name",
                (900., 215., 920., 92.),
                "{{set.community.pet.name}}",
                52.,
                "$text",
                Align::Left,
            );
            label(
                &mut doc,
                "Photo caption",
                (900., 318., 920., 100.),
                "{{set.community.pet.caption}}",
                34.,
                "$muted",
                Align::Left,
            );
            label(
                &mut doc,
                "Manual forecast period",
                (900., 425., 920., 62.),
                "{{set.community.pet.period}}",
                32.,
                "$primary",
                Align::Left,
            );
            label(
                &mut doc,
                "Manual forecast heading",
                (900., 494., 920., 34.),
                "MANUAL FORECAST COPY",
                23.,
                "$muted",
                Align::Left,
            );
            label(
                &mut doc,
                "Manual forecast copy",
                (900., 534., 920., 180.),
                "{{set.community.pet.forecast}}",
                38.,
                "$text",
                Align::Left,
            );
            label(
                &mut doc,
                "Photo credit",
                (900., 740., 920., 66.),
                "{{set.community.pet.credit}}",
                26.,
                "$muted",
                Align::Left,
            );
        }
        Format::Portrait => {
            photo(
                &mut doc,
                "{{set.community.pet.photo}}",
                (72., 255., 936., 760.),
            );
            label(
                &mut doc,
                "Pet name",
                (80., 1060., 920., 96.),
                "{{set.community.pet.name}}",
                52.,
                "$text",
                Align::Center,
            );
            label(
                &mut doc,
                "Photo caption",
                (90., 1162., 900., 92.),
                "{{set.community.pet.caption}}",
                31.,
                "$muted",
                Align::Center,
            );
            label(
                &mut doc,
                "Manual forecast period",
                (92., 1280., 896., 58.),
                "{{set.community.pet.period}}",
                31.,
                "$primary",
                Align::Left,
            );
            label(
                &mut doc,
                "Manual forecast heading",
                (92., 1340., 896., 34.),
                "MANUAL FORECAST COPY",
                23.,
                "$muted",
                Align::Left,
            );
            label(
                &mut doc,
                "Manual forecast copy",
                (92., 1378., 896., 150.),
                "{{set.community.pet.forecast}}",
                34.,
                "$text",
                Align::Left,
            );
            label(
                &mut doc,
                "Photo credit",
                (92., 1550., 896., 84.),
                "{{set.community.pet.credit}}",
                27.,
                "$muted",
                Align::Left,
            );
        }
        Format::Square => {
            photo(
                &mut doc,
                "{{set.community.pet.photo}}",
                (64., 205., 952., 405.),
            );
            label(
                &mut doc,
                "Pet name",
                (74., 632., 932., 70.),
                "{{set.community.pet.name}}",
                42.,
                "$text",
                Align::Center,
            );
            label(
                &mut doc,
                "Photo caption",
                (82., 710., 916., 74.),
                "{{set.community.pet.caption}}",
                28.,
                "$muted",
                Align::Center,
            );
            label(
                &mut doc,
                "Manual forecast period",
                (82., 792., 916., 44.),
                "{{set.community.pet.period}}",
                26.,
                "$primary",
                Align::Left,
            );
            label(
                &mut doc,
                "Manual forecast heading",
                (82., 840., 916., 24.),
                "MANUAL FORECAST COPY",
                18.,
                "$muted",
                Align::Left,
            );
            label(
                &mut doc,
                "Manual forecast copy",
                (82., 868., 916., 50.),
                "{{set.community.pet.forecast}}",
                28.,
                "$text",
                Align::Left,
            );
            label(
                &mut doc,
                "Photo credit",
                (82., 928., 916., 38.),
                "{{set.community.pet.credit}}",
                22.,
                "$muted",
                Align::Left,
            );
        }
        Format::LowerThird => unreachable!("pet feature has no lower-third catalog row"),
    }
    doc
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = *SPECS.iter().find(|spec| spec.key == key)?;
    let mut doc = match spec.kind {
        Kind::Birthday => birthday(spec.format),
        Kind::PetForecast => pet_forecast(spec.format),
    };
    doc.template = Some(key.into());
    Some(doc)
}

/// Safe manual values for fields producers may enter through GraphicDoc data overrides.
pub(super) fn bind(bindings: &mut Bindings) {
    for (key, value) in [
        ("set.community.birthday.name", "Name not entered"),
        (
            "set.community.birthday.message",
            "Shout-out message not entered",
        ),
        ("set.community.birthday.occasion", "Community shout-out"),
        ("set.community.pet.photo", ""),
        ("set.community.pet.photo_note", "Pet photo"),
        ("set.community.pet.name", "Pet name not entered"),
        ("set.community.pet.caption", "Caption not entered"),
        ("set.community.pet.credit", "Photo credit not entered"),
        (
            "set.community.pet.forecast",
            "Manual forecast copy not entered",
        ),
        ("set.community.pet.period", "Forecast period not entered"),
    ] {
        if bindings.get(key).map(str::trim).unwrap_or("").is_empty()
            || bindings.get(key) == Some("--")
        {
            bindings.set(key, value);
        }
    }
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(spec) = SPECS
        .iter()
        .find(|spec| Some(spec.key) == doc.template.as_deref())
    else {
        return;
    };
    let values: &[(&str, &str)] = match spec.kind {
        Kind::Birthday => &[
            (
                "set.community.birthday.name",
                "SAMPLE ONLY • Community member",
            ),
            (
                "set.community.birthday.message",
                "SAMPLE ONLY • Sending warm wishes from the station.",
            ),
            (
                "set.community.birthday.occasion",
                "SAMPLE ONLY • Example community greeting",
            ),
        ],
        Kind::PetForecast => &[
            ("set.community.pet.photo", "sets-qa-image.png"),
            (
                "set.community.pet.photo_note",
                "SAMPLE ONLY • Synthetic image fixture • Not a real pet photo",
            ),
            ("set.community.pet.name", "SAMPLE ONLY • Pet"),
            (
                "set.community.pet.caption",
                "SAMPLE ONLY • Synthetic local media placeholder",
            ),
            (
                "set.community.pet.credit",
                "SAMPLE ONLY • Producer credit placeholder",
            ),
            (
                "set.community.pet.forecast",
                "SAMPLE ONLY • Producer-authored forecast copy placeholder • Not a forecast",
            ),
            (
                "set.community.pet.period",
                "SAMPLE ONLY • Producer-entered period placeholder",
            ),
        ],
    };
    for (key, value) in values {
        doc.data_overrides.push(((*key).into(), (*value).into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_keys_native_sizes_and_layer_bounds() {
        let expected = [
            ("set_community_birthday", 1920, 1080),
            ("set_community_birthday__portrait", 1080, 1920),
            ("set_community_birthday__square", 1080, 1080),
            ("set_community_birthday__lowerthird", 1920, 1080),
            ("set_community_pet_forecast", 1920, 1080),
            ("set_community_pet_forecast__portrait", 1080, 1920),
            ("set_community_pet_forecast__square", 1080, 1080),
        ];
        for (key, width, height) in expected {
            let doc = build(key).unwrap();
            assert_eq!((doc.width, doc.height), (width, height), "{key}");
            for layer in &doc.layers {
                assert!(layer.x >= 0. && layer.y >= 0., "{key}: {}", layer.name);
                assert!(layer.x + layer.w <= width as f32, "{key}: {}", layer.name);
                assert!(layer.y + layer.h <= height as f32, "{key}: {}", layer.name);
            }
        }
        assert!(build("set_community_pet_forecast__lowerthird").is_none());
    }

    #[test]
    fn manual_defaults_are_independent_and_never_weather_fallbacks() {
        let mut bindings = Bindings::default();
        bind(&mut bindings);
        assert_eq!(
            bindings.get("set.community.birthday.name"),
            Some("Name not entered")
        );
        assert_eq!(
            bindings.get("set.community.birthday.message"),
            Some("Shout-out message not entered")
        );
        assert_eq!(
            bindings.get("set.community.pet.name"),
            Some("Pet name not entered")
        );
        assert_eq!(
            bindings.get("set.community.pet.forecast"),
            Some("Manual forecast copy not entered")
        );
        assert_eq!(
            bindings.get("set.community.pet.period"),
            Some("Forecast period not entered")
        );
        assert_eq!(bindings.get("set.community.pet.photo"), Some(""));

        bindings.set("set.community.pet.name", "Producer pet name");
        bind(&mut bindings);
        assert_eq!(
            bindings.get("set.community.pet.name"),
            Some("Producer pet name")
        );
        assert_eq!(
            bindings.get("set.community.pet.forecast"),
            Some("Manual forecast copy not entered")
        );
    }

    #[test]
    fn pet_photo_uses_contained_local_media_and_lowerthird_stays_transparent() {
        let pet = build("set_community_pet_forecast").unwrap();
        assert!(pet.layers.iter().any(|layer| matches!(
            &layer.kind,
            LayerKind::Image { media, fit: ImageFit::Contain } if media == "{{set.community.pet.photo}}"
        )));
        let forecast = pet
            .layers
            .iter()
            .find(|layer| layer.name == "Manual forecast copy")
            .unwrap();
        let credit = pet
            .layers
            .iter()
            .find(|layer| layer.name == "Photo credit")
            .unwrap();
        assert!(forecast.y + forecast.h <= credit.y);
        let lower = build("set_community_birthday__lowerthird").unwrap();
        assert!(lower.background.is_none());
        assert!(lower
            .layers
            .iter()
            .any(|layer| layer.name == "Shout-out band"));
    }

    #[test]
    fn sample_overrides_are_generic_and_clearly_synthetic() {
        let mut birthday = build("set_community_birthday").unwrap();
        sample(&mut birthday);
        assert!(birthday.data_overrides.iter().all(|(key, value)| {
            key.starts_with("set.community.birthday.") && value.contains("SAMPLE ONLY")
        }));
        assert!(birthday.data_overrides.iter().any(|(key, value)| {
            key == "set.community.birthday.name" && value == "SAMPLE ONLY • Community member"
        }));

        let mut pet = build("set_community_pet_forecast__square").unwrap();
        sample(&mut pet);
        assert!(pet.data_overrides.iter().any(|(key, value)| {
            key == "set.community.pet.photo" && value == "sets-qa-image.png"
        }));
        assert!(pet.data_overrides.iter().any(|(key, value)| {
            key == "set.community.pet.photo_note" && value.contains("Not a real pet photo")
        }));
        assert!(pet.data_overrides.iter().all(|(key, value)| {
            key.starts_with("set.community.pet.")
                && (key == "set.community.pet.photo" || value.contains("SAMPLE ONLY"))
        }));
    }
}
