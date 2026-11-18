//! Model sounding at a point (Open-Meteo pressure levels): hodograph, storm-relative helicity,
//! bulk shear, LCL, lapse rates, freezing and wet-bulb-zero heights, inversions, boundary layer,
//! effective inflow layer, convective temperature, and surface / mixed-layer / most-unstable
//! parcels with our own parcel ascent. Also a time-height wind profile (VAD-style display).

use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Result};

const LEVELS: [u32; 14] = [1000, 975, 950, 925, 900, 850, 800, 700, 600, 500, 400, 300, 250, 200];

#[derive(Clone, Debug, Default)]
pub struct Level {
    pub p: f32,
    /// Height above ground, m.
    pub z: f32,
    pub t: f32,
    pub td: f32,
    /// Wind, m/s (u east, v north).
    pub u: f32,
    pub v: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Parcel {
    pub cape: f32,
    pub cin: f32,
    pub lcl_m: f32,
    pub lfc_m: Option<f32>,
}

#[derive(Clone, Debug, Default)]
pub struct Sounding {
    pub lat: f64,
    pub lon: f64,
    pub time: i64,
    pub levels: Vec<Level>,
    pub sb: Parcel,
    pub ml: Parcel,
    pub mu: Parcel,
    pub srh01: f32,
    pub srh03: f32,
    pub shear01: f32,
    pub shear06: f32,
    /// Bunkers right-mover storm motion, m/s.
    pub storm: (f32, f32),
    pub lapse_03: f32,
    pub lapse_75: f32,
    pub freezing_m: Option<f32>,
    pub wbz_m: Option<f32>,
    pub pbl_m: f32,
    /// Base heights (m) of layers where temperature rises with height.
    pub inversions: Vec<f32>,
    /// Effective inflow layer (bottom, top) m.
    pub inflow: Option<(f32, f32)>,
    /// Convective temperature, °F.
    pub conv_temp_f: Option<f32>,
    pub stp: f32,
    pub scp: f32,
    /// Time-height wind profile: per hour, (hour unix, per level (z m, u, v)).
    pub profile: Vec<(i64, Vec<(f32, f32, f32)>)>,
}

fn store() -> &'static Mutex<Option<Sounding>> {
    static S: OnceLock<Mutex<Option<Sounding>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

/// The newest sounding (for diagrams and bindings).
pub fn latest() -> Option<Sounding> {
    store().lock().ok().and_then(|s| s.clone())
}

pub fn set_latest(s: Sounding) {
    if let Ok(mut slot) = store().lock() {
        *slot = Some(s);
    }
}

// ---- Thermodynamics (°C, hPa) ----

fn es(t: f32) -> f32 {
    6.112 * (17.67 * t / (t + 243.5)).exp()
}
fn mixing_ratio(td: f32, p: f32) -> f32 {
    let e = es(td);
    0.622 * e / (p - e).max(1.0)
}
fn virtual_t(t: f32, td: f32, p: f32) -> f32 {
    let r = mixing_ratio(td, p);
    (t + 273.15) * (1.0 + 0.61 * r) - 273.15
}
fn theta_e(t: f32, td: f32, p: f32) -> f32 {
    let r = mixing_ratio(td, p);
    let tk = t + 273.15;
    tk * (1000.0 / p).powf(0.2854) * ((2.5e6 * r) / (1004.0 * tk)).exp()
}
/// Moist-adiabatic lapse rate, °C per m.
fn moist_lapse(t: f32, p: f32) -> f32 {
    let tk = t + 273.15;
    let r = mixing_ratio(t, p);
    let (g, lv, rd, cp) = (9.81f32, 2.5e6f32, 287.0f32, 1004.0f32);
    g * (1.0 + lv * r / (rd * tk)) / (cp + lv * lv * r * 0.622 / (rd * tk * tk))
}

/// Interpolate environment (t, td, p) at height z.
fn env_at(levels: &[Level], z: f32) -> Option<(f32, f32, f32, f32, f32)> {
    for w in levels.windows(2) {
        if z >= w[0].z && z <= w[1].z {
            let f = (z - w[0].z) / (w[1].z - w[0].z).max(1.0);
            let lerp = |a: f32, b: f32| a + (b - a) * f;
            return Some((lerp(w[0].t, w[1].t), lerp(w[0].td, w[1].td), lerp(w[0].p, w[1].p), lerp(w[0].u, w[1].u), lerp(w[0].v, w[1].v)));
        }
    }
    None
}

/// Lift a parcel from height z0 with (t, td): dry to the LCL, then moist. CAPE and CIN with the
/// virtual-temperature correction, integrated in 50 m steps to 12 km.
fn lift(levels: &[Level], z0: f32, t0: f32, td0: f32) -> Parcel {
    let lcl_m = (125.0 * (t0 - td0)).max(0.0);
    let (mut cape, mut cin) = (0.0f32, 0.0f32);
    let mut lfc = None;
    let mut tp = t0;
    let mut z = z0;
    let step = 50.0;
    while z < 12000.0 {
        let Some((te, tde, p, _, _)) = env_at(levels, z) else { break };
        let below_lcl = z - z0 < lcl_m;
        if below_lcl {
            tp -= 0.0098 * step;
        } else {
            tp -= moist_lapse(tp, p) * step;
        }
        let tdp = if below_lcl { td0 - 0.0018 * (z - z0) } else { tp };
        let tvp = virtual_t(tp, tdp, p);
        let tve = virtual_t(te, tde, p);
        let buoy = 9.81 * (tvp - tve) / (tve + 273.15) * step;
        if buoy > 0.0 {
            cape += buoy;
            if lfc.is_none() && !below_lcl {
                lfc = Some(z);
            }
        } else if lfc.is_none() {
            cin += buoy;
        }
        z += step;
    }
    Parcel { cape, cin, lcl_m, lfc_m: lfc }
}

fn wind_at(levels: &[Level], z: f32) -> (f32, f32) {
    env_at(levels, z).map(|(_, _, _, u, v)| (u, v)).unwrap_or((0.0, 0.0))
}

/// Storm-relative helicity from the surface to `top` m.
fn srh(levels: &[Level], storm: (f32, f32), top: f32) -> f32 {
    let mut total = 0.0;
    let mut z = levels.first().map(|l| l.z).unwrap_or(0.0);
    let step = 100.0;
    while z + step <= top {
        let (u1, v1) = wind_at(levels, z);
        let (u2, v2) = wind_at(levels, z + step);
        total += (u2 - storm.0) * (v1 - storm.1) - (u1 - storm.0) * (v2 - storm.1);
        z += step;
    }
    total
}

pub async fn fetch(lat: f64, lon: f64) -> Result<Sounding> {
    let mut vars = vec!["temperature_2m".to_string(), "dew_point_2m".into(), "wind_speed_10m".into(), "wind_direction_10m".into(), "surface_pressure".into(), "boundary_layer_height".into()];
    for p in LEVELS {
        for v in ["temperature", "dew_point", "wind_speed", "wind_direction", "geopotential_height"] {
            vars.push(format!("{v}_{p}hPa"));
        }
    }
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.3}&longitude={lon:.3}&hourly={}&forecast_hours=24&wind_speed_unit=ms&timeformat=unixtime&timezone=GMT&elevation=nan",
        vars.join(",")
    );
    let v: serde_json::Value = crate::net::get_logged(&url, std::time::Duration::from_secs(30)).await?.json().await?;
    let elevation = v.get("elevation").and_then(|e| e.as_f64()).unwrap_or(0.0) as f32;
    let series = |k: &str| -> Vec<f32> {
        v.pointer(&format!("/hourly/{k}")).and_then(|a| a.as_array()).map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN) as f32).collect()).unwrap_or_default()
    };
    let times: Vec<i64> = v.pointer("/hourly/time").and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
    if times.is_empty() {
        return Err(anyhow!("no sounding data"));
    }
    let uv = |speed: f32, dir: f32| {
        let r = dir.to_radians();
        (-speed * r.sin(), -speed * r.cos())
    };
    let column = |h: usize| -> Vec<Level> {
        let at = |k: &str| series(k).get(h).copied().unwrap_or(f32::NAN);
        let (u0, v0) = uv(at("wind_speed_10m"), at("wind_direction_10m"));
        let psfc = at("surface_pressure");
        let mut levels = vec![Level { p: psfc, z: 0.0, t: at("temperature_2m"), td: at("dew_point_2m"), u: u0, v: v0 }];
        for p in LEVELS {
            if (p as f32) >= psfc {
                continue;
            }
            let (u, vv) = uv(at(&format!("wind_speed_{p}hPa")), at(&format!("wind_direction_{p}hPa")));
            let z = at(&format!("geopotential_height_{p}hPa")) - elevation;
            let l = Level { p: p as f32, z, t: at(&format!("temperature_{p}hPa")), td: at(&format!("dew_point_{p}hPa")), u, v: vv };
            if l.z.is_finite() && l.t.is_finite() && l.z > levels.last().map(|x| x.z).unwrap_or(0.0) {
                levels.push(l);
            }
        }
        levels
    };
    // "Now" is the hour nearest the current time.
    let now = chrono::Utc::now().timestamp();
    let h = times.iter().enumerate().min_by_key(|(_, t)| (**t - now).abs()).map(|(i, _)| i).unwrap_or(0);
    let levels = column(h);
    if levels.len() < 6 {
        return Err(anyhow!("sounding has too few levels"));
    }
    let mut s = analyse(levels, lat, lon, times[h]);
    s.pbl_m = series("boundary_layer_height").get(h).copied().unwrap_or(0.0);
    s.profile = (0..times.len()).map(|k| (times[k], column(k).iter().filter(|l| l.z <= 9000.0).map(|l| (l.z, l.u, l.v)).collect())).collect();
    Ok(s)
}

pub fn analyse(levels: Vec<Level>, lat: f64, lon: f64, time: i64) -> Sounding {
    let sfc = levels[0].clone();
    let sb = lift(&levels, 0.0, sfc.t, sfc.td);
    // Mixed layer: mean temperature / dew point of the lowest 100 hPa.
    let ml_levels: Vec<&Level> = levels.iter().filter(|l| l.p >= sfc.p - 100.0).collect();
    let n = ml_levels.len().max(1) as f32;
    let ml = lift(&levels, 0.0, ml_levels.iter().map(|l| l.t).sum::<f32>() / n, ml_levels.iter().map(|l| l.td).sum::<f32>() / n);
    // Most unstable: highest theta-e in the lowest 300 hPa.
    let mu_src = levels.iter().filter(|l| l.p >= sfc.p - 300.0).max_by(|a, b| theta_e(a.t, a.td, a.p).total_cmp(&theta_e(b.t, b.td, b.p))).cloned().unwrap_or(sfc.clone());
    let mu = lift(&levels, mu_src.z, mu_src.t, mu_src.td);
    // Bunkers right-mover: mean 0-6 km wind plus 7.5 m/s to the right of the shear.
    let mean = {
        let (mut su, mut sv, mut k) = (0.0, 0.0, 0.0);
        let mut z = 0.0;
        while z <= 6000.0 {
            let (u, v) = wind_at(&levels, z);
            su += u;
            sv += v;
            k += 1.0;
            z += 250.0;
        }
        (su / k, sv / k)
    };
    let (u6, v6) = wind_at(&levels, 6000.0);
    let (u0, v0) = (sfc.u, sfc.v);
    let (su, sv) = (u6 - u0, v6 - v0);
    let mag = (su * su + sv * sv).sqrt().max(0.1);
    let storm = (mean.0 + 7.5 * sv / mag, mean.1 - 7.5 * su / mag);
    let (u1, v1) = wind_at(&levels, 1000.0);
    let t_at = |z: f32| env_at(&levels, z).map(|e| e.0);
    let lapse_03 = match t_at(3000.0) { Some(t3) => (sfc.t - t3) / 3.0, None => 0.0 };
    let z700 = levels.iter().find(|l| l.p as u32 == 700).map(|l| (l.z, l.t));
    let z500 = levels.iter().find(|l| l.p as u32 == 500).map(|l| (l.z, l.t));
    let lapse_75 = match (z700, z500) { (Some(a), Some(b)) => (a.1 - b.1) / ((b.0 - a.0) / 1000.0).max(0.5), _ => 0.0 };
    let crossing = |f: &dyn Fn(&Level) -> f32| -> Option<f32> {
        levels.windows(2).find(|w| f(&w[0]) > 0.0 && f(&w[1]) <= 0.0).map(|w| {
            let (a, b) = (f(&w[0]), f(&w[1]));
            w[0].z + (w[1].z - w[0].z) * a / (a - b)
        })
    };
    let freezing_m = crossing(&|l: &Level| l.t);
    let wet_bulb = |l: &Level| {
        let rh = (100.0 * es(l.td) / es(l.t)).clamp(1.0, 100.0);
        l.t * (0.151977 * (rh + 8.313659).sqrt()).atan() + (l.t + rh).atan() - (rh - 1.676331).atan() + 0.00391838 * rh.powf(1.5) * (0.023101 * rh).atan() - 4.686035
    };
    let wbz_m = crossing(&wet_bulb);
    let inversions = levels.windows(2).filter(|w| w[1].t > w[0].t + 0.3 && w[0].z < 5000.0).map(|w| w[0].z).collect();
    // Effective inflow layer: contiguous levels whose parcels have CAPE >= 100 and CIN > -250.
    let mut inflow: Option<(f32, f32)> = None;
    for l in levels.iter().filter(|l| l.z < 4000.0) {
        let p = lift(&levels, l.z, l.t, l.td);
        if p.cape >= 100.0 && p.cin > -250.0 {
            inflow = Some(match inflow { None => (l.z, l.z), Some((b, _)) => (b, l.z) });
        } else if inflow.is_some() {
            break;
        }
    }
    // Convective temperature: where the surface mixing-ratio line meets the environment (CCL),
    // brought dry-adiabatically back down.
    let w_sfc = mixing_ratio(sfc.td, sfc.p);
    let conv_temp_f = levels.windows(2).find_map(|w| {
        let td_line = |p: f32| {
            let e = w_sfc * p / (0.622 + w_sfc);
            let a = (e / 6.112).ln();
            243.5 * a / (17.67 - a)
        };
        let (d0, d1) = (w[0].t - td_line(w[0].p), w[1].t - td_line(w[1].p));
        (d0 > 0.0 && d1 <= 0.0).then(|| {
            let f = d0 / (d0 - d1);
            let p = w[0].p + (w[1].p - w[0].p) * f;
            let t = w[0].t + (w[1].t - w[0].t) * f;
            let tc = (t + 273.15) * (sfc.p / p).powf(0.2854) - 273.15;
            tc * 9.0 / 5.0 + 32.0
        })
    });
    let srh01 = srh(&levels, storm, 1000.0);
    let srh03 = srh(&levels, storm, 3000.0);
    let shear06 = mag;
    let shear01 = ((u1 - u0).powi(2) + (v1 - v0).powi(2)).sqrt();
    let shear_term = if shear06 < 12.5 { 0.0 } else { (shear06 / 20.0).min(1.5) };
    let lcl_term = ((2000.0 - ml.lcl_m) / 1000.0).clamp(0.0, 1.0);
    let cin_term = if ml.cin > -50.0 { 1.0 } else if ml.cin < -200.0 { 0.0 } else { (200.0 + ml.cin) / 150.0 };
    let stp = (ml.cape / 1500.0) * lcl_term * (srh01 / 150.0).max(0.0) * shear_term * cin_term;
    let scp = (mu.cape / 1000.0) * (srh03 / 50.0).max(0.0) * shear_term;
    Sounding {
        lat,
        lon,
        time,
        sb,
        ml,
        mu,
        srh01,
        srh03,
        shear01,
        shear06,
        storm,
        lapse_03,
        lapse_75,
        freezing_m,
        wbz_m,
        pbl_m: 0.0,
        inversions,
        inflow,
        conv_temp_f,
        stp,
        scp,
        profile: Vec::new(),
        levels,
    }
}

/// Bindings: snd.* for graphics.
pub fn bind(b: &mut crate::gfx::bind::Bindings) {
    let Some(s) = latest() else { return };
    let ft = |m: f32| format!("{:.0} ft", (m * 3.281 / 100.0).round() * 100.0);
    let kt = |ms: f32| format!("{:.0} kt", ms * 1.944);
    b.set("snd.sbcape", &format!("{:.0} J/kg", s.sb.cape));
    b.set("snd.mlcape", &format!("{:.0} J/kg", s.ml.cape));
    b.set("snd.mucape", &format!("{:.0} J/kg", s.mu.cape));
    b.set("snd.mlcin", &format!("{:.0} J/kg", s.ml.cin));
    b.set("snd.lcl", &ft(s.ml.lcl_m));
    b.set("snd.srh01", &format!("{:.0} m²/s²", s.srh01));
    b.set("snd.srh03", &format!("{:.0} m²/s²", s.srh03));
    b.set("snd.shear01", &kt(s.shear01));
    b.set("snd.shear06", &kt(s.shear06));
    b.set("snd.stp", &format!("{:.1}", s.stp));
    b.set("snd.scp", &format!("{:.1}", s.scp));
    b.set("snd.lapse03", &format!("{:.1} °C/km", s.lapse_03));
    b.set("snd.lapse75", &format!("{:.1} °C/km", s.lapse_75));
    b.set("snd.freezing", &s.freezing_m.map(ft).unwrap_or_else(|| "Surface".into()));
    b.set("snd.wbz", &s.wbz_m.map(ft).unwrap_or_else(|| "Surface".into()));
    b.set("snd.pbl", &ft(s.pbl_m));
    b.set("snd.inversion", &s.inversions.first().map(|z| format!("Inversion at {}", ft(*z))).unwrap_or_else(|| "No low inversion".into()));
    b.set("snd.inflow", &s.inflow.map(|(a, c)| format!("{} to {}", ft(a), ft(c))).unwrap_or_else(|| "None (elevated or stable)".into()));
    b.set("snd.convtemp", &s.conv_temp_f.map(|t| format!("{t:.0}°F")).unwrap_or_else(|| "—".into()));
    let (su, sv) = s.storm;
    let sdir = (su.atan2(sv).to_degrees() + 180.0).rem_euclid(360.0);
    b.set("snd.storm_motion", &format!("Toward {} at {:.0} mph", crate::map::geo::compass(((sdir + 180.0) % 360.0) as f64), (su * su + sv * sv).sqrt() * 2.237));
}
