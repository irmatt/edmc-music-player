use std::sync::Arc;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use url::Url;

use crate::state::AppState;

const LOGIN_URL: &str = "https://edmc.to/login/";

/// Open a real login window. The password is typed into edmc.to's own page;
/// the app never sees it and only keeps the resulting session cookies.
pub fn sign_in(app: &AppHandle, state: &Arc<AppState>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("signin") {
        let _ = window.set_focus();
        return Ok(());
    }

    let login = Url::parse(LOGIN_URL).map_err(|e| e.to_string())?;
    WebviewWindowBuilder::new(app, "signin", WebviewUrl::External(login))
        .title("Sign in to EDMC")
        .inner_size(520.0, 720.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;

    let app_handle = app.clone();
    let state_handle = state.clone();

    tauri::async_runtime::spawn(async move {
        for _ in 0..300 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;

            let Some(window) = app_handle.get_webview_window("signin") else {
                break;
            };
            let Ok(url) = window.url() else {
                continue;
            };
            let address = url.as_str();
            if address.contains("/login/") || !address.starts_with("https://edmc.to/") {
                continue;
            }

            // On Windows, reading cookies inside a synchronous Tauri command
            // deadlocks; a blocking task keeps it off that thread.
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
    });

    Ok(())
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
