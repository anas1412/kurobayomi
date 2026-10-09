# SQL catalog, chunk 3 of 4

Source of the item list: `python3 plans/_tools/inventory.py items sql 3 4` (38 items). First item: `getFavoriteBySourceId`. Last item: `insert_anime_category_update_version`. Every path below is repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`); `.sq` files live under `data/src/main/sqldelight` (manga database) and `data/src/main/sqldelightanime` (anime database). Cross-references use `area:<id>`: the authoritative schema and migration design is area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations, the repository layer is area:05-data-repos-entries-items and area:06-data-repos-history-track-source-misc; this catalog supplies the per-statement facts those areas need.

The 38 items are named queries (`name:` blocks inside `.sq` files), two views (`historyView`, and the query `history` that reads it), one table (`history`), one index (`history_history_chapter_id_index`), one trigger (`insert_anime_category_update_version`) and the generic name `insert` which is shared by 14 different statements in 14 `.sq` files. Where Aniyomi has a manga/anime twin of an item (for example `getFavorites` exists in `mangas.sq` and `animes.sq`) both twins are documented in the same section. Where a twin has a different name (for example the anime twin of `getSourceIdWithFavoriteCount` is `getAnimeSourceIdWithFavoriteCount`) the twin name is given as a plain word so it can be found.

## 1. Background facts needed to read every section (OBSERVED)

### 1.1 Two databases, not one

* Manga database: file `tachiyomi.db`, schema class `Database` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:78-101`). Anime database: file `tachiyomi.animedb`, schema class `AnimeDatabase` (`AppModule.kt:103-126`). They never share a connection, so `categories`, `history`-style tables and `extension_*` tables exist twice with independent `_id` spaces.
* Both are opened with `AndroidSqliteDriver` and in `onOpen` execute three pragmas: `PRAGMA foreign_keys = ON`, `PRAGMA journal_mode = WAL`, `PRAGMA synchronous = NORMAL` (`AppModule.kt:88-100` and `113-125`). Foreign-key enforcement is therefore on and every `ON DELETE CASCADE` in this chapter is live. `recursive_triggers` is never set, so it keeps SQLite's default OFF (this matters for the triggers in 1.5).
* Schema versions (SQLDelight rule: version = highest migration number + 1): manga database 33 (migrations `data/src/main/sqldelight/migrations/1.sqm` to `32.sqm`), anime database 139 (migrations `data/src/main/sqldelightanime/migrations/113.sqm` to `138.sqm`; the anime numbering starts at 113 because the anime database was forked from the manga schema at that point, so anime migration `1NN` mostly mirrors manga migration `NN`).
* Database access goes through `MangaDatabaseHandler` / `AnimeDatabaseHandler` (`data/src/main/java/tachiyomi/data/handlers/manga/MangaDatabaseHandler.kt`, `data/src/main/java/tachiyomi/data/handlers/anime/AnimeDatabaseHandler.kt`). Functions: `await` (run a block, optionally in a transaction), `awaitList`, `awaitOne` (throws if there is no row or more than one), `awaitOneOrNull` (null if no row, throws if more than one row), `subscribeToList` / `subscribeToOne` / `subscribeToOneOrNull` (a Flow that emits the current result and re-runs the query whenever SQLDelight reports a write to any table the query reads). `inTransaction = true` wraps the block in `db.transactionWithResult`; SQLDelight runs at most one transaction at a time and queues the others.

### 1.2 SQLDelight conventions that change the meaning of the SQL

* `:name` parameters are bound by position of first appearance; `?` is anonymous. `IN :list` expands at run time to one placeholder per list element, for example `IN (?, ?, ?)` for a three-element list (an empty list yields `IN ()`, which SQLite accepts and which matches nothing).
* "Partial update" statements (`update:` blocks) use `col = coalesce(:param, col)`. Consequence: a parameter that is NULL means "leave unchanged"; there is no way to set a nullable column back to NULL through these statements.
* Column adapters (`data/src/main/java/tachiyomi/data/DatabaseAdapter.kt`):

| Kotlin type | SQL storage | Adapter rule (file:line) |
|---|---|---|
| `Boolean` (`INTEGER AS Boolean`) | INTEGER 0/1 | built in; any non-zero reads as true |
| `java.util.Date` (`history.last_read`, `animehistory.last_seen`) | INTEGER epoch milliseconds | `DateColumnAdapter`, `DatabaseAdapter.kt:11-14` |
| `List<String>` (`mangas.genre`, `animes.genre`) | TEXT, elements joined by `", "` (comma + one space) | `StringListColumnAdapter`, `DatabaseAdapter.kt:16-27`: decode of the empty string is the empty list, otherwise `split(", ")`; a SQL NULL stays null at the Kotlin level (`List<String>?`) |
| `UpdateStrategy` (manga) | INTEGER ordinal: `ALWAYS_UPDATE` = 0, `ONLY_FETCH_ONCE` = 1 | `MangaUpdateStrategyColumnAdapter`, `DatabaseAdapter.kt:29-34`; an out-of-range value decodes to `ALWAYS_UPDATE` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt`) |
| `AnimeUpdateStrategy` | INTEGER ordinal, same two values | `AnimeUpdateStrategyColumnAdapter`, `DatabaseAdapter.kt:36-41` |
| `FetchType` (anime) | INTEGER ordinal: `Seasons` = 0, `Episodes` = 1 | `FetchTypeColumnAdapter`, `DatabaseAdapter.kt:43-48`; an out-of-range value decodes to `Episodes` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt`) |
| `JsonObject` (`animes.memo`, `episodes.memo`) | BLOB holding the UTF-8 text of a JSON object; default `x'7b7d'` which is the two bytes `{}` | `MemoColumnAdapter`, `DatabaseAdapter.kt:50-58`; decode uses `Json.decodeFromString`, so malformed content throws |

* Manga status integers (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:48-54`): `UNKNOWN = 0`, `ONGOING = 1`, `COMPLETED = 2`, `LICENSED = 3`, `PUBLISHING_FINISHED = 4`, `CANCELLED = 5`, `ON_HIATUS = 6`. Anime status integers (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-79`): the same seven plus `UPCOMING = 7`.

### 1.3 Units of time

All instants stored by the application are epoch milliseconds (UTC): `history.last_read`, `history.time_read` (a duration in milliseconds), `animehistory.last_seen`, `chapters.date_fetch`, `chapters.date_upload`, `episodes.date_fetch`, `episodes.date_upload`, `mangas.date_added`, `mangas.last_update`, `mangas.next_update`, `mangas.cover_last_modified`, `manga_sync.start_date`, `manga_sync.finish_date`, `anime_relations.last_fetched_at`. The exception is the sync bookkeeping written by triggers: `last_modified_at` and `favorite_modified_at` hold epoch SECONDS because the triggers use `strftime('%s', 'now')`. `date_added` is set to `Instant.now().toEpochMilli()` when an entry is added to the library and back to `0` when it is removed (`app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt:97-105`).

### 1.4 Foreign keys and ids

* `_id INTEGER NOT NULL PRIMARY KEY` is a rowid alias without `AUTOINCREMENT`; after the highest row is deleted its id can be handed out again.
* `chapters.manga_id` and `episodes.anime_id` cascade on delete, `history.chapter_id` and `animehistory.episode_id` cascade on delete, so removing a chapter removes its history row and removing an entry removes chapters, history, tracks, category links and relations.

### 1.5 The trigger families that the statements in this chapter fire

| Trigger | Table / event | Effect | Defined at |
|---|---|---|---|
| `update_last_modified_at_mangas` / `update_last_modified_at_animes` | `AFTER UPDATE ON mangas` / `animes`, per row | sets `last_modified_at = strftime('%s', 'now')` (seconds) on the updated row | `mangas.sq:44-51`, `animes.sq:56-63` |
| `update_last_favorite_at_mangas` / `update_last_favorite_at_animes` | `AFTER UPDATE OF favorite` | sets `favorite_modified_at = strftime('%s', 'now')`. The SQLite rule is "fires when `favorite` is named in the SET list", NOT "when its value changes". The shared `update:` statement names `favorite` in every call (`favorite = coalesce(:favorite, favorite)`), so `favorite_modified_at` is refreshed by every partial update of an entry, not only by favorite toggles | `mangas.sq:36-42`, `animes.sq:48-54` |
| `update_manga_version` / `update_anime_version` | `AFTER UPDATE ON mangas` / `animes` | `version = version + 1` when `new.is_syncing = 0` and `new.url != old.url OR new.description != old.description OR new.favorite != old.favorite`. NULL comparison rule: if either `description` is NULL the `!=` is NULL, so a description going from NULL to text (or text to NULL) does NOT bump the version | `mangas.sq:53-61`, `animes.sq:65-73` |
| `update_last_modified_at_chapters` / `update_last_modified_at_episodes` | `AFTER UPDATE`, per row | same seconds stamp on the chapter/episode | `chapters.sq:26-33`, `episodes.sq:32-39` |
| `update_chapter_and_manga_version` / `update_episode_and_anime_version` | `AFTER UPDATE ON chapters` / `episodes`, `WHEN new.is_syncing = 0 AND (read / bookmark / last_page_read changed)` (anime: seen / bookmark / last_second_seen) | bumps the chapter version and, when the parent's `is_syncing = 0`, the parent entry version | `chapters.sq:35-49`, `episodes.sq:41-55` |
| `insert_manga_category_update_version` / `insert_anime_category_update_version` | `AFTER INSERT ON mangas_categories` / `animes_categories` | bumps the parent entry `version` when `is_syncing = 0` | `mangas_categories.sq:11-16`, `animes_categories.sq:11-16` |
| `system_category_delete_trigger` | `BEFORE DELETE ON categories` | `RAISE(ABORT, "System category can't be deleted")` when `old._id <= 0` | `categories.sq:12-18` (both databases) |

Trigger chaining: the version/last-modified triggers issue their own `UPDATE` on the same table; because `recursive_triggers` is OFF, a trigger program is not re-entered while it is running, so the chain terminates. Rust must keep `recursive_triggers` OFF (the rusqlite default).

### 1.6 Drift between the `.sq` files and the migrated databases (OBSERVED)

A database created fresh from the `.sq` files and a database upgraded through the `.sqm` files are not identical. The differences that matter for this chapter:

* `history.last_read` is `INTEGER AS Date` (nullable) in `history.sq:3-9` but `NOT NULL` in migration `15.sqm` (manga) and `115.sqm` (`animehistory.last_seen`). The repository mappers accept a null `Date?` but `upsert` always supplies a value.
* `mangas.next_update` was added by `11.sqm` as `INTEGER DEFAULT 0` and then re-created without default by `14.sqm`; the mapper turns NULL into 0 (`MangaMapper.kt:38-39`).
* `animes.season_flags`, `season_number`, `season_source_order`, `fetch_type`, `background_last_modified` carry defaults (`0`, `-1.0`, `0`, `1`, `0`) in migrations `131.sqm` and `134.sqm` but are plain `NOT NULL` in `animes.sq:33-39`.
* `mangas_categories` and `animes_categories` gained a `last_modified_at INTEGER NOT NULL DEFAULT 0` column plus an `AFTER UPDATE` trigger in migrations `26.sqm` and `126.sqm`; the current `.sq` definitions (`mangas_categories.sq:1-9`, `animes_categories.sq:1-9`) no longer list that column or trigger.
* The two triggers `update_last_favorite_at_*` were created under the names `update_last_favorited_at_*` by migrations `26.sqm` / `126.sqm`.
* The anime system-category trigger was created as `system_anime_category_delete_trigger` by `120.sqm` while `animes`' `categories.sq:12` names it `system_category_delete_trigger`.

Proposal: Kurobayomi does not read Aniyomi `.db` files directly (Aniyomi data arrives through the backup file, area:22-backup-restore), so it creates databases only from one canonical `schema.sql` per database (the `.sq` form, with the migrated-form `NOT NULL` on the history timestamp column) and numbers its own migrations with `PRAGMA user_version`.

### 1.7 Proposal: Rust-side conventions used in every "Proposal" block below

* Crate features: `rusqlite = { version = "0.3x", features = ["bundled", "functions", "hooks", "serde_json"] }`. `bundled` gives a modern SQLite (UPSERT, `RETURNING`, partial indexes and `json_each` are all available); `functions` is needed for `unicode_lower`; `hooks` for the change feed.
* Layout: `src/db/mod.rs` (open, pragmas, helpers), `src/db/manga/{entries,history,updates,tracks,categories,sources}.rs` and a mirrored `src/db/anime/` directory with the same file names. Every function is a free function `fn name(conn: &rusqlite::Connection, <statement arguments>) -> rusqlite::Result<T>` so it works with any threading model; area:03-db-manga-schema-migrations owns whether the connection lives on one dedicated database thread (recommended: a single `std::thread` owning the connection and a `tokio::sync::oneshot` per call, which reproduces SQLDelight's one-at-a-time rule) or in a pool. Two files, `manga.db` and `anime.db`, in `$XDG_DATA_HOME/kurobayomi/`, so the table names in this catalog can be used unchanged.
* Opening a connection:

```rust
pub fn open(path: &std::path::Path) -> rusqlite::Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.create_scalar_function(
        "unicode_lower",
        1,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| Ok(ctx.get::<Option<String>>(0)?.map(|s| s.to_lowercase())),
    )?;
    Ok(conn)
}
```

* `IN` lists: `fn placeholders(n: usize) -> String { vec!["?"; n].join(",") }` then `rusqlite::params_from_iter`.
* "Start of the local day in milliseconds" (used by the upcoming queries; mirrors `LocalDate.now().atStartOfDay(ZoneId.systemDefault()).toEpochSecond() * 1000`):

```rust
pub fn start_of_local_day_ms() -> i64 {
    use chrono::{Duration, Local, LocalResult, TimeZone};
    let mut t = Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
    loop {
        match Local.from_local_datetime(&t) {
            LocalResult::Single(d) | LocalResult::Ambiguous(d, _) => return d.timestamp_millis(),
            LocalResult::None => t += Duration::minutes(1), // midnight skipped by a DST jump: first valid minute
        }
    }
}
```

* Case-insensitive search: SQLite `lower()` and `LIKE` only fold ASCII; Kotlin's `lowercase()` folds Unicode. Wherever the Kotlin passes a lowercased string into SQL, the Rust port registers `unicode_lower` (above) and compares `instr(unicode_lower(col), unicode_lower(?)) > 0` (substring) or `unicode_lower(col) = unicode_lower(?)` (equality). This also removes the `%` and `_` wildcard behaviour of `LIKE` that the Kotlin code never escapes.
* Row types (used by every signature): `Manga` has fields `id, source, url, artist, author, description, genre: Option<Vec<String>>, title, status, thumbnail_url, favorite: bool, last_update: i64 (NULL -> 0), next_update: i64 (NULL -> 0), initialized: bool, viewer_flags, chapter_flags, cover_last_modified, date_added, update_strategy: UpdateStrategy, fetch_interval: i32 (column calculate_interval), last_modified_at, favorite_modified_at: Option<i64>, version`. `Anime` adds `fetch_type: FetchType, parent_id: Option<i64>, season_flags, season_number: f64, season_source_order, background_url: Option<String>, background_last_modified, memo: serde_json::Map<String, serde_json::Value>` and calls `chapter_flags` `episode_flags`. The column lists, in the exact order the mappers read them, are:

```rust
pub const MANGA_COLS: &str = "_id, source, url, artist, author, description, genre, title, status, \
 thumbnail_url, favorite, last_update, next_update, initialized, viewer, chapter_flags, \
 cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, \
 favorite_modified_at, version";
pub const ANIME_COLS: &str = "_id, source, url, artist, author, description, genre, title, status, \
 thumbnail_url, favorite, last_update, next_update, initialized, viewer, episode_flags, \
 cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, \
 favorite_modified_at, version, fetch_type, parent_id, season_flags, season_number, \
 season_source_order, background_url, background_last_modified, memo";

pub fn manga_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Manga> {
    Ok(Manga {
        id: r.get(0)?, source: r.get(1)?, url: r.get(2)?, artist: r.get(3)?, author: r.get(4)?,
        description: r.get(5)?,
        genre: r.get::<_, Option<String>>(6)?
            .map(|s| if s.is_empty() { vec![] } else { s.split(", ").map(str::to_owned).collect() }),
        title: r.get(7)?, status: r.get(8)?, thumbnail_url: r.get(9)?, favorite: r.get(10)?,
        last_update: r.get::<_, Option<i64>>(11)?.unwrap_or(0),
        next_update: r.get::<_, Option<i64>>(12)?.unwrap_or(0),
        initialized: r.get(13)?, viewer_flags: r.get(14)?, chapter_flags: r.get(15)?,
        cover_last_modified: r.get(16)?, date_added: r.get(17)?,
        update_strategy: UpdateStrategy::from_ordinal(r.get(18)?),
        fetch_interval: r.get(19)?, last_modified_at: r.get(20)?,
        favorite_modified_at: r.get(21)?, version: r.get(22)?,
    })
}
```

`anime_from_row` is the same for indexes 0 to 22 (index 15 is `episode_flags`) and then reads `fetch_type` at 23 (`FetchType::from_ordinal`, unknown maps to `Episodes`), `parent_id` 24, `season_flags` 25, `season_number` 26, `season_source_order` 27, `background_url` 28, `background_last_modified` 29, `memo` 30 (`serde_json::from_slice::<Map<_, _>>(&blob).unwrap_or_default()`). The `is_syncing` column is deliberately omitted from both lists because the Kotlin mappers discard it (`MangaMapper.kt:32-33`).

* Change feed (replacement for SQLDelight's `asFlow`): the owning thread installs `conn.update_hook(Some(|_action, _db, table, _rowid| bus.send(table)))` once; `watch(tables, query)` runs `query` immediately, then again after each committed batch that touched one of `tables` (coalesce bursts for 50 ms). Each section below lists "Watch tables" for the queries Aniyomi subscribes to. SQLite's update hook also fires for rows changed by triggers, so the Rust feed notifies slightly more often than SQLDelight (harmless).

## 2. Index of the 38 items

| # | Item | Kind | Databases | Defined at |
|---|---|---|---|---|
| 1 | `getFavoriteBySourceId` | query | manga + anime | `mangas.sq:111`, `animes.sq:122` |
| 2 | `getFavorites` | query | manga + anime | `mangas.sq:76`, `animes.sq:87` |
| 3 | `getHistoryByAnimeId` | query | anime | `animehistory.sq:13` |
| 4 | `getHistoryByChapterUrl` | query | manga | `history.sq:25` |
| 5 | `getHistoryByEpisodeUrl` | query | anime | `animehistory.sq:23` |
| 6 | `getHistoryByMangaId` | query | manga | `history.sq:14` |
| 7 | `getLastFetchedAt` | query | anime | `anime_relations.sq:25` |
| 8 | `getLatestAnimeHistory` | query over view | anime | `animehistoryView.sq:46` |
| 9 | `getLatestHistory` | query over view | manga | `historyView.sq:48` |
| 10 | `getMangaById` | query | manga | `mangas.sq:63` |
| 11 | `getMangaByUrlAndSource` | query | manga | `mangas.sq:69` |
| 12 | `getMangasWithFavoriteTimestamp` | query (no caller) | manga | `mangas.sq:98` |
| 13 | `getReadDuration` | query | manga | `history.sq:71` |
| 14 | `getReadMangaNotInLibrary` | query | manga | `mangas.sq:81` |
| 15 | `getRecentAnimeUpdates` | query over view | anime | `animeupdatesView.sq:25` |
| 16 | `getRecentUpdates` | query over view | manga | `updatesView.sq:23` |
| 17 | `getRelatedAnimeByAnimeId` | query | anime | `anime_relations.sq:17` |
| 18 | `getScanlatorsByMangaId` | query | manga | `chapters.sq:68` |
| 19 | `getSourceIdWithFavoriteCount` | query | manga | `mangas.sq:103` |
| 20 | `getSourceIdsWithNonLibraryManga` | query | manga | `mangas.sq:141` |
| 21 | `getTrackByAnimeId` | query | anime | `anime_sync.sq:31` |
| 22 | `getTrackById` | query | manga | `manga_sync.sq:31` |
| 23 | `getTracks` | query | manga | `manga_sync.sq:27` |
| 24 | `getTracksByAnimeId` | query | anime | `anime_sync.sq:37` |
| 25 | `getTracksByMangaId` | query | manga | `manga_sync.sq:36` |
| 26 | `getUpcomingAnime` | query | anime | `animes.sq:135` |
| 27 | `getUpcomingManga` | query | manga | `mangas.sq:124` |
| 28 | `getUpdatesByReadStatus` | query over view | manga | `updatesView.sq:29` |
| 29 | `getUpdatesBySeenStatus` | query over view | anime | `animeupdatesView.sq:31` |
| 30 | `getVisibleCategories` | query | manga + anime | `categories.sq:36` (both databases) |
| 31 | `getVisibleCategoriesByAnimeId` | query | anime | `categories.sq:59` (anime) |
| 32 | `getVisibleCategoriesByMangaId` | query | manga | `categories.sq:59` (manga) |
| 33 | `getWatchedAnimeNotInLibrary` | query | anime | `animes.sq:92` |
| 34 | `history` | table (`history.sq:3`) and query over view (`historyView.sq:29`) | manga | see section 5.34 |
| 35 | `historyView` | view | manga | `historyView.sq:1` |
| 36 | `history_history_chapter_id_index` | index | manga | `history.sq:12` |
| 37 | `insert` | 14 INSERT statements | both | section 5.37 |
| 38 | `insert_anime_category_update_version` | trigger | anime | `animes_categories.sq:11` |

File names in column "Defined at" are shortened: `mangas.sq` is `data/src/main/sqldelight/data/mangas.sq`, `history.sq` is `data/src/main/sqldelight/data/history.sq`, `historyView.sq` is `data/src/main/sqldelight/view/historyView.sq`, `updatesView.sq` is `data/src/main/sqldelight/view/updatesView.sq`, `chapters.sq`, `manga_sync.sq` and `categories.sq` (manga) are under `data/src/main/sqldelight/data/`, and the anime files `animes.sq`, `animehistory.sq`, `anime_relations.sq`, `anime_sync.sq`, `animes_categories.sq` and `categories.sq` (anime) are under `data/src/main/sqldelightanime/dataanime/` while `animehistoryView.sq` and `animeupdatesView.sq` are under `data/src/main/sqldelightanime/view/`.

## 3. Tables, indexes and triggers touched by this chunk

Only tables that the 38 items read or write are listed. "Baseline" means the column already existed in the first schema snapshot that the migrations start from (manga: before `1.sqm`; anime: version 113, before `113.sqm`). Migration numbers refer to `data/src/main/sqldelight/migrations/N.sqm` (manga) and `data/src/main/sqldelightanime/migrations/N.sqm` (anime). The Proposal column of every table is the same as the `.sq` definition unless a line starting with "Proposal:" says otherwise.

### 3.1 `history` (manga database, item `history`)

Defined at `data/src/main/sqldelight/data/history.sq:3-10`:

```sql
CREATE TABLE history(
    _id INTEGER NOT NULL PRIMARY KEY,
    chapter_id INTEGER NOT NULL UNIQUE,
    last_read INTEGER AS Date,
    time_read INTEGER NOT NULL,
    FOREIGN KEY(chapter_id) REFERENCES chapters (_id)
    ON DELETE CASCADE
);
```

| Column | Type | Null / default / constraint | Introduced by | Meaning |
|---|---|---|---|---|
| `_id` | INTEGER | NOT NULL, PRIMARY KEY (rowid alias) | `2.sqm` as `history_id`; renamed by `15.sqm` | history row id |
| `chapter_id` | INTEGER | NOT NULL, UNIQUE, FOREIGN KEY to `chapters(_id)` ON DELETE CASCADE | `2.sqm` as `history_chapter_id`; renamed by `15.sqm` | at most one history row per chapter |
| `last_read` | INTEGER (Date) | `.sq`: nullable; migrated databases: NOT NULL (`15.sqm`, null back-filled to 0) | `2.sqm` as `history_last_read`; renamed by `15.sqm` | epoch milliseconds of the last reading session; `0` means "reset" (hidden from the History screen but row and `time_read` kept) |
| `time_read` | INTEGER | NOT NULL (`15.sqm` back-filled null to 0) | `2.sqm` as `history_time_read`; renamed by `15.sqm` | accumulated reading time in milliseconds over all sessions |

Migration history: `2.sqm` created the table with prefixed column names, `14.sqm` rebuilt it with `Long` adapters, `15.sqm` rebuilt it again (`last_read` made not-null, `time_read` kept and made not-null, prefix removed). Indexes: `history_history_chapter_id_index` (item 36). Triggers: none. Rust (Proposal): create with `last_read INTEGER NOT NULL DEFAULT 0` and `time_read INTEGER NOT NULL DEFAULT 0`.

### 3.2 `animehistory` (anime database)

Defined at `data/src/main/sqldelightanime/dataanime/animehistory.sq:3-9`:

```sql
CREATE TABLE animehistory(
    _id INTEGER NOT NULL PRIMARY KEY,
    episode_id INTEGER NOT NULL UNIQUE,
    last_seen INTEGER AS Date,
    FOREIGN KEY(episode_id) REFERENCES episodes (_id)
    ON DELETE CASCADE
);
```

Columns `_id`, `episode_id`, `last_seen` mirror `history` (`last_seen` is epoch milliseconds, `0` = reset). There is no duration column: `115.sqm` removed `time_seen` ("`[time_seen]` was removed"), so the anime side never records watch time in history (anime watch time in the Statistics screen is computed from `episodes.last_second_seen` and `episodes.total_seconds`, `app/src/main/java/eu/kanade/tachiyomi/ui/stats/anime/AnimeStatsScreenModel.kt:133-135`). Index: `animehistory_history_episode_id_index ON animehistory(episode_id)` (`animehistory.sq:11`, created by `114.sqm`/`115.sqm`).

### 3.3 `mangas` (manga database)

Defined at `data/src/main/sqldelight/data/mangas.sq:6-31`; indexes `library_favorite_index ON mangas(favorite) WHERE favorite = 1` (`:33`, partial; created by `7.sqm`, re-created by `14.sqm`) and `mangas_url_index ON mangas(url)` (`:34`); triggers in section 1.5.

| Column | Type | Null / default | Introduced by | Meaning |
|---|---|---|---|---|
| `_id` | INTEGER | PK | baseline | local id |
| `source` | INTEGER | NOT NULL | baseline | source id (Long, the local source is `0`: `source-local/src/commonMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt`, `ID = 0L`) |
| `url` | TEXT | NOT NULL | baseline | source-relative URL; not unique (see `getMangaByUrlAndSource`) |
| `artist`, `author`, `description` | TEXT | nullable | baseline | metadata |
| `genre` | TEXT (List<String>) | nullable | baseline | `", "`-joined |
| `title` | TEXT | NOT NULL | baseline | display title |
| `status` | INTEGER | NOT NULL | baseline | `SManga` status (1.2) |
| `thumbnail_url` | TEXT | nullable | baseline | cover URL |
| `favorite` | INTEGER (Boolean) | NOT NULL | baseline | in library |
| `last_update` | INTEGER | nullable | baseline | last library update, epoch ms |
| `next_update` | INTEGER | nullable (migrated DBs may carry default 0) | `11.sqm` | predicted next update, epoch ms |
| `initialized` | INTEGER (Boolean) | NOT NULL | baseline | details fetched at least once |
| `viewer` | INTEGER | NOT NULL | baseline | reader flags (`viewer_flags` in Kotlin) |
| `chapter_flags` | INTEGER | NOT NULL | baseline | sort/filter/display bit flags (`Manga.kt` constants `CHAPTER_SORT_DIR_MASK = 0x1`, `CHAPTER_UNREAD_MASK = 0x6`, `CHAPTER_DOWNLOADED_MASK = 0x18`, `CHAPTER_BOOKMARKED_MASK = 0x60`, `CHAPTER_SORTING_MASK = 0x300`, `CHAPTER_DISPLAY_MASK = 0x100000`) |
| `cover_last_modified` | INTEGER | NOT NULL | `9.sqm` | cover cache-buster, epoch ms |
| `date_added` | INTEGER | NOT NULL | `10.sqm` | epoch ms when favorited; 0 when not |
| `update_strategy` | INTEGER (UpdateStrategy) | NOT NULL DEFAULT 0 | `20.sqm` | 0 always, 1 only-fetch-once |
| `calculate_interval` | INTEGER | NOT NULL DEFAULT 0 | `24.sqm` | fetch interval in days (`fetchInterval` in Kotlin) |
| `last_modified_at` | INTEGER | NOT NULL DEFAULT 0 | `26.sqm` | seconds, written by trigger |
| `favorite_modified_at` | INTEGER | nullable | `26.sqm` | seconds, written by trigger |
| `version` | INTEGER | NOT NULL DEFAULT 0 | `30.sqm` | sync version counter |
| `is_syncing` | INTEGER | NOT NULL DEFAULT 0 | `30.sqm` | sync-in-progress flag (reset by `resetIsSyncing`, `mangas.sq:136`) |

### 3.4 `animes` (anime database)

Defined at `data/src/main/sqldelightanime/dataanime/animes.sq:8-41`. Columns `_id` to `is_syncing` mirror `mangas` with `chapter_flags` renamed `episode_flags` and the migrations `121.sqm` (`update_strategy`), `124.sqm` (`calculate_interval`), `126.sqm` (`last_modified_at`, `favorite_modified_at`), `127.sqm` (`version`, `is_syncing`); the `update_strategy` type is `AnimeUpdateStrategy`. Anime-only columns:

| Column | Type | Null / default | Introduced by | Meaning |
|---|---|---|---|---|
| `fetch_type` | INTEGER (FetchType) | NOT NULL DEFAULT 1 | `131.sqm`; remapped by `132.sqm` (`2 -> 1`, `1 -> 0`, `0 -> 1`) and reset to 1 for all rows by `135.sqm` | 0 = Seasons (children are other `animes` rows), 1 = Episodes |
| `parent_id` | INTEGER | nullable | `131.sqm` | `_id` of the season-group parent |
| `season_flags` | INTEGER | NOT NULL (migrated default 0) | `131.sqm` | season list sort/filter flags |
| `season_number` | REAL | NOT NULL (migrated default -1.0) | `131.sqm` | season number |
| `season_source_order` | INTEGER | NOT NULL (migrated default 0) | `131.sqm` | order in the source's season list |
| `background_url` | TEXT | nullable | `134.sqm` | backdrop image URL |
| `background_last_modified` | INTEGER | NOT NULL (migrated default 0) | `134.sqm` | backdrop cache-buster |
| `memo` | BLOB (JsonObject) | NOT NULL DEFAULT `'{}'` (migrated: `x'7b7d'`) | `136.sqm` | free-form JSON written by extensions |

Indexes: `animelib_favorite_index ON animes(favorite) WHERE favorite = 1` (`animes.sq:43`; migration `114.sqm` created it under the name `library_favorite_index`), `animes_url_index ON animes(url)` (`:44`), `animes_parent_id ON animes(parent_id)` (`:45`, `133.sqm`), `animes_fetch_type ON animes(fetch_type)` (`:46`, `133.sqm`).

### 3.5 `chapters` (manga database) and `episodes` (anime database)

`chapters.sq:3-21`: `_id` PK; `manga_id` NOT NULL FK `mangas(_id)` ON DELETE CASCADE; `url` TEXT NOT NULL; `name` TEXT NOT NULL; `scanlator` TEXT nullable (`4.sqm`; `28.sqm` trims whitespace of existing values); `read` INTEGER (Boolean) NOT NULL; `bookmark` INTEGER (Boolean) NOT NULL (`3.sqm`); `last_page_read` INTEGER NOT NULL; `chapter_number` REAL NOT NULL; `source_order` INTEGER NOT NULL (`1.sqm`); `date_fetch` INTEGER NOT NULL (epoch ms); `date_upload` INTEGER NOT NULL (epoch ms; `13.sqm` back-filled zero with `date_fetch`); `last_modified_at` INTEGER NOT NULL DEFAULT 0 (`26.sqm`); `version` INTEGER NOT NULL DEFAULT 0 (`30.sqm`); `is_syncing` INTEGER NOT NULL DEFAULT 0 (`30.sqm`). Indexes `chapters_manga_id_index ON chapters(manga_id)` and `chapters_unread_by_manga_index ON chapters(manga_id, read) WHERE read = 0`.

`episodes.sq:4-27`: `_id`; `anime_id` FK `animes(_id)` cascade; `url`; `name`; `scanlator`; `seen` (Boolean); `bookmark` (Boolean); `last_second_seen` INTEGER NOT NULL; `total_seconds` INTEGER NOT NULL; `episode_number` REAL NOT NULL; `source_order`; `date_fetch`; `date_upload`; `last_modified_at` (`126.sqm`); `version`, `is_syncing` (`127.sqm`); `summary` TEXT nullable, `preview_url` TEXT nullable, `fillermark` INTEGER (Boolean) NOT NULL (default 0 in the migration) (all `134.sqm`); `memo` BLOB NOT NULL DEFAULT `'{}'` (`136.sqm`). Indexes `episodes_anime_id_index ON episodes(anime_id)`, `episodes_unseen_by_anime_index ON episodes(anime_id, seen) WHERE seen = 0`.

### 3.6 `categories`, `mangas_categories`, `animes_categories`

`categories` is defined identically in both databases (`categories.sq:1-7` of each): `_id` INTEGER PK, `name` TEXT NOT NULL, `sort` INTEGER NOT NULL, `flags` INTEGER NOT NULL, `hidden` INTEGER NOT NULL DEFAULT 0 (`25.sqm` manga, `125.sqm` anime). `categories.sq:10` inserts the system row `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0);` (migrations `19.sqm` and `120.sqm`) which is the "Default" / uncategorised category (`Category.UNCATEGORIZED_ID = 0L`, `domain/src/main/java/tachiyomi/domain/category/model/Category.kt`). `flags` stores the library sort of the category: `MangaLibrarySort` bits (`domain/src/main/java/tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt`): type mask `0b00111100` with values `Alphabetical 0b00000000`, `LastRead 0b00000100`, `LastUpdate 0b00001000`, `UnreadCount 0b00001100`, `TotalChapters 0b00010000`, `LatestChapter 0b00010100`, `ChapterFetchDate 0b00011000`, `DateAdded 0b00011100`, `TrackerMean 0b00100000`, `Random 0b00111100`; direction mask `0b01000000` with `Ascending = 0b01000000`, `Descending = 0b00000000`.

`mangas_categories` (`mangas_categories.sq:1-9`): `_id` PK, `manga_id` NOT NULL FK `mangas` cascade, `category_id` NOT NULL FK `categories` cascade. `animes_categories`: `_id`, `anime_id`, `category_id` likewise. Neither has a uniqueness constraint on `(manga_id, category_id)`; the application always deletes all links of an entry and re-inserts the new set inside one transaction (`MangaRepositoryImpl.kt:91-98`, `AnimeRepositoryImpl.kt:95-102`).

### 3.7 `manga_sync` and `anime_sync` (tracker bindings)

`manga_sync.sq:3-21`: `_id` PK; `manga_id` NOT NULL FK `mangas` cascade; `sync_id` INTEGER NOT NULL (tracker id: MyAnimeList 1, AniList 2, Kitsu 3, Shikimori 4, Bangumi 5, Komga 6, MangaUpdates 7, Kavita 8, Suwayomi 9, Simkl 101, Jellyfin 102 per `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:19-37`); `remote_id` INTEGER NOT NULL; `library_id` INTEGER nullable (`6.sqm`); `title` TEXT NOT NULL; `last_chapter_read` REAL NOT NULL; `total_chapters` INTEGER NOT NULL; `status` INTEGER NOT NULL; `score` REAL NOT NULL (`29.sqm` clamps MangaUpdates scores `max(score, 0)` where `sync_id = 7`); `remote_url` TEXT NOT NULL (`5.sqm`); `start_date` INTEGER NOT NULL (`8.sqm`); `finish_date` INTEGER NOT NULL (`8.sqm`); `private` INTEGER (Boolean) NOT NULL DEFAULT 0 (`32.sqm`); table constraint `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE` (rebuilt by `12.sqm`). `anime_sync.sq:3-21`: same shape with `anime_id`, `last_episode_seen` REAL NOT NULL, `total_episodes` INTEGER NOT NULL and `private` added by `130.sqm`.

### 3.8 `anime_relations` (anime database, `138.sqm`)

`anime_relations.sq:1-12`: `anime_id` INTEGER NOT NULL FK `animes(_id)` cascade; `related_anime_id` INTEGER NOT NULL FK `animes(_id)` cascade; `name` TEXT NOT NULL (group title such as a franchise heading); `sort_order` INTEGER NOT NULL; `last_fetched_at` INTEGER NOT NULL (epoch ms); `PRIMARY KEY(anime_id, related_anime_id, name)`. Indexes `anime_relations_anime_id_index ON anime_relations(anime_id)`, `anime_relations_related_anime_id_index ON anime_relations(related_anime_id)`.

### 3.9 `excluded_scanlators`, `extension_repos`, `custom_buttons`

* `excluded_scanlators` (manga, `27.sqm`): `manga_id` INTEGER NOT NULL FK `mangas` cascade, `scanlator` TEXT NOT NULL; no primary key; index `excluded_scanlators_manga_id_index`.
* `extension_repos` (manga only, `31.sqm`; the anime database created the same table in `128.sqm` and dropped it in `137.sqm` in favour of `extension_store`): `base_url` TEXT NOT NULL PRIMARY KEY, `name` TEXT NOT NULL, `short_name` TEXT nullable, `website` TEXT NOT NULL, `signing_key_fingerprint` TEXT NOT NULL UNIQUE.
* `custom_buttons` (anime, `129.sqm`): `_id` PK, `name` TEXT NOT NULL, `isFavorite` INTEGER (Boolean) NOT NULL, `sortIndex` INTEGER NOT NULL, `content` TEXT NOT NULL, `longPressContent` TEXT NOT NULL, `onStartup` TEXT NOT NULL; the schema also inserts row 1 (`'+85 s'`, favourite, sort 0, a seek-forward-by-intro-length Lua snippet).

## 4. Reading guide for the per-item sections

Each section has the same fields: **SQL** (exact text, trimmed), **Parameters**, **Returns**, **Semantics** (joins, filters, ordering, NULL handling, trigger effects), **Called by** (`path:line`), **Performance**, **Proposal** (rusqlite statement and signature). Section numbers 5.N follow the index in section 2. The anime twin of a manga query uses the table and column names listed in section 3 and the `anime_from_row` mapper of section 1.7.

## 5. Per-item catalog

### 5.1 `getFavoriteBySourceId`

SQL, manga (`data/src/main/sqldelight/data/mangas.sq:111-115`):

```sql
getFavoriteBySourceId:
SELECT *
FROM mangas
WHERE favorite = 1
AND source = :sourceId;
```

SQL, anime (`data/src/main/sqldelightanime/dataanime/animes.sq:122-126`): identical text with `FROM animes`.

* Parameters: `sourceId` Long (the source id; the local source is 0).
* Returns: every column of `mangas` (23 columns plus `is_syncing`; 31 plus `is_syncing` for `animes`), mapped by `MangaMapper.mapManga` (`data/src/main/java/tachiyomi/data/entries/manga/MangaMapper.kt:8-58`) / `AnimeMapper.mapAnime`.
* Semantics: only library entries of one source. No ORDER BY, so the row order is whatever the partial index `library_favorite_index` yields (rowid order in practice); the only consumer sorts in memory. NULL handling: nullable text columns arrive as null; `last_update` and `next_update` NULL become 0.
* Triggers: none (read).
* Called by: `data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt:65` (`getMangaFavoritesBySourceId`, `subscribeToList`) via `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetMangaFavorites.kt:16` (`subscribe(sourceId)`) from `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/manga/MigrateMangaScreenModel.kt:41`, which then sorts with `compareBy(String.CASE_INSENSITIVE_ORDER) { it.title }` (`:41-52`) for the Migrate screen's per-source list. Anime: `AnimeRepositoryImpl.kt:69`, `GetAnimeFavorites.kt:16`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/anime/MigrateAnimeScreenModel.kt:41`.
* Performance: the planner uses `library_favorite_index` (its partial predicate `favorite = 1` matches the query) and filters `source` row by row; linear in the number of favourites, no extra index needed.
* Watch tables: `mangas` (`animes`).
* Proposal:

```rust
pub fn favorites_by_source(conn: &Connection, source_id: i64) -> rusqlite::Result<Vec<Manga>> {
    let sql = format!("SELECT {MANGA_COLS} FROM mangas WHERE favorite = 1 AND source = ?1 ORDER BY _id");
    conn.prepare_cached(&sql)?.query_map(params![source_id], manga_from_row)?.collect()
}
// anime twin: db::anime::favorites_by_source(conn, source_id) over animes with ANIME_COLS / anime_from_row
```

The caller sorts the result by `title` case-insensitively using Unicode lowercase (`to_lowercase`) and a locale collator is not required (Kotlin uses `String.CASE_INSENSITIVE_ORDER`, a per-character comparison).

### 5.2 `getFavorites`

SQL, manga (`mangas.sq:76-79`):

```sql
getFavorites:
SELECT *
FROM mangas
WHERE favorite = 1;
```

SQL, anime (`animes.sq:87-90`): identical with `FROM animes`.

* Parameters: none. Returns: all columns of every library entry (list).
* Semantics: no ORDER BY; consumers do not depend on order (they iterate). NULL handling as in 5.1.
* Called by: `MangaRepositoryImpl.kt:49` (`getMangaFavorites`, one-shot `awaitList`) via `GetMangaFavorites.kt:12` (`await()`) from `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt:112` (backup of library entries, followed by non-favourite read entries when `BackupOptions.readEntries`, default true), `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt:364-369` (library CSV export, `toExportEntry()`), `domain/src/main/java/tachiyomi/domain/items/chapter/interactor/SetMangaDefaultChapterFlags.kt:33` (apply the new global chapter-flag defaults to each favourite). Anime: `AnimeRepositoryImpl.kt:53`, `GetAnimeFavorites.kt:12`, `BackupCreator.kt:106`, `SettingsDataScreen.kt:368`, `domain/src/main/java/tachiyomi/domain/items/episode/interactor/SetAnimeDefaultEpisodeFlags.kt:36`, `domain/src/main/java/tachiyomi/domain/items/season/interactor/SetAnimeDefaultSeasonFlags.kt:43`.
* Performance: partial-index scan; the result is materialised fully (library sizes of tens of thousands of rows are a few megabytes).
* Proposal:

```rust
pub fn favorites(conn: &Connection) -> rusqlite::Result<Vec<Manga>> {
    let sql = format!("SELECT {MANGA_COLS} FROM mangas WHERE favorite = 1 ORDER BY _id");
    conn.prepare_cached(&sql)?.query_map([], manga_from_row)?.collect()
}
```

### 5.3 `getHistoryByAnimeId`

SQL (`animehistory.sq:13-21`):

```sql
getHistoryByAnimeId:
SELECT
H._id,
H.episode_id,
H.last_seen
FROM animehistory H
JOIN episodes EP
ON H.episode_id = EP._id
WHERE EP.anime_id = :animeId AND EP._id = H.episode_id;
```

* Parameters: `animeId` Long. Returns: rows `(_id Long, episode_id Long, last_seen Date?)` mapped by `AnimeHistoryMapper.mapAnimeHistory(id, episodeId, seenAt: Date?)` (`data/src/main/java/tachiyomi/data/history/anime/AnimeHistoryMapper.kt:9-17`). The manga twin is `getHistoryByMangaId` (5.6).
* Semantics: every history row of every episode of one anime, including rows whose `last_seen` is 0 (reset rows). The second predicate `EP._id = H.episode_id` repeats the join condition and has no effect. No ORDER BY.
* Called by: `data/src/main/java/tachiyomi/data/history/anime/AnimeHistoryRepositoryImpl.kt:30` (`getHistoryByAnimeId`) via `domain/src/main/java/tachiyomi/domain/history/anime/interactor/GetAnimeHistory.kt:13` (`await(animeId)`) from `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeBackupCreator.kt:61` (each row becomes `BackupAnimeHistory(episode.url, seenAt?.time ?: 0)`) and `app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt:59` (earliest `seenAt` becomes the tracker start date; because reset rows carry `Date(0)` and the list is `sortedBy { it.seenAt }`, a single reset row makes the "first seen" date the Unix epoch. The Rust caller must drop rows with `last_seen = 0` before taking the minimum).
* Performance: `animehistory_history_episode_id_index` (or the UNIQUE constraint index) for the join; `episodes_anime_id_index` for the filter. Fine.
* Proposal:

```rust
pub struct AnimeHistoryRow { pub id: i64, pub episode_id: i64, pub seen_at: Option<i64> /* epoch ms, Some(0) = reset */ }
pub fn history_by_anime_id(conn: &Connection, anime_id: i64) -> rusqlite::Result<Vec<AnimeHistoryRow>> {
    conn.prepare_cached(
        "SELECT H._id, H.episode_id, H.last_seen FROM animehistory H \
         JOIN episodes EP ON H.episode_id = EP._id WHERE EP.anime_id = ?1 ORDER BY H._id")?
        .query_map(params![anime_id], |r| Ok(AnimeHistoryRow { id: r.get(0)?, episode_id: r.get(1)?, seen_at: r.get(2)? }))?
        .collect()
}
```

### 5.4 `getHistoryByChapterUrl`

SQL (`history.sq:25-34`):

```sql
getHistoryByChapterUrl:
SELECT
H._id,
H.chapter_id,
H.last_read,
H.time_read
FROM history H
JOIN chapters C
ON H.chapter_id = C._id
WHERE C.url = :chapterUrl AND C._id = H.chapter_id;
```

* Parameters: `chapterUrl` String (exact, case-sensitive match on `chapters.url`). Returns: `(_id, chapter_id, last_read Date?, time_read Long)`.
* Semantics: finds the history row of the chapter with that URL in ANY manga: `chapters.url` is not unique across manga (two sources can both have `/chapter/1`), and the caller uses `awaitOneOrNull`, which throws `IllegalStateException` when more than one row comes back. So a backup that contains a history entry whose URL exists in two manga aborts the restore of that manga. The `C._id = H.chapter_id` predicate is redundant.
* Called by: `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt:323` inside `restoreHistory` (`:321-358`). Merge rule applied right after (`:337-346`): for an existing row the new `last_read` is `max(backup.readAt, db.last_read)` (0 becomes null), and the duration passed to `upsert` is `max(backup.readDuration, db.time_read) - db.time_read`, i.e. only the positive difference is added (see 5.34 `upsert`). For a missing row the chapter is looked up with `chaptersQueries.getChapterByUrl(history.url)` (also URL-only) and skipped when absent. The anime twin is `getHistoryByEpisodeUrl` (5.5).
* Performance: `chapters.url` has no index: a full scan of `chapters` per history entry; restoring N history rows costs N scans. Proposal fixes this.
* Proposal (changes the signature to scope by manga, which is what a restore always knows; the SQL stays a join):

```rust
pub struct HistoryRow { pub id: i64, pub chapter_id: i64, pub last_read: Option<i64>, pub time_read: i64 }
pub fn history_by_chapter_url(conn: &Connection, manga_id: i64, chapter_url: &str) -> rusqlite::Result<Option<HistoryRow>> {
    conn.prepare_cached(
        "SELECT H._id, H.chapter_id, H.last_read, H.time_read FROM history H \
         JOIN chapters C ON H.chapter_id = C._id WHERE C.manga_id = ?1 AND C.url = ?2")?
        .query_row(params![manga_id, chapter_url], |r| Ok(HistoryRow {
            id: r.get(0)?, chapter_id: r.get(1)?, last_read: r.get(2)?, time_read: r.get(3)? }))
        .optional()
}
```

With the `manga_id` filter the `chapters_manga_id_index` makes the lookup O(chapters of one manga).

### 5.5 `getHistoryByEpisodeUrl`

SQL (`animehistory.sq:23-31`):

```sql
getHistoryByEpisodeUrl:
SELECT
H._id,
H.episode_id,
H.last_seen
FROM animehistory H
JOIN episodes EP
ON H.episode_id = EP._id
WHERE EP.url = :episodeUrl AND EP._id = H.episode_id;
```

* Same semantics, hazard and performance note as 5.4 (URL-only lookup across all anime, `awaitOneOrNull` throws on a duplicate URL, `episodes.url` unindexed). Returns `(_id, episode_id, last_seen Date?)`.
* Called by: `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt:364` inside `restoreHistory` (`:362-398`); merge rule `seenAt = max(backup.seenAt, db.last_seen)` with 0 mapped to null; the episode is looked up with `episodesQueries.getEpisodeByUrl(history.url)` when no history row exists. Then `animehistoryQueries.upsert(episodeId, seenAt)` (`AnimeRestorer.kt:391-394`).
* Proposal: same shape as 5.4 with `anime_id` scoping:

```rust
pub fn history_by_episode_url(conn: &Connection, anime_id: i64, episode_url: &str) -> rusqlite::Result<Option<AnimeHistoryRow>> {
    conn.prepare_cached(
        "SELECT H._id, H.episode_id, H.last_seen FROM animehistory H \
         JOIN episodes EP ON H.episode_id = EP._id WHERE EP.anime_id = ?1 AND EP.url = ?2")?
        .query_row(params![anime_id, episode_url], |r| Ok(AnimeHistoryRow {
            id: r.get(0)?, episode_id: r.get(1)?, seen_at: r.get(2)? }))
        .optional()
}
```

### 5.6 `getHistoryByMangaId`

SQL (`history.sq:14-23`):

```sql
getHistoryByMangaId:
SELECT
H._id,
H.chapter_id,
H.last_read,
H.time_read
FROM history H
JOIN chapters C
ON H.chapter_id = C._id
WHERE C.manga_id = :mangaId AND C._id = H.chapter_id;
```

* Parameters: `mangaId` Long. Returns: `(_id, chapter_id, last_read Date?, time_read Long)` rows, mapped by `MangaHistoryMapper.mapMangaHistory(id, chapterId, readAt: Date?, readDuration: Long)` (`data/src/main/java/tachiyomi/data/history/manga/MangaHistoryMapper.kt:9-19`).
* Semantics: all history rows of one manga, reset rows (`last_read = 0`) included, unordered, redundant second predicate.
* Called by: `MangaHistoryRepositoryImpl.kt:33` via `domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetMangaHistory.kt:13` (`await(mangaId)`) from `MangaBackupCreator.kt:66` (backup row `BackupHistory(chapter.url, readAt?.time ?: 0, readDuration)` where `chapter` comes from `chaptersQueries.getChapterById(history.chapterId)`, `:68-70`) and `app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt:59` (earliest `readAt` becomes the tracker `startDate` after `convertEpochMillisZone(systemDefault, UTC)`; same reset-row epoch hazard as 5.3).
* Performance: PK/UNIQUE index on `history.chapter_id`, `chapters_manga_id_index`; fine.
* Proposal:

```rust
pub fn history_by_manga_id(conn: &Connection, manga_id: i64) -> rusqlite::Result<Vec<HistoryRow>> {
    conn.prepare_cached(
        "SELECT H._id, H.chapter_id, H.last_read, H.time_read FROM history H \
         JOIN chapters C ON H.chapter_id = C._id WHERE C.manga_id = ?1 ORDER BY H._id")?
        .query_map(params![manga_id], |r| Ok(HistoryRow {
            id: r.get(0)?, chapter_id: r.get(1)?, last_read: r.get(2)?, time_read: r.get(3)? }))?
        .collect()
}
// tracker start date helper: history_by_manga_id(..)?.into_iter().filter_map(|h| h.last_read).filter(|t| *t > 0).min()
```

### 5.7 `getLastFetchedAt`

SQL (`anime_relations.sq:25-29`):

```sql
getLastFetchedAt:
SELECT last_fetched_at
FROM anime_relations
WHERE anime_id = :animeId
LIMIT 1;
```

* Parameters: `animeId`. Returns: one Long (epoch ms) or no row.
* Semantics: every relation row of one anime carries the same `last_fetched_at` because `replaceRelations` stamps all rows of one refresh with the same `fetchedAt` (`AnimeRelationRepositoryImpl.kt:17-33`), so `LIMIT 1` without ORDER BY is safe. No row means either "never fetched" or "fetched but the source returned no related titles" (all groups empty are filtered out, `SyncRelatedAnimeWithSource.kt:35`), so an anime with no relations is re-fetched on every screen open (no negative caching).
* Called by: `data/src/main/java/tachiyomi/data/entries/anime/AnimeRelationRepositoryImpl.kt:14` (`getLastFetchedAt`) from `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncRelatedAnimeWithSource.kt:23`: unless `forceRefresh`, a fetch is skipped when `now - lastFetchedAt < TTL` with `TTL = 7.days.inWholeMilliseconds` = 604800000 ms (`:41-42`); a refreshed list keeps at most `MAX_PER_GROUP = 10` entries per group, de-duplicated by `url`, excluding the anime's own `url`.
* Performance: `anime_relations_anime_id_index`.
* Proposal:

```rust
pub fn relations_last_fetched_at(conn: &Connection, anime_id: i64) -> rusqlite::Result<Option<i64>> {
    conn.prepare_cached("SELECT last_fetched_at FROM anime_relations WHERE anime_id = ?1 LIMIT 1")?
        .query_row(params![anime_id], |r| r.get(0)).optional()
}
```

### 5.8 `getLatestAnimeHistory`

SQL (`animehistoryView.sq:46-58`):

```sql
getLatestAnimeHistory:
SELECT
id,
animeId,
episodeId,
title,
thumbnailUrl,
source,
favorite,
cover_last_modified,
episodeNumber,
seenAt
FROM animehistoryView
WHERE animehistoryView.seenAt > 0
ORDER BY seenAt DESC
LIMIT 1;
```

* Parameters: none. Returns: at most one row of 10 columns, mapped by `AnimeHistoryMapper.mapAnimeHistoryWithRelations(historyId, animeId, episodeId, title, thumbnailUrl, sourceId, isFavorite, coverLastModified, episodeNumber, seenAt)` (`AnimeHistoryMapper.kt:19-39`) into `AnimeHistoryWithRelations` with an `AnimeCover(animeId, sourceId, isAnimeFavorite, url = thumbnailUrl, lastModified = coverLastModified)`.
* Semantics: the single most recently watched episode over the whole library (reset rows with `seenAt = 0` are excluded by `> 0`). It does not need the `maxSeenAtEpisodeId` filter used by 5.34 because the global maximum is by definition the maximum of its own anime. Ties on `seenAt` pick an arbitrary row.
* Called by: `AnimeHistoryRepositoryImpl.kt:24` (`getLastAnimeHistory`) from `domain/src/main/java/tachiyomi/domain/history/anime/interactor/GetNextEpisodes.kt:17` (the "resume latest" action of the History tab). Manga twin: 5.9.
* Performance: the view joins four sources and a grouped subquery; ordering requires a sort of all history rows. Cheap for realistic history sizes (thousands of rows). Proposal avoids the view.
* Watch tables: not subscribed (one-shot).
* Proposal:

```rust
pub fn latest_anime_history(conn: &Connection) -> rusqlite::Result<Option<AnimeHistoryWithRelations>> {
    conn.prepare_cached(
        "SELECT h._id, a._id, e._id, a.title, a.thumbnail_url, a.source, a.favorite, a.cover_last_modified, \
                e.episode_number, h.last_seen \
         FROM animehistory h JOIN episodes e ON e._id = h.episode_id JOIN animes a ON a._id = e.anime_id \
         WHERE h.last_seen > 0 ORDER BY h.last_seen DESC, h._id DESC LIMIT 1")?
        .query_row([], anime_history_with_relations_from_row).optional()
}
```

### 5.9 `getLatestHistory`

SQL (`historyView.sq:48-63`):

```sql
getLatestHistory:
SELECT
id,
mangaId,
chapterId,
title,
thumbnailUrl,
source,
favorite,
cover_last_modified,
chapterNumber,
readAt,
readDuration
FROM historyView
WHERE historyView.readAt > 0
ORDER BY readAt DESC
LIMIT 1;
```

* Same semantics as 5.8 with 11 columns including `readDuration` (the history row's `time_read`). Mapper `MangaHistoryMapper.mapMangaHistoryWithRelations` (`MangaHistoryMapper.kt:21-48`) builds `MangaHistoryWithRelations` with `MangaCover(mangaId, sourceId, isMangaFavorite, url, lastModified)`.
* Called by: `MangaHistoryRepositoryImpl.kt:24` (`getLastMangaHistory`) from `domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetNextChapters.kt:17` (`await(onlyUnread)` without ids) from `app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt:107` (`getNextChapter()`, the Resume button in the History top bar, `MangaHistoryTab.kt:170`). The chosen chapter is the history's own chapter when it is not fully read, otherwise the following one (`GetNextChapters.kt:33-52`).
* Proposal:

```rust
pub fn latest_history(conn: &Connection) -> rusqlite::Result<Option<MangaHistoryWithRelations>> {
    conn.prepare_cached(
        "SELECT h._id, m._id, c._id, m.title, m.thumbnail_url, m.source, m.favorite, m.cover_last_modified, \
                c.chapter_number, h.last_read, h.time_read \
         FROM history h JOIN chapters c ON c._id = h.chapter_id JOIN mangas m ON m._id = c.manga_id \
         WHERE h.last_read > 0 ORDER BY h.last_read DESC, h._id DESC LIMIT 1")?
        .query_row([], manga_history_with_relations_from_row).optional()
}
```

### 5.10 `getMangaById`

SQL (`mangas.sq:63-66`):

```sql
getMangaById:
SELECT *
FROM mangas
WHERE _id = :id;
```

Anime twin `getAnimeById` (`animes.sq:75-78`) is the same over `animes`.

* Parameters: `id` Long. Returns: one full row. `awaitOne` / `subscribeToOne` throw when the row does not exist (SQLDelight `executeAsOne` fails with a null/state exception); `GetManga.await` catches every exception, logs it and returns null (`domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetManga.kt:12-19`).
* Semantics: primary-key lookup. NULL handling as in 5.1.
* Called by: `MangaRepositoryImpl.kt:21` (one-shot) and `:25` (`getMangaByIdAsFlow`, `subscribeToOne`), via `GetManga.kt:15` and `:23`; direct repository uses in `app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetMangaViewerFlags.kt:13,23`, `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt:54`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt:413,597`. Anime twin callers: `AnimeRepositoryImpl.kt:25,29`, `SetAnimeViewerFlags.kt:14,32,42`, `SetSeenStatus.kt:54`, `AnimeScreenModel.kt:489`, `app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt:81,134`.
* Performance: rowid lookup.
* Watch tables: `mangas` (the details screen re-reads the entry whenever any column changes).
* Proposal:

```rust
pub fn get_manga_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<Manga>> {
    let sql = format!("SELECT {MANGA_COLS} FROM mangas WHERE _id = ?1");
    conn.prepare_cached(&sql)?.query_row(params![id], manga_from_row).optional()
}
```

Missing rows are an `Option::None`, not an error; callers that want Aniyomi's "null on failure" behaviour use `.ok().flatten()`.

### 5.11 `getMangaByUrlAndSource`

SQL (`mangas.sq:69-74`; the line above it in the source carries a developer comment saying this should ideally never return more than one result):

```sql
getMangaByUrlAndSource:
SELECT *
FROM mangas
WHERE url = :url
AND source = :source
LIMIT 1;
```

Anime twin `getAnimeByUrlAndSource` (`animes.sq:81-85`) over `animes`.

* Parameters: `url` String (exact match), `source` Long. Returns: zero or one row.
* Semantics: this is the identity lookup "does this source entry already exist locally". `(source, url)` is not declared unique, so duplicates can exist (older imports and migrations); `LIMIT 1` without ORDER BY then returns an arbitrary one. Proposal makes the choice deterministic (lowest `_id`).
* Called by: `MangaRepositoryImpl.kt:30` (`getMangaByUrlAndSourceId`) and `:40` (`getMangaByUrlAndSourceIdAsFlow`, `subscribeToOneOrNull`). Consumers: `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/NetworkToLocalManga.kt:11,29` (the central "turn a source result into a local row" function: when no local row exists it inserts one and returns the copy with the new id; when the local row is not a favourite it only replaces the title with the source's title; otherwise it returns the stored row), `GetMangaByUrlAndSourceId` used by `MangaRestorer.kt:82` and `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreenModel.kt:77`, and the flow variant `GetManga.kt:27`. Anime twin: `AnimeRepositoryImpl.kt:34,44`, `NetworkToLocalAnime.kt:31`, `AnimeRestorer.kt:96`, `DeepLinkAnimeScreenModel.kt:74`, `GetAnime.kt:27`.
* Performance: `mangas_url_index` narrows by url then filters `source`; urls are nearly unique so this is O(1) in practice.
* Watch tables: `mangas`.
* Proposal: replace the url-only index with a composite one so the lookup is a single index probe, and make the tie-break explicit.

```sql
CREATE INDEX mangas_source_url_index ON mangas(source, url);
```

```rust
pub fn get_manga_by_url_and_source(conn: &Connection, url: &str, source: i64) -> rusqlite::Result<Option<Manga>> {
    let sql = format!("SELECT {MANGA_COLS} FROM mangas WHERE url = ?1 AND source = ?2 ORDER BY _id LIMIT 1");
    conn.prepare_cached(&sql)?.query_row(params![url, source], manga_from_row).optional()
}
```

### 5.12 `getMangasWithFavoriteTimestamp`

SQL (`mangas.sq:98-101`):

```sql
getMangasWithFavoriteTimestamp:
SELECT *
FROM mangas
WHERE favorite_modified_at IS NOT NULL;
```

Anime twin `getAnimesWithFavoriteTimestamp` (`animes.sq:109-112`) over `animes`.

* OBSERVED: no Kotlin caller exists in the repository (a search for `getMangasWithFavoriteTimestamp` and `getAnimesWithFavoriteTimestamp` finds only the `.sq` definitions); it is dead code left from the sync feature, where it was meant to select entries whose favourite state has a timestamp so a sync merge could resolve favourite conflicts. The column it filters is still maintained (trigger `update_last_favorite_at_mangas`, section 1.5) and is exported in the backup protobuf (`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupManga.kt:40`, field number 107 `favoriteModifiedAt`; `MangaBackupCreator.kt:100`).
* Returns: full rows. Note that `favorite_modified_at` is NULL for rows created by `insert` (the insert statement never sets it) and becomes non-NULL after the first partial update of any kind (see the "named in SET" rule in 1.5), so this query would not really mean "favourited entries".
* Performance: full table scan (no index on the column).
* Proposal: drop the statement (no feature in Kurobayomi uses it). Keep the column `favorite_modified_at` and its trigger so that backup export keeps writing field 107 (area:21-backup-format-and-create). What the user loses: nothing.

### 5.13 `getReadDuration`

SQL (`history.sq:71-73`):

```sql
getReadDuration:
SELECT coalesce(sum(time_read), 0)
FROM history;
```

* Parameters: none. Returns: one Long: total reading time in milliseconds, 0 when the table is empty.
* Semantics: sums `time_read` of ALL rows including reset rows (`last_read = 0`), so "Remove from history" in the UI (which only sets `last_read = 0`, item 5.34) does not reduce the statistic; only "Delete all history" (`removeAllHistory`) and the clear-database action (`removeResettedHistory`, `ClearDatabaseScreen.kt:213`) lower it. The anime database has no twin (no duration column).
* Called by: `MangaHistoryRepositoryImpl.kt:29` (`getTotalReadDuration`) via `domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetTotalReadDuration.kt:10` from `app/src/main/java/eu/kanade/tachiyomi/ui/stats/manga/MangaStatsScreenModel.kt:58`, displayed by `app/src/main/java/eu/kanade/presentation/more/stats/MangaStatsScreenContent.kt:61-64` (`toDuration(DurationUnit.MILLISECONDS).toDurationString(context, fallback = none)`).
* Performance: full scan of `history`; microseconds for thousands of rows.
* Proposal:

```rust
pub fn total_read_duration_ms(conn: &Connection) -> rusqlite::Result<i64> {
    conn.prepare_cached("SELECT coalesce(sum(time_read), 0) FROM history")?.query_row([], |r| r.get(0))
}
```

### 5.14 `getReadMangaNotInLibrary`

SQL (`mangas.sq:81-88`):

```sql
getReadMangaNotInLibrary:
SELECT *
FROM mangas
WHERE favorite = 0 AND _id IN (
    SELECT DISTINCT chapters.manga_id
    FROM chapters
    WHERE read = 1 OR last_page_read != 0
);
```

Anime twin `getWatchedAnimeNotInLibrary` (item 5.33).

* Parameters: none. Returns: full `mangas` rows of entries that are not in the library but have at least one chapter that is marked read or has a saved page position (`last_page_read` is the reader's resume page, 0 = untouched).
* Semantics: this is the set a backup should also carry so that read progress of browsed-but-not-favourited manga survives a restore. NULL handling: none (`read` and `last_page_read` are NOT NULL).
* Called by: `MangaRepositoryImpl.kt:53` (`getReadMangaNotInLibrary`) from `BackupCreator.kt:108`, only when `options.readEntries` is true (default true, `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt:14`, UI label `AYMR.strings.non_library_settings`), and only enabled in the UI when "library entries" is on (`BackupOptions.kt:79`). The result is concatenated after `getMangaFavorites.await()` (`BackupCreator.kt:112`).
* Performance: the subquery scans the whole `chapters` table once (the partial index `chapters_unread_by_manga_index` covers only `read = 0` rows and cannot help), then a hash of manga ids. With 100k chapters that is tens of milliseconds; acceptable for a backup but wasteful. The EXISTS form below probes `chapters_manga_id_index` for each non-favourite entry only and stops at the first match.
* Proposal:

```rust
pub fn read_manga_not_in_library(conn: &Connection) -> rusqlite::Result<Vec<Manga>> {
    let sql = format!(
        "SELECT {MANGA_COLS} FROM mangas m WHERE m.favorite = 0 AND EXISTS (\
           SELECT 1 FROM chapters c WHERE c.manga_id = m._id AND (c.read = 1 OR c.last_page_read != 0)) \
         ORDER BY m._id");
    conn.prepare_cached(&sql)?.query_map([], manga_from_row)?.collect()
}
```

(The column list must be prefixed with `m.` in the real code: build it with a helper `cols_prefixed("m", MANGA_COLS)`.)

### 5.15 `getRecentAnimeUpdates`

SQL (`animeupdatesView.sq:25-29`), reading the view `animeupdatesView` (`animeupdatesView.sq:1-23`):

```sql
CREATE VIEW animeupdatesView AS
SELECT
    animes._id AS animeId,
    animes.title AS animeTitle,
    episodes._id AS episodeId,
    episodes.name AS episodeName,
    episodes.scanlator,
    episodes.seen,
    episodes.bookmark,
    episodes.fillermark,
    episodes.last_second_seen,
    episodes.total_seconds AS totalSeconds,
    animes.source,
    animes.favorite,
    animes.thumbnail_url AS thumbnailUrl,
    animes.cover_last_modified AS coverLastModified,
    episodes.date_upload AS dateUpload,
    episodes.date_fetch AS datefetch
FROM animes JOIN episodes
ON animes._id = episodes.anime_id
WHERE favorite = 1
AND date_fetch > date_added
ORDER BY date_fetch DESC;

getRecentAnimeUpdates:
SELECT *
FROM animeupdatesView
WHERE dateUpload > :after
LIMIT :limit;
```

* Parameters: `after` Long (epoch ms, compared with `episodes.date_upload`), `limit` Long (always 500). Returns 16 columns: `animeId, animeTitle, episodeId, episodeName, scanlator (nullable), seen, bookmark, fillermark, last_second_seen, totalSeconds, source, favorite, thumbnailUrl (nullable), coverLastModified, dateUpload, datefetch`, mapped by the private `mapUpdatesWithRelations` in `data/src/main/java/tachiyomi/data/updates/anime/AnimeUpdatesRepositoryImpl.kt:45-82` into `AnimeUpdatesWithRelations` with an `AnimeCover`.
* Semantics: one row per episode of a favourite anime whose `date_fetch` is later than the anime's `date_added`, i.e. episodes discovered after the anime joined the library (episodes already present at favouriting time are excluded; this relies on the entry being favourited after its first episode list was fetched), restricted to episodes uploaded after `after`. Ordering comes from the view's `ORDER BY date_fetch DESC`, which SQLite applies to the outer query because the outer query has none; SQL does not guarantee this. Episodes with the same `date_fetch` tie arbitrarily (a sync stamps `nowMillis + itemCount--`, so values from one fetch are distinct, `SyncEpisodesWithSource.kt:197`). Column `datefetch` is spelled with a lowercase f in the view and read positionally by the mapper (`dateFetch`).
* Called by: `AnimeUpdatesRepositoryImpl.kt:26` (`subscribeAllAnimeUpdates`, `subscribeToList`) via `domain/src/main/java/tachiyomi/domain/updates/anime/interactor/GetAnimeUpdates.kt:16-18` (`subscribe(instant)`, limit 500) from `app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt:82`, where `limit = ZonedDateTime.now().minusMonths(3).toInstant()` (the Updates screen shows the last 3 months) and the screen groups rows by the local date of `dateFetch`.
* Performance: no index on `episodes.date_upload` or `date_fetch`; the query joins `animes` (favourites) with every `episodes` row, filters and sorts: O(total episodes of favourites). The ORDER BY then LIMIT 500 forces a full sort of the matching rows. For libraries with 100k+ episodes this is the slowest recurring query in the app (it re-runs after every write to `animes` or `episodes`, including each playback progress save). Proposal adds an index and debounces the feed.
* Watch tables: `animes`, `episodes`.
* Proposal: define the view without ORDER BY and order in the query; add one index.

```sql
CREATE VIEW animeupdatesView AS
SELECT animes._id AS animeId, animes.title AS animeTitle, episodes._id AS episodeId, episodes.name AS episodeName,
       episodes.scanlator, episodes.seen, episodes.bookmark, episodes.fillermark, episodes.last_second_seen,
       episodes.total_seconds AS totalSeconds, animes.source, animes.favorite, animes.thumbnail_url AS thumbnailUrl,
       animes.cover_last_modified AS coverLastModified, episodes.date_upload AS dateUpload, episodes.date_fetch AS datefetch
FROM animes JOIN episodes ON animes._id = episodes.anime_id
WHERE favorite = 1 AND date_fetch > date_added;
CREATE INDEX episodes_date_fetch_index ON episodes(date_fetch DESC);
```

```rust
pub fn recent_anime_updates(conn: &Connection, after_ms: i64, limit: i64) -> rusqlite::Result<Vec<AnimeUpdate>> {
    conn.prepare_cached(
        "SELECT animeId, animeTitle, episodeId, episodeName, scanlator, seen, bookmark, fillermark, \
                last_second_seen, totalSeconds, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch \
         FROM animeupdatesView WHERE dateUpload > ?1 ORDER BY datefetch DESC, episodeId DESC LIMIT ?2")?
        .query_map(params![after_ms, limit], anime_update_from_row)?.collect()
}
```

Call with `after_ms = (now - 3 months).timestamp_millis()` (calendar months, `chrono::Months::new(3)`) and `limit = 500`.

### 5.16 `getRecentUpdates`

SQL (`updatesView.sq:23-27`), reading the view `updatesView` (`updatesView.sq:1-21`):

```sql
CREATE VIEW updatesView AS
SELECT
    mangas._id AS mangaId,
    mangas.title AS mangaTitle,
    chapters._id AS chapterId,
    chapters.name AS chapterName,
    chapters.scanlator,
    chapters.read,
    chapters.bookmark,
    chapters.last_page_read,
    mangas.source,
    mangas.favorite,
    mangas.thumbnail_url AS thumbnailUrl,
    mangas.cover_last_modified AS coverLastModified,
    chapters.date_upload AS dateUpload,
    chapters.date_fetch AS datefetch
FROM mangas JOIN chapters
ON mangas._id = chapters.manga_id
WHERE favorite = 1
AND date_fetch > date_added
ORDER BY date_fetch DESC;

getRecentUpdates:
SELECT *
FROM updatesView
WHERE dateUpload > :after
LIMIT :limit;
```

* Same semantics as 5.15 for manga. Returns 14 columns `mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, last_page_read, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch`, mapped by `mapUpdatesWithRelations` (`data/src/main/java/tachiyomi/data/updates/manga/MangaUpdatesRepositoryImpl.kt:41-74`) into `MangaUpdatesWithRelations`. The view was created by `18.sqm`, extended with `last_page_read` by `23.sqm`.
* Date semantics worth knowing: `dateUpload` is the source's upload date; when a source gives no date the sync stamps the maximum upload date seen so far, or "now" when none (`app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt:107-110`), so every chapter has a non-zero `date_upload` and passes the `> after` test while it is recent. A chapter re-added after having been deleted reuses its old `date_fetch` so it does not reappear as new (`SyncChaptersWithSource.kt:175-176,200-202`).
* Called by: `MangaUpdatesRepositoryImpl.kt:26` (`subscribeAllMangaUpdates`) via `domain/src/main/java/tachiyomi/domain/updates/manga/interactor/GetMangaUpdates.kt:16-18` (`subscribe(instant)`, limit 500) from `app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt:78` (last 3 months, grouped by local date of `dateFetch`, `:375-379`).
* Performance and Watch tables: as 5.15 with `mangas`, `chapters`.
* Proposal:

```sql
CREATE VIEW updatesView AS
SELECT mangas._id AS mangaId, mangas.title AS mangaTitle, chapters._id AS chapterId, chapters.name AS chapterName,
       chapters.scanlator, chapters.read, chapters.bookmark, chapters.last_page_read, mangas.source, mangas.favorite,
       mangas.thumbnail_url AS thumbnailUrl, mangas.cover_last_modified AS coverLastModified,
       chapters.date_upload AS dateUpload, chapters.date_fetch AS datefetch
FROM mangas JOIN chapters ON mangas._id = chapters.manga_id
WHERE favorite = 1 AND date_fetch > date_added;
CREATE INDEX chapters_date_fetch_index ON chapters(date_fetch DESC);
```

```rust
pub fn recent_updates(conn: &Connection, after_ms: i64, limit: i64) -> rusqlite::Result<Vec<MangaUpdate>> {
    conn.prepare_cached(
        "SELECT mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, last_page_read, \
                source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch \
         FROM updatesView WHERE dateUpload > ?1 ORDER BY datefetch DESC, chapterId DESC LIMIT ?2")?
        .query_map(params![after_ms, limit], manga_update_from_row)?.collect()
}
```

### 5.17 `getRelatedAnimeByAnimeId`

SQL (`anime_relations.sq:17-23`):

```sql
getRelatedAnimeByAnimeId:
SELECT A.*, R.name
FROM anime_relations AS R
JOIN animes AS A
ON A._id = R.related_anime_id
WHERE R.anime_id = :animeId
ORDER BY R.sort_order ASC;
```

* Parameters: `animeId`. Returns: all 32 columns of the related `animes` row followed by `name` (the relation group title), mapped by `AnimeMapper.mapRelatedAnime` (the 31 anime columns followed by the extra parameter `name`) into `Pair<String, Anime>` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeMapper.kt:250`).
* Semantics: relations stored for one anime in source order. `sort_order` is a single running counter across all groups of one refresh (group 1 gets 0..k, group 2 continues at k+1; `AnimeRelationRepositoryImpl.kt:24-34`), so ordering by it preserves both group order and in-group order. The repository then does `rows.groupBy({ it.first }, { it.second })`, which keeps first-seen group order (`AnimeRelationRepositoryImpl.kt:48-50`) and yields `AnimeRelationGroup(name, anime)`. The same related anime may appear under two group names because the key is `(anime_id, related_anime_id, name)`. Related anime are ordinary `animes` rows created through `NetworkToLocalAnime` (not favourites); deleting such an anime cascades its relation rows in both directions.
* Called by: `AnimeRelationRepositoryImpl.kt:42` (`subscribeRelatedAnime`, `subscribeToList`) via `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetRelatedAnime.kt:11` from `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:1463` (the "Related" section of the anime details screen; refreshed by `SyncRelatedAnimeWithSource`, 5.7).
* Performance: `anime_relations_anime_id_index`, PK lookup into `animes`, sort on at most a few dozen rows.
* Watch tables: `anime_relations`, `animes`.
* Proposal:

```rust
pub fn related_anime(conn: &Connection, anime_id: i64) -> rusqlite::Result<Vec<(String, Anime)>> {
    let sql = format!("SELECT {}, R.name FROM anime_relations R JOIN animes A ON A._id = R.related_anime_id \
                       WHERE R.anime_id = ?1 ORDER BY R.sort_order ASC", prefixed("A", ANIME_COLS));
    conn.prepare_cached(&sql)?
        .query_map(params![anime_id], |r| Ok((r.get::<_, String>(31)?, anime_from_row(r)?)))?.collect()
}
```

The caller groups consecutive rows with equal name; the name column is index 31 because `ANIME_COLS` has 31 columns (indexes 0 to 30).

### 5.18 `getScanlatorsByMangaId`

SQL (`chapters.sq:68-71`):

```sql
getScanlatorsByMangaId:
SELECT scanlator
FROM chapters
WHERE manga_id = :mangaId;
```

* Parameters: `mangaId`. Returns: one nullable text per chapter (NOT distinct; duplicates for every chapter of the same group; NULL for chapters without scanlator). The repository maps NULL to the empty string (`ChapterRepositoryImpl.kt:91,97`: `chaptersQueries.getScanlatorsByMangaId(mangaId) { it.orEmpty() }`).
* Semantics: feeds the "Filter by scanlator" picker. `GetAvailableScanlators` (`app/src/main/java/eu/kanade/domain/items/chapter/interactor/GetAvailableScanlators.kt:10-23`) turns the list into a `Set<String>` after dropping blank values (`ifBlank { null }`, Kotlin whitespace rule) so the UI sees each non-blank name once. There is no manga/anime asymmetry to consider: the anime side has no scanlator-exclusion feature (no `excluded_scanlators` in the anime database), only the `scanlator` column.
* Called by: `ChapterRepositoryImpl.kt:91` (`getScanlatorsByMangaId`) and `:97` (`getScanlatorsByMangaIdAsFlow`, `subscribeToList`) via `GetAvailableScanlators.kt:16,21` from `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt:203` (`subscribe`, stored as `availableScanlators` in the screen state) and `:234` (`await`); the state computes `scanlatorFilterActive = excludedScanlators.intersect(availableScanlators).isNotEmpty()` (`:1192-1193`).
* Performance: `chapters_manga_id_index`; transfers one string per chapter (thousands for long series) only to de-duplicate in memory.
* Watch tables: `chapters`.
* Proposal: de-duplicate in SQL, keep the blank filter in Rust (SQLite `trim` strips only spaces, Kotlin `isBlank` strips all Unicode whitespace).

```rust
pub fn available_scanlators(conn: &Connection, manga_id: i64) -> rusqlite::Result<Vec<String>> {
    let mut v: Vec<String> = conn
        .prepare_cached("SELECT DISTINCT scanlator FROM chapters WHERE manga_id = ?1 AND scanlator IS NOT NULL")?
        .query_map(params![manga_id], |r| r.get::<_, String>(0))?
        .collect::<Result<_, _>>()?;
    v.retain(|s| !s.trim().is_empty());
    Ok(v)
}
```

### 5.19 `getSourceIdWithFavoriteCount`

SQL, manga (`mangas.sq:103-109`):

```sql
getSourceIdWithFavoriteCount:
SELECT
source,
count(*)
FROM mangas
WHERE favorite = 1
GROUP BY source;
```

Anime twin `getAnimeSourceIdWithFavoriteCount` (`animes.sq:114-120`): identical text over `animes`.

* Parameters: none. Returns: rows `(source Long, count Long)`; the unaliased `count(*)` column is called `count` by SQLDelight and destructured positionally as `(sourceId, count)`.
* Semantics: library entries per source; sources with zero favourites do not appear. No ORDER BY (the interactor sorts). The Kotlin side resolves each id to a source object with `sourceManager.getOrStub(sourceId)` (an uninstalled extension yields a stub flagged `isStub = true`, `MangaSourceRepositoryImpl.kt:44-52`) and re-emits when the installed-source list changes (`combine` of the `subscribeToList` flow and `sourceManager.catalogueSources`, `:42-46`).
* Called by: `data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt:43` (`getMangaSourcesWithFavoriteCount`) via `app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaSourcesWithFavoriteCount.kt:22`, which drops the local source (`LocalMangaSource.ID = 0L`) and sorts by the persisted migration sort preferences: mode `ALPHABETICAL` (stubs first, then `name.lowercase()` compared with `compareToWithCollator` of the other name) or `TOTAL` (stubs first, then count ascending), direction ascending or descending (`GetMangaSourcesWithFavoriteCount.kt:25-26,30-56`); screen model `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/sources/MigrateMangaSourceScreenModel.kt:37`. Anime: `AnimeSourceRepositoryImpl.kt:41`, `MigrateAnimeSourceScreenModel.kt:37`.
* Performance: index scan of the partial favourite index plus a temp b-tree for the group; trivial.
* Watch tables: `mangas`.
* Proposal:

```rust
pub fn favorite_count_by_source(conn: &Connection) -> rusqlite::Result<Vec<(i64, i64)>> {
    conn.prepare_cached("SELECT source, count(*) FROM mangas WHERE favorite = 1 GROUP BY source ORDER BY source")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
}
```

The source-name sort uses `icu_collator` / `str::to_lowercase` with a locale-aware compare in the UI layer (area:40-ui-browse-manga), not SQL.

### 5.20 `getSourceIdsWithNonLibraryManga`

SQL (`mangas.sq:141-145`):

```sql
getSourceIdsWithNonLibraryManga:
SELECT source, COUNT(*) AS manga_count
FROM mangas
WHERE favorite = 0
GROUP BY source;
```

* Parameters: none. Returns: `(source Long, manga_count Long)` per source that has at least one non-library entry (entries created by browsing, searching or opening a source result; the local source id 0 is included).
* Semantics: feeds Settings > Advanced > "Clear database": the user ticks sources and the app runs, in this order (`app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearDatabaseScreen.kt:211-213`): `mangasQueries.deleteMangasNotInLibraryBySourceIds(sourceIds)` (`mangas.sq:147-150`: `DELETE FROM mangas WHERE favorite = 0 AND source IN :sourceIds;`) and then `historyQueries.removeResettedHistory()` (`history.sq:57-59`: `DELETE FROM history WHERE last_read = 0;`). Cascades remove the chapters of the deleted entries and, through them, their history rows and tracks. The second statement is global, not limited to the cleared sources: it also permanently discards every reset history row of library entries, which lowers `getReadDuration` (5.13). The anime database has no direct twin: `GetAnimeSourcesWithNonLibraryAnime.subscribe()` (`domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetAnimeSourcesWithNonLibraryAnime.kt:11-13`) returns the deletable parent entries from the view `animedeletableView` (a view owned by another chunk) and `getDeletableChildren(parentId)` reads `getChildrenByParentId` for the season tree.
* Called by: `MangaSourceRepositoryImpl.kt:59` (`getMangaSourcesWithNonLibraryManga`, `subscribeToList`) via `domain/src/main/java/tachiyomi/domain/source/manga/interactor/GetMangaSourcesWithNonLibraryManga.kt:12`, consumed at `ClearDatabaseScreen.kt:192,197`; the screen sorts by source name (`list.sortedBy { it.name }`, `:200`).
* Performance: scan of `mangas` with a temp group b-tree. There is no index on `favorite = 0`; for 100k stored entries this is a few milliseconds.
* Watch tables: `mangas`.
* Proposal:

```rust
pub fn non_library_manga_count_by_source(conn: &Connection) -> rusqlite::Result<Vec<(i64, i64)>> {
    conn.prepare_cached("SELECT source, COUNT(*) AS manga_count FROM mangas WHERE favorite = 0 GROUP BY source ORDER BY source")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
}
pub fn clear_non_library_by_source(conn: &mut Connection, source_ids: &[i64]) -> rusqlite::Result<usize> {
    let tx = conn.transaction()?;
    let sql = format!("DELETE FROM mangas WHERE favorite = 0 AND source IN ({})", placeholders(source_ids.len()));
    let n = tx.execute(&sql, rusqlite::params_from_iter(source_ids))?;
    tx.execute("DELETE FROM history WHERE last_read = 0", [])?;
    tx.commit()?;
    Ok(n)
}
```

Proposal: restrict the second delete to rows of the deleted entries is not possible (they are already gone through the cascade), so keep Aniyomi's behaviour but show the confirmation text that reset history is purged too.

### 5.21 `getTrackByAnimeId`

SQL (`anime_sync.sq:31-35`):

```sql
getTrackByAnimeId:
SELECT *
FROM anime_sync
WHERE _id = :id;
```

* OBSERVED naming trap: despite the name the parameter is the track row id `anime_sync._id`, not an anime id. The manga twin is called `getTrackById` (5.22), which is the honest name.
* Parameters: `id` Long (track row id). Returns: zero or one row; columns in order `_id, anime_id, sync_id, remote_id, library_id, title, last_episode_seen, total_episodes, status, score, remote_url, start_date, finish_date, private`, mapped by `AnimeTrackMapper.mapTrack(id, animeId, syncId, remoteId, libraryId, title, lastEpisodeSeen, totalEpisodes, status, score, remoteUrl, startDate, finishDate, private)` into `AnimeTrack` (with `trackerId = syncId`) (`data/src/main/java/tachiyomi/data/track/anime/AnimeTrackMapper.kt:5-33`).
* Called by: `data/src/main/java/tachiyomi/data/track/anime/AnimeTrackRepositoryImpl.kt:13` (`getTrackByAnimeId`, `awaitOneOrNull`) via `domain/src/main/java/tachiyomi/domain/track/anime/interactor/GetAnimeTracks.kt:13-19` (`awaitOne(id)`: any exception is logged and turned into null) from `app/src/main/java/eu/kanade/domain/track/anime/service/DelayedAnimeTrackingUpdateJob.kt:39`, which resolves queued "update this track later" items by the `trackId` stored in `DelayedAnimeTrackingStore`; if the track no longer exists the queued item is removed (`:40-42`) and the job gives up after `runAttemptCount > 3` (`:27`).
* Important interaction with `insert` (5.37): `anime_sync` is declared `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE` and every track save is an INSERT without `_id`, so saving an existing track deletes the old row and creates a new row with a new `_id`. A delayed-update item that was queued with the old `_id` therefore resolves to null on the next retry and is discarded.
* Performance: rowid lookup.
* Proposal: use the honest name and make saves stable (see `insert`, 5.37, `anime_sync` statement) so queued ids stay valid:

```rust
pub fn get_track_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<AnimeTrack>> {
    conn.prepare_cached(
        "SELECT _id, anime_id, sync_id, remote_id, library_id, title, last_episode_seen, total_episodes, \
                status, score, remote_url, start_date, finish_date, private FROM anime_sync WHERE _id = ?1")?
        .query_row(params![id], anime_track_from_row).optional()
}
```

### 5.22 `getTrackById`

SQL (`manga_sync.sq:31-34`):

```sql
getTrackById:
SELECT *
FROM manga_sync
WHERE _id = :id;
```

* Parameters: `id` Long (track row id). Returns: zero or one row of `_id, manga_id, sync_id, remote_id, library_id, title, last_chapter_read, total_chapters, status, score, remote_url, start_date, finish_date, private`, mapped by `MangaTrackMapper.mapTrack` (`data/src/main/java/tachiyomi/data/track/manga/MangaTrackMapper.kt:5-35`) with `trackerId = syncId`.
* Called by: `data/src/main/java/tachiyomi/data/track/manga/MangaTrackRepositoryImpl.kt:13` (repository method misleadingly named `getTrackByMangaId(id)`) via `domain/src/main/java/tachiyomi/domain/track/manga/interactor/GetMangaTracks.kt:13-19` (`awaitOne`, exception becomes null) from `app/src/main/java/eu/kanade/domain/track/manga/service/DelayedMangaTrackingUpdateJob.kt:38` (same delayed-update flow and the same stale-id trap as 5.21; the job gives up after `runAttemptCount > 3`, `:26`).
* Performance: rowid lookup.
* Proposal:

```rust
pub fn get_track_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<MangaTrack>> {
    conn.prepare_cached(
        "SELECT _id, manga_id, sync_id, remote_id, library_id, title, last_chapter_read, total_chapters, \
                status, score, remote_url, start_date, finish_date, private FROM manga_sync WHERE _id = ?1")?
        .query_row(params![id], manga_track_from_row).optional()
}
```

### 5.23 `getTracks`

SQL (`manga_sync.sq:27-29`):

```sql
getTracks:
SELECT *
FROM manga_sync;
```

Anime twin `getAnimeTracks` (`anime_sync.sq:27-29`) over `anime_sync`.

* Parameters: none. Returns: every tracker binding of every manga (14 columns as 5.22). No ORDER BY.
* Semantics: used to answer "which manga have which trackers" in one query.
* Called by: `MangaTrackRepositoryImpl.kt:24` (`getMangaTracksAsFlow`, `subscribeToList`) via `domain/src/main/java/tachiyomi/domain/track/manga/interactor/GetTracksPerManga.kt:13` (`groupBy { it.mangaId }`) from `app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt:109` (the library tracking filter and the "tracker mean" sort). Anime: `AnimeTrackRepositoryImpl.kt:24`, `GetTracksPerAnime`, `AnimeLibraryScreenModel.kt:112`.
* Performance: full scan; one row per binding (hundreds to a few thousand). Re-runs after every write to `manga_sync`, which happens whenever reading progress is pushed to a tracker.
* Watch tables: `manga_sync`.
* Proposal:

```rust
pub fn all_tracks(conn: &Connection) -> rusqlite::Result<Vec<MangaTrack>> {
    conn.prepare_cached(
        "SELECT _id, manga_id, sync_id, remote_id, library_id, title, last_chapter_read, total_chapters, \
                status, score, remote_url, start_date, finish_date, private FROM manga_sync ORDER BY _id")?
        .query_map([], manga_track_from_row)?.collect()
}
```

### 5.24 `getTracksByAnimeId`

SQL (`anime_sync.sq:37-40`):

```sql
getTracksByAnimeId:
SELECT *
FROM anime_sync
WHERE anime_id = :animeId;
```

* Parameters: `animeId`. Returns: the bindings of one anime (at most one per tracker because of the UNIQUE constraint), mapped as 5.21.
* Called by: `AnimeTrackRepositoryImpl.kt:18` (`getTracksByAnimeId`, one-shot) and `:30` (`getTracksByAnimeIdAsFlow`, `subscribeToList`) via `GetAnimeTracks.kt:24` (`await`, exceptions yield an empty list) and `:32` (`subscribe`); consumers: `app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt:27` (push new progress to each logged-in tracker whose stored `lastEpisodeSeen` is lower), `RefreshAnimeTracks.kt:27`, `AnimeRestorer.kt:401`, and `AnimeBackupCreator.kt:54` (reads the table directly with `getTracksByAnimeId(anime.id, backupAnimeTrackMapper)` when `options.tracking`).
* Performance: no index on `anime_id` except the one implied by `UNIQUE (anime_id, sync_id)` (its leading column serves this lookup).
* Watch tables: `anime_sync`.
* Proposal:

```rust
pub fn tracks_by_anime_id(conn: &Connection, anime_id: i64) -> rusqlite::Result<Vec<AnimeTrack>> {
    conn.prepare_cached(
        "SELECT _id, anime_id, sync_id, remote_id, library_id, title, last_episode_seen, total_episodes, \
                status, score, remote_url, start_date, finish_date, private FROM anime_sync WHERE anime_id = ?1 ORDER BY sync_id")?
        .query_map(params![anime_id], anime_track_from_row)?.collect()
}
```

### 5.25 `getTracksByMangaId`

SQL (`manga_sync.sq:36-39`):

```sql
getTracksByMangaId:
SELECT *
FROM manga_sync
WHERE manga_id = :mangaId;
```

* Parameters: `mangaId`. Returns: bindings of one manga, mapped as 5.22.
* Called by: `MangaTrackRepositoryImpl.kt:18` (one-shot) and `:30` (`getTracksByMangaIdAsFlow`) via `GetMangaTracks.kt:24` and `:32`; consumers `app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt:26`, `RefreshMangaTracks.kt:27`, `MangaRestorer.kt:362`, `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt:659` (tracker URLs written into the downloaded `ComicInfo.xml`), and `MangaBackupCreator.kt:59` (direct read when `options.tracking`).
* Restore rule built on it (`MangaRestorer.kt:361-421`): per backup track, a track with a tracker id not yet bound is inserted with `id = 0` (new row); an existing binding is skipped when equal ignoring `id` and `mangaId`; otherwise the stored row is updated to take the backup's `remoteId` and `libraryId` and `max(db.lastChapterRead, backup.lastChapterRead)` via `manga_syncQueries.update`.
* Watch tables: `manga_sync`.
* Proposal:

```rust
pub fn tracks_by_manga_id(conn: &Connection, manga_id: i64) -> rusqlite::Result<Vec<MangaTrack>> {
    conn.prepare_cached(
        "SELECT _id, manga_id, sync_id, remote_id, library_id, title, last_chapter_read, total_chapters, \
                status, score, remote_url, start_date, finish_date, private FROM manga_sync WHERE manga_id = ?1 ORDER BY sync_id")?
        .query_map(params![manga_id], manga_track_from_row)?.collect()
}
```

### 5.26 `getUpcomingAnime`

SQL (`animes.sq:135-141`):

```sql
getUpcomingAnime:
SELECT *
FROM animes
WHERE next_update >= :startOfDay
AND favorite = 1
AND status IN :statuses
ORDER BY next_update ASC;
```

* Parameters: `startOfDay` Long = local midnight today in epoch ms, computed in the repository as `LocalDate.now().atStartOfDay(ZoneId.systemDefault()).toEpochSecond() * 1000` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:79`); `statuses` Set<Long> = `{SAnime.ONGOING, SAnime.PUBLISHING_FINISHED, SAnime.UPCOMING}` = `{1, 4, 7}` (`domain/src/main/java/mihon/domain/upcoming/anime/interactor/GetUpcomingAnime.kt:11-15`). Returns: full anime rows.
* Semantics: library entries whose predicted next release is today or later and whose publishing status can still release. Rows with `next_update` NULL, 0 or in the past are excluded (a comparison with NULL is not true); anime with status unknown/completed/licensed/cancelled/on-hiatus are excluded. Ordered by soonest prediction first; rows with the same instant tie arbitrarily. `next_update` is written by the library updater: `MangaFetchInterval` / `AnimeFetchInterval` computes `nextUpdate` and `fetchInterval` (`domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt:31-33`; area:26-library-update-jobs owns the algorithm).
* Called by: `AnimeRepositoryImpl.kt:81` (`getUpcomingAnime`, `subscribeToList`) via `GetUpcomingAnime.kt:19` from the Upcoming calendar screen `app/src/main/java/mihon/feature/upcoming/anime/UpcomingAnimeScreenModel.kt` (rows are grouped by the local date of `expectedNextUpdate`; `expectedNextUpdate` is null when `status == COMPLETED`, which the status filter already excludes). The flow is created once with the `startOfDay` of the moment the screen opened, so a screen kept open across midnight keeps yesterday's boundary.
* Performance: `animelib_favorite_index` then filter and sort; fine.
* Watch tables: `animes`.
* Proposal:

```rust
pub fn upcoming_anime(conn: &Connection, start_of_day_ms: i64) -> rusqlite::Result<Vec<Anime>> {
    let sql = format!(
        "SELECT {ANIME_COLS} FROM animes WHERE next_update >= ?1 AND favorite = 1 AND status IN (1, 4, 7) \
         ORDER BY next_update ASC, _id ASC");
    conn.prepare_cached(&sql)?.query_map(params![start_of_day_ms], anime_from_row)?.collect()
}
```

Call with `start_of_local_day_ms()` (section 1.7) and re-run the watch when the local date changes.

### 5.27 `getUpcomingManga`

SQL (`mangas.sq:124-130`):

```sql
getUpcomingManga:
SELECT *
FROM mangas
WHERE next_update >= :startOfDay
AND favorite = 1
AND status IN :statuses
ORDER BY next_update ASC;
```

* Same as 5.26 for manga with `statuses = {SManga.ONGOING, SManga.PUBLISHING_FINISHED}` = `{1, 4}` (`domain/src/main/java/mihon/domain/upcoming/manga/interactor/GetUpcomingManga.kt:10-13`; `UPCOMING` does not exist for manga).
* Called by: `MangaRepositoryImpl.kt:77` (`startOfDay` computed at `:75`) via `GetUpcomingManga.kt:18` from `app/src/main/java/mihon/feature/upcoming/manga/UpcomingMangaScreenModel.kt:26,31` which inserts a date header between rows with different local `expectedNextUpdate` dates and counts the rows per date for the calendar dots (`:41-70`).
* Proposal:

```rust
pub fn upcoming_manga(conn: &Connection, start_of_day_ms: i64) -> rusqlite::Result<Vec<Manga>> {
    let sql = format!(
        "SELECT {MANGA_COLS} FROM mangas WHERE next_update >= ?1 AND favorite = 1 AND status IN (1, 4) \
         ORDER BY next_update ASC, _id ASC");
    conn.prepare_cached(&sql)?.query_map(params![start_of_day_ms], manga_from_row)?.collect()
}
```

### 5.28 `getUpdatesByReadStatus`

SQL (`updatesView.sq:29-34`; the source has a blank line before `LIMIT`):

```sql
getUpdatesByReadStatus:
SELECT *
FROM updatesView
WHERE read = :read
AND dateUpload > :after

LIMIT :limit;
```

* Parameters: `read` Boolean bound as 0/1, `after` Long epoch ms, `limit` Long (500). Returns the 14 columns of `updatesView` (5.16).
* Semantics: same view and ordering rule as 5.16 filtered by read state. The only live consumer asks for `read = false`: the unread updates shown by the Android home-screen widget. `GetMangaUpdates.await(read, after)` (`GetMangaUpdates.kt:12-14`, repository `awaitWithRead`, `MangaUpdatesRepositoryImpl.kt:13-20`) has no caller in the repository.
* Called by: `MangaUpdatesRepositoryImpl.kt:15` (one-shot `awaitWithRead`) and `:32` (`subscribeWithRead`, `subscribeToList`) via `GetMangaUpdates.kt:20-22` (`subscribe(read, after)`, limit 500) from `presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt:24` and `BaseMangaUpdatesGridGlanceWidget.kt:96`, both with `after = ZonedDateTime.now().minusMonths(3).toInstant()` (`BaseMangaUpdatesGridGlanceWidget.kt:159-160`, `DateLimit`).
* Decision (P3): Android app widgets (Glance) have no Linux equivalent in a GTK desktop app; Kurobayomi drops the widget (area:50-android-widgets). What the user loses: the home-screen "recent updates" grid. The statement is still useful as the source of an "unread updates" badge, so port it as below but do not build UI for it in the first milestones.
* Watch tables: `mangas`, `chapters`.
* Proposal:

```rust
pub fn updates_by_read_status(conn: &Connection, read: bool, after_ms: i64, limit: i64) -> rusqlite::Result<Vec<MangaUpdate>> {
    conn.prepare_cached(
        "SELECT mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, last_page_read, \
                source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch \
         FROM updatesView WHERE read = ?1 AND dateUpload > ?2 ORDER BY datefetch DESC, chapterId DESC LIMIT ?3")?
        .query_map(params![read, after_ms, limit], manga_update_from_row)?.collect()
}
```

### 5.29 `getUpdatesBySeenStatus`

SQL (`animeupdatesView.sq:31-36`):

```sql
getUpdatesBySeenStatus:
SELECT *
FROM animeupdatesView
WHERE seen = :seen
AND dateUpload > :after

LIMIT :limit;
```

* Anime twin of 5.28 over `animeupdatesView` (16 columns, 5.15). Called by: `AnimeUpdatesRepositoryImpl.kt:15` (`awaitWithSeen`, no live caller) and `:36` (`subscribeWithSeen`) via `GetAnimeUpdates.kt:20` from `presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/AnimeWidgetManager.kt:24` and `BaseAnimeUpdatesGridGlanceWidget.kt:96`. Same P3 decision and proposal with `seen`:

```rust
pub fn updates_by_seen_status(conn: &Connection, seen: bool, after_ms: i64, limit: i64) -> rusqlite::Result<Vec<AnimeUpdate>> {
    conn.prepare_cached(
        "SELECT animeId, animeTitle, episodeId, episodeName, scanlator, seen, bookmark, fillermark, \
                last_second_seen, totalSeconds, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch \
         FROM animeupdatesView WHERE seen = ?1 AND dateUpload > ?2 ORDER BY datefetch DESC, episodeId DESC LIMIT ?3")?
        .query_map(params![seen, after_ms, limit], anime_update_from_row)?.collect()
}
```

### 5.30 `getVisibleCategories`

SQL, identical in both databases (`data/src/main/sqldelight/data/categories.sq:36-45` and `data/src/main/sqldelightanime/dataanime/categories.sq:36-45`):

```sql
getVisibleCategories:
SELECT
_id AS id,
name,
sort AS `order`,
flags,
hidden
FROM categories
WHERE hidden = 0
ORDER BY sort;
```

The unfiltered sibling `getCategories` (`categories.sq:26-34`) has the same text without the `WHERE hidden = 0` line.

* Parameters: none. Returns: `(id, name, order, flags, hidden)` rows mapped by `mapCategory(id, name, order, flags, hidden: Long)` into `Category(id, name, order, flags, hidden = hidden == 1L)` (`MangaCategoryRepositoryImpl.kt:106-120`). The alias `order` is a reserved word, hence the backticks.
* Semantics: categories the user has not hidden, in manual order, INCLUDING the system category `_id = 0` (`name = ""`, `sort = -1`, so it sorts first; its `hidden` is 0 unless the UI toggled it). Ties on `sort` are arbitrary. New user categories receive `sort = max(existing sort) + 1`, so the first user category gets 0 because the system row contributes -1 (`CreateMangaCategoryWithName.kt:22-31`) and are created with `flags` = the current default library sort (`sort.type.flag or sort.direction.flag`, section 3.6) and `hidden = false`.
* Hidden-category behaviour: a category marked hidden (via `HideMangaCategory`, `categoryRepository.updatePartialMangaCategory(CategoryUpdate(id, hidden = !category.hidden))`, `domain/src/main/java/tachiyomi/domain/category/manga/interactor/HideMangaCategory.kt:14-21`) disappears from the library tabs; an entry that belongs only to hidden categories is therefore not shown anywhere in the library. The setting `hideHiddenCategoriesSettings` (preference key `"hidden_categories"`, default false, `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:72`) decides whether the Categories settings screen and the Storage screen list only the visible categories (`MangaCategoryScreenModel.kt:43-47`, `MangaStorageScreenModel.kt:25-31`).
* Library tab logic on top of it: `MangaLibraryScreenModel.kt:393-399` removes the system category from the tab list when the library has entries but none are uncategorised (`!libraryManga.containsKey(0)`).
* Called by: `MangaCategoryRepositoryImpl.kt:23` (`getAllVisibleMangaCategories`, one-shot) and `:31` (`getAllVisibleMangaCategoriesAsFlow`, `subscribeToList`) via `domain/src/main/java/tachiyomi/domain/category/manga/interactor/GetVisibleMangaCategories.kt:11,19` from `MangaLibraryScreenModel.kt:393` (library tabs), `MangaCategoryScreenModel.kt:44`, `MangaStorageScreenModel.kt:29`. Anime twin: `AnimeCategoryRepositoryImpl.kt:23,31`, `GetVisibleAnimeCategories.kt`, `AnimeLibraryScreenModel.kt:409`, `AnimeCategoryScreenModel.kt:44`, `AnimeStorageScreenModel.kt:29`.
* Performance: table has tens of rows at most.
* Watch tables: `categories`.
* Proposal:

```rust
pub struct Category { pub id: i64, pub name: String, pub order: i64, pub flags: i64, pub hidden: bool }
fn category_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Category> {
    Ok(Category { id: r.get(0)?, name: r.get(1)?, order: r.get(2)?, flags: r.get(3)?, hidden: r.get::<_, i64>(4)? == 1 })
}
pub fn visible_categories(conn: &Connection) -> rusqlite::Result<Vec<Category>> {
    conn.prepare_cached("SELECT _id, name, sort, flags, hidden FROM categories WHERE hidden = 0 ORDER BY sort, _id")?
        .query_map([], category_from_row)?.collect()
}
```

### 5.31 `getVisibleCategoriesByAnimeId`

SQL (anime `categories.sq:59-69`):

```sql
getVisibleCategoriesByAnimeId:
SELECT
C._id AS id,
C.name,
C.sort AS `order`,
C.flags,
C.hidden
FROM categories C
JOIN animes_categories AC
ON C._id = AC.category_id
WHERE AC.anime_id = :animeId AND C.hidden = 0;
```

* Parameters: `animeId`. Returns: the visible categories one anime is linked to, as in 5.30 (no ORDER BY, so unordered). The unfiltered sibling is `getCategoriesByAnimeId` (`categories.sq:47-57`, without `AND C.hidden = 0`). An anime with no link rows returns nothing; it implicitly belongs to the system category 0 (the library groups such entries under `category = 0` through `coalesce(MCA.category_id, 0)` in the library view).
* Called by: `AnimeCategoryRepositoryImpl.kt:42` (one-shot) and `:54` (`getVisibleCategoriesByAnimeIdAsFlow`, `subscribeToList`) via `domain/src/main/java/tachiyomi/domain/category/anime/interactor/GetVisibleAnimeCategories.kt:15,23` (`subscribe(animeId)` / `await(animeId)`); the Library screen's selection dialogs use `await` (anime twin of `MangaLibraryScreenModel.kt:432,447,554`).
* OBSERVED hazard (data loss): the Library bulk "edit categories" action reads each selected entry's current categories with the VISIBLE lookup (`MangaLibraryScreenModel.kt:554-558`: `getCategories.await(manga.id)`, where `getCategories` is a `GetVisibleMangaCategories`, `:84`), subtracts and adds the chosen ids, and then `setMangaCategories` deletes all of the entry's links and re-inserts the computed list (`MangaRepositoryImpl.kt:91-98`). Links to hidden categories are not in the computed list, so a bulk edit silently removes the entries from every hidden category. Proposal: the bulk edit must use the unfiltered lookup `getCategoriesByAnimeId` / `getCategoriesByMangaId` for the base set and only use the visible lookup for what the dialog displays.
* Watch tables: `animes_categories`, `categories`.
* Proposal:

```rust
pub fn visible_categories_by_anime_id(conn: &Connection, anime_id: i64) -> rusqlite::Result<Vec<Category>> {
    conn.prepare_cached(
        "SELECT C._id, C.name, C.sort, C.flags, C.hidden FROM categories C \
         JOIN animes_categories AC ON C._id = AC.category_id \
         WHERE AC.anime_id = ?1 AND C.hidden = 0 ORDER BY C.sort, C._id")?
        .query_map(params![anime_id], category_from_row)?.collect()
}
```

### 5.32 `getVisibleCategoriesByMangaId`

SQL (manga `categories.sq:59-69`):

```sql
getVisibleCategoriesByMangaId:
SELECT
C._id AS id,
C.name,
C.sort AS `order`,
C.flags,
C.hidden
FROM categories C
JOIN mangas_categories MC
ON C._id = MC.category_id
WHERE MC.manga_id = :mangaId AND C.hidden = 0;
```

* Manga twin of 5.31 (unfiltered sibling `getCategoriesByMangaId`, `categories.sq:47-57`); same hazard and watch tables `mangas_categories`, `categories`.
* Called by: `MangaCategoryRepositoryImpl.kt:42` (one-shot) and `:54` (`getVisibleCategoriesByMangaIdAsFlow`) via `GetVisibleMangaCategories.kt:15,23`; consumers `MangaLibraryScreenModel.kt:432` (common categories of the selection, intersection of the per-entry sets), `:447` (mixed categories, the difference to the common set shown as "indeterminate" checkboxes), `:554` (bulk edit).
* Proposal:

```rust
pub fn visible_categories_by_manga_id(conn: &Connection, manga_id: i64) -> rusqlite::Result<Vec<Category>> {
    conn.prepare_cached(
        "SELECT C._id, C.name, C.sort, C.flags, C.hidden FROM categories C \
         JOIN mangas_categories MC ON C._id = MC.category_id \
         WHERE MC.manga_id = ?1 AND C.hidden = 0 ORDER BY C.sort, C._id")?
        .query_map(params![manga_id], category_from_row)?.collect()
}
```

### 5.33 `getWatchedAnimeNotInLibrary`

SQL (`animes.sq:92-99`):

```sql
getWatchedAnimeNotInLibrary:
SELECT *
FROM animes
WHERE favorite = 0 AND _id IN (
    SELECT DISTINCT episodes.anime_id
    FROM episodes
    WHERE seen = 1 OR last_second_seen != 0
);
```

* Anime twin of 5.14: non-library anime that have at least one fully seen episode or a saved playback position (`last_second_seen`, seconds). Called by: `AnimeRepositoryImpl.kt:57` (`getWatchedAnimeNotInLibrary`) from `BackupCreator.kt:102`, only when `options.readEntries` (default true); the result is appended after `getAnimeFavorites.await()` (`:106`).
* Performance and Proposal as 5.14:

```rust
pub fn watched_anime_not_in_library(conn: &Connection) -> rusqlite::Result<Vec<Anime>> {
    let sql = format!(
        "SELECT {} FROM animes a WHERE a.favorite = 0 AND EXISTS (\
           SELECT 1 FROM episodes e WHERE e.anime_id = a._id AND (e.seen = 1 OR e.last_second_seen != 0)) \
         ORDER BY a._id", prefixed("a", ANIME_COLS));
    conn.prepare_cached(&sql)?.query_map([], anime_from_row)?.collect()
}
```

### 5.34 `history` (the table and the query of the same name)

The identifier `history` names two things: the table (`data/src/main/sqldelight/data/history.sq:3-10`, columns in section 3.1) and the query `history:` inside `data/src/main/sqldelight/view/historyView.sq:29-46`. Both are documented here together with the statements that live next to the table in `history.sq` and that every caller of the table needs.

#### 5.34.1 The table

```sql
CREATE TABLE history(
    _id INTEGER NOT NULL PRIMARY KEY,
    chapter_id INTEGER NOT NULL UNIQUE,
    last_read INTEGER AS Date,
    time_read INTEGER NOT NULL,
    FOREIGN KEY(chapter_id) REFERENCES chapters (_id)
    ON DELETE CASCADE
);
```

Columns and introducing migrations: section 3.1. Anime twin `animehistory(_id, episode_id UNIQUE, last_seen)`, section 3.2. Semantics in one paragraph: a row exists per chapter that was ever opened in the reader outside incognito mode (`ReaderViewModel.kt:594-596`: `if (incognitoMode) return`). `last_read` is the time of the last save, `time_read` accumulates the milliseconds spent in all sessions. "Remove from history" in the UI only sets `last_read = 0` (the row, and its `time_read`, stay), "Remove all for this entry" resets every row of the entry, and "Delete all history" deletes every row. Deleting a chapter or entry removes the row through the foreign-key cascade.

Rust (Proposal), canonical DDL:

```sql
CREATE TABLE history(
    _id INTEGER NOT NULL PRIMARY KEY,
    chapter_id INTEGER NOT NULL UNIQUE,
    last_read INTEGER NOT NULL DEFAULT 0,
    time_read INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(chapter_id) REFERENCES chapters (_id) ON DELETE CASCADE
);
```

#### 5.34.2 The query `history`

```sql
history:
SELECT
id,
mangaId,
chapterId,
title,
thumbnailUrl,
source,
favorite,
cover_last_modified,
chapterNumber,
readAt,
readDuration
FROM historyView
WHERE historyView.readAt > 0
AND maxReadAtChapterId = historyView.chapterId
AND lower(historyView.title) LIKE ('%' || :query || '%')
ORDER BY readAt DESC;
```

Anime twin `animehistory` (`animehistoryView.sq:28-44`):

```sql
animehistory:
SELECT
id,
animeId,
episodeId,
title,
thumbnailUrl,
source,
favorite,
cover_last_modified,
episodeNumber,
seenAt
FROM animehistoryView
WHERE animehistoryView.seenAt > 0
AND maxSeenAtEpisodeId = animehistoryView.episodeId
AND lower(animehistoryView.title) LIKE ('%' || :query || '%')
ORDER BY seenAt DESC;
```

* Parameters: `query` String; the History screen passes `query ?: ""`, and the empty string matches every title (`'%' || '' || '%'` is `'%%'`).
* Returns: 11 columns (anime: 10, no duration) mapped to `MangaHistoryWithRelations` / `AnimeHistoryWithRelations` as in 5.9 / 5.8.
* Semantics, step by step: (1) the view produces one row per history row (joined with its chapter and manga) annotated with, for the same manga, the chapter id that has the largest `last_read` (`maxReadAtChapterId`); (2) the query keeps only the row whose own chapter equals that chapter, i.e. ONE row per manga: its most recently read chapter; (3) rows with `readAt = 0` (reset) are dropped; so after "Remove" on a row the manga falls back to its next most recent chapter row, if any; (4) the title filter; (5) newest first. Ties: when two chapters of one manga share the identical maximum `last_read`, SQLite's bare-column rule picks an arbitrary one of them as `maxReadAtChapterId` (rare, millisecond resolution); with equal `readAt` between different manga the order is arbitrary.
* Search behaviour (OBSERVED): `lower()` and `LIKE` fold only ASCII letters and the query is not lowercased in Kotlin, so ASCII queries match case-insensitively but a non-ASCII query is compared byte-wise against a non-folded title (searching `Ōkami` does not find `ōkami`, searching `привет` does not find `Привет`); `%` and `_` typed by the user act as wildcards.
* Called by: `data/src/main/java/tachiyomi/data/history/manga/MangaHistoryRepositoryImpl.kt:18` (`getMangaHistory`, `subscribeToList`) via `domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetMangaHistory.kt:16` (`subscribe(query)`) from `app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt:74` (re-subscribes on every new search text, `collectLatest`). The screen inserts a date header between rows whose local dates differ (`toHistoryUiModels`, `:93-103`). Anime: `AnimeHistoryRepositoryImpl.kt:18`, `AnimeHistoryScreenModel.kt:74`.
* Performance: the grouped subquery inside the view reads the whole `history` joined to `chapters`, then the outer join multiplies rows by the number of history rows per manga before the filter keeps one: O(rows per manga squared) per manga, harmless for realistic histories but it re-runs on every write to `mangas`, `chapters` or `history` (each page turn that saves progress writes `chapters`). The Rust query below avoids the view and the self-multiplication.
* Watch tables: `mangas`, `chapters`, `history`.
* Proposal (deterministic tie-break, Unicode case folding, no view):

```rust
pub fn history_search(conn: &Connection, query: &str) -> rusqlite::Result<Vec<MangaHistoryWithRelations>> {
    conn.prepare_cached(
        "SELECT h._id, m._id, c._id, m.title, m.thumbnail_url, m.source, m.favorite, m.cover_last_modified, \
                c.chapter_number, h.last_read, h.time_read \
         FROM history h \
         JOIN chapters c ON c._id = h.chapter_id \
         JOIN mangas m ON m._id = c.manga_id \
         WHERE h.last_read > 0 \
           AND (?1 = '' OR instr(unicode_lower(m.title), unicode_lower(?1)) > 0) \
           AND NOT EXISTS ( \
                 SELECT 1 FROM history h2 JOIN chapters c2 ON c2._id = h2.chapter_id \
                 WHERE c2.manga_id = c.manga_id \
                   AND (h2.last_read > h.last_read OR (h2.last_read = h.last_read AND h2._id > h._id))) \
         ORDER BY h.last_read DESC, h._id DESC")?
        .query_map(params![query], manga_history_with_relations_from_row)?.collect()
}
// anime twin: db::anime::history_search over animehistory / episodes / animes with last_seen, no duration column
```

#### 5.34.3 Statements stored beside the table (needed by the History screen, the reader and restore)

```sql
resetHistoryById:
UPDATE history
SET last_read = 0
WHERE _id = :historyId;

resetHistoryByMangaId:
UPDATE history
SET last_read = 0
WHERE _id IN (
    SELECT H._id
    FROM mangas M
    INNER JOIN chapters C
    ON M._id = C.manga_id
    INNER JOIN history H
    ON C._id = H.chapter_id
    WHERE M._id = :mangaId
);

removeAllHistory:
DELETE FROM history;

removeResettedHistory:
DELETE FROM history
WHERE last_read = 0;

upsert:
INSERT INTO history(chapter_id, last_read, time_read)
VALUES (:chapterId, :readAt, :time_read)
ON CONFLICT(chapter_id)
DO UPDATE
SET
    last_read = :readAt,
    time_read = time_read + :time_read
WHERE chapter_id = :chapterId;
```

Anime twins (`animehistory.sq:33-62`): `resetAnimeHistoryById` (`UPDATE animehistory SET last_seen = 0 WHERE _id = :historyId;`), `resetHistoryByAnimeId` (same sub-select over `animes A`, `episodes EP`, `animehistory H` filtered by `A._id = :animeId`), `removeAllHistory` (`DELETE FROM animehistory;`), `removeResettedHistory` (`DELETE FROM animehistory WHERE last_seen = 0;`) and

```sql
upsert:
INSERT INTO animehistory(episode_id, last_seen)
VALUES (:episodeId, :seenAt)
ON CONFLICT(episode_id)
DO UPDATE
SET
    last_seen = :seenAt
WHERE episode_id = :episodeId;
```

* `upsert` semantics: first read of a chapter inserts `(chapter_id, readAt, duration)`; any later save for the same chapter sets `last_read = readAt` and ADDS the session duration to the stored `time_read` (the unqualified `time_read` on the right-hand side is the existing row's value; the `:time_read` parameter is bound twice with the same value). The trailing `WHERE chapter_id = :chapterId` is always true. NULL: none of the parameters is nullable (`MangaHistoryUpdate(chapterId, readAt: Date, sessionReadDuration: Long)`).
* Writers of `upsert`: reader: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:601` inside `updateHistory(readerChapter)` (`:594-603`): `readAt = Date()`, `sessionReadDuration = readAt.time - chapterReadStartTime` or 0 when the timer is not running; afterwards the timer is cleared. The timer starts in `restartReadTimer()` (`:579-581`, `Instant.now().toEpochMilli()`), is flushed (history written) by `flushReadTimer()` (`:583-588`) which the reader calls on `onPause` (`ReaderActivity.kt:259`) and on every chapter load (`ReaderViewModel.kt:346-347` flush then restart), and restarted on `onResume` (`ReaderActivity.kt:269`). So a session is the time between resume/chapter-load and pause/next-chapter-load. Player: `PlayerViewModel.kt:1796` (`saveEpisodeHistory`, called at `:1766`) and `ExternalIntents.kt:443` write `AnimeHistoryUpdate(episodeId, Date())` (no duration). Restore: `MangaRestorer.kt:351` / `AnimeRestorer.kt:391` (merge rule in 5.4 / 5.5). Repository wrappers swallow every exception and log it (`MangaHistoryRepositoryImpl.kt:62-71`), so a failed history write is silent.
* Writers of the resets: `MangaHistoryRepositoryImpl.kt:38` (`resetHistoryById`), `:46` (`resetHistoryByMangaId`), `:54` (`removeAllHistory`, returns false on exception) via `RemoveMangaHistory.kt:14,18,10`; UI: `MangaHistoryScreenModel.kt:121-131` (remove one / remove all of an entry) and `:133-139` (delete all, then event `HistoryCleared`). `removeResettedHistory` is used only by `ClearDatabaseScreen.kt:213` (5.20).
* Proposal:

```rust
pub fn upsert_history(conn: &Connection, chapter_id: i64, read_at_ms: i64, session_ms: i64) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO history(chapter_id, last_read, time_read) VALUES (?1, ?2, ?3) \
         ON CONFLICT(chapter_id) DO UPDATE SET last_read = excluded.last_read, \
                                               time_read = history.time_read + excluded.time_read")?
        .execute(params![chapter_id, read_at_ms, session_ms]).map(|_| ())
}
pub fn reset_history_by_id(conn: &Connection, history_id: i64) -> rusqlite::Result<usize> {
    conn.execute("UPDATE history SET last_read = 0 WHERE _id = ?1", params![history_id])
}
pub fn reset_history_by_manga_id(conn: &Connection, manga_id: i64) -> rusqlite::Result<usize> {
    conn.execute("UPDATE history SET last_read = 0 WHERE chapter_id IN (SELECT _id FROM chapters WHERE manga_id = ?1)", params![manga_id])
}
pub fn delete_all_history(conn: &Connection) -> rusqlite::Result<usize> { conn.execute("DELETE FROM history", []) }
pub fn delete_reset_history(conn: &Connection) -> rusqlite::Result<usize> { conn.execute("DELETE FROM history WHERE last_read = 0", []) }
```

Anime upsert: `INSERT INTO animehistory(episode_id, last_seen) VALUES (?1, ?2) ON CONFLICT(episode_id) DO UPDATE SET last_seen = excluded.last_seen`.

### 5.35 `historyView`

SQL, manga (`historyView.sq:1-27`; created by `14.sqm` in a reduced form, extended by `15.sqm` with `time_read AS readDuration` and by `17.sqm` with `source`, `favorite`, `cover_last_modified`):

```sql
CREATE VIEW historyView AS
SELECT
    history._id AS id,
    mangas._id AS mangaId,
    chapters._id AS chapterId,
    mangas.title,
    mangas.thumbnail_url AS thumbnailUrl,
    mangas.source,
    mangas.favorite,
    mangas.cover_last_modified,
    chapters.chapter_number AS chapterNumber,
    history.last_read AS readAt,
    history.time_read AS readDuration,
    max_last_read.last_read AS maxReadAt,
    max_last_read.chapter_id AS maxReadAtChapterId
FROM mangas
JOIN chapters
ON mangas._id = chapters.manga_id
JOIN history
ON chapters._id = history.chapter_id
JOIN (
    SELECT chapters.manga_id,chapters._id AS chapter_id, MAX(history.last_read) AS last_read
    FROM chapters JOIN history
    ON chapters._id = history.chapter_id
    GROUP BY chapters.manga_id
) AS max_last_read
ON chapters.manga_id = max_last_read.manga_id;
```

Anime twin `animehistoryView` (`animehistoryView.sq:1-26`; last revised by `118.sqm`): the same text with `animes`, `episodes`, `animehistory`, columns `id, animeId, episodeId, title, thumbnailUrl, source, favorite, cover_last_modified, episodeNumber (episodes.episode_number), seenAt (animehistory.last_seen), maxSeenAt, maxSeenAtEpisodeId`, and the subquery `SELECT episodes.anime_id,episodes._id AS episode_id, MAX(animehistory.last_seen) AS last_seen FROM episodes JOIN animehistory ON episodes._id = animehistory.episode_id GROUP BY episodes.anime_id`.

* Columns of the view: 13 (anime: 12). `readDuration` is the row's own accumulated `time_read`, not the manga total. `maxReadAt` / `maxReadAtChapterId` come from the grouped subquery; because the subquery selects a bare column next to a single `MAX()` aggregate, SQLite returns the chapter id from the row that holds the maximum (documented SQLite behaviour for exactly one min/max aggregate).
* Joins: inner joins everywhere, so a history row whose chapter or manga is missing cannot occur (foreign keys) and a manga without history is absent. The extra join to the subquery repeats the manga for each of its history rows (see 5.34.2 for the performance consequence).
* Consumers: the queries `history` (5.34.2) and `getLatestHistory` (5.9); anime: `animehistory` and `getLatestAnimeHistory`. Nothing else reads the views.
* Trigger side effects: none (views).
* Proposal: do not create `historyView`. Its two consumers are rewritten against the base tables (5.9, 5.34.2), which is both faster and deterministic. If a later feature needs the annotated rows, create it as `CREATE VIEW historyView AS` with the text above; nothing in the Rust schema depends on its absence. What the user loses: nothing.

### 5.36 `history_history_chapter_id_index`

```sql
CREATE INDEX history_history_chapter_id_index ON history(chapter_id);
```

(`history.sq:12`.) History: created by `2.sqm` on the old column name `history_chapter_id`, dropped and re-created by `14.sqm` and by `15.sqm` after the rename. The anime twin is `animehistory_history_episode_id_index ON animehistory(episode_id)` (`animehistory.sq:11`, `114.sqm`, `115.sqm`).

* Purpose: accelerate joins from `history` to `chapters` and look-ups by `chapter_id`.
* OBSERVED redundancy: `chapter_id` is declared `UNIQUE`, and SQLite already builds an automatic unique index (`sqlite_autoindex_history_1`) for it that serves the same lookups; this explicit index is a second b-tree over the same column and adds write cost to every `upsert` for no read benefit.
* Used by: the join in 5.3 to 5.6, 5.9, 5.34 and the `ON CONFLICT(chapter_id)` target of `upsert`.
* Proposal: do not create it. The `UNIQUE` constraint provides the index. Lookups in the reverse direction (chapter list to history) also use it. What the user loses: nothing; the database file is a few kilobytes smaller.

### 5.37 `insert`

`insert:` is the conventional name of the plain INSERT statement in fourteen `.sq` files; they have nothing in common except the name. All fourteen are listed. The Kotlin wrappers run each inside the repository's own transaction handling noted per statement. `_id` is never bound (SQLite assigns the next rowid) except for the seed rows listed in section 3.

| # | Statement | File:line | Database | Table |
|---|---|---|---|---|
| 1 | `INSERT INTO mangas` (21 columns) | `mangas.sq:152` | manga | `mangas` |
| 2 | `INSERT INTO chapters` (14 columns) | `chapters.sq:99` | manga | `chapters` |
| 3 | `INSERT INTO categories` (3 columns) | `categories.sq:71` (manga) | manga | `categories` |
| 4 | `INSERT INTO mangas_categories` (2 columns) | `mangas_categories.sq:18` | manga | `mangas_categories` |
| 5 | `INSERT INTO excluded_scanlators` (2 columns) | `excluded_scanlators.sq:10` | manga | `excluded_scanlators` |
| 6 | `INSERT INTO extension_repos` (5 columns) | `extension_repos.sq:27` | manga | `extension_repos` |
| 7 | `INSERT INTO manga_sync` (13 columns) | `manga_sync.sq:41` | manga | `manga_sync` |
| 8 | `INSERT INTO animes` (29 columns) | `animes.sq:166` | anime | `animes` |
| 9 | `INSERT INTO episodes` (19 columns) | `episodes.sq:94` | anime | `episodes` |
| 10 | `INSERT INTO categories` (3 columns) | `categories.sq:71` (anime) | anime | `categories` |
| 11 | `INSERT INTO animes_categories` (2 columns) | `animes_categories.sq:18` | anime | `animes_categories` |
| 12 | `INSERT INTO anime_sync` (13 columns) | `anime_sync.sq:42` | anime | `anime_sync` |
| 13 | `INSERT OR REPLACE INTO anime_relations` (5 columns) | `anime_relations.sq:31` | anime | `anime_relations` |
| 14 | `INSERT INTO custom_buttons` (6 columns) | `custom_buttons.sq:37` | anime | `custom_buttons` |

#### 5.37.1 `mangas.insert`

```sql
insert:
INSERT INTO mangas(source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, chapter_flags, cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, version)
VALUES (:source, :url, :artist, :author, :description, :genre, :title, :status, :thumbnailUrl, :favorite, :lastUpdate, :nextUpdate, :initialized, :viewerFlags, :chapterFlags, :coverLastModified, :dateAdded, :updateStrategy, :calculateInterval, 0, :version);
```

* Parameters: 20 bound values (`viewerFlags` fills column `viewer`); `last_modified_at` is the literal `0`; `favorite_modified_at` is not listed (NULL), `is_syncing` takes its default 0. `genre` is passed through `StringListColumnAdapter` (`", "` join; a null list stays NULL); `update_strategy` is the ordinal.
* Trigger effects: none (the three `mangas` triggers are `AFTER UPDATE`), so a fresh row has `last_modified_at = 0`, `favorite_modified_at = NULL`, `version` = the bound value (0 for new rows).
* Called by: `MangaRepositoryImpl.kt:102` (`insertManga`) in `awaitOneOrNullExecutable(inTransaction = true)`: insert then `SELECT last_insert_rowid()` (`mangas.sq:181`), returning the new id, via `NetworkToLocalManga.insertManga` (`NetworkToLocalManga.kt:32`); `MangaRestorer.kt:243` for backup restore (preserves `dateAdded`, `lastModifiedAt` is again written as 0).
* Rust (Proposal):

```rust
pub fn insert_manga(conn: &Connection, m: &NewManga) -> rusqlite::Result<i64> {
    conn.prepare_cached(
        "INSERT INTO mangas(source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, \
                            last_update, next_update, initialized, viewer, chapter_flags, cover_last_modified, date_added, \
                            update_strategy, calculate_interval, last_modified_at, version) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,0,?20) RETURNING _id")?
        .query_row(params![m.source, m.url, m.artist, m.author, m.description, m.genre.as_deref().map(|g| g.join(", ")),
                           m.title, m.status, m.thumbnail_url, m.favorite, m.last_update, m.next_update, m.initialized,
                           m.viewer_flags, m.chapter_flags, m.cover_last_modified, m.date_added,
                           m.update_strategy.ordinal(), m.fetch_interval, m.version], |r| r.get(0))
}
```

#### 5.37.2 `chapters.insert`

```sql
insert:
INSERT INTO chapters(manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order,date_fetch, date_upload, last_modified_at, version, is_syncing)
VALUES (:mangaId, :url, :name, :scanlator, :read, :bookmark, :lastPageRead, :chapterNumber, :sourceOrder, :dateFetch, :dateUpload, 0, :version, 0);
```

* Parameters: 12 bound values; `last_modified_at` and `is_syncing` are literals 0. `scanlator` is nullable. No uniqueness on `(manga_id, url)`, so duplicate chapter URLs are not rejected by the database; the chapter sync algorithm prevents them (area:07-domain-entries-items).
* Called by: `data/src/main/java/tachiyomi/data/items/chapter/ChapterRepositoryImpl.kt:20` (`addAllChapters`): one transaction, one `insert` per chapter followed by `selectLastInsertedRowId()` to copy the new id into the model (`:34-35`); any exception rolls the whole batch back and the method returns an empty list after logging (`:38-41`), i.e. the caller sees "no chapters added" rather than an error. Also `MangaRestorer.kt:195`.
* Rust (Proposal): `insert_chapters(conn: &mut Connection, chapters: &[NewChapter]) -> rusqlite::Result<Vec<i64>>`, one `Transaction`, statement `INSERT INTO chapters(manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload, last_modified_at, version, is_syncing) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,0,?12,0) RETURNING _id`, returning ids in input order. Unlike the Kotlin wrapper it returns the error to the caller.

#### 5.37.3 `categories.insert` (both databases)

```sql
insert:
INSERT INTO categories(name, sort, flags)
VALUES (:name, :order, :flags);
```

* `hidden` takes its default 0. No uniqueness on `name`; duplicate names are prevented by the UI. Called by: `MangaCategoryRepositoryImpl.kt:60` / `AnimeCategoryRepositoryImpl.kt:60` (`insertMangaCategory`, no id returned) from `CreateMangaCategoryWithName.kt:22-35` (sort = `max(order) + 1`, flags = current default library sort); restore: `MangaCategoriesRestorer.kt:29` / `AnimeCategoriesRestorer.kt:29` (`categoriesQueries.insert(it.name, order, it.flags)`).
* Rust: `pub fn insert_category(conn: &Connection, name: &str, order: i64, flags: i64) -> rusqlite::Result<i64>` with `INSERT INTO categories(name, sort, flags) VALUES (?1,?2,?3) RETURNING _id`.

#### 5.37.4 `mangas_categories.insert` and `animes_categories.insert`

```sql
insert:
INSERT INTO mangas_categories(manga_id, category_id)
VALUES (:mangaId, :categoryId);
```

```sql
insert:
INSERT INTO animes_categories(anime_id, category_id)
VALUES (:animeId, :categoryId);
```

* Trigger effects: `AFTER INSERT` fires `insert_manga_category_update_version` / `insert_anime_category_update_version` (5.38). Category ids of 0 are legal (the system category) but the UI never inserts link rows to it.
* Called by: `MangaRepositoryImpl.kt:95` and `AnimeRepositoryImpl.kt:99` inside `setMangaCategories` / `setAnimeCategories` (`inTransaction = true`: `DELETE FROM mangas_categories WHERE manga_id = :mangaId` then one insert per id, `:91-98`); restore at `MangaRestorer.kt:315` and `AnimeRestorer.kt:356`.
* Rust: `pub fn set_manga_categories(conn: &mut Connection, manga_id: i64, category_ids: &[i64]) -> rusqlite::Result<()>`: transaction; `DELETE FROM mangas_categories WHERE manga_id = ?1`; for each id `INSERT INTO mangas_categories(manga_id, category_id) VALUES (?1, ?2)`. (Anime: `set_anime_categories` with `animes_categories`.)

#### 5.37.5 `excluded_scanlators.insert`

```sql
insert:
INSERT INTO excluded_scanlators(manga_id, scanlator)
VALUES (:mangaId, :scanlator);
```

* The table has no primary key or unique constraint, so the database accepts duplicates; the writers diff against the stored set first. Called by: `app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetExcludedScanlators.kt:16` (in a transaction: read current set, insert `excluded - current`, then `DELETE FROM excluded_scanlators WHERE manga_id = :mangaId AND scanlator IN :scanlators` for `current - excluded`, `excluded_scanlators.sq:14-17`); restore `MangaRestorer.kt:432` inserts those not already present.
* Rust: `pub fn set_excluded_scanlators(conn: &mut Connection, manga_id: i64, wanted: &HashSet<String>) -> rusqlite::Result<()>` using the same diff algorithm; Proposal also creates `CREATE UNIQUE INDEX excluded_scanlators_unique ON excluded_scanlators(manga_id, scanlator)` and uses `INSERT OR IGNORE` so concurrent writers cannot duplicate.

#### 5.37.6 `extension_repos.insert`

```sql
insert:
INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint)
VALUES (:base_url, :name, :short_name, :website, :fingerprint);
```

* Constraint failures: `base_url` is the primary key and `signing_key_fingerprint` is UNIQUE, so inserting an existing URL or an existing fingerprint raises `SQLiteException`, which `MangaExtensionRepoRepositoryImpl.kt:46-48` wraps into `SaveExtensionRepoException`. Called by: `MangaExtensionRepoRepositoryImpl.kt:44` (`insertRepo`) from the "add repo" flow, and `MangaExtensionRepoRestorer.kt:28` which pre-checks: same URL with a different fingerprint fails with the message "Already Exists with different signing key fingerprint", a fingerprint already used by another repo fails with `"<name> has the same signing key fingerprint"`, otherwise insert. The anime database no longer has this table (replaced by `extension_store`, `137.sqm`; owned by area:19-extensions-anime and area:20-extension-strategy-redesign).
* Rust: `pub fn insert_extension_repo(conn: &Connection, r: &ExtensionRepo) -> rusqlite::Result<()>`; map `rusqlite::Error::SqliteFailure` with code `ConstraintViolation` to a typed `SaveExtensionRepoError`.

#### 5.37.7 `manga_sync.insert` and `anime_sync.insert`

```sql
insert:
INSERT INTO manga_sync(manga_id,sync_id,remote_id,library_id,title,last_chapter_read,total_chapters,status,score,remote_url,start_date,finish_date,private)
VALUES (:mangaId,:syncId,:remoteId,:libraryId,:title,:lastChapterRead,:totalChapters,:status,:score,:remoteUrl,:startDate,:finishDate,:private);
```

```sql
insert:
INSERT INTO anime_sync(anime_id,sync_id,remote_id,library_id,title,last_episode_seen,total_episodes,status,score,remote_url,start_date,finish_date,private)
VALUES (:animeId,:syncId,:remoteId,:libraryId,:title,:lastEpisodeSeen,:totalEpisodes,:status,:score,:remoteUrl,:startDate,:finishDate,:private);
```

* Conflict behaviour (OBSERVED): the table constraint `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE` turns every insert for an existing `(entry, tracker)` pair into "delete the old row, insert a new one with a NEW `_id`". The track repository uses this insert both for first binding and for every later update of score/status/progress (`insertValues` in `MangaTrackRepositoryImpl.kt:54` and `AnimeTrackRepositoryImpl.kt:54`, one transaction per call). The `id` field of the model is ignored. The restore code path for existing tracks uses the separate `update:` statement to keep the id (`MangaRestorer.kt:396-413`). Consequence: `_id` is unstable, which breaks the delayed-update queues (5.21 and 5.22).
* Called by: `InsertMangaTrack.await/awaitAll` (`domain/src/main/java/tachiyomi/domain/track/manga/interactor/InsertMangaTrack.kt:11-25`, exceptions logged and swallowed) from `TrackChapter.kt` (after pushing progress), `RefreshMangaTracks.kt`, `AddMangaTracks.kt`, the tracker bind dialog, and `MangaRestorer.kt` (new tracks).
* Rust (Proposal): keep a stable id by using an upsert on the unique pair; the schema keeps `UNIQUE (manga_id, sync_id)` without the REPLACE clause.

```sql
INSERT INTO manga_sync(manga_id, sync_id, remote_id, library_id, title, last_chapter_read, total_chapters, status, score, remote_url, start_date, finish_date, private)
VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
ON CONFLICT(manga_id, sync_id) DO UPDATE SET
  remote_id = excluded.remote_id, library_id = excluded.library_id, title = excluded.title,
  last_chapter_read = excluded.last_chapter_read, total_chapters = excluded.total_chapters, status = excluded.status,
  score = excluded.score, remote_url = excluded.remote_url, start_date = excluded.start_date,
  finish_date = excluded.finish_date, private = excluded.private
RETURNING _id;
```

Signature `pub fn upsert_track(conn: &Connection, t: &MangaTrack) -> rusqlite::Result<i64>`; anime twin `upsert_anime_track` with `anime_id`, `last_episode_seen`, `total_episodes`.

#### 5.37.8 `animes.insert`

```sql
insert:
INSERT INTO animes(source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, episode_flags, cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, version, fetch_type, parent_id, season_flags, season_number, season_source_order, background_url, background_last_modified, memo)
VALUES (:source, :url, :artist, :author, :description, :genre, :title, :status, :thumbnailUrl, :favorite, :lastUpdate, :nextUpdate, :initialized, :viewerFlags, :episodeFlags, :coverLastModified,:dateAdded, :updateStrategy, :calculateInterval, 0, :version, :fetchType, :parentId, :seasonFlags, :seasonNumber, :seasonSourceOrder, :backgroundUrl, :backgroundLastModified, :memo);
```

* Same rules as 5.37.1 plus: `fetch_type` ordinal (0 Seasons, 1 Episodes), `parent_id` nullable, `memo` encoded as UTF-8 JSON bytes (`MemoColumnAdapter.encode`). Called by: `AnimeRepositoryImpl.kt:106` (`insertAnime`, `awaitOneOrNullExecutable(inTransaction = true)` plus `selectLastInsertedRowId()`, `animes.sq:203`) via `NetworkToLocalAnime`; restore `AnimeRestorer.kt:278`.
* Rust: `pub fn insert_anime(conn: &Connection, a: &NewAnime) -> rusqlite::Result<i64>` with the same statement shape (`RETURNING _id`, `memo` bound as `serde_json::to_vec(&a.memo)?`).

#### 5.37.9 `episodes.insert`

```sql
insert:
INSERT INTO episodes(anime_id, url, name, scanlator, seen, bookmark, last_second_seen, total_seconds, episode_number, source_order, date_fetch, date_upload, last_modified_at, version, is_syncing, summary, preview_url, fillermark, memo)
VALUES (:animeId, :url, :name, :scanlator, :seen, :bookmark, :lastSecondSeen, :totalSeconds, :episodeNumber, :sourceOrder, :dateFetch, :dateUpload, 0, :version, 0, :summary, :previewUrl, :fillermark, :memo);
```

* 17 bound values, `last_modified_at` and `is_syncing` literal 0; `summary` and `preview_url` nullable; `memo` JSON bytes. Called by: `data/src/main/java/tachiyomi/data/items/episode/EpisodeRepositoryImpl.kt:21` (`addAllEpisodes`, the same transaction/rollback/empty-list behaviour as chapters) and `AnimeRestorer.kt:220`.
* Rust: `pub fn insert_episodes(conn: &mut Connection, eps: &[NewEpisode]) -> rusqlite::Result<Vec<i64>>`, statement as above with `RETURNING _id`.

#### 5.37.10 `anime_relations.insert`

```sql
insert:
INSERT OR REPLACE INTO anime_relations(anime_id, related_anime_id, name, sort_order, last_fetched_at)
VALUES (:animeId, :relatedAnimeId, :name, :sortOrder, :lastFetchedAt);
```

* Conflict rule: a row with the same primary key `(anime_id, related_anime_id, name)` is replaced. Called by: `AnimeRelationRepositoryImpl.kt:27` inside `replaceRelations` (one transaction: `DELETE FROM anime_relations WHERE anime_id = :animeId` (`anime_relations.sq:35-37`), then inserts with `sortOrder` counting up across all groups from 0 and the same `lastFetchedAt` for all rows).
* Rust: `pub fn replace_relations(conn: &mut Connection, anime_id: i64, groups: &[(String, Vec<i64>)], fetched_at_ms: i64) -> rusqlite::Result<()>`: transaction; the delete; for each group and id `INSERT OR REPLACE INTO anime_relations(anime_id, related_anime_id, name, sort_order, last_fetched_at) VALUES (?1,?2,?3,?4,?5)` with a running `sort_order`.

#### 5.37.11 `custom_buttons.insert`

```sql
insert:
INSERT INTO custom_buttons(name, isFavorite, sortIndex, content, longPressContent, onStartup)
VALUES (:name, :isFavorite, :sortIndex, :content, :longPressContent, :onStartup);
```

* Player custom buttons (Lua snippets run by the mpv layer, area:48-settings-player-and-scripts). No unique constraint on `name`. Called by: `data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt:31` (`insertCustomButton`, always `isFavorite = false`; an `SQLiteException` becomes `SaveCustomButtonException`) and `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/CustomButtonRestorer.kt:30` (restore: buttons are matched by `name` and skipped if present; `isFavorite` is kept only when the database has no favourite button yet; `sortIndex` continues from the current maximum). Seed row 1 is inserted by the schema (`custom_buttons.sq:14-30`, `129.sqm`).
* Rust: `pub fn insert_custom_button(conn: &Connection, b: &NewCustomButton) -> rusqlite::Result<i64>` with `INSERT INTO custom_buttons(name, isFavorite, sortIndex, content, longPressContent, onStartup) VALUES (?1,?2,?3,?4,?5,?6) RETURNING _id`.

### 5.38 `insert_anime_category_update_version`

SQL (`animes_categories.sq:11-16`; created by migration `127.sqm`, manga twin `insert_manga_category_update_version` in `mangas_categories.sq:11-16` and `30.sqm`):

```sql
CREATE TRIGGER insert_anime_category_update_version AFTER INSERT ON animes_categories
BEGIN
    UPDATE animes
    SET version = version + 1
    WHERE _id = new.anime_id AND (SELECT is_syncing FROM animes WHERE _id = new.anime_id) = 0;
END;
```

* Event: every row inserted into `animes_categories` (not updates, not deletes). Effect: the parent anime's `version` goes up by one unless the anime is mid-sync (`is_syncing = 1`). The version counter is the change marker inherited from Mihon's cloud-sync feature (which decides which side of a sync is newer); this clone has no sync package under `app/src/main/java/eu/kanade/tachiyomi/data/`, so the counters are only kept consistent and exported in the backup (`BackupAnime.kt`, field `version`).
* Chained effects: the trigger's own `UPDATE animes` fires `update_last_modified_at_animes` (stamps `last_modified_at` in seconds) and `update_anime_version` (does nothing: it only bumps when `url`, `description` or `favorite` differ, and none do), but not `update_last_favorite_at_animes` (that trigger fires only for statements that name `favorite` in their SET list). `recursive_triggers` is OFF, so the version trigger does not re-enter itself.
* Behavioural consequences observed from callers: assigning an anime to `k` categories (`setAnimeCategories`, 5.37.4) bumps `version` by `k`, assigning to none bumps it by zero even though the previous links were deleted, and the delete side has no trigger at all.
* Performance: one indexed update per inserted link.
* Proposal: keep the trigger (needed so `version` in exported backups behaves like Aniyomi's); simplified body, same semantics:

```sql
CREATE TRIGGER insert_anime_category_update_version AFTER INSERT ON animes_categories
BEGIN
    UPDATE animes SET version = version + 1 WHERE _id = new.anime_id AND is_syncing = 0;
END;
```

Rust test: insert two link rows, read `version` before and after, expect `+2`; set `is_syncing = 1` first and expect `+0`.

## 6. Behaviours the Rust port must not copy (summary of OBSERVED hazards)

| Hazard | Where | Rust rule |
|---|---|---|
| Restore looks up history by chapter/episode URL only; `executeAsOneOrNull` throws when two entries share a URL | 5.4, 5.5 | scope the lookup by entry id (`history_by_chapter_url(conn, manga_id, url)`) |
| A reset history row (`last_read = 0`) is included by `getHistoryByMangaId`/`ByAnimeId`, so the tracker start date can become 1970-01-01 | 5.3, 5.6 | drop rows with `last_read <= 0` before taking the minimum |
| `lower()`/`LIKE` fold ASCII only and treat `%`/`_` as wildcards | 5.34.2 | `unicode_lower` + `instr` |
| `update:` statements name `favorite` every time, so `favorite_modified_at` is refreshed by every update | 1.5, 5.12 | keep as is only if backup fidelity matters; do not build features on `favorite_modified_at` |
| `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE` on track inserts changes `_id` on every save | 5.21, 5.22, 5.37.7 | `ON CONFLICT DO UPDATE` upsert |
| Bulk "edit categories" reads only visible categories then rewrites all links, dropping hidden-category links | 5.31, 5.32 | read the unfiltered set for the base |
| Views rely on `ORDER BY` inside the view | 5.15, 5.16 | `ORDER BY` in the outer query |
| `addAllChapters` / `addAllEpisodes` swallow errors and return an empty list | 5.37.2, 5.37.9 | return the `Result` |
| `getMangaByUrlAndSource` has no tie-break although `(source, url)` is not unique | 5.11 | `ORDER BY _id LIMIT 1` and the composite index |
| Anime relations with no results are re-fetched on every screen open | 5.7 | optional: store a sentinel row or a `relations_fetched_at` column on `animes` (P2) |

## 7. Test checklist for the database layer (acceptance for the tasks that implement this chunk)

1. `foreign_keys` is ON: deleting a chapter removes its `history` row; deleting a manga removes its chapters, history, `manga_sync` and `mangas_categories` rows.
2. `PRAGMA recursive_triggers` reads 0 and an `UPDATE mangas SET title = 'x' WHERE _id = 1` terminates, sets `last_modified_at` to a value within 2 seconds of `strftime('%s','now')` and leaves `version` unchanged.
3. Updating `mangas.url` bumps `version` by exactly 1; changing `description` from NULL to text does not bump it; changing it from text to other text bumps it by 1.
4. `upsert_history` called twice for one chapter with durations 1000 and 2500 leaves one row with `time_read = 3500` and `last_read` equal to the second call's timestamp; `total_read_duration_ms` returns 3500; `reset_history_by_id` keeps 3500 in the total; `delete_all_history` returns it to 0.
5. `history_search("")` returns one row per manga (the latest chapter), newest first; after `reset_history_by_id` of that row the manga falls back to its previous chapter row; searching `"ōkami"` finds a title `"Ōkami"`.
6. `latest_history` returns the globally newest non-reset row and `None` when every row is reset.
7. `upcoming_manga(start)` returns only favourites with `next_update >= start` and status 1 or 4, ordered ascending; a row with `next_update = NULL` is absent; `upcoming_anime` also returns status 7.
8. `recent_updates(after, 500)` returns only chapters with `date_fetch > date_added` of favourites with `date_upload > after`, newest `date_fetch` first, at most 500 rows.
9. `upsert_track` twice for the same `(manga_id, sync_id)` keeps the same `_id`; `get_track_by_id` finds it afterwards.
10. `visible_categories` excludes a category after `hidden = 1`, includes the system category 0 first, and deleting category 0 fails with "System category can't be deleted".
11. `read_manga_not_in_library` returns a non-favourite manga with one chapter marked read, and does not return one whose chapters all have `read = 0` and `last_page_read = 0`.
12. `insert_anime_category_update_version`: two link inserts raise `animes.version` by 2; with `is_syncing = 1` they raise it by 0.
13. `replace_relations` followed by `related_anime` returns the groups in input order with `sort_order` running across groups, and `relations_last_fetched_at` returns the stamp; after 7 days (604800000 ms) the caller treats it as stale.
14. `clear_non_library_by_source(&[s])` deletes only non-favourite rows of source `s` and also purges reset history rows of other entries.
