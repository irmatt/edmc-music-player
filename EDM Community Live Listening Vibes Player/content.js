function scrapePostUrlsWithTitles(limit = 5) {
  const postData = {};
  let subforumName = "Unknown Subforum";
  const breadcrumbs = document.querySelectorAll('[itemtype="http://schema.org/BreadcrumbList"] [itemprop="itemListElement"]');
  if (breadcrumbs.length > 0) {
    const lastBreadcrumb = breadcrumbs[breadcrumbs.length - 1];
    subforumName = lastBreadcrumb.querySelector('[itemprop="name"]').textContent.trim();
  } else {
    const heading = document.querySelector('h1.ipsType_pageTitle');
    if (heading) subforumName = heading.textContent.trim();
  }

  const postEntries = [];
  const postElements = document.querySelectorAll('ol.ipsDataList li.ipsDataItem');
  console.log(`Found ${postElements.length} post elements with selector 'ol.ipsDataList li.ipsDataItem'`);

  postElements.forEach((post, index) => {
    if (index >= limit) return;
    const linkElement = post.querySelector('h4.ipsDataItem_title a');
    if (linkElement) {
      const postUrl = linkElement.href.split('?')[0];
      const songName = linkElement.textContent.trim();
      console.log(`Post ${index + 1}: URL=${postUrl}, Song Name=${songName}`);
      if (postUrl && postUrl.includes('edmc.to/music/')) {
        postEntries.push({ postUrl: postUrl, songName: songName });
      }
    } else {
      console.log(`Post ${index + 1}: No link element found with selector 'h4.ipsDataItem_title a'`);
    }
  });

  console.log("Scraped post entries:", postEntries);
  postData[subforumName] = postEntries;
  return postData;
}

function scrapeBeatexsUrlAndReaction() {
  let subforumName = "Unknown Subforum";
  const breadcrumbs = document.querySelectorAll('[itemtype="http://schema.org/BreadcrumbList"] [itemprop="itemListElement"]');
  if (breadcrumbs.length > 0) {
    const lastBreadcrumb = breadcrumbs[breadcrumbs.length - 1];
    subforumName = lastBreadcrumb.querySelector('[itemprop="name"]').textContent.trim();
  }

  const posts = document.querySelectorAll('li.ipsDataItem');
  if (posts.length === 0) {
    console.log("No posts found on page");
    return { subforum: subforumName, beatexsUrl: null, reactionUrl: null };
  }

  const post = posts[0];
  let beatexsUrl = null;
  const iframe = document.querySelector('iframe[src*="beatexs.com"]');
  if (iframe) {
    console.log(`Found beatexs URL in iframe src: ${iframe.src}`);
    beatexsUrl = iframe.src;
  } else {
    const beatexsLink = document.querySelector('a[href*="beatexs.com"]');
    if (beatexsLink) {
      console.log(`Found beatexs URL in <a> tag: ${beatexsLink.href}`);
      beatexsUrl = beatexsLink.href;
    }
  }

  const reactionLink = document.querySelector('a.ipsReact_reaction.ipsReact_reaction_thanks');
  const reactionUrl = reactionLink ? reactionLink.href : null;
  console.log(`Reaction URL: ${reactionUrl}`);

  return { subforum: subforumName, beatexsUrl: beatexsUrl, reactionUrl: reactionUrl };
}

function scrapeMp3Metadata() {
  const audioSource = document.querySelector('body > div audio source[src$=".mp3"], body > div audio source[src$=".wav"], body > div audio source[src$=".ogg"]');
  const songName = document.querySelector('.file-title .the-title')?.textContent || 'Unknown Song';
  const albumArt = document.querySelector('body > div')?.getAttribute('data-thumb') || '';
  const beatexsUrl = window.location.href;

  if (audioSource) {
    console.log(`Found mp3 URL: ${audioSource.src}`);
    console.log(`Song name: ${songName}`);
    console.log(`Album art: ${albumArt}`);
    console.log(`Beatexs URL: ${beatexsUrl}`);
    return {
      url: audioSource.src,
      songName: songName,
      albumArt: albumArt,
      beatexsUrl: beatexsUrl
    };
  }

  console.log("No mp3 URL found");
  return {
    url: null,
    songName: 'Unknown Song',
    albumArt: '',
    beatexsUrl: window.location.href
  };
}

chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
  console.log("Content script received message:", request);
  if (request.action === "scrapePostUrlsWithTitles") {
    const postData = scrapePostUrlsWithTitles(request.limit || 50);
    sendResponse({ postData: postData });
    return false;
  } else if (request.action === "scrapeBeatexsUrlAndReaction") {
    sendResponse(scrapeBeatexsUrlAndReaction());
    return false;
  } else if (request.action === "scrapeMp3Metadata") {
    sendResponse(scrapeMp3Metadata());
    return false;
  }
});