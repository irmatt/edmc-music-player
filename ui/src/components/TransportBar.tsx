import { useEffect, useState } from "react";
import type { FrontendState, Track } from "../types";
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
  onBrowse: (track: Track) => void;
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
  onBrowse,
}: TransportBarProps) {
  const [artOpen, setArtOpen] = useState(false);
  const [artFailed, setArtFailed] = useState(false);
  const [thumbFailed, setThumbFailed] = useState(false);
  const track = state.tracks[state.current_index];
  useEffect(() => {
    setArtFailed(false);
    setThumbFailed(false);
  }, [track?.id]);
  const primary = track?.art_url || null;
  const artwork = artFailed ? track?.thumb_url || null : primary || track?.thumb_url || null;
  const showImage = Boolean(artwork) && !(artFailed && thumbFailed);
  return (
    <div className="transport-bar">
      <div className="transport-track">
        <button
          className="transport-thumb"
          title="Show album art"
          onClick={() => showImage && setArtOpen(true)}
        >
          {showImage ? (
            <img
              src={artwork!}
              alt="Album art"
              onError={() => (artFailed ? setThumbFailed(true) : setArtFailed(true))}
            />
          ) : (
            <span>♫</span>
          )}
        </button>
        <div className="transport-meta">
          <span className="transport-title">{track?.title ?? "Nothing playing"}</span>
          <span className="transport-artist">{track?.artist || "EDMC Music Player"}</span>
          {track && (
            <button className="transport-breadcrumb" onClick={() => onBrowse(track)} title="Browse more from this category">
              Music › {track.forum}
            </button>
          )}
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
      {artOpen && showImage && artwork && (
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
