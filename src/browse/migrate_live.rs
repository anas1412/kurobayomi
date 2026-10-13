//! The app side of the migration engine for both kinds (T-40a-016, T-40a-041, T-41a-034): the steps of `MigrateSteps` over
//! the kind's database, its source directory, the one download service and the custom image files. The new entry's
//! chapters or episodes are fetched and stored by the same backend its entry page refreshes with. The database steps
//! share one transaction, so a failed step rolls all of them back; the file steps run after it commits.
//!
//! Downloads: both kinds read the one download index for Delete downloaded and delete the old entry's folder and its
//! queued units through the one download service.

use std::sync::Arc;

use kuro_data::entry::EntryRepo;
use kuro_data::item::Item;
use kuro_db::tables::{EntryPatch, ItemPatch, TrackRow, categories, entries, items, tracks};
use kuro_db::{Db, DbError, Transaction};
use kuro_downloads::service::Downloads;
use kuro_images::cover_paths::{Family, custom_file};
use kuro_platform::linux::LinuxPaths;
use kuro_prefs::store::PrefStore;
use kuro_source::anime::model::FetchType;
use tokio::runtime::Handle;

use crate::downloads::directory::AppDirectory;
use super::directory::SourceDirectory;
use super::migrate_engine::{CustomImage, Entry, MigrateSteps, MigrateTx, StepFuture, TxWork};
use super::migrate_merge::{item_updates, merge_item_state};
use kuro_platform::kind::Kind;
use crate::entry_core::details::entry_state::RefreshPlan;
use crate::entry_core::refresh_rules::Trigger;
use crate::entry_core::details::remote::{LiveSources, RemoteAnimeBackend};
use crate::entry_core::details::runner::EntryBackend;
use crate::entry_core::backend::SourceBackend;

pub struct LiveSteps {
    kind: Kind,
    db: Arc<Db>,
    directory: Arc<dyn SourceDirectory>,
    /// The one download service; `None` when the app runs none.
    downloads: Option<Downloads>,
    prefs: PrefStore,
}

impl LiveSteps {
    /// The fetch runs on the app runtime: the one entered on the calling thread, or the shell's.
    pub fn new(kind: Kind, db: Arc<Db>, directory: Arc<dyn SourceDirectory>, downloads: Option<Downloads>, prefs: PrefStore) -> Self {
        Self { kind, db, directory, downloads, prefs }
    }

    /// The library entry of `id` as the engine reads it. `None` when it is not in the library.
    pub fn entry(&self, id: i64) -> Option<Entry> {
        let entry = EntryRepo::new(Arc::clone(&self.db)).by_id(id).ok().filter(|entry| entry.kind == self.kind)?;
        Some(Entry {
            id: entry.id,
            source: entry.source,
            title: entry.title,
            item_flags: entry.item_flags,
            viewer_flags: entry.viewer_flags,
            date_added: entry.date_added,
        })
    }

    /// How the entry lists its items, for the dialog's rules: a manga, like an anime that is not a seasons title, lists
    /// episodes (chapters). The library stores the fetch type as the database's enum; the rules take the source API's.
    pub fn fetch_type(&self, id: i64) -> FetchType {
        EntryRepo::new(Arc::clone(&self.db)).by_id(id).map_or(FetchType::Episodes, |entry| FetchType::from(entry.fetch_type() as i64))
    }

    /// Whether the old entry has downloaded chapters or episodes on disk (the dialog offers Delete downloaded only then).
    pub fn has_downloads(&self, entry: &Entry) -> bool {
        self.downloads.as_ref().is_some_and(|downloads| downloads.index().unit_count_entry(self.kind, entry.source, &entry.title) > 0)
    }

    /// The image files of a custom image of this kind; a manga has no background.
    fn family(&self, image: CustomImage) -> Option<Family> {
        match (self.kind, image) {
            (Kind::Manga, CustomImage::Cover) => Some(Family::MangaCover),
            (Kind::Manga, CustomImage::Background) => None,
            (Kind::Anime, CustomImage::Cover) => Some(Family::AnimeCover),
            (Kind::Anime, CustomImage::Background) => Some(Family::AnimeBackground),
        }
    }
}

/// The app runtime: the one entered on this thread, or the shell's.
fn runtime() -> Option<Handle> {
    Handle::try_current().ok().or_else(|| crate::shell::host::Shell::get().map(|shell| shell.runtime().clone()))
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

struct LiveTx<'a, 'c> {
    kind: Kind,
    tx: &'a Transaction<'c>,
}

impl MigrateTx for LiveTx<'_, '_> {
    fn copy_item_state(&mut self, old: i64, new: i64) -> Result<(), String> {
        let tx = self.tx;
        let read = |id: i64| items::by_entry(tx, id, false).map(|rows| rows.into_iter().map(Item::from).collect::<Vec<Item>>()).map_err(text);
        let (prev, current) = (read(old)?, read(new)?);
        let updates = item_updates(&current, &merge_item_state(&prev, &current));
        updates.iter().try_for_each(|update| items::update(tx, &ItemPatch::from(update)).map(drop)).map_err(text)
    }

    fn category_ids(&mut self, id: i64) -> Result<Vec<i64>, String> {
        categories::by_entry(self.tx, self.kind, id).map(|rows| rows.into_iter().map(|row| row.id).collect()).map_err(text)
    }

    /// Plain statements inside this transaction: the repository's `set_categories` opens a transaction of its own, which
    /// cannot nest inside this one.
    fn set_categories(&mut self, id: i64, ids: &[i64]) -> Result<(), String> {
        entries::set_categories(self.tx, id, ids).map_err(text)
    }

    /// Each binding becomes a new row (`id` -1) of the new entry; the old one keeps its own.
    fn move_tracks(&mut self, old: i64, new: i64) -> Result<(), String> {
        let rows = tracks::by_entry(self.tx, old).map_err(text)?;
        rows.into_iter().try_for_each(|row| tracks::insert(self.tx, &TrackRow { id: -1, entry_id: new, ..row }).map(drop)).map_err(text)
    }

    fn set_favorite(&mut self, id: i64, favorite: bool, date_added: i64, flags: Option<(i64, i64)>) -> Result<(), String> {
        let patch = EntryPatch {
            id,
            favorite: Some(favorite),
            date_added: Some(date_added),
            item_flags: flags.map(|(items, _)| items),
            viewer: flags.map(|(_, viewer)| viewer),
            ..EntryPatch::default()
        };
        entries::update(self.tx, &patch).map(drop).map_err(text)
    }
}

impl MigrateSteps for LiveSteps {
    fn source_present(&self, source_id: i64) -> bool {
        self.directory.catalogue(source_id).is_some()
    }

    /// The chapters or episodes of the new entry, fetched and stored by its entry page's backend on the app runtime.
    fn fetch_items(&self, new: &Entry) -> StepFuture<Result<(), String>> {
        let Some(handle) = runtime() else {
            return Box::pin(async { Err("the app runtime is not running".to_owned()) });
        };
        match self.kind {
            Kind::Manga => {
                let directory = Arc::clone(&self.directory);
                let lookup = Arc::new(move |id: i64| directory.catalogue(id)?.manga());
                let backend = SourceBackend::new(Arc::clone(&self.db), lookup, crate::misc::diet_defaults::LIBRARY_MARK_DUPLICATE_CHAPTER_READ);
                let task = backend.fetch_chapters(new.id);
                Box::pin(async move {
                    match handle.spawn(task).await {
                        Ok(Ok(_)) => Ok(()),
                        Ok(Err(error)) => Err(error.user_message()),
                        Err(error) => Err(error.to_string()),
                    }
                })
            }
            Kind::Anime => {
                let Some(source) = self.directory.catalogue(new.source).and_then(|catalogue| catalogue.anime()) else {
                    return Box::pin(async { Err("the source of the new entry is not installed".to_owned()) });
                };
                let backend = RemoteAnimeBackend::new(Arc::clone(&self.db), &self.prefs, Arc::new(LiveSources::new(vec![source])), handle.clone());
                let task = backend.fetch(new.id, RefreshPlan { info: false, episodes: true, seasons: false }, Trigger::Manual);
                Box::pin(async move {
                    match handle.spawn(task).await {
                        Ok(Ok(_)) => Ok(()),
                        Ok(Err(error)) => Err(error.user_message()),
                        Err(error) => Err(error.to_string()),
                    }
                })
            }
        }
    }

    fn transact(&self, work: TxWork<'_>) -> Result<(), String> {
        self.db
            .write(|tx| work(&mut LiveTx { kind: self.kind, tx }).map_err(DbError::Constraint))
            .map_err(|error| match error {
                DbError::Constraint(message) => message,
                other => other.to_string(),
            })
    }

    fn now_ms(&self) -> i64 {
        super::browse_screen::now_ms()
    }

    /// Removes the old entry's downloads, queued units included, through the download service. An entry or a source the
    /// app no longer knows has nothing to delete.
    fn delete_downloads(&self, old: &Entry) -> Result<(), String> {
        let Some(downloads) = &self.downloads else { return Ok(()) };
        let Some((entry, source)) = AppDirectory::new(Arc::clone(&self.db)).entry_context(self.kind, old.id) else { return Ok(()) };
        let work = downloads.delete_entry(&entry, &source, true);
        // the engine runs on the main context; inside a runtime the wait must not stall its worker
        let done = match Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(work)),
            Err(_) => runtime().ok_or("no runtime to delete the downloads on")?.block_on(work),
        };
        done.map_err(text)
    }

    fn has_custom(&self, image: CustomImage, old: &Entry) -> bool {
        self.family(image).is_some_and(|family| LinuxPaths::system().is_ok_and(|paths| custom_file(&paths, family, old.id).is_file()))
    }

    fn copy_custom(&self, image: CustomImage, old: &Entry, new: &Entry) -> Result<(), String> {
        let Some(family) = self.family(image) else { return Ok(()) };
        let paths = LinuxPaths::system().map_err(text)?;
        if kuro_images::cover_file::copy_custom(&paths, family, old.id, new.id) { Ok(()) } else { Err(format!("the custom {image:?} could not be copied")) }
    }
}

#[cfg(test)]
mod tests {
    use kuro_data::category::CategoryRepo;
    use kuro_data::entry::Entry as Stored;
    use kuro_data::item::ItemRepo;
    use kuro_data::track::{Track, TrackRepo};

    use super::*;
    use crate::browse::directory::{Catalogues, DirectorySource};
    use crate::browse::fake_catalogue::{FakeAnime, FakeCatalogue};
    use crate::browse::loader::Catalogue;
    use crate::browse::migrate_engine::{MigrateError, MigratePlan, migrate};
    use crate::browse::migrate_flags::{CATEGORIES, ITEMS};
    use kuro_domain::source::model::Source;

    const KINDS: [Kind; 2] = [Kind::Manga, Kind::Anime];

    fn runtime() -> &'static tokio::runtime::Runtime {
        Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().expect("runtime")))
    }

    fn db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("library.db");
        let writer = kuro_db::schema_unified::open_file(&path).expect("schema");
        (dir, Arc::new(Db::new(&path, writer).expect("db")))
    }

    /// A directory whose sources 1 and 2 are runnable.
    fn directory(sources: Vec<Catalogue>) -> Arc<dyn SourceDirectory> {
        let rows = sources
            .into_iter()
            .map(|source| {
                let row = DirectorySource { source: Source::new(source.id(), "S", "en"), package: String::new(), home_url: None };
                (source, row)
            })
            .collect();
        Arc::new(Catalogues::with_store(rows, Arc::new(|| None)))
    }

    fn add(repo: &EntryRepo, kind: Kind, source: i64, url: &str, favorite: bool) -> i64 {
        repo.insert(&Stored { source, url: url.into(), title: format!("t{url}"), favorite, item_flags: 5, viewer_flags: 6, date_added: 42, ..Stored::create(kind) }).expect("insert")
    }

    /// The old and the new entry of a migration: sources 1 and 2, the old one in the library.
    fn pair(db: &Arc<Db>, kind: Kind) -> (i64, i64) {
        let repo = EntryRepo::new(Arc::clone(db));
        (add(&repo, kind, 1, "/old", true), add(&repo, kind, 2, "/new", false))
    }

    #[test]
    fn a_manga_migration_moves_the_chapter_state_the_categories_the_tracks_and_the_favourite_in_one_go() {
        let rt = runtime();
        let _entered = rt.enter();
        let (_dir, db) = db();
        let repo = EntryRepo::new(Arc::clone(&db));
        let (old_id, new_id) = pair(&db, Kind::Manga);
        let items = ItemRepo::new(Arc::clone(&db));
        items.add_all(&[Item { entry_id: old_id, url: "/old1".into(), name: "Chapter 1".into(), number: 1.0, done: true, bookmark: true, date_fetch: 9, ..Item::create(Kind::Manga) }]).expect("old chapters");
        TrackRepo::new(Arc::clone(&db)).insert(&Track { entry_id: old_id, remote_id: 77, title: "Tracked".into(), ..Track::new(Kind::Manga, 1) }).expect("track");
        let categories = CategoryRepo::new(Arc::clone(&db), Kind::Manga);
        let category = kuro_testkit::insert_category(&db, Kind::Manga, "Reading");
        repo.set_categories(old_id, &[category]).expect("categories");

        let steps = LiveSteps::new(Kind::Manga, Arc::clone(&db), directory(vec![Catalogue::Manga(FakeCatalogue::new(1)), Catalogue::Manga(FakeCatalogue::new(2))]), None, PrefStore::in_memory());
        let (old, new) = (steps.entry(old_id).expect("old"), steps.entry(new_id).expect("new"));
        let flags = (ITEMS | CATEGORIES) as i32;
        let outcome = rt.block_on(migrate(&steps, &old, &new, MigratePlan { replace: true, flags })).expect("migrates");
        assert!(outcome.warnings.is_empty());

        let moved = items.by_entry_id(new_id, false).expect("new chapters");
        let first = moved.iter().find(|c| c.number == 1.0).expect("chapter 1");
        assert!(first.done && first.bookmark && first.date_fetch == 9, "the old state carried over: {first:?}");
        assert_eq!(TrackRepo::new(Arc::clone(&db)).by_entry(new_id).expect("tracks").len(), 1, "the track follows");
        assert_eq!(categories.by_entry(new_id).expect("categories").len(), 1);
        let (old_after, new_after) = (repo.by_id(old_id).expect("old"), repo.by_id(new_id).expect("new"));
        assert!(!old_after.favorite && new_after.favorite, "Migrate swaps the favourite");
        assert_eq!((new_after.item_flags, new_after.viewer_flags, new_after.date_added), (5, 6, 42), "flags copied, date added from the old manga");
    }

    #[test]
    fn a_failed_fetch_of_either_kind_stops_before_any_write() {
        let rt = runtime();
        let _entered = rt.enter();
        // the manga source answers HTTP 503; the anime source lists no episodes
        let failing = FakeCatalogue::new(2);
        failing.fail.store(true, std::sync::atomic::Ordering::SeqCst);
        let kinds = [(Kind::Manga, Catalogue::Manga(failing)), (Kind::Anime, Catalogue::Anime(FakeAnime::new(2, Vec::new())))];
        for (kind, source) in kinds {
            let (_dir, db) = db();
            let (old_id, new_id) = pair(&db, kind);
            let steps = LiveSteps::new(kind, Arc::clone(&db), directory(vec![source]), None, PrefStore::in_memory());
            let (old, new) = (steps.entry(old_id).expect("old"), steps.entry(new_id).expect("new"));
            let result = rt.block_on(migrate(&steps, &old, &new, MigratePlan { replace: true, flags: i32::MAX }));
            assert!(matches!(result, Err(MigrateError::FetchItems(_))), "{kind:?}: {result:?}");
            let (old_after, new_after) = (steps.entry(old_id).expect("old"), steps.entry(new_id).expect("new"));
            assert_eq!((old_after.date_added, new_after.date_added), (42, 42), "{kind:?}: nothing was written");
        }
    }

    #[test]
    fn with_no_installed_source_the_new_source_is_missing() {
        let rt = runtime();
        let _entered = rt.enter();
        let (_dir, db) = db();
        let (old_id, new_id) = pair(&db, Kind::Manga);
        let steps = LiveSteps::new(Kind::Manga, db, directory(Vec::new()), None, PrefStore::in_memory());
        let (old, new) = (steps.entry(old_id).expect("old"), steps.entry(new_id).expect("new"));
        assert_eq!(rt.block_on(migrate(&steps, &old, &new, MigratePlan { replace: false, flags: 0 })), Err(MigrateError::NewSourceMissing));
    }

    /// The transaction itself: every database step through the trait, committed together, for both kinds.
    #[test]
    fn the_database_steps_move_items_categories_and_tracks_and_flip_the_favourites_in_one_transaction() {
        for kind in KINDS {
            let (_dir, db) = db();
            let (repo, categories, tracks, items) = (EntryRepo::new(Arc::clone(&db)), CategoryRepo::new(Arc::clone(&db), kind), TrackRepo::new(Arc::clone(&db)), ItemRepo::new(Arc::clone(&db)));
            let category = kuro_testkit::insert_category(&db, kind, "Watching");
            let (old_id, new_id) = pair(&db, kind);
            repo.set_categories(old_id, &[category]).expect("link");
            tracks.insert(&Track { entry_id: old_id, remote_id: 99, title: "t".into(), ..Track::new(kind, 7) }).expect("track");
            let item = |entry_id: i64, url: &str| Item { entry_id, url: url.into(), number: 1.0, ..Item::create(kind) };
            items.add_all(&[Item { done: true, bookmark: true, date_fetch: 500, ..item(old_id, "/o1") }, item(new_id, "/n1")]).expect("items");
            let steps = LiveSteps::new(kind, Arc::clone(&db), directory(Vec::new()), None, PrefStore::in_memory());
            steps
                .transact(&mut |tx| {
                    tx.copy_item_state(old_id, new_id)?;
                    let ids = tx.category_ids(old_id)?;
                    tx.set_categories(new_id, &ids)?;
                    tx.move_tracks(old_id, new_id)?;
                    tx.set_favorite(new_id, true, 42, Some((5, 6)))?;
                    tx.set_favorite(old_id, false, 0, None)
                })
                .expect("commit");
            assert_eq!(categories.by_entry(new_id).expect("categories").into_iter().map(|c| c.id).collect::<Vec<_>>(), [category], "{kind}");
            assert_eq!(tracks.by_entry(new_id).expect("tracks").len(), 1, "{kind}: the binding moved");
            let moved = items.by_entry_id(new_id, false).expect("items");
            assert_eq!((moved[0].done, moved[0].bookmark, moved[0].date_fetch), (true, true, 500), "{kind}: the old state carried over");
            let (old_row, new_row) = (repo.by_id(old_id).expect("old"), repo.by_id(new_id).expect("new"));
            assert!(!old_row.favorite && new_row.favorite);
            assert_eq!((new_row.item_flags, new_row.viewer_flags, new_row.date_added), (5, 6, 42));
        }
    }

    #[test]
    fn a_failing_step_rolls_every_earlier_write_back() {
        for kind in KINDS {
            let (_dir, db) = db();
            let steps = LiveSteps::new(kind, Arc::clone(&db), directory(Vec::new()), None, PrefStore::in_memory());
            let (old_id, new_id) = pair(&db, kind);
            let result = steps.transact(&mut |tx| {
                tx.set_favorite(new_id, true, 1, None)?;
                tx.set_favorite(old_id, false, 0, None)?;
                Err("disk full".to_owned())
            });
            assert_eq!(result, Err("disk full".to_owned()));
            let (old, new) = (steps.entry(old_id).expect("old"), steps.entry(new_id).expect("new"));
            assert_eq!((old.date_added, new.date_added), (42, 42), "{kind:?}: both flips rolled back");
        }
    }

    /// T-40b-085 `migrate_counts_query_orders_by_source`: the migrate tab's count of favourites per source, through the
    /// repository over the real schema. Sources come back in order, favourites only.
    #[test]
    fn migrate_counts_query_orders_by_source() {
        for kind in KINDS {
            let (_dir, db) = db();
            let repo = EntryRepo::new(db);
            for (source, url, favorite) in [(9, "/a", true), (3, "/b", true), (9, "/c", true), (7, "/d", false)] {
                add(&repo, kind, source, url, favorite);
            }
            assert_eq!(repo.favorite_counts_by_source(kind).expect("counts"), [(3, 1), (9, 2)], "{kind}");
        }
    }

    #[test]
    fn a_manga_has_no_background_and_reads_as_an_episodes_title() {
        let (_dir, db) = db();
        let steps = LiveSteps::new(Kind::Manga, db, directory(Vec::new()), None, PrefStore::in_memory());
        let entry = Entry { id: 1, source: 1, title: "t".into(), item_flags: 0, viewer_flags: 0, date_added: 0 };
        assert!(!steps.has_custom(CustomImage::Background, &entry));
        assert_eq!(steps.copy_custom(CustomImage::Background, &entry, &entry), Ok(()));
        assert_eq!(steps.fetch_type(1), FetchType::Episodes);
        assert!(!steps.has_downloads(&entry), "no download index, no downloads");
    }

    /// The old entry's downloads go through the one service for both kinds (anime downloads were left behind before).
    #[test]
    fn delete_downloads_removes_the_old_entrys_folder_and_index_rows_for_both_kinds() {
        use kuro_data::source::StubSourceRepo;
        use kuro_downloads::gate::Connectivity;
        use kuro_downloads::index::{DownloadIndex, IndexRoots};
        use kuro_downloads::layout::Layout;
        use kuro_downloads::naming::UnitName;
        use kuro_downloads::pending::PendingStore;
        use kuro_downloads::queue::{Layouts, Queue, QueueParams};
        use kuro_downloads::store::QueueStore;

        let rt = runtime();
        let _entered = rt.enter();
        for kind in KINDS {
            let (dir, db) = db();
            StubSourceRepo::new(Arc::clone(&db), kind).upsert(1, "en", "S").expect("stub source");
            let (old_id, _) = pair(&db, kind);
            let roots = IndexRoots { manga: dir.path().join("manga"), anime: dir.path().join("anime") };
            let cache = dir.path().join("cache");
            std::fs::create_dir_all(&cache).expect("cache folder");
            let index = DownloadIndex::open(roots.clone(), &cache);
            let layouts = || Layouts { manga: Layout::new(Kind::Manga, &roots.manga), anime: Layout::new(Kind::Anime, &roots.anime) };
            let app_directory = AppDirectory::new(Arc::clone(&db));
            let (_net, connectivity) = tokio::sync::watch::channel(Connectivity { online: true, metered: false });
            let downloads = rt.block_on(async {
                let queue = Queue::spawn(QueueParams {
                    db: Arc::clone(&db),
                    store: Arc::new(QueueStore::new(Arc::clone(&db))),
                    runners: Vec::new(),
                    directory: Arc::new(app_directory.clone()),
                    layouts: layouts(),
                    index: index.clone(),
                    free_space: |_| None,
                    max_active: Arc::new(|| 1),
                    connectivity,
                    only_over_wifi: false,
                });
                Downloads::new(queue, index.clone(), layouts(), Arc::new(PendingStore::new(Arc::clone(&db))), Arc::new(app_directory), Arc::clone(&db))
            });
            let steps = LiveSteps::new(kind, Arc::clone(&db), directory(Vec::new()), Some(downloads), PrefStore::in_memory());
            let old = steps.entry(old_id).expect("old");
            let layout = layouts();
            let layout = if kind == Kind::Manga { layout.manga } else { layout.anime };
            let source = Layout::source_dir_name("S", "en");
            let unit = layout.unit_dir_names(UnitName::new("Unit 1", None))[0].clone();
            std::fs::create_dir_all(layout.entry_dir(&source, &old.title).expect("entry folder").join(&unit)).expect("unit folder");
            index.add_unit(kind, 1, &source, &old.title, &unit);
            assert!(steps.has_downloads(&old), "{kind:?}");
            assert_eq!(steps.delete_downloads(&old), Ok(()), "{kind:?}");
            assert!(layout.find_entry_dir(&source, &old.title).is_none(), "{kind:?}: the folder is gone");
            assert!(!steps.has_downloads(&old), "{kind:?}: the index followed");
        }
    }
}
