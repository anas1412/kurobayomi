//! What the entry page needs from a manga source outside its own database: the source name, the two fetches, the
//! download states and the download removal (`EntryBackend` for manga). The fetches persist their result (as
//! `updateManga.awaitUpdateFromSource` does), so the page sees the new rows through the store subscriptions. Nothing here
//! imports GTK. A source the lookup does not know reads as not installed.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use kuro_data::entry::{Entry, EntryRepo};
use kuro_data::item::{Item, ItemRepo};
use kuro_data::source::{StubSource, StubSourceRepo};
use kuro_db::handle::Db;
use kuro_domain::entries::source_convert::to_s_manga;
use kuro_domain::entries::update::{CacheFiles, UpdateEntry, remote_from_manga};
use kuro_domain::items::source_sync::EntrySyncLocks;
use kuro_domain::items::sync::{NoRenames, SourceItem, SyncItems, SyncItemsError, SyncOptions, source_item_from_chapter};
use kuro_downloads::DownloadState;
use kuro_platform::kind::Kind;
use kuro_source::model::{SChapter, SManga};
use kuro_source::MangaSource;

use crate::browse::errors::limited;
use crate::entry_core::source_info::SourceInfo;
use crate::entry_core::details::entry_state::RefreshPlan;
use crate::entry_core::details::runner::{EntryBackend, EntryFetch};
use crate::entry_core::refresh_rules::Trigger;
use crate::entry_core::downloaded::download_states;
use crate::entry_core::errors::DetailError;
use crate::updates::download_host::{self, DownloadBackend};

type Fetch = Pin<Box<dyn Future<Output = Result<(), DetailError>> + Send>>;

/// What a chapter fetch hands back: the chapters the sync added, which a manual refresh queues for download (T-REF-4).
pub type FetchItems = Pin<Box<dyn Future<Output = Result<Vec<Item>, DetailError>> + Send>>;

const LOCAL_SOURCE_ID: i64 = kuro_source::local::LOCAL_SOURCE_ID;

/// The source line of a source the registry does not know, for either kind: the local source reads as installed under its
/// own name, any other as a stub named by its stored row (or by its id when no row is stored).
pub fn stub_source_info(stubs: &StubSourceRepo, kind: Kind, source_id: i64) -> SourceInfo {
    if source_id == LOCAL_SOURCE_ID {
        let name = match kind {
            Kind::Manga => kuro_source::local::LOCAL_MANGA_NAME,
            Kind::Anime => kuro_source::local::LOCAL_ANIME_NAME,
        };
        return SourceInfo { name: name.to_owned(), stub: false };
    }
    let stub = stubs.get(source_id).ok().flatten().unwrap_or_else(|| StubSource::fallback(source_id));
    SourceInfo { name: stub.display_name(), stub: true }
}

/// Finds the live manga source of an id; `None` when it is not installed.
pub type MangaLookup = Arc<dyn Fn(i64) -> Option<Arc<dyn MangaSource>> + Send + Sync>;

/// No cover cache is wired: an entry never has a custom cover file, and no file is deleted. The cover rule
/// (`image_action`) still decides and stamps the timestamp, so a changed cover reloads.
pub struct NoCoverFiles;

impl CacheFiles for NoCoverFiles {
    fn has_custom_cover(&self, _e: &Entry) -> bool {
        false
    }
    fn has_custom_background(&self, _e: &Entry) -> bool {
        false
    }
    fn delete_cover(&self, _e: &Entry) {}
    fn delete_background(&self, _e: &Entry) {}
}

/// The source's `prepareNewChapter` hook as the item sync calls it: on the source shape of one item.
fn prepare_hook<'a>(source: &'a dyn MangaSource, listed: &'a SManga) -> impl Fn(&mut SourceItem) + Sync + 'a {
    move |item: &mut SourceItem| {
        let mut chapter = SChapter {
            url: item.url.clone(),
            name: item.name.clone(),
            date_upload: item.date_upload,
            chapter_number: item.number,
            scanlator: item.scanlator.clone(),
        };
        source.prepare_new_chapter(&mut chapter, listed);
        item.url = chapter.url;
        item.name = chapter.name;
        item.date_upload = chapter.date_upload;
        item.number = chapter.chapter_number;
        item.scanlator = chapter.scanlator;
    }
}

/// The backend over the live manga sources (`awaitUpdateFromSource` and `syncChaptersWithSource`): the details are
/// written with the shared refresh rules (`UpdateEntry::update_from_source`), the chapter list through the domain's item
/// sync, both on the row the Browse listing inserted with its source id. A source the lookup does not know reads as not installed.
/// Source calls run under `limited` (the panic guard and the 30 second limit), so a panicking or silent source is an error
/// on the page, not a crash or an endless refresh.
pub struct SourceBackend {
    stubs: StubSourceRepo,
    db: Arc<Db>,
    lookup: MangaLookup,
    chapters: SyncItems,
    mark_duplicate_read: bool,
    /// The one download service as the screens see it. `None` (the plain `new`): nothing is queued or shown.
    downloads: Option<Arc<dyn DownloadBackend>>,
}

impl SourceBackend {
    pub fn new(db: Arc<Db>, lookup: MangaLookup, mark_duplicate_read: bool) -> Self {
        let chapters = SyncItems::new(Arc::clone(&db), Arc::new(EntrySyncLocks::default()), Arc::new(NoRenames));
        Self { stubs: StubSourceRepo::new(Arc::clone(&db), Kind::Manga), db, lookup, chapters, mark_duplicate_read, downloads: None }
    }

    /// Gives the backend the download service of the app.
    pub fn with_downloads(mut self, downloads: Arc<dyn DownloadBackend>) -> Self {
        self.downloads = Some(downloads);
        self
    }

    /// The app's backend: the sources the Browse directory hands out (the recipe sources and the installed packages),
    /// with the app's download service.
    pub fn live(db: Arc<Db>, _prefs: &kuro_prefs::store::PrefStore) -> Self {
        let backend = Self::new(db, Arc::new(|id| crate::browse::directory::current(crate::browse::route::BrowseKind::Manga).catalogue(id)?.manga()), crate::misc::diet_defaults::LIBRARY_MARK_DUPLICATE_CHAPTER_READ);
        backend.with_downloads(download_host::current())
    }
}

fn db_error(error: impl std::fmt::Display) -> DetailError {
    DetailError::Other { class: "SQLiteException".to_owned(), message: Some(error.to_string()) }
}

/// The stored manga and its live source. The row is read on the blocking pool.
async fn target(db: Arc<Db>, lookup: MangaLookup, manga_id: i64) -> Result<(Entry, Arc<dyn MangaSource>), DetailError> {
    let manga = tokio::task::spawn_blocking(move || EntryRepo::new(db).by_id(manga_id)).await.map_err(db_error)?.map_err(db_error)?;
    let source = lookup(manga.source).ok_or(DetailError::SourceNotInstalled)?;
    Ok((manga, source))
}

impl SourceBackend {
    fn source_info(&self, source_id: i64) -> SourceInfo {
        match (self.lookup)(source_id) {
            Some(source) => SourceInfo { name: kuro_source::display_name(source.name(), source.lang()), stub: false },
            None => stub_source_info(&self.stubs, Kind::Manga, source_id),
        }
    }

    fn fetch_details(&self, manga_id: i64) -> Fetch {
        let (db, lookup) = (Arc::clone(&self.db), Arc::clone(&self.lookup));
        Box::pin(async move {
            let (manga, source) = target(Arc::clone(&db), lookup, manga_id).await?;
            let remote = limited(source.get_manga_details(&to_s_manga(&manga))).await.map_err(|error| DetailError::from_source(source.name(), error))?;
            let remote = remote_from_manga(&remote);
            let update = UpdateEntry::new(EntryRepo::new(Arc::clone(&db)), ItemRepo::new(db));
            tokio::task::spawn_blocking(move || update.update_from_source(&manga, &remote, false, &NoCoverFiles, &chrono::Local::now()))
                .await
                .map_err(db_error)?
                .map_err(db_error)
        })
    }

    pub fn fetch_chapters(&self, manga_id: i64) -> FetchItems {
        let (db, lookup, sync, mark_duplicate_done) = (Arc::clone(&self.db), Arc::clone(&self.lookup), self.chapters.clone(), self.mark_duplicate_read);
        Box::pin(async move {
            let (manga, source) = target(db, lookup, manga_id).await?;
            let listed = to_s_manga(&manga);
            let raw = limited(source.get_chapter_list(&listed)).await.map_err(|error| DetailError::from_source(source.name(), error))?;
            let options = SyncOptions { now_ms: chrono::Utc::now().timestamp_millis(), manual: true, window: (0, 0), mark_duplicate_done, is_local: false };
            let items: Vec<SourceItem> = raw.iter().map(source_item_from_chapter).collect();
            let prepare = prepare_hook(source.as_ref(), &listed);
            sync.run(items, &manga, options, Some(&prepare)).await.map_err(|error| match error {
                SyncItemsError::NoItems => DetailError::NoChapters,
                SyncItemsError::Db(error) => db_error(error),
            })
        })
    }

    /// The state of each chapter that is queued or on disk: a queued chapter shows its queue state, a finished one in the
    /// index shows Downloaded. A backend with no download service has no states.
    fn downloads(&self, manga_id: i64) -> HashMap<i64, DownloadState> {
        let Some(host) = &self.downloads else { return HashMap::new() };
        let Ok(manga) = EntryRepo::new(Arc::clone(&self.db)).by_id(manga_id) else { return HashMap::new() };
        let Ok(chapters) = ItemRepo::new(Arc::clone(&self.db)).by_entry_id(manga_id, false) else { return HashMap::new() };
        download_states(host.as_ref(), Kind::Manga, &manga, &chapters)
    }

    /// Queues the chapters and starts the queue: all of them, or with `start_now` the first one at the front. `false`
    /// when there is no download service or none of the chapters could be queued.
    fn enqueue_downloads(&self, manga_id: i64, chapter_ids: &[i64], start_now: bool) -> bool {
        let Some(host) = &self.downloads else { return false };
        if start_now {
            let Some(first) = chapter_ids.first() else { return false };
            host.start_now(Kind::Manga, *first);
            return true;
        }
        let queued = host.enqueue(Kind::Manga, manga_id, chapter_ids);
        if queued {
            host.start_queue(Kind::Manga);
        }
        queued
    }
}

/// The entry page of either kind asks for what `plan` names and gets the chapters the sync added.
impl EntryBackend for SourceBackend {
    fn source_info(&self, source_id: i64) -> SourceInfo {
        SourceBackend::source_info(self, source_id)
    }

    fn fetch(&self, entry_id: i64, plan: RefreshPlan, _trigger: Trigger) -> EntryFetch {
        let details = plan.info.then(|| self.fetch_details(entry_id));
        let chapters = plan.episodes.then(|| self.fetch_chapters(entry_id));
        Box::pin(async move {
            let (details, chapters) = futures::join!(async { Some(details?.await) }, async { Some(chapters?.await) });
            // a details failure that the page ignores (HTTP 103) does not stop the chapters
            if let Some(Err(error)) = details {
                if !error.is_ignored() {
                    return Err(error);
                }
            }
            chapters.unwrap_or(Ok(Vec::new()))
        })
    }

    fn enqueue_new_items(&self, entry_id: i64, items: &[Item]) {
        let ids: Vec<i64> = items.iter().map(|item| item.id).collect();
        self.enqueue_downloads(entry_id, &ids, false);
    }

    fn bind_enhanced(&self, entry_id: i64) {
        crate::updates::track_binding::bind_entry(&self.db, Kind::Manga, entry_id);
    }

    fn remove_cover(&self, entry_id: i64) -> bool {
        crate::library::mount::clear_cached(&self.db, entry_id, kuro_images::art::ArtKind::Cover) > 0
    }

    fn downloaded_count(&self, entry_id: i64) -> usize {
        self.downloaded_items(entry_id).len()
    }

    fn delete_downloads(&self, entry_id: i64) {
        let Ok(items) = ItemRepo::new(Arc::clone(&self.db)).by_entry_id(entry_id, false) else { return };
        let ids: Vec<i64> = items.iter().map(|item| item.id).collect();
        self.delete_items(entry_id, &ids);
    }

    fn has_download_handler(&self) -> bool {
        self.downloads.is_some()
    }

    fn downloaded_items(&self, entry_id: i64) -> std::collections::HashSet<i64> {
        self.downloads(entry_id).into_iter().filter(|(_, state)| *state == DownloadState::Downloaded).map(|(id, _)| id).collect()
    }

    fn enqueue_items(&self, entry_id: i64, item_ids: &[i64], start_now: bool) -> bool {
        self.enqueue_downloads(entry_id, item_ids, start_now)
    }

    fn delete_items(&self, entry_id: i64, item_ids: &[i64]) {
        let Some(host) = &self.downloads else { return };
        host.delete(Kind::Manga, entry_id, item_ids);
    }

    fn has_logged_in_tracker(&self) -> bool {
        crate::entry_core::tracking::session::has_logged_in()
    }

    fn is_configurable_source(&self, source_id: i64) -> bool {
        (self.lookup)(source_id).is_some_and(|source| source.is_configurable())
    }

    fn source_language(&self, source_id: i64) -> String {
        (self.lookup)(source_id).map(|source| source.lang().to_owned()).unwrap_or_default()
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
        let listed = EntryRepo::new(Arc::clone(&db)).by_id(id).unwrap();
        assert_eq!((listed.source, listed.url.as_str(), listed.favorite, listed.initialized), (41, "/m/one", false, false));

        // details: the page's backend fills the row and its chapters
        let source: Arc<dyn MangaSource> = fake.clone();
        let backend = SourceBackend::new(Arc::clone(&db), Arc::new(move |sid| (sid == 41).then(|| Arc::clone(&source))), false);
        assert_eq!(backend.source_info(41), SourceInfo { name: "Fake Central (EN)".into(), stub: false });
        rt.block_on(backend.fetch_details(id)).expect("details");
        rt.block_on(backend.fetch_chapters(id)).expect("chapters");
        let manga = EntryRepo::new(Arc::clone(&db)).by_id(id).unwrap();
        assert_eq!(manga.description.as_deref(), Some("A story."));
        assert_eq!(manga.genre, Some(vec!["Action".to_owned(), "Drama".to_owned()]));
        assert_eq!((manga.status, manga.initialized, manga.title.as_str()), (1, true, "Remote title"));
        assert_eq!(manga.author.as_deref(), Some("Author A"));
        let chapters = ItemRepo::new(Arc::clone(&db)).by_entry_id(id, false).unwrap();
        let mut urls: Vec<_> = chapters.iter().map(|c| c.url.as_str()).collect();
        urls.sort_unstable();
        assert_eq!(urls, ["/m/one/c1", "/m/one/c2"]);

        // add to library, then the library lists it
        store.write_favorite(id, FavoriteWrite { favorite: true, date_added: 5 }).unwrap();
        let favorites = EntryRepo::new(Arc::clone(&db)).favorites(Kind::Manga).unwrap();
        assert_eq!(favorites.iter().map(|m| (m.id, m.source)).collect::<Vec<_>>(), [(id, 41)]);

        // a failing source and an unknown one are errors the page shows
        fake.fail.store(true, std::sync::atomic::Ordering::SeqCst);
        for failed in [rt.block_on(backend.fetch_details(id)), rt.block_on(backend.fetch_chapters(id)).map(drop)] {
            let text = failed.expect_err("the source is down").user_message();
            assert!(text.starts_with("Fake Central ") && text.contains("isn't working right now") && !text.contains("503"), "{text}");
        }
        assert_eq!(ItemRepo::new(Arc::clone(&db)).by_entry_id(id, false).unwrap().len(), 2, "a failed refresh keeps the chapters");
        let other = EntryRepo::new(Arc::clone(&db)).insert(&Entry { source: 99, url: "/x".into(), title: "X".into(), ..Entry::create(Kind::Manga) }).unwrap();
        assert_eq!(rt.block_on(backend.fetch_details(other)), Err(DetailError::SourceNotInstalled));
        assert!(backend.source_info(99).stub);
    }

    #[test]
    fn a_known_stub_shows_its_name_and_an_unknown_one_its_id() {
        let dir = tempfile::tempdir().unwrap();
        let db = db_of(&dir.path().join("test.db"));
        StubSourceRepo::new(Arc::clone(&db), Kind::Manga).upsert(7, "en", "Reader").unwrap();
        let backend = SourceBackend::new(db, Arc::new(|_| None), false);
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
    fn the_local_source_is_not_a_stub() {
        let dir = tempfile::tempdir().unwrap();
        let backend = SourceBackend::new(db_of(&dir.path().join("test.db")), Arc::new(|_| None), false);
        let local = backend.source_info(LOCAL_SOURCE_ID);
        assert!(!local.stub);
        assert!(!local.name.is_empty());
    }

    /// Queueing, cancelling and deleting go through the one download service: a queued chapter shows its queue state, a
    /// chapter in the index shows Downloaded, and without a service nothing is queued and no state is shown.
    #[test]
    fn downloads_go_through_the_download_service() {
        use crate::updates::download_host::fake::{Call, Fake};

        let dir = tempfile::tempdir().unwrap();
        let db = db_of(&dir.path().join("test.db"));
        let id = EntryRepo::new(Arc::clone(&db)).insert(&Entry { source: 41, url: "/m/one".into(), title: "Remote title".into(), ..Entry::create(Kind::Manga) }).unwrap();
        let chapters: Vec<_> = (1..=2).map(|n| kuro_data::item::Item { entry_id: id, url: format!("/c{n}"), name: format!("c{n}"), ..kuro_data::item::Item::create(Kind::Manga) }).collect();
        let ids: Vec<i64> = ItemRepo::new(Arc::clone(&db)).add_all(&chapters).unwrap().iter().map(|c| c.id).collect();

        let fake = Arc::new(Fake::installed());
        let backend = SourceBackend::new(Arc::clone(&db), Arc::new(|_| None), false).with_downloads(fake.clone());
        assert!(backend.downloads(id).is_empty());
        fake.queue(ids[0], DownloadState::Downloading, 40);
        fake.disk.lock().unwrap().push(ids[1]);
        let states = backend.downloads(id);
        assert_eq!((states.get(&ids[0]), states.get(&ids[1])), (Some(&DownloadState::Downloading), Some(&DownloadState::Downloaded)));

        assert!(backend.enqueue_downloads(id, &ids, false));
        assert!(backend.enqueue_downloads(id, &ids, true));
        EntryBackend::delete_items(&backend, id, &ids[..1]);
        assert_eq!(
            fake.calls(),
            [
                Call::Enqueue { entry_id: id, unit_ids: ids.clone() },
                Call::StartQueue,
                Call::StartNow(ids[0]),
                Call::Delete { entry_id: id, unit_ids: vec![ids[0]] },
            ]
        );

        let plain = SourceBackend::new(Arc::clone(&db), Arc::new(|_| None), false);
        assert!(!plain.enqueue_downloads(id, &ids, false), "no service, nothing queued");
        assert!(plain.downloads(id).is_empty());
    }
}
