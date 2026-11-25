use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, Result};

/// Hard floors. Settings can raise these but never lower them.
pub const MIN_DELAY_MS: u64 = 1_000;
pub const MAX_PER_HOST_PER_HOUR: u32 = 180;
pub const MAX_PER_SESSION: u32 = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolitenessPolicy {
    pub min_delay_ms: u64,
    pub max_per_host_per_hour: u32,
    pub max_per_session: u32,
    pub metadata_ttl_secs: u64,
}

impl Default for PolitenessPolicy {
    fn default() -> Self {
        Self {
            min_delay_ms: 2_000,
            max_per_host_per_hour: MAX_PER_HOST_PER_HOUR,
            max_per_session: MAX_PER_SESSION,
            metadata_ttl_secs: 7 * 24 * 3600,
        }
    }
}

impl PolitenessPolicy {
    pub fn clamped(&self) -> Self {
        Self {
            min_delay_ms: self.min_delay_ms.max(MIN_DELAY_MS),
            max_per_host_per_hour: self.max_per_host_per_hour.min(MAX_PER_HOST_PER_HOUR),
            max_per_session: self.max_per_session.min(MAX_PER_SESSION),
            metadata_ttl_secs: self.metadata_ttl_secs,
        }
    }
}

#[derive(Default)]
struct Inner {
    hosts: HashMap<String, VecDeque<Instant>>,
    session_count: u32,
}

/// Serial, per-host rate limiter. `acquire` waits its turn rather than
/// allowing parallel bursts, and enforces both an hourly and a session cap.
pub struct RateLimiter {
    policy: PolitenessPolicy,
    inner: Mutex<Inner>,
}

impl RateLimiter {
    pub fn new(policy: PolitenessPolicy) -> Self {
        Self {
            policy: policy.clamped(),
            inner: Mutex::new(Inner::default()),
        }
    }

    pub async fn acquire(&self, url: &Url) -> Result<()> {
        let host = url.host_str().unwrap_or("unknown").to_owned();
        let hour = Duration::from_secs(3600);
        let delay = Duration::from_millis(self.policy.min_delay_ms);

        loop {
            let wait = {
                let mut inner = self.inner.lock().unwrap();
                let now = Instant::now();

                if inner.session_count >= self.policy.max_per_session {
                    return Err(Error::RateLimited);
                }

                let times = inner.hosts.entry(host.clone()).or_default();
                while times.front().is_some_and(|t| now.duration_since(*t) >= hour) {
                    times.pop_front();
                }

                if times.len() >= self.policy.max_per_host_per_hour as usize {
                    let oldest = *times.front().expect("non-empty queue");
                    Some(hour - now.duration_since(oldest))
                } else if let Some(last) = times.back() {
                    let elapsed = now.duration_since(*last);
                    if elapsed >= delay {
                        times.push_back(now);
                        inner.session_count += 1;
                        None
                    } else {
                        Some(delay - elapsed)
                    }
                } else {
                    times.push_back(now);
                    inner.session_count += 1;
                    None
                }
            };

            match wait {
                None => return Ok(()),
                Some(duration) => tokio::time::sleep(duration).await,
            }
        }
    }

    /// Called when the user starts a new explicit batch.
    pub fn reset_session(&self) {
        self.inner.lock().unwrap().session_count = 0;
    }
}
