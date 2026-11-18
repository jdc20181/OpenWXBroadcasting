//! County risk maps inside graphics (a "Risk map" layer): the counties around home, each filled
//! with the SPC outlook (overall risk or a hazard's chance) or the WPC flood outlook at its
//! centre, with the largest towns labelled. Drawn from the cached Census county outlines, so it
//! needs no map tiles and renders the same in previews, exports and on air.
//!
//! A layer's spec is `<hazard>[:<day>]`: hazard = categorical, tornado, wind, hail or flood;
//! day 1-3 (default 1), e.g. `wind:2`.

use std::sync::{Arc, OnceLock, RwLock};

use tiny_skia::{Color, FillRule, Mask, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::{
    config::Settings,
    data::{SpcArea, WxSnapshot},
    gfx::text::{draw_line, line_metrics, measure, GlyphCache, TextStyle, Weight},
};

/// What the risk maps draw from: the newest outlooks and home.
struct Snapshot {
    key: (i64, usize),
    home: (f64, f64),
    /// Per day 1-3: categorical, tornado, wind, hail, flood.
    layers: Vec<[Vec<SpcArea>; 5]>,
    /// Moving storm cells (lat, lon, heading, km/h) for the timing map, and when they were seen.
    cells: Vec<(f64, f64, f64, f64)>,
    cells_at: i64,
}

fn store() -> &'static RwLock<Option<Arc<Snapshot>>> {
    static S: OnceLock<RwLock<Option<Arc<Snapshot>>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

const HAZARDS: [&str; 5] = ["categorical", "tornado", "wind", "hail", "flood"];

/// Keep the outlooks for risk map layers (called whenever graphics bindings are built; copies
/// only when the data changed).
pub fn remember(data: &WxSnapshot, settings: &Settings) {
    let key = (data.updated_at.map(|t| t.timestamp()).unwrap_or(0) ^ (settings.home.lat * 1e4) as i64, data.spc.len() * 1000 + data.cells.len());
    if store().read().ok().and_then(|s| s.as_ref().map(|s| s.key)) == Some(key) {
        return;
    }
    let layers = (0..3)
        .map(|i| {
            let day = data.spc.iter().find(|d| d.day as usize == i + 1);
            let get = |k: &str| day.map(|d| d.areas(k).to_vec()).unwrap_or_default();
            [get("categorical"), get("tornado"), get("wind"), get("hail"), data.ero[i].clone()]
        })
        .collect();
    if let Ok(mut s) = store().write() {
        let cells = data
            .cells
            .iter()
            .filter(|c| c.intensity.level >= 2)
            .filter_map(|c| c.motion.as_ref().filter(|m| m.speed_kmh >= 8.0).map(|m| (c.lat, c.lon, m.heading_deg, m.speed_kmh)))
            .collect();
        let cells_at = data.updated_at.map(|t| t.timestamp()).unwrap_or_else(|| chrono::Utc::now().timestamp());
        *s = Some(Arc::new(Snapshot { key, home: (settings.home.lat, settings.home.lon), layers, cells, cells_at }));
    }
}

/// (hazard index, day index) from a layer spec.
fn parse_spec(spec: &str) -> (usize, usize) {
    let (hazard, day) = spec.trim().split_once(':').unwrap_or((spec.trim(), "1"));
    let h = HAZARDS.iter().position(|k| k.eq_ignore_ascii_case(hazard.trim())).unwrap_or(0);
    let d = day.trim().parse::<usize>().unwrap_or(1).clamp(1, 3) - 1;
    (h, d)
}

fn inside(ring: &[(f64, f64)], lon: f64, lat: f64) -> bool {
    let mut c = false;
    let mut j = ring.len().saturating_sub(1);
    for i in 0..ring.len() {
        let (xi, yi) = ring[i];
        let (xj, yj) = ring[j];
        if (yi > lat) != (yj > lat) && lon < (xj - xi) * (lat - yi) / (yj - yi) + xi {
            c = !c;
        }
        j = i;
    }
    c
}

fn parse_hex(s: &str) -> Option<Color> {
    let h = s.trim().trim_start_matches('#');
    if h.len() < 6 {
        return None;
    }
    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some(Color::from_rgba8(v(0)?, v(2)?, v(4)?, 255))
}

/// The colour for a point: the highest-ranked (drawn last) area containing it.
fn color_at(areas: &[SpcArea], lon: f64, lat: f64) -> Option<Color> {
    areas
        .iter()
        .filter(|a| !a.label.to_uppercase().contains("SIG"))
        .filter(|a| a.rings.iter().any(|r| inside(r, lon, lat)))
        .max_by_key(|a| a.rank)
        .and_then(|a| parse_hex(&a.fill))
}

struct Region {
    counties: Vec<(Vec<Vec<(f64, f64)>>, (f64, f64))>,
    /// (west, south, east, north) shown; counties crossing it are cut at its edge.
    bbox: (f64, f64, f64, f64),
    /// Towns by importance: the home county's first, then the largest, kept apart.
    towns: Vec<(String, f64, f64)>,
    /// Index into `counties` of the county containing home.
    home_county: Option<usize>,
    /// The state the map is limited to (whole-state maps): draws no state lines.
    whole_state: bool,
}

/// Counties whose centre is within ~70 miles of home, or every county of one state (state FIPS
/// in `state`); cached.
fn region(home: (f64, f64), state: Option<&str>) -> Option<Arc<Region>> {
    static CACHE: OnceLock<RwLock<Option<((i64, i64), Arc<Region>)>>> = OnceLock::new();
    static STATE_CACHE: OnceLock<RwLock<Option<(String, Arc<Region>)>>> = OnceLock::new();
    if let Some(st) = state {
        let slot = STATE_CACHE.get_or_init(Default::default);
        if let Some((k, r)) = slot.read().ok()?.as_ref() {
            if k == st {
                return Some(r.clone());
            }
        }
        let r = build_region(|c| c.state == st, None, None)?;
        if let Ok(mut w) = slot.write() {
            *w = Some((st.to_string(), r.clone()));
        }
        return Some(r);
    }
    let slot = CACHE.get_or_init(Default::default);
    let key = ((home.0 * 100.0) as i64, (home.1 * 100.0) as i64);
    if let Some((k, r)) = slot.read().ok()?.as_ref() {
        if *k == key {
            return Some(r.clone());
        }
    }
    // A rectangle around home (about 175 × 175 miles, 16:9 friendly): whole counties are drawn
    // and cut at its edge, instead of a ragged circle of whole counties (CWXB-75).
    let (dlat, dlon) = (1.25, 1.25 / home.0.to_radians().cos() * 1.25);
    let rect = (home.1 - dlon, home.0 - dlat, home.1 + dlon, home.0 + dlat);
    let r = build_region(
        |c| {
            let b = c.rings.iter().flatten().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, (lon, lat)| (b.0.min(*lon), b.1.min(*lat), b.2.max(*lon), b.3.max(*lat)));
            b.0 < rect.2 && b.2 > rect.0 && b.1 < rect.3 && b.3 > rect.1
        },
        Some(rect),
        Some(home),
    )?;
    if let Ok(mut w) = slot.write() {
        *w = Some((key, r.clone()));
    }
    Some(r)
}

fn centre_of(rings: &[Vec<(f64, f64)>]) -> (f64, f64) {
    let n = rings.iter().map(|r| r.len()).sum::<usize>().max(1) as f64;
    let (sx, sy) = rings.iter().flatten().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
    (sx / n, sy / n)
}

/// `rect`: show exactly this box (counties cut at its edge); `None` fits the counties kept.
/// `home`: (lat, lon) to mark the home county and put its towns first.
fn build_region(keep: impl Fn(&crate::map::boundaries::County) -> bool, rect: Option<(f64, f64, f64, f64)>, home: Option<(f64, f64)>) -> Option<Arc<Region>> {
    let all = crate::map::boundaries::counties_named()?;
    let mut counties = Vec::new();
    let mut bbox = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut home_county = None;
    for c in all.iter() {
        if !keep(c) {
            continue;
        }
        let centre = centre_of(&c.rings);
        for (lon, lat) in c.rings.iter().flatten() {
            bbox = (bbox.0.min(*lon), bbox.1.min(*lat), bbox.2.max(*lon), bbox.3.max(*lat));
        }
        if let Some((hlat, hlon)) = home {
            if home_county.is_none() && c.rings.iter().any(|r| inside(r, hlon, hlat)) {
                home_county = Some(counties.len());
            }
        }
        counties.push((c.rings.clone(), centre));
    }
    if counties.is_empty() {
        return None;
    }
    let bbox = rect.unwrap_or(bbox);
    let mut towns: Vec<_> = crate::places::in_box(bbox.1 + 0.12, bbox.3 - 0.12, bbox.0 + 0.18, bbox.2 - 0.18);
    // Real places have populations; the built-in list (before the full one downloads) has none,
    // and filtering on it left the map with no town names at all.
    if towns.iter().any(|p| p.population > 0) {
        towns.retain(|p| p.population > 0);
    }
    // The home county's towns first (Jasper, Huntingburg…), then the largest.
    let in_home = |lon: f64, lat: f64| home_county.is_some_and(|i| counties[i].0.iter().any(|r: &Vec<(f64, f64)>| inside(r, lon, lat)));
    towns.sort_by(|a, b| in_home(b.lon, b.lat).cmp(&in_home(a.lon, a.lat)).then(b.population.cmp(&a.population)));
    // Keep towns apart so their labels don't collide; up to 8 (the draw picks how many fit).
    let mut picked: Vec<(String, f64, f64)> = Vec::new();
    // At most two from the home county (close together is fine there); the rest spread out.
    let mut from_home = 0;
    for t in towns {
        if picked.len() >= 8 {
            break;
        }
        let home_town = in_home(t.lon, t.lat);
        if home_town && from_home >= 2 {
            continue;
        }
        let gap = if home_town { 20.0 } else { 55.0 };
        if picked.iter().all(|p| crate::map::geo::haversine_km(p.2, p.1, t.lat, t.lon) > gap) {
            from_home += home_town as usize;
            picked.push((t.name.clone(), t.lon, t.lat));
        }
    }
    Some(Arc::new(Region { counties, bbox, towns: picked, home_county, whole_state: rect.is_none() }))
}

/// Indiana's Winter Storm Warning snow amounts (inches in 12 h): 6 across the northern third,
/// 4 in far south and southeast Indiana, 5 in between (NWS Indianapolis). The bands follow
/// county centres, so border counties are approximate.
pub fn wsw_inches(lon: f64, lat: f64) -> u8 {
    if lat >= 40.45 {
        6
    } else if lat < 38.75 || (lat < 39.35 && lon > -85.75) {
        4
    } else {
        5
    }
}

/// Hours until the first moving storm passes within ~12 miles of a point (None: none in 3 h).
fn arrival_hours(cells: &[(f64, f64, f64, f64)], lon: f64, lat: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for &(clat, clon, heading, kmh) in cells {
        let d = crate::map::geo::haversine_km(clat, clon, lat, lon);
        let bearing = crate::map::geo::bearing_deg(clat, clon, lat, lon);
        let off = (bearing - heading).to_radians();
        let along = d * off.cos();
        let cross = (d * off.sin()).abs();
        if along < -10.0 || cross > 20.0 {
            continue;
        }
        let hours = along.max(0.0) / kmh;
        if hours <= 3.0 {
            best = Some(best.map_or(hours, |b: f64| b.min(hours)));
        }
    }
    best
}

/// Storm timing bands: within 1 h, 1-2 h, 2-3 h.
pub const TIMING_COLORS: [&str; 3] = ["#E8473B", "#F7961E", "#F2D21E"];

pub const WSW_COLORS: [(u8, &str); 3] = [(6, "#3D7DD8"), (5, "#7FB2F0"), (4, "#C9E0FA")];

fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

/// Draw the risk map for `spec` into a `w` x `h` layer.
#[allow(clippy::too_many_arguments)]
pub fn draw(pm: &mut Pixmap, glyphs: &mut GlyphCache, local: Transform, mask: Option<&Mask>, w: f32, h: f32, spec: &str, opacity: f32, font: &str) {
    let alpha = |c: Color, a: f32| {
        let mut c = c;
        c.set_alpha((c.alpha() * a * opacity).clamp(0.0, 1.0));
        c
    };
    // Catalog sets (gfx::sets) draw their own maps.
    if spec.trim().starts_with("set:") && crate::gfx::sets::draw_map(pm, glyphs, local, mask, w, h, spec.trim(), opacity, font) {
        return;
    }
    if crate::gfx::history_set::draw_map(pm, glyphs, local, mask, w, h, spec, opacity, font) || crate::gfx::explain_set::draw_map(pm, glyphs, local, mask, w, h, spec, opacity, font) {
        return;
    }
    if let Some(id) = spec.trim().strip_prefix("winter:") {
        crate::gfx::winter_map::draw(pm, glyphs, local, mask, w, h, id, opacity, font);
        return;
    }
    let snap = store().read().ok().and_then(|s| s.clone());
    let home = snap.as_ref().map(|s| s.home).unwrap_or((38.39, -86.93));
    let wsw = spec.trim().starts_with("wsw");
    let timing = spec.trim().starts_with("timing");
    let Some(region) = region(home, wsw.then_some("18")) else {
        // Counties not downloaded yet (first run): a neutral placeholder.
        if let Some(rect) = Rect::from_xywh(0.0, 0.0, w, h) {
            pm.fill_rect(rect, &paint(alpha(Color::from_rgba8(60, 66, 78, 255), 1.0)), local, mask);
        }
        return;
    };
    let (h_i, d_i) = parse_spec(spec);
    let areas: &[SpcArea] = snap.as_ref().and_then(|s| s.layers.get(d_i)).map(|l| l[h_i].as_slice()).unwrap_or(&[]);

    // Fit the region's box into the layer, keeping its shape (longitude shrinks with latitude).
    let (west, south, east, north) = region.bbox;
    let k = ((south + north) / 2.0).to_radians().cos();
    let (gw, gh) = ((east - west) * k, north - south);
    let pad = 0.04;
    let scale = ((w as f64 * (1.0 - 2.0 * pad)) / gw).min((h as f64 * (1.0 - 2.0 * pad)) / gh);
    let ox = (w as f64 - gw * scale) / 2.0;
    let oy = (h as f64 - gh * scale) / 2.0;
    let to = |lon: f64, lat: f64| -> (f32, f32) { ((ox + (lon - west) * k * scale) as f32, (oy + (north - lat) * scale) as f32) };

    let none = Color::from_rgba8(138, 143, 152, 255);
    let line = Color::from_rgba8(24, 28, 36, 255);
    let line_w = (w / 520.0).clamp(1.0, 3.0);
    // Counties crossing the box are cut at its edge: a clean rectangle, not a ragged cut-out.
    let clip = Rect::from_xywh(ox as f32, oy as f32, (gw * scale) as f32, (gh * scale) as f32)
        .and_then(|r| {
            let mut m = match mask {
                Some(m) => m.clone(),
                None => {
                    let mut m = Mask::new(pm.width(), pm.height())?;
                    m.fill_path(&PathBuilder::from_rect(Rect::from_xywh(0.0, 0.0, pm.width() as f32, pm.height() as f32)?), FillRule::Winding, false, Transform::identity());
                    m
                }
            };
            m.intersect_path(&PathBuilder::from_rect(r), FillRule::Winding, true, local);
            Some(m)
        });
    let mask = if region.whole_state { mask } else { clip.as_ref().or(mask) };
    let mut shaded = false;
    for (rings, (clon, clat)) in &region.counties {
        let mut pb = PathBuilder::new();
        for ring in rings {
            let mut first = true;
            for &(lon, lat) in ring {
                let (x, y) = to(lon, lat);
                if first {
                    pb.move_to(x, y);
                    first = false;
                } else {
                    pb.line_to(x, y);
                }
            }
            pb.close();
        }
        let Some(path) = pb.finish() else { continue };
        let fill = if timing {
            let elapsed = snap.as_ref().map(|s| (chrono::Utc::now().timestamp() - s.cells_at).max(0) as f64 / 3600.0).unwrap_or(0.0);
            snap.as_ref()
                .and_then(|s| arrival_hours(&s.cells, *clon, *clat))
                .map(|h| (h - elapsed).max(0.0))
                .and_then(|h| TIMING_COLORS.get((h.floor() as usize).min(2)))
                .and_then(|c| parse_hex(c))
                .unwrap_or(none)
        } else if wsw {
            let inches = wsw_inches(*clon, *clat);
            WSW_COLORS.iter().find(|(i, _)| *i == inches).and_then(|(_, c)| parse_hex(c)).unwrap_or(none)
        } else {
            color_at(areas, *clon, *clat).unwrap_or(none)
        };
        shaded |= fill != none;
        pm.fill_path(&path, &paint(alpha(fill, 1.0)), FillRule::EvenOdd, local, mask);
        let mut s = Stroke::default();
        s.width = line_w;
        pm.stroke_path(&path, &paint(alpha(line, 0.9)), &s, local, mask);
    }

    let path_of = |rings: &[Vec<(f64, f64)>]| {
        let mut pb = PathBuilder::new();
        for ring in rings {
            for (i, &(lon, lat)) in ring.iter().enumerate() {
                let (x, y) = to(lon, lat);
                if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
            }
            pb.close();
        }
        pb.finish()
    };
    // State lines, thicker than county lines, so Indiana and Kentucky read apart.
    if !region.whole_state {
        let rings = crate::map::boundaries::state_rings();
        let near: Vec<Vec<(f64, f64)>> = rings
            .into_iter()
            .filter(|r| r.iter().any(|(lon, lat)| *lon > west - 1.0 && *lon < east + 1.0 && *lat > south - 1.0 && *lat < north + 1.0))
            .collect();
        if let Some(path) = path_of(&near) {
            let s = Stroke { width: line_w * 2.6, line_join: tiny_skia::LineJoin::Round, ..Stroke::default() };
            pm.stroke_path(&path, &paint(alpha(Color::from_rgba8(10, 12, 16, 255), 1.0)), &s, local, mask);
        }
    }
    // Home county: a bright outline so viewers find themselves.
    if let Some(path) = region.home_county.and_then(|i| path_of(&region.counties[i].0)) {
        let s = Stroke { width: line_w * 3.2, line_join: tiny_skia::LineJoin::Round, ..Stroke::default() };
        pm.stroke_path(&path, &paint(alpha(Color::from_rgba8(10, 12, 16, 255), 0.9)), &Stroke { width: line_w * 5.0, ..s.clone() }, local, mask);
        pm.stroke_path(&path, &paint(alpha(Color::WHITE, 1.0)), &s, local, mask);
    }
    // Nothing to shade on a severe map: say so on the map instead of a bare grey region.
    if !shaded && !timing && !wsw {
        let size = (h * 0.07).clamp(14.0, 44.0) * local.sy.abs().max(0.01);
        let st = TextStyle { family: font.to_string(), weight: Weight::Bold, size, color: Color::WHITE, letter_spacing: 0.0 };
        // The flooding-rain map (WPC) isn't about storms.
        let text = if HAZARDS.get(h_i) == Some(&"flood") { "No flooding rain expected" } else { "No severe storms expected" };
        let tw = measure(text, &st);
        let (cx, cy) = (local.tx + w / 2.0 * local.sx, local.ty + h * 0.14 * local.sy);
        if let Some(path) = crate::gfx::render::rounded_rect(cx - tw / 2.0 - size * 0.6, cy - size * 0.95, tw + size * 1.2, size * 1.5, size * 0.3) {
            pm.fill_path(&path, &paint(alpha(Color::from_rgba8(8, 10, 14, 225), 1.0)), FillRule::Winding, Transform::identity(), None);
        }
        draw_line(pm, glyphs, text, cx - tw / 2.0, cy + size * 0.15, &st, opacity, None);
    }

    // Town tags (black, like broadcast county maps): more on bigger maps.
    let size = (h * 0.055).clamp(12.0, 30.0) * local.sy.abs().max(0.01);
    let st = TextStyle { family: font.to_string(), weight: Weight::Bold, size, color: Color::WHITE, letter_spacing: 0.0 };
    let (ascent, descent) = line_metrics(&st);
    let count = ((w / 160.0) as usize).clamp(3, 8);
    let mut boxes: Vec<(f32, f32, f32, f32)> = Vec::new();
    for (name, lon, lat) in region.towns.iter().take(count) {
        let (x, y) = to(*lon, *lat);
        let text = name.to_uppercase();
        let tw = measure(&text, &st);
        let (px, py) = (local.tx + x * local.sx, local.ty + y * local.sy);
        let (bw, bh) = (tw + size * 0.6, ascent + descent + size * 0.35);
        // Skip a tag that would overlap one already placed.
        let b = (px - bw / 2.0, py - bh / 2.0, bw, bh);
        if boxes.iter().any(|o| b.0 < o.0 + o.2 + 4.0 && o.0 < b.0 + b.2 + 4.0 && b.1 < o.1 + o.3 + 4.0 && o.1 < b.1 + b.3 + 4.0) {
            continue;
        }
        boxes.push(b);
        if let Some(rect) = Rect::from_xywh(px - bw / 2.0, py - bh / 2.0, bw, bh) {
            pm.fill_rect(rect, &paint(alpha(Color::from_rgba8(8, 10, 14, 235), 1.0)), Transform::identity(), None);
        }
        draw_line(pm, glyphs, &text, px - tw / 2.0, py - (ascent + descent) / 2.0 + ascent, &st, opacity, None);
    }
}
