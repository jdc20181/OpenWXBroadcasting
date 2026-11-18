//! Modern winter radar colours. LibreWXR paints snow with a flat, saturated blue ramp. With the
//! Modern HD palette (TWC colours) on, every decoded radar tile is sorted pixel by pixel into
//! rain or snow and a dBZ (the two colour tables never share a colour, so this survives lossy
//! WebP and smoothing), and snow is repainted with a soft pale-blue to deep-blue to violet ramp
//! like current TV winter radar. Rain keeps LibreWXR's own TWC colours.

use std::sync::{
    atomic::{AtomicBool, AtomicI64, Ordering},
    OnceLock,
};

use image::RgbaImage;

static ENABLED: AtomicBool = AtomicBool::new(true);
static SNOW_SEEN: AtomicI64 = AtomicI64::new(0);

/// Turn the snow repaint on or off (the radar layer names change with it, so tiles reload).
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Whether a radar tile with snow in it arrived in the last 20 minutes (shows the snow legend).
pub fn snow_recent() -> bool {
    chrono::Utc::now().timestamp() - SNOW_SEEN.load(Ordering::Relaxed) < 20 * 60
}

/// Snow ramp (dBZ, colour): pale ice blue for flurries, deepening blues for steady snow, violet
/// and pink for heavy bursts.
pub const SNOW_RAMP: [(f32, [u8; 3]); 12] = [
    (0.0, [214, 236, 255]),
    (8.0, [178, 218, 255]),
    (14.0, [132, 190, 250]),
    (19.0, [92, 158, 242]),
    (24.0, [58, 124, 228]),
    (29.0, [38, 92, 206]),
    (34.0, [30, 64, 172]),
    (39.0, [58, 44, 156]),
    (44.0, [108, 46, 170]),
    (49.0, [160, 64, 186]),
    (54.0, [214, 118, 218]),
    (60.0, [246, 204, 246]),
];

pub fn snow_color(dbz: f32) -> [u8; 3] {
    crate::grid::ramp(&SNOW_RAMP, dbz)
}

/// Quantised RGB (5 bits each) → 0 = no match, else (snow << 8 | dBZ index 0..127) + 1.
fn table() -> &'static [u16] {
    static T: OnceLock<Vec<u16>> = OnceLock::new();
    T.get_or_init(|| {
        let entries: Vec<(bool, usize, [f32; 3])> = TWC_RAIN
            .iter()
            .map(|c| (false, *c))
            .enumerate()
            .chain(TWC_SNOW.iter().map(|c| (true, *c)).enumerate())
            .filter(|(_, (_, c))| c & 0xFF > 0)
            .map(|(i, (snow, c))| (snow, i, [((c >> 24) & 0xFF) as f32, ((c >> 16) & 0xFF) as f32, ((c >> 8) & 0xFF) as f32]))
            .collect();
        let mut out = vec![0u16; 32 * 32 * 32];
        for r in 0..32u32 {
            for g in 0..32u32 {
                for b in 0..32u32 {
                    let p = [(r * 8 + 4) as f32, (g * 8 + 4) as f32, (b * 8 + 4) as f32];
                    let best = entries
                        .iter()
                        .map(|(snow, i, c)| {
                            let d = (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2);
                            (d, *snow, *i)
                        })
                        .min_by(|a, b| a.0.total_cmp(&b.0));
                    if let Some((_, snow, i)) = best {
                        out[((r << 10) | (g << 5) | b) as usize] = ((snow as u16) << 8 | i as u16) + 1;
                    }
                }
            }
        }
        out
    })
}

/// The tile filter installed into the map renderer: repaint snow on Modern HD radar layers
/// (layer names `radar-4w…`).
pub fn filter(layer_id: &str, img: &mut RgbaImage) {
    // One tile filter serves every layer: single-site reflectivity has its own clean-up.
    if layer_id.starts_with("nexrad-ref-") {
        crate::map::site_radar::filter(layer_id, img);
        return;
    }
    let Some(rest) = layer_id.strip_prefix("radar-") else { return };
    let rest = rest.strip_prefix(|c: char| c == 'z').map(|r| r.split_once('-').map(|x| x.1).unwrap_or(r)).unwrap_or(rest);
    if !rest.starts_with("4w") {
        return;
    }
    let lut = table();
    let mut snow_px = 0u32;
    for px in img.pixels_mut() {
        let a = px.0[3];
        if a == 0 {
            continue;
        }
        let key = ((px.0[0] as usize >> 3) << 10) | ((px.0[1] as usize >> 3) << 5) | (px.0[2] as usize >> 3);
        let v = lut[key];
        if v == 0 || (v - 1) >> 8 == 0 {
            continue;
        }
        let dbz = ((v - 1) & 0xFF) as f32 - 32.0;
        let [r, g, b] = snow_color(dbz);
        // Flurries stay soft so a light snow shield doesn't paint the whole map.
        let fade = ((dbz + 4.0) / 18.0).clamp(0.55, 1.0);
        px.0 = [r, g, b, (a as f32 * fade) as u8];
        snow_px += 1;
    }
    if snow_px > 200 {
        SNOW_SEEN.store(chrono::Utc::now().timestamp(), Ordering::Relaxed);
    }
}

/// The snow row under the radar legend: a "SNOW" tag and the winter ramp with light / moderate /
/// heavy words.
pub fn draw_snow_bar(pm: &mut tiny_skia::Pixmap, glyphs: &mut crate::gfx::text::GlyphCache, rect: (f32, f32, f32, f32), text: tiny_skia::Color, font: &str) {
    use crate::gfx::text::{draw_line, measure, TextStyle, Weight};
    use tiny_skia::{Color, FillRule, GradientStop, LinearGradient, Paint, Point, Shader, SpreadMode, Transform};
    let (x, y, w, h) = rect;
    let tag = TextStyle { family: font.to_string(), weight: Weight::Bold, size: h * 0.5, color: text, letter_spacing: h * 0.03 };
    let tag_w = measure("SNOW", &tag) + h * 0.5;
    draw_line(pm, glyphs, "SNOW", x, y + h * 0.68, &tag, 1.0, None);
    let (bx, bw) = (x + tag_w, w - tag_w);
    let stops: Vec<GradientStop> = SNOW_RAMP
        .iter()
        .filter(|(d, _)| *d <= 54.0)
        .map(|(d, [r, g, b])| GradientStop::new(d / 54.0, Color::from_rgba8(*r, *g, *b, 255)))
        .collect();
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.shader = LinearGradient::new(Point::from_xy(bx, 0.0), Point::from_xy(bx + bw, 0.0), stops, SpreadMode::Pad, Transform::identity()).unwrap_or(Shader::SolidColor(Color::WHITE));
    let bar_h = h * 0.42;
    if let Some(path) = crate::gfx::render::rounded_rect(bx, y + (h - bar_h) / 2.0, bw, bar_h, bar_h * 0.3) {
        pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
    let word = TextStyle { family: font.to_string(), weight: Weight::Bold, size: h * 0.3, color: Color::from_rgba8(12, 18, 34, 230), letter_spacing: h * 0.02 };
    for (at, label) in [(0.12, "LIGHT"), (0.5, "MODERATE"), (0.86, "HEAVY")] {
        let tw = measure(label, &word);
        let color = if at > 0.6 { Color::WHITE } else { word.color };
        let st = TextStyle { color, ..word.clone() };
        draw_line(pm, glyphs, label, bx + bw * at - tw / 2.0, y + h / 2.0 + h * 0.11, &st, 1.0, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snow_and_rain_are_told_apart() {
        let lut = table();
        let look = |c: u32| {
            let (r, g, b) = ((c >> 24) & 0xFF, (c >> 16) & 0xFF, (c >> 8) & 0xFF);
            lut[(((r >> 3) << 10) | ((g >> 3) << 5) | (b >> 3)) as usize]
        };
        // TWC rain green at ~16 dBZ and snow blue at ~40 dBZ.
        assert_eq!((look(0x088815FF) - 1) >> 8, 0);
        assert_eq!((look(0x2F8FFFFF) - 1) >> 8, 1);
        let mut img = RgbaImage::from_pixel(4, 4, image::Rgba([0x2F, 0x8F, 0xFF, 255]));
        filter("radar-4w1-1794499800", &mut img);
        assert_ne!(img.get_pixel(0, 0).0[..3], [0x2F, 0x8F, 0xFF]);
        let mut rain = RgbaImage::from_pixel(4, 4, image::Rgba([0x08, 0x88, 0x15, 255]));
        filter("radar-4w1-1794499800", &mut rain);
        assert_eq!(rain.get_pixel(0, 0).0, [0x08, 0x88, 0x15, 255]);
    }
}

/// LibreWXR "The Weather Channel (TWC)" colour table (scheme 4 = Modern HD), RGBA, one per dBZ from -32 to 95.
pub const TWC_RAIN: [u32; 128] = [0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x01B714FF, 0x01B614FF, 0x01B414FF, 0x02AD14FF, 0x049F14FF, 0x088915FF, 0x088815FF, 0x088615FF, 0x098116FF, 0x0C7617FF, 0x11651AFF, 0x106419FF, 0x106218FF, 0x0E5D15FF, 0x0B5310FF, 0x064307FF, 0x074407FF, 0x154D07FF, 0x3B6707FF, 0x859A07FF, 0xFFEE07FF, 0xFEED07FF, 0xFEEA07FF, 0xFDE207FF, 0xFBD307FF, 0xF8BB08FF, 0xF7BA08FF, 0xF7B708FF, 0xF6B008FF, 0xF5A208FF, 0xF38B08FF, 0xF28A08FF, 0xF28908FF, 0xF28508FF, 0xF17D08FF, 0xF07108FF, 0xEF7008FF, 0xEF6F08FF, 0xEE6C08FF, 0xEC6708FF, 0xEA5E09FF, 0xE95D09FF, 0xE95B09FF, 0xE75509FF, 0xE44A09FF, 0xDF370AFF, 0xDE360AFF, 0xDE340AFF, 0xDC2E0AFF, 0xD8230BFF, 0xD3100CFF, 0xD20F0BFF, 0xD10F0BFF, 0xCE0F0BFF, 0xC90E0AFF, 0xC00D09FF, 0xBF0C08FF, 0xBF0C08FF, 0xBE0C08FF, 0xBB0C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF, 0xB80C08FF];
pub const TWC_SNOW: [u32; 128] = [0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0xEFFFFFFF, 0xEEFFFFFF, 0xEDFFFFFF, 0xEBFFFFFF, 0xE6FFFFFF, 0xDFFFFFFF, 0xDEFFFFFF, 0xDDFFFFFF, 0xDBFFFFFF, 0xD6FFFFFF, 0xCFFFFFFF, 0xCEFFFFFF, 0xCDFFFFFF, 0xCBFFFFFF, 0xC6FFFFFF, 0xBFFFFFFF, 0xBEFFFFFF, 0xBDFFFFFF, 0xBBFFFFFF, 0xB6FFFFFF, 0xAFFFFFFF, 0xAEFFFFFF, 0xADFFFFFF, 0xABFFFFFF, 0xA6FFFFFF, 0x9FFFFFFF, 0x9EFFFFFF, 0x9DFFFFFF, 0x9BFFFFFF, 0x96FFFFFF, 0x8FFFFFFF, 0x8EFEFFFF, 0x8DFDFFFF, 0x8BFBFFFF, 0x86F6FFFF, 0x7FEFFFFF, 0x7EEEFFFF, 0x7DEDFFFF, 0x7BEBFFFF, 0x76E6FFFF, 0x6FDFFFFF, 0x6EDEFFFF, 0x6DDDFFFF, 0x6BDBFFFF, 0x66D6FFFF, 0x5FCFFFFF, 0x5ECEFFFF, 0x5DCCFFFF, 0x5BC8FFFF, 0x56BEFFFF, 0x4FAFFFFF, 0x4EAEFFFF, 0x4DADFFFF, 0x4BABFFFF, 0x46A6FFFF, 0x3F9FFFFF, 0x3E9EFFFF, 0x3D9DFFFF, 0x3B9BFFFF, 0x3696FFFF, 0x2F8FFFFF, 0x2E8EFFFF, 0x2D8DFFFF, 0x2B8BFFFF, 0x2686FFFF, 0x1F7FFFFF, 0x1E7EFFFF, 0x1D7DFFFF, 0x1B7BFFFF, 0x1676FFFF, 0x0F6FFFFF, 0x0E6EFFFF, 0x0E6DFFFF, 0x0B6BFFFF, 0x0766FFFF, 0x005FFFFF, 0x005EFFFF, 0x005DFFFF, 0x005BFFFF, 0x0056FFFF, 0x004FFFFF, 0x004EFFFF, 0x004DFFFF, 0x004BFFFF, 0x0046FFFF, 0x003FFFFF, 0x003EFFFF, 0x003DFFFF, 0x003BFFFF, 0x0036FFFF, 0x002FFFFF, 0x002EFFFF, 0x002DFFFF, 0x002BFFFF, 0x0026FFFF, 0x001FFFFF, 0x001EFFFF, 0x001DFFFF, 0x001BFFFF, 0x0016FFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF, 0x000FFFFF];
