document.addEventListener('DOMContentLoaded', () => {
  const iframePlayer = document.getElementById('iframe-player');
  const songName = document.getElementById('song-name');
  const prevBtn = document.getElementById('prev-btn');
  const nextBtn = document.getElementById('next-btn');
  const songList = document.getElementById('song-list');
  const loading = document.getElementById('loading');
  const error = document.getElementById('error');
  const playPauseBtn = document.getElementById('play-pause-btn');
  const seekBar = document.getElementById('seek-bar');
  const timeDisplay = document.getElementById('time-display');

  let tracks = [];
  let currentTrackIndex = 0;
  let isPlaying = false;
  let thankedSongs = new Set();
  let addedSongIndices = new Set();
  let wavesurfer = null;
  let isLoading = false;

  function createWaveSurfer() {
    if (wavesurfer) {
      if (wavesurfer.isPlaying()) wavesurfer.stop();
      wavesurfer.destroy();
    }
    wavesurfer = WaveSurfer.create({
      container: '#waveform',
      waveColor: '#aaa',
      progressColor: '#007bff',
      cursorColor: '#fff',
      height: 80,
      barWidth: 0.5,          // Thinner bars for more detail (was 2)
      barGap: 0.2,            // No gap between bars (was 1)
      barMinHeight: 0.4,      // Ensures quiet sections remain visible
      barRadius: 0,       // Slight rounding for smoothness (optional)
      pixelRatio: 3,        // Higher resolution for sharper rendering
      responsive: true,
      interact: true,
      backend: 'WebAudio',
      normalize: false
    });

    wavesurfer.on('ready', () => {
      if (isLoading) return;
      const duration = wavesurfer.getDuration();
      if (duration <= 0 || !isFinite(duration)) {
        console.error("Invalid duration:", duration);
        handleError("Track has invalid duration");
        return;
      }
      timeDisplay.textContent = `0:00 - ${formatTime(duration)}`;
      if (isPlaying) {
        wavesurfer.setTime(0);
        wavesurfer.play();
      }
    });

    wavesurfer.on('finish', () => {
      isPlaying = false;
      if (currentTrackIndex < tracks.length - 1) {
        currentTrackIndex++;
        playTrack(currentTrackIndex);
      } else {
        playPauseBtn.innerHTML = '<i class="fas fa-play"></i>';
        updateProgress(0);
      }
    });

    // Throttle audioprocess updates to reduce overhead
    let lastUpdate = 0;
    wavesurfer.on('audioprocess', (currentTime) => {
      const now = Date.now();
      if (now - lastUpdate < 100) return; // Update every 100ms
      lastUpdate = now;
      const duration = wavesurfer.getDuration();
      if (duration && isFinite(duration)) updateProgress(currentTime);
    });

    wavesurfer.on('error', (e) => {
      console.error("WaveSurfer error:", e);
      handleError(`Track ${currentTrackIndex + 1} failed: ${e.message || 'Unknown error'}`);
    });

    wavesurfer.on('loading', (percent) => {
      if (percent === 100) isLoading = false;
    });
  }

  function formatTime(seconds) {
    const minutes = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${minutes}:${secs < 10 ? '0' : ''}${secs}`;
  }

  function safeTrackIndex(index) {
    return Math.max(0, Math.min(index, tracks.length - 1));
  }

  function updateProgress(currentTime) {
    const duration = wavesurfer.getDuration();
    if (duration && isFinite(duration)) {
      seekBar.value = (currentTime / duration) * 100;
      timeDisplay.textContent = `${formatTime(currentTime)} - ${formatTime(duration)}`;
    } else {
      seekBar.value = 0;
      timeDisplay.textContent = "0:00 - 0:00";
    }
  }

  function playTrack(index) {
    index = safeTrackIndex(index);
    const track = tracks[index];
    if (!track.url) {
      handleError(`Track ${index + 1} has no audio URL`);
      return;
    }

    if (wavesurfer && wavesurfer.isPlaying()) wavesurfer.stop();
    createWaveSurfer();
    isLoading = true;
    isPlaying = true;
    currentTrackIndex = index;
    wavesurfer.load(track.url);
    updateUI(track);
    playPauseBtn.innerHTML = '<i class="fas fa-pause"></i>';
  }

  function handleError(message) {
    error.style.display = 'block';
    error.textContent = message;
    isPlaying = false;
    playPauseBtn.innerHTML = '<i class="fas fa-play"></i>';
    if (currentTrackIndex < tracks.length - 1) {
      currentTrackIndex++;
      playTrack(currentTrackIndex);
    }
  }

  function updateUI(track) {
    if (!track || !track.url) {
      handleError("Track not available");
      return;
    }

    songName.textContent = track.songName || 'Unknown Song';
    updateSongListHighlight();

    if (track.beatexsUrl) {
      iframePlayer.src = track.beatexsUrl;
      iframePlayer.style.display = 'block';
    } else {
      iframePlayer.style.display = 'none';
    }

    seekBar.value = 0;
    timeDisplay.textContent = "0:00 - 0:00";
  }

  function updateSongListHighlight() {
    const songItems = songList.getElementsByClassName('song-item');
    for (let i = 0; i < songItems.length; i++) {
      songItems[i].classList.toggle('playing', i === currentTrackIndex);
    }
  }

  function createThanksButton(index, reactionUrl) {
    const thanksBtn = document.createElement('button');
    thanksBtn.className = 'thanks-btn';
    thanksBtn.innerHTML = `
      <img src="https://edmc.to/uploads/reactions/react_thanks.png" alt="Thanks">
      <span class="status-icon"></span>
    `;
    thanksBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      if (thankedSongs.has(index)) return;
      const statusIcon = thanksBtn.querySelector('.status-icon');
      statusIcon.innerHTML = '<i class="fas fa-spinner"></i>';
      chrome.runtime.sendMessage({ action: 'performReaction', reactionUrl }, (response) => {
        if (response && response.success) {
          statusIcon.innerHTML = '<i class="fas fa-check" style="color: #28a745;"></i>';
          thankedSongs.add(index);
          thanksBtn.disabled = true;
        } else {
          statusIcon.innerHTML = '<i class="fas fa-exclamation-circle" style="color: #dc3545;"></i>';
        }
      });
    });
    return thanksBtn;
  }

  function addSongListItem(index) {
    if (addedSongIndices.has(index)) return;
    const track = tracks[index];
    const songItem = document.createElement('div');
    songItem.className = `song-item ${!track.url ? 'no-audio' : ''}`;
    songItem.dataset.index = index;

    const songTitle = document.createElement('span');
    songTitle.textContent = track.songName || 'Unknown Song';
    songTitle.style.flex = '1';
    songItem.appendChild(songTitle);

    if (!track.url) {
      const warningIcon = document.createElement('span');
      warningIcon.innerHTML = '<i class="fas fa-exclamation-triangle" style="color: #ff5555;"></i>';
      warningIcon.title = "No audio file found";
      songItem.appendChild(warningIcon);
    } else if (track.reactionUrl && !thankedSongs.has(index)) {
      const thanksBtn = createThanksButton(index, track.reactionUrl);
      songItem.appendChild(thanksBtn);
    }

    songItem.addEventListener('click', (e) => {
      if (!e.target.closest('.thanks-btn')) {
        currentTrackIndex = safeTrackIndex(parseInt(songItem.dataset.index));
        playTrack(currentTrackIndex);
      }
    });
    songList.appendChild(songItem);
    addedSongIndices.add(index);
  }

  function populateSongList() {
    addedSongIndices.clear();
    tracks.forEach((_, index) => addSongListItem(index));
  }

  function toggleUIElements(enabled) {
    if (enabled && tracks.length > 0) {
      songList.style.display = 'block';
      document.getElementById('custom-player').style.display = 'flex';
      loading.style.display = 'none';
      error.style.display = 'none';
      prevBtn.disabled = false;
      nextBtn.disabled = false;
      playPauseBtn.disabled = false;
    } else {
      iframePlayer.style.display = 'none';
      songList.style.display = 'none';
      document.getElementById('custom-player').style.display = 'none';
      loading.style.display = 'none';
      error.style.display = 'block';
    }
  }

  async function initializePlayer() {
    loading.style.display = 'block';
    error.style.display = 'none';
    loading.textContent = "Loading tracks (0%)...";
    try {
      const response = await new Promise(resolve => 
        chrome.runtime.sendMessage({ action: 'getCurrentState' }, resolve)
      );
      if (response && response.tracks.length > 0) {
        tracks = response.tracks;
        currentTrackIndex = safeTrackIndex(response.currentTrackIndex);
        toggleUIElements(true);

        for (let i = 0; i < tracks.length; i++) {
          addSongListItem(i);
          const progress = Math.round(((i + 1) / tracks.length) * 100);
          loading.textContent = `Loading tracks (${progress}%)...`;
          await new Promise(resolve => setTimeout(resolve, 50));
        }
        loading.style.display = 'none';
        playTrack(currentTrackIndex);
      } else {
        loading.style.display = 'none';
        error.style.display = 'block';
        error.textContent = "No tracks available.";
      }
    } catch (err) {
      console.error("Error initializing player:", err);
      loading.style.display = 'none';
      error.style.display = 'block';
      error.textContent = "Failed to initialize player.";
    }
  }

  createWaveSurfer();
  initializePlayer();

  playPauseBtn.addEventListener('click', () => {
    if (isPlaying) {
      wavesurfer.pause();
      isPlaying = false;
      playPauseBtn.innerHTML = '<i class="fas fa-play"></i>';
    } else if (!isLoading) {
      wavesurfer.play();
      isPlaying = true;
      playPauseBtn.innerHTML = '<i class="fas fa-pause"></i>';
    }
  });

  nextBtn.addEventListener('click', () => {
    if (currentTrackIndex < tracks.length - 1) {
      currentTrackIndex++;
      playTrack(currentTrackIndex);
    }
  });

  prevBtn.addEventListener('click', () => {
    if (currentTrackIndex > 0) {
      currentTrackIndex--;
      playTrack(currentTrackIndex);
    }
  });

  seekBar.addEventListener('input', () => {
    const duration = wavesurfer.getDuration();
    if (isFinite(duration)) {
      const seekTime = (seekBar.value / 100) * duration;
      wavesurfer.setTime(seekTime);
      updateProgress(seekTime);
    }
  });

  window.addEventListener('unload', () => {
    if (wavesurfer) wavesurfer.destroy();
  });
});