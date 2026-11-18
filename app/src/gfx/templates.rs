//! Built-in broadcast templates. Each returns a ready-to-air [`GraphicDoc`] bound to live data
//! and styled from the station brand ($primary, $secondary, $display, …).

use crate::gfx::{
    model::{AnimKind, Fill, GraphicDoc, Layer, LayerKind, StrokeStyle},
    text::{Align, Weight},
};

#[path = "templates_ext.rs"]
mod ext;
#[path = "templates_more.rs"]
pub(crate) mod more;
pub use ext::TEMPLATES_EXT;
pub use more::{winter_map, WINTER_MAP_RECT};

pub struct TemplateInfo {
    pub key: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
}

pub const TEMPLATES: &[TemplateInfo] = &[
    TemplateInfo { key: "current", name: "Current Conditions", category: "Forecast", description: "Temperature, sky and the full observation panel." },
    TemplateInfo { key: "seven_day", name: "7-Day Forecast", category: "Forecast", description: "Seven day cards with icons, highs, lows and rain chances." },
    TemplateInfo { key: "three_day", name: "3-Day Forecast", category: "Forecast", description: "Big three-day outlook for the open." },
    TemplateInfo { key: "regional_outlook", name: "Regional Outlook", category: "Forecast", description: "Every coverage town's 7-day in one table: the supported counties (Settings > Coverage area), watched spots and forecast regions." },
    TemplateInfo { key: "tonight_tomorrow", name: "Tonight / Tomorrow", category: "Forecast", description: "Two-panel near-term forecast with NWS wording." },
    TemplateInfo { key: "alert", name: "Weather Alert", category: "Alerts", description: "Full-screen warning card for the top local alert." },
    TemplateInfo { key: "alert_crawl", name: "Alert Crawl", category: "Alerts", description: "Scrolling alert ticker across the bottom (overlay)." },
    TemplateInfo { key: "breaking", name: "Breaking Weather", category: "Alerts", description: "Breaking weather title card." },
    TemplateInfo { key: "severe_outlook", name: "Severe Outlook", category: "Severe", description: "SPC Day 1-3 risk with the risk scale." },
    TemplateInfo { key: "lower_third", name: "Lower Third", category: "Overlays", description: "Title + subtitle bar for over maps and video (overlay)." },
    TemplateInfo { key: "name_super", name: "Name Super", category: "Overlays", description: "Meteorologist name and title (overlay)." },
    TemplateInfo { key: "temp_bug", name: "Temperature Bug", category: "Overlays", description: "Corner temperature + icon bug (overlay)." },
    TemplateInfo { key: "coming_up", name: "Coming Up", category: "Show", description: "Tease / coming up next card." },
    TemplateInfo { key: "station_id", name: "Station ID", category: "Show", description: "Station name and tagline title card." },
    TemplateInfo { key: "sign_off", name: "Sign-Off", category: "Show", description: "Ending card: thanks for watching, tomorrow's highlight, where to follow, sponsor." },
    TemplateInfo { key: "sponsor_bug", name: "Sponsor Bug", category: "Overlays", description: "Corner sponsor logo + line, for over maps (overlay)." },
    TemplateInfo { key: "sponsor_lower_third", name: "Sponsor Lower Third", category: "Overlays", description: "\"Brought to you by\" bar with the sponsor logo (overlay)." },
];

pub(crate) const DISPLAY: &str = "$display";
pub(crate) const BODY: &str = "$body";

pub(crate) fn bg_gradient() -> Option<Fill> {
    Some(Fill::linear("#0C1626", "#173A5C", 115.0))
}

pub(crate) fn text(
    name: &str,
    rect: (f32, f32, f32, f32),
    content: &str,
    font: &str,
    size: f32,
    weight: Weight,
    color: &str,
    align: Align,
) -> Layer {
    Layer::text(name, rect.0, rect.1, rect.2, rect.3, content, font, size, weight, color, align)
}

pub(crate) fn caps(mut layer: Layer, spacing: f32) -> Layer {
    if let LayerKind::Text { uppercase, letter_spacing, .. } = &mut layer.kind {
        *uppercase = true;
        *letter_spacing = spacing;
    }
    layer
}

pub(crate) fn wrapped(mut layer: Layer) -> Layer {
    if let LayerKind::Text { wrap, vcenter, .. } = &mut layer.kind {
        *wrap = true;
        *vcenter = false;
    }
    layer
}

pub(crate) fn shadowed(mut layer: Layer) -> Layer {
    if let LayerKind::Text { shadow, .. } = &mut layer.kind {
        *shadow = true;
    }
    layer
}

pub(crate) fn header(doc: &mut GraphicDoc, title: &str, right: &str) {
    doc.layers.push(
        Layer::rect("Header", 0.0, 0.0, 1920.0, 140.0, Fill::linear("$secondary", "$panel", 0.0), 0.0)
            .with_anim(AnimKind::SlideDown, 0.0, 0.45),
    );
    doc.layers.push(
        Layer::rect("Header accent", 0.0, 140.0, 1920.0, 8.0, Fill::solid("$primary"), 0.0)
            .with_anim(AnimKind::Wipe, 0.25, 0.5),
    );
    doc.layers.push(
        caps(
            text("Title", (80.0, 28.0, 1100.0, 90.0), title, DISPLAY, 70.0, Weight::Bold, "$text", Align::Left),
            3.0,
        )
        .with_anim(AnimKind::SlideLeft, 0.15, 0.5),
    );
    doc.layers.push(
        text("Header right", (1100.0, 34.0, 740.0, 80.0), right, BODY, 40.0, Weight::SemiBold, "$primary", Align::Right)
            .with_anim(AnimKind::Fade, 0.4, 0.4),
    );
}

pub(crate) fn footer(doc: &mut GraphicDoc, left: &str, right: &str) {
    doc.layers.push(
        Layer::rect("Footer", 0.0, 980.0, 1920.0, 100.0, Fill::solid("$panel"), 0.0)
            .with_anim(AnimKind::SlideUp, 0.1, 0.45),
    );
    doc.layers.push(
        Layer::rect("Footer accent", 0.0, 980.0, 1920.0, 4.0, Fill::solid("$primary"), 0.0)
            .with_anim(AnimKind::Wipe, 0.3, 0.5),
    );
    doc.layers.push(
        caps(
            text("Footer left", (80.0, 995.0, 900.0, 70.0), left, DISPLAY, 40.0, Weight::Bold, "$text", Align::Left),
            2.0,
        )
        .with_anim(AnimKind::Fade, 0.5, 0.4),
    );
    doc.layers.push(
        text("Footer right", (960.0, 995.0, 880.0, 70.0), right, BODY, 30.0, Weight::Regular, "$muted", Align::Right)
            .with_anim(AnimKind::Fade, 0.6, 0.4),
    );
}

pub(crate) fn panel(name: &str, x: f32, y: f32, w: f32, h: f32) -> Layer {
    Layer::rect(name, x, y, w, h, Fill::solid("rgba(255,255,255,0.07)"), 22.0)
}

/// Build the template `key` in the station's default look (Modern unless changed in
/// Settings), or None for an unknown key.
pub fn build(key: &str) -> Option<GraphicDoc> {
    let look = crate::gfx::look::default();
    build_in(key, &look)
}

/// Build the template `key` in a given look ("modern" or "classic").
pub fn build_in(key: &str, look: &str) -> Option<GraphicDoc> {
    let mut doc = build_classic(key)?;
    // Templates written for the modern look already carry it; classic ones are restyled.
    if crate::gfx::look::normalize(look) == "modern" || !doc.look.is_empty() {
        crate::gfx::look::apply(&mut doc, look);
    }
    Some(doc)
}

/// Every template: the built-in ones, then the catalog sets (`gfx::sets`).
pub fn all() -> impl Iterator<Item = &'static TemplateInfo> {
    TEMPLATES.iter().chain(TEMPLATES_EXT.iter()).chain(crate::gfx::sets::TEMPLATES.iter()).chain(crate::gfx::history_set::TEMPLATES.iter()).chain(crate::gfx::explain_set::TEMPLATES.iter())
}

/// Every template's categories in display order (landing page).
pub fn categories() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for t in all() {
        // "Severe weather" (catalog sets) and "Severe Weather" (built-ins) are one category:
        // the first spelling seen is the one shown, and filters match it ignoring case.
        if !out.iter().any(|c| c.eq_ignore_ascii_case(t.category)) {
            out.push(t.category);
        }
    }
    out
}

/// Build the template `key` in the style it was written in.
pub fn build_classic(key: &str) -> Option<GraphicDoc> {
    let Some(info) = TEMPLATES.iter().find(|t| t.key == key) else {
        return ext::build(key).or_else(|| crate::gfx::sets::build(key)).or_else(|| crate::gfx::history_set::build(key)).or_else(|| crate::gfx::explain_set::build(key));
    };
    let mut doc = match key {
        "current" => current(),
        "seven_day" => seven_day(),
        "three_day" => three_day(),
        "regional_outlook" => regional_outlook(),
        "tonight_tomorrow" => tonight_tomorrow(),
        "alert" => alert(),
        "alert_crawl" => alert_crawl(),
        "breaking" => breaking(),
        "severe_outlook" => severe_outlook(),
        "lower_third" => lower_third(),
        "name_super" => name_super(),
        "temp_bug" => temp_bug(),
        "coming_up" => coming_up(),
        "station_id" => station_id(),
        "sign_off" => sign_off(),
        "sponsor_bug" => sponsor_bug(),
        "sponsor_lower_third" => sponsor_lower_third(),
        _ => return None,
    };
    doc.name = info.name.to_string();
    doc.category = info.category.to_string();
    doc.template = Some(key.to_string());
    Some(doc)
}

/// A blank document (full screen or overlay).
pub fn blank(overlay: bool) -> GraphicDoc {
    if overlay {
        GraphicDoc::new("Untitled overlay", "Overlays", None)
    } else {
        GraphicDoc::new("Untitled graphic", "Custom", bg_gradient())
    }
}

fn current() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Current Conditions", "Forecast", bg_gradient());
    header(&mut doc, "Current Conditions", "{{location}}");

    doc.layers.push(panel("Temp panel", 80.0, 200.0, 820.0, 740.0).with_anim(AnimKind::Fade, 0.2, 0.5));
    doc.layers.push(Layer::icon("Sky icon", 110.0, 230.0, 380.0, "{{condition}}", "{{daypart}}").with_anim(AnimKind::Pop, 0.35, 0.6));
    doc.layers.push(
        shadowed(text("Temperature", (470.0, 240.0, 410.0, 330.0), "{{temp}}", DISPLAY, 250.0, Weight::Black, "$text", Align::Center))
            .with_anim(AnimKind::SlideRight, 0.4, 0.55),
    );
    doc.layers.push(
        text("Condition", (120.0, 640.0, 740.0, 100.0), "{{condition}}", DISPLAY, 64.0, Weight::Bold, "$text", Align::Left)
            .with_anim(AnimKind::Fade, 0.6, 0.4),
    );
    doc.layers.push(
        caps(text("Feels like", (120.0, 750.0, 740.0, 70.0), "Feels like {{feels}}", BODY, 42.0, Weight::SemiBold, "$primary", Align::Left), 1.0)
            .with_anim(AnimKind::Fade, 0.7, 0.4),
    );
    doc.layers.push(
        text("Observed", (120.0, 840.0, 740.0, 60.0), "{{obs_station}} • {{obs_time}}", BODY, 30.0, Weight::Regular, "$muted", Align::Left)
            .with_anim(AnimKind::Fade, 0.8, 0.4),
    );

    let rows = [
        ("Humidity", "{{humidity}}"),
        ("Dew point", "{{dewpoint}}"),
        ("Wind", "{{wind}}"),
        ("Gusts", "{{gust}}"),
        ("Pressure", "{{pressure}}"),
        ("Visibility", "{{visibility}}"),
    ];
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = 200.0 + i as f32 * 123.0;
        let delay = 0.35 + i as f32 * 0.08;
        doc.layers.push(panel(&format!("{label} row"), 960.0, y, 880.0, 108.0).with_anim(AnimKind::SlideRight, delay, 0.45));
        doc.layers.push(
            caps(text(&format!("{label} label"), (990.0, y + 14.0, 420.0, 80.0), label, BODY, 38.0, Weight::SemiBold, "$muted", Align::Left), 2.0)
                .with_anim(AnimKind::SlideRight, delay, 0.45),
        );
        doc.layers.push(
            text(&format!("{label} value"), (1400.0, y + 10.0, 410.0, 88.0), value, DISPLAY, 56.0, Weight::Bold, "$text", Align::Right)
                .with_anim(AnimKind::SlideRight, delay + 0.05, 0.45),
        );
    }
    footer(&mut doc, "{{station}}", "{{weekday}}, {{date}}");
    doc
}

fn seven_day() -> GraphicDoc {
    let mut doc = GraphicDoc::new("7-Day Forecast", "Forecast", bg_gradient());
    header(&mut doc, "7-Day Forecast", "{{location}}");
    let card_w = 250.0;
    let gap = 14.0;
    let start = (1920.0 - (card_w * 7.0 + gap * 6.0)) / 2.0;
    for i in 0..7 {
        let x = start + i as f32 * (card_w + gap);
        let y = 190.0;
        let delay = 0.25 + i as f32 * 0.08;
        let d = |s: &str| s.replace("N.", &format!("{i}."));
        doc.layers.push(panel(&format!("Day {i} card"), x, y, card_w, 760.0).with_anim(AnimKind::SlideUp, delay, 0.5));
        doc.layers.push(
            caps(text(&format!("Day {i} name"), (x, y + 20.0, card_w, 70.0), &d("{{dayN.short}}"), DISPLAY, 54.0, Weight::Bold, "$primary", Align::Center), 2.0)
                .with_anim(AnimKind::SlideUp, delay, 0.5),
        );
        doc.layers.push(
            Layer::icon(&format!("Day {i} icon"), x + 35.0, y + 110.0, 180.0, &d("{{dayN.condition}}"), &d("{{dayN.daypart}}"))
                .with_anim(AnimKind::Pop, delay + 0.2, 0.5),
        );
        doc.layers.push(
            wrapped(text(&format!("Day {i} condition"), (x + 12.0, y + 305.0, card_w - 24.0, 120.0), &d("{{dayN.condition}}"), BODY, 30.0, Weight::SemiBold, "$text", Align::Center))
                .with_anim(AnimKind::SlideUp, delay, 0.5),
        );
        doc.layers.push(
            shadowed(text(&format!("Day {i} high"), (x, y + 440.0, card_w, 120.0), &d("{{dayN.hi}}"), DISPLAY, 104.0, Weight::Black, "$text", Align::Center))
                .with_anim(AnimKind::SlideUp, delay + 0.05, 0.5),
        );
        doc.layers.push(
            text(&format!("Day {i} low"), (x, y + 565.0, card_w, 80.0), &d("{{dayN.lo}}"), DISPLAY, 62.0, Weight::Bold, "$muted", Align::Center)
                .with_anim(AnimKind::SlideUp, delay + 0.08, 0.5),
        );
        doc.layers.push(
            caps(text(&format!("Day {i} precip"), (x, y + 670.0, card_w, 60.0), &d("Rain {{dayN.precip}}"), BODY, 30.0, Weight::Bold, "#5EC8FF", Align::Center), 1.0)
                .with_anim(AnimKind::Fade, delay + 0.2, 0.4),
        );
    }
    footer(&mut doc, "{{station}}", "Forecast from the National Weather Service");
    doc
}

/// Coverage towns down the side, days across: each cell an icon, the high and the low. Rows
/// past the number of towns stay blank (cov{i}.* are empty).
fn regional_outlook() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Regional Outlook", "Forecast", bg_gradient());
    header(&mut doc, "Regional Outlook", "{{cov.area}}");
    let (name_x, name_w) = (70.0, 330.0);
    let (grid_x, grid_r) = (410.0, 1850.0);
    let col_w = (grid_r - grid_x) / 7.0;
    for j in 0..7 {
        let x = grid_x + j as f32 * col_w;
        let delay = 0.2 + j as f32 * 0.05;
        doc.layers.push(
            caps(text(&format!("Day {j} heading"), (x, 168.0, col_w, 50.0), &format!("{{{{covday{j}.short}}}}"), DISPLAY, 40.0, Weight::Bold, "$primary", Align::Center), 2.0)
                .with_anim(AnimKind::Fade, delay, 0.4),
        );
        doc.layers.push(
            caps(text(&format!("Day {j} date"), (x, 214.0, col_w, 34.0), &format!("{{{{covday{j}.date}}}}"), BODY, 24.0, Weight::SemiBold, "$muted", Align::Center), 1.0)
                .with_anim(AnimKind::Fade, delay, 0.4),
        );
    }
    let (top, row_h, gap) = (262.0, 82.0, 6.0);
    for i in 0..8 {
        let y = top + i as f32 * (row_h + gap);
        let delay = 0.3 + i as f32 * 0.06;
        let fill = if i == 0 { Fill::solid("rgba(94,200,255,0.16)") } else { Fill::solid("rgba(255,255,255,0.06)") };
        doc.layers.push(Layer::rect(&format!("Row {i}"), name_x - 20.0, y, grid_r - name_x + 40.0, row_h, fill, 14.0).with_anim(AnimKind::SlideRight, delay, 0.45).show_if(&format!("{{{{cov{i}.name}}}}")));
        doc.layers.push(
            caps(text(&format!("Town {i}"), (name_x, y, name_w, row_h), &format!("{{{{cov{i}.name}}}}"), DISPLAY, 40.0, Weight::Bold, if i == 0 { "$primary" } else { "$text" }, Align::Left), 1.5)
                .with_anim(AnimKind::SlideRight, delay, 0.45),
        );
        for j in 0..7 {
            let x = grid_x + j as f32 * col_w;
            let k = |s: &str| format!("{{{{cov{i}.d{j}.{s}}}}}");
            doc.layers.push(Layer::icon(&format!("Town {i} day {j} icon"), x + 14.0, y + 9.0, 64.0, &k("condition"), "day").with_anim(AnimKind::Pop, delay + 0.1, 0.4));
            doc.layers.push(
                text(&format!("Town {i} day {j} high"), (x + 80.0, y + 4.0, col_w - 92.0, 44.0), &k("hi"), DISPLAY, 42.0, Weight::Black, "$text", Align::Right)
                    .with_anim(AnimKind::Fade, delay + 0.1, 0.4),
            );
            doc.layers.push(
                text(&format!("Town {i} day {j} low"), (x + 80.0, y + 46.0, col_w - 92.0, 32.0), &k("lo"), DISPLAY, 28.0, Weight::Bold, "$muted", Align::Right)
                    .with_anim(AnimKind::Fade, delay + 0.12, 0.4),
            );
        }
    }
    footer(&mut doc, "{{station}}", "Forecasts from the National Weather Service");
    doc
}

fn three_day() -> GraphicDoc {
    let mut doc = GraphicDoc::new("3-Day Forecast", "Forecast", bg_gradient());
    header(&mut doc, "3-Day Forecast", "{{location}}");
    let card_w = 560.0;
    let gap = 40.0;
    let start = (1920.0 - (card_w * 3.0 + gap * 2.0)) / 2.0;
    for i in 0..3 {
        let x = start + i as f32 * (card_w + gap);
        let y = 200.0;
        let delay = 0.25 + i as f32 * 0.15;
        let d = |s: &str| s.replace("N.", &format!("{i}."));
        doc.layers.push(panel(&format!("Day {i} card"), x, y, card_w, 740.0).with_anim(AnimKind::SlideUp, delay, 0.5));
        doc.layers.push(
            caps(text(&format!("Day {i} name"), (x, y + 24.0, card_w, 90.0), &d("{{dayN.name}}"), DISPLAY, 66.0, Weight::Bold, "$primary", Align::Center), 2.0)
                .with_anim(AnimKind::SlideUp, delay, 0.5),
        );
        doc.layers.push(
            Layer::icon(&format!("Day {i} icon"), x + 150.0, y + 130.0, 260.0, &d("{{dayN.condition}}"), &d("{{dayN.daypart}}"))
                .with_anim(AnimKind::Pop, delay + 0.2, 0.55),
        );
        doc.layers.push(
            wrapped(text(&format!("Day {i} condition"), (x + 30.0, y + 400.0, card_w - 60.0, 110.0), &d("{{dayN.condition}}"), BODY, 40.0, Weight::SemiBold, "$text", Align::Center))
                .with_anim(AnimKind::Fade, delay + 0.25, 0.4),
        );
        doc.layers.push(
            shadowed(text(&format!("Day {i} high"), (x + 20.0, y + 520.0, card_w / 2.0, 150.0), &d("{{dayN.hi}}"), DISPLAY, 130.0, Weight::Black, "$text", Align::Center))
                .with_anim(AnimKind::SlideUp, delay + 0.1, 0.5),
        );
        doc.layers.push(
            text(&format!("Day {i} low"), (x + card_w / 2.0 - 20.0, y + 540.0, card_w / 2.0, 120.0), &d("{{dayN.lo}}"), DISPLAY, 90.0, Weight::Bold, "$muted", Align::Center)
                .with_anim(AnimKind::SlideUp, delay + 0.12, 0.5),
        );
        doc.layers.push(
            caps(text(&format!("Day {i} precip"), (x, y + 670.0, card_w, 54.0), &d("Rain chance {{dayN.precip}}"), BODY, 32.0, Weight::Bold, "#5EC8FF", Align::Center), 1.0)
                .with_anim(AnimKind::Fade, delay + 0.3, 0.4),
        );
    }
    footer(&mut doc, "{{station}}", "{{weekday}}, {{date}}");
    doc
}

fn tonight_tomorrow() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Tonight / Tomorrow", "Forecast", bg_gradient());
    header(&mut doc, "The Forecast", "{{location}}");
    for i in 0..2 {
        let x = 80.0 + i as f32 * 900.0;
        let delay = 0.25 + i as f32 * 0.2;
        let d = |s: &str| s.replace("N.", &format!("{i}."));
        doc.layers.push(panel(&format!("Panel {i}"), x, 200.0, 860.0, 740.0).with_anim(AnimKind::SlideUp, delay, 0.5));
        doc.layers.push(
            caps(text(&format!("Period {i} name"), (x + 40.0, 225.0, 780.0, 90.0), &d("{{periodN.name}}"), DISPLAY, 68.0, Weight::Bold, "$primary", Align::Left), 2.0)
                .with_anim(AnimKind::SlideRight, delay, 0.5),
        );
        doc.layers.push(
            Layer::icon(&format!("Period {i} icon"), x + 40.0, 330.0, 260.0, &d("{{periodN.short}}"), &d("{{periodN.daypart}}"))
                .with_anim(AnimKind::Pop, delay + 0.2, 0.5),
        );
        doc.layers.push(
            shadowed(text(&format!("Period {i} temp"), (x + 330.0, 330.0, 490.0, 260.0), &d("{{periodN.temp}}"), DISPLAY, 210.0, Weight::Black, "$text", Align::Center))
                .with_anim(AnimKind::SlideRight, delay + 0.1, 0.5),
        );
        doc.layers.push(
            wrapped(text(&format!("Period {i} text"), (x + 40.0, 620.0, 780.0, 300.0), &d("{{periodN.text}}"), BODY, 36.0, Weight::Regular, "$text", Align::Left))
                .with_anim(AnimKind::Fade, delay + 0.3, 0.5),
        );
    }
    footer(&mut doc, "{{station}}", "National Weather Service forecast");
    doc
}

fn alert() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Weather Alert", "Alerts", Some(Fill::linear("#22070A", "#5C0E16", 115.0)));
    doc.layers.push(Layer::rect("Alert band", 0.0, 0.0, 1920.0, 170.0, Fill::linear("$alert", "#8E1016", 0.0), 0.0).with_anim(AnimKind::SlideDown, 0.0, 0.4));
    doc.layers.push(
        caps(shadowed(text("Alert title", (80.0, 30.0, 1760.0, 110.0), "Weather Alert", DISPLAY, 96.0, Weight::Black, "#FFFFFF", Align::Left)), 8.0)
            .with_anim(AnimKind::SlideLeft, 0.1, 0.5),
    );
    doc.layers.push(
        caps(shadowed(text("Event", (80.0, 220.0, 1760.0, 150.0), "{{alert0.event}}", DISPLAY, 120.0, Weight::Black, "#FFFFFF", Align::Left)), 2.0)
            .with_anim(AnimKind::Pop, 0.35, 0.6),
    );
    doc.layers.push(
        text("Expires", (80.0, 380.0, 1760.0, 70.0), "{{alert0.expires}}", DISPLAY, 52.0, Weight::Bold, "$warning", Align::Left)
            .with_anim(AnimKind::Fade, 0.6, 0.4),
    );
    doc.layers.push(
        wrapped(text("Areas", (80.0, 470.0, 1760.0, 180.0), "{{alert0.area}}", BODY, 42.0, Weight::SemiBold, "#FFFFFF", Align::Left))
            .with_anim(AnimKind::Fade, 0.7, 0.4),
    );
    doc.layers.push(Layer::rect("Action box", 80.0, 680.0, 1760.0, 270.0, Fill::solid("rgba(0,0,0,0.35)"), 18.0).with_anim(AnimKind::SlideUp, 0.8, 0.5));
    doc.layers.push(
        wrapped(text("Instruction", (120.0, 705.0, 1680.0, 225.0), "{{alert0.instruction}}", BODY, 38.0, Weight::Regular, "#FFFFFF", Align::Left))
            .with_anim(AnimKind::Fade, 1.0, 0.5),
    );
    footer(&mut doc, "{{station}}", "Source: National Weather Service");
    doc
}

fn alert_crawl() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Alert Crawl", "Alerts", None);
    doc.layers.push(Layer::rect("Crawl bar", 0.0, 985.0, 1920.0, 95.0, Fill::solid("rgba(10,12,18,0.92)"), 0.0).with_anim(AnimKind::SlideUp, 0.0, 0.4));
    doc.layers.push(Layer::rect("Crawl label", 0.0, 985.0, 330.0, 95.0, Fill::linear("$alert", "#9E0F16", 0.0), 0.0).with_anim(AnimKind::SlideRight, 0.1, 0.4));
    doc.layers.push(
        caps(text("Crawl label text", (20.0, 990.0, 290.0, 85.0), "Weather Alerts", DISPLAY, 40.0, Weight::Black, "#FFFFFF", Align::Center), 1.5)
            .with_anim(AnimKind::Fade, 0.2, 0.3),
    );
    doc.layers.push(
        Layer::new(
            "Crawl",
            340.0,
            985.0,
            1580.0,
            95.0,
            LayerKind::Ticker {
                text: "{{alerts.ticker}}".into(),
                font: BODY.into(),
                size: 44.0,
                color: "#FFFFFF".into(),
                speed: 150.0,
            },
        )
        .with_anim(AnimKind::Fade, 0.3, 0.4),
    );
    doc.duration = 0.0;
    doc
}

fn breaking() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Breaking Weather", "Alerts", Some(Fill::linear("#0B0E15", "#3A0A10", 120.0)));
    for i in 0..6 {
        doc.layers.push(
            Layer::rect(&format!("Stripe {i}"), -200.0 + i as f32 * 420.0, 0.0, 120.0, 1080.0, Fill::solid("rgba(229,56,59,0.10)"), 0.0)
                .with_anim(AnimKind::SlideLeft, i as f32 * 0.05, 0.6),
        );
    }
    for layer in doc.layers.iter_mut() {
        layer.rotation = 18.0;
    }
    doc.layers.push(Layer::rect("Breaking band", 0.0, 360.0, 1920.0, 220.0, Fill::linear("$alert", "#8E1016", 0.0), 0.0).with_anim(AnimKind::Wipe, 0.2, 0.5));
    doc.layers.push(
        caps(shadowed(text("Breaking", (60.0, 370.0, 1800.0, 200.0), "Breaking Weather", DISPLAY, 170.0, Weight::Black, "#FFFFFF", Align::Center)), 10.0)
            .with_anim(AnimKind::Pop, 0.45, 0.6),
    );
    doc.layers.push(
        text("Subtitle", (60.0, 620.0, 1800.0, 110.0), "Severe weather update for {{location}}", DISPLAY, 70.0, Weight::Bold, "$text", Align::Center)
            .with_anim(AnimKind::SlideUp, 0.7, 0.5),
    );
    footer(&mut doc, "{{station}}", "{{time}} • {{weekday}}, {{date}}");
    doc
}

fn severe_outlook() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Severe Outlook", "Severe", bg_gradient());
    header(&mut doc, "Severe Weather Outlook", "Source: NWS Storm Prediction Center");
    doc.layers.push(panel("Day 1 panel", 80.0, 200.0, 1760.0, 300.0).with_anim(AnimKind::SlideUp, 0.2, 0.5));
    doc.layers.push(
        caps(text("Day 1 label", (120.0, 220.0, 400.0, 80.0), "Day 1 • Today", DISPLAY, 50.0, Weight::Bold, "$primary", Align::Left), 2.0)
            .with_anim(AnimKind::Fade, 0.3, 0.4),
    );
    doc.layers.push(
        shadowed(text("Day 1 risk", (120.0, 300.0, 1680.0, 170.0), "{{spc.day1.home}}", DISPLAY, 120.0, Weight::Black, "$text", Align::Left))
            .with_anim(AnimKind::SlideRight, 0.4, 0.5),
    );

    let levels = [
        ("TSTM", "Thunder", "#C1E9C1"),
        ("MRGL", "1 Marginal", "#66A366"),
        ("SLGT", "2 Slight", "#FFE066"),
        ("ENH", "3 Enhanced", "#FFA366"),
        ("MDT", "4 Moderate", "#E06666"),
        ("HIGH", "5 High", "#EE99EE"),
    ];
    let w = 1760.0 / 6.0;
    for (i, (code, label, color)) in levels.iter().enumerate() {
        let x = 80.0 + i as f32 * w;
        let delay = 0.5 + i as f32 * 0.07;
        doc.layers.push(Layer::rect(&format!("{code} box"), x + 4.0, 540.0, w - 8.0, 150.0, Fill::solid(color), 12.0).with_anim(AnimKind::SlideUp, delay, 0.45));
        doc.layers.push(
            caps(text(&format!("{code} text"), (x + 4.0, 560.0, w - 8.0, 110.0), label, DISPLAY, 44.0, Weight::Black, "#10131A", Align::Center), 1.0)
                .with_anim(AnimKind::SlideUp, delay, 0.45),
        );
    }

    for (i, day) in ["2", "3"].iter().enumerate() {
        let y = 730.0 + i as f32 * 120.0;
        doc.layers.push(panel(&format!("Day {day} row"), 80.0, y, 1760.0, 100.0).with_anim(AnimKind::SlideRight, 0.8 + i as f32 * 0.1, 0.45));
        doc.layers.push(
            caps(text(&format!("Day {day} label"), (120.0, y + 10.0, 400.0, 80.0), &format!("Day {day}"), DISPLAY, 46.0, Weight::Bold, "$primary", Align::Left), 2.0)
                .with_anim(AnimKind::SlideRight, 0.8 + i as f32 * 0.1, 0.45),
        );
        doc.layers.push(
            text(&format!("Day {day} risk"), (520.0, y + 10.0, 1280.0, 80.0), &format!("{{{{spc.day{day}.home}}}}"), DISPLAY, 50.0, Weight::Bold, "$text", Align::Right)
                .with_anim(AnimKind::SlideRight, 0.85 + i as f32 * 0.1, 0.45),
        );
    }
    footer(&mut doc, "{{station}}", "Source: NWS Storm Prediction Center");
    doc
}

fn lower_third() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Lower Third", "Overlays", None);
    doc.layers.push(Layer::rect("Tag", 120.0, 788.0, 330.0, 50.0, Fill::solid("$primary"), 6.0).with_anim(AnimKind::Wipe, 0.0, 0.35));
    doc.layers.push(
        caps(text("Tag text", (135.0, 790.0, 300.0, 46.0), "{{station_short}} Weather", DISPLAY, 30.0, Weight::Black, "#0B0F18", Align::Left), 2.0)
            .with_anim(AnimKind::Fade, 0.15, 0.3),
    );
    doc.layers.push(Layer::rect("Bar", 120.0, 840.0, 1180.0, 120.0, Fill::linear("$secondary", "$panel", 0.0), 6.0).with_anim(AnimKind::Wipe, 0.1, 0.45));
    doc.layers.push(Layer::rect("Accent", 120.0, 840.0, 14.0, 120.0, Fill::solid("$primary"), 0.0).with_anim(AnimKind::Fade, 0.1, 0.3));
    doc.layers.push(
        caps(text("Title", (160.0, 846.0, 1110.0, 66.0), "Weather Update", DISPLAY, 56.0, Weight::Bold, "$text", Align::Left), 1.5)
            .with_anim(AnimKind::SlideLeft, 0.25, 0.45),
    );
    doc.layers.push(
        text("Subtitle", (160.0, 908.0, 1110.0, 46.0), "{{location}} • {{time}}", BODY, 34.0, Weight::SemiBold, "$primary", Align::Left)
            .with_anim(AnimKind::Fade, 0.4, 0.4),
    );
    doc.duration = 0.0;
    doc
}

fn name_super() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Name Super", "Overlays", None);
    doc.layers.push(Layer::rect("Name bar", 120.0, 820.0, 900.0, 100.0, Fill::linear("$secondary", "$panel", 0.0), 4.0).with_anim(AnimKind::Wipe, 0.0, 0.45));
    doc.layers.push(Layer::rect("Accent", 120.0, 920.0, 900.0, 8.0, Fill::solid("$primary"), 0.0).with_anim(AnimKind::Wipe, 0.15, 0.45));
    doc.layers.push(
        caps(text("Name", (150.0, 826.0, 840.0, 88.0), "Your Name", DISPLAY, 64.0, Weight::Bold, "$text", Align::Left), 2.0)
            .with_anim(AnimKind::SlideLeft, 0.2, 0.45),
    );
    doc.layers.push(Layer::rect("Title bar", 120.0, 928.0, 620.0, 58.0, Fill::solid("rgba(10,12,18,0.85)"), 0.0).with_anim(AnimKind::Wipe, 0.3, 0.4));
    doc.layers.push(
        caps(text("Title", (150.0, 930.0, 580.0, 54.0), "Meteorologist • {{station_short}}", BODY, 32.0, Weight::SemiBold, "$primary", Align::Left), 1.0)
            .with_anim(AnimKind::Fade, 0.45, 0.4),
    );
    doc.duration = 0.0;
    doc
}

fn temp_bug() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Temperature Bug", "Overlays", None);
    let mut bg = Layer::rect("Bug", 60.0, 60.0, 360.0, 140.0, Fill::solid("rgba(16,20,30,0.85)"), 18.0);
    if let LayerKind::Rect { stroke, .. } = &mut bg.kind {
        *stroke = Some(StrokeStyle {
            color: "$primary".into(),
            width: 3.0,
        });
    }
    doc.layers.push(bg.with_anim(AnimKind::Pop, 0.0, 0.5));
    doc.layers.push(Layer::icon("Icon", 72.0, 70.0, 120.0, "{{condition}}", "{{daypart}}").with_anim(AnimKind::Fade, 0.2, 0.4));
    doc.layers.push(
        text("Temp", (200.0, 64.0, 210.0, 100.0), "{{temp}}", DISPLAY, 92.0, Weight::Black, "$text", Align::Center)
            .with_anim(AnimKind::Fade, 0.25, 0.4),
    );
    doc.layers.push(
        caps(text("Place", (200.0, 158.0, 210.0, 36.0), "{{station_short}}", BODY, 24.0, Weight::Bold, "$primary", Align::Center), 2.0)
            .with_anim(AnimKind::Fade, 0.35, 0.4),
    );
    doc.duration = 0.0;
    doc
}

fn coming_up() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Coming Up", "Show", bg_gradient());
    doc.layers.push(
        caps(text("Kicker", (120.0, 300.0, 1680.0, 120.0), "Coming Up", DISPLAY, 110.0, Weight::Black, "$primary", Align::Left), 8.0)
            .with_anim(AnimKind::SlideLeft, 0.1, 0.5),
    );
    doc.layers.push(Layer::rect("Rule", 120.0, 440.0, 600.0, 10.0, Fill::solid("$primary"), 0.0).with_anim(AnimKind::Wipe, 0.3, 0.5));
    doc.layers.push(
        wrapped(text("Tease", (120.0, 490.0, 1680.0, 300.0), "Your full 7-day forecast and a look at the storms on radar", DISPLAY, 80.0, Weight::Bold, "$text", Align::Left))
            .with_anim(AnimKind::Fade, 0.5, 0.5),
    );
    footer(&mut doc, "{{station}}", "{{tagline}}");
    doc
}

fn station_id() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Station ID", "Show", Some(Fill::linear("#0A0F1A", "#1D2638", 100.0)));
    doc.layers.push(
        caps(shadowed(text("Station", (100.0, 340.0, 1720.0, 220.0), "{{station_short}}", DISPLAY, 210.0, Weight::Black, "$text", Align::Center)), 6.0)
            .with_anim(AnimKind::Pop, 0.1, 0.7),
    );
    doc.layers.push(Layer::rect("Rule", 660.0, 575.0, 600.0, 10.0, Fill::solid("$primary"), 5.0).with_anim(AnimKind::Wipe, 0.5, 0.5));
    doc.layers.push(
        caps(text("Name", (100.0, 600.0, 1720.0, 90.0), "{{station}}", BODY, 58.0, Weight::Light, "$muted", Align::Center), 10.0)
            .with_anim(AnimKind::Fade, 0.7, 0.5),
    );
    doc.layers.push(
        text("Tagline", (100.0, 700.0, 1720.0, 70.0), "{{tagline}}", DISPLAY, 44.0, Weight::Bold, "$primary", Align::Center)
            .with_anim(AnimKind::SlideUp, 0.9, 0.5),
    );
    doc
}

/// Ending card for a show.
fn sign_off() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Sign-Off", "Show", Some(Fill::linear("#0A0F1A", "#1D2638", 100.0)));
    doc.layers.push(
        caps(shadowed(text("Thanks", (100.0, 150.0, 1720.0, 140.0), "Thanks for watching", DISPLAY, 120.0, Weight::Black, "$text", Align::Center)), 4.0)
            .with_anim(AnimKind::Pop, 0.1, 0.6),
    );
    doc.layers.push(Layer::rect("Rule", 660.0, 315.0, 600.0, 10.0, Fill::solid("$primary"), 5.0).with_anim(AnimKind::Wipe, 0.4, 0.5));
    doc.layers.push(panel("Tomorrow panel", 360.0, 370.0, 1200.0, 250.0).with_anim(AnimKind::Fade, 0.5, 0.5));
    doc.layers.push(
        caps(text("Tomorrow label", (400.0, 390.0, 1120.0, 50.0), "Next up: {{day1.name}}", BODY, 36.0, Weight::Bold, "$primary", Align::Center), 2.0)
            .with_anim(AnimKind::Fade, 0.6, 0.4),
    );
    doc.layers.push(Layer::icon("Tomorrow icon", 440.0, 450.0, 150.0, "{{day1.condition}}", "day").with_anim(AnimKind::Pop, 0.7, 0.5));
    doc.layers.push(
        text("Tomorrow temps", (620.0, 455.0, 880.0, 90.0), "High {{day1.hi}}  •  Low {{day1.lo}}", DISPLAY, 70.0, Weight::Bold, "$text", Align::Left)
            .with_anim(AnimKind::SlideLeft, 0.75, 0.5),
    );
    doc.layers.push(
        text("Tomorrow text", (620.0, 545.0, 880.0, 50.0), "{{day1.condition}}", BODY, 34.0, Weight::SemiBold, "$muted", Align::Left)
            .with_anim(AnimKind::Fade, 0.85, 0.5),
    );
    doc.layers.push(
        caps(text("Station", (100.0, 680.0, 1720.0, 90.0), "{{station}}", DISPLAY, 64.0, Weight::Black, "$text", Align::Center), 3.0)
            .with_anim(AnimKind::Fade, 1.0, 0.5),
    );
    doc.layers.push(
        text("Follow", (100.0, 770.0, 1720.0, 60.0), "{{station}}  •  {{tagline}}", BODY, 40.0, Weight::SemiBold, "$primary", Align::Center)
            .with_anim(AnimKind::Fade, 1.1, 0.5),
    );
    doc.layers.push(
        text("Sponsor line", (100.0, 900.0, 1400.0, 60.0), "{{sponsor_line}}", BODY, 34.0, Weight::SemiBold, "$muted", Align::Right)
            .with_anim(AnimKind::Fade, 1.3, 0.5),
    );
    doc.layers.push(
        Layer::new("Sponsor logo", 1530.0, 870.0, 290.0, 120.0, LayerKind::Image { media: "{{sponsor_logo}}".into(), fit: crate::gfx::model::ImageFit::Contain })
            .with_anim(AnimKind::Fade, 1.3, 0.5),
    );
    doc
}

/// Small corner sponsor bug.
fn sponsor_bug() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Sponsor Bug", "Overlays", None);
    doc.layers.push(Layer::rect("Bug", 1480.0, 150.0, 400.0, 150.0, Fill::solid("rgba(16,20,30,0.85)"), 14.0).with_anim(AnimKind::SlideLeft, 0.0, 0.4));
    doc.layers.push(
        caps(text("Label", (1500.0, 158.0, 360.0, 30.0), "Presented by", BODY, 22.0, Weight::Bold, "$primary", Align::Center), 2.0)
            .with_anim(AnimKind::Fade, 0.2, 0.3),
    );
    doc.layers.push(
        Layer::new("Sponsor logo", 1500.0, 190.0, 360.0, 100.0, LayerKind::Image { media: "{{sponsor_logo}}".into(), fit: crate::gfx::model::ImageFit::Contain })
            .with_anim(AnimKind::Fade, 0.25, 0.4),
    );
    doc.layers.push(
        text("Sponsor name", (1500.0, 210.0, 360.0, 60.0), "{{sponsor}}", DISPLAY, 40.0, Weight::Bold, "$text", Align::Center).with_anim(AnimKind::Fade, 0.25, 0.4),
    );
    doc.duration = 0.0;
    doc
}

/// "Brought to you by" bar.
fn sponsor_lower_third() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Sponsor Lower Third", "Overlays", None);
    doc.layers.push(Layer::rect("Bar", 120.0, 860.0, 1100.0, 100.0, Fill::linear("$secondary", "$panel", 0.0), 6.0).with_anim(AnimKind::Wipe, 0.0, 0.45));
    doc.layers.push(Layer::rect("Accent", 120.0, 860.0, 14.0, 100.0, Fill::solid("$primary"), 0.0).with_anim(AnimKind::Fade, 0.1, 0.3));
    doc.layers.push(
        text("Line", (160.0, 872.0, 760.0, 76.0), "{{sponsor_line}}", DISPLAY, 44.0, Weight::Bold, "$text", Align::Left).with_anim(AnimKind::SlideLeft, 0.2, 0.45),
    );
    doc.layers.push(
        Layer::new("Sponsor logo", 940.0, 866.0, 260.0, 88.0, LayerKind::Image { media: "{{sponsor_logo}}".into(), fit: crate::gfx::model::ImageFit::Contain })
            .with_anim(AnimKind::Fade, 0.3, 0.4),
    );
    doc.duration = 0.0;
    doc
}

// ---- Graphics generated from a specific warning (Severe Center) -------------------------------

fn alert_summary(alert: &crate::data::Alert) -> (String, String, String) {
    let towns = alert.towns();
    let places = if towns.is_empty() {
        alert.area_desc.replace(';', ",")
    } else {
        towns.iter().take(6).cloned().collect::<Vec<_>>().join(" • ")
    };
    let until = alert
        .expires_at()
        .map(|t| format!("UNTIL {}", t.with_timezone(&chrono::Local).format("%-I:%M %p")))
        .unwrap_or_default();
    let motion = alert
        .motion()
        .map(|m| format!("Moving {} at {:.0} mph", crate::map::geo::compass(m.heading_deg), m.speed_mph))
        .unwrap_or_default();
    (places, until, motion)
}

/// The event as shown on air: practice warnings are labelled PRACTICE wherever they appear.
fn event_label(alert: &crate::data::Alert) -> String {
    if alert.is_practice() { format!("PRACTICE · {}", alert.event) } else { alert.event.clone() }
}

/// Source credit for a warning graphic.
fn warning_source(alert: &crate::data::Alert) -> &'static str {
    if alert.is_practice() { "PRACTICE ONLY · not a real warning · OpenWX rehearsal" } else { "Source: National Weather Service" }
}

/// Lower third for one warning: coloured event bar, towns, expiry and motion.
pub fn warning_lower_third(alert: &crate::data::Alert) -> GraphicDoc {
    let (places, until, motion) = alert_summary(alert);
    let color = alert.color().to_string();
    let mut doc = GraphicDoc::new(&format!("{} • lower third", event_label(alert)), "Alerts", None);
    doc.layers.push(Layer::rect("Event bar", 100.0, 770.0, 1720.0, 84.0, Fill::linear(&color, "#111111", 0.0), 6.0).with_anim(AnimKind::Wipe, 0.0, 0.4));
    doc.layers.push(
        caps(shadowed(text("Event", (130.0, 776.0, 1100.0, 72.0), &event_label(alert), DISPLAY, 58.0, Weight::Black, "#FFFFFF", Align::Left)), 2.0)
            .with_anim(AnimKind::SlideLeft, 0.15, 0.4),
    );
    doc.layers.push(
        caps(shadowed(text("Until", (1200.0, 782.0, 590.0, 60.0), &until, DISPLAY, 44.0, Weight::Bold, "#FFFFFF", Align::Right)), 1.0)
            .with_anim(AnimKind::Fade, 0.3, 0.4),
    );
    doc.layers.push(Layer::rect("Body", 100.0, 854.0, 1720.0, 120.0, Fill::linear("$secondary", "$panel", 0.0), 6.0).with_anim(AnimKind::Wipe, 0.1, 0.45));
    doc.layers.push(
        wrapped(text("Places", (130.0, 860.0, 1660.0, 64.0), &places, BODY, 40.0, Weight::Bold, "$text", Align::Left))
            .with_anim(AnimKind::Fade, 0.35, 0.4),
    );
    doc.layers.push(
        text("Motion", (130.0, 922.0, 1660.0, 46.0), &if alert.is_practice() { warning_source(alert).to_string() } else if motion.is_empty() { alert.sender.clone() } else { motion }, BODY, 32.0, Weight::SemiBold, "$primary", Align::Left)
            .with_anim(AnimKind::Fade, 0.5, 0.4),
    );
    doc.duration = 0.0;
    doc
}

/// Full-screen card for one warning (the "one-click tornado warning graphic").
pub fn warning_fullscreen(alert: &crate::data::Alert) -> GraphicDoc {
    let (places, until, motion) = alert_summary(alert);
    let color = alert.color().to_string();
    let mut doc = GraphicDoc::new(&format!("{} • full screen", event_label(alert)), "Alerts", Some(Fill::linear("#0B0F18", "#1A0A0C", 115.0)));
    doc.layers.push(Layer::rect("Band", 0.0, 0.0, 1920.0, 190.0, Fill::linear(&color, "#111111", 0.0), 0.0).with_anim(AnimKind::SlideDown, 0.0, 0.4));
    doc.layers.push(
        wrapped(caps(shadowed(text("Event", (80.0, 30.0, 1760.0, 130.0), &event_label(alert), DISPLAY, 110.0, Weight::Black, "#FFFFFF", Align::Left)), 4.0))
            .with_anim(AnimKind::SlideLeft, 0.1, 0.5),
    );
    if let Some(tag) = alert.threat_tag() {
        doc.layers.push(Layer::rect("Tag", 80.0, 220.0, 1100.0, 80.0, Fill::solid("#8E00A8"), 8.0).with_anim(AnimKind::Pop, 0.3, 0.4));
        doc.layers.push(caps(text("Tag text", (100.0, 226.0, 1060.0, 68.0), tag, DISPLAY, 52.0, Weight::Black, "#FFFFFF", Align::Left), 2.0).with_anim(AnimKind::Fade, 0.4, 0.3));
    }
    doc.layers.push(
        caps(text("Until", (80.0, 320.0, 1760.0, 90.0), &until, DISPLAY, 72.0, Weight::Bold, "$warning", Align::Left), 1.0).with_anim(AnimKind::Fade, 0.5, 0.4),
    );
    doc.layers.push(
        wrapped(text("Places", (80.0, 430.0, 1760.0, 250.0), &format!("Including {places}"), BODY, 54.0, Weight::Bold, "#FFFFFF", Align::Left))
            .with_anim(AnimKind::Fade, 0.6, 0.4),
    );
    if !motion.is_empty() {
        doc.layers.push(text("Motion", (80.0, 700.0, 1760.0, 70.0), &motion, DISPLAY, 50.0, Weight::Bold, "$primary", Align::Left).with_anim(AnimKind::Fade, 0.7, 0.4));
    }
    let action = if alert.instruction.trim().is_empty() { safety_text(&alert.event).to_string() } else { alert.instruction.replace('\n', " ") };
    doc.layers.push(Layer::rect("Action box", 80.0, 790.0, 1760.0, 190.0, Fill::solid("rgba(0,0,0,0.4)"), 16.0).with_anim(AnimKind::SlideUp, 0.8, 0.5));
    doc.layers.push(
        wrapped(text("Action", (110.0, 805.0, 1700.0, 165.0), &action, BODY, 36.0, Weight::Regular, "#FFFFFF", Align::Left)).with_anim(AnimKind::Fade, 0.9, 0.5),
    );
    footer(&mut doc, "{{station}}", warning_source(alert));
    doc
}

fn safety_text(event: &str) -> &'static str {
    let e = event.to_lowercase();
    if e.contains("tornado") {
        "Go to the lowest floor of a sturdy building, in an interior room away from windows. Mobile homes are not safe - get to a shelter now. Cover your head and neck."
    } else if e.contains("flood") {
        "Move to higher ground now. Never drive into flooded roads - Turn Around, Don't Drown. Six inches of moving water can knock you down; a foot can carry a car away."
    } else if e.contains("thunderstorm") || e.contains("wind") {
        "Get indoors and away from windows. Damaging winds and large hail can break glass and down trees and power lines. When thunder roars, go indoors."
    } else if e.contains("winter") || e.contains("snow") || e.contains("ice") || e.contains("blizzard") {
        "Avoid travel if you can. If you must drive, slow down, leave extra room, and keep a winter kit in the car. Check on neighbors."
    } else if e.contains("heat") {
        "Drink plenty of water, stay in air conditioning, and never leave people or pets in a parked car. Check on the elderly."
    } else {
        "Stay weather aware, keep a way to get warnings, and follow instructions from local officials."
    }
}

/// Safety card ("what to do") for a hazard: tornado, flood, thunderstorm, winter, heat.
pub fn safety_card(hazard: &str) -> GraphicDoc {
    let title = match hazard {
        "tornado" => "Tornado Safety",
        "flood" => "Flood Safety",
        "thunderstorm" => "Severe Storm Safety",
        "winter" => "Winter Weather Safety",
        "heat" => "Heat Safety",
        _ => "Weather Safety",
    };
    let mut doc = GraphicDoc::new(title, "Alerts", bg_gradient());
    header(&mut doc, title, "{{station_short}} Weather");
    doc.layers.push(panel("Panel", 80.0, 200.0, 1760.0, 700.0).with_anim(AnimKind::Fade, 0.2, 0.5));
    doc.layers.push(
        wrapped(text("Safety", (140.0, 250.0, 1640.0, 600.0), safety_text(hazard), BODY, 60.0, Weight::Bold, "$text", Align::Left))
            .with_anim(AnimKind::Fade, 0.4, 0.5),
    );
    footer(&mut doc, "{{station}}", "Source: National Weather Service");
    doc
}

#[cfg(test)]
mod practice_tests {
    use super::*;

    fn practice() -> crate::data::Alert {
        crate::data::Alert {
            id: "TEST-1-1".into(),
            event: "Tornado Warning".into(),
            area_desc: "TEST AREA near Jasper".into(),
            sender: "OpenWX rehearsal (TEST)".into(),
            ..Default::default()
        }
    }

    /// Practice warnings are labelled PRACTICE and never credited to the NWS; real ones are.
    #[test]
    fn practice_warning_graphics_say_practice() {
        let texts = |doc: &GraphicDoc| -> String {
            doc.layers.iter().filter_map(|l| match &l.kind { LayerKind::Text { text, .. } => Some(text.clone()), _ => None }).collect::<Vec<_>>().join(" | ")
        };
        for doc in [warning_fullscreen(&practice()), warning_lower_third(&practice())] {
            let all = texts(&doc);
            assert!(doc.name.starts_with("PRACTICE"), "{}", doc.name);
            assert!(all.contains("PRACTICE · Tornado Warning"), "{all}");
            assert!(!all.contains("National Weather Service"), "{all}");
        }
        let real = crate::data::Alert { id: "urn:oid:2.49.0.1.840.0.abc".into(), ..practice() };
        let doc = warning_fullscreen(&real);
        assert!(!doc.name.starts_with("PRACTICE"));
        assert!(texts(&doc).contains("Source: National Weather Service"));
        if let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) {
            let settings = crate::config::Settings::default();
            let data = crate::data::WxSnapshot::default();
            let pm = crate::program::render_graphic_mode(&warning_fullscreen(&practice()), &settings, &data, 10_000., 960, 540, None, crate::gfx::check::PreviewData::Sample);
            pm.save_png(dir.join("practice-warning.png")).unwrap();
        }
    }
}

#[cfg(test)]
mod credit_tests {
    use crate::gfx::model::LayerKind;

    /// Graphics whose numbers still come from Open-Meteo (until CWXB-68 replaces them) may
    /// credit it; nothing else may.
    const STILL_OPEN_METEO: &[&str] = &[
        "aqi_today", "smoke_visibility", "climate_daily", "climate_month_temp", "climate_month_precip",
        "climate_record_high", "climate_record_low", "climate_normals", "climate_this_day",
        "ens_scenarios", "severe_ingredients", "winter_ingredients",
    ];

    #[test]
    fn no_wrong_open_meteo_credit() {
        let mut wrong = Vec::new();
        for info in super::all() {
            let Some(doc) = super::build_classic(info.key) else { continue };
            let credits = doc.layers.iter().any(|l| matches!(&l.kind, LayerKind::Text { text, .. } if text.contains("Open-Meteo")));
            if credits && !STILL_OPEN_METEO.contains(&info.key) {
                wrong.push(info.key);
            }
        }
        assert!(wrong.is_empty(), "these graphics credit Open-Meteo but don't use it: {wrong:?}");
    }

    #[test]
    fn table_titles_are_not_source_lines() {
        for key in ["damage_survey", "tropical_track", "drought_status"] {
            let Some(doc) = super::build_classic(key) else { continue };
            for l in &doc.layers {
                if let LayerKind::Text { text, .. } = &l.kind {
                    if l.name == "Title" || l.name == "Subtitle" {
                        assert!(!text.to_lowercase().starts_with("source:"), "{key}: {} reads {text:?}", l.name);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod diagram_qa {
    /// Renders the explainer cards that got their own diagrams (CWX_QA_NEW_DIR set), as a
    /// 2-column contact sheet at two moments of the loop.
    #[test]
    fn qa_explainer_diagrams() {
        let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) else { return };
        let keys = [
            "edu_watch_warning", "demo_wall_cloud", "demo_funnel", "demo_debris", "demo_hook", "hur_categories",
            "hur_evac_timeline", "safe_tornado_school", "shelter_interior", "flash_flood_explainer", "radar_debris",
        ];
        let settings = crate::config::Settings::default();
        let data = crate::data::WxSnapshot::default();
        let mut found = Vec::new();
        for info in super::all() {
            let hit = keys.contains(&info.key)
                || ["Interior Hallway", "Interior Room", "How Flash Floods Happen", "Spotting Debris on Radar"].contains(&info.name);
            if hit {
                found.push(info.key);
            }
        }
        let (cw, ch) = (960u32, 540u32);
        for (t, tag) in [(2_500.0f32, "a"), (6_000.0, "b")] {
            let rows = found.len().div_ceil(2) as u32;
            let mut sheet = tiny_skia::Pixmap::new(cw * 2, ch * rows).unwrap();
            for (i, key) in found.iter().enumerate() {
                let doc = super::build_classic(key).unwrap();
                let pm = crate::program::render_graphic_mode(&doc, &settings, &data, t, cw, ch, None, crate::gfx::check::PreviewData::Sample);
                let (x, y) = ((i as u32 % 2) * cw, (i as u32 / 2) * ch);
                sheet.draw_pixmap(x as i32, y as i32, pm.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
            }
            sheet.save_png(dir.join(format!("diagrams-{tag}.png"))).unwrap();
        }
        eprintln!("rendered {found:?}");
    }
}
