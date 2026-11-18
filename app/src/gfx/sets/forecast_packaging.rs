//! Story frames and sponsorship are presentation parameters of the NWS point card.
use super::{
    reference::{label, MISSING},
    social_forecast,
};
use crate::gfx::{
    model::{GraphicDoc, LayerKind},
    text::Align,
};

const DAYS: [(&str, &str); 3] = [
    ("Today", "today"),
    ("Tomorrow", "tomorrow"),
    ("Day 3", "day3"),
];

fn card(day: &str, suffix: &str) -> GraphicDoc {
    social_forecast::build(&format!("set_social_forecast__{day}_point{suffix}")).unwrap()
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let (story, suffix) = match key {
        "set_forecast_story" => (true, ""),
        "set_forecast_story__portrait" => (true, "_portrait"),
        "set_sponsored_forecast" => (false, ""),
        "set_sponsored_forecast__portrait" => (false, "_portrait"),
        "set_sponsored_forecast__square" => (false, "_square"),
        _ => return None,
    };
    let mut doc = card("today", suffix);
    doc.name = if story {
        "Three-frame local forecast"
    } else {
        "Sponsored local forecast"
    }
    .into();
    if story {
        doc.states = DAYS.iter().map(|(state, _)| (*state).into()).collect();
        doc.layers.clear();
        for (i, (state, day)) in DAYS.iter().enumerate() {
            let mut frame = card(day, suffix);
            for layer in &mut frame.layers {
                if matches!(layer.name.as_str(), "Title" | "Social Title") {
                    if let LayerKind::Text { text, .. } = &mut layer.kind {
                        *text = format!("Your Forecast • {} / 3", i + 1);
                    }
                }
                layer.states = vec![(*state).into()];
            }
            doc.layers.extend(frame.layers);
        }
    } else {
        for layer in &mut doc.layers {
            if matches!(layer.name.as_str(), "Title" | "Social Title") {
                if let LayerKind::Text { text, .. } = &mut layer.kind {
                    *text = "Sponsored Forecast".into();
                }
            }
        }
        let (x, y, w, h, size) = match suffix {
            "_portrait" => (54., 228., 972., 34., 25.),
            "_square" => (54., 180., 972., 27., 22.),
            _ => (80., 170., 1760., 35., 29.),
        };
        label(
            &mut doc,
            "Sponsor credit",
            (x, y, w, h),
            "Brought to you by {{set.packaging.sponsor}}",
            size,
            "$muted",
            Align::Left,
        );
    }
    Some(doc)
}

pub(super) fn bind(b: &mut crate::gfx::bind::Bindings) {
    b.set("set.packaging.sponsor", MISSING);
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(key) = doc.template.as_deref() else {
        return;
    };
    let story = key.starts_with("set_forecast_story");
    if !story && !key.starts_with("set_sponsored_forecast") {
        return;
    }
    let suffix = if key.ends_with("_portrait") {
        "_portrait"
    } else if key.ends_with("_square") {
        "_square"
    } else {
        ""
    };
    for (i, (_, day)) in DAYS.iter().enumerate().take(if story { 3 } else { 1 }) {
        let mut child = card(day, suffix);
        child.template = Some(format!("set_social_forecast__{day}_point{suffix}"));
        social_forecast::sample(&mut child);
        doc.data_overrides.extend(child.data_overrides);
        for (field, value) in [
            ("period", format!("SAMPLE • {} • Nov {}", DAYS[i].0, 14 + i)),
            ("daylabel", "DAYTIME".into()),
            ("daytemp", format!("{}°F", 74 - i * 3)),
            ("nighttemp", format!("{}°F", 51 - i * 2)),
        ] {
            // Today's fixture retains the reviewed remaining-night semantics.
            if i == 0 && field != "period" {
                continue;
            }
            doc.data_overrides
                .push((format!("set.socialforecast.{i}.{field}"), value));
        }
    }
    if !story {
        doc.data_overrides.push((
            "set.packaging.sponsor".into(),
            "SAMPLE • Jeffersonville Community Market".into(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn story_states_isolate_each_forecast_day_and_default_to_today() {
        for key in ["set_forecast_story", "set_forecast_story__portrait"] {
            let doc = build(key).unwrap();
            for (i, (state, _)) in DAYS.iter().enumerate() {
                let shown = doc.in_state(state, &[]);
                let visible: Vec<_> = shown.layers.iter().filter(|l| l.visible).collect();
                assert!(!visible.is_empty());
                assert!(visible.iter().all(|l| l.states == [*state]));
                let texts: Vec<_> = visible
                    .iter()
                    .filter_map(|l| match &l.kind {
                        LayerKind::Text { text, .. } => Some(text),
                        _ => None,
                    })
                    .collect();
                assert!(texts
                    .iter()
                    .any(|t| t.contains(&format!("set.socialforecast.{i}.period"))));
                for other in 0..3 {
                    if other != i {
                        assert!(!texts
                            .iter()
                            .any(|t| t.contains(&format!("set.socialforecast.{other}."))));
                    }
                }
            }
            assert!(doc
                .in_state("", &[])
                .layers
                .iter()
                .filter(|l| l.visible)
                .all(|l| l.states == ["Today"]));
        }
    }
}
