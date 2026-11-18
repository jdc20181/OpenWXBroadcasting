//! Graphics Studio landing navigation (CWXB-103): category groups for the sidebar, format and
//! data filters, and sub-groups inside big categories. Pure functions, unit-tested.

use crate::gfx::templates::TemplateInfo;

/// Sidebar groups in display order.
pub const GROUPS: &[&str] = &[
    "Social & sharing",
    "Forecast & conditions",
    "Severe & alerts",
    "Winter",
    "Rain & flooding",
    "Tropical",
    "Heat, fire & air",
    "Maps & models",
    "History & climate",
    "Explainers & safety",
    "Show, overlays & more",
    "Other",
];

/// Which sidebar group a category belongs to (by its words; new categories land sensibly).
pub fn group_of(category: &str) -> &'static str {
    let c = category.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| c.contains(w));
    if has(&["social", "viewer"]) {
        "Social & sharing"
    } else if has(&["explainer", "education", "safety", "preparedness", "science"]) {
        "Explainers & safety"
    } else if has(&["histor", "climate", "record", "comparison", "verification"]) {
        "History & climate"
    } else if has(&["alert", "severe", "tornado", "lightning", "warning"]) {
        "Severe & alerts"
    } else if has(&["winter", "snow", "cold", "ice"]) {
        "Winter"
    } else if has(&["rain", "precip", "flood", "river"]) {
        "Rain & flooding"
    } else if has(&["tropical", "hurricane"]) {
        "Tropical"
    } else if has(&["heat", "fire", "smoke", "air quality", "drought", "agricult"]) {
        "Heat, fire & air"
    } else if has(&["model", "map", "radar", "satellite", "noaa", "product"]) {
        "Maps & models"
    } else if has(&["forecast", "current", "hour", "by day", "condition", "temperature", "wind", "fog", "observ"]) {
        "Forecast & conditions"
    } else if has(&["show", "station", "brand", "holiday", "season", "countdown", "school", "community", "astronomy", "travel", "energy", "health", "aviation", "marine", "sky", "overlay", "access", "espa", "power", "outage"]) {
        "Show, overlays & more"
    } else {
        "Other"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Full,
    Overlay,
    Portrait,
    Square,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Full, Format::Overlay, Format::Portrait, Format::Square];
    pub fn label(self) -> &'static str {
        match self {
            Format::Full => "Full screen",
            Format::Overlay => "Overlay",
            Format::Portrait => "Portrait",
            Format::Square => "Square",
        }
    }
}

pub fn format_of(t: &TemplateInfo) -> Format {
    let key = t.key.to_lowercase();
    let name = t.name.to_lowercase();
    let desc = t.description.to_lowercase();
    if key.contains("portrait") || name.contains("portrait") || key.contains("9x16") || name.contains("story") {
        Format::Portrait
    } else if key.contains("square") || name.contains("square") || key.contains("1x1") {
        Format::Square
    } else if desc.contains("overlay") || desc.contains("lower third") || desc.contains("crawl") || desc.contains("ticker") || name.contains("lower third") || key.contains("crawl") || key.contains("ticker") || key.contains("lower") {
        Format::Overlay
    } else {
        Format::Full
    }
}

/// Live data (fetched feeds) vs static (explainers and reference cards that never change).
pub fn is_live(t: &TemplateInfo) -> bool {
    !(t.key.starts_with("explain_") || t.category.eq_ignore_ascii_case("explainers") || t.category.to_lowercase().contains("education"))
}

/// Sub-group inside a category: explainer topic, history period, or a catalog set's base
/// graphic (its variants share a key before "__"). Empty when the template stands alone.
pub fn subgroup(t: &TemplateInfo, all: &[&'static TemplateInfo]) -> String {
    if let Some(s) = crate::gfx::explain_set::subcategory(t.key) {
        return s.to_string();
    }
    if let Some(s) = crate::gfx::history_set::subcategory(t.key) {
        return s.to_string();
    }
    if let Some((base, _)) = t.key.split_once("__") {
        let named = all.iter().find(|o| o.key == base).map(|o| o.name).unwrap_or(t.name);
        return clean_name(named);
    }
    String::new()
}

/// "Tornado Top Years (Portrait)" → "Tornado Top Years".
pub fn clean_name(name: &str) -> String {
    let mut out = name;
    for sep in [" (", " — ", " – ", " · ", " - "] {
        if let Some((head, _)) = out.split_once(sep) {
            out = head;
        }
    }
    out.trim().to_string()
}

/// Search match on name, category, description and sub-group words; every word must match.
pub fn matches(t: &TemplateInfo, query: &str) -> bool {
    let hay = format!("{} {} {} {}", t.name, t.category, t.description, t.key).to_lowercase();
    query.split_whitespace().all(|w| hay.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(key: &'static str, name: &'static str, category: &'static str, description: &'static str) -> TemplateInfo {
        TemplateInfo { key, name, category, description }
    }

    #[test]
    fn groups_cover_known_categories() {
        assert_eq!(group_of("Severe weather"), "Severe & alerts");
        assert_eq!(group_of("Winter, snow, ice and cold"), "Winter");
        assert_eq!(group_of("History"), "History & climate");
        assert_eq!(group_of("Overlays"), "Show, overlays & more");
        assert_eq!(group_of("Science"), "Explainers & safety");
        assert_eq!(group_of("Observed"), "Forecast & conditions");
        assert_eq!(group_of("Explainers"), "Explainers & safety");
        assert_eq!(group_of("Model output maps"), "Maps & models");
        assert_eq!(group_of("Forecast by day"), "Forecast & conditions");
        assert_eq!(group_of("Something new"), "Other");
        // Every real category lands in a listed group.
        for c in crate::gfx::templates::categories() {
            assert!(GROUPS.contains(&group_of(c)), "{c}");
        }
    }

    #[test]
    fn formats_and_live() {
        assert_eq!(format_of(&t("set_x__portrait", "X (Portrait)", "A", "")), Format::Portrait);
        assert_eq!(format_of(&t("set_x__square", "X", "A", "")), Format::Square);
        assert_eq!(format_of(&t("alert_crawl", "Alert Crawl", "Alerts", "Scrolling alert ticker (overlay).")), Format::Overlay);
        assert_eq!(format_of(&t("seven_day", "7-Day", "Forecast", "Seven day cards")), Format::Full);
        assert!(!is_live(&t("explain_fans", "Fans", "Explainers", "")));
        assert!(is_live(&t("hist_freeze", "Freeze", "Historical Data", "")));
    }

    #[test]
    fn variants_group_under_their_base() {
        let base: &'static TemplateInfo = Box::leak(Box::new(t("set_top", "Top Years", "Tornado", "")));
        let v = t("set_top__portrait", "Top Years (Portrait)", "Tornado", "");
        assert_eq!(subgroup(&v, &[base]), "Top Years");
        assert_eq!(clean_name("Top Years — State"), "Top Years");
        assert!(matches(&v, "top portrait"));
        assert!(!matches(&v, "top snow"));
    }
}
