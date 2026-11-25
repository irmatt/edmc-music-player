import { useCallback, useEffect, useRef, type MouseEvent } from "react";

interface WaveformProps {
  peaks: number[];
  position: number;
  duration: number;
  onSeek: (seconds: number) => void;
  compact?: boolean;
}

export function Waveform({
  peaks,
  position,
  duration,
  onSeek,
  compact = false,
}: WaveformProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ratio = window.devicePixelRatio || 1;
    const width = Math.max(1, canvas.clientWidth);
    const height = Math.max(1, canvas.clientHeight);
    canvas.width = width * ratio;
    canvas.height = height * ratio;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, width, height);

    const styles = getComputedStyle(canvas);
    const played = styles.getPropertyValue("--wave-played").trim() || "#6366f1";
    const rest = styles.getPropertyValue("--wave-rest").trim() || "#4b5563";

    const gap = compact ? 1 : 1;
    const barWidth = compact ? 2 : 2;
    const bars = Math.max(1, Math.floor(width / (barWidth + gap)));
    const progress = duration > 0 ? Math.min(1, position / duration) : 0;
    const middle = height / 2;

    for (let i = 0; i < bars; i += 1) {
      const index = Math.min(
        peaks.length - 1,
        Math.floor((i * peaks.length) / bars),
      );
      const value = peaks.length ? peaks[index] : 0.12;
      const half = Math.max(1, value * (middle - 3));
      const x = i * (barWidth + gap);
      ctx.fillStyle = i / bars <= progress ? played : rest;
      ctx.fillRect(x, middle - half - 1, barWidth, half);
      ctx.fillRect(x, middle + 1, barWidth, half);
    }
  }, [peaks, position, duration, compact]);

  useEffect(() => {
    draw();
    window.addEventListener("resize", draw);
    return () => window.removeEventListener("resize", draw);
  }, [draw]);

  const handleClick = (event: MouseEvent<HTMLCanvasElement>) => {
    if (duration <= 0) return;
    const box = event.currentTarget.getBoundingClientRect();
    const ratio = (event.clientX - box.left) / box.width;
    onSeek(Math.max(0, Math.min(1, ratio)) * duration);
  };

  return (
    <canvas
      ref={canvasRef}
      className="waveform"
      onClick={handleClick}
      aria-label="Waveform — click to seek"
    />
  );
}
