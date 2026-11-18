//! Broadcast operations services that run beside the UI:
//!
//! - **Control server** (127.0.0.1 only): the Program output as a live page or PNG for an OBS
//!   browser source, library graphics as transparent PNGs, and a small automation API
//!   (take, next, black, Severe Mode…) usable from Stream Deck, Companion, scripts or webhooks.
//! - **Session restore**: the live map and show position are saved every 30 s and restored after
//!   a crash or restart.
//! - **Warning archive**: every new alert is appended to a daily JSONL file.
//! - **Diagnostics bundle**: logs + settings (passwords removed) zipped to the desktop.
//! - **Memory watch** and **text to speech** (Windows speech, no extra install).

use std::{
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::{map::scene::MapScene, program::Compositor};

pub const DEFAULT_PORT: u16 = 8765;

/// A request from the control server, handled on the UI thread (AppState::poll_ops).
pub enum Command {
    TakeScene(usize),
    Next,
    Prev,
    Live,
    Black(bool),
    /// Take a library graphic to air by name or id.
    Graphic(String),
    /// "severe", "tornado", "flood" or "off".
    Severe(String),
    Refresh,
    /// The emergency backup scene (station ID / "we'll be right back") on or off.
    Backup(bool),
    /// Render a library graphic (name or id) as PNG bytes.
    RenderGraphic(String, Sender<Option<Vec<u8>>>),
    /// JSON status for /api/status.
    Status(Sender<String>),
    /// Live caption text for the Program output (empty clears it).
    Caption(String),
    /// Acknowledge a warning by id (empty = the oldest unacknowledged one).
    Acknowledge(String),
    /// Put a library graphic on Channel 2 / 3.
    ChannelTake(usize, String),
    ChannelClear(usize),
    /// Same graphic to Program and a channel, starting on the same frame.
    TakeSynced(usize, String),
    /// JSON list of library graphics (name, category, approved).
    ListGraphics(Sender<String>),
    /// JSON rundown (scenes, on-air index).
    Rundown(Sender<String>),
    /// Approve a graphic for air (producer page).
    Approve(String),
    /// Jump the graphic on Program to a marker / cue point.
    Cue(String),
    /// Play a media audio file on the cue bus (scheduled audio cues).
    PlayAudio(String),
    /// Interactive graphic action: "state:Name", "toggle:Layer", "graphic:Name", "pause", "play".
    Interact(String),
}

/// Start the control server on `port`. Returns the receiver the app polls each tick.
pub fn start_server(port: u16, compositor: Compositor) -> Receiver<Command> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("control-server".into())
        .spawn(move || {
            let listener = match TcpListener::bind(("127.0.0.1", port)) {
                Ok(l) => l,
                Err(e) => {
                    log::warn!("control server: port {port} unavailable ({e}); OBS browser source and automation API are off");
                    return;
                }
            };
            log::info!("control server on http://127.0.0.1:{port}/ (OBS browser source, automation API)");
            for stream in listener.incoming().flatten() {
                let tx = tx.clone();
                let compositor = compositor.clone();
                let _ = std::thread::Builder::new()
                    .name("control-conn".into())
                    .spawn(move || {
                        if let Err(e) = handle(stream, &tx, &compositor, port) {
                            log::debug!("control server: {e}");
                        }
                    });
            }
        })
        .ok();
    rx
}

struct Request {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    body: String,
}

impl Request {
    fn param(&self, key: &str) -> Option<String> {
        self.query.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).or_else(|| {
            serde_json::from_str::<serde_json::Value>(&self.body)
                .ok()
                .and_then(|v| v.get(key).map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())))
        })
    }
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn read_request(stream: &mut TcpStream) -> Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = header.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                length = v.trim().parse().unwrap_or(0).min(64 * 1024);
            }
        }
    }
    let mut body = vec![0u8; length];
    if length > 0 {
        reader.read_exact(&mut body)?;
    }
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let query = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (url_decode(k), url_decode(v))
        })
        .collect();
    Ok(Request { method, path: path.to_string(), query, body: String::from_utf8_lossy(&body).into_owned() })
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}

/// BGRA bus frame -> PNG (opaque) or JPEG bytes.
pub fn encode_frame(frame: &crate::program::BusFrame, jpeg: bool, scale_to: Option<u32>) -> Option<Vec<u8>> {
    let mut rgba = frame.bgra.as_ref().clone();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
    let mut img = image::RgbaImage::from_raw(frame.width, frame.height, rgba)?;
    if let Some(w) = scale_to.filter(|w| *w < frame.width) {
        let h = (frame.height as u64 * w as u64 / frame.width as u64) as u32;
        img = image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle);
    }
    let mut out = std::io::Cursor::new(Vec::new());
    if jpeg {
        let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85).encode_image(&rgb).ok()?;
    } else {
        img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    }
    Some(out.into_inner())
}

/// Straight-alpha RGBA PNG from a tiny-skia pixmap (keeps transparency).
pub fn pixmap_png(pm: &tiny_skia::Pixmap) -> Option<Vec<u8>> {
    let mut rgba = Vec::with_capacity(pm.data().len());
    for p in pm.pixels() {
        let c = p.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    let img = image::RgbaImage::from_raw(pm.width(), pm.height(), rgba)?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

const INDEX: &str = r#"<!doctype html><meta charset="utf-8"><title>OpenWX control</title>
<style>body{font:15px system-ui;background:#121418;color:#e4e6ea;max-width:760px;margin:40px auto;padding:0 16px}code{color:#5b93ee}li{margin:6px 0}</style>
<h1>OpenWX control server</h1>
<p>Only this computer can reach it (127.0.0.1).</p>
<h2>OBS browser source</h2><ul>
<li><code>/program</code> live Program output (set the source to 1920x1080)</li>
<li><code>/program.png</code> one frame &middot; <code>/program.mjpg</code> video stream</li>
<li><code>/vertical</code> 9:16 vertical version of the Program output (phones, vertical streams)</li>
<li><code>/graphic.png?name=7-Day Forecast</code> a library graphic with transparency</li>
<li><code>/graphic?name=Temperature Bug</code> the same, refreshing every 5 s</li></ul>
<h2>Automation API (GET or POST, also JSON body)</h2><ul>
<li><code>/api/next</code> &middot; <code>/api/prev</code> &middot; <code>/api/take?scene=3</code> &middot; <code>/api/live</code></li>
<li><code>/api/black?on=1</code> &middot; <code>/api/backup?on=1</code> (emergency backup scene)</li>
<li><code>/api/graphic?name=Current Conditions</code> take a graphic to air</li>
<li><code>/api/severe?mode=severe|tornado|flood|off</code> &middot; <code>/api/refresh</code></li>
<li><code>/api/caption?text=...</code> live caption on Program (empty clears) &middot; <code>/api/ack?id=...</code> acknowledge a warning (no id = oldest)</li>
<li><code>/producer</code> producer page: rundown, graphics, Program and channel takes, approvals</li>
<li><code>/channel2</code> &middot; <code>/channel2.png</code> (transparent) &middot; <code>/channel2.mjpg</code> &middot; same for 3: independent graphics outputs</li>
<li><code>/api/channel?n=2&amp;name=...</code> &middot; <code>&amp;clear=1</code> &middot; <code>&amp;sync=1</code> (Program and channel together)</li>
<li><code>/api/graphics</code> &middot; <code>/api/rundown</code> JSON &middot; <code>/api/approve?name=...</code></li>
<li><code>/api/status</code> JSON status &middot; <code>/api/webhook</code> POST {"action":"next"} (same actions)</li></ul>"#;

fn live_page(src: &str, every_ms: u32) -> String {
    format!(
        r#"<!doctype html><meta charset="utf-8"><style>html,body{{margin:0;background:transparent;overflow:hidden}}img{{width:100vw;height:100vh;object-fit:contain;display:block}}</style>
<img id="f" src="{src}"><script>
const f=document.getElementById('f');let busy=false;
setInterval(()=>{{if(busy)return;busy=true;const n=new Image();n.onload=()=>{{f.src=n.src;busy=false}};n.onerror=()=>{{busy=false}};n.src='{src}'+('{src}'.includes('?')?'&':'?')+'t='+Date.now()}},{every_ms});
</script>"#
    )
}

fn handle(mut stream: TcpStream, tx: &Sender<Command>, compositor: &Compositor, port: u16) -> Result<()> {
    let req = read_request(&mut stream)?;
    let ok_json = |stream: &mut TcpStream, msg: &str| respond(stream, "200 OK", "application/json", format!("{{\"ok\":true,\"done\":\"{msg}\"}}").as_bytes());
    let send = |cmd: Command| tx.send(cmd).map_err(|_| anyhow!("app is closing"));
    match req.path.as_str() {
        "/" => respond(&mut stream, "200 OK", "text/html; charset=utf-8", INDEX.replace("127.0.0.1", &format!("127.0.0.1:{port}")).as_bytes()),
        "/program" => respond(&mut stream, "200 OK", "text/html; charset=utf-8", live_page("/program.jpg", 100).as_bytes()),
        "/producer" => respond(&mut stream, "200 OK", "text/html; charset=utf-8", PRODUCER.as_bytes()),
        path if path.starts_with("/channel") => {
            // /channel2 (page), /channel2.png (transparent frame), /channel2.mjpg (stream)
            let rest = path.trim_start_matches("/channel");
            let (num, ext) = rest.split_once('.').unwrap_or((rest, ""));
            let Some(ch) = num.parse::<usize>().ok().and_then(crate::channels::get) else {
                return respond(&mut stream, "404 Not Found", "text/plain", b"channels are 2 and 3");
            };
            match ext {
                "" => respond(&mut stream, "200 OK", "text/html; charset=utf-8", live_page(&format!("/channel{num}.png"), 100).as_bytes()),
                "png" | "jpg" => match ch.frame().and_then(|f| encode_frame(&f, ext == "jpg", req.param("width").and_then(|w| w.parse().ok()))) {
                    Some(bytes) => respond(&mut stream, "200 OK", if ext == "jpg" { "image/jpeg" } else { "image/png" }, &bytes),
                    None => respond(&mut stream, "204 No Content", "text/plain", b""),
                },
                "mjpg" => {
                    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary=frame\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n")?;
                    let mut last = 0u64;
                    loop {
                        if let Some(frame) = ch.frame().filter(|f| f.serial != last) {
                            last = frame.serial;
                            if let Some(jpg) = encode_frame(&frame, true, None) {
                                write!(stream, "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n", jpg.len())?;
                                stream.write_all(&jpg)?;
                                stream.write_all(b"\r\n")?;
                            }
                        }
                        std::thread::sleep(Duration::from_millis(66));
                    }
                }
                _ => respond(&mut stream, "404 Not Found", "text/plain", b"use /channelN, .png, .jpg or .mjpg"),
            }
        }
        "/vertical" => respond(&mut stream, "200 OK", "text/html; charset=utf-8", live_page("/vertical.jpg", 100).as_bytes()),
        "/vertical.jpg" => {
            // 9:16 centre crop of the Program output (vertical streams and phone screens).
            let crop = compositor.program_frame().and_then(|f| {
                let mut rgba = f.bgra.as_ref().clone();
                for px in rgba.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
                let img = image::RgbaImage::from_raw(f.width, f.height, rgba)?;
                let cw = (f.height as f32 * 9.0 / 16.0) as u32;
                let x = (f.width - cw) / 2;
                let sub = image::imageops::crop_imm(&img, x, 0, cw, f.height).to_image();
                let rgb = image::DynamicImage::ImageRgba8(sub).to_rgb8();
                let mut out = std::io::Cursor::new(Vec::new());
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85).encode_image(&rgb).ok()?;
                Some(out.into_inner())
            });
            match crop {
                Some(bytes) => respond(&mut stream, "200 OK", "image/jpeg", &bytes),
                None => respond(&mut stream, "503 Service Unavailable", "text/plain", b"no program frame yet"),
            }
        }
        "/program.png" | "/program.jpg" => {
            let jpeg = req.path.ends_with(".jpg");
            match compositor.program_frame().and_then(|f| encode_frame(&f, jpeg, req.param("width").and_then(|w| w.parse().ok()))) {
                Some(bytes) => respond(&mut stream, "200 OK", if jpeg { "image/jpeg" } else { "image/png" }, &bytes),
                None => respond(&mut stream, "503 Service Unavailable", "text/plain", b"no program frame yet"),
            }
        }
        "/program.mjpg" => {
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary=frame\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n")?;
            let mut last = 0u64;
            loop {
                if let Some(frame) = compositor.program_frame().filter(|f| f.serial != last) {
                    last = frame.serial;
                    if let Some(jpg) = encode_frame(&frame, true, None) {
                        write!(stream, "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n", jpg.len())?;
                        stream.write_all(&jpg)?;
                        stream.write_all(b"\r\n")?;
                    }
                }
                std::thread::sleep(Duration::from_millis(66));
            }
        }
        "/graphic" => {
            let name = req.param("name").unwrap_or_default();
            let src = format!("/graphic.png?name={}", name.replace(' ', "%20"));
            respond(&mut stream, "200 OK", "text/html; charset=utf-8", live_page(&src, 5000).as_bytes())
        }
        "/graphic.png" => {
            let name = req.param("name").unwrap_or_default();
            let (reply, wait) = mpsc::channel();
            send(Command::RenderGraphic(name, reply))?;
            match wait.recv_timeout(Duration::from_secs(10)).ok().flatten() {
                Some(png) => respond(&mut stream, "200 OK", "image/png", &png),
                None => respond(&mut stream, "404 Not Found", "text/plain", b"no graphic by that name"),
            }
        }
        path if path.starts_with("/api/") => {
            let mut action = path.trim_start_matches("/api/").to_string();
            if action == "webhook" {
                action = req.param("action").unwrap_or_default();
            }
            let on = || req.param("on").map(|v| v != "0" && v != "false").unwrap_or(true);
            match action.as_str() {
                "status" => {
                    let (reply, wait) = mpsc::channel();
                    send(Command::Status(reply))?;
                    let body = wait.recv_timeout(Duration::from_secs(5)).unwrap_or_else(|_| "{}".into());
                    respond(&mut stream, "200 OK", "application/json", body.as_bytes())
                }
                "next" => send(Command::Next).and_then(|_| ok_json(&mut stream, "next")),
                "prev" => send(Command::Prev).and_then(|_| ok_json(&mut stream, "prev")),
                "live" => send(Command::Live).and_then(|_| ok_json(&mut stream, "live")),
                "refresh" => send(Command::Refresh).and_then(|_| ok_json(&mut stream, "refresh")),
                "black" => send(Command::Black(on())).and_then(|_| ok_json(&mut stream, "black")),
                "backup" => send(Command::Backup(on())).and_then(|_| ok_json(&mut stream, "backup")),
                "take" => match req.param("scene").and_then(|s| s.parse::<usize>().ok()) {
                    Some(n) if n >= 1 => send(Command::TakeScene(n - 1)).and_then(|_| ok_json(&mut stream, "take")),
                    _ => respond(&mut stream, "400 Bad Request", "text/plain", b"scene=N (1 = first scene)"),
                },
                "graphic" => match req.param("name") {
                    Some(name) => send(Command::Graphic(name)).and_then(|_| ok_json(&mut stream, "graphic")),
                    None => respond(&mut stream, "400 Bad Request", "text/plain", b"name=..."),
                },
                "severe" => send(Command::Severe(req.param("mode").unwrap_or_else(|| "severe".into())))
                    .and_then(|_| ok_json(&mut stream, "severe")),
                "caption" => send(Command::Caption(req.param("text").unwrap_or_default())).and_then(|_| ok_json(&mut stream, "caption")),
                "channel" => {
                    let n = req.param("n").and_then(|v| v.parse::<usize>().ok()).unwrap_or(2);
                    if req.param("clear").is_some() {
                        send(Command::ChannelClear(n)).and_then(|_| ok_json(&mut stream, "channel cleared"))
                    } else if let Some(name) = req.param("name") {
                        if req.param("sync").is_some() {
                            send(Command::TakeSynced(n, name)).and_then(|_| ok_json(&mut stream, "synced take"))
                        } else {
                            send(Command::ChannelTake(n, name)).and_then(|_| ok_json(&mut stream, "channel"))
                        }
                    } else {
                        respond(&mut stream, "400 Bad Request", "text/plain", b"n=2&name=... or n=2&clear=1")
                    }
                }
                "graphics" | "rundown" => {
                    let (reply, wait) = mpsc::channel();
                    send(if action == "graphics" { Command::ListGraphics(reply) } else { Command::Rundown(reply) })?;
                    let body = wait.recv_timeout(Duration::from_secs(5)).unwrap_or_else(|_| "[]".into());
                    respond(&mut stream, "200 OK", "application/json", body.as_bytes())
                }
                "state" => send(Command::Interact(format!("state:{}", req.param("name").unwrap_or_default()))).and_then(|_| ok_json(&mut stream, "state")),
                "toggle" => send(Command::Interact(format!("toggle:{}", req.param("layer").unwrap_or_default()))).and_then(|_| ok_json(&mut stream, "toggle")),
                "drill" => send(Command::Interact(format!("graphic:{}", req.param("name").unwrap_or_default()))).and_then(|_| ok_json(&mut stream, "drill")),
                "pause" => send(Command::Interact("pause".into())).and_then(|_| ok_json(&mut stream, "pause")),
                "resume" => send(Command::Interact("play".into())).and_then(|_| ok_json(&mut stream, "resume")),
                "cue" => match req.param("name") {
                    Some(name) => send(Command::Cue(name)).and_then(|_| ok_json(&mut stream, "cue")),
                    None => respond(&mut stream, "400 Bad Request", "text/plain", b"name=..."),
                },
                "approve" => match req.param("name") {
                    Some(name) => send(Command::Approve(name)).and_then(|_| ok_json(&mut stream, "approved")),
                    None => respond(&mut stream, "400 Bad Request", "text/plain", b"name=..."),
                },
                "ack" => send(Command::Acknowledge(req.param("id").unwrap_or_default())).and_then(|_| ok_json(&mut stream, "ack")),
                _ => respond(&mut stream, "404 Not Found", "text/plain", b"unknown action; see /"),
            }
        }
        _ => {
            let _ = req.method;
            respond(&mut stream, "404 Not Found", "text/plain", b"not found; see /")
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Session restore
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub live: Option<MapScene>,
    pub show_id: Option<String>,
    pub show_pos: Option<usize>,
    pub severe_mode: String,
    /// Unix seconds of the save.
    pub saved: i64,
    /// False while the app runs; set true on a clean exit.
    pub clean_exit: bool,
}

fn session_path() -> PathBuf {
    crate::config::config_dir().join("session.json")
}

pub fn save_session(session: &Session) {
    let path = session_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(session) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn load_session() -> Option<Session> {
    let text = std::fs::read_to_string(session_path()).ok()?;
    serde_json::from_str(&text).ok()
}

/// Mark the session as cleanly closed (no "restored after a crash" notice next time).
pub fn mark_clean_exit() {
    if let Some(mut s) = load_session() {
        s.clean_exit = true;
        save_session(&s);
    }
}

// ---------------------------------------------------------------------------------------------
// Warning archive (CAP messages as received from api.weather.gov)
// ---------------------------------------------------------------------------------------------

pub fn archive_dir() -> PathBuf {
    crate::config::LibraryPaths::get().root.join("Warning archive")
}

pub fn archive_alert(alert: &crate::data::Alert) {
    if alert.id.starts_with("TEST-") {
        return;
    }
    let dir = archive_dir();
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join(format!("{}.jsonl", chrono::Local::now().format("%Y-%m-%d")));
    let line = serde_json::json!({
        "received": chrono::Local::now().to_rfc3339(),
        "id": alert.id,
        "event": alert.event,
        "headline": alert.headline,
        "severity": alert.severity,
        "urgency": alert.urgency,
        "areas": alert.area_desc,
        "sender": alert.sender,
        "sent": alert.sent,
        "expires": alert.expires,
        "message_type": alert.message_type,
        "description": alert.description,
        "instruction": alert.instruction,
        "motion": alert.motion_text,
        "polygons": alert.polygons,
    });
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{line}");
    }
}

// ---------------------------------------------------------------------------------------------
// Diagnostics bundle
// ---------------------------------------------------------------------------------------------

/// Zip the logs, settings (passwords removed), session and system facts to the desktop.
pub fn export_diagnostics(settings: &crate::config::Settings) -> Result<PathBuf> {
    let desktop = dirs::desktop_dir().or_else(dirs::document_dir).ok_or_else(|| anyhow!("no desktop folder"))?;
    let path = desktop.join(format!("OpenWX diagnostics {}.zip", chrono::Local::now().format("%Y-%m-%d %H%M")));
    let file = std::fs::File::create(&path)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();

    let mut clean = settings.clone();
    clean.obs_password = if clean.obs_password.is_empty() { String::new() } else { "(removed)".into() };
    zip.start_file("settings.json", opts)?;
    zip.write_all(serde_json::to_string_pretty(&clean)?.as_bytes())?;

    zip.start_file("recent-log.txt", opts)?;
    let log: Vec<String> = crate::devlog::snapshot().iter().map(|e| e.line()).collect();
    zip.write_all(log.join("\n").as_bytes())?;

    if let Ok(events) = std::fs::read(crate::devlog::path()) {
        zip.start_file("dev-events.jsonl", opts)?;
        zip.write_all(&events)?;
    }
    if let Ok(session) = std::fs::read(session_path()) {
        zip.start_file("session.json", opts)?;
        zip.write_all(&session)?;
    }
    zip.start_file("system.txt", opts)?;
    let mem = memory_mb().map(|m| format!("{m:.0} MB")).unwrap_or_else(|| "unknown".into());
    zip.write_all(
        format!(
            "OpenWXBroadcasting {} build {}\nOS: {} {}\nMemory in use: {mem}\nCPUs: {}\nCreated: {}\n",
            crate::config::APP_VERSION,
            crate::config::APP_BUILD,
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
            chrono::Local::now().to_rfc3339(),
        )
        .as_bytes(),
    )?;
    zip.finish()?;
    Ok(path)
}

// ---------------------------------------------------------------------------------------------
// Memory watch
// ---------------------------------------------------------------------------------------------

/// Memory this process is using, in MB.
pub fn memory_mb() -> Option<f64> {
    memory_stats::memory_stats().map(|m| m.physical_mem as f64 / (1024.0 * 1024.0))
}

// ---------------------------------------------------------------------------------------------
// Text to speech (Windows System.Speech through PowerShell; nothing to install)
// ---------------------------------------------------------------------------------------------

pub fn speak(text: &str) {
    let clean: String = text
        .chars()
        .map(|c| if c == '\'' { ' ' } else { c })
        .filter(|c| !c.is_control() || *c == ' ')
        .take(4000)
        .collect();
    let script = format!(
        "Add-Type -AssemblyName System.Speech; $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.Rate = 0; $s.Speak('{clean}')"
    );
    #[cfg(windows)]
    let mut cmd = {
        let mut c = std::process::Command::new("powershell");
        c.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let _ = &script;
        let mut c = std::process::Command::new("say");
        c.arg(&clean);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let _ = &script;
        let mut c = std::process::Command::new("spd-say");
        c.arg(&clean);
        c
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    match cmd.spawn() {
        Ok(_) => log::info!("reading aloud ({} characters)", clean.len()),
        Err(e) => log::warn!("text to speech failed: {e}"),
    }
}

/// Stop any speech started by [`speak`].
pub fn stop_speaking() {
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new(if cfg!(target_os = "macos") { "killall" } else { "spd-say" }).arg(if cfg!(target_os = "macos") { "say" } else { "-S" }).spawn();
        return;
    }
    #[allow(unreachable_code)]
    let mut cmd = std::process::Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "Get-CimInstance Win32_Process -Filter \"Name='powershell.exe'\" | Where-Object { $_.CommandLine -like '*SpeechSynthesizer*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }",
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.creation_flags(0x0800_0000);
    }
    let _ = cmd.spawn();
}

// ---------------------------------------------------------------------------------------------
// Tile cache housekeeping
// ---------------------------------------------------------------------------------------------

/// Trim the map tile cache to `max_mb`, oldest files first. Returns MB freed.
pub fn trim_tile_cache(max_mb: u64) -> u64 {
    let dir = crate::config::tile_cache_dir();
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = Vec::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                stack.push(e.path());
            } else {
                files.push((meta.modified().unwrap_or(std::time::UNIX_EPOCH), meta.len(), e.path()));
            }
        }
    }
    let total: u64 = files.iter().map(|f| f.1).sum();
    let limit = max_mb * 1024 * 1024;
    if total <= limit {
        return 0;
    }
    files.sort_by_key(|f| f.0);
    let mut freed = 0u64;
    for (_, len, path) in files {
        if total - freed <= limit {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            freed += len;
        }
    }
    log::info!("tile cache trimmed by {} MB", freed / (1024 * 1024));
    freed / (1024 * 1024)
}

/// Producer page: a browser view of the rundown and library with take buttons, synced live.
const PRODUCER: &str = r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>OpenWX producer</title>
<style>
body{margin:0;font:15px "Segoe UI",system-ui,sans-serif;background:#14171c;color:#e6e8eb}
header{padding:12px 16px;border-bottom:1px solid #2a2f37;display:flex;gap:12px;align-items:center}
h1{font-size:18px;margin:0;font-weight:600}h2{font-size:15px;margin:16px 0 8px;font-weight:600;color:#9aa3ad}
main{display:grid;grid-template-columns:1fr 1fr;gap:16px;padding:0 16px 16px}
@media(max-width:800px){main{grid-template-columns:1fr}}
.row{display:flex;gap:8px;align-items:center;padding:8px 10px;border:1px solid #2a2f37;border-radius:6px;margin-bottom:6px;background:#1b1f25}
.row.air{border-color:#d9534f;background:#2a1517}.name{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
button{font:inherit;font-size:13px;padding:5px 10px;border-radius:5px;border:1px solid #39404a;background:#232830;color:#e6e8eb;cursor:pointer}
button:hover{background:#2c323b}.tag{font-size:12px;color:#9aa3ad}.ok{color:#5cb85c}.no{color:#e0a030}
img{width:100%;border-radius:6px;background:#000;aspect-ratio:16/9}
</style></head><body>
<header><h1>Producer</h1><span id="st" class="tag"></span><span style="flex:1"></span>
<button onclick="api('next')">Next</button><button onclick="api('live')">Live map</button><button onclick="api('black?on=1')">Black</button><button onclick="api('black?on=0')">Unblack</button></header>
<main><section><h2>Program</h2><img id="pgm" src="/program.jpg?width=640"><h2>Rundown</h2><div id="rd"></div></section>
<section><h2>Graphics</h2><div id="gx"></div></section></main>
<script>
const api=(a)=>fetch('/api/'+a).then(()=>setTimeout(load,300));
const esc=(s)=>String(s).replace(/[&<>"]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]));
async function load(){
 try{
  const st=await (await fetch('/api/status')).json();
  document.getElementById('st').textContent='On air: '+st.on_air+(st.black?' (black)':'');
  const rd=await (await fetch('/api/rundown')).json();
  document.getElementById('rd').innerHTML=rd.scenes.map((s,i)=>`<div class="row ${i===rd.on_air?'air':''}"><span class="tag">${i+1}</span><span class="name">${esc(s.title)}</span><span class="tag">${esc(s.kind)}</span><button onclick="api('take?scene=${i+1}')">Take</button></div>`).join('');
  const gx=await (await fetch('/api/graphics')).json();
  document.getElementById('gx').innerHTML=gx.map(g=>{const n=encodeURIComponent(g.name);return `<div class="row"><span class="name">${esc(g.name)}</span><span class="tag ${g.approved?'ok':'no'}">${g.approved?'approved':'not approved'}</span>${g.approved?'':`<button onclick="api('approve?name=${n}')">Approve</button>`}<button onclick="api('graphic?name=${n}')">Program</button><button onclick="api('channel?n=2&name=${n}')">Ch 2</button><button onclick="api('channel?n=3&name=${n}')">Ch 3</button></div>`}).join('');
 }catch(e){document.getElementById('st').textContent='Not connected';}
 document.getElementById('pgm').src='/program.jpg?width=640&t='+Date.now();
}
load();setInterval(load,2000);
</script></body></html>"#;
