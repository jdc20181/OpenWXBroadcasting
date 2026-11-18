//! Broadcast graphics engine: one software renderer (tiny-skia + fontdue) used for the
//! Graphics Studio preview, on-air overlays, PNG export and video recording, so what you see
//! while editing is exactly what goes to air.

pub mod bind;
pub mod backgrounds;
pub mod browser;
pub mod bind_ext;
pub mod chart;
pub mod diagrams;
pub mod check;
pub mod icons;
pub mod look;
pub mod meteocons;
pub mod model;
pub mod motifs;
pub mod remix;
pub mod holidays;
pub mod riskmap;
pub mod sets;
pub mod history_set;
pub mod explain_set;
pub mod winter_map;
pub mod render;
pub mod templates;
pub mod testing;
pub mod text;
mod variable_font;

use tiny_skia::Color;

use crate::config::Brand;

/// Parse "#RGB", "#RRGGBB", "#RRGGBBAA", "rgba(r,g,b,a)" or a brand token ("$primary", …).
pub fn parse_color(value: &str, brand: &Brand) -> Color {
    let value = value.trim();
    let resolved = match value {
        "$primary" => brand.primary.as_str(),
        "$secondary" => brand.secondary.as_str(),
        "$panel" => brand.panel.as_str(),
        "$text" => brand.text.as_str(),
        "$muted" => brand.muted.as_str(),
        "$alert" => brand.alert.as_str(),
        "$warning" => brand.warning.as_str(),
        other => other,
    };
    parse_hex(resolved).unwrap_or(Color::WHITE)
}

pub fn parse_hex(value: &str) -> Option<Color> {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))
        .and_then(|v| v.strip_suffix(')'))
    {
        let parts: Vec<f32> = inner
            .split(',')
            .filter_map(|p| p.trim().parse::<f32>().ok())
            .collect();
        if parts.len() >= 3 {
            let a = parts.get(3).copied().unwrap_or(1.0);
            return Color::from_rgba(parts[0] / 255.0, parts[1] / 255.0, parts[2] / 255.0, a);
        }
        return None;
    }
    let hex = value.strip_prefix('#')?;
    let digits: Vec<u8> = hex
        .chars()
        .map(|c| c.to_digit(16).map(|d| d as u8))
        .collect::<Option<Vec<u8>>>()?;
    let (r, g, b, a) = match digits.len() {
        3 => (digits[0] * 17, digits[1] * 17, digits[2] * 17, 255),
        6 => (
            digits[0] * 16 + digits[1],
            digits[2] * 16 + digits[3],
            digits[4] * 16 + digits[5],
            255,
        ),
        8 => (
            digits[0] * 16 + digits[1],
            digits[2] * 16 + digits[3],
            digits[4] * 16 + digits[5],
            digits[6] * 16 + digits[7],
        ),
        _ => return None,
    };
    Some(Color::from_rgba8(r, g, b, a))
}

pub fn with_alpha(color: Color, alpha: f32) -> Color {
    let mut c = color;
    c.set_alpha((c.alpha() * alpha).clamp(0.0, 1.0));
    c
}

/// Convert a premultiplied RGBA pixmap buffer to straight RGBA (for PNG/encoders).
pub fn demultiply_rgba(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for px in data.chunks_exact(4) {
        let a = px[3];
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else if a == 255 {
            out.extend_from_slice(px);
        } else {
            let f = 255.0 / a as f32;
            out.extend_from_slice(&[
                (px[0] as f32 * f).min(255.0) as u8,
                (px[1] as f32 * f).min(255.0) as u8,
                (px[2] as f32 * f).min(255.0) as u8,
                a,
            ]);
        }
    }
    out
}

/// Easing used by every animation (easeOutCubic).
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Easing with a small overshoot (for "pop" animations).
pub fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

static TEXT_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1000);

/// Scale for on-air text (Settings > Accessibility > On-air text size), 0.8-1.5.
pub fn set_text_scale(scale: f32) {
    TEXT_SCALE.store((scale.clamp(0.5, 2.0) * 1000.0) as u32, std::sync::atomic::Ordering::Relaxed);
}

pub fn text_scale() -> f32 {
    TEXT_SCALE.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0
}

#[cfg(test)]
mod recovery_tests;
#[cfg(test)]
mod program_icon_tests;
