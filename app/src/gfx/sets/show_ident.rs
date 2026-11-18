//! Manual station identity and program-continuity cards in native social formats.
//! These templates contain producer-authored copy only; they infer no live status.
use super::reference::{label, social_frame};
use crate::{
    config::Settings,
    gfx::{
        bind::Bindings,
        model::{AnimKind, Fill, GraphicDoc, Layer},
        text::Align,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    StationId,
    ComingUp,
    Signoff,
    Technical,
}

#[derive(Clone, Copy, Debug)]
struct Spec {
    key: &'static str,
    purpose: Purpose,
    /// 1 = native portrait, 2 = native square.
    format: u8,
}

const SPECS: &[Spec] = &[
    Spec {
        key: "set_station_id__portrait",
        purpose: Purpose::StationId,
        format: 1,
    },
    Spec {
        key: "set_station_id__square",
        purpose: Purpose::StationId,
        format: 2,
    },
    Spec {
        key: "set_coming_up__portrait",
        purpose: Purpose::ComingUp,
        format: 1,
    },
    Spec {
        key: "set_coming_up__square",
        purpose: Purpose::ComingUp,
        format: 2,
    },
    Spec {
        key: "set_sign_off__portrait",
        purpose: Purpose::Signoff,
        format: 1,
    },
    Spec {
        key: "set_sign_off__square",
        purpose: Purpose::Signoff,
        format: 2,
    },
    Spec {
        key: "set_please_stay_with_us__portrait",
        purpose: Purpose::Technical,
        format: 1,
    },
    Spec {
        key: "set_please_stay_with_us__square",
        purpose: Purpose::Technical,
        format: 2,
    },
];

fn token(purpose: &str, field: &str) -> String {
    format!("{{{{set.showident.{purpose}.{field}}}}}")
}

fn edition() -> &'static str {
    "{{set.showident.edition}}"
}

fn source() -> &'static str {
    "{{set.showident.source}}"
}

fn card(d: &mut GraphicDoc, portrait: bool, name: &str) -> (f32, f32, f32, f32) {
    let rect = if portrait {
        (54., 280., 972., 1360.)
    } else {
        (54., 220., 972., 700.)
    };
    d.layers.push(
        crate::gfx::templates::panel(name, rect.0, rect.1, rect.2, rect.3)
            .with_anim(AnimKind::Fade, 0.12, 0.55),
    );
    d.layers.push(
        Layer::rect(
            "Identity accent",
            rect.0,
            rect.1,
            9.,
            rect.3,
            Fill::solid("$primary"),
            0.,
        )
        .with_anim(AnimKind::Grow, 0.22, 0.45),
    );
    rect
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = *SPECS.iter().find(|s| s.key == key)?;
    let portrait = spec.format == 1;
    let (title, card_name) = match spec.purpose {
        Purpose::StationId => ("Station ID", "Station identity card"),
        Purpose::ComingUp => ("Coming Up", "Coming up card"),
        Purpose::Signoff => ("Sign-off", "Sign-off card"),
        Purpose::Technical => ("Please Stay With Us", "Continuity card"),
    };
    let mut d = social_frame(title, "Station", edition(), source(), portrait);
    let (x, y, w, h) = card(&mut d, portrait, card_name);
    let inner_x = x + 58.;
    let inner_w = w - 116.;

    match spec.purpose {
        Purpose::StationId => {
            label(
                &mut d,
                "Station identifier",
                (inner_x, y + if portrait { 180. } else { 95. }, inner_w, if portrait { 280. } else { 160. }),
                &token("station", "name"),
                if portrait { 86. } else { 68. },
                "$text",
                Align::Center,
            );
            label(
                &mut d,
                "Station short identifier",
                (inner_x, y + if portrait { 565. } else { 310. }, inner_w, 96.),
                &token("station", "short"),
                if portrait { 48. } else { 40. },
                "$primary",
                Align::Center,
            );
            label(
                &mut d,
                "Station tagline",
                (inner_x + 28., y + if portrait { 760. } else { 430. }, inner_w - 56., if portrait { 250. } else { 160. }),
                &token("station", "tagline"),
                if portrait { 43. } else { 34. },
                "$muted",
                Align::Center,
            );
        }
        Purpose::ComingUp => {
            label(
                &mut d,
                "Coming up cue",
                (inner_x, y + if portrait { 120. } else { 70. }, inner_w, 80.),
                "COMING UP",
                if portrait { 48. } else { 38. },
                "$primary",
                Align::Center,
            );
            label(
                &mut d,
                "Next segment",
                (inner_x + 20., y + if portrait { 360. } else { 205. }, inner_w - 40., if portrait { 350. } else { 190. }),
                &token("coming", "title"),
                if portrait { 78. } else { 62. },
                "$text",
                Align::Center,
            );
            label(
                &mut d,
                "Coming up detail",
                (inner_x + 30., y + if portrait { 820. } else { 430. }, inner_w - 60., if portrait { 290. } else { 180. }),
                &token("coming", "detail"),
                if portrait { 42. } else { 34. },
                "$muted",
                Align::Center,
            );
        }
        Purpose::Signoff => {
            label(
                &mut d,
                "Sign-off station name",
                (inner_x, y + if portrait { 205. } else { 105. }, inner_w, 90.),
                &token("station", "name"),
                if portrait { 47. } else { 38. },
                "$primary",
                Align::Center,
            );
            label(
                &mut d,
                "Sign-off thanks",
                (inner_x + 20., y + if portrait { 410. } else { 250. }, inner_w - 40., if portrait { 300. } else { 175. }),
                &token("signoff", "title"),
                if portrait { 76. } else { 61. },
                "$text",
                Align::Center,
            );
            label(
                &mut d,
                "Sign-off message",
                (inner_x + 30., y + if portrait { 850. } else { 465. }, inner_w - 60., if portrait { 250. } else { 145. }),
                &token("signoff", "message"),
                if portrait { 41. } else { 33. },
                "$muted",
                Align::Center,
            );
        }
        Purpose::Technical => {
            label(
                &mut d,
                "Continuity headline",
                (inner_x + 20., y + if portrait { 300. } else { 175. }, inner_w - 40., if portrait { 300. } else { 170. }),
                &token("technical", "title"),
                if portrait { 78. } else { 61. },
                "$text",
                Align::Center,
            );
            label(
                &mut d,
                "Continuity message",
                (inner_x + 35., y + if portrait { 760. } else { 405. }, inner_w - 70., if portrait { 310. } else { 180. }),
                &token("technical", "message"),
                if portrait { 43. } else { 34. },
                "$muted",
                Align::Center,
            );
        }
    }
    Some(d)
}

pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let station_name = if settings.station_name.trim().is_empty() {
        "Station name not entered"
    } else {
        &settings.station_name
    };
    let station_short = if settings.station_short.trim().is_empty() {
        "Station identifier not entered"
    } else {
        &settings.station_short
    };
    let tagline = if settings.tagline.trim().is_empty() {
        "Station tagline not entered"
    } else {
        &settings.tagline
    };
    for (field, value) in [
        ("edition", "Manual producer copy"),
        ("source", "Source: Station production"),
        ("station.name", station_name),
        ("station.short", station_short),
        ("station.tagline", tagline),
        ("coming.title", "Next segment not entered yet"),
        ("coming.detail", "Producer tease not entered yet"),
        ("signoff.title", "Thank you for watching"),
        ("signoff.message", "Thank you for being part of our community"),
        ("technical.title", "Please stay with us"),
        (
            "technical.message",
            "Technical difficulties • Thank you for your patience",
        ),
    ] {
        b.set(&format!("set.showident.{field}"), value);
    }
}

#[cfg(test)]
pub(super) fn sample(d: &mut GraphicDoc) {
    if !SPECS.iter().any(|s| Some(s.key) == d.template.as_deref()) {
        return;
    }
    for (field, value) in [
        ("edition", "SAMPLE • Synthetic station packaging copy"),
        ("source", "SAMPLE ONLY • Synthetic producer copy"),
        ("station.name", "SAMPLE • Jeffersonville Community Weather Network"),
        ("station.short", "SAMPLE • JCWN"),
        (
            "station.tagline",
            "SAMPLE • Forecasts, updates and local perspective",
        ),
        ("coming.title", "SAMPLE • Jeffersonville community forecast"),
        (
            "coming.detail",
            "SAMPLE • Local outlook, road conditions and community updates",
        ),
        ("signoff.title", "SAMPLE • Thank you for watching"),
        (
            "signoff.message",
            "SAMPLE • Thank you for being part of our community",
        ),
        ("technical.title", "SAMPLE • Please stay with us"),
        (
            "technical.message",
            "SAMPLE • A technical-difficulties card for layout review",
        ),
    ] {
        d.data_overrides
            .push((format!("set.showident.{field}"), value.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn text_layers(d: &GraphicDoc) -> impl Iterator<Item = &str> {
        d.layers.iter().filter_map(|layer| match &layer.kind {
            crate::gfx::model::LayerKind::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
    }

    #[test]
    fn all_manual_packaging_variants_have_unique_native_geometry_and_purpose_bindings() {
        let mut keys = HashSet::new();
        for spec in SPECS {
            assert!(keys.insert(spec.key), "duplicate key {}", spec.key);
            let d = build(spec.key).expect("registered manual packaging key builds");
            assert_eq!(d.width, 1080);
            assert_eq!(d.height, if spec.format == 1 { 1920 } else { 1080 });
            assert!(d.layers.len() >= 12, "{} has a complete branded composition", spec.key);
            assert!(d.layers.iter().all(|layer| {
                layer.x.is_finite()
                    && layer.y.is_finite()
                    && layer.w.is_finite()
                    && layer.h.is_finite()
                    && layer.x >= 0.
                    && layer.y >= 0.
                    && layer.w >= 0.
                    && layer.h >= 0.
                    && layer.x + layer.w <= d.width as f32
                    && layer.y + layer.h <= d.height as f32
            }), "{} has a layer outside its native canvas", spec.key);
            let purpose_field = match spec.purpose {
                Purpose::StationId => "{{set.showident.station.name}}",
                Purpose::ComingUp => "{{set.showident.coming.title}}",
                Purpose::Signoff => "{{set.showident.signoff.title}}",
                Purpose::Technical => "{{set.showident.technical.title}}",
            };
            assert!(text_layers(&d).any(|text| text == purpose_field), "{} binds its own purpose copy", spec.key);
        }
        assert_eq!(keys.len(), 8);
    }

    #[test]
    fn sample_copy_is_explicitly_synthetic_and_long_name_is_bound() {
        let mut d = build("set_station_id__portrait").unwrap();
        d.template = Some("set_station_id__portrait".into());
        sample(&mut d);
        assert!(d.data_overrides.iter().any(|(key, value)| {
            key == "set.showident.station.name"
                && value.contains("SAMPLE")
                && value.contains("Jeffersonville")
        }));
        assert!(d.data_overrides.iter().all(|(_, value)| value.contains("SAMPLE")));
    }

    #[test]
    fn binding_uses_station_settings_and_safe_manual_defaults() {
        let settings = Settings::default();
        let mut b = Bindings::default();
        bind(&mut b, &settings);
        assert_eq!(b.apply("{{set.showident.station.name}}"), settings.station_name);
        assert_eq!(
            b.apply("{{set.showident.coming.title}}"),
            "Next segment not entered yet"
        );
        assert!(b
            .apply("{{set.showident.technical.message}}")
            .contains("Thank you for your patience"));
    }
}
