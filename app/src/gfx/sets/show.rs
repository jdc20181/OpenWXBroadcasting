//! Editable station packaging. No weather facts or live-stream status are inferred.
use super::reference::{frame, label, lowerthird_frame, social_frame, MISSING};
use crate::gfx::{
    bind::Bindings,
    model::{Anim, AnimKind, Fill, GraphicDoc, Layer},
    text::Align,
};
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Open,
    Headlines,
    Live,
    Thumbnail,
    Promo,
    Follow,
    Bumper,
    Stinger,
}
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    kind: Kind,
    format: u8,
}
const SPECS: &[Spec] = &[
    Spec {
        key: "set_segment_bumper__transition",
        kind: Kind::Bumper,
        format: 4,
    },
    Spec {
        key: "set_stinger_transition",
        kind: Kind::Stinger,
        format: 4,
    },
    Spec {
        key: "set_segment_bumper",
        kind: Kind::Bumper,
        format: 0,
    },
    Spec {
        key: "set_station_promo",
        kind: Kind::Promo,
        format: 0,
    },
    Spec {
        key: "set_station_promo__portrait",
        kind: Kind::Promo,
        format: 1,
    },
    Spec {
        key: "set_station_promo__square",
        kind: Kind::Promo,
        format: 2,
    },
    Spec {
        key: "set_station_promo__lowerthird",
        kind: Kind::Promo,
        format: 3,
    },
    Spec {
        key: "set_follow_us",
        kind: Kind::Follow,
        format: 0,
    },
    Spec {
        key: "set_follow_us__portrait",
        kind: Kind::Follow,
        format: 1,
    },
    Spec {
        key: "set_follow_us__square",
        kind: Kind::Follow,
        format: 2,
    },
    Spec {
        key: "set_follow_us__lowerthird",
        kind: Kind::Follow,
        format: 3,
    },
    Spec {
        key: "set_show_open",
        kind: Kind::Open,
        format: 0,
    },
    Spec {
        key: "set_show_headlines",
        kind: Kind::Headlines,
        format: 0,
    },
    Spec {
        key: "set_show_headlines__portrait",
        kind: Kind::Headlines,
        format: 1,
    },
    Spec {
        key: "set_show_headlines__square",
        kind: Kind::Headlines,
        format: 2,
    },
    Spec {
        key: "set_show_headlines__lowerthird",
        kind: Kind::Headlines,
        format: 3,
    },
    Spec {
        key: "set_live_now",
        kind: Kind::Live,
        format: 0,
    },
    Spec {
        key: "set_live_now__portrait",
        kind: Kind::Live,
        format: 1,
    },
    Spec {
        key: "set_live_now__square",
        kind: Kind::Live,
        format: 2,
    },
    Spec {
        key: "set_live_now__lowerthird",
        kind: Kind::Live,
        format: 3,
    },
    Spec {
        key: "set_video_thumbnail",
        kind: Kind::Thumbnail,
        format: 0,
    },
];
fn token(f: &str) -> String {
    format!("{{{{set.show.{f}}}}}")
}
fn hero_fields(kind: Kind) -> (&'static str, &'static str, &'static str) {
    match kind {
        Kind::Open => ("open.cue", "open.title", "open.detail"),
        Kind::Thumbnail => ("thumbnail.cue", "thumbnail.title", "thumbnail.detail"),
        Kind::Promo => ("promo.cue", "promo.title", "promo.destination"),
        Kind::Bumper => ("bumper.cue", "bumper.title", "bumper.detail"),
        Kind::Stinger => ("stinger.cue", "stinger.title", "stinger.detail"),
        _ => ("live.cue", "live.topic", "live.destination"),
    }
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|s| s.key == key)?;
    let title = match s.kind {
        Kind::Open => "Your Local Weather",
        Kind::Headlines => "Weather Headlines",
        Kind::Live => "Live Weather",
        Kind::Thumbnail => "Weather Update",
        Kind::Promo => "Your Local Weather",
        Kind::Follow => "Follow Our Weather",
        Kind::Bumper => "Coming Up",
        Kind::Stinger => "Local Weather",
    };
    let subtitle = if s.kind == Kind::Open {
        "{{weekday}}".into()
    } else {
        token("edition")
    };
    let source = if s.kind == Kind::Headlines {
        token("headlines.source")
    } else {
        token("source")
    };
    if s.format == 4 {
        return Some(transition(s, title, &subtitle, &source));
    }
    let mut d = match s.format {
        1 => social_frame(title, "Station", &subtitle, &source, true),
        2 => social_frame(title, "Station", &subtitle, &source, false),
        3 => lowerthird_frame(title, "Station", &subtitle, &source),
        _ => frame(title, "Station", &subtitle, &source),
    };
    if s.format == 3 {
        d.layers.push(
            Layer::rect(
                "Show content band",
                0.,
                865.,
                1920.,
                131.,
                Fill::solid("$panel"),
                0.,
            )
            .with_anim(AnimKind::Rise, 0.15, 0.4),
        );
    }
    if s.kind == Kind::Headlines || s.kind == Kind::Follow {
        for i in 0..3 {
            let (x, y, w, h) = match s.format {
                1 => (54., 330. + i as f32 * 430., 972., 380.),
                2 => (54., 238. + i as f32 * 232., 972., 204.),
                3 => (80. + i as f32 * 600., 872., 560., 116.),
                _ => (80., 250. + i as f32 * 225., 1760., 200.),
            };
            if s.format != 3 {
                d.layers.push(
                    crate::gfx::templates::panel(&format!("Headline card {i}"), x, y, w, h)
                        .with_anim(AnimKind::Rise, 0.15 + i as f32 * 0.15, 0.45),
                );
            }
            let inset = if s.format == 3 { 0. } else { 24. };
            label(
                &mut d,
                &format!("Headline number {i}"),
                (x + inset, y + 12., 75., 42.),
                &format!("{:02}", i + 1),
                if s.format == 3 { 25. } else { 30. },
                "$primary",
                Align::Left,
            );
            let (tx, ty, tw, th, sz) = match s.format {
                1 => (x + 24., y + 80., w - 48., 130., 49.),
                2 => (x + 105., y + 16., w - 129., 85., 36.),
                3 => (x + 84., y + 8., w - 84., 100., 31.),
                _ => (x + 120., y + 18., w - 152., 80., 49.),
            };
            label(
                &mut d,
                &format!("Headline {i}"),
                (tx, ty, tw, th),
                &token(&if s.kind == Kind::Follow {
                    format!(
                        "follow.{i}.{}",
                        if s.format == 3 { "handle" } else { "platform" }
                    )
                } else {
                    format!("headlines.{i}.title")
                }),
                sz,
                "$text",
                Align::Left,
            );
            if s.format != 3 {
                let (dy, dh, ds) = match s.format {
                    1 => (y + 230., 120., 34.),
                    2 => (y + 110., 78., 27.),
                    _ => (y + 108., 70., 32.),
                };
                label(
                    &mut d,
                    &format!("Headline detail {i}"),
                    (tx, dy, tw, dh),
                    &token(&if s.kind == Kind::Follow {
                        format!("follow.{i}.handle")
                    } else {
                        format!("headlines.{i}.detail")
                    }),
                    ds,
                    "$muted",
                    Align::Left,
                );
            }
        }
        if s.format != 3 {
            label(
                &mut d,
                "Headline editorial note",
                (
                    if s.format == 0 { 80. } else { 54. },
                    if s.format == 1 { 1660. } else { 925. },
                    if s.format == 0 { 1760. } else { 972. },
                    42.,
                ),
                &token(if s.kind == Kind::Follow {
                    "follow.note"
                } else {
                    "headlines.note"
                }),
                24.,
                "$muted",
                Align::Left,
            );
        }
    } else if s.format == 3 {
        let (cue, title, detail) = hero_fields(s.kind);
        label(
            &mut d,
            "Live cue",
            (80., 875., 280., 98.),
            &token(cue),
            39.,
            "$primary",
            Align::Left,
        );
        label(
            &mut d,
            "Live topic",
            (400., 875., 1050., 98.),
            &token(title),
            39.,
            "$text",
            Align::Left,
        );
        label(
            &mut d,
            "Live destination",
            (1490., 875., 350., 98.),
            &token(detail),
            26.,
            "$muted",
            Align::Left,
        );
    } else {
        let (x, w, cy, ch) = match s.format {
            1 => (54., 972., 340., 1250.),
            2 => (54., 972., 270., 610.),
            _ => (80., 1760., 280., 570.),
        };
        d.layers.push(
            crate::gfx::templates::panel("Show hero panel", x, cy, w, ch).with_anim(
                AnimKind::Reveal,
                0.12,
                0.65,
            ),
        );
        d.layers.push(
            Layer::rect(
                "Show hero accent",
                x,
                cy,
                8.,
                ch,
                Fill::solid("$primary"),
                0.,
            )
            .with_anim(AnimKind::Grow, 0.35, 0.6),
        );
        let (cue, title, detail) = hero_fields(s.kind);
        label(
            &mut d,
            "Show cue",
            (x + 40., cy + 40., w - 80., 70.),
            &token(cue),
            if s.format == 1 { 43. } else { 37. },
            "$primary",
            Align::Left,
        );
        label(
            &mut d,
            "Show main headline",
            (
                x + 40.,
                cy + if s.format == 1 { 190. } else { 150. },
                w - 80.,
                if s.format == 1 { 470. } else { 230. },
            ),
            &token(title),
            if s.format == 1 {
                102.
            } else if s.format == 2 {
                78.
            } else {
                110.
            },
            "$text",
            Align::Left,
        );
        label(
            &mut d,
            "Show supporting line",
            (
                x + 40.,
                cy + if s.format == 1 { 740. } else { 410. },
                w - 80.,
                if s.format == 1 { 320. } else { 120. },
            ),
            &token(detail),
            if s.format == 1 { 45. } else { 35. },
            "$muted",
            Align::Left,
        );
        if s.kind == Kind::Open {
            label(
                &mut d,
                "Show date",
                (80., 905., 1760., 55.),
                "{{weekday}} • {{date}}",
                32.,
                "$muted",
                Align::Left,
            );
        }
    }
    Some(d)
}
fn transition(s: Spec, title: &str, subtitle: &str, source: &str) -> GraphicDoc {
    let mut d = frame(title, "Station", subtitle, source);
    d.background = None;
    d.duration = 1.6;
    let mut panes = Vec::new();
    for (i, (x, w, color)) in [
        (0., 720., "$secondary"),
        (720., 640., "$primary"),
        (1360., 560., "$panel"),
    ]
    .into_iter()
    .enumerate()
    {
        panes.push(
            Layer::rect(
                &format!("Transition pane {i}"),
                x,
                0.,
                w,
                1080.,
                Fill::solid(color),
                0.,
            )
            .with_anim(AnimKind::Grow, i as f32 * 0.08, 0.32),
        );
    }
    d.layers.splice(0..0, panes);
    d.layers.push(
        Layer::rect(
            "Transition message panel",
            160.,
            300.,
            1600.,
            470.,
            Fill::solid("$panel"),
            0.,
        )
        .with_anim(AnimKind::Wipe, 0.28, 0.25),
    );
    let (cue, headline, detail) = hero_fields(s.kind);
    for (name, field, box_, size, color) in [
        (
            "Transition cue",
            cue,
            (220., 330., 1480., 60.),
            35.,
            "$primary",
        ),
        (
            "Transition headline",
            headline,
            (220., 408., 1480., 180.),
            102.,
            "$text",
        ),
        (
            "Transition supporting text",
            detail,
            (220., 630., 1480., 80.),
            32.,
            "$muted",
        ),
    ] {
        label(&mut d, name, box_, &token(field), size, color, Align::Left);
    }
    for (i, layer) in d.layers.iter_mut().enumerate() {
        if i >= 3 {
            layer.anim_in = Anim::new(AnimKind::Wipe, 0.28, 0.25);
        }
        layer.anim_out = Anim::new(AnimKind::Wipe, 0., 0.30);
    }
    d
}

pub(super) fn bind(b: &mut Bindings) {
    for (f, v) in [
        ("source", "Station production"),
        ("edition", "Local weather"),
        ("open.cue", "YOUR WEATHER"),
        ("open.title", "Local weather. Your community."),
        ("open.detail", "Forecasts • Updates • Perspective"),
        ("live.cue", "LIVE NOW"),
        ("live.topic", MISSING),
        ("live.destination", ""),
        ("thumbnail.cue", "WEATHER UPDATE"),
        ("thumbnail.title", MISSING),
        ("thumbnail.detail", ""),
        ("headlines.source", "Source: Forecaster headlines"),
        ("headlines.note", ""),
        ("promo.cue", "LOCAL WEATHER"),
        ("promo.title", "Weather for our community."),
        ("promo.destination", ""),
        ("bumper.cue", "NEXT"),
        ("bumper.title", MISSING),
        ("bumper.detail", ""),
        ("follow.note", ""),
        ("stinger.cue", "LOCAL WEATHER"),
        ("stinger.title", "Your weather. Your community."),
        ("stinger.detail", "Forecasts • Updates • Perspective"),
    ] {
        b.set(&format!("set.show.{f}"), v);
    }
    // Manual fields are intentionally unavailable until the producer fills them in.
    for i in 0..3 {
        b.set(
            &format!("set.show.headlines.{i}.title"),
            if i == 0 { MISSING } else { "" },
        );
        b.set(&format!("set.show.headlines.{i}.detail"), "");
        b.set(
            &format!("set.show.follow.{i}.platform"),
            if i == 0 { "Social updates" } else { "" },
        );
        b.set(
            &format!("set.show.follow.{i}.handle"),
            if i == 0 { MISSING } else { "" },
        );
    }
}
#[cfg(test)]
pub(super) fn sample(d: &mut GraphicDoc) {
    if !SPECS.iter().any(|s| Some(s.key) == d.template.as_deref()) {
        return;
    }
    for (f, v) in [
        ("source", "SAMPLE ONLY • Not for broadcast"),
        ("edition", "SAMPLE • Local weather"),
        ("open.cue", "SAMPLE • YOUR WEATHER"),
        ("open.title", "Your weather. Your community."),
        ("live.cue", "LIVE NOW • SAMPLE"),
        ("live.topic", "Your local weather update"),
        ("live.destination", "Join the conversation"),
        ("thumbnail.title", "Your next weather change"),
        ("thumbnail.detail", "SAMPLE • A forecast conversation"),
        ("promo.title", "Your Jeffersonville weather"),
        ("promo.destination", "SAMPLE • Station website / channel"),
        ("bumper.title", "Your Jeffersonville forecast"),
        (
            "bumper.detail",
            "SAMPLE • Local forecast and community updates",
        ),
        ("follow.0.platform", "SAMPLE • Website"),
        ("follow.0.handle", "SAMPLE • Station weather website"),
        ("follow.1.platform", "SAMPLE • Social"),
        ("follow.1.handle", "SAMPLE • Station social handle"),
        ("follow.2.platform", "SAMPLE • Video"),
        ("follow.2.handle", "SAMPLE • Station video channel"),
        ("headlines.source", "SAMPLE ONLY • Not for broadcast"),
        (
            "headlines.note",
            "SAMPLE • Synthetic editorial copy for layout review",
        ),
        ("headlines.0.title", "Rain chances return tomorrow"),
        (
            "headlines.0.detail",
            "SAMPLE • Timing for Jeffersonville and surrounding communities",
        ),
        ("headlines.1.title", "A cooler start to the weekend"),
        (
            "headlines.1.detail",
            "SAMPLE • What to expect before outdoor plans",
        ),
        ("headlines.2.title", "Looking ahead to next week"),
        (
            "headlines.2.detail",
            "SAMPLE • The next forecast update and your local outlook",
        ),
    ] {
        d.data_overrides.push((format!("set.show.{f}"), v.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transitions_clear_cover_and_clear_again_in_both_looks() {
        use crate::gfx::render::{GraphicsRenderer, RenderCtx};
        let settings = crate::config::Settings::default();
        let data = crate::data::WxSnapshot::default();
        for key in ["set_segment_bumper__transition", "set_stinger_transition"] {
            for look in ["modern", "classic"] {
                let mut d = crate::gfx::templates::build_in(key, look).unwrap();
                sample(&mut d);
                assert!(d.background.is_none());
                assert_eq!(d.duration, 1.6);
                let mut bindings = Bindings::build(&data, &settings, None, &settings.radar_palette);
                for (key, value) in &d.data_overrides {
                    let value = bindings.apply(value);
                    bindings.set(key, &value);
                }
                let mut renderer = GraphicsRenderer::new();
                for (phase, time, out_time, expected) in [
                    ("00-clear", 0., None, 0),
                    ("01-enter", 0.16, None, 1),
                    ("02-cover", 0.9, None, 2),
                    ("03-exit", 0.9, Some(0.15), 1),
                    ("04-clear", 0.9, Some(0.4), 0),
                ] {
                    let mut pm = tiny_skia::Pixmap::new(1920, 1080).unwrap();
                    renderer.render(
                        &mut pm,
                        &d,
                        &RenderCtx {
                            bindings: &bindings,
                            brand: &settings.brand,
                            time,
                            out_time,
                            media_dir: std::path::Path::new("."),
                        },
                    );
                    let pixels = pm.data().chunks_exact(4).filter(|p| p[3] != 0).count();
                    let full = 1920 * 1080;
                    match expected {
                        0 => assert_eq!(pixels, 0, "{key} {look} {phase}"),
                        2 => assert_eq!(pixels, full, "{key} {look} {phase}"),
                        _ => assert!(
                            pixels > 0 && pixels < full,
                            "{key} {look} {phase}: {pixels}"
                        ),
                    }
                    let include = std::env::var("CWX_QA_SET_KEYS")
                        .ok()
                        .is_none_or(|keys| keys.split(',').any(|k| k.trim() == key));
                    if include {
                        if let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR") {
                            let dir = std::path::PathBuf::from(dir).join("transition-sequence");
                            std::fs::create_dir_all(&dir).unwrap();
                            pm.save_png(dir.join(format!("{key}-{look}-{phase}.png")))
                                .unwrap();
                        }
                    }
                }
            }
        }
    }
}
