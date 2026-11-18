//! Single-site reflectivity clean-up (CWXB-88). The NEXRAD site tiles from the Iowa
//! Environmental Mesonet arrive already coloured in the NWS reflectivity scale. Each pixel is
//! matched back to a dBZ on that scale; weak returns (clear air, insects, ground clutter around
//! the radar) under the layer's threshold are hidden, and with the Modern HD palette (TWC,
//! LibreWXR palette 4) the rest is repainted in the same colours as the composite radar.
//!
//! Layer ids carry the settings, so changing them reloads the tiles:
//! `nexrad-ref-{t|n}{min_dbz}` (t = repaint in TWC colours, n = keep NWS colours).

use std::sync::OnceLock;

use image::RgbaImage;

/// NWS reflectivity scale (dBZ, colour), the one the site tiles are drawn in.
const NWS_RAMP: [(f32, [u8; 3]); 15] = [
    (5.0, [4, 233, 231]),
    (10.0, [1, 159, 244]),
    (15.0, [3, 0, 244]),
    (20.0, [2, 253, 2]),
    (25.0, [1, 197, 1]),
    (30.0, [0, 142, 0]),
    (35.0, [253, 248, 2]),
    (40.0, [229, 188, 0]),
    (45.0, [253, 149, 0]),
    (50.0, [253, 0, 0]),
    (55.0, [212, 0, 0]),
    (60.0, [188, 0, 0]),
    (65.0, [248, 0, 253]),
    (70.0, [152, 84, 198]),
    (75.0, [253, 253, 253]),
];

/// The default clear-air threshold, dBZ.
pub const DEFAULT_MIN_DBZ: f32 = 18.0;

/// Threshold choices offered in the Radar tab (0 = off).
pub const MIN_DBZ_CHOICES: [f32; 4] = [0.0, 10.0, 18.0, 25.0];

fn ramp_color(dbz: f32) -> [u8; 3] {
    let last = NWS_RAMP.len() - 1;
    if dbz <= NWS_RAMP[0].0 {
        return NWS_RAMP[0].1;
    }
    for i in 0..last {
        let (a, ca) = NWS_RAMP[i];
        let (b, cb) = NWS_RAMP[i + 1];
        if dbz <= b {
            let t = (dbz - a) / (b - a);
            let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
            return [mix(ca[0], cb[0]), mix(ca[1], cb[1]), mix(ca[2], cb[2])];
        }
    }
    NWS_RAMP[last].1
}

/// 15-bit colour -> dBZ + 64 (0 = not a colour on the scale).
fn table() -> &'static [u8] {
    static T: OnceLock<Vec<u8>> = OnceLock::new();
    T.get_or_init(|| {
        let ramp: Vec<(f32, [u8; 3])> = (0..=140).map(|k| 5.0 + k as f32 * 0.5).map(|d| (d, ramp_color(d))).collect();
        let mut out = vec![0u8; 1 << 15];
        for (key, slot) in out.iter_mut().enumerate() {
            let c = [((key >> 10) & 31) as i32 * 8 + 4, ((key >> 5) & 31) as i32 * 8 + 4, (key & 31) as i32 * 8 + 4];
            let mut best = (i32::MAX, 0.0f32);
            for (d, rc) in &ramp {
                let dist = (0..3).map(|i| (c[i] - rc[i] as i32).pow(2)).sum::<i32>();
                if dist < best.0 {
                    best = (dist, *d);
                }
            }
            // Only colours close to the scale count (anti-aliased edges and map art stay).
            if best.0 <= 40 * 40 {
                *slot = (best.1.round() as i32 + 64).clamp(1, 255) as u8;
            }
        }
        out
    })
}

/// The layer id for a reflectivity site layer with these settings.
pub fn layer_id(twc: bool, min_dbz: f32) -> String {
    format!("nexrad-ref-{}{:.0}", if twc { "t" } else { "n" }, min_dbz.max(0.0))
}

/// TWC rain colour for a dBZ (None below the table's first colour).
pub fn twc_color(dbz: f32) -> Option<[u8; 3]> {
    let i = (dbz.round() as i32 + 32).clamp(0, 127) as usize;
    let v = crate::map::winter_radar::TWC_RAIN[i];
    (v != 0).then(|| [(v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8])
}

/// Tile filter for single-site reflectivity layers.
pub fn filter(layer_id: &str, img: &mut RgbaImage) {
    let Some(rest) = layer_id.strip_prefix("nexrad-ref-") else { return };
    let twc = rest.starts_with('t');
    let min_dbz: f32 = rest[1..].parse().unwrap_or(DEFAULT_MIN_DBZ);
    let lut = table();
    for px in img.pixels_mut() {
        if px.0[3] == 0 {
            continue;
        }
        let key = ((px.0[0] as usize >> 3) << 10) | ((px.0[1] as usize >> 3) << 5) | (px.0[2] as usize >> 3);
        let v = lut[key];
        if v == 0 {
            continue;
        }
        let dbz = v as f32 - 64.0;
        if dbz < min_dbz {
            px.0[3] = 0;
            continue;
        }
        if twc {
            match twc_color(dbz) {
                Some([r, g, b]) => px.0 = [r, g, b, px.0[3]],
                None => px.0[3] = 0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weak_returns_hidden_and_repainted() {
        let mut img = RgbaImage::new(3, 1);
        let c = |d: f32| {
            let [r, g, b] = ramp_color(d);
            image::Rgba([r, g, b, 255])
        };
        img.put_pixel(0, 0, c(10.0));
        img.put_pixel(1, 0, c(40.0));
        img.put_pixel(2, 0, image::Rgba([120, 120, 120, 255]));
        filter(&layer_id(true, 18.0), &mut img);
        assert_eq!(img.get_pixel(0, 0).0[3], 0, "clear-air return hidden");
        assert_eq!(&img.get_pixel(1, 0).0[..3], &twc_color(40.0).unwrap()[..], "repainted in TWC colours");
        assert_eq!(img.get_pixel(2, 0).0, [120, 120, 120, 255], "non-radar colour untouched");
    }
}
