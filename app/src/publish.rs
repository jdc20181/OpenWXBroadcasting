//! Publishing: a queue that renders graphics in each platform's format and delivers them to
//! folders, HTTP / CDN storage (with cache purge) or chat webhooks — with approval, conditions,
//! schedules, retries, delivery status, version tracking, an audit history and failure alerts.
//! Also the public snapshot used by the viewer portal's public image / data endpoints.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, OnceLock, RwLock},
    time::Duration,
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::{config::Settings, data::WxSnapshot, gfx::model::GraphicDoc};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Target {
    pub name: String,
    /// "folder", "http" (PUT to a CDN / storage URL), "discord", "slack", "dcweather" (Facebook
    /// page through the DCWeather site; address = caption).
    pub kind: String,
    /// Folder path, upload URL with {file}, or webhook URL.
    pub address: String,
    /// Optional auth header for HTTP uploads, "Name: value" (e.g. "AccessKey: …").
    pub header: String,
    /// Optional cache purge URL (POST) after an upload, with {file}.
    pub purge_url: String,
    /// "landscape" (1920x1080), "square" (1080x1080), "vertical" (1080x1920), "story" (1080x1350).
    pub format: String,
    /// "png" or "jpg".
    pub encoding: String,
    /// DCWeather targets: sponsor id from the site's list ("" = no sponsor line or tag).
    pub sponsor_id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Schedule {
    pub graphic_id: String,
    pub targets: Vec<String>,
    pub every_minutes: u32,
    /// "always", "warnings" (only while warnings are in coverage), "changed" (only when the
    /// picture changed since the last publish), "calm" (only without warnings).
    pub condition: String,
    #[serde(skip)]
    pub last_run: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub graphic: String,
    pub graphic_id: String,
    /// Graphic version (its updated time) and a hash of the rendered file.
    pub version: i64,
    pub hash: u64,
    pub target: String,
    pub status: String,
    pub attempts: u32,
    pub error: String,
    pub time: i64,
}

struct Queue {
    jobs: VecDeque<Job>,
    history: Vec<Job>,
    next_id: u64,
    last_hash: std::collections::HashMap<String, u64>,
}

fn queue() -> &'static Mutex<Queue> {
    static Q: OnceLock<Mutex<Queue>> = OnceLock::new();
    Q.get_or_init(|| Mutex::new(Queue { jobs: VecDeque::new(), history: Vec::new(), next_id: 1, last_hash: Default::default() }))
}

/// Jobs waiting in the queue.
pub fn pending() -> usize {
    queue().lock().map(|q| q.jobs.len()).unwrap_or(0)
}

/// Pending and recent jobs for the status panel.
pub fn status() -> (Vec<Job>, Vec<Job>) {
    queue().lock().map(|q| (q.jobs.iter().cloned().collect(), q.history.iter().rev().take(40).cloned().collect())).unwrap_or_default()
}

fn audit(job: &Job) {
    let path = crate::config::LibraryPaths::get().root.join("Publishing history.csv");
    let new = !path.exists();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        use std::io::Write;
        if new {
            let _ = writeln!(f, "time,graphic,version,target,status,attempts,error");
        }
        let when = chrono::DateTime::from_timestamp(job.time, 0).map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default();
        let _ = writeln!(f, "{when},\"{}\",{},\"{}\",{},{},\"{}\"", job.graphic, job.version, job.target, job.status, job.attempts, job.error.replace('"', "'"));
    }
}

fn size_for(format: &str) -> (u32, u32) {
    match format {
        "square" => (1080, 1080),
        "vertical" => (1080, 1920),
        "story" => (1080, 1350),
        _ => (1920, 1080),
    }
}

/// Render a graphic in a target's format (laid out for the canvas, not stretched).
pub fn render(doc: &GraphicDoc, settings: &Settings, data: &WxSnapshot, target: &Target) -> Result<Vec<u8>> {
    let (w, h) = size_for(&target.format);
    let laid = if (doc.width, doc.height) == (w, h) { doc.clone() } else { doc.laid_out(w, h) };
    let mut bg = tiny_skia::Pixmap::new(w, h).ok_or_else(|| anyhow!("pixmap"))?;
    if target.encoding == "jpg" {
        bg.fill(tiny_skia::Color::BLACK);
    }
    let pm = crate::program::render_graphic(&laid, settings, data, laid.intro_length() + 0.5, w, h, (target.encoding == "jpg").then_some(&bg));
    if target.encoding == "jpg" {
        let rgb: Vec<u8> = pm.data().chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        let mut out = std::io::Cursor::new(Vec::new());
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90).encode(&rgb, w, h, image::ExtendedColorType::Rgb8)?;
        Ok(out.into_inner())
    } else {
        crate::ops::pixmap_png(&pm).ok_or_else(|| anyhow!("PNG encoding failed"))
    }
}

fn hash(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

/// Queue a graphic for every named target. Unapproved graphics are refused when approval is
/// required. Returns how many jobs were queued.
pub fn enqueue(doc: &GraphicDoc, settings: &Settings, targets: &[String]) -> Result<usize> {
    if settings.require_approval && doc.approved_by.trim().is_empty() {
        return Err(anyhow!("\"{}\" isn't approved for publishing", doc.name));
    }
    let mut n = 0;
    if let Ok(mut q) = queue().lock() {
        for t in targets {
            let id = q.next_id;
            q.next_id += 1;
            q.jobs.push_back(Job { id, graphic: doc.name.clone(), graphic_id: doc.id.clone(), version: doc.updated, hash: 0, target: t.clone(), status: "queued".into(), attempts: 0, error: String::new(), time: chrono::Utc::now().timestamp() });
            n += 1;
        }
    }
    Ok(n)
}

/// Work the queue (called every few seconds from the app, off the UI thread).
pub async fn work(settings: Arc<Settings>, data: Arc<WxSnapshot>, graphics: Arc<Vec<GraphicDoc>>) {
    let Some(mut job) = queue().lock().ok().and_then(|mut q| q.jobs.pop_front()) else { return };
    let target = settings.publish_targets.iter().find(|t| t.name == job.target).cloned();
    let doc = graphics.iter().find(|g| g.id == job.graphic_id).cloned();
    let no_retry = target.as_ref().is_some_and(|t| t.kind == "dcweather");
    job.attempts += 1;
    let result: Result<u64> = async {
        let target = target.ok_or_else(|| anyhow!("target \"{}\" no longer exists", job.target))?;
        let doc = doc.ok_or_else(|| anyhow!("graphic was deleted"))?;
        let bytes = render(&doc, &settings, &data, &target)?;
        let h = hash(&bytes);
        let clean: String = doc.name.chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
        let shape = if target.format.is_empty() { "landscape" } else { target.format.as_str() };
        let file = format!("{}-{}.{}", clean.trim_matches('-').to_lowercase(), shape, if target.encoding == "jpg" { "jpg" } else { "png" });
        let client = reqwest::Client::new();
        match target.kind.as_str() {
            "folder" => {
                std::fs::create_dir_all(&target.address)?;
                std::fs::write(std::path::Path::new(&target.address).join(&file), &bytes)?;
            }
            "http" => {
                let url = target.address.replace("{file}", &file);
                let mut req = client.put(&url).body(bytes.clone()).timeout(Duration::from_secs(60));
                if let Some((k, v)) = target.header.split_once(':') {
                    req = req.header(k.trim(), v.trim());
                }
                let resp = req.send().await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("upload answered {}", resp.status()));
                }
                // Content cache invalidation on the CDN.
                if !target.purge_url.trim().is_empty() {
                    let mut p = client.post(target.purge_url.replace("{file}", &file)).timeout(Duration::from_secs(30));
                    if let Some((k, v)) = target.header.split_once(':') {
                        p = p.header(k.trim(), v.trim());
                    }
                    let _ = p.send().await;
                }
            }
            "discord" | "slack" => {
                let part = reqwest::multipart::Part::bytes(bytes.clone()).file_name(file.clone());
                let form = reqwest::multipart::Form::new().text("content", format!("{} • {}", doc.name, settings.station_name)).part("file", part);
                let resp = client.post(&target.address).multipart(form).timeout(Duration::from_secs(60)).send().await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("webhook answered {}", resp.status()));
                }
            }
            // The DCWeather site's Facebook page, through the studio bridge (signed-in token).
            // The address is the caption: {name} = graphic name, {station} = station name.
            "dcweather" => {
                let caption = if target.address.trim().is_empty() { "{name}".to_string() } else { target.address.clone() };
                let message = caption.replace("{name}", &doc.name).replace("{station}", &settings.station_name);
                let sponsor = Some(target.sponsor_id.clone()).filter(|s| !s.is_empty());
                crate::dcweather::post_image(settings.api_base.clone(), bytes.clone(), file.clone(), message, sponsor).await?;
            }
            other => return Err(anyhow!("unknown target type {other}")),
        }
        Ok(h)
    }
    .await;
    if let Ok(mut q) = queue().lock() {
        match result {
            Ok(h) => {
                job.status = "delivered".into();
                job.hash = h;
                job.error.clear();
                q.last_hash.insert(format!("{}|{}", job.graphic_id, job.target), h);
                audit(&job);
                q.history.push(job);
            }
            // Never retry a Facebook post: a timeout after the post went through would post twice.
            Err(e) if job.attempts < 3 && !no_retry => {
                // Retry with back-off (re-queued at the end; the worker runs every few seconds).
                job.status = format!("retrying ({})", job.attempts);
                job.error = e.to_string();
                q.jobs.push_back(job);
            }
            Err(e) => {
                job.status = "failed".into();
                job.error = e.to_string();
                audit(&job);
                crate::broadcast_ops::notify("Publishing failed", &format!("{} to {}: {e}", job.graphic, job.target), &settings.notify_webhook);
                q.history.push(job);
            }
        }
        if q.history.len() > 500 {
            q.history.drain(..100);
        }
    }
}

/// Scheduled and conditional publishing: queue what is due.
pub fn run_schedules(settings: &Settings, data: &WxSnapshot, graphics: &[GraphicDoc], warnings_in_coverage: bool) -> Vec<Schedule> {
    let now = chrono::Utc::now().timestamp();
    let mut sched = settings.publish_schedules.clone();
    for s in sched.iter_mut() {
        if s.every_minutes == 0 || now - s.last_run < s.every_minutes as i64 * 60 {
            continue;
        }
        s.last_run = now;
        let Some(doc) = graphics.iter().find(|g| g.id == s.graphic_id) else { continue };
        let ok = match s.condition.as_str() {
            "warnings" => warnings_in_coverage,
            "calm" => !warnings_in_coverage,
            "changed" => {
                // Only when the rendered picture differs from the last delivery.
                let first = s.targets.first().and_then(|t| settings.publish_targets.iter().find(|x| &x.name == t));
                match first.and_then(|t| render(doc, settings, data, t).ok()) {
                    Some(bytes) => {
                        let h = hash(&bytes);
                        queue().lock().map(|q| q.last_hash.get(&format!("{}|{}", doc.id, s.targets[0])) != Some(&h)).unwrap_or(true)
                    }
                    None => false,
                }
            }
            _ => true,
        };
        if ok {
            let _ = enqueue(doc, settings, &s.targets);
        }
    }
    sched
}

// ---- Public snapshot for the portal's public endpoints ----

pub struct Public {
    pub data: Arc<WxSnapshot>,
    pub settings: Arc<Settings>,
    pub graphics: Arc<Vec<GraphicDoc>>,
    /// Newest radar map frame as JPEG (updated every 30 s while the portal is on).
    pub radar_jpg: Vec<u8>,
    pub version: u64,
}

pub fn public() -> &'static RwLock<Option<Public>> {
    static P: OnceLock<RwLock<Option<Public>>> = OnceLock::new();
    P.get_or_init(|| RwLock::new(None))
}

/// Rendered-image cache for public endpoints: (key) -> (data version, time, bytes).
fn image_cache() -> &'static Mutex<std::collections::HashMap<String, (u64, i64, Vec<u8>)>> {
    static C: OnceLock<Mutex<std::collections::HashMap<String, (u64, i64, Vec<u8>)>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// Usage analytics: (endpoint, day) -> requests.
fn usage() -> &'static Mutex<std::collections::BTreeMap<(String, String), u64>> {
    static U: OnceLock<Mutex<std::collections::BTreeMap<(String, String), u64>>> = OnceLock::new();
    U.get_or_init(Default::default)
}

pub fn record_use(endpoint: &str) {
    let day = chrono::Local::now().format("%Y-%m-%d").to_string();
    if let Ok(mut u) = usage().lock() {
        *u.entry((endpoint.to_string(), day)).or_default() += 1;
    }
}

pub fn usage_rows() -> Vec<(String, String, u64)> {
    usage().lock().map(|u| u.iter().rev().take(60).map(|((e, d), n)| (d.clone(), e.clone(), *n)).collect()).unwrap_or_default()
}

/// Whether warnings are in effect for the coverage area (public warning image).
pub fn has_warnings() -> bool {
    public().read().ok().and_then(|g| g.as_ref().map(|p| !p.data.local_alerts.is_empty())).unwrap_or(false)
}

/// Current conditions, forecast and warnings as JSON for widgets and partner sites.
pub fn public_json() -> Option<String> {
    let guard = public().read().ok()?;
    let p = guard.as_ref()?;
    let b = crate::gfx::bind::Bindings::build(&p.data, &p.settings, None, &p.settings.radar_palette);
    const KEEP: [&str; 16] = ["temp", "feels", "dewpoint", "humidity", "wind", "condition", "location", "station", "day", "alert", "updated", "sunrise", "sunset", "pressure", "visibility", "hi"];
    let values: serde_json::Map<String, serde_json::Value> = b
        .values()
        .iter()
        .filter(|(k, _)| KEEP.iter().any(|pre| k.starts_with(pre)))
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();
    let warnings: Vec<serde_json::Value> = p
        .data
        .local_alerts
        .iter()
        .map(|a| serde_json::json!({ "event": a.event, "headline": a.headline, "areas": a.area_desc, "severity": a.severity, "expires": a.expires }))
        .collect();
    Some(serde_json::json!({ "station": p.settings.station_name, "values": values, "warnings": warnings, "updated": p.data.updated_at.map(|t| t.to_rfc3339()) }).to_string())
}

/// Newest radar map frame (JPEG), if the app has shared one.
pub fn radar_jpg() -> Vec<u8> {
    public().read().ok().and_then(|g| g.as_ref().map(|p| p.radar_jpg.clone())).unwrap_or_default()
}

/// A public image: a library graphic (by name) or a template, rendered and cached until the data
/// changes or `max_age` seconds pass (cache invalidation + expiration controls).
pub fn public_image(key: &str) -> Option<Vec<u8>> {
    let guard = public().read().ok()?;
    let p = guard.as_ref()?;
    let max_age = p.settings.public_cache_secs.max(10) as i64;
    let now = chrono::Utc::now().timestamp();
    if let Some((v, t, bytes)) = image_cache().lock().ok()?.get(key) {
        if *v == p.version && now - t < max_age {
            return Some(bytes.clone());
        }
    }
    let doc = p.graphics.iter().find(|g| g.name.eq_ignore_ascii_case(key)).cloned().or_else(|| crate::gfx::templates::build(key))?;
    let target = Target { format: "landscape".into(), encoding: "png".into(), ..Default::default() };
    let bytes = render(&doc, &p.settings, &p.data, &target).ok()?;
    image_cache().lock().ok()?.insert(key.to_string(), (p.version, now, bytes.clone()));
    Some(bytes)
}
