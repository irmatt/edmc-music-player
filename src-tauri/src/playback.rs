use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use edmc_core::paths::audio_cache_dir;
use edmc_core::TrackStatus;
use edmc_source::TrackSource;
use serde_json::json;
use tauri::{AppHandle, Emitter};
use url::Url;

use crate::state::{AppState, RepeatMode};

fn cache_path(url: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    audio_cache_dir().join(format!("{:016x}.mp3", hasher.finish()))
}

async fn ensure_audio(state: &AppState, track: &edmc_core::Track) -> Result<PathBuf, String> {
    if let Some(path) = state.db.audio_path(&track.id).map_err(|e| e.to_string())? {
        if path.exists() {
            let _ = state.db.touch_audio(&track.id);
            return Ok(path);
        }
    }

    let audio_url = track
        .audio_url
        .as_ref()
        .ok_or_else(|| "This track has no audio file".to_string())?;
    let parsed = Url::parse(audio_url).map_err(|e| e.to_string())?;
    let destination = cache_path(audio_url);
    tokio::fs::create_dir_all(audio_cache_dir())
        .await
        .map_err(|e| e.to_string())?;

    let bytes = state
        .http
        .download_to_file(&parsed, &destination, None)
        .await
        .map_err(|e| e.to_string())?;
    state
        .db
        .register_audio(&track.id, &destination, bytes)
        .map_err(|e| e.to_string())?;

    let limit = (state.settings.lock().unwrap().cache_limit_mb as u64) * 1024 * 1024;
    let keep: HashSet<String> = [track.id.clone()].into_iter().collect();
    let _ = state.db.prune_audio(limit, &keep);

    Ok(destination)
}

async fn play_from_queue(
    app: &AppHandle,
    state: &Arc<AppState>,
    queue: &[edmc_core::Track],
    index: usize,
) -> Result<(), String> {
    let mut track = queue
        .get(index)
        .cloned()
        .ok_or_else(|| "Track not found".to_string())?;

    if track.audio_url.is_none() {
        let _ = app.emit("track-resolving", json!({ "index": index, "id": track.id }));
        track = state
            .source
            .resolve_track(&track)
            .await
            .map_err(|e| e.to_string())?;
        let _ = app.emit("track-changed", json!({ "index": index, "track": track }));
    }

    let path = ensure_audio(state, &track).await?;
    let path_for_task = path.clone();
    let analysed = tokio::task::spawn_blocking(move || edmc_audio::analyze(&path_for_task)).await;
    let (duration, peaks) = match analysed {
        Ok(Ok(result)) => result,
        _ => (track.duration_secs.unwrap_or(0.0), Vec::new()),
    };

    track.duration_secs = Some(duration);
    track.peaks = peaks;
    track.status = TrackStatus::Ready;
    let _ = state.db.save_track(&track);

    // Keep both the active playback snapshot and any matching browse row fresh.
    if let Some(item) = state.playback_queue.lock().unwrap().get_mut(index) {
        *item = track.clone();
    }
    if let Some(item) = state.queue.lock().unwrap().iter_mut().find(|item| item.id == track.id) {
        *item = track.clone();
    }
    *state.playback_index.lock().unwrap() = index;

    let volume = state.settings.lock().unwrap().volume;
    state.engine.set_volume(volume);
    state
        .engine
        .play_file(&path, Some(duration))
        .map_err(|e| e.to_string())?;

    *state.now_playing.lock().unwrap() = Some(track.clone());
    let _ = app.emit(
        "track-changed",
        json!({ "index": index, "track": track, "playing": true }),
    );
    Ok(())
}

/// Starts a fresh playback queue from the list the user clicked in.
pub async fn play_track(app: &AppHandle, state: &Arc<AppState>, index: usize) -> Result<(), String> {
    let queue = state.queue.lock().unwrap().clone();
    if index >= queue.len() {
        return Err("Track not found".into());
    }
    *state.playback_queue.lock().unwrap() = queue.clone();
    *state.playback_index.lock().unwrap() = index;
    *state.playback_origin.lock().unwrap() = state
        .current_forum
        .lock()
        .unwrap()
        .as_ref()
        .map(|forum| forum.url.clone());
    play_from_queue(app, state, &queue, index).await
}

fn ensure_playback_queue(state: &AppState) -> Vec<edmc_core::Track> {
    let active = state.playback_queue.lock().unwrap();
    if !active.is_empty() {
        return active.clone();
    }
    drop(active);
    let browse = state.queue.lock().unwrap().clone();
    if !browse.is_empty() {
        *state.playback_queue.lock().unwrap() = browse.clone();
        *state.playback_index.lock().unwrap() = *state.current_index.lock().unwrap();
        *state.playback_origin.lock().unwrap() = state
            .current_forum
            .lock()
            .unwrap()
            .as_ref()
            .map(|forum| forum.url.clone());
    }
    browse
}

pub async fn advance(app: &AppHandle, state: &Arc<AppState>, direction: i32) -> Result<(), String> {
    let queue = ensure_playback_queue(state);
    let length = queue.len();
    if length == 0 {
        return Ok(());
    }
    let current = *state.playback_index.lock().unwrap();
    let repeat = state.repeat.lock().unwrap().clone();

    if direction > 0 {
        if repeat == RepeatMode::One {
            return play_from_queue(app, state, &queue, current).await;
        }
        let next = if state.shuffle.load(std::sync::atomic::Ordering::Relaxed) && length > 1 {
            use rand::Rng;
            let offset = rand::rng().random_range(1..length);
            (current + offset) % length
        } else {
            current + 1
        };
        if next >= length {
            if repeat == RepeatMode::All {
                return play_from_queue(app, state, &queue, 0).await;
            }
            state.engine.stop();
            *state.now_playing.lock().unwrap() = None;
            let _ = app.emit("track-changed", json!({ "index": current, "playing": false }));
            return Ok(());
        }
        return play_from_queue(app, state, &queue, next).await;
    }

    let previous = if current == 0 {
        if repeat == RepeatMode::All {
            length - 1
        } else {
            0
        }
    } else {
        current - 1
    };
    play_from_queue(app, state, &queue, previous).await
}
