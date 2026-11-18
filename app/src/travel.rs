//! Traffic and travel: weather / traffic cameras (any still-image URL), highway travel-time
//! estimates from distance, weather along the route and INDOT incidents, and a multi-source
//! storm intensity comparison (radar, lightning, reports, warnings).

use serde::{Deserialize, Serialize};

use crate::{
    config::{LibraryPaths, Settings},
    data::{RoadEvent, WxSnapshot},
    map::{geo::haversine_km, scene::DataField},
};

const KM_PER_MI: f64 = 1.609_344;

/// A camera that publishes a still image at a URL (DOT traffic cams, school / TV weather cams).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WebCam {
    pub name: String,
    pub url: String,
    pub lat: f64,
    pub lon: f64,
    /// PTZ control URL with {move} (left, right, up, down, home, zoomin, zoomout), e.g.
    /// http://cam/axis-cgi/com/ptz.cgi?move={move}. Empty = fixed camera.
    pub ptz: String,
    /// Credit line shown with the image.
    pub credit: String,
    /// Station tags ("Downtown", "Interstate").
    pub tags: Vec<String>,
}

/// Health and conditions of a camera from its last refresh.
#[derive(Clone, Debug, Default)]
pub struct CamStatus {
    pub last_ok: Option<i64>,
    pub error: String,
    /// "Night", "Fog / low visibility", "Clear view", from the image itself.
    pub condition: String,
}

fn status_store() -> &'static std::sync::Mutex<std::collections::HashMap<String, CamStatus>> {
    static S: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, CamStatus>>> = std::sync::OnceLock::new();
    S.get_or_init(Default::default)
}

pub fn cam_status(name: &str) -> CamStatus {
    status_store().lock().ok().and_then(|m| m.get(name).cloned()).unwrap_or_default()
}

/// Tag the view from the picture: dark = night, flat and bright-grey = fog or low visibility.
fn classify(img: &image::RgbImage) -> String {
    let (mut sum, mut sq, mut n) = (0.0f64, 0.0f64, 0.0f64);
    for p in img.pixels().step_by(17) {
        let l = 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
        sum += l;
        sq += l * l;
        n += 1.0;
    }
    let mean = sum / n.max(1.0);
    let sd = (sq / n.max(1.0) - mean * mean).max(0.0).sqrt();
    if mean < 45.0 {
        "Night".into()
    } else if sd < 28.0 && mean > 110.0 {
        "Fog / low visibility".into()
    } else {
        "Clear view".into()
    }
}

/// Folder of archived snapshots for a camera.
pub fn archive_dir(name: &str) -> std::path::PathBuf {
    LibraryPaths::get().media.join("Camera archive").join(cam_file(name).trim_end_matches(".jpg"))
}

/// Archived frames for a camera: (unix time, path), oldest first.
pub fn archive(name: &str) -> Vec<(i64, std::path::PathBuf)> {
    let mut v: Vec<(i64, std::path::PathBuf)> = std::fs::read_dir(archive_dir(name))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let stem = e.path().file_stem()?.to_string_lossy().to_string();
            let t = chrono::NaiveDateTime::parse_from_str(&stem, "%Y%m%d-%H%M").ok()?.and_utc().timestamp();
            Some((t, e.path()))
        })
        .collect();
    v.sort_by_key(|x| x.0);
    v
}

/// The archived frame nearest a time (camera / radar synchronization), as a media-relative name.
pub fn archive_near(name: &str, t: i64) -> Option<String> {
    let (_, p) = archive(name).into_iter().min_by_key(|(at, _)| (at - t).abs())?;
    p.strip_prefix(LibraryPaths::get().media).ok().map(|r| r.to_string_lossy().replace('\\', "/"))
}

/// Move a PTZ camera.
pub fn ptz(cam: &WebCam, movement: &str) {
    if cam.ptz.trim().is_empty() {
        return;
    }
    let url = cam.ptz.replace("{move}", movement);
    crate::net::spawn(async move {
        if let Err(e) = crate::net::get_logged(&url, std::time::Duration::from_secs(8)).await {
            log::warn!("PTZ: {e}");
        }
    });
}

/// Time-lapse MP4 from a camera's archive (last `hours`), 12 frames a second.
pub fn timelapse(name: &str, hours: i64) -> anyhow::Result<std::path::PathBuf> {
    let since = chrono::Utc::now().timestamp() - hours * 3600;
    let frames: Vec<std::path::PathBuf> = archive(name).into_iter().filter(|(t, _)| *t >= since).map(|(_, p)| p).collect();
    anyhow::ensure!(frames.len() >= 2, "not enough archived frames yet");
    let (w, h) = (1280u32, 720u32);
    let out = crate::library::exports_dir().join(format!("{} time-lapse {}.mp4", name, chrono::Local::now().format("%Y-%m-%d %H-%M")));
    let fps = 12u32;
    let n = frames.len();
    crate::record::export_animation_mp4(
        |t| {
            let i = ((t * fps as f32) as usize).min(n - 1);
            let mut pm = tiny_skia::Pixmap::new(w, h).expect("pixmap");
            if let Ok(img) = image::open(&frames[i]) {
                let img = img.resize_exact(w, h, image::imageops::FilterType::Triangle).to_rgba8();
                pm.data_mut().copy_from_slice(&img);
            }
            pm
        },
        n as f32 / fps as f32,
        fps,
        out,
    )
}

/// Media-library file the camera's latest image is saved as.
pub fn cam_file(name: &str) -> String {
    let slug: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    format!("cam-{}.jpg", slug.trim_matches('-'))
}

/// Download every camera's image into the media library (off the UI thread).
pub fn refresh_cameras(cams: Vec<WebCam>) {
    if cams.is_empty() {
        return;
    }
    crate::net::spawn(async move {
        let dir = LibraryPaths::get().media;
        let _ = std::fs::create_dir_all(&dir);
        for cam in cams {
            if !cam.url.starts_with("http") {
                continue;
            }
            match crate::net::get_logged(&cam.url, std::time::Duration::from_secs(15)).await {
                Ok(resp) => {
                    if let Ok(bytes) = resp.bytes().await {
                        // Re-encode as JPEG so any format the camera serves works in graphics.
                        if let Ok(img) = image::load_from_memory(&bytes) {
                            let rgb = img.to_rgb8();
                            let path = dir.join(cam_file(&cam.name));
                            let tmp = path.with_extension("tmp.jpg");
                            if rgb.save(&tmp).is_ok() {
                                let _ = std::fs::rename(&tmp, &path);
                            }
                            // Snapshot archive (5-minute steps), kept by the retention setting.
                            let adir = archive_dir(&cam.name);
                            let _ = std::fs::create_dir_all(&adir);
                            let stamp = chrono::Utc::now();
                            let stamp = stamp - chrono::Duration::minutes((chrono::Timelike::minute(&stamp) % 5) as i64);
                            let _ = rgb.save(adir.join(format!("{}.jpg", stamp.format("%Y%m%d-%H%M"))));
                            if let Ok(mut m) = status_store().lock() {
                                m.insert(cam.name.clone(), CamStatus { last_ok: Some(chrono::Utc::now().timestamp()), error: String::new(), condition: classify(&rgb) });
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("camera {}: {e}", cam.name);
                    if let Ok(mut m) = status_store().lock() {
                        let s = m.entry(cam.name.clone()).or_default();
                        s.error = e.to_string();
                    }
                }
            }
        }
    });
}

/// The camera nearest a point (for "camera nearest the storm").
pub fn nearest_cam<'a>(cams: &'a [WebCam], lat: f64, lon: f64) -> Option<(&'a WebCam, f64)> {
    cams.iter()
        .filter(|c| c.lat != 0.0 || c.lon != 0.0)
        .map(|c| (c, haversine_km(lat, lon, c.lat, c.lon) / KM_PER_MI))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TravelRoute {
    pub from_label: String,
    pub from: (f64, f64),
    pub to_label: String,
    pub to: (f64, f64),
}

impl TravelRoute {
    pub fn name(&self) -> String {
        format!("{} to {}", self.from_label, self.to_label)
    }
}

pub struct TravelEstimate {
    pub name: String,
    pub normal_min: f64,
    pub now_min: f64,
    pub note: String,
}

/// The station's routes, or home to the four largest towns 25-150 miles away.
pub fn routes(settings: &Settings) -> Vec<TravelRoute> {
    if !settings.travel_routes.is_empty() {
        return settings.travel_routes.clone();
    }
    let home = settings.home;
    let d = 150.0 / 69.0;
    let mut towns = crate::places::in_box(home.lat - d, home.lat + d, home.lon - d * 1.3, home.lon + d * 1.3);
    towns.retain(|t| {
        let mi = haversine_km(home.lat, home.lon, t.lat, t.lon) / KM_PER_MI;
        (25.0..=150.0).contains(&mi)
    });
    towns.sort_by(|a, b| b.population.cmp(&a.population));
    let from_label = if settings.home_label.trim().is_empty() { "Home".to_string() } else { settings.home_label.split(',').next().unwrap_or("Home").trim().to_string() };
    towns
        .into_iter()
        .take(4)
        .map(|t| TravelRoute { from_label: from_label.clone(), from: (home.lat, home.lon), to_label: t.name.clone(), to: (t.lat, t.lon) })
        .collect()
}

/// Distance from a point to the segment a-b, in miles (flat-earth, fine at these scales).
fn dist_to_segment_mi(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let k = p.0.to_radians().cos();
    let (px, py) = (p.1 * k, p.0);
    let (ax, ay) = (a.1 * k, a.0);
    let (bx, by) = (b.1 * k, b.0);
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let (cx, cy) = (ax + dx * t, ay + dy * t);
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() * 69.0
}

/// Estimated drive time now vs normal: roads ~1.25x the straight line at 55 mph, slowed by the
/// worst weather along the way and by incidents within 3 miles of the route.
pub fn estimate(route: &TravelRoute, data: &WxSnapshot) -> TravelEstimate {
    let miles = haversine_km(route.from.0, route.from.1, route.to.0, route.to.1) / KM_PER_MI * 1.25;
    let normal = miles / 55.0 * 60.0;
    let mut factor: f64 = 1.0;
    let mut notes: Vec<String> = Vec::new();
    if let Some(grid) = data.grid.as_deref() {
        let tf = grid.time_index(0.0);
        let mut worst = (1.0f64, "");
        for i in 0..=10 {
            let f = i as f64 / 10.0;
            let (lat, lon) = (route.from.0 + (route.to.0 - route.from.0) * f, route.from.1 + (route.to.1 - route.from.1) * f);
            let road = grid.value(DataField::RoadHazard, tf, lat, lon).unwrap_or(0.0);
            let fog = grid.value(DataField::FogRisk, tf, lat, lon).unwrap_or(0.0);
            let gust = grid.value(DataField::WindGust, tf, lat, lon).unwrap_or(0.0);
            let (rate, kind) = grid.precip(tf, lat, lon).unwrap_or((0.0, crate::grid::PrecipKind::Rain));
            let (f, why) = if road >= 2.5 {
                (1.6, "snow-covered roads")
            } else if road >= 1.5 {
                (1.5, "icy spots")
            } else if kind == crate::grid::PrecipKind::Snow && rate >= 0.01 {
                (1.35, "snow")
            } else if fog >= 70.0 {
                (1.15, "dense fog")
            } else if rate >= 0.25 {
                (1.15, "heavy rain")
            } else if gust >= 45.0 {
                (1.1, "strong crosswinds")
            } else if rate >= 0.02 {
                (1.05, "rain")
            } else {
                (1.0, "")
            };
            if f > worst.0 {
                worst = (f, why);
            }
        }
        factor = worst.0;
        if !worst.1.is_empty() {
            notes.push(worst.1.to_string());
        }
    }
    let mut extra = 0.0;
    let near: Vec<&RoadEvent> = data
        .roads
        .iter()
        .filter(|e| dist_to_segment_mi((e.lat, e.lon), route.from, route.to) <= 3.0)
        .collect();
    for e in near.iter().take(4) {
        match e.kind.as_str() {
            "closure" => {
                extra += 20.0;
                notes.push(format!("closure{}", if e.route.is_empty() { String::new() } else { format!(" on {}", e.route) }));
            }
            "incident" | "delay" | "traffic-condition" | "obstruction" => {
                extra += 8.0;
                notes.push(format!("{}{}", e.label.to_lowercase(), if e.route.is_empty() { String::new() } else { format!(" on {}", e.route) }));
            }
            _ => {}
        }
    }
    notes.dedup();
    TravelEstimate {
        name: route.name(),
        normal_min: normal,
        now_min: normal * factor + extra,
        note: if notes.is_empty() { "No problems expected".to_string() } else { capitalize(&notes.join(", ")) },
    }
}

pub fn minutes_text(m: f64) -> String {
    let m = m.round() as i64;
    if m >= 60 {
        format!("{} hr {:02} min", m / 60, m % 60)
    } else {
        format!("{m} min")
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// One storm compared across sources.
pub struct StormComparison {
    pub name: String,
    pub radar: String,
    pub lightning: String,
    pub reports: String,
    pub warning: String,
    /// How many sources point to a severe storm (0-4).
    pub agreement: u8,
}

/// The strongest storms near home seen by each source: DCWeather radar cells, DCWeather
/// lightning, IEM storm reports and NWS warnings.
pub fn storm_comparison(data: &WxSnapshot, settings: &Settings) -> Vec<StormComparison> {
    let home = settings.home;
    let mut cells: Vec<&crate::data::StormCell> = data
        .cells
        .iter()
        .filter(|c| haversine_km(home.lat, home.lon, c.lat, c.lon) / KM_PER_MI <= settings.coverage_miles.max(50.0) * 1.5)
        .collect();
    cells.sort_by(|a, b| b.max_dbz.total_cmp(&a.max_dbz));
    let since = chrono::Utc::now().timestamp() - 2 * 3600;
    cells
        .into_iter()
        .take(5)
        .map(|c| {
            let near_town = crate::places::in_box(c.lat - 0.4, c.lat + 0.4, c.lon - 0.5, c.lon + 0.5)
                .into_iter()
                .max_by_key(|p| p.population)
                .map(|p| format!("near {}", p.name))
                .unwrap_or_else(|| format!("{:.0} mi {} of home", c.distance_mi, c.direction));
            let flashes: u32 = crate::map::extras::flash_rate(&data.lightning, c.lat, c.lon, 10.0).iter().rev().take(2).sum();
            let reports: Vec<&crate::data::StormReport> = data
                .reports
                .iter()
                .filter(|r| r.time >= since && haversine_km(c.lat, c.lon, r.lat, r.lon) / KM_PER_MI <= 15.0)
                .collect();
            let warning = data
                .map_alerts
                .iter()
                .chain(data.local_alerts.iter())
                .filter(|a| a.is_warning() && a.contains(c.lat, c.lon))
                .map(|a| a.event.clone())
                .next();
            let radar_severe = c.max_dbz >= 60.0 || c.intensity.hail.is_some();
            let lightning_severe = flashes >= 20;
            let reports_severe = reports.iter().any(|r| matches!(r.kind.as_str(), "T" | "C" | "H" | "D" | "G"));
            let warned = warning.is_some();
            StormComparison {
                name: format!("Storm {near_town}"),
                radar: if c.max_dbz > 0.0 {
                    format!("{:.0} dBZ{}", c.max_dbz, c.intensity.hail.as_ref().map(|h| format!(", hail {h}")).unwrap_or_default())
                } else {
                    "No radar data".into()
                },
                lightning: if flashes > 0 { format!("{flashes} flashes / 10 min") } else { "No lightning".into() },
                reports: if reports.is_empty() { "No reports".into() } else { format!("{} report(s) nearby", reports.len()) },
                warning: warning.unwrap_or_else(|| "Not warned".into()),
                agreement: [radar_severe, lightning_severe, reports_severe, warned].iter().filter(|f| **f).count() as u8,
            }
        })
        .collect()
}
