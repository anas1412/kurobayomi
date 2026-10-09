//! Test fixtures for the anime browse models, the fakes of `p2-edge-cases-and-tests.md` section 8: `FakeSource`,
//! `FakeCatalog`, `InMemoryAnimeRepo`, `RecordingCover`, `RecordingBackground`, `RecordingTrackBinder`,
//! `RecordingEpisodeFlags`, `FixedClock`, `StubLocaleText` and `Collate`, plus sample anime and a recording library
//! lookup. No network, no real source binary.
//!
//! Location decision (T-41b-047 asks for `tests/browse_anime/`): the app is one binary crate with no library target, so
//! an integration test under `tests/` cannot import any `src/browse` type. The fakes sit here, behind `#[cfg(test)]`,
//! where every browse test module reaches them.

use std::cmp::Ordering;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kuro_source::SourceError;
use kuro_source::anime::{AnimeSource, AnimesPage, SAnime};
use kuro_source::filter::Filter;
use kuro_source::filter_list::FilterList;
use tokio::sync::watch;

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

/// `FakeSource`: a source whose listing calls answer from a script, one answer per call, in order. Every call is logged as
/// `popular PAGE`, `latest PAGE` or `search PAGE TEXT FILTERS`.
pub struct FakeSource {
    id: i64,
    supports_latest: bool,
    filters: FilterList,
    script: Mutex<VecDeque<Result<AnimesPage, SourceError>>>,
    calls: Mutex<Vec<String>>,
}

impl FakeSource {
    pub fn new(id: i64, script: Vec<Result<AnimesPage, SourceError>>) -> Arc<Self> {
        Self::configured(id, true, FilterList::default(), script)
    }

    /// A source with `supports_latest` and the filters of its fresh browse screen set.
    pub fn configured(id: i64, supports_latest: bool, filters: FilterList, script: Vec<Result<AnimesPage, SourceError>>) -> Arc<Self> {
        Arc::new(Self { id, supports_latest, filters, script: Mutex::new(script.into()), calls: Mutex::new(Vec::new()) })
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
impl AnimeSource for FakeSource {
    fn id(&self) -> i64 {
        self.id
    }

    fn name(&self) -> &str {
        "Scripted"
    }

    fn supports_latest(&self) -> bool {
        self.supports_latest
    }

    fn filter_list(&self) -> FilterList {
        self.filters.clone()
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

/// The sources of the worked example in `p2-topic-sources-list.md` section 2: the local source, five installed sources
/// with the languages `en`, `all`, `ar` and the empty language, and the order the example needs. Names and ids are the
/// document's.
pub fn worked_example() -> Vec<kuro_domain::source::model::Source> {
    use kuro_domain::source::model::Source;
    vec![
        Source::new(0, "Local anime source", "other"),
        Source::new(11, "Alpha", "en"),
        Source::new(12, "beta", "en"),
        Source::new(13, "Gamma", "all"),
        Source::new(14, "Delta", "ar"),
        Source::new(15, "Epsilon", ""),
        Source::new(16, "Zeta", "en"),
    ]
}

/// A filter list with one of every `Filter` variant, for the filter fixtures.
pub fn every_filter_variant() -> FilterList {
    FilterList(vec![
        Filter::header("Header"),
        Filter::separator("Separator"),
        Filter::select("Select", vec!["A".into(), "B".into()]),
        Filter::text("Text"),
        Filter::check_box("CheckBox"),
        Filter::tri_state("TriState"),
        Filter::group("Group", vec![Filter::check_box("Inner")]),
        Filter::sort("Sort", vec!["Title".into(), "Date".into()]),
    ])
}

/// `FakeCatalog`: the installed sources, their extension packages and the registry's initialised flag. An id that is
/// not installed answers a stub.
pub struct FakeCatalog {
    sources: Vec<kuro_domain::source::model::Source>,
    packages: BTreeMap<i64, String>,
    initialized: watch::Sender<bool>,
}

impl FakeCatalog {
    /// The local source (id 0, lang "other"), English, Multi (`all`), Arabic and the empty-language source of the worked
    /// example, each installed from an extension package named after its id, and the registry initialised.
    pub fn new() -> Self {
        let sources = worked_example();
        let packages = sources.iter().filter(|s| s.id != 0).map(|s| (s.id, format!("eu.example.ext{}", s.id))).collect();
        Self { sources, packages, initialized: watch::channel(true).0 }
    }

    pub fn sources(&self) -> &[kuro_domain::source::model::Source] {
        &self.sources
    }

    /// The installed source with `id`, or a stub with no name and no language (`get_or_stub`).
    pub fn get_or_stub(&self, id: i64) -> kuro_domain::source::model::Source {
        match self.sources.iter().find(|s| s.id == id) {
            Some(source) => source.clone(),
            None => kuro_domain::source::model::Source { is_stub: true, ..kuro_domain::source::model::Source::new(id, "", "") }
        }
    }

    /// The package of the extension that provides `id`; the local source and a stub have none.
    pub fn package_of(&self, id: i64) -> Option<&str> {
        self.packages.get(&id).map(String::as_str)
    }

    pub fn is_initialized(&self) -> bool {
        *self.initialized.borrow()
    }

    pub fn set_initialized(&self, value: bool) {
        self.initialized.send_replace(value);
    }

    pub fn subscribe_initialized(&self) -> watch::Receiver<bool> {
        self.initialized.subscribe()
    }
}

/// One stored row of the `InMemoryAnimeRepo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: i64,
    pub source: i64,
    pub url: String,
    pub title: String,
    pub favorite: bool,
}

/// `InMemoryAnimeRepo`: rows with url, source, favourite flag and title, the `(url, source)` lookup that inserts a
/// non-favourite row, the duplicate query (favourites with the same lower-cased title, other id, by id) and the
/// favourite count per source. Row ids start at 1.
#[derive(Default)]
pub struct InMemoryAnimeRepo {
    rows: Mutex<Vec<Row>>,
}

impl InMemoryAnimeRepo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, source: i64, url: &str, title: &str, favorite: bool) -> i64 {
        let mut rows = self.rows.lock().expect("rows");
        let id = rows.len() as i64 + 1;
        rows.push(Row { id, source, url: url.to_owned(), title: title.to_owned(), favorite });
        id
    }

    /// The row of `(url, source)`, inserted as a non-favourite with `title` when there is none. An existing row keeps
    /// its stored title.
    pub fn lookup_or_insert(&self, source: i64, url: &str, title: &str) -> Row {
        let found = self.rows.lock().expect("rows").iter().find(|r| r.source == source && r.url == url).cloned();
        match found {
            Some(row) => row,
            None => {
                let id = self.insert(source, url, title, false);
                self.row(id).expect("inserted")
            }
        }
    }

    pub fn row(&self, id: i64) -> Option<Row> {
        self.rows.lock().expect("rows").iter().find(|r| r.id == id).cloned()
    }

    /// The ids of the other favourites that carry `title`, compared with ASCII lower case, by id.
    pub fn duplicates(&self, id: i64, title: &str) -> Vec<i64> {
        let wanted = title.to_ascii_lowercase();
        let mut found: Vec<i64> =
            self.rows.lock().expect("rows").iter().filter(|r| r.favorite && r.id != id && r.title.to_ascii_lowercase() == wanted).map(|r| r.id).collect();
        found.sort_unstable();
        found
    }

    /// `(source, favourite count)` for each source that has a favourite, by source.
    pub fn favorite_counts(&self) -> Vec<(i64, usize)> {
        let mut counts: BTreeMap<i64, usize> = BTreeMap::new();
        for row in self.rows.lock().expect("rows").iter().filter(|r| r.favorite) {
            *counts.entry(row.source).or_default() += 1;
        }
        counts.into_iter().collect()
    }

    pub fn set_favorite(&self, id: i64, favorite: bool) {
        if let Some(row) = self.rows.lock().expect("rows").iter_mut().find(|r| r.id == id) {
            row.favorite = favorite;
        }
    }

    /// A browse `Lookup` over the rows of `source`: an existing row shows its stored title (E-BR-10).
    pub fn lookup(self: &Arc<Self>, source: i64) -> Lookup {
        let repo = Arc::clone(self);
        Arc::new(move |entry: &NetworkEntry| {
            let row = repo.lookup_or_insert(source, &entry.url, &entry.title);
            Ok(LocalEntry { id: row.id, favorite: row.favorite, title: row.title })
        })
    }
}

/// `RecordingCover` and `RecordingBackground`: record each `delete_for` call and answer a configured count.
pub struct RecordingCover {
    count: usize,
    calls: Mutex<Vec<i64>>,
}

pub type RecordingBackground = RecordingCover;

impl RecordingCover {
    pub fn answering(count: usize) -> Self {
        Self { count, calls: Mutex::new(Vec::new()) }
    }

    pub fn delete_for(&self, anime_id: i64) -> usize {
        self.calls.lock().expect("calls").push(anime_id);
        self.count
    }

    pub fn calls(&self) -> Vec<i64> {
        self.calls.lock().expect("calls").clone()
    }
}

/// `RecordingTrackBinder`: records the anime a track was bound for.
#[derive(Default)]
pub struct RecordingTrackBinder {
    calls: Mutex<Vec<i64>>,
}

impl RecordingTrackBinder {
    pub fn bind(&self, anime_id: i64) {
        self.calls.lock().expect("calls").push(anime_id);
    }

    pub fn calls(&self) -> Vec<i64> {
        self.calls.lock().expect("calls").clone()
    }
}

/// `RecordingEpisodeFlags`: records `(anime id, flags)` writes.
#[derive(Default)]
pub struct RecordingEpisodeFlags {
    calls: Mutex<Vec<(i64, i64)>>,
}

impl RecordingEpisodeFlags {
    pub fn set(&self, anime_id: i64, flags: i64) {
        self.calls.lock().expect("calls").push((anime_id, flags));
    }

    pub fn calls(&self) -> Vec<(i64, i64)> {
        self.calls.lock().expect("calls").clone()
    }
}

/// `FixedClock`: always the same instant, in milliseconds since the epoch.
#[derive(Clone, Copy)]
pub struct FixedClock(pub i64);

impl FixedClock {
    pub fn now_ms(self) -> i64 {
        self.0
    }
}

/// `StubLocaleText`: the English text of a string key (`p2-data-and-config.md` section 7), and the display name of the
/// languages the fixtures use. A key it does not know reads as itself.
pub struct StubLocaleText;

impl StubLocaleText {
    pub fn text(key: &str) -> String {
        match key {
            "action_global_search" => "Global search",
            "action_filter" => "Filter",
            "internal_error" => "InternalError: Check crash logs for further information",
            "source_empty_screen" => "No source found",
            "action_pin" => "Pin",
            "action_unpin" => "Unpin",
            "action_disable" => "Disable",
            "last_used_source" => "Last used",
            "pinned_sources" => "Pinned",
            "other_source" => "Other",
            "multi_lang" => "Multi",
            "latest" => "Latest",
            "popular" => "Popular",
            "local_anime_source" => "Local anime source",
            "action_display_mode" => "Display mode",
            "action_display_comfortable_grid" => "Comfortable grid",
            "action_display_grid" => "Compact grid",
            "action_display_list" => "List",
            "label_help" => "Help",
            "action_open_in_web_view" => "Open in WebView",
            "action_settings" => "Settings",
            "action_retry" => "Retry",
            "no_results_found" => "No results found",
            other => other,
        }
        .to_owned()
    }

    /// The English display name of a language tag: `all` is Multi and the empty tag is Other.
    pub fn language(tag: &str) -> String {
        match tag {
            "ar" => "Arabic",
            "en" => "English",
            "all" => "Multi",
            "" => "Other",
            other => other,
        }
        .to_owned()
    }
}

/// `Collate`: the comparison the migrate list sorts with. Byte order, for deterministic tests; a locale-aware (ICU)
/// collation is the optional second variant and is not faked.
#[derive(Clone, Copy)]
pub enum Collate {
    Bytes,
}

impl Collate {
    pub fn compare(self, a: &str, b: &str) -> Ordering {
        match self {
            Collate::Bytes => a.as_bytes().cmp(b.as_bytes()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_answers_stubs_packages_and_the_initialised_flag() {
        let catalog = FakeCatalog::new();
        let stub = catalog.get_or_stub(999);
        assert_eq!((stub.is_stub, stub.name.as_str(), stub.lang.as_str()), (true, "", ""));
        assert!(!catalog.get_or_stub(11).is_stub);
        assert_eq!(catalog.package_of(11), Some("eu.example.ext11"));
        assert_eq!(catalog.package_of(0), None, "the local source has no package");
        assert_eq!(catalog.package_of(999), None);
        let ready = catalog.subscribe_initialized();
        assert!(catalog.is_initialized() && *ready.borrow());
        catalog.set_initialized(false);
        assert!(!*ready.borrow());
    }

    #[test]
    fn the_repo_keeps_stored_titles_finds_duplicates_by_ascii_lower_case_and_counts_favourites() {
        let repo = Arc::new(InMemoryAnimeRepo::new());
        let a = repo.insert(7, "/a", "Naruto", true);
        let b = repo.insert(7, "/b", "NARUTO", true);
        repo.insert(8, "/c", "naruto", false);
        assert_eq!(repo.duplicates(a, "Naruto"), [b], "a non-favourite and the entry itself are left out");
        assert_eq!(repo.lookup_or_insert(7, "/a", "Listed").title, "Naruto", "an existing row keeps its stored title");
        assert_eq!(repo.lookup_or_insert(7, "/new", "Listed").title, "Listed");
        assert_eq!(repo.favorite_counts(), [(7, 2)]);
        repo.set_favorite(a, false);
        assert_eq!(repo.favorite_counts(), [(7, 1)]);
        let lookup = repo.lookup(7);
        let entry = lookup(&NetworkEntry { url: "/b".into(), title: "x".into(), cover: None }).expect("row");
        assert_eq!((entry.id, entry.favorite, entry.title.as_str()), (b, true, "NARUTO"));
    }

    #[test]
    fn the_recorders_keep_their_calls_and_the_clock_and_collation_are_fixed() {
        let cover = RecordingCover::answering(3);
        assert_eq!(cover.delete_for(5), 3);
        assert_eq!(cover.calls(), [5]);
        let (binder, flags) = (RecordingTrackBinder::default(), RecordingEpisodeFlags::default());
        binder.bind(1);
        flags.set(1, 6);
        assert_eq!((binder.calls(), flags.calls()), (vec![1], vec![(1, 6)]));
        assert_eq!(FixedClock(1_700_000_000_000).now_ms(), 1_700_000_000_000);
        assert_eq!(Collate::Bytes.compare("B", "a"), Ordering::Less, "byte order puts upper case first");
        assert_eq!(StubLocaleText::text("multi_lang"), "Multi");
        assert_eq!(StubLocaleText::language(""), "Other");
        assert_eq!(StubLocaleText::text("unknown_key"), "unknown_key");
    }

    #[test]
    fn the_fake_source_scripts_latest_support_filters_and_errors() {
        let source = FakeSource::configured(1, false, every_filter_variant(), vec![Err(SourceError::Http(500))]);
        assert!(!source.supports_latest());
        assert_eq!(source.filter_list().0.len(), 8);
    }
}
