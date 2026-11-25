use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::Result;
use crate::models::{Track, UiPreferences};
use crate::paths::{app_dir, db_path};
use crate::settings::Settings;

pub struct Db {
    conn: Mutex<Connection>,
}

const TRACK_COLUMNS: &str = "SELECT id,title,artist,author,uploaded_at,last_post_at,replies,views,thanks_count,thumb_url,forum,forum_url,post_url,beatexs_ids,beatexs_url,reaction_url,audio_url,art_url,duration_secs,peaks,thanked,hidden,status,updated_at";

#[derive(Debug, Clone)]
pub struct CachedHttp {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub body: Vec<u8>,
    pub fetched_at: i64,
}

impl Db {
    pub fn open() -> Result<Self> {
        fs::create_dir_all(app_dir())?;
        let conn = Connection::open(db_path())?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            CREATE TABLE IF NOT EXISTS tracks_v2 (
                id           TEXT PRIMARY KEY,
                title        TEXT NOT NULL,
                artist       TEXT NOT NULL DEFAULT '',
                author       TEXT NOT NULL DEFAULT '',
                uploaded_at  TEXT NOT NULL,
                last_post_at TEXT NOT NULL DEFAULT '',
                replies      INTEGER NOT NULL DEFAULT 0,
                views        INTEGER NOT NULL DEFAULT 0,
                thanks_count INTEGER NOT NULL DEFAULT 0,
                thumb_url    TEXT,
                forum        TEXT NOT NULL,
                forum_url    TEXT NOT NULL,
                post_url     TEXT NOT NULL,
                beatexs_ids  TEXT NOT NULL DEFAULT '[]',
                beatexs_url  TEXT,
                reaction_url TEXT,
                audio_url    TEXT,
                art_url      TEXT,
                duration_secs REAL,
                peaks        BLOB NOT NULL DEFAULT X'',
                thanked      INTEGER NOT NULL DEFAULT 0,
                hidden       INTEGER NOT NULL DEFAULT 0,
                status       TEXT NOT NULL DEFAULT 'pending',
                updated_at   INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_tracks_forum ON tracks_v2(forum_url, uploaded_at DESC);
            CREATE INDEX IF NOT EXISTS idx_tracks_thanks ON tracks_v2(forum_url, thanks_count DESC);

            CREATE TABLE IF NOT EXISTS loaded_pages (
                forum_url TEXT NOT NULL,
                page      INTEGER NOT NULL,
                fetched_at INTEGER NOT NULL,
                PRIMARY KEY (forum_url, page)
            );

            CREATE TABLE IF NOT EXISTS ui_prefs (
                id   INTEGER PRIMARY KEY CHECK (id = 1),
                json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS http_cache (
                url           TEXT PRIMARY KEY,
                etag          TEXT,
                last_modified TEXT,
                body          BLOB NOT NULL,
                fetched_at    INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS session (
                id      INTEGER PRIMARY KEY CHECK (id = 1),
                cookies TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS settings (
                id   INTEGER PRIMARY KEY CHECK (id = 1),
                json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS audio_cache (
                url       TEXT PRIMARY KEY,
                path      TEXT NOT NULL,
                bytes     INTEGER NOT NULL,
                last_used INTEGER NOT NULL
            );
            "#,
        )?;
        // Add columns introduced after the first release without losing data.
        let has_last_post = {
            let mut stmt = conn.prepare("PRAGMA table_info(tracks_v2)")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
            let mut found = false;
            for row in rows {
                if row? == "last_post_at" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_last_post {
            conn.execute(
                "ALTER TABLE tracks_v2 ADD COLUMN last_post_at TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // ---- tracks ---------------------------------------------------------

    /// Merge listing metadata without discarding local state (hidden, liked,
    /// resolved audio, duration and waveform).
    pub fn upsert_listing_tracks(&self, tracks: &[Track]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                r#"INSERT INTO tracks_v2
                   (id,title,artist,author,uploaded_at,last_post_at,replies,views,thanks_count,thumb_url,
                    forum,forum_url,post_url,beatexs_ids,beatexs_url,reaction_url,audio_url,
                    art_url,duration_secs,peaks,thanked,hidden,status,updated_at)
                   VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24)
                   ON CONFLICT(id) DO UPDATE SET
                     title=excluded.title, artist=excluded.artist, author=excluded.author,
                     uploaded_at=excluded.uploaded_at, last_post_at=excluded.last_post_at,
                     replies=excluded.replies,
                     views=excluded.views, thanks_count=excluded.thanks_count,
                     thumb_url=excluded.thumb_url, forum=excluded.forum,
                     forum_url=excluded.forum_url, post_url=excluded.post_url,
                     updated_at=excluded.updated_at"#,
            )?;
            for track in tracks {
                stmt.execute(params![
                    track.id,
                    track.title,
                    track.artist,
                    track.author,
                    track.uploaded_at,
                    track.last_post_at,
                    track.replies,
                    track.views,
                    track.thanks_count,
                    track.thumb_url,
                    track.forum,
                    track.forum_url,
                    track.post_url,
                    serde_json::to_string(&track.beatexs_ids)?,
                    track.beatexs_url,
                    track.reaction_url,
                    track.audio_url,
                    track.art_url,
                    track.duration_secs,
                    encode_peaks(&track.peaks),
                    track.thanked as i64,
                    track.hidden as i64,
                    status_string(&track.status),
                    track.updated_at,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn save_track(&self, track: &Track) -> Result<()> {
        self.conn.lock().unwrap().execute(
            r#"INSERT OR REPLACE INTO tracks_v2
               (id,title,artist,author,uploaded_at,last_post_at,replies,views,thanks_count,thumb_url,
                forum,forum_url,post_url,beatexs_ids,beatexs_url,reaction_url,audio_url,
                art_url,duration_secs,peaks,thanked,hidden,status,updated_at)
               VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24)"#,
            params![
                track.id,
                track.title,
                track.artist,
                track.author,
                track.uploaded_at,
                track.last_post_at,
                track.replies,
                track.views,
                track.thanks_count,
                track.thumb_url,
                track.forum,
                track.forum_url,
                track.post_url,
                serde_json::to_string(&track.beatexs_ids)?,
                track.beatexs_url,
                track.reaction_url,
                track.audio_url,
                track.art_url,
                track.duration_secs,
                encode_peaks(&track.peaks),
                track.thanked as i64,
                track.hidden as i64,
                status_string(&track.status),
                track.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn tracks_for_forum(&self, forum_url: &str, show_hidden: bool) -> Result<Vec<Track>> {
        let conn = self.conn.lock().unwrap();
        let select = TRACK_COLUMNS;
        let sql = if show_hidden {
            format!("{select} FROM tracks_v2 WHERE forum_url = ?1 ORDER BY uploaded_at DESC")
        } else {
            format!("{select} FROM tracks_v2 WHERE forum_url = ?1 AND hidden = 0 ORDER BY uploaded_at DESC")
        };
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([forum_url], |row| Ok(row_to_track(row)))?;
        let mut tracks = Vec::new();
        for row in rows {
            tracks.push(row??);
        }
        Ok(tracks)
    }

    pub fn all_tracks(&self) -> Result<Vec<Track>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "{TRACK_COLUMNS} FROM tracks_v2 ORDER BY updated_at DESC"
        ))?;
        let rows = stmt.query_map([], |row| Ok(row_to_track(row)))?;
        let mut tracks = Vec::new();
        for row in rows {
            tracks.push(row??);
        }
        Ok(tracks)
    }

    pub fn set_hidden(&self, id: &str, hidden: bool) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE tracks_v2 SET hidden = ?2 WHERE id = ?1",
            params![id, hidden as i64],
        )?;
        Ok(())
    }

    pub fn set_thanked(&self, id: &str, thanked: bool) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE tracks_v2 SET thanked = ?2 WHERE id = ?1",
            params![id, thanked as i64],
        )?;
        Ok(())
    }

    // ---- loaded forum pages --------------------------------------------

    pub fn loaded_pages(&self, forum_url: &str) -> Result<Vec<u32>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT page FROM loaded_pages WHERE forum_url = ?1 ORDER BY page",
        )?;
        let rows = stmt.query_map([forum_url], |row| row.get::<_, u32>(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn mark_pages_loaded(&self, forum_url: &str, pages: &[u32]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO loaded_pages (forum_url, page, fetched_at) VALUES (?1, ?2, ?3)",
            )?;
            for page in pages {
                stmt.execute(params![forum_url, page, now_secs()])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // ---- ui preferences -------------------------------------------------

    pub fn load_ui_prefs(&self) -> Result<UiPreferences> {
        let conn = self.conn.lock().unwrap();
        let raw: Option<String> = conn
            .query_row("SELECT json FROM ui_prefs WHERE id = 1", [], |row| row.get(0))
            .optional()?;
        match raw {
            Some(json) => Ok(serde_json::from_str(&json)?),
            None => Ok(UiPreferences::default()),
        }
    }

    pub fn save_ui_prefs(&self, prefs: &UiPreferences) -> Result<()> {
        let json = serde_json::to_string(prefs)?;
        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO ui_prefs (id, json) VALUES (1, ?1)",
            params![json],
        )?;
        Ok(())
    }

    // ---- http cache -----------------------------------------------------

    pub fn get_http(&self, url: &str) -> Result<Option<CachedHttp>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT etag, last_modified, body, fetched_at FROM http_cache WHERE url = ?1",
        )?;
        Ok(stmt
            .query_row([url], |row| {
                Ok(CachedHttp {
                    etag: row.get(0)?,
                    last_modified: row.get(1)?,
                    body: row.get(2)?,
                    fetched_at: row.get(3)?,
                })
            })
            .optional()?)
    }

    pub fn put_http(&self, url: &str, body: &[u8], etag: Option<&str>, last_modified: Option<&str>) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO http_cache (url, etag, last_modified, body, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![url, etag, last_modified, body, now_secs()],
        )?;
        Ok(())
    }

    pub fn touch_http(&self, url: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE http_cache SET fetched_at = ?2 WHERE url = ?1",
            params![url, now_secs()],
        )?;
        Ok(())
    }

    // ---- session --------------------------------------------------------

    pub fn get_session(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row("SELECT cookies FROM session WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()?)
    }

    pub fn set_session(&self, cookies: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO session (id, cookies) VALUES (1, ?1)",
            params![cookies],
        )?;
        Ok(())
    }

    pub fn clear_session(&self) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM session WHERE id = 1", [])?;
        Ok(())
    }

    // ---- settings -------------------------------------------------------

    pub fn load_settings(&self) -> Result<Settings> {
        let conn = self.conn.lock().unwrap();
        let raw: Option<String> = conn
            .query_row("SELECT json FROM settings WHERE id = 1", [], |row| row.get(0))
            .optional()?;
        match raw {
            Some(json) => Ok(serde_json::from_str(&json)?),
            None => Ok(Settings::default()),
        }
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        let json = serde_json::to_string(settings)?;
        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO settings (id, json) VALUES (1, ?1)",
            params![json],
        )?;
        Ok(())
    }

    // ---- audio cache ----------------------------------------------------

    pub fn audio_path(&self, url: &str) -> Result<Option<PathBuf>> {
        let conn = self.conn.lock().unwrap();
        let path: Option<String> = conn
            .query_row("SELECT path FROM audio_cache WHERE url = ?1", [url], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(path.map(PathBuf::from))
    }

    pub fn register_audio(&self, url: &str, path: &PathBuf, bytes: u64) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO audio_cache (url, path, bytes, last_used) VALUES (?1, ?2, ?3, ?4)",
            params![url, path.to_string_lossy(), bytes as i64, now_secs()],
        )?;
        Ok(())
    }

    pub fn clear_audio_cache(&self) -> Result<()> {
        let paths: Vec<String> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT path FROM audio_cache")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            let mut paths = Vec::new();
            for row in rows {
                paths.push(row?);
            }
            paths
        };
        for path in paths {
            let _ = std::fs::remove_file(path);
        }
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM audio_cache", [])?;
        Ok(())
    }

    pub fn touch_audio(&self, url: &str) -> Result<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE audio_cache SET last_used = ?2 WHERE url = ?1",
            params![url, now_secs()],
        )?;
        Ok(())
    }

    pub fn prune_audio(&self, limit_bytes: u64, keep: &HashSet<String>) -> Result<Vec<PathBuf>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT url, path, bytes FROM audio_cache ORDER BY last_used ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            let (url, path, bytes) = row?;
            if !keep.contains(&url) {
                entries.push((url, PathBuf::from(path), bytes.max(0) as u64));
            }
        }
        let total: u64 = entries.iter().map(|e| e.2).sum();
        let mut remove = Vec::new();
        let mut freed = 0u64;
        if total > limit_bytes {
            for entry in entries {
                if freed >= total - limit_bytes {
                    break;
                }
                freed += entry.2;
                remove.push(entry);
            }
        }
        drop(stmt);
        drop(conn);

        let mut paths = Vec::new();
        for (url, path, _) in remove {
            let _ = std::fs::remove_file(&path);
            self.conn
                .lock()
                .unwrap()
                .execute("DELETE FROM audio_cache WHERE url = ?1", params![url])?;
            paths.push(path);
        }
        Ok(paths)
    }
}

fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    let status = match row.get::<_, String>(22)?.as_str() {
        "resolving" => crate::models::TrackStatus::Resolving,
        "ready" => crate::models::TrackStatus::Ready,
        "no_audio" => crate::models::TrackStatus::NoAudio,
        "failed" => crate::models::TrackStatus::Failed,
        _ => crate::models::TrackStatus::Pending,
    };
    let ids: Vec<String> = serde_json::from_str(&row.get::<_, String>(13)?).unwrap_or_default();
    Ok(Track {
        id: row.get(0)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        author: row.get(3)?,
        uploaded_at: row.get(4)?,
        last_post_at: row.get(5)?,
        replies: row.get(6)?,
        views: row.get(7)?,
        thanks_count: row.get(8)?,
        thumb_url: row.get(9)?,
        forum: row.get(10)?,
        forum_url: row.get(11)?,
        post_url: row.get(12)?,
        beatexs_ids: ids,
        beatexs_url: row.get(14)?,
        reaction_url: row.get(15)?,
        audio_url: row.get(16)?,
        art_url: row.get(17)?,
        duration_secs: row.get(18)?,
        peaks: decode_peaks(row.get::<_, Vec<u8>>(19)?),
        thanked: row.get::<_, i64>(20)? != 0,
        hidden: row.get::<_, i64>(21)? != 0,
        status,
        updated_at: row.get(23)?,
    })
}

fn status_string(status: &crate::models::TrackStatus) -> &'static str {
    match status {
        crate::models::TrackStatus::Pending => "pending",
        crate::models::TrackStatus::Resolving => "resolving",
        crate::models::TrackStatus::Ready => "ready",
        crate::models::TrackStatus::NoAudio => "no_audio",
        crate::models::TrackStatus::Failed => "failed",
    }
}

fn encode_peaks(peaks: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(peaks.len() * 4);
    for peak in peaks {
        bytes.extend_from_slice(&peak.to_le_bytes());
    }
    bytes
}

fn decode_peaks(bytes: Vec<u8>) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
        .collect()
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
