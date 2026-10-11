//! A manga recipe as a catalogue source.

use std::sync::Arc;

use async_trait::async_trait;
use kuro_net::NetworkClient;
use kuro_source::model::{MangasPage, Page, SChapter, SManga};
use kuro_source::{CatalogueSource, FilterList, MangaSource, SourceError};
use url::Url;

use crate::engine::{self, Found};
use crate::fetch::{Fetcher, outside};
use crate::recipe::{Kind, Listing, Recipe, Template};

pub struct ScrapeManga {
    fetcher: Fetcher,
}

impl ScrapeManga {
    /// `None` when the recipe is not a manga recipe.
    pub fn new(client: NetworkClient, recipe: Arc<Recipe>) -> Option<Self> {
        (recipe.kind == Kind::Manga).then(|| Self { fetcher: Fetcher::new(client, recipe) })
    }

    fn recipe(&self) -> &Recipe {
        self.fetcher.recipe()
    }

    async fn list(&self, op: &Listing, template: &Template, page: i32, query: &str) -> Result<MangasPage, SourceError> {
        let url = self.recipe().listing_url(template, page, query).map_err(outside)?;
        let (html, at) = self.fetcher.get(&url).await?;
        let found = engine::listing(op, &html, &at, page)?;
        let mangas = found
            .entries
            .into_iter()
            .map(|entry| SManga { thumbnail_url: entry.cover, ..SManga::new(entry.url, entry.title) })
            .collect();
        Ok(MangasPage { mangas, has_next_page: found.has_next })
    }

    async fn page_of(&self, stored: &str) -> Result<(String, Url), SourceError> {
        self.fetcher.get(&self.recipe().address(stored).map_err(outside)?).await
    }
}

/// The details found on the page laid over the manga: what the recipe does not read stays as it was.
fn merged(manga: &SManga, found: Found) -> SManga {
    let mut out = manga.clone();
    if let Some(title) = found.title {
        out.title = title;
    }
    out.author = found.author.or(out.author);
    out.artist = found.artist.or(out.artist);
    out.description = found.description.or(out.description);
    out.genre = found.genre.or(out.genre);
    out.thumbnail_url = found.cover.or(out.thumbnail_url);
    if let Some(status) = found.status {
        out.status = status;
    }
    out.initialized = true;
    out
}

#[async_trait]
impl MangaSource for ScrapeManga {
    fn id(&self) -> i64 {
        self.fetcher.id()
    }

    fn name(&self) -> &str {
        &self.recipe().name
    }

    fn lang(&self) -> &str {
        &self.recipe().lang
    }

    async fn get_manga_details(&self, manga: &SManga) -> Result<SManga, SourceError> {
        let (html, at) = self.page_of(&manga.url).await?;
        Ok(merged(manga, engine::details(&self.recipe().details, &html, &at)?))
    }

    async fn get_chapter_list(&self, manga: &SManga) -> Result<Vec<SChapter>, SourceError> {
        let op = self.recipe().chapters.as_ref().ok_or(SourceError::Unsupported("chapters"))?;
        let (html, at) = self.page_of(&manga.url).await?;
        let items = engine::chapters(op, &html, &at)?;
        Ok(items
            .into_iter()
            .map(|item| SChapter {
                date_upload: item.date.unwrap_or(0),
                chapter_number: item.number.unwrap_or(-1.0),
                scanlator: item.scanlator,
                ..SChapter::new(item.url, item.name)
            })
            .collect())
    }

    async fn get_page_list(&self, chapter: &SChapter) -> Result<Vec<Page>, SourceError> {
        let op = self.recipe().pages.as_ref().ok_or(SourceError::Unsupported("pages"))?;
        let (html, at) = self.page_of(&chapter.url).await?;
        Ok(engine::pages(op, &html, &at)?
            .into_iter()
            .enumerate()
            .map(|(index, image)| Page { image_url: Some(image), ..Page::new(i32::try_from(index).unwrap_or(i32::MAX), "") })
            .collect())
    }
}

#[async_trait]
impl CatalogueSource for ScrapeManga {
    fn supports_latest(&self) -> bool {
        self.recipe().latest.is_some()
    }

    fn filter_list(&self) -> FilterList {
        FilterList::default()
    }

    async fn popular(&self, page: i32) -> Result<MangasPage, SourceError> {
        self.list(&self.recipe().popular, &self.recipe().urls.popular, page, "").await
    }

    async fn search(&self, page: i32, query: &str, _filters: &FilterList) -> Result<MangasPage, SourceError> {
        match (&self.recipe().search, &self.recipe().urls.search) {
            (Some(op), Some(template)) => self.list(op, template, page, query).await,
            _ => Err(SourceError::Unsupported("search")),
        }
    }

    async fn latest(&self, page: i32) -> Result<MangasPage, SourceError> {
        match (&self.recipe().latest, &self.recipe().urls.latest) {
            (Some(op), Some(template)) => self.list(op, template, page, "").await,
            _ => Err(SourceError::Unsupported("latest")),
        }
    }
}
