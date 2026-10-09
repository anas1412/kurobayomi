# Area 05 — data models, schema as seen by the repositories, preference keys, files (part 1 of 1)

All paths repo-relative to `/home/blackbox/aniyomi-src`. "OBSERVED" is what the Kotlin and SQL do. "Proposal:" is what the Rust port should do. Schema creation and migrations belong to area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations; this file quotes the tables only as far as the repositories in this area depend on them. No network endpoints are touched by this area (it is a pure local-database layer).

## 1. Database files, driver configuration

OBSERVED (app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:78-125):

| Item | Manga database | Anime database |
|---|---|---|
| SQLite file name (Android `databases/` directory) | `tachiyomi.db` | `tachiyomi.animedb` |
| SQLDelight schema class | `Database` (package `tachiyomi.data`) | `AnimeDatabase` (package `tachiyomi.mi.data`) |
| Pragmas run in `onOpen` (each through `PRAGMA <x>` + `moveToFirst`) | `foreign_keys = ON`, `journal_mode = WAL`, `synchronous = NORMAL` | same three |
| Adapters registered | `History.last_read` = `DateColumnAdapter`; `Mangas.genre` = `StringListColumnAdapter`; `Mangas.update_strategy` = `MangaUpdateStrategyColumnAdapter` | `Animehistory.last_seen` = `DateColumnAdapter`; `Animes.genre` = `StringListColumnAdapter`; `Animes.update_strategy` = `AnimeUpdateStrategyColumnAdapter`; `Animes.fetch_type` = `FetchTypeColumnAdapter`; `Animes.memo` = `MemoColumnAdapter`; `Episodes.memo` = `MemoColumnAdapter` |
| Query dispatcher | `Dispatchers.IO` (also the transaction dispatcher) | same |

The two databases are separate SQLite files. The `categories` table exists in both with the same shape and the same seed row; manga categories and anime categories are therefore independent lists (the same category name can exist in both, ids are unrelated).

Proposal: keep two database files under the `Paths` trait data directory (`kuro-platform`, requirement rule 4 in plans/addenda/requirements-cross-platform.md): `tachiyomi.db` and `tachiyomi.animedb` (same names so a user-supplied Aniyomi database can be dropped in for a one-off import, and so the schema documents of area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations apply unchanged). Open each with `PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;` on every connection (writer and readers). Leave `PRAGMA recursive_triggers` at its SQLite default (OFF): the triggers in section 5 update the same table they fire on and rely on non-recursion.

## 2. Row shapes (domain models used by the repositories)

Types: `Long` = signed 64-bit (Rust `i64`), `Int` = signed 32-bit (`i32`), `Double` = `f64`, `String?` = nullable text.

### 2.1 Manga (domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt)

| Field | Type | Nullable | `Manga.create()` default | Table column |
|---|---|---|---|---|
| id | Long | no | -1 | `mangas._id` INTEGER PRIMARY KEY |
| source | Long | no | -1 | `source` INTEGER NOT NULL |
| favorite | Boolean | no | false | `favorite` INTEGER NOT NULL (0/1) |
| lastUpdate | Long | no (maps NULL to 0) | 0 | `last_update` INTEGER (nullable column) |
| nextUpdate | Long | no (maps NULL to 0) | 0 | `next_update` INTEGER (nullable column) |
| fetchInterval | Int | no | 0 | `calculate_interval` INTEGER NOT NULL DEFAULT 0 (stored Long, narrowed with `toInt()`) |
| dateAdded | Long | no | 0 | `date_added` INTEGER NOT NULL |
| viewerFlags | Long | no | 0 | `viewer` INTEGER NOT NULL |
| chapterFlags | Long | no | 0 | `chapter_flags` INTEGER NOT NULL |
| coverLastModified | Long | no | 0 | `cover_last_modified` INTEGER NOT NULL |
| url | String | no | "" | `url` TEXT NOT NULL |
| title | String | no | "" | `title` TEXT NOT NULL |
| artist | String? | yes | null | `artist` TEXT |
| author | String? | yes | null | `author` TEXT |
| description | String? | yes | null | `description` TEXT |
| genre | List<String>? | yes | null | `genre` TEXT (StringListColumnAdapter) |
| status | Long | no | 0 | `status` INTEGER NOT NULL |
| thumbnailUrl | String? | yes | null | `thumbnail_url` TEXT |
| updateStrategy | UpdateStrategy | no | ALWAYS_UPDATE | `update_strategy` INTEGER NOT NULL DEFAULT 0 |
| initialized | Boolean | no | false | `initialized` INTEGER NOT NULL |
| lastModifiedAt | Long | no | 0 | `last_modified_at` INTEGER NOT NULL DEFAULT 0 (epoch SECONDS, see section 5) |
| favoriteModifiedAt | Long? | yes | null | `favorite_modified_at` INTEGER (epoch SECONDS) |
| version | Long | no | 0 | `version` INTEGER NOT NULL DEFAULT 0 |

`is_syncing` (INTEGER NOT NULL DEFAULT 0) exists in the table and in the mapper signature (`@Suppress("UNUSED_PARAMETER") isSyncing`) but is not a domain field.

`MangaUpdate` (domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaUpdate.kt): `id: Long` plus every field above except `lastModifiedAt` and `favoriteModifiedAt` as nullable with default `null`: source, favorite, lastUpdate, nextUpdate, fetchInterval, dateAdded, viewerFlags, chapterFlags, coverLastModified, url, title, artist, author, description, genre, status, thumbnailUrl, updateStrategy, initialized, version. `Manga.toMangaUpdate()` copies all of those fields (including nullable ones as non-null values when present).

`LibraryManga` (domain/src/main/java/tachiyomi/domain/library/manga/LibraryManga.kt): `manga: Manga`, `category: Long`, `totalChapters: Long`, `readCount: Long`, `bookmarkCount: Long`, `latestUpload: Long`, `chapterFetchedAt: Long`, `lastRead: Long`; derived `id = manga.id`, `unreadCount = totalChapters - readCount`, `hasBookmarks = bookmarkCount > 0`, `hasStarted = readCount > 0` (evaluated once at construction).

### 2.2 Anime (domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt)

Same 23 fields as Manga with `chapterFlags` renamed `episodeFlags` (column `episode_flags`), plus:

| Field | Type | Nullable | `Anime.create()` default | Table column |
|---|---|---|---|---|
| backgroundLastModified | Long | no | 0 | `background_last_modified` INTEGER NOT NULL |
| backgroundUrl | String? | yes | null | `background_url` TEXT |
| fetchType | FetchType | no | `FetchType.Episodes` | `fetch_type` INTEGER NOT NULL DEFAULT 1 (FetchTypeColumnAdapter) |
| parentId | Long? | yes | null | `parent_id` INTEGER (no FOREIGN KEY clause in the CREATE TABLE) |
| seasonFlags | Long | no | 0 | `season_flags` INTEGER NOT NULL (no default in DDL) |
| seasonNumber | Double | no | -1.0 | `season_number` REAL NOT NULL (no default in DDL) |
| seasonSourceOrder | Long | no | 0 | `season_source_order` INTEGER NOT NULL (no default in DDL) |
| memo | JsonObject | no | `JsonObject.EMPTY` (`{}`) | `memo` BLOB NOT NULL DEFAULT '{}' (MemoColumnAdapter) |

`AnimeUpdate` (domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeUpdate.kt): `id: Long` plus every field above except `lastModifiedAt`, `favoriteModifiedAt` as nullable with default `null` (source, favorite, lastUpdate, nextUpdate, fetchInterval, dateAdded, viewerFlags, episodeFlags, coverLastModified, backgroundLastModified, url, title, artist, author, description, genre, status, thumbnailUrl, backgroundUrl, updateStrategy, initialized, version, fetchType, parentId, seasonFlags, seasonNumber, seasonSourceOrder, memo).

`LibraryAnime` (domain/src/main/java/tachiyomi/domain/library/anime/LibraryAnime.kt): `anime`, `category: Long`, `totalCount`, `seenCount`, `bookmarkCount`, `fillermarkCount`, `latestUpload`, `episodeFetchedAt`, `lastSeen` (all Long); derived `id`, `unseenCount = totalCount - seenCount`, `hasBookmarks = bookmarkCount > 0`, `hasStarted = seenCount > 0`.

`SeasonAnime` (domain/src/main/java/aniyomi/domain/anime/SeasonAnime.kt): `anime`, `totalCount`, `seenCount`, `bookmarkCount`, `fillermarkCount`, `latestUpload`, `fetchedAt`, `lastSeen`; derived `id`, `seen = totalCount == seenCount`, `unseenCount = totalCount - seenCount`, `hasStarted = seenCount > 0`, `hasBookmarks`, `hasFillermarks`; `toLibraryAnime()` sets `category = -1L` and maps `fetchedAt` to `episodeFetchedAt`.

`DeletableAnime` (domain/src/main/java/tachiyomi/domain/source/anime/model/DeletableAnime.kt): `animeId: Long`, `sourceId: Long`, `fetchType: FetchType`.

`AnimeRelationGroup` (domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeRelationGroup.kt): `name: String`, `anime: List<Anime>`.

### 2.3 Chapter and Episode

`Chapter` (domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt) with `Chapter.create()` defaults:

| Field | Type | Nullable | create() default | Column |
|---|---|---|---|---|
| id | Long | no | -1 | `_id` |
| mangaId | Long | no | -1 | `manga_id` |
| read | Boolean | no | false | `read` |
| bookmark | Boolean | no | false | `bookmark` |
| lastPageRead | Long | no | 0 | `last_page_read` |
| dateFetch | Long | no | 0 | `date_fetch` |
| sourceOrder | Long | no | 0 | `source_order` |
| url | String | no | "" | `url` |
| name | String | no | "" | `name` |
| dateUpload | Long | no | -1 | `date_upload` |
| chapterNumber | Double | no | -1.0 | `chapter_number` REAL |
| scanlator | String? | yes | null | `scanlator` |
| lastModifiedAt | Long | no | 0 | `last_modified_at` (epoch seconds) |
| version | Long | no | 1 | `version` (note: create() uses 1, column default is 0) |

Derived: `isRecognizedNumber = chapterNumber >= 0f`. `copyFrom(other)` copies name, url, dateUpload, chapterNumber and `scanlator?.ifBlank { null }`.

`ChapterUpdate`: `id` plus nullable (default null): mangaId, read, bookmark, lastPageRead, dateFetch, sourceOrder, url, name, dateUpload, chapterNumber, scanlator, version. `Chapter.toChapterUpdate()` passes positionally (id, mangaId, read, bookmark, lastPageRead, dateFetch, sourceOrder, url, name, dateUpload, chapterNumber, scanlator, version).

`Episode` (domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt) fields and `Episode.create()` defaults: id -1, animeId -1, seen false, bookmark false, fillermark false, lastSecondSeen 0, totalSeconds 0, dateFetch 0, sourceOrder 0, url "", name "", dateUpload -1, episodeNumber -1.0, scanlator null, summary null, previewUrl null, lastModifiedAt 0, version 1, memo `JsonObject.EMPTY`. Columns: `_id`, `anime_id`, `seen`, `bookmark`, `fillermark`, `last_second_seen`, `total_seconds`, `date_fetch`, `source_order`, `url`, `name`, `date_upload`, `episode_number` REAL, `scanlator`, `summary`, `preview_url`, `last_modified_at`, `version`, `memo` BLOB. Derived `isRecognizedNumber = episodeNumber >= 0f`. `copyFrom(other)` copies name, url, dateUpload, episodeNumber, fillermark, and `ifBlank { null }` of scanlator, summary and previewUrl.

`EpisodeUpdate`: `id` plus nullable: animeId, seen, bookmark, fillermark, lastSecondSeen, totalSeconds, dateFetch, sourceOrder, url, name, dateUpload, episodeNumber, scanlator, summary, previewUrl, version, memo.

### 2.4 Category (domain/src/main/java/tachiyomi/domain/category/model/Category.kt, CategoryUpdate.kt)

`Category(id: Long, name: String, order: Long, flags: Long, hidden: Boolean)`; `isSystemCategory = (id == UNCATEGORIZED_ID)`; `UNCATEGORIZED_ID = 0L`. Columns: `_id` (id), `name`, `sort` (order, aliased `sort AS \`order\`` in SELECTs), `flags`, `hidden` (INTEGER NOT NULL DEFAULT 0, mapped `hidden == 1L`).

`CategoryUpdate(id, name: String? = null, order: Long? = null, flags: Long? = null, hidden: Boolean? = null)`.

## 3. Enums and their stored values

| Enum | Source | Stored as | Values (ordinal = stored value) | Decode fallback |
|---|---|---|---|---|
| `UpdateStrategy` (manga) | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt | INTEGER ordinal | 0 `ALWAYS_UPDATE`, 1 `ONLY_FETCH_ONCE` | any other value decodes to `ALWAYS_UPDATE` |
| `AnimeUpdateStrategy` | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt | INTEGER ordinal | 0 `ALWAYS_UPDATE`, 1 `ONLY_FETCH_ONCE` | `ALWAYS_UPDATE` |
| `FetchType` | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt | INTEGER ordinal | 0 `Seasons`, 1 `Episodes` | `Episodes` (note the column DEFAULT is 1) |
| `SeasonDisplayMode` (grid mode inside `season_flags`) | domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt | 2 bits of `season_flags` | 0 CompactGrid, 1 ComfortableGrid, 2 CoverOnlyGrid, 3 List; any other value throws `IllegalArgumentException("Invalid sorting value")` | throws |
| Entry status (`status` column) | source-api/.../source/model/SManga.kt:48-54 and animesource/model/SAnime.kt:72-79 | INTEGER | manga 0 UNKNOWN, 1 ONGOING, 2 COMPLETED, 3 LICENSED, 4 PUBLISHING_FINISHED, 5 CANCELLED, 6 ON_HIATUS; anime adds 7 UPCOMING | none: stored verbatim |
| Upcoming statuses | domain/src/main/java/mihon/domain/upcoming/manga/interactor/GetUpcomingManga.kt, anime/interactor/GetUpcomingAnime.kt | `status IN :statuses` | manga {1, 4}; anime {1, 4, 7} | n/a |

Proposal: Rust `#[repr(i64)]`-free enums with explicit `fn from_i64(v) -> Self` (fallbacks above) and `fn to_i64(self)`; never use `as` casts on a `#[derive]`d enum order (an inserted variant would silently renumber stored data). Unit-test every discriminant.

## 4. Value codecs (DatabaseAdapter.kt)

| Adapter | DB type | Encode | Decode |
|---|---|---|---|
| `DateColumnAdapter` | INTEGER | `value.time` (epoch milliseconds) | `Date(databaseValue)` |
| `StringListColumnAdapter` (separator `LIST_OF_STRINGS_SEPARATOR = ", "`, comma followed by one space) | TEXT | `value.joinToString(separator = ", ")` (empty list encodes to `""`) | `""` decodes to the empty list; otherwise `split(", ")` (Kotlin `String.split(String)` keeps empty segments, so `"a, , b"` yields `["a", "", "b"]` and a trailing `", "` yields a trailing empty string) |
| `MangaUpdateStrategyColumnAdapter` / `AnimeUpdateStrategyColumnAdapter` | INTEGER | `ordinal.toLong()` | `entries.getOrElse(value.toInt()) { ALWAYS_UPDATE }` (a negative or too-large value falls back) |
| `FetchTypeColumnAdapter` | INTEGER | `ordinal.toLong()` | `entries.getOrElse(value.toInt()) { Episodes }` |
| `MemoColumnAdapter` | BLOB | `value.toString().encodeToByteArray()` (kotlinx.serialization compact JSON, UTF-8) | `Json.decodeFromString<JsonObject>(bytes.decodeToString())` (throws on invalid JSON or non-object) |

Genre note: a genre that itself contains `", "` is split into two genres on the next read (lossy by construction; also reachable through source data). Both Kotlin and the Rust port share this limitation; Proposal: preserve the codec exactly because backups and the migrations in area:11-app-migrations assume it.

`toInt()` narrowing: `fetchInterval = calculateInterval.toInt()` and `.toInt()` in decoders use two's-complement truncation; Rust must use `i32::try_from(v).unwrap_or(...)` or `v as i32` deliberately (Proposal: `as i32`, identical to Kotlin).

## 5. SQL statements executed by the in-scope repositories

Everything below is quoted from the `.sq` files. `:name` are named bind parameters. Parameter order for generated `insert`/`update` functions in SQLDelight follows first appearance in the statement text; the Kotlin code calls them with named arguments, so the Rust port binds by name.

### 5.1 Manga database

`mangas` DDL (data/src/main/sqldelight/data/mangas.sq): columns in order `_id, source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, chapter_flags, cover_last_modified, date_added, update_strategy (DEFAULT 0), calculate_interval (DEFAULT 0 NOT NULL), last_modified_at (DEFAULT 0), favorite_modified_at, version (DEFAULT 0), is_syncing (DEFAULT 0)`. Indexes: `library_favorite_index ON mangas(favorite) WHERE favorite = 1`, `mangas_url_index ON mangas(url)`.

Statements used by `MangaRepositoryImpl`:

```sql
getMangaById:            SELECT * FROM mangas WHERE _id = :id;
getMangaByUrlAndSource:  SELECT * FROM mangas WHERE url = :url AND source = :source LIMIT 1;
getFavorites:            SELECT * FROM mangas WHERE favorite = 1;
getReadMangaNotInLibrary:
  SELECT * FROM mangas WHERE favorite = 0 AND _id IN (
      SELECT DISTINCT chapters.manga_id FROM chapters WHERE read = 1 OR last_page_read != 0);
getFavoriteBySourceId:   SELECT * FROM mangas WHERE favorite = 1 AND source = :sourceId;
getDuplicateLibraryManga:
  SELECT * FROM mangas WHERE favorite = 1 AND LOWER(title) = :title AND _id != :id;
getUpcomingManga:
  SELECT * FROM mangas WHERE next_update >= :startOfDay AND favorite = 1
  AND status IN :statuses ORDER BY next_update ASC;
resetViewerFlags:        UPDATE mangas SET viewer = 0;
insert: INSERT INTO mangas(source, url, artist, author, description, genre, title, status, thumbnail_url,
  favorite, last_update, next_update, initialized, viewer, chapter_flags, cover_last_modified, date_added,
  update_strategy, calculate_interval, last_modified_at, version)
  VALUES (:source, :url, :artist, :author, :description, :genre, :title, :status, :thumbnailUrl, :favorite,
  :lastUpdate, :nextUpdate, :initialized, :viewerFlags, :chapterFlags, :coverLastModified, :dateAdded,
  :updateStrategy, :calculateInterval, 0, :version);
update: UPDATE mangas SET source = coalesce(:source, source), url = coalesce(:url, url),
  artist = coalesce(:artist, artist), author = coalesce(:author, author),
  description = coalesce(:description, description), genre = coalesce(:genre, genre),
  title = coalesce(:title, title), status = coalesce(:status, status),
  thumbnail_url = coalesce(:thumbnailUrl, thumbnail_url), favorite = coalesce(:favorite, favorite),
  last_update = coalesce(:lastUpdate, last_update), next_update = coalesce(:nextUpdate, next_update),
  initialized = coalesce(:initialized, initialized), viewer = coalesce(:viewer, viewer),
  chapter_flags = coalesce(:chapterFlags, chapter_flags),
  cover_last_modified = coalesce(:coverLastModified, cover_last_modified),
  date_added = coalesce(:dateAdded, date_added),
  update_strategy = coalesce(:updateStrategy, update_strategy),
  calculate_interval = coalesce(:calculateInterval, calculate_interval),
  version = coalesce(:version, version), is_syncing = coalesce(:isSyncing, is_syncing)
  WHERE _id = :mangaId;
selectLastInsertedRowId: SELECT last_insert_rowid();
```

Note the `insert` hard-codes `last_modified_at = 0` and never writes `favorite_modified_at` (stays NULL) or `is_syncing` (stays 0).

`libraryView` (data/src/main/sqldelight/view/libraryView.sq) and the query `library: SELECT * FROM libraryView;`:

```sql
CREATE VIEW libraryView AS
SELECT M.*,
    coalesce(C.total, 0) AS totalCount, coalesce(C.readCount, 0) AS readCount,
    coalesce(C.latestUpload, 0) AS latestUpload, coalesce(C.fetchedAt, 0) AS chapterFetchedAt,
    coalesce(C.lastRead, 0) AS lastRead, coalesce(C.bookmarkCount, 0) AS bookmarkCount,
    coalesce(MC.category_id, 0) AS category
FROM mangas M
LEFT JOIN( SELECT chapters.manga_id, count(*) AS total, sum(read) AS readCount,
        coalesce(max(chapters.date_upload), 0) AS latestUpload,
        coalesce(max(history.last_read), 0) AS lastRead,
        coalesce(max(chapters.date_fetch), 0) AS fetchedAt,
        sum(chapters.bookmark) AS bookmarkCount
    FROM chapters
    LEFT JOIN excluded_scanlators ON chapters.manga_id = excluded_scanlators.manga_id
        AND chapters.scanlator = excluded_scanlators.scanlator
    LEFT JOIN history ON chapters._id = history.chapter_id
    WHERE excluded_scanlators.scanlator IS NULL
    GROUP BY chapters.manga_id ) AS C ON M._id = C.manga_id
LEFT JOIN mangas_categories AS MC ON MC.manga_id = M._id
WHERE M.favorite = 1;
```

Result columns after `M.*` (24 columns): `totalCount, readCount, latestUpload, chapterFetchedAt, lastRead, bookmarkCount, category` (7 columns), matching `MangaMapper.mapLibraryManga`.

`chapters` DDL (data/src/main/sqldelight/data/chapters.sq): columns `_id, manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload, last_modified_at (DEFAULT 0), version (DEFAULT 0), is_syncing (DEFAULT 0)`; `FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE`; indexes `chapters_manga_id_index ON chapters(manga_id)` and `chapters_unread_by_manga_index ON chapters(manga_id, read) WHERE read = 0`.

```sql
getChapterById:  SELECT * FROM chapters WHERE _id = :id;
getChaptersByMangaId:
  SELECT C.* FROM chapters C
  LEFT JOIN excluded_scanlators ES ON C.manga_id = ES.manga_id AND C.scanlator = ES.scanlator
  WHERE C.manga_id = :mangaId AND ( :applyScanlatorFilter = 0 OR ES.scanlator IS NULL );
getScanlatorsByMangaId:  SELECT scanlator FROM chapters WHERE manga_id = :mangaId;
getBookmarkedChaptersByMangaId: SELECT * FROM chapters WHERE bookmark AND manga_id = :mangaId;
getChapterByUrlAndMangaId: SELECT * FROM chapters WHERE url = :chapterUrl AND manga_id = :mangaId;
removeChaptersWithIds: DELETE FROM chapters WHERE _id IN :chapterIds;
insert: INSERT INTO chapters(manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number,
  source_order, date_fetch, date_upload, last_modified_at, version, is_syncing)
  VALUES (:mangaId, :url, :name, :scanlator, :read, :bookmark, :lastPageRead, :chapterNumber, :sourceOrder,
  :dateFetch, :dateUpload, 0, :version, 0);
update: UPDATE chapters SET manga_id = coalesce(:mangaId, manga_id), url = coalesce(:url, url),
  name = coalesce(:name, name), scanlator = coalesce(:scanlator, scanlator), read = coalesce(:read, read),
  bookmark = coalesce(:bookmark, bookmark), last_page_read = coalesce(:lastPageRead, last_page_read),
  chapter_number = coalesce(:chapterNumber, chapter_number), source_order = coalesce(:sourceOrder, source_order),
  date_fetch = coalesce(:dateFetch, date_fetch), date_upload = coalesce(:dateUpload, date_upload),
  version = coalesce(:version, version), is_syncing = coalesce(:isSyncing, is_syncing)
  WHERE _id = :chapterId;
```

`excluded_scanlators` (manga_id INTEGER NOT NULL, scanlator TEXT NOT NULL, FK to mangas ON DELETE CASCADE, index `excluded_scanlators_manga_id_index`). Not in the unique key: duplicates of `(manga_id, scanlator)` are possible (the table has no PRIMARY KEY or UNIQUE) and would multiply rows in the `LEFT JOIN` of `getChaptersByMangaId` (filter off) — see `p1-edge-cases-and-tests.md` EC-12.

`categories` and `mangas_categories` (data/src/main/sqldelight/data/categories.sq, mangas_categories.sq):

```sql
CREATE TABLE categories( _id INTEGER NOT NULL PRIMARY KEY, name TEXT NOT NULL, sort INTEGER NOT NULL,
    flags INTEGER NOT NULL, hidden INTEGER NOT NULL DEFAULT 0 );
INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0);
CREATE TRIGGER IF NOT EXISTS system_category_delete_trigger BEFORE DELETE ON categories
BEGIN SELECT CASE WHEN old._id <= 0 THEN RAISE(ABORT, "System category can't be deleted") END; END;
getCategory:       SELECT * FROM categories WHERE _id = :id LIMIT 1;
getCategories:     SELECT _id AS id, name, sort AS `order`, flags, hidden FROM categories ORDER BY sort;
getVisibleCategories: same SELECT with WHERE hidden = 0 ORDER BY sort;
getCategoriesByMangaId:
  SELECT C._id AS id, C.name, C.sort AS `order`, C.flags, C.hidden
  FROM categories C JOIN mangas_categories MC ON C._id = MC.category_id WHERE MC.manga_id = :mangaId;
getVisibleCategoriesByMangaId: same with AND C.hidden = 0;
insert:  INSERT INTO categories(name, sort, flags) VALUES (:name, :order, :flags);
delete:  DELETE FROM categories WHERE _id = :categoryId;
update:  UPDATE categories SET name = coalesce(:name, name), sort = coalesce(:order, sort),
         flags = coalesce(:flags, flags), hidden = coalesce(:hidden, hidden) WHERE _id = :categoryId;
updateAllFlags: UPDATE categories SET flags = coalesce(?, flags);
CREATE TABLE mangas_categories( _id INTEGER NOT NULL PRIMARY KEY, manga_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL,
    FOREIGN KEY(category_id) REFERENCES categories (_id) ON DELETE CASCADE,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE );
insert: INSERT INTO mangas_categories(manga_id, category_id) VALUES (:mangaId, :categoryId);
deleteMangaCategoryByMangaId: DELETE FROM mangas_categories WHERE manga_id = :mangaId;
```

`getCategory` is `SELECT *` (columns `_id, name, sort, flags, hidden`) and is mapped through `mapCategory(id, name, order, flags, hidden)` positionally, so the column aliases do not matter there. The `*ByMangaId` queries have no `ORDER BY` (result order follows the join plan, in practice the `mangas_categories` index/rowid order); callers that need category order must sort by `Category.order` themselves.

### 5.2 Anime database

`animes` DDL (data/src/main/sqldelightanime/dataanime/animes.sq), columns in order: `_id, source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, episode_flags, cover_last_modified, date_added, update_strategy (DEFAULT 0), calculate_interval (DEFAULT 0 NOT NULL), last_modified_at (DEFAULT 0), favorite_modified_at, version (DEFAULT 0), is_syncing (DEFAULT 0), fetch_type (DEFAULT 1), parent_id, season_flags, season_number, season_source_order, background_url, background_last_modified, memo (BLOB DEFAULT '{}')` (32 columns). Indexes: `animelib_favorite_index ON animes(favorite) WHERE favorite = 1`, `animes_url_index ON animes(url)`, `animes_parent_id ON animes(parent_id)`, `animes_fetch_type ON animes(fetch_type)`.

Statements used by `AnimeRepositoryImpl`: identical to the manga ones with `animes`, `animeId`, `episodes`/`seen`/`last_second_seen` substitutions (`getAnimeById`, `getAnimeByUrlAndSource` with `LIMIT 1`, `getFavorites`, `getWatchedAnimeNotInLibrary` using `seen = 1 OR last_second_seen != 0`, `getFavoriteBySourceId`, `getDuplicateLibraryAnime` with `LOWER(title) = :title`, `getUpcomingAnime`, `resetViewerFlags`), and the extra ones:

```sql
removeParentIdByIds:    UPDATE animes SET parent_id = NULL WHERE _id IN :animeIds;
getChildrenByParentId:  SELECT * FROM animes WHERE parent_id = :parentId;
getAnimeSeasonsById:    SELECT * FROM animeseasonsView WHERE parent_id = :parentId;
getDeletableParentAnime: SELECT * FROM animedeletableView;
animelib:               SELECT * FROM animelibView;
insert: INSERT INTO animes(source, url, artist, author, description, genre, title, status, thumbnail_url,
  favorite, last_update, next_update, initialized, viewer, episode_flags, cover_last_modified, date_added,
  update_strategy, calculate_interval, last_modified_at, version, fetch_type, parent_id, season_flags,
  season_number, season_source_order, background_url, background_last_modified, memo)
  VALUES (:source, :url, :artist, :author, :description, :genre, :title, :status, :thumbnailUrl, :favorite,
  :lastUpdate, :nextUpdate, :initialized, :viewerFlags, :episodeFlags, :coverLastModified, :dateAdded,
  :updateStrategy, :calculateInterval, 0, :version, :fetchType, :parentId, :seasonFlags, :seasonNumber,
  :seasonSourceOrder, :backgroundUrl, :backgroundLastModified, :memo);
update: UPDATE animes SET source = coalesce(:source, source), url = coalesce(:url, url),
  artist = coalesce(:artist, artist), author = coalesce(:author, author),
  description = coalesce(:description, description), genre = coalesce(:genre, genre),
  title = coalesce(:title, title), status = coalesce(:status, status),
  thumbnail_url = coalesce(:thumbnailUrl, thumbnail_url), favorite = coalesce(:favorite, favorite),
  last_update = coalesce(:lastUpdate, last_update), next_update = coalesce(:nextUpdate, next_update),
  initialized = coalesce(:initialized, initialized), viewer = coalesce(:viewer, viewer),
  episode_flags = coalesce(:episodeFlags, episode_flags),
  cover_last_modified = coalesce(:coverLastModified, cover_last_modified),
  date_added = coalesce(:dateAdded, date_added), update_strategy = coalesce(:updateStrategy, update_strategy),
  calculate_interval = coalesce(:calculateInterval, calculate_interval), version = coalesce(:version, version),
  is_syncing = coalesce(:isSyncing, is_syncing), fetch_type = coalesce(:fetchType, fetch_type),
  parent_id = coalesce(:parentId, parent_id), season_flags = coalesce(:seasonFlags, season_flags),
  season_number = coalesce(:seasonNumber, season_number),
  season_source_order = coalesce(:seasonSourceOrder, season_source_order),
  background_url = coalesce(:backgroundUrl, background_url),
  background_last_modified = coalesce(:backgroundLastModified, background_last_modified),
  memo = coalesce(:memo, memo) WHERE _id = :animeId;
```

Views (full text in `p1-topic-library-and-season-views.md`): `animelibView`, `animeseasonsView`, `animedeletableView`, plus the helper views `episodestatsView`, `animehistorystatsView`, `animeseasonstatsView`.

`episodes` DDL (data/src/main/sqldelightanime/dataanime/episodes.sq): `_id, anime_id, url, name, scanlator, seen, bookmark, last_second_seen, total_seconds, episode_number, source_order, date_fetch, date_upload, last_modified_at (DEFAULT 0), version (DEFAULT 0), is_syncing (DEFAULT 0), summary, preview_url, fillermark, memo (BLOB DEFAULT '{}')`; FK `anime_id` to `animes(_id)` ON DELETE CASCADE; indexes `episodes_anime_id_index ON episodes(anime_id)`, `episodes_unseen_by_anime_index ON episodes(anime_id, seen) WHERE seen = 0`.

```sql
getEpisodeById:               SELECT * FROM episodes WHERE _id = :id;
getEpisodesByAnimeId:         SELECT * FROM episodes WHERE anime_id = :animeId;
getBookmarkedEpisodesByAnimeId: SELECT * FROM episodes WHERE bookmark AND anime_id = :animeId;
getEpisodeByUrlAndAnimeId:    SELECT * FROM episodes WHERE url = :episodeUrl AND anime_id = :animeId;
removeEpisodesWithIds:        DELETE FROM episodes WHERE _id IN :episodeIds;
insert: INSERT INTO episodes(anime_id, url, name, scanlator, seen, bookmark, last_second_seen, total_seconds,
  episode_number, source_order, date_fetch, date_upload, last_modified_at, version, is_syncing, summary,
  preview_url, fillermark, memo)
  VALUES (:animeId, :url, :name, :scanlator, :seen, :bookmark, :lastSecondSeen, :totalSeconds, :episodeNumber,
  :sourceOrder, :dateFetch, :dateUpload, 0, :version, 0, :summary, :previewUrl, :fillermark, :memo);
update: UPDATE episodes SET anime_id = coalesce(:animeId, anime_id), url = coalesce(:url, url),
  name = coalesce(:name, name), scanlator = coalesce(:scanlator, scanlator), seen = coalesce(:seen, seen),
  bookmark = coalesce(:bookmark, bookmark), last_second_seen = coalesce(:lastSecondSeen, last_second_seen),
  total_seconds = coalesce(:totalSeconds, total_seconds), episode_number = coalesce(:episodeNumber, episode_number),
  source_order = coalesce(:sourceOrder, source_order), date_fetch = coalesce(:dateFetch, date_fetch),
  date_upload = coalesce(:dateUpload, date_upload), version = coalesce(:version, version),
  is_syncing = coalesce(:isSyncing, is_syncing), summary = coalesce(:summary, summary),
  preview_url = coalesce(:previewUrl, preview_url), fillermark = coalesce(:fillermark, fillermark),
  memo = coalesce(:memo, memo) WHERE _id = :episodeId;
```

Anime `categories` is identical to the manga one; `getCategoriesByAnimeId` joins `animes_categories AC ON C._id = AC.category_id WHERE AC.anime_id = :animeId`; `animes_categories(_id, anime_id, category_id)` has the same two cascading foreign keys; `insert: INSERT INTO animes_categories(anime_id, category_id) VALUES (:animeId, :categoryId)`, `deleteAnimeCategoryByAnimeId: DELETE FROM animes_categories WHERE anime_id = :animeId`.

`anime_relations` (data/src/main/sqldelightanime/dataanime/anime_relations.sq):

```sql
CREATE TABLE anime_relations( anime_id INTEGER NOT NULL, related_anime_id INTEGER NOT NULL,
    name TEXT NOT NULL, sort_order INTEGER NOT NULL, last_fetched_at INTEGER NOT NULL,
    PRIMARY KEY(anime_id, related_anime_id, name),
    FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE,
    FOREIGN KEY(related_anime_id) REFERENCES animes (_id) ON DELETE CASCADE );
CREATE INDEX anime_relations_anime_id_index ON anime_relations(anime_id);
CREATE INDEX anime_relations_related_anime_id_index ON anime_relations(related_anime_id);
getRelatedAnimeByAnimeId: SELECT A.*, R.name FROM anime_relations AS R JOIN animes AS A
    ON A._id = R.related_anime_id WHERE R.anime_id = :animeId ORDER BY R.sort_order ASC;
getLastFetchedAt: SELECT last_fetched_at FROM anime_relations WHERE anime_id = :animeId LIMIT 1;
insert: INSERT OR REPLACE INTO anime_relations(anime_id, related_anime_id, name, sort_order, last_fetched_at)
        VALUES (:animeId, :relatedAnimeId, :name, :sortOrder, :lastFetchedAt);
deleteByAnimeId: DELETE FROM anime_relations WHERE anime_id = :animeId;
```

## 6. Triggers that affect every repository write

OBSERVED (animes.sq, mangas.sq, chapters.sq, episodes.sq, mangas_categories.sq, animes_categories.sq). Full semantics and traps in `p1-topic-sql-semantics-and-triggers.md`.

| Trigger | Fires | Effect |
|---|---|---|
| `update_last_favorite_at_mangas` / `_animes` | `AFTER UPDATE OF favorite` (any UPDATE whose SET list names `favorite`, whether or not the value changes) | `favorite_modified_at = strftime('%s', 'now')` (epoch seconds) |
| `update_last_modified_at_mangas` / `_animes` / `_chapters` / `_episodes` | `AFTER UPDATE` for each row | `last_modified_at = strftime('%s', 'now')` (epoch seconds) |
| `update_manga_version` / `update_anime_version` | `AFTER UPDATE` | `version = version + 1` when `new.is_syncing = 0` and (`new.url != old.url` or `new.description != old.description` or `new.favorite != old.favorite`) |
| `update_chapter_and_manga_version` | `AFTER UPDATE ON chapters WHEN new.is_syncing = 0 AND (new.read != old.read OR new.bookmark != old.bookmark OR new.last_page_read != old.last_page_read)` | chapter `version + 1`, and parent `mangas.version + 1` when the parent has `is_syncing = 0` |
| `update_episode_and_anime_version` | same on `episodes` with `seen`, `bookmark`, `last_second_seen` | episode `version + 1`, parent `animes.version + 1` when the parent has `is_syncing = 0` |
| `insert_manga_category_update_version` / `insert_anime_category_update_version` | `AFTER INSERT ON mangas_categories / animes_categories` | parent `version + 1` when parent `is_syncing = 0` |
| `system_category_delete_trigger` | `BEFORE DELETE ON categories` | `RAISE(ABORT, "System category can't be deleted")` when `old._id <= 0` |

## 7. Preference keys and constants touched by this area's callers

The repositories themselves read no preference. The category flows in `p1-topic-categories.md` use these (owned by area:02-preferences-system, listed here because the repositories' contracts depend on them):

| Key | Type | Default | Where defined |
|---|---|---|---|
| `library_sorting_mode` | object (serialised `TYPE,DIRECTION`) | `MangaLibrarySort.default` = Alphabetical + Ascending, i.e. `"ALPHABETICAL,ASCENDING"` | domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:25 |
| `animelib_sorting_mode` | object | `AnimeLibrarySort.default` | LibraryPreferences.kt:32 |
| `default_category` (`DEFAULT_MANGA_CATEGORY_PREF_KEY`) | Int | -1 | LibraryPreferences.kt:154, 413 |
| `default_anime_category` (`DEFAULT_ANIME_CATEGORY_PREF_KEY`) | Int | -1 | LibraryPreferences.kt:153, 414 |
| `library_update_categories`, `library_update_categories_exclude` (manga); `animelib_update_categories`, `animelib_update_categories_exclude` (anime) | string set of category ids as decimal strings | empty set | LibraryPreferences.kt:415-418 |
| `remove_exclude_categories`, `download_new_categories`, `download_new_categories_exclude` (manga); `remove_exclude_anime_categories`, `download_new_anime_categories`, `download_new_anime_categories_exclude` (anime) | string set | empty | domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:76-81 (area:09-domain-source-track-download-release-misc) |
| `Preference.appStateKey("last_used_category")` / `("last_used_anime_category")` | Int | 0 | LibraryPreferences.kt:156-157 |

Sort flag layout in `categories.flags` (domain/src/main/java/tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt, anime/model/AnimeLibrarySortMode.kt): bits 2-5 type (`mask 0b00111100`), bit 6 direction (`mask 0b01000000`); types manga: Alphabetical 0b000000, LastRead 0b000100, LastUpdate 0b001000, UnreadCount 0b001100, TotalChapters 0b010000, LatestChapter 0b010100, ChapterFetchDate 0b011000, DateAdded 0b011100, TrackerMean 0b100000, Random 0b111100; anime: same plus AiringTime 0b110000 and the names LastSeen, UnseenCount, TotalEpisodes, LatestEpisode, EpisodeFetchDate; Direction Ascending 0b01000000, Descending 0b00000000. `Long.plus(Flag)` = `(this and other.mask.inv()) or (other.flag and other.mask)` for masked flags. Details in `p1-topic-categories.md`.

## 8. Entry-level flag layouts (stored in `episode_flags`, `chapter_flags`, `season_flags`, `viewer`)

The repositories store these as opaque Longs, but the Rust model and the library queries must agree with the masks (domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt companion; entries/anime/model/Anime.kt companion):

Manga `chapter_flags`: sort direction bit 0 (`CHAPTER_SORT_DESC=0x0`, `CHAPTER_SORT_ASC=0x1`, mask 0x1); unread filter mask 0x6 (`SHOW_UNREAD=0x2`, `SHOW_READ=0x4`); downloaded mask 0x18 (`0x8`, `0x10`); bookmarked mask 0x60 (`0x20`, `0x40`); sorting mask 0x300 (`SOURCE=0x000`, `NUMBER=0x100`, `UPLOAD_DATE=0x200`, `ALPHABET=0x300`); display mask 0x100000 (`NAME=0x0`, `NUMBER=0x100000`).

Anime `episode_flags`: sort direction bit 0 (DESC 0x0, ASC 0x1); unseen mask 0x6 (`SHOW_UNSEEN=0x2`, `SHOW_SEEN=0x4`); downloaded mask 0x18 (`0x8`, `0x10`); bookmarked mask 0x60 (`0x20`, `0x40`); fillermarked mask 0x180 (`0x80`, `0x100`); sorting mask 0x600 (`SOURCE=0x000`, `NUMBER=0x200`, `UPLOAD_DATE=0x400`, `ALPHABET=0x600`); previews mask 0x800 (`SHOW=0x0`, `NOT=0x800`); summaries mask 0x1000 (`SHOW=0x0`, `NOT=0x1000`); display mask 0x100000.

Anime `season_flags`: sort direction bit 0; downloaded mask 0x6 (`0x2`, `0x4`); unseen mask 0x18 (`0x8`, `0x10`); started mask 0x60 (`0x20`, `0x40`); completed mask 0x180 (`0x80`, `0x100`); bookmarked mask 0x600 (`0x200`, `0x400`); fillermarked mask 0x1800 (`0x800`, `0x1000`); sort mask 0xE000 (`SOURCE=0x0000`, `SEASON=0x2000`, `UPLOAD=0x4000`, `ALPHABET=0x6000`, `COUNT=0x8000`, `LAST_SEEN=0xA000`, `FETCHED=0xC000`); grid display mode bits 16-17 (mask 0x30000, offset 16); grid size bits 18-21 (mask 0x3C0000, offset 18); overlays downloaded 0x400000, unseen 0x800000, local 0x1000000, lang 0x2000000, continue 0x4000000; display mode mask 0x8000000 (`SOURCE=0x0`, `NUMBER=0x8000000`).

Anime `viewer` (viewerFlags) packs per-anime player data: skip-intro length bits 0-7 (`ANIME_INTRO_MASK=0xFF`), next-airing episode number bits 8-23 (`ANIME_AIRING_EPISODE_MASK=0xFFFF00`, divided by 16^2 = 256), airing time bits 24-55 (`ANIME_AIRING_TIME_MASK=0x0FFFFFFFF000000`, divided by 16^6 = 16777216), skip-intro-disable bit 56 (`ANIME_INTRO_DISABLE_MASK=0x100000000000000`). Manga `viewer` stores reader flags (reading mode, orientation; owned by area:43-reader-core); `resetViewerFlags` zeroes the column for every row of the table.

## 9. Files and folders on disk

This area touches only the two SQLite files and their WAL sidecars (`tachiyomi.db`, `tachiyomi.db-wal`, `tachiyomi.db-shm`, `tachiyomi.animedb`, `tachiyomi.animedb-wal`, `tachiyomi.animedb-shm`). No other files are read or written.
