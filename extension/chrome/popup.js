document.addEventListener('DOMContentLoaded', () => {
  const songLimitSelect = document.getElementById('song-limit');
  const loadBtn = document.getElementById('load-btn');

  loadBtn.addEventListener('click', () => {
    const limit = parseInt(songLimitSelect.value, 10);
    chrome.runtime.sendMessage({ action: 'openPlayerWindow', limit: limit }, (response) => {
      if (response && response.success) {
        window.close();
      } else {
        console.error("Failed to open player window:", response);
      }
    });
  });
});