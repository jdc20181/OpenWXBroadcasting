//! Scenes, part 4: seasons, hail, yard and home, winter driving, wind. Canvas is 1920 × 832.

use tiny_skia::Color;

use super::kit::*;
use super::{divider, tag, tag_at};
use crate::gfx::history_set::paint::{ellipse, hex, linear, mix, poly, radial, rgba, rrect, smooth, solid, Al, Canvas};
use crate::gfx::text::Weight;

fn earth(cv: &mut Canvas, cx: f32, cy: f32, r: f32, tilt_deg: f32, lit_from_left: bool) {
    cv.fill(ellipse(cx, cy, r, r), &radial(cx - r * 0.3, cy - r * 0.3, r * 1.4, &[(0., hex("#4FA3E0")), (1., hex("#1A4A80"))]));
    // Land blobs.
    let mut rng = Rng::new(4);
    for _ in 0..6 {
        let (a, d) = (rng.f() * std::f32::consts::TAU, rng.f().sqrt() * r * 0.7);
        let (lx, ly) = (cx + a.cos() * d, cy + a.sin() * d);
        cv.fill(ellipse(lx, ly, r * rng.range(0.12, 0.25), r * rng.range(0.08, 0.18)), &solid(hex("#4E8A3A")));
    }
    // Night side.
    let shade = if lit_from_left { (cx + r * 0.2, cx + r) } else { (cx - r, cx - r * 0.2) };
    cv.fill(ellipse(cx, cy, r, r), &linear(shade.0, 0., shade.1, 0., &[(0., rgba(0, 0, 20, if lit_from_left { 0. } else { 0.65 })), (1., rgba(0, 0, 20, if lit_from_left { 0.65 } else { 0. }))]));
    let t = tilt_deg.to_radians();
    let (dx, dy) = (t.sin() * r * 1.3, t.cos() * r * 1.3);
    cv.line((cx - dx, cy + dy), (cx + dx, cy - dy), Color::WHITE, 4.);
}

pub fn seasons_tilt(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Night);
    // Orbit ellipse, sun in the middle, Earth on each side with the same tilt.
    cv.dashed(ellipse(960., 430., 700., 220.), rgba(255, 255, 255, 0.4), 3., 14., 10.);
    sun(cv, 960., 430., 80.);
    earth(cv, 300., 430., 110., 23.5, false);
    earth(cv, 1620., 430., 110., 23.5, true);
    for (x, toward) in [(300., true), (1620., false)] {
        let _ = toward;
        cv.dashed(poly(&[(x, 430.), (960., 430.)], false), rgba(255, 220, 120, 0.5), 3., 10., 8.);
    }
    tag_at(cv, "JUNE", 230., 600., "#FFD34D", 40.);
    tag_at(cv, "DECEMBER", 1520., 600., "#9FD2FF", 40.);
    tag(cv, "North tilted toward the sun: summer here", (60., 220.), Some((330., 330.)), "#FFD34D");
    tag(cv, "North tilted away: winter here", (1300., 220.), Some((1580., 330.)), "#9FD2FF");
    tag(cv, "Tilt: 23.5°", (840., 700.), None, "#FFFFFF");
}

pub fn hail_car(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Storm);
    ground(cv, 0., 640., 1920., 192., Ground::Grass, 3);
    // Car parked in a garage on the left, car in the open on the right with hail falling.
    cv.round(80., 300., 620., 360., 4., &linear(80., 0., 700., 0., &[(0., hex("#CFC3AE")), (1., hex("#9C907A"))]));
    cv.fill(poly(&[(50., 300.), (390., 170.), (730., 300.)], true), &solid(hex("#4D3B30")));
    cv.round(140., 380., 500., 280., 2., &solid(hex("#2A2620")));
    car(cv, 180., 660., 420., "#2E6DB4", Car::Sedan);
    car(cv, 1100., 680., 460., "#B03A2E", Car::Sedan);
    let mut r = Rng::new(8);
    for _ in 0..90 {
        let (x, y) = (900. + r.f() * 1000., r.f() * 640.);
        hailstone(cv, x, y, r.range(5., 11.), false, r.f().to_bits() as u64);
    }
    for _ in 0..30 {
        let x = 960. + r.f() * 900.;
        hailstone(cv, x, 650. + r.f() * 40., r.range(5., 9.), false, 3);
    }
    tag(cv, "Garage or covered parking", (120., 100.), Some((390., 300.)), "#5BE08A");
    tag(cv, "Hail dents cars and cracks glass", (1180., 80.), Some((1330., 520.)), "#FFD34D");
}

pub fn tree_limbs(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Day);
    ground(cv, 0., 620., 1920., 212., Ground::Grass, 5);
    house(cv, 900., 640., 560., 320., &HouseStyle { wall: "#D8CFC0", roof: "#5B4A3E", lit: false });
    tree(cv, 600., 640., 520., Tree::Leafy, 0.1, 6);
    // A dead, bare limb reaching over the roof.
    cv.line((660., 330.), (1020., 210.), hex("#6E5A48"), 18.);
    cv.line((900., 250.), (980., 170.), hex("#6E5A48"), 9.);
    cv.line((960., 228.), (1060., 240.), hex("#6E5A48"), 8.);
    cv.dashed(poly(&[(780., 300.), (820., 250.)], false), hex("#E53935"), 5., 10., 6.);
    tag(cv, "Dead limbs fall in strong wind", (1100., 90.), Some((980., 200.)), "#FFD34D");
    tag(cv, "Trim before storm season", (200., 120.), Some((790., 280.)), "#FFFFFF");
}

pub fn sandbags(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    ground(cv, 0., 560., 1920., 272., Ground::Grass, 4);
    house(cv, 1060., 580., 600., 320., &HouseStyle { wall: "#D8CFC0", roof: "#5B4A3E", lit: true });
    water(cv, 0., 610., 780., 222., true, 3);
    // Staggered sandbag wall like brickwork.
    for row in 0..4 {
        let y = 690. - row as f32 * 44.;
        let off = if row % 2 == 0 { 0. } else { 40. };
        for k in 0..4 {
            let x = 800. + off + k as f32 * 80. - row as f32 * 10.;
            cv.fill(rrect(x, y, 84., 46., 18.), &radial(x + 30., y + 14., 70., &[(0., hex("#D9C49A")), (1., hex("#8A7650"))]));
            cv.stroke(rrect(x, y, 84., 46., 18.), rgba(80, 60, 30, 0.6), 2.);
        }
    }
    tag(cv, "Stagger the bags like bricks", (760., 420.), Some((900., 560.)), "#FFD34D");
    tag(cv, "Plastic sheet on the water side", (100., 440.), Some((780., 640.)), "#FFFFFF");
}

pub fn uv_snow(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Day);
    sun(cv, 300., 140., 80.);
    hills(cv, 0., 380., 1920., 160., "#E9F0F6", 5);
    ground(cv, 0., 520., 1920., 312., Ground::Snow, 3);
    person(cv, 1100., 700., 300., Pose::Walk, "#C0392B");
    cv.round(1080., 432., 44., 12., 4., &solid(hex("#111111")));
    // Direct and reflected rays.
    for k in 0..3 {
        let x = 1000. + k as f32 * 80.;
        cv.arrow((380. + k as f32 * 30., 200.), (x, 640.), hex("#FFD34D"), 6.);
        cv.arrow((x, 650.), (x + 60., 460.), rgba(255, 211, 77, 0.6), 5.);
    }
    tag(cv, "Snow reflects up to 80% of UV", (1300., 600.), Some((1180., 560.)), "#FFD34D");
    tag(cv, "Sunglasses and sunscreen in winter too", (1300., 300.), Some((1110., 440.)), "#FFFFFF");
}

pub fn cloudy_nights(cv: &mut Canvas) {
    for (x, cloudy) in [(0., false), (960., true)] {
        sky(cv, x, 0., 960., 832., Sky::Night);
        ground(cv, x, 620., 960., 212., Ground::Grass, 4);
        house(cv, x + 360., 640., 240., 150., &HouseStyle { wall: "#BDB4A6", roof: "#3F332A", lit: true });
        if cloudy {
            cloud(cv, x + 300., 160., 1.5, 0.75, 3);
            cloud(cv, x + 680., 180., 1.4, 0.8, 4);
            for k in 0..3 {
                let ax = x + 240. + k as f32 * 220.;
                cv.arrow((ax, 560.), (ax, 300.), rgba(255, 130, 90, 0.9), 7.);
                cv.arrow((ax + 40., 280.), (ax + 40., 520.), rgba(255, 130, 90, 0.7), 7.);
            }
        } else {
            for k in 0..3 {
                let ax = x + 240. + k as f32 * 220.;
                cv.arrow((ax, 560.), (ax, 60.), rgba(255, 130, 90, 0.9), 7.);
            }
        }
        thermometer(cv, x + 820., 360., 200., if cloudy { 0.6 } else { 0.25 }, if cloudy { "#FF8A4A" } else { "#4AA3FF" });
    }
    divider(cv, 960.);
    tag_at(cv, "CLEAR NIGHT", 40., 40., "#9FD2FF", 40.);
    tag_at(cv, "CLOUDY NIGHT", 1000., 40., "#FFB08A", 40.);
    tag(cv, "Heat escapes to space: colder", (60., 640.), None, "#9FD2FF");
    tag(cv, "Clouds send heat back down: milder", (1020., 640.), None, "#FFB08A");
}

pub fn sleet_vs_hail(cv: &mut Canvas) {
    // Sleet: winter, small clear pellets. Hail: warm-season thunderstorm, layered stones.
    sky(cv, 0., 0., 960., 832., Sky::Winter);
    ground(cv, 0., 640., 960., 192., Ground::Ice, 3);
    sleet(cv, 40., 40., 880., 580., 260, 4);
    tree(cv, 760., 660., 300., Tree::Bare, 0., 2);
    cv.round(140., 380., 300., 220., 20., &solid(rgba(10, 20, 35, 0.75)));
    for k in 0..12 {
        let (x, y) = (180. + (k % 4) as f32 * 70., 430. + (k / 4) as f32 * 55.);
        cv.fill(ellipse(x, y, 10., 10.), &radial(x - 3., y - 3., 13., &[(0., Color::WHITE), (0.6, hex("#BFE0F5")), (1., hex("#7FA8C8"))]));
    }
    sky(cv, 960., 0., 960., 832., Sky::Storm);
    ground(cv, 960., 640., 960., 192., Ground::Grass, 5);
    let mut r = Rng::new(6);
    for _ in 0..60 {
        hailstone(cv, 1000. + r.f() * 880., 40. + r.f() * 560., r.range(8., 16.), false, 4);
    }
    cv.round(1100., 380., 340., 240., 20., &solid(rgba(10, 20, 35, 0.75)));
    hailstone(cv, 1270., 500., 90., true, 9);
    divider(cv, 960.);
    tag_at(cv, "SLEET", 40., 40., "#9FD2FF", 44.);
    tag_at(cv, "HAIL", 1000., 40., "#FFD34D", 44.);
    tag(cv, "Winter; small frozen raindrops", (100., 680.), None, "#FFFFFF");
    tag(cv, "Thunderstorms; layers of ice", (1060., 680.), None, "#FFFFFF");
}

pub fn ice_thickness(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Winter);
    hills(cv, 0., 260., 1920., 120., "#DCE6EE", 3);
    // Pond cross-section: ice on top of dark water, a ruler for thickness.
    cv.round(0., 380., 1920., 452., 0., &linear(0., 380., 0., 832., &[(0., hex("#2A5A7A")), (1., hex("#0E2A3E"))]));
    let rows = [("Under 4 in", "Stay off", "#FF6B5A", 26.), ("4 in", "Walking", "#FFD34D", 40.), ("5 to 7 in", "Snowmobile or ATV", "#9FD2FF", 60.), ("8 to 12 in", "Car or small truck", "#5BE08A", 90.)];
    for (i, (thick, use_, c, h)) in rows.iter().enumerate() {
        let x = 60. + i as f32 * 460.;
        cv.round(x, 380., 420., *h, 4., &linear(0., 380., 0., 380. + h, &[(0., hex("#EAF6FF")), (1., hex("#9EC8E4"))]));
        cv.line((x, 380. + h), (x + 420., 380. + h), rgba(255, 255, 255, 0.7), 3.);
        cv.round(x, 520., 420., 230., 18., &solid(rgba(0, 0, 0, 0.45)));
        cv.round(x, 520., 420., 16., 4., &solid(hex(c)));
        cv.text(thick, x + 210., 600., 44., Weight::Bold, hex(c), Al::Center, 400.);
        cv.text(use_, x + 210., 670., 32., Weight::SemiBold, Color::WHITE, Al::Center, 400.);
    }
    person(cv, 520., 380., 180., Pose::Stand, "#C0392B");
    tag(cv, "New, clear ice. Check thickness in several spots.", (520., 80.), None, "#FFFFFF");
}

pub fn gust_vs_sustained(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Day);
    ground(cv, 0., 680., 1920., 152., Ground::Grass, 6);
    // Anemometer on a mast, and a wind trace: steady average with gust spikes.
    let (mx, top) = (200., 220.);
    cv.line((mx, 690.), (mx, top), hex("#9AA2AA"), 10.);
    for k in 0..3 {
        let a = k as f32 * std::f32::consts::TAU / 3.;
        let (ex, ey) = (mx + a.cos() * 70., top + a.sin() * 22.);
        cv.line((mx, top), (ex, ey), hex("#5A6168"), 5.);
        cv.fill(ellipse(ex, ey, 18., 14.), &radial(ex - 4., ey - 4., 22., &[(0., hex("#E9ECEF")), (1., hex("#7D848E"))]));
    }
    cv.round(420., 90., 1440., 520., 20., &solid(rgba(10, 20, 35, 0.8)));
    let (gx, gy, gw, gh) = (480., 140., 1340., 420.);
    let y_of = |mph: f32| gy + gh - mph / 60. * gh;
    for m in [0., 20., 40., 60.] {
        cv.line((gx, y_of(m)), (gx + gw, y_of(m)), rgba(255, 255, 255, 0.12), 2.);
        cv.text(&format!("{m:.0}"), gx - 14., y_of(m) + 8., 22., Weight::SemiBold, rgba(220, 230, 245, 1.), Al::Right, 0.);
    }
    let mut r = Rng::new(12);
    let pts: Vec<(f32, f32)> = (0..=120).map(|i| {
        let t = i as f32 / 120.;
        let base = 22. + (t * 9.).sin() * 4.;
        let spike = if r.f() > 0.9 { r.range(14., 24.) } else { r.range(-5., 6.) };
        (gx + t * gw, y_of(base + spike))
    }).collect();
    cv.stroke(poly(&pts, false), hex("#9FD2FF"), 3.);
    cv.line((gx, y_of(22.)), (gx + gw, y_of(22.)), hex("#5BE08A"), 5.);
    cv.text("mph", gx - 14., gy - 14., 22., Weight::Bold, hex("#FFD34D"), Al::Right, 0.);
    tag(cv, "Sustained: the 2-minute average", (1200., 600.), Some((1300., y_of(22.))), "#5BE08A");
    tag(cv, "Gusts: brief peaks", (560., 40.), Some((900., y_of(42.))), "#9FD2FF");
}

pub fn windshield_clear(cv: &mut Canvas) {
    for (x, cleared) in [(0., false), (960., true)] {
        sky(cv, x, 0., 960., 832., Sky::Winter);
        ground(cv, x, 620., 960., 212., Ground::Snow, 3);
        // Car seen from the front: frosted glass, cleared or with a peephole.
        let cx = x + 480.;
        cv.round(cx - 330., 360., 660., 260., 50., &linear(0., 360., 0., 620., &[(0., hex("#4A5A6C")), (1., hex("#202A36"))]));
        cv.fill(poly(&[(cx - 240., 360.), (cx + 240., 360.), (cx + 190., 220.), (cx - 190., 220.)], true), &solid(hex("#3A4A5C")));
        let glass = poly(&[(cx - 220., 355.), (cx + 220., 355.), (cx + 175., 232.), (cx - 175., 232.)], true);
        cv.fill(glass.clone(), &linear(0., 232., 0., 355., &[(0., hex("#CFE6F5")), (1., hex("#7FA8C8"))]));
        if !cleared {
            cv.fill(glass, &solid(rgba(240, 246, 252, 0.92)));
            cv.fill(ellipse(cx - 60., 300., 70., 34.), &linear(0., 266., 0., 334., &[(0., hex("#9CC3DE")), (1., hex("#3E6585"))]));
            cv.round(cx - 330., 340., 660., 30., 14., &solid(rgba(255, 255, 255, 0.9)));
        }
        for hx in [cx - 250., cx + 170.] {
            cv.round(hx, 450., 80., 40., 10., &solid(hex("#FFF4C2")));
        }
        for wx in [cx - 260., cx + 200.] {
            cv.round(wx, 600., 60., 50., 8., &solid(hex("#141416")));
        }
    }
    divider(cv, 960.);
    tag_at(cv, "NO", 40., 40., "#FF6B5A", 44.);
    tag_at(cv, "YES", 1000., 40., "#5BE08A", 44.);
    tag(cv, "A peephole hides people and cars", (100., 700.), None, "#FFFFFF");
    tag(cv, "Clear every window, lights and roof", (1060., 700.), None, "#FFFFFF");
}

pub fn snowmelt_runoff(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    rain(cv, 0., 0., 1920., 420., 200, -0.05, 3);
    // Hillside with melting snow over frozen ground; water runs off to a rising creek.
    cv.fill(poly(&[(0., 260.), (900., 560.), (1300., 640.), (1920., 600.), (1920., 832.), (0., 832.)], true), &linear(0., 260., 0., 832., &[(0., hex("#6E5A44")), (1., hex("#3E3224"))]));
    let mut r = Rng::new(4);
    for _ in 0..30 {
        let x = r.f() * 800.;
        let y = 270. + x * 0.33 + r.range(0., 30.);
        cv.fill(ellipse(x, y, r.range(30., 70.), r.range(8., 14.)), &solid(rgba(245, 248, 252, 0.9)));
    }
    for k in 0..5 {
        let y0 = 300. + k as f32 * 40.;
        cv.arrow((100. + k as f32 * 60., y0), (900., 540. + k as f32 * 12.), rgba(120, 190, 255, 0.85), 6.);
    }
    water(cv, 900., 560., 1020., 272., true, 6);
    cv.round(0., 700., 900., 132., 0., &solid(rgba(30, 24, 18, 0.5)));
    cv.text("Frozen ground", 450., 780., 32., Weight::Bold, Color::WHITE, Al::Center, 0.);
    tag(cv, "Melting snow plus rain", (80., 120.), Some((300., 330.)), "#FFFFFF");
    tag(cv, "Frozen ground can't soak it up", (380., 620.), Some((450., 720.)), "#9FD2FF");
    tag(cv, "Creeks and rivers rise", (1280., 440.), Some((1400., 590.)), "#FFD34D");
}

pub fn sump_pump(cv: &mut Canvas) {
    // Basement cut-away with a sump pit, pump, discharge pipe and battery backup.
    cv.round(0., 0., 1920., 832., 0., &linear(0., 0., 0., 832., &[(0., hex("#5E9A3A")), (1., hex("#3E6A2A"))]));
    sky(cv, 0., 0., 1920., 200., Sky::Overcast);
    rain(cv, 0., 0., 1920., 200., 120, 0.0, 4);
    cv.round(300., 200., 1300., 600., 0., &linear(0., 200., 0., 800., &[(0., hex("#8E8578")), (1., hex("#6A6258"))]));
    cv.round(300., 760., 1300., 40., 0., &solid(hex("#55504A")));
    // Pit and water.
    cv.round(760., 620., 280., 180., 6., &solid(hex("#2A2620")));
    water(cv, 770., 700., 260., 96., true, 3);
    appliance(cv, 850., 620., 100., 120., "#3A6EA8");
    pipe(cv, (900., 620.), (900., 260.), 24., false);
    pipe(cv, (900., 260.), (1700., 260.), 24., false);
    pipe(cv, (1700., 260.), (1800., 330.), 24., false);
    appliance(cv, 1120., 660., 160., 120., "#2C3138");
    cv.round(1150., 690., 100., 30., 4., &solid(hex("#2ED16B")));
    // Ground water seeping in.
    for k in 0..4 {
        cv.arrow((200., 420. + k as f32 * 80.), (300., 420. + k as f32 * 80.), rgba(120, 190, 255, 0.85), 6.);
    }
    tag(cv, "Pump water away from the house", (1300., 140.), Some((1750., 290.)), "#FFFFFF");
    tag(cv, "Test the pump before spring rains", (400., 520.), Some((850., 660.)), "#FFD34D");
    tag(cv, "Battery backup for outages", (1320., 560.), Some((1200., 660.)), "#9FD2FF");
    let _ = mix(Color::WHITE, Color::BLACK, 0.5);
}
