//! A cautious one-word theme derived from the official NWS short forecast.
use super::reference::{MISSING, frame, label, lowerthird_frame, social_frame};
use crate::{
    config::Settings,
    gfx::{
        bind::Bindings,
        model::{AnimKind, GraphicDoc},
        text::Align,
    },
};

#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    day: u8,
    format: u8,
}
include!("weather_word_specs.rs");

fn classify(condition: &str) -> (&'static str, &'static str) {
    let c = condition.to_ascii_lowercase();
    if c.trim().is_empty() || c == MISSING.to_ascii_lowercase() {
        return ("Unavailable", "none");
    }
    // Freeze-related wording takes precedence over rain/snow substrings. The
    // untouched NWS wording remains visible beside this deliberately broad theme.
    if c.contains("freezing") || c.contains("ice") || c.contains("icy") {
        ("Icy", "snow")
    } else if c.contains("thunder") || c.contains("lightning") {
        ("Stormy", "thunder")
    } else if c.contains("snow") || c.contains("sleet") || c.contains("flurr") {
        ("Snowy", "snow")
    } else if c.contains("hail") {
        ("Hail", "hail")
    } else if c.contains("rain") || c.contains("shower") || c.contains("drizzle") {
        ("Wet", "rain")
    } else if c.contains("fog") || c.contains("mist") {
        ("Foggy", "cloud")
    } else if c.contains("wind") || c.contains("breez") {
        ("Windy", "wind")
    } else if c.contains("cloud") || c.contains("overcast") {
        ("Cloudy", "cloud")
    } else if c.contains("sun") || c.contains("clear") || c.contains("fair") {
        ("Clear", "suncloud")
    } else {
        ("Unavailable", "none")
    }
}

/// Returns the established Remix weather-word icon key, suppressing daytime sun at night.
pub(super) fn icon_for(condition: &str, remaining_night: bool) -> &'static str {
    let (_, mut icon) = classify(condition);
    if remaining_night && icon == "suncloud" {
        icon = if condition.to_ascii_lowercase().contains("cloud") {
            "cloud"
        } else {
            "none"
        };
    }
    icon
}
fn get<'a>(values: &'a [(&str, String)], key: &str) -> &'a str {
    values
        .iter()
        .find(|(field, _)| *field == key)
        .map(|(_, value)| value.as_str())
        .unwrap_or("")
}

pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let now = chrono::Utc::now().timestamp();
    for day in 0..=2 {
        let values = super::social_forecast::daily_values(settings.home, false, day, now);
        let condition = get(&values, "condition");
        let remaining_night = get(&values, "note").starts_with("Remaining nighttime");
        let (word, mut icon) = classify(condition);
        if remaining_night && icon == "suncloud" {
            icon = if condition.to_ascii_lowercase().contains("cloud") {
                "cloud"
            } else {
                // The icon set has no moon. Suppress a daytime sun glyph at night.
                "none"
            };
        }
        let p = format!("set.weatherword.{day}");
        for field in [
            "source",
            "period",
            "issued",
            "condition",
            "daytemp",
            "nighttemp",
            "note",
        ] {
            let mut value = get(&values, field).to_owned();
            if field == "condition" && word != "Unavailable" {
                // Keep qualifiers and the full forecast phrase intact.
                value = condition.to_owned();
            }
            if field == "note" && value.starts_with("Remaining nighttime") {
                value = "Remaining nighttime forecast • Daytime temperature withheld".into();
            }
            b.set(&format!("{p}.{field}"), &value);
        }
        b.set(&format!("{p}.word"), word);
        b.set(&format!("{p}.icon"), icon);
        let qualified = condition.to_ascii_lowercase();
        b.set(
            &format!("{p}.theme_label"),
            if word == "Unavailable" {
                "FORECAST THEME • UNAVAILABLE"
            } else if qualified.contains("chance") || qualified.contains("possible") {
                "FORECAST THEME • POSSIBLE"
            } else {
                "FORECAST THEME"
            },
        );
    }
}

fn token(day: u8, field: &str) -> String {
    format!("{{{{set.weatherword.{day}.{field}}}}}")
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|s| s.key == key)?;
    let period = token(s.day, "period");
    let source = token(s.day, "source");
    let mut d = match s.format {
        1 | 2 => social_frame("Weather Word", "Forecast", &period, &source, s.format == 1),
        3 => lowerthird_frame("Weather Word", "Forecast", &period, &source),
        _ => frame("Weather Word", "Forecast", &period, &source),
    };
    if s.format == 3 {
        d.layers.push(
            crate::gfx::model::Layer::rect(
                "Weather word content band",
                0.,
                865.,
                1920.,
                131.,
                crate::gfx::model::Fill::solid("$panel"),
                0.,
            )
            .with_anim(crate::gfx::model::AnimKind::Fade, 0.1, 0.35),
        );
    }
    let word = token(s.day, "word");
    let condition = token(s.day, "condition");
    let date = token(s.day, "issued");
    let theme = token(s.day, "theme_label");
    let note = token(s.day, "note");
    let icon = LayerIcon::spec(s.day);
    let (icon_box, word_box, label_box, forecast_box, context_box, font) = match s.format {
        1 => (
            (350., 385., 380., 300.),
            (65., 755., 950., 190.),
            (65., 708., 950., 42.),
            (90., 1040., 900., 210.),
            (75., 1335., 930., 120.),
            138.,
        ),
        2 => (
            (365., 210., 350., 270.),
            (65., 526., 950., 110.),
            (65., 488., 950., 32.),
            (105., 660., 870., 115.),
            (80., 810., 920., 70.),
            112.,
        ),
        3 => (
            (82., 878., 105., 96.),
            (205., 870., 350., 100.),
            (590., 870., 260., 36.),
            (850., 870., 930., 63.),
            (0., 0., 0., 0.),
            64.,
        ),
        _ => (
            (580., 235., 260., 235.),
            (260., 510., 1400., 155.),
            (260., 455., 1400., 42.),
            (180., 700., 1560., 120.),
            (180., 855., 1560., 42.),
            178.,
        ),
    };
    let icon_layer = crate::gfx::model::Layer::risk_map(
        "Weather word Remix icon",
        icon_box.0,
        icon_box.1,
        icon_box.2,
        icon_box.3,
        &icon,
    );
    d.layers.push(icon_layer.with_anim(AnimKind::Pop, 0.2, 0.4));
    label(
        &mut d,
        "Forecast theme label",
        label_box,
        &theme,
        if s.format == 3 { 18. } else { 24. },
        "$muted",
        Align::Center,
    );
    label(
        &mut d,
        "Weather word",
        word_box,
        &word,
        font,
        "$primary",
        Align::Center,
    );
    label(
        &mut d,
        "Official NWS forecast wording",
        forecast_box,
        &condition,
        if s.format == 3 { 25. } else { 40. },
        "$text",
        Align::Center,
    );
    if s.format == 3 {
        label(
            &mut d,
            "Forecast period note",
            (850., 936., 930., 40.),
            &note,
            17.,
            "$muted",
            Align::Center,
        );
    } else {
        label(
            &mut d,
            "Temperature context",
            context_box,
            &format!(
                "HIGH  {}     LOW  {}",
                token(s.day, "daytemp"),
                token(s.day, "nighttemp")
            ),
            if s.format == 1 { 31. } else { 26. },
            "$text",
            Align::Center,
        );
        label(
            &mut d,
            "Forecast issue and note",
            (
                context_box.0,
                context_box.1 + context_box.3 + 7.,
                context_box.2,
                45.,
            ),
            &format!("{} • {}", date, token(s.day, "note")),
            if s.format == 1 { 22. } else { 19. },
            "$muted",
            Align::Center,
        );
    }
    Some(d)
}

struct LayerIcon;
impl LayerIcon {
    fn spec(day: u8) -> String {
        format!("set:cityforecast-icon:{}", token(day, "icon"))
    }
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(s) = SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
    else {
        return;
    };
    let p = format!("set.weatherword.{}", s.day);
    let sample_period = format!(
        "SAMPLE ONLY • {}",
        match s.day {
            0 => "Today",
            1 => "Tomorrow",
            _ => "Day 3",
        }
    );
    let examples = [
        ("word", "Stormy"),
        ("icon", "thunder"),
        ("theme_label", "FORECAST THEME"),
        ("condition", "Chance of thunderstorms after midnight"),
        ("period", sample_period.as_str()),
        ("source", "SAMPLE ONLY • NOAA / National Weather Service"),
        ("daytemp", "68°F"),
        ("nighttemp", "54°F"),
        ("issued", "SAMPLE ONLY • Updated 18:00 UTC"),
        (
            "note",
            "SAMPLE ONLY • Remaining night shown when applicable",
        ),
    ];
    for (field, value) in examples {
        doc.data_overrides
            .push((format!("{p}.{field}"), value.to_owned()));
    }
}

#[cfg(test)]
mod tests {
    use super::classify;

    #[test]
    fn keeps_qualifiers_out_of_certainty_claims() {
        assert_eq!(classify("Chance of thunderstorms"), ("Stormy", "thunder"));
        assert_eq!(classify("Thunderstorms possible"), ("Stormy", "thunder"));
    }

    #[test]
    fn freezing_and_fog_have_specific_precedence() {
        assert_eq!(classify("Freezing rain"), ("Icy", "snow"));
        assert_eq!(classify("Fog and mist"), ("Foggy", "cloud"));
    }

    #[test]
    fn missing_and_unmapped_conditions_stay_unavailable() {
        assert_eq!(classify(""), ("Unavailable", "none"));
        assert_eq!(classify("Data not available yet"), ("Unavailable", "none"));
        assert_eq!(classify("Patchy blowing dust"), ("Unavailable", "none"));
    }
}
