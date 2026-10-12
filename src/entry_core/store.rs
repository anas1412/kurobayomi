//! The database side of the details page: the page read, the masked flag writes, the read and bookmark writes,
//! the category writes and the live subscriptions. Every method is synchronous; `runner.rs` calls them on the
//! tokio blocking pool. Nothing here imports GTK.
//! Upstream: `MangaScreenModel.kt:173-256` (load), `SetMangaChapterFlags.kt`, `UpdateChapter`, `UpdateManga`.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use kuro_data::DataResult;
use kuro_data::category::CategoryRepo;
use kuro_data::chapter::{Chapter, ChapterRepo, ChapterUpdate};
use kuro_data::manga::{Manga, MangaRepo};
use kuro_db::flags::chapter as c;
use kuro_domain::entries::fetch_interval::{get_window, set_manga_fetch_interval};
use kuro_db::handle::Db;
use kuro_db::models::MangaPatch;
use kuro_db::statements_manga as sm;
use kuro_db::subscribe::Subscription;
use kuro_downloads::DownloadState;

use crate::entry_core::chapter::ChapterRow;
use crate::entry_core::detail::MangaRow;
use crate::entry_core::flags::ChapterFlags;
use crate::entry_core::readstate::{ReadPlan, plan_mark_read};
use crate::entry_core::tracking::facts::Summary;
use crate::entry_core::tracking::session;
use kuro_platform::kind::Kind;

/// `LocalSource.ID`: rows of the local source are always downloaded.
const LOCAL_SOURCE_ID: i64 = kuro_source::local::LOCAL_SOURCE_ID;

const MASKED_WRITE: &str =
    "UPDATE mangas SET chapter_flags = (chapter_flags & ~?2) | (?1 & ?2), version = version + 1 WHERE _id = ?3";

/// Every chapter-flag bit the library defaults write (`SetMangaDefaultChapterFlags`).
pub const ALL_CHAPTER_FLAG_BITS: i64 = c::SORT_DIR_MASK
    | c::UNREAD_MASK
    | c::DOWNLOADED_MASK
    | c::BOOKMARKED_MASK
    | c::SORTING_MASK
    | c::DISPLAY_MASK;

/// The manga row fields the page reads.
pub fn manga_row(m: &Manga) -> MangaRow {
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
        flags: ChapterFlags(m.chapter_flags),
    }
}

/// The chapter rows, each with the download state of its chapter (rows with no entry are not downloaded).
pub fn chapter_rows(chapters: &[Chapter], downloads: &HashMap<i64, DownloadState>) -> Vec<ChapterRow> {
    chapters
        .iter()
        .map(|ch| ChapterRow {
            id: ch.id,
            name: ch.name.clone(),
            number: ch.chapter_number,
            source_order: ch.source_order,
            date_upload: ch.date_upload,
            read: ch.read,
            bookmark: ch.bookmark,
            last_page_read: ch.last_page_read,
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
    mangas: MangaRepo,
    chapters: ChapterRepo,
    categories: CategoryRepo,
    db: Arc<Db>,
}

impl DbStore {
    pub fn new(db: Arc<Db>) -> Self {
        Self {
            mangas: MangaRepo::new(Arc::clone(&db)),
            chapters: ChapterRepo::new(Arc::clone(&db)),
            categories: CategoryRepo::new(Arc::clone(&db), Kind::Manga),
            db,
        }
    }

    /// The row, or `None` when it does not exist.
    pub fn manga(&self, id: i64) -> DataResult<Option<MangaRow>> {
        match self.mangas.by_id(id) {
            Ok(m) => Ok(Some(manga_row(&m))),
            Err(kuro_data::DataError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The chapter list with the scanlator exclusions applied.
    pub fn chapter_list(&self, manga_id: i64, downloads: &HashMap<i64, DownloadState>) -> DataResult<Vec<ChapterRow>> {
        Ok(chapter_rows(&self.chapters.by_manga_id(manga_id, true)?, downloads))
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
        Ok(kuro_data::track::MangaTrackRepo::new(Arc::clone(&self.db)).by_entry(manga_id)?.len())
    }

    /// The tracking count and the logged-in flag of the manga, through the same filter as the sheet (T-TRK-12, D-10).
    pub fn track_summary(&self, manga_id: i64, source_id: i64) -> DataResult<Summary> {
        let bound: Vec<i64> = kuro_data::track::MangaTrackRepo::new(Arc::clone(&self.db)).by_entry(manga_id)?.iter().map(|t| t.tracker_id).collect();
        Ok(session::summary(Kind::Manga, source_id, &bound))
    }

    pub fn scanlators(&self, manga_id: i64) -> DataResult<Scanlators> {
        Ok(Scanlators {
            available: self.mangas.available_scanlators(manga_id)?.into_iter().collect(),
            excluded: self.mangas.excluded_scanlators(manga_id)?,
        })
    }

    /// Every category (the default row `0` included, the dialog drops it) and the ids the manga is linked to.
    pub fn categories(&self, manga_id: i64) -> DataResult<(Vec<(i64, String)>, Vec<i64>)> {
        let all = self.categories.all()?.into_iter().map(|cat| (cat.id, cat.name)).collect();
        let membership = self.categories.by_entry(manga_id)?.into_iter().map(|cat| cat.id).collect();
        Ok((all, membership))
    }

    /// Library defaults for a manga that is not in the library (`SetMangaDefaultChapterFlags`): one masked write.
    pub fn write_defaults(&self, manga_id: i64, defaults: ChapterFlags) -> DataResult<bool> {
        self.set_flag_bits(manga_id, defaults.0, ALL_CHAPTER_FLAG_BITS)
    }

    /// `setFetchInterval`: stores `-wheel` (0 is automatic) and recomputes the next update through the domain (T-38b-022).
    pub fn set_fetch_interval(&self, manga_id: i64, wheel: i32) -> DataResult<()> {
        let manga = self.mangas.by_id(manga_id)?;
        let now = chrono::Local::now();
        let window = get_window(&now);
        set_manga_fetch_interval(&self.mangas, &self.chapters, &manga, wheel, &now, window, now.timestamp_millis())
    }

    /// One masked UPDATE of `mangas.chapter_flags`: `(old & !mask) | (flag & mask)`. The schema trigger bumps
    /// `mangas.version` only for url, description and favorite changes, so the statement bumps it itself: every flag
    /// write is a versioned write (E-DB-1). `Ok(false)` when the row is gone.
    pub fn set_flag_bits(&self, manga_id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        self.db.write(|tx| Ok(tx.execute(MASKED_WRITE, (flag, mask, manga_id))? > 0))
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
        let rows = self.chosen_rows(manga_id, chapter_ids)?;
        let Some(plan) = plan_mark_read(read, &rows, remove_after_read) else {
            return Ok(None);
        };
        let updates: Vec<ChapterUpdate> = plan
            .writes
            .iter()
            .map(|w| ChapterUpdate {
                read: Some(w.read),
                last_page_read: w.last_page_read,
                ..ChapterUpdate::new(w.id)
            })
            .collect();
        self.chapters.update_all(&updates)?;
        Ok(Some(plan))
    }

    /// Sets the bookmark of the given chapters in one transaction. Returns how many rows changed.
    pub fn set_bookmark(&self, manga_id: i64, chapter_ids: &[i64], bookmark: bool) -> DataResult<usize> {
        let rows = self.chosen_rows(manga_id, chapter_ids)?;
        let updates: Vec<ChapterUpdate> = crate::entry_core::readstate::plan_bookmark(bookmark, &rows)
            .into_iter()
            .map(|(id, on)| ChapterUpdate {
                bookmark: Some(on),
                ..ChapterUpdate::new(id)
            })
            .collect();
        if !updates.is_empty() {
            self.chapters.update_all(&updates)?;
        }
        Ok(updates.len())
    }

    /// Adds the manga to the library and links its categories in one transaction (`CategoryConfirm::AddToLibrary`).
    pub fn add_to_library(&self, manga_id: i64, category_ids: &[i64], now_ms: i64) -> DataResult<()> {
        let patch = MangaPatch {
            id: manga_id,
            favorite: Some(true),
            date_added: Some(now_ms),
            ..MangaPatch::default()
        };
        self.db.write(|tx| {
            sm::mangas::update(tx, &patch)?;
            sm::mangas_categories::set_categories(tx, manga_id, category_ids)?;
            Ok(())
        })
    }

    /// Removes the manga from the library: `favorite = false` and `date_added = 0` in one write (E-FAV-7). The cover is
    /// deleted by the caller after this succeeds. `Ok(false)` when the row is gone.
    pub fn remove_from_library(&self, manga_id: i64) -> DataResult<bool> {
        let patch = MangaPatch {
            id: manga_id,
            favorite: Some(false),
            date_added: Some(0),
            ..MangaPatch::default()
        };
        self.db.write(|tx| Ok(sm::mangas::update(tx, &patch)? > 0))
    }

    /// `(id, title)` of every favorite, for the duplicate check.
    pub fn favorite_titles(&self) -> DataResult<Vec<(i64, String)>> {
        Ok(self.mangas.favorites()?.into_iter().map(|m| (m.id, m.title)).collect())
    }

    /// The user categories (the default row `0` is left out), for the default-category routing.
    pub fn user_categories(&self) -> DataResult<Vec<kuro_data::category::Category>> {
        Ok(self.categories.all()?.into_iter().filter(|c| c.id != 0).collect())
    }

    /// Replaces the category links in one transaction (`CategoryConfirm::SetCategories`).
    pub fn set_categories(&self, manga_id: i64, category_ids: &[i64]) -> DataResult<()> {
        self.mangas.set_categories(manga_id, category_ids)
    }

    /// `SetExcludedScanlators`: the excluded set becomes exactly `wanted`, in one transaction.
    pub fn set_excluded_scanlators(&self, manga_id: i64, wanted: &BTreeSet<String>) -> DataResult<()> {
        self.mangas.set_excluded_scanlators(manga_id, wanted)
    }

    /// The distinct scanlators of the chapters, published again after each change of the chapters.
    pub fn watch_available(&self, manga_id: i64) -> Subscription<BTreeSet<String>> {
        self.mangas.subscribe_available_scanlators(manga_id)
    }

    /// The excluded set, published again after each change of the exclusions.
    pub fn watch_excluded(&self, manga_id: i64) -> Subscription<BTreeSet<String>> {
        self.mangas.subscribe_excluded_scanlators(manga_id)
    }

    pub fn watch_manga(&self, manga_id: i64) -> Subscription<Option<Manga>> {
        self.mangas.subscribe_by_id(manga_id)
    }

    pub fn watch_chapters(&self, manga_id: i64) -> Subscription<Vec<Chapter>> {
        self.chapters.subscribe_by_manga_id(manga_id, true)
    }

    fn chosen_rows(&self, manga_id: i64, chapter_ids: &[i64]) -> DataResult<Vec<ChapterRow>> {
        let rows = self.chapter_list(manga_id, &HashMap::new())?;
        Ok(rows.into_iter().filter(|r| chapter_ids.contains(&r.id)).collect())
    }
}

#[cfg(test)]
pub(crate) mod fixture {
    use super::*;

    /// A migrated temp database with one manga and `chapters` chapters. Keep the `TempDir` alive.
    pub fn store(favorite: bool, chapters: usize) -> (tempfile::TempDir, DbStore, i64) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("manga.db");
        let writer = kuro_db::schema_manga::open(&path).expect("manga schema");
        let db = Arc::new(Db::new(&path, writer).expect("manga handle"));
        let store = DbStore::new(Arc::clone(&db));
        let manga_id = MangaRepo::new(Arc::clone(&db))
            .insert(&Manga {
                source: 7,
                url: "/m".into(),
                title: "Title".into(),
                favorite,
                ..Manga::create()
            })
            .expect("insert manga");
        let list: Vec<Chapter> = (1..=chapters)
            .map(|n| Chapter {
                manga_id,
                url: format!("/c{n}"),
                name: format!("Chapter {n}"),
                chapter_number: n as f64,
                source_order: n as i64,
                ..Chapter::create()
            })
            .collect();
        ChapterRepo::new(db).add_all(&list).expect("insert chapters");
        (dir, store, manga_id)
    }

    impl DbStore {
        /// Marks the details as fetched, so a load starts no fetch for it.
        pub fn mark_initialized_for_test(&self, manga_id: i64) {
            let update = kuro_data::manga::MangaUpdate {
                initialized: Some(true),
                ..kuro_data::manga::MangaUpdate::new(manga_id)
            };
            self.mangas.update(&update).expect("update");
        }
    }

    impl DbStore {
        /// Another manga in the same database, with the given title and favorite flag.
        pub fn insert_manga_for_test(&self, title: &str, favorite: bool) -> i64 {
            self.mangas
                .insert(&Manga { source: 8, url: format!("/{title}"), title: title.into(), favorite, ..Manga::create() })
                .expect("insert manga")
        }

        /// A user category.
        pub fn insert_category_for_test(&self, name: &str) -> i64 {
            self.categories
                .insert(&kuro_data::category::Category { id: 0, name: name.into(), order: 1, flags: 0, hidden: false })
                .expect("insert category")
        }
    }

    pub fn version(store: &DbStore, manga_id: i64) -> i64 {
        store.mangas.by_id(manga_id).expect("row").version
    }
}

#[cfg(test)]
mod tests {
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
        let rows = store.chapters.by_manga_id(id, false).unwrap();
        let by = ["A", "B"];
        for (i, chapter) in rows.iter().enumerate() {
            let scanlator = by.get(i).map(|s| (*s).to_owned());
            store.chapters.update_all(&[ChapterUpdate { scanlator, ..ChapterUpdate::new(chapter.id) }]).unwrap();
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
        store.set_flag_bits(id, 0x1234, i64::MAX).unwrap();
        let before = version(&store, id);
        assert!(store.set_flag_bits(id, 0x2, c::UNREAD_MASK).unwrap());
        let row = store.manga(id).unwrap().unwrap();
        assert_eq!(row.flags.0, (0x1234 & !c::UNREAD_MASK) | 0x2);
        assert_eq!(version(&store, id), before + 1, "every write bumps mangas.version");
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
            for (flag, mask) in [(c::SHOW_UNREAD, c::UNREAD_MASK), (c::SHOW_BOOKMARKED, c::BOOKMARKED_MASK)] {
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
            c::SHOW_UNREAD | c::SHOW_BOOKMARKED,
            "neither write lost the other's bits"
        );
    }

    #[test]
    fn mark_read_writes_one_batch_and_skips_rows_that_are_already_read() {
        let (_dir, store, id) = store(true, 3);
        let ids: Vec<i64> = store.chapter_list(id, &HashMap::new()).unwrap().iter().map(|r| r.id).collect();
        let plan = store.mark_read(id, &ids[..2], true, true).unwrap().expect("two rows change");
        assert_eq!(plan.writes.len(), 2);
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
    fn add_to_library_sets_favorite_and_categories_together_and_rolls_back_together() {
        let (_dir, store, id) = store(false, 0);
        let cat = kuro_data::category::CategoryRepo::new(Arc::clone(&store.db), Kind::Manga)
            .insert(&kuro_data::category::Category {
                id: 0,
                name: "Reading".into(),
                order: 1,
                flags: 0,
                hidden: false,
            })
            .unwrap();
        store.add_to_library(id, &[cat], 1_000).unwrap();
        assert!(store.manga(id).unwrap().unwrap().favorite);
        assert_eq!(store.categories(id).unwrap().1, vec![cat]);

        let (_dir2, store2, id2) = store_fresh();
        assert!(store2.add_to_library(id2, &[4242], 1_000).is_err(), "unknown category");
        assert!(!store2.manga(id2).unwrap().unwrap().favorite, "the favorite write rolled back too");
    }

    #[test]
    fn remove_from_library_clears_favorite_and_date_added_in_one_write() {
        let (_dir, store, id) = store(false, 0);
        store.add_to_library(id, &[], 5_000).unwrap();
        assert_eq!(store.mangas.by_id(id).unwrap().date_added, 5_000);
        let before = version(&store, id);
        assert!(store.remove_from_library(id).unwrap());
        let row = store.mangas.by_id(id).unwrap();
        assert!(!row.favorite);
        assert_eq!(row.date_added, 0, "E-FAV-7");
        assert!(version(&store, id) > before);
        assert!(!store.remove_from_library(999).unwrap(), "a missing row changes nothing");
    }

    #[test]
    fn favorite_titles_and_user_categories_feed_the_add_routing() {
        let (_dir, store, id) = store(true, 0);
        let other = store.insert_manga_for_test("Other", false);
        assert_eq!(store.favorite_titles().unwrap(), vec![(id, "Title".to_owned())], "only favorites are checked for duplicates");
        let cat = store.insert_category_for_test("Reading");
        let users = store.user_categories().unwrap();
        assert_eq!(users.iter().map(|c| c.id).collect::<Vec<_>>(), vec![cat], "the default row 0 is left out");
        let _ = other;
    }

    fn store_fresh() -> (tempfile::TempDir, DbStore, i64) {
        store(false, 0)
    }

    #[test]
    fn set_categories_replaces_the_links_and_categories_reload() {
        let (_dir, store, id) = store(true, 0);
        let repo = kuro_data::category::CategoryRepo::new(Arc::clone(&store.db), Kind::Manga);
        let mk = |name: &str, order| kuro_data::category::Category {
            id: 0,
            name: name.into(),
            order,
            flags: 0,
            hidden: false,
        };
        let a = repo.insert(&mk("A", 1)).unwrap();
        let b = repo.insert(&mk("B", 2)).unwrap();
        store.set_categories(id, &[a]).unwrap();
        store.set_categories(id, &[b]).unwrap();
        let (all, membership) = store.categories(id).unwrap();
        assert_eq!(membership, vec![b]);
        assert!(all.iter().any(|(cid, name)| *cid == a && name == "A"));
    }

    #[test]
    fn defaults_are_written_for_a_non_favorite() {
        let (_dir, store, id) = store(false, 0);
        store.write_defaults(id, ChapterFlags(c::SORTING_NUMBER | c::SORT_ASC)).unwrap();
        assert_eq!(store.manga(id).unwrap().unwrap().flags.0, c::SORTING_NUMBER | c::SORT_ASC);
    }
}
