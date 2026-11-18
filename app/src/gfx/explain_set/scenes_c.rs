//! Scenes, part 3: more home, travel, water and measuring topics. Canvas is 1920 × 832.

use tiny_skia::Color;

use super::kit::*;
use super::{divider, tag, tag_at};
use crate::gfx::history_set::paint::{ellipse, hex, linear, mix, poly, radial, rgba, rrect, smooth, solid, Al, Canvas};
use crate::gfx::text::Weight;

pub fn co_alarm(cv: &mut Canvas) {
    // House cut-away: bedrooms upstairs, furnace in the basement, alarms on each level.
    cv.round(0., 0., 1920., 832., 0., &linear(0., 0., 0., 832., &[(0., hex("#1E2E44")), (1., hex("#101A28"))]));
    let (x, w) = (360., 1200.);
    let floors = [(80., "#D9CDB8"), (330., "#CFC3AE"), (580., "#8E8578")];
    cv.fill(poly(&[(x - 40., 80.), (x + w / 2., -60.), (x + w + 40., 80.)], true), &solid(hex("#4D3B30")));
    for (y, wall) in floors {
        cv.round(x, y, w, 240., 0., &linear(x, 0., x + w, 0., &[(0., hex(wall)), (1., mix(hex(wall), Color::BLACK, 0.15))]));
        cv.round(x, y + 230., w, 14., 0., &solid(hex("#6E5236")));
    }
    // Beds upstairs.
    for bx in [x + 120., x + 700.] {
        cv.round(bx, 240., 300., 70., 10., &linear(0., 240., 0., 310., &[(0., hex("#F2F4F7")), (1., hex("#AEB6C0"))]));
        cv.round(bx, 200., 40., 110., 6., &solid(hex("#6E5236")));
        cv.round(bx + 50., 225., 80., 30., 10., &solid(Color::WHITE));
    }
    // Couch on the main floor; furnace and water heater in the basement.
    cv.round(x + 160., 470., 340., 90., 24., &linear(0., 470., 0., 560., &[(0., hex("#5C7A99")), (1., hex("#38506A"))]));
    appliance(cv, x + 820., 620., 180., 190., "#9AA2AA");
    cv.round(x + 860., 700., 100., 40., 6., &solid(hex("#FF7A2F")));
    appliance(cv, x + 1040., 640., 110., 170., "#C9D1DA");
    // Alarms.
    for (ax, ay) in [(x + 560., 110.), (x + 600., 360.), (x + 600., 610.)] {
        cv.fill(ellipse(ax, ay, 38., 14.), &radial(ax - 8., ay - 6., 46., &[(0., Color::WHITE), (1., hex("#C9D1DA"))]));
        cv.fill(ellipse(ax + 18., ay, 4., 4.), &solid(hex("#2ED16B")));
    }
    tag(cv, "CO alarm outside sleeping areas", (x + w + 60., 100.), Some((x + 600., 110.)), "#FFD34D");
    tag(cv, "One on every level", (x + w + 60., 350.), Some((x + 640., 360.)), "#FFD34D");
    tag(cv, "Fuel-burning furnace and water heater", (x + w + 60., 600.), Some((x + 1000., 660.)), "#FFFFFF");
}

pub fn heat_work(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Hazy);
    sun(cv, 1700., 110., 90.);
    ground(cv, 0., 600., 1920., 232., Ground::Dry, 3);
    // Shade canopy with a water cooler and a resting worker.
    let (tx, ty) = (300., 330.);
    cv.fill(poly(&[(tx - 40., ty), (tx + 520., ty), (tx + 470., ty + 60.), (tx + 10., ty + 60.)], true), &linear(0., ty, 0., ty + 60., &[(0., hex("#2E6DB4")), (1., hex("#1E4A80"))]));
    for px in [tx + 20., tx + 450.] {
        cv.line((px, ty + 60.), (px, 620.), hex("#9AA2AA"), 8.);
    }
    cv.fill(poly(&[(tx + 10., ty + 60.), (tx + 470., ty + 60.), (tx + 620., 640.), (tx - 120., 640.)], true), &solid(rgba(0, 0, 0, 0.18)));
    cv.round(tx + 300., 520., 90., 110., 12., &linear(0., 520., 0., 630., &[(0., hex("#F2B01E")), (1., hex("#B07A10"))]));
    cv.round(tx + 320., 500., 50., 24., 6., &solid(hex("#FFFFFF")));
    person(cv, tx + 150., 640., 230., Pose::Crouch, "#F28C28");
    // Worker out in the sun.
    person(cv, 1200., 660., 300., Pose::Shovel, "#F28C28");
    for k in 0..3 {
        cv.stroke(smooth(&[(1050. + k as f32 * 60., 420.), (1060. + k as f32 * 60., 380.), (1045. + k as f32 * 60., 340.)], false), rgba(255, 255, 255, 0.35), 4.);
    }
    tag(cv, "Rest in the shade", (tx - 60., 230.), Some((tx + 150., 420.)), "#FFD34D");
    tag(cv, "Drink water often", (tx + 420., 430.), Some((tx + 345., 520.)), "#9FD2FF");
    tag(cv, "Do heavy work early in the day", (1300., 300.), Some((1250., 420.)), "#FFFFFF");
}

pub fn pool_lightning(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Storm);
    cumulonimbus(cv, 1500., 380., 0.7, true);
    bolt(cv, (1520., 390.), (1580., 560.), 6);
    // Pool deck and water.
    cv.round(0., 520., 1920., 312., 0., &linear(0., 520., 0., 832., &[(0., hex("#D9D2C2")), (1., hex("#A39A86"))]));
    cv.round(160., 580., 1100., 220., 20., &solid(hex("#E9F1F5")));
    cv.round(180., 600., 1060., 180., 14., &linear(0., 600., 0., 780., &[(0., hex("#5FC3E8")), (1., hex("#1E7FB0"))]));
    let mut r = Rng::new(4);
    for _ in 0..30 {
        let (x, y) = (200. + r.f() * 1020., 610. + r.f() * 160.);
        cv.stroke(smooth(&[(x, y), (x + 20., y - 4.), (x + 40., y)], false), rgba(255, 255, 255, 0.35), 2.);
    }
    // Swimmers climbing out and walking to the building.
    person(cv, 1300., 600., 220., Pose::Walk, "#2E6DB4");
    person(cv, 1430., 600., 140., Pose::Child, "#C0392B");
    cv.round(1600., 300., 300., 300., 6., &linear(1600., 0., 1900., 0., &[(0., hex("#BFAF94")), (1., hex("#857659"))]));
    cv.round(1700., 460., 90., 140., 3., &solid(hex("#4A2E1E")));
    cv.arrow((1360., 690.), (1680., 690.), hex("#5BE08A"), 10.);
    tag(cv, "Out of the water at the first thunder", (120., 420.), Some((600., 600.)), "#FFD34D");
    tag(cv, "Into a building, not under an umbrella", (1180., 760.), Some((1600., 690.)), "#5BE08A");
}

pub fn boat_storm(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Storm);
    cumulonimbus(cv, 560., 420., 0.75, true);
    bolt(cv, (560., 430.), (600., 560.), 3);
    // Lake with a shoreline and dock on the right.
    cv.round(0., 520., 1920., 312., 0., &linear(0., 520., 0., 832., &[(0., hex("#3E6A80")), (1., hex("#1E3A4A"))]));
    let mut r = Rng::new(7);
    for _ in 0..80 {
        let (x, y) = (r.f() * 1600., 540. + r.f() * 280.);
        cv.stroke(smooth(&[(x, y), (x + 30., y - 8.), (x + 60., y)], false), rgba(255, 255, 255, 0.35), 2.5);
    }
    cv.fill(poly(&[(1500., 520.), (1920., 480.), (1920., 832.), (1640., 832.)], true), &linear(0., 480., 0., 832., &[(0., hex("#5E9A3A")), (1., hex("#2E5A1E"))]));
    cv.round(1380., 600., 280., 18., 3., &solid(hex("#7A5A3A")));
    // Small boat heading for shore with a wake.
    let (bx, by) = (1000., 640.);
    cv.fill(poly(&[(bx, by), (bx + 220., by), (bx + 260., by - 50.), (bx - 20., by - 50.)], true), &linear(0., by - 50., 0., by, &[(0., hex("#F2F4F7")), (1., hex("#9AA2AA"))]));
    cv.round(bx + 60., by - 100., 90., 50., 6., &solid(hex("#C0392B")));
    person(cv, bx + 170., by - 50., 100., Pose::Stand, "#F1C40F");
    for k in 0..4 {
        cv.stroke(smooth(&[(bx - 30. - k as f32 * 60., by - 20. + k as f32 * 10.), (bx - 60. - k as f32 * 60., by - 10. + k as f32 * 14.)], false), rgba(255, 255, 255, 0.6), 4.);
    }
    cv.arrow((bx + 280., by - 30.), (1360., 590.), hex("#5BE08A"), 10.);
    tag(cv, "Head for shore when storms build", (1080., 420.), Some((1250., 580.)), "#5BE08A");
    tag(cv, "Check the forecast before you go", (40., 760.), None, "#FFFFFF");
}

pub fn wind_truck(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Day);
    ground(cv, 0., 540., 1920., 292., Ground::Grass, 4);
    road(cv, 0., 600., 1920., 140., false);
    // Box truck leaning slightly with crosswind arrows.
    let (tx, gy) = (760., 700.);
    cv.round(tx, gy - 300., 520., 260., 6., &linear(0., gy - 300., 0., gy - 40., &[(0., hex("#F4F6F8")), (1., hex("#BCC4CC"))]));
    cv.round(tx + 520., gy - 200., 180., 160., 16., &linear(0., gy - 200., 0., gy - 40., &[(0., hex("#2E6DB4")), (1., hex("#1E4A80"))]));
    cv.round(tx + 600., gy - 185., 80., 60., 6., &linear(0., gy - 185., 0., gy - 125., &[(0., hex("#CFE6F5")), (1., hex("#5F86A3"))]));
    for wx in [tx + 90., tx + 420., tx + 610.] {
        cv.fill(ellipse(wx, gy - 20., 38., 38.), &solid(hex("#141416")));
        cv.fill(ellipse(wx, gy - 20., 16., 16.), &solid(hex("#9AA2AA")));
    }
    for k in 0..5 {
        let y = 180. + k as f32 * 80.;
        cv.arrow((80., y), (640., y + 20.), hex("#9FD2FF"), 10.);
    }
    for (i, x) in [200., 1500., 1700.].iter().enumerate() {
        tree(cv, *x, 560., 220., Tree::Leafy, 0.9, i as u64 + 2);
    }
    tag(cv, "Strong crosswinds", (80., 90.), None, "#9FD2FF");
    tag(cv, "Tall, empty trucks and trailers get pushed", (900., 300.), Some((1000., 420.)), "#FFD34D");
    tag(cv, "Both hands on the wheel", (1300., 760.), None, "#FFFFFF");
}

pub fn secure_yard(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    ground(cv, 0., 560., 1920., 272., Ground::Grass, 9);
    house(cv, 1300., 580., 520., 300., &HouseStyle { wall: "#D8CFC0", roof: "#5B4A3E", lit: false });
    // Trampoline anchored, patio chairs stacked, grill tied down, trash can secured.
    let (tx, ty) = (300., 560.);
    cv.fill(ellipse(tx, ty, 220., 46.), &radial(tx, ty, 220., &[(0., hex("#1A1A1A")), (1., hex("#333333"))]));
    cv.stroke(ellipse(tx, ty, 230., 50.), hex("#2E6DB4"), 10.);
    for lx in [tx - 180., tx - 60., tx + 60., tx + 180.] {
        cv.line((lx, ty + 10.), (lx, ty + 110.), hex("#8E979F"), 8.);
        cv.line((lx, ty + 110.), (lx + 30., ty + 140.), hex("#F2B01E"), 4.);
    }
    for k in 0..3 {
        cv.round(700., 560. - k as f32 * 26., 160., 20., 4., &solid(hex("#E9ECEF")));
    }
    cv.round(720., 580., 120., 40., 4., &solid(hex("#C9D1DA")));
    cv.round(960., 470., 160., 120., 14., &linear(0., 470., 0., 590., &[(0., hex("#3A3A3E")), (1., hex("#1A1A1C"))]));
    cv.line((950., 530.), (1130., 530.), hex("#F2B01E"), 6.);
    for k in 0..3 {
        let y = 140. + k as f32 * 70.;
        cv.arrow((300., y), (900., y + 10.), hex("#9FD2FF"), 9.);
    }
    tag(cv, "Anchor trampolines", (100., 720.), Some((300., 660.)), "#FFD34D");
    tag(cv, "Bring in or stack light furniture", (560., 380.), Some((780., 500.)), "#FFFFFF");
    tag(cv, "Tie down grills and cans", (920., 650.), Some((1040., 590.)), "#FFFFFF");
}

pub fn ice_dams(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Winter);
    ground(cv, 0., 700., 1920., 132., Ground::Snow, 3);
    // Close cut-away of a roof edge: warm attic, snow melting, refreezing at the eave.
    let roof = [(160., 640.), (1160., 140.), (1180., 170.), (200., 680.)];
    cv.fill(poly(&roof, true), &solid(hex("#4D3B30")));
    cv.fill(poly(&[(200., 680.), (1180., 170.), (1180., 700.), (200., 700.)], true), &linear(0., 170., 0., 700., &[(0., hex("#FFB070")), (1., hex("#E07A3A"))]));
    cv.fill(poly(&[(160., 630.), (1150., 135.), (1150., 95.), (130., 600.)], true), &solid(Color::WHITE));
    // Ice dam ridge at the eave and icicles.
    cv.fill(smooth(&[(120., 620.), (220., 570.), (300., 560.), (330., 600.), (220., 650.)], true), &radial(220., 590., 120., &[(0., hex("#E9F5FF")), (1., hex("#9EC3DB"))]));
    for k in 0..7 {
        let ix = 140. + k as f32 * 26.;
        let iy = 640. - k as f32 * 13.;
        cv.fill(poly(&[(ix - 7., iy), (ix + 7., iy), (ix, iy + 60. + (k % 3) as f32 * 25.)], true), &linear(0., iy, 0., iy + 100., &[(0., rgba(230, 245, 255, 0.95)), (1., rgba(150, 200, 230, 0.6))]));
    }
    water(cv, 330., 520., 240., 30., false, 2);
    for k in 0..3 {
        cv.arrow((700. + k as f32 * 120., 600.), (700. + k as f32 * 120., 400. - k as f32 * 60.), hex("#FF7A3A"), 8.);
    }
    tag(cv, "Heat escapes from the attic", (820., 640.), Some((820., 540.)), "#FFB08A");
    tag(cv, "Snow melts on the warm roof", (600., 260.), Some((620., 400.)), "#FFFFFF");
    tag(cv, "Refreezes at the cold edge: ice dam", (60., 380.), Some((220., 570.)), "#9FD2FF");
    tag(cv, "Insulate and vent the attic", (1250., 400.), None, "#FFD34D");
}

pub fn storm_life_cycle(cv: &mut Canvas) {
    let titles = ["CUMULUS", "MATURE", "DISSIPATING"];
    for i in 0..3 {
        let x = i as f32 * 640.;
        sky(cv, x, 0., 640., 832., if i == 1 { Sky::Storm } else { Sky::Day });
        ground(cv, x, 700., 640., 132., Ground::Grass, i as u64 + 3);
        let cx = x + 320.;
        match i {
            0 => {
                cloud(cv, cx, 480., 1.3, 0.05, 4);
                cloud(cv, cx + 10., 380., 1.0, 0.0, 5);
                for dx in [-60., 0., 60.] {
                    cv.arrow((cx + dx, 680.), (cx + dx, 520.), hex("#FF7A3A"), 9.);
                }
            }
            1 => {
                cumulonimbus(cv, cx - 20., 520., 0.62, true);
                cv.arrow((cx + 90., 660.), (cx + 90., 420.), hex("#FF7A3A"), 9.);
                cv.arrow((cx - 140., 460.), (cx - 140., 660.), hex("#4AA3FF"), 9.);
                bolt(cv, (cx + 40., 530.), (cx + 70., 690.), 3);
            }
            _ => {
                cv.fill(smooth(&[(x + 60., 200.), (x + 300., 150.), (x + 580., 190.), (x + 560., 250.), (x + 300., 260.), (x + 80., 250.)], true), &solid(rgba(220, 225, 232, 0.9)));
                cloud(cv, cx, 470., 1.1, 0.55, 6);
                rain(cv, cx - 120., 510., 240., 180., 60, 0.0, 7);
                for dx in [-60., 60.] {
                    cv.arrow((cx + dx, 520.), (cx + dx, 680.), hex("#4AA3FF"), 9.);
                }
            }
        }
        tag_at(cv, titles[i], x + 30., 40., if i == 1 { "#FF6B5A" } else { "#FFFFFF" }, 40.);
    }
    divider(cv, 640.);
    divider(cv, 1280.);
    tag(cv, "Updrafts build the cloud", (60., 760.), None, "#FFB08A");
    tag(cv, "Updrafts and downdrafts: rain, lightning", (680., 760.), None, "#FFD34D");
    tag(cv, "Downdrafts take over", (1320., 760.), None, "#9FD2FF");
}

pub fn underpass_flood(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    rain(cv, 0., 0., 1920., 500., 220, -0.05, 3);
    // Bridge deck over a dipped road with water pooled underneath.
    cv.round(0., 220., 1920., 70., 0., &linear(0., 220., 0., 290., &[(0., hex("#B8B3A8")), (1., hex("#7E786C"))]));
    for px in [520., 1340.] {
        cv.round(px, 290., 70., 300., 4., &linear(px, 0., px + 70., 0., &[(0., hex("#A9A398")), (1., hex("#6E685D"))]));
    }
    let profile: Vec<(f32, f32)> = (0..=48).map(|i| { let x = i as f32 * 40.; let d = ((x - 960.) / 520.).clamp(-1., 1.); (x, 560. + (1. - d * d).max(0.) * 140.) }).collect();
    let mut road_body = profile.clone();
    road_body.push((1920., 832.));
    road_body.push((0., 832.));
    cv.fill(poly(&road_body, true), &linear(0., 560., 0., 832., &[(0., hex("#4A4D52")), (1., hex("#2C2E31"))]));
    water(cv, 460., 600., 1000., 232., true, 5);
    car(cv, 60., 568., 300., "#B03A2E", Car::Sedan);
    // Depth markers on the pillar.
    for (k, label) in ["1 ft", "2 ft", "3 ft"].iter().enumerate() {
        let y = 690. - k as f32 * 60.;
        cv.line((600., y), (640., y), Color::WHITE, 4.);
        cv.text(label, 650., y + 8., 22., Weight::Bold, Color::WHITE, Al::Left, 0.);
    }
    tag(cv, "Underpasses fill first", (900., 380.), Some((960., 620.)), "#FFD34D");
    tag(cv, "Find another way", (60., 420.), Some((210., 520.)), "#5BE08A");
}

pub fn flood_stages(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    // River bank cross-section with a staff gauge and the NWS flood categories.
    cv.fill(poly(&[(0., 300.), (420., 360.), (620., 700.), (1300., 700.), (1500., 360.), (1920., 300.), (1920., 832.), (0., 832.)], true), &linear(0., 300., 0., 832., &[(0., hex("#6E8A48")), (1., hex("#3E4C2A"))]));
    water(cv, 520., 520., 880., 312., true, 3);
    house(cv, 1560., 300., 240., 140., &HouseStyle { wall: "#D8CFC0", roof: "#5B4A3E", lit: false });
    tree(cv, 250., 320., 220., Tree::Leafy, 0., 2);
    let gx = 980.;
    cv.round(gx - 18., 120., 36., 620., 4., &solid(Color::WHITE));
    let stages = [("Major", "#B35CF0", 200.), ("Moderate", "#FF4A3D", 300.), ("Minor", "#FF9A2E", 400.), ("Action", "#F2D04B", 480.)];
    for (name, c, y) in stages {
        cv.round(gx + 24., y, 260., 50., 8., &solid(hex(c)));
        cv.text(name, gx + 40., y + 36., 30., Weight::Bold, hex("#111418"), Al::Left, 0.);
        cv.line((gx - 30., y + 25.), (gx + 24., y + 25.), hex(c), 6.);
    }
    for k in 0..12 {
        let y = 140. + k as f32 * 50.;
        cv.line((gx - 18., y), (gx, y), hex("#222222"), 3.);
    }
    tag(cv, "River level now", (560., 560.), Some((gx - 20., 520.)), "#9FD2FF");
    tag(cv, "Water reaches homes and roads", (1380., 160.), Some((1500., 300.)), "#FFFFFF");
}

pub fn lightning_indoors(cv: &mut Canvas) {
    let floor = room(cv, 0., 0., 1920., 832., "#DCD2C2", true, false);
    bolt(cv, (1480., 0.), (1500., 330.), 3);
    // Kitchen sink (avoid), corded phone (avoid), person on couch with a cell phone (fine).
    let sx = 260.;
    cv.round(sx, 400., 420., 30., 4., &solid(hex("#5B5F66")));
    cv.round(sx, 430., 420., floor - 430., 4., &linear(sx, 0., sx + 420., 0., &[(0., hex("#F4F1EA")), (1., hex("#CFC8BA"))]));
    cv.stroke(smooth(&[(sx + 210., 400.), (sx + 240., 340.), (sx + 300., 350.), (sx + 305., 390.)], false), hex("#B9C0C8"), 14.);
    for k in 0..4 {
        cv.fill(ellipse(sx + 305., 405. + k as f32 * 18., 4., 6.), &solid(hex("#6FB6E8")));
    }
    cv.line((sx - 20., 330.), (sx + 450., 500.), hex("#E53935"), 12.);
    cv.line((sx + 450., 330.), (sx - 20., 500.), hex("#E53935"), 12.);
    // Couch and person with a cell phone.
    cv.round(860., floor - 60., 420., 150., 30., &linear(0., floor - 60., 0., floor + 90., &[(0., hex("#6E8DAD")), (1., hex("#4A6684"))]));
    person(cv, 1020., floor + 40., 220., Pose::Crouch, "#2E6DB4");
    cv.round(1060., floor - 120., 22., 38., 4., &solid(hex("#1B1F24")));
    tag(cv, "Skip showers, dishes and baths", (180., 220.), Some((sx + 210., 330.)), "#FF6B5A");
    tag(cv, "Stay off corded phones; cell phones are fine", (820., 260.), Some((1070., floor - 120.)), "#FFFFFF");
    tag(cv, "Stay away from windows", (1360., 560.), Some((1430., 420.)), "#FFD34D");
}

pub fn heat_island(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Dusk);
    // Countryside on the left, city on the right, a temperature line across.
    ground(cv, 0., 600., 760., 232., Ground::Grass, 5);
    for (i, x) in [120., 300., 520., 680.].iter().enumerate() {
        tree(cv, *x, 620., 200., Tree::Leafy, 0., i as u64);
    }
    cv.round(760., 600., 1160., 232., 0., &linear(0., 600., 0., 832., &[(0., hex("#5A5D62")), (1., hex("#2C2E31"))]));
    let mut r = Rng::new(9);
    let mut x = 800.;
    while x < 1880. {
        let w = r.range(90., 160.);
        let h = r.range(200., 480.);
        cv.round(x, 600. - h, w, h, 2., &linear(x, 0., x + w, 0., &[(0., hex("#5E6B7C")), (1., hex("#2E3644"))]));
        let mut wy = 600. - h + 20.;
        while wy < 580. {
            let mut wx = x + 12.;
            while wx < x + w - 16. {
                if r.f() > 0.35 {
                    cv.round(wx, wy, 12., 16., 1., &solid(rgba(255, 220, 140, 0.85)));
                }
                wx += 24.;
            }
            wy += 30.;
        }
        x += w + 14.;
    }
    // Heat shimmer over the city.
    for k in 0..4 {
        cv.arrow((1000. + k as f32 * 220., 300.), (1000. + k as f32 * 220., 140.), rgba(255, 140, 90, 0.85), 7.);
    }
    let pts: Vec<(f32, f32)> = (0..=24).map(|i| { let t = i as f32 / 24.; let x = 60. + t * 1800.; (x, 420. - (1. / (1. + (-(x - 900.) / 120.).exp())) * 120.) }).collect();
    cv.stroke(smooth(&pts, false), hex("#FFD34D"), 6.);
    tag(cv, "Cooler: grass and trees", (60., 470.), None, "#9FD2FF");
    tag(cv, "Warmer: pavement and buildings hold heat", (1000., 90.), None, "#FFB08A");
}

pub fn measure_snow(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Winter);
    ground(cv, 0., 420., 1920., 412., Ground::Snow, 7);
    // A white snow board with a ruler pushed straight down; three spots averaged.
    for (i, &x) in [380f32, 900., 1420.].iter().enumerate() {
        let depth = [5.5f32, 6.0, 6.5][i];
        cv.round(x - 120., 640., 240., 18., 3., &solid(hex("#F4F6F8")));
        cv.fill(poly(&[(x - 120., 640.), (x + 120., 640.), (x + 110., 640. - depth * 30.), (x - 110., 640. - depth * 30.)], true), &radial(x, 640. - depth * 20., 200., &[(0., Color::WHITE), (1., hex("#D5E2EE"))]));
        cv.round(x - 14., 640. - 300., 28., 300., 3., &linear(x - 14., 0., x + 14., 0., &[(0., hex("#F2D04B")), (1., hex("#C9A82E"))]));
        for k in 0..=10 {
            cv.line((x - 14., 640. - k as f32 * 30.), (x - 2., 640. - k as f32 * 30.), hex("#222222"), 2.);
        }
        cv.text(&format!("{depth:.1}\""), x, 300., 40., Weight::Bold, hex("#1E2E44"), Al::Center, 0.);
    }
    tag(cv, "Measure in a few open spots and average", (520., 100.), None, "#FFD34D");
    tag(cv, "Ruler straight down to the board", (1500., 720.), Some((1420., 600.)), "#FFFFFF");
    tag(cv, "Average: 6.0 inches", (120., 720.), None, "#9FD2FF");
}

pub fn measure_rain(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    ground(cv, 0., 640., 1920., 192., Ground::Grass, 5);
    rain(cv, 0., 0., 1920., 600., 200, 0.0, 3);
    // 4-inch clear rain gauge on a post, away from the house and trees.
    let (gx, gy) = (960., 260.);
    cv.round(gx - 10., gy + 300., 20., 140., 3., &solid(hex("#7A5A3A")));
    cv.round(gx - 70., gy, 140., 300., 18., &linear(gx - 70., 0., gx + 70., 0., &[(0., rgba(255, 255, 255, 0.75)), (0.5, rgba(255, 255, 255, 0.25)), (1., rgba(220, 230, 240, 0.7))]));
    cv.fill(poly(&[(gx - 80., gy), (gx + 80., gy), (gx + 30., gy + 50.), (gx - 30., gy + 50.)], true), &solid(rgba(240, 245, 250, 0.9)));
    water(cv, gx - 30., gy + 180., 60., 110., false, 2);
    cv.round(gx - 30., gy + 50., 60., 240., 6., &solid(rgba(255, 255, 255, 0.12)));
    for k in 0..=10 {
        cv.line((gx + 30., gy + 290. - k as f32 * 24.), (gx + 50., gy + 290. - k as f32 * 24.), hex("#1E2E44"), 2.);
    }
    house(cv, 120., 660., 360., 230., &HouseStyle { wall: "#D8CFC0", roof: "#5B4A3E", lit: false });
    tree(cv, 1650., 660., 360., Tree::Leafy, 0., 3);
    cv.dashed(poly(&[(500., 700.), (880., 700.)], false), Color::WHITE, 4., 14., 10.);
    cv.dashed(poly(&[(1040., 700.), (1480., 700.)], false), Color::WHITE, 4., 14., 10.);
    tag(cv, "Open spot, away from roofs and trees", (560., 740.), None, "#FFFFFF");
    tag(cv, "Read at the same time every day", (1120., 160.), Some((1010., 300.)), "#FFD34D");
}

pub fn plow_room(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Winter);
    snow(cv, 0., 0., 1920., 560., 160, 4);
    ground(cv, 0., 520., 1920., 312., Ground::Snow, 6);
    road(cv, 0., 580., 1920., 160., false);
    // Plow truck with blade, salt spinner spray, flashing amber light.
    let (px, gy) = (980., 700.);
    cv.round(px, gy - 230., 380., 190., 8., &linear(0., gy - 230., 0., gy - 40., &[(0., hex("#F28C28")), (1., hex("#B0560E"))]));
    cv.round(px + 380., gy - 170., 160., 130., 12., &linear(0., gy - 170., 0., gy - 40., &[(0., hex("#F28C28")), (1., hex("#B0560E"))]));
    cv.round(px + 420., gy - 155., 90., 55., 6., &linear(0., gy - 155., 0., gy - 100., &[(0., hex("#CFE6F5")), (1., hex("#5F86A3"))]));
    cv.fill(poly(&[(px + 540., gy - 110.), (px + 640., gy - 150.), (px + 660., gy - 10.), (px + 560., gy)], true), &linear(0., gy - 150., 0., gy, &[(0., hex("#F2D04B")), (1., hex("#A88A1E"))]));
    cv.fill(ellipse(px + 460., gy - 180., 22., 12.), &radial(px + 460., gy - 180., 60., &[(0., rgba(255, 200, 60, 1.)), (1., rgba(255, 200, 60, 0.))]));
    for wx in [px + 70., px + 300., px + 470.] {
        cv.fill(ellipse(wx, gy - 20., 40., 40.), &solid(hex("#141416")));
        cv.fill(ellipse(wx, gy - 20., 16., 16.), &solid(hex("#9AA2AA")));
    }
    let mut r = Rng::new(3);
    for _ in 0..90 {
        let a = r.range(2.6, 3.7);
        let d = r.range(20., 160.);
        cv.fill(ellipse(px - 10. + a.cos() * d, gy - 30. + a.sin() * d * 0.4, 3., 3.), &solid(rgba(240, 240, 240, 0.9)));
    }
    car(cv, 160., 720., 300., "#2E6DB4", Car::Sedan);
    cv.line((520., 760.), (940., 760.), Color::WHITE, 4.);
    tag(cv, "Stay well back; don't pass a plow", (360., 440.), Some((700., 700.)), "#FFD34D");
    tag(cv, "Salt and sand spray behind it", (1080., 380.), Some((1000., 640.)), "#FFFFFF");
}

pub fn drifting_snow(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Winter);
    // Open field, wind from the left, snow drifting across a road behind a fence line.
    ground(cv, 0., 460., 1920., 372., Ground::Snow, 8);
    for k in 0..6 {
        let y = 160. + k as f32 * 50.;
        cv.arrow((60., y), (560., y + 20.), hex("#9FD2FF"), 8.);
    }
    let mut r = Rng::new(5);
    for _ in 0..240 {
        let (x, y) = (r.f() * 1920., 380. + r.f() * 200.);
        cv.line((x, y), (x + r.range(20., 50.), y + r.range(-3., 3.)), rgba(255, 255, 255, 0.55), 2.);
    }
    road(cv, 0., 620., 1920., 110., false);
    for (cx, w) in [(700., 300.), (1200., 380.), (1650., 260.)] {
        cv.fill(smooth(&[(cx - w / 2., 730.), (cx - w * 0.2, 640.), (cx + w * 0.1, 610.), (cx + w / 2., 700.), (cx + w / 2., 730.)], true), &radial(cx, 650., w * 0.6, &[(0., Color::WHITE), (1., hex("#C9D7E6"))]));
    }
    for k in 0..14 {
        let x = 60. + k as f32 * 130.;
        cv.line((x, 600.), (x, 520.), hex("#6E5236"), 6.);
    }
    cv.line((60., 540.), (1750., 540.), hex("#6E5236"), 3.);
    tag(cv, "Wind across open fields", (80., 80.), None, "#9FD2FF");
    tag(cv, "Drifts block roads, even after the snow stops", (900., 760.), Some((1200., 680.)), "#FFD34D");
}
