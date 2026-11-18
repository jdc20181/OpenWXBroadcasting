//! The radar map panel: basemap tiles, radar tiles (TWC palette, fetched by Node), county
//! outlines and names, affected-county shading and warning polygons, all on one Web Mercator
//! projection so everything lines up.

use crate::gfx::{self, Align, Canvas, Rgba, Style, WHITE};
use resvg::tiny_skia::{PathBuilder, Transform};
use serde::Deserialize;

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct Tile {
    pub x: i64,
    pub y: i64,
    /// Zoom of this tile when it differs from the map's (e.g. a sharper basemap one level up).
    pub z: Option<u8>,
    /// PNG/JPEG/WebP bytes as base64 or a data: URI.
    pub data: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
pub struct County {
    pub name: String,
    pub active: bool,
    pub affected: bool,
    /// Lines only, no name (e.g. a neighbouring state's county whose name repeats a home one).
    pub unlabeled: bool,
    /// Outer and inner rings as [lon, lat] pairs.
    pub rings: Vec<Vec<[f64; 2]>>,
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct Lines {
    pub county_color: String,
    pub county_width: f32,
    pub state_color: String,
    pub state_width: f32,
    /// 0..1, applied to both (Map Studio's boundary opacity).
    pub opacity: f32,
    pub counties: bool,
    pub states: bool,
}

impl Default for Lines {
    fn default() -> Self {
        Lines { county_color: String::new(), county_width: 1.0, state_color: String::new(), state_width: 1.8, opacity: 1.0, counties: true, states: true }
    }
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct MapData {
    pub zoom: u8,
    pub tile_size: f64,
    /// Area that must be visible: [west, south, east, north]. Already padded by Node.
    pub bbox: [f64; 4],
    pub basemap: Vec<Tile>,
    pub radar: Vec<Tile>,
    pub radar_opacity: f32,
    pub counties: Vec<County>,
    /// State outline rings as [lon, lat] pairs, drawn brighter than county lines.
    pub states: Vec<Vec<[f64; 2]>>,
    /// Line colours and widths (design pixels). Missing = the card's own style. The studio
    /// passes its Map Studio theme here so cards match the maps on air.
    pub lines: Option<Lines>,
    /// Warning polygons: each is a list of rings.
    pub polygons: Vec<Vec<Vec<[f64; 2]>>>,
    /// "Regional view" tag.
    pub regional: bool,
    /// Bottom-right credit, e.g. "Radar • LibreWXR • 7:42 PM EST".
    pub badge: String,
}

impl Default for MapData {
    fn default() -> Self {
        MapData {
            zoom: 10,
            tile_size: 512.0,
            bbox: [-87.3, 38.1, -86.5, 38.7],
            basemap: vec![],
            radar: vec![],
            radar_opacity: 0.9,
            counties: vec![],
            states: vec![],
            lines: None,
            polygons: vec![],
            regional: false,
            badge: String::new(),
        }
    }
}

struct Proj {
    world: f64,
    k: f64,
    ox: f64,
    oy: f64,
}

impl Proj {
    fn gx(&self, lon: f64) -> f64 {
        (lon + 180.0) / 360.0 * self.world
    }
    fn gy(&self, lat: f64) -> f64 {
        let r = lat.to_radians();
        (1.0 - (r.tan() + 1.0 / r.cos()).ln() / std::f64::consts::PI) / 2.0 * self.world
    }
    fn pt(&self, lon: f64, lat: f64) -> (f32, f32) {
        ((self.ox + self.gx(lon) * self.k) as f32, (self.oy + self.gy(lat) * self.k) as f32)
    }
}

pub struct Style2 {
    pub kind_color: Rgba,
    pub county_line: Rgba,
    pub label_px: f32,
}

/// Draws the map into the rectangle. The bbox is fitted with a "cover" scale (no empty edges).
pub fn draw(cv: &mut Canvas, m: &MapData, x: f32, y: f32, w: f32, h: f32, radius: f32, st: &Style2) {
    cv.clip_rrect(x, y, w, h, radius);
    cv.fill_rect(x, y, w, h, Rgba::rgb(6, 10, 15));

    let world = m.tile_size * 2f64.powi(m.zoom as i32);
    let mut p = Proj { world, k: 1.0, ox: 0.0, oy: 0.0 };
    let [west, south, east, north] = m.bbox;
    let (x0, x1, y0, y1) = (p.gx(west), p.gx(east), p.gy(north), p.gy(south));
    let k = (w as f64 / (x1 - x0).max(1e-6)).max(h as f64 / (y1 - y0).max(1e-6));
    p.k = k;
    p.ox = x as f64 + w as f64 / 2.0 - (x0 + x1) / 2.0 * k;
    p.oy = y as f64 + h as f64 / 2.0 - (y0 + y1) / 2.0 * k;

    let tile_layer = |cv: &mut Canvas, tiles: &[Tile], opacity: f32| {
        for t in tiles {
            let Some(bytes) = gfx::b64(&t.data) else { continue };
            let Some(img) = gfx::decode(&bytes) else { continue };
            // Size of this tile in the map's global pixels.
            let span = world / 2f64.powi(t.z.unwrap_or(m.zoom) as i32);
            let tk = (span * k / img.width() as f64) as f32;
            let tx = (p.ox + t.x as f64 * span * k) as f32;
            let ty = (p.oy + t.y as f64 * span * k) as f32;
            // A hair of overlap hides seams between tiles.
            let tk2 = tk * (1.0 + 0.6 / img.width() as f32);
            cv.draw_pixmap(&img, Transform::from_scale(tk2, tk2).post_translate(tx, ty), opacity);
        }
    };
    tile_layer(cv, &m.basemap, 1.0);
    tile_layer(cv, &m.radar, m.radar_opacity);

    let ring_path = |rings: &[Vec<[f64; 2]>]| {
        let mut pb = PathBuilder::new();
        for ring in rings {
            for (i, c) in ring.iter().enumerate() {
                let (px, py) = p.pt(c[0], c[1]);
                if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
            }
            pb.close();
        }
        pb.finish()
    };

    // Affected counties shaded in the alert colour.
    for c in m.counties.iter().filter(|c| c.affected) {
        if let Some(path) = ring_path(&c.rings) {
            cv.fill_path(&path, st.kind_color.alpha(0.26));
        }
    }
    // Line style: the caller's (studio theme) or the card's own.
    let ls = m.lines.clone().unwrap_or_default();
    let op = ls.opacity.clamp(0.05, 1.0);
    let county_ink = gfx::css(&ls.county_color, st.county_line);
    let state_ink = gfx::css(&ls.state_color, WHITE.alpha(0.88));
    let (cw, sw) = (ls.county_width.clamp(0.2, 8.0), ls.state_width.clamp(0.3, 10.0));
    // County lines: thin, with a soft dark under-stroke so they read over radar.
    if ls.counties {
        for c in &m.counties {
            if let Some(path) = ring_path(&c.rings) {
                cv.stroke_path(&path, cw + 1.4, Rgba::rgb(0, 0, 0).alpha(0.35 * op));
                cv.stroke_path(&path, cw, county_ink.alpha(county_ink.a * op));
            }
        }
    }
    // State lines on top, brighter and a little heavier (as in the studio's maps).
    if ls.states {
        for ring in &m.states {
            if let Some(path) = ring_path(std::slice::from_ref(ring)) {
                cv.stroke_path(&path, sw + 2.2, Rgba::rgb(0, 0, 0).alpha(0.5 * op));
                cv.stroke_path(&path, sw, state_ink.alpha(state_ink.a * op));
            }
        }
    }
    // Warning polygons: wide glow, dark edge, solid line.
    for poly in &m.polygons {
        if let Some(path) = ring_path(poly) {
            cv.fill_path(&path, st.kind_color.alpha(0.12));
            cv.stroke_path(&path, 10.0, st.kind_color.alpha(0.32));
            cv.stroke_path(&path, 5.0, Rgba::rgb(0, 0, 0).alpha(0.75));
            cv.stroke_path(&path, 3.2, st.kind_color);
        }
    }
    // Home county outlined last, on top.
    for c in m.counties.iter().filter(|c| c.active) {
        if let Some(path) = ring_path(&c.rings) {
            cv.stroke_path(&path, 6.0, Rgba::rgb(0, 0, 0).alpha(0.6));
            cv.stroke_path(&path, 3.0, WHITE);
        }
    }
    // County names at the centre of each county that is on screen. Home county first, then
    // affected ones; a name that would overlap one already placed is left out.
    let mut order: Vec<&County> = m.counties.iter().collect();
    order.sort_by_key(|c| (!c.active, !c.affected));
    let mut placed: Vec<(f32, f32, f32, f32)> = vec![];
    if m.regional {
        // Keep names out from under the "Regional view" pill.
        placed.push((x, y, x + 240.0, y + 60.0));
    }
    for c in order {
        let Some(outer) = c.rings.first() else { continue };
        if outer.is_empty() || c.name.is_empty() || (c.unlabeled && !c.active && !c.affected) {
            continue;
        }
        let (mut sx, mut sy) = (0.0, 0.0);
        for pt in outer {
            sx += pt[0];
            sy += pt[1];
        }
        let n = outer.len() as f64;
        let (lx, ly) = p.pt(sx / n, sy / n);
        if lx < x + 30.0 || lx > x + w - 30.0 || ly < y + 24.0 || ly > y + h - 40.0 {
            continue;
        }
        let weight = if c.active || c.affected { 800 } else { 700 };
        let s = Style::new(st.label_px * if c.active { 1.15 } else { 1.0 }, weight, WHITE.alpha(if c.active || c.affected { 1.0 } else { 0.82 })).track(0.06);
        let tw = cv.measure(&c.name.to_uppercase(), &s);
        let rect = (lx - tw / 2.0 - 6.0, ly - s.px - 4.0, lx + tw / 2.0 + 6.0, ly + 6.0);
        if placed.iter().any(|r| rect.0 < r.2 && rect.2 > r.0 && rect.1 < r.3 && rect.3 > r.1) {
            continue;
        }
        placed.push(rect);
        cv.text_halo(lx, ly, &c.name.to_uppercase(), &s, Align::Center, Rgba::rgb(0, 0, 0).alpha(0.85), 1.6);
    }

    cv.vignette(x, y, w, h, 0.55);

    if m.regional {
        pill(cv, x + 14.0, y + 14.0, "REGIONAL VIEW", 15.0);
    }
    if !m.badge.is_empty() {
        let s = Style::new(16.0, 700, WHITE.alpha(0.9)).track(0.1);
        let tw = cv.measure(&m.badge.to_uppercase(), &s);
        let (bw, bh) = (tw + 26.0, 30.0);
        let (bx, by) = (x + w - bw - 14.0, y + h - bh - 14.0);
        cv.fill_rrect(bx, by, bw, bh, 7.0, Rgba::rgb(0, 0, 0).alpha(0.7));
        cv.stroke_rrect(bx, by, bw, bh, 7.0, 1.0, WHITE.alpha(0.16));
        let cap = cv.cap_height(&s);
        cv.text(bx + 13.0, by + (bh + cap) / 2.0, &m.badge.to_uppercase(), &s, Align::Left);
    }
    cv.unclip();
}

fn pill(cv: &mut Canvas, x: f32, y: f32, text: &str, px: f32) {
    let s = Style::new(px, 800, WHITE.alpha(0.9)).track(0.12);
    let tw = cv.measure(text, &s);
    let h = px * 1.9;
    cv.fill_rrect(x, y, tw + 24.0, h, 6.0, Rgba::rgb(0, 0, 0).alpha(0.65));
    cv.stroke_rrect(x, y, tw + 24.0, h, 6.0, 1.0, WHITE.alpha(0.18));
    let cap = cv.cap_height(&s);
    cv.text(x + 12.0, y + (h + cap) / 2.0, text, &s, Align::Left);
}
