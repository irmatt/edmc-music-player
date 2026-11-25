#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod commands;
mod playback;
mod state;

use std::sync::Arc;
use std::time::{Duration, Instant};

use edmc_audio::AudioEngine;
use edmc_core::db::Db;
use edmc_core::Settings;
use edmc_source::{HtmlSource, HttpClient, PolitenessPolicy};
use serde_json::json;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::playback::advance;
use crate::state::AppState;

fn policy_from(settings: &Settings) -> PolitenessPolicy {
    PolitenessPolicy {
        min_delay_ms: settings.min_delay_ms,
        max_per_host_per_hour: settings.max_per_host_per_hour,
        max_per_session: settings.max_per_session,
        metadata_ttl_secs: settings.metadata_ttl_days * 24 * 3600,
    }
}

fn create_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Show / Hide", true, None::<&str>)?;
    let play = MenuItem::with_id(app, "play", "Play / Pause", true, None::<&str>)?;
    let previous = MenuItem::with_id(app, "previous", "Previous", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "Next", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &toggle,
            &play,
            &previous,
            &next,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("EDMC Music Player")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let state = Arc::clone(&*app.state::<Arc<AppState>>());
            match event.id.as_ref() {
                "quit" => app.exit(0),
                "toggle" => {
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
                "play" => state.engine.toggle(),
                "next" | "previous" => {
                    let direction = if event.id.as_ref() == "next" { 1 } else { -1 };
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = advance(&handle, &state, direction).await;
                    });
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn register_shortcuts(app: &tauri::AppHandle) {
    const SHORTCUTS: &[&str] = &[
        "MediaPlayPause",
        "MediaTrackNext",
        "MediaTrackPrevious",
        "Ctrl+Shift+Space",
    ];
    for &shortcut in SHORTCUTS {
        let _ = app.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let state = Arc::clone(&*app.state::<Arc<AppState>>());
            match shortcut {
                "MediaPlayPause" | "Ctrl+Shift+Space" => state.engine.toggle(),
                "MediaTrackNext" => {
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = advance(&handle, &state, 1).await;
                    });
                }
                "MediaTrackPrevious" => {
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = advance(&handle, &state, -1).await;
                    });
                }
                _ => {}
            }
        });
    }
}

fn start_ticker(app: &tauri::AppHandle, state: Arc<AppState>) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut last_advance = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;

            let has_track = state.engine.state().is_some();
            if !has_track {
                continue;
            }
            let position = state.engine.position().as_secs_f64();
            let duration = state.engine.duration().as_secs_f64();
            let playing = !state.engine.is_paused();
            let index = *state.current_index.lock().unwrap();

            let _ = handle.emit(
                "playback-tick",
                json!({
                    "position": position,
                    "duration": duration,
                    "playing": playing,
                    "index": index,
                }),
            );

            if playing
                && duration > 0.0
                && position >= duration - 0.4
                && last_advance.elapsed() > Duration::from_secs(2)
            {
                last_advance = Instant::now();
                let _ = advance(&handle, &state, 1).await;
            }
        }
    });
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let db = Arc::new(Db::open()?);
            let settings = db.load_settings()?;
            let prefs = db.load_ui_prefs()?;
            let http = Arc::new(HttpClient::new(db.clone(), policy_from(&settings))?);
            if let Some(cookies) = db.get_session()? {
                if !cookies.trim().is_empty() {
                    http.restore_session(&cookies)?;
                }
            }
            let source = Arc::new(HtmlSource::new(http.clone(), db.clone()));
            let engine = Arc::new(AudioEngine::new());
            engine.set_volume(settings.volume);

            let app_state = Arc::new(AppState::new(
                db.clone(),
                http,
                source,
                engine,
                settings,
                prefs,
            ));
            *app_state.queue.lock().unwrap() = db.all_tracks()?;

            app.manage(app_state.clone());
            create_tray(app.handle())?;
            register_shortcuts(app.handle());
            start_ticker(app.handle(), app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::list_tree,
            commands::list_children,
            commands::load_category,
            commands::load_liked,
            commands::reload_tracks,
            commands::load_more,
            commands::refresh_category,
            commands::play_track,
            commands::toggle_play,
            commands::next_track,
            commands::previous_track,
            commands::seek,
            commands::set_volume,
            commands::set_shuffle,
            commands::set_repeat,
            commands::play_next,
            commands::add_to_queue,
            commands::remove_from_queue,
            commands::reorder_queue,
            commands::like_track,
            commands::hide_track,
            commands::get_qualities,
            commands::download_track,
            commands::open_post,
            commands::open_download_page,
            commands::copy_link,
            commands::save_ui_prefs,
            commands::get_settings,
            commands::update_settings,
            commands::clear_cache,
            commands::watch_sign_in,
            commands::sign_out,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EDMC Music Player");
}
