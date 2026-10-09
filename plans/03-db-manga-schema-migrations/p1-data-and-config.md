# Area 03 / part 1 — Data models, enums, bit layouts, preferences, files, endpoints

Paths are repo-relative to `/home/blackbox/aniyomi-src`. Column types are SQLite storage classes; "Kotlin" is the type after SQLDelight's adapters/mappers; "Rust" is the Proposal for `kuro-db::models` (full signatures in `p1-rust-design.md`).

## 1. Tables, column by column

### 1.1 `mangas` (`data/src/main/sqldelight/data/mangas.sq:6-31`)

| # | Column | SQLite type | Null | Default | Kotlin (domain `Manga`) | Rust | Notes |
|---|---|---|---|---|---|---|---|
| 1 | `_id` | INTEGER PK | NOT NULL | rowid | `id: Long` | `i64` | no AUTOINCREMENT |
| 2 | `source` | INTEGER | NOT NULL | - | `source: Long` | `i64` | |
| 3 | `url` | TEXT | NOT NULL | - | `url: String` | `String` | index `mangas_url_index` |
| 4 | `artist` | TEXT | NULL | NULL | `artist: String?` | `Option<String>` | |
| 5 | `author` | TEXT | NULL | NULL | `author: String?` | `Option<String>` | |
| 6 | `description` | TEXT | NULL | NULL | `description: String?` | `Option<String>` | |
| 7 | `genre` | TEXT (adapter `List<String>`) | NULL | NULL | `genre: List<String>?` | `Option<GenreList>` | codec in 3.1 |
| 8 | `title` | TEXT | NOT NULL | - | `title: String` | `String` | |
| 9 | `status` | INTEGER | NOT NULL | - | `status: Long` | `i64` (wrapper `MangaStatus`) | enum 3.2 |
| 10 | `thumbnail_url` | TEXT | NULL | NULL | `thumbnailUrl: String?` | `Option<String>` | |
| 11 | `favorite` | INTEGER (Boolean) | NOT NULL | - | `favorite: Boolean` | `bool` | stored 0/1; SQL tests use `= 1` |
| 12 | `last_update` | INTEGER | NULL | NULL | `lastUpdate: Long` (`?: 0`) | `i64` (None -> 0) | ms |
| 13 | `next_update` | INTEGER | NULL | NULL | `nextUpdate: Long` (`?: 0`) | `i64` (None -> 0) | ms |
| 14 | `initialized` | INTEGER (Boolean) | NOT NULL | - | `initialized: Boolean` | `bool` | |
| 15 | `viewer` | INTEGER | NOT NULL | - | `viewerFlags: Long` | `i64` | layout 4.3 |
| 16 | `chapter_flags` | INTEGER | NOT NULL | - | `chapterFlags: Long` | `i64` | layout 4.1 |
| 17 | `cover_last_modified` | INTEGER | NOT NULL | - | `coverLastModified: Long` | `i64` | ms |
| 18 | `date_added` | INTEGER | NOT NULL | - | `dateAdded: Long` | `i64` | ms; 0 when not in library |
| 19 | `update_strategy` | INTEGER (adapter `UpdateStrategy`) | NOT NULL | 0 | `updateStrategy` | `UpdateStrategy` | enum 3.3 |
| 20 | `calculate_interval` | INTEGER | NOT NULL | 0 | `fetchInterval: Int` (`toInt()`) | `i32` | days |
| 21 | `last_modified_at` | INTEGER | NOT NULL | 0 | `lastModifiedAt: Long` | `i64` | SECONDS (trigger) |
| 22 | `favorite_modified_at` | INTEGER | NULL | NULL | `favoriteModifiedAt: Long?` | `Option<i64>` | SECONDS (trigger) |
| 23 | `version` | INTEGER | NOT NULL | 0 | `version: Long` | `i64` | |
| 24 | `is_syncing` | INTEGER | NOT NULL | 0 | dropped by mapper | `bool` | |

Indexes: `library_favorite_index ON mangas(favorite) WHERE favorite = 1`, `mangas_url_index ON mangas(url)`. Domain defaults for a brand-new in-memory entry (`Manga.create()` in `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt`): `id = -1`, `url = ""`, `title = ""`, `source = -1`, `favorite = false`, `lastUpdate = 0`, `nextUpdate = 0`, `fetchInterval = 0`, `dateAdded = 0`, `viewerFlags = 0`, `chapterFlags = 0`, `coverLastModified = 0`, `status = 0`, `updateStrategy = ALWAYS_UPDATE`, `initialized = false`, `lastModifiedAt = 0`, `favoriteModifiedAt = null`, `version = 0`.

`MangaUpdate` (`domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaUpdate.kt`) is the partial-update model: `id` plus nullable `source, favorite, lastUpdate, nextUpdate, fetchInterval, dateAdded, viewerFlags, chapterFlags, coverLastModified, url, title, artist, author, description, genre, status, thumbnailUrl, updateStrategy, initialized, version`; null = unchanged (coalesce).

### 1.2 `chapters` (`chapters.sq:3-21`)

| # | Column | Type | Null | Default | Kotlin (`Chapter`) | Rust | Notes |
|---|---|---|---|---|---|---|---|
| 1 | `_id` | INTEGER PK | NOT NULL | rowid | `id: Long` | `i64` | |
| 2 | `manga_id` | INTEGER FK -> mangas(_id) CASCADE | NOT NULL | - | `mangaId` | `i64` | index `chapters_manga_id_index` |
| 3 | `url` | TEXT | NOT NULL | - | `url` | `String` | |
| 4 | `name` | TEXT | NOT NULL | - | `name` | `String` | |
| 5 | `scanlator` | TEXT | NULL | NULL | `scanlator: String?` | `Option<String>` | |
| 6 | `read` | INTEGER (Boolean) | NOT NULL | - | `read` | `bool` | partial index `chapters_unread_by_manga_index (manga_id, read) WHERE read = 0` |
| 7 | `bookmark` | INTEGER (Boolean) | NOT NULL | - | `bookmark` | `bool` | |
| 8 | `last_page_read` | INTEGER | NOT NULL | - | `lastPageRead: Long` | `i64` | 0-based |
| 9 | `chapter_number` | REAL | NOT NULL | - | `chapterNumber: Double` | `f64` | `-1.0` unknown; `isRecognizedNumber = chapterNumber >= 0` |
| 10 | `source_order` | INTEGER | NOT NULL | - | `sourceOrder: Long` | `i64` | |
| 11 | `date_fetch` | INTEGER | NOT NULL | - | `dateFetch` | `i64` | ms |
| 12 | `date_upload` | INTEGER | NOT NULL | - | `dateUpload` | `i64` | ms |
| 13 | `last_modified_at` | INTEGER | NOT NULL | 0 | `lastModifiedAt` | `i64` | SECONDS |
| 14 | `version` | INTEGER | NOT NULL | 0 | `version` | `i64` | |
| 15 | `is_syncing` | INTEGER | NOT NULL | 0 | dropped | `bool` | |

In-memory defaults (`Chapter.create()`): `id = -1`, `mangaId = -1`, `read = false`, `bookmark = false`, `lastPageRead = 0`, `dateFetch = 0`, `sourceOrder = 0`, `url = ""`, `name = ""`, `dateUpload = -1`, `chapterNumber = -1.0`, `scanlator = null`, `lastModifiedAt = 0`, `version = 1` (note: `version = 1` here versus the column default 0; inserts write `chapter.version`). `Chapter.copyFrom(other)` copies `name, url, dateUpload, chapterNumber` and `scanlator = other.scanlator?.ifBlank { null }`.

`ChapterUpdate`: `id` plus nullable `mangaId, read, bookmark, lastPageRead, dateFetch, sourceOrder, url, name, dateUpload, chapterNumber, scanlator, version`.

### 1.3 `categories` (`categories.sq:1-7`)

| Column | Type | Null | Default | Kotlin (`Category`) | Rust |
|---|---|---|---|---|---|
| `_id` | INTEGER PK | NOT NULL | rowid | `id: Long` (system = `UNCATEGORIZED_ID = 0L`) | `i64` |
| `name` | TEXT | NOT NULL | - | `name: String` | `String` |
| `sort` | INTEGER | NOT NULL | - | `order: Long` | `i64` |
| `flags` | INTEGER | NOT NULL | - | `flags: Long` | `i64` |
| `hidden` | INTEGER | NOT NULL | 0 | `hidden: Boolean` (`== 1L`) | `bool` |

Seed row: `(0, '', -1, 0, 0)`. `Category.isSystemCategory = id == 0`. `CategoryUpdate(id, name?, order?, flags?, hidden?)`.

### 1.4 `mangas_categories` (`mangas_categories.sq:1-9`)

`_id INTEGER PK`, `manga_id INTEGER NOT NULL FK mangas CASCADE`, `category_id INTEGER NOT NULL FK categories CASCADE`. No unique constraint, no extra index. Upgraded databases also contain `last_modified_at` (ignored).

### 1.5 `history` (`history.sq:3-10`)

| Column | Type | Null | Kotlin (`MangaHistory`) | Rust | Notes |
|---|---|---|---|---|---|
| `_id` | INTEGER PK | NOT NULL | `id` | `i64` | |
| `chapter_id` | INTEGER UNIQUE FK -> chapters CASCADE | NOT NULL | `chapterId` | `i64` | |
| `last_read` | INTEGER (adapter `Date`) | `.sq`: NULL allowed; baseline: NOT NULL | `readAt: Date?` | `i64` ms (0 = reset) | `DateColumnAdapter`: `Date(ms)` |
| `time_read` | INTEGER | NOT NULL | `readDuration: Long` | `i64` ms | accumulated |

`MangaHistory.create()`: `id = -1`, `chapterId = -1`, `readAt = null`, `readDuration = -1`. `MangaHistoryUpdate(chapterId, readAt: Date, sessionReadDuration: Long)`. `MangaHistoryWithRelations(id, chapterId, mangaId, title, chapterNumber, readAt: Date?, readDuration, coverData: MangaCover)`.

### 1.6 `manga_sync` (`manga_sync.sq:3-21`)

| Column | Type | Null | Default | Kotlin (`MangaTrack`) | Rust |
|---|---|---|---|---|---|
| `_id` | INTEGER PK | NOT NULL | rowid | `id` | `i64` |
| `manga_id` | INTEGER FK mangas CASCADE | NOT NULL | - | `mangaId` | `i64` |
| `sync_id` | INTEGER | NOT NULL | - | `trackerId` | `i64` |
| `remote_id` | INTEGER | NOT NULL | - | `remoteId` | `i64` |
| `library_id` | INTEGER | NULL | NULL | `libraryId: Long?` | `Option<i64>` |
| `title` | TEXT | NOT NULL | - | `title` | `String` |
| `last_chapter_read` | REAL | NOT NULL | - | `lastChapterRead: Double` | `f64` |
| `total_chapters` | INTEGER | NOT NULL | - | `totalChapters: Long` | `i64` |
| `status` | INTEGER | NOT NULL | - | `status: Long` | `i64` |
| `score` | REAL | NOT NULL | - | `score: Double` | `f64` |
| `remote_url` | TEXT | NOT NULL | - | `remoteUrl` | `String` |
| `start_date` | INTEGER | NOT NULL | - | `startDate: Long` | `i64` (ms, 0 none) |
| `finish_date` | INTEGER | NOT NULL | - | `finishDate: Long` | `i64` (ms, 0 none) |
| `private` | INTEGER (Boolean) | NOT NULL | 0 | `private: Boolean` | `bool` |

Constraint: `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`. Tracker ids stored in `sync_id`: 1 MyAnimeList, 2 AniList, 3 Kitsu, 4 Shikimori, 5 Bangumi, 6 Komga, 7 MangaUpdates, 8 Kavita, 9 Suwayomi (`TrackerManager.kt`; manga trackers are those implementing `MangaTracker`; Simkl 101 and Jellyfin 102 are anime-only).

### 1.7 `excluded_scanlators`, `sources`, `extension_repos`

| Table | Columns | Constraints |
|---|---|---|
| `excluded_scanlators` | `manga_id INTEGER NOT NULL`, `scanlator TEXT NOT NULL` | FK `manga_id` -> `mangas(_id)` CASCADE; index `excluded_scanlators_manga_id_index` on `manga_id`; no uniqueness |
| `sources` | `_id INTEGER NOT NULL PRIMARY KEY`, `lang TEXT NOT NULL`, `name TEXT NOT NULL` | `_id` = source id (Long) |
| `extension_repos` | `base_url TEXT NOT NULL PRIMARY KEY`, `name TEXT NOT NULL`, `short_name TEXT`, `website TEXT NOT NULL`, `signing_key_fingerprint TEXT UNIQUE NOT NULL` | Kotlin `ExtensionRepo(baseUrl, name, shortName: String?, website, signingKeyFingerprint)`; fingerprint may start with the literal prefix `NOFINGERPRINT` (legacy placeholder accepted by `UpdateMangaExtensionRepo`) |

### 1.8 Views (result shapes)

- `libraryView`: 24 `mangas` columns (same names/order as 1.1) + `totalCount INTEGER`, `readCount` (sum, treated as REAL by SQLDelight), `latestUpload INTEGER`, `chapterFetchedAt INTEGER`, `lastRead INTEGER`, `bookmarkCount` (sum), `category INTEGER`. Kotlin `LibraryManga(manga, category, totalChapters, readCount, bookmarkCount, latestUpload, chapterFetchedAt, lastRead)` with derived `unreadCount`, `hasBookmarks`, `hasStarted`.
- `historyView` (13 columns): `id, mangaId, chapterId, title, thumbnailUrl, source, favorite, cover_last_modified, chapterNumber, readAt, readDuration, maxReadAt, maxReadAtChapterId`; the two queries project the first 11.
- `updatesView` (14 columns): `mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, last_page_read, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch` (sic, lowercase `f`). Kotlin `MangaUpdatesWithRelations(mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, lastPageRead, sourceId, dateFetch, coverData)`.
- Shared model `MangaCover(mangaId, sourceId, isMangaFavorite, url, lastModified)` is built from `(_id, source, favorite, thumbnail_url, cover_last_modified)` and drives the cover cache key (area:34-images-covers-cache).

## 2. Units and conventions

| Quantity | Unit | Written by |
|---|---|---|
| `last_update`, `next_update`, `date_added`, `cover_last_modified`, `date_fetch`, `date_upload`, `history.last_read`, `history.time_read`, `manga_sync.start_date`, `manga_sync.finish_date` | milliseconds since the UNIX epoch (UTC); `time_read` is a duration in ms | Kotlin `Instant.now().toEpochMilli()` / `Date().time` |
| `last_modified_at`, `favorite_modified_at` | seconds since the UNIX epoch, from `strftime('%s','now')` | triggers; inserts write 0 |
| `calculate_interval` | days | `MangaFetchInterval` |
| Booleans | `0` / `1` | `Boolean.toLong()` |
| `chapter_number` | REAL; `-1` unknown | `ChapterRecognition` |
| `score` | REAL, tracker-native scale (MangaUpdates: `max(score,0)` fix by migration 29) | tracker code |

## 3. Enums and codecs

### 3.1 Genre list (`data/src/main/java/tachiyomi/data/DatabaseAdapter.kt`, `StringListColumnAdapter`)

Separator `", "` (comma + one space). `decode("") = []`; `decode(s) = s.split(", ")` (no trimming, no de-duplication, no blank removal); `encode(list) = list.joinToString(", ")`. NULL column = `None`. A genre that itself contains `", "` is split on re-read (OBSERVED limitation, kept). Source-side parsing (`SManga.getGenres()` in `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt`): `genre.split(", ").map { it.trim() }.filterNot { it.isBlank() }.distinct()`, `null` when the string is null/blank.

### 3.2 Manga status (`SManga` constants, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt`)

`UNKNOWN = 0`, `ONGOING = 1`, `COMPLETED = 2`, `LICENSED = 3`, `PUBLISHING_FINISHED = 4`, `CANCELLED = 5`, `ON_HIATUS = 6`. The column stores the raw integer without validation.

### 3.3 Update strategy (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt`, `MangaUpdateStrategyColumnAdapter`)

`ALWAYS_UPDATE` = ordinal 0, `ONLY_FETCH_ONCE` = ordinal 1. Decode: `entries.getOrElse(value.toInt()) { ALWAYS_UPDATE }` (unknown/negative values -> `ALWAYS_UPDATE`). Encode: `ordinal`.

### 3.4 Date adapter

`DateColumnAdapter`: `decode(Long) = Date(ms)`, `encode(Date) = date.time`. Rust stores plain `i64` ms.

## 4. Bit layouts

All masks are `Long`. Source: `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt` companion, `tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt`, `ReaderOrientation.kt`.

### 4.1 `mangas.chapter_flags`

| Bits | Mask | Field | Values |
|---|---|---|---|
| bit 0 | `0x00000001` (`CHAPTER_SORT_DIR_MASK`) | sort direction | `0x0` `CHAPTER_SORT_DESC`, `0x1` `CHAPTER_SORT_ASC` |
| bits 1-2 | `0x00000006` (`CHAPTER_UNREAD_MASK`) | unread filter | `0x0` `SHOW_ALL`, `0x2` `CHAPTER_SHOW_UNREAD`, `0x4` `CHAPTER_SHOW_READ` |
| bits 3-4 | `0x00000018` (`CHAPTER_DOWNLOADED_MASK`) | downloaded filter | `0x0` all, `0x08` `CHAPTER_SHOW_DOWNLOADED`, `0x10` `CHAPTER_SHOW_NOT_DOWNLOADED` |
| bits 5-6 | `0x00000060` (`CHAPTER_BOOKMARKED_MASK`) | bookmark filter | `0x0` all, `0x20` `CHAPTER_SHOW_BOOKMARKED`, `0x40` `CHAPTER_SHOW_NOT_BOOKMARKED` |
| bits 8-9 | `0x00000300` (`CHAPTER_SORTING_MASK`) | sort key | `0x000` `CHAPTER_SORTING_SOURCE`, `0x100` `CHAPTER_SORTING_NUMBER`, `0x200` `CHAPTER_SORTING_UPLOAD_DATE`, `0x300` `CHAPTER_SORTING_ALPHABET` |
| bit 20 | `0x00100000` (`CHAPTER_DISPLAY_MASK`) | title display | `0x0` `CHAPTER_DISPLAY_NAME`, `0x100000` `CHAPTER_DISPLAY_NUMBER` |

`setFlag(flag, mask) = (value and mask.inv()) or (flag and mask)` (`SetMangaChapterFlags.kt`). `TriState` view: unread `0x2` -> `ENABLED_IS`, `0x4` -> `ENABLED_NOT`, else `DISABLED`; bookmarked `0x20` -> `ENABLED_IS`, `0x40` -> `ENABLED_NOT`, else `DISABLED`. Sorting action `awaitSetSortingModeOrFlipOrder(flag)`: when `sorting == flag` toggle direction bit; else set the key and force ASC. `sortDescending() = chapterFlags and 1 == 0` (DESC is the all-zero default). Default for new entries = the global preferences below (all `0` by default), applied by `SetMangaDefaultChapterFlags.awaitSetAllFlags` which REBUILDS the value from `0L` through the six masks.

### 4.2 `categories.flags` (library sort for that category)

| Bits | Mask | Field | Values |
|---|---|---|---|
| bits 2-5 | `0b00111100` (`0x3C`) | sort type | `Alphabetical 0b000000 (0)`, `LastRead 0b000100 (4)`, `LastUpdate 0b001000 (8)`, `UnreadCount 0b001100 (12)`, `TotalChapters 0b010000 (16)`, `LatestChapter 0b010100 (20)`, `ChapterFetchDate 0b011000 (24)`, `DateAdded 0b011100 (28)`, `TrackerMean 0b100000 (32)`, `Random 0b111100 (60)` |
| bit 6 | `0b01000000` (`0x40`) | direction | `Ascending 0x40`, `Descending 0x00` |

Decode: type = the entry whose `flag == value and 0x3C` (unknown -> default Alphabetical); direction = `value and 0x40`. Default sort (`MangaLibrarySort.default`) = Alphabetical + Ascending = `0x40`. Serialised preference form: `"<TYPE>,<DIRECTION>"` with TYPE in `ALPHABETICAL, LAST_READ, LAST_MANGA_UPDATE, UNREAD_COUNT, TOTAL_CHAPTERS, LATEST_CHAPTER, CHAPTER_FETCH_DATE, DATE_ADDED, TRACKER_MEAN, RANDOM` (unknown -> Alphabetical) and DIRECTION in `ASCENDING, DESCENDING` (anything other than `ASCENDING` -> Descending); an empty string or exception -> default. The seed system category has `flags = 0` (Alphabetical + Descending) but is overridden by the global preference unless `categorized_display` is on. Note: the type group occupies bits 2-5; `TrackerMean = 0b000100000 = 32` is bit 5 (the highest bit of the group) and `Random = 0b00111100 = 60` sets all four bits, so every type survives `value and 0x3C` unchanged.

### 4.3 `mangas.viewer` (reader settings)

| Bits | Mask | Field | Values |
|---|---|---|---|
| bits 0-2 | `0x00000007` (`ReadingMode.MASK`) | reading mode | `DEFAULT 0`, `LEFT_TO_RIGHT 1`, `RIGHT_TO_LEFT 2`, `VERTICAL 3`, `WEBTOON 4`, `CONTINUOUS_VERTICAL 5` |
| bits 3-5 | `0x00000038` (`ReaderOrientation.MASK`) | orientation | `DEFAULT 0x00`, `FREE 0x08`, `PORTRAIT 0x10`, `LANDSCAPE 0x18`, `LOCKED_PORTRAIT 0x20`, `LOCKED_LANDSCAPE 0x28`, `REVERSE_PORTRAIT 0x30` |

`fromPreference(value)` returns the entry whose `flagValue == value` (exact match, so a combined word must first be masked) else `DEFAULT`. `resetViewerFlags` zeroes the whole column. The backup creator writes `viewer = viewerFlags and ReadingMode.MASK` plus `viewer_flags = viewerFlags` (proto 103). On desktop the orientation bits have no meaning (see area:43-reader-core) but are preserved verbatim for backup round-trips.

## 5. Preference keys used by the database layer (types/defaults; store defined by area:02-preferences-system)

| Key | Type | Default | Used for |
|---|---|---|---|
| `library_sorting_mode` | object (string `"TYPE,DIRECTION"`) | `ALPHABETICAL,ASCENDING` | initial `flags` of new categories; global library sort; `ResetMangaCategoryFlags` |
| `categorized_display` | boolean | `false` | per-category sort vs global sort (`SetSortModeForMangaCategory`, restore) |
| `library_random_manga_sort_seed` | int | `0` | seed regenerated (`Random.nextInt()`) whenever the Random sort is chosen |
| `default_category` | int | `-1` | default category for "add to library"; deleted when its category is deleted |
| `__APP_STATE_last_used_category` | int | `0` | last visited library tab (key `Preference.appStateKey("last_used_category")`, prefix `__APP_STATE_`) |
| `library_update_categories` | string set of category ids | empty | categories included in library update |
| `library_update_categories_exclude` | string set | empty | categories excluded from library update |
| `remove_exclude_categories` | string set | empty | categories excluded from "delete after read" |
| `download_new_categories` | string set | empty | categories auto-downloaded |
| `download_new_categories_exclude` | string set | empty | categories excluded from auto-download |
| `pref_display_mode_library` | object (`COMPACT_GRID`, `COMFORTABLE_GRID`, `LIST`, `COVER_ONLY_GRID`) | `COMPACT_GRID` | library layout (not DB, listed because categories UI toggles it) |
| `default_chapter_filter_by_read` | long | `0` (`Manga.SHOW_ALL`) | default `unread` bits for `chapter_flags` |
| `default_chapter_filter_by_downloaded` | long | `0` | default downloaded bits |
| `default_chapter_filter_by_bookmarked` | long | `0` | default bookmark bits |
| `default_chapter_sort_by_source_or_number` | long | `0` (`CHAPTER_SORTING_SOURCE`) | default sort key |
| `default_chapter_display_by_name_or_number` | long | `0` (`CHAPTER_DISPLAY_NAME`) | default display mode (SAME key string as the anime preference `displayEpisodeByNameOrNumber`: manga and anime share it, an upstream collision documented in area:02-preferences-system) |
| `default_chapter_sort_by_ascending_or_descending` | long | `0` (`CHAPTER_SORT_DESC`) | default direction (also shared with anime) |
| `mark_duplicate_read_chapter_read` | string set | empty | chapter sync duplicate handling (values `MARK_DUPLICATE_CHAPTER_READ_EXISTING` / `_NEW`) |

## 6. Files and folders

| Path (Linux) | Content | Note |
|---|---|---|
| `<data dir>/manga.db` | the database (Aniyomi: `tachiyomi.db`) | `Paths::data_dir()` |
| `<data dir>/manga.db-wal`, `manga.db-shm` | WAL sidecars | copy only with the main file |
| `<data dir>/backups/db/manga.db.pre-v<from>-<UTC>.bak` | pre-migration copies, newest 3 kept | Proposal |
| `<data dir>/manga.db.dirty` | marker file present while the app is open | Proposal; triggers integrity check after a crash |
| `crates/kuro-db/sql/schema_v33.sql` | embedded baseline | in-repo |
| `crates/kuro-db/sql/legacy/1.sql` .. `32.sql` | embedded legacy migrations for the importer | in-repo, generated per `p1-topic-migration-history.md` section 4 |
| `crates/kuro-db/sql/migrations/34.sql` and later | Kurobayomi migrations | in-repo |

Database header fields (Proposal): `PRAGMA user_version` = schema version (33 at baseline; Aniyomi uses the same field the same way), `PRAGMA application_id` = `0x4B555230` (ASCII "KUR0", decimal 1263882800; Aniyomi files have 0), `PRAGMA journal_mode` = `wal` (persistent in the file), `PRAGMA foreign_keys` and `PRAGMA synchronous` are per-connection and re-applied on open.

## 7. Network endpoints touched

Only the extension-repository flow (statement family H) performs network I/O (via `mihon.domain.extensionrepo.service.ExtensionRepoService`, `domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoService.kt`):

| Item | Value |
|---|---|
| Method / URL | `GET <baseUrl>/repo.json` where `baseUrl` = the user URL without the trailing `/index.min.json` |
| Headers/auth | none beyond the shared OkHttp client defaults (area:12-network-stack) |
| Success body | `{"meta": {"name": str, "shortName": str|null, "website": str, "signingKeyFingerprint": str}}` (`ExtensionRepoMetaDto` / `ExtensionRepoDto`, `ignoreUnknownKeys = true` and `explicitNulls = false` come from the shared `Json` instance in `AppModule.kt`) |
| Failure | any exception, non-2xx (`awaitSuccess`) or malformed body returns `null` -> `InvalidUrl` / "skip" |
| Validation of the user URL | `^https://.*/index\.min\.json$` after OkHttp normalisation |

Rust: `reqwest` GET with the shared client from area:12-network-stack, `serde` structs `RepoMeta { meta: RepoMetaInner }` with `#[serde(rename_all = "camelCase")]` and `short_name: Option<String>` defaulting to `None`.

## 8. DB tables touched by this area's statements (read/write matrix)

| Statement family | mangas | chapters | categories | mangas_categories | history | manga_sync | excluded_scanlators | sources | extension_repos |
|---|---|---|---|---|---|---|---|---|---|
| mangas.sq | RW | R (`getReadMangaNotInLibrary`) | - | cascade | cascade | cascade | cascade | - | - |
| chapters.sq | W (trigger T6) | RW | - | - | cascade | - | R | - | - |
| categories.sq | - | - | RW | R, cascade | - | - | - | - | - |
| mangas_categories.sq | W (trigger T7) | - | - | RW | - | - | - | - | - |
| history.sq | R | R | - | - | RW | - | - | - | - |
| manga_sync.sq | - | - | - | - | - | RW | - | - | - |
| excluded_scanlators.sq | - | - | - | - | - | - | RW | - | - |
| sources.sq | - | - | - | - | - | - | - | RW | - |
| extension_repos.sq | - | - | - | - | - | - | - | - | RW |
| libraryView / historyView / updatesView | R | R | - | R (library) | R | - | R (library) | - | - |

(`R` = read, `W` = written indirectly by trigger, `RW` = read and written, `cascade` = rows deleted by `ON DELETE CASCADE`.)
