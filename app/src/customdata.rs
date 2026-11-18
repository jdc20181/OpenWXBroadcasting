//! Custom data maps: bring your own CSV, GeoJSON or shapefile and show it as points, a heatmap,
//! a choropleth, an interpolated surface, contours, or the difference of two fields — with a
//! colour scale and legend the station builds.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};
use tiny_skia::{Color, FillRule, PathBuilder, Pixmap, Rect, Transform};

use crate::{
    config::Brand,
    gfx::text::GlyphCache,
    map::{
        geo::Viewport,
        overlays::{halo_text, paint, stroke, style},
    },
};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CustomLayer {
    pub name: String,
    pub path: String,
    /// "points", "heatmap", "choropleth", "interpolate", "contours", "compare".
    pub mode: String,
    /// Numeric field to show (and the second field for "compare": value - field2).
    pub field: String,
    pub field2: String,
    /// Field used for point labels.
    pub label_field: String,
    /// Colour scale stops: (value, "#RRGGBB"), ascending.
    pub scale: Vec<(f32, String)>,
    pub opacity: f32,
    pub legend: bool,
    pub visible: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Feature {
    pub props: HashMap<String, String>,
    /// Point (lat, lon), or rings in (lon, lat).
    pub point: Option<(f64, f64)>,
    pub rings: Vec<Vec<(f64, f64)>>,
}

#[derive(Clone, Debug, Default)]
pub struct Dataset {
    pub features: Vec<Feature>,
    pub numeric_fields: Vec<String>,
    pub fields: Vec<String>,
}

pub const PRESET_SCALES: &[(&str, &[(f32, &str)])] = &[
    ("Blue to red", &[(0.0, "#2B83BA"), (0.25, "#ABDDA4"), (0.5, "#FFFFBF"), (0.75, "#FDAE61"), (1.0, "#D7191C")]),
    ("Greens", &[(0.0, "#EDF8E9"), (0.5, "#74C476"), (1.0, "#006D2C")]),
    ("Purples", &[(0.0, "#F2F0F7"), (0.5, "#9E9AC8"), (1.0, "#54278F")]),
    ("Heat", &[(0.0, "#FFFFB2"), (0.5, "#FD8D3C"), (1.0, "#BD0026")]),
    ("Diverging", &[(0.0, "#2166AC"), (0.5, "#F7F7F7"), (1.0, "#B2182B")]),
];

fn cache() -> &'static Mutex<HashMap<PathBuf, (Option<std::time::SystemTime>, std::sync::Arc<Dataset>)>> {
    static C: OnceLock<Mutex<HashMap<PathBuf, (Option<std::time::SystemTime>, std::sync::Arc<Dataset>)>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// Load (cached by modification time) a CSV, GeoJSON or zipped / plain shapefile.
pub fn load(path: &Path) -> Option<std::sync::Arc<Dataset>> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    if let Some((m, d)) = cache().lock().ok()?.get(path) {
        if *m == modified {
            return Some(d.clone());
        }
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let features = match ext.as_str() {
        "csv" | "txt" => read_csv(path),
        "json" | "geojson" => read_geojson(path),
        "shp" => read_shapefile(path),
        "zip" => read_zipped_shapefile(path),
        _ => None,
    }?;
    let mut fields: Vec<String> = features.iter().flat_map(|f| f.props.keys().cloned()).collect();
    fields.sort();
    fields.dedup();
    let numeric_fields = fields.iter().filter(|k| features.iter().filter_map(|f| f.props.get(*k)).take(20).all(|v| v.trim().parse::<f64>().is_ok() || v.trim().is_empty())).cloned().collect();
    let d = std::sync::Arc::new(Dataset { features, numeric_fields, fields });
    cache().lock().ok()?.insert(path.to_path_buf(), (modified, d.clone()));
    Some(d)
}

fn read_csv(path: &Path) -> Option<Vec<Feature>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let split = |l: &str| -> Vec<String> {
        // Simple CSV: quotes around fields with commas.
        let mut out = Vec::new();
        let (mut cur, mut q) = (String::new(), false);
        for ch in l.chars() {
            match ch {
                '"' => q = !q,
                ',' if !q => out.push(std::mem::take(&mut cur).trim().to_string()),
                _ => cur.push(ch),
            }
        }
        out.push(cur.trim().to_string());
        out
    };
    let header: Vec<String> = split(lines.next()?);
    let find = |names: &[&str]| header.iter().position(|h| names.iter().any(|n| h.eq_ignore_ascii_case(n)));
    let (lat_i, lon_i) = (find(&["lat", "latitude", "y"])?, find(&["lon", "lng", "long", "longitude", "x"])?);
    Some(
        lines
            .filter_map(|l| {
                let cols = split(l);
                let lat = cols.get(lat_i)?.parse::<f64>().ok()?;
                let lon = cols.get(lon_i)?.parse::<f64>().ok()?;
                let props = header.iter().cloned().zip(cols.iter().cloned()).collect();
                Some(Feature { props, point: Some((lat, lon)), rings: Vec::new() })
            })
            .collect(),
    )
}

fn read_geojson(path: &Path) -> Option<Vec<Feature>> {
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let feats = v.get("features")?.as_array()?;
    Some(
        feats
            .iter()
            .filter_map(|f| {
                let props = f.get("properties").and_then(|p| p.as_object()).map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))).collect()).unwrap_or_default();
                let g = f.get("geometry")?;
                let pt = |p: &serde_json::Value| Some((p.get(0)?.as_f64()?, p.get(1)?.as_f64()?));
                let ring = |a: &serde_json::Value| a.as_array().map(|v| v.iter().filter_map(pt).collect::<Vec<_>>()).unwrap_or_default();
                let c = g.get("coordinates")?;
                let mut feature = Feature { props, point: None, rings: Vec::new() };
                match g.get("type")?.as_str()? {
                    "Point" => feature.point = pt(c).map(|(lon, lat)| (lat, lon)),
                    "Polygon" => feature.rings = c.as_array()?.iter().map(ring).collect(),
                    "MultiPolygon" => feature.rings = c.as_array()?.iter().flat_map(|p| p.as_array().cloned().unwrap_or_default()).map(|r| ring(&r)).collect(),
                    "LineString" => feature.rings = vec![ring(c)],
                    _ => return None,
                }
                Some(feature)
            })
            .collect(),
    )
}

/// Minimal ESRI shapefile reader: points and polygons from .shp, attributes from .dbf.
fn read_shapefile(path: &Path) -> Option<Vec<Feature>> {
    let shp = std::fs::read(path).ok()?;
    let dbf = std::fs::read(path.with_extension("dbf")).ok();
    read_shape_bytes(&shp, dbf.as_deref())
}

fn read_zipped_shapefile(path: &Path) -> Option<Vec<Feature>> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    if zip.len() > 1000 { return None; }
    let mut shapes = Vec::new();
    for i in 0..zip.len() { let f = zip.by_index(i).ok()?; if f.name().ok()?.to_lowercase().ends_with(".shp") { shapes.push((i, f.name().ok()?.to_string())); } }
    if shapes.len() != 1 { return None; }
    let (index, name) = &shapes[0];
    let mut shp = Vec::new();
    { let mut f = zip.by_index(*index).ok()?; if f.size() > 128 * 1024 * 1024 { return None; } f.read_to_end(&mut shp).ok()?; }
    let dbf_name = format!("{}.dbf", &name[..name.len()-4]);
    let mut dbf = None;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).ok()?;
        if f.name().ok()?.eq_ignore_ascii_case(&dbf_name) { if f.size() > 128 * 1024 * 1024 { return None; } let mut b = Vec::new(); f.read_to_end(&mut b).ok()?; dbf = Some(b); break; }
    }
    read_shape_bytes(&shp, dbf.as_deref())
}

fn read_shape_bytes(shp: &[u8], dbf: Option<&[u8]>) -> Option<Vec<Feature>> {
    if shp.len() < 100 || shp.get(..4)? != [0, 0, 0x27, 0x0a] { return None; }
    let be_i32 = |b: &[u8], o: usize| -> Option<i32> { Some(i32::from_be_bytes(b.get(o..o + 4)?.try_into().ok()?)) };
    let le_f64 = |b: &[u8], o: usize| -> Option<f64> { Some(f64::from_le_bytes(b.get(o..o + 8)?.try_into().ok()?)) };
    // Attributes.
    let mut records: Vec<HashMap<String, String>> = Vec::new();
    if let Some(d) = dbf.as_ref() {
        let n = u32::from_le_bytes(d.get(4..8)?.try_into().ok()?) as usize;
        let header_len = u16::from_le_bytes(d.get(8..10)?.try_into().ok()?) as usize;
        let rec_len = u16::from_le_bytes(d.get(10..12)?.try_into().ok()?) as usize;
        if header_len > d.len() || header_len < 33 || rec_len == 0 || n > (d.len() - header_len) / rec_len { return None; }
        let mut fields = Vec::new();
        let mut o = 32;
        while o + 32 <= header_len && d.get(o) != Some(&0x0D) {
            let name = String::from_utf8_lossy(&d[o..o + 11]).trim_end_matches('\0').to_string();
            let len = d[o + 16] as usize;
            fields.push((name, len));
            o += 32;
        }
        for r in 0..n {
            let mut p = header_len + r * rec_len + 1;
            let mut map = HashMap::new();
            for (name, len) in &fields {
                let v = d.get(p..p + len).map(|b| String::from_utf8_lossy(b).trim().to_string()).unwrap_or_default();
                map.insert(name.clone(), v);
                p += len;
            }
            records.push(map);
        }
    }
    let mut out = Vec::new();
    let mut o = 100;
    let mut idx = 0;
    while o + 8 <= shp.len() {
        let words = usize::try_from(be_i32(&shp, o + 4)?).ok()?;
        let len = words.checked_mul(2)?;
        if len < 4 || o.checked_add(8)?.checked_add(len)? > shp.len() { return None; }
        let body = o + 8;
        let kind = i32::from_le_bytes(shp.get(body..body + 4)?.try_into().ok()?);
        let props = records.get(idx).cloned().unwrap_or_default();
        match kind {
            1 => out.push(Feature { props, point: Some((le_f64(&shp, body + 12)?, le_f64(&shp, body + 4)?)), rings: Vec::new() }),
            3 | 5 => {
                let parts = i32::from_le_bytes(shp.get(body + 36..body + 40)?.try_into().ok()?) as usize;
                let points = i32::from_le_bytes(shp.get(body + 40..body + 44)?.try_into().ok()?) as usize;
                if parts > points || points > 1_000_000 || 44usize.checked_add(parts.checked_mul(4)?)?.checked_add(points.checked_mul(16)?)? > len { return None; }
                let starts: Vec<usize> = (0..parts).filter_map(|k| Some(i32::from_le_bytes(shp.get(body + 44 + k * 4..body + 48 + k * 4)?.try_into().ok()?) as usize)).collect();
                if starts.len() != parts || starts.iter().any(|s| *s > points) || starts.windows(2).any(|p| p[0] > p[1]) { return None; }
                let pts_at = body + 44 + parts * 4;
                let mut rings = Vec::new();
                for (k, s) in starts.iter().enumerate() {
                    let e = starts.get(k + 1).copied().unwrap_or(points);
                    rings.push((*s..e).filter_map(|q| Some((le_f64(&shp, pts_at + q * 16)?, le_f64(&shp, pts_at + q * 16 + 8)?))).collect());
                }
                out.push(Feature { props, point: None, rings });
            }
            _ => {}
        }
        o = body + len;
        idx += 1;
    }
    Some(out)
}

fn hex(c: &str) -> Color {
    crate::gfx::parse_hex(c).unwrap_or(Color::WHITE)
}

/// Colour for a value on the layer's scale (stops can be absolute values or 0-1 fractions of
/// the data range).
fn colour(scale: &[(f32, String)], v: f32, lo: f32, hi: f32, alpha: f32) -> Color {
    if scale.is_empty() {
        return Color::from_rgba(1.0, 1.0, 1.0, alpha).unwrap_or(Color::WHITE);
    }
    let fractional = scale.last().map(|s| s.0 <= 1.0).unwrap_or(false) && hi > 1.0;
    let x = if fractional { (v - lo) / (hi - lo).max(1e-6) } else { v };
    let mut c = hex(&scale[0].1);
    for w in scale.windows(2) {
        if x >= w[0].0 && x <= w[1].0 {
            let f = (x - w[0].0) / (w[1].0 - w[0].0).max(1e-6);
            let (a, b) = (hex(&w[0].1), hex(&w[1].1));
            c = Color::from_rgba(a.red() + (b.red() - a.red()) * f, a.green() + (b.green() - a.green()) * f, a.blue() + (b.blue() - a.blue()) * f, 1.0).unwrap_or(a);
            break;
        }
        if x > w[1].0 {
            c = hex(&w[1].1);
        }
    }
    c.set_alpha(alpha);
    c
}

fn value(f: &Feature, layer: &CustomLayer) -> Option<f32> {
    let a = f.props.get(&layer.field)?.trim().parse::<f32>().ok()?;
    if layer.mode == "compare" {
        let b = f.props.get(&layer.field2)?.trim().parse::<f32>().ok()?;
        Some(a - b)
    } else {
        Some(a)
    }
}

fn centroid(f: &Feature) -> Option<(f64, f64)> {
    f.point.or_else(|| {
        let r = f.rings.first()?;
        let n = r.len().max(1) as f64;
        Some((r.iter().map(|p| p.1).sum::<f64>() / n, r.iter().map(|p| p.0).sum::<f64>() / n))
    })
}

pub fn draw(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, layer: &CustomLayer, brand: &Brand) {
    if layer.mode == "image" {
        draw_image(pm, vp, layer);
        return;
    }
    let Some(data) = load(Path::new(&layer.path)) else { return };
    let vals: Vec<f32> = data.features.iter().filter_map(|f| value(f, layer)).collect();
    if vals.is_empty() && !matches!(layer.mode.as_str(), "points" | "choropleth") {
        return;
    }
    let lo = vals.iter().copied().fold(f32::MAX, f32::min);
    let hi = vals.iter().copied().fold(f32::MIN, f32::max);
    let alpha = if layer.opacity > 0.0 { layer.opacity } else { 0.75 };
    let small = style(&brand.font_body, crate::gfx::text::Weight::SemiBold, 16.0, Color::WHITE);
    match layer.mode.as_str() {
        "heatmap" | "interpolate" | "contours" => {
            // Points with values: kernel density (heatmap) or inverse-distance weighting.
            let pts: Vec<(f32, f32, f32)> = data
                .features
                .iter()
                .filter_map(|f| {
                    let (lat, lon) = centroid(f)?;
                    let (x, y) = vp.to_screen(lat, lon);
                    Some((x, y, value(f, layer).unwrap_or(1.0)))
                })
                .collect();
            let cell = 24.0f32;
            let (cols, rows) = ((vp.width as f32 / cell) as usize + 1, (vp.height as f32 / cell) as usize + 1);
            let mut grid = vec![f32::NAN; cols * rows];
            for r in 0..rows {
                for c in 0..cols {
                    let (x, y) = (c as f32 * cell, r as f32 * cell);
                    let mut num = 0.0;
                    let mut den = 0.0;
                    for (px, py, v) in &pts {
                        let d2 = (px - x).powi(2) + (py - y).powi(2);
                        if layer.mode == "heatmap" {
                            num += v.abs() * (-d2 / (2.0 * 60.0f32.powi(2))).exp();
                        } else {
                            let w = 1.0 / d2.max(1.0);
                            num += v * w;
                            den += w;
                        }
                    }
                    grid[r * cols + c] = if layer.mode == "heatmap" { num } else if den > 0.0 { num / den } else { f32::NAN };
                }
            }
            let (glo, ghi) = grid.iter().filter(|v| v.is_finite()).fold((f32::MAX, f32::MIN), |a, v| (a.0.min(*v), a.1.max(*v)));
            if layer.mode == "contours" {
                // Marching squares at five levels.
                for k in 1..=5 {
                    let level = glo + (ghi - glo) * k as f32 / 6.0;
                    let mut pb = PathBuilder::new();
                    for r in 0..rows - 1 {
                        for c in 0..cols - 1 {
                            let v = [grid[r * cols + c], grid[r * cols + c + 1], grid[(r + 1) * cols + c + 1], grid[(r + 1) * cols + c]];
                            if v.iter().any(|x| !x.is_finite()) {
                                continue;
                            }
                            let corners = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
                            let mut cross = Vec::new();
                            for e in 0..4 {
                                let (a, b) = (v[e], v[(e + 1) % 4]);
                                if (a < level) != (b < level) {
                                    let t = (level - a) / (b - a);
                                    let (ca, cb) = (corners[e], corners[(e + 1) % 4]);
                                    cross.push(((c as f32 + ca.0 + (cb.0 - ca.0) * t) * cell, (r as f32 + ca.1 + (cb.1 - ca.1) * t) * cell));
                                }
                            }
                            for pair in cross.chunks(2) {
                                if pair.len() == 2 {
                                    pb.move_to(pair[0].0, pair[0].1);
                                    pb.line_to(pair[1].0, pair[1].1);
                                }
                            }
                        }
                    }
                    if let Some(path) = pb.finish() {
                        pm.stroke_path(&path, &paint(colour(&layer.scale, level, glo, ghi, 1.0)), &stroke(2.5), Transform::identity(), None);
                    }
                }
            } else {
                for r in 0..rows {
                    for c in 0..cols {
                        let v = grid[r * cols + c];
                        if !v.is_finite() || (layer.mode == "heatmap" && v < ghi * 0.05) {
                            continue;
                        }
                        let a = if layer.mode == "heatmap" { alpha * (v / ghi.max(1e-6)).clamp(0.2, 1.0) } else { alpha };
                        if let Some(rect) = Rect::from_xywh(c as f32 * cell - cell / 2.0, r as f32 * cell - cell / 2.0, cell, cell) {
                            pm.fill_rect(rect, &paint(colour(&layer.scale, v, glo, ghi, a)), Transform::identity(), None);
                        }
                    }
                }
            }
            if layer.legend {
                draw_legend(pm, glyphs, layer, glo, ghi, brand);
            }
            return;
        }
        _ => {}
    }
    for f in &data.features {
        let v = value(f, layer);
        let c = v.map(|v| colour(&layer.scale, v, lo, hi, alpha)).unwrap_or(Color::from_rgba8(255, 255, 255, (alpha * 255.0) as u8));
        if !f.rings.is_empty() {
            for ring in &f.rings {
                let mut pb = PathBuilder::new();
                for (i, (lon, lat)) in ring.iter().enumerate() {
                    let (x, y) = vp.to_screen(*lat, *lon);
                    if i == 0 { pb.move_to(x, y); } else { pb.line_to(x, y); }
                }
                pb.close();
                if let Some(path) = pb.finish() {
                    pm.fill_path(&path, &paint(c), FillRule::EvenOdd, Transform::identity(), None);
                    pm.stroke_path(&path, &paint(Color::from_rgba8(255, 255, 255, 160)), &stroke(1.2), Transform::identity(), None);
                }
            }
        } else if let Some((lat, lon)) = f.point {
            let (x, y) = vp.to_screen(lat, lon);
            let r = v.map(|v| 6.0 + 12.0 * ((v - lo) / (hi - lo).max(1e-6)).clamp(0.0, 1.0)).unwrap_or(7.0);
            let mut pb = PathBuilder::new();
            pb.push_circle(x, y, r);
            if let Some(path) = pb.finish() {
                pm.fill_path(&path, &paint(c), FillRule::Winding, Transform::identity(), None);
                pm.stroke_path(&path, &paint(Color::WHITE), &stroke(1.5), Transform::identity(), None);
            }
            if let Some(l) = f.props.get(&layer.label_field).filter(|l| !l.is_empty()) {
                halo_text(pm, glyphs, l, x + r + 4.0, y + 5.0, &small);
            }
        }
    }
    if layer.legend && !vals.is_empty() {
        draw_legend(pm, glyphs, layer, lo, hi, brand);
    }
}

fn draw_legend(pm: &mut Pixmap, glyphs: &mut GlyphCache, layer: &CustomLayer, lo: f32, hi: f32, brand: &Brand) {
    let (x, y, w, h) = (pm.width() as f32 * 0.05, pm.height() as f32 * 0.84, 420.0, 26.0);
    for k in 0..60 {
        let v = lo + (hi - lo) * k as f32 / 59.0;
        if let Some(r) = Rect::from_xywh(x + w * k as f32 / 60.0, y, w / 60.0 + 1.0, h) {
            pm.fill_rect(r, &paint(colour(&layer.scale, v, lo, hi, 1.0)), Transform::identity(), None);
        }
    }
    let st = style(&brand.font_body, crate::gfx::text::Weight::Bold, 18.0, Color::WHITE);
    halo_text(pm, glyphs, &layer.name, x, y - 10.0, &st);
    halo_text(pm, glyphs, &format!("{lo:.1}"), x, y + h + 22.0, &st);
    let hs = format!("{hi:.1}");
    halo_text(pm, glyphs, &hs, x + w - crate::gfx::text::measure(&hs, &st), y + h + 22.0, &st);
}

/// Georeferenced image (lat/lon grid, e.g. a PNG with a .pgw world file) reprojected onto the
/// Web Mercator map row by row. Bounds are kept in `field` as "north,south,west,east".
fn draw_image(pm: &mut Pixmap, vp: &Viewport, layer: &CustomLayer) {
    static IMAGES: OnceLock<Mutex<HashMap<String, std::sync::Arc<image::RgbaImage>>>> = OnceLock::new();
    let cache = IMAGES.get_or_init(Default::default);
    let img = {
        let Ok(mut c) = cache.lock() else { return };
        match c.get(&layer.path) {
            Some(i) => i.clone(),
            None => {
                let Ok(i) = image::open(&layer.path) else { return };
                let i = std::sync::Arc::new(i.to_rgba8());
                c.insert(layer.path.clone(), i.clone());
                i
            }
        }
    };
    let b: Vec<f64> = layer.field.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    if b.len() != 4 {
        return;
    }
    let (n, s, w, e) = (b[0], b[1], b[2], b[3]);
    let (iw, ih) = (img.width() as f64, img.height() as f64);
    let (pw, ph) = (pm.width() as usize, pm.height() as usize);
    let alpha = if layer.opacity > 0.0 { layer.opacity } else { 0.75 };
    let step = 2usize;
    let data = pm.data_mut();
    for y in (0..ph).step_by(step) {
        let (lat, _) = vp.to_geo(0.0, y as f64);
        if lat > n || lat < s {
            continue;
        }
        // Equirectangular source: rows are linear in latitude.
        let sy = (((n - lat) / (n - s)) * ih) as u32;
        for x in (0..pw).step_by(step) {
            let (_, lon) = vp.to_geo(x as f64, y as f64);
            if lon < w || lon > e {
                continue;
            }
            let sx = (((lon - w) / (e - w)) * iw) as u32;
            let p = img.get_pixel(sx.min(img.width() - 1), sy.min(img.height() - 1));
            let a = p[3] as f32 / 255.0 * alpha;
            for dy in 0..step {
                for dx in 0..step {
                    let (xx, yy) = (x + dx, y + dy);
                    if xx >= pw || yy >= ph {
                        continue;
                    }
                    let i = (yy * pw + xx) * 4;
                    for ch in 0..3 {
                        data[i + ch] = (data[i + ch] as f32 * (1.0 - a) + p[ch] as f32 * a) as u8;
                    }
                    data[i + 3] = (data[i + 3] as f32 * (1.0 - a) + 255.0 * a) as u8;
                }
            }
        }
    }
}

/// A georeferenced image layer from a PNG / JPEG with a world file (.pgw / .jgw / .wld).
pub fn image_layer(path: &Path) -> Option<CustomLayer> {
    let img = image::image_dimensions(path).ok()?;
    let world = ["pgw", "jgw", "wld", "pngw", "jpgw"].iter().map(|e| path.with_extension(e)).find(|p| p.exists())?;
    let v: Vec<f64> = std::fs::read_to_string(world).ok()?.lines().filter_map(|l| l.trim().parse().ok()).collect();
    if v.len() < 6 {
        return None;
    }
    let (a, e_, c, f) = (v[0], v[3], v[4], v[5]);
    let (w, n) = (c, f);
    let (e, s) = (c + a * img.0 as f64, f + e_ * img.1 as f64);
    Some(CustomLayer {
        name: path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
        path: path.to_string_lossy().to_string(),
        mode: "image".into(),
        field: format!("{n},{s},{w},{e}"),
        opacity: 0.75,
        visible: true,
        ..Default::default()
    })
}

/// A new layer for a file, with sensible defaults from what the file contains.
pub fn new_layer(path: &Path) -> Option<CustomLayer> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ["png", "jpg", "jpeg"].contains(&ext.as_str()) {
        return image_layer(path);
    }
    let data = load(path)?;
    let polygons = data.features.iter().any(|f| !f.rings.is_empty());
    let field = data.numeric_fields.iter().find(|f| !["lat", "latitude", "lon", "lng", "longitude", "x", "y"].contains(&f.to_lowercase().as_str())).cloned().unwrap_or_default();
    let label_field = data.fields.iter().find(|f| ["name", "label", "title", "city", "town"].contains(&f.to_lowercase().as_str())).cloned().unwrap_or_default();
    Some(CustomLayer {
        name: path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Custom data".into()),
        path: path.to_string_lossy().to_string(),
        mode: if polygons { "choropleth".into() } else { "points".into() },
        field,
        field2: String::new(),
        label_field,
        scale: PRESET_SCALES[0].1.iter().map(|(v, c)| (*v, c.to_string())).collect(),
        opacity: 0.7,
        legend: true,
        visible: true,
    })
}

/// Average model rain (next `hours`) inside each polygon of a layer: basin averages for small
/// watersheds. Returns (name, inches), wettest first.
pub fn basin_rain(layer: &CustomLayer, grid: &crate::grid::WxGrid, hours: f32) -> Vec<(String, f32)> {
    let Some(data) = load(Path::new(&layer.path)) else { return Vec::new() };
    let tf = grid.time_index(hours);
    let mut out: Vec<(String, f32)> = data
        .features
        .iter()
        .filter(|f| !f.rings.is_empty())
        .filter_map(|f| {
            let ring = f.rings.first()?;
            let (mut s, mut n, mut w, mut e) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
            for (lon, lat) in ring {
                s = s.min(*lat);
                n = n.max(*lat);
                w = w.min(*lon);
                e = e.max(*lon);
            }
            let (mut sum, mut count) = (0.0f32, 0.0f32);
            for a in 0..8 {
                for b in 0..8 {
                    let (lat, lon) = (s + (n - s) * (a as f64 + 0.5) / 8.0, w + (e - w) * (b as f64 + 0.5) / 8.0);
                    if f.rings.iter().any(|r| crate::data::point_in_ring(r, lon, lat)) {
                        if let Some(v) = grid.value(crate::map::scene::DataField::RainTotal, tf, lat, lon) {
                            sum += v;
                            count += 1.0;
                        }
                    }
                }
            }
            let name = f.props.get(&layer.label_field).cloned().or_else(|| f.props.values().next().cloned()).unwrap_or_default();
            (count > 0.0).then(|| (name, sum / count))
        })
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[test]
    fn zipped_shapefile_loads_without_extracting_members() {
        use std::io::Write;
        let mut shp = vec![0u8; 128]; shp[..4].copy_from_slice(&9994u32.to_be_bytes());
        shp[104..108].copy_from_slice(&10u32.to_be_bytes()); shp[108..112].copy_from_slice(&1u32.to_le_bytes());
        shp[112..120].copy_from_slice(&(-87f64).to_le_bytes()); shp[120..128].copy_from_slice(&38f64.to_le_bytes());
        let path = std::env::temp_dir().join(format!("cwx-{}.zip", uuid::Uuid::new_v4()));
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        writer.start_file("nested/weather.shp", zip::write::SimpleFileOptions::default()).unwrap(); writer.write_all(&shp).unwrap(); writer.finish().unwrap();
        let result = read_zipped_shapefile(&path); let _ = std::fs::remove_file(path);
        assert_eq!(result.unwrap()[0].point, Some((38., -87.)));
        assert!(read_shape_bytes(&shp[..110], None).is_none());
    }
}
