//! Scenes, part 2: home and winter safety, heat, fog, sky. Canvas is 1920 × 832 design units.

use tiny_skia::Color;

use super::kit::*;
use super::{divider, tag, tag_at};
use crate::gfx::history_set::paint::{ellipse, hex, linear, mix, poly, radial, rgba, rrect, smooth, solid, Al, Canvas};
use crate::gfx::text::Weight;

/// Owner's reference: summer (counter-clockwise, air pushed down) vs winter (clockwise, low,
/// warm air pushed up and down the walls).
pub fn fans(cv: &mut Canvas) {
    for (x, winter) in [(0., false), (960., true)] {
        cv.set_clip(None);
        let floor = room(cv, x, 0., 960., 832., if winter { "#C9B79C" } else { "#BFD3DF" }, true, winter);
        let cx = x + 400.;
        ceiling_fan(cv, cx, 36., 520.);
        let blue = hex("#2F8BFF");
        let red = hex("#FF4A3D");
        // Rotation arrows around the hub, seen from below.
        let (rx, ry, hy) = (300., 40., 36. + 520. * 0.18 + 20.);
        let arc: Vec<(f32, f32)> = (0..=14)
            .map(|i| {
                let t = i as f32 / 14.;
                let a = if winter { 3.6 - t * 2.4 } else { 0.9 + t * 2.4 };
                (cx + a.cos() * rx, hy + a.sin() * ry + 40.)
            })
            .collect();
        flow(cv, &arc, if winter { red } else { blue }, 9.);
        if winter {
            // Air drawn up the middle, across the ceiling and down the walls.
            for side in [-1f32, 1.] {
                flow(cv, &[(cx + side * 80., floor - 60.), (cx + side * 60., 420.), (cx + side * 40., 230.)], red, 9.);
                flow(cv, &[(cx + side * 120., 70.), (cx + side * 330., 64.), (cx + side * 390., 200.), (cx + side * 390., floor - 60.)], red, 9.);
            }
        } else {
            for dx in [-160., 0., 160.] {
                cv.arrow((cx + dx, 230.), (cx + dx, floor - 70.), blue, 11.);
            }
        }
        // A person in the breeze (summer) / sitting area (winter).
        person(cv, x + 160., floor + 60., 260., Pose::Stand, if winter { "#8E3B2E" } else { "#2E6DB4" });
    }
    cv.set_clip(None);
    divider(cv, 960.);
    tag_at(cv, "SUMMER", 30., 46., "#2F8BFF", 44.);
    tag_at(cv, "WINTER", 990., 46., "#FF4A3D", 44.);
    tag(cv, "Counter-clockwise", (430., 200.), None, "#9FD2FF");
    tag(cv, "Clockwise, low speed", (1320., 300.), None, "#FFB0A8");
}

fn temp_column(cv: &mut Canvas, x: f32, warm_layer: bool, cold_surface_layer: f32) {
    // Small vertical temperature profile: red = above freezing, blue = below.
    let (top, bottom) = (60., 600.);
    cv.round(x, top, 46., bottom - top, 14., &solid(rgba(0, 0, 0, 0.45)));
    let blue = hex("#4AA3FF");
    let red = hex("#FF5A4A");
    cv.round(x + 8., top + 8., 30., bottom - top - 16., 10., &solid(blue));
    if warm_layer {
        let wl = top + (bottom - top) * 0.38;
        let wh = (bottom - top) * (0.62 - cold_surface_layer) - 10.;
        cv.round(x + 8., wl, 30., wh.max(10.), 8., &solid(red));
    }
    cv.text("32°F", x + 54., top + (bottom - top) * 0.38 + 8., 22., Weight::SemiBold, Color::WHITE, Al::Left, 0.);
}

pub fn precip_ground(cv: &mut Canvas) {
    let kinds = [("SNOW", false, 0.0), ("SLEET", true, 0.3), ("FREEZING RAIN", true, 0.08)];
    for (i, (name, warm, cold)) in kinds.iter().enumerate() {
        let x = i as f32 * 640.;
        sky(cv, x, 0., 640., 832., Sky::Winter);
        match i {
            0 => {
                ground(cv, x, 600., 640., 232., Ground::Snow, 3);
                tree(cv, x + 420., 620., 340., Tree::Pine, 0., 2);
                for k in 0..5 {
                    let ty = 620. - 340. * (0.25 + k as f32 * 0.16);
                    cv.fill(ellipse(x + 420., ty, 110. - k as f32 * 18., 10.), &solid(rgba(255, 255, 255, 0.95)));
                }
                snow(cv, x + 80., 40., 560., 560., 160, 4);
            }
            1 => {
                ground(cv, x, 600., 640., 232., Ground::Ice, 5);
                car(cv, x + 140., 640., 420., "#5B6B7C", Car::Sedan);
                sleet(cv, x + 80., 40., 560., 560., 220, 6);
                for k in 0..30 {
                    let bx = x + 100. + (k * 17 % 500) as f32;
                    cv.fill(ellipse(bx, 640. + (k % 4) as f32 * 30., 4., 4.), &solid(rgba(220, 240, 255, 0.95)));
                }
            }
            _ => {
                ground(cv, x, 600., 640., 232., Ground::Ice, 7);
                rain(cv, x + 80., 40., 560., 500., 160, 0.0, 8);
                // Ice-glazed branch and power line with icicles.
                cv.line((x + 60., 300.), (x + 640., 330.), hex("#2A2A2A"), 4.);
                cv.line((x + 60., 300.), (x + 640., 330.), rgba(220, 240, 255, 0.6), 12.);
                cv.line((x + 120., 460.), (x + 560., 420.), hex("#4A3423"), 16.);
                cv.line((x + 120., 460.), (x + 560., 420.), rgba(210, 235, 255, 0.55), 26.);
                for k in 0..12 {
                    let ix = x + 140. + k as f32 * 36.;
                    let iy = 460. - (ix - x - 120.) * 40. / 440. + 10.;
                    cv.fill(poly(&[(ix - 6., iy), (ix + 6., iy), (ix, iy + 30. + (k % 3) as f32 * 14.)], true), &linear(0., iy, 0., iy + 50., &[(0., rgba(230, 245, 255, 0.95)), (1., rgba(160, 200, 230, 0.6))]));
                }
            }
        }
        temp_column(cv, x + 26., *warm, *cold);
        tag_at(cv, name, x + 110., 40., "#FFFFFF", 40.);
    }
    divider(cv, 640.);
    divider(cv, 1280.);
    tag(cv, "Cold all the way down", (110., 650.), None, "#9FD2FF");
    tag(cv, "Refreezes before landing", (750., 650.), None, "#9FD2FF");
    tag(cv, "Freezes on contact", (1390., 650.), None, "#9FD2FF");
}

pub fn frozen_pipes(cv: &mut Canvas) {
    // Kitchen: outside wall cutaway on the left, sink cabinet with doors open.
    let floor = room(cv, 0., 0., 1920., 832., "#E8DCC6", false, true);
    // Exterior wall cutaway with insulation.
    cv.round(0., 0., 260., floor, 0., &linear(0., 0., 260., 0., &[(0., hex("#AFC2D2")), (1., hex("#DCE6EE"))]));
    snow(cv, 0., 0., 110., floor, 60, 2);
    cv.round(110., 0., 150., floor, 0., &solid(hex("#F2C6D2")));
    let mut r = Rng::new(4);
    for _ in 0..80 {
        let (fx, fy) = (115. + r.f() * 140., r.f() * floor);
        cv.line((fx, fy), (fx + r.range(-14., 14.), fy + r.range(4., 14.)), rgba(220, 140, 165, 0.8), 2.);
    }
    // Cabinet, counter, sink and faucet.
    let (cx, cw) = (700., 640.);
    cv.round(cx, 380., cw, floor - 380., 6., &linear(cx, 0., cx + cw, 0., &[(0., hex("#F4F1EA")), (1., hex("#CFC8BA"))]));
    cv.round(cx + 30., 420., cw - 60., floor - 450., 4., &solid(hex("#3B342C")));
    // Open doors swung out.
    cv.fill(poly(&[(cx + 30., 420.), (cx - 120., 450.), (cx - 120., floor - 10.), (cx + 30., floor - 30.)], true), &linear(cx - 120., 0., cx + 30., 0., &[(0., hex("#DCD5C8")), (1., hex("#F2EDE4"))]));
    cv.fill(poly(&[(cx + cw - 30., 420.), (cx + cw + 120., 450.), (cx + cw + 120., floor - 10.), (cx + cw - 30., floor - 30.)], true), &linear(cx + cw - 30., 0., cx + cw + 120., 0., &[(0., hex("#F2EDE4")), (1., hex("#DCD5C8"))]));
    cv.round(cx - 20., 350., cw + 40., 40., 6., &linear(0., 350., 0., 390., &[(0., hex("#5B5F66")), (1., hex("#2E3136"))]));
    // Pipes from the outside wall to under the sink, with foam sleeves.
    pipe(cv, (110., 600.), (cx + 200., 600.), 22., true);
    pipe(cv, (cx + 200., 600.), (cx + 200., 400.), 22., true);
    pipe(cv, (110., 660.), (cx + 420., 660.), 22., true);
    pipe(cv, (cx + 420., 660.), (cx + 420., 400.), 22., true);
    for (a, b) in [((300., 600.), (640., 600.)), ((300., 660.), (640., 660.))] {
        cv.line(a, b, hex("#2B2B2B"), 40.);
        cv.line(a, (b.0, b.1 - 10.), rgba(255, 255, 255, 0.12), 6.);
    }
    // Faucet with a drip.
    let fx = cx + 320.;
    cv.round(fx - 14., 270., 28., 80., 8., &linear(fx - 14., 0., fx + 14., 0., &[(0., hex("#F2F4F6")), (1., hex("#8E979F"))]));
    cv.stroke(smooth(&[(fx, 280.), (fx + 40., 230.), (fx + 110., 240.), (fx + 120., 290.)], false), hex("#B9C0C8"), 18.);
    cv.fill(ellipse(fx + 120., 320., 7., 11.), &radial(fx + 118., 316., 12., &[(0., Color::WHITE), (1., hex("#6FB6E8"))]));
    // Thermostat.
    cv.round(1580., 260., 150., 150., 20., &linear(0., 260., 0., 410., &[(0., hex("#FFFFFF")), (1., hex("#D5DBE1"))]));
    cv.text("55°", 1655., 360., 64., Weight::Bold, hex("#2C3440"), Al::Center, 0.);
    tag(cv, "Open cabinet doors", (cx + cw + 160., 520.), Some((cx + cw + 100., 560.)), "#FFD34D");
    tag(cv, "Let a faucet drip", (fx + 160., 200.), Some((fx + 125., 300.)), "#FFD34D");
    tag(cv, "Insulate pipes near outside walls", (300., 720.), Some((470., 680.)), "#FFD34D");
    tag(cv, "Keep heat on, at least 55°F", (1420., 450.), Some((1600., 400.)), "#FFD34D");
}

pub fn car_kit(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Winter);
    ground(cv, 0., 560., 1920., 272., Ground::Snow, 4);
    car(cv, 60., 600., 560., "#3C4E66", Car::Suv);
    snow(cv, 0., 0., 1920., 560., 160, 3);
    // Items laid out on a tarp.
    cv.round(700., 560., 1180., 250., 18., &linear(0., 560., 0., 810., &[(0., hex("#2E5E8E")), (1., hex("#1E3E5E"))]));
    // Blanket.
    cv.round(730., 590., 220., 150., 16., &linear(730., 590., 950., 740., &[(0., hex("#C0392B")), (1., hex("#7E1F16"))]));
    for k in 0..4 {
        cv.line((730., 610. + k as f32 * 36.), (950., 610. + k as f32 * 36.), rgba(255, 255, 255, 0.25), 6.);
    }
    // Flashlight.
    cv.round(990., 640., 150., 40., 12., &linear(0., 640., 0., 680., &[(0., hex("#4A4F55")), (1., hex("#1E2124"))]));
    cv.round(1130., 630., 40., 60., 8., &linear(0., 630., 0., 690., &[(0., hex("#9AA2AA")), (1., hex("#5A6168"))]));
    cv.fill(ellipse(1168., 660., 12., 26.), &solid(hex("#FFF4C2")));
    // Water bottles.
    for k in 0..3 {
        let bx = 1210. + k as f32 * 46.;
        cv.round(bx, 600., 38., 130., 12., &linear(bx, 0., bx + 38., 0., &[(0., rgba(200, 230, 255, 0.95)), (1., rgba(110, 170, 220, 0.9))]));
        cv.round(bx + 10., 586., 18., 16., 4., &solid(hex("#2E6DB4")));
    }
    // Jumper cables.
    cv.stroke(smooth(&[(1380., 700.), (1420., 620.), (1500., 700.), (1560., 620.), (1620., 700.)], false), hex("#C62828"), 10.);
    cv.stroke(smooth(&[(1390., 720.), (1440., 640.), (1510., 720.), (1570., 640.), (1630., 720.)], false), hex("#1E1E1E"), 10.);
    // Ice scraper and shovel.
    cv.fill(poly(&[(1660., 600.), (1760., 600.), (1740., 640.), (1680., 640.)], true), &solid(hex("#F1C40F")));
    cv.line((1710., 640.), (1710., 760.), hex("#F1C40F"), 14.);
    cv.line((1800., 590.), (1800., 720.), hex("#8A6A45"), 12.);
    cv.fill(poly(&[(1770., 720.), (1830., 720.), (1840., 790.), (1760., 790.)], true), &solid(hex("#3E5E8E")));
    tag(cv, "Blankets", (730., 470.), Some((840., 590.)), "#FFFFFF");
    tag(cv, "Flashlight", (980., 520.), Some((1060., 640.)), "#FFFFFF");
    tag(cv, "Water and snacks", (1180., 440.), Some((1260., 590.)), "#FFFFFF");
    tag(cv, "Jumper cables", (1400., 520.), Some((1500., 640.)), "#FFFFFF");
    tag(cv, "Scraper, shovel", (1640., 470.), Some((1720., 600.)), "#FFFFFF");
    tag(cv, "Full tank of gas", (120., 300.), Some((300., 470.)), "#FFD34D");
}

pub fn bridge_ice(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Winter);
    ground(cv, 0., 520., 1920., 312., Ground::Snow, 6);
    // Creek valley under the bridge.
    cv.fill(poly(&[(560., 520.), (760., 760.), (1160., 760.), (1360., 520.)], true), &linear(0., 520., 0., 760., &[(0., hex("#C9D7E6")), (1., hex("#7F98B0"))]));
    water(cv, 760., 720., 400., 60., false, 3);
    // Bridge deck and road on the ground.
    road(cv, 0., 500., 560., 40., false);
    road(cv, 1360., 500., 560., 40., false);
    cv.round(560., 488., 800., 40., 4., &linear(0., 488., 0., 528., &[(0., hex("#C6D3DE")), (1., hex("#7E8C98"))]));
    cv.round(560., 470., 800., 18., 2., &solid(hex("#9AA6B0")));
    for px in [760., 1160.] {
        cv.round(px - 20., 528., 40., 200., 4., &solid(hex("#8E979F")));
    }
    // Cold air above and below the deck; ground heat elsewhere.
    for x in [700., 960., 1220.] {
        cv.arrow((x, 330.), (x, 450.), hex("#4AA3FF"), 10.);
        cv.arrow((x, 700.), (x, 560.), hex("#4AA3FF"), 10.);
    }
    for x in [200., 1700.] {
        cv.arrow((x, 640.), (x, 560.), hex("#FF8A4A"), 10.);
    }
    car(cv, 140., 500., 280., "#B03A2E", Car::Sedan);
    // Warning sign.
    let (sx, sy) = (1480., 260.);
    cv.line((sx, sy + 120.), (sx, 500.), hex("#8E979F"), 8.);
    cv.fill(poly(&[(sx, sy - 70.), (sx + 110., sy + 40.), (sx, sy + 150.), (sx - 110., sy + 40.)], true), &solid(hex("#F7C600")));
    cv.text("BRIDGE", sx, sy + 20., 26., Weight::Bold, Color::BLACK, Al::Center, 150.);
    cv.text("ICES", sx, sy + 50., 26., Weight::Bold, Color::BLACK, Al::Center, 150.);
    cv.text("BEFORE ROAD", sx, sy + 80., 20., Weight::Bold, Color::BLACK, Al::Center, 140.);
    tag(cv, "Cold air above and below", (760., 240.), Some((960., 330.)), "#9FD2FF");
    tag(cv, "Ground holds heat", (90., 690.), Some((200., 620.)), "#FFB08A");
}

pub fn generator(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    ground(cv, 0., 600., 1920., 232., Ground::Grass, 5);
    let (hx, hw) = (100., 640.);
    house(cv, hx, 620., hw, 300., &HouseStyle { wall: "#D9CDB8", roof: "#4D3B30", lit: false });
    // Attached garage (open) with a crossed-out generator inside.
    cv.round(hx + hw, 380., 380., 240., 0., &linear(0., 0., 1., 0., &[(0., hex("#CFC3AE")), (1., hex("#CFC3AE"))]));
    cv.round(hx + hw + 40., 440., 300., 180., 2., &solid(hex("#2A2620")));
    gen_body(cv, hx + hw + 130., 540., 120.);
    cv.line((hx + hw + 100., 450.), (hx + hw + 280., 610.), hex("#E53935"), 14.);
    cv.line((hx + hw + 280., 450.), (hx + hw + 100., 610.), hex("#E53935"), 14.);
    // Generator outside, far away, exhaust pointing away.
    gen_body(cv, 1600., 560., 180.);
    for k in 0..4 {
        cv.fill(ellipse(1800. + k as f32 * 30., 540. - k as f32 * 22., 16. + k as f32 * 8., 12. + k as f32 * 6.), &solid(rgba(160, 160, 160, 0.35 - k as f32 * 0.07)));
    }
    cv.line((hx + hw + 380., 700.), (1600., 700.), Color::WHITE, 5.);
    for x in [hx + hw + 380., 1600.] {
        cv.line((x, 680.), (x, 720.), Color::WHITE, 5.);
    }
    cv.round(1260., 676., 180., 50., 10., &solid(rgba(0, 0, 0, 0.75)));
    cv.text("20+ feet", 1350., 712., 32., Weight::Bold, hex("#FFD34D"), Al::Center, 0.);
    tag(cv, "Never in a garage or inside", (760., 270.), Some((900., 450.)), "#FF6B5A");
    tag(cv, "Outside, away from windows and doors", (1300., 380.), Some((1660., 530.)), "#5BE08A");
}

fn gen_body(cv: &mut Canvas, x: f32, gy: f32, w: f32) {
    let h = w * 0.6;
    cv.round(x - w * 0.05, gy - h - w * 0.1, w * 1.1, w * 0.08, 6., &solid(hex("#2A2A2A")));
    appliance(cv, x, gy - h, w, h, "#F2B01E");
    cv.round(x + w * 0.1, gy - h * 0.75, w * 0.45, h * 0.5, 6., &solid(hex("#2A2A2A")));
    cv.fill(ellipse(x + w * 0.78, gy - h * 0.5, w * 0.1, w * 0.1), &solid(hex("#2A2A2A")));
    for wx in [x + w * 0.15, x + w * 0.85] {
        cv.fill(ellipse(wx, gy, w * 0.1, w * 0.1), &solid(hex("#1A1A1A")));
    }
}

pub fn space_heater(cv: &mut Canvas) {
    let floor = room(cv, 0., 0., 1920., 832., "#D7C9B0", true, true);
    // Heater on the floor, plugged straight into the wall.
    let (hx, gy) = (900., floor + 120.);
    appliance(cv, hx, gy - 220., 160., 220., "#E9ECEF");
    for k in 0..6 {
        cv.round(hx + 20., gy - 190. + k as f32 * 28., 120., 14., 4., &linear(0., 0., 120., 0., &[(0., hex("#FF7A2F")), (1., hex("#C2410C"))]));
    }
    cv.stroke(smooth(&[(hx + 160., gy - 40.), (hx + 260., gy - 30.), (hx + 330., floor - 60.)], false), hex("#2A2A2A"), 6.);
    cv.round(hx + 310., floor - 110., 50., 70., 6., &solid(hex("#F4F1EA")));
    // 3-foot circle on the floor (ellipse in perspective).
    cv.dashed(ellipse(hx + 80., gy, 420., 110.), hex("#FFD34D"), 6., 24., 14.);
    // Couch and curtains kept outside the circle.
    let (sx, sy) = (120., floor + 140.);
    cv.round(sx, sy - 170., 330., 170., 30., &linear(0., sy - 170., 0., sy, &[(0., hex("#5C7A99")), (1., hex("#38506A"))]));
    cv.round(sx - 20., sy - 220., 370., 80., 30., &linear(0., sy - 220., 0., sy - 140., &[(0., hex("#6E8DAD")), (1., hex("#4A6684"))]));
    // Smoke alarm on the ceiling.
    cv.fill(ellipse(1500., 54., 50., 16.), &radial(1490., 48., 60., &[(0., Color::WHITE), (1., hex("#C9D1DA"))]));
    cv.fill(ellipse(1520., 54., 5., 5.), &solid(hex("#E53935")));
    tag(cv, "Keep 3 feet clear", (980., 300.), Some((1000., floor + 30.)), "#FFD34D");
    tag(cv, "Plug into the wall, not a power strip", (1280., 560.), Some((1230., floor - 80.)), "#FFFFFF");
    tag(cv, "Working smoke alarm", (1300., 130.), Some((1460., 64.)), "#FFFFFF");
    tag(cv, "Turn off when you leave or sleep", (100., 300.), None, "#FFFFFF");
}

/// NWS wind chill: 35.74 + 0.6215T − 35.75V^0.16 + 0.4275TV^0.16.
pub fn wind_chill(t: f32, v: f32) -> f32 {
    35.74 + 0.6215 * t - 35.75 * v.powf(0.16) + 0.4275 * t * v.powf(0.16)
}

pub fn wind_chill_chart(cv: &mut Canvas) {
    cv.round(0., 0., 1920., 832., 0., &linear(0., 0., 0., 832., &[(0., hex("#14304E")), (1., hex("#0B1A2C"))]));
    let temps: Vec<f32> = (0..18).map(|i| 40. - i as f32 * 5.).collect();
    let winds: Vec<f32> = (1..=12).map(|i| i as f32 * 5.).collect();
    let (gx, gy) = (200., 120.);
    let cw = (1920. - gx - 40.) / temps.len() as f32;
    let ch = (832. - gy - 90.) / winds.len() as f32;
    cv.text("Temperature (°F)", gx + cw * temps.len() as f32 / 2., 66., 26., Weight::Bold, hex("#FFD34D"), Al::Center, 0.);
    cv.text("Wind (mph)", 100., gy - 20., 24., Weight::Bold, hex("#FFD34D"), Al::Center, 0.);
    for (i, t) in temps.iter().enumerate() {
        cv.text(&format!("{t:.0}"), gx + cw * (i as f32 + 0.5), gy - 18., 26., Weight::Bold, Color::WHITE, Al::Center, 0.);
    }
    for (j, v) in winds.iter().enumerate() {
        let y = gy + ch * j as f32;
        cv.text(&format!("{v:.0}"), gx - 30., y + ch * 0.68, 26., Weight::Bold, Color::WHITE, Al::Right, 0.);
        for (i, t) in temps.iter().enumerate() {
            let wc = wind_chill(*t, *v);
            let c = if wc <= -18. { hex("#7A3FC0") } else if wc <= 0. { hex("#2F6FD6") } else if wc <= 20. { hex("#5FA8F0") } else { hex("#BFE0FA") };
            let x = gx + cw * i as f32;
            cv.round(x + 2., y + 2., cw - 4., ch - 4., 5., &solid(c));
            cv.text(&format!("{:.0}", wc.round()), x + cw / 2., y + ch * 0.68, (ch * 0.48).min(26.), Weight::Bold, if wc > 20. { hex("#0B1A2C") } else { Color::WHITE }, Al::Center, cw - 6.);
        }
    }
    let ly = 800.;
    for (i, (c, label)) in [(hex("#5FA8F0"), "1 to 20°F"), (hex("#2F6FD6"), "-17 to 0°F"), (hex("#7A3FC0"), "-18°F or colder: frostbite in 30 minutes or less")].iter().enumerate() {
        let x = 200. + i as f32 * 330.;
        cv.round(x, ly - 24., 40., 28., 5., &solid(*c));
        cv.text(label, x + 52., ly, 24., Weight::SemiBold, Color::WHITE, Al::Left, 0.);
    }
}

/// NWS heat index (Rothfusz regression with the NWS adjustments).
pub fn heat_index(t: f32, rh: f32) -> f32 {
    let simple = 0.5 * (t + 61. + (t - 68.) * 1.2 + rh * 0.094);
    if (simple + t) / 2. < 80. {
        return simple;
    }
    let mut hi = -42.379 + 2.04901523 * t + 10.14333127 * rh - 0.22475541 * t * rh - 0.00683783 * t * t - 0.05481717 * rh * rh + 0.00122874 * t * t * rh + 0.00085282 * t * rh * rh - 0.00000199 * t * t * rh * rh;
    if rh < 13. && (80. ..=112.).contains(&t) {
        hi -= ((13. - rh) / 4.) * ((17. - (t - 95.).abs()) / 17.).sqrt();
    } else if rh > 85. && (80. ..=87.).contains(&t) {
        hi += ((rh - 85.) / 10.) * ((87. - t) / 5.);
    }
    hi
}

pub fn heat_index_chart(cv: &mut Canvas) {
    cv.round(0., 0., 1920., 832., 0., &linear(0., 0., 0., 832., &[(0., hex("#3A1A12")), (1., hex("#1A0B08"))]));
    let temps: Vec<f32> = (0..17).map(|i| 80. + i as f32 * 2.).collect();
    let rhs: Vec<f32> = (0..13).map(|i| 40. + i as f32 * 5.).collect();
    let (gx, gy) = (200., 120.);
    let cw = (1920. - gx - 40.) / temps.len() as f32;
    let ch = (832. - gy - 90.) / rhs.len() as f32;
    cv.text("Temperature (°F)", gx + cw * temps.len() as f32 / 2., 66., 26., Weight::Bold, hex("#FFD34D"), Al::Center, 0.);
    cv.text("Humidity (%)", 100., gy - 20., 24., Weight::Bold, hex("#FFD34D"), Al::Center, 0.);
    for (i, t) in temps.iter().enumerate() {
        cv.text(&format!("{t:.0}"), gx + cw * (i as f32 + 0.5), gy - 18., 26., Weight::Bold, Color::WHITE, Al::Center, 0.);
    }
    for (j, rh) in rhs.iter().enumerate() {
        let y = gy + ch * j as f32;
        cv.text(&format!("{rh:.0}"), gx - 30., y + ch * 0.68, 26., Weight::Bold, Color::WHITE, Al::Right, 0.);
        for (i, t) in temps.iter().enumerate() {
            let hi = heat_index(*t, *rh);
            let x = gx + cw * i as f32;
            if hi > 137. {
                cv.round(x + 2., y + 2., cw - 4., ch - 4., 5., &solid(rgba(255, 255, 255, 0.04)));
                continue;
            }
            let c = if hi >= 125. { hex("#B3122E") } else if hi >= 103. { hex("#F2542D") } else if hi >= 90. { hex("#FFB13B") } else { hex("#FFE38A") };
            cv.round(x + 2., y + 2., cw - 4., ch - 4., 5., &solid(c));
            cv.text(&format!("{:.0}", hi.round()), x + cw / 2., y + ch * 0.68, (ch * 0.48).min(26.), Weight::Bold, if hi >= 103. { Color::WHITE } else { hex("#2A1206") }, Al::Center, cw - 6.);
        }
    }
    let ly = 800.;
    for (i, (c, label)) in [(hex("#FFE38A"), "Caution 80–89"), (hex("#FFB13B"), "Extreme caution 90–102"), (hex("#F2542D"), "Danger 103–124"), (hex("#B3122E"), "Extreme danger 125+")].iter().enumerate() {
        let x = 200. + i as f32 * 420.;
        cv.round(x, ly - 24., 40., 28., 5., &solid(*c));
        cv.text(label, x + 52., ly, 24., Weight::SemiBold, Color::WHITE, Al::Left, 0.);
    }
}

pub fn frost_vs_freeze(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Night);
    // A low spot in a field: cold air pools there and frost forms.
    cv.fill(poly(&[(0., 480.), (600., 560.), (1000., 640.), (1400., 560.), (1920., 470.), (1920., 832.), (0., 832.)], true), &linear(0., 480., 0., 832., &[(0., hex("#2E4A2A")), (1., hex("#15240F"))]));
    cv.fill(smooth(&[(700., 610.), (1000., 650.), (1300., 610.), (1250., 700.), (1000., 730.), (750., 700.)], true), &solid(rgba(220, 235, 250, 0.55)));
    let mut r = Rng::new(5);
    for _ in 0..240 {
        let (x, y) = (720. + r.f() * 560., 620. + r.f() * 100.);
        cv.line((x, y), (x + r.range(-3., 3.), y - r.range(6., 16.)), rgba(240, 250, 255, 0.85), 2.);
    }
    for k in 0..3 {
        flow(cv, &[(200. + k as f32 * 60., 420. + k as f32 * 30.), (500., 520.), (800., 600.)], rgba(120, 190, 255, 0.8), 6.);
    }
    // Thermometers: air at 5 feet vs the grass.
    thermometer(cv, 1560., 220., 300., 0.62, "#FF5A4A");
    cv.text("Air at 5 ft: 36°F", 1600., 600., 30., Weight::Bold, Color::WHITE, Al::Left, 0.);
    thermometer(cv, 980., 430., 160., 0.45, "#4AA3FF");
    cv.text("Grass: 32°F", 980., 410., 30., Weight::Bold, Color::WHITE, Al::Center, 0.);
    tag(cv, "Cold air settles in low spots", (120., 300.), Some((420., 470.)), "#9FD2FF");
    tag(cv, "Frost on the grass", (700., 760.), Some((900., 690.)), "#FFFFFF");
}

pub fn radiation_fog(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Night);
    cv.fill(ellipse(1580., 140., 46., 46.), &radial(1570., 130., 60., &[(0., hex("#FFFFF0")), (1., hex("#D9D6C8"))]));
    hills(cv, 0., 330., 1920., 200., "#22351E", 3);
    cv.fill(poly(&[(0., 520.), (500., 640.), (960., 700.), (1420., 640.), (1920., 520.), (1920., 832.), (0., 832.)], true), &linear(0., 520., 0., 832., &[(0., hex("#1E3018")), (1., hex("#0E180A"))]));
    // Fog filling the valley.
    // Soft fog banks following the valley floor.
    let mut r = Rng::new(12);
    for _ in 0..70 {
        let fx = r.range(120., 1800.);
        let floor = 520. + (1. - ((fx - 960.) / 960.).abs()).powf(0.8) * 170.;
        let fy = floor + r.range(-60., 40.);
        let (rx, ry) = (r.range(160., 300.), r.range(36., 70.));
        cv.fill(ellipse(fx, fy, rx, ry), &radial(fx, fy, rx, &[(0., rgba(228, 234, 242, 0.30)), (0.6, rgba(228, 234, 242, 0.14)), (1., rgba(228, 234, 242, 0.))]));
    }
    for x in [500., 1300.] {
        house(cv, x, 690., 150., 90., &HouseStyle { wall: "#BDB4A6", roof: "#3F332A", lit: true });
    }
    for k in 0..4 {
        cv.arrow((300. + k as f32 * 420., 420.), (300. + k as f32 * 420., 300.), rgba(255, 150, 120, 0.8), 6.);
    }
    tag(cv, "Clear sky: the ground loses heat", (240., 200.), Some((340., 320.)), "#FFB08A");
    tag(cv, "Fog forms in valleys", (760., 780.), Some((960., 640.)), "#FFFFFF");
    tag(cv, "Light wind", (1500., 260.), None, "#FFFFFF");
}

pub fn advection_fog(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    ground(cv, 0., 560., 1920., 272., Ground::Snow, 8);
    // Warm humid air arriving from the left over the cold snow cover.
    for k in 0..4 {
        let y = 260. + k as f32 * 70.;
        cv.arrow((60., y), (520., y + 20.), hex("#FF8A4A"), 10.);
    }
    for k in 0..6 {
        let y = 420. + k as f32 * 24.;
        cv.fill(smooth(&[(560., y + 30.), (1000., y - 10.), (1500., y - 20.), (1960., y), (1960., y + 120.), (560., y + 120.)], true), &solid(rgba(225, 230, 236, 0.28)));
    }
    tree(cv, 1300., 600., 260., Tree::Bare, 0., 2);
    tree(cv, 1600., 600., 220., Tree::Pine, 0., 3);
    tag(cv, "Warm, moist air moves in", (60., 160.), Some((300., 270.)), "#FFB08A");
    tag(cv, "Cooled from below by snow", (700., 700.), Some((900., 600.)), "#9FD2FF");
}

pub fn fog_driving(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    // Road in perspective.
    cv.fill(poly(&[(860., 300.), (1060., 300.), (1700., 832.), (220., 832.)], true), &linear(0., 300., 0., 832., &[(0., hex("#5A5D62")), (1., hex("#2C2E31"))]));
    for k in 0..6 {
        let t = k as f32 / 6.;
        let y = 320. + t * t * 500.;
        let w = 6. + t * 18.;
        cv.round(960. - w / 2., y, w, 20. + t * 50., 3., &solid(hex("#F2D04B")));
    }
    // Car from behind with low beams.
    let (cx, cy) = (960., 700.);
    cv.fill(poly(&[(cx - 160., cy - 60.), (cx - 260., 330.), (cx + 260., 330.), (cx + 160., cy - 60.)], true), &linear(0., cy, 0., 330., &[(0., rgba(255, 245, 200, 0.35)), (1., rgba(255, 245, 200, 0.))]));
    cv.round(cx - 200., cy - 80., 400., 130., 30., &linear(0., cy - 80., 0., cy + 50., &[(0., hex("#4A5A6C")), (1., hex("#202A36"))]));
    cv.round(cx - 140., cy - 150., 280., 80., 24., &linear(0., cy - 150., 0., cy - 70., &[(0., hex("#5A6A7C")), (1., hex("#2A3644"))]));
    for x in [cx - 170., cx + 130.] {
        cv.round(x, cy - 50., 40., 22., 6., &solid(hex("#E53935")));
    }
    // Fog overlay, thicker with distance.
    cv.round(0., 0., 1920., 832., 0., &linear(0., 0., 0., 832., &[(0., rgba(220, 225, 230, 0.92)), (0.55, rgba(220, 225, 230, 0.55)), (1., rgba(220, 225, 230, 0.1))]));
    tag(cv, "Low beams: high beams reflect off the fog", (1140., 470.), Some((1100., 560.)), "#FFD34D");
    tag(cv, "Slow down and leave extra room", (120., 620.), None, "#FFFFFF");
}

pub fn dew_point(cv: &mut Canvas) {
    cv.round(0., 0., 1920., 832., 0., &linear(0., 0., 0., 832., &[(0., hex("#1E3F5C")), (1., hex("#0E1E2E"))]));
    // Glass of ice water with condensation on a table.
    cv.round(0., 620., 900., 212., 0., &linear(0., 620., 0., 832., &[(0., hex("#7A5430")), (1., hex("#4A321C"))]));
    let (gx, gy) = (320., 260.);
    cv.fill(poly(&[(gx, gy), (gx + 260., gy), (gx + 230., 640.), (gx + 30., 640.)], true), &linear(gx, 0., gx + 260., 0., &[(0., rgba(220, 240, 255, 0.35)), (0.5, rgba(255, 255, 255, 0.15)), (1., rgba(200, 225, 245, 0.4))]));
    water(cv, gx + 14., 330., 232., 300., false, 3);
    for (k, (ix, iy)) in [(gx + 60., 360.), (gx + 140., 380.), (gx + 100., 450.)].iter().enumerate() {
        cv.round(*ix, *iy, 70., 60., 10., &linear(*ix, *iy, *ix + 70., *iy + 60., &[(0., rgba(255, 255, 255, 0.9)), (1., rgba(180, 220, 245, 0.7))]));
        let _ = k;
    }
    let mut r = Rng::new(6);
    for _ in 0..60 {
        let (dx, dy) = (gx + 20. + r.f() * 220., gy + 40. + r.f() * 330.);
        let s = r.range(3., 7.);
        cv.fill(ellipse(dx, dy, s * 0.8, s), &radial(dx - 1., dy - 1., s, &[(0., rgba(255, 255, 255, 0.95)), (1., rgba(150, 200, 235, 0.4))]));
    }
    // Comfort scale.
    let rows = [("Under 55°F", "Dry and comfortable", "#5FD38A"), ("55 to 64°F", "Getting sticky", "#F2D04B"), ("65 to 69°F", "Muggy", "#FF9A2E"), ("70°F and up", "Oppressive", "#E53935")];
    for (i, (dp, feel, c)) in rows.iter().enumerate() {
        let y = 140. + i as f32 * 150.;
        cv.round(1000., y, 840., 120., 18., &solid(rgba(255, 255, 255, 0.07)));
        cv.round(1000., y, 24., 120., 8., &solid(hex(c)));
        cv.text(dp, 1060., y + 76., 44., Weight::Bold, hex(c), Al::Left, 0.);
        cv.text(feel, 1820., y + 76., 40., Weight::SemiBold, Color::WHITE, Al::Right, 0.);
    }
    cv.text("Dew point", 1420., 100., 36., Weight::Bold, hex("#FFD34D"), Al::Center, 0.);
    tag(cv, "Air cools to its dew point on the glass", (60., 120.), Some((360., 300.)), "#FFFFFF");
}

pub fn uv_index(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Day);
    sun(cv, 300., 200., 110.);
    ground(cv, 0., 620., 1920., 212., Ground::Dry, 4);
    person(cv, 600., 660., 300., Pose::Stand, "#F1C40F");
    // Hat brim and sunglasses on the person.
    cv.fill(ellipse(600., 660. - 300. + 14., 58., 12.), &solid(hex("#E9D8A6")));
    cv.round(582., 660. - 300. + 22., 40., 10., 4., &solid(hex("#111111")));
    let scale = [("0–2", "Low", "#3EA72D"), ("3–5", "Moderate", "#FFF300"), ("6–7", "High", "#F18B00"), ("8–10", "Very high", "#E53210"), ("11+", "Extreme", "#B567A4")];
    for (i, (n, name, c)) in scale.iter().enumerate() {
        let x = 820. + i as f32 * 210.;
        cv.round(x, 220., 196., 300., 20., &linear(0., 220., 0., 520., &[(0., mix(hex(c), Color::WHITE, 0.15)), (1., mix(hex(c), Color::BLACK, 0.15))]));
        let dark = i == 1;
        cv.text(n, x + 98., 360., 56., Weight::Bold, if dark { hex("#222222") } else { Color::WHITE }, Al::Center, 180.);
        cv.text(name, x + 98., 440., 30., Weight::Bold, if dark { hex("#222222") } else { Color::WHITE }, Al::Center, 180.);
    }
    tag(cv, "Hat, sunglasses, SPF 15+ sunscreen", (430., 720.), Some((600., 660.)), "#FFD34D");
    tag(cv, "Strongest 10 a.m. to 4 p.m.", (820., 536.), None, "#FFFFFF");
}

pub fn hot_pavement(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Hazy);
    sun(cv, 1700., 120., 80.);
    road(cv, 0., 520., 1920., 312., false);
    // Heat shimmer.
    for k in 0..6 {
        let y = 500. - k as f32 * 16.;
        cv.stroke(smooth(&[(0., y), (300., y - 6.), (600., y + 4.), (900., y - 4.), (1200., y + 6.), (1500., y - 6.), (1920., y)], false), rgba(255, 255, 255, 0.12), 4.);
    }
    // Dog.
    let (dx, dy) = (700., 640.);
    cv.fill(ellipse(dx, dy - 90., 130., 60.), &radial(dx - 40., dy - 120., 160., &[(0., hex("#D9A066")), (1., hex("#8A5A2E"))]));
    cv.fill(ellipse(dx + 130., dy - 150., 52., 44.), &radial(dx + 120., dy - 165., 70., &[(0., hex("#D9A066")), (1., hex("#8A5A2E"))]));
    cv.fill(ellipse(dx + 170., dy - 140., 30., 18.), &solid(hex("#B97F45")));
    cv.fill(ellipse(dx + 196., dy - 146., 9., 7.), &solid(hex("#1A1A1A")));
    cv.fill(ellipse(dx + 112., dy - 175., 18., 30.), &solid(hex("#6E4523")));
    for lx in [dx - 90., dx - 50., dx + 50., dx + 90.] {
        cv.line((lx, dy - 60.), (lx, dy), hex("#8A5A2E"), 20.);
    }
    cv.stroke(smooth(&[(dx - 130., dy - 100.), (dx - 180., dy - 150.), (dx - 200., dy - 190.)], false), hex("#8A5A2E"), 14.);
    // Hand test.
    let (hx, hy) = (1200., 600.);
    cv.fill(smooth(&[(hx, hy), (hx + 160., hy - 10.), (hx + 190., hy + 10.), (hx + 160., hy + 26.), (hx, hy + 30.)], true), &linear(0., hy, 0., hy + 30., &[(0., hex("#E2A882")), (1., hex("#B07A58"))]));
    cv.round(hx - 200., hy - 6., 210., 40., 18., &solid(hex("#2E6DB4")));
    tag(cv, "Back of your hand on the pavement for 7 seconds", (1000., 420.), Some((1300., 595.)), "#FFD34D");
    tag(cv, "Too hot for you = too hot for paws", (380., 380.), Some((700., 520.)), "#FFFFFF");
}

pub fn pet_cold(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Winter);
    ground(cv, 0., 600., 1920., 232., Ground::Snow, 3);
    snow(cv, 0., 0., 1920., 600., 140, 4);
    house(cv, 1200., 620., 520., 300., &HouseStyle { wall: "#E2D6C2", roof: "#4D3B30", lit: true });
    // Dog going inside through the door.
    let (dx, dy) = (1050., 640.);
    cv.fill(ellipse(dx, dy - 70., 100., 46.), &radial(dx - 30., dy - 95., 120., &[(0., hex("#3A3A3A")), (1., hex("#111111"))]));
    cv.fill(ellipse(dx + 100., dy - 120., 40., 34.), &radial(dx + 90., dy - 130., 54., &[(0., hex("#3A3A3A")), (1., hex("#111111"))]));
    cv.fill(ellipse(dx + 132., dy - 112., 22., 13.), &solid(hex("#2A2A2A")));
    cv.round(dx + 60., dy - 104., 50., 12., 4., &solid(hex("#C62828")));
    for lx in [dx - 70., dx - 40., dx + 40., dx + 70.] {
        cv.line((lx, dy - 40.), (lx, dy), hex("#1A1A1A"), 15.);
    }
    cv.arrow((1170., 560.), (1300., 560.), hex("#5BE08A"), 10.);
    // Water bowl (not frozen).
    cv.fill(ellipse(400., 640., 80., 22.), &solid(hex("#2E6DB4")));
    cv.fill(ellipse(400., 632., 64., 14.), &radial(390., 628., 70., &[(0., hex("#9FD2FF")), (1., hex("#4A8FD0"))]));
    tag(cv, "Bring pets inside", (1000., 380.), Some((1100., 520.)), "#FFD34D");
    tag(cv, "Fresh water that isn't frozen", (180., 470.), Some((400., 620.)), "#FFFFFF");
    tag(cv, "Wipe paws: salt and ice melt", (500., 720.), None, "#FFFFFF");
}

pub fn shoveling(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Winter);
    ground(cv, 0., 600., 1920., 232., Ground::Snow, 2);
    house(cv, 1300., 620., 480., 280., &HouseStyle { wall: "#C9D6E0", roof: "#3F332A", lit: true });
    // Cleared path.
    cv.fill(poly(&[(560., 600.), (900., 600.), (1000., 832.), (480., 832.)], true), &linear(0., 600., 0., 832., &[(0., hex("#6E7378")), (1., hex("#3E4246"))]));
    person(cv, 640., 760., 300., Pose::Shovel, "#C0392B");
    cv.fill(smooth(&[(300., 620.), (460., 560.), (560., 620.), (520., 700.), (320., 700.)], true), &radial(420., 600., 150., &[(0., Color::WHITE), (1., hex("#C9D7E6"))]));
    cv.arrow((760., 800.), (980., 800.), hex("#FFD34D"), 10.);
    tag(cv, "Push snow instead of lifting it", (900., 700.), Some((880., 790.)), "#FFD34D");
    tag(cv, "Take breaks, stay warm and dry", (60., 300.), None, "#FFFFFF");
    tag(cv, "Lift with your legs", (720., 260.), Some((680., 480.)), "#FFFFFF");
}

pub fn snow_ratio(cv: &mut Canvas) {
    cv.round(0., 0., 1920., 832., 0., &linear(0., 0., 0., 832., &[(0., hex("#1F3A57")), (1., hex("#0E1D2D"))]));
    // Two tall clear cylinders on a table: 10 in of snow → about 1 in of water.
    cv.round(0., 680., 1920., 152., 0., &linear(0., 680., 0., 832., &[(0., hex("#6E5236")), (1., hex("#3D2B1A"))]));
    let px_in = 52.;
    for (x, label, fill, snowy) in [(560., "10 inches of snow", 10., true), (1160., "about 1 inch of water", 1., false)] {
        let (top, w) = (680. - 11. * px_in, 200.);
        if snowy {
            cv.round(x, 680. - fill * px_in, w, fill * px_in, 8., &radial(x + 60., 680. - fill * px_in, 400., &[(0., Color::WHITE), (1., hex("#C9D7E6"))]));
            snow(cv, x + 10., 680. - fill * px_in, w - 20., fill * px_in, 120, 3);
        } else {
            water(cv, x, 680. - fill * px_in, w, fill * px_in, false, 2);
        }
        cv.round(x, top, w, 11. * px_in, 12., &linear(x, 0., x + w, 0., &[(0., rgba(255, 255, 255, 0.28)), (0.3, rgba(255, 255, 255, 0.06)), (1., rgba(255, 255, 255, 0.22))]));
        cv.stroke(rrect(x, top, w, 11. * px_in, 12.), rgba(255, 255, 255, 0.6), 3.);
        for i in 0..=10 {
            let ty = 680. - i as f32 * px_in;
            cv.line((x + w, ty), (x + w + 20., ty), Color::WHITE, 2.);
            if i % 2 == 0 {
                cv.text(&i.to_string(), x + w + 30., ty + 8., 22., Weight::SemiBold, Color::WHITE, Al::Left, 0.);
            }
        }
        cv.text(label, x + w / 2., 770., 32., Weight::Bold, hex("#FFD34D"), Al::Center, 420.);
    }
    cv.arrow((820., 400.), (1100., 400.), Color::WHITE, 10.);
    tag(cv, "Wet snow: less. Fluffy snow: much more.", (120., 120.), None, "#FFFFFF");
}

pub fn fridge_outage(cv: &mut Canvas) {
    let floor = room(cv, 0., 0., 1920., 832., "#DCE3E8", false, false);
    // Fridge with closed doors.
    let (fx, fw) = (700., 420.);
    let top = 110.;
    appliance(cv, fx, top, fw, floor + 40. - top, "#E3E7EB");
    cv.line((fx, top + 230.), (fx + fw, top + 230.), rgba(0, 0, 0, 0.3), 4.);
    for (y0, h) in [(top + 40., 140.), (top + 270., 300.)] {
        cv.round(fx + fw - 40., y0, 16., h, 6., &linear(0., 0., 16., 0., &[(0., hex("#C9D1DA")), (1., hex("#7D848E"))]));
    }
    // Lights off (outage): dark tint.
    cv.round(0., 0., 1920., 832., 0., &solid(rgba(10, 16, 30, 0.35)));
    cv.round(fx - 280., top + 60., 240., 110., 16., &solid(rgba(0, 0, 0, 0.75)));
    cv.text("Freezer", fx - 160., top + 100., 28., Weight::SemiBold, Color::WHITE, Al::Center, 0.);
    cv.text("48 hours full", fx - 160., top + 146., 32., Weight::Bold, hex("#9FD2FF"), Al::Center, 220.);
    cv.round(fx - 280., top + 380., 240., 110., 16., &solid(rgba(0, 0, 0, 0.75)));
    cv.text("Refrigerator", fx - 160., top + 420., 28., Weight::SemiBold, Color::WHITE, Al::Center, 0.);
    cv.text("About 4 hours", fx - 160., top + 466., 32., Weight::Bold, hex("#FFD34D"), Al::Center, 220.);
    tag(cv, "Keep the doors closed", (fx + fw + 60., 300.), Some((fx + fw - 30., 330.)), "#FFD34D");
    tag(cv, "Half-full freezer: about 24 hours", (fx + fw + 60., 520.), None, "#FFFFFF");
}

pub fn snow_squall(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    ground(cv, 0., 480., 1920., 352., Ground::Snow, 3);
    cv.fill(poly(&[(820., 480.), (1100., 480.), (1800., 832.), (120., 832.)], true), &linear(0., 480., 0., 832., &[(0., hex("#8A9098")), (1., hex("#4A4E54"))]));
    car(cv, 760., 760., 380., "#B03A2E", Car::Sedan);
    // Wall of snow ahead.
    cv.round(0., 0., 1920., 560., 0., &linear(0., 0., 0., 560., &[(0., rgba(240, 244, 248, 0.15)), (0.6, rgba(240, 244, 248, 0.85)), (1., rgba(240, 244, 248, 0.5))]));
    snow(cv, 0., 0., 1920., 832., 420, 9);
    tag(cv, "Sudden whiteout, slick roads", (1100., 160.), None, "#FFFFFF");
    tag(cv, "Slow down, lights on, no sudden braking", (60., 600.), Some((760., 700.)), "#FFD34D");
}

pub fn ice_accum(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Winter);
    ground(cv, 0., 640., 1920., 192., Ground::Ice, 2);
    for (i, &(x, inch)) in [(320f32, 0.25f32), (960., 0.5), (1600., 1.0)].iter().enumerate() {
        // Power pole with lines, coated by ice thickness.
        cv.round(x - 12., 140., 24., 500., 4., &linear(x - 12., 0., x + 12., 0., &[(0., hex("#7A5A3A")), (1., hex("#4A321C"))]));
        cv.round(x - 120., 170., 240., 16., 3., &solid(hex("#5A4028")));
        let coat = 6. + inch * 40.;
        for k in 0..3 {
            let y = 200. + k as f32 * 24.;
            cv.stroke(smooth(&[(x - 300., y + 30.), (x, y), (x + 300., y + 30.)], false), rgba(210, 235, 255, 0.6), coat);
            cv.stroke(smooth(&[(x - 300., y + 30.), (x, y), (x + 300., y + 30.)], false), hex("#222222"), 3.);
        }
        if inch >= 0.5 {
            tree(cv, x - 180., 650., 260., Tree::Bare, 0.0, i as u64);
            cv.line((x + 120., 640.), (x + 260., 580.), hex("#4A3423"), 12.);
        }
        let label = match i {
            0 => "1/4 inch",
            1 => "1/2 inch",
            _ => "1 inch",
        };
        tag_at(cv, label, x - 100., 700., "#9FD2FF", 40.);
    }
    divider(cv, 640.);
    divider(cv, 1280.);
    tag(cv, "Some limbs and lines can break", (180., 470.), None, "#FFFFFF");
    tag(cv, "Widespread outages possible", (800., 470.), None, "#FFD34D");
    tag(cv, "Major damage", (1500., 470.), None, "#FF6B5A");
}

pub fn cloud_types(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Day);
    ground(cv, 0., 740., 1920., 92., Ground::Grass, 2);
    // Height bands.
    for (y, label) in [(120., "High: above 20,000 ft"), (360., "Middle: 6,500–20,000 ft"), (600., "Low: below 6,500 ft")] {
        cv.dashed(poly(&[(0., y + 110.), (1920., y + 110.)], false), rgba(255, 255, 255, 0.4), 2., 12., 10.);
        cv.text(label, 30., y + 100., 24., Weight::SemiBold, Color::WHITE, Al::Left, 0.);
    }
    // Cirrus wisps.
    // Cirrus: fine streaks with hooked "mare's tail" ends, many thin strands per streak.
    let mut r = Rng::new(21);
    for k in 0..5 {
        let (x, y) = (360. + k as f32 * 150., 90. + r.range(-20., 30.));
        for s in 0..9 {
            let off = s as f32 * 3.5;
            let pts = [(x + off, y + off * 0.6), (x + 120. + off, y - 10. + off * 0.4), (x + 230., y + 6. + off * 0.5), (x + 270., y + 34. + off * 0.3), (x + 262., y + 58.)];
            cv.stroke(smooth(&pts, false), rgba(255, 255, 255, 0.10 + (s % 3) as f32 * 0.07), 2. + (s % 2) as f32 * 1.5);
        }
    }
    // Altostratus sheet.
    cv.round(380., 390., 700., 70., 35., &linear(0., 390., 0., 460., &[(0., rgba(230, 235, 240, 0.9)), (1., rgba(170, 180, 190, 0.9))]));
    // Cumulus.
    cloud(cv, 640., 660., 1.0, 0.0, 3);
    // Cumulonimbus spanning all levels.
    cumulonimbus(cv, 1500., 700., 1.12, true);
    cv.text("Cirrus", 700., 200., 34., Weight::Bold, hex("#FFD34D"), Al::Center, 0.);
    cv.text("Altostratus", 730., 510., 34., Weight::Bold, hex("#FFD34D"), Al::Center, 0.);
    cv.text("Cumulus", 800., 690., 34., Weight::Bold, hex("#FFD34D"), Al::Left, 0.);
    tag(cv, "Cumulonimbus: thunderstorm", (1200., 40.), Some((1500., 200.)), "#FFD34D");
}

pub fn rainbow(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    cv.round(0., 0., 820., 832., 0., &linear(0., 0., 820., 0., &[(0., rgba(255, 230, 170, 0.35)), (1., rgba(255, 230, 170, 0.))]));
    ground(cv, 0., 640., 1920., 192., Ground::Grass, 4);
    rain(cv, 1100., 100., 800., 540., 260, -0.05, 6);
    let (cx, cy) = (1450., 700.);
    let colors = ["#E53935", "#FB8C00", "#FDD835", "#43A047", "#1E88E5", "#3949AB", "#8E24AA"];
    for (i, c) in colors.iter().enumerate() {
        let r = 520. - i as f32 * 16.;
        let pts: Vec<(f32, f32)> = (0..=40).map(|k| {
            let a = std::f32::consts::PI + std::f32::consts::PI * k as f32 / 40.;
            (cx + a.cos() * r, cy + a.sin() * r)
        }).collect();
        cv.stroke(smooth(&pts, false), mix(hex(c), Color::WHITE, 0.15), 16.);
    }
    sun(cv, 120., 160., 60.);
    person(cv, 640., 690., 220., Pose::Stand, "#2E6DB4");
    cv.dashed(poly(&[(160., 190.), (640., 560.)], false), rgba(255, 240, 180, 0.9), 4., 16., 10.);
    tag(cv, "Sun behind you", (60., 300.), Some((160., 200.)), "#FFD34D");
    tag(cv, "Rain in front of you", (1500., 760.), Some((1600., 600.)), "#9FD2FF");
}

pub fn hydroplaning(cv: &mut Canvas) {
    cv.round(0., 0., 1920., 832., 0., &linear(0., 0., 0., 832., &[(0., hex("#1E2A36")), (1., hex("#0E151C"))]));
    // Close-up tyre riding up on a wedge of water.
    let (cx, cy, r) = (900., 420., 300.);
    cv.round(0., 690., 1920., 142., 0., &linear(0., 690., 0., 832., &[(0., hex("#3A3D42")), (1., hex("#1E2023"))]));
    cv.fill(poly(&[(200., 690.), (1300., 690.), (1100., 640.), (700., 700.)], true), &solid(rgba(90, 150, 210, 0.75)));
    water(cv, 0., 676., 1920., 30., false, 4);
    cv.fill(ellipse(cx, cy, r, r), &radial(cx - 80., cy - 80., r * 1.4, &[(0., hex("#3A3A3E")), (1., hex("#0E0E10"))]));
    for k in 0..24 {
        let a = k as f32 * std::f32::consts::TAU / 24.;
        cv.line((cx + a.cos() * r * 0.86, cy + a.sin() * r * 0.86), (cx + a.cos() * r * 0.99, cy + a.sin() * r * 0.99), hex("#222226"), 10.);
    }
    cv.fill(ellipse(cx, cy, r * 0.55, r * 0.55), &radial(cx - 30., cy - 30., r * 0.7, &[(0., hex("#E6E9EE")), (1., hex("#7D848E"))]));
    for k in 0..5 {
        let a = k as f32 * std::f32::consts::TAU / 5.;
        cv.fill(ellipse(cx + a.cos() * r * 0.3, cy + a.sin() * r * 0.3, 20., 20.), &solid(hex("#5A6168")));
    }
    // Spray.
    for k in 0..8 {
        cv.fill(ellipse(1260. + k as f32 * 30., 640. - k as f32 * 20., 14. + k as f32 * 3., 6.), &solid(rgba(180, 210, 240, 0.5)));
    }
    cv.arrow((300., 300.), (560., 300.), Color::WHITE, 10.);
    tag(cv, "Tyre rides on water, not road", (1220., 470.), Some((1100., 660.)), "#9FD2FF");
    tag(cv, "Slow down; no cruise control in rain", (120., 160.), None, "#FFD34D");
}

pub fn storm_drain(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Overcast);
    rain(cv, 0., 0., 1920., 600., 300, -0.05, 2);
    // Street, curb and drain clogged with leaves.
    cv.round(0., 560., 1920., 272., 0., &linear(0., 560., 0., 832., &[(0., hex("#4A4D52")), (1., hex("#2C2E31"))]));
    cv.round(0., 520., 1920., 40., 0., &solid(hex("#B8B3A8")));
    house(cv, 100., 520., 400., 240., &HouseStyle { wall: "#D8CFC0", roof: "#5B4A3E", lit: false });
    let (dx, dy) = (1100., 560.);
    cv.round(dx, dy - 40., 260., 40., 4., &solid(hex("#1A1A1A")));
    for k in 0..7 {
        cv.line((dx + 20. + k as f32 * 34., dy - 36.), (dx + 20. + k as f32 * 34., dy - 4.), hex("#55595E"), 6.);
    }
    let mut r = Rng::new(8);
    for _ in 0..40 {
        let (lx, ly) = (dx - 40. + r.f() * 340., dy - 20. + r.f() * 40.);
        cv.fill(ellipse(lx, ly, r.range(10., 18.), r.range(5., 9.)), &solid([hex("#C4561E"), hex("#E0A12E"), hex("#8A3A12")][(r.f() * 2.99) as usize]));
    }
    water(cv, 0., 600., 1920., 232., true, 5);
    tag(cv, "Clear leaves from storm drains", (1060., 360.), Some((1200., 540.)), "#FFD34D");
    tag(cv, "Blocked drains flood streets", (200., 720.), None, "#FFFFFF");
}

pub fn layering(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Winter);
    ground(cv, 0., 700., 1920., 132., Ground::Snow, 6);
    snow(cv, 0., 0., 1920., 700., 120, 3);
    // Three figures showing layers added left to right.
    for (i, (coat, label)) in [("#7FB3D5", "1. Base layer"), ("#C0392B", "2. Warm middle layer"), ("#2C3E50", "3. Wind and water shell")].iter().enumerate() {
        let x = 360. + i as f32 * 600.;
        person(cv, x, 740., 460., Pose::Stand, coat);
        if i == 2 {
            cv.fill(ellipse(x, 740. - 460. + 20., 40., 24.), &solid(hex("#C0392B")));
            cv.round(x - 46., 740. - 460. + 70., 92., 24., 10., &solid(hex("#F1C40F")));
        }
        tag_at(cv, label, x - 180., 40., "#FFFFFF", 34.);
    }
    tag(cv, "Hat, scarf and gloves", (1400., 180.), Some((1560., 330.)), "#FFD34D");
}

pub fn red_flag(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Hazy);
    ground(cv, 0., 560., 1920., 272., Ground::Dry, 7);
    // Dry grass and a smoke plume blowing sideways.
    let mut r = Rng::new(3);
    for _ in 0..600 {
        let (x, y) = (r.f() * 1920., 560. + r.f() * 270.);
        cv.line((x, y), (x + 8., y - r.range(10., 30.)), mix(hex("#E0C27A"), hex("#9C7A3A"), r.f()), 2.);
    }
    for k in 0..10 {
        cv.fill(ellipse(1400. + k as f32 * 50., 520. - k as f32 * 40., 40. + k as f32 * 14., 26. + k as f32 * 8.), &solid(rgba(120, 110, 100, 0.45 - k as f32 * 0.035)));
    }
    for k in 0..4 {
        cv.arrow((200., 200. + k as f32 * 70.), (620., 200. + k as f32 * 70.), Color::WHITE, 9.);
    }
    // Burn barrel, crossed out.
    cv.round(760., 420., 150., 180., 10., &linear(760., 0., 910., 0., &[(0., hex("#6E6E6E")), (1., hex("#2E2E2E"))]));
    cv.fill(smooth(&[(780., 420.), (830., 340.), (860., 380.), (890., 320.), (900., 420.)], true), &linear(0., 320., 0., 420., &[(0., hex("#FFD34D")), (1., hex("#E8501E"))]));
    cv.line((720., 330.), (950., 620.), hex("#E53935"), 16.);
    cv.line((950., 330.), (720., 620.), hex("#E53935"), 16.);
    tag(cv, "Strong wind + low humidity + dry grass", (120., 80.), None, "#FFD34D");
    tag(cv, "No outdoor burning", (980., 380.), Some((900., 440.)), "#FF6B5A");
}

pub fn drought_levels(cv: &mut Canvas) {
    cv.round(0., 0., 1920., 832., 0., &linear(0., 0., 0., 832., &[(0., hex("#3A2A1A")), (1., hex("#1A120A"))]));
    let levels = [("D0", "Abnormally dry", "#FFFF00"), ("D1", "Moderate", "#FCD37F"), ("D2", "Severe", "#FFAA00"), ("D3", "Extreme", "#E60000"), ("D4", "Exceptional", "#730000")];
    for (i, (d, name, c)) in levels.iter().enumerate() {
        let x = 30. + i as f32 * 378.;
        // Soil tile: cracks deepen with each level.
        // Soil seen from above: moist and whole at D0, then dried mud plates that get smaller
        // with wider, darker cracks at each level.
        let soil = mix(hex("#6E5236"), hex("#D8B988"), i as f32 / 4.);
        cv.round(x, 40., 360., 520., 20., &solid(hex("#2A1A0C")));
        let n = [1usize, 3, 4, 6, 8][i];
        let gap = [0f32, 3., 5., 7., 9.][i];
        let mut r = Rng::new(i as u64 + 1);
        let (cw, chh) = (360. / n as f32, 520. / (n as f32 * 1.4).round().max(1.));
        let rows = (520. / chh).round() as usize;
        let mut grid = vec![vec![(0f32, 0f32); n + 1]; rows + 1];
        for (j, row) in grid.iter_mut().enumerate() {
            for (k, p) in row.iter_mut().enumerate() {
                let edge_x = k == 0 || k == n;
                let edge_y = j == 0 || j == rows;
                let jx = if edge_x { 0. } else { r.range(-0.28, 0.28) * cw };
                let jy = if edge_y { 0. } else { r.range(-0.28, 0.28) * chh };
                *p = (x + k as f32 * cw + jx, 40. + j as f32 * chh + jy);
            }
        }
        for j in 0..rows {
            for k in 0..n {
                let quad = [grid[j][k], grid[j][k + 1], grid[j + 1][k + 1], grid[j + 1][k]];
                let c = (quad.iter().map(|p| p.0).sum::<f32>() / 4., quad.iter().map(|p| p.1).sum::<f32>() / 4.);
                let shrink = |p: (f32, f32)| {
                    let (dx, dy) = (c.0 - p.0, c.1 - p.1);
                    let d = (dx * dx + dy * dy).sqrt().max(0.01);
                    (p.0 + dx / d * gap, p.1 + dy / d * gap)
                };
                let pts: Vec<(f32, f32)> = quad.iter().map(|p| shrink(*p)).collect();
                let tone = mix(soil, Color::WHITE, r.range(0., 0.12));
                cv.fill(poly(&pts, true), &linear(pts[0].0, pts[0].1, pts[2].0, pts[2].1, &[(0., mix(tone, Color::WHITE, 0.12)), (1., mix(tone, Color::BLACK, 0.18))]));
                // Curled, lighter plate edges once it is really dry.
                if i >= 3 {
                    cv.stroke(poly(&pts, true), rgba(255, 240, 210, 0.25), 2.);
                }
            }
        }
        if i < 2 {
            for k in 0..(6 - i * 3) {
                cv.line((x + 60. + k as f32 * 50., 540.), (x + 70. + k as f32 * 50., 470.), hex("#6E9A3A"), 4.);
            }
        }
        cv.round(x, 590., 360., 200., 20., &solid(hex(c)));
        let dark = i <= 2;
        cv.text(d, x + 180., 680., 64., Weight::Bold, if dark { hex("#2A1A0A") } else { Color::WHITE }, Al::Center, 0.);
        cv.text(name, x + 180., 750., 32., Weight::Bold, if dark { hex("#2A1A0A") } else { Color::WHITE }, Al::Center, 330.);
    }
}

pub fn moon_halo(cv: &mut Canvas) {
    sky(cv, 0., 0., 1920., 832., Sky::Night);
    hills(cv, 0., 620., 1920., 212., "#0E1A10", 5);
    let (cx, cy) = (960., 330.);
    cv.round(0., 0., 1920., 700., 0., &solid(rgba(200, 210, 230, 0.06)));
    cv.stroke(ellipse(cx, cy, 250., 250.), rgba(230, 236, 255, 0.35), 26.);
    cv.stroke(ellipse(cx, cy, 250., 250.), rgba(255, 220, 200, 0.25), 6.);
    cv.fill(ellipse(cx, cy, 60., 60.), &radial(cx - 15., cy - 15., 75., &[(0., hex("#FFFFF6")), (1., hex("#D8D4C2"))]));
    for k in 0..5 {
        cv.stroke(smooth(&[(200., 120. + k as f32 * 40.), (800., 100. + k as f32 * 30.), (1400., 140. + k as f32 * 35.), (1800., 120. + k as f32 * 40.)], false), rgba(255, 255, 255, 0.07), 30.);
    }
    tag(cv, "Thin, high ice-crystal clouds", (1260., 120.), Some((1150., 160.)), "#FFFFFF");
    tag(cv, "Can mean moisture is moving in", (1260., 560.), None, "#FFD34D");
}
