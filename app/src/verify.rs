//! Forecast verification. Each day the station forecast (with the forecaster's edits), the
//! Open-Meteo blend and the GFS / ECMWF / ICON runs are recorded for home and each forecast
//! region; recent observed highs, lows and rain are fetched for the same places. From the pairs:
//! bias, MAE, RMSE, rain-chance reliability, Brier score, skill against persistence and
//! climatology, rankings by place and season, and an automatic weekly report.

use std::{
    collections::{BTreeMap, HashMap},
    io::Write,
    path::PathBuf,
};

use anyhow::{anyhow, Result};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::config::{LibraryPaths, Settings};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Record {
    pub made: NaiveDate,
    pub source: String,
    pub place: String,
    pub target: NaiveDate,
    pub hi: Option<f32>,
    pub lo: Option<f32>,
    /// Rain chance 0-100.
    pub pop: Option<f32>,
    /// Inches.
    pub precip: Option<f32>,
}

impl Record {
    pub fn lead(&self) -> i64 {
        (self.target - self.made).num_days()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Observed {
    pub hi: f32,
    pub lo: f32,
    pub precip: f32,
}

fn dir() -> PathBuf {
    LibraryPaths::get().root.join("Verification")
}

pub fn load_records() -> Vec<Record> {
    std::fs::read_to_string(dir().join("forecasts.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// place -> date -> observed.
pub fn load_observed() -> BTreeMap<String, BTreeMap<NaiveDate, Observed>> {
    std::fs::read_to_string(dir().join("observed.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Append today's forecasts (once per made-date / source / place / target).
pub fn record(new: Vec<Record>) -> Result<usize> {
    std::fs::create_dir_all(dir())?;
    let have: std::collections::HashSet<(NaiveDate, String, String, NaiveDate)> = load_records().into_iter().map(|r| (r.made, r.source, r.place, r.target)).collect();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(dir().join("forecasts.jsonl"))?;
    let mut n = 0;
    for r in new {
        if have.contains(&(r.made, r.source.clone(), r.place.clone(), r.target)) {
            continue;
        }
        writeln!(f, "{}", serde_json::to_string(&r)?)?;
        n += 1;
    }
    Ok(n)
}

/// Places verified: home plus the centre of each forecast region.
pub fn places(settings: &Settings, edits: &crate::forecast_edit::EditSet) -> Vec<(String, f64, f64)> {
    let mut out = vec![(if settings.home_label.is_empty() { "Home".to_string() } else { settings.home_label.clone() }, settings.home.lat, settings.home.lon)];
    for r in &edits.regions {
        let centre = match &r.shape {
            crate::forecast_edit::Shape::Circle { lat, lon, .. } => Some((*lat, *lon)),
            crate::forecast_edit::Shape::Polygon(pts) if !pts.is_empty() => {
                let n = pts.len() as f64;
                Some((pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n))
            }
            _ => None,
        };
        if let Some((lat, lon)) = centre {
            out.push((r.name.clone(), lat, lon));
        }
    }
    out
}

/// Fetch everything needed for today's record and observations (async, off the UI thread).
pub async fn daily_update(places: Vec<(String, f64, f64)>, station: Vec<Record>) -> Result<(usize, usize)> {
    let today = chrono::Local::now().date_naive();
    let mut recs = station;
    let mut observed = load_observed();
    let mut obs_n = 0;
    for (name, lat, lon) in &places {
        // Model runs (GFS / ECMWF / ICON) and the Open-Meteo blend with rain chance.
        if let Ok(models) = crate::climate::models(*lat, *lon).await {
            for m in models {
                for (date, hi, pr) in m.days {
                    if date > today {
                        recs.push(Record { made: today, source: m.name.to_string(), place: name.clone(), target: date, hi: Some(hi), precip: Some(pr), ..Default::default() });
                    }
                }
            }
        }
        if let Ok((_, days)) = crate::climate::daily(*lat, *lon).await {
            for d in days.into_iter().filter(|d| d.date > today && (d.date - today).num_days() <= 7) {
                recs.push(Record { made: today, source: "Blend".into(), place: name.clone(), target: d.date, hi: Some(d.hi), lo: Some(d.lo), pop: Some(d.pop), precip: Some(d.precip) });
            }
        }
        // Observed: measured daily highs, lows and rain at the NWS (ASOS) station nearest the
        // place, from IEM's daily summaries (no Open-Meteo: CWXB-80).
        if let Ok(days) = observed_iem(*lat, *lon, today).await {
            let place = observed.entry(name.clone()).or_default();
            for (date, obs) in days {
                place.insert(date, obs);
                obs_n += 1;
            }
        }
    }
    std::fs::create_dir_all(dir())?;
    std::fs::write(dir().join("observed.json"), serde_json::to_string(&observed)?)?;
    let n = record(recs)?;
    Ok((n, obs_n))
}

/// Daily observations (before `today`) this month and last month at the nearest NWS ASOS
/// station with data, from IEM (`api/1/daily.json`, network "<state>_ASOS").
async fn observed_iem(lat: f64, lon: f64, today: NaiveDate) -> Result<Vec<(NaiveDate, Observed)>> {
    use chrono::Datelike;
    let (state, stations) = crate::history::nearest_stations_with_state((lat, lon)).await?;
    let last_month = today.with_day(1).and_then(|d| d.pred_opt()).unwrap_or(today);
    for station in stations {
        let id = if station.len() == 4 && station.starts_with('K') { &station[1..] } else { station.as_str() };
        let mut out = Vec::new();
        for month in [last_month, today] {
            let url = format!("https://mesonet.agron.iastate.edu/api/1/daily.json?station={id}&network={state}_ASOS&year={}&month={}", month.year(), month.month());
            if let Ok(v) = crate::net::get_json(&url).await {
                out.extend(parse_iem_daily(&v).into_iter().filter(|(d, _)| *d < today));
            }
        }
        if out.len() >= 5 {
            return Ok(out);
        }
    }
    Err(anyhow!("no IEM daily observations near {lat:.2}, {lon:.2}"))
}

/// IEM daily rows → (date, observed). Days missing a high or low are skipped; trace rain
/// (IEM sends 0.0001) counts as 0.
fn parse_iem_daily(v: &serde_json::Value) -> Vec<(NaiveDate, Observed)> {
    v.get("data").and_then(|d| d.as_array()).into_iter().flatten().filter_map(|row| {
        let date = NaiveDate::parse_from_str(row.get("date")?.as_str()?, "%Y-%m-%d").ok()?;
        let hi = row.get("max_tmpf")?.as_f64()? as f32;
        let lo = row.get("min_tmpf")?.as_f64()? as f32;
        let precip = row.get("precip").and_then(|p| p.as_f64()).map(|p| if p < 0.001 { 0.0 } else { p as f32 }).unwrap_or(0.0);
        Some((date, Observed { hi, lo, precip }))
    }).collect()
}

#[cfg(test)]
mod iem_tests {
    #[test]
    fn parses_iem_daily_rows() {
        let v = serde_json::json!({"data": [
            {"date": "2026-11-12", "max_tmpf": 55.0, "min_tmpf": 38.0, "precip": 0.02},
            {"date": "2026-11-13", "max_tmpf": 43.0, "min_tmpf": 37.0, "precip": 0.0001},
            {"date": "2026-11-14", "max_tmpf": null, "min_tmpf": 30.0, "precip": null}
        ]});
        let rows = super::parse_iem_daily(&v);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].1.precip, 0.02);
        assert_eq!(rows[1].1.precip, 0.0);
        assert_eq!(rows[1].1.hi, 43.0);
    }
}

/// The station's own forecast (with the forecaster's edits) as records.
pub fn station_records(data: &crate::data::WxSnapshot, place: &str) -> Vec<Record> {
    let today = chrono::Local::now().date_naive();
    let Some(o) = &data.overview else { return Vec::new() };
    let days = o.days();
    // A leading "Tonight" entry means the daytime entries start tomorrow.
    let offset = if days.first().map(|d| d.is_daytime).unwrap_or(true) { 0 } else { 1 };
    let mut out: Vec<Record> = Vec::new();
    let mut k = 0i64;
    for (i, d) in days.iter().enumerate() {
        if !d.is_daytime {
            continue;
        }
        let target = today + chrono::Duration::days(k + offset);
        k += 1;
        let edit = data.forecast_edits.day(i);
        let hi = edit.and_then(|e| e.hi).or(d.high.map(|v| v as f32));
        let lo = edit.and_then(|e| e.lo).or(d.low.map(|v| v as f32));
        let pop = edit.and_then(|e| e.pop).or(d.precip.map(|v| v as f32));
        if target > today && (target - today).num_days() <= 7 {
            out.push(Record { made: today, source: "Station".into(), place: place.to_string(), target, hi, lo, pop, precip: None });
        }
    }
    out
}

pub fn season(d: NaiveDate) -> &'static str {
    match d.month() {
        12 | 1 | 2 => "Winter",
        3..=5 => "Spring",
        6..=8 => "Summer",
        _ => "Fall",
    }
}

#[derive(Clone, Debug, Default)]
pub struct Errors {
    pub n: u32,
    pub sum: f32,
    pub abs: f32,
    pub sq: f32,
}

impl Errors {
    fn add(&mut self, e: f32) {
        self.n += 1;
        self.sum += e;
        self.abs += e.abs();
        self.sq += e * e;
    }
    pub fn bias(&self) -> f32 {
        if self.n == 0 { 0.0 } else { self.sum / self.n as f32 }
    }
    pub fn mae(&self) -> f32 {
        if self.n == 0 { 0.0 } else { self.abs / self.n as f32 }
    }
    pub fn rmse(&self) -> f32 {
        if self.n == 0 { 0.0 } else { (self.sq / self.n as f32).sqrt() }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SourceStats {
    pub hi: Errors,
    pub lo: Errors,
    pub precip: Errors,
    /// Rain chance bins (0-10 %, … 90-100 %): (forecasts, wet days, sum of forecast prob).
    pub reliability: [(u32, u32, f32); 10],
    pub brier_sum: f32,
    pub brier_n: u32,
}

impl SourceStats {
    pub fn brier(&self) -> Option<f32> {
        (self.brier_n > 0).then(|| self.brier_sum / self.brier_n as f32)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    /// (source, lead) -> stats over all places.
    pub by_lead: BTreeMap<(String, i64), SourceStats>,
    /// source -> stats (all leads, all places).
    pub by_source: BTreeMap<String, SourceStats>,
    /// place -> source -> high MAE (ranking by region).
    pub by_place: BTreeMap<String, BTreeMap<String, Errors>>,
    /// season -> source -> high MAE.
    pub by_season: BTreeMap<String, BTreeMap<String, Errors>>,
    /// Persistence (yesterday's high as the forecast) MAE for day-1 highs.
    pub persistence: Errors,
    /// Climatological wet-day frequency and its Brier score (reference for skill).
    pub clim_freq: f32,
    pub clim_brier: f32,
    pub pairs: usize,
    /// Forecasts and observations on file (pairs need a forecast day that has passed).
    pub recorded: usize,
    pub observed: usize,
}

impl Report {
    /// MAE skill against persistence for day-1 highs (1 = perfect, 0 = no better).
    pub fn skill_vs_persistence(&self, source: &str) -> Option<f32> {
        let s = self.by_lead.get(&(source.to_string(), 1))?;
        (self.persistence.n > 0 && s.hi.n > 0 && self.persistence.mae() > 0.0).then(|| 1.0 - s.hi.mae() / self.persistence.mae())
    }
    /// Brier skill against climatology.
    pub fn brier_skill(&self, source: &str) -> Option<f32> {
        let b = self.by_source.get(source)?.brier()?;
        (self.clim_brier > 0.0).then(|| 1.0 - b / self.clim_brier)
    }
}

/// Verify every recorded forecast that has an observation. `days` limits to recent targets.
pub fn compute(days: i64) -> Report {
    let records = load_records();
    let observed = load_observed();
    let cutoff = chrono::Local::now().date_naive() - chrono::Duration::days(days.max(1));
    let mut r = Report { recorded: records.len(), observed: observed.values().map(|days| days.len()).sum(), ..Report::default() };
    let mut wet = (0u32, 0u32);
    for rec in records.iter().filter(|x| x.target >= cutoff) {
        let Some(obs) = observed.get(&rec.place).and_then(|p| p.get(&rec.target)) else { continue };
        r.pairs += 1;
        let lead = rec.lead();
        let wet_day = obs.precip >= 0.01;
        let apply = |s: &mut SourceStats| {
            if let Some(h) = rec.hi {
                s.hi.add(h - obs.hi);
            }
            if let Some(l) = rec.lo {
                s.lo.add(l - obs.lo);
            }
            if let Some(p) = rec.precip {
                s.precip.add(p - obs.precip);
            }
            if let Some(pop) = rec.pop {
                let p = (pop / 100.0).clamp(0.0, 1.0);
                let bin = ((p * 10.0) as usize).min(9);
                s.reliability[bin].0 += 1;
                s.reliability[bin].1 += wet_day as u32;
                s.reliability[bin].2 += p;
                let o = if wet_day { 1.0 } else { 0.0 };
                s.brier_sum += (p - o) * (p - o);
                s.brier_n += 1;
            }
        };
        apply(r.by_lead.entry((rec.source.clone(), lead)).or_default());
        apply(r.by_source.entry(rec.source.clone()).or_default());
        if let Some(h) = rec.hi {
            r.by_place.entry(rec.place.clone()).or_default().entry(rec.source.clone()).or_default().add(h - obs.hi);
            r.by_season.entry(season(rec.target).to_string()).or_default().entry(rec.source.clone()).or_default().add(h - obs.hi);
        }
        if rec.pop.is_some() {
            wet.0 += 1;
            wet.1 += wet_day as u32;
        }
    }
    // Persistence and climatology references.
    for days_obs in observed.values() {
        let mut prev: Option<f32> = None;
        for (d, o) in days_obs.iter().filter(|(d, _)| **d >= cutoff) {
            let _ = d;
            if let Some(p) = prev {
                r.persistence.add(p - o.hi);
            }
            prev = Some(o.hi);
        }
    }
    if wet.0 > 0 {
        let f = wet.1 as f32 / wet.0 as f32;
        r.clim_freq = f;
        r.clim_brier = f * (1.0 - f);
    }
    r
}

/// Sources ranked by high-temperature MAE (best first).
pub fn ranking(map: &BTreeMap<String, Errors>) -> Vec<(String, f32, u32)> {
    let mut v: Vec<(String, f32, u32)> = map.iter().filter(|(_, e)| e.n > 0).map(|(s, e)| (s.clone(), e.mae(), e.n)).collect();
    v.sort_by(|a, b| a.1.total_cmp(&b.1));
    v
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;")
}

/// Reliability diagram as inline SVG.
fn reliability_svg(s: &SourceStats) -> String {
    let mut pts = String::new();
    for (i, (n, w, sum)) in s.reliability.iter().enumerate() {
        if *n == 0 {
            continue;
        }
        let _ = i;
        let fx = sum / *n as f32;
        let fy = *w as f32 / *n as f32;
        pts += &format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"#2b6cb0\"/>", 30.0 + fx * 240.0, 270.0 - fy * 240.0, 3.0 + (*n as f32).sqrt().min(9.0));
    }
    format!("<svg width=\"300\" height=\"300\" viewBox=\"0 0 300 300\" xmlns=\"http://www.w3.org/2000/svg\"><rect x=\"30\" y=\"30\" width=\"240\" height=\"240\" fill=\"none\" stroke=\"#999\"/><line x1=\"30\" y1=\"270\" x2=\"270\" y2=\"30\" stroke=\"#bbb\" stroke-dasharray=\"4\"/>{pts}<text x=\"150\" y=\"295\" font-size=\"11\" text-anchor=\"middle\">forecast chance</text><text x=\"12\" y=\"150\" font-size=\"11\" transform=\"rotate(-90 12 150)\" text-anchor=\"middle\">observed frequency</text></svg>")
}

/// HTML verification report in Exports.
pub fn export_report(days: i64, station: &str) -> Result<PathBuf> {
    let r = compute(days);
    let mut h = String::from("<!doctype html><meta charset=\"utf-8\"><title>Forecast verification</title><style>body{font:14px system-ui;max-width:1000px;margin:30px auto;padding:0 16px;color:#111}table{border-collapse:collapse;margin:8px 0 20px}td,th{border:1px solid #ddd;padding:4px 8px;font-size:13px;text-align:right}th:first-child,td:first-child{text-align:left}h2{margin-top:28px}</style>");
    h += &format!("<h1>Forecast verification</h1><p>{} • last {days} days • {} forecast / observation pairs • generated {}</p>", esc(station), r.pairs, chrono::Local::now().format("%b %-d %Y %-I:%M %p"));
    h += "<h2>By source and lead time (highs / lows, °F)</h2><table><tr><th>Source</th><th>Day</th><th>n</th><th>High bias</th><th>High MAE</th><th>High RMSE</th><th>Low bias</th><th>Low MAE</th><th>Rain MAE (in)</th><th>Brier</th></tr>";
    for ((src, lead), s) in &r.by_lead {
        h += &format!(
            "<tr><td>{}</td><td>{lead}</td><td>{}</td><td>{:+.1}</td><td>{:.1}</td><td>{:.1}</td><td>{:+.1}</td><td>{:.1}</td><td>{:.2}</td><td>{}</td></tr>",
            esc(src), s.hi.n, s.hi.bias(), s.hi.mae(), s.hi.rmse(), s.lo.bias(), s.lo.mae(), s.precip.mae(), s.brier().map(|b| format!("{b:.3}")).unwrap_or_else(|| "–".into())
        );
    }
    h += "</table><h2>Skill</h2><table><tr><th>Source</th><th>Day-1 high skill vs persistence</th><th>Rain chance skill vs climatology</th></tr>";
    for src in r.by_source.keys() {
        h += &format!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>", esc(src), r.skill_vs_persistence(src).map(|v| format!("{:.0}%", v * 100.0)).unwrap_or_else(|| "–".into()), r.brier_skill(src).map(|v| format!("{:.0}%", v * 100.0)).unwrap_or_else(|| "–".into()));
    }
    h += "</table><h2>Ranking by place (high MAE)</h2>";
    for (place, m) in &r.by_place {
        h += &format!("<h3>{}</h3><table><tr><th>Source</th><th>MAE</th><th>n</th></tr>", esc(place));
        for (s, mae, n) in ranking(m) {
            h += &format!("<tr><td>{}</td><td>{mae:.1}</td><td>{n}</td></tr>", esc(&s));
        }
        h += "</table>";
    }
    h += "<h2>Ranking by season (high MAE)</h2>";
    for (season, m) in &r.by_season {
        h += &format!("<h3>{season}</h3><table><tr><th>Source</th><th>MAE</th><th>n</th></tr>");
        for (s, mae, n) in ranking(m) {
            h += &format!("<tr><td>{}</td><td>{mae:.1}</td><td>{n}</td></tr>", esc(&s));
        }
        h += "</table>";
    }
    h += "<h2>Rain chance reliability</h2>";
    for (src, s) in r.by_source.iter().filter(|(_, s)| s.brier_n > 0) {
        h += &format!("<h3>{}</h3>{}", esc(src), reliability_svg(s));
    }
    let out = LibraryPaths::get().exports;
    std::fs::create_dir_all(&out)?;
    let file = out.join(format!("Verification report {}.html", chrono::Local::now().format("%Y-%m-%d")));
    std::fs::write(&file, h)?;
    Ok(file)
}

/// Weekly automatic report (Mondays), once.
pub fn weekly_due() -> bool {
    let today = chrono::Local::now().date_naive();
    if today.weekday() != chrono::Weekday::Mon {
        return false;
    }
    !LibraryPaths::get().exports.join(format!("Verification report {}.html", today.format("%Y-%m-%d"))).exists()
}

/// Graphics bindings (cached; recomputed every 10 minutes).
pub fn bind(b: &mut crate::gfx::bind::Bindings) {
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<(i64, HashMap<String, String>)>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new((0, HashMap::new())));
    let Ok(mut c) = cache.lock() else { return };
    let now = chrono::Utc::now().timestamp();
    if now - c.0 > 600 {
        *c = (now, bindings());
    }
    for (k, v) in &c.1 {
        b.set(k, v);
    }
}

/// Bindings for graphics: verif.<source>.mae / .bias (high temperature), verif.best, verif.pairs.
pub fn bindings() -> HashMap<String, String> {
    let r = compute(30);
    let mut out = HashMap::new();
    out.insert("verif.pairs".into(), r.pairs.to_string());
    for (src, s) in &r.by_source {
        let k = src.to_lowercase();
        out.insert(format!("verif.{k}.mae"), format!("{:.1}°", s.hi.mae()));
        out.insert(format!("verif.{k}.bias"), format!("{:+.1}°", s.hi.bias()));
    }
    let mut all: BTreeMap<String, Errors> = BTreeMap::new();
    for ((src, lead), s) in &r.by_lead {
        if *lead == 1 {
            all.insert(src.clone(), s.hi.clone());
        }
    }
    if let Some((best, mae, _)) = ranking(&all).first() {
        out.insert("verif.best".into(), best.clone());
        out.insert("verif.best_mae".into(), format!("{mae:.1}°"));
    }
    out
}
