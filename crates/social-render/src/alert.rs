//! The alert card (1200 x 630 design pixels, Facebook landscape). Node prepares every string
//! (times already in local time, threat parsed, county list resolved); this file only lays out
//! and draws. Three looks: "modern" (default), "classic" (today's layout, fixed) and "bold"
//! (full-bleed map with a broadcast lower third).

use crate::gfx::{css, text_on, Align, Canvas, Rgba, Style, INK_DARK, WHITE};
use crate::map::{self, MapData};
use crate::{Brand, Sponsor};
use serde::Deserialize;

pub const W: f32 = 1200.0;
pub const H: f32 = 630.0;

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Tag {
    pub label: String,
    pub value: String,
    /// danger | elevated | info
    pub level: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Row {
    /// hazard | source | impact | details | action
    pub kind: String,
    pub label: String,
    pub value: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Alert {
    pub design: String,
    pub event: String,
    /// The banner headline (same wording as today's cards).
    pub headline: String,
    pub banner_color: String,
    pub kind_color: String,
    /// Severity / urgency / certainty.
    pub chips: Vec<String>,
    pub until: String,
    pub issued: String,
    pub tags: Vec<Tag>,
    pub rows: Vec<Row>,
    /// Affected county names; collapsed to "A, B and 6 more" when they don't fit.
    pub area: Vec<String>,
    /// Used when `area` is empty (raw NWS areaDesc).
    pub area_text: String,
    pub brand: Brand,
    pub sponsor: Option<Sponsor>,
    pub map: MapData,
    pub test: bool,
}

const MUTED: Rgba = Rgba { r: 255, g: 255, b: 255, a: 0.62 };

fn row_icon(kind: &str) -> &'static str {
    match kind {
        "hazard" => "alert-fill",
        "source" => "radar-fill",
        "impact" => "error-warning-fill",
        "action" => "shield-check-fill",
        _ => "information-fill",
    }
}

fn tag_colors(level: &str) -> (Rgba, Rgba, Rgba) {
    match level {
        "danger" => (Rgba::rgb(220, 38, 38).alpha(0.30), Rgba::rgb(248, 113, 113).alpha(0.7), Rgba::rgb(254, 226, 226)),
        "elevated" => (Rgba::rgb(234, 88, 12).alpha(0.28), Rgba::rgb(251, 146, 60).alpha(0.7), Rgba::rgb(255, 237, 213)),
        _ => (Rgba::rgb(148, 163, 184).alpha(0.20), Rgba::rgb(148, 163, 184).alpha(0.45), Rgba::rgb(241, 245, 249)),
    }
}

pub fn draw(cv: &mut Canvas, a: &Alert) {
    match a.design.as_str() {
        "classic" => classic(cv, a),
        "bold" => bold(cv, a),
        _ => modern(cv, a),
    }
    if a.test {
        test_stamp(cv, map_rect(&a.design));
    }
}

/// Where each design puts its map: (x, y, w, h) in design pixels.
pub fn map_rect(design: &str) -> (f32, f32, f32, f32) {
    match design {
        "classic" => (420.0, 76.0, W - 420.0, H - 76.0),
        "bold" => (0.0, 0.0, W, 372.0),
        _ => (456.0, 96.0, W - 456.0, H - 96.0 - 48.0),
    }
}

fn kind_color(a: &Alert) -> Rgba {
    css(&a.kind_color, css(&a.banner_color, Rgba::rgb(192, 192, 192)))
}

fn banner_color(a: &Alert) -> Rgba {
    css(&a.banner_color, kind_color(a))
}

fn map_style(a: &Alert) -> map::Style2 {
    map::Style2 { kind_color: kind_color(a), county_line: WHITE.alpha(0.62), label_px: 15.0 }
}

// ----------------------------------------------------------------- modern ---

fn modern(cv: &mut Canvas, a: &Alert) {
    let (top, foot) = (96.0, 48.0);
    cv.fill_rect(0.0, 0.0, W, H, Rgba::rgb(11, 16, 24));

    // Banner in the NWS colour, text colour picked for contrast.
    let bc = banner_color(a);
    cv.gradient_rrect(0.0, 0.0, W, top, 0.0, &[(0.0, bc), (1.0, bc.mix(WHITE, 0.10))], false);
    let ink = text_on(bc);
    let sponsor_w = sponsor_box(cv, a.sponsor.as_ref(), W - 20.0, 12.0, top - 24.0, true);
    cv.icon("alert-fill", 26.0, (top - 40.0) / 2.0, 40.0, ink);
    let text_x = 80.0;
    let max_w = W - text_x - 28.0 - if sponsor_w > 0.0 { sponsor_w + 20.0 } else { 0.0 };
    let (s, lines) = cv.fit(&a.headline.to_uppercase(), &Style::new(36.0, 800, ink).track(0.02), 22.0, max_w, 2);
    let block = lines.len() as f32 * s.px * 1.08;
    cv.paragraph(text_x, (top - block) / 2.0, &lines, &s, 1.08, Align::Left);

    // Info column.
    let col_w = 456.0;
    cv.gradient_rrect(0.0, top, col_w, H - top - foot, 0.0, &[(0.0, Rgba::rgb(19, 25, 36)), (1.0, Rgba::rgb(12, 17, 26))], true);
    cv.fill_rect(0.0, top, 7.0, H - top - foot, kind_color(a));
    info_column(cv, a, 32.0, top + 20.0, col_w - 60.0, H - foot - 16.0);

    let (mx, my, mw, mh) = map_rect("modern");
    map::draw(cv, &a.map, mx, my, mw, mh, 0.0, &map_style(a));

    footer(cv, a, 0.0, H - foot, W, foot);
}

// ---------------------------------------------------------------- classic ---

fn classic(cv: &mut Canvas, a: &Alert) {
    let top = 76.0;
    cv.fill_rect(0.0, 0.0, W, H, Rgba::rgb(13, 17, 23));
    let bc = banner_color(a);
    cv.gradient_rrect(0.0, 0.0, W, top, 0.0, &[(0.0, bc), (1.0, bc.mix(WHITE, 0.08))], false);
    let ink = text_on(bc);
    let sponsor_w = sponsor_box(cv, a.sponsor.as_ref(), W - 16.0, 8.0, top - 16.0, false);
    let max_w = W - 48.0 - if sponsor_w > 0.0 { sponsor_w + 20.0 } else { 0.0 };
    let (s, lines) = cv.fit(&a.headline.to_uppercase(), &Style::new(30.0, 800, ink).track(0.04), 22.0, max_w, 2);
    let block = lines.len() as f32 * s.px * 1.1;
    cv.paragraph(24.0, (top - block) / 2.0, &lines, &s, 1.1, Align::Left);

    let col_w = 420.0;
    let foot = 56.0;
    cv.gradient_rrect(0.0, top, col_w, H - top, 0.0, &[(0.0, Rgba::rgb(17, 22, 32)), (1.0, Rgba::rgb(12, 16, 24))], true);
    cv.fill_rect(0.0, top, 5.0, H - top, kind_color(a));
    info_column(cv, a, 28.0, top + 20.0, col_w - 52.0, H - foot - 12.0);
    footer(cv, a, 0.0, H - foot, col_w, foot);

    let (mx, my, mw, mh) = map_rect("classic");
    map::draw(cv, &a.map, mx, my, mw, mh, 0.0, &map_style(a));
}

// ------------------------------------------------------------------- bold ---

fn bold(cv: &mut Canvas, a: &Alert) {
    // The map stops at the lower third so its radar credit stays visible.
    let (mx, my, mw, mh) = map_rect("bold");
    map::draw(cv, &a.map, mx, my, mw, mh, 0.0, &map_style(a));
    let kc = banner_color(a);

    // Brand chip top-left, sponsor top-right.
    let bx = 18.0;
    cv.fill_rrect(bx, 18.0, 300.0, 56.0, 12.0, Rgba::rgb(5, 8, 13).alpha(0.82));
    brand_lockup(cv, a, bx + 12.0, 18.0, 56.0, 276.0);
    sponsor_box(cv, a.sponsor.as_ref(), W - 18.0, 18.0, 74.0, true);

    // Lower third.
    let bar_y = 372.0;
    let bar_h = 86.0;
    let ink = text_on(kc);
    cv.fill_rect(0.0, bar_y, W, bar_h, kc);
    cv.icon("alert-fill", 28.0, bar_y + (bar_h - 46.0) / 2.0, 46.0, ink);
    let until = if a.until.is_empty() { String::new() } else { format!("UNTIL {}", a.until.to_uppercase()) };
    let us = Style::new(30.0, 800, ink).track(0.02);
    let uw = if until.is_empty() { 0.0 } else { cv.measure(&until, &us) };
    let (es, el) = cv.fit(&a.event.to_uppercase(), &Style::new(58.0, 900, ink).track(0.01), 30.0, W - 110.0 - uw - 50.0, 1);
    cv.text(90.0, bar_y + (bar_h + cv.cap_height(&es)) / 2.0, el.first().map(String::as_str).unwrap_or(""), &es, Align::Left);
    if !until.is_empty() {
        cv.text(W - 30.0, bar_y + (bar_h + cv.cap_height(&us)) / 2.0, &until, &us, Align::Right);
    }

    let panel_y = bar_y + bar_h;
    cv.fill_rect(0.0, panel_y, W, H - panel_y, Rgba::rgb(8, 12, 19).alpha(0.94));
    let mut y = panel_y + 18.0;
    // Headline in the panel, then threat tags, then the first row.
    let (hs, hl) = cv.fit(&a.headline, &Style::new(26.0, 700, WHITE), 21.0, W - 60.0, 2);
    y = cv.paragraph(30.0, y, &hl, &hs, 1.15, Align::Left) + 8.0;
    let mut x = 30.0;
    for t in &a.tags {
        let w = tag_chip(cv, x, y, t);
        x += w + 10.0;
        if x > W - 200.0 {
            break;
        }
    }
    if !a.tags.is_empty() {
        y += 46.0;
    }
    let area = area_line(cv, a, &Style::new(21.0, 700, MUTED), W - 60.0);
    if y < H - 30.0 && !area.is_empty() {
        let s = Style::new(21.0, 700, MUTED);
        cv.text(30.0, H - 22.0, &area, &s, Align::Left);
    }
    // Issued / site at the far right of the bottom line.
    let site = Style::new(18.0, 700, WHITE.alpha(0.55)).track(0.08);
    cv.text(W - 30.0, H - 22.0, &a.brand.site.to_uppercase(), &site, Align::Right);
}

// ----------------------------------------------------------------- shared ---

/// Event title, chips, until, threat tags, rows, affected area. Rows that don't fit are left
/// out (the caption always has the full text). `bottom` is where the area block must end.
fn info_column(cv: &mut Canvas, a: &Alert, x: f32, top: f32, w: f32, bottom: f32) {
    let mut y = top;
    let (es, el) = cv.fit(&a.event.to_uppercase(), &Style::new(48.0, 900, WHITE).track(0.01), 30.0, w, 2);
    y = cv.paragraph(x, y, &el, &es, 1.02, Align::Left) + 10.0;

    // Severity / urgency / certainty.
    if !a.chips.is_empty() {
        let s = Style::new(16.0, 800, WHITE.alpha(0.85)).track(0.1);
        let mut cx = x;
        for c in &a.chips {
            let t = c.to_uppercase();
            let tw = cv.measure(&t, &s) + 22.0;
            if cx + tw > x + w {
                break;
            }
            cv.fill_rrect(cx, y, tw, 28.0, 14.0, WHITE.alpha(0.08));
            cv.stroke_rrect(cx, y, tw, 28.0, 14.0, 1.0, WHITE.alpha(0.18));
            cv.text(cx + 11.0, y + (28.0 + cv.cap_height(&s)) / 2.0, &t, &s, Align::Left);
            cx += tw + 8.0;
        }
        y += 40.0;
    }

    if !a.until.is_empty() {
        let label = Style::new(15.0, 800, MUTED).track(0.16);
        cv.icon("time-fill", x, y, 17.0, MUTED);
        cv.text(x + 23.0, y + 14.0, "IN EFFECT UNTIL", &label, Align::Left);
        y += 22.0;
        let (us, ul) = cv.fit(&a.until, &Style::new(34.0, 800, WHITE), 24.0, w, 1);
        y = cv.paragraph(x, y, &ul, &us, 1.05, Align::Left);
        if !a.issued.is_empty() {
            let s = Style::new(17.0, 600, WHITE.alpha(0.55));
            cv.text(x, y + 17.0, &format!("Issued {}", a.issued), &s, Align::Left);
            y += 24.0;
        }
        y += 12.0;
    }

    // Threat tags wrap onto as many rows as fit.
    let mut cx = x;
    let mut used = false;
    for t in &a.tags {
        let tw = tag_width(cv, t);
        if cx + tw > x + w {
            cx = x;
            y += 44.0;
        }
        tag_chip(cv, cx, y, t);
        cx += tw + 8.0;
        used = true;
    }
    if used {
        y += 48.0;
    }

    // Area block at the bottom.
    let area_s = Style::new(21.0, 700, WHITE.alpha(0.9));
    let area = area_line(cv, a, &area_s, w);
    let area_top = if area.is_empty() { bottom } else { bottom - 52.0 };

    // Rows fill the space between.
    for r in &a.rows {
        if r.value.trim().is_empty() {
            continue;
        }
        let room = area_top - y - 22.0 - 8.0;
        let vs = Style::new(22.0, 600, WHITE.alpha(0.94));
        let max_lines = ((room / (vs.px * 1.18)).floor() as usize).min(3);
        if max_lines == 0 {
            break;
        }
        let ls = Style::new(14.0, 800, MUTED).track(0.16);
        cv.icon(row_icon(&r.kind), x, y, 17.0, kind_color(a).mix(WHITE, 0.35));
        cv.text(x + 23.0, y + 14.0, &r.label.to_uppercase(), &ls, Align::Left);
        y += 22.0;
        let (s, lines) = cv.fit(&r.value, &vs, 18.0, w, max_lines);
        y = cv.paragraph(x, y, &lines, &s, 1.18, Align::Left) + 10.0;
    }

    if !area.is_empty() {
        cv.fill_rect(x, area_top, w, 1.0, WHITE.alpha(0.1));
        let ls = Style::new(14.0, 800, MUTED).track(0.16);
        cv.icon("map-pin-2-fill", x, area_top + 9.0, 16.0, MUTED);
        cv.text(x + 22.0, area_top + 22.0, "AFFECTED AREA", &ls, Align::Left);
        cv.text(x, area_top + 48.0, &area, &area_s, Align::Left);
    }
}

fn tag_width(cv: &Canvas, t: &Tag) -> f32 {
    let ls = Style::new(13.0, 800, WHITE).track(0.1);
    let vs = Style::new(20.0, 800, WHITE);
    cv.measure(&t.label.to_uppercase(), &ls) + 8.0 + cv.measure(&t.value, &vs) + 24.0
}

fn tag_chip(cv: &mut Canvas, x: f32, y: f32, t: &Tag) -> f32 {
    let (bg, border, ink) = tag_colors(&t.level);
    let w = tag_width(cv, t);
    cv.fill_rrect(x, y, w, 36.0, 9.0, bg);
    cv.stroke_rrect(x, y, w, 36.0, 9.0, 1.2, border);
    let ls = Style::new(13.0, 800, ink.alpha(0.85)).track(0.1);
    let vs = Style::new(20.0, 800, ink);
    let base = y + (36.0 + cv.cap_height(&vs)) / 2.0;
    let lw = cv.text(x + 12.0, base, &t.label.to_uppercase(), &ls, Align::Left);
    cv.text(x + 12.0 + lw + 8.0, base, &t.value, &vs, Align::Left);
    w
}

/// "Dubois, Perry and 6 more", or the raw text fitted to one line on word boundaries.
fn area_line(cv: &Canvas, a: &Alert, s: &Style, w: f32) -> String {
    let names: Vec<&str> = a.area.iter().map(String::as_str).filter(|n| !n.is_empty()).collect();
    if names.is_empty() {
        if a.area_text.trim().is_empty() {
            return String::new();
        }
        let (_, lines) = cv.fit(&a.area_text, s, s.px, w, 1);
        return lines.into_iter().next().unwrap_or_default();
    }
    let all = join_names(&names);
    if cv.measure(&all, s) <= w {
        return all;
    }
    for keep in (1..names.len()).rev() {
        let text = format!("{} and {} more", names[..keep].join(", "), names.len() - keep);
        if cv.measure(&text, s) <= w {
            return text;
        }
    }
    format!("{} counties", names.len())
}

fn join_names(n: &[&str]) -> String {
    match n.len() {
        0 => String::new(),
        1 => n[0].to_string(),
        _ => format!("{} and {}", n[..n.len() - 1].join(", "), n[n.len() - 1]),
    }
}

fn brand_lockup(cv: &mut Canvas, a: &Alert, x: f32, y: f32, h: f32, w: f32) {
    let mut tx = x;
    if let Some(bytes) = a.brand.logo.as_deref().and_then(crate::gfx::b64) {
        let lh = (h - 10.0).min(44.0);
        if let Some((dw, _)) = cv.image(&bytes, x, y + (h - lh) / 2.0, 110.0, lh, 1.0, Align::Left) {
            tx += dw + 12.0;
            // The logo already carries the name: skip the text rather than cut it off.
            if x + w - tx < 210.0 {
                return;
            }
        }
    }
    let ns = Style::new(19.0, 800, WHITE).track(0.02);
    let ss = Style::new(15.0, 700, MUTED).track(0.08);
    let (ns, nl) = cv.fit(&a.brand.name, &ns, 15.0, x + w - tx, 1);
    let name = nl.into_iter().next().unwrap_or_default();
    if a.brand.sub.is_empty() {
        cv.text(tx, y + (h + cv.cap_height(&ns)) / 2.0, &name, &ns, Align::Left);
    } else {
        cv.text(tx, y + h / 2.0 - 2.0, &name, &ns, Align::Left);
        let (ss, sl) = cv.fit(&a.brand.sub.to_uppercase(), &ss, 12.0, x + w - tx, 1);
        cv.text(tx, y + h / 2.0 + 18.0, sl.first().map(String::as_str).unwrap_or(""), &ss, Align::Left);
    }
}

fn footer(cv: &mut Canvas, a: &Alert, x: f32, y: f32, w: f32, h: f32) {
    cv.fill_rect(x, y, w, h, Rgba::rgb(6, 9, 14));
    cv.fill_rect(x, y, w, 1.0, WHITE.alpha(0.07));
    let site = Style::new(17.0, 700, WHITE.alpha(0.6)).track(0.08);
    let site_w = if a.brand.site.is_empty() { 0.0 } else { cv.text(x + w - 22.0, y + (h + cv.cap_height(&site)) / 2.0, &a.brand.site.to_uppercase(), &site, Align::Right) };
    brand_lockup(cv, a, x + 24.0, y, h, w - 60.0 - site_w);
}

/// Sponsor in a white box (logos are made for white). Right-aligned at `right`.
/// Returns the width used (0 when there is no sponsor).
fn sponsor_box(cv: &mut Canvas, s: Option<&Sponsor>, right: f32, y: f32, h: f32, light: bool) -> f32 {
    let Some(s) = s else { return 0.0 };
    let logo = s.logo.as_deref().and_then(crate::gfx::b64);
    if logo.is_none() && s.name.trim().is_empty() {
        return 0.0;
    }
    let (bg, ink, muted) = if light {
        (WHITE, INK_DARK, Rgba::rgb(71, 85, 105))
    } else {
        (Rgba::rgb(3, 6, 12).alpha(0.75), WHITE, WHITE.alpha(0.65))
    };
    let label = if s.label.trim().is_empty() { "Brought to you by".to_string() } else { s.label.trim().to_string() };
    let ls = Style::new(13.0, 800, muted).track(0.12);
    let label_w = cv.measure(&label.to_uppercase(), &ls);
    let logo_h = h - 30.0;
    let logo_max_w: f32 = 210.0;
    let name_s = Style::new(logo_h.min(30.0), 800, ink);
    let content_w = match &logo {
        Some(_) => logo_max_w.min(label_w.max(120.0) + 60.0),
        None => cv.measure(&s.name, &name_s).min(logo_max_w),
    }
    .max(label_w);
    let w = content_w + 28.0;
    let x = right - w;
    cv.fill_rrect(x, y, w, h, 12.0, bg);
    if !light {
        cv.stroke_rrect(x, y, w, h, 12.0, 1.0, WHITE.alpha(0.15));
    }
    cv.text(x + w / 2.0, y + 19.0, &label.to_uppercase(), &ls, Align::Center);
    let drawn = logo.as_ref().and_then(|b| cv.image(b, x + 14.0, y + 24.0, content_w, logo_h, 1.0, Align::Center));
    if drawn.is_none() {
        let (ns, nl) = cv.fit(&s.name, &name_s, 14.0, content_w, 1);
        cv.text(x + w / 2.0, y + 24.0 + (logo_h + cv.cap_height(&ns)) / 2.0, nl.first().map(String::as_str).unwrap_or(""), &ns, Align::Center);
    }
    w
}

/// Practice cards look unmistakably like tests. The stamp sits on the map so it never covers
/// the card's text.
fn test_stamp(cv: &mut Canvas, (mx, my, mw, mh): (f32, f32, f32, f32)) {
    let yellow = Rgba::rgb(250, 204, 21);
    let text = "TEST  •  NO WEATHER ACTION NEEDED";
    let (s, lines) = cv.fit(text, &Style::new(30.0, 900, yellow).track(0.14), 16.0, mw - 80.0, 1);
    let line = lines.into_iter().next().unwrap_or_default();
    let tw = cv.measure(&line, &s);
    let (w, h) = (tw + 48.0, s.px * 1.8);
    let (x, y) = (mx + (mw - w) / 2.0, my + (mh - h) / 2.0);
    cv.fill_rrect(x, y, w, h, 10.0, Rgba::rgb(0, 0, 0).alpha(0.82));
    cv.stroke_rrect(x, y, w, h, 10.0, 3.0, yellow);
    cv.text(mx + mw / 2.0, y + (h + cv.cap_height(&s)) / 2.0, &line, &s, Align::Center);
}
