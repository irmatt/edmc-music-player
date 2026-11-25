import { useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { QualityVariant, Settings, Track } from "../types";

function bytes(value: number): string {
  if (!value) return "—";
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function Shell({ title, onClose, children, footer }: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
}) {
  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div className="modal" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-title">
          <span>{title}</span>
          <button className="modal-close" onClick={onClose}>✕</button>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-footer">{footer}</div>}
      </div>
    </div>
  );
}

export function QualityDialog({ qualities, onChoose, onClose }: {
  qualities: QualityVariant[];
  onChoose: (quality: QualityVariant) => void;
  onClose: () => void;
}) {
  return (
    <Shell title="Choose download quality" onClose={onClose}>
      <p className="dialog-hint">The highest quality is highlighted. Nothing downloads until you choose.</p>
      <div className="quality-list">
        {qualities.map((quality, index) => (
          <button className={`quality-option ${index === 0 ? "best" : ""}`} key={quality.id} onClick={() => onChoose(quality)}>
            <span className="quality-format">{quality.format}</span>
            <span className="quality-name">{quality.filename}</span>
            <span className="quality-meta">
              {quality.quality_label} · {bytes(quality.size_bytes)}
              {quality.lossless ? " · lossless" : ""}
            </span>
            {index === 0 && <span className="quality-badge">Highest</span>}
          </button>
        ))}
      </div>
    </Shell>
  );
}

const COLUMN_INFO: Array<[string, string]> = [
  ["thumb", "Album art"],
  ["track", "Track name"],
  ["artist", "Artist"],
  ["uploaded", "Date uploaded"],
  ["last_post", "Last activity"],
  ["thanks", "Thanks count"],
  ["comments", "Comments"],
  ["views", "Views"],
  ["author", "Uploader"],
  ["forum", "Category"],
  ["duration", "Length"],
  ["like", "Like button"],
  ["download", "Download button"],
];

export function ColumnChooser({ columns, onChange, onClose }: {
  columns: string[];
  onChange: (columns: string[]) => void;
  onClose: () => void;
}) {
  const toggle = (key: string) => {
    onChange(columns.includes(key) ? columns.filter((item) => item !== key) : [...columns, key]);
  };
  return (
    <Shell title="Columns" onClose={onClose} footer={<button className="primary" onClick={onClose}>Done</button>}>
      <div className="column-grid">
        {COLUMN_INFO.map(([key, label]) => (
          <label className="column-option" key={key}>
            <input type="checkbox" checked={columns.includes(key)} onChange={() => toggle(key)} />
            <span>{label}</span>
          </label>
        ))}
      </div>
      <button className="secondary wide" onClick={() => onChange(COLUMN_INFO.map(([key]) => key))}>Show all columns</button>
    </Shell>
  );
}

export function SettingsDialog({ settings, onSave, onClearCache, onClose }: {
  settings: Settings;
  onSave: (settings: Settings) => void;
  onClearCache: () => void;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState(settings);
  const set = <K extends keyof Settings>(key: K, value: Settings[K]) =>
    setDraft((current) => ({ ...current, [key]: value }));

  const chooseFolder = async () => {
    const selected = await open({ directory: true, defaultPath: draft.download_dir });
    if (typeof selected === "string") set("download_dir", selected);
  };

  return (
    <Shell
      title="Settings"
      onClose={onClose}
      footer={
        <>
          <button className="secondary" onClick={onClose}>Cancel</button>
          <button className="primary" onClick={() => onSave(draft)}>Save</button>
        </>
      }
    >
      <label className="field">
        <span>Download folder</span>
        <div className="folder-row">
          <input value={draft.download_dir} readOnly />
          <button className="secondary" onClick={chooseFolder}>Browse…</button>
        </div>
      </label>
      <label className="switch-row">
        <input type="checkbox" checked={draft.auto_like_on_download} onChange={(event) => set("auto_like_on_download", event.target.checked)} />
        <span>Auto-like track on download</span>
      </label>
      <p className="dialog-hint">Sent once per track, after a successful download, and only when signed in.</p>
      <label className="field">
        <span>Minimum delay between requests (ms)</span>
        <input type="number" min={1000} step={500} value={draft.min_delay_ms} onChange={(event) => set("min_delay_ms", Number(event.target.value))} />
      </label>
      <label className="field">
        <span>Requests per forum per hour</span>
        <input type="number" min={1} max={180} value={draft.max_per_host_per_hour} onChange={(event) => set("max_per_host_per_hour", Number(event.target.value))} />
      </label>
      <label className="field">
        <span>Audio cache limit (MB)</span>
        <input type="number" min={256} step={256} value={draft.cache_limit_mb} onChange={(event) => set("cache_limit_mb", Number(event.target.value))} />
      </label>
      <button className="secondary wide" onClick={onClearCache}>Clear downloaded preview audio</button>
      <p className="dialog-hint">Preview cache only; your downloaded originals are never touched.</p>
    </Shell>
  );
}

export function PropertiesDialog({ track, onClose }: { track: Track; onClose: () => void }) {
  const rows: Array<[string, string]> = [
    ["Track", track.title],
    ["Artist", track.artist || "—"],
    ["Category", track.forum],
    ["Uploaded", track.uploaded_at ? new Date(track.uploaded_at).toLocaleString() : "—"],
    ["Last activity", track.last_post_at ? new Date(track.last_post_at).toLocaleString() : "—"],
    ["Uploader", track.author || "—"],
    ["Comments", String(track.replies)],
    ["Views", String(track.views)],
    ["Thanks", String(track.thanks_count)],
    ["Status", track.status],
    ["Length", track.duration_secs ? `${Math.floor(track.duration_secs / 60)}:${String(Math.floor(track.duration_secs % 60)).padStart(2, "0")}` : "Unknown until played"],
    ["Forum post", track.post_url],
  ];
  return (
    <Shell title="Track properties" onClose={onClose}>
      <table className="properties-table">
        <tbody>{rows.map(([key, value]) => <tr key={key}><th>{key}</th><td>{value}</td></tr>)}</tbody>
      </table>
    </Shell>
  );
}

export function DataDirDialog({ defaultDir, onChoose, onUseDefault }: {
  defaultDir: string;
  onChoose: (path: string) => void;
  onUseDefault: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const choose = async () => {
    setBusy(true);
    try {
      const selected = await open({ directory: true, defaultPath: defaultDir });
      if (typeof selected === "string") onChoose(selected);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Shell title="Choose data folder" onClose={() => getCurrentWindow().close()}>
      <p className="dialog-hint">
        This portable copy stores its library, cache and settings in:
      </p>
      <p className="data-dir-path">{defaultDir}</p>
      <p className="dialog-hint">
        You can keep that location or choose another folder. The app will restart
        once you decide.
      </p>
      <div className="data-dir-actions">
        <button className="secondary" onClick={onUseDefault} disabled={busy}>
          Use this folder
        </button>
        <button className="primary" onClick={choose} disabled={busy}>
          Choose another folder…
        </button>
      </div>
    </Shell>
  );
}

export function QueueDialog({ queue, currentIndex, onPlay, onClose }: {
  queue: Track[];
  currentIndex: number;
  onPlay: (index: number) => void;
  onClose: () => void;
}) {
  return (
    <Shell title={`Playback queue (${queue.length})`} onClose={onClose}>
      {queue.length === 0 ? (
        <p className="dialog-hint">Nothing queued. Play a track to start a queue.</p>
      ) : (
        <div className="queue-dialog-list">
          {queue.map((track, index) => (
            <button
              key={`${track.id}-${index}`}
              className={`queue-dialog-row ${index === currentIndex ? "active" : ""}`}
              onClick={() => {
                onPlay(index);
                onClose();
              }}
            >
              <span className="queue-dialog-index">{index === currentIndex ? "▶" : index + 1}</span>
              <span className="queue-dialog-main">
                <span>{track.title || "Unknown track"}</span>
                <span>{track.artist || track.forum}</span>
              </span>
            </button>
          ))}
        </div>
      )}
    </Shell>
  );
}

export function AboutDialog({ onClose }: { onClose: () => void }) {
  return (
    <Shell title="About" onClose={onClose}>
      <p className="about-title">EDMC Music Player</p>
      <p className="dialog-hint">
        A low-volume companion player for the Electronic Dance Music Community forum.
        Browsing, playback and downloads happen only when you ask for them.
      </p>
      <p className="dialog-hint">Playback uses the forum's lightweight preview. Downloads fetch the original quality you choose from beatexs.</p>
    </Shell>
  );
}
