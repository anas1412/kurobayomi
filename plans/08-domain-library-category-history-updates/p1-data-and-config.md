# Area 08 — Domain: library, category, history, updates — data models and configuration

Everything marked OBSERVED comes from the Kotlin source (repo-relative to `/home/blackbox/aniyomi-src`); "Proposal" marks Rust decisions. Companion: `p1-behavior.md`, `p1-rust-design.md`.

## 1. Data models (every field)

Kotlin `Date` fields are stored as epoch milliseconds (`INTEGER`), via `DateColumnAdapter` (`data/src/main/java/tachiyomi/data/DatabaseAdapter.kt:11`: `decode = Date(long)`, `encode = date.time`). Proposal: Rust uses `i64` epoch ms everywhere (type alias `EpochMs`), `Option<i64>` where Kotlin has `Date?`; conversion to local time only at the UI edge (`time` crate with `local-offset`, area:01).

### 1.1 Category

`domain/src/main/java/tachiyomi/domain/category/model/Category.kt` (implements `java.io.Serializable`, irrelevant in Rust).

| Field | Kotlin type | Nullable | Default / constraint | DB column (`categories`) | Rust |
|---|---|---|---|---|---|
| `id` | Long | no | 0 = system category; new rows get an auto id | `_id INTEGER NOT NULL PRIMARY KEY` | `i64` |
| `name` | String | no | `""` for the system category | `name TEXT NOT NULL` | `String` |
| `order` | Long | no | system category -1; user categories 0..n-1 (see behaviour 1.6 for drift) | `sort INTEGER NOT NULL` | `i64` |
| `flags` | Long | no | sort flags, bits layout in section 3.1; system row 0 | `flags INTEGER NOT NULL` | `i64` (use `u16`-sized logic, keep `i64` storage) |
| `hidden` | Boolean | no | false | `hidden INTEGER NOT NULL DEFAULT 0` (1 = hidden) | `bool` |

Derived: `isSystemCategory = (id == 0)`; companion `UNCATEGORIZED_ID = 0L`.

`CategoryUpdate` (`domain/src/main/java/tachiyomi/domain/category/model/CategoryUpdate.kt`): `id: Long` (required), `name: String? = null`, `order: Long? = null`, `flags: Long? = null`, `hidden: Boolean? = null`. SQL `update` uses `coalesce(:x, x)` per column.

Membership tables (no Kotlin model): `mangas_categories(_id INTEGER PK, manga_id INTEGER NOT NULL FK mangas ON DELETE CASCADE, category_id INTEGER NOT NULL FK categories ON DELETE CASCADE)`; `animes_categories(_id, anime_id, category_id)` same. No UNIQUE on `(entry, category)`. Trigger on insert bumps `version` of the entry unless `is_syncing`. Full DDL: area:03/04 `p1-topic-baseline-schema.md` / `p1-topic-fresh-vs-upgraded-schema.md`.

### 1.2 History

| Model (file) | Fields (Kotlin type, nullable) | Defaults / factory |
|---|---|---|
| `MangaHistory` (`domain/src/main/java/tachiyomi/domain/history/manga/model/MangaHistory.kt`) | `id: Long`, `chapterId: Long`, `readAt: Date?`, `readDuration: Long` | `create()` = `id -1, chapterId -1, readAt null, readDuration -1` |
| `MangaHistoryUpdate` (`domain/src/main/java/tachiyomi/domain/history/manga/model/MangaHistoryUpdate.kt`) | `chapterId: Long`, `readAt: Date`, `sessionReadDuration: Long` | none |
| `MangaHistoryWithRelations` (`domain/src/main/java/tachiyomi/domain/history/manga/model/MangaHistoryWithRelations.kt`) | `id: Long`, `chapterId: Long`, `mangaId: Long`, `title: String`, `chapterNumber: Double`, `readAt: Date?`, `readDuration: Long`, `coverData: MangaCover` | none |
| `AnimeHistory` (`domain/src/main/java/tachiyomi/domain/history/anime/model/AnimeHistory.kt`) | `id: Long`, `episodeId: Long`, `seenAt: Date?` | `create()` = `-1, -1, null` |
| `AnimeHistoryUpdate` | `episodeId: Long`, `seenAt: Date` | none |
| `AnimeHistoryWithRelations` | `id`, `episodeId`, `animeId`, `title: String`, `episodeNumber: Double`, `seenAt: Date?`, `coverData: AnimeCover` | none |

DB tables: `history(_id INTEGER NOT NULL PRIMARY KEY, chapter_id INTEGER NOT NULL UNIQUE FK chapters(_id) ON DELETE CASCADE, last_read INTEGER AS Date, time_read INTEGER NOT NULL)` + index `history_history_chapter_id_index ON history(chapter_id)`; `animehistory(_id INTEGER NOT NULL PRIMARY KEY, episode_id INTEGER NOT NULL UNIQUE FK episodes(_id) ON DELETE CASCADE, last_seen INTEGER AS Date)` + index `animehistory_history_episode_id_index`. `last_read`/`last_seen` is nullable in SQL (`INTEGER AS Date` without NOT NULL); history rows deleted automatically when their chapter/episode row is deleted (FK cascade; chapter rows are deleted when a chapter disappears from the source and is not kept, or when the entry is deleted).

Mapper `MangaHistoryMapper.mapMangaHistoryWithRelations(historyId, mangaId, chapterId, title, thumbnailUrl, sourceId, isFavorite, coverLastModified, chapterNumber, readAt, readDuration)` (note the parameter order: `historyId, mangaId, chapterId` = view columns `id, mangaId, chapterId`). Anime: `(historyId, animeId, episodeId, title, thumbnailUrl, sourceId, isFavorite, coverLastModified, episodeNumber, seenAt)`.

Cover models (owned by area:07): `MangaCover(mangaId: Long, sourceId: Long, isMangaFavorite: Boolean, url: String?, lastModified: Long)`; `AnimeCover(animeId, sourceId, isAnimeFavorite, url: String?, lastModified)`. Used as the key of the image loader (area:34-images-covers-cache).

### 1.3 Library rows

| Model | Fields (Kotlin type) | Source column in view | Derived |
|---|---|---|---|
| `LibraryManga` (`domain/src/main/java/tachiyomi/domain/library/manga/LibraryManga.kt`) | `manga: Manga`, `category: Long`, `totalChapters: Long`, `readCount: Long`, `bookmarkCount: Long`, `latestUpload: Long`, `chapterFetchedAt: Long`, `lastRead: Long` | `category` = `coalesce(MC.category_id, 0)`; `totalChapters` = `totalCount`; `readCount` (REAL in mapper, `.toLong()`); `bookmarkCount` (REAL, `.toLong()`); `latestUpload`; `chapterFetchedAt`; `lastRead` | `id = manga.id`; `unreadCount = totalChapters - readCount`; `hasBookmarks = bookmarkCount > 0`; `hasStarted = readCount > 0` |
| `LibraryAnime` (`domain/src/main/java/tachiyomi/domain/library/anime/LibraryAnime.kt`) | `anime: Anime`, `category: Long`, `totalCount: Long`, `seenCount: Long`, `bookmarkCount: Long`, `fillermarkCount: Long`, `latestUpload: Long`, `episodeFetchedAt: Long`, `lastSeen: Long` | `animelibView` columns of the same names | `id`, `unseenCount = totalCount - seenCount`, `hasBookmarks`, `hasStarted = seenCount > 0` |

The embedded `Manga`/`Anime` entity belongs to area:07-domain-entries-items. Fields consumed by this area: `id`, `source`, `title`, `author`, `artist`, `description`, `genre: List<String>?`, `status: Long` (`SManga.COMPLETED = 2`; anime `SAnime.COMPLETED = 2`), `lastUpdate: Long` (epoch ms, 0 = never), `dateAdded: Long`, `fetchInterval: Int` (< 0 means a custom/predicted interval), `nextUpdate: Long`, `viewerFlags: Long` (anime airing decode, section 3.4), `chapterFlags`/`episodeFlags: Long`, `favorite: Boolean`, `thumbnailUrl: String?`, `coverLastModified: Long`, plus `isLocal()` = `source == 0L` (`LocalMangaSource.ID = 0L`, `LocalAnimeSource.ID = 0L`).

### 1.4 Update feed rows

| Model | Fields (Kotlin type) | View column |
|---|---|---|
| `MangaUpdatesWithRelations` | `mangaId: Long`, `mangaTitle: String`, `chapterId: Long`, `chapterName: String`, `scanlator: String?`, `read: Boolean`, `bookmark: Boolean`, `lastPageRead: Long`, `sourceId: Long`, `dateFetch: Long`, `coverData: MangaCover` | `mangaId`, `mangaTitle`, `chapterId`, `chapterName`, `scanlator`, `read`, `bookmark`, `last_page_read`, `source`, (`favorite`, `thumbnailUrl`, `coverLastModified`, `dateUpload` are consumed by the mapper but only `coverData`/`dateFetch` survive), `datefetch` |
| `AnimeUpdatesWithRelations` | `animeId`, `animeTitle`, `episodeId`, `episodeName`, `scanlator: String?`, `seen: Boolean`, `bookmark: Boolean`, `fillermark: Boolean`, `lastSecondSeen: Long`, `totalSeconds: Long`, `sourceId: Long`, `dateFetch: Long`, `coverData: AnimeCover` | `animeId`, `animeTitle`, `episodeId`, `episodeName`, `scanlator`, `seen`, `bookmark`, `fillermark`, `last_second_seen`, `totalSeconds`, `source`, `datefetch` |

`dateUpload` is passed through the mapper but not stored in the model; the feed row never exposes the upload date.

### 1.5 Sort and display models

- `MangaLibrarySort(type: Type, direction: Direction)` with `flag = type + direction`, `mask = type.mask or direction.mask`, `isAscending = direction == Ascending`. `AnimeLibrarySort` the same with the anime `Type` set. Defaults `MangaLibrarySort.default = (Alphabetical, Ascending)`, `AnimeLibrarySort.default` same.
- `LibraryDisplayMode`: `CompactGrid | ComfortableGrid | List | CoverOnlyGrid`; `default = CompactGrid`; `values = {CompactGrid, ComfortableGrid, List, CoverOnlyGrid}` (insertion order).
- `Flag { flag: Long }`, `Mask { mask: Long }`, `FlagWithMask : Flag, Mask`.

## 2. Enumerations and stored values

| Enum | Variants and stored value | Where stored |
|---|---|---|
| `TriState` (`core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt`) | `DISABLED`, `ENABLED_IS`, `ENABLED_NOT` (name string); `next()`: DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED. Legacy integer form `0`/`1` existed before migration 52 (`MigrateToTriStateMigration`: Boolean true -> `1`); current store uses the enum name | all `pref_filter_*_v2` keys |
| `LibraryDisplayMode` | `COMPACT_GRID`, `COMFORTABLE_GRID`, `COVER_ONLY_GRID`, `LIST`; unknown -> `COMPACT_GRID` | `pref_display_mode_library` |
| Manga sort type (string) | `ALPHABETICAL`, `LAST_READ`, `LAST_MANGA_UPDATE`, `UNREAD_COUNT`, `TOTAL_CHAPTERS`, `LATEST_CHAPTER`, `CHAPTER_FETCH_DATE`, `DATE_ADDED`, `TRACKER_MEAN`, `RANDOM`; unknown -> `ALPHABETICAL` | `library_sorting_mode` as `"<TYPE>,<DIR>"` |
| Anime sort type (string) | `ALPHABETICAL`, `LAST_SEEN`, `LAST_ANIME_UPDATE`, `UNSEEN_COUNT`, `TOTAL_EPISODES`, `LATEST_EPISODE`, `EPISODE_FETCH_DATE`, `DATE_ADDED`, `TRACKER_MEAN`, `AIRING_TIME`, `RANDOM` | `animelib_sorting_mode` |
| Sort direction (string) | `ASCENDING`, `DESCENDING`; any other second token -> `Descending`; missing second token -> parse error -> default | both sort keys |
| `LibraryPreferences.EpisodeSwipeAction` | `ToggleSeen`, `ToggleBookmark`, `ToggleFillermark`, `Download`, `Disabled` (enum name) | `pref_episode_swipe_end_action`, `pref_episode_swipe_start_action` |
| `LibraryPreferences.ChapterSwipeAction` | `ToggleRead`, `ToggleBookmark`, `Download`, `Disabled` | `pref_chapter_swipe_end_action`, `pref_chapter_swipe_start_action` |
| Device restrictions (strings) | `DEVICE_ONLY_ON_WIFI = "wifi"`, `DEVICE_NETWORK_NOT_METERED = "network_not_metered"`, `DEVICE_CHARGING = "ac"` | `library_update_restriction` |
| Item restrictions (strings) | `ENTRY_NON_COMPLETED = "manga_ongoing"`, `ENTRY_HAS_UNVIEWED = "manga_fully_read"`, `ENTRY_NON_VIEWED = "manga_started"`, `ENTRY_OUTSIDE_RELEASE_PERIOD = "manga_outside_release_period"` | `library_update_manga_restriction` |
| Duplicate marking (strings) | `MARK_DUPLICATE_CHAPTER_READ_NEW = "new"`, `MARK_DUPLICATE_CHAPTER_READ_EXISTING = "existing"`, `MARK_DUPLICATE_EPISODE_SEEN_NEW = "new_episode"`, `MARK_DUPLICATE_EPISODE_SEEN_EXISTING = "existing_episode"` | `mark_duplicate_read_chapter_read`, `mark_duplicate_seen_episode_seen` |
| `SeasonDisplayMode` (`domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt`) | `CompactGrid = 0L`, `ComfortableGrid = 1L`, `CoverOnlyGrid = 2L`, `List = 3L`; `fromLong` throws `IllegalArgumentException("Invalid sorting value")` for any other value | `default_season_grid_display_mode` (Long), and bits 16-17 of `season_flags` |
| `CheckboxState` | `State.Checked/None`; `TriState.Include/Exclude/None`; cycles in behaviour 1.10 | in-memory only |
| `SManga.COMPLETED` / `SAnime.COMPLETED` | `2` (other statuses: 0 UNKNOWN, 1 ONGOING, 3 LICENSED, 4 PUBLISHING_FINISHED, 5 CANCELLED, 6 ON_HIATUS) | `mangas.status` |
| History/download state strings | n/a | n/a |

Dropped/legacy: before migration 64 the sort direction was a separate Boolean key `library_sorting_ascending`; migration 82 merged it into the `"<TYPE>,<DIR>"` string (`MergeSortTypeDirectionMigration`). A fresh Kurobayomi install never needs these; area:22-backup-restore reads backups containing only the merged form.

## 3. Bit layouts

### 3.1 Library sort flags (`categories.flags`, 7 significant bits)

```
bit:   6   5 4 3 2   1 0
       D   T T T T   - -          D = direction bit (mask 0b01000000 = 0x40), T = type (mask 0b00111100 = 0x3C), bits 0-1 and 7+ unused
```

| Value | Binary | Hex | Manga type | Anime type |
|---|---|---|---|---|
| type 0 | `0b00000000` | 0x00 | Alphabetical | Alphabetical |
| type 1 | `0b00000100` | 0x04 | LastRead | LastSeen |
| type 2 | `0b00001000` | 0x08 | LastUpdate | LastUpdate |
| type 3 | `0b00001100` | 0x0C | UnreadCount | UnseenCount |
| type 4 | `0b00010000` | 0x10 | TotalChapters | TotalEpisodes |
| type 5 | `0b00010100` | 0x14 | LatestChapter | LatestEpisode |
| type 6 | `0b00011000` | 0x18 | ChapterFetchDate | EpisodeFetchDate |
| type 7 | `0b00011100` | 0x1C | DateAdded | DateAdded |
| type 8 | `0b00100000` (source writes `0b000100000`) | 0x20 | TrackerMean | TrackerMean |
| type 12 | `0b00110000` | 0x30 | (none) | AiringTime |
| type 15 | `0b00111100` | 0x3C | Random | Random |
| direction | `0b01000000` | 0x40 | Ascending | Ascending |
| direction | `0b00000000` | 0x00 | Descending | Descending |

Unassigned type patterns (0x24, 0x28, 0x2C, 0x34, 0x38) decode to the default type Alphabetical (`Type.valueOf` falls back to `default.type`). The fallback of `Direction.valueOf` is unreachable (every 0/0x40 pattern matches). Examples (from `LibraryFlagsTest`): `LastRead|Asc = 0x44`; `DateAdded|Asc = 0b01011100 = 0x5C = 92`; default `Alphabetical|Ascending = 0b01000000 = 64`; `UnreadCount|Desc = 0b00001100 = 12`; system category seed flags `0` = Alphabetical Descending.

Flag algebra (`domain/src/main/java/tachiyomi/domain/library/model/Flag.kt`), all on `Long`:
- `Long.contains(other: Flag)`: if `other` is a `Mask`: `other.flag == (this and other.mask)`, else `other.flag == this`. (Parameter order is "does `this` value contain that flag".)
- `Long.plus(other: Flag): Long`: if `other` is a `Mask`: `(this and other.mask.inv()) or (other.flag and other.mask)`, else `this or other.flag`.
- `Flag.plus(other: Flag): Long`: if `other` is a `Mask`: `(this.flag and other.mask.inv()) or (other.flag and other.mask)`, else `this.flag or other.flag`.
- Kotlin evaluates `a and b.inv() or (c and d)` left to right as `((a and b.inv()) or (c and d))`.
- `Type` and `Direction` classes are both `FlagWithMask`; a combined `XLibrarySort` has `flag = type + direction` (`Flag.plus(Flag)` with the direction as the mask carrier) and `mask = type.mask or direction.mask = 0x7C`.

String form of a sort: `"<TYPE>,<DIRECTION>"` (see section 2), `serialize()` always emits both tokens; `deserialize`: empty string -> default; `split(",")`; unknown type token -> Alphabetical; `values[1]` missing -> `IndexOutOfBoundsException` -> default; direction `"ASCENDING"` -> Ascending, anything else -> Descending.

### 3.2 Chapter/episode flags (read by `LibraryPreferences` defaults; owned by area:07)

Manga `chapter_flags` (`domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:77-103`): `SHOW_ALL = 0x0`; sort direction `CHAPTER_SORT_DESC = 0x0`, `CHAPTER_SORT_ASC = 0x1`, mask `0x1`; unread filter `CHAPTER_SHOW_UNREAD = 0x2`, `CHAPTER_SHOW_READ = 0x4`, mask `0x6`; downloaded `0x8` / `0x10`, mask `0x18`; bookmarked `0x20` / `0x40`, mask `0x60`; sorting `CHAPTER_SORTING_SOURCE = 0x000`, `NUMBER = 0x100`, `UPLOAD_DATE = 0x200`, `ALPHABET = 0x300`, mask `0x300`; display `CHAPTER_DISPLAY_NAME = 0x0`, `NUMBER = 0x100000`, mask `0x100000`.

Anime `episode_flags` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:217-255`): `EPISODE_SORT_DESC = 0x0`, `ASC = 0x1`, mask `0x1`; unseen `SHOW_UNSEEN = 0x2`, `SHOW_SEEN = 0x4`, mask `0x6`; downloaded `0x8`/`0x10`, mask `0x18`; bookmarked `0x20`/`0x40`, mask `0x60`; fillermarked `0x80`/`0x100`, mask `0x180`; sorting `SOURCE = 0x0`, `NUMBER = 0x200`, `UPLOAD_DATE = 0x400`, `ALPHABET = 0x600`, mask `0x600`; previews `EPISODE_SHOW_PREVIEWS = 0x0`, `SHOW_NOT_PREVIEWS = 0x800`, mask `0x800`; summaries `SHOW_SUMMARIES = 0x0`, `NOT = 0x1000`, mask `0x1000`; display `NAME = 0x0`, `NUMBER = 0x100000`, mask `0x100000`.

Because the display (`0x100000`) and direction (`0x1`) values are identical in both kinds, the shared preference keys of section 5 (rows 58-61) hold values valid for both.

### 3.3 Anime season flags (`season_flags`, read/written by `setSeasonSettingsDefault`; owned by area:07)

`SEASON_SORT_DESC = 0x0`, `ASC = 0x1` (mask `0x1`); downloaded `0x2`/`0x4` (mask `0x6`); unseen `0x8`/`0x10` (mask `0x18`); started `0x20`/`0x40` (mask `0x60`); completed `0x80`/`0x100` (mask `0x180`); bookmarked `0x200`/`0x400` (mask `0x600`); fillermarked `0x800`/`0x1000` (mask `0x1800`); sort `SOURCE 0x0`, `SEASON 0x2000`, `UPLOAD 0x4000`, `ALPHABET 0x6000`, `COUNT 0x8000`, `LAST_SEEN 0xA000`, `FETCHED 0xC000` (mask `0xE000`); grid display mode bits 16-17 (`SEASON_GRID_DISPLAY_MODE_BIT_OFFSET = 16`, mask `0x30000`); grid size bits 18-21 (offset 18, mask `0x3C0000`); overlays `DOWNLOADED 0x400000`, `UNSEEN 0x800000`, `LOCAL 0x1000000`, `LANG 0x2000000`, `CONT 0x4000000`; display mode `SOURCE 0x0`, `NUMBER 0x8000000` (mask `0x8000000`).

### 3.4 Anime `viewer_flags` (airing decode used by the AiringTime sort)

`ANIME_INTRO_MASK = 0xFF`, `ANIME_AIRING_EPISODE_MASK = 0xFFFF00`, `ANIME_AIRING_TIME_MASK = 0x0FFFFFFFF000000`, `ANIME_INTRO_DISABLE_MASK = 0x100000000000000`. `nextEpisodeToAir = ((viewerFlags and 0xFFFF00) / 16^2).toInt()`; `nextEpisodeAiringAt = (viewerFlags and 0x0FFFFFFFF000000) / 16^6` (a `Double` division by `Math.pow(16.0, zeros)` truncated to Long; equals a right shift by 8 and 24 bits respectively, exact for the 32-bit time field). Value 0 means unknown.

## 4. SQL touched by this area (tables, statements, columns)

Authoritative DDL lives in area:03 / area:04; this section lists what the domain relies on.

| Table / view | DB | Columns read/written here | Statements used by this area |
|---|---|---|---|
| `categories` | both | `_id`, `name`, `sort`, `flags`, `hidden` | `getCategory`, `getCategories`, `getVisibleCategories`, `getCategoriesByMangaId`/`ByAnimeId`, `getVisibleCategoriesByMangaId`/`ByAnimeId`, `insert(name, sort, flags)`, `delete`, `update` (coalesce partial), `updateAllFlags`, `selectLastInsertedRowId` (backup restore) |
| `mangas_categories` / `animes_categories` | manga / anime | `manga_id`/`anime_id`, `category_id` | `insert`, `deleteMangaCategoryByMangaId` / `deleteAnimeCategoryByAnimeId` |
| `history` | manga | `_id`, `chapter_id` UNIQUE, `last_read`, `time_read` | `getHistoryByMangaId`, `getHistoryByChapterUrl` (used by area:22 restore), `resetHistoryById`, `resetHistoryByMangaId`, `removeAllHistory`, `removeResettedHistory`, `upsert`, `getReadDuration` |
| `animehistory` | anime | `_id`, `episode_id` UNIQUE, `last_seen` | `getHistoryByAnimeId`, `getHistoryByEpisodeUrl`, `resetAnimeHistoryById`, `resetHistoryByAnimeId`, `removeAllHistory`, `removeResettedHistory`, `upsert` |
| `historyView` / `animehistoryView` | | see behaviour 3.3 | `history(query)` / `animehistory(query)`, `getLatestHistory` / `getLatestAnimeHistory` |
| `libraryView` / `animelibView` | | see behaviour 2.1 | `library` / `animelib` |
| `updatesView` / `animeupdatesView` | | see behaviour 4.1 | `getRecentUpdates`, `getUpdatesByReadStatus` / `getRecentAnimeUpdates`, `getUpdatesBySeenStatus` |

Verbatim statements that matter for behaviour (repeat of the sources so the implementer never opens the `.sq`):

```sql
-- data/src/main/sqldelight/data/categories.sq (identical in data/src/main/sqldelightanime/dataanime/categories.sq)
INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0);
CREATE TRIGGER IF NOT EXISTS system_category_delete_trigger BEFORE DELETE ON categories
BEGIN SELECT CASE WHEN old._id <= 0 THEN RAISE(ABORT, "System category can't be deleted") END; END;
update: UPDATE categories SET name = coalesce(:name, name), sort = coalesce(:order, sort),
        flags = coalesce(:flags, flags), hidden = coalesce(:hidden, hidden) WHERE _id = :categoryId;
updateAllFlags: UPDATE categories SET flags = coalesce(?, flags);
-- data/src/main/sqldelight/data/history.sq
upsert: INSERT INTO history(chapter_id, last_read, time_read) VALUES (:chapterId, :readAt, :time_read)
        ON CONFLICT(chapter_id) DO UPDATE SET last_read = :readAt, time_read = time_read + :time_read WHERE chapter_id = :chapterId;
resetHistoryById: UPDATE history SET last_read = 0 WHERE _id = :historyId;
resetHistoryByMangaId: UPDATE history SET last_read = 0 WHERE _id IN (SELECT H._id FROM mangas M INNER JOIN chapters C ON M._id = C.manga_id INNER JOIN history H ON C._id = H.chapter_id WHERE M._id = :mangaId);
getReadDuration: SELECT coalesce(sum(time_read), 0) FROM history;
-- data/src/main/sqldelightanime/dataanime/animehistory.sq
upsert: INSERT INTO animehistory(episode_id, last_seen) VALUES (:episodeId, :seenAt) ON CONFLICT(episode_id) DO UPDATE SET last_seen = :seenAt WHERE episode_id = :episodeId;
```

SQLite versions: the `ON CONFLICT ... DO UPDATE` upsert needs SQLite >= 3.24; Kurobayomi bundles SQLite through `rusqlite` feature `bundled` (area:01), so this is satisfied.

Network endpoints: none. Files/folders: this area writes no files of its own; its state is the two SQLite databases (`manga.db`, `anime.db`, area:03/04) and the preference stores (`config.toml`, `state.toml`, area:02).

## 5. Preference keys (`LibraryPreferences`, 87 accessors)

Namespaces (area:02): `normal` keys are user preferences (included in backups); `app-state` keys carry the `__APP_STATE_` prefix in the Kotlin store (`Preference.appStateKey`) and are never backed up. The "Key" column shows the logical key (without prefix). Rust field names follow the area:02 rule (snake_case of the Kotlin function). One struct `LibraryPrefs` in `kuro-prefs::library`. Unit: Long preferences hold flag values from section 3.2/3.3.

| # | Kotlin function (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:line`) | Key | Namespace | Type | Default | Rust field (`LibraryPrefs`) | Note |
|---|---|---|---|---|---|---|---|
| 1 | `displayMode()` :18 | `pref_display_mode_library` | normal | object (string codec) | `CompactGrid` (`COMPACT_GRID`) | `display_mode` | shared by manga and anime libraries |
| 2 | `mangaSortingMode()` :25 | `library_sorting_mode` | normal | object (string codec) | `Alphabetical, Ascending` (`ALPHABETICAL,ASCENDING`) | `manga_sorting_mode` | global sort; per-category flags live in `categories.flags` |
| 3 | `animeSortingMode()` :32 | `animelib_sorting_mode` | normal | object (string codec) | `Alphabetical, Ascending` (`ALPHABETICAL,ASCENDING`) | `anime_sorting_mode` |  |
| 4 | `lastUpdatedTimestamp()` :39 | `library_update_last_timestamp` | app-state (`__APP_STATE_` prefix) | Long | 0L | `last_updated_timestamp` | app-state; epoch ms; written by both library update jobs |
| 5 | `autoUpdateInterval()` :40 | `pref_library_update_interval_key` | normal | Int | 0 | `auto_update_interval` | hours; UI options 0,12,24,48,72,168 |
| 6 | `autoUpdateDeviceRestrictions()` :42 | `library_update_restriction` | normal | Set<String> | `{wifi}` | `auto_update_device_restrictions` | values `wifi`, `network_not_metered`, `ac` |
| 7 | `autoUpdateItemRestrictions()` :49 | `library_update_manga_restriction` | normal | Set<String> | `{manga_fully_read, manga_ongoing, manga_started, manga_outside_release_period}` | `auto_update_item_restrictions` | values `manga_fully_read`, `manga_ongoing`, `manga_started`, `manga_outside_release_period` (key spelled `library_update_manga_restriction`, shared by manga and anime) |
| 8 | `autoUpdateMetadata()` :59 | `auto_update_metadata` | normal | Boolean | false | `auto_update_metadata` | refresh cover/details during library update |
| 9 | `showContinueViewingButton()` :61 | `display_continue_reading_button` | normal | Boolean | false | `show_continue_viewing_button` | one key for both libraries |
| 10 | `categoryTabs()` :66 | `display_category_tabs` | normal | Boolean | true | `category_tabs` |  |
| 11 | `categoryNumberOfItems()` :68 | `display_number_of_items` | normal | Boolean | false | `category_number_of_items` |  |
| 12 | `categorizedDisplaySettings()` :70 | `categorized_display` | normal | Boolean | false | `categorized_display_settings` | per-category sort on/off; shared manga+anime |
| 13 | `hideHiddenCategoriesSettings()` :72 | `hidden_categories` | normal | Boolean | false | `hide_hidden_categories_settings` | affects categories screen and storage screen only |
| 14 | `filterIntervalCustom()` :74 | `pref_filter_library_interval_custom` | normal | enum (by name) | `DISABLED` | `filter_interval_custom` | one key shared by manga and anime |
| 15 | `downloadBadge()` :81 | `display_download_badge` | normal | Boolean | false | `download_badge` |  |
| 16 | `unreadBadge()` :83 | `display_unread_badge` | normal | Boolean | true | `unread_badge` | also drives the anime unseen badge |
| 17 | `localBadge()` :85 | `display_local_badge` | normal | Boolean | true | `local_badge` |  |
| 18 | `languageBadge()` :87 | `display_language_badge` | normal | Boolean | false | `language_badge` |  |
| 19 | `newShowUpdatesCount()` :89 | `library_show_updates_count` | normal | Boolean | true | `new_show_updates_count` |  |
| 20 | `autoClearItemCache()` :93 | `auto_clear_chapter_cache` | normal | Boolean | false | `auto_clear_item_cache` | clear chapter cache at cold start |
| 21 | `randomAnimeSortSeed()` :97 | `library_random_anime_sort_seed` | normal | Int | 0 | `random_anime_sort_seed` | re-rolled by Random sort selection |
| 22 | `randomMangaSortSeed()` :98 | `library_random_manga_sort_seed` | normal | Int | 0 | `random_manga_sort_seed` |  |
| 23 | `animePortraitColumns()` :102 | `pref_animelib_columns_portrait_key` | normal | Int | 0 | `anime_portrait_columns` |  |
| 24 | `mangaPortraitColumns()` :103 | `pref_library_columns_portrait_key` | normal | Int | 0 | `manga_portrait_columns` |  |
| 25 | `animeLandscapeColumns()` :105 | `pref_animelib_columns_landscape_key` | normal | Int | 0 | `anime_landscape_columns` |  |
| 26 | `mangaLandscapeColumns()` :106 | `pref_library_columns_landscape_key` | normal | Int | 0 | `manga_landscape_columns` |  |
| 27 | `filterDownloadedAnime()` :110 | `pref_filter_animelib_downloaded_v2` | normal | enum (by name) | `DISABLED` | `filter_downloaded_anime` |  |
| 28 | `filterDownloadedManga()` :113 | `pref_filter_library_downloaded_v2` | normal | enum (by name) | `DISABLED` | `filter_downloaded_manga` |  |
| 29 | `filterUnseen()` :116 | `pref_filter_animelib_unread_v2` | normal | enum (by name) | `DISABLED` | `filter_unseen` | anime filter; key says `unread` |
| 30 | `filterUnread()` :119 | `pref_filter_library_unread_v2` | normal | enum (by name) | `DISABLED` | `filter_unread` | manga filter |
| 31 | `filterStartedAnime()` :122 | `pref_filter_animelib_started_v2` | normal | enum (by name) | `DISABLED` | `filter_started_anime` |  |
| 32 | `filterStartedManga()` :125 | `pref_filter_library_started_v2` | normal | enum (by name) | `DISABLED` | `filter_started_manga` |  |
| 33 | `filterBookmarkedAnime()` :128 | `pref_filter_animelib_bookmarked_v2` | normal | enum (by name) | `DISABLED` | `filter_bookmarked_anime` |  |
| 34 | `filterBookmarkedManga()` :131 | `pref_filter_library_bookmarked_v2` | normal | enum (by name) | `DISABLED` | `filter_bookmarked_manga` |  |
| 35 | `filterCompletedAnime()` :134 | `pref_filter_animelib_completed_v2` | normal | enum (by name) | `DISABLED` | `filter_completed_anime` |  |
| 36 | `filterCompletedManga()` :137 | `pref_filter_library_completed_v2` | normal | enum (by name) | `DISABLED` | `filter_completed_manga` |  |
| 37 | `filterTrackedAnime(id: Int)` :140 | `pref_filter_animelib_tracked_{id}_v2` | normal | enum (by name) | `DISABLED` | `filter_tracked_anime(id)` | dynamic family; id = tracker id |
| 38 | `filterTrackedManga(id: Int)` :143 | `pref_filter_library_tracked_{id}_v2` | normal | enum (by name) | `DISABLED` | `filter_tracked_manga(id)` | dynamic family; id = tracker id |
| 39 | `newMangaUpdatesCount()` :148 | `library_unread_updates_count` | normal | Int | 0 | `new_manga_updates_count` | reset to 0 when the Updates tab opens/closes |
| 40 | `newAnimeUpdatesCount()` :149 | `library_unseen_updates_count` | normal | Int | 0 | `new_anime_updates_count` |  |
| 41 | `defaultAnimeCategory()` :153 | `default_anime_category` | normal | Int | -1 | `default_anime_category` | -1 = always ask, 0 = Default, else category id |
| 42 | `defaultMangaCategory()` :154 | `default_category` | normal | Int | -1 | `default_manga_category` |  |
| 43 | `lastUsedAnimeCategory()` :156 | `last_used_anime_category` | app-state (`__APP_STATE_` prefix) | Int | 0 | `last_used_anime_category` | app-state; INDEX in displayed list |
| 44 | `lastUsedMangaCategory()` :157 | `last_used_category` | app-state (`__APP_STATE_` prefix) | Int | 0 | `last_used_manga_category` | app-state; INDEX in displayed list |
| 45 | `animeUpdateCategories()` :159 | `animelib_update_categories` | normal | Set<String> | `{}` (empty) | `anime_update_categories` | include set of category id strings |
| 46 | `mangaUpdateCategories()` :162 | `library_update_categories` | normal | Set<String> | `{}` (empty) | `manga_update_categories` |  |
| 47 | `animeUpdateCategoriesExclude()` :165 | `animelib_update_categories_exclude` | normal | Set<String> | `{}` (empty) | `anime_update_categories_exclude` | exclude set |
| 48 | `mangaUpdateCategoriesExclude()` :168 | `library_update_categories_exclude` | normal | Set<String> | `{}` (empty) | `manga_update_categories_exclude` |  |
| 49 | `filterEpisodeBySeen()` :173 | `default_episode_filter_by_seen` | normal | Long | 0x0 (SHOW_ALL) | `filter_episode_by_seen` |  |
| 50 | `filterChapterByRead()` :176 | `default_chapter_filter_by_read` | normal | Long | 0x0 (SHOW_ALL) | `filter_chapter_by_read` |  |
| 51 | `filterEpisodeByDownloaded()` :179 | `default_episode_filter_by_downloaded` | normal | Long | 0x0 (SHOW_ALL) | `filter_episode_by_downloaded` |  |
| 52 | `filterChapterByDownloaded()` :182 | `default_chapter_filter_by_downloaded` | normal | Long | 0x0 (SHOW_ALL) | `filter_chapter_by_downloaded` |  |
| 53 | `filterEpisodeByBookmarked()` :185 | `default_episode_filter_by_bookmarked` | normal | Long | 0x0 (SHOW_ALL) | `filter_episode_by_bookmarked` |  |
| 54 | `filterChapterByBookmarked()` :188 | `default_chapter_filter_by_bookmarked` | normal | Long | 0x0 (SHOW_ALL) | `filter_chapter_by_bookmarked` |  |
| 55 | `filterEpisodeByFillermarked()` :191 | `default_episode_filter_by_fillermarked` | normal | Long | 0x0 (SHOW_ALL) | `filter_episode_by_fillermarked` |  |
| 56 | `sortEpisodeBySourceOrNumber()` :195 | `default_episode_sort_by_source_or_number` | normal | Long | 0x0 (SOURCE) | `sort_episode_by_source_or_number` |  |
| 57 | `sortChapterBySourceOrNumber()` :200 | `default_chapter_sort_by_source_or_number` | normal | Long | 0x0 (SOURCE) | `sort_chapter_by_source_or_number` |  |
| 58 | `displayEpisodeByNameOrNumber()` :205 | `default_chapter_display_by_name_or_number` | normal | Long | 0x0 (NAME) | `display_episode_by_name_or_number` | **same key as the chapter twin** |
| 59 | `displayChapterByNameOrNumber()` :210 | `default_chapter_display_by_name_or_number` | normal | Long | 0x0 (NAME) | `display_chapter_by_name_or_number` | **same key as the episode twin** |
| 60 | `sortEpisodeByAscendingOrDescending()` :215 | `default_chapter_sort_by_ascending_or_descending` | normal | Long | 0x0 (DESC) | `sort_episode_by_ascending_or_descending` | **same key as the chapter twin** |
| 61 | `sortChapterByAscendingOrDescending()` :220 | `default_chapter_sort_by_ascending_or_descending` | normal | Long | 0x0 (DESC) | `sort_chapter_by_ascending_or_descending` | **same key as the episode twin** |
| 62 | `showEpisodeThumbnailPreviews()` :225 | `default_episode_show_thumbnail_previews` | normal | Long | 0x0 (SHOW) | `show_episode_thumbnail_previews` |  |
| 63 | `showEpisodeSummaries()` :230 | `default_episode_show_summaries` | normal | Long | 0x0 (SHOW) | `show_episode_summaries` |  |
| 64 | `filterSeasonByDownload()` :262 | `default_season_filter_by_downloaded` | normal | Long | 0x0 (SHOW_ALL) | `filter_season_by_download` |  |
| 65 | `filterSeasonByUnseen()` :265 | `default_season_filter_by_unseen` | normal | Long | 0x0 (SHOW_ALL) | `filter_season_by_unseen` |  |
| 66 | `filterSeasonByStarted()` :268 | `default_season_filter_by_started` | normal | Long | 0x0 (SHOW_ALL) | `filter_season_by_started` |  |
| 67 | `filterSeasonByCompleted()` :271 | `default_season_filter_by_completed` | normal | Long | 0x0 (SHOW_ALL) | `filter_season_by_completed` |  |
| 68 | `filterSeasonByBookmarked()` :274 | `default_season_filter_by_bookmarked` | normal | Long | 0x0 (SHOW_ALL) | `filter_season_by_bookmarked` |  |
| 69 | `filterSeasonByFillermarked()` :277 | `default_season_filter_by_fillermarked` | normal | Long | 0x0 (SHOW_ALL) | `filter_season_by_fillermarked` |  |
| 70 | `sortSeasonBySourceOrNumber()` :280 | `default_season_sort_by_source_or_number` | normal | Long | 0x0 (SOURCE) | `sort_season_by_source_or_number` |  |
| 71 | `sortSeasonByAscendingOrDescending()` :285 | `default_season_sort_by_ascending_or_descending` | normal | Long | 0x0 (DESC) | `sort_season_by_ascending_or_descending` |  |
| 72 | `seasonDisplayGridMode()` :290 | `default_season_grid_display_mode` | normal | Long | 0 (CompactGrid) | `season_display_grid_mode` | SeasonDisplayMode ordinal |
| 73 | `seasonDisplayGridSize()` :295 | `default_season_grid_display_size` | normal | Int | 0 | `season_display_grid_size` | 0..15 (4 bits) |
| 74 | `seasonDownloadOverlay()` :300 | `default_season_download_overlay` | normal | Boolean | false | `season_download_overlay` |  |
| 75 | `seasonUnseenOverlay()` :305 | `default_season_unseen_overlay` | normal | Boolean | true | `season_unseen_overlay` |  |
| 76 | `seasonLocalOverlay()` :310 | `default_season_local_overlay` | normal | Boolean | true | `season_local_overlay` |  |
| 77 | `seasonLangOverlay()` :315 | `default_season_lang_overlay` | normal | Boolean | false | `season_lang_overlay` |  |
| 78 | `seasonContinueOverlay()` :320 | `default_season_continue_overlay` | normal | Boolean | true | `season_continue_overlay` |  |
| 79 | `seasonDisplayMode()` :325 | `default_season_display_mode` | normal | Long | 0x0 (SOURCE) | `season_display_mode` |  |
| 80 | `updateSeasonOnRefresh()` :353 | `pref_update_season_on_refresh` | normal | Boolean | false | `update_season_on_refresh` |  |
| 81 | `updateSeasonOnLibraryUpdate()` :356 | `pref_update_season_on_library_update` | normal | Boolean | false | `update_season_on_library_update` |  |
| 82 | `swipeEpisodeStartAction()` :361 | `pref_episode_swipe_end_action` | normal | enum (by name) | `ToggleSeen` | `swipe_episode_start_action` | **key says `end`**; effective: swipe LEFT |
| 83 | `swipeEpisodeEndAction()` :364 | `pref_episode_swipe_start_action` | normal | enum (by name) | `ToggleBookmark` | `swipe_episode_end_action` | **key says `start`**; effective: swipe RIGHT |
| 84 | `swipeChapterStartAction()` :369 | `pref_chapter_swipe_end_action` | normal | enum (by name) | `ToggleRead` | `swipe_chapter_start_action` | **key says `end`**; effective: swipe LEFT |
| 85 | `swipeChapterEndAction()` :372 | `pref_chapter_swipe_start_action` | normal | enum (by name) | `ToggleBookmark` | `swipe_chapter_end_action` | **key says `start`**; effective: swipe RIGHT |
| 86 | `markDuplicateReadChapterAsRead()` :377 | `mark_duplicate_read_chapter_read` | normal | Set<String> | `{}` (empty) | `mark_duplicate_read_chapter_as_read` | values `existing`, `new` |
| 87 | `markDuplicateSeenEpisodeAsSeen()` :379 | `mark_duplicate_seen_episode_seen` | normal | Set<String> | `{}` (empty) | `mark_duplicate_seen_episode_as_seen` | values `existing_episode`, `new_episode` |

### 5.1 Constants (companion object)

`DEVICE_ONLY_ON_WIFI = "wifi"`, `DEVICE_NETWORK_NOT_METERED = "network_not_metered"`, `DEVICE_CHARGING = "ac"`; `ENTRY_NON_COMPLETED = "manga_ongoing"`, `ENTRY_HAS_UNVIEWED = "manga_fully_read"`, `ENTRY_NON_VIEWED = "manga_started"`, `ENTRY_OUTSIDE_RELEASE_PERIOD = "manga_outside_release_period"`; `MARK_DUPLICATE_CHAPTER_READ_NEW = "new"`, `MARK_DUPLICATE_CHAPTER_READ_EXISTING = "existing"`, `MARK_DUPLICATE_EPISODE_SEEN_NEW = "new_episode"`, `MARK_DUPLICATE_EPISODE_SEEN_EXISTING = "existing_episode"`; `DEFAULT_MANGA_CATEGORY_PREF_KEY = "default_category"`, `DEFAULT_ANIME_CATEGORY_PREF_KEY = "default_anime_category"`, `LIBRARY_UPDATE_MANGA_CATEGORIES_PREF_KEY = "library_update_categories"`, `LIBRARY_UPDATE_ANIME_CATEGORIES_PREF_KEY = "animelib_update_categories"`, `LIBRARY_UPDATE_MANGA_CATEGORIES_EXCLUDE_PREF_KEY = "library_update_categories_exclude"`, `LIBRARY_UPDATE_ANIME_CATEGORIES_EXCLUDE_PREF_KEY = "animelib_update_categories_exclude"`; `categoryPreferenceKeys` = those six keys (used by the backup restorer to know which sets hold category ids).

### 5.2 Related preferences owned by other classes but read/written by this area

| Key | Class (file) | Namespace | Type | Default | Used for |
|---|---|---|---|---|---|
| `pref_downloaded_only` | `BasePreferences.downloadedOnly()` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:16`) | app-state | Boolean | false | forces the library "Downloaded" filter to `ENABLED_IS` and disables it |
| `incognito_mode` | `BasePreferences.incognitoMode()` (`:21`) | app-state | Boolean | false | no history writes |
| `relative_time_v2` | `UiPreferences.relativeTime()` (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:33`) | normal | Boolean | true | "Today/Yesterday/N days ago" headers |
| `app_date_format` | `UiPreferences.dateFormat()` (`:35`) | normal | String | `""` (= locale SHORT date) | absolute date headers |
| `remove_exclude_categories`, `remove_exclude_anime_categories`, `download_new_categories`, `download_new_anime_categories`, `download_new_categories_exclude`, `download_new_anime_categories_exclude` | `DownloadPreferences` (`domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:39-90`) | normal | Set<String> | empty | category id sets cleaned on category delete and remapped on restore (owned by area:09) |
| `download_new` / `download_new_episode`, `download_new_unread_chapters_only` / `download_new_unread_episodes_only` | `DownloadPreferences` | normal | Boolean | false | not touched here (listed to avoid confusion with the category sets) |
| `library_update_categories*` | this class | | | | see section 5 rows 45-48 |

## 6. Rust data layout (Proposal)

```rust
pub type EpochMs = i64;
pub const SYSTEM_CATEGORY_ID: i64 = 0;
pub const UPDATES_LIMIT: i64 = 500;
pub const UPDATES_WINDOW_MONTHS: i64 = 3;
pub const SEARCH_DEBOUNCE_MS: u64 = 250;
```

Persisted layout is unchanged from Aniyomi: same two SQLite schemas, same preference keys and value encodings (so backups restore byte-for-byte equivalent state). Rust-only additions: none in this area's tables.
