//! Manga chapter model and its partial update (`Chapter.kt`, `ChapterUpdate.kt`), and `ChapterRepo`
//! (`ChapterRepositoryImpl.kt`) over the shared `Item` and `ItemRepo`. The behaviour (`copy_from`, `to_update`,
//! recognition) lives in `kuro-domain::items`. Phase E deletes this file.

use kuro_db::statements_manga::ChapterRow;
use kuro_db::subscribe::Subscription;
use kuro_platform::kind::Kind;

use crate::DataResult;
use crate::item::{Item, ItemKey, ItemUpdate};
use crate::legacy::{LegacyItem, LegacyItemRepo};

/// One `chapters` row as the domain sees it. `scanlator` is `None` for a blank scanlator.
#[derive(Clone, Debug, PartialEq)]
pub struct Chapter {
    pub id: i64,
    pub manga_id: i64,
    pub read: bool,
    pub bookmark: bool,
    pub last_page_read: i64,
    pub date_fetch: i64,
    pub source_order: i64,
    pub url: String,
    pub name: String,
    pub date_upload: i64,
    /// `-1.0` when unrecognised, `-2.0` for the intentionally numberless sentinel.
    pub chapter_number: f64,
    pub scanlator: Option<String>,
    pub last_modified_at: i64,
    pub version: i64,
}

impl Chapter {
    /// `Chapter.create()`: id and manga id -1, date upload -1, chapter number -1.0, version 1.
    pub fn create() -> Self {
        Self::from(Item::create(Kind::Manga))
    }
}

impl From<Item> for Chapter {
    fn from(i: Item) -> Self {
        Self {
            id: i.id,
            manga_id: i.entry_id,
            read: i.done,
            bookmark: i.bookmark,
            last_page_read: i.progress,
            date_fetch: i.date_fetch,
            source_order: i.source_order,
            url: i.url,
            name: i.name,
            date_upload: i.date_upload,
            chapter_number: i.number,
            scanlator: i.scanlator,
            last_modified_at: i.last_modified_at,
            version: i.version,
        }
    }
}

impl From<&Chapter> for Item {
    fn from(c: &Chapter) -> Self {
        Self {
            id: c.id,
            entry_id: c.manga_id,
            url: c.url.clone(),
            name: c.name.clone(),
            scanlator: c.scanlator.clone(),
            done: c.read,
            bookmark: c.bookmark,
            progress: c.last_page_read,
            number: c.chapter_number,
            source_order: c.source_order,
            date_fetch: c.date_fetch,
            date_upload: c.date_upload,
            last_modified_at: c.last_modified_at,
            version: c.version,
            ..Item::create(Kind::Manga)
        }
    }
}

/// Partial chapter update. `None` keeps the stored value; `id` is required.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChapterUpdate {
    pub id: i64,
    pub manga_id: Option<i64>,
    pub read: Option<bool>,
    pub bookmark: Option<bool>,
    pub last_page_read: Option<i64>,
    pub date_fetch: Option<i64>,
    pub source_order: Option<i64>,
    pub url: Option<String>,
    pub name: Option<String>,
    pub date_upload: Option<i64>,
    pub chapter_number: Option<f64>,
    pub scanlator: Option<String>,
    pub version: Option<i64>,
}

impl ChapterUpdate {
    pub fn new(id: i64) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }
}

impl From<&ChapterUpdate> for ItemUpdate {
    fn from(u: &ChapterUpdate) -> Self {
        Self {
            id: u.id,
            entry_id: u.manga_id,
            done: u.read,
            bookmark: u.bookmark,
            progress: u.last_page_read,
            date_fetch: u.date_fetch,
            source_order: u.source_order,
            url: u.url.clone(),
            name: u.name.clone(),
            date_upload: u.date_upload,
            number: u.chapter_number,
            scanlator: u.scanlator.clone(),
            version: u.version,
            ..Default::default()
        }
    }
}

impl LegacyItem for Chapter {
    const KIND: Kind = Kind::Manga;
    type Update = ChapterUpdate;

    fn to_item(&self) -> Item {
        self.into()
    }

    fn to_update(u: &ChapterUpdate) -> ItemUpdate {
        u.into()
    }
}

/// Identity of a chapter as Kotlin's `ChapterImpl.equals`/`hashCode` define it: the pair (id, url).
pub type ChapterKey = ItemKey;

impl From<&Chapter> for ItemKey {
    fn from(c: &Chapter) -> Self {
        Self { id: c.id, url: c.url.clone() }
    }
}

/// Chapter access over the manga items (`ChapterRepositoryImpl.kt`).
pub type ChapterRepo = LegacyItemRepo<Chapter>;

impl ChapterRepo {
    /// The lowest-id row with this url in the manga, or `None`. A duplicate url logs a warning.
    pub fn by_url_and_manga_id(&self, url: &str, manga_id: i64) -> DataResult<Option<Chapter>> {
        Ok(self.items.by_url_and_entry_id(url, manga_id)?.map(Chapter::from))
    }

    /// Chapters of a manga. With `apply_scanlator_filter`, chapters of excluded scanlators are dropped.
    pub fn by_manga_id(&self, manga_id: i64, apply_scanlator_filter: bool) -> DataResult<Vec<Chapter>> {
        Ok(Self::list(self.items.by_entry_id(manga_id, apply_scanlator_filter)?))
    }

    /// The scanlators of a manga's chapters, one entry per chapter, with a missing scanlator as `""`.
    pub fn scanlators(&self, manga_id: i64) -> DataResult<Vec<String>> {
        self.items.scanlators(manga_id)
    }

    /// The chapter list of a manga, published again after a change to the chapters or to the exclusions.
    pub fn subscribe_by_manga_id(&self, manga_id: i64, apply_scanlator_filter: bool) -> Subscription<Vec<Chapter>> {
        self.items.subscribe_by_entry_id_as(manga_id, apply_scanlator_filter)
    }
}

/// One `chapters` row of `statements_manga` as the domain sees it; `is_syncing` is dropped.
pub fn map_chapter(r: &ChapterRow) -> Chapter {
    Chapter {
        id: r.id,
        manga_id: r.manga_id,
        read: r.read,
        bookmark: r.bookmark,
        last_page_read: r.last_page_read,
        date_fetch: r.date_fetch,
        source_order: r.source_order,
        url: r.url.clone(),
        name: r.name.clone(),
        date_upload: r.date_upload,
        chapter_number: r.chapter_number,
        scanlator: r.scanlator.clone(),
        last_modified_at: r.last_modified_at,
        version: r.version,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::{insert_chapter, insert_manga, test_db};

    fn chapter(manga_id: i64, url: &str, scanlator: Option<&str>) -> Chapter {
        Chapter {
            manga_id,
            url: url.to_owned(),
            name: url.to_owned(),
            scanlator: scanlator.map(str::to_owned),
            ..Chapter::create()
        }
    }

    #[test]
    fn create_defaults_match_the_kotlin_factory() {
        let c = Chapter::create();
        assert_eq!((c.id, c.manga_id, c.date_upload, c.version), (-1, -1, -1, 1));
        assert_eq!(c.chapter_number, -1.0);
    }

    #[test]
    fn add_all_returns_new_ids_in_input_order() {
        let (_dir, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let repo = ChapterRepo::new(db.clone());
        let added = repo.add_all(&[chapter(m, "a", None), chapter(m, "b", None), chapter(m, "c", None)]).unwrap();
        let urls: Vec<&str> = added.iter().map(|c| c.url.as_str()).collect();
        assert_eq!(urls, ["a", "b", "c"]);
        assert!(added.windows(2).all(|w| w[0].id < w[1].id));
        assert_eq!(repo.by_manga_id(m, false).unwrap().len(), 3);
    }

    #[test]
    fn add_all_with_a_failing_row_leaves_the_table_unchanged() {
        let (_dir, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let repo = ChapterRepo::new(db.clone());
        // The second row points at a manga that does not exist: a foreign key failure.
        let result = repo.add_all(&[chapter(m, "ok", None), chapter(999, "bad", None)]);
        assert!(result.is_err());
        assert!(repo.by_manga_id(m, false).unwrap().is_empty());
    }

    #[test]
    fn remove_with_ids_handles_1200_ids_in_one_call() {
        let (_dir, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let repo = ChapterRepo::new(db.clone());
        let chapters: Vec<Chapter> = (0..1200).map(|i| chapter(m, &format!("u{i}"), None)).collect();
        let added = repo.add_all(&chapters).unwrap();
        let ids: Vec<i64> = added.iter().map(|c| c.id).collect();
        repo.remove_with_ids(&ids).unwrap();
        assert!(repo.by_manga_id(m, false).unwrap().is_empty());
    }

    #[test]
    fn by_manga_id_applies_the_scanlator_filter_exactly() {
        let (_dir, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let repo = ChapterRepo::new(db.clone());
        repo.add_all(&[chapter(m, "a", Some("Group A")), chapter(m, "b", Some("Group B")), chapter(m, "c", None)])
            .unwrap();
        db.write(|tx| Ok(kuro_db::statements_manga::excluded_scanlators::insert(tx, m, "Group B")?)).unwrap();
        let unfiltered: Vec<String> = repo.by_manga_id(m, false).unwrap().into_iter().map(|c| c.url).collect();
        let filtered: Vec<String> = repo.by_manga_id(m, true).unwrap().into_iter().map(|c| c.url).collect();
        assert_eq!(unfiltered.len(), 3);
        assert_eq!(filtered, ["a", "c"]);
    }

    /// T-05a-021: a chapter without a scanlator is never excluded, even when a NULL-like name is listed.
    #[test]
    fn a_null_scanlator_chapter_is_never_excluded() {
        let (_dir, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let repo = ChapterRepo::new(db.clone());
        repo.add_all(&[chapter(m, "c", None)]).unwrap();
        db.write(|tx| Ok(kuro_db::statements_manga::excluded_scanlators::insert(tx, m, "")?)).unwrap();
        let filtered: Vec<String> = repo.by_manga_id(m, true).unwrap().into_iter().map(|c| c.url).collect();
        assert_eq!(filtered, ["c"]);
    }

    /// EC-12: duplicate exclusion rows inserted by hand multiply the rows of a non-excluded chapter when the
    /// filter flag is 0, because the join runs regardless of the flag. With the flag set, the row is kept once.
    #[test]
    fn duplicate_exclusion_rows_multiply_chapters_only_when_the_filter_flag_is_zero() {
        let (_dir, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let repo = ChapterRepo::new(db.clone());
        repo.add_all(&[chapter(m, "a", Some("Group A"))]).unwrap();
        db.write(|tx| {
            kuro_db::statements_manga::excluded_scanlators::insert(tx, m, "Group B")?;
            kuro_db::statements_manga::excluded_scanlators::insert(tx, m, "Group B")?;
            kuro_db::statements_manga::excluded_scanlators::insert(tx, m, "Group A")?;
            kuro_db::statements_manga::excluded_scanlators::insert(tx, m, "Group A")?;
            Ok(())
        })
        .unwrap();
        // Group A is excluded twice: the flag-set query drops it, the flag-clear query still multiplies it.
        assert_eq!(repo.by_manga_id(m, false).unwrap().len(), 2);
        assert!(repo.by_manga_id(m, true).unwrap().is_empty());
    }

    #[test]
    fn scanlators_map_null_to_empty_and_keep_duplicates() {
        let (_dir, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let repo = ChapterRepo::new(db.clone());
        repo.add_all(&[chapter(m, "a", Some("G")), chapter(m, "b", Some("G")), chapter(m, "c", None)]).unwrap();
        let mut names = repo.scanlators(m).unwrap();
        names.sort();
        assert_eq!(names, ["", "G", "G"]);
    }

    #[test]
    fn bookmarked_returns_only_bookmarked_chapters() {
        let (_dir, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let repo = ChapterRepo::new(db.clone());
        let added = repo.add_all(&[chapter(m, "a", None), chapter(m, "b", None)]).unwrap();
        repo.update(&ChapterUpdate { bookmark: Some(true), ..ChapterUpdate::new(added[1].id) }).unwrap();
        let marked = repo.bookmarked(m).unwrap();
        assert_eq!(marked.iter().map(|c| c.url.as_str()).collect::<Vec<_>>(), ["b"]);
    }

    #[test]
    fn duplicate_url_returns_the_lowest_id() {
        let (_dir, db) = test_db();
        let m = insert_manga(&db, "m", false);
        let first = insert_chapter(&db, m, "dup");
        let second = insert_chapter(&db, m, "dup");
        assert!(first < second);
        let found = ChapterRepo::new(db.clone()).by_url_and_manga_id("dup", m).unwrap().unwrap();
        assert_eq!(found.id, first);
    }

    #[test]
    fn a_chapter_list_subscription_re_emits_when_the_manga_is_deleted() {
        let (_dir, db) = test_db();
        let m = insert_manga(&db, "m", false);
        insert_chapter(&db, m, "a");
        let repo = ChapterRepo::new(db.clone());
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut sub = repo.subscribe_by_manga_id(m, false);
            let first = sub.changed().await.unwrap().unwrap();
            assert_eq!(first.len(), 1);
            db.write(|tx| Ok(tx.execute("DELETE FROM entries WHERE _id = ?1", [m]).map(drop)?)).unwrap();
            let after = sub.changed().await.unwrap().unwrap();
            assert!(after.is_empty(), "chapters of a deleted manga are gone");
        });
    }
}
