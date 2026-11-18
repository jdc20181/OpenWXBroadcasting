//! OpenWX Relay: the small backend between OpenWX Broadcasting and public weather data.
//!
//! It runs in two ways from the same code:
//! - **Local (bundled):** the desktop app starts it inside its own process on `127.0.0.1`.
//! - **Remote:** the `openwx-relay` server program on a station's own server, behind HTTPS, with
//!   an access token (`server/install.sh` sets it up).
//!
//! Live weather is never cached; identical simultaneous requests share one upstream call, and only
//! files that cannot change (finished model runs, basemap tiles, past radar frames, normals) are
//! kept in a small memory cache.

pub mod broadcaster;
pub mod climate;
pub mod config;
pub mod fetch;
pub mod prompt;
pub mod radar;
pub mod region;
pub mod weather;

use std::{net::SocketAddr, sync::Arc, time::Instant};

use axum::{
    body::Body,
    extract::State,
    http::{Method, Request, StatusCode, Uri},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use tower::{Layer, ServiceExt};

pub use config::{RelayConfig, DEFAULT_PORT};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RelayConfig>,
    pub fetch: Arc<fetch::Fetcher>,
    pub started: Instant,
}

impl AppState {
    pub fn new(config: RelayConfig) -> anyhow::Result<Self> {
        let fetch = fetch::Fetcher::new(&config.user_agent(), config.cache_mb, config.max_inflight)?;
        Ok(Self { config: Arc::new(config), fetch, started: Instant::now() })
    }
}

/// Every route the relay serves (before the access-token check).
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/openwx/check", get(check))
        .nest("/api/weatherbroadcaster", broadcaster::routes())
        .route("/api/weather/overview", get(weather::overview))
        .route("/api/weather/spc-outlooks", get(weather::spc_outlooks))
        .route("/api/radar", get(radar::radar))
        .route("/api/radar/storm-cells", get(radar::storm_cells))
        .route("/api/public/lightning.json", get(radar::lightning))
        .route("/__openwx_unauthorized", axum::routing::any(unauthorized))
        .fallback(not_here)
        .with_state(state)
}

async fn unauthorized() -> impl IntoResponse {
    (StatusCode::UNAUTHORIZED, Json(json!({ "error": "This relay needs its access token in the backend URL (https://host/t/<token>)" })))
}

/// Features the community edition leaves to plug-ins (road feeds, a website's own studio API)
/// answer 404, which the app treats as "not connected".
async fn not_here(uri: Uri) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Json(json!({ "error": format!("{} is not served by OpenWX Relay", uri.path()) })))
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "name": "OpenWX Relay",
        "version": VERSION,
        "uptimeSeconds": state.started.elapsed().as_secs(),
        "station": state.config.station,
        "contactSet": !state.config.contact.trim().is_empty(),
        "homeSet": state.config.home().is_some(),
        "radarUpstream": state.config.radar_upstream,
        "openMeteo": state.config.open_meteo,
        "tokenRequired": !state.config.access_token.is_empty(),
        "providers": broadcaster::PROVIDERS.iter().filter(|p| state.config.open_meteo || !p.open_meteo).map(|p| p.id).collect::<Vec<_>>(),
    }))
}

/// The feed check the setup wizard and `openwx status` show as a green/red list.
pub const CHECKS: &[(&str, &str)] = &[
    ("NWS alerts", "https://api.weather.gov/alerts/active/count"),
    ("SPC outlooks", "https://mapservices.weather.noaa.gov/vector/rest/services/outlooks/SPC_wx_outlks/MapServer/1/query?where=1%3D1&returnGeometry=false&f=json"),
    ("Radar & satellite", "{radar}/public/weather-maps.json"),
    ("USGS earthquakes", "https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/significant_month.geojson"),
    ("Iowa Environmental Mesonet", "https://mesonet.agron.iastate.edu/json/network.py?network=IA_ASOS"),
    ("NOAA climate (ACIS)", "https://data.rcc-acis.org/StnMeta?state=DE&meta=name&elems=maxt"),
    ("Space weather (SWPC)", "https://services.swpc.noaa.gov/products/noaa-scales.json"),
];

async fn check(State(state): State<AppState>) -> Json<Value> {
    let jobs = CHECKS.iter().map(|&(name, address)| {
        let state = state.clone();
        async move {
            let address = address.replace("{radar}", state.config.radar_upstream.trim_end_matches('/'));
            let start = Instant::now();
            let result = match url::Url::parse(&address) {
                Ok(target) => state.fetch.get(target, fetch::LIVE, fetch::FetchOptions::default()).await.map(|_| ()),
                Err(_) => Err(fetch::ApiError::new(500, "bad address")),
            };
            json!({ "name": name, "ok": result.is_ok(), "ms": start.elapsed().as_millis() as u64, "error": result.err().map(|e| e.message) })
        }
    });
    let results = futures_util::future::join_all(jobs).await;
    let ok = results.iter().all(|r| r["ok"] == true);
    Json(json!({ "ok": ok, "checks": results }))
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// With an access token set, every route except `/health` must arrive as `/t/<token>/...`.
/// The prefix is removed before routing, so the app's normal paths work unchanged.
fn authorize(token: &str, mut request: Request<Body>) -> Request<Body> {
    if request.method() != Method::GET && request.method() != Method::HEAD {
        *request.uri_mut() = Uri::from_static("/__openwx_unauthorized");
        return request;
    }
    if token.is_empty() || request.uri().path() == "/health" {
        return request;
    }
    let path = request.uri().path().to_owned();
    let rest = path.strip_prefix("/t/").and_then(|r| {
        let (given, tail) = r.split_once('/').unwrap_or((r, ""));
        constant_time_eq(given, token).then(|| format!("/{tail}"))
    });
    let target = match rest {
        Some(tail) => match request.uri().query() {
            Some(q) => format!("{tail}?{q}"),
            None => tail,
        },
        None => "/__openwx_unauthorized".into(),
    };
    if let Ok(uri) = target.parse() {
        *request.uri_mut() = uri;
    }
    request
}

/// A running relay: its address and a way to stop it.
pub struct RelayHandle {
    pub addr: SocketAddr,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl RelayHandle {
    /// `http://127.0.0.1:8737` style base URL for the app.
    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }
    pub fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

/// Bind and serve on the current Tokio runtime. When `try_next_ports` is set and the port is busy,
/// the next nine ports are tried (the desktop app's bundled relay).
pub async fn start(config: RelayConfig, try_next_ports: bool) -> anyhow::Result<RelayHandle> {
    config.validate()?;
    let mut listener = None;
    let tries = if try_next_ports { 10 } else { 1 };
    let mut last_error = None;
    for offset in 0..tries {
        let addr = format!("{}:{}", config.bind, config.port.saturating_add(offset));
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(l) => {
                listener = Some(l);
                break;
            }
            Err(e) => last_error = Some(anyhow::anyhow!("{addr}: {e}")),
        }
    }
    let listener = listener.ok_or_else(|| last_error.unwrap_or_else(|| anyhow::anyhow!("could not bind")))?;
    let addr = listener.local_addr()?;
    let token = config.access_token.clone();
    let app = router(AppState::new(config)?);
    let service = tower::util::MapRequestLayer::new(move |req| authorize(&token, req)).layer(app);
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let result = axum::serve(listener, axum::ServiceExt::<Request<Body>>::into_make_service(service))
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await;
        if let Err(error) = result {
            log::error!("relay stopped: {error}");
        }
    });
    log::info!("OpenWX Relay {VERSION} listening on http://{addr}");
    Ok(RelayHandle { addr, shutdown: Some(tx) })
}

/// For tests and embedding: answer one request without a socket.
pub async fn oneshot(state: AppState, request: Request<Body>) -> axum::response::Response {
    let token = state.config.access_token.clone();
    let app = router(state);
    app.oneshot(authorize(&token, request)).await.unwrap_or_else(|never| match never {})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(token: &str) -> AppState {
        AppState::new(RelayConfig { access_token: token.into(), ..Default::default() }).unwrap()
    }

    async fn status(state: AppState, path: &str) -> u16 {
        oneshot(state, Request::get(path).body(Body::empty()).unwrap()).await.status().as_u16()
    }

    #[tokio::test]
    async fn health_and_unknown_routes() {
        assert_eq!(status(state(""), "/health").await, 200);
        assert_eq!(status(state(""), "/api/indot/events").await, 404);
        assert_eq!(status(state(""), "/api/weatherbroadcaster/model-run").await, 200);
        assert_eq!(status(state(""), "/api/weatherbroadcaster/feed?provider=nws&resource=/users").await, 400);
    }

    #[tokio::test]
    async fn token_is_required_when_set() {
        let token = "a".repeat(48);
        assert_eq!(status(state(&token), "/health").await, 200);
        assert_eq!(status(state(&token), "/api/weatherbroadcaster/model-run").await, 401);
        assert_eq!(status(state(&token), &format!("/t/{}/api/weatherbroadcaster/model-run", "b".repeat(48))).await, 401);
        assert_eq!(status(state(&token), &format!("/t/{token}/api/weatherbroadcaster/model-run")).await, 200);
        assert_eq!(status(state(&token), &format!("/t/{token}/api/weatherbroadcaster/feed?provider=nws&resource=/users")).await, 400);
    }

    #[tokio::test]
    async fn only_get_is_allowed() {
        let response = oneshot(state(""), Request::post("/health").body(Body::empty()).unwrap()).await;
        assert_eq!(response.status().as_u16(), 401);
    }

    #[tokio::test]
    async fn serves_on_a_socket() {
        let handle = start(RelayConfig { port: 0, ..Default::default() }, false).await.unwrap();
        assert!(handle.base_url().starts_with("http://127.0.0.1:"));
        handle.stop();
    }
}
