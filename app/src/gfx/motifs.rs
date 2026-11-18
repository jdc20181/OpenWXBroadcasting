//! Built-in decorative motifs for themed graphics (holidays): simple original vector shapes
//! drawn in any colour. An Image layer whose media is `motif:<name>:<color>` shows one, e.g.
//! `motif:star:#D9B84A`. Rendered with resvg and cached per size.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use tiny_skia::{IntSize, Pixmap};

/// Every motif name (for the editor and tests).
pub const NAMES: &[&str] = &[
    "star", "heart", "shamrock", "leaf", "pumpkin", "snowflake", "ornament", "firework", "flower", "egg", "flag", "bat", "moon",
    "tie", "confetti", "burst", "dove", "bell",
];

/// The SVG body (inside a 0 0 100 100 view box) for a motif in colour `c`.
fn body(name: &str, c: &str) -> Option<String> {
    Some(match name {
        "star" => format!(r#"<polygon fill="{c}" points="50,4 61,37 96,37 68,58 79,93 50,72 21,93 32,58 4,37 39,37"/>"#),
        "heart" => format!(r#"<path fill="{c}" d="M50 90 C20 66 4 49 4 30 C4 14 16 4 30 4 C40 4 47 10 50 18 C53 10 60 4 70 4 C84 4 96 14 96 30 C96 49 80 66 50 90 Z"/>"#),
        "shamrock" => format!(
            r#"<g fill="{c}"><circle cx="38" cy="26" r="15"/><circle cx="62" cy="26" r="15"/><circle cx="26" cy="50" r="15"/><circle cx="38" cy="66" r="15"/><circle cx="74" cy="50" r="15"/><circle cx="62" cy="66" r="15"/><circle cx="50" cy="46" r="14"/></g><path d="M50 52 Q54 76 66 96" stroke="{c}" stroke-width="7" fill="none" stroke-linecap="round"/>"#
        ),
        "leaf" => format!(r#"<polygon fill="{c}" points="50,4 58,24 75,14 70,38 93,34 78,52 89,60 62,62 64,82 53,70 52,96 48,96 47,70 36,82 38,62 11,60 22,52 7,34 30,38 25,14 42,24"/>"#),
        "pumpkin" => format!(
            r##"<g fill="{c}"><ellipse cx="30" cy="58" rx="22" ry="32"/><ellipse cx="70" cy="58" rx="22" ry="32"/><ellipse cx="50" cy="58" rx="24" ry="35"/></g><g fill="none" stroke="#000" stroke-opacity="0.22" stroke-width="3"><path d="M38 28 Q30 58 38 90"/><path d="M62 28 Q70 58 62 90"/></g><rect x="45" y="12" width="10" height="16" rx="3" fill="#4A7A2A"/>"##
        ),
        "snowflake" => {
            let arm = format!(r#"<path d="M50 50 L50 6 M50 20 L40 10 M50 20 L60 10 M50 34 L38 24 M50 34 L62 24" stroke="{c}" stroke-width="6" stroke-linecap="round" fill="none"/>"#);
            (0..6).map(|i| format!(r#"<g transform="rotate({} 50 50)">{arm}</g>"#, i * 60)).collect()
        }
        "ornament" => format!(
            r##"<circle cx="50" cy="60" r="34" fill="{c}"/><path d="M22 54 Q50 66 78 54" stroke="#fff" stroke-opacity="0.55" stroke-width="5" fill="none"/><rect x="40" y="16" width="20" height="12" rx="2" fill="#C9B26A"/><path d="M50 16 Q44 4 50 4 Q56 4 50 16" stroke="#C9B26A" stroke-width="3" fill="none"/><circle cx="38" cy="46" r="7" fill="#fff" fill-opacity="0.45"/>"##
        ),
        "firework" => {
            let mut s = String::new();
            for i in 0..12 {
                let a = (i as f32) * 30.0_f32.to_radians();
                let (x1, y1) = (50.0 + a.cos() * 14.0, 50.0 + a.sin() * 14.0);
                let (x2, y2) = (50.0 + a.cos() * 42.0, 50.0 + a.sin() * 42.0);
                s += &format!(r#"<line x1="{x1:.1}" y1="{y1:.1}" x2="{x2:.1}" y2="{y2:.1}" stroke="{c}" stroke-width="5" stroke-linecap="round"/><circle cx="{:.1}" cy="{:.1}" r="4" fill="{c}"/>"#, 50.0 + a.cos() * 48.0, 50.0 + a.sin() * 48.0);
            }
            s
        }
        "flower" => {
            let mut s: String = (0..5).map(|i| format!(r#"<ellipse cx="50" cy="26" rx="15" ry="24" fill="{c}" transform="rotate({} 50 50)"/>"#, i * 72)).collect();
            s += r##"<circle cx="50" cy="50" r="14" fill="#F6D24A"/>"##;
            s
        }
        "egg" => format!(
            r##"<path d="M50 6 C76 6 90 46 90 64 C90 84 72 96 50 96 C28 96 10 84 10 64 C10 46 24 6 50 6 Z" fill="{c}"/><path d="M14 58 L26 50 L38 58 L50 50 L62 58 L74 50 L86 58" stroke="#fff" stroke-width="5" fill="none" stroke-linejoin="round"/><g fill="#fff" fill-opacity="0.7"><circle cx="34" cy="76" r="5"/><circle cx="50" cy="80" r="5"/><circle cx="66" cy="76" r="5"/></g>"##
        ),
        "flag" => {
            let mut s = String::from(r##"<rect x="2" y="14" width="96" height="70" fill="#FFFFFF"/>"##);
            for i in 0..7 {
                s += &format!(r##"<rect x="2" y="{:.1}" width="96" height="5.4" fill="#B22234"/>"##, 14.0 + i as f32 * 10.77);
            }
            s += r##"<rect x="2" y="14" width="42" height="37.7" fill="#3C3B6E"/>"##;
            for r in 0..4 {
                for k in 0..5 {
                    s += &format!(r##"<circle cx="{:.1}" cy="{:.1}" r="1.8" fill="#FFFFFF"/>"##, 7.0 + k as f32 * 8.0 + (r % 2) as f32 * 4.0, 19.0 + r as f32 * 9.0);
                }
            }
            s
        }
        "bat" => format!(r#"<polygon fill="{c}" points="4,40 20,34 30,46 40,30 45,38 50,28 55,38 60,30 70,46 80,34 96,40 86,56 76,50 66,64 50,56 34,64 24,50 14,56"/>"#),
        "moon" => format!(r#"<path fill="{c}" d="M62 6 A44 44 0 1 0 94 74 A36 36 0 1 1 62 6 Z"/>"#),
        "tie" => format!(r##"<polygon fill="{c}" points="42,6 58,6 55,20 67,72 50,96 33,72 45,20"/><polygon fill="#000" fill-opacity="0.18" points="42,6 58,6 55,20 45,20"/>"##),
        "confetti" => {
            let pieces = [(12, 18, 20, "#E84855"), (40, 8, -30, "#F9DC5C"), (70, 20, 45, "#3185FC"), (20, 50, 70, "#4CC25A"), (55, 44, 10, "#E84855"), (82, 52, -60, "#F9DC5C"), (30, 80, 35, "#3185FC"), (62, 76, -15, "#E634E8"), (86, 84, 60, "#4CC25A")];
            pieces.iter().map(|(x, y, r, col)| format!(r#"<rect x="{x}" y="{y}" width="12" height="6" rx="1" fill="{col}" transform="rotate({r} {} {})"/>"#, x + 6, y + 3)).collect()
        }
        "burst" => {
            let mut pts = String::new();
            for i in 0..24 {
                let a = (i as f32) * 15.0_f32.to_radians();
                let r = if i % 2 == 0 { 47.0 } else { 30.0 };
                pts += &format!("{:.1},{:.1} ", 50.0 + a.cos() * r, 50.0 + a.sin() * r);
            }
            format!(r#"<polygon fill="{c}" points="{pts}"/>"#)
        }
        "dove" => format!(r#"<path fill="{c}" d="M8 56 C24 40 40 40 52 46 C58 30 70 18 92 14 C80 26 74 38 72 50 C80 52 88 50 94 46 C88 62 72 72 52 72 L34 86 L36 72 C24 70 14 64 8 56 Z"/>"#),
        "bell" => format!(r##"<path fill="{c}" d="M50 8 C68 8 74 24 74 40 C74 58 82 66 90 74 L10 74 C18 66 26 58 26 40 C26 24 32 8 50 8 Z"/><circle cx="50" cy="84" r="9" fill="{c}"/><rect x="46" y="2" width="8" height="8" rx="2" fill="{c}"/>"##),
        _ => return None,
    })
}

/// Parse `motif:<name>:<color>` (colour optional, default white).
pub fn parse(media: &str) -> Option<(&str, &str)> {
    let rest = media.trim().strip_prefix("motif:")?;
    Some(match rest.split_once(':') {
        Some((name, color)) => (name, color),
        None => (rest, "#FFFFFF"),
    })
}

type Cache = Mutex<HashMap<(String, u32), Option<Arc<Pixmap>>>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// The motif for an Image layer's media string, about `size` px square (cached in 32 px steps).
pub fn raster(media: &str, size: f32) -> Option<Arc<Pixmap>> {
    let (name, color) = parse(media)?;
    let px = ((size.max(16.0) / 32.0).ceil() as u32 * 32).min(1024);
    let key = (format!("{name}:{color}"), px);
    if let Some(hit) = cache().lock().ok()?.get(&key) {
        return hit.clone();
    }
    let rendered = render(name, color, px).map(Arc::new);
    if let Ok(mut c) = cache().lock() {
        if c.len() > 256 {
            c.clear();
        }
        c.insert(key, rendered.clone());
    }
    rendered
}

fn render(name: &str, color: &str, px: u32) -> Option<Pixmap> {
    // Only plain colour values reach the SVG.
    let color: String = color.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '#' | '(' | ')' | ',' | '.' | ' ')).collect();
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">{}</svg>"#, body(name, &color)?);
    let tree = resvg::usvg::Tree::from_data(svg.as_bytes(), &resvg::usvg::Options::default()).ok()?;
    let mut big = resvg::tiny_skia::Pixmap::new(px, px)?;
    let scale = px as f32 / 100.0;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut big.as_mut());
    // resvg uses a newer tiny-skia: copy the pixels into ours.
    Pixmap::from_vec(big.take(), IntSize::from_wh(px, px)?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_motif_renders() {
        for name in super::NAMES {
            let p = super::raster(&format!("motif:{name}:#D9B84A"), 64.0).unwrap_or_else(|| panic!("{name} did not render"));
            assert!(p.pixels().iter().any(|px| px.alpha() > 0), "{name} is empty");
        }
    }
}
