import {
  useEffect,
  useRef,
  useState,
  type MouseEvent,
  type ReactNode,
} from "react";
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
  onReorderColumns: (columns: string[]) => void;
  onResizeColumn: (key: string, width: number) => void;
}

interface ColumnDef {
  key: string;
  label: string;
  defaultWidth: number;
  numeric?: boolean;
  render: (track: Track, index: number) => ReactNode;
}

function relativeDate(value: string): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
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
  onReorderColumns,
  onResizeColumn,
}: TrackTableProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [selected, setSelected] = useState<number | null>(null);
  const [draggedColumn, setDraggedColumn] = useState<string | null>(null);
  const [hoverColumn, setHoverColumn] = useState<string | null>(null);
  const dragState = useRef<{ key: string; startX: number; active: boolean } | null>(null);
  const hoverRef = useRef<string | null>(null);
  const suppressClick = useRef(false);
  const previewRef = useRef<string[] | null>(null);
  const [previewColumns, setPreviewColumns] = useState<string[] | null>(null);
  const [dragPos, setDragPos] = useState<{ x: number; y: number } | null>(null);
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
      defaultWidth: 48,
      render: (track, index) => (
        <button
          className="cell-thumb"
          title="Play this track"
          onClick={(event) => {
            event.stopPropagation();
            onPlay(index);
          }}
        >
          {track.thumb_url ? (
            <img src={track.thumb_url} alt="" loading="lazy" />
          ) : (
            <span>♫</span>
          )}
        </button>
      ),
    },
    {
      key: "track",
      label: "Track",
      defaultWidth: Math.round(250 * prefs.row_scale),
      render: (track) => (
        <div className="cell-title">
          <span className="title-line">
            {track === tracks[currentIndex] && playing ? (
              <span className="playing-dot">▶</span>
            ) : null}
            {track.title || "Unknown track"}
          </span>
          {!track.artist && <span className="sub-line">Unknown artist</span>}
        </div>
      ),
    },
    {
      key: "artist",
      label: "Artist",
      defaultWidth: Math.round(170 * prefs.row_scale),
      render: (track) => track.artist || "—",
    },
    {
      key: "uploaded",
      label: "Added",
      defaultWidth: 118,
      render: (track) => relativeDate(track.uploaded_at),
    },
    {
      key: "thanks",
      label: "Thanks",
      defaultWidth: 82,
      numeric: true,
      render: (track) => track.thanks_count,
    },
    {
      key: "comments",
      label: "Comments",
      defaultWidth: 92,
      numeric: true,
      render: (track) => track.replies,
    },
    {
      key: "views",
      label: "Views",
      defaultWidth: 78,
      numeric: true,
      render: (track) => track.views,
    },
    {
      key: "author",
      label: "Uploader",
      defaultWidth: 120,
      render: (track) => track.author || "—",
    },
    {
      key: "forum",
      label: "Category",
      defaultWidth: 150,
      render: (track) => track.forum,
    },
    {
      key: "duration",
      label: "Length",
      defaultWidth: 72,
      numeric: true,
      render: (track) => duration(track),
    },
    {
      key: "like",
      label: "Like",
      defaultWidth: 58,
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
      defaultWidth: 100,
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
            {progress !== undefined
              ? `${Math.round(progress * 100)}%`
              : "⤓ Download"}
          </button>
        );
      },
    },
  ];

  const ordered = [...(previewColumns ?? prefs.columns)]
    .map((key) => columns.find((column) => column.key === key))
    .filter((column): column is ColumnDef => Boolean(column));
  if (prefs.artist_first) {
    const trackIndex = ordered.findIndex((column) => column.key === "track");
    const artistIndex = ordered.findIndex((column) => column.key === "artist");
    if (trackIndex >= 0 && artistIndex >= 0 && trackIndex < artistIndex) {
      [ordered[trackIndex], ordered[artistIndex]] = [
        ordered[artistIndex],
        ordered[trackIndex],
      ];
    }
  }

  const widthOf = (column: ColumnDef) =>
    prefs.column_widths[column.key] ?? column.defaultWidth;

  const startColumnDrag = (event: MouseEvent, key: string) => {
    event.preventDefault();
    event.stopPropagation();
    dragState.current = { key, startX: event.clientX, active: false };
    hoverRef.current = null;

    const move = (moveEvent: globalThis.MouseEvent) => {
      const state = dragState.current;
      if (!state) return;
      if (!state.active && Math.abs(moveEvent.clientX - state.startX) > 5) {
        state.active = true;
        setDraggedColumn(state.key);
      }
      if (!state.active) return;
      setDragPos({ x: moveEvent.clientX, y: moveEvent.clientY });
      const element = document.elementFromPoint(moveEvent.clientX, moveEvent.clientY);
      const target = element?.closest("[data-column]")?.getAttribute("data-column") ?? null;
      hoverRef.current = target;
      setHoverColumn(target);
      if (target && target !== state.key) {
        const base = previewRef.current ?? prefs.columns;
        const next = [...base];
        next.splice(next.indexOf(state.key), 1);
        const destination = next.indexOf(target);
        if (destination >= 0) {
          next.splice(destination, 0, state.key);
          previewRef.current = next;
          setPreviewColumns(next);
        }
      }
    };

    const up = () => {
      const state = dragState.current;
      if (state?.active) {
        const next = previewRef.current;
        if (next) onReorderColumns(next);
        suppressClick.current = true;
      }
      dragState.current = null;
      hoverRef.current = null;
      previewRef.current = null;
      setPreviewColumns(null);
      setDragPos(null);
      setDraggedColumn(null);
      setHoverColumn(null);
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
    };

    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  };

  const startResize = (event: MouseEvent, column: ColumnDef) => {
    event.preventDefault();
    event.stopPropagation();
    const startX = event.clientX;
    const startWidth = widthOf(column);
    const move = (moveEvent: globalThis.MouseEvent) => {
      onResizeColumn(column.key, Math.max(40, startWidth + moveEvent.clientX - startX));
    };
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  };

  return (
    <div className="track-table" ref={scrollRef}>
      <div className="track-head">
        {ordered.map((column) => (
          <button
            key={column.key}
            data-column={column.key}
            className={`head-cell ${column.numeric ? "numeric" : ""} ${
              prefs.sort_by === column.key ? "sorted" : ""
            } ${draggedColumn === column.key ? "dragging" : ""} ${
              hoverColumn === column.key && draggedColumn !== column.key
                ? "drop-target"
                : ""
            }`}
            style={{ width: widthOf(column), flex: "0 0 auto" }}
            onClick={() => {
              if (suppressClick.current) {
                suppressClick.current = false;
                return;
              }
              if (!["thumb", "like", "download"].includes(column.key)) {
                onSort(column.key);
              }
            }}
          >
            <span
              className="head-label"
              onMouseDown={(event) => startColumnDrag(event, column.key)}
            >
              {column.label}
              {prefs.sort_by === column.key &&
                (prefs.descending ? " ▾" : " ▴")}
            </span>
            <span
              className="column-resizer"
              onMouseDown={(event) => startResize(event, column)}
              onClick={(event) => event.stopPropagation()}
            />
          </button>
        ))}
      </div>
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const track = tracks[virtualRow.index];
          return (
            <div
              key={track.id + virtualRow.index}
              className={`track-line ${
                virtualRow.index === currentIndex ? "active" : ""
              } ${virtualRow.index === selected ? "selected" : ""} ${
                track.hidden ? "hidden" : ""
              }`}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                right: 0,
                height: rowHeight,
                transform: `translateY(${virtualRow.start}px)`,
              }}
              onClick={() => {
                if (selected === virtualRow.index) onPlay(virtualRow.index);
                else setSelected(virtualRow.index);
              }}
              onContextMenu={(event) => {
                event.preventDefault();
                onContextMenu(event, track);
              }}
            >
              {ordered.map((column) => (
                <div
                  key={column.key}
                  className={`row-cell ${column.numeric ? "numeric" : ""}`}
                  style={{ width: widthOf(column), flex: "0 0 auto" }}
                >
                  {column.render(track, virtualRow.index)}
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
      {draggedColumn && dragPos && (
        <div
          className="column-drag-ghost"
          style={{ left: dragPos.x + 12, top: dragPos.y + 12 }}
        >
          {columns.find((column) => column.key === draggedColumn)?.label}
        </div>
      )}
    </div>
  );
}
