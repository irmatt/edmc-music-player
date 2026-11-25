use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use edmc_core::{Forum, QualityVariant, Settings, Track, TrackBatch, UiPreferences};
use edmc_source::TrackSource;
use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

use crate::auth;
use crate::playback::{advance, play_index};
use crate::state::{AppState, FrontendState, RepeatMode};


#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "status", content = "value")]
pub enum DownloadOutcome {
    NeedsChoice(Vec<QualityVariant>),
    NeedsSignIn,
    Complete {
        path: String,
        warning: Option<String>,
    },
}

fn progress_emitter(app: &AppHandle) -> edmc_source::ProgressFn {
    let emitter = app.clone();
    Arc::new(move |done: usize, total: usize, title: &str| {
        let _ = emitter.emit(
            "load-progress",
            json!({ "done": done, "total": total, "title": title }),
        );
    })
}

const LIKED_URL: &str = "liked://tracks";

fn liked_forum() -> Forum {
    Forum::new("Liked Tracks".into(), LIKED_URL.into(), None)
}

fn set_liked_queue(state: &AppState) -> Result<Vec<Track>, String> {
    let tracks: Vec<Track> = state
        .db
        .all_tracks()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|track| track.thanked)
        .collect();
    *state.queue.lock().unwrap() = tracks.clone();
    *state.current_index.lock().unwrap() = 0;
    *state.current_forum.lock().unwrap() = Some(liked_forum());
    *state.total_pages.lock().unwrap() = 1;
    Ok(tracks)
}

fn reload_queue(state: &AppState, forum: &Forum) -> Result<Vec<Track>, String> {
    let show_hidden = state.prefs.lock().unwrap().show_hidden;
    state
        .db
        .tracks_for_forum(&forum.url, show_hidden)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_state(state: State<'_, Arc<AppState>>) -> FrontendState {
    state.snapshot()
}

#[tauri::command]
pub async fn list_tree(state: State<'_, Arc<AppState>>) -> Result<Vec<Forum>, String> {
    state
        .source
        .list_tree()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_children(
    forum: Forum,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<Forum>, String> {
    state
        .source
        .list_children(&forum)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn load_category(
    app: AppHandle,
    forum: Forum,
    state: State<'_, Arc<AppState>>,
) -> Result<TrackBatch, String> {
    let shared = Arc::clone(&state);
    let batch = shared
        .source
        .load_pages(&forum, 1, 5, progress_emitter(&app))
        .await
        .map_err(|error| error.to_string())?;
    *shared.current_forum.lock().unwrap() = Some(forum.clone());
    *shared.total_pages.lock().unwrap() = batch.total_pages.max(1);
    let tracks = reload_queue(&shared, &forum)?;
    *shared.queue.lock().unwrap() = tracks.clone();
    *shared.current_index.lock().unwrap() = 0;
    let _ = app.emit("tracks-loaded", json!({ "count": tracks.len() }));
    Ok(TrackBatch {
        tracks,
        loaded_pages: batch.loaded_pages,
        total_pages: batch.total_pages,
        errors: batch.errors,
    })
}

#[tauri::command]
pub fn search_tracks(
    query: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<Track>, String> {
    let needle = query.trim().to_lowercase();
    let tracks: Vec<Track> = state
        .db
        .all_tracks()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|track| {
            needle.is_empty()
                || track.title.to_lowercase().contains(&needle)
                || track.artist.to_lowercase().contains(&needle)
                || track.author.to_lowercase().contains(&needle)
                || track.forum.to_lowercase().contains(&needle)
        })
        .collect();
    *state.queue.lock().unwrap() = tracks.clone();
    *state.current_index.lock().unwrap() = 0;
    *state.current_forum.lock().unwrap() = Some(Forum::new(
        format!("Search: {query}"),
        format!("search://{query}"),
        None,
    ));
    *state.total_pages.lock().unwrap() = 1;
    let _ = app.emit("tracks-loaded", json!({ "count": tracks.len() }));
    Ok(tracks)
}

#[tauri::command]
pub fn load_liked(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<Track>, String> {
    let tracks = set_liked_queue(&state)?;
    let _ = app.emit("tracks-loaded", json!({ "count": tracks.len() }));
    Ok(tracks)
}

#[tauri::command]
pub fn reload_tracks(state: State<'_, Arc<AppState>>) -> Result<Vec<Track>, String> {
    if state
        .current_forum
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|forum| forum.url.starts_with("search://"))
    {
        return Ok(state.queue.lock().unwrap().clone());
    }
    if state
        .current_forum
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|forum| forum.url.starts_with("liked://"))
    {
        return set_liked_queue(&state);
    }
    let forum = state
        .current_forum
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "Choose a category first".to_string())?;
    let tracks = reload_queue(&state, &forum)?;
    *state.queue.lock().unwrap() = tracks.clone();
    Ok(tracks)
}

#[tauri::command]
pub async fn load_more(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<TrackBatch, String> {
    let shared = Arc::clone(&state);
    let forum = shared
        .current_forum
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "Choose a category first".to_string())?;
    if forum.url.starts_with("liked://") {
        let tracks = set_liked_queue(&shared)?;
        return Ok(TrackBatch {
            tracks,
            loaded_pages: Vec::new(),
            total_pages: 1,
            errors: Vec::new(),
        });
    }
    let loaded = shared.db.loaded_pages(&forum.url).map_err(|e| e.to_string())?;
    // Fill the first gap left by a failed page before moving forward.
    let start = (1u32..)
        .find(|page| !loaded.contains(page))
        .unwrap_or_else(|| loaded.iter().max().copied().unwrap_or(0) + 1);
    let batch = shared
        .source
        .load_pages(&forum, start, 5, progress_emitter(&app))
        .await
        .map_err(|error| error.to_string())?;
    *shared.total_pages.lock().unwrap() = batch.total_pages.max(1);
    let tracks = reload_queue(&shared, &forum)?;
    *shared.queue.lock().unwrap() = tracks.clone();
    let _ = app.emit("tracks-loaded", json!({ "count": tracks.len() }));
    Ok(TrackBatch {
        tracks,
        loaded_pages: batch.loaded_pages,
        total_pages: batch.total_pages,
        errors: batch.errors,
    })
}

#[tauri::command]
pub async fn refresh_category(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<TrackBatch, String> {
    let shared = Arc::clone(&state);
    let forum = shared
        .current_forum
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| "Choose a category first".to_string())?;
    if forum.url.starts_with("liked://") {
        let tracks = set_liked_queue(&shared)?;
        return Ok(TrackBatch {
            tracks,
            loaded_pages: Vec::new(),
            total_pages: 1,
            errors: Vec::new(),
        });
    }
    let batch = shared
        .source
        .refresh_page_one(&forum, progress_emitter(&app))
        .await
        .map_err(|error| error.to_string())?;
    *shared.total_pages.lock().unwrap() = batch.total_pages.max(1);
    let tracks = reload_queue(&shared, &forum)?;
    *shared.queue.lock().unwrap() = tracks.clone();
    let _ = app.emit("tracks-loaded", json!({ "count": tracks.len() }));
    Ok(TrackBatch {
        tracks,
        loaded_pages: batch.loaded_pages,
        total_pages: batch.total_pages,
        errors: batch.errors,
    })
}

#[tauri::command]
pub async fn play_track(
    app: AppHandle,
    index: usize,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    play_index(&app, &Arc::clone(&state), index).await
}

#[tauri::command]
pub fn toggle_play(state: State<'_, Arc<AppState>>) {
    state.engine.toggle();
}

#[tauri::command]
pub async fn next_track(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    advance(&app, &Arc::clone(&state), 1).await
}

#[tauri::command]
pub async fn previous_track(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    advance(&app, &Arc::clone(&state), -1).await
}

#[tauri::command]
pub fn seek(seconds: f64, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state
        .engine
        .seek(Duration::from_secs_f64(seconds.max(0.0)))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_volume(volume: f32, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let clamped = volume.clamp(0.0, 1.0);
    state.engine.set_volume(clamped);
    let mut settings = state.settings.lock().unwrap();
    settings.volume = clamped;
    state.db.save_settings(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_shuffle(enabled: bool, state: State<'_, Arc<AppState>>) {
    state
        .shuffle
        .store(enabled, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
pub fn set_repeat(mode: String, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let parsed = match mode.as_str() {
        "off" => RepeatMode::Off,
        "all" => RepeatMode::All,
        "one" => RepeatMode::One,
        other => return Err(format!("Unknown repeat mode: {other}")),
    };
    *state.repeat.lock().unwrap() = parsed;
    Ok(())
}

#[tauri::command]
pub fn play_next(index: usize, state: State<'_, Arc<AppState>>) {
    let mut queue = state.queue.lock().unwrap();
    if index >= queue.len() {
        return;
    }
    let current = *state.current_index.lock().unwrap();
    let track = queue.remove(index);
    let insert_at = if index <= current { current } else { current + 1 };
    let len = queue.len();
    queue.insert(insert_at.min(len), track);
    *state.current_index.lock().unwrap() = current.min(queue.len().saturating_sub(1));
}

#[tauri::command]
pub fn add_to_queue(index: usize, state: State<'_, Arc<AppState>>) {
    let mut queue = state.queue.lock().unwrap();
    if let Some(track) = queue.get(index).cloned() {
        queue.push(track);
    }
}

#[tauri::command]
pub fn remove_from_queue(index: usize, state: State<'_, Arc<AppState>>) {
    let mut queue = state.queue.lock().unwrap();
    if index >= queue.len() {
        return;
    }
    let current = *state.current_index.lock().unwrap();
    queue.remove(index);
    *state.current_index.lock().unwrap() = if index < current {
        current - 1
    } else if index == current {
        current.min(queue.len().saturating_sub(1))
    } else {
        current
    };
}

#[tauri::command]
pub fn reorder_queue(from: usize, to: usize, state: State<'_, Arc<AppState>>) {
    let mut queue = state.queue.lock().unwrap();
    if from >= queue.len() || to >= queue.len() || from == to {
        return;
    }
    let current = *state.current_index.lock().unwrap();
    let track = queue.remove(from);
    queue.insert(to, track);
    *state.current_index.lock().unwrap() = if from < current && to >= current {
        current - 1
    } else if from > current && to <= current {
        current + 1
    } else {
        current
    }
    .min(queue.len().saturating_sub(1));
}

fn find_track(state: &AppState, id: &str) -> Result<Track, String> {
    state
        .queue
        .lock()
        .unwrap()
        .iter()
        .find(|track| track.id == id)
        .cloned()
        .ok_or_else(|| "Track not found".into())
}

async fn like_inner(app: &AppHandle, state: &Arc<AppState>, id: &str) -> Result<(), String> {
    let mut track = find_track(state, id)?;
    if track.thanked {
        return Ok(());
    }
    if !state.signed_in() {
        return Err("SIGN_IN_REQUIRED".into());
    }
    if track.reaction_url.is_none() {
        track = state
            .source
            .resolve_track(&track)
            .await
            .map_err(|e| e.to_string())?;
    }
    let reaction = track
        .reaction_url
        .clone()
        .ok_or_else(|| "This post has no Thanks link".to_string())?;
    state
        .source
        .send_thanks(&reaction)
        .await
        .map_err(|e| e.to_string())?;
    track.thanked = true;
    state.db.save_track(&track).map_err(|e| e.to_string())?;
    if let Some(item) = state.queue.lock().unwrap().iter_mut().find(|t| t.id == id) {
        *item = track.clone();
    }
    let _ = app.emit("track-changed", json!({ "id": id, "thanked": true }));
    Ok(())
}

#[tauri::command]
pub async fn like_track(
    id: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    like_inner(&app, &Arc::clone(&state), &id).await
}

#[tauri::command]
pub fn hide_track(
    id: String,
    hidden: bool,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.db.set_hidden(&id, hidden).map_err(|e| e.to_string())?;
    if !hidden || state.prefs.lock().unwrap().show_hidden {
        if let Some(track) = state.queue.lock().unwrap().iter_mut().find(|t| t.id == id) {
            track.hidden = hidden;
        }
    } else {
        state.queue.lock().unwrap().retain(|track| track.id != id);
    }
    let _ = app.emit("tracks-loaded", json!({ "count": state.queue.lock().unwrap().len() }));
    Ok(())
}

#[tauri::command]
pub async fn get_qualities(
    id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<QualityVariant>, String> {
    let track = find_track(&state, &id)?;
    state
        .source
        .qualities(&track)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_track(
    id: String,
    quality_id: Option<String>,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<DownloadOutcome, String> {
    let shared = Arc::clone(&state);
    let mut track = find_track(&shared, &id)?;
    if track.beatexs_ids.is_empty() {
        track = shared
            .source
            .resolve_track(&track)
            .await
            .map_err(|e| e.to_string())?;
    }
    let qualities = shared
        .source
        .qualities(&track)
        .await
        .map_err(|e| e.to_string())?;
    let selected = match quality_id {
        Some(id) => qualities
            .iter()
            .find(|quality| quality.id == id)
            .cloned()
            .ok_or_else(|| "That quality is no longer available".to_string())?,
        None if qualities.len() > 1 => return Ok(DownloadOutcome::NeedsChoice(qualities)),
        None => qualities.first().cloned().ok_or_else(|| "No qualities found".to_string())?,
    };

    if shared.settings.lock().unwrap().auto_like_on_download && !shared.signed_in() {
        return Ok(DownloadOutcome::NeedsSignIn);
    }

    let directory = PathBuf::from(shared.settings.lock().unwrap().download_dir.clone());
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|e| e.to_string())?;
    let destination = unique_download_path(&directory, &selected.filename);

    let emitter = app.clone();
    let track_id = id.clone();
    let progress: edmc_source::TransferProgress = Arc::new(move |done, total| {
        let _ = emitter.emit(
            "download-progress",
            json!({ "id": track_id, "done": done, "total": total }),
        );
    });
    shared
        .http
        .download_beatexs_original(&selected.id, &destination, Some(progress))
        .await
        .map_err(|e| e.to_string())?;

    let mut warning = None;
    if shared.settings.lock().unwrap().auto_like_on_download && !track.thanked {
        if let Err(error) = like_inner(&app, &shared, &id).await {
            warning = Some(format!("Downloaded, but Thanks failed: {error}"));
        }
    }
    let _ = app.emit("download-complete", json!({ "id": id }));
    Ok(DownloadOutcome::Complete {
        path: destination.to_string_lossy().into_owned(),
        warning,
    })
}

#[tauri::command]
pub async fn open_post(
    id: String,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let track = find_track(&state, &id)?;
    app.opener()
        .open_url(track.post_url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_download_page(
    id: String,
    quality_id: Option<String>,
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<DownloadOutcome, String> {
    let track = find_track(&state, &id)?;
    let qualities = state
        .source
        .qualities(&track)
        .await
        .map_err(|e| e.to_string())?;
    let selected = match quality_id {
        Some(id) => qualities
            .iter()
            .find(|quality| quality.id == id)
            .cloned()
            .ok_or_else(|| "That quality is no longer available".to_string())?,
        None if qualities.len() > 1 => return Ok(DownloadOutcome::NeedsChoice(qualities)),
        None => qualities.first().cloned().ok_or_else(|| "No qualities found".to_string())?,
    };
    let page_url = selected.page_url.clone();
    app.opener()
        .open_url(page_url.clone(), None::<&str>)
        .map_err(|e| e.to_string())?;
    Ok(DownloadOutcome::Complete {
        path: page_url,
        warning: None,
    })
}

#[tauri::command]
pub fn copy_link(id: String, app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let track = find_track(&state, &id)?;
    app.clipboard()
        .write_text(track.post_url)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_ui_prefs(
    prefs: UiPreferences,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.db.save_ui_prefs(&prefs).map_err(|e| e.to_string())?;
    *state.prefs.lock().unwrap() = prefs;
    Ok(())
}

#[tauri::command]
pub fn set_data_dir(path: String, app: AppHandle) -> Result<(), String> {
    edmc_core::paths::save_portable_config(&edmc_core::paths::PortableConfig {
        data_dir: Some(path),
    })
    .map_err(|e| e.to_string())?;
    app.restart();
}

#[tauri::command]
pub fn use_default_data_dir(app: AppHandle) -> Result<(), String> {
    edmc_core::paths::save_portable_config(&edmc_core::paths::PortableConfig {
        data_dir: Some(
            edmc_core::paths::portable_default_dir()
                .to_string_lossy()
                .into_owned(),
        ),
    })
    .map_err(|e| e.to_string())?;
    app.restart();
}

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn update_settings(
    settings: Settings,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state
        .db
        .save_settings(&settings)
        .map_err(|e| e.to_string())?;
    state.engine.set_volume(settings.volume);
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
pub fn clear_cache(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.db.clear_audio_cache().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn watch_sign_in(app: AppHandle, state: State<'_, Arc<AppState>>) {
    auth::watch_sign_in(&app, &Arc::clone(&state));
}

#[tauri::command]
pub fn sign_out(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    auth::sign_out(&app, &Arc::clone(&state))
}

fn unique_download_path(directory: &PathBuf, filename: &str) -> PathBuf {
    let safe = sanitise_filename(filename);
    let first = directory.join(&safe);
    if !first.exists() {
        return first;
    }
    let (stem, extension) = safe
        .rsplit_once('.')
        .map(|(stem, ext)| (stem.to_owned(), format!(".{ext}")))
        .unwrap_or((safe.clone(), String::new()));
    for index in 1..10_000 {
        let candidate = directory.join(format!("{stem} ({index}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("{stem} (download){extension}"))
}

fn sanitise_filename(filename: &str) -> String {
    let cleaned: String = filename
        .chars()
        .map(|ch| if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') { '_' } else { ch })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_owned();
    if cleaned.is_empty() {
        "track.flac".into()
    } else {
        cleaned
    }
}
