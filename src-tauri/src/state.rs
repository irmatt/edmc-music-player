use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use edmc_audio::AudioEngine;
use edmc_core::db::Db;
use edmc_core::{Forum, Settings, Track, UiPreferences};
use edmc_source::{HtmlSource, HttpClient};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    Off,
    All,
    One,
}

pub struct AppState {
    pub db: Arc<Db>,
    pub http: Arc<HttpClient>,
    pub source: Arc<HtmlSource>,
    pub engine: Arc<AudioEngine>,
    pub settings: Mutex<Settings>,
    pub prefs: Mutex<UiPreferences>,
    pub queue: Mutex<Vec<Track>>,
    pub current_index: Mutex<usize>,
    /// Snapshot of the list that started playback; browsing must not change it.
    pub playback_queue: Mutex<Vec<Track>>,
    pub playback_index: Mutex<usize>,
    pub playback_origin: Mutex<Option<String>>,
    pub current_forum: Mutex<Option<Forum>>,
    pub now_playing: Mutex<Option<Track>>,
    pub total_pages: Mutex<u32>,
    pub shuffle: AtomicBool,
    pub repeat: Mutex<RepeatMode>,
    pub portable: bool,
    pub needs_data_dir: bool,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FrontendState {
    pub tracks: Vec<Track>,
    pub current_index: usize,
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub volume: f32,
    pub signed_in: bool,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub settings: Settings,
    pub prefs: UiPreferences,
    pub current_forum: Option<Forum>,
    pub now_playing: Option<Track>,
    pub loaded_pages: Vec<u32>,
    pub total_pages: u32,
    pub liked_count: usize,
    pub portable: bool,
    pub needs_data_dir: bool,
    pub data_dir: String,
}

impl AppState {
    pub fn new(
        db: Arc<Db>,
        http: Arc<HttpClient>,
        source: Arc<HtmlSource>,
        engine: Arc<AudioEngine>,
        settings: Settings,
        prefs: UiPreferences,
        portable: bool,
        needs_data_dir: bool,
        data_dir: String,
    ) -> Self {
        Self {
            db,
            http,
            source,
            engine,
            settings: Mutex::new(settings),
            prefs: Mutex::new(prefs),
            queue: Mutex::new(Vec::new()),
            current_index: Mutex::new(0),
            playback_queue: Mutex::new(Vec::new()),
            playback_index: Mutex::new(0),
            playback_origin: Mutex::new(None),
            current_forum: Mutex::new(None),
            now_playing: Mutex::new(None),
            total_pages: Mutex::new(1),
            shuffle: AtomicBool::new(false),
            repeat: Mutex::new(RepeatMode::Off),
            portable,
            needs_data_dir,
            data_dir,
        }
    }

    pub fn signed_in(&self) -> bool {
        self.db
            .get_session()
            .ok()
            .flatten()
            .is_some_and(|cookies| !cookies.trim().is_empty())
    }

    pub fn snapshot(&self) -> FrontendState {
        let tracks = self.queue.lock().unwrap().clone();
        let current_forum = self.current_forum.lock().unwrap().clone();
        let loaded_pages = current_forum
            .as_ref()
            .and_then(|forum| self.db.loaded_pages(&forum.url).ok())
            .unwrap_or_default();
        let total_pages = *self.total_pages.lock().unwrap();
        FrontendState {
            tracks,
            current_index: *self.current_index.lock().unwrap(),
            playing: self.engine.state().is_some() && !self.engine.is_paused(),
            position: self.engine.position().as_secs_f64(),
            duration: self.engine.duration().as_secs_f64(),
            volume: self.engine.volume(),
            signed_in: self.signed_in(),
            shuffle: self.shuffle.load(std::sync::atomic::Ordering::Relaxed),
            repeat: self.repeat.lock().unwrap().clone(),
            settings: self.settings.lock().unwrap().clone(),
            prefs: self.prefs.lock().unwrap().clone(),
            current_forum,
            now_playing: self.now_playing.lock().unwrap().clone(),
            loaded_pages,
            total_pages,
            liked_count: self
                .db
                .all_tracks()
                .map(|tracks| tracks.iter().filter(|track| track.thanked).count())
                .unwrap_or(0),
            portable: self.portable,
            needs_data_dir: self.needs_data_dir,
            data_dir: self.data_dir.clone(),
        }
    }
}
