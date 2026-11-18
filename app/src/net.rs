//! Networking: one Tokio runtime and one pooled HTTP client for the whole app.
//!
//! Radar and most feeds come through the backend (`settings.api_base`): OpenWX Relay, bundled
//! inside the app on 127.0.0.1 or hosted by the station (see `backend.rs`).

use std::{sync::{OnceLock, RwLock, atomic::{AtomicI64, Ordering}}, time::Duration};

use anyhow::{anyhow, Result};

use crate::{
    config::APP_VERSION,
    data::{parse_spc, Alert, Overview, RadarIndex, SpcDay, StormCell, StormCells},
};

pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("cwx-net")
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

/// Run a future on the network runtime. The returned handle can be awaited from GPUI tasks.
pub fn spawn<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    runtime().spawn(future)
}

pub fn user_agent() -> String {
    let contact = crate::backend::contact();
    let contact = if contact.is_empty() { "+https://github.com/jdc20181/OpenWX-Broadcasting".to_owned() } else { contact };
    format!("OpenWXBroadcasting/{APP_VERSION}+{} ({contact})", crate::config::APP_BUILD)
}

pub fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(user_agent())
            .timeout(Duration::from_secs(25))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(16)
            .gzip(true)
            .build()
            .expect("http client")
    })
}

fn api_base() -> &'static RwLock<String> {
    static BASE: OnceLock<RwLock<String>> = OnceLock::new();
    BASE.get_or_init(|| RwLock::new(String::new()))
}

pub fn set_api_base(base: &str) {
    if let Ok(mut current) = api_base().write() {
        *current = base.trim_end_matches('/').to_owned();
    }
}

pub fn proxy_url(base: &str, address: &str) -> String {
    let Ok(upstream) = reqwest::Url::parse(address) else { return address.into() };
    let provider = match upstream.host_str().unwrap_or_default() {
        "api.weather.gov" => "nws",
        "mapservices.weather.noaa.gov" => "noaaMap",
        "api.water.noaa.gov" => "water",
        "www.spc.noaa.gov" => "spc",
        "ftp.nhc.noaa.gov" => "nhc",
        "services.swpc.noaa.gov" => "space",
        "nasstatus.faa.gov" => "faa",
        "droughtmonitor.unl.edu" => "drought",
        "mesonet.agron.iastate.edu" => "iem",
        "server.arcgisonline.com" => "esri",
        "services3.arcgis.com" => "esriServices",
        "tiles.arcgis.com" => "esriTiles",
        "gibs.earthdata.nasa.gov" => "gibs",
        "api.zippopotam.us" => "zip",
        // NOAA NCEI daily station summaries + GHCN station list, and the NWS damage-survey
        // service (the relay allows only the exact paths; there is no direct fallback).
        "www.ncei.noaa.gov" => "ncei",
        "services.dat.noaa.gov" => "dat",
        "api.open-meteo.com" => "meteo",
        "air-quality-api.open-meteo.com" => "air",
        "ensemble-api.open-meteo.com" => "ensemble",
        "archive-api.open-meteo.com" => "archive",
        "historical-forecast-api.open-meteo.com" => "historical",
        "geocoding-api.open-meteo.com" => "places",
        _ => return address.into(),
    };
    let Ok(mut relay) = reqwest::Url::parse(&format!(
        "{}/api/weatherbroadcaster/feed", base.trim_end_matches('/')
    )) else { return address.into() };
    let resource = match upstream.query() {
        Some(query) => format!("{}?{query}", upstream.path()),
        None => upstream.path().to_owned(),
    };
    relay.query_pairs_mut().append_pair("provider", provider).append_pair("resource", &resource);
    let mut target = relay.to_string();
    for key in ["z", "x", "y"] {
        target = target.replace(&format!("%257B{key}%257D"), &format!("{{{key}}}"))
            .replace(&format!("%7B{key}%7D"), &format!("{{{key}}}"));
    }
    target
}

pub async fn get_json(url: &str) -> Result<serde_json::Value> {
    Ok(get_logged(url, Duration::from_secs(30)).await?.json().await?)
}

/// Prefer a dedicated broadcaster route, keeping compatibility with older relays.
pub async fn broadcaster_product(path: &str, legacy_upstream: &str, timeout: Duration) -> Result<reqwest::Response> {
    let base = api_base().read().map(|base| base.clone()).unwrap_or_default();
    if base.is_empty() {
        return get_logged(legacy_upstream, timeout).await;
    }
    let url = format!("{base}/api/weatherbroadcaster/{path}");
    let response = http().get(&url).header("Accept", "*/*").timeout(timeout).send().await
        .inspect_err(|error| crate::devlog::http_unreachable(&url, error))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return get_logged(legacy_upstream, timeout).await;
    }
    check(response).await
}

/// `broadcaster_product` for an optional feed (airport delays): a failing route is a warning,
/// not an error that stays on the status bar all session.
pub async fn broadcaster_product_optional(path: &str, legacy_upstream: &str, timeout: Duration) -> Result<reqwest::Response> {
    let base = api_base().read().map(|base| base.clone()).unwrap_or_default();
    if base.is_empty() {
        return get_logged(legacy_upstream, timeout).await;
    }
    let url = format!("{base}/api/weatherbroadcaster/{path}");
    let response = http().get(&url).header("Accept", "*/*").timeout(timeout).send().await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return get_logged(legacy_upstream, timeout).await;
    }
    check_level(response, true).await
}

/// A broadcaster-only relay route with no legacy upstream (404 means the relay is older).
pub async fn broadcaster_route(path: &str, timeout: Duration) -> Result<reqwest::Response> {
    let base = api_base().read().map(|base| base.clone()).unwrap_or_default();
    anyhow::ensure!(!base.is_empty(), "no DCWeather API base configured");
    let url = format!("{base}/api/weatherbroadcaster/{path}");
    let started = std::time::Instant::now();
    let result = match http().get(&url).header("Accept", "application/json").timeout(timeout).send().await {
        Ok(response) => check(response).await,
        Err(e) => {
            crate::devlog::http_unreachable(&url, &e);
            Err(e.into())
        }
    };
    let ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(_) => crate::broadcast_ops::record_fetch(&url, ms, true, ""),
        Err(e) => crate::broadcast_ops::record_fetch(&url, ms, false, &e.to_string()),
    }
    result
}

#[derive(Debug)]
pub struct RateLimit {
    host: String,
    pub retry_after: Duration,
}

impl std::fmt::Display for RateLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rate limited (429 Too Many Requests) by {}. Retry in {}s", self.host, self.retry_after.as_secs())
    }
}

impl std::error::Error for RateLimit {}

fn retry_after(response: &reqwest::Response) -> Duration {
    let seconds = response.headers().get("Retry-After").and_then(|header| header.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok()).unwrap_or(600);
    Duration::from_secs(seconds.clamp(1, 86400))
}

pub async fn get_archive_json(url: &str) -> Result<serde_json::Value> {
    match get_json(url).await {
        Err(error) => {
            if let Some(limit) = error.downcast_ref::<RateLimit>().filter(|limit| limit.retry_after <= Duration::from_secs(65)) {
                tokio::time::sleep(limit.retry_after).await;
                return get_json(url).await;
            }
            Err(error)
        }
        result => result,
    }
}

/// Pass a successful response through; log a failed one (status, URL, body) to the developer
/// log and turn it into an error that names the status.
pub async fn check(response: reqwest::Response) -> Result<reqwest::Response> {
    check_level(response, false).await
}

/// `check` for an optional feed: failures are logged as warnings.
async fn check_level(response: reqwest::Response, optional: bool) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let url = response.url().to_string();
    let retry_after = retry_after(&response);
    let html = response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"));
    let body = if html { "HTML error page from the gateway".to_owned() } else { response.text().await.unwrap_or_default() };
    crate::devlog::http_failure_level(&url, status, &body, optional);
    if status.as_u16() == 429 {
        return Err(RateLimit { host: host_of(&url), retry_after }.into());
    }
    Err(anyhow!("{status} from {url}"))
}

fn host_of(url: &str) -> String {
    url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or(url).to_string()
}

/// GET with failures logged to the developer log.
pub async fn get_logged(url: &str, timeout: Duration) -> Result<reqwest::Response> {
    static METEO_BACKOFF: AtomicI64 = AtomicI64::new(0);
    let meteo = host_of(url).ends_with(".open-meteo.com");
    let now = chrono::Utc::now().timestamp();
    if meteo && now < METEO_BACKOFF.load(Ordering::Relaxed) {
        let remaining = (METEO_BACKOFF.load(Ordering::Relaxed) - now).max(1);
        return Err(RateLimit { host: host_of(url), retry_after: Duration::from_secs(remaining as u64) }.into());
    }
    let base = api_base().read().map(|b| b.clone()).unwrap_or_default();
    let target = proxy_url(&base, url);
    let started = std::time::Instant::now();
    let result = match http().get(target).header("Accept", "*/*").timeout(timeout).send().await.inspect_err(|e| crate::devlog::http_unreachable(url, e)) {
        Ok(response) => {
            if meteo && response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let seconds = retry_after(&response).as_secs() as i64;
                let relay_budget = response.headers().get("X-Broadcaster-Quota").is_some_and(|value| value == "relay");
                if !relay_budget && seconds >= 600 {
                    METEO_BACKOFF.fetch_max(chrono::Utc::now().timestamp() + seconds, Ordering::Relaxed);
                }
            }
            check(response).await
        }
        Err(e) => Err(e.into()),
    };
    let ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(_) => crate::broadcast_ops::record_fetch(url, ms, true, ""),
        Err(e) => crate::broadcast_ops::record_fetch(url, ms, false, &e.to_string()),
    }
    result
}

/// LibreWXR frame index, through the site proxy.
pub async fn radar_index(api_base: String) -> Result<RadarIndex> {
    let url = format!(
        "{}/api/radar?path=public/weather-maps.json",
        api_base.trim_end_matches('/')
    );
    Ok(serde_json::from_value(get_json(&url).await?)?)
}

/// Current conditions + forecast.
pub async fn overview(api_base: String, lat: f64, lon: f64) -> Result<Overview> {
    let url = format!(
        "{}/api/weather/overview?lat={lat:.4}&lon={lon:.4}",
        api_base.trim_end_matches('/')
    );
    Ok(serde_json::from_value(get_json(&url).await?)?)
}

/// Storm cells around a point (radius in km).
pub async fn storm_cells(api_base: String, lat: f64, lon: f64, radius_km: f64) -> Result<Vec<StormCell>> {
    let url = format!(
        "{}/api/radar/storm-cells?lat={lat:.4}&lon={lon:.4}&radius={radius_km:.0}&refLat={lat:.4}&refLon={lon:.4}",
        api_base.trim_end_matches('/')
    );
    let cells: StormCells = serde_json::from_value(get_json(&url).await?)?;
    Ok(cells.cells)
}

/// SPC convective outlooks.
pub async fn spc(api_base: String) -> Result<Vec<SpcDay>> {
    let url = format!("{}/api/weather/spc-outlooks", api_base.trim_end_matches('/'));
    Ok(parse_spc(&get_json(&url).await?))
}

fn alerts_from(value: &serde_json::Value) -> Vec<Alert> {
    let mut alerts: Vec<Alert> = value
        .get("features")
        .and_then(|f| f.as_array())
        .into_iter()
        .flatten()
        .filter_map(Alert::from_feature)
        .collect();
    alerts.sort_by_key(|a| a.priority());
    alerts
}

/// Active NWS alerts for a point (the station's area).
pub async fn alerts_for_point(lat: f64, lon: f64) -> Result<Vec<Alert>> {
    let url = format!("https://api.weather.gov/alerts/active?point={lat:.4},{lon:.4}");
    let mut alerts = alerts_from(&get_json(&url).await?);
    for alert in alerts.iter_mut().filter(|a| a.polygons.is_empty() && !a.fips.is_empty()) {
        alert.polygons = crate::map::boundaries::county_rings(&alert.fips);
        alert.county_based = !alert.polygons.is_empty();
    }
    Ok(alerts)
}

/// Active NWS alerts with polygons for the map. `area` is "" (whole US) or state codes.
pub async fn alerts_for_map(area: String) -> Result<Vec<Alert>> {
    let area = area.replace(' ', "");
    let url = if area.is_empty() {
        "https://api.weather.gov/alerts/active?status=actual".to_string()
    } else {
        format!("https://api.weather.gov/alerts/active?status=actual&area={area}")
    };
    let mut alerts = alerts_from(&get_json(&url).await?);
    // Watches and most advisories carry no polygon, only their counties: draw those.
    for alert in alerts.iter_mut().filter(|a| a.polygons.is_empty() && !a.fips.is_empty()) {
        alert.polygons = crate::map::boundaries::county_rings(&alert.fips);
        alert.county_based = !alert.polygons.is_empty();
    }
    Ok(alerts.into_iter().filter(|a| !a.polygons.is_empty()).collect())
}

/// Lightning strikes from the site's Blitzortung relay (public feed).
pub async fn lightning(api_base: String) -> Result<Vec<crate::data::Strike>> {
    let url = format!("{}/api/public/lightning.json", api_base.trim_end_matches('/'));
    Ok(crate::data::parse_lightning(&get_json(&url).await?))
}

/// Online place lookup for what the built-in town list cannot answer: US ZIP codes
/// (zippopotam.us) and counties, landmarks or small places (Open-Meteo geocoding, GeoNames).
/// Returns (label, lat, lon).
pub async fn geocode(query: String) -> Result<Vec<(String, f64, f64)>> {
    let q = query.trim().to_string();
    if q.len() == 5 && q.chars().all(|c| c.is_ascii_digit()) {
        let v = get_json(&format!("https://api.zippopotam.us/us/{q}")).await?;
        let places = v.get("places").and_then(|p| p.as_array()).cloned().unwrap_or_default();
        return Ok(places
            .iter()
            .filter_map(|p| {
                let lat: f64 = p.get("latitude")?.as_str()?.parse().ok()?;
                let lon: f64 = p.get("longitude")?.as_str()?.parse().ok()?;
                let name = p.get("place name")?.as_str()?;
                let st = p.get("state abbreviation").and_then(|s| s.as_str()).unwrap_or("");
                Some((format!("{q}: {name}, {st}"), lat, lon))
            })
            .collect());
    }
    // "Dubois County, IN" -> name "Dubois County", state filter "IN".
    let (name, state) = match q.rsplit_once(',') {
        Some((n, s)) if s.trim().len() == 2 => (n.trim().to_string(), Some(s.trim().to_uppercase())),
        _ => (q.clone(), None),
    };
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=10&language=en&format=json&countryCode=US",
        name.replace(' ', "+")
    );
    let v = get_json(&url).await?;
    let results = v.get("results").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    let states = crate::places::STATE_NAMES;
    Ok(results
        .iter()
        .filter_map(|r| {
            let lat = r.get("latitude")?.as_f64()?;
            let lon = r.get("longitude")?.as_f64()?;
            let n = r.get("name")?.as_str()?;
            let admin1 = r.get("admin1").and_then(|a| a.as_str()).unwrap_or("");
            let abbr = states.iter().find(|(_, full)| *full == admin1).map(|(a, _)| *a).unwrap_or(admin1);
            if let Some(st) = &state {
                if !abbr.eq_ignore_ascii_case(st) {
                    return None;
                }
            }
            let admin2 = r.get("admin2").and_then(|a| a.as_str()).filter(|a| *a != n).map(|a| format!(" ({a})")).unwrap_or_default();
            Some((format!("{n}, {abbr}{admin2}"), lat, lon))
        })
        .collect())
}

/// Active SPC mesoscale discussions (NOAA map services; no DCWeather route exists for these).
pub async fn mesoscale() -> Result<Vec<crate::data::MesoDiscussion>> {
    let value = get_json(
        "https://mapservices.weather.noaa.gov/vector/rest/services/outlooks/spc_mesoscale_discussion/MapServer/0/query?where=1%3D1&outFields=*&f=geojson",
    )
    .await?;
    Ok(crate::data::parse_mesoscale(&value))
}

/// NWS Local Storm Reports for the last 24 hours (Iowa Environmental Mesonet).
pub async fn storm_reports() -> Result<Vec<crate::data::StormReport>> {
    let value = get_json("https://mesonet.agron.iastate.edu/geojson/lsr.geojson?hours=24").await?;
    Ok(crate::data::parse_lsr(&value))
}

/// Airport observations (METAR) inside a box, through the broadcaster relay.
pub async fn metars(south: f64, west: f64, north: f64, east: f64) -> Result<Vec<crate::data::Metar>> {
    let path = format!("aviation/metar?bbox={south:.2},{west:.2},{north:.2},{east:.2}");
    let value: serde_json::Value = broadcaster_route(&path, Duration::from_secs(30)).await?.json().await?;
    Ok(crate::data::parse_metars(&value))
}

/// INDOT traffic events from the DCWeather API.
pub async fn road_events(api_base: String) -> Result<Vec<crate::data::RoadEvent>> {
    let url = format!("{}/api/indot/events", api_base.trim_end_matches('/'));
    Ok(crate::data::parse_road_events(&get_json(&url).await?))
}

/// Plain GET of bytes (update downloads use their own streaming path).
pub async fn get_bytes(url: &str) -> Result<Vec<u8>> {
    let response = get_logged(url, Duration::from_secs(25)).await?;
    Ok(response.bytes().await?.to_vec())
}

/// Round-trip check of the radar proxy, for Settings → Test connection.
pub async fn test_connection(api_base: String) -> Result<String> {
    let started = std::time::Instant::now();
    let index = radar_index(api_base).await?;
    let ms = started.elapsed().as_millis();
    let latest = index
        .latest_past()
        .map(|f| {
            chrono::DateTime::from_timestamp(f.time, 0)
                .map(|t| t.with_timezone(&chrono::Local).format("%-I:%M %p").to_string())
                .unwrap_or_default()
        })
        .unwrap_or_else(|| "none".into());
    Ok(format!(
        "Radar proxy OK in {ms} ms - {} frames, latest {latest}",
        index.radar.past.len()
    ))
}
