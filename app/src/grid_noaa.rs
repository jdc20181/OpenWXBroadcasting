//! The model grid read straight from NOAA's GRIB2 files (GFS 0.25° through the relay's
//! NOMADS filter), with no Open-Meteo. Child module of `grid` so it fills the same layers and
//! every map, graphic and derived field works unchanged.
//!
//! GFS publishes every hour to 120 h and every 3 h after. The grid fetches hourly files for the
//! first day, then every 3 h, then every 6 h, and fills the hours between by linear time
//! interpolation (precipitation by spreading the run total evenly across the gap).

use std::collections::HashMap;

use anyhow::{anyhow, ensure, Context, Result};
use futures_util::{stream, StreamExt};
use grib::{projection::Project, LatLons};

use super::{air_quality, dew_point_f, fill_node, node_points, GridModel, NodeSeries, Var, WxGrid, HOURS, NX, NY, VARS};

/// Nodes across and down. NOAA has no per-point cost, so this is 16× the Open-Meteo grid.
pub const NOAA_NX: usize = 40;
pub const NOAA_NY: usize = 24;
/// Hourly files for this many hours from now, then 3-hourly to 120 h, then 6-hourly.
const HOURLY_LEADS: i64 = 24;
/// Files downloaded at once. NOMADS blocks addresses that send too many requests a minute.
const PARALLEL: usize = 2;
const RESOLUTION: f64 = 0.25;
const MAX_LEAD: i64 = 384;

/// The GRIB records the grid uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum F {
    T2,
    Rh2,
    U10,
    V10,
    Gust,
    Mslp,
    Cloud,
    /// Precipitation since the run started (the larger of the two APCP records), mm.
    Apcp,
    Rain,
    Freezing,
    Pellets,
    Snow,
    Cape,
    T850,
    T700,
    T500,
    Rh850,
    U850,
    V850,
    U500,
    V500,
    W700,
    Z850,
    Z500,
    ZFreeze,
    SoilT,
    SoilW,
}

/// Which grid field a GRIB record is, from its discipline, category, number, product template
/// and first fixed surface (type, value). Checked against NOMADS GFS 0.25° files, 2026-11-12.
fn classify(discipline: u8, category: u8, number: u8, template: u16, surface: u8, value: f64) -> Option<F> {
    let at = |want: f64| (value - want).abs() < 0.5;
    Some(match (discipline, category, number) {
        (0, 0, 0) if surface == 103 && at(2.0) => F::T2,
        (0, 1, 1) if surface == 103 && at(2.0) => F::Rh2,
        (0, 2, 2) if surface == 103 && at(10.0) => F::U10,
        (0, 2, 3) if surface == 103 && at(10.0) => F::V10,
        (0, 2, 22) if surface == 1 => F::Gust,
        (0, 3, 1) if surface == 101 => F::Mslp,
        // HRRR / RAP publish MSLP as MSLMA (MAPS system reduction).
        (0, 3, 198) if surface == 101 => F::Mslp,
        // Instantaneous total cloud (GFS also ships a time average, template 8).
        (0, 6, 1) if surface == 10 && template == 0 => F::Cloud,
        (0, 1, 8) if surface == 1 && template == 8 => F::Apcp,
        // Categorical precipitation type, instantaneous (NCEP local numbers and WMO ones).
        (0, 1, 192 | 33) if template == 0 => F::Rain,
        (0, 1, 193 | 34) if template == 0 => F::Freezing,
        (0, 1, 194 | 35) if template == 0 => F::Pellets,
        (0, 1, 195 | 36) if template == 0 => F::Snow,
        (0, 7, 6) if surface == 1 => F::Cape,
        (0, 0, 0) if surface == 100 && at(85000.0) => F::T850,
        (0, 0, 0) if surface == 100 && at(70000.0) => F::T700,
        (0, 0, 0) if surface == 100 && at(50000.0) => F::T500,
        (0, 1, 1) if surface == 100 && at(85000.0) => F::Rh850,
        (0, 2, 2) if surface == 100 && at(85000.0) => F::U850,
        (0, 2, 3) if surface == 100 && at(85000.0) => F::V850,
        (0, 2, 2) if surface == 100 && at(50000.0) => F::U500,
        (0, 2, 3) if surface == 100 && at(50000.0) => F::V500,
        (0, 2, 8) if surface == 100 && at(70000.0) => F::W700,
        (0, 3, 5) if surface == 100 && at(85000.0) => F::Z850,
        (0, 3, 5) if surface == 100 && at(50000.0) => F::Z500,
        (0, 3, 5) if surface == 4 => F::ZFreeze,
        // Land surface: soil temperature and volumetric moisture, 0-10 cm layer.
        (2, 0, 2) if surface == 106 && at(0.0) => F::SoilT,
        (2, 0, 192) if surface == 106 && at(0.0) => F::SoilW,
        _ => return None,
    })
}

/// A Lambert conformal conic grid (GRIB template 3.30), as HRRR and RAP use.
struct LambertSpec {
    projection: grib::projection::Lcc,
    origin: (f64, f64),
    step: (f64, f64),
    ni: usize,
    nj: usize,
    /// Central meridian and cone constant, for turning grid-relative winds to true north.
    lov: f64,
    cone: f64,
}

impl LambertSpec {
    fn new(spec: &grib::def::grib2::template::Template3_30) -> Result<Self> {
        use grib::projection::{Ellipsoid, Lcc, LccParams};
        let deg = |v: f64| v * 1e-6;
        let (a, b) = spec.earth_shape.radii().context("unknown GRIB earth shape")?;
        let (lat1, lat2, lov) = (deg(spec.latin1 as f64), deg(spec.latin2 as f64), deg(spec.lov as f64));
        let projection = Lcc::new(&LccParams { ellipsoid: Ellipsoid::from_a_and_b(a, b), lat_0: deg(spec.lad as f64), lon_0: lov, lat_1: lat1, lat_2: lat2 }).map_err(anyhow::Error::msg)?;
        let origin = projection
            .project(&(deg(spec.first_point_lon as f64).to_radians(), deg(spec.first_point_lat as f64).to_radians()), false)
            .map_err(anyhow::Error::msg)?;
        let step = (
            spec.dx as f64 * if spec.scanning_mode.scans_positively_for_i() { 0.001 } else { -0.001 },
            spec.dy as f64 * if spec.scanning_mode.scans_positively_for_j() { 0.001 } else { -0.001 },
        );
        let (ni, nj) = (spec.ni as usize, spec.nj as usize);
        ensure!(ni > 1 && nj > 1 && ni * nj <= 4_000_000 && step.0 != 0.0 && step.1 != 0.0, "NOAA regional grid has an invalid size");
        Ok(Self { projection, origin, step, ni, nj, lov, cone: cone_constant(lat1, lat2) })
    }
}

/// Lambert cone constant from the two standard parallels (degrees).
fn cone_constant(lat1: f64, lat2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    if (lat1 - lat2).abs() < 1e-6 {
        return p1.sin();
    }
    let t = |p: f64| (std::f64::consts::FRAC_PI_4 + p / 2.0).tan();
    (p1.cos().ln() - p2.cos().ln()) / (t(p2).ln() - t(p1).ln())
}

/// Grid-relative (u, v) at longitude `lon` on a Lambert grid → earth-relative (east, north).
fn earth_wind(u: f32, v: f32, lon: f64, lov: f64, cone: f64) -> (f32, f32) {
    let d = ((lon - lov + 540.0).rem_euclid(360.0) - 180.0).to_radians();
    let alpha = (cone * d) as f32;
    let (s, c) = alpha.sin_cos();
    (c * u + s * v, -s * u + c * v)
}

/// Bilinear weights from a regular lat/lon grid to the grid nodes.
struct Sampler {
    /// Per node: four (cell index, weight) pairs, or None outside the file's area.
    taps: Vec<Option<[(usize, f32); 4]>>,
}

impl Sampler {
    fn new(north: f64, west: f64, width: usize, height: usize, points: &[(f64, f64)]) -> Self {
        let taps = points
            .iter()
            .map(|&(lat, lon)| {
                let x = (lon - west) / RESOLUTION;
                let y = (north - lat) / RESOLUTION;
                if x < 0.0 || y < 0.0 || x > (width - 1) as f64 || y > (height - 1) as f64 {
                    return None;
                }
                let (x0, y0) = (x.floor() as usize, y.floor() as usize);
                let (x1, y1) = ((x0 + 1).min(width - 1), (y0 + 1).min(height - 1));
                let (dx, dy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
                Some([
                    (y0 * width + x0, (1.0 - dx) * (1.0 - dy)),
                    (y0 * width + x1, dx * (1.0 - dy)),
                    (y1 * width + x0, (1.0 - dx) * dy),
                    (y1 * width + x1, dx * dy),
                ])
            })
            .collect();
        Self { taps }
    }

    /// Bilinear weights on a Lambert conformal grid (HRRR, RAP): each node is projected into the
    /// grid's x / y and sampled between the four surrounding cells (row-major `j * ni + i`).
    fn lambert(spec: &LambertSpec, points: &[(f64, f64)]) -> Self {
        let taps = points
            .iter()
            .map(|&(lat, lon)| {
                let (x, y) = spec.projection.project(&(lon.to_radians(), lat.to_radians()), false).ok()?;
                let x = (x - spec.origin.0) / spec.step.0;
                let y = (y - spec.origin.1) / spec.step.1;
                let (w, h) = (spec.ni, spec.nj);
                if x < 0.0 || y < 0.0 || x > (w - 1) as f64 || y > (h - 1) as f64 {
                    return None;
                }
                let (x0, y0) = (x.floor() as usize, y.floor() as usize);
                let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
                let (dx, dy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
                Some([
                    (y0 * w + x0, (1.0 - dx) * (1.0 - dy)),
                    (y0 * w + x1, dx * (1.0 - dy)),
                    (y1 * w + x0, (1.0 - dx) * dy),
                    (y1 * w + x1, dx * dy),
                ])
            })
            .collect();
        Self { taps }
    }

    fn sample(&self, cells: &[f32]) -> Vec<f32> {
        self.taps
            .iter()
            .map(|taps| {
                // Missing cells (soil over lakes) are skipped and the rest re-weighted, so a
                // shoreline node takes the land value; mostly-missing stays missing.
                let (mut total, mut weight) = (0.0, 0.0);
                for &(k, w) in taps.iter().flatten() {
                    if cells[k].is_finite() {
                        total += cells[k] * w;
                        weight += w;
                    }
                }
                if weight > 0.5 { total / weight } else { f32::NAN }
            })
            .collect()
    }
}

/// Decode one GFS file and sample every field the grid uses at the nodes.
fn decode(bytes: &[u8], points: &[(f64, f64)]) -> Result<HashMap<F, Vec<f32>>> {
    let message = grib::from_reader(std::io::Cursor::new(bytes))?;
    let mut out: HashMap<F, Vec<f32>> = HashMap::new();
    // The file's grid (shared by every record): cell index of each value, and the node sampler.
    let mut layout: Option<(Vec<usize>, usize, Sampler)> = None;
    // (central meridian, cone constant) when the file is on a Lambert grid.
    let mut lambert: Option<(f64, f64)> = None;
    for (_, field) in message.iter() {
        let discipline = field.indicator().discipline;
        let def = field.prod_def();
        let (Some(category), Some(number)) = (def.parameter_category(), def.parameter_number()) else { continue };
        let Some((level, _)) = def.fixed_surfaces() else { continue };
        let Some(kind) = classify(discipline, category, number, def.prod_tmpl_num(), level.surface_type, level.value() as f64) else { continue };
        // HRRR / RAP: a Lambert grid; values are laid out row by row from the (i, j) indices.
        if layout.is_none() {
            if let Ok(grib::GridDefinitionTemplateValues::Template30(spec)) = grib::GridDefinitionTemplateValues::try_from(field.grid_def()) {
                let spec = LambertSpec::new(&spec)?;
                let index = field.ij()?.map(|(i, j)| j * spec.ni + i).collect::<Vec<_>>();
                ensure!(index.iter().all(|k| *k < spec.ni * spec.nj), "NOAA regional grid indices are out of range");
                let sampler = Sampler::lambert(&spec, points);
                layout = Some((index, spec.ni * spec.nj, sampler));
                lambert = Some((spec.lov, spec.cone));
            }
        }
        if layout.is_none() {
            let coordinates: Vec<(f64, f64)> = field.latlons()?.map(|(lat, lon)| (lat as f64, (lon as f64 + 180.0).rem_euclid(360.0) - 180.0)).collect();
            ensure!(!coordinates.is_empty() && coordinates.len() <= 400_000, "NOAA GFS region is empty or too large");
            let north = coordinates.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
            let south = coordinates.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
            let east = coordinates.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
            let west = coordinates.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
            let width = ((east - west) / RESOLUTION).round() as usize + 1;
            let height = ((north - south) / RESOLUTION).round() as usize + 1;
            ensure!(width * height == coordinates.len(), "NOAA GFS region is not a rectangular 0.25° grid");
            let index = coordinates
                .iter()
                .map(|(lat, lon)| ((north - lat) / RESOLUTION).round() as usize * width + ((lon - west) / RESOLUTION).round() as usize)
                .collect();
            layout = Some((index, width * height, Sampler::new(north, west, width, height, points)));
        }
        let (index, cells_len, sampler) = layout.as_ref().context("no grid")?;
        let decoder = grib::Grib2SubmessageDecoder::from(field)?;
        let mut cells = vec![f32::NAN; *cells_len];
        let mut count = 0;
        for (k, value) in index.iter().zip(decoder.dispatch()?) {
            cells[*k] = value;
            count += 1;
        }
        ensure!(count == *cells_len, "NOAA GFS records use different grids");
        let nodes = sampler.sample(&cells);
        match out.get_mut(&kind) {
            // APCP comes twice (the 6-hour bucket and the run total). The run total contains
            // the bucket, so the larger value is the run total.
            Some(previous) if kind == F::Apcp => {
                for (p, v) in previous.iter_mut().zip(nodes) {
                    if v.is_finite() && (!p.is_finite() || v > *p) {
                        *p = v;
                    }
                }
            }
            _ => {
                out.insert(kind, nodes);
            }
        }
    }
    ensure!(out.contains_key(&F::T2), "NOAA model file has no 2 m temperature");
    // Lambert-grid winds point along the grid's axes: turn them to true north and east.
    if let (Some((lov, cone)), Some(u), Some(v)) = (lambert, out.get(&F::U10).cloned(), out.get(&F::V10).cloned()) {
        let (mut east, mut north) = (Vec::with_capacity(u.len()), Vec::with_capacity(v.len()));
        for ((uu, vv), (_, lon)) in u.iter().zip(&v).zip(points) {
            let (e, n) = earth_wind(*uu, *vv, *lon, lov, cone);
            east.push(e);
            north.push(n);
        }
        out.insert(F::U10, east);
        out.insert(F::V10, north);
    }
    Ok(out)
}

/// The last lead for `hours` from `first`: within the run, and on a published hour (past 120 h
/// GFS files exist only every 3 h).
fn last_lead(model: GridModel, first: i64, hours: u32) -> i64 {
    let last = (first + i64::from(hours)).min(max_lead(model));
    if last > 120 { last / 3 * 3 } else { last }
}

/// Models the grid reads from NOAA, and their relay source name.
pub fn noaa_source(model: GridModel) -> Option<&'static str> {
    match model {
        GridModel::Gfs => Some("gfs"),
        GridModel::Hrrr => Some("hrrr"),
        _ => None,
    }
}

/// False after HRRR from NOAA failed (relay not updated yet) and HRRR fell back to Open-Meteo;
/// true again once a NOAA HRRR grid loads. Keeps the on-air credit honest.
static HRRR_FROM_NOAA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_hrrr_from_noaa(ok: bool) {
    HRRR_FROM_NOAA.store(ok, std::sync::atomic::Ordering::Relaxed);
    if !ok {
        HRRR_FAILED_AT.store(chrono::Utc::now().timestamp(), std::sync::atomic::Ordering::Relaxed);
    }
}

static HRRR_FAILED_AT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// False for 30 minutes after a NOAA HRRR file was missing; the map shows GFS meanwhile.
pub fn hrrr_available() -> bool {
    HRRR_FROM_NOAA.load(std::sync::atomic::Ordering::Relaxed)
        || chrono::Utc::now().timestamp() - HRRR_FAILED_AT.load(std::sync::atomic::Ordering::Relaxed) > 30 * 60
}

/// Whether a grid of this model over these bounds really comes from NOAA (for credits).
pub fn from_noaa(model: GridModel, bounds: (f64, f64, f64, f64)) -> bool {
    // Every map grid is NOAA now (HRRR, or GFS when HRRR can't cover the view).
    let _ = (model, bounds);
    true
}

fn max_lead(model: GridModel) -> i64 {
    if model == GridModel::Hrrr { 48 } else { MAX_LEAD }
}

/// Whether a region can be read from NOAA for this model: GFS anywhere; HRRR inside the
/// continental U.S. and within the relay's 20° x 30° cap (after it snaps out to whole 5°).
pub fn fits(model: GridModel, (north, south, west, east): (f64, f64, f64, f64)) -> bool {
    match model {
        GridModel::Gfs => true,
        GridModel::Hrrr => {
            let (s, w) = ((south / 5.0).floor() * 5.0, (west / 5.0).floor() * 5.0);
            let (n, e) = ((north / 5.0).ceil() * 5.0, (east / 5.0).ceil() * 5.0);
            s >= 15.0 && n <= 60.0 && w >= -135.0 && e <= -55.0 && n - s <= 20.0 && e - w <= 30.0
        }
        _ => false,
    }
}

/// Lead hours to download for the hours `first..=last` (absolute hours from the run start).
fn leads(first: i64, last: i64) -> Vec<i64> {
    let mut leads: Vec<i64> = ((first - 1).max(1)..=(first + HOURLY_LEADS).min(last)).collect();
    let mut lead = first + HOURLY_LEADS;
    while lead < last {
        lead += if lead < 120 { 3 } else { 6 };
        // Past 120 h GFS files exist only every 3 h.
        let snapped = if lead > 120 { lead / 3 * 3 } else { lead };
        leads.push(snapped.min(last));
    }
    leads.push(last);
    leads.sort_unstable();
    leads.dedup();
    leads
}

/// The WMO weather code the categorical type, rate (in/h) and cloud (%) describe, so the
/// precipitation-type logic treats NOAA and Open-Meteo hours alike.
fn weather_code(rate: f32, rain: bool, freezing: bool, pellets: bool, snow: bool, cloud: f32) -> f32 {
    if rate < 0.004 {
        return match cloud {
            c if c < 20.0 => 0.0,
            c if c < 50.0 => 1.0,
            c if c < 85.0 => 2.0,
            _ => 3.0,
        };
    }
    if freezing {
        return if rate < 0.1 { 66.0 } else { 67.0 };
    }
    if pellets {
        return 79.0;
    }
    match (snow, rain) {
        (true, true) => 68.0,
        (true, false) => {
            if rate < 0.04 { 71.0 } else if rate < 0.1 { 73.0 } else { 75.0 }
        }
        _ => {
            if rate < 0.1 { 61.0 } else if rate < 0.3 { 63.0 } else { 65.0 }
        }
    }
}

/// Files done / total for the NOAA grid download in progress (0 total when idle).
static PROGRESS: (std::sync::atomic::AtomicUsize, std::sync::atomic::AtomicUsize) =
    (std::sync::atomic::AtomicUsize::new(0), std::sync::atomic::AtomicUsize::new(0));

/// (files done, files total) while a NOAA grid is downloading, for "Loading GFS: 14 of 40".
pub fn progress() -> Option<(usize, usize)> {
    use std::sync::atomic::Ordering;
    let total = PROGRESS.1.load(Ordering::Relaxed);
    (total > 0).then(|| (PROGRESS.0.load(Ordering::Relaxed).min(total), total))
}

/// The map grid for a view (`fetch` in the parent module).
pub(super) async fn fetch(bounds: (f64, f64, f64, f64), model: GridModel, zoom: f64) -> Result<WxGrid> {
    let mut grid = fetch_box(bounds, NOAA_NX, NOAA_NY, model, HOURS).await?;
    grid.zoom = zoom;
    Ok(grid)
}

/// A grid of `nx` × `ny` nodes over (north, south, west, east) for the next `hours` hours,
/// read from NOAA. Used by the map grid and the Winter Studio.
pub async fn fetch_box((north, south, west, east): (f64, f64, f64, f64), nx: usize, ny: usize, model: GridModel, hours: u32) -> Result<WxGrid> {
    let source = noaa_source(model).with_context(|| format!("{} is not read from NOAA yet", model.label()))?;
    ensure!(fits(model, (north, south, west, east)), "Zoom in: NOAA {} covers the continental U.S. in views up to about 20° by 30°", model.label());
    ensure!(nx >= 2 && ny >= 2, "grid needs at least 2 × 2 nodes");
    let points = node_points((north, south, west, east), nx, ny);
    let timeout = std::time::Duration::from_secs(60);
    let run = crate::net::broadcaster_route("model-run", timeout).await?.json::<serde_json::Value>().await?["run"]
        .as_i64()
        .context("The relay sent no NOAA model run")?;
    let now = chrono::Utc::now().timestamp();
    ensure!((0..=24 * 3600).contains(&(now - run)), "The latest NOAA run on the relay is out of date");
    // Start one hour back, like the point forecasts, so "now" is inside the grid.
    let first = ((now - run).div_euclid(3600) - 1).max(1);
    let last = last_lead(model, first, hours);
    let leads = leads(first, last);
    // The relay snaps the region outward to whole 5° and asks NOMADS for just these fields.
    let region = format!("south={}&west={}&north={}&east={}", south.floor(), west.floor(), north.ceil(), east.ceil());
    {
        use std::sync::atomic::Ordering;
        PROGRESS.0.store(0, Ordering::Relaxed);
        PROGRESS.1.store(leads.len(), Ordering::Relaxed);
    }
    let outcomes: Vec<(i64, Result<HashMap<F, Vec<f32>>>)> = stream::iter(leads.clone())
        .map(|lead| {
            let path = format!("model-frame?source={source}&fields=grid&run={run}&lead={lead}&{region}");
            let points = points.clone();
            async move {
                // A busy relay (503) or a NOMADS throttle fails one file now and then: try it again
                // after a short wait before giving up on it. A decode error is not retried.
                let mut result = Err(anyhow!("not fetched"));
                for (attempt, wait) in [0u64, 3, 8, 15].into_iter().enumerate() {
                    if wait > 0 {
                        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                    }
                    let fetched = async { anyhow::Ok(crate::net::broadcaster_route(&path, timeout).await?.bytes().await?) }.await;
                    match fetched {
                        Ok(bytes) => {
                            let points = points.clone();
                            result = tokio::task::spawn_blocking(move || decode(&bytes, &points)).await.map_err(anyhow::Error::from).and_then(|r| r);
                            break;
                        }
                        Err(e) => {
                            log::warn!("NOAA {source} file +{lead} h, try {}: {e:#}", attempt + 1);
                            result = Err(e);
                        }
                    }
                }
                PROGRESS.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                (lead, result)
            }
        })
        .buffer_unordered(PARALLEL)
        .collect()
        .await;
    PROGRESS.1.store(0, std::sync::atomic::Ordering::Relaxed);
    let mut frames: Vec<(i64, HashMap<F, Vec<f32>>)> = Vec::new();
    let mut first_error = None;
    for (lead, outcome) in outcomes {
        match outcome {
            Ok(frame) => frames.push((lead, frame)),
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }
    frames.sort_by_key(|(lead, _)| *lead);
    // An older relay ignores fields=grid and sends only the precipitation fields.
    if frames.len() != leads.len() && first_error.as_ref().is_some_and(|e| e.to_string().contains("no 2 m temperature")) {
        return Err(anyhow!("The relay is older than this app: update OpenWX Relay so NOAA map grids get every field"));
    }
    // A gap would be filled by interpolation and hide a real change, so the grid keeps only the
    // unbroken run of files from the first one: if a later file still failed after its retries,
    // the map ends at the hour before it instead of showing nothing at all.
    let unbroken = leads.iter().zip(frames.iter()).take_while(|(want, (got, _))| *want == got).count();
    if unbroken < 2 {
        let missing = leads.len() - frames.len();
        return Err(first_error.unwrap_or_else(|| anyhow!("missing files")).context(format!("{missing} of {} NOAA {} files failed", leads.len(), model.label())));
    }
    let last = if unbroken < leads.len() {
        let end = leads[unbroken - 1];
        log::warn!("NOAA {} grid ends at +{end} h: {} of {} files failed ({:#})", model.label(), leads.len() - frames.len(), leads.len(), first_error.as_ref().map(|e| e.to_string()).unwrap_or_default());
        frames.truncate(unbroken);
        end
    } else {
        last
    };
    build(model, run, first, last, &frames, nx, ny, (north, south, west, east))
}

/// Assemble the hourly grid from the downloaded files.
fn build(model: GridModel, run: i64, first: i64, last: i64, frames: &[(i64, HashMap<F, Vec<f32>>)], nx: usize, ny: usize, (north, south, west, east): (f64, f64, f64, f64)) -> Result<WxGrid> {
    let hours: Vec<i64> = (first..=last).collect();
    let nt = hours.len();
    let nodes = nx * ny;
    // Value of a field at absolute hour `h` and node `p`: linear between the bracketing files;
    // `step` takes the file at or after `h` instead (precipitation type).
    let value = |field: F, h: i64, p: usize, step: bool| -> f32 {
        let at = frames.partition_point(|(lead, _)| *lead < h);
        let get = |k: usize| frames.get(k).and_then(|(_, f)| f.get(&field)).and_then(|v| v.get(p)).copied().unwrap_or(f32::NAN);
        match frames.get(at) {
            Some((lead, _)) if *lead == h || step || at == 0 => get(at),
            Some((b, _)) => {
                let (a, _) = &frames[at - 1];
                let w = (h - a) as f32 / (b - a) as f32;
                get(at - 1) * (1.0 - w) + get(at) * w
            }
            None => get(frames.len() - 1),
        }
    };
    let k_to_f = |k: f32| (k - 273.15) * 1.8 + 32.0;
    let mph = 2.236_94f32;
    let speed_dir = |u: f32, v: f32| ((u.hypot(v) * mph), (f32::atan2(-u, -v).to_degrees() + 360.0) % 360.0);
    let mut data = vec![vec![0f32; nt * nodes]; VARS];
    for p in 0..nodes {
        let mut s = NodeSeries::default();
        let mut ext: Vec<(Var, Vec<f32>)> = [Var::Cape, Var::T850, Var::Td850, Var::T700, Var::T500, Var::Omega700, Var::Z850, Var::Z500, Var::FreezingLevel, Var::SoilTemp, Var::SoilMoist]
            .into_iter()
            .map(|v| (v, Vec::with_capacity(nt)))
            .collect();
        for &h in &hours {
            let temp = k_to_f(value(F::T2, h, p, false));
            let (wind, dir) = speed_dir(value(F::U10, h, p, false), value(F::V10, h, p, false));
            let cloud = value(F::Cloud, h, p, false);
            // Liquid in the hour ending at `h`, inches (APCP is kg/m², i.e. mm).
            let rate = ((value(F::Apcp, h, p, false) - value(F::Apcp, h - 1, p, false)) / 25.4).max(0.0);
            let rate = if rate.is_finite() { rate } else { 0.0 };
            let flag = |f: F| value(f, h, p, true) > 0.5;
            let (rain, freezing, pellets, snow) = (flag(F::Rain), flag(F::Freezing), flag(F::Pellets), flag(F::Snow));
            let snow_share = match (snow, rain || freezing || pellets) {
                (true, false) => 1.0,
                (true, true) => 0.5,
                _ => 0.0,
            };
            s.temp.push(temp);
            s.rh.push(value(F::Rh2, h, p, false));
            s.wind.push(wind);
            s.dir.push(dir);
            s.gust.push(value(F::Gust, h, p, false) * mph);
            s.pres.push(value(F::Mslp, h, p, false) / 100.0);
            s.cloud.push(cloud);
            s.precip.push(rate);
            // The grid keeps snowfall at 7:1 (as Open-Meteo sends it); 10:1 and Kuchera totals are
            // derived from it.
            s.snow.push(rate * snow_share * 7.0);
            s.code.push(weather_code(rate, rain, freezing, pellets, snow, cloud));
            let t850 = k_to_f(value(F::T850, h, p, false));
            let (ws850, wd850) = speed_dir(value(F::U850, h, p, false), value(F::V850, h, p, false));
            let (ws500, wd500) = speed_dir(value(F::U500, h, p, false), value(F::V500, h, p, false));
            s.ws850.push(ws850);
            s.wd850.push(wd850);
            s.ws500.push(ws500);
            s.wd500.push(wd500);
            for (var, series) in ext.iter_mut() {
                series.push(match var {
                    Var::Cape => value(F::Cape, h, p, false),
                    Var::T850 => t850,
                    Var::Td850 => dew_point_f(t850, value(F::Rh850, h, p, false)),
                    Var::T700 => k_to_f(value(F::T700, h, p, false)),
                    Var::T500 => k_to_f(value(F::T500, h, p, false)),
                    // Omega (Pa/s, sinking positive) to microbars per second, rising positive.
                    Var::Omega700 => -value(F::W700, h, p, false) * 10.0,
                    Var::Z850 => value(F::Z850, h, p, false),
                    Var::Z500 => value(F::Z500, h, p, false),
                    Var::FreezingLevel => value(F::ZFreeze, h, p, false),
                    Var::SoilTemp => k_to_f(value(F::SoilT, h, p, false)),
                    _ => value(F::SoilW, h, p, false),
                });
            }
        }
        s.ext = std::mem::take(&mut ext);
        fill_node(&mut data, (nt, nx, ny), (p / nx, p % nx), &s, true);
    }
    // Air-quality layers stay missing until `add_air_quality` fills them.
    for var in [Var::Aqi, Var::Pm25] {
        data[var as usize].fill(f32::NAN);
    }
    Ok(WxGrid {
        north,
        south,
        west,
        east,
        nx,
        ny,
        times: hours.iter().map(|h| run + h * 3600).collect(),
        data,
        fetched_at: chrono::Utc::now().timestamp(),
        extended: model == GridModel::Gfs,
        air: false,
        zoom: 0.0,
        model,
        derived: Default::default(),
        analysis: Default::default(),
    })
}

/// Fill the air-quality layers from Open-Meteo's air-quality API (no NOAA source covers US AQI
/// and PM2.5 together yet). Fetched on the coarse 10 × 6 grid to keep the request small, then
/// interpolated to the nodes.
pub(super) async fn add_air_quality(grid: &mut WxGrid) {
    let coarse = node_points((grid.north, grid.south, grid.west, grid.east), NX, NY);
    let Some(series) = air_quality(&coarse, &grid.times).await else { return };
    let (nx, ny, nt) = (grid.nx, grid.ny, grid.hours());
    for j in 0..ny {
        for i in 0..nx {
            let gx = i as f32 / (nx - 1) as f32 * (NX - 1) as f32;
            let gy = j as f32 / (ny - 1) as f32 * (NY - 1) as f32;
            let (i0, j0) = (gx.floor() as usize, gy.floor() as usize);
            let (i1, j1) = ((i0 + 1).min(NX - 1), (j0 + 1).min(NY - 1));
            let (fx, fy) = (gx - i0 as f32, gy - j0 as f32);
            for (slot, var) in [(0, Var::Aqi), (1, Var::Pm25)] {
                for t in 0..nt {
                    let c = |jj: usize, ii: usize| series[jj * NX + ii][slot][t];
                    let v = (c(j0, i0) * (1.0 - fx) + c(j0, i1) * fx) * (1.0 - fy) + (c(j1, i0) * (1.0 - fx) + c(j1, i1) * fx) * fy;
                    grid.data[var as usize][(t * ny + j) * nx + i] = v;
                }
            }
        }
    }
    grid.air = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lead_schedule_is_valid_for_gfs() {
        let l = leads(8, last_lead(GridModel::Gfs, 8, 170));
        assert_eq!(l.first(), Some(&7));
        assert_eq!(l.last(), Some(&177));
        assert!(l.windows(2).all(|w| w[0] < w[1]));
        // Hourly for the first day.
        assert!((7..=32).all(|h| l.contains(&h)));
        // Past 120 h only files that exist (every 3 h).
        assert!(l.iter().all(|&h| h <= 120 || h % 3 == 0), "{l:?}");
        assert!(l.len() < 80, "{} files", l.len());
    }

    #[test]
    fn weather_codes_match_the_type_rules() {
        assert_eq!(weather_code(0.0, false, false, false, false, 10.0), 0.0);
        assert_eq!(weather_code(0.05, false, true, false, false, 100.0), 66.0);
        assert_eq!(weather_code(0.05, false, false, false, true, 100.0), 73.0);
        assert_eq!(weather_code(0.2, true, false, false, false, 100.0), 63.0);
        assert_eq!(weather_code(0.02, true, false, false, true, 100.0), 68.0);
    }

    #[test]
    fn sampler_interpolates_inside_and_is_missing_outside() {
        // 2 × 2 cells: north-west 0, north-east 1, south-west 2, south-east 3.
        let s = Sampler::new(40.0, -90.0, 2, 2, &[(39.875, -89.875), (41.0, -90.0)]);
        let v = s.sample(&[0.0, 1.0, 2.0, 3.0]);
        assert!((v[0] - 1.5).abs() < 1e-5);
        assert!(v[1].is_nan());
    }

    /// Two consecutive NOMADS "grid" files (`CWX_GFS_GRID_DIR` holds gfs_grid_f009.grib2 and
    /// gfs_grid_f010.grib2 for 35-45°N, 90-80°W).
    #[test]
    fn decodes_real_gfs_grid_files() {
        let Some(dir) = std::env::var_os("CWX_GFS_GRID_DIR").map(std::path::PathBuf::from) else { return };
        let points = node_points((42.0, 37.0, -89.0, -84.0), 6, 4);
        let read = |lead: i64| decode(&std::fs::read(dir.join(format!("gfs_grid_f{lead:03}.grib2"))).unwrap(), &points).unwrap();
        let (a, b) = (read(9), read(10));
        let all = [F::T2, F::Rh2, F::U10, F::V10, F::Gust, F::Mslp, F::Cloud, F::Apcp, F::Rain, F::Freezing, F::Pellets, F::Snow, F::Cape, F::T850, F::T700, F::T500, F::Rh850, F::U850, F::V850, F::U500, F::V500, F::W700, F::Z850, F::Z500, F::ZFreeze, F::SoilT, F::SoilW];
        for f in all {
            let v = a.get(&f).unwrap_or_else(|| panic!("{f:?} missing"));
            // Soil has no value over water (a node sits in Lake Michigan).
            let gaps = v.iter().filter(|x| !x.is_finite()).count();
            let allowed = if matches!(f, F::SoilT | F::SoilW) { 2 } else { 0 };
            assert!(gaps <= allowed, "{f:?} has {gaps} gaps: {v:?}");
        }
        let range = |f: F, lo: f32, hi: f32| assert!(a[&f].iter().filter(|x| x.is_finite()).all(|x| (lo..=hi).contains(x)), "{f:?} out of range: {:?}", a[&f]);
        range(F::T2, 230.0, 320.0);
        range(F::Mslp, 95_000.0, 106_000.0);
        range(F::Z500, 4800.0, 6000.0);
        range(F::Z850, 900.0, 1700.0);
        range(F::SoilT, 250.0, 315.0);
        range(F::SoilW, 0.0, 0.6);
        range(F::Cloud, 0.0, 100.0);
        // The run total never shrinks from one hour to the next.
        assert!(a[&F::Apcp].iter().zip(&b[&F::Apcp]).all(|(x, y)| y >= x));
        let frames = vec![(9, a), (10, b)];
        let grid = build(GridModel::Gfs, 0, 10, 10, &frames, 6, 4, (42.0, 37.0, -89.0, -84.0)).unwrap();
        assert_eq!(grid.hours(), 1);
        let t = grid.data[Var::Temp as usize][0];
        assert!((-40.0..110.0).contains(&t), "{t}");
        assert!(grid.data[Var::Omega700 as usize].iter().all(|w| w.abs() < 200.0));
        eprintln!("T2 {t:.1}F, soil {:.1}F, freezing level {:.0} m", grid.data[Var::SoilTemp as usize][0], grid.data[Var::FreezingLevel as usize][0]);
    }

    /// HRRR (Lambert grid) and GFS files for the same region and hour agree once the HRRR winds
    /// are turned to true north. Files from the relay's model-frame?fields=grid in
    /// CWX_HRRR_GRID_DIR (hrrr-grid.grib2, gfs-grid.grib2).
    #[test]
    fn hrrr_lambert_grid_matches_gfs() {
        let Some(dir) = std::env::var_os("CWX_HRRR_GRID_DIR").map(std::path::PathBuf::from) else { return };
        let points = node_points((40.5, 37.5, -89.5, -84.5), 8, 5);
        let hrrr = decode(&std::fs::read(dir.join("hrrr-grid.grib2")).unwrap(), &points).unwrap();
        let gfs = decode(&std::fs::read(dir.join("gfs-grid.grib2")).unwrap(), &points).unwrap();
        for f in [F::T2, F::Rh2, F::U10, F::V10, F::Gust, F::Mslp, F::Cloud, F::Apcp, F::Rain, F::Freezing, F::Pellets, F::Snow, F::Cape] {
            let v = hrrr.get(&f).unwrap_or_else(|| panic!("HRRR {f:?} missing"));
            assert!(v.iter().all(|x| x.is_finite()), "HRRR {f:?} has gaps: {v:?}");
        }
        let mean_abs = |f: F| hrrr[&f].iter().zip(&gfs[&f]).map(|(a, b)| (a - b).abs()).sum::<f32>() / points.len() as f32;
        assert!(mean_abs(F::T2) < 4.0, "T2 differs by {} K", mean_abs(F::T2));
        assert!(mean_abs(F::Mslp) < 400.0, "MSLP differs by {} Pa", mean_abs(F::Mslp));
        // Wind direction agrees where the wind is not light (a wrong rotation shows up here).
        let dir = |u: f32, v: f32| f32::atan2(-u, -v).to_degrees();
        let diffs: Vec<f32> = (0..points.len())
            .filter(|&p| hrrr[&F::U10][p].hypot(hrrr[&F::V10][p]) > 3.0 && gfs[&F::U10][p].hypot(gfs[&F::V10][p]) > 3.0)
            .map(|p| {
                let d = (dir(hrrr[&F::U10][p], hrrr[&F::V10][p]) - dir(gfs[&F::U10][p], gfs[&F::V10][p])).rem_euclid(360.0);
                d.min(360.0 - d)
            })
            .collect();
        if !diffs.is_empty() {
            let mean = diffs.iter().sum::<f32>() / diffs.len() as f32;
            assert!(mean < 35.0, "wind directions differ by {mean}° on average");
        }
        eprintln!("HRRR vs GFS: T2 {:.2} K, MSLP {:.0} Pa, wind dir {:?}", mean_abs(F::T2), mean_abs(F::Mslp), diffs.iter().sum::<f32>() / diffs.len().max(1) as f32);
    }

    #[test]
    fn lambert_wind_rotation() {
        // HRRR: both standard parallels 38.5° N, central meridian 97.5° W.
        let n = cone_constant(38.5, 38.5);
        assert!((n - 38.5f64.to_radians().sin()).abs() < 1e-12);
        // On the central meridian the grid is north-up: nothing turns.
        let (e, nn) = earth_wind(3.0, 4.0, -97.5, -97.5, n);
        assert!((e - 3.0).abs() < 1e-5 && (nn - 4.0).abs() < 1e-5);
        // East of it the grid's +y leans east of north, so a grid-north wind gains an east part;
        // the speed never changes.
        let (e, nn) = earth_wind(0.0, 10.0, -85.0, -97.5, n);
        assert!(e > 0.0 && ((e * e + nn * nn).sqrt() - 10.0).abs() < 1e-4, "{e} {nn}");
        assert!(fits(GridModel::Hrrr, (41.0, 37.0, -90.0, -84.0)));
        assert!(!fits(GridModel::Hrrr, (50.0, 25.0, -125.0, -67.0)));
        assert!(!fits(GridModel::Ecmwf, (41.0, 37.0, -90.0, -84.0)));
    }

    #[test]
    #[ignore = "downloads live GFS files from NOAA through the relay"]
    fn live_gfs_grid() {
        let grid = crate::net::runtime().block_on(fetch_box((42.0, 37.0, -89.0, -84.0), 12, 8, GridModel::Gfs, 30)).unwrap();
        let t = grid.time_index(0.0);
        let temp = grid.sample(Var::Temp, t, 39.0, -86.5).unwrap();
        assert!((-40.0..120.0).contains(&temp), "{temp}");
        assert!(grid.sample(Var::Pressure, t, 39.0, -86.5).is_some_and(|p| (950.0..1060.0).contains(&p)));
        assert!(grid.sample(Var::SoilTemp, t, 39.0, -86.5).is_some_and(f32::is_finite));
        eprintln!("hours {} temp {temp:.1}F", grid.hours());
    }
}
