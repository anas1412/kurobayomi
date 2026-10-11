//! A manga catalogue source over fixed answers for the tests of the Browse and entry flows. Every call is logged; `fail`
//! makes every call answer HTTP 503.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kuro_source::filter_list::FilterList;
use kuro_source::model::{MangasPage, Page, SChapter, SManga};
use kuro_source::{CatalogueSource, MangaSource, SourceError};

/// Two mangas per listing, one detail text and two chapters.
pub struct FakeCatalogue {
    pub id: i64,
    pub fail: AtomicBool,
    pub calls: Mutex<Vec<String>>,
}

impl FakeCatalogue {
    pub fn new(id: i64) -> Arc<Self> {
        Arc::new(Self { id, fail: false.into(), calls: Mutex::default() })
    }

    fn log(&self, call: String) -> Result<(), SourceError> {
        self.calls.lock().unwrap().push(call);
        if self.fail.load(Ordering::SeqCst) { Err(SourceError::Http(503)) } else { Ok(()) }
    }
}

#[async_trait]
impl MangaSource for FakeCatalogue {
    fn id(&self) -> i64 {
        self.id
    }
    fn name(&self) -> &str {
        "Fake Central"
    }
    fn lang(&self) -> &str {
        "en"
    }
    async fn get_manga_details(&self, manga: &SManga) -> Result<SManga, SourceError> {
        self.log(format!("details {}", manga.url))?;
        Ok(SManga {
            description: Some("A story.".into()),
            genre: Some("Action, Drama".into()),
            status: kuro_source::model::manga::ONGOING,
            author: Some("Author A".into()),
            thumbnail_url: Some("https://img.example/cover.jpg".into()),
            ..SManga::new(manga.url.clone(), "Remote title")
        })
    }
    async fn get_chapter_list(&self, manga: &SManga) -> Result<Vec<SChapter>, SourceError> {
        self.log(format!("chapters {}", manga.url))?;
        let chapter = |n: u32| SChapter {
            url: format!("{}/c{n}", manga.url),
            name: format!("Chapter {n}"),
            date_upload: 1_700_000_000_000,
            chapter_number: n as f32,
            scanlator: None,
        };
        Ok(vec![chapter(2), chapter(1)])
    }
    async fn get_page_list(&self, _chapter: &SChapter) -> Result<Vec<Page>, SourceError> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl CatalogueSource for FakeCatalogue {
    fn supports_latest(&self) -> bool {
        true
    }
    fn filter_list(&self) -> FilterList {
        FilterList(vec![kuro_source::Filter::check_box("Completed")])
    }
    async fn popular(&self, page: i32) -> Result<MangasPage, SourceError> {
        self.log(format!("popular {page}"))?;
        let manga = |url: &str, title: &str| SManga { thumbnail_url: Some(format!("https://img.example{url}.jpg")), ..SManga::new(url, title) };
        Ok(MangasPage { mangas: vec![manga("/m/one", "One"), manga("/m/two", "Two")], has_next_page: true })
    }
    async fn search(&self, page: i32, query: &str, filters: &FilterList) -> Result<MangasPage, SourceError> {
        self.log(format!("search {page} {query} {}", filters.len()))?;
        Ok(MangasPage { mangas: vec![SManga::new("/m/found", query)], has_next_page: false })
    }
    async fn latest(&self, page: i32) -> Result<MangasPage, SourceError> {
        self.log(format!("latest {page}"))?;
        Ok(MangasPage { mangas: Vec::new(), has_next_page: false })
    }
}
