//! Custom broadcast basemaps ("map themes"): a base layer, terrain, water, roads and labels, each
//! with its own colour adjustments (brightness, contrast, saturation, hue, opacity), plus the
//! station's county / state line colours, town label font and colours, map symbols (town dots,
//! home marker) and the legend look. Themes are kept in Library\Map themes; every save keeps the
//! previous version.

use std::{
    path::PathBuf,
    sync::{OnceLock, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::config::LibraryPaths;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Adjust {
    pub on: bool,
    pub opacity: f32,
    pub brightness_min: f32,
    pub brightness_max: f32,
    /// -1 … 1
    pub contrast: f32,
    /// -1 … 1
    pub saturation: f32,
    /// degrees
    pub hue: f32,
}

impl Default for Adjust {
    fn default() -> Self {
        Self { on: true, opacity: 1.0, brightness_min: 0.0, brightness_max: 1.0, contrast: 0.0, saturation: 0.0, hue: 0.0 }
    }
}

impl Adjust {
    pub fn paint(&self) -> serde_json::Value {
        serde_json::json!({
            "raster-opacity": self.opacity.clamp(0.0, 1.0),
            "raster-brightness-min": self.brightness_min.clamp(0.0, 1.0),
            "raster-brightness-max": self.brightness_max.clamp(0.0, 1.0),
            "raster-contrast": self.contrast.clamp(-1.0, 0.99),
            "raster-saturation": self.saturation.clamp(-1.0, 1.0),
            "raster-hue-rotate": self.hue,
        })
    }
    fn off() -> Self {
        Self { on: false, ..Default::default() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Symbol {
    /// "circle", "square", "diamond", "star", "triangle".
    pub shape: String,
    pub fill: String,
    pub outline: String,
    pub size: f32,
}

impl Default for Symbol {
    fn default() -> Self {
        Self { shape: "circle".into(), fill: "#FFFFFF".into(), outline: "#000000".into(), size: 4.0 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Theme {
    pub name: String,
    pub version: u32,
    pub updated: i64,
    /// "dark", "light", "streets", "imagery", "shaded", "none".
    pub base: String,
    pub background: String,
    pub base_adjust: Adjust,
    pub terrain: Adjust,
    pub water: Adjust,
    pub roads: Adjust,
    pub labels: Adjust,
    /// "dark" or "light" reference labels.
    pub labels_style: String,
    pub county_color: String,
    pub county_width: f32,
    pub state_color: String,
    pub state_width: f32,
    pub label_font: String,
    pub label_color: String,
    pub label_size: f32,
    pub label_caps: bool,
    pub town_symbol: Symbol,
    pub home_symbol: Symbol,
    /// Legend: "bottom-left", "bottom-right", "top-right", "hidden", and its colours.
    pub legend_position: String,
    pub legend_background: String,
    pub legend_text: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "New theme".into(),
            version: 1,
            updated: 0,
            base: "dark".into(),
            background: "#0B0F17".into(),
            base_adjust: Adjust::default(),
            terrain: Adjust { opacity: 0.35, ..Adjust::off() },
            water: Adjust::off(),
            roads: Adjust::off(),
            labels: Adjust::default(),
            labels_style: "dark".into(),
            county_color: "#C8D2E1".into(),
            county_width: 1.2,
            state_color: "#EBF0FA".into(),
            state_width: 2.4,
            label_font: String::new(),
            label_color: "#FFFFFF".into(),
            label_size: 22.0,
            label_caps: false,
            town_symbol: Symbol::default(),
            home_symbol: Symbol { shape: "star".into(), fill: "#FFD60A".into(), outline: "#000000".into(), size: 9.0 },
            legend_position: "bottom-left".into(),
            legend_background: "rgba(10,14,22,0.82)".into(),
            legend_text: "#FFFFFF".into(),
        }
    }
}

pub const BASES: &[(&str, &str)] = &[("dark", "Dark gray"), ("light", "Light gray"), ("streets", "Streets"), ("imagery", "Satellite imagery"), ("shaded", "Shaded relief"), ("none", "Plain colour")];

pub fn base_tiles(base: &str) -> Option<&'static str> {
    Some(match base {
        "dark" => "https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Dark_Gray_Base/MapServer/tile/{z}/{y}/{x}",
        "light" => "https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Base/MapServer/tile/{z}/{y}/{x}",
        "streets" => "https://server.arcgisonline.com/ArcGIS/rest/services/World_Street_Map/MapServer/tile/{z}/{y}/{x}",
        "imagery" => "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
        "shaded" => "https://server.arcgisonline.com/ArcGIS/rest/services/World_Shaded_Relief/MapServer/tile/{z}/{y}/{x}",
        _ => return None,
    })
}

pub const TERRAIN_TILES: &str = "https://server.arcgisonline.com/ArcGIS/rest/services/Elevation/World_Hillshade/MapServer/tile/{z}/{y}/{x}";
pub const WATER_TILES: &str = "https://tiles.arcgis.com/tiles/P3ePLMYs2RVChkJx/arcgis/rest/services/Esri_Hydro_Reference_Overlay/MapServer/tile/{z}/{y}/{x}";
pub const ROAD_TILES: &str = "https://server.arcgisonline.com/ArcGIS/rest/services/Reference/World_Transportation/MapServer/tile/{z}/{y}/{x}";

pub fn label_tiles(style: &str) -> &'static str {
    if style == "light" {
        "https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Reference/MapServer/tile/{z}/{y}/{x}"
    } else {
        "https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Dark_Gray_Reference/MapServer/tile/{z}/{y}/{x}"
    }
}

fn dir() -> PathBuf {
    LibraryPaths::get().root.join("Map themes")
}

fn file_name(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect::<String>().trim().to_string()
}

fn registry() -> &'static RwLock<Vec<Theme>> {
    static R: OnceLock<RwLock<Vec<Theme>>> = OnceLock::new();
    R.get_or_init(|| {
        let mut list: Vec<Theme> = std::fs::read_dir(dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
            .filter_map(|e| serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok())
            .collect();
        list.sort_by(|a: &Theme, b: &Theme| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        RwLock::new(list)
    })
}

pub fn list() -> Vec<Theme> {
    registry().read().map(|r| r.clone()).unwrap_or_default()
}

pub fn get(name: &str) -> Option<Theme> {
    if name.is_empty() {
        return None;
    }
    registry().read().ok()?.iter().find(|t| t.name == name).cloned()
}

/// Save as a new version (the old one goes to Map themes\history).
pub fn save(mut t: Theme) -> anyhow::Result<Theme> {
    let d = dir();
    std::fs::create_dir_all(d.join("history"))?;
    let path = d.join(format!("{}.json", file_name(&t.name)));
    if let Some(old) = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<Theme>(&s).ok()) {
        let mut a = old.clone();
        let mut b = t.clone();
        a.version = 0;
        a.updated = 0;
        b.version = 0;
        b.updated = 0;
        if a == b {
            return Ok(old);
        }
        std::fs::write(d.join("history").join(format!("{}-v{}.json", file_name(&old.name), old.version)), serde_json::to_string_pretty(&old)?)?;
        t.version = old.version + 1;
    }
    t.updated = chrono::Utc::now().timestamp();
    std::fs::write(&path, serde_json::to_string_pretty(&t)?)?;
    if let Ok(mut r) = registry().write() {
        r.retain(|x| x.name != t.name);
        r.push(t.clone());
        r.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }
    Ok(t)
}

pub fn delete(name: &str) {
    let _ = std::fs::remove_file(dir().join(format!("{}.json", file_name(name))));
    if let Ok(mut r) = registry().write() {
        r.retain(|x| x.name != name);
    }
}

/// Older versions: (version, saved time).
pub fn versions(name: &str) -> Vec<(u32, i64)> {
    let prefix = format!("{}-v", file_name(name));
    let mut out: Vec<(u32, i64)> = std::fs::read_dir(dir().join("history"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let f = e.file_name().to_string_lossy().to_string();
            let v = f.strip_prefix(&prefix)?.strip_suffix(".json")?.parse().ok()?;
            let t: Theme = serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            Some((v, t.updated))
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out
}

pub fn restore(name: &str, version: u32) -> anyhow::Result<Theme> {
    let old: Theme = serde_json::from_str(&std::fs::read_to_string(dir().join("history").join(format!("{}-v{version}.json", file_name(name))))?)?;
    save(old)
}

/// "#RRGGBB", "#RRGGBBAA" or "rgba(r,g,b,a)" -> tiny-skia colour.
pub fn color(text: &str, fallback: tiny_skia::Color) -> tiny_skia::Color {
    let t = text.trim();
    if let Some(hex) = t.strip_prefix('#') {
        let v = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
        return match hex.len() {
            6 => v(0).zip(v(2)).zip(v(4)).map(|((r, g), b)| tiny_skia::Color::from_rgba8(r, g, b, 255)),
            8 => v(0).zip(v(2)).zip(v(4)).zip(v(6)).map(|(((r, g), b), a)| tiny_skia::Color::from_rgba8(r, g, b, a)),
            _ => None,
        }
        .unwrap_or(fallback);
    }
    if let Some(inner) = t.strip_prefix("rgba(").or_else(|| t.strip_prefix("rgb(")).and_then(|s| s.strip_suffix(')')) {
        let p: Vec<f32> = inner.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if p.len() >= 3 {
            let a = p.get(3).copied().unwrap_or(1.0);
            return tiny_skia::Color::from_rgba8(p[0] as u8, p[1] as u8, p[2] as u8, (a * 255.0) as u8);
        }
    }
    fallback
}

/// Draw a map symbol centred at (x, y).
pub fn draw_symbol(pm: &mut tiny_skia::Pixmap, s: &Symbol, x: f32, y: f32, scale: f32) {
    use tiny_skia::{FillRule, Paint, PathBuilder, Stroke, Transform};
    let r = s.size.max(1.0) * scale;
    let mut pb = PathBuilder::new();
    match s.shape.as_str() {
        "square" => {
            if let Some(rect) = tiny_skia::Rect::from_xywh(x - r, y - r, r * 2.0, r * 2.0) {
                pb.push_rect(rect);
            }
        }
        "diamond" => {
            pb.move_to(x, y - r * 1.2);
            pb.line_to(x + r * 1.2, y);
            pb.line_to(x, y + r * 1.2);
            pb.line_to(x - r * 1.2, y);
            pb.close();
        }
        "triangle" => {
            pb.move_to(x, y - r * 1.2);
            pb.line_to(x + r * 1.1, y + r * 0.9);
            pb.line_to(x - r * 1.1, y + r * 0.9);
            pb.close();
        }
        "star" => {
            for i in 0..10 {
                let a = std::f32::consts::PI * i as f32 / 5.0 - std::f32::consts::FRAC_PI_2;
                let rr = if i % 2 == 0 { r * 1.3 } else { r * 0.55 };
                let (px, py) = (x + rr * a.cos(), y + rr * a.sin());
                if i == 0 {
                    pb.move_to(px, py);
                } else {
                    pb.line_to(px, py);
                }
            }
            pb.close();
        }
        _ => pb.push_circle(x, y, r),
    }
    if let Some(path) = pb.finish() {
        let mut fill = Paint::default();
        fill.anti_alias = true;
        fill.set_color(color(&s.fill, tiny_skia::Color::WHITE));
        pm.fill_path(&path, &fill, FillRule::Winding, Transform::identity(), None);
        let mut line = Paint::default();
        line.anti_alias = true;
        line.set_color(color(&s.outline, tiny_skia::Color::BLACK));
        pm.stroke_path(&path, &line, &Stroke { width: 1.5, ..Default::default() }, Transform::identity(), None);
    }
}
