//! Program renders a graphic frame after frame with one long-lived renderer. These tests drive
//! the renderer the same way (one instance, 30 fps clock) so on-air-only defects show up.

use super::{bind::Bindings, model::GraphicDoc, render::{GraphicsRenderer, RenderCtx}};
use crate::{config::{Settings, FRAME_H, FRAME_W}, data::WxSnapshot};
use tiny_skia::{Color, Pixmap};

fn program_frames(doc: &GraphicDoc, times: &[f32]) -> Vec<Pixmap> {
    let settings = Settings::default();
    let data = WxSnapshot::default();
    let mut bindings = Bindings::build(&data, &settings, None, &settings.radar_palette);
    bindings.set("condition", "Clear");
    bindings.set("daypart", "day");
    bindings.set("temp", "52°");
    let media = std::env::temp_dir();
    let mut renderer = GraphicsRenderer::new();
    times
        .iter()
        .map(|&time| {
            let mut pm = Pixmap::new(FRAME_W, FRAME_H).unwrap();
            pm.fill(Color::BLACK);
            let ctx = RenderCtx { bindings: &bindings, brand: &settings.brand, time, out_time: None, media_dir: &media };
            renderer.render(&mut pm, doc, &ctx);
            pm
        })
        .collect()
}

/// Settled Program frames must not depend on the frames rendered before them.
#[test]
fn settled_program_frame_matches_a_fresh_render() {
    let doc = super::templates::build("current_conditions")
        .or_else(|| super::templates::TEMPLATES.iter().find(|t| t.name == "Current Conditions").and_then(|t| super::templates::build(t.key)))
        .expect("Current Conditions template");
    let mut times: Vec<f32> = (0..90).map(|i| i as f32 / 30.0).collect();
    times.push(10.0);
    let frames = program_frames(&doc, &times);
    let fresh = program_frames(&doc, &[10.0]).pop().unwrap();
    if let Some(dir) = std::env::var_os("CWX_QA_OUTPUT_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (i, t) in times.iter().enumerate().filter(|(i, _)| i % 6 == 0 || *i == times.len() - 1) {
            frames[i].save_png(dir.join(format!("program_{:05.2}.png", t))).unwrap();
        }
        fresh.save_png(dir.join("fresh_settled.png")).unwrap();
    }
    assert!(frames.last().unwrap().data() == fresh.data(), "settled Program frame differs from a fresh render");
}
