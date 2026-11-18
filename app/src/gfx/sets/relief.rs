//! Producer-maintained relief information; never invent organizations or instructions.
use super::reference::{frame, label, lowerthird_frame, social_frame, MISSING};
use crate::gfx::{
    bind::Bindings,
    model::{AnimKind, Fill, GraphicDoc, Layer},
    text::Align,
};

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let f = match key {
        "set_relief_info" => 0,
        "set_relief_info__portrait" => 1,
        "set_relief_info__square" => 2,
        "set_relief_info__lowerthird" => 3,
        _ => return None,
    };
    let title = "Community Relief";
    let mut d = match f {
        1 | 2 => social_frame(
            title,
            "Community",
            "{{set.relief.area}}",
            "Producer-provided relief information",
            f == 1,
        ),
        3 => lowerthird_frame(
            title,
            "Community",
            "{{set.relief.area}}",
            "Producer-provided relief information",
        ),
        _ => frame(
            title,
            "Community",
            "{{set.relief.area}}",
            "Producer-provided relief information",
        ),
    };
    if f == 3 {
        d.layers.push(
            Layer::rect(
                "Relief content",
                0.,
                865.,
                1920.,
                131.,
                Fill::solid("$panel"),
                0.,
            )
            .with_anim(AnimKind::Fade, 0., 0.2),
        );
        label(
            &mut d,
            "Relief organization",
            (80., 873., 1160., 48.),
            "{{set.relief.organization}}",
            38.,
            "$text",
            Align::Left,
        );
        label(
            &mut d,
            "Relief action",
            (80., 929., 1160., 48.),
            "{{set.relief.action}}",
            27.,
            "$text",
            Align::Left,
        );
        label(
            &mut d,
            "Relief verified",
            (1280., 873., 560., 42.),
            "{{set.relief.verified}}",
            23.,
            "$muted",
            Align::Left,
        );
        label(
            &mut d,
            "Relief confirmation",
            (1280., 925., 560., 55.),
            "Confirm details with the organization",
            23.,
            "$muted",
            Align::Left,
        );
    } else {
        let (x, w, start, step, h, font) = match f {
            1 => (54., 972., 280., 420., 350., 48.),
            2 => (54., 972., 215., 220., 190., 31.),
            _ => (80., 1760., 220., 210., 180., 40.),
        };
        for (i, (heading, field)) in [
            ("ORGANIZATION", "organization"),
            ("CURRENT NEED", "need"),
            ("HOW TO HELP", "action"),
        ]
        .iter()
        .enumerate()
        {
            let y = start + i as f32 * step;
            d.layers.push(
                crate::gfx::templates::panel(&format!("Relief panel {i}"), x, y, w, h).with_anim(
                    AnimKind::Rise,
                    0.1 + i as f32 * 0.1,
                    0.3,
                ),
            );
            label(
                &mut d,
                &format!("Relief heading {i}"),
                (x + 24., y + 16., w - 48., 40.),
                heading,
                24.,
                "$primary",
                Align::Left,
            );
            label(
                &mut d,
                &format!("Relief detail {i}"),
                (x + 24., y + 65., w - 48., h - 85.),
                &format!("{{{{set.relief.{field}}}}}"),
                font,
                "$text",
                Align::Left,
            );
        }
        let y = match f {
            1 => 1580.,
            2 => 885.,
            _ => 880.,
        };
        label(
            &mut d,
            "Relief verified",
            (x, y, w, 36.),
            "{{set.relief.verified}}",
            24.,
            "$muted",
            Align::Left,
        );
        label(
            &mut d,
            "Relief confirmation",
            (x, y + 45., w, 40.),
            "Confirm details with the organization before traveling or donating.",
            if f == 2 { 20. } else { 24. },
            "$muted",
            Align::Left,
        );
    }
    Some(d)
}

pub(super) fn bind(b: &mut Bindings) {
    for field in ["area", "organization", "need", "action", "verified"] {
        b.set(&format!("set.relief.{field}"), MISSING);
    }
}
#[cfg(test)]
pub(super) fn sample(d: &mut GraphicDoc) {
    if !d
        .template
        .as_deref()
        .is_some_and(|k| k.starts_with("set_relief_info"))
    {
        return;
    }
    for (field, value) in [
        ("area", "SAMPLE • Clark County, Indiana"),
        ("organization", "SAMPLE • Jeffersonville Community Relief"),
        (
            "need",
            "SAMPLE • Requested supplies listed by the organization",
        ),
        (
            "action",
            "SAMPLE • Contact the organization for current drop-off instructions",
        ),
        ("verified", "SAMPLE • Producer verified Nov 14, 9:00 AM ET"),
    ] {
        d.data_overrides
            .push((format!("set.relief.{field}"), value.into()));
    }
}
