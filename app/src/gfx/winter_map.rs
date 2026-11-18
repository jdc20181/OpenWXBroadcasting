//! Winter map layer (`winter:<id>` risk-map spec): one state's model snowfall / ice / storm
//! potential as smooth contours in the map's legend colours, hand-painted counties on top, a
//! soft drop shadow and white rim around the state, light county lines, and town placards
//! ("blips") with each town's total.

use std::sync::{Arc, OnceLock, RwLock};

use tiny_skia::{Color, FillRule, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform};

use crate::{
    gfx::text::{draw_line, line_metrics, measure, GlyphCache, TextStyle, Weight},
    winter::{self, WinterMap},
};

struct County {
    fips: String,
    rings: Vec<Vec<(f64, f64)>>,
    centre: (f64, f64),
}

struct StateShape {
    counties: Vec<County>,
    bbox: (f64, f64, f64, f64),
}

/// The state's towns, biggest first: (name, lon, lat). Not cached with the outlines: the
/// national place list loads after start-up.
fn towns(map: &WinterMap, s: &StateShape) -> Vec<(String, f64, f64)> {
    let codes: Vec<&str> = winter::state_list(&map.state).iter().map(|f| winter::state_code(f)).collect();
    let (west, south, east, north) = s.bbox;
    let in_counties = |lon: f64, lat: f64| s.counties.iter().any(|c| c.rings.iter().any(|r| inside(r, lon, lat)));
    crate::places::in_box(south, north, west, east)
        .into_iter()
        .filter(|p| {
            if p.state.is_empty() || map.focus.is_some() {
                // Unknown state, or only the home area: the town must sit inside a map county.
                in_counties(p.lon, p.lat)
            } else {
                codes.contains(&p.state.as_str())
            }
        })
        .take(300)
        .map(|t| (t.name, t.lon, t.lat))
        .collect()
}

fn shape(map: &WinterMap) -> Option<Arc<StateShape>> {
    static CACHE: OnceLock<RwLock<Vec<(String, Arc<StateShape>)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = format!("{}|{:?}", map.state, map.focus);
    if let Some((_, s)) = cache.read().ok()?.iter().find(|(k, _)| *k == key) {
        return Some(s.clone());
    }
    let all = crate::map::boundaries::counties_named()?;
    let mut counties = Vec::new();
    let mut bbox = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for c in all.iter() {
        let centre = winter::county_centre(&c.rings);
        if !winter::in_region(&map.state, map.focus, &c.state, centre) {
            continue;
        }
        for (lon, lat) in c.rings.iter().flatten() {
            bbox = (bbox.0.min(*lon), bbox.1.min(*lat), bbox.2.max(*lon), bbox.3.max(*lat));
        }
        counties.push(County { fips: c.fips.clone(), rings: c.rings.clone(), centre });
    }
    if counties.is_empty() {
        return None;
    }
    let s = Arc::new(StateShape { counties, bbox });
    if let Ok(mut w) = cache.write() {
        w.push((key, s.clone()));
    }
    Some(s)
}

/// Fit of the state into a `w` x `h` box: lon/lat → box pixels, and back.
#[derive(Clone, Copy)]
struct Fit {
    west: f64,
    north: f64,
    k: f64,
    scale: f64,
    ox: f64,
    oy: f64,
}

impl Fit {
    fn new(bbox: (f64, f64, f64, f64), w: f64, h: f64) -> Self {
        let (west, south, east, north) = bbox;
        let k = ((south + north) / 2.0).to_radians().cos();
        let (gw, gh) = ((east - west) * k, north - south);
        let pad = 0.035;
        let scale = ((w * (1.0 - 2.0 * pad)) / gw).min((h * (1.0 - 2.0 * pad)) / gh);
        Self { west, north, k, scale, ox: (w - gw * scale) / 2.0, oy: (h - gh * scale) / 2.0 }
    }
    fn to(&self, lon: f64, lat: f64) -> (f32, f32) {
        ((self.ox + (lon - self.west) * self.k * self.scale) as f32, (self.oy + (self.north - lat) * self.scale) as f32)
    }
    fn from(&self, x: f64, y: f64) -> (f64, f64) {
        (self.west + (x - self.ox) / (self.k * self.scale), self.north - (y - self.oy) / self.scale)
    }
}

fn county_path(c: &County, fit: &Fit, dx: f32, dy: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    for ring in &c.rings {
        for (i, &(lon, lat)) in ring.iter().enumerate() {
            let (x, y) = fit.to(lon, lat);
            if i == 0 {
                pb.move_to(x + dx, y + dy);
            } else {
                pb.line_to(x + dx, y + dy);
            }
        }
        pb.close();
    }
    pb.finish()
}

/// Point-in-ring test (lon/lat).
pub fn inside_pub(ring: &[(f64, f64)], lon: f64, lat: f64) -> bool {
    inside(ring, lon, lat)
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

/// The county (FIPS) under a point of a winter map drawn `w` x `h` (for painting by click).
pub fn pick(map: &WinterMap, w: f64, h: f64, x: f64, y: f64) -> Option<String> {
    let s = shape(map)?;
    let (lon, lat) = Fit::new(s.bbox, w, h).from(x, y);
    s.counties.iter().find(|c| c.rings.iter().any(|r| inside(r, lon, lat))).map(|c| c.fips.clone())
}

/// The lon/lat under a point of a winter map drawn `w` x `h` (for drawing zones by click).
pub fn to_lonlat(map: &WinterMap, w: f64, h: f64, x: f64, y: f64) -> Option<(f64, f64)> {
    let s = shape(map)?;
    Some(Fit::new(s.bbox, w, h).from(x, y))
}

fn rgb(hex: &str) -> [u8; 3] {
    crate::gfx::parse_hex(hex).map(|c| {
        let c = c.to_color_u8();
        [c.red(), c.green(), c.blue()]
    }).unwrap_or([120, 132, 150])
}

fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

const NONE_RGB: [u8; 3] = [112, 124, 142];

/// Draw winter map `id` into a `w` x `h` layer.
#[allow(clippy::too_many_arguments)]
pub fn draw(pm: &mut Pixmap, glyphs: &mut GlyphCache, local: Transform, mask: Option<&Mask>, w: f32, h: f32, id: &str, opacity: f32, font: &str) {
    let Some(map) = winter::get(id.trim()) else { return };
    draw_map(pm, glyphs, local, mask, w, h, &map, opacity, font);
}

#[allow(clippy::too_many_arguments)]
pub fn draw_map(pm: &mut Pixmap, glyphs: &mut GlyphCache, local: Transform, mask: Option<&Mask>, w: f32, h: f32, map: &WinterMap, opacity: f32, font: &str) {
    let Some(s) = shape(map) else { return };
    let (dw, dh) = ((w * local.sx.abs()).round().max(8.0) as u32, (h * local.sy.abs()).round().max(8.0) as u32);
    let Some(mut off) = Pixmap::new(dw, dh) else { return };
    let fit = Fit::new(s.bbox, dw as f64, dh as f64);
    let unit = dh as f32 / 1000.0;

    // County id raster (exact, no anti-aliasing) for per-pixel values.
    let Some(mut ids) = Pixmap::new(dw, dh) else { return };
    let paths: Vec<Option<tiny_skia::Path>> = s.counties.iter().map(|c| county_path(c, &fit, 0.0, 0.0)).collect();
    for (i, path) in paths.iter().enumerate() {
        if let Some(path) = path {
            let n = i as u32 + 1;
            let mut p = Paint::default();
            p.set_color_rgba8((n & 0xFF) as u8, ((n >> 8) & 0xFF) as u8, 0, 255);
            p.anti_alias = false;
            ids.fill_path(path, &p, FillRule::EvenOdd, Transform::identity(), None);
        }
    }

    // Drop shadow and white rim (the rim's inner half is covered by the fill).
    for (off_px, alpha) in [(14.0 * unit, 0.22), (7.0 * unit, 0.45)] {
        for c in &s.counties {
            if let Some(path) = county_path(c, &fit, off_px, off_px * 1.3) {
                off.fill_path(&path, &paint(Color::from_rgba(0.0, 0.0, 0.0, alpha).unwrap_or(Color::BLACK)), FillRule::EvenOdd, Transform::identity(), None);
            }
        }
    }
    let mut rim = Stroke::default();
    rim.width = (9.0 * unit).max(3.0);
    rim.line_join = tiny_skia::LineJoin::Round;
    for path in paths.iter().flatten() {
        off.stroke_path(path, &paint(Color::from_rgba8(255, 255, 255, 235)), &rim, Transform::identity(), None);
    }

    // Values: per county centre (blocky mode) or per pixel (smooth contours).
    let centre_values: Vec<Option<f32>> = s.counties.iter().map(|c| map.value(Some(&c.fips), c.centre.0, c.centre.1)).collect();
    let value_at = |county: usize, x: f64, y: f64| -> Option<f32> {
        let c = &s.counties[county];
        if !map.smooth || map.overrides.contains_key(&c.fips) {
            return centre_values[county];
        }
        let (lon, lat) = fit.from(x, y);
        map.value(Some(&c.fips), lon, lat)
    };
    let band_rgb: Vec<[u8; 3]> = map.bands.iter().map(|b| rgb(&b.color)).collect();
    let band_index = |v: Option<f32>| -> Option<usize> {
        let v = v?;
        map.bands.iter().enumerate().filter(|(_, b)| v >= b.min).max_by(|a, b| a.1.min.total_cmp(&b.1.min)).map(|(i, _)| i)
    };
    let id_data = ids.data();
    let (wu, hu) = (dw as usize, dh as usize);
    let mut idx: Vec<i16> = vec![-2; wu * hu]; // -2 outside, -1 no band, else band
    for y in 0..hu {
        for x in 0..wu {
            let o = (y * wu + x) * 4;
            let n = id_data[o] as usize | (id_data[o + 1] as usize) << 8;
            if n == 0 || id_data[o + 3] == 0 {
                continue;
            }
            idx[y * wu + x] = band_index(value_at(n - 1, x as f64 + 0.5, y as f64 + 0.5)).map(|b| b as i16).unwrap_or(-1);
        }
    }
    let color_of = |b: i16| -> [u8; 3] { if b >= 0 { band_rgb[b as usize] } else { NONE_RGB } };
    let mut field = vec![0u8; wu * hu * 4];
    let ice_rgb = rgb(winter::ICE_HATCH);
    let hatch_period = ((22.0 * unit).round() as usize).max(6);
    let hatch_width = ((8.0 * unit).round() as usize).clamp(2, hatch_period - 2);
    for y in 0..hu {
        for x in 0..wu {
            let b = idx[y * wu + x];
            if b == -2 {
                continue;
            }
            // Anti-alias contour edges: average four sub-samples where the band changes.
            let edge = map.smooth && [(1i32, 0i32), (0, 1), (-1, 0), (0, -1)].iter().any(|(dx, dy)| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                nx >= 0 && ny >= 0 && (nx as usize) < wu && (ny as usize) < hu && { let nb = idx[ny as usize * wu + nx as usize]; nb != b && nb != -2 }
            });
            let mut c = [0f32; 3];
            if edge {
                let o = (y * wu + x) * 4;
                let n = id_data[o] as usize | (id_data[o + 1] as usize) << 8;
                for (sx, sy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let cc = color_of(band_index(value_at(n - 1, x as f64 + sx, y as f64 + sy)).map(|b| b as i16).unwrap_or(-1));
                    for k in 0..3 {
                        c[k] += cc[k] as f32 / 4.0;
                    }
                }
            } else {
                let cc = color_of(b);
                c = [cc[0] as f32, cc[1] as f32, cc[2] as f32];
            }
            // Snow-and-ice maps: diagonal glaze stripes where freezing rain adds ice.
            if map.kind.with_ice() && (x + y) % hatch_period < hatch_width {
                let (lon, lat) = fit.from(x as f64 + 0.5, y as f64 + 0.5);
                if map.ice_at(lon, lat).is_some_and(|i| i >= winter::ICE_HATCH_MIN) {
                    for k in 0..3 {
                        c[k] = c[k] * 0.15 + ice_rgb[k] as f32 * 0.85;
                    }
                }
            }
            // A gentle top-to-bottom light falloff gives the fill some depth.
            let shade = 1.04 - 0.08 * (y as f32 / hu as f32);
            let o = (y * wu + x) * 4;
            field[o] = (c[0] * shade).min(255.0) as u8;
            field[o + 1] = (c[1] * shade).min(255.0) as u8;
            field[o + 2] = (c[2] * shade).min(255.0) as u8;
            field[o + 3] = 255;
        }
    }
    if let Some(fp) = tiny_skia::IntSize::from_wh(dw, dh).and_then(|size| Pixmap::from_vec(field, size)) {
        off.draw_pixmap(0, 0, fp.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }

    // County lines.
    let mut line = Stroke::default();
    line.width = (1.4 * unit).max(0.8);
    for path in paths.iter().flatten() {
        off.stroke_path(path, &paint(Color::from_rgba8(14, 22, 38, 120)), &line, Transform::identity(), None);
    }

    // Hand-drawn zones over the fill, kept inside the map's counties; town placards keep clear
    // of their labels.
    let mut reserved = Vec::new();
    if !map.zones.is_empty() {
        let mut clip = Mask::new(dw, dh);
        if let Some(clip) = clip.as_mut() {
            for path in paths.iter().flatten() {
                clip.fill_path(path, FillRule::EvenOdd, true, Transform::identity());
            }
        }
        for zone in &map.zones {
            reserved.extend(draw_zone(&mut off, glyphs, zone, &fit, unit, font, clip.as_ref()));
        }
    }

    // Town placards.
    if map.blips {
        draw_blips(&mut off, glyphs, map, &s, &fit, &paths, unit, font, (dw, dh), reserved);
    }

    let mut pp = PixmapPaint::default();
    pp.opacity = opacity.clamp(0.0, 1.0);
    pp.quality = tiny_skia::FilterQuality::Bilinear;
    pm.draw_pixmap(local.tx.round() as i32, local.ty.round() as i32, off.as_ref(), &pp, Transform::identity(), mask);
}

/// A smooth closed curve through the zone's corners (Catmull-Rom as cubic Béziers).
fn zone_path(pts: &[(f32, f32)], closed: bool) -> Option<tiny_skia::Path> {
    let n = pts.len();
    let mut pb = PathBuilder::new();
    pb.move_to(pts.first()?.0, pts[0].1);
    if n < 3 || !closed {
        for p in &pts[1..] {
            pb.line_to(p.0, p.1);
        }
        return pb.finish();
    }
    for i in 0..n {
        let (p0, p1, p2, p3) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
        pb.cubic_to(c1.0, c1.1, c2.0, c2.1, p2.0, p2.1);
    }
    pb.close();
    pb.finish()
}

/// One hand-drawn zone: translucent fill, white dashed edge and a label tag in the middle.
/// A zone still being drawn shows its corners and the edges so far.
/// Returns the label tag's box (placards keep clear of it).
fn draw_zone(off: &mut Pixmap, glyphs: &mut GlyphCache, zone: &winter::Zone, fit: &Fit, unit: f32, font: &str, clip: Option<&Mask>) -> Option<tiny_skia::Rect> {
    let pts: Vec<(f32, f32)> = zone.points.iter().map(|(lon, lat)| fit.to(*lon, *lat)).collect();
    if pts.is_empty() {
        return None;
    }
    let [r, g, b] = rgb(&zone.color);
    let closed = !zone.open && pts.len() >= 3;
    if let Some(path) = zone_path(&pts, closed || (zone.open && pts.len() >= 3)) {
        if closed || pts.len() >= 3 {
            off.fill_path(&path, &paint(Color::from_rgba8(r, g, b, if closed { 120 } else { 70 })), FillRule::Winding, Transform::identity(), clip);
        }
        // A finished zone's edge stays inside the map; the one being drawn shows all of it.
        let edge_clip = if zone.open { None } else { clip };
        let mut st = Stroke::default();
        st.width = (4.0 * unit).max(2.0);
        st.line_join = tiny_skia::LineJoin::Round;
        st.dash = tiny_skia::StrokeDash::new(vec![16.0 * unit, 9.0 * unit], 0.0);
        off.stroke_path(&path, &paint(Color::from_rgba8(255, 255, 255, 230)), &st, Transform::identity(), edge_clip);
        let mut inner = Stroke::default();
        inner.width = (2.0 * unit).max(1.0);
        off.stroke_path(&path, &paint(Color::from_rgba8(r, g, b, 255)), &inner, Transform::identity(), edge_clip);
    }
    if zone.open {
        for (x, y) in &pts {
            let mut pb = PathBuilder::new();
            pb.push_circle(*x, *y, 7.0 * unit);
            if let Some(p) = pb.finish() {
                off.fill_path(&p, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
                let mut st = Stroke::default();
                st.width = 2.5 * unit;
                off.stroke_path(&p, &paint(Color::from_rgba8(r, g, b, 255)), &st, Transform::identity(), None);
            }
        }
    }
    let label = zone.label.trim();
    if label.is_empty() || pts.len() < 3 {
        return None;
    }
    // Label tag at the middle of the part of the shape that is on the map.
    let inside_map = |x: f32, y: f32| clip.is_none_or(|m| {
        let (xi, yi) = (x.round() as i64, y.round() as i64);
        xi >= 0 && yi >= 0 && (xi as u32) < m.width() && (yi as u32) < m.height() && m.data()[yi as usize * m.width() as usize + xi as usize] > 0
    });
    let (mut cx, mut cy) = (pts.iter().map(|p| p.0).sum::<f32>() / pts.len() as f32, pts.iter().map(|p| p.1).sum::<f32>() / pts.len() as f32);
    if !inside_map(cx, cy) {
        // Average of a coarse sample of points inside both the shape and the map.
        let (x0, x1) = pts.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
        let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
        let poly: Vec<(f64, f64)> = pts.iter().map(|p| (p.0 as f64, p.1 as f64)).collect();
        let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
        for i in 0..24 {
            for j in 0..24 {
                let (x, y) = (x0 + (x1 - x0) * (i as f32 + 0.5) / 24.0, y0 + (y1 - y0) * (j as f32 + 0.5) / 24.0);
                if inside(&poly, x as f64, y as f64) && inside_map(x, y) {
                    (sx, sy, n) = (sx + x, sy + y, n + 1.0);
                }
            }
        }
        if n == 0.0 {
            return None;
        }
        (cx, cy) = (sx / n, sy / n);
    }
    let st = TextStyle { family: font.to_string(), weight: Weight::Black, size: 30.0 * unit, color: Color::WHITE, letter_spacing: 0.5 * unit };
    let text = label.to_uppercase();
    let (a, d) = line_metrics(&st);
    let tw = measure(&text, &st);
    let (pw, ph) = (tw + 30.0 * unit, a + d + 16.0 * unit);
    if let Some(p) = crate::gfx::render::rounded_rect(cx - pw / 2.0 + 3.0 * unit, cy - ph / 2.0 + 4.0 * unit, pw, ph, ph / 2.0) {
        off.fill_path(&p, &paint(Color::from_rgba8(0, 0, 0, 90)), FillRule::Winding, Transform::identity(), None);
    }
    if let Some(p) = crate::gfx::render::rounded_rect(cx - pw / 2.0, cy - ph / 2.0, pw, ph, ph / 2.0) {
        off.fill_path(&p, &paint(Color::from_rgba8(12, 18, 32, 225)), FillRule::Winding, Transform::identity(), None);
        let mut s = Stroke::default();
        s.width = 2.5 * unit;
        off.stroke_path(&p, &paint(Color::from_rgba8(r, g, b, 255)), &s, Transform::identity(), None);
    }
    draw_line(off, glyphs, &text, cx - tw / 2.0, cy - (a + d) / 2.0 + a, &st, 1.0, None);
    tiny_skia::Rect::from_xywh(cx - pw / 2.0, cy - ph / 2.0, pw, ph)
}

#[allow(clippy::too_many_arguments)]
fn draw_blips(off: &mut Pixmap, glyphs: &mut GlyphCache, map: &WinterMap, s: &StateShape, fit: &Fit, _paths: &[Option<tiny_skia::Path>], unit: f32, font: &str, (dw, dh): (u32, u32), reserved: Vec<tiny_skia::Rect>) {
    let value_st = TextStyle { family: font.to_string(), weight: Weight::Black, size: 40.0 * unit, color: Color::WHITE, letter_spacing: 0.0 };
    let name_st = TextStyle { family: font.to_string(), weight: Weight::Bold, size: 20.0 * unit, color: Color::from_rgba8(200, 214, 234, 255), letter_spacing: 1.2 * unit };
    let (va, vd) = line_metrics(&value_st);
    let (na, nd) = line_metrics(&name_st);
    let mut placed: Vec<tiny_skia::Rect> = reserved;
    let mut dots: Vec<tiny_skia::Rect> = Vec::new();
    let mut count = 0;
    for (name, lon, lat) in &towns(map, s) {
        if count >= map.towns as usize {
            break;
        }
        let fips = s.counties.iter().find(|c| c.rings.iter().any(|r| inside(r, *lon, *lat))).map(|c| c.fips.as_str());
        // Towns outside every band get a name-only tag (like city labels on a printed map).
        let value = map.value(fips, *lon, *lat);
        let mut text = value.and_then(|v| map.blip_text(v));
        let mut band = value.and_then(|v| map.band_for(v)).map(|b| rgb(&b.color));
        // Snow-and-ice maps: a town with glaze but no snow band shows its ice amount.
        if text.is_none() {
            if let Some(ice) = map.ice_at(*lon, *lat).filter(|i| *i >= winter::ICE_HATCH_MIN) {
                text = Some(format!("{ice:.2}\" ICE"));
                band = Some(rgb(winter::ICE_HATCH));
            }
        }
        let (x, y) = fit.to(*lon, *lat);
        let label = name.to_uppercase();
        let tw = text.as_deref().map(|t| measure(t, &value_st)).unwrap_or(0.0);
        let nw = measure(&label, &name_st);
        let pad = 14.0 * unit;
        let tab = if band.is_some() { 9.0 * unit } else { 0.0 };
        let bw = tw.max(nw) + pad * 2.0 + tab;
        let bh = if text.is_some() { va + vd + na + nd + pad * 0.9 } else { na + nd + pad * 0.8 };
        let gap = 16.0 * unit;
        // Above the town, else below it; skip the town only when neither spot is clear of
        // other placards and town dots (CWXB-84: a placard covered a neighbour's label).
        let bx = x - bw / 2.0;
        let dot = tiny_skia::Rect::from_xywh(x - 8.0 * unit, y - 8.0 * unit, 16.0 * unit, 16.0 * unit);
        let clear = |by: f32| -> Option<tiny_skia::Rect> {
            let rect = tiny_skia::Rect::from_xywh(bx - 6.0 * unit, by - 6.0 * unit, bw + 12.0 * unit, bh + 12.0 * unit)?;
            let fits = bx >= 0.0 && by >= 0.0 && by + bh <= dh as f32 && bx + bw <= dw as f32;
            let hits = placed.iter().any(|r| r.intersect(&rect).is_some() || dot.is_some_and(|d| r.intersect(&d).is_some()))
                || dots.iter().any(|d: &tiny_skia::Rect| d.intersect(&rect).is_some());
            (fits && !hits).then_some(rect)
        };
        let (above, by, rect) = if let Some(r) = clear(y - bh - gap) {
            (true, y - bh - gap, r)
        } else if let Some(r) = clear(y + gap) {
            (false, y + gap, r)
        } else {
            continue;
        };
        placed.push(rect);
        if let Some(d) = dot {
            dots.push(d);
        }
        count += 1;
        let plate = if band.is_some() { Color::from_rgba8(12, 18, 32, 235) } else { Color::from_rgba8(12, 18, 32, 200) };
        // Shadow, plate, colour tab, pointer and town dot.
        if let Some(p) = crate::gfx::render::rounded_rect(bx + 3.0 * unit, by + 5.0 * unit, bw, bh, 12.0 * unit) {
            off.fill_path(&p, &paint(Color::from_rgba8(0, 0, 0, 90)), FillRule::Winding, Transform::identity(), None);
        }
        let edge = if above { by + bh - 1.0 } else { by + 1.0 };
        let mut pb = PathBuilder::new();
        pb.move_to(x - 9.0 * unit, edge);
        pb.line_to(x + 9.0 * unit, edge);
        pb.line_to(x, if above { y - 6.0 * unit } else { y + 6.0 * unit });
        pb.close();
        if let Some(p) = pb.finish() {
            off.fill_path(&p, &paint(plate), FillRule::Winding, Transform::identity(), None);
        }
        if let Some(p) = crate::gfx::render::rounded_rect(bx, by, bw, bh, 12.0 * unit) {
            off.fill_path(&p, &paint(plate), FillRule::Winding, Transform::identity(), None);
            let mut st = Stroke::default();
            st.width = 1.5 * unit;
            off.stroke_path(&p, &paint(Color::from_rgba8(255, 255, 255, 60)), &st, Transform::identity(), None);
        }
        if let (Some(band), Some(p)) = (band, crate::gfx::render::rounded_rect(bx + 5.0 * unit, by + 6.0 * unit, tab, bh - 12.0 * unit, tab / 2.0)) {
            off.fill_path(&p, &paint(Color::from_rgba8(band[0], band[1], band[2], 255)), FillRule::Winding, Transform::identity(), None);
        }
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, 5.0 * unit);
        if let Some(p) = pb.finish() {
            off.fill_path(&p, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
            let mut st = Stroke::default();
            st.width = 2.0 * unit;
            off.stroke_path(&p, &paint(Color::from_rgba8(12, 18, 32, 255)), &st, Transform::identity(), None);
        }
        let cx = bx + tab + (bw - tab) / 2.0;
        match &text {
            Some(t) => {
                draw_line(off, glyphs, t, cx - tw / 2.0, by + pad * 0.5 + va, &value_st, 1.0, None);
                draw_line(off, glyphs, &label, cx - nw / 2.0, by + pad * 0.5 + va + vd + na, &name_st, 1.0, None);
            }
            None => draw_line(off, glyphs, &label, cx - nw / 2.0, by + pad * 0.4 + na, &name_st, 1.0, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::winter::{WinterGrid, WinterKind, WinterMap};

    /// Renders sample winter maps to CWX_QA_NEW_DIR (lake-effect style: heavy north-west).
    #[test]
    fn qa_winter_maps() {
        let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) else { return };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(crate::places::ensure_loaded());
        let _ = rt.block_on(crate::map::boundaries::ensure_loaded());
        std::fs::create_dir_all(&dir).unwrap();
        let settings = crate::config::Settings::default();
        let data = crate::data::WxSnapshot::default();
        for (i, kind) in WinterKind::ALL.iter().enumerate() {
            let mut map = WinterMap::new(*kind, "18");
            map.title = match kind { WinterKind::Snow => "Final Call Snow Totals".into(), _ => map.title };
            map.subtitle = if i == 0 { "Lake-effect snow tonight into tomorrow".into() } else { String::new() };
            let (nx, ny) = (14, 14);
            let (north, south, west, east) = (41.9, 37.6, -88.2, -84.6);
            // CWX_QA_DRY=1: an ice map with no ice (empty-state note).
            let scale = if *kind == WinterKind::Ice { if std::env::var("CWX_QA_DRY").is_ok() { 0.0 } else { 0.05 } } else { 1.0 };
            let values = (0..ny).flat_map(|j| (0..nx).map(move |i| {
                let (x, y) = (i as f32 / (nx - 1) as f32, j as f32 / (ny - 1) as f32);
                let d = ((x - 0.05).powi(2) + (y - 0.0).powi(2)).sqrt();
                ((1.0 - d * 1.25).max(0.0) * 9.0 + (0.6 - y).max(0.0) * 1.5) * scale
            })).collect();
            // Snow and ice: glaze across the south third.
            let ice = if kind.with_ice() { (0..ny).flat_map(|j| (0..nx).map(move |_| if j as f32 / (ny - 1) as f32 > 0.62 { 0.12 } else { 0.0 })).collect() } else { Vec::new() };
            map.grid = Some(WinterGrid { model: crate::grid::GridModel::Gfs, hours: 48, north, south, west, east, nx, ny, values, ice, ..Default::default() });
            // Both looks for the first map, the Modern look for the others.
            for look in if i == 0 { &["modern", "classic"][..] } else { &["modern"][..] } {
                map.look = look.to_string();
                crate::winter::save(&map);
                let doc = crate::gfx::templates::winter_map(&map);
                let pm = crate::program::render_graphic_mode(&doc, &settings, &data, 10_000., 1920, 1080, None, crate::gfx::check::PreviewData::Sample);
                pm.save_png(dir.join(format!("winter-{i}-{look}.png"))).unwrap();
            }
        }
        // Hand-drawn zones, several states, and only the home area (CWXB-83 items 8 and 9).
        let mut map = WinterMap::new(WinterKind::Potential, "18,21,17");
        map.title = "Snow Potential".into();
        let (nx, ny) = (16, 16);
        let (north, south, west, east) = (42.6, 36.4, -91.6, -81.8);
        let values = (0..ny).flat_map(|j| (0..nx).map(move |i| (i as f32 / 15.0 * 2.0 + (1.0 - j as f32 / 15.0) * 6.0).max(0.0))).collect();
        map.grid = Some(WinterGrid { model: crate::grid::GridModel::Gfs, hours: 48, north, south, west, east, nx, ny, values, ..Default::default() });
        map.zones = vec![
            crate::winter::Zone { label: "Accumulation possible".into(), color: "#3F8FFF".into(), points: vec![(-88.6, 41.6), (-85.2, 41.7), (-84.9, 40.3), (-86.9, 39.6), (-88.9, 40.2)], open: false },
            crate::winter::Zone { label: "Flakes possible".into(), color: "#9FD8FF".into(), points: vec![(-89.4, 39.2), (-85.6, 39.0), (-84.4, 38.0), (-86.6, 37.3), (-89.2, 37.9)], open: false },
        ];
        for (name, focus) in [("region", None), ("home-area", Some(crate::winter::Focus { lon: -86.93, lat: 38.39, km: 161.0 }))] {
            map.focus = focus;
            crate::winter::save(&map);
            let doc = crate::gfx::templates::winter_map(&map);
            let pm = crate::program::render_graphic_mode(&doc, &settings, &data, 10_000., 1920, 1080, None, crate::gfx::check::PreviewData::Sample);
            pm.save_png(dir.join(format!("winter-{name}-zones.png"))).unwrap();
        }
    }
}
