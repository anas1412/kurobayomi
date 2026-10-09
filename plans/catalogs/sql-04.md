# SQL catalog, chunk 4 of 4

Source of the item list: `python3 plans/_tools/inventory.py items sql 4 4` (38 items). First item: `insert_manga_category_update_version`. Last item: `upsert`. Every path is repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`). Cross-references use `area:<id>`; the schema/migration design belongs to area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations, the Kotlin repositories that run these statements to area:05-data-repos-entries-items and area:06-data-repos-history-track-source-misc. This catalog supplies the per-statement facts.

The 38 items are names, not single statements. SqlDelight lets many files reuse a statement label, so several items below cover up to nine statements in different files (`update` = 9 statements, `upsert` = 6, `selectLastInsertedRowId` = 7, `resetIsSyncing` = 4, `removeAllHistory` = 2, `removeResettedHistory` = 2, `resetViewerFlags` = 2, `updateAllFlags` = 2, `system_category_delete_trigger` = 2). Every variant is written out. Items by kind:

| Kind | Items |
|---|---|
| Table | `mangas`, `mangas_categories`, `manga_sync`, `sources` |
| View | `libraryView` |
| Index | `library_favorite_index`, `mangas_url_index` |
| Trigger | `insert_manga_category_update_version`, `system_category_delete_trigger`, `update_anime_version`, `update_chapter_and_manga_version`, `update_episode_and_anime_version`, `update_last_favorite_at_animes`, `update_last_favorite_at_mangas`, `update_last_modified_at_animes`, `update_last_modified_at_chapters`, `update_last_modified_at_episodes`, `update_last_modified_at_mangas`, `update_manga_version` |
| Named query | `library`, `remove`, `removeAllHistory`, `removeChaptersWithIds`, `removeEpisodesWithIds`, `removeParentIdByIds`, `removeResettedHistory`, `replace`, `resetAnimeHistoryById`, `resetHistoryByAnimeId`, `resetHistoryById`, `resetHistoryByMangaId`, `resetIsSyncing`, `resetViewerFlags`, `selectLastInsertedRowId`, `update`, `updateAllFlags`, `upsert` |
| Query defined in a view file | `updatesView` is the view; its two queries are `getRecentUpdates` and `getUpdatesByReadStatus` (described under `updatesView`) |

## 0. Facts shared by every statement (read this first)

### 0.1 Two database files, opened with three pragmas (OBSERVED)

`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:78-125` builds two `AndroidSqliteDriver`s:

| Database | File name | SqlDelight class | Statement sources | Migration files |
|---|---|---|---|---|
| manga | `tachiyomi.db` (`AppModule.kt:81`) | `tachiyomi.data.Database` | `data/src/main/sqldelight/data/*.sq`, `data/src/main/sqldelight/view/*.sq` | `data/src/main/sqldelight/migrations/1.sqm` .. `32.sqm` |
| anime | `tachiyomi.animedb` (`AppModule.kt:106`) | `tachiyomi.mi.data.AnimeDatabase` | `data/src/main/sqldelightanime/dataanime/*.sq`, `data/src/main/sqldelightanime/view/*.sq` | `data/src/main/sqldelightanime/migrations/113.sqm` .. `138.sqm` |

(`data/build.gradle.kts:15-30` declares both databases.) Both are opened with a `Callback.onOpen` that runs exactly:

```
PRAGMA foreign_keys = ON
PRAGMA journal_mode = WAL
PRAGMA synchronous = NORMAL
```

Nothing sets `recursive_triggers`, `busy_timeout`, `temp_store`, or `cache_size`. `recursive_triggers` therefore stays at the SQLite default OFF, which matters for the triggers below (a trigger whose action updates its own table does not re-fire itself; see 0.5). The two databases share table names (`categories`, `extension_repos` before anime migration 137) but never a connection, so there are no cross-database joins anywhere in the SQL: a manga's `category_id` and an anime's `category_id` point into different `categories` tables.

SqlDelight schema version numbers (used as `PRAGMA user_version` by the Android driver): manga DB = 33 (32 migration files + 1), anime DB = 139 (migration files 113..138; the first anime schema is version 113). A brand-new install executes the `.sq` files directly (final shape), not the migrations. An upgrade runs `N.sqm` for every N from the stored version up to the latest.

### 0.2 Units of time and other value conventions (OBSERVED)

| Column(s) | Unit | Source of the value |
|---|---|---|
| `mangas.last_update`, `next_update`, `date_added`, `cover_last_modified`; `chapters.date_fetch`, `date_upload`; `history.last_read`; anime equivalents (`animehistory.last_seen`, `episodes.date_fetch`, `episodes.date_upload`, `animes.last_update`, `animes.next_update`, `animes.date_added`, `animes.cover_last_modified`, `animes.background_last_modified`) | Unix epoch MILLISECONDS (`Instant.now().toEpochMilli()`, `Date.time`) | `UpdateManga.kt:90-104` (`awaitUpdateLastUpdate`, `awaitUpdateCoverLastModified`, `awaitUpdateFavorite`), `ReaderViewModel.kt:597-602` |
| `history.time_read` | MILLISECONDS of reading time, summed over sessions | `ReaderViewModel.kt:599` `readAt.time - chapterReadStartTime` |
| `mangas.last_modified_at`, `mangas.favorite_modified_at`, `chapters.last_modified_at`, `episodes.last_modified_at` and anime equivalents | Unix epoch SECONDS (written only by triggers with `strftime('%s', 'now')`) | triggers below |
| `mangas.date_added` | `0` when the manga is not a favorite; set to now (ms) when favorited; reset to `0` when un-favorited | `UpdateManga.kt:97-105` |
| `mangas.next_update` | millis of the next planned library check; `0`/NULL = none | `MangaFetchInterval.kt` (`calculateNextUpdate`) |
| `mangas.calculate_interval` | days. `< 0` = user fixed interval of `abs(value)` days; `1..28` = computed (`MAX_INTERVAL = 28`); `0` = not computed yet | `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt:19-25,98-121` |
| `genre` | TEXT, items joined with the two-character separator `", "` (comma+space); NULL = unknown; empty string decodes to empty list | `data/src/main/java/tachiyomi/data/DatabaseAdapter.kt:21-32` |
| Boolean columns (`favorite`, `initialized`, `read`, `bookmark`, `seen`, `fillermark`, `private`, `hidden`, `isFavorite`, `is_legacy`) | INTEGER 0/1 | SqlDelight `AS Boolean` |
| `update_strategy` | INTEGER = enum ordinal. Manga: `0 ALWAYS_UPDATE`, `1 ONLY_FETCH_ONCE`; an unknown ordinal decodes to `ALWAYS_UPDATE` (`DatabaseAdapter.kt:34-39`) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt` |
| anime `update_strategy` | same two ordinals (`AnimeUpdateStrategy`, `DatabaseAdapter.kt:41-46`) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt` |
| anime `fetch_type` | enum ordinal: `0 Seasons`, `1 Episodes` (`FetchType`); unknown decodes to `Episodes` (`DatabaseAdapter.kt:48-53`) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt` |
| `status` | `SManga` constants: `UNKNOWN 0`, `ONGOING 1`, `COMPLETED 2`, `LICENSED 3`, `PUBLISHING_FINISHED 4`, `CANCELLED 5`, `ON_HIATUS 6` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:48-54` |
| `viewer` (manga reader flags) | bit field: reading mode in mask `0x7` (`DEFAULT 0, LEFT_TO_RIGHT 1, RIGHT_TO_LEFT 2, VERTICAL 3, WEBTOON 4, CONTINUOUS_VERTICAL 5`), orientation in mask `0x38` (`DEFAULT 0x0, FREE 0x8, PORTRAIT 0x10, LANDSCAPE 0x18, LOCKED_PORTRAIT 0x20, LOCKED_LANDSCAPE 0x28, REVERSE_PORTRAIT 0x30`) | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt:21-60`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt:20-65`, `app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:18-21` |
| `chapter_flags` | bit field, see the table under item `mangas` | `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:75-110` |
| `memo` (anime + episode) | BLOB holding UTF-8 JSON object text, empty object = the 2 bytes `{}` = `x'7b7d'` | `DatabaseAdapter.kt:55-63` |

### 0.3 Fresh install vs upgraded database (OBSERVED differences)

A fresh install is built from the `.sq` files; an upgraded one from the baseline plus migrations. They are NOT byte-identical. Kurobayomi creates only the fresh shape, but anything that reads a copied Aniyomi database or compares schemas must know these differences:

| Object | Fresh (`.sq`) | Upgraded (`.sqm`) |
|---|---|---|
| `history.last_read` | nullable (`INTEGER AS Date`, `history.sq:6`) | `NOT NULL` (`15.sqm`) |
| `animehistory.last_seen` | nullable (`animehistory.sq:5`) | `NOT NULL` (`115.sqm`) |
| manga favorite trigger name | `update_last_favorite_at_mangas` (`mangas.sq:36`) | `update_last_favorited_at_mangas` (`26.sqm`) |
| anime favorite trigger name | `update_last_favorite_at_animes` (`animes.sq:48`) | `update_last_favorited_at_animes` (`126.sqm`) |
| `mangas_categories.last_modified_at` column and trigger `update_last_modified_at_mangas_categories` | absent (`mangas_categories.sq:1-9`) | present (`26.sqm`), never dropped |
| `animes_categories.last_modified_at` and trigger `update_last_modified_at_animes_categories` | absent | present (`126.sqm`) |
| anime category delete trigger name | `system_category_delete_trigger` (`dataanime/categories.sq:12`) | `system_anime_category_delete_trigger` (`120.sqm`) |
| anime `season_flags`, `season_number`, `season_source_order`, `background_last_modified` | `NOT NULL` without DEFAULT (`animes.sq:30-35`) | `NOT NULL DEFAULT 0 / -1.0 / 0 / 0` (`131.sqm`, `134.sqm`) |
| anime `memo` default | `DEFAULT '{}'` (TEXT) | `DEFAULT x'7b7d'` (BLOB) (`136.sqm`) |
| `manga_sync.score`, `start_date`, `finish_date` | plain `REAL` / `INTEGER` | `AS Float` / `AS Long` annotations only, same storage |

Consequence for the Rust reader of any row: read `memo` accepting both TEXT and BLOB values, treat a NULL `history.last_read` as "no read time", and never rely on a trigger name.

### 0.4 SQLite behaviours these statements rely on (OBSERVED + verified against SQLite docs)

1. `INTEGER NOT NULL PRIMARY KEY` is the rowid alias and there is NO `AUTOINCREMENT` anywhere. New ids are `max(rowid)+1`. After deleting the highest row, its id is reused by the next insert. Nothing in Aniyomi assumes ids are never reused.
2. `AFTER UPDATE OF favorite ON mangas` fires whenever `favorite` appears in the `SET` list, even when the value does not change. `mangas.update` always lists `favorite = coalesce(:favorite, favorite)`, so the trigger fires on EVERY update made through that statement (see `update_last_favorite_at_mangas`).
3. `coalesce(:x, col)` in an `UPDATE` means "bind NULL to leave the column alone". A column therefore can never be set back to NULL through these statements (for example `thumbnail_url`, `description`).
4. `x != y` is NULL (not true) when either side is NULL. The version triggers compare `new.description != old.description`, so a change from or to NULL does not bump `version`.
5. `recursive_triggers` OFF: a trigger program does not re-enter itself. A different trigger fired by a trigger's own `UPDATE` does run. Net effect of one user `UPDATE mangas` is derived in 0.5.
6. `strftime('%s', 'now')` returns TEXT (`'1760000000'`). Assigning it to an INTEGER-affinity column stores an integer (seconds).
7. `ON CONFLICT(col) DO UPDATE SET <assignments> WHERE <cond>`: the `WHERE` is evaluated against the existing row. The conditions written in these statements (`WHERE _id = :id`, `WHERE chapter_id = :chapterId`, `WHERE base_url = base_url`) are tautologies given the conflict target.
8. `IN :list` in SqlDelight expands to `IN (?,?,?)` with one `?` per element (three elements shown) and NO chunking. An empty list becomes `IN ()`, which SQLite accepts and evaluates to false.
9. SqlDelight binds parameters positionally in order of first appearance in the SQL text. A `:name` used twice is bound once per occurrence from the same Kotlin argument.
10. `strftime`, `last_insert_rowid()`, `coalesce`, `lower`, `LIKE` are the only functions used. In stock SQLite `LIKE` is case-insensitive for ASCII only and `lower()` lowercases ASCII only (the rusqlite bundled build has no ICU). Kotlin lowercases with `String.lowercase()` (full Unicode) before binding, so for non-ASCII titles the SQL comparison can disagree with Kotlin's expectation (see the `getDuplicateLibraryManga` note under item `mangas`). Whether Aniyomi's `RequerySQLiteOpenHelperFactory` build includes ICU was not verified from the sources.

### 0.5 What one `UPDATE mangas` does after the triggers (derived, OBSERVED statements)

Three `AFTER UPDATE` triggers exist on `mangas`: `update_last_favorite_at_mangas`, `update_last_modified_at_mangas`, `update_manga_version`. For a user statement that updates a row, with `recursive_triggers` OFF:

1. `update_last_modified_at_mangas` always runs: `last_modified_at = now (s)`.
2. `update_last_favorite_at_mangas` runs iff `favorite` is in the SET list. It sets `favorite_modified_at = now (s)`. Its own `UPDATE` re-fires `update_last_modified_at_mangas` (not currently running on this path) which stamps `last_modified_at` again.
3. `update_manga_version` runs on every update; its inner `UPDATE mangas SET version = version + 1` executes only if `new.is_syncing = 0` AND (`new.url != old.url` OR `new.description != old.description` OR `new.favorite != old.favorite`), all comparisons NULL-propagating. If it bumps, `update_last_modified_at_mangas` fires again.

The same shape holds for `animes`, `chapters`, `episodes`. Order of trigger firing among the three is not specified by SQLite and does not change the net result.

### 0.6 Proposal: shared Rust plumbing used by every "rusqlite equivalent" below

Crate and module names follow area:03-db-manga-schema-migrations / area:04-db-anime-schema-migrations; here only the pieces referenced by this catalog are fixed.

* Two files `manga.db` and `anime.db` under `Paths::data_dir()` (area:01-foundation-app-bootstrap), same table shapes as Aniyomi so that a future importer of a copied Android `tachiyomi.db` only has to run the remaining `.sqm` steps. Both set `PRAGMA user_version = 33` / `139`.
* On every connection open: `PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 5000;` and do NOT enable `recursive_triggers`. (rusqlite's bundled SQLite compiles with `SQLITE_DEFAULT_FOREIGN_KEYS=1`, the explicit pragma is kept anyway.)
* `Db` = one `rusqlite::Connection` for writes behind a `std::sync::Mutex`, a small read pool (`r2d2_sqlite`, 4 connections), accessed from async code through `tokio::task::spawn_blocking`. A transaction is `conn.transaction()`; Kotlin's `handler.await(inTransaction = true)` maps to one `Transaction` that the callee receives as `&Transaction` (which derefs to `&Connection`).
* All SQL is held as `const &str` with named parameters (`:name`) copied from the `.sq` text; statements are run via `conn.prepare_cached(SQL)?` with `rusqlite::named_params!{}`. Optional parameters are `Option<T>` (None binds NULL, which `coalesce` then ignores), reproducing the SqlDelight partial-update contract exactly.
* Quote rule: Rust DDL uses single-quoted SQL strings only. Aniyomi's `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0)` and `RAISE(ABORT, "System category can't be deleted")` use double quotes, which SQLite accepts only through the legacy double-quoted-string fallback; the Rust text writes `''` and `'System category can''t be deleted'`.
* `IN` lists: helper `const IN_CHUNK: usize = 500; fn in_chunks<T>(ids: &[T]) -> std::slice::Chunks<'_, T>` (= `ids.chunks(IN_CHUNK)`) and a `placeholders(k: usize) -> String` helper (`placeholders(3)` = `"?,?,?"`). Chunking at 500 (Aniyomi does none) keeps every statement well under the 999-variable limit of older SQLite builds and bounds the prepared-statement cache; an empty slice returns early without executing (same observable result as `IN ()`).
* Reactive reads (Kotlin `handler.subscribeToList` = SqlDelight `asFlow()`, re-run when any table the query touches changes): register `Connection::update_hook` (rusqlite feature `hooks`) on the write connection; it publishes `(table_name)` on a `tokio::sync::broadcast` channel; `Db::subscribe(tables: &[&str], query_fn)` re-runs `query_fn` on each matching message, coalescing messages that arrive within 50 ms. Each item below lists its "invalidated by" table set. The hook fires for rows changed by triggers and by `ON DELETE CASCADE`; a unit test in the db crate asserts that (see section 4).
* Row mapping always reads columns BY NAME (`row.get("totalCount")`), never by index, because `M.*` and `A.*` views change width when columns are added.
* Id convention: Rust uses `i64` for every `_id`, source id and epoch value; `f64` for `chapter_number`, `episode_number`, `season_number`, `score`, `last_chapter_read`, `last_episode_seen`.

## 1. Index (all 38 items)

| # | Item | Kind | Defined in |
|---|---|---|---|
| 1 | `insert_manga_category_update_version` | trigger on `mangas_categories` | `data/src/main/sqldelight/data/mangas_categories.sq:11` |
| 2 | `library` | query on `libraryView` | `data/src/main/sqldelight/view/libraryView.sq:35` |
| 3 | `libraryView` | view | `data/src/main/sqldelight/view/libraryView.sq:1` |
| 4 | `library_favorite_index` | partial index on `mangas(favorite)` | `data/src/main/sqldelight/data/mangas.sq:33` |
| 5 | `manga_sync` | table | `data/src/main/sqldelight/data/manga_sync.sq:3` |
| 6 | `mangas` | table | `data/src/main/sqldelight/data/mangas.sq:6` |
| 7 | `mangas_categories` | table | `data/src/main/sqldelight/data/mangas_categories.sq:1` |
| 8 | `mangas_url_index` | index on `mangas(url)` | `data/src/main/sqldelight/data/mangas.sq:34` |
| 9 | `remove` | query on `excluded_scanlators` | `data/src/main/sqldelight/data/excluded_scanlators.sq:14` |
| 10 | `removeAllHistory` | query (2 variants) | `history.sq:54`, `animehistory.sq:51` |
| 11 | `removeChaptersWithIds` | query | `data/src/main/sqldelight/data/chapters.sq:90` |
| 12 | `removeEpisodesWithIds` | query | `data/src/main/sqldelightanime/dataanime/episodes.sq:85` |
| 13 | `removeParentIdByIds` | query | `data/src/main/sqldelightanime/dataanime/animes.sq:152` |
| 14 | `removeResettedHistory` | query (2 variants) | `history.sq:57`, `animehistory.sq:54` |
| 15 | `replace` | query on `extension_repos` | `data/src/main/sqldelight/data/extension_repos.sq:43` |
| 16 | `resetAnimeHistoryById` | query | `animehistory.sq:33` |
| 17 | `resetHistoryByAnimeId` | query | `animehistory.sq:38` |
| 18 | `resetHistoryById` | query | `history.sq:36` |
| 19 | `resetHistoryByMangaId` | query | `history.sq:41` |
| 20 | `resetIsSyncing` | query (4 variants) | `chapters.sq:94`, `mangas.sq:136`, `animes.sq:147`, `episodes.sq:89` |
| 21 | `resetViewerFlags` | query (2 variants) | `mangas.sq:132`, `animes.sq:143` |
| 22 | `selectLastInsertedRowId` | query (7 variants) | `categories.sq:91` (manga), `chapters.sq:120`, `mangas.sq:181`, `animes.sq:203`, `dataanime/categories.sq:91`, `custom_buttons.sq:55`, `episodes.sq:120` |
| 23 | `sources` | table | `data/src/main/sqldelight/data/sources.sq:1` |
| 24 | `system_category_delete_trigger` | trigger (2 variants) | `data/src/main/sqldelight/data/categories.sq:12`, `data/src/main/sqldelightanime/dataanime/categories.sq:12` |
| 25 | `update` | query (9 variants) | `categories.sq:79` (manga), `chapters.sq:103`, `manga_sync.sq:45`, `mangas.sq:156`, `anime_sync.sq:46`, `animes.sq:170`, `dataanime/categories.sq:79`, `custom_buttons.sq:45`, `episodes.sq:98` |
| 26 | `updateAllFlags` | query (2 variants) | `categories.sq:87` (manga), `dataanime/categories.sq:87` |
| 27 | `update_anime_version` | trigger on `animes` | `animes.sq:65` |
| 28 | `update_chapter_and_manga_version` | trigger on `chapters` | `chapters.sq:35` |
| 29 | `update_episode_and_anime_version` | trigger on `episodes` | `episodes.sq:41` |
| 30 | `update_last_favorite_at_animes` | trigger on `animes` | `animes.sq:48` |
| 31 | `update_last_favorite_at_mangas` | trigger on `mangas` | `mangas.sq:36` |
| 32 | `update_last_modified_at_animes` | trigger on `animes` | `animes.sq:56` |
| 33 | `update_last_modified_at_chapters` | trigger on `chapters` | `chapters.sq:26` |
| 34 | `update_last_modified_at_episodes` | trigger on `episodes` | `episodes.sq:32` |
| 35 | `update_last_modified_at_mangas` | trigger on `mangas` | `mangas.sq:44` |
| 36 | `update_manga_version` | trigger on `mangas` | `mangas.sq:53` |
| 37 | `updatesView` | view (+ 2 queries) | `data/src/main/sqldelight/view/updatesView.sq:1,23,29` |
| 38 | `upsert` | query (6 variants) | `sources.sq:16`, `extension_repos.sq:31`, `history.sq:61`, `animesources.sq:16`, `extension_store.sq:27`, `animehistory.sq:58` |

In the `Defined in` column a bare file name is shorthand for the unique file with that name under `data/src/main/sqldelight/data/` (manga DB) or `data/src/main/sqldelightanime/dataanime/` (anime DB), except where a full prefix is written.

## 2. Items

Items appear in the order of the inventory list (ASCII order). Each section has: SQL (trimmed, verbatim), meaning, callers, notes, Proposal (rusqlite).

### 2.1 `insert_manga_category_update_version`

Trigger on the manga-to-category link table. Defined `data/src/main/sqldelight/data/mangas_categories.sq:11-16`; introduced by `data/src/main/sqldelight/migrations/30.sqm` (the identical anime trigger is `insert_anime_category_update_version`, `data/src/main/sqldelightanime/dataanime/animes_categories.sq:11-16`, anime migration `127.sqm`).

```sql
CREATE TRIGGER insert_manga_category_update_version AFTER INSERT ON mangas_categories
BEGIN
    UPDATE mangas
    SET version = version + 1
    WHERE _id = new.manga_id AND (SELECT is_syncing FROM mangas WHERE _id = new.manga_id) = 0;
END;
```

* Fires once per inserted `mangas_categories` row (no `FOR EACH ROW` keyword needed; SQLite triggers are always row-level).
* Effect: `mangas.version` increases by one per inserted link unless the manga is currently flagged `is_syncing = 1`. Putting a manga into three categories therefore bumps the version three times (one per `INSERT`), and `setMangaCategories` first deletes all links (deletion has NO trigger) then inserts, so even re-saving identical categories bumps `version` by the number of categories.
* Side effects of the `UPDATE mangas` it issues: `update_last_modified_at_mangas` fires (`last_modified_at` = now seconds); `update_manga_version` fires but its condition (url/description/favorite changed) is false so no second bump; `update_last_favorite_at_mangas` does not fire (`favorite` not in SET).
* `version` meaning: a per-row counter used by the (SyncYomi) sync feature and by backup restore conflict resolution (`MangaRestorer.kt:96-103`: the side with the larger `version` wins field-by-field in `copyFrom`).
* Called through: `MangaRepositoryImpl.setMangaCategories` (`data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt:91-98`), `MangaRestorer.restoreCategories` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt:313-317`).
* Perf: one indexed point update (`mangas._id`) per link row. Negligible.

Proposal: create the trigger verbatim in the manga DB schema (same name and body), and the anime twin in the anime DB. It costs nothing and keeps `version` semantics so a future sync feature or Aniyomi backup import/export stays consistent. Rust statement for the call site:

```rust
pub const SQL_MC_INSERT: &str =
    "INSERT INTO mangas_categories(manga_id, category_id) VALUES (:mangaId, :categoryId)";
pub const SQL_MC_DELETE_BY_MANGA: &str =
    "DELETE FROM mangas_categories WHERE manga_id = :mangaId";
/// One transaction; trigger bumps `mangas.version` once per inserted id.
pub fn set_manga_categories(tx: &Transaction, manga_id: i64, category_ids: &[i64]) -> rusqlite::Result<()> { /* delete, then insert each */ }
```

### 2.2 `library`

Named query over `libraryView`. `data/src/main/sqldelight/view/libraryView.sq:35-37`.

```sql
library:
SELECT *
FROM libraryView;
```

* No parameters, no `ORDER BY`. Returns one row per (favorite manga, category link). Column order: all 24 `mangas` columns in table order (`_id, source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, chapter_flags, cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, favorite_modified_at, version, is_syncing`) then `totalCount, readCount, latestUpload, chapterFetchedAt, lastRead, bookmarkCount, category` (`MangaMapper.mapLibraryManga`, `data/src/main/java/tachiyomi/data/entries/manga/MangaMapper.kt:238-304`).
* Mapping: `readCount` and `bookmarkCount` arrive as `Double` (SqlDelight types `sum()` as REAL) and are truncated with `.toLong()`; `lastUpdate`/`nextUpdate` NULL become `0` (`MangaMapper.kt:216-217`); `genre` decodes through `StringListColumnAdapter`.
* Callers: `MangaRepositoryImpl.getLibraryManga` / `getLibraryMangaAsFlow` (`MangaRepositoryImpl.kt:56-62`), wrapped by `GetLibraryManga` (`domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetLibraryManga.kt`; `subscribe()` retries every 0.5 s on `NullPointerException` and logs other errors). Consumers: `MangaLibraryScreenModel.kt:367` (groups rows with `groupBy { it.libraryManga.category }`; shows only non-system categories when no row has `category = 0`, lines 394-400), `MangaLibraryUpdateJob.addMangaToQueue` (`app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt:158-180`, which dedupes with `distinctBy { it.manga.id }` (line 179) only in the branch where no category id is passed), `MangaMetadataUpdateJob.kt:97` (does NOT dedupe: a manga in two categories has its metadata refreshed twice), `MangaStatsScreenModel.kt:44-46` (dedupes with `fastDistinctBy { it.id }`).
* Invalidated by (Flow re-emits when any of these tables change): `mangas`, `chapters`, `history`, `excluded_scanlators`, `mangas_categories`.
* Perf: the aggregate sub-select reads every chapter row (three joins) on each emission, O(chapters). Mitigated by the `chapters_manga_id_index` and the history unique index, and in Kotlin by the UI collecting a single shared flow.

Proposal:

```rust
pub const SQL_LIBRARY: &str = "SELECT * FROM libraryView";
pub struct LibraryMangaRow { pub manga: Manga, pub category: i64, pub total_count: i64, pub read_count: i64,
    pub latest_upload: i64, pub chapter_fetched_at: i64, pub last_read: i64, pub bookmark_count: i64 }
impl LibraryMangaRow { pub fn unread_count(&self) -> i64 { self.total_count - self.read_count }
                       pub fn has_started(&self) -> bool { self.read_count > 0 }
                       pub fn has_bookmarks(&self) -> bool { self.bookmark_count > 0 } }
pub fn library(conn: &Connection) -> rusqlite::Result<Vec<LibraryMangaRow>>;
```

Read `readCount`/`bookmarkCount` with `row.get::<_, f64>(name)? as i64` (SQLite returns INTEGER for `sum()` of integer columns, but `f64` accepts both and mirrors Kotlin). Add a `fn library_distinct(rows) -> Vec<LibraryMangaRow>` helper (keeps the first row per `manga.id`) and use it in the metadata-refresh job and the stats screen; the library screen keeps duplicates because it groups by `category`. The reactive wrapper subscribes to the five tables above.

### 2.3 `libraryView`

`data/src/main/sqldelight/view/libraryView.sq:1-33`. History: created by `21.sqm`, replaced by `22.sqm` (bookmark count, `totalCount`), replaced by `27.sqm` (excluded scanlators). The text below is the current one.

```sql
CREATE VIEW libraryView AS
SELECT
    M.*,
    coalesce(C.total, 0) AS totalCount,
    coalesce(C.readCount, 0) AS readCount,
    coalesce(C.latestUpload, 0) AS latestUpload,
    coalesce(C.fetchedAt, 0) AS chapterFetchedAt,
    coalesce(C.lastRead, 0) AS lastRead,
    coalesce(C.bookmarkCount, 0) AS bookmarkCount,
    coalesce(MC.category_id, 0) AS category
FROM mangas M
LEFT JOIN(
    SELECT
        chapters.manga_id,
        count(*) AS total,
        sum(read) AS readCount,
        coalesce(max(chapters.date_upload), 0) AS latestUpload,
        coalesce(max(history.last_read), 0) AS lastRead,
        coalesce(max(chapters.date_fetch), 0) AS fetchedAt,
        sum(chapters.bookmark) AS bookmarkCount
    FROM chapters
    LEFT JOIN excluded_scanlators
    ON chapters.manga_id = excluded_scanlators.manga_id
    AND chapters.scanlator = excluded_scanlators.scanlator
    LEFT JOIN history
    ON chapters._id = history.chapter_id
    WHERE excluded_scanlators.scanlator IS NULL
    GROUP BY chapters.manga_id
) AS C
ON M._id = C.manga_id
LEFT JOIN mangas_categories AS MC
ON MC.manga_id = M._id
WHERE M.favorite = 1;
```

Output columns beyond `M.*`:

| Column | Meaning | NULL / empty handling |
|---|---|---|
| `totalCount` | number of chapters of the manga NOT hidden by an excluded scanlator | no chapters -> `0` |
| `readCount` | `sum(read)` = number of read chapters (visible ones) | `0` |
| `latestUpload` | max `chapters.date_upload` (ms) | `0` |
| `chapterFetchedAt` | max `chapters.date_fetch` (ms) | `0` |
| `lastRead` | max `history.last_read` (ms) over visible chapters; includes "reset" rows whose `last_read = 0`, which contribute 0 | `0` |
| `bookmarkCount` | `sum(chapters.bookmark)` | `0` |
| `category` | `mangas_categories.category_id` | no link -> `0` (the system category id, `Category.UNCATEGORIZED_ID`, `domain/src/main/java/tachiyomi/domain/category/model/Category.kt`) |

Semantics and edge cases (OBSERVED):

* Scanlator filter: a chapter is dropped when a row exists in `excluded_scanlators` with the same `manga_id` and exactly the same `scanlator` string. A chapter with `scanlator IS NULL` can never match (`NULL = x` is not true), so it is always counted. The `WHERE excluded_scanlators.scanlator IS NULL` runs after the LEFT JOIN, i.e. it keeps rows that found no exclusion. The library view therefore ALWAYS applies the scanlator filter; there is no switch.
* Multiple categories multiply rows (see `library`). The category join is outside the aggregate, so `totalCount`, `readCount`, `latestUpload`, `chapterFetchedAt`, `lastRead` and `bookmarkCount` repeat unchanged on every duplicate row.
* `WHERE M.favorite = 1` is evaluated in the outer query; the `library_favorite_index` partial index serves it.
* `unreadCount` is not a column; the domain computes `totalChapters - readCount` (`domain/src/main/java/tachiyomi/domain/library/manga/LibraryManga.kt`).
* The aggregate counts all chapters regardless of downloaded state; download filtering is done in Kotlin using the download cache.
* SQLite re-expands `M.*` each time the view is compiled, so columns added with `ALTER TABLE <table> ADD COLUMN` appear automatically in later queries. In Rust read by name.

Proposal: create the view byte-for-byte in `manga.db` (fresh shape) with these additions, which do not change results: `CREATE INDEX mangas_categories_manga_id_index ON mangas_categories(manga_id)` and `CREATE INDEX mangas_categories_category_id_index ON mangas_categories(category_id)`. Reason: `mangas_categories` has no index on either FK column in Aniyomi (see `mangas_categories`), so this join and every FK cascade from `categories` use an automatic transient index or a full scan. Keep both indexes out of backups (backups never contain indexes).

### 2.4 `library_favorite_index`

`data/src/main/sqldelight/data/mangas.sq:33`.

```sql
CREATE INDEX library_favorite_index ON mangas(favorite) WHERE favorite = 1;
```

* Partial index: it only contains favorite rows, so it is tiny and only usable by queries whose `WHERE` implies `favorite = 1` literally (the planner needs the literal `favorite = 1` term; a bound parameter `favorite = :x` cannot use it). Queries that qualify: `libraryView` (`WHERE M.favorite = 1`), `getFavorites`, `getFavoriteBySourceId`, `getDuplicateLibraryManga`, `getUpcomingManga`, `getSourceIdWithFavoriteCount` (all `favorite = 1` literals in `mangas.sq`).
* History: created by `7.sqm` (replacing the dropped `mangas_favorite_index`), dropped and recreated by the table rebuild in `14.sqm`. Anime twin `animelib_favorite_index` (`animes.sq:43`; the anime migration `114.sqm` recreated it under the manga-style name `library_favorite_index`, another upgraded-vs-fresh difference).
* Rust: `conn.execute_batch("CREATE INDEX IF NOT EXISTS library_favorite_index ON mangas(favorite) WHERE favorite = 1")`. rusqlite's bundled SQLite supports partial indexes (SQLite >= 3.8.0).

### 2.5 `manga_sync`

Table that stores one tracker binding per (manga, tracker). `data/src/main/sqldelight/data/manga_sync.sq:1-21`. Anime twin: `anime_sync` (`anime_sync.sq`, same shape with `anime_id`, `last_episode_seen`, `total_episodes`).

```sql
CREATE TABLE manga_sync(
    _id INTEGER NOT NULL PRIMARY KEY,
    manga_id INTEGER NOT NULL,
    sync_id INTEGER NOT NULL,
    remote_id INTEGER NOT NULL,
    library_id INTEGER,
    title TEXT NOT NULL,
    last_chapter_read REAL NOT NULL,
    total_chapters INTEGER NOT NULL,
    status INTEGER NOT NULL,
    score REAL NOT NULL,
    remote_url TEXT NOT NULL,
    start_date INTEGER NOT NULL,
    finish_date INTEGER NOT NULL,
    private INTEGER AS Boolean DEFAULT 0 NOT NULL,
    UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id)
    ON DELETE CASCADE
);
```

| Column | Type / constraint | Meaning | Introduced |
|---|---|---|---|
| `_id` | INTEGER PK | local row id (shown as `MangaTrack.id`) | baseline; rebuilt `12.sqm`, `14.sqm` |
| `manga_id` | INTEGER NOT NULL, FK -> `mangas._id` ON DELETE CASCADE | owning manga | baseline |
| `sync_id` | INTEGER NOT NULL | tracker id (`MangaTrack.trackerId`): MyAnimeList 1, AniList 2, Kitsu 3, Shikimori 4, Bangumi 5, Komga 6, MangaUpdates 7, Kavita 8, Suwayomi 9 (`app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:20-35`; Simkl 101 and Jellyfin 102 are anime-only trackers and appear only in `anime_sync`) | baseline |
| `remote_id` | INTEGER NOT NULL | id of the entry on the tracker site | baseline |
| `library_id` | INTEGER NULL | the tracker's list-entry id (AniList media-list id, Kitsu library entry id) | `6.sqm` |
| `title` | TEXT NOT NULL | title on the tracker | baseline |
| `last_chapter_read` | REAL NOT NULL | progress, fractional chapters allowed | baseline |
| `total_chapters` | INTEGER NOT NULL | total on the tracker, `0` = unknown | baseline |
| `status` | INTEGER NOT NULL | tracker-specific status code (each tracker defines its own constants; area:27-tracking-core) | baseline |
| `score` | REAL NOT NULL | tracker-specific score scale; `29.sqm` clamps MangaUpdates (`sync_id = 7`) scores to `max(score, 0)` | baseline |
| `remote_url` | TEXT NOT NULL | link to the tracker page | `5.sqm` (`DEFAULT ''`), made NOT NULL by `12.sqm` |
| `start_date` | INTEGER NOT NULL | epoch ms, `0` = unset | `8.sqm` |
| `finish_date` | INTEGER NOT NULL | epoch ms, `0` = unset | `8.sqm` |
| `private` | INTEGER NOT NULL DEFAULT 0 | privacy flag on the tracker | `32.sqm` |

Constraint details:

* `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`: inserting (or updating a row into) a pair that already exists deletes the old row and keeps the new one. Because `_id` is not supplied by the insert statement, a "re-insert" gets a NEW `_id` (max+1). The tracker code relies on this: `MangaTrackRepositoryImpl.insertManga` (`data/src/main/java/tachiyomi/data/track/manga/MangaTrackRepositoryImpl.kt:289-317`) is both create and update, via `InsertMangaTrack` (`domain/src/main/java/tachiyomi/domain/track/manga/interactor/InsertMangaTrack.kt`, errors are logged and swallowed).
* The unique constraint creates the automatic index `sqlite_autoindex_manga_sync_1` on `(manga_id, sync_id)`, which also serves `getTracksByMangaId`.
* No index on `sync_id` alone: tracker-wide scans (stats) read the whole table via `getTracks`.
* `ON DELETE CASCADE`: deleting a manga (for example "clear database" or removing non-library entries) removes its tracks.

Statements in `manga_sync.sq` (only the ones touching this table; labels as in the file):

```sql
delete:
DELETE FROM manga_sync
WHERE manga_id = :mangaId AND sync_id = :syncId;

getTracks:
SELECT *
FROM manga_sync;

getTrackById:
SELECT *
FROM manga_sync
WHERE _id = :id;

getTracksByMangaId:
SELECT *
FROM manga_sync
WHERE manga_id = :mangaId;

insert:
INSERT INTO manga_sync(manga_id,sync_id,remote_id,library_id,title,last_chapter_read,total_chapters,status,score,remote_url,start_date,finish_date,private)
VALUES (:mangaId,:syncId,:remoteId,:libraryId,:title,:lastChapterRead,:totalChapters,:status,:score,:remoteUrl,:startDate,:finishDate,:private);
```

The `update` statement of this table is documented under item `update`.

Callers: `MangaTrackRepositoryImpl.kt:258-317` (`getTrackByMangaId` actually looks up by `_id`: its parameter is named `id` and is passed to `getTrackById`), `MangaBackupCreator.kt:59`, `MangaRestorer.kt:380-411`. Invalidated by: `manga_sync`.

Proposal:

```rust
pub struct MangaTrack { pub id: i64, pub manga_id: i64, pub tracker_id: i64, pub remote_id: i64,
    pub library_id: Option<i64>, pub title: String, pub last_chapter_read: f64, pub total_chapters: i64,
    pub status: i64, pub score: f64, pub remote_url: String, pub start_date: i64, pub finish_date: i64, pub private: bool }
pub fn tracks_by_manga(conn:&Connection, manga_id:i64)->rusqlite::Result<Vec<MangaTrack>>;
pub fn track_by_id(conn:&Connection, id:i64)->rusqlite::Result<Option<MangaTrack>>;   // not by manga id
pub fn all_tracks(conn:&Connection)->rusqlite::Result<Vec<MangaTrack>>;
pub fn upsert_tracks(tx:&Transaction, tracks:&[MangaTrack])->rusqlite::Result<()>;      // runs `insert` (REPLACE semantics)
pub fn delete_track(conn:&Connection, manga_id:i64, tracker_id:i64)->rusqlite::Result<usize>;
```

Create the table text exactly as above (column `private` declared `INTEGER DEFAULT 0 NOT NULL`). Keep `ON CONFLICT REPLACE` (rusqlite passes it through untouched). The `anime_sync` twin in `anime.db`: columns `anime_id`, `last_episode_seen REAL`, `total_episodes INTEGER`, otherwise identical, `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE`, FK to `animes(_id)` ON DELETE CASCADE, migration `130.sqm` added `private`.

### 2.6 `mangas`

Main entry table of the manga database. `data/src/main/sqldelight/data/mangas.sq:6-31`. Anime twin `animes` (`animes.sq:8-41`, extra columns listed at the end of this section).

```sql
CREATE TABLE mangas(
    _id INTEGER NOT NULL PRIMARY KEY,
    source INTEGER NOT NULL,
    url TEXT NOT NULL,
    artist TEXT,
    author TEXT,
    description TEXT,
    genre TEXT AS List<String>,
    title TEXT NOT NULL,
    status INTEGER NOT NULL,
    thumbnail_url TEXT,
    favorite INTEGER AS Boolean NOT NULL,
    last_update INTEGER,
    next_update INTEGER,
    initialized INTEGER AS Boolean NOT NULL,
    viewer INTEGER NOT NULL,
    chapter_flags INTEGER NOT NULL,
    cover_last_modified INTEGER NOT NULL,
    date_added INTEGER NOT NULL,
    update_strategy INTEGER AS UpdateStrategy NOT NULL DEFAULT 0,
    calculate_interval INTEGER DEFAULT 0 NOT NULL,
    last_modified_at INTEGER NOT NULL DEFAULT 0,
    favorite_modified_at INTEGER,
    version INTEGER NOT NULL DEFAULT 0,
    is_syncing INTEGER NOT NULL DEFAULT 0
);
```

| Column | Type / default | Meaning and edge cases | Introduced |
|---|---|---|---|
| `_id` | INTEGER PK | local id; reused after deleting the highest id (0.4 item 1) | baseline |
| `source` | INTEGER NOT NULL | source id (64-bit hash of name/lang/version, see area:15-source-api-manga); `0` = local source (`LocalMangaSource.ID = 0L`, `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:378`) | baseline |
| `url` | TEXT NOT NULL | source-relative URL; the identity of a manga is `(source, url)` (`getMangaByUrlAndSource`, `LIMIT 1`, the comment in the file admits duplicates are possible) | baseline |
| `artist`, `author` | TEXT NULL | | baseline |
| `description` | TEXT NULL | | baseline |
| `genre` | TEXT NULL | `", "`-joined (0.2) | baseline |
| `title` | TEXT NOT NULL | | baseline |
| `status` | INTEGER NOT NULL | `SManga` constants (0.2) | baseline |
| `thumbnail_url` | TEXT NULL | cover URL | baseline |
| `favorite` | INTEGER NOT NULL | 1 = in library | baseline |
| `last_update` | INTEGER NULL | ms of last chapter-list change (read as `0` when NULL) | baseline |
| `next_update` | INTEGER NULL | ms of predicted next fetch (`11.sqm` added it with `DEFAULT 0`; `14.sqm` rebuilt it nullable without default) | `11.sqm` |
| `initialized` | INTEGER NOT NULL | 1 once details were fetched from the source | baseline |
| `viewer` | INTEGER NOT NULL | reader flags (0.2); set to `0` by `resetViewerFlags` | baseline |
| `chapter_flags` | INTEGER NOT NULL | chapter list sort/filter/display bits, below | baseline |
| `cover_last_modified` | INTEGER NOT NULL | ms; cache-buster of the cover file | `9.sqm` (`DEFAULT 0`) |
| `date_added` | INTEGER NOT NULL | ms when favorited, else `0` | `10.sqm` (back-filled with the minimum `date_fetch`) |
| `update_strategy` | INTEGER NOT NULL DEFAULT 0 | 0 `ALWAYS_UPDATE` / 1 `ONLY_FETCH_ONCE` | `20.sqm` |
| `calculate_interval` | INTEGER NOT NULL DEFAULT 0 | fetch interval days (0.2) | `24.sqm` |
| `last_modified_at` | INTEGER NOT NULL DEFAULT 0 | seconds, trigger-maintained (`update_last_modified_at_mangas`) | `26.sqm` (back-filled with now) |
| `favorite_modified_at` | INTEGER NULL | seconds, trigger-maintained (`update_last_favorite_at_mangas`) | `26.sqm` |
| `version` | INTEGER NOT NULL DEFAULT 0 | change counter (`update_manga_version`, `update_chapter_and_manga_version`, `insert_manga_category_update_version`) | `30.sqm` |
| `is_syncing` | INTEGER NOT NULL DEFAULT 0 | suppresses version bumps while 1 | `30.sqm` |

`chapter_flags` layout (`domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:75-110`; the anime `episode_flags` uses the same numbers with Episode names):

| Field | Mask | Values |
|---|---|---|
| sort direction | `CHAPTER_SORT_DIR_MASK = 0x1` | `CHAPTER_SORT_DESC = 0x0`, `CHAPTER_SORT_ASC = 0x1` |
| unread filter | `CHAPTER_UNREAD_MASK = 0x6` | `0x0` off, `CHAPTER_SHOW_UNREAD = 0x2`, `CHAPTER_SHOW_READ = 0x4` |
| downloaded filter | `CHAPTER_DOWNLOADED_MASK = 0x18` | `0x0` off, `CHAPTER_SHOW_DOWNLOADED = 0x8`, `CHAPTER_SHOW_NOT_DOWNLOADED = 0x10` |
| bookmarked filter | `CHAPTER_BOOKMARKED_MASK = 0x60` | `0x0` off, `CHAPTER_SHOW_BOOKMARKED = 0x20`, `CHAPTER_SHOW_NOT_BOOKMARKED = 0x40` |
| sorting | `CHAPTER_SORTING_MASK = 0x300` | `CHAPTER_SORTING_SOURCE = 0x0`, `CHAPTER_SORTING_NUMBER = 0x100`, `CHAPTER_SORTING_UPLOAD_DATE = 0x200`, `CHAPTER_SORTING_ALPHABET = 0x300` |
| title display | `CHAPTER_DISPLAY_MASK = 0x100000` | `CHAPTER_DISPLAY_NAME = 0x0`, `CHAPTER_DISPLAY_NUMBER = 0x100000` |

Indexes on this table: `library_favorite_index` (2.4), `mangas_url_index` (2.8). There is no unique index on `(source, url)`; duplicates are possible and handled by `LIMIT 1`.

Foreign keys referencing `mangas`: `chapters.manga_id`, `mangas_categories.manga_id`, `manga_sync.manga_id`, `excluded_scanlators.manga_id` (all `ON DELETE CASCADE`). Deleting a manga therefore deletes its chapters, links, tracks, exclusions, and (through `chapters`) history rows.

Triggers on the table: `update_last_favorite_at_mangas` (2.31), `update_last_modified_at_mangas` (2.35), `update_manga_version` (2.36). Statements of `mangas.sq` that belong to other catalog items: `resetViewerFlags`, `resetIsSyncing`, `update`, `selectLastInsertedRowId`. Two further statements are referenced by notes in this catalog and are reproduced here:

```sql
getDuplicateLibraryManga:
SELECT *
FROM mangas
WHERE favorite = 1
AND LOWER(title) = :title
AND _id != :id;

getUpcomingManga:
SELECT *
FROM mangas
WHERE next_update >= :startOfDay
AND favorite = 1
AND status IN :statuses
ORDER BY next_update ASC;
```

* `getDuplicateLibraryManga`: Kotlin passes `manga.title.lowercase()` (`GetDuplicateLibraryManga.kt`); SQLite `LOWER` lowercases ASCII only, so a non-ASCII upper-case letter in the stored title will not match Kotlin's Unicode-lowercased text. Proposal: lowercase both sides in Rust (`SELECT * FROM mangas WHERE favorite = 1 AND _id != :id` then filter `title.to_lowercase() == wanted`), or register a scalar function `lower_unicode` with `Connection::create_scalar_function` and compare with it. The first is simpler and the library is small.
* `getUpcomingManga`: `:startOfDay` = local midnight in ms (`MangaRepositoryImpl.kt:75`); `:statuses` is `{ONGOING = 1, PUBLISHING_FINISHED = 4}` (`domain/src/main/java/mihon/domain/upcoming/manga/interactor/GetUpcomingManga.kt:11-14`).

Anime twin `animes` additional columns (all in `animes.sq:8-41`): `episode_flags` (replaces `chapter_flags`), `fetch_type INTEGER AS FetchType NOT NULL DEFAULT 1` (131.sqm; ordinals `0 Seasons`, `1 Episodes`; 132.sqm and 135.sqm rewrote existing values), `parent_id INTEGER NULL` (131.sqm; season hierarchy, no FK declared), `season_flags INTEGER NOT NULL` (131.sqm, default 0 on upgraded DBs), `season_number REAL NOT NULL` (131.sqm, default `-1.0`), `season_source_order INTEGER NOT NULL` (131.sqm, default 0), `background_url TEXT NULL` (134.sqm), `background_last_modified INTEGER NOT NULL` (134.sqm, default 0), `memo BLOB AS JsonObject NOT NULL DEFAULT '{}'` (136.sqm, default `x'7b7d'`). Anime-only indexes: `animes_parent_id` (`133.sqm`, recreated), `animes_fetch_type`. The anime flags bit layout is documented in `plans/catalogs/preferences-02.md` section on `episode_flags` / `season_flags`.

Proposal: create `mangas` with the exact DDL above in `manga.db` (SQL text stripped of the `AS Boolean` / `AS List<String>` / `AS UpdateStrategy` annotations, which are SqlDelight-only and make SQLite reject the DDL). The `Manga` struct holds `genre: Option<Vec<String>>` encoded with `join(", ")` / `split(", ")` (empty string -> empty vec, NULL -> `None`), `update_strategy: UpdateStrategy` (`#[repr(i64)] enum { AlwaysUpdate = 0, OnlyFetchOnce = 1 }`, unknown value -> `AlwaysUpdate`), bools via rusqlite's `bool` support. Anime: `fetch_type: FetchType { Seasons = 0, Episodes = 1 }`, `memo: serde_json::Map<String, Value>` stored as BLOB `serde_json::to_vec`; on read accept `ValueRef::Blob` and `ValueRef::Text`; map a parse error to `{}`.

### 2.7 `mangas_categories`

Link table manga <-> category. `data/src/main/sqldelight/data/mangas_categories.sq:1-9`. Anime twin `animes_categories`.

```sql
CREATE TABLE mangas_categories(
    _id INTEGER NOT NULL PRIMARY KEY,
    manga_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL,
    FOREIGN KEY(category_id) REFERENCES categories (_id)
    ON DELETE CASCADE,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id)
    ON DELETE CASCADE
);
```

| Column | Meaning | Introduced |
|---|---|---|
| `_id` | row id | baseline |
| `manga_id` | FK `mangas._id`, cascade | baseline |
| `category_id` | FK `categories._id`, cascade (`categories` row `0` is the system "Default" category and is never linked: a manga without a link is shown in category 0 by `libraryView`) | baseline |
| (upgraded DBs only) `last_modified_at INTEGER NOT NULL DEFAULT 0` | seconds | `26.sqm`, absent on fresh installs |

* No `UNIQUE (manga_id, category_id)`. `setMangaCategories` deletes then inserts, so duplicates only occur if a caller passes duplicate ids. A duplicate would multiply library rows.
* No index on `manga_id` or `category_id` (see the Proposal in 2.3).
* Statements in the file:

```sql
insert:
INSERT INTO mangas_categories(manga_id, category_id)
VALUES (:mangaId, :categoryId);

deleteMangaCategoryByMangaId:
DELETE FROM mangas_categories
WHERE manga_id = :mangaId;
```

* Callers: `MangaRepositoryImpl.setMangaCategories` (`MangaRepositoryImpl.kt:91-98`, one transaction: delete then insert each id; the Kotlin `.map` call there is used only for side effects), wrapped by `SetMangaCategories` (`domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetMangaCategories.kt`, catches and logs every exception, so failures are silent); UI callers: `MangaScreenModel.kt:476`, `BrowseMangaSourceScreenModel.kt:312`, `MangaHistoryScreenModel.kt:165`, `MangaLibraryScreenModel.kt:547-560`, `MigrateMangaDialog.kt:261`; backup: `MangaRestorer.kt:313-317`.
* Triggers: `insert_manga_category_update_version` (2.1). Deleting rows has no trigger.
* Invalidated by: `mangas_categories`.

Proposal: DDL exactly as above plus the two indexes from 2.3 (`mangas_categories_manga_id_index`, `mangas_categories_category_id_index`); do NOT create the `last_modified_at` column. Add `UNIQUE(manga_id, category_id)`? No: keep Aniyomi's shape; instead `set_manga_categories` de-duplicates the id list (`BTreeSet`) before inserting. Anime: table `animes_categories` with `anime_id`, trigger `insert_anime_category_update_version`.

### 2.8 `mangas_url_index`

`data/src/main/sqldelight/data/mangas.sq:34`.

```sql
CREATE INDEX mangas_url_index ON mangas(url);
```

* Full (non-partial) index on `url`. Serves `getMangaByUrlAndSource` (`WHERE url = :url AND source = :source LIMIT 1`): the planner seeks `url` and filters `source`. Also serves `NetworkToLocalManga` (look up before insert, `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/NetworkToLocalManga.kt`, via `GetMangaByUrlAndSourceId`).
* A composite `(source, url)` index would be more selective; Aniyomi does not have one because many sources use short URLs like `/manga/1`. Proposal: create exactly this index (name and shape), and in addition nothing; per-source collisions are bounded by the number of sources.
* Introduced: baseline, dropped and recreated in `14.sqm`. Anime twin `animes_url_index` (`animes.sq:44`).
* Rust: `CREATE INDEX IF NOT EXISTS mangas_url_index ON mangas(url)`.

### 2.9 `remove`

Query of the `excluded_scanlators` table. `data/src/main/sqldelight/data/excluded_scanlators.sq:14-17`.

```sql
remove:
DELETE FROM excluded_scanlators
WHERE manga_id = :mangaId
AND scanlator IN :scanlators;
```

Table (same file, lines 1-8; introduced by `27.sqm`, no later change). Manga-only: the anime DB has no excluded-scanlators table.

```sql
CREATE TABLE excluded_scanlators(
    manga_id INTEGER NOT NULL,
    scanlator TEXT NOT NULL,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id)
    ON DELETE CASCADE
);

CREATE INDEX excluded_scanlators_manga_id_index ON excluded_scanlators(manga_id);
```

Sibling statements: `insert: INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (:mangaId, :scanlator);` and `getExcludedScanlatorsByMangaId: SELECT scanlator FROM excluded_scanlators WHERE manga_id = :mangaId;`.

* Parameters: `:mangaId` Long; `:scanlators` `Collection<String>` expanded to `IN (?,?,?)` (one `?` per element). Comparison is byte-exact, case-sensitive (default BINARY collation).
* Returns nothing (rows deleted). An empty collection makes `IN ()` false, nothing is deleted.
* No primary key and no unique constraint: duplicate `(manga_id, scanlator)` rows are possible; `remove` deletes all duplicates of a given scanlator.
* Caller: `SetExcludedScanlators.await` (`app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetExcludedScanlators.kt:9-21`), one transaction: read the current set, `toAdd = wanted - current` inserted one by one, `toRemove = current - wanted` removed with this statement. The UI is the "Filter by scanlator" dialog (`app/src/main/java/eu/kanade/presentation/entries/manga/components/ScanlatorFilterDialog.kt`, opened from `MangaScreenModel`). Effects after the commit: `libraryView` counts, the chapter list (`getChaptersByMangaId` with `:applyScanlatorFilter = 1`), next-chapter selection and the updates list change.
* Interaction notes (OBSERVED): (1) A chapter whose `scanlator` is NULL can never be excluded, because the join in `getChaptersByMangaId` and in `libraryView` compares `chapters.scanlator = excluded_scanlators.scanlator`. (2) `ChapterRepositoryImpl.getScanlatorsByMangaId` maps NULL to the empty string (`{ it.orEmpty() }`, `ChapterRepositoryImpl.kt:89-99`), so the dialog may offer `""`; excluding `""` stores an empty-string row that matches only chapters whose scanlator is the empty string, not NULL. (3) `27.sqm`/`28.sqm`: scanlator strings are trimmed (`trim(scanlator)`) once by migration 28; new chapters are not trimmed by SQL.
* Invalidated by: `excluded_scanlators` (and, through `libraryView`, the library flow).

Proposal:

```rust
pub const SQL_EXCLUDED_REMOVE_PREFIX: &str = "DELETE FROM excluded_scanlators WHERE manga_id = :mangaId AND scanlator IN ";
/// Builds `IN (?,?,..)` per chunk of 500 scanlators; no-op for an empty slice.
pub fn remove_excluded_scanlators(tx: &Transaction, manga_id: i64, scanlators: &[String]) -> rusqlite::Result<usize>;
pub fn set_excluded_scanlators(conn: &mut Connection, manga_id: i64, wanted: &BTreeSet<String>) -> rusqlite::Result<()>; // one tx, same add/remove diff
```

Positional parameters `?` are used for the chunk list and `:mangaId` is bound by name, which rusqlite does not allow mixing in one statement; implement as `?1` for the manga id and `?2..?n+1` for the list.

### 2.10 `removeAllHistory`

Two variants (manga DB and anime DB).

```sql
-- data/src/main/sqldelight/data/history.sq:54
removeAllHistory:
DELETE FROM history;

-- data/src/main/sqldelightanime/dataanime/animehistory.sq:51
removeAllHistory:
DELETE FROM animehistory;
```

* No parameters, deletes every row, no `WHERE`. No triggers exist on `history` / `animehistory`. Returns the affected row count in SQLite; Kotlin ignores it.
* This is a HARD delete, unlike the `reset*` statements: `history.time_read` (the accumulated reading time that feeds the Statistics screen via `getReadDuration`: `SELECT coalesce(sum(time_read), 0) FROM history`) is lost too. Chapter `read`/`bookmark` flags are not touched. The anime table has no duration column.
* Callers: `MangaHistoryRepositoryImpl.deleteAllMangaHistory` (`data/src/main/java/tachiyomi/data/history/manga/MangaHistoryRepositoryImpl.kt:52-60`: returns `true`, or `false` after logging an exception), `RemoveMangaHistory.awaitAll` (`domain/src/main/java/tachiyomi/domain/history/manga/interactor/RemoveMangaHistory.kt`), `MangaHistoryScreenModel.removeAllHistory` (`app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt:133-139`: only on `true` it emits `Event.HistoryCleared`), button in `MangaHistoryTab.kt:109`. Anime: `AnimeHistoryRepositoryImpl.deleteAllAnimeHistory` (`data/src/main/java/tachiyomi/data/history/anime/AnimeHistoryRepositoryImpl.kt:53-61`), `AnimeHistoryTab.kt:112`.
* Invalidated by effect: the history list, "continue reading" (`getLatestHistory` returns no row), library `lastRead` sort values (`libraryView.lastRead` becomes 0 for everyone).

Proposal:

```rust
pub const SQL_HISTORY_REMOVE_ALL: &str = "DELETE FROM history";
pub const SQL_ANIME_HISTORY_REMOVE_ALL: &str = "DELETE FROM animehistory";
pub fn remove_all_history(conn: &Connection) -> rusqlite::Result<usize>;        // manga.db
pub fn remove_all_anime_history(conn: &Connection) -> rusqlite::Result<usize>;  // anime.db
```

The confirmation dialog text must state that reading statistics (total time) are reset too, since that is the observable difference from "remove from history" (the user loses total read time and the "continue reading" shortcut).

### 2.11 `removeChaptersWithIds`

`data/src/main/sqldelight/data/chapters.sq:90-92`.

```sql
removeChaptersWithIds:
DELETE FROM chapters
WHERE _id IN :chapterIds;
```

* Parameter: list of `chapters._id`. Hard delete. Cascades by FK (`ON DELETE CASCADE`): the `history` row of each chapter is deleted too (so reading time of a vanished chapter leaves the total). The unread index and manga index are maintained automatically. No trigger fires on DELETE.
* Caller: `ChapterRepositoryImpl.removeChaptersWithIds` (`data/src/main/java/tachiyomi/data/items/chapter/ChapterRepositoryImpl.kt:75-81`; exceptions are logged, not rethrown) <- `SyncChaptersWithSource.kt:209-212` (`removedChapters` = chapters in the DB whose `url` is not in the freshly fetched list). Context OBSERVED in the same file: before deleting, read/bookmark state and the `date_fetch` of the removed chapters are remembered by chapter NUMBER (`deletedReadChapterNumbers`, `deletedBookmarkedChapterNumbers`, `deletedChapterNumberDateFetchMap`, lines 160-175) and copied onto newly inserted chapters with the same recognized number (lines 184-205), so a source that re-publishes chapters under new URLs does not reset progress and does not flood the Updates list. Order inside one sync: delete removed ids, `addAllChapters`, `updateAllChapters`, then fetch-interval update; these are separate transactions (not one).
* Invalidated by: `chapters`, `history` (cascade).
* Downloads for the deleted chapters are not touched here (area:23-downloads-manga).

Proposal:

```rust
pub fn remove_chapters_with_ids(tx: &Transaction, ids: &[i64]) -> rusqlite::Result<usize>; // chunks of 500, empty = Ok(0)
```

In Kurobayomi run delete + insert + update of one sync in ONE transaction (improvement over Aniyomi): a crash between the delete and the insert would otherwise leave the manga without its chapters.

### 2.12 `removeEpisodesWithIds`

`data/src/main/sqldelightanime/dataanime/episodes.sq:85-87`.

```sql
removeEpisodesWithIds:
DELETE FROM episodes
WHERE _id IN :episodeIds;
```

Same semantics as `removeChaptersWithIds` for the anime DB: hard delete, cascades to `animehistory` (`FOREIGN KEY(episode_id) REFERENCES episodes (_id) ON DELETE CASCADE`), no trigger. Caller: `EpisodeRepositoryImpl.removeEpisodesWithIds` (`data/src/main/java/tachiyomi/data/items/episode/EpisodeRepositoryImpl.kt:161-167`, logs exceptions) <- `SyncEpisodesWithSource.kt:221-224`, with the identical number-based carry-over of `seen`, `bookmark` and `date_fetch` (lines 181-216 of that file). Proposal: `pub fn remove_episodes_with_ids(tx:&Transaction, ids:&[i64]) -> rusqlite::Result<usize>`, chunks of 500.

### 2.13 `removeParentIdByIds`

Anime DB only. `data/src/main/sqldelightanime/dataanime/animes.sq:152-155`.

```sql
removeParentIdByIds:
UPDATE animes
SET parent_id = NULL
WHERE _id IN :animeIds;
```

* `animes.parent_id` links a season entry to its parent series (no declared FK). Seasons are themselves rows of `animes`. This statement detaches rows (they become top-level entries) and is the ONLY way to set `parent_id` back to NULL, because the generic `update` uses `coalesce(:parentId, parent_id)` (NULL = keep).
* Triggers fired per row: `update_last_modified_at_animes` (stamps seconds); `update_anime_version` (condition false, no bump); `update_last_favorite_at_animes` not fired (no `favorite` in SET).
* Callers: `AnimeRepositoryImpl.removeParentIdByIds` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:170-176`, logs exceptions) <- `SyncSeasonsWithSource.kt:93-96` (seasons that the source no longer lists are detached, not deleted) and `ClearAnimeDatabaseScreen.kt:290-296` (when "clear database" deletes non-library parents, the library favourites among their children are detached so they do not dangle; the preceding `deleteAnimesNotInLibraryByAnimeIds` and this statement are two separate statements, not one transaction).
* Invalidated by: `animes` (seasons views, library).

Proposal: `pub fn remove_parent_id_by_ids(tx:&Transaction, ids:&[i64]) -> rusqlite::Result<usize>` chunked at 500; in the clear-database action run `delete_animes_not_in_library_by_ids` and this call in one transaction.

### 2.14 `removeResettedHistory`

Two variants.

```sql
-- history.sq:57
removeResettedHistory:
DELETE FROM history
WHERE last_read = 0;

-- animehistory.sq:54
removeResettedHistory:
DELETE FROM animehistory
WHERE last_seen = 0;
```

* Deletes the rows that `resetHistoryById` / `resetHistoryByMangaId` (and anime twins) have hidden (`last_read = 0`). Rows with `last_read IS NULL` (possible only on fresh installs, see 0.3) are NOT matched.
* Deleting these rows discards their `time_read`, so the Statistics "total read duration" drops by the hidden amount; Aniyomi calls it only as housekeeping after clearing non-library entries.
* Callers: only the "Clear database" settings screens: `ClearDatabaseScreen.kt:212-213` (after `deleteMangasNotInLibraryBySourceIds(state.selection)`) and `ClearAnimeDatabaseScreen.kt:294-296`. Both run inside `withNonCancellableContext`, not in a DB transaction.
* Invalidated by: `history` / `animehistory`.

Proposal: `pub fn remove_resetted_history(conn:&Connection) -> rusqlite::Result<usize>` (one function per DB). Run it in the same transaction as the preceding delete.

### 2.15 `replace`

Manga DB only (the anime DB dropped `extension_repos` in migration `137.sqm`). `data/src/main/sqldelight/data/extension_repos.sq:43-53`.

```sql
replace:
INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint)
VALUES (:base_url, :name, :short_name, :website, :fingerprint)
ON CONFLICT(signing_key_fingerprint)
DO UPDATE
SET
    base_url = :base_url,
    name = :name,
    short_name = :short_name,
    website =: website
WHERE signing_key_fingerprint = signing_key_fingerprint;
```

Table (`extension_repos.sq:1-7`, created by `31.sqm` in the manga DB, by `128.sqm` in the anime DB then dropped by `137.sqm`):

```sql
CREATE TABLE extension_repos (
    base_url TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    short_name TEXT,
    website TEXT NOT NULL,
    signing_key_fingerprint TEXT UNIQUE NOT NULL
);
```

* Behaviour: inserts a new repo; if another row already has the same `signing_key_fingerprint`, that row is rewritten with the new `base_url`, `name`, `short_name`, `website` (its fingerprint stays). `website =: website` is a typo that SQLite parses as `website = :website`. The trailing `WHERE signing_key_fingerprint = signing_key_fingerprint` is always true.
* Not handled: a PRIMARY KEY conflict on `base_url` (new URL equal to a different existing row). The upsert target only names the fingerprint, so SQLite raises `UNIQUE constraint failed: extension_repos.base_url`. `MangaExtensionRepoRepositoryImpl.replaceRepo` (`mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt:479-489`) has no try/catch, unlike `insertRepo`/`upsertRepo`, so the exception would propagate; it cannot happen on the UI path because `CreateMangaExtensionRepo` (`domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt:17-24,45-58`) first reports `RepoAlreadyExists` when the URL exists.
* Caller chain: Settings > Browse > Manga extension repos, the "repository with this signing key already exists, replace?" dialog (`MangaExtensionReposScreen.kt:74` -> `MangaExtensionReposScreenModel.replaceRepo` lines 71-75 -> `ReplaceMangaExtensionRepo` -> `replaceRepo`). That is the "migrate repo URL for the same maintainer" case.
* Invalidated by: `extension_repos`.

Proposal: carry over only if area:20-extension-strategy-redesign keeps an extension-repo table; if so the statement is:

```rust
pub const SQL_EXT_REPO_REPLACE: &str = "INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) \
 VALUES (:base_url, :name, :short_name, :website, :fingerprint) \
 ON CONFLICT(signing_key_fingerprint) DO UPDATE SET base_url = :base_url, name = :name, short_name = :short_name, website = :website";
pub fn replace_repo(conn:&Connection, r:&ExtensionRepo) -> rusqlite::Result<()>; // maps SQLITE_CONSTRAINT to an Err the UI shows as the "repo already exists" message
```

(The redundant `WHERE` is dropped, `=:` is written `= :`.) The user-visible outcome is unchanged: one repo per signing key.

### 2.16 `resetAnimeHistoryById`

Anime DB. `data/src/main/sqldelightanime/dataanime/animehistory.sq:33-36`.

```sql
resetAnimeHistoryById:
UPDATE animehistory
SET last_seen = 0
WHERE _id = :historyId;
```

* Sets the row's `last_seen` to `0` (the "removed from history list" marker; the row, its `episode_id` and the episode's `seen` state stay). The anime history list query filters `WHERE animehistoryView.seenAt > 0` (`data/src/main/sqldelightanime/view/animehistoryView.sq`), so the row disappears from the list. A later `upsert` for the same episode revives it (`last_seen` is overwritten).
* Caller: `AnimeHistoryRepositoryImpl.resetAnimeHistory` (`AnimeHistoryRepositoryImpl.kt:37-43`, logs failures) <- `RemoveAnimeHistory.await(history)` <- `AnimeHistoryScreenModel.removeFromHistory` (`app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryScreenModel.kt:121-125`), confirmed in `AnimeHistoryTab.kt:103` (dialog "remove this entry").
* Invalidated by: `animehistory`.

Proposal: `pub fn reset_anime_history_by_id(conn:&Connection, history_id:i64)->rusqlite::Result<usize>` with the SQL above verbatim.

### 2.17 `resetHistoryByAnimeId`

Anime DB. `animehistory.sq:38-49`.

```sql
resetHistoryByAnimeId:
UPDATE animehistory
SET last_seen = 0
WHERE _id IN (
    SELECT H._id
    FROM animes M
    INNER JOIN episodes EP
    ON M._id = EP.anime_id
    INNER JOIN animehistory H
    ON EP._id = H.episode_id
    WHERE M._id = :animeId
);
```

* Hides the whole history of one anime (every episode's history row), keeps rows. Equivalent shorter form (because `animehistory.episode_id` is UNIQUE and the inner joins only filter): `UPDATE animehistory SET last_seen = 0 WHERE episode_id IN (SELECT _id FROM episodes WHERE anime_id = :animeId)`. Kotlin uses the long form; Rust may use either.
* Caller: `AnimeHistoryRepositoryImpl.resetHistoryByAnimeId` (`:45-51`, logs) <- `RemoveAnimeHistory.await(animeId)` <- `AnimeHistoryScreenModel.removeAllFromHistory(animeId)` (`AnimeHistoryScreenModel.kt:127-131`), triggered by "remove all for this series" in the history tab.
* Invalidated by: `animehistory`.

Proposal: `pub fn reset_history_by_anime_id(conn:&Connection, anime_id:i64)->rusqlite::Result<usize>` (long form kept verbatim to stay diff-able against Aniyomi).

### 2.18 `resetHistoryById`

Manga DB. `data/src/main/sqldelight/data/history.sq:36-39`.

```sql
resetHistoryById:
UPDATE history
SET last_read = 0
WHERE _id = :historyId;
```

* Sets `last_read = 0`; keeps the row, `chapter_id` and `time_read`. The manga history query filters `historyView.readAt > 0` and `maxReadAtChapterId = historyView.chapterId`, where `maxReadAtChapterId` is the chapter with the greatest `last_read` of that manga (including reset rows, which count as 0). Resetting the newest chapter's row therefore makes the manga reappear with its second newest chapter if that one still has `last_read > 0`; if all rows are reset the manga disappears from the list.
* `time_read` is preserved so statistics are unchanged until `removeResettedHistory` runs.
* Caller: `MangaHistoryRepositoryImpl.resetMangaHistory` (`MangaHistoryRepositoryImpl.kt:36-42`, logs failures) <- `RemoveMangaHistory.await(history)` <- `MangaHistoryScreenModel.removeFromHistory` (`MangaHistoryScreenModel.kt:121-125`).
* Invalidated by: `history`.

Proposal: `pub fn reset_history_by_id(conn:&Connection, history_id:i64)->rusqlite::Result<usize>`.

### 2.19 `resetHistoryByMangaId`

Manga DB. `history.sq:41-52`.

```sql
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
```

* Hides every history row of the manga; `time_read` kept. Same equivalence note as 2.17.
* Caller: `MangaHistoryRepositoryImpl.resetHistoryByMangaId` (`:44-50`, logs) <- `RemoveMangaHistory.await(mangaId)` <- `MangaHistoryScreenModel.removeAllFromHistory(mangaId)` (`MangaHistoryScreenModel.kt:127-131`).
* Invalidated by: `history`.

Proposal: `pub fn reset_history_by_manga_id(conn:&Connection, manga_id:i64)->rusqlite::Result<usize>`.

### 2.20 `resetIsSyncing`

Four variants, all identical in shape.

```sql
-- data/src/main/sqldelight/data/chapters.sq:94
resetIsSyncing:
UPDATE chapters
SET is_syncing = 0
WHERE is_syncing = 1;

-- data/src/main/sqldelight/data/mangas.sq:136
resetIsSyncing:
UPDATE mangas
SET is_syncing = 0
WHERE is_syncing = 1;

-- data/src/main/sqldelightanime/dataanime/animes.sq:147
resetIsSyncing:
UPDATE animes
SET is_syncing = 0
WHERE is_syncing = 1;

-- data/src/main/sqldelightanime/dataanime/episodes.sq:89
resetIsSyncing:
UPDATE episodes
SET is_syncing = 0
WHERE is_syncing = 1;
```

* OBSERVED: no Kotlin code calls any of these four (searched all `*.kt` for `resetIsSyncing`; the only hits are the generated names inside the `.sq` files). They are dead statements. The flag is nevertheless live: the restorers set `is_syncing = 1` while merging a backup into an existing entry (`MangaRestorer.kt:131`, `AnimeRestorer.kt:147`) so the version triggers stay quiet, and it returns to `0` through the regular partial update (`MangaRepositoryImpl.partialUpdateManga` always binds `isSyncing = 0`, line 173) which the restorer reaches at the end of each entry via `updateManga.awaitUpdateFetchInterval` (`MangaRestorer.kt:283`, `UpdateManga.kt:79-85`). The whole entry restore runs in one transaction (`MangaRestorer.kt:56-60`), so an abort rolls the flag back; there is no state in which the flag could be stuck.
* Row effect if it did run: per matching row, `update_last_modified_at_*` fires (stamps `last_modified_at`); no version bump.
* Decision: DROPPED (not ported). Reason: dead code with no failure mode to repair. What the user loses: nothing. The `is_syncing` columns, their DEFAULT 0 and the triggers that read them are kept (see `mangas`, `update_manga_version`).

### 2.21 `resetViewerFlags`

Two variants.

```sql
-- data/src/main/sqldelight/data/mangas.sq:132
resetViewerFlags:
UPDATE mangas
SET viewer = 0;

-- data/src/main/sqldelightanime/dataanime/animes.sq:143
resetViewerFlags:
UPDATE animes
SET viewer = 0;
```

* No `WHERE`: every row (favorite or not) gets `viewer = 0` = reading mode DEFAULT + orientation DEFAULT (per-series reader overrides are forgotten and the global reader preferences apply).
* Triggers: for each row `update_last_modified_at_mangas` stamps `last_modified_at`, `update_manga_version` does not bump (url/description/favorite unchanged), `update_last_favorite_at_mangas` is not fired (no `favorite` in SET). Result: every manga's `last_modified_at` changes, which only matters for sync/backup ordering.
* Caller: Settings > Advanced > "Reset per-series reader settings" (strings `pref_reset_viewer_flags` = "Reset per-series reader settings", summary "Resets reading mode and orientation of all series", `pref_reset_viewer_flags_success` = "All reader settings reset", `pref_reset_viewer_flags_error` = "Couldn't reset reader settings"; `i18n/src/commonMain/moko-resources/base/strings.xml:620-623`) at `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:326-340` -> `ResetMangaViewerFlags` -> `MangaRepositoryImpl.resetMangaViewerFlags` (`MangaRepositoryImpl.kt:81-89`: `false` after a logged exception). Toast shown with the success or error string. The anime variant has an interactor (`ResetAnimeViewerFlags`, `DomainModule.kt:247`) and repository method but NO UI entry point; the Advanced screen resets only manga. Anime viewer flags are therefore never reset from the UI.
* Invalidated by: `mangas` / `animes` (library flow re-emits).

Proposal:

```rust
pub const SQL_MANGA_RESET_VIEWER_FLAGS: &str = "UPDATE mangas SET viewer = 0";
pub fn reset_viewer_flags(conn:&Connection) -> rusqlite::Result<usize>; // manga.db; caller maps Err -> toast "Couldn't reset reader settings"
```

Port the manga entry point to Settings > Advanced with the same strings; for anime provide the same function in the anime DB crate and expose one combined action ("Reset per-series reader settings" runs both) because Kurobayomi has one Advanced page; the user gains a reset that Aniyomi never exposed for anime.

### 2.22 `selectLastInsertedRowId`

Seven variants, all `SELECT last_insert_rowid();`.

```sql
selectLastInsertedRowId:
SELECT last_insert_rowid();
```

| Variant file | Used by |
|---|---|
| `data/src/main/sqldelight/data/mangas.sq:181` | `MangaRepositoryImpl.insertManga` (`MangaRepositoryImpl.kt:100-126`: `awaitOneOrNullExecutable(inTransaction = true) { mangasQueries.insert(<all columns>); mangasQueries.selectLastInsertedRowId() }`, returns `Long?` new `_id`); `MangaRestorer.restoreNewManga` (`MangaRestorer.kt:137-148,243-265`) |
| `data/src/main/sqldelight/data/chapters.sq:120` | `ChapterRepositoryImpl.addAllChapters` (`ChapterRepositoryImpl.kt:16-42`: one transaction, per chapter `insert` then `selectLastInsertedRowId().executeAsOne()`, returns the chapters with their new ids; on any exception the transaction rolls back and the function returns an EMPTY list after logging) |
| `data/src/main/sqldelightanime/dataanime/animes.sq:203` | `AnimeRepositoryImpl.insertAnime` (`AnimeRepositoryImpl.kt:104-138`), `AnimeRestorer` |
| `data/src/main/sqldelightanime/dataanime/episodes.sq:120` | `EpisodeRepositoryImpl.addAllEpisodes` (`EpisodeRepositoryImpl.kt:92-123`, same rollback-to-empty-list behaviour) |
| `data/src/main/sqldelight/data/categories.sq:91` | no caller (dead): `MangaCategoryRepositoryImpl.insertMangaCategory` returns nothing |
| `data/src/main/sqldelightanime/dataanime/categories.sq:91` | no caller (dead) |
| `data/src/main/sqldelightanime/dataanime/custom_buttons.sq:55` | no caller (dead): `CustomButtonRepositoryImpl.insertCustomButton` returns nothing |

* The result is the rowid of the last successful `INSERT` on THE SAME connection. It is only meaningful when the insert and this select run on one connection with nothing in between; Aniyomi guarantees that by running both in one transaction on the single transaction thread. No trigger in these schemas inserts a row, so there is no trigger interference.
* Because there is no `AUTOINCREMENT` the id may equal a previously deleted highest id (0.4 item 1).

Proposal: no SQL. Use `Transaction::last_insert_rowid()` immediately after `execute`; do not port the select. The three dead variants are dropped (no loss). Signatures: `pub fn insert_manga(tx:&Transaction, m:&NewManga) -> rusqlite::Result<i64>`, `pub fn add_all_chapters(tx:&Transaction, chapters:&[NewChapter]) -> rusqlite::Result<Vec<Chapter>>` (propagates the error instead of returning an empty list; the caller's transaction rolls back), `insert_anime`, `add_all_episodes` likewise.

### 2.23 `sources`

Table caching the id, language and name of every source ever loaded, so that library entries of a source whose extension is no longer installed can still show a name and so that downloads can be located and renamed. `data/src/main/sqldelight/data/sources.sq:1-24`; created by `16.sqm`. Anime twin `animesources` (`data/src/main/sqldelightanime/dataanime/animesources.sq`, created by `117.sqm`, identical columns).

```sql
CREATE TABLE sources(
    _id INTEGER NOT NULL PRIMARY KEY,
    lang TEXT NOT NULL,
    name TEXT NOT NULL
);

findAll:
SELECT *
FROM sources;

findOne:
SELECT *
FROM sources
WHERE _id = :id;
```

(`upsert` of this table: see item `upsert`, variant 1.)

| Column | Meaning |
|---|---|
| `_id` | the 64-bit source id (not auto-generated; the extension chooses it) |
| `lang` | language code the source reported (for example `"en"` or `"all"`; any string the extension reports); `""` for a source never seen with data |
| `name` | display name |

* How it is filled (OBSERVED): `AndroidMangaSourceManager` (`app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt:60-123`) calls `registerStubSource(StubMangaSource.from(it))` for every source of every loaded extension (line 66) and `registerStubSource` upserts the row if `dbSource != source`, and when a row already existed with different data it calls `downloadManager.renameSource(dbSource, source)` so the download folder follows a renamed source (lines 104-113). `createStubSource(id)` returns the DB row, else the extension manager's source data, else `StubMangaSource(id, lang = "", name = "")` (lines 115-123). Lookup `getOrStub(id)` is therefore never null and is what the library, history and source-management screens call to name a source.
* Quirk (OBSERVED): the collector at `AndroidMangaSourceManager.kt:74-82` copies `stubSourcesMap` into a new mutable map, adds the DB rows to the copy, and drops the copy, so the in-memory stub map is only filled by `createStubSource`, not by the flow.
* Rows are never deleted by the app. No FK (not referenced by `mangas.source`).
* Used for: `getStubSources()` for the download cache (`MangaDownloadCache.kt:412`, `AnimeDownloadCache.kt:412`), `getMangaSourcesWithFavoriteCount`, `getMangaSourcesWithNonLibraryManga`, "Source not installed" labels, migration screens.
* Invalidated by: `sources`.

Proposal: keep the table verbatim (`sources` in `manga.db`, `animesources` in `anime.db`), because Kurobayomi still needs a name for entries whose source plugin is not loaded and for download folders (area:17-source-app-layer, area:23-downloads-manga). Rust:

```rust
pub struct StubSource { pub id: i64, pub lang: String, pub name: String }
pub fn stub_sources(conn:&Connection) -> rusqlite::Result<Vec<StubSource>>;               // SELECT * FROM sources
pub fn stub_source(conn:&Connection, id:i64) -> rusqlite::Result<Option<StubSource>>;     // WHERE _id = :id
pub fn upsert_stub_source(conn:&Connection, s:&StubSource) -> rusqlite::Result<()>;       // see `upsert`
```

Also port the "rename the download folder when a known source changes name" behaviour as a caller of `upsert_stub_source`; implement the in-memory map as the cache of this table and fix the Aniyomi quirk by assigning the merged map.

### 2.24 `system_category_delete_trigger`

Two variants (one per database), same text. `data/src/main/sqldelight/data/categories.sq:1-18` (manga) and `data/src/main/sqldelightanime/dataanime/categories.sq:1-18` (anime).

```sql
CREATE TABLE categories(
    _id INTEGER NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    sort INTEGER NOT NULL,
    flags INTEGER NOT NULL,
    hidden INTEGER NOT NULL DEFAULT 0
);

-- Insert system category
INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0);
-- Disallow deletion of default category
CREATE TRIGGER IF NOT EXISTS system_category_delete_trigger BEFORE DELETE
ON categories
BEGIN SELECT CASE
    WHEN old._id <= 0 THEN
        RAISE(ABORT, "System category can't be deleted")
    END;
END;
```

* `categories` columns: `_id` (0 is the system "Default/uncategorized" row, user rows get `max+1` ids), `name`, `sort` (display order; the system row has `-1` so it sorts first), `flags` (library sort bits of that category, below), `hidden` (0/1, added by manga `25.sqm` / anime `125.sqm`; `14.sqm`/`114.sqm` had rebuilt the table without it).
* The system row is created at schema creation (`INSERT OR IGNORE` is a no-op if it exists). Introduced for upgraded DBs by `19.sqm` (manga) and `120.sqm` (anime, trigger named `system_anime_category_delete_trigger` there; see 0.3).
* Trigger semantics: for ANY direct `DELETE` that would remove a row with `_id <= 0`, `RAISE(ABORT, "System category can't be deleted")` aborts the statement with `SQLITE_CONSTRAINT_TRIGGER` and the message `System category can't be deleted`; the whole statement is rolled back (a multi-row `DELETE` containing the system row deletes nothing). `BEFORE DELETE` also fires for deletes by cascade from another table, but nothing cascades into `categories`.
* UI never offers the deletion: categories screens filter out `Category.isSystemCategory` (`ReorderMangaCategory.kt:21` filters it; the category list screens do the same), so the trigger is a safety net. `DeleteMangaCategory` (`domain/src/main/java/tachiyomi/domain/category/manga/interactor/DeleteMangaCategory.kt`) wraps `categoriesQueries.delete` in try/catch and returns `Result.InternalError`.
* Quirk (OBSERVED): after a deletion `DeleteMangaCategory` renumbers `sort` with `getAllMangaCategories().mapIndexed { index, _ -> order = index }`, and that list INCLUDES the system row (sort `-1`, first). The system row becomes `sort = 0` and user categories `1..n`, whereas `ReorderMangaCategory` (which excludes it) numbers user categories `0..n-1` and leaves the system row at `-1`. Display order is unaffected because the system row still sorts first.

Library-sort flags stored in `categories.flags` (`domain/src/main/java/tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt:7-80`): type mask `0b00111100` (0x3C): `Alphabetical 0x00`, `LastRead 0x04`, `LastUpdate 0x08`, `UnreadCount 0x0C`, `TotalChapters 0x10`, `LatestChapter 0x14`, `ChapterFetchDate 0x18`, `DateAdded 0x1C`, `TrackerMean 0x20`, `Random 0x3C`; direction mask `0b01000000` (0x40): `Ascending 0x40`, `Descending 0x00`. Anime (`domain/src/main/java/tachiyomi/domain/library/anime/model/AnimeLibrarySortMode.kt:25-53`): the same bits with `LastSeen 0x04`, `UnseenCount 0x0C`, `TotalEpisodes 0x10`, `LatestEpisode 0x14`, `EpisodeFetchDate 0x18`, and one extra type `AiringTime 0x30`. Unknown type bits decode to the default `Alphabetical`, unknown direction to `Ascending` (`default = MangaLibrarySort(Type.Alphabetical, Direction.Ascending)`). The preference serialization is `<TYPE>,<DIRECTION>` with TYPE one of `ALPHABETICAL`, `LAST_READ`, `LAST_MANGA_UPDATE`, `UNREAD_COUNT`, `TOTAL_CHAPTERS`, `LATEST_CHAPTER`, `CHAPTER_FETCH_DATE`, `DATE_ADDED`, `TRACKER_MEAN`, `RANDOM` and DIRECTION `ASCENDING` or `DESCENDING` (unparseable input gives the default `ALPHABETICAL,ASCENDING`); the preference itself is in area:02-preferences-system.

Proposal: create exactly this (with single quotes):

```sql
CREATE TABLE categories(
    _id INTEGER NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    sort INTEGER NOT NULL,
    flags INTEGER NOT NULL,
    hidden INTEGER NOT NULL DEFAULT 0
);
INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, '', -1, 0);
CREATE TRIGGER IF NOT EXISTS system_category_delete_trigger BEFORE DELETE ON categories
BEGIN SELECT CASE WHEN old._id <= 0 THEN RAISE(ABORT, 'System category can''t be deleted') END; END;
```

in BOTH `manga.db` and `anime.db` (trigger name identical in both, as in the `.sq` files). The Rust repository maps a trigger abort (`rusqlite::Error::SqliteFailure` with `ErrorCode::ConstraintViolation`) to `CategoryError::SystemCategory` and the UI never enables delete for `id == 0`. When renumbering after delete, use the `ReorderMangaCategory` rule (exclude the system row) rather than Aniyomi's inconsistent one.

### 2.25 `update`

Nine statements, all partial updates built from `coalesce(:param, column)`: passing NULL for a parameter keeps the stored value, any non-NULL value overwrites. A column therefore can never be reset to NULL through these statements. Rust uses the SAME SQL with `Option<T>` parameters (`None` binds NULL), which reproduces the contract exactly; the structs below are the "update" DTOs (every field `Option<_>`, plus the id).

Common notes for all nine: the affected row is selected by the primary key; zero matching rows is not an error (0 rows changed). Kotlin wraps one-or-many updates in a single transaction (`handler.await(inTransaction = true)`), logs exceptions and returns a Boolean (manga/anime) or swallows (chapters/episodes: the repository methods do not catch, the interactors `UpdateChapter` / `UpdateEpisode`, `domain/src/main/java/tachiyomi/domain/items/chapter/interactor/UpdateChapter.kt`, catch, log and return nothing).

#### 2.25.1 `mangas` update (`mangas.sq:156-179`)

```sql
update:
UPDATE mangas SET
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
    chapter_flags = coalesce(:chapterFlags, chapter_flags),
    cover_last_modified = coalesce(:coverLastModified, cover_last_modified),
    date_added = coalesce(:dateAdded, date_added),
    update_strategy = coalesce(:updateStrategy, update_strategy),
    calculate_interval = coalesce(:calculateInterval, calculate_interval),
    version = coalesce(:version, version),
    is_syncing = coalesce(:isSyncing, is_syncing)
WHERE _id = :mangaId;
```

* Parameters (21 + id): `source Long?, url String?, artist, author, description String?, genre String? (already ", "-joined by the caller), title String?, status Long?, thumbnailUrl String?, favorite Boolean?, lastUpdate Long?, nextUpdate Long?, initialized Boolean?, viewer Long? (named `:viewer` here, `:viewerFlags` in `insert`), chapterFlags Long?, coverLastModified Long?, dateAdded Long?, updateStrategy Long? (ordinal), calculateInterval Long?, version Long?, isSyncing Long?, mangaId Long`. Columns `last_modified_at` and `favorite_modified_at` are never written here (triggers only).
* Callers: `MangaRepositoryImpl.partialUpdateManga` (`MangaRepositoryImpl.kt:148-177`; always binds `isSyncing = 0`; `genre = value.genre?.let(StringListColumnAdapter::encode)`; `calculateInterval = value.fetchInterval?.toLong()`), used by `updateManga` / `updateAllManga` (`:128-146`, return `false` on exception) and so by every metadata refresh, favorite toggle, flag change, fetch-interval update. `MangaRestorer.updateManga` (`MangaRestorer.kt:106-135`) for restoring over an existing entry: passes `nextUpdate = null`, `calculateInterval = null`, `isSyncing = 1`, `version = manga.version`.
* Trigger effects: 0.5. Consequence worth knowing: `favorite_modified_at` is stamped by every call of this statement, not only when favorite changes (0.4 item 2), and `version` bumps only on a real change of `url`, `description` (non-NULL to non-NULL) or `favorite`.
* Invalidated: `mangas`.

Proposal:

```rust
#[derive(Default)] pub struct MangaUpdate { pub id: i64, pub source: Option<i64>, pub url: Option<String>, pub artist: Option<String>,
  pub author: Option<String>, pub description: Option<String>, pub genre: Option<Vec<String>>, pub title: Option<String>,
  pub status: Option<i64>, pub thumbnail_url: Option<String>, pub favorite: Option<bool>, pub last_update: Option<i64>,
  pub next_update: Option<i64>, pub initialized: Option<bool>, pub viewer_flags: Option<i64>, pub chapter_flags: Option<i64>,
  pub cover_last_modified: Option<i64>, pub date_added: Option<i64>, pub update_strategy: Option<UpdateStrategy>,
  pub fetch_interval: Option<i32>, pub version: Option<i64> }
pub fn update_manga(tx:&Transaction, u:&MangaUpdate) -> rusqlite::Result<usize>; // binds is_syncing = Some(0)
pub fn update_manga_restoring(tx:&Transaction, u:&MangaUpdate) -> rusqlite::Result<usize>; // binds is_syncing = Some(1), next_update/calculate_interval None
```

#### 2.25.2 `chapters` update (`chapters.sq:103-118`)

```sql
update:
UPDATE chapters
SET manga_id = coalesce(:mangaId, manga_id),
    url = coalesce(:url, url),
    name = coalesce(:name, name),
    scanlator = coalesce(:scanlator, scanlator),
    read = coalesce(:read, read),
    bookmark = coalesce(:bookmark, bookmark),
    last_page_read = coalesce(:lastPageRead, last_page_read),
    chapter_number = coalesce(:chapterNumber, chapter_number),
    source_order = coalesce(:sourceOrder, source_order),
    date_fetch = coalesce(:dateFetch, date_fetch),
    date_upload = coalesce(:dateUpload, date_upload),
    version = coalesce(:version, version),
    is_syncing = coalesce(:isSyncing, is_syncing)
WHERE _id = :chapterId;
```

* Table `chapters` (`chapters.sq:3-21`): `_id` PK, `manga_id` FK cascade, `url`, `name`, `scanlator` NULL, `read` bool, `bookmark` bool, `last_page_read INTEGER NOT NULL` (page index; `0` = not started), `chapter_number REAL`, `source_order INTEGER` (position in the source list, 0 = newest first), `date_fetch`, `date_upload` (ms), `last_modified_at` (s, trigger), `version`, `is_syncing`. Added by migrations: `source_order` 1, `bookmark` 3, `scanlator` 4, last_modified_at 26, version/is_syncing 30. Indexes `chapters_manga_id_index`, partial `chapters_unread_by_manga_index ON chapters(manga_id, read) WHERE read = 0` (7.sqm).
* Parameters: `mangaId, url, name, scanlator, read, bookmark, lastPageRead, chapterNumber, sourceOrder, dateFetch, dateUpload, version, isSyncing` all nullable, then `chapterId`. A NULL scanlator cannot be written back through `coalesce`: to change a scanlator to "none" the code must delete and re-insert (never done).
* Callers: `ChapterRepositoryImpl.partialUpdate` (`ChapterRepositoryImpl.kt:52-73`, `isSyncing = 0` always) <- `UpdateChapter` (read marks, bookmarks, `last_page_read` saves from the reader, sync with source), `MangaRestorer.updateExistingChapters` (`MangaRestorer.kt:213-233`: `read`, `bookmark`, `lastPageRead`, `version`, `isSyncing = 0`, all else NULL).
* Triggers: `update_last_modified_at_chapters` always; `update_chapter_and_manga_version` when `read`, `bookmark` or `last_page_read` changed value: chapter `version + 1` and parent manga `version + 1` (unless the manga is_syncing). Note the restorer writes `version = chapter.version` and the trigger then adds 1 on top.
* Invalidated: `chapters`; the library view depends on it, so every page-progress save re-runs the library query (Aniyomi saves `last_page_read` only on chapter close / chapter change / pause, not per page).

Proposal: `ChapterUpdate` DTO analogous; `pub fn update_chapter(tx:&Transaction, u:&ChapterUpdate) -> rusqlite::Result<usize>`; `pub fn update_chapters(conn:&mut Connection, us:&[ChapterUpdate])` runs them in one transaction.

#### 2.25.3 `manga_sync` update (`manga_sync.sq:45-62`)

```sql
update:
UPDATE manga_sync
SET
    manga_id = coalesce(:mangaId, manga_id),
    sync_id = coalesce(:syncId, sync_id),
    remote_id = coalesce(:mediaId, remote_id),
    library_id = coalesce(:libraryId, library_id),
    title = coalesce(:title, title),
    last_chapter_read = coalesce(:lastChapterRead, last_chapter_read),
    total_chapters = coalesce(:totalChapter, total_chapters),
    status = coalesce(:status, status),
    score = coalesce(:score, score),
    remote_url = coalesce(:trackingUrl, remote_url),
    start_date = coalesce(:startDate, start_date),
    finish_date = coalesce(:finishDate, finish_date),
    private = coalesce(:private, private)
WHERE _id = :id;
```

* Parameter names differ from column names: `:mediaId` -> `remote_id`, `:totalChapter` -> `total_chapters`, `:trackingUrl` -> `remote_url`. Positional call order in Kotlin: `mangaId, syncId, mediaId, libraryId, title, lastChapterRead, totalChapter, status, score, trackingUrl, startDate, finishDate, private, id`.
* Only caller: `MangaRestorer.restoreTracking` (`MangaRestorer.kt:392-409`) for tracks that already exist locally (matched by tracker id; `lastChapterRead = max(db, backup)`, other fields from the backup copy `dbTrack.copy(remoteId, libraryId, lastChapterRead)`). Normal tracker syncing does NOT use `update`: it re-runs `insert` and relies on `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`.
* Edge: updating `manga_id`/`sync_id` onto an existing pair deletes the other row (REPLACE). Updates cannot clear `library_id` to NULL.

Proposal: `pub fn update_track(tx:&Transaction, u:&MangaTrackUpdate) -> rusqlite::Result<usize>` with the SQL verbatim and parameter names `:mediaId`, `:totalChapter`, `:trackingUrl`. `anime_sync.update` (`anime_sync.sq:46-63`) is identical with `anime_id`, `last_episode_seen` (`:lastEpisodeSeen`), `total_episodes` (`:totalEpisodes`); caller `AnimeRestorer.kt:433`.

#### 2.25.4 `anime_sync` update (`anime_sync.sq:46-63`)

```sql
update:
UPDATE anime_sync
SET
    anime_id = coalesce(:animeId, anime_id),
    sync_id = coalesce(:syncId, sync_id),
    remote_id = coalesce(:mediaId, remote_id),
    library_id = coalesce(:libraryId, library_id),
    title = coalesce(:title, title),
    last_episode_seen = coalesce(:lastEpisodeSeen, last_episode_seen),
    total_episodes = coalesce(:totalEpisodes, total_episodes),
    status = coalesce(:status, status),
    score = coalesce(:score, score),
    remote_url = coalesce(:trackingUrl, remote_url),
    start_date = coalesce(:startDate, start_date),
    finish_date = coalesce(:finishDate, finish_date),
    private = coalesce(:private, private)
WHERE _id = :id;
```

Same contract as 2.25.3; caller `AnimeRestorer.restoreTracking` (`AnimeRestorer.kt:400-450`). Trackers 101 (Simkl) and 102 (Jellyfin) exist only here.

#### 2.25.5 `animes` update (`animes.sq:170-201`)

```sql
update:
UPDATE animes SET
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
    episode_flags = coalesce(:episodeFlags, episode_flags),
    cover_last_modified = coalesce(:coverLastModified, cover_last_modified),
    date_added = coalesce(:dateAdded, date_added),
    update_strategy = coalesce(:updateStrategy, update_strategy),
    calculate_interval = coalesce(:calculateInterval, calculate_interval),
    version = coalesce(:version, version),
    is_syncing = coalesce(:isSyncing, is_syncing),
    fetch_type = coalesce(:fetchType, fetch_type),
    parent_id = coalesce(:parentId, parent_id),
    season_flags = coalesce(:seasonFlags, season_flags),
    season_number = coalesce(:seasonNumber, season_number),
    season_source_order = coalesce(:seasonSourceOrder, season_source_order),
    background_url = coalesce(:backgroundUrl, background_url),
    background_last_modified = coalesce(:backgroundLastModified, background_last_modified),
    memo = coalesce(:memo, memo)
WHERE _id = :animeId;
```

* Same contract as 2.25.1 plus: `fetch_type` (ordinal), `parent_id` (cannot be cleared here, use `removeParentIdByIds`), `season_flags`, `season_number REAL`, `season_source_order`, `background_url`, `background_last_modified`, `memo` (BLOB of JSON text; `memo = value.memo?.let(MemoColumnAdapter::encode)`).
* Callers: `AnimeRepositoryImpl.partialUpdateAnime` (`AnimeRepositoryImpl.kt:188-225`, `isSyncing = 0`), `AnimeRestorer.updateAnime` (`AnimeRestorer.kt:123-150`, `isSyncing = 1`).
* Mapper quirk: `AnimeMapper.mapAnime` maps the column `episode_flags` into a parameter called `chapterFlags` (`AnimeMapper.kt:254,281`) and then into `Anime.episodeFlags`. Rust maps by name `episode_flags`.
* Triggers: `update_last_modified_at_animes`, `update_last_favorite_at_animes`, `update_anime_version` (same as manga).

Proposal: `AnimeUpdate` DTO mirroring `MangaUpdate` plus the extra fields; `pub fn update_anime(tx, &AnimeUpdate)`.

#### 2.25.6 `episodes` update (`episodes.sq:98-118`)

```sql
update:
UPDATE episodes
SET anime_id = coalesce(:animeId, anime_id),
    url = coalesce(:url, url),
    name = coalesce(:name, name),
    scanlator = coalesce(:scanlator, scanlator),
    seen = coalesce(:seen, seen),
    bookmark = coalesce(:bookmark, bookmark),
    last_second_seen = coalesce(:lastSecondSeen, last_second_seen),
    total_seconds = coalesce(:totalSeconds, total_seconds),
    episode_number = coalesce(:episodeNumber, episode_number),
    source_order = coalesce(:sourceOrder, source_order),
    date_fetch = coalesce(:dateFetch, date_fetch),
    date_upload = coalesce(:dateUpload, date_upload),
    version = coalesce(:version, version),
    is_syncing = coalesce(:isSyncing, is_syncing),
    summary = coalesce(:summary, summary),
    preview_url = coalesce(:previewUrl, preview_url),
    fillermark = coalesce(:fillermark, fillermark),
    memo = coalesce(:memo, memo)
WHERE _id = :episodeId;
```

* Table `episodes` (`episodes.sq:4-27`): `_id`, `anime_id` FK cascade, `url`, `name`, `scanlator` NULL, `seen`, `bookmark`, `last_second_seen INTEGER` (resume position in SECONDS), `total_seconds INTEGER` (duration in seconds, 0 unknown), `episode_number REAL`, `source_order`, `date_fetch`, `date_upload`, `last_modified_at`, `version`, `is_syncing`, `summary TEXT NULL`, `preview_url TEXT NULL`, `fillermark` bool (added `134.sqm`), `memo` BLOB (136.sqm). Indexes `episodes_anime_id_index`, partial `episodes_unseen_by_anime_index ON episodes(anime_id, seen) WHERE seen = 0`.
* Callers: `EpisodeRepositoryImpl.partialUpdate` (`EpisodeRepositoryImpl.kt:133-159`, `isSyncing = 0`) <- `UpdateEpisode` (player progress, mark seen, bookmark, fillermark), `AnimeRestorer.updateExistingEpisodes` (`AnimeRestorer.kt:243-267`).
* Triggers: `update_last_modified_at_episodes`; `update_episode_and_anime_version` fires only for `seen`, `bookmark`, `last_second_seen` changes (NOT for `fillermark`, `total_seconds`, `summary`).

Proposal: `EpisodeUpdate` DTO; `pub fn update_episode(tx:&Transaction, &EpisodeUpdate)`; `memo` written with `serde_json::to_vec`.

#### 2.25.7 `categories` update, manga DB (`categories.sq:79-85`) and anime DB (`dataanime/categories.sq:79-85`)

```sql
update:
UPDATE categories
SET name = coalesce(:name, name),
    sort = coalesce(:order, sort),
    flags = coalesce(:flags, flags),
    hidden = coalesce(:hidden, hidden)
WHERE _id = :categoryId;
```

* Parameters: `name String?`, `order Long?` (column `sort`), `flags Long?`, `hidden Long?` (0/1; Kotlin converts `Boolean?` with `if (it) 1L else 0L`), `categoryId Long`. The parameter is literally named `order`; in SQL a named parameter `:order` is legal.
* Callers: `MangaCategoryRepositoryImpl.updatePartialBlocking` (`MangaCategoryRepositoryImpl.kt:82-90`) via `updatePartialMangaCategory` (single, own transaction-less call) and `updatePartialMangaCategories` (one transaction for many): rename (`RenameMangaCategory`), reorder (`ReorderMangaCategory`), hide/show (`HideMangaCategory`), per-category sort (`SetSortModeForMangaCategory` when categorized display settings are on), renumber after delete (`DeleteMangaCategory`). Anime: `AnimeCategoryRepositoryImpl.kt:82-90`.
* No triggers on `categories` other than the delete guard. The `hidden` column: hidden categories are excluded by `getVisibleCategories` and the visible-by-manga variant.
* Invalidated: `categories`.

Proposal: `CategoryUpdate { id, name: Option<String>, order: Option<i64>, flags: Option<i64>, hidden: Option<bool> }`; `pub fn update_category(tx, &CategoryUpdate)`, `pub fn update_categories(conn:&mut Connection, &[CategoryUpdate])` (one transaction), in both DB crates.

#### 2.25.8 `custom_buttons` update (`custom_buttons.sq:45-53`, anime DB only)

```sql
update:
UPDATE custom_buttons
SET name = coalesce(:name, name),
    isFavorite = coalesce(:isFavorite, isFavorite),
    sortIndex = coalesce(:sortIndex, sortIndex),
    content = coalesce(:content, content),
    longPressContent = coalesce(:longPressContent, longPressContent),
    onStartup = coalesce(:onStartup, onStartup)
WHERE _id = :customButtonId;
```

* Table (`custom_buttons.sq:3-11`, `129.sqm`): `_id INTEGER PK`, `name TEXT`, `isFavorite INTEGER AS Boolean`, `sortIndex INTEGER`, `content TEXT`, `longPressContent TEXT`, `onStartup TEXT` (the three Lua snippets of a user-defined player button), plus a seeded row `_id = 1`, `'+85 s'`, favorite, sort 0, containing the skip-intro Lua scripts (verbatim in `custom_buttons.sq:13-30`; player scripting design in area:48-settings-player-and-scripts).
* Callers: `CustomButtonRepositoryImpl.updatePartialBlocking` (`CustomButtonRepositoryImpl.kt:61-71`), `updatePartialCustomButton` (single) and `updatePartialCustomButtons` (one transaction), used by the custom-button editor and the reorder/favorite toggles. `insertCustomButton` throws `SaveCustomButtonException` on `SQLiteException`.
* Invalidated: `custom_buttons`.

Proposal: `CustomButtonUpdate` DTO and `update_custom_button`; seed row inserted with `INSERT OR IGNORE` at schema creation.

### 2.26 `updateAllFlags`

Two variants, identical text. `data/src/main/sqldelight/data/categories.sq:87-89` and `data/src/main/sqldelightanime/dataanime/categories.sq:87-89`.

```sql
updateAllFlags:
UPDATE categories SET
flags = coalesce(?, flags);
```

* One POSITIONAL parameter `?` (`Long?`). `NULL` leaves every row unchanged. Otherwise sets `flags` of EVERY category row, including the system row `_id = 0`. No `WHERE`.
* Callers: `MangaCategoryRepositoryImpl.updateAllMangaCategoryFlags(flags)` (`MangaCategoryRepositoryImpl.kt:92-96`) <- `SetSortModeForMangaCategory.await` (`domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetSortModeForMangaCategory.kt:14-37`) for the global sort when `categorizedDisplaySettings` is off or no category is selected: `flags = (category?.flags ?: 0) + type + direction` where `Long + Flag` clears the flag's mask bits then ORs the flag value (`domain/src/main/java/tachiyomi/domain/library/model/Flag.kt`: `this and other.mask.inv() or (other.flag and other.mask)`); this also sets the preference `mangaSortingMode` and, for `Random`, a new `randomMangaSortSeed = Random.nextInt()`. `ResetMangaCategoryFlags.await` (`ResetMangaCategoryFlags.kt:13-15`) writes `sort.type + sort.direction` (all other bits cleared) from the preference, used when "per-category display settings" is switched off. Anime: `AnimeCategoryRepositoryImpl.updateAllAnimeCategoryFlags` (`:92-96`).
* Invalidated: `categories`.

Proposal: `pub fn update_all_category_flags(conn:&Connection, flags:Option<i64>) -> rusqlite::Result<usize>` with `UPDATE categories SET flags = coalesce(?1, flags)`; a `Flag` helper `fn with_flag(current: i64, flag: i64, mask: i64) -> i64 { (current & !mask) | (flag & mask) }` shared by the sort and display code.

### 2.27 `update_anime_version`

Trigger on `animes`. `data/src/main/sqldelightanime/dataanime/animes.sq:65-73`; created by `127.sqm` (dropped and recreated there).

```sql
CREATE TRIGGER update_anime_version AFTER UPDATE ON animes
BEGIN
    UPDATE animes SET version = version + 1
    WHERE _id = new._id AND new.is_syncing = 0 AND (
        new.url != old.url OR
        new.description != old.description OR
        new.favorite != old.favorite
    );
END;
```

Identical in logic to `update_manga_version` (2.36): bumps the anime's `version` by one when `is_syncing = 0` and `url`, `description` or `favorite` changed (NULL-propagating comparisons: a NULL<->text description change is NOT counted; two non-NULL different values are). Does not look at `fetch_type`, `parent_id`, `memo` or flags. Fires on every `UPDATE animes`, including those produced by `removeParentIdByIds`, `resetViewerFlags` (no bump there). Proposal: create verbatim in `anime.db`.

### 2.28 `update_chapter_and_manga_version`

Trigger on `chapters`. `chapters.sq:35-49`; created by `30.sqm` (drop + create).

```sql
CREATE TRIGGER update_chapter_and_manga_version AFTER UPDATE ON chapters
WHEN new.is_syncing = 0 AND (
    new.read != old.read OR
    new.bookmark != old.bookmark OR
    new.last_page_read != old.last_page_read
)
BEGIN
    -- Update the chapter version
    UPDATE chapters SET version = version + 1
    WHERE _id = new._id;

    -- Update the manga version
    UPDATE mangas SET version = version + 1
    WHERE _id = new.manga_id AND (SELECT is_syncing FROM mangas WHERE _id = new.manga_id) = 0;
END;
```

* The `WHEN` guard is evaluated per updated row: only user-visible progress changes count (read flag, bookmark, page), and only when the chapter itself is not `is_syncing`. Then the chapter counter and, unless the parent manga is `is_syncing = 1`, the manga counter go up by one. `is_syncing` of the chapter is checked in the `WHEN`, that of the manga in the second statement.
* Nested trigger effects: the inner `UPDATE chapters` re-fires `update_last_modified_at_chapters`; the inner `UPDATE mangas` fires `update_last_modified_at_mangas` (manga `last_modified_at` = now seconds). So marking a chapter read touches the manga row: its `last_modified_at` and `version` change. `update_manga_version` also fires but its condition is false.
* Counter use: restore conflict resolution (`MangaRestorer.restoreExistingManga`, `MangaRestorer.kt:96-103`: higher `version` wins), backup field `version` (`BackupManga.kt:42` ProtoNumber 109, `BackupChapter`), sync (area:21-backup-format-and-create).
* Proposal: create verbatim. Test: update a chapter's `read` 0->1 twice (second is no change) and assert chapter and manga `version` each +1 only once.

### 2.29 `update_episode_and_anime_version`

Trigger on `episodes`. `episodes.sq:41-55`; `127.sqm`.

```sql
CREATE TRIGGER update_episode_and_anime_version AFTER UPDATE ON episodes
WHEN new.is_syncing = 0 AND (
    new.seen != old.seen OR
    new.bookmark != old.bookmark OR
    new.last_second_seen != old.last_second_seen
)
BEGIN
    -- Update the episode version
    UPDATE episodes SET version = version + 1
    WHERE _id = new._id;

    -- Update the anime version
    UPDATE animes SET version = version + 1
    WHERE _id = new.anime_id AND (SELECT is_syncing FROM animes WHERE _id = new.anime_id) = 0;
END;
```

Anime twin of 2.28 with `seen`, `bookmark`, `last_second_seen` (resume position seconds). `fillermark` and `total_seconds` changes do not bump. Because `last_second_seen` is saved periodically while playing, `episodes.version` and `animes.version` can grow quickly during playback; the player saves position on pause/seek/exit and every autosave tick (area:45-player-core) so expect tens of bumps per episode. Proposal: create verbatim in `anime.db`.

### 2.30 `update_last_favorite_at_animes`

Trigger on `animes`. `animes.sq:48-54`; fresh name. Upgraded DBs: `update_last_favorited_at_animes` (`126.sqm`).

```sql
CREATE TRIGGER update_last_favorite_at_animes
AFTER UPDATE OF favorite ON animes
BEGIN
  UPDATE animes
  SET favorite_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;
```

Same as 2.31. Proposal: see 2.31 (the same `WHEN new.favorite != old.favorite` refinement), created in `anime.db`.

### 2.31 `update_last_favorite_at_mangas`

Trigger on `mangas`. `mangas.sq:36-42`; created by `26.sqm` under the name `update_last_favorited_at_mangas`.

```sql
CREATE TRIGGER update_last_favorite_at_mangas
AFTER UPDATE OF favorite ON mangas
BEGIN
  UPDATE mangas
  SET favorite_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;
```

* Writes `favorite_modified_at` = current time in SECONDS (as text converted to integer) for the updated row. The column is NULL until the first such update; migration `26.sqm` back-filled it for current favorites only (`UPDATE mangas SET favorite_modified_at = strftime('%s', 'now') WHERE favorite = 1`).
* OBSERVED quirk: `UPDATE OF favorite` fires when `favorite` is listed in the SET clause, regardless of value change; the generic `update` statement always lists it (`favorite = coalesce(:favorite, favorite)`), so every call of that statement stamps `favorite_modified_at`, including updates that do not touch the favorite flag (last-update time, cover refresh, fetch interval). Only statements without `favorite` in SET (`resetViewerFlags`, history/chapter work) leave it alone. The nested `UPDATE` stamps `last_modified_at` too through `update_last_modified_at_mangas`.
* Readers: only the backup export (`MangaBackupCreator.kt:100`, ProtoNumber 107 `favoriteModifiedAt`) and `getMangasWithFavoriteTimestamp` (`WHERE favorite_modified_at IS NOT NULL`), which no Kotlin code calls.
* Proposal (documented deviation): create

```sql
CREATE TRIGGER update_last_favorite_at_mangas
AFTER UPDATE OF favorite ON mangas
WHEN new.favorite IS NOT old.favorite
BEGIN
  UPDATE mangas SET favorite_modified_at = strftime('%s', 'now') WHERE _id = new._id;
END;
```

so the column means what its name says. No reader depends on the sloppy behaviour; exported backups stay valid. Same for `animes`.

### 2.32 `update_last_modified_at_animes`

Trigger on `animes`. `animes.sq:56-63`; `126.sqm`.

```sql
CREATE TRIGGER update_last_modified_at_animes
AFTER UPDATE ON animes
FOR EACH ROW
BEGIN
  UPDATE animes
  SET last_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;
```

Stamps `last_modified_at` (seconds) on every update of the row (including updates made by other triggers). With `recursive_triggers` OFF it does not re-fire itself. `FOR EACH ROW` is redundant in SQLite. Exported in backups as `lastModifiedAt` (`BackupAnime.kt:41`, ProtoNumber 106) and used by `AnimeRestorer.sortByNew` (`AnimeRestorer.kt:54`) to restore the most recently modified entries first among those that are new to the DB. Proposal: create verbatim; a future "touch" must not be done in application code.

### 2.33 `update_last_modified_at_chapters`

Trigger on `chapters`. `chapters.sq:26-33`; `26.sqm`.

```sql
CREATE TRIGGER update_last_modified_at_chapters
AFTER UPDATE ON chapters
FOR EACH ROW
BEGIN
  UPDATE chapters
  SET last_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;
```

Stamps seconds on every chapter update. Exported as `BackupChapter.lastModifiedAt` (ProtoNumber 11). The restore comparison ignores it (`MangaRestorer.kt:190`, `forComparison()` zeroes `lastModifiedAt`, `version`, `dateFetch`, `dateUpload`, `id`, `mangaId`). Note: a chapter INSERT does not stamp it; new chapters keep `last_modified_at = 0` because `insert` writes literal `0` (`chapters.sq:99-101`). Proposal: verbatim.

### 2.34 `update_last_modified_at_episodes`

Trigger on `episodes`. `episodes.sq:32-39`; `126.sqm`.

```sql
CREATE TRIGGER update_last_modified_at_episodes
AFTER UPDATE ON episodes
FOR EACH ROW
BEGIN
  UPDATE episodes
  SET last_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;
```

Anime twin of 2.33 (`BackupEpisode.lastModifiedAt`, ProtoNumber 11; `EpisodeRepositoryImpl.mapEpisode` reads it). Proposal: verbatim.

### 2.35 `update_last_modified_at_mangas`

Trigger on `mangas`. `mangas.sq:44-51`; `26.sqm`.

```sql
CREATE TRIGGER update_last_modified_at_mangas
AFTER UPDATE ON mangas
FOR EACH ROW
BEGIN
  UPDATE mangas
  SET last_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;
```

Stamps `last_modified_at` (seconds) on every update of a manga row, including updates issued by `update_manga_version`, `update_chapter_and_manga_version`, `insert_manga_category_update_version` and the favorite trigger. Used by backups (`MangaBackupCreator.kt:99`, ProtoNumber 106) and `MangaRestorer.sortByNew` (`MangaRestorer.kt:46-54`: entries not yet in the DB are restored in order of newest `lastModifiedAt` first, existing ones after). Insert does not stamp (`insert` writes literal `0`). Proposal: verbatim; unit test asserts that `last_modified_at` is within 2 seconds of `strftime('%s','now')` after any `UPDATE`.

### 2.36 `update_manga_version`

Trigger on `mangas`. `mangas.sq:53-61`; `30.sqm` (drop + create).

```sql
CREATE TRIGGER update_manga_version AFTER UPDATE ON mangas
BEGIN
    UPDATE mangas SET version = version + 1
    WHERE _id = new._id AND new.is_syncing = 0 AND (
        new.url != old.url OR
        new.description != old.description OR
        new.favorite != old.favorite
    );
END;
```

* Fires on every update of a `mangas` row; the inner `UPDATE` changes a row only when `new.is_syncing = 0` and one of `url`, `description`, `favorite` differs (all three compared with `!=`: NULL on either side gives NULL = no bump, so adding a description to a manga that had none does NOT bump).
* When it bumps, `update_last_modified_at_mangas` re-fires (stamp), `update_manga_version` itself does not re-enter.
* `version` is also bumped by `update_chapter_and_manga_version` and `insert_manga_category_update_version`; it never decreases; the update statement can overwrite it (`version = coalesce(:version, version)`; the restorer writes the backup's value and the trigger may add 1 afterwards if url/description/favorite differ).
* Proposal: create verbatim.

### 2.37 `updatesView`

View joining favorites' chapters, newest fetched first, plus two queries. `data/src/main/sqldelight/view/updatesView.sq:1-35`. History: `18.sqm` (created), `23.sqm` (added `chapters.last_page_read`). Anime twin `animeupdatesView` (`data/src/main/sqldelightanime/view/animeupdatesView.sq`; extra columns `fillermark`, `last_second_seen`, `totalSeconds`).

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

getUpdatesByReadStatus:
SELECT *
FROM updatesView
WHERE read = :read
AND dateUpload > :after

LIMIT :limit;
```

* Row filter: only chapters of favorite manga whose `date_fetch` is greater than the manga's `date_added`, i.e. chapters discovered AFTER the manga was added to the library (the initial chapter list of a newly added manga is not an "update"). The sync code keeps `date_fetch = now + remaining count` so chapters of one fetch sort in source order, and re-created chapters inherit the old `date_fetch` so they do not reappear (`SyncChaptersWithSource.kt:184-205`).
* Ordering: `ORDER BY date_fetch DESC` lives in the view; the outer queries have no `ORDER BY`, so SQLite flattens the view and applies the ordering before `LIMIT`. Ties on `date_fetch` have no defined order. Proposal: write the `ORDER BY chapters.date_fetch DESC, chapters._id DESC` explicitly in the Rust query to make ties deterministic.
* `getRecentUpdates(:after, :limit)`: `after` is epoch ms; the filter is on the UPLOAD date (`dateUpload > :after`), not the fetch date, so a chapter uploaded more than the window ago never shows even if it was fetched today (the sync sets `date_upload` to the fetch time for chapters whose source gives 0, `SyncChaptersWithSource.kt:107-114`). `limit` is 500 everywhere.
* `getUpdatesByReadStatus(:read, :after, :limit)`: the same plus `read = :read` (Boolean bound as 0/1).
* Mapper (`MangaUpdatesRepositoryImpl.kt:286-319`): column order is `mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, last_page_read, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch`; `MangaUpdatesWithRelations` carries `coverData = MangaCover(mangaId, sourceId, isMangaFavorite = favorite, url = thumbnailUrl, lastModified = coverLastModified)`. No exclusion of scanlators here (excluded scanlators still appear in Updates; OBSERVED, the view does not join `excluded_scanlators`).
* Callers: `MangaUpdatesScreenModel` (`app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt:75-78`): `getUpdates.subscribe(ZonedDateTime.now().minusMonths(3).toInstant())` = `subscribeAllMangaUpdates(after, 500)` -> `getRecentUpdates`, combined with download-cache changes and the download queue; anime `AnimeUpdatesScreenModel.kt:80-82`. The read-status variants (`awaitWithRead`, `subscribeWithRead`) are used only by the Android home-screen widgets: `presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt:24` and `BaseMangaUpdatesGridGlanceWidget.kt:96` (`subscribe(false, DateLimit.toEpochMilli())`, `DateLimit` = 3 months ago, lines 159-160), and the anime equivalents.
* Invalidated by: `mangas`, `chapters`.
* Perf: join + filter + sort over all chapters of favorites each emission. Aniyomi relies on the `chapters_manga_id_index`; the `date_fetch` sort is a temp b-tree over the filtered set (bounded by the date filter). Fine for tens of thousands of chapters.

Decisions: `getRecentUpdates` PORTED (Updates screen, P1). `getUpdatesByReadStatus` and the home-screen widget DROPPED (Android app widgets have no desktop equivalent; replacement: the Updates screen filter "unread" and an optional "N new chapters" desktop notification from the library-update job, area:26-library-update-jobs; the user loses a glanceable launcher widget).

Proposal:

```rust
pub const SQL_RECENT_UPDATES: &str = "SELECT * FROM updatesView WHERE dateUpload > :after ORDER BY datefetch DESC, chapterId DESC LIMIT :limit";
pub struct MangaUpdateRow { pub manga_id:i64, pub manga_title:String, pub chapter_id:i64, pub chapter_name:String,
  pub scanlator:Option<String>, pub read:bool, pub bookmark:bool, pub last_page_read:i64, pub source:i64,
  pub favorite:bool, pub thumbnail_url:Option<String>, pub cover_last_modified:i64, pub date_upload:i64, pub date_fetch:i64 }
pub fn recent_updates(conn:&Connection, after_ms:i64, limit:i64) -> rusqlite::Result<Vec<MangaUpdateRow>>; // limit 500, after = now - 3 calendar months
```

"3 calendar months" = local-time `now.minus_months(3)` (chrono `checked_sub_months(Months::new(3))`), as in `ZonedDateTime.now().minusMonths(3)`. The anime view is created identically with `animes`/`episodes`/`seen` (`animeupdatesView.sq`).

### 2.38 `upsert`

Six statements. In every one the `WHERE` after `DO UPDATE SET` is a tautology on the conflict key.

#### 2.38.1 manga `sources.upsert` (`sources.sq:16-24`)

```sql
upsert:
INSERT INTO sources(_id, lang, name)
VALUES (:id, :lang, :name)
ON CONFLICT(_id)
DO UPDATE
SET
    lang = :lang,
    name = :name
WHERE _id = :id;
```

Inserts a stub source or overwrites its language and name. Caller: `MangaStubSourceRepositoryImpl.upsertStubMangaSource` (`data/src/main/java/tachiyomi/data/source/manga/MangaStubSourceRepositoryImpl.kt`) <- `AndroidMangaSourceManager.registerStubSource` (2.23). No error handling; failure would throw inside the launched coroutine. Anime twin `animesources.upsert` (`animesources.sq:16-24`, same text with table `animesources`) <- `AndroidAnimeSourceManager.kt:110`.

#### 2.38.2 `extension_repos.upsert` (`extension_repos.sq:31-41`, manga DB)

```sql
upsert:
INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint)
VALUES (:base_url, :name, :short_name, :website, :fingerprint)
ON CONFLICT(base_url)
DO UPDATE
SET
    name = :name,
    short_name = :short_name,
    website =: website,
    signing_key_fingerprint = :fingerprint
WHERE base_url = base_url;
```

Inserts a repo, or refreshes name, short name, website and fingerprint of the row with the same `base_url`. A fingerprint equal to ANOTHER row's raises `UNIQUE constraint failed: extension_repos.signing_key_fingerprint`, which `MangaExtensionRepoRepositoryImpl.upsertRepo` (`MangaExtensionRepoRepositoryImpl.kt:465-477`) converts into `SaveExtensionRepoException` (any `SQLiteException`). `website =: website` parses as `website = :website`. Callers: `UpdateMangaExtensionRepo.await` (`UpdateMangaExtensionRepo.kt`: refetch `repo.json`; the upsert runs only if the stored fingerprint starts with `NOFINGERPRINT` or equals the fetched one, so an unexpected key change is never applied silently), invoked for all repos from the repos screen refresh (`refreshRepos`) and by `MangaExtensionApi` (`app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt:34,81`, `updateExtensionRepo.awaitAll()`); `TrustExtensionRepositoryMigration` (`app/src/main/java/mihon/core/migration/migrations/TrustExtensionRepositoryMigration.kt:32-47`): turns each legacy preference URL into `(base_url = url, name = "Repo #<n>", short_name = null, website = url, fingerprint = "NOFINGERPRINT-<n>")` with `n` starting at 1.

#### 2.38.3 `history.upsert` (`history.sq:61-69`, manga DB)

```sql
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

* Parameters: `chapterId Long`, `readAt Date?` (epoch ms via `DateColumnAdapter`), `time_read Long` (ms of THIS session; the same value is bound twice).
* Table `history` (`history.sq:3-12`): `_id` PK, `chapter_id INTEGER NOT NULL UNIQUE` FK `chapters(_id)` ON DELETE CASCADE, `last_read INTEGER AS Date` (nullable on fresh installs, NOT NULL upgraded), `time_read INTEGER NOT NULL`; index `history_history_chapter_id_index` (redundant with the UNIQUE autoindex; `2.sqm`, rebuilt `15.sqm`).
* Semantics: first read of a chapter inserts `(readAt, session)`; every later call overwrites `last_read` (even with an OLDER date) and ADDS the session time to the total. A row that was hidden with `resetHistoryById` (`last_read = 0`) is revived by the next call.
* Callers: `MangaHistoryRepositoryImpl.upsertMangaHistory` (`MangaHistoryRepositoryImpl.kt:232-244`, logs and swallows exceptions) <- `UpsertMangaHistory` <- `ReaderViewModel.updateHistory` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:590-603`, function at 594): not in incognito mode; `readAt = Date()`; `sessionReadDuration = readAt.time - chapterReadStartTime` (0 if the timer was not running); afterwards `chapterReadStartTime = null` (timer restarted by `restartReadTimer`, lines 579-581, when the reader resumes; `flushReadTimer`, lines 583-589, saves on pause/exit). Restore: `MangaRestorer.restoreHistory` (`MangaRestorer.kt:322-364`): for an existing row binds `readAt = max(backup, db)` (NULL when both are 0) and `time = max(backup.time, db.time) - db.time`, so restoring never double counts; for a chapter not in the DB nothing is inserted.
* Edge: binding `readAt = NULL` (restore with both timestamps 0) works on fresh DBs and fails with a NOT NULL constraint on upgraded ones.
* Invalidated: `history`.

#### 2.38.4 `animehistory.upsert` (`animehistory.sq:58-65`, anime DB)

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

Anime history has no duration column (`animehistory`: `_id`, `episode_id INTEGER NOT NULL UNIQUE` FK cascade, `last_seen INTEGER AS Date`). Callers: `AnimeHistoryRepositoryImpl.upsertAnimeHistory` (`AnimeHistoryRepositoryImpl.kt:63-74`, logs/swallows) <- `PlayerViewModel.saveEpisodeHistory` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1792-1800`, called at line 1766, skipped in incognito, `seenAt = Date()`), `ExternalIntents.saveEpisodeHistory` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt:441-446`, called at line 417 when the external player returns), `AnimeRestorer.restoreHistory` (`AnimeRestorer.kt:362-395`).

#### 2.38.5 `extension_store.upsert` (`extension_store.sq:27-40`, anime DB)

```sql
upsert:
INSERT INTO extension_store(index_url, name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url)
VALUES (:indexUrl, :name, :badgeLabel, :signingKey, :contactWebsite, :contactDiscord, :isLegacy, :extensionListUrl)
ON CONFLICT(index_url)
DO UPDATE
SET
    name = :name,
    badge_label = :badgeLabel,
    signing_key =: signingKey,
    contact_website = :contactWebsite,
    contact_discord = :contactDiscord,
    is_legacy = :isLegacy,
    extension_list_url = :extensionListUrl
WHERE index_url = :indexUrl;
```

Table (`extension_store.sq:3-12`, created by `137.sqm`, which also copied every old `extension_repos` row into it as `index_url = base_url || '/repo.json'`, `badge_label = coalesce(short_name, name)`, `signing_key = signing_key_fingerprint`, `contact_website = website`, `contact_discord = NULL`, `is_legacy = 1`, `extension_list_url = NULL`, then dropped `extension_repos`): `index_url TEXT PK`, `name`, `badge_label`, `signing_key`, `contact_website`, `contact_discord TEXT NULL`, `is_legacy INTEGER NOT NULL`, `extension_list_url TEXT NULL`. Callers (`mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt`): `insert(indexUrl)` fetches the store index and upserts (`:19-21,54-67`); `insertFromPreference(indexUrl, name)` upserts with `badge_label = name`, `signing_key = "NO_SIGNING_KEY"`, `contact_website = indexUrl`, `contact_discord = NULL`, `is_legacy = false`, `extension_list_url = NULL` (`:23-36`); `refreshAll()` upserts each fetched store, logging failures per store (`:38-52`). Readers: `get`, `getAll`, `getCount`, `delete(indexUrl)`.

#### 2.38.6 Decisions and Rust

| Variant | Decision |
|---|---|
| `sources.upsert`, `animesources.upsert` | PORTED (stub-source cache, 2.23) |
| `history.upsert`, `animehistory.upsert` | PORTED, P0 (reading/watching progress history) |
| `extension_repos.upsert`, `extension_store.upsert` | PORTED only if area:20-extension-strategy-redesign keeps a store/repo table; otherwise replaced by that area's table. Statement text is kept here so the engineer can copy it |

```rust
pub const SQL_HISTORY_UPSERT: &str = "INSERT INTO history(chapter_id, last_read, time_read) VALUES (:chapterId, :readAt, :timeRead) \
 ON CONFLICT(chapter_id) DO UPDATE SET last_read = :readAt, time_read = time_read + :timeRead";
/// `read_at_ms` None is bound as 0, never NULL; `session_ms` is clamped to >= 0.
pub fn upsert_history(conn:&Connection, chapter_id:i64, read_at_ms:i64, session_ms:i64) -> rusqlite::Result<()>;
pub const SQL_ANIME_HISTORY_UPSERT: &str = "INSERT INTO animehistory(episode_id, last_seen) VALUES (:episodeId, :seenAt) \
 ON CONFLICT(episode_id) DO UPDATE SET last_seen = :seenAt";
pub fn upsert_anime_history(conn:&Connection, episode_id:i64, seen_at_ms:i64) -> rusqlite::Result<()>;
pub fn upsert_extension_repo(conn:&Connection, r:&ExtensionRepo) -> rusqlite::Result<()>;   // `ON CONFLICT(base_url)`, tautology WHERE dropped, `= :website`
pub fn upsert_extension_store(conn:&Connection, s:&AnimeExtensionStore) -> rusqlite::Result<()>; // same shape
```

(The tautological `WHERE chapter_id = :chapterId` is dropped; SQLite 3.24+ upsert syntax is required and rusqlite's bundled SQLite satisfies it.) Reading time is clamped at `>= 0` because the clock difference can be negative after a wall-clock change (Aniyomi does not clamp). Decision for history rows with unread-but-opened chapters: the reader calls `upsert_history` on chapter open and on close, the player calls `upsert_anime_history` on open and on exit, so "continue" works after a crash.

## 3. Cross-cutting tables

### 3.1 Trigger matrix (all twelve trigger items plus the two helper triggers that exist only on upgraded DBs)

| Trigger | Table | Event | Condition | Effect on |
|---|---|---|---|---|
| `update_last_modified_at_mangas` | `mangas` | AFTER UPDATE | always | `mangas.last_modified_at` = now (s) |
| `update_last_favorite_at_mangas` | `mangas` | AFTER UPDATE OF favorite | when `favorite` in SET (Aniyomi); `new.favorite IS NOT old.favorite` (Proposal) | `mangas.favorite_modified_at` = now (s) |
| `update_manga_version` | `mangas` | AFTER UPDATE | `is_syncing = 0` and url/description/favorite changed (non-NULL) | `mangas.version` + 1 |
| `update_last_modified_at_chapters` | `chapters` | AFTER UPDATE | always | `chapters.last_modified_at` = now (s) |
| `update_chapter_and_manga_version` | `chapters` | AFTER UPDATE | chapter `is_syncing = 0` and read/bookmark/last_page_read changed | `chapters.version` + 1, `mangas.version` + 1 (if manga not syncing) |
| `insert_manga_category_update_version` | `mangas_categories` | AFTER INSERT | manga not syncing | `mangas.version` + 1 per row |
| `system_category_delete_trigger` | `categories` | BEFORE DELETE | `old._id <= 0` | aborts the statement |
| `update_last_modified_at_animes` | `animes` | AFTER UPDATE | always | `animes.last_modified_at` |
| `update_last_favorite_at_animes` | `animes` | AFTER UPDATE OF favorite | as manga | `animes.favorite_modified_at` |
| `update_anime_version` | `animes` | AFTER UPDATE | as manga | `animes.version` + 1 |
| `update_last_modified_at_episodes` | `episodes` | AFTER UPDATE | always | `episodes.last_modified_at` |
| `update_episode_and_anime_version` | `episodes` | AFTER UPDATE | episode not syncing and seen/bookmark/last_second_seen changed | `episodes.version` + 1, `animes.version` + 1 |
| `insert_anime_category_update_version` (not a catalog item here) | `animes_categories` | AFTER INSERT | anime not syncing | `animes.version` + 1 per row |
| `update_last_modified_at_mangas_categories`, `update_last_modified_at_animes_categories` (upgraded DBs only) | link tables | AFTER UPDATE | always | `last_modified_at` of the link row; never fires because link rows are never updated |

### 3.2 Reactive-query dependency table (what to invalidate)

| Query / view | Re-run when these tables change |
|---|---|
| `library` (`libraryView`) | `mangas`, `chapters`, `history`, `excluded_scanlators`, `mangas_categories` |
| `getRecentUpdates` (`updatesView`) | `mangas`, `chapters` |
| `history` list (`historyView`, other chunk) | `mangas`, `chapters`, `history` |
| source lists with counts | `mangas`, `sources` |
| category list | `categories` |
| tracks by manga | `manga_sync` |
| excluded scanlators | `excluded_scanlators` |

Cascades (FK `ON DELETE CASCADE`) also emit change events for the child tables (`chapters`, `history`, `mangas_categories`, `manga_sync`, `excluded_scanlators`) when a manga is deleted.

### 3.3 Dropped or deviating items (summary)

| Item | Decision | Why / what the user loses |
|---|---|---|
| `resetIsSyncing` (4 statements) | dropped | never called; flag cleared by `update`. Loses nothing |
| `selectLastInsertedRowId` (7 variants) | replaced by `last_insert_rowid()` API; 3 variants never used | no loss |
| `getUpdatesByReadStatus` + widgets | dropped | Android launcher widget; Updates screen covers it |
| `update_last_favorite_at_*` | ported with a `WHEN` refinement | `favorite_modified_at` now means "favorite changed at" |
| `resetViewerFlags` (anime) | ported and exposed in Advanced settings | gain: Aniyomi never exposed it for anime |
| history writes | ported, clamped >= 0 | none |
| `replace`, `upsert` of extension repos/stores | follow area:20-extension-strategy-redesign | repo table shape may differ |

## 4. Acceptance tests for the db crate (one `#[test]` each, in-memory SQLite with the fresh schema)

1. `foreign_keys`: deleting a manga removes its `chapters`, `history` (through chapters), `mangas_categories`, `manga_sync`, `excluded_scanlators`.
2. `system_category`: `DELETE FROM categories WHERE _id = 0` fails with a constraint error whose message contains `System category can't be deleted`; deleting a user category succeeds and removes its `mangas_categories` rows.
3. `triggers_modified_at`: after `UPDATE mangas SET title='x'`, `last_modified_at` is within 2 s of now and `favorite_modified_at` is NULL (Proposal trigger) / non-NULL (Aniyomi trigger when `favorite` listed in SET).
4. `version_bumps`: `favorite` 0->1 bumps `version` once; `description` NULL->'a' does not; 'a'->'b' does; `is_syncing = 1` suppresses; reading a chapter (`read` 0->1) bumps chapter and manga `version` once; inserting a manga-category link bumps once per row.
5. `library_view`: manga in two categories gives two rows; no category gives `category = 0`; an excluded scanlator reduces `totalCount` but a NULL scanlator chapter is still counted; `lastRead` is the max over history.
6. `updates_view`: chapters with `date_fetch <= manga.date_added` are absent; `dateUpload <= :after` is filtered; result is sorted by `date_fetch DESC`; limit honoured.
7. `history_upsert`: two calls with 1000 ms and 500 ms yield `time_read = 1500` and `last_read` of the second call; `resetHistoryById` sets `last_read = 0` and keeps `time_read`; `removeResettedHistory` removes it.
8. `partial_update_nulls`: an update with every `Option` as `None` changes nothing except `last_modified_at`; `is_syncing` returns to 0 after `update_manga`.
9. `in_chunks`: removing 1200 chapter ids works (3 chunks); removing an empty list is a no-op returning 0.
10. `update_hook`: updating a chapter emits `chapters` and (through the trigger) `mangas`; deleting a manga emits events for the cascaded child tables.
11. `memo_roundtrip` (anime): a row whose `memo` was written as TEXT `'{}'` and one written as BLOB `x'7b7d'` both read as an empty object.
12. `track_replace`: inserting a second `manga_sync` row with the same `(manga_id, sync_id)` leaves one row with the new values and a new `_id`.
