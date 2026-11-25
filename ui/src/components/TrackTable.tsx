import { useEffect, useRef, type MouseEvent } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { Track, UiPreferences } from "../types";

interface TrackTableProps {
  tracks: Track[];
  prefs: UiPreferences;
  currentIndex: number;
  playing: boolean;
  downloading: Record<string, number>;
  hasMore: boolean;
  loading: boolean;
  onSort: (column: string) => void;
  onPlay: (index: number) => void;
  onLike: (track: Track) => void;
  onDownload: (track: Track) => void;
  onContextMenu: (event: MouseEvent, track: Track) => void;
  onLoadMore: () => void;
}

interface ColumnDef {
  key: string;
  label: string;
  width?: string;
  numeric?: boolean;
  render: (track: Track) => React.ReactNode;
}

function relativeDate(value: string): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

function duration(track: Track): string {
  if (!track.duration_secs || !Number.isFinite(track.duration_secs)) return "—";
  const m = Math.floor(track.duration_secs / 60);
  const s = Math.floor(track.duration_secs % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function TrackTable({
  tracks,
  prefs,
  currentIndex,
  playing,
  downloading,
  hasMore,
  loading,
  onSort,
  onPlay,
  onLike,
  onDownload,
  onContextMenu,
  onLoadMore,
}: TrackTableProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const rowHeight = Math.round(38 * prefs.row_scale);

  const virtualizer = useVirtualizer({
    count: tracks.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight,
    overscan: 12,
  });

  useEffect(() => {
    virtualizer.measure();
  }, [rowHeight, virtualizer]);

  const columns: ColumnDef[] = [
    {
      key: "thumb",
      label: "",
      width: "48px",
      render: (track) => (
        <div className="cell-thumb">
          {track.thumb_url ? <img src={track.thumb_url} alt="" loading="lazy" /> : <span>♫</span>}
        </div>
      ),
    },
    {
      key: "track",
      label: "Track",
      width: `${Math.round(250 * prefs.row_scale)}px`,
      render: (track) => (
        <div className="cell-title">
          <span className="title-line">
            {track === tracks[currentIndex] && playing ? <span className="playing-dot">▶</span> : null}
            {track.title || "Unknown track"}
          </span>
          {!track.artist && <span className="sub-line">Unknown artist</span>}
        </div>
      ),
    },
    { key: "artist", label: "Artist", width: `${Math.round(170 * prefs.row_scale)}px`, render: (track) => track.artist || "—" },
    { key: "uploaded", label: "Added", width: "118px", render: (track) => relativeDate(track.uploaded_at) },
    { key: "thanks", label: "Thanks", width: "82px", numeric: true, render: (track) => track.thanks_count },
    { key: "comments", label: "Comments", width: "92px", numeric: true, render: (track) => track.replies },
    { key: "views", label: "Views", width: "78px", numeric: true, render: (track) => track.views },
    { key: "author", label: "Uploader", width: "120px", render: (track) => track.author || "—" },
    { key: "forum", label: "Category", width: "150px", render: (track) => track.forum },
    { key: "duration", label: "Length", width: "72px", numeric: true, render: (track) => duration(track) },
    {
      key: "like",
      label: "Like",
      width: "58px",
      render: (track) => (
        <button
          className={`row-like ${track.thanked ? "done" : ""}`}
          title={track.thanked ? "Thanked" : "Send Thanks"}
          disabled={track.thanked}
          onClick={(event) => {
            event.stopPropagation();
            onLike(track);
          }}
        >
          {track.thanked ? "♥" : "♡"}
        </button>
      ),
    },
    {
      key: "download",
      label: "Download",
      width: "94px",
      render: (track) => {
        const progress = downloading[track.id];
        return (
          <button
            className="row-download"
            title="Download the highest-quality original"
            onClick={(event) => {
              event.stopPropagation();
              onDownload(track);
            }}
          >
            {progress !== undefined ? `${Math.round(progress * 100)}%` : "⤓ Download"}
          </button>
        );
      },
    },
  ];

  const visible = prefs.columns
    .map((key) => columns.find((column) => column.key === key))
    .filter((column): column is ColumnDef => Boolean(column));

  const ordered = [...visible];
  if (prefs.artist_first) {
    const trackIndex = ordered.findIndex((column) => column.key === "track");
    const artistIndex = ordered.findIndex((column) => column.key === "artist");
    if (trackIndex >= 0 && artistIndex >= 0 && trackIndex < artistIndex) {
      [ordered[trackIndex], ordered[artistIndex]] = [ordered[artistIndex], ordered[trackIndex]];
    }
  }

  return (
    <div className="track-table" ref={scrollRef}>
      <div className="track-head">
        {ordered.map((column) => (
          <button
            key={column.key}
            className={`head-cell ${column.numeric ? "numeric" : ""} ${prefs.sort_by === column.key ? "sorted" : ""}`}
            style={{ width: column.width ?? "120px", flex: "0 0 auto" }}
            onClick={() => column.key !== "thumb" && column.key !== "like" && column.key !== "download" && onSort(column.key)}
          >
            {column.label}
            {prefs.sort_by === column.key && (prefs.descending ? " ▾" : " ▴")}
          </button>
        ))}
      </div>
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const track = tracks[virtualRow.index];
          const active = virtualRow.index === currentIndex;
          return (
            <div
              key={track.id + virtualRow.index}
              className={`track-line ${active ? "active" : ""} ${track.hidden ? "hidden" : ""}`}
              style={{ position: "absolute", top: 0, left: 0, right: 0, height: rowHeight, transform: `translateY(${virtualRow.start}px)` }}
              onClick={() => onPlay(virtualRow.index)}
              onContextMenu={(event) => {
                event.preventDefault();
                onContextMenu(event, track);
              }}
            >
              {ordered.map((column) => (
                <div
                  key={column.key}
                  className={`row-cell ${column.numeric ? "numeric" : ""}`}
                  style={{ width: column.width ?? "120px", flex: "0 0 auto" }}
                >
                  {column.render(track)}
                </div>
              ))}
            </div>
          );
        })}
      </div>
      {hasMore && tracks.length > 0 && (
        <div className="load-more-row">
          <button className="load-more" onClick={onLoadMore} disabled={loading}>
            {loading ? "Loading…" : "Load 250 more"}
          </button>
        </div>
      )}
      {tracks.length === 0 && (
        <div className="table-empty">
          <span>♫</span>
          <p>Choose a genre on the left to load its tracks.</p>
        </div>
      )}
    </div>
  );
}
