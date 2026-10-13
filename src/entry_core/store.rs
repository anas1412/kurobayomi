//! The database side of the details page: the page read, the masked flag writes, the read and bookmark writes,
//! the category writes and the live subscriptions. Every method is synchronous; `runner.rs` calls them on the
//! tokio blocking pool. Nothing here imports GTK. The writes both pages make the same way are `entry_ops`.
//! Upstream: `MangaScreenModel.kt:173-256` (load), `SetMangaChapterFlags.kt`, `UpdateChapter`, `UpdateManga`.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use kuro_data::DataResult;
use kuro_data::category::CategoryRepo;
use kuro_data::entry::{Entry, EntryRepo};
use kuro_data::item::{Item, ItemRepo, ItemUpdate};
use kuro_data::track::TrackRepo;
use kuro_db::handle::Db;
use kuro_db::subscribe::Subscription;
use kuro_downloads::DownloadState;
use kuro_platform::kind::Kind;
use kuro_prefs::store::PrefStore;

use crate::entry_core::chapter::ChapterRow;
use crate::entry_core::detail::MangaRow;
use crate::entry_core::entry_ops;
use crate::entry_core::library_flow::LibraryRepos;
use crate::entry_core::flags::ChapterFlags;
use crate::entry_core::readstate::{ReadPlan, plan_bookmark, plan_done};
use crate::entry_core::refresh_rules::{self, Trigger};
use crate::entry_core::tracking::facts::Summary;

/// `LocalSource.ID`: rows of the local source are always downloaded.
const LOCAL_SOURCE_ID: i64 = kuro_source::local::LOCAL_SOURCE_ID;

/// The manga row fields the page reads.
pub fn manga_row(m: &Entry) -> MangaRow {
    MangaRow {
        id: m.id,
        source: m.source,
        url: m.url.clone(),
        title: m.title.clone(),
        author: m.author.clone(),
        artist: m.artist.clone(),
        description: m.description.clone(),
        genres: m.genre.clone().unwrap_or_default(),
        status: m.status,
        thumbnail_url: m.thumbnail_url.clone(),
        cover_last_modified: m.cover_last_modified,
        initialized: m.initialized,
        favorite: m.favorite,
        next_update: m.next_update,
        fetch_interval: m.fetch_interval,
        local: m.source == LOCAL_SOURCE_ID,
        flags: ChapterFlags(m.item_flags),
    }
}

/// The chapter rows, each with the download state of its chapter (rows with no entry are not downloaded).
pub fn chapter_rows(chapters: &[Item], downloads: &HashMap<i64, DownloadState>) -> Vec<ChapterRow> {
    chapters
        .iter()
        .map(|ch| ChapterRow {
            id: ch.id,
            name: ch.name.clone(),
            number: ch.number,
            source_order: ch.source_order,
            date_upload: ch.date_upload,
            read: ch.done,
            bookmark: ch.bookmark,
            last_page_read: ch.progress,
            scanlator: ch.scanlator.clone(),
            download: downloads.get(&ch.id).copied().unwrap_or(DownloadState::NotDownloaded),
            download_progress: 0,
        })
        .collect()
}

/// What the load reads besides the rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scanlators {
    pub available: Vec<String>,
    pub excluded: BTreeSet<String>,
}

/// Typed access to the tables of one manga page. Cheap to clone: it shares one `Db`.
#[derive(Clone)]
pub struct DbStore {
    entries: EntryRepo,
    items: ItemRepo,
    categories: CategoryRepo,
    tracks: TrackRepo,
    #[cfg(test)]
    db: Arc<Db>,
}

impl DbStore {
    pub fn new(db: Arc<Db>) -> Self {
        Self {
            entries: EntryRepo::new(Arc::clone(&db)),
            items: ItemRepo::new(Arc::clone(&db)),
            categories: CategoryRepo::new(Arc::clone(&db), Kind::Manga),
            tracks: TrackRepo::new(Arc::clone(&db)),
            #[cfg(test)]
            db,
        }
    }

    /// The row, or `None` when it does not exist.
    pub fn manga(&self, id: i64) -> DataResult<Option<MangaRow>> {
        Ok(entry_ops::entry_or_none(&self.entries, id)?.map(|m| manga_row(&m)))
    }

    /// The chapter list with the scanlator exclusions applied.
    pub fn chapter_list(&self, manga_id: i64, downloads: &HashMap<i64, DownloadState>) -> DataResult<Vec<ChapterRow>> {
        Ok(chapter_rows(&self.items.by_entry_id(manga_id, true)?, downloads))
    }

    /// The manga row, then its chapters. A missing row reads as `(None, [])`.
    pub fn read_page(
        &self,
        manga_id: i64,
        downloads: &HashMap<i64, DownloadState>,
    ) -> DataResult<(Option<MangaRow>, Vec<ChapterRow>)> {
        let Some(manga) = self.manga(manga_id)? else {
            return Ok((None, Vec::new()));
        };
        Ok((Some(manga), self.chapter_list(manga_id, downloads)?))
    }

    #[cfg(test)]
    /// How many trackers the manga is bound to, for the tracking button of the action row.
    pub fn track_count(&self, manga_id: i64) -> DataResult<usize> {
        Ok(self.tracks.by_entry(manga_id)?.len())
    }

    /// The tracking count and the logged-in flag of the manga, through the same filter as the sheet (T-TRK-12, D-10).
    pub fn track_summary(&self, manga_id: i64, source_id: i64) -> DataResult<Summary> {
        entry_ops::track_summary(&self.tracks, Kind::Manga, manga_id, source_id)
    }

    pub fn scanlators(&self, manga_id: i64) -> DataResult<Scanlators> {
        Ok(Scanlators {
            available: self.entries.available_scanlators(manga_id)?.into_iter().collect(),
            excluded: self.entries.excluded_scanlators(manga_id)?,
        })
    }

    /// Every category (the default row `0` included, the dialog drops it) and the ids the manga is linked to.
    pub fn categories(&self, manga_id: i64) -> DataResult<(Vec<(i64, String)>, Vec<i64>)> {
        entry_ops::categories(&self.categories, manga_id)
    }

    /// The Reset item of the settings sheet: the library's default item word replaces the manga's own (what
    /// `apply_item_defaults` writes). Opening the page never writes it: `SessionFlags` shows the defaults of a manga that
    /// is not in the library. `Ok(false)` when the row is gone.
    pub fn reset_flags(&self, manga_id: i64, defaults: ChapterFlags) -> DataResult<bool> {
        self.entries.update_item_flags(manga_id, |_| defaults.0)
    }

    /// `setFetchInterval`: stores `-wheel` (0 is automatic) and recomputes the next update through the domain (T-38b-022).
    pub fn set_fetch_interval(&self, manga_id: i64, wheel: i32) -> DataResult<()> {
        entry_ops::set_fetch_interval(&self.entries, &self.items, manga_id, wheel)
    }

    /// One masked UPDATE of `entries.item_flags`: `(old & !mask) | (flag & mask)`. A flag write is a versioned write for
    /// both kinds (E-DB-1, decision 11a). `Ok(false)` when the row is gone.
    pub fn set_flag_bits(&self, manga_id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        self.entries.set_item_flags_masked(manga_id, flag, mask)
    }

    /// Marks the given chapters read or unread in one transaction. Returns the plan that ran, or `None` when no row
    /// changed (the upstream `Result.NoChapters`).
    pub fn mark_read(
        &self,
        manga_id: i64,
        chapter_ids: &[i64],
        read: bool,
        remove_after_read: bool,
    ) -> DataResult<Option<ReadPlan>> {
        let rows = self.chosen_items(manga_id, chapter_ids)?;
        let Some(plan) = plan_done(read, &rows, remove_after_read) else {
            return Ok(None);
        };
        entry_ops::mark_done(&self.items, &rows, read)?;
        Ok(Some(plan))
    }

    /// Sets the bookmark of the given chapters in one transaction. Returns how many rows changed.
    pub fn set_bookmark(&self, manga_id: i64, chapter_ids: &[i64], bookmark: bool) -> DataResult<usize> {
        let rows = self.chosen_items(manga_id, chapter_ids)?;
        let updates: Vec<ItemUpdate> = plan_bookmark(bookmark, &rows)
            .into_iter()
            .map(|(id, on)| ItemUpdate { bookmark: Some(on), ..ItemUpdate::new(id) })
            .collect();
        if !updates.is_empty() {
            self.items.update_all(&updates)?;
        }
        Ok(updates.len())
    }

    /// The new chapters a refresh queues (`refresh_rules::new_item_targets`).
    pub fn new_item_targets(&self, prefs: &PrefStore, id: i64, trigger: Trigger, new_items: &[Item]) -> DataResult<Vec<Item>> {
        refresh_rules::new_item_targets(&self.entries, &self.categories, &self.items, prefs, Kind::Manga, id, trigger, new_items)
    }

    /// The repositories of the library button (`library_flow`).
    pub fn library(&self, prefs: &PrefStore) -> LibraryRepos {
        LibraryRepos::new(self.entries.clone(), self.categories.clone(), prefs.clone())
    }

    /// Replaces the category links in one transaction (`CategoryConfirm::SetCategories`).
    pub fn set_categories(&self, manga_id: i64, category_ids: &[i64]) -> DataResult<()> {
        self.entries.set_categories(manga_id, category_ids)
    }

    /// `SetExcludedScanlators`: the excluded set becomes exactly `wanted`, in one transaction.
    pub fn set_excluded_scanlators(&self, manga_id: i64, wanted: &BTreeSet<String>) -> DataResult<()> {
        self.entries.set_excluded_scanlators(manga_id, wanted)
    }

    /// The distinct scanlators of the chapters, published again after each change of the chapters.
    pub fn watch_available(&self, manga_id: i64) -> Subscription<BTreeSet<String>> {
        self.entries.subscribe_available_scanlators(manga_id)
    }

    /// The excluded set, published again after each change of the exclusions.
    pub fn watch_excluded(&self, manga_id: i64) -> Subscription<BTreeSet<String>> {
        self.entries.subscribe_excluded_scanlators(manga_id)
    }

    pub fn watch_manga(&self, manga_id: i64) -> Subscription<Option<Entry>> {
        self.entries.subscribe_by_id(manga_id)
    }

    pub fn watch_chapters(&self, manga_id: i64) -> Subscription<Vec<Item>> {
        self.items.subscribe_by_entry_id(manga_id, true)
    }

    /// The scanlator-filtered chapters that the ids name.
    fn chosen_items(&self, manga_id: i64, chapter_ids: &[i64]) -> DataResult<Vec<Item>> {
        let rows = self.items.by_entry_id(manga_id, true)?;
        Ok(rows.into_iter().filter(|r| chapter_ids.contains(&r.id)).collect())
    }
}

#[cfg(test)]
pub(crate) mod fixture {
    use super::*;

    /// A migrated temp database with one manga and `chapters` chapters. Keep the `TempDir` alive.
    pub fn store(favorite: bool, chapters: usize) -> (tempfile::TempDir, DbStore, i64) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("test.db");
        let writer = kuro_db::schema_unified::open_file(&path).expect("manga schema");
        let db = Arc::new(Db::new(&path, writer).expect("manga handle"));
        let store = DbStore::new(Arc::clone(&db));
        let manga_id = EntryRepo::new(Arc::clone(&db))
            .insert(&Entry {
                source: 7,
                url: "/m".into(),
                title: "Title".into(),
                favorite,
                ..Entry::create(Kind::Manga)
            })
            .expect("insert manga");
        let list: Vec<Item> = (1..=chapters)
            .map(|n| Item {
                entry_id: manga_id,
                url: format!("/c{n}"),
                name: format!("Chapter {n}"),
                number: n as f64,
                source_order: n as i64,
                ..Item::create(Kind::Manga)
            })
            .collect();
        ItemRepo::new(db).add_all(&list).expect("insert chapters");
        (dir, store, manga_id)
    }

    impl DbStore {
        /// Marks the details as fetched, so a load starts no fetch for it.
        pub fn mark_initialized_for_test(&self, manga_id: i64) {
            let update = kuro_data::entry::EntryUpdate {
                initialized: Some(true),
                ..kuro_data::entry::EntryUpdate::new(manga_id)
            };
            self.entries.update(&update).expect("update");
        }
    }

    impl DbStore {
        /// Another manga in the same database, with the given title and favorite flag.
        pub fn insert_manga_for_test(&self, title: &str, favorite: bool) -> i64 {
            self.entries
                .insert(&Entry { source: 8, url: format!("/{title}"), title: title.into(), favorite, ..Entry::create(Kind::Manga) })
                .expect("insert manga")
        }

        /// A user category.
        pub fn insert_category_for_test(&self, name: &str) -> i64 {
            kuro_testkit::insert_category(&self.db, Kind::Manga, name)
        }
    }

    pub fn version(store: &DbStore, manga_id: i64) -> i64 {
        store.entries.by_id(manga_id).expect("row").version
    }
}

#[cfg(test)]
mod tests {
    use kuro_db::flags::item as c;
    use crate::entry_core::library_flow::LibraryStore;

    use super::fixture::{store, version};
    use super::*;

    #[test]
    fn read_page_returns_manga_and_chapters_and_missing_is_none() {
        let (_dir, store, id) = store(true, 3);
        let (manga, chapters) = store.read_page(id, &HashMap::new()).unwrap();
        assert_eq!(manga.unwrap().title, "Title");
        assert_eq!(chapters.len(), 3);
        assert_eq!(store.read_page(999, &HashMap::new()).unwrap(), (None, vec![]));
    }

    /// The store with chapters 1, 2 and 3 by scanlators `A`, `B` and none.
    fn store_with_scanlators() -> (tempfile::TempDir, DbStore, i64) {
        let (dir, store, id) = store(true, 3);
        let rows = store.items.by_entry_id(id, false).unwrap();
        let by = ["A", "B"];
        for (i, chapter) in rows.iter().enumerate() {
            let scanlator = by.get(i).map(|s| (*s).to_owned());
            store.items.update_all(&[ItemUpdate { scanlator, ..ItemUpdate::new(chapter.id) }]).unwrap();
        }
        (dir, store, id)
    }

    #[test]
    fn excluded_null_scanlator_not_removed() {
        let (_dir, store, id) = store_with_scanlators();
        store.set_excluded_scanlators(id, &["A".to_owned(), "B".to_owned()].into()).unwrap();
        let shown = store.chapter_list(id, &HashMap::new()).unwrap();
        assert_eq!(shown.len(), 1, "both named scanlators are excluded");
        assert_eq!(shown[0].scanlator, None, "a chapter with no scanlator is never excluded (E-SCAN-1)");
    }

    #[test]
    fn excluded_diff_transaction() {
        let (_dir, store, id) = store_with_scanlators();
        store.set_excluded_scanlators(id, &["A".to_owned(), "B".to_owned()].into()).unwrap();
        store.set_excluded_scanlators(id, &["B".to_owned(), "C".to_owned()].into()).unwrap();
        let sets = store.scanlators(id).unwrap();
        assert_eq!(sets.excluded, ["B".to_owned(), "C".to_owned()].into(), "A left, C joined, B stayed");
        assert_eq!(sets.available, vec!["A".to_owned(), "B".to_owned()], "the available list reads the chapters");
        store.set_excluded_scanlators(id, &BTreeSet::new()).unwrap();
        assert!(store.scanlators(id).unwrap().excluded.is_empty());
        assert_eq!(store.chapter_list(id, &HashMap::new()).unwrap().len(), 3);
    }

    #[test]
    fn set_flag_bits_is_one_masked_update_that_bumps_the_version() {
        let (_dir, store, id) = store(true, 0);
        // a word of the six manga groups: the shared write cuts a manga's mask to them
        store.set_flag_bits(id, 0x234, i64::MAX).unwrap();
        let before = version(&store, id);
        assert!(store.set_flag_bits(id, 0x2, c::UNDONE_MASK).unwrap());
        let row = store.manga(id).unwrap().unwrap();
        assert_eq!(row.flags.0, (0x234 & !c::UNDONE_MASK) | 0x2);
        assert_eq!(version(&store, id), before + 1, "every write bumps entries.version");
        assert!(!store.set_flag_bits(999, 1, 1).unwrap(), "a missing row changes nothing");
    }

    #[test]
    fn a_new_sort_key_is_one_versioned_write() {
        let (_dir, store, id) = store(true, 0);
        let before = version(&store, id);
        assert!(store.set_flag_bits(id, c::SORTING_ALPHABET, c::SORTING_MASK).unwrap());
        assert_eq!(version(&store, id), before + 1);
        assert!(!store.set_flag_bits(999, c::SORTING_ALPHABET, c::SORTING_MASK).unwrap());
    }

    #[test]
    fn concurrent_writes_with_different_masks_both_persist() {
        let (_dir, store, id) = store(true, 0);
        store.set_flag_bits(id, 0, i64::MAX).unwrap();
        std::thread::scope(|scope| {
            for (flag, mask) in [(c::SHOW_UNDONE, c::UNDONE_MASK), (c::SHOW_BOOKMARKED, c::BOOKMARKED_MASK)] {
                let store = store.clone();
                scope.spawn(move || {
                    for _ in 0..20 {
                        store.set_flag_bits(id, flag, mask).unwrap();
                    }
                });
            }
        });
        assert_eq!(
            store.manga(id).unwrap().unwrap().flags.0,
            c::SHOW_UNDONE | c::SHOW_BOOKMARKED,
            "neither write lost the other's bits"
        );
    }

    #[test]
    fn mark_read_writes_one_batch_and_skips_rows_that_are_already_read() {
        let (_dir, store, id) = store(true, 3);
        let ids: Vec<i64> = store.chapter_list(id, &HashMap::new()).unwrap().iter().map(|r| r.id).collect();
        let plan = store.mark_read(id, &ids[..2], true, true).unwrap().expect("two rows change");
        assert_eq!(plan.changed.len(), 2);
        assert_eq!(plan.delete_downloads.len(), 2);
        assert!(store.mark_read(id, &ids[..2], true, false).unwrap().is_none(), "nothing left to mark");
        let rows = store.chapter_list(id, &HashMap::new()).unwrap();
        assert_eq!(rows.iter().filter(|r| r.read).count(), 2);
        store.mark_read(id, &ids, false, false).unwrap();
        assert!(store.chapter_list(id, &HashMap::new()).unwrap().iter().all(|r| !r.read));
    }

    #[test]
    fn bookmark_writes_only_rows_that_differ() {
        let (_dir, store, id) = store(true, 3);
        let ids: Vec<i64> = store.chapter_list(id, &HashMap::new()).unwrap().iter().map(|r| r.id).collect();
        assert_eq!(store.set_bookmark(id, &ids, true).unwrap(), 3);
        assert_eq!(store.set_bookmark(id, &ids, true).unwrap(), 0);
        assert_eq!(store.set_bookmark(id, &ids[..1], false).unwrap(), 1);
    }

    #[test]
    fn add_to_library_sets_favorite_and_categories_together_and_a_failed_link_leaves_it_out() {
        let (_dir, store, id) = store(false, 0);
        let cat = kuro_testkit::insert_category(&store.db, Kind::Manga, "Reading");
        let prefs = PrefStore::in_memory();
        store.library(&prefs).add(id, &[cat], 1_000).unwrap();
        assert!(store.manga(id).unwrap().unwrap().favorite);
        assert_eq!(store.categories(id).unwrap().1, vec![cat]);

        let (_dir2, store2, id2) = store_fresh();
        assert!(store2.library(&prefs).add(id2, &[4242], 1_000).is_err(), "unknown category");
        assert!(!store2.manga(id2).unwrap().unwrap().favorite, "the favorite was not written either");
    }

    #[test]
    fn adding_writes_the_library_defaults_and_opening_does_not() {
        let (_dir, store, id) = store(false, 0);
        let prefs = PrefStore::in_memory();
        kuro_domain::entries::defaults::set_item_settings_default(&prefs, Kind::Manga, c::SORTING_NUMBER | c::SORT_ASC | c::SHOW_UNDONE);
        assert_eq!(store.manga(id).unwrap().unwrap().flags.0, 0, "the stored word is untouched until the add");
        store.library(&prefs).add(id, &[], 1_000).unwrap();
        assert_eq!(store.manga(id).unwrap().unwrap().flags.0, c::SORTING_NUMBER | c::SORT_ASC | c::SHOW_UNDONE);
    }

    #[test]
    fn remove_from_library_clears_favorite_and_date_added_in_one_write() {
        let (_dir, store, id) = store(false, 0);
        store.library(&PrefStore::in_memory()).add(id, &[], 5_000).unwrap();
        assert_eq!(store.entries.by_id(id).unwrap().date_added, 5_000);
        let before = version(&store, id);
        assert!(store.library(&PrefStore::in_memory()).remove(id, 0).unwrap());
        let row = store.entries.by_id(id).unwrap();
        assert!(!row.favorite);
        assert_eq!(row.date_added, 0, "E-FAV-7");
        assert!(version(&store, id) > before);
        assert!(!store.library(&PrefStore::in_memory()).remove(999, 0).unwrap(), "a missing row changes nothing");
    }

    #[test]
    fn duplicates_and_user_categories_feed_the_add_routing() {
        let (_dir, store, id) = store(false, 0);
        let other = store.insert_manga_for_test("TITLE", true);
        store.insert_manga_for_test("Another", false);
        assert_eq!(store.library(&PrefStore::in_memory()).duplicates(id).unwrap().iter().map(|m| m.id).collect::<Vec<_>>(), vec![other], "only favorites are duplicates");
        assert!(store.library(&PrefStore::in_memory()).duplicates(other).unwrap().is_empty(), "the entry is never its own duplicate");
        let cat = store.insert_category_for_test("Reading");
        let users = store.library(&PrefStore::in_memory()).user_categories().unwrap();
        assert_eq!(users.iter().map(|c| c.id).collect::<Vec<_>>(), vec![cat], "the default row 0 is left out");
    }

    fn store_fresh() -> (tempfile::TempDir, DbStore, i64) {
        store(false, 0)
    }

    #[test]
    fn set_categories_replaces_the_links_and_categories_reload() {
        let (_dir, store, id) = store(true, 0);
        let a = kuro_testkit::insert_category(&store.db, Kind::Manga, "A");
        let b = kuro_testkit::insert_category(&store.db, Kind::Manga, "B");
        store.set_categories(id, &[a]).unwrap();
        store.set_categories(id, &[b]).unwrap();
        let (all, membership) = store.categories(id).unwrap();
        assert_eq!(membership, vec![b]);
        assert!(all.iter().any(|(cid, name)| *cid == a && name == "A"));
    }

    #[test]
    fn reset_writes_the_defaults_over_the_manga_word() {
        let (_dir, store, id) = store(true, 0);
        store.set_flag_bits(id, c::SHOW_BOOKMARKED, c::BOOKMARKED_MASK).unwrap();
        assert!(store.reset_flags(id, ChapterFlags(c::SORTING_NUMBER | c::SORT_ASC)).unwrap());
        assert_eq!(store.manga(id).unwrap().unwrap().flags.0, c::SORTING_NUMBER | c::SORT_ASC);
        assert!(!store.reset_flags(999, ChapterFlags(0)).unwrap());
    }
}
