//! Power outages from the utilities' public outage maps. Most utilities publish through Kubra
//! StormCenter (AES Indiana, Duke Energy, …): paste the outage map's address in Settings and the
//! county summaries are read from it. A plain JSON feed ({"areas":[{"name","out","served","etr"}]})
//! works too. Counts are kept per county and utility with a history for the trend chart and
//! restoration progress; critical facilities (hospitals, water plants, fire / EMS) come from
//! OpenStreetMap for the infrastructure disruption map.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tiny_skia::{Color, FillRule, PathBuilder, Pixmap, Transform};

use crate::{
    config::{Brand, LibraryPaths},
    gfx::text::{GlyphCache, Weight},
    map::{
        geo::Viewport,
        overlays::{halo_text, paint, stroke, style},
    },
};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Source {
    /// Utility name shown on air.
    pub name: String,
    /// Kubra StormCenter map address (…/stormcenter/…/views/…) or a JSON feed URL.
    pub url: String,
}

/// One-click outage feeds that were checked to work (name, address). Duke Energy's map does not
/// expose a public feed, and NIPSCO / CenterPoint load theirs by script, so they still need a
/// feed address pasted when one is found.
pub const PRESETS: &[(&str, &str)] = &[
    ("Indiana Michigan Power", "https://kubra.io/stormcenter/e07a4ffb-83dd-4251-8dad-3fe5a26979b6/views/1b319a36-d1cf-443d-9ecf-6267acae0c3a"),
    ("AES Indiana", "https://myaccount.aesindiana.com/OMSDATA/OMSDATA_OSI.xml"),
];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CountyOutage {
    pub name: String,
    pub out: u32,
    pub served: u32,
    /// Estimated restoration (text from the utility), if any.
    pub etr: String,
    pub utilities: Vec<(String, u32)>,
    /// Highest count in this event (restoration progress).
    pub peak: u32,
}

impl CountyOutage {
    pub fn pct(&self) -> f32 {
        if self.served == 0 { 0.0 } else { self.out as f32 * 100.0 / self.served as f32 }
    }
    pub fn restored_pct(&self) -> f32 {
        if self.peak == 0 { 100.0 } else { (1.0 - self.out as f32 / self.peak as f32) * 100.0 }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facility {
    pub name: String,
    /// "hospital", "water", "wastewater", "fire", "ems", "nursing".
    pub kind: String,
    pub lat: f64,
    pub lon: f64,
    pub county: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outages {
    pub counties: Vec<CountyOutage>,
    /// (utility, out, served)
    pub utilities: Vec<(String, u32, u32)>,
    pub total_out: u32,
    pub total_served: u32,
    /// (unix time, customers out), last 72 hours.
    pub history: Vec<(i64, u32)>,
    pub peak: u32,
    pub facilities: Vec<Facility>,
    pub updated: i64,
    pub problems: Vec<String>,
}

impl Outages {
    pub fn restored_pct(&self) -> f32 {
        if self.peak == 0 { 100.0 } else { (1.0 - self.total_out as f32 / self.peak as f32) * 100.0 }
    }
    /// Change over the last hour (customers).
    pub fn trend(&self) -> i64 {
        let now = chrono::Utc::now().timestamp();
        let past = self.history.iter().rev().find(|(t, _)| now - t >= 3600).map(|(_, n)| *n as i64);
        past.map(|p| self.total_out as i64 - p).unwrap_or(0)
    }
    pub fn county(&self, name: &str) -> Option<&CountyOutage> {
        self.counties.iter().find(|c| c.name.eq_ignore_ascii_case(name))
    }
}

/// Read a number from Kubra's {"val": n} or a plain number.
fn num(v: Option<&serde_json::Value>) -> u32 {
    match v {
        Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(0.0) as u32,
        Some(serde_json::Value::Object(o)) => o.get("val").and_then(|x| x.as_f64()).unwrap_or(0.0) as u32,
        Some(serde_json::Value::String(s)) => s.replace(',', "").parse().unwrap_or(0),
        _ => 0,
    }
}

/// Every object with a name and an outage count, anywhere in the document.
fn areas(v: &serde_json::Value, out: &mut Vec<(String, u32, u32, String)>) {
    match v {
        serde_json::Value::Object(o) => {
            let name = o.get("name").or_else(|| o.get("area_name")).or_else(|| o.get("county")).and_then(|n| n.as_str().map(str::to_string).or_else(|| n.get("val").and_then(|x| x.as_str()).map(str::to_string)));
            let out_n = o.get("cust_a").or_else(|| o.get("out")).or_else(|| o.get("customers_out"));
            if let (Some(name), Some(_)) = (name, out_n) {
                let served = num(o.get("cust_s").or_else(|| o.get("served")).or_else(|| o.get("customers_served")));
                let etr = o.get("etr").and_then(|e| e.as_str()).filter(|e| !e.contains("NULL") && !e.is_empty()).unwrap_or("").to_string();
                out.push((name, num(out_n), served, etr));
                return;
            }
            for x in o.values() {
                areas(x, out);
            }
        }
        serde_json::Value::Array(a) => {
            for x in a {
                areas(x, out);
            }
        }
        _ => {}
    }
}

/// Kubra: map address -> instance / view -> current data path -> summary files.
async fn kubra(url: &str) -> Result<Vec<(String, u32, u32, String)>> {
    let (instance, view) = {
        let parts: Vec<&str> = url.split('/').collect();
        let after = |key: &str| parts.iter().position(|p| *p == key).and_then(|i| parts.get(i + 1)).map(|s| s.split(['?', '#']).next().unwrap_or("").to_string());
        (after("stormcenter").or_else(|| after("stormcenters")).filter(|s| s != "api").ok_or_else(|| anyhow!("Kubra address needs …/stormcenter/<id>/…"))?, after("views").ok_or_else(|| anyhow!("Kubra address needs …/views/<id>"))?)
    };
    let state = crate::net::get_json(&format!("https://kubra.io/stormcenter/api/v1/stormcenters/{instance}/views/{view}/currentState?preview=false")).await?;
    let path = state
        .pointer("/data/interval_generation_data")
        .and_then(|p| p.as_str())
        .ok_or_else(|| anyhow!("Kubra: no current data"))?
        .to_string();
    let mut rows = Vec::new();
    let mut totals: Option<(u32, u32)> = None;
    // Summary files hold the totals and the county / region lists.
    for n in 1..=4 {
        if let Ok(v) = crate::net::get_json(&format!("https://kubra.io/{path}/public/summary-{n}/data.json")).await {
            let mut found = Vec::new();
            if let Some(list) = v.pointer("/summaryFileData/areas") {
                areas(list, &mut found);
            }
            if totals.is_none() {
                let t = v.pointer("/summaryFileData/totals/0");
                let out = t.and_then(|t| t.pointer("/total_cust_a/val")).and_then(|x| x.as_u64());
                let served = t.and_then(|t| t.get("total_cust_s")).and_then(|x| x.as_u64());
                if let (Some(out), Some(served)) = (out, served) {
                    totals = Some((out as u32, served as u32));
                }
            }
            // Prefer the most detailed list (counties) over state / service-area totals.
            if found.len() > rows.len() {
                rows = found;
            }
        }
    }
    if rows.is_empty() {
        // Quiet weather ("blue sky" mode): utilities publish only their totals, no county list.
        return match totals {
            Some((out, served)) => Ok(vec![("Service area".to_string(), out, served, String::new())]),
            None => Err(anyhow!("Kubra: no area summaries")),
        };
    }
    Ok(rows)
}

/// Outage incidents as XML (AES Indiana's OMSDATA feed): each <Marker> has Lat, Long,
/// CustAffected, Etr and outageType. Unplanned outages are totalled per county.
async fn oms_xml(url: &str) -> Result<Vec<(String, u32, u32, String)>> {
    let xml = String::from_utf8_lossy(&crate::net::get_bytes(url).await?).into_owned();
    let counties = crate::map::boundaries::counties_named().ok_or_else(|| anyhow!("county outlines are still downloading"))?;
    Ok(oms_rows(&xml, |lat, lon| crate::map::boundaries::county_at(&counties, lat, lon)))
}

fn oms_rows(xml: &str, county: impl Fn(f64, f64) -> Option<String>) -> Vec<(String, u32, u32, String)> {
    let tag = |s: &str, t: &str| -> Option<String> {
        let a = s.find(&format!("<{t}>"))? + t.len() + 2;
        let b = s[a..].find(&format!("</{t}>"))? + a;
        Some(s[a..b].trim().to_string())
    };
    let mut by: BTreeMap<String, (u32, String)> = BTreeMap::new();
    for m in xml.split("<Marker>").skip(1) {
        if tag(m, "outageType").is_some_and(|t| t.to_uppercase().contains("PLANNED")) {
            continue;
        }
        let (Some(lat), Some(lon)) = (tag(m, "Lat").and_then(|v| v.parse::<f64>().ok()), tag(m, "Long").and_then(|v| v.parse::<f64>().ok())) else { continue };
        let out = tag(m, "CustAffected").and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let name = county(lat, lon).unwrap_or_else(|| "Other".into());
        let entry = by.entry(name).or_insert((0, String::new()));
        entry.0 += out;
        if let Some(etr) = tag(m, "Etr").filter(|e| !e.is_empty()) {
            entry.1 = etr;
        }
    }
    by.into_iter().map(|(name, (out, etr))| (name, out, 0, etr)).collect()
}

async fn plain(url: &str) -> Result<Vec<(String, u32, u32, String)>> {
    let v = crate::net::get_json(url).await?;
    let mut rows = Vec::new();
    areas(&v, &mut rows);
    Ok(rows)
}

fn clean_county(name: &str) -> String {
    name.trim().trim_end_matches(" County").trim_end_matches(" COUNTY").trim().to_string()
}

fn history_path() -> std::path::PathBuf {
    LibraryPaths::get().root.join("Outages").join("history.json")
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    history: Vec<(i64, u32)>,
    peak: u32,
    county_peak: HashMap<String, u32>,
}

/// Fetch every configured utility and combine by county.
pub async fn fetch(sources: Vec<Source>, previous_facilities: Vec<Facility>, bbox: (f64, f64, f64, f64)) -> Outages {
    let mut by_county: BTreeMap<String, CountyOutage> = BTreeMap::new();
    let mut utilities = Vec::new();
    let mut problems = Vec::new();
    for s in &sources {
        let rows = if s.url.contains("kubra") || s.url.contains("stormcenter") {
            kubra(&s.url).await
        } else if s.url.contains("OMSDATA") || s.url.to_lowercase().ends_with(".xml") {
            oms_xml(&s.url).await
        } else {
            plain(&s.url).await
        };
        match rows {
            Ok(rows) => {
                let (mut o, mut sv) = (0, 0);
                for (name, out, served, etr) in rows {
                    let c = by_county.entry(clean_county(&name).to_lowercase()).or_insert_with(|| CountyOutage { name: clean_county(&name), ..Default::default() });
                    c.out += out;
                    c.served += served;
                    if !etr.is_empty() && c.etr.is_empty() {
                        c.etr = etr;
                    }
                    if out > 0 {
                        c.utilities.push((s.name.clone(), out));
                    }
                    o += out;
                    sv += served;
                }
                utilities.push((s.name.clone(), o, sv));
            }
            Err(e) => problems.push(format!("{}: {e}", s.name)),
        }
    }
    let now = chrono::Utc::now().timestamp();
    let mut saved: Saved = std::fs::read_to_string(history_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let total_out: u32 = utilities.iter().map(|u| u.1).sum();
    let total_served: u32 = utilities.iter().map(|u| u.2).sum();
    if !utilities.is_empty() {
        saved.history.push((now, total_out));
        saved.history.retain(|(t, _)| now - t <= 72 * 3600);
        // A new event starts once outages fall to almost nothing.
        if total_out < 50 {
            saved.peak = 0;
            saved.county_peak.clear();
        }
        saved.peak = saved.peak.max(total_out);
        for c in by_county.values_mut() {
            let p = saved.county_peak.entry(c.name.to_lowercase()).or_default();
            *p = (*p).max(c.out);
            c.peak = *p;
        }
        if let Some(dir) = history_path().parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(t) = serde_json::to_string(&saved) {
            let _ = std::fs::write(history_path(), t);
        }
    }
    let facilities = if previous_facilities.is_empty() { facilities(bbox).await.unwrap_or_else(|e| {
        problems.push(format!("facilities: {e}"));
        Vec::new()
    }) } else { previous_facilities };
    let mut counties: Vec<CountyOutage> = by_county.into_values().collect();
    counties.sort_by(|a, b| b.out.cmp(&a.out));
    Outages { counties, utilities, total_out, total_served, history: saved.history, peak: saved.peak, facilities, updated: now, problems }
}

/// Hospitals, water / wastewater plants, fire and EMS stations, nursing homes (OpenStreetMap,
/// cached for a week).
async fn facilities(bbox: (f64, f64, f64, f64)) -> Result<Vec<Facility>> {
    let cache = LibraryPaths::get().root.join("Outages").join("facilities.json");
    let fresh = std::fs::metadata(&cache).and_then(|m| m.modified()).map(|t| t.elapsed().map(|e| e.as_secs() < 7 * 86_400).unwrap_or(false)).unwrap_or(false);
    let json: serde_json::Value = if fresh {
        serde_json::from_str(&std::fs::read_to_string(&cache)?)?
    } else {
        let (s, w, n, e) = bbox;
        let b = format!("{s:.3},{w:.3},{n:.3},{e:.3}");
        let q = format!(
            "[out:json][timeout:60];(nwr[\"amenity\"=\"hospital\"]({b});nwr[\"man_made\"=\"water_works\"]({b});nwr[\"man_made\"=\"wastewater_plant\"]({b});nwr[\"amenity\"=\"fire_station\"]({b});nwr[\"emergency\"=\"ambulance_station\"]({b});nwr[\"amenity\"=\"nursing_home\"]({b});nwr[\"social_facility\"=\"nursing_home\"]({b}););out center tags;"
        );
        let resp = reqwest::Client::new().post("https://overpass-api.de/api/interpreter").form(&[("data", q)]).timeout(std::time::Duration::from_secs(90)).send().await?;
        let v: serde_json::Value = resp.json().await?;
        if let Some(dir) = cache.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&cache, v.to_string());
        v
    };
    let counties = crate::map::boundaries::counties_named();
    Ok(json
        .get("elements")
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
        .filter_map(|el| {
            let lat = el.get("lat").or_else(|| el.pointer("/center/lat"))?.as_f64()?;
            let lon = el.get("lon").or_else(|| el.pointer("/center/lon"))?.as_f64()?;
            let tags = el.get("tags")?;
            let t = |k: &str| tags.get(k).and_then(|x| x.as_str()).unwrap_or("");
            let kind = match (t("amenity"), t("man_made"), t("emergency"), t("social_facility")) {
                ("hospital", _, _, _) => "hospital",
                (_, "water_works", _, _) => "water",
                (_, "wastewater_plant", _, _) => "wastewater",
                ("fire_station", _, _, _) => "fire",
                (_, _, "ambulance_station", _) => "ems",
                ("nursing_home", _, _, _) | (_, _, _, "nursing_home") => "nursing",
                _ => return None,
            };
            let name = if t("name").is_empty() { kind.to_string() } else { t("name").to_string() };
            let county = counties.as_ref().and_then(|c| crate::map::boundaries::county_at(c, lat, lon)).unwrap_or_default();
            Some(Facility { name, kind: kind.into(), lat, lon, county })
        })
        .collect())
}

fn ramp(pct: f32) -> (u8, u8, u8, u8) {
    match pct {
        p if p >= 50.0 => (150, 0, 60, 190),
        p if p >= 25.0 => (220, 30, 30, 175),
        p if p >= 10.0 => (245, 120, 20, 160),
        p if p >= 2.0 => (250, 200, 40, 140),
        p if p > 0.0 => (120, 200, 120, 110),
        _ => (0, 0, 0, 0),
    }
}

/// Map layer: counties shaded by share of customers out ("now") or by how much has been restored
/// since the peak ("restoration"); critical facilities in hard-hit counties ("infrastructure").
pub fn draw(pm: &mut Pixmap, glyphs: &mut GlyphCache, vp: &Viewport, o: &Outages, mode: &str, state_fips: &str, brand: &Brand) {
    let label = style(&brand.font_body, Weight::Bold, 17.0, Color::WHITE);
    let Some(counties) = crate::map::boundaries::counties_named() else { return };
    for c in counties.iter().filter(|c| state_fips.is_empty() || c.state == state_fips) {
        let Some(out) = o.county(&c.name) else { continue };
        let (r, g, b, a) = if mode == "restoration" {
            let p = out.restored_pct();
            if out.peak == 0 {
                continue;
            }
            match p {
                p if p >= 90.0 => (40, 190, 100, 150),
                p if p >= 60.0 => (160, 210, 60, 150),
                p if p >= 30.0 => (245, 170, 30, 160),
                _ => (220, 40, 40, 170),
            }
        } else {
            ramp(out.pct())
        };
        if a == 0 {
            continue;
        }
        let mut centre: Option<(f32, f32, f32)> = None;
        for ring in &c.rings {
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
                pm.stroke_path(&path, &paint(Color::from_rgba8(255, 255, 255, 120)), &stroke(1.0), Transform::identity(), None);
                let bb = path.bounds();
                let area = bb.width() * bb.height();
                if centre.map(|c| area > c.2).unwrap_or(true) {
                    centre = Some(((bb.left() + bb.right()) / 2.0, (bb.top() + bb.bottom()) / 2.0, area));
                }
            }
        }
        if let Some((x, y, area)) = centre {
            if area > 2500.0 && out.out > 0 {
                let t = if mode == "restoration" { format!("{:.0}%", out.restored_pct()) } else if out.out >= 1000 { format!("{:.1}k", out.out as f32 / 1000.0) } else { out.out.to_string() };
                halo_text(pm, glyphs, &t, x - 16.0, y + 6.0, &label);
            }
        }
    }
    if mode == "infrastructure" {
        for f in &o.facilities {
            let pct = o.county(&f.county).map(|c| c.pct()).unwrap_or(0.0);
            if pct < 2.0 {
                continue;
            }
            let (x, y) = vp.to_screen(f.lat, f.lon);
            let (cr, cg, cb) = match f.kind.as_str() {
                "hospital" => (230, 60, 60),
                "water" | "wastewater" => (40, 150, 230),
                "fire" | "ems" => (240, 140, 30),
                _ => (180, 120, 220),
            };
            let mut pb = PathBuilder::new();
            pb.push_circle(x, y, 8.0 + (pct / 10.0).min(6.0));
            if let Some(path) = pb.finish() {
                pm.fill_path(&path, &paint(Color::from_rgba8(cr, cg, cb, 255)), FillRule::Winding, Transform::identity(), None);
                pm.stroke_path(&path, &paint(Color::WHITE), &stroke(2.0), Transform::identity(), None);
            }
            if vp.view.zoom >= 8.5 {
                halo_text(pm, glyphs, &f.name, x + 14.0, y + 6.0, &label);
            }
        }
    }
}

/// Bindings: power.total, power.pct, power.top1..5 (+ .count / .pct / .etr), power.utility1..4,
/// power.restored, power.trend, power.etr, power.facilities; series power.history.
pub fn bind(b: &mut crate::gfx::bind::Bindings, o: &Outages) {
    let fmt = |n: u32| {
        let s = n.to_string();
        let mut out = String::new();
        for (i, ch) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(ch);
        }
        out
    };
    // Nothing fetched yet: no utilities are set up (Emergency Ops > Power outages).
    if o.updated == 0 && o.utilities.is_empty() {
        b.set("power.status", "Outage tracking isn't set up: add your utilities in Emergency Ops > Power outages");
        b.set("power.total", "--");
        b.set("power.top1", "No outage data yet");
        b.set("power.utility1", "No utilities set up");
        return;
    }
    let updated = chrono::DateTime::from_timestamp(o.updated, 0)
        .map(|d| d.with_timezone(&chrono::Local).format("%-I:%M %p").to_string())
        .unwrap_or_default();
    b.set("power.status", &format!("From {} utilit{} • updated {updated}", o.utilities.len(), if o.utilities.len() == 1 { "y" } else { "ies" }));
    if o.total_out == 0 {
        b.set("power.top1", "No outages reported");
    }
    b.set("power.total", &fmt(o.total_out));
    b.set("power.pct", &if o.total_served > 0 { format!("{:.1}%", o.total_out as f32 * 100.0 / o.total_served as f32) } else { "--".into() });
    b.set("power.restored", &format!("{:.0}%", o.restored_pct()));
    b.set("power.peak", &fmt(o.peak));
    let t = o.trend();
    b.set("power.trend", &if t > 0 { format!("Up {} in the last hour", fmt(t as u32)) } else if t < 0 { format!("Down {} in the last hour", fmt((-t) as u32)) } else { "Steady".into() });
    for (i, c) in o.counties.iter().filter(|c| c.out > 0).take(8).enumerate() {
        let k = format!("power.top{}", i + 1);
        b.set(&k, &c.name);
        b.set(&format!("{k}.count"), &fmt(c.out));
        b.set(&format!("{k}.pct"), &format!("{:.0}%", c.pct()));
        b.set(&format!("{k}.etr"), if c.etr.is_empty() { "Assessing" } else { &c.etr });
        b.set(&format!("{k}.restored"), &format!("{:.0}%", c.restored_pct()));
    }
    for (i, (u, out, served)) in o.utilities.iter().enumerate().take(6) {
        let k = format!("power.utility{}", i + 1);
        b.set(&k, u);
        b.set(&format!("{k}.count"), &fmt(*out));
        b.set(&format!("{k}.pct"), &if *served > 0 { format!("{:.1}%", *out as f32 * 100.0 / *served as f32) } else { "--".into() });
    }
    let etr = o.counties.iter().filter(|c| !c.etr.is_empty()).map(|c| c.etr.clone()).max().unwrap_or_else(|| "Not yet estimated".into());
    b.set("power.etr", &etr);
    let hit: Vec<&Facility> = o.facilities.iter().filter(|f| o.county(&f.county).map(|c| c.pct() >= 10.0).unwrap_or(false)).collect();
    b.set("power.facilities", &format!("{} hospitals, {} water plants in hard-hit counties", hit.iter().filter(|f| f.kind == "hospital").count(), hit.iter().filter(|f| f.kind == "water" || f.kind == "wastewater").count()));
    let pts: Vec<(String, f32)> = o
        .history
        .iter()
        .rev()
        .step_by((o.history.len() / 24).max(1))
        .take(24)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|(t, n)| (chrono::DateTime::from_timestamp(*t, 0).map(|d| d.with_timezone(&chrono::Local).format("%-I%P").to_string()).unwrap_or_default(), *n as f32))
        .collect();
    b.set_series("power.history", pts);
    let counties: Vec<(String, f32)> = o.counties.iter().filter(|c| c.out > 0).take(10).map(|c| (c.name.clone(), c.out as f32)).collect();
    b.set_series("power.counties", counties);
    let utilities: Vec<(String, f32)> = o.utilities.iter().map(|u| (u.0.clone(), u.1 as f32)).collect();
    b.set_series("power.utilities", utilities);
}

pub type Shared = Arc<Outages>;

/// State FIPS filter for the map, set from Settings by the app.
pub fn state_filter() -> String {
    STATE.read().map(|s| s.clone()).unwrap_or_default()
}

pub fn set_state_filter(fips: &str) {
    if let Ok(mut s) = STATE.write() {
        *s = fips.to_string();
    }
}

static STATE: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());

#[cfg(test)]
mod oms_tests {
    #[test]
    fn totals_unplanned_outages_by_county() {
        let xml = "<OutageData><Incidents><Marker><Lat>39.7</Lat><Long>-86.1</Long><Etr>11/11/2026 09:00 PM</Etr><CustAffected>12</CustAffected><outageType>OUTAGE</outageType></Marker><Marker><Lat>39.71</Lat><Long>-86.11</Long><Etr></Etr><CustAffected>3</CustAffected><outageType>OUTAGE</outageType></Marker><Marker><Lat>39.7</Lat><Long>-86.1</Long><Etr></Etr><CustAffected>40</CustAffected><outageType>PLANNED_OUTAGE</outageType></Marker></Incidents></OutageData>";
        let rows = super::oms_rows(xml, |_, _| Some("Marion".into()));
        assert_eq!(rows, vec![("Marion".to_string(), 15, 0, "11/11/2026 09:00 PM".to_string())]);
    }
}
