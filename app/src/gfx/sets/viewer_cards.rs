//! Native portrait and square variants for approved viewer cards, plus manual spotter cards.
//! This module formats existing bindings; it does not fetch, approve, or infer submissions.

use super::reference::{frame, label, lowerthird_frame, social_frame};
use crate::gfx::{
    bind::Bindings,
    chart::ChartKind,
    model::{AnimKind, GraphicDoc, ImageFit, Layer, LayerKind},
    templates::{panel, text, DISPLAY},
    text::{Align, Weight},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Photo,
    Wall,
    Poll,
    Question,
    Spotter,
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
        key: "set_viewer_photo__portrait",
        kind: Kind::Photo,
        format: Format::Portrait,
    },
    Spec {
        key: "set_viewer_photo__square",
        kind: Kind::Photo,
        format: Format::Square,
    },
    Spec {
        key: "set_viewer_wall__portrait",
        kind: Kind::Wall,
        format: Format::Portrait,
    },
    Spec {
        key: "set_viewer_wall__square",
        kind: Kind::Wall,
        format: Format::Square,
    },
    Spec {
        key: "set_viewer_poll__portrait",
        kind: Kind::Poll,
        format: Format::Portrait,
    },
    Spec {
        key: "set_viewer_poll__square",
        kind: Kind::Poll,
        format: Format::Square,
    },
    Spec {
        key: "set_viewer_question__portrait",
        kind: Kind::Question,
        format: Format::Portrait,
    },
    Spec {
        key: "set_viewer_question__square",
        kind: Kind::Question,
        format: Format::Square,
    },
    Spec {
        key: "set_spotter_week",
        kind: Kind::Spotter,
        format: Format::Wide,
    },
    Spec {
        key: "set_spotter_week__portrait",
        kind: Kind::Spotter,
        format: Format::Portrait,
    },
    Spec {
        key: "set_spotter_week__square",
        kind: Kind::Spotter,
        format: Format::Square,
    },
    Spec {
        key: "set_spotter_week__lowerthird",
        kind: Kind::Spotter,
        format: Format::LowerThird,
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
        *vcenter = false;
    }
    layer
}

fn image(d: &mut GraphicDoc, name: &str, media: &str, rect: (f32, f32, f32, f32)) {
    d.layers.push(panel(
        &format!("{name} image well"),
        rect.0,
        rect.1,
        rect.2,
        rect.3,
    ));
    d.layers.push(
        Layer::new(
            name,
            rect.0 + 10.,
            rect.1 + 10.,
            rect.2 - 20.,
            rect.3 - 20.,
            LayerKind::Image {
                media: media.into(),
                fit: ImageFit::Contain,
            },
        )
        .with_anim(AnimKind::Fade, 0.24, 0.5),
    );
}

fn native_frame(title: &str, subtitle: &str, format: Format) -> GraphicDoc {
    match format {
        Format::Portrait => social_frame(title, "Community", subtitle, "Station community", true),
        Format::Square => social_frame(title, "Community", subtitle, "Station community", false),
        Format::LowerThird => lowerthird_frame(title, "Community", subtitle, "Station production"),
        Format::Wide => frame(title, "Community", subtitle, "Station production"),
    }
}

fn photo(format: Format) -> GraphicDoc {
    let portrait = format == Format::Portrait;
    let mut d = native_frame("Viewer Photo", "{{station}}", format);
    let (well, credit_y, credit_h, credit_size, caption_y, caption_h, caption_size) = if portrait {
        ((54., 232., 972., 1000.), 1260., 120., 36., 1390., 270., 34.)
    } else {
        ((54., 190., 972., 520.), 730., 92., 33., 830., 120., 29.)
    };
    image(&mut d, "Approved viewer photo", "{{viewer1.photo}}", well);
    label(
        &mut d,
        "Viewer photo availability",
        (well.0 + 22., well.1 + 22., well.2 - 44., 64.),
        "{{set.viewer.photo.status}}",
        if portrait { 31. } else { 28. },
        "$muted",
        Align::Center,
    );
    d.layers
        .push(panel("Photo credit panel", 54., credit_y, 972., credit_h));
    d.layers.push(txt(
        "Photo credit",
        (78., credit_y + 10., 924., credit_h * 0.42),
        "{{set.viewer.photo.credit}}",
        credit_size,
        "$text",
        Align::Left,
    ));
    d.layers.push(txt(
        "Photo caption",
        (78., caption_y, 924., caption_h),
        "{{set.viewer.photo.caption}}",
        caption_size,
        "$muted",
        Align::Left,
    ));
    d
}

fn wall(format: Format) -> GraphicDoc {
    let portrait = format == Format::Portrait;
    let mut d = native_frame("Viewer Photo Wall", "Approved viewer photos", format);
    let (start_y, tile_w, tile_h, image_h, credit_h, cols) = if portrait {
        (230., 472., 700., 530., 120., 2)
    } else {
        (185., 472., 350., 238., 82., 2)
    };
    let gap_x = 28.;
    let gap_y = if portrait { 28. } else { 16. };
    let x0 = (1080. - (tile_w * cols as f32 + gap_x * (cols as f32 - 1.))) / 2.;
    for i in 0..4 {
        let row = i / 2;
        let col = i % 2;
        let x = x0 + col as f32 * (tile_w + gap_x);
        let y = start_y + row as f32 * (tile_h + gap_y);
        d.layers.push(panel(
            &format!("Viewer photo tile {}", i + 1),
            x,
            y,
            tile_w,
            tile_h,
        ));
        image(
            &mut d,
            &format!("Approved viewer photo {}", i + 1),
            &format!("{{{{viewer{}.photo}}}}", i + 1),
            (x + 14., y + 14., tile_w - 28., image_h),
        );
        d.layers.push(txt(
            &format!("Viewer photo slot status {}", i + 1),
            (x + 28., y + 28., tile_w - 56., 58.),
            &format!("{{{{set.viewer.wall.empty{}}}}}", i + 1),
            if portrait { 23. } else { 19. },
            "$muted",
            Align::Center,
        ));
        let credit_y = y + image_h + 20.;
        d.layers.push(txt(
            &format!("Viewer photo credit {}", i + 1),
            (x + 22., credit_y, tile_w - 44., credit_h),
            &format!("{{{{set.viewer.wall.credit{}}}}}", i + 1),
            if portrait { 25. } else { 22. },
            "$text",
            Align::Left,
        ));
    }
    let footer_y = if portrait { 1660. } else { 910. };
    label(
        &mut d,
        "Photo wall availability",
        (54., footer_y, 972., 60.),
        "{{set.viewer.wall.status}}",
        25.,
        "$muted",
        Align::Center,
    );
    d
}

fn poll(format: Format) -> GraphicDoc {
    let portrait = format == Format::Portrait;
    let mut d = native_frame("Viewer Poll", "{{poll.question}}", format);
    let chart_rect = if portrait {
        (78., 360., 924., 760.)
    } else {
        (74., 230., 932., 410.)
    };
    d.layers.push(panel(
        "Poll results panel",
        chart_rect.0,
        chart_rect.1,
        chart_rect.2,
        chart_rect.3,
    ));
    d.layers.push(
        Layer::new(
            "Poll results",
            chart_rect.0 + 20.,
            chart_rect.1 + 18.,
            chart_rect.2 - 40.,
            chart_rect.3 - 36.,
            LayerKind::Chart {
                chart: ChartKind::Bar,
                series: "set.viewer.poll.numbered".into(),
                series2: String::new(),
                color: "$primary".into(),
                color2: "#FFFFFF".into(),
                text_color: "$text".into(),
                show_values: true,
                min: Some(0.),
                max: Some(100.),
                unit: "%".into(),
                highlight: String::new(),
                note: "{{poll.total}}".into(),
            },
        )
        .with_anim(AnimKind::Fade, 0.25, 0.5),
    );
    d.layers.push(txt(
        "Poll option legend",
        if portrait {
            (94., 1150., 892., 360.)
        } else {
            (94., 662., 892., 180.)
        },
        "{{set.viewer.poll.options}}",
        if portrait { 34. } else { 27. },
        "$text",
        Align::Left,
    ));
    d.layers.push(txt(
        "Poll vote total",
        if portrait {
            (94., 1690., 892., 54.)
        } else {
            (94., 850., 892., 32.)
        },
        "{{poll.total}}",
        if portrait { 30. } else { 24. },
        "$muted",
        Align::Left,
    ));
    let status_y = if portrait { 1540. } else { 890. };
    let status_h = if portrait { 120. } else { 64. };
    d.layers.push(txt(
        "Poll availability",
        (74., status_y, 932., status_h),
        "{{set.viewer.poll.status}}",
        if portrait { 29. } else { 25. },
        "$muted",
        Align::Center,
    ));
    d
}

fn question(format: Format) -> GraphicDoc {
    let portrait = format == Format::Portrait;
    let mut d = native_frame("Viewer Question", "{{set.viewer.question.from}}", format);
    let box_ = if portrait {
        (54., 325., 972., 1120.)
    } else {
        (54., 230., 972., 620.)
    };
    d.layers.push(panel(
        "Viewer question panel",
        box_.0,
        box_.1,
        box_.2,
        box_.3,
    ));
    label(
        &mut d,
        "Viewer question label",
        (box_.0 + 34., box_.1 + 36., box_.2 - 68., 84.),
        "QUESTION",
        if portrait { 38. } else { 30. },
        "$primary",
        Align::Left,
    );
    d.layers.push(txt(
        "Viewer question",
        (box_.0 + 44., box_.1 + 155., box_.2 - 88., box_.3 - 300.),
        "{{set.viewer.question.text}}",
        if portrait { 56. } else { 46. },
        "$text",
        Align::Left,
    ));
    let cue_y = box_.1 + box_.3 - if portrait { 120. } else { 100. };
    d.layers.push(txt(
        "Question prompt",
        (box_.0 + 36., cue_y, box_.2 - 72., 56.),
        "Send a weather question to the station",
        if portrait { 26. } else { 23. },
        "$muted",
        Align::Center,
    ));
    d
}

fn spotter(format: Format) -> GraphicDoc {
    let portrait = format == Format::Portrait;
    let mut d = match format {
        Format::Wide => frame(
            "Weather Spotter of the Week",
            "Community",
            "Manual recognition",
            "Station production",
        ),
        _ => native_frame("Weather Spotter of the Week", "Manual recognition", format),
    };
    if format == Format::LowerThird {
        d.layers
            .push(panel("Spotter recognition panel", 80., 870., 1760., 110.));
        d.layers.push(txt(
            "Spotter name",
            (104., 884., 620., 72.),
            "{{set.viewer.spotter.name}}",
            36.,
            "$text",
            Align::Left,
        ));
        d.layers.push(txt(
            "Spotter recognition copy",
            (760., 884., 1050., 72.),
            "{{set.viewer.spotter.note}}",
            27.,
            "$muted",
            Align::Right,
        ));
        return d;
    }
    let rect = match format {
        Format::Wide => (100., 210., 1720., 650.),
        Format::Portrait => (54., 330., 972., 1080.),
        Format::Square => (54., 220., 972., 610.),
        Format::LowerThird => unreachable!("handled above"),
    };
    d.layers.push(panel(
        "Spotter recognition panel",
        rect.0,
        rect.1,
        rect.2,
        rect.3,
    ));
    let name_y = rect.1 + if portrait { 180. } else { 105. };
    d.layers.push(txt(
        "Spotter name",
        (rect.0 + 36., name_y, rect.2 - 72., 140.),
        "{{set.viewer.spotter.name}}",
        if portrait { 68. } else { 62. },
        "$text",
        Align::Center,
    ));
    let place_y = name_y + if portrait { 170. } else { 150. };
    d.layers.push(txt(
        "Spotter place",
        (rect.0 + 42., place_y, rect.2 - 84., 88.),
        "{{set.viewer.spotter.place}}",
        if portrait { 36. } else { 30. },
        "$primary",
        Align::Center,
    ));
    let note_y = place_y + if portrait { 155. } else { 115. };
    d.layers.push(txt(
        "Spotter recognition copy",
        (rect.0 + 56., note_y, rect.2 - 112., 130.),
        "{{set.viewer.spotter.note}}",
        if portrait { 34. } else { 30. },
        "$muted",
        Align::Center,
    ));
    d
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = *SPECS.iter().find(|s| s.key == key)?;
    let mut d = match spec.kind {
        Kind::Photo => photo(spec.format),
        Kind::Wall => wall(spec.format),
        Kind::Poll => poll(spec.format),
        Kind::Question => question(spec.format),
        Kind::Spotter => spotter(spec.format),
    };
    d.template = Some(key.into());
    Some(d)
}

/// Called after core viewer bindings are populated. Never replaces their values or series.
pub(super) fn bind(b: &mut Bindings) {
    fn has_photo(b: &Bindings, i: usize) -> bool {
        b.get(&format!("viewer{i}.photo"))
            .map(|v| !v.trim().is_empty() && v != "--")
            .unwrap_or(false)
    }
    b.set(
        "set.viewer.photo.status",
        if has_photo(b, 1) {
            ""
        } else {
            "Approved viewer photo not available yet"
        },
    );
    let photo_credit = if has_photo(b, 1) {
        b.get("viewer1.credit").unwrap_or("").to_owned()
    } else {
        "".to_owned()
    };
    let photo_caption = if has_photo(b, 1) {
        b.get("viewer1.caption").unwrap_or("").to_owned()
    } else {
        "".to_owned()
    };
    b.set("set.viewer.photo.credit", &photo_credit);
    b.set("set.viewer.photo.caption", &photo_caption);
    let photo_count = (1..=4).filter(|i| has_photo(b, *i)).count();
    b.set(
        "set.viewer.wall.status",
        if photo_count == 0 {
            "No approved viewer photos available yet"
        } else {
            "Approved viewer submissions"
        },
    );
    for i in 1..=4 {
        let has = has_photo(b, i);
        let credit = if has {
            b.get(&format!("viewer{i}.credit")).unwrap_or("").to_owned()
        } else {
            "No approved viewer photo available yet".to_owned()
        };
        b.set(&format!("set.viewer.wall.credit{i}"), &credit);
        b.set(
            &format!("set.viewer.wall.empty{i}"),
            if has {
                ""
            } else {
                "Approved photo not available yet"
            },
        );
    }
    let has_poll = b
        .series("poll.results")
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let points = b.series("poll.results").unwrap_or(&[]).to_vec();
    let options = points
        .iter()
        .enumerate()
        .map(|(i, (name, _))| format!("{}. {}", i + 1, name))
        .collect::<Vec<_>>()
        .join("\n");
    b.set("set.viewer.poll.options", &options);
    b.set_series(
        "set.viewer.poll.numbered",
        points
            .iter()
            .enumerate()
            .map(|(i, (_, value))| ((i + 1).to_string(), *value))
            .collect(),
    );
    b.set(
        "set.viewer.poll.status",
        if has_poll {
            ""
        } else {
            "No poll results available yet"
        },
    );
    let has_question = b
        .get("question.from")
        .map(|v| v != "Viewer questions" && !v.trim().is_empty())
        .unwrap_or(false);
    if has_question {
        let from = b
            .get("question.from")
            .unwrap_or("Viewer question")
            .to_owned();
        let text = b
            .get("question.text")
            .unwrap_or("No viewer question available yet")
            .to_owned();
        b.set("set.viewer.question.from", &from);
        b.set("set.viewer.question.text", &text);
    } else {
        b.set("set.viewer.question.from", "Viewer Q&A");
        b.set(
            "set.viewer.question.text",
            "No on-air viewer question available yet",
        );
    }
    for (field, value) in [
        ("name", "Spotter name not entered"),
        ("place", "Place not entered"),
        ("note", "Recognition copy not entered"),
    ] {
        let key = format!("set.viewer.spotter.{field}");
        if b.get(&key).map(str::trim).unwrap_or("").is_empty() {
            b.set(&key, value);
        }
    }
}

#[cfg(test)]
pub(super) fn sample(d: &mut GraphicDoc) {
    let Some(spec) = SPECS.iter().find(|s| Some(s.key) == d.template.as_deref()) else {
        return;
    };
    let mut put = |key: &str, value: &str| d.data_overrides.push((key.into(), value.into()));
    match spec.kind {
        Kind::Photo => {
            put("set.viewer.photo.status", "");
            put(
                "set.viewer.photo.credit",
                "SAMPLE • Approved viewer submission, Jeffersonville, IN",
            );
            put(
                "set.viewer.photo.caption",
                "SAMPLE ONLY • Synthetic photo fixture • Not for broadcast",
            );
            put("viewer1.photo", "sets-qa-image.png");
            put(
                "viewer1.credit",
                "SAMPLE • Approved viewer submission, Jeffersonville, IN",
            );
            put(
                "viewer1.caption",
                "SAMPLE ONLY • Synthetic photo fixture • Not for broadcast",
            );
        }
        Kind::Wall => {
            for i in 1..=4 {
                put(&format!("viewer{i}.photo"), "sets-qa-image.png");
                put(
                    &format!("set.viewer.wall.credit{i}"),
                    &format!("SAMPLE • Approved viewer photo {i}, Jeffersonville, IN"),
                );
                put(&format!("set.viewer.wall.empty{i}"), "");
                put(
                    &format!("viewer{i}.credit"),
                    &format!("SAMPLE • Approved viewer photo {i}, Jeffersonville, IN"),
                );
                put(
                    &format!("viewer{i}.caption"),
                    &format!("SAMPLE • Synthetic caption {i}"),
                );
            }
            put(
                "set.viewer.wall.status",
                "SAMPLE ONLY • Synthetic approved-photo fixtures",
            );
        }
        Kind::Poll => {
            put(
                "set.viewer.poll.options",
                "1. Forecast updates\n2. Weather explainers\n3. Community events",
            );
            put(
                "poll.question",
                "SAMPLE • Which local topic should we cover next?",
            );
            put(
                "poll.total",
                "SAMPLE ONLY • 100 synthetic votes • Not for broadcast",
            );
            put(
                "set.viewer.poll.status",
                "SAMPLE ONLY • Synthetic poll results • Not for broadcast",
            );
        }
        Kind::Question => {
            put(
                "set.viewer.question.text",
                "SAMPLE ONLY • What should I keep in my severe weather kit?",
            );
            put(
                "set.viewer.question.from",
                "SAMPLE • A Jeffersonville viewer",
            );
        }
        Kind::Spotter => {
            put("set.viewer.spotter.name", "SAMPLE • Alex Morgan");
            put("set.viewer.spotter.place", "SAMPLE • Clark County, Indiana");
            put(
                "set.viewer.spotter.note",
                "SAMPLE ONLY • Synthetic recognition copy • Not for broadcast",
            );
        }
    }
    if spec.kind == Kind::Poll {
        // Test-only inline points use the existing chart renderer's fixture syntax.
        // The live template continues to read the supplied poll.results series.
        for layer in &mut d.layers {
            if let LayerKind::Chart { series, .. } = &mut layer.kind {
                *series = "1:45,2:35,3:20".into();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_twelve_keys_have_native_geometry_and_bounded_layers() {
        let mut keys = HashSet::new();
        assert_eq!(SPECS.len(), 12);
        for spec in SPECS {
            assert!(spec.key.starts_with("set_"));
            assert!(keys.insert(spec.key), "duplicate key {}", spec.key);
            let d = build(spec.key).expect("spec builds");
            assert_eq!(
                d.width,
                if spec.format == Format::Wide || spec.format == Format::LowerThird {
                    1920
                } else {
                    1080
                }
            );
            assert_eq!(
                d.height,
                if spec.format == Format::Portrait {
                    1920
                } else {
                    1080
                }
            );
            assert!(d.layers.len() <= 80, "{} has too many layers", spec.key);
            assert!(
                d.layers.iter().all(|l| {
                    l.x.is_finite()
                        && l.y.is_finite()
                        && l.w.is_finite()
                        && l.h.is_finite()
                        && l.x >= 0.
                        && l.y >= 0.
                        && l.w >= 0.
                        && l.h >= 0.
                        && l.x + l.w <= d.width as f32
                        && l.y + l.h <= d.height as f32
                }),
                "{} layer outside canvas",
                spec.key
            );
        }
    }

    #[test]
    fn images_contain_and_bind_only_approved_viewer_slots() {
        for key in [
            "set_viewer_photo__portrait",
            "set_viewer_photo__square",
            "set_viewer_wall__portrait",
            "set_viewer_wall__square",
        ] {
            let d = build(key).unwrap();
            let images: Vec<_> = d
                .layers
                .iter()
                .filter_map(|l| match &l.kind {
                    LayerKind::Image { media, fit } => Some((media.as_str(), *fit)),
                    _ => None,
                })
                .collect();
            assert!(!images.is_empty());
            assert!(images
                .iter()
                .all(|(media, fit)| media.starts_with("{{viewer")
                    && media.ends_with(".photo}}")
                    && *fit == ImageFit::Contain));
        }
    }

    #[test]
    fn poll_states_are_unavailable_without_result_series_and_never_invent_zeroes() {
        let mut b = Bindings::default();
        b.set("poll.total", "No poll running yet");
        bind(&mut b);
        assert_eq!(b.get("poll.total"), Some("No poll running yet"));
        assert_eq!(
            b.get("set.viewer.poll.status"),
            Some("No poll results available yet")
        );
        let d = build("set_viewer_poll__square").unwrap();
        assert!(d.layers.iter().any(|l| match &l.kind {
            LayerKind::Chart {
                series, min, max, ..
            } => {
                series == "set.viewer.poll.numbered" && *min == Some(0.) && *max == Some(100.)
            }
            _ => false,
        }));
        let mut sample_doc = d;
        sample(&mut sample_doc);
        assert!(sample_doc
            .data_overrides
            .iter()
            .any(|(k, v)| k == "set.viewer.poll.status" && v.contains("SAMPLE ONLY")));
        assert!(!sample_doc
            .data_overrides
            .iter()
            .any(|(_, v)| v == "0" || v == "0%"));
        let fixture = sample_doc
            .layers
            .iter()
            .find_map(|l| match &l.kind {
                LayerKind::Chart { series, .. } => Some(crate::gfx::chart::parse_inline(series)),
                _ => None,
            })
            .unwrap();
        assert_eq!(fixture.len(), 3);
        assert_eq!(fixture.iter().map(|(_, value)| value).sum::<f32>(), 100.);
    }

    #[test]
    fn manual_spotter_defaults_and_explicit_sample_overrides() {
        let mut b = Bindings::default();
        bind(&mut b);
        assert_eq!(
            b.get("set.viewer.spotter.name"),
            Some("Spotter name not entered")
        );
        let mut d = build("set_spotter_week__portrait").unwrap();
        sample(&mut d);
        assert!(d
            .data_overrides
            .iter()
            .all(|(k, v)| k.starts_with("set.viewer.spotter.") && v.contains("SAMPLE")));
    }

    #[test]
    fn numbered_poll_labels_preserve_full_option_text_and_supplied_values() {
        let original = vec![
            ("Jeffersonville community weather updates".to_string(), 45.),
            ("Local weather education and science".to_string(), 35.),
            ("Community events and school information".to_string(), 20.),
        ];
        let mut b = Bindings::default();
        b.set_series("poll.results", original.clone());
        bind(&mut b);
        assert_eq!(b.series("poll.results").unwrap(), original.as_slice());
        let numbered = b.series("set.viewer.poll.numbered").unwrap();
        for (i, ((name, value), (number, shown))) in original.iter().zip(numbered).enumerate() {
            assert_eq!(number, &(i + 1).to_string());
            assert_eq!(shown, value);
            assert!(b.get("set.viewer.poll.options").unwrap().contains(name));
        }
    }
}
