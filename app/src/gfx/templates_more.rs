//! Holiday forecast cards, the Weather Alert Day card and the county risk-map graphics (child
//! module of `templates`, sharing its layout helpers).

use super::*;
use crate::gfx::holidays;

/// Decorative motif image at (x, y), `size` square.
fn motif(name: &str, color: &str, x: f32, y: f32, size: f32, rotation: f32, opacity: f32, delay: f32) -> Layer {
    let mut l = Layer::new(
        &format!("Motif {name}"),
        x,
        y,
        size,
        size,
        LayerKind::Image { media: format!("motif:{name}:{color}"), fit: crate::gfx::model::ImageFit::Contain },
    );
    l.rotation = rotation;
    l.opacity = opacity;
    l.with_anim(AnimKind::Zoom, delay, 0.6)
}

/// A holiday forecast card: the holiday's colours and motifs around a framed card with the
/// forecast for that day (icon, high, rain chance, conditions).
pub fn holiday(id: &str) -> Option<GraphicDoc> {
    let h = holidays::get(id)?;
    let b = |field: &str| format!("{{{{holiday.{id}.{field}}}}}");
    let mut doc = GraphicDoc::new(&format!("{} Forecast", h.name), "Holidays", Some(Fill::linear(h.bg.0, h.bg.1, 135.0)));
    // Motifs: two loose columns either side of the card, larger toward the bottom.
    let slots = [
        (90.0, 120.0, 150.0, -12.0, 0.9),
        (230.0, 420.0, 120.0, 10.0, 0.75),
        (70.0, 640.0, 210.0, -6.0, 0.95),
        (250.0, 840.0, 130.0, 14.0, 0.7),
        (1650.0, 110.0, 150.0, 12.0, 0.9),
        (1560.0, 400.0, 120.0, -10.0, 0.75),
        (1640.0, 620.0, 210.0, 6.0, 0.95),
        (1520.0, 850.0, 130.0, -14.0, 0.7),
    ];
    for (i, (x, y, size, rot, op)) in slots.iter().enumerate() {
        let (name, color) = h.motifs[i % h.motifs.len()];
        doc.layers.push(motif(name, color, *x, *y, *size, *rot, *op, 0.1 + i as f32 * 0.06));
    }
    // Framed card.
    let (cx, cy, cw, ch) = (420.0, 150.0, 1080.0, 780.0);
    doc.layers.push(Layer::rect("Card frame", cx - 14.0, cy - 14.0, cw + 28.0, ch + 28.0, Fill::linear(h.frame, h.frame, 0.0), 30.0).with_anim(AnimKind::Zoom, 0.0, 0.55));
    doc.layers.push(Layer::rect("Card", cx, cy, cw, ch, Fill::linear(h.card, "#05070D", 90.0), 22.0).with_anim(AnimKind::Zoom, 0.05, 0.55));
    doc.layers.push(Layer::rect("Card sheen", cx, cy, cw, 8.0, Fill::solid(h.frame), 0.0).with_anim(AnimKind::Grow, 0.3, 0.5));
    doc.layers.push(caps(shadowed(text("Holiday name", (cx + 40.0, cy + 40.0, cw - 80.0, 120.0), h.name, DISPLAY, 96.0, Weight::Black, h.accent, Align::Center)), 2.0).with_anim(AnimKind::Rise, 0.25, 0.5));
    doc.layers.push(text("Holiday date", (cx + 40.0, cy + 160.0, cw - 80.0, 50.0), &b("date"), BODY, 34.0, Weight::SemiBold, "#C9D2E0", Align::Center).with_anim(AnimKind::Fade, 0.35, 0.4));
    // Icon and rain chance on the left, the high on the right.
    doc.layers.push(Layer::icon("Holiday icon", cx + 90.0, cy + 240.0, 320.0, &b("icon"), &b("daypart")).with_anim(AnimKind::Elastic, 0.45, 0.8));
    doc.layers.push(caps(text("Rain chance", (cx + 60.0, cy + 560.0, 380.0, 60.0), &b("pop_line"), DISPLAY, 40.0, Weight::Bold, h.accent, Align::Center), 1.0).with_anim(AnimKind::Rise, 0.6, 0.4));
    doc.layers.push(caps(text("High label", (cx + 500.0, cy + 250.0, 520.0, 50.0), &b("hi_label"), DISPLAY, 40.0, Weight::Bold, "#C9D2E0", Align::Center), 4.0).with_anim(AnimKind::Fade, 0.5, 0.4));
    doc.layers.push(shadowed(text("High", (cx + 500.0, cy + 280.0, 520.0, 300.0), &b("hi"), DISPLAY, 280.0, Weight::Black, h.accent, Align::Center)).with_anim(AnimKind::Zoom, 0.55, 0.6));
    doc.layers.push(Layer::rect("Condition rule", cx + 80.0, cy + 640.0, cw - 160.0, 3.0, Fill::solid(h.frame), 0.0).with_anim(AnimKind::Grow, 0.65, 0.5));
    doc.layers.push(wrapped(caps(text("Condition", (cx + 60.0, cy + 660.0, cw - 120.0, 100.0), &b("condition"), DISPLAY, 54.0, Weight::Black, "#FFFFFF", Align::Center), 1.0)).with_anim(AnimKind::Rise, 0.7, 0.45));
    doc.layers.push(text("Station", (60.0, 1010.0, 900.0, 50.0), "{{station}}", BODY, 30.0, Weight::Bold, "#FFFFFF", Align::Left).with_anim(AnimKind::Fade, 0.8, 0.4));
    doc.layers.push(text("Source", (960.0, 1010.0, 900.0, 50.0), &b("source"), BODY, 26.0, Weight::Regular, "#C9D2E0", Align::Right).with_anim(AnimKind::Fade, 0.8, 0.4));
    doc.look = "modern".into();
    doc.duration = 8.0;
    Some(doc)
}

/// Weather Alert Day: a bold red card for a day with a real severe weather risk.
pub fn alert_day(day: u8) -> GraphicDoc {
    let p = format!("alertday{day}");
    let mut doc = GraphicDoc::new("Weather Alert Day", "Severe", Some(Fill::linear("#4A0508", "#1A0204", 160.0)));
    // Top band: station name and the title, on red and white bars.
    doc.layers.push(Layer::rect("Red band", 0.0, 60.0, 1920.0, 150.0, Fill::linear("#B3080F", "#E01B22", 0.0), 0.0).with_anim(AnimKind::Wipe, 0.0, 0.45));
    doc.layers.push(Layer::rect("White band", 0.0, 210.0, 1920.0, 140.0, Fill::solid("#FFFFFF"), 0.0).with_anim(AnimKind::Wipe, 0.1, 0.45));
    doc.layers.push(Layer::plate("Station block", 120.0, 40.0, 470.0, 330.0, Fill::linear("#7A0A10", "#D0161E", 135.0), 0.0, true).with_anim(AnimKind::Zoom, 0.15, 0.55));
    doc.layers.push(wrapped(caps(shadowed(text("Station", (140.0, 150.0, 430.0, 200.0), "{{station_short}}", DISPLAY, 84.0, Weight::Black, "#FFFFFF", Align::Center)), 1.0)).with_anim(AnimKind::Rise, 0.3, 0.45));
    doc.layers.push(caps(text("Title top", (640.0, 66.0, 1220.0, 140.0), "Weather Alert", DISPLAY, 118.0, Weight::Black, "#FFFFFF", Align::Center), 2.0).with_anim(AnimKind::SlideLeft, 0.25, 0.5));
    doc.layers.push(caps(text("Title bottom", (640.0, 214.0, 1220.0, 132.0), "Day", DISPLAY, 112.0, Weight::Black, "#C8101A", Align::Center), 6.0).with_anim(AnimKind::SlideLeft, 0.35, 0.5));
    // Body: when, then three plain lines.
    doc.layers.push(caps(shadowed(text("When", (100.0, 400.0, 1720.0, 230.0), &format!("{{{{{p}.when}}}}"), DISPLAY, 210.0, Weight::Black, "#FFFFFF", Align::Center)), 2.0).with_anim(AnimKind::Zoom, 0.5, 0.55));
    for i in 1..=3 {
        let y = 640.0 + (i - 1) as f32 * 112.0;
        doc.layers.push(shadowed(text(&format!("Line {i}"), (100.0, y, 1720.0, 104.0), &format!("{{{{{p}.line{i}}}}}"), DISPLAY, 84.0, Weight::Black, "#FFFFFF", Align::Center)).with_anim(AnimKind::Rise, 0.65 + i as f32 * 0.12, 0.45));
    }
    doc.layers.push(text("Source", (960.0, 1010.0, 900.0, 50.0), "Source: NWS Storm Prediction Center", BODY, 26.0, Weight::Regular, "#F2C9CB", Align::Right).with_anim(AnimKind::Fade, 1.0, 0.4));
    doc.look = "modern".into();
    doc.duration = 9.0;
    doc
}

/// A row of coloured scale chips (legend) above a risk map.
fn chips(doc: &mut GraphicDoc, name: &str, x: f32, y: f32, w: f32, chips: &[(&str, &str, &str)], delay: f32) {
    let cw = w / chips.len() as f32;
    for (i, (label, fill, ink)) in chips.iter().enumerate() {
        let cx = x + i as f32 * cw;
        doc.layers.push(Layer::rect(&format!("{name} chip {i}"), cx + 2.0, y, cw - 4.0, 46.0, Fill::solid(fill), 4.0).with_anim(AnimKind::Grow, delay + i as f32 * 0.04, 0.35));
        doc.layers.push(text(&format!("{name} chip {i} label"), (cx + 2.0, y, cw - 4.0, 46.0), label, DISPLAY, 28.0, Weight::Black, ink, Align::Center).with_anim(AnimKind::Fade, delay + 0.1 + i as f32 * 0.04, 0.3));
    }
}

// WPC flooding-rain levels 1-4 (words don't fit these small chips; codes like MRGL aren't for air).
const FLOOD_CHIPS: &[(&str, &str, &str)] = &[("1", "#5CB85C", "#0B1020"), ("2", "#F2E35B", "#0B1020"), ("3", "#E8473B", "#FFFFFF"), ("4", "#D24BD2", "#FFFFFF")];
const WIND_CHIPS: &[(&str, &str, &str)] = &[("5%", "#8B4726", "#FFFFFF"), ("15%", "#F2D21E", "#0B1020"), ("30%", "#E8473B", "#FFFFFF"), ("45%", "#E83EE8", "#FFFFFF")];
const HAIL_CHIPS: &[(&str, &str, &str)] = WIND_CHIPS;
const TORNADO_CHIPS: &[(&str, &str, &str)] = &[("2%", "#008B00", "#FFFFFF"), ("5%", "#8B4726", "#FFFFFF"), ("10%", "#F2D21E", "#0B1020"), ("15%", "#E8473B", "#FFFFFF")];
const RISK_CHIPS: &[(&str, &str, &str)] = &[("TSTM", "#C1E9C1", "#0B1020"), ("MRGL", "#66A366", "#FFFFFF"), ("SLGT", "#F6F67B", "#0B1020"), ("ENH", "#E6C27A", "#0B1020"), ("MDT", "#E67F7F", "#FFFFFF"), ("HIGH", "#FF66FF", "#FFFFFF")];

/// Four county maps side by side: flooding, wind, hail and tornado chances for the day.
pub fn risks_overview(day: u8) -> GraphicDoc {
    let mut doc = GraphicDoc::new("Severe Risks Overview", "Severe", Some(Fill::linear("#050B18", "#0E1E38", 160.0)));
    header(&mut doc, &format!("Severe Risks {{{{spc.day{day}.weekday}}}}"), "{{location}}");
    let panels: [(&str, &str, &[(&str, &str, &str)]); 4] = [("Flood", "flood", FLOOD_CHIPS), ("Wind", "wind", WIND_CHIPS), ("Hail", "hail", HAIL_CHIPS), ("Tornado", "tornado", TORNADO_CHIPS)];
    let (x0, pw, gap) = (60.0, 430.0, 20.0);
    for (i, (name, key, scale)) in panels.iter().enumerate() {
        let x = x0 + i as f32 * (pw + gap);
        let d = 0.2 + i as f32 * 0.12;
        doc.layers.push(Layer::rect(&format!("{name} head"), x, 190.0, pw, 76.0, Fill::solid("#FFFFFF"), 4.0).with_anim(AnimKind::Grow, d, 0.4));
        doc.layers.push(caps(text(&format!("{name} title"), (x, 192.0, pw, 72.0), name, DISPLAY, 56.0, Weight::Black, "#0B1020", Align::Center), 1.0).with_anim(AnimKind::Rise, d + 0.05, 0.4));
        chips(&mut doc, name, x, 272.0, pw, scale, d + 0.1);
        doc.layers.push(Layer::risk_map(&format!("{name} map"), x, 330.0, pw, 560.0, &format!("{key}:{day}")).with_anim(AnimKind::Fade, d + 0.2, 0.5));
    }
    footer(&mut doc, "{{station}}", "Source: NWS Storm Prediction Center and Weather Prediction Center");
    doc.look = "modern".into();
    doc
}

/// One large county map of the overall severe risk with its scale.
pub fn risk_map_big(day: u8) -> GraphicDoc {
    let mut doc = GraphicDoc::new("Severe Risk Map", "Severe", Some(Fill::linear("#050B18", "#0E1E38", 160.0)));
    header(&mut doc, &format!("Severe Risk {{{{spc.day{day}.weekday}}}}"), &format!("{{{{spc.day{day}.home.number}}}}"));
    doc.layers.push(Layer::risk_map("Risk map", 80.0, 170.0, 1180.0, 790.0, &format!("categorical:{day}")).with_anim(AnimKind::Fade, 0.2, 0.5));
    doc.layers.push(panel("Scale panel", 1300.0, 190.0, 540.0, 760.0).with_anim(AnimKind::Fade, 0.3, 0.4));
    doc.layers.push(caps(text("Scale title", (1320.0, 210.0, 500.0, 60.0), "Risk levels", DISPLAY, 44.0, Weight::Black, "#FFFFFF", Align::Left), 2.0).with_anim(AnimKind::Rise, 0.35, 0.4));
    let names = ["Thunderstorms", "1 Marginal", "2 Slight", "3 Enhanced", "4 Moderate", "5 High"];
    for (i, (_, fill, _)) in RISK_CHIPS.iter().enumerate() {
        let y = 300.0 + i as f32 * 104.0;
        let d = 0.4 + i as f32 * 0.07;
        doc.layers.push(Layer::rect(&format!("Level {i} swatch"), 1330.0, y, 80.0, 80.0, Fill::solid(fill), 10.0).with_anim(AnimKind::Zoom, d, 0.4));
        doc.layers.push(caps(text(&format!("Level {i} name"), (1430.0, y + 8.0, 390.0, 64.0), names[i], DISPLAY, 40.0, Weight::Bold, "#FFFFFF", Align::Left), 1.0).with_anim(AnimKind::Rise, d, 0.4));
    }
    footer(&mut doc, &format!("{{{{location}}}}: {{{{spc.day{day}.home}}}}"), "Source: NWS Storm Prediction Center");
    doc.look = "modern".into();
    doc
}

/// Indiana's Winter Storm Warning snow criteria by county (6, 5 or 4 inches).
pub fn wsw_criteria() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Winter Storm Warning Criteria", "Winter", Some(Fill::linear("#06142A", "#123058", 160.0)));
    header(&mut doc, "Winter Storm Warning", "Snow needed in 12 hours");
    doc.layers.push(Layer::risk_map("Criteria map", 120.0, 165.0, 760.0, 800.0, "wsw").with_anim(AnimKind::Fade, 0.2, 0.5));
    let rows = [("6 in", "Northern third", crate::gfx::riskmap::WSW_COLORS[0].1), ("5 in", "Central Indiana", crate::gfx::riskmap::WSW_COLORS[1].1), ("4 in", "Far south and southeast", crate::gfx::riskmap::WSW_COLORS[2].1)];
    for (i, (inches, area, color)) in rows.iter().enumerate() {
        let y = 250.0 + i as f32 * 220.0;
        let d = 0.35 + i as f32 * 0.12;
        doc.layers.push(panel(&format!("Row {i} panel"), 960.0, y, 880.0, 190.0).with_anim(AnimKind::Rise, d, 0.45));
        doc.layers.push(Layer::rect(&format!("Row {i} swatch"), 990.0, y + 25.0, 140.0, 140.0, Fill::solid(color), 18.0).with_anim(AnimKind::Zoom, d + 0.05, 0.45));
        doc.layers.push(shadowed(text(&format!("Row {i} inches"), (990.0, y + 25.0, 140.0, 140.0), inches, DISPLAY, 56.0, Weight::Black, "#0B1020", Align::Center)).with_anim(AnimKind::Fade, d + 0.1, 0.4));
        doc.layers.push(caps(text(&format!("Row {i} area"), (1160.0, y + 50.0, 660.0, 90.0), area, DISPLAY, 50.0, Weight::Bold, "#FFFFFF", Align::Left), 1.0).with_anim(AnimKind::Rise, d + 0.1, 0.4));
    }
    footer(&mut doc, "{{station}}", "Source: NWS Indianapolis. County bands approximate; other NWS offices may differ");
    doc.look = "modern".into();
    doc
}

/// Storm timing: counties shaded by when the moving storms reach them, in hour bands.
pub fn storm_timing() -> GraphicDoc {
    let mut doc = GraphicDoc::new("Storm Timing", "Severe", Some(Fill::linear("#050B18", "#0E1E38", 160.0)));
    header(&mut doc, "Storm Timing", "{{location}}");
    doc.layers.push(Layer::risk_map("Timing map", 80.0, 170.0, 1180.0, 790.0, "timing").with_anim(AnimKind::Fade, 0.2, 0.5));
    doc.layers.push(panel("Legend panel", 1300.0, 190.0, 540.0, 760.0).with_anim(AnimKind::Fade, 0.3, 0.4));
    doc.layers.push(caps(text("Legend title", (1320.0, 210.0, 500.0, 60.0), "Storms arrive", DISPLAY, 44.0, Weight::Black, "#FFFFFF", Align::Left), 2.0).with_anim(AnimKind::Rise, 0.35, 0.4));
    for i in 0..3 {
        let y = 310.0 + i as f32 * 150.0;
        let d = 0.4 + i as f32 * 0.1;
        doc.layers.push(Layer::rect(&format!("Band {i} swatch"), 1330.0, y, 90.0, 110.0, Fill::solid(crate::gfx::riskmap::TIMING_COLORS[i]), 12.0).with_anim(AnimKind::Zoom, d, 0.4));
        doc.layers.push(wrapped(caps(text(&format!("Band {i} time"), (1440.0, y + 6.0, 380.0, 100.0), &format!("{{{{timing.band{}}}}}", i + 1), DISPLAY, 40.0, Weight::Bold, "#FFFFFF", Align::Left), 1.0)).with_anim(AnimKind::Rise, d, 0.4));
    }
    doc.layers.push(wrapped(text("Legend note", (1330.0, 770.0, 490.0, 160.0), "Based on the current movement of storms on radar. Timing can change.", BODY, 28.0, Weight::SemiBold, "#B9C6DA", Align::Left)).with_anim(AnimKind::Fade, 0.8, 0.4));
    footer(&mut doc, "{{station}}", "Radar: LibreWXR storm cells");
    doc.look = "modern".into();
    doc
}

/// Storms Possible with the county risk map: the map beside the level and hazard meters.
pub fn severe_day_map(day: u8) -> GraphicDoc {
    let p = if day == 1 { "spc.home".to_string() } else { format!("spc.day{day}.home") };
    let mut doc = GraphicDoc::new("Severe Storms Possible", "Severe", Some(Fill::linear("#050E1C", "#12305A", 155.0)));
    header(&mut doc, &format!("Severe Storms Possible {{{{spc.day{day}.weekday}}}}"), &format!("{{{{{p}.number}}}}"));
    doc.layers.push(Layer::risk_map("Risk map", 60.0, 170.0, 1000.0, 790.0, &format!("categorical:{day}")).with_anim(AnimKind::Fade, 0.2, 0.5));
    let hazards = [("Tornado", "tornado"), ("Damaging wind", "wind"), ("Large hail", "hail"), ("Flash flooding", "flood")];
    for (i, (name, key)) in hazards.iter().enumerate() {
        let y = 190.0 + i as f32 * 150.0;
        let d = 0.35 + i as f32 * 0.1;
        doc.layers.push(panel(&format!("{name} row"), 1100.0, y, 760.0, 132.0).with_anim(AnimKind::Rise, d, 0.45));
        doc.layers.push(caps(text(&format!("{name} name"), (1130.0, y + 10.0, 700.0, 56.0), name, DISPLAY, 42.0, Weight::Bold, "#FFFFFF", Align::Left), 1.0).with_anim(AnimKind::Rise, d, 0.45));
        doc.layers.push(Layer::meter(&format!("{name} meter"), 1130.0, y + 74.0, 700.0, 40.0, &format!("{{{{{p}.{key}.level}}}}"), "blocks").with_anim(AnimKind::Fade, d + 0.1, 0.3));
    }
    doc.layers.push(Layer::plate("Timing plate", 1100.0, 800.0, 760.0, 80.0, Fill::solid("#F2B705"), 0.0, true).with_anim(AnimKind::Grow, 0.8, 0.5));
    doc.layers.push(wrapped(caps(text("Timing label", (1120.0, 804.0, 720.0, 72.0), "{{severe.window}}", DISPLAY, 32.0, Weight::Black, "#10131A", Align::Center), 1.0)).with_anim(AnimKind::Rise, 0.85, 0.4));
    footer(&mut doc, "{{station}}", "Source: NWS Storm Prediction Center");
    doc.look = "modern".into();
    doc
}

/// The map area of [`winter_map`] graphics (x, y, w, h), for painting counties by click.
pub const WINTER_MAP_RECT: (f32, f32, f32, f32) = (50.0, 165.0, 1200.0, 805.0);

/// A full-screen winter map (Winter Studio) in the station's own look: the station header and
/// footer, the state map on the left and the legend as swatch rows on a side panel.
pub fn winter_map(map: &crate::winter::WinterMap) -> GraphicDoc {
    let mut doc = GraphicDoc::new(&map.title, "Winter", Some(Fill::linear("#050B18", "#0E1E38", 160.0)));
    // The legend heading carries the date; the header says the window, so the date isn't
    // printed twice (CWXB-84).
    let right = if !map.subtitle.trim().is_empty() {
        map.subtitle.clone()
    } else if map.start > 0 {
        // A window that starts later: the legend heading names the days.
        format!("{} hours", map.hours)
    } else {
        format!("Next {} hours", map.hours)
    };
    header(&mut doc, &map.title, &right);
    let (mx, my, mw, mh) = WINTER_MAP_RECT;
    doc.layers.push(Layer::risk_map("Winter map", mx, my, mw, mh, &format!("winter:{}", map.id)).with_anim(AnimKind::Fade, 0.2, 0.5));
    // Legend: a side panel with a heading and one swatch row per band.
    let (lx, ly, lw, lh) = (1290.0, 185.0, 570.0, 770.0);
    doc.layers.push(panel("Legend panel", lx, ly, lw, lh).with_anim(AnimKind::Fade, 0.3, 0.4));
    doc.layers.push(wrapped(caps(text("Period", (lx + 30.0, ly + 22.0, lw - 60.0, 64.0), &map.period, DISPLAY, 36.0, Weight::Black, "$primary", Align::Left), 1.0)).with_anim(AnimKind::Rise, 0.35, 0.4));
    doc.layers.push(Layer::rect("Legend rule", lx + 30.0, ly + 96.0, lw - 60.0, 3.0, Fill::solid("rgba(255,255,255,0.18)"), 0.0).with_anim(AnimKind::Grow, 0.4, 0.4));
    // Snow-and-ice maps add a row for the hatched glaze.
    let mut rows: Vec<crate::winter::Band> = map.bands.clone();
    if map.kind.with_ice() {
        rows.push(crate::winter::Band { label: "Ice (pink stripes)".into(), color: crate::winter::ICE_HATCH.into(), min: 0.0 });
    }
    let n = rows.len().max(1) as f32;
    let gap = 18.0;
    let top = ly + 122.0;
    let note = map.empty_note();
    let avail = lh - 150.0 - if note.is_some() { 110.0 } else { 0.0 };
    let rh = ((avail - gap * (n - 1.0)) / n).min(110.0);
    for (i, band) in rows.iter().enumerate() {
        let y = top + i as f32 * (rh + gap);
        let d = 0.45 + i as f32 * 0.07;
        let sw = rh.min(96.0);
        doc.layers.push(Layer::rect(&format!("Band {} swatch", i + 1), lx + 30.0, y + (rh - sw) / 2.0, sw * 1.5, sw, Fill::solid(&band.color), 10.0).with_anim(AnimKind::Zoom, d, 0.4));
        let size = (rh * 0.46).min(46.0);
        doc.layers.push(wrapped(caps(text(&format!("Band {} label", i + 1), (lx + 60.0 + sw * 1.5, y + (rh - size * 1.3) / 2.0, lw - 90.0 - sw * 1.5, size * 1.3), &band.label, DISPLAY, size, Weight::Bold, "$text", Align::Left), 1.0)).with_anim(AnimKind::Rise, d, 0.4));
    }
    // Nothing reaches the lowest band anywhere: say so instead of a bare legend.
    if let Some(note) = note {
        let y = top + rows.len() as f32 * (rh + gap);
        {
            doc.layers.push(wrapped(text("Empty note", (lx + 30.0, y, lw - 60.0, (ly + lh - y - 20.0).min(120.0)), note, BODY, 36.0, Weight::SemiBold, "$muted", Align::Left)).with_anim(AnimKind::Fade, 0.7, 0.4));
        }
    }
    footer(&mut doc, "{{station}}", &map.source_text());
    crate::gfx::look::apply(&mut doc, &map.look);
    doc.duration = 10.0;
    doc
}
