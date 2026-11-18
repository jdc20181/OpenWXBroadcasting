//! Social graphics in the studio: the same alert cards and daily forecast posts the website posts
//! automatically, drawn by the same renderer (`dcwx-render`, shared with DCWeatherSite), so a post
//! can be made by hand for something special or to correct one.
//!
//! Sources:
//! - Live: NWS active alerts and the NWS 7-day forecast (api.weather.gov), live LibreWXR radar
//!   through the site's /api/radar proxy (TWC palette).
//! - Past: IEM's NWS archive. Warnings, watches and advisories for the home county
//!   (vtec_events_byugc), each with its original text and polygon; past forecasts from the
//!   archived zone forecast (ZFP) issued that morning; radar from IEM's composite archive.
//!
//! Nothing here is specific to one station: names, logo, colours and the home area come from
//! Settings, so this works for any community.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Datelike, Local, NaiveDate, Offset, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub use social_render::{ALERT_DESIGNS, FORECAST_DESIGNS, FORECAST_RATIOS};

/// What the studio remembers between uses. Modern is the production default.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Prefs {
    pub alert_design: String,
    pub forecast_design: String,
    pub forecast_theme: String,
    pub forecast_ratio: String,
    pub show_graph: bool,
    /// 0-100.
    pub watermark_opacity: f32,
    /// "outlook" (TV-style 7-Day Outlook), "county" (home 7-day in the chosen design),
    /// "coverage" (the coverage towns as a city table) or "regional" (the state forecast's table).
    pub forecast_scope: String,
    /// Section of the state forecast ("SOUTHERN INDIANA"); "" = from the home county's position.
    pub regional_section: String,
    /// City highlighted (and listed first) in the regional table; "" = none.
    pub regional_home_city: String,
    /// Outlook / home-county forecast for one coverage town (an alert spot's name); "" = home.
    pub forecast_place: String,
    /// Coverage towns left out of the coverage table (names).
    pub coverage_skip: Vec<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            alert_design: "modern".into(),
            forecast_design: "modern".into(),
            forecast_theme: "default".into(),
            forecast_ratio: "landscape".into(),
            show_graph: true,
            watermark_opacity: 45.0,
            forecast_scope: "outlook".into(),
            regional_section: String::new(),
            regional_home_city: String::new(),
            forecast_place: String::new(),
            coverage_skip: Vec::new(),
        }
    }
}

pub fn design_label(key: &str) -> &'static str {
    match key {
        "classic" => "Classic",
        "bold" => "Bold",
        "broadcast" => "Broadcast",
        _ => "Modern",
    }
}

pub fn ratio_label(key: &str) -> &'static str {
    match key {
        "square" => "Square 1080",
        "story" => "Story 9:16",
        _ => "Landscape 16:9",
    }
}

/// The six forecast themes (same as the website's Social Creator):
/// key, label, start, mid, end, accent, accent_strong, panel, border, ink, heading, muted, sun.
pub const THEMES: [(&str, &str, [&str; 11]); 6] = [
    ("default", "Night blue", ["#091221", "#0d1729", "#121d33", "#6de4ff", "#1fb9d8", "rgba(10, 19, 37, 0.78)", "rgba(155, 180, 214, 0.16)", "#e7ecf6", "#ffffff", "#9aa6ba", "#ffd166"]),
    ("midnight", "Midnight", ["#07080f", "#0b0d17", "#0f1120", "#9d8fff", "#7b6de0", "rgba(14, 16, 32, 0.78)", "rgba(140, 148, 210, 0.12)", "#e8eaf6", "#f0f2ff", "#8a90b8", "#ffd166"]),
    ("dracula", "Slate", ["#1e1f29", "#242634", "#2a2d39", "#8ec0cf", "#6da7ba", "rgba(35, 37, 50, 0.84)", "rgba(217, 224, 244, 0.12)", "#edf0f7", "#fbfcff", "#a8aec2", "#ffd166"]),
    ("light", "Light", ["#f5f7fb", "#edf1f7", "#e4e9f2", "#2e6ea4", "#235782", "rgba(255, 255, 255, 0.85)", "rgba(20, 32, 51, 0.12)", "#223042", "#142033", "#627087", "#d97706"]),
    ("overcast", "Overcast", ["#202733", "#26313d", "#2b3945", "#90b4c2", "#6f95a5", "rgba(34, 45, 56, 0.82)", "rgba(206, 219, 227, 0.13)", "#edf3f7", "#ffffff", "#9fb0bc", "#ffd166"]),
    ("lilac", "Lilac", ["#f5f3ff", "#f0edff", "#ebe8fa", "#6e58fe", "#5542e0", "rgba(255, 255, 255, 0.95)", "rgba(110, 105, 140, 0.12)", "#1e1c2d", "#141223", "#6e698c", "#d97706"]),
];

fn theme_json(key: &str) -> Value {
    let (_, _, t) = THEMES.iter().find(|t| t.0 == key).unwrap_or(&THEMES[0]);
    json!({ "start": t[0], "mid": t[1], "end": t[2], "accent": t[3], "accent_strong": t[4], "panel": t[5], "border": t[6], "ink": t[7], "heading": t[8], "muted": t[9], "sun": t[10] })
}

/// Renders a job (from [`alert_job`] or [`forecast_job`]) to PNG bytes. CPU work: call it off the
/// UI thread.
pub fn render(job: &Value) -> Result<Vec<u8>> {
    social_render::render(&serde_json::to_vec(job)?).map_err(|e| anyhow!(e))
}

// ------------------------------------------------------------------------------- text ---

/// Leaves out what the embedded font can't draw (emoji, symbols), and tidies spaces.
pub fn drawable(s: &str) -> String {
    let kept: String = s
        .chars()
        .filter(|&c| (c as u32) <= 0x024F || ('\u{2010}'..='\u{2027}').contains(&c) || c == '\u{2032}' || c == '\u{2033}' || c == '\u{2122}')
        .collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "Nov 14, 7:45 PM EST" in the computer's local time.
fn card_time(t: i64) -> String {
    let Some(d) = Local.timestamp_opt(t, 0).single() else { return String::new() };
    format!("{}, {} {}", d.format("%b %-d"), d.format("%-I:%M %p"), zone_abbr(t))
}

/// "7:42 PM EST".
fn clock_time(t: i64) -> String {
    let Some(d) = Local.timestamp_opt(t, 0).single() else { return String::new() };
    format!("{} {}", d.format("%-I:%M %p"), zone_abbr(t))
}

/// US zone letters for the local offset at `t` (EST / EDT / CST ...). Windows gives no names,
/// so daylight time is told by comparing with the January offset.
fn zone_abbr(t: i64) -> String {
    let Some(at) = Local.timestamp_opt(t, 0).single() else { return String::new() };
    let year = at.year();
    let jan = Local.with_ymd_and_hms(year, 1, 1, 12, 0, 0).single().map(|d| d.offset().fix().local_minus_utc()).unwrap_or(0);
    let now = at.offset().fix().local_minus_utc();
    let letter = match jan / 3600 {
        -5 => "E",
        -6 => "C",
        -7 => "M",
        -8 => "P",
        -9 => "AK",
        -10 => "H",
        -4 => "A",
        _ => return format!("UTC{:+}", now / 3600),
    };
    format!("{letter}{}T", if now != jan { "D" } else { "S" })
}

fn parse_time(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.timestamp())
        .ok()
        .or_else(|| chrono::NaiveDateTime::parse_from_str(s.trim_end_matches('Z'), "%Y-%m-%dT%H:%M:%S").ok().map(|d| d.and_utc().timestamp()))
        .or_else(|| chrono::NaiveDateTime::parse_from_str(s.trim_end_matches('Z'), "%Y-%m-%dT%H:%M").ok().map(|d| d.and_utc().timestamp()))
}

// ----------------------------------------------------------------------- home area ---

const STATE_FIPS: [(&str, &str); 52] = [
    ("AL", "01"), ("AK", "02"), ("AZ", "04"), ("AR", "05"), ("CA", "06"), ("CO", "08"), ("CT", "09"), ("DE", "10"), ("DC", "11"),
    ("FL", "12"), ("GA", "13"), ("HI", "15"), ("ID", "16"), ("IL", "17"), ("IN", "18"), ("IA", "19"), ("KS", "20"), ("KY", "21"),
    ("LA", "22"), ("ME", "23"), ("MD", "24"), ("MA", "25"), ("MI", "26"), ("MN", "27"), ("MS", "28"), ("MO", "29"), ("MT", "30"),
    ("NE", "31"), ("NV", "32"), ("NH", "33"), ("NJ", "34"), ("NM", "35"), ("NY", "36"), ("NC", "37"), ("ND", "38"), ("OH", "39"),
    ("OK", "40"), ("OR", "41"), ("PA", "42"), ("RI", "44"), ("SC", "45"), ("SD", "46"), ("TN", "47"), ("TX", "48"), ("UT", "49"),
    ("VT", "50"), ("VA", "51"), ("WA", "53"), ("WV", "54"), ("WI", "55"), ("WY", "56"), ("PR", "72"),
];

fn state_fips(abbr: &str) -> Option<&'static str> {
    STATE_FIPS.iter().find(|(a, _)| a.eq_ignore_ascii_case(abbr)).map(|(_, f)| *f)
}

/// The home point's NWS identifiers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Home {
    /// County UGC, e.g. INC037.
    pub county_ugc: String,
    /// Forecast zone, e.g. INZ045.
    pub zone: String,
    /// Forecast office, e.g. LMK.
    pub wfo: String,
    pub state: String,
    /// 5-digit county FIPS, e.g. 18037.
    pub fips: String,
    pub county_name: String,
}

pub async fn home(lat: f64, lon: f64) -> Result<Home> {
    let _ = crate::map::boundaries::ensure_loaded().await;
    let p = crate::net::get_json(&format!("https://api.weather.gov/points/{lat:.4},{lon:.4}")).await?;
    let last = |key: &str| p.pointer(&format!("/properties/{key}")).and_then(|v| v.as_str()).and_then(|u| u.rsplit('/').next()).unwrap_or_default().to_string();
    let county_ugc = last("county");
    let state = county_ugc.get(0..2).unwrap_or_default().to_string();
    let fips = state_fips(&state).map(|s| format!("{s}{}", county_ugc.get(3..6).unwrap_or_default())).unwrap_or_default();
    let county_name = crate::map::boundaries::counties_named()
        .and_then(|cs| cs.iter().find(|c| c.fips == fips).map(|c| c.name.clone()))
        .unwrap_or_default();
    Ok(Home {
        county_ugc,
        zone: last("forecastZone"),
        wfo: p.pointer("/properties/cwa").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        state,
        fips,
        county_name,
    })
}

/// Expands NWS UGC lines ("INC037-051>055-150845-") into codes ("INC037", "INC051" ...).
pub fn expand_ugc(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut prefix = String::new();
    for tok in text.split('-').map(str::trim).filter(|t| !t.is_empty()) {
        let (head, rest) = if tok.len() >= 6 && tok[..2].chars().all(|c| c.is_ascii_uppercase()) && matches!(&tok[2..3], "C" | "Z") {
            (Some(&tok[..3]), &tok[3..])
        } else {
            (None, tok)
        };
        if let Some(h) = head {
            prefix = h.to_string();
        }
        if prefix.is_empty() || rest.len() == 6 {
            continue; // expiry time (DDHHMM)
        }
        match rest.split_once('>') {
            Some((a, b)) => {
                if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>()) {
                    for n in a..=b.min(a + 300) {
                        out.push(format!("{prefix}{n:03}"));
                    }
                }
            }
            None => {
                if rest.len() == 3 && rest.chars().all(|c| c.is_ascii_digit()) {
                    out.push(format!("{prefix}{rest}"));
                }
            }
        }
    }
    out
}

/// County UGC (INC037) to FIPS (18037).
fn ugc_fips(ugc: &str) -> Option<String> {
    (ugc.get(2..3) == Some("C")).then(|| state_fips(&ugc[..2]).map(|s| format!("{s}{}", &ugc[3..]))).flatten()
}

// --------------------------------------------------------------------------- alerts ---

#[derive(Clone, Debug)]
pub enum PickSource {
    /// The NWS GeoJSON feature.
    Live(Value),
    Past { wfo: String, year: i32, phen: String, sig: String, etn: u32 },
}

/// One alert the studio can make a card for.
#[derive(Clone, Debug)]
pub struct AlertPick {
    pub event: String,
    pub issued: i64,
    pub expires: i64,
    pub source: PickSource,
}

impl AlertPick {
    pub fn label(&self) -> String {
        let when = Local.timestamp_opt(self.issued, 0).single().map(|d| d.format("%b %-d %Y, %-I:%M %p").to_string()).unwrap_or_default();
        format!("{} · {when}", self.event)
    }
    pub fn is_past(&self) -> bool {
        matches!(self.source, PickSource::Past { .. })
    }
}

/// Active NWS alerts at the home point.
pub async fn live_alerts(lat: f64, lon: f64) -> Result<Vec<AlertPick>> {
    let v = crate::net::get_json(&format!("https://api.weather.gov/alerts/active?point={lat:.4},{lon:.4}")).await?;
    let mut out: Vec<AlertPick> = v["features"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| {
            let p = &f["properties"];
            let t = |k: &str| p[k].as_str().and_then(parse_time);
            AlertPick {
                event: p["event"].as_str().unwrap_or("Alert").to_string(),
                issued: t("effective").or_else(|| t("sent")).unwrap_or_default(),
                expires: t("ends").or_else(|| t("expires")).unwrap_or_default(),
                source: PickSource::Live(f.clone()),
            }
        })
        .collect();
    out.sort_by_key(|a| crate::data::Alert { event: a.event.clone(), ..Default::default() }.priority());
    Ok(out)
}

/// Every warning, watch and advisory IEM has for the county between two dates, newest first.
pub async fn past_alerts(county_ugc: &str, from: NaiveDate, to: NaiveDate) -> Result<Vec<AlertPick>> {
    let url = format!(
        "https://mesonet.agron.iastate.edu/json/vtec_events_byugc.py?ugc={county_ugc}&sdate={}&edate={}",
        from.format("%Y-%m-%d"),
        (to + chrono::Duration::days(1)).format("%Y-%m-%d")
    );
    let v = crate::net::get_json(&url).await?;
    let mut out: Vec<AlertPick> = v["events"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let issued = parse_time(e["issue"].as_str()?)?;
            let year = Utc.timestamp_opt(issued, 0).single()?.year();
            Some(AlertPick {
                event: e["name"].as_str().unwrap_or("Alert").to_string(),
                issued,
                expires: e["expire"].as_str().and_then(parse_time).unwrap_or(issued),
                source: PickSource::Past {
                    wfo: e["wfo"].as_str()?.to_string(),
                    year,
                    phen: e["phenomena"].as_str()?.to_string(),
                    sig: e["significance"].as_str()?.to_string(),
                    etn: e["eventid"].as_u64()? as u32,
                },
            })
        })
        .collect();
    out.sort_by_key(|a| std::cmp::Reverse(a.issued));
    Ok(out)
}

/// The alert in one shape, whatever its source.
#[derive(Clone, Debug, Default)]
struct CardInput {
    event: String,
    description: String,
    instruction: String,
    nws_headline: String,
    chips: Vec<String>,
    issued: i64,
    expires: i64,
    /// Impact-based tags as NWS API parameter names (maxWindGust, tornadoDetection ...).
    params: Vec<(String, String)>,
    area: Vec<String>,
    area_text: String,
    polygons: Vec<Vec<[f64; 2]>>,
    /// Affected county FIPS.
    fips: Vec<String>,
    test: bool,
}

fn live_input(f: &Value) -> CardInput {
    let p = &f["properties"];
    let s = |k: &str| p[k].as_str().unwrap_or_default().to_string();
    let t = |k: &str| p[k].as_str().and_then(parse_time);
    let mut params = Vec::new();
    if let Some(obj) = p["parameters"].as_object() {
        for (k, v) in obj {
            if let Some(first) = v.as_array().and_then(|a| a.first()).and_then(|x| x.as_str()) {
                params.push((k.clone(), first.to_string()));
            }
        }
    }
    let area_desc = s("areaDesc");
    let parts: Vec<&str> = area_desc.split(';').map(str::trim).filter(|x| !x.is_empty()).collect();
    let states: std::collections::HashSet<&str> = parts.iter().filter_map(|x| x.split(',').nth(1).map(str::trim)).collect();
    let area = parts.iter().map(|x| drawable(if states.len() > 1 { x } else { x.split(',').next().unwrap_or(x) })).collect();
    let mut polygons = Vec::new();
    if let Some(g) = f.get("geometry").filter(|g| !g.is_null()) {
        let mut rings = Vec::new();
        crate::data::collect_rings(g, &mut rings);
        polygons = rings.into_iter().map(|r| r.into_iter().map(|(x, y)| [x, y]).collect()).collect();
    }
    let fips = p["geocode"]["SAME"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(|s| s.trim_start_matches('0').to_string())).map(|s| format!("{s:0>5}")).collect();
    let chips = ["severity", "urgency", "certainty"].iter().map(|k| s(k)).filter(|v| !v.is_empty() && v != "Unknown").collect();
    CardInput {
        event: s("event"),
        description: s("description"),
        instruction: s("instruction"),
        nws_headline: s("headline"),
        chips,
        issued: t("effective").or_else(|| t("sent")).unwrap_or_default(),
        expires: t("ends").or_else(|| t("expires")).unwrap_or_default(),
        params,
        area,
        area_text: drawable(&area_desc),
        polygons,
        fips,
        test: s("status") == "Test" || s("event").to_uppercase().starts_with("TEST"),
    }
}

/// Reads an archived NWS product (IEM) into the same shape as a live alert.
async fn past_input(pick: &AlertPick, wfo: &str, year: i32, phen: &str, sig: &str, etn: u32) -> Result<CardInput> {
    let q = format!("wfo={wfo}&year={year}&phenomena={phen}&significance={sig}&etn={etn}");
    let detail = crate::net::get_json(&format!("https://mesonet.agron.iastate.edu/json/vtec_event.py?{q}")).await?;
    let text = detail.pointer("/report/text").and_then(|v| v.as_str()).context("IEM has no text for this alert")?.replace('\r', "");
    let mut input = parse_product(&text, &pick.event);
    input.issued = pick.issued;
    input.expires = pick.expires;
    // Storm-based polygon, when the product has one.
    if let Ok(g) = crate::net::get_json(&format!("https://mesonet.agron.iastate.edu/geojson/vtec_event.py?{q}&sbw=1")).await {
        for f in g["features"].as_array().into_iter().flatten() {
            let mut rings = Vec::new();
            crate::data::collect_rings(&f["geometry"], &mut rings);
            input.polygons.extend(rings.into_iter().map(|r| r.into_iter().map(|(x, y)| [x, y]).collect::<Vec<_>>()));
        }
    }
    Ok(input)
}

/// Splits an NWS text product (warning, watch, statement) into card fields.
fn parse_product(text: &str, event: &str) -> CardInput {
    // The first segment only (a product can carry several).
    let seg = text.split("$$").next().unwrap_or(text);
    let lines: Vec<&str> = seg.lines().collect();
    // UGC block: the line(s) that start with a UGC code, up to the one ending in the expiry.
    let mut ugc = String::new();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i].trim();
        if l.len() > 6 && l[..2].chars().all(|c| c.is_ascii_uppercase()) && matches!(&l[2..3], "C" | "Z") && l[3..6].chars().all(|c| c.is_ascii_digit()) {
            while i < lines.len() {
                ugc.push_str(lines[i].trim());
                if lines[i].trim().ends_with('-') && lines[i].trim().trim_end_matches('-').rsplit('-').next().is_some_and(|t| t.len() == 6 && t.chars().all(|c| c.is_ascii_digit())) {
                    break;
                }
                i += 1;
            }
            break;
        }
        i += 1;
    }
    let codes = expand_ugc(&ugc);
    let counties: Vec<_> = crate::map::boundaries::counties_named().map(|c| c.to_vec_names()).unwrap_or_default();
    let mut fips: Vec<String> = codes.iter().filter_map(|c| ugc_fips(c)).collect();
    let mut area: Vec<String> = fips.iter().filter_map(|f| counties.iter().find(|(cf, _)| cf == f).map(|(_, n)| n.clone())).collect();
    // Zone products: the names follow the VTEC line(s) as "Dubois-Pike-Gibson-".
    if area.is_empty() {
        let after = lines.iter().skip(i + 1).skip_while(|l| l.trim().starts_with('/')).take_while(|l| l.trim().ends_with('-')).map(|l| l.trim()).collect::<String>();
        area = after.split('-').map(str::trim).filter(|n| !n.is_empty() && !n.chars().next().is_some_and(|c| c.is_ascii_digit())).map(drawable).collect();
        let states: Vec<&str> = codes.iter().filter_map(|c| c.get(..2)).collect();
        if let Some(cs) = crate::map::boundaries::counties_named() {
            for name in &area {
                if let Some(c) = cs.iter().find(|c| c.name.eq_ignore_ascii_case(name) && states.iter().any(|s| state_fips(s) == Some(c.state.as_str()))) {
                    fips.push(c.fips.clone());
                }
            }
        }
    }
    fips.sort();
    fips.dedup();

    let body = seg;
    let instruction = body
        .split_once("PRECAUTIONARY/PREPAREDNESS ACTIONS...")
        .map(|(_, rest)| rest.split("&&").next().unwrap_or(rest))
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    // Impact-based tags (the same names the NWS API uses).
    let mut params = Vec::new();
    for l in body.lines().map(str::trim) {
        let Some((k, v)) = l.split_once("...") else { continue };
        let key = match k {
            "TORNADO" => "tornadoDetection",
            "TORNADO DAMAGE THREAT" => "tornadoDamageThreat",
            "THUNDERSTORM DAMAGE THREAT" => "thunderstormDamageThreat",
            "FLASH FLOOD DAMAGE THREAT" => "flashFloodDamageThreat",
            "MAX HAIL SIZE" => "maxHailSize",
            "MAX WIND GUST" => "maxWindGust",
            _ => continue,
        };
        let mut v = v.trim().to_string();
        if key == "maxHailSize" {
            // "<.75 IN" -> "<0.75", "1.00 IN" -> "1.00"
            let under = v.starts_with('<');
            v = v.trim_start_matches(['<', '>']).trim_end_matches(" IN").trim().to_string();
            if v.starts_with('.') {
                v.insert(0, '0');
            }
            if under {
                v.insert(0, '<');
            }
        }
        params.push((key.to_string(), v));
    }
    // Description: from the first bullet or "The National Weather Service" line up to the actions.
    let start = body.find("* ").or_else(|| body.find("The National Weather Service")).or_else(|| body.find("...")).unwrap_or(0);
    let mut description = body[start..].to_string();
    for stop in ["PRECAUTIONARY/PREPAREDNESS ACTIONS...", "&&", "LAT...LON"] {
        if let Some(n) = description.find(stop) {
            description.truncate(n);
        }
    }
    CardInput {
        event: event.to_string(),
        description,
        instruction,
        nws_headline: String::new(),
        chips: vec![],
        area_text: area.join(", "),
        area,
        fips,
        params,
        ..Default::default()
    }
}

trait CountyNames {
    fn to_vec_names(&self) -> Vec<(String, String)>;
}
impl CountyNames for Vec<crate::map::boundaries::County> {
    fn to_vec_names(&self) -> Vec<(String, String)> {
        self.iter().map(|c| (c.fips.clone(), c.name.clone())).collect()
    }
}

fn title_case(s: &str) -> String {
    s.to_lowercase()
        .split(' ')
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Impact chips, strongest first (same rules as the website's alert-threat.js).
fn threat_tags(params: &[(String, String)]) -> Vec<Value> {
    let get = |k: &str| params.iter().find(|(pk, _)| pk == k).map(|(_, v)| v.trim().to_string()).filter(|v| !v.is_empty());
    let mut tags: Vec<(f32, Value)> = Vec::new();
    if let Some(t) = get("tornadoDetection").map(|t| t.to_uppercase()) {
        let obs = t.contains("OBSERVED");
        tags.push((if obs { 100.0 } else { 80.0 }, json!({ "label": "Tornado", "value": title_case(&t), "level": if obs { "danger" } else { "elevated" } })));
    }
    for (key, label, top) in [("tornadoDamageThreat", "Damage Threat", "CATASTROPHIC"), ("thunderstormDamageThreat", "Damage Threat", "DESTRUCTIVE"), ("flashFloodDamageThreat", "Flash Flood", "CATASTROPHIC")] {
        if let Some(t) = get(key).map(|t| t.to_uppercase()) {
            let high = t.contains(top);
            let rank = match key {
                "tornadoDamageThreat" => if high { 98.0 } else { 90.0 },
                "thunderstormDamageThreat" => if high { 95.0 } else { 75.0 },
                _ => if high { 92.0 } else { 72.0 },
            };
            tags.push((rank, json!({ "label": label, "value": title_case(&t), "level": if high { "danger" } else { "elevated" } })));
        }
    }
    if let Some(w) = get("maxWindGust") {
        let mph: f32 = w.split(|c: char| !c.is_ascii_digit()).next().and_then(|n| n.parse().ok()).unwrap_or(0.0);
        let value = if w.to_lowercase().contains("mph") { w.to_lowercase() } else { format!("{w} mph") };
        tags.push((60.0 + (mph / 10.0).min(9.0), json!({ "label": "Wind Gusts", "value": value, "level": if mph >= 70.0 { "danger" } else if mph >= 58.0 { "elevated" } else { "info" } })));
    }
    if let Some(h) = get("maxHailSize") {
        let under = h.starts_with('<');
        let inches: Option<f32> = h.trim_start_matches('<').trim_end_matches(|c: char| !c.is_ascii_digit()).parse().ok();
        let value = inches.map(|i| format!("{}{i:.2}\"", if under { "Under " } else { "" })).unwrap_or(h.clone());
        let i = inches.unwrap_or(0.0);
        tags.push((50.0 + (i * 4.0).min(9.0), json!({ "label": "Hail", "value": value, "level": if i >= 2.0 { "danger" } else if i >= 1.0 { "elevated" } else { "info" } })));
    }
    tags.sort_by(|a, b| b.0.total_cmp(&a.0));
    tags.into_iter().map(|t| t.1).collect()
}

/// The labelled sections NWS writes into warnings (WHAT/WHERE/WHEN/IMPACTS or HAZARD/SOURCE/IMPACT).
fn sections(description: &str) -> (String, String, String) {
    const LABELS: [&str; 9] = ["PRECAUTIONARY/PREPAREDNESS ACTIONS", "ADDITIONAL DETAILS", "IMPACTS", "IMPACT", "HAZARD", "SOURCE", "WHERE", "WHEN", "WHAT"];
    let upper = description.to_uppercase();
    // Every "LABEL..." position, in text order.
    let mut marks: Vec<(usize, &str)> = Vec::new();
    for label in LABELS {
        let pat = format!("{label}...");
        let mut from = 0;
        while let Some(n) = upper[from..].find(&pat) {
            let at = from + n;
            // A longer label already claimed this spot (IMPACTS vs IMPACT).
            if !marks.iter().any(|(m, l)| *m <= at && at < m + l.len() + 3) {
                marks.push((at, label));
            }
            from = at + pat.len();
        }
    }
    marks.sort();
    let mut get = |want: &[&str]| -> String {
        for (i, (at, label)) in marks.iter().enumerate() {
            if want.contains(label) {
                let start = at + label.len() + 3;
                let end = marks.get(i + 1).map(|m| m.0).unwrap_or(description.len());
                let v = description[start.min(end)..end].trim_end_matches(['*', ' ', '\n']).split_whitespace().collect::<Vec<_>>().join(" ");
                if !v.is_empty() {
                    return v;
                }
            }
        }
        String::new()
    };
    let hazard = Some(get(&["HAZARD"])).filter(|s| !s.is_empty()).unwrap_or_else(|| get(&["WHAT"]));
    let source = get(&["SOURCE"]);
    let impact = get(&["IMPACTS", "IMPACT"]);
    (hazard, source, impact)
}

/// Banner wording (same as the website's getBannerConfig).
fn banner_text(event: &str) -> String {
    let e = event.to_lowercase();
    let kind = if e.contains("warning") { "warning" } else if e.contains("watch") { "watch" } else { "other" };
    if e.contains("tornado") && (e.contains("emergency") || e.contains("particularly dangerous")) {
        "THIS IS A PARTICULARLY DANGEROUS SITUATION — TAKE SHELTER IMMEDIATELY".into()
    } else if e.contains("tornado warning") {
        "TORNADO WARNING IN EFFECT — TAKE COVER NOW".into()
    } else if e.contains("severe thunderstorm warning") {
        "SEVERE THUNDERSTORM WARNING — CONSIDERABLE DAMAGE THREAT".into()
    } else if e.contains("flash flood warning") {
        "FLASH FLOOD WARNING IN EFFECT".into()
    } else if e.contains("flash flood watch") {
        "FLASH FLOOD WATCH IN EFFECT".into()
    } else if e.contains("tornado watch") {
        "TORNADO WATCH IN EFFECT — CONDITIONS FAVORABLE FOR TORNADOES".into()
    } else if e.contains("severe thunderstorm watch") {
        "SEVERE THUNDERSTORM WATCH IN EFFECT".into()
    } else if e.contains("flood warning") {
        "FLOOD WARNING IN EFFECT".into()
    } else if e.contains("flood watch") {
        "FLOOD WATCH IN EFFECT".into()
    } else if kind != "other" {
        format!("{} IN EFFECT", event.to_uppercase())
    } else {
        event.to_uppercase()
    }
}

// ------------------------------------------------------------------------------ map ---

#[derive(Clone, Copy, Debug)]
struct BBox {
    west: f64,
    south: f64,
    east: f64,
    north: f64,
}

fn extent<'a>(rings: impl Iterator<Item = &'a [f64; 2]>) -> Option<BBox> {
    let mut b = BBox { west: f64::MAX, south: f64::MAX, east: f64::MIN, north: f64::MIN };
    let mut any = false;
    for p in rings {
        any = true;
        b.west = b.west.min(p[0]);
        b.east = b.east.max(p[0]);
        b.south = b.south.min(p[1]);
        b.north = b.north.max(p[1]);
    }
    any.then_some(b)
}

fn lon_x(lon: f64, n: f64) -> f64 {
    (lon + 180.0) / 360.0 * n
}
fn lat_y(lat: f64, n: f64) -> f64 {
    let r = lat.to_radians();
    (1.0 - (r.tan() + 1.0 / r.cos()).ln() / std::f64::consts::PI) / 2.0 * n
}

fn tiles_for(b: BBox, z: u8) -> Vec<(i64, i64, u8)> {
    let n = 2f64.powi(z as i32);
    let (x0, x1) = (lon_x(b.west, n).floor() as i64, lon_x(b.east, n).floor() as i64);
    let (y0, y1) = (lat_y(b.north, n).floor() as i64, lat_y(b.south, n).floor() as i64);
    let max = n as i64;
    let mut out = Vec::new();
    for x in x0..=x1 {
        for y in y0..=y1 {
            if x >= 0 && y >= 0 && x < max && y < max {
                out.push((x, y, z));
            }
        }
    }
    out
}

const RADAR_TILE: f64 = 512.0;

/// Zoom that gives roughly 1500-3000 source pixels across the box (sharp at 2x, few tiles).
fn zoom_for(b: BBox) -> u8 {
    let w = (b.east - b.west).max(0.05);
    ((2200.0 * 360.0 / (RADAR_TILE * w)).log2().floor()).clamp(5.0, 10.0) as u8
}

async fn fetch_tiles(urls: Vec<((i64, i64, u8), String)>, explicit_z: bool) -> Vec<Value> {
    use base64::Engine as _;
    use futures_util::StreamExt as _;
    // A few at a time with a retry: archive and basemap servers drop bursts, which left
    // grey / black squares in the map.
    let jobs = urls.into_iter().map(|((x, y, z), url)| async move {
        let mut bytes = None;
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(400 * attempt)).await;
            }
            if let Ok(b) = crate::net::get_bytes(&url).await {
                bytes = Some(b);
                break;
            }
        }
        let data = base64::engine::general_purpose::STANDARD.encode(bytes?);
        Some(if explicit_z { json!({ "x": x, "y": y, "z": z, "data": data }) } else { json!({ "x": x, "y": y, "data": data }) })
    });
    futures_util::stream::iter(jobs).buffer_unordered(8).filter_map(|t| async move { t }).collect().await
}

/// County / state line style for the cards, taken from the live Map Studio scene (its theme,
/// boundary opacity and county / state toggles), so the cards match what goes to air. Widths
/// are Map Studio's at county zoom (it thickens lines past zoom 7-8), turned into card design
/// pixels: Map Studio draws on a 1920-wide frame, a card's map is drawn at 1.25x that per
/// design pixel (2400 px output / 2), so a design pixel is about 0.6 Map Studio pixels.
pub fn lines_from_scene(scene: &crate::map::scene::MapScene) -> Value {
    const K: f32 = 0.6;
    let to_css = |c: tiny_skia::Color| format!("rgba({}, {}, {}, {:.3})", (c.red() * 255.0) as u8, (c.green() * 255.0) as u8, (c.blue() * 255.0) as u8, c.alpha());
    let (cc, cw, sc, sw) = match crate::maptheme::get(&scene.theme) {
        Some(t) => (
            to_css(crate::maptheme::color(&t.county_color, tiny_skia::Color::WHITE)),
            t.county_width.max(0.3) * 1.3,
            to_css(crate::maptheme::color(&t.state_color, tiny_skia::Color::WHITE)),
            t.state_width.max(0.5) * 1.2,
        ),
        // Map Studio's built-in dark-map lines.
        None => ("rgba(200, 210, 225, 0.745)".to_string(), 1.6, "rgba(235, 240, 250, 0.824)".to_string(), 2.6),
    };
    json!({
        "county_color": cc,
        "county_width": cw * K,
        "state_color": sc,
        "state_width": sw * K,
        "opacity": scene.boundary_opacity.clamp(0.05, 1.0),
        "counties": scene.counties,
        "states": scene.states,
    })
}

/// The part of `b` the renderer shows in a w x h pixel panel (it fills the panel, so one side is
/// cropped), and k = output pixels per 256-px world pixel at zoom 0.
fn visible_box(b: BBox, w: f64, h: f64) -> (BBox, f64) {
    let n = 256.0;
    let (x0, x1) = (lon_x(b.west, n), lon_x(b.east, n));
    let (y0, y1) = (lat_y(b.north, n), lat_y(b.south, n));
    let k = (w / (x1 - x0).max(1e-9)).max(h / (y1 - y0).max(1e-9));
    let (cx, cy, hx, hy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, w / k / 2.0, h / k / 2.0);
    let lon = |x: f64| x / n * 360.0 - 180.0;
    let lat = |y: f64| (std::f64::consts::PI * (1.0 - 2.0 * y / n)).sinh().atan().to_degrees();
    (BBox { west: lon(cx - hx), east: lon(cx + hx), north: lat(cy - hy), south: lat(cy + hy) }, k)
}

/// Map framed on the alert: its polygon, or its counties, never tighter than about a county.
/// The basemap comes at full detail for the design's map panel (same as the website).
async fn alert_map(input: &CardInput, home_fips: &str, api_base: &str, past_time: Option<i64>, design: &str) -> Value {
    let counties = crate::map::boundaries::counties_named();
    let all: &[crate::map::boundaries::County] = counties.as_deref().map(|v| v.as_slice()).unwrap_or(&[]);
    let affected: Vec<&crate::map::boundaries::County> = all.iter().filter(|c| input.fips.contains(&c.fips)).collect();
    let home = all.iter().find(|c| c.fips == home_fips);
    let polys = !input.polygons.is_empty();
    let mut b = if polys {
        extent(input.polygons.iter().flatten())
    } else if !affected.is_empty() {
        let pts: Vec<[f64; 2]> = affected.iter().flat_map(|c| c.rings.iter().flatten().map(|&(x, y)| [x, y])).collect();
        extent(pts.iter())
    } else {
        let pts: Vec<[f64; 2]> = home.map(|c| c.rings.iter().flatten().map(|&(x, y)| [x, y]).collect()).unwrap_or_default();
        extent(pts.iter())
    }
    .unwrap_or(BBox { west: -87.3, south: 38.1, east: -86.5, north: 38.7 });
    let (cx, cy) = ((b.west + b.east) / 2.0, (b.south + b.north) / 2.0);
    let (hw, hh) = ((b.east - b.west).max(0.55) / 2.0, (b.north - b.south).max(0.385) / 2.0);
    b = BBox { west: cx - hw, east: cx + hw, south: cy - hh, north: cy + hh };
    // The website's padding: 15%, then 35% more so neighbouring counties and names show.
    for pad in [0.15, 0.35] {
        let (px, py) = ((b.east - b.west) * pad, (b.north - b.south) * pad);
        b = BBox { west: b.west - px, east: b.east + px, south: b.south - py, north: b.north + py };
    }
    let (pw, ph) = social_render::alert_map_size(design);
    let (vis, k) = visible_box(b, pw as f64 * 2.0, ph as f64 * 2.0);
    b = vis;
    let zoom = zoom_for(b);
    // Never upscaled: each output pixel gets at least one basemap pixel.
    let base_zoom = (k.log2() - 1e-9).ceil().clamp(3.0, 16.0) as u8;

    let near_pt = |&(x, y): &(f64, f64)| x > b.west - 0.3 && x < b.east + 0.3 && y > b.south - 0.3 && y < b.north + 0.3;
    let near = |c: &crate::map::boundaries::County| c.rings.iter().flatten().any(near_pt);
    let shown: Vec<&crate::map::boundaries::County> = all.iter().filter(|c| near(c)).collect();
    // Names that appear twice in view (Crawford IN / Crawford IL) are named on the home side only.
    let home_state = home_fips.get(..2).unwrap_or("");
    let dupes = |name: &str| shown.iter().filter(|c| c.name == name).count() > 1;
    let county_json: Vec<Value> = shown
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "active": c.fips == home_fips,
                "affected": !polys && input.fips.contains(&c.fips),
                "unlabeled": dupes(&c.name) && c.fips.get(..2).unwrap_or("") != home_state,
                "rings": c.rings.iter().map(|r| r.iter().map(|&(x, y)| [x, y]).collect::<Vec<_>>()).collect::<Vec<_>>(),
            })
        })
        .collect();
    let states: Vec<Vec<[f64; 2]>> = crate::map::boundaries::state_rings()
        .into_iter()
        .filter(|r| r.iter().any(near_pt))
        .map(|r| r.into_iter().map(|(x, y)| [x, y]).collect())
        .collect();

    let basemap_urls = tiles_for(b, base_zoom)
        .into_iter()
        .map(|t| (t, format!("https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Dark_Gray_Base/MapServer/tile/{}/{}/{}", t.2, t.1, t.0)))
        .collect();
    let (radar, badge) = match past_time {
        // IEM composite archive: 256 px tiles one zoom up (same detail as 512 px at `zoom`).
        Some(t) => {
            let tpl = crate::history::radar_tiles(t);
            let urls = tiles_for(b, zoom + 1).into_iter().map(|tl| (tl, tpl.replace("{z}", &tl.2.to_string()).replace("{x}", &tl.0.to_string()).replace("{y}", &tl.1.to_string()))).collect();
            let tiles = fetch_tiles(urls, true).await;
            let badge = if tiles.is_empty() { "Radar archive unavailable".to_string() } else { format!("Radar • IEM archive • {}", clock_time(t - t.rem_euclid(300))) };
            (tiles, badge)
        }
        None => match crate::net::radar_index(api_base.to_string()).await.ok().and_then(|i| i.radar.past.last().map(|f| f.time)) {
            Some(time) => {
                let base = api_base.trim_end_matches('/');
                let urls = tiles_for(b, zoom).into_iter().map(|tl| (tl, format!("{base}/api/radar?path=v2/radar/{time}/512/{}/{}/{}/4/1_1.webp", tl.2, tl.0, tl.1))).collect();
                let tiles = fetch_tiles(urls, false).await;
                let stale = Utc::now().timestamp() - time > 15 * 60;
                (tiles, format!("Radar • LibreWXR • {}{}", clock_time(time), if stale { " • delayed" } else { "" }))
            }
            None => (vec![], "Radar unavailable".to_string()),
        },
    };
    let basemap = fetch_tiles(basemap_urls, true).await;
    json!({
        "zoom": zoom,
        "tile_size": RADAR_TILE,
        "bbox": [b.west, b.south, b.east, b.north],
        "basemap": basemap,
        "radar": radar,
        "counties": county_json,
        "states": states,
        "polygons": input.polygons.iter().map(|r| vec![r.clone()]).collect::<Vec<_>>(),
        "regional": !polys && affected.len() > 1,
        "badge": badge,
    })
}

// ---------------------------------------------------------------------------- brand ---

/// What every card carries about the station (from Settings; nothing hard-coded).
#[derive(Clone, Debug, Default)]
pub struct Station {
    pub name: String,
    pub region: String,
    pub site: String,
    pub logo: Option<String>,
    /// Coverage area region name (Settings > Coverage area); "" = "<home> & surrounding counties".
    pub area: String,
}

fn media_b64(name: Option<&str>) -> Option<String> {
    use base64::Engine as _;
    let name = name.map(str::trim).filter(|n| !n.is_empty())?;
    let bytes = std::fs::read(crate::config::LibraryPaths::get().media.join(name)).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(bytes))
}

impl Station {
    pub fn from_settings(s: &crate::config::Settings) -> Self {
        let site = s.api_base.trim().trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("www.").trim_end_matches('/').to_string();
        Station {
            name: s.station_name.clone(),
            region: s.home_label.clone(),
            site,
            logo: media_b64(s.brand.logo_media.as_deref()),
            area: crate::coverage::region_name(s).unwrap_or_default().to_string(),
        }
    }

    /// The area line on outlook cards.
    pub fn area_line(&self) -> String {
        if self.area.trim().is_empty() { outlook_area(&self.region) } else { self.area.trim().to_string() }
    }
}

/// A sponsor block on the card.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SponsorPick {
    pub label: String,
    pub name: String,
    pub logo_media: Option<String>,
}

pub fn sponsor_json(s: &SponsorPick) -> Value {
    json!({ "label": drawable(&s.label), "name": drawable(&s.name), "logo": media_b64(s.logo_media.as_deref()) })
}

/// Builds the alert card job (fetches the map).
/// `lines`: county / state line style ([`lines_from_scene`]), or Null for the card's own.
pub async fn alert_job(pick: AlertPick, station: Station, home_fips: String, api_base: String, design: String, sponsor: Option<SponsorPick>, lines: Value) -> Result<Value> {
    let input = match &pick.source {
        PickSource::Live(f) => live_input(f),
        PickSource::Past { wfo, year, phen, sig, etn } => past_input(&pick, wfo, *year, phen, sig, *etn).await?,
    };
    let past_time = pick.is_past().then_some(input.issued);
    let mut map = alert_map(&input, &home_fips, &api_base, past_time, &design).await;
    map["lines"] = lines;
    let kind_color = crate::data::alert_color(&input.event);
    let (hazard, source, impact) = sections(&input.description);
    let mut rows = Vec::new();
    for (kind, label, value) in [("hazard", "Hazard", &hazard), ("source", "Source", &source), ("impact", "Impact", &impact)] {
        if !value.is_empty() {
            rows.push(json!({ "kind": kind, "label": label, "value": drawable(value) }));
        }
    }
    if rows.is_empty() {
        if !input.nws_headline.is_empty() {
            rows.push(json!({ "kind": "details", "label": "Details", "value": drawable(&input.nws_headline) }));
        }
        if !input.instruction.is_empty() {
            rows.push(json!({ "kind": "action", "label": "Action", "value": drawable(&input.instruction) }));
        }
    }
    let sub = if station.region.is_empty() { String::new() } else { station.region.clone() };
    Ok(json!({
        "kind": "alert",
        "scale": 2,
        "design": design,
        "event": drawable(&input.event),
        "headline": drawable(&banner_text(&input.event)),
        "banner_color": kind_color,
        "kind_color": if input.test { "#94a3b8" } else { kind_color },
        "chips": input.chips,
        "until": if input.expires > 0 { card_time(input.expires) } else { String::new() },
        "issued": if input.issued > 0 { card_time(input.issued) } else { String::new() },
        "tags": threat_tags(&input.params),
        "rows": rows,
        "area": input.area,
        "area_text": input.area_text,
        "brand": { "name": station.name, "sub": sub, "site": station.site, "logo": station.logo },
        "sponsor": sponsor.as_ref().map(sponsor_json),
        "map": map,
        "test": input.test,
    }))
}

// ------------------------------------------------------------------------- forecast ---

/// One day card.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Day {
    pub label: String,
    pub date: NaiveDate,
    pub description: String,
    pub high: Option<i32>,
    pub low: Option<i32>,
    /// Chance of rain, percent.
    pub rain: Option<i32>,
}

/// A forecast ready to draw.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForecastData {
    pub eyebrow: String,
    pub headline: String,
    pub description: String,
    pub days: Vec<Day>,
    /// Where it came from, for the studio ("NWS forecast, 7:12 PM" / "Archived zone forecast ...").
    pub note: String,
    /// The regional city table, when the scope is Regional or Coverage.
    pub regional: Option<Regional>,
    /// The coverage town this forecast is for; "" = home.
    pub place: String,
}

/// A coverage town: the supported counties (or home), then each extra alert spot from Settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub home: bool,
    /// Supported county FIPS ("" for home without counties, watched spots and regions).
    pub fips: String,
    /// The name is a stand-in (county name) until the NWS city nearest the centre is known.
    pub auto_name: bool,
}

/// The supported counties (Settings > Coverage area, home county first, each at its centre
/// point and labelled with its town name or the NWS city nearest the centre) — or, with none
/// set, home ("Dubois County" from "Dubois County, IN") — then the watched spots (Settings >
/// Alerts) and the forecaster's published forecast regions (Map Studio), the same places the
/// Graphics Studio's Location picker offers. Names are unique; at most 8.
pub fn coverage_places(s: &crate::config::Settings, regions: &[crate::forecast_edit::Region]) -> Vec<Place> {
    let mut out: Vec<Place> = Vec::new();
    for c in crate::coverage::county_places(s) {
        if out.len() >= 8 {
            break;
        }
        let taken = |n: &str| out.iter().any(|p: &Place| p.name.eq_ignore_ascii_case(n));
        let name = if taken(&c.label) { format!("{} ({})", c.label, c.fips) } else { c.label };
        out.push(Place { name, lat: c.lat, lon: c.lon, home: c.home, fips: c.fips, auto_name: c.auto });
    }
    if out.is_empty() {
        let home = s.home_label.split(", ").next().unwrap_or("").trim();
        out.push(Place { name: if home.is_empty() { "Home".into() } else { home.to_string() }, lat: s.home.lat, lon: s.home.lon, home: true, fips: String::new(), auto_name: false });
    }
    let extra = s
        .alert_zones
        .iter()
        .map(|z| (z.name.trim().to_string(), z.lat, z.lon))
        .chain(regions.iter().map(|r| {
            let (lat, lon) = r.shape.center();
            (r.name.trim().to_string(), lat, lon)
        }));
    for (name, lat, lon) in extra {
        if !name.is_empty() && !out.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) && out.len() < 8 {
            out.push(Place { name, lat, lon, home: false, fips: String::new(), auto_name: false });
        }
    }
    out
}

/// The coverage towns' live NWS point forecasts as a city table (columns from the first town).
pub async fn coverage_forecast(places: Vec<Place>) -> Result<Regional> {
    let rows = futures_util::future::join_all(places.into_iter().map(|mut p| async move {
        let f = live_forecast_at(p.lat, p.lon).await.map(|(f, city)| {
            // A supported county without its own town name shows the NWS city nearest its
            // centre (as the website does); remember it for the town chips.
            if let Some(city) = city.filter(|c| p.auto_name && !p.fips.is_empty() && !c.trim().is_empty()) {
                crate::coverage::remember_town(&p.fips, city.trim());
                p.name = city.trim().to_string();
            }
            f
        });
        (p, f)
    }))
    .await;
    let mut columns: Vec<(String, String)> = Vec::new();
    let mut dates: Vec<NaiveDate> = Vec::new();
    let mut cities = Vec::new();
    let mut failed = Vec::new();
    for (p, f) in rows {
        match f {
            Ok(f) => {
                if dates.is_empty() {
                    dates = f.days.iter().map(|d| d.date).collect();
                    columns = f.days.iter().map(|d| (short_day_label(&d.label), d.date.format("%b %-d").to_string())).collect();
                }
                let days = dates
                    .iter()
                    .map(|date| f.days.iter().find(|d| d.date == *date).cloned().unwrap_or(Day { label: String::new(), date: *date, description: String::new(), high: None, low: None, rain: None }))
                    .collect();
                cities.push((p.name, days));
            }
            Err(e) => failed.push(format!("{} ({e})", p.name)),
        }
    }
    if cities.is_empty() {
        anyhow::bail!("no coverage forecasts could be read: {}", failed.join(", "));
    }
    Ok(Regional { section: "COVERAGE AREA".into(), sections: Vec::new(), office: String::new(), columns, cities })
}

/// One section of the NWS Tabular State Forecast (SFT): cities across part of the home state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Regional {
    /// "SOUTHERN INDIANA"
    pub section: String,
    pub sections: Vec<String>,
    /// Issuing office ("IND").
    pub office: String,
    /// Column headings: ("Today", "Nov 15").
    pub columns: Vec<(String, String)>,
    pub cities: Vec<(String, Vec<Day>)>,
}

/// SFT weather word -> words shown on the card.
fn sft_words(code: &str) -> String {
    let c = code.to_lowercase();
    let w = match c.as_str() {
        "sunny" => "Sunny",
        "clear" => "Clear",
        "mosunny" => "Mostly sunny",
        "ptcldy" => "Partly cloudy",
        "mocldy" => "Mostly cloudy",
        "cloudy" => "Cloudy",
        "rain" => "Rain",
        "showers" => "Showers",
        "tstrms" => "Thunderstorms",
        "snow" => "Snow",
        "snoshwr" => "Snow showers",
        "flurries" => "Flurries",
        "rnsnow" => "Rain and snow",
        "sleet" => "Sleet",
        "frzrain" => "Freezing rain",
        "frzdrzl" => "Freezing drizzle",
        "drizzle" => "Drizzle",
        "fog" => "Fog",
        "windy" | "wndy" => "Windy",
        "hot" => "Hot",
        "cold" => "Cold",
        "blzzrd" => "Blizzard",
        "smoke" => "Smoke",
        "haze" => "Haze",
        "dust" => "Dust",
        _ => return title_case(&c),
    };
    w.to_string()
}

/// Parses an SFT product into (columns, sections of (name, cities)).
pub fn parse_sft(text: &str) -> (Vec<(String, String)>, Vec<(String, Vec<(String, Vec<Day>)>)>) {
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
    let Some(start) = lines.iter().position(|l| {
        let w: Vec<&str> = l.split_whitespace().collect();
        w.len() > 1 && w.iter().all(|x| *x == "FCST")
    }) else {
        return (vec![], vec![]);
    };
    let labels: Vec<&str> = lines.get(start + 1).map(|l| l.split_whitespace().collect()).unwrap_or_default();
    let date_words: Vec<&str> = lines.get(start + 2).map(|l| l.split_whitespace().collect()).unwrap_or_default();
    let columns: Vec<(String, String)> = labels.iter().enumerate().map(|(i, l)| (short_day_label(l), date_words.get(i * 2..i * 2 + 2).map(|d| d.join(" ")).unwrap_or_default())).collect();
    let num = |s: &str| s.trim().parse::<i32>().ok();
    let mut sections: Vec<(String, Vec<(String, Vec<Day>)>)> = Vec::new();
    let mut i = start + 3;
    while i < lines.len() {
        let l = lines[i];
        if l.starts_with("$$") {
            break;
        }
        let t = l.trim();
        if t.starts_with("...") && t.ends_with("...") && t.len() > 6 {
            sections.push((t.trim_matches('.').trim().to_string(), Vec::new()));
            i += 1;
            continue;
        }
        let is_city = l.starts_with("  ") && !t.is_empty() && !t.chars().any(|c| c.is_ascii_digit()) && lines.get(i + 2).is_some_and(|n| n.contains('/'));
        if let (true, Some(section)) = (is_city, sections.last_mut()) {
            let name = t.rsplit_once(',').filter(|(_, st)| st.trim().len() == 2).map(|(n, _)| n.trim()).unwrap_or(t).to_string();
            let words: Vec<&str> = lines[i + 1].split_whitespace().collect();
            let temps: Vec<&str> = lines[i + 2].split_whitespace().collect();
            let pops: Vec<&str> = lines.get(i + 3).filter(|n| n.contains('/')).map(|n| n.split_whitespace().collect()).unwrap_or_default();
            let days = (0..columns.len())
                .map(|n| {
                    let (lo, hi) = temps.get(n).and_then(|t| t.split_once('/')).unwrap_or(("", ""));
                    let rain = pops.get(n).and_then(|p| p.split_once('/')).map(|(a, b)| num(a).into_iter().chain(num(b)).max()).flatten();
                    Day { label: String::new(), date: NaiveDate::default(), description: sft_words(words.get(n).copied().unwrap_or("")), high: num(hi), low: num(lo), rain }
                })
                .collect();
            section.1.push((name, days));
            i += if pops.is_empty() { 3 } else { 4 };
            continue;
        }
        i += 1;
    }
    (columns, sections)
}

/// The SFT section that best matches where the home county sits in its state (NORTHERN /
/// CENTRAL / SOUTHEAST ...), or the named one.
fn pick_section<'a>(sections: &'a [(String, Vec<(String, Vec<Day>)>)], home_fips: &str, wanted: &str) -> Option<&'a (String, Vec<(String, Vec<Day>)>)> {
    if let Some(s) = sections.iter().find(|s| !wanted.is_empty() && s.0.eq_ignore_ascii_case(wanted)) {
        return Some(s);
    }
    let counties = crate::map::boundaries::counties_named();
    let all = counties.as_deref().map(|v| v.as_slice()).unwrap_or(&[]);
    let st = home_fips.get(..2).unwrap_or("");
    let pts = |c: &crate::map::boundaries::County| c.rings.iter().flatten().map(|&(x, y)| [x, y]).collect::<Vec<_>>();
    let state_pts: Vec<[f64; 2]> = all.iter().filter(|c| c.fips.get(..2) == Some(st)).flat_map(pts).collect();
    let home_pts: Vec<[f64; 2]> = all.iter().filter(|c| c.fips == home_fips).flat_map(pts).collect();
    let (Some(sb), Some(hb)) = (extent(state_pts.iter()), extent(home_pts.iter())) else { return sections.first() };
    let fx = ((hb.west + hb.east) / 2.0 - sb.west) / (sb.east - sb.west).max(1e-6);
    let fy = ((hb.south + hb.north) / 2.0 - sb.south) / (sb.north - sb.south).max(1e-6);
    let score = |name: &str| {
        let n = name.to_uppercase();
        let y = if n.contains("NORTH") { 0.83 } else if n.contains("SOUTH") { 0.17 } else { 0.5 };
        let x = if n.contains("EAST") { 0.83 } else if n.contains("WEST") { 0.17 } else { 0.5 };
        (x - fx).powi(2) + (y - fy).powi(2)
    };
    sections.iter().min_by(|a, b| score(&a.0).total_cmp(&score(&b.0)))
}

/// The home state's NWS Tabular State Forecast: the latest one (live), or the last one IEM
/// archived before 7 AM local on `date` (past). `section` "" = picked from the home position.
pub async fn regional_forecast(home: &Home, date: Option<NaiveDate>, section: &str, home_city: &str) -> Result<Regional> {
    anyhow::ensure!(home.state.len() == 2, "the home state is unknown");
    // Live: the NWS API; if that fails (e.g. a relay that doesn't pass /products yet), the
    // newest copy IEM archived, which is the same product within a minute or two.
    let live = async {
        let list = crate::net::get_json(&format!("https://api.weather.gov/products/types/SFT/locations/{}", home.state)).await?;
        let latest = list["@graph"].get(0).context("this state has no NWS Tabular State Forecast")?;
        let id = latest["id"].as_str().context("no product id")?;
        let p = crate::net::get_json(&format!("https://api.weather.gov/products/{id}")).await?;
        anyhow::Ok((p["productText"].as_str().unwrap_or_default().to_string(), latest["issuingOffice"].as_str().unwrap_or("").trim_start_matches('K').to_string()))
    };
    let live_result = if date.is_none() { Some(live.await) } else { None };
    let (text, office) = match (date, live_result) {
        (None, Some(Ok(found))) => found,
        (None, _) => {
            let today = Utc::now().date_naive();
            let mut found = None;
            for day in [today, today - chrono::Duration::days(1)] {
                let list = crate::net::get_json(&format!("https://mesonet.agron.iastate.edu/api/1/nws/afos/list.json?pil=SFT{}&date={}", home.state, day.format("%Y-%m-%d"))).await?;
                let newest = list["data"].as_array().into_iter().flatten().filter_map(|r| Some((parse_time(r["entered"].as_str()?)?, r["product_id"].as_str()?.to_string()))).max();
                if let Some((_, id)) = newest {
                    found = Some(id);
                    break;
                }
            }
            let id = found.context("no state forecast found (NWS or IEM)")?;
            let office = id.split('-').nth(1).unwrap_or("").trim_start_matches('K').to_string();
            (String::from_utf8(crate::net::get_bytes(&format!("https://mesonet.agron.iastate.edu/api/1/nwstext/{id}")).await?)?, office)
        }
        (Some(day), _) => {
            let list = crate::net::get_json(&format!("https://mesonet.agron.iastate.edu/api/1/nws/afos/list.json?pil=SFT{}&date={}", home.state, day.format("%Y-%m-%d"))).await?;
            let mut products: Vec<(i64, String, String)> = list["data"]
                .as_array()
                .into_iter()
                .flatten()
                // product_id: 202611150913-KIND-FPUS63-SFTIN (office is the second part).
                .filter_map(|r| {
                    let id = r["product_id"].as_str()?.to_string();
                    let office = id.split('-').nth(1).unwrap_or("").to_string();
                    Some((parse_time(r["entered"].as_str()?)?, id, office))
                })
                .collect();
            products.sort();
            let morning = Local.from_local_datetime(&day.and_hms_opt(7, 0, 0).unwrap()).single().map(|d| d.timestamp()).unwrap_or(0);
            let (_, id, src) = products.iter().rev().find(|(t, _, _)| *t <= morning).or_else(|| products.first()).cloned().context("IEM has no state forecast for that day")?;
            let text = String::from_utf8(crate::net::get_bytes(&format!("https://mesonet.agron.iastate.edu/api/1/nwstext/{id}")).await?)?;
            (text, src.trim_start_matches('K').to_string())
        }
    };
    let (columns, sections) = parse_sft(&text);
    let chosen = pick_section(&sections, &home.fips, section).context("the state forecast has no sections")?;
    let mut cities = chosen.1.clone();
    cities.sort_by_key(|(n, _)| !n.eq_ignore_ascii_case(home_city));
    Ok(Regional { section: chosen.0.clone(), sections: sections.iter().map(|s| s.0.clone()).collect(), office, columns, cities })
}

/// Remix icon for a short forecast (the website's forecastIcon, filled set).
pub fn icon_for(summary: &str) -> &'static str {
    let s = summary.to_lowercase();
    let has = |w: &str| s.contains(w);
    if has("mostly sunny") || has("partly sunny") || has("partly cloudy") {
        return "sun-cloudy-fill";
    }
    if has("tornado") {
        "tornado-fill"
    } else if has("snow") || has("sleet") || has("ice") || has("freezing rain") || has("flurr") {
        "snowy-fill"
    } else if has("thunder") {
        "thunderstorms-fill"
    } else if has("rain") || has("shower") {
        "rainy-fill"
    } else if has("drizzle") {
        "drizzle-fill"
    } else if has("fog") || has("mist") {
        "foggy-fill"
    } else if has("wind") || has("breezy") || has("gust") {
        "windy-fill"
    } else if has("smoke") || has("haze") {
        "haze-2-fill"
    } else if has("mostly clear") {
        "sun-fill"
    } else if has("mostly cloudy") || has("overcast") || has("fair") || has("cloud") {
        "cloudy-2-fill"
    } else if has("sunny") || has("clear") {
        "sun-fill"
    } else {
        "cloudy-fill"
    }
}

/// "Monday" -> "Mon", "Monday Night" -> "Mon Night".
pub fn short_day_label(label: &str) -> String {
    const DAYS: [(&str, &str); 9] = [("monday", "Mon"), ("tuesday", "Tue"), ("wednesday", "Wed"), ("thursday", "Thu"), ("friday", "Fri"), ("saturday", "Sat"), ("sunday", "Sun"), ("tonight", "Tonight"), ("today", "Today")];
    let l = label.trim();
    let lower = l.to_lowercase();
    if let Some((_, a)) = DAYS.iter().find(|(f, _)| *f == lower) {
        return a.to_string();
    }
    for (full, abbr) in DAYS {
        if lower.starts_with(full) && lower.ends_with("night") {
            return format!("{abbr} Night");
        }
    }
    l.to_string()
}

fn holiday(date: NaiveDate) -> Option<(&'static str, &'static str)> {
    match (date.month(), date.day()) {
        (7, 4) => Some(("4th of July", "flag-fill")),
        (12, 25) => Some(("Christmas", "gift-fill")),
        (1, 1) => Some(("New Year's", "sparkling-fill")),
        _ => None,
    }
}

/// Today's NWS 7-day forecast for the home point.
pub async fn live_forecast(lat: f64, lon: f64) -> Result<ForecastData> {
    live_forecast_at(lat, lon).await.map(|(f, _)| f)
}

/// [`live_forecast`] plus the NWS relativeLocation city for the point.
pub async fn live_forecast_at(lat: f64, lon: f64) -> Result<(ForecastData, Option<String>)> {
    let p = crate::net::get_json(&format!("https://api.weather.gov/points/{lat:.4},{lon:.4}")).await?;
    let city = p.pointer("/properties/relativeLocation/properties/city").and_then(|v| v.as_str()).map(str::to_string);
    let url = p.pointer("/properties/forecast").and_then(|v| v.as_str()).context("no NWS forecast for home")?;
    let f = crate::net::get_json(url).await?;
    let periods: Vec<crate::data::Period> = serde_json::from_value(f.pointer("/properties/periods").cloned().unwrap_or_default()).unwrap_or_default();
    let first = periods.first().context("the NWS forecast is empty")?;
    let mut days: Vec<Day> = Vec::new();
    for (i, per) in periods.iter().enumerate() {
        let date = per.start_time.as_deref().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|d| d.date_naive()).unwrap_or_default();
        if per.is_daytime {
            let night = periods.get(i + 1).filter(|n| !n.is_daytime);
            let low = night.and_then(|n| n.temperature).map(|t| t.round() as i32);
            // Chance of rain: the higher of the day and the night after it.
            let pop = |n: usize| f.pointer(&format!("/properties/periods/{n}/probabilityOfPrecipitation/value")).and_then(|v| v.as_f64()).map(|v| v.round() as i32);
            let rain = [Some(i), night.map(|_| i + 1)].into_iter().flatten().filter_map(pop).max();
            days.push(Day { label: per.name.clone(), date, description: per.short_forecast.clone(), high: per.temperature.map(|t| t.round() as i32), low, rain });
        }
    }
    days.truncate(7);
    Ok((
        ForecastData {
            eyebrow: "DAILY OUTLOOK".into(),
            headline: format!("{} OUTLOOK", first.name.to_uppercase()),
            description: first.detailed_forecast.clone(),
            days,
            note: format!("NWS forecast · {}", clock_time(Utc::now().timestamp())),
            regional: None,
            place: String::new(),
        },
        city,
    ))
}

/// The zone forecast (ZFP) the home office issued on the morning of `date`, from IEM's archive.
/// Temperatures come from the wording: "upper 70s" counts as 78, "mid 50s" as 55, "around 40" as 40.
pub async fn past_forecast(home: &Home, date: NaiveDate) -> Result<ForecastData> {
    anyhow::ensure!(!home.wfo.is_empty() && !home.zone.is_empty(), "the home forecast zone is unknown");
    let list = crate::net::get_json(&format!("https://mesonet.agron.iastate.edu/api/1/nws/afos/list.json?pil=ZFP{}&date={}", home.wfo, date.format("%Y-%m-%d"))).await?;
    let mut products: Vec<(i64, String)> = list["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| Some((parse_time(r["entered"].as_str()?)?, r["product_id"].as_str()?.to_string())))
        .collect();
    products.sort();
    // The last one out before 7 AM local (the morning forecast), else the first of the day.
    let morning = Local.from_local_datetime(&date.and_hms_opt(7, 0, 0).unwrap()).single().map(|d| d.timestamp()).unwrap_or(0);
    let (issued, id) = products.iter().rev().find(|(t, _)| *t <= morning).or_else(|| products.first()).cloned().context("IEM has no zone forecast for that day")?;
    let text = String::from_utf8(crate::net::get_bytes(&format!("https://mesonet.agron.iastate.edu/api/1/nwstext/{id}")).await?)?.replace('\r', "");
    let issued_date = Local.timestamp_opt(issued, 0).single().map(|d| d.date_naive()).unwrap_or(date);
    let mut fc = parse_zfp(&text, &home.zone, issued_date).context("the zone forecast has no section for the home zone")?;
    fc.eyebrow = format!("FORECAST ISSUED {}", Local.timestamp_opt(issued, 0).single().map(|d| d.format("%b %-d, %Y").to_string().to_uppercase()).unwrap_or_default());
    fc.note = format!("Archived NWS zone forecast {id} (IEM) · highs and lows read from its wording");
    Ok(fc)
}

fn parse_zfp(text: &str, zone: &str, issued: NaiveDate) -> Option<ForecastData> {
    let seg = text.split("$$").find(|seg| {
        let ugc: String = seg.lines().map(str::trim).skip_while(|l| !(l.len() > 6 && l.chars().take(2).all(|c| c.is_ascii_uppercase()) && l.get(2..3) == Some("Z"))).take_while(|l| !l.is_empty() && l.ends_with('-')).collect();
        expand_ugc(&ugc).iter().any(|z| z == zone)
    })?;
    // ".TONIGHT...Partly cloudy ..." periods.
    let mut periods: Vec<(String, String)> = Vec::new();
    for line in seg.lines() {
        let l = line.trim_end();
        if let Some(rest) = l.strip_prefix('.') {
            if let Some((name, body)) = rest.split_once("...") {
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_uppercase() || c == ' ') {
                    periods.push((title_case(name), body.trim().to_string()));
                    continue;
                }
            }
        }
        if let Some(last) = periods.last_mut() {
            if !l.trim().is_empty() && !l.starts_with("&&") {
                last.1.push(' ');
                last.1.push_str(l.trim());
            }
        }
    }
    let first = periods.first()?.clone();
    let mut days = Vec::new();
    for (i, (name, body)) in periods.iter().enumerate() {
        let lower = name.to_lowercase();
        if lower.contains("night") {
            continue;
        }
        let date = day_date(&lower, issued);
        let low = periods.get(i + 1).filter(|(n, _)| n.to_lowercase().contains("night")).and_then(|(_, b)| temp_from_words(b, "Lows"));
        // "Chance of rain 40 percent" in the archived wording.
        let rain = body.find(" percent").and_then(|at| body[..at].rsplit(' ').next()).and_then(|n| n.parse::<i32>().ok());
        days.push(Day { label: name.clone(), date, description: short_from_text(body), high: temp_from_words(body, "Highs"), low, rain });
    }
    days.truncate(7);
    Some(ForecastData { eyebrow: String::new(), headline: format!("{} OUTLOOK", first.0.to_uppercase()), description: first.1, days, note: String::new(), regional: None, place: String::new() })
}

fn day_date(name: &str, issued: NaiveDate) -> NaiveDate {
    const NAMES: [&str; 7] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];
    match NAMES.iter().position(|d| name.starts_with(d)) {
        Some(wd) => {
            let from = issued.weekday().num_days_from_monday() as i64;
            let ahead = (wd as i64 - from).rem_euclid(7);
            issued + chrono::Duration::days(if ahead == 0 { 7 } else { ahead })
        }
        None => issued, // Today, This Afternoon, Rest Of Today
    }
}

/// The sky / precipitation sentence of a forecast period ("Mostly sunny"), not "Breezy." or "Colder.".
fn short_from_text(body: &str) -> String {
    const WORDS: [&str; 16] = ["sunny", "cloudy", "clear", "rain", "shower", "thunder", "snow", "sleet", "freezing", "drizzle", "fog", "sprinkle", "flurr", "overcast", "fair", "ice"];
    let sentences: Vec<&str> = body.split(". ").map(|s| s.trim().trim_end_matches('.')).filter(|s| !s.is_empty()).collect();
    let pick = sentences.iter().find(|s| WORDS.iter().any(|w| s.to_lowercase().contains(w))).or(sentences.first()).copied().unwrap_or_default();
    let mut s = pick.to_string();
    for cut in [", then ", " in the morning", " in the afternoon", " in the evening", " after midnight", " until "] {
        if let Some(n) = s.find(cut) {
            if n > 6 {
                s.truncate(n);
            }
        }
    }
    s
}

/// "Highs in the upper 70s" -> 78, "Lows around 40" -> 40, "Highs 55 to 60" -> 58,
/// "Lows 5 below zero" -> -5, "in the lower to mid 70s" -> 73.
pub fn temp_from_words(body: &str, key: &str) -> Option<i32> {
    let at = body.find(&format!("{key} "))?;
    let phrase: String = body[at + key.len()..].split('.').next().unwrap_or("").to_lowercase();
    let words: Vec<&str> = phrase.split(|c: char| c.is_whitespace() || c == ',').filter(|w| !w.is_empty()).collect();
    let mut values: Vec<f64> = Vec::new();
    let mut quals: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        match w {
            "lower" | "low" | "mid" | "middle" | "upper" | "high" => quals.push(w),
            "teens" => {
                values.push(qualify(&quals, 10.0, 13.0, 15.0, 18.0));
                quals.clear();
            }
            "single" if words.get(i + 1).is_some_and(|n| n.starts_with("digit")) => {
                values.push(qualify(&quals, 0.0, 2.0, 5.0, 8.0));
                quals.clear();
                i += 1;
            }
            "zero" if values.is_empty() => values.push(0.0),
            "below" if words.get(i + 1) == Some(&"zero") => {
                if let Some(v) = values.last_mut() {
                    *v = -v.abs();
                }
                i += 1;
            }
            "with" | "and" | "but" => break,
            _ => {
                let digits: String = w.chars().filter(|c| c.is_ascii_digit() || *c == '-').collect();
                if let Ok(n) = digits.parse::<f64>() {
                    if w.ends_with('s') && n % 10.0 == 0.0 {
                        values.push(qualify(&quals, n, n + 2.0, n + 5.0, n + 8.0));
                    } else {
                        values.push(n);
                    }
                    quals.clear();
                }
            }
        }
        i += 1;
        if values.len() >= 2 {
            break;
        }
    }
    (!values.is_empty()).then(|| (values.iter().sum::<f64>() / values.len() as f64).round() as i32)
}

/// Decade + qualifier: "lower" +2, "mid" +5, "upper" +8, "lower to mid" +3.5, none +5.
fn qualify(quals: &[&str], _base: f64, lower: f64, mid: f64, upper: f64) -> f64 {
    let v: Vec<f64> = quals
        .iter()
        .map(|q| match *q {
            "lower" | "low" => lower,
            "upper" | "high" => upper,
            _ => mid,
        })
        .collect();
    if v.is_empty() {
        mid
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

/// Builds the forecast post job.
pub fn forecast_job(data: &ForecastData, station: &Station, prefs: &Prefs, sponsors: &[SponsorPick]) -> Value {
    let location = if station.region.is_empty() { format!("{} Weather", station.name) } else { format!("{} Weather", station.region) };
    let mut job = json!({
        "kind": "forecast",
        "scale": 2,
        "design": prefs.forecast_design,
        "ratio": prefs.forecast_ratio,
        "theme": theme_json(&prefs.forecast_theme),
        "eyebrow": drawable(&data.eyebrow),
        "location": drawable(&location.to_uppercase()),
        "headline": drawable(&data.headline),
        "description": drawable(&data.description),
        "banner_type": "accent",
        "show_graph": prefs.show_graph,
        "watermark_opacity": prefs.watermark_opacity,
        "brand": { "name": station.name, "site": station.site, "logo": station.logo },
        "sponsors": sponsors.iter().map(sponsor_json).collect::<Vec<_>>(),
        "days": data.days.iter().map(|d| {
            let h = holiday(d.date);
            json!({
                "label": drawable(&h.map(|h| h.0.to_string()).unwrap_or_else(|| short_day_label(&d.label))),
                "date": d.date.format("%b %-d").to_string(),
                "icon": h.map(|h| h.1).unwrap_or_else(|| icon_for(&d.description)),
                "description": drawable(&d.description),
                "high": d.high,
                "low": d.low,
                "holiday": h.is_some(),
                "rain": d.rain,
            })
        }).collect::<Vec<_>>(),
    });
    // Regional: the state forecast's city table, always in the Broadcast design.
    let coverage = prefs.forecast_scope == "coverage";
    if let (true, Some(r)) = (prefs.forecast_scope == "regional" || coverage, &data.regional) {
        let (cities, days) = match prefs.forecast_ratio.as_str() {
            "square" => (9, 4),
            "story" => (12, 3),
            _ => (7, 7),
        };
        let cell = |d: &Day| json!({ "icon": icon_for(&d.description), "description": drawable(&d.description), "high": d.high, "low": d.low, "rain": d.rain });
        job["design"] = "broadcast".into();
        job["location"] = drawable(&title_case(&r.section)).into();
        job["days"] = r.columns.iter().take(days).map(|(l, d)| json!({ "label": l, "date": d })).collect::<Vec<_>>().into();
        job["cities"] = r
            .cities
            .iter()
            .take(cities)
            .enumerate()
            .map(|(i, (name, ds))| {
                // Coverage: home is the first row. Regional: the picked highlighted city.
                let home = if coverage { i == 0 } else { !prefs.regional_home_city.is_empty() && name.eq_ignore_ascii_case(&prefs.regional_home_city) };
                json!({ "name": drawable(name), "home": home, "days": ds.iter().take(days).map(cell).collect::<Vec<_>>() })
            })
            .collect::<Vec<_>>()
            .into();
        if coverage {
            job["title"] = drawable(&format!("{} Regional Outlook", station.name)).into();
            job["location"] = drawable(&station.area_line()).into();
            job["source"] = "Forecasts from the National Weather Service".into();
        } else {
            job["source"] = format!("NWS Tabular State Forecast{}", if r.office.is_empty() { String::new() } else { format!(" • {}", r.office) }).into();
        }
    } else if prefs.forecast_scope == "outlook" {
        // TV-style 7-Day Outlook: Broadcast design, brand title, area strip, today's headline kept.
        let n = data.days.len();
        job["design"] = "broadcast".into();
        job["title"] = drawable(&format!("{} {} Outlook", station.name, if n > 1 { format!("{n}-Day") } else { "Daily".into() })).into();
        let area = if data.place.is_empty() { station.area_line() } else { data.place.clone() };
        job["location"] = drawable(&area).into();
    } else if !data.place.is_empty() {
        job["location"] = drawable(&format!("{} Weather", data.place).to_uppercase()).into();
    }
    job
}

/// The area line for coverage graphics: the Coverage area's region name when set, else
/// [`outlook_area`] of the home label.
pub fn coverage_area(s: &crate::config::Settings) -> String {
    crate::coverage::region_name(s).map(str::to_string).unwrap_or_else(|| outlook_area(&s.home_label))
}

/// "Dubois County, IN" -> "Dubois County, Indiana & surrounding counties".
pub fn outlook_area(home_label: &str) -> String {
    let label = home_label.trim();
    let full = match label.rsplit_once(", ") {
        Some((place, st)) if st.len() == 2 => {
            let name = crate::gfx::history_set::data::state_name(&st.to_ascii_uppercase());
            if name.is_empty() { label.to_string() } else { format!("{place}, {name}") }
        }
        _ => label.to_string(),
    };
    if full.is_empty() { "Surrounding counties".into() } else { format!("{full} & surrounding counties") }
}

/// Saves a PNG into Exports and returns its path.
pub fn save_png(bytes: &[u8], base_name: &str) -> Result<std::path::PathBuf> {
    let clean: String = base_name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let path = crate::library::exports_dir().join(format!("{clean}_{}.png", Local::now().format("%Y-%m-%d_%H-%M-%S")));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ugc_lines_expand() {
        assert_eq!(expand_ugc("INC037-150845-"), vec!["INC037"]);
        assert_eq!(expand_ugc("INZ023>025-045-KYZ001-150845-"), vec!["INZ023", "INZ024", "INZ025", "INZ045", "KYZ001"]);
        assert_eq!(ugc_fips("INC037").as_deref(), Some("18037"));
        assert_eq!(ugc_fips("INZ045"), None);
    }

    #[test]
    fn forecast_wording_becomes_numbers() {
        assert_eq!(temp_from_words("Mostly sunny. Highs in the upper 70s. South winds.", "Highs"), Some(78));
        assert_eq!(temp_from_words("Clear. Lows in the mid 50s.", "Lows"), Some(55));
        assert_eq!(temp_from_words("Lows in the lower 40s. South winds.", "Lows"), Some(42));
        assert_eq!(temp_from_words("Highs around 55.", "Highs"), Some(55));
        assert_eq!(temp_from_words("Highs 55 to 60.", "Highs"), Some(58));
        assert_eq!(temp_from_words("Lows in the lower to mid 30s.", "Lows"), Some(34));
        assert_eq!(temp_from_words("Highs in the upper 70s to lower 80s.", "Highs"), Some(80));
        assert_eq!(temp_from_words("Lows in the teens.", "Lows"), Some(15));
        assert_eq!(temp_from_words("Lows 5 below zero.", "Lows"), Some(-5));
        assert_eq!(temp_from_words("Sunny.", "Highs"), None);
    }

    #[test]
    fn zone_forecast_parses() {
        let zfp = "INZ045-140845-\nDubois-\nIncluding the city of Jasper\n939 PM EDT Thu Mar 13 2025\n\n.TONIGHT...Partly cloudy. Lows in the mid 50s.\n.FRIDAY...Mostly sunny. Highs in the upper 70s. South winds 5 to\n15 mph.\n.FRIDAY NIGHT...Breezy. Partly cloudy with chance of showers. Lows in the lower 60s.\n.SATURDAY...Thunderstorms in the morning, then showers. Highs in the mid 70s.\n$$\n";
        let thu = NaiveDate::from_ymd_opt(2025, 3, 13).unwrap();
        let f = parse_zfp(zfp, "INZ045", thu).unwrap();
        assert_eq!(f.headline, "TONIGHT OUTLOOK");
        assert_eq!(f.days.len(), 2);
        assert_eq!((f.days[0].label.as_str(), f.days[0].high, f.days[0].low), ("Friday", Some(78), Some(62)));
        assert_eq!(f.days[0].date, NaiveDate::from_ymd_opt(2025, 3, 14).unwrap());
        assert_eq!(f.days[0].description, "Mostly sunny");
        assert_eq!(f.days[1].description, "Thunderstorms");
        assert!(parse_zfp(zfp, "INZ046", thu).is_none());
    }

    #[test]
    fn archived_warning_text_becomes_card_fields() {
        let text = "WFUS53 KLMK 150808\nTORLMK\nINC037-150845-\n/O.NEW.KLMK.TO.W.0009.250315T0808Z-250315T0845Z/\n\nBULLETIN - EAS ACTIVATION REQUESTED\nTornado Warning\n\nThe National Weather Service in Louisville has issued a\n\n* Tornado Warning for...\n  Northern Dubois County in south central Indiana...\n\n* At 408 AM EDT, a confirmed large tornado was located near Oakland City.\n\n  HAZARD...Damaging tornado.\n\n  SOURCE...Radar confirmed tornado.\n\n  IMPACT...You are in a life-threatening situation. Mobile\n           homes will be destroyed.\n\nPRECAUTIONARY/PREPAREDNESS ACTIONS...\n\nTAKE COVER NOW!\n\n&&\n\nLAT...LON 3851 8707 3850 8706\n\nTORNADO...OBSERVED\nTORNADO DAMAGE THREAT...CONSIDERABLE\nMAX HAIL SIZE...<.75 IN\n\n$$\n";
        let c = parse_product(text, "Tornado Warning");
        let (hazard, source, impact) = sections(&c.description);
        assert_eq!(hazard, "Damaging tornado.");
        assert_eq!(source, "Radar confirmed tornado.");
        assert_eq!(impact, "You are in a life-threatening situation. Mobile homes will be destroyed.");
        assert_eq!(c.instruction, "TAKE COVER NOW!");
        let tags = threat_tags(&c.params);
        assert_eq!(tags[0]["label"], "Tornado");
        assert_eq!(tags[0]["level"], "danger");
        assert!(tags.iter().any(|t| t["label"] == "Hail" && t["value"] == "Under 0.75\""));
        assert_eq!(banner_text("Tornado Warning"), "TORNADO WARNING IN EFFECT — TAKE COVER NOW");
    }

    /// Network: the 2025-03-15 Dubois County tornado warning and that day's archived forecast,
    /// written as PNGs to $CWX_SOCIAL_OUT. `cargo test social_graphics -- --ignored`.
    fn sample_coverage() -> crate::config::Settings {
        use crate::config::CoverageCounty;
        let county = |fips: &str, name: &str, town: &str| CoverageCounty { fips: fips.into(), name: name.into(), state: "IN".into(), town: town.into() };
        let mut s = crate::config::Settings::default();
        s.station_name = "OpenWX".into();
        s.coverage.name = "Southwest Indiana".into();
        s.coverage.home_fips = "18037".into();
        s.coverage.counties = vec![county("18125", "Pike", ""), county("18037", "Dubois", "Jasper"), county("18123", "Perry", "Tell City"), county("18147", "Spencer", ""), county("18117", "Orange", "")];
        s.alert_zones = vec![crate::config::AlertZone { name: "Patoka Lake".into(), lat: 38.43, lon: -86.66, miles: 5.0 }];
        s
    }

    /// Supported counties are the coverage towns (home county first, at each county's centre,
    /// town name or the county name until NWS names it), then watched spots and regions.
    #[test]
    fn coverage_places_use_supported_counties() {
        let Some(all) = crate::map::boundaries::counties_named() else {
            eprintln!("county outlines not cached here; skipped");
            return;
        };
        let s = sample_coverage();
        let regions = vec![crate::forecast_edit::Region { name: "River valley".into(), shape: crate::forecast_edit::Shape::Circle { lat: 37.95, lon: -86.9, radius_mi: 8.0 }, ..Default::default() }];
        let places = coverage_places(&s, &regions);
        let names: Vec<&str> = places.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names[0], "Jasper");
        assert!(places[0].home && places[0].fips == "18037" && !places[0].auto_name);
        assert_eq!(&names[1..3], ["Pike", "Tell City"]);
        assert!(places[1].auto_name);
        assert_eq!(&names[5..], ["Patoka Lake", "River valley"]);
        // Each county place sits at its county's centre, inside the county.
        let dubois = all.iter().find(|c| c.fips == "18037").unwrap();
        assert_eq!(crate::coverage::county_at(std::slice::from_ref(dubois), places[0].lat, places[0].lon).map(|c| c.fips.as_str()), Some("18037"));
        assert_eq!(coverage_area(&s), "Southwest Indiana");
        // Without supported counties: home first, as before.
        let legacy = coverage_places(&crate::config::Settings::default(), &[]);
        assert!(legacy[0].home && legacy[0].fips.is_empty());
    }

    /// Live NWS: the supported counties' forecasts in the Regional Outlook graphic
    /// (`DCWX_OUT=<dir> cargo test live_supported_counties_outlook -- --ignored`).
    #[test]
    #[ignore]
    fn live_supported_counties_outlook() {
        let Ok(dir) = std::env::var("DCWX_OUT") else { return };
        let s = sample_coverage();
        let places = coverage_places(&s, &[]);
        let regional = crate::net::runtime().block_on(coverage_forecast(places)).expect("coverage forecasts");
        let names: Vec<&str> = regional.cities.iter().map(|c| c.0.as_str()).collect();
        eprintln!("coverage towns: {names:?}");
        assert_eq!(names[0], "Jasper");
        assert!(!names.contains(&"Pike"), "Pike gets its NWS city name");
        let data = crate::data::WxSnapshot { coverage: Some(regional), ..Default::default() };
        let b = crate::gfx::bind::Bindings::build(&data, &s, None, "");
        assert_eq!(b.get("cov.area"), Some("Southwest Indiana"));
        let doc = crate::gfx::templates::build("regional_outlook").expect("template");
        let px = crate::program::render_graphic_mode(&doc, &s, &data, doc.intro_length() + 1.0, 1920, 1080, None, crate::gfx::check::PreviewData::Live);
        px.save_png(std::path::Path::new(&dir).join("studio-regional-outlook-counties.png")).unwrap();
    }

    #[test]
    #[ignore]
    fn live_archive_cards() {
        let Ok(out) = std::env::var("CWX_SOCIAL_OUT") else { return };
        let out = std::path::PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        crate::net::runtime().block_on(async {
            let home = home(38.3914, -86.9311).await.unwrap();
            assert_eq!((home.county_ugc.as_str(), home.wfo.as_str(), home.fips.as_str()), ("INC037", "LMK", "18037"));
            let day = NaiveDate::from_ymd_opt(2025, 3, 15).unwrap();
            let picks = past_alerts(&home.county_ugc, day, day).await.unwrap();
            let tor = picks.iter().find(|p| p.event == "Tornado Warning").expect("tornado warning in the archive").clone();
            let station = Station { name: "Dubois County Weather".into(), region: "Dubois County, IN".into(), site: "dcweather.xyz".into(), logo: None, area: String::new() };
            for design in ALERT_DESIGNS {
                let job = alert_job(tor.clone(), station.clone(), home.fips.clone(), "https://dcweather.xyz".into(), design.into(), None, lines_from_scene(&crate::map::scene::MapScene::default())).await.unwrap();
                std::fs::write(out.join(format!("past-tornado-{design}.png")), render(&job).unwrap()).unwrap();
            }
            let fc = past_forecast(&home, NaiveDate::from_ymd_opt(2025, 3, 14).unwrap()).await.unwrap();
            assert!(fc.days.len() >= 5, "{fc:?}");
            for ratio in FORECAST_RATIOS {
                let prefs = Prefs { forecast_ratio: ratio.into(), ..Default::default() };
                std::fs::write(out.join(format!("past-forecast-{ratio}.png")), render(&forecast_job(&fc, &station, &prefs, &[])).unwrap()).unwrap();
            }
            let live = live_forecast(38.3914, -86.9311).await.unwrap();
            std::fs::write(out.join("live-forecast.png"), render(&forecast_job(&live, &station, &Prefs::default(), &[])).unwrap()).unwrap();
        });
    }

    /// The Graphics Studio's Regional Outlook with five coverage towns (PNG in $DCWX_OUT when set),
    /// the Social panel's coverage card, and the outlook's area line.
    #[test]
    fn coverage_table_renders() {
        assert_eq!(outlook_area("Dubois County, IN"), "Dubois County, Indiana & surrounding counties");
        let words = ["Sunny", "Partly Sunny", "Chance Showers", "Rain Likely", "Mostly Cloudy", "Mostly Sunny", "Sunny"];
        let days = |t: i32| (0..7).map(|i| Day { label: "Monday".into(), date: NaiveDate::from_ymd_opt(2026, 11, 15 + i as u32).unwrap(), description: words[i as usize].into(), high: Some(55 + i + t), low: Some(32 + i), rain: Some(10 * i) }).collect::<Vec<_>>();
        let names = ["Jasper", "Huntingburg", "Ferdinand", "Petersburg", "Tell City"];
        let regional = Regional {
            section: "COVERAGE AREA".into(),
            columns: ["Today", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].iter().enumerate().map(|(i, d)| (d.to_string(), format!("Nov {}", 15 + i))).collect(),
            cities: names.iter().enumerate().map(|(t, n)| (n.to_string(), days(t as i32))).collect(),
            ..Default::default()
        };
        let mut settings = crate::config::Settings::default();
        settings.station_name = "OpenWX".into();
        let data = crate::data::WxSnapshot { coverage: Some(regional.clone()), ..Default::default() };
        let b = crate::gfx::bind::Bindings::build(&data, &settings, None, "");
        assert_eq!(b.get("cov1.name"), Some("Huntingburg"));
        assert_eq!(b.get("cov6.d0.condition"), Some("No icon"));
        let doc = crate::gfx::templates::build("regional_outlook").expect("template");
        let px = crate::program::render_graphic_mode(&doc, &settings, &data, doc.intro_length() + 1.0, 1920, 1080, None, crate::gfx::check::PreviewData::Live);
        let station = Station { name: "OpenWX".into(), region: "Dubois County, IN".into(), site: "dcweather.xyz".into(), logo: None, area: String::new() };
        let prefs = Prefs { forecast_scope: "coverage".into(), ..Default::default() };
        let job = forecast_job(&ForecastData { regional: Some(regional), headline: "Mild week".into(), ..Default::default() }, &station, &prefs, &[]);
        assert_eq!(job["title"], "OpenWX Regional Outlook");
        let card = render(&job).expect("coverage card");
        if let Ok(dir) = std::env::var("DCWX_OUT") {
            px.save_png(std::path::Path::new(&dir).join("studio-regional-outlook.png")).unwrap();
            std::fs::write(std::path::Path::new(&dir).join("studio-coverage-card.png"), card).unwrap();
        }
    }

    #[test]
    fn drawable_drops_emoji_and_symbols() {
        assert_eq!(drawable("⚠ TORNADO  WARNING — now 🌪"), "TORNADO WARNING — now");
    }

    #[test]
    fn state_forecast_parses_and_renders() {
        let text = "000\nFPUS63 KIND 151413\nSFTIN\n\n   FCST     FCST     FCST     \n   Today    Mon      Tue      \n   Nov 15   Nov 16   Nov 17   \n\n...NORTHERN INDIANA...\n   Angola, IN\n   Ptcldy   Ptcldy   Sunny    \n     /47    32/51    30/54    \n      /00    00/00    00/20   \n\n...SOUTHERN INDIANA...\n   Evansville, IN\n   Sunny    Tstrms   Sunny    \n     /54    30/56    32/60    \n      /00    40/60    00/00   \n\n   Jasper, IN\n   Sunny    Rnsnow   Sunny    \n     /54    30/57    29/60    \n      /00    00/30    00/10   \n\n$$\n";
        let (cols, sections) = parse_sft(text);
        assert_eq!(cols, vec![("Today".to_string(), "Nov 15".to_string()), ("Mon".into(), "Nov 16".into()), ("Tue".into(), "Nov 17".into())]);
        assert_eq!(sections.len(), 2);
        let (name, cities) = &sections[1];
        assert_eq!(name, "SOUTHERN INDIANA");
        assert_eq!(cities[1].0, "Jasper");
        let jasper = &cities[1].1;
        assert_eq!((jasper[0].high, jasper[0].low, jasper[0].rain), (Some(54), None, Some(0)));
        assert_eq!((jasper[1].high, jasper[1].low, jasper[1].rain, jasper[1].description.as_str()), (Some(57), Some(30), Some(30), "Rain and snow"));
        assert_eq!(cities[0].1[1].description, "Thunderstorms");

        let station = Station { name: "Test Station".into(), region: "Test County".into(), site: "example.org".into(), logo: None, area: String::new() };
        let regional = Regional { section: name.clone(), sections: vec![name.clone()], office: "IND".into(), columns: cols, cities: cities.clone() };
        let data = ForecastData { regional: Some(regional), ..Default::default() };
        for ratio in FORECAST_RATIOS {
            let prefs = Prefs { forecast_scope: "regional".into(), forecast_ratio: ratio.into(), regional_home_city: "Jasper".into(), ..Default::default() };
            let mut job = forecast_job(&data, &station, &prefs, &[]);
            assert_eq!(job["design"], "broadcast");
            assert_eq!(job["cities"][1]["home"], true);
            job["scale"] = json!(0.5);
            assert!(render(&job).unwrap().starts_with(b"\x89PNG"), "regional {ratio}");
        }
    }

    #[test]
    fn renders_every_design_offline() {
        let station = Station { name: "Test Station".into(), region: "Test County".into(), site: "example.org".into(), logo: None, area: String::new() };
        let data = ForecastData {
            eyebrow: "DAILY OUTLOOK".into(),
            headline: "TODAY OUTLOOK".into(),
            description: "Sunny and mild.".into(),
            days: (0..7).map(|i| Day { label: "Monday".into(), date: NaiveDate::from_ymd_opt(2026, 12, 21 + i).unwrap(), description: "Mostly sunny".into(), high: Some(50 + i as i32), low: Some(30), rain: Some(10 * i as i32) }).collect(),
            note: String::new(),
            regional: None,
            place: String::new(),
        };
        for design in FORECAST_DESIGNS {
            for ratio in FORECAST_RATIOS {
                for (theme, _, _) in THEMES {
                    let prefs = Prefs { forecast_design: design.into(), forecast_ratio: ratio.into(), forecast_theme: theme.into(), ..Default::default() };
                    let mut job = forecast_job(&data, &station, &prefs, &[]);
                    job["scale"] = json!(0.5);
                    assert!(render(&job).unwrap().starts_with(b"\x89PNG"), "{design} {ratio} {theme}");
                }
            }
        }
        for design in ALERT_DESIGNS {
            let job = json!({ "kind": "alert", "scale": 0.5, "design": design, "event": "Tornado Warning", "headline": banner_text("Tornado Warning"), "banner_color": "#FF0000", "kind_color": "#FF0000", "area": ["Dubois"], "brand": { "name": "Test" } });
            assert!(render(&job).unwrap().starts_with(b"\x89PNG"), "{design}");
        }
    }
}
