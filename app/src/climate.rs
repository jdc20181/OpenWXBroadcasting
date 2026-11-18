//! Climate and outlook feeds for home (all keyless):
//! - 1991-2020 station normals, freeze dates and growing season (NOAA NCEI through the
//!   broadcaster relay, cached for 30 days). Daily records come from the nearest long-record
//!   NOAA station (ACIS, the data behind NWS NOWData) when the operator turns records on.
//! - month-to-date rain vs normal, a 16-day daily forecast and a three-model comparison
//!   (Open-Meteo forecast API),
//! - the aurora Kp forecast (NOAA SWPC) and US airport delays (FAA NAS status).

use anyhow::{anyhow, Result};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct DayNormals {
    /// Normal high / low for each day of the year (index 0 = Jan 1, 366 entries).
    pub hi: Vec<f32>,
    pub lo: Vec<f32>,
    /// Record high / low and their years per day of year.
    pub rec_hi: Vec<(f32, i32)>,
    pub rec_lo: Vec<(f32, i32)>,
    /// Normal precipitation per month (inches), index 0 = January.
    pub month_precip: Vec<f32>,
    /// Median last spring freeze and first fall freeze, as day of year (0-based).
    pub last_freeze: Option<u32>,
    pub first_freeze: Option<u32>,
    /// Median growing season (days between them).
    pub growing_days: Option<u32>,
    /// Years the normals cover.
    pub years: String,
    /// Unix seconds when computed.
    pub computed: i64,
    pub lat: f64,
    pub lon: f64,
    /// Where the normals came from, for on-air provenance. Empty in caches written before
    /// build 8, which were always Open-Meteo reanalysis.
    #[serde(default)]
    pub source: String,
    /// Station behind the temperature normals ("PATOKA LAKE, IN US"); empty for gridded data.
    #[serde(default)]
    pub station: String,
    /// Station behind the precipitation normals, when it differs.
    #[serde(default)]
    pub precip_station: String,
    /// NOAA station behind the daily records (ACIS period of record); empty when there are none.
    #[serde(default)]
    pub records_station: String,
}

pub const NOAA_NORMALS: &str = "NOAA 1991-2020 normals";

impl DayNormals {
    /// Index into the 366-entry daily tables. Feb 29 has its own slot, so dates after
    /// February line up in leap and common years alike.
    pub fn index(date: NaiveDate) -> usize {
        NaiveDate::from_ymd_opt(2024, date.month(), date.day()).map_or(0, |d| d.ordinal0() as usize).min(365)
    }

    pub fn is_noaa(&self) -> bool {
        self.source == NOAA_NORMALS
    }

    /// Short provenance line for graphics footers.
    pub fn source_label(&self) -> String {
        if !self.is_noaa() {
            return format!("{} normals (model reanalysis)", self.years);
        }
        if self.station.is_empty() {
            return NOAA_NORMALS.into();
        }
        format!("{NOAA_NORMALS}, {}", station_label(&self.station))
    }
}

/// "PATOKA LAKE, IN US" -> "Patoka Lake, IN". Two-letter state codes stay upper case.
pub fn station_label(name: &str) -> String {
    let name = name.trim().trim_end_matches(" US").trim();
    name.split(' ')
        .map(|w| {
            if w.len() == 2 && w.chars().all(|c| c.is_ascii_uppercase()) {
                return w.to_string();
            }
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c.flat_map(|x| x.to_lowercase())).collect()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Day16 {
    pub date: NaiveDate,
    pub hi: f32,
    pub lo: f32,
    pub pop: f32,
    pub precip: f32,
    pub code: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelRun {
    pub name: &'static str,
    /// (date, high °F, precipitation in)
    pub days: Vec<(NaiveDate, f32, f32)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AirportDelay {
    pub code: String,
    pub kind: String,
    pub reason: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Climate {
    pub normals: Option<DayNormals>,
    /// Rain since the 1st of the month (model analysis, inches).
    pub mtd_precip: Option<f32>,
    pub daily16: Vec<Day16>,
    pub models: Vec<ModelRun>,
    /// Highest Kp forecast in the next 3 days.
    pub kp_max: Option<f32>,
    pub airports: Vec<AirportDelay>,
}

fn cache_path() -> std::path::PathBuf {
    crate::config::config_dir().join("climate-normals.json")
}

fn doy(d: NaiveDate) -> usize {
    DayNormals::index(d)
}

/// Normals for home: NOAA station normals (NCEI 1991-2020), with daily record highs / lows
/// from the nearest long-record NOAA station (ACIS). Cached for 30 days; a failed refresh keeps
/// the cache. `with_records` is the operator's "records and history" switch.
async fn normals(lat: f64, lon: f64, with_records: bool) -> Result<Option<DayNormals>> {
    let cached = match tokio::fs::read_to_string(cache_path()).await {
        Ok(text) => serde_json::from_str::<DayNormals>(&text).ok()
            .filter(|n| (n.lat - lat).abs() < 0.05 && (n.lon - lon).abs() < 0.05),
        Err(_) => None,
    };
    let has_noaa_records = |n: &DayNormals| n.rec_hi.len() >= 365 && !n.records_station.is_empty();
    if let Some(n) = &cached {
        let fresh = chrono::Utc::now().timestamp() - n.computed < 30 * 86_400;
        if fresh && n.is_noaa() && (!with_records || has_noaa_records(n)) {
            return Ok(cached);
        }
    }
    let mut out = match noaa_normals(lat, lon).await {
        Ok(n) => n,
        Err(e) => {
            if cached.is_some() {
                return Ok(cached);
            }
            return Err(e.context("NOAA station normals unavailable"));
        }
    };
    // Records: NOAA station period of record (ACIS). Older caches held model-reanalysis
    // records; those are dropped rather than shown as station records.
    let records = if with_records { noaa_records(lat, lon).await } else { Err(anyhow!("records off")) };
    match records {
        Ok((hi, lo, station)) => {
            out.rec_hi = hi;
            out.rec_lo = lo;
            out.records_station = station;
        }
        Err(_) => {
            if let Some(c) = cached.as_ref().filter(|c| has_noaa_records(c)) {
                out.rec_hi = c.rec_hi.clone();
                out.rec_lo = c.rec_lo.clone();
                out.records_station = c.records_station.clone();
            }
        }
    }
    write_cache(&out).await;
    Ok(Some(out))
}

/// Daily records (366 slots) and the station name from NOAA ACIS.
async fn noaa_records(lat: f64, lon: f64) -> Result<(Vec<(f32, i32)>, Vec<(f32, i32)>, String)> {
    let (station, _) = crate::acis::stations(lat, lon).await?;
    let (hi, lo) = crate::acis::records(&station.sid).await?;
    Ok((hi, lo, station.name))
}

async fn write_cache(n: &DayNormals) {
    if let Some(dir) = cache_path().parent() {
        let _ = tokio::fs::create_dir_all(dir).await;
    }
    if let Ok(text) = serde_json::to_string(n) {
        let _ = tokio::fs::write(cache_path(), text).await;
    }
}

#[derive(Deserialize)]
struct NoaaStation {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NoaaNormals {
    station: NoaaStation,
    #[serde(default)]
    precip_station: Option<NoaaStation>,
    daily: Vec<NoaaDay>,
    #[serde(default)]
    monthly_precip: Vec<Option<f32>>,
    last_freeze: Option<String>,
    first_freeze: Option<String>,
    growing_days: Option<f32>,
}

#[derive(Deserialize)]
struct NoaaDay {
    date: String,
    hi: f32,
    lo: f32,
}

/// NOAA NCEI 1991-2020 normals for the nearest station (relay route `climate/normals`).
async fn noaa_normals(lat: f64, lon: f64) -> Result<DayNormals> {
    let path = format!("climate/normals?lat={lat:.4}&lon={lon:.4}");
    let raw: NoaaNormals = crate::net::broadcaster_route(&path, std::time::Duration::from_secs(60)).await?.json().await?;
    let mut hi = vec![f32::NAN; 366];
    let mut lo = vec![f32::NAN; 366];
    for d in &raw.daily {
        let Some((m, day)) = d.date.split_once('-') else { continue };
        let (Ok(m), Ok(day)) = (m.parse::<u32>(), day.parse::<u32>()) else { continue };
        if let Some(date) = NaiveDate::from_ymd_opt(2024, m, day) {
            hi[doy(date)] = d.hi;
            lo[doy(date)] = d.lo;
        }
    }
    // A missing Feb 29 borrows Feb 28.
    for t in [&mut hi, &mut lo] {
        if !t[59].is_finite() {
            t[59] = t[58];
        }
    }
    anyhow::ensure!(hi.iter().chain(&lo).all(|v| v.is_finite()), "NOAA normals are incomplete");
    let this_year = chrono::Local::now().year();
    let month_day = |s: &Option<String>| -> Option<u32> {
        let (m, d) = s.as_deref()?.split_once('/')?;
        NaiveDate::from_ymd_opt(this_year, m.trim().parse().ok()?, d.trim().parse().ok()?).map(|d| d.ordinal0())
    };
    let precip_complete = raw.monthly_precip.len() == 12 && raw.monthly_precip.iter().all(Option::is_some);
    Ok(DayNormals {
        hi,
        lo,
        rec_hi: Vec::new(),
        rec_lo: Vec::new(),
        month_precip: if precip_complete { raw.monthly_precip.iter().map(|p| p.unwrap_or(0.0)).collect() } else { Vec::new() },
        last_freeze: month_day(&raw.last_freeze),
        first_freeze: month_day(&raw.first_freeze),
        growing_days: raw.growing_days.filter(|g| *g > 0.0).map(|g| g.round() as u32),
        years: "1991-2020".into(),
        computed: chrono::Utc::now().timestamp(),
        lat,
        lon,
        source: NOAA_NORMALS.into(),
        precip_station: raw.precip_station.filter(|_| precip_complete).map(|s| s.name).unwrap_or_default(),
        station: raw.station.name,
        records_station: String::new(),
    })
}

/// Rain since the 1st of the month plus the 16-day daily forecast (one call).
pub async fn daily(lat: f64, lon: f64) -> Result<(Option<f32>, Vec<Day16>)> {
    let today = chrono::Local::now().date_naive();
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}&daily=temperature_2m_max,temperature_2m_min,precipitation_probability_max,precipitation_sum,weather_code&temperature_unit=fahrenheit&precipitation_unit=inch&timezone=auto&forecast_days=16&past_days={}",
        today.day0()
    );
    let v = crate::net::get_json(&url).await?;
    let d = v.get("daily").ok_or_else(|| anyhow!("forecast: no daily data"))?;
    let arr = |k: &str| d.get(k).and_then(|a| a.as_array()).cloned().unwrap_or_default();
    let (times, hi, lo, pop, pr, code) = (arr("time"), arr("temperature_2m_max"), arr("temperature_2m_min"), arr("precipitation_probability_max"), arr("precipitation_sum"), arr("weather_code"));
    let mut mtd = 0.0f32;
    let mut days = Vec::new();
    for (i, t) in times.iter().enumerate() {
        let Some(date) = t.as_str().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()) else { continue };
        let f = |a: &Vec<serde_json::Value>| a.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
        if date < today {
            mtd += f(&pr);
        } else {
            days.push(Day16 { date, hi: f(&hi), lo: f(&lo), pop: f(&pop), precip: f(&pr), code: f(&code) as i32 });
        }
    }
    Ok((Some(mtd), days))
}

/// Daily highs and precipitation from GFS, ECMWF and ICON for a week.
pub async fn models(lat: f64, lon: f64) -> Result<Vec<ModelRun>> {
    let names: [(&'static str, &str); 3] = [("GFS", "gfs_seamless"), ("ECMWF", "ecmwf_ifs025"), ("ICON", "icon_seamless")];
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}&daily=temperature_2m_max,precipitation_sum&models={}&forecast_days=7&temperature_unit=fahrenheit&precipitation_unit=inch&timezone=auto",
        names.iter().map(|(_, k)| *k).collect::<Vec<_>>().join(",")
    );
    let v = crate::net::get_json(&url).await?;
    let d = v.get("daily").ok_or_else(|| anyhow!("models: no daily data"))?;
    let times: Vec<NaiveDate> = d
        .get("time")
        .and_then(|a| a.as_array())
        .into_iter()
        .flatten()
        .filter_map(|t| NaiveDate::parse_from_str(t.as_str()?, "%Y-%m-%d").ok())
        .collect();
    Ok(names
        .iter()
        .map(|(label, key)| {
            let col = |k: &str| d.get(format!("{k}_{key}")).and_then(|a| a.as_array()).cloned().unwrap_or_default();
            let (hi, pr) = (col("temperature_2m_max"), col("precipitation_sum"));
            ModelRun {
                name: label,
                days: times
                    .iter()
                    .enumerate()
                    .filter_map(|(i, t)| Some((*t, hi.get(i)?.as_f64()? as f32, pr.get(i).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32)))
                    .collect(),
            }
        })
        .collect())
}

/// Highest predicted Kp in the coming three days (NOAA SWPC).
pub async fn kp_max() -> Result<Option<f32>> {
    let v = crate::net::get_json("https://services.swpc.noaa.gov/products/noaa-planetary-k-index-forecast.json").await?;
    let now = chrono::Utc::now();
    Ok(v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let t = chrono::NaiveDateTime::parse_from_str(row.get("time_tag")?.as_str()?, "%Y-%m-%dT%H:%M:%S").ok()?.and_utc();
            let predicted = row.get("observed").and_then(|o| o.as_str()) != Some("observed");
            (predicted && t >= now && t <= now + chrono::Duration::days(3)).then(|| row.get("kp")?.as_f64().map(|k| k as f32))?
        })
        .fold(None, |m: Option<f32>, k| Some(m.map_or(k, |m| m.max(k)))))
}

/// FAA airport delays and closures (US).
pub async fn airports() -> Result<Vec<AirportDelay>> {
    let xml = crate::net::broadcaster_product_optional(
        "aviation/airport-status", "https://nasstatus.faa.gov/api/airport-status-information", std::time::Duration::from_secs(25),
    )
        .await?
        .text()
        .await?;
    anyhow::ensure!(xml.contains("<AIRPORT_STATUS_INFORMATION>"), "FAA returned no airport status XML");
    let mut out = Vec::new();
    // Blocks: <Ground_Delay>, <Ground_Stop>, <Delay> (arrival/departure), <Airport> (closures).
    for (tag, kind) in [("Ground_Delay", "Ground delay"), ("Program", "Ground stop"), ("Delay", "Delay"), ("Airport", "Closure"), ("Airport_Closure", "Closure")] {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        let mut rest = xml.as_str();
        while let Some(s) = rest.find(&open) {
            let after = &rest[s + open.len()..];
            let Some(e) = after.find(&close) else { break };
            let block = &after[..e];
            let field = |name: &str| -> String {
                let o = format!("<{name}>");
                block.find(&o).and_then(|i| {
                    let a = &block[i + o.len()..];
                    a.find('<').map(|j| a[..j].trim().to_string())
                }).unwrap_or_default()
            };
            let code = field("ARPT");
            if !code.is_empty() {
                let detail = [field("Avg"), field("Min"), field("Max"), field("End_Time"), field("Reopen")]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push(AirportDelay { code, kind: kind.to_string(), reason: field("Reason"), detail });
            }
            rest = &after[e + close.len()..];
        }
    }
    out.dedup_by(|a, b| a.code == b.code && a.kind == b.kind);
    Ok(out)
}

/// Fetch everything (each part is optional: one failing feed doesn't hide the others).
pub async fn fetch_all(lat: f64, lon: f64, with_records: bool) -> (Climate, Vec<String>) {
    let mut problems = Vec::new();
    let (n, d, m, k, a) = futures_util::join!(normals(lat, lon, with_records), daily(lat, lon), models(lat, lon), kp_max(), airports());
    let mut c = Climate::default();
    match n {
        Ok(v) => c.normals = v,
        Err(e) => problems.push(format!("normals: {e}")),
    }
    match d {
        Ok((mtd, days)) => {
            c.mtd_precip = mtd;
            c.daily16 = days;
        }
        Err(e) => problems.push(format!("16-day: {e}")),
    }
    match m {
        Ok(v) => c.models = v,
        Err(e) => problems.push(format!("models: {e}")),
    }
    match k {
        Ok(v) => c.kp_max = v,
        Err(e) => problems.push(format!("aurora: {e}")),
    }
    match a {
        Ok(v) => c.airports = v,
        Err(e) => problems.push(format!("airports: {e}")),
    }
    (c, problems)
}

/// Upcoming major meteor showers (peak month, day, name, typical rate per hour).
pub const METEOR_SHOWERS: &[(u32, u32, &str, u32)] = &[
    (1, 3, "Quadrantids", 110),
    (4, 22, "Lyrids", 18),
    (5, 6, "Eta Aquariids", 50),
    (7, 30, "Delta Aquariids", 25),
    (8, 12, "Perseids", 100),
    (10, 21, "Orionids", 20),
    (11, 17, "Leonids", 15),
    (12, 14, "Geminids", 150),
    (12, 22, "Ursids", 10),
];

/// Major eclipses visible from the US (date, kind, where). Graphics say to confirm local
/// timing on NASA's eclipse site.
pub const US_ECLIPSES: &[(i32, u32, u32, &str, &str)] = &[
    (2028, 1, 12, "Partial lunar eclipse", "Visible across the US at night"),
    (2029, 1, 14, "Partial solar eclipse", "Much of North America"),
    (2029, 6, 26, "Total lunar eclipse", "Visible from the Americas"),
    (2029, 12, 20, "Total lunar eclipse", "Visible from the Americas"),
    (2033, 3, 30, "Total solar eclipse", "Alaska; partial elsewhere"),
    (2044, 8, 23, "Total solar eclipse", "Montana and North Dakota"),
    (2045, 8, 12, "Total solar eclipse", "Coast to coast, California to Florida"),
];
