//! Test fixtures for the anime browse models: a source whose listing calls answer from a script, sample anime, and a
//! library lookup that records its calls. No network and no database.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kuro_source::SourceError;
use kuro_source::anime::{AnimeSource, AnimesPage, SAnime};
use kuro_source::filter_list::FilterList;

use super::loader::Lookup;
use crate::browse::entries::LocalEntry;
use crate::browse::loader::NetworkEntry;

/// An anime with `url` and `title`, and a thumbnail named after the url.
pub fn anime(url: &str, title: &str) -> SAnime {
    SAnime { url: url.to_owned(), title: title.to_owned(), thumbnail_url: Some(format!("{url}.jpg")), ..SAnime::default() }
}

pub fn page(animes: Vec<SAnime>, has_next_page: bool) -> AnimesPage {
    AnimesPage { animes, has_next_page }
}

/// A source whose listing calls answer from a script, one answer per call, in order. Every call is logged as
/// `popular PAGE`, `latest PAGE` or `search PAGE TEXT FILTERS`.
pub struct ScriptedSource {
    id: i64,
    script: Mutex<VecDeque<Result<AnimesPage, SourceError>>>,
    calls: Mutex<Vec<String>>,
}

impl ScriptedSource {
    pub fn new(id: i64, script: Vec<Result<AnimesPage, SourceError>>) -> Arc<Self> {
        Arc::new(Self { id, script: Mutex::new(script.into()), calls: Mutex::new(Vec::new()) })
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls").clone()
    }

    fn answer(&self, call: String) -> Result<AnimesPage, SourceError> {
        self.calls.lock().expect("calls").push(call);
        self.script.lock().expect("script").pop_front().expect("a scripted answer for each call")
    }
}

#[async_trait]
impl AnimeSource for ScriptedSource {
    fn id(&self) -> i64 {
        self.id
    }

    fn name(&self) -> &str {
        "Scripted"
    }

    fn supports_latest(&self) -> bool {
        true
    }

    fn provides_hosters(&self) -> bool {
        false
    }

    async fn popular(&self, page: i32) -> Result<AnimesPage, SourceError> {
        self.answer(format!("popular {page}"))
    }

    async fn latest(&self, page: i32) -> Result<AnimesPage, SourceError> {
        self.answer(format!("latest {page}"))
    }

    async fn search(&self, page: i32, query: &str, filters: &FilterList) -> Result<AnimesPage, SourceError> {
        self.answer(format!("search {page} {query} {}", filters.0.len()))
    }
}

/// A library lookup over `favourites` (url, row id). A listed url found there is a favourite whose stored title is
/// "stored URL". Any other url is inserted as a non-favourite row with id 900 and its own title. Every call is recorded
/// as `local URL`, and the record is returned with the lookup.
pub fn lookup(favourites: &[(&str, i64)]) -> (Lookup, Arc<Mutex<Vec<String>>>) {
    let known: Vec<(String, i64)> = favourites.iter().map(|(url, id)| ((*url).to_owned(), *id)).collect();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&calls);
    let lookup: Lookup = Arc::new(move |entry: &NetworkEntry| {
        log.lock().expect("log").push(format!("local {}", entry.url));
        Ok(match known.iter().find(|(url, _)| *url == entry.url) {
            Some((_, id)) => LocalEntry { id: *id, favorite: true, title: format!("stored {}", entry.url) },
            None => LocalEntry { id: 900, favorite: false, title: entry.title.clone() },
        })
    });
    (lookup, calls)
}
