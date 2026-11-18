//! Data for the historical graphics, all from the Iowa Environmental Mesonet (NWS VTEC
//! warnings, storm-based warning polygons, Local Storm Reports, ASOS daily summaries) and the
//! SPC tornado database. Finished years are kept on disk (they can't change); the current
//! year is fetched again at most once an hour.

use std::sync::{OnceLock, RwLock};

use chrono::{Datelike, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

/// The three warning types the graphics count.
pub const PHENS: [&str; 3] = ["TO", "SV", "FF"];

pub fn phen_name(p: &str) -> &'static str {
    match p {
        "TO" => "Tornado",
        "SV" => "Severe Thunderstorm",
        "FF" => "Flash Flood",
        _ => "Warning",
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Warn {
    pub phen: String,
    /// Unix seconds (UTC).
    pub issue: i64,
    /// County names in the home state.
    pub counties: Vec<String>,
    pub wfo: String,
    pub id: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Poly {
    pub phen: String,
    pub issue: i64,
    pub rings: Vec<Vec<(f64, f64)>>,
}

#[derive(Clone, Debug, Default)]
pub struct Report {
    pub kind: String,
    pub mag: Option<f64>,
    pub lon: f64,
    pub lat: f64,
    pub city: String,
    pub county: String,
    pub valid: i64,
}

#[derive(Clone, Debug, Default)]
pub struct CountyEvent {
    pub phen: String,
    pub sig: String,
    pub name: String,
    pub issue: i64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Daily {
    pub date: NaiveDate,
    pub hi: Option<f32>,
    pub lo: Option<f32>,
    pub precip: Option<f32>,
}

/// Monthly warning counts for one finished year: [phen][month].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct YearCounts {
    pub year: i32,
    pub months: [[u32; 12]; 3],
}

impl YearCounts {
    pub fn total(&self, p: usize) -> u32 {
        self.months[p].iter().sum()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Hist {
    /// Two-letter state, its FIPS, and the home county (name, 5-digit FIPS, UGC like INC037).
    pub state: String,
    pub state_fips: String,
    pub county: String,
    pub county_fips: String,
    pub ugc: String,
    pub year: i32,
    /// Unix seconds of the last successful refresh.
    pub fetched: i64,
    pub warns: Vec<Warn>,
    pub past: Vec<YearCounts>,
    pub polys: Vec<Poly>,
    pub reports: Vec<Report>,
    pub county_events: Vec<CountyEvent>,
    pub station: String,
    pub station_name: String,
    pub daily: Vec<Daily>,
    /// SPC tornadoes touching the home state, and those within ~40 miles of home.
    pub tornadoes: Vec<crate::history::Tornado>,
    pub near: Vec<crate::history::Tornado>,
    /// Home point (lat, lon) the data was fetched for.
    pub home: (f64, f64),
}

pub fn store() -> &'static RwLock<Option<Hist>> {
    static S: OnceLock<RwLock<Option<Hist>>> = OnceLock::new();
    S.get_or_init(|| RwLock::new(None))
}

pub fn snapshot() -> Option<Hist> {
    store().read().ok().and_then(|s| s.clone())
}

/// Where finished years are kept. Tests point it elsewhere with `CWX_HISTORY_CACHE` so they
/// never write into the app's own settings folder.
fn cache_dir() -> std::path::PathBuf {
    match std::env::var_os("CWX_HISTORY_CACHE") {
        Some(d) => std::path::PathBuf::from(d),
        None => crate::config::config_dir().join("history-cache"),
    }
}

const STATE_FIPS: &[(&str, &str)] = &[
    ("AL", "01"), ("AK", "02"), ("AZ", "04"), ("AR", "05"), ("CA", "06"), ("CO", "08"), ("CT", "09"), ("DE", "10"), ("DC", "11"),
    ("FL", "12"), ("GA", "13"), ("HI", "15"), ("ID", "16"), ("IL", "17"), ("IN", "18"), ("IA", "19"), ("KS", "20"), ("KY", "21"),
    ("LA", "22"), ("ME", "23"), ("MD", "24"), ("MA", "25"), ("MI", "26"), ("MN", "27"), ("MS", "28"), ("MO", "29"), ("MT", "30"),
    ("NE", "31"), ("NV", "32"), ("NH", "33"), ("NJ", "34"), ("NM", "35"), ("NY", "36"), ("NC", "37"), ("ND", "38"), ("OH", "39"),
    ("OK", "40"), ("OR", "41"), ("PA", "42"), ("RI", "44"), ("SC", "45"), ("SD", "46"), ("TN", "47"), ("TX", "48"), ("UT", "49"),
    ("VT", "50"), ("VA", "51"), ("WA", "53"), ("WV", "54"), ("WI", "55"), ("WY", "56"), ("PR", "72"),
];

pub fn state_abbr(fips: &str) -> Option<&'static str> {
    STATE_FIPS.iter().find(|(_, f)| *f == fips).map(|(a, _)| *a)
}

pub fn state_name(abbr: &str) -> &'static str {
    const NAMES: &[(&str, &str)] = &[
        ("AL", "Alabama"), ("AK", "Alaska"), ("AZ", "Arizona"), ("AR", "Arkansas"), ("CA", "California"), ("CO", "Colorado"),
        ("CT", "Connecticut"), ("DE", "Delaware"), ("DC", "District of Columbia"), ("FL", "Florida"), ("GA", "Georgia"), ("HI", "Hawaii"),
        ("ID", "Idaho"), ("IL", "Illinois"), ("IN", "Indiana"), ("IA", "Iowa"), ("KS", "Kansas"), ("KY", "Kentucky"), ("LA", "Louisiana"),
        ("ME", "Maine"), ("MD", "Maryland"), ("MA", "Massachusetts"), ("MI", "Michigan"), ("MN", "Minnesota"), ("MS", "Mississippi"),
        ("MO", "Missouri"), ("MT", "Montana"), ("NE", "Nebraska"), ("NV", "Nevada"), ("NH", "New Hampshire"), ("NJ", "New Jersey"),
        ("NM", "New Mexico"), ("NY", "New York"), ("NC", "North Carolina"), ("ND", "North Dakota"), ("OH", "Ohio"), ("OK", "Oklahoma"),
        ("OR", "Oregon"), ("PA", "Pennsylvania"), ("RI", "Rhode Island"), ("SC", "South Carolina"), ("SD", "South Dakota"),
        ("TN", "Tennessee"), ("TX", "Texas"), ("UT", "Utah"), ("VT", "Vermont"), ("VA", "Virginia"), ("WA", "Washington"),
        ("WV", "West Virginia"), ("WI", "Wisconsin"), ("WY", "Wyoming"), ("PR", "Puerto Rico"),
    ];
    NAMES.iter().find(|(a, _)| *a == abbr).map(|(_, n)| *n).unwrap_or("")
}

/// The home county from the home point: (state FIPS, county FIPS, county name).
pub fn home_county(lat: f64, lon: f64) -> Option<(String, String, String)> {
    let all = crate::map::boundaries::counties_named()?;
    all.iter()
        .find(|c| c.rings.iter().fold(false, |v, r| v ^ crate::gfx::winter_map::inside_pub(r, lon, lat)))
        .map(|c| (c.state.clone(), c.fips.clone(), c.name.clone()))
}

pub fn parse_time(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.timestamp())
        .ok()
        .or_else(|| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ").ok().map(|d| d.and_utc().timestamp()))
        .or_else(|| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%MZ").ok().map(|d| d.and_utc().timestamp()))
}

/// "Sullivan [IN], Vigo [IN], Clark [KY]" → the names in `state`.
pub fn parse_locations(text: &str, state: &str) -> Vec<String> {
    let tag = format!("[{state}]");
    text.split(',')
        .map(str::trim)
        .filter(|t| t.ends_with(&tag))
        .map(|t| t.trim_end_matches(&tag).trim().to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

/// IEM `vtec_events_bystate.py` → warnings.
pub fn parse_bystate(v: &serde_json::Value, state: &str) -> Vec<Warn> {
    v.get("events")
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
        .filter_map(|e| {
            Some(Warn {
                phen: e.get("phenomena")?.as_str()?.to_string(),
                issue: parse_time(e.get("issue")?.as_str()?)?,
                counties: parse_locations(e.get("locations").and_then(|l| l.as_str()).unwrap_or(""), state),
                wfo: e.get("wfo").and_then(|w| w.as_str()).unwrap_or("").to_string(),
                id: e.get("eventid").and_then(|i| i.as_u64()).unwrap_or(0) as u32,
            })
        })
        .collect()
}

pub fn month_counts(warns: &[Warn], year: i32) -> [[u32; 12]; 3] {
    let mut m = [[0u32; 12]; 3];
    for w in warns {
        let Some(p) = PHENS.iter().position(|p| *p == w.phen) else { continue };
        let Some(d) = chrono::DateTime::from_timestamp(w.issue, 0) else { continue };
        let local = d.with_timezone(&chrono::Local);
        if local.year() == year {
            m[p][local.month0() as usize] += 1;
        }
    }
    m
}

/// Storm-based warning features (`api/1/vtec/sbw_interval.geojson`) touching `state`.
pub fn parse_polys(v: &serde_json::Value, state: &str) -> Vec<Poly> {
    let tag = format!("{state}C");
    let mut out = Vec::new();
    for f in v.get("features").and_then(|f| f.as_array()).into_iter().flatten() {
        let p = f.get("properties").cloned().unwrap_or_default();
        let ugcs = p.get("ugclist").and_then(|u| u.as_str()).unwrap_or("");
        if !ugcs.contains(&tag) {
            continue;
        }
        let mut rings = Vec::new();
        if let Some(g) = f.get("geometry") {
            crate::data::collect_rings(g, &mut rings);
        }
        if rings.is_empty() {
            continue;
        }
        out.push(Poly {
            phen: p.get("phenomena").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            issue: p.get("utc_issue").and_then(|x| x.as_str()).and_then(parse_time).unwrap_or(0),
            rings,
        });
    }
    out
}

pub fn parse_lsr(v: &serde_json::Value) -> Vec<Report> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<Report> = v.get("features")
        .and_then(|f| f.as_array())
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let p = f.get("properties")?;
            let s = |k: &str| p.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let mag = p.get("magnitude").and_then(|m| m.as_f64().or_else(|| m.as_str().and_then(|s| s.trim().parse().ok())));
            Some(Report {
                kind: s("typetext"),
                mag,
                lon: p.get("lon").and_then(|x| x.as_f64())?,
                lat: p.get("lat").and_then(|x| x.as_f64())?,
                city: s("city"),
                county: s("county"),
                valid: parse_time(&s("valid")).unwrap_or(0),
            })
        })
        .collect();
    // The same report is often sent twice (a correction or a summary): keep one.
    out.retain(|r| seen.insert((r.kind.clone(), r.city.clone(), r.valid, r.mag.map(|m| (m * 100.).round() as i64))));
    out
}

pub fn parse_byugc(v: &serde_json::Value) -> Vec<CountyEvent> {
    v.get("events")
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let s = |k: &str| e.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            Some(CountyEvent { phen: s("phenomena"), sig: s("significance"), name: s("name"), issue: parse_time(&s("issue"))? })
        })
        .collect()
}

pub fn parse_daily(v: &serde_json::Value) -> Vec<Daily> {
    v.get("data")
        .and_then(|d| d.as_array())
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let f = |k: &str| r.get(k).and_then(|x| x.as_f64()).map(|x| x as f32);
            Some(Daily {
                date: NaiveDate::parse_from_str(r.get("date")?.as_str()?, "%Y-%m-%d").ok()?,
                hi: f("max_tmpf"),
                lo: f("min_tmpf"),
                precip: f("precip").map(|p| if p < 0.001 { 0.0 } else { p }),
            })
        })
        .collect()
}

async fn json(url: &str) -> Option<serde_json::Value> {
    let r = crate::net::get_logged(url, std::time::Duration::from_secs(90)).await.ok()?;
    let bytes = r.bytes().await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// A finished year from the disk cache, or fetched and kept.
async fn cached_json(name: &str, url: &str) -> Option<serde_json::Value> {
    let path = cache_dir().join(name);
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(v) = serde_json::from_str(&text) {
            return Some(v);
        }
    }
    let v = json(url).await?;
    let _ = std::fs::create_dir_all(cache_dir());
    let _ = std::fs::write(&path, v.to_string());
    Some(v)
}

const IEM: &str = "https://mesonet.agron.iastate.edu";

/// Fetch everything (at most once an hour; the first call fills the store).
pub async fn refresh(home: crate::config::GeoView) {
    let now = chrono::Utc::now().timestamp();
    if let Some(h) = snapshot() {
        if now - h.fetched < 3600 {
            return;
        }
    }
    let _ = crate::map::boundaries::ensure_loaded().await;
    let Some((state_fips, county_fips, county)) = home_county(home.lat, home.lon) else { return };
    let Some(state) = state_abbr(&state_fips).map(str::to_string) else { return };
    let year = chrono::Local::now().year();
    let ugc = format!("{state}C{}", county_fips.get(2..5).unwrap_or(""));
    let mut h = Hist { state: state.clone(), state_fips, county, county_fips, ugc: ugc.clone(), year, fetched: now, home: (home.lat, home.lon), ..Default::default() };

    // This year's warnings statewide.
    for p in PHENS {
        if let Some(v) = json(&format!("{IEM}/json/vtec_events_bystate.py?state={state}&year={year}&phenomena={p}&significance=W")).await {
            h.warns.extend(parse_bystate(&v, &state));
        }
    }
    // The ten finished years before it (kept on disk).
    for y in (year - 10)..year {
        let mut warns = Vec::new();
        let mut ok = true;
        for p in PHENS {
            match cached_json(&format!("bystate-{state}-{y}-{p}.json"), &format!("{IEM}/json/vtec_events_bystate.py?state={state}&year={y}&phenomena={p}&significance=W")).await {
                Some(v) => warns.extend(parse_bystate(&v, &state)),
                None => ok = false,
            }
        }
        if ok {
            h.past.push(YearCounts { year: y, months: month_counts(&warns, y) });
        }
    }
    // Polygons this year, from every office that warned in the state.
    let mut wfos: Vec<String> = h.warns.iter().map(|w| w.wfo.clone()).filter(|w| !w.is_empty()).collect();
    wfos.sort();
    wfos.dedup();
    for wfo in &wfos {
        for p in ["TO", "SV"] {
            let url = format!("{IEM}/api/1/vtec/sbw_interval.geojson?begints={year}-01-01T00:00Z&endts={}&wfo={wfo}&only_new=true&ph={p}", chrono::Utc::now().format("%Y-%m-%dT%H:%MZ"));
            if let Some(v) = json(&url).await {
                h.polys.extend(parse_polys(&v, &state));
            }
        }
    }
    // Storm reports this year.
    if let Some(v) = json(&format!("{IEM}/geojson/lsr.geojson?sts={year}-01-01T00:00Z&ets={}&states={state}", chrono::Utc::now().format("%Y-%m-%dT%H:%MZ"))).await {
        h.reports = parse_lsr(&v);
    }
    // Every warning / advisory for the home county since 2005.
    if let Some(v) = json(&format!("{IEM}/json/vtec_events_byugc.py?ugc={ugc}&sdate=2005-01-01&edate={}", chrono::Local::now().format("%Y-%m-%d"))).await {
        h.county_events = parse_byugc(&v);
    }
    // Daily highs, lows and rain at the nearest ASOS station, 21 years.
    // Stations from IEM's own network list (the relay doesn't pass the NWS station list).
    if let Some(net) = json(&format!("{IEM}/geojson/network/{state}_ASOS.geojson")).await {
        let st = state.clone();
        let mut stations: Vec<(f64, String)> = net
            .get("features")
            .and_then(|f| f.as_array())
            .into_iter()
            .flatten()
            .filter_map(|f| {
                let p = f.get("properties")?;
                if p.get("online").and_then(|o| o.as_bool()) != Some(true) {
                    return None;
                }
                let begin = p.get("archive_begin").and_then(|b| b.as_str()).and_then(|b| b.get(0..4)).and_then(|y| y.parse::<i32>().ok()).unwrap_or(9999);
                if begin > year - 15 {
                    return None;
                }
                let lon = f.pointer("/geometry/coordinates/0")?.as_f64()?;
                let lat = f.pointer("/geometry/coordinates/1")?.as_f64()?;
                Some((super::miles((home.lat, home.lon), (lat, lon)), p.get("sid")?.as_str()?.to_string()))
            })
            .collect();
        stations.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        for (_, id) in stations.into_iter().take(3) {
            let mut days = Vec::new();
            for y in (year - 20)..=year {
                let url = format!("{IEM}/api/1/daily.json?station={id}&network={st}_ASOS&year={y}");
                let v = if y < year { cached_json(&format!("asos-{id}-{y}.json"), &url).await } else { json(&url).await };
                if let Some(v) = v {
                    if h.station_name.is_empty() {
                        h.station_name = v.pointer("/data/0/name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                    }
                    days.extend(parse_daily(&v));
                }
            }
            if days.len() > 365 * 5 {
                h.station = id;
                h.daily = days;
                break;
            }
            h.station_name.clear();
        }
    }
    // SPC tornado database (1950 to last year) within the state's box.
    let _ = crate::history::ensure_tornado_db().await;
    if let Some(all) = crate::map::boundaries::counties_named() {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for c in all.iter().filter(|c| c.state == h.state_fips) {
            for &(lon, lat) in c.rings.iter().flatten() {
                b = (b.0.min(lon), b.1.min(lat), b.2.max(lon), b.3.max(lat));
            }
        }
        if b.0 < b.2 {
            let counties: Vec<_> = all.iter().filter(|c| c.state == h.state_fips).collect();
            let hit = |(lat, lon): (f64, f64)| counties.iter().any(|c| c.rings.iter().fold(false, |v, r| v ^ crate::gfx::winter_map::inside_pub(r, lon, lat)));
            h.tornadoes = crate::history::tornadoes(b.1, b.3, b.0, b.2).into_iter().filter(|t| hit(t.start) || hit(t.end)).collect();
        }
        let d = 0.6;
        h.near = crate::history::tornadoes(home.lat - d, home.lat + d, home.lon - d * 1.3, home.lon + d * 1.3);
    }
    if let Ok(mut s) = store().write() {
        *s = Some(h);
    }
}

/// Days of year as month/day for "on this day" matching.
pub fn same_day(t: i64, month: u32, day: u32) -> bool {
    chrono::DateTime::from_timestamp(t, 0)
        .map(|d| d.with_timezone(&chrono::Local))
        .is_some_and(|d| d.month() == month && d.day() == day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_keep_only_the_state() {
        assert_eq!(parse_locations("Sullivan [IN], Vigo [IN], Hardin [KY]", "IN"), vec!["Sullivan", "Vigo"]);
        assert!(parse_locations("", "IN").is_empty());
    }

    #[test]
    fn bystate_and_months() {
        let v = serde_json::json!({"events": [
            {"phenomena": "TO", "issue": "2026-02-19T22:31:00Z", "locations": "Sullivan [IN]", "wfo": "IND", "eventid": 1},
            {"phenomena": "SV", "issue": "2026-05-07T18:00:00Z", "locations": "Dubois [IN], Pike [IN]", "wfo": "LMK", "eventid": 9}
        ]});
        let w = parse_bystate(&v, "IN");
        assert_eq!(w.len(), 2);
        assert_eq!(w[1].counties, vec!["Dubois", "Pike"]);
        let m = month_counts(&w, 2026);
        assert_eq!(m[0][1], 1);
        assert_eq!(m[1][4], 1);
    }

    #[test]
    fn lsr_magnitude_text_or_number() {
        let v = serde_json::json!({"features": [
            {"properties": {"typetext": "HAIL", "magnitude": "1.75", "lon": -86.9, "lat": 38.4, "city": "Jasper", "county": "Dubois", "valid": "2026-05-07T18:00:00Z"}},
            {"properties": {"typetext": "TSTM WND GST", "magnitude": 61.0, "lon": -86.9, "lat": 38.4, "city": "Huntingburg", "county": "Dubois", "valid": "2026-05-07T18:10:00Z"}}
        ]});
        let r = parse_lsr(&v);
        assert_eq!(r[0].mag, Some(1.75));
        assert_eq!(r[1].mag, Some(61.0));
    }
}
