//! Historical events: search a date / time window, rebuild its warnings (IEM storm-based
//! warnings), storm reports (IEM LSRs), radar (IEM composite archive) and satellite (NASA GIBS
//! GOES-East), play back observations, compare the archived model forecast with what happened,
//! build a timeline, save case studies and export them as a show. Also the SPC tornado database
//! (1950 on) for tornado history maps.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CaseStudy {
    pub name: String,
    /// Unix seconds.
    pub start: i64,
    pub end: i64,
    /// south, north, west, east
    pub bbox: (f64, f64, f64, f64),
    pub notes: String,
}

#[derive(Clone, Debug, Default)]
pub struct Event {
    pub case: CaseStudy,
    pub warnings: Vec<crate::data::Alert>,
    pub reports: Vec<crate::data::StormReport>,
    /// Hourly ASOS reports at the nearest station: (unix, temp °F, precip in, wind gust mph).
    pub obs: Vec<(i64, f32, f32, f32)>,
    /// GFS MOS forecast issued before the event, 3-hourly: (unix, temp, precip = NaN).
    pub forecast: Vec<(i64, f32, f32)>,
    /// Station ids the observations and the forecast came from (e.g. KHNB).
    pub obs_station: String,
    pub forecast_station: String,
    /// (unix, text) in order.
    pub timeline: Vec<(i64, String)>,
}

#[derive(Clone, Debug)]
pub struct Tornado {
    pub year: i32,
    pub date: String,
    pub ef: i32,
    pub start: (f64, f64),
    pub end: (f64, f64),
    pub fatalities: u32,
}

fn iso(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0).map(|d| d.format("%Y-%m-%dT%H:%MZ").to_string()).unwrap_or_default()
}

pub fn cases_path() -> std::path::PathBuf {
    crate::config::LibraryPaths::get().root.join("Case studies.json")
}

pub fn load_cases() -> Vec<CaseStudy> {
    std::fs::read_to_string(cases_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_cases(cases: &[CaseStudy]) {
    if let Ok(t) = serde_json::to_string_pretty(cases) {
        let _ = std::fs::write(cases_path(), t);
    }
}

/// Everything for a window: warnings, reports, observations, archived forecast, timeline.
pub async fn load_event(case: CaseStudy, home: (f64, f64)) -> Result<Event> {
    let (s, n, w, e) = case.bbox;
    let mut ev = Event { case: case.clone(), ..Default::default() };
    // Storm-based warnings (polygons).
    let sbw = format!("https://mesonet.agron.iastate.edu/geojson/sbw.geojson?sts={}&ets={}", iso(case.start), iso(case.end));
    if let Ok(v) = crate::net::get_json(&sbw).await {
        for f in v.get("features").and_then(|f| f.as_array()).into_iter().flatten() {
            let p = f.get("properties").cloned().unwrap_or_default();
            let mut rings = Vec::new();
            if let Some(polys) = f.pointer("/geometry/coordinates").and_then(|c| c.as_array()) {
                for poly in polys {
                    let ring: Vec<(f64, f64)> = poly.as_array().map(|r| r.iter().filter_map(|pt| Some((pt.get(0)?.as_f64()?, pt.get(1)?.as_f64()?))).collect()).unwrap_or_default();
                    // MultiPolygon nests one level deeper.
                    if ring.is_empty() {
                        for r in poly.as_array().into_iter().flatten() {
                            rings.push(r.as_array().map(|r| r.iter().filter_map(|pt| Some((pt.get(0)?.as_f64()?, pt.get(1)?.as_f64()?))).collect()).unwrap_or_default());
                        }
                    } else {
                        rings.push(ring);
                    }
                }
            }
            let inside = rings.iter().flatten().any(|(lon, lat)| *lat >= s && *lat <= n && *lon >= w && *lon <= e);
            if !inside {
                continue;
            }
            let phen = p.get("phenomena").and_then(|x| x.as_str()).unwrap_or("");
            let event = match phen {
                "TO" => "Tornado Warning",
                "SV" => "Severe Thunderstorm Warning",
                "FF" => "Flash Flood Warning",
                "MA" => "Special Marine Warning",
                "SQ" => "Snow Squall Warning",
                _ => "Warning",
            };
            let issue = p.get("issue").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let expire = p.get("expire").and_then(|x| x.as_str()).unwrap_or("").to_string();
            ev.warnings.push(crate::data::Alert {
                id: format!("HIST-{}-{}", p.get("wfo").and_then(|x| x.as_str()).unwrap_or(""), p.get("eventid").map(|x| x.to_string()).unwrap_or_default()),
                event: event.to_string(),
                headline: format!("{event} ({})", p.get("wfo").and_then(|x| x.as_str()).unwrap_or("")),
                sent: Some(issue.clone()),
                expires: Some(expire),
                polygons: rings,
                ..Default::default()
            });
            if let Some(t) = chrono::DateTime::parse_from_rfc3339(&issue).ok().map(|d| d.timestamp()).or_else(|| chrono::NaiveDateTime::parse_from_str(&issue, "%Y-%m-%dT%H:%MZ").ok().map(|d| d.and_utc().timestamp())) {
                ev.timeline.push((t, format!("{event} issued")));
            }
        }
    }
    // Local storm reports.
    let lsr = format!("https://mesonet.agron.iastate.edu/geojson/lsr.geojson?sts={}&ets={}", iso(case.start), iso(case.end));
    if let Ok(v) = crate::net::get_json(&lsr).await {
        for f in v.get("features").and_then(|f| f.as_array()).into_iter().flatten() {
            let p = f.get("properties").cloned().unwrap_or_default();
            let (Some(lon), Some(lat)) = (f.pointer("/geometry/coordinates/0").and_then(|x| x.as_f64()), f.pointer("/geometry/coordinates/1").and_then(|x| x.as_f64())) else { continue };
            if lat < s || lat > n || lon < w || lon > e {
                continue;
            }
            let valid = p.get("valid").and_then(|x| x.as_str()).unwrap_or("");
            let time = chrono::NaiveDateTime::parse_from_str(valid, "%Y-%m-%dT%H:%M:%SZ").or_else(|_| chrono::NaiveDateTime::parse_from_str(valid, "%Y-%m-%d %H:%M")).map(|d| d.and_utc().timestamp()).unwrap_or(case.start);
            let text = p.get("typetext").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let city = p.get("city").and_then(|x| x.as_str()).unwrap_or("").to_string();
            ev.timeline.push((time, format!("{text} at {city}")));
            ev.reports.push(crate::data::StormReport {
                lat,
                lon,
                kind: p.get("type").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                text,
                magnitude: p.get("magnitude").map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())).unwrap_or_default(),
                city,
                county: p.get("county").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                source: p.get("source").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                remark: p.get("remark").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                time,
                ..Default::default()
            });
        }
    }
    // Observations and the forecast made at the time, at the NWS observation stations nearest
    // home: measured ASOS reports from IEM's archive, and NOAA's GFS MOS guidance from the run
    // issued before the event (no Open-Meteo: CWXB-48).
    let stations = nearest_stations(home).await.unwrap_or_default();
    for id in &stations {
        if let Ok(obs) = asos_obs(id, case.start, case.end).await {
            if !obs.is_empty() {
                ev.obs = obs;
                ev.obs_station = id.clone();
                break;
            }
        }
    }
    for id in &stations {
        if let Ok(fc) = mos_forecast(id, case.start, case.end).await {
            if !fc.is_empty() {
                ev.forecast = fc;
                ev.forecast_station = id.clone();
                break;
            }
        }
    }
    ev.timeline.sort_by_key(|t| t.0);
    ev.timeline.dedup_by(|a, b| a.1 == b.1 && (a.0 - b.0).abs() < 300);
    Ok(ev)
}

/// The NWS observation stations nearest `home` (lat, lon), closest first (e.g. KHNB, KEVV).
async fn nearest_stations(home: (f64, f64)) -> Result<Vec<String>> {
    Ok(nearest_stations_with_state(home).await?.1)
}

/// The point's state ("IN") and its nearest NWS observation stations, closest first.
pub(crate) async fn nearest_stations_with_state(home: (f64, f64)) -> Result<(String, Vec<String>)> {
    let point = crate::net::get_json(&format!("https://api.weather.gov/points/{:.4},{:.4}", home.0, home.1)).await?;
    let state = point.pointer("/properties/relativeLocation/properties/state").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let url = point.pointer("/properties/observationStations").and_then(|v| v.as_str()).context("no observation stations for home")?;
    let list = crate::net::get_json(url).await?;
    let ids = list["features"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| f.pointer("/properties/stationIdentifier").and_then(|v| v.as_str()).map(String::from))
        .take(4)
        .collect();
    Ok((state, ids))
}

/// Hourly ASOS reports (time, °F, precipitation in, gust mph) from IEM's archive. IEM names US
/// stations without the leading K (KHNB → HNB). Trace precipitation counts as 0.
async fn asos_obs(station: &str, start: i64, end: i64) -> Result<Vec<(i64, f32, f32, f32)>> {
    let id = if station.len() == 4 && station.starts_with('K') { &station[1..] } else { station };
    let iso = |t: i64| chrono::DateTime::from_timestamp(t, 0).map(|d| d.format("%Y-%m-%dT%H:%MZ").to_string()).unwrap_or_default();
    let url = format!(
        "https://mesonet.agron.iastate.edu/cgi-bin/request/asos.py?station={id}&data=tmpf&data=p01i&data=gust&tz=Etc/UTC&format=onlycomma&latlon=no&missing=M&trace=T&report_type=3&sts={}&ets={}",
        iso(start),
        iso(end)
    );
    let text = String::from_utf8(crate::net::get_bytes(&url).await?)?;
    Ok(parse_asos(&text))
}

/// IEM ASOS CSV: station,valid,tmpf,p01i,gust ("M" missing, "T" trace).
fn parse_asos(text: &str) -> Vec<(i64, f32, f32, f32)> {
    let num = |v: &str| match v.trim() {
        "M" | "" => f32::NAN,
        "T" => 0.0,
        x => x.parse().unwrap_or(f32::NAN),
    };
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let c: Vec<&str> = line.split(',').collect();
            let t = chrono::NaiveDateTime::parse_from_str(c.get(1)?, "%Y-%m-%d %H:%M").ok()?.and_utc().timestamp();
            let rain = num(c.get(3)?);
            Some((t, num(c.get(2)?), if rain.is_nan() { 0.0 } else { rain }, num(c.get(4)?)))
        })
        .collect()
}

/// The GFS MOS forecast (time, °F, precipitation) issued before `start`, 3-hourly, from IEM's
/// MOS archive. MOS gives rain chances and categories, not amounts, so precipitation is NaN
/// (the replay then shows what fell without a made-up forecast amount).
async fn mos_forecast(station: &str, start: i64, end: i64) -> Result<Vec<(i64, f32, f32)>> {
    // GFS MOS runs at 00/06/12/18Z and is out ~4 h later: take the last run 6 h or more before
    // the event.
    let run = (start - 6 * 3600).div_euclid(6 * 3600) * 6 * 3600;
    let run_iso = chrono::DateTime::from_timestamp(run, 0).map(|d| d.format("%Y-%m-%dT%H:%MZ").to_string()).unwrap_or_default();
    let v = crate::net::get_json(&format!("https://mesonet.agron.iastate.edu/api/1/mos.json?station={station}&model=GFS&runtime={run_iso}")).await?;
    Ok(v["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let t = chrono::NaiveDateTime::parse_from_str(r["ftime"].as_str()?, "%Y-%m-%d %H:%M").ok()?.and_utc().timestamp();
            let tmp = r["tmp"].as_f64()? as f32;
            (t >= start - 3 * 3600 && t <= end + 3 * 3600).then_some((t, tmp, f32::NAN))
        })
        .collect())
}

pub const RADAR_REPLAY_HOURS: [u32; 4] = [3, 6, 12, 24];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteReplay {
    pub site: String,
    pub scans: Vec<i64>,
}

pub async fn site_replay(base: &str, site: String) -> Result<SiteReplay> {
    let data = crate::net::get_json(&format!("{}/api/weatherbroadcaster/archive-scans?site={site}", base.trim_end_matches('/'))).await?;
    let mut scans = data["scans"].as_array().into_iter().flatten()
        .filter_map(|scan| {
            let stamp = scan["ts"].as_str()?;
            let utc = chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%dT%H:%MZ").ok()?;
            Some(utc.and_utc().timestamp())
        }).collect::<Vec<_>>();
    scans.sort_unstable();
    scans.dedup();
    anyhow::ensure!(!scans.is_empty(), "No archived storm-relative velocity scans for K{site}");
    let stride = scans.len().div_ceil(49);
    let last = scans.last().copied().unwrap();
    let mut scans = scans.into_iter().step_by(stride).collect::<Vec<_>>();
    if scans.last() != Some(&last) { scans.push(last); }
    Ok(SiteReplay { site, scans })
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RadarReplay {
    pub end: i64,
    pub hours: u32,
}

impl RadarReplay {
    pub fn recent(hours: u32, now: i64) -> Self {
        // Give the archive time to publish its most recent composite.
        let end = (now - 15 * 60).div_euclid(300) * 300;
        Self { end, hours }
    }

    pub fn frames(self) -> Vec<i64> {
        let hours = self.hours.clamp(1, 24);
        let minutes = match hours { 1..=3 => 5, 4..=6 => 10, 7..=12 => 15, _ => 30 };
        let start = self.end - i64::from(hours) * 3600;
        (start..self.end).step_by(minutes * 60).chain(std::iter::once(self.end)).collect()
    }
}

#[cfg(test)]
mod radar_replay_tests {
    use super::*;

    #[test]
    fn replay_covers_the_requested_span_with_archived_composite_times() {
        let now = 1_800_001_241;
        for hours in RADAR_REPLAY_HOURS {
            let replay = RadarReplay::recent(hours, now);
            let frames = replay.frames();
            assert!(frames.len() <= 49);
            assert_eq!(frames.last().copied(), Some(replay.end));
            assert_eq!(replay.end - frames[0], i64::from(hours) * 3600);
            assert!(frames.iter().all(|time| time % 300 == 0 && *time <= now - 900));
        }
    }
}

/// IEM composite radar tiles for a past time (5-minute steps).
pub fn radar_tiles(t: i64) -> String {
    let t = t - t.rem_euclid(300);
    let stamp = chrono::DateTime::from_timestamp(t, 0).map(|d| d.format("%Y%m%d%H%M").to_string()).unwrap_or_default();
    format!("https://mesonet.agron.iastate.edu/cache/tile.py/1.0.0/ridge::USCOMP-N0Q-{stamp}/{{z}}/{{x}}/{{y}}.png")
}

/// NASA GIBS GOES-East clean infrared for a past time (10-minute steps).
pub fn satellite_tiles(t: i64) -> String {
    let t = t - t.rem_euclid(600);
    let stamp = chrono::DateTime::from_timestamp(t, 0).map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string()).unwrap_or_default();
    format!("https://gibs.earthdata.nasa.gov/wmts/epsg3857/best/GOES-East_ABI_Band13_Clean_Infrared/default/{stamp}/GoogleMapsCompatible_Level7/{{z}}/{{y}}/{{x}}.png")
}

fn tornado_path() -> std::path::PathBuf {
    crate::config::config_dir().join("spc-tornadoes.csv")
}

/// Download the SPC tornado database once (about 10 MB) and keep it.
pub async fn ensure_tornado_db() -> Result<()> {
    if tornado_path().exists() {
        return Ok(());
    }
    let year = chrono::Utc::now().format("%Y").to_string().parse::<i32>().unwrap_or(2026);
    for last in [year - 1, year - 2, year - 3] {
        let url = format!("https://www.spc.noaa.gov/wcm/data/1950-{last}_actual_tornadoes.csv");
        if let Ok(resp) = crate::net::get_logged(&url, std::time::Duration::from_secs(120)).await {
            if let Ok(bytes) = resp.bytes().await {
                if bytes.len() > 100_000 {
                    std::fs::write(tornado_path(), &bytes)?;
                    return Ok(());
                }
            }
        }
    }
    Err(anyhow!("couldn't download the SPC tornado database"))
}

/// Tornadoes whose path touches the box (from the downloaded database).
pub fn tornadoes(south: f64, north: f64, west: f64, east: f64) -> Vec<Tornado> {
    static CACHE: std::sync::OnceLock<Vec<Tornado>> = std::sync::OnceLock::new();
    let all = CACHE.get_or_init(|| {
        let Ok(text) = std::fs::read_to_string(tornado_path()) else { return Vec::new() };
        let mut lines = text.lines();
        let header: Vec<&str> = lines.next().unwrap_or("").split(',').collect();
        let col = |n: &str| header.iter().position(|h| h.trim().eq_ignore_ascii_case(n));
        let (Some(yr), Some(date), Some(mag), Some(fat), Some(slat), Some(slon), Some(elat), Some(elon)) = (col("yr"), col("date"), col("mag"), col("fat"), col("slat"), col("slon"), col("elat"), col("elon")) else { return Vec::new() };
        lines
            .filter_map(|l| {
                let f: Vec<&str> = l.split(',').collect();
                let num = |i: usize| f.get(i).and_then(|v| v.trim().parse::<f64>().ok());
                let start = (num(slat)?, num(slon)?);
                let mut end = (num(elat).unwrap_or(0.0), num(elon).unwrap_or(0.0));
                if end.0 == 0.0 {
                    end = start;
                }
                Some(Tornado { year: num(yr)? as i32, date: f.get(date)?.to_string(), ef: num(mag).unwrap_or(-9.0) as i32, start, end, fatalities: num(fat).unwrap_or(0.0) as u32 })
            })
            .collect()
    });
    all.iter().filter(|t| (t.start.0 >= south && t.start.0 <= north && t.start.1 >= west && t.start.1 <= east) || (t.end.0 >= south && t.end.0 <= north && t.end.1 >= west && t.end.1 <= east)).cloned().collect()
}

#[cfg(test)]
mod asos_tests {
    /// IEM ASOS CSV (real sample from EVV, 2024-05-07): trace rain counts as 0, missing is NaN.
    #[test]
    fn parses_iem_asos_csv() {
        let csv = "station,valid,tmpf,p01i,gust\nEVV,2024-05-07 16:54,75.00,0.00,19.00\nEVV,2024-05-07 17:54,78.00,T,29.00\nEVV,2024-05-07 18:54,M,0.12,M\n";
        let rows = super::parse_asos(csv);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].0, chrono::NaiveDate::from_ymd_opt(2024, 5, 7).unwrap().and_hms_opt(16, 54, 0).unwrap().and_utc().timestamp());
        assert_eq!((rows[0].1, rows[0].2, rows[0].3), (75.0, 0.0, 19.0));
        assert_eq!(rows[1].2, 0.0);
        assert!(rows[2].1.is_nan() && rows[2].3.is_nan());
        assert!((rows[2].2 - 0.12).abs() < 1e-6);
    }
}
