//! Extra map layers: river gauges, warming/cooling centers, lightning density, and town labels
//! inside the called-out warning.

use tiny_skia::{Color, FillRule, PathBuilder, Pixmap, Rect, Transform};

use crate::{
    config::Brand,
    data::{Alert, Strike},
    feeds::RiverGauge,
    gfx::text::{measure, GlyphCache, Weight},
    map::{
        geo::Viewport,
        overlays::{halo_text, paint, stroke, style},
    },
};

fn on_screen(vp: &Viewport, x: f32, y: f32, pad: f32) -> bool {
    x > -pad && y > -pad && x < vp.width as f32 + pad && y < vp.height as f32 + pad
}

/// River gauges as flood-category coloured diamonds; named when flooding or zoomed in. Zoomed
/// out, only gauges at action stage or above are drawn: dozens of "normal" diamonds read as
/// clutter (and their colours as flooding) on air.
pub fn draw_rivers(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, gauges: &[RiverGauge], brand: &Brand) {
    let label = style(&brand.font_body, Weight::Bold, 18.0, Color::WHITE);
    let zoomed = vp.view.zoom >= 8.0;
    // Worst last so it draws on top.
    for g in gauges.iter().rev() {
        let (x, y) = vp.to_screen(g.lat, g.lon);
        if !on_screen(vp, x, y, 20.0) {
            continue;
        }
        let status = g.worst_status();
        if !zoomed && RiverGauge::severity(status) == 0 {
            continue;
        }
        let [r, gg, b] = RiverGauge::status_rgb(status);
        let size = if RiverGauge::severity(status) >= 2 { 13.0 } else { 9.0 };
        let mut pb = PathBuilder::new();
        pb.move_to(x, y - size);
        pb.line_to(x + size, y);
        pb.line_to(x, y + size);
        pb.line_to(x - size, y);
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(r, gg, b, 255)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 220)), &stroke(2.5), Transform::identity(), None);
        }
        if RiverGauge::severity(status) >= 1 || zoomed {
            let stage = g.observed.map(|v| format!(" {v:.1} ft")).unwrap_or_default();
            halo_text(pm, glyphs, &format!("{}{stage}", g.location), x + size + 6.0, y + 6.0, &label);
        }
    }
}

/// A warming or cooling center from the community board.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Center {
    pub name: String,
    /// "warming" or "cooling".
    pub kind: String,
    pub address: String,
    pub hours: String,
    pub lat: f64,
    pub lon: f64,
}

pub fn draw_centers(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, centers: &[Center], brand: &Brand) {
    let letter = style(&brand.font_display, Weight::Black, 18.0, Color::WHITE);
    let label = style(&brand.font_body, Weight::Bold, 18.0, Color::WHITE);
    for c in centers {
        let (x, y) = vp.to_screen(c.lat, c.lon);
        if !on_screen(vp, x, y, 30.0) {
            continue;
        }
        let warming = c.kind.eq_ignore_ascii_case("warming");
        let shelter = c.kind.eq_ignore_ascii_case("shelter");
        let fill = if shelter {
            Color::from_rgba8(40, 170, 90, 255)
        } else if warming {
            Color::from_rgba8(235, 110, 40, 255)
        } else {
            Color::from_rgba8(40, 150, 235, 255)
        };
        if let Some(rect) = Rect::from_xywh(x - 14.0, y - 14.0, 28.0, 28.0) {
            let path = PathBuilder::from_rect(rect);
            pm.fill_path(&path, &paint(fill), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(255, 255, 255, 240)), &stroke(2.5), Transform::identity(), None);
        }
        let l = if shelter { "S" } else if warming { "W" } else { "C" };
        let w = measure(l, &letter);
        crate::gfx::text::draw_line(pm, glyphs, l, x - w / 2.0, y + 7.0, &letter, 1.0, None);
        halo_text(pm, glyphs, &c.name, x + 20.0, y + 6.0, &label);
    }
}

/// Strike density over the last `minutes`: strikes counted in screen cells, coloured by count.
pub fn draw_lightning_density(pm: &mut Pixmap, vp: &Viewport, strikes: &[Strike], minutes: i64) {
    let now = chrono::Utc::now().timestamp_millis();
    let cell = 36.0f32;
    let cols = (vp.width as f32 / cell).ceil() as usize + 1;
    let rows = (vp.height as f32 / cell).ceil() as usize + 1;
    let mut counts = vec![0u32; cols * rows];
    for s in strikes.iter().filter(|s| now - s.time_ms <= minutes * 60_000) {
        let (x, y) = vp.to_screen(s.lat, s.lon);
        if x < 0.0 || y < 0.0 || x >= vp.width as f32 || y >= vp.height as f32 {
            continue;
        }
        counts[(y / cell) as usize * cols + (x / cell) as usize] += 1;
    }
    for (i, n) in counts.iter().enumerate() {
        if *n == 0 {
            continue;
        }
        let (r, g, b, a) = match n {
            1 => (120, 120, 255, 90),
            2..=3 => (60, 200, 255, 120),
            4..=7 => (255, 230, 60, 150),
            8..=15 => (255, 140, 30, 170),
            _ => (255, 40, 40, 190),
        };
        let x = (i % cols) as f32 * cell;
        let y = (i / cols) as f32 * cell;
        if let Some(rect) = Rect::from_xywh(x + 1.0, y + 1.0, cell - 2.0, cell - 2.0) {
            pm.fill_rect(rect, &paint(Color::from_rgba8(r, g, b, a)), Transform::identity(), None);
        }
    }
}

/// Forecast regions: outline, light fill, name and the region's high.
pub fn draw_regions(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, data: &crate::data::WxSnapshot, brand: &Brand) {
    use crate::forecast_edit::Shape;
    let name_style = style(&brand.font_display, Weight::Bold, 26.0, Color::WHITE);
    for region in &data.forecast_edits.regions {
        let ring: Vec<(f32, f32)> = match &region.shape {
            Shape::Polygon(pts) => pts.iter().map(|(la, lo)| vp.to_screen(*la, *lo)).collect(),
            Shape::Circle { lat, lon, radius_mi } => (0..48)
                .map(|k| {
                    let (la, lo) = crate::map::geo::destination(*lat, *lon, k as f64 * 7.5, radius_mi * 1.609_344);
                    vp.to_screen(la, lo)
                })
                .collect(),
        };
        if ring.len() < 3 {
            continue;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(ring[0].0, ring[0].1);
        for (x, y) in &ring[1..] {
            pb.line_to(*x, *y);
        }
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(91, 147, 238, 40)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(91, 147, 238, 230)), &stroke(3.0), Transform::identity(), None);
        }
        let (clat, clon) = region.shape.center();
        let (x, y) = vp.to_screen(clat, clon);
        let hi = data.grid.as_deref().and_then(|g| {
            (0..=24)
                .filter_map(|h| g.sample(crate::grid::Var::Temp, g.time_index(h as f32), clat, clon))
                .reduce(f32::max)
                .map(|v| v + region.hi_delta)
        });
        let label = match hi {
            Some(h) => format!("{}  {h:.0}°", region.name),
            None => region.name.clone(),
        };
        let w = measure(&label, &name_style);
        halo_text(pm, glyphs, &label, x - w / 2.0, y, &name_style);
    }
}

/// Strikes per 5 minutes over the last hour, oldest first (12 buckets), inside `radius_mi` of a
/// point (0 = everywhere).
pub fn flash_rate(strikes: &[Strike], lat: f64, lon: f64, radius_mi: f64) -> Vec<u32> {
    let now = chrono::Utc::now().timestamp_millis();
    let mut buckets = vec![0u32; 12];
    for s in strikes {
        let age_min = (now - s.time_ms) as f64 / 60_000.0;
        if !(0.0..60.0).contains(&age_min) {
            continue;
        }
        if radius_mi > 0.0 && crate::map::geo::haversine_km(lat, lon, s.lat, s.lon) * 0.621371 > radius_mi {
            continue;
        }
        buckets[11 - (age_min / 5.0) as usize] += 1;
    }
    buckets
}

/// Towns inside the called-out warning, dotted and named in bold so viewers find their town.
pub fn draw_warning_towns(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, alert: &Alert, brand: &Brand) {
    let (mut n, mut s, mut w, mut e) = (f64::MIN, f64::MAX, f64::MAX, f64::MIN);
    for ring in &alert.polygons {
        for (lon, lat) in ring {
            n = n.max(*lat);
            s = s.min(*lat);
            w = w.min(*lon);
            e = e.max(*lon);
        }
    }
    if n == f64::MIN {
        return;
    }
    let label = style(&brand.font_display, Weight::Bold, 24.0, Color::WHITE);
    let mut placed: Vec<(f32, f32, f32)> = Vec::new();
    for place in crate::places::in_box(s, n, w, e).iter().filter(|p| alert.contains(p.lat, p.lon)).take(14) {
        let (x, y) = vp.to_screen(place.lat, place.lon);
        let tw = measure(&place.name, &label);
        // Keep labels inside the title-safe area (5% in from each edge).
        let (sx, sy) = (vp.width as f32 * 0.05, vp.height as f32 * 0.05);
        if x < sx || y < sy + 20.0 || x + 12.0 + tw > vp.width as f32 - sx || y > vp.height as f32 - sy {
            continue;
        }
        // Skip labels that would overlap one already placed (largest towns come first).
        if placed.iter().any(|(px, py, pw)| (x - px).abs() < (tw + pw) / 2.0 + 10.0 && (y - py).abs() < 30.0) {
            continue;
        }
        placed.push((x + 12.0 + tw / 2.0, y, tw));
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, 6.0);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 230)), &stroke(2.5), Transform::identity(), None);
        }
        halo_text(pm, glyphs, &place.name, x + 12.0, y + 8.0, &label);
    }
}

/// Damage-producing reports in the last `hours`: tornado (red), wind damage (blue) and hail 1"+
/// (green), sized by magnitude, with tornado reports joined into tracks and a summary box.
pub fn draw_damage_map(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, reports: &[crate::data::StormReport], hours: u32, brand: &Brand) {
    let since = chrono::Utc::now().timestamp() - hours as i64 * 3600;
    let mag = |r: &crate::data::StormReport| r.magnitude.trim().trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.').parse::<f64>().ok();
    let damage: Vec<&crate::data::StormReport> = reports
        .iter()
        .filter(|r| r.time >= since)
        .filter(|r| match r.kind.as_str() {
            "T" | "C" => true,
            "D" | "G" | "M" | "N" | "W" => r.kind == "D" || mag(r).map(|m| m >= 58.0).unwrap_or(true),
            "H" => mag(r).map(|m| m >= 1.0).unwrap_or(false),
            _ => false,
        })
        .collect();
    // Tornado tracks: reports within 25 miles and 90 minutes of each other, in time order.
    let mut tornado: Vec<&&crate::data::StormReport> = damage.iter().filter(|r| r.kind == "T" || r.kind == "C").collect();
    tornado.sort_by_key(|r| r.time);
    let mut track = PathBuilder::new();
    for pair in tornado.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let km = crate::map::geo::haversine_km(a.lat, a.lon, b.lat, b.lon);
        if km < 40.0 && (b.time - a.time) <= 5400 && b.time >= a.time {
            let (x1, y1) = vp.to_screen(a.lat, a.lon);
            let (x2, y2) = vp.to_screen(b.lat, b.lon);
            track.move_to(x1, y1);
            track.line_to(x2, y2);
        }
    }
    if let Some(path) = track.finish() {
        pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 180)), &stroke(9.0), Transform::identity(), None);
        pm.stroke_path(&path, &paint(Color::from_rgba8(230, 30, 40, 255)), &stroke(5.0), Transform::identity(), None);
    }
    let label = style(&brand.font_body, Weight::Bold, 20.0, Color::WHITE);
    let (mut nt, mut nw, mut nh) = (0, 0, 0);
    for r in &damage {
        let (x, y) = vp.to_screen(r.lat, r.lon);
        let (rgb, radius, text) = match r.kind.as_str() {
            "T" | "C" => {
                nt += 1;
                ([230, 30, 40], 11.0, "Tornado".to_string())
            }
            "H" => {
                nh += 1;
                let m = mag(r).unwrap_or(1.0);
                ([40, 190, 80], (6.0 + m * 4.0) as f32, format!("{m:.2}\" hail"))
            }
            _ => {
                nw += 1;
                let m = mag(r);
                ([40, 120, 240], m.map(|m| (5.0 + (m - 50.0).max(0.0) / 4.0) as f32).unwrap_or(8.0), m.map(|m| format!("{m:.0} mph")).unwrap_or_else(|| "Wind damage".into()))
            }
        };
        if !on_screen(vp, x, y, 20.0) {
            continue;
        }
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, radius.min(22.0));
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(rgb[0], rgb[1], rgb[2], 220)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), Transform::identity(), None);
        }
        if vp.view.zoom >= 8.0 {
            halo_text(pm, glyphs, &text, x + radius.min(22.0) + 6.0, y + 7.0, &label);
        }
    }
    // Summary box, lower left inside title safe.
    let title = style(&brand.font_display, Weight::Bold, 26.0, Color::WHITE);
    let body = style(&brand.font_body, Weight::SemiBold, 22.0, Color::WHITE);
    let lines = [
        format!("Damage reports, last {hours} hours"),
        format!("{nt} tornado • {nw} wind • {nh} hail"),
    ];
    let w = lines.iter().enumerate().map(|(i, l)| measure(l, if i == 0 { &title } else { &body })).fold(0.0, f32::max) + 36.0;
    let (x0, y0) = (vp.width as f32 * 0.05, vp.height as f32 * 0.95 - 96.0);
    if let Some(rect) = Rect::from_xywh(x0, y0, w, 86.0) {
        pm.fill_rect(rect, &paint(crate::map::overlays::panel_color(brand, 0.88)), Transform::identity(), None);
    }
    halo_text(pm, glyphs, &lines[0], x0 + 18.0, y0 + 36.0, &title);
    halo_text(pm, glyphs, &lines[1], x0 + 18.0, y0 + 70.0, &body);
}

/// Flood extent: flood and flash flood warnings filled deep blue, gauges at or above flood stage,
/// and road reports that mention water (or sit inside a flood warning) marked as flooded roads.
pub fn draw_flood_map(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, data: &crate::data::WxSnapshot, brand: &Brand) {
    let floods: Vec<&Alert> = data
        .map_alerts
        .iter()
        .chain(data.local_alerts.iter())
        .filter(|a| a.event.to_lowercase().contains("flood") && !a.event.to_lowercase().contains("watch"))
        .collect();
    for a in &floods {
        let emergency = a.threat_tag().is_some();
        let fill = if a.event.to_lowercase().contains("flash") { Color::from_rgba8(20, 80, 200, 130) } else { Color::from_rgba8(30, 140, 110, 120) };
        for ring in &a.polygons {
            if let Some(path) = crate::map::overlays::ring_path(vp, ring, true) {
                pm.fill_path(&path, &paint(fill), FillRule::Winding, Transform::identity(), None);
                pm.stroke_path(&path, &paint(if emergency { Color::from_rgba8(200, 40, 200, 255) } else { Color::WHITE }), &stroke(if emergency { 5.0 } else { 2.5 }), Transform::identity(), None);
            }
        }
    }
    // Gauges in flood, circled by category.
    for g in data.rivers.iter() {
        let status = if g.forecast_status.is_empty() { g.status.as_str() } else { g.forecast_status.as_str() };
        let (r, rgb) = match status {
            "minor" => (14.0, [255, 150, 40]),
            "moderate" => (18.0, [230, 40, 40]),
            "major" => (22.0, [200, 40, 200]),
            _ => continue,
        };
        let (x, y) = vp.to_screen(g.lat, g.lon);
        if !on_screen(vp, x, y, 30.0) {
            continue;
        }
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, r);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(rgb[0], rgb[1], rgb[2], 90)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(rgb[0], rgb[1], rgb[2], 255)), &stroke(3.0), Transform::identity(), None);
        }
    }
    // Flooded / flood-prone roads.
    let label = style(&brand.font_body, Weight::Bold, 18.0, Color::WHITE);
    for e in &data.roads {
        let text = format!("{} {} {}", e.kind, e.label, e.headline).to_lowercase();
        let watery = text.contains("flood") || text.contains("water") || text.contains("washout") || text.contains("high water");
        let inside = floods.iter().any(|a| a.contains(e.lat, e.lon));
        if !watery && !inside {
            continue;
        }
        let (x, y) = vp.to_screen(e.lat, e.lon);
        if !on_screen(vp, x, y, 20.0) {
            continue;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(x, y - 11.0);
        pb.line_to(x + 11.0, y);
        pb.line_to(x, y + 11.0);
        pb.line_to(x - 11.0, y);
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(if watery { Color::from_rgba8(20, 120, 255, 255) } else { Color::from_rgba8(255, 190, 40, 255) }), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), Transform::identity(), None);
        }
        if vp.view.zoom >= 8.0 && !e.route.is_empty() {
            halo_text(pm, glyphs, &if watery { format!("{} flooded", e.route) } else { format!("{} (in warning)", e.route) }, x + 15.0, y + 6.0, &label);
        }
    }
}

/// Traffic congestion: slowdowns (amber), crashes / incidents (orange) and closures (red), with
/// the route named when zoomed in.
pub fn draw_traffic(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, roads: &[crate::data::RoadEvent], brand: &Brand) {
    let label = style(&brand.font_body, Weight::Bold, 18.0, Color::WHITE);
    for e in roads {
        let (rgb, r, text) = match e.kind.as_str() {
            "closure" => ([220, 40, 40], 13.0, "Closed"),
            "incident" | "obstruction" => ([240, 130, 30], 11.0, "Crash / incident"),
            "delay" | "traffic-condition" => ([240, 200, 40], 10.0, "Slow traffic"),
            _ => continue,
        };
        let (x, y) = vp.to_screen(e.lat, e.lon);
        if !on_screen(vp, x, y, 20.0) {
            continue;
        }
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, r * 1.8);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(rgb[0], rgb[1], rgb[2], 70)), FillRule::Winding, Transform::identity(), None);
        }
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, r);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(rgb[0], rgb[1], rgb[2], 255)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), Transform::identity(), None);
        }
        if vp.view.zoom >= 8.5 {
            let t = if e.route.is_empty() { text.to_string() } else { format!("{} {}", e.route, text.to_lowercase()) };
            halo_text(pm, glyphs, &t, x + r + 6.0, y + 6.0, &label);
        }
    }
}

/// Camera locations as small rounded squares with a lens, named.
pub fn draw_cameras(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, cams: &[crate::travel::WebCam], brand: &Brand) {
    let label = style(&brand.font_body, Weight::Bold, 18.0, Color::WHITE);
    for c in cams.iter().filter(|c| c.lat != 0.0 || c.lon != 0.0) {
        let (x, y) = vp.to_screen(c.lat, c.lon);
        if !on_screen(vp, x, y, 20.0) {
            continue;
        }
        if let Some(rect) = Rect::from_xywh(x - 12.0, y - 9.0, 24.0, 18.0) {
            pm.fill_rect(rect, &paint(Color::from_rgba8(30, 34, 40, 235)), Transform::identity(), None);
        }
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, 5.5);
        if let Some(path) = pb.finish() {
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), Transform::identity(), None);
        }
        halo_text(pm, glyphs, &c.name, x + 16.0, y + 6.0, &label);
    }
}

/// Airport station plots: a dot colored by flight category, temperature (red) above and dew
/// point (green) below on the left, and a wind line toward where the wind comes from.
pub fn draw_metar_plots(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, metars: &[crate::data::Metar], brand: &Brand) {
    // Sized to read at TV distance.
    let temp_style = style(&brand.font_body, Weight::Bold, 24.0, Color::from_rgba8(255, 120, 110, 255));
    let dew_style = style(&brand.font_body, Weight::Bold, 21.0, Color::from_rgba8(120, 230, 140, 255));
    let wind_style = style(&brand.font_body, Weight::Bold, 17.0, Color::WHITE);
    let mut placed: Vec<(f32, f32)> = Vec::new();
    // Main airports (four-letter K/P ids such as KEVV) win the space over small fields.
    let mut order: Vec<&crate::data::Metar> = metars.iter().collect();
    order.sort_by_key(|m| !(m.id.len() == 4 && (m.id.starts_with('K') || m.id.starts_with('P')) && m.id.chars().all(|c| c.is_ascii_alphabetic())));
    for m in order {
        let (x, y) = vp.to_screen(m.lat, m.lon);
        if !on_screen(vp, x, y, 30.0) {
            continue;
        }
        // Keep plots legible: about one per 110 × 80 px, none on top of another.
        if placed.iter().any(|(px, py)| (px - x).abs() < 110.0 && (py - y).abs() < 80.0) {
            continue;
        }
        placed.push((x, y));
        let dot = match m.flight_category.as_str() {
            "VFR" => Color::from_rgba8(60, 200, 90, 255),
            "MVFR" => Color::from_rgba8(70, 130, 240, 255),
            "IFR" => Color::from_rgba8(230, 60, 60, 255),
            "LIFR" => Color::from_rgba8(210, 70, 220, 255),
            _ => Color::from_rgba8(200, 200, 200, 255),
        };
        if let (Some(dir), Some(kt)) = (m.wind_dir, m.wind_kt.filter(|kt| *kt >= 1.0)) {
            let rad = dir.to_radians();
            let len = 14.0 + kt.min(40.0) * 0.9;
            let mut pb = PathBuilder::new();
            pb.move_to(x, y);
            pb.line_to(x + rad.sin() * len, y - rad.cos() * len);
            if let Some(path) = pb.finish() {
                pm.stroke_path(&path, &paint(Color::from_rgba8(10, 14, 20, 220)), &stroke(5.0), Transform::identity(), None);
                pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.5), Transform::identity(), None);
            }
            let text = match m.gust_kt {
                Some(g) if g > kt => format!("{kt:.0}G{g:.0}"),
                _ => format!("{kt:.0}"),
            };
            halo_text(pm, glyphs, &text, x + 12.0, y + 22.0, &wind_style);
        }
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, 6.0);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(dot), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(10, 14, 20, 255)), &stroke(1.5), Transform::identity(), None);
        }
        if let Some(t) = m.temp_f {
            let text = format!("{t:.0}");
            halo_text(pm, glyphs, &text, x - 10.0 - measure(&text, &temp_style), y - 4.0, &temp_style);
        }
        if let Some(d) = m.dew_f {
            let text = format!("{d:.0}");
            halo_text(pm, glyphs, &text, x - 10.0 - measure(&text, &dew_style), y + 20.0, &dew_style);
        }
    }
}

/// Storm tracks: path over the last hour, id and trend, a ring when lightning jumps.
pub fn draw_tracks(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, tracks: &[crate::cells::Track], brand: &Brand) {
    let label = style(&brand.font_body, Weight::Bold, 18.0, Color::WHITE);
    let now = chrono::Utc::now().timestamp();
    for tr in tracks.iter().filter(|t| !t.ended) {
        let pts: Vec<(f32, f32)> = tr.points.iter().filter(|p| now - p.time <= 3600).map(|p| vp.to_screen(p.lat, p.lon)).collect();
        let Some(&(x, y)) = pts.last() else { continue };
        if !on_screen(vp, x, y, 40.0) {
            continue;
        }
        if pts.len() >= 2 {
            let mut pb = PathBuilder::new();
            pb.move_to(pts[0].0, pts[0].1);
            for p in &pts[1..] {
                pb.line_to(p.0, p.1);
            }
            if let Some(path) = pb.finish() {
                pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 150)), &stroke(5.0), Transform::identity(), None);
                pm.stroke_path(&path, &paint(Color::from_rgba8(255, 255, 255, 220)), &stroke(2.0), Transform::identity(), None);
            }
            for p in &pts[..pts.len() - 1] {
                let mut d = PathBuilder::new();
                d.push_circle(p.0, p.1, 3.0);
                if let Some(path) = d.finish() {
                    pm.fill_path(&path, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
                }
            }
        }
        let color = match tr.trend_score {
            s if s >= 2 => Color::from_rgba8(230, 40, 40, 255),
            1 => Color::from_rgba8(240, 150, 30, 255),
            s if s <= -1 => Color::from_rgba8(80, 160, 240, 255),
            _ => Color::from_rgba8(200, 200, 200, 255),
        };
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, 8.0);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), Transform::identity(), None);
        }
        if tr.jump {
            let mut pb = PathBuilder::new();
            pb.push_circle(x, y, 18.0);
            if let Some(path) = pb.finish() {
                pm.stroke_path(&path, &paint(Color::from_rgba8(255, 230, 60, 255)), &stroke(3.0), Transform::identity(), None);
            }
        }
        let arrow = match tr.trend_score {
            s if s > 0 => " ▲",
            s if s < 0 => " ▼",
            _ => "",
        };
        halo_text(pm, glyphs, &format!("{}{arrow}{}", tr.id, if tr.jump { " ⚡" } else { "" }), x + 14.0, y - 8.0, &label);
    }
}

/// Lightning clusters: a ring sized by strike count, with a motion arrow and speed.
pub fn draw_clusters(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, clusters: &[crate::cells::Cluster], brand: &Brand) {
    let label = style(&brand.font_body, Weight::Bold, 18.0, Color::WHITE);
    for c in clusters {
        let (x, y) = vp.to_screen(c.lat, c.lon);
        if !on_screen(vp, x, y, 60.0) {
            continue;
        }
        let r = (10.0 + (c.strikes as f32).sqrt() * 3.0).min(60.0);
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, r);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(255, 220, 60, 50)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(255, 220, 60, 230)), &stroke(2.5), Transform::identity(), None);
        }
        if let Some((mph, heading)) = c.motion {
            let a = (heading as f32).to_radians();
            let (dx, dy) = (a.sin(), -a.cos());
            let len = r + 30.0;
            let mut pb = PathBuilder::new();
            pb.move_to(x, y);
            pb.line_to(x + dx * len, y + dy * len);
            pb.move_to(x + dx * len, y + dy * len);
            pb.line_to(x + dx * (len - 10.0) - dy * 6.0, y + dy * (len - 10.0) + dx * 6.0);
            pb.move_to(x + dx * len, y + dy * len);
            pb.line_to(x + dx * (len - 10.0) + dy * 6.0, y + dy * (len - 10.0) - dx * 6.0);
            if let Some(path) = pb.finish() {
                pm.stroke_path(&path, &paint(Color::from_rgba8(255, 220, 60, 255)), &stroke(3.0), Transform::identity(), None);
            }
            halo_text(pm, glyphs, &format!("{} strikes • {mph:.0} mph", c.strikes), x + r + 6.0, y + r, &label);
        } else {
            halo_text(pm, glyphs, &format!("{} strikes", c.strikes), x + r + 6.0, y + r, &label);
        }
    }
}

/// Lightning density stepping through the last hour, 10 minutes at a time (6-second loop).
pub fn draw_lightning_animation(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, strikes: &[Strike], brand: &Brand) {
    let step = ((crate::map::overlays::anim_clock() % 6.0) as i64).min(5);
    let now = chrono::Utc::now().timestamp_millis();
    // Oldest slice first: step 0 = 60-50 min ago … step 5 = the last 10 minutes.
    let (from, to) = (now - (6 - step) * 600_000, now - (5 - step) * 600_000);
    let slice: Vec<Strike> = strikes.iter().filter(|s| s.time_ms >= from && s.time_ms < to).cloned().collect();
    // Reuse the density squares with the slice as "the last 10 minutes".
    let shifted: Vec<Strike> = slice.into_iter().map(|mut s| { s.time_ms = now - 1; s }).collect();
    draw_lightning_density(pm, vp, &shifted, 10);
    let st = style(&brand.font_body, Weight::Bold, 22.0, Color::WHITE);
    let text = if step == 5 { "Lightning: last 10 min".to_string() } else { format!("Lightning: {}-{} min ago", (6 - step) * 10, (5 - step) * 10) };
    halo_text(pm, glyphs, &text, vp.width as f32 * 0.05, vp.height as f32 * 0.12, &st);
}

/// MCS / squall line outline with its length.
pub fn draw_mcs(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, systems: &[crate::cells::Mcs], brand: &Brand) {
    let label = style(&brand.font_display, Weight::Bold, 26.0, Color::WHITE);
    for m in systems {
        let pts: Vec<(f32, f32)> = m.hull.iter().map(|(lat, lon)| vp.to_screen(*lat, *lon)).collect();
        if pts.len() < 3 {
            continue;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        for p in &pts[1..] {
            pb.line_to(p.0, p.1);
        }
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(170, 80, 220, 40)), FillRule::Winding, Transform::identity(), None);
            let mut st = stroke(4.0);
            st.dash = tiny_skia::StrokeDash::new(vec![18.0, 10.0], 0.0);
            pm.stroke_path(&path, &paint(Color::from_rgba8(200, 120, 255, 255)), &st, Transform::identity(), None);
        }
        let top = pts.iter().fold(pts[0], |a, b| if b.1 < a.1 { *b } else { a });
        halo_text(pm, glyphs, &format!("{} • {:.0} mi long", if m.squall_line { "Squall line" } else { "Storm complex (MCS)" }, m.length_mi), top.0, top.1 - 14.0, &label);
    }
}

/// Our own town labels. Visibility rules: the smallest town shown depends on the zoom; bigger
/// towns go first and a label never overlaps one already placed. Size scales with zoom and
/// population. Overrides can nudge, hide or force a label.
pub fn draw_town_labels(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, scene: &crate::map::scene::MapScene, brand: &Brand) {
    let z = vp.view.zoom;
    let min_pop: u32 = match z {
        z if z < 5.5 => 500_000,
        z if z < 6.5 => 150_000,
        z if z < 7.5 => 40_000,
        z if z < 8.5 => 12_000,
        z if z < 9.5 => 4_000,
        z if z < 10.5 => 1_000,
        _ => 0,
    };
    let (s, w, n, e) = vp.bounds();
    let mut towns = crate::places::in_box(s, n, w, e);
    towns.sort_by(|a, b| b.population.cmp(&a.population));
    let theme = crate::maptheme::get(&scene.theme);
    let (text_color, halo, weight, caps, base) = match (theme.as_ref(), scene.label_style.as_str()) {
        (Some(t), _) => (crate::maptheme::color(&t.label_color, Color::WHITE), true, Weight::Bold, t.label_caps, t.label_size.clamp(10.0, 48.0)),
        (None, "bold") => (Color::WHITE, true, Weight::Black, true, 26.0),
        (None, "minimal") => (Color::from_rgba8(225, 228, 232, 255), true, Weight::SemiBold, false, 19.0),
        (None, "dark") => (Color::from_rgba8(25, 28, 34, 255), false, Weight::Bold, false, 22.0),
        _ => (Color::WHITE, true, Weight::Bold, false, 22.0),
    };
    let theme_font = theme.as_ref().map(|t| t.label_font.clone()).filter(|f| !f.trim().is_empty());
    let zoom_scale = (0.8 + (z as f32 - 6.0) * 0.08).clamp(0.75, 1.3) * scene.label_scale.clamp(0.6, 1.8);
    let mut placed: Vec<Rect> = Vec::new();
    for t in towns.iter().take(400) {
        let over = scene.label_overrides.iter().find(|o| o.name.eq_ignore_ascii_case(&t.name));
        if over.map(|o| o.hidden).unwrap_or(false) {
            continue;
        }
        let always = over.map(|o| o.always).unwrap_or(false);
        if t.population < min_pop && !always {
            continue;
        }
        let big = if t.population >= 100_000 { 1.2 } else if t.population >= 25_000 { 1.08 } else { 1.0 };
        let size = base * zoom_scale * big;
        let st = style(theme_font.as_deref().unwrap_or(if caps { &brand.font_display } else { &brand.font_body }), weight, size, text_color);
        let name = if caps { t.name.to_uppercase() } else { t.name.clone() };
        let (x, y) = vp.to_screen(t.lat, t.lon);
        let (dx, dy) = over.map(|o| (o.dx, o.dy)).unwrap_or((0.0, 0.0));
        let tw = measure(&name, &st);
        let (lx, ly) = (x + 9.0 + dx, y + size * 0.35 + dy);
        let Some(rect) = Rect::from_xywh(lx - 4.0, ly - size, tw + 8.0, size * 1.3) else { continue };
        if !always && placed.iter().any(|r| r.intersect(&rect).is_some()) {
            continue;
        }
        if !on_screen(vp, x, y, 0.0) {
            continue;
        }
        placed.push(rect);
        if let Some(th) = &theme {
            crate::maptheme::draw_symbol(pm, &th.town_symbol, x, y, if t.population >= 100_000 { 1.3 } else { 1.0 });
        } else {
            let mut pb = PathBuilder::new();
            pb.push_circle(x, y, if t.population >= 100_000 { 5.0 } else { 3.5 });
            if let Some(path) = pb.finish() {
                pm.fill_path(&path, &paint(text_color), FillRule::Winding, Transform::identity(), None);
                pm.stroke_path(&path, &paint(if halo { Color::from_rgba8(0, 0, 0, 200) } else { Color::WHITE }), &stroke(1.5), Transform::identity(), None);
            }
        }
        if scene.label_style == "dark" {
            // Dark text on a light basemap: a white halo instead of black.
            let white = style(&brand.font_body, weight, size, Color::from_rgba8(255, 255, 255, 230));
            for (ox, oy) in [(-1.5, 0.0), (1.5, 0.0), (0.0, -1.5), (0.0, 1.5)] {
                crate::gfx::text::draw_line(pm, glyphs, &name, lx + ox, ly + oy, &white, 1.0, None);
            }
            crate::gfx::text::draw_line(pm, glyphs, &name, lx, ly, &st, 1.0, None);
        } else {
            halo_text(pm, glyphs, &name, lx, ly, &st);
        }
    }
}

/// Heaviest snow band each hour (snowfall-weighted centre of the strongest cells), joined into a
/// track with hour labels.
pub fn draw_snow_band(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, grid: &crate::grid::WxGrid, brand: &Brand) {
    let label = style(&brand.font_body, Weight::Bold, 18.0, Color::WHITE);
    let mut track: Vec<(f32, f32, String)> = Vec::new();
    for hour in 0..12 {
        let tf = grid.time_index(hour as f32);
        let (mut sx, mut sy, mut sw, mut best) = (0.0f64, 0.0f64, 0.0f64, 0.0f32);
        for j in 0..grid.ny {
            for i in 0..grid.nx {
                let rate = grid.node(crate::grid::Var::Snow, tf, j, i);
                best = best.max(rate);
                if rate >= 0.04 {
                    let (lat, lon) = grid.node_geo(j, i);
                    sx += lon * rate as f64;
                    sy += lat * rate as f64;
                    sw += rate as f64;
                }
            }
        }
        if sw > 0.0 && best >= 0.06 {
            let (x, y) = vp.to_screen(sy / sw, sx / sw);
            let t = chrono::DateTime::from_timestamp(grid.time_at(tf), 0).map(|d| d.with_timezone(&chrono::Local).format("%-I %p").to_string()).unwrap_or_default();
            track.push((x, y, t));
        }
    }
    if track.len() < 2 {
        return;
    }
    let mut pb = PathBuilder::new();
    pb.move_to(track[0].0, track[0].1);
    for p in &track[1..] {
        pb.line_to(p.0, p.1);
    }
    if let Some(path) = pb.finish() {
        pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 150)), &stroke(8.0), Transform::identity(), None);
        pm.stroke_path(&path, &paint(Color::from_rgba8(150, 200, 255, 255)), &stroke(4.0), Transform::identity(), None);
    }
    for (k, (x, y, t)) in track.iter().enumerate() {
        let mut d = PathBuilder::new();
        d.push_circle(*x, *y, 6.0);
        if let Some(path) = d.finish() {
            pm.fill_path(&path, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
        }
        if k % 2 == 0 {
            halo_text(pm, glyphs, &format!("Heaviest snow {t}"), x + 10.0, y - 6.0, &label);
        }
    }
}

/// Ensemble value at each town as a coloured dot and label (mean, spread, percentile or chance).
pub fn draw_ensemble_towns(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, mode: &str, brand: &Brand) {
    let Some(e) = crate::ensemble::latest() else { return };
    let label = style(&brand.font_display, Weight::Bold, 24.0, Color::WHITE);
    let title = match mode {
        "mean_high" => "Ensemble average high tomorrow",
        "spread_high" => "Uncertainty in tomorrow's high (± spread)",
        "p10_snow" => "Low-end snow (10th percentile), 72 h",
        "p90_snow" => "High-end snow (90th percentile), 72 h",
        "prob_snow1" => "Chance of 1\" or more of snow, 72 h",
        "prob_rain1" => "Chance of 1\" or more of rain, 72 h",
        "prob_freeze" => "Chance of a freeze, next 48 h",
        _ => "",
    };
    halo_text(pm, glyphs, title, vp.width as f32 * 0.05, vp.height as f32 * 0.1, &label);
    for t in &e.towns {
        let Some((v, text)) = crate::ensemble::town_value(t, mode) else { continue };
        let (x, y) = vp.to_screen(t.lat, t.lon);
        if !on_screen(vp, x, y, 30.0) || !v.is_finite() {
            continue;
        }
        let pct = mode.starts_with("prob");
        let c = if pct {
            let a = (v / 100.0).clamp(0.0, 1.0);
            Color::from_rgba8((60.0 + 180.0 * a) as u8, (140.0 - 60.0 * a) as u8, (220.0 - 140.0 * a) as u8, 235)
        } else {
            Color::from_rgba8(60, 120, 200, 235)
        };
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, 20.0);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(c), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), Transform::identity(), None);
        }
        let tw = measure(&text, &label);
        halo_text(pm, glyphs, &text, x - tw / 2.0, y + 8.0, &label);
        let small = style(&brand.font_body, Weight::SemiBold, 16.0, Color::WHITE);
        halo_text(pm, glyphs, &t.name, x + 24.0, y - 12.0, &small);
    }
}

fn shape_path(vp: &Viewport, ring: &[(f64, f64)]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for (i, (lon, lat)) in ring.iter().enumerate() {
        let (x, y) = vp.to_screen(*lat, *lon);
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    pb.close();
    pb.finish()
}

/// Drought (D0-D4), fire outlook (elevated / critical / extreme) and wildfire perimeters.
///
/// The categories nest (D1 inside D0, and so on), so each is filled solid on a separate layer,
/// worst last, and that layer is blended over the map once: the official category colours
/// instead of stacked translucent fills turning muddy. Each category is one even-odd path over
/// all its rings, so holes (non-drought pockets) stay open.
pub fn draw_hazard_shapes(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, shapes: &[crate::hazards::Shape], kind: &str, brand: &Brand) {
    let label = style(&brand.font_body, Weight::Bold, 18.0, Color::WHITE);
    let colour = |s: &crate::hazards::Shape| -> (u8, u8, u8) {
        match kind {
            // US Drought Monitor categories in the TWC palette (LibreWXR palette 4) at 30, 36,
            // 42, 56 and 70 dBZ: the official yellow-to-dark-red order, in the house colours.
            "drought" => drought_rgb(s.value.max(0.0) as u8),
            // CPC drought outlook classes in CPC's map colours.
            "drought_outlook" => drought_trend_rgb(s.value as u8),
            "fire" => match s.value as i32 {
                v if v >= 8 => (255, 0, 255),
                v if v >= 5 => (255, 0, 0),
                _ => (255, 165, 0),
            },
            _ => (255, 90, 30),
        }
    };
    let opacity = match kind {
        "drought" => 0.85,
        "drought_outlook" => 0.8,
        "fire" => 0.55,
        _ => 0.7,
    };
    // The national drought map is thousands of points: drawing it every frame held Program at
    // about 13 fps. The finished layer is kept and reused until the view, size or data change.
    type Cached = (String, usize, usize, [u64; 3], (u32, u32), Pixmap, Vec<(String, f32, f32)>);
    static CACHE: std::sync::Mutex<Vec<Cached>> = std::sync::Mutex::new(Vec::new());
    let view_key = [vp.view.lat.to_bits(), vp.view.lon.to_bits(), vp.view.zoom.to_bits()];
    let size = (pm.width(), pm.height());
    if let Ok(cache) = CACHE.lock() {
        if let Some(c) = cache.iter().find(|c| c.0 == kind && c.1 == shapes.as_ptr() as usize && c.2 == shapes.len() && c.3 == view_key && c.4 == size) {
            pm.draw_pixmap(0, 0, c.5.as_ref(), &tiny_skia::PixmapPaint { opacity, ..Default::default() }, Transform::identity(), None);
            for (text, x, y) in &c.6 {
                if on_screen(vp, *x, *y, 0.0) {
                    halo_text(pm, glyphs, text, x + 10.0, *y, &label);
                }
            }
            return;
        }
    }
    let Some(mut layer) = Pixmap::new(pm.width(), pm.height()) else { return };
    let mut order: Vec<&crate::hazards::Shape> = shapes.iter().collect();
    order.sort_by(|a, b| a.value.partial_cmp(&b.value).unwrap_or(std::cmp::Ordering::Equal));
    let mut labels = Vec::new();
    for s in order {
        let Some(path) = rings_path(vp, &s.rings) else { continue };
        let (r, g, b) = colour(s);
        layer.fill_path(&path, &paint(Color::from_rgba8(r, g, b, 255)), FillRule::EvenOdd, Transform::identity(), None);
        // A thin darker edge separates touching categories.
        layer.stroke_path(&path, &paint(Color::from_rgba8((r as f32 * 0.6) as u8, (g as f32 * 0.6) as u8, (b as f32 * 0.6) as u8, 255)), &stroke(1.2), Transform::identity(), None);
        if kind == "wildfire" && vp.view.zoom >= 7.0 {
            let bounds = path.bounds();
            labels.push((s.label.clone(), (bounds.left() + bounds.right()) / 2.0, (bounds.top() + bounds.bottom()) / 2.0));
        }
    }
    pm.draw_pixmap(0, 0, layer.as_ref(), &tiny_skia::PixmapPaint { opacity, ..Default::default() }, Transform::identity(), None);
    for (text, x, y) in &labels {
        if on_screen(vp, *x, *y, 0.0) {
            halo_text(pm, glyphs, text, x + 10.0, *y, &label);
        }
    }
    if let Ok(mut cache) = CACHE.lock() {
        cache.retain(|c| c.0 != kind);
        cache.push((kind.to_owned(), shapes.as_ptr() as usize, shapes.len(), view_key, size, layer, labels));
    }
}

/// Earthquakes: circles sized by magnitude and coloured by age (USGS convention), oldest and
/// smallest first so the newest, largest sit on top. Quakes from the past hour get a halo ring;
/// M4.5+ and the newest quake are labelled.
pub fn draw_quakes(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, quakes: &[crate::quakes::Quake], days: u32, min_mag: f32, brand: &Brand) {
    let now = chrono::Utc::now().timestamp();
    let mut shown = crate::quakes::select(quakes, now, days.max(1), min_mag, None);
    if shown.is_empty() {
        return;
    }
    let newest = shown.iter().max_by_key(|q| q.time).map(|q| q.id.clone());
    shown.sort_by(|a, b| b.age_secs(now).cmp(&a.age_secs(now)).then(a.mag.total_cmp(&b.mag)));
    // Bigger markers when zoomed in; never smaller than readable.
    let k = (0.75 + (vp.view.zoom as f32 - 4.0) * 0.12).clamp(0.7, 1.5);
    let label = style(&brand.font_body, Weight::Bold, 19.0, Color::WHITE);
    let mut labels = Vec::new();
    for q in shown {
        let (x, y) = vp.to_screen(q.lat, q.lon);
        let r = crate::quakes::radius(q.mag) * k;
        if !on_screen(vp, x, y, r + 4.0) {
            continue;
        }
        let (cr, cg, cb) = crate::quakes::age_rgb(q.age_secs(now));
        if let Some(path) = PathBuilder::from_circle(x, y, r) {
            pm.fill_path(&path, &paint(Color::from_rgba8(cr, cg, cb, 200)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(20, 20, 20, 230)), &stroke(1.6), Transform::identity(), None);
        }
        if q.age_secs(now) < 3600 {
            if let Some(path) = PathBuilder::from_circle(x, y, r + 7.0 * k) {
                pm.stroke_path(&path, &paint(Color::from_rgba8(cr, cg, cb, 230)), &stroke(3.0), Transform::identity(), None);
            }
        }
        if q.mag >= 4.5 || newest.as_deref() == Some(q.id.as_str()) {
            labels.push((format!("M{:.1}", q.mag), x + r + 6.0, y + 6.0));
        }
    }
    for (text, x, y) in labels {
        halo_text(pm, glyphs, &text, x, y, &label);
    }
}

/// CPC drought outlook colours: persists (brown), improves (tan), removal likely (green),
/// development likely (yellow), worsens (dark brown).
pub fn drought_trend_rgb(code: u8) -> (u8, u8, u8) {
    match code {
        1 => (155, 118, 83),
        2 => (222, 209, 166),
        3 => (95, 170, 85),
        4 => (255, 222, 99),
        _ => (110, 70, 40),
    }
}

/// US Drought Monitor category colours (D0-D4) as drawn on the map.
pub fn drought_rgb(category: u8) -> (u8, u8, u8) {
    match category {
        0 => (255, 238, 7),
        1 => (247, 186, 8),
        2 => (242, 137, 8),
        3 => (222, 54, 10),
        _ => (184, 12, 8),
    }
}

/// One path over many rings (lon, lat), in screen pixels: points closer than about a pixel to
/// the last kept one are skipped and rings wholly off screen are dropped (the drought map has
/// about 140,000 points; only what can show is drawn).
fn rings_path(vp: &Viewport, rings: &[Vec<(f64, f64)>]) -> Option<tiny_skia::Path> {
    let (w, h) = (vp.width as f32, vp.height as f32);
    let mut pb = PathBuilder::new();
    let mut points = Vec::new();
    for ring in rings {
        points.clear();
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (lon, lat) in ring {
            let (x, y) = vp.to_screen(*lat, *lon);
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            if points.last().is_none_or(|&(px, py): &(f32, f32)| (x - px).abs() + (y - py).abs() >= 0.8) {
                points.push((x, y));
            }
        }
        if points.len() < 3 || max_x < 0.0 || max_y < 0.0 || min_x > w || min_y > h || (max_x - min_x < 0.5 && max_y - min_y < 0.5) {
            continue;
        }
        pb.move_to(points[0].0, points[0].1);
        for &(x, y) in &points[1..] {
            pb.line_to(x, y);
        }
        pb.close();
    }
    pb.finish()
}

/// NHC cone (white), forecast track and points with category, watches / warnings, and the
/// chosen extra layer: wind probability, earliest arrival, storm surge or model spaghetti.
pub fn draw_tropical(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, t: &crate::hazards::Tropical, layer: &str, brand: &Brand) {
    let label = style(&brand.font_display, Weight::Bold, 20.0, Color::WHITE);
    let extra: &[crate::hazards::Shape] = match layer {
        "wind_prob" => &t.wind_prob,
        "arrival" => &t.arrival,
        "surge" => &t.surge,
        _ => &[],
    };
    for s in extra {
        let c = match layer {
            "wind_prob" => {
                let a = (s.value / 100.0).clamp(0.05, 1.0);
                Color::from_rgba8((60.0 + 190.0 * a) as u8, (180.0 - 120.0 * a) as u8, (220.0 - 180.0 * a) as u8, 120)
            }
            "surge" => Color::from_rgba8(40, 120, 230, 140),
            _ => Color::from_rgba8(240, 200, 60, 70),
        };
        for ring in &s.rings {
            if let Some(path) = shape_path(vp, ring) {
                pm.fill_path(&path, &paint(c), FillRule::EvenOdd, Transform::identity(), None);
                pm.stroke_path(&path, &paint(Color::from_rgba8(255, 255, 255, 120)), &stroke(1.0), Transform::identity(), None);
            }
        }
    }
    if layer == "models" {
        let colours = [(255, 80, 80), (80, 200, 255), (255, 210, 60), (120, 230, 120), (220, 120, 255), (255, 150, 60), (200, 200, 200), (60, 140, 255), (255, 255, 255), (160, 255, 220)];
        for (k, (name, pts)) in t.models.iter().enumerate() {
            let (r, g, b) = colours[k % colours.len()];
            let screen: Vec<(f32, f32)> = pts.iter().map(|(la, lo)| vp.to_screen(*la, *lo)).collect();
            if screen.len() < 2 {
                continue;
            }
            let mut pb = PathBuilder::new();
            pb.move_to(screen[0].0, screen[0].1);
            for p in &screen[1..] {
                pb.line_to(p.0, p.1);
            }
            if let Some(path) = pb.finish() {
                pm.stroke_path(&path, &paint(Color::from_rgba8(r, g, b, 230)), &stroke(3.0), Transform::identity(), None);
            }
            if let Some(last) = screen.last() {
                let st = style(&brand.font_body, Weight::Bold, 15.0, Color::from_rgba8(r, g, b, 255));
                halo_text(pm, glyphs, name, last.0 + 6.0, last.1, &st);
            }
        }
    }
    for s in &t.cone {
        for ring in &s.rings {
            if let Some(path) = shape_path(vp, ring) {
                pm.fill_path(&path, &paint(Color::from_rgba8(255, 255, 255, 50)), FillRule::EvenOdd, Transform::identity(), None);
                pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.5), Transform::identity(), None);
            }
        }
    }
    for s in &t.watches {
        let c = if s.label.to_uppercase().contains("HW") || s.label.to_lowercase().contains("hurricane warning") { (230, 30, 30) } else if s.label.to_uppercase().starts_with('H') { (255, 140, 200) } else if s.label.to_uppercase().contains("TW") { (40, 80, 230) } else { (250, 230, 60) };
        for line in s.lines.iter().chain(s.rings.iter()) {
            let pts: Vec<(f32, f32)> = line.iter().map(|(lo, la)| vp.to_screen(*la, *lo)).collect();
            if pts.len() >= 2 {
                let mut pb = PathBuilder::new();
                pb.move_to(pts[0].0, pts[0].1);
                for p in &pts[1..] {
                    pb.line_to(p.0, p.1);
                }
                if let Some(path) = pb.finish() {
                    pm.stroke_path(&path, &paint(Color::from_rgba8(c.0, c.1, c.2, 255)), &stroke(7.0), Transform::identity(), None);
                }
            }
        }
    }
    for s in &t.track {
        for line in &s.lines {
            let pts: Vec<(f32, f32)> = line.iter().map(|(lo, la)| vp.to_screen(*la, *lo)).collect();
            if pts.len() >= 2 {
                let mut pb = PathBuilder::new();
                pb.move_to(pts[0].0, pts[0].1);
                for p in &pts[1..] {
                    pb.line_to(p.0, p.1);
                }
                if let Some(path) = pb.finish() {
                    pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 160)), &stroke(6.0), Transform::identity(), None);
                    pm.stroke_path(&path, &paint(Color::WHITE), &stroke(3.0), Transform::identity(), None);
                }
            }
        }
    }
    for p in &t.points {
        let (x, y) = vp.to_screen(p.lat, p.lon);
        if !on_screen(vp, x, y, 30.0) {
            continue;
        }
        let c = match p.wind_mph {
            w if w >= 157.0 => (200, 0, 200),
            w if w >= 130.0 => (220, 30, 30),
            w if w >= 111.0 => (240, 100, 30),
            w if w >= 96.0 => (250, 170, 40),
            w if w >= 74.0 => (250, 230, 60),
            w if w >= 39.0 => (60, 170, 255),
            _ => (150, 150, 150),
        };
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, 12.0);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(c.0, c.1, c.2, 255)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), Transform::identity(), None);
        }
        halo_text(pm, glyphs, &format!("{} {}", p.category, p.label), x + 16.0, y + 6.0, &label);
    }
}

/// SPC tornado paths since a year, coloured by rating (EF0 green … EF5 magenta).
pub fn draw_tornado_history(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, since: i32, brand: &Brand) {
    let (s, w, n, e) = vp.bounds();
    let list = crate::history::tornadoes(s, n, w, e);
    let mut count = 0;
    for t in list.iter().filter(|t| t.year >= since) {
        count += 1;
        let (x0, y0) = vp.to_screen(t.start.0, t.start.1);
        let (x1, y1) = vp.to_screen(t.end.0, t.end.1);
        let c = match t.ef {
            5 => (220, 40, 220),
            4 => (220, 30, 30),
            3 => (245, 120, 30),
            2 => (245, 210, 40),
            1 => (120, 200, 80),
            0 => (100, 170, 240),
            _ => (180, 180, 180),
        };
        let width = 2.0 + t.ef.max(0) as f32 * 1.5;
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y0);
        pb.line_to(x1 + 0.5, y1 + 0.5);
        if let Some(path) = pb.finish() {
            pm.stroke_path(&path, &paint(Color::from_rgba8(c.0, c.1, c.2, 220)), &stroke(width), Transform::identity(), None);
        }
    }
    let st = style(&brand.font_display, Weight::Bold, 26.0, Color::WHITE);
    let text = if list.is_empty() { "Tornado history: downloading the SPC database…".to_string() } else { format!("{count} tornadoes since {since} (SPC)") };
    halo_text(pm, glyphs, &text, vp.width as f32 * 0.05, vp.height as f32 * 0.1, &st);
}

/// Approved viewer reports and photos that came with a location.
pub fn draw_viewer_reports(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, subs: &[crate::portal::Submission], brand: &Brand) {
    let label = style(&brand.font_body, Weight::Bold, 17.0, Color::WHITE);
    for s in subs.iter().filter(|s| s.status == "approved") {
        let (Some(lat), Some(lon)) = (s.lat, s.lon) else { continue };
        let (x, y) = vp.to_screen(lat, lon);
        if !on_screen(vp, x, y, 20.0) {
            continue;
        }
        let c = match s.kind.as_str() {
            "photo" | "video" => (90, 180, 255),
            _ => (255, 190, 60),
        };
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, 9.0);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(c.0, c.1, c.2, 255)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), Transform::identity(), None);
        }
        if vp.view.zoom >= 8.0 {
            let t = if s.report_type.is_empty() { s.place.clone() } else { format!("{}: {}", s.report_type, s.text.chars().take(30).collect::<String>()) };
            halo_text(pm, glyphs, &t, x + 13.0, y + 6.0, &label);
        }
    }
}

/// Urban drainage vulnerability: towns of 10,000+ as circles sized by population, red where the
/// next 3 hours bring an inch of rain or more (pavement floods first).
pub fn draw_urban_flood(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, grid: &crate::grid::WxGrid, brand: &Brand) {
    let (s, w, n, e) = vp.bounds();
    let label = style(&brand.font_body, Weight::Bold, 17.0, Color::WHITE);
    let tf = grid.time_index(3.0);
    for t in crate::places::in_box(s, n, w, e).into_iter().filter(|t| t.population >= 10_000) {
        let (x, y) = vp.to_screen(t.lat, t.lon);
        let rain = grid.value(crate::map::scene::DataField::RainTotal, tf, t.lat, t.lon).unwrap_or(0.0);
        let r = (6.0 + (t.population as f32).sqrt() / 18.0).min(40.0);
        let c = if rain >= 1.0 { Color::from_rgba8(230, 50, 50, 150) } else if rain >= 0.5 { Color::from_rgba8(240, 160, 40, 130) } else { Color::from_rgba8(120, 140, 170, 90) };
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, r);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(c), FillRule::Winding, Transform::identity(), None);
        }
        if rain >= 0.5 {
            halo_text(pm, glyphs, &format!("{} {rain:.1}\"", t.name), x + r + 4.0, y + 5.0, &label);
        }
    }
}

#[cfg(test)]
mod drought_qa {
    use super::*;

    /// Live drought feed through the app's own fetch, drawn over Indiana and timed
    /// (CWX_QA_NEW_DIR set; downloads about 15 MB).
    #[test]
    fn qa_drought_live() {
        let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) else { return };
        let started = std::time::Instant::now();
        let shapes = crate::net::runtime().block_on(crate::hazards::drought()).expect("drought fetch");
        let fetched = started.elapsed();
        let points: usize = shapes.iter().map(|s| s.rings.iter().map(Vec::len).sum::<usize>()).sum();
        let rings: usize = shapes.iter().map(|s| s.rings.len()).sum();
        let mut pm = Pixmap::new(1920, 1080).unwrap();
        pm.fill(Color::from_rgba8(30, 32, 36, 255));
        let vp = crate::map::geo::Viewport::new(crate::config::GeoView { lat: 39.8, lon: -86.2, zoom: 6.3 }, 1920, 1080);
        let mut glyphs = GlyphCache::default();
        let brand = crate::config::Settings::default().brand;
        let t = std::time::Instant::now();
        draw_hazard_shapes(&mut pm, &mut glyphs, &vp, &shapes, "drought", &brand);
        let drawn = t.elapsed();
        eprintln!("shapes {} rings {rings} points {points} fetch {fetched:?} draw {drawn:?} values {:?}", shapes.len(), shapes.iter().map(|s| s.value).collect::<Vec<_>>());
        pm.save_png(dir.join("drought-indiana.png")).unwrap();
    }
}
