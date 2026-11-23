//! Generated forecast backgrounds (sunny, cloudy, rain, storm, snow, night) written to the media
//! library once, plus the binding that picks one for the current sky: an Image layer with media
//! `{{sky_image}}` (or `{{day0.sky_image}}`) follows the weather.

use tiny_skia::{Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point, RadialGradient, Rect, SpreadMode, Transform};

/// (file, top colour, bottom colour, accent) for each sky.
const SKIES: &[(&str, (u8, u8, u8), (u8, u8, u8), (u8, u8, u8, u8))] = &[
    ("bg_sunny.png", (40, 120, 210), (150, 205, 245), (255, 230, 140, 150)),
    ("bg_cloudy.png", (70, 84, 104), (150, 160, 175), (230, 235, 240, 70)),
    ("bg_rain.png", (28, 40, 58), (70, 88, 110), (120, 160, 210, 60)),
    ("bg_storm.png", (16, 14, 30), (60, 40, 70), (190, 170, 255, 80)),
    ("bg_snow.png", (120, 140, 170), (215, 225, 238), (255, 255, 255, 90)),
    ("bg_night.png", (6, 10, 28), (30, 40, 80), (220, 230, 255, 60)),
];

fn sky(top: (u8, u8, u8), bottom: (u8, u8, u8), accent: (u8, u8, u8, u8), seed: u32) -> Option<Pixmap> {
    let (w, h) = (1920.0f32, 1080.0f32);
    let mut pm = Pixmap::new(w as u32, h as u32)?;
    let rect = Rect::from_xywh(0.0, 0.0, w, h)?;
    let mut paint = Paint::default();
    paint.shader = LinearGradient::new(
        Point::from_xy(0.0, 0.0),
        Point::from_xy(0.0, h),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(top.0, top.1, top.2, 255)),
            GradientStop::new(1.0, Color::from_rgba8(bottom.0, bottom.1, bottom.2, 255)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    )?;
    pm.fill_rect(rect, &paint, Transform::identity(), None);
    // Soft accent blobs (sun glow, cloud banks, stars) placed by a small deterministic hash.
    let mut x = seed;
    let mut rnd = || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        (x % 10_000) as f32 / 10_000.0
    };
    for i in 0..14 {
        let (cx, cy) = (rnd() * w, rnd() * h * 0.7);
        let r = 120.0 + rnd() * 380.0;
        let a = accent.3 as f32 / 255.0 * (0.4 + 0.6 * rnd());
        let mut p = Paint::default();
        p.anti_alias = true;
        p.shader = RadialGradient::new(
            Point::from_xy(cx, cy),
            0.0,
            Point::from_xy(cx, cy),
            r,
            vec![
                GradientStop::new(0.0, Color::from_rgba(accent.0 as f32 / 255.0, accent.1 as f32 / 255.0, accent.2 as f32 / 255.0, a).unwrap_or(Color::WHITE)),
                GradientStop::new(1.0, Color::from_rgba(0.0, 0.0, 0.0, 0.0).unwrap_or(Color::TRANSPARENT)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )?;
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, r);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
        }
        if i > 9 && seed % 7 == 5 {
            break;
        }
    }
    Some(pm)
}

/// Write any missing background into the media library.
pub fn ensure() {
    let dir = crate::config::LibraryPaths::get().media;
    let _ = std::fs::create_dir_all(&dir);
    for (i, (file, top, bottom, accent)) in SKIES.iter().enumerate() {
        let path = dir.join(file);
        if path.exists() {
            continue;
        }
        if let Some(pm) = sky(*top, *bottom, *accent, 2_463_534_242u32.wrapping_add(i as u32 * 7919)) {
            if let Some(png) = crate::ops::pixmap_png(&pm) {
                let _ = std::fs::write(&path, png);
            }
        }
    }
}

/// Background file for a condition ("Thunderstorms", "Light Snow"…) and day / night.
pub fn for_condition(condition: &str, night: bool) -> &'static str {
    let c = condition.to_lowercase();
    if c.contains("thunder") || c.contains("storm") {
        "bg_storm.png"
    } else if c.contains("snow") || c.contains("sleet") || c.contains("ice") || c.contains("freezing") || c.contains("wintry") {
        "bg_snow.png"
    } else if c.contains("rain") || c.contains("shower") || c.contains("drizzle") {
        "bg_rain.png"
    } else if night {
        "bg_night.png"
    } else if c.contains("cloud") || c.contains("overcast") || c.contains("fog") {
        "bg_cloudy.png"
    } else {
        "bg_sunny.png"
    }
}
