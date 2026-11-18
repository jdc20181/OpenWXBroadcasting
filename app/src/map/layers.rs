//! Data layers and severe-weather call-outs drawn by the compositor:
//! gridded fields (temperature, wind, dew point, Futurecast…), isobars with H/L centres, city
//! value plots, animated wind particles, lightning, warning countdown tags, the clickable alert
//! "blip" call-out, and their legends.

use tiny_skia::{
    Color, FillRule, FilterQuality, Pixmap, PixmapPaint, PathBuilder, Rect, Transform,
};

use crate::{
    config::Brand,
    data::{Alert, Strike},
    gfx::{
        parse_color, parse_hex,
        render::rounded_rect,
        text::{draw_line, measure, GlyphCache, Weight},
        with_alpha,
    },
    grid::{self, PrecipKind, Var, WxGrid},
    map::{
        geo::Viewport,
        overlays::{halo_text, paint, panel_color, ring_path, stroke, style},
        scene::DataField,
    },
};

/// Shade a data field over the view. Sampled on a coarse block grid and scaled up with
/// bilinear filtering, which gives the smooth broadcast look.
pub fn render_field(pm: &mut Pixmap, vp: &Viewport, grid: &WxGrid, field: DataField, tf: f32, opacity: f32) {
    render_samples(pm, vp, |lat, lon| {
        grid.value(field, tf, lat, lon).and_then(|v| grid::field_rgba(field, v, opacity))
    });
}

pub fn render_futurecast(pm: &mut Pixmap, vp: &Viewport, frame: &crate::futurecast::ModelFrame, product: crate::futurecast::Product, opacity: f32) {
    render_samples(pm, vp, |lat, lon| match product {
        crate::futurecast::Product::Precipitation => frame.sample(lat, lon).and_then(|(rate, kind)| grid::precip_rgba(rate, kind, opacity)),
        product => {
            let value = frame.value(product, lat, lon)?;
            match product {
                crate::futurecast::Product::Temperature => grid::field_rgba(DataField::Temperature, value, opacity),
                crate::futurecast::Product::TotalPrecip => grid::field_rgba(DataField::RainTotal, value, opacity),
                crate::futurecast::Product::DewPoint => grid::field_rgba(DataField::DewPoint, value, opacity),
                crate::futurecast::Product::Humidity => grid::field_rgba(DataField::Humidity, value, opacity),
                crate::futurecast::Product::Wind => grid::field_rgba(DataField::WindSpeed, value, opacity),
                crate::futurecast::Product::Gust => grid::field_rgba(DataField::WindGust, value, opacity),
                crate::futurecast::Product::Pressure => grid::field_rgba(DataField::Pressure, value, opacity),
                // Liquid rate in the TWC radar colours, at its Marshall-Palmer reflectivity.
                crate::futurecast::Product::PrecipitationRate => crate::twc::rain(crate::twc::rain_dbz(value)).map(|[r, g, b]| [r, g, b, (opacity.clamp(0.0, 1.0) * 255.0) as u8]),
                crate::futurecast::Product::Snowfall | crate::futurecast::Product::SnowfallLow | crate::futurecast::Product::SnowfallHigh => grid::field_rgba(DataField::SnowTotal, value, opacity),
                crate::futurecast::Product::SnowfallChance | crate::futurecast::Product::IceChance => grid::field_rgba(DataField::SnowChance, value, opacity),
                crate::futurecast::Product::Ice | crate::futurecast::Product::IceLow | crate::futurecast::Product::IceHigh => grid::field_rgba(DataField::IceTotal, value, opacity),
                // Simulated radar: the TWC table at the model's reflectivity, like the live radar.
                _ => crate::twc::rain(value).map(|[r, g, b]| [r, g, b, (opacity.clamp(0.0, 1.0) * 255.0) as u8]),
            }
        }
    });
}

/// Legend stops for simulated radar: exact samples of the TWC table (LibreWXR palette 4).
const MODEL_REFLECTIVITY: &[(f32, [u8; 3])] = &[
    (10.0, [1, 183, 20]), (20.0, [17, 101, 26]), (30.0, [255, 238, 7]), (40.0, [243, 139, 8]),
    (50.0, [234, 94, 9]), (60.0, [211, 16, 12]), (70.0, [184, 12, 8]),
];

pub fn draw_model_scale_legend(pm: &mut Pixmap, glyphs: &mut GlyphCache, brand: &Brand, (dx, dy): (f32, f32), product: crate::futurecast::Product) {
    let (heading, stops, labels): (_, _, &[&str]) = if product == crate::futurecast::Product::PrecipitationRate {
        ("PRECIPITATION RATE (LIQUID EQUIV., IN/HR)", grid::rain_ramp(), &[".006", ".02", ".05", ".1", ".25", ".5", "1+"])
    } else {
        ("SIMULATED REFLECTIVITY (dBZ)", MODEL_REFLECTIVITY, &["10", "20", "30", "40", "50", "60", "70"])
    };
    let (x, y) = (40.0 + dx, pm.height() as f32 - 136.0 + dy);
    if let Some(path) = rounded_rect(x, y, 620.0, 96.0, 12.0) {
        pm.fill_path(&path, &paint(panel_color(brand, 0.82)), FillRule::Winding, Transform::identity(), None);
    }
    let title = style(&brand.font_display, Weight::Bold, 22.0, parse_color("$text", brand));
    let small = style(&brand.font_body, Weight::SemiBold, 16.0, parse_color("$text", brand));
    draw_line(pm, glyphs, heading, x + 18.0, y + 34.0, &title, 1.0, None);
    let (left, width) = (x + 18.0, 584.0);
    draw_gradient_even(pm, stops, (left, y + 44.0, width, 18.0));
    let last = (stops.len() - 1).max(1) as f32;
    for (i, label) in labels.iter().enumerate().take(stops.len()) {
        let tick = left + i as f32 / last * width;
        let text_width = measure(label, &small);
        draw_line(pm, glyphs, label, (tick - text_width / 2.0).clamp(left, left + width - text_width), y + 86.0, &small, 1.0, None);
    }
}

fn render_samples(pm: &mut Pixmap, vp: &Viewport, sample: impl Fn(f64, f64) -> Option<[u8; 4]>) {
    const BLOCK: u32 = 4;
    let w = (vp.width as u32 / BLOCK).max(1);
    let h = (vp.height as u32 / BLOCK).max(1);
    let Some(mut small) = Pixmap::new(w, h) else { return };
    // In this Mercator viewport longitude depends only on x and latitude only on y.
    let longitudes: Vec<_> = (0..w).map(|x| vp.to_geo((x * BLOCK + BLOCK / 2) as f64, 0.0).1).collect();
    {
        let data = small.data_mut();
        for y in 0..h {
            let lat = vp.to_geo(0.0, (y * BLOCK + BLOCK / 2) as f64).0;
            for (x, &lon) in longitudes.iter().enumerate() {
                if let Some([r, g, b, a]) = sample(lat, lon) {
                    // Premultiplied.
                    let i = (y as usize * w as usize + x) * 4;
                    let af = a as f32 / 255.0;
                    data[i] = (r as f32 * af) as u8;
                    data[i + 1] = (g as f32 * af) as u8;
                    data[i + 2] = (b as f32 * af) as u8;
                    data[i + 3] = a;
                }
            }
        }
    }
    let paint = PixmapPaint {
        quality: FilterQuality::Bilinear,
        ..PixmapPaint::default()
    };
    pm.draw_pixmap(0, 0, small.as_ref(), &paint, Transform::from_scale(BLOCK as f32, BLOCK as f32), None);
}

/// Isobars every 4 mb (marching squares on the model grid) and labelled H / L centres.
pub fn draw_isobars(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, grid: &WxGrid, tf: f32, brand: &Brand) {
    let (nx, ny) = (grid.nx, grid.ny);
    let value = |j: usize, i: usize| grid.node(Var::Pressure, tf, j, i);
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for j in 0..ny {
        for i in 0..nx {
            let v = value(j, i);
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    if !(lo.is_finite() && hi.is_finite()) || hi - lo < 0.5 {
        return;
    }
    let screen = |j: f32, i: f32| -> (f32, f32) {
        let lat = grid.north - (grid.north - grid.south) * j as f64 / (ny - 1) as f64;
        let lon = grid.west + (grid.east - grid.west) * i as f64 / (nx - 1) as f64;
        vp.to_screen(lat, lon)
    };
    let mut level = (lo / 4.0).ceil() * 4.0;
    let line_color = Color::from_rgba8(255, 255, 255, 210);
    let label_style = style(&brand.font_body, Weight::Bold, 18.0, Color::WHITE);
    while level <= hi {
        let mut pb = PathBuilder::new();
        let mut label_at: Option<(f32, f32)> = None;
        for j in 0..ny - 1 {
            for i in 0..nx - 1 {
                let (a, b, c, d) = (value(j, i), value(j, i + 1), value(j + 1, i + 1), value(j + 1, i));
                let interp = |v0: f32, v1: f32| ((level - v0) / (v1 - v0)).clamp(0.0, 1.0);
                let mut cross = Vec::with_capacity(4);
                if (a < level) != (b < level) {
                    cross.push(screen(j as f32, i as f32 + interp(a, b)));
                }
                if (b < level) != (c < level) {
                    cross.push(screen(j as f32 + interp(b, c), (i + 1) as f32));
                }
                if (d < level) != (c < level) {
                    cross.push(screen((j + 1) as f32, i as f32 + interp(d, c)));
                }
                if (a < level) != (d < level) {
                    cross.push(screen(j as f32 + interp(a, d), i as f32));
                }
                if cross.len() >= 2 {
                    pb.move_to(cross[0].0, cross[0].1);
                    pb.line_to(cross[1].0, cross[1].1);
                    if cross.len() == 4 {
                        pb.move_to(cross[2].0, cross[2].1);
                        pb.line_to(cross[3].0, cross[3].1);
                    }
                    let inside = cross[0].0 > 80.0 && cross[0].1 > 80.0 && cross[0].0 < vp.width as f32 - 80.0 && cross[0].1 < vp.height as f32 - 80.0;
                    if inside && label_at.is_none() && (i + j) % 7 == 3 {
                        label_at = Some(cross[0]);
                    }
                }
            }
        }
        if let Some(path) = pb.finish() {
            pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 120)), &stroke(4.0), Transform::identity(), None);
            pm.stroke_path(&path, &paint(line_color), &stroke(2.0), Transform::identity(), None);
        }
        if let Some((x, y)) = label_at {
            let text = format!("{level:.0}");
            let w = measure(&text, &label_style);
            halo_text(pm, glyphs, &text, x - w / 2.0, y + 6.0, &label_style);
        }
        level += 4.0;
    }

    // H / L at local extrema over a 5x5 neighbourhood with a real pressure difference.
    let mut centres = Vec::new();
    for j in 2..ny.saturating_sub(2) {
        for i in 2..nx.saturating_sub(2) {
            let v = value(j, i);
            let (mut is_max, mut is_min) = (true, true);
            let mut edge_mean = 0.0;
            let mut count = 0.0;
            for dj in -2i32..=2 {
                for di in -2i32..=2 {
                    if dj == 0 && di == 0 {
                        continue;
                    }
                    let o = value((j as i32 + dj) as usize, (i as i32 + di) as usize);
                    is_max &= v >= o;
                    is_min &= v <= o;
                    edge_mean += o;
                    count += 1.0;
                }
            }
            edge_mean /= count;
            if is_max && v - edge_mean > 0.8 {
                centres.push((j, i, true, v));
            } else if is_min && edge_mean - v > 0.8 {
                centres.push((j, i, false, v));
            }
        }
    }
    for (j, i, high, v) in centres {
        let (x, y) = screen(j as f32, i as f32);
        if x < 40.0 || y < 40.0 || x > vp.width as f32 - 40.0 || y > vp.height as f32 - 40.0 {
            continue;
        }
        let color = if high { Color::from_rgba8(40, 110, 255, 255) } else { Color::from_rgba8(235, 40, 40, 255) };
        let st = style(&brand.font_display, Weight::Black, 64.0, color);
        let letter = if high { "H" } else { "L" };
        let w = measure(letter, &st);
        halo_text(pm, glyphs, letter, x - w / 2.0, y + 22.0, &st);
        let vs = style(&brand.font_body, Weight::Bold, 18.0, Color::WHITE);
        let label = format!("{v:.0}");
        let vw = measure(&label, &vs);
        halo_text(pm, glyphs, &label, x - vw / 2.0, y + 46.0, &vs);
    }
}

/// Value placards on towns, de-cluttered so they never overlap.
/// `reserved`: on-air boxes (title, legend, logo, clock…) no plate may sit under.
pub fn draw_city_plots(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, grid: &WxGrid, field: DataField, tf: f32, brand: &Brand, reserved: &[(f32, f32, f32, f32)]) {
    // Fewer plates zoomed out: ~60 on a regional view read as clutter (CWXB-95).
    let spacing = match (vp.width < 1200.0, vp.view.zoom < 6.5) {
        (true, _) => 120.0,
        (false, true) => 200.0,
        (false, false) => 160.0,
    };
    let mut placed: Vec<(f32, f32)> = Vec::new();
    let value_style = style(&brand.font_display, Weight::Black, 34.0, parse_color("$text", brand));
    let name_style = style(&brand.font_body, Weight::SemiBold, 16.0, parse_color("$muted", brand));
    let (n, w) = vp.to_geo(0.0, 0.0);
    let (south, e) = vp.to_geo(vp.width, vp.height);
    let visible = crate::places::in_box(south, n, w, e);
    for place in visible.iter() {
        let (name, lat, lon) = (place.name.as_str(), &place.lat, &place.lon);
        let (x, y) = vp.to_screen(*lat, *lon);
        if x < 50.0 || y < 70.0 || x > vp.width as f32 - 50.0 || y > vp.height as f32 - 160.0 {
            continue;
        }
        if placed.iter().any(|(px, py)| (px - x).abs() < spacing && (py - y).abs() < spacing * 0.6) {
            continue;
        }
        // A plate is about 140 × 70 px around the town: skip it if that touches an on-air box.
        if reserved.iter().any(|(rx, ry, rw, rh)| x + 70.0 > *rx && x - 70.0 < rx + rw && y + 40.0 > *ry && y - 40.0 < ry + rh) {
            continue;
        }
        let text = match field {
            DataField::Futurecast => match grid.precip(tf, *lat, *lon) {
                Some((rate, kind)) if rate >= 0.004 => match kind {
                    PrecipKind::Snow => "SNOW".to_string(),
                    PrecipKind::Mix => "MIX".to_string(),
                    PrecipKind::Rain => "RAIN".to_string(),
                },
                _ => match grid.value(DataField::Temperature, tf, *lat, *lon) {
                    Some(t) => format!("{t:.0}°"),
                    None => continue,
                },
            },
            DataField::WindSpeed | DataField::WindGust => match grid.value(field, tf, *lat, *lon) {
                Some(v) => format!("{v:.0}"),
                None => continue,
            },
            DataField::Pressure => match grid.value(field, tf, *lat, *lon) {
                Some(v) => format!("{v:.0}"),
                None => continue,
            },
            DataField::IceTotal => match grid.value(field, tf, *lat, *lon) {
                Some(v) if v >= 0.01 => format!("{v:.2}\""),
                _ => continue,
            },
            DataField::Convergence | DataField::MoistureConvergence | DataField::ThetaEAdvection | DataField::Dryline | DataField::Boundaries | DataField::DiffHeating | DataField::ColdPool | DataField::PressureAnomaly => match grid.value(field, tf, *lat, *lon) {
                Some(v) if v.abs() >= 1.0 => format!("{v:+.0}"),
                _ => continue,
            },
            DataField::TempChange => match grid.value(field, tf, *lat, *lon) {
                Some(v) => format!("{v:+.0}°"),
                None => continue,
            },
            DataField::SnowChance | DataField::FogRisk => match grid.value(field, tf, *lat, *lon) {
                Some(v) if v >= 10.0 => format!("{v:.0}%"),
                _ => continue,
            },
            DataField::RoadHazard => match grid.value(field, tf, *lat, *lon).map(|v| v.round() as i32) {
                Some(1) => "WET".to_string(),
                Some(2) => "ICY".to_string(),
                Some(3) => "SNOW".to_string(),
                _ => continue,
            },
            DataField::RainTotal | DataField::SnowTotal | DataField::Snow10 | DataField::SnowKuchera => match grid.value(field, tf, *lat, *lon) {
                // A map full of 0.0" says nothing: only towns that get some.
                Some(v) if v >= 0.05 => format!("{v:.1}\""),
                _ => continue,
            },
            _ => match grid.value(field, tf, *lat, *lon) {
                Some(v) => format!("{v:.0}{}", field.unit()),
                None => continue,
            },
        };
        placed.push((x, y));
        let vw = measure(&text, &value_style);
        let nw = measure(name, &name_style);
        let box_w = vw.max(nw) + 28.0;
        let box_h = 66.0;
        let bx = x - box_w / 2.0;
        let by = y - box_h / 2.0;
        if let Some(path) = rounded_rect(bx, by, box_w, box_h, 10.0) {
            pm.fill_path(&path, &paint(panel_color(brand, 0.82)), FillRule::Winding, Transform::identity(), None);
        }
        // Colour tab keyed to the value.
        let tab = match field {
            DataField::Futurecast => grid
                .value(DataField::Temperature, tf, *lat, *lon)
                .and_then(|v| grid::field_rgba(DataField::Temperature, v, 1.0)),
            _ => grid.value(field, tf, *lat, *lon).and_then(|v| grid::field_rgba(field, v, 1.0)),
        };
        if let (Some([r, g, b, _]), Some(rect)) = (tab, Rect::from_xywh(bx, by + box_h - 6.0, box_w, 6.0)) {
            pm.fill_rect(rect, &paint(Color::from_rgba8(r, g, b, 255)), Transform::identity(), None);
        }
        draw_line(pm, glyphs, &text, x - vw / 2.0, by + 36.0, &value_style, 1.0, None);
        draw_line(pm, glyphs, name, x - nw / 2.0, by + 56.0, &name_style, 1.0, None);

        // Wind arrow for wind fields.
        if matches!(field, DataField::WindSpeed | DataField::WindGust) {
            if let Some((u, v)) = grid.wind(tf, *lat, *lon) {
                let speed = (u * u + v * v).sqrt();
                if speed > 1.0 {
                    let (dx, dy) = (u / speed, -v / speed);
                    let (ax, ay) = (bx + box_w + 4.0, y);
                    let len = 26.0;
                    let mut pb = PathBuilder::new();
                    pb.move_to(ax - dx * len / 2.0, ay - dy * len / 2.0);
                    pb.line_to(ax + dx * len / 2.0, ay + dy * len / 2.0);
                    if let Some(path) = pb.finish() {
                        pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 160)), &stroke(6.0), Transform::identity(), None);
                        pm.stroke_path(&path, &paint(Color::WHITE), &stroke(3.0), Transform::identity(), None);
                    }
                    let (hx, hy) = (ax + dx * len / 2.0, ay + dy * len / 2.0);
                    let angle = dy.atan2(dx);
                    let mut pb = PathBuilder::new();
                    pb.move_to(hx + angle.cos() * 6.0, hy + angle.sin() * 6.0);
                    pb.line_to(hx - 12.0 * (angle - 0.6).cos(), hy - 12.0 * (angle - 0.6).sin());
                    pb.line_to(hx - 12.0 * (angle + 0.6).cos(), hy - 12.0 * (angle + 0.6).sin());
                    pb.close();
                    if let Some(path) = pb.finish() {
                        pm.fill_path(&path, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
                    }
                }
            }
        }
    }
}

/// Animated wind-flow particles. Lives on the compositor thread across frames.
/// Impact labels: scan the view for the worst spot of each hazard and label it with a leader line
/// ("Heavy snow 8\"", "Gusts 55 mph", "Icy roads"...). Up to four, never overlapping.
pub fn draw_impact_callouts(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, grid: &WxGrid, tf: f32, brand: &Brand) {
    // (field, threshold, label maker, priority) - higher priority first.
    let checks: [(DataField, f32, fn(f32) -> String); 8] = [
        (DataField::IceTotal, 0.1, |v| format!("Ice {v:.2}\"")),
        (DataField::SnowTotal, 2.0, |v| format!("Heavy snow {v:.0}\"")),
        (DataField::RoadHazard, 1.5, |_| "Icy roads".to_string()),
        (DataField::MaxGust, 40.0, |v| format!("Gusts {v:.0} mph")),
        (DataField::RainTotal, 1.0, |v| format!("Heavy rain {v:.1}\"")),
        (DataField::FogRisk, 70.0, |_| "Dense fog likely".to_string()),
        (DataField::HighTemp, 95.0, |v| format!("Dangerous heat {v:.0}°")),
        (DataField::LowTemp, 10.0, |v| format!("Bitter cold {v:.0}°")),
    ];
    let (w, h) = (vp.width as f32, vp.height as f32);
    let step = 60.0_f32;
    let st = style(&brand.font_display, Weight::Bold, 26.0, Color::WHITE);
    let mut placed: Vec<Rect> = Vec::new();
    for (field, threshold, make) in checks {
        if placed.len() >= 4 {
            break;
        }
        let low = field == DataField::LowTemp;
        let mut best: Option<(f32, f32, f32)> = None;
        let mut y = h * 0.12;
        while y < h * 0.88 {
            let mut x = w * 0.08;
            while x < w * 0.92 {
                let (lat, lon) = vp.to_geo(x as f64, y as f64);
                if let Some(v) = grid.value(field, tf, lat, lon) {
                    let score = if low { -v } else { v };
                    if best.map(|b| score > b.2).unwrap_or(true) {
                        best = Some((x, y, score));
                    }
                }
                x += step;
            }
            y += step;
        }
        let Some((x, y, score)) = best else { continue };
        let v = if low { -score } else { score };
        if (low && v > threshold) || (!low && v < threshold) {
            continue;
        }
        let text = make(v);
        let tw = measure(&text, &st);
        // Label up and to the right of the spot (left if near the right edge).
        let left = x + tw + 120.0 > w * 0.95;
        let (lx, ly) = if left { (x - tw - 90.0, y - 70.0) } else { (x + 60.0, y - 70.0) };
        let Some(rect) = Rect::from_xywh(lx - 14.0, ly - 32.0, tw + 28.0, 44.0) else { continue };
        if placed.iter().any(|r| r.intersect(&rect).is_some()) {
            continue;
        }
        placed.push(rect);
        let mut pb = PathBuilder::new();
        pb.move_to(x, y);
        pb.line_to(if left { lx + tw + 14.0 } else { lx - 14.0 }, ly - 10.0);
        if let Some(path) = pb.finish() {
            pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 150)), &stroke(5.0), Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.5), Transform::identity(), None);
        }
        let mut dot = PathBuilder::new();
        dot.push_circle(x, y, 7.0);
        if let Some(path) = dot.finish() {
            pm.fill_path(&path, &paint(parse_color("$alert", brand)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.5), Transform::identity(), None);
        }
        if let Some(path) = rounded_rect(rect.x(), rect.y(), rect.width(), rect.height(), 8.0) {
            pm.fill_path(&path, &paint(panel_color(brand, 0.9)), FillRule::Winding, Transform::identity(), None);
        }
        draw_line(pm, glyphs, &text, lx, ly, &st, 1.0, None);
    }
}

/// Wind barbs (knots): half barb 5, full barb 10, pennant 50. The staff points to where the
/// wind blows from; calm is a ring.
pub fn draw_wind_barbs(pm: &mut Pixmap, vp: &Viewport, grid: &WxGrid, tf: f32) {
    let spacing = 78.0_f32;
    let staff = 30.0_f32;
    let (w, h) = (vp.width as f32, vp.height as f32);
    let mut lines = PathBuilder::new();
    let mut flags = PathBuilder::new();
    let mut y = spacing * 0.6;
    while y < h - spacing * 0.3 {
        let mut x = spacing * 0.6;
        while x < w - spacing * 0.3 {
            let (lat, lon) = vp.to_geo(x as f64, y as f64);
            if let Some((u, v)) = grid.wind(tf, lat, lon) {
                let mph = (u * u + v * v).sqrt();
                let knots = mph * 0.868_976;
                if knots < 2.5 {
                    lines.push_circle(x, y, 4.0);
                } else {
                    // Unit vector toward the source (screen y grows downward).
                    let (dx, dy) = (-u / mph, v / mph);
                    // Barbs sit on the left of the staff, seen from the station.
                    let (px_, py_) = (dy, -dx);
                    let (tx, ty) = (x + dx * staff, y + dy * staff);
                    lines.move_to(x, y);
                    lines.line_to(tx, ty);
                    let mut rest = ((knots / 5.0).round() * 5.0) as i32;
                    let mut pos = 0.0_f32;
                    let step = 5.0_f32;
                    let barb = 13.0_f32;
                    while rest >= 50 {
                        let (bx, by) = (tx - dx * pos, ty - dy * pos);
                        let (ex, ey) = (tx - dx * (pos + step * 1.4), ty - dy * (pos + step * 1.4));
                        flags.move_to(bx, by);
                        flags.line_to(bx + px_ * barb + dx * 3.0, by + py_ * barb + dy * 3.0);
                        flags.line_to(ex, ey);
                        flags.close();
                        pos += step * 1.8;
                        rest -= 50;
                    }
                    while rest >= 10 {
                        let (bx, by) = (tx - dx * pos, ty - dy * pos);
                        lines.move_to(bx, by);
                        lines.line_to(bx + px_ * barb + dx * 4.0, by + py_ * barb + dy * 4.0);
                        pos += step;
                        rest -= 10;
                    }
                    if rest >= 5 {
                        // A lone half barb sits one step in from the tip.
                        if pos == 0.0 {
                            pos = step;
                        }
                        let (bx, by) = (tx - dx * pos, ty - dy * pos);
                        lines.move_to(bx, by);
                        lines.line_to(bx + px_ * barb * 0.5 + dx * 2.0, by + py_ * barb * 0.5 + dy * 2.0);
                    }
                }
            }
            x += spacing;
        }
        y += spacing;
    }
    let halo = Color::from_rgba8(0, 0, 0, 170);
    if let Some(path) = lines.finish() {
        pm.stroke_path(&path, &paint(halo), &stroke(4.5), Transform::identity(), None);
        pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), Transform::identity(), None);
    }
    if let Some(path) = flags.finish() {
        pm.stroke_path(&path, &paint(halo), &stroke(3.0), Transform::identity(), None);
        pm.fill_path(&path, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
    }
}

pub struct WindParticles {
    particles: Vec<Particle>,
    seed: u32,
    view_key: (i64, i64, i64),
}

#[derive(Clone, Copy)]
struct Particle {
    trail: [(f32, f32); 8],
    len: usize,
    age: u32,
    life: u32,
}

impl Default for WindParticles {
    fn default() -> Self {
        Self {
            particles: Vec::new(),
            seed: 0x9E3779B9,
            view_key: (0, 0, 0),
        }
    }
}

impl WindParticles {
    fn rand(&mut self) -> f32 {
        // xorshift32
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed % 100_000) as f32 / 100_000.0
    }

    fn spawn(&mut self, w: f32, h: f32) -> Particle {
        let (x, y) = (self.rand() * w, self.rand() * h);
        let life = 40 + (self.rand() * 80.0) as u32;
        Particle {
            trail: [(x, y); 8],
            len: 1,
            age: 0,
            life,
        }
    }

    /// Advance one frame and draw.
    pub fn step_and_draw(&mut self, pm: &mut Pixmap, vp: &Viewport, grid: &WxGrid, tf: f32) {
        let (w, h) = (vp.width as f32, vp.height as f32);
        let key = ((vp.view.lat * 100.0) as i64, (vp.view.lon * 100.0) as i64, (vp.view.zoom * 10.0) as i64);
        // Capped: ~1,500 trails at 1080p cost Program too much (CWXB-53); 700 still reads as flow.
        let count = (((w * h) / 1400.0) as usize).min(700);
        if key != self.view_key || self.particles.len() != count {
            self.view_key = key;
            self.particles.clear();
            for _ in 0..count {
                let p = self.spawn(w, h);
                self.particles.push(p);
            }
        }
        // Pixels per frame per mph: faster winds visibly race.
        let k = 0.09_f32;
        let mut buckets: Vec<PathBuilder> = (0..4).map(|_| PathBuilder::new()).collect();
        for idx in 0..self.particles.len() {
            let mut p = self.particles[idx];
            let (x, y) = p.trail[0];
            let (lat, lon) = vp.to_geo(x as f64, y as f64);
            let wind = grid.wind(tf, lat, lon);
            let respawn = match wind {
                Some((u, v)) => {
                    let (nx, ny) = (x + u * k, y - v * k);
                    p.trail.rotate_right(1);
                    p.trail[0] = (nx, ny);
                    p.len = (p.len + 1).min(8);
                    p.age += 1;
                    let speed = (u * u + v * v).sqrt();
                    let bucket = if speed < 8.0 { 0 } else if speed < 16.0 { 1 } else if speed < 28.0 { 2 } else { 3 };
                    if p.len >= 2 {
                        let pb = &mut buckets[bucket];
                        pb.move_to(p.trail[0].0, p.trail[0].1);
                        for t in 1..p.len {
                            pb.line_to(p.trail[t].0, p.trail[t].1);
                        }
                    }
                    p.age > p.life || nx < 0.0 || ny < 0.0 || nx > w || ny > h
                }
                None => true,
            };
            self.particles[idx] = if respawn { self.spawn(w, h) } else { p };
        }
        let colors = [
            Color::from_rgba8(200, 230, 255, 150),
            Color::from_rgba8(150, 240, 200, 180),
            Color::from_rgba8(255, 230, 120, 200),
            Color::from_rgba8(255, 130, 110, 220),
        ];
        for (pb, color) in buckets.into_iter().zip(colors) {
            if let Some(path) = pb.finish() {
                pm.stroke_path(&path, &paint(color), &stroke(2.2), Transform::identity(), None);
            }
        }
    }
}

/// Lightning strikes from the last 15 minutes as bolt icons, coloured by age
/// (white = last 2 minutes, then yellow, orange, red).
pub fn draw_lightning(pm: &mut Pixmap, vp: &Viewport, strikes: &[Strike]) {
    let now = chrono::Utc::now().timestamp_millis();
    // One path per age bucket: four fills instead of one per strike.
    let mut buckets: Vec<PathBuilder> = (0..4).map(|_| PathBuilder::new()).collect();
    let mut placed: Vec<(f32, f32)> = Vec::new();
    // Newest first so fresh strikes win the de-clutter.
    let mut recent: Vec<&Strike> = strikes.iter().filter(|s| (0.0..=15.0).contains(&((now - s.time_ms) as f32 / 60_000.0))).collect();
    recent.sort_by(|a, b| b.time_ms.cmp(&a.time_ms));
    for s in recent {
        let age_min = (now - s.time_ms) as f32 / 60_000.0;
        let (x, y) = vp.to_screen(s.lat, s.lon);
        if x < -10.0 || y < -10.0 || x > vp.width as f32 + 10.0 || y > vp.height as f32 + 10.0 {
            continue;
        }
        // Dense clusters read as a storm, not a smear.
        if placed.iter().any(|(px, py)| (px - x).abs() < 9.0 && (py - y).abs() < 9.0) {
            continue;
        }
        placed.push((x, y));
        let (bucket, size) = if age_min < 2.0 {
            (0, 13.0)
        } else if age_min < 5.0 {
            (1, 11.0)
        } else if age_min < 10.0 {
            (2, 10.0)
        } else {
            (3, 9.0)
        };
        // Bolt centred on the strike.
        let k = size / 12.0;
        let pts = [(2.0, -12.0), (-6.0, 1.0), (-0.5, 1.0), (-3.0, 12.0), (7.0, -3.0), (1.5, -3.0), (5.0, -12.0)];
        let pb = &mut buckets[bucket];
        pb.move_to(x + pts[0].0 * k, y + pts[0].1 * k);
        for (px, py) in &pts[1..] {
            pb.line_to(x + px * k, y + py * k);
        }
        pb.close();
    }
    let colors = [
        Color::WHITE,
        Color::from_rgba8(255, 230, 40, 255),
        Color::from_rgba8(255, 145, 25, 255),
        Color::from_rgba8(225, 45, 35, 240),
    ];
    for (pb, color) in buckets.into_iter().zip(colors) {
        if let Some(path) = pb.finish() {
            pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 210)), &stroke(2.6), Transform::identity(), None);
            pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
        }
    }
}

/// NWS Local Storm Reports as lettered markers (T tornado, H hail, W wind, F flood, S snow),
/// with hail size / gust on the marker.
pub fn draw_storm_reports(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, reports: &[crate::data::StormReport], hours: u32, brand: &Brand) {
    let cutoff = chrono::Utc::now().timestamp() - hours.max(1) as i64 * 3600;
    let letter_style = style(&brand.font_display, Weight::Black, 20.0, Color::WHITE);
    let mag_style = style(&brand.font_body, Weight::Bold, 16.0, Color::WHITE);
    for r in reports.iter().filter(|r| r.time >= cutoff) {
        let (x, y) = vp.to_screen(r.lat, r.lon);
        if x < -20.0 || y < -20.0 || x > vp.width as f32 + 20.0 || y > vp.height as f32 + 20.0 {
            continue;
        }
        let (letter, [cr, cg, cb]) = r.marker();
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, 15.0);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(cr, cg, cb, 255)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 230)), &stroke(3.0), Transform::identity(), None);
        }
        let light = (cr as f32 * 0.299 + cg as f32 * 0.587 + cb as f32 * 0.114) > 160.0;
        let mut ls = letter_style.clone();
        if light {
            ls.color = Color::from_rgba8(10, 10, 10, 255);
        }
        let w = measure(letter, &ls);
        draw_line(pm, glyphs, letter, x - w / 2.0, y + 7.0, &ls, 1.0, None);
        if !r.magnitude.is_empty() && r.magnitude != "0" && r.magnitude != "null" {
            let unit = if r.kind == "H" { "\"" } else if matches!(r.kind.as_str(), "G" | "D" | "M" | "N") { " MPH" } else { "" };
            halo_text(pm, glyphs, &format!("{}{unit}", r.magnitude), x + 19.0, y + 6.0, &mag_style);
        }
    }
}

/// INDOT road events: closures and incidents as diamonds in INDOT's colours.
pub fn draw_road_events(pm: &mut Pixmap, vp: &Viewport, events: &[crate::data::RoadEvent], all: bool) {
    for e in events.iter().filter(|e| all || e.is_hazard()) {
        let (x, y) = vp.to_screen(e.lat, e.lon);
        if x < -10.0 || y < -10.0 || x > vp.width as f32 + 10.0 || y > vp.height as f32 + 10.0 {
            continue;
        }
        let [r, g, b] = e.rgb();
        let s = if e.kind == "closure" { 10.0 } else { 8.0 };
        let mut pb = PathBuilder::new();
        pb.move_to(x, y - s);
        pb.line_to(x + s, y);
        pb.line_to(x, y + s);
        pb.line_to(x - s, y);
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(r, g, b, 255)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 220)), &stroke(2.5), Transform::identity(), None);
        }
    }
}

/// "TOR • 14 MIN" tags on active warnings.
pub fn draw_warning_labels(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, alerts: &[Alert], brand: &Brand) {
    let mut placed: Vec<(f32, f32)> = Vec::new();
    // Only short-fused storm warnings get countdowns; river flood warnings that run for days
    // would just clutter the map.
    for alert in alerts.iter().filter(|a| a.is_short_fused()) {
        let Some((lat, lon)) = alert.centroid() else { continue };
        let (x, y) = vp.to_screen(lat, lon);
        if x < 30.0 || y < 30.0 || x > vp.width as f32 - 30.0 || y > vp.height as f32 - 30.0 {
            continue;
        }
        if placed.iter().any(|(px, py)| (px - x).abs() < 130.0 && (py - y).abs() < 40.0) {
            continue;
        }
        placed.push((x, y));
        let minutes = alert.minutes_left().unwrap_or(0).max(0);
        let when = if minutes <= 90 {
            format!("{minutes} MIN")
        } else {
            alert
                .expires_at()
                .map(|t| format!("TIL {}", t.with_timezone(&chrono::Local).format("%-I:%M%P")))
                .unwrap_or_default()
        };
        let text = format!("{} {}", alert.short_code(), when);
        let color = parse_hex(alert.color()).unwrap_or(Color::WHITE);
        // Solid tag in the warning's own colour, with text that contrasts with it.
        let light = color.red() * 0.299 + color.green() * 0.587 + color.blue() * 0.114 > 0.6;
        let text_color = if light { Color::from_rgba8(8, 8, 8, 255) } else { Color::WHITE };
        let st = style(&brand.font_display, Weight::Black, 22.0, text_color);
        let w = measure(&text, &st);
        if let Some(path) = rounded_rect(x - w / 2.0 - 11.0, y - 17.0, w + 22.0, 34.0, 6.0) {
            pm.fill_path(&path, &paint(Color::from_rgba8(0, 0, 0, 160)), FillRule::Winding, Transform::from_translate(2.0, 3.0), None);
            pm.fill_path(&path, &paint(color), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 220)), &stroke(2.0), Transform::identity(), None);
        }
        draw_line(pm, glyphs, &text, x - w / 2.0, y + 8.0, &st, 1.0, None);
    }
}

/// The TV-style alert call-out: pulsing outline on the polygon and an animated card with the
/// event, areas, threat tag and an expiry countdown, tied to the polygon by a leader line.
pub fn draw_alert_blip(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, alert: &Alert, seconds: f32, brand: &Brand, clock_24h: bool) {
    let color = parse_hex(alert.color()).unwrap_or(Color::WHITE);
    // Pulsing outline.
    let pulse = 0.5 + 0.5 * (seconds * 5.0).sin();
    for ring in &alert.polygons {
        if let Some(path) = ring_path(vp, ring, true) {
            pm.fill_path(&path, &paint(with_alpha(color, 0.18 + 0.15 * pulse)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(with_alpha(color, 0.35)), &stroke(10.0 + 10.0 * pulse), Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(3.0), Transform::identity(), None);
        }
    }
    let Some((lat, lon)) = alert.centroid() else { return };
    let (ax, ay) = vp.to_screen(lat, lon);

    // Card slides/scales in over 0.45 s.
    let t = (seconds / 0.45).clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - t).powi(3);
    let card_w = 640.0;
    let tag = alert.threat_tag();
    let card_h = if tag.is_some() { 250.0 } else { 210.0 };
    // Put the card on the side of the polygon with more room.
    let right_side = ax < vp.width as f32 * 0.55;
    let cx = if right_side { (ax + 120.0).min(vp.width as f32 - card_w - 30.0) } else { (ax - 120.0 - card_w).max(30.0) };
    let cy = (ay - card_h / 2.0).clamp(150.0, vp.height as f32 - card_h - 160.0);
    let slide = (1.0 - ease) * if right_side { 60.0 } else { -60.0 };
    let alpha = ease;

    // Leader line + anchor dot.
    let (lx, ly) = (if right_side { cx } else { cx + card_w }, cy + 40.0);
    let mut pb = PathBuilder::new();
    pb.move_to(ax, ay);
    pb.line_to(lx + slide, ly);
    if let Some(path) = pb.finish() {
        pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, (160.0 * alpha) as u8)), &stroke(6.0), Transform::identity(), None);
        pm.stroke_path(&path, &paint(with_alpha(Color::WHITE, alpha)), &stroke(3.0), Transform::identity(), None);
    }
    let mut pb = PathBuilder::new();
    pb.push_circle(ax, ay, 10.0 + 4.0 * pulse);
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
        pm.stroke_path(&path, &paint(color), &stroke(4.0), Transform::identity(), None);
    }

    let x = cx + slide;
    if let Some(path) = rounded_rect(x, cy, card_w, card_h, 14.0) {
        pm.fill_path(&path, &paint(panel_color(brand, 0.94 * alpha)), FillRule::Winding, Transform::identity(), None);
    }
    if let Some(rect) = Rect::from_xywh(x, cy, card_w, 64.0) {
        pm.fill_rect(rect, &paint(with_alpha(color, alpha)), Transform::identity(), None);
    }
    let dark_text = {
        let c = color;
        (c.red() * 0.299 + c.green() * 0.587 + c.blue() * 0.114) > 0.62
    };
    let title_color = if dark_text { Color::from_rgba8(10, 10, 10, 255) } else { Color::WHITE };
    let title = style(&brand.font_display, Weight::Black, 36.0, with_alpha(title_color, alpha));
    draw_line(pm, glyphs, &alert.event.to_uppercase(), x + 22.0, cy + 46.0, &title, 1.0, None);

    let mut y = cy + 64.0;
    if let Some(tag) = tag {
        if let Some(rect) = Rect::from_xywh(x, y, card_w, 40.0) {
            let flash = if (seconds * 2.0) as i32 % 2 == 0 { 1.0 } else { 0.75 };
            pm.fill_rect(rect, &paint(with_alpha(Color::from_rgba8(150, 0, 170, 255), alpha * flash)), Transform::identity(), None);
        }
        let ts = style(&brand.font_display, Weight::Black, 26.0, with_alpha(Color::WHITE, alpha));
        draw_line(pm, glyphs, tag, x + 22.0, y + 30.0, &ts, 1.0, None);
        y += 40.0;
    }

    let body = style(&brand.font_body, Weight::SemiBold, 24.0, with_alpha(parse_color("$text", brand), alpha));
    let areas: String = {
        let mut s = alert.area_desc.replace(';', ",");
        if s.chars().count() > 52 {
            s = s.chars().take(50).collect::<String>() + "…";
        }
        s
    };
    draw_line(pm, glyphs, &areas, x + 22.0, y + 40.0, &body, 1.0, None);

    let until = alert.expires_at().map(|t| {
        let local = t.with_timezone(&chrono::Local);
        if clock_24h {
            local.format("%H:%M").to_string()
        } else {
            local.format("%-I:%M %p").to_string()
        }
    });
    let minutes = alert.minutes_left();
    let _ = until;
    let line = alert
        .expires_label(clock_24h)
        .map(|l| l.to_uppercase())
        .unwrap_or_else(|| alert.sender.clone());
    let info = style(&brand.font_display, Weight::Bold, 28.0, with_alpha(parse_color("$text", brand), alpha));
    draw_line(pm, glyphs, &line, x + 22.0, y + 92.0, &info, 1.0, None);
    if let Some(m) = minutes.filter(|m| *m >= 0) {
        let count = format!("{} LEFT", crate::data::duration_words(m).to_uppercase());
        let cs = style(&brand.font_display, Weight::Black, 32.0, with_alpha(parse_color("$primary", brand), alpha));
        let w = measure(&count, &cs);
        draw_line(pm, glyphs, &count, x + card_w - 22.0 - w, y + 92.0, &cs, 1.0, None);
    }
}

/// Outlook legend (bottom left): the SPC risk levels 1-5 with general thunderstorms, a hazard's
/// probability colours, or the WPC excessive rainfall levels. Colours follow SPC / WPC.
pub fn draw_outlook_legend(pm: &mut Pixmap, glyphs: &mut GlyphCache, brand: &Brand, (dx, dy): (f32, f32), spc_day: Option<u8>, layer: &str, ero: bool) {
    let h = pm.height() as f32;
    let rows: Vec<(String, [u8; 3])> = if spc_day.is_some() {
        match layer {
            "tornado" => [("2%", [0, 139, 0]), ("5%", [139, 71, 38]), ("10%", [255, 200, 0]), ("15%", [255, 0, 0]), ("30%", [255, 0, 255]), ("45%", [145, 44, 238]), ("60%", [16, 78, 139])]
                .into_iter().map(|(t, c)| (t.to_string(), c)).collect(),
            "wind" | "hail" | "severe" => [("5%", [139, 71, 38]), ("15%", [255, 200, 0]), ("30%", [255, 0, 0]), ("45%", [255, 0, 255]), ("60%", [145, 44, 238])]
                .into_iter().map(|(t, c)| (t.to_string(), c)).collect(),
            "significant" => vec![("Hatched: significant severe".to_string(), [40, 40, 40])],
            _ => [("5  High", [238, 153, 238]), ("4  Moderate", [224, 102, 102]), ("3  Enhanced", [255, 163, 102]), ("2  Slight", [255, 224, 102]), ("1  Marginal", [102, 163, 102]), ("Thunderstorms", [193, 233, 193])]
                .into_iter().map(|(t, c)| (t.to_string(), c)).collect(),
        }
    } else if ero {
        [("High", [255, 0, 255]), ("Moderate", [226, 30, 30]), ("Slight", [246, 220, 52]), ("Marginal", [100, 190, 90])]
            .into_iter().map(|(t, c)| (t.to_string(), c)).collect()
    } else {
        [("Extremely critical", [255, 0, 255]), ("Critical", [255, 80, 50]), ("Elevated", [255, 170, 50])]
            .into_iter().map(|(t, c)| (t.to_string(), c)).collect()
    };
    let heading = match (spc_day.is_some(), layer) {
        (true, "tornado") => "TORNADO CHANCE",
        (true, "wind") => "DAMAGING WIND CHANCE",
        (true, "hail") => "LARGE HAIL CHANCE",
        (true, "severe") => "SEVERE CHANCE",
        (true, "significant") => "SIGNIFICANT SEVERE",
        (true, _) => "RISK LEVEL",
        (false, _) if ero => "FLOODING RAIN RISK",
        _ => "FIRE WEATHER",
    };
    let row_h = 34.0;
    let lw = 300.0;
    let lh = 52.0 + rows.len() as f32 * row_h;
    let (lx, ly) = (40.0 + dx, h - 40.0 - lh + dy);
    if let Some(path) = rounded_rect(lx, ly, lw, lh, 12.0) {
        pm.fill_path(&path, &paint(panel_color(brand, 0.86)), FillRule::Winding, Transform::identity(), None);
    }
    let title = style(&brand.font_display, Weight::Bold, 22.0, parse_color("$text", brand));
    draw_line(pm, glyphs, heading, lx + 16.0, ly + 34.0, &title, 1.0, None);
    let label_st = style(&brand.font_body, Weight::SemiBold, 19.0, parse_color("$text", brand));
    for (i, (text, rgb)) in rows.iter().enumerate() {
        let y = ly + 48.0 + i as f32 * row_h;
        if let Some(path) = rounded_rect(lx + 16.0, y + 4.0, 34.0, row_h - 8.0, 5.0) {
            pm.fill_path(&path, &paint(Color::from_rgba8(rgb[0], rgb[1], rgb[2], 255)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 120)), &stroke(1.5), Transform::identity(), None);
        }
        draw_line(pm, glyphs, text, lx + 62.0, y + row_h - 9.0, &label_st, 1.0, None);
    }
}

/// Earthquake legend (bottom left): marker sizes by magnitude and colours by age.
pub fn draw_quake_legend(pm: &mut Pixmap, glyphs: &mut GlyphCache, brand: &Brand, (dx, dy): (f32, f32), days: u32, min_mag: f32) {
    let h = pm.height() as f32;
    let (lw, lh) = (430.0, 176.0);
    let (lx, ly) = (40.0 + dx, h - 40.0 - lh + dy);
    if let Some(path) = rounded_rect(lx, ly, lw, lh, 12.0) {
        pm.fill_path(&path, &paint(panel_color(brand, 0.86)), FillRule::Winding, Transform::identity(), None);
    }
    let text = parse_color("$text", brand);
    let title = style(&brand.font_display, Weight::Bold, 22.0, text);
    let period = match days {
        1 => "PAST DAY".to_string(),
        7 => "PAST 7 DAYS".to_string(),
        d => format!("PAST {d} DAYS"),
    };
    draw_line(pm, glyphs, &format!("EARTHQUAKES · {period}"), lx + 16.0, ly + 34.0, &title, 1.0, None);
    let small = style(&brand.font_body, Weight::SemiBold, 16.0, with_alpha(text, 0.8));
    let min_text = if min_mag <= 0.0 { "All sizes · USGS".to_string() } else { format!("Magnitude {min_mag:.1}+ · USGS") };
    draw_line(pm, glyphs, &min_text, lx + 16.0, ly + 58.0, &small, 1.0, None);
    // Sizes.
    let mut x = lx + 24.0;
    for mag in [3.0f32, 5.0, 7.0] {
        let r = crate::quakes::radius(mag).min(26.0);
        if let Some(path) = PathBuilder::from_circle(x + r, ly + 98.0, r) {
            pm.fill_path(&path, &paint(Color::from_rgba8(205, 214, 226, 210)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(20, 20, 20, 220)), &stroke(1.4), Transform::identity(), None);
        }
        draw_line(pm, glyphs, &format!("M{mag:.0}"), x + 2.0 * r + 6.0, ly + 104.0, &small, 1.0, None);
        x += 2.0 * r + 62.0;
    }
    // Ages.
    let mut x = lx + 16.0;
    for (text_s, age) in [("Hour", 60i64), ("Day", 7200), ("Week", 2 * 86_400), ("Older", 9 * 86_400)] {
        let (r, g, b) = crate::quakes::age_rgb(age);
        if let Some(path) = rounded_rect(x, ly + 134.0, 22.0, 22.0, 11.0) {
            pm.fill_path(&path, &paint(Color::from_rgba8(r, g, b, 255)), FillRule::Winding, Transform::identity(), None);
        }
        draw_line(pm, glyphs, text_s, x + 28.0, ly + 152.0, &small, 1.0, None);
        x += 100.0;
    }
}

/// Drought legend (bottom left): the US Drought Monitor categories with the week the map is
/// valid for, or the CPC outlook classes.
pub fn draw_drought_legend(pm: &mut Pixmap, glyphs: &mut GlyphCache, brand: &Brand, (dx, dy): (f32, f32), view: &str) {
    let h = pm.height() as f32;
    let rgb = |(r, g, b): (u8, u8, u8)| [r, g, b];
    let (heading, sub, rows): (&str, String, Vec<(&str, [u8; 3])>) = match view {
        "month" | "season" => (
            if view == "month" { "DROUGHT OUTLOOK: NEXT MONTH" } else { "DROUGHT OUTLOOK: SEASON" },
            "NOAA Climate Prediction Center".into(),
            vec![
                ("Drought continues", rgb(super::extras::drought_trend_rgb(1))),
                ("Drought improves", rgb(super::extras::drought_trend_rgb(2))),
                ("Drought ends", rgb(super::extras::drought_trend_rgb(3))),
                ("Drought develops", rgb(super::extras::drought_trend_rgb(4))),
            ],
        ),
        _ => (
            "DROUGHT MONITOR",
            format!("Valid {}", crate::hazards::drought_valid_date(chrono::Local::now().date_naive()).format("%a %b %-d")),
            vec![
                ("Abnormally dry", rgb(super::extras::drought_rgb(0))),
                ("Moderate", rgb(super::extras::drought_rgb(1))),
                ("Severe", rgb(super::extras::drought_rgb(2))),
                ("Extreme", rgb(super::extras::drought_rgb(3))),
                ("Exceptional", rgb(super::extras::drought_rgb(4))),
            ],
        ),
    };
    let row_h = 34.0;
    let lw = 360.0;
    let lh = 78.0 + rows.len() as f32 * row_h;
    let (lx, ly) = (40.0 + dx, h - 40.0 - lh + dy);
    if let Some(path) = rounded_rect(lx, ly, lw, lh, 12.0) {
        pm.fill_path(&path, &paint(panel_color(brand, 0.86)), FillRule::Winding, Transform::identity(), None);
    }
    let title = style(&brand.font_display, Weight::Bold, 22.0, parse_color("$text", brand));
    draw_line(pm, glyphs, heading, lx + 16.0, ly + 34.0, &title, 1.0, None);
    let sub_st = style(&brand.font_body, Weight::SemiBold, 16.0, with_alpha(parse_color("$text", brand), 0.75));
    draw_line(pm, glyphs, &sub, lx + 16.0, ly + 58.0, &sub_st, 1.0, None);
    let label_st = style(&brand.font_body, Weight::SemiBold, 19.0, parse_color("$text", brand));
    for (i, (text, rgb)) in rows.iter().enumerate() {
        let y = ly + 72.0 + i as f32 * row_h;
        if let Some(path) = rounded_rect(lx + 16.0, y + 4.0, 34.0, row_h - 8.0, 5.0) {
            pm.fill_path(&path, &paint(Color::from_rgba8(rgb[0], rgb[1], rgb[2], 255)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 120)), &stroke(1.5), Transform::identity(), None);
        }
        draw_line(pm, glyphs, text, lx + 62.0, y + row_h - 9.0, &label_st, 1.0, None);
    }
}

/// Color-bar legend for a data field (bottom left), replacing the radar legend.
pub fn draw_field_legend(pm: &mut Pixmap, glyphs: &mut GlyphCache, field: DataField, brand: &Brand, (dx, dy): (f32, f32)) {
    draw_named_field_legend(pm, glyphs, field, brand, (dx, dy), None);
}

pub fn draw_named_field_legend(pm: &mut Pixmap, glyphs: &mut GlyphCache, field: DataField, brand: &Brand, (dx, dy): (f32, f32), heading: Option<&str>) {
    let h = pm.height() as f32;
    let (lx, ly, lw, lh) = (40.0 + dx, h - 136.0 + dy, 620.0, 96.0);
    if let Some(path) = rounded_rect(lx, ly, lw, lh, 12.0) {
        pm.fill_path(&path, &paint(panel_color(brand, 0.82)), FillRule::Winding, Transform::identity(), None);
    }
    let title = style(&brand.font_display, Weight::Bold, 22.0, parse_color("$text", brand));
    let label_st = style(&brand.font_body, Weight::SemiBold, 16.0, parse_color("$text", brand));
    let bar = (lx + 18.0, ly + 44.0, lw - 36.0, 18.0);

    if field == DataField::Futurecast {
        // RAIN | MIX | SNOW, like the network Futurecast legends.
        let parts: [(&str, &[(f32, [u8; 3])]); 3] = [("RAIN", grid::rain_ramp()), ("MIX", grid::mix_ramp()), ("SNOW", grid::snow_ramp())];
        let seg_w = (bar.2 - 20.0) / 3.0;
        for (k, (name, stops)) in parts.iter().enumerate() {
            let sx = bar.0 + k as f32 * (seg_w + 10.0);
            draw_gradient_even(pm, stops, (sx, bar.1, seg_w, bar.3));
            let w = measure(name, &title);
            draw_line(pm, glyphs, name, sx + seg_w / 2.0 - w / 2.0, ly + 34.0, &title, 1.0, None);
        }
        draw_line(pm, glyphs, "LIGHT", bar.0, ly + 86.0, &label_st, 1.0, None);
        let heavy = "HEAVY";
        let w = measure(heavy, &label_st);
        draw_line(pm, glyphs, heavy, bar.0 + bar.2 - w, ly + 86.0, &label_st, 1.0, None);
        return;
    }

    let (stops, labels) = grid::legend(field);
    // Grid temperatures are Fahrenheit: the legend says so ("(°)" alone named no unit).
    let unit = match field.unit().trim() {
        "°" => "°F",
        "°/hr" => "°F/hr",
        u => u,
    };
    let name = format!("{}{}", heading.unwrap_or_else(|| field.viewer_label()).to_uppercase(), if unit.is_empty() { String::new() } else { format!(" ({unit})") });
    draw_line(pm, glyphs, &name, lx + 18.0, ly + 32.0, &title, 1.0, None);
    let even = labels.len() == stops.len();
    if even {
        draw_gradient_even(pm, stops, bar);
    } else {
        draw_gradient(pm, stops, bar);
    }
    let n = labels.len().max(2);
    // On a value-scaled bar, numbers sit at their values: evenly spaced labels put "32" at about
    // 23° on the -20…110 temperature bar. Only when every label is a number inside the scale;
    // otherwise (words, "4000+", "1k") they stay evenly spaced.
    let (lo, hi) = (stops[0].0, stops[stops.len() - 1].0);
    let values: Option<Vec<f32>> = labels
        .iter()
        .map(|l| l.trim().trim_end_matches(['"', '°', '%']).trim().parse::<f32>().ok().filter(|v| (lo..=hi).contains(v)))
        .collect();
    let by_value = !even && hi > lo && values.is_some();
    for (i, label) in labels.iter().enumerate() {
        let x = match (&values, by_value) {
            (Some(v), true) => bar.0 + bar.2 * (v[i] - lo) / (hi - lo),
            _ => bar.0 + bar.2 * i as f32 / (n - 1) as f32,
        };
        let w = measure(label, &label_st);
        draw_line(pm, glyphs, label, (x - w / 2.0).clamp(bar.0, bar.0 + bar.2 - w), ly + 86.0, &label_st, 1.0, None);
    }
}

fn draw_gradient(pm: &mut Pixmap, stops: &[(f32, [u8; 3])], (x, y, w, h): (f32, f32, f32, f32)) {
    let steps = 60;
    let (lo, hi) = (stops[0].0, stops[stops.len() - 1].0);
    for s in 0..steps {
        let v = lo + (hi - lo) * s as f32 / (steps - 1) as f32;
        let c = sample_stops(stops, v);
        if let Some(rect) = Rect::from_xywh(x + w * s as f32 / steps as f32, y, w / steps as f32 + 1.0, h) {
            pm.fill_rect(rect, &paint(Color::from_rgba8(c[0], c[1], c[2], 255)), Transform::identity(), None);
        }
    }
}

/// A legend bar with the stops evenly spaced (stop i at i / (n - 1) of the width), the broadcast
/// style where each label sits on its own colour.
fn draw_gradient_even(pm: &mut Pixmap, stops: &[(f32, [u8; 3])], (x, y, w, h): (f32, f32, f32, f32)) {
    let steps = 120;
    let last = (stops.len() - 1).max(1) as f32;
    for s in 0..steps {
        let f = s as f32 / (steps - 1) as f32 * last;
        let (i, t) = ((f.floor() as usize).min(stops.len() - 1), f.fract());
        let (a, b) = (stops[i].1, stops[(i + 1).min(stops.len() - 1)].1);
        let mix = |p: u8, q: u8| (p as f32 + (q as f32 - p as f32) * t) as u8;
        if let Some(rect) = Rect::from_xywh(x + w * s as f32 / steps as f32, y, w / steps as f32 + 1.0, h) {
            pm.fill_rect(rect, &paint(Color::from_rgba8(mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2]), 255)), Transform::identity(), None);
        }
    }
}

fn sample_stops(stops: &[(f32, [u8; 3])], v: f32) -> [u8; 3] {
    if v <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        if v <= w[1].0 {
            let t = ((v - w[0].0) / (w[1].0 - w[0].0)).clamp(0.0, 1.0);
            let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
            return [mix(w[0].1[0], w[1].1[0]), mix(w[0].1[1], w[1].1[1]), mix(w[0].1[2], w[1].1[2])];
        }
    }
    stops[stops.len() - 1].1
}

/// Legend for single-site products: velocity (inbound/outbound) or echo tops.
#[allow(clippy::too_many_arguments)]
pub fn draw_site_legend(pm: &mut Pixmap, glyphs: &mut GlyphCache, product: crate::map::scene::SiteProduct, site: &str, brand: &Brand, (dx, dy): (f32, f32), twc: bool, min_dbz: f32) {
    use crate::map::scene::SiteProduct;
    let h = pm.height() as f32;
    let (lx, ly, lw, lh) = (40.0 + dx, h - 136.0 + dy, 620.0, 96.0);
    if let Some(path) = rounded_rect(lx, ly, lw, lh, 12.0) {
        pm.fill_path(&path, &paint(panel_color(brand, 0.82)), FillRule::Winding, Transform::identity(), None);
    }
    let title = style(&brand.font_display, Weight::Bold, 22.0, parse_color("$text", brand));
    let label_st = style(&brand.font_body, Weight::SemiBold, 16.0, parse_color("$text", brand));
    draw_line(pm, glyphs, &format!("K{site} • {}", product.label().to_uppercase()), lx + 18.0, ly + 32.0, &title, 1.0, None);
    let bar = (lx + 18.0, ly + 44.0, lw - 36.0, 18.0);
    let (stops, labels): (&[(f32, [u8; 3])], &[&str]) = match product {
        SiteProduct::Velocity | SiteProduct::StormRelative => (
            &[
                (-60.0, [255, 0, 120]),
                (-40.0, [0, 120, 0]),
                (-20.0, [60, 200, 60]),
                (-5.0, [150, 230, 150]),
                (0.0, [120, 120, 120]),
                (5.0, [230, 150, 150]),
                (20.0, [220, 50, 50]),
                (40.0, [150, 0, 0]),
                (60.0, [255, 230, 0]),
            ],
            &["INBOUND", "← TOWARD RADAR", "0", "AWAY →", "OUTBOUND"],
        ),
        SiteProduct::EchoTops => (
            &[(0.0, [60, 60, 60]), (15.0, [60, 140, 220]), (30.0, [60, 200, 90]), (40.0, [240, 220, 40]), (50.0, [240, 120, 30]), (65.0, [220, 30, 200])],
            &["0", "15K FT", "30K", "40K", "50K", "65K"],
        ),
        _ => (&[][..], &[][..]),
    };
    // Reflectivity: the scale the map is really drawn in (TWC colours on Modern HD), starting
    // at the clear-air threshold (CWXB-88).
    let refl: Vec<(f32, [u8; 3])>;
    let refl_labels: Vec<String>;
    let (stops, labels): (&[(f32, [u8; 3])], Vec<&str>) = if stops.is_empty() {
        let lo = min_dbz.max(if twc { 10.0 } else { 5.0 });
        refl = (0..=12)
            .map(|k| lo + (65.0 - lo) * k as f32 / 12.0)
            .filter_map(|d| if twc { crate::map::site_radar::twc_color(d) } else { Some(nws_site_color(d)) }.map(|c| (d, c)))
            .collect();
        refl_labels = (0..5).map(|k| lo + (65.0 - lo) * k as f32 / 4.0).map(|d| format!("{d:.0}")).chain(std::iter::once("dBZ".to_string())).collect();
        (&refl[..], refl_labels.iter().take(4).map(String::as_str).chain(std::iter::once("65 dBZ")).collect())
    } else {
        (stops, labels.to_vec())
    };
    draw_gradient(pm, stops, bar);
    let n = labels.len().max(2);
    for (i, label) in labels.iter().enumerate() {
        let x = bar.0 + bar.2 * i as f32 / (n - 1) as f32;
        let w = measure(label, &label_st);
        draw_line(pm, glyphs, label, (x - w / 2.0).clamp(bar.0, bar.0 + bar.2 - w), ly + 86.0, &label_st, 1.0, None);
    }
}

/// NWS reflectivity colours for the site legend when the house palette is off.
fn nws_site_color(d: f32) -> [u8; 3] {
    let stops: [(f32, [u8; 3]); 5] = [(5.0, [40, 200, 240]), (20.0, [40, 200, 60]), (35.0, [240, 220, 40]), (50.0, [240, 40, 30]), (65.0, [200, 40, 200])];
    for w in stops.windows(2) {
        if d <= w[1].0 {
            let t = ((d - w[0].0) / (w[1].0 - w[0].0)).clamp(0.0, 1.0);
            let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
            return [m(w[0].1[0], w[1].1[0]), m(w[0].1[1], w[1].1[1]), m(w[0].1[2], w[1].1[2])];
        }
    }
    stops[4].1
}

#[cfg(test)]
mod twc_qa {
    use super::*;

    /// Single-site reflectivity legend, TWC and NWS colours (CWX_QA_NEW_DIR set).
    #[test]
    fn qa_site_legend() {
        let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) else { return };
        let mut pm = Pixmap::new(700, 330).unwrap();
        pm.fill(Color::from_rgba8(26, 28, 32, 255));
        let brand = crate::config::Settings::default().brand;
        let mut glyphs = GlyphCache::default();
        draw_site_legend(&mut pm, &mut glyphs, crate::map::scene::SiteProduct::SuperRes, "VWX", &brand, (0.0, -160.0), true, 18.0);
        draw_site_legend(&mut pm, &mut glyphs, crate::map::scene::SiteProduct::SuperRes, "VWX", &brand, (0.0, 0.0), false, 0.0);
        pm.save_png(dir.join("site-legend.png")).unwrap();
    }

    /// Legends and a rate strip for the TWC precipitation colours (CWX_QA_NEW_DIR set).
    #[test]
    fn qa_twc_precip() {
        let Some(dir) = std::env::var_os("CWX_QA_NEW_DIR").map(std::path::PathBuf::from) else { return };
        let mut pm = Pixmap::new(1400, 640).unwrap();
        pm.fill(Color::from_rgba8(26, 28, 32, 255));
        let brand = crate::config::Settings::default().brand;
        let mut glyphs = GlyphCache::default();
        // Strips: rain, mix and snow rates from 0.002 to 1 in/h (log scale), left to right.
        for (row, kind) in [PrecipKind::Rain, PrecipKind::Mix, PrecipKind::Snow].into_iter().enumerate() {
            for x in 0..1320u32 {
                let rate = 0.002f32 * (500.0f32).powf(x as f32 / 1319.0);
                if let Some([r, g, b, _]) = grid::precip_rgba(rate, kind, 1.0) {
                    if let Some(rect) = Rect::from_xywh(40.0 + x as f32, 20.0 + row as f32 * 50.0, 1.0, 40.0) {
                        pm.fill_rect(rect, &paint(Color::from_rgba8(r, g, b, 255)), Transform::identity(), None);
                    }
                }
            }
        }
        draw_named_field_legend(&mut pm, &mut glyphs, DataField::RainTotal, &brand, (0.0, -330.0), None);
        draw_named_field_legend(&mut pm, &mut glyphs, DataField::SnowTotal, &brand, (680.0, -330.0), None);
        draw_named_field_legend(&mut pm, &mut glyphs, DataField::Futurecast, &brand, (0.0, -200.0), None);
        draw_model_scale_legend(&mut pm, &mut glyphs, &brand, (680.0, -200.0), crate::futurecast::Product::PrecipitationRate);
        draw_model_scale_legend(&mut pm, &mut glyphs, &brand, (0.0, -60.0), crate::futurecast::Product::GfsReflectivity);
        pm.save_png(dir.join("twc-precip.png")).unwrap();
    }
}
