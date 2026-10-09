# Area 42 part 1 — data models, preferences, storage, network, DB

All paths repo-relative to `/home/blackbox/aniyomi-src`. Nullability follows Kotlin (`?` = nullable). Rust types are the proposed mapping (see p1-rust-design.md section 3).

Network: **none**. No file in part 1 issues an HTTP request. The only remote-facing action is the library refresh (`MangaLibraryUpdateJob.startNow`, `AnimeLibraryUpdateJob.startNow`, area:26-library-update-jobs), which is triggered from the Updates pull gesture.

Files on disk: **none written directly** by part 1. Storage reads sizes of download directories through `MangaDownloadCache` / `AnimeDownloadCache` (area:23-downloads-manga, area:24-downloads-anime) and deletes through `deleteManga`/`deleteAnime`/`deleteChapters`/`deleteEpisodes` (area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared). Local-source entries are measured with `LocalMangaSourceFileSystem` / `LocalAnimeSourceFileSystem` (area:14-source-local-and-metadata).

---

## 1. Domain models read by this area

### 1.1 History

`domain/src/main/java/tachiyomi/domain/history/manga/model/MangaHistoryWithRelations.kt`:

| Field | Kotlin type | Nullable | Source column / expression | Rust |
|---|---|---|---|---|
| `id` | `Long` | no | `historyView.id` = `history._id` | `i64` |
| `chapterId` | `Long` | no | `chapters._id` | `i64` |
| `mangaId` | `Long` | no | `mangas._id` | `i64` |
| `title` | `String` | no | `mangas.title` | `String` |
| `chapterNumber` | `Double` | no | `chapters.chapter_number` (`-1.0` = no number; `> -1` gates the "Ch. N" subtitle) | `f64` |
| `readAt` | `java.util.Date?` | yes | `history.last_read` (`AS Date`, epoch ms) | `Option<DateTime<Utc>>` (store `i64` ms) |
| `readDuration` | `Long` | no | `history.time_read` (ms) | `i64` |
| `coverData` | `MangaCover` | no | see 1.4 | `MangaCover` |

Anime `AnimeHistoryWithRelations` (`domain/src/main/java/tachiyomi/domain/history/anime/model/AnimeHistoryWithRelations.kt`): `id: Long`, `episodeId: Long`, `animeId: Long`, `title: String`, `episodeNumber: Double`, `seenAt: Date?`, `coverData: AnimeCover`. No duration field (the `animehistory` table has none).

`MangaHistory` (`.../history/manga/model/MangaHistory.kt`): `id`, `chapterId`, `readAt: Date?`, `readDuration: Long` (used by other areas).

Write models (not read here; listed because the clear/remove actions write them):
- `MangaHistoryUpdate(chapterId: Long, readAt: Date, sessionReadDuration: Long)` (`domain/.../history/manga/model/MangaHistoryUpdate.kt`).
- `AnimeHistoryUpdate(episodeId: Long, seenAt: Date)` (`domain/.../history/anime/model/AnimeHistoryUpdate.kt`).

### 1.2 Updates

`domain/src/main/java/tachiyomi/domain/updates/manga/model/MangaUpdatesWithRelations.kt`:

| Field | Type | Source |
|---|---|---|
| `mangaId` | `Long` | `mangas._id` |
| `mangaTitle` | `String` | `mangas.title` |
| `chapterId` | `Long` | `chapters._id` |
| `chapterName` | `String` | `chapters.name` |
| `scanlator` | `String?` | `chapters.scanlator` |
| `read` | `Boolean` | `chapters.read` |
| `bookmark` | `Boolean` | `chapters.bookmark` |
| `lastPageRead` | `Long` | `chapters.last_page_read` (0-based page index; UI shows `+1`) |
| `sourceId` | `Long` | `mangas.source` |
| `dateFetch` | `Long` | `chapters.date_fetch` (epoch ms) — grouping key |
| `coverData` | `MangaCover` | `mangas.favorite`, `thumbnail_url`, `cover_last_modified` |

Not carried into the model: `chapters.date_upload` (used only in the SQL `WHERE` window).

`domain/src/main/java/tachiyomi/domain/updates/anime/model/AnimeUpdatesWithRelations.kt`:
`animeId: Long`, `animeTitle: String`, `episodeId: Long`, `episodeName: String`, `scanlator: String?`, `seen: Boolean`, `bookmark: Boolean`, `fillermark: Boolean`, `lastSecondSeen: Long`, `totalSeconds: Long`, `sourceId: Long`, `dateFetch: Long`, `coverData: AnimeCover`.

Note: `totalSeconds` and `lastSecondSeen` are milliseconds (the UI formats them with ms thresholds, `AnimeUpdatesUiItem.kt:265-283`). Keep the name of the DB column (`total_seconds`) only as a legacy label; the Rust field is `total_ms`/`last_ms` as proposal (rename is safe because the Rust database is new; see p1-rust-design.md section 4).

### 1.3 Covers

`MangaCover(mangaId: Long, sourceId: Long, isMangaFavorite: Boolean, url: String?, lastModified: Long)` (domain entries model package `tachiyomi.domain.entries.manga.model`; constructed at `data/src/main/java/tachiyomi/data/history/manga/MangaHistoryMapper.kt:33-40`). `AnimeCover(animeId: Long, sourceId: Long, isAnimeFavorite: Boolean, url: String?, lastModified: Long)` (used in `AnimeHistoryWithRelationsProvider.kt:16-22`).

### 1.4 Library rows (read by stats and storage)

`domain/src/main/java/tachiyomi/domain/library/manga/LibraryManga.kt`:
- `manga: Manga`, `category: Long`, `totalChapters: Long`, `readCount: Long`, `bookmarkCount: Long`, `latestUpload: Long`, `chapterFetchedAt: Long`, `lastRead: Long`.
- Derived: `id = manga.id`; `unreadCount = totalChapters - readCount`; `hasBookmarks = bookmarkCount > 0`; `hasStarted = readCount > 0`.

`domain/src/main/java/tachiyomi/domain/library/anime/LibraryAnime.kt`:
- `anime: Anime`, `category: Long`, `totalCount: Long`, `seenCount: Long`, `bookmarkCount: Long`, `fillermarkCount: Long`, `latestUpload: Long`, `episodeFetchedAt: Long`, `lastSeen: Long`.
- Derived: `id`, `unseenCount = totalCount - seenCount`, `hasBookmarks`, `hasStarted = seenCount > 0`.

`Manga.status` / `Anime.status` are `Long` (`.toInt()` compared to `SManga.COMPLETED = 2` / `SAnime.COMPLETED = 2`; `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:50`, `.../animesource/model/SAnime.kt:74`).

### 1.5 Categories

`domain/src/main/java/tachiyomi/domain/category/model/Category.kt`: `id: Long`, `name: String`, `order: Long`, `flags: Long`, `hidden: Boolean`; `isSystemCategory: Boolean = id == UNCATEGORIZED_ID`; `const val UNCATEGORIZED_ID = 0L` (`Category.kt:13-16`). Dummy "All" category used by storage: `id = -1L`, `name = "All"`, `order = 0L`, `flags = 0L`, `hidden = false` (`CommonStorageScreenModel.kt:114-120`).

### 1.6 Storage item (presentation model)

`StorageItem(id: Long, title: String, size: Long, thumbnail: String?, entriesCount: Int, color: androidx.compose.ui.graphics.Color)` — `app/src/main/java/eu/kanade/presentation/more/storage/StorageItem.kt:42-49`. Rust: `StorageItem { id: i64, title: String, size_bytes: u64, thumbnail_url: Option<String>, entries_count: u32, color: Rgb8 }` with `Rgb8 = [u8; 3]` (alpha = 255). `size` is `Long` bytes.

### 1.7 Stats payloads (`app/src/main/java/eu/kanade/presentation/more/stats/data/StatsData.kt`)

| Type | Fields (Kotlin type) |
|---|---|
| `MangaOverview` | `libraryMangaCount: Int`, `completedMangaCount: Int`, `totalReadDuration: Long` (ms) |
| `AnimeOverview` | `libraryAnimeCount: Int`, `completedAnimeCount: Int`, `totalSeenDuration: Long` (ms) |
| `MangaTitles` | `globalUpdateItemCount: Int`, `startedMangaCount: Int`, `localMangaCount: Int` |
| `AnimeTitles` | `globalUpdateItemCount: Int`, `startedAnimeCount: Int`, `localAnimeCount: Int` |
| `Chapters` | `totalChapterCount: Int`, `readChapterCount: Int`, `downloadCount: Int` |
| `Episodes` | `totalEpisodeCount: Int`, `readEpisodeCount: Int`, `downloadCount: Int` |
| `Trackers` | `trackedTitleCount: Int`, `meanScore: Double` (NaN = none), `trackerCount: Int` |

`StatsScreenState` (`app/src/main/java/eu/kanade/presentation/more/stats/StatsScreenState.kt`): `Loading` (data object), `SuccessManga(overview: MangaOverview, titles: MangaTitles, chapters: Chapters, trackers: Trackers)`, `SuccessAnime(overview: AnimeOverview, titles: AnimeTitles, episodes: Episodes, trackers: Trackers)`.

Rust: `enum StatsState { Loading, Manga(MangaStats), Anime(AnimeStats) }`; `Trackers.mean_score: Option<f64>` (None = NaN).

### 1.8 Storage state (`app/src/main/java/eu/kanade/presentation/more/storage/StorageScreenState.kt`)

`sealed class StorageScreenState { object Loading; data class Success(selectedCategory: Category, items: List<StorageItem>, categories: List<Category>) }`.

### 1.9 Update list items (presentation, part 1 consumes)

`MangaUpdatesItem(update: MangaUpdatesWithRelations, downloadStateProvider: () -> MangaDownload.State, downloadProgressProvider: () -> Int, selected: Boolean = false)` and `AnimeUpdatesItem(update: AnimeUpdatesWithRelations, downloadStateProvider: () -> AnimeDownload.State, downloadProgressProvider: () -> Int, selected: Boolean = false)` — both `@Immutable data class` in the part-2 model files (`MangaUpdatesScreenModel.kt:398-403`, `AnimeUpdatesScreenModel.kt:437-442`). Rust: `UpdateItem<T> { update: T, download: DownloadSnapshot { state, progress }, selected: bool }` (state is a value, not a closure).

### 1.10 Screen-model state and dialogs

| Model | State fields | Dialog variants | Event variants |
|---|---|---|---|
| `MangaHistoryScreenModel` (`ui/history/manga/MangaHistoryScreenModel.kt:250-272`) | `searchQuery: String?`, `list: List<MangaHistoryUiModel>?`, `dialog: Dialog?` | `DeleteAll`; `Delete(history: MangaHistoryWithRelations)`; `DuplicateManga(manga: Manga, duplicate: Manga)`; `ChangeCategory(manga: Manga, initialSelection: ImmutableList<CheckboxState<Category>>)`; `Migrate(newManga: Manga, oldManga: Manga)` | `OpenChapter(chapter: Chapter?)`; `InternalError`; `HistoryCleared` |
| `AnimeHistoryScreenModel` (`ui/history/anime/AnimeHistoryScreenModel.kt:245-267`) | same with `AnimeHistoryWithRelations`, `list: List<AnimeHistoryUiModel>?` | `DeleteAll`; `Delete(history)`; `DuplicateAnime(anime, duplicate)`; `ChangeCategory(anime, initialSelection)`; `Migrate(newAnime, oldAnime)` | `OpenEpisode(episode: Episode?)`; `InternalError`; `HistoryCleared` |
| `MangaUpdatesScreenModel` (`ui/updates/manga/MangaUpdatesScreenModel.kt:363-395`) | `isLoading: Boolean = true`, `items: PersistentList<MangaUpdatesItem>`, `dialog: Dialog?`; derived `selected`, `selectionMode` | `DeleteConfirmation(toDelete: List<MangaUpdatesItem>)` | `InternalError`; `LibraryUpdateTriggered(started: Boolean)` |
| `AnimeUpdatesScreenModel` (`ui/updates/anime/AnimeUpdatesScreenModel.kt:396-434`) | same | `DeleteConfirmation(toDelete)`; `ShowQualities(episodeTitle: String, episodeId: Long, animeId: Long, sourceId: Long)` | `InternalError`; `LibraryUpdateTriggered(started)` |
| `AnimeStatsScreenModel` / `MangaStatsScreenModel` | `StatsScreenState` only | none | none |
| `CommonStorageScreenModel<T>` | `StorageScreenState` only | none | none |

Event channel capacities (OBSERVED): history `Channel(Channel.UNLIMITED)` (`MangaHistoryScreenModel.kt:65`); updates `Channel(Int.MAX_VALUE)` (`MangaUpdatesScreenModel.kt:63`); the two history resume channels are `Channel<Unit>()` rendezvous (`MangaHistoryTab.kt:41`, `AnimeHistoryTab.kt:43`).

---

## 2. Enums and their stored values

| Enum | Values (Kotlin ordinal name = `value`) | Source |
|---|---|---|
| `MangaDownload.State` | `NOT_DOWNLOADED(0)`, `QUEUE(1)`, `DOWNLOADING(2)`, `DOWNLOADED(3)`, `ERROR(4)` | `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt:67-73` |
| `AnimeDownload.State` | same five values | `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt:62-68` |
| `ChapterDownloadAction` | `START`, `START_NOW`, `CANCEL`, `DELETE` | `app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt` (used at `:95-240`) |
| `EpisodeDownloadAction` | `START`, `START_NOW`, `CANCEL`, `DELETE`, `SHOW_QUALITIES` | `app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt` (`:96-240`) |
| `NavStyle` | `MOVE_HISTORY_TO_MORE`, `MOVE_BROWSE_TO_MORE`, `MOVE_UPDATES_TO_MORE`, `MOVE_MANGA_TO_MORE` (`UpdatesTab` maps all four explicitly, `UpdatesTab.kt:31-36`; `HistoriesTab` uses an `else`, `HistoriesTab.kt:36-40`) | Enum defined in the `eu.kanade.domain.ui.model` package (not opened by this area; owned by area:36-ui-shell-navigation-security-webview); used at `HistoriesTab.kt:36-40`, `UpdatesTab.kt:31-36` |
| `HapticFeedbackType.LongPress` | Compose | dropped |

Persisted enum: the nav style is stored with `preferenceStore.getEnum("bottom_rail_nav_style", NavStyle.MOVE_HISTORY_TO_MORE)` (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:40`). Its stored representation is the enum `name()` (area:02-preferences-system owns the codec).

Entry-restriction tokens (strings, stored in a string set; `LibraryPreferences.kt:403-406`): `"manga_ongoing"` (`ENTRY_NON_COMPLETED`), `"manga_fully_read"` (`ENTRY_HAS_UNVIEWED`), `"manga_started"` (`ENTRY_NON_VIEWED`), `"manga_outside_release_period"` (`ENTRY_OUTSIDE_RELEASE_PERIOD`). Default set (`LibraryPreferences.kt:49-56`): all four. Anime tokens are the same constants (the anime preference uses the same `ENTRY_*` strings).

Status codes: `SManga.COMPLETED = 2`, `SAnime.COMPLETED = 2`. Other status codes are not used in this area.

---

## 3. Preference keys used by this area

Type column: `Bool`, `Int`, `Long`, `Str`, `StrSet` (string set), `Enum`. "Default" is the value passed to the getter at `path:line`. Keys written by this area are marked **W**.

| Key (exact string) | Type | Default | Getter location | Read/Write | Used for |
|---|---|---|---|---|---|
| `library_update_last_timestamp` (wrapped by `Preference.appStateKey(...)`, area:02-preferences-system owns the prefix) | Long | `0L` | `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:39` | R | "Library last updated" header (2.3) |
| `library_unread_updates_count` | Int | `0` | `LibraryPreferences.kt:148` | R/W (W = set to 0 in `resetNewUpdatesCount`) | Manga updates badge |
| `library_unseen_updates_count` | Int | `0` | `LibraryPreferences.kt:149` | R/W | Anime updates badge |
| `hidden_categories` | Bool | `false` | `LibraryPreferences.kt:72` | R (once per storage screen open) | Storage: hide hidden categories |
| `default_category` | Int | `-1` | `LibraryPreferences.kt:154` (`DEFAULT_MANGA_CATEGORY_PREF_KEY`, `:413`) | R | Add-to-library default category (1.10) |
| `default_anime_category` | Int | `-1` | `LibraryPreferences.kt:153` (`:414`) | R | Same for anime |
| `library_update_categories` | StrSet | `{}` | `LibraryPreferences.kt:162` (`:415`) | R | Stats global-update include list (5.5) |
| `animelib_update_categories` | StrSet | `{}` | `LibraryPreferences.kt:159` (`:416`) | R | Same for anime |
| `library_update_categories_exclude` | StrSet | `{}` | `LibraryPreferences.kt:168` (`:417`) | R | Stats global-update exclude list |
| `animelib_update_categories_exclude` | StrSet | `{}` | `LibraryPreferences.kt:165` (`:418`) | R | Same for anime |
| `library_update_manga_restriction` | StrSet | `{manga_fully_read, manga_ongoing, manga_started, manga_outside_release_period}` | `LibraryPreferences.kt:49-56` | R | `autoUpdateItemRestrictions()` (stats 5.5) |
| `relative_time_v2` | Bool | `true` | `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:33` | R | Relative vs absolute dates (1.6, 2.x) |
| `app_date_format` | Str | `""` (= localized SHORT) | `UiPreferences.kt:35`; codec `UiPreferences.kt:44-48` | R | Absolute date format; non-empty value is a Java `DateTimeFormatter` pattern with `Locale.getDefault()` |
| `bottom_rail_nav_style` | Enum | `MOVE_HISTORY_TO_MORE` | `UiPreferences.kt:40` | R | History/Updates nav-index map (0.1) |
| `pref_always_use_external_player` | Bool | `false` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt:67-70` | R | Anime resume opens external vs internal player (1.7, 2.6) |
| `use_external_downloader` | Bool | `false` | `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:14` | R (captured once in the anime updates model as `useExternalDownloader`, `AnimeUpdatesScreenModel.kt:70`) | Downloads (area:24-downloads-anime) |

Not read here but used by the same screens through other areas: `alwaysUseExternalPlayer` (above), `Security`/`Incognito` (area:36-ui-shell-navigation-security-webview), the library update job's own prefs (area:26-library-update-jobs).

---

## 4. Database tables and views touched

### 4.1 Reads (through views)

| Object | Kind | Defined in | Columns used by this area |
|---|---|---|---|
| `historyView` | VIEW | `data/src/main/sqldelight/view/historyView.sq` | `id`, `mangaId`, `chapterId`, `title`, `thumbnailUrl`, `source`, `favorite`, `cover_last_modified`, `chapterNumber`, `readAt`, `readDuration`, `maxReadAtChapterId` |
| `history` (query file) | queries `history`, `getLatestHistory`, `getReadDuration`, `getHistoryByMangaId`, `removeAllHistory`, `resetHistoryById`, `resetHistoryByMangaId` | `data/src/main/sqldelight/data/history.sq` | as in 1.1 of p1-behavior.md |
| `animehistoryView` | VIEW | `data/src/main/sqldelightanime/view/animehistoryView.sq` | `id`, `animeId`, `episodeId`, `title`, `thumbnailUrl`, `source`, `favorite`, `cover_last_modified`, `episodeNumber`, `seenAt`, `maxSeenAtEpisodeId` |
| `animehistory` (query file) | queries `animehistory`, `getLatestAnimeHistory`, `getHistoryByAnimeId`, `resetAnimeHistoryById`, `resetHistoryByAnimeId`, `removeAllHistory`, `upsert` | `data/src/main/sqldelightanime/dataanime/animehistory.sq` | as in 1.1 |
| `updatesView` | VIEW | `data/src/main/sqldelight/view/updatesView.sq` | as in 2.1 (`getRecentUpdates(after, limit)`; `getUpdatesByReadStatus(read, after, limit)`) |
| `animeupdatesView` | VIEW | `data/src/main/sqldelightanime/view/animeupdatesView.sq` | as in 2.1 (`getRecentAnimeUpdates`, `getUpdatesBySeenStatus`) |
| `animehistorystatsView` | VIEW | `data/src/main/sqldelightanime/view/animehistorystatsView.sq` | defined (`episodes.anime_id`, `max(ah.last_seen) AS lastSeen`) but NOT referenced by this area's code (dead for part 1; verify usage in area:06-data-repos-history-track-source-misc) |
| `mangas` | TABLE | `data/src/main/sqldelight/data/mangas.sq` | `_id`, `title`, `source`, `favorite`, `thumbnail_url`, `cover_last_modified`, `date_added`, `status`, `chapter_flags` (via library queries) |
| `chapters` | TABLE | `data/src/main/sqldelight/data/chapters.sq` | `_id`, `manga_id`, `name`, `scanlator`, `read`, `bookmark`, `last_page_read INTEGER NOT NULL`, `chapter_number`, `date_fetch`, `date_upload` |
| `animes` | TABLE | `data/src/main/sqldelightanime/dataanime/animes.sq` | `_id`, `title`, `source`, `favorite`, `thumbnail_url`, `cover_last_modified`, `date_added`, `status` |
| `episodes` | TABLE | `data/src/main/sqldelightanime/dataanime/episodes.sq` | `_id`, `anime_id`, `name`, `scanlator`, `seen`, `bookmark`, `fillermark` (`INTEGER AS Boolean NOT NULL`), `last_second_seen INTEGER NOT NULL`, `total_seconds INTEGER NOT NULL`, `episode_number`, `date_fetch`, `date_upload` |
| `categories` | TABLE | `data/src/main/sqldelight/data/categories.sq` | via `GetMangaCategories` / `GetAnimeCategories` (area:08-domain-library-category-history-updates) |
| `mangas_categories` | join table | `data/src/main/sqldelight/data/mangas_categories.sq` | membership for change-category dialog (anime counterpart is in `data/src/main/sqldelightanime/`, owned by area:04-db-anime-schema-migrations, area:05-data-repos-entries-items) |

Views in SQLite: `historyView` and `updatesView` contain a `WHERE favorite = 1` (updates) and `readAt > 0` (history) filter as described in p1-behavior.md sections 1.1 and 2.1.

### 4.2 Writes (through repositories; not performed by UI code directly)

| Operation | Table | SQL semantics | Caller in part 1 |
|---|---|---|---|
| reset one history row | `history` | `UPDATE history SET last_read = 0 WHERE _id = :historyId` | `RemoveMangaHistory.await(history)` |
| reset all history of an entry | `history` | `UPDATE history SET last_read = 0 WHERE _id IN (...)` (see behavior 1.8) | `RemoveMangaHistory.await(mangaId)` |
| clear all manga history | `history` | `DELETE FROM history;` (hard delete, drops `time_read`) | `RemoveMangaHistory.awaitAll()` |
| reset one anime history row | `animehistory` | `UPDATE animehistory SET last_seen = 0 WHERE _id = :historyId` | `RemoveAnimeHistory.await(history)` |
| reset anime history of an entry | `animehistory` | analogous | `RemoveAnimeHistory.await(animeId)` |
| clear all anime history | `animehistory` | `DELETE FROM animehistory;` | `RemoveAnimeHistory.awaitAll()` |
| set favorite | `mangas`/`animes` | via `UpdateManga.awaitUpdateFavorite(id, true)` (area:05-data-repos-entries-items, area:10-app-domain-layer) | add-to-library |
| set categories | `mangas_categories`/`animes_categories` | via `SetMangaCategories.await(id, ids)` / `SetAnimeCategories` (area:08-domain-library-category-history-updates) | add-to-library, change-category |
| set chapter read/bookmark | `chapters` | via `UpdateChapter.awaitAll(ChapterUpdate)` / `SetReadStatus.await(read, chapters)` (area:07-domain-entries-items) | updates bulk actions |
| set episode seen/bookmark/fillermark | `episodes` | via `UpdateEpisode.awaitAll(EpisodeUpdate)` / `SetSeenStatus.await(seen, episodes)` (area:07-domain-entries-items) | updates bulk actions |

Note on `history.upsert` (`history.sq` last queries): `INSERT ... ON CONFLICT(chapter_id) DO UPDATE SET last_read = :readAt, time_read = time_read + :time_read` — the reader writes this (area:43-reader-core, area:44-reader-viewers), and it is what makes `time_read` accumulate. Anime `animehistory.upsert`: `ON CONFLICT(episode_id) DO UPDATE SET last_seen = :seenAt`. These define the data this area displays.

### 4.3 Transactions and reactivity

- `subscribeToList` / `subscribeToOne` on `MangaDatabaseHandler` / `AnimeDatabaseHandler` emit whenever the underlying tables change (`data/src/main/java/tachiyomi/data/handlers/...`, area:06-data-repos-history-track-source-misc). This is why history and updates pages refresh automatically after reading progress changes.
- Non-reactive `awaitList`, `await`, `awaitOne`, `awaitOneOrNull` are used for one-shot reads (`getTotalReadDuration`, `getLastMangaHistory`).
- Each write method in the repositories wraps the call in `try { } catch (e: Exception) { logcat(ERROR) }` and returns nothing (or `false` for `deleteAll*`), i.e. write failures are swallowed (OBSERVED; behavior 1.8, 1.9).

---

## 5. Constants (exact values)

| Name | Value | Where |
|---|---|---|
| Updates window (upload date) | `ZonedDateTime.now().minusMonths(3).toInstant()` | `MangaUpdatesScreenModel.kt:75-76`, `AnimeUpdatesScreenModel.kt:80` |
| Updates row limit | `500` | `GetMangaUpdates.kt` (`subscribe(instant)` passes `limit = 500`); `GetAnimeUpdates` analogous |
| History row limit | none (all rows) | `history.sq` has no `LIMIT` |
| Fake refresh spinner | `1.seconds` | `MangaUpdatesScreen.kt:81`, `AnimeUpdatesScreen.kt:87` |
| History/update row height | `96.dp` (history), `56.dp` (updates) | `MangaHistoryItem.kt:35`, `MangaUpdatesUiItem.kt:166`, `AnimeUpdatesUiItem.kt:173` |
| Cover in history | `ItemCover.Book` (fills height) | `MangaHistoryItem.kt:56-60` |
| Cover in updates | `ItemCover.Square`, padding vertical `6.dp` | `MangaUpdatesUiItem.kt:170-176` |
| Unread dot | `Icons.Filled.Circle`, height `8.dp`, end padding `4.dp` | `MangaUpdatesUiItem.kt:192-199` |
| Icon spacer | `2.dp` | `MangaUpdatesUiItem.kt:211` |
| Bookmark icon max height | `textHeight - 2.dp` | `MangaUpdatesUiItem.kt:206-208` |
| Disabled alpha | `DISABLED_ALPHA` (presentation-core constant) | `MangaUpdatesUiItem.kt:154` |
| Storage cover | `ItemCover.Square`, height `48.dp` | `StorageItem.kt:68-72` |
| Storage colour dot | `12.dp` circle | `StorageItem.kt:88-89` |
| Storage separator dot | `padding.small / 2` | `StorageItem.kt:96-101` |
| Donut stroke width | `15f` (px) | `CumulativeStorage.kt:20` |
| Donut arc total | `180f` degrees, `rotate(180f)` | `CumulativeStorage.kt:37-39` |
| Donut canvas aspect | `1f` | `CumulativeStorage.kt:32` |
| Donut layout height | `canvas.height / 2` | `CumulativeStorage.kt:66` |
| Byte units | `1000` (decimal), formats `"%.2f GB"`, `"%.2f MB"`, `"%.2f KB"`, `"$n B"` | `app/src/main/java/eu/kanade/tachiyomi/util/StorageUtil.kt:8-19` |
| Mean score format | `"%.2f ★"` with `Locale.ENGLISH` | `AnimeStatsScreenContent.kt:145`, `MangaStatsScreenContent.kt:141` |
| Stats duration units | `DurationUnit.MILLISECONDS` | `AnimeStatsScreenContent.kt:67`, `MangaStatsScreenContent.kt:63` |
| Watch/read time units | ms | see behavior 5.3, `AnimeUpdatesUiItem.kt:265-283` |
| Storage colour RNG | `kotlin.random.Random(entryId)`, `nextInt(255)` x3 | `CommonStorageScreenModel.kt:74-87` |
| Tracks score threshold | `score > 0.0` | `AnimeStatsScreenModel.kt:146`, `MangaStatsScreenModel.kt:131` |
| Storage tablet weights | `Info` `weight(2f)` + `padding(end = extraLarge)`, list `weight(3f)` | `StorageScreenContent.kt:78-109` |
| Storage page padding | horizontal `padding.small`; `Info` vertical `padding.medium`; bottom `padding.medium` on phone | `StorageScreenContent.kt:73-76`, `:54-66` |
| Stats section spacing | `Arrangement.spacedBy(MaterialTheme.padding.small)` | `AnimeStatsScreenContent.kt:42`, `MangaStatsScreenContent.kt:38` |
| Stats overview row | `Modifier.height(IntrinsicSize.Min)` | `AnimeStatsScreenContent.kt:72`, `MangaStatsScreenContent.kt:68` |
| Tab indices | Stats 8, Storage 8, History 2/3/5, Updates 1/2/5 | section 0.1 |

---

## 6. Files and folders

No file paths are produced by part 1 code. Derived facts used for display:
- Storage size per entry = `downloadManager.getDownloadSize(entry)`: for a local entry it is the size of `LocalMangaSourceFileSystem.getMangaDirectory(url)` (`MangaDownloadManager.kt`, `getDownloadSize(manga)`); otherwise the download cache sums the entry's chapter directories (area:23-downloads-manga, area:24-downloads-anime).
- Storage count per entry = `getDownloadCount(entry)`: for local entries the number of directories or archives inside the manga directory (`ArchiveManga.isSupported`), otherwise the cached chapter/episode directory count.
- Total stats download count = `getDownloadCount()` over the whole cache (all sources).

---

## 7. Serialization and wire formats

None in part 1. No JSON, protobuf or backup is read or written here. Backups of history/updates are area:21-backup-format-and-create, area:22-backup-restore (the `history` and `chapters` fields `last_read`, `time_read`, `last_second_seen` are part of backup payloads there).

---

## 8. Concurrency-relevant data facts

- `ZonedDateTime.now()` at `init` fixes the updates window for the life of the screen model (`MangaUpdatesScreenModel.kt:75`).
- `libraryPreferences.hideHiddenCategoriesSettings().get()` read once (`CommonStorageScreenModel.kt:40`).
- `useExternalDownloader` captured once in the anime updates model (`AnimeUpdatesScreenModel.kt:70`).
- `lastUpdated` uses `asState(screenModelScope)` (live).
- `Date` values (`readAt`, `seenAt`) are mutable `java.util.Date` objects; Rust uses `i64` millis (immutable).
