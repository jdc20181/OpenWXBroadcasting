//! NWS educational copy, verified 2026-11-13. No active-warning or location claims.
//! Sources and review dates: docs/codex/SET-DATA.md.
use super::reference::{frame, label, lowerthird_frame, social_frame};
use crate::gfx::{
    model::{AnimKind, Fill, GraphicDoc, Layer},
    templates::panel,
    text::Align,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Broadcast,
    Portrait,
    Square,
    LowerThird,
}
const SPECS: &[(&str, Format)] = &[
    ("set_warning_methods", Format::Broadcast),
    ("set_warning_methods__portrait", Format::Portrait),
    ("set_warning_methods__square", Format::Square),
    ("set_warning_methods__lowerthird", Format::LowerThird),
];
const METHODS: &[(&str, &str)] = &[
    (
        "Phone alerts",
        "Keep Wireless Emergency Alerts enabled on a compatible phone.",
    ),
    (
        "NOAA Weather Radio",
        "Use an alert-capable receiver for National Weather Service warnings.",
    ),
    (
        "Local broadcasts and apps",
        "Use trusted local broadcasts and alert apps for warning details.",
    ),
    (
        "Outdoor sirens",
        "Sirens alert people outdoors. Activation rules vary by community.",
    ),
];

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let format = SPECS.iter().find(|s| s.0 == key)?.1;
    let title = "Ways to Get Warnings";
    let subtitle = "Build your warning plan";
    let source = "Source: National Weather Service";
    let mut d = match format {
        Format::Portrait | Format::Square => social_frame(
            title,
            "Safety",
            subtitle,
            source,
            format == Format::Portrait,
        ),
        Format::LowerThird => lowerthird_frame(title, "Safety", subtitle, source),
        _ => frame(title, "Safety", subtitle, source),
    };
    if format == Format::LowerThird {
        d.layers.push(
            Layer::rect(
                "Warning methods band",
                0.,
                865.,
                1920.,
                131.,
                Fill::solid("$panel"),
                0.,
            )
            .with_anim(AnimKind::Rise, 0.2, 0.4),
        );
        label(
            &mut d,
            "Warning channels",
            (80., 875., 1760., 55.),
            "Weather radio • Phone alerts • Local broadcasts and apps",
            38.,
            "$text",
            Align::Left,
        );
        label(
            &mut d,
            "Siren context",
            (80., 936., 1760., 42.),
            "Outdoor sirens alert people outside. Plan for alerts indoors and overnight.",
            27.,
            "$text",
            Align::Left,
        );
        return Some(d);
    }
    let portrait = format == Format::Portrait;
    let square = format == Format::Square;
    label(
        &mut d,
        "Warning plan intro",
        if portrait {
            (54., 242., 972., 110.)
        } else if square {
            (54., 196., 972., 60.)
        } else {
            (80., 198., 1760., 75.)
        },
        "Use more than one way to receive alerts",
        if square { 34. } else { 44. },
        "$text",
        Align::Left,
    );
    for (i, &(title, body)) in METHODS.iter().enumerate() {
        let (x, y, w, h, inset, title_h, body_y, body_h, title_font, body_font) = if portrait {
            (
                54.,
                400. + i as f32 * 280.,
                972.,
                244.,
                112.,
                66.,
                100.,
                110.,
                43.,
                36.,
            )
        } else if square {
            (
                54.,
                280. + i as f32 * 150.,
                972.,
                136.,
                101.,
                48.,
                58.,
                64.,
                32.,
                26.,
            )
        } else {
            (
                80. + (i % 2) as f32 * 910.,
                300. + (i / 2) as f32 * 274.,
                850.,
                246.,
                105.,
                68.,
                100.,
                112.,
                41.,
                32.,
            )
        };
        d.layers.push(
            panel(&format!("Warning channel {i}"), x, y, w, h).with_anim(
                AnimKind::Rise,
                0.2 + i as f32 * 0.08,
                0.45,
            ),
        );
        label(
            &mut d,
            &format!("Channel number {i}"),
            (x + 26., y + 20., 64., 60.),
            &(i + 1).to_string(),
            if square { 36. } else { 43. },
            "$primary",
            Align::Left,
        );
        label(
            &mut d,
            &format!("Channel title {i}"),
            (
                x + inset,
                y + if square { 8. } else { 20. },
                w - inset - 24.,
                title_h,
            ),
            title,
            title_font,
            "$text",
            Align::Left,
        );
        label(
            &mut d,
            &format!("Channel explanation {i}"),
            (x + inset, y + body_y, w - inset - 24., body_h),
            body,
            body_font,
            "$muted",
            Align::Left,
        );
    }
    label(
        &mut d,
        "Indoor alert planning",
        if portrait {
            (54., 1580., 972., 112.)
        } else if square {
            (54., 902., 972., 64.)
        } else {
            (80., 865., 1760., 85.)
        },
        "Plan for alerts indoors and overnight.",
        if square { 28. } else { 34. },
        "$text",
        Align::Left,
    );
    Some(d)
}
