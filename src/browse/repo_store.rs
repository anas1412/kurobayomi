//! The `FavoriteStore` of each kind over its repositories: the production side of `entries`. Every write names the
//! columns it sets (`MangaUpdate` and `AnimeUpdate` keep the rest), so flags written by the default-flags step and the
//! stored title and status survive a stale listed snapshot (B-EC-12, E-BR-02). These are the kind-specific database
//! adapters; the browse pages only see the trait.

use std::collections::BTreeSet;
use std::sync::Arc;

use kuro_data::LookupOrInsert;
use kuro_data::anime::{Anime, AnimeRepo, AnimeUpdate};
use kuro_data::category::CategoryRepo;
use kuro_data::manga::{Manga, MangaRepo, MangaUpdate};
use kuro_domain::entries::defaults::apply_default_episode_flags;
use kuro_domain::entries::network_to_local::network_to_local_manga;
use kuro_local::ids::is_local_source;
use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;
use tokio::sync::watch;

use super::entries::{CategoryChoice, FavoriteStore, LocalEntry, TrackerBinder};
use super::favorite::FavoriteWrite;

/// Deletes the cached cover files of an entry and returns how many went (the cover cache is the image layer's).
pub type CoverClearer = Arc<dyn Fn(i64) -> usize + Send + Sync>;

pub struct RepoStore {
    manga: MangaRepo,
    categories: CategoryRepo,
    default_category: Pref<i32>,
    clear_cover: CoverClearer,
}

impl RepoStore {
    pub fn new(manga: MangaRepo, categories: CategoryRepo, default_category: Pref<i32>, clear_cover: CoverClearer) -> Self {
        Self { manga, categories, default_category, clear_cover }
    }
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn urls(list: &[Manga]) -> BTreeSet<String> {
    list.iter().map(|m| m.url.clone()).collect()
}

impl FavoriteStore for RepoStore {
    fn local_entry(&self, source_id: i64, url: &str, title: &str, cover: Option<&str>) -> Result<LocalEntry, String> {
        let listed = Manga {
            source: source_id,
            url: url.to_owned(),
            title: title.to_owned(),
            thumbnail_url: cover.map(str::to_owned),
            ..Manga::create()
        };
        let local = network_to_local_manga(&self.manga, &listed).map_err(text)?;
        Ok(LocalEntry { id: local.id, favorite: local.favorite, title: local.title })
    }

    fn duplicates(&self, id: i64, title: &str) -> Result<Vec<i64>, String> {
        Ok(self.manga.duplicates(id, title).map_err(text)?.into_iter().map(|m| m.id).collect())
    }

    fn categories(&self) -> Result<Vec<CategoryChoice>, String> {
        let all = self.categories.all().map_err(text)?;
        Ok(all.into_iter().filter(|c| c.id != 0).map(|c| CategoryChoice { id: c.id, name: c.name }).collect())
    }

    fn default_category(&self) -> i64 {
        i64::from(self.default_category.get())
    }

    fn write_favorite(&self, id: i64, write: FavoriteWrite) -> Result<(), String> {
        let update = MangaUpdate { favorite: Some(write.favorite), date_added: Some(write.date_added), ..MangaUpdate::new(id) };
        self.manga.update(&update).map_err(text)
    }

    fn set_categories(&self, id: i64, ids: &[i64]) -> Result<(), String> {
        self.manga.set_categories(id, ids).map_err(text)
    }

    fn entry_categories(&self, id: i64) -> Result<Vec<i64>, String> {
        Ok(self.categories.by_entry(id).map_err(text)?.into_iter().filter(|c| c.id != 0).map(|c| c.id).collect())
    }

    fn clear_cover(&self, id: i64) -> usize {
        (self.clear_cover)(id)
    }

    fn stamp_cover(&self, id: i64, now_ms: i64) -> Result<(), String> {
        let update = MangaUpdate { cover_last_modified: Some(now_ms), ..MangaUpdate::new(id) };
        self.manga.update(&update).map_err(text)
    }

    /// Needs a tokio runtime: the follower task that republishes the subscription runs on it.
    fn favorite_urls(&self, source_id: i64) -> watch::Receiver<BTreeSet<String>> {
        let mut subscription = self.manga.subscribe_favorites_by_source(source_id);
        let initial = subscription.current().and_then(|p| p.ok()).map(|list| urls(&list)).unwrap_or_default();
        let (tx, rx) = watch::channel(initial);
        tokio::spawn(async move {
            while let Some(published) = subscription.changed().await {
                let Ok(list) = published else { continue };
                if tx.send(urls(&list)).is_err() {
                    break;
                }
            }
        });
        rx
    }
}

/// The anime store. The local source keeps its files: removing one of its entries deletes no cover and no background
/// (B§4.8.1).
pub struct AnimeStore {
    repo: AnimeRepo,
    categories: CategoryRepo,
    default_category: Pref<i32>,
    clear_cover: CoverClearer,
    clear_background: CoverClearer,
    /// The preferences the default episode flags come from. `None` skips the defaults.
    defaults: Option<PrefStore>,
    /// Binds the enhanced trackers of an added entry. `None` binds nothing.
    binder: Option<TrackerBinder>,
}

impl AnimeStore {
    pub fn new(
        repo: AnimeRepo,
        categories: CategoryRepo,
        default_category: Pref<i32>,
        clear_cover: CoverClearer,
        clear_background: CoverClearer,
    ) -> Self {
        Self { repo, categories, default_category, clear_cover, clear_background, defaults: None, binder: None }
    }

    /// Turns on what an add does besides the favourite: the default episode flags from `defaults`, and the enhanced
    /// tracker bindings through `binder`.
    pub fn with_add_effects(mut self, defaults: PrefStore, binder: TrackerBinder) -> Self {
        self.defaults = Some(defaults);
        self.binder = Some(binder);
        self
    }

    /// Runs one of the file clearers, unless the entry belongs to the local source (its files are not a cache).
    fn clear_files(&self, id: i64, clear: &CoverClearer) -> usize {
        match self.repo.by_id(id) {
            Ok(anime) if !is_local_source(anime.source) => clear(id),
            Ok(_) => 0,
            Err(error) => {
                tracing::warn!(entry = id, %error, "the files of an entry could not be looked up; none were deleted");
                0
            }
        }
    }
}

fn anime_urls(list: &[Anime]) -> BTreeSet<String> {
    list.iter().map(|a| a.url.clone()).collect()
}

impl FavoriteStore for AnimeStore {
    /// A listed anime is found by `(url, source)`, or inserted as a non-favourite row with the `Anime::create()` defaults
    /// (`NetworkToLocalAnime`). An existing row, favourite or not, shows its stored title: the listed title is kept for a
    /// new row only (Q-1 default, E-BR-10). Each entry is its own write transaction (`AnimeRepo` has no batch upsert).
    fn local_entry(&self, source_id: i64, url: &str, title: &str, cover: Option<&str>) -> Result<LocalEntry, String> {
        let listed = Anime { source: source_id, url: url.to_owned(), title: title.to_owned(), thumbnail_url: cover.map(str::to_owned), ..Anime::create() };
        Ok(match self.repo.lookup_or_insert(&listed).map_err(text)? {
            LookupOrInsert::Inserted(id) => LocalEntry { id, favorite: false, title: listed.title },
            LookupOrInsert::Found(stored) => LocalEntry { id: stored.id, favorite: stored.favorite, title: stored.title },
        })
    }

    fn duplicates(&self, id: i64, title: &str) -> Result<Vec<i64>, String> {
        Ok(self.repo.duplicates(id, title).map_err(text)?.into_iter().map(|a| a.id).collect())
    }

    fn categories(&self) -> Result<Vec<CategoryChoice>, String> {
        let all = self.categories.all().map_err(text)?;
        Ok(all.into_iter().filter(|c| c.id != 0).map(|c| CategoryChoice { id: c.id, name: c.name }).collect())
    }

    fn default_category(&self) -> i64 {
        i64::from(self.default_category.get())
    }

    fn write_favorite(&self, id: i64, write: FavoriteWrite) -> Result<(), String> {
        let update = AnimeUpdate { favorite: Some(write.favorite), date_added: Some(write.date_added), ..AnimeUpdate::new(id) };
        self.repo.update(&update).map_err(text)
    }

    fn set_categories(&self, id: i64, ids: &[i64]) -> Result<(), String> {
        self.repo.set_categories(id, ids).map_err(text)
    }

    fn clear_cover(&self, id: i64) -> usize {
        self.clear_files(id, &self.clear_cover)
    }

    fn stamp_cover(&self, id: i64, now_ms: i64) -> Result<(), String> {
        let update = AnimeUpdate { cover_last_modified: Some(now_ms), ..AnimeUpdate::new(id) };
        self.repo.update(&update).map_err(text)
    }

    fn clear_background(&self, id: i64) -> usize {
        self.clear_files(id, &self.clear_background)
    }

    fn stamp_background(&self, id: i64, now_ms: i64) -> Result<(), String> {
        let update = AnimeUpdate { background_last_modified: Some(now_ms), ..AnimeUpdate::new(id) };
        self.repo.update(&update).map_err(text)
    }

    fn after_add(&self, id: i64) -> Result<(), String> {
        let anime = self.repo.by_id(id).map_err(text)?;
        let flags = match &self.defaults {
            Some(prefs) => apply_default_episode_flags(&self.repo, prefs, &anime).map(|_| ()).map_err(text),
            None => Ok(()),
        };
        // the trackers are asked even when the flags failed: they do not depend on each other
        if let Some(bind) = &self.binder {
            bind(anime.id, &anime.title, anime.source);
        }
        flags
    }

    /// Needs a tokio runtime: the follower task that republishes the subscription runs on it.
    fn favorite_urls(&self, source_id: i64) -> watch::Receiver<BTreeSet<String>> {
        let mut subscription = self.repo.subscribe_favorites_by_source(source_id);
        let initial = subscription.current().and_then(|p| p.ok()).map(|list| anime_urls(&list)).unwrap_or_default();
        let (tx, rx) = watch::channel(initial);
        tokio::spawn(async move {
            while let Some(published) = subscription.changed().await {
                let Ok(list) = published else { continue };
                if tx.send(anime_urls(&list)).is_err() {
                    break;
                }
            }
        });
        rx
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use kuro_db::handle::Db;
    use kuro_prefs::store::PrefStore;

    use super::*;
    use crate::browse::entries::{add_entry, remove_entry};

    fn store(deleted: Arc<AtomicUsize>) -> (tempfile::TempDir, RepoStore, MangaRepo) {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("tachiyomi.db");
        let writer = kuro_db::schema_manga::open(&path).expect("schema");
        let db = Arc::new(Db::new(&path, writer).expect("db"));
        let manga = MangaRepo::new(Arc::clone(&db));
        let categories = CategoryRepo::new(db, kuro_data::category::CategoryKind::Manga);
        let prefs = PrefStore::in_memory().pref("default_category", 0_i32);
        let clearer: CoverClearer = Arc::new(move |_| deleted.load(Ordering::SeqCst));
        (dir, RepoStore::new(manga.clone(), categories, prefs, clearer), manga)
    }

    #[test]
    fn an_unknown_entry_is_inserted_as_a_non_favourite_and_found_again() {
        let (_dir, store, manga) = store(Arc::default());
        let first = store.local_entry(5, "/series/1", "One", Some("http://c/1.png")).expect("entry");
        assert!(!first.favorite);
        let again = store.local_entry(5, "/series/1", "Renamed", None).expect("entry");
        assert_eq!(again.id, first.id, "the same (url, source) is one row");
        assert_eq!(manga.by_id(first.id).expect("row").thumbnail_url.as_deref(), Some("http://c/1.png"));
    }

    #[test]
    fn add_and_remove_write_only_their_columns_and_stamp_the_cover_when_files_went() {
        let deleted = Arc::new(AtomicUsize::new(0));
        let (_dir, store, manga) = store(Arc::clone(&deleted));
        let entry = store.local_entry(5, "/series/1", "One", None).expect("entry");
        manga.set_chapter_flags_masked(entry.id, 0b1010, 0b1111).expect("flags");

        add_entry(&store, entry.id, 4_000);
        let added = manga.by_id(entry.id).expect("row");
        assert!(added.favorite);
        assert_eq!(added.date_added, 4_000);
        assert_eq!(added.chapter_flags & 0b1111, 0b1010, "the flags written earlier survive");

        deleted.store(1, Ordering::SeqCst);
        remove_entry(&store, entry.id, 9_000).expect("removed");
        let removed = manga.by_id(entry.id).expect("row");
        assert!(!removed.favorite);
        assert_eq!(removed.date_added, 0);
        assert_eq!(removed.cover_last_modified, 9_000, "a deleted cover file stamps the row");
        assert_eq!(removed.chapter_flags & 0b1111, 0b1010);
    }

    #[test]
    fn the_entry_categories_are_the_user_category_links_of_the_row() {
        let (_dir, store, _manga) = store(Arc::default());
        let entry = store.local_entry(5, "/series/1", "One", None).expect("entry");
        let reading = store.categories.insert(&kuro_data::category::Category { id: 0, name: "Reading".into(), order: 1, flags: 0, hidden: false }).expect("category");
        assert_eq!(store.entry_categories(entry.id), Ok(Vec::new()), "a new entry has no links");
        store.set_categories(entry.id, &[reading]).expect("links");
        assert_eq!(store.entry_categories(entry.id), Ok(vec![reading]));
        let rows = crate::browse::entries::chooser_rows(&store.categories().expect("list"), &store.entry_categories(entry.id).expect("links"));
        assert_eq!(rows.iter().map(|(c, on)| (c.id, *on)).collect::<Vec<_>>(), [(reading, true)]);
    }

    #[test]
    fn the_favourite_urls_of_a_source_follow_the_library() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime");
        let _entered = rt.enter();
        let (_dir, store, _manga) = store(Arc::default());
        let entry = store.local_entry(5, "/series/1", "One", None).expect("entry");
        let mut urls = store.favorite_urls(5);
        assert!(urls.borrow().is_empty());
        add_entry(&store, entry.id, 1);
        let seen = rt.block_on(async { tokio::time::timeout(Duration::from_secs(3), urls.wait_for(|set| set.contains("/series/1"))).await });
        assert!(matches!(seen, Ok(Ok(_))), "the added entry reaches the follower");
    }
}

#[cfg(test)]
mod anime_tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use kuro_data::category::{CategoryKind, create_anime_category};
    use kuro_db::handle::Db;
    use kuro_domain::category::target::AddTarget;
    use kuro_prefs::store::PrefStore;

    use super::*;
    use crate::browse::entries::{add_entry, remove_entry};
    use crate::browse::favorite::{AddOutcome, toggle_write};

    struct Rig {
        _dir: tempfile::TempDir,
        store: AnimeStore,
        repo: AnimeRepo,
        categories: CategoryRepo,
        db: Arc<Db>,
        default: Pref<i32>,
    }

    fn rig(cover_files: Arc<AtomicUsize>, background_files: Arc<AtomicUsize>) -> Rig {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("animetachiyomi.db");
        let writer = kuro_db::schema_anime::open(&path).expect("schema");
        let db = Arc::new(Db::new(&path, writer).expect("db"));
        let repo = AnimeRepo::new(Arc::clone(&db));
        let categories = CategoryRepo::new(Arc::clone(&db), CategoryKind::Anime);
        let default = PrefStore::in_memory().pref("default_anime_category", -1_i32);
        let clear_cover: CoverClearer = Arc::new(move |_| cover_files.load(Ordering::SeqCst));
        let clear_background: CoverClearer = Arc::new(move |_| background_files.load(Ordering::SeqCst));
        let store = AnimeStore::new(repo.clone(), categories.clone(), default.clone(), clear_cover, clear_background);
        Rig { _dir: dir, store, repo, categories, db, default }
    }

    fn entry(rig: &Rig, source: i64, url: &str, title: &str) -> i64 {
        rig.store.local_entry(source, url, title, None).expect("entry").id
    }

    #[test]
    fn adding_writes_the_favourite_and_its_date_and_nothing_else() {
        let rig = rig(Arc::default(), Arc::default());
        let id = entry(&rig, 5, "/show/1", "Stored title");
        rig.repo.update(&AnimeUpdate { status: Some(2), ..AnimeUpdate::new(id) }).expect("status");

        rig.store.write_favorite(id, toggle_write(false, 4_000)).expect("favourite");

        let row = rig.repo.by_id(id).expect("row");
        assert!(row.favorite);
        assert_eq!(row.date_added, 4_000);
        assert_eq!(row.title, "Stored title", "a listed title does not overwrite the stored one (E-BR-02)");
        assert_eq!(row.status, 2, "the stored status survives the write");
    }

    #[test]
    fn a_listed_entry_that_exists_keeps_its_stored_title_and_favourite_flag() {
        let rig = rig(Arc::default(), Arc::default());
        let id = entry(&rig, 5, "/show/1", "Stored");
        rig.store.write_favorite(id, toggle_write(false, 1)).expect("favourite");

        let again = rig.store.local_entry(5, "/show/1", "Renamed upstream", None).expect("entry");
        assert_eq!(again.id, id);
        assert!(again.favorite);
        assert_eq!(again.title, "Stored");
    }

    #[test]
    fn removing_stamps_the_cover_and_background_only_when_files_went() {
        let covers = Arc::new(AtomicUsize::new(0));
        let backgrounds = Arc::new(AtomicUsize::new(1));
        let rig = rig(Arc::clone(&covers), Arc::clone(&backgrounds));
        let id = entry(&rig, 5, "/show/1", "One");
        add_entry(&rig.store, id, 4_000);

        remove_entry(&rig.store, id, 9_000).expect("removed");

        let row = rig.repo.by_id(id).expect("row");
        assert!(!row.favorite);
        assert_eq!(row.date_added, 0);
        assert_eq!(row.cover_last_modified, 0, "no cover file went, so no stamp");
        assert_eq!(row.background_last_modified, 9_000, "a deleted background file stamps the row");
    }

    #[test]
    fn the_local_source_deletes_no_files_on_remove() {
        let covers = Arc::new(AtomicUsize::new(3));
        let rig = rig(Arc::clone(&covers), Arc::default());
        let id = entry(&rig, 0, "/local/1", "Local one");
        add_entry(&rig.store, id, 4_000);

        remove_entry(&rig.store, id, 9_000).expect("removed");

        assert_eq!(rig.repo.by_id(id).expect("row").cover_last_modified, 0, "the local source's files are not a cache");
    }

    #[test]
    fn an_unset_default_category_asks_and_a_set_one_is_written_before_the_favourite() {
        let rig = rig(Arc::default(), Arc::default());
        let action = create_anime_category(&rig.db, "Action", 0).expect("category");
        let id = entry(&rig, 5, "/show/1", "One");

        assert_eq!(add_entry(&rig.store, id, 1), AddOutcome::Chooser, "default -1 opens the chooser");
        assert!(!rig.repo.by_id(id).expect("row").favorite, "the chooser writes nothing");

        rig.default.set(action as i32).expect("default");
        assert_eq!(
            add_entry(&rig.store, id, 2),
            AddOutcome::Added { category_error: None },
            "the default category is written, then the favourite"
        );
        let links: Vec<i64> = rig.categories.by_entry(id).expect("links").into_iter().map(|c| c.id).collect();
        assert_eq!(links, vec![action]);
        assert!(rig.repo.by_id(id).expect("row").favorite);
    }

    #[test]
    fn a_duplicate_is_another_entry_with_the_same_title_ignoring_case() {
        let rig = rig(Arc::default(), Arc::default());
        let first = entry(&rig, 5, "/show/1", "Same Show");
        let second = entry(&rig, 6, "/other/1", "same show");
        rig.store.write_favorite(first, toggle_write(false, 1)).expect("favourite");

        assert_eq!(rig.store.duplicates(second, "same show").expect("duplicates"), vec![first]);
        assert!(rig.store.duplicates(first, "Same Show").expect("duplicates").is_empty(), "an entry is not its own duplicate");
    }

    #[test]
    fn the_favourite_urls_of_a_source_follow_the_library() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime");
        let _entered = rt.enter();
        let rig = rig(Arc::default(), Arc::default());
        let id = entry(&rig, 5, "/show/1", "One");
        let mut urls = rig.store.favorite_urls(5);
        assert!(urls.borrow().is_empty());

        rig.store.write_favorite(id, toggle_write(false, 1)).expect("favourite");

        let seen = rt.block_on(async {
            tokio::time::timeout(std::time::Duration::from_secs(3), urls.wait_for(|set| set.contains("/show/1"))).await
        });
        assert!(matches!(seen, Ok(Ok(_))), "the added entry reaches the follower");
    }

    #[test]
    fn an_add_with_the_default_target_writes_no_categories_and_then_the_favourite() {
        // AddTarget::Categories(vec![]) is the Default category: the link set is emptied, then the favourite is written.
        let rig = rig(Arc::default(), Arc::default());
        let id = entry(&rig, 5, "/show/1", "One");
        let outcome = crate::browse::favorite::add(
            &AddTarget::Categories(vec![]),
            7,
            |ids| rig.store.set_categories(id, ids),
            |write| rig.store.write_favorite(id, write),
        );
        assert_eq!(outcome, AddOutcome::Added { category_error: None });
        assert!(rig.repo.by_id(id).expect("row").favorite);
    }

    #[test]
    fn an_add_applies_the_default_episode_flags_and_binds_the_enhanced_trackers() {
        let rig = rig(Arc::default(), Arc::default());
        let prefs = PrefStore::in_memory();
        let word = kuro_domain::entries::defaults::episode_default_word(&prefs);
        let bound: Arc<std::sync::Mutex<Vec<(i64, String, i64)>>> = Arc::default();
        let log = Arc::clone(&bound);
        let binder: TrackerBinder = Arc::new(move |id, title, source| log.lock().expect("lock").push((id, title.to_owned(), source)));
        let store = AnimeStore { defaults: Some(prefs), binder: Some(binder), ..rig.store };
        let id = store.local_entry(5, "/show/1", "One", None).expect("entry").id;
        rig.repo.update_episode_flags(id, |_| word ^ 0b1000).expect("scrambled flags");
        assert_ne!(rig.repo.by_id(id).expect("row").episode_flags, word);

        rig.default.set(0).expect("default category");
        assert_eq!(add_entry(&store, id, 4_000), AddOutcome::Added { category_error: None });

        let row = rig.repo.by_id(id).expect("row");
        assert!(row.favorite);
        assert_eq!(row.date_added, 4_000);
        assert_eq!(row.episode_flags, word, "the default episode flags are written");
        assert_eq!(*bound.lock().expect("lock"), [(id, "One".to_owned(), 5)], "the trackers are asked once, with the source id");
    }
}
