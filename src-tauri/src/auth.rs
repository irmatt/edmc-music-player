use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

static WATCHING: AtomicBool = AtomicBool::new(false);


/// The login window is created from the frontend with Tauri's stable
/// `WebviewWindow` API. This side watches it and captures the session cookies
/// once the forum redirects away from the login page.
pub fn watch_sign_in(app: &AppHandle, state: &Arc<AppState>) {
    if WATCHING.swap(true, Ordering::SeqCst) {
        return;
    }

    let app_handle = app.clone();
    let state_handle = state.clone();
    tauri::async_runtime::spawn(async move {
        for _ in 0..300 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;

            let Some(window) = app_handle.get_webview_window("signin") else {
                continue;
            };
            let Ok(url) = window.url() else {
                continue;
            };
            let address = url.as_str();

            if address.contains("/login/") || !address.starts_with("https://edmc.to/") {
                continue;
            }

            // Reading cookies in a synchronous Tauri command deadlocks on
            // Windows, so use a blocking task on a separate thread.
            let cookie_window = window.clone();
            let captured = tokio::task::spawn_blocking(move || cookie_window.cookies()).await;
            let Ok(Ok(cookies)) = captured else {
                continue;
            };
            let stored = cookies
                .iter()
                .map(|cookie| cookie.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            if stored.trim().is_empty() {
                continue;
            }

            if let Err(error) = state_handle.db.set_session(&stored) {
                let _ = app_handle.emit("error", json!({ "message": error.to_string() }));
                break;
            }
            if let Err(error) = state_handle.http.restore_session(&stored) {
                let _ = app_handle.emit("error", json!({ "message": error.to_string() }));
            }
            let _ = app_handle.emit("auth-changed", json!({ "signed_in": true }));
            let _ = window.close();
            break;
        }
        WATCHING.store(false, Ordering::SeqCst);
    });
}

pub fn sign_out(app: &AppHandle, state: &Arc<AppState>) -> Result<(), String> {
    let existing = state.db.get_session().map_err(|e| e.to_string())?;
    if let Some(cookies) = existing {
        state
            .http
            .clear_session(&cookies)
            .map_err(|e| e.to_string())?;
    }
    state.db.clear_session().map_err(|e| e.to_string())?;
    let _ = app.emit("auth-changed", json!({ "signed_in": false }));
    Ok(())
}
