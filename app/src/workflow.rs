//! Workflow automation: node graphs built in the visual designer (ui/workflows.rs). A trigger
//! (schedule, weather data, warning issued, broadcast state, or by hand) starts a run that walks
//! conditions (yes / no branches), waits and actions (with retries and an error branch). Runs are
//! recorded for replay and metrics; every save keeps the previous version.

use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};

use crate::{config::{LibraryPaths, Settings}, gfx::model::GraphicDoc, ops::Command};

pub const NODE_W: f32 = 210.0;
pub const NODE_H: f32 = 66.0;

/// (kind, op, label, argument hint)
pub const OPS: &[(&str, &str, &str, &str)] = &[
    ("trigger", "manual", "By hand", ""),
    ("trigger", "schedule", "Schedule", "every 30  |  at 06:00  |  at 06:00 weekdays"),
    ("trigger", "data", "Weather data", "temp < 32  |  wind_gust >= 45  |  alert.count > 0"),
    ("trigger", "warning", "Warning issued", "Tornado Warning (empty = any warning)"),
    ("trigger", "broadcast", "Broadcast state", "show_start, show_stop, severe_on, severe_off, graphic_on, live_map"),
    ("condition", "data", "If weather data", "temp < 32"),
    ("condition", "warning_active", "If warning in effect", "Tornado (empty = any)"),
    ("condition", "time", "If time between", "06:00-22:00"),
    ("condition", "on_air", "If show running", ""),
    ("condition", "severe", "If severe mode on", ""),
    ("wait", "wait", "Wait", "seconds, e.g. 30"),
    ("action", "graphic", "Take graphic", "graphic name"),
    ("action", "scene", "Take scene", "scene number"),
    ("action", "next", "Next scene", ""),
    ("action", "live", "Live map", ""),
    ("action", "severe", "Severe mode", "severe, tornado, flood, off"),
    ("action", "caption", "Caption", "text ({{bindings}} allowed)"),
    ("action", "channel", "Channel take", "2:Graphic name"),
    ("action", "state", "Graphic state", "state:Name, toggle:Layer, pause, play"),
    ("action", "audio", "Play audio", "file in Media"),
    ("action", "backup", "Backup scene", "on / off"),
    ("action", "refresh", "Refresh data", ""),
    ("action", "publish", "Publish graphic", "graphic name (all targets)"),
    ("action", "notify", "Notify staff", "message ({{bindings}} allowed)"),
];

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Node {
    pub id: u32,
    pub kind: String,
    pub op: String,
    pub arg: String,
    pub x: f32,
    pub y: f32,
    /// Yes / success path.
    pub next: Option<u32>,
    /// No (conditions) or error (actions) path.
    pub alt: Option<u32>,
    /// Actions: retries before the error path, 5 s apart.
    pub retries: u32,
}

impl Node {
    pub fn label(&self) -> String {
        let name = OPS.iter().find(|(k, o, _, _)| *k == self.kind && *o == self.op).map(|(_, _, l, _)| *l).unwrap_or("?");
        if self.arg.is_empty() { name.to_string() } else { format!("{name}: {}", self.arg) }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub version: u32,
    pub updated: i64,
    pub nodes: Vec<Node>,
}

impl Workflow {
    pub fn node(&self, id: u32) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn next_id(&self) -> u32 {
        self.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub node: u32,
    pub label: String,
    /// "yes", "no", "done", "error", "retry", "waiting", "dry run".
    pub result: String,
    pub detail: String,
    pub ms: f32,
    pub time: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub id: u64,
    pub workflow: String,
    pub name: String,
    pub version: u32,
    pub trigger: String,
    pub started: i64,
    pub finished: Option<i64>,
    /// "running", "done", "failed", "dry run".
    pub status: String,
    pub steps: Vec<Step>,
    pub dry: bool,
    #[serde(skip)]
    cursor: Option<u32>,
    #[serde(skip)]
    wake: i64,
    #[serde(skip)]
    attempts: u32,
}

/// What the engine can see each tick.
#[derive(Clone, Debug, Default)]
pub struct Ctx {
    /// Every text binding (numbers are read from the start of the text).
    pub values: BTreeMap<String, String>,
    /// Warnings in coverage: (id, event).
    pub warnings: Vec<(String, String)>,
    pub show_running: bool,
    pub severe: bool,
    pub graphic_on_air: bool,
}

impl Ctx {
    fn number(&self, key: &str) -> Option<f64> {
        let v = self.values.get(key)?;
        let t: String = v.trim().chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
        t.parse().ok()
    }

    /// "key op number": op is <, <=, >, >=, =, ==, !=.
    pub fn test(&self, expr: &str) -> Result<bool, String> {
        let ops = ["<=", ">=", "!=", "==", "<", ">", "="];
        let (key, op, rhs) = ops
            .iter()
            .find_map(|op| expr.split_once(op).map(|(a, b)| (a.trim(), *op, b.trim())))
            .ok_or_else(|| format!("Can't read \"{expr}\" (use: name < number)"))?;
        let want: f64 = rhs.trim_end_matches(['°', '%', '"']).parse().map_err(|_| format!("\"{rhs}\" isn't a number"))?;
        let have = self.number(key).ok_or_else(|| format!("No number for {key} right now"))?;
        Ok(match op {
            "<" => have < want,
            "<=" => have <= want,
            ">" => have > want,
            ">=" => have >= want,
            "!=" => (have - want).abs() > 1e-9,
            _ => (have - want).abs() < 1e-9,
        })
    }

    fn apply(&self, text: &str) -> String {
        let mut out = text.to_string();
        for (k, v) in &self.values {
            let token = format!("{{{{{k}}}}}");
            if out.contains(&token) {
                out = out.replace(&token, v);
            }
        }
        out
    }
}

#[derive(Clone, Debug, Default)]
pub struct Metric {
    pub runs: u32,
    pub failures: u32,
    pub total_ms: f32,
    pub last: i64,
    /// node id -> (count, total ms)
    pub nodes: HashMap<u32, (u32, f32)>,
}

#[derive(Default)]
struct Engine {
    loaded: bool,
    workflows: Vec<Workflow>,
    runs: Vec<Run>,
    history: Vec<Run>,
    next_run: u64,
    /// (workflow, node) -> last truth (data triggers fire on false -> true).
    truth: HashMap<(String, u32), bool>,
    /// (workflow, node) -> last fire (schedules).
    fired: HashMap<(String, u32), i64>,
    seen_warnings: Vec<String>,
    last_state: Option<(bool, bool, bool)>,
    metrics: HashMap<String, Metric>,
}

fn engine() -> &'static Mutex<Engine> {
    static E: OnceLock<Mutex<Engine>> = OnceLock::new();
    E.get_or_init(Default::default)
}

fn dir() -> PathBuf {
    LibraryPaths::get().root.join("Workflows")
}

fn ensure_loaded(e: &mut Engine) {
    if e.loaded {
        return;
    }
    e.loaded = true;
    e.next_run = 1;
    for entry in std::fs::read_dir(dir()).into_iter().flatten().flatten() {
        let p = entry.path();
        if p.extension().map(|x| x == "json").unwrap_or(false) {
            if let Some(w) = std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str::<Workflow>(&t).ok()) {
                e.workflows.push(w);
            }
        }
    }
    e.workflows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
}

pub fn list() -> Vec<Workflow> {
    engine().lock().map(|mut e| {
        ensure_loaded(&mut e);
        e.workflows.iter().filter(|w| !w.id.starts_with("replay-")).cloned().collect()
    }).unwrap_or_default()
}

/// Save (new version). The previous version is kept in Workflows\history.
pub fn save(mut wf: Workflow) -> anyhow::Result<Workflow> {
    let d = dir();
    std::fs::create_dir_all(d.join("history"))?;
    if wf.id.is_empty() {
        wf.id = format!("wf{}", chrono::Utc::now().timestamp_millis());
    }
    let path = d.join(format!("{}.json", wf.id));
    if let Some(old) = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Workflow>(&t).ok()) {
        if old.nodes == wf.nodes && old.name == wf.name && old.enabled == wf.enabled {
            return Ok(old);
        }
        std::fs::write(d.join("history").join(format!("{}-v{}.json", old.id, old.version)), serde_json::to_string_pretty(&old)?)?;
        wf.version = old.version + 1;
    } else {
        wf.version = wf.version.max(1);
    }
    wf.updated = chrono::Utc::now().timestamp();
    std::fs::write(&path, serde_json::to_string_pretty(&wf)?)?;
    if let Ok(mut e) = engine().lock() {
        ensure_loaded(&mut e);
        e.workflows.retain(|w| w.id != wf.id);
        e.workflows.push(wf.clone());
        e.workflows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }
    Ok(wf)
}

pub fn delete(id: &str) {
    let _ = std::fs::remove_file(dir().join(format!("{id}.json")));
    if let Ok(mut e) = engine().lock() {
        e.workflows.retain(|w| w.id != id);
    }
}

/// Saved versions: (version, saved time).
pub fn versions(id: &str) -> Vec<(u32, i64)> {
    let mut out: Vec<(u32, i64)> = std::fs::read_dir(dir().join("history"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let v = name.strip_prefix(&format!("{id}-v"))?.strip_suffix(".json")?.parse().ok()?;
            let w: Workflow = serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            Some((v, w.updated))
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out
}

/// Restore an older version (saved as a new version, so nothing is lost).
pub fn restore(id: &str, version: u32) -> anyhow::Result<Workflow> {
    let old: Workflow = serde_json::from_str(&std::fs::read_to_string(dir().join("history").join(format!("{id}-v{version}.json")))?)?;
    let current = list().into_iter().find(|w| w.id == id);
    let mut wf = old;
    wf.version = current.map(|c| c.version).unwrap_or(wf.version);
    save(wf)
}

/// Anything to evaluate: an enabled workflow or a run in progress.
pub fn busy() -> bool {
    engine().lock().map(|mut e| {
        ensure_loaded(&mut e);
        !e.runs.is_empty() || e.workflows.iter().any(|w| w.enabled)
    }).unwrap_or(false)
}

pub fn runs() -> Vec<Run> {
    engine().lock().map(|e| e.runs.iter().chain(e.history.iter().rev()).take(80).cloned().collect()).unwrap_or_default()
}

pub fn metrics(id: &str) -> Metric {
    engine().lock().ok().and_then(|e| e.metrics.get(id).cloned()).unwrap_or_default()
}

fn start(e: &mut Engine, wf: &Workflow, from: u32, trigger: String, dry: bool) {
    let id = e.next_run;
    e.next_run += 1;
    let first = wf.node(from).and_then(|n| if n.kind == "trigger" { n.next } else { Some(n.id) });
    e.runs.push(Run {
        id,
        workflow: wf.id.clone(),
        name: wf.name.clone(),
        version: wf.version,
        trigger,
        started: chrono::Utc::now().timestamp(),
        finished: None,
        status: if dry { "dry run".into() } else { "running".into() },
        steps: Vec::new(),
        dry,
        cursor: first,
        wake: 0,
        attempts: 0,
    });
}

/// Start a workflow by hand (from its first trigger), or a dry run that only evaluates.
pub fn run_now(id: &str, dry: bool) {
    if let Ok(mut e) = engine().lock() {
        ensure_loaded(&mut e);
        if let Some(wf) = e.workflows.iter().find(|w| w.id == id).cloned() {
            let from = wf.nodes.iter().find(|n| n.kind == "trigger").map(|n| n.id).unwrap_or_else(|| wf.nodes.first().map(|n| n.id).unwrap_or(0));
            start(&mut e, &wf, from, if dry { "dry run".into() } else { "by hand".into() }, dry);
        }
    }
}

/// Replay a recorded run: the same actions, in the same order, with the same arguments.
pub fn replay(run_id: u64) {
    if let Ok(mut e) = engine().lock() {
        let Some(run) = e.history.iter().find(|r| r.id == run_id).cloned() else { return };
        let Some(wf) = e.workflows.iter().find(|w| w.id == run.workflow).cloned() else { return };
        // A one-off workflow made from the recorded action steps.
        let mut nodes: Vec<Node> = run
            .steps
            .iter()
            .filter(|s| s.result == "done")
            .filter_map(|s| wf.node(s.node).cloned())
            .filter(|n| n.kind == "action" || n.kind == "wait")
            .collect();
        for (i, n) in nodes.iter_mut().enumerate() {
            n.id = i as u32 + 1;
            n.next = Some(i as u32 + 2);
            n.alt = None;
        }
        if let Some(last) = nodes.last_mut() {
            last.next = None;
        }
        let copy = Workflow { id: wf.id.clone(), name: format!("{} (replay of run {run_id})", wf.name), version: run.version, nodes, ..Default::default() };
        start(&mut e, &copy, 1, format!("replay of run {run_id}"), false);
        // Remember the replay graph for stepping.
        if let Some(r) = e.runs.last_mut() {
            r.cursor = Some(1);
        }
        e.workflows.push(Workflow { id: format!("replay-{run_id}"), ..copy });
        if let Some(r) = e.runs.last_mut() {
            r.workflow = format!("replay-{run_id}");
        }
    }
}

fn time_between(spec: &str) -> Result<bool, String> {
    let (a, b) = spec.split_once('-').ok_or("use 06:00-22:00")?;
    let p = |s: &str| chrono::NaiveTime::parse_from_str(s.trim(), "%H:%M").map_err(|_| format!("\"{s}\" isn't a time"));
    let (a, b, now) = (p(a)?, p(b)?, chrono::Local::now().time());
    Ok(if a <= b { now >= a && now < b } else { now >= a || now < b })
}

fn schedule_due(spec: &str, last: i64) -> bool {
    let now = chrono::Local::now();
    let ts = now.timestamp();
    let spec = spec.trim().to_lowercase();
    if let Some(m) = spec.strip_prefix("every") {
        let mins: i64 = m.trim().trim_end_matches("min").trim().parse().unwrap_or(0);
        return mins > 0 && ts - last >= mins * 60;
    }
    if let Some(rest) = spec.strip_prefix("at") {
        let mut parts = rest.split_whitespace();
        let Some(t) = parts.next().and_then(|t| chrono::NaiveTime::parse_from_str(t, "%H:%M").ok()) else { return false };
        use chrono::Datelike;
        if parts.next() == Some("weekdays") && now.weekday().number_from_monday() > 5 {
            return false;
        }
        let today = now.date_naive().and_time(t);
        let at = today.and_local_timezone(chrono::Local).single().map(|d| d.timestamp()).unwrap_or(i64::MAX);
        return ts >= at && last < at;
    }
    false
}

/// Evaluate triggers and advance runs. Returns the commands for the app to carry out.
pub fn tick(ctx: &Ctx, graphics: &[GraphicDoc], settings: &Settings) -> Vec<Command> {
    let Ok(mut e) = engine().lock() else { return Vec::new() };
    ensure_loaded(&mut e);
    let now = chrono::Utc::now().timestamp();

    // Broadcast state edges.
    let state = (ctx.show_running, ctx.severe, ctx.graphic_on_air);
    let mut events: Vec<&str> = Vec::new();
    if let Some(prev) = e.last_state {
        if state.0 != prev.0 {
            events.push(if state.0 { "show_start" } else { "show_stop" });
        }
        if state.1 != prev.1 {
            events.push(if state.1 { "severe_on" } else { "severe_off" });
        }
        if state.2 != prev.2 {
            events.push(if state.2 { "graphic_on" } else { "live_map" });
        }
    }
    e.last_state = Some(state);
    let new_warnings: Vec<(String, String)> = ctx.warnings.iter().filter(|(id, _)| !e.seen_warnings.contains(id)).cloned().collect();
    let first_pass = e.seen_warnings.is_empty() && e.history.is_empty() && e.runs.is_empty() && e.truth.is_empty();
    e.seen_warnings.extend(new_warnings.iter().map(|(id, _)| id.clone()));

    // Triggers.
    let wfs: Vec<Workflow> = e.workflows.iter().filter(|w| w.enabled).cloned().collect();
    for wf in &wfs {
        for n in wf.nodes.iter().filter(|n| n.kind == "trigger") {
            let key = (wf.id.clone(), n.id);
            let fire: Option<String> = match n.op.as_str() {
                "schedule" => {
                    let last = *e.fired.get(&key).unwrap_or(&0);
                    let last = if last == 0 { now } else { last };
                    if !e.fired.contains_key(&key) {
                        e.fired.insert(key.clone(), now);
                    }
                    schedule_due(&n.arg, last).then(|| format!("schedule {}", n.arg))
                }
                "data" => {
                    let truth = ctx.test(&n.arg).unwrap_or(false);
                    let was = e.truth.insert(key.clone(), truth).unwrap_or(true);
                    (truth && !was).then(|| format!("data: {}", n.arg))
                }
                "warning" => (!first_pass)
                    .then(|| new_warnings.iter().find(|(_, ev)| n.arg.trim().is_empty() || ev.to_lowercase().contains(&n.arg.trim().to_lowercase())).map(|(_, ev)| format!("warning: {ev}")))
                    .flatten(),
                "broadcast" => events.iter().find(|ev| n.arg.split(',').any(|a| a.trim() == **ev)).map(|ev| format!("broadcast: {ev}")),
                _ => None,
            };
            if let Some(t) = fire {
                e.fired.insert(key, now);
                start(&mut e, wf, n.id, t, false);
            }
        }
    }

    // Advance runs (up to 20 steps each per tick; waits pause until their time).
    let mut commands = Vec::new();
    let mut runs = std::mem::take(&mut e.runs);
    for run in runs.iter_mut() {
        let Some(wf) = e.workflows.iter().find(|w| w.id == run.workflow).cloned() else {
            run.status = "failed".into();
            run.cursor = None;
            continue;
        };
        let mut budget = 20;
        while let Some(cur) = run.cursor {
            if run.wake > now || budget == 0 {
                break;
            }
            budget -= 1;
            let Some(n) = wf.node(cur).cloned() else {
                run.cursor = None;
                break;
            };
            let t0 = std::time::Instant::now();
            let (result, detail, next): (&str, String, Option<u32>) = match n.kind.as_str() {
                "condition" => {
                    let r = match n.op.as_str() {
                        "data" => ctx.test(&n.arg),
                        "warning_active" => Ok(ctx.warnings.iter().any(|(_, ev)| n.arg.trim().is_empty() || ev.to_lowercase().contains(&n.arg.trim().to_lowercase()))),
                        "time" => time_between(&n.arg),
                        "on_air" => Ok(ctx.show_running),
                        "severe" => Ok(ctx.severe),
                        _ => Err("unknown condition".into()),
                    };
                    match r {
                        Ok(true) => ("yes", String::new(), n.next),
                        Ok(false) => ("no", String::new(), n.alt),
                        Err(err) => ("error", err, n.alt),
                    }
                }
                "wait" => {
                    if run.wake == 0 {
                        run.wake = now + n.arg.trim().trim_end_matches('s').parse::<i64>().unwrap_or(10).clamp(1, 86_400);
                        run.steps.push(Step { node: n.id, label: n.label(), result: "waiting".into(), detail: String::new(), ms: 0.0, time: now });
                        continue;
                    }
                    run.wake = 0;
                    ("done", String::new(), n.next)
                }
                "action" if run.dry => ("dry run", ctx.apply(&n.arg), n.next),
                "action" => match action(&n, ctx, graphics, settings) {
                    Ok(cmd) => {
                        commands.extend(cmd);
                        run.attempts = 0;
                        ("done", ctx.apply(&n.arg), n.next)
                    }
                    Err(err) if run.attempts < n.retries => {
                        run.attempts += 1;
                        run.wake = now + 5;
                        run.steps.push(Step { node: n.id, label: n.label(), result: "retry".into(), detail: err, ms: 0.0, time: now });
                        // Re-run this node after the wait.
                        continue;
                    }
                    Err(err) => {
                        run.attempts = 0;
                        ("error", err, n.alt)
                    }
                },
                _ => ("done", String::new(), n.next),
            };
            let ms = t0.elapsed().as_secs_f32() * 1000.0;
            if result == "error" && next.is_none() {
                run.status = "failed".into();
                crate::broadcast_ops::notify("Workflow failed", &format!("{}: {} — {detail}", wf.name, n.label()), &settings.notify_webhook);
            }
            run.steps.push(Step { node: n.id, label: n.label(), result: result.into(), detail, ms, time: now });
            run.cursor = next;
        }
        if run.cursor.is_none() && run.finished.is_none() {
            run.finished = Some(now);
            if run.status == "running" {
                run.status = "done".into();
            }
        }
    }
    // Finished runs go to history and metrics.
    let (done, active): (Vec<Run>, Vec<Run>) = runs.into_iter().partition(|r| r.finished.is_some());
    e.runs = active;
    for r in done {
        if !r.dry {
            let m = e.metrics.entry(r.workflow.clone()).or_default();
            m.runs += 1;
            if r.status == "failed" {
                m.failures += 1;
            }
            let total: f32 = r.steps.iter().map(|s| s.ms).sum::<f32>() + ((r.finished.unwrap_or(r.started) - r.started) as f32 * 1000.0);
            m.total_ms += total;
            m.last = r.started;
            for s in &r.steps {
                let entry = m.nodes.entry(s.node).or_default();
                entry.0 += 1;
                entry.1 += s.ms;
            }
            // Run log for audits.
            let log = dir().join("runs.jsonl");
            if let Ok(line) = serde_json::to_string(&r) {
                use std::io::Write;
                let _ = std::fs::create_dir_all(dir());
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
                    let _ = writeln!(f, "{line}");
                }
            }
        }
        e.history.push(r);
    }
    if e.history.len() > 300 {
        e.history.drain(..100);
    }
    let active: std::collections::HashSet<_> = e.runs.iter().map(|r| r.workflow.clone()).collect();
    e.workflows.retain(|w| !w.id.starts_with("replay-") || active.contains(&w.id));
    commands
}

fn action(n: &Node, ctx: &Ctx, graphics: &[GraphicDoc], settings: &Settings) -> Result<Vec<Command>, String> {
    let arg = ctx.apply(n.arg.trim());
    let find = |name: &str| graphics.iter().find(|g| g.name.eq_ignore_ascii_case(name.trim()) || g.id == name.trim()).cloned();
    Ok(match n.op.as_str() {
        "graphic" => {
            find(&arg).ok_or_else(|| format!("No graphic named \"{arg}\""))?;
            vec![Command::Graphic(arg)]
        }
        "scene" => vec![Command::TakeScene(arg.parse::<usize>().map_err(|_| "scene number?".to_string())?.max(1) - 1)],
        "next" => vec![Command::Next],
        "live" => vec![Command::Live],
        "severe" => vec![Command::Severe(arg)],
        "caption" => vec![Command::Caption(arg)],
        "channel" => {
            let (ch, name) = arg.split_once(':').ok_or("use 2:Graphic name")?;
            let ch: usize = ch.trim().parse().map_err(|_| "channel number?".to_string())?;
            find(name).ok_or_else(|| format!("No graphic named \"{}\"", name.trim()))?;
            vec![Command::ChannelTake(ch, name.trim().to_string())]
        }
        "state" => vec![Command::Interact(arg)],
        "audio" => {
            if !LibraryPaths::get().media.join(&arg).exists() {
                return Err(format!("{arg} isn't in Media"));
            }
            vec![Command::PlayAudio(arg)]
        }
        "backup" => vec![Command::Backup(arg.eq_ignore_ascii_case("on"))],
        "refresh" => vec![Command::Refresh],
        "publish" => {
            let doc = find(&arg).ok_or_else(|| format!("No graphic named \"{arg}\""))?;
            let targets: Vec<String> = settings.publish_targets.iter().map(|t| t.name.clone()).collect();
            if targets.is_empty() {
                return Err("No publishing targets".into());
            }
            crate::publish::enqueue(&doc, settings, &targets).map_err(|e| e.to_string())?;
            Vec::new()
        }
        "notify" => {
            if settings.notify_webhook.trim().is_empty() {
                return Err("No notification webhook (Settings > Weather & maps > Warning desk)".into());
            }
            crate::broadcast_ops::notify("Workflow", &arg, &settings.notify_webhook);
            Vec::new()
        }
        other => return Err(format!("Unknown action {other}")),
    })
}
