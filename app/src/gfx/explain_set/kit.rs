//! Realistic drawing components for the explainers: shaded sky, ground, clouds, storm cells,
//! precipitation, trees, houses, cars, people, thermometers, water, rooms, fans, lightning.
//! Components are combined into a different scene for each topic (scenes.rs); a whole scene
//! is never reused for an unrelated topic.

use tiny_skia::{Color, PathBuilder};

use crate::gfx::history_set::paint::{ellipse, hex, linear, mix, poly, radial, rgba, rrect, smooth, solid, with_alpha, Canvas};

/// Small deterministic random numbers (same picture every frame).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407))
    }
    pub fn f(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32) / (u32::MAX >> 1) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Sky {
    Day,
    Hazy,
    Storm,
    Night,
    Dusk,
    Overcast,
    Winter,
}

pub fn sky(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, kind: Sky) {
    let (top, bottom) = match kind {
        Sky::Day => (hex("#2F7FD0"), hex("#A9D4F5")),
        Sky::Hazy => (hex("#6FA2C8"), hex("#E8E2CF")),
        Sky::Storm => (hex("#232A33"), hex("#5E6B73")),
        Sky::Night => (hex("#050B1A"), hex("#1C2D4E")),
        Sky::Dusk => (hex("#23305E"), hex("#F2A15E")),
        Sky::Overcast => (hex("#7D8892"), hex("#C3CAD0")),
        Sky::Winter => (hex("#8FA6BC"), hex("#DCE5EC")),
    };
    cv.round(x, y, w, h, 0., &linear(0., y, 0., y + h, &[(0., top), (1., bottom)]));
    if kind == Sky::Night {
        let mut r = Rng::new(7);
        for _ in 0..90 {
            let (sx, sy) = (x + r.f() * w, y + r.f() * h * 0.7);
            let s = r.range(0.8, 2.2);
            cv.fill_c(ellipse(sx, sy, s, s), rgba(255, 255, 255, r.range(0.4, 0.9)));
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Ground {
    Grass,
    Snow,
    Dirt,
    Dry,
    Ice,
}

/// Ground from `y` to the bottom of the box, with a soft horizon and texture.
pub fn ground(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, kind: Ground, seed: u64) {
    let (a, b) = match kind {
        Ground::Grass => (hex("#5E9A3A"), hex("#2E5A1E")),
        Ground::Snow => (hex("#F4F8FC"), hex("#C9D7E6")),
        Ground::Dirt => (hex("#8A6A45"), hex("#4E3A24")),
        Ground::Dry => (hex("#C9B27A"), hex("#8C7444")),
        Ground::Ice => (hex("#D9ECF7"), hex("#9EC3DB")),
    };
    cv.round(x, y, w, h, 0., &linear(0., y, 0., y + h, &[(0., a), (1., b)]));
    let mut r = Rng::new(seed);
    let blades = (w / 6.) as usize;
    for _ in 0..blades {
        let bx = x + r.f() * w;
        let by = y + 4. + r.f().powf(1.6) * (h - 6.);
        let depth = (by - y) / h.max(1.);
        match kind {
            Ground::Grass => {
                let len = 4. + depth * 12.;
                let c = mix(hex("#8CC65A"), hex("#1E3E12"), r.f() * 0.8);
                cv.line((bx, by), (bx + r.range(-3., 3.), by - len), c, 1. + depth * 1.4);
            }
            Ground::Snow | Ground::Ice => {
                let s = 0.8 + depth * 2.;
                cv.fill_c(ellipse(bx, by, s * 2., s * 0.6), rgba(255, 255, 255, 0.55));
            }
            Ground::Dirt | Ground::Dry => {
                let s = 0.8 + depth * 2.4;
                cv.fill_c(ellipse(bx, by, s, s * 0.6), rgba(40, 28, 16, 0.25));
            }
        }
    }
}

/// Puffy cloud of shaded lobes. `shade` 0 = bright white, 1 = dark storm grey.
pub fn cloud(cv: &mut Canvas, cx: f32, cy: f32, scale: f32, shade: f32, seed: u64) {
    let mut r = Rng::new(seed);
    let lobes: Vec<(f32, f32, f32)> = (0..9)
        .map(|i| {
            let t = i as f32 / 8.;
            let lx = cx + (t - 0.5) * 220. * scale + r.range(-12., 12.) * scale;
            let ly = cy - (1. - (t - 0.5).abs() * 2.).powf(0.7) * 46. * scale + r.range(-8., 8.) * scale;
            (lx, ly, r.range(42., 64.) * scale)
        })
        .collect();
    let light = mix(hex("#FFFFFF"), hex("#8C949C"), shade);
    let dark = mix(hex("#C6CED8"), hex("#3B434C"), shade);
    // Flat base, then lobes lit from the upper left.
    cv.fill(rrect(cx - 130. * scale, cy - 10. * scale, 260. * scale, 52. * scale, 26. * scale), &linear(0., cy - 10. * scale, 0., cy + 42. * scale, &[(0., light), (1., dark)]));
    for (lx, ly, lr) in &lobes {
        cv.fill(ellipse(*lx, *ly, *lr, *lr * 0.92), &radial(lx - lr * 0.35, ly - lr * 0.4, lr * 1.5, &[(0., light), (0.6, mix(light, dark, 0.45)), (1., dark)]));
    }
}

/// Cumulonimbus: tall tower with an anvil, dark base and a rain shaft below.
pub fn cumulonimbus(cv: &mut Canvas, cx: f32, base_y: f32, scale: f32, rain: bool) {
    let s = scale;
    let top = base_y - 520. * s;
    // Anvil: a flat, wind-sheared plate spreading downstream (right).
    let anvil = smooth(
        &[(cx - 300. * s, top + 62. * s), (cx - 330. * s, top + 30. * s), (cx - 120. * s, top + 4. * s), (cx + 220. * s, top), (cx + 540. * s, top + 18. * s), (cx + 580. * s, top + 44. * s), (cx + 360. * s, top + 70. * s), (cx + 120. * s, top + 96. * s), (cx - 120. * s, top + 92. * s)],
        true,
    );
    cv.fill(anvil, &linear(0., top, 0., top + 100. * s, &[(0., hex("#F7F8FA")), (0.6, hex("#D6DCE2")), (1., hex("#9EA8B3"))]));
    // Tower silhouette, lit from the left.
    let tower = smooth(
        &[(cx - 260. * s, base_y), (cx - 245. * s, base_y - 130. * s), (cx - 205. * s, base_y - 280. * s), (cx - 165. * s, top + 110. * s), (cx - 70. * s, top + 64. * s), (cx + 80. * s, top + 66. * s), (cx + 150. * s, top + 110. * s), (cx + 200. * s, base_y - 270. * s), (cx + 250. * s, base_y - 120. * s), (cx + 270. * s, base_y)],
        true,
    );
    cv.fill(tower, &linear(cx - 260. * s, 0., cx + 270. * s, 0., &[(0., hex("#F2F4F7")), (0.45, hex("#C9D0D8")), (1., hex("#7E8893"))]));
    // Cauliflower texture along both edges and inside, smaller toward the top.
    let mut r = Rng::new(31);
    for i in 0..9 {
        let t = 0.08 + i as f32 / 9. * 0.8;
        let y = base_y - t * (base_y - top - 90. * s);
        let half = (255. - t * 110.) * s;
        let lr = (64. - t * 26.) * s;
        for (side, light, dark) in [(-1f32, hex("#FBFCFD"), hex("#AEB7C1")), (1., hex("#C2CAD3"), hex("#6E7883"))] {
            let lx = cx + side * (half - lr * 0.55) + r.range(-8., 8.) * s;
            cv.fill(ellipse(lx, y, lr, lr * 0.88), &radial(lx - lr * 0.45, y - lr * 0.5, lr * 1.7, &[(0., light), (1., dark)]));
        }
        let ix = cx + r.range(-0.5, 0.4) * half;
        let ir = lr * 0.9;
        cv.fill(ellipse(ix, y - 20. * s, ir, ir * 0.8), &radial(ix - ir * 0.4, y - 20. * s - ir * 0.5, ir * 1.6, &[(0., rgba(250, 251, 253, 0.65)), (1., rgba(150, 160, 172, 0.0))]));
    }
    // Overshooting top.
    cv.fill(ellipse(cx - 10. * s, top + 40. * s, 110. * s, 46. * s), &radial(cx - 40. * s, top + 20. * s, 140. * s, &[(0., hex("#FFFFFF")), (1., hex("#BCC4CD"))]));
    // Dark, flat base.
    cv.fill(rrect(cx - 285. * s, base_y - 46. * s, 570. * s, 62. * s, 30. * s), &linear(0., base_y - 46. * s, 0., base_y + 16. * s, &[(0., hex("#6A727B")), (1., hex("#2B3138"))]));
    if rain {
        let top = base_y + 5. * s;
        cv.fill(poly(&[(cx - 220. * s, top), (cx + 40. * s, top), (cx + 10. * s, top + 420. * s), (cx - 260. * s, top + 420. * s)], true), &linear(0., top, 0., top + 420. * s, &[(0., rgba(70, 80, 92, 0.75)), (1., rgba(70, 80, 92, 0.15))]));
    }
}

pub fn rain(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, n: usize, slant: f32, seed: u64) {
    let mut r = Rng::new(seed);
    for _ in 0..n {
        let (rx, ry) = (x + r.f() * w, y + r.f() * h);
        let len = r.range(14., 26.);
        cv.line((rx, ry), (rx + slant * len, ry + len), rgba(200, 220, 240, r.range(0.35, 0.7)), 1.6);
    }
}

pub fn snow(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, n: usize, seed: u64) {
    let mut r = Rng::new(seed);
    for _ in 0..n {
        let (sx, sy) = (x + r.f() * w, y + r.f() * h);
        let s = r.range(2., 5.);
        cv.fill(ellipse(sx, sy, s, s), &radial(sx, sy, s, &[(0., rgba(255, 255, 255, 0.95)), (1., rgba(255, 255, 255, 0.1))]));
    }
}

/// Ice pellets: small clear beads with a highlight.
pub fn sleet(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, n: usize, seed: u64) {
    let mut r = Rng::new(seed);
    for _ in 0..n {
        let (sx, sy) = (x + r.f() * w, y + r.f() * h);
        let s = r.range(2.5, 4.5);
        cv.fill(ellipse(sx, sy, s, s), &radial(sx - s * 0.3, sy - s * 0.3, s * 1.3, &[(0., rgba(255, 255, 255, 0.95)), (0.5, rgba(190, 220, 240, 0.8)), (1., rgba(120, 160, 190, 0.7))]));
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tree {
    Leafy,
    Pine,
    Bare,
    Autumn,
}

/// A tree standing on `gy`; `lean` bends the crown sideways (wind).
pub fn tree(cv: &mut Canvas, x: f32, gy: f32, h: f32, kind: Tree, lean: f32, seed: u64) {
    let tw = h * 0.07;
    let top = (x + lean * h * 0.25, gy - h * 0.55);
    cv.fill(poly(&[(x - tw, gy), (x + tw, gy), (top.0 + tw * 0.4, top.1), (top.0 - tw * 0.4, top.1)], true), &linear(x - tw, 0., x + tw, 0., &[(0., hex("#6B4A2E")), (1., hex("#3A2716"))]));
    let mut r = Rng::new(seed);
    match kind {
        Tree::Pine => {
            for i in 0..5 {
                let t = i as f32 / 4.;
                let ty = gy - h * (0.2 + t * 0.72);
                let half = h * (0.32 - t * 0.22);
                let cx = x + lean * h * 0.3 * (t + 0.2);
                cv.fill(poly(&[(cx, ty - h * 0.22), (cx + half, ty + h * 0.04), (cx - half, ty + h * 0.04)], true), &linear(cx - half, 0., cx + half, 0., &[(0., hex("#2F6B3A")), (1., hex("#163A1E"))]));
            }
        }
        Tree::Bare => {
            for i in 0..7 {
                let a = -1.4 + i as f32 * 0.47 + lean * 0.4;
                let len = h * r.range(0.22, 0.34);
                let start = (top.0, top.1 + r.range(0., h * 0.15));
                let end = (start.0 + a.sin() * len, start.1 - a.cos().abs() * len);
                cv.line(start, end, hex("#4A3423"), tw * 0.45);
                cv.line(end, (end.0 + a.sin() * len * 0.4 + r.range(-8., 8.), end.1 - len * 0.35), hex("#4A3423"), tw * 0.22);
            }
        }
        Tree::Leafy | Tree::Autumn => {
            let (light, dark) = if kind == Tree::Autumn { (hex("#F2A73B"), hex("#A8481C")) } else { (hex("#6DB24A"), hex("#245C22")) };
            for _ in 0..11 {
                let lx = top.0 + r.range(-0.26, 0.26) * h + lean * h * 0.12;
                let ly = top.1 - r.range(0., 0.32) * h;
                let lr = h * r.range(0.11, 0.17);
                cv.fill(ellipse(lx, ly, lr, lr * 0.9), &radial(lx - lr * 0.4, ly - lr * 0.5, lr * 1.6, &[(0., light), (1., dark)]));
            }
        }
    }
}

pub struct HouseStyle {
    pub wall: &'static str,
    pub roof: &'static str,
    pub lit: bool,
}

/// A two-storey house, side-lit, sitting on `gy`. Returns (left, top, width, height) of the walls.
pub fn house(cv: &mut Canvas, x: f32, gy: f32, w: f32, h: f32, st: &HouseStyle) -> (f32, f32, f32, f32) {
    let wall = hex(st.wall);
    let top = gy - h;
    cv.fill(rrect(x + 10., gy - 6., w, 14., 4.), &solid(rgba(0, 0, 0, 0.25)));
    cv.fill(rrect(x, top, w, h, 0.), &linear(x, 0., x + w, 0., &[(0., mix(wall, Color::WHITE, 0.12)), (1., mix(wall, Color::BLACK, 0.18))]));
    // Siding lines.
    let mut y = top + 14.;
    while y < gy - 4. {
        cv.line((x + 2., y), (x + w - 2., y), with_alpha(mix(wall, Color::BLACK, 0.3), 0.35), 1.);
        y += 14.;
    }
    // Roof.
    let rh = h * 0.55;
    cv.fill(poly(&[(x - w * 0.08, top + 4.), (x + w / 2., top - rh), (x + w * 1.08, top + 4.)], true), &linear(0., top - rh, 0., top, &[(0., mix(hex(st.roof), Color::WHITE, 0.1)), (1., mix(hex(st.roof), Color::BLACK, 0.3))]));
    cv.line((x - w * 0.08, top + 4.), (x + w * 1.08, top + 4.), rgba(0, 0, 0, 0.35), 3.);
    // Windows and door.
    let win = |cv: &mut Canvas, wx: f32, wy: f32, ww: f32, wh: f32| {
        cv.fill(rrect(wx - 4., wy - 4., ww + 8., wh + 8., 2.), &solid(hex("#F4F1EA")));
        let glass = if st.lit { linear(0., wy, 0., wy + wh, &[(0., hex("#FFE7A3")), (1., hex("#F2B95A"))]) } else { linear(wx, wy, wx + ww, wy + wh, &[(0., hex("#9CC3DE")), (0.5, hex("#3E6585")), (1., hex("#203A52"))]) };
        cv.fill(rrect(wx, wy, ww, wh, 1.), &glass);
        cv.line((wx + ww / 2., wy), (wx + ww / 2., wy + wh), hex("#F4F1EA"), 3.);
        cv.line((wx, wy + wh / 2.), (wx + ww, wy + wh / 2.), hex("#F4F1EA"), 3.);
    };
    let (ww, wh) = (w * 0.16, h * 0.2);
    win(cv, x + w * 0.12, top + h * 0.14, ww, wh);
    win(cv, x + w * 0.72, top + h * 0.14, ww, wh);
    win(cv, x + w * 0.12, top + h * 0.6, ww, wh);
    cv.fill(rrect(x + w * 0.62, gy - h * 0.38, w * 0.16, h * 0.38, 2.), &linear(0., gy - h * 0.38, 0., gy, &[(0., hex("#7A2E22")), (1., hex("#4A1A12"))]));
    cv.fill(ellipse(x + w * 0.75, gy - h * 0.19, 3., 3.), &solid(hex("#E8C46A")));
    (x, top, w, h)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Car {
    Sedan,
    Suv,
    Pickup,
}

/// Side view car facing right, wheels on `gy`.
pub fn car(cv: &mut Canvas, x: f32, gy: f32, w: f32, body: &str, kind: Car) {
    let c = hex(body);
    let h = w * if kind == Car::Sedan { 0.3 } else { 0.4 };
    let wheel_r = w * if kind == Car::Sedan { 0.085 } else { 0.1 };
    let base = gy - wheel_r * 0.9;
    cv.fill(ellipse(x + w / 2., gy + 3., w * 0.52, 8.), &solid(rgba(0, 0, 0, 0.3)));
    // Lower body.
    let body_top = base - h * 0.5;
    cv.fill(rrect(x, body_top, w, h * 0.5, h * 0.12), &linear(0., body_top, 0., base, &[(0., mix(c, Color::WHITE, 0.25)), (0.5, c), (1., mix(c, Color::BLACK, 0.35))]));
    // Cabin.
    let cab: Vec<(f32, f32)> = match kind {
        Car::Sedan => vec![(x + w * 0.2, body_top), (x + w * 0.32, base - h), (x + w * 0.66, base - h), (x + w * 0.8, body_top)],
        Car::Suv => vec![(x + w * 0.08, body_top), (x + w * 0.14, base - h), (x + w * 0.74, base - h), (x + w * 0.86, body_top)],
        Car::Pickup => vec![(x + w * 0.38, body_top), (x + w * 0.44, base - h), (x + w * 0.7, base - h), (x + w * 0.8, body_top)],
    };
    cv.fill(poly(&cab, true), &linear(0., base - h, 0., body_top, &[(0., mix(c, Color::WHITE, 0.15)), (1., c)]));
    // Glass.
    let inset = |p: (f32, f32), k: f32| (p.0 + (x + w * 0.5 - p.0) * k, p.1 + 7.);
    let glass: Vec<(f32, f32)> = cab.iter().map(|p| inset(*p, 0.07)).collect();
    cv.fill(poly(&glass, true), &linear(glass[1].0, glass[1].1, glass[2].0, glass[0].1, &[(0., hex("#CFE6F5")), (0.45, hex("#5F86A3")), (1., hex("#22384C"))]));
    let mid = (glass[1].0 + glass[2].0) / 2.;
    cv.line((mid, glass[1].1), (mid, body_top), mix(c, Color::BLACK, 0.2), 5.);
    // Lights, handle, wheels.
    cv.fill(rrect(x + w - 10., body_top + 6., 10., h * 0.12, 3.), &solid(hex("#FFF4C2")));
    cv.fill(rrect(x, body_top + 6., 8., h * 0.12, 3.), &solid(hex("#C8242A")));
    cv.line((x + w * 0.5, body_top + h * 0.12), (x + w * 0.56, body_top + h * 0.12), mix(c, Color::BLACK, 0.4), 3.);
    for wx in [x + w * 0.2, x + w * 0.8] {
        cv.fill(ellipse(wx, gy - wheel_r, wheel_r * 1.15, wheel_r * 1.15), &solid(rgba(15, 15, 18, 1.)));
        cv.fill(ellipse(wx, gy - wheel_r, wheel_r, wheel_r), &radial(wx, gy - wheel_r, wheel_r, &[(0., hex("#2A2A2E")), (0.7, hex("#18181B")), (1., hex("#0C0C0E"))]));
        cv.fill(ellipse(wx, gy - wheel_r, wheel_r * 0.5, wheel_r * 0.5), &radial(wx - 3., gy - wheel_r - 3., wheel_r * 0.6, &[(0., hex("#E6E9EE")), (1., hex("#7D848E"))]));
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Pose {
    Stand,
    Walk,
    Shovel,
    Crouch,
    Child,
}

/// A person silhouette (shaded, not a flat icon) standing on `gy`, `h` tall.
pub fn person(cv: &mut Canvas, x: f32, gy: f32, h: f32, pose: Pose, coat: &str) {
    let h = if pose == Pose::Child { h * 0.62 } else { h };
    let c = hex(coat);
    let skin = hex("#C68E6A");
    let head_r = h * 0.075;
    let (hx, hy) = match pose {
        Pose::Shovel => (x + h * 0.08, gy - h + head_r + h * 0.08),
        Pose::Crouch => (x + h * 0.05, gy - h * 0.62),
        _ => (x, gy - h + head_r),
    };
    let leg = hex("#2B3445");
    match pose {
        Pose::Walk => {
            cv.line((x, gy - h * 0.48), (x - h * 0.12, gy), leg, h * 0.07);
            cv.line((x, gy - h * 0.48), (x + h * 0.12, gy), mix(leg, Color::BLACK, 0.3), h * 0.07);
        }
        Pose::Crouch => {
            cv.line((x, gy - h * 0.3), (x + h * 0.14, gy - h * 0.12), leg, h * 0.08);
            cv.line((x + h * 0.14, gy - h * 0.12), (x + h * 0.05, gy), leg, h * 0.08);
        }
        _ => {
            cv.line((x - h * 0.04, gy - h * 0.48), (x - h * 0.05, gy), leg, h * 0.07);
            cv.line((x + h * 0.04, gy - h * 0.48), (x + h * 0.06, gy), mix(leg, Color::BLACK, 0.3), h * 0.07);
        }
    }
    let torso_top = hy + head_r * 1.1;
    let torso_bottom = if pose == Pose::Crouch { gy - h * 0.28 } else { gy - h * 0.45 };
    let lean = if pose == Pose::Shovel { h * 0.1 } else { 0. };
    cv.fill(
        poly(&[(hx - h * 0.1, torso_top + 6.), (hx + h * 0.1, torso_top + 6.), (x + h * 0.09, torso_bottom), (x - h * 0.09 - lean * 0.2, torso_bottom)], true),
        &linear(x - h * 0.1, 0., x + h * 0.1, 0., &[(0., mix(c, Color::WHITE, 0.2)), (1., mix(c, Color::BLACK, 0.3))]),
    );
    cv.fill(ellipse(hx, torso_top + 8., h * 0.1, h * 0.05), &solid(c));
    // Arms.
    let shoulder = (hx + h * 0.06, torso_top + h * 0.06);
    match pose {
        Pose::Shovel => {
            let hand = (x + h * 0.32, gy - h * 0.32);
            cv.line(shoulder, hand, c, h * 0.06);
            cv.line(hand, (x + h * 0.55, gy - h * 0.02), hex("#8A6A45"), h * 0.025);
            cv.fill(poly(&[(x + h * 0.5, gy - h * 0.04), (x + h * 0.68, gy - h * 0.06), (x + h * 0.66, gy), (x + h * 0.5, gy)], true), &solid(hex("#3E5E8E")));
        }
        Pose::Walk => {
            cv.line(shoulder, (x + h * 0.12, gy - h * 0.5), c, h * 0.06);
            cv.line((hx - h * 0.06, torso_top + h * 0.06), (x - h * 0.13, gy - h * 0.52), mix(c, Color::BLACK, 0.25), h * 0.06);
        }
        _ => {
            cv.line(shoulder, (x + h * 0.11, gy - h * 0.48), c, h * 0.06);
            cv.line((hx - h * 0.06, torso_top + h * 0.06), (x - h * 0.11, gy - h * 0.48), mix(c, Color::BLACK, 0.25), h * 0.06);
        }
    }
    cv.fill(ellipse(hx, hy, head_r, head_r * 1.08), &radial(hx - head_r * 0.3, hy - head_r * 0.3, head_r * 1.5, &[(0., mix(skin, Color::WHITE, 0.2)), (1., mix(skin, Color::BLACK, 0.3))]));
    // Hair / hat.
    cv.fill(ellipse(hx, hy - head_r * 0.45, head_r * 1.02, head_r * 0.62), &solid(hex("#3A2A1E")));
}

/// Glass thermometer; `frac` 0..1 fill, liquid colour.
pub fn thermometer(cv: &mut Canvas, x: f32, y: f32, h: f32, frac: f32, liquid: &str) {
    let w = h * 0.12;
    cv.fill(rrect(x - w / 2., y, w, h, w / 2.), &linear(x - w / 2., 0., x + w / 2., 0., &[(0., rgba(255, 255, 255, 0.95)), (1., rgba(190, 200, 210, 0.95))]));
    let bulb_r = w * 0.95;
    let by = y + h + bulb_r * 0.4;
    let l = hex(liquid);
    cv.fill(ellipse(x, by, bulb_r, bulb_r), &radial(x - bulb_r * 0.3, by - bulb_r * 0.3, bulb_r * 1.4, &[(0., mix(l, Color::WHITE, 0.4)), (1., mix(l, Color::BLACK, 0.2))]));
    let tube = w * 0.36;
    let top = y + w * 0.6 + (h - w * 0.6) * (1. - frac.clamp(0., 1.));
    cv.fill(rrect(x - tube / 2., top, tube, by - top, tube / 2.), &solid(l));
    for i in 0..=10 {
        let ty = y + w * 0.6 + (h - w * 0.6) * i as f32 / 10.;
        cv.line((x + w * 0.5, ty), (x + w * if i % 5 == 0 { 0.95 } else { 0.75 }, ty), rgba(40, 50, 60, 0.8), 2.);
    }
    cv.line((x - w * 0.25, y + w * 0.5), (x - w * 0.25, y + h - w * 0.3), rgba(255, 255, 255, 0.8), 2.5);
}

/// Translucent water from `y` down with a wavy top and highlight.
pub fn water(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, muddy: bool, seed: u64) {
    let mut r = Rng::new(seed);
    let mut pts = vec![(x, y + h)];
    let n = (w / 30.) as usize + 2;
    for i in 0..=n {
        pts.push((x + w * i as f32 / n as f32, y + r.range(-5., 5.)));
    }
    pts.push((x + w, y + h));
    let (top, bottom) = if muddy { (rgba(150, 118, 72, 0.82), rgba(92, 70, 40, 0.92)) } else { (rgba(70, 140, 190, 0.7), rgba(25, 70, 110, 0.85)) };
    cv.fill(poly(&pts, true), &linear(0., y, 0., y + h, &[(0., top), (1., bottom)]));
    for _ in 0..(w / 60.) as usize {
        let (wx, wy) = (x + r.f() * w, y + 8. + r.f() * (h - 12.).max(1.) * 0.5);
        cv.line((wx, wy), (wx + r.range(20., 50.), wy), rgba(255, 255, 255, 0.25), 2.);
    }
}

/// Jagged lightning bolt with glow from a to b.
pub fn bolt(cv: &mut Canvas, a: (f32, f32), b: (f32, f32), seed: u64) {
    let mut r = Rng::new(seed);
    let mut pts = vec![a];
    let n = 12;
    for i in 1..n {
        let t = i as f32 / n as f32;
        pts.push((a.0 + (b.0 - a.0) * t + r.range(-28., 28.), a.1 + (b.1 - a.1) * t));
    }
    pts.push(b);
    for (wd, al) in [(26., 0.10), (14., 0.22), (7., 0.5)] {
        cv.stroke(poly(&pts, false), rgba(190, 170, 255, al), wd);
    }
    cv.stroke(poly(&pts, false), rgba(255, 255, 255, 1.), 3.5);
    // A branch.
    let k = pts.len() / 3;
    let p = pts[k];
    cv.stroke(poly(&[p, (p.0 + 40., p.1 + 50.), (p.0 + 30., p.1 + 90.), (p.0 + 70., p.1 + 130.)], false), rgba(230, 225, 255, 0.8), 2.);
}

/// Room interior: wall, crown moulding, baseboard, wooden floor, optional window.
pub fn room(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, wall: &str, window: bool, cold_outside: bool) -> f32 {
    let floor_y = y + h * 0.78;
    let wc = hex(wall);
    cv.round(x, y, w, floor_y - y, 0., &linear(0., y, 0., floor_y, &[(0., mix(wc, Color::BLACK, 0.12)), (1., mix(wc, Color::WHITE, 0.08))]));
    // Ceiling strip and crown moulding.
    cv.round(x, y, w, 26., 0., &linear(0., y, 0., y + 26., &[(0., hex("#F7F5F0")), (1., hex("#D9D4CA"))]));
    cv.line((x, y + 26.), (x + w, y + 26.), rgba(0, 0, 0, 0.18), 3.);
    cv.round(x, y + 28., w, 8., 0., &solid(hex("#ECE8E0")));
    // Floor boards in perspective.
    cv.round(x, floor_y, w, y + h - floor_y, 0., &linear(0., floor_y, 0., y + h, &[(0., hex("#B98A58")), (1., hex("#7A5430"))]));
    let mut r = Rng::new(11);
    for i in 0..18 {
        let fx = x + w * i as f32 / 17.;
        let bx = x + w / 2. + (fx - x - w / 2.) * 1.9;
        cv.line((fx, floor_y), (bx, y + h), rgba(60, 36, 16, 0.35), 1.5);
    }
    for _ in 0..30 {
        let (gx, gy) = (x + r.f() * w, floor_y + r.f() * (y + h - floor_y));
        cv.line((gx, gy), (gx + r.range(20., 60.), gy), rgba(70, 44, 20, 0.18), 1.);
    }
    cv.round(x, floor_y - 18., w, 18., 0., &linear(0., floor_y - 18., 0., floor_y, &[(0., hex("#F4F1EA")), (1., hex("#CFC8BA"))]));
    if window {
        let (wx, wy, ww, wh) = (x + w * 0.70, y + h * 0.18, w * 0.16, h * 0.42);
        cv.fill(rrect(wx - 12., wy - 12., ww + 24., wh + 24., 3.), &solid(hex("#F7F5F0")));
        let (a, b) = if cold_outside { (hex("#A9BED1"), hex("#E9EFF4")) } else { (hex("#4F9BE0"), hex("#BFE2F7")) };
        cv.fill(rrect(wx, wy, ww, wh, 2.), &linear(0., wy, 0., wy + wh, &[(0., a), (1., b)]));
        if cold_outside {
            snow(cv, wx, wy, ww, wh, 40, 5);
        } else {
            cloud(cv, wx + ww * 0.5, wy + wh * 0.3, 0.22, 0., 3);
        }
        cv.line((wx + ww / 2., wy), (wx + ww / 2., wy + wh), hex("#F7F5F0"), 6.);
        cv.line((wx, wy + wh / 2.), (wx + ww, wy + wh / 2.), hex("#F7F5F0"), 6.);
        cv.round(wx - 20., wy + wh + 10., ww + 40., 10., 2., &solid(hex("#E4DFD5")));
    }
    floor_y
}

/// Ceiling fan seen slightly from below: downrod, motor housing, five blades with light kit.
pub fn ceiling_fan(cv: &mut Canvas, cx: f32, ceil_y: f32, span: f32) {
    let rod = span * 0.18;
    cv.fill(rrect(cx - 18., ceil_y, 36., 12., 6.), &solid(hex("#3B3B3E")));
    cv.line((cx, ceil_y + 10.), (cx, ceil_y + rod), hex("#2A2A2D"), 8.);
    let hy = ceil_y + rod + 20.;
    // Blades: ellipses foreshortened (seen from slightly below).
    for i in 0..5 {
        let a = i as f32 * std::f32::consts::TAU / 5. + 0.3;
        let (dx, dy) = (a.cos() * span * 0.5, a.sin() * span * 0.08);
        let tip = (cx + dx, hy + dy);
        let mid = (cx + dx * 0.55, hy + dy * 0.55);
        let blade = smooth(&[(cx + dx * 0.12, hy - 6.), (mid.0, mid.1 - 12.), (tip.0, tip.1 - 8.), (tip.0 + a.cos() * 8., tip.1 + 2.), (mid.0, mid.1 + 10.), (cx + dx * 0.12, hy + 6.)], true);
        let shade = 0.15 + 0.5 * ((a.sin() + 1.) / 2.);
        cv.fill(blade, &linear(cx, hy - 12., cx, hy + 12., &[(0., mix(hex("#9A6B42"), Color::WHITE, 0.1)), (1., mix(hex("#5E3D22"), Color::BLACK, shade))]));
    }
    cv.fill(ellipse(cx, hy, 46., 26.), &radial(cx - 12., hy - 10., 56., &[(0., hex("#6A6A70")), (1., hex("#232326"))]));
    cv.fill(ellipse(cx, hy + 30., 30., 22.), &radial(cx - 6., hy + 24., 34., &[(0., hex("#FFF8E6")), (1., hex("#D9CBA6"))]));
}

/// Sun with soft glow.
pub fn sun(cv: &mut Canvas, cx: f32, cy: f32, r: f32) {
    cv.fill(ellipse(cx, cy, r * 3., r * 3.), &radial(cx, cy, r * 3., &[(0., rgba(255, 240, 180, 0.55)), (1., rgba(255, 240, 180, 0.))]));
    cv.fill(ellipse(cx, cy, r, r), &radial(cx - r * 0.3, cy - r * 0.3, r * 1.3, &[(0., hex("#FFFBE0")), (0.6, hex("#FFD54A")), (1., hex("#F5A623"))]));
}

/// A road from left to right with lane markings, at `y`, `h` thick.
pub fn road(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, wet: bool) {
    cv.round(x, y, w, h, 0., &linear(0., y, 0., y + h, &[(0., hex("#4A4D52")), (1., hex("#2C2E31"))]));
    let mut dx = x + 20.;
    while dx < x + w {
        cv.round(dx, y + h / 2. - 3., 60., 6., 2., &solid(hex("#F2D04B")));
        dx += 110.;
    }
    if wet {
        let mut r = Rng::new(17);
        for _ in 0..(w / 40.) as usize {
            let (px, py) = (x + r.f() * w, y + r.f() * h);
            cv.fill(ellipse(px, py, r.range(20., 60.), r.range(3., 6.)), &solid(rgba(180, 200, 220, 0.22)));
        }
    }
}

/// Simple vertical arrow-head helper for air-flow arrows (curved).
pub fn flow(cv: &mut Canvas, pts: &[(f32, f32)], c: Color, width: f32) {
    if pts.len() < 2 {
        return;
    }
    let n = pts.len();
    cv.stroke(smooth(&pts[..n - 1].iter().copied().chain(std::iter::once(pts[n - 1])).collect::<Vec<_>>(), false), c, width);
    let (a, b) = (pts[n - 2], pts[n - 1]);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(0.01);
    let (ux, uy) = (dx / len, dy / len);
    let head = width * 3.;
    let base = (b.0 - ux * head, b.1 - uy * head);
    let (nx, ny) = (-uy * head * 0.65, ux * head * 0.65);
    cv.fill_c(poly(&[(b.0 + ux * 2., b.1 + uy * 2.), (base.0 + nx, base.1 + ny), (base.0 - nx, base.1 - ny)], true), c);
}

/// Pipe segment (copper or PVC) between two points with highlight.
pub fn pipe(cv: &mut Canvas, a: (f32, f32), b: (f32, f32), d: f32, copper: bool) {
    let (base, hi) = if copper { (hex("#B4683A"), hex("#F2B48A")) } else { (hex("#D9DDE2"), hex("#FFFFFF")) };
    cv.line(a, b, mix(base, Color::BLACK, 0.35), d);
    cv.line(a, b, base, d * 0.8);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(0.01);
    let (nx, ny) = (-dy / len * d * 0.2, dx / len * d * 0.2);
    cv.line((a.0 + nx, a.1 + ny), (b.0 + nx, b.1 + ny), with_alpha(hi, 0.7), d * 0.18);
}

/// Generic boxy appliance body with rounded corners and sheen.
pub fn appliance(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, color: &str) {
    let c = hex(color);
    cv.fill(rrect(x + 6., y + h - 4., w, 10., 4.), &solid(rgba(0, 0, 0, 0.3)));
    cv.fill(rrect(x, y, w, h, w.min(h) * 0.08), &linear(x, y, x + w, y + h, &[(0., mix(c, Color::WHITE, 0.25)), (0.6, c), (1., mix(c, Color::BLACK, 0.35))]));
}

/// A mounting hill / mound silhouette across the box at `y` (for horizons).
pub fn hills(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, color: &str, seed: u64) {
    let mut r = Rng::new(seed);
    let mut pts = vec![(x, y + h)];
    let n = 8;
    for i in 0..=n {
        pts.push((x + w * i as f32 / n as f32, y + r.range(0., h * 0.6)));
    }
    pts.push((x + w, y + h));
    cv.fill(smooth(&pts, true), &linear(0., y, 0., y + h, &[(0., hex(color)), (1., mix(hex(color), Color::BLACK, 0.3))]));
}

/// Hailstone with onion-like layers (cut view when `cut`).
pub fn hailstone(cv: &mut Canvas, cx: f32, cy: f32, r: f32, cut: bool, seed: u64) {
    let mut rng = Rng::new(seed);
    let lumpy = |rr: f32, rng: &mut Rng| -> Vec<(f32, f32)> {
        (0..14)
            .map(|i| {
                let a = i as f32 * std::f32::consts::TAU / 14.;
                let k = rr * rng.range(0.9, 1.08);
                (cx + a.cos() * k, cy + a.sin() * k)
            })
            .collect()
    };
    let outer = lumpy(r, &mut rng);
    cv.fill(smooth(&outer, true), &radial(cx - r * 0.35, cy - r * 0.35, r * 1.5, &[(0., hex("#FFFFFF")), (0.5, hex("#DCEBF5")), (1., hex("#93B5CC"))]));
    if cut {
        for (i, k) in [0.78, 0.56, 0.34].iter().enumerate() {
            let ring = lumpy(r * k, &mut rng);
            let c = if i % 2 == 0 { rgba(245, 250, 255, 0.95) } else { rgba(170, 205, 230, 0.9) };
            cv.fill_c(smooth(&ring, true), c);
        }
        cv.fill(ellipse(cx, cy, r * 0.16, r * 0.16), &solid(hex("#E9F3FA")));
    }
}

/// Builds a closed shape from `pts` (convenience re-export for scenes).
pub fn shape(pts: &[(f32, f32)]) -> Option<tiny_skia::Path> {
    smooth(pts, true)
}

pub fn pb() -> PathBuilder {
    PathBuilder::new()
}
