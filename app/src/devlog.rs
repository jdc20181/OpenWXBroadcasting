//! Developer log.
//!
//! Every record the app logs, every failed HTTP request (status, URL, response text) and every
//! error toast is kept in memory for the Developer Logs panel (Ctrl+Shift+D, or the counter in the
//! status bar). Warnings and errors are also written to `logs/dev-events.jsonl`, one JSON object
//! per line, so a session can be reviewed or shared outside the app.

use std::{
    collections::VecDeque,
    io::Write as _,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
        }
    }

    fn from_log(level: log::Level) -> Self {
        match level {
            log::Level::Error => Level::Error,
            log::Level::Warn => Level::Warn,
            log::Level::Info => Level::Info,
            log::Level::Debug | log::Level::Trace => Level::Debug,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub seq: u64,
    pub at: chrono::DateTime<chrono::Local>,
    pub level: Level,
    /// Where it came from: a module path, `http`, `toast`, …
    pub source: String,
    pub message: String,
}

impl Entry {
    pub fn line(&self) -> String {
        format!("{} {:<5} [{}] {}", self.at.format("%H:%M:%S%.3f"), self.level.label(), self.source, self.message)
    }

    /// Network-related: failed requests, tile fetches, connection errors.
    pub fn is_network(&self) -> bool {
        self.source == "http"
            || self.source == "map tiles"
            || self.source.contains("request")
            || self.message.contains("http://")
            || self.message.contains("https://")
    }
}

const MAX_ENTRIES: usize = 4000;

struct Store {
    entries: VecDeque<Entry>,
    file: Option<std::fs::File>,
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| {
        let path = path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::rename(&path, path.with_extension("old.jsonl"));
        Mutex::new(Store { entries: VecDeque::with_capacity(MAX_ENTRIES), file: std::fs::File::create(&path).ok() })
    })
}

static SEQ: AtomicU64 = AtomicU64::new(0);
static ERRORS: AtomicU64 = AtomicU64::new(0);
static WARNINGS: AtomicU64 = AtomicU64::new(0);

/// `logs/dev-events.jsonl`, next to the main log file.
pub fn path() -> PathBuf {
    crate::config::log_path().with_file_name("dev-events.jsonl")
}

/// Add an entry. Never calls the `log` macros (the logger calls this).
pub fn push(level: Level, source: impl Into<String>, message: impl Into<String>) {
    let entry = Entry {
        seq: SEQ.fetch_add(1, Ordering::Relaxed) + 1,
        at: chrono::Local::now(),
        level,
        source: source.into(),
        message: message.into(),
    };
    match level {
        Level::Error => {
            ERRORS.fetch_add(1, Ordering::Relaxed);
        }
        Level::Warn => {
            WARNINGS.fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }
    let Ok(mut store) = store().lock() else { return };
    if level <= Level::Warn {
        if let Some(file) = store.file.as_mut() {
            let line = serde_json::json!({
                "t": entry.at.to_rfc3339(),
                "level": level.label(),
                "source": entry.source,
                "msg": entry.message,
                "build": crate::config::APP_BUILD,
            });
            let _ = writeln!(file, "{line}");
        }
    }
    store.entries.push_back(entry);
    while store.entries.len() > MAX_ENTRIES {
        store.entries.pop_front();
    }
}

/// A failed HTTP request: status, URL and the start of the response body.
pub fn http_failure(url: &str, status: reqwest::StatusCode, body: &str) {
    http_failure_level(url, status, body, false);
}

/// `optional`: a feed the app works fine without (airport delays…): its failures are warnings,
/// so one flaky optional route doesn't hold the error counter up for the whole session.
pub fn http_failure_level(url: &str, status: reqwest::StatusCode, body: &str, optional: bool) {
    let body = body.trim();
    let body: String = body.chars().take(300).collect();
    let hint = match status.as_u16() {
        429 => " (rate limited: too many requests to this service)",
        401 | 403 => " (refused: key, origin or permission)",
        500..=599 => " (gateway or upstream error)",
        _ => "",
    };
    let message = if body.is_empty() {
        format!("{status}{hint} • {url}")
    } else {
        format!("{status}{hint} • {url} • {body}")
    };
    let serious = !optional && (status.as_u16() == 429 || status.is_server_error());
    let message = if optional { format!("{message} (optional feed: tried again on the next refresh)") } else { message };
    push(if serious { Level::Error } else { Level::Warn }, "http", message);
}

/// The relay couldn't serve a public feed, so the app read the source directly. A warning:
/// the data still arrived.
pub fn relay_fallback(direct_url: &str, relay_problem: &str) {
    push(Level::Warn, "http", format!("relay {relay_problem}; read directly instead • {direct_url}"));
}

/// The relay doesn't serve a feed (it answered 4xx): logged once per feed per session, as info.
/// The feed is read straight from its public source from then on.
pub fn relay_refused(route: &str, example_url: &str, status: reqwest::StatusCode) {
    push(Level::Info, "http", format!("relay doesn't serve {route} ({status}); reading it directly from now on • e.g. {example_url}"));
}

/// A request that never got a response (DNS, TLS, timeout, connection reset).
pub fn http_unreachable(url: &str, error: &dyn std::fmt::Display) {
    push(Level::Error, "http", format!("no response • {url} • {error}"));
}

/// Highest sequence number so far (the UI redraws when it changes).
pub fn latest_seq() -> u64 {
    SEQ.load(Ordering::Relaxed)
}

/// (errors, warnings) since start or the last clear.
pub fn counts() -> (u64, u64) {
    (ERRORS.load(Ordering::Relaxed), WARNINGS.load(Ordering::Relaxed))
}

pub fn snapshot() -> Vec<Entry> {
    store().lock().map(|s| s.entries.iter().cloned().collect()).unwrap_or_default()
}

pub fn clear() {
    if let Ok(mut store) = store().lock() {
        store.entries.clear();
    }
    ERRORS.store(0, Ordering::Relaxed);
    WARNINGS.store(0, Ordering::Relaxed);
}

/// Report text for sharing: app version, the newest warnings and errors, then recent activity.
pub fn report(entries: &[Entry]) -> String {
    let mut out = format!(
        "OpenWXBroadcasting v{} build {} • {}\n",
        crate::config::APP_VERSION,
        crate::config::APP_BUILD,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S %Z")
    );
    for e in entries {
        out.push_str(&e.line());
        out.push('\n');
    }
    out
}

#[derive(Default)]
struct TileFailures {
    since: Option<std::time::Instant>,
    total: u64,
    /// Status code (or "no response") to count.
    by_kind: std::collections::BTreeMap<String, u64>,
    hosts: std::collections::BTreeSet<String>,
}

fn tile_failures() -> &'static Mutex<TileFailures> {
    static T: OnceLock<Mutex<TileFailures>> = OnceLock::new();
    T.get_or_init(Default::default)
}

fn note_tile_failure(message: &str) {
    let kind = message
        .split("Status(")
        .nth(1)
        .and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "no response".to_string());
    let host = message
        .split("https://")
        .nth(1)
        .or_else(|| message.split("http://").nth(1))
        .and_then(|r| r.split(['/', '?', '"']).next())
        .unwrap_or("")
        .to_string();
    if let Ok(mut t) = tile_failures().lock() {
        t.since.get_or_insert_with(std::time::Instant::now);
        t.total += 1;
        *t.by_kind.entry(kind).or_default() += 1;
        if !host.is_empty() {
            t.hosts.insert(host);
        }
    }
}

/// Write the tile-failure summary once a minute (or now, with `force`).
pub fn flush_tile_failures(force: bool) {
    let summary = {
        let Ok(mut t) = tile_failures().lock() else { return };
        let due = t.since.map(|s| s.elapsed().as_secs() >= 60).unwrap_or(false);
        if t.total == 0 || !(due || force) {
            return;
        }
        let kinds = t
            .by_kind
            .iter()
            .map(|(k, n)| match k.as_str() {
                "429" => format!("{n} × 429 rate limited"),
                "502" | "503" | "504" => format!("{n} × {k} server busy"),
                _ => format!("{n} × {k}"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let hosts = t.hosts.iter().cloned().collect::<Vec<_>>().join(", ");
        let text = format!(
            "{} map tiles failed in the last {}s from {} ({kinds}). They are retried automatically; filter Everything + search \"raster tile\" for each one.",
            t.total,
            t.since.map(|s| s.elapsed().as_secs()).unwrap_or(0),
            if hosts.is_empty() { "tile servers".to_string() } else { hosts }
        );
        *t = TileFailures::default();
        text
    };
    push(Level::Warn, "map tiles", summary);
}

/// Logger that writes to env_logger's file and keeps a copy here.
struct Tee {
    inner: env_logger::Logger,
}

impl log::Log for Tee {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.inner.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        if !self.inner.matches(record) {
            return;
        }
        self.inner.log(record);
        let message = record.args().to_string();
        let target = record.target();
        // Map tiles fail in bursts of hundreds; keep each line (as detail) but count them into
        // one summary per minute instead of hundreds of warnings.
        if target.starts_with("maplibre::raster::request_system") && message.contains("failed") {
            note_tile_failure(&message);
            push(Level::Debug, target.to_string(), message);
        } else {
            push(Level::from_log(record.level()), target.to_string(), message);
        }
        flush_tile_failures(false);
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

/// Install the logger (call once, before anything logs).
pub fn install(mut builder: env_logger::Builder) {
    let inner = builder.build();
    let max = inner.filter();
    if log::set_boxed_logger(Box::new(Tee { inner })).is_ok() {
        log::set_max_level(max);
    }
}
