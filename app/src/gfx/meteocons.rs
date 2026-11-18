//! Meteocons weather icons for the Modern look: Bas Milius's weather icon set
//! (https://github.com/basmilius/weather-icons, MIT license; the license ships in
//! assets/icons/meteocons and is credited in About). The SVGs are embedded, rendered with
//! resvg and cached per size. The Classic look keeps the drawn icons in `icons.rs`.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use tiny_skia::{IntSize, Pixmap};

use super::icons::IconKind;

macro_rules! icon {
    ($name:literal) => {
        ($name, include_bytes!(concat!("../../assets/icons/meteocons/", $name, ".svg")) as &[u8])
    };
}

const ICONS: &[(&str, &[u8])] = &[
    icon!("clear-day"),
    icon!("clear-night"),
    icon!("partly-cloudy-day"),
    icon!("partly-cloudy-night"),
    icon!("overcast-day"),
    icon!("overcast-night"),
    icon!("cloudy"),
    icon!("overcast"),
    icon!("partly-cloudy-day-rain"),
    icon!("partly-cloudy-night-rain"),
    icon!("rain"),
    icon!("drizzle"),
    icon!("thunderstorms-day-rain"),
    icon!("thunderstorms-night-rain"),
    icon!("thunderstorms-rain"),
    icon!("snow"),
    icon!("partly-cloudy-day-snow"),
    icon!("partly-cloudy-night-snow"),
    icon!("sleet"),
    icon!("fog-day"),
    icon!("fog-night"),
    icon!("fog"),
    icon!("wind"),
    icon!("thermometer-warmer"),
    icon!("thermometer-colder"),
    icon!("hail"),
    icon!("tornado"),
    icon!("smoke"),
    icon!("haze"),
];

/// Meteocons name for an icon kind and time of day. `condition` refines a few cases (drizzle,
/// hail, smoke, haze, tornado) that the kinds lump together.
pub fn name(kind: IconKind, day: bool, condition: &str) -> &'static str {
    let c = condition.to_lowercase();
    let dn = |d: &'static str, n: &'static str| if day { d } else { n };
    if c.contains("tornado") || c.contains("funnel") {
        return "tornado";
    }
    if c.contains("hail") {
        return "hail";
    }
    if c.contains("smoke") {
        return "smoke";
    }
    if c.contains("haze") {
        return "haze";
    }
    match kind {
        IconKind::Clear => dn("clear-day", "clear-night"),
        IconKind::MostlyClear | IconKind::PartlyCloudy => dn("partly-cloudy-day", "partly-cloudy-night"),
        IconKind::MostlyCloudy => dn("overcast-day", "overcast-night"),
        IconKind::Cloudy => "overcast",
        IconKind::Showers if c.contains("drizzle") => "drizzle",
        IconKind::Showers => dn("partly-cloudy-day-rain", "partly-cloudy-night-rain"),
        IconKind::Rain if c.contains("drizzle") => "drizzle",
        IconKind::Rain => "rain",
        IconKind::Thunder if c.contains("chance") || c.contains("isolated") || c.contains("scattered") => dn("thunderstorms-day-rain", "thunderstorms-night-rain"),
        IconKind::Thunder => "thunderstorms-rain",
        IconKind::Snow if c.contains("chance") || c.contains("flurr") => dn("partly-cloudy-day-snow", "partly-cloudy-night-snow"),
        IconKind::Snow => "snow",
        IconKind::Mix => "sleet",
        IconKind::Fog if c.contains("patchy") || c.contains("areas") => dn("fog-day", "fog-night"),
        IconKind::Fog => "fog",
        IconKind::Wind => "wind",
        IconKind::Hot => "thermometer-warmer",
        IconKind::Cold => "thermometer-colder",
    }
}

type Cache = Mutex<HashMap<(&'static str, u32), Option<Arc<Pixmap>>>>;

fn cache() -> &'static Cache {
    static C: OnceLock<Cache> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// The icon rendered `size` px square (sizes are rounded up to 32 px steps and cached).
pub fn raster(name: &'static str, size: f32) -> Option<Arc<Pixmap>> {
    let px = ((size.max(16.0) / 32.0).ceil() as u32 * 32).min(1024);
    let key = (name, px);
    if let Some(hit) = cache().lock().ok()?.get(&key) {
        return hit.clone();
    }
    let rendered = render(name, px).map(Arc::new);
    if let Ok(mut c) = cache().lock() {
        if c.len() > 256 {
            c.clear();
        }
        c.insert(key, rendered.clone());
    }
    rendered
}

fn render(name: &str, px: u32) -> Option<Pixmap> {
    let (_, data) = ICONS.iter().find(|(n, _)| *n == name)?;
    let tree = resvg::usvg::Tree::from_data(data, &resvg::usvg::Options::default()).ok()?;
    let view = tree.size();
    // The artwork sits in the middle of a 64-unit box with generous padding: zoom in a little
    // so the icon fills its layer like the drawn icons do.
    let zoom = 1.22;
    let scale = px as f32 / view.width().max(view.height()).max(1.0) * zoom;
    let offset = (px as f32 - view.width() * scale) / 2.0;
    let mut pm = resvg::tiny_skia::Pixmap::new(px, px)?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, offset, offset), &mut pm.as_mut());
    Pixmap::from_vec(pm.take(), IntSize::from_wh(px, px)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_rendered_icon() {
        for kind in IconKind::ALL {
            for day in [true, false] {
                let n = name(kind, day, "");
                assert!(raster(n, 120.0).is_some(), "{n}");
            }
        }
    }
}
