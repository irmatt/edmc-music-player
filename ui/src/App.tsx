import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "./api";
import { ContextMenu, type ContextAction } from "./components/ContextMenu";
import {
  AboutDialog,
  ColumnChooser,
  PropertiesDialog,
  QualityDialog,
  SettingsDialog,
} from "./components/Dialogs";
import { LibraryTree } from "./components/LibraryTree";
import { TitleBar } from "./components/TitleBar";
import { Toolbar } from "./components/Toolbar";
import { TrackTable } from "./components/TrackTable";
import { TransportBar } from "./components/TransportBar";
import type {
  DownloadOutcome,
  Forum,
  FrontendState,
  LoadProgress,
  QualityVariant,
  Track,
  UiPreferences,
} from "./types";

type Pending =
  | { kind: "like"; id: string }
  | { kind: "download"; id: string }
  | { kind: "open-download"; id: string };

export default function App() {
  const [state, setState] = useState<FrontendState | null>(null);
  const [tree, setTree] = useState<Forum[]>([]);
  const [selected, setSelected] = useState<Forum | null>(null);
  const [loadingChildren, setLoadingChildren] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [progress, setProgress] = useState<LoadProgress | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [context, setContext] = useState<{ x: number; y: number; track: Track } | null>(null);
  const [quality, setQuality] = useState<{ trackId: string; action: "download" | "open"; options: QualityVariant[] } | null>(null);
  const [properties, setProperties] = useState<Track | null>(null);
  const [dialog, setDialog] = useState<"settings" | "columns" | "about" | null>(null);
  const [downloading, setDownloading] = useState<Record<string, number>>({});
  const pending = useRef<Pending | null>(null);
  const toastTimer = useRef<number | null>(null);
  const refreshSeq = useRef(0);

  const notify = useCallback((message: string) => {
    setToast(message);
    if (toastTimer.current) window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 6000);
  }, []);

  const refresh = useCallback(async () => {
    const seq = ++refreshSeq.current;
    const next = await api.state();
    if (seq === refreshSeq.current) {
      setState(next);
      if (next.current_forum) setSelected(next.current_forum);
    }
    return next;
  }, []);

  useEffect(() => {
    if (!state) return;
    const theme = state.settings.theme;
    document.documentElement.setAttribute("data-theme", theme);
    document.body.setAttribute("data-theme", theme);
  }, [state?.settings.theme]);

  const savePrefs = useCallback(async (next: UiPreferences) => {
    setState((current) => (current ? { ...current, prefs: next } : current));
    await api.savePrefs(next);
  }, []);

  const toggleHidden = useCallback(async () => {
    if (!state) return;
    await savePrefs({ ...state.prefs, show_hidden: !state.prefs.show_hidden });
    await api.reloadTracks();
    await refresh();
  }, [refresh, savePrefs, state]);

  const handleOutcome = useCallback((trackId: string, outcome: DownloadOutcome) => {
    if (outcome.status === "needs_choice") {
      setQuality({ trackId, action: "download", options: outcome.value });
      return;
    }
    if (outcome.status === "needs_sign_in") {
      pending.current = { kind: "download", id: trackId };
      api.signIn().catch((error) => notify(String(error)));
      return;
    }
    setDownloading((current) => {
      const next = { ...current };
      delete next[trackId];
      return next;
    });
    if (outcome.value.warning) notify(outcome.value.warning);
    else notify(`Downloaded to ${outcome.value.path}`);
  }, [notify]);

  useEffect(() => {
    refresh().catch((error) => notify(String(error)));
    api.tree().then(setTree).catch((error) => notify(String(error)));
  }, [notify, refresh]);

  useEffect(() => {
    let active = true;
    Promise.all([
      listen<{ position: number; duration: number; playing: boolean; index: number }>("playback-tick", (event) => {
        const tick = event.payload;
        setState((current) => current ? { ...current, position: tick.position, duration: tick.duration, playing: tick.playing, current_index: tick.index } : current);
      }),
      listen("track-changed", () => { refresh().catch(() => {}); }),
      listen("track-resolving", () => notify("Resolving the track preview…")),
      listen("tracks-loaded", () => { refresh().catch(() => {}); setLoading(false); setProgress(null); }),
      listen<LoadProgress>("load-progress", (event) => setProgress(event.payload)),
      listen<{ signed_in: boolean }>("auth-changed", (event) => {
        refresh().then(async () => {
          setState((current) => current ? { ...current, signed_in: event.payload.signed_in } : current);
          if (event.payload.signed_in && pending.current) {
            const action = pending.current;
            pending.current = null;
            if (action.kind === "like") await api.like(action.id).catch((error) => notify(String(error)));
            if (action.kind === "download") {
              const outcome = await api.download(action.id).catch((error) => { notify(String(error)); return null; });
              if (outcome) handleOutcome(action.id, outcome);
            }
            if (action.kind === "open-download") {
              const outcome = await api.openDownloadPage(action.id).catch((error) => { notify(String(error)); return null; });
              if (outcome) handleOutcome(action.id, outcome);
            }
            refresh().catch(() => {});
          }
        }).catch(() => {});
      }),
      listen<{ id: string; done: number; total: number }>("download-progress", (event) => {
        const { id, done, total } = event.payload;
        setDownloading((current) => ({ ...current, [id]: total > 0 ? done / total : 0 }));
      }),
      listen<{ message: string }>("error", (event) => notify(event.payload.message)),
    ]).then((unlisteners) => {
      if (!active) unlisteners.forEach((unlisten) => unlisten());
    });
    return () => { active = false; };
  }, [handleOutcome, notify, refresh]);

  const updateTreeChildren = useCallback((forums: Forum[], url: string, children: Forum[]): Forum[] =>
    forums.map((forum) =>
      forum.url === url
        ? { ...forum, children }
        : { ...forum, children: updateTreeChildren(forum.children, url, children) },
    ), []);

  const expand = useCallback(async (forum: Forum) => {
    if (forum.children.length > 0) {
      const collapsed = state?.prefs.collapsed ?? [];
      const next = collapsed.includes(forum.url)
        ? collapsed.filter((url) => url !== forum.url)
        : [...collapsed, forum.url];
      if (state) await savePrefs({ ...state.prefs, collapsed: next });
      return;
    }
    setLoadingChildren((current) => new Set(current).add(forum.url));
    try {
      const children = await api.children(forum);
      setTree((current) => updateTreeChildren(current, forum.url, children));
    } catch (error) {
      notify(String(error));
    } finally {
      setLoadingChildren((current) => {
        const next = new Set(current);
        next.delete(forum.url);
        return next;
      });
    }
  }, [notify, savePrefs, state, updateTreeChildren]);

  const selectForum = useCallback(async (forum: Forum) => {
    setSelected(forum);
    if (forum.url.startsWith("liked://")) {
      setLoading(true);
      try {
        await api.loadLiked();
        await refresh();
      } catch (error) {
        notify(String(error));
      } finally {
        setLoading(false);
      }
      return;
    }
    if (forum.children.length > 0) {
      setState((current) => current ? { ...current, tracks: [], current_forum: forum } : current);
      if (state?.prefs.collapsed.includes(forum.url)) {
        await savePrefs({ ...state.prefs, collapsed: state.prefs.collapsed.filter((url) => url !== forum.url) });
      }
      return;
    }
    setLoading(true);
    setProgress(null);
    try {
      const batch = await api.loadCategory(forum);
      if (batch.errors.length) notify(batch.errors.join(" "));
      await refresh();
    } catch (error) {
      notify(String(error));
    } finally {
      setLoading(false);
      setProgress(null);
    }
  }, [notify, refresh]);

  const loadMore = useCallback(async () => {
    setLoading(true);
    try {
      const batch = await api.loadMore();
      if (batch.errors.length) notify(batch.errors.join(" "));
      await refresh();
    } catch (error) {
      notify(String(error));
    } finally {
      setLoading(false);
      setProgress(null);
    }
  }, [notify, refresh]);

  const doRefresh = useCallback(async () => {
    if (!selected) return;
    setLoading(true);
    try {
      const batch = await api.refresh();
      if (batch.errors.length) notify(batch.errors.join(" "));
      await refresh();
      notify("Refreshed the newest posts.");
    } catch (error) {
      notify(String(error));
    } finally {
      setLoading(false);
      setProgress(null);
    }
  }, [notify, refresh, selected]);

  const like = useCallback(async (track: Track) => {
    try {
      await api.like(track.id);
      await refresh();
    } catch (error) {
      if (String(error).includes("SIGN_IN_REQUIRED")) {
        pending.current = { kind: "like", id: track.id };
        await api.signIn().catch((e) => notify(String(e)));
      } else {
        notify(String(error));
      }
    }
  }, [notify, refresh]);

  const download = useCallback(async (track: Track) => {
    setDownloading((current) => ({ ...current, [track.id]: 0 }));
    try {
      const outcome = await api.download(track.id);
      handleOutcome(track.id, outcome);
    } catch (error) {
      setDownloading((current) => {
        const next = { ...current };
        delete next[track.id];
        return next;
      });
      notify(String(error));
    }
  }, [handleOutcome, notify]);

  const openDownload = useCallback(async (track: Track) => {
    try {
      const outcome = await api.openDownloadPage(track.id);
      if (outcome.status === "needs_choice") {
        setQuality({ trackId: track.id, action: "open", options: outcome.value });
      } else if (outcome.status === "complete") {
        setQuality(null);
      }
    } catch (error) {
      notify(String(error));
    }
  }, [notify]);

  const chooseQuality = useCallback(async (variant: QualityVariant) => {
    if (!quality) return;
    const { trackId, action } = quality;
    setQuality(null);
    if (action === "open") {
      await api.openDownloadPage(trackId, variant.id).catch((error) => notify(String(error)));
      return;
    }
    setDownloading((current) => ({ ...current, [trackId]: 0 }));
    const outcome = await api.download(trackId, variant.id).catch((error) => { notify(String(error)); return null; });
    if (outcome) handleOutcome(trackId, outcome);
  }, [handleOutcome, notify, quality]);

  const onContextAction = useCallback(async (action: ContextAction, track: Track) => {
    const index = state?.tracks.findIndex((item) => item.id === track.id) ?? -1;
    try {
      switch (action) {
        case "play": if (index >= 0) await api.playTrack(index); break;
        case "play-next": if (index >= 0) await api.playNext(index); break;
        case "add-queue": if (index >= 0) await api.addToQueue(index); break;
        case "remove-queue": if (index >= 0) await api.removeFromQueue(index); break;
        case "copy-link": await api.copyLink(track.id); notify("Link copied."); break;
        case "properties": setProperties(track); break;
        case "like": await like(track); break;
        case "download": await download(track); break;
        case "hide": await api.hide(track.id, true); await refresh(); break;
        case "unhide": await api.hide(track.id, false); await refresh(); break;
        case "open-post": await api.openPost(track.id); break;
        case "open-download": await openDownload(track); break;
      }
    } catch (error) {
      notify(String(error));
    }
  }, [download, like, notify, openDownload, refresh, state?.tracks]);

  const sortedTracks = useMemo(() => {
    if (!state) return [];
    const prefs = state.prefs;
    const value = (track: Track): string | number => {
      switch (prefs.sort_by) {
        case "artist": return track.artist.toLowerCase();
        case "title":
        case "track": return track.title.toLowerCase();
        case "uploaded": return track.uploaded_at;
        case "thanks": return track.thanks_count;
        case "comments": return track.replies;
        case "views": return track.views;
        case "author": return track.author.toLowerCase();
        case "forum": return track.forum.toLowerCase();
        case "duration": return track.duration_secs ?? -1;
        default: return track.uploaded_at;
      }
    };
    return [...state.tracks].sort((a, b) => {
      const av = value(a);
      const bv = value(b);
      const comparison = typeof av === "number" && typeof bv === "number"
        ? av - bv
        : String(av).localeCompare(String(bv));
      return prefs.descending ? -comparison : comparison;
    });
  }, [state]);

  if (!state) {
    return <div className="boot">Starting EDMC Music Player…</div>;
  }

  const prefs = state.prefs;
  const isLikedView = state.current_forum?.url.startsWith("liked://") ?? false;
  const hasMore = !isLikedView && state.total_pages > (state.loaded_pages.at(-1) ?? 0);
  const activeId = state.tracks[state.current_index]?.id;
  const activeSortedIndex = activeId ? sortedTracks.findIndex((track) => track.id === activeId) : -1;

  return (
    <div className="shell">
      <TitleBar
        theme={state.settings.theme}
        onTheme={(theme) => {
          const next = { ...state.settings, theme };
          api.updateSettings(next)
            .then(() => setState({ ...state, settings: next }))
            .catch((error) => notify(String(error)));
        }}
        onSettings={() => setDialog("settings")}
        onColumns={() => setDialog("columns")}
        onShowHidden={toggleHidden}
        onAbout={() => setDialog("about")}
        onToggle={() => api.toggle()}
        onNext={() => api.next()}
        onPrevious={() => api.previous()}
      />
      <div className="workspace">
        <aside className="sidebar">
          <LibraryTree
            tree={tree}
            prefs={prefs}
            likedCount={state.liked_count}
            likedSelected={isLikedView}
            selectedUrl={state.current_forum?.url ?? selected?.url ?? null}
            loadingChildren={loadingChildren}
            onSelect={selectForum}
            onExpand={expand}
            onToggleFavorite={(forum) => {
              const favorites = prefs.favorites.includes(forum.url)
                ? prefs.favorites.filter((url) => url !== forum.url)
                : [...prefs.favorites, forum.url];
              savePrefs({ ...prefs, favorites });
            }}
            onReorderFavorite={(from, to) => {
              const favorites = [...prefs.favorites];
              const [moved] = favorites.splice(from, 1);
              favorites.splice(to, 0, moved);
              savePrefs({ ...prefs, favorites });
            }}
          />
          <div className="sidebar-footer">
            {state.signed_in ? (
              <button className="sidebar-account" onClick={() => api.signOut().then(refresh)}>
                <span className="dot online" /> Signed in · Sign out
              </button>
            ) : (
              <button className="sidebar-account" onClick={() => api.signIn().catch((error) => notify(String(error)))}>
                <span className="dot" /> Sign in to like tracks
              </button>
            )}
          </div>
        </aside>
        <main className="main">
          <div className="main-heading">
            <div>
              <h1>{state.current_forum?.name ?? selected?.name ?? "Music"}</h1>
              <span>{state.tracks.length} tracks · {state.loaded_pages.length} pages loaded</span>
            </div>
            <button className="tool-button" onClick={doRefresh} disabled={loading || !selected || isLikedView}>↻ Refresh</button>
          </div>
          <Toolbar
            prefs={prefs}
            loading={loading}
            progress={progress}
            onRefresh={doRefresh}
            onToggleHidden={toggleHidden}
            onArtistFirst={(artist_first) => savePrefs({ ...prefs, artist_first })}
            onRowScale={(row_scale) => savePrefs({ ...prefs, row_scale })}
            onColumns={() => setDialog("columns")}
          />
          <TrackTable
            tracks={sortedTracks}
            prefs={prefs}
            currentIndex={activeSortedIndex}
            playing={state.playing}
            downloading={downloading}
            hasMore={hasMore}
            loading={loading}
            onLoadMore={loadMore}
            onReorderColumns={(columns) => savePrefs({ ...prefs, columns })}
            onResizeColumn={(key, width) =>
              savePrefs({
                ...prefs,
                column_widths: { ...prefs.column_widths, [key]: width },
              })
            }
            onSort={(column) => savePrefs({ ...prefs, sort_by: column, descending: prefs.sort_by === column ? !prefs.descending : true })}
            onPlay={(index) => {
              const track = sortedTracks[index];
              const queueIndex = state.tracks.findIndex((item) => item.id === track.id);
              if (queueIndex >= 0) api.playTrack(queueIndex).catch((error) => notify(String(error)));
            }}
            onLike={like}
            onDownload={download}
            onContextMenu={(event, track) => setContext({ x: event.clientX, y: event.clientY, track })}
          />
        </main>
      </div>
      <TransportBar
        state={state}
        onToggle={() => api.toggle()}
        onNext={() => api.next().catch((error) => notify(String(error)))}
        onPrevious={() => api.previous().catch((error) => notify(String(error)))}
        onSeek={(seconds) => api.seek(seconds).catch((error) => notify(String(error)))}
        onVolume={(volume) => api.setVolume(volume).then(refresh)}
        onShuffle={(shuffle) => api.setShuffle(shuffle).then(() => setState({ ...state, shuffle }))}
        onRepeat={async () => {
          const order = ["off", "all", "one"] as const;
          const next = order[(order.indexOf(state.repeat) + 1) % order.length];
          await api.setRepeat(next);
          setState({ ...state, repeat: next });
        }}
      />

      {context && (
        <ContextMenu
          x={context.x}
          y={context.y}
          track={context.track}
          onAction={onContextAction}
          onClose={() => setContext(null)}
        />
      )}
      {quality && (
        <QualityDialog qualities={quality.options} onChoose={chooseQuality} onClose={() => setQuality(null)} />
      )}
      {properties && <PropertiesDialog track={properties} onClose={() => setProperties(null)} />}
      {dialog === "columns" && (
        <ColumnChooser
          columns={prefs.columns}
          onChange={(columns) => savePrefs({ ...prefs, columns })}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "settings" && (
        <SettingsDialog
          settings={state.settings}
          onSave={(settings) => {
            api.updateSettings(settings).then(() => { setState({ ...state, settings }); setDialog(null); });
          }}
          onClearCache={() => api.clearCache().then(() => notify("Preview cache cleared."))}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog === "about" && <AboutDialog onClose={() => setDialog(null)} />}
      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
