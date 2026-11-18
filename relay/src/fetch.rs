//! Upstream fetching: one pooled client, identical simultaneous requests share one upstream call,
//! bounded response sizes, a content-type allow list, and a small memory cache used ONLY for
//! files that cannot change (finished model runs, basemap tiles, past radar frames, normals).

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use futures_util::{future::{BoxFuture, Shared}, FutureExt, StreamExt};

pub const MAX_RESPONSE: usize = 8 * 1024 * 1024;
pub const MAX_LARGE_RESPONSE: usize = 32 * 1024 * 1024;
/// Live weather: never cached (identical simultaneous requests still share one call).
pub const LIVE: Duration = Duration::ZERO;
pub const HOUR: Duration = Duration::from_secs(3600);
pub const DAY: Duration = Duration::from_secs(86_400);

#[derive(Clone, Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
    pub retry_after: Option<u64>,
    /// The upstream's own status, when it answered with an error.
    pub upstream: Option<u16>,
}

impl ApiError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
            message: message.into(),
            retry_after: None,
            upstream: None,
        }
    }
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, message)
    }
    pub fn retry(mut self, seconds: u64) -> Self {
        self.retry_after = Some(seconds);
        self
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, axum::Json(serde_json::json!({ "error": self.message }))).into_response();
        if let Some(seconds) = self.retry_after {
            if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
        }
        response
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Clone, Debug)]
pub struct Entry {
    pub body: Bytes,
    pub content_type: String,
    expires: Instant,
}

impl Entry {
    pub fn json(&self) -> ApiResult<serde_json::Value> {
        serde_json::from_slice(&self.body).map_err(|_| ApiError::new(502, "Weather provider sent unreadable JSON"))
    }
}

impl IntoResponse for Entry {
    fn into_response(self) -> Response {
        let mut response = self.body.into_response();
        if let Ok(value) = HeaderValue::from_str(&self.content_type) {
            response.headers_mut().insert(header::CONTENT_TYPE, value);
        }
        response
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FetchOptions {
    pub max_bytes: usize,
    /// NOMADS model files: must start with "GRIB".
    pub grib: bool,
    pub accept: Option<&'static str>,
    pub timeout: Option<Duration>,
}

type Pending = Shared<BoxFuture<'static, ApiResult<Entry>>>;

pub struct Fetcher {
    client: reqwest::Client,
    cache: Mutex<Cache>,
    inflight: Mutex<HashMap<String, Pending>>,
    max_inflight: usize,
}

struct Cache {
    map: HashMap<String, Entry>,
    order: VecDeque<String>,
    bytes: usize,
    limit: usize,
}

impl Cache {
    fn get(&mut self, key: &str) -> Option<Entry> {
        let hit = self.map.get(key)?;
        if hit.expires > Instant::now() {
            return Some(hit.clone());
        }
        if let Some(old) = self.map.remove(key) {
            self.bytes -= old.body.len();
        }
        None
    }

    fn put(&mut self, key: String, entry: Entry) {
        if entry.body.len() > self.limit / 4 {
            return;
        }
        if let Some(old) = self.map.remove(&key) {
            self.bytes -= old.body.len();
        }
        self.bytes += entry.body.len();
        self.order.push_back(key.clone());
        self.map.insert(key, entry);
        while self.bytes > self.limit || self.map.len() > 1500 {
            let Some(oldest) = self.order.pop_front() else { break };
            if let Some(old) = self.map.remove(&oldest) {
                self.bytes -= old.body.len();
            }
        }
    }
}

fn allowed_type(content_type: &str) -> bool {
    let t = content_type.to_ascii_lowercase();
    let t = t.split(';').next().unwrap_or("").trim();
    t.starts_with("application/") && (t.ends_with("json") || matches!(t, "application/zip" | "application/gzip" | "application/x-gzip" | "application/xml" | "application/octet-stream" | "application/vnd.google-earth.kml+xml"))
        || matches!(t, "image/png" | "image/jpeg" | "image/webp" | "image/gif")
        || matches!(t, "text/plain" | "text/csv" | "text/xml" | "text/html")
}

impl Fetcher {
    pub fn new(user_agent: &str, cache_mb: u64, max_inflight: usize) -> anyhow::Result<Arc<Self>> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(16)
            .gzip(true)
            .build()?;
        Ok(Arc::new(Self {
            client,
            cache: Mutex::new(Cache { map: HashMap::new(), order: VecDeque::new(), bytes: 0, limit: (cache_mb.max(4) as usize) * 1024 * 1024 }),
            inflight: Mutex::new(HashMap::new()),
            max_inflight: max_inflight.max(4),
        }))
    }

    /// GET `url`, sharing the call with identical requests in flight. `ttl` > 0 only for files
    /// that cannot change.
    pub async fn get(self: &Arc<Self>, url: url::Url, ttl: Duration, options: FetchOptions) -> ApiResult<Entry> {
        let key = url.as_str().to_owned();
        if let Some(hit) = self.cache.lock().unwrap().get(&key) {
            return Ok(hit);
        }
        let pending = {
            let mut inflight = self.inflight.lock().unwrap();
            if let Some(pending) = inflight.get(&key) {
                pending.clone()
            } else {
                if inflight.len() >= self.max_inflight {
                    return Err(ApiError::new(503, "The relay is busy").retry(5));
                }
                let this = Arc::clone(self);
                let job: BoxFuture<'static, ApiResult<Entry>> = Box::pin(async move { this.fetch(url, ttl, options).await });
                let shared = job.shared();
                inflight.insert(key.clone(), shared.clone());
                shared
            }
        };
        let result = pending.await;
        self.inflight.lock().unwrap().remove(&key);
        if let Ok(entry) = &result {
            if !ttl.is_zero() {
                self.cache.lock().unwrap().put(key, entry.clone());
            }
        }
        result
    }

    async fn fetch(&self, url: url::Url, ttl: Duration, options: FetchOptions) -> ApiResult<Entry> {
        let accept = options.accept.unwrap_or(if url.host_str() == Some("api.weather.gov") {
            "application/geo+json, application/json"
        } else {
            "*/*"
        });
        let mut request = self.client.get(url.clone()).header(reqwest::header::ACCEPT, accept);
        if let Some(timeout) = options.timeout {
            request = request.timeout(timeout);
        }
        let response = request.send().await.map_err(|error| {
            log::warn!("upstream unreachable: {} ({error})", url.host_str().unwrap_or("?"));
            ApiError::new(502, "Weather provider unavailable")
        })?;
        let status = response.status();
        if !status.is_success() {
            let retry = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok());
            let mut error = ApiError::new(if status.as_u16() == 429 { 429 } else { 502 }, format!("Weather provider returned {}", status.as_u16()));
            error.upstream = Some(status.as_u16());
            error.retry_after = retry;
            return Err(error);
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_owned();
        let max = if options.max_bytes == 0 { MAX_RESPONSE } else { options.max_bytes };
        if response.content_length().is_some_and(|n| n as usize > max) {
            return Err(ApiError::new(502, "Upstream response is too large"));
        }
        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| ApiError::new(502, "Weather provider connection dropped"))?;
            if body.len() + chunk.len() > max {
                return Err(ApiError::new(502, "Upstream response is too large"));
            }
            body.extend_from_slice(&chunk);
        }
        let content_type = if options.grib {
            if !body.starts_with(b"GRIB") {
                return Err(ApiError::new(502, "NOAA returned no GRIB model data"));
            }
            "application/octet-stream".to_owned()
        } else {
            if !allowed_type(&content_type) {
                return Err(ApiError::new(502, "Unexpected weather provider response type"));
            }
            content_type
        };
        Ok(Entry { body: Bytes::from(body), content_type, expires: Instant::now() + ttl })
    }
}

/// Parse a fixed upstream URL (only ever built from constants in this crate).
pub fn url(address: &str) -> url::Url {
    url::Url::parse(address).expect("static upstream URL")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_allow_list() {
        assert!(allowed_type("application/geo+json"));
        assert!(allowed_type("application/json; charset=utf-8"));
        assert!(allowed_type("image/webp"));
        assert!(allowed_type("text/xml"));
        assert!(!allowed_type("application/javascript"));
        assert!(!allowed_type("image/svg+xml"));
    }

    #[test]
    fn cache_evicts_oldest() {
        let mut cache = Cache { map: HashMap::new(), order: VecDeque::new(), bytes: 0, limit: 40 };
        let entry = |n: usize| Entry { body: Bytes::from(vec![0u8; n]), content_type: "image/png".into(), expires: Instant::now() + DAY };
        cache.put("a".into(), entry(10));
        cache.put("b".into(), entry(10));
        cache.put("c".into(), entry(10));
        cache.put("d".into(), entry(10));
        cache.put("e".into(), entry(10));
        assert!(cache.get("a").is_none());
        assert!(cache.get("e").is_some());
        assert!(cache.bytes <= 40);
    }
}
