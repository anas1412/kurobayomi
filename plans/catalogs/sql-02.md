# SQL catalog, chunk 2 of 4

Source of the item list: `python3 plans/_tools/inventory.py items sql 2 4` (38 items). First item: `excluded_scanlators_manga_id_index`. Last item: `getExcludedScanlatorsByMangaId`. Every path is repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`). Cross-references use `area:<id>`: the schemas and migration histories are owned by area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations, the repositories by area:05-data-repos-entries-items and area:06-data-repos-history-track-source-misc, the extension tables by area:18-extensions-manga, area:19-extensions-anime and area:20-extension-strategy-redesign, backup use by area:21-backup-format-and-create and area:22-backup-restore. This catalog supplies the per-statement facts; it does not replace those areas.

The 38 items are 1 index, 2 tables and 35 named SQLDelight statements. Four statement names occur in several `.sq` files (`findAll` in 4, `findOne` in 3, `getCategories` in 2, `getCategory` in 2); each file's copy is documented separately under the one heading. Six statements have **no live caller** in the Kotlin tree: `getAllAnime`, `getAllManga`, `getAnimesWithFavoriteTimestamp` and `get` (extension_store) are never referenced, and `getBookmarkedChaptersByMangaId` / `getBookmarkedEpisodesByAnimeId` are reached only through repository methods that nothing calls. They still get a full entry and a keep/drop decision.

## 1. How Aniyomi opens and uses the database (OBSERVED, needed to read the entries)

* **Two separate SQLite files**, one SQLDelight database each (`data/build.gradle.kts:15-27`):
  * manga: class `Database`, package `tachiyomi.data`, file name `tachiyomi.db`, sources `data/src/main/sqldelight`, opened at `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:78-100`.
  * anime: class `AnimeDatabase`, package `tachiyomi.mi.data`, file name `tachiyomi.animedb`, sources `data/src/main/sqldelightanime`, opened at `AppModule.kt:103-124`.
  * Table names collide across the two files (`categories`, `extension_repos` historically); they never interact, there are no cross-database joins or foreign keys.
* **Open-time pragmas** (run in `onOpen` for every connection, `AppModule.kt:91-93` manga and `AppModule.kt:116-118` anime): `PRAGMA foreign_keys = ON`, `PRAGMA journal_mode = WAL`, `PRAGMA synchronous = NORMAL`. No `busy_timeout`, no `recursive_triggers` (SQLite default OFF, which the trigger designs below rely on). Driver: Requery SQLite (`RequerySQLiteOpenHelperFactory`), or the framework helper in debug builds on API 30+.
* **Schema version**: SQLDelight sets the database version to the highest `.sqm` number plus one. Manga: `data/src/main/sqldelight/migrations/32.sqm` is the highest, so version 33. Anime: `data/src/main/sqldelightanime/migrations/138.sqm` is the highest, so version 139. (Derived from the SQLDelight rule; the number does not appear as a constant in the tree.) The anime migration directory starts at `113.sqm`; anime migration `N+100` mirrors manga migration `N` for N = 13..26, 30..32 (113=13, 114=14, 115=15, 117=16, 118=17, 119=18, 120=19, 121=20, 122=21 and 22, 123=23, 124=24, 125=25, 126=26, 127=30, 128=31, 130=32). Anime-only: 116 (view fix), 129 (custom_buttons), 131 to 138 (seasons, views, fillermark, memo, extension_store, anime_relations). Manga-only: 27 (excluded_scanlators), 28 and 29 (data fixes).
* **Fresh install versus upgrade differ** (important for the Rust schema, see section 7): a fresh install executes the `.sq` files (final schema); an upgrade runs the `.sqm` chain. The two disagree in several places (index name, trigger names, column defaults, one join-table column). All differences are listed in section 7.
* **Column adapters** (`data/src/main/java/tachiyomi/data/DatabaseAdapter.kt`):
  * `Boolean` columns (`INTEGER AS Boolean`): stored 0 or 1.
  * `genre TEXT AS List<String>` via `StringListColumnAdapter` (`DatabaseAdapter.kt:14-24`): separator is the two-character string `", "` (comma, space); the empty string decodes to an empty list; encode is `joinToString(", ")`. A tag containing `", "` does not round trip. NULL stays NULL (nullable column, mapper receives `List<String>?`).
  * `update_strategy` via `MangaUpdateStrategyColumnAdapter` / `AnimeUpdateStrategyColumnAdapter` (`DatabaseAdapter.kt:26-34`): stored as the enum **ordinal**: 0 = `ALWAYS_UPDATE`, 1 = `ONLY_FETCH_ONCE`; any other number decodes to `ALWAYS_UPDATE`.
  * `fetch_type` via `FetchTypeColumnAdapter` (`DatabaseAdapter.kt:36-39`): ordinal of `FetchType` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt`): 0 = `Seasons`, 1 = `Episodes`; unknown number decodes to `Episodes`.
  * `memo BLOB AS JsonObject` via `MemoColumnAdapter` (`DatabaseAdapter.kt:41-48`): the UTF-8 bytes of a JSON object, default `x'7b7d'` which is the two bytes `{}`; decode throws on invalid JSON.
  * `Date` adapter `DateColumnAdapter` (history table, not in this chunk): epoch milliseconds.
* **Time units**: item dates (`date_fetch`, `date_upload`, `date_added`, `last_update`, `next_update`, `cover_last_modified`) are epoch **milliseconds** (evidence: `MangaRepositoryImpl.kt:75` computes `toEpochSecond() * 1000` before comparing with `next_update`). `last_modified_at` and `favorite_modified_at` are written by SQL `strftime('%s', 'now')`, i.e. epoch **seconds** (text result coerced to INTEGER by column affinity).
* **Partial-update pattern `coalesce(:x, column)`**: every `update` statement sets each column to `coalesce(:param, column)`; a NULL parameter means "leave unchanged". Consequence: an update can never set a nullable column back to NULL (for example `parent_id`, `description`, `scanlator`, `artist`), which is why dedicated statements such as `removeParentIdByIds` exist. SQL text with named parameters is rewritten by SQLDelight to positional `?` placeholders.
* **List parameters `IN :ids`**: SQLDelight expands to `IN (?, ?, ?)` with one placeholder (bound variable) per element. There is no chunking in the Kotlin code, so very large lists hit SQLite's variable limit.
* **Triggers fire only AFTER UPDATE and rely on recursive triggers being OFF**: for example `update_last_modified_at_chapters` runs an `UPDATE chapters` inside an `AFTER UPDATE ON chapters` trigger; with `recursive_triggers` off SQLite does not re-enter the same trigger, but other triggers on the same table do fire for the inner update (harmless, because the version triggers test `old` against `new` columns that the inner update does not change). `new.description != old.description` yields NULL (falsy) when either side is NULL, so a description going from or to NULL does **not** bump `version`.
* **Reactive reads**: `handler.subscribeToList/One/OneOrNull` (`data/src/main/java/tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt:65-75`, manga twin `handlers/manga/AndroidMangaDatabaseHandler.kt`) wrap SQLDelight `asFlow().mapToList(queryDispatcher)`: the query re-runs and re-emits whenever a write notifies a table that the query reads. `await*` variants run once. `await(inTransaction = true)` wraps the block in a transaction (`AndroidAnimeDatabaseHandler.kt:90-104`).
* **`executeAsOne` semantics**: `awaitOne` throws when no row is returned; `awaitOneOrNull` returns null; `awaitList` returns an empty list.

## 2. Proposal: Rust database conventions used by every "Rust" block below

* **Two database files, same table and column names as Aniyomi**: `$XDG_DATA_HOME/kurobayomi/manga.db` and `$XDG_DATA_HOME/kurobayomi/anime.db`, schema version in `PRAGMA user_version` (initial value 1, then +1 per Rust migration; do not copy the Aniyomi numbers 33 and 139). Reason: keeping the names makes every statement in this catalog copy-and-paste valid and keeps Aniyomi backup import (area:22-backup-restore) a field-for-field mapping. A single file was rejected because `categories` exists in both schemas with different join tables.
* **Fresh-schema rule**: the Rust initial migration is the `.sq` final schema from section 3 **plus** the column defaults that only the `.sqm` chain declares (`season_flags DEFAULT 0`, `season_number DEFAULT -1.0`, `background_last_modified DEFAULT 0`, `fillermark DEFAULT 0`) so an INSERT that omits them cannot fail. Use the **`.sq` trigger and index names** (section 7 lists the differing names).
* **Connection setup** (rusqlite 0.3x, feature `bundled`, `hooks`, `functions`): every connection runs `PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 5000;` (busy timeout is new: Aniyomi has none, Kurobayomi writes from several `spawn_blocking` tasks). One writer `Connection` behind a `Mutex` plus a small read-only pool; `foreign_keys` is per connection, so the pool must run the pragma in its connection initialiser. Never enable `recursive_triggers`.
* **Row mapping by column name, never by position**: `SELECT *` is acceptable only together with `row.get("column")` mapping, because an upgraded Aniyomi database and a fresh one can order columns differently.
* **Lists** (`IN :ids`): bind one JSON text parameter and use `IN (SELECT value FROM json_each(?1))`. This avoids the variable limit and string building. Serialise with `serde_json::to_string(&ids)`.
* **Case folding**: SQLite's built-in `LOWER()` folds ASCII only, while Kotlin `String.lowercase()` is Unicode-aware. Register `lower_u` (below) on every connection and use it where the Kotlin caller compares to a Kotlin-lowercased string:

```rust
use rusqlite::{functions::FunctionFlags, Connection};

pub fn register_functions(c: &Connection) -> rusqlite::Result<()> {
    c.create_scalar_function(
        "lower_u",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| Ok(ctx.get::<Option<String>>(0)?.map(|s| s.to_lowercase())),
    )
}
```

* **Change notification** (replaces SQLDelight `asFlow()`): a `ChangeBus` (`tokio::sync::broadcast::Sender<&'static str>`) that every write path calls with the table names it touched, including tables changed by `ON DELETE CASCADE` and by triggers (SQLite's `update_hook` does not report every cascaded delete reliably, so list them explicitly per write method). Each subscription re-runs its query when it sees a table in its read set and emits only if the result differs from the last one. Read sets for the statements below are given per entry as "Notify tables".
* **Row types** used in the signatures (all `#[derive(Clone, Debug)]`; `Option<T>` mirrors a nullable column):

```rust
pub struct Manga { pub id: i64, pub source: i64, pub url: String, pub artist: Option<String>, pub author: Option<String>,
    pub description: Option<String>, pub genre: Option<Vec<String>>, pub title: String, pub status: i64,
    pub thumbnail_url: Option<String>, pub favorite: bool, pub last_update: i64 /* NULL -> 0 */, pub next_update: i64 /* NULL -> 0 */,
    pub initialized: bool, pub viewer: i64, pub chapter_flags: i64, pub cover_last_modified: i64, pub date_added: i64,
    pub update_strategy: UpdateStrategy, pub calculate_interval: i64, pub last_modified_at: i64,
    pub favorite_modified_at: Option<i64>, pub version: i64, pub is_syncing: i64 }
pub struct Anime { pub id: i64, pub source: i64, pub url: String, pub artist: Option<String>, pub author: Option<String>,
    pub description: Option<String>, pub genre: Option<Vec<String>>, pub title: String, pub status: i64,
    pub thumbnail_url: Option<String>, pub favorite: bool, pub last_update: i64 /* NULL -> 0 */, pub next_update: i64 /* NULL -> 0 */,
    pub initialized: bool, pub viewer: i64, pub episode_flags: i64, pub cover_last_modified: i64, pub date_added: i64,
    pub update_strategy: UpdateStrategy, pub calculate_interval: i64, pub last_modified_at: i64,
    pub favorite_modified_at: Option<i64>, pub version: i64, pub is_syncing: i64,
    pub fetch_type: FetchType, pub parent_id: Option<i64>, pub season_flags: i64, pub season_number: f64,
    pub season_source_order: i64, pub background_url: Option<String>, pub background_last_modified: i64,
    pub memo: serde_json::Map<String, serde_json::Value> }
pub struct Chapter { pub id: i64, pub manga_id: i64, pub url: String, pub name: String, pub scanlator: Option<String>,
    pub read: bool, pub bookmark: bool, pub last_page_read: i64, pub chapter_number: f64, pub source_order: i64,
    pub date_fetch: i64, pub date_upload: i64, pub last_modified_at: i64, pub version: i64, pub is_syncing: i64 }
pub struct Episode { pub id: i64, pub anime_id: i64, pub url: String, pub name: String, pub scanlator: Option<String>,
    pub seen: bool, pub bookmark: bool, pub last_second_seen: i64, pub total_seconds: i64, pub episode_number: f64,
    pub source_order: i64, pub date_fetch: i64, pub date_upload: i64, pub last_modified_at: i64, pub version: i64,
    pub is_syncing: i64, pub summary: Option<String>, pub preview_url: Option<String>, pub fillermark: bool,
    pub memo: serde_json::Map<String, serde_json::Value> }
pub struct Category { pub id: i64, pub name: String, pub order: i64, pub flags: i64, pub hidden: bool }
pub struct StubSource { pub id: i64, pub lang: String, pub name: String }
pub struct ExtensionRepo { pub base_url: String, pub name: String, pub short_name: Option<String>, pub website: String,
    pub signing_key_fingerprint: String }
pub struct ExtensionStore { pub index_url: String, pub name: String, pub badge_label: String, pub signing_key: String,
    pub contact_website: String, pub contact_discord: Option<String>, pub is_legacy: bool, pub extension_list_url: Option<String> }
pub struct CustomButton { pub id: i64, pub name: String, pub is_favorite: bool, pub sort_index: i64, pub content: String,
    pub long_press_content: String, pub on_startup: String }
pub struct AnimeTrack { pub id: i64, pub anime_id: i64, pub tracker_id: i64, pub remote_id: i64, pub library_id: Option<i64>,
    pub title: String, pub last_episode_seen: f64, pub total_episodes: i64, pub status: i64, pub score: f64,
    pub remote_url: String, pub start_date: i64, pub finish_date: i64, pub private: bool }
pub struct SeasonAnime { pub anime: Anime, pub total_count: i64, pub seen_count: i64, pub latest_upload: i64,
    pub fetched_at: i64, pub last_seen: i64, pub bookmark_count: i64, pub fillermark_count: i64 }
pub struct DeletableAnime { pub anime_id: i64, pub source_id: i64, pub fetch_type: FetchType }
```

* Shared helpers (referenced by the entries; implement once in `db/mod.rs`). Every `*_from_row` reads **by column name** so it works for `SELECT *`, for views (extra columns are ignored) and for any column order:

```rust
use rusqlite::{OptionalExtension, Result, Row};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateStrategy { AlwaysUpdate = 0, OnlyFetchOnce = 1 }
impl UpdateStrategy { pub fn from_ordinal(n: i64) -> Self { if n == 1 { Self::OnlyFetchOnce } else { Self::AlwaysUpdate } } }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FetchType { Seasons = 0, Episodes = 1 }
impl FetchType { pub fn from_ordinal(n: i64) -> Self { if n == 0 { Self::Seasons } else { Self::Episodes } } }

pub fn genre_from_sql(s: Option<String>) -> Option<Vec<String>> {          // StringListColumnAdapter
    s.map(|s| if s.is_empty() { Vec::new() } else { s.split(", ").map(str::to_owned).collect() })
}
pub fn genre_to_sql(g: &Option<Vec<String>>) -> Option<String> { g.as_ref().map(|v| v.join(", ")) }
pub fn memo_from_sql(b: Vec<u8>) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_slice(&b).unwrap_or_default()   // Aniyomi throws; Kurobayomi falls back to {} and logs
}

pub fn manga_from_row(r: &Row) -> Result<Manga> {
    Ok(Manga {
        id: r.get("_id")?, source: r.get("source")?, url: r.get("url")?, artist: r.get("artist")?, author: r.get("author")?,
        description: r.get("description")?, genre: genre_from_sql(r.get("genre")?), title: r.get("title")?,
        status: r.get("status")?, thumbnail_url: r.get("thumbnail_url")?, favorite: r.get("favorite")?,
        last_update: r.get::<_, Option<i64>>("last_update")?.unwrap_or(0),
        next_update: r.get::<_, Option<i64>>("next_update")?.unwrap_or(0),
        initialized: r.get("initialized")?, viewer: r.get("viewer")?, chapter_flags: r.get("chapter_flags")?,
        cover_last_modified: r.get("cover_last_modified")?, date_added: r.get("date_added")?,
        update_strategy: UpdateStrategy::from_ordinal(r.get("update_strategy")?),
        calculate_interval: r.get("calculate_interval")?, last_modified_at: r.get("last_modified_at")?,
        favorite_modified_at: r.get("favorite_modified_at")?, version: r.get("version")?, is_syncing: r.get("is_syncing")?,
    })
}
pub fn anime_from_row(r: &Row) -> Result<Anime> {
    Ok(Anime {
        id: r.get("_id")?, source: r.get("source")?, url: r.get("url")?, artist: r.get("artist")?, author: r.get("author")?,
        description: r.get("description")?, genre: genre_from_sql(r.get("genre")?), title: r.get("title")?,
        status: r.get("status")?, thumbnail_url: r.get("thumbnail_url")?, favorite: r.get("favorite")?,
        last_update: r.get::<_, Option<i64>>("last_update")?.unwrap_or(0),
        next_update: r.get::<_, Option<i64>>("next_update")?.unwrap_or(0),
        initialized: r.get("initialized")?, viewer: r.get("viewer")?, episode_flags: r.get("episode_flags")?,
        cover_last_modified: r.get("cover_last_modified")?, date_added: r.get("date_added")?,
        update_strategy: UpdateStrategy::from_ordinal(r.get("update_strategy")?),
        calculate_interval: r.get("calculate_interval")?, last_modified_at: r.get("last_modified_at")?,
        favorite_modified_at: r.get("favorite_modified_at")?, version: r.get("version")?, is_syncing: r.get("is_syncing")?,
        fetch_type: FetchType::from_ordinal(r.get("fetch_type")?), parent_id: r.get("parent_id")?,
        season_flags: r.get("season_flags")?, season_number: r.get("season_number")?,
        season_source_order: r.get("season_source_order")?, background_url: r.get("background_url")?,
        background_last_modified: r.get("background_last_modified")?, memo: memo_from_sql(r.get("memo")?),
    })
}
pub fn chapter_from_row(r: &Row) -> Result<Chapter> {
    Ok(Chapter {
        id: r.get("_id")?, manga_id: r.get("manga_id")?, url: r.get("url")?, name: r.get("name")?,
        scanlator: r.get("scanlator")?, read: r.get("read")?, bookmark: r.get("bookmark")?,
        last_page_read: r.get("last_page_read")?, chapter_number: r.get("chapter_number")?,
        source_order: r.get("source_order")?, date_fetch: r.get("date_fetch")?, date_upload: r.get("date_upload")?,
        last_modified_at: r.get("last_modified_at")?, version: r.get("version")?, is_syncing: r.get("is_syncing")?,
    })
}
pub fn episode_from_row(r: &Row) -> Result<Episode> {
    Ok(Episode {
        id: r.get("_id")?, anime_id: r.get("anime_id")?, url: r.get("url")?, name: r.get("name")?,
        scanlator: r.get("scanlator")?, seen: r.get("seen")?, bookmark: r.get("bookmark")?,
        last_second_seen: r.get("last_second_seen")?, total_seconds: r.get("total_seconds")?,
        episode_number: r.get("episode_number")?, source_order: r.get("source_order")?,
        date_fetch: r.get("date_fetch")?, date_upload: r.get("date_upload")?,
        last_modified_at: r.get("last_modified_at")?, version: r.get("version")?, is_syncing: r.get("is_syncing")?,
        summary: r.get("summary")?, preview_url: r.get("preview_url")?, fillermark: r.get("fillermark")?,
        memo: memo_from_sql(r.get("memo")?),
    })
}
pub fn category_from_row(r: &Row) -> Result<Category> {   // columns aliased id, name, order, flags, hidden
    Ok(Category { id: r.get("id")?, name: r.get("name")?, order: r.get("order")?, flags: r.get("flags")?,
                  hidden: r.get::<_, i64>("hidden")? == 1 })
}
pub fn repo_from_row(r: &Row) -> Result<ExtensionRepo> {
    Ok(ExtensionRepo { base_url: r.get("base_url")?, name: r.get("name")?, short_name: r.get("short_name")?,
                       website: r.get("website")?, signing_key_fingerprint: r.get("signing_key_fingerprint")? })
}
pub fn store_from_row(r: &Row) -> Result<ExtensionStore> {
    Ok(ExtensionStore { index_url: r.get("index_url")?, name: r.get("name")?, badge_label: r.get("badge_label")?,
        signing_key: r.get("signing_key")?, contact_website: r.get("contact_website")?,
        contact_discord: r.get("contact_discord")?, is_legacy: r.get("is_legacy")?,
        extension_list_url: r.get("extension_list_url")? })
}
pub fn anime_track_from_row(r: &Row) -> Result<AnimeTrack> {
    Ok(AnimeTrack { id: r.get("_id")?, anime_id: r.get("anime_id")?, tracker_id: r.get("sync_id")?,
        remote_id: r.get("remote_id")?, library_id: r.get("library_id")?, title: r.get("title")?,
        last_episode_seen: r.get("last_episode_seen")?, total_episodes: r.get("total_episodes")?,
        status: r.get("status")?, score: r.get("score")?, remote_url: r.get("remote_url")?,
        start_date: r.get("start_date")?, finish_date: r.get("finish_date")?, private: r.get("private")? })
}
```

## 3. Schema reference for every table, view, trigger and index the 38 items touch (OBSERVED)

DDL below is the **final `.sq` text** (what a fresh install creates). The "Introduced" column names the migration that first added the column on an upgraded database; "baseline" means it exists in the oldest schema the migration chain starts from (manga: before `1.sqm`; anime: before `113.sqm`) and was re-created with its final constraints by the table rebuild in `14.sqm` (manga) or `114.sqm` (anime).

### 3.1 `mangas` (`data/src/main/sqldelight/data/mangas.sq:6-31`)

| Column | Type / constraint | Introduced |
|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY | baseline |
| `source` | INTEGER NOT NULL | baseline |
| `url` | TEXT NOT NULL | baseline |
| `artist` | TEXT | baseline |
| `author` | TEXT | baseline |
| `description` | TEXT | baseline |
| `genre` | TEXT (adapter List&lt;String&gt;, separator `", "`) | baseline |
| `title` | TEXT NOT NULL | baseline |
| `status` | INTEGER NOT NULL (SManga: UNKNOWN 0, ONGOING 1, COMPLETED 2, LICENSED 3, PUBLISHING_FINISHED 4, CANCELLED 5, ON_HIATUS 6; `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:48-54`) | baseline |
| `thumbnail_url` | TEXT | baseline |
| `favorite` | INTEGER AS Boolean NOT NULL | baseline |
| `last_update` | INTEGER (nullable, ms) | baseline |
| `next_update` | INTEGER (nullable, ms) | `11.sqm` (`DEFAULT 0`; the default was dropped when `14.sqm` rebuilt the table) |
| `initialized` | INTEGER AS Boolean NOT NULL | baseline |
| `viewer` | INTEGER NOT NULL (reader viewer flags) | baseline |
| `chapter_flags` | INTEGER NOT NULL (sort/filter/display bit flags, `Manga.kt:77-103`) | baseline |
| `cover_last_modified` | INTEGER NOT NULL | `9.sqm` (`DEFAULT 0`) |
| `date_added` | INTEGER NOT NULL | `10.sqm` (`DEFAULT 0`; its backfill `UPDATE` uses an uncorrelated scalar subquery, so every row got the same value) |
| `update_strategy` | INTEGER AS UpdateStrategy NOT NULL DEFAULT 0 | `20.sqm` |
| `calculate_interval` | INTEGER DEFAULT 0 NOT NULL | `24.sqm` |
| `last_modified_at` | INTEGER NOT NULL DEFAULT 0 (seconds) | `26.sqm` |
| `favorite_modified_at` | INTEGER (nullable, seconds) | `26.sqm` |
| `version` | INTEGER NOT NULL DEFAULT 0 | `30.sqm` |
| `is_syncing` | INTEGER NOT NULL DEFAULT 0 | `30.sqm` |

Indexes: `library_favorite_index ON mangas(favorite) WHERE favorite = 1` (`7.sqm`, recreated `14.sqm`), `mangas_url_index ON mangas(url)` (baseline, recreated `14.sqm`). Triggers (final `.sq`, lines 36-61): `update_last_favorite_at_mangas` (AFTER UPDATE OF favorite: sets `favorite_modified_at = strftime('%s','now')`), `update_last_modified_at_mangas` (AFTER UPDATE FOR EACH ROW: sets `last_modified_at = strftime('%s','now')`), `update_manga_version` (AFTER UPDATE: `version = version + 1` when `new.is_syncing = 0` and `url`, `description` or `favorite` changed).

### 3.2 `animes` (`data/src/main/sqldelightanime/dataanime/animes.sq:8-41`)

Same columns as `mangas` (with `chapter_flags` renamed `episode_flags`, `update_strategy` typed `AnimeUpdateStrategy`) in this order, plus:

| Column | Type / constraint | Introduced |
|---|---|---|
| `update_strategy` | INTEGER NOT NULL DEFAULT 0 | `121.sqm` |
| `calculate_interval` | INTEGER DEFAULT 0 NOT NULL | `124.sqm` |
| `last_modified_at` | INTEGER NOT NULL DEFAULT 0 | `126.sqm` |
| `favorite_modified_at` | INTEGER | `126.sqm` |
| `version` | INTEGER NOT NULL DEFAULT 0 | `127.sqm` |
| `is_syncing` | INTEGER NOT NULL DEFAULT 0 | `127.sqm` |
| `fetch_type` | INTEGER AS FetchType NOT NULL DEFAULT 1 | `131.sqm`; data fix `132.sqm` remaps old ordinals (`2 -> 1`, `1 -> 0`, `0 -> 1`); data fix `135.sqm` sets every row to 1 (Episodes) |
| `parent_id` | INTEGER (nullable, season parent anime `_id`) | `131.sqm` |
| `season_flags` | INTEGER NOT NULL (`.sqm`: DEFAULT 0) | `131.sqm` |
| `season_number` | REAL NOT NULL (`.sqm`: DEFAULT -1.0) | `131.sqm` |
| `season_source_order` | INTEGER NOT NULL (`.sqm`: DEFAULT 0) | `131.sqm` |
| `background_url` | TEXT | `134.sqm` |
| `background_last_modified` | INTEGER NOT NULL (`.sqm`: DEFAULT 0) | `134.sqm` |
| `memo` | BLOB AS JsonObject NOT NULL DEFAULT '{}' (`.sqm`: `x'7b7d'`) | `136.sqm` |

`animes.status` additionally uses `SAnime.UPCOMING = 7` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-79`: UNKNOWN 0, ONGOING 1, COMPLETED 2, LICENSED 3, PUBLISHING_FINISHED 4, CANCELLED 5, ON_HIATUS 6, UPCOMING 7). Indexes: `animelib_favorite_index ON animes(favorite) WHERE favorite = 1` (the `.sqm` chain creates it as `library_favorite_index`, see section 7), `animes_url_index ON animes(url)`, `animes_parent_id ON animes(parent_id)` (131/133), `animes_fetch_type ON animes(fetch_type)` (131/133). Triggers: `update_last_favorite_at_animes`, `update_last_modified_at_animes`, `update_anime_version` with the same bodies as the manga ones.

### 3.3 `chapters` (`data/src/main/sqldelight/data/chapters.sq:3-21`)

| Column | Type / constraint | Introduced |
|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY | baseline |
| `manga_id` | INTEGER NOT NULL, FK -> `mangas(_id)` ON DELETE CASCADE | baseline |
| `url` | TEXT NOT NULL | baseline |
| `name` | TEXT NOT NULL | baseline |
| `scanlator` | TEXT (nullable) | `4.sqm` (`DEFAULT NULL`); `28.sqm` runs `UPDATE chapters SET scanlator = trim(scanlator) WHERE scanlator IS NOT NULL` |
| `read` | INTEGER AS Boolean NOT NULL | baseline |
| `bookmark` | INTEGER AS Boolean NOT NULL | `3.sqm` (`DEFAULT 0`) |
| `last_page_read` | INTEGER NOT NULL | baseline |
| `chapter_number` | REAL NOT NULL | baseline |
| `source_order` | INTEGER NOT NULL | `1.sqm` (`DEFAULT 0`) |
| `date_fetch` | INTEGER NOT NULL (ms) | baseline |
| `date_upload` | INTEGER NOT NULL (ms) | baseline; `13.sqm` sets `date_upload = date_fetch` where it was 0 |
| `last_modified_at` | INTEGER NOT NULL DEFAULT 0 (seconds) | `26.sqm` |
| `version` | INTEGER NOT NULL DEFAULT 0 | `30.sqm` |
| `is_syncing` | INTEGER NOT NULL DEFAULT 0 | `30.sqm` |

Indexes: `chapters_manga_id_index ON chapters(manga_id)`, `chapters_unread_by_manga_index ON chapters(manga_id, read) WHERE read = 0` (`7.sqm`; both recreated by `14.sqm`). Triggers: `update_last_modified_at_chapters` (AFTER UPDATE FOR EACH ROW: `last_modified_at = strftime('%s','now')`), `update_chapter_and_manga_version` (AFTER UPDATE `WHEN new.is_syncing = 0 AND (new.read != old.read OR new.bookmark != old.bookmark OR new.last_page_read != old.last_page_read)`: `chapters.version += 1`, and `mangas.version += 1` for `new.manga_id` when `(SELECT is_syncing FROM mangas WHERE _id = new.manga_id) = 0`).

### 3.4 `episodes` (`data/src/main/sqldelightanime/dataanime/episodes.sq:4-27`)

Same shape as `chapters` with these differences: parent column `anime_id` (FK -> `animes(_id)` ON DELETE CASCADE), `seen` instead of `read`, `last_second_seen INTEGER NOT NULL` instead of `last_page_read`, `total_seconds INTEGER NOT NULL`, `episode_number REAL NOT NULL`; extra columns `summary TEXT`, `preview_url TEXT`, `fillermark INTEGER AS Boolean NOT NULL` (all three introduced by `134.sqm`; `.sqm` declares `fillermark DEFAULT 0`), `memo BLOB AS JsonObject NOT NULL DEFAULT '{}'` (`136.sqm`). `last_modified_at` is `126.sqm`; `version`, `is_syncing` are `127.sqm`; `113.sqm` is the `date_upload` backfill. Indexes `episodes_anime_id_index ON episodes(anime_id)` and `episodes_unseen_by_anime_index ON episodes(anime_id, seen) WHERE seen = 0` (recreated by `114.sqm`). Triggers `update_last_modified_at_episodes` and `update_episode_and_anime_version` (condition on `seen`, `bookmark`, `last_second_seen`; bumps `episodes.version` and `animes.version`). Note `fillermark` and `summary`/`preview_url`/`memo` changes do not bump versions.

### 3.5 `categories` (identical text in `data/src/main/sqldelight/data/categories.sq:1-18` and `data/src/main/sqldelightanime/dataanime/categories.sq:1-18`)

| Column | Type / constraint | Introduced |
|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY | baseline |
| `name` | TEXT NOT NULL | baseline |
| `sort` | INTEGER NOT NULL | baseline |
| `flags` | INTEGER NOT NULL (library sort type + direction bits, see `getCategory`) | baseline |
| `hidden` | INTEGER NOT NULL DEFAULT 0 | `25.sqm` (manga), `125.sqm` (anime) |

Seed row: `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0);` (`19.sqm` manga, `120.sqm` anime) is the **system ("Default") category**: id 0 (`Category.UNCATEGORIZED_ID`, `domain/src/main/java/tachiyomi/domain/category/model/Category.kt:13-16`), empty name, sort -1; the UI shows the string `label_default` ("Default", `i18n/src/commonMain/moko-resources/base/strings.xml:39`) for it (`app/src/main/java/eu/kanade/presentation/category/CategoryExtensions.kt:13-19`). Trigger `system_category_delete_trigger BEFORE DELETE ON categories` raises `RAISE(ABORT, "System category can't be deleted")` when `old._id <= 0` (anime upgrade path names it `system_anime_category_delete_trigger`, `120.sqm`). There is **no UNIQUE constraint on `name`**; duplicate-name rejection (`error_category_exists`, "A category with this name already exists!") is done in the UI (`app/src/main/java/eu/kanade/presentation/category/components/CategoryDialogs.kt:83` and `:147`).

### 3.6 Join tables `mangas_categories` and `animes_categories` (`data/src/main/sqldelight/data/mangas_categories.sq`, `data/src/main/sqldelightanime/dataanime/animes_categories.sq`)

`mangas_categories(_id INTEGER NOT NULL PRIMARY KEY, manga_id INTEGER NOT NULL, category_id INTEGER NOT NULL, FOREIGN KEY(category_id) REFERENCES categories(_id) ON DELETE CASCADE, FOREIGN KEY(manga_id) REFERENCES mangas(_id) ON DELETE CASCADE)`; anime twin `animes_categories(_id, anime_id, category_id)`. No index on either foreign key and no UNIQUE(manga_id, category_id). Trigger `insert_manga_category_update_version` / `insert_anime_category_update_version` (AFTER INSERT, `30.sqm` / `127.sqm`): bumps the parent's `version` when its `is_syncing = 0`. Upgraded databases additionally carry `last_modified_at INTEGER NOT NULL DEFAULT 0` and trigger `update_last_modified_at_mangas_categories` / `update_last_modified_at_animes_categories` (`26.sqm`, `126.sqm`) that the `.sq` files do not declare (section 7).

### 3.7 `excluded_scanlators` (`data/src/main/sqldelight/data/excluded_scanlators.sq`, manga only, created by `27.sqm`)

`CREATE TABLE excluded_scanlators(manga_id INTEGER NOT NULL, scanlator TEXT NOT NULL, FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE)`. No primary key and no UNIQUE (rowid table). There is no anime equivalent: episodes have a `scanlator` column but Aniyomi has no exclusion list for them.

### 3.8 `anime_sync` (`data/src/main/sqldelightanime/dataanime/anime_sync.sq:3-21`, tracker bindings)

| Column | Type / constraint | Introduced |
|---|---|---|
| `_id` | INTEGER NOT NULL PRIMARY KEY | baseline |
| `anime_id` | INTEGER NOT NULL, FK -> `animes(_id)` ON DELETE CASCADE | baseline |
| `sync_id` | INTEGER NOT NULL (tracker id) | baseline |
| `remote_id` | INTEGER NOT NULL | baseline |
| `library_id` | INTEGER (nullable) | baseline |
| `title` | TEXT NOT NULL | baseline |
| `last_episode_seen` | REAL NOT NULL | baseline |
| `total_episodes` | INTEGER NOT NULL | baseline |
| `status` | INTEGER NOT NULL | baseline |
| `score` | REAL NOT NULL | baseline |
| `remote_url` | TEXT NOT NULL | baseline |
| `start_date` | INTEGER NOT NULL | baseline |
| `finish_date` | INTEGER NOT NULL | baseline |
| `private` | INTEGER AS Boolean DEFAULT 0 NOT NULL | `130.sqm` |

Constraint `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE`: inserting a second binding of the same tracker to the same anime silently **deletes the old row** and inserts the new one (new `_id`).

### 3.9 `sources` and `animesources` (stub sources)

`sources(_id INTEGER NOT NULL PRIMARY KEY, lang TEXT NOT NULL, name TEXT NOT NULL)` (`data/src/main/sqldelight/data/sources.sq`, `16.sqm`) and the identical `animesources` (`data/src/main/sqldelightanime/dataanime/animesources.sq`, `117.sqm`). They remember the language and display name of every source that has ever been loaded so that library entries of an uninstalled extension can still show a source name (stub sources).

### 3.10 Views touched by `getAnimeSeasonsById` and `getDeletableParentAnime` (anime only)

* `episodestatsView` (`data/src/main/sqldelightanime/view/episodestatsView.sq`; `131.sqm`, extended by `134.sqm`):

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

* `animehistorystatsView` (`data/src/main/sqldelightanime/view/animehistorystatsView.sq`; `131.sqm`):

```sql
CREATE VIEW animehistorystatsView AS
SELECT
    episodes.anime_id,
    max(ah.last_seen) AS lastSeen
FROM episodes
LEFT JOIN animehistory AS ah ON episodes._id = ah.episode_id
GROUP BY episodes.anime_id;
```

* `animeseasonstatsView` (`data/src/main/sqldelightanime/view/animeseasonstatsView.sq`; `133.sqm`, extended by `134.sqm`):

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

* `animeseasonsView` and `animedeletableView` are documented under `getAnimeSeasonsById` and `getDeletableParentAnime`.

Other tables that cascade from the rows above and are therefore in every "Notify tables" list: `animehistory` (`episode_id` FK -> `episodes`), `history` (`chapter_id` FK -> `chapters`), `anime_relations` (`anime_id` and `related_anime_id` FKs -> `animes`, `data/src/main/sqldelightanime/dataanime/anime_relations.sq`), `manga_sync`/`anime_sync`, `mangas_categories`/`animes_categories`, `excluded_scanlators`. They belong to other chunks.

## 4. Index of the 38 items

| # | Item | Kind | Defined in |
|---|---|---|---|
| 1 | `excluded_scanlators_manga_id_index` | index | `data/src/main/sqldelight/data/excluded_scanlators.sq:8` |
| 2 | `extension_repos` | table (+8 statements) | `data/src/main/sqldelight/data/extension_repos.sq` |
| 3 | `extension_store` | table (+6 statements) | `data/src/main/sqldelightanime/dataanime/extension_store.sq` |
| 4 | `findAll` | statement x4 | `extension_repos.sq:19`, `sources.sq:7`, `animesources.sq:7`, `custom_buttons.sq:32` |
| 5 | `findOne` | statement x3 | `extension_repos.sq:9`, `sources.sq:11`, `animesources.sq:11` |
| 6 | `findOneBySigningKeyFingerprint` | statement | `extension_repos.sq:14` |
| 7 | `get` | statement | `extension_store.sq:14` |
| 8 | `getAll` | statement | `extension_store.sq:19` |
| 9 | `getAllAnime` | statement (unused) | `animes.sq:101` |
| 10 | `getAllAnimeSourceAndUrl` | statement | `animes.sq:105` |
| 11 | `getAllManga` | statement (unused) | `mangas.sq:90` |
| 12 | `getAllMangaSourceAndUrl` | statement | `mangas.sq:94` |
| 13 | `getAnimeById` | statement | `animes.sq:75` |
| 14 | `getAnimeByUrlAndSource` | statement | `animes.sq:81` |
| 15 | `getAnimeSeasonsById` | statement on view | `view/animeseasonsView.sq:41` |
| 16 | `getAnimeSourceIdWithFavoriteCount` | statement | `animes.sq:114` |
| 17 | `getAnimeTracks` | statement | `anime_sync.sq:27` |
| 18 | `getAnimesWithFavoriteTimestamp` | statement (unused) | `animes.sq:109` |
| 19 | `getBookmarkedChaptersByMangaId` | statement | `chapters.sq:73` |
| 20 | `getBookmarkedEpisodesByAnimeId` | statement | `episodes.sq:67` |
| 21 | `getCategories` | statement x2 | `categories.sq:26` (manga and anime) |
| 22 | `getCategoriesByAnimeId` | statement | `dataanime/categories.sq:47` |
| 23 | `getCategoriesByMangaId` | statement | `data/categories.sq:47` |
| 24 | `getCategory` | statement x2 | `categories.sq:20` (manga and anime) |
| 25 | `getChapterById` | statement | `chapters.sq:51` |
| 26 | `getChapterByUrl` | statement | `chapters.sq:79` |
| 27 | `getChapterByUrlAndMangaId` | statement | `chapters.sq:84` |
| 28 | `getChaptersByMangaId` | statement | `chapters.sq:56` |
| 29 | `getChildrenByParentId` | statement | `animes.sq:157` |
| 30 | `getCount` | statement | `extension_store.sq:23` |
| 31 | `getDeletableParentAnime` | statement on view | `view/animedeletableView.sq:10` |
| 32 | `getDuplicateLibraryAnime` | statement | `animes.sq:128` |
| 33 | `getDuplicateLibraryManga` | statement | `mangas.sq:117` |
| 34 | `getEpisodeById` | statement | `episodes.sq:57` |
| 35 | `getEpisodeByUrl` | statement | `episodes.sq:74` |
| 36 | `getEpisodeByUrlAndAnimeId` | statement | `episodes.sq:79` |
| 37 | `getEpisodesByAnimeId` | statement | `episodes.sq:62` |
| 38 | `getExcludedScanlatorsByMangaId` | statement | `excluded_scanlators.sq:19` |

(Directory shorthand: `data/src/main/sqldelight/data/` for manga `.sq` files, `data/src/main/sqldelightanime/dataanime/` for anime `.sq` files.)

## 5. Entries, in inventory order

### `excluded_scanlators_manga_id_index`

OBSERVED. Index on the manga-only exclusion table, created by `data/src/main/sqldelight/data/excluded_scanlators.sq:8` (first created by `data/src/main/sqldelight/migrations/27.sqm`).

```sql
-- excluded_scanlators.sq
CREATE INDEX excluded_scanlators_manga_id_index ON excluded_scanlators(manga_id);
```

Table it belongs to (section 3.7):

```sql
-- excluded_scanlators.sq
CREATE TABLE excluded_scanlators(
    manga_id INTEGER NOT NULL,
    scanlator TEXT NOT NULL,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id)
    ON DELETE CASCADE
);
```

* **Who uses it**: `getExcludedScanlatorsByMangaId` (`WHERE manga_id = ?`), the `remove` statement (`WHERE manga_id = ? AND scanlator IN (list)`), the `LEFT JOIN excluded_scanlators ES ON C.manga_id = ES.manga_id AND C.scanlator = ES.scanlator` inside `getChaptersByMangaId`, the same join inside `libraryView` (`27.sqm`), and SQLite's foreign-key cascade scan when a manga row is deleted.
* **Index shape**: single column `manga_id`, non-unique, non-covering (the `scanlator` value needs a table lookup per match). Nothing prevents duplicate `(manga_id, scanlator)` rows; the app avoids them in `SetExcludedScanlators.kt:14-19` (inserts only `excludedScanlators - currentExcluded`) and `MangaRestorer.kt:429-434` (filters `it !in existingExcludedScanlators`).
* **Perf**: at most a few rows per manga, so cost is negligible; the index exists so the cascade and the per-manga lookups do not scan the whole table.
* **Proposal (Rust)**: keep the table and index name, but index both columns so the chapter join is index-only. Duplicates stay legal; inserts use `INSERT OR IGNORE`-free plain `INSERT` guarded by the same set arithmetic.

```rust
pub const EXCLUDED_SCANLATORS_DDL: &str = "
CREATE TABLE excluded_scanlators(
    manga_id INTEGER NOT NULL,
    scanlator TEXT NOT NULL,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE
);
CREATE INDEX excluded_scanlators_manga_id_index ON excluded_scanlators(manga_id, scanlator);";
```

* **Decision**: port (P1 with the chapter filter in area:38-ui-entry-details-manga). Anime has no counterpart; the user loses nothing by not adding one.

### `extension_repos`

OBSERVED. Manga extension repository list (Tachiyomi/Mihon "repo" = a URL ending in `index.min.json` plus a `repo.json` metadata file). Manga database only; created by `data/src/main/sqldelight/migrations/31.sqm`. The anime database also had this table (`data/src/main/sqldelightanime/migrations/128.sqm`) until `137.sqm` copied its rows into `extension_store` and dropped it.

```sql
-- extension_repos.sq
CREATE TABLE extension_repos (
    base_url TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    short_name TEXT,
    website TEXT NOT NULL,
    signing_key_fingerprint TEXT UNIQUE NOT NULL
);
```

| Column | Meaning |
|---|---|
| `base_url` | primary key: the repo URL without the trailing `/index.min.json`, e.g. `https://raw.githubusercontent.com/keiyoushi/extensions/repo` |
| `name` | display name from `repo.json` `meta.name` |
| `short_name` | nullable, `meta.shortName`, used as a short badge |
| `website` | `meta.website`, opened by the "Open source repo" action (`action_open_repo`) |
| `signing_key_fingerprint` | `meta.signingKeyFingerprint`, UNIQUE; the SHA-256 of the signing certificate that extensions from this repo must carry. Repos migrated from the old preference list get the placeholder `NOFINGERPRINT-<n>` (`app/src/main/java/mihon/core/migration/migrations/TrustExtensionRepositoryMigration.kt:36-42`) |

Statements in `data/src/main/sqldelight/data/extension_repos.sq` (all eight, SQL verbatim):

```sql
-- extension_repos.sq
findOne:
SELECT *
FROM extension_repos
WHERE base_url = :base_url;
```
```sql
-- extension_repos.sq
findOneBySigningKeyFingerprint:
SELECT *
FROM extension_repos
WHERE signing_key_fingerprint = :fingerprint;
```
```sql
-- extension_repos.sq
findAll:
SELECT *
FROM extension_repos;
```
```sql
-- extension_repos.sq
count:
SELECT COUNT(*)
FROM extension_repos;
```
```sql
-- extension_repos.sq
insert:
INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint)
VALUES (:base_url, :name, :short_name, :website, :fingerprint);
```
```sql
-- extension_repos.sq
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
```sql
-- extension_repos.sq
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
```sql
-- extension_repos.sq
delete:
DELETE FROM extension_repos
WHERE base_url = :base_url;
```

* **Constraint behaviour** (OBSERVED): `insert` raises a constraint error when `base_url` already exists (PRIMARY KEY) or when `signing_key_fingerprint` already exists (UNIQUE). `MangaExtensionRepoRepositoryImpl.kt:44-47` and `:57-62` wrap `SQLiteException` into `SaveExtensionRepoException` (`domain/src/main/java/mihon/domain/extensionrepo/exception/SaveExtensionRepoException.kt`) without constraint details.
* **`upsert`**: conflict target is `base_url`; it overwrites the other four columns. It still raises when the new row's fingerprint equals the fingerprint of a **different** row (UNIQUE conflict on a column that is not the conflict target). The text `website =: website` (space after the colon) and `WHERE base_url = base_url` (always true) are typos that SQLDelight tolerates and rewrites to positional placeholders; write `website = :website` in Rust.
* **`replace`**: conflict target is `signing_key_fingerprint`; on conflict it **rewrites `base_url`**, `name`, `short_name`, `website` of the row owning that fingerprint, i.e. the old repo URL is replaced by the new URL. It raises if `base_url` already exists on another row. Used only after the user confirms the "same fingerprint" dialog.
* **Add-repo algorithm** (`domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt:15-63`):
  1. `indexUrl.toHttpUrlOrNull()?.toString()` must match `^https://.*/index\.min\.json$`; otherwise `Result.InvalidUrl` (UI string `invalid_repo_name`, "Invalid repo URL").
  2. `baseUrl = formattedIndexUrl.removeSuffix("/index.min.json")`.
  3. `ExtensionRepoService.fetchRepoDetails(baseUrl)` does `GET "$baseUrl/repo.json"` (`domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoService.kt:28-44`) and decodes `{"meta": {"name": string, "shortName": string|null, "website": string, "signingKeyFingerprint": string}}` (`ExtensionRepoDto.kt:6-17`); any exception returns null which yields `Result.InvalidUrl`.
  4. `insertRepo`; on `SaveExtensionRepoException` run `handleInsertionError`: if `getRepo(baseUrl)` exists then `Result.RepoAlreadyExists` (string `error_repo_exists`, "This repo already exists!"); else if `getRepoBySigningKeyFingerprint(fingerprint)` exists then `Result.DuplicateFingerprint(oldRepo, newRepo)` (UI dialog title `action_replace_repo_title` "Signing Key Fingerprint Already Exists", body `action_replace_repo_message` "Repository %1$s has the same Signing Key Fingerprint as %2$s.\nIf this is expected, %2$s will be replaced, otherwise contact your repo maintainer.", button `action_replace_repo` "Replace" which calls `replaceRepo`); else `Result.Error` (no message shown, `MangaExtensionReposScreenModel.kt:54-62`).
* **Refresh algorithm** (`UpdateMangaExtensionRepo.kt:15-29`): for every repo in parallel (`async` inside `coroutineScope`, so one thrown exception cancels the rest) fetch `repo.json`; if it fails, skip; apply `upsertRepo(newRepo)` only when the stored fingerprint starts with `NOFINGERPRINT` or equals the fetched fingerprint (a changed fingerprint is silently ignored, which protects against a hijacked repo).
* **Callers (path:line)**: `findAll` is `subscribeAll` (`MangaExtensionRepoRepositoryImpl.kt:15`, UI list `MangaExtensionReposScreenModel.kt:37`) and `getAll` (`:19`, refresh, backup creator `BackupCreator.kt` `backupMangaExtensionRepos`, restorer `MangaExtensionRepoRestorer.kt:17`); `findOne` is `getRepo` (`:23`); `findOneBySigningKeyFingerprint` is `getRepoBySigningKeyFingerprint` (`:28`); `count` is `getCount` (`:33`, flow; the "Extension repos" row count in `SettingsBrowseScreen.kt:40,43`); `insert` `:44`; `upsert` `:58`; `replace` `:66`; `delete` `:77` via `DeleteMangaExtensionRepo.kt:10`. Backup restore (`MangaExtensionRepoRestorer.kt:20-36`) fails with the exceptions `"Already Exists with different signing key fingerprint"` (same URL, other fingerprint) and `"<existing name> has the same signing key fingerprint"` (same fingerprint, other URL); otherwise `insert`.
* **Perf**: tens of rows at most; all lookups hit the PK or the auto UNIQUE index.
* **Proposal (Rust)**: port verbatim into `manga.db` (P1, area:18-extensions-manga / area:20-extension-strategy-redesign decide whether Kurobayomi keeps Mihon-style repos at all, because Android APK extensions cannot execute on Linux; the table is cheap and keeps backup import lossless). Error discrimination uses the same two follow-up lookups instead of SQLite extended codes.

```rust
pub fn repo_find_all(c: &Connection) -> rusqlite::Result<Vec<ExtensionRepo>> {
    c.prepare_cached("SELECT * FROM extension_repos")?.query_map([], repo_from_row)?.collect()
}
pub fn repo_find_one(c: &Connection, base_url: &str) -> rusqlite::Result<Option<ExtensionRepo>> {
    c.query_row("SELECT * FROM extension_repos WHERE base_url = ?1", [base_url], repo_from_row).optional()
}
pub fn repo_find_one_by_signing_key_fingerprint(c: &Connection, fp: &str) -> rusqlite::Result<Option<ExtensionRepo>> {
    c.query_row("SELECT * FROM extension_repos WHERE signing_key_fingerprint = ?1", [fp], repo_from_row).optional()
}
pub fn repo_count(c: &Connection) -> rusqlite::Result<i64> {
    c.query_row("SELECT COUNT(*) FROM extension_repos", [], |r| r.get(0))
}
pub fn repo_insert(c: &Connection, r: &ExtensionRepo) -> rusqlite::Result<usize> {
    c.execute("INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) \
               VALUES (?1, ?2, ?3, ?4, ?5)",
              (&r.base_url, &r.name, &r.short_name, &r.website, &r.signing_key_fingerprint))
}
pub fn repo_upsert(c: &Connection, r: &ExtensionRepo) -> rusqlite::Result<usize> {
    c.execute("INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) \
               VALUES (?1, ?2, ?3, ?4, ?5) \
               ON CONFLICT(base_url) DO UPDATE SET name = ?2, short_name = ?3, website = ?4, signing_key_fingerprint = ?5",
              (&r.base_url, &r.name, &r.short_name, &r.website, &r.signing_key_fingerprint))
}
pub fn repo_replace(c: &Connection, r: &ExtensionRepo) -> rusqlite::Result<usize> {
    c.execute("INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) \
               VALUES (?1, ?2, ?3, ?4, ?5) \
               ON CONFLICT(signing_key_fingerprint) DO UPDATE SET base_url = ?1, name = ?2, short_name = ?3, website = ?4",
              (&r.base_url, &r.name, &r.short_name, &r.website, &r.signing_key_fingerprint))
}
pub fn repo_delete(c: &Connection, base_url: &str) -> rusqlite::Result<usize> {
    c.execute("DELETE FROM extension_repos WHERE base_url = ?1", [base_url])
}
pub enum CreateRepoResult { Success, InvalidUrl, RepoAlreadyExists, DuplicateFingerprint { old: ExtensionRepo, new: ExtensionRepo }, Error }
```

Notify tables: `extension_repos`.

### `extension_store`

OBSERVED. Anime-only replacement of `extension_repos` (Aniyomi's "extension store": a signed index that may be gzip JSON, protobuf, or the legacy Mihon repo layout). Created by `data/src/main/sqldelightanime/migrations/137.sqm`, which also copied old `extension_repos` rows with `index_url = base_url || '/repo.json'`, `badge_label = coalesce(short_name, name)`, `signing_key = signing_key_fingerprint`, `contact_website = website`, `contact_discord = NULL`, `is_legacy = 1`, `extension_list_url = NULL`, then `DROP TABLE extension_repos`.

```sql
-- extension_store.sq
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

| Column | Meaning |
|---|---|
| `index_url` | primary key; the URL that was fetched to describe the store. Legacy stores are normalised to the index URL with `/index.min.json` replaced by `/repo.json` (`AnimeExtensionStoreService.kt:39`) |
| `name` | store display name |
| `badge_label` | short label shown on extension rows; legacy default is `meta.shortName ?: meta.name` (`NetworkLegacyAnimeExtensionRepo.kt:34`) |
| `signing_key` | the APK signing-certificate SHA-256 fingerprint that is **trusted** for every extension of this store: `TrustAnimeExtension.kt:15-17` returns `trustedFingerprints.any { fingerprints.contains(it) }` over `getAll().map { it.signingKey }`. The value `NO_SIGNING_KEY` (written for stores migrated from the preference list, `AnimeExtensionStoreRepositoryImpl.kt:29`) never matches a real fingerprint |
| `contact_website` | `contact.website` (legacy: `meta.website`; preference migration: the index URL) |
| `contact_discord` | nullable |
| `is_legacy` | 1 when the index is the old repo layout (`index.min.json` plus `repo.json`) |
| `extension_list_url` | nullable; when set, the extension list is fetched from this URL instead of from the index (`AnimeExtensionStoreService.kt:72-83`) |

Statements in `extension_store.sq` (SQL verbatim): `get`, `getAll`, `getCount` have their own entries below; the write side is:

```sql
-- extension_store.sq
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
```sql
-- extension_store.sq
delete:
DELETE FROM extension_store
WHERE index_url = :indexUrl;
```

* **`upsert`** conflict target is `index_url` and overwrites the seven other columns. `signing_key =: signingKey` has the same harmless typo as `extension_repos.upsert`; `WHERE index_url = :indexUrl` is redundant.
* **Writers**: `AnimeExtensionStoreRepositoryImpl.upsert` (`:54-67`, after a successful `service.fetch`), `insertFromPreference` (`:23-36`, one-time migration with `name = badge_label = "Repo #<n>"`, `signing_key = "NO_SIGNING_KEY"`, `contact_website = index_url`, `is_legacy = 0`; `TrustExtensionRepositoryMigration.kt:20-25` first rewrites the preference URL to `url.removeSuffix("/index.min.json").removeSuffix("/index.json") + "/repo.json"`), the backup restorer (`AnimeExtensionStoreRestorer.kt:14-23`, which defaults `badge_label` to `name` and `is_legacy` to **true** when the backup field is null), and `remove` (`delete`, `:102-104`).
* **Fetch algorithm** (`data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt:28-68`): GET the index URL with the shared OkHttp client; transparently gunzip when the first two bytes are `0x1f 0x8b` (`:111-121`); then dispatch on the first byte: `0x5B` (`[`) means a legacy `index.min.json` array (URL must end with `/index.min.json`, otherwise `IllegalArgumentException("Provided legacy store url is not valid")`; the metadata is then fetched from the same URL with `/repo.json`); `0x7B` (`{`) means JSON, first tried as `NetworkLegacyAnimeExtensionRepo` (`{"index_v2": string|null, "meta": {"name","shortName","website","signingKeyFingerprint"}}`; when `index_v2` is non-null the fetch **restarts** with that URL) and on `IllegalArgumentException` as `NetworkAnimeExtensionStore` (JSON); anything else is decoded as protobuf `NetworkAnimeExtensionStore` (field numbers: `name`=1, `badgeLabel`=2, `signingKey`=3, `contact{website=1, discord=2}`=4, `extensionList`=101, `extensionListUrl`=102; `NetworkAnimeExtensionStore.kt`). Failures become `Result.failure` and are logged, never thrown (`CancellationException` excepted).
* **Refresh** (`refreshAll`, `:38-52`): loads `getAll()`, sequentially fetches each store and upserts; a failing store is logged as `"Failed to refresh extension store '<name> (<index_url>)'"` and skipped. Triggered from `AnimeExtensionApi.kt:46` (`updateExtensionStores()` before checking for updates).
* **Callers of the rows**: `getAllAsFlow` drives the settings list (`AnimeExtensionStoresScreenModel.kt:39`); `getCountAsFlow` drives the count in settings (`SettingsBrowseScreen.kt:41`); `fetchExtensions` (`:69-88`) reads `getAll(::extensionStoreMapper)` to build the available-extension list, one `async` per store inside `supervisorScope`, concatenating successes and logging failures (`"Failed to fetch extensions for store '<name> (<indexUrl>)'"`).
* **Proposal (Rust)**: port to `anime.db` as written. Fetch/index parsing belongs to area:19-extensions-anime and area:20-extension-strategy-redesign; the table is only the persisted trust and index list. Because Linux cannot load APKs, `signing_key` keeps its meaning only if Kurobayomi verifies downloaded extension bundles by signature; otherwise store it but do not rely on it.

```rust
pub fn store_upsert(c: &Connection, s: &ExtensionStore) -> rusqlite::Result<usize> {
    c.execute(
        "INSERT INTO extension_store(index_url, name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(index_url) DO UPDATE SET name = ?2, badge_label = ?3, signing_key = ?4, contact_website = ?5, \
             contact_discord = ?6, is_legacy = ?7, extension_list_url = ?8",
        (&s.index_url, &s.name, &s.badge_label, &s.signing_key, &s.contact_website, &s.contact_discord, s.is_legacy, &s.extension_list_url),
    )
}
pub fn store_delete(c: &Connection, index_url: &str) -> rusqlite::Result<usize> {
    c.execute("DELETE FROM extension_store WHERE index_url = ?1", [index_url])
}
```

Notify tables: `extension_store`.

### `findAll`

OBSERVED. Four unrelated statements share the name; all are full-table reads, no WHERE, no ORDER BY, no parameters.

**1. `extension_repos.sq:19`**

```sql
-- extension_repos.sq
findAll:
SELECT *
FROM extension_repos;
```

Returns every repo row (`base_url, name, short_name, website, signing_key_fingerprint`), unspecified order (rowid order in practice; the PK is TEXT so SQLite scans the table b-tree, rows come back in insertion order). Mapper `mapExtensionRepo` (`MangaExtensionRepoRepositoryImpl.kt:80-92`). Callers: `subscribeAll` `:15` (reactive; UI sorts nothing, so the list shows insertion order) and `getAll` `:19`. Rust: `repo_find_all` in the `extension_repos` entry.

**2. `sources.sq:7`** (manga stub sources)

```sql
-- sources.sq
findAll:
SELECT *
FROM sources;
```

Returns `(_id, lang, name)` for every remembered manga source. Caller: `MangaStubSourceRepositoryImpl.subscribeAllManga` (`data/src/main/java/tachiyomi/data/source/manga/MangaStubSourceRepositoryImpl.kt:13`), collected by `AndroidMangaSourceManager.kt:75` which copies the rows into `stubSourcesMap` (the map `getOrStub` falls back to). Reactive: the map refreshes whenever `sources` changes (written by `registerStubSource`, `AndroidMangaSourceManager.kt:104-113` area, calling `upsertStubMangaSource` when the stored row differs from the live source).

**3. `animesources.sq:7`** (anime stub sources)

```sql
-- animesources.sq
findAll:
SELECT *
FROM animesources;
```

Same shape for anime. Caller: `AnimeStubSourceRepositoryImpl.subscribeAllAnime` (`data/src/main/java/tachiyomi/data/source/anime/AnimeStubSourceRepositoryImpl.kt:13`), collected by `AndroidAnimeSourceManager.kt:75`.

**4. `custom_buttons.sq:32`** (anime player custom buttons)

```sql
-- custom_buttons.sq
findAll:
SELECT *
FROM custom_buttons
ORDER BY sortIndex;
```

Returns all custom buttons sorted by `sortIndex` ascending; ties have no secondary key (fall back to rowid in practice). Table (`129.sqm`, anime database only):

```sql
-- custom_buttons.sq
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

| Column | Meaning |
|---|---|
| `_id` | autoincrement-like rowid; the seeded button has `_id = 1` |
| `name` | label shown on the button (the seeded one is `+85 s`); **not unique** |
| `isFavorite` | 1 for the single "primary" button shown directly on the player controls; at most one is 1 by convention (`ToggleFavoriteCustomButton.kt:27-36` rewrites every row), nothing enforces it |
| `sortIndex` | zero-based order; contiguous after create/delete/reorder |
| `content` | Lua source run when the button is tapped (mpv Lua with Aniyomi helpers) |
| `longPressContent` | Lua source run on long press |
| `onStartup` | Lua source run when the player starts |

Placeholders substituted at use time (`domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt:8-26`): the literal text `$id` is replaced by the button's `_id`, and `$isPrimary` by `true` when the button is the favorite (`primaryId == id`) else `false`, applied separately to `content`, `longPressContent` and `onStartup`.

The seed row that both `129.sqm` and `custom_buttons.sq` insert (`INSERT OR IGNORE`, so re-running is harmless):

```sql
-- custom_buttons.sq
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

Behaviour of the seeded button: tap runs `aniyomi.right_seek_by(intro_length)` where `intro_length = mp.get_property_number("user-data/current-anime/intro-length")`; long press opens `aniyomi.int_picker("Change intro length", "%ds", 0, 255, 1, "user-data/current-anime/intro-length")` (title, format, min 0, max 255, step 1, property); on startup, when `$isPrimary` is `true`, it observes the same property and hides the button when the length is 0, otherwise shows it titled `"+" .. length .. " s"`.

Callers: `CustomButtonRepositoryImpl.subscribeAll` (`data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt:16`, settings list `PlayerSettingsCustomButtonScreenModel.kt:40`) and `getAll` (`:20`, used by `CreateCustomButton.kt:17` to compute `nextSortIndex = maxOfOrNull { it.sortIndex }?.plus(1) ?: 0`, `DeleteCustomButton.kt:20` (renumbers `sortIndex = index` after a delete), `ReorderCustomButton.kt:19` (move then renumber), `ToggleFavoriteCustomButton.kt:28`, the player `PlayerViewModel.kt:331-337` which picks `buttons.firstOrNull { it.isFavorite }` as primary, `CustomButtonBackupCreator.kt:13`, `CustomButtonRestorer.kt:17-37`). Restore matches by `name`, never overwrites an existing button, assigns new `sortIndex` values after the current maximum in backup order, and only keeps `isFavorite` when the database had no favorite (`CustomButtonRestorer.kt:19-29`).

Rust (all four):

```rust
pub fn stub_sources_find_all(c: &Connection, anime: bool) -> rusqlite::Result<Vec<StubSource>> {
    let sql = if anime { "SELECT * FROM animesources" } else { "SELECT * FROM sources" }; // anime.db vs manga.db
    c.prepare_cached(sql)?.query_map([], |r| Ok(StubSource { id: r.get("_id")?, lang: r.get("lang")?, name: r.get("name")? }))?.collect()
}
pub fn custom_buttons_find_all(c: &Connection) -> rusqlite::Result<Vec<CustomButton>> {
    c.prepare_cached("SELECT * FROM custom_buttons ORDER BY sortIndex")?.query_map([], |r| Ok(CustomButton {
        id: r.get("_id")?, name: r.get("name")?, is_favorite: r.get("isFavorite")?, sort_index: r.get("sortIndex")?,
        content: r.get("content")?, long_press_content: r.get("longPressContent")?, on_startup: r.get("onStartup")?,
    }))?.collect()
}
```

Proposal notes: `ORDER BY sortIndex, _id` for a stable order. Stub sources: keep as is, the Linux source mechanism (area:20-extension-strategy-redesign) still needs a persisted `(id, lang, name)` map so library entries of removed sources keep a name. Custom buttons: **replace the Lua-in-mpv mechanism by libmpv script messages** (area:48-settings-player-and-scripts, area:45-player-core); the table keeps its columns so Aniyomi backups import and the same Lua runs under libmpv's Lua scripting. Notify tables: `sources` / `animesources` / `custom_buttons` respectively.

### `findOne`

OBSERVED. Three statements, all primary-key or unique-key point lookups returning at most one row.

**1. `extension_repos.sq:9`**

```sql
-- extension_repos.sq
findOne:
SELECT *
FROM extension_repos
WHERE base_url = :base_url;
```

Parameter `:base_url` (TEXT). Returns the full repo row or no row. Caller: `getRepo` (`MangaExtensionRepoRepositoryImpl.kt:23`), used in `CreateMangaExtensionRepo.handleInsertionError` (`CreateMangaExtensionRepo.kt:54`) to detect `RepoAlreadyExists`. Rust: `repo_find_one`.

**2. `sources.sq:11`**

```sql
-- sources.sq
findOne:
SELECT *
FROM sources
WHERE _id = :id;
```

Parameter `:id` (Long, the 64-bit source id). Returns `(_id, lang, name)` or no row. Caller: `getStubMangaSource` (`MangaStubSourceRepositoryImpl.kt:18`), used by `AndroidMangaSourceManager.kt:106` (`registerStubSource`: skip the upsert when the stored stub equals the live one, `if (dbSource == source) return@launch`; when it differs, `upsertStubMangaSource` then `downloadManager.renameSource(dbSource, source)` renames the download folder) and `:116` (`createStubSource`: use the stored row; else ask `extensionManager.getSourceData(id)`; else fabricate `StubMangaSource(id, lang = "", name = "")`).

**3. `animesources.sq:11`**

```sql
-- animesources.sq
findOne:
SELECT *
FROM animesources
WHERE _id = :id;
```

Same, anime. Callers `AnimeStubSourceRepositoryImpl.getStubAnimeSource` (`:18`), `AndroidAnimeSourceManager.kt:108` and `:118`.

Rust:

```rust
pub fn stub_source_find_one(c: &Connection, anime: bool, id: i64) -> rusqlite::Result<Option<StubSource>> {
    let sql = if anime { "SELECT * FROM animesources WHERE _id = ?1" } else { "SELECT * FROM sources WHERE _id = ?1" };
    c.query_row(sql, [id], |r| Ok(StubSource { id: r.get("_id")?, lang: r.get("lang")?, name: r.get("name")? })).optional()
}
pub fn stub_source_upsert(c: &Connection, anime: bool, s: &StubSource) -> rusqlite::Result<usize> {
    // sources.sq:16-24 / animesources.sq:16-24
    let sql = if anime {
        "INSERT INTO animesources(_id, lang, name) VALUES (?1, ?2, ?3) ON CONFLICT(_id) DO UPDATE SET lang = ?2, name = ?3"
    } else {
        "INSERT INTO sources(_id, lang, name) VALUES (?1, ?2, ?3) ON CONFLICT(_id) DO UPDATE SET lang = ?2, name = ?3"
    };
    c.execute(sql, (s.id, &s.lang, &s.name))
}
```

Notify tables: `sources` / `animesources`.

### `findOneBySigningKeyFingerprint`

OBSERVED (`data/src/main/sqldelight/data/extension_repos.sq:14`).

```sql
-- extension_repos.sq
findOneBySigningKeyFingerprint:
SELECT *
FROM extension_repos
WHERE signing_key_fingerprint = :fingerprint;
```

* Parameter `:fingerprint` (TEXT, compared byte-exact, case-sensitive). Returns the single repo that owns the fingerprint (UNIQUE guarantees at most one) or no row. Uses the automatic index `sqlite_autoindex_extension_repos_2` created by the `UNIQUE NOT NULL` column.
* Caller: `getRepoBySigningKeyFingerprint` (`MangaExtensionRepoRepositoryImpl.kt:26-30`), used by `CreateMangaExtensionRepo.kt:58` to build `DuplicateFingerprint(oldRepo, newRepo)` for the replace dialog; the restorer builds the same map in memory instead (`MangaExtensionRepoRestorer.kt:18`).
* Rust: `repo_find_one_by_signing_key_fingerprint` (in the `extension_repos` entry). Notify tables: none (point read).

### `get`

OBSERVED (`data/src/main/sqldelightanime/dataanime/extension_store.sq:14`).

```sql
-- extension_store.sq
get:
SELECT *
FROM extension_store
WHERE index_url = :indexUrl;
```

* Parameter `:indexUrl` (TEXT). Returns the full store row or nothing.
* **No caller anywhere** (searched `extension_storeQueries` in the whole tree: only `upsert`, `getAll`, `getCount`, `delete` are used).
* Proposal: port as `store_get` (one line, needed by a "store details" screen and for the add-store flow's "already exists?" check, which Aniyomi does by catching the upsert instead; note that Aniyomi's add flow therefore **silently overwrites** an existing store with the same `index_url`).

```rust
pub fn store_get(c: &Connection, index_url: &str) -> rusqlite::Result<Option<ExtensionStore>> {
    c.query_row("SELECT * FROM extension_store WHERE index_url = ?1", [index_url], store_from_row).optional()
}
```

### `getAll`

OBSERVED (`extension_store.sq:19`).

```sql
-- extension_store.sq
getAll:
SELECT *
FROM extension_store;
```

* No parameters; returns every store row (8 columns), unspecified order (insertion order in practice).
* Two mappers: the generated row type (used by `refreshAll`, `AnimeExtensionStoreRepositoryImpl.kt:40`, which only reads `store.index_url` and `store.name`) and `extensionStoreMapper` (`:106-126`, builds `AnimeExtensionStore` with nested `Contact(website, discord)`).
* Callers: `getAll()` `:91` (`GetAnimeExtensionStores.get`, used by `TrustAnimeExtension.kt:15` to build the trusted fingerprint set on every extension trust decision, and by the backup creator `AnimeExtensionStoresBackupCreator.kt`), `getAllAsFlow()` `:95` (settings list, `GetAnimeExtensionStores.kt:12`), `fetchExtensions()` `:72`, `refreshAll()` `:40`.
* Perf: tiny table.

```rust
pub fn store_get_all(c: &Connection) -> rusqlite::Result<Vec<ExtensionStore>> {
    c.prepare_cached("SELECT * FROM extension_store")?.query_map([], store_from_row)?.collect()
}
```

Notify tables: `extension_store`.

### `getAllAnime`

OBSERVED (`data/src/main/sqldelightanime/dataanime/animes.sq:101`).

```sql
-- animes.sq
getAllAnime:
SELECT *
FROM animes;
```

* Returns every `animes` row (all 31 columns), library and non-library, including season children. **No Kotlin caller** (no `getAllAnime(` in the tree; the repository exposes `getAnimeFavorites` instead).
* Perf: full scan; on a large catalogue this would materialise every row with `genre` and `memo`.
* **Decision: drop.** Nothing in Kurobayomi needs "every anime including non-library"; the user loses nothing. If a debugging export is wanted later, write it ad hoc.

### `getAllAnimeSourceAndUrl`

OBSERVED (`animes.sq:105`).

```sql
-- animes.sq
getAllAnimeSourceAndUrl:
SELECT source, url
FROM animes;
```

* Returns `(source: Long, url: String)` for every anime row, no filter, no order. Mapper: generated 2-column row `GetAllAnimeSourceAndUrl(source, url)`.
* Caller: `AnimeRestorer.sortByNew` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt:48-49`): builds `urlsBySource = groupBy({ it.source }, { it.url })`, then sorts the backup entries with `compareBy { it.url in urlsBySource[it.source].orEmpty() }.then(compareByDescending { it.lastModifiedAt })`, i.e. **entries not yet in the database restore first** (`false` sorts before `true`), newest `lastModifiedAt` first within each group. Note `it.url in <List>` is O(n) per entry (a list, not a set).
* Perf: full scan returning two narrow columns; fine at thousands of rows. Rust should build a `HashSet<(i64, String)>` so membership is O(1).
* Proposal (Rust, used by area:22-backup-restore):

```rust
pub fn get_all_anime_source_and_url(c: &Connection) -> rusqlite::Result<std::collections::HashSet<(i64, String)>> {
    c.prepare_cached("SELECT source, url FROM animes")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
}
```

### `getAllManga`

OBSERVED (`data/src/main/sqldelight/data/mangas.sq:90`).

```sql
-- mangas.sq
getAllManga:
SELECT *
FROM mangas;
```

* Returns every `mangas` row (24 columns). **No Kotlin caller.**
* **Decision: drop**, same reasoning as `getAllAnime`.

### `getAllMangaSourceAndUrl`

OBSERVED (`mangas.sq:94`).

```sql
-- mangas.sq
getAllMangaSourceAndUrl:
SELECT source, url
FROM mangas;
```

* Returns `(source, url)` for every manga row. Caller: `MangaRestorer.sortByNew` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt:46-55`), identical algorithm to the anime one: backup entries absent from the database first, then newest `lastModifiedAt` first.
* Rust:

```rust
pub fn get_all_manga_source_and_url(c: &Connection) -> rusqlite::Result<std::collections::HashSet<(i64, String)>> {
    c.prepare_cached("SELECT source, url FROM mangas")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
}
```

### `getAnimeById`

OBSERVED (`animes.sq:75`).

```sql
-- animes.sq
getAnimeById:
SELECT *
FROM animes
WHERE _id = :id;
```

* Parameter `:id` (Long). Returns exactly one row, all 31 `animes` columns in `.sq` order (the mapper `AnimeMapper.mapAnime`, `data/src/main/java/tachiyomi/data/entries/anime/AnimeMapper.kt:12-78`, names the positional parameter for `episode_flags` `chapterFlags` (`:28`); `last_update` and `next_update` NULL become 0 at `:50-51`; `is_syncing` is dropped).
* PK lookup, no joins, no filter on `favorite`/`parent_id`, so it returns library, non-library and season-child rows.
* Callers: `AnimeRepositoryImpl.getAnimeById` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:25`, `awaitOne`: **throws when the row is absent**) and `getAnimeByIdAsFlow` (`:29`, `subscribeToOne`, re-emits on any write to `animes`). Interactors: `GetAnime.kt:15` and `:23`, `GetAnimeWithEpisodesAndSeasons.kt:18` and `:27`, `SetAnimeViewerFlags.kt:14,32,42`, `SetSeenStatus.kt:54`.
* Perf: O(log n).

```rust
pub fn get_anime_by_id(c: &Connection, id: i64) -> rusqlite::Result<Anime> {
    c.query_row("SELECT * FROM animes WHERE _id = ?1", [id], anime_from_row) // Err(QueryReturnedNoRows) when absent
}
```

Notify tables: `animes`.

### `getAnimeByUrlAndSource`

OBSERVED (`animes.sq:80-85`; a comment above the query states that it should ideally never return more than one row).

```sql
-- animes.sq
getAnimeByUrlAndSource:
SELECT *
FROM animes
WHERE url = :url AND source = :source
LIMIT 1;
```

* Parameters `:url` (TEXT, exact match, case-sensitive), `:source` (Long). Returns at most one row (all columns). **No UNIQUE constraint on `(source, url)`**, so duplicates are possible; `LIMIT 1` without `ORDER BY` returns an arbitrary one (in practice the lowest rowid reachable through `animes_url_index`).
* Callers: `AnimeRepositoryImpl.getAnimeByUrlAndSourceId` (`:34`, `awaitOneOrNull`) and `getAnimeByUrlAndSourceIdAsFlow` (`:44`); interactors `GetAnimeByUrlAndSourceId.kt:10` and `NetworkToLocalAnime.kt:13`/`:31`. **`NetworkToLocalAnime.await` is the dedupe point for everything browsed from a source** (`domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt:12-28`): if no row exists, insert the source's anime and return it with the new id; if a row exists and `favorite = false`, return the stored row with `title` replaced by the source's current title (display only, not persisted until it becomes a favorite); otherwise return the stored row.
* Perf: `animes_url_index` narrows by `url`, then `source` is filtered per candidate row; fine unless many sources share a URL.
* Proposal (Rust): port; replace `animes_url_index` with `CREATE INDEX animes_source_url_index ON animes(source, url)` so the lookup is one index probe and unrelated sources never match.

```rust
pub fn get_anime_by_url_and_source(c: &Connection, url: &str, source: i64) -> rusqlite::Result<Option<Anime>> {
    c.query_row("SELECT * FROM animes WHERE url = ?1 AND source = ?2 LIMIT 1", (url, source), anime_from_row).optional()
}
```

Notify tables: `animes`.

### `getAnimeSeasonsById`

OBSERVED. Anime-only, query over the view `animeseasonsView` (`data/src/main/sqldelightanime/view/animeseasonsView.sq`; created by `131.sqm`, rewritten by `133.sqm` and finally `134.sqm`).

```sql
-- animeseasonsView.sq
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
```sql
-- animeseasonsView.sq
getAnimeSeasonsById:
SELECT *
FROM animeseasonsView
WHERE parent_id = :parentId;
```

* **Meaning**: the "seasons" of an anime are the child rows of the same table (`animes.parent_id = parent._id`), each with episode statistics. For a child with `fetch_type = 1` (Episodes) the statistics come from its own episodes (`episodestatsView`) and history (`animehistorystatsView`); for a child with `fetch_type = 0` (Seasons, a nested level) they are aggregated over **its** children through `animeseasonstatsView` (join `M._id = ASS.parent_id`).
* **Parameter**: `:parentId` (Long) matches `parent_id` of the child rows; the view's own filter `WHERE M.parent_id IS NOT NULL` drops top-level rows first.
* **Returned columns** (mapper `AnimeMapper.mapSeasonAnime`, `AnimeMapper.kt:166-248`): all 31 `animes` columns (`M.*`), then `totalCount` (episode count or child count), `seenCount` (seen episodes, or number of fully seen child seasons), `latestUpload` (max `date_upload`, 0 when none), `episodeFetchedAt` (max `date_fetch`; the Kotlin parameter is named `fetchedAt`), `lastSeen` (max `animehistory.last_seen`, 0 when none), `bookmarkCount`, `fillermarkCount`. `seenCount`, `bookmarkCount`, `fillermarkCount` arrive as `Double` (SQLDelight infers REAL from the `CASE` over sums) and are `toLong()`-ed (`:242-244`).
* **Ordering**: none in SQL. The app sorts later: `UpdateAnimeFromRemote.kt:116-117` sorts by `seasonSourceOrder`; the details screen sorts according to `animes.season_flags` (`SEASON_SORT_*`, `Anime.kt:257-290`).
* **NULL/edge cases**: a child with no episodes has no `episodestatsView` row, so `coalesce(x, 0)` yields zeros (`seenCount = totalCount = 0`, which `animeseasonstatsView` counts as *fully seen*: `coalesce(ES.total,0) = coalesce(ES.seenCount,0)`). `CASE` has no `ELSE`, so a `fetch_type` outside {0,1} would yield NULL columns and fail the non-null mapper.
* **Callers**: `AnimeRepositoryImpl.getAnimeSeasonsById` (`:161`) and `getAnimeSeasonsByIdAsFlow` (`:166`); interactors `GetAnimeSeasonsByParentId.kt:13` (swallows exceptions, returns empty list), `GetAnimeWithEpisodesAndSeasons.kt:20` and `:35`, `UpdateAnimeFromRemote.kt:116`, `SyncSeasonsWithSource.kt:44` (diff against the source's season list: seasons missing from the source get `parent_id` cleared by `removeParentIdByIds`), `AnimeLibraryUpdateJob.kt:182`.
* **Perf**: the view does a three-way left join plus a grouped subquery per call. `animeseasonstatsView` aggregates **all** seasons of **all** parents each time it is joined (SQLite flattens views but still materialises the GROUP BY), so on a library of thousands of seasons it is wasteful. `animes_parent_id` serves the filter.
* **Proposal (Rust)**: port the view text unchanged and query it with the same statement; to bound cost make the statement select from `animes` with `parent_id = ?1` first by using the view as is (SQLite pushes `parent_id` into the view's WHERE because `M.*` includes `parent_id`; the `animeseasonstatsView` join remains a full aggregate, acceptable for desktop scale). Mapping: `seenCount`, `bookmarkCount`, `fillermarkCount` read as `f64` then cast to `i64`.

```rust
pub fn get_anime_seasons_by_id(c: &Connection, parent_id: i64) -> rusqlite::Result<Vec<SeasonAnime>> {
    c.prepare_cached("SELECT * FROM animeseasonsView WHERE parent_id = ?1")?
        .query_map([parent_id], |r| Ok(SeasonAnime {
            anime: anime_from_row(r)?,
            total_count: r.get("totalCount")?,
            seen_count: r.get::<_, f64>("seenCount")? as i64,
            latest_upload: r.get("latestUpload")?,
            fetched_at: r.get("episodeFetchedAt")?,
            last_seen: r.get("lastSeen")?,
            bookmark_count: r.get::<_, f64>("bookmarkCount")? as i64,
            fillermark_count: r.get::<_, f64>("fillermarkCount")? as i64,
        }))?.collect()
}
```

Notify tables: `animes`, `episodes`, `animehistory`. Decision: port (P1, needed by the season-aware anime details screen, area:39-ui-entry-details-anime).

### `getAnimeSourceIdWithFavoriteCount`

OBSERVED (`animes.sq:114-120`).

```sql
-- animes.sq
getAnimeSourceIdWithFavoriteCount:
SELECT
source,
count(*)
FROM animes
WHERE favorite = 1
GROUP BY source;
```

* No parameters. Returns one row per source that has at least one favorite: `(source: Long, count(*): Long)`; sources with zero favorites do not appear. No ORDER BY (the caller sorts).
* Caller: `AnimeSourceRepositoryImpl.getAnimeSourcesWithFavoriteCount` (`data/src/main/java/tachiyomi/data/source/anime/AnimeSourceRepositoryImpl.kt:41`): combined with `sourceManager.sources`; each `sourceId` is resolved with `getOrStub` and `isStub = source is StubAnimeSource`. Consumer `GetAnimeSourcesWithFavoriteCount.kt:22-50` removes the local source (`LocalAnimeSource.ID`) and sorts by migration sort preferences: alphabetical (stubs first, then `name.lowercase()` collator compare) or by count (stubs first). Feeds the "Migrate" source list (area:42-ui-updates-history-stats-storage-upcoming / migration screens).
* Perf: uses the partial index `animelib_favorite_index` (`WHERE favorite = 1`) to scan favorites, then groups on `source` with a temp b-tree.

```rust
pub fn get_anime_source_id_with_favorite_count(c: &Connection) -> rusqlite::Result<Vec<(i64, i64)>> {
    c.prepare_cached("SELECT source, count(*) FROM animes WHERE favorite = 1 GROUP BY source")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
}
```

Notify tables: `animes`.

### `getAnimeTracks`

OBSERVED (`data/src/main/sqldelightanime/dataanime/anime_sync.sq:27-29`).

```sql
-- anime_sync.sq
getAnimeTracks:
SELECT *
FROM anime_sync;
```

* No parameters; returns every tracker binding row (14 columns, section 3.8) for **all** anime. Mapper `AnimeTrackMapper.mapTrack` (`data/src/main/java/tachiyomi/data/track/anime/AnimeTrackMapper.kt:6-36`): `sync_id` becomes `trackerId`, `private` becomes Boolean.
* Caller: `AnimeTrackRepositoryImpl.getAnimeTracksAsFlow` (`data/src/main/java/tachiyomi/data/track/anime/AnimeTrackRepositoryImpl.kt:24`, reactive) via `GetTracksPerAnime.kt:13` (`groupBy { it.animeId }`), consumed by `AnimeLibraryScreenModel.kt:112` to filter/sort the library by tracker (the "Tracker mean" sort and "tracked / not tracked" filters).
* Perf: full table read on every change of `anime_sync` (and every score edit); fine for thousands of rows.

```rust
pub fn get_anime_tracks(c: &Connection) -> rusqlite::Result<Vec<AnimeTrack>> {
    c.prepare_cached("SELECT * FROM anime_sync")?.query_map([], anime_track_from_row)?.collect()
}
```

Notify tables: `anime_sync`. Decision: port (P1 with tracking, area:27-tracking-core).

### `getAnimesWithFavoriteTimestamp`

OBSERVED (`animes.sq:109-112`).

```sql
-- animes.sq
getAnimesWithFavoriteTimestamp:
SELECT *
FROM animes
WHERE favorite_modified_at IS NOT NULL;
```

* Returns every anime that has ever been favorited (`favorite_modified_at` is set by trigger `update_last_favorite_at_animes` on every change of `favorite`, including un-favoriting; NULL for rows never changed). **No Kotlin caller.** It is a leftover from Mihon's SyncYomi/Google-Drive library sync which this tree does not contain (no `SyncManager`).
* **Decision: drop.** The columns `favorite_modified_at`, `last_modified_at`, `version`, `is_syncing` stay only because backup restore uses `last_modified_at` and `version` to decide which side is newer (`MangaRestorer.kt:46-54`, `:84-104`).

### `getBookmarkedChaptersByMangaId`

OBSERVED (`data/src/main/sqldelight/data/chapters.sq:73-77`).

```sql
-- chapters.sq
getBookmarkedChaptersByMangaId:
SELECT *
FROM chapters
WHERE bookmark
AND manga_id = :mangaId;
```

* Parameter `:mangaId`. Returns all bookmarked chapters (15 columns) of one manga; `WHERE bookmark` is truthiness of the integer (any non-zero). No order, no scanlator filter.
* Repository method `ChapterRepositoryImpl.getBookmarkedChaptersByMangaId` (`data/src/main/java/tachiyomi/data/items/chapter/ChapterRepositoryImpl.kt:101-108`) exists but **nothing calls it**; the bookmarked filter is applied in memory (`ChapterFilter.kt:23`, `:49`).
* Perf: `chapters_manga_id_index` then per-row test.
* **Decision: drop** (YAGNI; the in-memory filter already covers the UI). If downloads-by-bookmark needs it later, it is one line.

### `getBookmarkedEpisodesByAnimeId`

OBSERVED (`data/src/main/sqldelightanime/dataanime/episodes.sq:67-71`).

```sql
-- episodes.sq
getBookmarkedEpisodesByAnimeId:
SELECT *
FROM episodes
WHERE bookmark
AND anime_id = :animeId;
```

* Anime twin of the previous entry (20 `episodes` columns). Repository method `EpisodeRepositoryImpl.getBookmarkedEpisodesByAnimeId` (`data/src/main/java/tachiyomi/data/items/episode/EpisodeRepositoryImpl.kt:98-105`) has **no caller**; filters are in memory (`EpisodeFilter.kt:24`, `:52`).
* **Decision: drop.**

### `getCategories`

OBSERVED. The same text exists in the manga and the anime database.

```sql
-- categories.sq
getCategories:
SELECT
_id AS id,
name,
sort AS `order`,
flags,
hidden
FROM categories
ORDER BY sort;
```

* Files: `data/src/main/sqldelight/data/categories.sq:26-34` (manga) and `data/src/main/sqldelightanime/dataanime/categories.sq:26-34` (anime). No parameters.
* Returns **every** category including the system category (`_id = 0`, `sort = -1`, name `""`), columns aliased `id, name, order (from sort), flags, hidden` (note the backticked reserved word `order`). `ORDER BY sort` ascending, so the system category is always first; ties between user categories have no secondary key (rowid order in practice).
* Mapper `mapCategory(id, name, order, flags, hidden)` with `hidden = (hidden == 1L)` (`MangaCategoryRepositoryImpl.kt:106-120`, anime twin same lines).
* Callers: `getAllMangaCategories` / `getAllAnimeCategories` (`MangaCategoryRepositoryImpl.kt:19`, `AnimeCategoryRepositoryImpl.kt:19`) and the flow forms (`:27`). Interactors: `GetMangaCategories.kt:11,19`, `GetAnimeCategories.kt:12,20`, `CreateMangaCategoryWithName.kt:22`, `DeleteMangaCategory.kt:21-26`, `ReorderMangaCategory.kt:19`, and anime twins.
* **Interactor algorithms that depend on this ordering** (OBSERVED):
  * *Create* (`CreateMangaCategoryWithName.kt:21-39`): `nextOrder = categories.maxOfOrNull { it.order }?.plus(1) ?: 0` (computed over the list that includes the system category, whose order is -1, so the first user category gets order 0); `flags = sort.type.flag or sort.direction.flag` from the current library sort preference (`mangaSortingMode`); `hidden = false`; then plain `insert`. No name uniqueness check here (the dialog checks).
  * *Reorder* (`ReorderMangaCategory.kt:18-40`, guarded by a `Mutex`): loads all, **removes the system category**, moves the element to `newIndex`, rewrites `order = index` for all user categories. Result codes `Success`, `Unchanged` (category id not found), `InternalError`.
  * *Delete* (`DeleteMangaCategory.kt:17-58`): `delete` (the trigger aborts for `_id <= 0`), reload with `getCategories`, rewrite `order = index` for **all rows including the system one** (so the system category's `sort` becomes 0 and user categories 1..n; relative order is preserved), clear the default-category preference when it pointed at the deleted id (`defaultMangaCategory().delete()`), and remove the id from the string-set preferences listed at `DeleteMangaCategory.kt:38-44` (the manga list names `mangaUpdateCategories()` **twice** by mistake where the anime version at `DeleteAnimeCategory.kt:39-40` correctly lists `animeUpdateCategories()` and `animeUpdateCategoriesExclude()`, so a deleted manga category can stay in the library-update **exclude** set; also `removeExcludeCategories`, `downloadNewChapterCategories`, `downloadNewChapterCategoriesExclude`).
* Perf: tiny table; scan plus sort.
* Proposal (Rust): port. Fix the duplicate-preference bug (remove the id from both the include and the exclude set). Keep the system row seed and the delete trigger in the initial migration.

```rust
pub fn get_categories(c: &Connection) -> rusqlite::Result<Vec<Category>> {
    c.prepare_cached("SELECT _id AS id, name, sort AS `order`, flags, hidden FROM categories ORDER BY sort")?
        .query_map([], category_from_row)?.collect()
}
```

(`category_from_row` reads columns `id`, `name`, `order`, `flags`, `hidden`.) Notify tables: `categories`. Applies to both `manga.db` and `anime.db`.

### `getCategoriesByAnimeId`

OBSERVED (`data/src/main/sqldelightanime/dataanime/categories.sq:47-57`).

```sql
-- categories.sq
getCategoriesByAnimeId:
SELECT
C._id AS id,
C.name,
C.sort AS `order`,
C.flags,
C.hidden
FROM categories C
JOIN animes_categories AC
ON C._id = AC.category_id
WHERE AC.anime_id = :animeId;
```

* Parameter `:animeId`. Inner join of `categories` with `animes_categories` on `category_id`; returns the categories the anime is assigned to, same aliased 5 columns as `getCategories`. **No ORDER BY** (join order, usually `animes_categories` rowid order), hidden categories **included**. An uncategorised anime has no rows (the system category 0 is never stored in the join table; "no rows" means "Default").
* Callers: `AnimeCategoryRepositoryImpl.getCategoriesByAnimeId` (`data/src/main/java/tachiyomi/data/category/anime/AnimeCategoryRepositoryImpl.kt:36`) and the flow `:48`; `GetAnimeCategories.kt:24` (plus the `subscribe(animeId)` form), used by the details screen (`AnimeScreenModel.kt:527`), the history screen (`AnimeHistoryScreenModel.kt:175`), the library selection dialogs (`AnimeLibraryScreenModel.kt:446`, `:461`, `:569`), the browse favourite dialog (`BrowseAnimeSourceScreenModel.kt:269`), migration (`MigrateAnimeDialog.kt:307`), download category rules (`AnimeDownloadManager.kt:413`), the player's info line (`PlayerViewModel.kt:1289`) and the backup creator (`AnimeBackupCreator.kt:47`). The "Edit categories" dialog itself lists **all** categories via `getCategories` minus the system one (`AnimeScreenModel.kt:517` `filterNot { it.isSystemCategory }`) and pre-ticks the ids returned here.
* Perf: `animes_categories` has no index on `anime_id` or `category_id`, so each call scans the join table (small). Proposal adds `CREATE INDEX animes_categories_anime_id_index ON animes_categories(anime_id)` (and `mangas_categories_manga_id_index` for manga).

```rust
pub fn get_categories_by_anime_id(c: &Connection, anime_id: i64) -> rusqlite::Result<Vec<Category>> {
    c.prepare_cached(
        "SELECT C._id AS id, C.name, C.sort AS `order`, C.flags, C.hidden \
         FROM categories C JOIN animes_categories AC ON C._id = AC.category_id \
         WHERE AC.anime_id = ?1 ORDER BY C.sort")?   // ORDER BY added: Aniyomi returns unspecified order
        .query_map([anime_id], category_from_row)?.collect()
}
```

The visible-only twin `getVisibleCategoriesByAnimeId` (same SQL plus `AND C.hidden = 0`) belongs to the same family; Rust: add `AND C.hidden = 0`. Notify tables: `categories`, `animes_categories`.

### `getCategoriesByMangaId`

OBSERVED (`data/src/main/sqldelight/data/categories.sq:47-57`).

```sql
-- categories.sq
getCategoriesByMangaId:
SELECT
C._id AS id,
C.name,
C.sort AS `order`,
C.flags,
C.hidden
FROM categories C
JOIN mangas_categories MC
ON C._id = MC.category_id
WHERE MC.manga_id = :mangaId;
```

* Identical to the anime version with `mangas_categories` / `manga_id`. Callers: `MangaCategoryRepositoryImpl.getCategoriesByMangaId` (`data/src/main/java/tachiyomi/data/category/manga/MangaCategoryRepositoryImpl.kt:36`) and flow `:48`; `GetMangaCategories.kt:15` and `:23`, used by the details screen (`MangaScreenModel.kt:451`; the dialog's all-categories list is `MangaScreenModel.kt:441` with the system category filtered out), history (`MangaHistoryScreenModel.kt:180`), library selection (`MangaLibraryScreenModel.kt:432`, `:447`, `:554`), browse (`BrowseMangaSourceScreenModel.kt:278`), migration (`MigrateMangaDialog.kt:260`), downloads (`MangaDownloadManager.kt:415` category rules, `MangaDownloader.kt:658` category names), and `MangaBackupCreator.kt:52`.
* Writers of the join table: `MangaRepositoryImpl.setMangaCategories` (`data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt:91-98`): in one transaction `DELETE FROM mangas_categories WHERE manga_id = ?` then one `INSERT INTO mangas_categories(manga_id, category_id)` per id; each insert fires `insert_manga_category_update_version` (bumps `mangas.version` unless `is_syncing = 1`). `SetMangaCategories.kt:11-17` swallows and logs exceptions.

```rust
pub fn get_categories_by_manga_id(c: &Connection, manga_id: i64) -> rusqlite::Result<Vec<Category>> {
    c.prepare_cached(
        "SELECT C._id AS id, C.name, C.sort AS `order`, C.flags, C.hidden \
         FROM categories C JOIN mangas_categories MC ON C._id = MC.category_id \
         WHERE MC.manga_id = ?1 ORDER BY C.sort")?
        .query_map([manga_id], category_from_row)?.collect()
}
```

Visible-only twin `getVisibleCategoriesByMangaId` adds `AND C.hidden = 0`. Notify tables: `categories`, `mangas_categories`.

### `getCategory`

OBSERVED. Same text in both databases.

```sql
-- categories.sq
getCategory:
SELECT *
FROM categories
WHERE _id = :id
LIMIT 1;
```

* Files: `data/src/main/sqldelight/data/categories.sq:20-24` and `data/src/main/sqldelightanime/dataanime/categories.sq:20-24`. Parameter `:id`. Returns the raw columns `(_id, name, sort, flags, hidden)` (not aliased; the mapper takes them positionally as `id, name, order, flags, hidden`). `LIMIT 1` is redundant (PK).
* Callers: `getMangaCategory` / `getAnimeCategory` (`MangaCategoryRepositoryImpl.kt:15`, `AnimeCategoryRepositoryImpl.kt:15`), used **only** by `SetSortModeForMangaCategory.kt:21` and `SetSortModeForAnimeCategory.kt:21`.
* **What `flags` means** (the reason this lookup exists, `SetSortModeFor*Category.kt:17-35`): `flags` stores the library sort chosen for the category. Layout (`MangaLibrarySortMode.kt:25-51`, anime `AnimeLibrarySortMode.kt:25-52`): sort type mask `0b00111100` (0x3C), direction mask `0b01000000` (0x40), other bits unused. Type values (manga): Alphabetical `0b00000000`, LastRead `0b00000100`, LastUpdate `0b00001000`, UnreadCount `0b00001100`, TotalChapters `0b00010000`, LatestChapter `0b00010100`, ChapterFetchDate `0b00011000`, DateAdded `0b00011100`, TrackerMean `0b00100000`, Random `0b00111100`. Anime: LastSeen `0b00000100`, LastUpdate `0b00001000`, UnseenCount `0b00001100`, TotalEpisodes `0b00010000`, LatestEpisode `0b00010100`, EpisodeFetchDate `0b00011000`, DateAdded `0b00011100`, TrackerMean `0b00100000`, **AiringTime `0b00110000`**, Random `0b00111100`. Direction: Ascending `0b01000000`, Descending `0b00000000`. Unknown type bits decode to Alphabetical; a NULL/absent category decodes to the default `Alphabetical, Ascending` (`Category?.sort`, `MangaLibrarySortMode.kt:140-141`).
* **Algorithm** `SetSortModeFor*Category.await(categoryId, type, direction)`: `category = categoryId?.let(getCategory)`; `flags = (category?.flags ?: 0) + type + direction` where `Long + Flag` is `(this and other.mask.inv()) or (other.flag and other.mask)` (`domain/src/main/java/tachiyomi/domain/library/model/Flag.kt:21-27`), i.e. replace only the masked bits and keep every other bit; if `type == Random` store a new `Random.nextInt()` into `randomMangaSortSeed`; if the category exists **and** the preference `categorizedDisplaySettings` is true then `updatePartialCategory(flags)` for that category, else set the global `mangaSortingMode` preference and write the same `flags` into **every** category with `updateAllFlags`. The seeded system category has `flags = 0`, which decodes as Alphabetical + Descending.
* Rust:

```rust
pub fn get_category(c: &Connection, id: i64) -> rusqlite::Result<Option<Category>> {
    c.query_row("SELECT _id AS id, name, sort AS `order`, flags, hidden FROM categories WHERE _id = ?1", [id], category_from_row).optional()
}
pub const SORT_TYPE_MASK: i64 = 0b0011_1100;
pub const SORT_DIR_MASK: i64 = 0b0100_0000;
pub fn flags_plus(flags: i64, flag: i64, mask: i64) -> i64 { (flags & !mask) | (flag & mask) } // Long.plus(Flag)
```

Notify tables: `categories`.

### `getChapterById`

OBSERVED (`chapters.sq:51-54`).

```sql
-- chapters.sq
getChapterById:
SELECT *
FROM chapters
WHERE _id = :id;
```

* Parameter `:id`. Returns the single chapter row (15 columns) or none. Mapper `ChapterRepositoryImpl.mapChapter` (`:130-162`; `is_syncing` dropped).
* Callers: `ChapterRepositoryImpl.getChapterById` (`:111`, `awaitOneOrNull`) via `GetChapter.kt:14` (reader, details); `MangaBackupCreator.kt:69` calls the raw query `chaptersQueries.getChapterById(history.chapterId)` with `awaitOne` to turn a history row into a `(url, lastRead, readDuration)` backup entry (throws if the chapter vanished).
* Perf: PK.

```rust
pub fn get_chapter_by_id(c: &Connection, id: i64) -> rusqlite::Result<Option<Chapter>> {
    c.query_row("SELECT * FROM chapters WHERE _id = ?1", [id], chapter_from_row).optional()
}
```

Notify tables: `chapters`.

### `getChapterByUrl`

OBSERVED (`chapters.sq:79-82`).

```sql
-- chapters.sq
getChapterByUrl:
SELECT *
FROM chapters
WHERE url = :chapterUrl;
```

* Parameter `:chapterUrl` (TEXT, exact). Returns one row or none. **Not manga-scoped**: two manga from different sources can have chapters with the same relative URL, and `executeAsOneOrNull` throws `IllegalStateException` when more than one row matches.
* Only caller: `MangaRestorer.restoreHistory` (`MangaRestorer.kt:327`) to attach a backed-up history entry to a chapter by URL when no history row exists yet; if no chapter has that URL the history entry is skipped.
* Perf: **no index on `chapters.url`**, so every call is a full table scan (a restore of N history entries costs N scans).
* Proposal (Rust): do not port as is. Backup restore should resolve `(manga, chapter url)` pairs (the backup stores history per manga) with `getChapterByUrlAndMangaId`; if a global lookup is kept, add `CREATE INDEX chapters_url_index ON chapters(url)` and return the first match.

```rust
pub fn get_chapter_by_url(c: &Connection, url: &str) -> rusqlite::Result<Option<Chapter>> {
    c.query_row("SELECT * FROM chapters WHERE url = ?1 LIMIT 1", [url], chapter_from_row).optional() // LIMIT 1 avoids the multi-row failure
}
```

### `getChapterByUrlAndMangaId`

OBSERVED (`chapters.sq:84-88`).

```sql
-- chapters.sq
getChapterByUrlAndMangaId:
SELECT *
FROM chapters
WHERE url = :chapterUrl
AND manga_id = :mangaId;
```

* Parameters `:chapterUrl`, `:mangaId`. Returns at most one row (no UNIQUE on `(manga_id, url)`; duplicates would make `awaitOneOrNull` throw).
* Callers: `ChapterRepositoryImpl.getChapterByUrlAndMangaId` (`:122`); interactors `GetChapter.kt:23` (`await(url, mangaId)`) and `GetChapterByUrlAndMangaId.kt:12` (its parameter is named `sourceId` in the Kotlin but the argument is passed to the `mangaId` slot of the repository call; callers pass a manga id). Used by the reader/deep-link paths that open a chapter by URL.
* Perf: `chapters_manga_id_index` narrows to one manga's chapters, then `url` is compared; fine.

```rust
pub fn get_chapter_by_url_and_manga_id(c: &Connection, url: &str, manga_id: i64) -> rusqlite::Result<Option<Chapter>> {
    c.query_row("SELECT * FROM chapters WHERE url = ?1 AND manga_id = ?2 LIMIT 1", (url, manga_id), chapter_from_row).optional()
}
```

Notify tables: `chapters`.

### `getChaptersByMangaId`

OBSERVED (`chapters.sq:56-66`). The workhorse read of the manga side.

```sql
-- chapters.sq
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

* Parameters: `:mangaId` (Long) and `:applyScanlatorFilter` (Long 0/1; Kotlin passes `Boolean.toLong()`, `ChapterRepositoryImpl.kt:85`, `:116`; backup passes literal 0).
* Join: `chapters C LEFT JOIN excluded_scanlators ES` on same manga **and** `C.scanlator = ES.scanlator`. When the filter is off every chapter of the manga is returned; when on, a chapter is dropped iff the join found a match (`ES.scanlator IS NULL` keeps it). **NULL scanlator never matches** (`NULL = x` is NULL), so chapters without a scanlator are always kept. Matching is exact and case-sensitive on the stored (trimmed, `28.sqm`) text.
* Returns `C.*`: the 15 chapter columns in table order. **No ORDER BY**: Kotlin sorts afterwards (`ChapterFilter.kt`, `getNextUnread`, reader chapter list by `chapter_flags` sorting masks `CHAPTER_SORTING_*`, `Manga.kt:95-99`).
* Filter on is used by the details screen (`MangaScreenModel.kt:176`, `:217`), the reader (`ReaderViewModel.kt:156`) and library "next unread" (`MangaLibraryScreenModel.kt:437`). Off (default of `GetChaptersByMangaId.await`, `GetChaptersByMangaId.kt:13`) for chapter sync (`SyncChaptersWithSource.kt:72`), tracker sync (`SyncChapterProgressWithTrack.kt:30`, `AddMangaTracks.kt:34`), migration (`MigrateMangaDialog.kt:226-227`), restore (`MangaRestorer.kt:148`), and backup (`MangaBackupCreator.kt:40-45`) so excluded chapters are still synced and backed up.
* Repository: `getChapterByMangaId` (`:83-87`), flow `getChapterByMangaIdAsFlow` (`:114-118`, re-emits on writes to `chapters` or `excluded_scanlators`); interactor `GetMangaWithChapters.subscribe(id, applyScanlatorFilter)` combines it with the manga row.
* Perf: `chapters_manga_id_index` (+ `excluded_scanlators_manga_id_index`); O(chapters of that manga). Typical manga has hundreds, some thousands of chapters.

```rust
pub fn get_chapters_by_manga_id(c: &Connection, manga_id: i64, apply_scanlator_filter: bool) -> rusqlite::Result<Vec<Chapter>> {
    c.prepare_cached(
        "SELECT C.* FROM chapters C \
         LEFT JOIN excluded_scanlators ES ON C.manga_id = ES.manga_id AND C.scanlator = ES.scanlator \
         WHERE C.manga_id = ?1 AND (?2 = 0 OR ES.scanlator IS NULL)")?
        .query_map((manga_id, apply_scanlator_filter as i64), chapter_from_row)?.collect()
}
```

Notify tables: `chapters`, `excluded_scanlators`. Note: with duplicate `(manga_id, scanlator)` rows in `excluded_scanlators` the LEFT JOIN yields duplicate chapter rows when the filter is **off**; keep the table duplicate-free (see `excluded_scanlators_manga_id_index`). Proposal: add `DISTINCT` is wrong (it hides legitimately identical rows); instead guarantee uniqueness in the writer.

### `getChildrenByParentId`

OBSERVED (`animes.sq:157-160`).

```sql
-- animes.sq
getChildrenByParentId:
SELECT *
FROM animes
WHERE parent_id = :parentId;
```

* Parameter `:parentId`. Returns the direct children (all 31 columns) of a season-parent anime, any `favorite` value, no order, no stats (unlike `getAnimeSeasonsById`).
* Caller: `AnimeRepositoryImpl.getChildrenByParentId` (`:185`) via `GetAnimeSourcesWithNonLibraryAnime.getDeletableChildren` (`GetAnimeSourcesWithNonLibraryAnime.kt:16-18`), used only by the "Clear database" screen (`ClearAnimeDatabaseScreen.kt:263-284`): recursion over children of non-library parents; a non-favorite child goes into `ids` (to delete) and, when its `fetch_type == Seasons`, recurses; a **favorite** child goes into `orphaned` (its `parent_id` will be cleared so it survives and stays deletable-later).
* Perf: `animes_parent_id` index.

```rust
pub fn get_children_by_parent_id(c: &Connection, parent_id: i64) -> rusqlite::Result<Vec<Anime>> {
    c.prepare_cached("SELECT * FROM animes WHERE parent_id = ?1")?.query_map([parent_id], anime_from_row)?.collect()
}
```

Notify tables: `animes`.

### `getCount`

OBSERVED (`extension_store.sq:23-25`).

```sql
-- extension_store.sq
getCount:
SELECT COUNT(*)
FROM extension_store;
```

* Returns one row with one Long: the number of configured stores. Caller: `getCountAsFlow` (`AnimeExtensionStoreRepositoryImpl.kt:99`, `subscribeToOne`, re-emits when `extension_store` changes) via `GetAnimeExtensionStoreCountAsFlow.kt:8`, shown on the Browse settings row (`SettingsBrowseScreen.kt:41,44`). (Its manga sibling is `count` in `extension_repos.sq:23`, Kotlin `getCount()` at `MangaExtensionRepoRepositoryImpl.kt:33` returns `Flow<Int>`.)

```rust
pub fn store_get_count(c: &Connection) -> rusqlite::Result<i64> {
    c.query_row("SELECT COUNT(*) FROM extension_store", [], |r| r.get(0))
}
```

Notify tables: `extension_store`.

### `getDeletableParentAnime`

OBSERVED. Query over view `animedeletableView` (`data/src/main/sqldelightanime/view/animedeletableView.sq`, `131.sqm`).

```sql
-- animedeletableView.sq
CREATE VIEW animedeletableView AS
SELECT
    M._id,
    M.source,
    M.fetch_type
FROM animes AS M
WHERE M.favorite = 0
AND M.parent_id IS NULL;
```
```sql
-- animedeletableView.sq
getDeletableParentAnime:
SELECT *
FROM animedeletableView;
```

* No parameters. Returns `(_id, source, fetch_type)` of every **top-level** (`parent_id IS NULL`) anime that is **not in the library** (`favorite = 0`). Despite the name "parent", a plain episode anime qualifies too; it only means "root of a season tree".
* Mapper `AnimeMapper.mapDeletableAnime` (`AnimeMapper.kt:319-327`) to `DeletableAnime(animeId, sourceId, fetchType)`.
* Caller: `AnimeRepositoryImpl.getDeletableParentAnime` (`:178-182`, reactive) via `GetAnimeSourcesWithNonLibraryAnime.subscribe` (`GetAnimeSourcesWithNonLibraryAnime.kt:12-14`), used only by the "Clear database" screen (`ClearAnimeDatabaseScreen.kt:225-262`): grouped by source, each source row shows the count; deletion runs `deleteAnimesNotInLibraryByAnimeIds(ids)` (`DELETE FROM animes WHERE favorite = 0 AND _id IN :animeIds`, `animes.sq:162-164`), `removeParentIdByIds(orphaned)` and `animehistoryQueries.removeResettedHistory()` (`ClearAnimeDatabaseScreen.kt:286-297`).
* Perf: scans `animes` (no index on `favorite = 0`; the partial index covers only `favorite = 1`); an admin screen, run rarely.
* Manga twin: `getSourceIdsWithNonLibraryManga` (`mangas.sq:141-145`, `SELECT source, COUNT(*) AS manga_count FROM mangas WHERE favorite = 0 GROUP BY source`, `MangaSourceRepositoryImpl.kt:59`) and `deleteMangasNotInLibraryBySourceIds` (`ClearDatabaseScreen.kt:212`); catalogued elsewhere.

```rust
pub fn get_deletable_parent_anime(c: &Connection) -> rusqlite::Result<Vec<DeletableAnime>> {
    c.prepare_cached("SELECT _id, source, fetch_type FROM animedeletableView")?
        .query_map([], |r| Ok(DeletableAnime {
            anime_id: r.get("_id")?, source_id: r.get("source")?,
            fetch_type: FetchType::from_ordinal(r.get("fetch_type")?), // 0 Seasons, 1 Episodes, else Episodes
        }))?.collect()
}
```

Notify tables: `animes`. Decision: port with the Clear-database screen (P2, area:47-settings-screens-general advanced).

### `getDuplicateLibraryAnime`

OBSERVED (`animes.sq:128-133`).

```sql
-- animes.sq
getDuplicateLibraryAnime:
SELECT *
FROM animes
WHERE favorite = 1
AND LOWER(title) = :title
AND _id != :id;
```

* Parameters: `:title` (TEXT, **already lowercased by the caller**: `anime.title.lowercase()` at `GetDuplicateLibraryAnime.kt:11`), `:id` (Long, the anime being checked, excluded).
* Returns every favorite anime (31 columns) whose title equals the given title case-insensitively (`LOWER(title) = :title`), other than itself, **across all sources**. No order. The caller uses only the first (`getOrNull(0)`): `AnimeScreenModel.kt:412` (when adding to library from the details screen) and `BrowseAnimeSourceScreenModel.kt:293-294` (browse grid; dialog at `BrowseAnimeSourceScreen.kt:234`); a non-null result opens the "duplicate entry" dialog.
* Case folding: SQLite `LOWER()` folds ASCII only (unless the platform build includes ICU, which cannot be verified from the tree), Kotlin `lowercase()` folds Unicode; a non-ASCII upper-case title may therefore fail to match its stored copy. Rust fixes this with `lower_u`.
* Perf: partial index `animelib_favorite_index` for `favorite = 1`, then a full evaluation of `LOWER(title)` per favorite (no index on the expression): O(favorites).
* Proposal (Rust):

```rust
pub fn get_duplicate_library_anime(c: &Connection, id: i64, title: &str) -> rusqlite::Result<Vec<Anime>> {
    let lowered = title.to_lowercase(); // same as Kotlin String.lowercase()
    c.prepare_cached("SELECT * FROM animes WHERE favorite = 1 AND lower_u(title) = ?1 AND _id != ?2")?
        .query_map((lowered, id), anime_from_row)?.collect()
}
```

Notify tables: none (one-shot read).

### `getDuplicateLibraryManga`

OBSERVED (`mangas.sq:117-122`).

```sql
-- mangas.sq
getDuplicateLibraryManga:
SELECT *
FROM mangas
WHERE favorite = 1
AND LOWER(title) = :title
AND _id != :id;
```

* Identical to the anime one on `mangas`. Parameters `:title` (lowercased by `GetDuplicateLibraryManga.kt:11`), `:id`. Callers: `MangaRepositoryImpl.getDuplicateLibraryManga` (`MangaRepositoryImpl.kt:70`); UI `BrowseMangaSourceScreenModel.kt:302-303` (`getOrNull(0)`, dialog opened at `BrowseMangaSourceScreen.kt:234`) and `MangaScreenModel.kt:335` (details screen add-to-library).

```rust
pub fn get_duplicate_library_manga(c: &Connection, id: i64, title: &str) -> rusqlite::Result<Vec<Manga>> {
    let lowered = title.to_lowercase();
    c.prepare_cached("SELECT * FROM mangas WHERE favorite = 1 AND lower_u(title) = ?1 AND _id != ?2")?
        .query_map((lowered, id), manga_from_row)?.collect()
}
```

### `getEpisodeById`

OBSERVED (`episodes.sq:57-60`).

```sql
-- episodes.sq
getEpisodeById:
SELECT *
FROM episodes
WHERE _id = :id;
```

* Parameter `:id`. One row (20 columns) or none. Callers: `EpisodeRepositoryImpl.getEpisodeById` (`:108`, `awaitOneOrNull`) via `GetEpisode.kt:14` (player, details); `AnimeBackupCreator.kt:64` raw `awaitOne` to convert an `animehistory` row into a backup entry.
* Mapper `EpisodeRepositoryImpl.mapEpisode` (`:130-172`): drops `is_syncing`, keeps `memo` as `JsonObject`.

```rust
pub fn get_episode_by_id(c: &Connection, id: i64) -> rusqlite::Result<Option<Episode>> {
    c.query_row("SELECT * FROM episodes WHERE _id = ?1", [id], episode_from_row).optional()
}
```

Notify tables: `episodes`.

### `getEpisodeByUrl`

OBSERVED (`episodes.sq:74-77`).

```sql
-- episodes.sq
getEpisodeByUrl:
SELECT *
FROM episodes
WHERE url = :episodeUrl;
```

* Parameter `:episodeUrl`. Not anime-scoped; `executeAsOneOrNull` throws on more than one match. Only caller: `AnimeRestorer.restoreHistory` (`AnimeRestorer.kt:368`), attaching a backed-up history entry (matched by episode URL) to an episode when no `animehistory` row exists yet; no matching episode skips the entry.
* Perf: **no index on `episodes.url`**: full scan per call.
* Proposal: same as `getChapterByUrl`: restore by `(anime, url)` or add `CREATE INDEX episodes_url_index ON episodes(url)`.

```rust
pub fn get_episode_by_url(c: &Connection, url: &str) -> rusqlite::Result<Option<Episode>> {
    c.query_row("SELECT * FROM episodes WHERE url = ?1 LIMIT 1", [url], episode_from_row).optional()
}
```

### `getEpisodeByUrlAndAnimeId`

OBSERVED (`episodes.sq:79-83`).

```sql
-- episodes.sq
getEpisodeByUrlAndAnimeId:
SELECT *
FROM episodes
WHERE url = :episodeUrl
AND anime_id = :animeId;
```

* Parameters `:episodeUrl`, `:animeId`. At most one row expected (no UNIQUE). Callers: `EpisodeRepositoryImpl.getEpisodeByUrlAndAnimeId` (`:120-128`), `GetEpisode.kt:23`, `GetEpisodeByUrlAndAnimeId.kt:12` (parameter again named `sourceId` in Kotlin but used as the anime id).
* Perf: `episodes_anime_id_index` then URL compare.

```rust
pub fn get_episode_by_url_and_anime_id(c: &Connection, url: &str, anime_id: i64) -> rusqlite::Result<Option<Episode>> {
    c.query_row("SELECT * FROM episodes WHERE url = ?1 AND anime_id = ?2 LIMIT 1", (url, anime_id), episode_from_row).optional()
}
```

### `getEpisodesByAnimeId`

OBSERVED (`episodes.sq:62-65`).

```sql
-- episodes.sq
getEpisodesByAnimeId:
SELECT *
FROM episodes
WHERE anime_id = :animeId;
```

* Parameter `:animeId`. All episodes of one anime (20 columns), unordered; **no scanlator filter** (anime has no exclusion list). Sorting happens in Kotlin from `animes.episode_flags` (`EPISODE_SORTING_*`, `Anime.kt:239-243`).
* Callers: `EpisodeRepositoryImpl.getEpisodeByAnimeId` (`:94-96`) and the flow `getEpisodeByAnimeIdAsFlow` (`:111-118`); interactors `GetEpisodesByAnimeId.kt:14`, `GetAnimeWithEpisodesAndSeasons.kt:19`/`:31`, `SetSeenStatus.kt:67`, `UpdateAnimeFromRemote.kt:63`, plus episode sync (`SyncEpisodesWithSource.kt`), tracking and migration.
* Perf: `episodes_anime_id_index`; O(episodes).

```rust
pub fn get_episodes_by_anime_id(c: &Connection, anime_id: i64) -> rusqlite::Result<Vec<Episode>> {
    c.prepare_cached("SELECT * FROM episodes WHERE anime_id = ?1")?.query_map([anime_id], episode_from_row)?.collect()
}
```

Notify tables: `episodes`.

### `getExcludedScanlatorsByMangaId`

OBSERVED (`excluded_scanlators.sq:19-22`).

```sql
-- excluded_scanlators.sq
getExcludedScanlatorsByMangaId:
SELECT scanlator
FROM excluded_scanlators
WHERE manga_id = :mangaId;
```

* Parameter `:mangaId`. Returns the excluded scanlator strings of that manga, one per row, unordered, duplicates possible; mapper is the identity on the single `scanlator` column (generated `String`).
* Callers: `GetExcludedScanlators.await` (`app/src/main/java/eu/kanade/domain/entries/manga/interactor/GetExcludedScanlators.kt:11-16`, `toSet()`) and `subscribe` (`:18-23`, reactive, drives the chapter list); `SetExcludedScanlators.await` (`SetExcludedScanlators.kt:9-21`): inside one transaction read the current set, insert `excludedScanlators - current` one row at a time, then `remove(mangaId, current - excludedScanlators)` (`DELETE FROM excluded_scanlators WHERE manga_id = :mangaId AND scanlator IN :scanlators`); backup creator `MangaBackupCreator.kt:33-35` (field `excludedScanlators`); backup restore `MangaRestorer.kt:425-434` (insert only those not already present; no-op on empty list).
* The set of **available** scanlators offered in the dialog comes from `getScanlatorsByMangaId` (`chapters.sq:68-71`, `SELECT scanlator FROM chapters WHERE manga_id = :mangaId`) cleaned by `GetAvailableScanlators.kt:11-13` (`mapNotNull { it.ifBlank { null } }.toSet()`, i.e. NULL and blank strings are dropped).
* Perf: `excluded_scanlators_manga_id_index`.

```rust
pub fn get_excluded_scanlators_by_manga_id(c: &Connection, manga_id: i64) -> rusqlite::Result<std::collections::HashSet<String>> {
    c.prepare_cached("SELECT scanlator FROM excluded_scanlators WHERE manga_id = ?1")?
        .query_map([manga_id], |r| r.get(0))?.collect()
}
pub fn set_excluded_scanlators(tx: &rusqlite::Transaction, manga_id: i64, wanted: &std::collections::HashSet<String>) -> rusqlite::Result<()> {
    let current = get_excluded_scanlators_by_manga_id(tx, manga_id)?;
    for s in wanted.difference(&current) {
        tx.execute("INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (?1, ?2)", (manga_id, s))?;
    }
    let remove: Vec<&String> = current.difference(wanted).collect();
    tx.execute("DELETE FROM excluded_scanlators WHERE manga_id = ?1 AND scanlator IN (SELECT value FROM json_each(?2))",
               (manga_id, serde_json::to_string(&remove).unwrap()))?;
    Ok(())
}
```

Notify tables: `excluded_scanlators` (and `chapters`, because the filtered chapter list depends on it).

## 6. Dead and obsolete statements: decisions in one place

| Item | Callers | Decision | What the user loses |
|---|---|---|---|
| `getAllAnime` | none | drop | nothing |
| `getAllManga` | none | drop | nothing |
| `getAnimesWithFavoriteTimestamp` | none (library-sync leftover) | drop | nothing |
| `get` (extension_store) | none | port as `store_get` (one line) | nothing either way |
| `getBookmarkedChaptersByMangaId` | repository method only | drop | nothing (UI filters in memory) |
| `getBookmarkedEpisodesByAnimeId` | repository method only | drop | nothing |
| `getChapterByUrl` | restore only | replace by `getChapterByUrlAndMangaId` (+ optional `chapters_url_index`) | nothing |
| `getEpisodeByUrl` | restore only | replace by `getEpisodeByUrlAndAnimeId` (+ optional `episodes_url_index`) | nothing |

## 7. OBSERVED discrepancies between the `.sq` files (fresh install) and the `.sqm` chain (upgrade)

Section 2 states the Rust rule: take the `.sq` shape, add the `.sqm` defaults, use the `.sq` names.

| # | Topic | `.sq` (fresh install) | `.sqm` chain (upgraded database) |
|---|---|---|---|
| 1 | Favorite index on anime | `animelib_favorite_index` (`animes.sq:43`) | `library_favorite_index` on `animes` (`114.sqm:118`) |
| 2 | Favorite-time trigger names | `update_last_favorite_at_mangas` (`mangas.sq:36`), `update_last_favorite_at_animes` (`animes.sq:48`) | `update_last_favorited_at_mangas` (`26.sqm:23`), `update_last_favorited_at_animes` (`126.sqm:23`); the preceding `DROP TRIGGER IF EXISTS update_favorite_modified_at_*` drops a trigger that never existed |
| 3 | Anime system-category trigger | `system_category_delete_trigger` | `system_anime_category_delete_trigger` (`120.sqm`) |
| 4 | Join-table `last_modified_at` | absent in `mangas_categories.sq` / `animes_categories.sq` | column plus triggers `update_last_modified_at_mangas_categories` / `_animes_categories` added by `26.sqm` / `126.sqm` |
| 5 | `animes.season_flags`, `season_number`, `season_source_order` | `NOT NULL` without default | `DEFAULT 0`, `DEFAULT -1.0`, `DEFAULT 0` (`131.sqm`) |
| 6 | `animes.background_last_modified`, `episodes.fillermark` | `NOT NULL` without default | `DEFAULT 0` (`134.sqm`) |
| 7 | `animes.memo`, `episodes.memo` default | `'{}'` (text literal) | `x'7b7d'` (blob literal, same bytes) (`136.sqm`) |
| 8 | `mangas.next_update` | nullable, no default | `11.sqm` added `DEFAULT 0`; `14.sqm` rebuild removed it |
| 9 | Column types in rebuilt tables | `AS Boolean` and the other adapter annotations | rebuilt tables in `14.sqm`/`114.sqm` have `favorite INTEGER NOT NULL` without the adapter annotation (same storage) |

Consequence for the port: Kurobayomi never runs these migrations; it creates the `.sq` schema once, so every insert must provide the values that only the `.sqm` chain defaulted (the Rust insert helpers always bind all columns).
