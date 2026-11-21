//! Native portrait and square season countdown cards using the existing season bindings.
//! The core calendar calculation remains authoritative; this module only lays out its text.

use super::reference::{label, social_frame};
use crate::gfx::{bind::Bindings, model::GraphicDoc, text::Align};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Portrait,
    Square,
}

#[derive(Clone, Copy, Debug)]
struct Spec {
    key: &'static str,
    format: Format,
}

const SPECS: &[Spec] = &[
    Spec {
        key: "set_season_countdown__portrait",
        format: Format::Portrait,
    },
    Spec {
        key: "set_season_countdown__square",
        format: Format::Square,
    },
];

fn card(format: Format) -> GraphicDoc {
    let portrait = format == Format::Portrait;
    let mut doc = social_frame(
        "Season Countdown",
        "Seasons",
        "Next season • Calendar countdown",
        "Computed season dates • calendar days remaining",
        portrait,
    );
    if portrait {
        label(
            &mut doc,
            "Countdown label",
            (72., 330., 936., 76.),
            "NEXT SEASON",
            38.,
            "$primary",
            Align::Center,
        );
        label(
            &mut doc,
            "Days remaining",
            (70., 438., 940., 300.),
            "{{set.season.days}}",
            188.,
            "$text",
            Align::Center,
        );
        label(
            &mut doc,
            "Season identity",
            (72., 800., 936., 150.),
            "{{set.season.next}}",
            76.,
            "$primary",
            Align::Center,
        );
        label(
            &mut doc,
            "Calendar note",
            (96., 1020., 888., 110.),
            "Calendar days until the next season",
            36.,
            "$muted",
            Align::Center,
        );
        label(
            &mut doc,
            "Sample disclosure",
            (80., 1538., 920., 54.),
            "{{set.season.sample_note}}",
            26.,
            "$muted",
            Align::Center,
        );
    } else {
        label(
            &mut doc,
            "Countdown label",
            (72., 245., 936., 58.),
            "NEXT SEASON",
            31.,
            "$primary",
            Align::Center,
        );
        label(
            &mut doc,
            "Days remaining",
            (72., 322., 936., 210.),
            "{{set.season.days}}",
            132.,
            "$text",
            Align::Center,
        );
        label(
            &mut doc,
            "Season identity",
            (72., 564., 936., 110.),
            "{{set.season.next}}",
            58.,
            "$primary",
            Align::Center,
        );
        label(
            &mut doc,
            "Calendar note",
            (94., 706., 892., 82.),
            "Calendar days until the next season",
            29.,
            "$muted",
            Align::Center,
        );
        label(
            &mut doc,
            "Sample disclosure",
            (80., 890., 920., 44.),
            "{{set.season.sample_note}}",
            22.,
            "$muted",
            Align::Center,
        );
    }
    doc
}

/// Adds display aliases for the authoritative core season bindings, without changing them.
pub(super) fn bind(bindings: &mut Bindings) {
    let next = bindings.get("season.next").unwrap_or("").trim().to_owned();
    let days = bindings.get("season.days").unwrap_or("").trim().to_owned();
    if !next.is_empty() && next != "--" && !days.is_empty() && days != "--" {
        bindings.set("set.season.next", &next);
        bindings.set("set.season.days", &days);
        bindings.set("set.season.sample_note", "");
    } else {
        bindings.set("set.season.next", "Season date unavailable");
        bindings.set("set.season.days", "Unavailable");
        bindings.set("set.season.sample_note", "");
    }
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = SPECS.iter().find(|spec| spec.key == key)?;
    let mut doc = card(spec.format);
    doc.template = Some(key.to_owned());
    Some(doc)
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    if !SPECS
        .iter()
        .any(|spec| Some(spec.key) == doc.template.as_deref())
    {
        return;
    }
    for (key, value) in [
        ("set.season.next", "Winter • December 21"),
        ("set.season.days", "37 days"),
        (
            "set.season.sample_note",
            "SAMPLE ONLY • illustrative calendar values",
        ),
    ] {
        doc.data_overrides.push((key.into(), value.into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_keys_and_canvas_sizes_are_exact() {
        let portrait = build("set_season_countdown__portrait").unwrap();
        let square = build("set_season_countdown__square").unwrap();
        assert_eq!((portrait.width, portrait.height), (1080, 1920));
        assert_eq!((square.width, square.height), (1080, 1080));
        assert!(build("set_season_countdown").is_none());
        for doc in [&portrait, &square] {
            assert!(doc.layers.iter().any(|l| l.name == "Social Header"));
            assert!(doc.layers.iter().any(|l| l.name == "Social Footer"));
            for layer in &doc.layers {
                assert!(layer.x >= 0. && layer.y >= 0.);
                assert!(layer.x + layer.w <= doc.width as f32);
                assert!(layer.y + layer.h <= doc.height as f32);
            }
        }
    }

    #[test]
    fn binding_aliases_copy_only_existing_calendar_values() {
        let mut bindings = Bindings::default();
        bind(&mut bindings);
        assert_eq!(
            bindings.get("set.season.next"),
            Some("Season date unavailable")
        );
        assert_eq!(bindings.get("set.season.days"), Some("Unavailable"));

        bindings.set("season.next", "Spring • March 20");
        bindings.set("season.days", "122 days");
        bind(&mut bindings);
        assert_eq!(bindings.get("set.season.next"), Some("Spring • March 20"));
        assert_eq!(bindings.get("set.season.days"), Some("122 days"));
        assert_eq!(bindings.get("season.next"), Some("Spring • March 20"));
        assert_eq!(bindings.get("season.days"), Some("122 days"));
    }

    #[test]
    fn sample_fixture_is_explicitly_marked() {
        let mut doc = build("set_season_countdown__portrait").unwrap();
        sample(&mut doc);
        assert!(doc.data_overrides.iter().any(|(key, value)| {
            key == "set.season.sample_note" && value.contains("SAMPLE ONLY")
        }));
        assert!(doc
            .data_overrides
            .iter()
            .any(|(key, value)| { key == "set.season.next" && value == "Winter • December 21" }));
    }
}
