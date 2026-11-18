//! Viewer portal: a small public web server (its own port, never the control API) where viewers
//! send weather photos and storm video, weather reports with their location, and questions, and
//! vote in live polls. Everything lands in Documents\OpenWX\Submissions as pending; nothing
//! reaches air until the station approves it (Viewer submissions).

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::Duration,
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Submission {
    pub id: String,
    /// "photo", "video", "report", "question".
    pub kind: String,
    pub name: String,
    pub place: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub text: String,
    /// Report type ("Hail", "Wind damage", "Flooding", "Snow depth", "Other").
    pub report_type: String,
    /// Stored media file (inside the submission folder).
    pub file: String,
    pub time: i64,
    /// "pending", "approved", "rejected", "onair" (question on air), "answered".
    pub status: String,
    pub consent: bool,
    /// Media library file name once approved.
    pub media: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Poll {
    pub question: String,
    pub options: Vec<String>,
    pub votes: Vec<u32>,
    pub open: bool,
    /// Voter addresses, one vote each.
    #[serde(skip)]
    pub voters: Vec<String>,
}

pub fn dir() -> PathBuf {
    crate::config::LibraryPaths::get().root.join("Submissions")
}

fn poll_store() -> &'static Mutex<Poll> {
    static P: OnceLock<Mutex<Poll>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(std::fs::read_to_string(dir().join("poll.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()))
}

pub fn poll() -> Poll {
    poll_store().lock().map(|p| p.clone()).unwrap_or_default()
}

pub fn set_poll(question: &str, options: Vec<String>) {
    if let Ok(mut p) = poll_store().lock() {
        *p = Poll { question: question.to_string(), votes: vec![0; options.len()], options, open: true, voters: Vec::new() };
        let _ = std::fs::create_dir_all(dir());
        let _ = std::fs::write(dir().join("poll.json"), serde_json::to_string(&*p).unwrap_or_default());
    }
}

pub fn close_poll() {
    if let Ok(mut p) = poll_store().lock() {
        p.open = false;
        let _ = std::fs::write(dir().join("poll.json"), serde_json::to_string(&*p).unwrap_or_default());
    }
}

/// Every submission, newest first.
pub fn list() -> Vec<Submission> {
    let mut out: Vec<Submission> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| serde_json::from_str(&std::fs::read_to_string(e.path().join("meta.json")).ok()?).ok())
        .collect();
    out.sort_by(|a, b| b.time.cmp(&a.time));
    out
}

pub fn save(s: &Submission) -> Result<()> {
    let d = dir().join(&s.id);
    std::fs::create_dir_all(&d)?;
    std::fs::write(d.join("meta.json"), serde_json::to_string_pretty(s)?)?;
    Ok(())
}

/// Approve: photos and video are copied into Media as "Viewer - <name> - <id>.ext".
pub fn approve(id: &str) -> Result<Option<String>> {
    let mut s = list().into_iter().find(|s| s.id == id).ok_or_else(|| anyhow!("not found"))?;
    s.status = "approved".into();
    save(&s)?;
    if s.file.is_empty() {
        return Ok(None);
    }
    let src = dir().join(&s.id).join(&s.file);
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    let clean: String = s.name.chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect();
    let name = format!("Viewer - {} - {}.{ext}", if clean.trim().is_empty() { "Anonymous" } else { clean.trim() }, &s.id[s.id.len().saturating_sub(6)..]);
    std::fs::copy(src, crate::config::LibraryPaths::get().media.join(&name))?;
    s.media = name.clone();
    save(&s)?;
    Ok(Some(name))
}

pub fn set_status(id: &str, status: &str) -> Result<()> {
    let mut s = list().into_iter().find(|s| s.id == id).ok_or_else(|| anyhow!("not found"))?;
    s.status = status.to_string();
    save(&s)
}

/// Start the portal on `port` (all interfaces, so phones can reach it through the station's
/// router or a tunnel). Only the portal pages and endpoints exist here.
pub fn start(port: u16, station: String) {
    let _ = std::thread::Builder::new().name("viewer-portal".into()).spawn(move || {
        let listener = match TcpListener::bind(("0.0.0.0", port)) {
            Ok(l) => l,
            Err(e) => {
                log::warn!("viewer portal could not open port {port}: {e}");
                return;
            }
        };
        log::info!("viewer portal on port {port}");
        for stream in listener.incoming().flatten() {
            let station = station.clone();
            let _ = std::thread::Builder::new().name("portal-conn".into()).spawn(move || {
                if let Err(e) = handle(stream, &station) {
                    log::debug!("portal: {e}");
                }
            });
        }
    });
}

fn respond(stream: &mut TcpStream, status: &str, ctype: &str, body: &[u8]) -> Result<()> {
    write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", body.len())?;
    stream.write_all(body)?;
    Ok(())
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 2;
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

const MAX_UPLOAD: usize = 300 * 1024 * 1024;

fn handle(mut stream: TcpStream, station: &str) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(120)))?;
    let peer = stream.peer_addr().map(|a| a.ip().to_string()).unwrap_or_default();
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let mut length = 0usize;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                length = v.trim().parse().unwrap_or(0);
            }
        }
    }
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let q: Vec<(String, String)> = query.split('&').filter(|p| !p.is_empty()).map(|p| {
        let (k, v) = p.split_once('=').unwrap_or((p, ""));
        (decode(k), decode(v))
    }).collect();
    let param = |k: &str| q.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone()).unwrap_or_default();
    match (method.as_str(), path) {
        ("GET", "/") => respond(&mut stream, "200 OK", "text/html; charset=utf-8", PAGE.replace("{{STATION}}", station).as_bytes()),
        ("GET", "/poll.json") => {
            let p = poll();
            respond(&mut stream, "200 OK", "application/json", serde_json::json!({ "question": p.question, "options": p.options, "votes": p.votes, "open": p.open }).to_string().as_bytes())
        }
        ("POST", "/vote") => {
            let choice = param("option").parse::<usize>().unwrap_or(usize::MAX);
            let ok = poll_store().lock().map(|mut p| {
                if p.open && choice < p.options.len() && !p.voters.contains(&peer) {
                    p.votes[choice] += 1;
                    p.voters.push(peer.clone());
                    true
                } else {
                    false
                }
            }).unwrap_or(false);
            respond(&mut stream, "200 OK", "application/json", if ok { &br#"{"ok":true}"#[..] } else { &br#"{"ok":false}"#[..] })
        }
        ("POST", "/submit") => {
            if length > MAX_UPLOAD {
                return respond(&mut stream, "413 Payload Too Large", "text/plain", b"File too large (300 MB max)");
            }
            let kind = param("kind");
            if !["photo", "video", "report", "question"].contains(&kind.as_str()) {
                return respond(&mut stream, "400 Bad Request", "text/plain", b"unknown kind");
            }
            let id = format!("{}{:04x}", chrono::Utc::now().format("%Y%m%d%H%M%S"), rand_suffix());
            let folder = dir().join(&id);
            std::fs::create_dir_all(&folder)?;
            let mut file = String::new();
            if length > 0 && (kind == "photo" || kind == "video") {
                let original = param("filename");
                let ext = original.rsplit('.').next().filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or(if kind == "photo" { "jpg" } else { "mp4" }).to_lowercase();
                let allowed = if kind == "photo" { ["jpg", "jpeg", "png", "webp", "heic"].contains(&ext.as_str()) } else { ["mp4", "mov", "webm", "m4v"].contains(&ext.as_str()) };
                if !allowed {
                    return respond(&mut stream, "415 Unsupported Media Type", "text/plain", b"That file type isn't accepted");
                }
                file = format!("upload.{ext}");
                let mut out = std::fs::File::create(folder.join(&file))?;
                let mut remaining = length;
                let mut buf = vec![0u8; 64 * 1024];
                while remaining > 0 {
                    let chunk_len = remaining.min(buf.len());
                    let n = reader.read(&mut buf[..chunk_len])?;
                    if n == 0 {
                        break;
                    }
                    out.write_all(&buf[..n])?;
                    remaining -= n;
                }
            }
            let s = Submission {
                id,
                kind,
                name: param("name").chars().take(60).collect(),
                place: param("place").chars().take(80).collect(),
                lat: param("lat").parse().ok(),
                lon: param("lon").parse().ok(),
                text: param("text").chars().take(1000).collect(),
                report_type: param("type").chars().take(40).collect(),
                file,
                time: chrono::Utc::now().timestamp(),
                status: "pending".into(),
                consent: param("consent") == "1",
                media: String::new(),
            };
            save(&s)?;
            respond(&mut stream, "200 OK", "application/json", br#"{"ok":true}"#)
        }
        ("GET", p) if p.starts_with("/public") || p == "/embed" => public_route(&mut stream, p, &param),
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"not found"),
    }
}

fn respond_public(stream: &mut TcpStream, ctype: &str, max_age: u32, body: &[u8]) -> Result<()> {
    write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: public, max-age={max_age}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n", body.len())?;
    stream.write_all(body)?;
    Ok(())
}

fn url_encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Public images, data, widget and embeds (Publishing > Public endpoints), with the optional
/// access key, cache lifetime and usage counting.
fn public_route(stream: &mut TcpStream, path: &str, param: &dyn Fn(&str) -> String) -> Result<()> {
    let snapshot = crate::publish::public().read().ok().and_then(|g| g.as_ref().map(|p| (p.settings.public_endpoints, p.settings.public_key.clone(), p.settings.public_cache_secs.max(10), p.settings.public_graphics.clone(), p.settings.station_name.clone())));
    let Some((on, key, max_age, allowed, station)) = snapshot else {
        return respond(stream, "503 Service Unavailable", "text/plain", b"Starting up, try again in a minute");
    };
    if !on {
        return respond(stream, "404 Not Found", "text/plain", b"not found");
    }
    if !key.is_empty() && param("key") != key {
        return respond(stream, "403 Forbidden", "text/plain", b"A valid key is required");
    }
    crate::publish::record_use(path);
    let ks = if key.is_empty() { String::new() } else { format!("key={}&", url_encode(&key)) };
    let image = |stream: &mut TcpStream, k: &str| match crate::publish::public_image(k) {
        Some(bytes) => respond_public(stream, "image/png", max_age, &bytes),
        None => respond(stream, "503 Service Unavailable", "text/plain", b"Image not ready"),
    };
    match path {
        "/public" | "/public/" => respond(stream, "200 OK", "text/html; charset=utf-8", PUBLIC_INDEX.replace("{{STATION}}", &station).replace("{{KEY}}", &ks).as_bytes()),
        "/public/forecast.png" => image(stream, "seven_day"),
        "/public/current.png" => image(stream, "current"),
        "/public/warning.png" => image(stream, if crate::publish::has_warnings() { "alert" } else { "warning_history" }),
        "/public/radar.jpg" => {
            let jpg = crate::publish::radar_jpg();
            if jpg.is_empty() {
                respond(stream, "503 Service Unavailable", "text/plain", b"Radar not ready")
            } else {
                respond_public(stream, "image/jpeg", 30, &jpg)
            }
        }
        "/public/graphic.png" => {
            let name = param("name");
            if !allowed.iter().any(|a| a.eq_ignore_ascii_case(name.trim())) {
                return respond(stream, "403 Forbidden", "text/plain", b"That graphic isn't public");
            }
            image(stream, name.trim())
        }
        "/public/shows.json" => {
            let (live_url, station) = crate::publish::public()
                .read()
                .ok()
                .and_then(|g| g.as_ref().map(|p| (p.settings.youtube_live_url.clone(), p.settings.station_name.clone())))
                .unwrap_or_default();
            respond_public(stream, "application/json", 30, crate::shows::json(&live_url, &station).as_bytes())
        }
        "/public/data.json" => match crate::publish::public_json() {
            Some(j) => respond_public(stream, "application/json", 60, j.as_bytes()),
            None => respond(stream, "503 Service Unavailable", "text/plain", b"Data not ready"),
        },
        "/public/widget.js" => respond_public(stream, "application/javascript", 300, WIDGET_JS.replace("{{KEY}}", &ks).as_bytes()),
        "/embed" => {
            let src = match param("what").as_str() {
                "current" => format!("/public/current.png?{ks}"),
                "warning" => format!("/public/warning.png?{ks}"),
                "radar" => format!("/public/radar.jpg?{ks}"),
                "graphic" => format!("/public/graphic.png?{ks}name={}&", url_encode(&param("name"))),
                _ => format!("/public/forecast.png?{ks}"),
            };
            let every = if src.contains("radar") { 30_000 } else { max_age * 1000 };
            let page = format!(
                "<!doctype html><meta charset=\"utf-8\"><style>html,body{{margin:0;background:transparent}}img{{width:100%;height:auto;display:block}}</style><img id=\"i\" src=\"{src}\" alt=\"{station} weather\"><script>setInterval(function(){{document.getElementById('i').src='{src}t='+Date.now()}},{every})</script>",
                station = station.replace(['<', '>', '"', '&'], ""),
            );
            respond(stream, "200 OK", "text/html; charset=utf-8", page.as_bytes())
        }
        _ => respond(stream, "404 Not Found", "text/plain", b"not found"),
    }
}

const PUBLIC_INDEX: &str = r##"<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{{STATION}} weather feeds</title>
<style>body{font:15px system-ui;background:#10141c;color:#e8eaee;max-width:760px;margin:32px auto;padding:0 16px}code{color:#7fb0f5;word-break:break-all}li{margin:8px 0}img{max-width:100%;border-radius:8px}</style>
<h1>{{STATION}} weather feeds</h1>
<ul>
<li><a href="/public/forecast.png?{{KEY}}">Forecast image</a> <code>/public/forecast.png</code></li>
<li><a href="/public/current.png?{{KEY}}">Current conditions image</a> <code>/public/current.png</code></li>
<li><a href="/public/warning.png?{{KEY}}">Warnings image</a> <code>/public/warning.png</code></li>
<li><a href="/public/radar.jpg?{{KEY}}">Radar image</a> <code>/public/radar.jpg</code></li>
<li><a href="/public/data.json?{{KEY}}">Data (JSON)</a> <code>/public/data.json</code></li>
</ul>
<h2>Widget</h2><p>Paste where the widget should appear:</p>
<code>&lt;script src="http://THIS-ADDRESS/public/widget.js?{{KEY}}"&gt;&lt;/script&gt;</code>
<h2>Embed</h2><p>Self-refreshing image for any page:</p>
<code>&lt;iframe src="http://THIS-ADDRESS/embed?{{KEY}}what=forecast" width="640" height="360" frameborder="0"&gt;&lt;/iframe&gt;</code>
<p>what = forecast, current, warning, radar, or graphic&amp;name=…</p>
<script>document.querySelectorAll('code').forEach(function(c){c.textContent=c.textContent.replace('THIS-ADDRESS',location.host)})</script>"##;

const WIDGET_JS: &str = r##"(function(){var s=document.currentScript;var base=s.src.replace(/\/public\/widget\.js.*$/,'');var el=document.createElement('div');el.style.cssText='font:14px system-ui;background:#10141c;color:#e8eaee;border-radius:10px;padding:12px 14px;max-width:320px';s.parentNode.insertBefore(el,s);
function esc(t){return String(t==null?'':t).replace(/[&<>"]/g,function(c){return {'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]})}
function load(){fetch(base+'/public/data.json?{{KEY}}').then(function(r){return r.json()}).then(function(d){var v=d.values||{};var h='<div style="opacity:.7;font-size:12px">'+esc(d.station)+' &bull; '+esc(v.location)+'</div><div style="font-size:34px;font-weight:700">'+esc(v.temp)+'</div><div>'+esc(v.condition)+'</div>';(d.warnings||[]).forEach(function(w){h+='<div style="margin-top:8px;padding:6px 8px;border-radius:6px;background:#8a1c1c">'+esc(w.event)+'</div>'});el.innerHTML=h}).catch(function(){})}
load();setInterval(load,120000)})();"##;

fn rand_suffix() -> u16 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64);
    h.finish() as u16
}

const PAGE: &str = r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{{STATION}}: send us your weather</title>
<style>
body{margin:0;font:16px system-ui,sans-serif;background:#f4f6f8;color:#1b1f24}
main{max-width:560px;margin:0 auto;padding:16px}
h1{font-size:22px;margin:8px 0 4px}h2{font-size:17px;margin:20px 0 8px}
p.note{color:#5b6570;font-size:14px;margin:0 0 12px}
.card{background:#fff;border-radius:10px;padding:14px;margin-bottom:14px;box-shadow:0 1px 3px rgba(0,0,0,.08)}
label{display:block;font-size:14px;margin:10px 0 4px;color:#3a434d}
input,select,textarea,button{font:inherit;width:100%;box-sizing:border-box;padding:10px;border:1px solid #c9d1d9;border-radius:8px;background:#fff}
button{background:#2f6fd6;color:#fff;border:0;margin-top:12px;cursor:pointer}
.tabs{display:flex;gap:6px;flex-wrap:wrap}.tabs button{width:auto;padding:8px 12px;background:#e6ebf1;color:#1b1f24;margin:0}
.tabs button.on{background:#2f6fd6;color:#fff}.ok{color:#1a7f37}.err{color:#b42318}
.bar{height:10px;background:#2f6fd6;border-radius:5px}
</style></head><body><main>
<h1>{{STATION}}</h1><p class="note">Send us your weather. We review everything before it airs.</p>
<div class="tabs"><button class="on" onclick="tab('photo',this)">Photo</button><button onclick="tab('video',this)">Video</button><button onclick="tab('report',this)">Report</button><button onclick="tab('question',this)">Question</button></div>
<div class="card" id="form">
<div id="filebox"><label>File</label><input type="file" id="file" accept="image/*"></div>
<div id="typebox" hidden><label>What happened</label><select id="type"><option>Hail</option><option>Wind damage</option><option>Flooding</option><option>Snow depth</option><option>Tornado or funnel</option><option>Other</option></select></div>
<label id="textlabel">Caption</label><textarea id="text" rows="3"></textarea>
<label>Your name</label><input id="name">
<label>Town</label><input id="place"><button type="button" style="background:#e6ebf1;color:#1b1f24" onclick="locate()">Use my location</button>
<label><input type="checkbox" id="consent" style="width:auto"> You can show this on air and online with my name</label>
<button onclick="send()">Send</button><p id="status"></p></div>
<div class="card" id="pollcard" hidden><h2 id="pq"></h2><div id="po"></div></div>
</main><script>
let kind='photo',lat='',lon='';
function tab(k,b){kind=k;document.querySelectorAll('.tabs button').forEach(x=>x.classList.remove('on'));b.classList.add('on');
document.getElementById('filebox').hidden=!(k=='photo'||k=='video');document.getElementById('file').accept=k=='video'?'video/*':'image/*';
document.getElementById('typebox').hidden=k!='report';document.getElementById('textlabel').textContent=k=='question'?'Your question':(k=='report'?'Details (size, depth, damage)':'Caption');}
function locate(){navigator.geolocation&&navigator.geolocation.getCurrentPosition(p=>{lat=p.coords.latitude.toFixed(4);lon=p.coords.longitude.toFixed(4);document.getElementById('status').textContent='Location added';});}
async function send(){const st=document.getElementById('status');const f=document.getElementById('file').files[0];
if((kind=='photo'||kind=='video')&&!f){st.textContent='Choose a file first';st.className='err';return;}
const q=new URLSearchParams({kind,name:name.value,place:place.value,text:text.value,type:type.value,lat,lon,consent:consent.checked?'1':'0',filename:f?f.name:''});
st.textContent='Sending…';st.className='';
try{const r=await fetch('/submit?'+q,{method:'POST',body:(kind=='photo'||kind=='video')?f:''});st.textContent=r.ok?'Thank you! We got it.':'Something went wrong';st.className=r.ok?'ok':'err';}catch(e){st.textContent='Could not send';st.className='err';}}
async function poll(){try{const p=await (await fetch('/poll.json')).json();if(!p.question){return;}document.getElementById('pollcard').hidden=false;pq.textContent=p.question;
const total=p.votes.reduce((a,b)=>a+b,0)||1;po.innerHTML=p.options.map((o,i)=>`<button ${p.open?'':'disabled'} onclick="vote(${i})" style="text-align:left;background:#e6ebf1;color:#1b1f24">${o}<div class="bar" style="width:${Math.round(p.votes[i]/total*100)}%"></div></button>`).join('');}catch(e){}}
async function vote(i){await fetch('/vote?option='+i,{method:'POST'});poll();}
poll();setInterval(poll,10000);
</script></body></html>"#;
