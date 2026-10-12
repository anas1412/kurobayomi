//! Anime SQL statement functions for entries, episodes and history (T-04a-009, T-04a-011,
//! T-04a-012). Plain functions over `&Connection` with no runtime, so an async wrapper can call them
//! from `spawn_blocking` later. Rows are read by column name.
//!
//! The anime kind lives in the unified tables (`entries`, `items`, `tracks`, ...) with `kind = 'anime'` pinned in
//! every statement. SELECT lists alias the unified columns back to the old anime names, so every row struct and
//! `from_row` is unchanged. The SQL is otherwise the Aniyomi `.sq` text, except that `HISTORY_LIST` adds
//! `ESCAPE '\'` so that `%` and `_` typed by the user match literally (decision D5), and the `IN :list` statements
//! get their placeholder list at run time.

use rusqlite::{named_params, params_from_iter, Connection, Row};

use crate::codec::{GenreList, UpdateStrategy};
use crate::schema_anime::{FetchType, Memo, Millis};

/// Ids per `IN (...)` statement, the chunk size of the Kotlin repositories.
const IN_CHUNK: usize = 500;

/// The old `animes` columns of an `entries` row (`$p` is a table alias with its dot, or empty). `item_flags` is the
/// anime layout already, `is_syncing` is gone and reads as 0.
macro_rules! anime_cols {
    ($p:literal) => {
        concat!(
            $p, "_id, ", $p, "source, ", $p, "url, ", $p, "artist, ", $p, "author, ", $p, "description, ", $p, "genre, ",
            $p, "title, ", $p, "status, ", $p, "thumbnail_url, ", $p, "favorite, ", $p, "last_update, ", $p, "next_update, ",
            $p, "initialized, ", $p, "viewer, ", $p, "item_flags AS episode_flags, ", $p, "cover_last_modified, ",
            $p, "date_added, ", $p, "update_strategy, ", $p, "calculate_interval, ", $p, "last_modified_at, ",
            $p, "favorite_modified_at, ", $p, "version, 0 AS is_syncing, ", $p, "fetch_type, ", $p, "parent_id, ",
            $p, "season_flags, ", $p, "season_number, ", $p, "season_source_order, ", $p, "background_url, ",
            $p, "background_last_modified, ", $p, "memo"
        )
    };
}

/// The old `episodes` columns of an `items` row (`$p` is an alias with its dot, or empty).
macro_rules! episode_cols {
    ($p:literal) => {
        concat!(
            $p, "_id, ", $p, "entry_id AS anime_id, ", $p, "url, ", $p, "name, ", $p, "scanlator, ", $p, "done AS seen, ",
            $p, "bookmark, ", $p, "progress AS last_second_seen, ", $p, "total_seconds, ", $p, "number AS episode_number, ",
            $p, "source_order, ", $p, "date_fetch, ", $p, "date_upload, ", $p, "last_modified_at, ", $p, "version, ",
            "0 AS is_syncing, ", $p, "summary, ", $p, "preview_url, ", $p, "fillermark, ", $p, "memo"
        )
    };
}

/// The old `anime_sync` columns of a `tracks` row (`$p` is an alias with its dot, or empty).
macro_rules! anime_track_cols {
    ($p:literal) => {
        concat!(
            $p, "_id, ", $p, "entry_id AS anime_id, ", $p, "tracker_id AS sync_id, ", $p, "remote_id, ", $p, "library_id, ",
            $p, "title, ", $p, "last_item AS last_episode_seen, ", $p, "total_items AS total_episodes, ", $p, "status, ",
            $p, "score, ", $p, "remote_url, ", $p, "start_date, ", $p, "finish_date, ", $p, "private"
        )
    };
}

/// The ids of the anime entries, as a subquery.
macro_rules! anime_ids {
    () => {
        "(SELECT _id FROM entries WHERE kind = 'anime')"
    };
}

/// The ids of the anime items, as a subquery.
macro_rules! anime_item_ids {
    () => {
        "(SELECT i._id FROM items i JOIN entries e ON e._id = i.entry_id WHERE e.kind = 'anime')"
    };
}

/// The two aggregate CTEs of the library view, over a scope of entry ids. `$scope` is the body of
/// `scope(entry_id)`; `$seasons` is the filter on the entries whose children roll up (design section 1.1).
macro_rules! stats_ctes {
    ($scope:literal, $seasons:literal) => {
        concat!(
            "WITH scope(entry_id) AS (", $scope, "), ",
            "item_stats AS (SELECT i.entry_id, count(*) AS total, sum(i.done) AS doneCount, ",
            "coalesce(max(i.date_upload), 0) AS latestUpload, coalesce(max(i.date_fetch), 0) AS fetchedAt, ",
            "sum(i.bookmark) AS bookmarkCount, sum(i.fillermark) AS fillermarkCount, coalesce(max(h.last_at), 0) AS lastActivity ",
            "FROM items i LEFT JOIN excluded_scanlators x ON x.entry_id = i.entry_id AND x.scanlator = i.scanlator ",
            "LEFT JOIN history h ON h.item_id = i._id ",
            "WHERE x.scanlator IS NULL AND i.entry_id IN (SELECT entry_id FROM scope) GROUP BY i.entry_id), ",
            "season_stats AS (SELECT s.parent_id, count(*) AS child_count, ",
            "sum(CASE WHEN s.fetch_type = 1 AND coalesce(t.total, 0) = coalesce(t.doneCount, 0) THEN 1 ELSE 0 END) AS fully_done, ",
            "max(coalesce(t.latestUpload, 0)) AS latestUpload, max(coalesce(t.fetchedAt, 0)) AS fetchedAt, ",
            "max(coalesce(t.lastActivity, 0)) AS lastActivity, sum(coalesce(t.bookmarkCount, 0)) AS bookmarkCount, ",
            "sum(coalesce(t.fillermarkCount, 0)) AS fillermarkCount ",
            "FROM entries s LEFT JOIN item_stats t ON t.entry_id = s._id WHERE s.parent_id IN (", $seasons, ") GROUP BY s.parent_id) "
        )
    };
}

const ANIME_BY_ID: &str = concat!("SELECT ", anime_cols!(""), " FROM entries WHERE _id = :id AND kind = 'anime'");

const ANIME_BY_URL_AND_SOURCE: &str =
    concat!("SELECT ", anime_cols!(""), " FROM entries WHERE url = :url AND source = :source AND kind = 'anime' LIMIT 1");

const ANIME_FAVORITES: &str = concat!("SELECT ", anime_cols!(""), " FROM entries WHERE favorite = 1 AND kind = 'anime'");

const ANIME_WATCHED_NOT_IN_LIBRARY: &str = concat!(
    "SELECT ", anime_cols!(""), " FROM entries WHERE kind = 'anime' AND favorite = 0 AND _id IN (SELECT DISTINCT entry_id FROM items WHERE done = 1 OR progress != 0)"
);

const ANIME_FAVORITES_BY_SOURCE: &str =
    concat!("SELECT ", anime_cols!(""), " FROM entries WHERE kind = 'anime' AND favorite = 1 AND source = :sourceId");

const ANIME_RESET_VIEWER_FLAGS: &str = "UPDATE entries SET viewer = 0 WHERE kind = 'anime'";

/// Replaces only the bits of `mask` in `viewer` of one anime, in one statement (EC-21 and EC-22 write their fields this
/// way, so the skip-intro and the airing groups never overwrite each other).
const ANIME_SET_VIEWER_BITS: &str =
    "UPDATE entries SET viewer = (viewer & ~:mask) | (:value & :mask) WHERE _id = :animeId AND kind = 'anime'";

const ANIME_INSERT: &str = "INSERT INTO entries(kind, source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, item_flags, cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, version, fetch_type, parent_id, season_flags, season_number, season_source_order, background_url, background_last_modified, memo)
VALUES ('anime', :source, :url, :artist, :author, :description, :genre, :title, :status, :thumbnailUrl, :favorite, :lastUpdate, :nextUpdate, :initialized, :viewerFlags, :episodeFlags, :coverLastModified,:dateAdded, :updateStrategy, :calculateInterval, 0, :version, :fetchType, :parentId, :seasonFlags, :seasonNumber, :seasonSourceOrder, :backgroundUrl, :backgroundLastModified, :memo)";

const ANIME_UPDATE: &str = r#"UPDATE entries SET
    source = coalesce(:source, source),
    url = coalesce(:url, url),
    artist = coalesce(:artist, artist),
    author = coalesce(:author, author),
    description = coalesce(:description, description),
    genre = coalesce(:genre, genre),
    title = coalesce(:title, title),
    status = coalesce(:status, status),
    thumbnail_url = coalesce(:thumbnailUrl, thumbnail_url),
    favorite = coalesce(:favorite, favorite),
    last_update = coalesce(:lastUpdate, last_update),
    next_update = coalesce(:nextUpdate, next_update),
    initialized = coalesce(:initialized, initialized),
    viewer = coalesce(:viewer, viewer),
    item_flags = coalesce(:episodeFlags, item_flags),
    cover_last_modified = coalesce(:coverLastModified, cover_last_modified),
    date_added = coalesce(:dateAdded, date_added),
    update_strategy = coalesce(:updateStrategy, update_strategy),
    calculate_interval = coalesce(:calculateInterval, calculate_interval),
    version = coalesce(:version, version),
    fetch_type = coalesce(:fetchType, fetch_type),
    parent_id = coalesce(:parentId, parent_id),
    season_flags = coalesce(:seasonFlags, season_flags),
    season_number = coalesce(:seasonNumber, season_number),
    season_source_order = coalesce(:seasonSourceOrder, season_source_order),
    background_url = coalesce(:backgroundUrl, background_url),
    background_last_modified = coalesce(:backgroundLastModified, background_last_modified),
    memo = coalesce(:memo, memo)
WHERE _id = :animeId AND kind = 'anime'"#;

const EPISODE_BY_ID: &str = concat!(
    "SELECT ", episode_cols!("i."), " FROM items i JOIN entries e ON e._id = i.entry_id AND e.kind = 'anime' WHERE i._id = :id"
);

const EPISODES_BY_ANIME_ID: &str = concat!(
    "SELECT ", episode_cols!("i."), " FROM items i JOIN entries e ON e._id = i.entry_id AND e.kind = 'anime' WHERE i.entry_id = :animeId"
);

const BOOKMARKED_EPISODES_BY_ANIME_ID: &str = concat!(
    "SELECT ", episode_cols!("i."), " FROM items i JOIN entries e ON e._id = i.entry_id AND e.kind = 'anime' WHERE i.bookmark AND i.entry_id = :animeId"
);

const EPISODE_BY_URL: &str = concat!(
    "SELECT ", episode_cols!("i."), " FROM items i JOIN entries e ON e._id = i.entry_id AND e.kind = 'anime' WHERE i.url = :episodeUrl"
);

const EPISODE_BY_URL_AND_ANIME_ID: &str = concat!(
    "SELECT ", episode_cols!("i."), " FROM items i JOIN entries e ON e._id = i.entry_id AND e.kind = 'anime' WHERE i.url = :episodeUrl AND i.entry_id = :animeId"
);

const EPISODE_INSERT: &str = r#"INSERT INTO items(entry_id, url, name, scanlator, done, bookmark, progress, total_seconds, number, source_order, date_fetch, date_upload, last_modified_at, version, summary, preview_url, fillermark, memo)
VALUES (:animeId, :url, :name, :scanlator, :seen, :bookmark, :lastSecondSeen, :totalSeconds, :episodeNumber, :sourceOrder, :dateFetch, :dateUpload, 0, :version, :summary, :previewUrl, :fillermark, :memo)"#;

const EPISODE_UPDATE: &str = r#"UPDATE items
SET entry_id = coalesce(:animeId, entry_id),
    url = coalesce(:url, url),
    name = coalesce(:name, name),
    scanlator = coalesce(:scanlator, scanlator),
    done = coalesce(:seen, done),
    bookmark = coalesce(:bookmark, bookmark),
    progress = coalesce(:lastSecondSeen, progress),
    total_seconds = coalesce(:totalSeconds, total_seconds),
    number = coalesce(:episodeNumber, number),
    source_order = coalesce(:sourceOrder, source_order),
    date_fetch = coalesce(:dateFetch, date_fetch),
    date_upload = coalesce(:dateUpload, date_upload),
    version = coalesce(:version, version),
    summary = coalesce(:summary, summary),
    preview_url = coalesce(:previewUrl, preview_url),
    fillermark = coalesce(:fillermark, fillermark),
    memo = coalesce(:memo, memo)
WHERE _id = :episodeId AND entry_id IN (SELECT _id FROM entries WHERE kind = 'anime')"#;

const HISTORY_RESET_BY_ID: &str =
    concat!("UPDATE history SET last_at = 0 WHERE _id = :historyId AND item_id IN ", anime_item_ids!());

const HISTORY_RESET_BY_ANIME_ID: &str = r#"UPDATE history
SET last_at = 0
WHERE _id IN (
    SELECT H._id
    FROM entries M
    INNER JOIN items EP
    ON M._id = EP.entry_id
    INNER JOIN history H
    ON EP._id = H.item_id
    WHERE M._id = :animeId AND M.kind = 'anime'
)"#;

const HISTORY_DELETE_ALL: &str = concat!("DELETE FROM history WHERE item_id IN ", anime_item_ids!());

const HISTORY_REMOVE_RESETTED: &str = concat!("DELETE FROM history WHERE last_at = 0 AND item_id IN ", anime_item_ids!());

const HISTORY_UPSERT: &str = r#"INSERT INTO history(item_id, last_at)
VALUES (:episodeId, :seenAt)
ON CONFLICT(item_id)
DO UPDATE
SET
    last_at = :seenAt
WHERE item_id = :episodeId"#;

const HISTORY_LIST: &str = r#"SELECT
id,
entryId AS animeId,
itemId AS episodeId,
title,
thumbnailUrl,
source,
favorite,
cover_last_modified,
itemNumber AS episodeNumber,
lastAt AS seenAt
FROM historyView
WHERE kind = 'anime'
AND lastAt > 0
AND maxLastAtItemId = itemId
AND kuro_lower(title) LIKE ('%' || kuro_lower(:query) || '%') ESCAPE '\'
ORDER BY lastAt DESC"#;

/// One `animes` row, all 32 columns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimeRow {
    pub id: i64,
    pub source: i64,
    pub url: String,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<GenreList>,
    pub title: String,
    pub status: i64,
    pub thumbnail_url: Option<String>,
    pub favorite: bool,
    pub last_update: Option<i64>,
    pub next_update: Option<i64>,
    pub initialized: bool,
    pub viewer: i64,
    pub episode_flags: i64,
    pub cover_last_modified: i64,
    pub date_added: i64,
    pub update_strategy: UpdateStrategy,
    pub calculate_interval: i64,
    pub last_modified_at: i64,
    pub favorite_modified_at: Option<i64>,
    pub version: i64,
    pub is_syncing: i64,
    pub fetch_type: FetchType,
    pub parent_id: Option<i64>,
    pub season_flags: i64,
    pub season_number: f64,
    pub season_source_order: i64,
    pub background_url: Option<String>,
    pub background_last_modified: i64,
    pub memo: Memo,
}

impl AnimeRow {
    pub fn from_row(r: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("_id")?,
            source: r.get("source")?,
            url: r.get("url")?,
            artist: r.get("artist")?,
            author: r.get("author")?,
            description: r.get("description")?,
            genre: r.get("genre")?,
            title: r.get("title")?,
            status: r.get("status")?,
            thumbnail_url: r.get("thumbnail_url")?,
            favorite: r.get("favorite")?,
            last_update: r.get("last_update")?,
            next_update: r.get("next_update")?,
            initialized: r.get("initialized")?,
            viewer: r.get("viewer")?,
            episode_flags: r.get("episode_flags")?,
            cover_last_modified: r.get("cover_last_modified")?,
            date_added: r.get("date_added")?,
            update_strategy: r.get("update_strategy")?,
            calculate_interval: r.get("calculate_interval")?,
            last_modified_at: r.get("last_modified_at")?,
            favorite_modified_at: r.get("favorite_modified_at")?,
            version: r.get("version")?,
            is_syncing: r.get("is_syncing")?,
            fetch_type: r.get("fetch_type")?,
            parent_id: r.get("parent_id")?,
            season_flags: r.get("season_flags")?,
            season_number: r.get("season_number")?,
            season_source_order: r.get("season_source_order")?,
            background_url: r.get("background_url")?,
            background_last_modified: r.get("background_last_modified")?,
            memo: r.get("memo")?,
        })
    }
}

/// Partial update of an anime (domain `AnimeUpdate`). `None` keeps the stored value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimeUpdate {
    pub id: i64,
    pub source: Option<i64>,
    pub url: Option<String>,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<GenreList>,
    pub title: Option<String>,
    pub status: Option<i64>,
    pub thumbnail_url: Option<String>,
    pub favorite: Option<bool>,
    pub last_update: Option<i64>,
    pub next_update: Option<i64>,
    pub initialized: Option<bool>,
    pub viewer_flags: Option<i64>,
    pub episode_flags: Option<i64>,
    pub cover_last_modified: Option<i64>,
    pub date_added: Option<i64>,
    pub update_strategy: Option<UpdateStrategy>,
    pub fetch_interval: Option<i64>,
    pub version: Option<i64>,
    pub fetch_type: Option<FetchType>,
    pub parent_id: Option<i64>,
    pub season_flags: Option<i64>,
    pub season_number: Option<f64>,
    pub season_source_order: Option<i64>,
    pub background_url: Option<String>,
    pub background_last_modified: Option<i64>,
    pub memo: Option<Memo>,
}

/// One `episodes` row, all 20 columns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EpisodeRow {
    pub id: i64,
    pub anime_id: i64,
    pub url: String,
    pub name: String,
    pub scanlator: Option<String>,
    pub seen: bool,
    pub bookmark: bool,
    pub last_second_seen: i64,
    pub total_seconds: i64,
    pub episode_number: f64,
    pub source_order: i64,
    pub date_fetch: i64,
    pub date_upload: i64,
    pub last_modified_at: i64,
    pub version: i64,
    pub is_syncing: i64,
    pub summary: Option<String>,
    pub preview_url: Option<String>,
    pub fillermark: bool,
    pub memo: Memo,
}

impl EpisodeRow {
    pub fn from_row(r: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("_id")?,
            anime_id: r.get("anime_id")?,
            url: r.get("url")?,
            name: r.get("name")?,
            scanlator: r.get("scanlator")?,
            seen: r.get("seen")?,
            bookmark: r.get("bookmark")?,
            last_second_seen: r.get("last_second_seen")?,
            total_seconds: r.get("total_seconds")?,
            episode_number: r.get("episode_number")?,
            source_order: r.get("source_order")?,
            date_fetch: r.get("date_fetch")?,
            date_upload: r.get("date_upload")?,
            last_modified_at: r.get("last_modified_at")?,
            version: r.get("version")?,
            is_syncing: r.get("is_syncing")?,
            summary: r.get("summary")?,
            preview_url: r.get("preview_url")?,
            fillermark: r.get("fillermark")?,
            memo: r.get("memo")?,
        })
    }
}

/// Partial update of an episode. `None` keeps the stored value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EpisodeUpdate {
    pub id: i64,
    pub anime_id: Option<i64>,
    pub url: Option<String>,
    pub name: Option<String>,
    pub scanlator: Option<String>,
    pub seen: Option<bool>,
    pub bookmark: Option<bool>,
    pub last_second_seen: Option<i64>,
    pub total_seconds: Option<i64>,
    pub episode_number: Option<f64>,
    pub source_order: Option<i64>,
    pub date_fetch: Option<i64>,
    pub date_upload: Option<i64>,
    pub version: Option<i64>,
    pub summary: Option<String>,
    pub preview_url: Option<String>,
    pub fillermark: Option<bool>,
    pub memo: Option<Memo>,
}

/// One row of `animehistoryView`: the newest history row of an anime, with its title and cover.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryEntry {
    pub id: i64,
    pub anime_id: i64,
    pub episode_id: i64,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub source: i64,
    pub favorite: bool,
    pub cover_last_modified: i64,
    pub episode_number: f64,
    pub seen_at: Millis,
}

impl HistoryEntry {
    fn from_row(r: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("id")?,
            anime_id: r.get("animeId")?,
            episode_id: r.get("episodeId")?,
            title: r.get("title")?,
            thumbnail_url: r.get("thumbnailUrl")?,
            source: r.get("source")?,
            favorite: r.get("favorite")?,
            cover_last_modified: r.get("cover_last_modified")?,
            episode_number: r.get("episodeNumber")?,
            seen_at: r.get("seenAt")?,
        })
    }
}

/// `?,?,...` for an `IN` list of `n` ids.
fn in_placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

/// Escapes `\`, `%` and `_` so that the user's text matches literally under `ESCAPE '\'`.
fn escape_like(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn anime_rows(c: &Connection, sql: &str, params: &[(&str, &dyn rusqlite::ToSql)]) -> rusqlite::Result<Vec<AnimeRow>> {
    c.prepare_cached(sql)?.query_map(params, AnimeRow::from_row)?.collect()
}

fn episode_rows(c: &Connection, sql: &str, params: &[(&str, &dyn rusqlite::ToSql)]) -> rusqlite::Result<Vec<EpisodeRow>> {
    c.prepare_cached(sql)?.query_map(params, EpisodeRow::from_row)?.collect()
}

/// Inserts an anime. `id`, `last_modified_at` (written as the literal 0), `favorite_modified_at` and
/// is not bound: the first two come from the statement, the other takes its column default.
/// Returns the new `_id`.
pub fn insert_anime(c: &Connection, a: &AnimeRow) -> rusqlite::Result<i64> {
    c.prepare_cached(ANIME_INSERT)?.execute(named_params! {
        ":source": a.source,
        ":url": a.url,
        ":artist": a.artist,
        ":author": a.author,
        ":description": a.description,
        ":genre": a.genre,
        ":title": a.title,
        ":status": a.status,
        ":thumbnailUrl": a.thumbnail_url,
        ":favorite": a.favorite,
        ":lastUpdate": a.last_update,
        ":nextUpdate": a.next_update,
        ":initialized": a.initialized,
        ":viewerFlags": a.viewer,
        ":episodeFlags": a.episode_flags,
        ":coverLastModified": a.cover_last_modified,
        ":dateAdded": a.date_added,
        ":updateStrategy": a.update_strategy,
        ":calculateInterval": a.calculate_interval,
        ":version": a.version,
        ":fetchType": a.fetch_type,
        ":parentId": a.parent_id,
        ":seasonFlags": a.season_flags,
        ":seasonNumber": a.season_number,
        ":seasonSourceOrder": a.season_source_order,
        ":backgroundUrl": a.background_url,
        ":backgroundLastModified": a.background_last_modified,
        ":memo": a.memo,
    })?;
    Ok(c.last_insert_rowid())
}

/// Coalesce update: every `None` keeps the stored column.
pub fn update_anime(c: &Connection, u: &AnimeUpdate) -> rusqlite::Result<usize> {
    c.prepare_cached(ANIME_UPDATE)?.execute(named_params! {
        ":source": u.source,
        ":url": u.url,
        ":artist": u.artist,
        ":author": u.author,
        ":description": u.description,
        ":genre": u.genre,
        ":title": u.title,
        ":status": u.status,
        ":thumbnailUrl": u.thumbnail_url,
        ":favorite": u.favorite,
        ":lastUpdate": u.last_update,
        ":nextUpdate": u.next_update,
        ":initialized": u.initialized,
        ":viewer": u.viewer_flags,
        ":episodeFlags": u.episode_flags,
        ":coverLastModified": u.cover_last_modified,
        ":dateAdded": u.date_added,
        ":updateStrategy": u.update_strategy,
        ":calculateInterval": u.fetch_interval,
        ":version": u.version,
        ":fetchType": u.fetch_type,
        ":parentId": u.parent_id,
        ":seasonFlags": u.season_flags,
        ":seasonNumber": u.season_number,
        ":seasonSourceOrder": u.season_source_order,
        ":backgroundUrl": u.background_url,
        ":backgroundLastModified": u.background_last_modified,
        ":memo": u.memo,
        ":animeId": u.id,
    })
}

pub fn anime_by_id(c: &Connection, id: i64) -> rusqlite::Result<Option<AnimeRow>> {
    Ok(anime_rows(c, ANIME_BY_ID, named_params! {":id": id})?.into_iter().next())
}

/// At most one row: `(kind, source, url)` is unique.
pub fn anime_by_url_and_source(c: &Connection, url: &str, source: i64) -> rusqlite::Result<Option<AnimeRow>> {
    let params = named_params! {":url": url, ":source": source};
    Ok(anime_rows(c, ANIME_BY_URL_AND_SOURCE, params)?.into_iter().next())
}

pub fn favorite_animes(c: &Connection) -> rusqlite::Result<Vec<AnimeRow>> {
    anime_rows(c, ANIME_FAVORITES, &[])
}

pub fn favorite_animes_by_source(c: &Connection, source: i64) -> rusqlite::Result<Vec<AnimeRow>> {
    anime_rows(c, ANIME_FAVORITES_BY_SOURCE, named_params! {":sourceId": source})
}

/// Non-favorite anime with at least one seen episode or some progress.
pub fn watched_not_in_library(c: &Connection) -> rusqlite::Result<Vec<AnimeRow>> {
    anime_rows(c, ANIME_WATCHED_NOT_IN_LIBRARY, &[])
}

/// Sets `viewer = 0` on every anime (the "reset player settings" action).
pub fn reset_viewer_flags(c: &Connection) -> rusqlite::Result<usize> {
    c.prepare_cached(ANIME_RESET_VIEWER_FLAGS)?.execute([])
}

/// Writes `value` under `mask` into `viewer` of one anime. Returns the rows changed: `0` for a deleted anime.
pub fn set_viewer_bits(c: &Connection, anime_id: i64, value: i64, mask: i64) -> rusqlite::Result<usize> {
    c.prepare_cached(ANIME_SET_VIEWER_BITS)?.execute(named_params! {
        ":animeId": anime_id,
        ":value": value,
        ":mask": mask,
    })
}

/// Inserts an episode and returns its new `_id`. `last_modified_at` is written as literal 0 by the statement.
pub fn insert_episode(c: &Connection, e: &EpisodeRow) -> rusqlite::Result<i64> {
    c.prepare_cached(EPISODE_INSERT)?.execute(named_params! {
        ":animeId": e.anime_id,
        ":url": e.url,
        ":name": e.name,
        ":scanlator": e.scanlator,
        ":seen": e.seen,
        ":bookmark": e.bookmark,
        ":lastSecondSeen": e.last_second_seen,
        ":totalSeconds": e.total_seconds,
        ":episodeNumber": e.episode_number,
        ":sourceOrder": e.source_order,
        ":dateFetch": e.date_fetch,
        ":dateUpload": e.date_upload,
        ":version": e.version,
        ":summary": e.summary,
        ":previewUrl": e.preview_url,
        ":fillermark": e.fillermark,
        ":memo": e.memo,
    })?;
    Ok(c.last_insert_rowid())
}

/// Coalesce update of an episode.
pub fn update_episode(c: &Connection, u: &EpisodeUpdate) -> rusqlite::Result<usize> {
    c.prepare_cached(EPISODE_UPDATE)?.execute(named_params! {
        ":animeId": u.anime_id,
        ":url": u.url,
        ":name": u.name,
        ":scanlator": u.scanlator,
        ":seen": u.seen,
        ":bookmark": u.bookmark,
        ":lastSecondSeen": u.last_second_seen,
        ":totalSeconds": u.total_seconds,
        ":episodeNumber": u.episode_number,
        ":sourceOrder": u.source_order,
        ":dateFetch": u.date_fetch,
        ":dateUpload": u.date_upload,
        ":version": u.version,
        ":summary": u.summary,
        ":previewUrl": u.preview_url,
        ":fillermark": u.fillermark,
        ":memo": u.memo,
        ":episodeId": u.id,
    })
}

pub fn episode_by_id(c: &Connection, id: i64) -> rusqlite::Result<Option<EpisodeRow>> {
    Ok(episode_rows(c, EPISODE_BY_ID, named_params! {":id": id})?.into_iter().next())
}

pub fn episodes_by_anime_id(c: &Connection, anime_id: i64) -> rusqlite::Result<Vec<EpisodeRow>> {
    episode_rows(c, EPISODES_BY_ANIME_ID, named_params! {":animeId": anime_id})
}

pub fn bookmarked_episodes_by_anime_id(c: &Connection, anime_id: i64) -> rusqlite::Result<Vec<EpisodeRow>> {
    episode_rows(c, BOOKMARKED_EPISODES_BY_ANIME_ID, named_params! {":animeId": anime_id})
}

/// Every episode with this URL in any anime. Restore uses the first row.
pub fn episodes_by_url(c: &Connection, url: &str) -> rusqlite::Result<Vec<EpisodeRow>> {
    episode_rows(c, EPISODE_BY_URL, named_params! {":episodeUrl": url})
}

pub fn episodes_by_url_and_anime_id(c: &Connection, url: &str, anime_id: i64) -> rusqlite::Result<Vec<EpisodeRow>> {
    episode_rows(c, EPISODE_BY_URL_AND_ANIME_ID, named_params! {":episodeUrl": url, ":animeId": anime_id})
}

/// Deletes the episodes with these ids, in chunks of 500 ids. Returns the number of deleted rows.
pub fn remove_episodes_with_ids(c: &Connection, ids: &[i64]) -> rusqlite::Result<usize> {
    let mut removed = 0;
    for chunk in ids.chunks(IN_CHUNK) {
        let sql = format!("DELETE FROM items\nWHERE _id IN ({}) AND entry_id IN {}", in_placeholders(chunk.len()), anime_ids!());
        removed += c.prepare(&sql)?.execute(params_from_iter(chunk))?;
    }
    Ok(removed)
}

/// Inserts the history row of an episode, or moves its `last_seen` when it has one.
pub fn upsert_history(c: &Connection, episode_id: i64, seen_at: Millis) -> rusqlite::Result<usize> {
    c.prepare_cached(HISTORY_UPSERT)?.execute(named_params! {":episodeId": episode_id, ":seenAt": seen_at})
}

/// The newest history row of each anime with `seenAt > 0`, newest first, whose title contains
/// `query` (case-insensitive for ASCII, `%` and `_` are literal).
pub fn history(c: &Connection, query: &str) -> rusqlite::Result<Vec<HistoryEntry>> {
    let mut st = c.prepare_cached(HISTORY_LIST)?;
    st.query_map(named_params! {":query": escape_like(query)}, HistoryEntry::from_row)?
        .collect()
}

/// Sets `last_seen = 0` on one history row. The row stays. If it was the anime's newest row,
/// `history` then shows that anime's next most recent row.
pub fn reset_history(c: &Connection, history_id: i64) -> rusqlite::Result<usize> {
    c.prepare_cached(HISTORY_RESET_BY_ID)?.execute(named_params! {":historyId": history_id})
}

/// Sets `last_seen = 0` on every history row of one anime.
pub fn reset_history_by_anime(c: &Connection, anime_id: i64) -> rusqlite::Result<usize> {
    c.prepare_cached(HISTORY_RESET_BY_ANIME_ID)?.execute(named_params! {":animeId": anime_id})
}

/// Deletes every history row. The change bus must report `animehistory` for this statement itself:
/// SQLite's truncate optimisation can hide it from the update hook (`p1-rust-design.md` section 4).
pub fn delete_all_history(c: &Connection) -> rusqlite::Result<usize> {
    c.prepare_cached(HISTORY_DELETE_ALL)?.execute([])
}

/// Deletes the history rows that were reset (`last_seen = 0`).
pub fn remove_resetted_history(c: &Connection) -> rusqlite::Result<usize> {
    c.prepare_cached(HISTORY_REMOVE_RESETTED)?.execute([])
}

/// One `libraryView` row of the anime kind (T-04a-010): the entry, its aggregates and one category it belongs to
/// (`0` when it has none).
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryAnimeRow {
    pub anime: AnimeRow,
    pub category: i64,
    pub total_count: i64,
    pub seen_count: i64,
    pub bookmark_count: i64,
    pub fillermark_count: i64,
    pub latest_upload: i64,
    pub episode_fetched_at: i64,
    pub last_seen: i64,
}

/// An aggregate column as the Kotlin mapper reads it: a REAL (`sum` of an integer column can come back
/// as either), truncated to an integer. NULL reads as 0.
fn count_column(r: &Row<'_>, name: &str) -> rusqlite::Result<i64> {
    Ok(r.get::<_, Option<f64>>(name)?.unwrap_or(0.0) as i64)
}

impl LibraryAnimeRow {
    pub fn from_row(r: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            anime: AnimeRow::from_row(r)?,
            category: r.get("category")?,
            total_count: r.get("totalCount")?,
            seen_count: count_column(r, "seenCount")?,
            bookmark_count: count_column(r, "bookmarkCount")?,
            fillermark_count: count_column(r, "fillermarkCount")?,
            latest_upload: r.get("latestUpload")?,
            episode_fetched_at: r.get("episodeFetchedAt")?,
            last_seen: r.get("lastSeen")?,
        })
    }
}

const LIBRARY: &str = concat!(
    "SELECT ", anime_cols!(""), ", category, totalCount, doneCount AS seenCount, bookmarkCount, fillermarkCount, latestUpload, itemFetchedAt AS episodeFetchedAt, lastActivity AS lastSeen FROM libraryView WHERE kind = 'anime'"
);

/// The seasons of one parent: the two stats CTEs of the library view over the children of `:parentId` and their
/// children. A browsed parent that is not a favourite has seasons too, which the view's favourites prefilter would
/// zero out, so this is a statement and not a view.
const SEASONS_OF: &str = concat!(
    stats_ctes!(
        "SELECT _id FROM entries WHERE parent_id = :parentId UNION SELECT _id FROM entries WHERE parent_id IN (SELECT _id FROM entries WHERE parent_id = :parentId)",
        "SELECT _id FROM entries WHERE parent_id = :parentId"
    ),
    "SELECT ", anime_cols!("e."), ", ",
    "CASE WHEN e.fetch_type = 0 THEN coalesce(ss.child_count, 0) ELSE coalesce(st.total, 0) END AS totalCount, ",
    "CASE WHEN e.fetch_type = 0 THEN coalesce(ss.fully_done, 0) ELSE coalesce(st.doneCount, 0) END AS seenCount, ",
    "CASE WHEN e.fetch_type = 0 THEN coalesce(ss.latestUpload, 0) ELSE coalesce(st.latestUpload, 0) END AS latestUpload, ",
    "CASE WHEN e.fetch_type = 0 THEN coalesce(ss.fetchedAt, 0) ELSE coalesce(st.fetchedAt, 0) END AS episodeFetchedAt, ",
    "CASE WHEN e.fetch_type = 0 THEN coalesce(ss.lastActivity, 0) ELSE coalesce(st.lastActivity, 0) END AS lastSeen, ",
    "CASE WHEN e.fetch_type = 0 THEN coalesce(ss.bookmarkCount, 0) ELSE coalesce(st.bookmarkCount, 0) END AS bookmarkCount, ",
    "CASE WHEN e.fetch_type = 0 THEN coalesce(ss.fillermarkCount, 0) ELSE coalesce(st.fillermarkCount, 0) END AS fillermarkCount ",
    "FROM entries e LEFT JOIN item_stats st ON st.entry_id = e._id LEFT JOIN season_stats ss ON ss.parent_id = e._id ",
    "WHERE e.parent_id = :parentId AND e.kind = 'anime' ORDER BY e._id"
);

/// Every library entry, one row per category membership. Read by column name, so a column added to
/// `entries` does not shift the aggregates.
pub fn library(c: &Connection) -> rusqlite::Result<Vec<LibraryAnimeRow>> {
    c.prepare_cached(LIBRARY)?.query_map([], LibraryAnimeRow::from_row)?.collect()
}

/// One row of [`seasons_of`]: a season (child entry) with its episode aggregates.
#[derive(Clone, Debug, PartialEq)]
pub struct SeasonAnimeRow {
    pub anime: AnimeRow,
    pub total_count: i64,
    pub seen_count: i64,
    pub bookmark_count: i64,
    pub fillermark_count: i64,
    pub latest_upload: i64,
    pub episode_fetched_at: i64,
    pub last_seen: i64,
}

/// The seasons of one parent entry, in `_id` order.
pub fn seasons_of(c: &Connection, parent_id: i64) -> rusqlite::Result<Vec<SeasonAnimeRow>> {
    c.prepare_cached(SEASONS_OF)?
        .query_map(named_params! {":parentId": parent_id}, |r| {
            Ok(SeasonAnimeRow {
                anime: AnimeRow::from_row(r)?,
                total_count: r.get("totalCount")?,
                seen_count: count_column(r, "seenCount")?,
                bookmark_count: count_column(r, "bookmarkCount")?,
                fillermark_count: count_column(r, "fillermarkCount")?,
                latest_upload: r.get("latestUpload")?,
                episode_fetched_at: r.get("episodeFetchedAt")?,
                last_seen: r.get("lastSeen")?,
            })
        })?
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Batch and repository statements: episodes (T-04a-011), categories (T-04a-013), tracks
// (T-04a-014), updates feed (T-04a-015), stub sources (T-04a-016), relations (T-04a-017),
// custom buttons (T-04a-018), extension store (T-04a-019), clear (T-04a-021), duplicate lookup
// (T-04a-022, T-04a-029), favourite counts (T-04a-029) and upcoming (T-04a-030).
// ---------------------------------------------------------------------------------------------

/// Runs `f` atomically: in its own transaction when `c` is in autocommit mode, otherwise (inside a
/// `Db::write`) as a savepoint of the open transaction. Never starts a nested `BEGIN`.
fn atomically<T>(c: &Connection, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> rusqlite::Result<T> {
    if c.is_autocommit() {
        let tx = c.unchecked_transaction()?;
        let value = f(&tx)?;
        tx.commit()?;
        return Ok(value);
    }
    c.execute_batch("SAVEPOINT kuro_batch")?;
    match f(c) {
        Ok(value) => {
            c.execute_batch("RELEASE kuro_batch")?;
            Ok(value)
        }
        Err(e) => {
            // The original error is the one the caller needs; a failing rollback adds nothing to it.
            let _ = c.execute_batch("ROLLBACK TO kuro_batch; RELEASE kuro_batch");
            Err(e)
        }
    }
}

/// Inserts every episode in one transaction. Returns each episode with its new `_id`, in input
/// order. A failure rolls the whole batch back and returns an empty list (the Kotlin repository
/// logs the error and returns nothing).
pub fn add_all_episodes(c: &Connection, episodes: &[EpisodeRow]) -> Vec<EpisodeRow> {
    match atomically(c, |c| insert_all_episodes(c, episodes)) {
        Ok(added) => added,
        Err(err) => {
            tracing::error!(error = %err, "add_all_episodes rolled back");
            Vec::new()
        }
    }
}

fn insert_all_episodes(c: &Connection, episodes: &[EpisodeRow]) -> rusqlite::Result<Vec<EpisodeRow>> {
    let mut added = Vec::with_capacity(episodes.len());
    for e in episodes {
        let id = insert_episode(c, e)?;
        added.push(EpisodeRow { id, ..e.clone() });
    }
    Ok(added)
}

/// Coalesce update of every episode in one transaction. Returns the number of rows written.
pub fn update_all_episodes(c: &Connection, updates: &[EpisodeUpdate]) -> rusqlite::Result<usize> {
    atomically(c, |c| {
        let mut written = 0;
        for u in updates {
            written += update_episode(c, u)?;
        }
        Ok(written)
    })
}

/// `AnimeRepositoryImpl.updateAllAnime`: the coalesce update of every entry in one transaction. Any
/// failure rolls the batch back, is logged, and gives `false`; nothing is propagated.
pub fn update_all_anime(c: &Connection, updates: &[AnimeUpdate]) -> bool {
    let result = atomically(c, |c| {
        for u in updates {
            update_anime(c, u)?;
        }
        Ok(())
    });
    match result {
        Ok(()) => true,
        Err(err) => {
            tracing::error!(error = %err, "update_all_anime rolled back");
            false
        }
    }
}

/// `AnimeHistoryRepositoryImpl.deleteAllAnimeHistory`: `true` on success, `false` after logging on error.
pub fn delete_all_history_logged(c: &Connection) -> bool {
    match delete_all_history(c) {
        Ok(_) => true,
        Err(err) => {
            tracing::error!(error = %err, "delete_all_history failed");
            false
        }
    }
}

const CATEGORY_BY_ID: &str = "SELECT _id AS id, name, sort AS \"order\", flags, hidden FROM categories WHERE _id = :id AND kind = 'anime' LIMIT 1";

const CATEGORIES: &str = "SELECT _id AS id, name, sort AS \"order\", flags, hidden FROM categories WHERE kind = 'anime' ORDER BY sort, _id";

const VISIBLE_CATEGORIES: &str = "SELECT _id AS id, name, sort AS \"order\", flags, hidden FROM categories WHERE kind = 'anime' AND hidden = 0 ORDER BY sort, _id";

const CATEGORIES_BY_ANIME_ID: &str = "SELECT C._id AS id, C.name, C.sort AS \"order\", C.flags, C.hidden FROM categories C JOIN entry_categories AC ON C._id = AC.category_id AND C.kind = AC.kind WHERE AC.entry_id = :animeId AND C.kind = 'anime' ORDER BY C.sort, C._id";

const VISIBLE_CATEGORIES_BY_ANIME_ID: &str = "SELECT C._id AS id, C.name, C.sort AS \"order\", C.flags, C.hidden FROM categories C JOIN entry_categories AC ON C._id = AC.category_id AND C.kind = AC.kind WHERE AC.entry_id = :animeId AND C.kind = 'anime' AND C.hidden = 0 ORDER BY C.sort, C._id";

const CATEGORY_INSERT: &str = "INSERT INTO categories(kind, _id, name, sort, flags) SELECT 'anime', coalesce(max(_id), 0) + 1, :name, :order, :flags FROM categories WHERE kind = 'anime' RETURNING _id";

const CATEGORY_UPDATE: &str = r#"UPDATE categories
SET name = coalesce(:name, name),
    sort = coalesce(:order, sort),
    flags = coalesce(:flags, flags),
    hidden = coalesce(:hidden, hidden)
WHERE _id = :categoryId AND kind = 'anime'"#;

const CATEGORY_DELETE: &str = "DELETE FROM categories WHERE _id = :categoryId AND kind = 'anime'";

const CATEGORY_SET_ORDER: &str = "UPDATE categories SET sort = :order WHERE _id = :categoryId AND kind = 'anime'";

const CATEGORY_UPDATE_ALL_FLAGS: &str = "UPDATE categories SET flags = coalesce(:flags, flags) WHERE kind = 'anime'";

const ANIME_CATEGORY_INSERT: &str = "INSERT OR IGNORE INTO entry_categories(entry_id, kind, category_id) VALUES (:animeId, 'anime', :categoryId)";

const ANIME_CATEGORY_DELETE_BY_ANIME_ID: &str = "DELETE FROM entry_categories WHERE entry_id = :animeId AND kind = 'anime'";

/// A row of `categories`. `order` is the `sort` column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimeCategory {
    pub id: i64,
    pub name: String,
    pub order: i64,
    pub flags: i64,
    pub hidden: bool,
}

impl AnimeCategory {
    fn from_row(r: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("id")?,
            name: r.get("name")?,
            order: r.get("order")?,
            flags: r.get("flags")?,
            hidden: r.get("hidden")?,
        })
    }
}

fn category_rows(c: &Connection, sql: &str, params: &[(&str, &dyn rusqlite::ToSql)]) -> rusqlite::Result<Vec<AnimeCategory>> {
    c.prepare_cached(sql)?.query_map(params, AnimeCategory::from_row)?.collect()
}

pub fn category_by_id(c: &Connection, id: i64) -> rusqlite::Result<Option<AnimeCategory>> {
    Ok(category_rows(c, CATEGORY_BY_ID, named_params! {":id": id})?.into_iter().next())
}

pub fn categories(c: &Connection) -> rusqlite::Result<Vec<AnimeCategory>> {
    category_rows(c, CATEGORIES, &[])
}

pub fn visible_categories(c: &Connection) -> rusqlite::Result<Vec<AnimeCategory>> {
    category_rows(c, VISIBLE_CATEGORIES, &[])
}

pub fn categories_by_anime_id(c: &Connection, anime_id: i64) -> rusqlite::Result<Vec<AnimeCategory>> {
    category_rows(c, CATEGORIES_BY_ANIME_ID, named_params! {":animeId": anime_id})
}

pub fn visible_categories_by_anime_id(c: &Connection, anime_id: i64) -> rusqlite::Result<Vec<AnimeCategory>> {
    category_rows(c, VISIBLE_CATEGORIES_BY_ANIME_ID, named_params! {":animeId": anime_id})
}

/// Inserts a category and returns its `_id`. `hidden` takes its default (0).
pub fn insert_category(c: &Connection, name: &str, order: i64, flags: i64) -> rusqlite::Result<i64> {
    c.prepare_cached(CATEGORY_INSERT)?
        .query_row(named_params! {":name": name, ":order": order, ":flags": flags}, |r| r.get(0))
}

/// Coalesce update: every `None` keeps the stored value.
pub fn update_category(
    c: &Connection,
    id: i64,
    name: Option<&str>,
    order: Option<i64>,
    flags: Option<i64>,
    hidden: Option<bool>,
) -> rusqlite::Result<usize> {
    c.prepare_cached(CATEGORY_UPDATE)?.execute(named_params! {
        ":name": name,
        ":order": order,
        ":flags": flags,
        ":hidden": hidden,
        ":categoryId": id,
    })
}

/// Deletes a category. Category 0 (the system category) is refused by the trigger
/// `system_category_delete_trigger` with the message "System category can't be deleted".
pub fn delete_category(c: &Connection, id: i64) -> rusqlite::Result<usize> {
    c.prepare_cached(CATEGORY_DELETE)?.execute(named_params! {":categoryId": id})
}

/// Sets `flags` on every category. `None` changes nothing.
pub fn update_all_category_flags(c: &Connection, flags: Option<i64>) -> rusqlite::Result<usize> {
    c.prepare_cached(CATEGORY_UPDATE_ALL_FLAGS)?.execute(named_params! {":flags": flags})
}

/// Writes `sort = index` for each id in `ids`, in one transaction.
pub fn set_category_orders(c: &Connection, ids: &[i64]) -> rusqlite::Result<()> {
    let tx = c.unchecked_transaction()?;
    for (order, id) in ids.iter().enumerate() {
        tx.prepare_cached(CATEGORY_SET_ORDER)?
            .execute(named_params! {":order": order as i64, ":categoryId": id})?;
    }
    tx.commit()
}

/// Replaces the category links of one anime in one transaction. A repeated id is linked once, and
/// each link insert bumps `animes.version` once (trigger `insert_anime_category_update_version`).
pub fn set_anime_categories(c: &Connection, anime_id: i64, category_ids: &[i64]) -> rusqlite::Result<()> {
    let tx = c.unchecked_transaction()?;
    tx.prepare_cached(ANIME_CATEGORY_DELETE_BY_ANIME_ID)?
        .execute(named_params! {":animeId": anime_id})?;
    let mut seen = std::collections::HashSet::new();
    for &category_id in category_ids.iter().filter(|id| seen.insert(**id)) {
        tx.prepare_cached(ANIME_CATEGORY_INSERT)?
            .execute(named_params! {":animeId": anime_id, ":categoryId": category_id})?;
    }
    tx.commit()
}

const TRACKS: &str = concat!("SELECT ", anime_track_cols!("t."), " FROM tracks t JOIN entries e ON e._id = t.entry_id AND e.kind = 'anime'");

const TRACK_BY_ID: &str = concat!("SELECT ", anime_track_cols!("t."), " FROM tracks t JOIN entries e ON e._id = t.entry_id AND e.kind = 'anime' WHERE t._id = :id");

const TRACKS_BY_ANIME_ID: &str = concat!("SELECT ", anime_track_cols!("t."), " FROM tracks t JOIN entries e ON e._id = t.entry_id AND e.kind = 'anime' WHERE t.entry_id = :animeId");

const TRACK_INSERT: &str = "INSERT INTO tracks(entry_id,tracker_id,remote_id,library_id,title,last_item,total_items,status,score,remote_url,start_date,finish_date,private) VALUES (:animeId,:syncId,:remoteId,:libraryId,:title,:lastEpisodeSeen,:totalEpisodes,:status,:score,:remoteUrl,:startDate,:finishDate,:private)";

const TRACK_UPDATE: &str = concat!(
    "UPDATE tracks SET entry_id = coalesce(:animeId, entry_id), tracker_id = coalesce(:syncId, tracker_id), remote_id = coalesce(:mediaId, remote_id), library_id = coalesce(:libraryId, library_id), title = coalesce(:title, title), last_item = coalesce(:lastEpisodeSeen, last_item), total_items = coalesce(:totalEpisodes, total_items), status = coalesce(:status, status), score = coalesce(:score, score), remote_url = coalesce(:trackingUrl, remote_url), start_date = coalesce(:startDate, start_date), finish_date = coalesce(:finishDate, finish_date), private = coalesce(:private, private) WHERE _id = :id AND entry_id IN ", anime_ids!()
);

/// Upsert keyed by `(anime_id, sync_id)`: the row keeps its `_id` (Q-TRK-2). Returns the row id.
const TRACK_UPSERT: &str = "INSERT INTO tracks(entry_id,tracker_id,remote_id,library_id,title,last_item,total_items,status,score,remote_url,start_date,finish_date,private) VALUES (:animeId,:syncId,:remoteId,:libraryId,:title,:lastEpisodeSeen,:totalEpisodes,:status,:score,:remoteUrl,:startDate,:finishDate,:private) ON CONFLICT(entry_id, tracker_id) DO UPDATE SET remote_id = excluded.remote_id, library_id = excluded.library_id, title = excluded.title, last_item = excluded.last_item, total_items = excluded.total_items, status = excluded.status, score = excluded.score, remote_url = excluded.remote_url, start_date = excluded.start_date, finish_date = excluded.finish_date, private = excluded.private RETURNING _id";
const TRACK_DELETE: &str = concat!("DELETE FROM tracks WHERE entry_id = :animeId AND tracker_id = :syncId AND entry_id IN ", anime_ids!());

/// A row of `anime_sync`. `private` is read as a boolean.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimeTrack {
    pub id: i64,
    pub anime_id: i64,
    pub sync_id: i64,
    pub remote_id: i64,
    pub library_id: Option<i64>,
    pub title: String,
    pub last_episode_seen: f64,
    pub total_episodes: i64,
    pub status: i64,
    pub score: f64,
    pub remote_url: String,
    pub start_date: i64,
    pub finish_date: i64,
    pub private: bool,
}

impl AnimeTrack {
    fn from_row(r: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get("_id")?,
            anime_id: r.get("anime_id")?,
            sync_id: r.get("sync_id")?,
            remote_id: r.get("remote_id")?,
            library_id: r.get("library_id")?,
            title: r.get("title")?,
            last_episode_seen: r.get("last_episode_seen")?,
            total_episodes: r.get("total_episodes")?,
            status: r.get("status")?,
            score: r.get("score")?,
            remote_url: r.get("remote_url")?,
            start_date: r.get("start_date")?,
            finish_date: r.get("finish_date")?,
            private: r.get("private")?,
        })
    }
}

/// Coalesce update of an `anime_sync` row. `remote_id` and `remote_url` are the Kotlin fields
/// `mediaId` and `trackingUrl`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimeTrackUpdate {
    pub id: i64,
    pub anime_id: Option<i64>,
    pub sync_id: Option<i64>,
    pub remote_id: Option<i64>,
    pub library_id: Option<i64>,
    pub title: Option<String>,
    pub last_episode_seen: Option<f64>,
    pub total_episodes: Option<i64>,
    pub status: Option<i64>,
    pub score: Option<f64>,
    pub remote_url: Option<String>,
    pub start_date: Option<i64>,
    pub finish_date: Option<i64>,
    pub private: Option<bool>,
}

/// Inserts tracks in one transaction. A row for an existing `(anime_id, sync_id)` pair is replaced
/// (`UNIQUE ... ON CONFLICT REPLACE`), and the replacement gets a new `_id`.
pub fn insert_tracks(c: &Connection, tracks: &[AnimeTrack]) -> rusqlite::Result<()> {
    let tx = c.unchecked_transaction()?;
    for t in tracks {
        tx.prepare_cached(TRACK_INSERT)?.execute(named_params! {
            ":animeId": t.anime_id,
            ":syncId": t.sync_id,
            ":remoteId": t.remote_id,
            ":libraryId": t.library_id,
            ":title": t.title,
            ":lastEpisodeSeen": t.last_episode_seen,
            ":totalEpisodes": t.total_episodes,
            ":status": t.status,
            ":score": t.score,
            ":remoteUrl": t.remote_url,
            ":startDate": t.start_date,
            ":finishDate": t.finish_date,
            ":private": t.private,
        })?;
    }
    tx.commit()
}

/// Upserts one track (see `TRACK_UPSERT`) and returns its row id. No transaction is opened here, so a caller
/// inside a write transaction can call it in a loop.
pub fn upsert_track(c: &Connection, t: &AnimeTrack) -> rusqlite::Result<i64> {
    c.prepare_cached(TRACK_UPSERT)?.query_row(
        named_params! {
            ":animeId": t.anime_id,
            ":syncId": t.sync_id,
            ":remoteId": t.remote_id,
            ":libraryId": t.library_id,
            ":title": t.title,
            ":lastEpisodeSeen": t.last_episode_seen,
            ":totalEpisodes": t.total_episodes,
            ":status": t.status,
            ":score": t.score,
            ":remoteUrl": t.remote_url,
            ":startDate": t.start_date,
            ":finishDate": t.finish_date,
            ":private": t.private,
        },
        |row| row.get(0),
    )
}

/// Coalesce update of one track row. Used by restore.
pub fn update_track(c: &Connection, u: &AnimeTrackUpdate) -> rusqlite::Result<usize> {
    c.prepare_cached(TRACK_UPDATE)?.execute(named_params! {
        ":animeId": u.anime_id,
        ":syncId": u.sync_id,
        ":mediaId": u.remote_id,
        ":libraryId": u.library_id,
        ":title": u.title,
        ":lastEpisodeSeen": u.last_episode_seen,
        ":totalEpisodes": u.total_episodes,
        ":status": u.status,
        ":score": u.score,
        ":trackingUrl": u.remote_url,
        ":startDate": u.start_date,
        ":finishDate": u.finish_date,
        ":private": u.private,
        ":id": u.id,
    })
}

/// Deletes the one track of `sync_id` bound to `anime_id`.
pub fn delete_track(c: &Connection, anime_id: i64, sync_id: i64) -> rusqlite::Result<usize> {
    c.prepare_cached(TRACK_DELETE)?.execute(named_params! {":animeId": anime_id, ":syncId": sync_id})
}

fn track_rows(c: &Connection, sql: &str, params: &[(&str, &dyn rusqlite::ToSql)]) -> rusqlite::Result<Vec<AnimeTrack>> {
    c.prepare_cached(sql)?.query_map(params, AnimeTrack::from_row)?.collect()
}

pub fn tracks(c: &Connection) -> rusqlite::Result<Vec<AnimeTrack>> {
    track_rows(c, TRACKS, &[])
}

pub fn track_by_id(c: &Connection, id: i64) -> rusqlite::Result<Option<AnimeTrack>> {
    Ok(track_rows(c, TRACK_BY_ID, named_params! {":id": id})?.into_iter().next())
}

pub fn tracks_by_anime_id(c: &Connection, anime_id: i64) -> rusqlite::Result<Vec<AnimeTrack>> {
    track_rows(c, TRACKS_BY_ANIME_ID, named_params! {":animeId": anime_id})
}

const RECENT_UPDATES: &str = r#"SELECT entryId AS animeId, entryTitle AS animeTitle, itemId AS episodeId, itemName AS episodeName, scanlator, done AS seen, bookmark, fillermark, progress AS last_second_seen, totalSeconds, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch
FROM updatesView
WHERE kind = 'anime'
AND dateUpload > :after
ORDER BY datefetch DESC
LIMIT :limit"#;

const UPDATES_BY_SEEN: &str = r#"SELECT entryId AS animeId, entryTitle AS animeTitle, itemId AS episodeId, itemName AS episodeName, scanlator, done AS seen, bookmark, fillermark, progress AS last_second_seen, totalSeconds, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch
FROM updatesView
WHERE kind = 'anime'
AND done = :seen
AND dateUpload > :after
ORDER BY datefetch DESC
LIMIT :limit"#;

/// A row of `animeupdatesView`: an episode of a library anime, fetched after the anime was added.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimeUpdateRow {
    pub anime_id: i64,
    pub anime_title: String,
    pub episode_id: i64,
    pub episode_name: String,
    pub scanlator: Option<String>,
    pub seen: bool,
    pub bookmark: bool,
    pub fillermark: bool,
    pub last_second_seen: i64,
    pub total_seconds: i64,
    pub source: i64,
    pub favorite: bool,
    pub thumbnail_url: Option<String>,
    pub cover_last_modified: i64,
    pub date_upload: i64,
    pub date_fetch: i64,
}

impl AnimeUpdateRow {
    fn from_row(r: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            anime_id: r.get("animeId")?,
            anime_title: r.get("animeTitle")?,
            episode_id: r.get("episodeId")?,
            episode_name: r.get("episodeName")?,
            scanlator: r.get("scanlator")?,
            seen: r.get("seen")?,
            bookmark: r.get("bookmark")?,
            fillermark: r.get("fillermark")?,
            last_second_seen: r.get("last_second_seen")?,
            total_seconds: r.get("totalSeconds")?,
            source: r.get("source")?,
            favorite: r.get("favorite")?,
            thumbnail_url: r.get("thumbnailUrl")?,
            cover_last_modified: r.get("coverLastModified")?,
            date_upload: r.get("dateUpload")?,
            date_fetch: r.get("datefetch")?,
        })
    }
}

fn update_rows(c: &Connection, sql: &str, params: &[(&str, &dyn rusqlite::ToSql)]) -> rusqlite::Result<Vec<AnimeUpdateRow>> {
    c.prepare_cached(sql)?.query_map(params, AnimeUpdateRow::from_row)?.collect()
}

/// Updates with an upload date after `after`, newest fetch first, at most `limit` rows.
pub fn recent_updates(c: &Connection, after: i64, limit: i64) -> rusqlite::Result<Vec<AnimeUpdateRow>> {
    update_rows(c, RECENT_UPDATES, named_params! {":after": after, ":limit": limit})
}

/// As [`recent_updates`], restricted to episodes whose `seen` flag equals `seen`.
pub fn updates_by_seen(c: &Connection, seen: bool, after: i64, limit: i64) -> rusqlite::Result<Vec<AnimeUpdateRow>> {
    update_rows(c, UPDATES_BY_SEEN, named_params! {":seen": seen, ":after": after, ":limit": limit})
}

const STUB_SOURCE_UPSERT: &str = r#"INSERT INTO stub_sources(kind, id, lang, name)
VALUES ('anime', :id, :lang, :name)
ON CONFLICT(kind, id)
DO UPDATE
SET
    lang = :lang,
    name = :name"#;

const STUB_SOURCE_FIND_ONE: &str = "SELECT id AS _id, lang, name FROM stub_sources WHERE kind = 'anime' AND id = :id";

const STUB_SOURCE_FIND_ALL: &str = "SELECT id AS _id, lang, name FROM stub_sources WHERE kind = 'anime' ORDER BY rowid";

/// A row of `animesources`. The id is the signed 64-bit form of the source id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StubSource {
    pub id: i64,
    pub lang: String,
    pub name: String,
}

impl StubSource {
    fn from_row(r: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self { id: r.get("_id")?, lang: r.get("lang")?, name: r.get("name")? })
    }
}

/// Inserts a stub source, or overwrites its `lang` and `name` when the id exists.
pub fn upsert_stub_source(c: &Connection, id: i64, lang: &str, name: &str) -> rusqlite::Result<usize> {
    c.prepare_cached(STUB_SOURCE_UPSERT)?.execute(named_params! {":id": id, ":lang": lang, ":name": name})
}

pub fn stub_source(c: &Connection, id: i64) -> rusqlite::Result<Option<StubSource>> {
    c.prepare_cached(STUB_SOURCE_FIND_ONE)?
        .query_map(named_params! {":id": id}, StubSource::from_row)?
        .next()
        .transpose()
}

pub fn stub_sources(c: &Connection) -> rusqlite::Result<Vec<StubSource>> {
    c.prepare_cached(STUB_SOURCE_FIND_ALL)?.query_map([], StubSource::from_row)?.collect()
}

/// Every anime column (old names) plus the group `name` of each related row, in `sort_order`.
pub const RELATED_BY_ANIME_ID: &str = concat!(
    "SELECT ", anime_cols!("A."), ", R.name FROM anime_relations AS R JOIN entries AS A ON A._id = R.related_id AND A.kind = 'anime' WHERE R.entry_id = :animeId ORDER BY R.sort_order ASC"
);

const RELATION_LAST_FETCHED_AT: &str = "SELECT last_fetched_at FROM anime_relations WHERE entry_id = :animeId LIMIT 1";

const RELATION_INSERT: &str = "INSERT OR REPLACE INTO anime_relations(entry_id, related_id, name, sort_order, last_fetched_at) VALUES (:animeId, :relatedAnimeId, :name, :sortOrder, :lastFetchedAt)";

const RELATION_DELETE_BY_ANIME_ID: &str = "DELETE FROM anime_relations WHERE entry_id = :animeId";

/// Replaces the relation groups of one anime in one transaction. `sort_order` counts up from 0
/// across all groups, in the order given.
pub fn replace_relations(
    c: &Connection,
    anime_id: i64,
    groups: Vec<(&str, Vec<i64>)>,
    fetched_at: i64,
) -> rusqlite::Result<()> {
    let tx = c.unchecked_transaction()?;
    tx.prepare_cached(RELATION_DELETE_BY_ANIME_ID)?.execute(named_params! {":animeId": anime_id})?;
    let mut sort_order = 0_i64;
    for (name, related) in groups {
        for related_anime_id in related {
            tx.prepare_cached(RELATION_INSERT)?.execute(named_params! {
                ":animeId": anime_id,
                ":relatedAnimeId": related_anime_id,
                ":name": name,
                ":sortOrder": sort_order,
                ":lastFetchedAt": fetched_at,
            })?;
            sort_order += 1;
        }
    }
    tx.commit()
}

/// The related anime of one anime, grouped by relation name. Groups appear in the order of their
/// first `sort_order`, and the anime of each group in `sort_order`.
pub fn related_groups(c: &Connection, anime_id: i64) -> rusqlite::Result<Vec<(String, Vec<AnimeRow>)>> {
    let mut groups: Vec<(String, Vec<AnimeRow>)> = Vec::new();
    let mut st = c.prepare_cached(RELATED_BY_ANIME_ID)?;
    let rows = st.query_map(named_params! {":animeId": anime_id}, |r| {
        Ok((r.get::<_, String>("name")?, AnimeRow::from_row(r)?))
    })?;
    for row in rows {
        let (name, anime) = row?;
        match groups.iter_mut().find(|(n, _)| *n == name) {
            Some((_, list)) => list.push(anime),
            None => groups.push((name, vec![anime])),
        }
    }
    Ok(groups)
}

/// `last_fetched_at` of an arbitrary relation row of this anime, or `None` when it has none.
pub fn relations_last_fetched_at(c: &Connection, anime_id: i64) -> rusqlite::Result<Option<i64>> {
    c.prepare_cached(RELATION_LAST_FETCHED_AT)?
        .query_map(named_params! {":animeId": anime_id}, |r| r.get(0))?
        .next()
        .transpose()
}

const ANIME_CHILDREN: &str = concat!("SELECT ", anime_cols!(""), " FROM entries WHERE parent_id = :parentId AND kind = 'anime'");

const ANIME_DUPLICATE_LIBRARY: &str = concat!("SELECT ", anime_cols!(""), " FROM entries WHERE kind = 'anime' AND favorite = 1 AND kuro_lower(title) = :titleLowercased AND _id != :id");

const ANIME_SOURCE_FAVORITE_COUNTS: &str = "SELECT source, count(*) FROM entries WHERE kind = 'anime' AND favorite = 1 GROUP BY source ORDER BY source";

const ANIME_UPCOMING: &str = concat!(
    "SELECT ", anime_cols!(""), " FROM entries WHERE kind = 'anime' AND next_update >= :startOfDay AND next_update > 0 AND favorite = 1 AND status IN (SELECT value FROM json_each(:statuses)) ORDER BY next_update ASC"
);

/// The ids an anime clear removes for one non-library entry `root`, and the ids it orphans. The
/// root is always removed. Below it, the Kotlin recursion of `ClearAnimeDatabaseScreenModel`
/// applies: a favourite child is orphaned, a non-favourite child is removed, and a non-favourite
/// child with `fetch_type = Seasons` is searched for its own children.
pub fn clearable_tree(c: &Connection, root: i64) -> rusqlite::Result<(Vec<i64>, Vec<i64>)> {
    let (children, orphaned) = clearable_children(c, root)?;
    let mut ids = vec![root];
    ids.extend(children);
    Ok((ids, orphaned))
}

fn clearable_children(c: &Connection, parent: i64) -> rusqlite::Result<(Vec<i64>, Vec<i64>)> {
    let mut ids = Vec::new();
    let mut orphaned = Vec::new();
    for child in anime_rows(c, ANIME_CHILDREN, named_params! {":parentId": parent})? {
        if child.favorite {
            orphaned.push(child.id);
            continue;
        }
        ids.push(child.id);
        if child.fetch_type == FetchType::Seasons {
            let (sub_ids, sub_orphaned) = clearable_children(c, child.id)?;
            ids.extend(sub_ids);
            orphaned.extend(sub_orphaned);
        }
    }
    Ok((ids, orphaned))
}

/// Non-library anime counted per source: `(source, count)` for every source with a non-favourite row.
pub fn non_library_counts_by_source(c: &Connection) -> rusqlite::Result<Vec<(i64, i64)>> {
    let mut st = c.prepare_cached("SELECT source, COUNT(*) FROM entries WHERE kind = 'anime' AND favorite = 0 GROUP BY source ORDER BY source;")?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

/// Clears the non-library anime of `source_ids` inside the caller's transaction: deletes their
/// non-favourite rows (their episodes and history go with them), orphans the favourite seasons that
/// pointed at a deleted row (`parent_id` NULL, as the Kotlin clear does), and removes the reset history.
/// An empty list changes nothing.
pub fn clear_non_library_anime_by_sources(tx: &rusqlite::Transaction<'_>, source_ids: &[i64]) -> rusqlite::Result<()> {
    if source_ids.is_empty() {
        return Ok(());
    }
    let ids = query_ids(tx, &format!("SELECT _id FROM entries WHERE kind = 'anime' AND favorite = 0 AND source IN ({})", in_placeholders(source_ids.len())), source_ids)?;
    for chunk in ids.chunks(IN_CHUNK) {
        let sql = format!("DELETE FROM entries WHERE kind = 'anime' AND favorite = 0 AND _id IN ({})", in_placeholders(chunk.len()));
        tx.prepare(&sql)?.execute(params_from_iter(chunk))?;
    }
    let orphaned = if ids.is_empty() {
        Vec::new()
    } else {
        query_ids(tx, &format!("SELECT _id FROM entries WHERE kind = 'anime' AND favorite = 1 AND parent_id IN ({})", in_placeholders(ids.len())), &ids)?
    };
    for chunk in orphaned.chunks(IN_CHUNK) {
        let sql = format!("UPDATE entries SET parent_id = NULL WHERE kind = 'anime' AND _id IN ({})", in_placeholders(chunk.len()));
        tx.prepare(&sql)?.execute(params_from_iter(chunk))?;
    }
    tx.execute(HISTORY_REMOVE_RESETTED, [])?;
    Ok(())
}

fn query_ids(tx: &rusqlite::Transaction<'_>, sql: &str, ids: &[i64]) -> rusqlite::Result<Vec<i64>> {
    let mut st = tx.prepare(sql)?;
    let rows = st.query_map(params_from_iter(ids), |r| r.get(0))?;
    rows.collect()
}

/// The clear step, in one transaction. Deletes the non-favourite anime in `ids` (chunks of 500),
/// sets `parent_id` to NULL on the `orphaned` ids that were not deleted, then deletes the reset
/// history rows (`last_seen = 0`).
pub fn clear_non_library_anime(c: &Connection, ids: &[i64], orphaned: &[i64]) -> rusqlite::Result<()> {
    let tx = c.unchecked_transaction()?;
    for chunk in ids.chunks(IN_CHUNK) {
        let sql = format!("DELETE FROM entries WHERE kind = 'anime' AND favorite = 0 AND _id IN ({})", in_placeholders(chunk.len()));
        tx.prepare(&sql)?.execute(params_from_iter(chunk))?;
    }
    let deleted: std::collections::HashSet<i64> = ids.iter().copied().collect();
    let kept: Vec<i64> = orphaned.iter().copied().filter(|id| !deleted.contains(id)).collect();
    for chunk in kept.chunks(IN_CHUNK) {
        let sql = format!("UPDATE entries SET parent_id = NULL WHERE kind = 'anime' AND _id IN ({})", in_placeholders(chunk.len()));
        tx.prepare(&sql)?.execute(params_from_iter(chunk))?;
    }
    tx.execute(HISTORY_REMOVE_RESETTED, [])?;
    tx.commit()
}

/// The first library anime other than `id` whose title, lowercased with `kuro_lower` (Rust's
/// `str::to_lowercase`), equals `title_lower`. The caller passes the lowercased title it searches for.
pub fn get_duplicate_library_anime(c: &Connection, id: i64, title_lower: &str) -> rusqlite::Result<Option<AnimeRow>> {
    Ok(anime_rows(c, ANIME_DUPLICATE_LIBRARY, named_params! {":id": id, ":titleLowercased": title_lower})?
        .into_iter()
        .next())
}

/// `(source, count)` of the library anime, one row per source, ordered by source.
pub fn source_favorite_counts(c: &Connection) -> rusqlite::Result<Vec<(i64, i64)>> {
    c.prepare_cached(ANIME_SOURCE_FAVORITE_COUNTS)?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
}

/// Library anime whose `next_update` is at or after `start_of_day_ms` (local midnight, epoch
/// milliseconds) and whose status is in `statuses`, soonest first. A NULL or zero `next_update`
/// never matches, and an empty status list matches nothing.
pub fn upcoming_animes(c: &Connection, statuses: &[i64], start_of_day_ms: i64) -> rusqlite::Result<Vec<AnimeRow>> {
    let json = format!("[{}]", statuses.iter().map(i64::to_string).collect::<Vec<_>>().join(","));
    anime_rows(c, ANIME_UPCOMING, named_params! {":startOfDay": start_of_day_ms, ":statuses": json})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_unified::open_in_memory;

    fn now_secs(c: &Connection) -> i64 {
        c.query_row("SELECT CAST(strftime('%s', 'now') AS INTEGER)", [], |r| r.get(0)).unwrap()
    }

    fn sample_anime(url: &str, favorite: bool) -> AnimeRow {
        AnimeRow {
            source: 1,
            url: url.into(),
            title: format!("T {url}"),
            status: 1,
            favorite,
            initialized: true,
            season_number: -1.0,
            ..Default::default()
        }
    }

    fn sample_episode(anime_id: i64, url: &str) -> EpisodeRow {
        EpisodeRow { anime_id, url: url.into(), name: format!("E {url}"), episode_number: 1.0, ..Default::default() }
    }

    #[test]
    fn every_statement_prepares_against_the_fresh_schema() {
        let c = open_in_memory().unwrap();
        for sql in [
            ANIME_BY_ID,
            ANIME_BY_URL_AND_SOURCE,
            ANIME_FAVORITES,
            ANIME_WATCHED_NOT_IN_LIBRARY,
            ANIME_FAVORITES_BY_SOURCE,
            ANIME_RESET_VIEWER_FLAGS,
            ANIME_SET_VIEWER_BITS,
            ANIME_INSERT,
            ANIME_UPDATE,
            EPISODE_BY_ID,
            EPISODES_BY_ANIME_ID,
            BOOKMARKED_EPISODES_BY_ANIME_ID,
            EPISODE_BY_URL,
            EPISODE_BY_URL_AND_ANIME_ID,
            EPISODE_INSERT,
            EPISODE_UPDATE,
            HISTORY_RESET_BY_ID,
            HISTORY_RESET_BY_ANIME_ID,
            HISTORY_DELETE_ALL,
            HISTORY_REMOVE_RESETTED,
            HISTORY_UPSERT,
            HISTORY_LIST,
        ] {
            c.prepare(sql).unwrap_or_else(|e| panic!("{e}: {sql}"));
        }
    }

    #[test]
    fn anime_insert_then_read_round_trips_every_column() {
        let c = open_in_memory().unwrap();
        let row = AnimeRow {
            id: 1,
            source: 42,
            url: "/show/1".into(),
            artist: Some("A".into()),
            author: Some("B".into()),
            description: Some("D".into()),
            genre: Some(GenreList(vec!["Action".into(), "Sci-Fi".into()])),
            title: "Title".into(),
            status: 2,
            thumbnail_url: Some("t".into()),
            favorite: true,
            last_update: Some(10),
            next_update: Some(20),
            initialized: true,
            viewer: 7,
            episode_flags: 0x40,
            cover_last_modified: 30,
            date_added: 40,
            update_strategy: UpdateStrategy::OnlyFetchOnce,
            calculate_interval: 3,
            last_modified_at: 0,
            favorite_modified_at: None,
            version: 5,
            is_syncing: 0,
            fetch_type: FetchType::Seasons,
            parent_id: Some(9),
            season_flags: 0x2,
            season_number: 2.5,
            season_source_order: 4,
            background_url: Some("bg".into()),
            background_last_modified: 50,
            memo: Memo(serde_json::json!({"k": 1}).as_object().unwrap().clone()),
        };
        let id = insert_anime(&c, &row).unwrap();
        assert_eq!(id, 1);
        assert_eq!(anime_by_id(&c, 1).unwrap(), Some(row));
        assert_eq!(anime_by_id(&c, 2).unwrap(), None);
        let stored: String = c.query_row("SELECT genre FROM entries WHERE kind = 'anime'", [], |r| r.get(0)).unwrap();
        assert_eq!(stored, "Action, Sci-Fi");
        let memo: Vec<u8> = c.query_row("SELECT memo FROM entries WHERE kind = 'anime'", [], |r| r.get(0)).unwrap();
        assert_eq!(memo, br#"{"k":1}"#);
    }

    #[test]
    fn anime_update_with_only_title_keeps_other_columns_and_stamps() {
        let c = open_in_memory().unwrap();
        let id = insert_anime(&c, &sample_anime("/a", false)).unwrap();
        let before = anime_by_id(&c, id).unwrap().unwrap();
        let n = update_anime(&c, &AnimeUpdate { id, title: Some("New".into()), ..Default::default() }).unwrap();
        assert_eq!(n, 1);
        let after = anime_by_id(&c, id).unwrap().unwrap();
        assert_eq!(after.title, "New");
        // The statement lists `favorite`, so `update_last_favorite_at_animes` stamps it too.
        let mut expected = after.clone();
        expected.title = before.title.clone();
        expected.last_modified_at = before.last_modified_at;
        expected.favorite_modified_at = before.favorite_modified_at;
        assert_eq!(expected, before);
        assert!((now_secs(&c) - after.last_modified_at).abs() <= 2);
        assert!((now_secs(&c) - after.favorite_modified_at.unwrap()).abs() <= 2);
        assert_eq!(after.version, 0);
    }

    #[test]
    fn anime_update_with_every_field_none_stamps_favorite() {
        let c = open_in_memory().unwrap();
        let id = insert_anime(&c, &sample_anime("/a", false)).unwrap();
        update_anime(&c, &AnimeUpdate { id, ..Default::default() }).unwrap();
        let row = anime_by_id(&c, id).unwrap().unwrap();
        assert_eq!(row.is_syncing, 0);
        assert!((now_secs(&c) - row.favorite_modified_at.unwrap()).abs() <= 2);
        assert!((now_secs(&c) - row.last_modified_at).abs() <= 2);
    }

    #[test]
    fn empty_genre_list_is_stored_as_empty_text_and_reads_back_empty() {
        let c = open_in_memory().unwrap();
        let id = insert_anime(&c, &sample_anime("/a", false)).unwrap();
        update_anime(&c, &AnimeUpdate { id, genre: Some(GenreList(vec![])), ..Default::default() }).unwrap();
        let stored: String = c.query_row("SELECT genre FROM entries WHERE kind = 'anime'", [], |r| r.get(0)).unwrap();
        assert_eq!(stored, "");
        assert_eq!(anime_by_id(&c, id).unwrap().unwrap().genre, Some(GenreList(vec![])));
    }

    #[test]
    fn lookups_by_url_source_and_favorite() {
        let c = open_in_memory().unwrap();
        insert_anime(&c, &sample_anime("/a", true)).unwrap();
        assert!(insert_anime(&c, &sample_anime("/a", true)).is_err(), "UNIQUE (kind, source, url)");
        insert_anime(&c, &sample_anime("/a2", true)).unwrap();
        let mut other = sample_anime("/b", true);
        other.source = 2;
        insert_anime(&c, &other).unwrap();
        insert_anime(&c, &sample_anime("/c", false)).unwrap();
        assert_eq!(anime_by_url_and_source(&c, "/a", 1).unwrap().unwrap().url, "/a");
        assert!(anime_by_url_and_source(&c, "/c", 1).unwrap().is_some());
        assert!(anime_by_url_and_source(&c, "/a", 9).unwrap().is_none());
        assert_eq!(favorite_animes(&c).unwrap().len(), 3);
        assert_eq!(favorite_animes_by_source(&c, 1).unwrap().len(), 2);
        assert_eq!(favorite_animes_by_source(&c, 2).unwrap().len(), 1);
    }

    #[test]
    fn watched_not_in_library_lists_non_favorites_with_progress() {
        let c = open_in_memory().unwrap();
        let seen = insert_anime(&c, &sample_anime("/50", false)).unwrap();
        let progress = insert_anime(&c, &sample_anime("/51", false)).unwrap();
        insert_anime(&c, &sample_anime("/52", false)).unwrap();
        let fav = insert_anime(&c, &sample_anime("/53", true)).unwrap();
        let mut e = sample_episode(seen, "e1");
        e.seen = true;
        insert_episode(&c, &e).unwrap();
        let mut e = sample_episode(progress, "e2");
        e.last_second_seen = 5;
        insert_episode(&c, &e).unwrap();
        let mut e = sample_episode(fav, "e3");
        e.seen = true;
        insert_episode(&c, &e).unwrap();
        let ids: Vec<i64> = watched_not_in_library(&c).unwrap().iter().map(|a| a.id).collect();
        assert_eq!(ids, vec![seen, progress]);
    }

    #[test]
    fn reset_viewer_flags_zeroes_every_row() {
        let c = open_in_memory().unwrap();
        let mut row = sample_anime("/a", false);
        row.viewer = 99;
        insert_anime(&c, &row).unwrap();
        assert_eq!(reset_viewer_flags(&c).unwrap(), 1);
        assert_eq!(anime_by_id(&c, 1).unwrap().unwrap().viewer, 0);
    }

    #[test]
    fn episode_insert_then_read_round_trips_every_column() {
        let c = open_in_memory().unwrap();
        let anime = insert_anime(&c, &sample_anime("/a", true)).unwrap();
        let row = EpisodeRow {
            id: 1,
            anime_id: anime,
            url: "/e/1".into(),
            name: "Episode 1".into(),
            scanlator: Some("S".into()),
            seen: true,
            bookmark: true,
            last_second_seen: 30,
            total_seconds: 1400,
            episode_number: 1.5,
            source_order: 3,
            date_fetch: 100,
            date_upload: 90,
            last_modified_at: 0,
            version: 2,
            is_syncing: 0,
            summary: Some("sum".into()),
            preview_url: Some("p".into()),
            fillermark: true,
            memo: Memo::default(),
        };
        assert_eq!(insert_episode(&c, &row).unwrap(), 1);
        assert_eq!(episode_by_id(&c, 1).unwrap(), Some(row.clone()));
        assert_eq!(episodes_by_anime_id(&c, anime).unwrap(), vec![row.clone()]);
        assert_eq!(bookmarked_episodes_by_anime_id(&c, anime).unwrap(), vec![row.clone()]);
        assert_eq!(episodes_by_url(&c, "/e/1").unwrap(), vec![row.clone()]);
        assert_eq!(episodes_by_url_and_anime_id(&c, "/e/1", anime).unwrap(), vec![row]);
        assert!(episodes_by_url_and_anime_id(&c, "/e/1", anime + 1).unwrap().is_empty());
    }

    #[test]
    fn episode_update_changes_only_given_columns() {
        let c = open_in_memory().unwrap();
        let anime = insert_anime(&c, &sample_anime("/a", true)).unwrap();
        insert_episode(&c, &sample_episode(anime, "e1")).unwrap();
        let n = update_episode(
            &c,
            &EpisodeUpdate { id: 1, name: Some("New".into()), last_second_seen: Some(12), ..Default::default() },
        )
        .unwrap();
        assert_eq!(n, 1);
        let row = episode_by_id(&c, 1).unwrap().unwrap();
        assert_eq!((row.name.as_str(), row.last_second_seen, row.is_syncing), ("New", 12, 0));
        assert_eq!(row.url, "e1");
    }

    #[test]
    fn remove_episodes_with_ids_deletes_in_chunks_of_500() {
        let c = open_in_memory().unwrap();
        let anime = insert_anime(&c, &sample_anime("/a", true)).unwrap();
        c.execute_batch("BEGIN").unwrap();
        for i in 0..1201 {
            insert_episode(&c, &sample_episode(anime, &format!("e{i}"))).unwrap();
        }
        c.execute_batch("COMMIT").unwrap();
        let ids: Vec<i64> = (1..=1201).collect();
        assert_eq!(remove_episodes_with_ids(&c, &ids).unwrap(), 1201);
        assert!(episodes_by_anime_id(&c, anime).unwrap().is_empty());
        assert_eq!(remove_episodes_with_ids(&c, &[]).unwrap(), 0);
    }

    #[test]
    fn deleting_an_anime_cascades_to_its_episodes() {
        let c = open_in_memory().unwrap();
        let anime = insert_anime(&c, &sample_anime("/a", true)).unwrap();
        insert_episode(&c, &sample_episode(anime, "e1")).unwrap();
        c.execute("DELETE FROM entries WHERE _id = ?1", [anime]).unwrap();
        assert!(episodes_by_anime_id(&c, anime).unwrap().is_empty());
    }

    /// Anime 1 `Alpha` with episodes 1 to 3 and history 1000, 3000, 2000 (`p1-edge-cases` V1).
    fn history_fixture(c: &Connection) -> i64 {
        let mut a = sample_anime("/alpha", true);
        a.title = "Alpha".into();
        let anime = insert_anime(c, &a).unwrap();
        for n in 1..=3 {
            insert_episode(c, &sample_episode(anime, &format!("e{n}"))).unwrap();
        }
        upsert_history(c, 1, Millis(1000)).unwrap();
        upsert_history(c, 2, Millis(3000)).unwrap();
        upsert_history(c, 3, Millis(2000)).unwrap();
        anime
    }

    fn history_row_id(c: &Connection, episode_id: i64) -> i64 {
        c.query_row("SELECT _id FROM history WHERE item_id = ?1", [episode_id], |r| r.get(0)).unwrap()
    }

    #[test]
    fn upsert_history_keeps_one_row_per_episode() {
        let c = open_in_memory().unwrap();
        history_fixture(&c);
        upsert_history(&c, 1, Millis(5000)).unwrap();
        let (rows, last): (i64, i64) = c
            .query_row(
                "SELECT count(*), max(last_at) FROM history WHERE item_id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((rows, last), (1, 5000));
    }

    #[test]
    fn reset_then_upsert_revives_the_history_row() {
        let c = open_in_memory().unwrap();
        history_fixture(&c);
        let id = history_row_id(&c, 1);
        reset_history(&c, id).unwrap();
        upsert_history(&c, 1, Millis(7000)).unwrap();
        let (rows, last): (i64, i64) = c
            .query_row(
                "SELECT count(*), max(last_at) FROM history WHERE item_id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((rows, last), (1, 7000));
        assert_eq!(history_row_id(&c, 1), id);
    }

    #[test]
    fn history_shows_the_newest_row_per_anime_and_the_next_one_after_a_reset() {
        let c = open_in_memory().unwrap();
        history_fixture(&c);
        let entries = history(&c, "").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!((entries[0].episode_id, entries[0].seen_at, entries[0].title.as_str()), (2, Millis(3000), "Alpha"));
        reset_history(&c, history_row_id(&c, 2)).unwrap();
        let entries = history(&c, "").unwrap();
        assert_eq!((entries[0].episode_id, entries[0].seen_at), (3, Millis(2000)));
        reset_history(&c, history_row_id(&c, 3)).unwrap();
        reset_history(&c, history_row_id(&c, 1)).unwrap();
        assert!(history(&c, "").unwrap().is_empty());
        assert_eq!(remove_resetted_history(&c).unwrap(), 3);
    }

    #[test]
    fn history_search_is_case_insensitive_and_escapes_wildcards() {
        let c = open_in_memory().unwrap();
        history_fixture(&c);
        let mut pct = sample_anime("/pct", true);
        pct.title = "100% Pure".into();
        let pct_anime = insert_anime(&c, &pct).unwrap();
        insert_episode(&c, &sample_episode(pct_anime, "p1")).unwrap();
        upsert_history(&c, 4, Millis(9000)).unwrap();
        let titles = |q: &str| -> Vec<String> {
            history(&c, q).unwrap().into_iter().map(|h| h.title).collect()
        };
        assert_eq!(titles("alp"), vec!["Alpha"]);
        assert_eq!(titles("ALP"), vec!["Alpha"]);
        assert_eq!(titles("%"), vec!["100% Pure"]);
        assert!(titles("zzz").is_empty());
        assert!(titles("_").is_empty());
    }

    #[test]
    fn history_search_folds_non_ascii_case_like_kuro_lower() {
        let c = open_in_memory().unwrap();
        let eclair = insert_anime(&c, &AnimeRow { title: "Éclair".into(), ..sample_anime("/e", true) }).unwrap();
        insert_episode(&c, &sample_episode(eclair, "e1")).unwrap();
        upsert_history(&c, 1, Millis(5000)).unwrap();
        let titles = |q: &str| -> Vec<String> { history(&c, q).unwrap().into_iter().map(|h| h.title).collect() };
        assert_eq!(titles("éclair"), vec!["Éclair"]);
        assert_eq!(titles("ÉCLAIR"), vec!["Éclair"]);
        assert!(titles("eclair").is_empty(), "the ASCII fold alone must not match a non-ASCII letter");
    }

    #[test]
    fn reset_by_anime_then_delete_all_empties_the_history() {
        let c = open_in_memory().unwrap();
        let anime = history_fixture(&c);
        assert_eq!(reset_history_by_anime(&c, anime).unwrap(), 3);
        assert!(history(&c, "").unwrap().is_empty());
        upsert_history(&c, 1, Millis(1)).unwrap();
        assert_eq!(delete_all_history(&c).unwrap(), 3);
        let rows: i64 = c.query_row("SELECT count(*) FROM history", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 0);
    }

    fn library_anime(c: &Connection, url: &str) -> i64 {
        insert_anime(c, &sample_anime(url, true)).unwrap()
    }

    fn version_of(c: &Connection, anime_id: i64) -> i64 {
        c.query_row("SELECT version FROM entries WHERE _id = ?1", [anime_id], |r| r.get(0)).unwrap()
    }

    #[test]
    fn add_all_episodes_returns_ids_in_insertion_order() {
        let c = open_in_memory().unwrap();
        let a = library_anime(&c, "/a");
        let batch: Vec<EpisodeRow> = (1..=5).map(|n| sample_episode(a, &format!("/e{n}"))).collect();
        let added = add_all_episodes(&c, &batch);
        assert_eq!(added.len(), 5);
        let ids: Vec<i64> = added.iter().map(|e| e.id).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "ids follow insertion order: {ids:?}");
        assert_eq!(added[2].url, "/e3");
        assert_eq!(episodes_by_anime_id(&c, a).unwrap().len(), 5);
    }

    #[test]
    fn add_all_episodes_rolls_back_when_one_row_fails() {
        let c = open_in_memory().unwrap();
        let a = library_anime(&c, "/a");
        let batch = [
            sample_episode(a, "/e1"),
            sample_episode(a, "/e2"),
            sample_episode(999, "/missing-anime"),
            sample_episode(a, "/e4"),
            sample_episode(a, "/e5"),
        ];
        assert!(add_all_episodes(&c, &batch).is_empty());
        assert!(episodes_by_anime_id(&c, a).unwrap().is_empty(), "the earlier rows were rolled back");
    }

    #[test]
    fn update_all_episodes_changes_only_the_listed_fields() {
        let c = open_in_memory().unwrap();
        let a = library_anime(&c, "/a");
        let added = add_all_episodes(&c, &[sample_episode(a, "/e1"), sample_episode(a, "/e2")]);
        let updates: Vec<EpisodeUpdate> = added
            .iter()
            .map(|e| EpisodeUpdate { id: e.id, name: Some(format!("renamed {}", e.id)), ..Default::default() })
            .collect();
        assert_eq!(update_all_episodes(&c, &updates).unwrap(), 2);
        for (before, _) in added.iter().zip(&updates) {
            let row = episode_by_id(&c, before.id).unwrap().unwrap();
            assert_eq!(row.name, format!("renamed {}", before.id));
            assert_eq!(row.url, before.url, "url untouched");
        }
    }

    #[test]
    fn categories_create_hide_delete_and_keep_the_system_category() {
        let c = open_in_memory().unwrap();
        let c1 = insert_category(&c, "one", 1, 0).unwrap();
        let c2 = insert_category(&c, "two", 2, 0).unwrap();
        let c3 = insert_category(&c, "three", 3, 0).unwrap();
        assert_eq!(delete_category(&c, c2).unwrap(), 1);
        set_category_orders(&c, &[0, c1, c3]).unwrap();
        let orders: Vec<(i64, i64)> = categories(&c).unwrap().iter().map(|k| (k.id, k.order)).collect();
        assert_eq!(orders, [(0, 0), (c1, 1), (c3, 2)]);

        update_category(&c, c1, None, None, None, Some(true)).unwrap();
        let visible: Vec<i64> = visible_categories(&c).unwrap().iter().map(|k| k.id).collect();
        assert_eq!(visible, [0, c3], "the hidden category is left out");
        assert!(category_by_id(&c, c1).unwrap().unwrap().hidden);

        let err = delete_category(&c, 0).unwrap_err();
        assert!(err.to_string().contains("System category can't be deleted"), "{err}");
        assert!(category_by_id(&c, 0).unwrap().is_some());
    }

    #[test]
    fn update_all_category_flags_sets_every_row_and_none_changes_nothing() {
        let c = open_in_memory().unwrap();
        let c1 = insert_category(&c, "one", 1, 0).unwrap();
        update_all_category_flags(&c, Some(5)).unwrap();
        assert!(categories(&c).unwrap().iter().all(|k| k.flags == 5), "system and user rows both get the flags");
        update_all_category_flags(&c, None).unwrap();
        assert_eq!(category_by_id(&c, c1).unwrap().unwrap().flags, 5);
    }

    #[test]
    fn set_anime_categories_dedupes_replaces_and_bumps_version_once_per_id() {
        let c = open_in_memory().unwrap();
        let a = library_anime(&c, "/a");
        let v0 = version_of(&c, a);
        let c1 = insert_category(&c, "one", 1, 0).unwrap();
        let c2 = insert_category(&c, "two", 2, 0).unwrap();
        let c3 = insert_category(&c, "three", 3, 0).unwrap();

        set_anime_categories(&c, a, &[c1, c2, c2]).unwrap();
        assert_eq!(version_of(&c, a), v0 + 2, "two distinct ids, two bumps");
        let linked: Vec<i64> = categories_by_anime_id(&c, a).unwrap().iter().map(|k| k.id).collect();
        assert_eq!(linked, [c1, c2]);

        set_anime_categories(&c, a, &[c2, c3]).unwrap();
        assert_eq!(version_of(&c, a), v0 + 4);
        let linked: Vec<i64> = categories_by_anime_id(&c, a).unwrap().iter().map(|k| k.id).collect();
        assert_eq!(linked, [c2, c3], "the old link to category one is gone");
    }

    #[test]
    fn tracks_reinsert_gets_a_new_id_update_renames_and_delete_removes_one() {
        let c = open_in_memory().unwrap();
        let a = library_anime(&c, "/a");
        let track = |sync_id: i64| AnimeTrack {
            anime_id: a,
            sync_id,
            remote_id: 42,
            title: "Remote".into(),
            remote_url: "https://example.test/42".into(),
            total_episodes: 12,
            score: 8.5,
            last_episode_seen: 3.5,
            ..Default::default()
        };
        insert_tracks(&c, &[track(1), track(2)]).unwrap();
        let first = tracks_by_anime_id(&c, a).unwrap().into_iter().find(|t| t.sync_id == 1).unwrap();

        insert_tracks(&c, &[track(1)]).unwrap();
        let rows = tracks_by_anime_id(&c, a).unwrap();
        assert_eq!(rows.len(), 2, "the second row for the same tracker replaces the first");
        let again = rows.iter().find(|t| t.sync_id == 1).unwrap();
        assert_ne!(again.id, first.id, "the replacement has a new _id");

        update_track(
            &c,
            &AnimeTrackUpdate { id: again.id, title: Some("Renamed".into()), remote_id: Some(77), ..Default::default() },
        )
        .unwrap();
        let row = track_by_id(&c, again.id).unwrap().unwrap();
        assert_eq!(row.title, "Renamed");
        assert_eq!(row.remote_id, 77, "mediaId maps to remote_id");
        assert_eq!(row.remote_url, "https://example.test/42", "untouched when None");

        assert_eq!(delete_track(&c, a, 1).unwrap(), 1);
        let left = tracks(&c).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].sync_id, 2);
    }

    #[test]
    fn stub_source_upsert_overwrites_and_keeps_signed_ids() {
        let c = open_in_memory().unwrap();
        upsert_stub_source(&c, 7, "en", "Alpha").unwrap();
        upsert_stub_source(&c, 7, "ja", "Beta").unwrap();
        let s = stub_source(&c, 7).unwrap().unwrap();
        assert_eq!((s.lang.as_str(), s.name.as_str()), ("ja", "Beta"));
        assert_eq!(stub_sources(&c).unwrap().len(), 1);

        let big = (1_u64 << 63) + 5;
        upsert_stub_source(&c, big as i64, "en", "Big").unwrap();
        let stored = big as i64;
        assert!(stored < 0, "above 2^63 the stored value is negative");
        assert_eq!(stub_source(&c, stored).unwrap().unwrap().id, stored);
    }

    #[test]
    fn relations_group_in_sort_order_and_cascade_when_a_related_anime_is_deleted() {
        let c = open_in_memory().unwrap();
        let a1 = library_anime(&c, "/1");
        let a2 = library_anime(&c, "/2");
        let a3 = library_anime(&c, "/3");
        let a4 = library_anime(&c, "/4");
        replace_relations(&c, a1, vec![("Sequel", vec![a2, a3]), ("Spin-off", vec![a4])], 500).unwrap();

        let orders: Vec<i64> = c
            .prepare("SELECT sort_order FROM anime_relations WHERE entry_id = ?1 ORDER BY sort_order")
            .unwrap()
            .query_map([a1], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(orders, [0, 1, 2]);

        let groups = |c: &Connection| -> Vec<(String, Vec<i64>)> {
            related_groups(c, a1)
                .unwrap()
                .into_iter()
                .map(|(name, list)| (name, list.iter().map(|a| a.id).collect()))
                .collect()
        };
        assert_eq!(groups(&c), vec![("Sequel".to_string(), vec![a2, a3]), ("Spin-off".to_string(), vec![a4])]);
        assert_eq!(relations_last_fetched_at(&c, a1).unwrap(), Some(500));

        c.execute("DELETE FROM entries WHERE _id = ?1", [a3]).unwrap();
        assert_eq!(groups(&c), vec![("Sequel".to_string(), vec![a2]), ("Spin-off".to_string(), vec![a4])]);
    }

    #[test]
    fn clear_removes_the_non_library_tree_and_orphans_favourite_children() {
        let c = open_in_memory().unwrap();
        let p1 = insert_anime(&c, &AnimeRow { fetch_type: FetchType::Seasons, ..sample_anime("/p1", false) }).unwrap();
        let c1 = insert_anime(&c, &AnimeRow { parent_id: Some(p1), ..sample_anime("/c1", false) }).unwrap();
        let c2 = insert_anime(&c, &AnimeRow { parent_id: Some(p1), ..sample_anime("/c2", true) }).unwrap();
        let c3 = insert_anime(
            &c,
            &AnimeRow { parent_id: Some(p1), fetch_type: FetchType::Seasons, ..sample_anime("/c3", false) },
        )
        .unwrap();
        let g1 = insert_anime(&c, &AnimeRow { parent_id: Some(c3), ..sample_anime("/g1", false) }).unwrap();
        let lib = library_anime(&c, "/lib");

        let e = insert_episode(&c, &sample_episode(c2, "/c2e")).unwrap();
        upsert_history(&c, e, Millis(0)).unwrap();
        let e_read = insert_episode(&c, &sample_episode(c2, "/c2r")).unwrap();
        upsert_history(&c, e_read, Millis(900)).unwrap();

        let (ids, orphaned) = clearable_tree(&c, p1).unwrap();
        assert_eq!(ids, [p1, c1, c3, g1]);
        assert_eq!(orphaned, [c2]);

        clear_non_library_anime(&c, &ids, &orphaned).unwrap();
        for gone in [p1, c1, c3, g1] {
            assert!(anime_by_id(&c, gone).unwrap().is_none(), "anime {gone} removed");
        }
        assert_eq!(anime_by_id(&c, c2).unwrap().unwrap().parent_id, None, "favourite child kept, orphaned");
        assert!(anime_by_id(&c, lib).unwrap().is_some(), "library anime untouched");
        let history_rows: i64 = c.query_row("SELECT COUNT(*) FROM history", [], |r| r.get(0)).unwrap();
        assert_eq!(history_rows, 1, "only the reset row is removed");
    }

    #[test]
    fn duplicate_lookup_matches_non_ascii_titles_and_excludes_the_entry_itself() {
        let c = open_in_memory().unwrap();
        let eclair = insert_anime(&c, &AnimeRow { title: "Éclair".into(), ..sample_anime("/e", true) }).unwrap();
        let naruto = insert_anime(&c, &AnimeRow { title: "Naruto".into(), ..sample_anime("/n", true) }).unwrap();
        let other = insert_anime(&c, &AnimeRow { title: "Other".into(), ..sample_anime("/o", true) }).unwrap();

        assert_eq!(get_duplicate_library_anime(&c, other, "éclair").unwrap().map(|a| a.id), Some(eclair));
        assert_eq!(get_duplicate_library_anime(&c, other, &"NARUTO".to_lowercase()).unwrap().map(|a| a.id), Some(naruto));
        assert_eq!(get_duplicate_library_anime(&c, eclair, "éclair").unwrap().map(|a| a.id), None);
    }

    #[test]
    fn source_favorite_counts_count_only_library_anime_per_source() {
        let c = open_in_memory().unwrap();
        for (i, (source, favorite)) in [(1, true), (1, true), (1, true), (2, true), (3, false)].into_iter().enumerate() {
            insert_anime(&c, &AnimeRow { source, ..sample_anime(&format!("/{i}"), favorite) }).unwrap();
        }
        assert_eq!(source_favorite_counts(&c).unwrap(), [(1, 3), (2, 1)]);
    }

    #[test]
    fn clearing_by_sources_removes_only_non_library_rows_of_the_listed_sources() {
        let mut c = open_in_memory().unwrap();
        let in_library = |source: i64| AnimeRow { source, ..sample_anime("/keep", true) };
        let keep = insert_anime(&c, &in_library(1)).unwrap();
        let gone = insert_anime(&c, &AnimeRow { source: 2, ..sample_anime("/gone", false) }).unwrap();
        let other = insert_anime(&c, &AnimeRow { source: 3, ..sample_anime("/other", false) }).unwrap();
        assert_eq!(non_library_counts_by_source(&c).unwrap(), [(2, 1), (3, 1)]);
        let tx = c.transaction().unwrap();
        clear_non_library_anime_by_sources(&tx, &[]).unwrap();
        clear_non_library_anime_by_sources(&tx, &[2]).unwrap();
        tx.commit().unwrap();
        let mut st = c.prepare("SELECT _id FROM entries WHERE kind = 'anime' ORDER BY _id").unwrap();
        let left: Vec<i64> = st.query_map([], |r| r.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        assert_eq!(left, [keep, other], "the library row and the other source survive");
        assert!(!left.contains(&gone), "the listed source's non-library row is deleted");
    }
}
