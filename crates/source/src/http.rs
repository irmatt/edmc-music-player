use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

use edmc_core::db::Db;
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED, RETRY_AFTER};
use reqwest::cookie::Jar;
use url::Url;

pub type TransferProgress = Arc<dyn Fn(u64, u64) + Send + Sync>;

use crate::error::{Error, Result};
use crate::policy::{PolitenessPolicy, RateLimiter};

const USER_AGENT: &str = concat!(
    "EDMC-Music-Player/0.1 (+https://github.com/irmatt/edmc-music-player) ",
    "polite client; explicit user requests only"
);

pub struct GetResult {
    pub body: String,
    pub from_cache: bool,
}

/// The only component that performs network requests against the sites.
/// Every request goes through the limiter and the on-disk HTTP cache.
pub struct HttpClient {
    client: reqwest::Client,
    jar: Arc<Jar>,
    limiter: Arc<RateLimiter>,
    db: Arc<Db>,
    ttl_secs: u64,
}

impl HttpClient {
    pub fn new(db: Arc<Db>, policy: PolitenessPolicy) -> Result<Self> {
        let jar = Arc::new(Jar::default());
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .cookie_provider(jar.clone())
            .gzip(true)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            jar,
            limiter: Arc::new(RateLimiter::new(policy.clone())),
            db,
            ttl_secs: policy.clamped().metadata_ttl_secs,
        })
    }

    pub fn limiter(&self) -> Arc<RateLimiter> {
        self.limiter.clone()
    }

    pub fn jar(&self) -> Arc<Jar> {
        self.jar.clone()
    }

    pub fn cache_ttl_secs(&self) -> u64 {
        self.ttl_secs
    }

    /// Restores cookies captured from the in-app sign-in window.
    pub fn restore_session(&self, cookies: &str) -> Result<()> {
        let base = Url::parse("https://edmc.to/")?;
        for line in cookies.lines().map(str::trim).filter(|l| !l.is_empty()) {
            self.jar.add_cookie_str(line, &base);
        }
        Ok(())
    }

    /// Expire the cookies captured at sign-in. `reqwest`'s jar has no removal
    /// API, so we overwrite each stored cookie with an already-expired one.
    pub fn clear_session(&self, cookies: &str) -> Result<()> {
        let base = Url::parse("https://edmc.to/")?;
        for line in cookies.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let name = line.split('=').next().unwrap_or("").trim();
            if name.is_empty() {
                continue;
            }
            let expired = format!("{}=; Max-Age=0; Path=/; Domain=edmc.to", name);
            self.jar.add_cookie_str(&expired, &base);
        }
        Ok(())
    }

    /// GET a document, using the local cache first and conditional requests
    /// second. A `304` refreshes the timestamp without transferring a body.
    pub async fn get_text(&self, url: &Url) -> Result<GetResult> {
        self.fetch_document(url, false).await
    }

    /// Explicit Refresh: bypass the freshness window but still send conditional
    /// validators, so an unchanged page costs a `304`.
    pub async fn get_text_revalidated(&self, url: &Url) -> Result<GetResult> {
        self.fetch_document(url, true).await
    }

    async fn fetch_document(&self, url: &Url, force: bool) -> Result<GetResult> {
        ensure_allowed(url)?;
        let key = url.as_str();
        let now = edmc_core::db::now_secs();

        if !force {
            if let Some(cached) = self.db.get_http(key)? {
                let age = now - cached.fetched_at;
                if age >= 0 && (age as u64) < self.ttl_secs {
                    return Ok(GetResult {
                        body: String::from_utf8_lossy(&cached.body).into_owned(),
                        from_cache: true,
                    });
                }
            }
        }

        self.limiter.acquire(url).await?;

        let mut request = self.client.get(url.clone());
        if let Some(cached) = self.db.get_http(key)? {
            if let Some(etag) = cached.etag {
                request = request.header(IF_NONE_MATCH, etag);
            }
            if let Some(last_modified) = cached.last_modified {
                request = request.header(IF_MODIFIED_SINCE, last_modified);
            }
        }

        let response = request.send().await?;
        let status = response.status();

        if status.as_u16() == 429 || status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            let retry = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(Error::Busy(format!("retry after {}s", retry)));
        }

        if status == reqwest::StatusCode::NOT_MODIFIED {
            self.db.touch_http(key)?;
            let cached = self
                .db
                .get_http(key)?
                .ok_or_else(|| Error::Other("lost cache entry after 304".into()))?;
            return Ok(GetResult {
                body: String::from_utf8_lossy(&cached.body).into_owned(),
                from_cache: true,
            });
        }

        if !status.is_success() {
            return Err(Error::Other(format!("HTTP {}", status)));
        }

        let etag = header_string(response.headers().get(ETAG));
        let last_modified = header_string(response.headers().get(LAST_MODIFIED));
        let body = response.bytes().await?;
        self.db.put_http(key, body.as_ref(), etag.as_deref(), last_modified.as_deref())?;

        Ok(GetResult {
            body: String::from_utf8_lossy(&body).into_owned(),
            from_cache: false,
        })
    }

    /// Stream an audio file into the local cache. The request is still
    /// serialised and counted by the limiter; only the bytes differ.
    pub async fn download_to_file(
        &self,
        url: &Url,
        destination: &Path,
        progress: Option<TransferProgress>,
    ) -> Result<u64> {
        ensure_allowed(url)?;
        self.limiter.acquire(url).await?;
        let response = self.client.get(url.clone()).send().await?;
        if !response.status().is_success() {
            return Err(Error::Other(format!("HTTP {}", response.status())));
        }
        let total = response.content_length().unwrap_or(0);
        if let Some(callback) = &progress {
            callback(0, total);
        }
        stream_response(response, destination, total, progress).await
    }

    /// Resolve beatexs's original-file form and stream the resulting temporary
    /// `*.7btx.com` URL. The temporary URL is never stored or logged.
    pub async fn download_beatexs_original(
        &self,
        id: &str,
        destination: &Path,
        progress: Option<TransferProgress>,
    ) -> Result<u64> {
        let page = Url::parse(&format!("https://beatexs.com/{id}"))?;
        ensure_allowed(&page)?;
        self.limiter.acquire(&page).await?;
        let response = self
            .client
            .post(page)
            .form(&[
                ("op", "download2"),
                ("id", id),
                ("rand", ""),
                ("referer", ""),
                ("method_free", ""),
                ("method_premium", ""),
            ])
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(Error::Other(format!("HTTP {}", response.status())));
        }
        let final_url = response.url().clone();
        ensure_allowed(&final_url)?;
        if final_url.host_str().is_none_or(|host| {
            !host.ends_with("7btx.com") && !host.ends_with("flenxi.com")
        }) {
            return Err(Error::Other("beatexs did not return an original file URL".into()));
        }
        let total = response.content_length().unwrap_or(0);
        if let Some(callback) = &progress {
            callback(0, total);
        }
        stream_response(response, destination, total, progress).await
    }

    /// A one-off authenticated action (e.g. a Thanks reaction). Never cached.
    pub async fn action(&self, url: &Url) -> Result<()> {
        ensure_allowed(url)?;
        self.limiter.acquire(url).await?;
        let response = self.client.get(url.clone()).send().await?;
        if !response.status().is_success() {
            return Err(Error::Other(format!("HTTP {}", response.status())));
        }
        Ok(())
    }
}

async fn stream_response(
    response: reqwest::Response,
    destination: &Path,
    total: u64,
    progress: Option<TransferProgress>,
) -> Result<u64> {
    let mut file = tokio::fs::File::create(destination).await?;
    let mut stream = response.bytes_stream();
    let mut written = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
        if let Some(callback) = &progress {
            callback(written, total);
        }
    }
    file.flush().await?;
    Ok(written)
}

fn ensure_allowed(url: &Url) -> Result<()> {
    let host = url.host_str().ok_or_else(|| Error::Other("URL has no host".into()))?;
    let allowed = host == "edmc.to"
        || host == "beatexs.com"
        || host.ends_with(".flenxi.com")
        || host.ends_with(".7btx.com");
    if !allowed || url.scheme() != "https" {
        return Err(Error::Other(format!("Blocked URL host: {host}")));
    }
    Ok(())
}

fn header_string(value: Option<&reqwest::header::HeaderValue>) -> Option<String> {
    value.and_then(|v| v.to_str().ok()).map(str::to_owned)
}
