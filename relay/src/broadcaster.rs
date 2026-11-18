//! `/api/weatherbroadcaster/*`: the fixed-provider relay the desktop app reads. The app sends a
//! provider ID and a path, never an upstream URL; each provider has one host and an allow list
//! of paths. Ported from the Node relay the app was built against, same contract.

use std::{collections::HashMap, sync::LazyLock, time::Duration};

use axum::{
    extract::{Query, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use regex::Regex;

use crate::{
    fetch::{url, ApiError, ApiResult, FetchOptions, DAY, HOUR, LIVE, MAX_LARGE_RESPONSE},
    AppState,
};

type Q = Query<HashMap<String, String>>;

pub struct Provider {
    pub id: &'static str,
    pub host: &'static str,
    pattern: &'static str,
    /// Open-Meteo providers obey `open_meteo = false`.
    pub open_meteo: bool,
}

pub const PROVIDERS: &[Provider] = &[
    Provider { id: "nws", host: "api.weather.gov", pattern: r"^/(alerts/active|points/[\d.,-]+|gridpoints/[A-Z]{3}/[\d,]+(?:/forecast(?:/hourly)?|/stations)?|products/types/SFT/locations/[A-Z]{2}|products/[0-9a-f-]{36}|zones/county|zones/forecast/[A-Z]{3}\d{3}|stations/[A-Z0-9]{3,5}/observations/latest)$", open_meteo: false },
    Provider { id: "noaaMap", host: "mapservices.weather.noaa.gov", pattern: r"^/(?:vector|raster|tropical|eventdriven)/rest/services/[\w./-]+/(?:query|MapServer)$", open_meteo: false },
    Provider { id: "water", host: "api.water.noaa.gov", pattern: r"^/nwps/v1/gauges(?:/[\w-]+(?:/stageflow)?)?$", open_meteo: false },
    Provider { id: "spc", host: "www.spc.noaa.gov", pattern: r"^/(?:climo/reports|products|wcm/data)/[\w./-]+$", open_meteo: false },
    Provider { id: "nhc", host: "ftp.nhc.noaa.gov", pattern: r"^/atcf/[\w./-]+$", open_meteo: false },
    Provider { id: "space", host: "services.swpc.noaa.gov", pattern: r"^/(?:products|json)/[\w./-]+$", open_meteo: false },
    Provider { id: "faa", host: "nasstatus.faa.gov", pattern: r"^/api/[\w./-]+$", open_meteo: false },
    Provider { id: "drought", host: "droughtmonitor.unl.edu", pattern: r"(?i)^/[\w./-]+\.(?:json|geojson|zip)$", open_meteo: false },
    Provider { id: "iem", host: "mesonet.agron.iastate.edu", pattern: r"^/(?:cache/tile\.py/1\.0\.0|json|api/1|cgi-bin|geojson)/[\w./:-]+$", open_meteo: false },
    Provider { id: "esri", host: "server.arcgisonline.com", pattern: r"(?i)^/ArcGIS/rest/services/[\w./-]+/MapServer/tile/\d+/\d+/\d+$", open_meteo: false },
    Provider { id: "esriServices", host: "services3.arcgis.com", pattern: r"(?i)^/[\w./-]+/FeatureServer/\d+/query$", open_meteo: false },
    Provider { id: "esriTiles", host: "tiles.arcgis.com", pattern: r"(?i)^/[\w./-]+/MapServer/tile/\d+/\d+/\d+$", open_meteo: false },
    Provider { id: "gibs", host: "gibs.earthdata.nasa.gov", pattern: r"^/wmts/[\w./:-]+$", open_meteo: false },
    Provider { id: "zip", host: "api.zippopotam.us", pattern: r"^/us/(?:\d{5}|[a-z]{2}/[\w%.' -]+)$", open_meteo: false },
    Provider { id: "ncei", host: "www.ncei.noaa.gov", pattern: r"^/(?:access/services/data/v1|pub/data/ghcn/daily/ghcnd-stations\.txt)$", open_meteo: false },
    Provider { id: "dat", host: "services.dat.noaa.gov", pattern: r"^/arcgis/rest/services/nws_damageassessmenttoolkit/DamageViewer/FeatureServer/1(?:/query)?$", open_meteo: false },
    Provider { id: "usgs", host: "earthquake.usgs.gov", pattern: r"^/earthquakes/feed/v1\.0/summary/[\w.]+\.geojson$", open_meteo: false },
    Provider { id: "acis", host: "data.rcc-acis.org", pattern: r"^/(?:StnMeta|StnData|MultiStnData|GridData)$", open_meteo: false },
    Provider { id: "meteo", host: "api.open-meteo.com", pattern: r"^/v1/(?:forecast|elevation)$", open_meteo: true },
    Provider { id: "air", host: "air-quality-api.open-meteo.com", pattern: r"^/v1/air-quality$", open_meteo: true },
    Provider { id: "ensemble", host: "ensemble-api.open-meteo.com", pattern: r"^/v1/ensemble$", open_meteo: true },
    Provider { id: "archive", host: "archive-api.open-meteo.com", pattern: r"^/v1/archive$", open_meteo: true },
    Provider { id: "historical", host: "historical-forecast-api.open-meteo.com", pattern: r"^/v1/forecast$", open_meteo: true },
    Provider { id: "places", host: "geocoding-api.open-meteo.com", pattern: r"^/v1/search$", open_meteo: true },
];

static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| PROVIDERS.iter().map(|p| Regex::new(p.pattern).expect("provider pattern")).collect());

const DROUGHT_PATH: &str = "/Data/json/USDM_current.json";
const STATION_LIST_PATH: &str = "/pub/data/ghcn/daily/ghcnd-stations.txt";
const FAA_PATH: &str = "/api/airport-status-information";
static QPF_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^/vector/rest/services/precip/wpc_qpf/MapServer/(?:1|9|11)/query$").unwrap());

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/feed", get(feed))
        .route("/aviation/airport-status", get(airport_status))
        .route("/aviation/metar", get(metar))
        .route("/outlooks/qpf", get(qpf))
        .route("/outlooks/drought", get(drought))
        .route("/nws/forecast", get(nws_forecast))
        .route("/climate/daily", get(climate_daily))
        .route("/climate/normals", get(crate::climate::normals))
        .route("/model-run", get(model_run_route))
        .route("/model-frame", get(model_frame))
        .route("/archive-scans", get(archive_scans))
        .route("/archive-velocity-tile", get(archive_velocity_tile))
}

/// Resolve a provider request to its upstream URL, or refuse it.
pub fn feed_url(provider: &str, resource: &str, open_meteo: bool) -> ApiResult<url::Url> {
    let index = PROVIDERS.iter().position(|p| p.id == provider);
    let Some(index) = index else { return Err(ApiError::bad_request("Invalid broadcaster data request")) };
    let config = &PROVIDERS[index];
    if config.open_meteo && !open_meteo {
        return Err(ApiError::new(403, "Open-Meteo is turned off on this relay"));
    }
    if resource.len() > 14_000 || !resource.starts_with('/') || resource.starts_with("//") || resource.contains(['\\', '#']) {
        return Err(ApiError::bad_request("Invalid broadcaster data request"));
    }
    let origin = format!("https://{}", config.host);
    let mut target = url::Url::parse(&origin)
        .and_then(|base| base.join(resource))
        .map_err(|_| ApiError::bad_request("Invalid broadcaster data request"))?;
    if target.origin().ascii_serialization() != origin || !PATTERNS[index].is_match(target.path()) {
        return Err(ApiError::bad_request("Unsupported weather feed path"));
    }
    if provider == "drought" && target.path().eq_ignore_ascii_case(DROUGHT_PATH) {
        target.set_path(DROUGHT_PATH);
    }
    let qpf = provider == "noaaMap" && QPF_PATH.is_match(target.path()) && target.query_pairs().any(|(k, v)| k == "f" && v == "geojson");
    // Drop the client's refresh token and sort the query so identical requests share one call.
    let mut pairs: Vec<(String, String)> = target.query_pairs().filter(|(k, _)| k != "r").map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
    if qpf {
        generalize_qpf(&mut pairs);
    }
    pairs.sort();
    if pairs.is_empty() {
        target.set_query(None);
    } else {
        target.query_pairs_mut().clear().extend_pairs(pairs);
    }
    Ok(target)
}

fn generalize_qpf(pairs: &mut Vec<(String, String)>) {
    for (key, value) in [("outSR", "4326"), ("maxAllowableOffset", "0.01"), ("geometryPrecision", "4")] {
        pairs.retain(|(k, _)| k != key);
        pairs.push((key.into(), value.into()));
    }
}

/// How long a feed response may be kept: only files that cannot change.
fn feed_ttl(provider: &str, target: &url::Url) -> Duration {
    let path = target.path();
    let tile = path.contains("/tile/") || path.contains("/tile.py/") || path.contains("/wmts/");
    let dated_radar = provider == "iem" && Regex::new(r"ridge::USCOMP-N0Q-\d{12}/").unwrap().is_match(path);
    let finished_archive = provider == "archive"
        && target
            .query_pairs()
            .find(|(k, _)| k == "end_date")
            .and_then(|(_, v)| chrono::NaiveDate::parse_from_str(&v, "%Y-%m-%d").ok())
            .is_some_and(|end| end < chrono::Utc::now().date_naive() - chrono::Duration::days(14));
    if finished_archive {
        30 * DAY
    } else if provider == "archive" || provider == "historical" {
        DAY
    } else if dated_radar || (tile && provider != "iem") {
        DAY
    } else if provider == "drought" && path == DROUGHT_PATH {
        6 * HOUR
    } else if provider == "ncei" && path == STATION_LIST_PATH {
        DAY
    } else {
        LIVE
    }
}

async fn feed(State(state): State<AppState>, Query(q): Q) -> ApiResult<Response> {
    let provider = q.get("provider").map(String::as_str).unwrap_or_default();
    let resource = q.get("resource").map(String::as_str).unwrap_or_default();
    let target = feed_url(provider, resource, state.config.open_meteo)?;
    let large = (provider == "drought" && target.path() == DROUGHT_PATH) || (provider == "ncei" && target.path() == STATION_LIST_PATH);
    let ttl = feed_ttl(provider, &target);
    let entry = state
        .fetch
        .get(target, ttl, FetchOptions { max_bytes: if large { MAX_LARGE_RESPONSE } else { 0 }, ..Default::default() })
        .await?;
    let image = entry.content_type.starts_with("image/");
    let mut response = entry.into_response();
    if image && !ttl.is_zero() {
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=86400"));
    }
    Ok(response)
}

async fn airport_status(State(state): State<AppState>) -> ApiResult<Response> {
    let target = url(&format!("https://nasstatus.faa.gov{FAA_PATH}"));
    let options = FetchOptions { accept: Some("application/xml, text/xml"), timeout: Some(Duration::from_secs(8)), ..Default::default() };
    let entry = match state.fetch.get(target.clone(), LIVE, options).await {
        Err(error) if error.status.as_u16() >= 500 => state.fetch.get(target, LIVE, options).await?,
        other => other?,
    };
    let head = String::from_utf8_lossy(&entry.body[..entry.body.len().min(2048)]).to_string();
    if !Regex::new(r"(?i)^\s*(?:<\?xml[^>]*>\s*)?<AIRPORT_STATUS_INFORMATION(?:\s|>)").unwrap().is_match(&head) {
        return Err(ApiError::new(502, "FAA returned no airport status XML"));
    }
    Ok(entry.into_response())
}

async fn metar(State(state): State<AppState>, Query(q): Q) -> ApiResult<Response> {
    let mut target = url("https://aviationweather.gov/api/data/metar");
    if let Some(bbox) = q.get("bbox") {
        let parts: Vec<f64> = bbox.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        if parts.len() != 4 {
            return Err(ApiError::bad_request("Use bbox=south,west,north,east"));
        }
        let (s, w, n, e) = ((parts[0] * 2.0).floor() / 2.0, (parts[1] * 2.0).floor() / 2.0, (parts[2] * 2.0).ceil() / 2.0, (parts[3] * 2.0).ceil() / 2.0);
        if s < -90.0 || n > 90.0 || w < -180.0 || e > 180.0 || n <= s || e <= w || n - s > 20.0 || e - w > 30.0 {
            return Err(ApiError::bad_request("Zoom in to a region under 20° tall and 30° wide for station plots"));
        }
        target.query_pairs_mut().append_pair("bbox", &format!("{s},{w},{n},{e}")).append_pair("format", "json");
    } else {
        let ids = q.get("stations").map(|s| s.to_ascii_uppercase()).unwrap_or_default();
        let mut stations: Vec<&str> = ids.split(',').filter(|s| !s.is_empty()).collect();
        let valid = Regex::new(r"^[A-Z][A-Z0-9]{3}$").unwrap();
        if stations.is_empty() || stations.len() > 8 || stations.iter().any(|id| !valid.is_match(id)) {
            return Err(ApiError::bad_request("Supply one to eight comma-separated ICAO station IDs"));
        }
        stations.sort();
        stations.dedup();
        target.query_pairs_mut().append_pair("ids", &stations.join(",")).append_pair("format", "json");
    }
    match state.fetch.get(target, LIVE, FetchOptions::default()).await {
        Ok(entry) => Ok(entry.into_response()),
        // aviationweather.gov answers 204 (empty body) when no station reported.
        Err(error) if error.upstream == Some(204) => Ok(([(header::CONTENT_TYPE, "application/json")], "[]").into_response()),
        Err(error) => Err(error),
    }
}

async fn qpf(State(state): State<AppState>, Query(q): Q) -> ApiResult<Response> {
    let layer = match q.get("days").map(String::as_str) {
        Some("1") => 1,
        Some("3") => 9,
        Some("7") => 11,
        _ => return Err(ApiError::bad_request("QPF period must be 1, 3 or 7 days")),
    };
    let mut pairs = vec![
        ("f".to_owned(), "geojson".to_owned()),
        ("outFields".into(), "qpf".into()),
        ("returnGeometry".into(), "true".into()),
        ("where".into(), "1=1".into()),
    ];
    generalize_qpf(&mut pairs);
    pairs.sort();
    let mut target = url(&format!("https://mapservices.weather.noaa.gov/vector/rest/services/precip/wpc_qpf/MapServer/{layer}/query"));
    target.query_pairs_mut().extend_pairs(pairs);
    Ok(state.fetch.get(target, LIVE, FetchOptions::default()).await?.into_response())
}

async fn drought(State(state): State<AppState>) -> ApiResult<Response> {
    let target = url(&format!("https://droughtmonitor.unl.edu{DROUGHT_PATH}"));
    Ok(state.fetch.get(target, 6 * HOUR, FetchOptions { max_bytes: MAX_LARGE_RESPONSE, ..Default::default() }).await?.into_response())
}

/// Validate and round a coordinate to 4 decimals ("38.3914").
pub fn coordinate(value: Option<&String>, name: &str, limit: f64) -> ApiResult<f64> {
    let raw = value.map(|s| s.trim()).unwrap_or_default();
    let ok = Regex::new(r"^-?\d{1,3}(?:\.\d{1,10})?$").unwrap().is_match(raw);
    let n: f64 = raw.parse().unwrap_or(f64::NAN);
    if !ok || !n.is_finite() || n.abs() > limit {
        return Err(ApiError::bad_request(format!("Invalid {name}")));
    }
    Ok((n * 10_000.0).round() / 10_000.0)
}

async fn nws_forecast(State(state): State<AppState>, Query(q): Q) -> ApiResult<Response> {
    let lat = coordinate(q.get("lat"), "latitude", 90.0)?;
    let lon = coordinate(q.get("lon"), "longitude", 180.0)?;
    let view = q.get("view").map(String::as_str).unwrap_or("daily");
    let suffix = match view {
        "daily" => "/forecast",
        "hourly" => "/forecast/hourly",
        "grid" => "",
        _ => return Err(ApiError::bad_request("View must be daily, hourly or grid")),
    };
    let (grid, x, y) = crate::weather::grid_point(&state, lat, lon).await?;
    let target = url(&format!("https://api.weather.gov/gridpoints/{grid}/{x},{y}{suffix}"));
    Ok(state.fetch.get(target, LIVE, FetchOptions::default()).await?.into_response())
}

fn climate_date(value: Option<&String>) -> ApiResult<chrono::NaiveDate> {
    value
        .and_then(|v| chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
        .filter(|d| value.is_some_and(|v| v.len() == 10 && d.format("%Y-%m-%d").to_string() == *v))
        .ok_or_else(|| ApiError::bad_request("Use YYYY-MM-DD dates"))
}

async fn climate_daily(State(state): State<AppState>, Query(q): Q) -> ApiResult<Response> {
    let station = q.get("station").cloned().unwrap_or_default();
    if !Regex::new(r"^[A-Z]{2}[A-Z0-9]{9}$").unwrap().is_match(&station) {
        return Err(ApiError::bad_request("Supply one GHCN-Daily station ID"));
    }
    let (start, end) = (climate_date(q.get("start"))?, climate_date(q.get("end"))?);
    let days = (end - start).num_days() + 1;
    if !(1..=366).contains(&days) {
        return Err(ApiError::bad_request("Request one to 366 days"));
    }
    let mut target = url("https://www.ncei.noaa.gov/access/services/data/v1");
    target.query_pairs_mut().extend_pairs([
        ("dataset", "daily-summaries"),
        ("stations", station.as_str()),
        ("startDate", &start.to_string()),
        ("endDate", &end.to_string()),
        ("dataTypes", "TMAX,TMIN,PRCP,SNOW"),
        ("includeAttributes", "true"),
        ("units", "standard"),
        ("format", "json"),
    ]);
    // Past days are final; today's row can still change.
    let ttl = if end < chrono::Utc::now().date_naive() - chrono::Duration::days(7) { DAY } else { LIVE };
    Ok(state.fetch.get(target, ttl, FetchOptions::default()).await?.into_response())
}

// ------------------------------------------------------------------------------------------
// NOAA model files (NOMADS grib filter): GFS, HRRR, RAP, NBM
// ------------------------------------------------------------------------------------------

const CYCLE: i64 = 6 * 3600;

/// The newest 6-hourly cycle with a full cycle of production time behind it.
pub fn model_run(now: i64) -> i64 {
    ((now - CYCLE) as f64 / CYCLE as f64).floor() as i64 * CYCLE
}

fn valid_lead(source: &str, lead: i64) -> bool {
    if lead < 1 {
        return false;
    }
    match source {
        "gfs" => lead <= 384 && (lead <= 120 || lead % 3 == 0),
        "hrrr" => lead <= 48,
        "rap" => lead <= 21,
        "nbm" => lead <= 264 && (lead <= 48 || if lead <= 192 { lead % 3 == 0 } else { lead % 6 == 0 }),
        _ => false,
    }
}

fn number(q: &HashMap<String, String>, name: &str) -> ApiResult<f64> {
    q.get(name)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|n| n.is_finite())
        .ok_or_else(|| ApiError::bad_request(format!("Invalid {name}")))
}

const RAPID_GRID: &[&str] = &[
    "lev_2_m_above_ground", "lev_10_m_above_ground", "lev_surface", "lev_mean_sea_level", "lev_entire_atmosphere",
    "var_TMP", "var_DPT", "var_RH", "var_UGRD", "var_VGRD", "var_GUST", "var_MSLMA", "var_TCDC", "var_APCP", "var_CAPE",
    "var_CRAIN", "var_CSNOW", "var_CFRZR", "var_CICEP",
];

pub fn model_url(q: &HashMap<String, String>, now: i64) -> ApiResult<url::Url> {
    let source = q.get("source").map(String::as_str).unwrap_or("gfs");
    if !["gfs", "hrrr", "nbm", "rap"].contains(&source) {
        return Err(ApiError::bad_request("Unsupported NOAA model"));
    }
    let run = number(q, "run")?;
    let lead = number(q, "lead")?;
    if run.fract() != 0.0 || (run as i64) % CYCLE != 0 || run as i64 > model_run(now) || (run as i64) < now - 72 * 3600 {
        return Err(ApiError::bad_request("Invalid GFS run"));
    }
    if lead.fract() != 0.0 || !valid_lead(source, lead as i64) {
        return Err(ApiError::bad_request("Invalid model lead"));
    }
    let (run, lead) = (run as i64, lead as i64);
    let south = (number(q, "south")? / 5.0).floor() * 5.0;
    let west = (number(q, "west")? / 5.0).floor() * 5.0;
    let north = (number(q, "north")? / 5.0).ceil() * 5.0;
    let east = (number(q, "east")? / 5.0).ceil() * 5.0;
    if south < -85.0 || north > 85.0 || west < -180.0 || east > 180.0 || north <= south || east <= west || north - south > 65.0 || east - west > 100.0 {
        return Err(ApiError::bad_request("Zoom in to a region under 100° wide and 65° tall for GFS"));
    }
    if source != "gfs" && (south < 15.0 || north > 60.0 || west < -135.0 || east > -55.0 || north - south > 20.0 || east - west > 30.0) {
        return Err(ApiError::bad_request("Zoom to a region within the continental U.S. model domain"));
    }
    let grid = q.get("fields").map(String::as_str) == Some("grid");
    if source == "nbm" && grid && (north - south > 15.0 || east - west > 20.0) {
        return Err(ApiError::bad_request("Zoom to a region under 20° wide and 15° tall for NBM"));
    }
    let time = chrono::DateTime::from_timestamp(run, 0).ok_or_else(|| ApiError::bad_request("Invalid GFS run"))?;
    let date = time.format("%Y%m%d").to_string();
    let hour = time.format("%H").to_string();
    let script = match source { "hrrr" => "hrrr_2d", "rap" => "rap", "nbm" => "blend", _ => "gfs_0p25" };
    let mut pairs: Vec<(String, String)> = vec![
        ("subregion".into(), String::new()),
        ("leftlon".into(), fmt_num(west)),
        ("rightlon".into(), fmt_num(east)),
        ("toplat".into(), fmt_num(north)),
        ("bottomlat".into(), fmt_num(south)),
    ];
    let on = |pairs: &mut Vec<(String, String)>, keys: &[&str]| pairs.extend(keys.iter().map(|k| (k.to_string(), "on".to_owned())));
    match source {
        "hrrr" | "rap" => {
            let (dir, file) = if source == "hrrr" {
                (format!("/hrrr.{date}/conus"), format!("hrrr.t{hour}z.wrfsfcf{lead:02}.grib2"))
            } else {
                (format!("/rap.{date}"), format!("rap.t{hour}z.awp130pgrbf{lead:02}.grib2"))
            };
            pairs.push(("dir".into(), dir));
            pairs.push(("file".into(), file));
            if grid { on(&mut pairs, RAPID_GRID) } else { on(&mut pairs, &["lev_entire_atmosphere", "var_REFC"]) }
        }
        "nbm" => {
            pairs.push(("dir".into(), format!("/blend.{date}/{hour}/core")));
            pairs.push(("file".into(), format!("blend.t{hour}z.core.f{lead:03}.co.grib2")));
            on(&mut pairs, &["lev_surface", "var_ASNOW", "var_FICEAC"]);
            if grid {
                on(&mut pairs, &["lev_2_m_above_ground", "lev_10_m_above_ground", "var_TMP", "var_DPT", "var_RH", "var_WIND", "var_WDIR", "var_GUST", "var_TCDC", "var_APCP", "var_CAPE", "var_PTYPE", "var_TSTM"]);
            }
        }
        _ => {
            pairs.push(("dir".into(), format!("/gfs.{date}/{hour}/atmos")));
            pairs.push(("file".into(), format!("gfs.t{hour}z.pgrb2.0p25.f{lead:03}")));
            on(&mut pairs, &["lev_surface", "var_PRATE", "var_CRAIN", "var_CSNOW", "var_CICEP", "var_CFRZR"]);
            match q.get("fields").map(String::as_str) {
                Some("standard") => on(&mut pairs, &["lev_2_m_above_ground", "var_TMP", "var_DPT", "var_RH", "lev_entire_atmosphere", "var_REFC", "var_APCP", "lev_10_m_above_ground", "var_UGRD", "var_VGRD", "var_GUST", "lev_mean_sea_level", "var_PRMSL"]),
                Some("grid") => on(&mut pairs, &[
                    "lev_2_m_above_ground", "lev_10_m_above_ground", "lev_mean_sea_level", "lev_entire_atmosphere", "lev_850_mb", "lev_700_mb", "lev_500_mb",
                    "lev_0C_isotherm", "lev_0-0.1_m_below_ground", "var_TMP", "var_RH", "var_UGRD", "var_VGRD", "var_GUST", "var_PRMSL", "var_TCDC", "var_APCP",
                    "var_CAPE", "var_VVEL", "var_HGT", "var_TSOIL", "var_SOILW",
                ]),
                _ => {}
            }
        }
    }
    let mut target = url(&format!("https://nomads.ncep.noaa.gov/cgi-bin/filter_{script}.pl"));
    target.query_pairs_mut().extend_pairs(pairs);
    Ok(target)
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 { format!("{}", n as i64) } else { n.to_string() }
}

async fn model_run_route() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "model": "GFS", "run": model_run(chrono::Utc::now().timestamp()) }))
}

async fn model_frame(State(state): State<AppState>, Query(q): Q) -> ApiResult<Response> {
    let target = model_url(&q, chrono::Utc::now().timestamp())?;
    // A finished run's file never changes.
    let options = FetchOptions { max_bytes: MAX_LARGE_RESPONSE, grib: true, timeout: Some(Duration::from_secs(60)), ..Default::default() };
    Ok(state.fetch.get(target, 12 * HOUR, options).await?.into_response())
}

fn archive_site(q: &HashMap<String, String>) -> ApiResult<String> {
    q.get("site")
        .filter(|s| Regex::new(r"^[A-Z0-9]{3}$").unwrap().is_match(s))
        .cloned()
        .ok_or_else(|| ApiError::bad_request("Invalid radar site"))
}

async fn archive_scans(State(state): State<AppState>, Query(q): Q) -> ApiResult<Response> {
    let site = archive_site(&q)?;
    let now = chrono::Utc::now().timestamp_millis();
    let end = (now - 900_000) / 300_000 * 300_000;
    let iso = |ms: i64| chrono::DateTime::from_timestamp_millis(ms).unwrap_or_default().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let mut target = url("https://mesonet.agron.iastate.edu/json/radar");
    target.query_pairs_mut().extend_pairs([
        ("operation", "list"),
        ("radar", site.as_str()),
        ("product", "N0S"),
        ("start", &iso(end - 10_800_000)),
        ("end", &iso(end)),
    ]);
    Ok(state.fetch.get(target, LIVE, FetchOptions::default()).await?.into_response())
}

async fn archive_velocity_tile(State(state): State<AppState>, Query(q): Q) -> ApiResult<Response> {
    let site = archive_site(&q)?;
    let time = number(&q, "time")?;
    let now = chrono::Utc::now().timestamp() as f64;
    if time.fract() != 0.0 || time > now || time < now - 30.0 * 86_400.0 {
        return Err(ApiError::bad_request("Invalid archive scan time"));
    }
    let (z, x, y) = (number(&q, "z")?, number(&q, "x")?, number(&q, "y")?);
    if z.fract() != 0.0 || !(0.0..=10.0).contains(&z) || [x, y].iter().any(|v| v.fract() != 0.0 || *v < 0.0 || *v >= 2f64.powf(z)) {
        return Err(ApiError::bad_request("Invalid archive tile coordinates"));
    }
    let edge = 20_037_508.342_789_244_f64;
    let size = edge * 2.0 / 2f64.powf(z);
    let date = chrono::DateTime::from_timestamp(time as i64, 0).unwrap_or_default();
    let mut target = url("https://mesonet.agron.iastate.edu/cgi-bin/wms/nexrad/ridge.cgi");
    target.query_pairs_mut().extend_pairs([
        ("BBOX", format!("{},{},{},{}", -edge + x * size, edge - (y + 1.0) * size, -edge + (x + 1.0) * size, edge - y * size).as_str()),
        ("SRS", "EPSG:3857"),
        ("TRANSPARENT", "TRUE"),
        ("format", "png"),
        ("height", "256"),
        ("width", "256"),
        ("layers", "single-archive"),
        ("prod", "N0S"),
        ("request", "GetMap"),
        ("sector", &site),
        ("service", "wms"),
        ("version", "1.1.1"),
        ("year", &date.format("%Y").to_string()),
        ("month", &date.format("%m").to_string()),
        ("day", &date.format("%d").to_string()),
        ("time", &date.format("%H%M").to_string()),
    ]);
    let entry = state.fetch.get(target, DAY, FetchOptions::default()).await?;
    if !entry.content_type.starts_with("image/png") {
        return Err(ApiError::new(502, "Archived radar scan unavailable"));
    }
    let mut response = entry.into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=86400"));
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_accepts_listed_paths_only() {
        let ok = feed_url("nws", "/alerts/active?area=IN&r=123", true).unwrap();
        assert_eq!(ok.as_str(), "https://api.weather.gov/alerts/active?area=IN");
        assert!(feed_url("nws", "/users/me", true).is_err());
        assert!(feed_url("nws", "//evil.example/alerts/active", true).is_err());
        assert!(feed_url("nws", "/alerts/active/../../x", true).is_err());
        assert!(feed_url("unknown", "/x", true).is_err());
        assert!(feed_url("meteo", "/v1/forecast?latitude=1&longitude=2", false).is_err());
        assert!(feed_url("meteo", "/v1/forecast?latitude=1&longitude=2", true).is_ok());
    }

    #[test]
    fn feed_sorts_query_and_generalizes_qpf() {
        let u = feed_url("noaaMap", "/vector/rest/services/precip/wpc_qpf/MapServer/1/query?where=1%3D1&f=geojson", true).unwrap();
        let q: Vec<_> = u.query_pairs().map(|(k, _)| k.into_owned()).collect();
        assert!(q.contains(&"maxAllowableOffset".to_string()));
        let mut sorted = q.clone();
        sorted.sort();
        assert_eq!(q, sorted);
    }

    #[test]
    fn drought_path_is_canonical() {
        let u = feed_url("drought", "/data/json/usdm_current.json", true).unwrap();
        assert_eq!(u.path(), DROUGHT_PATH);
    }

    #[test]
    fn model_runs_and_urls() {
        let now = 1_763_380_800; // 2025-11-17 12:00 UTC
        assert_eq!(model_run(now), now - 6 * 3600);
        let run = model_run(now);
        let mut q: HashMap<String, String> = [("run", run.to_string()), ("lead", "6".into()), ("south", "36".into()), ("west", "-90".into()), ("north", "41".into()), ("east", "-84".into())]
            .into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        let u = model_url(&q, now).unwrap();
        assert!(u.as_str().contains("filter_gfs_0p25.pl"));
        assert!(u.as_str().contains("gfs.t06z.pgrb2.0p25.f006"));
        q.insert("source".into(), "hrrr".into());
        assert!(model_url(&q, now).unwrap().as_str().contains("wrfsfcf06.grib2"));
        q.insert("lead".into(), "49".into());
        assert!(model_url(&q, now).is_err());
        q.insert("run".into(), (run + 6 * 3600).to_string());
        q.insert("lead".into(), "6".into());
        assert!(model_url(&q, now).is_err(), "future runs are refused");
    }

    #[test]
    fn coordinates_are_validated() {
        assert_eq!(coordinate(Some(&"38.391444".to_string()), "lat", 90.0).unwrap(), 38.3914);
        assert!(coordinate(Some(&"91".to_string()), "lat", 90.0).is_err());
        assert!(coordinate(Some(&"1e3".to_string()), "lat", 90.0).is_err());
        assert!(coordinate(None, "lat", 90.0).is_err());
    }
}
