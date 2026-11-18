//! Scenes, part 1: severe storms, lightning, flooding. Canvas is 1920 × 832 design units.

use tiny_skia::Color;

use super::kit::*;
use super::{divider, tag, tag_at};
use crate::gfx::history_set::paint::{ellipse, hex, linear, mix, poly, radial, rgba, rrect, smooth, solid, Al, Canvas};
use crate::gfx::text::Weight;

pub fn tornado_forms(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Storm);
    hills(cv, 0., 640., 1920., 80., "#3E5032", 4);
    ground(cv, 0., 700., 1920., 132., Ground::Grass, 9);
    // Storm base spanning the frame, darker under the updraft.
    let base_y = 330.;
    cv.fill(smooth(&[(-40., 120.), (500., 60.), (1200., 40.), (1960., 90.), (1960., base_y), (1500., base_y + 20.), (900., base_y + 10.), (300., base_y + 30.), (-40., base_y)], true), &linear(0., 40., 0., base_y + 30., &[(0., hex("#6E7880")), (1., hex("#2C3238"))]));
    // Rain and hail shaft (forward side, left).
    cv.fill(poly(&[(160., base_y + 20.), (560., base_y + 20.), (500., 700.), (80., 700.)], true), &linear(0., base_y, 0., 700., &[(0., rgba(60, 70, 82, 0.85)), (1., rgba(80, 92, 104, 0.35))]));
    rain(cv, 90., base_y + 30., 460., 360., 260, -0.15, 3);
    // Wall cloud lowering under the updraft.
    let (wx, wy) = (1080., base_y + 10.);
    cv.fill(smooth(&[(wx - 200., wy), (wx + 200., wy), (wx + 170., wy + 70.), (wx + 60., wy + 95.), (wx - 80., wy + 92.), (wx - 180., wy + 60.)], true), &linear(0., wy, 0., wy + 95., &[(0., hex("#3A4148")), (1., hex("#252A2F"))]));
    // Tornado funnel with a debris cloud.
    let top = wy + 90.;
    let funnel = smooth(&[(wx - 70., top), (wx + 60., top), (wx + 22., top + 150.), (wx + 10., 690.), (wx - 14., 690.), (wx - 34., top + 150.)], true);
    cv.fill(funnel, &linear(wx - 70., 0., wx + 60., 0., &[(0., hex("#4A4F55")), (0.5, hex("#7C838A")), (1., hex("#3A3F45"))]));
    for i in 0..9 {
        let a = i as f32 * 0.7;
        cv.fill(ellipse(wx + a.cos() * 60., 690. - (a.sin() * 18.).abs(), 46. - i as f32 * 2., 22.), &solid(rgba(110, 92, 70, 0.55)));
    }
    // Rotating updraft: spiral arrows.
    for k in 0..3 {
        let y = 260. - k as f32 * 70.;
        flow(cv, &[(wx - 160., y + 30.), (wx - 60., y - 10.), (wx + 90., y + 5.), (wx + 150., y + 40.)], rgba(255, 214, 90, 0.95), 7.);
    }
    flow(cv, &[(wx + 40., 470.), (wx + 30., 380.), (wx + 10., 120.)], rgba(255, 214, 90, 0.95), 9.);
    // Warm, moist inflow from the right, low.
    flow(cv, &[(1860., 640.), (1600., 610.), (1300., 520.), (1160., 430.)], rgba(255, 120, 80, 0.95), 9.);
    // Rear-flank downdraft wrapping around the back (right of the funnel, high to low).
    flow(cv, &[(1420., 300.), (1380., 420.), (1260., 560.), (1150., 620.)], rgba(120, 190, 255, 0.95), 8.);
    tag(cv, "Rotating updraft", (wx - 330., 70.), Some((wx - 60., 200.)), "#FFD34D");
    tag(cv, "Wall cloud", (wx - 520., wy + 40.), Some((wx - 170., wy + 50.)), "#FFFFFF");
    tag(cv, "Tornado", (wx - 380., 560.), Some((wx - 20., 560.)), "#FF6B5A");
    tag(cv, "Warm, moist inflow", (1500., 690.), Some((1600., 615.)), "#FFB08A");
    tag(cv, "Rear-flank downdraft", (1440., 230.), Some((1415., 310.)), "#9FD2FF");
    tag(cv, "Rain and hail", (170., 250.), Some((300., base_y + 70.)), "#D7DEE6");
}

pub fn hail_growth(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Day);
    ground(cv, 0., 740., 1920., 92., Ground::Grass, 2);
    cumulonimbus(cv, 760., 560., 0.98, true);
    // Freezing level.
    let fz = 330.;
    cv.dashed(poly(&[(200., fz), (1360., fz)], false), rgba(255, 255, 255, 0.95), 4., 22., 14.);
    cv.text("32°F (0°C) freezing level", 1360., fz - 14., 28., Weight::Bold, Color::WHITE, Al::Right, 0.);
    // Updraft core.
    for dx in [-40., 30.] {
        flow(cv, &[(760. + dx, 700.), (760. + dx + 10., 520.), (760. + dx, 260.)], rgba(255, 120, 80, 0.9), 10.);
    }
    // Hailstone path: loops up and down through the freezing level, growing each time.
    let path = [(900., 460.), (980., 320.), (1000., 200.), (1080., 170.), (1120., 260.), (1110., 380.), (1060., 450.), (1000., 380.), (1010., 240.), (1080., 120.), (1170., 150.), (1200., 300.), (1220., 520.), (1240., 720.)];
    cv.dashed(smooth(&path, false), rgba(255, 255, 255, 0.85), 4., 10., 10.);
    for (i, &(x, y)) in path.iter().enumerate().step_by(3) {
        hailstone(cv, x, y, 8. + i as f32 * 1.8, false, i as u64);
    }
    hailstone(cv, 1240., 722., 36., false, 99);
    // Cut-open stone.
    cv.round(1430., 330., 420., 380., 24., &solid(rgba(10, 20, 35, 0.78)));
    hailstone(cv, 1640., 500., 130., true, 5);
    cv.text("Cut open: layers of clear and cloudy ice", 1640., 690., 26., Weight::SemiBold, Color::WHITE, Al::Center, 400.);
    tag(cv, "Strong updraft", (420., 600.), Some((720., 600.)), "#FFB08A");
    tag(cv, "Stone rises and falls, adding ice", (1250., 70.), Some((1150., 140.)), "#FFD34D");
    tag(cv, "Falls when too heavy for the updraft", (1260., 760.), Some((1240., 700.)), "#FFFFFF");
}

/// Coin with a rim and embossed face.
fn coin(cv: &mut Canvas, cx: f32, cy: f32, r: f32, copper: bool) {
    let (a, b) = if copper { (hex("#E7A271"), hex("#8C4A22")) } else { (hex("#F2F4F6"), hex("#8E979F")) };
    cv.fill(ellipse(cx + 3., cy + 5., r, r), &solid(rgba(0, 0, 0, 0.35)));
    cv.fill(ellipse(cx, cy, r, r), &radial(cx - r * 0.4, cy - r * 0.4, r * 1.8, &[(0., a), (1., b)]));
    cv.stroke(ellipse(cx, cy, r * 0.86, r * 0.86), mix(b, Color::BLACK, 0.1), 2.);
    cv.fill(ellipse(cx + r * 0.05, cy - r * 0.05, r * 0.42, r * 0.5), &solid(rgba(0, 0, 0, 0.12)));
}

fn ball(cv: &mut Canvas, cx: f32, cy: f32, r: f32, kind: u8) {
    cv.fill(ellipse(cx + 6., cy + r * 0.95, r * 0.95, r * 0.18), &solid(rgba(0, 0, 0, 0.35)));
    let (a, b) = match kind {
        0 => (hex("#9BE07A"), hex("#3E8A2A")),   // pea
        1 => (hex("#FFFFFF"), hex("#B9C0C8")),   // golf ball
        2 => (hex("#E8F56A"), hex("#9FB227")),   // tennis ball
        _ => (hex("#FFFEF6"), hex("#C9C2B0")),   // baseball / softball
    };
    cv.fill(ellipse(cx, cy, r, r), &radial(cx - r * 0.4, cy - r * 0.45, r * 1.7, &[(0., a), (1., b)]));
    match kind {
        1 => {
            let mut rng = Rng::new(3);
            for _ in 0..60 {
                let (a, d) = (rng.f() * std::f32::consts::TAU, rng.f().sqrt() * r * 0.88);
                cv.fill(ellipse(cx + a.cos() * d, cy + a.sin() * d, r * 0.05, r * 0.05), &solid(rgba(120, 130, 140, 0.35)));
            }
        }
        2 => {
            cv.stroke(smooth(&[(cx - r * 0.75, cy - r * 0.62), (cx - r * 0.2, cy - r * 0.1), (cx - r * 0.3, cy + r * 0.6), (cx - r * 0.7, cy + r * 0.72)], false), rgba(255, 255, 255, 0.95), r * 0.06);
            cv.stroke(smooth(&[(cx + r * 0.72, cy - r * 0.7), (cx + r * 0.3, cy - r * 0.5), (cx + r * 0.22, cy + r * 0.1), (cx + r * 0.75, cy + r * 0.6)], false), rgba(255, 255, 255, 0.95), r * 0.06);
        }
        3 => {
            for side in [-1., 1.] {
                let pts: Vec<(f32, f32)> = (0..9).map(|i| {
                    let t = i as f32 / 8. - 0.5;
                    (cx + side * (r * 0.55 - t * t * r * 1.1), cy + t * r * 1.6)
                }).collect();
                cv.stroke(smooth(&pts, false), hex("#D23B32"), r * 0.035);
                for p in pts.iter().skip(1).take(7) {
                    cv.line((p.0 - r * 0.06, p.1 - r * 0.04), (p.0 + r * 0.06, p.1 + r * 0.04), hex("#D23B32"), r * 0.025);
                }
            }
        }
        _ => {}
    }
}

pub fn hail_sizes(cv: &mut Canvas) {
    cv.round(0., 0., 1920., 832., 0., &linear(0., 0., 0., 832., &[(0., hex("#1B2C44")), (1., hex("#0D1726"))]));
    // Table surface.
    cv.round(0., 470., 1920., 362., 0., &linear(0., 470., 0., 832., &[(0., hex("#6E5236")), (1., hex("#3D2B1A"))]));
    let k = 84.; // px per inch
    let items: [(&str, &str, f32, u8); 8] = [
        ("Pea", "1/4\"", 0.25, 0), ("Penny", "3/4\"", 0.75, 10), ("Quarter", "1\"", 1.0, 11), ("Half dollar", "1 1/4\"", 1.25, 11),
        ("Golf ball", "1 3/4\"", 1.75, 1), ("Tennis ball", "2 1/2\"", 2.5, 2), ("Baseball", "2 3/4\"", 2.75, 3), ("Softball", "4\"", 4.0, 3),
    ];
    let total: f32 = items.iter().map(|i| i.2 * k).sum::<f32>() + 7. * 70.;
    let mut x = (1920. - total) / 2.;
    let base = 580.;
    for (i, (name, size, inch, kind)) in items.iter().enumerate() {
        let r = inch * k / 2.;
        let cx = x + r;
        let cy = base - r;
        match kind {
            10 => coin(cv, cx, cy, r, true),
            11 => coin(cv, cx, cy, r, false),
            _ => ball(cv, cx, cy, r, *kind),
        }
        let severe = *inch >= 1.0;
        cv.text(name, cx, base + 60. + (i % 2) as f32 * 0., 28., Weight::Bold, Color::WHITE, Al::Center, (inch * k + 64.).max(110.));
        cv.text(size, cx, base + 102., 32., Weight::Bold, if severe { hex("#FFD34D") } else { rgba(220, 230, 245, 1.) }, Al::Center, 200.);
        if i == 2 {
            cv.dashed(poly(&[(x - 25., 200.), (x - 25., 720.)], false), hex("#FFD34D"), 4., 16., 10.);
        }
        x += inch * k + 70.;
    }
    let qx = (1920. - total) / 2. + (0.25 + 0.75) * k + 140. - 25.;
    tag(cv, "Severe: 1 inch and larger", (qx + 20., 150.), Some((qx, 220.)), "#FFD34D");
    cv.text("Shown to scale", 1880., 800., 24., Weight::SemiBold, rgba(230, 220, 205, 0.9), Al::Right, 0.);
}

pub fn microburst(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Storm);
    ground(cv, 0., 640., 1920., 192., Ground::Grass, 21);
    cloud(cv, 960., 150., 2.6, 0.85, 4);
    cloud(cv, 560., 170., 1.6, 0.9, 6);
    cloud(cv, 1380., 170., 1.7, 0.9, 8);
    // Rain-cooled downdraft column.
    cv.fill(poly(&[(800., 250.), (1120., 250.), (1180., 640.), (740., 640.)], true), &linear(0., 250., 0., 640., &[(0., rgba(150, 170, 190, 0.55)), (1., rgba(150, 170, 190, 0.2))]));
    rain(cv, 760., 260., 400., 380., 220, 0.0, 5);
    for dx in [-90., 0., 90.] {
        cv.arrow((960. + dx, 300.), (960. + dx, 590.), rgba(130, 200, 255, 1.), 12.);
    }
    // Outflow curling both ways.
    for side in [-1f32, 1.] {
        flow(cv, &[(960. + side * 120., 640.), (960. + side * 380., 650.), (960. + side * 640., 610.), (960. + side * 760., 540.)], rgba(130, 200, 255, 1.), 11.);
    }
    // Trees bent away from the centre.
    for (i, x) in [260., 420., 1500., 1660.].iter().enumerate() {
        let lean = if *x < 960. { -1.0 } else { 1.0 };
        tree(cv, *x, 660., 220., Tree::Leafy, lean, i as u64 + 3);
    }
    tag(cv, "Downdraft", (1200., 320.), Some((1060., 380.)), "#9FD2FF");
    tag(cv, "Winds spread out at the ground", (1180., 720.), Some((1400., 640.)), "#FFFFFF");
}

pub fn watch_warning(cv: &mut Canvas) {
    sky(cv, 0., 0., 960., 832., Sky::Day);
    ground(cv, 0., 660., 960., 172., Ground::Grass, 5);
    cumulonimbus(cv, 480., 560., 0.85, false);
    sun(cv, 150., 120., 46.);
    house(cv, 640., 680., 200., 120., &HouseStyle { wall: "#D8CFC0", roof: "#5B4A3E", lit: false });
    tree(cv, 140., 690., 230., Tree::Leafy, 0., 3);
    sky(cv, 960., 0., 960., 832., Sky::Storm);
    ground(cv, 960., 660., 960., 172., Ground::Grass, 6);
    cloud(cv, 1440., 160., 3.0, 0.92, 9);
    cv.fill(smooth(&[(1380., 300.), (1470., 300.), (1440., 420.), (1430., 640.), (1418., 640.), (1400., 420.)], true), &linear(1380., 0., 1470., 0., &[(0., hex("#555B61")), (1., hex("#2E3338"))]));
    rain(cv, 1060., 280., 300., 380., 160, -0.1, 7);
    bolt(cv, (1700., 250.), (1760., 640.), 4);
    house(cv, 1580., 680., 200., 120., &HouseStyle { wall: "#BDB4A6", roof: "#3F332A", lit: true });
    divider(cv, 960.);
    tag_at(cv, "WATCH", 60., 70., "#FFD34D", 54.);
    tag_at(cv, "WARNING", 1020., 70., "#FF6B5A", 54.);
}

pub fn lightning_distance(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Day);
    cv.round(0., 0., 760., 832., 0., &linear(0., 0., 760., 0., &[(0., rgba(30, 36, 44, 0.85)), (1., rgba(30, 36, 44, 0.))]));
    hills(cv, 0., 600., 1920., 80., "#4E6B3A", 11);
    ground(cv, 0., 660., 1920., 172., Ground::Grass, 12);
    cumulonimbus(cv, 380., 400., 0.75, true);
    // Bolt from the anvil out over clear sky to the far right.
    bolt(cv, (620., 40.), (1580., 650.), 8);
    person(cv, 1600., 690., 120., Pose::Stand, "#C0392B");
    tree(cv, 1760., 690., 240., Tree::Leafy, 0., 2);
    // Distance ruler.
    cv.line((380., 760.), (1600., 760.), Color::WHITE, 4.);
    for x in [380., 1600.] {
        cv.line((x, 740.), (x, 780.), Color::WHITE, 4.);
    }
    cv.round(860., 732., 240., 56., 12., &solid(rgba(0, 0, 0, 0.75)));
    cv.text("10 miles", 980., 772., 36., Weight::Bold, hex("#FFD34D"), Al::Center, 0.);
    tag(cv, "Lightning can strike 10 miles from the storm", (1000., 120.), Some((1120., 300.)), "#FFD34D");
}

pub fn lightning_wait(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Dusk);
    ground(cv, 0., 640., 1920., 192., Ground::Grass, 13);
    // Storm moving away to the right.
    cumulonimbus(cv, 1500., 420., 0.75, true);
    bolt(cv, (1500., 430.), (1560., 640.), 2);
    cv.arrow((1100., 200.), (1300., 200.), Color::WHITE, 8.);
    cv.text("Storm moving away", 1200., 170., 28., Weight::Bold, Color::WHITE, Al::Center, 0.);
    house(cv, 220., 660., 330., 220., &HouseStyle { wall: "#E2D6C2", roof: "#4D3B30", lit: true });
    person(cv, 300., 610., 70., Pose::Stand, "#2E6DB4");
    // A wall clock face.
    let (cx, cy, r) = (820., 360., 150.);
    cv.fill(ellipse(cx + 6., cy + 8., r, r), &solid(rgba(0, 0, 0, 0.35)));
    cv.fill(ellipse(cx, cy, r, r), &radial(cx - 40., cy - 40., r * 1.6, &[(0., hex("#FFFFFF")), (1., hex("#C9D1DA"))]));
    cv.stroke(ellipse(cx, cy, r, r), hex("#2C3440"), 10.);
    // The 30 minutes still to wait, shaded.
    let mut pts = vec![(cx, cy)];
    for i in 0..=30 {
        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 30.;
        pts.push((cx + a.cos() * r * 0.9, cy + a.sin() * r * 0.9));
    }
    cv.fill_c(poly(&pts, true), rgba(255, 196, 64, 0.55));
    for i in 0..12 {
        let a = i as f32 * std::f32::consts::TAU / 12.;
        cv.line((cx + a.cos() * r * 0.8, cy + a.sin() * r * 0.8), (cx + a.cos() * r * 0.92, cy + a.sin() * r * 0.92), hex("#2C3440"), 5.);
    }
    cv.line((cx, cy), (cx, cy - r * 0.7), hex("#2C3440"), 8.);
    cv.line((cx, cy), (cx, cy + r * 0.72), hex("#C0392B"), 5.);
    cv.fill(ellipse(cx, cy, 10., 10.), &solid(hex("#2C3440")));
    tag(cv, "Wait 30 minutes after the last thunder", (560., 560.), Some((820., 520.)), "#FFD34D");
}

fn pavilion(cv: &mut Canvas, x: f32, gy: f32, w: f32) {
    let h = w * 0.55;
    for px in [x + 10., x + w - 22.] {
        cv.round(px, gy - h * 0.62, 12., h * 0.62, 2., &linear(px, 0., px + 12., 0., &[(0., hex("#8B6B4A")), (1., hex("#5A4028"))]));
    }
    cv.fill(poly(&[(x - 30., gy - h * 0.6), (x + w / 2., gy - h), (x + w + 30., gy - h * 0.6)], true), &linear(0., gy - h, 0., gy - h * 0.6, &[(0., hex("#9C3B2E")), (1., hex("#5E2018"))]));
    cv.round(x + w * 0.2, gy - h * 0.22, w * 0.6, 12., 3., &solid(hex("#7B5A3A")));
    for lx in [x + w * 0.25, x + w * 0.72] {
        cv.line((lx, gy - h * 0.2), (lx, gy), hex("#5A4028"), 6.);
    }
}

fn tent(cv: &mut Canvas, x: f32, gy: f32, w: f32) {
    let h = w * 0.55;
    cv.fill(poly(&[(x, gy), (x + w * 0.5, gy - h), (x + w, gy)], true), &linear(x, 0., x + w, 0., &[(0., hex("#F28C28")), (1., hex("#A3520E"))]));
    cv.fill(poly(&[(x + w * 0.38, gy), (x + w * 0.5, gy - h * 0.6), (x + w * 0.62, gy)], true), &solid(hex("#4A2A0E")));
}

pub fn lightning_shelter(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Storm);
    ground(cv, 0., 640., 1920., 192., Ground::Grass, 14);
    bolt(cv, (1460., 0.), (1500., 470.), 5);
    house(cv, 120., 660., 320., 210., &HouseStyle { wall: "#D9CDB8", roof: "#4D3B30", lit: true });
    car(cv, 520., 680., 300., "#2E6DB4", Car::Sedan);
    pavilion(cv, 1080., 680., 220.);
    tent(cv, 1400., 680., 200.);
    tree(cv, 1740., 680., 300., Tree::Leafy, 0., 6);
    divider(cv, 960.);
    tag_at(cv, "SAFE", 60., 60., "#5BE08A", 52.);
    tag_at(cv, "NOT SAFE", 1020., 60., "#FF6B5A", 52.);
    tag(cv, "Sturdy building", (90., 300.), Some((260., 420.)), "#FFFFFF");
    tag(cv, "Hard-topped car", (520., 470.), Some((670., 590.)), "#FFFFFF");
    tag(cv, "Picnic shelter", (1020., 300.), Some((1180., 540.)), "#FFFFFF");
    tag(cv, "Tent", (1430., 520.), Some((1500., 600.)), "#FFFFFF");
    tag(cv, "Under a tree", (1640., 230.), Some((1740., 380.)), "#FFFFFF");
}

fn flood_panel(cv: &mut Canvas, x: f32, depth_px: f32, label: &str, what: u8, seed: u64) {
    let gy = 640.;
    sky(cv, x, 0., 640., 832., Sky::Overcast);
    ground(cv, x, gy, 640., 192., Ground::Dirt, seed);
    match what {
        0 => person(cv, x + 320., gy + 4., 300., Pose::Walk, "#2E6DB4"),
        1 => car(cv, x + 150., gy - 12., 340., "#B03A2E", Car::Sedan),
        _ => car(cv, x + 120., gy - 30., 400., "#3C4E66", Car::Pickup),
    }
    water(cv, x, gy - depth_px, 640., 832. - gy + depth_px, true, seed);
    // Flow arrows on the water.
    for k in 0..3 {
        let y = gy - depth_px + 30. + k as f32 * 60.;
        cv.arrow((x + 40., y), (x + 200., y), rgba(255, 255, 255, 0.8), 5.);
    }
    // Depth gauge.
    let gx = x + 590.;
    cv.round(gx - 10., gy - 330., 20., 330., 4., &solid(rgba(255, 255, 255, 0.9)));
    for i in 0..=6 {
        let ty = gy - i as f32 * 55.;
        cv.line((gx - 10., ty), (gx + 10., ty), rgba(30, 30, 30, 0.9), 3.);
    }
    cv.line((gx - 24., gy - depth_px), (gx + 24., gy - depth_px), hex("#FFD34D"), 5.);
    tag_at(cv, label, x + 30., 40., "#FFD34D", 46.);
}

pub fn flood_depth(cv: &mut Canvas) {
    // 1 foot = 110 px in these panels.
    flood_panel(cv, 0., 55., "6 inches", 0, 3);
    flood_panel(cv, 640., 110., "1 foot", 1, 4);
    flood_panel(cv, 1280., 220., "2 feet", 2, 5);
    divider(cv, 640.);
    divider(cv, 1280.);
}

pub fn flood_road(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    hills(cv, 0., 380., 1920., 120., "#4F6A3E", 9);
    ground(cv, 0., 460., 1920., 372., Ground::Grass, 15);
    // Road dipping into a low spot (side cutaway).
    let road_y = |x: f32| if x < 700. || x > 1400. { 520. } else { 520. + ((x - 700.) / 700. * std::f32::consts::PI).sin() * 160. };
    let mut top: Vec<(f32, f32)> = (0..=48).map(|i| { let x = i as f32 * 40.; (x, road_y(x)) }).collect();
    let mut body = top.clone();
    body.push((1920., 600.));
    for i in (0..=48).rev() {
        let x = i as f32 * 40.;
        body.push((x, road_y(x) + 30.));
    }
    cv.fill(poly(&body, true), &solid(hex("#3A3C40")));
    cv.stroke(poly(&top, false), hex("#F2D04B"), 3.);
    // Washed-out hole under the water.
    cv.fill(smooth(&[(980., 690.), (1060., 650.), (1150., 660.), (1200., 720.), (1120., 790.), (1010., 770.)], true), &solid(hex("#2A1E14")));
    water(cv, 760., 600., 600., 232., true, 2);
    top.clear();
    // Car stopping short, sign.
    car(cv, 230., 522., 300., "#E2E5E9", Car::Suv);
    let (sx, sy) = (620., 300.);
    cv.line((sx + 90., sy + 150.), (sx + 90., 520.), hex("#8E979F"), 10.);
    cv.round(sx, sy, 180., 150., 10., &solid(Color::WHITE));
    cv.stroke(rrect(sx + 6., sy + 6., 168., 138., 8.), Color::BLACK, 4.);
    for (i, l) in ["TURN", "AROUND", "DON'T", "DROWN"].iter().enumerate() {
        cv.text(l, sx + 90., sy + 40. + i as f32 * 30., 26., Weight::Bold, Color::BLACK, Al::Center, 160.);
    }
    tag(cv, "Road washed out under the water", (1260., 560.), Some((1150., 700.)), "#FFD34D");
    tag(cv, "Stop before the water", (130., 610.), Some((380., 540.)), "#FFFFFF");
}

pub fn flash_vs_river(cv: &mut Canvas) {
    // Flash flood: steep creek valley, heavy rain upstream.
    sky(cv, 0., 0., 960., 832., Sky::Storm);
    cloud(cv, 480., 120., 2.0, 0.9, 3);
    rain(cv, 200., 200., 560., 300., 260, -0.1, 4);
    cv.fill(poly(&[(0., 360.), (380., 640.), (580., 640.), (960., 380.), (960., 832.), (0., 832.)], true), &linear(0., 360., 0., 832., &[(0., hex("#6B7F4A")), (1., hex("#3E4C2A"))]));
    water(cv, 340., 600., 300., 232., true, 6);
    for k in 0..4 {
        cv.arrow((380. + k as f32 * 10., 640. + k as f32 * 40.), (560. + k as f32 * 15., 650. + k as f32 * 40.), rgba(255, 255, 255, 0.8), 5.);
    }
    // River flood: wide, slow, over the banks into fields.
    sky(cv, 960., 0., 960., 832., Sky::Overcast);
    ground(cv, 960., 420., 960., 412., Ground::Grass, 7);
    water(cv, 960., 480., 960., 352., true, 8);
    for i in 0..6 {
        let x = 1000. + i as f32 * 150.;
        cv.line((x, 470.), (x, 520.), hex("#6E5236"), 4.);
    }
    house(cv, 1560., 520., 200., 130., &HouseStyle { wall: "#D8CFC0", roof: "#5B4A3E", lit: false });
    tree(cv, 1100., 520., 200., Tree::Leafy, 0., 2);
    divider(cv, 960.);
    tag_at(cv, "FLASH FLOOD", 40., 40., "#FF6B5A", 46.);
    tag_at(cv, "RIVER FLOOD", 1000., 40., "#9FD2FF", 46.);
    tag(cv, "Minutes to hours", (60., 300.), None, "#FFFFFF");
    tag(cv, "Days, rising slowly", (1000., 300.), None, "#FFFFFF");
}

pub fn funnel_vs_tornado(cv: &mut Canvas) {
    for (x, touch) in [(0., false), (960., true)] {
        sky(cv, x, 0., 960., 832., Sky::Storm);
        hills(cv, x, 560., 960., 90., "#3E5032", 3);
        ground(cv, x, 620., 960., 212., Ground::Grass, 4);
        cv.fill(smooth(&[(x - 20., 100.), (x + 480., 60.), (x + 980., 100.), (x + 980., 280.), (x + 480., 300.), (x - 20., 280.)], true), &linear(0., 60., 0., 300., &[(0., hex("#5E6870")), (1., hex("#2A3036"))]));
        let cx = x + 480.;
        let bottom = if touch { 640. } else { 470. };
        let f = smooth(&[(cx - 60., 290.), (cx + 60., 290.), (cx + 14., bottom - 20.), (cx + 4., bottom), (cx - 6., bottom), (cx - 18., bottom - 20.)], true);
        cv.fill(f, &linear(cx - 60., 0., cx + 60., 0., &[(0., hex("#4A4F55")), (0.5, hex("#80878E")), (1., hex("#3A3F45"))]));
        if touch {
            for i in 0..10 {
                let a = i as f32 * 0.65;
                cv.fill(ellipse(cx + a.cos() * 70., 630. - (a.sin() * 20.).abs(), 50. - i as f32 * 2., 24.), &solid(rgba(110, 92, 70, 0.6)));
            }
        }
        tree(cv, x + 160., 640., 200., Tree::Leafy, 0., 5);
        tree(cv, x + 800., 640., 180., Tree::Leafy, 0., 6);
    }
    divider(cv, 960.);
    tag_at(cv, "FUNNEL CLOUD", 40., 340., "#FFFFFF", 44.);
    tag_at(cv, "TORNADO", 1000., 340., "#FF6B5A", 44.);
    tag(cv, "Not touching the ground", (520., 520.), Some((490., 470.)), "#FFFFFF");
    tag(cv, "Debris at the ground", (1480., 720.), Some((1480., 640.)), "#FFD34D");
}

pub fn shelf_vs_wall(cv: &mut Canvas) {
    // Shelf cloud: long low wedge on the leading edge.
    sky(cv, 0., 0., 960., 832., Sky::Storm);
    ground(cv, 0., 620., 960., 212., Ground::Grass, 8);
    cv.fill(smooth(&[(-20., 120.), (960., 120.), (980., 330.), (700., 360.), (300., 440.), (-20., 470.)], true), &linear(0., 120., 0., 470., &[(0., hex("#56616B")), (1., hex("#2B3138"))]));
    for k in 0..3 {
        let y = 380. + k as f32 * 26.;
        cv.stroke(smooth(&[(-20., y + 60.), (300., y + 10.), (700., y - 50.), (980., y - 70.)], false), rgba(160, 170, 180, 0.5), 6.);
    }
    flow(cv, &[(700., 540.), (500., 560.), (200., 580.)], rgba(130, 200, 255, 0.95), 8.);
    // Wall cloud: isolated lowering under the base.
    sky(cv, 960., 0., 960., 832., Sky::Storm);
    ground(cv, 960., 620., 960., 212., Ground::Grass, 9);
    cv.fill(smooth(&[(940., 100.), (1940., 100.), (1940., 300.), (940., 300.)], true), &linear(0., 100., 0., 300., &[(0., hex("#5E6870")), (1., hex("#2E343A"))]));
    cv.fill(smooth(&[(1300., 295.), (1620., 295.), (1600., 400.), (1460., 430.), (1320., 400.)], true), &linear(0., 295., 0., 430., &[(0., hex("#3A4148")), (1., hex("#22272C"))]));
    flow(cv, &[(1300., 450.), (1460., 470.), (1620., 440.), (1600., 380.)], rgba(255, 214, 90, 0.9), 6.);
    divider(cv, 960.);
    tag_at(cv, "SHELF CLOUD", 40., 30., "#9FD2FF", 44.);
    tag_at(cv, "WALL CLOUD", 1000., 30., "#FFD34D", 44.);
    tag(cv, "Wedge on the storm's leading edge", (120., 680.), Some((420., 420.)), "#FFFFFF");
    tag(cv, "Lowering that can rotate", (1280., 520.), Some((1460., 430.)), "#FFFFFF");
}

pub fn derecho(cv: &mut Canvas) {
    // From above: farm country with a bowed line of storms moving east.
    cv.round(0., 0., 1920., 832., 0., &linear(0., 0., 1920., 832., &[(0., hex("#5F7F45")), (1., hex("#3F5A2E"))]));
    let mut r = Rng::new(5);
    for i in 0..9 {
        for j in 0..5 {
            let c = if (i + j) % 3 == 0 { hex("#8A9A55") } else if (i + j) % 3 == 1 { hex("#6C8A48") } else { hex("#B59E5E") };
            cv.round(i as f32 * 220. + r.range(0., 20.), j as f32 * 170. + r.range(0., 20.), 200., 150., 6., &solid(c));
        }
    }
    for y in [160., 500.] {
        cv.line((0., y), (1920., y + 40.), hex("#C9C1AE"), 10.);
    }
    // The bow (radar-like colours: TWC rain palette).
    let bow: Vec<(f32, f32)> = (0..=20).map(|i| {
        let t = i as f32 / 20.;
        (900. + (1. - (2. * t - 1.).powi(2)) * 380., 40. + t * 750.)
    }).collect();
    for (wd, c) in [(150., crate::twc::rain(25.).unwrap_or([40, 160, 60])), (100., crate::twc::rain(40.).unwrap_or([40, 160, 60])), (55., crate::twc::rain(50.).unwrap_or([40, 160, 60])), (22., crate::twc::rain(60.).unwrap_or([40, 160, 60]))] {
        cv.stroke(smooth(&bow, false), Color::from_rgba8(c[0], c[1], c[2], 220), wd);
    }
    for y in [200., 330., 420., 520., 640.] {
        let t = (y - 40.) / 750.;
        let x = 900. + (1. - (2. * t - 1f32).powi(2)) * 380.;
        cv.arrow((x - 420., y), (x - 100., y), Color::WHITE, 10.);
    }
    cv.arrow((1450., 410.), (1700., 410.), hex("#FFD34D"), 12.);
    for (x, y) in [(500., 260.), (560., 400.), (640., 520.), (420., 600.), (700., 300.)] {
        cv.line((x, y), (x + 50., y + 14.), hex("#5A3B22"), 8.);
        cv.fill(ellipse(x + 58., y + 16., 22., 14.), &solid(hex("#2F5E22")));
    }
    tag(cv, "Line of storms bows outward", (1320., 120.), Some((1250., 250.)), "#FFFFFF");
    tag(cv, "Damaging winds behind the line", (80., 720.), Some((480., 520.)), "#FFD34D");
    tag(cv, "Moving east", (1500., 460.), None, "#FFD34D");
}

pub fn tornado_in_car(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Storm);
    ground(cv, 0., 600., 1920., 232., Ground::Grass, 3);
    road(cv, 0., 650., 1920., 110., false);
    // Overpass (crossed out).
    cv.round(980., 380., 900., 70., 4., &linear(0., 380., 0., 450., &[(0., hex("#B8B3A8")), (1., hex("#7E786C"))]));
    for px in [1040., 1780.] {
        cv.round(px, 450., 60., 200., 4., &linear(px, 0., px + 60., 0., &[(0., hex("#A9A398")), (1., hex("#6E685D"))]));
    }
    cv.line((1080., 360.), (1800., 640.), hex("#E53935"), 18.);
    cv.line((1800., 360.), (1080., 640.), hex("#E53935"), 18.);
    // Sturdy building to drive to.
    let (bx, gy) = (120., 640.);
    cv.round(bx, gy - 260., 420., 260., 4., &linear(bx, 0., bx + 420., 0., &[(0., hex("#C7B9A0")), (1., hex("#8D7F68"))]));
    for i in 0..4 {
        cv.round(bx + 30. + i as f32 * 100., gy - 220., 60., 70., 2., &solid(hex("#3E5F7A")));
    }
    cv.round(bx + 170., gy - 120., 80., 120., 2., &solid(hex("#5A3A26")));
    car(cv, 640., 720., 280., "#2E6DB4", Car::Sedan);
    cv.arrow((620., 680.), (560., 620.), hex("#5BE08A"), 12.);
    cloud(cv, 1500., 140., 2.2, 0.9, 6);
    tag(cv, "Get to a sturdy building", (90., 300.), Some((300., 400.)), "#5BE08A");
    tag(cv, "Never shelter under an overpass", (1060., 280.), Some((1300., 380.)), "#FF6B5A");
}

pub fn ef_scale(cv: &mut Canvas) {
    cv.round(0., 0., 1920., 832., 0., &linear(0., 0., 0., 832., &[(0., hex("#2A3B55")), (1., hex("#121C2C"))]));
    let rows: [(&str, &str, &str, u8); 6] = [
        ("EF0", "65–85 mph", "Shingles and gutters damaged", 0),
        ("EF1", "86–110 mph", "Roofs badly stripped", 1),
        ("EF2", "111–135 mph", "Roofs torn off", 2),
        ("EF3", "136–165 mph", "Walls of well-built houses down", 3),
        ("EF4", "166–200 mph", "Well-built houses levelled", 4),
        ("EF5", "Over 200 mph", "Houses swept from foundations", 5),
    ];
    for (i, (ef, mph, what, lvl)) in rows.iter().enumerate() {
        let x = 30. + i as f32 * 315.;
        let c = crate::gfx::history_set::canvas::ef_color(i as i32);
        cv.round(x, 30., 295., 772., 20., &solid(rgba(255, 255, 255, 0.06)));
        cv.round(x, 30., 295., 90., 20., &solid(c));
        cv.text(ef, x + 147., 98., 56., Weight::Bold, hex("#101820"), Al::Center, 0.);
        cv.text(mph, x + 147., 175., 32., Weight::Bold, Color::WHITE, Al::Center, 270.);
        // Ground and the house in its damage state.
        let gy = 500.;
        cv.round(x + 10., gy, 275., 40., 6., &solid(hex("#4E7A32")));
        damaged_house(cv, x + 50., gy, 200., 150., *lvl);
        let words: Vec<&str> = what.split(' ').collect();
        let mid = words.len().div_ceil(2);
        cv.text(&words[..mid].join(" "), x + 147., 620., 28., Weight::SemiBold, Color::WHITE, Al::Center, 270.);
        cv.text(&words[mid..].join(" "), x + 147., 658., 28., Weight::SemiBold, Color::WHITE, Al::Center, 270.);
    }
}

/// The same frame house at increasing damage (EF scale only).
fn damaged_house(cv: &mut Canvas, x: f32, gy: f32, w: f32, h: f32, lvl: u8) {
    let wall = hex("#D8CDB8");
    let top = gy - h;
    if lvl >= 5 {
        cv.round(x - 10., gy - 18., w + 20., 18., 2., &solid(hex("#8E8A82")));
        let mut r = Rng::new(9);
        for _ in 0..14 {
            let (dx, dy) = (x + r.f() * w, gy - 30. - r.f() * 40.);
            cv.line((dx, dy), (dx + r.range(-30., 30.), dy + r.range(-10., 10.)), hex("#7A5A3A"), 5.);
        }
        return;
    }
    let wall_h = if lvl >= 4 { h * 0.3 } else if lvl >= 3 { h * 0.65 } else { h };
    cv.round(x, gy - wall_h, w, wall_h, 0., &linear(x, 0., x + w, 0., &[(0., wall), (1., mix(wall, Color::BLACK, 0.2))]));
    if lvl <= 1 {
        let roof = poly(&[(x - 14., top + 2.), (x + w / 2., top - h * 0.5), (x + w + 14., top + 2.)], true);
        cv.fill(roof, &solid(hex("#4D3B30")));
        if lvl == 1 {
            cv.fill_c(poly(&[(x + w * 0.5, top - h * 0.48), (x + w * 0.85, top - h * 0.1), (x + w * 0.6, top - h * 0.05)], true), hex("#A47A4E"));
        } else {
            for k in 0..3 {
                cv.round(x + w * (0.2 + k as f32 * 0.2), top - h * 0.2, 16., 10., 2., &solid(hex("#A47A4E")));
            }
        }
    } else if lvl == 2 {
        for k in 0..5 {
            let rx = x + k as f32 * w / 4.;
            cv.line((rx, top), (rx + 20., top - 40.), hex("#A47A4E"), 5.);
        }
    }
    cv.round(x + w * 0.15, gy - wall_h + 20., w * 0.2, (wall_h * 0.25).min(30.), 2., &solid(hex("#2C3E50")));
    if lvl >= 3 {
        let mut r = Rng::new(lvl as u64);
        for _ in 0..8 {
            let (dx, dy) = (x + r.f() * w, gy - r.f() * 20.);
            cv.line((dx, dy), (dx + r.range(-25., 25.), dy - 6.), hex("#7A5A3A"), 5.);
        }
    }
}

pub fn severe_criteria(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Storm);
    ground(cv, 0., 620., 1920., 212., Ground::Grass, 6);
    cumulonimbus(cv, 960., 480., 0.9, true);
    tree(cv, 300., 640., 300., Tree::Leafy, -0.9, 3);
    tree(cv, 520., 640., 240., Tree::Leafy, -0.8, 4);
    for k in 0..4 {
        cv.arrow((760. - k as f32 * 20., 520. + k as f32 * 30.), (520. - k as f32 * 20., 520. + k as f32 * 30.), rgba(130, 200, 255, 0.95), 9.);
    }
    coin(cv, 1500., 560., 70., false);
    hailstone(cv, 1660., 560., 34., false, 3);
    cv.text("Quarter = 1 inch", 1500., 680., 30., Weight::Bold, Color::WHITE, Al::Center, 0.);
    tag(cv, "Wind gusts 58 mph or stronger", (130., 70.), Some((400., 380.)), "#9FD2FF");
    tag(cv, "Hail 1 inch or larger", (1360., 400.), Some((1560., 500.)), "#FFD34D");
}

pub fn outdoor_event(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Dusk);
    cumulonimbus(cv, 1560., 440., 0.8, true);
    bolt(cv, (1560., 450.), (1620., 600.), 3);
    ground(cv, 0., 560., 1920., 272., Ground::Grass, 7);
    // Field lines and bleachers.
    for i in 0..6 {
        cv.line((100. + i as f32 * 200., 600.), (40. + i as f32 * 240., 832.), rgba(255, 255, 255, 0.5), 4.);
    }
    for k in 0..5 {
        cv.round(60., 360. + k as f32 * 40., 520., 18., 3., &linear(0., 0., 520., 0., &[(0., hex("#C0C6CE")), (1., hex("#7D848E"))]));
    }
    for i in 0..9 {
        person(cv, 100. + i as f32 * 55., 400. + (i % 3) as f32 * 40., 70., Pose::Stand, ["#C0392B", "#2E6DB4", "#F1C40F"][i % 3]);
    }
    // Field-house to go to.
    cv.round(760., 380., 360., 200., 6., &linear(760., 0., 1120., 0., &[(0., hex("#BFAF94")), (1., hex("#857659"))]));
    cv.round(900., 470., 80., 110., 2., &solid(hex("#4A2E1E")));
    cv.arrow((560., 520.), (880., 520.), hex("#5BE08A"), 12.);
    tag(cv, "Move everyone to a sturdy building", (520., 680.), Some((860., 560.)), "#5BE08A");
    tag(cv, "No place outside is safe from lightning", (1180., 70.), Some((1560., 300.)), "#FFD34D");
}

pub fn siren_indoors(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Storm);
    ground(cv, 0., 640., 1920., 192., Ground::Grass, 5);
    // Siren tower.
    let tx = 1400.;
    cv.line((tx, 640.), (tx, 210.), hex("#8E979F"), 16.);
    cv.round(tx - 70., 170., 140., 70., 20., &linear(0., 170., 0., 240., &[(0., hex("#E4E8EC")), (1., hex("#9AA2AA"))]));
    for k in 1..4 {
        let r = 90. * k as f32;
        cv.stroke(smooth(&[(tx - r, 205. - r * 0.6), (tx - r * 1.1, 205.), (tx - r, 205. + r * 0.6)], false), rgba(255, 255, 255, 0.5 - k as f32 * 0.1), 5.);
        cv.stroke(smooth(&[(tx + r, 205. - r * 0.6), (tx + r * 1.1, 205.), (tx + r, 205. + r * 0.6)], false), rgba(255, 255, 255, 0.5 - k as f32 * 0.1), 5.);
    }
    house(cv, 200., 660., 420., 260., &HouseStyle { wall: "#D9CDB8", roof: "#4D3B30", lit: true });
    // Phone and weather radio indoors, shown large beside the house.
    let (px, py) = (760., 330.);
    cv.round(px, py, 150., 280., 22., &solid(hex("#1B1F24")));
    cv.round(px + 10., py + 24., 130., 230., 8., &linear(0., py, 0., py + 260., &[(0., hex("#C62828")), (1., hex("#7F1515"))]));
    cv.text("ALERT", px + 75., py + 120., 30., Weight::Bold, Color::WHITE, Al::Center, 120.);
    cv.text("Tornado", px + 75., py + 160., 22., Weight::SemiBold, Color::WHITE, Al::Center, 120.);
    cv.text("Warning", px + 75., py + 186., 22., Weight::SemiBold, Color::WHITE, Al::Center, 120.);
    weather_radio(cv, 960., 470., 300.);
    tag(cv, "Sirens are for people outdoors", (1500., 360.), Some((1440., 260.)), "#FFFFFF");
    tag(cv, "Indoors: phone alerts and NOAA Weather Radio", (640., 680.), Some((960., 600.)), "#FFD34D");
}

pub fn weather_radio(cv: &mut Canvas, x: f32, y: f32, w: f32) {
    let h = w * 0.45;
    appliance(cv, x, y, w, h, "#2C3138");
    cv.round(x + w * 0.06, y + h * 0.18, w * 0.42, h * 0.46, 6., &linear(0., y, 0., y + h, &[(0., hex("#9FE3B0")), (1., hex("#4FA36A"))]));
    cv.text("162.475", x + w * 0.27, y + h * 0.5, w * 0.07, Weight::Bold, hex("#0E2A16"), Al::Center, w * 0.38);
    let mut gx = x + w * 0.56;
    while gx < x + w * 0.94 {
        cv.line((gx, y + h * 0.18), (gx, y + h * 0.82), rgba(0, 0, 0, 0.45), 4.);
        gx += 12.;
    }
    cv.round(x + w * 0.06, y + h * 0.72, w * 0.12, h * 0.14, 4., &solid(hex("#D23B32")));
    cv.line((x + w * 0.85, y), (x + w * 0.95, y - h * 0.9), hex("#9AA2AA"), 5.);
}

pub fn wind_from(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Day);
    ground(cv, 0., 640., 1920., 192., Ground::Grass, 8);
    // Barn with a weather vane; a north wind blowing from left (north) to right (south).
    let (bx, gy) = (640., 660.);
    cv.round(bx, gy - 300., 520., 300., 0., &linear(bx, 0., bx + 520., 0., &[(0., hex("#B8382C")), (1., hex("#7C2018"))]));
    cv.fill(poly(&[(bx - 30., gy - 300.), (bx + 260., gy - 470.), (bx + 550., gy - 300.)], true), &linear(0., gy - 470., 0., gy - 300., &[(0., hex("#5A5F66")), (1., hex("#33373C"))]));
    cv.round(bx + 190., gy - 180., 140., 180., 2., &solid(hex("#F2EDE4")));
    cv.line((bx + 190., gy - 180.), (bx + 330., gy), hex("#F2EDE4"), 8.);
    cv.line((bx + 330., gy - 180.), (bx + 190., gy), hex("#F2EDE4"), 8.);
    // Vane: arrow points INTO the wind (north = left here).
    let (vx, vy) = (bx + 260., gy - 560.);
    cv.line((vx, gy - 470.), (vx, vy + 30.), hex("#2C2C2C"), 6.);
    cv.fill_c(poly(&[(vx - 140., vy), (vx - 90., vy - 26.), (vx - 90., vy + 26.)], true), hex("#2C2C2C"));
    cv.line((vx - 100., vy), (vx + 100., vy), hex("#2C2C2C"), 8.);
    cv.fill_c(poly(&[(vx + 60., vy - 40.), (vx + 130., vy - 40.), (vx + 130., vy + 40.), (vx + 60., vy + 40.)], true), hex("#2C2C2C"));
    cv.line((vx, vy + 50.), (vx, vy + 70.), hex("#2C2C2C"), 4.);
    for (i, d) in ["N", "S"].iter().enumerate() {
        let dx = if i == 0 { -60. } else { 60. };
        cv.text(d, vx + dx, vy + 100., 34., Weight::Bold, hex("#2C2C2C"), Al::Center, 0.);
    }
    for k in 0..4 {
        let y = 160. + k as f32 * 90.;
        cv.arrow((80., y), (420., y), rgba(255, 255, 255, 0.95), 10.);
    }
    for (i, x) in [1360., 1560., 1740.].iter().enumerate() {
        tree(cv, *x, 660., 260., Tree::Leafy, 0.7, i as u64);
    }
    tag(cv, "Wind blowing FROM the north", (60., 520.), Some((300., 430.)), "#FFD34D");
    tag(cv, "Vane points into the wind", (vx + 160., vy - 70.), Some((vx - 120., vy)), "#FFFFFF");
}

pub fn pop_meaning(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    ground(cv, 0., 560., 1920., 272., Ground::Grass, 9);
    // A forecast area outlined on the ground; rain falls on part of it.
    cv.dashed(rrect(80., 560., 1760., 250., 30.), hex("#FFD34D"), 5., 20., 12.);
    for (i, x) in [180., 420., 700., 980., 1260., 1540.].iter().enumerate() {
        house(cv, *x, 700., 150., 90., &HouseStyle { wall: ["#D8CFC0", "#C9D6E0", "#E2D2B8"][i % 3], roof: "#4D3B30", lit: false });
    }
    cloud(cv, 560., 200., 2.2, 0.75, 4);
    rain(cv, 300., 280., 520., 420., 260, -0.05, 5);
    cloud(cv, 1400., 180., 1.6, 0.2, 6);
    sun(cv, 1700., 110., 40.);
    tag(cv, "Rain here", (380., 160.), None, "#9FD2FF");
    tag(cv, "Dry here", (1300., 360.), None, "#FFFFFF");
    tag(cv, "Forecast area", (110., 500.), Some((150., 565.)), "#FFD34D");
}

pub fn radar_beam(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Day);
    // Earth curve: drop below a flat line grows with distance.
    let x0 = 140.;
    let px_per_mile = 11.;
    let curve = |m: f32| 660. + (m * m) * 0.0053;
    let mut land: Vec<(f32, f32)> = (0..=160).map(|m| (x0 + m as f32 * px_per_mile, curve(m as f32))).collect();
    land.push((1920., 832.));
    land.push((0., 832.));
    land.insert(0, (0., 660.));
    cv.fill(poly(&land, true), &linear(0., 640., 0., 832., &[(0., hex("#5E9A3A")), (1., hex("#2E5A1E"))]));
    // Radar dome.
    cv.round(x0 - 30., 560., 60., 100., 4., &solid(hex("#C9CFD6")));
    cv.fill(ellipse(x0, 545., 60., 55.), &radial(x0 - 20., 525., 80., &[(0., Color::WHITE), (1., hex("#AEB6C0"))]));
    // Beam centre height (feet) above the radar for 0.5°, standard 4/3 earth refraction.
    let ft = |m: f32| {
        let r = m * 1.609 * 1000.;
        let re = 6371000. * 4. / 3.;
        (r * (0.5f32.to_radians()).sin() + r * r / (2. * re)) * 3.281
    };
    // Vertical scale: 1000 ft = 11 px.
    let y_of = |m: f32| curve(m) - ft(m) / 1000. * 11.;
    let beam_top: Vec<(f32, f32)> = (0..=150).map(|m| (x0 + m as f32 * px_per_mile, y_of(m as f32) - m as f32 * 0.8)).collect();
    let beam_bot: Vec<(f32, f32)> = (0..=150).rev().map(|m| (x0 + m as f32 * px_per_mile, y_of(m as f32) + m as f32 * 0.8)).collect();
    let mut beam = beam_top.clone();
    beam.extend(beam_bot);
    cv.fill(poly(&beam, true), &linear(x0, 0., 1800., 0., &[(0., rgba(255, 214, 90, 0.85)), (1., rgba(255, 214, 90, 0.35))]));
    // Distant storm with low rotation the beam overshoots.
    cumulonimbus(cv, 1500., 640., 0.45, false);
    flow(cv, &[(1440., 640.), (1500., 620.), (1560., 640.)], hex("#FF6B5A"), 6.);
    for m in [25f32, 50., 100., 150.] {
        let x = x0 + m * px_per_mile;
        let y = y_of(m);
        cv.fill(ellipse(x, y, 7., 7.), &solid(Color::WHITE));
        let label = format!("{} mi: {:.0} ft", m as i32, (ft(m) / 100.).round() * 100.);
        cv.text(&label, x, y - 30., 28., Weight::Bold, Color::WHITE, Al::Center, 300.);
    }
    tag(cv, "Beam rises as the Earth curves away", (300., 120.), Some((700., y_of(50.) - 10.)), "#FFD34D");
    tag(cv, "Low-level rotation can be below the beam", (1080., 760.), Some((1480., 640.)), "#FF6B5A");
}
