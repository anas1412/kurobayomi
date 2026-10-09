# 38 part 2: data models, preferences, storage, and network

Paths are repo-relative to `/home/blackbox/aniyomi-src`. Nullability is Kotlin nullability. "Default" is the Kotlin default or the value used when the preference is unset.

This part makes no direct HTTP calls. Source calls (`getMangaDetails`, `getChapterList`) go through the source API (area:15-source-api-manga). Tracker calls go through tracker services (area:27 to 33). The tables below list what this part reads and writes.

---

## 1. Domain models

### 1.1 Manga (domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:10-35)

| Field | Kotlin type | Nullable | Notes |
|---|---|---|---|
| `id` | `Long` | no | `_id`; `-1` for `Manga.create()` |
| `source` | `Long` | no | source id; local source id is `LocalMangaSource.ID` |
| `favorite` | `Boolean` | no | library membership |
| `lastUpdate` | `Long` | no | epoch ms |
| `nextUpdate` | `Long` | no | epoch ms |
| `fetchInterval` | `Int` | no | negative = custom interval (days, stored as `-days`); `0` = automatic |
| `dateAdded` | `Long` | no | epoch ms; `0` when not favorite |
| `viewerFlags` | `Long` | no | reader flags (other area) |
| `chapterFlags` | `Long` | no | bit layout in section 3 |
| `coverLastModified` | `Long` | no | epoch ms |
| `url` | `String` | no | source URL |
| `title` | `String` | no | |
| `artist` | `String?` | yes | |
| `author` | `String?` | yes | |
| `description` | `String?` | yes | |
| `genre` | `List<String>?` | yes | DB column `genre TEXT AS List<String>` |
| `status` | `Long` | no | `SManga` status int stored as Long |
| `thumbnailUrl` | `String?` | yes | |
| `updateStrategy` | `UpdateStrategy` | no | enum from `eu.kanade.tachiyomi.source.model.UpdateStrategy`; DB default `0` |
| `initialized` | `Boolean` | no | `false` until details fetched |
| `lastModifiedAt` | `Long` | no | |
| `favoriteModifiedAt` | `Long?` | yes | |
| `version` | `Long` | no | |

Derived: `unreadFilter`, `bookmarkedFilter` (TriState), `sorting`, `displayMode`, `sortDescending()`, `expectedNextUpdate` (not used here). Extension in app: `downloadedFilter` (forced ENABLED_IS when downloadedOnly), `chaptersFiltered()`, `readingMode`, `readerOrientation`.

### 1.2 Chapter (domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt:3-18)

| Field | Type | Nullable | Default in `Chapter.create()` | Notes |
|---|---|---|---|---|
| `id` | `Long` | no | `-1` | `_id` |
| `mangaId` | `Long` | no | `-1` | |
| `read` | `Boolean` | no | `false` | |
| `bookmark` | `Boolean` | no | `false` | |
| `lastPageRead` | `Long` | no | `0` | |
| `dateFetch` | `Long` | no | `0` | |
| `sourceOrder` | `Long` | no | `0` | index in source list |
| `url` | `String` | no | `""` | |
| `name` | `String` | no | `""` | sanitised on sync |
| `dateUpload` | `Long` | no | `-1` | epoch ms |
| `chapterNumber` | `Double` | no | `-1.0` | `-1.0` = unrecognised |
| `scanlator` | `String?` | yes | `null` | blank is stored as `null` (see 4.4) |
| `lastModifiedAt` | `Long` | no | `0` | |
| `version` | `Long` | no | `1` | |

Derived: `isRecognizedNumber: Boolean = chapterNumber >= 0f`.

### 1.3 ChapterUpdate (domain/src/main/java/tachiyomi/domain/items/chapter/model/ChapterUpdate.kt)

All fields nullable; `null` = "do not change". Fields: `id: Long`, `mangaId: Long?`, `read: Boolean?`, `bookmark: Boolean?`, `lastPageRead: Long?`, `dateFetch: Long?`, `sourceOrder: Long?`, `url: String?`, `name: String?`, `dateUpload: Long?`, `chapterNumber: Double?`, `scanlator: String?`, `version: Long?`.

Used here: `read` + `lastPageRead` (SetReadStatus), `bookmark` (bookmarkChapters).

### 1.4 MangaUpdate (domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaUpdate.kt)

All fields nullable except `id: Long`. Used here: `favorite`, `dateAdded`, `chapterFlags`, `fetchInterval`, `nextUpdate` (via MangaFetchInterval), `coverLastModified`, `lastUpdate`, `initialized`, `title`, `author`, `artist`, `description`, `genre`, `status`, `thumbnailUrl`, `updateStrategy` (from source fetch).

### 1.5 MangaTrack (domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt:5-20)

| Field | Type | Nullable |
|---|---|---|
| `id` | `Long` | no |
| `mangaId` | `Long` | no |
| `trackerId` | `Long` | no |
| `remoteId` | `Long` | no |
| `libraryId` | `Long?` | yes |
| `title` | `String` | no |
| `lastChapterRead` | `Double` | no |
| `totalChapters` | `Long` | no |
| `status` | `Long` | no |
| `score` | `Double` | no |
| `remoteUrl` | `String` | no |
| `startDate` | `Long` | no (epoch ms, `0` = unset) |
| `finishDate` | `Long` | no (epoch ms, `0` = unset) |
| `private` | `Boolean` | no |

DB table `manga_sync` (data/src/main/sqldelight/data/manga_sync.sq): columns `_id`, `manga_id` (FK mangas ON DELETE CASCADE), `sync_id` (tracker id), `remote_id`, `library_id`, `title`, `last_chapter_read REAL`, `total_chapters INTEGER`, `status INTEGER`, `score REAL`, `remote_url`, `start_date`, `finish_date`, `private INTEGER DEFAULT 0 NOT NULL`, `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`.

### 1.6 MangaTrackSearch (app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt)

Legacy (non-domain) class. Fields with defaults: `id: Long?` = null, `manga_id: Long` = 0, `tracker_id: Long` = 0, `remote_id: Long` = 0, `library_id: Long?` = null, `title: String` (lateinit), `last_chapter_read: Double` = 0.0, `total_chapters: Long` = 0, `score: Double` = -1.0, `status: Long` = 0, `started_reading_date: Long` = 0, `finished_reading_date: Long` = 0, `private: Boolean` = false, `tracking_url: String` (lateinit), `authors: List<String>` = empty, `artists: List<String>` = empty, `cover_url: String` = "", `summary: String` = "", `publishing_status: String` = "", `publishing_type: String` = "", `start_date: String` = "". Equality and hash: `manga_id`, `tracker_id`, `remote_id` only.

Rust: `TrackSearchResult { tracker_id, remote_id, title, tracking_url, cover_url, summary, authors, artists, publishing_status, publishing_type, start_date, total_chapters, score, private }` with `remote_id`+`tracker_id` as identity.

### 1.7 Category (domain/src/main/java/tachiyomi/domain/category/model/Category.kt:5-18)

`id: Long`, `name: String`, `order: Long`, `flags: Long`, `hidden: Boolean`. Derived `isSystemCategory = id == 0L` (`UNCATEGORIZED_ID`). Table `categories(_id, name, sort, flags, hidden INTEGER NOT NULL DEFAULT 0)`. Membership table `mangas_categories(_id, manga_id, category_id)`, FKs ON DELETE CASCADE.

### 1.8 MangaDownload (UI-visible, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt)

Not persisted. `source: HttpSource`, `manga: Manga`, `chapter: Chapter`, `pages: List<Page>?` (null until resolved), `status: State` (default `NOT_DOWNLOADED`), `progress: Int` = `pages.map(progress).average().toInt()`, or 0 when pages are null. `downloadedImages` = count of pages with state READY. Enum `State(value)`: NOT_DOWNLOADED 0, QUEUE 1, DOWNLOADING 2, DOWNLOADED 3, ERROR 4.

### 1.9 UI-only models in this part

- `ChapterList.Item` (MSM:1234-1242): `chapter: Chapter`, `downloadState: MangaDownload.State`, `downloadProgress: Int`, `selected: Boolean` = false. Derived `id = chapter.id`, `isDownloaded = downloadState == DOWNLOADED`.
- `ChapterList.MissingCount` (MSM:1228-1231): `id: String` (`"${lowerId}-${higherId}"`), `count: Int`.
- `State.Success` (MSM:1144-1156): `manga`, `source: MangaSource`, `isFromSource: Boolean`, `chapters: List<ChapterList.Item>`, `availableScanlators: Set<String>`, `excludedScanlators: Set<String>`, `trackingCount: Int` = 0, `hasLoggedInTrackers: Boolean` = false, `isRefreshingData: Boolean` = false, `dialog: Dialog?` = null, `hasPromptedToAddBefore: Boolean` = false.
- `State.Loading` (data object).
- `MangaTrackItem(track: MangaTrack?, tracker: Tracker)` (MangaTrackItem.kt).
- `MTID.State(trackItems: List<MangaTrackItem>)` = empty default.
- `TrackStatusSelectorScreen.State(selection: Long)`, `TrackChapterSelectorScreen.State(selection: Int)`, `TrackScoreSelectorScreen.State(selection: String)`, `TrackServiceSearchScreen.State(queryResult: Result<List<MangaTrackSearch>>? = null, selected: MangaTrackSearch? = null)`.

---

## 2. Enums and stored values

| Enum | Kotlin definition | Stored value |
|---|---|---|
| `TriState` | `DISABLED, ENABLED_IS, ENABLED_NOT` (core/common/.../TriState.kt) | not stored directly; mapped to flag bits (section 3) |
| `LibraryPreferences.ChapterSwipeAction` | `ToggleRead, ToggleBookmark, Download, Disabled` (LibraryPreferences.kt:391-396) | persisted via `getEnum` (name string, see 4.1) |
| `AutoTrackState` | `ALWAYS, ASK, NEVER` (AutoTrackState.kt) | persisted via `getEnum` (name string) |
| `MangaDownload.State` | `NOT_DOWNLOADED=0, QUEUE=1, DOWNLOADING=2, DOWNLOADED=3, ERROR=4` | not persisted |
| `ChapterDownloadAction` | `START, START_NOW, CANCEL, DELETE` | not persisted |
| `DownloadAction` | `NEXT_1_ITEM, NEXT_5_ITEMS, NEXT_10_ITEMS, NEXT_25_ITEMS, UNVIEWED_ITEMS` | not persisted |
| `Manga.UpdateStrategy` (source) | `UpdateStrategy` enum in `eu.kanade.tachiyomi.source.model` | stored as integer (`update_strategy`, default 0) |

`getEnum` persistence: `preferenceStore.getEnum(key, default)` stores the enum `name` string (standard Android preference store; the Rust port uses the same name strings so an export/import round-trips).

---

## 3. Bit layouts (chapter_flags, INTEGER, 64-bit Long)

```
bit:  31 .......... 21 20 19 .......... 8 7 6 5 4 3 2 1 0
                          |  |     |     | | | | | | | | +-- 0x001  sort direction (0=DESC,1=ASC)
                          |  |     |     | | | | | | | +---- 0x002  unread=is   \  unread mask 0x006
                          |  |     |     | | | | | | +------ 0x004  unread=not  /
                          |  |     |     | | | | | +-------- 0x008  downloaded=is \ downloaded mask 0x018
                          |  |     |     | | | | +---------- 0x010  downloaded=not /
                          |  |     |     | | | +------------ 0x020  bookmarked=is  \ bookmarked mask 0x060
                          |  |     |     | | +-------------- 0x040  bookmarked=not /
                          |  |     |     +-+---------------- 0x300  sorting (0x000 source, 0x100 number, 0x200 upload date, 0x300 alphabet)
                          |  +------------------------------- 0x100000 display (0=name, 1=number)
```

Mask table:

| Name | Value |
|---|---|
| `CHAPTER_SORT_DIR_MASK` | `0x00000001` |
| `CHAPTER_UNREAD_MASK` | `0x00000006` |
| `CHAPTER_DOWNLOADED_MASK` | `0x00000018` |
| `CHAPTER_BOOKMARKED_MASK` | `0x00000060` |
| `CHAPTER_SORTING_MASK` | `0x00000300` |
| `CHAPTER_DISPLAY_MASK` | `0x00100000` |

Bits 9 to 19 and 21 to 63 are unused and must be preserved on write (`setFlag` keeps bits outside the mask).

Write rule (SetMangaChapterFlags.kt:94-96): `new = (old and mask.inv()) or (flag and mask)`.

Read rule: each filter reads only its own mask. Any value that is not in the table (for example `0x006` both set) maps to DISABLED for unread (see Manga.kt:57-62, `else -> DISABLED`).

Default for a new manga: `0` (DESC, SOURCE, no filters, NAME).

---

## 4. Preference keys, defaults, and where they are used

Keys are the literal strings passed to `preferenceStore.get*(...)`. The app uses an Android SharedPreferences-backed store; the Rust port maps each key to a row in its settings table (area:02). Exact key strings are required because they are used in backups (area:21/22).

### 4.1 LibraryPreferences (domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt)

| Key | Type | Default | Read by | Line |
|---|---|---|---|---|
| `default_chapter_filter_by_read` | Long | `Manga.SHOW_ALL` (0) | SetMangaDefaultChapterFlags, setChapterSettingsDefault | 176-177 |
| `default_chapter_filter_by_downloaded` | Long | `0` | same | 182-183 |
| `default_chapter_filter_by_bookmarked` | Long | `0` | same | 188-189 |
| `default_chapter_sort_by_source_or_number` | Long | `Manga.CHAPTER_SORTING_SOURCE` (0) | same | 200-203 |
| `default_chapter_display_by_name_or_number` | Long | `Manga.CHAPTER_DISPLAY_NAME` (0) | same | 210-213 |
| `default_chapter_sort_by_ascending_or_descending` | Long | `Manga.CHAPTER_SORT_DESC` (0) | same | 220-223 |
| `default_chapter_sort_by_ascending_or_descending` | Long | `Anime.EPISODE_SORT_DESC` (0) | anime side `sortEpisodeByAscendingOrDescending` | 215-218 |
| `pref_chapter_swipe_end_action` | enum name | `ToggleRead` | `swipeChapterStartAction()` | 369-370 |
| `pref_chapter_swipe_start_action` | enum name | `ToggleBookmark` | `swipeChapterEndAction()` | 372-375 |
| `default_category` | Int | `-1` | toggleFavorite routing | 154, companion 413 |
| `library_update_manga_restriction` | StringSet | `{manga_fully_read, manga_ongoing, manga_started, manga_outside_release_period}` | `isUpdateIntervalEnabled` (via ENTRY_OUTSIDE_RELEASE_PERIOD) | 49-56 |

Constants (companion, LibraryPreferences.kt:402-406):
- `ENTRY_NON_COMPLETED = "manga_ongoing"`
- `ENTRY_HAS_UNVIEWED = "manga_fully_read"`
- `ENTRY_NON_VIEWED = "manga_started"`
- `ENTRY_OUTSIDE_RELEASE_PERIOD = "manga_outside_release_period"`

OBSERVED DEFECT (key collision): `sortEpisodeByAscendingOrDescending()` (line 215) and `sortChapterByAscendingOrDescending()` (line 220) both use the key `default_chapter_sort_by_ascending_or_descending`. The anime and manga "default sort direction" are the same stored value. Both defaults are `0` today, so there is no visible effect yet, but changing one changes the other. Proposal: the Rust port uses `default_episode_sort_by_ascending_or_descending` for anime. This is a deliberate deviation. It is recorded here because backups carry this key. See E-DEF-1.

### 4.2 TrackPreferences (app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt)

| Key | Type | Default | Used here |
|---|---|---|---|
| `pref_auto_update_manga_on_mark_read` | enum name (AutoTrackState) | `ALWAYS` | `autoTrackState` (MSM:146) |
| `track_on_adding_to_library` | Boolean | `true` | `autoOpenTrack` (MSM:159) |
| `pref_auto_update_manga_sync_key` | Boolean | `true` | not used in this part (background sync, area:26/27) |

Other keys in the same file (not used here): `anilist_score_type` (`Anilist.POINT_10`), `track_token_${id}`, per-tracker username/password keys.

### 4.3 DownloadPreferences (domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt)

| Key | Type | Default | Used here | Line |
|---|---|---|---|---|
| `pref_remove_after_marked_as_read_key` | Boolean | `false` | SetReadStatus (remove download after read) | 30-33 |
| `download_new` | Boolean | `false` | FilterChaptersForDownload | 48 |
| `download_new_categories` | StringSet (category ids as strings) | `{}` | FilterChaptersForDownload (include) | 51-55, const 78 |
| `download_new_categories_exclude` | StringSet | `{}` | FilterChaptersForDownload (exclude) | 60-62, const 80 |
| `download_new_unread_chapters_only` | Boolean | `false` | FilterChaptersForDownload | 72 |

### 4.4 ReaderPreferences (app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:85)

| Key | Type | Default | Used here |
|---|---|---|---|
| `skip_filtered` | Boolean | `true` | `getUnreadChapters()` source list (MSM:654) |

### 4.5 BasePreferences (app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:16-19)

| Key | Type | Default | Used here |
|---|---|---|---|
| `Preference.appStateKey("pref_downloaded_only")` | Boolean | `false` | forces downloadedFilter to ENABLED_IS (Manga.kt:25) |

`Preference.appStateKey(x)` marks the key as app state (not a user setting). It is excluded from the settings screen and kept in backups only when the backup includes app state (area:21).

### 4.6 UiPreferences (app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:35)

| Key | Type | Default | Used by |
|---|---|---|---|
| `app_date_format` | String | `""` | `UiPreferences.dateFormat(...)` for track date display (MTID:103-107). Empty string = system locale default. |

### 4.7 Keys used in part 1 of this area (listed for completeness, not handled here)

None directly. Part 1 reads the same chapter-flag keys through `LibraryPreferences` for its settings sheet.

### 4.8 Summary of stored-state side effects

| Action | Writes |
|---|---|
| Set unread/downloaded/bookmark filter | `mangas.chapter_flags` |
| Set sort or display | `mangas.chapter_flags` |
| Set chapter settings as default | 6 keys above (`default_chapter_*`) |
| Set current settings as default + apply to existing | 6 keys + `mangas.chapter_flags` on every favorite |
| Non-favorite open | `mangas.chapter_flags` (overwritten by defaults) |
| Toggle favorite | `mangas.favorite`, `mangas.date_added`, `mangas.cover_last_modified` (on remove), `mangas_categories` |
| Set fetch interval | `mangas.fetch_interval`, `mangas.next_update` (computed) |
| Mark read/unread | `chapters.read`, `chapters.last_page_read` |
| Bookmark | `chapters.bookmark` |
| Excluded scanlators | `excluded_scanlators` rows |
| Tracker status/score/dates/private | tracker service + `manga_sync` row |
| Tracker unlink | `manga_sync` row delete |
| Download actions | queue (memory), download directories on disk |
| Delete downloads | download directories on disk, cache index |

---

## 5. Database tables touched

### 5.1 `mangas` (data/src/main/sqldelight/data/mangas.sq)

Columns: `_id INTEGER PK`, `source INTEGER NOT NULL`, `url TEXT NOT NULL`, `artist TEXT`, `author TEXT`, `description TEXT`, `genre TEXT AS List<String>`, `title TEXT NOT NULL`, `status INTEGER NOT NULL`, `thumbnail_url TEXT`, `favorite INTEGER AS Boolean NOT NULL`, `last_update INTEGER`, `next_update INTEGER`, `initialized INTEGER AS Boolean NOT NULL`, `viewer INTEGER NOT NULL`, `chapter_flags INTEGER NOT NULL`, `cover_last_modified INTEGER NOT NULL`, `date_added INTEGER NOT NULL`, `update_strategy INTEGER AS UpdateStrategy NOT NULL DEFAULT 0`, `calculate_interval INTEGER DEFAULT 0 NOT NULL`, `last_modified_at INTEGER NOT NULL DEFAULT 0`, `favorite_modified_at INTEGER`, `version INTEGER NOT NULL DEFAULT 0`, `is_syncing INTEGER NOT NULL DEFAULT 0`.

Indexes: `library_favorite_index ON mangas(favorite) WHERE favorite = 1`, `mangas_url_index ON mangas(url)`.

Query used: `getDuplicateLibraryManga` (mangas.sq:117-122):

```sql
SELECT * FROM mangas
WHERE favorite = 1
AND LOWER(title) = :title
AND _id != :id;
```

`:title` is passed as `manga.title.lowercase()` (Kotlin, locale-invariant lowercase). SQLite `LOWER()` only folds ASCII, so non-ASCII titles can miss duplicates in SQL while the Kotlin side lowercased them. OBSERVED. Proposal: Rust compares `title.to_lowercase()` on both sides, by a column `title_key` or a SQLite collation. See E-DUP-1.

Mutation of flags: `mangas.sq:172` `chapter_flags = coalesce(:chapterFlags, chapter_flags)` (update path). `MangaUpdate.chapterFlags == null` leaves the column unchanged.

### 5.2 `chapters` (data/src/main/sqldelight/data/chapters.sq)

Columns: `_id INTEGER PK`, `manga_id INTEGER NOT NULL FK mangas ON DELETE CASCADE`, `url TEXT NOT NULL`, `name TEXT NOT NULL`, `scanlator TEXT` (nullable), `read INTEGER AS Boolean NOT NULL`, `bookmark INTEGER AS Boolean NOT NULL`, `last_page_read INTEGER NOT NULL`, `chapter_number REAL NOT NULL`, `source_order INTEGER NOT NULL`, `date_fetch INTEGER NOT NULL`, `date_upload INTEGER NOT NULL`, `last_modified_at INTEGER NOT NULL DEFAULT 0`, `version INTEGER NOT NULL DEFAULT 0`, `is_syncing INTEGER NOT NULL DEFAULT 0`.

Indexes: `chapters_manga_id_index ON chapters(manga_id)`, `chapters_unread_by_manga_index ON chapters(manga_id, read) WHERE read = 0`.

Triggers (chapters.sq around line 40-50): on update, `chapters.version = version + 1`, and `mangas.version = version + 1` for the parent manga when `mangas.is_syncing = 0`. Port: keep the version counter semantics for backup/sync (area:21).

Query `getChaptersByMangaId` (chapters.sq:56-66):

```sql
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

Semantics: when `applyScanlatorFilter` is true, rows whose scanlator is excluded for this manga are removed. A row with `scanlator IS NULL` never matches the join (`NULL = x` is NULL), so it is never excluded. Result order: no `ORDER BY`. The Kotlin side applies sorting. Rust must not rely on DB order.

Query `getScanlatorsByMangaId` (chapters.sq:68-71): `SELECT scanlator FROM chapters WHERE manga_id = :mangaId;` (includes NULLs and duplicates; Kotlin deduplicates after `mapNotNull { ifBlank{null} }`).

Chapter numbers are `REAL`. Comparisons in SQL are not used for sorting here; sorting is in Kotlin (`getChapterSort`). Rust: keep the same comparator in Rust to match.

### 5.3 `excluded_scanlators` (data/src/main/sqldelight/data/excluded_scanlators.sq)

Columns: `manga_id INTEGER NOT NULL FK mangas ON DELETE CASCADE`, `scanlator TEXT NOT NULL`. Index `excluded_scanlators_manga_id_index ON excluded_scanlators(manga_id)`. No primary key and no uniqueness constraint: duplicates are prevented only by `SetExcludedScanlators` (which diffs in a transaction). Proposal: add `PRIMARY KEY (manga_id, scanlator)` in the Rust schema (the migration area owns the upgrade, area:03).

Statements: `insert` (`INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (:mangaId, :scanlator)`), `remove` (`DELETE FROM excluded_scanlators WHERE manga_id = :mangaId AND scanlator IN :scanlators`), `getExcludedScanlatorsByMangaId`.

### 5.4 `manga_sync` (data/src/main/sqldelight/data/manga_sync.sq)

See 1.5. `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`: `insert` replaces the existing link for the same tracker. `private INTEGER DEFAULT 0 NOT NULL` (Boolean).

### 5.5 `categories` and `mangas_categories`

`categories(_id, name TEXT NOT NULL, sort INTEGER NOT NULL, flags INTEGER NOT NULL, hidden INTEGER NOT NULL DEFAULT 0)`. `mangas_categories(_id, manga_id, category_id)` with FKs to both, ON DELETE CASCADE. Category id `0` is the virtual Uncategorized/Default and is not a row in the `categories` table in normal use. (Verify in area:08.)

### 5.6 Transaction boundaries used here

| Operation | Transaction |
|---|---|
| `SetExcludedScanlators.await` | one transaction (`handler.await(inTransaction = true)`) |
| Set category + set favorite (in `moveMangaToCategoriesAndAddToLibrary`) | two separate writes (no transaction, see behavior 4.4) |
| Set read status (many chapters) | `updateAllChapters` (one batch) |
| Bookmark (many chapters) | `updateChapter.awaitAll` (one batch) |
| Filter flags | one `updateManga` per call |

---

## 6. Files and folders on disk

This part does not write files directly. It triggers:

| Trigger | Effect on disk | Owner |
|---|---|---|
| Remove from library (toggleFavorite off) | cover cache deletion for the manga (`coverCache.deleteFromCache(manga, true)`) | area:34 |
| Delete downloads (snackbar action, or `deleteChapters`) | removes chapter directories inside the manga download directory; removes manga dir if empty; removes source dir if empty | area:23/25 |
| Mark read with `pref_remove_after_marked_as_read_key` on | `deleteDownload.awaitAll` per manga group | area:23/25 |
| Download actions | enqueue; directories are created by the downloader | area:23 |
| Manual refresh (`syncChaptersWithSource` with a rename) | `downloadManager.renameChapter` when the chapter directory name would change and the chapter is downloaded | area:07/23 |

Download directory layout is owned by area:23. Chapter directory names are derived from `name`, `scanlator`, and the manga title (`isChapterDownloaded(name, scanlator, mangaTitle, sourceId)`). Port rule (from the cross-platform addendum): the name must go through the one sanitizing function (NTFS-safe), and matching must respect case-insensitivity where the platform needs it.

Folder-related Paths used by the Rust port (from `kuro-platform::Paths`): downloads default dir, covers dir, data dir (DB). This part only consumes them.

---

## 7. Network endpoints

No endpoints are defined in this part. The calls made through other areas:

| Call | Through | Method/params owner |
|---|---|---|
| `source.getMangaDetails(SManga)` | `MangaSource` (HttpSource in the common case) | area:15 |
| `source.getChapterList(SManga)` | `MangaSource` | area:15 |
| tracker `searchManga(query)` | `tracker.mangaService` | area:27-33 |
| tracker `refresh(track)`, `update(track, true)`, `register(item, mangaId)`, `setRemote*` | tracker service | area:27-33 |
| tracker `delete(track)` | `DeletableMangaTracker` | area:27-33 |
| enhanced tracker `match(manga)` | `EnhancedMangaTracker` | area:27-33 |

Relevant contract shape from this part: `SManga` fields used are `url`, `title`, `artist`, `author`, `description`, `genre` (comma-joined string on the way out, parsed back by `SManga.getGenres()`), `status`, `thumbnail_url`, `initialized`, `update_strategy` (from `updateManga` path), and for chapters `url`, `name`, `date_upload`, `chapter_number`, `scanlator` (blank -> null). Area:15 owns the exact wire format.

HTTP status special case used here: `HttpException` with `code == 103` is ignored (behavior 3.3).

---

## 8. Error types surfaced in this part

| Type | Module | Thrown by | User message (`formattedMessage`) |
|---|---|---|---|
| `NoChaptersException` | domain/src/main/java/tachiyomi/domain/items/chapter/model | sync when source returns no chapters (non-local) | `no_results_found` (via formatter) AND, for this path only, `no_chapters_error` ("No chapters found") |
| `HttpException(code)` | eu.kanade.tachiyomi.network | network | `exception_http` with code |
| `UnknownHostException` | java.net | DNS | `exception_offline` if offline, else `exception_unknown_host` with message |
| `SourceNotInstalledException` | domain/src/main/java/tachiyomi/domain/source/manga/model | source missing | `loader_not_implemented_error` |
| any other `Exception` | | | `"$className: $message"` or message |

The Rust error enum for this part (proposal in p2-rust-design.md) maps 1:1 onto these.

---

## 9. Rust data model (summary, details in p2-rust-design.md)

- `Manga`, `Chapter`, `ChapterUpdate`, `MangaUpdate`, `MangaTrack`, `Category`: plain structs, `Option<T>` for nullable fields, `i64` for `Long`, `f64` for `Double`, `i32` for `Int`, `bool` for `Boolean`.
- Flags: newtype `ChapterFlags(u64)` with const masks and typed getters/setters (section 3). Stored as `INTEGER` (`i64` cast).
- Enums: `#[repr(i64)]` where a stored int exists; `as_str()`/`FromStr` with the exact name strings where an enum name is persisted (section 2).
- `ChapterList.Item` -> `ChapterRow { chapter, download: DownloadState, progress: u8, selected: bool }`.
- `ChapterSeparator` enum: `Missing { id: String, count: u32 }` (see behavior 6.7).
