//! Manga chapter model and its partial update (`Chapter.kt`, `ChapterUpdate.kt`), and `ChapterRepo`
//! (`ChapterRepositoryImpl.kt`). The behaviour (`copy_from`, `to_update`, recognition) lives in `kuro-domain::items`.

use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::statements_manga::{ChapterPatch, ChapterRow, chapters};
use kuro_db::subscribe::Subscription;
use rusqlite::named_params;

use crate::{DataResult, ENTRY_DEBOUNCE, ID_CHUNK, none_if_not_found};

/// Tables a chapter list reads: the rows and the scanlator exclusions of the filter.
const CHAPTER_TABLES: &[&str] = &["chapters", "excluded_scanlators"];

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
        Self {
            id: -1,
            manga_id: -1,
            read: false,
            bookmark: false,
            last_page_read: 0,
            date_fetch: 0,
            source_order: 0,
            url: String::new(),
            name: String::new(),
            date_upload: -1,
            chapter_number: -1.0,
            scanlator: None,
            last_modified_at: 0,
            version: 1,
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

/// Identity of a chapter as Kotlin's `ChapterImpl.equals`/`hashCode` define it: the pair (id, url).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChapterKey {
    pub id: i64,
    pub url: String,
}

impl From<&Chapter> for ChapterKey {
    fn from(c: &Chapter) -> Self {
        Self { id: c.id, url: c.url.clone() }
    }
}

/// Chapter access over the manga database (`ChapterRepositoryImpl.kt`).
#[derive(Clone)]
pub struct ChapterRepo {
    db: Arc<Db>,
}

impl ChapterRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// The row with `id`, or `None` when no row has it.
    pub fn by_id(&self, id: i64) -> DataResult<Option<Chapter>> {
        let row = self.db.read(|c| none_if_not_found(chapters::get_by_id(c, id)))?;
        Ok(row.as_ref().map(map_chapter))
    }

    /// The lowest-id row with this url in the manga, or `None`. A duplicate url logs a warning (EC-09).
    pub fn by_url_and_manga_id(&self, url: &str, manga_id: i64) -> DataResult<Option<Chapter>> {
        let rows = self.db.read(|c| {
            let mut st = c.prepare_cached(
                "SELECT * FROM chapters WHERE url = :chapterUrl AND manga_id = :mangaId ORDER BY _id",
            )?;
            let rows = st
                .query_map(named_params! {":chapterUrl": url, ":mangaId": manga_id}, ChapterRow::from_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })?;
        if rows.len() > 1 {
            tracing::warn!(manga_id, url, count = rows.len(), "duplicate chapter url in one manga; using the lowest id");
        }
        Ok(rows.first().map(map_chapter))
    }

    /// Every chapter with this url, in any manga (`getChapterByUrl`). Empty when none matches.
    pub fn by_url(&self, url: &str) -> DataResult<Vec<Chapter>> {
        let rows = self.db.read(|c| Ok(chapters::get_all_by_url(c, url)?))?;
        Ok(rows.iter().map(map_chapter).collect())
    }

    /// Chapters of a manga. With `apply_scanlator_filter`, chapters of excluded scanlators are dropped.
    pub fn by_manga_id(&self, manga_id: i64, apply_scanlator_filter: bool) -> DataResult<Vec<Chapter>> {
        let rows = self.db.read(|c| Ok(chapters::get_by_manga_id(c, manga_id, apply_scanlator_filter)?))?;
        Ok(rows.iter().map(map_chapter).collect())
    }

    /// The bookmarked chapters of a manga.
    pub fn bookmarked(&self, manga_id: i64) -> DataResult<Vec<Chapter>> {
        let rows = self.db.read(|c| Ok(chapters::get_bookmarked_by_manga_id(c, manga_id)?))?;
        Ok(rows.iter().map(map_chapter).collect())
    }

    /// The scanlators of a manga's chapters, one entry per chapter, with a missing scanlator as `""`.
    pub fn scanlators(&self, manga_id: i64) -> DataResult<Vec<String>> {
        self.db.read(|c| Ok(chapters::get_scanlators_by_manga_id(c, manga_id)?))
    }

    /// The chapter list of a manga, published again after a change to the chapters or to the exclusions.
    pub fn subscribe_by_manga_id(&self, manga_id: i64, apply_scanlator_filter: bool) -> Subscription<Vec<Chapter>> {
        self.db.subscribe(CHAPTER_TABLES, ENTRY_DEBOUNCE, move |c| {
            Ok(chapters::get_by_manga_id(c, manga_id, apply_scanlator_filter)?
                .iter()
                .map(map_chapter)
                .collect::<Vec<_>>())
        })
    }

    /// Inserts the chapters in one transaction, in order. Returns them with their new ids, in the same order.
    /// A failure on any row leaves the table unchanged (EC-07).
    pub fn add_all(&self, new: &[Chapter]) -> DataResult<Vec<Chapter>> {
        let rows: Vec<ChapterRow> = new.iter().map(to_row).collect();
        let ids = self.db.write(|tx| Ok(chapters::insert_all(tx, &rows)?))?;
        Ok(new.iter().zip(ids).map(|(c, id)| Chapter { id, ..c.clone() }).collect())
    }

    /// Deletes the chapters with these ids, in chunks of [`ID_CHUNK`], in one transaction (EC-17).
    pub fn remove_with_ids(&self, ids: &[i64]) -> DataResult<()> {
        self.db.write(|tx| {
            for chunk in ids.chunks(ID_CHUNK) {
                chapters::remove_with_ids(tx, chunk)?;
            }
            Ok(())
        })
    }

    /// Updates one chapter. A missing id matches no row and is not an error.
    pub fn update(&self, u: &ChapterUpdate) -> DataResult<()> {
        let patch = to_patch(u);
        self.db.write(|tx| Ok(chapters::update(tx, &patch).map(drop)?))
    }

    /// Every update in one transaction: one failure rolls back all of them.
    pub fn update_all(&self, updates: &[ChapterUpdate]) -> DataResult<()> {
        self.db.write(|tx| {
            for u in updates {
                chapters::update(tx, &to_patch(u))?;
            }
            Ok(())
        })
    }
}

/// One `chapters` row as the domain sees it; `is_syncing` is dropped.
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

/// A new `chapters` row. The id is assigned by the insert and `is_syncing` is always false.
fn to_row(c: &Chapter) -> ChapterRow {
    ChapterRow {
        id: c.id,
        manga_id: c.manga_id,
        url: c.url.clone(),
        name: c.name.clone(),
        scanlator: c.scanlator.clone(),
        read: c.read,
        bookmark: c.bookmark,
        last_page_read: c.last_page_read,
        chapter_number: c.chapter_number,
        source_order: c.source_order,
        date_fetch: c.date_fetch,
        date_upload: c.date_upload,
        last_modified_at: c.last_modified_at,
        version: c.version,
        is_syncing: false,
    }
}

/// `partialUpdate`: every set field is written, and `is_syncing` is always cleared.
fn to_patch(u: &ChapterUpdate) -> ChapterPatch {
    ChapterPatch {
        id: u.id,
        manga_id: u.manga_id,
        url: u.url.clone(),
        name: u.name.clone(),
        scanlator: u.scanlator.clone(),
        read: u.read,
        bookmark: u.bookmark,
        last_page_read: u.last_page_read,
        chapter_number: u.chapter_number,
        source_order: u.source_order,
        date_fetch: u.date_fetch,
        date_upload: u.date_upload,
        version: u.version,
        is_syncing: Some(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::{insert_chapter, insert_manga, manga_db};

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
        let (_dir, db) = manga_db();
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
        let (_dir, db) = manga_db();
        let m = insert_manga(&db, "m", false);
        let repo = ChapterRepo::new(db.clone());
        // The second row points at a manga that does not exist: a foreign key failure.
        let result = repo.add_all(&[chapter(m, "ok", None), chapter(999, "bad", None)]);
        assert!(result.is_err());
        assert!(repo.by_manga_id(m, false).unwrap().is_empty());
    }

    #[test]
    fn remove_with_ids_handles_1200_ids_in_one_call() {
        let (_dir, db) = manga_db();
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
        let (_dir, db) = manga_db();
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

    #[test]
    fn scanlators_map_null_to_empty_and_keep_duplicates() {
        let (_dir, db) = manga_db();
        let m = insert_manga(&db, "m", false);
        let repo = ChapterRepo::new(db.clone());
        repo.add_all(&[chapter(m, "a", Some("G")), chapter(m, "b", Some("G")), chapter(m, "c", None)]).unwrap();
        let mut names = repo.scanlators(m).unwrap();
        names.sort();
        assert_eq!(names, ["", "G", "G"]);
    }

    #[test]
    fn bookmarked_returns_only_bookmarked_chapters() {
        let (_dir, db) = manga_db();
        let m = insert_manga(&db, "m", false);
        let repo = ChapterRepo::new(db.clone());
        let added = repo.add_all(&[chapter(m, "a", None), chapter(m, "b", None)]).unwrap();
        repo.update(&ChapterUpdate { bookmark: Some(true), ..ChapterUpdate::new(added[1].id) }).unwrap();
        let marked = repo.bookmarked(m).unwrap();
        assert_eq!(marked.iter().map(|c| c.url.as_str()).collect::<Vec<_>>(), ["b"]);
    }

    #[test]
    fn duplicate_url_returns_the_lowest_id() {
        let (_dir, db) = manga_db();
        let m = insert_manga(&db, "m", false);
        let first = insert_chapter(&db, m, "dup");
        let second = insert_chapter(&db, m, "dup");
        assert!(first < second);
        let found = ChapterRepo::new(db.clone()).by_url_and_manga_id("dup", m).unwrap().unwrap();
        assert_eq!(found.id, first);
    }

    #[test]
    fn a_chapter_list_subscription_re_emits_when_the_manga_is_deleted() {
        let (_dir, db) = manga_db();
        let m = insert_manga(&db, "m", false);
        insert_chapter(&db, m, "a");
        let repo = ChapterRepo::new(db.clone());
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut sub = repo.subscribe_by_manga_id(m, false);
            let first = sub.changed().await.unwrap().unwrap();
            assert_eq!(first.len(), 1);
            db.write(|tx| Ok(tx.execute("DELETE FROM mangas WHERE _id = ?1", [m]).map(drop)?)).unwrap();
            let after = sub.changed().await.unwrap().unwrap();
            assert!(after.is_empty(), "chapters of a deleted manga are gone");
        });
    }
}
