//! Remix icons (Apache-2.0, bundled in assets/icons/remix) rendered onto the map and graphics:
//! storm badges use the solid weather icons, tinted any colour and cached per size.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use tiny_skia::{IntSize, Pixmap};

const ICONS: &[(&str, &[u8])] = &[
    ("hail-fill", include_bytes!("../../assets/icons/remix/hail-fill.svg")),
    ("thunderstorms-fill", include_bytes!("../../assets/icons/remix/thunderstorms-fill.svg")),
    ("heavy-showers-fill", include_bytes!("../../assets/icons/remix/heavy-showers-fill.svg")),
    ("flashlight-fill", include_bytes!("../../assets/icons/remix/flashlight-fill.svg")),
];

type Cache = Mutex<HashMap<(&'static str, u32, String), Option<Arc<Pixmap>>>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// The icon `px` square in `color` (a CSS colour such as "#FFFFFF").
pub fn raster(name: &'static str, px: u32, color: &str) -> Option<Arc<Pixmap>> {
    let px = px.clamp(8, 512);
    let key = (name, px, color.to_string());
    if let Some(hit) = cache().lock().ok()?.get(&key) {
        return hit.clone();
    }
    let rendered = render(name, px, color).map(Arc::new);
    if let Ok(mut c) = cache().lock() {
        if c.len() > 128 {
            c.clear();
        }
        c.insert(key, rendered.clone());
    }
    rendered
}

fn render(name: &str, px: u32, color: &str) -> Option<Pixmap> {
    let (_, data) = ICONS.iter().find(|(n, _)| *n == name)?;
    let svg = std::str::from_utf8(data).ok()?.replace("currentColor", color);
    let tree = resvg::usvg::Tree::from_data(svg.as_bytes(), &resvg::usvg::Options::default()).ok()?;
    let mut pm = resvg::tiny_skia::Pixmap::new(px, px)?;
    let scale = px as f32 / tree.size().width().max(1.0);
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    Pixmap::from_vec(pm.take(), IntSize::from_wh(px, px)?)
}
