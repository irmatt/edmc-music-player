use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

static DATA_ROOT: OnceLock<PathBuf> = OnceLock::new();

pub fn init_data_root(root: PathBuf) {
    let _ = DATA_ROOT.set(root);
}

pub fn app_dir() -> PathBuf {
    DATA_ROOT
        .get()
        .cloned()
        .unwrap_or_else(default_app_dir)
}

pub fn default_app_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("edmc-music-player")
}

pub fn db_path() -> PathBuf {
    app_dir().join("library.sqlite3")
}

pub fn audio_cache_dir() -> PathBuf {
    app_dir().join("audio-cache")
}

pub fn log_path() -> PathBuf {
    app_dir().join("app.log")
}

pub fn downloads_dir() -> PathBuf {
    dirs::download_dir().unwrap_or_else(|| app_dir().join("Downloads"))
}

pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn portable_config_path() -> PathBuf {
    exe_dir().join("edmc-music-player.json")
}

pub fn portable_default_dir() -> PathBuf {
    exe_dir().join("EDMC Music Player Data")
}

/// Installed builds carry this marker created by the installer.
pub fn is_installed() -> bool {
    exe_dir().join("installed.flag").exists()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PortableConfig {
    pub data_dir: Option<String>,
}

pub fn load_portable_config() -> Option<PortableConfig> {
    let raw = std::fs::read_to_string(portable_config_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_portable_config(config: &PortableConfig) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(portable_config_path(), json)
}
