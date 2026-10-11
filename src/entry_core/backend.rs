//! What the page needs from outside its own database: the source name, the two fetches, the download states and
//! the download removal. The fetches persist their result (as `updateManga.awaitUpdateFromSource` does), so the
//! page sees the new rows through the store subscriptions. Nothing here imports GTK. `SourceBackend` is the one the app
//! uses; `StubBackend` answers for a source that is not installed.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use kuro_data::manga::{Manga, MangaRepo};
use kuro_data::source::{MangaStubSourceRepo, StubSource};
use kuro_db::handle::Db;
use kuro_domain::entries::source_convert::{refresh_update, to_s_manga};
use kuro_domain::items::chapter_sync::{ChapterSyncError, ChapterSyncOptions, NoRenames, SyncChapters};
use kuro_domain::items::source_sync::MangaSyncLocks;
use kuro_downloads::DownloadState;
use kuro_source::model::SChapter;
use kuro_source::{MangaSource, SourceError};

use crate::entry_core::detail::SourceInfo;
use crate::entry_core::errors::DetailError;

pub type Fetch = Pin<Box<dyn Future<Output = Result<(), DetailError>> + Send>>;

pub trait Backend: Send + Sync + 'static {
    /// The source line of the header. `stub` is true when the extension is not installed.
    fn source_info(&self, source_id: i64) -> SourceInfo;
    /// Fetches the details from the source and stores them.
    fn fetch_details(&self, manga_id: i64) -> Fetch;
    /// Fetches the chapter list from the source and stores it.
    fn fetch_chapters(&self, manga_id: i64) -> Fetch;
    /// The download state of each chapter of the manga that has one.
    fn downloads(&self, _manga_id: i64) -> HashMap<i64, DownloadState> {
        HashMap::new()
    }
    /// Removes the downloads of these chapters (after a mark-read with "remove after read").
    fn delete_downloads(&self, _manga_id: i64, _chapter_ids: &[i64]) {}
    /// Queues these chapters for download. With `start_now` the one chapter goes to the front and the queue starts.
    /// `false` when the downloads layer is not there, or refused: nothing was queued.
    fn enqueue_downloads(&self, _manga_id: i64, _chapter_ids: &[i64], _start_now: bool) -> bool {
        false
    }
    /// Takes one chapter out of the download queue.
    fn cancel_download(&self, _chapter_id: i64) {}
    /// Binds the enhanced trackers of the source to a manga that just joined the library (`bindEnhancedTrackers`).
    fn bind_enhanced(&self, _manga_id: i64) {}
    /// Whether a tracker is logged in: the tracking button opens the tracking sheet then, the tracking settings
    /// otherwise (E-25). The default is no; the app's backends read the published tracker session.
    fn has_logged_in_tracker(&self) -> bool {
        false
    }
    /// Whether the source is an HTTP source: only then does the page offer WebView and Share.
    fn is_http_source(&self, _source_id: i64) -> bool {
        false
    }
    /// Deletes the cover files of a manga that just left the library (`removeCovers`), after the write succeeded.
    /// `true` when a file was removed.
    fn remove_cover(&self, _manga_id: i64) -> bool {
        false
    }
}

/// The backend of an app with no source registry yet: every source reads as not installed, the local source reads
/// as installed and has nothing to fetch.
pub struct StubBackend {
    stubs: MangaStubSourceRepo,
}

impl StubBackend {
    pub fn new(db: Arc<Db>) -> Self {
        Self {
            stubs: MangaStubSourceRepo::new(db),
        }
    }
}

const LOCAL_SOURCE_ID: i64 = kuro_source::local::LOCAL_SOURCE_ID;

impl Backend for StubBackend {
    fn has_logged_in_tracker(&self) -> bool {
        crate::entry_core::tracking::session::has_logged_in()
    }
    fn source_info(&self, source_id: i64) -> SourceInfo {
        if source_id == LOCAL_SOURCE_ID {
            return SourceInfo {
                name: kuro_source::local::LOCAL_MANGA_NAME.to_owned(),
                stub: false,
            };
        }
        let stub = self
            .stubs
            .get(source_id)
            .ok()
            .flatten()
            .unwrap_or_else(|| StubSource::fallback(source_id));
        SourceInfo {
            name: stub.display_name(),
            stub: true,
        }
    }

    fn fetch_details(&self, manga_id: i64) -> Fetch {
        self.fetch(manga_id)
    }

    fn fetch_chapters(&self, manga_id: i64) -> Fetch {
        self.fetch(manga_id)
    }
}

impl StubBackend {
    fn fetch(&self, _manga_id: i64) -> Fetch {
        Box::pin(async { Err(DetailError::SourceNotInstalled) })
    }
}

/// Finds the live manga source of an id; `None` when it is not installed.
pub type MangaLookup = Arc<dyn Fn(i64) -> Option<Arc<dyn MangaSource>> + Send + Sync>;

/// The backend over the live manga sources (`awaitUpdateFromSource` and `syncChaptersWithSource`): the details are
/// written with the library refresh rules (`refresh_update`), the chapter list through the domain's chapter sync, both on
/// the row the Browse listing inserted with its source id. A source the lookup does not know reads as [`StubBackend`].
/// Source calls run under `isolate`, so a panicking source is an error on the page, not a crash.
pub struct SourceBackend {
    stub: StubBackend,
    db: Arc<Db>,
    lookup: MangaLookup,
    chapters: Arc<SyncChapters>,
    mark_duplicate_read: bool,
}

impl SourceBackend {
    pub fn new(db: Arc<Db>, lookup: MangaLookup, mark_duplicate_read: bool) -> Self {
        let chapters = Arc::new(SyncChapters::new(Arc::clone(&db), Arc::new(MangaSyncLocks::default()), Arc::new(NoRenames)));
        Self { stub: StubBackend::new(Arc::clone(&db)), db, lookup, chapters, mark_duplicate_read }
    }

    /// The app's backend: the sources the Browse directory hands out (the recipe sources and the installed packages).
    pub fn live(db: Arc<Db>, _prefs: &kuro_prefs::store::PrefStore) -> Self {
        Self::new(db, Arc::new(|id| crate::browse::directory::current()?.manga_source(id)), crate::misc::diet_defaults::LIBRARY_MARK_DUPLICATE_CHAPTER_READ)
    }
}

fn source_error(error: SourceError) -> DetailError {
    match error {
        SourceError::NotInstalled(_) => DetailError::SourceNotInstalled,
        SourceError::Http(code) => DetailError::Http(i64::from(code)),
        SourceError::UnknownHost { host } => DetailError::UnknownHost { online: true, host },
        SourceError::Cancelled => DetailError::Cancelled,
        other => DetailError::Other { class: "IOException".to_owned(), message: Some(other.to_string()) },
    }
}

fn db_error(error: impl std::fmt::Display) -> DetailError {
    DetailError::Other { class: "SQLiteException".to_owned(), message: Some(error.to_string()) }
}

/// The stored manga and its live source. The row is read on the blocking pool.
async fn target(db: Arc<Db>, lookup: MangaLookup, manga_id: i64) -> Result<(Manga, Arc<dyn MangaSource>), DetailError> {
    let manga = tokio::task::spawn_blocking(move || MangaRepo::new(db).by_id(manga_id)).await.map_err(db_error)?.map_err(db_error)?;
    let source = lookup(manga.source).ok_or(DetailError::SourceNotInstalled)?;
    Ok((manga, source))
}

impl Backend for SourceBackend {
    fn has_logged_in_tracker(&self) -> bool {
        self.stub.has_logged_in_tracker()
    }

    fn source_info(&self, source_id: i64) -> SourceInfo {
        match (self.lookup)(source_id) {
            Some(source) => SourceInfo { name: kuro_source::display_name(source.name(), source.lang()), stub: false },
            None => self.stub.source_info(source_id),
        }
    }

    fn fetch_details(&self, manga_id: i64) -> Fetch {
        let (db, lookup) = (Arc::clone(&self.db), Arc::clone(&self.lookup));
        Box::pin(async move {
            let (manga, source) = target(Arc::clone(&db), lookup, manga_id).await?;
            let remote = kuro_source::isolate(source.get_manga_details(&to_s_manga(&manga))).await.map_err(source_error)?;
            let update = refresh_update(manga.id, manga.favorite, &remote, manga.thumbnail_url.as_deref(), chrono::Utc::now().timestamp_millis());
            tokio::task::spawn_blocking(move || MangaRepo::new(db).update(&update)).await.map_err(db_error)?.map_err(db_error)
        })
    }

    fn fetch_chapters(&self, manga_id: i64) -> Fetch {
        let (db, lookup, sync, mark_duplicate_as_read) = (Arc::clone(&self.db), Arc::clone(&self.lookup), Arc::clone(&self.chapters), self.mark_duplicate_read);
        Box::pin(async move {
            let (manga, source) = target(db, lookup, manga_id).await?;
            let listed = to_s_manga(&manga);
            let raw = kuro_source::isolate(source.get_chapter_list(&listed)).await.map_err(source_error)?;
            let options = ChapterSyncOptions { now_ms: chrono::Utc::now().timestamp_millis(), manual: true, window: (0, 0), mark_duplicate_as_read, is_local: false };
            let prepare = |chapter: &mut SChapter| source.prepare_new_chapter(chapter, &listed);
            sync.run(raw, &manga, options, &prepare).await.map(drop).map_err(|error| match error {
                ChapterSyncError::NoChapters(_) => DetailError::NoChapters,
                ChapterSyncError::Db(error) => db_error(error),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_of(path: &std::path::Path) -> Arc<Db> {
        let writer = kuro_db::schema_manga::open(path).expect("schema");
        Arc::new(Db::new(path, writer).expect("db"))
    }

    /// The whole flow over a fake catalogue source, no network: the Browse listing inserts each entry as a non-favourite
    /// row of its source, opening one fetches the details and the chapter list into the database, Add to library makes it
    /// a favourite, and the library then lists it with its chapters. A failing source is an error, never a panic.
    #[test]
    fn browse_then_details_then_add_to_library_lists_the_entry_with_its_chapters() {
        use kuro_data::category::{CategoryKind, CategoryRepo};
        use kuro_data::chapter::ChapterRepo;
        use kuro_domain::source::browse::BrowseQuery;
        use kuro_source::filter_list::FilterList;

        use crate::browse::entries::FavoriteStore;
        use crate::browse::fake_catalogue::FakeCatalogue;
        use crate::browse::favorite::FavoriteWrite;
        use crate::browse::listing::PageLoader;

        let dir = tempfile::tempdir().unwrap();
        let db = db_of(&dir.path().join("manga.db"));
        let prefs = crate::browse::prefs::BrowsePrefs::new(kuro_prefs::store::PrefStore::in_memory());
        let store = Arc::new(crate::browse::repo_store::RepoStore::new(
            MangaRepo::new(Arc::clone(&db)),
            CategoryRepo::new(Arc::clone(&db), CategoryKind::Manga),
            prefs.default_manga_category.clone(),
            Arc::new(|_| 0),
        ));
        let fake = FakeCatalogue::new(41);
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();

        // browse: two cards, each a library row of source 41
        let loader = crate::recipe_sources::MangaLoader { source: fake.clone(), store: store.clone() };
        let page = rt.block_on(loader.load(BrowseQuery::Popular, FilterList::default(), 1)).expect("a page");
        assert_eq!(page.items.len(), 2);
        let id = page.items[0].id;
        let listed = MangaRepo::new(Arc::clone(&db)).by_id(id).unwrap();
        assert_eq!((listed.source, listed.url.as_str(), listed.favorite, listed.initialized), (41, "/m/one", false, false));

        // details: the page's backend fills the row and its chapters
        let source: Arc<dyn MangaSource> = fake.clone();
        let backend = SourceBackend::new(Arc::clone(&db), Arc::new(move |sid| (sid == 41).then(|| Arc::clone(&source))), false);
        assert_eq!(backend.source_info(41), SourceInfo { name: "Fake Central (EN)".into(), stub: false });
        rt.block_on(backend.fetch_details(id)).expect("details");
        rt.block_on(backend.fetch_chapters(id)).expect("chapters");
        let manga = MangaRepo::new(Arc::clone(&db)).by_id(id).unwrap();
        assert_eq!(manga.description.as_deref(), Some("A story."));
        assert_eq!(manga.genre, Some(vec!["Action".to_owned(), "Drama".to_owned()]));
        assert_eq!((manga.status, manga.initialized, manga.title.as_str()), (1, true, "Remote title"));
        assert_eq!(manga.author.as_deref(), Some("Author A"));
        let chapters = ChapterRepo::new(Arc::clone(&db)).by_manga_id(id, false).unwrap();
        let mut urls: Vec<_> = chapters.iter().map(|c| c.url.as_str()).collect();
        urls.sort_unstable();
        assert_eq!(urls, ["/m/one/c1", "/m/one/c2"]);

        // add to library, then the library lists it
        store.write_favorite(id, FavoriteWrite { favorite: true, date_added: 5 }).unwrap();
        let favorites = MangaRepo::new(Arc::clone(&db)).favorites().unwrap();
        assert_eq!(favorites.iter().map(|m| (m.id, m.source)).collect::<Vec<_>>(), [(id, 41)]);

        // a failing source and an unknown one are errors the page shows
        fake.fail.store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(rt.block_on(backend.fetch_details(id)), Err(DetailError::Http(503)));
        assert_eq!(rt.block_on(backend.fetch_chapters(id)), Err(DetailError::Http(503)));
        assert_eq!(ChapterRepo::new(Arc::clone(&db)).by_manga_id(id, false).unwrap().len(), 2, "a failed refresh keeps the chapters");
        let other = MangaRepo::new(Arc::clone(&db)).insert(&Manga { source: 99, url: "/x".into(), title: "X".into(), ..Manga::create() }).unwrap();
        assert_eq!(rt.block_on(backend.fetch_details(other)), Err(DetailError::SourceNotInstalled));
        assert!(backend.source_info(99).stub);
    }

    #[test]
    fn a_known_stub_shows_its_name_and_an_unknown_one_its_id() {
        let dir = tempfile::tempdir().unwrap();
        let db = db_of(&dir.path().join("manga.db"));
        MangaStubSourceRepo::new(Arc::clone(&db)).upsert(7, "en", "Reader").unwrap();
        let backend = StubBackend::new(db);
        assert_eq!(
            backend.source_info(7),
            SourceInfo {
                name: "Reader (EN)".into(),
                stub: true
            }
        );
        assert_eq!(backend.source_info(99).name, "99");
        assert!(backend.source_info(99).stub);
    }

    #[test]
    fn the_local_source_is_not_a_stub_and_fetches_report_not_installed() {
        let dir = tempfile::tempdir().unwrap();
        let backend = StubBackend::new(db_of(&dir.path().join("manga.db")));
        let local = backend.source_info(LOCAL_SOURCE_ID);
        assert!(!local.stub);
        assert!(!local.name.is_empty());
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        assert_eq!(rt.block_on(backend.fetch_details(1)), Err(DetailError::SourceNotInstalled));
    }
}
