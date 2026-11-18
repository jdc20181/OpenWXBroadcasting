use std::path::PathBuf;

use async_trait::async_trait;
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use reqwest::{Client, StatusCode};
use reqwest_middleware::ClientWithMiddleware;

use crate::io::source_client::{HttpClient, SourceFetchError};

/// CrowdedWX: concurrent tile requests allowed per host.
const PER_HOST: usize = 8;

/// CrowdedWX: one semaphore per host, shared by every map instance.
fn host_permits(url: &str) -> std::sync::Arc<tokio::sync::Semaphore> {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex, OnceLock},
    };
    static HOSTS: OnceLock<Mutex<HashMap<String, Arc<tokio::sync::Semaphore>>>> = OnceLock::new();
    let host = url
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("")
        .to_string();
    let mut hosts = HOSTS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    hosts
        .entry(host)
        .or_insert_with(|| Arc::new(tokio::sync::Semaphore::new(PER_HOST)))
        .clone()
}

#[derive(Clone)]
pub struct ReqwestHttpClient {
    client: ClientWithMiddleware,
}

impl From<reqwest::Error> for SourceFetchError {
    fn from(err: reqwest::Error) -> Self {
        SourceFetchError(Box::new(err))
    }
}

impl From<reqwest_middleware::Error> for SourceFetchError {
    fn from(err: reqwest_middleware::Error) -> Self {
        SourceFetchError(Box::new(err))
    }
}

impl ReqwestHttpClient {
    /// cache_path: Under which path should we cache requests.
    pub fn new<P>(cache_path: Option<P>) -> Self
    where
        P: Into<PathBuf>,
    {
        let mut builder = reqwest_middleware::ClientBuilder::new(Client::new());

        if let Some(cache_path) = cache_path {
            builder = builder.with(Cache(HttpCache {
                mode: CacheMode::Default,
                manager: CACacheManager {
                    path: cache_path.into(),
                },
                options: HttpCacheOptions::default(),
            }))
        }
        let client = builder.build();

        Self { client }
    }
}

#[cfg_attr(not(feature = "thread-safe-futures"), async_trait(?Send))]
#[cfg_attr(feature = "thread-safe-futures", async_trait)]
impl HttpClient for ReqwestHttpClient {
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, SourceFetchError> {
        // CrowdedWX: a radar loop asks for every tile of every frame at once (hundreds of
        // requests), which tile proxies answer with 502/503. Keep at most a few in flight per
        // host; failed tiles are retried later by the request system.
        let _permit = host_permits(url).acquire_owned().await;
        let response = self.client.get(url).send().await?;
        match response.error_for_status() {
            Ok(response) => {
                if response.status() == StatusCode::NOT_MODIFIED {
                    log::info!("Using data from cache");
                }

                let body = response.bytes().await?;

                Ok(Vec::from(body.as_ref()))
            }
            Err(e) => Err(SourceFetchError(Box::new(e))),
        }
    }
}
