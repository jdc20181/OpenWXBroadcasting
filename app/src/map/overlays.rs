//! Geo overlays and on-air elements drawn over the map frame by the compositor.

use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, StrokeDash,
    Transform,
};

use crate::{
    config::{Brand, Settings},
    data::{alert_color, Alert, SpcArea, StormCell},
    gfx::{
        parse_color, parse_hex,
        render::{draw_legend, rounded_rect},
        text::{draw_line, line_metrics, measure, GlyphCache, TextStyle, Weight},
        with_alpha,
    },
    map::{
        geo::{bearing_deg, destination, haversine_km, Viewport, KM_PER_MILE},
        scene::{Annotation, AnnotationKind, MapScene, RangeRings, StormTrack},
    },
};

pub(crate) fn paint(color: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    p
}

pub(crate) fn stroke(width: f32) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    }
}

pub(crate) fn ring_path(vp: &Viewport, ring: &[(f64, f64)], close: bool) -> Option<tiny_skia::Path> {
    if ring.len() < 2 {
        return None;
    }
    let mut pb = PathBuilder::new();
    let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (i, (lon, lat)) in ring.iter().enumerate() {
        let (x, y) = vp.to_screen(*lat, *lon);
        minx = minx.min(x);
        miny = miny.min(y);
        maxx = maxx.max(x);
        maxy = maxy.max(y);
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    // Skip shapes entirely outside the frame.
    if maxx < -50.0 || maxy < -50.0 || minx > vp.width as f32 + 50.0 || miny > vp.height as f32 + 50.0 {
        return None;
    }
    if close {
        pb.close();
    }
    pb.finish()
}

/// The brand's panel colour (on-air boxes) at the given alpha, so themes restyle the whole frame.
pub fn panel_color(brand: &Brand, alpha: f32) -> Color {
    with_alpha(parse_color("$panel", brand), alpha)
}

pub(crate) fn style(family: &str, weight: Weight, size: f32, color: Color) -> TextStyle {
    TextStyle {
        family: family.to_string(),
        weight,
        size,
        color,
        letter_spacing: 0.0,
    }
}

/// Text with a dark halo so it reads over any map.
pub(crate) fn halo_text(pm: &mut Pixmap, glyphs: &mut GlyphCache, text: &str, x: f32, y: f32, st: &TextStyle) {
    let mut shadow = st.clone();
    shadow.color = Color::from_rgba8(0, 0, 0, 200);
    for (dx, dy) in [(-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0), (1.5, 1.5)] {
        draw_line(pm, glyphs, text, x + dx, y + dy, &shadow, 1.0, None);
    }
    draw_line(pm, glyphs, text, x, y, st, 1.0, None);
}

pub fn draw_spc(pm: &mut Pixmap, vp: &Viewport, areas: &[SpcArea]) {
    for area in areas {
        let fill = with_alpha(parse_hex(&area.fill).unwrap_or(Color::WHITE), area.fill_opacity.max(0.25));
        let line = parse_hex(&area.stroke).unwrap_or(Color::WHITE);
        for ring in &area.rings {
            if let Some(path) = ring_path(vp, ring, true) {
                pm.fill_path(&path, &paint(fill), FillRule::EvenOdd, Transform::identity(), None);
                pm.stroke_path(&path, &paint(line), &stroke(3.0), Transform::identity(), None);
            }
        }
    }
}

/// Outlook areas with stronger fills (the outlook is the whole map, not an overlay on radar).
/// Whether any ring's bounding box overlaps the frame.
fn area_in_view(vp: &Viewport, rings: &[Vec<(f64, f64)>]) -> bool {
    let (s, w, n, e) = vp.bounds();
    rings.iter().any(|r| {
        let (mut lo_x, mut hi_x, mut lo_y, mut hi_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for (lon, lat) in r {
            lo_x = lo_x.min(*lon);
            hi_x = hi_x.max(*lon);
            lo_y = lo_y.min(*lat);
            hi_y = hi_y.max(*lat);
        }
        hi_x >= w && lo_x <= e && hi_y >= s && lo_y <= n
    })
}

/// A calm pill in the middle of the map for an outlook with nothing in view.
fn draw_empty_note(pm: &mut Pixmap, glyphs: &mut GlyphCache, brand: &Brand, note: &str) {
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let st = style(&brand.font_body, Weight::SemiBold, 30.0, parse_color("$text", brand));
    let tw = measure(note, &st);
    let (bw, bh) = (tw + 56.0, 64.0);
    let (bx, by) = ((w - bw) / 2.0, (h - bh) / 2.0);
    if let Some(path) = rounded_rect(bx, by, bw, bh, 32.0) {
        pm.fill_path(&path, &paint(panel_color(brand, 0.86)), FillRule::Winding, Transform::identity(), None);
    }
    let (ascent, descent) = line_metrics(&st);
    draw_line(pm, glyphs, note, bx + 28.0, by + (bh - ascent - descent) / 2.0 + ascent, &st, 1.0, None);
}

pub fn draw_spc_solid(pm: &mut Pixmap, vp: &Viewport, areas: &[SpcArea], alpha: f32) {
    for area in areas {
        let fill = with_alpha(parse_hex(&area.fill).unwrap_or(Color::WHITE), alpha);
        let line = parse_hex(&area.stroke).unwrap_or(Color::WHITE);
        for ring in &area.rings {
            if let Some(path) = ring_path(vp, ring, true) {
                pm.fill_path(&path, &paint(fill), FillRule::EvenOdd, Transform::identity(), None);
                pm.stroke_path(&path, &paint(with_alpha(line, 0.95)), &stroke(2.5), Transform::identity(), None);
            }
        }
    }
}

/// Significant severe risk: SPC's hatched areas (EF2+ tornadoes, 2"+ hail, 75 mph+ wind) from every
/// hazard, black-hatched over the hazard's colour, labelled by hazard.
pub fn draw_spc_significant(pm: &mut Pixmap, vp: &Viewport, day: &crate::data::SpcDay) {
    for hazard in &day.probabilities {
        let tint = match hazard.key.as_str() {
            "tornado" => Color::from_rgba8(220, 40, 60, 90),
            "hail" => Color::from_rgba8(40, 170, 80, 90),
            _ => Color::from_rgba8(40, 110, 230, 90),
        };
        for area in hazard.areas.iter().filter(|a| a.label.to_uppercase().contains("SIG")) {
            for ring in &area.rings {
                if let Some(path) = ring_path(vp, ring, true) {
                    pm.fill_path(&path, &paint(tint), FillRule::EvenOdd, Transform::identity(), None);
                    if let Some(mut mask) = tiny_skia::Mask::new(pm.width(), pm.height()) {
                        mask.fill_path(&path, FillRule::EvenOdd, true, Transform::identity());
                        let b = path.bounds();
                        let mut pb = PathBuilder::new();
                        let mut x = b.left() - b.height();
                        while x < b.right() {
                            pb.move_to(x, b.bottom());
                            pb.line_to(x + b.height(), b.top());
                            x += 11.0;
                        }
                        if let Some(p) = pb.finish() {
                            pm.stroke_path(&p, &paint(Color::from_rgba8(0, 0, 0, 200)), &stroke(2.0), Transform::identity(), Some(&mask));
                        }
                    }
                    pm.stroke_path(&path, &paint(Color::BLACK), &stroke(3.0), Transform::identity(), None);
                }
            }
        }
    }
}

/// Hatch pattern for a warning type: (angles in degrees, spacing px, dots instead of lines).
fn hatch_for(event: &str) -> Option<(&'static [f32], f32, bool)> {
    let e = event.to_lowercase();
    if e.contains("watch") {
        Some((&[-45.0], 22.0, false)) // sparse diagonal the other way
    } else if e.contains("tornado") {
        Some((&[45.0, -45.0], 14.0, false)) // cross-hatch
    } else if e.contains("severe thunderstorm") {
        Some((&[45.0], 14.0, false)) // diagonal
    } else if e.contains("flash flood") || e.contains("flood") {
        Some((&[0.0], 12.0, false)) // horizontal
    } else if e.contains("winter") || e.contains("blizzard") || e.contains("ice storm") || e.contains("snow") {
        Some((&[], 16.0, true)) // dots
    } else if e.contains("special marine") || e.contains("extreme wind") || e.contains("hurricane") || e.contains("tropical") {
        Some((&[90.0], 14.0, false)) // vertical
    } else {
        None
    }
}

/// Fill `path` with the pattern for `event`, clipped to the polygon.
fn draw_hatch(pm: &mut Pixmap, path: &tiny_skia::Path, event: &str) {
    let Some((angles, gap, dots)) = hatch_for(event) else { return };
    let Some(mut mask) = tiny_skia::Mask::new(pm.width(), pm.height()) else { return };
    mask.fill_path(path, FillRule::Winding, true, Transform::identity());
    let b = path.bounds();
    let (cx, cy) = ((b.left() + b.right()) / 2.0, (b.top() + b.bottom()) / 2.0);
    let r = ((b.width().powi(2) + b.height().powi(2)).sqrt() / 2.0 + gap).min(4000.0);
    let mut pb = PathBuilder::new();
    if dots {
        let mut y = b.top();
        let mut row = 0;
        while y <= b.bottom() {
            let mut x = b.left() + if row % 2 == 0 { 0.0 } else { gap / 2.0 };
            while x <= b.right() {
                pb.push_circle(x, y, 2.2);
                x += gap;
            }
            y += gap * 0.87;
            row += 1;
        }
        if let Some(p) = pb.finish() {
            pm.fill_path(&p, &paint(Color::from_rgba8(255, 255, 255, 200)), FillRule::Winding, Transform::identity(), Some(&mask));
        }
        return;
    }
    for angle in angles {
        let (s, c) = angle.to_radians().sin_cos();
        let mut d = -r;
        while d <= r {
            // Line perpendicular offset d from the centre, running along the angle.
            let (ox, oy) = (cx - s * d, cy + c * d);
            pb.move_to(ox - c * r, oy - s * r);
            pb.line_to(ox + c * r, oy + s * r);
            d += gap;
        }
    }
    if let Some(p) = pb.finish() {
        pm.stroke_path(&p, &paint(Color::from_rgba8(0, 0, 0, 120)), &stroke(3.5), Transform::identity(), Some(&mask));
        pm.stroke_path(&p, &paint(Color::from_rgba8(255, 255, 255, 190)), &stroke(1.6), Transform::identity(), Some(&mask));
    }
}

pub fn draw_alerts(pm: &mut Pixmap, vp: &Viewport, alerts: &[Alert], fill: bool, patterns: bool) {
    // Lowest priority first so warnings end up on top.
    let mut ordered: Vec<&Alert> = alerts.iter().collect();
    ordered.sort_by_key(|a| std::cmp::Reverse(a.priority()));
    for alert in ordered {
        let color = parse_hex(alert_color(&alert.event)).unwrap_or(Color::WHITE);
        // Tornado/flash flood emergencies and PDS warnings get heavier, double outlines.
        let severe = alert.threat_tag().is_some() && alert.is_warning();
        let width = if severe { 7.0 } else if alert.is_warning() { 4.5 } else { 3.0 };
        for ring in &alert.polygons {
            if let Some(path) = ring_path(vp, ring, true) {
                if fill {
                    let a = if severe { 0.35 } else { 0.22 };
                    pm.fill_path(&path, &paint(with_alpha(color, a)), FillRule::Winding, Transform::identity(), None);
                }
                if patterns {
                    draw_hatch(pm, &path, &alert.event);
                }
                pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 170)), &stroke(width + 3.0), Transform::identity(), None);
                pm.stroke_path(&path, &paint(color), &stroke(width), Transform::identity(), None);
                if severe {
                    let mut st = stroke(2.0);
                    st.dash = StrokeDash::new(vec![12.0, 8.0], 0.0);
                    pm.stroke_path(&path, &paint(Color::WHITE), &st, Transform::identity(), None);
                }
            }
        }
    }
}

pub fn draw_range_rings(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, rings: &RangeRings, brand: &Brand) {
    let color = parse_color("$primary", brand);
    let dash = StrokeDash::new(vec![14.0, 10.0], 0.0);
    for miles in &rings.miles {
        let km = miles * KM_PER_MILE;
        let mut pb = PathBuilder::new();
        for i in 0..=96 {
            let (lat, lon) = destination(rings.lat, rings.lon, i as f64 * 3.75, km);
            let (x, y) = vp.to_screen(lat, lon);
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        pb.close();
        if let Some(path) = pb.finish() {
            let mut st = stroke(3.0);
            st.dash = dash.clone();
            pm.stroke_path(&path, &paint(with_alpha(color, 0.9)), &st, Transform::identity(), None);
        }
        let (lat, lon) = destination(rings.lat, rings.lon, 0.0, km);
        let (x, y) = vp.to_screen(lat, lon);
        let label = format!("{miles:.0} MI");
        let st = style(&brand.font_display, Weight::Bold, 26.0, parse_color("$text", brand));
        let w = measure(&label, &st);
        halo_text(pm, glyphs, &label, x - w / 2.0, y - 8.0, &st);
    }
    let (cx, cy) = vp.to_screen(rings.lat, rings.lon);
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, 8.0);
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
        pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.5), Transform::identity(), None);
    }
}

pub fn cell_color(level: u8) -> Color {
    match level {
        0 | 1 => Color::from_rgba8(80, 200, 120, 255),
        2 => Color::from_rgba8(255, 214, 10, 255),
        3 => Color::from_rgba8(255, 150, 30, 255),
        4 => Color::from_rgba8(255, 80, 40, 255),
        5 => Color::from_rgba8(230, 20, 60, 255),
        _ => Color::from_rgba8(220, 60, 255, 255),
    }
}

/// Storm cells: `tracks` draws the 30 / 60 minute paths, `icons` the storm badges.
/// Storm cells: `tracks` draws the 30 / 60 minute paths, `icons` the storm badges.
pub fn draw_storm_cells(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, cells: &[StormCell], tracks: bool, icons: bool, brand: &Brand) {
    for cell in cells.iter().filter(|c| c.intensity.level >= 2 || c.lightning.strikes > 0) {
        let (x, y) = vp.to_screen(cell.lat, cell.lon);
        if x < -40.0 || y < -40.0 || x > vp.width as f32 + 40.0 || y > vp.height as f32 + 40.0 {
            continue;
        }
        let color = cell_color(cell.intensity.level);

        if tracks {
            draw_cell_wedge(pm, vp, cell, x, y);
        }

        if icons {
            draw_cell_badge(pm, cell, x, y, cell.motion.as_ref().map(|m| m.heading_deg));
        }
    }
}

/// The storm's badge colour and symbol, like the consumer apps: red with a hail cloud for
/// hail, orange with a bolt for lightning, amber with a rain cloud for heavy rain.
fn badge_style(cell: &StormCell) -> (Color, u8) {
    let hail = cell.intensity.hail.as_deref().is_some_and(|h| !h.is_empty());
    if hail || cell.intensity.level >= 4 {
        (Color::from_rgba8(229, 57, 53, 255), 0)
    } else if cell.lightning.strikes > 0 {
        (Color::from_rgba8(245, 124, 0, 255), 1)
    } else {
        (Color::from_rgba8(242, 169, 0, 255), 2)
    }
}

/// A storm badge: solid disc with a white rim and a bold white symbol, plus a small pointer on
/// the rim toward where the storm is heading. Details are in the click pop-out.
fn draw_cell_badge(pm: &mut Pixmap, cell: &StormCell, x: f32, y: f32, heading: Option<f64>) {
    let (fill, symbol) = badge_style(cell);
    let r = if cell.intensity.level >= 4 { 22.0 } else { 18.0 };
    let shadow = Color::from_rgba8(0, 0, 0, 90);
    let id = Transform::identity();
    // Heading pointer (drawn first, so the disc sits on top of its base).
    if let Some(h) = heading {
        let a = (h as f32).to_radians();
        let (dx, dy) = (a.sin(), -a.cos());
        let (px, py) = (-dy, dx);
        let tip = (x + dx * (r + 13.0), y + dy * (r + 13.0));
        let base = r * 0.55;
        let mut pb = PathBuilder::new();
        pb.move_to(tip.0, tip.1);
        pb.line_to(x + dx * (r - 2.0) + px * base, y + dy * (r - 2.0) + py * base);
        pb.line_to(x + dx * (r - 2.0) - px * base, y + dy * (r - 2.0) - py * base);
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(fill), FillRule::Winding, id, None);
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), id, None);
        }
    }
    let mut pb = PathBuilder::new();
    pb.push_circle(x + 1.5, y + 2.5, r + 2.0);
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(shadow), FillRule::Winding, id, None);
    }
    let mut pb = PathBuilder::new();
    pb.push_circle(x, y, r);
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(fill), FillRule::Winding, id, None);
        pm.stroke_path(&path, &paint(Color::WHITE), &stroke(3.0), id, None);
    }
    // The symbol: a Remix weather icon in white.
    let icon = match symbol {
        0 => "hail-fill",
        1 => "thunderstorms-fill",
        _ => "heavy-showers-fill",
    };
    let size = (r * 1.45).round();
    if let Some(img) = crate::gfx::remix::raster(icon, (size * 2.0) as u32, "#FFFFFF") {
        let k = size / img.width() as f32;
        let ts = Transform::from_row(k, 0.0, 0.0, k, x - size / 2.0, y - size / 2.0);
        pm.draw_pixmap(0, 0, (*img).as_ref(), &tiny_skia::PixmapPaint::default(), ts, None);
    }
}

/// The storm's likely path for the next 30 minutes: a soft wedge in its badge colour.
fn draw_cell_wedge(pm: &mut Pixmap, vp: &Viewport, cell: &StormCell, x: f32, y: f32) {
    let Some(m) = &cell.motion else { return };
    let (fill, _) = badge_style(cell);
    let km = (m.speed_kmh * 0.5).max(8.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    for i in 0..=8 {
        let a = m.heading_deg - 18.0 + 36.0 * i as f64 / 8.0;
        let (lat, lon) = destination(cell.lat, cell.lon, a, km);
        let (px, py) = vp.to_screen(lat, lon);
        pb.line_to(px, py);
    }
    pb.close();
    if let Some(path) = pb.finish() {
        let mut f = fill;
        f.set_alpha(0.32);
        pm.fill_path(&path, &paint(f), FillRule::Winding, Transform::identity(), None);
        let mut e = fill;
        e.set_alpha(0.85);
        pm.stroke_path(&path, &paint(e), &stroke(2.0), Transform::identity(), None);
    }
}

/// Towns within `km` of a point worth naming on air (2,000+ people, or the built-in list).
fn places_near(lat: f64, lon: f64, km: f64) -> Vec<crate::places::Place> {
    let dlat = km / 110.57;
    let dlon = km / (111.32 * lat.to_radians().cos().max(0.2));
    crate::places::in_box(lat - dlat, lat + dlat, lon - dlon, lon + dlon)
        .into_iter()
        .filter(|p| p.population >= 2000 || p.population == 0)
        .collect()
}

/// Places inside the storm track cone, with ETA minutes, nearest first.
pub fn storm_track_etas(track: &StormTrack) -> Vec<(String, f64, f64)> {
    let speed_kmh = track.speed_mph * KM_PER_MILE;
    if speed_kmh <= 0.5 {
        return Vec::new();
    }
    if track.is_freeform() {
        return freeform_etas(track, speed_kmh);
    }
    let max_km = speed_kmh * track.minutes as f64 / 60.0;
    let mut hits: Vec<(String, f64, f64)> = places_near(track.lat, track.lon, max_km)
        .iter()
        .filter_map(|p| {
            let (name, lat, lon) = (&p.name, &p.lat, &p.lon);
            let dist = haversine_km(track.lat, track.lon, *lat, *lon);
            if dist < 1.0 || dist > max_km {
                return None;
            }
            let bearing = bearing_deg(track.lat, track.lon, *lat, *lon);
            let mut diff = (bearing - track.heading_deg).abs() % 360.0;
            if diff > 180.0 {
                diff = 360.0 - diff;
            }
            if diff > track.cone_deg {
                return None;
            }
            let along = dist * diff.to_radians().cos();
            Some((name.clone(), along / speed_kmh * 60.0, dist / KM_PER_MILE))
        })
        .collect();
    hits.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    hits
}

/// The freeform track as a polyline: storm origin + drawn path.
fn freeform_points(track: &StormTrack) -> Vec<(f64, f64)> {
    let mut pts = vec![(track.lat, track.lon)];
    pts.extend(track.path.iter().copied());
    pts
}

/// Cumulative km along a polyline.
fn cumulative_km(pts: &[(f64, f64)]) -> Vec<f64> {
    let mut acc = vec![0.0];
    for w in pts.windows(2) {
        let last = *acc.last().unwrap_or(&0.0);
        acc.push(last + haversine_km(w[0].0, w[0].1, w[1].0, w[1].1));
    }
    acc
}

/// Point `km` along a polyline.
fn point_along(pts: &[(f64, f64)], cum: &[f64], km: f64) -> Option<(f64, f64)> {
    for i in 1..pts.len() {
        if km <= cum[i] {
            let seg = (cum[i] - cum[i - 1]).max(1e-6);
            let t = (km - cum[i - 1]) / seg;
            return Some((pts[i - 1].0 + (pts[i].0 - pts[i - 1].0) * t, pts[i - 1].1 + (pts[i].1 - pts[i - 1].1) * t));
        }
    }
    None
}

/// Towns within the corridor of a freeform track: (name, ETA minutes, miles away now).
fn freeform_etas(track: &StormTrack, speed_kmh: f64) -> Vec<(String, f64, f64)> {
    let pts = freeform_points(track);
    let cum = cumulative_km(&pts);
    let corridor_km = track.corridor_mi.max(1.0) * KM_PER_MILE;
    let reach = cum.last().copied().unwrap_or(0.0) + corridor_km;
    let mut hits: Vec<(String, f64, f64)> = places_near(track.lat, track.lon, reach)
        .iter()
        .filter_map(|p| {
            let (name, lat, lon) = (&p.name, &p.lat, &p.lon);
            // Nearest point on the path, in a local flat projection.
            let kx = 111.32 * lat.to_radians().cos();
            let ky = 110.57;
            let mut best: Option<(f64, f64)> = None; // (cross km, along km)
            for i in 1..pts.len() {
                let (ax, ay) = ((pts[i - 1].1 - lon) * kx, (pts[i - 1].0 - lat) * ky);
                let (bx, by) = ((pts[i].1 - lon) * kx, (pts[i].0 - lat) * ky);
                let (dx, dy) = (bx - ax, by - ay);
                let len2 = (dx * dx + dy * dy).max(1e-9);
                let t = (-(ax * dx + ay * dy) / len2).clamp(0.0, 1.0);
                let (px, py) = (ax + dx * t, ay + dy * t);
                let cross = (px * px + py * py).sqrt();
                let along = cum[i - 1] + (cum[i] - cum[i - 1]) * t;
                if best.map(|b| cross < b.0).unwrap_or(true) {
                    best = Some((cross, along));
                }
            }
            let (cross, along) = best?;
            if cross > corridor_km || along < 1.0 {
                return None;
            }
            let now_mi = haversine_km(track.lat, track.lon, *lat, *lon) / KM_PER_MILE;
            Some((name.clone(), along / speed_kmh * 60.0, now_mi))
        })
        .collect();
    hits.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    hits
}

fn draw_freeform_track(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, track: &StormTrack, brand: &Brand, color: Color) {
    let pts = freeform_points(track);
    let cum = cumulative_km(&pts);
    let screen: Vec<(f32, f32)> = pts.iter().map(|(la, lo)| vp.to_screen(*la, *lo)).collect();
    let mut pb = PathBuilder::new();
    pb.move_to(screen[0].0, screen[0].1);
    // Smooth through midpoints so a hand-drawn path reads as a sweeping track.
    for i in 1..screen.len() {
        if i + 1 < screen.len() {
            let mid = ((screen[i].0 + screen[i + 1].0) / 2.0, (screen[i].1 + screen[i + 1].1) / 2.0);
            pb.quad_to(screen[i].0, screen[i].1, mid.0, mid.1);
        } else {
            pb.line_to(screen[i].0, screen[i].1);
        }
    }
    let Some(path) = pb.finish() else { return };
    // Corridor width in pixels from miles at this zoom.
    let mid = pts[pts.len() / 2];
    let (ex, ey) = {
        let (la, lo) = destination(mid.0, mid.1, 90.0, track.corridor_mi.max(1.0) * KM_PER_MILE);
        vp.to_screen(la, lo)
    };
    let (mx, my) = vp.to_screen(mid.0, mid.1);
    let corridor_px = (((ex - mx).powi(2) + (ey - my).powi(2)).sqrt() * 2.0).clamp(18.0, 400.0);
    pm.stroke_path(&path, &paint(with_alpha(color, 0.25)), &stroke(corridor_px), Transform::identity(), None);
    pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 170)), &stroke(9.0), Transform::identity(), None);
    pm.stroke_path(&path, &paint(color), &stroke(6.0), Transform::identity(), None);
    pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), Transform::identity(), None);

    // Arrow head at the end.
    let n = screen.len();
    let (x1, y1) = screen[n - 1];
    let (x0, y0) = screen[n - 2];
    let angle = (y1 - y0).atan2(x1 - x0);
    let head = 34.0;
    let mut pb = PathBuilder::new();
    pb.move_to(x1 + angle.cos() * 10.0, y1 + angle.sin() * 10.0);
    pb.line_to(x1 - head * (angle - 0.5).cos(), y1 - head * (angle - 0.5).sin());
    pb.line_to(x1 - head * (angle + 0.5).cos(), y1 - head * (angle + 0.5).sin());
    pb.close();
    if let Some(path) = pb.finish() {
        pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 170)), &stroke(4.0), Transform::identity(), None);
        pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
    }

    // Time ticks along the path.
    let speed_kmh = track.speed_mph * KM_PER_MILE;
    let total = *cum.last().unwrap_or(&0.0);
    let step = track.step_minutes.max(5) as f64;
    let mut minute = step;
    while speed_kmh > 0.5 && speed_kmh * minute / 60.0 < total {
        if let Some((la, lo)) = point_along(&pts, &cum, speed_kmh * minute / 60.0) {
            let (x, y) = vp.to_screen(la, lo);
            let mut pb = PathBuilder::new();
            pb.push_circle(x, y, 9.0);
            if let Some(path) = pb.finish() {
                pm.fill_path(&path, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
                pm.stroke_path(&path, &paint(color), &stroke(3.0), Transform::identity(), None);
            }
            let eta = chrono::Local::now() + chrono::Duration::minutes(minute as i64);
            let st = style(&brand.font_display, Weight::Bold, 24.0, parse_color("$text", brand));
            halo_text(pm, glyphs, &eta.format("%-I:%M").to_string(), x + 14.0, y + 8.0, &st);
        }
        minute += step;
    }
}

pub fn draw_storm_track(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, track: &StormTrack, brand: &Brand, panel: Option<(f32, f32)>) {
    let color = parse_hex(&track.color).unwrap_or(Color::from_rgba8(255, 59, 59, 255));
    let speed_kmh = track.speed_mph * KM_PER_MILE;
    let max_km = speed_kmh * track.minutes as f64 / 60.0;
    let (ox, oy) = vp.to_screen(track.lat, track.lon);

    if track.is_freeform() {
        draw_freeform_track(pm, glyphs, vp, track, brand, color);
    } else {
        draw_vector_track(pm, glyphs, vp, track, brand, color, speed_kmh, max_km, (ox, oy));
    }
    draw_track_origin_and_panel(pm, glyphs, vp, track, brand, color, (ox, oy), panel);
}

#[allow(clippy::too_many_arguments)]
fn draw_vector_track(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    vp: &Viewport,
    track: &StormTrack,
    brand: &Brand,
    color: Color,
    speed_kmh: f64,
    max_km: f64,
    (ox, oy): (f32, f32),
) {
    // Cone.
    let mut pb = PathBuilder::new();
    pb.move_to(ox, oy);
    let steps = 24;
    for i in 0..=steps {
        let b = track.heading_deg - track.cone_deg + 2.0 * track.cone_deg * i as f64 / steps as f64;
        let (lat, lon) = destination(track.lat, track.lon, b, max_km);
        let (x, y) = vp.to_screen(lat, lon);
        pb.line_to(x, y);
    }
    pb.close();
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(with_alpha(color, 0.22)), FillRule::Winding, Transform::identity(), None);
        pm.stroke_path(&path, &paint(with_alpha(color, 0.9)), &stroke(3.0), Transform::identity(), None);
    }

    // Center line + time marks.
    let (ex, ey) = {
        let (lat, lon) = destination(track.lat, track.lon, track.heading_deg, max_km);
        vp.to_screen(lat, lon)
    };
    let mut pb = PathBuilder::new();
    pb.move_to(ox, oy);
    pb.line_to(ex, ey);
    if let Some(path) = pb.finish() {
        pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 170)), &stroke(7.0), Transform::identity(), None);
        pm.stroke_path(&path, &paint(Color::WHITE), &stroke(3.5), Transform::identity(), None);
    }
    let step = track.step_minutes.max(5);
    let mut minute = step;
    while minute <= track.minutes {
        let km = speed_kmh * minute as f64 / 60.0;
        let (lat, lon) = destination(track.lat, track.lon, track.heading_deg, km);
        let (x, y) = vp.to_screen(lat, lon);
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, 9.0);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(color), &stroke(3.0), Transform::identity(), None);
        }
        let eta = chrono::Local::now() + chrono::Duration::minutes(minute as i64);
        let st = style(&brand.font_display, Weight::Bold, 24.0, parse_color("$text", brand));
        halo_text(pm, glyphs, &eta.format("%-I:%M").to_string(), x + 14.0, y + 8.0, &st);
        minute += step;
    }
}

fn draw_track_origin_and_panel(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    vp: &Viewport,
    track: &StormTrack,
    brand: &Brand,
    color: Color,
    (ox, oy): (f32, f32),
    panel: Option<(f32, f32)>,
) {
    // Origin marker.
    let mut pb = PathBuilder::new();
    pb.push_circle(ox, oy, 13.0);
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
        pm.stroke_path(&path, &paint(Color::WHITE), &stroke(3.0), Transform::identity(), None);
    }

    // ETA panel (bottom right) - only for the main track.
    let Some((pdx, pdy)) = panel else {
        return;
    };
    let etas = storm_track_etas(track);
    let rows = etas.len().min(7);
    let panel_w = 520.0;
    let row_h = 52.0;
    let sponsor_h = if track.sponsor.trim().is_empty() { 0.0 } else { 44.0 };
    let panel_h = 110.0 + rows.max(1) as f32 * row_h + sponsor_h;
    let px = vp.width as f32 - panel_w - 40.0 + pdx;
    let py = vp.height as f32 - panel_h - 150.0 + pdy;
    if let Some(path) = rounded_rect(px, py, panel_w, panel_h, 14.0) {
        pm.fill_path(&path, &paint(panel_color(brand, 0.9)), FillRule::Winding, Transform::identity(), None);
    }
    if let Some(rect) = Rect::from_xywh(px, py, panel_w, 64.0) {
        pm.fill_rect(rect, &paint(color), Transform::identity(), None);
    }
    let title = style(&brand.font_display, Weight::Black, 34.0, parse_color("$text", brand));
    draw_line(pm, glyphs, &track.label.to_uppercase(), px + 22.0, py + 45.0, &title, 1.0, None);
    let sub = style(&brand.font_body, Weight::SemiBold, 24.0, parse_color("$muted", brand));
    draw_line(
        pm,
        glyphs,
        &format!(
            "Moving {} at {:.0} mph",
            crate::map::geo::compass(track.heading_deg),
            track.speed_mph
        ),
        px + 22.0,
        py + 96.0,
        &sub,
        1.0,
        None,
    );
    let name_st = style(&brand.font_display, Weight::Bold, 30.0, parse_color("$text", brand));
    let eta_st = style(&brand.font_display, Weight::Bold, 30.0, parse_color("$primary", brand));
    if etas.is_empty() {
        draw_line(pm, glyphs, "No towns in path", px + 22.0, py + 150.0, &sub, 1.0, None);
    }
    for (i, (name, minutes, _miles)) in etas.iter().take(rows).enumerate() {
        let y = py + 110.0 + (i as f32 + 0.75) * row_h;
        draw_line(pm, glyphs, &name.to_uppercase(), px + 22.0, y, &name_st, 1.0, None);
        let eta = chrono::Local::now() + chrono::Duration::minutes(minutes.round() as i64);
        let label = eta.format("%-I:%M %p").to_string();
        let w = measure(&label, &eta_st);
        draw_line(pm, glyphs, &label, px + panel_w - 22.0 - w, y, &eta_st, 1.0, None);
    }
    if sponsor_h > 0.0 {
        let y = py + panel_h - sponsor_h;
        if let Some(rect) = Rect::from_xywh(px, y, panel_w, sponsor_h) {
            pm.fill_rect(rect, &paint(with_alpha(parse_color("$secondary", brand), 0.95)), Transform::identity(), None);
        }
        let st = style(&brand.font_body, Weight::SemiBold, 22.0, parse_color("$text", brand));
        draw_line(pm, glyphs, &track.sponsor, px + 22.0, y + 30.0, &st, 1.0, None);
    }
}

/// Seconds since the app started: the clock for looping map animations.
pub fn anim_clock() -> f32 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f32()
}

/// Emphasize the last visible raised region after the map and weather have been composed.
pub fn draw_raised_area(pm: &mut Pixmap, vp: &Viewport, scene: &MapScene, radar_time: Option<i64>) {
    use super::scene::RaiseStyle;
    if !scene.raise_visible || !scene.overlay_visible("annotations") { return; }
    let Some(area) = scene.annotations.iter().rev().find(|a| a.kind == AnnotationKind::RaisedArea) else { return };
    if !area.visible_at(radar_time) { return; }
    let rings = if area.area_rings.is_empty() { std::slice::from_ref(&area.points) } else { &area.area_rings };
    let mut builder = PathBuilder::new();
    for ring in rings.iter().filter(|ring| ring.len() >= 3) {
        let points: Vec<_> = ring.iter().map(|&(lat, lon)| vp.to_screen(lat, lon)).collect();
        let (left, top, right, bottom) = points.iter().fold(
            (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
            |(left, top, right, bottom), &(x, y)| (left.min(x), top.min(y), right.max(x), bottom.max(y)),
        );
        // An off-screen selection should leave the visible map at full brightness.
        if right < 0.0 || bottom < 0.0 || left > pm.width() as f32 || top > pm.height() as f32 { continue; }
        builder.move_to(points[0].0, points[0].1);
        for &(x, y) in &points[1..] { builder.line_to(x, y); }
        builder.close();
    }
    let Some(path) = builder.finish() else { return };
    let lift = if scene.raise_style == RaiseStyle::Lift { (pm.height() as f32 * 0.011).clamp(4.0, 12.0) } else { 0.0 };
    let transform = Transform::from_translate(0.0, -lift);
    let dim = paint(Color::from_rgba8(12, 17, 28, 100));
    if scene.raise_style != RaiseStyle::Outline {
        let Some(mut mask) = tiny_skia::Mask::new(pm.width(), pm.height()) else { return };
        let Some(rect) = Rect::from_xywh(0.0, 0.0, pm.width() as f32, pm.height() as f32) else { return };
        mask.fill_path(&path, FillRule::EvenOdd, true, transform);
        if mask.data().iter().all(|&alpha| alpha == 0) { return; }
        if scene.raise_style == RaiseStyle::Spotlight {
            mask.invert();
            pm.fill_rect(rect, &dim, Transform::identity(), Some(&mask));
        } else {
            let original = pm.clone();
            pm.fill_rect(rect, &dim, Transform::identity(), None);
            pm.fill_path(&path, &paint(Color::from_rgba8(4, 7, 12, 135)), FillRule::EvenOdd, Transform::from_translate(2.0, 3.0), None);
            let side = paint(Color::from_rgba8(18, 26, 41, 255));
            pm.fill_path(&path, &side, FillRule::EvenOdd, Transform::identity(), None);
            pm.stroke_path(&path, &side, &stroke(lift), Transform::from_translate(0.0, -lift * 0.5), None);
            pm.draw_pixmap(0, 0, original.as_ref(), &tiny_skia::PixmapPaint::default(), transform, Some(&mask));
        }
    }
    pm.stroke_path(&path, &paint(Color::from_rgba8(226, 235, 246, 245)), &stroke(2.0), transform, None);
}

pub fn draw_annotations(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, annotations: &[Annotation], brand: &Brand, radar_time: Option<i64>) {
    let clock = anim_clock();
    for a in annotations {
        // Event replay: only while the radar loop is inside the mark's window.
        if a.kind == AnnotationKind::RaisedArea || !a.visible_at(radar_time) {
            continue;
        }
        let color = parse_color(&a.color, brand);
        let period = if a.loop_secs > 0.5 { a.loop_secs } else { 8.0 };
        let phase = (clock % period) / period;
        // Travel for the first 75% of the loop, then hold at the end.
        let travel = crate::gfx::ease_out((phase / 0.75).min(1.0));
        let (dlat, dlon) = a.motion.map(|(la, lo)| (la * travel as f64, lo * travel as f64)).unwrap_or((0.0, 0.0));
        let mut pts: Vec<(f32, f32)> = a.points.iter().map(|(lat, lon)| vp.to_screen(*lat + dlat, *lon + dlon)).collect();
        if pts.is_empty() {
            continue;
        }
        // Movement trail: dashed line from where it started to where it is now.
        if let Some((mla, mlo)) = a.motion {
            let n = a.points.len() as f64;
            let (clat, clon) = a.points.iter().fold((0.0, 0.0), |acc, p| (acc.0 + p.0 / n, acc.1 + p.1 / n));
            let (x0, y0) = vp.to_screen(clat, clon);
            let (x1, y1) = vp.to_screen(clat + mla * travel as f64, clon + mlo * travel as f64);
            let mut pb = PathBuilder::new();
            pb.move_to(x0, y0);
            pb.line_to(x1, y1);
            if let Some(path) = pb.finish() {
                let mut st = stroke(3.0);
                st.dash = StrokeDash::new(vec![10.0, 8.0], 0.0);
                pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 120)), &stroke(6.0), Transform::identity(), None);
                pm.stroke_path(&path, &paint(Color::WHITE), &st, Transform::identity(), None);
            }
        }
        // Draw-on: keep the first part of the line, growing over the first half of the loop.
        if a.reveal && pts.len() >= 2 {
            let grow = crate::gfx::ease_out((phase / 0.5).min(1.0));
            let total: f32 = pts.windows(2).map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt()).sum();
            let mut left = total * grow;
            let mut cut = vec![pts[0]];
            for w in pts.windows(2) {
                let len = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
                if left >= len {
                    cut.push(w[1]);
                    left -= len;
                } else {
                    let t = if len > 0.0 { left / len } else { 0.0 };
                    cut.push((w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t));
                    break;
                }
            }
            if cut.len() >= 2 {
                pts = cut;
            }
        }
        let outline = |pm: &mut Pixmap, path: &tiny_skia::Path, width: f32| {
            pm.stroke_path(path, &paint(Color::from_rgba8(0, 0, 0, 150)), &stroke(width + 4.0), Transform::identity(), None);
            pm.stroke_path(path, &paint(color), &stroke(width), Transform::identity(), None);
        };
        match a.kind {
            AnnotationKind::RaisedArea => {}
            AnnotationKind::Pen | AnnotationKind::Polygon => {
                let mut pb = PathBuilder::new();
                pb.move_to(pts[0].0, pts[0].1);
                for p in &pts[1..] {
                    pb.line_to(p.0, p.1);
                }
                if a.kind == AnnotationKind::Polygon && pts.len() > 2 {
                    pb.close();
                }
                if let Some(path) = pb.finish() {
                    if a.kind == AnnotationKind::Polygon && (a.fill || pts.len() > 2) {
                        pm.fill_path(&path, &paint(with_alpha(color, 0.25)), FillRule::Winding, Transform::identity(), None);
                    }
                    outline(pm, &path, a.width);
                }
                if a.measure && a.points.len() >= 2 {
                    let text = if a.kind == AnnotationKind::Polygon {
                        let mut ring = a.points.clone();
                        ring.push(a.points[0]);
                        format!("{:.0} sq mi • {:.0} mi around", crate::map::geo::polygon_sq_mi(&a.points), crate::map::geo::path_miles(&ring))
                    } else {
                        format!("{:.1} mi", crate::map::geo::path_miles(&a.points))
                    };
                    let (lx, ly) = if a.kind == AnnotationKind::Polygon {
                        let n = pts.len() as f32;
                        (pts.iter().map(|p| p.0).sum::<f32>() / n, pts.iter().map(|p| p.1).sum::<f32>() / n)
                    } else {
                        pts[pts.len() - 1]
                    };
                    let st = style(&brand.font_display, Weight::Bold, 26.0, parse_color("$text", brand));
                    let w = measure(&text, &st);
                    if let Some(path) = rounded_rect(lx - w / 2.0 - 12.0, ly - 40.0, w + 24.0, 38.0, 8.0) {
                        pm.fill_path(&path, &paint(panel_color(brand, 0.88)), FillRule::Winding, Transform::identity(), None);
                    }
                    draw_line(pm, glyphs, &text, lx - w / 2.0, ly - 12.0, &st, 1.0, None);
                }
            }
            AnnotationKind::Arrow => {
                if pts.len() < 2 {
                    continue;
                }
                let (x0, y0) = pts[0];
                let (x1, y1) = pts[pts.len() - 1];
                let angle = (y1 - y0).atan2(x1 - x0);
                let head = (a.width * 4.5).max(26.0);
                let mut pb = PathBuilder::new();
                pb.move_to(x0, y0);
                pb.line_to(x1 - angle.cos() * head * 0.7, y1 - angle.sin() * head * 0.7);
                if let Some(path) = pb.finish() {
                    outline(pm, &path, a.width);
                }
                let mut pb = PathBuilder::new();
                pb.move_to(x1, y1);
                pb.line_to(x1 - head * (angle - 0.45).cos(), y1 - head * (angle - 0.45).sin());
                pb.line_to(x1 - head * (angle + 0.45).cos(), y1 - head * (angle + 0.45).sin());
                pb.close();
                if let Some(path) = pb.finish() {
                    pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 150)), &stroke(4.0), Transform::identity(), None);
                    pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
                }
            }
            AnnotationKind::Circle => {
                if pts.len() < 2 {
                    continue;
                }
                let (cx, cy) = pts[0];
                let r = ((pts[1].0 - cx).powi(2) + (pts[1].1 - cy).powi(2)).sqrt().max(4.0);
                let mut pb = PathBuilder::new();
                pb.push_circle(cx, cy, r);
                if let Some(path) = pb.finish() {
                    if a.fill {
                        pm.fill_path(&path, &paint(with_alpha(color, 0.22)), FillRule::Winding, Transform::identity(), None);
                    }
                    outline(pm, &path, a.width);
                }
            }
            AnnotationKind::Text => {
                let st = style(&brand.font_display, Weight::Bold, (a.width * 6.0).max(28.0), color);
                halo_text(pm, glyphs, &a.text, pts[0].0, pts[0].1, &st);
            }
            AnnotationKind::Marker => {
                let (x, y) = pts[0];
                let mut pb = PathBuilder::new();
                pb.move_to(x, y);
                pb.cubic_to(x - 22.0, y - 26.0, x - 18.0, y - 52.0, x, y - 52.0);
                pb.cubic_to(x + 18.0, y - 52.0, x + 22.0, y - 26.0, x, y);
                pb.close();
                if let Some(path) = pb.finish() {
                    pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
                    pm.stroke_path(&path, &paint(Color::WHITE), &stroke(3.0), Transform::identity(), None);
                }
                if !a.text.is_empty() {
                    let st = style(&brand.font_display, Weight::Bold, 32.0, parse_color("$text", brand));
                    halo_text(pm, glyphs, &a.text, x + 20.0, y - 26.0, &st);
                }
            }
            kind if kind.is_front() => draw_front(pm, &pts, kind, a.width),
            AnnotationKind::High | AnnotationKind::Low => {
                let high = a.kind == AnnotationKind::High;
                draw_pressure_center(pm, glyphs, pts[0], high, &a.text, brand);
            }
            AnnotationKind::Ruler => {
                if pts.len() < 2 || a.points.len() < 2 {
                    continue;
                }
                let (x0, y0) = pts[0];
                let (x1, y1) = pts[pts.len() - 1];
                let mut pb = PathBuilder::new();
                pb.move_to(x0, y0);
                pb.line_to(x1, y1);
                if let Some(path) = pb.finish() {
                    let mut st = stroke(4.0);
                    st.dash = StrokeDash::new(vec![16.0, 10.0], 0.0);
                    pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 160)), &stroke(8.0), Transform::identity(), None);
                    pm.stroke_path(&path, &paint(color), &st, Transform::identity(), None);
                }
                for (x, y) in [(x0, y0), (x1, y1)] {
                    let mut pb = PathBuilder::new();
                    pb.push_circle(x, y, 8.0);
                    if let Some(path) = pb.finish() {
                        pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
                        pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.5), Transform::identity(), None);
                    }
                }
                let (la0, lo0) = a.points[0];
                let (la1, lo1) = a.points[a.points.len() - 1];
                let miles = haversine_km(la0, lo0, la1, lo1) / KM_PER_MILE;
                // Rough drive time: roads ~1.25x straight-line distance at ~50 mph.
                let drive_min = (miles * 1.25 / 50.0 * 60.0).round();
                let brg = bearing_deg(la0, lo0, la1, lo1);
                let dir = crate::map::geo::compass(brg);
                let label = match a.elevation_ft {
                    Some((e0, e1)) => format!("{miles:.1} MI {dir} ({brg:.0}°) • ~{drive_min:.0} MIN DRIVE • {e0:.0} → {e1:.0} FT ({:+.0})", e1 - e0),
                    None => format!("{miles:.1} MI {dir} ({brg:.0}°) • ~{drive_min:.0} MIN DRIVE"),
                };
                let st = style(&brand.font_display, Weight::Bold, 28.0, parse_color("$text", brand));
                let w = measure(&label, &st);
                let (mx, my) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                if let Some(path) = rounded_rect(mx - w / 2.0 - 14.0, my - 46.0, w + 28.0, 42.0, 8.0) {
                    pm.fill_path(&path, &paint(panel_color(brand, 0.88)), FillRule::Winding, Transform::identity(), None);
                }
                draw_line(pm, glyphs, &label, mx - w / 2.0, my - 15.0, &st, 1.0, None);
            }
            AnnotationKind::Spotlight => {
                if pts.len() < 2 {
                    continue;
                }
                let (cx, cy) = pts[0];
                let r = ((pts[1].0 - cx).powi(2) + (pts[1].1 - cy).powi(2)).sqrt().max(20.0);
                // Dim everything outside the circle.
                let mut pb = PathBuilder::new();
                if let Some(rect) = Rect::from_xywh(0.0, 0.0, vp.width as f32, vp.height as f32) {
                    pb.push_rect(rect);
                }
                pb.push_circle(cx, cy, r);
                if let Some(path) = pb.finish() {
                    pm.fill_path(&path, &paint(Color::from_rgba8(0, 0, 0, 140)), FillRule::EvenOdd, Transform::identity(), None);
                }
                let mut pb = PathBuilder::new();
                pb.push_circle(cx, cy, r);
                if let Some(path) = pb.finish() {
                    pm.stroke_path(&path, &paint(with_alpha(color, 0.35)), &stroke(16.0), Transform::identity(), None);
                    pm.stroke_path(&path, &paint(color), &stroke(4.0), Transform::identity(), None);
                }
            }
            _ => {}
        }
    }
}

/// Smoothed screen path through hand-drawn points.
fn smooth_path(pts: &[(f32, f32)]) -> Option<tiny_skia::Path> {
    if pts.len() < 2 {
        return None;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].0, pts[0].1);
    for i in 1..pts.len() {
        if i + 1 < pts.len() {
            let mid = ((pts[i].0 + pts[i + 1].0) / 2.0, (pts[i].1 + pts[i + 1].1) / 2.0);
            pb.quad_to(pts[i].0, pts[i].1, mid.0, mid.1);
        } else {
            pb.line_to(pts[i].0, pts[i].1);
        }
    }
    pb.finish()
}

/// Positions and directions every `spacing` px along a polyline: (x, y, angle).
fn along_polyline(pts: &[(f32, f32)], spacing: f32, offset: f32) -> Vec<(f32, f32, f32)> {
    let mut out = Vec::new();
    let mut next = offset;
    let mut walked = 0.0;
    for w in pts.windows(2) {
        let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.01 {
            continue;
        }
        let angle = dy.atan2(dx);
        while next <= walked + len {
            let t = (next - walked) / len;
            out.push((w[0].0 + dx * t, w[0].1 + dy * t, angle));
            next += spacing;
        }
        walked += len;
    }
    out
}

/// Weather fronts with standard symbols. Symbols sit on the left of the drawing direction
/// (draw the front so the cold air is behind you).
fn draw_front(pm: &mut Pixmap, pts: &[(f32, f32)], kind: AnnotationKind, width: f32) {
    let blue = Color::from_rgba8(40, 110, 255, 255);
    let red = Color::from_rgba8(235, 40, 40, 255);
    let purple = Color::from_rgba8(170, 60, 210, 255);
    let orange = Color::from_rgba8(240, 150, 40, 255);
    let Some(path) = smooth_path(pts) else { return };
    let line_w = width.clamp(4.0, 10.0);
    let size = 16.0 + line_w * 1.4;

    let base_color = match kind {
        AnnotationKind::ColdFront => blue,
        AnnotationKind::WarmFront => red,
        AnnotationKind::OccludedFront => purple,
        AnnotationKind::Trough => orange,
        _ => blue,
    };
    pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 150)), &stroke(line_w + 4.0), Transform::identity(), None);
    if kind == AnnotationKind::Trough {
        let mut st = stroke(line_w);
        st.dash = StrokeDash::new(vec![22.0, 14.0], 0.0);
        pm.stroke_path(&path, &paint(base_color), &st, Transform::identity(), None);
        return;
    }
    if kind == AnnotationKind::StationaryFront {
        // Alternating red/blue line segments.
        let mut st = stroke(line_w);
        st.dash = StrokeDash::new(vec![size * 2.0, size * 2.0], 0.0);
        pm.stroke_path(&path, &paint(red), &stroke(line_w), Transform::identity(), None);
        pm.stroke_path(&path, &paint(blue), &st, Transform::identity(), None);
    } else {
        pm.stroke_path(&path, &paint(base_color), &stroke(line_w), Transform::identity(), None);
    }

    let spacing = size * 4.0;
    for (n, (x, y, angle)) in along_polyline(pts, spacing, spacing * 0.5).into_iter().enumerate() {
        // Normal pointing to the left of travel (screen coords: y down).
        let (nx, ny) = (angle.sin(), -angle.cos());
        let (tx, ty) = (angle.cos(), angle.sin());
        let (symbol, color, flip) = match kind {
            AnnotationKind::ColdFront => ('t', blue, false),
            AnnotationKind::WarmFront => ('s', red, false),
            AnnotationKind::OccludedFront => (if n % 2 == 0 { 't' } else { 's' }, purple, false),
            AnnotationKind::StationaryFront => {
                if n % 2 == 0 {
                    ('t', blue, false)
                } else {
                    ('s', red, true)
                }
            }
            _ => continue,
        };
        let (nx, ny) = if flip { (-nx, -ny) } else { (nx, ny) };
        let mut pb = PathBuilder::new();
        if symbol == 't' {
            pb.move_to(x - tx * size * 0.6, y - ty * size * 0.6);
            pb.line_to(x + nx * size, y + ny * size);
            pb.line_to(x + tx * size * 0.6, y + ty * size * 0.6);
            pb.close();
        } else {
            // Semicircle bulging along the normal.
            let r = size * 0.62;
            let steps = 12;
            pb.move_to(x - tx * r, y - ty * r);
            for k in 0..=steps {
                let a = std::f32::consts::PI * k as f32 / steps as f32;
                let (ca, sa) = (a.cos(), a.sin());
                pb.line_to(x - tx * r * ca + nx * r * sa, y - ty * r * ca + ny * r * sa);
            }
            pb.close();
        }
        if let Some(path) = pb.finish() {
            pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 140)), &stroke(3.0), Transform::identity(), None);
            pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
        }
    }
}

/// Broadcast H / L pressure centre: white disc, coloured ring and letter, optional value.
fn draw_pressure_center(pm: &mut Pixmap, glyphs: &mut GlyphCache, (x, y): (f32, f32), high: bool, value: &str, brand: &Brand) {
    let color = if high { Color::from_rgba8(30, 80, 220, 255) } else { Color::from_rgba8(220, 30, 40, 255) };
    let r = 40.0;
    let mut pb = PathBuilder::new();
    pb.push_circle(x, y, r + 6.0);
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(Color::from_rgba8(0, 0, 0, 90)), FillRule::Winding, Transform::from_translate(3.0, 5.0), None);
        pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
    }
    let mut pb = PathBuilder::new();
    pb.push_circle(x, y, r);
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
    }
    let st = style(&brand.font_display, Weight::Black, 58.0, color);
    let letter = if high { "H" } else { "L" };
    let w = measure(letter, &st);
    let (ascent, descent) = line_metrics(&st);
    draw_line(pm, glyphs, letter, x - w / 2.0, y + (ascent - descent) / 2.0 - 4.0, &st, 1.0, None);
    if !value.trim().is_empty() {
        let vs = style(&brand.font_display, Weight::Bold, 24.0, parse_color("$text", brand));
        let vw = measure(value, &vs);
        halo_text(pm, glyphs, value, x - vw / 2.0, y + r + 34.0, &vs);
    }
}

/// The geo layers of a scene (SPC, warnings, rings, cells, track, telestration) for one view.
pub fn draw_scene_layers(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    vp: &Viewport,
    scene: &MapScene,
    data: &crate::data::WxSnapshot,
    brand: &Brand,
    radar_time: Option<i64>,
) {
    // Only the alert classes switched on (warnings / watches / advisories / statements).
    let shown: Vec<Alert> = if scene.warnings {
        data.map_alerts.iter().filter(|a| scene.alert_kinds.allows(a)).cloned().collect()
    } else {
        Vec::new()
    };
    for key in scene.overlay_order() {
        if !scene.overlay_visible(key) {
            continue;
        }
        let fx = scene.layer_fx.get(key).cloned().unwrap_or_default();
        if fx.is_plain() {
            draw_overlay(key, pm, glyphs, vp, scene, data, brand, radar_time, &shown);
            continue;
        }
        // Draw the layer on its own sheet, then composite it with its look.
        let Some(mut sheet) = Pixmap::new(pm.width(), pm.height()) else { continue };
        draw_overlay(key, &mut sheet, glyphs, vp, scene, data, brand, radar_time, &shown);
        if let Some(tint) = &fx.tint {
            // Recolour: keep the layer's shape (alpha), replace its colour.
            let mut p = paint(parse_color(tint, brand));
            p.blend_mode = tiny_skia::BlendMode::SourceIn;
            if let Some(rect) = Rect::from_xywh(0.0, 0.0, pm.width() as f32, pm.height() as f32) {
                sheet.fill_rect(rect, &p, Transform::identity(), None);
            }
        }
        let mut opacity = fx.opacity.clamp(0.0, 1.0);
        if fx.pulse {
            opacity *= 0.55 + 0.45 * ((anim_clock() * std::f32::consts::TAU / 1.6).sin() * 0.5 + 0.5);
        }
        let mask = fx.clip.and_then(|(lat, lon, miles)| {
            let mut mask = tiny_skia::Mask::new(pm.width(), pm.height())?;
            let (cx, cy) = vp.to_screen(lat, lon);
            let r = (miles * KM_PER_MILE * vp.px_per_km()) as f32;
            let mut pb = PathBuilder::new();
            pb.push_circle(cx, cy, r.max(1.0));
            mask.fill_path(&pb.finish()?, FillRule::Winding, true, Transform::identity());
            Some(mask)
        });
        let blend = match fx.blend.as_str() {
            "multiply" => tiny_skia::BlendMode::Multiply,
            "screen" => tiny_skia::BlendMode::Screen,
            "overlay" => tiny_skia::BlendMode::Overlay,
            "lighten" => tiny_skia::BlendMode::Lighten,
            "darken" => tiny_skia::BlendMode::Darken,
            _ => tiny_skia::BlendMode::SourceOver,
        };
        let paint_px = tiny_skia::PixmapPaint { opacity, blend_mode: blend, ..tiny_skia::PixmapPaint::default() };
        pm.draw_pixmap(0, 0, sheet.as_ref(), &paint_px, Transform::identity(), mask.as_ref());
    }
}

thread_local! {
    /// Still layers drawn once per (layer, frame size): (key, sheet).
    static LAYER_CACHE: std::cell::RefCell<std::collections::HashMap<(&'static str, u32, u32), (u64, Pixmap)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// A key for the map view as `vp` shows it (centre, zoom, size).
pub fn view_key(vp: &Viewport) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (vp.view.lat.to_bits(), vp.view.lon.to_bits(), vp.view.zoom.to_bits(), vp.width.to_bits(), vp.height.to_bits()).hash(&mut h);
    h.finish()
}

/// Draw a layer that doesn't animate (station plots, river gauges) from a cached sheet: it is
/// drawn again only when `key` (view + data) changes, otherwise it is one blit. Hundreds of
/// haloed labels per frame took Program down to 12 fps (CWXB-53).
pub fn cached_layer(name: &'static str, key: u64, pm: &mut Pixmap, draw: impl FnOnce(&mut Pixmap)) {
    let slot = (name, pm.width(), pm.height());
    LAYER_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let blit = |pm: &mut Pixmap, sheet: &Pixmap| {
            pm.draw_pixmap(0, 0, sheet.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
        };
        if let Some((k, sheet)) = cache.get(&slot) {
            if *k == key {
                blit(pm, sheet);
                return;
            }
        }
        let Some(mut sheet) = Pixmap::new(pm.width(), pm.height()) else {
            draw(pm);
            return;
        };
        draw(&mut sheet);
        blit(pm, &sheet);
        cache.insert(slot, (key, sheet));
    });
}

/// One overlay layer by key (see scene::OVERLAY_LAYERS).
#[allow(clippy::too_many_arguments)]
fn draw_overlay(
    key: &str,
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    vp: &Viewport,
    scene: &MapScene,
    data: &crate::data::WxSnapshot,
    brand: &Brand,
    radar_time: Option<i64>,
    shown: &[Alert],
) {
    match key {
        "spc" => {
            if let Some(spc) = scene.spc_day.and_then(|day| data.spc.iter().find(|d| d.day == day)) {
                if scene.spc_layer == "significant" {
                    draw_spc_significant(pm, vp, spc);
                } else if !scene.radar.enabled && scene.data.field.is_none() {
                    // The outlook map on its own: solid broadcast-style fills.
                    draw_spc_solid(pm, vp, spc.areas(&scene.spc_layer), 0.78);
                } else {
                    draw_spc(pm, vp, spc.areas(&scene.spc_layer));
                }
            }
            // Nothing in view: a calm note, so an empty map doesn't look like missing data
            // (CWXB-91). Only on the outlook map itself, not under radar.
            if let Some(day) = scene.spc_day {
                let areas = data.spc.iter().find(|d| d.day == day).map(|d| d.areas(&scene.spc_layer)).unwrap_or(&[]);
                let loaded = !data.spc.is_empty();
                let severe: Vec<&crate::data::SpcArea> = areas.iter().filter(|a| !a.label.eq_ignore_ascii_case("TSTM")).collect();
                if loaded && !scene.radar.enabled && scene.data.field.is_none() && !severe.iter().any(|a| area_in_view(vp, &a.rings)) {
                    let when = match day { 1 => "today", 2 => "tomorrow", _ => "that day" };
                    let note = match scene.spc_layer.as_str() {
                        "tornado" => format!("Tornado chance under 2% in this area {when}"),
                        "wind" => format!("Damaging wind chance under 5% in this area {when}"),
                        "hail" => format!("Large hail chance under 5% in this area {when}"),
                        _ => format!("No severe storms expected in this area {when}"),
                    };
                    draw_empty_note(pm, glyphs, brand, &note);
                }
            }
        }
        "qpf" => {
            if let Some(period) = scene.qpf {
                let i = crate::feeds::QpfPeriod::ALL.iter().position(|p| *p == period).unwrap_or(0);
                draw_spc(pm, vp, &data.qpf[i]);
            }
        }
        "ero" => {
            if let Some(day) = scene.ero_day {
                let areas = &data.ero[(day.clamp(1, 3) - 1) as usize];
                draw_spc(pm, vp, areas);
                if !scene.radar.enabled && scene.data.field.is_none() && data.ero.iter().any(|d| !d.is_empty()) && !areas.iter().any(|a| area_in_view(vp, &a.rings)) {
                    let when = match day { 1 => "today", 2 => "tomorrow", _ => "that day" };
                    draw_empty_note(pm, glyphs, brand, &format!("No flooding rain risk in this area {when}"));
                }
            }
        }
        "mesoscale" => draw_mesoscale(pm, vp, data),
        "boundaries" => {
            let light = matches!(scene.basemap, crate::config::Basemap::Light | crate::config::Basemap::Terrain | crate::config::Basemap::Voyager);
            super::boundaries::draw(pm, vp, scene.counties, scene.states, light, scene.boundary_opacity, crate::maptheme::get(&scene.theme).as_ref());
        }
        "warnings" => draw_alerts(pm, vp, shown, scene.warnings_fill, scene.warning_patterns),
        "roads" => super::layers::draw_road_events(pm, vp, &data.roads, scene.roads_all),
        "rings" => {
            if let Some(rings) = &scene.range_rings {
                draw_range_rings(pm, glyphs, vp, rings, brand);
            }
        }
        "cells" => draw_storm_cells(pm, glyphs, vp, &data.cells, scene.cell_paths, scene.storm_cells, brand),
        "tracks" => {
            for track in &scene.extra_tracks {
                draw_storm_track(pm, glyphs, vp, track, brand, None);
            }
            if let Some(track) = &scene.storm_track {
                draw_storm_track(pm, glyphs, vp, track, brand, Some(scene.onair_offsets.get("eta").copied().unwrap_or((0.0, 0.0))));
            }
        }
        "reports" => {
            let shown: Vec<crate::data::StormReport> = data.reports.iter().filter(|r| !scene.report_hidden.contains(r.marker().0)).cloned().collect();
            super::layers::draw_storm_reports(pm, glyphs, vp, &shown, scene.report_hours, brand);
        }
        "countdowns" => super::layers::draw_warning_labels(pm, glyphs, vp, shown, brand),
        "lightning_density" => super::extras::draw_lightning_density(pm, vp, &data.lightning, 30),
        "lightning" => super::layers::draw_lightning(pm, vp, &data.lightning),
        "rivers" => {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            view_key(vp).hash(&mut h);
            for g in &data.rivers {
                (&g.location, g.worst_status(), g.observed.map(f64::to_bits)).hash(&mut h);
            }
            cached_layer("rivers", h.finish(), pm, |sheet| super::extras::draw_rivers(sheet, glyphs, vp, &data.rivers, brand));
        }
        "centers" => super::extras::draw_centers(pm, glyphs, vp, &data.community.centers, brand),
        "regions" => super::extras::draw_regions(pm, glyphs, vp, data, brand),
        "town_labels" => {
            super::extras::draw_town_labels(pm, glyphs, vp, scene, brand);
            // Map theme: the station's home marker.
            if let (Some(t), Some(o)) = (crate::maptheme::get(&scene.theme), data.overview.as_ref()) {
                let (x, y) = vp.to_screen(o.location.lat, o.location.lon);
                crate::maptheme::draw_symbol(pm, &t.home_symbol, x, y, 1.0);
            }
        }
        "mcs" => super::extras::draw_mcs(pm, glyphs, vp, &data.analysis.mcs, brand),
        "lightning_anim" => super::extras::draw_lightning_animation(pm, glyphs, vp, &data.lightning, brand),
        "lightning_clusters" => super::extras::draw_clusters(pm, glyphs, vp, &data.analysis.clusters, brand),
        "storm_tracks" => super::extras::draw_tracks(pm, glyphs, vp, &data.analysis.tracks, brand),
        "traffic" => super::extras::draw_traffic(pm, glyphs, vp, &data.roads, brand),
        "flood_map" => super::extras::draw_flood_map(pm, glyphs, vp, data, brand),
        "damage_map" => super::extras::draw_damage_map(pm, glyphs, vp, &data.reports, scene.damage_hours.max(6), brand),
        "annotations" => draw_annotations(pm, glyphs, vp, &scene.annotations, brand, radar_time),
        "viewer_reports" => super::extras::draw_viewer_reports(pm, glyphs, vp, &data.viewer, brand),
        "eoc" => crate::eoc::draw(pm, glyphs, vp, &data.eoc, &data.roads, brand),
        "nowcast" => crate::nowcast::draw(pm, glyphs, vp, &data.nowcast, &scene.nowcast_mode, brand),
        "outages" => crate::outages::draw(pm, glyphs, vp, &data.outages, &scene.outage_mode, &crate::outages::state_filter(), brand),
        "tornado_history" => super::extras::draw_tornado_history(pm, glyphs, vp, scene.tornado_since, brand),
        "history" => {
            if let (Some(ev), Some(t)) = (data.history_event.as_ref(), scene.history_time) {
                // Warnings in effect and reports so far at the replay time.
                let active: Vec<Alert> = ev
                    .warnings
                    .iter()
                    .filter(|a| {
                        let p = |s: &Option<String>| s.as_deref().and_then(|x| chrono::DateTime::parse_from_rfc3339(x).ok().map(|d| d.timestamp()).or_else(|| chrono::NaiveDateTime::parse_from_str(x, "%Y-%m-%dT%H:%MZ").ok().map(|d| d.and_utc().timestamp())));
                        p(&a.sent).map(|s| s <= t).unwrap_or(false) && p(&a.expires).map(|e| e > t).unwrap_or(true)
                    })
                    .cloned()
                    .collect();
                draw_alerts(pm, vp, &active, true, scene.warning_patterns);
                let so_far: Vec<crate::data::StormReport> = ev.reports.iter().filter(|r| r.time <= t).cloned().collect();
                super::layers::draw_storm_reports(pm, glyphs, vp, &so_far, 240, brand);
                let st = style(&brand.font_display, Weight::Bold, 28.0, parse_color("$text", brand));
                let label = chrono::DateTime::from_timestamp(t, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d, %Y %-I:%M %p").to_string()).unwrap_or_default();
                halo_text(pm, glyphs, &format!("Replay: {} • {label}", ev.case.name), vp.width as f32 * 0.05, vp.height as f32 * 0.16, &st);
            }
        }
        "drought" => match scene.drought_view.as_str() {
            "month" => super::extras::draw_hazard_shapes(pm, glyphs, vp, &data.hazards.drought_month, "drought_outlook", brand),
            "season" => super::extras::draw_hazard_shapes(pm, glyphs, vp, &data.hazards.drought_season, "drought_outlook", brand),
            _ => super::extras::draw_hazard_shapes(pm, glyphs, vp, &data.hazards.drought, "drought", brand),
        },
        "quakes" => super::extras::draw_quakes(pm, glyphs, vp, &data.quakes, scene.quake_days, scene.quake_min, brand),
        "fire_outlook" => super::extras::draw_hazard_shapes(pm, glyphs, vp, &data.hazards.fire_outlook, "fire", brand),
        "wildfires" => super::extras::draw_hazard_shapes(pm, glyphs, vp, &data.hazards.wildfires, "wildfire", brand),
        "tropical" => super::extras::draw_tropical(pm, glyphs, vp, &data.hazards.tropical, &scene.tropical_layer, brand),
        "custom" => {
            for layer in scene.custom_layers.iter().filter(|l| l.visible) {
                crate::customdata::draw(pm, glyphs, vp, layer, brand);
            }
        }
        _ => {}
    }
}

fn draw_mesoscale(pm: &mut Pixmap, vp: &Viewport, data: &crate::data::WxSnapshot) {
    if !data.mesoscale.is_empty() {
        // Drawn like an outlook area: blue outline, light fill, "MD 1234" label.
        let areas: Vec<crate::data::SpcArea> = data
            .mesoscale
            .iter()
            .map(|md| crate::data::SpcArea {
                label: md.name.clone(),
                rank: 0,
                stroke: "#3B82F6".into(),
                fill: "#3B82F6".into(),
                fill_opacity: 0.10,
                rings: md.rings.clone(),
            })
            .collect();
        draw_spc(pm, vp, &areas);
    }
}

/// Header strip across the top of a comparison panel: product name left, time right.
pub fn draw_panel_header(pm: &mut Pixmap, glyphs: &mut GlyphCache, title: &str, time: Option<&str>, brand: &Brand) {
    let w = pm.width() as f32;
    let bar_h = 46.0;
    if let Some(rect) = Rect::from_xywh(0.0, 0.0, w, bar_h) {
        pm.fill_rect(rect, &paint(panel_color(brand, 0.86)), Transform::identity(), None);
    }
    if let Some(rect) = Rect::from_xywh(0.0, bar_h - 4.0, w, 4.0) {
        pm.fill_rect(rect, &paint(parse_color("$primary", brand)), Transform::identity(), None);
    }
    let st = style(&brand.font_display, Weight::Bold, 26.0, parse_color("$text", brand));
    draw_line(pm, glyphs, title, 18.0, 32.0, &st, 1.0, None);
    if let Some(time) = time {
        let ts = style(&brand.font_display, Weight::Bold, 24.0, parse_color("$primary", brand));
        let tw = measure(time, &ts);
        draw_line(pm, glyphs, time, w - tw - 18.0, 32.0, &ts, 1.0, None);
    }
}

/// On-air elements: title banner, legend, radar time, station bug and clock.
pub fn draw_onair(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    scene: &MapScene,
    settings: &Settings,
    radar_time: Option<i64>,
    data_time: Option<i64>,
    logo: Option<&Pixmap>,
    sponsor_logo: Option<&Pixmap>,
) {
    let brand = &settings.brand;
    let w = pm.width() as f32;
    let h = pm.height() as f32;
    let primary = parse_color("$primary", brand);
    // Elements the presenter dragged (Arrange tool) are drawn at their offsets.
    let off = |key: &str| scene.onair_offsets.get(key).copied().unwrap_or((0.0, 0.0));

    let title = scene.banner_text();
    if scene.show_title && !title.trim().is_empty() {
        let title = if scene.automatic_title() { title.to_uppercase() } else { title.into_owned() };
        let (dx, dy) = off("title");
        let st = style(&brand.font_display, Weight::Black, 50.0, parse_color("$text", brand));
        let text_w = measure(&title, &st);
        let bw = text_w + 90.0;
        if let Some(path) = rounded_rect(40.0 + dx, 40.0 + dy, bw, 84.0, 10.0) {
            pm.fill_path(&path, &paint(with_alpha(parse_color("$secondary", brand), 0.92)), FillRule::Winding, Transform::identity(), None);
        }
        if let Some(rect) = Rect::from_xywh(40.0 + dx, 40.0 + dy, 14.0, 84.0) {
            pm.fill_rect(rect, &paint(primary), Transform::identity(), None);
        }
        let (ascent, descent) = line_metrics(&st);
        draw_line(pm, glyphs, &title, 80.0 + dx, 40.0 + dy + (84.0 - ascent - descent) / 2.0 + ascent, &st, 1.0, None);
    }

    use super::scene::RadarSource;
    let site_radar = scene.radar.enabled && scene.radar.source == RadarSource::Site;
    // Map theme legend placement and colours.
    let theme = crate::maptheme::get(&scene.theme);
    let legend_hidden = theme.as_ref().map(|t| t.legend_position == "hidden").unwrap_or(false);
    let legend_off = {
        let (dx, dy) = off("legend");
        match theme.as_ref().map(|t| t.legend_position.as_str()) {
            Some("bottom-right") => (dx + w - 700.0, dy),
            Some("top-right") => (dx + w - 700.0, dy - (h - 290.0)),
            _ => (dx, dy),
        }
    };
    if legend_hidden {
    } else if scene.show_legend && scene.data.field.is_some() {
        if let Some(field) = scene.data.field {
            use crate::futurecast::Product;
            use super::scene::DataField;
            match (field, scene.data.model_product) {
                (DataField::Futurecast, product @ (Product::GfsReflectivity | Product::HrrrReflectivity | Product::PrecipitationRate)) => {
                    super::layers::draw_model_scale_legend(pm, glyphs, brand, legend_off, product);
                }
                (DataField::Futurecast, product @ (Product::SnowfallChance | Product::IceChance)) => {
                    super::layers::draw_named_field_legend(pm, glyphs, DataField::SnowChance, brand, legend_off, Some(product.label()));
                }
                (DataField::Futurecast, product) => {
                    let legend = match product {
                        Product::Temperature => DataField::Temperature,
                        Product::TotalPrecip => DataField::RainTotal,
                        Product::DewPoint => DataField::DewPoint,
                        Product::Humidity => DataField::Humidity,
                        Product::Wind => DataField::WindSpeed,
                        Product::Gust => DataField::WindGust,
                        Product::Pressure => DataField::Pressure,
                        Product::Snowfall | Product::SnowfallLow | Product::SnowfallHigh => DataField::SnowTotal,
                        Product::Ice | Product::IceLow | Product::IceHigh => DataField::IceTotal,
                        _ => field,
                    };
                    super::layers::draw_field_legend(pm, glyphs, legend, brand, legend_off);
                }
                _ => super::layers::draw_field_legend(pm, glyphs, field, brand, legend_off),
            }
        }
    } else if scene.show_legend && !scene.radar.enabled && !scene.satellite.enabled && (scene.spc_day.is_some() || scene.ero_day.is_some() || scene.fire_outlook) {
        super::layers::draw_outlook_legend(pm, glyphs, brand, legend_off, scene.spc_day, &scene.spc_layer, scene.ero_day.is_some());
    } else if scene.show_legend && !scene.radar.enabled && !scene.satellite.enabled && scene.quakes {
        super::layers::draw_quake_legend(pm, glyphs, brand, legend_off, scene.quake_days, scene.quake_min);
    } else if scene.show_legend && !scene.radar.enabled && !scene.satellite.enabled && scene.drought {
        super::layers::draw_drought_legend(pm, glyphs, brand, legend_off, &scene.drought_view);
    } else if scene.show_legend && site_radar {
        let site = scene.radar.resolved_site(scene.view.lat, scene.view.lon);
        super::layers::draw_site_legend(pm, glyphs, scene.radar.product, &site, brand, legend_off, scene.radar.palette.trim() == "4", scene.radar.site_min_dbz);
    } else if scene.show_legend && scene.radar.enabled && scene.radar_replay.is_none() && scene.history_time.is_none() {
        let lw = 620.0;
        let snow_row = scene.radar.palette.trim() == "4" && super::winter_radar::enabled() && super::winter_radar::snow_recent();
        let lh = if snow_row { 140.0 } else { 96.0 };
        let lx = 40.0 + legend_off.0;
        let ly = h - lh - 40.0 + legend_off.1;
        let bg = theme.as_ref().map(|t| crate::maptheme::color(&t.legend_background, panel_color(brand, 0.82))).unwrap_or_else(|| panel_color(brand, 0.82));
        let fg = theme.as_ref().map(|t| crate::maptheme::color(&t.legend_text, parse_color("$text", brand))).unwrap_or_else(|| parse_color("$text", brand));
        if let Some(path) = rounded_rect(lx, ly, lw, lh, 12.0) {
            pm.fill_path(&path, &paint(bg), FillRule::Winding, Transform::identity(), None);
        }
        draw_legend(pm, glyphs, &scene.radar.palette, (lx + 18.0, ly + 8.0, lw - 36.0, 80.0), fg, &brand.font_body);
        if snow_row {
            super::winter_radar::draw_snow_bar(pm, glyphs, (lx + 18.0, ly + 92.0, lw - 36.0, 40.0), fg, &brand.font_body);
        }
    }

    // Time box: data/Futurecast valid time, the latest single-site scan, or the radar frame.
    let (kicker, time) = match scene.data.field {
        Some(field) if data_time.is_some() => {
            let future = data_time.map(|t| t - chrono::Utc::now().timestamp() > 1800).unwrap_or(false);
            let kicker = match field {
                super::scene::DataField::Futurecast => format!("{} {}", scene.data.model_product.source().label(), scene.data.model_product.label().to_uppercase()),
                _ if future => "FORECAST VALID".to_string(),
                f => f.viewer_label().to_uppercase(),
            };
            (kicker, data_time)
        }
        Some(_) => (String::new(), None),
        _ if scene.site_replay.is_some() => ("OBSERVED VELOCITY REPLAY".to_string(), radar_time),
        _ if site_radar => (
            format!("K{} LATEST SCAN", scene.radar.resolved_site(scene.view.lat, scene.view.lon)),
            Some(chrono::Utc::now().timestamp()),
        ),
        _ if scene.radar_replay.is_some() || scene.history_time.is_some() => ("OBSERVED RADAR REPLAY".to_string(), radar_time),
        _ => ("RADAR".to_string(), radar_time),
    };
    if scene.show_timestamp && (scene.radar.enabled || scene.data.field.is_some()) {
        if let Some(t) = time.and_then(|t| chrono::DateTime::from_timestamp(t, 0)) {
            let (dx, dy) = off("time");
            let local = t.with_timezone(&chrono::Local);
            let dated = scene.data.loop_hours > 168 || scene.is_radar_replay();
            let format = match (dated, settings.clock_24h) {
                (true, true) => "%b %-d %H:%M",
                (true, false) => "%b %-d %-I:%M %p",
                (false, true) => "%a %H:%M",
                (false, false) => "%a %-I:%M %p",
            };
            let label = local.format(format).to_string();
            // A live radar time more than 15 minutes old turns amber, so a paused or stalled loop
            // under a LIVE title can't go unnoticed (replays and forecasts are meant to be old/new).
            let live_radar = scene.data.field.is_none() && !scene.is_radar_replay() && scene.history_time.is_none() && !site_radar;
            let stale = live_radar && chrono::Utc::now().timestamp() - t.timestamp() > 15 * 60;
            let time_color = if stale { Color::from_rgba8(255, 190, 40, 255) } else { parse_color("$text", brand) };
            // Compact by default: the source line and time read clearly without competing
            // with the radar.
            let st = style(&brand.font_display, Weight::Bold, 30.0, time_color);
            let small = style(&brand.font_body, Weight::Bold, 14.0, primary);
            let tw = measure(&label, &st).max(measure(&kicker, &small));
            let bx = 680.0 + dx;
            let by = h - 112.0 + dy;
            if let Some(path) = rounded_rect(bx, by, tw + 36.0, 72.0, 10.0) {
                pm.fill_path(&path, &paint(panel_color(brand, 0.82)), FillRule::Winding, Transform::identity(), None);
            }
            draw_line(pm, glyphs, &kicker, bx + 18.0, by + 24.0, &small, 1.0, None);
            draw_line(pm, glyphs, &label, bx + 18.0, by + 58.0, &st, 1.0, None);
        }
    }

    // Data credits, bottom edge: the free sources' terms ask for attribution on air.
    {
        let mut sources: Vec<&str> = Vec::new();
        if scene.radar.enabled {
            sources.push(if site_radar { "NWS NEXRAD via IEM" } else { "LibreWXR" });
        }
        if let Some(field) = scene.data.field {
            // NOAA products (Futurecast frames) and the GFS grid come from NOAA; air quality and
            // the other grid models from Open-Meteo.
            let noaa = field == crate::map::scene::DataField::Futurecast || crate::grid::grid_from_noaa(scene.data.grid_model, scene.view);
            sources.push(if noaa { "NOAA" } else { "Open-Meteo" });
            if noaa && field.needs_air() {
                sources.push("Air quality: Open-Meteo");
            }
        }
        if scene.satellite.enabled {
            sources.push("NOAA GOES via IEM");
        }
        if scene.warnings || scene.spc_day.is_some() {
            sources.push("NWS / SPC");
        }
        if scene.lightning {
            sources.push("Lightning: NOAA GOES GLM");
        }
        if scene.roads {
            sources.push("INDOT");
        }
        sources.push("Map: Esri");
        let credit = format!("Data: {}", sources.join(" • "));
        // Its own small panel along the bottom edge, on the side away from the legend (it was
        // drawn bare under the legend and ran off the frame: CWXB-97).
        let cs = style(&brand.font_body, Weight::SemiBold, 15.0, with_alpha(parse_color("$text", brand), 0.85));
        let tw = measure(&credit, &cs);
        let legend_right = theme.as_ref().is_some_and(|t| t.legend_position == "bottom-right");
        let cx = if legend_right { 40.0 } else { w - 40.0 - tw - 20.0 };
        let cy = h - 34.0;
        if let Some(path) = rounded_rect(cx, cy, tw + 20.0, 26.0, 6.0) {
            pm.fill_path(&path, &paint(panel_color(brand, 0.7)), FillRule::Winding, Transform::identity(), None);
        }
        draw_line(pm, glyphs, &credit, cx + 10.0, cy + 18.0, &cs, 1.0, None);
    }

    if scene.shows_watermark() {
        let (dx, dy) = scene.watermark_offset();
        draw_wordmark(pm, glyphs, settings, w - 40.0 + dx, 102.0 + dy);
    }
    if scene.show_logo {
        let (dx, dy) = off("logo");
        let bug_h = 84.0;
        let by = if scene.shows_watermark() { 134.0 + dy } else { 40.0 + dy };
        if let Some(logo) = logo {
            let scale = bug_h / logo.height() as f32;
            let lw = logo.width() as f32 * scale;
            let ts = Transform::from_translate(w - lw - 40.0 + dx, by).pre_scale(scale, scale);
            pm.draw_pixmap(
                0,
                0,
                logo.as_ref(),
                &tiny_skia::PixmapPaint {
                    opacity: 0.95,
                    quality: tiny_skia::FilterQuality::Bicubic,
                    ..Default::default()
                },
                ts,
                None,
            );
        }
    }

    if scene.show_sponsor && !settings.sponsor_name.trim().is_empty() {
        let (x, y, badge_width, badge_height) = scene.sponsor_rect(w);
        let size = badge_width / 440.0;
        if let Some(path) = rounded_rect(x, y, badge_width, badge_height, 10.0 * size) {
            pm.fill_path(&path, &paint(with_alpha(parse_color("$secondary", brand), 0.94)), FillRule::Winding, Transform::identity(), None);
        }
        let text_x = if let Some(logo) = sponsor_logo {
            let scale = (104.0 / logo.width() as f32).min(82.0 / logo.height() as f32) * size;
            let transform = Transform::from_translate(x + 14.0 * size, y + (badge_height - logo.height() as f32 * scale) * 0.5)
                .pre_scale(scale, scale);
            pm.draw_pixmap(0, 0, logo.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() }, transform, None);
            x + 132.0 * size
        } else {
            x + 20.0 * size
        };
        let caption = if settings.sponsor_tagline.trim().is_empty() { "Sponsored by" } else { &settings.sponsor_tagline };
        let mut caption_style = style(&brand.font_body, Weight::Regular, 20.0 * size, parse_color("$primary", brand));
        let mut name_style = style(&brand.font_display, Weight::Black, 36.0 * size, parse_color("$text", brand));
        let available = x + 420.0 * size - text_x;
        for (text, baseline, text_style) in [(caption, y + 40.0 * size, &mut caption_style), (settings.sponsor_name.as_str(), y + 84.0 * size, &mut name_style)] {
            let scale = (available / measure(text, text_style).max(1.0)).min(1.0);
            draw_line(pm, glyphs, text, text_x, baseline, text_style, scale, None);
        }
    }

    if scene.show_clock {
        let (dx, dy) = off("clock");
        let now = chrono::Local::now();
        let label = if settings.clock_24h {
            now.format("%H:%M").to_string()
        } else {
            now.format("%-I:%M").to_string()
        };
        let st = style(&brand.font_display, Weight::Bold, 46.0, parse_color("$text", brand));
        let tw = measure(&label, &st);
        let bw = tw + 60.0;
        let bx = w - bw - 40.0 + dx;
        let by = h - 136.0 + dy;
        if let Some(path) = rounded_rect(bx, by, bw, 96.0, 12.0) {
            pm.fill_path(&path, &paint(panel_color(brand, 0.82)), FillRule::Winding, Transform::identity(), None);
        }
        if let Some(rect) = Rect::from_xywh(bx, by + 88.0, bw, 8.0) {
            pm.fill_rect(rect, &paint(primary), Transform::identity(), None);
        }
        draw_line(pm, glyphs, &label, bx + 30.0, by + 66.0, &st, 1.0, None);
    }
}

/// Approximate boxes of the draggable on-air elements (frame px, including their offsets), for
/// the Arrange tool's hit-testing. Later entries are on top.
pub fn onair_boxes(scene: &MapScene, w: f32, h: f32, has_logo: bool, has_sponsor: bool) -> Vec<(&'static str, (f32, f32, f32, f32))> {
    let off = |key: &str| scene.onair_offsets.get(key).copied().unwrap_or((0.0, 0.0));
    let mut boxes = Vec::new();
    if scene.storm_track.is_some() {
        let (dx, dy) = off("eta");
        boxes.push(("eta", (w - 560.0 + dx, h - 600.0 + dy, 520.0, 450.0)));
    }
    let title = scene.banner_text();
    if scene.show_title && !title.trim().is_empty() {
        let title = if scene.automatic_title() { title.to_uppercase() } else { title.into_owned() };
        let (dx, dy) = off("title");
        boxes.push(("title", (40.0 + dx, 40.0 + dy, 90.0 + title.chars().count() as f32 * 30.0, 84.0)));
    }
    if scene.show_legend {
        let (dx, dy) = off("legend");
        boxes.push(("legend", (40.0 + dx, h - 136.0 + dy, 620.0, 96.0)));
    }
    if scene.show_timestamp {
        let (dx, dy) = off("time");
        boxes.push(("time", (680.0 + dx, h - 112.0 + dy, 280.0, 72.0)));
    }
    if scene.shows_watermark() {
        let (dx, dy) = scene.watermark_offset();
        boxes.push(("watermark", (w - 480.0 + dx, 40.0 + dy, 440.0, 84.0)));
    }
    if scene.show_logo && has_logo {
        let (dx, dy) = off("logo");
        let y = if scene.shows_watermark() { 134.0 } else { 40.0 };
        boxes.push(("logo", (w - 480.0 + dx, y + dy, 440.0, 84.0)));
    }
    if scene.show_sponsor && has_sponsor {
        boxes.push(("sponsor", scene.sponsor_rect(w)));
    }
    if scene.show_clock {
        let (dx, dy) = off("clock");
        boxes.push(("clock", (w - 280.0 + dx, h - 136.0 + dy, 240.0, 96.0)));
    }
    boxes
}

/// The station's name as a watermark, right-aligned at `right`: the active brand's short name
/// (so switching brand switches it), with the OpenWX two-tone style for that brand.
fn draw_wordmark(pm: &mut Pixmap, glyphs: &mut GlyphCache, settings: &Settings, right: f32, baseline: f32) {
    let name = [&settings.station_short, &settings.station_name].into_iter().map(|n| n.trim()).find(|n| !n.is_empty()).unwrap_or("OpenWX").to_uppercase();
    // One-word names ending in "WX" (OPENWX) split before the "WX"; other names put the last
    // word in the brand colour.
    let (head, tail) = if !name.contains(' ') && name.len() > 2 && name.ends_with("WX") {
        (name[..name.len() - 2].to_string(), "WX".to_string())
    } else if let Some((first, last)) = name.rsplit_once(' ') {
        (format!("{first} "), last.to_string())
    } else {
        (name.clone(), String::new())
    };
    let (head, tail) = (head.as_str(), tail.as_str());
    // 54 px, smaller for long names so it stays inside its 440 px arrange box.
    let at = |size: f32| {
        let white = style(&settings.brand.font_display, Weight::Black, size, parse_color("$text", &settings.brand));
        let cyan = style(&settings.brand.font_display, Weight::Black, size, parse_color("$primary", &settings.brand));
        let total = measure(head, &white) + measure(tail, &cyan);
        (white, cyan, total)
    };
    let (_, _, full) = at(54.0);
    let (white, cyan, total) = at((54.0 * (430.0 / full.max(1.0))).clamp(28.0, 54.0));
    let x = right - total;
    halo_text(pm, glyphs, head, x, baseline, &white);
    halo_text(pm, glyphs, tail, x + measure(head, &white), baseline, &cyan);
}

#[cfg(test)]
mod badge_tests {
    use super::*;

    /// The empty-outlook note (CWX_QA_NEW_DIR set).
    #[test]
    fn qa_empty_outlook_note() {
        let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) else { return };
        let mut pm = Pixmap::new(1920, 1080).unwrap();
        pm.fill(Color::from_rgba8(20, 30, 44, 255));
        let brand = crate::config::Settings::default().brand;
        let mut glyphs = GlyphCache::default();
        draw_empty_note(&mut pm, &mut glyphs, &brand, "No severe storms expected in this area today");
        pm.save_png(dir.join("empty-outlook-note.png")).unwrap();
    }

    /// Renders storm badges on a dark map (CWX_QA_NEW_DIR set) for a visual check.
    #[test]
    fn qa_storm_badges() {
        let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) else { return };
        let mut pm = Pixmap::new(800, 400).unwrap();
        pm.fill(Color::from_rgba8(40, 42, 48, 255));
        let vp = Viewport::new(crate::config::GeoView { lat: 38.4, lon: -86.9, zoom: 8.0 }, 800, 400);
        let mk = |dlon: f64, level: u8, strikes: u32, hail: Option<&str>| crate::data::StormCell {
            lat: 38.4,
            lon: -86.9 + dlon,
            intensity: crate::data::CellIntensity { level, label: "Very heavy".into(), hail: hail.map(String::from) },
            lightning: crate::data::CellLightning { strikes, ..Default::default() },
            motion: Some(crate::data::CellMotion { speed_kmh: 60.0, speed_mph: 37.0, heading_deg: 80.0, ..Default::default() }),
            ..Default::default()
        };
        let cells = vec![mk(-1.0, 2, 0, None), mk(-0.4, 3, 0, None), mk(0.2, 3, 12, None), mk(0.8, 4, 40, Some("small hail possible"))];
        let mut glyphs = GlyphCache::default();
        draw_storm_cells(&mut pm, &mut glyphs, &vp, &cells, true, true, &crate::config::Settings::default().brand);
        pm.save_png(dir.join("storm-badges.png")).unwrap();
    }

    /// On-air legend and data credit (CWX_QA_NEW_DIR set): radar (palette 4), ground temperature
    /// and air temperature, stacked. The credit must sit in its own panel, clear of the legend.
    #[test]
    fn qa_onair_legend_credit() {
        let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) else { return };
        use crate::map::scene::DataField;
        let settings = Settings::default();
        let mut glyphs = GlyphCache::default();
        let (w, h) = (1920u32, 1080u32);
        let mut sheet = Pixmap::new(w, h * 3 / 2).unwrap();
        let fields: [Option<DataField>; 3] = [None, Some(DataField::GroundTemp), Some(DataField::Temperature)];
        for (i, field) in fields.into_iter().enumerate() {
            let mut pm = Pixmap::new(w, h).unwrap();
            pm.fill(Color::from_rgba8(46, 52, 60, 255));
            let mut scene = MapScene::from_settings(&settings);
            scene.radar.enabled = field.is_none();
            scene.radar.palette = "4".into();
            scene.data.field = field;
            scene.warnings = true;
            scene.show_legend = true;
            scene.show_timestamp = false;
            draw_onair(&mut pm, &mut glyphs, &scene, &settings, None, None, None, None);
            // Keep the bottom half of each frame (legend, credit, clock).
            let strip = tiny_skia::Pixmap::from_vec(
                pm.data()[(h as usize / 2) * w as usize * 4..].to_vec(),
                tiny_skia::IntSize::from_wh(w, h / 2).unwrap(),
            )
            .unwrap();
            sheet.draw_pixmap(0, (i as u32 * h / 2) as i32, strip.as_ref(), &tiny_skia::PixmapPaint::default(), Transform::identity(), None);
        }
        sheet.save_png(dir.join("onair-legend-credit.png")).unwrap();
    }

    /// The watermark follows the brand (CWX_QA_NEW_DIR set: renders both brands).
    #[test]
    fn qa_watermark_follows_brand() {
        let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) else { return };
        let mut glyphs = GlyphCache::default();
        let mut pm = Pixmap::new(960, 260).unwrap();
        pm.fill(Color::from_rgba8(40, 42, 48, 255));
        let mut settings = Settings::default();
        settings.station_short = "OpenWX".into();
        draw_wordmark(&mut pm, &mut glyphs, &settings, 920.0, 90.0);
        settings.station_short = "Dubois County Weather".into();
        draw_wordmark(&mut pm, &mut glyphs, &settings, 920.0, 200.0);
        // The 440 px arrange box, for reference.
        for y in [40.0, 150.0] {
            if let Some(r) = tiny_skia::Rect::from_xywh(480.0, y, 440.0, 84.0) {
                if let Some(path) = tiny_skia::PathBuilder::from_rect(r).into() {
                    pm.stroke_path(&path, &paint(Color::from_rgba8(255, 0, 0, 160)), &tiny_skia::Stroke::default(), Transform::identity(), None);
                }
            }
        }
        pm.save_png(dir.join("watermark-brands.png")).unwrap();
    }
}
