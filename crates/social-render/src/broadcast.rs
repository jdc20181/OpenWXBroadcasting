//! "Broadcast" forecast post: the station's 7-day forecast graphic, scaled to the
//! social sizes, with the post's extras: headline, sponsor and the temperature trend line.
//!
//! Slanted white title bar ("7-DAY FORECAST") over a blue location strip, the headline under
//! it, one card per day (name, Meteocons artwork, words, high, low, rain chance), the highs /
//! lows trend across the cards, and a footer with the station and "Forecast from the National
//! Weather Service". A regional forecast (`cities`) draws a city table in the same frame.

use crate::forecast::{day_icon, icon_color, palette, temps, Day, Forecast, Pal};
use crate::gfx::{css, Align, Canvas, Rgba, Style, WHITE};
use resvg::tiny_skia::PathBuilder;

const RAIN: Rgba = Rgba { r: 94, g: 200, b: 255, a: 1.0 };
const HIGH_LINE: Rgba = Rgba { r: 255, g: 179, b: 71, a: 1.0 };
const INK_DARK: Rgba = Rgba { r: 15, g: 23, b: 42, a: 1.0 };

/// Sizes per format, in design pixels.
struct Fmt {
    pad: f32,
    bar_h: f32,
    foot_h: f32,
    /// Overall text scale.
    k: f32,
    story: bool,
}

fn fmt(ratio: &str) -> Fmt {
    match ratio {
        "square" => Fmt { pad: 36.0, bar_h: 66.0, foot_h: 58.0, k: 1.1, story: false },
        "story" => Fmt { pad: 48.0, bar_h: 84.0, foot_h: 72.0, k: 1.35, story: true },
        _ => Fmt { pad: 30.0, bar_h: 56.0, foot_h: 48.0, k: 1.0, story: false },
    }
}

pub fn draw(cv: &mut Canvas, f: &Forecast) {
    let (w, h) = (cv.w, cv.h);
    let p = palette(&f.theme);
    let t = &f.theme;
    let d = crate::forecast::Theme::default();
    let bg = |v: &str, dv: &str| css(v, css(dv, WHITE));
    // Deep blue studio background: the theme's colours, a little brighter in the middle.
    cv.gradient_rrect(0.0, 0.0, w, h, 0.0, &[(0.0, bg(&t.start, &d.start)), (0.5, bg(&t.mid, &d.mid).mix(p.strong, 0.10)), (1.0, bg(&t.end, &d.end))], true);
    cv.glow(w * 0.18, h * 0.1, w * 0.45, p.strong.alpha(0.10));
    let m = fmt(&f.ratio);

    let mut y = title_block(cv, f, &p, &m, w);
    y = headline(cv, f, &p, &m, y, w);
    let bottom = h - m.foot_h - m.pad * 0.6;
    footer(cv, f, &p, &m, w, h);

    if !f.cities.is_empty() {
        city_table(cv, f, &p, &m, m.pad, y + 10.0 * m.k, w - m.pad * 2.0, bottom - y - 10.0 * m.k);
    } else if m.story || (f.ratio == "square" && f.days.len() > 5) {
        // Square with a full week: rows read better than seven tall, narrow columns.
        day_rows(cv, f, &p, &m, m.pad, y + 10.0 * m.k, w - m.pad * 2.0, bottom - y - 10.0 * m.k);
    } else {
        day_columns(cv, f, &p, &m, m.pad, y + 12.0 * m.k, w - m.pad * 2.0, bottom - y - 12.0 * m.k);
    }
}

/// A parallelogram (slanted left and right edges).
fn slant(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, skew: f32, c: Rgba) {
    let mut pb = PathBuilder::new();
    pb.move_to(x + skew, y);
    pb.line_to(x + w, y);
    pb.line_to(x + w - skew, y + h);
    pb.line_to(x, y + h);
    pb.close();
    if let Some(path) = pb.finish() {
        cv.fill_path(&path, c);
    }
}

/// Title bar, location strip and sponsor. Returns the y below them.
fn title_block(cv: &mut Canvas, f: &Forecast, p: &Pal, m: &Fmt, w: f32) -> f32 {
    let top = m.pad * 0.8;
    let strip_h = m.bar_h * 0.56;
    let block_h = m.bar_h + strip_h;
    // Sponsor (white box, logos are made for white) at the top right, as tall as the block.
    let sponsor_w = f.sponsors.first().map(|s| sponsor(cv, s, w - m.pad, top, block_h, m)).unwrap_or(0.0);
    let right = w - m.pad - if sponsor_w > 0.0 { sponsor_w + 16.0 * m.k } else { 0.0 };

    let n = f.days.len();
    let title = if !f.title.trim().is_empty() {
        f.title.trim().to_uppercase()
    } else if !f.cities.is_empty() {
        "REGIONAL FORECAST".to_string()
    } else if n > 1 {
        format!("{n}-DAY FORECAST")
    } else {
        "FORECAST".to_string()
    };
    let skew = m.bar_h * 0.32;
    slant(cv, m.pad, top, right - m.pad, m.bar_h, skew, Rgba::rgb(232, 236, 242));
    cv.fill_rect(m.pad + skew, top, right - m.pad - skew * 2.0, 2.0, WHITE);
    let ts = Style::new(m.bar_h * 0.7, 900, INK_DARK).track(0.02);
    let (ts, tl) = cv.fit(&title, &ts, m.bar_h * 0.45, right - m.pad - skew * 2.0 - 30.0, 1);
    cv.text(m.pad + skew + 22.0 * m.k, top + (m.bar_h + cv.cap_height(&ts)) / 2.0, tl.first().map(String::as_str).unwrap_or(""), &ts, Align::Left);

    // Location strip: the theme's strong accent fading out to the right.
    let ls = Style::new(strip_h * 0.52, 800, WHITE).track(0.12);
    let loc = f.location.to_uppercase();
    let lw = (cv.measure(&loc, &ls) + 60.0 * m.k).clamp(w * 0.3, right - m.pad - skew);
    let sy = top + m.bar_h;
    let mut pb = PathBuilder::new();
    pb.move_to(m.pad + 10.0 * m.k, sy);
    pb.line_to(m.pad + lw, sy);
    pb.line_to(m.pad + lw - strip_h * 0.5, sy + strip_h);
    pb.line_to(m.pad + 10.0 * m.k, sy + strip_h);
    pb.close();
    if let Some(path) = pb.finish() {
        cv.fill_path(&path, p.strong.mix(Rgba::rgb(20, 60, 140), 0.45).alpha(0.92));
    }
    let (ls, ll) = cv.fit(&loc, &ls, strip_h * 0.38, lw - 60.0 * m.k, 1);
    cv.text(m.pad + 34.0 * m.k, sy + (strip_h + cv.cap_height(&ls)) / 2.0, ll.first().map(String::as_str).unwrap_or(""), &ls, Align::Left);
    top + block_h
}

/// White sponsor box right-aligned at `right`. Returns its width.
fn sponsor(cv: &mut Canvas, s: &crate::Sponsor, right: f32, y: f32, h: f32, m: &Fmt) -> f32 {
    let logo = s.logo.as_deref().and_then(crate::gfx::b64);
    let name = s.name.trim();
    if logo.is_none() && name.is_empty() {
        return 0.0;
    }
    let label = if s.label.trim().is_empty() { "Brought to you by" } else { s.label.trim() };
    let ls = Style::new(11.0 * m.k, 800, Rgba::rgb(71, 85, 105)).track(0.14);
    let label_w = cv.measure(&label.to_uppercase(), &ls);
    let content_w = if logo.is_some() { (label_w + 30.0).max(150.0 * m.k).min(230.0 * m.k) } else { (cv.measure(name, &Style::new(20.0 * m.k, 900, INK_DARK)) + 10.0).max(label_w).min(230.0 * m.k) };
    let bw = content_w + 24.0;
    let x = right - bw;
    cv.fill_rrect(x, y, bw, h, 10.0, WHITE);
    cv.text(x + bw / 2.0, y + 8.0 + cv.cap_height(&ls), &label.to_uppercase(), &ls, Align::Center);
    let ly = y + 14.0 + cv.cap_height(&ls);
    let lh = h - (ly - y) - 8.0;
    let drawn = logo.as_ref().and_then(|b| cv.image(b, x + 12.0, ly, content_w, lh, 1.0, Align::Center));
    if drawn.is_none() {
        let (ns, nl) = cv.fit(name, &Style::new((lh * 0.6).min(24.0 * m.k), 900, INK_DARK), 11.0, content_w, 1);
        cv.text(x + bw / 2.0, ly + (lh + cv.cap_height(&ns)) / 2.0, nl.first().map(String::as_str).unwrap_or(""), &ns, Align::Center);
    }
    bw
}

/// Headline (accent) and one or two lines of the forecast words. Returns the y below.
fn headline(cv: &mut Canvas, f: &Forecast, p: &Pal, m: &Fmt, y: f32, w: f32) -> f32 {
    if f.headline.trim().is_empty() && f.description.trim().is_empty() {
        return y + 6.0;
    }
    let mut y = y + 14.0 * m.k;
    let inner = w - m.pad * 2.0 - 8.0;
    if !f.headline.trim().is_empty() {
        let (hs, hl) = cv.fit(&f.headline.to_uppercase(), &Style::new(22.0 * m.k, 900, p.accent).track(0.06), 14.0, inner, 1);
        y += cv.cap_height(&hs);
        cv.text(m.pad + 4.0, y, hl.first().map(String::as_str).unwrap_or(""), &hs, Align::Left);
    }
    if !f.description.trim().is_empty() {
        let lines = if m.story { 4 } else if f.ratio == "square" { 2 } else { 1 };
        let (ds, dl) = cv.fit(&f.description, &Style::new(16.0 * m.k, 600, p.ink.alpha(0.86)), 12.0, inner, lines);
        y = cv.paragraph(m.pad + 4.0, y + 8.0 * m.k, &dl, &ds, 1.25, Align::Left);
    }
    y
}

fn footer(cv: &mut Canvas, f: &Forecast, p: &Pal, m: &Fmt, w: f32, h: f32) {
    let y = h - m.foot_h;
    cv.fill_rect(0.0, y, w, m.foot_h, Rgba::rgb(4, 8, 16).alpha(0.55));
    cv.fill_rect(0.0, y, w, 3.0, p.accent);
    let mut x = m.pad;
    if let Some(bytes) = f.brand.logo.as_deref().and_then(crate::gfx::b64) {
        let lh = m.foot_h - 14.0;
        if let Some((dw, _)) = cv.image(&bytes, x, y + 8.0, lh * 2.2, lh, 1.0, Align::Left) {
            x += dw + 14.0;
        }
    }
    let ns = Style::new(m.foot_h * 0.36, 800, WHITE).track(0.14);
    let source = if f.source.trim().is_empty() { "Forecast from the National Weather Service" } else { f.source.trim() };
    let ss = Style::new(m.foot_h * 0.3, 600, p.muted);
    let sw = cv.text(w - m.pad, y + (m.foot_h + cv.cap_height(&ss)) / 2.0 + 1.0, source, &ss, Align::Right);
    let (ns, nl) = cv.fit(&f.brand.name.to_uppercase(), &ns, 11.0, (w - m.pad - sw - 20.0 - x).max(60.0), 1);
    cv.text(x, y + (m.foot_h + cv.cap_height(&ns)) / 2.0 + 1.0, nl.first().map(String::as_str).unwrap_or(""), &ns, Align::Left);
}

/// The weather picture for a day: Meteocons artwork, or the Remix icon for holidays.
fn art(cv: &mut Canvas, p: &Pal, d: &Day, x: f32, y: f32, size: f32) {
    match crate::meteo::for_day(day_icon(d), &d.description) {
        // The artwork has generous padding in its 64-unit box: draw it a little larger.
        Some(svg) => {
            let z = size * 1.25;
            cv.svg(svg, x - (z - size) / 2.0, y - (z - size) / 2.0, z, z, 1.0);
        }
        None => cv.icon(day_icon(d), x + size * 0.12, y + size * 0.12, size * 0.76, icon_color(p, day_icon(d))),
    }
}

#[allow(clippy::too_many_arguments)]
fn day_columns(cv: &mut Canvas, f: &Forecast, p: &Pal, m: &Fmt, x: f32, y: f32, w: f32, h: f32) {
    let days = &f.days;
    if days.is_empty() {
        return;
    }
    let n = days.len() as f32;
    let gap = 10.0 * m.k;
    let cw = (w - gap * (n - 1.0)) / n;
    let k = (cw / 165.0).clamp(0.7, 1.3) * if f.ratio == "square" { 0.95 } else { 1.0 };
    let graph = f.show_graph && days.len() > 1;
    // Fixed rows, top to bottom, the same in every card so everything lines up.
    let name_s = Style::new(30.0 * k, 900, p.accent).track(0.04);
    let date_s = Style::new(13.0 * k, 700, p.ink.alpha(0.7)).track(0.12);
    let desc_s = Style::new(17.0 * k, 600, p.ink);
    let high_s = Style::new(54.0 * k, 900, WHITE);
    let low_s = Style::new(32.0 * k, 800, p.muted);
    let rain_s = Style::new(15.0 * k, 800, RAIN).track(0.08);
    let any_rain = days.iter().any(|d| d.rain.is_some());

    let name_y = y + 14.0 * k + cv.cap_height(&name_s);
    let date_y = name_y + 8.0 * k + cv.cap_height(&date_s);
    let rain_y = y + h - 14.0 * k;
    let low_y = if any_rain { rain_y - cv.cap_height(&rain_s) - 14.0 * k } else { rain_y };
    let high_y = low_y - cv.cap_height(&low_s) - 12.0 * k;
    let graph_h = if graph { (h * 0.16).clamp(36.0, 90.0) } else { 0.0 };
    let graph_bot = high_y - cv.cap_height(&high_s) - 12.0 * k;
    let graph_top = graph_bot - graph_h;
    let desc_lines = 2usize;
    let desc_h = desc_s.px * 1.2 * desc_lines as f32;
    let desc_top = graph_top - 6.0 * k - desc_h;
    let art_top = date_y + 8.0 * k;
    let art_sz = (desc_top - 6.0 * k - art_top).min(cw * 0.78).max(24.0);

    let mut highs = vec![];
    let mut lows = vec![];
    for (i, d) in days.iter().enumerate() {
        let cx = x + i as f32 * (cw + gap);
        let mid = cx + cw / 2.0;
        cv.fill_rrect(cx, y, cw, h, 10.0, p.panel);
        cv.stroke_rrect(cx, y, cw, h, 10.0, 1.2, p.border);
        let (ns, nl) = cv.fit(&d.label.to_uppercase(), &name_s, 13.0, cw - 14.0, 1);
        cv.text(mid, name_y, nl.first().map(String::as_str).unwrap_or(""), &ns, Align::Center);
        if !d.date.is_empty() {
            cv.text(mid, date_y, &d.date.to_uppercase(), &date_s, Align::Center);
        }
        art(cv, p, d, mid - art_sz / 2.0, art_top + (desc_top - 6.0 * k - art_top - art_sz) / 2.0, art_sz);
        let (ds, dl) = cv.fit(&d.description, &desc_s, 11.0, cw - 16.0, desc_lines);
        let block = dl.len() as f32 * ds.px * 1.2;
        cv.paragraph(mid, desc_top + (desc_h - block) / 2.0, &dl, &ds, 1.2, Align::Center);
        let (big, small) = temps(d);
        if let Some(t) = big {
            cv.text(mid, high_y, &format!("{t}°"), &high_s, Align::Center);
            highs.push((mid, t));
        }
        if let Some(t) = small {
            cv.text(mid, low_y, &format!("{t}°"), &low_s, Align::Center);
            lows.push((mid, t));
        }
        if let Some(r) = d.rain {
            cv.text(mid, rain_y, &format!("RAIN {}%", r.clamp(0, 100)), &rain_s, Align::Center);
        }
    }
    if graph {
        trend(cv, p, &highs, &lows, x, graph_top, w, graph_h);
    }
}

/// Highs and lows as two lines across the cards, on one shared scale.
fn trend(cv: &mut Canvas, p: &Pal, highs: &[(f32, i32)], lows: &[(f32, i32)], x: f32, y: f32, w: f32, h: f32) {
    let all: Vec<i32> = highs.iter().chain(lows.iter()).map(|v| v.1).collect();
    if all.len() < 2 {
        return;
    }
    let (lo, hi) = (*all.iter().min().unwrap() as f32, *all.iter().max().unwrap() as f32);
    let (top, bot) = (y + 8.0, y + h - 8.0);
    let to_y = |t: i32| if hi > lo { bot - (t as f32 - lo) / (hi - lo) * (bot - top) } else { (top + bot) / 2.0 };
    cv.fill_rrect(x, y, w, h, 8.0, Rgba::rgb(0, 0, 0).alpha(0.16));
    for (pts, c) in [(lows, p.accent), (highs, HIGH_LINE)] {
        if pts.len() < 2 {
            continue;
        }
        let mut pb = PathBuilder::new();
        for (i, (gx, t)) in pts.iter().enumerate() {
            if i == 0 { pb.move_to(*gx, to_y(*t)) } else { pb.line_to(*gx, to_y(*t)) }
        }
        if let Some(path) = pb.finish() {
            cv.stroke_path(&path, 7.0, c.alpha(0.2));
            cv.stroke_path(&path, 2.6, c);
        }
        for (gx, t) in pts {
            cv.fill_rrect(gx - 5.0, to_y(*t) - 5.0, 10.0, 10.0, 5.0, c);
            cv.fill_rrect(gx - 2.2, to_y(*t) - 2.2, 4.4, 4.4, 2.2, Rgba::rgb(10, 16, 30));
        }
    }
}

/// Story: one row per day (artwork, name and words, rain, high / low).
#[allow(clippy::too_many_arguments)]
fn day_rows(cv: &mut Canvas, f: &Forecast, p: &Pal, m: &Fmt, x: f32, y: f32, w: f32, h: f32) {
    let days = &f.days;
    if days.is_empty() {
        return;
    }
    let n = days.len() as f32;
    let gap = 12.0;
    let rh = ((h - gap * (n - 1.0)) / n).min(230.0);
    // Shorter rows (square) scale their text down so the three lines still fit.
    let k = m.k * (rh / 150.0).clamp(0.6, 1.0);
    let highs: Vec<i32> = days.iter().filter_map(|d| temps(d).0).collect();
    let (lo, hi) = (highs.iter().min().copied().unwrap_or(0), highs.iter().max().copied().unwrap_or(0));
    for (i, d) in days.iter().enumerate() {
        let ry = y + i as f32 * (rh + gap);
        cv.fill_rrect(x, ry, w, rh, 12.0, p.panel);
        cv.stroke_rrect(x, ry, w, rh, 12.0, 1.2, p.border);
        let a = (rh - 24.0).min(150.0);
        art(cv, p, d, x + 18.0, ry + (rh - a) / 2.0, a);
        let (big, small) = temps(d);
        // Temperatures on the right: high, then low under it.
        let hs = Style::new(58.0 * k, 900, WHITE);
        let ls = Style::new(34.0 * k, 800, p.muted);
        let right = x + w - 28.0;
        let mut tx = right;
        if let Some(t) = big {
            let tw = cv.measure(&format!("{t}°"), &hs);
            cv.text(right, ry + rh / 2.0 + cv.cap_height(&hs) * 0.35, &format!("{t}°"), &hs, Align::Right);
            tx = right - tw - 20.0;
        }
        if let Some(t) = small {
            let lw = cv.measure(&format!("{t}°"), &ls);
            cv.text(tx, ry + rh / 2.0 + cv.cap_height(&ls) * 0.35, &format!("{t}°"), &ls, Align::Right);
            tx -= lw + 24.0;
        }
        // Trend: a short bar showing where this high sits in the week's range.
        if f.show_graph && hi > lo {
            if let Some(t) = big {
                let bw = 12.0;
                let bh = rh * 0.6;
                let by = ry + (rh - bh) / 2.0;
                let frac = (t - lo) as f32 / (hi - lo) as f32;
                cv.fill_rrect(tx - bw, by, bw, bh, bw / 2.0, WHITE.alpha(0.08));
                cv.fill_rrect(tx - bw, by + bh * (1.0 - frac), bw, bh * frac.max(0.08), bw / 2.0, HIGH_LINE);
                tx -= bw + 24.0;
            }
        }
        let lx = x + 18.0 + a + 24.0;
        let room = (tx - lx).max(120.0);
        let ns = Style::new(36.0 * k, 900, p.accent).track(0.04);
        let name = if d.date.is_empty() { d.label.to_uppercase() } else { format!("{}  {}", d.label.to_uppercase(), d.date.to_uppercase()) };
        let (ns, nl) = cv.fit(&name, &ns, 12.0, room, 1);
        let (ds, dl) = cv.fit(&d.description, &Style::new(24.0 * k, 600, p.ink), 11.0, room, 1);
        let rs = Style::new(18.0 * k, 800, RAIN).track(0.08);
        // Name, description and rain stacked and centred in the row.
        let (g1, g2) = (14.0 * k, 16.0 * k);
        let block = cv.cap_height(&ns) + g1 + cv.cap_height(&ds) + if d.rain.is_some() { g2 + cv.cap_height(&rs) } else { 0.0 };
        let base = ry + (rh - block) / 2.0 + cv.cap_height(&ns);
        cv.text(lx, base, nl.first().map(String::as_str).unwrap_or(""), &ns, Align::Left);
        let desc_y = base + g1 + cv.cap_height(&ds);
        cv.text(lx, desc_y, dl.first().map(String::as_str).unwrap_or(""), &ds, Align::Left);
        if let Some(r) = d.rain {
            cv.text(lx, desc_y + g2 + cv.cap_height(&rs), &format!("RAIN {}%", r.clamp(0, 100)), &rs, Align::Left);
        }
    }
}

/// Regional forecast: cities down the side, days across, each cell artwork + high / low.
#[allow(clippy::too_many_arguments)]
fn city_table(cv: &mut Canvas, f: &Forecast, p: &Pal, m: &Fmt, x: f32, y: f32, w: f32, h: f32) {
    let cols = f.days.len().max(1);
    let k = m.k;
    let name_w = (w * if m.story { 0.34 } else { 0.24 }).max(150.0);
    let head_h = 46.0 * k;
    let gap = 6.0 * k;
    let rows = f.cities.len().max(1) as f32;
    let rh = ((h - head_h - gap * rows) / rows).min(120.0 * k);
    let cw = (w - name_w) / cols as f32;
    // Column headings.
    let hs = Style::new(20.0 * k, 900, p.accent).track(0.06);
    let ds = Style::new(11.0 * k, 700, p.ink.alpha(0.7)).track(0.1);
    for (i, d) in f.days.iter().enumerate() {
        let mid = x + name_w + cw * (i as f32 + 0.5);
        let (hs2, hl) = cv.fit(&d.label.to_uppercase(), &hs, 11.0, cw - 8.0, 1);
        cv.text(mid, y + cv.cap_height(&hs2) + 4.0, hl.first().map(String::as_str).unwrap_or(""), &hs2, Align::Center);
        if !d.date.is_empty() {
            cv.text(mid, y + cv.cap_height(&hs2) + 12.0 * k + cv.cap_height(&ds), &d.date.to_uppercase(), &ds, Align::Center);
        }
    }
    let ts = Style::new((rh * 0.34).min(30.0 * k), 900, WHITE);
    let ls = Style::new((rh * 0.24).min(21.0 * k), 800, p.muted);
    for (r, city) in f.cities.iter().enumerate() {
        let ry = y + head_h + r as f32 * (rh + gap);
        let fill = if city.home { p.strong.alpha(0.22) } else { p.panel };
        cv.fill_rrect(x, ry, w, rh, 8.0, fill);
        cv.stroke_rrect(x, ry, w, rh, 8.0, if city.home { 1.6 } else { 1.0 }, if city.home { p.accent } else { p.border });
        let (ns, nl) = cv.fit(&city.name.to_uppercase(), &Style::new((rh * 0.3).min(24.0 * k), 900, if city.home { p.accent } else { p.heading }).track(0.04), 11.0, name_w - 24.0, 1);
        cv.text(x + 16.0, ry + (rh + cv.cap_height(&ns)) / 2.0, nl.first().map(String::as_str).unwrap_or(""), &ns, Align::Left);
        for (i, d) in city.days.iter().take(cols).enumerate() {
            let cx = x + name_w + cw * i as f32;
            let a = (rh - 12.0).min(cw * 0.42);
            art(cv, p, d, cx + 6.0, ry + (rh - a) / 2.0, a);
            let (big, small) = temps(d);
            let tx = cx + 6.0 + a + 4.0 + (cw - a - 10.0) / 2.0;
            let base = ry + rh / 2.0 + cv.cap_height(&ts) * 0.1;
            if let Some(t) = big {
                cv.text(tx, base, &format!("{t}°"), &ts, Align::Center);
            }
            if let Some(t) = small {
                cv.text(tx, base + 8.0 + cv.cap_height(&ls), &format!("{t}°"), &ls, Align::Center);
            }
        }
    }
}
