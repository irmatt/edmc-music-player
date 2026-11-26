use async_trait::async_trait;
use edmc_core::{Forum, QualityVariant, Track, TrackBatch};

use crate::error::{Error, Result};
use crate::scrape::{PageFn, ProgressFn, TrackSource};

/// Placeholder for the official Invision Community API at `edmc.to/api/`.
/// It requires an admin-issued key; the trait is already in place so switching
/// sources is a configuration change rather than a rewrite.
pub struct ApiSource {
    api_key: String,
}

impl ApiSource {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }

    pub fn is_configured(&self) -> bool {
        !self.api_key.trim().is_empty()
    }
}

#[async_trait]
impl TrackSource for ApiSource {
    async fn list_tree(&self) -> Result<Vec<Forum>> {
        Err(Error::NotConfigured(
            "official API endpoints have not been mapped yet; using the careful HTML source".into(),
        ))
    }

    async fn list_children(&self, _forum: &Forum) -> Result<Vec<Forum>> {
        Err(Error::NotConfigured(
            "official API endpoints have not been mapped yet".into(),
        ))
    }

    async fn load_pages(
        &self,
        _forum: &Forum,
        _start_page: u32,
        _page_count: u32,
        _progress: ProgressFn,
        _on_page: PageFn,
    ) -> Result<TrackBatch> {
        Err(Error::NotConfigured(
            "official API endpoints have not been mapped yet".into(),
        ))
    }

    async fn refresh_page_one(
        &self,
        _forum: &Forum,
        _progress: ProgressFn,
        _on_page: PageFn,
    ) -> Result<TrackBatch> {
        Err(Error::NotConfigured(
            "official API endpoints have not been mapped yet".into(),
        ))
    }

    async fn resolve_track(&self, _track: &Track) -> Result<Track> {
        Err(Error::NotConfigured(
            "official API endpoints have not been mapped yet".into(),
        ))
    }

    async fn qualities(&self, _track: &Track) -> Result<Vec<QualityVariant>> {
        Err(Error::NotConfigured(
            "official API endpoints have not been mapped yet".into(),
        ))
    }

    async fn send_thanks(&self, _reaction_url: &str) -> Result<()> {
        Err(Error::NotConfigured(
            "official API endpoints have not been mapped yet".into(),
        ))
    }
}
