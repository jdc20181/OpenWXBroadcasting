//! Native-format producer-supplied sponsor cards. No sponsor copy or destination is invented.
use super::reference::social_frame;
use crate::{
    config::Settings,
    gfx::{
        bind::Bindings,
        model::{AnimKind, Fill, GraphicDoc, ImageFit, Layer, LayerKind},
        templates::{BODY, DISPLAY, wrapped},
        text::{Align, Weight},
    },
};

const MISSING_NAME: &str = "Sponsor information not available yet";
const MISSING_MESSAGE: &str = "Sponsor message not provided";
const MISSING_DESTINATION: &str = "Destination not provided";
const MISSING_STATUS: &str = "Sponsor details not available";

#[derive(Clone, Copy)]
struct Format {
    key: &'static str,
    name: &'static str,
    width: u32,
    height: u32,
    portrait: bool,
}

const FORMATS: [Format; 2] = [
    Format {
        key: "set_sponsor_card__portrait",
        name: "Sponsor Card — Portrait",
        width: 1080,
        height: 1920,
        portrait: true,
    },
    Format {
        key: "set_sponsor_card__square",
        name: "Sponsor Card — Square",
        width: 1080,
        height: 1080,
        portrait: false,
    },
];

fn add_text(
    doc: &mut GraphicDoc,
    name: &str,
    bounds: (f32, f32, f32, f32),
    value: &str,
    font: &str,
    size: f32,
    weight: Weight,
    color: &str,
    align: Align,
    anim: AnimKind,
    delay: f32,
) {
    doc.layers.push(
        wrapped(Layer::text(
            name, bounds.0, bounds.1, bounds.2, bounds.3, value, font, size, weight, color, align,
        ))
        .with_anim(anim, delay, 0.5),
    );
}

fn build_format(format: Format) -> GraphicDoc {
    let title = "Community Sponsor";
    let mut doc = social_frame(
        title,
        "Show",
        "{{set.sponsor.status}}",
        "Producer supplied sponsor details",
        format.portrait,
    );
    doc.name = format.name.into();
    doc.category = "Show".into();
    doc.width = format.width;
    doc.height = format.height;
    doc.duration = 6.0;
    doc.template = Some(format.key.into());

    if format.portrait {
        add_text(
            &mut doc,
            "Logo label",
            (92.0, 258.0, 896.0, 44.0),
            "OPTIONAL SPONSOR LOGO",
            BODY,
            25.0,
            Weight::Bold,
            "$muted",
            Align::Center,
            AnimKind::Fade,
            0.25,
        );
        doc.layers.push(
            Layer::rect(
                "Logo plate",
                92.0,
                312.0,
                896.0,
                390.0,
                Fill::solid("$panel"),
                18.0,
            )
            .with_anim(AnimKind::Fade, 0.3, 0.4),
        );
        doc.layers.push(
            Layer::new(
                "Sponsor logo",
                152.0,
                332.0,
                776.0,
                350.0,
                LayerKind::Image {
                    media: "{{set.sponsor.logo}}".into(),
                    fit: ImageFit::Contain,
                },
            )
            .with_anim(AnimKind::Pop, 0.35, 0.55),
        );
        doc.layers.push(
            Layer::rect(
                "Name accent",
                452.0,
                754.0,
                176.0,
                6.0,
                Fill::solid("$primary"),
                0.0,
            )
            .with_anim(AnimKind::Wipe, 0.45, 0.35),
        );
        add_text(
            &mut doc,
            "Sponsor name",
            (70.0, 790.0, 940.0, 156.0),
            "{{set.sponsor.name}}",
            DISPLAY,
            76.0,
            Weight::Black,
            "$text",
            Align::Center,
            AnimKind::SlideUp,
            0.5,
        );
        add_text(
            &mut doc,
            "Sponsor message",
            (105.0, 980.0, 870.0, 110.0),
            "{{set.sponsor.message}}",
            BODY,
            38.0,
            Weight::SemiBold,
            "$primary",
            Align::Center,
            AnimKind::Fade,
            0.65,
        );
        doc.layers.push(
            Layer::rect(
                "Destination plate",
                94.0,
                1150.0,
                892.0,
                150.0,
                Fill::solid("$panel"),
                16.0,
            )
            .with_anim(AnimKind::Fade, 0.75, 0.4),
        );
        add_text(
            &mut doc,
            "Destination label",
            (132.0, 1165.0, 816.0, 38.0),
            "OPTIONAL DESTINATION",
            BODY,
            22.0,
            Weight::Bold,
            "$muted",
            Align::Center,
            AnimKind::Fade,
            0.8,
        );
        add_text(
            &mut doc,
            "Destination",
            (132.0, 1207.0, 816.0, 72.0),
            "{{set.sponsor.destination}}",
            BODY,
            32.0,
            Weight::SemiBold,
            "$text",
            Align::Center,
            AnimKind::SlideUp,
            0.85,
        );
        add_text(
            &mut doc,
            "Producer note",
            (82.0, 1388.0, 916.0, 56.0),
            "Sponsor information is supplied by the producer",
            BODY,
            22.0,
            Weight::Regular,
            "$muted",
            Align::Center,
            AnimKind::Fade,
            0.95,
        );
    } else {
        add_text(
            &mut doc,
            "Logo label",
            (100.0, 204.0, 880.0, 40.0),
            "OPTIONAL SPONSOR LOGO",
            BODY,
            22.0,
            Weight::Bold,
            "$muted",
            Align::Center,
            AnimKind::Fade,
            0.25,
        );
        doc.layers.push(
            Layer::rect(
                "Logo plate",
                140.0,
                252.0,
                800.0,
                258.0,
                Fill::solid("$panel"),
                16.0,
            )
            .with_anim(AnimKind::Fade, 0.3, 0.4),
        );
        doc.layers.push(
            Layer::new(
                "Sponsor logo",
                184.0,
                268.0,
                712.0,
                226.0,
                LayerKind::Image {
                    media: "{{set.sponsor.logo}}".into(),
                    fit: ImageFit::Contain,
                },
            )
            .with_anim(AnimKind::Pop, 0.35, 0.55),
        );
        doc.layers.push(
            Layer::rect(
                "Name accent",
                452.0,
                536.0,
                176.0,
                5.0,
                Fill::solid("$primary"),
                0.0,
            )
            .with_anim(AnimKind::Wipe, 0.45, 0.35),
        );
        add_text(
            &mut doc,
            "Sponsor name",
            (70.0, 558.0, 940.0, 112.0),
            "{{set.sponsor.name}}",
            DISPLAY,
            65.0,
            Weight::Black,
            "$text",
            Align::Center,
            AnimKind::SlideUp,
            0.5,
        );
        add_text(
            &mut doc,
            "Sponsor message",
            (100.0, 688.0, 880.0, 76.0),
            "{{set.sponsor.message}}",
            BODY,
            31.0,
            Weight::SemiBold,
            "$primary",
            Align::Center,
            AnimKind::Fade,
            0.65,
        );
        doc.layers.push(
            Layer::rect(
                "Destination plate",
                110.0,
                782.0,
                860.0,
                102.0,
                Fill::solid("$panel"),
                14.0,
            )
            .with_anim(AnimKind::Fade, 0.75, 0.4),
        );
        add_text(
            &mut doc,
            "Destination label",
            (132.0, 790.0, 816.0, 30.0),
            "OPTIONAL DESTINATION",
            BODY,
            18.0,
            Weight::Bold,
            "$muted",
            Align::Center,
            AnimKind::Fade,
            0.8,
        );
        add_text(
            &mut doc,
            "Destination",
            (132.0, 820.0, 816.0, 50.0),
            "{{set.sponsor.destination}}",
            BODY,
            25.0,
            Weight::SemiBold,
            "$text",
            Align::Center,
            AnimKind::SlideUp,
            0.85,
        );
        add_text(
            &mut doc,
            "Producer note",
            (80.0, 904.0, 920.0, 42.0),
            "Sponsor information is supplied by the producer",
            BODY,
            19.0,
            Weight::Regular,
            "$muted",
            Align::Center,
            AnimKind::Fade,
            0.95,
        );
    }
    doc
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    FORMATS
        .iter()
        .copied()
        .find(|format| format.key == key)
        .map(build_format)
}

pub(super) fn bind(bindings: &mut Bindings, settings: &Settings) {
    bindings.set(
        "set.sponsor.name",
        if settings.sponsor_name.trim().is_empty() {
            MISSING_NAME
        } else {
            settings.sponsor_name.trim()
        },
    );
    bindings.set(
        "set.sponsor.message",
        if settings.sponsor_tagline.trim().is_empty() {
            MISSING_MESSAGE
        } else {
            settings.sponsor_tagline.trim()
        },
    );
    bindings.set(
        "set.sponsor.logo",
        settings.sponsor_logo_media.as_deref().unwrap_or_default(),
    );
    bindings.set(
        "set.sponsor.status",
        if settings.sponsor_name.trim().is_empty() {
            MISSING_STATUS
        } else {
            "Producer supplied sponsor information"
        },
    );
    // Destination has no Settings field; producers may supply this optional manual text.
    bindings.set("set.sponsor.destination", MISSING_DESTINATION);
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    if !doc
        .template
        .as_deref()
        .is_some_and(|key| FORMATS.iter().any(|f| f.key == key))
    {
        return;
    }
    doc.data_overrides.extend([
        (
            "set.sponsor.name".into(),
            "SAMPLE • Community Sponsor".into(),
        ),
        (
            "set.sponsor.message".into(),
            "SAMPLE • Synthetic sponsor message".into(),
        ),
        (
            "set.sponsor.destination".into(),
            "SAMPLE • Not a real destination".into(),
        ),
        ("set.sponsor.logo".into(), "sets-qa-image.png".into()),
        (
            "set.sponsor.status".into(),
            "SAMPLE • Synthetic • Not for broadcast".into(),
        ),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_cards_fit_below_shared_header_and_above_footer() {
        for format in FORMATS {
            let doc = build(format.key).unwrap();
            assert_eq!((doc.width, doc.height), (format.width, format.height));
            assert_eq!(doc.template.as_deref(), Some(format.key));
            assert!(doc.layers.iter().any(|layer| layer.name == "Social Title"));
            assert!(
                doc.layers
                    .iter()
                    .any(|layer| layer.name == "Social Footer left")
            );
            let footer_top = if format.portrait { 1750.0 } else { 980.0 };
            let header_bottom = if format.portrait { 218.0 } else { 178.0 };
            assert!(doc.layers.iter().all(|layer| {
                layer.x >= 0.0
                    && layer.y >= 0.0
                    && layer.x + layer.w <= format.width as f32
                    && layer.y + layer.h <= format.height as f32
            }));
            for layer in doc
                .layers
                .iter()
                .filter(|layer| !layer.name.starts_with("Social ") && layer.name != "Brand frame")
            {
                assert!(
                    layer.y >= header_bottom,
                    "{} overlaps shared header",
                    layer.name
                );
                assert!(
                    layer.y + layer.h <= footer_top,
                    "{} overlaps shared footer",
                    layer.name
                );
                assert!(
                    layer.anim_in.kind != AnimKind::None,
                    "{} has no entrance",
                    layer.name
                );
            }

            let mut settings = Settings::default();
            let mut bindings = Bindings::default();
            bind(&mut bindings, &settings);
            assert_eq!(bindings.apply("{{set.sponsor.name}}"), MISSING_NAME);
            assert_eq!(bindings.apply("{{set.sponsor.message}}"), MISSING_MESSAGE);
            assert_eq!(
                bindings.apply("{{set.sponsor.destination}}"),
                MISSING_DESTINATION
            );
            assert_eq!(bindings.apply("{{set.sponsor.logo}}"), "");
            settings.sponsor_name = "Verified Producer Sponsor".into();
            settings.sponsor_tagline = "Producer supplied message".into();
            settings.sponsor_logo_media = Some("approved-sponsor-mark.png".into());
            bind(&mut bindings, &settings);
            assert_eq!(
                bindings.apply("{{set.sponsor.name}}"),
                "Verified Producer Sponsor"
            );
            assert_eq!(
                bindings.apply("{{set.sponsor.message}}"),
                "Producer supplied message"
            );
            assert_eq!(
                bindings.apply("{{set.sponsor.logo}}"),
                "approved-sponsor-mark.png"
            );

            let mut sample_doc = doc.clone();
            sample(&mut sample_doc);
            assert!(
                sample_doc
                    .data_overrides
                    .iter()
                    .any(|(k, v)| k == "set.sponsor.logo" && v == "sets-qa-image.png")
            );
            assert!(
                sample_doc
                    .data_overrides
                    .iter()
                    .any(|(k, v)| k == "set.sponsor.status" && v.contains("SAMPLE"))
            );
        }
    }
}
