import { useState } from "react";
import type { FrontendState } from "../types";
import { Waveform } from "./Waveform";

interface TransportBarProps {
  state: FrontendState;
  onToggle: () => void;
  onNext: () => void;
  onPrevious: () => void;
  onSeek: (seconds: number) => void;
  onVolume: (value: number) => void;
  onShuffle: (value: boolean) => void;
  onRepeat: () => void;
}

function time(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function TransportBar({
  state,
  onToggle,
  onNext,
  onPrevious,
  onSeek,
  onVolume,
  onShuffle,
  onRepeat,
}: TransportBarProps) {
  const [artOpen, setArtOpen] = useState(false);
  const track = state.tracks[state.current_index];
  const artwork = track?.art_url || track?.thumb_url || null;
  return (
    <div className="transport-bar">
      <div className="transport-track">
        <button
          className="transport-thumb"
          title="Show album art"
          onClick={() => artwork && setArtOpen(true)}
        >
          {artwork ? (
            <img src={artwork} alt="Album art" />
          ) : (
            <span>♫</span>
          )}
        </button>
        <div className="transport-meta">
          <span className="transport-title">{track?.title ?? "Nothing playing"}</span>
          <span className="transport-artist">{track?.artist || track?.forum || "EDMC Music Player"}</span>
        </div>
      </div>
      <div className="transport-center">
        <Waveform
          peaks={track?.peaks ?? []}
          position={state.position}
          duration={state.duration}
          onSeek={onSeek}
        />
        <div className="transport-controls">
          <button
            className={`transport-icon ${state.shuffle ? "pressed" : ""}`}
            title="Shuffle"
            onClick={() => onShuffle(!state.shuffle)}
          >
            ⤨
          </button>
          <button className="transport-icon" title="Previous" onClick={onPrevious}>⏮</button>
          <button className="transport-play" title="Play / Pause" onClick={onToggle}>
            {state.playing ? "❚❚" : "▶"}
          </button>
          <button className="transport-icon" title="Next" onClick={onNext}>⏭</button>
          <button
            className={`transport-icon ${state.repeat !== "off" ? "pressed" : ""}`}
            title={`Repeat: ${state.repeat}`}
            onClick={onRepeat}
          >
            {state.repeat === "one" ? "↻¹" : "↻"}
          </button>
          <span className="transport-time">{time(state.position)} / {time(state.duration)}</span>
          <div className="transport-volume">
            <span>◀))</span>
            <input
              type="range"
              min={0}
              max={1}
              step={0.01}
              value={state.volume}
              onChange={(event) => onVolume(Number(event.target.value))}
              aria-label="Volume"
            />
          </div>
        </div>
      </div>
      {artOpen && artwork && (
        <div className="art-overlay" onClick={() => setArtOpen(false)}>
          <div className="art-dialog" onClick={(event) => event.stopPropagation()}>
            <img src={artwork} alt="Album art" />
            <div className="art-caption">{track?.title}</div>
            <button onClick={() => setArtOpen(false)}>Close</button>
          </div>
        </div>
      )}
    </div>
  );
}
