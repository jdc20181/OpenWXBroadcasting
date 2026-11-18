//! Emergency operations: the incident, shelters, evacuation zones, resources, road closures,
//! contacts and a timeline, kept in Library\EOC\eoc.json. Drawn on the map ("eoc" layer),
//! summarised in a briefing and exported as an operations report.

use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tiny_skia::{Color, FillRule, PathBuilder, Pixmap, Transform};

use crate::{
    config::{Brand, LibraryPaths, Settings},
    data::WxSnapshot,
    gfx::text::{GlyphCache, Weight},
    map::{
        geo::Viewport,
        overlays::{halo_text, paint, stroke, style},
    },
};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Incident {
    pub name: String,
    /// "monitoring", "partial", "full" (activation level).
    pub level: String,
    pub started: i64,
    pub notes: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Shelter {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub capacity: u32,
    pub occupancy: u32,
    /// "open", "full", "closed", "standby".
    pub status: String,
    pub pets: bool,
    pub phone: String,
    pub updated: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Zone {
    pub name: String,
    /// (lon, lat) rings.
    pub rings: Vec<Vec<(f64, f64)>>,
    /// "none", "advisory", "voluntary", "mandatory".
    pub status: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Resource {
    pub name: String,
    /// "staging", "sandbags", "generator", "ems", "fire", "police", "water", "other".
    pub kind: String,
    pub lat: f64,
    pub lon: f64,
    pub note: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Closure {
    pub road: String,
    pub lat: f64,
    pub lon: f64,
    pub reason: String,
    pub time: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Contact {
    pub name: String,
    pub role: String,
    pub agency: String,
    pub phone: String,
    pub email: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Entry {
    pub time: i64,
    /// "note", "warning", "shelter", "zone", "road", "resource".
    pub kind: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Eoc {
    pub incident: Incident,
    pub shelters: Vec<Shelter>,
    pub zones: Vec<Zone>,
    pub resources: Vec<Resource>,
    pub closures: Vec<Closure>,
    pub contacts: Vec<Contact>,
    pub timeline: Vec<Entry>,
    /// Warning / road event ids already in the timeline (not saved).
    #[serde(skip)]
    pub seen: Vec<String>,
}

fn path() -> std::path::PathBuf {
    LibraryPaths::get().root.join("EOC").join("eoc.json")
}

fn store() -> &'static Mutex<(u64, Eoc)> {
    static S: OnceLock<Mutex<(u64, Eoc)>> = OnceLock::new();
    S.get_or_init(|| {
        let eoc = std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Mutex::new((1, eoc))
    })
}

pub fn get() -> Eoc {
    store().lock().map(|s| s.1.clone()).unwrap_or_default()
}

/// Bumped on every change, so the app knows when to share the new state with the map.
pub fn version() -> u64 {
    store().lock().map(|s| s.0).unwrap_or(0)
}

/// Change the EOC state, add a timeline line when `log` is given, and save.
pub fn update(log: Option<(&str, String)>, f: impl FnOnce(&mut Eoc)) {
    if let Ok(mut s) = store().lock() {
        f(&mut s.1);
        if let Some((kind, text)) = log {
            s.1.timeline.push(Entry { time: chrono::Utc::now().timestamp(), kind: kind.into(), text });
        }
        s.0 += 1;
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(&s.1) {
            let _ = std::fs::write(p, json);
        }
    }
}

/// Timeline: warnings issued and roads closed in the coverage area, once each.
pub fn watch(warnings: &[crate::data::Alert], roads: &[crate::data::RoadEvent]) {
    let eoc = get();
    if eoc.incident.name.is_empty() {
        return;
    }
    // `seen` isn't saved, so after a restart the saved timeline text stops repeats.
    let known = |id: &str, text: &str| eoc.seen.iter().any(|s| s == id) || eoc.timeline.iter().any(|t| t.text == text);
    let mut new: Vec<(String, &str, String)> = Vec::new();
    for a in warnings {
        let text = format!("{} for {}", a.event, a.area_desc);
        if !known(&a.id, &text) {
            new.push((a.id.clone(), "warning", text));
        }
    }
    for r in roads.iter().filter(|r| r.kind == "closure") {
        let id = format!("road:{}:{:.3}:{:.3}", r.route, r.lat, r.lon);
        let text = format!("Road closed: {} {}", r.route, r.headline).trim().to_string();
        if !known(&id, &text) {
            new.push((id, "road", text));
        }
    }
    if new.is_empty() {
        return;
    }
    let now = chrono::Utc::now().timestamp();
    update(None, |e| {
        for (id, kind, text) in new {
            e.seen.push(id);
            e.timeline.push(Entry { time: now, kind: kind.into(), text });
        }
    });
}

/// Evacuation zones from a GeoJSON or shapefile; `name_field` names each zone (else "Zone N").
pub fn import_zones(file: &std::path::Path, name_field: &str) -> anyhow::Result<usize> {
    let ds = crate::customdata::load(file).ok_or_else(|| anyhow::anyhow!("Couldn't read {}", file.display()))?;
    let zones: Vec<Zone> = ds
        .features
        .iter()
        .filter(|f| !f.rings.is_empty())
        .enumerate()
        .map(|(i, f)| Zone {
            name: f.props.get(name_field).cloned().or_else(|| ["name", "NAME", "Name", "ZONE", "zone"].iter().find_map(|k| f.props.get(*k).cloned())).unwrap_or_else(|| format!("Zone {}", i + 1)),
            rings: f.rings.clone(),
            status: "none".into(),
        })
        .collect();
    let n = zones.len();
    if n == 0 {
        anyhow::bail!("No areas in that file");
    }
    update(Some(("zone", format!("{n} evacuation zones loaded from {}", file.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()))), |e| e.zones.extend(zones));
    Ok(n)
}

fn when(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0).map(|d| d.with_timezone(&chrono::Local).format("%b %-d %-I:%M %p").to_string()).unwrap_or_default()
}

/// Plain-text incident briefing: hazards in effect, forecast, shelters, zones, roads, resources
/// and the latest timeline.
pub fn briefing(data: &WxSnapshot, settings: &Settings) -> String {
    let e = get();
    let b = crate::gfx::bind::Bindings::build(data, settings, None, &settings.radar_palette);
    let v = |k: &str| b.get(k).unwrap_or("--").to_string();
    let mut out = String::new();
    let title = if e.incident.name.is_empty() { "Weather briefing".to_string() } else { e.incident.name.clone() };
    out += &format!("{title}\n{} • {}\n", settings.station_name, chrono::Local::now().format("%A %b %-d, %-I:%M %p"));
    if !e.incident.level.is_empty() {
        out += &format!("Activation: {}\n", e.incident.level);
    }
    out += "\nHAZARDS IN EFFECT\n";
    if data.local_alerts.is_empty() {
        out += "  None for the coverage area.\n";
    }
    for a in &data.local_alerts {
        out += &format!("  • {} until {} — {}\n", a.event, a.expires.as_deref().map(|x| chrono::DateTime::parse_from_rfc3339(x).map(|d| d.with_timezone(&chrono::Local).format("%-I:%M %p %a").to_string()).unwrap_or_else(|_| x.to_string())).unwrap_or_else(|| "further notice".into()), a.area_desc);
    }
    out += &format!("\nWEATHER\n  Now: {}, {}. Wind {}.\n", v("temp"), v("condition"), v("wind"));
    for d in 0..3 {
        out += &format!("  {}: {} High {} Low {}, rain chance {}\n", v(&format!("day{d}.name")), v(&format!("day{d}.condition")), v(&format!("day{d}.hi")), v(&format!("day{d}.lo")), v(&format!("day{d}.precip")));
    }
    let open: Vec<&Shelter> = e.shelters.iter().filter(|s| s.status == "open" || s.status == "full").collect();
    out += &format!("\nSHELTERS ({} open)\n", open.len());
    for s in &e.shelters {
        out += &format!("  • {} — {} ({} of {}{})\n", s.name, s.status, s.occupancy, s.capacity, if s.pets { ", pets OK" } else { "" });
    }
    let active: Vec<&Zone> = e.zones.iter().filter(|z| z.status != "none" && !z.status.is_empty()).collect();
    out += &format!("\nEVACUATION ZONES ({} with orders)\n", active.len());
    for z in &active {
        out += &format!("  • {}: {}\n", z.name, z.status);
    }
    let indot: Vec<&crate::data::RoadEvent> = data.roads.iter().filter(|r| r.kind == "closure").collect();
    out += &format!("\nROAD CLOSURES ({})\n", indot.len() + e.closures.len());
    for c in &e.closures {
        out += &format!("  • {} — {}\n", c.road, c.reason);
    }
    for r in indot.iter().take(15) {
        out += &format!("  • {} {}\n", r.route, r.headline);
    }
    if !e.resources.is_empty() {
        out += "\nRESOURCES\n";
        for r in &e.resources {
            out += &format!("  • {} ({}){}\n", r.name, r.kind, if r.note.is_empty() { String::new() } else { format!(" — {}", r.note) });
        }
    }
    out += "\nRECENT TIMELINE\n";
    for t in e.timeline.iter().rev().take(10) {
        out += &format!("  {} — {}\n", when(t.time), t.text);
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Operations report (HTML, opens in any browser and prints cleanly) in Exports.
pub fn export_report(data: &WxSnapshot, settings: &Settings) -> anyhow::Result<std::path::PathBuf> {
    let e = get();
    let mut html = String::from("<!doctype html><meta charset=\"utf-8\"><title>Operations report</title><style>body{font:14px system-ui;max-width:900px;margin:30px auto;padding:0 16px;color:#111}h1{margin-bottom:0}h2{border-bottom:1px solid #ccc;padding-bottom:4px;margin-top:28px}table{border-collapse:collapse;width:100%}td,th{border:1px solid #ddd;padding:5px 8px;text-align:left;font-size:13px}pre{white-space:pre-wrap;font:13px ui-monospace,monospace;background:#f5f5f5;padding:12px}</style>");
    html += &format!("<h1>{}</h1><p>{} • generated {}</p>", esc(if e.incident.name.is_empty() { "Operations report" } else { &e.incident.name }), esc(&settings.station_name), chrono::Local::now().format("%A %b %-d %Y, %-I:%M %p"));
    html += &format!("<h2>Briefing</h2><pre>{}</pre>", esc(&briefing(data, settings)));
    html += "<h2>Shelters</h2><table><tr><th>Name</th><th>Status</th><th>Occupancy</th><th>Pets</th><th>Phone</th><th>Updated</th></tr>";
    for s in &e.shelters {
        html += &format!("<tr><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td><td>{}</td><td>{}</td></tr>", esc(&s.name), esc(&s.status), s.occupancy, s.capacity, if s.pets { "yes" } else { "no" }, esc(&s.phone), when(s.updated));
    }
    html += "</table><h2>Contacts</h2><table><tr><th>Name</th><th>Role</th><th>Agency</th><th>Phone</th><th>Email</th></tr>";
    for c in &e.contacts {
        html += &format!("<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", esc(&c.name), esc(&c.role), esc(&c.agency), esc(&c.phone), esc(&c.email));
    }
    html += "</table><h2>Timeline</h2><table><tr><th>Time</th><th>Type</th><th>Entry</th></tr>";
    for t in &e.timeline {
        html += &format!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>", when(t.time), esc(&t.kind), esc(&t.text));
    }
    html += "</table>";
    let dir = LibraryPaths::get().exports;
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!("Operations report {}.html", chrono::Local::now().format("%Y-%m-%d %H%M")));
    std::fs::write(&file, html)?;
    Ok(file)
}

fn on_screen(vp: &Viewport, x: f32, y: f32, pad: f32) -> bool {
    x > -pad && y > -pad && x < vp.width as f32 + pad && y < vp.height as f32 + pad
}

/// Map layer: evacuation zones by order, shelters by status and fill, resources, closures
/// (manual and INDOT).
pub fn draw(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, eoc: &Eoc, roads: &[crate::data::RoadEvent], brand: &Brand) {
    let label = style(&brand.font_body, Weight::Bold, 17.0, Color::WHITE);
    let big = style(&brand.font_body, Weight::Bold, 20.0, Color::WHITE);
    for z in &eoc.zones {
        let (r, g, b, a) = match z.status.as_str() {
            "mandatory" => (220, 30, 30, 120),
            "voluntary" => (240, 140, 20, 105),
            "advisory" => (240, 210, 40, 80),
            _ => (150, 160, 180, 35),
        };
        let mut centre = None;
        for ring in &z.rings {
            let mut pb = PathBuilder::new();
            for (i, (lon, lat)) in ring.iter().enumerate() {
                let (x, y) = vp.to_screen(*lat, *lon);
                if i == 0 {
                    pb.move_to(x, y);
                } else {
                    pb.line_to(x, y);
                }
            }
            pb.close();
            if let Some(path) = pb.finish() {
                pm.fill_path(&path, &paint(Color::from_rgba8(r, g, b, a)), FillRule::EvenOdd, Transform::identity(), None);
                pm.stroke_path(&path, &paint(Color::from_rgba8(r, g, b, 255)), &stroke(2.0), Transform::identity(), None);
                if centre.is_none() {
                    let bb = path.bounds();
                    centre = Some(((bb.left() + bb.right()) / 2.0, (bb.top() + bb.bottom()) / 2.0));
                }
            }
        }
        if let Some((x, y)) = centre.filter(|(x, y)| on_screen(vp, *x, *y, 0.0)) {
            if z.status != "none" && !z.status.is_empty() {
                halo_text(pm, glyphs, &format!("{} • {}", z.name, z.status.to_uppercase()), x - 40.0, y, &big);
            } else if vp.view.zoom >= 9.0 {
                halo_text(pm, glyphs, &z.name, x - 20.0, y, &label);
            }
        }
    }
    // Road closures: INDOT and the EOC's own.
    let mut xs: Vec<(f64, f64, String)> = roads.iter().filter(|r| r.kind == "closure").map(|r| (r.lat, r.lon, r.route.clone())).collect();
    xs.extend(eoc.closures.iter().map(|c| (c.lat, c.lon, c.road.clone())));
    for (lat, lon, name) in xs {
        let (x, y) = vp.to_screen(lat, lon);
        if !on_screen(vp, x, y, 20.0) {
            continue;
        }
        let mut pb = PathBuilder::new();
        pb.push_circle(x, y, 11.0);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(200, 20, 20, 255)), FillRule::Winding, Transform::identity(), None);
        }
        let mut pb = PathBuilder::new();
        pb.move_to(x - 5.0, y - 5.0);
        pb.line_to(x + 5.0, y + 5.0);
        pb.move_to(x + 5.0, y - 5.0);
        pb.line_to(x - 5.0, y + 5.0);
        if let Some(path) = pb.finish() {
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.5), Transform::identity(), None);
        }
        if vp.view.zoom >= 9.0 && !name.is_empty() {
            halo_text(pm, glyphs, &name, x + 15.0, y + 6.0, &label);
        }
    }
    for r in &eoc.resources {
        let (x, y) = vp.to_screen(r.lat, r.lon);
        if !on_screen(vp, x, y, 20.0) {
            continue;
        }
        let (c, letter) = match r.kind.as_str() {
            "staging" => ((90, 120, 220), "S"),
            "sandbags" => ((170, 130, 70), "B"),
            "generator" => ((240, 200, 40), "G"),
            "ems" => ((230, 60, 60), "+"),
            "fire" => ((230, 90, 30), "F"),
            "police" => ((40, 80, 200), "P"),
            "water" => ((40, 160, 230), "W"),
            _ => ((140, 140, 150), "•"),
        };
        if let Some(rect) = tiny_skia::Rect::from_xywh(x - 10.0, y - 10.0, 20.0, 20.0) {
            pm.fill_rect(rect, &paint(Color::from_rgba8(c.0, c.1, c.2, 255)), Transform::identity(), None);
        }
        halo_text(pm, glyphs, letter, x - 5.0, y + 6.0, &label);
        if vp.view.zoom >= 9.0 {
            halo_text(pm, glyphs, &r.name, x + 14.0, y + 6.0, &label);
        }
    }
    for s in &eoc.shelters {
        let (x, y) = vp.to_screen(s.lat, s.lon);
        if !on_screen(vp, x, y, 30.0) {
            continue;
        }
        let fill = if s.capacity > 0 { s.occupancy as f32 / s.capacity as f32 } else { 0.0 };
        let c = match s.status.as_str() {
            "closed" | "standby" => (130, 135, 150),
            "full" => (220, 40, 40),
            _ if fill >= 0.85 => (240, 160, 30),
            _ => (40, 190, 100),
        };
        // House shape.
        let mut pb = PathBuilder::new();
        pb.move_to(x, y - 14.0);
        pb.line_to(x + 13.0, y - 3.0);
        pb.line_to(x + 10.0, y - 3.0);
        pb.line_to(x + 10.0, y + 11.0);
        pb.line_to(x - 10.0, y + 11.0);
        pb.line_to(x - 10.0, y - 3.0);
        pb.line_to(x - 13.0, y - 3.0);
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(Color::from_rgba8(c.0, c.1, c.2, 255)), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), Transform::identity(), None);
        }
        if vp.view.zoom >= 7.5 {
            let t = if s.capacity > 0 { format!("{} {}/{}", s.name, s.occupancy, s.capacity) } else { s.name.clone() };
            halo_text(pm, glyphs, &t, x + 17.0, y + 6.0, &label);
        }
    }
}
