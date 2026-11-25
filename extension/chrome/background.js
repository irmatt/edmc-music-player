console.log("Background script loaded");

let globalState = {
  tracks: [],
  currentTrackIndex: 0,
  isExtensionEnabled: true,
  playerWindowId: null,
  sourceTabId: null,
  trackCache: new Map()
};

async function fetchTrackDetails(track) {
  const postUrl = track.postUrl;

  if (globalState.trackCache.has(postUrl)) {
    console.log(`Using cached details for ${postUrl}`);
    return globalState.trackCache.get(postUrl);
  }

  const cachedTrack = await new Promise(resolve => {
    chrome.storage.local.get(`track_${postUrl}`, (result) => {
      const cached = result[`track_${postUrl}`];
      if (cached && Date.now() - cached.timestamp < 24 * 60 * 60 * 1000) {
        globalState.trackCache.set(postUrl, cached);
        resolve(cached);
      } else {
        resolve(null);
      }
    });
  });
  if (cachedTrack) return cachedTrack;

  let fetchTab;
  try {
    fetchTab = await chrome.tabs.create({ url: 'about:blank', active: false });
    await chrome.tabs.update(fetchTab.id, { url: postUrl });

    await new Promise(resolve => {
      chrome.tabs.onUpdated.addListener(function listener(tabId, changeInfo) {
        if (tabId === fetchTab.id && changeInfo.status === 'complete') {
          chrome.tabs.onUpdated.removeListener(listener);
          resolve();
        }
      });
    });

    await chrome.scripting.executeScript({
      target: { tabId: fetchTab.id },
      files: ['content.js']
    });

    const response1 = await new Promise(resolve => {
      chrome.tabs.sendMessage(fetchTab.id, { action: "scrapeBeatexsUrlAndReaction" }, resolve);
    });

    let beatexsUrl = response1?.beatexsUrl || null;
    let reactionUrl = response1?.reactionUrl || null;

    let fullTrackDetails = {
      postUrl: postUrl,
      reactionUrl: reactionUrl,
      songName: track.songName,
      timestamp: Date.now(),
      url: null,
      beatexsUrl: null
    };

    if (beatexsUrl) {
      await chrome.tabs.update(fetchTab.id, { url: beatexsUrl });
      await new Promise(resolve => {
        chrome.tabs.onUpdated.addListener(function listener(tabId, changeInfo) {
          if (tabId === fetchTab.id && changeInfo.status === 'complete') {
            chrome.tabs.onUpdated.removeListener(listener);
            resolve();
          }
        });
      });

      await chrome.scripting.executeScript({
        target: { tabId: fetchTab.id },
        files: ['content.js']
      });

      const response2 = await new Promise(resolve => {
        chrome.tabs.sendMessage(fetchTab.id, { action: "scrapeMp3Metadata" }, resolve);
      });

      if (response2 && response2.url) {
        fullTrackDetails = {
          ...response2,
          postUrl: postUrl,
          reactionUrl: reactionUrl,
          songName: track.songName,
          beatexsUrl: beatexsUrl,
          timestamp: Date.now()
        };
      }
    }

    chrome.storage.local.set({ [`track_${postUrl}`]: fullTrackDetails });
    globalState.trackCache.set(postUrl, fullTrackDetails);
    return fullTrackDetails;
  } catch (error) {
    console.error('Fetch failed:', error);
    return { postUrl: postUrl, songName: track.songName, url: null, beatexsUrl: null, timestamp: Date.now() };
  } finally {
    if (fetchTab?.id) chrome.tabs.remove(fetchTab.id);
  }
}

async function fetchAllTracks(tabId, limit) {
  console.log(`Fetching all ${limit} tracks with details from tab:`, tabId);
  await chrome.scripting.executeScript({
    target: { tabId: tabId },
    files: ['content.js']
  });

  const postDataResponse = await new Promise(resolve => {
    chrome.tabs.sendMessage(tabId, { action: "scrapePostUrlsWithTitles", limit: limit }, resolve);
  });

  if (!postDataResponse || !postDataResponse.postData) {
    console.error("No post data found");
    return [];
  }

  const postData = postDataResponse.postData;
  const tracks = [];
  for (const subforum of Object.keys(postData)) {
    const postEntries = postData[subforum];
    for (const entry of postEntries) {
      tracks.push({ postUrl: entry.postUrl, songName: entry.songName || 'Unknown Song' });
    }
  }

  const detailedTracks = await Promise.all(tracks.map(track => fetchTrackDetails(track)));
  globalState.tracks = detailedTracks.filter(track => track);
  globalState.sourceTabId = tabId;

  chrome.storage.local.set({ tracks: globalState.tracks, forumUrl: (await chrome.tabs.get(tabId)).url });
  console.log("All tracks fetched and stored:", globalState.tracks);
  return globalState.tracks;
}

async function openPlayerWindow(limit) {
  const tabs = await chrome.tabs.query({ active: true, currentWindow: true });
  if (tabs.length > 0 && tabs[0].url.includes('edmc.to')) {
    globalState.sourceTabId = tabs[0].id;
    globalState.tracks = [];
    globalState.currentTrackIndex = 0;
    await fetchAllTracks(globalState.sourceTabId, limit);
  } else {
    const { tracks, forumUrl } = await new Promise(resolve => chrome.storage.local.get(['tracks', 'forumUrl'], resolve));
    if (tracks?.length > 0) {
      globalState.tracks = tracks;
    } else if (forumUrl) {
      const tab = await chrome.tabs.create({ url: forumUrl, active: false });
      await new Promise(resolve => {
        chrome.tabs.onUpdated.addListener(function listener(tabId, changeInfo) {
          if (tabId === tab.id && changeInfo.status === 'complete') {
            chrome.tabs.onUpdated.removeListener(listener);
            resolve();
          }
        });
      });
      globalState.sourceTabId = tab.id;
      await fetchAllTracks(tab.id, limit);
      chrome.tabs.remove(tab.id);
    } else {
      console.error("No valid edmc.to tab or stored data found");
      return;
    }
  }

  if (globalState.playerWindowId) {
    try {
      await chrome.windows.update(globalState.playerWindowId, { focused: true });
      return;
    } catch (error) {
      console.log("Player window not found, creating a new one.");
    }
  }

  const playerWindow = await chrome.windows.create({
    url: chrome.runtime.getURL('player.html'),
    type: 'popup',
    width: 800,
    height: 700
  });
  globalState.playerWindowId = playerWindow.id;
}

chrome.windows.onRemoved.addListener(windowId => {
  if (windowId === globalState.playerWindowId) {
    globalState.playerWindowId = null;
  }
});

chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
  if (request.action === "openPlayerWindow") {
    openPlayerWindow(request.limit).then(() => sendResponse({ success: true }))
      .catch(error => sendResponse({ success: false, error: error.message }));
    return true;
  }

  if (request.action === "getCurrentState") {
    sendResponse({
      tracks: globalState.tracks,
      currentTrackIndex: globalState.currentTrackIndex,
      isExtensionEnabled: globalState.isExtensionEnabled
    });
    return false;
  }

  if (request.action === "performReaction") {
    const reactionUrl = request.reactionUrl;
    chrome.tabs.create({ url: reactionUrl, active: false }, (tab) => {
      chrome.tabs.onUpdated.addListener(function listener(tabId, changeInfo) {
        if (tabId === tab.id && changeInfo.status === 'complete') {
          chrome.tabs.onUpdated.removeListener(listener);
          chrome.scripting.executeScript({
            target: { tabId: tab.id },
            func: () => {
              const thanksLink = document.querySelector('a.ipsReact_reaction.ipsReact_reaction_thanks');
              if (thanksLink) thanksLink.click();
            }
          }, () => {
            chrome.tabs.remove(tab.id);
            sendResponse({ success: true });
          });
        }
      });
    });
    return true;
  }

  return false;
});