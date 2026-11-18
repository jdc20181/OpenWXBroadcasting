//! The daily forecast post. Keeps every feature of the current forecast graphic and the
//! Social Creator: six themes, landscape / square / story sizes, eyebrow + location header,
//! headline banner with severe / alert / accent / neutral stripe, day cards with date,
//! holiday names and icons, high / low, sponsor "Brought to you by" and "Powered by" blocks,
//! brand watermark with opacity, the accent glow, and the optional temperature graph.

use crate::gfx::{css, Align, Canvas, Rgba, Style, WHITE};
use crate::{Brand, Sponsor};
use resvg::tiny_skia::PathBuilder;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Theme {
    pub start: String,
    pub mid: String,
    pub end: String,
    pub accent: String,
    pub accent_strong: String,
    pub panel: String,
    pub border: String,
    pub ink: String,
    pub heading: String,
    pub muted: String,
    pub sun: String,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            start: "#091221".into(),
            mid: "#0d1729".into(),
            end: "#121d33".into(),
            accent: "#6de4ff".into(),
            accent_strong: "#1fb9d8".into(),
            panel: "rgba(10, 19, 37, 0.78)".into(),
            border: "rgba(155, 180, 214, 0.16)".into(),
            ink: "#e7ecf6".into(),
            heading: "#ffffff".into(),
            muted: "#9aa6ba".into(),
            sun: "#ffd166".into(),
        }
    }
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct Day {
    /// "Today", "Mon", "Christmas" ...
    pub label: String,
    /// "Nov 14"
    pub date: String,
    /// Remix icon name (Node maps the NWS short forecast to it).
    pub icon: String,
    pub description: String,
    pub high: Option<i32>,
    pub low: Option<i32>,
    pub holiday: bool,
    /// Chance of rain / precipitation, percent (Broadcast design shows "RAIN 30%").
    pub rain: Option<i32>,
}

/// One city of a regional forecast (NWS Tabular State Forecast): a row of the table.
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct City {
    pub name: String,
    /// The home city is highlighted.
    pub home: bool,
    /// One entry per column in `Forecast::days` (which then only carries the column labels).
    pub days: Vec<Day>,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct Forecast {
    pub design: String,
    /// landscape (1280x720) | square (1080x1080) | story (1080x1920)
    pub ratio: String,
    pub theme: Theme,
    pub eyebrow: String,
    pub location: String,
    pub headline: String,
    pub description: String,
    /// severe | alert | accent | neutral
    pub banner_type: String,
    pub days: Vec<Day>,
    pub show_graph: bool,
    /// 0-100, like the Social Creator slider.
    pub watermark_opacity: f32,
    pub brand: Brand,
    /// "Brought to you by" first, then "Powered by" (either may be absent).
    pub sponsors: Vec<Sponsor>,
    /// Regional forecast: a city table instead of the home forecast (Broadcast design).
    pub cities: Vec<City>,
    /// Footer credit in the Broadcast design.
    pub source: String,
    /// Broadcast title bar ("Your Station 7-Day Outlook"); blank = "N-DAY FORECAST".
    pub title: String,
}

impl Default for Forecast {
    fn default() -> Self {
        Forecast {
            design: "modern".into(),
            ratio: "landscape".into(),
            theme: Theme::default(),
            eyebrow: "DAILY OUTLOOK".into(),
            location: String::new(),
            headline: String::new(),
            description: String::new(),
            banner_type: "accent".into(),
            days: vec![],
            show_graph: true,
            watermark_opacity: 45.0,
            brand: Brand::default(),
            sponsors: vec![],
            cities: vec![],
            source: String::new(),
            title: String::new(),
        }
    }
}

pub fn size(ratio: &str) -> (f32, f32) {
    match ratio {
        "square" => (1080.0, 1080.0),
        "story" => (1080.0, 1920.0),
        _ => (1280.0, 720.0),
    }
}

pub(crate) struct Pal {
    pub(crate) accent: Rgba,
    pub(crate) strong: Rgba,
    pub(crate) panel: Rgba,
    pub(crate) border: Rgba,
    pub(crate) ink: Rgba,
    pub(crate) heading: Rgba,
    pub(crate) muted: Rgba,
    pub(crate) sun: Rgba,
    pub(crate) light: bool,
}

pub(crate) fn palette(t: &Theme) -> Pal {
    let d = Theme::default();
    let c = |v: &str, dv: &str| css(v, css(dv, WHITE));
    let mid = c(&t.mid, &d.mid);
    Pal {
        accent: c(&t.accent, &d.accent),
        strong: c(&t.accent_strong, &d.accent_strong),
        panel: c(&t.panel, &d.panel),
        border: c(&t.border, &d.border),
        ink: c(&t.ink, &d.ink),
        heading: c(&t.heading, &d.heading),
        muted: c(&t.muted, &d.muted),
        sun: c(&t.sun, &d.sun),
        light: mid.luminance() > 0.5,
    }
}

/// Text sizes per look. "modern" is the new default; "classic" keeps today's proportions.
struct Look {
    modern: bool,
    eyebrow: f32,
    location: f32,
    headline: f32,
    body: f32,
    label: f32,
    date: f32,
    icon: f32,
    desc: f32,
    high: f32,
    low: f32,
}

fn look(design: &str, ratio: &str) -> Look {
    let big = ratio != "landscape";
    let modern = design != "classic";
    let k = if big { 1.2 } else { 1.0 };
    if modern {
        Look { modern, eyebrow: 18.0 * k, location: 34.0 * k, headline: 32.0 * k, body: 21.0 * k, label: 25.0 * k, date: 16.0 * k, icon: 64.0 * k, desc: 19.0 * k, high: 58.0 * k, low: 30.0 * k }
    } else {
        Look { modern, eyebrow: 16.0 * k, location: 28.0 * k, headline: 28.0 * k, body: 19.0 * k, label: 24.0 * k, date: 14.0 * k, icon: 60.0 * k, desc: 17.0 * k, high: 48.0 * k, low: 24.0 * k }
    }
}

pub fn draw(cv: &mut Canvas, f: &Forecast) {
    // The Broadcast look (and every regional table) is drawn by its own module.
    if f.design == "broadcast" || !f.cities.is_empty() {
        crate::broadcast::draw(cv, f);
        return;
    }
    let (w, h) = (cv.w, cv.h);
    let p = palette(&f.theme);
    let lk = look(&f.design, &f.ratio);
    let t = &f.theme;
    let d = Theme::default();
    let bg = |v: &str, dv: &str| css(v, css(dv, WHITE));
    cv.gradient_rrect(0.0, 0.0, w, h, 0.0, &[(0.0, bg(&t.start, &d.start)), (0.48, bg(&t.mid, &d.mid)), (1.0, bg(&t.end, &d.end))], true);
    cv.glow(w - 40.0, 40.0, 300.0, p.accent.alpha(if p.light { 0.10 } else { 0.16 }));

    let pad = if f.ratio == "landscape" { 40.0 } else { 56.0 };
    let mut y = header(cv, f, &p, &lk, pad, w);
    y = banner(cv, f, &p, &lk, pad, y + if f.ratio == "landscape" { 22.0 } else { 34.0 }, w - pad * 2.0);
    let gap = if f.ratio == "landscape" { 20.0 } else { 30.0 };
    cv.dashed_line(pad, y + gap, w - pad, 1.5, 7.0, p.border.alpha(1.6));
    let top = y + gap * 2.0;
    let days: Vec<&Day> = f.days.iter().collect();
    if days.is_empty() {
        return;
    }
    match f.ratio.as_str() {
        "story" => story_rows(cv, f, &p, &lk, &days, pad, top, w - pad * 2.0, h - pad - top),
        "square" => {
            let first = if days.len() > 4 { days.len().div_ceil(2) } else { days.len() };
            let rows: Vec<&[&Day]> = if first < days.len() { vec![&days[..first], &days[first..]] } else { vec![&days[..]] };
            let n = rows.len() as f32;
            let row_h = (h - pad - top - 20.0 * (n - 1.0)) / n;
            for (i, row) in rows.iter().enumerate() {
                let cols = first.max(1);
                card_row(cv, f, &p, &lk, row, cols, pad, top + i as f32 * (row_h + 20.0), w - pad * 2.0, row_h);
            }
        }
        _ => card_row(cv, f, &p, &lk, &days, days.len(), pad, top, w - pad * 2.0, h - pad - top),
    }
}

/// Eyebrow, location, sponsors and watermark. Returns the y below the header.
fn header(cv: &mut Canvas, f: &Forecast, p: &Pal, lk: &Look, pad: f32, w: f32) -> f32 {
    // Watermark logo top-right.
    let logo = 84.0 * if f.ratio == "landscape" { 1.0 } else { 1.25 };
    let mut right = w - pad;
    if let Some(bytes) = f.brand.logo.as_deref().and_then(crate::gfx::b64) {
        let op = (f.watermark_opacity / 100.0).clamp(0.05, 1.0);
        if cv.image(&bytes, right - logo, pad - 6.0, logo, logo, op, Align::Center).is_some() {
            right -= logo + 18.0;
        }
    }

    let es = Style::new(lk.eyebrow, 800, p.accent).track(0.2);
    let ls = Style::new(lk.location, 900, p.heading).track(0.04);
    let text_y = pad + cv.cap_height(&es);
    cv.text(pad, text_y, &f.eyebrow.to_uppercase(), &es, Align::Left);
    let loc_y = text_y + 10.0 + cv.cap_height(&ls) + 4.0;
    let loc_w = cv.measure(&f.location.to_uppercase(), &ls);

    // Sponsor blocks between the title and the watermark (stacked for story / square).
    let narrow = f.ratio != "landscape";
    let sp_h = if narrow { 74.0 } else { 62.0 };
    let mut sx = right;
    let mut extra = 0.0;
    for (i, s) in f.sponsors.iter().enumerate().rev() {
        let free_left = if narrow { pad } else { pad + loc_w.max(cv.measure(&f.eyebrow, &es)) + 30.0 };
        let max_w = (sx - free_left).max(180.0);
        let used = sponsor_pill(cv, s, sx, pad - 4.0 + if narrow { extra } else { 0.0 }, sp_h, max_w, p, i == 0);
        if used > 0.0 {
            if narrow {
                extra += sp_h + 10.0;
            } else {
                sx -= used + 12.0;
            }
        }
    }
    let (ls, ll) = cv.fit(&f.location.to_uppercase(), &ls, lk.location * 0.7, if narrow { right - pad } else { (sx - pad - 10.0).max(200.0) }, 1);
    cv.text(pad, loc_y, ll.first().map(String::as_str).unwrap_or(""), &ls, Align::Left);
    let below = loc_y + 10.0;
    if narrow { below.max(pad + extra) } else { below }
}

/// The dark "Brought to you by NAME [logo]" pill, right-aligned at `right`. Returns its width.
fn sponsor_pill(cv: &mut Canvas, s: &Sponsor, right: f32, y: f32, h: f32, max_w: f32, p: &Pal, sponsored: bool) -> f32 {
    let logo = s.logo.as_deref().and_then(crate::gfx::b64);
    let name = s.name.trim();
    let label = if s.label.trim().is_empty() { if sponsored { "Brought to you by" } else { "Powered by" } } else { s.label.trim() };
    // Admins often type the name into the label too; don't print it twice.
    let show_name = !name.is_empty() && !label.to_lowercase().contains(&name.to_lowercase());
    if logo.is_none() && name.is_empty() {
        return 0.0;
    }
    let ls = Style::new(16.0, 800, WHITE.alpha(0.82)).track(0.12);
    let ns = Style::new(18.0, 900, p.accent.mix(WHITE, 0.15));
    let logo_box = if logo.is_some() { (h - 14.0) * 3.0 } else { 0.0 };
    let text_w = cv.measure(&label.to_uppercase(), &ls) + if show_name { cv.measure(name, &ns) + 8.0 } else { 0.0 };
    let w = (text_w + if logo.is_some() { logo_box + 14.0 } else { 0.0 } + 32.0).min(max_w);
    let x = right - w;
    cv.fill_rrect(x, y, w, h, 14.0, Rgba::rgb(0, 0, 0).alpha(0.5));
    cv.stroke_rrect(x, y, w, h, 14.0, 1.0, WHITE.alpha(0.1));
    let mut tx = x + 16.0;
    let base = y + (h + cv.cap_height(&ls)) / 2.0;
    let room = w - 32.0 - if logo.is_some() { logo_box + 14.0 } else { 0.0 };
    let (ls2, l2) = cv.fit(&label.to_uppercase(), &ls, 11.0, room, 1);
    tx += cv.text(tx, base, l2.first().map(String::as_str).unwrap_or(""), &ls2, Align::Left) + 8.0;
    if show_name && tx < x + 16.0 + room {
        let (ns2, n2) = cv.fit(name, &ns, 12.0, x + 16.0 + room - tx, 1);
        cv.text(tx, base, n2.first().map(String::as_str).unwrap_or(""), &ns2, Align::Left);
    }
    if let Some(b) = logo {
        // Logos sit on a white tile so dark logos stay readable.
        let lx = x + w - 16.0 - logo_box;
        cv.fill_rrect(lx - 4.0, y + 5.0, logo_box + 8.0, h - 10.0, 8.0, WHITE);
        cv.image(&b, lx, y + 9.0, logo_box, h - 18.0, 1.0, Align::Center);
    }
    w
}

fn banner(cv: &mut Canvas, f: &Forecast, p: &Pal, lk: &Look, x: f32, y: f32, w: f32) -> f32 {
    let (border, bgc, text, stripe) = match f.banner_type.as_str() {
        "severe" => (Rgba::rgb(239, 68, 68).alpha(0.4), Rgba::rgb(239, 68, 68).alpha(0.16), if p.light { Rgba::rgb(127, 29, 29) } else { Rgba::rgb(254, 226, 226) }, Rgba::rgb(239, 68, 68)),
        "alert" => (Rgba::rgb(245, 158, 11).alpha(0.4), Rgba::rgb(245, 158, 11).alpha(0.16), if p.light { Rgba::rgb(120, 53, 15) } else { Rgba::rgb(254, 243, 199) }, Rgba::rgb(245, 158, 11)),
        "neutral" => (p.border, if p.light { Rgba::rgb(0, 0, 0).alpha(0.04) } else { WHITE.alpha(0.05) }, p.ink, p.muted),
        _ => (p.accent.alpha(0.3), p.accent.alpha(0.1), p.ink, p.strong),
    };
    let inner = w - 36.0 - 30.0;
    let (hs, hl) = cv.fit(&f.headline, &Style::new(lk.headline, 900, p.heading).track(0.03), lk.headline * 0.7, inner, 2);
    let max_lines = if f.ratio == "landscape" { 2 } else { 4 };
    let (bs, bl) = if f.description.trim().is_empty() {
        (Style::new(lk.body, 600, text), vec![])
    } else {
        cv.fit(&f.description, &Style::new(lk.body, 600, text), lk.body * 0.82, inner, max_lines)
    };
    let pad_v = if lk.modern { 22.0 } else { 20.0 };
    let h = pad_v * 2.0 + hl.len() as f32 * hs.px * 1.08 + if bl.is_empty() { 0.0 } else { 10.0 + bl.len() as f32 * bs.px * 1.32 };
    cv.fill_rrect(x, y, w, h, 24.0, bgc);
    cv.stroke_rrect(x, y, w, h, 24.0, 1.2, border);
    cv.clip_rrect(x, y, w, h, 24.0);
    cv.fill_rect(x, y, 8.0, h, stripe);
    cv.unclip();
    let mut ty = cv.paragraph(x + 36.0, y + pad_v, &hl, &hs, 1.08, Align::Left);
    if !bl.is_empty() {
        ty = cv.paragraph(x + 36.0, ty + 10.0, &bl, &bs, 1.32, Align::Left);
    }
    let _ = ty;
    y + h
}

pub(crate) fn icon_color(p: &Pal, icon: &str) -> Rgba {
    if icon.starts_with("sun") || icon == "temp-hot-fill" { p.sun } else { p.accent }
}

/// Holiday days keep their own icon (flag / gift / sparkles) like the Social Creator.
pub(crate) fn day_icon(d: &Day) -> &str {
    if d.icon.is_empty() { "cloudy-fill" } else { &d.icon }
}

pub(crate) fn temps(d: &Day) -> (Option<i32>, Option<i32>) {
    match (d.high, d.low) {
        (Some(h), l) => (Some(h), l),
        (None, Some(l)) => (Some(l), None), // night-only period: the low is the big number
        _ => (None, None),
    }
}

#[allow(clippy::too_many_arguments)]
fn card_row(cv: &mut Canvas, f: &Forecast, p: &Pal, lk: &Look, days: &[&Day], cols: usize, x: f32, y: f32, w: f32, h: f32) {
    let gap = 16.0;
    let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
    // Centre a short last row.
    let used = days.len() as f32 * cw + (days.len() as f32 - 1.0) * gap;
    let x0 = x + (w - used) / 2.0;
    let k = (cw / 160.0).clamp(0.72, 1.25);
    let mut graph_pts = vec![];
    let graph_band = if f.show_graph && lk.modern && days.len() > 1 && f.ratio == "landscape" { 44.0 * k } else { 0.0 };
    for (i, d) in days.iter().enumerate() {
        let cx = x0 + i as f32 * (cw + gap);
        cv.fill_rrect(cx, y, cw, h, 24.0, p.panel);
        cv.stroke_rrect(cx, y, cw, h, 24.0, 1.2, p.border);
        let mid = cx + cw / 2.0;
        let inner = cw - 20.0;
        let mut ty = y + 18.0 * k;

        let label_color = if d.holiday { p.accent } else { p.heading };
        let (ls, ll) = cv.fit(&d.label.to_uppercase(), &Style::new(lk.label * k, 900, label_color).track(0.1), 14.0, inner, 1);
        ty += cv.cap_height(&ls);
        cv.text(mid, ty, ll.first().map(String::as_str).unwrap_or(""), &ls, Align::Center);
        if !d.date.is_empty() {
            let ds = Style::new(lk.date * k, 800, p.ink.alpha(0.75)).track(0.12);
            ty += 8.0 + cv.cap_height(&ds);
            cv.text(mid, ty, &d.date.to_uppercase(), &ds, Align::Center);
        }
        ty += 12.0 * k;
        cv.fill_rect(cx + 14.0, ty, cw - 28.0, 1.0, p.border);

        // Temperatures pinned to the bottom.
        let (big, small) = temps(d);
        // Never more than about a sixth of the card, so short rows (square) keep room for text.
        let hs = Style::new((lk.high * k).min(h * 0.16), 900, p.heading).track(-0.03);
        let lo = Style::new((lk.low * k).min(h * 0.085), 800, p.accent);
        let bottom = y + h - 16.0 * k;
        let small_h = if small.is_some() { cv.cap_height(&lo) + 8.0 } else { 0.0 };
        let big_base = bottom - small_h;
        let temp_top = big_base - cv.cap_height(&hs) - 14.0 * k;
        cv.fill_rect(cx + 14.0, temp_top, cw - 28.0, 1.0, p.border.alpha(0.7));
        if let Some(t) = big {
            cv.text(mid, big_base, &format!("{t}°"), &hs, Align::Center);
            if f.show_graph && graph_band > 0.0 {
                graph_pts.push((mid, t, temp_top));
            }
        }
        if let Some(t) = small {
            cv.text(mid, bottom, &format!("{t}°"), &lo, Align::Center);
        }

        // Icon and description share the space between.
        let space_top = ty + 10.0 * k;
        let space_bot = temp_top - graph_band - 8.0 * k;
        // Leave at least one line of description under the icon.
        let extra = if lk.modern { 18.0 * k + 10.0 } else { 10.0 };
        let one_line = (lk.desc * k * 0.8).max(13.0) * 1.25;
        let icon = (lk.icon * k).min(space_bot - space_top - extra - one_line).max(20.0);
        let icon_name = day_icon(d);
        if lk.modern {
            let tile = icon + 18.0 * k;
            cv.fill_rrect(mid - tile / 2.0, space_top, tile, tile, 18.0 * k, icon_color(p, icon_name).alpha(0.1));
            cv.icon(icon_name, mid - icon / 2.0, space_top + 9.0 * k, icon, icon_color(p, icon_name));
        } else {
            cv.icon(icon_name, mid - icon / 2.0, space_top, icon, icon_color(p, icon_name));
        }
        let desc_top = space_top + icon + if lk.modern { 18.0 * k + 10.0 } else { 10.0 };
        let room = space_bot - desc_top;
        let ds = Style::new(lk.desc * k, 700, p.muted);
        let lines = ((room / (ds.px * 1.2)).floor() as usize).clamp(1, 3);
        let (ds, dl) = cv.fit(&d.description, &ds, (lk.desc * k * 0.75).max(13.0), inner, lines);
        let block = dl.len() as f32 * ds.px * 1.2;
        cv.paragraph(mid, desc_top + ((room - block) / 2.0).max(0.0), &dl, &ds, 1.2, Align::Center);
    }

    // Temperature graph: the highs as a line through each card.
    if graph_pts.len() > 1 {
        let lo = graph_pts.iter().map(|g| g.1).min().unwrap() as f32;
        let hi = graph_pts.iter().map(|g| g.1).max().unwrap() as f32;
        let k2 = (cw / 160.0).clamp(0.72, 1.25);
        // The band reserved above the temperatures (same in every card of the row).
        let base = graph_pts.iter().map(|g| g.2).fold(f32::INFINITY, f32::min);
        let (gb, gt) = (base - 10.0 * k2, base - graph_band + 6.0 * k2);
        let to_y = |t: i32| if hi > lo { gb - (t as f32 - lo) / (hi - lo) * (gb - gt) } else { (gb + gt) / 2.0 };
        let mut pb = PathBuilder::new();
        for (i, (gx, t, _)) in graph_pts.iter().enumerate() {
            if i == 0 { pb.move_to(*gx, to_y(*t)) } else { pb.line_to(*gx, to_y(*t)) }
        }
        if let Some(path) = pb.finish() {
            cv.stroke_path(&path, 6.0, p.accent.alpha(0.18));
            cv.stroke_path(&path, 2.5, p.accent);
        }
        for (gx, t, _) in &graph_pts {
            cv.fill_rrect(gx - 5.0, to_y(*t) - 5.0, 10.0, 10.0, 5.0, p.accent);
            cv.fill_rrect(gx - 2.5, to_y(*t) - 2.5, 5.0, 5.0, 2.5, p.panel.alpha(1.0 / p.panel.a.max(0.01)).mix(Rgba::rgb(0, 0, 0), 0.2));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn story_rows(cv: &mut Canvas, f: &Forecast, p: &Pal, lk: &Look, days: &[&Day], x: f32, y: f32, w: f32, h: f32) {
    let _ = f;
    let n = days.len() as f32;
    let gap = 16.0;
    let rh = ((h - gap * (n - 1.0)) / n).min(200.0);
    for (i, d) in days.iter().enumerate() {
        let ry = y + i as f32 * (rh + gap);
        cv.fill_rrect(x, ry, w, rh, 24.0, p.panel);
        cv.stroke_rrect(x, ry, w, rh, 24.0, 1.2, p.border);
        let tile = (rh - 40.0).min(110.0);
        let icon_name = day_icon(d);
        cv.fill_rrect(x + 24.0, ry + (rh - tile) / 2.0, tile, tile, 20.0, icon_color(p, icon_name).alpha(0.1));
        cv.icon(icon_name, x + 24.0 + tile * 0.15, ry + (rh - tile) / 2.0 + tile * 0.15, tile * 0.7, icon_color(p, icon_name));

        // Temperatures on the right as two pills.
        let (big, small) = temps(d);
        let ts = Style::new(lk.low * 1.25, 900, p.heading);
        let mut rx = x + w - 24.0;
        let mut pills = vec![];
        if let Some(t) = small {
            pills.push(("moon-clear-fill", t, p.accent));
        }
        if let Some(t) = big {
            pills.push((if small.is_some() { "sun-fill" } else { "moon-clear-fill" }, t, if small.is_some() { p.sun } else { p.accent }));
        }
        for (icon, t, c) in pills {
            let txt = format!("{t}°");
            let pw = cv.measure(&txt, &ts) + 30.0 + 36.0;
            let ph = 64.0;
            let px = rx - pw;
            let py = ry + (rh - ph) / 2.0;
            cv.fill_rrect(px, py, pw, ph, 16.0, if p.light { Rgba::rgb(0, 0, 0).alpha(0.05) } else { WHITE.alpha(0.06) });
            cv.icon(icon, px + 14.0, py + (ph - 28.0) / 2.0, 28.0, c);
            cv.text(px + 50.0, py + (ph + cv.cap_height(&ts)) / 2.0, &txt, &ts, Align::Left);
            rx = px - 12.0;
        }

        let tx = x + 24.0 + tile + 24.0;
        let room = rx - tx - 10.0;
        let ls = Style::new(lk.label * 1.1, 900, if d.holiday { p.accent } else { p.heading }).track(0.08);
        let lw = cv.measure(&d.label.to_uppercase(), &ls);
        let label_base = ry + rh / 2.0 - 8.0;
        cv.text(tx, label_base, &d.label.to_uppercase(), &ls, Align::Left);
        if !d.date.is_empty() {
            let ds = Style::new(lk.date * 1.1, 700, p.ink.alpha(0.75)).track(0.08);
            cv.text(tx + lw + 12.0, label_base, &d.date.to_uppercase(), &ds, Align::Left);
        }
        let (ds, dl) = cv.fit(&d.description, &Style::new(lk.desc * 1.2, 700, p.muted), 18.0, room, 1);
        cv.text(tx, label_base + 14.0 + cv.cap_height(&ds), dl.first().map(String::as_str).unwrap_or(""), &ds, Align::Left);
    }
}
