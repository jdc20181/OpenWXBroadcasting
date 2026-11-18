//! LibreWXR palette 4, "The Weather Channel (TWC)": the house colours for everything that looks
//! like radar or precipitation (radar, simulated radar, model precipitation, rain and snow
//! totals). Colours come from LibreWXR's own table (`map::winter_radar::TWC_RAIN` / `TWC_SNOW`,
//! one row per dBZ from -32 to 95), never from hand-picked lookalikes.
//!
//! Model precipitation rates become the reflectivity a radar would see with the standard Z-R
//! relations (Marshall-Palmer for rain, Z = 2000 R² for snow), so a model map and the live radar
//! use the same colour for the same intensity.

use crate::map::winter_radar::{TWC_RAIN, TWC_SNOW};

fn rgba(entry: u32) -> [u8; 4] {
    entry.to_be_bytes()
}

/// Colour at `dbz` in a table, interpolated between whole-dBZ rows. None below the table's first
/// coloured row (LibreWXR draws nothing there).
fn at(table: &[u32; 128], dbz: f32) -> Option<[u8; 3]> {
    if !dbz.is_finite() {
        return None;
    }
    let x = (dbz + 32.0).clamp(0.0, 127.0);
    let (i0, i1) = (x.floor() as usize, (x.floor() as usize + 1).min(127));
    let (a, b) = (rgba(table[i0]), rgba(table[i1]));
    let t = x - i0 as f32;
    match (a[3] > 0, b[3] > 0) {
        (false, false) => None,
        (false, true) => (t >= 0.5).then_some([b[0], b[1], b[2]]),
        (true, false) => Some([a[0], a[1], a[2]]),
        (true, true) => {
            let mix = |p: u8, q: u8| (p as f32 + (q as f32 - p as f32) * t).round() as u8;
            Some([mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])])
        }
    }
}

/// TWC rain colour for a reflectivity.
pub fn rain(dbz: f32) -> Option<[u8; 3]> {
    at(&TWC_RAIN, dbz)
}

/// TWC snow colour for a reflectivity.
pub fn snow(dbz: f32) -> Option<[u8; 3]> {
    at(&TWC_SNOW, dbz)
}

/// Reflectivity of rain falling at `rate` inches per hour (Marshall-Palmer, Z = 200 R^1.6).
pub fn rain_dbz(rate: f32) -> f32 {
    let mm = (rate * 25.4).max(1e-4);
    10.0 * (200.0 * mm.powf(1.6)).log10()
}

/// Reflectivity of snow with `rate` inches per hour of liquid (Z = 2000 R²).
pub fn snow_dbz(rate: f32) -> f32 {
    let mm = (rate * 25.4).max(1e-4);
    10.0 * (2000.0 * mm * mm).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_come_from_the_librewxr_table() {
        // Row 42 (10 dBZ) is TWC's first rain green, row 62 (30 dBZ) its yellow.
        assert_eq!(rain(10.0), Some([0x01, 0xB7, 0x14]));
        assert_eq!(rain(30.0), Some([0xFF, 0xEE, 0x07]));
        assert_eq!(rain(0.0), None);
        assert_eq!(snow(-20.0), Some([0xEF, 0xFF, 0xFF]));
        // Light rain is green, heavy rain orange or red, as on radar.
        assert!(rain_dbz(0.01) < 20.0 && rain_dbz(1.0) > 45.0);
    }

    /// The legend stops in `grid` are exact samples of this table.
    #[test]
    fn grid_ramps_match_the_table() {
        for &(rate, rgb) in crate::grid::rain_ramp() {
            assert_eq!(Some(rgb), rain(rain_dbz(rate)), "rain {rate}");
        }
        for &(rate, rgb) in crate::grid::snow_ramp() {
            assert_eq!(Some(rgb), snow(snow_dbz(rate)), "snow {rate}");
        }
        for (&(amount, rgb), dbz) in crate::grid::RAIN_TOTAL_STOPS.iter().zip(crate::grid::RAIN_TOTAL_DBZ) {
            assert_eq!(Some(rgb), rain(dbz), "rain total {amount}");
        }
        for (&(amount, rgb), dbz) in crate::grid::SNOW_TOTAL_STOPS.iter().zip(crate::grid::SNOW_TOTAL_DBZ) {
            assert_eq!(Some(rgb), snow(dbz), "snow total {amount}");
        }
    }
}
