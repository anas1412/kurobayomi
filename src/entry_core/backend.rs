//! What the page needs from outside its own database: the source name, the two fetches, the download states and
//! the download removal. The fetches persist their result (as `updateManga.awaitUpdateFromSource` does), so the
//! page sees the new rows through the store subscriptions. Nothing here imports GTK. `SourceBackend` is the one the app
//! uses; `StubBackend` answers for a source that is not installed.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use kuro_data::chapter::ChapterRepo;
use kuro_data::manga::{Manga, MangaRepo};
use kuro_data::source::{MangaStubSourceRepo, StubSource};
use kuro_db::handle::Db;
use kuro_domain::entries::source_convert::{refresh_update, to_s_manga};
use kuro_domain::items::chapter_sync::{ChapterSyncError, ChapterSyncOptions, NoRenames, SyncChapters};
use kuro_domain::items::source_sync::MangaSyncLocks;
use kuro_downloads::DownloadState;
use kuro_downloads::downloader::Command;
use kuro_downloads::engine::{MangaEngine, QueueEngine};
use kuro_downloads::manager::{DeleteRules, DownloadManager};
use kuro_downloads::model::{ChapterRef, SourceRef};
use kuro_downloads::source::FolderOnlySource;
use kuro_source::model::SChapter;
use kuro_source::MangaSource;

use crate::browse::errors::limited;
use crate::downloads::manga_io::{LiveChapterSource, chapter_ref, manga_ref};
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
/// Source calls run under `limited` (the panic guard and the 30 second limit), so a panicking or silent source is an error
/// on the page, not a crash or an endless refresh.
pub struct SourceBackend {
    stub: StubBackend,
    db: Arc<Db>,
    lookup: MangaLookup,
    chapters: Arc<SyncChapters>,
    mark_duplicate_read: bool,
    /// The manga download manager. `None` when the app runs no download managers: nothing can be queued then.
    downloads: Option<Arc<DownloadManager>>,
}

impl SourceBackend {
    pub fn new(db: Arc<Db>, lookup: MangaLookup, mark_duplicate_read: bool) -> Self {
        let chapters = Arc::new(SyncChapters::new(Arc::clone(&db), Arc::new(MangaSyncLocks::default()), Arc::new(NoRenames)));
        Self { stub: StubBackend::new(Arc::clone(&db)), db, lookup, chapters, mark_duplicate_read, downloads: None }
    }

    /// Gives the backend the manga download manager of the app.
    pub fn with_downloads(mut self, manager: Arc<DownloadManager>) -> Self {
        self.downloads = Some(manager);
        self
    }

    /// The app's backend: the sources the Browse directory hands out (the recipe sources and the installed packages),
    /// with the app's manga download manager when it runs.
    pub fn live(db: Arc<Db>, _prefs: &kuro_prefs::store::PrefStore) -> Self {
        let backend = Self::new(db, Arc::new(|id| crate::browse::directory::current(crate::browse::route::BrowseKind::Manga).catalogue(id)?.manga()), crate::misc::diet_defaults::LIBRARY_MARK_DUPLICATE_CHAPTER_READ);
        let manager = crate::shell::host::Shell::get().and_then(|shell| shell.download_queues()).map(|queues| queues.manga);
        match manager {
            Some(manager) => backend.with_downloads(manager),
            None => backend,
        }
    }

    /// The chapters of a manga that the ids name, and the manga, read from the database. `None` when the manga is gone.
    fn chapters_of(&self, manga_id: i64, ids: &[i64]) -> Option<(Manga, Vec<ChapterRef>)> {
        let manga = MangaRepo::new(Arc::clone(&self.db)).by_id(manga_id).ok()?;
        let chapters = ChapterRepo::new(Arc::clone(&self.db)).by_manga_id(manga_id, false).ok()?;
        let picked = chapters.iter().filter(|c| ids.contains(&c.id)).map(chapter_ref).collect();
        Some((manga, picked))
    }

    /// The live manga source of an id as a downloads source. `None` when the source is not installed.
    fn download_source(&self, source_id: i64) -> Option<SourceRef> {
        let source = (self.lookup)(source_id)?;
        Some(Arc::new(LiveChapterSource::new(source)))
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
            let remote = limited(source.get_manga_details(&to_s_manga(&manga))).await.map_err(|error| DetailError::from_source(source.name(), error))?;
            let update = refresh_update(manga.id, manga.favorite, &remote, manga.thumbnail_url.as_deref(), chrono::Utc::now().timestamp_millis());
            tokio::task::spawn_blocking(move || MangaRepo::new(db).update(&update)).await.map_err(db_error)?.map_err(db_error)
        })
    }

    fn fetch_chapters(&self, manga_id: i64) -> Fetch {
        let (db, lookup, sync, mark_duplicate_as_read) = (Arc::clone(&self.db), Arc::clone(&self.lookup), Arc::clone(&self.chapters), self.mark_duplicate_read);
        Box::pin(async move {
            let (manga, source) = target(db, lookup, manga_id).await?;
            let listed = to_s_manga(&manga);
            let raw = limited(source.get_chapter_list(&listed)).await.map_err(|error| DetailError::from_source(source.name(), error))?;
            let options = ChapterSyncOptions { now_ms: chrono::Utc::now().timestamp_millis(), manual: true, window: (0, 0), mark_duplicate_as_read, is_local: false };
            let prepare = |chapter: &mut SChapter| source.prepare_new_chapter(chapter, &listed);
            sync.run(raw, &manga, options, &prepare).await.map(drop).map_err(|error| match error {
                ChapterSyncError::NoChapters(_) => DetailError::NoChapters,
                ChapterSyncError::Db(error) => db_error(error),
            })
        })
    }

    /// The state of each chapter that is queued or on disk. A queued chapter shows its queue state; a finished one
    /// with a folder or archive in the index shows Downloaded. The index is read through the manager, so a manga with
    /// no manager has no states.
    fn downloads(&self, manga_id: i64) -> HashMap<i64, DownloadState> {
        let Some(manager) = &self.downloads else { return HashMap::new() };
        let Ok(manga) = MangaRepo::new(Arc::clone(&self.db)).by_id(manga_id) else { return HashMap::new() };
        let Ok(chapters) = ChapterRepo::new(Arc::clone(&self.db)).by_manga_id(manga_id, false) else { return HashMap::new() };
        let queued: HashMap<i64, DownloadState> = manager.snapshot().iter().map(|d| (d.chapter.id, d.status)).collect();
        chapters
            .iter()
            .filter_map(|chapter| {
                if let Some(state) = queued.get(&chapter.id) {
                    return Some((chapter.id, *state));
                }
                let on_disk = manager.index().is_chapter_downloaded(manga.source, &manga.title, &chapter.name, chapter.scanlator.as_deref());
                on_disk.then_some((chapter.id, DownloadState::Downloaded))
            })
            .collect()
    }

    /// Removes the downloads of these chapters: they leave the queue, then the folders and the index entries go. The
    /// deletion runs on the runtime. A source that is not installed is named by the folder the download index found for
    /// the manga, so its downloads can always be deleted.
    fn delete_downloads(&self, manga_id: i64, chapter_ids: &[i64]) {
        let Some(manager) = self.downloads.clone() else { return };
        let Some((manga, chapters)) = self.chapters_of(manga_id, chapter_ids) else { return };
        let source: SourceRef = match self.download_source(manga.source) {
            Some(source) => source,
            None => match manager.index().manga_source_dir(manga.source, &manga.title) {
                Some(folder) => Arc::new(FolderOnlySource { id: manga.source, folder }),
                None => return, // nothing of this manga is on disk
            },
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else { return };
        // The user picked these chapters to delete, so the bookmark and category rules do not keep any back
        let rules = DeleteRules { remove_bookmarked: true, ..DeleteRules::default() };
        let manga = manga_ref(&manga);
        handle.spawn(async move {
            if let Err(error) = manager.delete_chapters(chapters, &manga, source.as_ref(), &rules).await {
                tracing::error!(%error, "the downloads were not deleted");
            }
        });
    }

    /// Queues the chapters: all of them, or with `start_now` the first one at the front, which starts the run. `false`
    /// when there is no manager, the source is not installed, or none of the chapters exists.
    fn enqueue_downloads(&self, manga_id: i64, chapter_ids: &[i64], start_now: bool) -> bool {
        let Some(manager) = &self.downloads else { return false };
        let ids = if start_now { &chapter_ids[..chapter_ids.len().min(1)] } else { chapter_ids };
        let Some((manga, chapters)) = self.chapters_of(manga_id, ids) else { return false };
        if chapters.is_empty() {
            return false;
        }
        let Some(source) = self.download_source(manga.source) else { return false };
        let manga = manga_ref(&manga);
        let command = if start_now {
            let (reply, _answer) = tokio::sync::oneshot::channel();
            Command::EnqueueFront { source, manga, chapters, start: true, reply }
        } else {
            let (reply, _answer) = tokio::sync::oneshot::channel();
            Command::Enqueue { source, manga, chapters, auto_start: true, reply }
        };
        manager.post(command).is_ok()
    }

    /// Takes one chapter out of the queue. Its downloaded files, if any, are not touched.
    fn cancel_download(&self, chapter_id: i64) {
        let Some(manager) = &self.downloads else { return };
        MangaEngine::new(Arc::clone(manager)).remove(&[chapter_id]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_of(path: &std::path::Path) -> Arc<Db> {
        let writer = kuro_db::schema_unified::open_file(path).expect("schema");
        Arc::new(Db::new(path, writer).expect("db"))
    }

    /// The whole flow over a fake catalogue source, no network: the Browse listing inserts each entry as a non-favourite
    /// row of its source, opening one fetches the details and the chapter list into the database, Add to library makes it
    /// a favourite, and the library then lists it with its chapters. A failing source is an error, never a panic.
    #[test]
    fn browse_then_details_then_add_to_library_lists_the_entry_with_its_chapters() {
        use kuro_data::category::CategoryRepo;
        use kuro_platform::kind::Kind;
        use kuro_data::chapter::ChapterRepo;
        use kuro_domain::source::browse::BrowseQuery;
        use kuro_source::filter_list::FilterList;

        use crate::browse::entries::FavoriteStore;
        use crate::browse::fake_catalogue::FakeCatalogue;
        use crate::browse::favorite::FavoriteWrite;
        use crate::browse::listing::PageLoader;

        let dir = tempfile::tempdir().unwrap();
        let db = db_of(&dir.path().join("test.db"));
        let prefs = crate::browse::prefs::BrowsePrefs::new(kuro_prefs::store::PrefStore::in_memory(), crate::browse::route::BrowseKind::Manga);
        let store = Arc::new(crate::browse::repo_store::RepoStore::new(
            Kind::Manga,
            kuro_data::entry::EntryRepo::new(Arc::clone(&db)),
            CategoryRepo::new(Arc::clone(&db), Kind::Manga),
            prefs.default_category.clone(),
            Arc::new(|_| 0),
            Arc::new(|_| 0),
        ));
        let fake = FakeCatalogue::new(41);
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();

        // browse: two cards, each a library row of source 41
        let loader = crate::browse::loader::SourceLoader::new(crate::browse::loader::Catalogue::Manga(fake.clone()), store.clone());
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
        for failed in [rt.block_on(backend.fetch_details(id)), rt.block_on(backend.fetch_chapters(id))] {
            let text = failed.expect_err("the source is down").user_message();
            assert!(text.starts_with("Fake Central ") && text.contains("isn't working right now") && !text.contains("503"), "{text}");
        }
        assert_eq!(ChapterRepo::new(Arc::clone(&db)).by_manga_id(id, false).unwrap().len(), 2, "a failed refresh keeps the chapters");
        let other = MangaRepo::new(Arc::clone(&db)).insert(&Manga { source: 99, url: "/x".into(), title: "X".into(), ..Manga::create() }).unwrap();
        assert_eq!(rt.block_on(backend.fetch_details(other)), Err(DetailError::SourceNotInstalled));
        assert!(backend.source_info(99).stub);
    }

    #[test]
    fn a_known_stub_shows_its_name_and_an_unknown_one_its_id() {
        let dir = tempfile::tempdir().unwrap();
        let db = db_of(&dir.path().join("test.db"));
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
        let backend = StubBackend::new(db_of(&dir.path().join("test.db")));
        let local = backend.source_info(LOCAL_SOURCE_ID);
        assert!(!local.stub);
        assert!(!local.name.is_empty());
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        assert_eq!(rt.block_on(backend.fetch_details(1)), Err(DetailError::SourceNotInstalled));
    }

    /// Polls until `done` holds, for up to two seconds: the queue changes on the manager's own task.
    fn wait_for(done: impl Fn() -> bool) {
        for _ in 0..400 {
            if done() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the download queue did not settle");
    }

    /// Queueing, cancelling and deleting go through the manager. A queued chapter shows its queue state, leaves the
    /// queue when cancelled, and a delete takes it out of the queue too. The connectivity is offline in this test, so
    /// nothing downloads. Without a manager nothing can be queued and no state is shown.
    #[test]
    fn queued_chapters_show_their_state_and_leave_the_queue_on_cancel_and_delete() {
        use crate::browse::fake_catalogue::FakeCatalogue;

        let dir = tempfile::tempdir().unwrap();
        let db = db_of(&dir.path().join("test.db"));
        let fake = FakeCatalogue::new(41);
        let source: Arc<dyn MangaSource> = fake.clone();
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let (manager, _connectivity) = rt.block_on(async { crate::downloads::manga_io::test_manager(dir.path()) });
        let plain = SourceBackend::new(Arc::clone(&db), Arc::new(move |sid| (sid == 41).then(|| Arc::clone(&source))), false);
        let source: Arc<dyn MangaSource> = fake.clone();
        let backend = SourceBackend::new(Arc::clone(&db), Arc::new(move |sid| (sid == 41).then(|| Arc::clone(&source))), false).with_downloads(manager);

        let id = MangaRepo::new(Arc::clone(&db)).insert(&Manga { source: 41, url: "/m/one".into(), title: "Remote title".into(), ..Manga::create() }).unwrap();
        rt.block_on(backend.fetch_chapters(id)).expect("chapters");
        let ids: Vec<i64> = ChapterRepo::new(Arc::clone(&db)).by_manga_id(id, false).unwrap().iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), 2);
        assert!(backend.downloads(id).is_empty(), "nothing is queued or on disk yet");
        assert!(!backend.enqueue_downloads(id, &[-1], false), "an unknown chapter queues nothing");

        assert!(backend.enqueue_downloads(id, &ids[..1], false));
        wait_for(|| backend.downloads(id).get(&ids[0]) == Some(&DownloadState::Queue));
        assert_eq!(backend.downloads(id).len(), 1);

        backend.cancel_download(ids[0]);
        wait_for(|| backend.downloads(id).is_empty());

        assert!(backend.enqueue_downloads(id, &ids, false));
        wait_for(|| backend.downloads(id).len() == 2);
        rt.block_on(async { backend.delete_downloads(id, &ids[..1]) });
        wait_for(|| backend.downloads(id).len() == 1);
        assert!(backend.downloads(id).contains_key(&ids[1]), "the chapter that was not picked stays queued");

        assert!(!plain.enqueue_downloads(id, &ids, false), "no manager, nothing queued");
        assert!(plain.downloads(id).is_empty());
    }
}
