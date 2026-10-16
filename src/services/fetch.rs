//! One way out for every request, to any source, so pacing is global: a search and
//! a description fetch started at the same time still go out one gap apart.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::Mutex;

use super::camoufox;
use crate::model::{Backend, Settings};

const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

#[derive(Debug)]
pub enum FetchError {
    /// 429, or LinkedIn's own 999. Stop and wait; retrying makes it last longer.
    RateLimited(u16),
    Status(u16),
    Network(String),
    Browser(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::RateLimited(c) => write!(f, "rate-limited (HTTP {c})"),
            FetchError::Status(c) => write!(f, "HTTP {c}"),
            FetchError::Network(e) => write!(f, "network error: {e}"),
            FetchError::Browser(e) => write!(f, "Camoufox: {e}"),
        }
    }
}

pub async fn get(url: &str, settings: &Settings) -> Result<String, FetchError> {
    pace(settings.delay_ms).await;
    match settings.backend {
        Backend::Http => http_get(url).await,
        Backend::Camoufox => camoufox::get(url, settings).await,
    }
}

/// Like `get`, but a successful response is reused for `ttl`. LinkedIn sends
/// `no-store` and no validators, so there is nothing to revalidate against:
/// the age is the only freshness signal. Kept in memory only, so a restart
/// always starts fresh. Returns whether the body came from the cache.
pub async fn get_cached(
    url: &str,
    ttl: Duration,
    settings: &Settings,
) -> Result<(String, bool), FetchError> {
    if let Some(body) = cache_lookup(url, ttl) {
        return Ok((body, true));
    }
    let body = get(url, settings).await?;
    cache_store(url, &body, ttl);
    Ok((body, false))
}

/// A JSON API, over plain HTTP whatever the backend: an API answer is not a
/// page a browser needs to render, and Camoufox only loads URLs. Paced like
/// every other request.
pub async fn get_json(url: &str, settings: &Settings) -> Result<String, FetchError> {
    pace(settings.delay_ms).await;
    send(
        client()
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json"),
    )
    .await
}

/// POSTs `body` as JSON; cached like `get_cached`, keyed by URL and body.
pub async fn post_json_cached(
    url: &str,
    body: &serde_json::Value,
    ttl: Duration,
    settings: &Settings,
) -> Result<(String, bool), FetchError> {
    let key = format!("POST {url} {body}");
    if let Some(cached) = cache_lookup(&key, ttl) {
        return Ok((cached, true));
    }
    pace(settings.delay_ms).await;
    let req = client()
        .post(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .json(body);
    let resp = send(req).await?;
    cache_store(&key, &resp, ttl);
    Ok((resp, false))
}

const CACHE_MAX_ENTRIES: usize = 200;

fn cache() -> &'static std::sync::Mutex<HashMap<String, (Instant, String)>> {
    static CACHE: OnceLock<std::sync::Mutex<HashMap<String, (Instant, String)>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn cache_lookup(url: &str, ttl: Duration) -> Option<String> {
    let cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    cache
        .get(url)
        .filter(|(at, _)| at.elapsed() < ttl)
        .map(|(_, body)| body.clone())
}

fn cache_store(url: &str, body: &str, ttl: Duration) {
    if ttl.is_zero() {
        return;
    }
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|_, (at, _)| at.elapsed() < ttl);
    if cache.len() >= CACHE_MAX_ENTRIES {
        if let Some(oldest) = cache
            .iter()
            .min_by_key(|(_, (at, _))| *at)
            .map(|(k, _)| k.clone())
        {
            cache.remove(&oldest);
        }
    }
    cache.insert(url.to_string(), (Instant::now(), body.to_string()));
}

pub fn classify(status: u16, body: String) -> Result<String, FetchError> {
    match status {
        200..=299 => Ok(body),
        429 | 999 => Err(FetchError::RateLimited(status)),
        s => Err(FetchError::Status(s)),
    }
}

async fn http_get(url: &str) -> Result<String, FetchError> {
    send(client().get(url)).await
}

async fn send(req: reqwest::RequestBuilder) -> Result<String, FetchError> {
    let resp = req
        .send()
        .await
        .map_err(|e| FetchError::Network(e.to_string()))?;
    let status = resp.status().as_u16();
    let body = resp
        .text()
        .await
        .map_err(|e| FetchError::Network(e.to_string()))?;
    classify(status, body)
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
                .parse()
                .unwrap(),
        );
        headers.insert(
            reqwest::header::ACCEPT_LANGUAGE,
            "en-US,en;q=0.9".parse().unwrap(),
        );
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .expect("HTTP client builds")
    })
}

/// Waits until `delay_ms` (plus jitter) has passed since the previous request.
async fn pace(delay_ms: u64) {
    static LAST: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    let mut last = LAST.get_or_init(|| Mutex::new(None)).lock().await;
    if let Some(at) = *last {
        let gap = Duration::from_millis(delay_ms + jitter(delay_ms / 2));
        if let Some(wait) = gap.checked_sub(at.elapsed()) {
            tokio::time::sleep(wait).await;
        }
    }
    *last = Some(Instant::now());
}

fn jitter(max: u64) -> u64 {
    if max == 0 {
        return 0;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    nanos % max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_hits_within_ttl_only() {
        let url = "test://cache-hits";
        let ttl = Duration::from_millis(80);
        assert_eq!(cache_lookup(url, ttl), None);
        cache_store(url, "body", ttl);
        assert_eq!(cache_lookup(url, ttl).as_deref(), Some("body"));
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(cache_lookup(url, ttl), None);
    }

    #[test]
    fn zero_ttl_never_stores() {
        let url = "test://zero-ttl";
        cache_store(url, "body", Duration::ZERO);
        assert_eq!(cache_lookup(url, Duration::from_secs(60)), None);
    }
}
