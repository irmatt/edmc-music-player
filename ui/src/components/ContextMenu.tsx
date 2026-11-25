import { useEffect, useRef } from "react";
import type { Track } from "../types";

export type ContextAction =
  | "play"
  | "play-next"
  | "add-queue"
  | "remove-queue"
  | "copy-link"
  | "properties"
  | "like"
  | "download"
  | "hide"
  | "unhide"
  | "open-post"
  | "open-download";

interface ContextMenuProps {
  x: number;
  y: number;
  track: Track;
  onAction: (action: ContextAction, track: Track) => void;
  onClose: () => void;
}

export function ContextMenu({ x, y, track, onAction, onClose }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const close = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) onClose();
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", key);
    };
  }, [onClose]);

  const item = (action: ContextAction, label: string) => (
    <button onClick={() => { onAction(action, track); onClose(); }}>{label}</button>
  );

  return (
    <div
      className="context-menu"
      ref={ref}
      style={{ left: Math.min(x, window.innerWidth - 240), top: Math.min(y, window.innerHeight - 330) }}
    >
      {item("play", "Play now")}
      {item("play-next", "Play next")}
      {item("add-queue", "Add to queue")}
      {item("remove-queue", "Remove from queue")}
      <div className="menu-separator" />
      {item("like", track.thanked ? "Liked ✓" : "Like track")}
      {item("download", "Download track…")}
      {track.hidden ? item("unhide", "Unhide track") : item("hide", "Hide track")}
      <div className="menu-separator" />
      {item("open-post", "Go to forum post")}
      {item("open-download", "Go to download page")}
      {item("copy-link", "Copy link")}
      {item("properties", "Track properties…")}
    </div>
  );
}
