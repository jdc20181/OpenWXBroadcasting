//! Gridded model data for the data layers: temperature, feels like, dew point, humidity, wind,
//! gusts, pressure (isobars), cloud cover, rain/snow totals and the Futurecast (model
//! precipitation by type). GFS (the default) is read straight from NOAA's GRIB2 files
//! (`grid_noaa.rs`); the other models still come from Open-Meteo's multi-point API. Fetched as a
//! lat/lon grid over the current map view.
//!
//! Values are bilinear in space and linear in time, so the Futurecast and data loops tween
//! smoothly between model hours.

use anyhow::Result;

use crate::{map::scene::DataField, net};

#[path = "grid_noaa.rs"]
mod noaa;
pub use noaa::{fetch_box, fits as noaa_fits, progress as noaa_progress};

/// Grid resolution over the view. Keep the number of requested locations at 60.
pub const NX: usize = 10;
pub const NY: usize = 6;
pub const FORECAST_SPANS: [u32; 6] = [12, 24, 48, 72, 120, 168];
pub const MAX_FORECAST_HOURS: u32 = 168;
const HOURS: u32 = MAX_FORECAST_HOURS + 2;
const AIR_HOURS: u32 = 96;
const CHUNK: usize = 200;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Var {
    Temp = 0,
    Feels,
    Dew,
    Rh,
    Wind,
    U,
    V,
    Gust,
    Pressure,
    Cloud,
    Precip,
    Snow,
    Code,
    RainAcc,
    SnowAcc,
    // Severe / winter ingredients (filled only when the grid is fetched with `extended`).
    Cape,
    T850,
    Td850,
    T700,
    T500,
    U850,
    V850,
    U500,
    V500,
    Omega700,
    Z850,
    Z500,
    FreezingLevel,
    SoilTemp,
    Aqi,
    Pm25,
    SoilMoist,
    /// Snow accumulated at a fixed 10:1 snow-to-liquid ratio, inches.
    Snow10Acc,
    /// Snow accumulated with the Kuchera ratio (needs the upper-air variables), inches.
    KucheraAcc,
}
const VARS: usize = 34;

/// Which forecast model fills the grid. GFS is read from NOAA directly; the others come from
/// Open-Meteo (`models=`), where Blend lets Open-Meteo pick the best model per place and hour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GridModel {
    Blend,
    #[default]
    Gfs,
    Nam,
    Hrrr,
    Nbm,
    Ecmwf,
    Icon,
    Gem,
}

impl GridModel {
    /// The models the map offers: the ones read from NOAA's own files. The others stay in the
    /// enum so older settings still load; they show NOAA GFS.
    pub const ALL: [GridModel; 2] = [Self::Hrrr, Self::Gfs];

    pub fn label(self) -> &'static str {
        match self {
            Self::Blend => "Best blend",
            Self::Gfs => "GFS",
            Self::Nam => "NAM",
            Self::Hrrr => "HRRR",
            Self::Nbm => "NBM",
            Self::Ecmwf => "ECMWF",
            Self::Icon => "ICON",
            Self::Gem => "GEM (Canadian)",
        }
    }

    /// One-line description for the picker.
    /// A menu-length description: what it is and where it comes from.
    pub fn short_about(self) -> &'static str {
        match self {
            Self::Blend => "best model per place · Open-Meteo",
            Self::Gfs => "global, 7 days · NOAA",
            Self::Nam => "12 km, 2.5 days · Open-Meteo",
            Self::Hrrr => "3 km, 48 hours · NOAA",
            Self::Nbm => "NOAA blend · Open-Meteo",
            Self::Ecmwf => "European global · Open-Meteo",
            Self::Icon => "German global · Open-Meteo",
            Self::Gem => "Canadian global · Open-Meteo",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Self::Blend => "Open-Meteo picks the best model for each place and hour (Open-Meteo)",
            Self::Gfs => "NOAA global model, read directly from NOAA, 7 days",
            Self::Nam => "NOAA North American model, 12 km, about 2.5 days (Open-Meteo)",
            Self::Hrrr => "NOAA high-resolution rapid refresh, 3 km, 48 hours, read directly from NOAA (state- and region-size views; wider views show NOAA GFS)",
            Self::Nbm => "NOAA National Blend of Models (Open-Meteo)",
            Self::Ecmwf => "European global model, IFS 0.25° (Open-Meteo)",
            Self::Icon => "German global model, DWD (Open-Meteo)",
            Self::Gem => "Canadian global model (Open-Meteo)",
        }
    }

    /// Read straight from NOAA's GRIB files (not Open-Meteo).
    pub fn is_noaa(self) -> bool {
        matches!(self, Self::Gfs | Self::Hrrr)
    }

    /// Open-Meteo  id (None: best blend).
    pub fn api_id_pub(self) -> Option<&'static str> {
        self.api_id()
    }

    fn api_id(self) -> Option<&'static str> {
        match self {
            Self::Blend => None,
            Self::Gfs => Some("gfs_seamless"),
            Self::Nam => Some("ncep_nam_conus"),
            Self::Hrrr => Some("ncep_hrrr_conus"),
            Self::Nbm => Some("ncep_nbm_conus"),
            Self::Ecmwf => Some("ecmwf_ifs025"),
            Self::Icon => Some("icon_seamless"),
            Self::Gem => Some("gem_seamless"),
        }
    }

    /// Hours the model reaches (the grid stops where the model's data stops).
    pub fn max_hours(self) -> u32 {
        match self {
            Self::Hrrr => 48,
            Self::Nam => 60,
            _ => MAX_FORECAST_HOURS,
        }
    }

    /// The NOAA GRIB model that matches, for products drawn from the full-resolution files.
    pub fn noaa_source(self) -> Option<crate::futurecast::Source> {
        match self {
            Self::Gfs => Some(crate::futurecast::Source::Gfs),
            Self::Hrrr => Some(crate::futurecast::Source::Hrrr),
            Self::Nbm => Some(crate::futurecast::Source::Nbm),
            _ => None,
        }
    }
}

/// Kuchera snow-to-liquid ratio from the warmest of the surface, 850 and 700 mb temperatures (°F).
pub fn kuchera_ratio(t_sfc: f32, t850: f32, t700: f32) -> f32 {
    let tmax_k = [t_sfc, t850, t700].into_iter().fold(f32::MIN, f32::max) * 5.0 / 9.0 + 255.372;
    if tmax_k > 271.16 { 12.0 + 2.0 * (271.16 - tmax_k) } else { 12.0 + (271.16 - tmax_k) }.clamp(3.0, 30.0)
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrecipKind {
    Rain,
    Mix,
    Snow,
}

#[derive(Clone, Debug)]
pub struct WxGrid {
    pub north: f64,
    pub south: f64,
    pub west: f64,
    pub east: f64,
    pub nx: usize,
    pub ny: usize,
    /// Unix seconds of each model hour.
    pub times: Vec<i64>,
    data: Vec<Vec<f32>>,
    pub fetched_at: i64,
    /// Upper-air ingredients present (Settings > Data > severe and winter ingredients).
    pub extended: bool,
    /// Air-quality values present (Settings > Data > air quality).
    pub air: bool,
    /// Zoom the grid was fetched for (refetch when the view zooms far in/out).
    pub zoom: f64,
    /// Model the grid came from.
    pub model: GridModel,
    /// Fields computed once per fetch at the grid nodes: highs, lows, 24 h change, peak gust, ice.
    derived: [std::sync::OnceLock<Vec<f32>>; 5],
    /// Mesoscale analysis fields at the grid nodes, cached per (field, model hour).
    analysis: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<(DataField, usize), std::sync::Arc<Vec<f32>>>>>,
}

/// Bounds (north, south, west, east) of a view, padded by `pad` of its size.
pub fn view_bounds(view: crate::config::GeoView, pad: f64) -> (f64, f64, f64, f64) {
    let vp = crate::map::geo::Viewport::new(view, crate::config::FRAME_W, crate::config::FRAME_H);
    let (n, w) = vp.to_geo(0.0, 0.0);
    let (s, e) = vp.to_geo(crate::config::FRAME_W as f64, crate::config::FRAME_H as f64);
    let dlat = (n - s) * pad;
    let dlon = (e - w) * pad;
    ((n + dlat).min(84.0), (s - dlat).max(-84.0), w - dlon, e + dlon)
}

impl WxGrid {
    /// Share of grid values that are missing or not finite (integrity check).
    pub fn invalid_fraction(&self) -> f32 {
        let (mut bad, mut total) = (0usize, 0usize);
        // A field the model does not carry at all (soil on HRRR) is not a gap.
        for layer in self.data.iter().filter(|l| l.iter().any(|v| v.is_finite())) {
            total += layer.len();
            bad += layer.iter().filter(|v| !v.is_finite()).count();
        }
        if total == 0 { 1.0 } else { bad as f32 / total as f32 }
    }

    fn at(&self, var: Var, t: usize, j: usize, i: usize) -> f32 {
        self.data[var as usize][(t * self.ny + j) * self.nx + i]
    }

    pub fn hours(&self) -> usize {
        self.times.len()
    }

    pub fn available_hours(&self, field: Option<DataField>) -> u32 {
        let mut end = self.times.last().copied().unwrap_or_default();
        let air_variable = match field {
            Some(DataField::AirQuality) => Some(Var::Aqi),
            Some(DataField::Smoke | DataField::SmokeVisibility) => Some(Var::Pm25),
            _ => None,
        };
        if let Some(var) = air_variable {
            if !self.air { return 0; }
            let cells = self.nx * self.ny;
            let Some(last) = self.data[var as usize].chunks(cells).rposition(|hour| hour.iter().any(|value| value.is_finite())) else {
                return 0;
            };
            end = self.times[last];
        }
        ((end - chrono::Utc::now().timestamp()).max(0) / 3600) as u32
    }

    /// Fractional time index for "now + offset hours".
    pub fn time_index(&self, offset_hours: f32) -> f32 {
        if self.times.is_empty() {
            return 0.0;
        }
        let now = chrono::Utc::now().timestamp();
        let base = (now - self.times[0]) as f32 / 3600.0;
        (base + offset_hours).clamp(0.0, (self.times.len() - 1) as f32)
    }

    /// Unix time for a fractional time index.
    pub fn time_at(&self, tf: f32) -> i64 {
        let Some(first) = self.times.first() else {
            return chrono::Utc::now().timestamp();
        };
        first + (tf * 3600.0) as i64
    }

    /// Whether the grid still covers `view` well enough to keep using it.
    pub fn covers(&self, view: crate::config::GeoView) -> bool {
        let (n, s, w, e) = view_bounds(view, 0.0);
        n <= self.north && s >= self.south && w >= self.west && e <= self.east && (view.zoom - self.zoom).abs() < 1.6
    }

    fn grid_pos(&self, lat: f64, lon: f64) -> Option<(f32, f32)> {
        let gx = (lon - self.west) / (self.east - self.west) * (self.nx - 1) as f64;
        let gy = (self.north - lat) / (self.north - self.south) * (self.ny - 1) as f64;
        if gx < -0.5 || gy < -0.5 || gx > self.nx as f64 - 0.5 || gy > self.ny as f64 - 0.5 {
            return None;
        }
        Some((gx.clamp(0.0, (self.nx - 1) as f64) as f32, gy.clamp(0.0, (self.ny - 1) as f64) as f32))
    }

    fn bilinear(&self, var: Var, t: usize, gx: f32, gy: f32) -> f32 {
        let i0 = gx.floor() as usize;
        let j0 = gy.floor() as usize;
        let i1 = (i0 + 1).min(self.nx - 1);
        let j1 = (j0 + 1).min(self.ny - 1);
        let fx = gx - i0 as f32;
        let fy = gy - j0 as f32;
        let a = self.at(var, t, j0, i0) * (1.0 - fx) + self.at(var, t, j0, i1) * fx;
        let b = self.at(var, t, j1, i0) * (1.0 - fx) + self.at(var, t, j1, i1) * fx;
        a * (1.0 - fy) + b * fy
    }

    /// Variable at fractional time index `tf` and a point.
    pub fn sample(&self, var: Var, tf: f32, lat: f64, lon: f64) -> Option<f32> {
        let (gx, gy) = self.grid_pos(lat, lon)?;
        let t0 = tf.floor().max(0.0) as usize;
        let t0 = t0.min(self.hours() - 1);
        let t1 = (t0 + 1).min(self.hours() - 1);
        let ft = tf - t0 as f32;
        let a = self.bilinear(var, t0, gx, gy);
        let b = self.bilinear(var, t1, gx, gy);
        Some(a * (1.0 - ft) + b * ft)
    }

    /// Nearest-cell value (weather codes).
    fn nearest(&self, var: Var, tf: f32, lat: f64, lon: f64) -> Option<f32> {
        let (gx, gy) = self.grid_pos(lat, lon)?;
        let t = (tf.round().max(0.0) as usize).min(self.hours() - 1);
        Some(self.at(var, t, gy.round() as usize, gx.round() as usize))
    }

    /// Node values of a derived field (computed on first use, then cached).
    fn derived_nodes(&self, field: DataField) -> &[f32] {
        let slot = match field {
            DataField::HighTemp => 0,
            DataField::LowTemp => 1,
            DataField::TempChange => 2,
            DataField::MaxGust => 3,
            _ => 4,
        };
        self.derived[slot].get_or_init(|| {
            let now = self.time_index(0.0);
            let hours: Vec<f32> = (0..=24).map(|h| (now + h as f32).min((self.hours() - 1) as f32)).collect();
            let mut out = vec![0.0; self.nx * self.ny];
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let temps = || hours.iter().map(|tf| self.node(Var::Temp, *tf, j, i));
                    out[j * self.nx + i] = match field {
                        DataField::HighTemp => temps().fold(f32::MIN, f32::max),
                        DataField::LowTemp => temps().fold(f32::MAX, f32::min),
                        DataField::TempChange => self.node(Var::Temp, *hours.last().unwrap_or(&now), j, i) - self.node(Var::Temp, now, j, i),
                        DataField::MaxGust => hours.iter().map(|tf| self.node(Var::Gust, *tf, j, i)).fold(0.0, f32::max),
                        _ => {
                            // Liquid falling as freezing rain / mix over 48 h.
                            let (lat, lon) = self.node_geo(j, i);
                            (0..48)
                                .filter_map(|h| {
                                    let tf = (now + h as f32).min((self.hours() - 1) as f32);
                                    match self.precip(tf, lat, lon) {
                                        Some((rate, PrecipKind::Mix)) => Some(rate),
                                        _ => None,
                                    }
                                })
                                .sum()
                        }
                    };
                }
            }
            out
        })
    }

    /// Fields that need the upper-air ingredients (Settings > severe and winter ingredients).
    pub fn needs_extended(field: DataField) -> bool {
        !matches!(field, DataField::TempAdvection | DataField::Frontogenesis | DataField::SnowChance | DataField::RoadHazard | DataField::FogRisk | DataField::ColdPool | DataField::FireDanger | DataField::RainRateExceed | DataField::AirQuality | DataField::Smoke | DataField::SmokeVisibility)
            && Self::is_analysis(field)
            && !matches!(field, DataField::Convergence | DataField::MoistureConvergence | DataField::ThetaE | DataField::ThetaEAdvection | DataField::PressureAnomaly | DataField::Dryline | DataField::Boundaries | DataField::DiffHeating)
    }

    /// True for the mesoscale analysis fields (computed per model hour from finite differences).
    pub fn is_analysis(field: DataField) -> bool {
        matches!(
            field,
            DataField::Convergence
                | DataField::MoistureConvergence
                | DataField::ThetaE
                | DataField::ThetaEAdvection
                | DataField::PressureAnomaly
                | DataField::Dryline
                | DataField::Boundaries
                | DataField::DiffHeating
                | DataField::ColdPool
                | DataField::SnowChance
                | DataField::RoadHazard
                | DataField::FogRisk
                | DataField::SoilMoisture
                | DataField::FloodGuidance
                | DataField::FloodRatio
                | DataField::FloodPotential
                | DataField::RainRateExceed
                | DataField::AirQuality
                | DataField::Smoke
                | DataField::SmokeVisibility
                | DataField::FireDanger
                | DataField::Cape | DataField::Stp | DataField::Scp | DataField::LapseRate | DataField::Lcl | DataField::WetBulbZero | DataField::FreezingLevel | DataField::Omega | DataField::QVector | DataField::TempAdvection | DataField::Frontogenesis | DataField::PrecipType | DataField::IceAccretion | DataField::SnowRate | DataField::SnowRatio | DataField::LakeEffect | DataField::GroundTemp | DataField::SnowSquall
        )
    }

    /// Analysis field nodes at model hour `t`.
    fn analysis_nodes(&self, field: DataField, t: usize) -> std::sync::Arc<Vec<f32>> {
        if let Some(v) = self.analysis.lock().ok().and_then(|m| m.get(&(field, t)).cloned()) {
            return v;
        }
        let (nx, ny) = (self.nx, self.ny);
        let at = |var: Var, j: usize, i: usize| self.at(var, t, j, i);
        // Node spacing in metres (x varies with latitude).
        let dy_m = (self.north - self.south) / (ny - 1) as f64 * 111_000.0;
        let mid = (self.north + self.south) / 2.0;
        let dx_m = (self.east - self.west) / (nx - 1) as f64 * 111_000.0 * mid.to_radians().cos();
        // Centred differences with one-sided ones at the edges. d/dx is eastward, d/dy northward
        // (row j grows southward).
        let ddx = |f: &dyn Fn(usize, usize) -> f32, j: usize, i: usize| -> f32 {
            let (a, b, n) = if i == 0 { (1, 0, 1.0) } else if i == nx - 1 { (i, i - 1, 1.0) } else { (i + 1, i - 1, 2.0) };
            let (a, b) = if i == 0 { (a, b) } else { (a, b) };
            (f(j, a) - f(j, b)) / (n * dx_m as f32)
        };
        let ddy = |f: &dyn Fn(usize, usize) -> f32, j: usize, i: usize| -> f32 {
            let (a, b, n) = if j == 0 { (0, 1, 1.0) } else if j == ny - 1 { (j - 1, j, 1.0) } else { (j - 1, j + 1, 2.0) };
            (f(a, i) - f(b, i)) / (n * dy_m as f32)
        };
        // m/s winds from mph components.
        let u = |j: usize, i: usize| at(Var::U, j, i) * 0.44704;
        let v = |j: usize, i: usize| at(Var::V, j, i) * 0.44704;
        let theta_e = |j: usize, i: usize| -> f32 {
            let t_c = (at(Var::Temp, j, i) - 32.0) * 5.0 / 9.0;
            let td_c = (at(Var::Dew, j, i) - 32.0) * 5.0 / 9.0;
            let p = at(Var::Pressure, j, i).max(500.0);
            let e = 6.112 * ((17.67 * td_c) / (td_c + 243.5)).exp();
            let r = 0.622 * e / (p - e);
            let t_k = t_c + 273.15;
            let theta = t_k * (1000.0 / p).powf(0.2854);
            theta * ((2.5e6 * r) / (1004.0 * t_k)).exp()
        };
        let per10mi = 16_093.0f32;
        let mut out = vec![0.0f32; nx * ny];
        let mean_p: f32 = (0..ny).flat_map(|j| (0..nx).map(move |i| (j, i))).map(|(j, i)| at(Var::Pressure, j, i)).sum::<f32>() / (nx * ny) as f32;
        for j in 0..ny {
            for i in 0..nx {
                let div = ddx(&u, j, i) + ddy(&v, j, i);
                out[j * nx + i] = match field {
                    DataField::Convergence => -div * 1e5,
                    DataField::MoistureConvergence => {
                        let q = (at(Var::Dew, j, i) - 32.0).max(0.0) / 40.0;
                        -div * 1e5 * q
                    }
                    DataField::ThetaE => theta_e(j, i),
                    DataField::ThetaEAdvection => {
                        let adv = -(u(j, i) * ddx(&theta_e, j, i) + v(j, i) * ddy(&theta_e, j, i));
                        adv * 3600.0
                    }
                    DataField::PressureAnomaly => at(Var::Pressure, j, i) - mean_p,
                    DataField::Dryline => {
                        let d = |j: usize, i: usize| at(Var::Dew, j, i);
                        (ddx(&d, j, i).powi(2) + ddy(&d, j, i).powi(2)).sqrt() * per10mi
                    }
                    DataField::Boundaries => {
                        let tt = |j: usize, i: usize| at(Var::Temp, j, i);
                        (ddx(&tt, j, i).powi(2) + ddy(&tt, j, i).powi(2)).sqrt() * per10mi
                    }
                    DataField::DiffHeating => {
                        let c = |j: usize, i: usize| at(Var::Cloud, j, i);
                        (ddx(&c, j, i).powi(2) + ddy(&c, j, i).powi(2)).sqrt() * per10mi
                    }
                    DataField::SnowChance => {
                        // Model snowfall over the next 24 h turned into a chance of 1"+, with the
                        // spread widening with lead time (one model run, so a calibrated guess).
                        let last = (t + 24).min(self.hours() - 1);
                        let acc = self.at(Var::SnowAcc, last, j, i) - at(Var::SnowAcc, j, i);
                        let lead = t.saturating_sub(self.time_index(0.0).max(0.0) as usize) as f32;
                        let spread = 0.35 + 0.015 * lead;
                        100.0 / (1.0 + (-(acc - 1.0) / spread).exp())
                    }
                    DataField::RoadHazard => {
                        let temp = at(Var::Temp, j, i);
                        let rate = at(Var::Precip, j, i);
                        let snow = at(Var::Snow, j, i);
                        let recent = (1..=3).any(|h| self.at(Var::Precip, t.saturating_sub(h), j, i) >= 0.01);
                        let spread = temp - at(Var::Dew, j, i);
                        if snow >= 0.05 || (snow > 0.0 && temp <= 32.0) {
                            3.0
                        } else if temp <= 32.0 && (rate >= 0.01 || recent) {
                            2.0
                        } else if (temp <= 35.0 && rate >= 0.01) || (temp <= 32.0 && spread <= 2.0 && at(Var::Wind, j, i) <= 6.0) {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    DataField::TempAdvection => {
                        let tt = |j: usize, i: usize| at(Var::Temp, j, i);
                        // °F per hour: -(V · ∇T), winds in m/s, gradient per metre.
                        -(u(j, i) * ddx(&tt, j, i) + v(j, i) * ddy(&tt, j, i)) * 3600.0
                    }
                    DataField::Frontogenesis => {
                        // 2D kinematic frontogenesis on surface temperature (°F per 100 km per 3 h).
                        let tt = |j: usize, i: usize| at(Var::Temp, j, i);
                        let (tx, ty) = (ddx(&tt, j, i), ddy(&tt, j, i));
                        let mag = (tx * tx + ty * ty).sqrt().max(1e-9);
                        let f = -(tx * tx * ddx(&u, j, i) + tx * ty * (ddx(&v, j, i) + ddy(&u, j, i)) + ty * ty * ddy(&v, j, i)) / mag;
                        f * 1e5 * 10800.0
                    }
                    DataField::FireDanger => {
                        // Fosberg FFWI: equilibrium moisture from T (°F) and RH, wind in mph.
                        let (t, h) = (at(Var::Temp, j, i), at(Var::Rh, j, i).clamp(1.0, 100.0));
                        let m = if h < 10.0 {
                            0.03229 + 0.281073 * h - 0.000578 * h * t
                        } else if h < 50.0 {
                            2.22749 + 0.160107 * h - 0.01478 * t
                        } else {
                            21.0606 + 0.005565 * h * h - 0.00035 * h * t - 0.483199 * h
                        };
                        let x = m / 30.0;
                        let eta = 1.0 - 2.0 * x + 1.5 * x * x - 0.5 * x * x * x;
                        (eta * (1.0 + at(Var::Wind, j, i).powi(2)).sqrt() / 0.3002).clamp(0.0, 100.0)
                    }
                    DataField::RainRateExceed => {
                        let r = at(Var::Precip, j, i);
                        if r >= 1.0 { r } else { 0.0 }
                    }
                    DataField::AirQuality | DataField::Smoke | DataField::SmokeVisibility if !self.air => f32::NAN,
                    DataField::AirQuality => at(Var::Aqi, j, i),
                    DataField::Smoke => at(Var::Pm25, j, i),
                    DataField::SmokeVisibility => {
                        // Koschmieder with a smoke mass-extinction of ~4 m²/g: vis = 3.9 / b_ext.
                        let b = at(Var::Pm25, j, i).max(1.0) * 4.0e-6;
                        (3.912 / b / 1609.0).min(10.0)
                    }
                    f if !self.extended && Self::needs_extended(f) => f32::NAN,
                    DataField::Cape => at(Var::Cape, j, i),
                    DataField::Lcl => ((at(Var::Temp, j, i) - at(Var::Dew, j, i)) * 5.0 / 9.0 * 125.0 * 3.281).max(0.0),
                    DataField::LapseRate => {
                        let dz = ((at(Var::Z500, j, i) - at(Var::Z850, j, i)) / 1000.0).max(1.0);
                        ((at(Var::T850, j, i) - at(Var::T500, j, i)) * 5.0 / 9.0) / dz
                    }
                    DataField::Stp | DataField::Scp => {
                        let ms = 0.44704;
                        let (du6, dv6) = ((at(Var::U500, j, i) - at(Var::U, j, i)) * ms, (at(Var::V500, j, i) - at(Var::V, j, i)) * ms);
                        let shear = (du6 * du6 + dv6 * dv6).sqrt();
                        let (du1, dv1) = ((at(Var::U850, j, i) - at(Var::U, j, i)) * ms, (at(Var::V850, j, i) - at(Var::V, j, i)) * ms);
                        // Helicity estimate from the low-level shear vector (m²/s²).
                        let srh1 = 0.5 * (du1 * du1 + dv1 * dv1) * 10.0;
                        let cape = at(Var::Cape, j, i).max(0.0);
                        let lcl_m = ((at(Var::Temp, j, i) - at(Var::Dew, j, i)) * 5.0 / 9.0 * 125.0).max(0.0);
                        let shear_term = if shear < 12.5 { 0.0 } else { (shear / 20.0).min(1.5) };
                        if field == DataField::Stp {
                            let lcl_term = ((2000.0 - lcl_m) / 1000.0).clamp(0.0, 1.0);
                            (cape / 1500.0) * lcl_term * (srh1 / 150.0) * shear_term
                        } else {
                            (cape / 1000.0) * (srh1 * 1.5 / 50.0) * shear_term
                        }
                    }
                    DataField::WetBulbZero => {
                        // Wet bulb (Stull 2011) at the surface and 850 mb; interpolate to 0°C.
                        let wb = |t_c: f32, rh: f32| {
                            t_c * (0.151977 * (rh + 8.313659).sqrt()).atan() + (t_c + rh).atan() - (rh - 1.676331).atan() + 0.00391838 * rh.powf(1.5) * (0.023101 * rh).atan() - 4.686035
                        };
                        let ts = (at(Var::Temp, j, i) - 32.0) * 5.0 / 9.0;
                        let w0 = wb(ts, at(Var::Rh, j, i));
                        let t8 = (at(Var::T850, j, i) - 32.0) * 5.0 / 9.0;
                        let d8 = (at(Var::Td850, j, i) - 32.0) * 5.0 / 9.0;
                        let rh8 = (100.0 * ((17.62 * d8 / (243.12 + d8)) - (17.62 * t8 / (243.12 + t8))).exp()).clamp(1.0, 100.0);
                        let w8 = wb(t8, rh8);
                        let z8 = at(Var::Z850, j, i).max(1000.0);
                        if w0 <= 0.0 {
                            0.0
                        } else if w8 < 0.0 {
                            (w0 / (w0 - w8)) * z8 * 3.281
                        } else {
                            // Above 850 mb: continue at the moist lapse rate (~5 °C/km).
                            (z8 + w8 / 5.0 * 1000.0) * 3.281
                        }
                    }
                    DataField::FreezingLevel => at(Var::FreezingLevel, j, i) * 3.281,
                    DataField::Omega => at(Var::Omega700, j, i),
                    DataField::QVector => {
                        // Q = -(∂V/∂x·∇T, ∂V/∂y·∇T) at 850 mb; forcing for ascent = -2 ∇·Q.
                        let t8 = |j: usize, i: usize| at(Var::T850, j, i);
                        let u8 = |j: usize, i: usize| at(Var::U850, j, i) * 0.44704;
                        let v8 = |j: usize, i: usize| at(Var::V850, j, i) * 0.44704;
                        let q1 = |j: usize, i: usize| -(ddx(&u8, j, i) * ddx(&t8, j, i) + ddx(&v8, j, i) * ddy(&t8, j, i));
                        let q2 = |j: usize, i: usize| -(ddy(&u8, j, i) * ddx(&t8, j, i) + ddy(&v8, j, i) * ddy(&t8, j, i));
                        -2.0 * (ddx(&q1, j, i) + ddy(&q2, j, i)) * 1e15
                    }
                    DataField::PrecipType => {
                        if at(Var::Precip, j, i) < 0.005 {
                            0.0
                        } else {
                            let warm_aloft = at(Var::T850, j, i) > 33.0 || at(Var::T700, j, i) > 32.0;
                            let sfc = at(Var::Temp, j, i);
                            if !warm_aloft && sfc <= 34.0 {
                                3.0
                            } else if warm_aloft && sfc <= 32.5 {
                                2.0
                            } else {
                                1.0
                            }
                        }
                    }
                    DataField::IceAccretion => {
                        let sfc = at(Var::Temp, j, i);
                        let warm_aloft = at(Var::T850, j, i) > 33.0 || at(Var::T700, j, i) > 32.0;
                        if warm_aloft && sfc <= 32.0 {
                            // Simple FRAM-style: about half the liquid accretes, more with wind.
                            at(Var::Precip, j, i) * (0.5 + 0.01 * at(Var::Wind, j, i)).min(0.9)
                        } else {
                            0.0
                        }
                    }
                    DataField::SnowRate => at(Var::Snow, j, i),
                    DataField::SnowRatio => kuchera_ratio(at(Var::Temp, j, i), at(Var::T850, j, i), at(Var::T700, j, i)),
                    DataField::LakeEffect => {
                        // Lake water ~4°C in the cold season; ΔT(lake-850) of 13°C+ supports bands.
                        let dt = 4.0 - (at(Var::T850, j, i) - 32.0) * 5.0 / 9.0;
                        let conv = -(ddx(&u, j, i) + ddy(&v, j, i)) * 1e5;
                        if dt < 13.0 { 0.0 } else { (dt - 13.0) / 5.0 + conv.max(0.0) * 0.3 }
                    }
                    DataField::GroundTemp => at(Var::SoilTemp, j, i),
                    DataField::SoilMoisture | DataField::FloodGuidance | DataField::FloodRatio | DataField::FloodPotential => {
                        // Saturation: ~0.45 m³/m³ is saturated for most soils.
                        let sat = (at(Var::SoilMoist, j, i) / 0.45).clamp(0.0, 1.0);
                        // Guidance: 3.0" for dry ground down to 0.8" when saturated.
                        let ffg = 3.0 - 2.2 * sat;
                        let last = (t + 3).min(self.hours() - 1);
                        let rain3 = self.at(Var::RainAcc, last, j, i) - at(Var::RainAcc, j, i);
                        match field {
                            DataField::SoilMoisture => sat * 100.0,
                            DataField::FloodGuidance => ffg,
                            DataField::FloodRatio => (rain3 / ffg * 100.0).max(0.0),
                            _ => ((sat * 4.0) + (rain3 / ffg).min(1.5) * 4.0 + (at(Var::Precip, j, i) / 1.0).min(1.0) * 2.0).min(10.0),
                        }
                    }
                    DataField::SnowSquall => {
                        // NWS snow squall parameter (simplified): low-level RH, lapse rate, wind.
                        let rh = at(Var::Rh, j, i);
                        let lapse = ((at(Var::Temp, j, i) - at(Var::T850, j, i)) * 5.0 / 9.0) / (at(Var::Z850, j, i).max(1000.0) / 1000.0);
                        let wind = (at(Var::U850, j, i).powi(2) + at(Var::V850, j, i).powi(2)).sqrt() * 0.44704;
                        if at(Var::Temp, j, i) > 34.0 { 0.0 } else { (((rh - 60.0) / 15.0).max(0.0) * ((lapse - 4.0) / 4.0).max(0.0) * (wind / 9.0)).min(6.0) }
                    }
                    DataField::FogRisk => {
                        let spread = at(Var::Temp, j, i) - at(Var::Dew, j, i);
                        let wind = at(Var::Wind, j, i);
                        let rh = at(Var::Rh, j, i);
                        let a = ((4.0 - spread) / 4.0).clamp(0.0, 1.0);
                        let b = ((12.0 - wind) / 9.0).clamp(0.0, 1.0);
                        let c = ((rh - 85.0) / 12.0).clamp(0.0, 1.0);
                        (a * b * c * 100.0).round()
                    }
                    _ => {
                        // Cold pool: cooling over the last 2 model hours where it is raining.
                        let earlier = t.saturating_sub(2);
                        let raining = at(Var::Precip, j, i) >= 0.02;
                        let drop = self.at(Var::Temp, earlier, j, i) - at(Var::Temp, j, i);
                        if raining && drop > 0.0 { drop } else { 0.0 }
                    }
                };
            }
        }
        let arc = std::sync::Arc::new(out);
        if let Ok(mut m) = self.analysis.lock() {
            if m.len() > 64 {
                m.clear();
            }
            m.insert((field, t), arc.clone());
        }
        arc
    }

    /// Displayed value for a data field.
    pub fn value(&self, field: DataField, tf: f32, lat: f64, lon: f64) -> Option<f32> {
        if Self::is_analysis(field) {
            let (gx, gy) = self.grid_pos(lat, lon)?;
            let t = (tf.round().max(0.0) as usize).min(self.hours() - 1);
            let arr = self.analysis_nodes(field, t);
            let i0 = gx.floor() as usize;
            let j0 = gy.floor() as usize;
            let i1 = (i0 + 1).min(self.nx - 1);
            let j1 = (j0 + 1).min(self.ny - 1);
            let (fx, fy) = (gx - i0 as f32, gy - j0 as f32);
            let at = |j: usize, i: usize| arr[j * self.nx + i];
            let a = at(j0, i0) * (1.0 - fx) + at(j0, i1) * fx;
            let b = at(j1, i0) * (1.0 - fx) + at(j1, i1) * fx;
            let r = a * (1.0 - fy) + b * fy;
            return r.is_finite().then_some(r);
        }
        if matches!(field, DataField::HighTemp | DataField::LowTemp | DataField::TempChange | DataField::MaxGust | DataField::IceTotal) {
            let (gx, gy) = self.grid_pos(lat, lon)?;
            let arr = self.derived_nodes(field);
            let i0 = gx.floor() as usize;
            let j0 = gy.floor() as usize;
            let i1 = (i0 + 1).min(self.nx - 1);
            let j1 = (j0 + 1).min(self.ny - 1);
            let (fx, fy) = (gx - i0 as f32, gy - j0 as f32);
            let at = |j: usize, i: usize| arr[j * self.nx + i];
            let a = at(j0, i0) * (1.0 - fx) + at(j0, i1) * fx;
            let b = at(j1, i0) * (1.0 - fx) + at(j1, i1) * fx;
            return Some(a * (1.0 - fy) + b * fy);
        }
        let var = match field {
            DataField::Temperature => Var::Temp,
            DataField::FeelsLike => Var::Feels,
            DataField::DewPoint => Var::Dew,
            DataField::Humidity => Var::Rh,
            DataField::WindSpeed => Var::Wind,
            DataField::WindGust => Var::Gust,
            DataField::Pressure => Var::Pressure,
            DataField::CloudCover => Var::Cloud,
            DataField::Futurecast => Var::Precip,
            DataField::RainTotal => Var::RainAcc,
            DataField::SnowTotal => Var::SnowAcc,
            DataField::Snow10 => Var::Snow10Acc,
            DataField::SnowKuchera => Var::KucheraAcc,
            _ => return None,
        };
        let v = self.sample(var, tf, lat, lon)?;
        // Totals are measured from "now", not from the first model hour.
        if field.is_total() {
            let now = self.time_index(0.0);
            let start = self.sample(var, now, lat, lon).unwrap_or(0.0);
            return Some((v - start).max(0.0));
        }
        Some(v)
    }

    /// Wind vector (mph, east/north components).
    pub fn wind(&self, tf: f32, lat: f64, lon: f64) -> Option<(f32, f32)> {
        Some((self.sample(Var::U, tf, lat, lon)?, self.sample(Var::V, tf, lat, lon)?))
    }

    /// Futurecast precipitation: rate (in/hr) and type.
    pub fn precip(&self, tf: f32, lat: f64, lon: f64) -> Option<(f32, PrecipKind)> {
        let rate = self.sample(Var::Precip, tf, lat, lon)?;
        if rate < 0.003 {
            return Some((0.0, PrecipKind::Rain));
        }
        let snow = self.sample(Var::Snow, tf, lat, lon).unwrap_or(0.0);
        let code = self.nearest(Var::Code, tf, lat, lon).unwrap_or(0.0) as i32;
        let temp = self.sample(Var::Temp, tf, lat, lon).unwrap_or(50.0);
        let kind = match code {
            56 | 57 | 66 | 67 | 79 => PrecipKind::Mix,
            71..=77 | 85 | 86 => PrecipKind::Snow,
            _ if snow > rate * 4.0 => PrecipKind::Snow,
            _ if snow > 0.0 || (temp <= 33.5 && temp > 30.0) => PrecipKind::Mix,
            _ => PrecipKind::Rain,
        };
        Some((rate, kind))
    }

    /// Value at grid node (for isobars / extrema).
    pub fn node(&self, var: Var, tf: f32, j: usize, i: usize) -> f32 {
        let t0 = (tf.floor().max(0.0) as usize).min(self.hours() - 1);
        let t1 = (t0 + 1).min(self.hours() - 1);
        let ft = tf - t0 as f32;
        self.at(var, t0, j, i) * (1.0 - ft) + self.at(var, t1, j, i) * ft
    }

    /// A copy with the forecaster's area adjustments applied (Forecast editor).
    pub fn adjusted(&self, edits: &crate::forecast_edit::EditSet) -> WxGrid {
        use crate::forecast_edit::EditVar;
        let mut g = self.clone();
        g.derived = Default::default();
        g.analysis = Default::default();
        if edits.adjustments.is_empty() {
            return g;
        }
        let now_tf = self.time_index(0.0);
        let (nx, ny) = (g.nx, g.ny);
        for a in &edits.adjustments {
            let var = match a.var {
                EditVar::Temp => Var::Temp,
                EditVar::Dew => Var::Dew,
                EditVar::Wind => Var::Wind,
                EditVar::Gust => Var::Gust,
                EditVar::Cloud => Var::Cloud,
                EditVar::Precip => Var::Precip,
            };
            for t in 0..g.hours() {
                let tw = a.time_weight(t as f32 - now_tf);
                if tw <= 0.0 {
                    continue;
                }
                for j in 0..ny {
                    for i in 0..nx {
                        let (lat, lon) = g.node_geo(j, i);
                        let w = a.shape.weight(lat, lon) * tw;
                        if w <= 0.0 {
                            continue;
                        }
                        let k = (t * ny + j) * nx + i;
                        match a.var {
                            EditVar::Precip => g.data[var as usize][k] *= (1.0 + a.delta / 100.0 * w).max(0.0),
                            EditVar::Cloud => g.data[var as usize][k] = (g.data[var as usize][k] + a.delta * w).clamp(0.0, 100.0),
                            EditVar::Temp => {
                                g.data[Var::Temp as usize][k] += a.delta * w;
                                g.data[Var::Feels as usize][k] += a.delta * w;
                            }
                            _ => g.data[var as usize][k] = (g.data[var as usize][k] + a.delta * w).max(0.0),
                        }
                    }
                }
            }
        }
        g
    }

    /// Geographic position of grid node (j, i).
    pub fn node_geo(&self, j: usize, i: usize) -> (f64, f64) {
        let lat = self.north - (self.north - self.south) * j as f64 / (self.ny - 1) as f64;
        let lon = self.west + (self.east - self.west) * i as f64 / (self.nx - 1) as f64;
        (lat, lon)
    }
}


/// Dew point (F) from temperature (F) and relative humidity (%), Magnus formula.
fn dew_point_f(temp_f: f32, rh: f32) -> f32 {
    let t = (temp_f - 32.0) * 5.0 / 9.0;
    let (a, b) = (17.625f32, 243.04f32);
    let g = (rh.clamp(1.0, 100.0) / 100.0).ln() + a * t / (b + t);
    let td = b * g / (a - g);
    td * 9.0 / 5.0 + 32.0
}

/// NWS feels-like: heat index when hot, wind chill when cold and breezy, else the temperature.
fn feels_like_f(t: f32, rh: f32, wind_mph: f32) -> f32 {
    if t >= 80.0 && rh >= 40.0 {
        let r = rh;
        -42.379 + 2.049_015_3 * t + 10.143_331 * r - 0.224_755_4 * t * r - 0.006_837_83 * t * t
            - 0.054_817_17 * r * r
            + 0.001_228_74 * t * t * r
            + 0.000_852_82 * t * r * r
            - 0.000_001_99 * t * t * r * r
    } else if t <= 50.0 && wind_mph >= 3.0 {
        let v = wind_mph.powf(0.16);
        35.74 + 0.6215 * t - 35.75 * v + 0.4275 * t * v
    } else {
        t
    }
}

/// One grid node's hourly series in the grid's units: °F, %, mph (direction the wind comes
/// from), hPa, inches per hour (snow at Open-Meteo's 7:1), WMO weather code, plus the upper-air
/// and ground ingredients (850/700/500 mb °F, J/kg, µb/s rising, metres, soil °F and m³/m³).
#[derive(Default)]
struct NodeSeries {
    temp: Vec<f32>,
    rh: Vec<f32>,
    wind: Vec<f32>,
    dir: Vec<f32>,
    gust: Vec<f32>,
    pres: Vec<f32>,
    cloud: Vec<f32>,
    precip: Vec<f32>,
    snow: Vec<f32>,
    code: Vec<f32>,
    ext: Vec<(Var, Vec<f32>)>,
    ws850: Vec<f32>,
    wd850: Vec<f32>,
    ws500: Vec<f32>,
    wd500: Vec<f32>,
}

/// Write one node's series into the grid layers, with the derived values (feels like, dew
/// point, wind components and the running rain / snow totals). Shared by every source so they
/// agree exactly.
fn fill_node(data: &mut [Vec<f32>], (nt, nx, ny): (usize, usize, usize), (j, i): (usize, usize), s: &NodeSeries, extended: bool) {
    let mut rain_acc = 0.0;
    let mut snow_acc = 0.0;
    let mut snow10_acc = 0.0;
    let mut kuchera_acc = 0.0;
    let level = |want: Var| s.ext.iter().find(|(v, _)| *v as usize == want as usize).map(|(_, v)| v);
    let (t850s, t700s) = (level(Var::T850), level(Var::T700));
    for t in 0..nt {
        let k = (t * ny + j) * nx + i;
        let get = |v: &Vec<f32>| v.get(t).copied().unwrap_or(0.0);
        // Direction is where the wind comes FROM; the vector points downwind.
        let rad = get(&s.dir).to_radians();
        let speed = get(&s.wind);
        rain_acc += get(&s.precip);
        snow_acc += get(&s.snow);
        // Snowfall here is at 7:1; recover the liquid equivalent for other ratios.
        let swe = get(&s.snow) / 7.0;
        snow10_acc += swe * 10.0;
        let ratio = match (t850s.map(|a| get(a)), t700s.map(|b| get(b))) {
            (Some(a), Some(b)) if a.is_finite() && b.is_finite() => kuchera_ratio(get(&s.temp), a, b),
            _ => 10.0,
        };
        kuchera_acc += swe * ratio;
        data[Var::Temp as usize][k] = get(&s.temp);
        data[Var::Feels as usize][k] = feels_like_f(get(&s.temp), get(&s.rh), speed);
        data[Var::Dew as usize][k] = dew_point_f(get(&s.temp), get(&s.rh));
        data[Var::Rh as usize][k] = get(&s.rh);
        data[Var::Wind as usize][k] = speed;
        data[Var::U as usize][k] = -speed * rad.sin();
        data[Var::V as usize][k] = -speed * rad.cos();
        data[Var::Gust as usize][k] = get(&s.gust);
        data[Var::Pressure as usize][k] = get(&s.pres);
        data[Var::Cloud as usize][k] = get(&s.cloud);
        data[Var::Precip as usize][k] = get(&s.precip);
        for (v, series) in &s.ext {
            // Missing ingredients stay missing (NaN), never a false 0.
            data[*v as usize][k] = series.get(t).copied().unwrap_or(f32::NAN);
        }
        if extended {
            let (s8, r8) = (get(&s.ws850), get(&s.wd850).to_radians());
            let (s5, r5) = (get(&s.ws500), get(&s.wd500).to_radians());
            data[Var::U850 as usize][k] = -s8 * r8.sin();
            data[Var::V850 as usize][k] = -s8 * r8.cos();
            data[Var::U500 as usize][k] = -s5 * r5.sin();
            data[Var::V500 as usize][k] = -s5 * r5.cos();
        }
        data[Var::Snow as usize][k] = get(&s.snow);
        data[Var::Code as usize][k] = get(&s.code);
        data[Var::RainAcc as usize][k] = rain_acc;
        data[Var::SnowAcc as usize][k] = snow_acc;
        data[Var::Snow10Acc as usize][k] = snow10_acc;
        data[Var::KucheraAcc as usize][k] = kuchera_acc;
    }
}

/// Grid bounds for a view, padded so small pans don't refetch.
/// The NOAA model a view really gets. HRRR when it covers the view and the field needs no
/// upper-air data (the NOAA HRRR files are surface-only, so precipitation type and the winter and
/// severe ingredients come from GFS), and HRRR loaded recently; GFS otherwise.
pub fn effective_model(model: GridModel, view: crate::config::GeoView, extended: bool) -> GridModel {
    let hrrr_ok = model == GridModel::Hrrr && !extended && noaa::fits(model, grid_bounds(view)) && noaa::hrrr_available();
    if hrrr_ok { GridModel::Hrrr } else { GridModel::Gfs }
}

/// Whether the map grid for this view and model is read from NOAA (credits and info text).
pub fn grid_from_noaa(model: GridModel, view: crate::config::GeoView) -> bool {
    noaa::from_noaa(model, grid_bounds(view))
}

fn grid_bounds(view: crate::config::GeoView) -> (f64, f64, f64, f64) {
    let (north, south, west, east) = view_bounds(view, 0.25);
    (north.ceil().min(85.0), south.floor().max(-85.0), west.floor().max(-180.0), east.ceil().min(180.0))
}

/// Node positions (lat, lon), row-major from the north-west corner.
fn node_points((north, south, west, east): (f64, f64, f64, f64), nx: usize, ny: usize) -> Vec<(f64, f64)> {
    let mut points = Vec::with_capacity(nx * ny);
    for j in 0..ny {
        for i in 0..nx {
            let lat = north - (north - south) * j as f64 / (ny - 1) as f64;
            let lon = west + (east - west) * i as f64 / (nx - 1) as f64;
            points.push((lat, crate::map::geo::wrap_lon(lon)));
        }
    }
    points
}

/// US AQI and PM2.5 (smoke) per point and model hour from Open-Meteo's air-quality API (no
/// NOAA source covers both). None when the request fails or comes back short.
async fn air_quality(points: &[(f64, f64)], times: &[i64]) -> Option<Vec<[Vec<f32>; 2]>> {
    let mut tasks = Vec::new();
    for chunk in points.chunks(CHUNK) {
        let lats = chunk.iter().map(|p| format!("{:.3}", p.0)).collect::<Vec<_>>().join(",");
        let lons = chunk.iter().map(|p| format!("{:.3}", p.1)).collect::<Vec<_>>().join(",");
        let url = format!("https://air-quality-api.open-meteo.com/v1/air-quality?latitude={lats}&longitude={lons}&hourly=us_aqi,pm2_5&forecast_hours={AIR_HOURS}&past_hours=1&timeformat=unixtime&timezone=GMT");
        tasks.push(net::spawn(async move {
            let response = net::get_logged(&url, std::time::Duration::from_secs(40)).await?;
            Ok::<serde_json::Value, anyhow::Error>(response.json().await?)
        }));
    }
    let mut locations: Vec<serde_json::Value> = Vec::new();
    for task in tasks {
        match task.await.ok()?.ok()? {
            serde_json::Value::Array(items) => locations.extend(items),
            other => locations.push(other),
        }
    }
    if locations.len() != points.len() {
        return None;
    }
    Some(
        locations
            .iter()
            .map(|loc| {
                let aq_times: Vec<i64> = loc.pointer("/hourly/time").and_then(|t| t.as_array()).map(|a| a.iter().filter_map(|v| v.as_i64()).collect()).unwrap_or_default();
                ["us_aqi", "pm2_5"].map(|key| {
                    let vals: Vec<f32> = loc.pointer(&format!("/hourly/{key}")).and_then(|a| a.as_array()).map(|a| a.iter().map(|v| v.as_f64().unwrap_or(f64::NAN) as f32).collect()).unwrap_or_default();
                    // Align by timestamp (the APIs can start on different hours).
                    times.iter().map(|time| aq_times.iter().position(|x| x == time).and_then(|k| vals.get(k).copied()).unwrap_or(f32::NAN)).collect()
                })
            })
            .collect(),
    )
}

/// Fetch a grid covering `view`, read from NOAA's own model files (never Open-Meteo). HRRR covers
/// state- and region-size views of the continental U.S.; wider views, other models from older
/// settings, or a missing HRRR file use NOAA GFS, and the grid's `model` says which one it is.
/// Air quality is the one Open-Meteo layer: there is no NOAA or NWS source for it.
pub async fn fetch(view: crate::config::GeoView, extended: bool, air: bool, model: GridModel) -> Result<WxGrid> {
    let bounds = grid_bounds(view);
    let wanted = effective_model(model, view, extended);
    let mut grid = match noaa::fetch(bounds, wanted, view.zoom).await {
        Ok(grid) => {
            if wanted == GridModel::Hrrr {
                noaa::set_hrrr_from_noaa(true);
            }
            grid
        }
        Err(e) if wanted == GridModel::Hrrr => {
            noaa::set_hrrr_from_noaa(false);
            log::warn!("NOAA HRRR grid: {e:#}; showing NOAA GFS instead");
            noaa::fetch(bounds, GridModel::Gfs, view.zoom).await?
        }
        Err(e) => return Err(e),
    };
    if air {
        noaa::add_air_quality(&mut grid).await;
    }
    Ok(grid)
}

// ---- Color ramps -----------------------------------------------------------------------------
/// Temperature change: blue colder, grey the same, red warmer.
pub const CHANGE_STOPS: &[(f32, [u8; 3])] = &[
    (-20.0, [40, 70, 200]),
    (-10.0, [90, 150, 235]),
    (0.0, [150, 150, 160]),
    (10.0, [240, 150, 70]),
    (20.0, [210, 40, 40]),
];

/// Convergence (positive) vs divergence (negative).
pub const CONV_STOPS: &[(f32, [u8; 3])] = &[
    (-10.0, [60, 90, 200]),
    (-2.0, [120, 160, 230]),
    (0.0, [200, 200, 200]),
    (2.0, [240, 180, 80]),
    (10.0, [220, 40, 60]),
];

pub const THETA_E_STOPS: &[(f32, [u8; 3])] = &[
    (290.0, [60, 70, 180]),
    (310.0, [80, 170, 200]),
    (330.0, [120, 200, 90]),
    (345.0, [240, 200, 60]),
    (360.0, [220, 60, 40]),
];

/// Sharpness of a boundary (gradient magnitude).
pub const GRADIENT_STOPS: &[(f32, [u8; 3])] = &[
    (0.6, [250, 240, 150]),
    (2.0, [250, 170, 60]),
    (4.0, [220, 60, 40]),
    (8.0, [150, 20, 120]),
];

/// Ice accrual (inches).
pub const ICE_STOPS: &[(f32, [u8; 3])] = &[
    (0.01, [240, 190, 230]),
    (0.1, [230, 130, 200]),
    (0.25, [200, 60, 170]),
    (0.5, [150, 30, 140]),
    (1.0, [90, 10, 100]),
];

pub(crate) fn ramp(stops: &[(f32, [u8; 3])], v: f32) -> [u8; 3] {
    if v <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        let (a, ca) = w[0];
        let (b, cb) = w[1];
        if v <= b {
            let t = ((v - a) / (b - a)).clamp(0.0, 1.0);
            return [
                (ca[0] as f32 + (cb[0] as f32 - ca[0] as f32) * t) as u8,
                (ca[1] as f32 + (cb[1] as f32 - ca[1] as f32) * t) as u8,
                (ca[2] as f32 + (cb[2] as f32 - ca[2] as f32) * t) as u8,
            ];
        }
    }
    stops[stops.len() - 1].1
}

/// Ground (soil) temperature: same colour family as air temperature, spread over 10-90 °F with
/// a pale band at freezing, so a 44-51° map shows a pattern instead of one flat green.
pub const SOIL_STOPS: &[(f32, [u8; 3])] = &[
    (10.0, [60, 70, 200]),
    (20.0, [60, 140, 230]),
    (28.0, [120, 190, 240]),
    (32.0, [205, 235, 250]),
    (36.0, [110, 200, 200]),
    (40.0, [80, 190, 170]),
    (45.0, [70, 180, 110]),
    (50.0, [70, 180, 80]),
    (55.0, [130, 200, 70]),
    (60.0, [190, 215, 60]),
    (65.0, [245, 220, 60]),
    (70.0, [245, 180, 50]),
    (80.0, [245, 120, 40]),
    (90.0, [225, 60, 40]),
];

pub const TEMP_STOPS: &[(f32, [u8; 3])] = &[
    (-20.0, [120, 40, 160]),
    (0.0, [60, 70, 200]),
    (15.0, [60, 140, 230]),
    (32.0, [150, 210, 245]),
    (40.0, [80, 190, 170]),
    (50.0, [70, 180, 80]),
    (60.0, [170, 210, 60]),
    (70.0, [245, 220, 60]),
    (80.0, [245, 150, 40]),
    (90.0, [225, 70, 40]),
    (100.0, [170, 20, 40]),
    (110.0, [120, 10, 60]),
];

pub const DEW_STOPS: &[(f32, [u8; 3])] = &[
    (10.0, [150, 110, 70]),
    (30.0, [200, 170, 110]),
    (45.0, [150, 200, 120]),
    (55.0, [60, 180, 90]),
    (62.0, [30, 140, 70]),
    (68.0, [40, 160, 180]),
    (72.0, [40, 100, 200]),
    (78.0, [120, 60, 190]),
];

pub const HUMIDITY_STOPS: &[(f32, [u8; 3])] = &[
    (0.0, [170, 120, 60]),
    (30.0, [220, 190, 120]),
    (50.0, [150, 210, 140]),
    (70.0, [70, 180, 140]),
    (90.0, [40, 120, 200]),
    (100.0, [30, 70, 170]),
];

pub const WIND_STOPS: &[(f32, [u8; 3])] = &[
    (0.0, [40, 60, 110]),
    (5.0, [50, 120, 170]),
    (10.0, [60, 170, 150]),
    (15.0, [120, 200, 80]),
    (20.0, [230, 220, 60]),
    (30.0, [245, 150, 40]),
    (40.0, [225, 60, 40]),
    (55.0, [190, 30, 120]),
    (70.0, [140, 40, 190]),
];

pub const PRESSURE_STOPS: &[(f32, [u8; 3])] = &[
    (980.0, [140, 40, 140]),
    (996.0, [200, 70, 70]),
    (1008.0, [220, 200, 140]),
    (1016.0, [140, 190, 200]),
    (1028.0, [60, 110, 200]),
    (1040.0, [40, 50, 150]),
];

pub const CLOUD_STOPS: &[(f32, [u8; 3])] = &[(0.0, [20, 30, 50]), (50.0, [130, 140, 160]), (100.0, [235, 238, 245])];

/// Rain totals in TWC colours (LibreWXR palette 4): each amount is shown in the table's colour
/// for the reflectivity in [`RAIN_TOTAL_DBZ`], so totals read like the radar (greens, yellow,
/// orange, reds). Exact samples of the table, checked by `twc::tests`.
pub const RAIN_TOTAL_STOPS: &[(f32, [u8; 3])] = &[
    (0.01, [1, 180, 20]),
    (0.1, [17, 101, 26]),
    (0.25, [7, 68, 7]),
    (0.5, [254, 237, 7]),
    (1.0, [243, 139, 8]),
    (2.0, [234, 94, 9]),
    (3.0, [220, 46, 10]),
    (5.0, [191, 12, 8]),
];
pub const RAIN_TOTAL_DBZ: [f32; 8] = [12.0, 20.0, 26.0, 31.0, 40.0, 50.0, 58.0, 66.0];

/// Snow totals in TWC snow colours (white through cyan to deep blue), as [`RAIN_TOTAL_STOPS`].
pub const SNOW_TOTAL_STOPS: &[(f32, [u8; 3])] = &[
    (0.1, [207, 255, 255]),
    (1.0, [159, 255, 255]),
    (3.0, [111, 223, 255]),
    (6.0, [63, 159, 255]),
    (9.0, [15, 111, 255]),
    (12.0, [0, 77, 255]),
    (18.0, [0, 31, 255]),
];
pub const SNOW_TOTAL_DBZ: [f32; 7] = [-10.0, 5.0, 20.0, 35.0, 50.0, 62.0, 75.0];

/// The snow-total map's TWC colour for an amount (inches).
pub fn snow_total_rgb(v: f32) -> [u8; 3] {
    total_rgb(SNOW_TOTAL_STOPS, &SNOW_TOTAL_DBZ, v, true)
}

/// The TWC colour for a total: the amount becomes a reflectivity between the stops, then the
/// table colour at that reflectivity (so in-between amounts follow the table, not a blend).
fn total_rgb(stops: &[(f32, [u8; 3])], dbz: &[f32], v: f32, snow: bool) -> [u8; 3] {
    let k = stops.partition_point(|(amount, _)| *amount < v).clamp(1, stops.len() - 1);
    let (a, b) = (stops[k - 1].0, stops[k].0);
    let t = ((v - a) / (b - a)).clamp(0.0, 1.0);
    let d = dbz[k - 1] + (dbz[k] - dbz[k - 1]) * t;
    let rgb = if snow { crate::twc::snow(d) } else { crate::twc::rain(d) };
    rgb.unwrap_or(stops[0].1)
}

/// Model rain rate (in/h of liquid) in TWC radar colours: exact table samples at the rate's
/// Marshall-Palmer reflectivity (legend stops; the map itself uses `twc` directly).
const RAIN_RATE: &[(f32, [u8; 3])] = &[
    (0.006, [1, 183, 20]),
    (0.02, [10, 126, 22]),
    (0.05, [8, 72, 10]),
    (0.1, [192, 195, 7]),
    (0.25, [247, 186, 8]),
    (0.5, [242, 138, 8]),
    (1.0, [240, 113, 8]),
];
const MIX_RATE: &[(f32, [u8; 3])] = &[(0.005, [245, 170, 210]), (0.08, [230, 110, 180]), (0.3, [190, 50, 150])];
/// Model snow rate (in/h of liquid) in TWC snow colours, at Z = 2000 R².
const SNOW_RATE: &[(f32, [u8; 3])] = &[
    (0.003, [142, 254, 255]),
    (0.01, [110, 222, 255]),
    (0.02, [93, 203, 255]),
    (0.04, [74, 170, 255]),
    (0.08, [53, 149, 255]),
    (0.15, [34, 130, 255]),
];

const SNOW_CHANCE_STOPS: &[(f32, [u8; 3])] = &[
    (10.0, [200, 225, 250]),
    (30.0, [140, 190, 240]),
    (50.0, [80, 140, 225]),
    (70.0, [50, 90, 200]),
    (90.0, [90, 50, 170]),
];
const CAPE_STOPS: &[(f32, [u8; 3])] = &[(500.0, [240, 230, 120]), (1000.0, [245, 180, 70]), (2000.0, [235, 110, 50]), (3000.0, [210, 50, 60]), (4000.0, [160, 40, 140])];
const INDEX_STOPS: &[(f32, [u8; 3])] = &[(1.0, [250, 220, 90]), (2.0, [245, 150, 50]), (4.0, [220, 60, 60]), (8.0, [170, 50, 170])];
const LAPSE_STOPS: &[(f32, [u8; 3])] = &[(5.0, [90, 140, 220]), (6.0, [120, 200, 160]), (7.0, [240, 220, 90]), (8.0, [240, 140, 60]), (9.0, [210, 50, 60])];
const HEIGHT_STOPS: &[(f32, [u8; 3])] = &[(1000.0, [220, 70, 70]), (3000.0, [240, 170, 70]), (6000.0, [200, 210, 120]), (10000.0, [110, 180, 220]), (14000.0, [80, 110, 210])];
const PTYPE_STOPS: &[(f32, [u8; 3])] = &[(1.0, [80, 200, 90]), (2.0, [235, 120, 190]), (3.0, [120, 170, 245])];
const SMOKE_STOPS: &[(f32, [u8; 3])] = &[(12.0, [200, 190, 170]), (35.0, [180, 150, 110]), (55.0, [160, 110, 70]), (150.0, [130, 70, 50]), (250.0, [90, 40, 40])];
const AQI_STOPS: &[(f32, [u8; 3])] = &[(25.0, [0, 228, 0]), (75.0, [255, 255, 0]), (125.0, [255, 126, 0]), (175.0, [255, 0, 0]), (250.0, [143, 63, 151]), (350.0, [126, 0, 35])];
const ROAD_STOPS: &[(f32, [u8; 3])] = &[(1.0, [235, 205, 90]), (2.0, [230, 120, 60]), (3.0, [200, 60, 110])];
const FOG_STOPS: &[(f32, [u8; 3])] = &[(20.0, [190, 195, 200]), (60.0, [215, 218, 222]), (90.0, [245, 246, 248])];

/// RGBA for a data-field value (None = transparent).
pub fn field_rgba(field: DataField, v: f32, opacity: f32) -> Option<[u8; 4]> {
    let a = (opacity * 255.0) as u8;
    let rgb = match field {
        DataField::Temperature | DataField::FeelsLike | DataField::HighTemp | DataField::LowTemp => ramp(TEMP_STOPS, v),
        DataField::TempChange => ramp(CHANGE_STOPS, v),
        DataField::Convergence | DataField::MoistureConvergence | DataField::ThetaEAdvection => {
            if v.abs() < 0.5 {
                return None;
            }
            ramp(CONV_STOPS, v.clamp(-10.0, 10.0))
        }
        DataField::ThetaE => ramp(THETA_E_STOPS, v),
        DataField::PressureAnomaly => ramp(CHANGE_STOPS, v * 4.0),
        DataField::Dryline | DataField::Boundaries | DataField::DiffHeating => {
            let v = if field == DataField::DiffHeating { v / 4.0 } else { v };
            if v < 0.6 {
                return None;
            }
            ramp(GRADIENT_STOPS, v)
        }
        DataField::ColdPool => {
            if v < 1.0 {
                return None;
            }
            ramp(GRADIENT_STOPS, v / 1.5)
        }
        DataField::MaxGust => ramp(WIND_STOPS, v),
        DataField::SnowChance => {
            if v < 10.0 {
                return None;
            }
            ramp(SNOW_CHANCE_STOPS, v)
        }
        _ if v.is_nan() => return None,
        DataField::SoilMoisture => ramp(HUMIDITY_STOPS, v),
        DataField::FloodGuidance => ramp(RAIN_TOTAL_STOPS, (4.0 - v).max(0.0)),
        DataField::FloodRatio => {
            if v < 25.0 {
                return None;
            }
            ramp(INDEX_STOPS, v / 25.0)
        }
        DataField::FloodPotential => {
            if v < 3.0 {
                return None;
            }
            ramp(INDEX_STOPS, v * 0.8)
        }
        DataField::RainRateExceed => {
            if v < 1.0 {
                return None;
            }
            ramp(RAIN_TOTAL_STOPS, v * 2.0)
        }
        DataField::AirQuality => match v {
            v if v < 51.0 => [0, 228, 0],
            v if v < 101.0 => [255, 255, 0],
            v if v < 151.0 => [255, 126, 0],
            v if v < 201.0 => [255, 0, 0],
            v if v < 301.0 => [143, 63, 151],
            _ => [126, 0, 35],
        },
        DataField::Smoke => {
            if v < 12.0 {
                return None;
            }
            ramp(SMOKE_STOPS, v)
        }
        DataField::SmokeVisibility => {
            if v >= 6.0 {
                return None;
            }
            ramp(SMOKE_STOPS, 200.0 - v * 30.0)
        }
        DataField::FireDanger => {
            if v < 20.0 {
                return None;
            }
            ramp(INDEX_STOPS, v / 12.0)
        }
        DataField::Cape => {
            if v < 100.0 {
                return None;
            }
            ramp(CAPE_STOPS, v)
        }
        DataField::Stp | DataField::Scp | DataField::SnowSquall | DataField::LakeEffect => {
            if v < 0.5 {
                return None;
            }
            ramp(INDEX_STOPS, v)
        }
        DataField::LapseRate => ramp(LAPSE_STOPS, v),
        DataField::Lcl | DataField::WetBulbZero | DataField::FreezingLevel => ramp(HEIGHT_STOPS, v),
        DataField::Omega | DataField::QVector | DataField::TempAdvection | DataField::Frontogenesis => {
            if v.abs() < 0.3 {
                return None;
            }
            ramp(CONV_STOPS, v.clamp(-10.0, 10.0))
        }
        DataField::PrecipType => match v.round() as i32 {
            1 => [80, 200, 90],
            2 => [235, 120, 190],
            3 => [120, 170, 245],
            _ => return None,
        },
        DataField::IceAccretion => {
            if v < 0.01 {
                return None;
            }
            ramp(ICE_STOPS, v * 4.0)
        }
        DataField::SnowRate => {
            if v < 0.05 {
                return None;
            }
            ramp(SNOW_CHANCE_STOPS, v * 40.0)
        }
        DataField::SnowRatio => ramp(SNOW_CHANCE_STOPS, v * 3.0),
        DataField::GroundTemp => ramp(SOIL_STOPS, v),
        DataField::RoadHazard => {
            if v < 0.5 {
                return None;
            }
            ramp(ROAD_STOPS, v)
        }
        DataField::FogRisk => {
            if v < 20.0 {
                return None;
            }
            let c = ramp(FOG_STOPS, v);
            return Some([c[0], c[1], c[2], ((v / 100.0).max(0.35) * opacity * 255.0) as u8]);
        }
        DataField::IceTotal => {
            if v < 0.01 {
                return None;
            }
            ramp(ICE_STOPS, v)
        }
        DataField::DewPoint => ramp(DEW_STOPS, v),
        DataField::Humidity => ramp(HUMIDITY_STOPS, v),
        DataField::WindSpeed | DataField::WindGust => ramp(WIND_STOPS, v),
        DataField::Pressure => ramp(PRESSURE_STOPS, v),
        DataField::CloudCover => {
            if v < 8.0 {
                return None;
            }
            let c = ramp(CLOUD_STOPS, v);
            return Some([c[0], c[1], c[2], ((v / 100.0) * opacity * 255.0) as u8]);
        }
        DataField::RainTotal => {
            if v < 0.01 {
                return None;
            }
            total_rgb(RAIN_TOTAL_STOPS, &RAIN_TOTAL_DBZ, v, false)
        }
        DataField::SnowTotal | DataField::Snow10 | DataField::SnowKuchera => {
            if v < 0.1 {
                return None;
            }
            total_rgb(SNOW_TOTAL_STOPS, &SNOW_TOTAL_DBZ, v, true)
        }
        DataField::Futurecast => return None,
    };
    Some([rgb[0], rgb[1], rgb[2], a])
}

/// Futurecast precipitation color: rain greens→reds, mix pinks, snow blues.
pub fn precip_rgba(rate: f32, kind: PrecipKind, opacity: f32) -> Option<[u8; 4]> {
    if rate < 0.004 {
        return None;
    }
    // Rain and snow in the TWC radar colours (LibreWXR palette 4) at the reflectivity the rate
    // gives; nothing where that radar would show nothing. Mix keeps its pinks (palette 4 has no
    // mix colours).
    let rgb = match kind {
        PrecipKind::Rain => crate::twc::rain(crate::twc::rain_dbz(rate))?,
        PrecipKind::Mix => ramp(MIX_RATE, rate),
        PrecipKind::Snow => crate::twc::snow(crate::twc::snow_dbz(rate))?,
    };
    // Light precipitation fades in at the edges.
    let edge = ((rate - 0.004) / 0.02).clamp(0.35, 1.0);
    Some([rgb[0], rgb[1], rgb[2], (opacity.max(0.6) * edge * 255.0) as u8])
}

/// Legend stops for a field: (value, color) pairs and labels.
pub fn legend(field: DataField) -> (&'static [(f32, [u8; 3])], &'static [&'static str]) {
    match field {
        DataField::Temperature | DataField::FeelsLike | DataField::HighTemp | DataField::LowTemp => (TEMP_STOPS, &["-20", "0", "32", "50", "70", "90", "110"]),
        DataField::TempChange => (CHANGE_STOPS, &["-20", "-10", "0", "+10", "+20"]),
        DataField::Convergence | DataField::MoistureConvergence | DataField::ThetaEAdvection => (CONV_STOPS, &["DIVERGING", "", "CONVERGING"]),
        DataField::ThetaE => (THETA_E_STOPS, &["300", "320", "340", "360"]),
        DataField::PressureAnomaly => (CHANGE_STOPS, &["LOWER", "", "AVERAGE", "", "HIGHER"]),
        DataField::Dryline | DataField::Boundaries | DataField::DiffHeating | DataField::ColdPool => (GRADIENT_STOPS, &["WEAK", "", "SHARP"]),
        DataField::SnowChance => (SNOW_CHANCE_STOPS, &["10%", "30%", "50%", "70%", "90%"]),
        DataField::SoilMoisture => (HUMIDITY_STOPS, &["DRY", "", "", "", "", "SATURATED"]),
        DataField::FloodGuidance => (RAIN_TOTAL_STOPS, &["3\"", "", "2\"", "", "1\"", "<1\""]),
        DataField::FloodRatio => (INDEX_STOPS, &["25%", "50%", "100%", "200%"]),
        DataField::FloodPotential => (INDEX_STOPS, &["LOW", "", "HIGH", "EXTREME"]),
        DataField::RainRateExceed => (RAIN_TOTAL_STOPS, &["", "", "1\"/hr", "", "2\"/hr", "3\"/hr"]),
        DataField::AirQuality => (AQI_STOPS, &["GOOD", "MODERATE", "SENSITIVE", "UNHEALTHY", "VERY", "HAZARDOUS"]),
        DataField::Smoke => (SMOKE_STOPS, &["12", "35", "55", "150", "250"]),
        DataField::SmokeVisibility => (SMOKE_STOPS, &["6 MI", "", "1 MI", "", "<1/4"]),
        DataField::FireDanger => (INDEX_STOPS, &["LOW", "MODERATE", "HIGH", "EXTREME"]),
        DataField::Cape => (CAPE_STOPS, &["500", "1000", "2000", "3000", "4000+"]),
        DataField::Stp | DataField::Scp | DataField::SnowSquall | DataField::LakeEffect => (INDEX_STOPS, &["1", "2", "4", "8"]),
        DataField::LapseRate => (LAPSE_STOPS, &["5", "6", "7", "8", "9 °C/km"]),
        DataField::Lcl | DataField::WetBulbZero | DataField::FreezingLevel => (HEIGHT_STOPS, &["1k", "3k", "6k", "10k", "14k ft"]),
        DataField::Omega | DataField::QVector | DataField::TempAdvection | DataField::Frontogenesis => (CONV_STOPS, &["SINKING / COLD", "", "RISING / WARM"]),
        DataField::PrecipType => (PTYPE_STOPS, &["RAIN", "MIX", "SNOW"]),
        DataField::IceAccretion => (ICE_STOPS, &[".01", ".05", ".1", ".25\"/hr"]),
        DataField::SnowRate => (SNOW_CHANCE_STOPS, &["", "0.5", "1", "2\"/hr"]),
        DataField::SnowRatio => (SNOW_CHANCE_STOPS, &["5", "10", "15", "20", "30:1"]),
        DataField::GroundTemp => (SOIL_STOPS, &["10", "20", "32", "40", "50", "60", "70", "80", "90"]),
        DataField::RoadHazard => (ROAD_STOPS, &["WET / FROST", "ICY", "SNOW-COVERED"]),
        DataField::FogRisk => (FOG_STOPS, &["SOME", "LIKELY", "DENSE"]),
        DataField::MaxGust => (WIND_STOPS, &["0", "10", "20", "30", "40", "55", "70"]),
        DataField::IceTotal => (ICE_STOPS, &[".01", ".1", ".25", ".5", "1\""]),
        DataField::DewPoint => (DEW_STOPS, &["10", "30", "45", "55", "62", "68", "75"]),
        DataField::Humidity => (HUMIDITY_STOPS, &["0%", "30%", "50%", "70%", "90%", "100%"]),
        DataField::WindSpeed | DataField::WindGust => (WIND_STOPS, &["0", "10", "20", "30", "40", "55", "70"]),
        DataField::Pressure => (PRESSURE_STOPS, &["980", "996", "1008", "1016", "1028", "1040"]),
        DataField::CloudCover => (CLOUD_STOPS, &["CLEAR", "PARTLY", "OVERCAST"]),
        DataField::RainTotal => (RAIN_TOTAL_STOPS, &[".01", ".1", ".25", ".5", "1\"", "2\"", "3\"", "5\""]),
        DataField::SnowTotal | DataField::Snow10 | DataField::SnowKuchera => (SNOW_TOTAL_STOPS, &["T", "1\"", "3\"", "6\"", "9\"", "12\"", "18\""]),
        DataField::Futurecast => (RAIN_RATE, &["RAIN"]),
    }
}

pub fn rain_ramp() -> &'static [(f32, [u8; 3])] {
    RAIN_RATE
}
pub fn mix_ramp() -> &'static [(f32, [u8; 3])] {
    MIX_RATE
}
pub fn snow_ramp() -> &'static [(f32, [u8; 3])] {
    SNOW_RATE
}
