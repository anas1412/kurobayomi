# SQL catalog, chunk 1 of 4

Source of the item list: `python3 plans/_tools/inventory.py items sql 1 4` (38 items). First item: `anime_relations`. Last item: `excluded_scanlators`. The chunk covers the anime-side tables, indexes, triggers and views of `AnimeDatabase` (`data/src/main/sqldelightanime`), the manga `chapters` and `excluded_scanlators` tables, the shared `categories` table (it exists in both databases), and a handful of generic statement names (`count`, `delete`, `animehistory`, `animelib`, and four `delete*` statements). Every path is repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`). Statements that belong to the same `.sq` files but are not in this chunk (for example `getAnimeById`, `insert`, `update`) are catalogued in the other three SQL chunks; here they are only named so the reader knows they exist. Cross references use `area:<id>`: schema and migrations are owned by area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations, the repositories by area:05-data-repos-entries-items and area:06-data-repos-history-track-source-misc.

## Short file names used in this catalog

Where a bullet gives a full path once and then writes a bare file name with a line number (for example `AnimeRepositoryImpl.kt:25`), the bare name is resolved with this table. All paths are repo-relative.

| Short name | Full path |
|---|---|
| `AnimeRepositoryImpl.kt`, `AnimeMapper.kt`, `AnimeRelationRepositoryImpl.kt` | `data/src/main/java/tachiyomi/data/entries/anime/` + name |
| `MangaRepositoryImpl.kt` | `data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt` |
| `EpisodeRepositoryImpl.kt`, `EpisodeSanitizer.kt` | `data/src/main/java/tachiyomi/data/items/episode/` + name |
| `ChapterRepositoryImpl.kt`, `ChapterSanitizer.kt` | `data/src/main/java/tachiyomi/data/items/chapter/` + name |
| `AnimeHistoryRepositoryImpl.kt`, `AnimeHistoryMapper.kt` | `data/src/main/java/tachiyomi/data/history/anime/` + name |
| `AnimeTrackRepositoryImpl.kt` | `data/src/main/java/tachiyomi/data/track/anime/AnimeTrackRepositoryImpl.kt` |
| `MangaTrackRepositoryImpl.kt` | `data/src/main/java/tachiyomi/data/track/manga/MangaTrackRepositoryImpl.kt` |
| `AnimeUpdatesRepositoryImpl.kt` | `data/src/main/java/tachiyomi/data/updates/anime/AnimeUpdatesRepositoryImpl.kt` |
| `AnimeCategoryRepositoryImpl.kt` | `data/src/main/java/tachiyomi/data/category/anime/AnimeCategoryRepositoryImpl.kt` |
| `MangaCategoryRepositoryImpl.kt` | `data/src/main/java/tachiyomi/data/category/manga/MangaCategoryRepositoryImpl.kt` |
| `CustomButtonRepositoryImpl.kt` | `data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt` |
| `AnimeSourceRepositoryImpl.kt`, `AnimeStubSourceRepositoryImpl.kt` | `data/src/main/java/tachiyomi/data/source/anime/` + name |
| `MangaSourceRepositoryImpl.kt` | `data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt` |
| `MangaExtensionRepoRepositoryImpl.kt` | `data/src/main/java/mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt` |
| `AnimeExtensionStoreRepositoryImpl.kt` | `data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt` |
| `AnimeRestorer.kt`, `MangaRestorer.kt`, `AnimeCategoriesRestorer.kt`, `CustomButtonRestorer.kt` | `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/` + name |
| `AnimeBackupCreator.kt`, `MangaBackupCreator.kt` | `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/` + name |
| `BackupCreator.kt` | `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt` |
| `PlayerViewModel.kt`, `PlayerActivity.kt`, `ExternalIntents.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/` + name |
| `ReaderViewModel.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt` |
| `AnimeLibraryScreenModel.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt` |
| `MangaLibraryScreenModel.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt` |
| `AnimeHistoryScreenModel.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryScreenModel.kt` |
| `AnimeUpdatesScreenModel.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt` |
| `AnimeScreenModel.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt` |
| `MangaScreenModel.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt` |
| `AnimeTrackInfoDialog.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt` |
| `MangaTrackInfoDialog.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt` |
| `BrowseAnimeSourceScreenModel.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt` |
| `MigrateAnimeDialog.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeDialog.kt` |
| `AndroidAnimeSourceManager.kt` | `app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt` |
| `AnimeLibraryUpdateJob.kt`, `AnimeMetadataUpdateJob.kt` | `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/` + name |
| `SyncEpisodesWithSource.kt` | `app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt` |
| `SyncChaptersWithSource.kt`, `SetReadStatus.kt`, `GetAvailableScanlators.kt` | `app/src/main/java/eu/kanade/domain/items/chapter/interactor/` + name |
| `UpdateAnime.kt`, `SetAnimeViewerFlags.kt`, `SyncRelatedAnimeWithSource.kt` | `app/src/main/java/eu/kanade/domain/entries/anime/interactor/` + name |
| `GetExcludedScanlators.kt`, `SetExcludedScanlators.kt` | `app/src/main/java/eu/kanade/domain/entries/manga/interactor/` + name |
| `UpdateAnimeFromRemote.kt` | `app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt` |
| `ClearAnimeDatabaseScreen.kt`, `ClearDatabaseScreen.kt` | `app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/` + name |
| `DeleteAnimeCategory.kt`, `CreateAnimeCategoryWithName.kt`, `ReorderAnimeCategory.kt`, `HideAnimeCategory.kt`, `SetSortModeForAnimeCategory.kt`, `SetAnimeCategories.kt` | `domain/src/main/java/tachiyomi/domain/category/anime/interactor/` + name |
| `DeleteMangaCategory.kt`, `SetMangaCategories.kt` | `domain/src/main/java/tachiyomi/domain/category/manga/interactor/` + name |
| `DeleteCustomButton.kt`, `ReorderCustomButton.kt`, `ToggleFavoriteCustomButton.kt` | `domain/src/main/java/tachiyomi/domain/custombuttons/interactor/` + name |
| `CustomButton.kt` | `domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt` |
| `GetAnimeTracks.kt`, `DeleteAnimeTrack.kt`, `InsertAnimeTrack.kt` | `domain/src/main/java/tachiyomi/domain/track/anime/interactor/` + name |
| `DeleteMangaTrack.kt` | `domain/src/main/java/tachiyomi/domain/track/manga/interactor/DeleteMangaTrack.kt` |
| `GetLibraryAnime.kt`, `NetworkToLocalAnime.kt` | `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/` + name |
| `GetAnimeHistory.kt`, `GetNextEpisodes.kt`, `RemoveAnimeHistory.kt` | `domain/src/main/java/tachiyomi/domain/history/anime/interactor/` + name |
| `GetAnimeUpdates.kt` | `domain/src/main/java/tachiyomi/domain/updates/anime/interactor/GetAnimeUpdates.kt` |
| `GetAnimeSeasonsByParentId.kt` | `domain/src/main/java/tachiyomi/domain/items/season/interactor/GetAnimeSeasonsByParentId.kt` |
| `GetAnimeWithEpisodesAndSeasons.kt` | `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetAnimeWithEpisodesAndSeasons.kt` |
| `GetAnimeSourcesWithNonLibraryAnime.kt` | `domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetAnimeSourcesWithNonLibraryAnime.kt` |
| `GetMangaExtensionRepoCount.kt`, `DeleteMangaExtensionRepo.kt` | `domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/` + name |
| `SettingsBrowseScreen.kt` | `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsBrowseScreen.kt` |
| `GetRelatedAnime.kt` | `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetRelatedAnime.kt` |
| `GetTracksPerAnime.kt` | `domain/src/main/java/tachiyomi/domain/track/anime/interactor/GetTracksPerAnime.kt` |
| `AnimeFetchInterval.kt` | `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchInterval.kt` |

## 0. Facts that apply to every item (OBSERVED, then PROPOSAL)

### 0.1 Two databases, pragmas, versioning (OBSERVED)

* Two SQLite files, opened by `AndroidSqliteDriver` in `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:78-121`:
  * manga: file name `tachiyomi.db`, SQLDelight schema `Database`, `.sq` files in `data/src/main/sqldelight` (`data/`, `view/`), migrations `data/src/main/sqldelight/migrations/1.sqm` to `32.sqm`.
  * anime: file name `tachiyomi.animedb`, SQLDelight schema `AnimeDatabase`, `.sq` files in `data/src/main/sqldelightanime` (`dataanime/`, `view/`), migrations `data/src/main/sqldelightanime/migrations/113.sqm` to `138.sqm`.
  * Gradle wiring: `data/build.gradle.kts` (`create("Database")` and `create("AnimeDatabase")`, dialect `libs.sqldelight.dialects.sql`).
* On every open (`onOpen` in `AppModule.kt:89-93` and `114-118`) three pragmas run: `PRAGMA foreign_keys = ON`, `PRAGMA journal_mode = WAL`, `PRAGMA synchronous = NORMAL`. All `ON DELETE CASCADE` clauses below depend on `foreign_keys = ON`.
* Fresh install: SQLDelight runs the `CREATE` statements of the `.sq` files (this includes the `INSERT OR IGNORE` seed rows written at file top level and the triggers). Upgrade: it runs the `.sqm` files between the stored `PRAGMA user_version` and the latest. The two paths are not identical (see 0.4), so what a user sees depends on install history.
* `recursive_triggers` is never enabled (no pragma), so SQLite's default (OFF) applies to every trigger listed in this chunk.
* The Kotlin handler layer (`data/src/main/java/tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt:17-105`, manga twin in `handlers/manga/`) offers `await` (optionally `inTransaction = true`), `awaitList/One/OneOrNull` and `subscribeToList/One/OneOrNull`. A `subscribe*` call is a SQLDelight `Query.asFlow()`: it emits the current result immediately and re-runs the query whenever any table (views are expanded to their base tables) read by the query is written by the same driver.

### 0.2 Column adapters and value conventions (OBSERVED)

From `data/src/main/java/tachiyomi/data/DatabaseAdapter.kt:11-58`:

| Kotlin type | SQLite storage | Rule |
|---|---|---|
| `Date` (`animehistory.last_seen`) | INTEGER | epoch milliseconds (`Date(databaseValue)` / `value.time`) |
| `List<String>` (`animes.genre`) | TEXT | joined with the literal separator `", "` (comma plus one space); decode: empty string gives empty list, otherwise `split(", ")`; a SQL `NULL` is a Kotlin `null` list |
| `AnimeUpdateStrategy` (`animes.update_strategy`) | INTEGER | `ordinal`: `ALWAYS_UPDATE` = 0, `ONLY_FETCH_ONCE` = 1 (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt`); out of range decodes to `ALWAYS_UPDATE` |
| `FetchType` (`animes.fetch_type`) | INTEGER | `ordinal`: `Seasons` = 0, `Episodes` = 1 (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt`); out of range decodes to `Episodes`. Every view below compares `fetch_type` with the literals 1 (episodes) and 0 (seasons) |
| `JsonObject` (`animes.memo`, `episodes.memo`) | BLOB | UTF-8 bytes of the JSON text, e.g. `{}` is bytes `7b 7d` |
| `Boolean` | INTEGER | 0 / 1 (`bookmark`, `seen`, `favorite`, `initialized`, `private`, `isFavorite`, `fillermark`) |

* Time units are mixed. Milliseconds since epoch: `date_added`, `date_fetch`, `date_upload`, `last_update`, `next_update`, `cover_last_modified`, `background_last_modified`, `animehistory.last_seen`, `anime_relations.last_fetched_at`, `anime_sync.start_date`, `anime_sync.finish_date`. Seconds since epoch (written by SQL `strftime('%s', 'now')` in triggers): `last_modified_at`, `favorite_modified_at`.
* `episodes.last_second_seen` and `episodes.total_seconds` hold MILLISECONDS despite their names (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1664-1668`: `seconds = position * 1000L`, `currentEp.last_second_seen = seconds`, `currentEp.total_seconds = totalSeconds`).
* Partial updates: every repository `update` statement writes `col = coalesce(:param, col)`. A `NULL` parameter therefore means "leave unchanged"; a nullable column can never be set back to `NULL` through `update`. The Kotlin repositories always pass `isSyncing = 0` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:215`, `data/src/main/java/tachiyomi/data/items/episode/EpisodeRepositoryImpl.kt:76`, `data/src/main/java/tachiyomi/data/items/chapter/ChapterRepositoryImpl.kt:69`), so every ordinary update also resets `is_syncing`.
* `IN :ids` in `.sq` files is expanded by SQLDelight to `IN (?, ?, ?)` (for three elements) with one bound parameter per element; an empty list gives `IN ()`, which is valid in SQLite and matches nothing.

### 0.3 SQLite behaviours the schema relies on (OBSERVED)

* `AFTER UPDATE OF favorite` fires when `favorite` appears in the `SET` list, even if the value does not change. The repository `update` statements always list every column (via `coalesce`), so `update_last_favorite_at_animes` fires on every `animes` update, and `favorite_modified_at` is refreshed each time.
* `update_last_modified_at_*` triggers are `AFTER UPDATE` triggers declared `FOR EACH ROW` and themselves run an `UPDATE` on the same table: each ordinary write to `animes`, `episodes`, `chapters` costs at least two row writes, and the version triggers add one or two more.
* `SELECT *` and `M.*` return columns in table-definition order. The anime `.sq` order equals the `ALTER TABLE ADD COLUMN` order of the migrations, so fresh and upgraded databases return the same order. The generated Kotlin mappers are positional.
* A bare column in an aggregate query with `MAX()` takes its value from the row that supplied the maximum (SQLite documented behaviour). `animehistoryView` depends on this for `max_last_seen.episode_id`.
* A view containing `ORDER BY` (here `animeupdatesView`) keeps that order only because SQLite keeps the inner order when the outer query has no `ORDER BY` and is flattened; this is not guaranteed by the SQL standard.
* The double-quoted string literal `""` in `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0)` is read by SQLite as an empty string (its double-quote-as-string fallback). Single quotes are the portable spelling.

### 0.4 Fresh-install schema versus upgraded schema (OBSERVED drift)

| Object | Fresh install (`.sq`) | Upgraded install (`.sqm`) |
|---|---|---|
| partial index on `animes(favorite)` | `animelib_favorite_index` (`data/src/main/sqldelightanime/dataanime/animes.sq:43`) | `library_favorite_index` (`data/src/main/sqldelightanime/migrations/114.sqm`) |
| trigger on `animes` that sets `favorite_modified_at` | `update_last_favorite_at_animes` | `update_last_favorited_at_animes` (`data/src/main/sqldelightanime/migrations/126.sqm`) |
| trigger that blocks deleting category `_id <= 0` | `system_category_delete_trigger` | `system_anime_category_delete_trigger` (`data/src/main/sqldelightanime/migrations/120.sqm`); manga keeps `system_category_delete_trigger` (`data/src/main/sqldelight/migrations/19.sqm`) |
| `animes_categories.last_modified_at` column and trigger `update_last_modified_at_animes_categories` | absent | present (`data/src/main/sqldelightanime/migrations/126.sqm`) |
| `animehistory.last_seen` | `INTEGER AS Date` (nullable) | `INTEGER AS Date NOT NULL` (`data/src/main/sqldelightanime/migrations/115.sqm`) |
| defaults of `animes.season_flags`, `season_number`, `season_source_order`, `background_last_modified`, `episodes.fillermark` | none in `CREATE TABLE` | `DEFAULT 0`, `DEFAULT -1.0`, `DEFAULT 0`, `DEFAULT 0`, `DEFAULT 0` (`131.sqm`, `134.sqm`) |
| `animes.memo`, `episodes.memo` default | `DEFAULT '{}'` (TEXT literal in a BLOB column) | `DEFAULT x'7b7d'` (BLOB, `data/src/main/sqldelightanime/migrations/136.sqm`) |

All inserts in Kotlin pass every NOT NULL column explicitly, so the drift is invisible to the app. Proposal: Kurobayomi has no Android history to upgrade, so it ships one fresh schema (the "Rust DDL" blocks below) and uses `PRAGMA user_version` for its own future migrations (area:11-app-migrations). Where the two Aniyomi variants differ, the Rust DDL takes the `.sq` name and the stricter of the two column definitions.

### 0.5 Rust conventions used by every "Proposal" block

* Crates: `rusqlite` with features `bundled`, `hooks` (change notification), `functions` (custom scalar functions), `serde_json` for ids and memo. Bundled SQLite is newer than 3.38, so `json_each` and `RETURNING` are always available.
* One file per database, same layout as Aniyomi: `$XDG_DATA_HOME/kurobayomi/manga.db` and `$XDG_DATA_HOME/kurobayomi/anime.db`. Rationale: the SQL in this catalog is reused unchanged (two tables named `categories`, `history`, `sources` would otherwise collide), the Aniyomi backup format already splits manga and anime, and a failed anime migration cannot corrupt manga data.
* Connection setup, run once per connection:

```rust
pub fn open(path: &std::path::Path) -> rusqlite::Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open(path)?;
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;",
    )?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}
```

* Never run `PRAGMA recursive_triggers = ON`.
* Statement functions take `&rusqlite::Connection` (a `&rusqlite::Transaction` derefs to it) and use named parameters (`:name`, `rusqlite::named_params!`). Row mappers read columns BY NAME (`row.get("title")?`), never by index, so view column order never matters.
* Lists of ids are bound as one JSON text parameter and expanded in SQL with `IN (SELECT value FROM json_each(:ids))`; there is no per-statement parameter limit and no dynamic SQL:

```rust
fn ids_json(ids: &[i64]) -> String { serde_json::to_string(ids).expect("i64 slice serialises") }
```

* Shared decoders (used by every `AnimeRow`/`SeasonAnimeRow` mapper):

```rust
use rusqlite::{types::ValueRef, Row};

pub fn genre_from_row(row: &Row, col: &str) -> rusqlite::Result<Option<Vec<String>>> {
    Ok(row.get::<_, Option<String>>(col)?.map(|s| {
        if s.is_empty() { Vec::new() } else { s.split(", ").map(str::to_owned).collect() }
    }))
}
pub fn genre_to_sql(g: Option<&[String]>) -> Option<String> { g.map(|v| v.join(", ")) }

/// `memo` is declared BLOB but a fresh Aniyomi DB stores the TEXT default '{}'; accept both.
pub fn memo_from_row(row: &Row, col: &str) -> rusqlite::Result<serde_json::Map<String, serde_json::Value>> {
    let bytes: &[u8] = match row.get_ref(col)? {
        ValueRef::Blob(b) => b,
        ValueRef::Text(t) => t,
        _ => return Ok(Default::default()),
    };
    serde_json::from_slice(bytes).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Blob, Box::new(e))
    })
}
```

  (`Vec<u8>: FromSql` accepts only BLOB, so a plain `row.get::<_, Vec<u8>>("memo")` fails on a TEXT `'{}'` value; that is why the decoder reads a `ValueRef`.)
* Change notification replacing `subscribeTo*`: register `Connection::update_hook` on the single writer connection collecting table names into a set, publish the set from `commit_hook` over a `tokio::sync::broadcast` channel, clear it from `rollback_hook`. A subscriber declares the base tables its query reads (listed as "Reads" under each view/query below), re-runs the query when the published set intersects, and debounces by 250 ms because the player writes `episodes` and `animehistory` once per second of playback (`PlayerViewModel.kt:1764-1766`, `1792-1799`).

### 0.6 Row structs shared by the Rust blocks

```rust
pub struct AnimeRow {
    pub id: i64, pub source: i64, pub url: String,
    pub artist: Option<String>, pub author: Option<String>, pub description: Option<String>,
    pub genre: Option<Vec<String>>, pub title: String, pub status: i64,
    pub thumbnail_url: Option<String>, pub favorite: bool,
    pub last_update: Option<i64>, pub next_update: Option<i64>, pub initialized: bool,
    pub viewer: i64, pub episode_flags: i64, pub cover_last_modified: i64, pub date_added: i64,
    pub update_strategy: AnimeUpdateStrategy, pub calculate_interval: i64,
    pub last_modified_at: i64, pub favorite_modified_at: Option<i64>,
    pub version: i64, pub is_syncing: i64, pub fetch_type: FetchType, pub parent_id: Option<i64>,
    pub season_flags: i64, pub season_number: f64, pub season_source_order: i64,
    pub background_url: Option<String>, pub background_last_modified: i64,
    pub memo: serde_json::Map<String, serde_json::Value>,
}
pub fn anime_from_row(row: &rusqlite::Row) -> rusqlite::Result<AnimeRow>; // by column name; works on `animes` and on every view that exposes `M.*`
pub enum AnimeUpdateStrategy { AlwaysUpdate = 0, OnlyFetchOnce = 1 }
pub enum FetchType { Seasons = 0, Episodes = 1 }
```

(`last_update` and `next_update` are nullable in SQL; the Kotlin mapper turns `NULL` into `0` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeMapper.kt:50-51`). Proposal: the Rust domain type does the same `unwrap_or(0)` at the boundary and keeps `Option` in the row struct.)

## 1. Index of the 38 items

| # | Item | Kind | Database | Section |
|---|---|---|---|---|
| 1 | `anime_relations` | table | anime | 2.6 |
| 2 | `anime_relations_anime_id_index` | index | anime | 2.6 |
| 3 | `anime_relations_related_anime_id_index` | index | anime | 2.6 |
| 4 | `anime_sync` | table | anime | 2.5 |
| 5 | `animedeletableView` | view | anime | 4.8 |
| 6 | `animehistory` | table and view query | anime | 2.4 |
| 7 | `animehistoryView` | view | anime | 4.2 |
| 8 | `animehistory_history_episode_id_index` | index | anime | 2.4 |
| 9 | `animehistorystatsView` | view | anime | 4.3 |
| 10 | `animelib` | view query | anime | 4.7 |
| 11 | `animelibView` | view | anime | 4.6 |
| 12 | `animelib_favorite_index` | partial index | anime | 2.1 |
| 13 | `animes` | table (and 3 triggers) | anime | 2.1 |
| 14 | `animes_categories` | table (and 1 trigger) | anime | 2.2 |
| 15 | `animes_fetch_type` | index | anime | 2.1 |
| 16 | `animes_parent_id` | index | anime | 2.1 |
| 17 | `animes_url_index` | index | anime | 2.1 |
| 18 | `animeseasonsView` | view | anime | 4.5 |
| 19 | `animeseasonstatsView` | view | anime | 4.4 |
| 20 | `animesources` | table | anime | 2.7 |
| 21 | `animeupdatesView` | view | anime | 4.9 |
| 22 | `categories` | table (both databases, trigger, seed row) | anime and manga | 2.8 |
| 23 | `chapters` | table (and 2 triggers) | manga | 3.1 |
| 24 | `chapters_manga_id_index` | index | manga | 3.1 |
| 25 | `chapters_unread_by_manga_index` | partial index | manga | 3.1 |
| 26 | `count` | query (`extension_repos`) | manga | 5.1 |
| 27 | `custom_buttons` | table (and seed row) | anime | 2.9 |
| 28 | `delete` | 7 statements in 7 files | both | 5.2 |
| 29 | `deleteAnimeCategoryByAnimeId` | query | anime | 5.3 |
| 30 | `deleteAnimesNotInLibraryByAnimeIds` | query | anime | 5.4 |
| 31 | `deleteByAnimeId` | query | anime | 5.5 |
| 32 | `deleteMangaCategoryByMangaId` | query | manga | 5.6 |
| 33 | `deleteMangasNotInLibraryBySourceIds` | query | manga | 5.7 |
| 34 | `episodes` | table (and 2 triggers) | anime | 2.3 |
| 35 | `episodes_anime_id_index` | index | anime | 2.3 |
| 36 | `episodes_unseen_by_anime_index` | partial index | anime | 2.3 |
| 37 | `episodestatsView` | view | anime | 4.1 |
| 38 | `excluded_scanlators` | table | manga | 3.2 |

## 2. Anime database: tables, indexes, triggers (`data/src/main/sqldelightanime`)

### 2.1 `animes` (table), `animelib_favorite_index`, `animes_url_index`, `animes_parent_id`, `animes_fetch_type`, and the three `animes` triggers

OBSERVED. Source: `data/src/main/sqldelightanime/dataanime/animes.sq`. One row per anime (a "series"), whether or not it is in the library. Rows are created by `NetworkToLocalAnime` the first time a browse/search result is shown (`domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt:12-28`), and also for every related-anime suggestion (`SyncRelatedAnimeWithSource`, section 2.6) and every season child. `favorite = 1` means "in the library".

`data/src/main/sqldelightanime/dataanime/animes.sq:8-41`

```sql
CREATE TABLE animes(
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
    episode_flags INTEGER NOT NULL,
    cover_last_modified INTEGER NOT NULL,
    date_added INTEGER NOT NULL,
    update_strategy INTEGER AS AnimeUpdateStrategy NOT NULL DEFAULT 0,
    calculate_interval INTEGER DEFAULT 0 NOT NULL,
    last_modified_at INTEGER NOT NULL DEFAULT 0,
    favorite_modified_at INTEGER,
    version INTEGER NOT NULL DEFAULT 0,
    is_syncing INTEGER NOT NULL DEFAULT 0,
    fetch_type INTEGER AS FetchType NOT NULL DEFAULT 1,
    parent_id INTEGER,
    season_flags INTEGER NOT NULL,
    season_number REAL NOT NULL,
    season_source_order INTEGER NOT NULL,
    background_url TEXT,
    background_last_modified INTEGER NOT NULL,
    memo BLOB AS JsonObject NOT NULL DEFAULT '{}'
);
```

Columns (32). "Migration" names the `.sqm` in `data/src/main/sqldelightanime/migrations/` that introduced the column; "base" means present before `113.sqm` and rebuilt unchanged by `114.sqm`.

| Column | SQLite type, constraint | Kotlin type | Migration | Meaning |
|---|---|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY (rowid alias, no AUTOINCREMENT) | Long | base | local id; a deleted maximum id can be reused |
| `source` | INTEGER NOT NULL | Long | base | anime source id (`AnimeSource.id`) |
| `url` | TEXT NOT NULL | String | base | source-relative url; with `source` it is the natural key (not declared UNIQUE) |
| `artist` | TEXT | String? | base | |
| `author` | TEXT | String? | base | |
| `description` | TEXT | String? | base | `NULL` means details were never fetched |
| `genre` | TEXT | List<String>? | base | `", "`-joined (0.2) |
| `title` | TEXT NOT NULL | String | base | |
| `status` | INTEGER NOT NULL | Long | base | `SAnime` constants (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-79`): UNKNOWN 0, ONGOING 1, COMPLETED 2, LICENSED 3, PUBLISHING_FINISHED 4, CANCELLED 5, ON_HIATUS 6, UPCOMING 7 |
| `thumbnail_url` | TEXT | String? | base | cover url |
| `favorite` | INTEGER NOT NULL (Boolean) | Boolean | base | library membership |
| `last_update` | INTEGER | Long? | base | ms; last time the episode list changed (`UpdateAnime.awaitUpdateLastUpdate`, `app/src/main/java/eu/kanade/domain/entries/anime/interactor/UpdateAnime.kt:33-35`) |
| `next_update` | INTEGER | Long? | base | ms; predicted next library check (`AnimeFetchInterval`) |
| `initialized` | INTEGER NOT NULL (Boolean) | Boolean | base | details fetched once |
| `viewer` | INTEGER NOT NULL | Long | base | per-anime viewer flags, anime layout below |
| `episode_flags` | INTEGER NOT NULL | Long | base (the manga column is `chapter_flags`) | episode list sort/filter/display bits, layout below |
| `cover_last_modified` | INTEGER NOT NULL | Long | base | ms cache-buster for the cover |
| `date_added` | INTEGER NOT NULL | Long | base | ms when favorited, `0` when removed from the library (`UpdateAnime.kt:49-56`) |
| `update_strategy` | INTEGER NOT NULL DEFAULT 0 | AnimeUpdateStrategy | 121 | 0 ALWAYS_UPDATE, 1 ONLY_FETCH_ONCE |
| `calculate_interval` | INTEGER NOT NULL DEFAULT 0 | Long (Int in the model) | 124 | fetch interval in days; 0 not computed yet; a negative value is a user-pinned interval that `AnimeFetchInterval.toAnimeUpdate` keeps as is (`domain/src/main/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchInterval.kt:22`); computed values are clamped `1..MAX_INTERVAL` (area:26-library-update-jobs) |
| `last_modified_at` | INTEGER NOT NULL DEFAULT 0 | Long | 126 | seconds, maintained by trigger |
| `favorite_modified_at` | INTEGER | Long? | 126 | seconds, maintained by trigger |
| `version` | INTEGER NOT NULL DEFAULT 0 | Long | 127 | change counter, maintained by trigger |
| `is_syncing` | INTEGER NOT NULL DEFAULT 0 | Long | 127 | 1 suppresses version bumps; backup restore sets it to 1 on update (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt:147`) |
| `fetch_type` | INTEGER NOT NULL DEFAULT 1 | FetchType | 131 (values rewritten by 132 and 135) | 0 Seasons (parent whose children are season rows), 1 Episodes |
| `parent_id` | INTEGER | Long? | 131 | `_id` of the parent "Seasons" anime; NOT a foreign key, so deleting a parent leaves children pointing at a missing id (handled by `removeParentIdByIds`) |
| `season_flags` | INTEGER NOT NULL (migration default 0) | Long | 131 | season list sort/filter/display bits, layout below |
| `season_number` | REAL NOT NULL (migration default -1.0) | Double | 131 | `-1.0` when unknown (`Anime.create()` default) |
| `season_source_order` | INTEGER NOT NULL (migration default 0) | Long | 131 | position inside the parent as returned by the source |
| `background_url` | TEXT | String? | 134 | |
| `background_last_modified` | INTEGER NOT NULL (migration default 0) | Long | 134 | ms cache-buster for the background image |
| `memo` | BLOB NOT NULL DEFAULT `'{}'` (fresh) / `x'7b7d'` (upgraded) | JsonObject | 136 | opaque extension-owned JSON object, round-tripped unchanged |

History of `fetch_type` values (OBSERVED, `131.sqm`, `132.sqm`, `135.sqm`): 131 added the column with default 1 while the old enum order was different; 132 remapped `2 to 1`, `1 to 0`, `0 to 1`; 135 then executed `UPDATE animes SET fetch_type = 1` for every row. A fresh database never sees this; the final contract is 0 Seasons, 1 Episodes, default 1.

Bit layouts (OBSERVED, `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt` companion, lines 188-289). Rust must keep the numeric values because backups and the Aniyomi database carry them. A filter field is read as `flags & MASK`; setting a field is `flags = (flags & !MASK) | (value & MASK)` (`tachiyomi.domain.library.model.plus`).

`episode_flags`:

| Field | Mask | Values |
|---|---|---|
| sort direction | `0x1` | `0x0` EPISODE_SORT_DESC, `0x1` EPISODE_SORT_ASC |
| unseen filter | `0x6` | `0x2` show unseen only, `0x4` show seen only, `0x0` off |
| downloaded filter | `0x18` | `0x8` show downloaded, `0x10` show not downloaded |
| bookmarked filter | `0x60` | `0x20` bookmarked, `0x40` not bookmarked |
| fillermarked filter | `0x180` | `0x80` fillermarked, `0x100` not fillermarked |
| sorting | `0x600` | `0x0` source order, `0x200` episode number, `0x400` upload date, `0x600` alphabet |
| previews | `0x800` | `0x0` show previews, `0x800` hide |
| summaries | `0x1000` | `0x0` show summaries, `0x1000` hide |
| display | `0x100000` | `0x0` name, `0x100000` number |

`season_flags`:

| Field | Mask | Values |
|---|---|---|
| sort direction | `0x1` | `0x0` DESC, `0x1` ASC |
| downloaded filter | `0x6` | `0x2` downloaded, `0x4` not downloaded |
| unseen filter | `0x18` | `0x8` unseen, `0x10` seen |
| started filter | `0x60` | `0x20` started, `0x40` not started |
| completed filter | `0x180` | `0x80` completed, `0x100` not completed |
| bookmarked filter | `0x600` | `0x200` bookmarked, `0x400` not bookmarked |
| fillermarked filter | `0x1800` | `0x800` fillermarked, `0x1000` not fillermarked |
| sorting | `0xE000` | `0x0` source, `0x2000` season, `0x4000` upload, `0x6000` alphabet, `0x8000` count, `0xA000` last seen, `0xC000` fetched |
| grid display mode | `0x30000` | value = `(flags & mask) >> 16` (`SeasonDisplayMode.fromLong`) |
| grid size | `0x3C0000` | value = `(flags & mask) >> 18` |
| overlay downloaded | `0x400000` | bit set means shown |
| overlay unseen | `0x800000` | bit set means shown |
| overlay local | `0x1000000` | bit set means shown |
| overlay language | `0x2000000` | bit set means shown |
| overlay continue | `0x4000000` | bit set means shown |
| display mode | `0x8000000` | `0x0` source, `0x8000000` number |

`viewer` (anime only; the manga column stores reading mode `0x7` and orientation `0x38`, see area:43-reader-core):

| Field | Mask | Encoding |
|---|---|---|
| skip-intro length in seconds | `0xFF` (bits 0-7) | stored raw |
| next episode number to air | `0xFFFF00` (bits 8-23) | stored as `value * 16^2` (`value << 8`) |
| next episode airing time | `0x0FFFFFFFF000000` (bits 24-55) | stored as `value * 16^6` (`value << 24`), seconds epoch |
| skip-intro disabled | `0x100000000000000` (bit 56) | set to `1 << 56` when the length is set to 0 (`app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt:13-24`) |

`resetViewerFlags` (`UPDATE animes SET viewer = 0`, other chunk) clears all four fields for every row; it is what "reset per-anime reader/player settings" calls (`domain/src/main/java/tachiyomi/domain/entries/anime/interactor/ResetAnimeViewerFlags.kt:9`).

#### Indexes of `animes`

`data/src/main/sqldelightanime/dataanime/animes.sq:43-43`

```sql
CREATE INDEX animelib_favorite_index ON animes(favorite) WHERE favorite = 1;
```

`animelib_favorite_index`: partial index; only rows with `favorite = 1` are indexed, so it is tiny (library size). SQLite uses it only when the query's WHERE contains the literal `favorite = 1`: `animelibView`, `getFavorites`, `getFavoriteBySourceId`, `getDuplicateLibraryAnime`, `getUpcomingAnime`, `animeupdatesView` do; the `favorite = 0` queries (`getWatchedAnimeNotInLibrary`, `deleteAnimesNotInLibraryByAnimeIds`, `animedeletableView`) scan the table. Upgraded databases carry the same index under the name `library_favorite_index` (0.4).

`data/src/main/sqldelightanime/dataanime/animes.sq:44-44`

```sql
CREATE INDEX animes_url_index ON animes(url);
```

`animes_url_index`: serves `getAnimeByUrlAndSource` (`WHERE url = :url AND source = :source LIMIT 1`), which runs once per search result or episode deep link. It is a single-column index: SQLite reads all rows with that url and filters on `source`; the set is tiny in practice.

`data/src/main/sqldelightanime/dataanime/animes.sq:45-45`

```sql
CREATE INDEX animes_parent_id ON animes(parent_id);
```

`animes_parent_id`: serves `getChildrenByParentId`, `animeseasonsView` lookups by `parent_id`, `animeseasonstatsView` grouping, and `animedeletableView`'s `parent_id IS NULL` filter. Created by `data/src/main/sqldelightanime/migrations/133.sqm`.

`data/src/main/sqldelightanime/dataanime/animes.sq:46-46`

```sql
CREATE INDEX animes_fetch_type ON animes(fetch_type);
```

`animes_fetch_type`: created by `data/src/main/sqldelightanime/migrations/133.sqm`; no statement in the `.sq` files filters on `fetch_type` alone (the views use it inside `CASE`), so SQLite cannot use it for the catalogued queries. Proposal: omit it from the Rust DDL (it has 2 distinct values and costs a write per insert).

#### Triggers on `animes`

`data/src/main/sqldelightanime/dataanime/animes.sq:48-54`

```sql
CREATE TRIGGER update_last_favorite_at_animes
AFTER UPDATE OF favorite ON animes
BEGIN
  UPDATE animes
  SET favorite_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;
```

`update_last_favorite_at_animes`: after any `UPDATE` that names `favorite` in its `SET` list (see 0.3, that is every repository update), writes `favorite_modified_at = strftime('%s','now')` (seconds, TEXT result stored into the INTEGER column becomes an integer). Not part of Aniyomi's visible behaviour; the value only travels into backups (`BackupAnime.favoriteModifiedAt`, `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnime.kt:42`).

`data/src/main/sqldelightanime/dataanime/animes.sq:56-63`

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

`update_last_modified_at_animes`: every update stamps `last_modified_at` (seconds). Backed up as `BackupAnime.lastModifiedAt` (`BackupAnime.kt:41`).

`data/src/main/sqldelightanime/dataanime/animes.sq:65-73`

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

`update_anime_version`: increments `version` once when `url`, `description` or `favorite` changed and `is_syncing = 0`. NULL rule: `new.description != old.description` is NULL (not true) when either side is NULL, so setting a description for the first time (NULL to text) does NOT bump the version; text to text does. Nothing in the app reads `version` except backup/restore.

Callers of this table as a whole: `data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt` (whole file); statements in this file that are catalogued in other chunks: `getAnimeById` (`AnimeRepositoryImpl.kt:25,29`), `getAnimeByUrlAndSource` (`34,44`), `getFavorites` (`53`), `getWatchedAnimeNotInLibrary` (`57`, only called from `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt:102`), `getAllAnime` (no caller), `getAllAnimeSourceAndUrl` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt:48`), `getAnimesWithFavoriteTimestamp` (no caller), `getAnimeSourceIdWithFavoriteCount` (`data/src/main/java/tachiyomi/data/source/anime/AnimeSourceRepositoryImpl.kt:41`), `getFavoriteBySourceId` (`69`), `getDuplicateLibraryAnime` (`74`), `getUpcomingAnime` (`81`), `resetViewerFlags` (`87`), `resetIsSyncing` (no caller), `removeParentIdByIds` (`172`), `getChildrenByParentId` (`185`), `insert` (`106`), `update` (`191`), `selectLastInsertedRowId` (`136`).

Perf notes: `animes` is read by every screen; with a typical library (2,000 rows) every statement here is sub-millisecond. The three update triggers turn one logical `UPDATE` into about four row writes (0.3). Playback writes `episodes`, not `animes`, once per second, but the episode version trigger (2.3) also updates `animes.version`, which fires the three `animes` triggers again, so a second of playback causes about seven row writes in total. See the throttling proposal in 2.3.

Proposal: Rust DDL (fresh database; column order identical to the Aniyomi `.sq`):

```sql
CREATE TABLE animes(
    _id INTEGER NOT NULL PRIMARY KEY,
    source INTEGER NOT NULL,
    url TEXT NOT NULL,
    artist TEXT,
    author TEXT,
    description TEXT,
    genre TEXT,
    title TEXT NOT NULL,
    status INTEGER NOT NULL,
    thumbnail_url TEXT,
    favorite INTEGER NOT NULL,
    last_update INTEGER,
    next_update INTEGER,
    initialized INTEGER NOT NULL,
    viewer INTEGER NOT NULL,
    episode_flags INTEGER NOT NULL,
    cover_last_modified INTEGER NOT NULL,
    date_added INTEGER NOT NULL,
    update_strategy INTEGER NOT NULL DEFAULT 0,
    calculate_interval INTEGER NOT NULL DEFAULT 0,
    last_modified_at INTEGER NOT NULL DEFAULT 0,
    favorite_modified_at INTEGER,
    version INTEGER NOT NULL DEFAULT 0,
    is_syncing INTEGER NOT NULL DEFAULT 0,
    fetch_type INTEGER NOT NULL DEFAULT 1,
    parent_id INTEGER,
    season_flags INTEGER NOT NULL DEFAULT 0,
    season_number REAL NOT NULL DEFAULT -1.0,
    season_source_order INTEGER NOT NULL DEFAULT 0,
    background_url TEXT,
    background_last_modified INTEGER NOT NULL DEFAULT 0,
    memo BLOB NOT NULL DEFAULT x'7b7d'
);
CREATE INDEX animelib_favorite_index ON animes(favorite) WHERE favorite = 1;
CREATE INDEX animes_url_index ON animes(url);
CREATE INDEX animes_parent_id ON animes(parent_id);
-- animes_fetch_type intentionally not created (never used by a query plan)
-- the three triggers are created verbatim from the SQL above
```

Decisions for the three timestamp/version triggers: keep all three verbatim. Reason: the columns travel in the Aniyomi backup format (area:21-backup-format-and-create) and the cost is small once playback progress is throttled (2.3). The unused SyncYomi remnants (`is_syncing`, `resetIsSyncing`, `getAnimesWithFavoriteTimestamp`, `getAllAnime`) are kept as columns/semantics only; no Rust function is written for the statements without a caller.

Rust statement functions that this chunk owns for the table:

```rust
pub fn delete_animes_not_in_library_by_anime_ids(conn: &rusqlite::Connection, anime_ids: &[i64]) -> rusqlite::Result<usize>; // section 5.4
```

### 2.2 `animes_categories` (table) and trigger `insert_anime_category_update_version`

OBSERVED. Source: `data/src/main/sqldelightanime/dataanime/animes_categories.sq`. Join table anime to category. An anime with no row here is shown in the "Default" (system) category `0` by `animelibView` (`coalesce(MC.category_id, 0)`); the system category itself is never stored as a link row by the UI (the pickers exclude it: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:517`, `app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryScreenModel.kt:151`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:289`; `AnimeLibraryScreenModel.kt:407-412` hides category `0` when the library is non-empty and no entry is in it).

`data/src/main/sqldelightanime/dataanime/animes_categories.sq:1-9`

```sql
CREATE TABLE animes_categories(
    _id INTEGER NOT NULL PRIMARY KEY,
    anime_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL,
    FOREIGN KEY(category_id) REFERENCES categories (_id)
    ON DELETE CASCADE,
    FOREIGN KEY(anime_id) REFERENCES animes (_id)
    ON DELETE CASCADE
);
```

| Column | Type, constraint | Migration |
|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY | base |
| `anime_id` | INTEGER NOT NULL, FK `animes(_id)` ON DELETE CASCADE | base |
| `category_id` | INTEGER NOT NULL, FK `categories(_id)` ON DELETE CASCADE | base |
| `last_modified_at` | INTEGER NOT NULL DEFAULT 0 (upgraded databases only) | 126 (absent from the `.sq`, see 0.4) |

No UNIQUE constraint on `(anime_id, category_id)` and no secondary index. Deleting a category cascades to its link rows without touching `animes`, so the affected anime silently fall back to category 0.

`data/src/main/sqldelightanime/dataanime/animes_categories.sq:11-16`

```sql
CREATE TRIGGER insert_anime_category_update_version AFTER INSERT ON animes_categories
BEGIN
    UPDATE animes
    SET version = version + 1
    WHERE _id = new.anime_id AND (SELECT is_syncing FROM animes WHERE _id = new.anime_id) = 0;
END;
```

`insert_anime_category_update_version`: every link insert bumps `animes.version` (unless `is_syncing = 1`), which in turn fires the three `animes` triggers.

Callers: `data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:95-102` (`setAnimeCategories`: delete all links of the anime then insert one row per id, inside one transaction) called from `domain/src/main/java/tachiyomi/domain/category/anime/interactor/SetAnimeCategories.kt:13` (which swallows and logs any exception), and from `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:552`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:303`, `app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryScreenModel.kt:161`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeDialog.kt:308`, `app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt:575`; backup restore `AnimeRestorer.kt:354-356`. The `insert` statement itself is `INSERT INTO animes_categories(anime_id, category_id) VALUES (:animeId, :categoryId);` (other chunk).

Perf notes: `animelibView` joins `animes_categories` on `anime_id`; with no index SQLite builds an automatic transient index per query. The category FK cascade scans by `category_id`.

Proposal: Rust DDL adds the two missing indexes and a uniqueness rule (a duplicated id list would otherwise create duplicate library rows):

```sql
CREATE TABLE animes_categories(
    _id INTEGER NOT NULL PRIMARY KEY,
    anime_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL,
    UNIQUE (anime_id, category_id),
    FOREIGN KEY(category_id) REFERENCES categories (_id) ON DELETE CASCADE,
    FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE
);
CREATE INDEX animes_categories_category_id_index ON animes_categories(category_id);
-- the UNIQUE (anime_id, category_id) auto-index already serves lookups by anime_id
CREATE TRIGGER insert_anime_category_update_version AFTER INSERT ON animes_categories
BEGIN
    UPDATE animes
    SET version = version + 1
    WHERE _id = new.anime_id AND (SELECT is_syncing FROM animes WHERE _id = new.anime_id) = 0;
END;
```

`set_anime_categories` uses `INSERT OR IGNORE`. Signature: `pub fn set_anime_categories(conn: &mut rusqlite::Connection, anime_id: i64, category_ids: &[i64]) -> rusqlite::Result<()>` (opens a transaction, runs `deleteAnimeCategoryByAnimeId` from 5.3, then one `INSERT OR IGNORE INTO animes_categories(anime_id, category_id) VALUES (:anime_id, :category_id)` per id).

### 2.3 `episodes` (table), `episodes_anime_id_index`, `episodes_unseen_by_anime_index`, and the two `episodes` triggers

OBSERVED. Source: `data/src/main/sqldelightanime/dataanime/episodes.sq`. One row per episode of an anime with `fetch_type = 1`. Created and reconciled by `SyncEpisodesWithSource` (`app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt:47-253`).

`data/src/main/sqldelightanime/dataanime/episodes.sq:4-27`

```sql
CREATE TABLE episodes(
    _id INTEGER NOT NULL PRIMARY KEY,
    anime_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    name TEXT NOT NULL,
    scanlator TEXT,
    seen INTEGER AS Boolean NOT NULL,
    bookmark INTEGER AS Boolean NOT NULL,
    last_second_seen INTEGER NOT NULL,
    total_seconds INTEGER NOT NULL,
    episode_number REAL NOT NULL,
    source_order INTEGER NOT NULL,
    date_fetch INTEGER NOT NULL,
    date_upload INTEGER NOT NULL,
    last_modified_at INTEGER NOT NULL DEFAULT 0,
    version INTEGER NOT NULL DEFAULT 0,
    is_syncing INTEGER NOT NULL DEFAULT 0,
    summary TEXT,
    preview_url TEXT,
    fillermark INTEGER AS Boolean NOT NULL,
    memo BLOB AS JsonObject NOT NULL DEFAULT '{}',
    FOREIGN KEY(anime_id) REFERENCES animes (_id)
    ON DELETE CASCADE
);
```

| Column | Type, constraint | Kotlin | Migration | Meaning |
|---|---|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY | Long | base | |
| `anime_id` | INTEGER NOT NULL, FK `animes(_id)` ON DELETE CASCADE | Long | base | |
| `url` | TEXT NOT NULL | String | base | identity of the episode inside the anime (compared by exact string in `SyncEpisodesWithSource.kt:102`) |
| `name` | TEXT NOT NULL | String | base | sanitised: `trim().removePrefix(animeTitle).trim(whitespace + '-', '_', ',', ':')` (`data/src/main/java/tachiyomi/data/items/episode/EpisodeSanitizer.kt`) |
| `scanlator` | TEXT | String? | base | |
| `seen` | INTEGER NOT NULL | Boolean | base | |
| `bookmark` | INTEGER NOT NULL | Boolean | base | |
| `last_second_seen` | INTEGER NOT NULL | Long | base | MILLISECONDS of playback position |
| `total_seconds` | INTEGER NOT NULL | Long | base | MILLISECONDS of duration |
| `episode_number` | REAL NOT NULL | Double | base | `< 0` means "not recognised" (`Episode.isRecognizedNumber` is `episodeNumber >= 0f`, `domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt:27-28`) |
| `source_order` | INTEGER NOT NULL | Long | base | index in the source's list (0 = first returned = newest) |
| `date_fetch` | INTEGER NOT NULL | Long | base | ms; for a batch of new episodes `now + remainingCount` so that the first (newest) episode gets the largest value (`SyncEpisodesWithSource.kt:197`) |
| `date_upload` | INTEGER NOT NULL | Long | base (value fix-up in `113.sqm`: `UPDATE episodes SET date_upload = date_fetch WHERE date_upload = 0`) | ms; when the source gives none, the largest upload date seen so far in the list or `now` (`SyncEpisodesWithSource.kt:105-108`) |
| `last_modified_at` | INTEGER NOT NULL DEFAULT 0 | Long | 126 | seconds, trigger |
| `version` | INTEGER NOT NULL DEFAULT 0 | Long | 127 | trigger |
| `is_syncing` | INTEGER NOT NULL DEFAULT 0 | Long | 127 | |
| `summary` | TEXT | String? | 134 | |
| `preview_url` | TEXT | String? | 134 | |
| `fillermark` | INTEGER NOT NULL (upgrade default 0) | Boolean | 134 | |
| `memo` | BLOB NOT NULL DEFAULT `'{}'` | JsonObject | 136 | |

`data/src/main/sqldelightanime/dataanime/episodes.sq:29-29`

```sql
CREATE INDEX episodes_anime_id_index ON episodes(anime_id);
```

`episodes_anime_id_index`: serves every `WHERE anime_id = ?` statement, the three stats views, and the `animes` delete cascade.

`data/src/main/sqldelightanime/dataanime/episodes.sq:30-30`

```sql
CREATE INDEX episodes_unseen_by_anime_index ON episodes(anime_id, seen) WHERE seen = 0;
```

`episodes_unseen_by_anime_index`: partial index of unseen episodes per anime. No statement in the current `.sq` files contains the literal `seen = 0` (`getUpdatesBySeenStatus` binds `:seen` as a parameter, which a partial index cannot serve; the views aggregate `sum(seen)` instead), so the planner never uses it. It is legacy from the manga `chapters_unread_by_manga_index` (3.1). Proposal: omit from the Rust DDL.

`data/src/main/sqldelightanime/dataanime/episodes.sq:32-39`

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

`update_last_modified_at_episodes`: stamps `last_modified_at` (seconds) after every `UPDATE`.

`data/src/main/sqldelightanime/dataanime/episodes.sq:41-55`

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

`update_episode_and_anime_version`: only when the row is not syncing and `seen`, `bookmark` or `last_second_seen` changed: bumps `episodes.version` and, unless the parent anime is syncing, `animes.version`. Consequence: every position save during playback (value of `last_second_seen` changes each second) updates two tables and triggers the `animes` triggers (2.1).

Callers: `data/src/main/java/tachiyomi/data/items/episode/EpisodeRepositoryImpl.kt` (`addAllEpisodes` `:17` inserts all new episodes in one transaction, insert at `:21`, re-reads `last_insert_rowid()` after each insert at `:40`; update at `:61`; `removeEpisodesWithIds` `:86-88`), `SyncEpisodesWithSource.kt:223` (remove), `227` (add), `232` (update); progress writes `PlayerViewModel.kt:1774-1789` (`updateEpisode.await(EpisodeUpdate(id, seen, bookmark, fillermark, lastSecondSeen, totalSeconds))`), `app/src/main/java/eu/kanade/domain/items/episode/interactor/SetSeenStatus.kt:41`, `app/src/main/java/eu/kanade/domain/track/anime/interactor/SyncEpisodeProgressWithTrack.kt:45`, backup `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt:220,246`. Other statements of the file catalogued elsewhere: `getEpisodeById`, `getEpisodesByAnimeId`, `getBookmarkedEpisodesByAnimeId`, `getEpisodeByUrl`, `getEpisodeByUrlAndAnimeId`, `removeEpisodesWithIds`, `resetIsSyncing`, `insert`, `update`, `selectLastInsertedRowId`.

Perf notes: the progress write happens once per second of playback (`onSecondReached`, `PlayerViewModel.kt:1658-1686`) and also runs the history upsert (5.x `animehistory`, 2.4). Combined with the trigger fan-out that is about 7 row writes per second (4 on `episodes`: the update, the `last_modified_at` stamp, the version bump and its own `last_modified_at` stamp again; 2 on `animes`: version and `last_modified_at`; 1 on `animehistory`) plus a library/history/updates Flow re-query in the UI.

Proposal: Rust DDL (indexes `episodes_unseen_by_anime_index` dropped) and a throttle:

```sql
CREATE TABLE episodes(
    _id INTEGER NOT NULL PRIMARY KEY,
    anime_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    name TEXT NOT NULL,
    scanlator TEXT,
    seen INTEGER NOT NULL,
    bookmark INTEGER NOT NULL,
    last_second_seen INTEGER NOT NULL,
    total_seconds INTEGER NOT NULL,
    episode_number REAL NOT NULL,
    source_order INTEGER NOT NULL,
    date_fetch INTEGER NOT NULL,
    date_upload INTEGER NOT NULL,
    last_modified_at INTEGER NOT NULL DEFAULT 0,
    version INTEGER NOT NULL DEFAULT 0,
    is_syncing INTEGER NOT NULL DEFAULT 0,
    summary TEXT,
    preview_url TEXT,
    fillermark INTEGER NOT NULL DEFAULT 0,
    memo BLOB NOT NULL DEFAULT x'7b7d',
    FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE
);
CREATE INDEX episodes_anime_id_index ON episodes(anime_id);
-- both triggers created verbatim from the SQL above
```

* The player keeps position in memory and writes `last_second_seen` and the history row at most every 5 seconds, on pause, on seek, on episode change and on exit; the `seen = true` transition (progress threshold) is written immediately. This keeps Aniyomi's observable behaviour (resume point accurate to the last write) with one fifth of the writes (area:45-player-core owns the player side).
* `pub fn episode_from_row(row: &rusqlite::Row) -> rusqlite::Result<EpisodeRow>` reads `memo` through `memo_from_row` (0.5).
* Bulk insert: `INSERT INTO episodes(anime_id, url, name, scanlator, seen, bookmark, last_second_seen, total_seconds, episode_number, source_order, date_fetch, date_upload, last_modified_at, version, is_syncing, summary, preview_url, fillermark, memo) VALUES (:anime_id, :url, :name, :scanlator, :seen, :bookmark, :last_second_seen, :total_seconds, :episode_number, :source_order, :date_fetch, :date_upload, 0, :version, 0, :summary, :preview_url, :fillermark, :memo) RETURNING _id` (the Kotlin code runs `SELECT last_insert_rowid()` after each insert; `RETURNING` replaces that).

### 2.4 `animehistory` (table and view query), `animehistory_history_episode_id_index`

OBSERVED. Two items share the name `animehistory`: the table (`data/src/main/sqldelightanime/dataanime/animehistory.sq`) and a named query in the view file (`data/src/main/sqldelightanime/view/animehistoryView.sq:28-44`). The view `animehistoryView` is item 7 (section 4.2).

Table:

`data/src/main/sqldelightanime/dataanime/animehistory.sq:3-9`

```sql
CREATE TABLE animehistory(
    _id INTEGER NOT NULL PRIMARY KEY,
    episode_id INTEGER NOT NULL UNIQUE,
    last_seen INTEGER AS Date,
    FOREIGN KEY(episode_id) REFERENCES episodes (_id)
    ON DELETE CASCADE
);
```

| Column | Type, constraint | Migration | Meaning |
|---|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY | base (columns renamed from `animehistory_*` by `115.sqm`) | |
| `episode_id` | INTEGER NOT NULL UNIQUE, FK `episodes(_id)` ON DELETE CASCADE | base | exactly one history row per episode |
| `last_seen` | INTEGER (nullable in the fresh `.sq`; NOT NULL after `115.sqm`) | base | ms; `0` is the "reset" marker: the row stays but disappears from the history screen |

`115.sqm` also dropped the old `time_seen` column. Note the manga counterpart keeps `time_read` (`data/src/main/sqldelight/data/history.sq`), anime has no watch-duration accumulator.

`data/src/main/sqldelightanime/dataanime/animehistory.sq:11-11`

```sql
CREATE INDEX animehistory_history_episode_id_index ON animehistory(episode_id);
```

`animehistory_history_episode_id_index`: duplicates the automatic index SQLite already creates for `UNIQUE (episode_id)`. Proposal: omit it.

Sibling statements (other chunks): `getHistoryByAnimeId` (`data/src/main/java/tachiyomi/data/history/anime/AnimeHistoryRepositoryImpl.kt:30`), `getHistoryByEpisodeUrl` (`AnimeRestorer.kt:364`), `resetAnimeHistoryById` (`:39`), `resetHistoryByAnimeId` (`:47`), `removeAllHistory` (`:55`), `removeResettedHistory` (`app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearAnimeDatabaseScreen.kt:296`, run by "clear database"), `upsert` (`:66`). The upsert is `INSERT INTO animehistory(episode_id, last_seen) VALUES (:episodeId, :seenAt) ON CONFLICT(episode_id) DO UPDATE SET last_seen = :seenAt WHERE episode_id = :episodeId;` and it is the only writer used by the player (`PlayerViewModel.kt:1792-1799`: `seenAt = Date()`, skipped in incognito mode) and by external-player return (`app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt:443-445`). Reset (`last_seen = 0`) is how "remove from history" works: `RemoveAnimeHistory.await(history)` (`domain/src/main/java/tachiyomi/domain/history/anime/interactor/RemoveAnimeHistory.kt:15`) resets one row, `await(animeId)` (`:19`) resets all rows of an anime, `awaitAll()` (`:11`) deletes every row.

Query `animehistory` (history screen list):

`data/src/main/sqldelightanime/view/animehistoryView.sq:28-44`

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

* Parameter: `:query` (text from the history search box; `null` becomes `""`, `AnimeHistoryScreenModel.kt:74`).
* Returns 10 columns: `id` (history `_id`), `animeId`, `episodeId`, `title`, `thumbnailUrl`, `source`, `favorite`, `cover_last_modified`, `episodeNumber`, `seenAt`. Mapped by `data/src/main/java/tachiyomi/data/history/anime/AnimeHistoryMapper.kt:19` into `AnimeHistoryWithRelations` with an `AnimeCover(animeId, sourceId, isAnimeFavorite, url = thumbnailUrl, lastModified = coverLastModified)`.
* Semantics: one row per anime, the one whose episode has the newest `last_seen` (`maxSeenAtEpisodeId = episodeId`); rows with `seenAt <= 0` or `NULL` (reset) are hidden; if an anime's newest row is reset to 0, the maximum falls to the next-newest non-reset row, so that older episode becomes the displayed row; an anime disappears only when all its rows are reset (maximum 0, removed by `seenAt > 0`) or deleted.
* Ordering: `seenAt DESC` only; two rows with the same millisecond have undefined relative order. No `LIMIT`; the UI inserts day headers by local date (`AnimeHistoryScreenModel.kt:93-105`).
* Filter: `lower(title) LIKE '%' || :query || '%'`. `%` and `_` typed by the user act as wildcards (no `ESCAPE`); SQLite `lower()` and `LIKE` fold ASCII only, so non-ASCII titles match case-sensitively.
* Callers: `data/src/main/java/tachiyomi/data/history/anime/AnimeHistoryRepositoryImpl.kt:16-20` (`getAnimeHistory(query)` as a Flow), `domain/src/main/java/tachiyomi/domain/history/anime/interactor/GetAnimeHistory.kt:17`, `app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryScreenModel.kt:74`. Sibling `getLatestAnimeHistory` (`:24`, used by `GetNextEpisodes.kt:17` for "resume last watched").
* Reads tables: `animes`, `episodes`, `animehistory`.
* Perf: the view's inner grouped subquery scans the whole `animehistory` join each run; with 10,000 history rows that is a few milliseconds. The Flow re-runs on every playback write (once per second while watching) in Aniyomi.

Proposal: Rust DDL and function:

```sql
CREATE TABLE animehistory(
    _id INTEGER NOT NULL PRIMARY KEY,
    episode_id INTEGER NOT NULL UNIQUE,
    last_seen INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(episode_id) REFERENCES episodes (_id) ON DELETE CASCADE
);
```

```rust
pub struct AnimeHistoryWithRelations {
    pub id: i64, pub anime_id: i64, pub episode_id: i64, pub title: String,
    pub thumbnail_url: Option<String>, pub source: i64, pub favorite: bool,
    pub cover_last_modified: i64, pub episode_number: f64, pub seen_at: i64,
}
/// `query` is the raw user text; wildcards are escaped here (improvement over Aniyomi).
pub fn animehistory(conn: &rusqlite::Connection, query: &str) -> rusqlite::Result<Vec<AnimeHistoryWithRelations>> {
    let pattern = format!("%{}%", query.to_lowercase().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let mut st = conn.prepare_cached(
        "SELECT id, animeId, episodeId, title, thumbnailUrl, source, favorite, cover_last_modified, episodeNumber, seenAt
         FROM animehistoryView
         WHERE seenAt > 0 AND maxSeenAtEpisodeId = episodeId AND lower(title) LIKE :pattern ESCAPE '\\'
         ORDER BY seenAt DESC, id DESC")?;
    st.query_map(rusqlite::named_params! { ":pattern": pattern }, |r| Ok(AnimeHistoryWithRelations {
        id: r.get("id")?, anime_id: r.get("animeId")?, episode_id: r.get("episodeId")?, title: r.get("title")?,
        thumbnail_url: r.get("thumbnailUrl")?, source: r.get("source")?, favorite: r.get("favorite")?,
        cover_last_modified: r.get("cover_last_modified")?, episode_number: r.get("episodeNumber")?, seen_at: r.get("seenAt")?,
    }))?.collect()
}
```

(`to_lowercase()` on the Rust side lower-cases non-ASCII too, but SQL `lower(title)` still ASCII-only; to get full Unicode case-insensitivity register a scalar function `lower_u` with `Connection::create_scalar_function("lower_u", 1, FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC, |ctx| Ok(ctx.get::<String>(0)?.to_lowercase()))` and use `lower_u(title)` instead of `lower(title)`. The `id DESC` tiebreak is an addition.)

`upsert` signature: `pub fn upsert_anime_history(conn: &rusqlite::Connection, episode_id: i64, seen_at_ms: i64) -> rusqlite::Result<()>` with the statement `INSERT INTO animehistory(episode_id, last_seen) VALUES (:episode_id, :seen_at) ON CONFLICT(episode_id) DO UPDATE SET last_seen = excluded.last_seen`.

### 2.5 `anime_sync` (table)

OBSERVED. Source: `data/src/main/sqldelightanime/dataanime/anime_sync.sq`. One row per (anime, tracker) pair: the local copy of the remote list entry of a tracking service (AniList, MyAnimeList, Kitsu, Shikimori, Bangumi, Simkl, enhanced trackers; see area:27-tracking-core).

`data/src/main/sqldelightanime/dataanime/anime_sync.sq:3-21`

```sql
CREATE TABLE anime_sync(
    _id INTEGER NOT NULL PRIMARY KEY,
    anime_id INTEGER NOT NULL,
    sync_id INTEGER NOT NULL,
    remote_id INTEGER NOT NULL,
    library_id INTEGER,
    title TEXT NOT NULL,
    last_episode_seen REAL NOT NULL,
    total_episodes INTEGER NOT NULL,
    status INTEGER NOT NULL,
    score REAL NOT NULL,
    remote_url TEXT NOT NULL,
    start_date INTEGER NOT NULL,
    finish_date INTEGER NOT NULL,
    private INTEGER AS Boolean DEFAULT 0 NOT NULL,
    UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE,
    FOREIGN KEY(anime_id) REFERENCES animes (_id)
    ON DELETE CASCADE
);
```

| Column | Type, constraint | Migration | Meaning |
|---|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY | base | local track id (used by the delayed tracking job, `app/src/main/java/eu/kanade/domain/track/anime/service/DelayedAnimeTrackingUpdateJob.kt:39`) |
| `anime_id` | INTEGER NOT NULL, FK `animes(_id)` ON DELETE CASCADE | base | |
| `sync_id` | INTEGER NOT NULL | base | tracker id (the Kotlin field is `trackerId`) |
| `remote_id` | INTEGER NOT NULL | base | id of the media on the tracker (the `update` statement calls the parameter `mediaId`) |
| `library_id` | INTEGER | base | tracker-side library entry id; `NULL` when the tracker has none |
| `title` | TEXT NOT NULL | base | remote title |
| `last_episode_seen` | REAL NOT NULL | base | remote progress; REAL because fractional episodes exist |
| `total_episodes` | INTEGER NOT NULL | base | `0` when unknown |
| `status` | INTEGER NOT NULL | base | tracker-specific status code |
| `score` | REAL NOT NULL | base | tracker-specific scale |
| `remote_url` | TEXT NOT NULL | base | |
| `start_date` | INTEGER NOT NULL | base | ms epoch, `0` = unset |
| `finish_date` | INTEGER NOT NULL | base | ms epoch, `0` = unset |
| `private` | INTEGER NOT NULL DEFAULT 0 (Boolean) | 130 | tracker "private entry" flag |

Constraint `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE`: inserting a second row for the same anime and tracker silently deletes the old row and inserts the new one with a NEW `_id` (the `insert` statement does not name `_id`, `data/src/main/java/tachiyomi/data/track/anime/AnimeTrackRepositoryImpl.kt:54`). Consequence in Aniyomi: every `insertAnime(track)` used to persist a refreshed track changes its `_id`, so a `DelayedAnimeTrackingUpdateJob` scheduled with the old `trackId` finds nothing (`GetAnimeTracks.awaitOne` returns `null`, `domain/src/main/java/tachiyomi/domain/track/anime/interactor/GetAnimeTracks.kt:13-20`, and `DelayedAnimeTrackingUpdateJob.kt:38-42` then removes the queued item from `DelayedAnimeTrackingStore`, losing that pending progress update). Note the misleading name: `getTrackByAnimeId` selects `WHERE _id = :id`, i.e. by track id.

Statements of the file catalogued in other chunks: `delete` (item 28, section 5.2), `getAnimeTracks` (`AnimeTrackRepositoryImpl.kt:24`, all rows, observed as a Flow by `GetTracksPerAnime.kt:13` which groups by `animeId`), `getTrackByAnimeId` (`:13`), `getTracksByAnimeId` (`:18,30`; also `AnimeBackupCreator.kt:54`), `insert` (`:54`, via `InsertAnimeTrack.kt:14,22` which swallows exceptions), `update` (only `AnimeRestorer.kt:433`; every column uses `coalesce(:param, col)`; remote_id param `:mediaId`, remote_url param `:trackingUrl`).

Perf: tiny table (a few rows per library entry). `UNIQUE (anime_id, sync_id)` provides the only index; lookups by `anime_id` use its prefix.

Proposal: Rust DDL and an `_id`-preserving upsert:

```sql
CREATE TABLE anime_sync(
    _id INTEGER NOT NULL PRIMARY KEY,
    anime_id INTEGER NOT NULL,
    sync_id INTEGER NOT NULL,
    remote_id INTEGER NOT NULL,
    library_id INTEGER,
    title TEXT NOT NULL,
    last_episode_seen REAL NOT NULL,
    total_episodes INTEGER NOT NULL,
    status INTEGER NOT NULL,
    score REAL NOT NULL,
    remote_url TEXT NOT NULL,
    start_date INTEGER NOT NULL,
    finish_date INTEGER NOT NULL,
    private INTEGER NOT NULL DEFAULT 0,
    UNIQUE (anime_id, sync_id),
    FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE
);
```

```rust
pub fn upsert_track(conn: &rusqlite::Connection, t: &AnimeTrack) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO anime_sync(anime_id, sync_id, remote_id, library_id, title, last_episode_seen, total_episodes, status, score, remote_url, start_date, finish_date, private)
         VALUES (:anime_id, :sync_id, :remote_id, :library_id, :title, :last_episode_seen, :total_episodes, :status, :score, :remote_url, :start_date, :finish_date, :private)
         ON CONFLICT(anime_id, sync_id) DO UPDATE SET
           remote_id = excluded.remote_id, library_id = excluded.library_id, title = excluded.title,
           last_episode_seen = excluded.last_episode_seen, total_episodes = excluded.total_episodes,
           status = excluded.status, score = excluded.score, remote_url = excluded.remote_url,
           start_date = excluded.start_date, finish_date = excluded.finish_date, private = excluded.private",
        rusqlite::named_params! { ":anime_id": t.anime_id, ":sync_id": t.tracker_id, ":remote_id": t.remote_id,
            ":library_id": t.library_id, ":title": t.title, ":last_episode_seen": t.last_episode_seen,
            ":total_episodes": t.total_episodes, ":status": t.status, ":score": t.score,
            ":remote_url": t.remote_url, ":start_date": t.start_date, ":finish_date": t.finish_date, ":private": t.private })?;
    Ok(())
}
```

(Improvement: `_id` stays stable on refresh, so delayed-update records remain valid. The backup restore path keeps using plain field updates.)

### 2.6 `anime_relations` (table), `anime_relations_anime_id_index`, `anime_relations_related_anime_id_index`

OBSERVED. Source: `data/src/main/sqldelightanime/dataanime/anime_relations.sq`, created by `data/src/main/sqldelightanime/migrations/138.sqm` (the newest table of the anime database). It caches "related anime" groups ("Sequels", "Recommendations", names chosen by the source) shown on the details screen. A source opts in with `supportsRelatedAnime` and `getRelatedAnimeList` (area:16-source-api-anime).

`data/src/main/sqldelightanime/dataanime/anime_relations.sq:1-12`

```sql
CREATE TABLE anime_relations(
    anime_id INTEGER NOT NULL,
    related_anime_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    last_fetched_at INTEGER NOT NULL,
    PRIMARY KEY(anime_id, related_anime_id, name),
    FOREIGN KEY(anime_id) REFERENCES animes (_id)
    ON DELETE CASCADE,
    FOREIGN KEY(related_anime_id) REFERENCES animes (_id)
    ON DELETE CASCADE
);
```

| Column | Type, constraint | Meaning |
|---|---|---|
| `anime_id` | INTEGER NOT NULL, FK `animes(_id)` ON DELETE CASCADE, part of PK | the anime that owns the list |
| `related_anime_id` | INTEGER NOT NULL, FK `animes(_id)` ON DELETE CASCADE, part of PK | the related anime (a normal `animes` row, usually `favorite = 0`) |
| `name` | TEXT NOT NULL, part of PK | group title shown above the group |
| `sort_order` | INTEGER NOT NULL | global position across all groups (a single counter running over groups and members) |
| `last_fetched_at` | INTEGER NOT NULL | ms; equal for every row written by one refresh |

Primary key `(anime_id, related_anime_id, name)`: the same related anime may appear in two groups of one anime. There is no `_id` column. Deleting either anime removes the relation row, so an old suggestion silently vanishes when its anime is cleared from the database.

Refresh algorithm (OBSERVED, `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncRelatedAnimeWithSource.kt:18-42`): return unless the source `supportsRelatedAnime`; unless forced, return if a row exists and `now - last_fetched_at < TTL` with `TTL = 7.days` (604,800,000 ms); for each group returned by the source: `distinctBy { url }`, drop the anime's own url, `take(MAX_PER_GROUP = 10)`, convert each to a local `animes` row through `NetworkToLocalAnime.await` (which inserts a non-favorite row when unknown); drop groups with no members; then `replaceRelations`, which runs in one transaction: `deleteByAnimeId`, then one `insert` per member with `sort_order` counting up from 0 across all groups (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRelationRepositoryImpl.kt:17-37`). The UI Flow groups the ordered rows by `name` keeping first-seen order (`:39-52`).

`data/src/main/sqldelightanime/dataanime/anime_relations.sq:14-14`

```sql
CREATE INDEX anime_relations_anime_id_index ON anime_relations(anime_id);
```

`anime_relations_anime_id_index`: its column is the leading column of the primary key, whose automatic index already serves `getRelatedAnimeByAnimeId`, `getLastFetchedAt` and `deleteByAnimeId`. Redundant. Proposal: omit.

`data/src/main/sqldelightanime/dataanime/anime_relations.sq:15-15`

```sql
CREATE INDEX anime_relations_related_anime_id_index ON anime_relations(related_anime_id);
```

`anime_relations_related_anime_id_index`: needed. Without it, deleting an `animes` row would scan the whole relations table to enforce the second foreign key. Keep.

Sibling statements (other chunks): `getRelatedAnimeByAnimeId` (`SELECT A.*, R.name FROM anime_relations AS R JOIN animes AS A ON A._id = R.related_anime_id WHERE R.anime_id = :animeId ORDER BY R.sort_order ASC`, `AnimeRelationRepositoryImpl.kt:42`, consumed by `GetRelatedAnime.kt:11` and `AnimeScreenModel.kt:1463`), `getLastFetchedAt` (`:14`), `insert` (`INSERT OR REPLACE`, `:27`), and `deleteByAnimeId` (item 31, section 5.5).

Proposal: Rust DDL:

```sql
CREATE TABLE anime_relations(
    anime_id INTEGER NOT NULL,
    related_anime_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    last_fetched_at INTEGER NOT NULL,
    PRIMARY KEY(anime_id, related_anime_id, name),
    FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE,
    FOREIGN KEY(related_anime_id) REFERENCES animes (_id) ON DELETE CASCADE
);
CREATE INDEX anime_relations_related_anime_id_index ON anime_relations(related_anime_id);
```

Source-side support is optional for P0; the whole feature can ship after the details screen (P2).

### 2.7 `animesources` (table)

OBSERVED. Source: `data/src/main/sqldelightanime/dataanime/animesources.sq`, migration `117.sqm`. A persistent registry of "stub" sources: the id, language and name of every anime source ever installed, so that library entries, history and downloads of an uninstalled extension still show a source name instead of a number (`app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt:75-79,106-125`).

`data/src/main/sqldelightanime/dataanime/animesources.sq:1-5`

```sql
CREATE TABLE animesources(
    _id INTEGER NOT NULL PRIMARY KEY,
    lang TEXT NOT NULL,
    name TEXT NOT NULL
);
```

| Column | Type | Meaning |
|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY | source id (the 64-bit source id, NOT autogenerated: `upsert` supplies it) |
| `lang` | TEXT NOT NULL | language code, `""` for an unknown stub |
| `name` | TEXT NOT NULL | display name |

Statements (other chunks): `findAll` (`data/src/main/java/tachiyomi/data/source/anime/AnimeStubSourceRepositoryImpl.kt:13`, Flow), `findOne` (`:18`), `upsert` (`:26`; `INSERT INTO animesources(_id, lang, name) VALUES (:id, :lang, :name) ON CONFLICT(_id) DO UPDATE SET lang = :lang, name = :name WHERE _id = :id`). Behaviour: when an installed source's `(lang, name)` differs from the stored stub, `registerStubSource` upserts it and renames the download folders (`AndroidAnimeSourceManager.kt:106-113`, `downloadManager.renameSource`). A source id with no stub resolves to `StubAnimeSource(id, lang = "", name = "")` (`:125`).

Proposal: keep the table with the same DDL; it is what makes "Source not installed (id)" rows readable and keeps download directory names stable (area:24-downloads-anime). The manga twin is `sources` (`data/src/main/sqldelight/data/sources.sq`), chunk 2.

```sql
CREATE TABLE animesources(
    _id INTEGER NOT NULL PRIMARY KEY,
    lang TEXT NOT NULL,
    name TEXT NOT NULL
);
```

### 2.8 `categories` (table, in both databases)

OBSERVED. Sources: `data/src/main/sqldelightanime/dataanime/categories.sq` (anime database) and `data/src/main/sqldelight/data/categories.sq` (manga database). The two files are byte-identical except for four statements: the by-entry joins read `animes_categories` (`getCategoriesByAnimeId`, `getVisibleCategoriesByAnimeId`, parameter `:animeId`) versus `mangas_categories` (`getCategoriesByMangaId`, `getVisibleCategoriesByMangaId`, parameter `:mangaId`). Anime and manga categories are therefore separate lists with separate ids; a category named "Reading" in one database has nothing to do with one of the same name in the other.

`data/src/main/sqldelightanime/dataanime/categories.sq:1-7`

```sql
CREATE TABLE categories(
    _id INTEGER NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    sort INTEGER NOT NULL,
    flags INTEGER NOT NULL,
    hidden INTEGER NOT NULL DEFAULT 0
);
```

| Column | Type, constraint | Migration | Meaning |
|---|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY | base | `0` is the system "Default" category; user categories get auto ids >= 1 |
| `name` | TEXT NOT NULL | base | |
| `sort` | INTEGER NOT NULL | base | display order; the system category has `-1`; the Kotlin field is called `order`, produced by the SQL alias ``sort AS `order` `` |
| `flags` | INTEGER NOT NULL | base | library sort bits, layout below |
| `hidden` | INTEGER NOT NULL DEFAULT 0 | anime 125, manga 25 | `1` hides the category tab |

Seed row and protection trigger (run at table creation):

`data/src/main/sqldelightanime/dataanime/categories.sq:9-18`

```sql
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

The system category row `(0, "", -1, 0)` is inserted with `INSERT OR IGNORE`. The trigger aborts any `DELETE` of a row with `_id <= 0` with the message `System category can't be deleted`; Kotlin callers catch the exception (`DeleteAnimeCategory.kt:19-23`).

`flags` layout (OBSERVED, `domain/src/main/java/tachiyomi/domain/library/anime/model/AnimeLibrarySortMode.kt:21-62`; manga twin `domain/src/main/java/tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt:21-55`): bits 2-5 (`mask = 0b00111100 = 0x3C`) hold the sort type, bit 6 (`0b01000000 = 0x40`) the direction (`0x40` ascending, `0x00` descending). Bits 0, 1 and 7+ are unused. Display mode is not stored in `flags` (it is the preference `displayMode`).

| Type (anime) | Value | Type (manga) | Value |
|---|---|---|---|
| Alphabetical | `0x00` | Alphabetical | `0x00` |
| LastSeen | `0x04` | LastRead | `0x04` |
| LastUpdate | `0x08` | LastUpdate | `0x08` |
| UnseenCount | `0x0C` | UnreadCount | `0x0C` |
| TotalEpisodes | `0x10` | TotalChapters | `0x10` |
| LatestEpisode | `0x14` | LatestChapter | `0x14` |
| EpisodeFetchDate | `0x18` | ChapterFetchDate | `0x18` |
| DateAdded | `0x1C` | DateAdded | `0x1C` |
| TrackerMean | `0x20` | TrackerMean | `0x20` |
| AiringTime | `0x30` | (none) | |
| Random | `0x3C` | Random | `0x3C` |

(The source writes TrackerMean as `0b000100000`, which is `0x20`.) Decoding: `type = types.find { it.flag == flags & 0x3C } ?: Alphabetical`, `direction = if flags & 0x40 != 0 then Ascending else Descending`. The default for a new category is the global preference (`animeSortingMode`, default Alphabetical ascending, i.e. `0x40`): `CreateAnimeCategoryWithName.kt:15-19`. Setting a sort mode either rewrites one category (when the preference "categorized display settings" is on and a category is given) or rewrites the global preference and every category via `updateAllFlags` (`SetSortModeForAnimeCategory.kt:16-37`); picking Random also stores a new random seed.

Statements in this file other than the item `delete` (section 5.2): `getCategory` (`WHERE _id = :id LIMIT 1`, `data/src/main/java/tachiyomi/data/category/anime/AnimeCategoryRepositoryImpl.kt:15`), `getCategories` (`ORDER BY sort`, includes the system row, `:19,27`), `getVisibleCategories` (`WHERE hidden = 0 ORDER BY sort`, `:23,31`), the four by-entry joins (`:36-54`; no `ORDER BY`), `insert` (`INSERT INTO categories(name, sort, flags) VALUES (:name, :order, :flags)`, `:60`), `update` (coalesce of name/sort/flags/hidden `WHERE _id = :categoryId`, `:83`), `updateAllFlags` (`UPDATE categories SET flags = coalesce(?, flags)`, no WHERE, `:94`), `selectLastInsertedRowId`.

Behaviour of the interactors that matter for the schema:
* Create (`domain/src/main/java/tachiyomi/domain/category/anime/interactor/CreateAnimeCategoryWithName.kt:21`): `order = max(existing order) + 1` over ALL rows including the system row (so the first user category gets `0`), `flags = initial sort flags`, `hidden = false`.
* Reorder (`ReorderAnimeCategory.kt:18`): serialised by a mutex, excludes the system category, moves the item in the list, rewrites `sort` as the list index `0..n-1`.
* Delete (`DeleteAnimeCategory.kt:17`): deletes the row, then renumbers EVERY remaining row including the system one with `mapIndexed` (so after one delete the system category's `sort` becomes `0` and user categories shift by one; order among them is unchanged because the system row sorted first), removes the id from the default-category preference and from five category-id-set preferences (`animeUpdateCategories`, `animeUpdateCategoriesExclude`, `removeExcludeAnimeCategories`, `downloadNewEpisodeCategories`, `downloadNewEpisodeCategoriesExclude`), writes the renumbering. Link rows disappear through the FK cascade. Backup export skips the system category (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeCategoriesBackupCreator.kt:16`) and the category management screen lists user categories only (`app/src/main/java/eu/kanade/tachiyomi/ui/category/anime/AnimeCategoryScreenModel.kt:53`).
* Hide (`HideAnimeCategory.kt:14`): toggles `hidden`.
* Backup restore (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeCategoriesRestorer.kt:16-42`): matches categories by exact name, appends missing ones with order `max+1`, `flags` from the backup; afterwards sets `categorizedDisplaySettings = (distinct flag values among all categories) > 1`.

Perf: a handful of rows; no index needed.

Proposal: Rust DDL, identical in `anime.db` and `manga.db`:

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

Delete renumbering fix: renumber only non-system categories (`sort = index` over rows with `_id > 0`) so the system row keeps `-1`; ordering is unchanged and the system row can never collide with a user row (area:37-ui-library owns the category screen). The user-visible message key is whatever the localisation maps to `System category can't be deleted`; since the UI never offers delete for category 0, the trigger is a safety net only.

Rust signatures: `pub fn get_categories(conn: &Connection) -> Result<Vec<Category>>` (`SELECT _id AS id, name, sort AS "order", flags, hidden FROM categories ORDER BY sort`), `pub fn get_visible_categories(conn)` (same with `WHERE hidden = 0`), `pub fn get_categories_by_anime_id(conn, anime_id: i64)` and `get_visible_categories_by_anime_id` (the two joins verbatim; Proposal adds `ORDER BY C.sort` so the picker order is deterministic).

### 2.9 `custom_buttons` (table and seed row)

OBSERVED. Source: `data/src/main/sqldelightanime/dataanime/custom_buttons.sq`, migration `129.sqm`. User-defined buttons shown in the video player controls; each stores three Lua snippets executed inside mpv. Only in the anime database.

`data/src/main/sqldelightanime/dataanime/custom_buttons.sq:3-11`

```sql
CREATE TABLE custom_buttons (
    _id INTEGER NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    isFavorite INTEGER AS Boolean NOT NULL,
    sortIndex INTEGER NOT NULL,
    content TEXT NOT NULL,
    longPressContent TEXT NOT NULL,
    onStartup TEXT NOT NULL
);
```

| Column | Type | Meaning |
|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY | button id; substituted for the placeholder `$id` in the scripts |
| `name` | TEXT NOT NULL | label (the seed button is renamed live by its own script to `+<n> s`) |
| `isFavorite` | INTEGER NOT NULL (Boolean) | at most one button is the "primary" (shown in the main control row); the placeholder `$isPrimary` is replaced by `true` or `false` |
| `sortIndex` | INTEGER NOT NULL | 0-based order |
| `content` | TEXT NOT NULL | Lua run on tap |
| `longPressContent` | TEXT NOT NULL | Lua run on long press |
| `onStartup` | TEXT NOT NULL | Lua run when the player starts |

Placeholder substitution (`domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt:12-26`): in each of the three scripts `"$id"` is replaced by the button `_id` as decimal, then `"$isPrimary"` by `"true"` when `primaryId == id` else `"false"` (plain string replace, in that order).

Seed row inserted with the table (and by `129.sqm` on upgrade):

`data/src/main/sqldelightanime/dataanime/custom_buttons.sq:13-30`

```sql
-- Insert skip intro button --
INSERT OR IGNORE INTO custom_buttons (_id, name, isFavorite, sortIndex, content, longPressContent, onStartup)
VALUES (1, '+85 s', 1, 0, 'local intro_length = mp.get_property_number("user-data/current-anime/intro-length")
aniyomi.right_seek_by(intro_length)', 'aniyomi.int_picker("Change intro length", "%ds", 0, 255, 1, "user-data/current-anime/intro-length")', 'function update_button(_, length)
  if length ~= nil then
    if length == 0 then
	  aniyomi.hide_button()
	  return
	else
	  aniyomi.show_button()
	end
    aniyomi.set_button_title("+" .. length .. " s")
  end
end

if $isPrimary then
  mp.observe_property("user-data/current-anime/intro-length", "number", update_button)
end');
```

It is the "skip intro" button: tap seeks forward by `user-data/current-anime/intro-length` seconds (the property is set from the anime's `viewer` flags, `PlayerViewModel.kt:1286`; new anime take the preference `pref_default_intro_length`, default 85, `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt:21`; the seed label `+85 s` matches that default; changes made with the picker are written back through `PlayerActivity.kt:677` and `SetAnimeViewerFlags.awaitSetSkipIntroLength`), long press opens an integer picker 0..255 step 1 titled "Change intro length" with format `%ds`, and the startup script hides the button when the length is 0 and re-titles it on every change. The `aniyomi.*` functions and the `user-data/current-anime/*` properties are provided by the app's mpv Lua prelude (area:48-settings-player-and-scripts, area:45-player-core).

Statements (other chunks): `findAll` (`SELECT * FROM custom_buttons ORDER BY sortIndex`, `data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt:16,20`), `insert` (`:31`, passes `isFavorite = false`), `delete` (item 28, section 5.2), `update` (coalesce, `:56`), `selectLastInsertedRowId`.

Interactor behaviour (OBSERVED, `domain/src/main/java/tachiyomi/domain/custombuttons/interactor/`): create appends with `sortIndex = max + 1` (or 0); delete removes then renumbers `sortIndex = index` for all remaining rows (`DeleteCustomButton.kt:12`); reorder moves in memory and rewrites all indexes under a mutex (`ReorderCustomButton.kt`); toggle favorite: when turning ON, writes `isFavorite = (id == target)` for ALL buttons in one transaction, so at most one stays favourite; turning OFF clears only the one (`ToggleFavoriteCustomButton.kt:17`). Backup restore (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/CustomButtonRestorer.kt:13-43`): skips buttons whose `name` already exists, appends the others after the current max `sortIndex` in backup order, and makes a restored button favourite only if it was favourite in the backup and no favourite exists yet.

Proposal: Rust DDL identical to the above. Because Kurobayomi plays through libmpv, `content`, `longPressContent` and `onStartup` are executed by the same mpv Lua runtime when libmpv is built with Lua support; the seed row relies on the `aniyomi.*` shim, which must therefore be shipped with the player (area:45-player-core). If the Rust player drops mpv scripting, replace `custom_buttons` by a fixed "skip intro" button with a persisted intro length and list the loss (user-defined Lua buttons) as a P3 decision in area:48-settings-player-and-scripts; the table and its seed row are still created so Aniyomi backups restore without error.

```sql
CREATE TABLE custom_buttons (
    _id INTEGER NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    isFavorite INTEGER NOT NULL,
    sortIndex INTEGER NOT NULL,
    content TEXT NOT NULL,
    longPressContent TEXT NOT NULL,
    onStartup TEXT NOT NULL
);
-- seed row: copy the INSERT OR IGNORE statement above byte for byte (the Lua text contains tabs)
```

## 3. Manga database: `chapters` and `excluded_scanlators` (`data/src/main/sqldelight`)

### 3.1 `chapters` (table), `chapters_manga_id_index`, `chapters_unread_by_manga_index`, and the two `chapters` triggers

OBSERVED. Source: `data/src/main/sqldelight/data/chapters.sq`. The manga twin of `episodes` (2.3). One row per chapter of a manga; created and reconciled by `SyncChaptersWithSource` (`app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt`, remove `:211`, add `:215`, update `:220`).

`data/src/main/sqldelight/data/chapters.sq:3-21`

```sql
CREATE TABLE chapters(
    _id INTEGER NOT NULL PRIMARY KEY,
    manga_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    name TEXT NOT NULL,
    scanlator TEXT,
    read INTEGER AS Boolean NOT NULL,
    bookmark INTEGER AS Boolean NOT NULL,
    last_page_read INTEGER NOT NULL,
    chapter_number REAL NOT NULL,
    source_order INTEGER NOT NULL,
    date_fetch INTEGER NOT NULL,
    date_upload INTEGER NOT NULL,
    last_modified_at INTEGER NOT NULL DEFAULT 0,
    version INTEGER NOT NULL DEFAULT 0,
    is_syncing INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id)
    ON DELETE CASCADE
);
```

| Column | Type, constraint | Kotlin | Migration | Meaning |
|---|---|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY | Long | base | |
| `manga_id` | INTEGER NOT NULL, FK `mangas(_id)` ON DELETE CASCADE | Long | base | |
| `url` | TEXT NOT NULL | String | base | identity inside the manga |
| `name` | TEXT NOT NULL | String | base | sanitised like episode names (`data/src/main/java/tachiyomi/data/items/chapter/ChapterSanitizer.kt`) |
| `scanlator` | TEXT | String? | `4.sqm` | group name; `28.sqm` trimmed all existing values (`UPDATE chapters SET scanlator = trim(scanlator) WHERE scanlator IS NOT NULL`) |
| `read` | INTEGER NOT NULL | Boolean | base | |
| `bookmark` | INTEGER NOT NULL | Boolean | `3.sqm` | |
| `last_page_read` | INTEGER NOT NULL | Long | base | 0-based page index of the last page shown (`ReaderViewModel.kt:528-552` stores `page.index` on every page change) |
| `chapter_number` | REAL NOT NULL | Double | base | `< 0` means unrecognised (`domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt:19-20`) |
| `source_order` | INTEGER NOT NULL | Long | `1.sqm` | |
| `date_fetch` | INTEGER NOT NULL | Long | base | ms |
| `date_upload` | INTEGER NOT NULL | Long | base (`13.sqm` copied `date_fetch` into every `date_upload = 0`) | ms |
| `last_modified_at` | INTEGER NOT NULL DEFAULT 0 | Long | `26.sqm` | seconds, trigger |
| `version` | INTEGER NOT NULL DEFAULT 0 | Long | `30.sqm` | trigger |
| `is_syncing` | INTEGER NOT NULL DEFAULT 0 | Long | `30.sqm` | |

The table was rebuilt by `14.sqm` (types `AS Boolean`, `AS Float`, `AS Long` added, FK added); all later columns come from `ALTER TABLE ADD COLUMN`.

`data/src/main/sqldelight/data/chapters.sq:23-23`

```sql
CREATE INDEX chapters_manga_id_index ON chapters(manga_id);
```

`chapters_manga_id_index`: serves every `WHERE manga_id = ?` statement, the `libraryView` aggregate, the join in `getChaptersByMangaId`, and the cascade from `mangas`.

`data/src/main/sqldelight/data/chapters.sq:24-24`

```sql
CREATE INDEX chapters_unread_by_manga_index ON chapters(manga_id, read) WHERE read = 0;
```

`chapters_unread_by_manga_index`: partial index created by `7.sqm`, recreated by `14.sqm`. No statement in the `.sq` files has the literal `read = 0` (the library view sums `read`), so the planner never uses it. Proposal: omit from the Rust DDL.

`data/src/main/sqldelight/data/chapters.sq:26-33`

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

`update_last_modified_at_chapters`: stamps `last_modified_at` (seconds) after each `UPDATE`.

`data/src/main/sqldelight/data/chapters.sq:35-49`

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

`update_chapter_and_manga_version`: when the chapter is not syncing and `read`, `bookmark` or `last_page_read` changed, bumps `chapters.version` and (unless the manga is syncing) `mangas.version`. Because `last_page_read` changes on every page turn, the reader causes the same trigger fan-out as the player (2.3), at page-turn frequency instead of once per second.

Callers: `data/src/main/java/tachiyomi/data/items/chapter/ChapterRepositoryImpl.kt` (`addAllChapters` `:16`, insert `:20`, `last_insert_rowid()` `:34`; update `:55`; `removeChaptersWithIds` `:77`; `getChaptersByMangaId` `:85,116`; `getScanlatorsByMangaId` `:91,97`; `getBookmarkedChaptersByMangaId` `:103`; `getChapterById` `:111`; `getChapterByUrlAndMangaId` `:122`); progress writes `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:544-550` (skipped in incognito mode and when the page is in error state); `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt:41`; backup `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt:195,216`. Statements of the file catalogued in other chunks: `getChapterById`, `getChaptersByMangaId` (the only statement that joins `excluded_scanlators`, see 3.2), `getScanlatorsByMangaId`, `getBookmarkedChaptersByMangaId`, `getChapterByUrl`, `getChapterByUrlAndMangaId`, `removeChaptersWithIds`, `resetIsSyncing`, `insert`, `update`, `selectLastInsertedRowId`. The scanlator filter is applied (`applyScanlatorFilter = true`) by the manga details screen (`MangaScreenModel.kt:176,217`), the reader (`ReaderViewModel.kt:156`) and "next unread" (`MangaLibraryScreenModel.kt:437`); backup passes `0` (`MangaBackupCreator.kt:42`).

Proposal: Rust DDL (manga database; `chapters_unread_by_manga_index` dropped):

```sql
CREATE TABLE chapters(
    _id INTEGER NOT NULL PRIMARY KEY,
    manga_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    name TEXT NOT NULL,
    scanlator TEXT,
    read INTEGER NOT NULL,
    bookmark INTEGER NOT NULL,
    last_page_read INTEGER NOT NULL,
    chapter_number REAL NOT NULL,
    source_order INTEGER NOT NULL,
    date_fetch INTEGER NOT NULL,
    date_upload INTEGER NOT NULL,
    last_modified_at INTEGER NOT NULL DEFAULT 0,
    version INTEGER NOT NULL DEFAULT 0,
    is_syncing INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE
);
CREATE INDEX chapters_manga_id_index ON chapters(manga_id);
-- both triggers created verbatim from the SQL above
```

The reader keeps the current page in memory and writes `last_page_read` at most once per second, on chapter change, on reader exit and when the last page is reached (the `read = true` transition is written immediately); area:43-reader-core owns the reader side. Bulk insert uses `INSERT INTO chapters(manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload, last_modified_at, version, is_syncing) VALUES (:manga_id, :url, :name, :scanlator, :read, :bookmark, :last_page_read, :chapter_number, :source_order, :date_fetch, :date_upload, 0, :version, 0) RETURNING _id`.

### 3.2 `excluded_scanlators` (table)

OBSERVED. Source: `data/src/main/sqldelight/data/excluded_scanlators.sq`, migration `27.sqm` (which also rebuilt `libraryView` so that unread/total counts ignore excluded groups, `data/src/main/sqldelight/view/libraryView.sq:13-26`). Per-manga list of scanlator groups whose chapters the user hides. Manga only; anime has no scanlator filter.

`data/src/main/sqldelight/data/excluded_scanlators.sq:1-6`

```sql
CREATE TABLE excluded_scanlators(
    manga_id INTEGER NOT NULL,
    scanlator TEXT NOT NULL,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id)
    ON DELETE CASCADE
);
```

| Column | Type, constraint | Meaning |
|---|---|---|
| `manga_id` | INTEGER NOT NULL, FK `mangas(_id)` ON DELETE CASCADE | |
| `scanlator` | TEXT NOT NULL | exact group name; matched with `=` against `chapters.scanlator` |

No primary key, no UNIQUE: a duplicate row is harmless but possible. Index `excluded_scanlators_manga_id_index ON excluded_scanlators(manga_id)` (`data/src/main/sqldelight/data/excluded_scanlators.sq:8`) is created in the same file (catalogued with the other indexes in chunk 2).

How the filter works (OBSERVED, `getChaptersByMangaId`, `data/src/main/sqldelight/data/chapters.sq:56-66`): the whole statement is:

`data/src/main/sqldelight/data/chapters.sq:56-66`

```sql
getChaptersByMangaId:
SELECT C.*
FROM chapters C
LEFT JOIN excluded_scanlators ES
ON C.manga_id = ES.manga_id
AND C.scanlator = ES.scanlator
WHERE C.manga_id = :mangaId
AND (
    :applyScanlatorFilter = 0
    OR ES.scanlator IS NULL
);
```

 A chapter with `scanlator IS NULL` never equals anything, so chapters without a group are never excluded. The list the user edits comes from `getScanlatorsByMangaId`, which returns the column once per chapter (duplicates); `GetAvailableScanlators` removes blank values and de-duplicates with a set (`app/src/main/java/eu/kanade/domain/items/chapter/interactor/GetAvailableScanlators.kt:9-22`); the Kotlin mapper turns `NULL` into `""` before that (`ChapterRepositoryImpl.kt:91`).

Callers: `app/src/main/java/eu/kanade/domain/entries/manga/interactor/GetExcludedScanlators.kt:13,20` (`getExcludedScanlatorsByMangaId`, `SELECT scanlator FROM excluded_scanlators WHERE manga_id = :mangaId`, as a set and as a Flow), `app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetExcludedScanlators.kt:12-19` (one transaction: read current set; `insert` each name in `new - current`; `remove(mangaId, current - new)` which is `DELETE FROM excluded_scanlators WHERE manga_id = :mangaId AND scanlator IN :scanlators`), `SyncChaptersWithSource.kt:228-231` (chapters from excluded groups are removed from the returned list of newly added chapters, so they cause no new-chapter notification and no auto-download), `MangaScreenModel.kt:192` (Flow), backup `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/MangaBackupCreator.kt:34` and restore `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt:426,432`.

Perf: tiny table; the join in `getChaptersByMangaId` and `libraryView` is index-assisted by `excluded_scanlators_manga_id_index`.

Proposal: Rust DDL with the uniqueness the Kotlin code assumes (it computes set differences):

```sql
CREATE TABLE excluded_scanlators(
    manga_id INTEGER NOT NULL,
    scanlator TEXT NOT NULL,
    PRIMARY KEY (manga_id, scanlator),
    FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE
) WITHOUT ROWID;
```

(The primary key replaces `excluded_scanlators_manga_id_index`.) `pub fn set_excluded_scanlators(conn: &mut rusqlite::Connection, manga_id: i64, wanted: &std::collections::BTreeSet<String>) -> rusqlite::Result<()>` runs in one transaction: `DELETE FROM excluded_scanlators WHERE manga_id = :id AND scanlator NOT IN (SELECT value FROM json_each(:wanted))`, then `INSERT OR IGNORE INTO excluded_scanlators(manga_id, scanlator) SELECT :id, value FROM json_each(:wanted)`. `pub fn get_excluded_scanlators(conn: &rusqlite::Connection, manga_id: i64) -> rusqlite::Result<std::collections::BTreeSet<String>>`.

## 4. Views of the anime database (`data/src/main/sqldelightanime/view/`)

All views are created at schema creation time, in the order SQLite resolves dependencies (a view may only be created after the views and tables it names exist): `episodestatsView`, `animehistorystatsView`, then `animeseasonstatsView`, then `animelibView`, `animeseasonsView`, `animehistoryView`, `animedeletableView`, `animeupdatesView`. Rust creates them in exactly that order inside the initial schema transaction. Mapper note: all statements that select `M.*` return the 32 `animes` columns by name followed by the computed columns, so `anime_from_row` (0.6) works on every one of them.

### 4.1 `episodestatsView`

OBSERVED. `data/src/main/sqldelightanime/view/episodestatsView.sq`. Created by `131.sqm`, `fillermarkCount` added by `134.sqm`.

`data/src/main/sqldelightanime/view/episodestatsView.sq:1-11`

```sql
CREATE VIEW episodestatsView AS
SELECT
    episodes.anime_id,
    count(*) AS total,
    sum(seen) AS seenCount,
    coalesce(max(episodes.date_upload), 0) AS latestUpload,
    coalesce(max(episodes.date_fetch), 0) AS fetchedAt,
    sum(episodes.bookmark) AS bookmarkCount,
    sum(episodes.fillermark) AS fillermarkCount
FROM episodes
GROUP BY episodes.anime_id;
```

Columns: `anime_id`; `total` (count of episodes); `seenCount` (sum of `seen`); `latestUpload` (max `date_upload`, `0` if all NULL); `fetchedAt` (max `date_fetch`); `bookmarkCount`; `fillermarkCount`. One row per anime that has at least one episode; an anime without episodes has no row, which every consumer handles with `LEFT JOIN` plus `coalesce(<column>, 0)`. Consumers: `animelibView`, `animeseasonsView`, `animeseasonstatsView`. Reads tables: `episodes`.

Perf: SQLite materialises the grouped view once per statement that uses it (a full scan of `episodes`, using `episodes_anime_id_index` as a covering-order scan). With 50,000 episodes that is roughly 10 to 30 ms per library query. Each of `animelibView`, `animeseasonstatsView` and `animeseasonsView` embeds it separately, so the library query materialises it twice (once directly, once inside `animeseasonstatsView`).

Proposal: create verbatim. No Rust function reads it directly.

### 4.2 `animehistoryView`

OBSERVED. `data/src/main/sqldelightanime/view/animehistoryView.sq:1-26`. Lineage: `114.sqm` (first), `115.sqm` (column renames), `116.sqm`, `118.sqm` (added `source`, `favorite`, `cover_last_modified`).

`data/src/main/sqldelightanime/view/animehistoryView.sq:1-26`

```sql
CREATE VIEW animehistoryView AS
SELECT
    animehistory._id AS id,
    animes._id AS animeId,
    episodes._id AS episodeId,
    animes.title,
    animes.thumbnail_url AS thumbnailUrl,
    animes.source,
    animes.favorite,
    animes.cover_last_modified,
    episodes.episode_number AS episodeNumber,
    animehistory.last_seen AS seenAt,
    max_last_seen.last_seen AS maxSeenAt,
    max_last_seen.episode_id AS maxSeenAtEpisodeId
FROM animes
JOIN episodes
ON animes._id = episodes.anime_id
JOIN animehistory
ON episodes._id = animehistory.episode_id
JOIN (
    SELECT episodes.anime_id,episodes._id AS episode_id, MAX(animehistory.last_seen) AS last_seen
    FROM episodes JOIN animehistory
    ON episodes._id = animehistory.episode_id
    GROUP BY episodes.anime_id
) AS max_last_seen
ON episodes.anime_id = max_last_seen.anime_id;
```

Columns (12): `id` (`animehistory._id`), `animeId`, `episodeId`, `title`, `thumbnailUrl`, `source`, `favorite`, `cover_last_modified`, `episodeNumber`, `seenAt` (this row's `last_seen`), `maxSeenAt` (newest `last_seen` of the same anime), `maxSeenAtEpisodeId` (the episode that owns `maxSeenAt`; relies on the bare-column rule, 0.3). Fan-out: one row per history row, each carrying the same `maxSeen*` pair for its anime; the consumer keeps the rows where `maxSeenAtEpisodeId = episodeId`. If two rows of an anime tie on the maximum, `max_last_seen.episode_id` is one of the tied episodes and only that one passes the consumer filter. Reads tables: `animes`, `episodes`, `animehistory`. Consumers: query `animehistory` (2.4) and `getLatestAnimeHistory` (`SELECT id, animeId, episodeId, title, thumbnailUrl, source, favorite, cover_last_modified, episodeNumber, seenAt FROM animehistoryView WHERE seenAt > 0 ORDER BY seenAt DESC LIMIT 1`, `AnimeHistoryRepositoryImpl.kt:24`; note this one does not filter on `maxSeenAtEpisodeId`, which is harmless because the newest row overall is also the newest of its anime).

Proposal: create verbatim.

### 4.3 `animehistorystatsView`

OBSERVED. `data/src/main/sqldelightanime/view/animehistorystatsView.sq`, `131.sqm`.

`data/src/main/sqldelightanime/view/animehistorystatsView.sq:1-7`

```sql
CREATE VIEW animehistorystatsView AS
SELECT
    episodes.anime_id,
    max(ah.last_seen) AS lastSeen
FROM episodes
LEFT JOIN animehistory AS ah ON episodes._id = ah.episode_id
GROUP BY episodes.anime_id;
```

Columns: `anime_id`, `lastSeen` (newest `last_seen` over all episodes of the anime; reset rows count as `0`; `NULL` when the anime has episodes but no history). One row per anime that has at least one episode (the `LEFT JOIN` keeps episodes without history). Consumers: `animelibView`, `animeseasonstatsView`, `animeseasonsView`, always through `coalesce(<column>, 0)`. Reads tables: `episodes`, `animehistory`.

Proposal: create verbatim.

### 4.4 `animeseasonstatsView`

OBSERVED. `data/src/main/sqldelightanime/view/animeseasonstatsView.sq`. `133.sqm` created it; `134.sqm` added `total_fillermarks`. Rolls the stats of all children of a "Seasons" parent into one row per parent.

`data/src/main/sqldelightanime/view/animeseasonstatsView.sq:1-17`

```sql
CREATE VIEW animeseasonstatsView AS
SELECT
    season.parent_id,
    count(*) AS child_count,
    sum(CASE WHEN season.fetch_type = 1 AND
                 coalesce(ES.total, 0) = coalesce(ES.seenCount, 0)
            THEN 1 ELSE 0 END) AS fully_seen_seasons,
    max(coalesce(ES.latestUpload, 0)) AS max_latest_upload,
    max(coalesce(ES.fetchedAt, 0)) AS max_fetched_at,
    max(coalesce(AHS.lastSeen, 0)) AS max_last_seen,
    sum(coalesce(ES.bookmarkCount, 0)) AS total_bookmarks,
    sum(coalesce(ES.fillermarkCount, 0)) AS total_fillermarks
FROM animes season
LEFT JOIN episodestatsView ES ON season._id = ES.anime_id
LEFT JOIN animehistorystatsView AHS ON season._id = AHS.anime_id
WHERE season.parent_id IS NOT NULL
GROUP BY season.parent_id;
```

Columns: `parent_id`; `child_count` (number of child rows of any `fetch_type`); `fully_seen_seasons` (children with `fetch_type = 1` whose `total = seenCount`, where a child with no episodes counts as 0 = 0, so an empty season is counted as fully seen); `max_latest_upload`; `max_fetched_at`; `max_last_seen`; `total_bookmarks`; `total_fillermarks` (sums). Only parents appear (`WHERE season.parent_id IS NOT NULL GROUP BY season.parent_id`). Nested seasons (a child that itself has `fetch_type = 0`) count in `child_count` but contribute nothing to `fully_seen_seasons`, and their own children are not aggregated into the grandparent. Reads tables: `animes`, `episodes`, `animehistory`. Consumers: `animelibView`, `animeseasonsView`.

Proposal: create verbatim.

### 4.5 `animeseasonsView`

OBSERVED. `data/src/main/sqldelightanime/view/animeseasonsView.sq:1-39` plus the query `getAnimeSeasonsById` (`:41-44`, not an item here). `131.sqm` created it, `133.sqm` and `134.sqm` rewrote it. All non-root animes (children of a Seasons parent) with the same computed counters as the library.

`data/src/main/sqldelightanime/view/animeseasonsView.sq:1-39`

```sql
CREATE VIEW animeseasonsView AS
SELECT
    M.*,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.total, 0)
        WHEN 0 THEN coalesce(ASS.child_count, 0)
    END AS totalCount,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.seenCount, 0)
        WHEN 0 THEN coalesce(ASS.fully_seen_seasons, 0)
    END AS seenCount,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.latestUpload, 0)
        WHEN 0 THEN coalesce(ASS.max_latest_upload, 0)
    END AS latestUpload,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.fetchedAt, 0)
        WHEN 0 THEN coalesce(ASS.max_fetched_at, 0)
    END AS episodeFetchedAt,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(AHS.lastSeen, 0)
        WHEN 0 THEN coalesce(ASS.max_last_seen, 0)
    END AS lastSeen,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.bookmarkCount, 0)
        WHEN 0 THEN coalesce(ASS.total_bookmarks, 0)
    END AS bookmarkCount,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.fillermarkCount, 0)
        WHEN 0 THEN coalesce(ASS.total_fillermarks, 0)
    END AS fillermarkCount
FROM animes AS M
LEFT JOIN episodestatsView AS ES
ON M._id = ES.anime_id
LEFT JOIN animehistorystatsView AS AHS
ON M._id = AHS.anime_id
LEFT JOIN animeseasonstatsView AS ASS
ON M._id = ASS.parent_id
WHERE M.parent_id IS NOT NULL;
```

Columns: the 32 `animes` columns, then `totalCount`, `seenCount`, `latestUpload`, `episodeFetchedAt`, `lastSeen`, `bookmarkCount`, `fillermarkCount`. Per row: when `fetch_type = 1` the numbers come from `episodestatsView`/`animehistorystatsView` for that row itself; when `fetch_type = 0` (a season that is itself a parent) from `animeseasonstatsView`; any other value gives `NULL` (no `ELSE`). Filter: `parent_id IS NOT NULL`; no `favorite` filter and no category join, so there is exactly one row per child. The statement used by the app:

```sql
SELECT * FROM animeseasonsView WHERE parent_id = :parentId;
```

Callers: `data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:160-168` (`getAnimeSeasonsById` one-shot at `:161`, Flow at `:166`), `domain/src/main/java/tachiyomi/domain/items/season/interactor/GetAnimeSeasonsByParentId.kt:13`, `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetAnimeWithEpisodesAndSeasons.kt:35`, `app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt:116` (sorted by `season_source_order` in Kotlin before it is sent to the source). Mapping: `AnimeMapper.mapSeasonAnime` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeMapper.kt:166-248`) reads `seenCount` and `bookmarkCount` as `Double` (SQLDelight infers REAL because of `CASE` over mixed branches) and converts with `.toLong()`; SQLite actually returns integers, so Rust reads them as `i64`. `SeasonAnime.seen` is `totalCount == seenCount`. Reads tables: `animes`, `episodes`, `animehistory`.

Proposal: create verbatim, but add `ELSE 0` to every `CASE` so an unexpected `fetch_type` cannot produce `NULL` into a non-null field. Rust: `pub fn get_anime_seasons_by_id(conn: &rusqlite::Connection, parent_id: i64) -> rusqlite::Result<Vec<SeasonAnimeRow>>` with `SeasonAnimeRow { anime: AnimeRow, total_count: i64, seen_count: i64, latest_upload: i64, fetched_at: i64, last_seen: i64, bookmark_count: i64, fillermark_count: i64 }`, columns read by name (`episodeFetchedAt` maps to `fetched_at`). Add `ORDER BY season_source_order, _id` for deterministic output (Aniyomi sorts in Kotlin).

### 4.6 `animelibView`

OBSERVED. `data/src/main/sqldelightanime/view/animelibView.sq:1-38`. Lineage: `122.sqm` (first), `131.sqm`, `133.sqm`, `134.sqm`. The library: every favorite anime joined to its stats and category.

`data/src/main/sqldelightanime/view/animelibView.sq:1-38`

```sql
CREATE VIEW animelibView AS
SELECT
    M.*,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.total, 0)
        WHEN 0 THEN coalesce(ASS.child_count, 0)
    END AS totalCount,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.seenCount, 0)
        WHEN 0 THEN coalesce(ASS.fully_seen_seasons, 0)
    END AS seenCount,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.latestUpload, 0)
        WHEN 0 THEN coalesce(ASS.max_latest_upload, 0)
    END AS latestUpload,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.fetchedAt, 0)
        WHEN 0 THEN coalesce(ASS.max_fetched_at, 0)
    END AS episodeFetchedAt,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(AHS.lastSeen, 0)
        WHEN 0 THEN coalesce(ASS.max_last_seen, 0)
    END AS lastSeen,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.bookmarkCount, 0)
        WHEN 0 THEN coalesce(ASS.total_bookmarks, 0)
    END AS bookmarkCount,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.fillermarkCount, 0)
        WHEN 0 THEN coalesce(ASS.total_fillermarks, 0)
    END AS fillermarkCount,
    coalesce(MC.category_id, 0) AS category
FROM animes M
LEFT JOIN episodestatsView AS ES ON M._id = ES.anime_id
LEFT JOIN animehistorystatsView AS AHS ON M._id = AHS.anime_id
LEFT JOIN animes_categories AS MC ON MC.anime_id = M._id
LEFT JOIN animeseasonstatsView AS ASS ON M._id = ASS.parent_id
WHERE M.favorite = 1;
```

Columns: the 32 `animes` columns, then `totalCount`, `seenCount`, `latestUpload`, `episodeFetchedAt`, `lastSeen`, `bookmarkCount`, `fillermarkCount`, `category`.

Meaning of the counters, by `fetch_type`:

| Counter | `fetch_type = 1` (episodes) | `fetch_type = 0` (seasons parent) |
|---|---|---|
| `totalCount` | number of episodes | number of child rows |
| `seenCount` | number of seen episodes | number of fully seen child seasons |
| `latestUpload` | newest `date_upload` | newest across children |
| `episodeFetchedAt` | newest `date_fetch` | newest across children |
| `lastSeen` | newest history `last_seen` | newest across children |
| `bookmarkCount` / `fillermarkCount` | sums over the anime | sums over children |

All counters are `0` when there is no data (`coalesce`); any other `fetch_type` yields `NULL` (no `ELSE`). `category` is `coalesce(MC.category_id, 0)`.

Row multiplicity (important): `LEFT JOIN animes_categories` yields one row per (anime, category) pair, and one row with `category = 0` for an anime with no category. The row count is the sum over library anime of `max(1, number of category links)`; consumers group by `category` (`app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt:404`) and the update job de-duplicates by id. The view has NO `parent_id` filter: a favorited season child would also appear (the UI never favorites children).

Domain mapping: `LibraryAnime(anime, category, totalCount, seenCount, bookmarkCount, fillermarkCount, latestUpload, episodeFetchedAt, lastSeen)`; `unseenCount = totalCount - seenCount`, `hasStarted = seenCount > 0` (`domain/src/main/java/tachiyomi/domain/library/anime/LibraryAnime.kt`). Reads tables: `animes`, `episodes`, `animehistory`, `animes_categories`.

Perf: the view recomputes aggregates over all episodes of all anime, twice (4.1). For 2,000 library entries and 50,000 episodes that is about 30 to 60 ms in Aniyomi's SQLite on a phone, a few milliseconds on a desktop. It re-runs on any write to the four tables above, including the once-per-second progress write during playback (2.3).

Proposal: create verbatim with `ELSE 0` added to every `CASE`; throttle UI refresh by the 250 ms debounce of 0.5; if profiling shows the library load above 50 ms on the target library size, materialise `episodestatsView` into a table maintained by triggers (not needed for P0).

### 4.7 `animelib` (query)

OBSERVED. `data/src/main/sqldelightanime/view/animelibView.sq:40-42`.

`data/src/main/sqldelightanime/view/animelibView.sq:40-42`

```sql
animelib:
SELECT *
FROM animelibView;
```

No parameters, no `ORDER BY`, no `LIMIT`. Sorting, filtering (unseen, started, downloaded, bookmarked, tracked, source, language), grouping by category and search all happen in Kotlin over the returned list (`AnimeLibraryScreenModel.kt:381-411`); area:37-ui-library specifies those rules. Callers: `data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:60-66` (`getLibraryAnime` at `:61`, Flow at `:65`) wrapped by `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetLibraryAnime.kt:17-33` (the Flow variant retries after 0.5 s on a `NullPointerException` and logs other errors), consumed by `AnimeLibraryScreenModel.kt:381`, `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt:153` (filters by `category` in include/exclude lists, area:26-library-update-jobs) and `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeMetadataUpdateJob.kt:92`.

Proposal: Rust:

```rust
pub struct LibraryAnimeRow {
    pub anime: AnimeRow,
    pub total_count: i64, pub seen_count: i64, pub latest_upload: i64,
    pub episode_fetched_at: i64, pub last_seen: i64,
    pub bookmark_count: i64, pub fillermark_count: i64, pub category: i64,
}
pub fn animelib(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<LibraryAnimeRow>> {
    let mut st = conn.prepare_cached("SELECT * FROM animelibView")?;
    st.query_map([], |r| Ok(LibraryAnimeRow {
        anime: anime_from_row(r)?,
        total_count: r.get("totalCount")?, seen_count: r.get("seenCount")?,
        latest_upload: r.get("latestUpload")?, episode_fetched_at: r.get("episodeFetchedAt")?,
        last_seen: r.get("lastSeen")?, bookmark_count: r.get("bookmarkCount")?,
        fillermark_count: r.get("fillermarkCount")?, category: r.get("category")?,
    }))?.collect()
}
```

Reads tables for the change notifier: `animes`, `episodes`, `animehistory`, `animes_categories`.

### 4.8 `animedeletableView`

OBSERVED. `data/src/main/sqldelightanime/view/animedeletableView.sq`, `131.sqm`. The candidates for Settings, Advanced, "Clear anime database": non-library, top-level anime.

`data/src/main/sqldelightanime/view/animedeletableView.sq:1-12`

```sql
CREATE VIEW animedeletableView AS
SELECT
    M._id,
    M.source,
    M.fetch_type
FROM animes AS M
WHERE M.favorite = 0
AND M.parent_id IS NULL;

getDeletableParentAnime:
SELECT *
FROM animedeletableView;
```

The view selects `_id`, `source`, `fetch_type` of rows with `favorite = 0 AND parent_id IS NULL`. The named query in the same file is `getDeletableParentAnime` (`SELECT * FROM animedeletableView;`, `data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:178-182`, Flow) mapped to `DeletableAnime(animeId, sourceId, fetchType)` by `AnimeMapper.mapDeletableAnime` (`:319-327`). Consumer: `GetAnimeSourcesWithNonLibraryAnime.subscribe` (`domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetAnimeSourcesWithNonLibraryAnime.kt:13`) feeding `app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearAnimeDatabaseScreen.kt:227`. The screen groups rows by `source`, and for a parent with `fetch_type = Seasons` it walks `getChildrenByParentId` recursively (`:242,267-283`): non-favorite children are added to the delete set, favorite children are collected as "orphaned" and later get `parent_id = NULL` through `removeParentIdByIds`, so that they survive the parent's deletion as standalone library entries. Reads tables: `animes`.

Proposal: create verbatim. Rust: `pub fn get_deletable_parent_anime(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<DeletableAnime>>` with `SELECT _id, source, fetch_type FROM animedeletableView` and `DeletableAnime { anime_id: i64, source_id: i64, fetch_type: FetchType }`.

### 4.9 `animeupdatesView`

OBSERVED. `data/src/main/sqldelightanime/view/animeupdatesView.sq:1-23`. Lineage: `119.sqm`, `123.sqm` (added `last_second_seen`, `totalSeconds`), `134.sqm` (added `fillermark`). The "Updates" tab: episodes that appeared after the anime joined the library.

`data/src/main/sqldelightanime/view/animeupdatesView.sq:1-23`

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
```

Columns (16): `animeId`, `animeTitle`, `episodeId`, `episodeName`, `scanlator`, `seen`, `bookmark`, `fillermark`, `last_second_seen`, `totalSeconds`, `source`, `favorite`, `thumbnailUrl`, `coverLastModified`, `dateUpload`, `datefetch`. Rules: only library anime (`favorite = 1`) and only episodes fetched after the anime was added (`date_fetch > date_added`; `date_added` is `0` for non-library anime and is set to "now" when favorited, `UpdateAnime.kt:49-56`, so episodes that existed at favorite time are excluded and every later-discovered episode is included; a re-favorited anime gets a new `date_added` and loses older updates). The `ORDER BY date_fetch DESC` is inside the view (see 0.3).

Queries on the view (other chunks; `data/src/main/sqldelightanime/view/animeupdatesView.sq:25-37`):

```sql
getRecentAnimeUpdates: SELECT * FROM animeupdatesView WHERE dateUpload > :after LIMIT :limit;
getUpdatesBySeenStatus: SELECT * FROM animeupdatesView WHERE seen = :seen AND dateUpload > :after LIMIT :limit;
```

The date filter is on `dateUpload` (source upload date, which falls back to fetch time, see 2.3), not on `datefetch`. Parameters in Kotlin: `after` = now minus 3 months in ms (`app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt:78-82`), `limit` = 500 (`domain/src/main/java/tachiyomi/domain/updates/anime/interactor/GetAnimeUpdates.kt:13,17,21`). Callers: `data/src/main/java/tachiyomi/data/updates/anime/AnimeUpdatesRepositoryImpl.kt:13-43`; mapper `mapUpdatesWithRelations` (`:45-69`) builds `AnimeUpdatesWithRelations` with an `AnimeCover`. The screen groups by local date of `dateFetch` (`AnimeUpdatesScreenModel.kt:408-415`). Reads tables: `animes`, `episodes`.

Perf: no index on `episodes.date_fetch`; the join sorts every update-eligible episode of every library anime before applying `LIMIT 500`. Fine up to a few hundred thousand episodes.

Proposal: create the view verbatim, and in the two queries add an explicit `ORDER BY datefetch DESC, episodeId DESC` so the order no longer depends on SQLite's view flattening:

```rust
pub struct AnimeUpdatesRow {
    pub anime_id: i64, pub anime_title: String, pub episode_id: i64, pub episode_name: String,
    pub scanlator: Option<String>, pub seen: bool, pub bookmark: bool, pub fillermark: bool,
    pub last_second_seen: i64, pub total_seconds: i64, pub source: i64, pub favorite: bool,
    pub thumbnail_url: Option<String>, pub cover_last_modified: i64, pub date_upload: i64, pub date_fetch: i64,
}
pub fn get_recent_anime_updates(conn: &rusqlite::Connection, after_ms: i64, limit: i64) -> rusqlite::Result<Vec<AnimeUpdatesRow>>; // WHERE dateUpload > :after ORDER BY datefetch DESC, episodeId DESC LIMIT :limit
pub fn get_updates_by_seen_status(conn: &rusqlite::Connection, seen: bool, after_ms: i64, limit: i64) -> rusqlite::Result<Vec<AnimeUpdatesRow>>;
```

## 5. Generic-name statements: `count`, `delete`, and the four `delete*` queries

The inventory tool lists statements by bare name, so `count` and `delete` each stand for every statement of that name in the chunk's files.

### 5.1 `count`

OBSERVED. One statement, in `data/src/main/sqldelight/data/extension_repos.sq:23-25`. The table it counts belongs to the manga database (migration `31.sqm`); the anime database had the same table from `128.sqm` until `137.sqm` replaced it by `extension_store` (whose own `getCount` is `SELECT COUNT(*) FROM extension_store;`, `data/src/main/sqldelightanime/dataanime/extension_store.sq`, used by `mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt:98-100`).

`data/src/main/sqldelight/data/extension_repos.sq:1-7`

```sql
CREATE TABLE extension_repos (
    base_url TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    short_name TEXT,
    website TEXT NOT NULL,
    signing_key_fingerprint TEXT UNIQUE NOT NULL
);
```

| Column | Type, constraint | Meaning |
|---|---|---|
| `base_url` | TEXT NOT NULL PRIMARY KEY | repository base url without the trailing `/index.min.json` |
| `name` | TEXT NOT NULL | repository display name |
| `short_name` | TEXT | optional short label |
| `website` | TEXT NOT NULL | |
| `signing_key_fingerprint` | TEXT NOT NULL UNIQUE | SHA-256 fingerprint of the repository's signing key; legacy rows migrated from preferences use `NOFINGERPRINT-<n>` (`app/src/main/java/mihon/core/migration/migrations/TrustExtensionRepositoryMigration.kt:36`) |

`data/src/main/sqldelight/data/extension_repos.sq:23-25`

```sql
count:
SELECT COUNT(*)
FROM extension_repos;
```

* Returns one row with one integer. Used only to show the number of configured repositories beside the "Extension repos" entries in Settings, Browse (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsBrowseScreen.kt:43-44`, manga count and anime store count shown separately).
* Callers: `mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt:33` (`subscribeToOne { count() }.map { it.toInt() }`, a Flow that re-emits on every repository insert/delete), `domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/GetMangaExtensionRepoCount.kt:8`.
* Perf: `COUNT(*)` over a table of a handful of rows.
* Reads tables: `extension_repos`.

Proposal: whether Kurobayomi keeps Aniyomi's APK extension repositories is decided by area:20-extension-strategy-redesign; the repository list itself is a small table and is kept so Aniyomi backups restore and so the Settings screen can show a count. One table `extension_repos` in `manga.db` (as above) and the anime-side `extension_store` in `anime.db` (5.2, item 7 of `delete`):

```rust
pub fn count_extension_repos(conn: &rusqlite::Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM extension_repos", [], |r| r.get(0))
}
```

### 5.2 `delete`

OBSERVED. Seven statements named `delete`, all single-table `DELETE` statements whose `WHERE` names the primary or natural key, none with a `RETURNING` clause. A delete of a missing key succeeds and affects 0 rows; the Kotlin callers ignore the affected-row count.

**(1) Manga category** (`data/src/main/sqldelight/data/categories.sq:75-77`)

`data/src/main/sqldelight/data/categories.sq:75-77`

```sql
delete:
DELETE FROM categories
WHERE _id = :categoryId;
```

Trigger `system_category_delete_trigger` (2.8) aborts with `System category can't be deleted` when `:categoryId <= 0`. Cascades to `mangas_categories` (FK on `category_id`), which silently moves the affected manga to category 0. Caller: `data/src/main/java/tachiyomi/data/category/manga/MangaCategoryRepositoryImpl.kt:100`, wrapped by `domain/src/main/java/tachiyomi/domain/category/manga/interactor/DeleteMangaCategory.kt:19`, which afterwards renumbers `sort` and strips the id from the preference id-sets (same algorithm as anime, 2.8).

**(2) Anime category** (`data/src/main/sqldelightanime/dataanime/categories.sq:75-77`): identical text.

`data/src/main/sqldelightanime/dataanime/categories.sq:75-77`

```sql
delete:
DELETE FROM categories
WHERE _id = :categoryId;
```

Cascades to `animes_categories`. Caller: `data/src/main/java/tachiyomi/data/category/anime/AnimeCategoryRepositoryImpl.kt:100`, wrapped by `domain/src/main/java/tachiyomi/domain/category/anime/interactor/DeleteAnimeCategory.kt:19`.

**(3) Extension repo (manga)** (`data/src/main/sqldelight/data/extension_repos.sq:55-57`)

`data/src/main/sqldelight/data/extension_repos.sq:55-57`

```sql
delete:
DELETE FROM extension_repos
WHERE base_url = :base_url;
```

Key: `base_url`. Caller: `mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt:77`, `domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/DeleteMangaExtensionRepo.kt:9`, UI `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/MangaExtensionReposScreenModel.kt:93` (confirmed by a dialog, `MangaExtensionReposScreen.kt:67`). The interactor only deletes the row.

**(4) Manga track** (`data/src/main/sqldelight/data/manga_sync.sq:23-25`)

`data/src/main/sqldelight/data/manga_sync.sq:23-25`

```sql
delete:
DELETE FROM manga_sync
WHERE manga_id = :mangaId AND sync_id = :syncId;
```

Keys: `manga_id` plus `sync_id` (tracker id); there is at most one such row because of `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`. Caller: `data/src/main/java/tachiyomi/data/track/manga/MangaTrackRepositoryImpl.kt:36`, `domain/src/main/java/tachiyomi/domain/track/manga/interactor/DeleteMangaTrack.kt:13` (exceptions logged, not rethrown), UI `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt:871` ("remove tracker" action; the remote list entry is optionally deleted by the tracker client first, area:27-tracking-core). `manga_sync` has the same columns as `anime_sync` (2.5) with `manga_id`, `last_chapter_read`, `total_chapters` instead of the anime names (`data/src/main/sqldelight/data/manga_sync.sq:3-21`; `private` from `32.sqm`).

**(5) Anime track** (`data/src/main/sqldelightanime/dataanime/anime_sync.sq:23-25`)

`data/src/main/sqldelightanime/dataanime/anime_sync.sq:23-25`

```sql
delete:
DELETE FROM anime_sync
WHERE anime_id = :animeId AND sync_id = :syncId;
```

Caller: `data/src/main/java/tachiyomi/data/track/anime/AnimeTrackRepositoryImpl.kt:36`, `domain/src/main/java/tachiyomi/domain/track/anime/interactor/DeleteAnimeTrack.kt:13`, UI `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt:871`.

**(6) Custom button** (`data/src/main/sqldelightanime/dataanime/custom_buttons.sq:41-43`)

`data/src/main/sqldelightanime/dataanime/custom_buttons.sq:41-43`

```sql
delete:
DELETE FROM custom_buttons
WHERE _id = :customButtonId;
```

Key: `_id` (parameter `:customButtonId`). Caller: `data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt:52`, `domain/src/main/java/tachiyomi/domain/custombuttons/interactor/DeleteCustomButton.kt:14` (delete, then renumber `sortIndex` of the survivors with consecutive `0..n-1`, then returns Success or InternalError), UI `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/PlayerSettingsCustomButtonScreenModel.kt:84-86`. The seed button can be deleted like any other; it is not re-created (the seed `INSERT OR IGNORE` runs only at table creation).

**(7) Extension store (anime)** (`data/src/main/sqldelightanime/dataanime/extension_store.sq:42-44`)

`data/src/main/sqldelightanime/dataanime/extension_store.sq:3-12`

```sql
CREATE TABLE extension_store(
    index_url TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    badge_label TEXT NOT NULL,
    signing_key TEXT NOT NULL,
    contact_website TEXT NOT NULL,
    contact_discord TEXT,
    is_legacy INTEGER AS Boolean NOT NULL,
    extension_list_url TEXT
);
```

`data/src/main/sqldelightanime/dataanime/extension_store.sq:42-44`

```sql
delete:
DELETE FROM extension_store
WHERE index_url = :indexUrl;
```

`extension_store` (migration `137.sqm`) holds the anime extension repositories (an "index url" per store plus signing key, contact links, `is_legacy` flag for old-style repositories and an optional `extension_list_url`). `137.sqm` created it by copying every `extension_repos` row as `(base_url || '/repo.json', name, coalesce(short_name, name), signing_key_fingerprint, website, NULL, 1, NULL)` and dropped `extension_repos` from the anime database. Caller: `mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt:102-104` (`remove(indexUrl)`), UI `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt:110`.

Perf for all seven: key lookups on a primary key or the `UNIQUE` auto-index; sub-millisecond.

Proposal: Rust functions (named parameters, SQL identical to the above; `DELETE` on a system category surfaces the trigger's error as `rusqlite::Error::SqliteFailure` with message `System category can't be deleted`):

```rust
pub fn delete_category(conn: &rusqlite::Connection, category_id: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM categories WHERE _id = :id", rusqlite::named_params! { ":id": category_id })
}
pub fn delete_extension_repo(conn: &rusqlite::Connection, base_url: &str) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM extension_repos WHERE base_url = :base_url", rusqlite::named_params! { ":base_url": base_url })
}
pub fn delete_manga_track(conn: &rusqlite::Connection, manga_id: i64, sync_id: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM manga_sync WHERE manga_id = :manga_id AND sync_id = :sync_id",
        rusqlite::named_params! { ":manga_id": manga_id, ":sync_id": sync_id })
}
pub fn delete_anime_track(conn: &rusqlite::Connection, anime_id: i64, sync_id: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM anime_sync WHERE anime_id = :anime_id AND sync_id = :sync_id",
        rusqlite::named_params! { ":anime_id": anime_id, ":sync_id": sync_id })
}
pub fn delete_custom_button(conn: &rusqlite::Connection, id: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM custom_buttons WHERE _id = :id", rusqlite::named_params! { ":id": id })
}
pub fn delete_extension_store(conn: &rusqlite::Connection, index_url: &str) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM extension_store WHERE index_url = :index_url", rusqlite::named_params! { ":index_url": index_url })
}
```

The category and custom-button deletes are followed by the renumbering described in 2.8 and 2.9; the Rust interactor performs delete and renumber in ONE transaction (Aniyomi uses two separate statements and can be interrupted between them, leaving a gap in `sort`/`sortIndex`, which is harmless).

### 5.3 `deleteAnimeCategoryByAnimeId`

OBSERVED. `data/src/main/sqldelightanime/dataanime/animes_categories.sq:22-24`.

`data/src/main/sqldelightanime/dataanime/animes_categories.sq:22-24`

```sql
deleteAnimeCategoryByAnimeId:
DELETE FROM animes_categories
WHERE anime_id = :animeId;
```

* Parameter `:animeId`. Removes every category link of one anime (including links to hidden categories). No trigger fires on delete (the version trigger exists only for `INSERT`), so `animes.version` is not bumped by the removal itself.
* Callers: `data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:97` inside `setAnimeCategories` (`:95-102`, one transaction: delete then `insert` per id; the replace-all semantic is why unchecking every category in the picker removes the anime from all categories), restore `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt:354`.
* Perf: scans `animes_categories` by `anime_id` (no index in Aniyomi, so a full scan of the link table, which has at most a few thousand rows; the Rust DDL has the unique index with `anime_id` as its leading column).

Proposal: `set_anime_categories` (2.2) calls

```rust
pub fn delete_anime_category_by_anime_id(conn: &rusqlite::Connection, anime_id: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM animes_categories WHERE anime_id = :anime_id", rusqlite::named_params! { ":anime_id": anime_id })
}
```

### 5.4 `deleteAnimesNotInLibraryByAnimeIds`

OBSERVED. `data/src/main/sqldelightanime/dataanime/animes.sq:162-164`.

`data/src/main/sqldelightanime/dataanime/animes.sq:162-164`

```sql
deleteAnimesNotInLibraryByAnimeIds:
DELETE FROM animes
WHERE favorite = 0 AND _id IN :animeIds;
```

* Parameter `:animeIds` (list of `_id`). Deletes only rows that are not in the library: `favorite = 0` is a guard, so a library entry whose id is in the list survives. An empty list deletes nothing.
* Cascades (`foreign_keys = ON`): `episodes` (then `animehistory` through `episodes`), `animes_categories`, `anime_sync`, `anime_relations` (rows where the anime is the owner or the related one). `animes.parent_id` is not a foreign key, so children of a deleted parent keep a dangling `parent_id` unless handled by the caller. No delete trigger exists on `animes`.
* Caller: `app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearAnimeDatabaseScreen.kt:294`, in this order inside `removeAnimeBySourceId` (`:286-297`): (a) `deleteAnimesNotInLibraryByAnimeIds(animeIds)` where `animeIds` is the union over the selected sources of the non-library top-level anime from `animedeletableView` (4.8) plus all their non-favorite descendants, (b) `removeParentIdByIds(orphaned)` (`UPDATE animes SET parent_id = NULL WHERE _id IN :animeIds`) for the favorite descendants that must survive as top-level entries, (c) `animehistoryQueries.removeResettedHistory()` (`DELETE FROM animehistory WHERE last_seen = 0`). SQL does not touch cover or download files on disk.
* Perf: one statement over a primary-key list; the cascades are index-assisted (`episodes_anime_id_index`, the `UNIQUE (episode_id)` index of `animehistory`, `animes_categories` link scan, `anime_sync` unique index, `anime_relations` PK and `related_anime_id` index). Deleting 5,000 anime with 100,000 episodes takes a few seconds; run it in a transaction on a background thread and show progress.

Proposal:

```rust
pub fn delete_animes_not_in_library_by_anime_ids(conn: &rusqlite::Connection, anime_ids: &[i64]) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM animes WHERE favorite = 0 AND _id IN (SELECT value FROM json_each(:anime_ids))",
        rusqlite::named_params! { ":anime_ids": ids_json(anime_ids) },
    )
}
/// The three steps of "Clear anime database" for the selected sources, atomically.
pub fn clear_anime_database(conn: &mut rusqlite::Connection, anime_ids: &[i64], orphaned: &[i64]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    delete_animes_not_in_library_by_anime_ids(&tx, anime_ids)?;
    tx.execute("UPDATE animes SET parent_id = NULL WHERE _id IN (SELECT value FROM json_each(:ids))",
        rusqlite::named_params! { ":ids": ids_json(orphaned) })?;
    tx.execute("DELETE FROM animehistory WHERE last_seen = 0", [])?;
    tx.commit()
}
```

### 5.5 `deleteByAnimeId`

OBSERVED. `data/src/main/sqldelightanime/dataanime/anime_relations.sq:35-37`.

`data/src/main/sqldelightanime/dataanime/anime_relations.sq:35-37`

```sql
deleteByAnimeId:
DELETE FROM anime_relations
WHERE anime_id = :animeId;
```

* Parameter `:animeId`. Removes every relation row OWNED by the anime (rows where it is only the related side are untouched; they go away when that related anime is deleted).
* Caller: `data/src/main/java/tachiyomi/data/entries/anime/AnimeRelationRepositoryImpl.kt:23`, first step of `replaceRelations` (`:17-37`, one transaction: delete, then insert each member with a running `sort_order` starting at 0 and the same `last_fetched_at`). So a refresh that returns no groups leaves the anime with no rows, and `getLastFetchedAt` returns `NULL`, which makes the next details-screen open fetch again (no negative caching).
* Perf: uses the primary-key prefix.

Proposal:

```rust
pub fn delete_by_anime_id(conn: &rusqlite::Connection, anime_id: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM anime_relations WHERE anime_id = :anime_id", rusqlite::named_params! { ":anime_id": anime_id })
}
pub fn replace_relations(conn: &mut rusqlite::Connection, anime_id: i64, groups: &[(String, Vec<i64>)], fetched_at_ms: i64) -> rusqlite::Result<()>; // delete_by_anime_id + INSERT OR REPLACE per member, one transaction
```

### 5.6 `deleteMangaCategoryByMangaId`

OBSERVED. `data/src/main/sqldelight/data/mangas_categories.sq:22-24`. Manga twin of 5.3. For context, the table and its trigger (the table is catalogued in chunk 2):

`data/src/main/sqldelight/data/mangas_categories.sq:1-16`

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

CREATE TRIGGER insert_manga_category_update_version AFTER INSERT ON mangas_categories
BEGIN
    UPDATE mangas
    SET version = version + 1
    WHERE _id = new.manga_id AND (SELECT is_syncing FROM mangas WHERE _id = new.manga_id) = 0;
END;
```

`data/src/main/sqldelight/data/mangas_categories.sq:22-24`

```sql
deleteMangaCategoryByMangaId:
DELETE FROM mangas_categories
WHERE manga_id = :mangaId;
```

* Parameter `:mangaId`. Same semantics as 5.3.
* Callers: `data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt:93` inside `setMangaCategories` (`:91-98`, one transaction), wrapped by `domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetMangaCategories.kt:13`; restore `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt:313`.
* Migration note: `mangas_categories.last_modified_at` and its trigger exist only in upgraded databases (`26.sqm`), like the anime side (0.4).

Proposal: same DDL pattern as 2.2 with `manga_id`, `mangas`, trigger name `insert_manga_category_update_version`, and

```rust
pub fn delete_manga_category_by_manga_id(conn: &rusqlite::Connection, manga_id: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM mangas_categories WHERE manga_id = :manga_id", rusqlite::named_params! { ":manga_id": manga_id })
}
```

### 5.7 `deleteMangasNotInLibraryBySourceIds`

OBSERVED. `data/src/main/sqldelight/data/mangas.sq:147-150`.

`data/src/main/sqldelight/data/mangas.sq:147-150`

```sql
deleteMangasNotInLibraryBySourceIds:
DELETE FROM mangas
WHERE favorite = 0
AND source IN :sourceIds;
```

* Parameter `:sourceIds` (list of source ids). Deletes every manga of those sources that is not in the library (`favorite = 0`); there is no per-manga selection (the anime variant works by anime id because of seasons, 5.4). An empty list deletes nothing.
* Cascades: `chapters` (then `history` through `chapters`), `mangas_categories`, `manga_sync`, `excluded_scanlators`.
* Caller: `app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearDatabaseScreen.kt:212`, followed by `historyQueries.removeResettedHistory()` (`:213`, `DELETE FROM history WHERE last_read = 0`). The list of sources and per-source counts comes from `getSourceIdsWithNonLibraryManga` (`SELECT source, COUNT(*) AS manga_count FROM mangas WHERE favorite = 0 GROUP BY source`, `data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt:59`, other chunk).
* Perf: scans `mangas` for `favorite = 0` rows of the listed sources (the partial index `library_favorite_index` does not help a `favorite = 0` predicate); cascades are index-assisted.

Proposal:

```rust
pub fn delete_mangas_not_in_library_by_source_ids(conn: &rusqlite::Connection, source_ids: &[i64]) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM mangas WHERE favorite = 0 AND source IN (SELECT value FROM json_each(:source_ids))",
        rusqlite::named_params! { ":source_ids": ids_json(source_ids) },
    )
}
/// "Clear database" for the selected sources, atomically, then drop reset history rows.
pub fn clear_manga_database(conn: &mut rusqlite::Connection, source_ids: &[i64]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    delete_mangas_not_in_library_by_source_ids(&tx, source_ids)?;
    tx.execute("DELETE FROM history WHERE last_read = 0", [])?;
    tx.commit()
}
```

## 6. Dispositions and schema bootstrap

### 6.1 What happens to each item (planned, replaced or dropped)

| Item | Disposition |
|---|---|
| `anime_relations` | ported, DDL unchanged (feature itself P2) |
| `anime_relations_anime_id_index` | dropped: redundant with the primary key prefix; user loses nothing |
| `anime_relations_related_anime_id_index` | ported |
| `anime_sync` | ported; an upsert with `ON CONFLICT(anime_id, sync_id) DO UPDATE` replaces `ON CONFLICT REPLACE` so `_id` stays stable |
| `animedeletableView` | ported verbatim |
| `animehistory` (table and query) | ported; table column becomes `NOT NULL DEFAULT 0`; query gains escaped LIKE and an `id DESC` tiebreak |
| `animehistoryView` | ported verbatim |
| `animehistory_history_episode_id_index` | dropped: duplicates the `UNIQUE (episode_id)` index |
| `animehistorystatsView` | ported verbatim |
| `animelib` | ported (`SELECT * FROM animelibView`) |
| `animelibView` | ported, `ELSE 0` added to each `CASE` |
| `animelib_favorite_index` | ported |
| `animes` | ported; column order, names and types unchanged; defaults added for the four columns that had none; `memo` default is a BLOB |
| `animes_categories` | ported plus `UNIQUE (anime_id, category_id)` and `category_id` index |
| `animes_fetch_type` | dropped: no query can use it; user loses nothing |
| `animes_parent_id` | ported |
| `animes_url_index` | ported |
| `animeseasonsView` | ported, `ELSE 0` added |
| `animeseasonstatsView` | ported verbatim |
| `animesources` | ported |
| `animeupdatesView` | ported verbatim; the two queries on it add an explicit `ORDER BY` |
| `categories` | ported in both databases; delete renumbering no longer touches the system row |
| `chapters` | ported |
| `chapters_manga_id_index` | ported |
| `chapters_unread_by_manga_index` | dropped: never used by a query plan |
| `count` | ported (`count_extension_repos`); depends on the extension strategy of area:20-extension-strategy-redesign |
| `custom_buttons` | ported; scripting support depends on area:45-player-core and area:48-settings-player-and-scripts |
| `delete` | all seven ported |
| `deleteAnimeCategoryByAnimeId` | ported |
| `deleteAnimesNotInLibraryByAnimeIds` | ported with the transactional `clear_anime_database` wrapper |
| `deleteByAnimeId` | ported |
| `deleteMangaCategoryByMangaId` | ported |
| `deleteMangasNotInLibraryBySourceIds` | ported with `clear_manga_database` |
| `episodes` | ported |
| `episodes_anime_id_index` | ported |
| `episodes_unseen_by_anime_index` | dropped: never used by a query plan |
| `episodestatsView` | ported verbatim |
| `excluded_scanlators` | ported with a primary key `(manga_id, scanlator)` instead of a plain index |

The SyncYomi remnants in these tables (`is_syncing`, `version`, `last_modified_at`, `favorite_modified_at` and their triggers) are kept so the Aniyomi backup fields round-trip; the feature that consumed them is not part of Kurobayomi (area:21-backup-format-and-create decides whether the fields are exported as-is).

### 6.2 Creation order for `anime.db` (one transaction, then `PRAGMA user_version = 1`)

1. `categories` (+ seed row, + `system_category_delete_trigger`)
2. `animes` (+ 3 indexes, + 3 triggers)
3. `animes_categories` (+ index, + trigger)
4. `episodes` (+ index, + 2 triggers)
5. `animehistory`
6. `anime_sync`
7. `anime_relations` (+ index)
8. `animesources`
9. `custom_buttons` (+ seed row)
10. `extension_store`
11. views in the order of section 4 introduction

`manga.db`: `categories`, `mangas`, `mangas_categories`, `chapters`, `history`, `manga_sync`, `excluded_scanlators`, `sources`, `extension_repos`, then the three manga views (`libraryView`, `historyView`, `updatesView`; other chunks).

### 6.3 Checks that prove the port (for the task list)

* Open an in-memory database, run the creation script, `PRAGMA foreign_key_check` and `PRAGMA integrity_check` return no rows / `ok`, and `SELECT count(*) FROM categories` is `1` with `_id = 0`.
* Deleting category `0` fails with `System category can't be deleted`; deleting a user category removes its `animes_categories` rows.
* Insert an anime with 3 episodes of which 1 seen and 1 bookmarked, 1 history row at `last_seen = 5000`: `animelib` returns one row with `totalCount = 3`, `seenCount = 1`, `bookmarkCount = 1`, `lastSeen = 5000`, `category = 0`; link it to two categories: two rows.
* A Seasons parent with two child anime (one with all episodes seen, one with none): `animelib` row for the parent has `totalCount = 2`, `seenCount = 1`; `get_anime_seasons_by_id(parent)` returns two rows.
* `animehistory("")` returns one row per anime, the newest; resetting that history row (`last_seen = 0`) makes the next-newest episode of the same anime appear; resetting all hides the anime.
* `UPDATE episodes SET last_second_seen = 1000 WHERE _id = 1` increments `episodes.version` and `animes.version` once each, and sets `last_modified_at` on both; with `is_syncing = 1` on the episode no version changes.
* `animeupdatesView` excludes an episode whose `date_fetch <= animes.date_added` and every episode of a non-favorite anime.
* `delete_animes_not_in_library_by_anime_ids` with a favorite id in the list leaves that anime untouched and cascades episodes and history for the others.
* `upsert_track` twice with the same `(anime_id, sync_id)` keeps one row and the same `_id`.
