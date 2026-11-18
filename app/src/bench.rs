//! System performance benchmark (Settings > Performance > Run benchmark): times the work the
//! app does on air and writes a report to Exports.

use std::time::Instant;

use crate::{config::Settings, data::WxSnapshot};

pub struct BenchResult {
    pub name: &'static str,
    pub ms: f64,
    /// Plain-language verdict ("Good", "OK", "Slow: …").
    pub verdict: String,
}

fn time<T>(runs: u32, mut f: impl FnMut() -> T) -> f64 {
    let started = Instant::now();
    for _ in 0..runs {
        std::hint::black_box(f());
    }
    started.elapsed().as_secs_f64() * 1000.0 / runs as f64
}

fn verdict(ms: f64, good: f64, ok: f64, advice: &str) -> String {
    if ms <= good {
        "Good".into()
    } else if ms <= ok {
        "OK".into()
    } else {
        format!("Slow: {advice}")
    }
}

pub fn run(settings: &Settings, data: &WxSnapshot) -> Vec<BenchResult> {
    let mut out = Vec::new();
    let (w, h) = (crate::config::FRAME_W, crate::config::FRAME_H);

    // Full-screen graphic render (the heaviest template-style graphic).
    let doc = crate::gfx::templates::build("seven_day").or_else(|| crate::gfx::templates::build("current"));
    if let Some(doc) = doc {
        let t = doc.intro_length() + 0.5;
        let ms = time(10, || crate::program::render_graphic(&doc, settings, data, t, w, h, None));
        out.push(BenchResult { name: "Graphic render (1080p)", ms, verdict: verdict(ms, 16.0, 33.0, "turn on Low-resource mode") });
    }

    // Map overlays: draw every warning polygon (fill + outline) at 1080p.
    let ms = time(10, || {
        let mut pm = tiny_skia::Pixmap::new(w, h).expect("pixmap");
        let vp = crate::map::geo::Viewport::new(settings.home, w, h);
        crate::map::overlays::draw_alerts(&mut pm, &vp, &data.map_alerts, true, false);
        pm
    });
    out.push(BenchResult { name: "Warning polygons", ms, verdict: verdict(ms, 5.0, 12.0, "show fewer alert types (watches, advisories)") });

    // Text layout and drawing.
    let style = crate::gfx::text::TextStyle { family: settings.brand.font_body.clone(), weight: crate::gfx::text::Weight::SemiBold, size: 42.0, color: tiny_skia::Color::WHITE, letter_spacing: 0.0 };
    let mut cache = crate::gfx::text::GlyphCache::default();
    let ms = time(20, || {
        let mut pm = tiny_skia::Pixmap::new(1200, 400).expect("pixmap");
        crate::gfx::text::draw_text_box(&mut pm, &mut cache, "Severe thunderstorms with damaging winds and large hail are possible this afternoon and evening", (0.0, 0.0, 1200.0, 400.0), &style, crate::gfx::text::Align::Left, false, true, true, true, 1.0);
        pm
    });
    out.push(BenchResult { name: "Text layout", ms, verdict: verdict(ms, 4.0, 10.0, "fewer text layers with Shrink to fit") });

    // Image encoding (exports, control server, recording).
    let frame = tiny_skia::Pixmap::new(w, h).expect("pixmap");
    let ms = time(3, || crate::ops::pixmap_png(&frame));
    out.push(BenchResult { name: "PNG encode (1080p)", ms, verdict: verdict(ms, 60.0, 150.0, "OBS browser source: use /program.mjpg") });

    // Disk: write and read 16 MB in the exports folder.
    let dir = crate::library::exports_dir();
    let path = dir.join(".bench.tmp");
    let blob = vec![0x5Au8; 16 * 1024 * 1024];
    let started = Instant::now();
    let ok = std::fs::write(&path, &blob).is_ok() && std::fs::read(&path).map(|b| b.len() == blob.len()).unwrap_or(false);
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    let _ = std::fs::remove_file(&path);
    out.push(BenchResult { name: "Disk (16 MB write + read)", ms, verdict: if !ok { "Failed: check the Documents folder".into() } else { verdict(ms, 120.0, 400.0, "recordings may drop frames; record to an SSD") } });

    // Network: the station API round trip.
    let url = format!("{}/api/weather/spc-outlooks", settings.api_base.trim_end_matches('/'));
    let started = Instant::now();
    let net_ok = crate::net::runtime().block_on(async move { crate::net::get_logged(&url, std::time::Duration::from_secs(10)).await.is_ok() });
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    out.push(BenchResult { name: "Station API round trip", ms, verdict: if !net_ok { "Failed: check the connection (Ready for air?)".into() } else { verdict(ms, 400.0, 1500.0, "slow connection; radar loops will fill in slowly") } });
    out
}

/// Markdown report in Exports.
pub fn report(results: &[BenchResult]) -> anyhow::Result<std::path::PathBuf> {
    let mut md = format!("# Performance benchmark\n\n{}\n\n| Test | Time | Result |\n|---|---|---|\n", chrono::Local::now().format("%Y-%m-%d %H:%M"));
    for r in results {
        md.push_str(&format!("| {} | {:.1} ms | {} |\n", r.name, r.ms, r.verdict));
    }
    let path = crate::library::exports_dir().join(format!("Benchmark {}.md", chrono::Local::now().format("%Y-%m-%d %H-%M")));
    std::fs::write(&path, md)?;
    Ok(path)
}
