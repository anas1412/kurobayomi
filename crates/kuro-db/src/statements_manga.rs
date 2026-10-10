//! Manga SQL statements, one module per `.sq` file, plus the storage row and patch structs.
//!
//! SQL text is the `.sq` text. Deliberate changes: `IN :list` becomes
//! `IN (SELECT value FROM json_each(:list))` (see `codec::ids_json`); category lists add `_id` as a
//! tie-break after `sort`; history search escapes `%`, `_` and `\` (`views::history`); and the
//! renumbering statements have no `.sq` source (`categories::renumber_all`). Functions take
//! `&Connection`, so they also run on a `&Transaction` through deref. None of them holds a runtime,
//! so the async wrapper can call them later.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::types::ToSql;
use rusqlite::{Connection, ErrorCode, OptionalExtension, Result, Row, Transaction};

use crate::codec::{ids_json, strings_json};
use crate::error::{DbError, DbResult};

/// Named parameters of one statement, as rusqlite takes them.
type Params<'a> = &'a [(&'a str, &'a dyn ToSql)];

fn query_all<T>(c: &Connection, sql: &str, p: Params<'_>, f: fn(&Row<'_>) -> Result<T>) -> Result<Vec<T>> {
    let mut st = c.prepare_cached(sql)?;
    let rows = st.query_map(p, f)?;
    rows.collect()
}

fn query_one<T>(c: &Connection, sql: &str, p: Params<'_>, f: fn(&Row<'_>) -> Result<T>) -> Result<T> {
    c.prepare_cached(sql)?.query_row(p, f)
}

/// Like [`query_one`], with a missing row reported as [`DbError::NotFound`].
fn query_found<T>(c: &Connection, sql: &str, p: Params<'_>, f: fn(&Row<'_>) -> Result<T>) -> DbResult<T> {
    query_one(c, sql, p, f).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => DbError::NotFound,
        other => other.into(),
    })
}

fn query_opt<T>(c: &Connection, sql: &str, p: Params<'_>, f: fn(&Row<'_>) -> Result<T>) -> Result<Option<T>> {
    c.prepare_cached(sql)?.query_row(p, f).optional()
}

fn exec(c: &Connection, sql: &str, p: Params<'_>) -> Result<usize> {
    c.prepare_cached(sql)?.execute(p)
}

pub use crate::models::*;

/// Statements of `mangas.sq`.
pub mod mangas {
    use super::*;

    pub const GET_BY_ID: &str = "SELECT * FROM mangas WHERE _id = :id;";
    pub const GET_BY_URL_AND_SOURCE: &str =
        "SELECT * FROM mangas WHERE url = :url AND source = :source LIMIT 1;";
    pub const GET_FAVORITES: &str = "SELECT * FROM mangas WHERE favorite = 1;";
    pub const GET_READ_NOT_IN_LIBRARY: &str = "SELECT * FROM mangas WHERE favorite = 0 AND _id IN ( SELECT DISTINCT chapters.manga_id FROM chapters WHERE read = 1 OR last_page_read != 0 );";
    pub const GET_ALL: &str = "SELECT * FROM mangas;";
    pub const GET_ALL_SOURCE_AND_URL: &str = "SELECT source, url FROM mangas;";
    pub const GET_WITH_FAVORITE_TIMESTAMP: &str =
        "SELECT * FROM mangas WHERE favorite_modified_at IS NOT NULL;";
    pub const GET_SOURCE_ID_WITH_FAVORITE_COUNT: &str =
        "SELECT source, count(*) FROM mangas WHERE favorite = 1 GROUP BY source ORDER BY source;";
    pub const GET_FAVORITE_BY_SOURCE_ID: &str =
        "SELECT * FROM mangas WHERE favorite = 1 AND source = :sourceId;";
    pub const GET_DUPLICATE_LIBRARY_MANGA: &str =
        "SELECT * FROM mangas WHERE favorite = 1 AND kuro_lower(title) = :title AND _id != :id;";
    pub const GET_UPCOMING_MANGA: &str = "SELECT * FROM mangas WHERE next_update >= :startOfDay AND favorite = 1 AND status IN (SELECT value FROM json_each(:statuses)) ORDER BY next_update ASC;";
    pub const RESET_VIEWER_FLAGS: &str = "UPDATE mangas SET viewer = 0;";
    /// Replaces only the bits of `mask` in `viewer`, in one statement, so two setters of different bit groups cannot
    /// lose each other's write (EC-23). Named parameters: `:mangaId`, `:value`, `:mask`.
    pub const SET_VIEWER_BITS: &str =
        "UPDATE mangas SET viewer = (viewer & ~:mask) | (:value & :mask) WHERE _id = :mangaId;";
    pub const RESET_IS_SYNCING: &str = "UPDATE mangas SET is_syncing = 0 WHERE is_syncing = 1;";
    pub const GET_SOURCE_IDS_WITH_NON_LIBRARY_MANGA: &str =
        "SELECT source, COUNT(*) AS manga_count FROM mangas WHERE favorite = 0 GROUP BY source;";
    pub const DELETE_NOT_IN_LIBRARY_BY_SOURCE_IDS: &str = "DELETE FROM mangas WHERE favorite = 0 AND source IN (SELECT value FROM json_each(:sourceIds));";
    pub const INSERT: &str = "INSERT INTO mangas(source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, chapter_flags, cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, version) VALUES (:source, :url, :artist, :author, :description, :genre, :title, :status, :thumbnailUrl, :favorite, :lastUpdate, :nextUpdate, :initialized, :viewerFlags, :chapterFlags, :coverLastModified, :dateAdded, :updateStrategy, :calculateInterval, 0, :version);";
    pub const UPDATE: &str = "UPDATE mangas SET source = coalesce(:source, source), url = coalesce(:url, url), artist = coalesce(:artist, artist), author = coalesce(:author, author), description = coalesce(:description, description), genre = coalesce(:genre, genre), title = coalesce(:title, title), status = coalesce(:status, status), thumbnail_url = coalesce(:thumbnailUrl, thumbnail_url), favorite = coalesce(:favorite, favorite), last_update = coalesce(:lastUpdate, last_update), next_update = coalesce(:nextUpdate, next_update), initialized = coalesce(:initialized, initialized), viewer = coalesce(:viewer, viewer), chapter_flags = coalesce(:chapterFlags, chapter_flags), cover_last_modified = coalesce(:coverLastModified, cover_last_modified), date_added = coalesce(:dateAdded, date_added), update_strategy = coalesce(:updateStrategy, update_strategy), calculate_interval = coalesce(:calculateInterval, calculate_interval), version = coalesce(:version, version), is_syncing = coalesce(:isSyncing, is_syncing) WHERE _id = :mangaId;";
    pub const LAST_INSERT_ROWID: &str = "SELECT last_insert_rowid();";

    /// Upcoming-tab statuses: ONGOING (1) and PUBLISHING_FINISHED (4), as `GetUpcomingManga` passes them.
    pub const UPCOMING_STATUSES: [i64; 2] = [1, 4];

    pub fn get_by_id(c: &Connection, id: i64) -> DbResult<MangaRow> {
        query_found(c, GET_BY_ID, &[(":id", &id)], MangaRow::from_row)
    }

    pub fn get_by_url_and_source(c: &Connection, url: &str, source: i64) -> Result<Option<MangaRow>> {
        query_opt(c, GET_BY_URL_AND_SOURCE, &[(":url", &url), (":source", &source)], MangaRow::from_row)
    }

    pub fn get_favorites(c: &Connection) -> Result<Vec<MangaRow>> {
        query_all(c, GET_FAVORITES, &[], MangaRow::from_row)
    }

    pub fn get_read_not_in_library(c: &Connection) -> Result<Vec<MangaRow>> {
        query_all(c, GET_READ_NOT_IN_LIBRARY, &[], MangaRow::from_row)
    }

    pub fn get_all(c: &Connection) -> Result<Vec<MangaRow>> {
        query_all(c, GET_ALL, &[], MangaRow::from_row)
    }

    pub fn get_all_source_and_url(c: &Connection) -> Result<Vec<(i64, String)>> {
        query_all(c, GET_ALL_SOURCE_AND_URL, &[], |r| Ok((r.get(0)?, r.get(1)?)))
    }

    pub fn get_with_favorite_timestamp(c: &Connection) -> Result<Vec<MangaRow>> {
        query_all(c, GET_WITH_FAVORITE_TIMESTAMP, &[], MangaRow::from_row)
    }

    /// `(source, favourite count)` for every source with at least one favourite.
    pub fn get_source_id_with_favorite_count(c: &Connection) -> Result<Vec<(i64, i64)>> {
        query_all(c, GET_SOURCE_ID_WITH_FAVORITE_COUNT, &[], |r| Ok((r.get(0)?, r.get(1)?)))
    }

    pub fn get_favorite_by_source_id(c: &Connection, source_id: i64) -> Result<Vec<MangaRow>> {
        query_all(c, GET_FAVORITE_BY_SOURCE_ID, &[(":sourceId", &source_id)], MangaRow::from_row)
    }

    /// `title` must already be lowercased by the caller (`str::to_lowercase`), as upstream does.
    /// Library favourites with the same title, ignoring case for every Unicode letter (`kuro_lower`), except `id`.
    /// `title` is lowercased here so the comparison works for any input case.
    pub fn get_duplicate_library_manga(c: &Connection, title: &str, id: i64) -> Result<Vec<MangaRow>> {
        let title = title.to_lowercase();
        query_all(
            c,
            GET_DUPLICATE_LIBRARY_MANGA,
            &[(":title", &title), (":id", &id)],
            MangaRow::from_row,
        )
    }

    /// Favourites with `next_update >= start_of_day_ms` and a status in `statuses`, ascending by
    /// `next_update`. The caller computes start of day in local time. Empty `statuses` queries nothing.
    pub fn get_upcoming_manga(c: &Connection, start_of_day_ms: i64, statuses: &[i64]) -> Result<Vec<MangaRow>> {
        if statuses.is_empty() {
            return Ok(Vec::new());
        }
        let json = ids_json(statuses);
        query_all(
            c,
            GET_UPCOMING_MANGA,
            &[(":startOfDay", &start_of_day_ms), (":statuses", &json)],
            MangaRow::from_row,
        )
    }

    pub fn reset_viewer_flags(c: &Connection) -> Result<usize> {
        exec(c, RESET_VIEWER_FLAGS, &[])
    }

    /// Writes `value` under `mask` into `viewer` of one manga. Returns the rows changed: `0` for a deleted manga (EC-24).
    pub fn set_viewer_bits(c: &Connection, manga_id: i64, value: i64, mask: i64) -> Result<usize> {
        c.prepare_cached(SET_VIEWER_BITS)?.execute(rusqlite::named_params! {
            ":mangaId": manga_id,
            ":value": value,
            ":mask": mask,
        })
    }

    pub fn reset_is_syncing(c: &Connection) -> Result<usize> {
        exec(c, RESET_IS_SYNCING, &[])
    }

    /// `(source, non-library manga count)` for every source with at least one such entry.
    pub fn get_source_ids_with_non_library_manga(c: &Connection) -> Result<Vec<(i64, i64)>> {
        query_all(c, GET_SOURCE_IDS_WITH_NON_LIBRARY_MANGA, &[], |r| Ok((r.get(0)?, r.get(1)?)))
    }

    /// Deletes non-library manga of the given sources. Empty `source_ids` deletes nothing.
    pub fn delete_not_in_library_by_source_ids(c: &Connection, source_ids: &[i64]) -> Result<usize> {
        if source_ids.is_empty() {
            return Ok(0);
        }
        let json = ids_json(source_ids);
        exec(c, DELETE_NOT_IN_LIBRARY_BY_SOURCE_IDS, &[(":sourceIds", &json)])
    }

    /// Inserts `m` and returns its new id. `m.id`, `m.last_modified_at` and `m.favorite_modified_at`
    /// are ignored: the row gets `last_modified_at = 0` and a NULL `favorite_modified_at`.
    pub fn insert(c: &Connection, m: &MangaRow) -> Result<i64> {
        exec(
            c,
            INSERT,
            &[
                (":source", &m.source),
                (":url", &m.url),
                (":artist", &m.artist),
                (":author", &m.author),
                (":description", &m.description),
                (":genre", &m.genre),
                (":title", &m.title),
                (":status", &m.status),
                (":thumbnailUrl", &m.thumbnail_url),
                (":favorite", &m.favorite),
                (":lastUpdate", &m.last_update),
                (":nextUpdate", &m.next_update),
                (":initialized", &m.initialized),
                (":viewerFlags", &m.viewer),
                (":chapterFlags", &m.chapter_flags),
                (":coverLastModified", &m.cover_last_modified),
                (":dateAdded", &m.date_added),
                (":updateStrategy", &m.update_strategy),
                (":calculateInterval", &m.calculate_interval),
                (":version", &m.version),
            ],
        )?;
        Ok(c.last_insert_rowid())
    }

    /// Updates the set columns of `p`. An empty patch runs no statement and returns 0.
    pub fn update(c: &Connection, p: &MangaPatch) -> Result<usize> {
        if p.is_empty() {
            return Ok(0);
        }
        exec(
            c,
            UPDATE,
            &[
                (":source", &p.source),
                (":url", &p.url),
                (":artist", &p.artist),
                (":author", &p.author),
                (":description", &p.description),
                (":genre", &p.genre),
                (":title", &p.title),
                (":status", &p.status),
                (":thumbnailUrl", &p.thumbnail_url),
                (":favorite", &p.favorite),
                (":lastUpdate", &p.last_update),
                (":nextUpdate", &p.next_update),
                (":initialized", &p.initialized),
                (":viewer", &p.viewer),
                (":chapterFlags", &p.chapter_flags),
                (":coverLastModified", &p.cover_last_modified),
                (":dateAdded", &p.date_added),
                (":updateStrategy", &p.update_strategy),
                (":calculateInterval", &p.calculate_interval),
                (":version", &p.version),
                (":isSyncing", &p.is_syncing),
                (":mangaId", &p.id),
            ],
        )
    }

    pub fn last_inserted_row_id(c: &Connection) -> Result<i64> {
        query_one(c, LAST_INSERT_ROWID, &[], |r| r.get(0))
    }
}

/// Statements of `chapters.sq`.
pub mod chapters {
    use super::*;

    pub const GET_BY_ID: &str = "SELECT * FROM chapters WHERE _id = :id;";
    pub const GET_BY_MANGA_ID: &str = "SELECT C.* FROM chapters C LEFT JOIN excluded_scanlators ES ON C.manga_id = ES.manga_id AND C.scanlator = ES.scanlator WHERE C.manga_id = :mangaId AND ( :applyScanlatorFilter = 0 OR ES.scanlator IS NULL );";
    pub const GET_SCANLATORS_BY_MANGA_ID: &str = "SELECT scanlator FROM chapters WHERE manga_id = :mangaId;";
    pub const GET_BOOKMARKED_BY_MANGA_ID: &str = "SELECT * FROM chapters WHERE bookmark AND manga_id = :mangaId;";
    pub const GET_BY_URL: &str = "SELECT * FROM chapters WHERE url = :chapterUrl;";
    pub const GET_BY_URL_AND_MANGA_ID: &str = "SELECT * FROM chapters WHERE url = :chapterUrl AND manga_id = :mangaId;";
    pub const REMOVE_WITH_IDS: &str = "DELETE FROM chapters WHERE _id IN (SELECT value FROM json_each(:chapterIds));";
    pub const RESET_IS_SYNCING: &str = "UPDATE chapters SET is_syncing = 0 WHERE is_syncing = 1;";
    pub const INSERT: &str = "INSERT INTO chapters(manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order,date_fetch, date_upload, last_modified_at, version, is_syncing) VALUES (:mangaId, :url, :name, :scanlator, :read, :bookmark, :lastPageRead, :chapterNumber, :sourceOrder, :dateFetch, :dateUpload, 0, :version, 0);";
    pub const UPDATE: &str = "UPDATE chapters SET manga_id = coalesce(:mangaId, manga_id), url = coalesce(:url, url), name = coalesce(:name, name), scanlator = coalesce(:scanlator, scanlator), read = coalesce(:read, read), bookmark = coalesce(:bookmark, bookmark), last_page_read = coalesce(:lastPageRead, last_page_read), chapter_number = coalesce(:chapterNumber, chapter_number), source_order = coalesce(:sourceOrder, source_order), date_fetch = coalesce(:dateFetch, date_fetch), date_upload = coalesce(:dateUpload, date_upload), version = coalesce(:version, version), is_syncing = coalesce(:isSyncing, is_syncing) WHERE _id = :chapterId;";
    pub const LAST_INSERT_ROWID: &str = "SELECT last_insert_rowid();";

    pub fn get_by_id(c: &Connection, id: i64) -> DbResult<ChapterRow> {
        query_found(c, GET_BY_ID, &[(":id", &id)], ChapterRow::from_row)
    }

    /// Chapters of a manga. With `apply_scanlator_filter`, chapters whose `(manga_id, scanlator)`
    /// is excluded are dropped; NULL-scanlator chapters are never dropped. No order is guaranteed.
    pub fn get_by_manga_id(c: &Connection, manga_id: i64, apply_scanlator_filter: bool) -> Result<Vec<ChapterRow>> {
        query_all(
            c,
            GET_BY_MANGA_ID,
            &[(":mangaId", &manga_id), (":applyScanlatorFilter", &apply_scanlator_filter)],
            ChapterRow::from_row,
        )
    }

    /// Every row's scanlator, NOT distinct, with NULL mapped to `""`.
    pub fn get_scanlators_by_manga_id(c: &Connection, manga_id: i64) -> Result<Vec<String>> {
        query_all(c, GET_SCANLATORS_BY_MANGA_ID, &[(":mangaId", &manga_id)], |r| {
            Ok(r.get::<_, Option<String>>(0)?.unwrap_or_default())
        })
    }

    pub fn get_bookmarked_by_manga_id(c: &Connection, manga_id: i64) -> Result<Vec<ChapterRow>> {
        query_all(c, GET_BOOKMARKED_BY_MANGA_ID, &[(":mangaId", &manga_id)], ChapterRow::from_row)
    }

    /// First chapter with this url (any manga), as the backup restorer reads it.
    pub fn get_by_url(c: &Connection, url: &str) -> Result<Option<ChapterRow>> {
        query_opt(c, GET_BY_URL, &[(":chapterUrl", &url)], ChapterRow::from_row)
    }

    /// Every chapter with this url, in any manga (`getChapterByUrl`).
    pub fn get_all_by_url(c: &Connection, url: &str) -> Result<Vec<ChapterRow>> {
        query_all(c, GET_BY_URL, &[(":chapterUrl", &url)], ChapterRow::from_row)
    }

    pub fn get_by_url_and_manga_id(c: &Connection, url: &str, manga_id: i64) -> Result<Option<ChapterRow>> {
        query_opt(
            c,
            GET_BY_URL_AND_MANGA_ID,
            &[(":chapterUrl", &url), (":mangaId", &manga_id)],
            ChapterRow::from_row,
        )
    }

    /// Deletes the given chapters in one statement. Empty `ids` runs no statement.
    pub fn remove_with_ids(c: &Connection, ids: &[i64]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let json = ids_json(ids);
        exec(c, REMOVE_WITH_IDS, &[(":chapterIds", &json)])
    }

    pub fn reset_is_syncing(c: &Connection) -> Result<usize> {
        exec(c, RESET_IS_SYNCING, &[])
    }

    /// Inserts `r` and returns its new id. `r.id`, `r.last_modified_at` and `r.is_syncing` are ignored.
    pub fn insert(c: &Connection, r: &ChapterRow) -> Result<i64> {
        exec(
            c,
            INSERT,
            &[
                (":mangaId", &r.manga_id),
                (":url", &r.url),
                (":name", &r.name),
                (":scanlator", &r.scanlator),
                (":read", &r.read),
                (":bookmark", &r.bookmark),
                (":lastPageRead", &r.last_page_read),
                (":chapterNumber", &r.chapter_number),
                (":sourceOrder", &r.source_order),
                (":dateFetch", &r.date_fetch),
                (":dateUpload", &r.date_upload),
                (":version", &r.version),
            ],
        )?;
        Ok(c.last_insert_rowid())
    }

    /// Inserts every row of a batch. The caller owns the transaction: an error here must be
    /// propagated and the transaction dropped, which rolls back the rows already inserted.
    pub fn insert_all(tx: &Transaction<'_>, rows: &[ChapterRow]) -> Result<Vec<i64>> {
        rows.iter().map(|r| insert(tx, r)).collect()
    }

    /// Updates the set columns of `p`. An empty patch runs no statement and returns 0.
    pub fn update(c: &Connection, p: &ChapterPatch) -> Result<usize> {
        if p.is_empty() {
            return Ok(0);
        }
        exec(
            c,
            UPDATE,
            &[
                (":mangaId", &p.manga_id),
                (":url", &p.url),
                (":name", &p.name),
                (":scanlator", &p.scanlator),
                (":read", &p.read),
                (":bookmark", &p.bookmark),
                (":lastPageRead", &p.last_page_read),
                (":chapterNumber", &p.chapter_number),
                (":sourceOrder", &p.source_order),
                (":dateFetch", &p.date_fetch),
                (":dateUpload", &p.date_upload),
                (":version", &p.version),
                (":isSyncing", &p.is_syncing),
                (":chapterId", &p.id),
            ],
        )
    }

    pub fn last_inserted_row_id(c: &Connection) -> Result<i64> {
        query_one(c, LAST_INSERT_ROWID, &[], |r| r.get(0))
    }
}

/// Statements of `categories.sq`, plus the renumbering that a delete needs.
pub mod categories {
    use super::*;

    pub const GET_CATEGORY: &str = "SELECT * FROM categories WHERE _id = :id LIMIT 1;";
    pub const GET_CATEGORIES: &str = "SELECT _id AS id, name, sort AS `order`, flags, hidden FROM categories ORDER BY sort, _id;";
    pub const GET_VISIBLE_CATEGORIES: &str = "SELECT _id AS id, name, sort AS `order`, flags, hidden FROM categories WHERE hidden = 0 ORDER BY sort, _id;";
    pub const GET_CATEGORIES_BY_MANGA_ID: &str = "SELECT C._id AS id, C.name, C.sort AS `order`, C.flags, C.hidden FROM categories C JOIN mangas_categories MC ON C._id = MC.category_id WHERE MC.manga_id = :mangaId;";
    pub const GET_VISIBLE_CATEGORIES_BY_MANGA_ID: &str = "SELECT C._id AS id, C.name, C.sort AS `order`, C.flags, C.hidden FROM categories C JOIN mangas_categories MC ON C._id = MC.category_id WHERE MC.manga_id = :mangaId AND C.hidden = 0;";
    pub const INSERT: &str = "INSERT INTO categories(name, sort, flags) VALUES (:name, :order, :flags);";
    pub const DELETE: &str = "DELETE FROM categories WHERE _id = :categoryId;";
    pub const UPDATE: &str = "UPDATE categories SET name = coalesce(:name, name), sort = coalesce(:order, sort), flags = coalesce(:flags, flags), hidden = coalesce(:hidden, hidden) WHERE _id = :categoryId;";
    pub const UPDATE_ALL_FLAGS: &str = "UPDATE categories SET flags = coalesce(?, flags);";
    pub const LAST_INSERT_ROWID: &str = "SELECT last_insert_rowid();";
    /// Every category id in display order, the system category (0) included.
    pub const IDS_BY_ORDER: &str = "SELECT _id FROM categories ORDER BY sort, _id;";
    pub const SET_ORDER: &str = "UPDATE categories SET sort = :order WHERE _id = :categoryId;";

    pub fn get_category(c: &Connection, id: i64) -> Result<Option<CategoryRow>> {
        query_opt(c, GET_CATEGORY, &[(":id", &id)], CategoryRow::from_row)
    }

    pub fn get_categories(c: &Connection) -> Result<Vec<CategoryRow>> {
        query_all(c, GET_CATEGORIES, &[], CategoryRow::from_row)
    }

    pub fn get_visible_categories(c: &Connection) -> Result<Vec<CategoryRow>> {
        query_all(c, GET_VISIBLE_CATEGORIES, &[], CategoryRow::from_row)
    }

    pub fn get_categories_by_manga_id(c: &Connection, manga_id: i64) -> Result<Vec<CategoryRow>> {
        query_all(c, GET_CATEGORIES_BY_MANGA_ID, &[(":mangaId", &manga_id)], CategoryRow::from_row)
    }

    pub fn get_visible_categories_by_manga_id(c: &Connection, manga_id: i64) -> Result<Vec<CategoryRow>> {
        query_all(c, GET_VISIBLE_CATEGORIES_BY_MANGA_ID, &[(":mangaId", &manga_id)], CategoryRow::from_row)
    }

    /// Creates a category and returns its id (`max(_id) + 1`).
    pub fn insert(c: &Connection, name: &str, order: i64, flags: i64) -> Result<i64> {
        exec(c, INSERT, &[(":name", &name), (":order", &order), (":flags", &flags)])?;
        Ok(c.last_insert_rowid())
    }

    /// Deletes a category; its links cascade. Id 0 fails with `System category can't be deleted`.
    pub fn delete(c: &Connection, id: i64) -> Result<usize> {
        exec(c, DELETE, &[(":categoryId", &id)])
    }

    pub fn update(c: &Connection, p: &CategoryPatch) -> Result<usize> {
        exec(
            c,
            UPDATE,
            &[
                (":name", &p.name),
                (":order", &p.order),
                (":flags", &p.flags),
                (":hidden", &p.hidden),
                (":categoryId", &p.id),
            ],
        )
    }

    /// Sets `flags` on every row, system category included. `None` changes nothing.
    pub fn update_all_flags(c: &Connection, flags: Option<i64>) -> Result<usize> {
        c.prepare_cached(UPDATE_ALL_FLAGS)?.execute([flags])
    }

    pub fn last_inserted_row_id(c: &Connection) -> Result<i64> {
        query_one(c, LAST_INSERT_ROWID, &[], |r| r.get(0))
    }

    /// Renumbers every category, the system row included, to its index in display order, from 0 (as
    /// Aniyomi `DeleteMangaCategory` does; plans 05 EC-10 and 08 T-08a-010). Run inside the transaction
    /// of the delete; each category is one UPDATE, not a correlated subquery, because SQLite would
    /// read rows already renumbered by the same statement.
    pub fn renumber_all(tx: &Transaction<'_>) -> Result<()> {
        let ids: Vec<i64> = query_all(tx, IDS_BY_ORDER, &[], |r| r.get(0))?;
        for (order, id) in (0_i64..).zip(ids.iter()) {
            exec(tx, SET_ORDER, &[(":order", &order), (":categoryId", id)])?;
        }
        Ok(())
    }
}

/// Statements of `mangas_categories.sq`.
pub mod mangas_categories {
    use super::*;

    pub const INSERT: &str = "INSERT INTO mangas_categories(manga_id, category_id) VALUES (:mangaId, :categoryId);";
    pub const DELETE_BY_MANGA_ID: &str = "DELETE FROM mangas_categories WHERE manga_id = :mangaId;";

    pub fn insert(c: &Connection, manga_id: i64, category_id: i64) -> Result<usize> {
        exec(c, INSERT, &[(":mangaId", &manga_id), (":categoryId", &category_id)])
    }

    pub fn delete_by_manga_id(c: &Connection, manga_id: i64) -> Result<usize> {
        exec(c, DELETE_BY_MANGA_ID, &[(":mangaId", &manga_id)])
    }

    /// Replaces the categories of a manga: delete all links, then insert each distinct id once
    /// (sorted). Each inserted link bumps `mangas.version` through its trigger. An empty list leaves
    /// the manga uncategorised.
    pub fn set_categories(tx: &Transaction<'_>, manga_id: i64, ids: &[i64]) -> Result<()> {
        delete_by_manga_id(tx, manga_id)?;
        let mut unique = ids.to_vec();
        unique.sort_unstable();
        unique.dedup();
        for id in unique {
            insert(tx, manga_id, id)?;
        }
        Ok(())
    }
}

/// Statements of `history.sq`.
pub mod history {
    use super::*;

    pub const GET_BY_MANGA_ID: &str = "SELECT H._id, H.chapter_id, H.last_read, H.time_read FROM history H JOIN chapters C ON H.chapter_id = C._id WHERE C.manga_id = :mangaId AND C._id = H.chapter_id;";
    pub const GET_BY_CHAPTER_URL: &str = "SELECT H._id, H.chapter_id, H.last_read, H.time_read FROM history H JOIN chapters C ON H.chapter_id = C._id WHERE C.url = :chapterUrl AND C._id = H.chapter_id;";
    pub const RESET_BY_ID: &str = "UPDATE history SET last_read = 0 WHERE _id = :historyId;";
    pub const RESET_BY_MANGA_ID: &str = "UPDATE history SET last_read = 0 WHERE _id IN ( SELECT H._id FROM mangas M INNER JOIN chapters C ON M._id = C.manga_id INNER JOIN history H ON C._id = H.chapter_id WHERE M._id = :mangaId );";
    pub const REMOVE_ALL: &str = "DELETE FROM history;";
    pub const REMOVE_RESETTED: &str = "DELETE FROM history WHERE last_read = 0;";
    pub const UPSERT: &str = "INSERT INTO history(chapter_id, last_read, time_read) VALUES (:chapterId, :readAt, :time_read) ON CONFLICT(chapter_id) DO UPDATE SET last_read = :readAt, time_read = time_read + :time_read WHERE chapter_id = :chapterId;";
    pub const GET_READ_DURATION: &str = "SELECT coalesce(sum(time_read), 0) FROM history;";

    pub fn get_by_manga_id(c: &Connection, manga_id: i64) -> Result<Vec<HistoryRow>> {
        query_all(c, GET_BY_MANGA_ID, &[(":mangaId", &manga_id)], HistoryRow::from_row)
    }

    /// First history row of a chapter url, as the backup restorer reads it.
    pub fn get_by_chapter_url(c: &Connection, url: &str) -> Result<Option<HistoryRow>> {
        query_opt(c, GET_BY_CHAPTER_URL, &[(":chapterUrl", &url)], HistoryRow::from_row)
    }

    /// Sets `last_read = 0` on one row; the row and its `time_read` stay.
    pub fn reset_by_id(c: &Connection, history_id: i64) -> Result<usize> {
        exec(c, RESET_BY_ID, &[(":historyId", &history_id)])
    }

    pub fn reset_by_manga_id(c: &Connection, manga_id: i64) -> Result<usize> {
        exec(c, RESET_BY_MANGA_ID, &[(":mangaId", &manga_id)])
    }

    pub fn remove_all(c: &Connection) -> Result<usize> {
        exec(c, REMOVE_ALL, &[])
    }

    /// Deletes only rows reset to `last_read = 0`.
    pub fn remove_resetted(c: &Connection) -> Result<usize> {
        exec(c, REMOVE_RESETTED, &[])
    }

    /// Records a read: sets `last_read` to `read_at_ms` and ADDS `delta_ms` to `time_read`.
    pub fn upsert(c: &Connection, chapter_id: i64, read_at_ms: i64, delta_ms: i64) -> Result<usize> {
        exec(
            c,
            UPSERT,
            &[(":chapterId", &chapter_id), (":readAt", &read_at_ms), (":time_read", &delta_ms)],
        )
    }

    pub fn get_read_duration(c: &Connection) -> Result<i64> {
        query_one(c, GET_READ_DURATION, &[], |r| r.get(0))
    }
}

/// Statements of `libraryView.sq`, `historyView.sq` (the views are in `sql/schema_v33.sql`).
pub mod views {
    use super::*;

    pub const LIBRARY: &str = "SELECT * FROM libraryView;";
    pub const HISTORY: &str = "SELECT id, mangaId, chapterId, title, thumbnailUrl, source, favorite, cover_last_modified, chapterNumber, readAt, readDuration FROM historyView WHERE historyView.readAt > 0 AND maxReadAtChapterId = historyView.chapterId AND lower(historyView.title) LIKE ('%' || :query || '%') ESCAPE '\\' ORDER BY readAt DESC;";
    pub const GET_LATEST_HISTORY: &str = "SELECT id, mangaId, chapterId, title, thumbnailUrl, source, favorite, cover_last_modified, chapterNumber, readAt, readDuration FROM historyView WHERE historyView.readAt > 0 ORDER BY readAt DESC LIMIT 1;";
    pub const GET_RECENT_UPDATES: &str = "SELECT * FROM updatesView WHERE dateUpload > :after ORDER BY datefetch DESC LIMIT :limit;";
    pub const GET_UPDATES_BY_READ_STATUS: &str = "SELECT * FROM updatesView WHERE read = :read AND dateUpload > :after ORDER BY datefetch DESC LIMIT :limit;";

    /// Row limit the Updates screen passes to both update queries (`GetMangaUpdates`).
    pub const UPDATES_PAGE_LIMIT: i64 = 500;

    /// Every library entry with one row per category link (`category = 0` when it has none).
    pub fn library(c: &Connection) -> Result<Vec<LibraryRow>> {
        query_all(c, LIBRARY, &[], LibraryRow::from_row)
    }

    /// Groups library rows by their `category` value, keeping the order of the input.
    pub fn group_by_category(rows: Vec<LibraryRow>) -> BTreeMap<i64, Vec<LibraryRow>> {
        let mut grouped: BTreeMap<i64, Vec<LibraryRow>> = BTreeMap::new();
        for row in rows {
            grouped.entry(row.category).or_default().push(row);
        }
        grouped
    }

    /// Escapes `\`, `%` and `_` so the query matches them literally (`ESCAPE '\'`).
    pub fn escape_like(query: &str) -> String {
        let mut out = String::with_capacity(query.len());
        for ch in query.chars() {
            if matches!(ch, '\\' | '%' | '_') {
                out.push('\\');
            }
            out.push(ch);
        }
        out
    }

    /// One row per manga (its latest-read chapter), newest read first. An empty `query` matches all.
    /// The search is a substring match on the title, case-insensitive for ASCII (SQLite `lower`).
    pub fn history(c: &Connection, query: &str) -> Result<Vec<HistoryViewRow>> {
        let escaped = escape_like(query);
        query_all(c, HISTORY, &[(":query", &escaped)], HistoryViewRow::from_row)
    }

    pub fn get_latest_history(c: &Connection) -> Result<Option<HistoryViewRow>> {
        query_opt(c, GET_LATEST_HISTORY, &[], HistoryViewRow::from_row)
    }

    /// Updates whose upload date is after `after` (ms), newest fetch first, at most `limit` rows.
    pub fn get_recent_updates(c: &Connection, after: i64, limit: i64) -> Result<Vec<UpdateViewRow>> {
        query_all(
            c,
            GET_RECENT_UPDATES,
            &[(":after", &after), (":limit", &limit)],
            UpdateViewRow::from_row,
        )
    }

    /// As [`get_recent_updates`], restricted to chapters whose `read` flag equals `read`.
    pub fn get_updates_by_read_status(
        c: &Connection,
        read: bool,
        after: i64,
        limit: i64,
    ) -> Result<Vec<UpdateViewRow>> {
        query_all(
            c,
            GET_UPDATES_BY_READ_STATUS,
            &[(":read", &read), (":after", &after), (":limit", &limit)],
            UpdateViewRow::from_row,
        )
    }
}

/// Statements of `manga_sync.sq`: tracker bindings. `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`
/// means a second binding of one tracker to one manga replaces the first, under a new `_id`.
pub mod manga_sync {
    use super::*;

    /// Tracker ids stored in `sync_id` (`TrackerManager.kt`, manga trackers 1 to 9).
    pub const MYANIMELIST: i64 = 1;
    pub const ANILIST: i64 = 2;
    pub const KITSU: i64 = 3;
    pub const SHIKIMORI: i64 = 4;
    pub const BANGUMI: i64 = 5;
    pub const KOMGA: i64 = 6;
    pub const MANGAUPDATES: i64 = 7;
    pub const KAVITA: i64 = 8;
    pub const SUWAYOMI: i64 = 9;

    pub const DELETE: &str = "DELETE FROM manga_sync WHERE manga_id = :mangaId AND sync_id = :syncId;";
    pub const GET_TRACKS: &str = "SELECT * FROM manga_sync;";
    pub const GET_TRACK_BY_ID: &str = "SELECT * FROM manga_sync WHERE _id = :id;";
    pub const GET_TRACKS_BY_MANGA_ID: &str = "SELECT * FROM manga_sync WHERE manga_id = :mangaId;";
    pub const UPSERT: &str = "INSERT INTO manga_sync(manga_id,sync_id,remote_id,library_id,title,last_chapter_read,total_chapters,status,score,remote_url,start_date,finish_date,private) VALUES (:mangaId,:syncId,:remoteId,:libraryId,:title,:lastChapterRead,:totalChapters,:status,:score,:remoteUrl,:startDate,:finishDate,:private) ON CONFLICT(manga_id, sync_id) DO UPDATE SET remote_id = excluded.remote_id, library_id = excluded.library_id, title = excluded.title, last_chapter_read = excluded.last_chapter_read, total_chapters = excluded.total_chapters, status = excluded.status, score = excluded.score, remote_url = excluded.remote_url, start_date = excluded.start_date, finish_date = excluded.finish_date, private = excluded.private RETURNING _id;";
    pub const INSERT: &str = "INSERT INTO manga_sync(manga_id,sync_id,remote_id,library_id,title,last_chapter_read,total_chapters,status,score,remote_url,start_date,finish_date,private) VALUES (:mangaId,:syncId,:remoteId,:libraryId,:title,:lastChapterRead,:totalChapters,:status,:score,:remoteUrl,:startDate,:finishDate,:private);";
    pub const UPDATE: &str = "UPDATE manga_sync SET manga_id = coalesce(:mangaId, manga_id), sync_id = coalesce(:syncId, sync_id), remote_id = coalesce(:mediaId, remote_id), library_id = coalesce(:libraryId, library_id), title = coalesce(:title, title), last_chapter_read = coalesce(:lastChapterRead, last_chapter_read), total_chapters = coalesce(:totalChapter, total_chapters), status = coalesce(:status, status), score = coalesce(:score, score), remote_url = coalesce(:trackingUrl, remote_url), start_date = coalesce(:startDate, start_date), finish_date = coalesce(:finishDate, finish_date), private = coalesce(:private, private) WHERE _id = :id;";

    /// Removes the binding of `manga_id` to tracker `sync_id`.
    pub fn delete(c: &Connection, manga_id: i64, sync_id: i64) -> Result<usize> {
        exec(c, DELETE, &[(":mangaId", &manga_id), (":syncId", &sync_id)])
    }

    pub fn get_tracks(c: &Connection) -> Result<Vec<TrackRow>> {
        query_all(c, GET_TRACKS, &[], TrackRow::from_row)
    }

    /// The binding with row id `id` (the track's own `_id`, not the manga's).
    pub fn get_track_by_id(c: &Connection, id: i64) -> DbResult<TrackRow> {
        query_found(c, GET_TRACK_BY_ID, &[(":id", &id)], TrackRow::from_row)
    }

    pub fn get_tracks_by_manga_id(c: &Connection, manga_id: i64) -> Result<Vec<TrackRow>> {
        query_all(c, GET_TRACKS_BY_MANGA_ID, &[(":mangaId", &manga_id)], TrackRow::from_row)
    }

    /// Inserts `r` and returns its new id. `r.id` is ignored.
    pub fn insert(c: &Connection, r: &TrackRow) -> Result<i64> {
        exec(
            c,
            INSERT,
            &[
                (":mangaId", &r.manga_id),
                (":syncId", &r.sync_id),
                (":remoteId", &r.remote_id),
                (":libraryId", &r.library_id),
                (":title", &r.title),
                (":lastChapterRead", &r.last_chapter_read),
                (":totalChapters", &r.total_chapters),
                (":status", &r.status),
                (":score", &r.score),
                (":remoteUrl", &r.remote_url),
                (":startDate", &r.start_date),
                (":finishDate", &r.finish_date),
                (":private", &r.private),
            ],
        )?;
        Ok(c.last_insert_rowid())
    }

    /// Upsert keyed by `(manga_id, sync_id)` (Q-TRK-2): a second track of one tracker updates the row in place, so
    /// its `_id` is kept. Returns the row id. `ON CONFLICT ... DO UPDATE` takes precedence over the table's
    /// `ON CONFLICT REPLACE` for this key.
    pub fn upsert(c: &Connection, r: &TrackRow) -> Result<i64> {
        c.prepare_cached(UPSERT)?.query_row(
            &[
                (":mangaId", &r.manga_id as &dyn ToSql),
                (":syncId", &r.sync_id),
                (":remoteId", &r.remote_id),
                (":libraryId", &r.library_id),
                (":title", &r.title),
                (":lastChapterRead", &r.last_chapter_read),
                (":totalChapters", &r.total_chapters),
                (":status", &r.status),
                (":score", &r.score),
                (":remoteUrl", &r.remote_url),
                (":startDate", &r.start_date),
                (":finishDate", &r.finish_date),
                (":private", &r.private),
            ][..],
            |row| row.get(0),
        )
    }

    /// Updates the set columns of `p`. Unset columns keep their value (`coalesce`).
    pub fn update(c: &Connection, p: &TrackPatch) -> Result<usize> {
        exec(
            c,
            UPDATE,
            &[
                (":mangaId", &p.manga_id),
                (":syncId", &p.sync_id),
                (":mediaId", &p.remote_id),
                (":libraryId", &p.library_id),
                (":title", &p.title),
                (":lastChapterRead", &p.last_chapter_read),
                (":totalChapter", &p.total_chapters),
                (":status", &p.status),
                (":score", &p.score),
                (":trackingUrl", &p.remote_url),
                (":startDate", &p.start_date),
                (":finishDate", &p.finish_date),
                (":private", &p.private),
                (":id", &p.id),
            ],
        )
    }
}

/// Statements of `excluded_scanlators.sq`. The table has no key, so [`set_excluded`] is the only
/// writer that avoids duplicates.
pub mod excluded_scanlators {
    use super::*;

    pub const INSERT: &str = "INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (:mangaId, :scanlator);";
    pub const REMOVE: &str = "DELETE FROM excluded_scanlators WHERE manga_id = :mangaId AND scanlator IN (SELECT value FROM json_each(:scanlators));";
    pub const GET_BY_MANGA_ID: &str = "SELECT scanlator FROM excluded_scanlators WHERE manga_id = :mangaId;";

    pub fn insert(c: &Connection, manga_id: i64, scanlator: &str) -> Result<usize> {
        exec(c, INSERT, &[(":mangaId", &manga_id), (":scanlator", &scanlator)])
    }

    /// Deletes the listed names of `manga_id`. Empty `scanlators` runs no statement.
    pub fn remove(c: &Connection, manga_id: i64, scanlators: &[String]) -> Result<usize> {
        if scanlators.is_empty() {
            return Ok(0);
        }
        let json = strings_json(scanlators);
        exec(c, REMOVE, &[(":mangaId", &manga_id), (":scanlators", &json)])
    }

    pub fn get_by_manga_id(c: &Connection, manga_id: i64) -> Result<Vec<String>> {
        query_all(c, GET_BY_MANGA_ID, &[(":mangaId", &manga_id)], |r| r.get(0))
    }

    /// Makes the excluded set of `manga_id` equal to `wanted`: inserts the names not stored yet and
    /// removes the stored names not wanted. The caller owns `tx`, so the read and the writes commit
    /// together.
    pub fn set_excluded(tx: &Transaction<'_>, manga_id: i64, wanted: &BTreeSet<String>) -> Result<()> {
        let current: BTreeSet<String> = get_by_manga_id(tx, manga_id)?.into_iter().collect();
        let removed: Vec<String> = current.difference(wanted).cloned().collect();
        for name in wanted.difference(&current) {
            insert(tx, manga_id, name)?;
        }
        remove(tx, manga_id, &removed)?;
        Ok(())
    }
}

/// Statements of `extension_repos.sq`. `upsert` and `replace` spell `website = :website`: the
/// `.sq` text `website =: website` is accepted by SQLDelight only.
pub mod extension_repos {
    use super::*;

    pub const FIND_ONE: &str = "SELECT * FROM extension_repos WHERE base_url = :base_url;";
    pub const FIND_ONE_BY_SIGNING_KEY_FINGERPRINT: &str = "SELECT * FROM extension_repos WHERE signing_key_fingerprint = :fingerprint;";
    pub const FIND_ALL: &str = "SELECT * FROM extension_repos;";
    pub const COUNT: &str = "SELECT COUNT(*) FROM extension_repos;";
    pub const INSERT: &str = "INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) VALUES (:base_url, :name, :short_name, :website, :fingerprint);";
    pub const UPSERT: &str = "INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) VALUES (:base_url, :name, :short_name, :website, :fingerprint) ON CONFLICT(base_url) DO UPDATE SET name = :name, short_name = :short_name, website = :website, signing_key_fingerprint = :fingerprint WHERE base_url = base_url;";
    pub const REPLACE: &str = "INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) VALUES (:base_url, :name, :short_name, :website, :fingerprint) ON CONFLICT(signing_key_fingerprint) DO UPDATE SET base_url = :base_url, name = :name, short_name = :short_name, website = :website WHERE signing_key_fingerprint = signing_key_fingerprint;";
    pub const DELETE: &str = "DELETE FROM extension_repos WHERE base_url = :base_url;";

    /// Which repo an insert collided with, as `CreateMangaExtensionRepo.handleInsertionError` decides it.
    #[derive(Clone, Debug, PartialEq)]
    pub enum RepoConflict {
        RepoAlreadyExists,
        DuplicateFingerprint(ExtensionRepoRow),
        Other,
    }

    /// Maps a SQLite constraint failure to [`DbError::Constraint`]; other failures stay [`DbError::Sqlite`].
    fn constraint(e: rusqlite::Error) -> DbError {
        if let rusqlite::Error::SqliteFailure(f, _) = &e {
            if f.code == ErrorCode::ConstraintViolation {
                return DbError::Constraint(e.to_string());
            }
        }
        e.into()
    }

    fn bind(r: &ExtensionRepoRow) -> [(&'static str, &dyn ToSql); 5] {
        [
            (":base_url", &r.base_url),
            (":name", &r.name),
            (":short_name", &r.short_name),
            (":website", &r.website),
            (":fingerprint", &r.signing_key_fingerprint),
        ]
    }

    pub fn find_one(c: &Connection, base_url: &str) -> Result<Option<ExtensionRepoRow>> {
        query_opt(c, FIND_ONE, &[(":base_url", &base_url)], ExtensionRepoRow::from_row)
    }

    pub fn find_one_by_signing_key_fingerprint(c: &Connection, fingerprint: &str) -> Result<Option<ExtensionRepoRow>> {
        query_opt(c, FIND_ONE_BY_SIGNING_KEY_FINGERPRINT, &[(":fingerprint", &fingerprint)], ExtensionRepoRow::from_row)
    }

    pub fn find_all(c: &Connection) -> Result<Vec<ExtensionRepoRow>> {
        query_all(c, FIND_ALL, &[], ExtensionRepoRow::from_row)
    }

    pub fn count(c: &Connection) -> Result<i64> {
        query_one(c, COUNT, &[], |r| r.get(0))
    }

    /// Fails with [`DbError::Constraint`] when `base_url` or the fingerprint already exists.
    pub fn insert(c: &Connection, r: &ExtensionRepoRow) -> DbResult<usize> {
        exec(c, INSERT, &bind(r)).map_err(constraint)
    }

    /// Inserts, or updates the row with the same `base_url`. A fingerprint held by another repo
    /// fails with [`DbError::Constraint`].
    pub fn upsert(c: &Connection, r: &ExtensionRepoRow) -> DbResult<usize> {
        exec(c, UPSERT, &bind(r)).map_err(constraint)
    }

    /// Inserts, or on a fingerprint match overwrites `base_url`, `name`, `short_name` and `website` of
    /// the existing row. A `base_url` held by a different row fails with [`DbError::Constraint`].
    pub fn replace(c: &Connection, r: &ExtensionRepoRow) -> DbResult<usize> {
        exec(c, REPLACE, &bind(r)).map_err(constraint)
    }

    pub fn delete(c: &Connection, base_url: &str) -> Result<usize> {
        exec(c, DELETE, &[(":base_url", &base_url)])
    }

    /// Classifies a failed insert of `base_url` with `fingerprint`: an existing `base_url` wins, then
    /// an existing fingerprint (returned), else [`RepoConflict::Other`].
    pub fn classify_conflict(c: &Connection, base_url: &str, fingerprint: &str) -> Result<RepoConflict> {
        if find_one(c, base_url)?.is_some() {
            return Ok(RepoConflict::RepoAlreadyExists);
        }
        Ok(match find_one_by_signing_key_fingerprint(c, fingerprint)? {
            Some(existing) => RepoConflict::DuplicateFingerprint(existing),
            None => RepoConflict::Other,
        })
    }
}

/// Statements of `sources.sq`: stub sources (names of sources whose extension is gone).
pub mod sources {
    use super::*;

    pub const FIND_ALL: &str = "SELECT * FROM sources;";
    pub const FIND_ONE: &str = "SELECT * FROM sources WHERE _id = :id;";
    pub const UPSERT: &str = "INSERT INTO sources(_id, lang, name) VALUES (:id, :lang, :name) ON CONFLICT(_id) DO UPDATE SET lang = :lang, name = :name WHERE _id = :id;";

    pub fn find_all(c: &Connection) -> Result<Vec<StubSourceRow>> {
        query_all(c, FIND_ALL, &[], StubSourceRow::from_row)
    }

    pub fn find_one(c: &Connection, id: i64) -> Result<Option<StubSourceRow>> {
        query_opt(c, FIND_ONE, &[(":id", &id)], StubSourceRow::from_row)
    }

    /// Inserts the stub, or replaces `lang` and `name` of the row with the same id.
    pub fn upsert(c: &Connection, s: &StubSourceRow) -> Result<usize> {
        exec(c, UPSERT, &[(":id", &s.id), (":lang", &s.lang), (":name", &s.name)])
    }
}

/// Clear-database action for `source_ids` (T-03a-029): deletes the manga of those sources that are
/// not in the library, then the history rows left reset, in the caller's transaction. Cascades
/// remove their chapters, tracks, category links and excluded scanlators. An empty list does nothing.
pub fn clear_non_library_by_sources(tx: &Transaction<'_>, source_ids: &[i64]) -> Result<()> {
    if source_ids.is_empty() {
        return Ok(());
    }
    mangas::delete_not_in_library_by_source_ids(tx, source_ids)?;
    history::remove_resetted(tx)?;
    Ok(())
}

/// Test rows with the same defaults the app writes.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use crate::codec::UpdateStrategy;

    pub fn manga(url: &str) -> MangaRow {
        MangaRow {
            id: 0,
            source: 1,
            url: url.into(),
            artist: None,
            author: None,
            description: Some("d".into()),
            genre: None,
            title: format!("Title {url}"),
            status: 1,
            thumbnail_url: None,
            favorite: true,
            last_update: None,
            next_update: None,
            initialized: true,
            viewer: 0,
            chapter_flags: 0,
            cover_last_modified: 0,
            date_added: 1_000,
            update_strategy: UpdateStrategy::AlwaysUpdate,
            calculate_interval: 0,
            last_modified_at: 0,
            favorite_modified_at: None,
            version: 0,
            is_syncing: false,
        }
    }

    pub fn chapter(manga_id: i64, url: &str) -> ChapterRow {
        ChapterRow {
            id: 0,
            manga_id,
            url: url.into(),
            name: format!("Chapter {url}"),
            scanlator: None,
            read: false,
            bookmark: false,
            last_page_read: 0,
            chapter_number: 1.0,
            source_order: 0,
            date_fetch: 2_000,
            date_upload: 1_500,
            last_modified_at: 0,
            version: 0,
            is_syncing: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{chapter, manga};
    use super::*;
    use crate::codec::{GenreList, UpdateStrategy};
    use crate::flags::library_sort;
    use crate::schema_manga::test_db;

    fn scalar(c: &Connection, sql: &str) -> i64 {
        c.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    fn insert_manga(c: &Connection, url: &str) -> i64 {
        mangas::insert(c, &manga(url)).unwrap()
    }

    fn insert_chapter(c: &Connection, manga_id: i64, url: &str) -> i64 {
        chapters::insert(c, &chapter(manga_id, url)).unwrap()
    }

    #[test]
    fn clear_non_library_by_sources_keeps_the_library_and_removes_the_rest() {
        let mut c = test_db();
        let row = |url: &str, source: i64, favorite: bool| MangaRow { source, favorite, ..manga(url) };
        let keep = mangas::insert(&c, &row("/keep", 2, true)).unwrap();
        let gone = mangas::insert(&c, &row("/gone", 2, false)).unwrap();
        let other = mangas::insert(&c, &row("/other", 3, false)).unwrap();
        let gone_ch = insert_chapter(&c, gone, "/gone/1");
        insert_chapter(&c, keep, "/keep/1");
        insert_chapter(&c, other, "/other/1");
        history::upsert(&c, gone_ch, 1, 1).unwrap();
        assert_eq!(
            mangas::get_source_ids_with_non_library_manga(&c).unwrap(),
            vec![(2, 1), (3, 1)]
        );
        let tx = c.transaction().unwrap();
        clear_non_library_by_sources(&tx, &[]).unwrap();
        assert_eq!(scalar(&tx, "SELECT count(*) FROM mangas"), 3, "an empty list is a no-op");
        clear_non_library_by_sources(&tx, &[2]).unwrap();
        tx.commit().unwrap();
        let urls: Vec<String> = mangas::get_all(&c).unwrap().into_iter().map(|m| m.url).collect();
        assert_eq!(urls, ["/keep", "/other"]);
        assert_eq!(scalar(&c, "SELECT count(*) FROM chapters"), 2);
        assert_eq!(scalar(&c, "SELECT count(*) FROM history"), 0);
    }

    #[test]
    fn mangas_insert_then_get_round_trips_every_column() {
        let c = test_db();
        let mut m = manga("/a");
        m.genre = Some(GenreList(vec!["Action".into(), "Drama".into()]));
        m.artist = Some("art".into());
        m.update_strategy = UpdateStrategy::OnlyFetchOnce;
        m.version = 5;
        m.last_update = Some(10);
        m.status = 99;
        let id = mangas::insert(&c, &m).unwrap();
        assert_eq!(mangas::last_inserted_row_id(&c).unwrap(), id);
        let got = mangas::get_by_id(&c, id).unwrap();
        // Written by SQL, not by the struct: last_modified_at 0, NULL favorite time, not syncing.
        assert_eq!(
            got,
            MangaRow { id, last_modified_at: 0, favorite_modified_at: None, is_syncing: false, ..m }
        );
    }

    #[test]
    fn mangas_missing_id_is_no_rows() {
        let c = test_db();
        assert!(matches!(mangas::get_by_id(&c, 99), Err(DbError::NotFound)));
        assert!(mangas::get_by_url_and_source(&c, "/none", 1).unwrap().is_none());
    }

    #[test]
    fn mangas_url_and_source_returns_lowest_rowid_on_duplicates() {
        let c = test_db();
        let first = insert_manga(&c, "/x");
        insert_manga(&c, "/x");
        let got = mangas::get_by_url_and_source(&c, "/x", 1).unwrap();
        assert_eq!(got.map(|m| m.id), Some(first));
    }

    #[test]
    fn mangas_update_is_coalesce_and_cannot_clear() {
        let c = test_db();
        let id = insert_manga(&c, "/u");
        let patch = MangaPatch {
            id,
            title: Some("New".into()),
            is_syncing: Some(false),
            update_strategy: Some(UpdateStrategy::OnlyFetchOnce),
            ..Default::default()
        };
        assert_eq!(mangas::update(&c, &patch).unwrap(), 1);
        let got = mangas::get_by_id(&c, id).unwrap();
        assert_eq!(got.title, "New");
        assert_eq!(got.update_strategy, UpdateStrategy::OnlyFetchOnce);
        assert_eq!(got.url, "/u");
        assert_eq!(got.description.as_deref(), Some("d"), "None must not clear");
    }

    #[test]
    fn mangas_empty_patch_runs_no_statement() {
        let c = test_db();
        let id = insert_manga(&c, "/e");
        assert_eq!(mangas::update(&c, &MangaPatch { id, ..Default::default() }).unwrap(), 0);
        // A real update would fire the last_modified_at trigger; the empty patch must not.
        assert_eq!(mangas::get_by_id(&c, id).unwrap().last_modified_at, 0);
    }

    #[test]
    fn mangas_update_of_missing_row_changes_nothing() {
        let c = test_db();
        let patch = MangaPatch { id: 42, title: Some("x".into()), ..Default::default() };
        assert_eq!(mangas::update(&c, &patch).unwrap(), 0);
    }

    #[test]
    fn favorite_counts_per_source_come_back_ordered_by_source() {
        let c = test_db();
        let mut later = manga("/later");
        later.source = 2;
        mangas::insert(&c, &later).unwrap();
        insert_manga(&c, "/first");
        assert_eq!(mangas::get_source_id_with_favorite_count(&c).unwrap(), [(1, 1), (2, 1)]);
    }

    #[test]
    fn mangas_favorites_and_counts_per_source() {
        let c = test_db();
        insert_manga(&c, "/f1");
        insert_manga(&c, "/f2");
        let mut other = manga("/f3");
        other.source = 2;
        mangas::insert(&c, &other).unwrap();
        let mut not_fav = manga("/n");
        not_fav.favorite = false;
        mangas::insert(&c, &not_fav).unwrap();

        assert_eq!(mangas::get_favorites(&c).unwrap().len(), 3);
        assert_eq!(mangas::get_favorite_by_source_id(&c, 1).unwrap().len(), 2);
        assert_eq!(mangas::get_source_id_with_favorite_count(&c).unwrap(), [(1, 2), (2, 1)]);
        assert_eq!(mangas::get_source_ids_with_non_library_manga(&c).unwrap(), [(1, 1)]);
        assert_eq!(mangas::get_all(&c).unwrap().len(), 4);
        assert_eq!(mangas::get_all_source_and_url(&c).unwrap().len(), 4);
        assert!(mangas::get_with_favorite_timestamp(&c).unwrap().is_empty());
    }

    #[test]
    fn mangas_read_not_in_library_needs_read_or_progress() {
        let c = test_db();
        let read = insert_manga(&c, "/r");
        let progress = insert_manga(&c, "/p");
        let untouched = insert_manga(&c, "/u");
        for id in [read, progress, untouched] {
            c.execute("UPDATE mangas SET favorite = 0 WHERE _id = ?1", [id]).unwrap();
        }
        let ch = insert_chapter(&c, read, "/r1");
        c.execute("UPDATE chapters SET read = 1 WHERE _id = ?1", [ch]).unwrap();
        let ch = insert_chapter(&c, progress, "/p1");
        c.execute("UPDATE chapters SET last_page_read = 3 WHERE _id = ?1", [ch]).unwrap();
        insert_chapter(&c, untouched, "/u1");
        let ids: Vec<i64> = mangas::get_read_not_in_library(&c).unwrap().iter().map(|m| m.id).collect();
        assert_eq!(ids, [read, progress]);
    }

    #[test]
    fn mangas_upcoming_filters_status_and_day_and_orders_ascending() {
        let c = test_db();
        let at = |url: &str, status: i64, next: i64| {
            let mut m = manga(url);
            m.status = status;
            m.next_update = Some(next);
            mangas::insert(&c, &m).unwrap()
        };
        let ongoing_late = at("/a", 1, 120);
        let finished = at("/b", 4, 110);
        at("/c", 2, 130);
        at("/d", 1, 50);
        let got = mangas::get_upcoming_manga(&c, 100, &mangas::UPCOMING_STATUSES).unwrap();
        assert_eq!(got.iter().map(|m| m.id).collect::<Vec<_>>(), [finished, ongoing_late]);
        assert!(mangas::get_upcoming_manga(&c, 100, &[]).unwrap().is_empty());
    }

    #[test]
    fn mangas_duplicate_lookup_excludes_self_and_non_library() {
        let c = test_db();
        let original = insert_manga(&c, "/one");
        let mut dup = manga("/two");
        dup.title = "Same Title".into();
        let dup_id = mangas::insert(&c, &dup).unwrap();
        let mut hidden = dup.clone();
        hidden.favorite = false;
        mangas::insert(&c, &hidden).unwrap();
        // The caller lowercases the title; the match is on LOWER(title).
        let found = mangas::get_duplicate_library_manga(&c, "same title", original).unwrap();
        assert_eq!(found.iter().map(|m| m.id).collect::<Vec<_>>(), [dup_id]);
        assert!(mangas::get_duplicate_library_manga(&c, "same title", dup_id).unwrap().is_empty());
    }

    #[test]
    fn mangas_delete_non_library_by_source_ids() {
        let c = test_db();
        insert_manga(&c, "/keep");
        let mut gone = manga("/gone");
        gone.favorite = false;
        mangas::insert(&c, &gone).unwrap();
        let mut other_source = manga("/other");
        other_source.favorite = false;
        other_source.source = 2;
        mangas::insert(&c, &other_source).unwrap();

        assert_eq!(mangas::delete_not_in_library_by_source_ids(&c, &[]).unwrap(), 0);
        assert_eq!(mangas::delete_not_in_library_by_source_ids(&c, &[1]).unwrap(), 1);
        assert_eq!(scalar(&c, "SELECT count(*) FROM mangas"), 2);
        assert_eq!(scalar(&c, "SELECT count(*) FROM mangas WHERE url = '/gone'"), 0);
    }

    #[test]
    fn mangas_reset_viewer_flags_and_is_syncing() {
        let c = test_db();
        let id = insert_manga(&c, "/v");
        mangas::update(&c, &MangaPatch { id, viewer: Some(7), ..Default::default() }).unwrap();
        mangas::update(&c, &MangaPatch { id, is_syncing: Some(true), ..Default::default() }).unwrap();
        assert_eq!(mangas::reset_is_syncing(&c).unwrap(), 1);
        assert_eq!(mangas::reset_is_syncing(&c).unwrap(), 0);

        assert_eq!(mangas::reset_viewer_flags(&c).unwrap(), 1);
        let got = mangas::get_by_id(&c, id).unwrap();
        assert_eq!(got.viewer, 0);
        assert!(got.last_modified_at > 0, "reset rewrites last_modified_at through the trigger");
    }

    #[test]
    fn chapters_insert_get_update_round_trip() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let id = insert_chapter(&c, m, "/c1");
        assert_eq!(chapters::last_inserted_row_id(&c).unwrap(), id);
        let got = chapters::get_by_id(&c, id).unwrap();
        assert_eq!(got, ChapterRow { id, ..chapter(m, "/c1") });

        let patch = ChapterPatch {
            id,
            read: Some(true),
            scanlator: Some("Group".into()),
            chapter_number: Some(2.5),
            ..Default::default()
        };
        assert_eq!(chapters::update(&c, &patch).unwrap(), 1);
        let got = chapters::get_by_id(&c, id).unwrap();
        assert!(got.read);
        assert_eq!(got.scanlator.as_deref(), Some("Group"));
        assert_eq!(got.chapter_number, 2.5);
        assert_eq!(got.url, "/c1", "None must not clear");
        assert_eq!(chapters::update(&c, &ChapterPatch { id, ..Default::default() }).unwrap(), 0);
        assert!(matches!(chapters::get_by_id(&c, 999), Err(DbError::NotFound)));
    }

    #[test]
    fn chapters_scanlator_filter_keeps_null_and_drops_excluded() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        for (url, scanlator) in [("/n", None), ("/g", Some("G")), ("/h", Some("H"))] {
            let mut ch = chapter(m, url);
            ch.scanlator = scanlator.map(str::to_owned);
            chapters::insert(&c, &ch).unwrap();
        }
        c.execute("INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (?1, 'G')", [m]).unwrap();
        assert_eq!(chapters::get_by_manga_id(&c, m, false).unwrap().len(), 3);
        let filtered: Vec<String> = chapters::get_by_manga_id(&c, m, true)
            .unwrap()
            .into_iter()
            .map(|r| r.url)
            .collect();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&"/n".to_owned()) && filtered.contains(&"/h".to_owned()));
    }

    #[test]
    fn chapters_scanlators_are_not_distinct_and_null_is_blank() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        for (url, s) in [("/a", None), ("/b", Some("G")), ("/c", Some("G"))] {
            let mut ch = chapter(m, url);
            ch.scanlator = s.map(str::to_owned);
            chapters::insert(&c, &ch).unwrap();
        }
        let mut got = chapters::get_scanlators_by_manga_id(&c, m).unwrap();
        got.sort();
        assert_eq!(got, ["", "G", "G"]);
    }

    #[test]
    fn chapters_bookmarked_and_url_lookups() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let id = insert_chapter(&c, m, "/c1");
        insert_chapter(&c, m, "/c2");
        chapters::update(&c, &ChapterPatch { id, bookmark: Some(true), ..Default::default() }).unwrap();
        let bookmarked = chapters::get_bookmarked_by_manga_id(&c, m).unwrap();
        assert_eq!(bookmarked.iter().map(|r| r.id).collect::<Vec<_>>(), [id]);
        assert_eq!(chapters::get_by_url(&c, "/c2").unwrap().map(|r| r.url), Some("/c2".into()));
        assert!(chapters::get_by_url(&c, "/nope").unwrap().is_none());
        assert_eq!(chapters::get_by_url_and_manga_id(&c, "/c1", m).unwrap().map(|r| r.id), Some(id));
        assert!(chapters::get_by_url_and_manga_id(&c, "/c1", 999).unwrap().is_none());
    }

    #[test]
    fn chapters_remove_forty_thousand_in_one_statement() {
        let mut c = test_db();
        let m = insert_manga(&c, "/m");
        let tx = c.transaction().unwrap();
        let ids: Vec<i64> = (0..40_000).map(|i| insert_chapter(&tx, m, &format!("/c{i}"))).collect();
        tx.commit().unwrap();
        assert_eq!(chapters::remove_with_ids(&c, &[]).unwrap(), 0);
        assert_eq!(chapters::remove_with_ids(&c, &ids).unwrap(), 40_000);
        assert_eq!(scalar(&c, "SELECT count(*) FROM chapters"), 0);
    }

    #[test]
    fn chapters_insert_all_rolls_back_on_error() {
        let mut c = test_db();
        let m = insert_manga(&c, "/m");
        let tx = c.transaction().unwrap();
        let rows = [chapter(m, "/ok"), chapter(999, "/orphan")];
        assert!(chapters::insert_all(&tx, &rows).is_err());
        drop(tx);
        assert_eq!(scalar(&c, "SELECT count(*) FROM chapters"), 0);
    }

    #[test]
    fn chapters_reset_is_syncing() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let id = insert_chapter(&c, m, "/c1");
        c.execute("UPDATE chapters SET is_syncing = 1 WHERE _id = ?1", [id]).unwrap();
        assert_eq!(chapters::reset_is_syncing(&c).unwrap(), 1);
        assert!(!chapters::get_by_id(&c, id).unwrap().is_syncing);
    }

    #[test]
    fn categories_insert_gives_first_user_id_one() {
        let c = test_db();
        let id = categories::insert(&c, "Reading", 0, library_sort::DEFAULT).unwrap();
        assert_eq!(id, 1);
        let got = categories::get_category(&c, id).unwrap().unwrap();
        assert_eq!(got, CategoryRow { id: 1, name: "Reading".into(), order: 0, flags: 0x40, hidden: false });
        assert!(categories::get_category(&c, 77).unwrap().is_none());
    }

    #[test]
    fn categories_list_is_ordered_by_sort_then_id() {
        let c = test_db();
        categories::insert(&c, "b", 2, 0).unwrap();
        categories::insert(&c, "a", 1, 0).unwrap();
        let ids: Vec<i64> = categories::get_categories(&c).unwrap().iter().map(|r| r.id).collect();
        assert_eq!(ids, [0, 2, 1]);
    }

    #[test]
    fn categories_partial_update_and_visibility() {
        let c = test_db();
        let id = categories::insert(&c, "a", 0, 5).unwrap();
        let patch = CategoryPatch { id, name: Some("z".into()), ..Default::default() };
        assert_eq!(categories::update(&c, &patch).unwrap(), 1);
        let got = categories::get_category(&c, id).unwrap().unwrap();
        assert_eq!((got.name.as_str(), got.flags), ("z", 5));

        categories::update(&c, &CategoryPatch { id, hidden: Some(true), ..Default::default() }).unwrap();
        let visible: Vec<i64> = categories::get_visible_categories(&c).unwrap().iter().map(|r| r.id).collect();
        assert_eq!(visible, [0]);
    }

    #[test]
    fn categories_delete_user_row_and_refuse_system_row() {
        let c = test_db();
        let id = categories::insert(&c, "a", 0, 0).unwrap();
        let err = categories::delete(&c, 0).unwrap_err();
        assert!(err.to_string().contains("System category can't be deleted"), "{err}");
        assert_eq!(categories::delete(&c, id).unwrap(), 1);
        assert!(categories::get_category(&c, id).unwrap().is_none());
    }

    #[test]
    fn categories_update_all_flags_none_keeps_values_some_sets_all() {
        let c = test_db();
        let id = categories::insert(&c, "a", 0, 5).unwrap();
        categories::update_all_flags(&c, None).unwrap();
        assert_eq!(categories::get_category(&c, id).unwrap().unwrap().flags, 5);
        assert_eq!(categories::get_category(&c, 0).unwrap().unwrap().flags, 0, "the seed row keeps its flags");
        categories::update_all_flags(&c, Some(64)).unwrap();
        assert_eq!(categories::get_category(&c, id).unwrap().unwrap().flags, 64);
        assert_eq!(categories::get_category(&c, 0).unwrap().unwrap().flags, 64, "system row included");
    }

    #[test]
    fn categories_by_manga_lists_only_assigned_and_visible() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        for name in ["a", "b", "c"] {
            categories::insert(&c, name, 0, 0).unwrap();
        }
        mangas_categories::insert(&c, m, 1).unwrap();
        mangas_categories::insert(&c, m, 3).unwrap();
        let ids = |rows: Vec<CategoryRow>| rows.iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(ids(categories::get_categories_by_manga_id(&c, m).unwrap()), [1, 3]);
        categories::update(&c, &CategoryPatch { id: 3, hidden: Some(true), ..Default::default() }).unwrap();
        assert_eq!(ids(categories::get_visible_categories_by_manga_id(&c, m).unwrap()), [1]);
    }

    #[test]
    fn categories_renumber_all_numbers_the_system_row_too() {
        let mut c = test_db();
        categories::insert(&c, "x", 5, 0).unwrap();
        categories::insert(&c, "y", 3, 0).unwrap();
        categories::insert(&c, "z", 4, 0).unwrap();
        let tx = c.transaction().unwrap();
        categories::renumber_all(&tx).unwrap();
        tx.commit().unwrap();
        let orders: Vec<(i64, i64)> = categories::get_categories(&c).unwrap().iter().map(|r| (r.id, r.order)).collect();
        assert_eq!(orders, [(0, 0), (2, 1), (3, 2), (1, 3)], "system row first (sort -1), then every row numbered by index");
    }

    #[test]
    fn mangas_categories_insert_and_delete_by_manga() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        categories::insert(&c, "a", 0, 0).unwrap();
        categories::insert(&c, "b", 1, 0).unwrap();
        assert_eq!(mangas_categories::insert(&c, m, 1).unwrap(), 1);
        mangas_categories::insert(&c, m, 2).unwrap();
        assert_eq!(scalar(&c, "SELECT count(*) FROM mangas_categories"), 2);
        assert_eq!(mangas_categories::delete_by_manga_id(&c, m).unwrap(), 2);
        assert_eq!(scalar(&c, "SELECT count(*) FROM mangas_categories"), 0);
    }

    #[test]
    fn mangas_categories_set_dedupes_and_empty_clears() {
        let mut c = test_db();
        let m = insert_manga(&c, "/m");
        for name in ["a", "b"] {
            categories::insert(&c, name, 0, 0).unwrap();
        }
        let tx = c.transaction().unwrap();
        mangas_categories::set_categories(&tx, m, &[2, 2, 1]).unwrap();
        tx.commit().unwrap();
        assert_eq!(scalar(&c, "SELECT count(*) FROM mangas_categories"), 2);

        let tx = c.transaction().unwrap();
        mangas_categories::set_categories(&tx, m, &[]).unwrap();
        tx.commit().unwrap();
        assert_eq!(scalar(&c, "SELECT count(*) FROM mangas_categories"), 0);
    }

    #[test]
    fn history_upsert_sets_last_read_and_adds_delta() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let ch = insert_chapter(&c, m, "/c1");
        history::upsert(&c, ch, 100, 7).unwrap();
        history::upsert(&c, ch, 200, 7).unwrap();
        let rows = history::get_by_manga_id(&c, m).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].chapter_id, rows[0].last_read, rows[0].time_read), (ch, 200, 14));
        let by_url = history::get_by_chapter_url(&c, "/c1").unwrap().unwrap();
        assert_eq!(by_url.time_read, 14);
        assert!(history::get_by_chapter_url(&c, "/none").unwrap().is_none());
    }

    #[test]
    fn history_reset_keeps_row_and_remove_resetted_deletes_only_reset() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let a = insert_chapter(&c, m, "/a");
        let b = insert_chapter(&c, m, "/b");
        history::upsert(&c, a, 100, 5).unwrap();
        history::upsert(&c, b, 200, 5).unwrap();
        let a_row = history::get_by_chapter_url(&c, "/a").unwrap().unwrap();
        assert_eq!(history::reset_by_id(&c, a_row.id).unwrap(), 1);
        let kept = history::get_by_chapter_url(&c, "/a").unwrap().unwrap();
        assert_eq!((kept.last_read, kept.time_read), (0, 5));
        assert_eq!(history::remove_resetted(&c).unwrap(), 1);
        assert_eq!(scalar(&c, "SELECT count(*) FROM history"), 1);
        assert_eq!(history::reset_by_manga_id(&c, m).unwrap(), 1);
        assert_eq!(history::remove_resetted(&c).unwrap(), 1);
        assert_eq!(scalar(&c, "SELECT count(*) FROM history"), 0);
    }

    #[test]
    fn history_read_duration_and_remove_all() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        assert_eq!(history::get_read_duration(&c).unwrap(), 0);
        history::upsert(&c, insert_chapter(&c, m, "/a"), 1, 7).unwrap();
        history::upsert(&c, insert_chapter(&c, m, "/b"), 1, 5).unwrap();
        assert_eq!(history::get_read_duration(&c).unwrap(), 12);
        assert_eq!(history::remove_all(&c).unwrap(), 2);
        assert_eq!(history::get_read_duration(&c).unwrap(), 0);
    }

    #[test]
    fn history_row_goes_with_its_chapter() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let ch = insert_chapter(&c, m, "/a");
        history::upsert(&c, ch, 1, 1).unwrap();
        chapters::remove_with_ids(&c, &[ch]).unwrap();
        assert_eq!(scalar(&c, "SELECT count(*) FROM history"), 0);
    }

    #[test]
    fn library_view_counts_reads_bookmarks_and_latest_values() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let c1 = insert_chapter(&c, m, "/c1");
        let c2 = insert_chapter(&c, m, "/c2");
        let c3 = insert_chapter(&c, m, "/c3");
        c.execute("UPDATE chapters SET read = 1, date_upload = 10 WHERE _id = ?1", [c1]).unwrap();
        c.execute("UPDATE chapters SET date_upload = 20 WHERE _id = ?1", [c3]).unwrap();
        c.execute("UPDATE chapters SET read = 1, bookmark = 1, date_upload = 30 WHERE _id = ?1", [c2]).unwrap();
        history::upsert(&c, c2, 500, 1).unwrap();

        let rows = views::library(&c).unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.manga.id, m);
        assert_eq!((row.total_count, row.read_count, row.bookmark_count), (3, 2, 1));
        assert_eq!((row.last_read, row.latest_upload, row.chapter_fetched_at), (500, 30, 2_000));
        assert_eq!(row.category, 0);
    }

    #[test]
    fn library_view_one_row_per_category_link_and_excludes_non_favourites() {
        let c = test_db();
        let m1 = insert_manga(&c, "/m1");
        let m2 = insert_manga(&c, "/m2");
        let mut hidden = manga("/hidden");
        hidden.favorite = false;
        mangas::insert(&c, &hidden).unwrap();
        categories::insert(&c, "a", 0, 0).unwrap();
        categories::insert(&c, "b", 1, 0).unwrap();
        mangas_categories::insert(&c, m1, 1).unwrap();
        mangas_categories::insert(&c, m1, 2).unwrap();

        let rows = views::library(&c).unwrap();
        assert_eq!(rows.len(), 3, "two links for m1, one unlinked row for m2");
        let grouped = views::group_by_category(rows);
        assert_eq!(grouped.keys().copied().collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(grouped[&0][0].manga.id, m2);
        assert_eq!(grouped[&1][0].manga.id, m1);
    }

    #[test]
    fn library_view_excluded_scanlators_are_not_counted() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        for (url, s) in [("/n", None), ("/g", Some("G"))] {
            let mut ch = chapter(m, url);
            ch.scanlator = s.map(str::to_owned);
            chapters::insert(&c, &ch).unwrap();
        }
        c.execute("INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (?1, 'G')", [m]).unwrap();
        assert_eq!(views::library(&c).unwrap()[0].total_count, 1, "NULL scanlator still counted");

        let all_excluded = insert_manga(&c, "/all");
        let mut g = chapter(all_excluded, "/g");
        g.scanlator = Some("G".into());
        chapters::insert(&c, &g).unwrap();
        c.execute("INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (?1, 'G')", [all_excluded]).unwrap();
        let rows = views::library(&c).unwrap();
        let row = rows.iter().find(|r| r.manga.id == all_excluded).expect("manga stays listed");
        assert_eq!((row.total_count, row.read_count), (0, 0));
    }

    #[test]
    fn history_view_latest_per_manga_newest_first_and_search() {
        let c = test_db();
        let a = insert_manga(&c, "/a");
        c.execute("UPDATE mangas SET title = 'Alpha 100%' WHERE _id = ?1", [a]).unwrap();
        let a1 = insert_chapter(&c, a, "/a1");
        let a2 = insert_chapter(&c, a, "/a2");
        history::upsert(&c, a1, 100, 1).unwrap();
        history::upsert(&c, a2, 300, 1).unwrap();
        let b = insert_manga(&c, "/b");
        c.execute("UPDATE mangas SET title = 'Beta_gamma' WHERE _id = ?1", [b]).unwrap();
        let b1 = insert_chapter(&c, b, "/b1");
        history::upsert(&c, b1, 200, 1).unwrap();
        let unread = insert_manga(&c, "/c");
        insert_chapter(&c, unread, "/c1");

        let chapter_ids = |rows: Vec<HistoryViewRow>| rows.iter().map(|r| r.chapter_id).collect::<Vec<_>>();
        assert_eq!(chapter_ids(views::history(&c, "").unwrap()), [a2, b1]);
        assert_eq!(chapter_ids(views::history(&c, "ALPHA").unwrap()), [a2], "case-insensitive");
        assert_eq!(chapter_ids(views::history(&c, "100%").unwrap()), [a2], "% is literal");
        assert_eq!(chapter_ids(views::history(&c, "a%").unwrap()), Vec::<i64>::new());
        assert_eq!(chapter_ids(views::history(&c, "_").unwrap()), [b1], "_ is literal");

        let latest = views::get_latest_history(&c).unwrap().unwrap();
        assert_eq!((latest.chapter_id, latest.read_at), (a2, 300));
    }

    #[test]
    fn history_view_latest_is_none_without_reads() {
        let c = test_db();
        assert!(views::get_latest_history(&c).unwrap().is_none());
    }

    #[test]
    fn history_view_leaves_out_a_manga_whose_only_read_was_reset() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let ch = insert_chapter(&c, m, "/m1");
        history::upsert(&c, ch, 500, 1).unwrap();
        let row = history::get_by_manga_id(&c, m).unwrap().remove(0);
        assert_eq!(history::reset_by_id(&c, row.id).unwrap(), 1);

        assert!(views::history(&c, "").unwrap().is_empty());
        assert!(views::get_latest_history(&c).unwrap().is_none());
    }

    #[test]
    fn history_view_shows_the_read_chapter_of_a_partly_reset_manga() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let reset_ch = insert_chapter(&c, m, "/m1");
        let read_ch = insert_chapter(&c, m, "/m2");
        history::upsert(&c, reset_ch, 400, 1).unwrap();
        history::upsert(&c, read_ch, 300, 1).unwrap();
        let reset_row = history::get_by_manga_id(&c, m)
            .unwrap()
            .into_iter()
            .find(|r| r.chapter_id == reset_ch)
            .unwrap();
        history::reset_by_id(&c, reset_row.id).unwrap();

        let rows = views::history(&c, "").unwrap();
        assert_eq!(rows.iter().map(|r| r.chapter_id).collect::<Vec<_>>(), [read_ch]);
        let latest = views::get_latest_history(&c).unwrap().unwrap();
        assert_eq!((latest.chapter_id, latest.read_at), (read_ch, 300));
    }

    #[test]
    fn escape_like_escapes_only_the_three_metacharacters() {
        assert_eq!(views::escape_like(r"a%b_c\d"), r"a\%b\_c\\d");
        assert_eq!(views::escape_like("plain Ärger"), "plain Ärger");
    }

    fn track_row(manga_id: i64, sync_id: i64) -> TrackRow {
        TrackRow {
            id: 0,
            manga_id,
            sync_id,
            remote_id: 42,
            library_id: Some(7),
            title: "Remote title".into(),
            last_chapter_read: 3.5,
            total_chapters: 20,
            status: 1,
            score: 8.5,
            remote_url: "https://example.test/42".into(),
            start_date: 100,
            finish_date: 0,
            private: false,
        }
    }

    #[test]
    fn manga_sync_round_trips_every_column_and_private_as_bool() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let id = manga_sync::insert(&c, &TrackRow { private: true, ..track_row(m, manga_sync::ANILIST) }).unwrap();
        let got = manga_sync::get_track_by_id(&c, id).unwrap();
        assert_eq!(got, TrackRow { id, private: true, ..track_row(m, manga_sync::ANILIST) });
        assert_eq!(manga_sync::get_tracks_by_manga_id(&c, m).unwrap(), std::slice::from_ref(&got));
        assert_eq!(manga_sync::get_tracks(&c).unwrap(), [got]);
        let kid = manga_sync::insert(&c, &TrackRow { library_id: None, ..track_row(m, manga_sync::KITSU) }).unwrap();
        assert_eq!(manga_sync::get_track_by_id(&c, kid).unwrap().library_id, None);
        assert!(matches!(manga_sync::get_track_by_id(&c, 999), Err(DbError::NotFound)));
    }

    #[test]
    fn a_second_binding_for_the_same_tracker_replaces_the_first() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let first = manga_sync::insert(&c, &track_row(m, manga_sync::MYANIMELIST)).unwrap();
        let second = manga_sync::insert(&c, &TrackRow { score: 1.0, ..track_row(m, manga_sync::MYANIMELIST) }).unwrap();
        assert_ne!(first, second, "TC-S01: the replacement has a new _id");
        let rows = manga_sync::get_tracks_by_manga_id(&c, m).unwrap();
        assert_eq!((rows.len(), rows[0].id, rows[0].score), (1, second, 1.0));
        assert!(matches!(manga_sync::get_track_by_id(&c, first), Err(DbError::NotFound)));
    }

    #[test]
    fn manga_sync_update_keeps_unset_columns_and_cannot_clear_library_id() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let id = manga_sync::insert(&c, &track_row(m, manga_sync::ANILIST)).unwrap();
        let patch = TrackPatch { id, title: Some("New".into()), score: Some(9.0), ..Default::default() };
        assert_eq!(manga_sync::update(&c, &patch).unwrap(), 1);
        let got = manga_sync::get_track_by_id(&c, id).unwrap();
        assert_eq!((got.title.as_str(), got.score, got.library_id, got.remote_id), ("New", 9.0, Some(7), 42));
        manga_sync::update(&c, &TrackPatch { id, library_id: Some(8), private: Some(true), ..Default::default() }).unwrap();
        let got = manga_sync::get_track_by_id(&c, id).unwrap();
        assert_eq!((got.library_id, got.private, got.title.as_str()), (Some(8), true, "New"));
    }

    #[test]
    fn manga_sync_delete_by_tracker_and_cascade_with_the_manga() {
        let c = test_db();
        let a = insert_manga(&c, "/a");
        let b = insert_manga(&c, "/b");
        manga_sync::insert(&c, &track_row(a, manga_sync::ANILIST)).unwrap();
        manga_sync::insert(&c, &track_row(a, manga_sync::KITSU)).unwrap();
        manga_sync::insert(&c, &track_row(b, manga_sync::ANILIST)).unwrap();
        assert_eq!(manga_sync::delete(&c, a, manga_sync::KITSU).unwrap(), 1);
        assert_eq!(manga_sync::get_tracks(&c).unwrap().len(), 2);
        c.execute("DELETE FROM mangas WHERE _id = ?1", [a]).unwrap();
        let left: Vec<i64> = manga_sync::get_tracks(&c).unwrap().iter().map(|t| t.manga_id).collect();
        assert_eq!(left, [b], "TC-S03: deleting the manga removes its tracks");
    }

    #[test]
    fn excluded_scanlators_insert_get_and_remove() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        excluded_scanlators::insert(&c, m, "A").unwrap();
        excluded_scanlators::insert(&c, m, "B").unwrap();
        assert_eq!(excluded_scanlators::get_by_manga_id(&c, m).unwrap(), ["A", "B"]);
        assert_eq!(excluded_scanlators::remove(&c, m, &[]).unwrap(), 0, "an empty set is a no-op");
        assert_eq!(excluded_scanlators::remove(&c, m, &["A".to_string()]).unwrap(), 1);
        assert_eq!(excluded_scanlators::get_by_manga_id(&c, m).unwrap(), ["B"]);
    }

    #[test]
    fn set_excluded_diffs_the_stored_set_and_repeats_without_duplicates() {
        let mut c = test_db();
        let m = insert_manga(&c, "/m");
        let set = |names: &[&str]| names.iter().map(|s| s.to_string()).collect::<BTreeSet<String>>();
        let tx = c.transaction().unwrap();
        excluded_scanlators::set_excluded(&tx, m, &set(&["A", "B"])).unwrap();
        excluded_scanlators::set_excluded(&tx, m, &set(&["A", "B"])).unwrap();
        excluded_scanlators::set_excluded(&tx, m, &set(&["B", "C"])).unwrap();
        tx.commit().unwrap();
        let mut stored = excluded_scanlators::get_by_manga_id(&c, m).unwrap();
        stored.sort();
        assert_eq!(stored, ["B", "C"]);
        let tx = c.transaction().unwrap();
        excluded_scanlators::set_excluded(&tx, m, &BTreeSet::new()).unwrap();
        tx.commit().unwrap();
        assert!(excluded_scanlators::get_by_manga_id(&c, m).unwrap().is_empty());
    }

    #[test]
    fn excluding_a_scanlator_drops_its_chapters_from_the_library_counts() {
        let mut c = test_db();
        let m = mangas::insert(&c, &manga("/m")).unwrap();
        let in_scanlator = |url: &str, scanlator: &str| ChapterRow { scanlator: Some(scanlator.into()), ..chapter(m, url) };
        chapters::insert(&c, &in_scanlator("/1", "A")).unwrap();
        chapters::insert(&c, &in_scanlator("/2", "A")).unwrap();
        chapters::insert(&c, &in_scanlator("/3", "B")).unwrap();
        let total = |c: &Connection| views::library(c).unwrap()[0].total_count;
        assert_eq!(total(&c), 3);
        let tx = c.transaction().unwrap();
        excluded_scanlators::set_excluded(&tx, m, &BTreeSet::from(["A".to_string()])).unwrap();
        tx.commit().unwrap();
        assert_eq!(total(&c), 1, "TC-L05: excluded chapters leave the count");
    }

    #[test]
    fn updates_filter_on_upload_date_and_order_by_fetch_newest_first() {
        let c = test_db();
        let m = insert_manga(&c, "/m");
        let hidden = insert_manga(&c, "/hidden");
        c.execute("UPDATE mangas SET favorite = 0 WHERE _id = ?1", [hidden]).unwrap();
        let upd = |manga_id: i64, url: &str, date_fetch: i64, date_upload: i64, read: bool| {
            chapters::insert(&c, &ChapterRow { date_fetch, date_upload, read, ..chapter(manga_id, url) }).unwrap()
        };
        upd(m, "/fetched-before-added", 500, 1_500, false);
        upd(m, "/old-upload", 3_000, 900, false);
        upd(m, "/a", 2_000, 1_500, false);
        upd(m, "/b", 3_000, 1_600, true);
        upd(m, "/c", 2_500, 1_700, false);
        upd(hidden, "/not-a-favourite", 9_000, 9_000, false);
        let fetched = |rows: Vec<UpdateViewRow>| rows.iter().map(|r| r.date_fetch).collect::<Vec<_>>();
        let recent = views::get_recent_updates(&c, 1_000, views::UPDATES_PAGE_LIMIT).unwrap();
        assert_eq!(fetched(recent.clone()), [3_000, 2_500, 2_000], "TC-U01..U03: fetch order, newest first");
        assert_eq!((recent[0].date_upload, recent[0].manga_id), (1_600, m), "datefetch and dateUpload map to the row");
        assert_eq!(fetched(views::get_recent_updates(&c, 1_000, 2).unwrap()), [3_000, 2_500], "LIMIT keeps the view order");
        let read = views::get_updates_by_read_status(&c, true, 1_000, 500).unwrap();
        assert_eq!(fetched(read), [3_000]);
        let unread = views::get_updates_by_read_status(&c, false, 1_000, 500).unwrap();
        assert_eq!(fetched(unread), [2_500, 2_000]);
    }

    #[test]
    fn sources_round_trip_and_upsert_replaces_lang_and_name() {
        let c = test_db();
        sources::upsert(&c, &StubSourceRow { id: 5, lang: "en".into(), name: "Old".into() }).unwrap();
        sources::upsert(&c, &StubSourceRow { id: 6, lang: "ja".into(), name: "Other".into() }).unwrap();
        sources::upsert(&c, &StubSourceRow { id: 5, lang: "fr".into(), name: "New".into() }).unwrap();
        assert_eq!(sources::find_one(&c, 5).unwrap(), Some(StubSourceRow { id: 5, lang: "fr".into(), name: "New".into() }), "TC-G01");
        assert_eq!(sources::find_one(&c, 9).unwrap(), None);
        let ids: Vec<i64> = sources::find_all(&c).unwrap().iter().map(|s| s.id).collect();
        assert_eq!(ids, [5, 6]);
    }

    #[test]
    fn stub_visual_name_is_name_with_upper_lang_or_the_id() {
        let row = |lang: &str, name: &str| StubSourceRow { id: 77, lang: lang.into(), name: name.into() };
        assert_eq!(row("en", "Foo").visual_name(), "Foo (EN)");
        assert_eq!(row("pt-BR", "Foo").visual_name(), "Foo (PT-BR)");
        assert_eq!(row("", "Foo").visual_name(), "77");
        assert_eq!(row("en", "  ").visual_name(), "77");
    }

    fn repo(base: &str, fingerprint: &str) -> ExtensionRepoRow {
        ExtensionRepoRow {
            base_url: base.into(),
            name: format!("Repo {base}"),
            short_name: Some("R".into()),
            website: format!("{base}/site"),
            signing_key_fingerprint: fingerprint.into(),
        }
    }

    #[test]
    fn extension_repo_find_count_delete_in_insertion_order() {
        let c = test_db();
        extension_repos::insert(&c, &repo("https://z", "f1")).unwrap();
        extension_repos::insert(&c, &repo("https://a", "f2")).unwrap();
        assert_eq!(extension_repos::find_one(&c, "https://a").unwrap(), Some(repo("https://a", "f2")));
        assert_eq!(extension_repos::find_one_by_signing_key_fingerprint(&c, "f1").unwrap(), Some(repo("https://z", "f1")));
        assert_eq!(extension_repos::find_one(&c, "https://none").unwrap(), None);
        assert_eq!(extension_repos::count(&c).unwrap(), 2);
        let order: Vec<String> = extension_repos::find_all(&c).unwrap().into_iter().map(|r| r.base_url).collect();
        assert_eq!(order, ["https://z", "https://a"], "insertion order, not base_url order");
        assert_eq!(extension_repos::delete(&c, "https://z").unwrap(), 1);
        assert_eq!(extension_repos::count(&c).unwrap(), 1);
    }

    #[test]
    fn extension_repo_insert_conflicts_map_to_constraint() {
        let c = test_db();
        extension_repos::insert(&c, &repo("https://a", "f1")).unwrap();
        assert!(matches!(extension_repos::insert(&c, &repo("https://a", "f9")), Err(DbError::Constraint(_))), "TC-R01");
        assert!(matches!(extension_repos::insert(&c, &repo("https://b", "f1")), Err(DbError::Constraint(_))), "TC-R02");
        assert!(matches!(extension_repos::upsert(&c, &repo("https://b", "f1")), Err(DbError::Constraint(_))));
        assert_eq!(extension_repos::count(&c).unwrap(), 1);
    }

    #[test]
    fn extension_repo_upsert_updates_the_row_with_the_same_base_url() {
        let c = test_db();
        extension_repos::insert(&c, &repo("https://a", "f1")).unwrap();
        let changed = ExtensionRepoRow { name: "Renamed".into(), short_name: None, signing_key_fingerprint: "f2".into(), ..repo("https://a", "f1") };
        extension_repos::upsert(&c, &changed).unwrap();
        assert_eq!(extension_repos::find_one(&c, "https://a").unwrap(), Some(changed));
        assert_eq!(extension_repos::count(&c).unwrap(), 1);
    }

    #[test]
    fn extension_repo_replace_on_fingerprint_match_overwrites_the_old_row() {
        let c = test_db();
        extension_repos::insert(&c, &repo("https://old", "f1")).unwrap();
        let new = ExtensionRepoRow {
            base_url: "https://new".into(),
            name: "New".into(),
            short_name: None,
            website: "https://w".into(),
            signing_key_fingerprint: "f1".into(),
        };
        extension_repos::replace(&c, &new).unwrap();
        assert_eq!(extension_repos::find_one(&c, "https://old").unwrap(), None, "TC-R03: old base_url is gone");
        assert_eq!(extension_repos::find_one(&c, "https://new").unwrap(), Some(new), "TC-R04");
        assert_eq!(extension_repos::count(&c).unwrap(), 1);
        extension_repos::insert(&c, &repo("https://other", "f2")).unwrap();
        let clash = ExtensionRepoRow { base_url: "https://other".into(), ..repo("https://x", "f1") };
        assert!(matches!(extension_repos::replace(&c, &clash), Err(DbError::Constraint(_))));
    }

    #[test]
    fn classify_conflict_checks_base_url_then_fingerprint() {
        let c = test_db();
        extension_repos::insert(&c, &repo("https://a", "f1")).unwrap();
        use extension_repos::RepoConflict;
        assert_eq!(extension_repos::classify_conflict(&c, "https://a", "zz").unwrap(), RepoConflict::RepoAlreadyExists);
        assert_eq!(
            extension_repos::classify_conflict(&c, "https://b", "f1").unwrap(),
            RepoConflict::DuplicateFingerprint(repo("https://a", "f1"))
        );
        assert_eq!(extension_repos::classify_conflict(&c, "https://b", "f9").unwrap(), RepoConflict::Other);
    }
}
