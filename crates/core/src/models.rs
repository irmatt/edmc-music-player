use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrackStatus {
    #[default]
    Pending,
    Resolving,
    Ready,
    NoAudio,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Forum {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub parent_url: Option<String>,
    #[serde(default)]
    pub children: Vec<Forum>,
}

impl Forum {
    pub fn new(name: String, url: String, parent_url: Option<String>) -> Self {
        Self {
            name,
            url,
            parent_url,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// The forum post URL is the stable identity of a track.
    pub id: String,
    pub title: String,
    pub artist: String,
    pub author: String,
    pub uploaded_at: String,
    pub replies: i64,
    pub views: i64,
    pub thanks_count: i64,
    pub thumb_url: Option<String>,
    pub forum: String,
    pub forum_url: String,
    pub post_url: String,
    /// All beatexs file IDs embedded in the post, in page order.
    #[serde(default)]
    pub beatexs_ids: Vec<String>,
    pub beatexs_url: Option<String>,
    pub reaction_url: Option<String>,
    /// Lightweight beatexs preview (normally 96 kbps MP3).
    pub audio_url: Option<String>,
    pub art_url: Option<String>,
    pub duration_secs: Option<f64>,
    #[serde(default)]
    pub peaks: Vec<f32>,
    pub thanked: bool,
    pub hidden: bool,
    pub status: TrackStatus,
    pub updated_at: i64,
}

impl Track {
    pub fn from_listing(
        post_url: String,
        full_title: String,
        forum: &Forum,
        author: String,
        uploaded_at: String,
        replies: i64,
        views: i64,
        thanks_count: i64,
        thumb_url: Option<String>,
    ) -> Self {
        let (artist, title) = split_artist_title(&full_title);
        Self {
            id: post_url.clone(),
            title,
            artist,
            author,
            uploaded_at,
            replies,
            views,
            thanks_count,
            thumb_url,
            forum: forum.name.clone(),
            forum_url: forum.url.clone(),
            post_url,
            beatexs_ids: Vec::new(),
            beatexs_url: None,
            reaction_url: None,
            audio_url: None,
            art_url: None,
            duration_secs: None,
            peaks: Vec::new(),
            thanked: false,
            hidden: false,
            status: TrackStatus::Pending,
            updated_at: 0,
        }
    }
}

pub fn split_artist_title(full: &str) -> (String, String) {
    let full = full.trim();
    if let Some((artist, title)) = full.split_once(" - ") {
        if !artist.trim().is_empty() && !title.trim().is_empty() {
            return (artist.trim().to_owned(), title.trim().to_owned());
        }
    }
    (String::new(), full.to_owned())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityVariant {
    pub id: String,
    pub filename: String,
    pub format: String,
    pub quality_label: String,
    pub size_bytes: u64,
    pub lossless: bool,
    pub page_url: String,
    pub score: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPreferences {
    pub show_hidden: bool,
    pub row_scale: f32,
    pub artist_first: bool,
    pub sort_by: String,
    pub descending: bool,
    pub columns: Vec<String>,
    pub column_widths: BTreeMap<String, u32>,
    pub favorites: Vec<String>,
    pub collapsed: Vec<String>,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            show_hidden: false,
            row_scale: 1.0,
            artist_first: false,
            sort_by: "uploaded".into(),
            descending: true,
            columns: vec![
                "thumb".into(),
                "track".into(),
                "artist".into(),
                "uploaded".into(),
                "thanks".into(),
                "comments".into(),
                "views".into(),
                "author".into(),
                "forum".into(),
                "duration".into(),
                "like".into(),
                "download".into(),
            ],
            column_widths: BTreeMap::new(),
            favorites: Vec::new(),
            collapsed: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackBatch {
    pub tracks: Vec<Track>,
    pub loaded_pages: Vec<u32>,
    pub total_pages: u32,
    pub errors: Vec<String>,
}
