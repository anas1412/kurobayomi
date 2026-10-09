//! Anime entries: the `Anime` and `AnimeUpdate` shapes, the view rows (library, seasons, deletable
//! parents), the five mappers (AnimeMapper.kt) and `AnimeRepo` (AnimeRepositoryImpl.kt).

use std::collections::HashSet;
use std::sync::Arc;

use kuro_db::codecs::{GenreList, UpdateStrategy, ids_json};
use kuro_db::handle::Db;

use crate::flags_write;
use kuro_db::schema_anime::{FetchType, Memo};
use kuro_db::statements_anime::{
    AnimeRow, AnimeUpdate as DbAnimeUpdate, anime_by_id, anime_by_url_and_source, clearable_tree, favorite_animes,
    favorite_animes_by_source, insert_anime, reset_viewer_flags, source_favorite_counts, update_anime,
    upcoming_animes, watched_not_in_library,
};
use kuro_db::subscribe::Subscription;
use rusqlite::{Connection, Row, ToSql};

use crate::{
    DataError, DataResult, ENTRY_DEBOUNCE, LIBRARY_DEBOUNCE, LIBRARY_RETRIES, LIBRARY_RETRY_DELAY, LibraryState,
    retrying, time,
};

/// Tables of one anime row (`subscribe_by_id`, `subscribe_by_url_and_source`, `subscribe_favorites_by_source`).
const ANIME_TABLE: &[&str] = &["animes"];

/// Tables an `animelibView` row reads (`subscribe_library`).
const ANIME_LIBRARY_TABLES: &[&str] = &["animes", "episodes", "animehistory", "animes_categories"];

/// Upcoming-tab statuses for anime: ONGOING (1), PUBLISHING_FINISHED (4) and ONGOING_ANNOUNCED (7),
/// as `GetUpcomingAnime` passes them.
pub const UPCOMING_STATUSES: [i64; 3] = [1, 4, 7];

const LIBRARY_VIEW: &str = "SELECT * FROM animelibView";
const SEASONS_BY_PARENT: &str = "SELECT * FROM animeseasonsView WHERE parent_id = :parentId";
const DELETABLE_PARENTS: &str = "SELECT * FROM animedeletableView";
const CHILDREN_BY_PARENT: &str = "SELECT * FROM animes WHERE parent_id = :parentId";
const SOURCE_AND_URL: &str = "SELECT source, url FROM animes";
/// Ids per `remove_parent_ids` statement; all statements run in one transaction.
const PARENT_ID_CHUNK: usize = 500;
const REMOVE_PARENT_IDS: &str =
    "UPDATE animes SET parent_id = NULL WHERE _id IN (SELECT value FROM json_each(?1))";
const DELETE_NOT_IN_LIBRARY: &str =
    "DELETE FROM animes WHERE favorite = 0 AND _id IN (SELECT value FROM json_each(?1))";
const CATEGORY_LINKS_DELETE: &str = "DELETE FROM animes_categories WHERE anime_id = ?1";
const CATEGORY_LINK_INSERT: &str = "INSERT INTO animes_categories(anime_id, category_id) VALUES (?1, ?2)";

/// One `animes` row as the domain sees it (`Anime.kt`). `is_syncing` is not exposed.
#[derive(Clone, Debug, PartialEq)]
pub struct Anime {
    pub id: i64,
    pub source: i64,
    pub url: String,
    pub title: String,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<Vec<String>>,
    pub status: i64,
    pub thumbnail_url: Option<String>,
    pub favorite: bool,
    pub last_update: i64,
    pub next_update: i64,
    pub fetch_interval: i32,
    pub date_added: i64,
    pub viewer_flags: i64,
    pub episode_flags: i64,
    pub cover_last_modified: i64,
    pub update_strategy: UpdateStrategy,
    pub initialized: bool,
    pub last_modified_at: i64,
    pub favorite_modified_at: Option<i64>,
    pub version: i64,
    pub background_last_modified: i64,
    pub background_url: Option<String>,
    pub fetch_type: FetchType,
    pub parent_id: Option<i64>,
    pub season_flags: i64,
    pub season_number: f64,
    pub season_source_order: i64,
    pub memo: serde_json::Map<String, serde_json::Value>,
}

impl Anime {
    /// `Anime.create()`: id -1, source -1, `Episodes`, season number -1.0, empty memo, version 0.
    pub fn create() -> Self {
        Self {
            id: -1,
            source: -1,
            url: String::new(),
            title: String::new(),
            artist: None,
            author: None,
            description: None,
            genre: None,
            status: 0,
            thumbnail_url: None,
            favorite: false,
            last_update: 0,
            next_update: 0,
            fetch_interval: 0,
            date_added: 0,
            viewer_flags: 0,
            episode_flags: 0,
            cover_last_modified: 0,
            update_strategy: UpdateStrategy::AlwaysUpdate,
            initialized: false,
            last_modified_at: 0,
            favorite_modified_at: None,
            version: 0,
            background_last_modified: 0,
            background_url: None,
            fetch_type: FetchType::Episodes,
            parent_id: None,
            season_flags: 0,
            season_number: -1.0,
            season_source_order: 0,
            memo: serde_json::Map::new(),
        }
    }
}

/// Partial update (`AnimeUpdate.kt`). `None` keeps the stored value. `id` is required: use
/// [`AnimeUpdate::new`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimeUpdate {
    pub id: i64,
    pub source: Option<i64>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<Vec<String>>,
    pub status: Option<i64>,
    pub thumbnail_url: Option<String>,
    pub favorite: Option<bool>,
    pub last_update: Option<i64>,
    pub next_update: Option<i64>,
    pub fetch_interval: Option<i32>,
    pub date_added: Option<i64>,
    pub viewer_flags: Option<i64>,
    pub episode_flags: Option<i64>,
    pub cover_last_modified: Option<i64>,
    pub update_strategy: Option<UpdateStrategy>,
    pub initialized: Option<bool>,
    pub version: Option<i64>,
    pub background_last_modified: Option<i64>,
    pub background_url: Option<String>,
    pub fetch_type: Option<FetchType>,
    pub parent_id: Option<i64>,
    pub season_flags: Option<i64>,
    pub season_number: Option<f64>,
    pub season_source_order: Option<i64>,
    pub memo: Option<serde_json::Map<String, serde_json::Value>>,
}

impl AnimeUpdate {
    pub fn new(id: i64) -> Self {
        Self { id, ..Default::default() }
    }
}

/// One `animelibView` row (`LibraryAnime.kt`): the anime, its category link and the aggregates
/// (episode counts for `Episodes`, season aggregates for `Seasons`).
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryAnime {
    pub anime: Anime,
    pub category: i64,
    pub total_count: i64,
    pub seen_count: i64,
    pub bookmark_count: i64,
    pub fillermark_count: i64,
    pub latest_upload: i64,
    pub episode_fetched_at: i64,
    pub last_seen: i64,
}

impl LibraryAnime {
    pub fn id(&self) -> i64 {
        self.anime.id
    }

    pub fn unseen_count(&self) -> i64 {
        self.total_count - self.seen_count
    }

    pub fn has_bookmarks(&self) -> bool {
        self.bookmark_count > 0
    }

    pub fn has_started(&self) -> bool {
        self.seen_count > 0
    }
}

/// One `animeseasonsView` row (`SeasonAnime.kt`): a child of a `Seasons` parent with its aggregates.
#[derive(Clone, Debug, PartialEq)]
pub struct SeasonAnime {
    pub anime: Anime,
    pub total_count: i64,
    pub seen_count: i64,
    pub bookmark_count: i64,
    pub fillermark_count: i64,
    pub latest_upload: i64,
    pub fetched_at: i64,
    pub last_seen: i64,
}

impl SeasonAnime {
    pub fn id(&self) -> i64 {
        self.anime.id
    }

    pub fn seen(&self) -> bool {
        self.total_count == self.seen_count
    }

    pub fn unseen_count(&self) -> i64 {
        self.total_count - self.seen_count
    }

    pub fn has_started(&self) -> bool {
        self.seen_count > 0
    }

    pub fn has_bookmarks(&self) -> bool {
        self.bookmark_count > 0
    }

    pub fn has_fillermarks(&self) -> bool {
        self.fillermark_count > 0
    }

    /// `toLibraryAnime()`: category -1 and the fetch date carried over.
    pub fn to_library_anime(&self) -> LibraryAnime {
        LibraryAnime {
            anime: self.anime.clone(),
            category: -1,
            total_count: self.total_count,
            seen_count: self.seen_count,
            bookmark_count: self.bookmark_count,
            fillermark_count: self.fillermark_count,
            latest_upload: self.latest_upload,
            episode_fetched_at: self.fetched_at,
            last_seen: self.last_seen,
        }
    }
}

/// One `animedeletableView` row (`DeletableAnime.kt`): a non-favourite top-level anime.
#[derive(Clone, Debug, PartialEq)]
pub struct DeletableEntry {
    pub anime_id: i64,
    pub source_id: i64,
    pub fetch_type: FetchType,
}

/// A named group of related anime (`AnimeRelationGroup.kt`).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimeRelationGroup {
    pub name: String,
    pub anime: Vec<Anime>,
}

/// `AnimeMapper.mapAnime`: the 31 domain fields of an `animes` row.
pub fn map_anime(r: &AnimeRow) -> Anime {
    Anime {
        id: r.id,
        source: r.source,
        url: r.url.clone(),
        title: r.title.clone(),
        artist: r.artist.clone(),
        author: r.author.clone(),
        description: r.description.clone(),
        genre: r.genre.as_ref().map(|g| g.0.clone()),
        status: r.status,
        thumbnail_url: r.thumbnail_url.clone(),
        favorite: r.favorite,
        last_update: r.last_update.unwrap_or(0),
        next_update: r.next_update.unwrap_or(0),
        fetch_interval: r.calculate_interval as i32,
        date_added: r.date_added,
        viewer_flags: r.viewer,
        episode_flags: r.episode_flags,
        cover_last_modified: r.cover_last_modified,
        update_strategy: r.update_strategy,
        initialized: r.initialized,
        last_modified_at: r.last_modified_at,
        favorite_modified_at: r.favorite_modified_at,
        version: r.version,
        background_last_modified: r.background_last_modified,
        background_url: r.background_url.clone(),
        fetch_type: r.fetch_type,
        parent_id: r.parent_id,
        season_flags: r.season_flags,
        season_number: r.season_number,
        season_source_order: r.season_source_order,
        memo: r.memo.0.clone(),
    }
}

/// One aggregate column of an anime view. The views pick their stats with a `CASE` on `fetch_type`
/// and no `ELSE`, so a `fetch_type` outside {0, 1} gives NULL. That reads as 0 (EC-16).
fn stat(r: &Row<'_>, name: &str) -> rusqlite::Result<i64> {
    Ok(r.get::<_, Option<i64>>(name)?.unwrap_or(0))
}

/// `AnimeMapper.mapLibraryAnime`: reads the anime columns and the `animelibView` aggregates by name.
/// NULL stat columns read as 0.
pub fn map_library_anime(r: &Row<'_>) -> rusqlite::Result<LibraryAnime> {
    Ok(LibraryAnime {
        anime: map_anime(&AnimeRow::from_row(r)?),
        category: r.get("category")?,
        total_count: stat(r, "totalCount")?,
        seen_count: stat(r, "seenCount")?,
        bookmark_count: stat(r, "bookmarkCount")?,
        fillermark_count: stat(r, "fillermarkCount")?,
        latest_upload: stat(r, "latestUpload")?,
        episode_fetched_at: stat(r, "episodeFetchedAt")?,
        last_seen: stat(r, "lastSeen")?,
    })
}

/// `AnimeMapper.mapSeasonAnime`: the `animeseasonsView` aggregates, which have no category column.
/// NULL stat columns read as 0.
pub fn map_season_anime(r: &Row<'_>) -> rusqlite::Result<SeasonAnime> {
    Ok(SeasonAnime {
        anime: map_anime(&AnimeRow::from_row(r)?),
        total_count: stat(r, "totalCount")?,
        seen_count: stat(r, "seenCount")?,
        bookmark_count: stat(r, "bookmarkCount")?,
        fillermark_count: stat(r, "fillermarkCount")?,
        latest_upload: stat(r, "latestUpload")?,
        fetched_at: stat(r, "episodeFetchedAt")?,
        last_seen: stat(r, "lastSeen")?,
    })
}

/// `AnimeMapper` for one row of the related query: every `animes` column plus the group `name` of
/// the `anime_relations` row that links it.
pub fn map_related_anime(r: &Row<'_>) -> rusqlite::Result<(String, Anime)> {
    Ok((r.get("name")?, map_anime(&AnimeRow::from_row(r)?)))
}

/// `AnimeMapper.mapDeletableAnime`: the three columns of `animedeletableView`.
pub fn map_deletable_anime(r: &Row<'_>) -> rusqlite::Result<DeletableEntry> {
    Ok(DeletableEntry {
        anime_id: r.get("_id")?,
        source_id: r.get("source")?,
        fetch_type: r.get("fetch_type")?,
    })
}

/// The non-library anime of every source, as `(source, anime id)` rows: `removable` holds each deletable parent
/// and the ids its clear removes, `orphaned` the favourite children that lose their parent. The children are
/// searched only under `Seasons` parents, as the clear screen does (`ClearAnimeDatabaseScreen.kt`). Feeds
/// `anime_listing`.
fn non_library_tree(c: &Connection) -> rusqlite::Result<(Vec<(i64, i64)>, Vec<(i64, i64)>)> {
    let mut removable = Vec::new();
    let mut orphaned = Vec::new();
    for parent in query(c, DELETABLE_PARENTS, &[], map_deletable_anime)? {
        let (ids, gone) = if parent.fetch_type == FetchType::Seasons {
            clearable_tree(c, parent.anime_id)?
        } else {
            (vec![parent.anime_id], Vec::new())
        };
        removable.extend(ids.into_iter().map(|id| (parent.source_id, id)));
        orphaned.extend(gone.into_iter().map(|id| (parent.source_id, id)));
    }
    Ok((removable, orphaned))
}

/// Row for the insert statement. The statement ignores `id`, `last_modified_at` and
/// `favorite_modified_at`.
fn to_row(a: &Anime) -> AnimeRow {
    AnimeRow {
        id: a.id,
        source: a.source,
        url: a.url.clone(),
        artist: a.artist.clone(),
        author: a.author.clone(),
        description: a.description.clone(),
        genre: a.genre.clone().map(GenreList),
        title: a.title.clone(),
        status: a.status,
        thumbnail_url: a.thumbnail_url.clone(),
        favorite: a.favorite,
        last_update: Some(a.last_update),
        next_update: Some(a.next_update),
        initialized: a.initialized,
        viewer: a.viewer_flags,
        episode_flags: a.episode_flags,
        cover_last_modified: a.cover_last_modified,
        date_added: a.date_added,
        update_strategy: a.update_strategy,
        calculate_interval: i64::from(a.fetch_interval),
        last_modified_at: 0,
        favorite_modified_at: None,
        version: a.version,
        is_syncing: 0,
        fetch_type: a.fetch_type,
        parent_id: a.parent_id,
        season_flags: a.season_flags,
        season_number: a.season_number,
        season_source_order: a.season_source_order,
        background_url: a.background_url.clone(),
        background_last_modified: a.background_last_modified,
        memo: Memo(a.memo.clone()),
    }
}

/// Coalesce patch. The statement always writes `is_syncing = 0` (see `update_anime`), so the
/// triggers fire on every update (EC-04).
fn to_update(u: &AnimeUpdate) -> DbAnimeUpdate {
    DbAnimeUpdate {
        id: u.id,
        source: u.source,
        url: u.url.clone(),
        artist: u.artist.clone(),
        author: u.author.clone(),
        description: u.description.clone(),
        genre: u.genre.clone().map(GenreList),
        title: u.title.clone(),
        status: u.status,
        thumbnail_url: u.thumbnail_url.clone(),
        favorite: u.favorite,
        last_update: u.last_update,
        next_update: u.next_update,
        initialized: u.initialized,
        viewer_flags: u.viewer_flags,
        episode_flags: u.episode_flags,
        cover_last_modified: u.cover_last_modified,
        date_added: u.date_added,
        update_strategy: u.update_strategy,
        fetch_interval: u.fetch_interval.map(i64::from),
        version: u.version,
        fetch_type: u.fetch_type,
        parent_id: u.parent_id,
        season_flags: u.season_flags,
        season_number: u.season_number,
        season_source_order: u.season_source_order,
        background_url: u.background_url.clone(),
        background_last_modified: u.background_last_modified,
        memo: u.memo.clone().map(Memo),
    }
}

/// Runs a query that returns rows of `f`, with named parameters.
fn query<T>(
    c: &Connection,
    sql: &str,
    params: &[(&str, &dyn ToSql)],
    f: fn(&Row<'_>) -> rusqlite::Result<T>,
) -> rusqlite::Result<Vec<T>> {
    c.prepare_cached(sql)?.query_map(params, f)?.collect()
}

/// Typed access to the anime tables. Shares one `Db` with the other repositories of the database.
#[derive(Clone)]
pub struct AnimeRepo {
    db: Arc<Db>,
}

impl AnimeRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// `NotFound` when no row has `id`.
    pub fn by_id(&self, id: i64) -> DataResult<Anime> {
        let row = self.db.read(|c| Ok(anime_by_id(c, id)?))?.ok_or(DataError::NotFound)?;
        Ok(map_anime(&row))
    }

    /// The first row (lowest `_id`) with this url and source.
    pub fn by_url_and_source(&self, url: &str, source: i64) -> DataResult<Option<Anime>> {
        let row = self.db.read(|c| Ok(anime_by_url_and_source(c, url, source)?))?;
        Ok(row.as_ref().map(map_anime))
    }

    /// The row with `id`, published again after each change to it. `None` once the row is deleted (EC-01).
    pub fn subscribe_by_id(&self, id: i64) -> Subscription<Option<Anime>> {
        self.db.subscribe(ANIME_TABLE, ENTRY_DEBOUNCE, move |c| {
            Ok(anime_by_id(c, id)?.as_ref().map(map_anime))
        })
    }

    /// The first row (lowest `_id`) with this url and source, published again after each change.
    pub fn subscribe_by_url_and_source(&self, url: &str, source: i64) -> Subscription<Option<Anime>> {
        let url = url.to_owned();
        self.db.subscribe(ANIME_TABLE, ENTRY_DEBOUNCE, move |c| {
            Ok(anime_by_url_and_source(c, &url, source)?.as_ref().map(map_anime))
        })
    }

    pub fn subscribe_favorites_by_source(&self, source: i64) -> Subscription<Vec<Anime>> {
        self.db.subscribe(ANIME_TABLE, ENTRY_DEBOUNCE, move |c| {
            Ok(favorite_animes_by_source(c, source)?.iter().map(map_anime).collect::<Vec<_>>())
        })
    }

    /// The `library()` rows, published again after a change to any table they read.
    pub fn subscribe_library(&self) -> Subscription<LibraryState<LibraryAnime>> {
        self.db.subscribe(
            ANIME_LIBRARY_TABLES,
            LIBRARY_DEBOUNCE,
            retrying(LIBRARY_RETRIES, LIBRARY_RETRY_DELAY, |c| Ok(query(c, LIBRARY_VIEW, &[], map_library_anime)?)),
        )
    }

    /// Season rows of one parent, published again after a change to any table they read.
    pub fn subscribe_seasons_by_parent(&self, parent_id: i64) -> Subscription<Vec<SeasonAnime>> {
        self.db.subscribe(ANIME_LIBRARY_TABLES, LIBRARY_DEBOUNCE, move |c| {
            Ok(query(c, SEASONS_BY_PARENT, &[(":parentId", &parent_id)], map_season_anime)?)
        })
    }

    pub fn favorites(&self) -> DataResult<Vec<Anime>> {
        let rows = self.db.read(|c| Ok(favorite_animes(c)?))?;
        Ok(rows.iter().map(map_anime).collect())
    }

    pub fn favorites_by_source(&self, source: i64) -> DataResult<Vec<Anime>> {
        let rows = self.db.read(|c| Ok(favorite_animes_by_source(c, source)?))?;
        Ok(rows.iter().map(map_anime).collect())
    }

    pub fn watched_not_in_library(&self) -> DataResult<Vec<Anime>> {
        let rows = self.db.read(|c| Ok(watched_not_in_library(c)?))?;
        Ok(rows.iter().map(map_anime).collect())
    }

    /// One row per (favourite anime, category link), with the aggregates chosen by `fetch_type`.
    pub fn library(&self) -> DataResult<Vec<LibraryAnime>> {
        Ok(self.db.read(|c| Ok(query(c, LIBRARY_VIEW, &[], map_library_anime)?))?)
    }

    /// Library anime other than `id` whose title equals `title`, both lowercased with
    /// `str::to_lowercase`. The comparison runs in Rust, so it is Unicode-correct.
    pub fn duplicates(&self, id: i64, title: &str) -> DataResult<Vec<Anime>> {
        let lowered = title.to_lowercase();
        let rows = self.db.read(|c| Ok(favorite_animes(c)?))?;
        Ok(rows
            .iter()
            .filter(|a| a.id != id && a.title.to_lowercase() == lowered)
            .map(map_anime)
            .collect())
    }

    /// Favourites whose `next_update` is at or after local midnight of `now_ms`'s day, soonest first.
    pub fn upcoming(&self, statuses: &[i64], now_ms: i64) -> DataResult<Vec<Anime>> {
        let start_of_day = time::start_of_local_day_millis(now_ms);
        let rows = self.db.read(|c| Ok(upcoming_animes(c, statuses, start_of_day)?))?;
        Ok(rows.iter().map(map_anime).collect())
    }

    /// Writes `value` under `mask` into the viewer flags of one anime, in one statement. `Ok(false)` when the anime does
    /// not exist.
    pub fn set_viewer_bits(&self, id: i64, value: i64, mask: i64) -> DataResult<bool> {
        self.db.write(|tx| Ok(kuro_db::statements_anime::set_viewer_bits(tx, id, value, mask)? > 0))
    }

    /// Sets `viewer = 0` on every anime. Not wired to any screen (EC-29).
    pub fn reset_viewer_flags(&self) -> DataResult<()> {
        self.db.write(|tx| Ok(reset_viewer_flags(tx).map(drop)?))
    }

    /// Replaces the category links of one anime in one transaction. A repeated id is linked once.
    /// Sets the `mask` bits of `episode_flags` to those of `flag` in one UPDATE. Returns whether the row changed.
    pub fn set_episode_flags_masked(&self, anime_id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        flags_write::masked(&self.db, flags_write::ANIME_EPISODE_MASKED, anime_id, flag, mask)
    }

    /// Reads `episode_flags`, applies `change` and stores the result in the writer transaction.
    pub fn update_episode_flags(&self, anime_id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool> {
        flags_write::read_modify_write(
            &self.db,
            flags_write::ANIME_EPISODE_READ,
            flags_write::ANIME_EPISODE_WRITE,
            anime_id,
            change,
        )
    }

    /// Sets the `mask` bits of `season_flags` to those of `flag` in one UPDATE. Returns whether the row changed.
    pub fn set_season_flags_masked(&self, anime_id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        flags_write::masked(&self.db, flags_write::ANIME_SEASON_MASKED, anime_id, flag, mask)
    }

    /// Reads `season_flags`, applies `change` and stores the result in the writer transaction.
    pub fn update_season_flags(&self, anime_id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool> {
        flags_write::read_modify_write(
            &self.db,
            flags_write::ANIME_SEASON_READ,
            flags_write::ANIME_SEASON_WRITE,
            anime_id,
            change,
        )
    }

    pub fn set_categories(&self, anime_id: i64, category_ids: &[i64]) -> DataResult<()> {
        self.db.write(|tx| {
            tx.execute(CATEGORY_LINKS_DELETE, [anime_id])?;
            let mut seen = HashSet::new();
            for &category_id in category_ids.iter().filter(|id| seen.insert(**id)) {
                tx.execute(CATEGORY_LINK_INSERT, (anime_id, category_id))?;
            }
            Ok(())
        })
    }

    /// Inserts `a` and returns the new id.
    pub fn insert(&self, a: &Anime) -> DataResult<i64> {
        let row = to_row(a);
        self.db.write(|tx| Ok(insert_anime(tx, &row)?))
    }

    /// Coalesce update of one row. A missing id is not an error (EC-27).
    pub fn update(&self, u: &AnimeUpdate) -> DataResult<()> {
        let patch = to_update(u);
        self.db.write(|tx| Ok(update_anime(tx, &patch).map(drop)?))
    }

    /// Every update in one transaction: one failure rolls back all of them.
    pub fn update_all(&self, updates: &[AnimeUpdate]) -> DataResult<()> {
        self.db.write(|tx| {
            for u in updates {
                update_anime(tx, &to_update(u))?;
            }
            Ok(())
        })
    }

    /// Season rows (`animeseasonsView`) of one `Seasons` parent. Includes non-favourite children.
    pub fn seasons_by_parent(&self, parent_id: i64) -> DataResult<Vec<SeasonAnime>> {
        Ok(self.db.read(|c| Ok(query(c, SEASONS_BY_PARENT, &[(":parentId", &parent_id)], map_season_anime)?))?)
    }

    /// Every row whose `parent_id` is `parent_id`, favourite or not.
    pub fn children_by_parent(&self, parent_id: i64) -> DataResult<Vec<Anime>> {
        let rows = self.db.read(|c| Ok(query(c, CHILDREN_BY_PARENT, &[(":parentId", &parent_id)], AnimeRow::from_row)?))?;
        Ok(rows.iter().map(map_anime).collect())
    }

    /// Clears `parent_id` on the rows named by `ids` (their own `_id`s, as Kotlin's
    /// `removeParentIdByIds`). Used after a parent is deleted, because `parent_id` has no foreign key.
    pub fn remove_parent_ids(&self, ids: &[i64]) -> DataResult<()> {
        self.db.write(|tx| {
            for chunk in ids.chunks(PARENT_ID_CHUNK) {
                tx.execute(REMOVE_PARENT_IDS, [ids_json(chunk)])?;
            }
            Ok(())
        })
    }

    /// Non-favourite top-level anime, the candidates of the clear-database screen.
    pub fn deletable_parents(&self) -> DataResult<Vec<DeletableEntry>> {
        Ok(self.db.read(|c| Ok(query(c, DELETABLE_PARENTS, &[], map_deletable_anime)?))?)
    }

    /// `deletable_parents`, published again after a change to an anime, its episodes or its history.
    pub fn subscribe_deletable_parents(&self) -> Subscription<Vec<DeletableEntry>> {
        self.db.subscribe(ANIME_LIBRARY_TABLES, LIBRARY_DEBOUNCE, |c| {
            Ok(query(c, DELETABLE_PARENTS, &[], map_deletable_anime)?)
        })
    }

    /// `(source, anime id)` rows of the non-library anime and of the orphaned children (`non_library_tree`),
    /// published again after a change to an anime, its episodes or its history.
    pub fn subscribe_non_library_tree(&self) -> Subscription<(Vec<(i64, i64)>, Vec<(i64, i64)>)> {
        self.db.subscribe(ANIME_LIBRARY_TABLES, LIBRARY_DEBOUNCE, |c| Ok(non_library_tree(c)?))
    }

    /// `favorite_counts_by_source`, published again after a change to an anime.
    pub fn subscribe_favorite_counts(&self) -> Subscription<Vec<(i64, i64)>> {
        self.db.subscribe(ANIME_TABLE, ENTRY_DEBOUNCE, |c| Ok(source_favorite_counts(c)?))
    }

    /// Favourites due from local midnight of the day `now` falls in, soonest first, published again after a
    /// change and at each local midnight. The cut-off is recomputed on every run (EC-03).
    pub fn subscribe_upcoming(&self, statuses: &[i64]) -> Subscription<Vec<Anime>> {
        self.subscribe_upcoming_with_clock(statuses, || chrono::Utc::now().timestamp_millis())
    }

    /// `subscribe_upcoming` with the clock injected, as epoch milliseconds.
    pub fn subscribe_upcoming_with_clock(
        &self,
        statuses: &[i64],
        now: impl Fn() -> i64 + Send + Sync + 'static,
    ) -> Subscription<Vec<Anime>> {
        let statuses = statuses.to_vec();
        let now = Arc::new(now);
        let clock = Arc::clone(&now);
        self.db.subscribe_timed(
            ANIME_TABLE,
            ENTRY_DEBOUNCE,
            move || time::until_next_local_midnight((*clock)()),
            move |c| {
                let start_of_day = time::start_of_local_day_millis((*now)());
                Ok(upcoming_animes(c, &statuses, start_of_day)?.iter().map(map_anime).collect::<Vec<_>>())
            },
        )
    }

    /// Deletes the non-favourite anime named by `ids`. A favourite is never deleted, even when its id
    /// is passed. Episodes, category links and relations go with the row (foreign keys cascade).
    /// One `json_each` statement in one transaction. Consumed by the clear-database screen (area:47) and
    /// restore (area:22).
    pub fn delete_non_library_by_ids(&self, ids: &[i64]) -> DataResult<()> {
        let json = ids_json(ids);
        self.db.write(|tx| Ok(tx.execute(DELETE_NOT_IN_LIBRARY, [json]).map(drop)?))
    }

    /// `(source, url)` of every anime, library or not.
    pub fn all_source_and_url(&self) -> DataResult<Vec<(i64, String)>> {
        Ok(self.db.read(|c| Ok(query(c, SOURCE_AND_URL, &[], |r| Ok((r.get(0)?, r.get(1)?)))?))?)
    }

    /// `(source, favourite count)` for every source with at least one favourite, ordered by source.
    pub fn favorite_counts_by_source(&self) -> DataResult<Vec<(i64, i64)>> {
        Ok(self.db.read(|c| Ok(source_favorite_counts(c)?))?)
    }
}

impl AnimeRepo {
    /// Looks up `(url, source)` and inserts `a` when no row has it, in one writer transaction. Callers that
    /// race on the same key leave one row and get the same id (`NetworkToLocalAnime`).
    pub fn lookup_or_insert(&self, a: &Anime) -> DataResult<crate::LookupOrInsert<Anime>> {
        let row = to_row(a);
        self.db.write(|tx| match anime_by_url_and_source(tx, &a.url, a.source)? {
            Some(found) => Ok(crate::LookupOrInsert::Found(map_anime(&found))),
            None => Ok(crate::LookupOrInsert::Inserted(insert_anime(tx, &row)?)),
        })
    }
}
