//! An anime recipe as an anime source. Videos are the direct addresses the page gives (a file or an HLS playlist); no
//! extractors.

use kuro_platform::kind::Kind;
use std::sync::Arc;

use async_trait::async_trait;
use kuro_net::NetworkClient;
use kuro_source::anime::{AnimeSource, AnimeStatus, AnimesPage, Headers, SAnime, SEpisode, Video};
use kuro_source::{FilterList, SourceError};

use crate::engine;
use crate::fetch::{Fetcher, outside};
use crate::recipe::{Listing, Recipe, Template};

pub struct ScrapeAnime {
    fetcher: Fetcher,
}

impl ScrapeAnime {
    /// `None` when the recipe is not an anime recipe.
    pub fn new(client: NetworkClient, recipe: Arc<Recipe>) -> Option<Self> {
        (recipe.kind == Kind::Anime).then(|| Self { fetcher: Fetcher::new(client, recipe) })
    }

    fn recipe(&self) -> &Recipe {
        self.fetcher.recipe()
    }

    async fn list(&self, op: &Listing, template: &Template, page: i32, query: &str) -> Result<AnimesPage, SourceError> {
        let url = self.recipe().listing_url(template, page, query).map_err(outside)?;
        let (html, at) = self.fetcher.get(&url).await?;
        let found = engine::listing(op, &html, &at, page)?;
        let animes = found
            .entries
            .into_iter()
            .map(|entry| SAnime { url: entry.url, title: entry.title, thumbnail_url: entry.cover, ..SAnime::default() })
            .collect();
        Ok(AnimesPage { animes, has_next_page: found.has_next })
    }
}

#[async_trait]
impl AnimeSource for ScrapeAnime {
    fn id(&self) -> i64 {
        self.fetcher.id()
    }

    fn name(&self) -> &str {
        &self.recipe().name
    }

    fn lang(&self) -> &str {
        &self.recipe().lang
    }

    fn supports_latest(&self) -> bool {
        self.recipe().latest.is_some()
    }

    fn supports_season_list(&self) -> bool {
        false
    }

    fn provides_hosters(&self) -> bool {
        false
    }

    fn filter_list(&self) -> FilterList {
        FilterList::default()
    }

    async fn popular(&self, page: i32) -> Result<AnimesPage, SourceError> {
        self.list(&self.recipe().popular, &self.recipe().urls.popular, page, "").await
    }

    async fn latest(&self, page: i32) -> Result<AnimesPage, SourceError> {
        match (&self.recipe().latest, &self.recipe().urls.latest) {
            (Some(op), Some(template)) => self.list(op, template, page, "").await,
            _ => Err(SourceError::Unsupported("latest")),
        }
    }

    async fn search(&self, page: i32, query: &str, _filters: &FilterList) -> Result<AnimesPage, SourceError> {
        match (&self.recipe().search, &self.recipe().urls.search) {
            (Some(op), Some(template)) => self.list(op, template, page, query).await,
            _ => Err(SourceError::Unsupported("search")),
        }
    }

    async fn anime_details(&self, anime: &SAnime) -> Result<SAnime, SourceError> {
        let url = self.recipe().address(&anime.url).map_err(outside)?;
        let (html, at) = self.fetcher.get(&url).await?;
        let found = engine::details(&self.recipe().details, &html, &at)?;
        let mut out = anime.clone();
        if let Some(title) = found.title {
            out.title = title;
        }
        out.author = found.author.or(out.author);
        out.artist = found.artist.or(out.artist);
        out.description = found.description.or(out.description);
        out.genre = found.genre.or(out.genre);
        out.thumbnail_url = found.cover.or(out.thumbnail_url);
        if let Some(status) = found.status {
            out.status = AnimeStatus::from_i32(status);
        }
        out.initialized = true;
        Ok(out)
    }

    async fn episode_list(&self, anime: &SAnime) -> Result<Vec<SEpisode>, SourceError> {
        let op = self.recipe().episodes.as_ref().ok_or(SourceError::Unsupported("episodes"))?;
        let url = self.recipe().address(&anime.url).map_err(outside)?;
        let (html, at) = self.fetcher.get(&url).await?;
        Ok(engine::chapters(op, &html, &at)?
            .into_iter()
            .map(|item| SEpisode {
                url: item.url,
                name: item.name,
                episode_number: item.number.unwrap_or(-1.0),
                scanlator: item.scanlator,
                date_upload: item.date.unwrap_or(0),
                ..SEpisode::default()
            })
            .collect())
    }

    async fn videos_for_episode(&self, episode: &SEpisode) -> Result<Vec<Video>, SourceError> {
        let op = self.recipe().videos.as_ref().ok_or(SourceError::Unsupported("videos"))?;
        let url = self.recipe().address(&episode.url).map_err(outside)?;
        let (html, at) = self.fetcher.get(&url).await?;
        let headers = self.fetcher.file_headers();
        Ok(engine::videos(op, &html, &at)?
            .into_iter()
            .map(|link| Video {
                headers: (!headers.is_empty()).then(|| Headers(headers.clone())),
                initialized: true,
                ..Video::new(link.url, link.title)
            })
            .collect())
    }
}
