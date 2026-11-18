//! Animated 2D diagrams for explainer and safety graphics (LayerKind::Diagram). Each diagram is
//! drawn procedurally in the layer's own box (0..w, 0..h) and loops every few seconds, so the
//! same graphic works as a still or as an animation on air.

use tiny_skia::{Color, FillRule, Mask, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use super::text::{draw_line, measure, GlyphCache, TextStyle, Weight};

/// Every diagram, for the Graphics Studio picker: (key, label).
pub const DIAGRAMS: &[(&str, &str)] = &[
    ("supercell", "Supercell and tornado"),
    ("tornado_lifecycle", "Tornado life cycle"),
    ("rfd", "Rear-flank downdraft and hook"),
    ("tornado_vs_straight", "Tornado vs straight-line wind"),
    ("wall_cloud", "Wall cloud under the storm base"),
    ("funnel_cloud", "Funnel cloud vs tornado"),
    ("debris_cloud", "Tornado debris cloud"),
    ("hook_echo", "Hook echo on radar"),
    ("alert_terms", "Advisory, watch, warning"),
    ("radar_beam", "How radar works"),
    ("radar_velocity", "Reflectivity vs velocity"),
    ("radar_cc", "Correlation coefficient (debris)"),
    ("lightning_cg", "Cloud-to-ground lightning"),
    ("lightning_ic", "Intracloud lightning"),
    ("lightning_positive", "Positive vs negative lightning"),
    ("thunder_distance", "Thunder distance"),
    ("hail", "How hail grows"),
    ("fronts", "Cold and warm fronts"),
    ("precip_types", "Rain, sleet, freezing rain, snow"),
    ("shelter_house", "Where to shelter in a house"),
    ("shelter_mobile", "Mobile home: leave for a shelter"),
    ("downed_line", "Downed power lines: stay 35 feet away"),
    ("shelter_vehicle", "Caught in a vehicle"),
    ("shelter_school", "School and workplace sheltering"),
    ("shelter_outdoors", "Caught outdoors"),
    ("flood_depth", "Floodwater depth vs people and cars"),
    ("flash_flood", "Flash flood vs river flood"),
    ("urban_flood", "Street flooding"),
    ("low_water", "Low-water crossing"),
    ("storm_surge", "Storm surge"),
    ("hurricane", "Hurricane structure"),
    ("hurricane_categories", "Saffir-Simpson wind scale"),
    ("evac_timeline", "Hurricane evacuation timeline"),
    ("atmosphere", "Layers of the atmosphere"),
    ("jet_stream", "Jet stream"),
    ("high_low", "High and low pressure"),
    ("sea_breeze", "Sea breeze and land breeze"),
    ("inversion", "Temperature inversion"),
    ("orographic", "Mountains: rain and rain shadow"),
    ("instability", "Unstable air"),
    ("cloud_formation", "How clouds form"),
    ("coriolis", "Coriolis effect"),
    ("heat_body", "Heat illness"),
    ("car_heat", "Hot car"),
    ("cold_body", "Frostbite and hypothermia"),
    ("wet_bulb", "Wet-bulb: humid heat"),
    ("wind_chill", "Wind chill"),
    ("snow_city", "Snow piling up"),
    ("ice_lines", "Ice on power lines"),
    ("blizzard", "Blizzard whiteout"),
    ("lake_effect", "Lake-effect snow bands"),
    ("snow_squall", "Snow squall"),
    ("black_ice", "Black ice"),
    ("generator_co", "Generator safety"),
    ("power_line", "Downed power lines"),
    ("emergency_kit", "Emergency kit"),
    ("hodograph", "Hodograph (live sounding)"),
    ("skewt", "Sounding (live)"),
    ("wind_profile", "Wind profile, next 24 h (live)"),
];

struct Canvas<'a> {
    pm: &'a mut Pixmap,
    tf: Transform,
    mask: Option<&'a Mask>,
    w: f32,
    h: f32,
    t: f32,
    alpha: f32,
}

fn rgba(c: Color, a: f32) -> Paint<'static> {
    let mut p = Paint::default();
    let mut c = c;
    c.apply_opacity(a);
    p.set_color(c);
    p.anti_alias = true;
    p
}

fn col(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba8(r, g, b, 255)
}

impl Canvas<'_> {
    fn fill(&mut self, pb: PathBuilder, c: Color) {
        if let Some(p) = pb.finish() {
            self.pm.fill_path(&p, &rgba(c, self.alpha), FillRule::Winding, self.tf, self.mask);
        }
    }
    fn line(&mut self, pts: &[(f32, f32)], c: Color, width: f32) {
        let mut pb = PathBuilder::new();
        for (i, p) in pts.iter().enumerate() {
            if i == 0 {
                pb.move_to(p.0, p.1);
            } else {
                pb.line_to(p.0, p.1);
            }
        }
        if let Some(p) = pb.finish() {
            let st = Stroke { width, line_cap: tiny_skia::LineCap::Round, line_join: tiny_skia::LineJoin::Round, ..Stroke::default() };
            self.pm.stroke_path(&p, &rgba(c, self.alpha), &st, self.tf, self.mask);
        }
    }
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Color) {
        if let Some(r) = Rect::from_xywh(x, y, w.max(0.5), h.max(0.5)) {
            self.pm.fill_rect(r, &rgba(c, self.alpha), self.tf, self.mask);
        }
    }
    fn circle(&mut self, x: f32, y: f32, r: f32, c: Color) {
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, r.max(0.5));
        self.fill(pb, c);
    }
    fn poly(&mut self, pts: &[(f32, f32)], c: Color) {
        let mut pb = PathBuilder::new();
        for (i, p) in pts.iter().enumerate() {
            if i == 0 {
                pb.move_to(p.0, p.1);
            } else {
                pb.line_to(p.0, p.1);
            }
        }
        pb.close();
        self.fill(pb, c);
    }
    fn arrow(&mut self, from: (f32, f32), to: (f32, f32), c: Color, width: f32) {
        self.line(&[from, to], c, width);
        let a = (to.1 - from.1).atan2(to.0 - from.0);
        let head = width * 3.5;
        self.poly(&[to, (to.0 - head * (a - 0.5).cos(), to.1 - head * (a - 0.5).sin()), (to.0 - head * (a + 0.5).cos(), to.1 - head * (a + 0.5).sin())], c);
    }
    /// Vertical gradient rectangle (top colour to bottom colour).
    fn vgrad(&mut self, x: f32, y: f32, w: f32, h: f32, top: Color, bottom: Color) {
        let Some(r) = Rect::from_xywh(x, y, w.max(0.5), h.max(0.5)) else { return };
        let mut a = top;
        a.apply_opacity(self.alpha);
        let mut b = bottom;
        b.apply_opacity(self.alpha);
        let mut p = Paint::default();
        p.anti_alias = true;
        p.shader = tiny_skia::LinearGradient::new(
            tiny_skia::Point::from_xy(x, y),
            tiny_skia::Point::from_xy(x, y + h),
            vec![tiny_skia::GradientStop::new(0.0, a), tiny_skia::GradientStop::new(1.0, b)],
            tiny_skia::SpreadMode::Pad,
            Transform::identity(),
        )
        .unwrap_or(tiny_skia::Shader::SolidColor(a));
        self.pm.fill_rect(r, &p, self.tf, self.mask);
    }

    /// Puffy cloud: overlapping circles along a base, with a soft shadow underneath, a
    /// darker base and lit tops so it reads as a 3-D cloud.
    fn cloud(&mut self, x: f32, y: f32, w: f32, h: f32, c: Color) {
        let n = 6;
        let mix = |c: Color, t: Color, k: f32| Color::from_rgba(
            c.red() + (t.red() - c.red()) * k,
            c.green() + (t.green() - c.green()) * k,
            c.blue() + (t.blue() - c.blue()) * k,
            c.alpha(),
        ).unwrap_or(c);
        let shade = mix(c, Color::BLACK, 0.28);
        let light = mix(c, Color::WHITE, 0.12);
        // Soft shadow on whatever is behind.
        self.circle_alpha(x + w / 2.0, y + h * 0.3, w * 0.5, h * 0.18, Color::from_rgba8(0, 0, 0, 28));
        for i in 0..n {
            let fx = i as f32 / (n - 1) as f32;
            let r = h * (0.45 + 0.25 * (fx * std::f32::consts::PI).sin());
            self.circle(x + fx * w, y - r * 0.4, r, shade);
        }
        self.rect(x, y - h * 0.2, w, h * 0.4, shade);
        for i in 0..n {
            let fx = i as f32 / (n - 1) as f32;
            let r = h * (0.45 + 0.25 * (fx * std::f32::consts::PI).sin());
            self.circle(x + fx * w, y - r * 0.5, r * 0.9, c);
        }
        for i in 1..n - 1 {
            let fx = i as f32 / (n - 1) as f32;
            let r = h * (0.45 + 0.25 * (fx * std::f32::consts::PI).sin());
            self.circle(x + fx * w, y - r * 0.62, r * 0.72, light);
        }
    }

    /// Filled ellipse (shadows).
    fn circle_alpha(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Color) {
        let mut pb = PathBuilder::new();
        if let Some(r) = Rect::from_xywh(cx - rx, cy - ry, (rx * 2.0).max(1.0), (ry * 2.0).max(1.0)) {
            pb.push_oval(r);
        }
        self.fill(pb, c);
    }
    fn house(&mut self, x: f32, ground: f32, w: f32, h: f32, wall: Color, roof: Color) {
        self.rect(x, ground - h, w, h, wall);
        self.poly(&[(x - w * 0.08, ground - h), (x + w / 2.0, ground - h - h * 0.6), (x + w * 1.08, ground - h)], roof);
    }
    /// A standing person `size` tall: head, neck, shoulders, torso, two arms and two legs,
    /// with round joints so it reads clearly at any size.
    fn person(&mut self, x: f32, ground: f32, size: f32, c: Color) {
        self.person_pose(x, ground, size, c, 0.0);
    }

    /// Person with the arms raised by `raise` (0 = at the sides, 1 = straight up), for
    /// "wave for help" or "cover your head" poses.
    fn person_pose(&mut self, x: f32, ground: f32, size: f32, c: Color, raise: f32) {
        let s = size;
        let limb = (s * 0.075).max(2.0);
        let head_r = s * 0.085;
        let head_y = ground - s + head_r;
        let shoulder_y = ground - s * 0.76;
        let hip_y = ground - s * 0.44;
        let shoulder_w = s * 0.13;
        let hip_w = s * 0.075;
        // Head and neck.
        self.circle(x, head_y, head_r, c);
        self.line(&[(x, head_y + head_r * 0.8), (x, shoulder_y)], c, limb * 0.8);
        // Torso: shoulders taper to the hips.
        self.poly(
            &[
                (x - shoulder_w, shoulder_y),
                (x + shoulder_w, shoulder_y),
                (x + hip_w * 1.25, hip_y),
                (x - hip_w * 1.25, hip_y),
            ],
            c,
        );
        self.circle(x - shoulder_w + limb * 0.3, shoulder_y + limb * 0.3, limb * 0.6, c);
        self.circle(x + shoulder_w - limb * 0.3, shoulder_y + limb * 0.3, limb * 0.6, c);
        // Arms: upper arm and forearm, elbows slightly out.
        let raise = raise.clamp(0.0, 1.0);
        for side in [-1.0f32, 1.0] {
            let sx = x + side * (shoulder_w - limb * 0.3);
            let sy = shoulder_y + limb * 0.4;
            let (ex, ey, hx, hy) = if raise > 0.0 {
                let ex = sx + side * s * (0.12 - 0.04 * raise);
                let ey = sy - s * 0.02 - s * 0.16 * raise;
                (ex, ey, ex + side * s * 0.02 * (1.0 - raise), ey - s * 0.17 * raise + s * 0.15 * (1.0 - raise))
            } else {
                let ex = sx + side * s * 0.05;
                let ey = sy + s * 0.16;
                (ex, ey, ex + side * s * 0.01, ey + s * 0.15)
            };
            self.line(&[(sx, sy), (ex, ey), (hx, hy)], c, limb);
            self.circle(hx, hy, limb * 0.62, c);
        }
        // Legs.
        for side in [-1.0f32, 1.0] {
            let hx = x + side * hip_w * 0.8;
            let kx = hx + side * s * 0.01;
            let ky = hip_y + s * 0.22;
            let fx = hx + side * s * 0.02;
            self.line(&[(hx, hip_y), (kx, ky), (fx, ground - limb * 0.5)], c, limb * 1.15);
            // Feet point forward.
            self.line(&[(fx, ground - limb * 0.45), (fx + side * s * 0.05, ground - limb * 0.45)], c, limb);
        }
    }
    fn car(&mut self, x: f32, ground: f32, w: f32, c: Color) {
        let h = w * 0.32;
        self.rect(x, ground - h * 0.85, w, h * 0.55, c);
        self.poly(&[(x + w * 0.2, ground - h * 0.85), (x + w * 0.32, ground - h * 1.35), (x + w * 0.7, ground - h * 1.35), (x + w * 0.82, ground - h * 0.85)], c);
        self.circle(x + w * 0.22, ground - h * 0.25, h * 0.25, col(30, 30, 34));
        self.circle(x + w * 0.78, ground - h * 0.25, h * 0.25, col(30, 30, 34));
    }
    fn label(&mut self, glyphs: &mut GlyphCache, text: &str, x: f32, y: f32, size: f32, family: &str) {
        let st = TextStyle { family: family.to_string(), weight: Weight::Bold, size, color: Color::WHITE, letter_spacing: 0.0 };
        let tw = measure(text, &st);
        // Labels are positioned in layer space; map through the transform's scale/translate.
        let (sx, sy) = (self.tf.sx, self.tf.sy);
        let px = self.tf.tx + (x - tw / 2.0) * sx;
        let py = self.tf.ty + y * sy;
        let mut st2 = st;
        st2.size = size * sx.abs().max(0.01);
        // Dark outline so labels read over sky, cloud, water or ground.
        let mut halo = st2.clone();
        halo.color = Color::from_rgba8(6, 12, 24, 210);
        let o = (st2.size * 0.06).max(1.0);
        for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o), (o, o)] {
            draw_line(self.pm, glyphs, text, px + dx, py + dy, &halo, self.alpha, None);
        }
        draw_line(self.pm, glyphs, text, px, py, &st2, self.alpha, None);
    }
    /// 0-1 phase of a loop of `secs` seconds.
    fn phase(&self, secs: f32) -> f32 {
        (self.t % secs) / secs
    }
}


/// Draw diagram `name` into the layer box at animation time `time`.
#[allow(clippy::too_many_arguments)]
pub fn draw(pm: &mut Pixmap, glyphs: &mut GlyphCache, tf: Transform, mask: Option<&Mask>, w: f32, h: f32, name: &str, time: f32, opacity: f32, font: &str) {
    #[allow(non_snake_case)]
    let (SKY, GROUND, CLOUD, DARK, WARM, COLD, YELLOW, WATER, WHITE, RED, GREEN) = (
        col(36, 64, 104),
        col(54, 92, 58),
        col(200, 206, 214),
        col(96, 104, 116),
        col(232, 96, 64),
        col(70, 140, 235),
        col(255, 214, 10),
        col(46, 120, 200),
        Color::WHITE,
        col(230, 50, 50),
        col(60, 190, 90),
    );
    let mut c = Canvas { pm, tf, mask, w, h, t: time.max(0.0), alpha: opacity };
    let ground = h * 0.86;
    let fs = (h * 0.045).max(14.0);
    // Sky and ground backdrop for the outdoor scenes.
    if matches!(name, "hodograph" | "skewt" | "wind_profile") {
        c.rect(0.0, 0.0, w, h, col(20, 26, 38));
        draw_sounding(&mut c, glyphs, name, fs, font);
        return;
    }
    let outdoor = !matches!(
        name,
        "atmosphere" | "heat_body" | "cold_body" | "wet_bulb" | "wind_chill" | "emergency_kit" | "precip_types" | "inversion" | "instability" | "coriolis" | "radar_velocity" | "radar_cc"
            | "hook_echo" | "alert_terms" | "hurricane_categories" | "evac_timeline"
    );
    let _ = (SKY, GROUND);
    if outdoor {
        // Modern backdrop: deep sky fading to a lighter horizon, a soft horizon glow and a
        // shaded ground.
        c.vgrad(0.0, 0.0, w, ground, col(14, 34, 66), col(62, 108, 160));
        c.vgrad(0.0, ground - h * 0.06, w, h * 0.06, Color::from_rgba8(255, 255, 255, 0), Color::from_rgba8(190, 215, 240, 40));
        c.vgrad(0.0, ground, w, h - ground, col(66, 120, 72), col(34, 66, 40));
        c.rect(0.0, ground, w, (h * 0.006).max(1.5), Color::from_rgba8(150, 200, 140, 200));
    } else {
        c.vgrad(0.0, 0.0, w, h, col(26, 36, 58), col(12, 18, 32));
    }
    match name {
        "supercell" | "tornado_lifecycle" | "rfd" => {
            let p = c.phase(8.0);
            // Tower and anvil.
            c.poly(&[(w * 0.25, ground * 0.62), (w * 0.35, h * 0.12), (w * 0.95, h * 0.08), (w * 0.95, h * 0.16), (w * 0.62, h * 0.2), (w * 0.62, ground * 0.62)], DARK);
            c.cloud(w * 0.18, ground * 0.62, w * 0.55, h * 0.16, DARK);
            // Rotating updraft: arrows going round.
            for k in 0..3 {
                let a = (p * std::f32::consts::TAU + k as f32 * 2.1) % std::f32::consts::TAU;
                let (cx, cy, rx, ry) = (w * 0.45, ground * 0.45, w * 0.08, h * 0.05);
                let (x0, y0) = (cx + rx * a.cos(), cy + ry * a.sin());
                let (x1, y1) = (cx + rx * (a + 0.6).cos(), cy + ry * (a + 0.6).sin());
                c.arrow((x0, y0), (x1, y1), WARM, 5.0);
            }
            c.arrow((w * 0.42, ground * 0.95), (w * 0.44, ground * 0.66), WARM, 7.0);
            // Wall cloud lowers, then the funnel grows (life cycle: forms, matures, ropes out).
            let wall = (p * 3.0).min(1.0);
            c.rect(w * 0.36, ground * 0.62, w * 0.16, h * 0.05 * wall, DARK);
            let grow = ((p - 0.3) / 0.4).clamp(0.0, 1.0);
            let rope = if name == "tornado_lifecycle" { ((p - 0.75) / 0.25).clamp(0.0, 1.0) } else { 0.0 };
            if grow > 0.0 {
                let top_w = w * 0.06 * (1.0 - rope * 0.7);
                let bottom = ground * 0.67 + (ground - ground * 0.67) * grow;
                let sway = rope * w * 0.06 * (c.t * 3.0).sin();
                c.poly(&[(w * 0.44 - top_w, ground * 0.67), (w * 0.44 + top_w, ground * 0.67), (w * 0.44 + sway + w * 0.008, bottom), (w * 0.44 + sway - w * 0.008, bottom)], col(150, 150, 160));
                if grow >= 1.0 {
                    for k in 0..8 {
                        let a = c.t * 4.0 + k as f32;
                        c.circle(w * 0.44 + sway + a.cos() * w * 0.04, ground - 6.0 - (a * 1.7).sin().abs() * h * 0.05, 4.0, col(120, 100, 80));
                    }
                }
            }
            if name == "rfd" {
                c.arrow((w * 0.62, ground * 0.35), (w * 0.52, ground * 0.85), COLD, 7.0);
                c.label(glyphs, "Cool air sinks behind the storm", w * 0.74, ground * 0.2, fs, font);
            }
            c.label(glyphs, "Rising air that spins", w * 0.3, ground * 0.42, fs, font);
            if grow > 0.0 {
                c.label(glyphs, if rope > 0.0 { "Thinning out: the tornado is ending" } else { "Tornado" }, w * 0.58, ground * 0.8, fs, font);
            }
        }
        "wall_cloud" => {
            // Side view: the storm base, the rain shaft off to one side and the wall cloud
            // lowering under the updraft, slowly turning.
            let p = c.phase(6.0);
            let base = ground * 0.38;
            c.cloud(w * 0.04, ground * 0.34, w * 0.92, h * 0.14, DARK);
            c.rect(w * 0.04, ground * 0.3, w * 0.92, base - ground * 0.3, DARK);
            c.vgrad(w * 0.64, base, w * 0.3, ground - base, Color::from_rgba8(150, 170, 190, 110), Color::from_rgba8(150, 170, 190, 30));
            for k in 0..14 {
                let x = w * (0.65 + 0.02 * k as f32);
                let y = base + ((c.t * 0.9 + k as f32 * 0.37) % 1.0) * (ground - base - h * 0.05);
                c.line(&[(x, y), (x - w * 0.01, y + h * 0.05)], col(160, 200, 240), 3.0);
            }
            let lower = 0.6 + 0.4 * (p * 2.0).min(1.0);
            let (wx, ww, wh) = (w * 0.24, w * 0.24, h * 0.13 * lower);
            c.poly(&[(wx, base), (wx + ww, base), (wx + ww * 0.92, base + wh), (wx + ww * 0.08, base + wh)], col(78, 86, 98));
            for k in 0..6 {
                let a = c.t * 1.2 + k as f32 * 1.05;
                c.circle(wx + ww * 0.5 + a.cos() * ww * 0.5, base + wh * 0.65 + a.sin() * wh * 0.25, h * 0.018, col(118, 126, 138));
            }
            c.arrow((w * 0.03, ground * 0.92), (wx + ww * 0.3, base + wh + h * 0.03), WARM, 6.0);
            c.arrow((w * 0.66, ground * 0.72), (wx + ww * 0.98, base + wh * 0.7), COLD, 6.0);
            c.label(glyphs, "Storm base", w * 0.5, h * 0.12, fs, font);
            c.label(glyphs, "Wall cloud: lower, slowly turning", w * 0.36, base + wh + h * 0.1, fs, font);
            c.label(glyphs, "Rain-cooled air", w * 0.8, ground * 0.86, fs * 0.9, font);
            c.label(glyphs, "Warm, moist air flows in", w * 0.24, ground + h * 0.09, fs * 0.9, font);
        }
        "funnel_cloud" | "debris_cloud" => {
            let p = c.phase(7.0);
            let base = ground * 0.36;
            c.cloud(w * 0.08, ground * 0.32, w * 0.84, h * 0.13, DARK);
            c.rect(w * 0.08, ground * 0.28, w * 0.84, base - ground * 0.28, DARK);
            let (fx, top_w) = (w * 0.5, w * 0.07);
            c.poly(&[(fx - w * 0.12, base), (fx + w * 0.12, base), (fx + w * 0.1, base + h * 0.05), (fx - w * 0.1, base + h * 0.05)], col(78, 86, 98));
            let top = base + h * 0.05;
            if name == "funnel_cloud" {
                // The funnel lowers part way; later a dust whirl shows the spin has reached the
                // ground (a tornado, even though the funnel doesn't touch).
                let reach = 0.25 + 0.35 * (p * 2.5).min(1.0);
                let bottom = top + (ground - top) * reach;
                c.poly(&[(fx - top_w, top), (fx + top_w, top), (fx + w * 0.006, bottom), (fx - w * 0.006, bottom)], col(150, 150, 162));
                let touched = p > 0.62;
                if touched {
                    for k in 0..12 {
                        let a = c.t * 5.0 + k as f32 * 0.52;
                        c.circle(fx + a.cos() * w * 0.05, ground - h * 0.02 - (a * 1.3).sin().abs() * h * 0.05, 4.0, col(150, 120, 90));
                    }
                }
                c.label(glyphs, if touched { "Dust whirl below: it's a tornado" } else { "Funnel cloud: spin not on the ground" }, w * 0.5, h * 0.12, fs, font);
                c.label(glyphs, "Spin reaching the ground makes it a tornado", w * 0.5, ground + h * 0.09, fs * 0.85, font);
            } else {
                // A tornado with a broad cloud of dirt and debris churning at its base.
                c.poly(&[(fx - top_w, top), (fx + top_w, top), (fx + w * 0.03, ground), (fx - w * 0.03, ground)], col(150, 150, 162));
                let grow = 0.7 + 0.3 * (p * std::f32::consts::TAU).sin().abs();
                c.circle_alpha(fx, ground - h * 0.07, w * 0.15 * grow, h * 0.09, Color::from_rgba8(120, 96, 72, 210));
                c.circle_alpha(fx, ground - h * 0.12, w * 0.11 * grow, h * 0.07, Color::from_rgba8(140, 112, 84, 200));
                for k in 0..16 {
                    let a = c.t * 3.0 + k as f32 * 0.39;
                    let r = w * (0.06 + 0.012 * (k % 5) as f32);
                    let y = ground - h * (0.05 + 0.03 * (k % 6) as f32) - ((c.t * 0.8 + k as f32 * 0.21) % 1.0) * h * 0.12;
                    c.rect(fx + a.cos() * r, y, 6.0 + (k % 3) as f32 * 3.0, 4.0, col(70, 56, 44));
                }
                c.label(glyphs, "Debris cloud: dirt and debris lifted", w * 0.5, h * 0.12, fs, font);
                c.label(glyphs, "Sometimes all you can see of the tornado", w * 0.5, ground + h * 0.09, fs * 0.85, font);
            }
        }
        "hook_echo" => {
            // Radar view (from above) in the house TWC palette: the storm core with rain
            // wrapping around the rotation into a hook.
            let tw = |dbz: f32| crate::twc::rain(dbz).map(|[r, g, b]| col(r, g, b)).unwrap_or(DARK);
            let (rcx, rcy) = (w * 0.42, h * 0.66);
            let a_pt = (w * 0.8, h * 0.2);
            let b_pt = (w * 0.5, h * 0.55);
            let r_hook = ((b_pt.0 - rcx).powi(2) + (b_pt.1 - rcy).powi(2)).sqrt();
            let a0 = (b_pt.1 - rcy).atan2(b_pt.0 - rcx);
            let mut spine: Vec<(f32, f32, f32)> = Vec::new();
            for k in 0..=40 {
                let s = k as f32 / 40.0;
                spine.push((a_pt.0 + (b_pt.0 - a_pt.0) * s, a_pt.1 + (b_pt.1 - a_pt.1) * s, h * (0.17 - 0.05 * s)));
            }
            for k in 1..=40 {
                let s = k as f32 / 40.0;
                let a = a0 + s * 3.6;
                spine.push((rcx + a.cos() * r_hook, rcy + a.sin() * r_hook, h * (0.12 - 0.09 * s)));
            }
            let wobble = 1.0 + 0.03 * (c.t * 1.5).sin();
            for (dbz, k, core) in [(20.0, 1.0, 1.0), (35.0, 0.74, 1.0), (45.0, 0.52, 0.75), (55.0, 0.32, 0.55), (65.0, 0.16, 0.4)] {
                let colour = tw(dbz);
                let n = (spine.len() as f32 * core) as usize;
                for (x, y, r) in &spine[..n] {
                    c.circle(*x, *y, r * k * wobble, colour);
                }
            }
            let turn = c.t * 2.0;
            for k in 0..3 {
                let a = turn + k as f32 * 2.09;
                c.arrow((rcx + a.cos() * h * 0.07, rcy + a.sin() * h * 0.07), (rcx + (a + 0.7).cos() * h * 0.07, rcy + (a + 0.7).sin() * h * 0.07), WHITE, 4.0);
            }
            c.label(glyphs, "Heavy rain and hail core", w * 0.78, h * 0.06, fs * 0.9, font);
            c.label(glyphs, "Rain wraps around the rotation", w * 0.36, h * 0.37, fs * 0.9, font);
            c.label(glyphs, "Tornado threat at the tip", rcx, h * 0.95, fs * 0.9, font);
        }
        "alert_terms" => {
            // Advisory, watch, warning as rising steps of urgency; the highlight walks up.
            let rows = [
                ("ADVISORY", "Be aware: it may cause problems", col(255, 214, 10)),
                ("WATCH", "Be prepared: conditions are right", col(255, 140, 20)),
                ("WARNING", "Take action: it's happening now", col(230, 40, 40)),
            ];
            let lit = ((c.phase(6.0) * 3.0) as usize).min(2);
            for (i, (word, line, colour)) in rows.iter().enumerate() {
                let y = h * (0.66 - 0.29 * i as f32);
                let rw = w * (0.62 + 0.15 * i as f32);
                let a = if i == lit { 1.0 } else { 0.55 };
                c.rect(w * 0.04, y, rw, h * 0.22, Color::from_rgba8(255, 255, 255, 18));
                let mut fill = *colour;
                fill.apply_opacity(a);
                c.rect(w * 0.04, y, w * 0.022, h * 0.22, fill);
                c.label(glyphs, word, w * 0.04 + w * 0.17, y + h * 0.09, fs * 1.2, font);
                c.label(glyphs, line, w * 0.04 + rw * 0.5, y + h * 0.18, fs * 0.8, font);
            }
            c.arrow((w * 0.93, h * 0.86), (w * 0.93, h * 0.1), WHITE, 5.0);
            c.label(glyphs, "More urgent", w * 0.9, h * 0.97, fs * 0.75, font);
        }
        "hurricane_categories" => {
            // Saffir-Simpson: lowest sustained wind for each category, bars in the house palette.
            let p = (c.phase(6.0) * 2.0).min(1.0);
            let cats = [(1, 74.0, 35.0), (2, 96.0, 45.0), (3, 111.0, 52.0), (4, 130.0, 58.0), (5, 157.0, 66.0)];
            let (floor, top) = (h * 0.84, h * 0.16);
            let bw = w * 0.13;
            for (i, (cat, mph, dbz)) in cats.iter().enumerate() {
                let x = w * (0.08 + 0.175 * i as f32);
                let bh = (floor - top) * (mph / 170.0) * p;
                let [r, g, b] = crate::twc::rain(*dbz).unwrap_or([200, 200, 200]);
                c.rect(x, floor - bh, bw, bh, col(r, g, b));
                c.label(glyphs, &format!("Cat {cat}"), x + bw / 2.0, floor + h * 0.08, fs, font);
                if p >= 1.0 {
                    c.label(glyphs, &format!("{mph:.0}+ mph"), x + bw / 2.0, floor - bh - h * 0.02, fs * 0.8, font);
                }
            }
            c.line(&[(w * 0.04, floor), (w * 0.96, floor)], WHITE, 2.0);
            c.label(glyphs, "Lowest sustained wind for each category", w * 0.5, h * 0.07, fs * 0.85, font);
        }
        "evac_timeline" => {
            // Days before the wind arrives, left to right; the marker walks the line.
            let stops = [(0.1, "3 days", "Check your plan"), (0.37, "48 hours", "Hurricane watch"), (0.64, "36 hours", "Hurricane warning"), (0.9, "Winds arrive", "Shelter in place")];
            let y = h * 0.5;
            c.line(&[(w * 0.06, y), (w * 0.94, y)], WHITE, 4.0);
            let p = c.phase(8.0);
            for (i, (fx, when, what)) in stops.iter().enumerate() {
                let x = w * fx;
                let reached = p >= i as f32 / 4.0;
                c.circle(x, y, h * 0.035, if reached { YELLOW } else { DARK });
                c.label(glyphs, when, x, y - h * 0.08, fs, font);
                c.label(glyphs, what, x, y + h * 0.12, fs * 0.8, font);
            }
            c.circle(w * (0.06 + 0.88 * p), y, h * 0.018, WHITE);
            c.label(glyphs, "Leave when officials say: don't wait for the wind", w * 0.5, h * 0.88, fs * 0.85, font);
        }
        "tornado_vs_straight" => {
            let p = c.phase(4.0);
            c.label(glyphs, "Tornado: debris swirls in", w * 0.25, h * 0.12, fs, font);
            c.label(glyphs, "Straight-line wind: all one way", w * 0.75, h * 0.12, fs, font);
            c.line(&[(w * 0.5, h * 0.05), (w * 0.5, h)], WHITE, 2.0);
            for k in 0..10 {
                let a = p * std::f32::consts::TAU + k as f32 * 0.63;
                let r = h * (0.08 + 0.025 * k as f32);
                c.circle(w * 0.25 + a.cos() * r, ground * 0.6 + a.sin() * r * 0.5, 5.0, col(170, 140, 100));
            }
            for k in 0..8 {
                let y = ground * (0.3 + 0.07 * k as f32);
                let x = w * 0.55 + ((p + k as f32 * 0.13) % 1.0) * w * 0.4;
                c.arrow((x, y), (x + w * 0.06, y), COLD, 4.0);
            }
        }
        "radar_beam" | "radar_velocity" | "radar_cc" => {
            let p = c.phase(3.0);
            if name == "radar_beam" {
                // Dish, beam rising over the curve of the earth, pulse out and echo back.
                c.rect(w * 0.06, ground - h * 0.1, w * 0.02, h * 0.1, WHITE);
                c.circle(w * 0.07, ground - h * 0.12, h * 0.035, WHITE);
                c.poly(&[(w * 0.08, ground - h * 0.12), (w * 0.95, ground - h * 0.42), (w * 0.95, ground - h * 0.22)], Color::from_rgba8(255, 255, 255, 50));
                c.cloud(w * 0.62, ground * 0.62, w * 0.18, h * 0.12, DARK);
                let px = w * 0.08 + p.min(0.5) * 2.0 * w * 0.56;
                let back = if p > 0.5 { w * 0.64 - (p - 0.5) * 2.0 * w * 0.56 } else { -100.0 };
                c.circle(px, ground - h * 0.12 - (px - w * 0.08) * 0.24, 8.0, YELLOW);
                if back > 0.0 {
                    c.circle(back, ground - h * 0.12 - (back - w * 0.08) * 0.24, 6.0, GREEN);
                }
                c.label(glyphs, "Radar sends out a pulse", w * 0.3, ground * 0.62, fs, font);
                c.label(glyphs, "Rain bounces it back", w * 0.45, ground * 0.95, fs, font);
            } else if name == "radar_velocity" {
                // Left: reflectivity blob; right: green toward / red away couplet.
                c.label(glyphs, "How much rain is falling", w * 0.25, h * 0.1, fs, font);
                c.label(glyphs, "Which way the wind blows", w * 0.75, h * 0.1, fs, font);
                for (k, (r, col_)) in [(h * 0.3, col(40, 160, 60)), (h * 0.2, col(240, 220, 40)), (h * 0.1, col(230, 40, 40))].into_iter().enumerate() {
                    c.circle(w * 0.25, h * 0.55, r * (1.0 + 0.03 * (c.t * 2.0 + k as f32).sin()), col_);
                }
                c.circle(w * 0.68, h * 0.55, h * 0.16, col(30, 200, 80));
                c.circle(w * 0.82, h * 0.55, h * 0.16, col(230, 50, 60));
                c.label(glyphs, "Green: coming in", w * 0.75, h * 0.82, fs, font);
                c.label(glyphs, "Red: going away", w * 0.75, h * 0.9, fs, font);
            } else {
                c.label(glyphs, "Just rain", w * 0.25, h * 0.1, fs, font);
                c.label(glyphs, "matching drops", w * 0.25, h * 0.18, fs * 0.8, font);
                c.label(glyphs, "Debris in the air", w * 0.75, h * 0.1, fs, font);
                c.label(glyphs, "mixed shapes", w * 0.75, h * 0.18, fs * 0.8, font);
                for k in 0..20 {
                    let (x, y) = (w * (0.08 + 0.02 * (k % 10) as f32 * 2.0), h * (0.3 + 0.12 * (k / 10) as f32));
                    c.circle(x, y + ((c.t + k as f32) % 2.0) * h * 0.05, 6.0, COLD);
                }
                for k in 0..20 {
                    let (x, y) = (w * (0.58 + 0.035 * (k % 10) as f32), h * (0.3 + 0.12 * (k / 10) as f32));
                    let s = 4.0 + (k * 7 % 9) as f32;
                    c.rect(x, y + ((c.t * 1.5 + k as f32) % 2.0) * h * 0.05, s, s * 0.6, col(170, 130, 90));
                }
            }
        }
        "lightning_cg" | "lightning_ic" | "lightning_positive" | "thunder_distance" => {
            let p = c.phase(3.0);
            c.cloud(w * 0.15, ground * 0.35, w * 0.6, h * 0.16, DARK);
            if name != "thunder_distance" {
                for k in 0..6 {
                    c.label(glyphs, "+", w * (0.25 + 0.08 * k as f32), ground * 0.18, fs * 1.2, font);
                    c.label(glyphs, "−", w * (0.25 + 0.08 * k as f32), ground * 0.4, fs * 1.2, font);
                }
            }
            let bolt = |c: &mut Canvas, x0: f32, y0: f32, y1: f32, reach: f32| {
                let mut pts = vec![(x0, y0)];
                let steps = 7;
                for k in 1..=steps {
                    let f = k as f32 / steps as f32;
                    if f > reach {
                        break;
                    }
                    pts.push((x0 + if k % 2 == 0 { 18.0 } else { -16.0 }, y0 + (y1 - y0) * f));
                }
                c.line(&pts, YELLOW, 5.0);
            };
            match name {
                "lightning_ic" => {
                    if p > 0.3 && p < 0.6 {
                        c.line(&[(w * 0.25, ground * 0.2), (w * 0.4, ground * 0.32), (w * 0.55, ground * 0.22), (w * 0.7, ground * 0.35)], YELLOW, 5.0);
                    }
                }
                "lightning_positive" => {
                    let reach = (p * 2.0).min(1.0);
                    bolt(&mut c, w * 0.85, ground * 0.18, ground, reach);
                    c.label(glyphs, "Some strikes hit miles from the storm", w * 0.75, ground * 0.95, fs, font);
                }
                "thunder_distance" => {
                    if p < 0.15 {
                        bolt(&mut c, w * 0.3, ground * 0.38, ground, 1.0);
                    }
                    let r = p * w * 0.7;
                    let mut pb = PathBuilder::new();
                    pb.push_circle(w * 0.3, ground * 0.7, r);
                    if let Some(path) = pb.finish() {
                        c.pm.stroke_path(&path, &rgba(WHITE, c.alpha * (1.0 - p)), &Stroke { width: 3.0, ..Stroke::default() }, c.tf, c.mask);
                    }
                    c.person(w * 0.85, ground, h * 0.18, WHITE);
                    // Flash-to-bang: count in whole 5-second steps so the sum is always exact
                    // (5 s = 1 mile, 10 s = 2 miles, 15 s = 3 miles).
                    let miles = ((p * 3.0).floor() as u32 + 1).min(3);
                    let secs = miles * 5;
                    let unit = if miles == 1 { "mile" } else { "miles" };
                    c.label(glyphs, &format!("{secs} seconds ÷ 5 = about {miles} {unit}"), w * 0.6, h * 0.1, fs * 1.1, font);
                }
                _ => {
                    let reach = (p * 2.5).min(1.0);
                    bolt(&mut c, w * 0.45, ground * 0.42, ground, reach);
                    if reach >= 1.0 && p < 0.6 {
                        c.rect(0.0, 0.0, w, h, Color::from_rgba8(255, 255, 255, 60));
                        c.label(glyphs, "The bright flash races back up", w * 0.72, ground * 0.7, fs, font);
                    } else {
                        c.label(glyphs, "Lightning reaches down from the cloud", w * 0.72, ground * 0.6, fs, font);
                    }
                }
            }
        }
        "hail" => {
            let p = c.phase(6.0);
            c.cloud(w * 0.15, ground * 0.45, w * 0.7, h * 0.3, DARK);
            c.line(&[(0.0, ground * 0.22), (w, ground * 0.22)], COLD, 2.0);
            c.label(glyphs, "Below freezing above this line", w * 0.12, ground * 0.2, fs * 0.8, font);
            c.arrow((w * 0.45, ground * 0.95), (w * 0.45, ground * 0.15), WARM, 8.0);
            let a = p * std::f32::consts::TAU * 2.0;
            let size = 6.0 + p * 18.0;
            let (x, y) = if p < 0.85 { (w * 0.5 + a.cos() * w * 0.1, ground * 0.32 + a.sin() * h * 0.12) } else { (w * 0.62, ground * 0.32 + (p - 0.85) / 0.15 * ground * 0.68) };
            c.circle(x, y, size, WHITE);
            c.circle(x, y, size * 0.6, col(200, 225, 255));
            c.label(glyphs, "More ice freezes on with each trip up", w * 0.72, ground * 0.6, fs, font);
        }
        "fronts" => {
            let p = c.phase(8.0);
            let x = w * (0.2 + 0.5 * p);
            c.poly(&[(0.0, ground), (x, ground), (x - w * 0.15, ground * 0.45), (0.0, ground * 0.45)], Color::from_rgba8(70, 140, 235, 120));
            c.poly(&[(x, ground), (w, ground), (w, ground * 0.2), (x + w * 0.3, ground * 0.2)], Color::from_rgba8(232, 96, 64, 90));
            c.cloud(x - w * 0.05, ground * 0.35, w * 0.16, h * 0.14, DARK);
            c.arrow((x + w * 0.05, ground * 0.9), (x - w * 0.02, ground * 0.45), WARM, 6.0);
            c.label(glyphs, "Cold air pushes in", x - w * 0.18, ground * 0.75, fs, font);
            c.label(glyphs, "Warm air forced up", x + w * 0.2, ground * 0.5, fs, font);
        }
        "precip_types" => {
            let labels = ["Snow", "Sleet", "Freezing rain", "Rain"];
            let warm = [0.0, 0.25, 0.4, 1.0];
            for (i, l) in labels.iter().enumerate() {
                let x = w * (0.05 + 0.24 * i as f32);
                let cw = w * 0.2;
                c.rect(x, h * 0.15, cw, h * 0.7, COLD);
                let warm_h = h * 0.7 * warm[i];
                if warm[i] > 0.0 && warm[i] < 1.0 {
                    c.rect(x, h * 0.35, cw, warm_h, WARM);
                } else if warm[i] >= 1.0 {
                    c.rect(x, h * 0.15, cw, h * 0.7, WARM);
                }
                for k in 0..5 {
                    let y = h * 0.15 + ((c.t * 0.3 + k as f32 * 0.2) % 1.0) * h * 0.7;
                    if i == 0 || (i == 1 && y > h * 0.6) {
                        c.circle(x + cw * (0.2 + 0.15 * k as f32), y, 5.0, WHITE);
                    } else {
                        c.line(&[(x + cw * (0.2 + 0.15 * k as f32), y), (x + cw * (0.2 + 0.15 * k as f32), y + 12.0)], col(140, 200, 255), 3.0);
                    }
                }
                c.label(glyphs, l, x + cw / 2.0, h * 0.95, fs, font);
            }
        }
        "shelter_school" => {
            // Cross-section of a school or office: classrooms with windows on the outside walls,
            // an interior hallway in the middle; people move to the hallway and cover their heads.
            let p = c.phase(6.0);
            let (top, left, right) = (ground - h * 0.42, w * 0.06, w * 0.94);
            c.rect(left, top, right - left, ground - top, col(178, 168, 150));
            c.rect(left - w * 0.01, top - h * 0.03, right - left + w * 0.02, h * 0.03, col(96, 90, 84));
            let hall = (w * 0.4, w * 0.6);
            c.rect(hall.0, top + h * 0.03, hall.1 - hall.0, ground - top - h * 0.03, Color::from_rgba8(60, 190, 90, 150));
            for wx in [left + w * 0.04, right - w * 0.12] {
                c.rect(wx, top + h * 0.1, w * 0.08, h * 0.12, col(150, 200, 235));
                c.label(glyphs, "×", wx + w * 0.04, top + h * 0.2, fs * 1.4, font);
            }
            c.rect(hall.0 - w * 0.008, top + h * 0.03, w * 0.008, ground - top - h * 0.03, col(120, 110, 100));
            c.rect(hall.1, top + h * 0.03, w * 0.008, ground - top - h * 0.03, col(120, 110, 100));
            let k = (p * 2.0).min(1.0);
            for (i, from) in [w * 0.22, w * 0.78].into_iter().enumerate() {
                let to = if i == 0 { w * 0.45 } else { w * 0.55 };
                let x = from + (to - from) * k;
                let crouch = if k >= 1.0 { h * 0.05 } else { 0.0 };
                c.person_pose(x, ground + crouch, h * 0.16, WHITE, if k >= 1.0 { 0.85 } else { 0.0 });
            }
            c.label(glyphs, "Interior hallway, lowest floor", w * 0.5, h * 0.12, fs, font);
            c.label(glyphs, "Away from windows and big open rooms", w * 0.5, ground + h * 0.09, fs * 0.85, font);
        }
        "shelter_house" => {
            let p = c.phase(6.0);
            c.house(w * 0.2, ground, w * 0.6, h * 0.42, col(190, 170, 140), col(120, 60, 50));
            // Basement and an interior room.
            c.rect(w * 0.2, ground, w * 0.6, h * 0.12, col(110, 100, 90));
            c.rect(w * 0.44, ground - h * 0.42 * 0.6, w * 0.12, h * 0.42 * 0.6, Color::from_rgba8(60, 190, 90, 140));
            let x = w * (0.3 + 0.2 * (p * 2.0).min(1.0));
            c.person(x, ground, h * 0.16, WHITE);
            c.label(glyphs, "Lowest floor, interior room", w * 0.5, h * 0.12, fs, font);
            c.label(glyphs, "Basement is best; if none, a small inside room", w * 0.5, h * 0.96, fs * 0.85, font);
        }
        "downed_line" => {
            let p = c.phase(6.0);
            // Pole, sagging line on the ground, sparks, a 35 ft circle and a person backing away.
            c.rect(w * 0.18, ground - h * 0.55, w * 0.025, h * 0.55, col(120, 90, 60));
            c.rect(w * 0.12, ground - h * 0.55, w * 0.14, h * 0.02, col(120, 90, 60));
            c.line(&[(w * 0.25, ground - h * 0.53), (w * 0.35, ground - h * 0.2), (w * 0.45, ground - h * 0.01), (w * 0.62, ground)], col(30, 30, 30), 5.0);
            for i in 0..5 {
                let a = (p * 12.0 + i as f32 * 1.3).sin();
                c.line(&[(w * 0.45, ground - h * 0.01), (w * (0.45 + 0.03 * a), ground - h * (0.04 + 0.03 * (i as f32 * 0.7 + p * 9.0).cos().abs()))], YELLOW, 3.0);
            }
            c.poly(&[(w * 0.2, ground + h * 0.02), (w * 0.45, ground - h * 0.03), (w * 0.7, ground + h * 0.02), (w * 0.45, ground + h * 0.07)], col(230, 60, 60));
            c.label(glyphs, "35 ft danger zone: water and fences carry current", w * 0.45, ground + h * 0.11, fs * 0.8, font);
            let x = w * (0.72 + 0.16 * (p * 1.5).min(1.0));
            c.person(x, ground, h * 0.16, WHITE);
            c.arrow((w * 0.74, ground - h * 0.25), (w * 0.9, ground - h * 0.25), GREEN, 6.0);
            c.label(glyphs, "Shuffle away, feet together", w * 0.8, h * 0.12, fs, font);
            c.label(glyphs, "Assume every line is live", w * 0.3, h * 0.12, fs, font);
        }
        "shelter_mobile" => {
            let p = c.phase(6.0);
            c.rect(w * 0.08, ground - h * 0.16, w * 0.3, h * 0.16, col(200, 200, 205));
            c.house(w * 0.62, ground, w * 0.3, h * 0.3, col(150, 150, 160), col(90, 90, 100));
            let x = w * (0.23 + 0.55 * (p * 1.4).min(1.0));
            c.person(x, ground, h * 0.16, WHITE);
            c.arrow((w * 0.4, ground - h * 0.25), (w * 0.6, ground - h * 0.25), GREEN, 6.0);
            c.label(glyphs, "Leave before the warning", w * 0.23, h * 0.12, fs, font);
            c.label(glyphs, "Sturdy shelter", w * 0.77, h * 0.12, fs, font);
        }
        "shelter_vehicle" | "shelter_outdoors" => {
            let p = c.phase(6.0);
            c.poly(&[(w * 0.55, ground), (w * 0.62, ground + h * 0.08), (w * 0.85, ground + h * 0.08), (w * 0.92, ground)], col(40, 70, 44));
            if name == "shelter_vehicle" {
                c.car(w * 0.12, ground, w * 0.25, col(200, 60, 60));
            }
            let x = w * (0.3 + 0.42 * (p * 1.5).min(1.0));
            let low = if p > 0.7 { h * 0.07 } else { 0.0 };
            c.person_pose(x, ground + low, h * 0.15, WHITE, if p > 0.7 { 0.85 } else { 0.0 });
            c.label(glyphs, "Get to a low spot, cover your head", w * 0.5, h * 0.12, fs, font);
            c.label(glyphs, "Never under an overpass", w * 0.5, h * 0.2, fs * 0.85, font);
        }
        "flood_depth" | "low_water" | "urban_flood" | "flash_flood" => {
            let p = c.phase(8.0);
            let level = (p * 1.2).min(1.0);
            if name == "flash_flood" {
                // Left: a narrow creek under a downpour rises in minutes. Right: a wide river
                // valley fills slowly over days.
                c.line(&[(w * 0.5, h * 0.04), (w * 0.5, h)], WHITE, 2.0);
                c.cloud(w * 0.08, h * 0.32, w * 0.32, h * 0.1, DARK);
                for k in 0..12 {
                    let x = w * (0.1 + 0.026 * k as f32);
                    let y = h * 0.34 + ((c.t * 1.4 + k as f32 * 0.29) % 1.0) * (ground - h * 0.4);
                    c.line(&[(x, y), (x - w * 0.006, y + h * 0.04)], col(160, 200, 240), 3.0);
                }
                let fast = (p * 3.0).min(1.0);
                let creek_top = ground - h * 0.3 * fast;
                c.poly(&[(w * 0.04, ground - h * 0.32), (w * 0.2, ground), (w * 0.3, ground), (w * 0.46, ground - h * 0.32), (w * 0.46, h), (w * 0.04, h)], col(66, 110, 70));
                c.poly(&[(w * 0.2 - (ground - creek_top) * 0.5, creek_top), (w * 0.3 + (ground - creek_top) * 0.5, creek_top), (w * 0.3, ground), (w * 0.2, ground)], WATER);
                let slow = p;
                let river_top = ground - h * 0.16 * slow;
                c.poly(&[(w * 0.54, ground - h * 0.16), (w * 0.6, ground), (w * 0.9, ground), (w * 0.96, ground - h * 0.16), (w * 0.96, h), (w * 0.54, h)], col(66, 110, 70));
                c.poly(&[(w * 0.6 - (ground - river_top) * 0.37, river_top), (w * 0.9 + (ground - river_top) * 0.37, river_top), (w * 0.9, ground), (w * 0.6, ground)], WATER);
                c.label(glyphs, "Flash flood", w * 0.25, h * 0.1, fs, font);
                c.label(glyphs, "minutes to hours", w * 0.25, h * 0.18, fs * 0.8, font);
                c.label(glyphs, "River flood", w * 0.75, h * 0.1, fs, font);
                c.label(glyphs, &format!("rises over days (day {})", 1 + (slow * 3.0).min(2.0) as u32), w * 0.75, h * 0.18, fs * 0.8, font);
            } else {
                if name == "urban_flood" {
                    c.house(w * 0.05, ground, w * 0.2, h * 0.3, col(170, 160, 150), col(110, 70, 60));
                    c.house(w * 0.75, ground, w * 0.2, h * 0.3, col(170, 160, 150), col(110, 70, 60));
                }
                c.car(w * 0.4, ground, w * 0.22, col(80, 120, 200));
                c.person(w * 0.25, ground, h * 0.3, WHITE);
                let depth = h * 0.32 * level;
                c.rect(0.0, ground - depth, w, depth + h * 0.14, Color::from_rgba8(46, 120, 200, 190));
                for (inches, frac) in [(6.0f32, 0.09f32), (12.0, 0.18), (24.0, 0.32)] {
                    let y = ground - h * frac;
                    c.line(&[(w * 0.78, y), (w * 0.96, y)], WHITE, 2.0);
                    let text = if inches >= 24.0 { "2 ft: floats most vehicles".to_string() } else if inches >= 12.0 { "1 ft: sweeps away cars".to_string() } else { "6 in: knocks you down".to_string() };
                    c.label(glyphs, &text, w * 0.8, y - 6.0, fs * 0.75, font);
                }
                c.label(glyphs, "Turn around, don't drown", w * 0.5, h * 0.1, fs * 1.1, font);
            }
        }
        "storm_surge" => {
            let p = c.phase(8.0);
            c.poly(&[(w * 0.45, ground), (w, ground - h * 0.18), (w, h), (w * 0.45, h)], col(196, 178, 128));
            c.house(w * 0.7, ground - h * 0.1, w * 0.12, h * 0.15, col(220, 210, 190), col(140, 80, 60));
            let level = ground - h * 0.05 - p * h * 0.3;
            c.rect(0.0, level, w * 0.9, h - level, Color::from_rgba8(46, 120, 200, 200));
            for k in 0..6 {
                let x = (c.t * 60.0 + k as f32 * w * 0.16) % w;
                c.circle(x, level, 10.0, Color::from_rgba8(255, 255, 255, 120));
            }
            c.label(glyphs, "Wind piles water onto the coast", w * 0.4, h * 0.12, fs, font);
        }
        "hurricane" => {
            let (cx, cy) = (w * 0.5, h * 0.5);
            let rot = c.t * 0.6;
            for arm in 0..4 {
                let mut pts = Vec::new();
                for k in 0..60 {
                    let f = k as f32 / 60.0;
                    let a = rot + arm as f32 * std::f32::consts::FRAC_PI_2 + f * 4.5;
                    let r = h * 0.06 + f * h * 0.4;
                    pts.push((cx + a.cos() * r * 1.2, cy + a.sin() * r));
                }
                c.line(&pts, CLOUD, 22.0);
            }
            c.circle(cx, cy, h * 0.11, CLOUD);
            c.circle(cx, cy, h * 0.05, col(40, 60, 90));
            c.label(glyphs, "Eye", cx, cy + 6.0, fs, font);
            c.label(glyphs, "Eyewall: strongest winds", cx, cy - h * 0.15, fs, font);
        }
        "atmosphere" => {
            let layers = [("Thermosphere", col(40, 30, 80)), ("Mesosphere", col(50, 50, 110)), ("Stratosphere", col(60, 90, 150)), ("Troposphere: weather happens here", col(80, 140, 200))];
            for (i, (l, cc)) in layers.iter().enumerate() {
                let y = h * 0.25 * i as f32;
                c.rect(0.0, y, w, h * 0.25, *cc);
                c.label(glyphs, l, w * 0.5, y + h * 0.15, fs, font);
            }
            c.cloud(w * 0.1, h * 0.95, w * 0.2, h * 0.06, CLOUD);
            c.circle(w * (0.1 + (c.t * 0.05) % 0.8), h * 0.6, 6.0, YELLOW);
        }
        "jet_stream" => {
            let mut pts = Vec::new();
            for k in 0..=80 {
                let f = k as f32 / 80.0;
                pts.push((w * f, h * 0.45 + (f * 12.0 + c.t * 1.2).sin() * h * 0.15));
            }
            c.line(&pts, Color::from_rgba8(80, 160, 255, 200), 26.0);
            let mut arrow = pts.clone();
            arrow.truncate(((c.phase(4.0)) * 80.0) as usize + 2);
            c.line(&arrow, WHITE, 6.0);
            c.label(glyphs, "Cold air north of the jet", w * 0.5, h * 0.12, fs, font);
            c.label(glyphs, "Warm air south", w * 0.5, h * 0.92, fs, font);
        }
        "high_low" => {
            for (cx, hi) in [(w * 0.3, true), (w * 0.7, false)] {
                let cy = h * 0.5;
                c.circle(cx, cy, h * 0.12, if hi { COLD } else { RED });
                c.label(glyphs, if hi { "H" } else { "L" }, cx, cy + fs * 0.6, fs * 2.2, font);
                for k in 0..6 {
                    let dir = if hi { 1.0 } else { -1.0 };
                    let a = dir * c.t * 0.8 + k as f32;
                    let r = h * 0.3;
                    c.arrow((cx + a.cos() * r, cy + a.sin() * r), (cx + (a + dir * 0.35).cos() * r, cy + (a + dir * 0.35).sin() * r), WHITE, 4.0);
                }
                c.label(glyphs, if hi { "Sinking air, fair weather" } else { "Rising air, clouds and rain" }, cx, h * 0.95, fs, font);
            }
        }
        "sea_breeze" => {
            let day = c.phase(10.0) < 0.5;
            c.rect(0.0, ground, w * 0.5, h - ground, WATER);
            let dir = if day { 1.0 } else { -1.0 };
            let (a, b) = if day { (w * 0.2, w * 0.8) } else { (w * 0.8, w * 0.2) };
            c.arrow((a, ground - 20.0), (b, ground - 20.0), COLD, 6.0);
            c.arrow((b, ground * 0.35), (a, ground * 0.35), WARM, 6.0);
            let _ = dir;
            c.label(glyphs, if day { "Day: sea breeze blows inland" } else { "Night: land breeze blows out to sea" }, w * 0.5, h * 0.12, fs, font);
        }
        "inversion" => {
            c.line(&[(w * 0.2, h * 0.9), (w * 0.35, h * 0.55), (w * 0.55, h * 0.45), (w * 0.4, h * 0.1)], RED, 5.0);
            c.rect(0.0, h * 0.45, w, h * 0.1, Color::from_rgba8(232, 96, 64, 60));
            for k in 0..12 {
                let x = w * (0.6 + 0.03 * k as f32);
                let y = h * 0.6 + ((c.t * 0.4 + k as f32 * 0.3) % 1.0) * h * 0.3;
                c.circle(x, y, 8.0, Color::from_rgba8(150, 150, 150, 150));
            }
            c.label(glyphs, "A warm layer traps the air below", w * 0.7, h * 0.42, fs, font);
            c.label(glyphs, "Temperature", w * 0.35, h * 0.97, fs * 0.8, font);
        }
        "orographic" => {
            c.poly(&[(w * 0.25, ground), (w * 0.5, h * 0.3), (w * 0.75, ground)], col(110, 100, 90));
            c.arrow((w * 0.05, ground - 20.0), (w * 0.4, h * 0.38), COLD, 6.0);
            c.cloud(w * 0.3, h * 0.35, w * 0.18, h * 0.1, DARK);
            for k in 0..6 {
                let y = h * 0.4 + ((c.t * 0.6 + k as f32 * 0.17) % 1.0) * (ground - h * 0.4);
                c.line(&[(w * (0.32 + 0.03 * k as f32), y), (w * (0.32 + 0.03 * k as f32), y + 12.0)], col(140, 200, 255), 3.0);
            }
            c.label(glyphs, "Wet side", w * 0.25, h * 0.15, fs, font);
            c.label(glyphs, "Rain shadow (dry)", w * 0.8, h * 0.15, fs, font);
        }
        "instability" | "cloud_formation" => {
            let p = c.phase(5.0);
            let y = h * 0.9 - p * h * 0.7;
            c.circle(w * 0.5, y, h * 0.06, if name == "instability" { WARM } else { Color::from_rgba8(255, 255, 255, 180) });
            if name == "cloud_formation" && p > 0.6 {
                c.cloud(w * 0.35, h * 0.25, w * 0.3, h * 0.1, CLOUD);
                c.label(glyphs, "Air cools until a cloud forms", w * 0.5, h * 0.08, fs, font);
            } else {
                c.label(glyphs, if name == "instability" { "Warmer than the air around it: keeps rising" } else { "Rising air cools" }, w * 0.5, h * 0.08, fs, font);
            }
        }
        "coriolis" => {
            let rot = c.t * 0.5;
            c.circle(w * 0.5, h * 0.52, h * 0.4, col(40, 90, 150));
            let mut pts = Vec::new();
            for k in 0..40 {
                let f = k as f32 / 40.0 * c.phase(4.0);
                let r = f * h * 0.38;
                let a = rot - f * 1.4;
                pts.push((w * 0.5 + a.cos() * r, h * 0.52 + a.sin() * r));
            }
            if pts.len() > 1 {
                c.line(&pts, YELLOW, 5.0);
            }
            c.label(glyphs, "Earth's spin bends moving air to the right", w * 0.5, h * 0.06, fs, font);
        }
        "heat_body" | "cold_body" | "wet_bulb" | "wind_chill" => {
            let hot = matches!(name, "heat_body" | "wet_bulb");
            let p = c.phase(6.0);
            c.rect(w * 0.45, h * 0.1, w * 0.1, h * 0.7, col(60, 64, 74));
            let fill = if hot { p } else { 1.0 - p };
            c.rect(w * 0.47, h * 0.8 - h * 0.68 * fill, w * 0.06, h * 0.68 * fill, if hot { RED } else { COLD });
            c.circle(w * 0.5, h * 0.85, h * 0.07, if hot { RED } else { COLD });
            // Each card gets labels about its own subject (the thermometer is shared).
            let (left, right) = match name {
                "heat_body" => ("Heat exhaustion: heavy sweating", "Heatstroke: hot, dry skin"),
                "wet_bulb" => ("Dry air: sweat cools you", "Humid air: sweat can't cool you"),
                "wind_chill" => ("Calm: skin holds its warmth", "Wind: skin loses heat faster"),
                _ => ("Frostbite: numb, pale skin", "Hypothermia: shivering"),
            };
            c.label(glyphs, left, w * 0.22, h * 0.4, fs, font);
            c.label(glyphs, right, w * 0.78, h * 0.2, fs, font);
        }
        "car_heat" => {
            let p = c.phase(8.0);
            c.car(w * 0.25, ground, w * 0.5, col(80, 90, 110));
            let temp = 80.0 + p * 40.0;
            c.label(glyphs, &format!("Outside 80°F • inside {temp:.0}°F after {:.0} min", p * 60.0), w * 0.5, h * 0.15, fs * 1.1, font);
            c.circle(w * 0.85, h * 0.18, h * 0.07, YELLOW);
        }
        "snow_city" | "blizzard" | "snow_squall" => {
            let p = c.phase(10.0);
            c.house(w * 0.1, ground, w * 0.18, h * 0.3, col(150, 140, 130), col(90, 60, 50));
            c.house(w * 0.7, ground, w * 0.2, h * 0.35, col(150, 140, 130), col(90, 60, 50));
            c.car(w * 0.38, ground, w * 0.22, col(180, 40, 40));
            let depth = h * 0.1 * p;
            c.rect(0.0, ground - depth, w, depth + 4.0, WHITE);
            c.rect(w * 0.4, ground - w * 0.22 * 0.32 * 1.35 - depth * 0.6, w * 0.18, depth * 0.6 + 2.0, WHITE);
            let n = if name == "snow_city" { 60 } else { 240 };
            for k in 0..n {
                let x = ((k * 37 % 100) as f32 / 100.0 * w + c.t * if name == "snow_city" { 10.0 } else { 160.0 }) % w;
                let y = ((k * 53 % 100) as f32 / 100.0 * h + c.t * 60.0) % h;
                c.circle(x, y, 3.0, WHITE);
            }
            if name != "snow_city" {
                c.rect(0.0, 0.0, w, h, Color::from_rgba8(230, 235, 240, (p * 140.0) as u8));
                c.label(glyphs, if name == "blizzard" { "Whiteout: you can't see the road" } else { "Snow squall: sudden, brief, dangerous" }, w * 0.5, h * 0.1, fs, font);
            }
        }
        "ice_lines" | "black_ice" => {
            let p = c.phase(8.0);
            if name == "ice_lines" {
                c.rect(w * 0.1, ground - h * 0.6, w * 0.02, h * 0.6, col(110, 80, 50));
                c.rect(w * 0.88, ground - h * 0.6, w * 0.02, h * 0.6, col(110, 80, 50));
                let sag = h * (0.05 + p * 0.15);
                let pts: Vec<(f32, f32)> = (0..=30).map(|k| {
                    let f = k as f32 / 30.0;
                    (w * (0.11 + 0.78 * f), ground - h * 0.58 + (f * std::f32::consts::PI).sin() * sag)
                }).collect();
                c.line(&pts, col(200, 230, 255), 3.0 + p * 14.0);
                c.line(&pts, col(30, 30, 30), 2.0);
                c.label(glyphs, &format!("Ice {:.2} in: lines sag and snap", p * 0.75), w * 0.5, h * 0.12, fs, font);
            } else {
                c.rect(0.0, ground, w, h * 0.05, col(60, 60, 64));
                c.rect(w * 0.3, ground, w * 0.4, h * 0.05, Color::from_rgba8(200, 230, 255, (80.0 + 60.0 * (c.t * 3.0).sin().abs()) as u8));
                c.car(w * (0.05 + p * 0.5), ground, w * 0.22, col(80, 120, 200));
                c.label(glyphs, "Black ice: looks wet, is ice. Bridges freeze first", w * 0.5, h * 0.12, fs, font);
            }
        }
        "lake_effect" => {
            c.rect(w * 0.05, ground, w * 0.5, h - ground, WATER);
            c.arrow((w * 0.02, h * 0.3), (w * 0.45, h * 0.3), COLD, 6.0);
            for k in 0..3 {
                let x = w * (0.55 + 0.12 * k as f32);
                c.cloud(x, h * 0.32, w * 0.1, h * 0.06, DARK);
                for j in 0..6 {
                    let y = h * 0.35 + ((c.t * 0.5 + j as f32 * 0.17) % 1.0) * (ground - h * 0.35);
                    c.circle(x + w * 0.05 + (j as f32 * 7.0) % 20.0, y, 3.0, WHITE);
                }
            }
            c.label(glyphs, "Cold air over a warm lake: narrow, heavy snow bands", w * 0.5, h * 0.1, fs, font);
        }
        "generator_co" | "power_line" => {
            if name == "generator_co" {
                c.house(w * 0.05, ground, w * 0.3, h * 0.35, col(180, 170, 150), col(110, 70, 60));
                c.rect(w * 0.75, ground - h * 0.08, w * 0.08, h * 0.08, col(220, 160, 30));
                c.line(&[(w * 0.36, ground + 10.0), (w * 0.74, ground + 10.0)], WHITE, 3.0);
                c.label(glyphs, "At least 20 feet from the house", w * 0.55, ground + 40.0, fs, font);
                for k in 0..5 {
                    let y = ground - h * 0.1 - ((c.t * 0.4 + k as f32 * 0.2) % 1.0) * h * 0.3;
                    c.circle(w * 0.79, y, 8.0, Color::from_rgba8(160, 160, 160, 140));
                }
                c.label(glyphs, "Never in a garage or near windows", w * 0.5, h * 0.1, fs, font);
            } else {
                c.rect(w * 0.15, ground - h * 0.6, w * 0.02, h * 0.6, col(110, 80, 50));
                c.line(&[(w * 0.16, ground - h * 0.58), (w * 0.55, ground)], col(30, 30, 30), 4.0);
                let pulse = 0.5 + 0.5 * (c.t * 6.0).sin();
                c.circle(w * 0.55, ground, 10.0 + 6.0 * pulse, YELLOW);
                let mut pb = PathBuilder::new();
                pb.push_circle(w * 0.55, ground, h * 0.3);
                if let Some(path) = pb.finish() {
                    c.pm.stroke_path(&path, &rgba(RED, c.alpha), &Stroke { width: 4.0, ..Stroke::default() }, c.tf, c.mask);
                }
                c.label(glyphs, "Stay 35 feet away. Assume it is live", w * 0.5, h * 0.1, fs, font);
            }
        }
        "emergency_kit" => {
            let items = ["Water", "Food", "Flashlight", "Radio", "Batteries", "First aid", "Medicines", "Phone charger", "Blankets", "Documents"];
            let shown = ((c.phase(10.0) * items.len() as f32 * 1.2) as usize).min(items.len());
            for (i, it) in items.iter().enumerate().take(shown) {
                let (x, y) = (w * (0.12 + 0.19 * (i % 5) as f32), h * (0.3 + 0.35 * (i / 5) as f32));
                c.rect(x - w * 0.07, y - h * 0.1, w * 0.14, h * 0.2, col(50, 70, 100));
                c.label(glyphs, it, x, y + 6.0, fs * 0.8, font);
            }
            c.label(glyphs, "Build a kit before the storm", w * 0.5, h * 0.1, fs * 1.1, font);
        }
        _ => {
            c.label(glyphs, "Choose a diagram", w * 0.5, h * 0.5, fs, font);
        }
    }
}

/// Hodograph, sounding (temperature / dew point vs height) or time-height wind profile from the
/// newest model sounding at home.
fn draw_sounding(c: &mut Canvas, glyphs: &mut GlyphCache, name: &str, fs: f32, font: &str) {
    let (w, h) = (c.w, c.h);
    let Some(s) = crate::sounding::latest() else {
        c.label(glyphs, "Sounding loads with the next data refresh", w / 2.0, h / 2.0, fs, font);
        return;
    };
    let grid_c = Color::from_rgba8(255, 255, 255, 40);
    match name {
        "hodograph" => {
            let (cx, cy) = (w * 0.5, h * 0.5);
            let r = h * 0.42;
            let scale = r / 40.0; // 40 m/s to the edge
            for ring in [10.0f32, 20.0, 30.0, 40.0] {
                let mut pb = PathBuilder::new();
                pb.push_circle(cx, cy, ring * scale);
                if let Some(p) = pb.finish() {
                    c.pm.stroke_path(&p, &rgba(grid_c, c.alpha), &Stroke { width: 1.5, ..Stroke::default() }, c.tf, c.mask);
                }
            }
            c.line(&[(cx - r, cy), (cx + r, cy)], grid_c, 1.5);
            c.line(&[(cx, cy - r), (cx, cy + r)], grid_c, 1.5);
            // Coloured by height: 0-1 red, 1-3 green, 3-6 yellow, above blue.
            let reveal = (c.t / 3.0).min(1.0);
            let pts: Vec<(f32, f32, f32)> = s.levels.iter().filter(|l| l.z <= 9000.0).map(|l| (cx + l.u * scale, cy - l.v * scale, l.z)).collect();
            let shown = ((pts.len() as f32 * reveal).ceil() as usize).max(2).min(pts.len());
            for wnd in pts[..shown].windows(2) {
                let col_ = match wnd[0].2 { z if z < 1000.0 => col(230, 60, 60), z if z < 3000.0 => col(60, 200, 90), z if z < 6000.0 => col(240, 210, 60), _ => col(90, 150, 240) };
                c.line(&[(wnd[0].0, wnd[0].1), (wnd[1].0, wnd[1].1)], col_, 5.0);
            }
            let (su, sv) = s.storm;
            c.circle(cx + su * scale, cy - sv * scale, 8.0, Color::WHITE);
            c.label(glyphs, "Where the storm is heading", cx + su * scale, cy - sv * scale - 14.0, fs * 0.7, font);
            c.label(glyphs, &format!("0-1 km SRH {:.0} • 0-3 km {:.0} • 0-6 km shear {:.0} kt", s.srh01, s.srh03, s.shear06 * 1.944), w * 0.5, h * 0.06, fs * 0.85, font);
        }
        "skewt" => {
            // Temperature and dew point (°C) against height (0-12 km), with the parcel path.
            let (x0, x1, y0, y1) = (w * 0.08, w * 0.95, h * 0.92, h * 0.08);
            let tx = |t: f32, z: f32| x0 + ((t + 40.0) / 80.0) * (x1 - x0) + (z / 12000.0) * (x1 - x0) * 0.35;
            let zy = |z: f32| y0 - (z / 12000.0) * (y0 - y1);
            for k in 0..=6 {
                let z = k as f32 * 2000.0;
                c.line(&[(x0, zy(z)), (x1, zy(z))], grid_c, 1.0);
                c.label(glyphs, &format!("{} km", k * 2), x0 - 10.0, zy(z) + 6.0, fs * 0.6, font);
            }
            let levels: Vec<_> = s.levels.iter().filter(|l| l.z <= 12000.0).collect();
            let t_line: Vec<(f32, f32)> = levels.iter().map(|l| (tx(l.t, l.z), zy(l.z))).collect();
            let d_line: Vec<(f32, f32)> = levels.iter().map(|l| (tx(l.td, l.z), zy(l.z))).collect();
            c.line(&t_line, col(230, 60, 60), 5.0);
            c.line(&d_line, col(60, 200, 90), 5.0);
            // Parcel: dry to the LCL, then about 6 °C/km.
            let t0 = s.levels[0].t;
            let lcl = s.ml.lcl_m;
            let parcel: Vec<(f32, f32)> = (0..=24).map(|k| {
                let z = k as f32 * 500.0;
                let t = if z <= lcl { t0 - 9.8 * z / 1000.0 } else { t0 - 9.8 * lcl / 1000.0 - 6.0 * (z - lcl) / 1000.0 };
                (tx(t, z), zy(z))
            }).collect();
            c.line(&parcel, Color::from_rgba8(255, 255, 255, 170), 3.0);
            c.label(glyphs, &format!("MLCAPE {:.0} • LCL {:.0} ft • freezing level {}", s.ml.cape, s.ml.lcl_m * 3.281, s.freezing_m.map(|z| format!("{:.0} ft", z * 3.281)).unwrap_or_else(|| "surface".into())), w * 0.5, h * 0.05, fs * 0.8, font);
        }
        _ => {
            // Time-height wind profile (VAD-style): barbs-as-arrows per hour and height.
            let hours = s.profile.len().max(1);
            let (x0, x1, y0, y1) = (w * 0.08, w * 0.97, h * 0.9, h * 0.1);
            for (k, (t, col_levels)) in s.profile.iter().enumerate() {
                let x = x0 + (x1 - x0) * k as f32 / hours as f32;
                if k % 3 == 0 {
                    let label = chrono::DateTime::from_timestamp(*t, 0).map(|d| d.with_timezone(&chrono::Local).format("%-I %p").to_string()).unwrap_or_default();
                    c.label(glyphs, &label, x, h * 0.97, fs * 0.6, font);
                }
                for (z, u, v) in col_levels {
                    let y = y0 - (z / 9000.0) * (y0 - y1);
                    let sp = (u * u + v * v).sqrt();
                    let len = 10.0 + sp.min(40.0) * 0.8;
                    let colr = match sp * 1.944 { k if k < 20.0 => col(110, 170, 240), k if k < 40.0 => col(90, 200, 120), k if k < 60.0 => col(240, 210, 60), _ => col(230, 70, 70) };
                    c.arrow((x, y), (x + u / sp.max(0.1) * len, y - v / sp.max(0.1) * len), colr, 2.5);
                }
            }
            for km in [0, 3, 6, 9] {
                let y = y0 - (km as f32 / 9.0) * (y0 - y1);
                c.label(glyphs, &format!("{km} km"), w * 0.04, y + 6.0, fs * 0.6, font);
            }
            c.label(glyphs, "Wind by height, next 24 hours", w * 0.5, h * 0.05, fs * 0.85, font);
        }
    }
}
