//! Graphic looks: "classic" (the original OpenWX frame) and "modern" (angled title plate,
//! glass panels, slimmer footer and smoother motion, like current network graphics).
//!
//! A look restyles the frame layers every built-in template shares (Header, Header accent,
//! Title, Header right, Footer, Footer accent, Footer left / right and the content panels),
//! matched by name, so it works on saved graphics too and can be switched back and forth
//! without losing the graphic's own content. Other layers keep their positions; only the
//! entrance animations are swapped (Slide up <-> Rise, Pop <-> Zoom in, Wipe <-> Grow).

use std::sync::RwLock;

use super::model::{AnimKind, Fill, GraphicDoc, LayerKind, StrokeStyle};

pub const LOOKS: [(&str, &str); 2] = [("modern", "Modern"), ("classic", "Classic")];

const CLASSIC_BG: (&str, &str, f32) = ("#0C1626", "#173A5C", 115.0);
const MODERN_BG: (&str, &str, f32) = ("#050E1C", "#12305A", 155.0);
const CLASSIC_PANEL: &str = "rgba(255,255,255,0.07)";
const MODERN_PANEL_FROM: &str = "rgba(10,22,42,0.88)";
const MODERN_PANEL_TO: &str = "rgba(22,42,72,0.80)";

fn default_look() -> &'static RwLock<String> {
    static L: std::sync::OnceLock<RwLock<String>> = std::sync::OnceLock::new();
    L.get_or_init(|| RwLock::new("modern".into()))
}

/// The look new graphics are built in (Settings > Graphics, default Modern).
pub fn set_default(look: &str) {
    if let Ok(mut l) = default_look().write() {
        *l = normalize(look).to_string();
    }
}

pub fn default() -> String {
    default_look().read().map(|l| l.clone()).unwrap_or_else(|_| "modern".into())
}

pub fn normalize(look: &str) -> &'static str {
    if look.eq_ignore_ascii_case("classic") { "classic" } else { "modern" }
}

/// Look of a saved graphic ("" was saved before looks existed: classic).
pub fn of(doc: &GraphicDoc) -> &'static str {
    if doc.look.is_empty() { "classic" } else { normalize(&doc.look) }
}

fn is_classic_panel(kind: &LayerKind) -> bool {
    matches!(kind, LayerKind::Rect { fill: Fill::Solid { color }, .. } if color.replace(' ', "") == CLASSIC_PANEL)
}

fn is_modern_panel(kind: &LayerKind) -> bool {
    matches!(kind, LayerKind::Rect { fill: Fill::Linear { from, to, .. }, .. } if from == MODERN_PANEL_FROM && to == MODERN_PANEL_TO)
}

fn swap_anim(kind: AnimKind, modern: bool) -> AnimKind {
    match (kind, modern) {
        (AnimKind::SlideUp, true) => AnimKind::Rise,
        (AnimKind::Pop, true) => AnimKind::Zoom,
        (AnimKind::Wipe, true) => AnimKind::Grow,
        (AnimKind::Rise, false) => AnimKind::SlideUp,
        (AnimKind::Zoom, false) => AnimKind::Pop,
        (AnimKind::Grow, false) => AnimKind::Wipe,
        (k, _) => k,
    }
}

fn set_text(kind: &mut LayerKind, size_v: f32, color_v: &str, upper: bool, spacing: f32) {
    if let LayerKind::Text { size, color, uppercase, letter_spacing, auto_fit, shadow, .. } = kind {
        *size = size_v;
        *color = color_v.into();
        *uppercase = upper;
        *letter_spacing = spacing;
        *auto_fit = true;
        *shadow = color_v != "#0B1A30" && color_v != "$primary" && color_v != "$muted";
    }
}

fn set_align(kind: &mut LayerKind, a: crate::gfx::text::Align) {
    if let LayerKind::Text { align, .. } = kind {
        *align = a;
    }
}

/// Restyle `doc` in `look` ("modern" or "classic").
pub fn apply(doc: &mut GraphicDoc, look: &str) {
    use crate::gfx::text::Align;
    let modern = normalize(look) == "modern";
    doc.look = normalize(look).to_string();

    // Build 9's first severe graphics had their own backgrounds that clashed with the frame.
    if modern {
        if let Some(Fill::Linear { from, to, angle }) = &mut doc.background {
            if ["#1A0B10", "#0B0F19", "#0E1A2C", "#07140C"].contains(&from.as_str()) {
                *from = MODERN_BG.0.into();
                *to = MODERN_BG.1.into();
                *angle = MODERN_BG.2;
            }
        }
    }
    // Background: only the stock gradient is swapped (custom backgrounds stay).
    if let Some(Fill::Linear { from, to, angle }) = &mut doc.background {
        let (src, dst) = if modern { (CLASSIC_BG, MODERN_BG) } else { (MODERN_BG, CLASSIC_BG) };
        if from.as_str() == src.0 && to.as_str() == src.1 {
            *from = dst.0.into();
            *to = dst.1.into();
            *angle = dst.2;
        }
    }

    let full_width = doc.width as f32;
    // Only graphics built with the standard header / footer get them restyled. Lower thirds,
    // cards and custom graphics also name layers "Title" or "Footer…"; moving those broke them
    // (a lower third's headline jumped to the top-left in dark navy).
    let has_header = doc.layers.iter().any(|l| l.name == "Header" && matches!(l.kind, LayerKind::Rect { .. }) && l.y <= 40.0 && l.h >= 90.0);
    let has_footer = doc.layers.iter().any(|l| l.name == "Footer" && matches!(l.kind, LayerKind::Rect { .. }) && l.y >= 960.0);
    for l in &mut doc.layers {
        l.anim_in.kind = swap_anim(l.anim_in.kind, modern);
        let name = match l.name.as_str() {
            // The header's own title sits in the top band; any other "Title" is content.
            "Title" if !(has_header && l.y < 200.0) => "",
            "Header" | "Header accent" | "Header right" | "Title" if !has_header => "",
            "Footer" | "Footer accent" | "Footer left" | "Footer right" if !has_footer => "",
            name => name,
        };
        match name {
            "Header" => {
                if let LayerKind::Rect { fill, radius, skew, sheen, .. } = &mut l.kind {
                    if modern {
                        (l.x, l.y, l.w, l.h) = (44.0, 32.0, full_width.min(1920.0) - 88.0, 100.0);
                        *fill = Fill::linear("#FFFFFF", "#E3E9F2", 90.0);
                        *radius = 0.0;
                        *skew = 0.16;
                        *sheen = false;
                        l.anim_in.kind = AnimKind::Grow;
                        l.anim_in.duration = 0.6;
                    } else {
                        (l.x, l.y, l.w, l.h) = (0.0, 0.0, full_width, 140.0);
                        *fill = Fill::linear("$secondary", "$panel", 0.0);
                        *radius = 0.0;
                        *skew = 0.0;
                        *sheen = false;
                        l.anim_in.kind = AnimKind::SlideDown;
                        l.anim_in.duration = 0.45;
                    }
                }
            }
            "Header accent" => {
                if let LayerKind::Rect { fill, skew, sheen, .. } = &mut l.kind {
                    *fill = Fill::solid("$primary");
                    if modern {
                        // Deep navy so the white subtitle stays readable when shrunk for social
                        // media (a bright station colour under white text washed it out).
                        *fill = Fill::linear("#0A1B36", "#163A70", 0.0);
                        (l.x, l.y, l.w, l.h) = (60.0, 132.0, 1040.0, 42.0);
                        *skew = 0.16;
                        *sheen = false;
                    } else {
                        (l.x, l.y, l.w, l.h) = (0.0, 140.0, full_width, 8.0);
                        *skew = 0.0;
                        *sheen = false;
                    }
                }
            }
            "Title" => {
                if modern {
                    (l.x, l.y, l.w, l.h) = (96.0, 36.0, 1360.0, 92.0);
                    set_text(&mut l.kind, 72.0, "#0B1A30", true, 1.0);
                    l.anim_in.kind = AnimKind::Rise;
                } else {
                    (l.x, l.y, l.w, l.h) = (80.0, 28.0, 1100.0, 90.0);
                    set_text(&mut l.kind, 70.0, "$text", true, 3.0);
                    l.anim_in.kind = AnimKind::SlideLeft;
                }
            }
            "Header right" => {
                if modern {
                    (l.x, l.y, l.w, l.h) = (104.0, 134.0, 960.0, 38.0);
                    set_text(&mut l.kind, 27.0, "#FFFFFF", true, 2.0);
                    set_align(&mut l.kind, Align::Left);
                } else {
                    (l.x, l.y, l.w, l.h) = (1100.0, 34.0, 740.0, 80.0);
                    set_text(&mut l.kind, 40.0, "$primary", false, 0.0);
                    set_align(&mut l.kind, Align::Right);
                }
            }
            "Footer" => {
                if let LayerKind::Rect { fill, .. } = &mut l.kind {
                    if modern {
                        (l.x, l.y, l.w, l.h) = (0.0, 1004.0, full_width, 76.0);
                        *fill = Fill::linear("#040B17", "#0B1D38", 0.0);
                    } else {
                        (l.x, l.y, l.w, l.h) = (0.0, 980.0, full_width, 100.0);
                        *fill = Fill::solid("$panel");
                    }
                }
            }
            "Footer accent" => {
                if modern {
                    (l.x, l.y, l.w, l.h) = (0.0, 1000.0, full_width, 4.0);
                } else {
                    (l.x, l.y, l.w, l.h) = (0.0, 980.0, full_width, 4.0);
                }
            }
            "Footer left" => {
                if modern {
                    (l.x, l.y, l.w, l.h) = (80.0, 1010.0, 900.0, 62.0);
                    set_text(&mut l.kind, 32.0, "$text", true, 2.0);
                } else {
                    (l.x, l.y, l.w, l.h) = (80.0, 995.0, 900.0, 70.0);
                    set_text(&mut l.kind, 40.0, "$text", true, 2.0);
                }
            }
            "Footer right" => {
                if modern {
                    (l.x, l.y, l.w, l.h) = (960.0, 1010.0, 880.0, 62.0);
                    set_text(&mut l.kind, 26.0, "$muted", false, 0.0);
                } else {
                    (l.x, l.y, l.w, l.h) = (960.0, 995.0, 880.0, 70.0);
                    set_text(&mut l.kind, 30.0, "$muted", false, 0.0);
                }
            }
            _ => {
                // Content panels.
                if modern && is_classic_panel(&l.kind) {
                    if let LayerKind::Rect { fill, stroke, radius, .. } = &mut l.kind {
                        *fill = Fill::linear(MODERN_PANEL_FROM, MODERN_PANEL_TO, 90.0);
                        *stroke = Some(StrokeStyle { color: "rgba(255,255,255,0.12)".into(), width: 2.0 });
                        *radius = 12.0;
                    }
                } else if !modern && is_modern_panel(&l.kind) {
                    if let LayerKind::Rect { fill, stroke, radius, .. } = &mut l.kind {
                        *fill = Fill::solid(CLASSIC_PANEL);
                        *stroke = None;
                        *radius = 22.0;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Renders the new and restyled graphics to PNGs when CWX_QA_NEW_DIR is set (visual QA).
    #[test]
    fn qa_render_new_graphics() {
        let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) else { return };
        std::fs::create_dir_all(&dir).unwrap();
        let settings = crate::config::Settings::default();
        let mut data = crate::data::WxSnapshot::default();
        // Sample outlooks around home for the county risk maps: a west-to-east band.
        let home = settings.home;
        let band = |dx: f64, label: &str, fill: &str, rank: i32| crate::data::SpcArea {
            label: label.into(),
            rank,
            stroke: fill.into(),
            fill: fill.into(),
            fill_opacity: 1.0,
            rings: vec![vec![(home.lon - 3.0, home.lat - 3.0), (home.lon + dx, home.lat - 3.0), (home.lon + dx, home.lat + 3.0), (home.lon - 3.0, home.lat + 3.0)]],
        };
        let areas = vec![band(3.0, "0.05", "#8B4726", 1), band(0.0, "0.15", "#F2D21E", 2), band(-0.6, "0.30", "#E8473B", 3)];
        data.spc = vec![crate::data::SpcDay {
            day: 1,
            label: "Day 1".into(),
            categorical: vec![band(3.0, "TSTM", "#C1E9C1", 1), band(0.4, "MRGL", "#66A366", 2), band(-0.5, "SLGT", "#F6F67B", 3)],
            probabilities: ["tornado", "wind", "hail"].iter().map(|k| crate::data::SpcHazard { key: k.to_string(), label: k.to_string(), areas: areas.clone() }).collect(),
        }];
        data.ero[0] = vec![band(0.2, "MRGL", "#5CB85C", 1)];
        // NOAA normals for home (holiday cards past the 7-day forecast use them).
        data.climate.normals = Some(crate::climate::DayNormals {
            hi: vec![51.0; 366],
            lo: vec![33.0; 366],
            rec_hi: vec![(74.0, 1999); 366],
            rec_lo: vec![(8.0, 1950); 366],
            source: crate::climate::NOAA_NORMALS.into(),
            station: "PATOKA LAKE, IN US".into(),
            ..Default::default()
        });
        // CWX_QA_NIGHT=1: an NWS forecast that starts with Tonight (no high), as at night.
        if std::env::var("CWX_QA_NIGHT").is_ok() {
            let p = |name: &str, day: bool, t: f64| crate::data::Period { name: name.into(), is_daytime: day, temperature: Some(t), short_forecast: "Mostly Clear".into(), ..Default::default() };
            let mut forecast = vec![p("Tonight", false, 35.0)];
            for (i, d) in ["Saturday", "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday"].iter().enumerate() {
                forecast.push(p(d, true, 55.0 + i as f64));
                forecast.push(p(&format!("{d} Night"), false, 38.0 + i as f64));
            }
            data.overview = Some(crate::data::Overview { forecast, ..Default::default() });
        }
        // A storm line west of home moving east at 60 km/h (timing map).
        for dlat in [-0.5, -0.2, 0.1, 0.4] {
            data.cells.push(crate::data::StormCell {
                lat: home.lat + dlat,
                lon: home.lon - 0.9,
                intensity: crate::data::CellIntensity { level: 3, ..Default::default() },
                motion: Some(crate::data::CellMotion { speed_kmh: 60.0, speed_mph: 37.0, heading_deg: 85.0, ..Default::default() }),
                ..Default::default()
            });
        }
        crate::gfx::riskmap::remember(&data, &settings);
        // State outlines come from the app's boundary cache, loaded at start in the app.
        let _ = tokio::runtime::Runtime::new().map(|rt| rt.block_on(crate::map::boundaries::ensure_loaded()));
        let keys: Vec<String> = match std::env::var("CWX_QA_KEYS") { Ok(k) => k.split(',').map(String::from).collect(), Err(_) => ["severe_day", "lightning_leader", "edu_radar", "edu_ref_vel", "edu_cc", "edu_supercell", "demo_tornado_life", "edu_hail", "sci_coriolis"].iter().map(|s| s.to_string()).collect() };
        for key in &keys {
            let key = key.as_str();
            for look in ["modern", "classic"] {
                let doc = crate::gfx::templates::build_in(key, look).expect(key);
                let pm = crate::program::render_graphic_mode(&doc, &settings, &data, 10_000., 960, 540, None, crate::gfx::check::PreviewData::Sample);
                pm.save_png(dir.join(format!("{key}-{look}.png"))).unwrap();
            }
        }
    }

    /// The station logo in place of the station name (CWX_QA_NEW_DIR set, and CWX_QA_LOGO the
    /// path of a logo image; the media folder is a scratch copy, never the real library).
    #[test]
    fn qa_station_logo_on_graphics() {
        let (Some(dir), Some(logo)) = (std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from), std::env::var_os("CWX_QA_LOGO").map(std::path::PathBuf::from)) else { return };
        let media = dir.join("logo-media");
        std::fs::create_dir_all(&media).unwrap();
        std::fs::copy(&logo, media.join("station-logo.png")).unwrap();
        let mut settings = crate::config::Settings::default();
        settings.brand.logo_media = Some("station-logo.png".into());
        let data = crate::data::WxSnapshot::default();
        let live = crate::gfx::bind::Bindings::build(&data, &settings, None, &settings.radar_palette);
        let bindings = crate::gfx::check::preview_bindings(&live, crate::gfx::check::PreviewData::Sample);
        for (key, look) in [("alert_day", "modern"), ("seven_day", "modern"), ("seven_day", "classic")] {
            let Some(doc) = crate::gfx::templates::build_in(key, look) else { continue };
            let mut pm = tiny_skia::Pixmap::new(960, 540).unwrap();
            let ctx = crate::gfx::render::RenderCtx { bindings: &bindings, brand: &settings.brand, time: 10_000.0, out_time: None, media_dir: &media };
            crate::gfx::render::GraphicsRenderer::new().render(&mut pm, &doc, &ctx);
            pm.save_png(dir.join(format!("logo-{key}-{look}.png"))).unwrap();
        }
    }

    #[test]
    fn modern_and_back_restores_the_frame() {
        let original = crate::gfx::templates::build_classic("seven_day").expect("template");
        let mut doc = original.clone();
        apply(&mut doc, "modern");
        assert_eq!(doc.look, "modern");
        let header = doc.layers.iter().find(|l| l.name == "Header").expect("header");
        assert!(matches!(header.kind, LayerKind::Rect { skew, .. } if skew > 0.1));
        apply(&mut doc, "classic");
        for (a, b) in original.layers.iter().zip(doc.layers.iter()) {
            assert_eq!((a.x, a.y, a.w, a.h), (b.x, b.y, b.w, b.h), "{}", a.name);
        }
    }
}
