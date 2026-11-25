use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use edmc_core::db::{now_secs, Db};
use edmc_core::{Forum, QualityVariant, Track, TrackBatch, TrackStatus};
use scraper::{Element, Html, Selector};
use url::Url;

use crate::error::{Error, Result};
use crate::http::HttpClient;

const HOME: &str = "https://edmc.to/";
const PAGE_SIZE: u32 = 50;

pub type ProgressFn = Arc<dyn Fn(usize, usize, &str) + Send + Sync>;

#[async_trait]
pub trait TrackSource: Send + Sync {
    async fn list_tree(&self) -> Result<Vec<Forum>>;
    async fn list_children(&self, forum: &Forum) -> Result<Vec<Forum>>;
    async fn load_pages(
        &self,
        forum: &Forum,
        start_page: u32,
        page_count: u32,
        progress: ProgressFn,
    ) -> Result<TrackBatch>;
    async fn refresh_page_one(&self, forum: &Forum, progress: ProgressFn) -> Result<TrackBatch>;
    async fn resolve_track(&self, track: &Track) -> Result<Track>;
    async fn qualities(&self, track: &Track) -> Result<Vec<QualityVariant>>;
    async fn send_thanks(&self, reaction_url: &str) -> Result<()>;
}

pub struct HtmlSource {
    http: Arc<HttpClient>,
    db: Arc<Db>,
}

impl HtmlSource {
    pub fn new(http: Arc<HttpClient>, db: Arc<Db>) -> Self {
        Self { http, db }
    }

    fn parse_forum_grid(page: &str, base: &Url, parent: Option<&str>) -> Result<Vec<Forum>> {
        let doc = Html::parse_document(page);
        let sel = Selector::parse(".cForumGrid--forum")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let title_sel = Selector::parse(".cForumGrid__title a[href]")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let mut seen = HashSet::new();
        let mut forums = Vec::new();
        for block in doc.select(&sel) {
            let Some(title) = block.select(&title_sel).next() else {
                continue;
            };
            let Some(href) = title.value().attr("href") else {
                continue;
            };
            let Ok(url) = base.join(href) else {
                continue;
            };
            let key = url.as_str().trim_end_matches('/').to_owned();
            if !seen.insert(key.clone()) {
                continue;
            }
            let name = title.text().collect::<String>().trim().to_owned();
            if name.is_empty() {
                continue;
            }
            forums.push(Forum::new(name, key + "/", parent.map(str::to_owned)));
        }
        Ok(forums)
    }

    fn parse_page(page: &str, base: &Url, forum: &Forum) -> Result<(Vec<Track>, u32)> {
        let doc = Html::parse_document(page);
        let row_sel = Selector::parse("li.ipsDataItem[data-rowid]")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let link_sel = Selector::parse("h4.ipsDataItem_title a[href]")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let meta_sel = Selector::parse(".ipsDataItem_meta")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let time_sel = Selector::parse("time[datetime]")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let stat_sel = Selector::parse(".ipsDataItem_stats li")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let reactions_sel = Selector::parse(".ipsReactOverview p")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let thumb_sel = Selector::parse("[data-background-src]")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let author_span_sel = Selector::parse("span")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let stat_number_sel = Selector::parse(".ipsDataItem_stats_number")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let stat_type_sel = Selector::parse(".ipsDataItem_stats_type")
            .map_err(|e| Error::Parse(e.to_string()))?;

        let mut tracks = Vec::new();
        let mut seen = HashSet::new();
        for row in doc.select(&row_sel) {
            let Some(link) = row.select(&link_sel).next() else {
                continue;
            };
            let Some(href) = link.value().attr("href") else {
                continue;
            };
            let Ok(post_url) = base.join(href) else {
                continue;
            };
            if !post_url.as_str().contains("/music/") {
                continue;
            }
            let id = post_url.as_str().to_owned();
            if !seen.insert(id.clone()) {
                continue;
            }
            let full_title = link
                .value()
                .attr("title")
                .map(str::to_owned)
                .unwrap_or_else(|| link.text().collect::<String>().trim().to_owned());

            let uploaded_at = row
                .select(&time_sel)
                .next()
                .and_then(|el| el.value().attr("datetime"))
                .unwrap_or("")
                .to_owned();

            let author = row
                .select(&meta_sel)
                .next()
                .map(|meta| {
                    meta.select(&author_span_sel)
                        .next()
                        .map(|span| span.text().collect::<String>().trim().to_owned())
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| {
                            meta.text()
                                .collect::<String>()
                                .trim()
                                .trim_start_matches("By")
                                .split(',')
                                .next()
                                .unwrap_or("")
                                .trim()
                                .to_owned()
                        })
                })
                .unwrap_or_default();

            let mut replies = 0i64;
            let mut views = 0i64;
            for stat in row.select(&stat_sel) {
                let number = stat
                    .select(&stat_number_sel)
                    .next()
                    .and_then(|el| el.text().collect::<String>().trim().replace(',', "").parse::<i64>().ok())
                    .unwrap_or(0);
                let label = stat
                    .select(&stat_type_sel)
                    .next()
                    .map(|el| el.text().collect::<String>().to_lowercase())
                    .unwrap_or_default();
                if label.contains("repl") {
                    replies = number;
                } else if label.contains("view") {
                    views = number;
                }
            }

            let thanks_count = row
                .select(&reactions_sel)
                .next()
                .and_then(|el| {
                    let text = el.text().collect::<String>();
                    text.split_whitespace().next()?.replace(',', "").parse::<i64>().ok()
                })
                .unwrap_or(0);

            let thumb_url = row
                .select(&thumb_sel)
                .next()
                .and_then(|el| el.value().attr("data-background-src"))
                .and_then(|src| absolute_url(base, src))
                .map(|url| url.to_string());

            let mut track = Track::from_listing(
                id,
                full_title,
                forum,
                author,
                uploaded_at,
                replies,
                views,
                thanks_count,
                thumb_url,
            );
            track.updated_at = now_secs();
            tracks.push(track);
        }

        let pagination = Selector::parse("[data-ipspagination-pages]")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let total_pages = doc
            .select(&pagination)
            .next()
            .and_then(|el| {
                el.value()
                    .attr("data-ipspagination-pages")
                    .or_else(|| el.value().attr("data-ipsPagination-pages"))
                    .or_else(|| el.value().attr("data-pages"))
            })
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(1)
            .max(1);
        Ok((tracks, total_pages))
    }

    async fn fetch_page(
        &self,
        forum: &Forum,
        page: u32,
        force: bool,
    ) -> Result<(Vec<Track>, u32)> {
        let base = Url::parse(&forum.url)?;
        let url = if page <= 1 {
            base.clone()
        } else {
            Url::parse(&format!("{}?page={}", forum.url.trim_end_matches('/'), page))?
        };
        let body = if force {
            self.http.get_text_revalidated(&url).await?.body
        } else {
            self.http.get_text(&url).await?.body
        };
        Self::parse_page(&body, &url, forum)
    }

    async fn load_range(
        &self,
        forum: &Forum,
        start_page: u32,
        page_count: u32,
        progress: ProgressFn,
        force_first: bool,
    ) -> Result<TrackBatch> {
        let mut tracks = Vec::new();
        let mut loaded = Vec::new();
        let mut errors = Vec::new();
        let mut total_pages = 1u32;
        let mut processed = 0usize;
        let expected = (page_count * PAGE_SIZE) as usize;

        for offset in 0..page_count {
            let page = start_page + offset;
            progress(processed, expected, &format!("Page {page}"));
            match self.fetch_page(forum, page, force_first && page == 1).await {
                Ok((mut page_tracks, pages)) => {
                    if page_tracks.is_empty() {
                        break;
                    }
                    total_pages = total_pages.max(pages);
                    processed += page_tracks.len();
                    progress(processed, expected, &format!("Page {page}"));
                    tracks.append(&mut page_tracks);
                    loaded.push(page);
                }
                Err(Error::Busy(_)) | Err(Error::RateLimited) => {
                    errors.push("The site asked us to slow down. Loaded pages were kept.".into());
                    break;
                }
                Err(error) => errors.push(format!("Page {page}: {error}")),
            }
        }

        if !tracks.is_empty() {
            self.db.upsert_listing_tracks(&tracks)?;
        }
        if !loaded.is_empty() {
            self.db.mark_pages_loaded(&forum.url, &loaded)?;
        }
        let mut all_loaded = self.db.loaded_pages(&forum.url)?;
        all_loaded.sort_unstable();
        all_loaded.dedup();
        Ok(TrackBatch {
            tracks,
            loaded_pages: all_loaded,
            total_pages,
            errors,
        })
    }

    async fn resolve_track_inner(&self, track: &Track) -> Result<Track> {
        if track.audio_url.is_some() && !track.beatexs_ids.is_empty() {
            return Ok(track.clone());
        }
        let post = Url::parse(&track.post_url)?;
        let body = self.http.get_text(&post).await?.body;
        let (ids, reaction_url) = {
            let doc = Html::parse_document(&body);
            let iframe_sel = Selector::parse("iframe[src*='beatexs.com']")
                .map_err(|e| Error::Parse(e.to_string()))?;
            let link_sel = Selector::parse("a[href*='beatexs.com']")
                .map_err(|e| Error::Parse(e.to_string()))?;
            let mut ids = Vec::new();
            let mut seen = HashSet::new();
            for el in doc.select(&iframe_sel).chain(doc.select(&link_sel)) {
                let Some(href) = el.value().attr("src").or_else(|| el.value().attr("href")) else {
                    continue;
                };
                let Ok(url) = post.join(href) else {
                    continue;
                };
                if let Some(id) = beatexs_id(&url) {
                    if seen.insert(id.clone()) {
                        ids.push(id);
                    }
                }
            }
            let thanks_sel = Selector::parse("a.ipsReact_reaction_thanks[href]")
                .map_err(|e| Error::Parse(e.to_string()))?;
            let reaction = doc
                .select(&thanks_sel)
                .next()
                .and_then(|el| el.value().attr("href"))
                .and_then(|href| post.join(href).ok())
                .map(|url| url.to_string());
            (ids, reaction)
        };

        let mut resolved = track.clone();
        resolved.beatexs_ids = ids.clone();
        resolved.reaction_url = reaction_url;
        resolved.beatexs_url = ids
            .first()
            .map(|id| format!("https://beatexs.com/nodl/{id}"));

        for id in ids.iter().take(4) {
            let preview = Url::parse(&format!("https://beatexs.com/nodl/{id}"))?;
            let Ok(page) = self.http.get_text(&preview).await else {
                continue;
            };
            let (audio, art) = {
                let doc = Html::parse_document(&page.body);
                let source_sel = Selector::parse("[data-source]")
                    .map_err(|e| Error::Parse(e.to_string()))?;
                let thumb_sel = Selector::parse("[data-thumb]")
                    .map_err(|e| Error::Parse(e.to_string()))?;
                let audio = doc
                    .select(&source_sel)
                    .next()
                    .and_then(|el| el.value().attr("data-source"))
                    .and_then(|src| absolute_url(&preview, src));
                let art = doc
                    .select(&thumb_sel)
                    .next()
                    .and_then(|el| el.value().attr("data-thumb"))
                    .and_then(|src| absolute_url(&preview, src));
                (audio, art)
            };
            if audio.is_some() {
                resolved.audio_url = audio.map(|url| url.to_string());
                resolved.art_url = art.map(|url| url.to_string());
                resolved.beatexs_url = Some(preview.to_string());
                break;
            }
        }

        resolved.status = if resolved.audio_url.is_some() {
            TrackStatus::Ready
        } else {
            TrackStatus::NoAudio
        };
        resolved.updated_at = now_secs();
        Ok(resolved)
    }
}

#[async_trait]
impl TrackSource for HtmlSource {
    async fn list_tree(&self) -> Result<Vec<Forum>> {
        let home = self.http.get_text(&Url::parse(HOME)?).await?.body;
        let doc = Html::parse_document(&home);
        let music_sel = Selector::parse("li[data-categoryID='6']")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let base = Url::parse(HOME)?;
        let Some(music) = doc.select(&music_sel).next() else {
            return Ok(vec![Forum::new(
                "Music".into(),
                "https://edmc.to/genre/music-6/".into(),
                None,
            )]);
        };
        let html = music.html();
        let mut children =
            Self::parse_forum_grid(&html, &base, Some("https://edmc.to/genre/music-6/"))?;
        let mut root = Forum::new("Music".into(), "https://edmc.to/genre/music-6/".into(), None);
        let mut urls: HashSet<String> = children.iter().map(|forum| forum.url.clone()).collect();
        let extras = [
            ("Drum & Bass", "https://edmc.to/genre/drum-bass-37/"),
            ("Dubstep", "https://edmc.to/genre/dubstep-31/"),
        ];
        for (name, url) in extras {
            if urls.insert(url.to_owned()) {
                children.push(Forum::new(
                    name.into(),
                    url.into(),
                    Some(root.url.clone()),
                ));
            }
        }
        root.children = children;
        Ok(vec![root])
    }

    async fn list_children(&self, forum: &Forum) -> Result<Vec<Forum>> {
        let base = Url::parse(&forum.url)?;
        let body = self.http.get_text(&base).await?.body;

        let grid = Self::parse_forum_grid(&body, &base, Some(&forum.url))?;
        if !grid.is_empty() {
            return Ok(grid);
        }

        // Genres like Dubstep and Drum & Bass list their children in the
        // site's navigation menu rather than in a forum grid.
        let doc = Html::parse_document(&body);
        let group_sel = Selector::parse("li[aria-haspopup]")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let heading_sel = Selector::parse("a[href='#']")
            .map_err(|e| Error::Parse(e.to_string()))?;
        let child_sel = Selector::parse(".grid-container3 li a[href]")
            .map_err(|e| Error::Parse(e.to_string()))?;

        let mut children = Vec::new();
        let mut seen = HashSet::new();
        for group in doc.select(&group_sel) {
            let heading = group
                .select(&heading_sel)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_owned())
                .unwrap_or_default();
            if !heading.eq_ignore_ascii_case(&forum.name) {
                continue;
            }
            for link in group.select(&child_sel) {
                let Some(href) = link.value().attr("href") else {
                    continue;
                };
                let Ok(url) = base.join(href) else {
                    continue;
                };
                if !url.as_str().contains("/genre/") || url.as_str() == forum.url {
                    continue;
                }
                let name = link.text().collect::<String>().trim().to_owned();
                if name.is_empty() || !seen.insert(url.as_str().to_owned()) {
                    continue;
                }
                children.push(Forum::new(name, url.to_string(), Some(forum.url.clone())));
            }
            break;
        }
        Ok(children)
    }

    async fn load_pages(
        &self,
        forum: &Forum,
        start_page: u32,
        page_count: u32,
        progress: ProgressFn,
    ) -> Result<TrackBatch> {
        self.load_range(forum, start_page.max(1), page_count.clamp(1, 5), progress, false)
            .await
    }

    async fn refresh_page_one(&self, forum: &Forum, progress: ProgressFn) -> Result<TrackBatch> {
        self.load_range(forum, 1, 1, progress, true).await
    }

    async fn resolve_track(&self, track: &Track) -> Result<Track> {
        let resolved = self.resolve_track_inner(track).await?;
        self.db.save_track(&resolved)?;
        Ok(resolved)
    }

    async fn qualities(&self, track: &Track) -> Result<Vec<QualityVariant>> {
        let track = if track.beatexs_ids.is_empty() {
            self.resolve_track_inner(track).await?
        } else {
            track.clone()
        };
        let mut variants = Vec::new();
        for id in track.beatexs_ids.iter().take(8) {
            let page_url = format!("https://beatexs.com/{id}");
            let url = Url::parse(&page_url)?;
            let Ok(page) = self.http.get_text(&url).await else {
                continue;
            };
            if let Some(variant) = parse_quality(&page.body, id, &page_url) {
                variants.push(variant);
            }
        }
        variants.sort_by(|a, b| b.score.cmp(&a.score));
        variants.dedup_by(|a, b| a.id == b.id);
        if variants.is_empty() {
            return Err(Error::Other("No downloadable originals were found on this post".into()));
        }
        Ok(variants)
    }

    async fn send_thanks(&self, reaction_url: &str) -> Result<()> {
        self.http.action(&Url::parse(reaction_url)?).await
    }
}

fn absolute_url(base: &Url, src: &str) -> Option<Url> {
    if src.starts_with("//") {
        Url::parse(&format!("https:{src}")).ok()
    } else {
        base.join(src).ok()
    }
}

pub fn beatexs_id(url: &Url) -> Option<String> {
    if url.host_str()? != "beatexs.com" {
        return None;
    }
    let segments: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    for (index, segment) in segments.iter().enumerate() {
        if matches!(*segment, "nodl" | "edl") {
            return segments.get(index + 1).map(|s| (*s).to_owned());
        }
    }
    segments.last().map(|s| (*s).to_owned())
}

fn parse_quality(page: &str, id: &str, page_url: &str) -> Option<QualityVariant> {
    let doc = Html::parse_document(page);
    let h4_sel = Selector::parse("h4").ok()?;
    let filename = doc
        .select(&h4_sel)
        .map(|el| el.text().collect::<String>().trim().to_owned())
        .find(|text| has_audio_extension(text))?;
    let format = filename
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_uppercase())
        .unwrap_or_else(|| "AUDIO".into());

    let stat_sel = Selector::parse(".statd").ok()?;
    let mut size_bytes = 0u64;
    let mut quality_label = format.clone();
    for stat in doc.select(&stat_sel) {
        let label = stat.text().collect::<String>().trim().to_lowercase();
        let Some(value_el) = stat.next_sibling_element() else {
            continue;
        };
        let value = value_el.text().collect::<String>().trim().to_owned();
        if label == "size" {
            size_bytes = parse_size(&value);
        } else if label == "bitrate" && !value.is_empty() {
            quality_label = value;
        }
    }

    let lossless = matches!(format.as_str(), "FLAC" | "WAV" | "AIFF" | "ALAC");
    let score = if lossless {
        1_000_000 + size_bytes
    } else {
        parse_bitrate(&quality_label).unwrap_or(0) * 1000 + size_bytes.min(999_999)
    };
    Some(QualityVariant {
        id: id.to_owned(),
        filename,
        format,
        quality_label,
        size_bytes,
        lossless,
        page_url: page_url.to_owned(),
        score,
    })
}

fn has_audio_extension(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["flac", "wav", "mp3", "m4a", "aac", "ogg", "aiff", "alac"]
        .iter()
        .any(|ext| lower.ends_with(&format!(".{ext}")))
}

fn parse_size(text: &str) -> u64 {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() < 2 {
        return 0;
    }
    let value = parts[0].parse::<f64>().unwrap_or(0.0);
    let multiplier = match parts[1].to_lowercase().as_str() {
        "gb" => 1024.0 * 1024.0 * 1024.0,
        "mb" => 1024.0 * 1024.0,
        "kb" => 1024.0,
        _ => 1.0,
    };
    (value * multiplier) as u64
}

fn parse_bitrate(text: &str) -> Option<u64> {
    text.to_lowercase()
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()
}
