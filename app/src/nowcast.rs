//! Nowcasting from the tracked storm cells, the home sounding and the model grid: storm motion
//! uncertainty (cones), thunderstorm initiation areas, decay chances, hail / damaging wind /
//! heavy rain threats, arrival probability maps for the next hour, and a running check of past
//! nowcast positions against where the storms actually went.

use std::sync::{Mutex, OnceLock};

use tiny_skia::{Color, FillRule, PathBuilder, Pixmap, Rect, Transform};

use crate::{
    cells::Track,
    config::Brand,
    data::{StormCell, WxSnapshot},
    gfx::text::{GlyphCache, Weight},
    map::{
        geo::{haversine_km, Viewport},
        overlays::{halo_text, paint, stroke, style},
        scene::DataField,
    },
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CellNowcast {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    /// km/h and degrees (toward), with one-sigma spreads.
    pub speed: f64,
    pub heading: f64,
    pub speed_sd: f64,
    pub heading_sd: f64,
    /// 0-100 %.
    pub decay: f32,
    /// 0 none … 3 high.
    pub hail: u8,
    pub hail_size_in: f32,
    pub wind: u8,
    pub rain: u8,
    pub summary: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nowcast {
    pub cells: Vec<CellNowcast>,
    /// Initiation boxes: (lat, lon, half-size deg, chance 0-1).
    pub initiation: Vec<(f64, f64, f64, f32)>,
    /// Arrival probability at home within 60 min.
    pub home_arrival: f32,
    pub home_eta_min: Option<f32>,
    /// Mean 30-minute position error of past nowcasts, km, and how many were checked.
    pub verify_km: Option<f32>,
    pub verify_n: u32,
    /// Recent (predicted, actual) position pairs for the map.
    pub verify_pairs: Vec<((f64, f64), (f64, f64))>,
}

/// Velocity samples between consecutive track points: (km/h, heading toward).
fn velocities(t: &Track) -> Vec<(f64, f64)> {
    t.points
        .windows(2)
        .filter_map(|w| {
            let dt = (w[1].time - w[0].time) as f64 / 3600.0;
            if dt <= 0.0 {
                return None;
            }
            let d = haversine_km(w[0].lat, w[0].lon, w[1].lat, w[1].lon);
            let dy = w[1].lat - w[0].lat;
            let dx = (w[1].lon - w[0].lon) * w[0].lat.to_radians().cos();
            Some((d / dt, dx.atan2(dy).to_degrees().rem_euclid(360.0)))
        })
        .collect()
}

fn mean_sd(v: &[f64]) -> (f64, f64) {
    if v.is_empty() {
        return (0.0, 0.0);
    }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len() as f64;
    (m, var.sqrt())
}

/// Position after `minutes` along heading at speed.
pub fn advect(lat: f64, lon: f64, speed_kmh: f64, heading: f64, minutes: f64) -> (f64, f64) {
    let d = speed_kmh * minutes / 60.0;
    let h = heading.to_radians();
    (lat + d * h.cos() / 111.0, lon + d * h.sin() / (111.0 * lat.to_radians().cos().max(0.2)))
}

struct History {
    /// (cell id, made unix, predicted lat, lon at made + 30 min)
    predictions: Vec<(String, i64, f64, f64)>,
    errors: Vec<(i64, f32, (f64, f64), (f64, f64))>,
}

fn history() -> &'static Mutex<History> {
    static H: OnceLock<Mutex<History>> = OnceLock::new();
    H.get_or_init(|| Mutex::new(History { predictions: Vec::new(), errors: Vec::new() }))
}

/// Build the nowcast. `home` is (lat, lon).
pub fn compute(data: &WxSnapshot, home: (f64, f64)) -> Nowcast {
    let snd = crate::sounding::latest();
    let mucape = snd.as_ref().map(|s| s.mu.cape).unwrap_or(0.0);
    let shear = snd.as_ref().map(|s| s.shear06).unwrap_or(0.0);
    let fzl_ft = snd.as_ref().and_then(|s| s.freezing_m).map(|m| m * 3.281).unwrap_or(12000.0);
    let lapse = snd.as_ref().map(|s| s.lapse_75).unwrap_or(6.5);
    let now = chrono::Utc::now().timestamp();
    let mut out = Nowcast::default();
    let tracks = &data.analysis.tracks;

    for cell in &data.cells {
        let track = tracks.iter().find(|t| t.cell_id == cell.id && !t.ended);
        let vel = track.map(velocities).unwrap_or_default();
        let (speed, speed_sd, heading, heading_sd) = if vel.len() >= 2 {
            let (sm, ssd) = mean_sd(&vel.iter().map(|v| v.0).collect::<Vec<_>>());
            // Circular mean for headings.
            let (s, c) = vel.iter().fold((0.0, 0.0), |(s, c), v| (s + v.1.to_radians().sin(), c + v.1.to_radians().cos()));
            let hm = s.atan2(c).to_degrees().rem_euclid(360.0);
            let r = ((s * s + c * c).sqrt() / vel.len() as f64).clamp(1e-6, 1.0);
            let hsd = (-2.0 * r.ln()).sqrt().to_degrees();
            (sm, ssd.max(sm * 0.1), hm, hsd.clamp(5.0, 60.0))
        } else if let Some(m) = &cell.motion {
            (m.speed_kmh, m.speed_kmh * 0.25, m.heading_deg, 25.0)
        } else {
            (0.0, 0.0, 0.0, 90.0)
        };
        // Decay: weakening trend, shrinking area, fading lightning, little instability left.
        let mut decay = 30.0f32;
        if let Some(t) = track {
            decay -= t.trend_score as f32 * 8.0;
            if t.points.len() >= 3 {
                let a = &t.points[t.points.len() - 3];
                let b = t.points.last().unwrap();
                decay += (a.dbz - b.dbz).clamp(-10.0, 10.0) * 3.0;
                decay += ((a.area - b.area) / a.area.max(1.0) * 40.0).clamp(-20.0, 20.0);
                decay += (a.flashes as f32 - b.flashes as f32).clamp(-10.0, 10.0) * 2.0;
            }
            if t.jump {
                decay -= 20.0;
            }
        }
        if mucape < 500.0 {
            decay += 20.0;
        } else if mucape > 2000.0 {
            decay -= 15.0;
        }
        let decay = decay.clamp(2.0, 98.0);
        // Hail: reflectivity core, instability, freezing level.
        let dbz = cell.max_dbz as f32;
        let size = ((dbz - 50.0) * 0.12 + mucape / 4000.0 - ((fzl_ft - 10000.0) / 10000.0) * 0.5 + if cell.tags.iter().any(|t| t.to_lowercase().contains("hail")) { 0.5 } else { 0.0 }).max(0.0);
        let hail = match size {
            s if s >= 1.75 => 3,
            s if s >= 1.0 => 2,
            s if s >= 0.5 => 1,
            _ => 0,
        };
        // Damaging wind: fast motion, strong shear, steep lapse rates, bowing / line segments.
        let mph = speed / 1.609;
        // Part of a squall line / MCS: inside its outline (hull is (lat, lon)).
        let in_mcs = data.analysis.mcs.iter().any(|m| {
            let (mut lo_lat, mut hi_lat, mut lo_lon, mut hi_lon) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
            for (a, b) in &m.hull {
                lo_lat = lo_lat.min(*a);
                hi_lat = hi_lat.max(*a);
                lo_lon = lo_lon.min(*b);
                hi_lon = hi_lon.max(*b);
            }
            cell.lat >= lo_lat - 0.05 && cell.lat <= hi_lat + 0.05 && cell.lon >= lo_lon - 0.05 && cell.lon <= hi_lon + 0.05
        });
        let wind_score = (mph / 20.0) as f32 + shear / 20.0 + (lapse - 6.5).max(0.0) * 0.6 + if in_mcs { 1.0 } else { 0.0 } + if dbz >= 55.0 { 0.5 } else { 0.0 };
        let wind = match wind_score {
            s if s >= 3.5 => 3,
            s if s >= 2.5 => 2,
            s if s >= 1.5 => 1,
            _ => 0,
        };
        // Heavy rain: slow movement over a strong core, more so when cells train on the same path.
        let training = data.cells.iter().filter(|o| o.id != cell.id).filter(|o| {
            let d = haversine_km(o.lat, o.lon, cell.lat, cell.lon);
            let back = (heading + 180.0).to_radians();
            let dy = (o.lat - cell.lat) * 111.0;
            let dx = (o.lon - cell.lon) * 111.0 * cell.lat.to_radians().cos();
            let along = dx * back.sin() + dy * back.cos();
            d < 60.0 && along > 10.0 && (dx * back.cos() - dy * back.sin()).abs() < 12.0
        }).count();
        let rain_score = (if mph < 10.0 { 1.5 } else if mph < 20.0 { 0.8 } else { 0.0 }) + ((dbz - 45.0) / 10.0).max(0.0) + training as f32 * 0.8;
        let rain = match rain_score {
            s if s >= 3.0 => 3,
            s if s >= 2.0 => 2,
            s if s >= 1.0 => 1,
            _ => 0,
        };
        let level = |n: u8| ["low", "marginal", "elevated", "high"][n as usize];
        let summary = format!(
            "Hail {}{} • wind {} • heavy rain {} • {:.0}% chance it weakens in 30 min",
            level(hail),
            if hail > 0 { format!(" (up to {:.2}\")", size.min(4.0)) } else { String::new() },
            level(wind),
            level(rain),
            decay
        );
        out.cells.push(CellNowcast { id: cell.id.clone(), lat: cell.lat, lon: cell.lon, speed, heading, speed_sd, heading_sd, decay, hail, hail_size_in: size, wind, rain, summary });
    }

    // Arrival probability at home in the next hour.
    let mut miss = 1.0f32;
    let mut eta: Option<f32> = None;
    for c in &out.cells {
        let p = arrival_probability(c, home.0, home.1, 60.0);
        miss *= 1.0 - p;
        if p > 0.3 {
            let d = haversine_km(c.lat, c.lon, home.0, home.1);
            if c.speed > 5.0 {
                let m = (d / c.speed * 60.0) as f32;
                eta = Some(eta.map(|e| e.min(m)).unwrap_or(m));
            }
        }
    }
    out.home_arrival = 1.0 - miss;
    out.home_eta_min = eta;

    // Initiation: unstable, converging air in the model grid with no storm nearby yet, and storms
    // in the model's next two hours.
    if let Some(grid) = &data.grid {
        let tf1 = grid.time_index(1.0);
        let tf2 = grid.time_index(2.0);
        let (lat0, lon0) = home;
        let step = 0.25;
        for iy in -8..=8 {
            for ix in -10..=10 {
                let (lat, lon) = (lat0 + iy as f64 * step, lon0 + ix as f64 * step * 1.3);
                let cape = grid.value(DataField::Cape, tf1, lat, lon).unwrap_or(0.0);
                if cape < 800.0 {
                    continue;
                }
                if data.cells.iter().any(|c| haversine_km(c.lat, c.lon, lat, lon) < 30.0) {
                    continue;
                }
                let conv = grid.value(DataField::MoistureConvergence, tf1, lat, lon).unwrap_or(0.0);
                let sim = grid.value(DataField::Futurecast, tf2, lat, lon).unwrap_or(0.0).max(grid.value(DataField::Futurecast, tf1, lat, lon).unwrap_or(0.0));
                let chance = ((cape / 3000.0).min(1.0) * 0.4 + (conv.max(0.0) / 2.0).min(1.0) * 0.3 + (sim / 45.0).min(1.0) * 0.3).clamp(0.0, 1.0);
                if chance >= 0.35 {
                    out.initiation.push((lat, lon, step / 2.0, chance));
                }
            }
        }
    }

    // Nowcast verification: record +30 min positions, check the ones now due.
    if let Ok(mut h) = history().lock() {
        for c in &out.cells {
            if c.speed > 5.0 && !h.predictions.iter().any(|(id, made, _, _)| id == &c.id && now - made < 300) {
                let (plat, plon) = advect(c.lat, c.lon, c.speed, c.heading, 30.0);
                h.predictions.push((c.id.clone(), now, plat, plon));
            }
        }
        let due: Vec<(String, i64, f64, f64)> = h.predictions.iter().filter(|(_, made, _, _)| now - made >= 1800).cloned().collect();
        h.predictions.retain(|(_, made, _, _)| now - made < 1800);
        for (id, made, plat, plon) in due {
            // Where did the cell really go? The track point nearest the predicted time.
            if let Some(t) = tracks.iter().find(|t| t.cell_id == id) {
                if let Some(p) = t.points.iter().min_by_key(|p| (p.time - (made + 1800)).abs()).filter(|p| (p.time - (made + 1800)).abs() < 600) {
                    let err = haversine_km(plat, plon, p.lat, p.lon) as f32;
                    h.errors.push((now, err, (plat, plon), (p.lat, p.lon)));
                }
            }
        }
        h.errors.retain(|(t, _, _, _)| now - t < 6 * 3600);
        if !h.errors.is_empty() {
            out.verify_km = Some(h.errors.iter().map(|e| e.1).sum::<f32>() / h.errors.len() as f32);
            out.verify_n = h.errors.len() as u32;
            out.verify_pairs = h.errors.iter().rev().take(40).map(|e| (e.2, e.3)).collect();
        }
    }
    out
}

/// Chance a cell's core (≈10 km wide) passes over a point within `minutes`, from its motion and
/// the spread of its recent motion.
pub fn arrival_probability(c: &CellNowcast, lat: f64, lon: f64, minutes: f64) -> f32 {
    let dy = (lat - c.lat) * 111.0;
    let dx = (lon - c.lon) * 111.0 * c.lat.to_radians().cos();
    let h = c.heading.to_radians();
    let along = dx * h.sin() + dy * h.cos();
    let cross = dx * h.cos() - dy * h.sin();
    if c.speed < 3.0 {
        let d = (dx * dx + dy * dy).sqrt();
        return if d < 8.0 { 0.9 } else { 0.0 };
    }
    let reach = (c.speed + c.speed_sd) * minutes / 60.0;
    if along < -5.0 || along > reach + 10.0 {
        return 0.0;
    }
    // Cross-track spread grows with distance travelled.
    let sigma = 5.0 + along.max(0.0) * c.heading_sd.to_radians().tan().abs().min(1.0);
    let core = 7.0;
    let z = (cross.abs() - core).max(0.0) / sigma;
    ((-0.5 * z * z).exp() * 0.95) as f32
}

/// Map layer modes: "arrival" (probability of a storm in the next hour), "cones" (motion
/// uncertainty), "initiation", "threats" (hail / wind / rain badges, decay), "verify".
pub fn draw(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, n: &Nowcast, mode: &str, brand: &Brand) {
    let label = style(&brand.font_body, Weight::Bold, 16.0, Color::WHITE);
    match mode {
        "arrival" => {
            let (s, w, nn, e) = vp.bounds();
            let (cols, rows) = (64, 36);
            for j in 0..rows {
                for i in 0..cols {
                    let lat = nn - (nn - s) * (j as f64 + 0.5) / rows as f64;
                    let lon = w + (e - w) * (i as f64 + 0.5) / cols as f64;
                    let p = 1.0 - n.cells.iter().map(|c| 1.0 - arrival_probability(c, lat, lon, 60.0)).product::<f32>();
                    if p < 0.1 {
                        continue;
                    }
                    let (x0, y0) = vp.to_screen(nn - (nn - s) * j as f64 / rows as f64, w + (e - w) * i as f64 / cols as f64);
                    let (x1, y1) = vp.to_screen(nn - (nn - s) * (j + 1) as f64 / rows as f64, w + (e - w) * (i + 1) as f64 / cols as f64);
                    let (r, g, b) = if p >= 0.7 { (220, 40, 60) } else if p >= 0.4 { (245, 150, 30) } else { (250, 220, 60) };
                    if let Some(rect) = Rect::from_ltrb(x0, y0, x1 + 0.5, y1 + 0.5) {
                        pm.fill_rect(rect, &paint(Color::from_rgba8(r, g, b, (60.0 + p * 110.0) as u8)), Transform::identity(), None);
                    }
                }
            }
        }
        "cones" => {
            for c in n.cells.iter().filter(|c| c.speed > 5.0) {
                let mut pts = vec![vp.to_screen(c.lat, c.lon)];
                for side in [-1.0, 1.0] {
                    for k in 1..=6 {
                        let m = k as f64 * 10.0;
                        let (la, lo) = advect(c.lat, c.lon, c.speed + c.speed_sd * 0.5, c.heading + side * c.heading_sd, m);
                        pts.push(vp.to_screen(la, lo));
                    }
                    if side < 0.0 {
                        pts.push(vp.to_screen(advect(c.lat, c.lon, c.speed + c.speed_sd, c.heading, 60.0).0, advect(c.lat, c.lon, c.speed + c.speed_sd, c.heading, 60.0).1));
                    }
                }
                // Left edge out, right edge back.
                let left: Vec<(f32, f32)> = pts[1..8].to_vec();
                let mut right: Vec<(f32, f32)> = pts[8..].to_vec();
                right.reverse();
                let mut pb = PathBuilder::new();
                pb.move_to(pts[0].0, pts[0].1);
                for p in left.iter().chain(right.iter()) {
                    pb.line_to(p.0, p.1);
                }
                pb.close();
                if let Some(path) = pb.finish() {
                    pm.fill_path(&path, &paint(Color::from_rgba8(255, 255, 255, 50)), FillRule::Winding, Transform::identity(), None);
                    pm.stroke_path(&path, &paint(Color::from_rgba8(255, 255, 255, 200)), &stroke(1.5), Transform::identity(), None);
                }
                for m in [15.0, 30.0, 45.0, 60.0] {
                    let (la, lo) = advect(c.lat, c.lon, c.speed, c.heading, m);
                    let (x, y) = vp.to_screen(la, lo);
                    let mut pb = PathBuilder::new();
                    pb.push_circle(x, y, 4.0);
                    if let Some(path) = pb.finish() {
                        pm.fill_path(&path, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
                    }
                    if vp.view.zoom >= 8.0 {
                        halo_text(pm, glyphs, &format!("{m:.0}m"), x + 6.0, y + 5.0, &label);
                    }
                }
            }
        }
        "initiation" => {
            for (lat, lon, half, chance) in &n.initiation {
                let (x0, y0) = vp.to_screen(lat + half, lon - half * 1.3);
                let (x1, y1) = vp.to_screen(lat - half, lon + half * 1.3);
                let a = (60.0 + chance * 120.0) as u8;
                if let Some(rect) = Rect::from_ltrb(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)) {
                    pm.fill_rect(rect, &paint(Color::from_rgba8(255, 210, 40, a / 2)), Transform::identity(), None);
                    // Hatching.
                    let mut pb = PathBuilder::new();
                    let mut k = rect.left() - rect.height();
                    while k < rect.right() {
                        pb.move_to(k.max(rect.left()), rect.bottom() - (k.max(rect.left()) - k));
                        pb.line_to((k + rect.height()).min(rect.right()), rect.bottom() - ((k + rect.height()).min(rect.right()) - k));
                        k += 10.0;
                    }
                    if let Some(path) = pb.finish() {
                        pm.stroke_path(&path, &paint(Color::from_rgba8(255, 210, 40, a)), &stroke(1.2), Transform::identity(), None);
                    }
                }
            }
            if let Some((lat, lon, _, _)) = n.initiation.iter().max_by(|a, b| a.3.total_cmp(&b.3)) {
                let (x, y) = vp.to_screen(*lat, *lon);
                halo_text(pm, glyphs, "New storms possible", x + 10.0, y, &label);
            }
        }
        "threats" => {
            for c in &n.cells {
                let (x, y) = vp.to_screen(c.lat, c.lon);
                let badges = [("H", c.hail, (90, 200, 255)), ("W", c.wind, (255, 160, 40)), ("R", c.rain, (60, 220, 120))];
                let mut bx = x + 12.0;
                for (letter, lvl, col) in badges {
                    if lvl == 0 {
                        continue;
                    }
                    let size = 12.0 + lvl as f32 * 3.0;
                    if let Some(rect) = Rect::from_xywh(bx, y - size, size, size) {
                        pm.fill_rect(rect, &paint(Color::from_rgba8(col.0, col.1, col.2, 235)), Transform::identity(), None);
                    }
                    halo_text(pm, glyphs, letter, bx + size * 0.25, y - size * 0.2, &label);
                    bx += size + 4.0;
                }
                if c.decay >= 60.0 && vp.view.zoom >= 7.5 {
                    halo_text(pm, glyphs, &format!("{:.0}% weakening", c.decay), x + 12.0, y + 18.0, &label);
                }
            }
        }
        "verify" => {
            for ((pa, po), (aa, ao)) in &n.verify_pairs {
                let (x0, y0) = vp.to_screen(*pa, *po);
                let (x1, y1) = vp.to_screen(*aa, *ao);
                let mut pb = PathBuilder::new();
                pb.move_to(x0, y0);
                pb.line_to(x1, y1);
                if let Some(path) = pb.finish() {
                    pm.stroke_path(&path, &paint(Color::from_rgba8(255, 255, 255, 160)), &stroke(1.5), Transform::identity(), None);
                }
                for (x, y, col) in [(x0, y0, (255, 200, 40)), (x1, y1, (60, 200, 255))] {
                    let mut pb = PathBuilder::new();
                    pb.push_circle(x, y, 5.0);
                    if let Some(path) = pb.finish() {
                        pm.fill_path(&path, &paint(Color::from_rgba8(col.0, col.1, col.2, 255)), FillRule::Winding, Transform::identity(), None);
                    }
                }
            }
            let text = match n.verify_km {
                Some(km) => format!("30-minute nowcast error: {:.1} mi average ({} checked) • yellow = predicted, blue = actual", km / 1.609, n.verify_n),
                None => "Nowcast check: waiting for 30 minutes of tracked storms".to_string(),
            };
            halo_text(pm, glyphs, &text, vp.width as f32 * 0.04, vp.height as f32 * 0.93, &label);
        }
        _ => {}
    }
}

/// Bindings: nowcast.arrival (home, next hour), nowcast.eta, nowcast.top (strongest threat),
/// nowcast.hail / .wind / .rain (highest level), nowcast.error.
pub fn bind(b: &mut crate::gfx::bind::Bindings, n: &Nowcast) {
    let level = |v: u8| ["None", "Marginal", "Elevated", "High"][v.min(3) as usize];
    b.set("nowcast.arrival", &format!("{:.0}%", n.home_arrival * 100.0));
    b.set("nowcast.eta", &n.home_eta_min.map(|m| format!("{m:.0} min")).unwrap_or_else(|| "--".into()));
    b.set("nowcast.hail", level(n.cells.iter().map(|c| c.hail).max().unwrap_or(0)));
    b.set("nowcast.wind", level(n.cells.iter().map(|c| c.wind).max().unwrap_or(0)));
    b.set("nowcast.rain", level(n.cells.iter().map(|c| c.rain).max().unwrap_or(0)));
    let top = n.cells.iter().max_by_key(|c| c.hail.max(c.wind).max(c.rain));
    b.set("nowcast.top", &top.map(|c| c.summary.clone()).unwrap_or_else(|| "No storms tracked".into()));
    b.set("nowcast.error", &n.verify_km.map(|k| format!("{:.1} mi", k / 1.609)).unwrap_or_else(|| "--".into()));
    b.set("nowcast.initiation", if n.initiation.is_empty() { "No new storms expected nearby" } else { "New storms possible nearby" });
}

/// Cell list for the Lab tab and the Severe Center.
pub fn cell_line(c: &CellNowcast, cell: Option<&StormCell>) -> String {
    format!(
        "{} • moving {:.0}±{:.0} mph toward {:.0}°±{:.0}° • {}",
        cell.map(|c| format!("{:.0} dBZ, {:.0} mi {}", c.max_dbz, c.distance_mi, c.direction)).unwrap_or_else(|| c.id.clone()),
        c.speed / 1.609,
        c.speed_sd / 1.609,
        c.heading,
        c.heading_sd,
        c.summary
    )
}
