//! More broadcast operations:
//! - warning acknowledgment, operator assignment, on-air (delivery) tracking and an audit trail,
//!   shared through a sync folder so several workstations see the same status;
//! - quality incidents (log, report) and operator notifications (Windows toast, phone push via
//!   a webhook such as ntfy.sh);
//! - control inputs beyond HTTP: OSC (UDP), MOS (TCP, for newsroom systems), GPI over HTTP / MIDI,
//!   a timed command queue, GPO-style webhooks out, and a diagnostics log of every command;
//! - switchers through Bitfocus Companion (ATEM, TriCaster and many more), TriCaster's own HTTP
//!   shortcuts, tally webhooks and OBS stream / audio health;
//! - data-source metrics (latency, success, last fetch) and an access log.

use std::{
    collections::HashMap,
    io::Write as _,
    sync::{mpsc::Sender, Mutex, OnceLock},
    time::Duration,
};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------------------------
// Warning status (ack / operator / aired), audit trail, workstation sync
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WarningStatus {
    /// Alert id -> (unix time acknowledged, operator).
    pub acks: HashMap<String, (i64, String)>,
    /// Alert id -> operator assigned to cover it.
    pub assigned: HashMap<String, String>,
    /// Alert id -> unix time it first went on air (call-out, graphic or break-in).
    pub aired: HashMap<String, i64>,
    pub updated: i64,
}

fn status_path(sync_folder: &str) -> std::path::PathBuf {
    if sync_folder.trim().is_empty() {
        crate::config::config_dir().join("warning-status.json")
    } else {
        std::path::PathBuf::from(sync_folder.trim()).join("OpenWX warning-status.json")
    }
}

pub fn load_status(sync_folder: &str) -> WarningStatus {
    std::fs::read_to_string(status_path(sync_folder)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Merge with what other workstations wrote, then save.
pub fn save_status(sync_folder: &str, mine: &mut WarningStatus) {
    let theirs = load_status(sync_folder);
    for (k, v) in theirs.acks {
        mine.acks.entry(k).or_insert(v);
    }
    for (k, v) in theirs.assigned {
        mine.assigned.entry(k).or_insert(v);
    }
    for (k, v) in theirs.aired {
        mine.aired.entry(k).and_modify(|t| *t = (*t).min(v)).or_insert(v);
    }
    mine.updated = chrono::Utc::now().timestamp();
    let path = status_path(sync_folder);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(mine) {
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(tmp, path);
        }
    }
}

/// One line in today's audit trail (Warning archive\audit-YYYY-MM-DD.log).
pub fn audit(line: &str, operator: &str) {
    let dir = crate::ops::archive_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("audit-{}.log", chrono::Local::now().format("%Y-%m-%d")));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let who = if operator.trim().is_empty() { "operator" } else { operator.trim() };
        let _ = writeln!(f, "{} [{who}] {line}", chrono::Local::now().format("%H:%M:%S"));
    }
}

// ---------------------------------------------------------------------------------------------
// Captions
// ---------------------------------------------------------------------------------------------

/// Fix common misspellings and casing of weather words in caption text.
pub fn correct_caption(text: &str) -> String {
    const FIXES: &[(&str, &str)] = &[
        ("tornato", "tornado"),
        ("tornadoe ", "tornado "),
        ("thunder storm", "thunderstorm"),
        ("thunderstorm's", "thunderstorms"),
        ("hale ", "hail "),
        ("derechos", "derechos"),
        ("dereko", "derecho"),
        ("dopler", "Doppler"),
        ("doppler", "Doppler"),
        ("nws", "NWS"),
        ("noaa", "NOAA"),
        ("spc", "SPC"),
        ("mph", "mph"),
        ("wind chil ", "wind chill "),
        ("heat indexes", "heat index values"),
        ("flash flood warning", "Flash Flood Warning"),
        ("tornado warning", "Tornado Warning"),
        ("tornado watch", "Tornado Watch"),
        ("severe thunderstorm warning", "Severe Thunderstorm Warning"),
        ("severe thunderstorm watch", "Severe Thunderstorm Watch"),
        ("winter storm warning", "Winter Storm Warning"),
    ];
    let mut out = format!("{text} ");
    for (bad, good) in FIXES {
        let lower = out.to_lowercase();
        if let Some(i) = lower.find(bad) {
            out.replace_range(i..i + bad.len(), good);
        }
    }
    out.trim_end().to_string()
}

// ---------------------------------------------------------------------------------------------
// Incidents and notifications
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Incident {
    pub time: i64,
    pub kind: String,
    pub detail: String,
}

fn incidents_path() -> std::path::PathBuf {
    crate::devlog::path().with_file_name("incidents.jsonl")
}

pub fn log_incident(kind: &str, detail: &str) {
    let i = Incident { time: chrono::Utc::now().timestamp(), kind: kind.into(), detail: detail.into() };
    if let Some(dir) = incidents_path().parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let (Ok(mut f), Ok(line)) = (std::fs::OpenOptions::new().create(true).append(true).open(incidents_path()), serde_json::to_string(&i)) {
        let _ = writeln!(f, "{line}");
    }
    log::warn!("incident: {kind}: {detail}");
}

pub fn incidents() -> Vec<Incident> {
    std::fs::read_to_string(incidents_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Incident report (Markdown) for a date range, saved to the desktop.
pub fn incident_report(days: i64) -> anyhow::Result<std::path::PathBuf> {
    let since = chrono::Utc::now().timestamp() - days * 86_400;
    let list: Vec<Incident> = incidents().into_iter().filter(|i| i.time >= since).collect();
    let mut md = format!("# Broadcast incidents, last {days} day(s)\n\nGenerated {}\n\n", chrono::Local::now().format("%Y-%m-%d %H:%M"));
    let mut by_kind: HashMap<String, usize> = HashMap::new();
    for i in &list {
        *by_kind.entry(i.kind.clone()).or_default() += 1;
    }
    md.push_str("## Summary\n\n");
    for (k, n) in &by_kind {
        md.push_str(&format!("- {k}: {n}\n"));
    }
    md.push_str("\n## Log\n\n| Time | Kind | Detail |\n|---|---|---|\n");
    for i in &list {
        let t = chrono::DateTime::from_timestamp(i.time, 0).map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default();
        md.push_str(&format!("| {t} | {} | {} |\n", i.kind, i.detail.replace('|', "/")));
    }
    let desktop = dirs::desktop_dir().or_else(dirs::document_dir).ok_or_else(|| anyhow::anyhow!("no desktop"))?;
    let path = desktop.join(format!("OpenWX incidents {}.md", chrono::Local::now().format("%Y-%m-%d")));
    std::fs::write(&path, md)?;
    Ok(path)
}

/// Windows notification (Action Center) and, if set, a push to a webhook (ntfy.sh, Slack, …).
pub fn notify(title: &str, body: &str, webhook: &str) {
    let clean = |s: &str| s.replace(['\'', '"', '`', '$'], " ");
    let script = format!(
        "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; \
         $t = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
         $x = $t.GetElementsByTagName('text'); $x.Item(0).AppendChild($t.CreateTextNode('{}')) > $null; $x.Item(1).AppendChild($t.CreateTextNode('{}')) > $null; \
         [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('OpenWX Broadcaster').Show([Windows.UI.Notifications.ToastNotification]::new($t))",
        clean(title),
        clean(body)
    );
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        let mut cmd = std::process::Command::new("powershell");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
        cmd.creation_flags(0x0800_0000);
        let _ = cmd.spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = &script;
        let _ = std::process::Command::new("osascript").arg("-e").arg(format!("display notification \"{}\" with title \"{}\"", clean(body), clean(title))).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = &script;
        let _ = std::process::Command::new("notify-send").arg(clean(title)).arg(clean(body)).spawn();
    }
    if !webhook.trim().is_empty() {
        let (url, msg) = (webhook.trim().to_string(), format!("{title}: {body}"));
        crate::net::spawn(async move {
            let client = reqwest::Client::new();
            if let Err(e) = client.post(url).body(msg).timeout(Duration::from_secs(10)).send().await {
                log::warn!("notification webhook failed: {e}");
            }
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Control inputs: OSC, MOS, GPO webhooks, timed queue, diagnostics
// ---------------------------------------------------------------------------------------------

/// Diagnostics: the last 200 control commands with their source.
fn control_log() -> &'static Mutex<Vec<String>> {
    static LOG: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    LOG.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn log_control(source: &str, what: &str) {
    let line = format!("{} [{source}] {what}", chrono::Local::now().format("%H:%M:%S"));
    log::info!("control: {line}");
    if let Ok(mut l) = control_log().lock() {
        l.push(line);
        if l.len() > 200 {
            l.remove(0);
        }
    }
}

pub fn control_lines() -> Vec<String> {
    control_log().lock().map(|l| l.clone()).unwrap_or_default()
}

/// Map an action word (OSC address, MOS item, GPI pin action) to a command.
pub fn action_command(action: &str, arg: &str) -> Option<crate::ops::Command> {
    use crate::ops::Command;
    let a = action.trim().trim_start_matches('/').trim_start_matches("cwx/").to_lowercase();
    Some(match a.as_str() {
        "next" => Command::Next,
        "prev" | "previous" => Command::Prev,
        "live" => Command::Live,
        "black" => Command::Black(arg != "0"),
        "backup" => Command::Backup(arg != "0"),
        "refresh" => Command::Refresh,
        "severe" => Command::Severe(if arg.is_empty() { "severe".into() } else { arg.to_string() }),
        "take" => Command::TakeScene(arg.parse::<usize>().ok()?.checked_sub(1)?),
        "graphic" => Command::Graphic(arg.to_string()),
        "caption" => Command::Caption(arg.to_string()),
        "cue" => Command::Cue(arg.to_string()),
        "audio" => Command::PlayAudio(arg.to_string()),
        "state" => Command::Interact(format!("state:{arg}")),
        "pause" => Command::Interact("pause".into()),
        "resume" => Command::Interact("play".into()),
        "ack" | "acknowledge" => Command::Acknowledge(arg.to_string()),
        "channel2" => Command::ChannelTake(2, arg.to_string()),
        "channel3" => Command::ChannelTake(3, arg.to_string()),
        "clear2" => Command::ChannelClear(2),
        "clear3" => Command::ChannelClear(3),
        _ => return None,
    })
}

/// OSC over UDP: /cwx/next, /cwx/take 3, /cwx/graphic "7-Day Forecast", /cwx/severe "tornado".
pub fn start_osc(port: u16, tx: Sender<crate::ops::Command>) {
    std::thread::Builder::new()
        .name("osc".into())
        .spawn(move || {
            let Ok(sock) = std::net::UdpSocket::bind(("0.0.0.0", port)) else {
                log::warn!("OSC: port {port} unavailable");
                return;
            };
            log::info!("OSC control listening on UDP {port}");
            let mut buf = [0u8; 2048];
            while let Ok((n, from)) = sock.recv_from(&mut buf) {
                let msg = &buf[..n];
                // Address (padded string), type tags ",s"/",i"/",f", then the first argument.
                let s_end = msg.iter().position(|b| *b == 0).unwrap_or(n);
                let address = String::from_utf8_lossy(&msg[..s_end]).to_string();
                let mut i = (s_end + 4) & !3;
                let mut arg = String::new();
                if i < n && msg[i] == b',' {
                    let tags_end = msg[i..].iter().position(|b| *b == 0).map(|p| i + p).unwrap_or(n);
                    let tags = String::from_utf8_lossy(&msg[i + 1..tags_end]).to_string();
                    i = (tags_end + 4) & !3;
                    match tags.chars().next() {
                        Some('s') => {
                            let e = msg[i..].iter().position(|b| *b == 0).map(|p| i + p).unwrap_or(n);
                            arg = String::from_utf8_lossy(&msg[i..e]).to_string();
                        }
                        Some('i') if i + 4 <= n => arg = i32::from_be_bytes([msg[i], msg[i + 1], msg[i + 2], msg[i + 3]]).to_string(),
                        Some('f') if i + 4 <= n => arg = (f32::from_be_bytes([msg[i], msg[i + 1], msg[i + 2], msg[i + 3]]) as i64).to_string(),
                        _ => {}
                    }
                }
                log_control(&format!("OSC {from}"), &format!("{address} {arg}"));
                if let Some(cmd) = action_command(&address, &arg) {
                    if tx.send(cmd).is_err() {
                        break;
                    }
                }
            }
        })
        .ok();
}

/// MOS (newsroom systems): a TCP listener that answers heartbeats and acts on item cues.
/// A rundown item whose <objID> or <itemSlug> names a library graphic is taken to air when the
/// newsroom system sends roItemCue / roElementAction "TAKE", and the next scene on "NEXT".
pub fn start_mos(port: u16, mos_id: String, tx: Sender<crate::ops::Command>) {
    std::thread::Builder::new()
        .name("mos".into())
        .spawn(move || {
            let Ok(listener) = std::net::TcpListener::bind(("0.0.0.0", port)) else {
                log::warn!("MOS: port {port} unavailable");
                return;
            };
            log::info!("MOS listening on TCP {port} as {mos_id}");
            for stream in listener.incoming().flatten() {
                let tx = tx.clone();
                let mos_id = mos_id.clone();
                std::thread::spawn(move || {
                    let mut stream = stream;
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
                    let mut buf = vec![0u8; 64 * 1024];
                    loop {
                        let n = match std::io::Read::read(&mut stream, &mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => n,
                        };
                        // MOS is UTF-16BE; accept UTF-8 too.
                        let text = if n >= 2 && buf[0] == 0 {
                            let units: Vec<u16> = buf[..n].chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
                            String::from_utf16_lossy(&units)
                        } else {
                            String::from_utf8_lossy(&buf[..n]).to_string()
                        };
                        let field = |name: &str| -> Option<String> {
                            let o = format!("<{name}>");
                            let i = text.find(&o)? + o.len();
                            let e = text[i..].find('<')? + i;
                            Some(text[i..e].trim().to_string())
                        };
                        let message_id = field("messageID").unwrap_or_else(|| "0".into());
                        let reply = if text.contains("<heartbeat>") {
                            format!("<mos><mosID>{mos_id}</mosID><ncsID>NCS</ncsID><messageID>{message_id}</messageID><heartbeat><time>{}</time></heartbeat></mos>", chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S"))
                        } else {
                            let cue = text.contains("roItemCue") || (text.contains("roElementAction") && text.to_uppercase().contains("TAKE"));
                            let next = text.contains("roElementAction") && text.to_uppercase().contains("NEXT");
                            if cue {
                                if let Some(name) = field("objID").or_else(|| field("itemSlug")).or_else(|| field("objSlug")) {
                                    log_control("MOS", &format!("take {name}"));
                                    let _ = tx.send(crate::ops::Command::Graphic(name));
                                }
                            } else if next {
                                log_control("MOS", "next");
                                let _ = tx.send(crate::ops::Command::Next);
                            }
                            format!("<mos><mosID>{mos_id}</mosID><ncsID>NCS</ncsID><messageID>{message_id}</messageID><roAck><roID>0</roID><roStatus>OK</roStatus></roAck></mos>")
                        };
                        let bytes: Vec<u8> = reply.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
                        if stream.write_all(&bytes).is_err() {
                            break;
                        }
                    }
                });
            }
        })
        .ok();
}

/// Fire a GPO-style webhook (tally lights, external automation) for an event.
pub fn fire_webhook(url: &str, event: &str, detail: &str) {
    if url.trim().is_empty() {
        return;
    }
    let url = url.trim().replace("{event}", event).replace("{detail}", &detail.replace(' ', "%20"));
    let body = serde_json::json!({ "event": event, "detail": detail, "time": chrono::Utc::now().to_rfc3339() }).to_string();
    crate::net::spawn(async move {
        let client = reqwest::Client::new();
        if let Err(e) = client.post(&url).header("Content-Type", "application/json").body(body).timeout(Duration::from_secs(8)).send().await {
            log::warn!("webhook {url} failed: {e}");
        }
    });
}

/// A command scheduled for a clock time (Show > Schedule).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Scheduled {
    /// Local time "HH:MM".
    pub at: String,
    /// Action word: next, take, graphic, black, live, severe, backup, refresh.
    pub action: String,
    pub arg: String,
    /// Repeat every day (otherwise runs once).
    pub daily: bool,
    /// Date last run (YYYY-MM-DD) so daily items run once a day.
    pub last_run: String,
}

// ---------------------------------------------------------------------------------------------
// Switchers: Companion (ATEM, TriCaster, …), TriCaster HTTP, tally
// ---------------------------------------------------------------------------------------------

/// Press a Bitfocus Companion button (page/row/column), e.g. an ATEM cut or macro.
pub async fn companion_press(base: String, location: String) -> anyhow::Result<String> {
    let url = format!("{}/api/location/{}/press", base.trim_end_matches('/'), location.trim().replace(' ', ""));
    let resp = reqwest::Client::new().post(&url).timeout(Duration::from_secs(5)).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Companion answered {}", resp.status());
    }
    Ok(format!("Companion button {location}"))
}

/// TriCaster shortcut (e.g. main_take, main_auto, macro name).
pub async fn tricaster(base: String, shortcut: String) -> anyhow::Result<String> {
    let url = format!("{}/v1/shortcut?name={}", base.trim_end_matches('/'), shortcut);
    let resp = crate::net::get_logged(&url, Duration::from_secs(5)).await?;
    if !resp.status().is_success() {
        anyhow::bail!("TriCaster answered {}", resp.status());
    }
    Ok(format!("TriCaster {shortcut}"))
}

/// OBS hotkey by name (switcher macros set up in OBS).
pub async fn obs_hotkey(url: String, password: String, name: String) -> anyhow::Result<String> {
    crate::obs::raw_request(&url, &password, "TriggerHotkeyByName", serde_json::json!({ "hotkeyName": name })).await?;
    Ok(format!("OBS hotkey {name}"))
}

/// OBS stream / output health for the monitor.
#[derive(Clone, Debug, Default)]
pub struct ObsHealth {
    pub streaming: bool,
    pub recording: bool,
    /// kbit/s since the previous sample.
    pub bitrate_kbps: f64,
    pub skipped_frames: u64,
    pub total_frames: u64,
    pub congestion: f64,
    pub render_skipped: u64,
    pub output_skipped: u64,
    pub cpu: f64,
    pub fps: f64,
    pub audio_sync_ms: Option<f64>,
    pub stream_timecode: String,
}

pub async fn obs_health(url: String, password: String, mic_input: String) -> anyhow::Result<(ObsHealth, u64)> {
    let stream = crate::obs::raw_request(&url, &password, "GetStreamStatus", serde_json::json!({})).await?;
    let rec = crate::obs::raw_request(&url, &password, "GetRecordStatus", serde_json::json!({})).await.unwrap_or_default();
    let stats = crate::obs::raw_request(&url, &password, "GetStats", serde_json::json!({})).await.unwrap_or_default();
    let sync = if mic_input.trim().is_empty() {
        None
    } else {
        crate::obs::raw_request(&url, &password, "GetInputAudioSyncOffset", serde_json::json!({ "inputName": mic_input }))
            .await
            .ok()
            .and_then(|v| v["inputAudioSyncOffset"].as_f64())
    };
    let n = |v: &serde_json::Value, k: &str| v[k].as_f64().unwrap_or(0.0);
    let bytes = stream["outputBytes"].as_u64().unwrap_or(0);
    Ok((
        ObsHealth {
            streaming: stream["outputActive"].as_bool().unwrap_or(false),
            recording: rec["outputActive"].as_bool().unwrap_or(false),
            bitrate_kbps: 0.0,
            skipped_frames: stream["outputSkippedFrames"].as_u64().unwrap_or(0),
            total_frames: stream["outputTotalFrames"].as_u64().unwrap_or(0),
            congestion: n(&stream, "outputCongestion"),
            render_skipped: stats["renderSkippedFrames"].as_u64().unwrap_or(0),
            output_skipped: stats["outputSkippedFrames"].as_u64().unwrap_or(0),
            cpu: n(&stats, "cpuUsage"),
            fps: n(&stats, "activeFps"),
            audio_sync_ms: sync,
            stream_timecode: stream["outputTimecode"].as_str().unwrap_or("").to_string(),
        },
        bytes,
    ))
}

/// OBS audio meters over a persistent connection: returns the loudest input peak (dBFS) seen in
/// `seconds` (None if OBS sent no meter events).
pub async fn obs_audio_peak(url: String, password: String, seconds: u64) -> anyhow::Result<Option<f32>> {
    crate::obs::meter_peak(&url, &password, seconds).await
}

// ---------------------------------------------------------------------------------------------
// Data-source metrics and access log
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct SourceStats {
    pub requests: u64,
    pub failures: u64,
    pub total_ms: u64,
    pub last_ok: Option<i64>,
    pub last_error: String,
}

impl SourceStats {
    pub fn avg_ms(&self) -> u64 {
        if self.requests == 0 { 0 } else { self.total_ms / self.requests }
    }

    /// 0-100: success rate, speed and freshness.
    pub fn quality(&self) -> u32 {
        if self.requests == 0 {
            return 0;
        }
        let success = 1.0 - self.failures as f64 / self.requests as f64;
        let speed = (1.0 - (self.avg_ms() as f64 / 5000.0)).clamp(0.0, 1.0);
        let fresh = match self.last_ok {
            Some(t) if chrono::Utc::now().timestamp() - t < 3600 => 1.0,
            Some(_) => 0.5,
            None => 0.0,
        };
        ((success * 0.6 + speed * 0.2 + fresh * 0.2) * 100.0).round() as u32
    }
}

fn stats() -> &'static Mutex<HashMap<String, SourceStats>> {
    static S: OnceLock<Mutex<HashMap<String, SourceStats>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Record one request (called from the network layer).
pub fn record_fetch(url: &str, ms: u64, ok: bool, error: &str) {
    let host = url.split("//").nth(1).and_then(|r| r.split('/').next()).unwrap_or(url).to_string();
    if let Ok(mut m) = stats().lock() {
        let s = m.entry(host.clone()).or_default();
        s.requests += 1;
        s.total_ms += ms;
        if ok {
            s.last_ok = Some(chrono::Utc::now().timestamp());
        } else {
            s.failures += 1;
            s.last_error = error.chars().take(200).collect();
        }
    }
    // Access log (one line per request), one file per day.
    let path = crate::devlog::path().with_file_name(format!("data-access-{}.log", chrono::Local::now().format("%Y-%m-%d")));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} {host} {ms}ms {}", chrono::Local::now().format("%H:%M:%S"), if ok { "ok" } else { "FAILED" });
    }
}

pub fn source_stats() -> Vec<(String, SourceStats)> {
    let mut v: Vec<(String, SourceStats)> = stats().lock().map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    v.sort_by(|a, b| b.1.requests.cmp(&a.1.requests));
    v
}

/// Delete warning-archive, access-log and export files older than `days` (0 = keep forever).
pub fn apply_retention(days: u32) -> usize {
    if days == 0 {
        return 0;
    }
    let cutoff = std::time::SystemTime::now() - Duration::from_secs(days as u64 * 86_400);
    let mut removed = 0;
    let dirs = [crate::ops::archive_dir(), crate::devlog::path().parent().map(|p| p.to_path_buf()).unwrap_or_default(), crate::config::LibraryPaths::get().exports];
    for dir in dirs {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let Ok(meta) = e.metadata() else { continue };
            let name = e.file_name().to_string_lossy().to_string();
            let eligible = name.ends_with(".jsonl") || name.starts_with("audit-") || name.starts_with("data-access-") || name.ends_with(".png") || name.ends_with(".jpg") || name.ends_with(".mp4") || name.ends_with(".gif");
            if meta.is_file() && eligible && name != "dev-events.jsonl" && name != "incidents.jsonl" && meta.modified().map(|m| m < cutoff).unwrap_or(false) && std::fs::remove_file(e.path()).is_ok() {
                removed += 1;
            }
        }
    }
    if removed > 0 {
        log::info!("retention: removed {removed} file(s) older than {days} days");
    }
    removed
}
