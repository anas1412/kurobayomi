//! The database side of the details page: the page read, the masked flag writes, the read and bookmark writes,
//! the category writes and the live subscriptions. Every method is synchronous; `runner.rs` calls them on the
//! tokio blocking pool. Nothing here imports GTK.
//! Upstream: `MangaScreenModel.kt:173-256` (load), `SetMangaChapterFlags.kt`, `UpdateChapter`, `UpdateManga`.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use kuro_data::DataResult;
use kuro_data::category::{CategoryKind, CategoryRepo};
use kuro_data::chapter::{Chapter, ChapterRepo, ChapterUpdate};
use kuro_data::manga::{Manga, MangaRepo};
use kuro_db::flags::chapter as c;
use kuro_db::handle::Db;
use kuro_db::models::MangaPatch;
use kuro_db::statements_manga as sm;
use kuro_db::subscribe::Subscription;
use kuro_downloads::DownloadState;

use crate::entry_core::chapter::ChapterRow;
use crate::entry_core::detail::MangaRow;
use crate::entry_core::flags::ChapterFlags;
use crate::entry_core::readstate::{ReadPlan, plan_mark_read};

/// `LocalSource.ID`: rows of the local source are always downloaded.
const LOCAL_SOURCE_ID: i64 = kuro_source::local::LOCAL_SOURCE_ID;

const MASKED_WRITE: &str =
    "UPDATE mangas SET chapter_flags = (chapter_flags & ~?2) | (?1 & ?2), version = version + 1 WHERE _id = ?3";
const WORD_WRITE: &str = "UPDATE mangas SET chapter_flags = ?1, version = version + 1 WHERE _id = ?2";

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
        initialized: m.initialized,
        favorite: m.favorite,
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
            categories: CategoryRepo::new(Arc::clone(&db), CategoryKind::Manga),
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

    /// One masked UPDATE of `mangas.chapter_flags`: `(old & !mask) | (flag & mask)`. The schema trigger bumps
    /// `mangas.version` only for url, description and favorite changes, so the statement bumps it itself: every flag
    /// write is a versioned write (E-DB-1). `Ok(false)` when the row is gone.
    pub fn set_flag_bits(&self, manga_id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        self.db.write(|tx| Ok(tx.execute(MASKED_WRITE, (flag, mask, manga_id))? > 0))
    }

    /// Sort by `mode`, or flip the direction when `mode` is the stored key. The stored word is read and written inside
    /// one writer transaction, so two quick taps cannot both see the same word. One versioned write.
    pub fn sort_by(&self, manga_id: i64, mode: i64) -> DataResult<bool> {
        self.db.write(|tx| {
            let stored = tx
                .prepare_cached("SELECT chapter_flags FROM mangas WHERE _id = ?1")?
                .query_map([manga_id], |r| r.get::<_, i64>(0))?
                .next();
            let Some(stored) = stored else { return Ok(false) };
            let stored = stored?;
            let word = ChapterFlags(stored)
                .sort_writes(mode)
                .into_iter()
                .fold(stored, |word, (flag, mask)| ChapterFlags(word).with_masked(flag, mask).0);
            Ok(tx.execute(WORD_WRITE, (word, manga_id))? > 0)
        })
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

    /// Replaces the category links in one transaction (`CategoryConfirm::SetCategories`).
    pub fn set_categories(&self, manga_id: i64, category_ids: &[i64]) -> DataResult<()> {
        self.mangas.set_categories(manga_id, category_ids)
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
    fn a_sort_flip_is_one_versioned_write() {
        let (_dir, store, id) = store(true, 0);
        let before = version(&store, id);
        store.sort_by(id, c::SORTING_ALPHABET).unwrap(); // new key: two masks, one write
        assert_eq!(version(&store, id), before + 1);
        assert!(!store.sort_by(999, c::SORTING_ALPHABET).unwrap());
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
    fn sort_by_flips_from_the_stored_row_inside_the_transaction() {
        let (_dir, store, id) = store(true, 0);
        store.set_flag_bits(id, c::SORTING_NUMBER, i64::MAX).unwrap(); // number, descending
        store.sort_by(id, c::SORTING_NUMBER).unwrap();
        assert_eq!(store.manga(id).unwrap().unwrap().flags.0, c::SORTING_NUMBER | c::SORT_ASC);
        store.sort_by(id, c::SORTING_NUMBER).unwrap();
        assert_eq!(store.manga(id).unwrap().unwrap().flags.0, c::SORTING_NUMBER);
        store.sort_by(id, c::SORTING_ALPHABET).unwrap();
        assert_eq!(
            store.manga(id).unwrap().unwrap().flags.0,
            c::SORTING_ALPHABET | c::SORT_ASC,
            "a new key starts ascending"
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
        let cat = kuro_data::category::CategoryRepo::new(Arc::clone(&store.db), CategoryKind::Manga)
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

    fn store_fresh() -> (tempfile::TempDir, DbStore, i64) {
        store(false, 0)
    }

    #[test]
    fn set_categories_replaces_the_links_and_categories_reload() {
        let (_dir, store, id) = store(true, 0);
        let repo = kuro_data::category::CategoryRepo::new(Arc::clone(&store.db), CategoryKind::Manga);
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
