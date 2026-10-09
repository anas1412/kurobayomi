# Area 04 — Anime database — data models, constants, preference keys, files, endpoints

All paths are repo-relative to `/home/blackbox/aniyomi-src`. "OBSERVED" is what the SQL/Kotlin does, "Proposal" is what Kurobayomi does. Column listings are the FRESH-install shape (the `.sq` files); the differences of an upgraded database are in `p1-topic-fresh-vs-upgraded-schema.md`. Every table, index, trigger and view below was also executed in SQLite 3.53.4 (the schema script, the 26-step migration chain on a reconstructed baseline, and the behaviours quoted in `p1-behavior.md`).

## 1. Files, versions, pragmas

| Item | Aniyomi value (OBSERVED) | Source | Kurobayomi (Proposal) |
|---|---|---|---|
| Database file | `tachiyomi.animedb` in the app's databases dir (manga one is `tachiyomi.db`) | `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:106` (`name = "tachiyomi.animedb"`) | `<data_dir>/anime.db` where `data_dir` is `Paths::data_dir()` of `kuro-platform` (Linux: `$XDG_DATA_HOME/kurobayomi`, default `~/.local/share/kurobayomi`) |
| Side files | `-wal`, `-shm` (WAL mode) | same | same names `anime.db-wal`, `anime.db-shm`; never deleted by the app while open |
| Schema version | 139: 26 migration files `113.sqm` to `138.sqm`, SQLDelight schema version = highest migration number + 1; stored in `PRAGMA user_version` by the Android driver | `data/src/main/sqldelightanime/migrations/138.sqm`, `data/build.gradle.kts:23-28` | `ANIME_SCHEMA_VERSION = 139`; Kurobayomi-only changes use 139 and upwards (file `139.sql` upgrades 139 to 140) |
| Oldest upgradable version | 113 (first script is `113.sqm`; nothing exists for versions below) | migrations dir | same limit for the legacy import (`T-04a-025`) |
| Pragmas on open | `foreign_keys = ON`, `journal_mode = WAL`, `synchronous = NORMAL` (run through `PRAGMA x = y` queries in `Callback.onOpen`) | `AppModule.kt:114-123` | the same three plus `busy_timeout = 5000` (Proposal; Android's driver relies on the framework default). `recursive_triggers` is NOT enabled |
| SQLite build | Android: `RequerySQLiteOpenHelperFactory` (a bundled recent SQLite) in release; framework SQLite in debug on API 30+ | `AppModule.kt:107-112` | `rusqlite` with `bundled` (current SQLite, no ICU) |
| Two databases | the manga and anime databases share table names (`categories`, ...) but never a connection | `AppModule.kt` | two files, two `Db` handles, no cross-database SQL |
| Adapters | `Animehistory.Adapter(last_seenAdapter = DateColumnAdapter)`, `Animes.Adapter(genreAdapter = StringListColumnAdapter, update_strategyAdapter = AnimeUpdateStrategyColumnAdapter, fetch_typeAdapter = FetchTypeColumnAdapter, memoAdapter = MemoColumnAdapter)`, `Episodes.Adapter(memoAdapter = MemoColumnAdapter)` | `AppModule.kt:142-157` | codecs in `kuro-db::anime::codec` (T-04a-005) |

## 2. Value conventions

| Convention | Rule | Source |
|---|---|---|
| Booleans | `INTEGER` 0/1; SQLDelight `AS Boolean` reads non-zero as true | `.sq` annotations |
| Timestamps stored by application code | Unix epoch MILLISECONDS (`Instant.now().toEpochMilli()`, `Date.time`) in `last_update`, `next_update`, `date_added`, `cover_last_modified`, `background_last_modified`, `date_fetch`, `date_upload`, `animehistory.last_seen`, `anime_relations.last_fetched_at`, `anime_sync.start_date`, `anime_sync.finish_date` | `app/src/main/java/eu/kanade/domain/entries/anime/interactor/UpdateAnime.kt`, `data/src/main/java/tachiyomi/data/DatabaseAdapter.kt:11-14` |
| Timestamps written by triggers | Unix epoch SECONDS (`strftime('%s','now')`): `animes.last_modified_at`, `animes.favorite_modified_at`, `episodes.last_modified_at` | `animes.sq:48-63`, `episodes.sq:32-39` |
| Progress | `episodes.last_second_seen` and `episodes.total_seconds` are whole SECONDS (INTEGER), `0` = not started / unknown | `episodes.sq:15-16` |
| `date_added` | 0 when the entry is not a favorite; `now` (ms) when favorited; reset to 0 when removed from the library | `UpdateAnime.awaitUpdateFavorite` |
| `calculate_interval` | days; `< 0` = fixed user interval of `abs(value)` days; `1..28` computed (`MAX_INTERVAL = 28`); `0` = not computed yet | `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchInterval.kt` |
| `genre` | text joined with the 2-character separator `", "`; NULL = unknown; `""` decodes to an empty list | `DatabaseAdapter.kt:16-27` |
| `status` | `SAnime` ints: `UNKNOWN 0`, `ONGOING 1`, `COMPLETED 2`, `LICENSED 3`, `PUBLISHING_FINISHED 4`, `CANCELLED 5`, `ON_HIATUS 6`, `UPCOMING 7` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-79` |
| `update_strategy` | ordinal of `AnimeUpdateStrategy`: `0 ALWAYS_UPDATE`, `1 ONLY_FETCH_ONCE`; unknown value decodes to `ALWAYS_UPDATE` | `DatabaseAdapter.kt:36-41` |
| `fetch_type` | ordinal of `FetchType`: `0 Seasons`, `1 Episodes`; unknown value decodes to `Episodes`; default 1 | `DatabaseAdapter.kt:43-48`, `FetchType.kt` |
| `memo` | BLOB with the UTF-8 text of a JSON object; empty object is the 2 bytes `{}` (= `x'7b7d'`); Kotlin writes `JsonObject.toString()` (compact), reads with `Json.decodeFromString<JsonObject>`; the fresh schema default is the TEXT literal `'{}'`, the upgraded default is the BLOB `x'7b7d'` | `DatabaseAdapter.kt:50-58`, `animes.sq:37`, `136.sqm` |
| Ids | `_id INTEGER NOT NULL PRIMARY KEY` is the rowid alias; no `AUTOINCREMENT` anywhere, so after deleting the highest row its id is reused; source ids are signed 64-bit (Kotlin `Long`) | all `.sq` files |
| Rust types | ids and epochs `i64`; numbers (`episode_number`, `season_number`, `score`, `last_episode_seen`) `f64`; strings `String`; flags `i64` | Proposal |

## 3. Tables (fresh shape)

Column order below is the physical column order and the order of `SELECT *`.

### 3.1 `animes` (`data/src/main/sqldelightanime/dataanime/animes.sq:8-41`)

| # | Column | SQL type | Null | Default | Adapter / Kotlin type | Meaning |
|---|---|---|---|---|---|---|
| 1 | `_id` | INTEGER | NOT NULL PK | none | Long | row id |
| 2 | `source` | INTEGER | NOT NULL | none | Long | source id (hash of name/lang/version in Aniyomi, see area:16-source-api-anime) |
| 3 | `url` | TEXT | NOT NULL | none | String | source-relative url; with `source` identifies the entry (no UNIQUE constraint) |
| 4 | `artist` | TEXT | NULL | NULL | String? | |
| 5 | `author` | TEXT | NULL | NULL | String? | |
| 6 | `description` | TEXT | NULL | NULL | String? | |
| 7 | `genre` | TEXT | NULL | NULL | `List<String>?` via `StringListColumnAdapter` | `", "`-joined |
| 8 | `title` | TEXT | NOT NULL | none | String | |
| 9 | `status` | INTEGER | NOT NULL | none | Long | see section 2 |
| 10 | `thumbnail_url` | TEXT | NULL | NULL | String? | cover url |
| 11 | `favorite` | INTEGER | NOT NULL | none | Boolean | in library |
| 12 | `last_update` | INTEGER | NULL | NULL | Long? | ms of last library refresh of this entry; mapper turns NULL into 0 |
| 13 | `next_update` | INTEGER | NULL | NULL | Long? | ms of next planned refresh; NULL into 0 |
| 14 | `initialized` | INTEGER | NOT NULL | none | Boolean | details fetched from the source at least once |
| 15 | `viewer` | INTEGER | NOT NULL | none | Long | player flags, section 5.3 (Kotlin name `viewerFlags`) |
| 16 | `episode_flags` | INTEGER | NOT NULL | none | Long | episode-list display flags, section 5.1 |
| 17 | `cover_last_modified` | INTEGER | NOT NULL | none | Long | cache-busting stamp for the cover image |
| 18 | `date_added` | INTEGER | NOT NULL | none | Long | ms added to library, 0 if not a favorite |
| 19 | `update_strategy` | INTEGER | NOT NULL | 0 | `AnimeUpdateStrategy` | |
| 20 | `calculate_interval` | INTEGER | NOT NULL | 0 | Long (`fetchInterval` Int in the domain model) | |
| 21 | `last_modified_at` | INTEGER | NOT NULL | 0 | Long | seconds, written by trigger `update_last_modified_at_animes`; `insert` writes literal 0 |
| 22 | `favorite_modified_at` | INTEGER | NULL | NULL | Long? | seconds, trigger `update_last_favorite_at_animes` |
| 23 | `version` | INTEGER | NOT NULL | 0 | Long | change counter, section 4 triggers |
| 24 | `is_syncing` | INTEGER | NOT NULL | 0 | Long | 1 while a restore/sync write is in flight (suppresses version bumps) |
| 25 | `fetch_type` | INTEGER | NOT NULL | 1 | `FetchType` | 0 Seasons, 1 Episodes |
| 26 | `parent_id` | INTEGER | NULL | NULL | Long? | id of the parent entry when this row is a season; no foreign key |
| 27 | `season_flags` | INTEGER | NOT NULL | none | Long | season-list display flags of THIS entry's children, section 5.2 |
| 28 | `season_number` | REAL | NOT NULL | none | Double | `-1.0` = unknown (Kotlin default) |
| 29 | `season_source_order` | INTEGER | NOT NULL | none | Long | position in the source's season list |
| 30 | `background_url` | TEXT | NULL | NULL | String? | |
| 31 | `background_last_modified` | INTEGER | NOT NULL | none | Long | |
| 32 | `memo` | BLOB | NOT NULL | `'{}'` | `JsonObject` via `MemoColumnAdapter` | opaque extension data |

The table has 32 columns.

Indexes: `animelib_favorite_index ON animes(favorite) WHERE favorite = 1`; `animes_url_index ON animes(url)`; `animes_parent_id ON animes(parent_id)`; `animes_fetch_type ON animes(fetch_type)`.
Triggers: section 4.1.

### 3.2 `episodes` (`episodes.sq:4-27`)

| # | Column | SQL type | Null | Default | Kotlin | Meaning |
|---|---|---|---|---|---|---|
| 1 | `_id` | INTEGER | NOT NULL PK | | Long | |
| 2 | `anime_id` | INTEGER | NOT NULL | | Long | FK to `animes(_id)` ON DELETE CASCADE |
| 3 | `url` | TEXT | NOT NULL | | String | |
| 4 | `name` | TEXT | NOT NULL | | String | |
| 5 | `scanlator` | TEXT | NULL | NULL | String? | |
| 6 | `seen` | INTEGER | NOT NULL | | Boolean | |
| 7 | `bookmark` | INTEGER | NOT NULL | | Boolean | |
| 8 | `last_second_seen` | INTEGER | NOT NULL | | Long | resume position, seconds |
| 9 | `total_seconds` | INTEGER | NOT NULL | | Long | duration, seconds |
| 10 | `episode_number` | REAL | NOT NULL | | Double | `-1.0` = not recognized |
| 11 | `source_order` | INTEGER | NOT NULL | | Long | |
| 12 | `date_fetch` | INTEGER | NOT NULL | | Long | ms |
| 13 | `date_upload` | INTEGER | NOT NULL | | Long | ms; `-1` when the source gives none (`Episode.create()` default) |
| 14 | `last_modified_at` | INTEGER | NOT NULL | 0 | Long | seconds, trigger |
| 15 | `version` | INTEGER | NOT NULL | 0 | Long | |
| 16 | `is_syncing` | INTEGER | NOT NULL | 0 | Long | |
| 17 | `summary` | TEXT | NULL | NULL | String? | |
| 18 | `preview_url` | TEXT | NULL | NULL | String? | |
| 19 | `fillermark` | INTEGER | NOT NULL | none (fresh) / 0 (upgraded) | Boolean | |
| 20 | `memo` | BLOB | NOT NULL | `'{}'` | `JsonObject` | |

Indexes: `episodes_anime_id_index ON episodes(anime_id)`; `episodes_unseen_by_anime_index ON episodes(anime_id, seen) WHERE seen = 0`. Triggers: section 4.2. No UNIQUE on `(anime_id, url)`.

### 3.3 `animehistory` (`animehistory.sq:3-9`)

| Column | SQL type | Null | Notes |
|---|---|---|---|
| `_id` | INTEGER | NOT NULL PK | |
| `episode_id` | INTEGER | NOT NULL UNIQUE | FK to `episodes(_id)` ON DELETE CASCADE; one row per episode |
| `last_seen` | INTEGER | NULL (fresh) / NOT NULL (upgraded) | `AS Date` via `DateColumnAdapter` (ms); `0` = reset |

Index: `animehistory_history_episode_id_index ON animehistory(episode_id)` (redundant with the UNIQUE constraint's own index).

### 3.4 `animes_categories` (`animes_categories.sq:1-9`)

| Column | SQL type | Null | Notes |
|---|---|---|---|
| `_id` | INTEGER | NOT NULL PK | |
| `anime_id` | INTEGER | NOT NULL | FK to `animes(_id)` ON DELETE CASCADE |
| `category_id` | INTEGER | NOT NULL | FK to `categories(_id)` ON DELETE CASCADE |

No UNIQUE on `(anime_id, category_id)`. Trigger: section 4.3.

### 3.5 `categories` (`categories.sq:1-8`)

| Column | SQL type | Null | Default | Notes |
|---|---|---|---|---|
| `_id` | INTEGER | NOT NULL PK | | `0` is the system (default) category |
| `name` | TEXT | NOT NULL | | `''` for the system category |
| `sort` | INTEGER | NOT NULL | | display order; system row is `-1`; Kotlin name `order` |
| `flags` | INTEGER | NOT NULL | | per-category library sort, section 5.4 |
| `hidden` | INTEGER | NOT NULL | 0 | |

Seed row: `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0)`. No UNIQUE on `name` (uniqueness is enforced in the UI layer with `MR.strings.error_category_exists` = "A category with this name already exists!").

### 3.6 `anime_sync` (`anime_sync.sq:3-21`)

| Column | SQL type | Null | Default | Kotlin | Notes |
|---|---|---|---|---|---|
| `_id` | INTEGER | NOT NULL PK | | Long | |
| `anime_id` | INTEGER | NOT NULL | | Long | FK to `animes(_id)` ON DELETE CASCADE |
| `sync_id` | INTEGER | NOT NULL | | Long | tracker id (`trackerId`) |
| `remote_id` | INTEGER | NOT NULL | | Long | id at the tracker |
| `library_id` | INTEGER | NULL | NULL | Long? | list-entry id at the tracker |
| `title` | TEXT | NOT NULL | | String | |
| `last_episode_seen` | REAL | NOT NULL | | Double | |
| `total_episodes` | INTEGER | NOT NULL | | Long | |
| `status` | INTEGER | NOT NULL | | Long | tracker-specific status code (area:27-tracking-core) |
| `score` | REAL | NOT NULL | | Double | |
| `remote_url` | TEXT | NOT NULL | | String | |
| `start_date` | INTEGER | NOT NULL | | Long | ms |
| `finish_date` | INTEGER | NOT NULL | | Long | ms |
| `private` | INTEGER | NOT NULL | 0 | Boolean | |

Constraint: `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE`.

### 3.7 `animesources` (`animesources.sq:1-5`)

`_id INTEGER NOT NULL PRIMARY KEY` (source id), `lang TEXT NOT NULL`, `name TEXT NOT NULL`.

### 3.8 `anime_relations` (`anime_relations.sq:1-12`)

`anime_id INTEGER NOT NULL`, `related_anime_id INTEGER NOT NULL`, `name TEXT NOT NULL` (group title), `sort_order INTEGER NOT NULL`, `last_fetched_at INTEGER NOT NULL` (ms); `PRIMARY KEY(anime_id, related_anime_id, name)`; both ids FK to `animes(_id)` ON DELETE CASCADE. Indexes `anime_relations_anime_id_index`, `anime_relations_related_anime_id_index`.

### 3.9 `custom_buttons` (`custom_buttons.sq:3-11`)

| Column | SQL type | Null | Notes |
|---|---|---|---|
| `_id` | INTEGER | NOT NULL PK | |
| `name` | TEXT | NOT NULL | label; the restore matches buttons by name |
| `isFavorite` | INTEGER | NOT NULL | Boolean, at most one true by application rule ("primary" button) |
| `sortIndex` | INTEGER | NOT NULL | 0-based position |
| `content` | TEXT | NOT NULL | Lua run on tap |
| `longPressContent` | TEXT | NOT NULL | Lua run on long press |
| `onStartup` | TEXT | NOT NULL | Lua run when the player starts |

Seed row `_id = 1`: `name = '+85 s'`, `isFavorite = 1`, `sortIndex = 0`; `content` = the two Lua lines `local intro_length = mp.get_property_number("user-data/current-anime/intro-length")` / `aniyomi.right_seek_by(intro_length)`; `longPressContent` = `aniyomi.int_picker("Change intro length", "%ds", 0, 255, 1, "user-data/current-anime/intro-length")`; `onStartup` = the 13-line Lua function `update_button` shown in `custom_buttons.sq:19-30` (hides the button when the length is 0, otherwise shows it with title `"+" .. length .. " s"`, observed with `mp.observe_property("user-data/current-anime/intro-length", "number", update_button)` only when `$isPrimary` is true). The Rust schema file copies these bytes verbatim, including the tab characters of the `else` branch.

### 3.10 `extension_store` (`extension_store.sq:3-12`)

| Column | SQL type | Null | Notes |
|---|---|---|---|
| `index_url` | TEXT | NOT NULL PK | |
| `name` | TEXT | NOT NULL | |
| `badge_label` | TEXT | NOT NULL | short label shown on extension rows |
| `signing_key` | TEXT | NOT NULL | fingerprint; `NO_SIGNING_KEY` for entries migrated from the old preference |
| `contact_website` | TEXT | NOT NULL | |
| `contact_discord` | TEXT | NULL | |
| `is_legacy` | INTEGER | NOT NULL | Boolean; 1 = old `repo.json` + `index.min.json` format |
| `extension_list_url` | TEXT | NULL | |

### 3.11 Object inventory (what `sqlite_master` must list after creation)

Tables (10): `anime_relations`, `anime_sync`, `animehistory`, `animes`, `animes_categories`, `animesources`, `categories`, `custom_buttons`, `episodes`, `extension_store`. Named indexes (9): `anime_relations_anime_id_index`, `anime_relations_related_anime_id_index`, `animehistory_history_episode_id_index`, `animelib_favorite_index`, `animes_fetch_type`, `animes_parent_id`, `animes_url_index`, `episodes_anime_id_index`, `episodes_unseen_by_anime_index`. Triggers (7): `insert_anime_category_update_version`, `system_category_delete_trigger`, `update_anime_version`, `update_episode_and_anime_version`, `update_last_favorite_at_animes`, `update_last_modified_at_animes`, `update_last_modified_at_episodes`. Views (8): `animedeletableView`, `animehistoryView`, `animehistorystatsView`, `animelibView`, `animeseasonsView`, `animeseasonstatsView`, `animeupdatesView`, `episodestatsView`. Automatic indexes: `sqlite_autoindex_*` for `animehistory.episode_id`, `anime_sync (anime_id, sync_id)`, `anime_relations` PK, `extension_store` PK.

## 4. Triggers (verbatim semantics; full analysis in `p1-topic-triggers-and-versioning.md`)

### 4.1 On `animes`

```sql
CREATE TRIGGER update_last_favorite_at_animes
AFTER UPDATE OF favorite ON animes
BEGIN
  UPDATE animes SET favorite_modified_at = strftime('%s', 'now') WHERE _id = new._id;
END;

CREATE TRIGGER update_last_modified_at_animes
AFTER UPDATE ON animes
FOR EACH ROW
BEGIN
  UPDATE animes SET last_modified_at = strftime('%s', 'now') WHERE _id = new._id;
END;

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

### 4.2 On `episodes`

```sql
CREATE TRIGGER update_last_modified_at_episodes
AFTER UPDATE ON episodes
FOR EACH ROW
BEGIN
  UPDATE episodes SET last_modified_at = strftime('%s', 'now') WHERE _id = new._id;
END;

CREATE TRIGGER update_episode_and_anime_version AFTER UPDATE ON episodes
WHEN new.is_syncing = 0 AND (
    new.seen != old.seen OR
    new.bookmark != old.bookmark OR
    new.last_second_seen != old.last_second_seen
)
BEGIN
    UPDATE episodes SET version = version + 1 WHERE _id = new._id;
    UPDATE animes SET version = version + 1
    WHERE _id = new.anime_id AND (SELECT is_syncing FROM animes WHERE _id = new.anime_id) = 0;
END;
```

### 4.3 On `animes_categories` and `categories`

```sql
CREATE TRIGGER insert_anime_category_update_version AFTER INSERT ON animes_categories
BEGIN
    UPDATE animes SET version = version + 1
    WHERE _id = new.anime_id AND (SELECT is_syncing FROM animes WHERE _id = new.anime_id) = 0;
END;

CREATE TRIGGER IF NOT EXISTS system_category_delete_trigger BEFORE DELETE ON categories
BEGIN SELECT CASE WHEN old._id <= 0 THEN RAISE(ABORT, "System category can't be deleted") END; END;
```

Rust text uses `'System category can''t be deleted'`.

## 5. Bit-flag layouts

### 5.1 `animes.episode_flags` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt`, companion object)

| Field | Mask | Values |
|---|---|---|
| sort direction | `EPISODE_SORT_DIR_MASK = 0x00000001` | `EPISODE_SORT_DESC = 0x0`, `EPISODE_SORT_ASC = 0x1` |
| unseen filter | `EPISODE_UNSEEN_MASK = 0x00000006` | `SHOW_ALL = 0x0`, `EPISODE_SHOW_UNSEEN = 0x2`, `EPISODE_SHOW_SEEN = 0x4` |
| downloaded filter | `EPISODE_DOWNLOADED_MASK = 0x00000018` | `EPISODE_SHOW_DOWNLOADED = 0x8`, `EPISODE_SHOW_NOT_DOWNLOADED = 0x10` |
| bookmarked filter | `EPISODE_BOOKMARKED_MASK = 0x00000060` | `EPISODE_SHOW_BOOKMARKED = 0x20`, `EPISODE_SHOW_NOT_BOOKMARKED = 0x40` |
| fillermarked filter | `EPISODE_FILLERMARKED_MASK = 0x00000180` | `EPISODE_SHOW_FILLERMARKED = 0x80`, `EPISODE_SHOW_NOT_FILLERMARKED = 0x100` |
| sorting | `EPISODE_SORTING_MASK = 0x00000600` | `EPISODE_SORTING_SOURCE = 0x0`, `EPISODE_SORTING_NUMBER = 0x200`, `EPISODE_SORTING_UPLOAD_DATE = 0x400`, `EPISODE_SORTING_ALPHABET = 0x600` |
| previews | `EPISODE_PREVIEWS_MASK = 0x00000800` | `EPISODE_SHOW_PREVIEWS = 0x0`, `EPISODE_SHOW_NOT_PREVIEWS = 0x800` |
| summaries | `EPISODE_SUMMARIES_MASK = 0x00001000` | `EPISODE_SHOW_SUMMARIES = 0x0`, `EPISODE_SHOW_NOT_SUMMARIES = 0x1000` |
| display name/number | `EPISODE_DISPLAY_MASK = 0x00100000` | `EPISODE_DISPLAY_NAME = 0x0`, `EPISODE_DISPLAY_NUMBER = 0x100000` |

Generic setter used everywhere: `flags and mask.inv() or (flag and mask)`. Bits 13 to 19 (0x2000 to 0x80000) and 21 and up are unused. Accessors: filters map to a tri-state (`TriState.ENABLED_IS` for the "show" flag, `ENABLED_NOT` for the "not" flag, `DISABLED` otherwise). Default new-entry flags come from `SetAnimeDefaultEpisodeFlags` (area:07-domain-entries-items).

### 5.2 `animes.season_flags`

| Field | Mask | Values |
|---|---|---|
| sort direction | `SEASON_SORT_DIR_MASK = 0x1` | `SEASON_SORT_DESC = 0x0`, `SEASON_SORT_ASC = 0x1` |
| downloaded filter | `SEASON_DOWNLOADED_MASK = 0x6` | `SEASON_SHOW_DOWNLOADED = 0x2`, `SEASON_SHOW_NOT_DOWNLOADED = 0x4` |
| unseen filter | `SEASON_UNSEEN_MASK = 0x18` | `SEASON_SHOW_UNSEEN = 0x8`, `SEASON_SHOW_SEEN = 0x10` |
| started filter | `SEASON_STARTED_MASK = 0x60` | `SEASON_SHOW_STARTED = 0x20`, `SEASON_SHOW_NOT_STARTED = 0x40` |
| completed filter | `SEASON_COMPLETED_MASK = 0x180` | `SEASON_SHOW_COMPLETED = 0x80`, `SEASON_SHOW_NOT_COMPLETED = 0x100` |
| bookmarked filter | `SEASON_BOOKMARKED_MASK = 0x600` | `SEASON_SHOW_BOOKMARKED = 0x200`, `SEASON_SHOW_NOT_BOOKMARKED = 0x400` |
| fillermarked filter | `SEASON_FILLERMARKED_MASK = 0x1800` | `SEASON_SHOW_FILLERMARKED = 0x800`, `SEASON_SHOW_NOT_FILLERMARKED = 0x1000` |
| sorting | `SEASON_SORT_MASK = 0xE000` | `SEASON_SORT_SOURCE = 0x0`, `SEASON_SORT_SEASON = 0x2000`, `SEASON_SORT_UPLOAD = 0x4000`, `SEASON_SORT_ALPHABET = 0x6000`, `SEASON_SORT_COUNT = 0x8000`, `SEASON_SORT_LAST_SEEN = 0xA000`, `SEASON_SORT_FETCHED = 0xC000` (0xE000 is unused) |
| grid display mode | `SEASON_GRID_DISPLAY_MODE_MASK = 0x30000`, shift 16 | `SeasonDisplayMode` 0 CompactGrid, 1 ComfortableGrid, 2 CoverOnlyGrid, 3 List (other values: Kotlin throws `IllegalArgumentException("Invalid sorting value")`) |
| grid display size | `SEASON_GRID_DISPLAY_SIZE_MASK = 0x3C0000`, shift 18 | integer 0 to 15 |
| overlay downloaded | `SEASON_OVERLAY_DOWNLOADED_MASK = 0x400000` | set = on |
| overlay unseen | `SEASON_OVERLAY_UNSEEN_MASK = 0x800000` | |
| overlay local | `SEASON_OVERLAY_LOCAL_MASK = 0x1000000` | |
| overlay language | `SEASON_OVERLAY_LANG_MASK = 0x2000000` | |
| overlay continue | `SEASON_OVERLAY_CONT_MASK = 0x4000000` | |
| display name/number | `SEASON_DISPLAY_MODE_MASK = 0x8000000` | `SEASON_DISPLAY_MODE_SOURCE = 0x0`, `SEASON_DISPLAY_MODE_NUMBER = 0x8000000` |

### 5.3 `animes.viewer` (player/airing data, Kotlin `viewerFlags`)

| Field | Mask | Decoding |
|---|---|---|
| skip-intro length (seconds) | `ANIME_INTRO_MASK = 0xFF` | `viewer and 0xFF` (0 to 255) |
| next episode to air | `ANIME_AIRING_EPISODE_MASK = 0x0FFFF00` | `(viewer and mask) / 16^2` as Long, then Int |
| next episode airing time | `ANIME_AIRING_TIME_MASK = 0x0FFFFFFFF000000` | `(viewer and mask) / 16^6` as Long |
| skip-intro disabled | `ANIME_INTRO_DISABLE_MASK = 0x100000000000000` | true when `viewer and mask == mask` |

The division is done in floating point (`hex.pow(zeros)` is a Double) and truncated; for the 32-bit time field the quotient is exact. `resetViewerFlags` sets `viewer = 0` for every row (menu action "reset reader/player settings for all entries").

### 5.4 `categories.flags` (library sort per category; `domain/src/main/java/tachiyomi/domain/library/anime/model/AnimeLibrarySortMode.kt`)

Mask for the type: `0b00111100` = `0x3C`; mask for the direction: `0b01000000` = `0x40`.

| Type | Flag | Serialized name (preference form) |
|---|---|---|
| Alphabetical | `0x00` | `ALPHABETICAL` |
| LastSeen | `0x04` | `LAST_SEEN` |
| LastUpdate | `0x08` | `LAST_ANIME_UPDATE` |
| UnseenCount | `0x0C` | `UNSEEN_COUNT` |
| TotalEpisodes | `0x10` | `TOTAL_EPISODES` |
| LatestEpisode | `0x14` | `LATEST_EPISODE` |
| EpisodeFetchDate | `0x18` | `EPISODE_FETCH_DATE` |
| DateAdded | `0x1C` | `DATE_ADDED` |
| TrackerMean | `0x20` | `TRACKER_MEAN` |
| AiringTime | `0x30` | `AIRING_TIME` |
| Random | `0x3C` | `RANDOM` |

Direction: `Ascending = 0x40`, `Descending = 0x00`. Default `AnimeLibrarySort(Alphabetical, Ascending)` = `0x40`. A type value outside the table decodes to the default type (`Type.valueOf` uses `find { flag == flag and mask } ?: default`). New categories get `flags = sort.type.flag or sort.direction.flag` from preference `animelib_sorting_mode`.

## 6. Preference keys touched by this area

| Key | Type | Default | Where used here | Source |
|---|---|---|---|---|
| `default_anime_category` | Int | `-1` | deleted (reset to default) when the default category is deleted | `LibraryPreferences.kt:153,414` |
| `animelib_update_categories` | StringSet | empty | category id removed on category delete | `LibraryPreferences.kt:159,416` |
| `animelib_update_categories_exclude` | StringSet | empty | same | `LibraryPreferences.kt:165,418` |
| `remove_exclude_anime_categories` | StringSet | empty | same | `DownloadPreferences.kt:43,77` |
| `download_new_anime_categories` | StringSet | empty | same | `DownloadPreferences.kt:55,79` |
| `download_new_anime_categories_exclude` | StringSet | empty | same | `DownloadPreferences.kt:64,81` |
| `categorized_display` | Boolean | `false` | when true a category's sort is stored in `categories.flags`; when false the sort goes to the preference and `updateAllFlags` writes every category | `LibraryPreferences.kt:70` |
| `animelib_sorting_mode` | object `"TYPE,DIRECTION"` | `ALPHABETICAL,ASCENDING` | initial `flags` of a new category; `ResetAnimeCategoryFlags` writes it to all rows | `LibraryPreferences.kt:32` |
| `library_random_anime_sort_seed` | Int | `0` | rewritten with `Random.nextInt()` when sort type Random is chosen | `LibraryPreferences.kt:97` |
| `pref_display_mode_library` | object | `LibraryDisplayMode.default` | written by `SetAnimeDisplayMode` | `LibraryPreferences.kt:18` |
| `last_used_anime_category` | Int (app-state key) | `0` | selected library tab (the key is `Preference.appStateKey("last_used_anime_category")`, which stores under the private prefix of area:02-preferences-system) | `LibraryPreferences.kt:156` |

The preference system itself is area:02-preferences-system; Kurobayomi stores these in its own settings file with the same key strings so a future import of a settings backup keeps working.

## 7. Backup-format fields that map to this database (for area:21-backup-format-and-create and area:22-backup-restore)

`BackupAnime` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnime.kt`): proto numbers `1 source`, `2 url`, `3 title`, `4 artist`, `5 author`, `6 description`, `7 genre`, `8 status`, `9 thumbnailUrl`, `13 dateAdded`, `16 episodes`, `17 categories` (list of category ORDER values, not ids), `18 tracking`, `100 favorite`, `101 episodeFlags`, `103 viewer_flags`, `104 history`, `105 updateStrategy`, `106 lastModifiedAt`, `107 favoriteModifiedAt`, `109 version`, `112 memo` (bytes), `500 backgroundUrl`, `502 parentId`, `503 id` (used only to associate seasons with parents), `504 seasonFlags`, `505 seasonNumber`, `506 seasonSourceOrder`, `507 fetchType`. `BackupEpisode`: `1 url`, `2 name`, `3 scanlator`, `4 seen`, `5 bookmark`, `6 lastSecondSeen`, `16 totalSeconds`, `7 dateFetch`, `8 dateUpload`, `9 episodeNumber` (Float), `10 sourceOrder`, `11 lastModifiedAt`, `12 version`, `13 memo`, `501 fillermark`, `502 summary`, `503 previewUrl`. `BackupCustomButtons`: `1 name`, `2 isFavorite`, `3 sortIndex`, `4 content`, `5 longPressContent`, `6 onStartup`. Memo is moved as raw bytes through `MemoColumnAdapter.encode/decode`.

## 8. Network endpoints

This area issues no network requests. The only network-facing data it stores is `extension_store`; the fetch logic (`AnimeExtensionStoreService`, `data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt`) belongs to area:19-extensions-anime and area:20-extension-strategy-redesign. Facts recorded here because they decide what the table holds:

- `GET <index_url>`; the first byte of the (optionally gzip-decompressed, magic `0x1f8b`) body selects the parser: `0x5B` (`[`) means a legacy extension list and requires the url to end with `/index.min.json` (it is rewritten to `/repo.json` and refetched), `0x7B` (`{`) tries the legacy repo JSON first and falls back to the new JSON store format, anything else is Protocol Buffers.
- The new store record has `name`, `badgeLabel`, `signingKey`, `contact{website, discord?}`, optional inline `extensionList`, optional `extensionListUrl`; a legacy repo with an `indexV2` url is replaced by a refetch of that url.
- Stored row: `index_url` = the final url fetched, `is_legacy = 0` for the new format and `1` for legacy, `signing_key` = `signingKey` (or the legacy `signingKeyFingerprint`).

## 9. Folders and files on disk

| Path | Content | Created by |
|---|---|---|
| `<data_dir>/anime.db`, `anime.db-wal`, `anime.db-shm` | the database | `kuro-db` |
| `<data_dir>/anime.db.bak-v<N>` | copy taken before a legacy upgrade, max 3 kept | T-04a-031 |
| `<data_dir>/anime.db.corrupt-<unix seconds>` | quarantined file after a failed `quick_check` | T-04a-031 |

Android-only artefacts that are not carried: SQLDelight generated Kotlin classes (`tachiyomi.mi.data.AnimeDatabase`, `AnimeDatabaseImpl`), the `.db` schema snapshot directory named by `schemaOutputDirectory`, `Database Inspector` support.
