//! Vector weather icons in a glossy broadcast style, drawn with tiny-skia.

use tiny_skia::{
    BlendMode, Color, FillRule, GradientStop, LineCap, LinearGradient, Paint, PathBuilder, Pixmap,
    PixmapPaint, Point, Shader, SpreadMode, Stroke, Transform,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconKind {
    Clear,
    MostlyClear,
    PartlyCloudy,
    MostlyCloudy,
    Cloudy,
    Showers,
    Rain,
    Thunder,
    Snow,
    Mix,
    Fog,
    Wind,
    Hot,
    Cold,
}

impl IconKind {
    pub const ALL: [IconKind; 14] = [
        IconKind::Clear,
        IconKind::MostlyClear,
        IconKind::PartlyCloudy,
        IconKind::MostlyCloudy,
        IconKind::Cloudy,
        IconKind::Showers,
        IconKind::Rain,
        IconKind::Thunder,
        IconKind::Snow,
        IconKind::Mix,
        IconKind::Fog,
        IconKind::Wind,
        IconKind::Hot,
        IconKind::Cold,
    ];

    pub fn label(self) -> &'static str {
        match self {
            IconKind::Clear => "Clear",
            IconKind::MostlyClear => "Mostly Clear",
            IconKind::PartlyCloudy => "Partly Cloudy",
            IconKind::MostlyCloudy => "Mostly Cloudy",
            IconKind::Cloudy => "Cloudy",
            IconKind::Showers => "Showers",
            IconKind::Rain => "Rain",
            IconKind::Thunder => "Thunderstorms",
            IconKind::Snow => "Snow",
            IconKind::Mix => "Wintry Mix",
            IconKind::Fog => "Fog",
            IconKind::Wind => "Windy",
            IconKind::Hot => "Hot",
            IconKind::Cold => "Cold",
        }
    }

    /// Map NWS forecast wording ("Chance Showers And Thunderstorms", "Patchy Fog", …).
    pub fn from_condition(text: &str) -> IconKind {
        let t = text.to_lowercase();
        if t.contains("thunder") || t.contains("t-storm") || t.contains("tstorm") {
            IconKind::Thunder
        } else if t.contains("sleet") || t.contains("freezing") || t.contains("mix") || t.contains("ice") {
            IconKind::Mix
        } else if t.contains("snow") || t.contains("flurr") || t.contains("blizzard") {
            IconKind::Snow
        } else if t.contains("shower") || t.contains("drizzle") || t.contains("sprinkle") {
            IconKind::Showers
        } else if t.contains("rain") {
            IconKind::Rain
        } else if t.contains("fog") || t.contains("haze") || t.contains("smoke") || t.contains("mist") {
            IconKind::Fog
        } else if t.contains("wind") || t.contains("breezy") || t.contains("blustery") {
            IconKind::Wind
        } else if t.contains("hot") {
            IconKind::Hot
        } else if t.contains("cold") || t.contains("frost") {
            IconKind::Cold
        } else if t.contains("mostly cloudy") || t.contains("considerable") {
            IconKind::MostlyCloudy
        } else if t.contains("partly") {
            IconKind::PartlyCloudy
        } else if t.contains("mostly sunny") || t.contains("mostly clear") {
            IconKind::MostlyClear
        } else if t.contains("cloudy") || t.contains("overcast") {
            IconKind::Cloudy
        } else if t.contains("sunny") || t.contains("clear") || t.contains("fair") {
            IconKind::Clear
        } else {
            IconKind::PartlyCloudy
        }
    }
}

fn solid(color: Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    paint
}

fn vertical_gradient(top: Color, bottom: Color, y0: f32, y1: f32) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.shader = LinearGradient::new(
        Point::from_xy(0.0, y0),
        Point::from_xy(0.0, y1),
        vec![GradientStop::new(0.0, top), GradientStop::new(1.0, bottom)],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(top));
    paint
}

fn circle(pb: &mut PathBuilder, cx: f32, cy: f32, r: f32) {
    pb.push_circle(cx, cy, r);
}

fn draw_sun(pm: &mut Pixmap, cx: f32, cy: f32, r: f32) {
    let rays = Stroke {
        width: r * 0.16,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    let mut pb = PathBuilder::new();
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::PI / 4.0;
        pb.move_to(cx + a.cos() * r * 1.28, cy + a.sin() * r * 1.28);
        pb.line_to(cx + a.cos() * r * 1.62, cy + a.sin() * r * 1.62);
    }
    if let Some(path) = pb.finish() {
        pm.stroke_path(&path, &solid(Color::from_rgba8(255, 196, 41, 255)), &rays, Transform::identity(), None);
    }
    let mut pb = PathBuilder::new();
    circle(&mut pb, cx, cy, r);
    if let Some(path) = pb.finish() {
        let paint = vertical_gradient(
            Color::from_rgba8(255, 230, 90, 255),
            Color::from_rgba8(255, 150, 20, 255),
            cy - r,
            cy + r,
        );
        pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_moon(pm: &mut Pixmap, cx: f32, cy: f32, r: f32) {
    // Crescent: full disc, then clear an offset disc on a scratch layer.
    let size = (r * 2.6).ceil() as u32 + 4;
    let Some(mut layer) = Pixmap::new(size, size) else {
        return;
    };
    let c = size as f32 / 2.0;
    let mut pb = PathBuilder::new();
    circle(&mut pb, c, c, r);
    if let Some(path) = pb.finish() {
        let paint = vertical_gradient(
            Color::from_rgba8(250, 246, 220, 255),
            Color::from_rgba8(214, 206, 170, 255),
            c - r,
            c + r,
        );
        layer.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
    let mut pb = PathBuilder::new();
    circle(&mut pb, c + r * 0.55, c - r * 0.35, r * 0.88);
    if let Some(path) = pb.finish() {
        let mut clear = solid(Color::BLACK);
        clear.blend_mode = BlendMode::Clear;
        layer.fill_path(&path, &clear, FillRule::Winding, Transform::identity(), None);
    }
    pm.draw_pixmap(
        (cx - c) as i32,
        (cy - c) as i32,
        layer.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

fn draw_cloud(pm: &mut Pixmap, x: f32, y: f32, w: f32, dark: bool) {
    // Cloud body from overlapping circles + a flat base, with a soft gradient.
    let h = w * 0.55;
    let mut pb = PathBuilder::new();
    circle(&mut pb, x + w * 0.28, y + h * 0.62, h * 0.36);
    circle(&mut pb, x + w * 0.50, y + h * 0.42, h * 0.48);
    circle(&mut pb, x + w * 0.74, y + h * 0.60, h * 0.38);
    if let Some(rect) = tiny_skia::Rect::from_xywh(x + w * 0.2, y + h * 0.55, w * 0.62, h * 0.43) {
        pb.push_rect(rect);
    }
    if let Some(path) = pb.finish() {
        let (top, bottom) = if dark {
            (Color::from_rgba8(170, 178, 192, 255), Color::from_rgba8(98, 106, 122, 255))
        } else {
            (Color::from_rgba8(255, 255, 255, 255), Color::from_rgba8(196, 206, 220, 255))
        };
        let paint = vertical_gradient(top, bottom, y, y + h);
        pm.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_drops(pm: &mut Pixmap, x: f32, y: f32, w: f32, count: usize, color: Color) {
    let stroke = Stroke {
        width: w * 0.055,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    let mut pb = PathBuilder::new();
    for i in 0..count {
        let dx = x + w * (0.28 + i as f32 * 0.2);
        pb.move_to(dx, y);
        pb.line_to(dx - w * 0.07, y + w * 0.2);
    }
    if let Some(path) = pb.finish() {
        pm.stroke_path(&path, &solid(color), &stroke, Transform::identity(), None);
    }
}

fn draw_flakes(pm: &mut Pixmap, x: f32, y: f32, w: f32, count: usize) {
    let stroke = Stroke {
        width: w * 0.035,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    let mut pb = PathBuilder::new();
    for i in 0..count {
        let cx = x + w * (0.28 + i as f32 * 0.2);
        let cy = y + w * (0.08 + (i % 2) as f32 * 0.1);
        let r = w * 0.06;
        for k in 0..3 {
            let a = k as f32 * std::f32::consts::PI / 3.0;
            pb.move_to(cx - a.cos() * r, cy - a.sin() * r);
            pb.line_to(cx + a.cos() * r, cy + a.sin() * r);
        }
    }
    if let Some(path) = pb.finish() {
        pm.stroke_path(&path, &solid(Color::WHITE), &stroke, Transform::identity(), None);
    }
}

fn draw_bolt(pm: &mut Pixmap, x: f32, y: f32, w: f32) {
    let mut pb = PathBuilder::new();
    pb.move_to(x + w * 0.52, y);
    pb.line_to(x + w * 0.36, y + w * 0.2);
    pb.line_to(x + w * 0.48, y + w * 0.2);
    pb.line_to(x + w * 0.38, y + w * 0.38);
    pb.line_to(x + w * 0.62, y + w * 0.14);
    pb.line_to(x + w * 0.50, y + w * 0.14);
    pb.line_to(x + w * 0.60, y);
    pb.close();
    if let Some(path) = pb.finish() {
        pm.fill_path(
            &path,
            &solid(Color::from_rgba8(255, 214, 10, 255)),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn draw_lines(pm: &mut Pixmap, x: f32, y: f32, w: f32, rows: usize, color: Color, wavy: bool) {
    let stroke = Stroke {
        width: w * 0.05,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    let mut pb = PathBuilder::new();
    for i in 0..rows {
        let ly = y + i as f32 * w * 0.13;
        let inset = if i % 2 == 0 { 0.0 } else { w * 0.08 };
        pb.move_to(x + inset, ly);
        if wavy {
            pb.cubic_to(
                x + w * 0.3,
                ly - w * 0.08,
                x + w * 0.5,
                ly + w * 0.08,
                x + w * 0.8 - inset,
                ly,
            );
        } else {
            pb.line_to(x + w * 0.85 - inset, ly);
        }
    }
    if let Some(path) = pb.finish() {
        pm.stroke_path(&path, &solid(color), &stroke, Transform::identity(), None);
    }
}

fn draw_thermometer(pm: &mut Pixmap, x: f32, y: f32, w: f32, hot: bool) {
    let color = if hot {
        Color::from_rgba8(255, 80, 60, 255)
    } else {
        Color::from_rgba8(90, 170, 255, 255)
    };
    let cx = x + w * 0.5;
    let mut pb = PathBuilder::new();
    if let Some(rect) = tiny_skia::Rect::from_xywh(cx - w * 0.08, y + w * 0.1, w * 0.16, w * 0.55) {
        pb.push_rect(rect);
    }
    circle(&mut pb, cx, y + w * 0.75, w * 0.16);
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &solid(Color::WHITE), FillRule::Winding, Transform::identity(), None);
    }
    let mut pb = PathBuilder::new();
    let fill_top = if hot { y + w * 0.18 } else { y + w * 0.5 };
    if let Some(rect) = tiny_skia::Rect::from_xywh(cx - w * 0.04, fill_top, w * 0.08, y + w * 0.7 - fill_top) {
        pb.push_rect(rect);
    }
    circle(&mut pb, cx, y + w * 0.75, w * 0.11);
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &solid(color), FillRule::Winding, Transform::identity(), None);
    }
}

/// Draw `kind` into the square (x, y, size). `day` picks sun vs moon.
pub fn draw_icon(pm: &mut Pixmap, kind: IconKind, x: f32, y: f32, size: f32, day: bool) {
    let s = size;
    let celestial = |pm: &mut Pixmap, cx: f32, cy: f32, r: f32| {
        if day {
            draw_sun(pm, cx, cy, r);
        } else {
            draw_moon(pm, cx, cy, r);
        }
    };
    let rain = Color::from_rgba8(80, 170, 255, 255);
    match kind {
        IconKind::Clear => celestial(pm, x + s * 0.5, y + s * 0.5, s * 0.28),
        IconKind::MostlyClear => {
            celestial(pm, x + s * 0.45, y + s * 0.45, s * 0.26);
            draw_cloud(pm, x + s * 0.42, y + s * 0.55, s * 0.5, false);
        }
        IconKind::PartlyCloudy => {
            celestial(pm, x + s * 0.38, y + s * 0.36, s * 0.22);
            draw_cloud(pm, x + s * 0.15, y + s * 0.38, s * 0.8, false);
        }
        IconKind::MostlyCloudy => {
            celestial(pm, x + s * 0.3, y + s * 0.3, s * 0.18);
            draw_cloud(pm, x + s * 0.05, y + s * 0.3, s * 0.92, false);
        }
        IconKind::Cloudy => {
            draw_cloud(pm, x + s * 0.25, y + s * 0.18, s * 0.7, true);
            draw_cloud(pm, x + s * 0.02, y + s * 0.32, s * 0.9, false);
        }
        IconKind::Showers => {
            celestial(pm, x + s * 0.32, y + s * 0.28, s * 0.18);
            draw_cloud(pm, x + s * 0.1, y + s * 0.2, s * 0.85, false);
            draw_drops(pm, x + s * 0.1, y + s * 0.72, s * 0.85, 3, rain);
        }
        IconKind::Rain => {
            draw_cloud(pm, x + s * 0.05, y + s * 0.12, s * 0.9, true);
            draw_drops(pm, x + s * 0.05, y + s * 0.66, s * 0.9, 4, rain);
        }
        IconKind::Thunder => {
            draw_cloud(pm, x + s * 0.05, y + s * 0.1, s * 0.9, true);
            draw_bolt(pm, x + s * 0.12, y + s * 0.56, s * 0.8);
            draw_drops(pm, x - s * 0.08, y + s * 0.66, s * 0.6, 2, rain);
        }
        IconKind::Snow => {
            draw_cloud(pm, x + s * 0.05, y + s * 0.12, s * 0.9, false);
            draw_flakes(pm, x + s * 0.05, y + s * 0.68, s * 0.9, 4);
        }
        IconKind::Mix => {
            draw_cloud(pm, x + s * 0.05, y + s * 0.12, s * 0.9, true);
            draw_drops(pm, x + s * 0.0, y + s * 0.66, s * 0.6, 2, rain);
            draw_flakes(pm, x + s * 0.42, y + s * 0.68, s * 0.6, 2);
        }
        IconKind::Fog => {
            draw_cloud(pm, x + s * 0.1, y + s * 0.08, s * 0.8, false);
            draw_lines(pm, x + s * 0.12, y + s * 0.62, s * 0.85, 3, Color::from_rgba8(210, 216, 226, 255), false);
        }
        IconKind::Wind => {
            draw_lines(pm, x + s * 0.08, y + s * 0.3, s * 0.9, 4, Color::from_rgba8(225, 235, 245, 255), true);
        }
        IconKind::Hot => {
            draw_sun(pm, x + s * 0.35, y + s * 0.4, s * 0.2);
            draw_thermometer(pm, x + s * 0.45, y + s * 0.15, s * 0.6, true);
        }
        IconKind::Cold => {
            draw_flakes(pm, x + s * 0.0, y + s * 0.25, s * 0.6, 2);
            draw_thermometer(pm, x + s * 0.4, y + s * 0.12, s * 0.6, false);
        }
    }
}
