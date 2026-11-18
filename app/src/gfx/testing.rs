//! Graphic tests (Graphics Studio > Test this graphic): render snapshots compared with a saved
//! baseline, layout regression against the baseline's layer boxes, a low-resolution legibility
//! check, animation timing, export compatibility, and template / data checks.

use std::time::Instant;

use crate::{
    config::{LibraryPaths, Settings},
    data::WxSnapshot,
    gfx::{
        check::PreviewData,
        model::{GraphicDoc, LayerKind},
    },
    program::render_graphic_mode,
};

pub struct TestResult {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

fn snap_dir() -> std::path::PathBuf {
    LibraryPaths::get().graphics.join(".snapshots")
}

fn snap_paths(doc: &GraphicDoc) -> (std::path::PathBuf, std::path::PathBuf) {
    let id = doc.id.replace(['/', '\\', '.'], "_");
    (snap_dir().join(format!("{id}.png")), snap_dir().join(format!("{id}.json")))
}

const SNAP_W: u32 = 960;
const SNAP_H: u32 = 540;

fn render_snapshot(doc: &GraphicDoc, settings: &Settings, data: &WxSnapshot) -> tiny_skia::Pixmap {
    // Sample data so the snapshot does not change with the weather.
    render_graphic_mode(doc, settings, data, (doc.intro_length() + 0.5), SNAP_W, SNAP_H, None, PreviewData::Sample)
}

/// Save the current render and layer boxes as the baseline for future comparisons.
pub fn save_baseline(doc: &GraphicDoc, settings: &Settings, data: &WxSnapshot) -> anyhow::Result<()> {
    std::fs::create_dir_all(snap_dir())?;
    let (png, json) = snap_paths(doc);
    render_snapshot(doc, settings, data).save_png(png)?;
    let boxes: Vec<(String, [f32; 4])> = doc.layers.iter().map(|l| (l.name.clone(), [l.x, l.y, l.w, l.h])).collect();
    std::fs::write(json, serde_json::to_string(&boxes)?)?;
    Ok(())
}

pub fn run(doc: &GraphicDoc, settings: &Settings, data: &WxSnapshot) -> Vec<TestResult> {
    let mut out = Vec::new();
    let (png, json) = snap_paths(doc);

    // ---- Render snapshot ----
    let now = render_snapshot(doc, settings, data);
    match tiny_skia::Pixmap::load_png(&png) {
        Ok(base) if base.width() == now.width() && base.height() == now.height() => {
            let total: u64 = base.data().iter().zip(now.data()).map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs() as u64).sum();
            let pct = total as f64 / (base.data().len() as f64 * 255.0) * 100.0;
            out.push(TestResult { name: "Render snapshot", ok: pct < 2.0, detail: format!("{pct:.2}% different from the baseline") });
        }
        Ok(_) => out.push(TestResult { name: "Render snapshot", ok: false, detail: "Baseline has a different size: save a new baseline".into() }),
        Err(_) => out.push(TestResult { name: "Render snapshot", ok: true, detail: "No baseline yet: press Save baseline".into() }),
    }

    // ---- Layout regression ----
    match std::fs::read_to_string(&json).ok().and_then(|t| serde_json::from_str::<Vec<(String, [f32; 4])>>(&t).ok()) {
        Some(boxes) => {
            let mut moved = Vec::new();
            for l in &doc.layers {
                if let Some((_, b)) = boxes.iter().find(|(n, _)| *n == l.name) {
                    let d = (l.x - b[0]).abs() + (l.y - b[1]).abs() + (l.w - b[2]).abs() + (l.h - b[3]).abs();
                    if d > 4.0 {
                        moved.push(l.name.clone());
                    }
                }
            }
            let missing: Vec<String> = boxes.iter().filter(|(n, _)| !doc.layers.iter().any(|l| &l.name == n)).map(|(n, _)| n.clone()).collect();
            let ok = moved.is_empty() && missing.is_empty();
            out.push(TestResult {
                name: "Layout regression",
                ok,
                detail: if ok {
                    "Every layer is where the baseline has it".into()
                } else {
                    format!("Moved: {} • Removed: {}", if moved.is_empty() { "none".into() } else { moved.join(", ") }, if missing.is_empty() { "none".into() } else { missing.join(", ") })
                },
            });
        }
        None => out.push(TestResult { name: "Layout regression", ok: true, detail: "No baseline yet".into() }),
    }

    // ---- Low-resolution legibility (a phone-size 480 px wide preview) ----
    let scale = 480.0 / doc.width.max(1) as f32;
    let tiny: Vec<String> = doc
        .layers
        .iter()
        .filter(|l| l.visible)
        .filter_map(|l| match &l.kind {
            LayerKind::Text { size, .. } if size * scale * crate::gfx::text_scale() < 9.0 => Some(l.name.clone()),
            LayerKind::Ticker { size, .. } if size * scale < 9.0 => Some(l.name.clone()),
            _ => None,
        })
        .collect();
    let _ = render_graphic_mode(doc, settings, data, (doc.intro_length() + 0.5), 480, (480.0 * doc.height as f32 / doc.width.max(1) as f32) as u32, None, PreviewData::Sample);
    out.push(TestResult {
        name: "Low-resolution preview",
        ok: tiny.is_empty(),
        detail: if tiny.is_empty() { "All text stays readable at 480 px wide".into() } else { format!("Too small on phones: {}", tiny.join(", ")) },
    });

    // ---- Animation performance (30 frames through the intro at full size) ----
    let settled = (doc.intro_length() + 0.5).max(0.5);
    let mut worst = 0.0f32;
    let mut total = 0.0f32;
    for i in 0..30 {
        let t = settled * i as f32 / 29.0;
        let started = Instant::now();
        let _ = render_graphic_mode(doc, settings, data, t, crate::config::FRAME_W, crate::config::FRAME_H, None, PreviewData::Sample);
        let ms = started.elapsed().as_secs_f32() * 1000.0;
        worst = worst.max(ms);
        total += ms;
    }
    out.push(TestResult {
        name: "Animation performance",
        ok: worst < 33.0,
        detail: format!("{:.1} ms per frame on average, worst {worst:.1} ms (30 fps needs under 33 ms)", total / 30.0),
    });

    // ---- Export compatibility ----
    let mut problems = Vec::new();
    if crate::ops::pixmap_png(&now).is_none() {
        problems.push("PNG encoding failed".to_string());
    }
    if doc.width % 2 == 1 || doc.height % 2 == 1 {
        problems.push("MP4 needs even width and height".to_string());
    }
    let ffmpeg = std::process::Command::new("ffmpeg").arg("-version").stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    out.push(TestResult {
        name: "Export compatibility",
        ok: problems.is_empty(),
        detail: format!(
            "{}{}",
            if problems.is_empty() { "PNG, JPEG and MP4 are fine".to_string() } else { problems.join("; ") },
            if ffmpeg { "; WebM available" } else { "; WebM needs ffmpeg installed" }
        ),
    });

    // ---- Template and data checks (outage and long values) ----
    let live = crate::gfx::bind::Bindings::build(data, settings, None, &settings.radar_palette);
    let mut serious = 0;
    for mode in [PreviewData::Missing, PreviewData::Long] {
        let b = crate::gfx::check::preview_bindings(&live, mode);
        serious += crate::gfx::check::check_doc(doc, &b, &settings.brand, &LibraryPaths::get().media).iter().filter(|i| i.serious).count();
    }
    let compat = crate::collections::validate(doc, &live);
    out.push(TestResult {
        name: "Template compatibility",
        ok: serious == 0 && compat.is_empty(),
        detail: format!("{serious} serious issue(s) with missing or long data; {}", if compat.is_empty() { "compatible with this version".to_string() } else { compat.join("; ") }),
    });
    out
}
