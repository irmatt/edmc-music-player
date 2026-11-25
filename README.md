# EDMC Music Player

A Windows desktop music player for the tracks posted on
[edmc.to](https://edmc.to), styled after Windows Media Player 11. The player is a
Tauri 2 application: scraping, metadata, caching, downloads and audio playback
are Rust; the interface is React + TypeScript. The original Chrome extension is
kept in `extension/chrome`.

## What it does

- Full-height library tree: music categories, genres and subgenres, collapsible
  and pinnable to a favourites section.
- A paged track list with sortable columns: album art, track, artist, upload date,
  Thanks count, comments, views, uploader, category and length.
- Every column can be shown or hidden; the table scrolls horizontally.
- Row actions for Like (the forum Thanks reaction) and Download.
- Right-click menu: play now, play next, add/remove queue, copy link, properties,
  like, download, hide/unhide, open forum post, open download page.
- Playback uses the forum preview (normally a 96 kbps MP3) and is cached on disk.
- Downloads resolve every beatexs file embedded in the post and show a quality
  chooser — FLAC/WAV lossless, 320 kbps MP3, etc. The selected original is saved
  with its original filename.
- Settings: download folder, auto-like on download, request delay, hourly cap,
  cache limit and preview-cache clearing.

## Being a good guest

The sites disallow generic automated clients in `robots.txt`, so this app is
deliberately low-volume:

- Requests happen only on an explicit category load, Refresh or Load more.
- Requests are serial, at least 2 seconds apart (hard floor: 1 s).
- 180 requests per host per hour and 500 per app session, both enforced in code.
- Pages are cached for 7 days and revalidated with `ETag`/`Last-Modified`.
- A category load fetches five pages (250 posts); one page contains all list
  metadata. Audio resolution happens only when a track is first played.
- Preview audio is cached locally (2 GB LRU); downloaded originals are separate.
- `429`/`503` stop the batch and keep the pages already loaded.
- Thanks only fire on a click, or on download when auto-like is explicitly on.

## Layout

```
crates/core/     models, normalized SQLite store, settings, UI preferences
crates/source/   polite HTTP client, rate limiter, tree/list/quality parsing
crates/audio/    rodio playback, decoding and waveform peaks
src-tauri/       app shell, commands, login window, tray, media keys
ui/              React + TypeScript WMP11-style interface
extension/chrome/  the original MV3 extension, kept as a reference
```

## Build on Windows

Prerequisites:

1. Rust stable (`rustup`) with the `x86_64-pc-windows-msvc` target.
2. Visual Studio Build Tools with the **C++ build tools** workload.
3. WebView2 Runtime (included with Windows 11).
4. Node.js 20+.

```powershell
cd edmc-music-player\ui
npm install --include=dev
cd ..
ui\node_modules\.bin\tauri.cmd build
```

Run the CLI from the **repository root** — from `ui/` it cannot find
`src-tauri/tauri.conf.json`. The installer is written to
`src-tauri/target/release/bundle/nsis/`.

For development, use `ui\node_modules\.bin\tauri.cmd dev` from the root.

## Signing in

Use **Sign in to like tracks** in the sidebar. A window opens on
`edmc.to/login/`; credentials are typed into the forum's own page and the app
only keeps the resulting session cookies locally. Signing in is only needed for
Thanks reactions.

## Roadmap

- Android target using the same Tauri project and shared `core`/`source` crates.
- Official Invision Community API source once an admin key is available.
