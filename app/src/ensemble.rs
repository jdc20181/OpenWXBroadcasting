//! Ensemble forecasts (Open-Meteo ensemble API, GFS 31 members) for home and the largest towns
//! in the coverage area: spaghetti and plume charts, mean / spread / percentile / probability
//! maps at towns, confidence by day, and best / likely / worst-case scenarios.

use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Result};

#[derive(Clone, Debug, Default)]
pub struct TownEnsemble {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    /// [member][hour]
    pub temp: Vec<Vec<f32>>,
    pub precip: Vec<Vec<f32>>,
    pub snow: Vec<Vec<f32>>,
}

#[derive(Clone, Debug, Default)]
pub struct Ensemble {
    pub times: Vec<i64>,
    /// Home first.
    pub towns: Vec<TownEnsemble>,
}

fn store() -> &'static Mutex<Option<Ensemble>> {
    static S: OnceLock<Mutex<Option<Ensemble>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(None))
}

pub fn latest() -> Option<Ensemble> {
    store().lock().ok().and_then(|s| s.clone())
}

pub fn set_latest(e: Ensemble) {
    if let Ok(mut s) = store().lock() {
        *s = Some(e);
    }
}

pub async fn fetch(points: Vec<(String, f64, f64)>) -> Result<Ensemble> {
    let lats = points.iter().map(|p| format!("{:.3}", p.1)).collect::<Vec<_>>().join(",");
    let lons = points.iter().map(|p| format!("{:.3}", p.2)).collect::<Vec<_>>().join(",");
    let url = format!(
        "https://ensemble-api.open-meteo.com/v1/ensemble?latitude={lats}&longitude={lons}&hourly=temperature_2m,precipitation,snowfall&models=gfs_seamless&forecast_days=7&temperature_unit=fahrenheit&precipitation_unit=inch&timeformat=unixtime&timezone=GMT"
    );
    let v: serde_json::Value = crate::net::get_logged(&url, std::time::Duration::from_secs(40)).await?.json().await?;
    let locs: Vec<serde_json::Value> = match v {
        serde_json::Value::Array(a) => a,
        other => vec![other],
    };
    if locs.len() != points.len() {
        return Err(anyhow!("ensemble: {} of {} places", locs.len(), points.len()));
    }
    let times: Vec<i64> = locs[0].pointer("/hourly/time").and_then(|t| t.as_array()).map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default();
    let members = |loc: &serde_json::Value, var: &str| -> Vec<Vec<f32>> {
        let Some(h) = loc.get("hourly").and_then(|h| h.as_object()) else { return Vec::new() };
        let mut keys: Vec<&String> = h.keys().filter(|k| *k == var || k.starts_with(&format!("{var}_member"))).collect();
        keys.sort();
        keys.iter()
            .filter_map(|k| h.get(*k).and_then(|a| a.as_array()))
            .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN) as f32).collect())
            .collect()
    };
    let towns = points
        .into_iter()
        .zip(locs.iter())
        .map(|((name, lat, lon), loc)| TownEnsemble { name, lat, lon, temp: members(loc, "temperature_2m"), precip: members(loc, "precipitation"), snow: members(loc, "snowfall") })
        .collect();
    Ok(Ensemble { times, towns })
}

/// Percentile (0-100) of a set of values.
pub fn percentile(values: &[f32], p: f32) -> f32 {
    let mut v: Vec<f32> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return f32::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let idx = (p / 100.0 * (v.len() - 1) as f32).round() as usize;
    v[idx.min(v.len() - 1)]
}

impl TownEnsemble {
    /// Each member's total over the first `hours`.
    pub fn totals(series: &[Vec<f32>], hours: usize) -> Vec<f32> {
        series.iter().map(|m| m.iter().take(hours).filter(|x| x.is_finite()).sum()).collect()
    }

    /// Each member's daily high for day `d` (0 = today, 24-hour blocks).
    pub fn day_highs(&self, d: usize) -> Vec<f32> {
        self.temp.iter().map(|m| m.iter().skip(d * 24).take(24).copied().filter(|x| x.is_finite()).fold(f32::MIN, f32::max)).filter(|x| *x > -200.0).collect()
    }
}

/// Map value at a town for an ensemble map mode.
pub fn town_value(t: &TownEnsemble, mode: &str) -> Option<(f32, String)> {
    let highs = t.day_highs(1);
    let snow = TownEnsemble::totals(&t.snow, 72);
    let rain = TownEnsemble::totals(&t.precip, 72);
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
    let spread = |v: &[f32]| {
        let m = mean(v);
        (v.iter().map(|x| (x - m).powi(2)).sum::<f32>() / v.len().max(1) as f32).sqrt()
    };
    let prob = |v: &[f32], th: f32| v.iter().filter(|x| **x >= th).count() as f32 / v.len().max(1) as f32 * 100.0;
    Some(match mode {
        "mean_high" => (mean(&highs), format!("{:.0}°", mean(&highs))),
        "spread_high" => (spread(&highs), format!("±{:.0}°", spread(&highs))),
        "p10_snow" => (percentile(&snow, 10.0), format!("{:.1}\"", percentile(&snow, 10.0))),
        "p90_snow" => (percentile(&snow, 90.0), format!("{:.1}\"", percentile(&snow, 90.0))),
        "prob_snow1" => (prob(&snow, 1.0), format!("{:.0}%", prob(&snow, 1.0))),
        "prob_rain1" => (prob(&rain, 1.0), format!("{:.0}%", prob(&rain, 1.0))),
        "prob_freeze" => {
            let lows: Vec<f32> = t.temp.iter().map(|m| m.iter().take(48).copied().fold(f32::MAX, f32::min)).collect();
            let p = lows.iter().filter(|x| **x <= 32.0).count() as f32 / lows.len().max(1) as f32 * 100.0;
            (p, format!("{p:.0}%"))
        }
        _ => return None,
    })
}

/// Bindings and chart series from the home ensemble.
pub fn bind(b: &mut crate::gfx::bind::Bindings) {
    let Some(e) = latest() else { return };
    let Some(home) = e.towns.first() else { return };
    let label = |i: usize| e.times.get(i).and_then(|t| chrono::DateTime::from_timestamp(*t, 0)).map(|t| t.with_timezone(&chrono::Local).format("%a %-I%p").to_string()).unwrap_or_default();
    // Every 6 hours for 7 days: spaghetti members, mean and percentiles for plumes.
    let idx: Vec<usize> = (0..e.times.len()).step_by(6).collect();
    for (m, member) in home.temp.iter().enumerate() {
        b.set_series(&format!("ens.temp.m{m:02}"), idx.iter().map(|i| (label(*i), member.get(*i).copied().unwrap_or(f32::NAN))).collect());
    }
    for (name, p) in [("p10", 10.0), ("p25", 25.0), ("p50", 50.0), ("p75", 75.0), ("p90", 90.0)] {
        b.set_series(&format!("ens.temp.{name}"), idx.iter().map(|i| (label(*i), percentile(&home.temp.iter().filter_map(|m| m.get(*i).copied()).collect::<Vec<_>>(), p))).collect());
    }
    b.set_series("ens.temp", idx.iter().map(|i| {
        let v: Vec<f32> = home.temp.iter().filter_map(|m| m.get(*i).copied()).filter(|x| x.is_finite()).collect();
        (label(*i), v.iter().sum::<f32>() / v.len().max(1) as f32)
    }).collect());
    let snow = TownEnsemble::totals(&home.snow, 72);
    let rain = TownEnsemble::totals(&home.precip, 72);
    b.set_series("ens.snow72", snow.iter().enumerate().map(|(i, v)| (format!("{i}"), *v)).collect());
    b.set_series("ens.rain72", rain.iter().enumerate().map(|(i, v)| (format!("{i}"), *v)).collect());
    b.set("ens.snow_range", &format!("{:.1}\" to {:.1}\" (most likely {:.1}\")", percentile(&snow, 10.0), percentile(&snow, 90.0), percentile(&snow, 50.0)));
    b.set("ens.rain_range", &format!("{:.2}\" to {:.2}\" (most likely {:.2}\")", percentile(&rain, 10.0), percentile(&rain, 90.0), percentile(&rain, 50.0)));
    b.set("ens.best", &format!("{:.1}\" snow, {:.2}\" rain", percentile(&snow, 10.0), percentile(&rain, 10.0)));
    b.set("ens.likely", &format!("{:.1}\" snow, {:.2}\" rain", percentile(&snow, 50.0), percentile(&rain, 50.0)));
    b.set("ens.worst", &format!("{:.1}\" snow, {:.2}\" rain", percentile(&snow, 90.0), percentile(&rain, 90.0)));
    let mut conf = Vec::new();
    for d in 0..7 {
        let highs = home.day_highs(d);
        if highs.is_empty() {
            continue;
        }
        let m = highs.iter().sum::<f32>() / highs.len() as f32;
        let sd = (highs.iter().map(|x| (x - m).powi(2)).sum::<f32>() / highs.len() as f32).sqrt();
        let c = (100.0 - sd * 8.0).clamp(10.0, 98.0);
        b.set(&format!("ens.day{d}.confidence"), &format!("{c:.0}%"));
        b.set(&format!("ens.day{d}.range"), &format!("{:.0}° to {:.0}°", percentile(&highs, 10.0), percentile(&highs, 90.0)));
        let day = e.times.get(d * 24).and_then(|t| chrono::DateTime::from_timestamp(*t, 0)).map(|t| t.with_timezone(&chrono::Local).format("%a").to_string()).unwrap_or_default();
        conf.push((day, c));
    }
    b.set_series("ens.confidence", conf);
}
