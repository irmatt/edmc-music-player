use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use thiserror::Error;

/// Normalised waveform resolution sent to the UI.
pub const PEAK_COUNT: usize = 1_200;

#[derive(Debug, Error)]
pub enum Error {
    #[error("audio device: {0}")]
    Device(String),

    #[error("decode: {0}")]
    Decode(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct AudioState {
    pub path: String,
    pub duration: Duration,
    pub paused: bool,
}

/// Playback is owned by Rust. The sink handle must stay alive for as long as
/// the player exists, so both are kept here.
pub struct AudioEngine {
    _sink: Mutex<Option<MixerDeviceSink>>,
    player: Mutex<Option<Player>>,
    current: Mutex<Option<AudioState>>,
    /// Volume in thousandths, so it can live in an atomic without a lock.
    volume_milli: AtomicU32,
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEngine {
    pub fn new() -> Self {
        Self {
            _sink: Mutex::new(None),
            player: Mutex::new(None),
            current: Mutex::new(None),
            volume_milli: AtomicU32::new(800),
        }
    }

    fn ensure_player(&self) -> Result<()> {
        if self.player.lock().unwrap().is_some() {
            return Ok(());
        }
        let handle = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| Error::Device(e.to_string()))?;
        let player = Player::connect_new(&handle.mixer());
        player.set_volume(self.volume() as f32);
        *self._sink.lock().unwrap() = Some(handle);
        *self.player.lock().unwrap() = Some(player);
        Ok(())
    }

    pub fn play_file(&self, path: &Path, duration_secs: Option<f64>) -> Result<()> {
        self.ensure_player()?;
        let file = File::open(path)?;
        let decoder = Decoder::try_from(file).map_err(|e| Error::Decode(e.to_string()))?;
        let duration = duration_secs
            .map(Duration::from_secs_f64)
            .unwrap_or_else(|| {
                decoder
                    .total_duration()
                    .unwrap_or_default()
            });

        let guard = self.player.lock().unwrap();
        let player = guard.as_ref().ok_or_else(|| Error::Other("no player".into()))?;
        player.clear();
        player.append(decoder);
        player.play();
        drop(guard);

        *self.current.lock().unwrap() = Some(AudioState {
            path: path.to_string_lossy().into_owned(),
            duration,
            paused: false,
        });
        Ok(())
    }

    pub fn play(&self) {
        if let Some(player) = self.player.lock().unwrap().as_ref() {
            player.play();
        }
        if let Some(state) = self.current.lock().unwrap().as_mut() {
            state.paused = false;
        }
    }

    pub fn pause(&self) {
        if let Some(player) = self.player.lock().unwrap().as_ref() {
            player.pause();
        }
        if let Some(state) = self.current.lock().unwrap().as_mut() {
            state.paused = true;
        }
    }

    pub fn toggle(&self) {
        if self.is_paused() {
            self.play();
        } else {
            self.pause();
        }
    }

    pub fn stop(&self) {
        if let Some(player) = self.player.lock().unwrap().as_ref() {
            player.clear();
        }
        *self.current.lock().unwrap() = None;
    }

    pub fn seek(&self, position: Duration) -> Result<()> {
        if let Some(player) = self.player.lock().unwrap().as_ref() {
            player
                .try_seek(position)
                .map_err(|e| Error::Other(format!("seek failed: {e}")))?;
        }
        Ok(())
    }

    pub fn position(&self) -> Duration {
        self.player
            .lock()
            .unwrap()
            .as_ref()
            .map(|player| player.get_pos())
            .unwrap_or_default()
    }

    pub fn duration(&self) -> Duration {
        self.current
            .lock()
            .unwrap()
            .as_ref()
            .map(|state| state.duration)
            .unwrap_or_default()
    }

    pub fn is_paused(&self) -> bool {
        self.current
            .lock()
            .unwrap()
            .as_ref()
            .map(|state| state.paused)
            .unwrap_or(true)
    }

    pub fn state(&self) -> Option<AudioState> {
        self.current.lock().unwrap().clone()
    }

    pub fn set_volume(&self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        self.volume_milli
            .store((clamped * 1000.0) as u32, Ordering::Relaxed);
        if let Some(player) = self.player.lock().unwrap().as_ref() {
            player.set_volume(clamped);
        }
    }

    pub fn volume(&self) -> f32 {
        self.volume_milli.load(Ordering::Relaxed) as f32 / 1000.0
    }
}

/// Decode a file once to produce a duration and a normalised waveform.
/// Runs off the UI thread; sample data is never retained.
pub fn analyze(path: &Path) -> Result<(f64, Vec<f32>)> {
    let file = File::open(path)?;
    let decoder = Decoder::try_from(file).map_err(|e| Error::Decode(e.to_string()))?;
    let sample_rate = decoder.sample_rate().get() as f64;
    let channels = decoder.channels().get() as f64;

    const FRAMES_PER_BUCKET: u64 = 256;
    let mut coarse: Vec<f32> = Vec::new();
    let mut loudest = 0f32;
    let mut in_bucket = 0u64;
    let mut samples = 0u64;

    for sample in decoder {
        let amplitude = sample.abs();
        if amplitude > loudest {
            loudest = amplitude;
        }
        in_bucket += 1;
        samples += 1;
        if in_bucket >= FRAMES_PER_BUCKET {
            coarse.push(loudest);
            loudest = 0.0;
            in_bucket = 0;
        }
    }
    if in_bucket > 0 {
        coarse.push(loudest);
    }

    let duration = if sample_rate > 0.0 && channels > 0.0 {
        samples as f64 / (sample_rate * channels)
    } else {
        0.0
    };

    let mut peaks = vec![0f32; PEAK_COUNT];
    if !coarse.is_empty() {
        for index in 0..PEAK_COUNT {
            let start = index * coarse.len() / PEAK_COUNT;
            let end = ((index + 1) * coarse.len() / PEAK_COUNT)
                .max(start + 1)
                .min(coarse.len());
            let maximum = coarse[start..end]
                .iter()
                .copied()
                .fold(0f32, |a, b| a.max(b));
            peaks[index] = maximum;
        }
        let peak = peaks.iter().copied().fold(0f32, |a, b| a.max(b)).max(1e-4);
        for value in &mut peaks {
            *value = (*value / peak).clamp(0.0, 1.0);
        }
    }

    Ok((duration, peaks))
}
