use std::path::PathBuf;

pub fn app_dir() -> PathBuf {
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
