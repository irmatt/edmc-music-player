use serde::{Deserialize, Serialize};

use crate::paths::downloads_dir;

/// Every value the user may tune. The `source` crate applies hard floors to
/// the politeness values before use, so editing this file cannot turn the app
/// into a bulk scraper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub volume: f32,
    pub min_delay_ms: u64,
    pub max_per_host_per_hour: u32,
    pub max_per_session: u32,
    pub metadata_ttl_days: u64,
    pub cache_limit_mb: u64,
    pub prefetch_next: bool,
    pub media_keys: bool,
    pub download_dir: String,
    pub auto_like_on_download: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "wmp11".into(),
            volume: 0.8,
            min_delay_ms: 2_000,
            max_per_host_per_hour: 180,
            max_per_session: 500,
            metadata_ttl_days: 7,
            cache_limit_mb: 2_048,
            prefetch_next: false,
            media_keys: true,
            download_dir: downloads_dir().to_string_lossy().into_owned(),
            auto_like_on_download: false,
        }
    }
}
