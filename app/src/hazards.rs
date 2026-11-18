//! Tropical, fire and drought feeds, plus climate history at home:
//! - NHC active storms (NOAA map services): forecast cone, track, points, watches / warnings,
//!   tropical-storm wind probabilities, earliest arrival of tropical-storm winds, storm surge;
//!   ATCF model guidance for the spaghetti plot.
//! - SPC fire weather outlook, NIFC wildfire perimeters, US Drought Monitor.
//! - NOAA station climate data at home (ACIS / NOWData): month-to-date, year-to-date and season snowfall vs past years,
//!   and "on this day" history.

use anyhow::Result;

const NHC: &str = "https://mapservices.weather.noaa.gov/tropical/rest/services/tropical/NHC_tropical_weather/MapServer";
const SPC_FIRE: &str = "https://mapservices.weather.noaa.gov/vector/rest/services/fire_weather/SPC_firewx/MapServer";

#[derive(Clone, Debug, Default)]
pub struct Shape {
    pub label: String,
    /// (lon, lat) rings.
    pub rings: Vec<Vec<(f64, f64)>>,
    /// Lines (tracks), (lon, lat).
    pub lines: Vec<Vec<(f64, f64)>>,
    pub value: f32,
}

#[derive(Clone, Debug, Default)]
pub struct TrackPoint {
    pub lat: f64,
    pub lon: f64,
    pub label: String,
    /// Saffir-Simpson category text ("TS", "H1"…), wind mph.
    pub category: String,
    pub wind_mph: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Tropical {
    pub storm: String,
    pub cone: Vec<Shape>,
    pub track: Vec<Shape>,
    pub points: Vec<TrackPoint>,
    pub watches: Vec<Shape>,
    pub wind_prob: Vec<Shape>,
    pub arrival: Vec<Shape>,
    pub surge: Vec<Shape>,
    /// Model guidance: (model, [(lat, lon)]).
    pub models: Vec<(String, Vec<(f64, f64)>)>,
}

#[derive(Clone, Debug, Default)]
pub struct Hazards {
    pub tropical: Tropical,
    pub fire_outlook: Vec<Shape>,
    pub wildfires: Vec<Shape>,
    pub drought: Vec<Shape>,
    /// NOAA CPC drought outlooks: next month and the season ahead. Shape value = `DroughtTrend`
    /// code, label = CPC's class name.
    pub drought_month: Vec<Shape>,
    pub drought_season: Vec<Shape>,
    /// What the outlooks cover ("Nov 2026", "through January 31") and when CPC issued them.
    pub drought_month_target: String,
    pub drought_season_target: String,
    pub drought_outlook_issued: String,
    pub history: History,
}

/// The Tuesday a US Drought Monitor map is valid for: maps are valid Tuesday morning and come
/// out on Thursday, so from Monday to Wednesday the current map is last week's.
pub fn drought_valid_date(today: chrono::NaiveDate) -> chrono::NaiveDate {
    use chrono::Datelike;
    let wd = today.weekday().num_days_from_monday() as i64;
    let back = if wd >= 3 { wd - 1 } else { wd - 1 + 7 };
    today - chrono::Duration::days(back)
}

/// Whether a shape covers a point, even-odd over all its rings (drought maps have holes:
/// a town inside a non-drought pocket is not in drought).
pub fn shape_contains(s: &Shape, lat: f64, lon: f64) -> bool {
    s.rings.iter().filter(|r| crate::data::point_in_ring(r, lon, lat)).count() % 2 == 1
}

/// Worst US Drought Monitor category (0 = D0 … 4 = D4) at a point, None when not dry.
pub fn drought_at(shapes: &[Shape], lat: f64, lon: f64) -> Option<u8> {
    shapes.iter().filter(|s| shape_contains(s, lat, lon)).map(|s| s.value.max(0.0) as u8).max()
}

pub fn drought_label(category: Option<u8>) -> &'static str {
    match category {
        None => "No drought",
        Some(0) => "Abnormally dry (D0)",
        Some(1) => "Moderate drought (D1)",
        Some(2) => "Severe drought (D2)",
        Some(3) => "Extreme drought (D3)",
        Some(_) => "Exceptional drought (D4)",
    }
}

/// The CPC outlook at a point in plain words.
pub fn drought_trend_at(shapes: &[Shape], lat: f64, lon: f64) -> &'static str {
    match shapes.iter().find(|s| shape_contains(s, lat, lon)).map(|s| s.value as u8) {
        Some(1) => "Drought likely to continue",
        Some(2) => "Drought likely to improve",
        Some(3) => "Drought likely to end",
        Some(4) => "Drought likely to develop",
        Some(5) => "Drought likely to worsen",
        _ => "No drought expected",
    }
}

/// CPC drought outlook class → code (0 = no drought, skipped).
pub fn drought_trend_code(class: &str) -> u8 {
    match class.trim().to_ascii_lowercase().replace([' ', '-'], "_").as_str() {
        "persistence" | "persists" => 1,
        "improvement" | "improves" => 2,
        "removal" | "removal_likely" => 3,
        "development" | "development_likely" => 4,
        "worsens" | "worsening" => 5,
        _ => 0,
    }
}

const CPC_DROUGHT: &str = "https://mapservices.weather.noaa.gov/vector/rest/services/outlooks/cpc_drought_outlk/MapServer";

/// One CPC drought outlook (layer 1 = monthly, 4 = seasonal, US + Puerto Rico), simplified for
/// map drawing: (shapes, target, issued date).
pub async fn drought_outlook(layer: u32) -> Result<(Vec<Shape>, String, String)> {
    let url = format!("{CPC_DROUGHT}/{layer}/query?where=1%3D1&outFields=outlook,target,fcst_date&returnGeometry=true&outSR=4326&geometryPrecision=3&maxAllowableOffset=0.01&f=geojson");
    let v = crate::net::get_json(&url).await?;
    let feats = v.get("features").and_then(|f| f.as_array()).ok_or_else(|| anyhow::anyhow!("CPC drought outlook returned no features"))?;
    let prop = |f: &serde_json::Value, k: &str| f.pointer(&format!("/properties/{k}")).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let target = feats.first().map(|f| prop(f, "target")).unwrap_or_default();
    let issued = feats.first().map(|f| prop(f, "fcst_date")).unwrap_or_default();
    let mut out = Vec::new();
    for f in feats {
        let class = prop(f, "outlook");
        let code = drought_trend_code(&class);
        if code == 0 {
            continue;
        }
        let (mut rings, mut lines) = (Vec::new(), Vec::new());
        if let Some(g) = f.get("geometry") {
            rings_of(g, &mut rings, &mut lines);
        }
        out.push(Shape { label: class.replace('_', " "), rings, lines, value: code as f32 });
    }
    Ok((out, target, issued))
}

#[derive(Clone, Debug, Default)]
pub struct History {
    /// Month to date: (avg high, avg low, normal high, normal low, precip, normal precip).
    pub month: Option<(f32, f32, f32, f32, f32, f32)>,
    /// Year-to-date cumulative precipitation by day of year, this year and normal.
    pub ytd: Vec<(u32, f32, f32)>,
    /// Season snowfall to date (Jul 1 on) for this and the last 10 seasons: (season start year, inches).
    pub snow_seasons: Vec<(i32, f32)>,
    /// Yesterday: (high, low, precip, snow).
    pub yesterday: Option<(f32, f32, f32, f32)>,
    /// This date in past years: (year, high, low, precip), most notable first.
    pub this_day: Vec<(i32, f32, f32, f32)>,
    /// NOAA station behind the temperatures and precipitation (ACIS), for on-air credits.
    pub station: String,
    /// NOAA station behind snowfall, when it differs or exists.
    pub snow_station: String,
}

fn rings_of(geom: &serde_json::Value, rings: &mut Vec<Vec<(f64, f64)>>, lines: &mut Vec<Vec<(f64, f64)>>) {
    let pt = |p: &serde_json::Value| Some((p.get(0)?.as_f64()?, p.get(1)?.as_f64()?));
    let path = |a: &serde_json::Value| a.as_array().map(|v| v.iter().filter_map(pt).collect::<Vec<_>>()).unwrap_or_default();
    let coords = geom.get("coordinates");
    match geom.get("type").and_then(|t| t.as_str()) {
        Some("Polygon") => {
            for r in coords.and_then(|c| c.as_array()).into_iter().flatten() {
                rings.push(path(r));
            }
        }
        Some("MultiPolygon") => {
            for poly in coords.and_then(|c| c.as_array()).into_iter().flatten() {
                for r in poly.as_array().into_iter().flatten() {
                    rings.push(path(r));
                }
            }
        }
        Some("LineString") => lines.push(coords.map(path).unwrap_or_default()),
        Some("MultiLineString") => {
            for l in coords.and_then(|c| c.as_array()).into_iter().flatten() {
                lines.push(path(l));
            }
        }
        _ => {}
    }
}

fn shapes(features: &[serde_json::Value], label_keys: &[&str], value_keys: &[&str]) -> Vec<Shape> {
    features
        .iter()
        .filter_map(|f| {
            let p = f.get("properties")?;
            let label = label_keys.iter().find_map(|k| p.get(*k).and_then(|v| v.as_str().map(String::from).or_else(|| v.as_f64().map(|x| format!("{x}"))))).unwrap_or_default();
            let value = value_keys.iter().find_map(|k| p.get(*k).and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim_end_matches('%').parse().ok())))).unwrap_or(0.0) as f32;
            let (mut rings, mut lines) = (Vec::new(), Vec::new());
            rings_of(f.get("geometry")?, &mut rings, &mut lines);
            Some(Shape { label, rings, lines, value })
        })
        .collect()
}

async fn geojson(url: &str) -> Result<Vec<serde_json::Value>> {
    let v = crate::net::get_json(url).await?;
    Ok(v.get("features").and_then(|f| f.as_array()).cloned().unwrap_or_default())
}

/// Layer ids of a map service whose names contain `needle` (services renumber layers per storm).
async fn layers_named(service: &str, needle: &str) -> Vec<u32> {
    let Ok(v) = crate::net::get_json(&format!("{service}?f=json")).await else { return Vec::new() };
    v.get("layers")
        .and_then(|l| l.as_array())
        .map(|a| {
            a.iter()
                .filter(|l| l.get("name").and_then(|n| n.as_str()).map(|n| n.to_lowercase().contains(&needle.to_lowercase())).unwrap_or(false))
                .filter(|l| l.get("subLayerIds").map(|s| s.is_null()).unwrap_or(true))
                .filter_map(|l| l.get("id").and_then(|i| i.as_u64()).map(|i| i as u32))
                .collect()
        })
        .unwrap_or_default()
}

async fn query_named(service: &str, needle: &str) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for id in layers_named(service, needle).await.into_iter().take(6) {
        if let Ok(mut f) = geojson(&format!("{service}/{id}/query?where=1%3D1&outFields=*&returnGeometry=true&f=geojson")).await {
            out.append(&mut f);
        }
    }
    out
}

pub async fn tropical() -> Result<Tropical> {
    let mut t = Tropical::default();
    t.cone = shapes(&query_named(NHC, "forecast cone").await, &["stormname", "STORMNAME"], &[]);
    t.track = shapes(&query_named(NHC, "forecast track").await, &["stormname", "STORMNAME"], &[]);
    t.watches = shapes(&query_named(NHC, "watch").await, &["tcww", "TCWW"], &[]);
    t.wind_prob = shapes(&query_named(NHC, "34 kt").await, &["percentage", "PERCENTAGE"], &["percentage", "PERCENTAGE"]);
    t.arrival = shapes(&query_named(NHC, "arrival").await, &["label", "LABEL", "time", "TIME"], &[]);
    t.surge = shapes(&query_named(NHC, "surge").await, &["label", "LABEL"], &["value", "VALUE"]);
    for f in query_named(NHC, "forecast points").await {
        let Some(p) = f.get("properties") else { continue };
        let Some(c) = f.pointer("/geometry/coordinates") else { continue };
        let (Some(lon), Some(lat)) = (c.get(0).and_then(|x| x.as_f64()), c.get(1).and_then(|x| x.as_f64())) else { continue };
        let get = |k: &str| p.get(k).or_else(|| p.get(&k.to_uppercase())).cloned().unwrap_or_default();
        let wind_kt = get("maxwind").as_f64().unwrap_or(0.0) as f32;
        t.points.push(TrackPoint {
            lat,
            lon,
            label: get("datelbl").as_str().unwrap_or_default().to_string(),
            category: get("ssnum").as_f64().map(|n| if n >= 1.0 { format!("Cat {n:.0}") } else { get("tcdvlp").as_str().unwrap_or("TS").to_string() }).unwrap_or_default(),
            wind_mph: wind_kt * 1.15078,
        });
        if t.storm.is_empty() {
            t.storm = get("stormname").as_str().unwrap_or_default().to_string();
        }
    }
    Ok(t)
}

/// ATCF model guidance ("a-deck") for an Atlantic storm number this year: the newest run of
/// each common model.
pub async fn atcf_models(storm_num: u32) -> Result<Vec<(String, Vec<(f64, f64)>)>> {
    let year = chrono::Utc::now().format("%Y");
    let url = format!("https://ftp.nhc.noaa.gov/atcf/aid_public/aal{storm_num:02}{year}.dat.gz");
    let bytes = crate::net::get_logged(&url, std::time::Duration::from_secs(30)).await?.bytes().await?;
    let mut text = String::new();
    {
        use std::io::Read;
        let mut gz = flate2::read::GzDecoder::new(&bytes[..]);
        gz.read_to_string(&mut text)?;
    }
    let wanted = ["AVNO", "HWRF", "HMON", "CTCX", "EGRR", "NVGM", "OFCL", "TVCN", "HAFS", "EMXI"];
    let mut newest = String::new();
    let mut runs: std::collections::BTreeMap<String, Vec<(i32, f64, f64)>> = Default::default();
    for line in text.lines() {
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        if f.len() < 8 {
            continue;
        }
        let (date, model, tau) = (f[2].to_string(), f[4].to_string(), f[5].parse::<i32>().unwrap_or(-1));
        if !wanted.contains(&model.as_str()) {
            continue;
        }
        let parse = |s: &str| -> Option<f64> {
            let (num, hemi) = s.split_at(s.len().checked_sub(1)?);
            let v = num.parse::<f64>().ok()? / 10.0;
            Some(if hemi == "S" || hemi == "W" { -v } else { v })
        };
        let (Some(lat), Some(lon)) = (parse(f[6]), parse(f[7])) else { continue };
        if date > newest {
            newest = date.clone();
            runs.clear();
        }
        if date == newest {
            let e = runs.entry(model).or_default();
            if !e.iter().any(|p| p.0 == tau) {
                e.push((tau, lat, lon));
            }
        }
    }
    Ok(runs.into_iter().map(|(m, mut pts)| {
        pts.sort_by_key(|p| p.0);
        (m, pts.into_iter().map(|p| (p.1, p.2)).collect())
    }).collect())
}

pub async fn fire_outlook() -> Result<Vec<Shape>> {
    Ok(shapes(&query_named(SPC_FIRE, "day 1").await, &["label", "LABEL", "dn"], &["dn", "DN"]))
}

pub async fn wildfires(south: f64, north: f64, west: f64, east: f64) -> Result<Vec<Shape>> {
    let url = format!(
        "https://services3.arcgis.com/T4QMspbfLg3qTGWY/arcgis/rest/services/WFIGS_Interagency_Perimeters_Current/FeatureServer/0/query?where=1%3D1&outFields=poly_IncidentName,attr_IncidentSize,attr_PercentContained&geometry={west},{south},{east},{north}&geometryType=esriGeometryEnvelope&inSR=4326&spatialRel=esriSpatialRelIntersects&outSR=4326&f=geojson"
    );
    let feats = geojson(&url).await?;
    Ok(feats
        .iter()
        .filter_map(|f| {
            let p = f.get("properties")?;
            let name = p.get("poly_IncidentName").and_then(|v| v.as_str()).unwrap_or("Fire").to_string();
            let acres = p.get("attr_IncidentSize").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let pct = p.get("attr_PercentContained").and_then(|v| v.as_f64());
            let (mut rings, mut lines) = (Vec::new(), Vec::new());
            rings_of(f.get("geometry")?, &mut rings, &mut lines);
            Some(Shape { label: format!("{name}: {acres:.0} acres{}", pct.map(|p| format!(", {p:.0}% contained")).unwrap_or_default()), rings, lines, value: acres as f32 })
        })
        .collect())
}

pub async fn drought() -> Result<Vec<Shape>> {
    let value: serde_json::Value = crate::net::broadcaster_product(
        "outlooks/drought", "https://droughtmonitor.unl.edu/Data/json/USDM_current.json", std::time::Duration::from_secs(35),
    ).await?.json().await?;
    let feats = value.get("features").and_then(|f| f.as_array()).ok_or_else(|| anyhow::anyhow!("Drought feed returned no GeoJSON features"))?;
    Ok(shapes(feats, &["DM", "dm"], &["DM", "dm"]))
}

/// Climate history at home from NOAA station data (ACIS, as NWS NOWData).
pub async fn history(lat: f64, lon: f64, normals: Option<crate::climate::DayNormals>) -> Result<History> {
    use chrono::Datelike;
    let today = chrono::Local::now().date_naive();
    let start = chrono::NaiveDate::from_ymd_opt(today.year() - 11, 7, 1).unwrap_or(today);
    let end = today - chrono::Duration::days(1);
    // NOAA station data (ACIS, the source behind NWS NOWData): temperature and precipitation
    // from the nearest long-record station, snowfall from the nearest station that measures it.
    let (temp_station, snow_station) = crate::acis::stations(lat, lon).await?;
    let rows = crate::acis::daily(&temp_station.sid, start, end).await?;
    let snow_rows = match &snow_station {
        Some(s) if s.sid != temp_station.sid => crate::acis::daily(&s.sid, start, end).await.unwrap_or_default(),
        Some(_) => rows.clone(),
        None => Vec::new(),
    };
    let snow_by_date: std::collections::HashMap<chrono::NaiveDate, f32> = snow_rows.iter().map(|(d, v)| (*d, v[3])).collect();
    let dates: Vec<chrono::NaiveDate> = rows.iter().map(|(d, _)| *d).collect();
    let hi: Vec<f32> = rows.iter().map(|(_, v)| v[0]).collect();
    let lo: Vec<f32> = rows.iter().map(|(_, v)| v[1]).collect();
    let pr: Vec<f32> = rows.iter().map(|(_, v)| v[2]).collect();
    let sn: Vec<f32> = dates.iter().map(|d| snow_by_date.get(d).copied().unwrap_or(f32::NAN)).collect();
    let mut h = History {
        station: temp_station.name.clone(),
        snow_station: snow_station.map(|s| s.name).unwrap_or_default(),
        ..History::default()
    };
    let n = dates.len();
    if n == 0 {
        return Ok(h);
    }
    let last = n - 1;
    // Snowfall in inches as the observer measured it (trace counts as 0, missing days skipped).
    let snow_in = |i: usize| sn.get(i).copied().unwrap_or(0.0);
    h.yesterday = Some((hi[last], lo[last], pr[last], snow_in(last)));
    // Month to date.
    let month_idx: Vec<usize> = (0..n).filter(|i| dates[*i].year() == today.year() && dates[*i].month() == today.month()).collect();
    if let (false, Some(nm)) = (month_idx.is_empty(), normals.as_ref()) {
        let k = month_idx.len() as f32;
        let avg = |s: &[f32]| month_idx.iter().map(|i| s[*i]).filter(|x| x.is_finite()).sum::<f32>() / k;
        let nhi = month_idx.iter().map(|i| nm.hi.get(dates[*i].ordinal0() as usize).copied().unwrap_or(f32::NAN)).sum::<f32>() / k;
        let nlo = month_idx.iter().map(|i| nm.lo.get(dates[*i].ordinal0() as usize).copied().unwrap_or(f32::NAN)).sum::<f32>() / k;
        let mp = nm.month_precip.get(today.month0() as usize).copied().unwrap_or(0.0) * (today.day() as f32 - 1.0) / 30.0;
        h.month = Some((avg(&hi), avg(&lo), nhi, nlo, month_idx.iter().map(|i| pr[*i]).filter(|x| x.is_finite()).sum(), mp));
    }
    // Year to date, cumulative, with a normal built from the monthly normals.
    if let Some(nm) = normals.as_ref() {
        let (mut acc, mut nacc) = (0.0f32, 0.0f32);
        for i in (0..n).filter(|i| dates[*i].year() == today.year()) {
            acc += if pr[i].is_finite() { pr[i] } else { 0.0 };
            nacc += nm.month_precip.get(dates[i].month0() as usize).copied().unwrap_or(0.0) / 30.4;
            h.ytd.push((dates[i].ordinal(), acc, nacc));
        }
    }
    // Season snowfall to this date (Jul 1 to today's month/day) for each season.
    for back in 0..11 {
        let y = if today.month() >= 7 { today.year() - back } else { today.year() - 1 - back };
        let s0 = chrono::NaiveDate::from_ymd_opt(y, 7, 1);
        let s1 = chrono::NaiveDate::from_ymd_opt(y + if today.month() >= 7 { 0 } else { 1 }, today.month(), today.day().min(28));
        if let (Some(s0), Some(s1)) = (s0, s1) {
            let total: f32 = (0..n).filter(|i| dates[*i] >= s0 && dates[*i] <= s1).map(snow_in).filter(|x| x.is_finite()).sum();
            h.snow_seasons.push((y, total));
        }
    }
    // This date in past years.
    let mut days: Vec<(i32, f32, f32, f32)> = (0..n).filter(|i| dates[*i].month() == today.month() && dates[*i].day() == today.day()).map(|i| (dates[i].year(), hi[i], lo[i], pr[i])).collect();
    days.sort_by(|a, b| (b.3 * 20.0 + (b.1 - 75.0).abs()).total_cmp(&(a.3 * 20.0 + (a.1 - 75.0).abs())));
    h.this_day = days;
    Ok(h)
}
