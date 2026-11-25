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

    const bars = peaks.length || 160;
    const gap = compact ? 1 : 2;
    const barWidth = Math.max(1, width / bars - gap);
    const progress = duration > 0 ? Math.min(1, position / duration) : 0;
    const middle = height / 2;

    for (let i = 0; i < bars; i += 1) {
      const value = peaks[i] ?? 0.04;
      const barHeight = Math.max(2, value * (height - 4));
      const x = i * (barWidth + gap);
      ctx.fillStyle = i / bars <= progress ? played : rest;
      ctx.fillRect(x, middle - barHeight / 2, barWidth, barHeight);
    }

    if (progress > 0) {
      ctx.fillStyle = played;
      ctx.fillRect(progress * width - 1, 0, 2, height);
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
