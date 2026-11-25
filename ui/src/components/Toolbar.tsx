import { useState } from "react";
import type { LoadProgress, UiPreferences } from "../types";

interface ToolbarProps {
  prefs: UiPreferences;
  loading: boolean;
  progress: LoadProgress | null;
  onSearch: (query: string) => void;
  onToggleHidden: () => void;
  onArtistFirst: (value: boolean) => void;
  onRowScale: (value: number) => void;
  onColumns: () => void;
}

export function Toolbar({
  prefs,
  loading,
  progress,
  onSearch,
  onToggleHidden,
  onArtistFirst,
  onRowScale,
  onColumns,
}: ToolbarProps) {
  const [query, setQuery] = useState("");
  const percent =
    progress && progress.total > 0
      ? Math.round((progress.done / progress.total) * 100)
      : 0;

  return (
    <div className="toolbar">
      <form
        className="search-form"
        onSubmit={(event) => {
          event.preventDefault();
          onSearch(query);
        }}
      >
        <span>⌕</span>
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search cached tracks"
          aria-label="Search cached tracks"
        />
        <button type="submit">Search</button>
      </form>
      <button className="tool-button" onClick={onColumns}>
        Columns
      </button>
      <button
        className={`tool-button ${prefs.show_hidden ? "pressed" : ""}`}
        onClick={onToggleHidden}
      >
        {prefs.show_hidden ? "Hide hidden" : "Show hidden"}
      </button>
      <label className="toolbar-check">
        <input
          type="checkbox"
          checked={prefs.artist_first}
          onChange={(event) => onArtistFirst(event.target.checked)}
        />
        Artist first
      </label>
      <label className="zoom-control">
        <span>Zoom</span>
        <input
          type="range"
          min={0.8}
          max={1.6}
          step={0.1}
          value={prefs.row_scale}
          onChange={(event) => onRowScale(Number(event.target.value))}
        />
      </label>
      <div className="toolbar-spacer" />
      {loading && (
        <div className="inline-progress">
          <div style={{ width: `${percent}%` }} />
          <span>{progress?.title ?? "Loading…"}</span>
        </div>
      )}
    </div>
  );
}
