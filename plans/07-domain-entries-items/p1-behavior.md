# Area 07 — behavior spec: entries (manga/anime/season) and items (chapter/episode/season)

Scope: the 74 files listed in p1-inventory.md (the `domain` module's `entries`, `items`, `aniyomi/domain/anime` packages and `domain/src/test`). Paths are repo-relative to `/home/blackbox/aniyomi-src`. "OBSERVED" = what the Kotlin does, "Proposal:" = what Rust should do. Data layouts: p1-data-and-config.md. Deep dives: p1-topic-flags-bit-layouts.md, p1-topic-item-number-recognition.md, p1-topic-fetch-interval.md, p1-topic-sorting-and-filtering.md, p1-topic-release-check.md, p1-topic-quirks-and-decisions.md.

## 0. Layering and who calls what

OBSERVED. The `domain` module is pure Kotlin: immutable data classes, `suspend` interactors that wrap one repository call, and a handful of pure algorithms. It has no UI. Callers outside the scope:

| Caller (repo-relative) | Uses |
|---|---|
| `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt`, `app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt`, `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt` (area:10-app-domain-layer) | `Chapter.create/copyFrom/toChapterUpdate`, `ChapterRecognition`, `ShouldUpdateDbChapter` (and episode/season twins), `GetChaptersByMangaId`, `UpdateChapter`, `NetworkToLocalAnime`, `GetAnimeSeasonsByParentId`, `NoChaptersException`, `NoEpisodesException`, `NoSeasonsException` |
| `app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt`, `app/src/main/java/eu/kanade/domain/entries/anime/interactor/UpdateAnime.kt` | `MangaFetchInterval.toMangaUpdate/getWindow`, `AnimeFetchInterval.toAnimeUpdate/getWindow`, `MangaUpdate`, `AnimeUpdate` |
| `app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt`, `app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt` | `getChapterSort`, `getEpisodeSort`, `applyFilter`, TriState getters |
| `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:1701-1721`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt` | `getSeasonSortComparator`, `seasonSortAlphabetically`, all Set*Flags interactors, SetDefault*Flags |
| `app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt:443-449`, `app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt:545-548` | `missingChaptersCount`, `missingEntriesCount`, `calculateChapterGap`, `calculateEpisodeGap` |
| `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt:184,217,256` | `MangaFetchInterval.getWindow`, `nextUpdate > fetchWindowUpperBound` skip test (message `MR.strings.skipped_reason_not_in_release_period` = "Skipped because no release was expected today") |
| `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:206`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1340`, `domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetNextChapters.kt:24`, `domain/src/main/java/tachiyomi/domain/history/anime/interactor/GetNextEpisodes.kt:24` | `getChapterSort(manga, sortDescending = false)` / `getEpisodeSort(anime, sortDescending = false)` to get playback order regardless of the list direction |

Proposal: in Rust the interactor classes dissolve into plain functions and thin query methods (the Kotlin classes exist only for Injekt injection, area:01-foundation-app-bootstrap `T-01a-003`). Pure algorithms become free functions in `kuro-core/src/domain/` (design in p1-rust-design.md). Behaviour must stay identical.

## 1. Flag primitives

### 1.1 `setFlag`

OBSERVED (private copy in `SetMangaChapterFlags.kt:94-96`, `SetAnimeEpisodeFlags.kt:127-129`, `SetAnimeSeasonFlags.kt:186-188`, `SetAnimeViewerFlags.kt`): `this and mask.inv() or (flag and mask)`. Kotlin infix precedence: `and`/`or`/`inv` are all named functions of equal precedence evaluated left to right, so the expression is `((this and mask.inv()) or (flag and mask))`. Bits of `flag` outside `mask` are discarded. Proposal: one function `set_flag(v: i64, flag: i64, mask: i64) -> i64 { (v & !mask) | (flag & mask) }`.

### 1.2 `TriState` and `applyFilter`

OBSERVED. `tachiyomi.core.common.preference.TriState` (`core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:3-16`) has three values `DISABLED`, `ENABLED_IS`, `ENABLED_NOT`; `next()` cycles `DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED` (UI checkbox cycle in the filter sheet). `applyFilter(filter, predicate)` (`domain/src/main/java/tachiyomi/domain/entries/TriState.kt:5-9`) returns `true` for DISABLED without evaluating the predicate, `predicate()` for ENABLED_IS, `!predicate()` for ENABLED_NOT. The predicate is evaluated lazily; Proposal: `fn apply_filter(f: TriState, p: impl FnOnce() -> bool) -> bool`.

Usage in lists (`ChapterFilter.kt:343-363`): unread filter `applyFilter(unreadFilter) { !chapter.read }`; bookmarked `applyFilter(bookmarkedFilter) { chapter.bookmark }`; downloaded `applyFilter(downloadedFilter) { downloaded || isLocalManga }` (a local-source entry counts as downloaded); then `sortedWith(getChapterSort(manga))`. The three filters are AND-combined in the order unread, bookmarked, downloaded; the result does not depend on the order, but the downloaded predicate is the expensive one (a download-index lookup) and, because `Sequence`/`filter` chains short-circuit per element, it only runs for rows that survived the first two. Keep that order in Rust. Episode filters (`EpisodeFilter.kt`) are the same with seen, bookmarked, fillermarked, downloaded (read `app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt` in area:10-app-domain-layer). Season filters (`AnimeScreenModel.kt:1713-1721`): unseen `!season.seen`, started `season.hasStarted`, completed `season.anime.status.toInt() == SAnime.COMPLETED` (2), bookmarked `season.hasBookmarks`, fillermarked `season.hasFillermarks`, downloaded `downloadCount > 0 || isLocal`.

## 2. Entry model behaviour

### 2.1 Defaults and identity

`Manga.create()` / `Anime.create()` produce the "blank" entries used by source-to-domain mapping (`SAnime.toDomainAnime`, area:10-app-domain-layer) before insert: `id = -1`, `source = -1`, strings empty, all flags 0, `updateStrategy = ALWAYS_UPDATE`, `initialized = false`, `version = 0`, anime `fetchType = Episodes`, `seasonNumber = -1.0`, `memo = {}`. An entry is identified in the DB by the unique pair `(url, source)` (`getMangaByUrlAndSourceId`).

### 2.2 `expectedNextUpdate`

OBSERVED (`Manga.kt:37-40`, `Anime.kt:50-53`): `nextUpdate.takeIf { status != COMPLETED.toLong() }?.let { Instant.ofEpochMilli(it) }`. `COMPLETED = 2`. Returns `null` only for completed entries. A never-computed `nextUpdate = 0` yields `Instant.EPOCH` (1970-01-01), not null; the UI (`SetIntervalDialog`, `app/src/main/java/eu/kanade/presentation/entries/components/ItemsDialogs.kt:70-100`) turns that into `0` days because it does `now.until(nextUpdate, DAYS).toInt().coerceAtLeast(0)`. Note statuses PUBLISHING_FINISHED (4) and CANCELLED (5) are not treated as finished here. Proposal: return `Option<i64>` (epoch ms) and let the UI format it.

### 2.3 Cover descriptors

`MangaCover` / `AnimeCover` are plain value objects keyed on `(id, source, favorite, thumbnailUrl, coverLastModified)`; the image loader uses them as cache keys (`MangaCoverFetcher`/`AnimeCoverFetcher`, area:10-app-domain-layer/area:13-core-util-storage-archive-torrent). `favorite` is part of the key because favourites are cached on disk permanently while non-favourites live in the LRU image cache. Proposal: the same struct is the cache key of the Kurobayomi cover loader; hash all five fields.

### 2.4 Partial updates

`MangaUpdate`/`AnimeUpdate` carry optional fields; a `None` field is not written (repository builds `COALESCE(:x, col)`, area:05-data-repos-entries-items `p1-topic-sql-semantics-and-triggers.md`). `toMangaUpdate()` / `toAnimeUpdate()` snapshot every optional field of the entry. Consequences used elsewhere: `awaitUpdateFetchInterval` returns an update holding only `id`, `nextUpdate`, `fetchInterval`; `SetAnimeEpisodeFlags` returns an update holding only `id` and one flag word.

## 3. Flag setters (Set*Flags interactors)

All setters are "read-modify-write on a snapshot": they take the entry object the caller already holds, compute the new flag word from `entry.<flags>` (NOT from a fresh DB read) and call `repository.update*(Update(id, <flags> = new))`. They return the repository's `Boolean`. There is no locking; two rapid taps on the filter sheet use the entry state at the time of the tap. Proposal: Rust takes `&Manga` / `&Anime` and returns `Result<bool>`; the details screen model must refresh its entry from the repository subscription before the next call (it does in Aniyomi because the flow re-emits).

### 3.1 Chapters (`SetMangaChapterFlags.kt`)

| Method | New value of `chapterFlags` |
|---|---|
| `awaitSetDownloadedFilter(manga, flag)` | `setFlag(flag, CHAPTER_DOWNLOADED_MASK 0x18)` |
| `awaitSetUnreadFilter(manga, flag)` | `setFlag(flag, CHAPTER_UNREAD_MASK 0x06)` |
| `awaitSetBookmarkFilter(manga, flag)` | `setFlag(flag, CHAPTER_BOOKMARKED_MASK 0x60)` |
| `awaitSetDisplayMode(manga, flag)` | `setFlag(flag, CHAPTER_DISPLAY_MASK 0x100000)` |
| `awaitSetSortingModeOrFlipOrder(manga, flag)` | see 3.1.1 |
| `awaitSetAllFlags(mangaId, unreadFilter, downloadedFilter, bookmarkedFilter, sortingMode, sortingDirection, displayMode)` | start from `0L` (all other bits cleared) then `setFlag` for each of the six groups in this order: unread, downloaded, bookmarked, sorting (`0x300`), direction (`0x1`), display |

#### 3.1.1 Sort-or-flip state machine (identical for episodes with `EPISODE_*`, and seasons with `SEASON_*`)

```
input: entry E, requested sorting value F (one of the SORTING constants)
if E.sorting == F:                 // same column tapped again
    dir' = if E.sortDescending() then ASC else DESC
    flags' = setFlag(E.flags, dir', DIR_MASK)
else:                              // different column
    flags' = setFlag(setFlag(E.flags, F, SORTING_MASK), ASC, DIR_MASK)   // ALWAYS ascending
write flags'
```

`E.sorting` is `flags & SORTING_MASK`. A new column always starts ascending (`CHAPTER_SORT_ASC = 1`), even when the previous direction was descending. Initial state `flags = 0` = source sorting, descending. Sequence from 0 for an episode: tap Number -> `0x201`; tap Number again -> `0x200`; tap Number again -> `0x201`.

### 3.2 Episodes (`SetAnimeEpisodeFlags.kt`)

Same methods as chapters with: `awaitSetUnseenFilter` (mask `0x6`), `awaitSetDownloadedFilter` (`0x18`), `awaitSetBookmarkFilter` (`0x60`), `awaitSetFillermarkFilter` (`0x180`), `awaitSetDisplayMode` (`0x100000`), `awaitSetSortingModeOrFlipOrder` (sorting mask `0x600`, direction `0x1`), `awaitShowEpisodePreviews(anime, flag)` (mask `0x800`), `awaitShowEpisodeSummaries(anime, flag)` (mask `0x1000`). `awaitSetAllFlags(animeId, unseenFilter, downloadedFilter, bookmarkedFilter, fillermarkedFilter, sortingMode, sortingDirection, displayMode, showPreviews, showSummaries)` starts from `0L` and applies, in this order: unseen, downloaded, bookmarked, fillermarked, sorting, direction, display, previews, summaries.

### 3.3 Seasons (`SetAnimeSeasonFlags.kt`)

Filter setters (all through private `setFlag(anime, flag, mask)` on `anime.seasonFlags`): downloaded (`0x6`), unseen (`0x18`), started (`0x60`), completed (`0x180`), bookmarked (`0x600`), fillermarked (`0x1800`), `awaitSetDisplayMode` (`0x08000000`). Sorting flip: section 3.1.1 with `SEASON_SORT_MASK 0xE000` and `SEASON_SORT_DIR_MASK 0x1`. `awaitSetGridMode(anime, mode)`: `flag = SeasonDisplayMode.toLong(mode) shl 16`, mask `0x30000`. `awaitSetGridSize(anime, size)`: `flag = size.toLong() shl 18`, mask `0x3C0000` (a size above 15 is silently truncated to its low 4 bits by the mask). Overlay setters use `setBooleanFlag(anime, value, mask)`: `flag = if (value) mask else 0`: downloaded `0x400000`, unseen `0x800000`, local `0x1000000`, lang `0x2000000`, continue `0x4000000`. `awaitSetAllFlags(animeId, downloadFilter, unseenFilter, startedFilter, completedFilter, bookmarkedFilter, fillermarkedFilter, sortingMode, sortingDirection, displayGridMode: SeasonDisplayMode, displayGridSize: Int, downloadedOverlay, unseenOverlay, localOverlay, langOverlay, continueOverlay, displayMode)` starts from `0L` and applies in this exact order: downloaded, unseen, started, completed, bookmarked, fillermarked, sorting, direction, grid mode (shifted), grid size (shifted), the five overlays (each `if (b) mask else 0`), display mode.

### 3.4 Default flags (`SetMangaDefaultChapterFlags`, `SetAnimeDefaultEpisodeFlags`, `SetAnimeDefaultSeasonFlags`)

OBSERVED. `await(entry)` runs inside `withNonCancellableContext` (cannot be cancelled mid-write; core/common `withNonCancellableContext` = `withContext(NonCancellable)`), reads the library preferences and calls `awaitSetAllFlags(entryId, <prefs>)`:

| Group | Chapter pref (sections in p1-data-and-config.md 4) | Episode pref | Season pref |
|---|---|---|---|
| filter 1 | `filterChapterByRead` | `filterEpisodeBySeen` | `filterSeasonByDownload` |
| filter 2 | `filterChapterByDownloaded` | `filterEpisodeByDownloaded` | `filterSeasonByUnseen` |
| filter 3 | `filterChapterByBookmarked` | `filterEpisodeByBookmarked` | `filterSeasonByStarted` |
| filter 4 | | `filterEpisodeByFillermarked` | `filterSeasonByCompleted` |
| filter 5/6 | | | `filterSeasonByBookmarked`, `filterSeasonByFillermarked` |
| sorting mode | `sortChapterBySourceOrNumber` | `sortEpisodeBySourceOrNumber` | `sortSeasonBySourceOrNumber` |
| direction | `sortChapterByAscendingOrDescending` | `sortEpisodeByAscendingOrDescending` | `sortSeasonByAscendingOrDescending` |
| display | `displayChapterByNameOrNumber` | `displayEpisodeByNameOrNumber` | `seasonDisplayMode` |
| extras | | `showEpisodeThumbnailPreviews`, `showEpisodeSummaries` | grid mode (`SeasonDisplayMode.fromLong(seasonDisplayGridMode().get())`), grid size, five overlay booleans |

`awaitAll()` loads every library favourite (`GetMangaFavorites.await()` / `GetAnimeFavorites.await()`) and calls `await(it)` sequentially for each, all inside one non-cancellable block: this is the "Apply to all entries in library" action. Call sites (OBSERVED): (a) every time the details screen loads an entry that is NOT a favourite (`MangaScreenModel.kt:220-222`, `AnimeScreenModel.kt:243-246`: the anime screen applies both the episode and the season defaults), so a non-library entry always shows the current default settings and any per-entry choice made on it is lost when it is reopened; (b) when a browse card is toggled into the library (`BrowseMangaSourceScreenModel.kt:247` and `BrowseAnimeSourceScreenModel.kt:239`; only the episode defaults are applied there, the season defaults are not); (c) the sheet action "Set as default" -> `libraryPreferences.setChapterSettingsDefault(manga)` (or `setEpisodeSettingsDefault`/`setSeasonSettingsDefault`) then, if the user ticked "apply to existing", `awaitAll()`, then a snackbar `MR.strings.chapter_settings_updated` = "Updated default chapter settings" (anime `AYMR.strings.episode_settings_updated` = "Updated default episode settings", seasons `AYMR.strings.season_settings_updated` = "Updated default season settings"); (d) "Reset to default" in the same sheet -> `await(entry)` (`MangaScreenModel.kt:964-968`). The save-as-default and reset run in `launchNonCancellable`. A preference value that is not a valid member of a group is not validated; `setFlag` masks it.

The reverse direction (persisting an entry's current settings as the user's default) is `LibraryPreferences.setEpisodeSettingsDefault/setChapterSettingsDefault/setSeasonSettingsDefault` (area:02-preferences-system/area:08-domain-library-category-history-updates). Known defects: Q2 and Q3 in p1-topic-quirks-and-decisions.md.

### 3.5 Viewer flags reset

`ResetMangaViewerFlags.await()` / `ResetAnimeViewerFlags.await()`: `repository.reset*ViewerFlags()` -> SQL `UPDATE mangas SET viewer = 0` / `UPDATE animes SET viewer = 0`; returns the repository's `Boolean`. Triggered from Settings > Reader/Player "Reset reading mode and orientation for all entries" (reader and player series (no plans folder yet)). For anime it also wipes skip-intro length, disable bit and cached airing data (all live in `viewer`).

## 4. Query interactors (read side)

Error semantics are the only behaviour; the table is exhaustive.

| Interactor (file) | Method | Repository call | On exception | Return |
|---|---|---|---|---|
| `GetManga` | `await(id)` | `getMangaById(id)` | catch `Exception`, `logcat(ERROR)` | `Manga?` null on failure (missing row throws, see below) |
| | `subscribe(id)` (suspend) | `getMangaByIdAsFlow(id)` | none | `Flow<Manga>` |
| | `subscribe(url, sourceId)` | `getMangaByUrlAndSourceIdAsFlow` | none | `Flow<Manga?>` |
| `GetAnime` | same three | `getAnimeById`, `getAnimeByIdAsFlow`, `getAnimeByUrlAndSourceIdAsFlow` | same | same |
| `GetMangaByUrlAndSourceId` / `GetAnimeByUrlAndSourceId` | `await(url, sourceId)` | `getMangaByUrlAndSourceId` | none (propagates) | `Manga?` null when absent |
| `GetMangaFavorites` / `GetAnimeFavorites` | `await()` | `getMangaFavorites()` | none | all `favorite = 1` rows |
| | `subscribe(sourceId)` | `getMangaFavoritesBySourceId` | none | `Flow<List>` |
| `GetDuplicateLibraryManga` / `Anime` | `await(manga)` | `getDuplicateLibraryManga(manga.id, manga.title.lowercase())` | none | favourites with same lower-cased title and different id |
| `GetLibraryManga` / `GetLibraryAnime` | `await()` | `getLibraryManga()` | none | `List<LibraryManga>` |
| | `subscribe()` | `getLibraryMangaAsFlow().retry{...}.catch{...}` | see 4.1 | `Flow<List<LibraryManga>>` |
| `GetMangaWithChapters` | `subscribe(id, applyScanlatorFilter = false)` | `combine(getMangaByIdAsFlow(id), getChapterByMangaIdAsFlow(id, applyScanlatorFilter)) -> Pair` | none | `Flow<Pair<Manga, List<Chapter>>>` |
| | `awaitManga(id)`, `awaitChapters(id, applyScanlatorFilter = false)` | `getMangaById`, `getChapterByMangaId` | none (propagate) | |
| `GetAnimeWithEpisodesAndSeasons` | `subscribe(id)` | `combine(getAnimeByIdAsFlow, getEpisodeByAnimeIdAsFlow, getAnimeSeasonsByIdAsFlow) -> Triple` | none | `Flow<Triple<Anime, List<Episode>, List<SeasonAnime>>>` |
| | `awaitAnime`, `awaitEpisodes`, `awaitSeasons` | `getAnimeById`, `getEpisodeByAnimeId`, `getAnimeSeasonsById` | none | |
| `GetRelatedAnime` | `subscribe(animeId)` | `subscribeRelatedAnime(animeId)` | none | `Flow<List<AnimeRelationGroup>>` |
| `GetChapter` | `await(id)` | `getChapterById` | catch, log | `Chapter?` |
| | `await(url, mangaId)` | `getChapterByUrlAndMangaId` | catch, log | `Chapter?` |
| `GetChapterByUrlAndMangaId` | `await(url, sourceId)` (the second parameter is really a manga id; misnamed) | `getChapterByUrlAndMangaId(url, sourceId)` | catch `Exception` silently (no log) | `Chapter?` |
| `GetChaptersByMangaId` | `await(mangaId, applyScanlatorFilter = false)` | `getChapterByMangaId` | catch, log | `emptyList()` on failure |
| `GetEpisode`, `GetEpisodeByUrlAndAnimeId`, `GetEpisodesByAnimeId` | as the chapter twins | `getEpisodeById`, `getEpisodeByUrlAndAnimeId`, `getEpisodeByAnimeId` | as above | `Episode?` / `emptyList()` |
| `GetAnimeSeasonsByParentId` | `await(animeId)` | `getAnimeSeasonsById(animeId)` | catch, log | `emptyList()` |
| `UpdateChapter` / `UpdateEpisode` | `await(update)`, `awaitAll(updates)` | `updateChapter`, `updateAllChapters` | catch `Exception`, log, swallow | `Unit` (the caller cannot tell a write failed) |

`getMangaById` throws when the row does not exist (SQLDelight `executeAsOne`, area:05-data-repos-entries-items); that is why `GetManga.await` exists in the exception-to-null form and why code that wants the exception calls `mangaRepository.getMangaById` directly (e.g. `MangaScreenModel.setFetchInterval`). `getChapterById` returns `null` for a missing row (nullable in the repository signature) and so never reaches the catch; the catch covers driver errors. `awaitChapters` and `awaitEpisodes` accept `applyScanlatorFilter` only for manga; the anime side has no scanlator filter.

`ChapterRepository`/`EpisodeRepository`/`MangaRepository`/`AnimeRepository`/`AnimeRelationRepository` method lists (for the coverage task in p1-tasks.md T-07a-033):

- `MangaRepository`: getMangaById, getMangaByIdAsFlow (suspend returning Flow), getMangaByUrlAndSourceId, getMangaByUrlAndSourceIdAsFlow, getMangaFavorites, getReadMangaNotInLibrary, getLibraryManga, getLibraryMangaAsFlow, getMangaFavoritesBySourceId (Flow), getDuplicateLibraryManga(id, title), getUpcomingManga(statuses: Set<Long>) (suspend returning Flow), resetMangaViewerFlags, setMangaCategories(mangaId, categoryIds), insertManga (returns `Long?`), updateManga(update): Boolean, updateAllManga(updates): Boolean.
- `AnimeRepository`: the same 16 with anime names (`getWatchedAnimeNotInLibrary` for `getReadMangaNotInLibrary`) plus getAnimeSeasonsById(parentId), getAnimeSeasonsByIdAsFlow(parentId), removeParentIdByIds(animeIds), getDeletableParentAnime(): Flow<List<DeletableAnime>>, getChildrenByParentId(parentId): List<Anime> (21 methods).
- `ChapterRepository`: addAllChapters(chapters): List<Chapter>, updateChapter, updateAllChapters, removeChaptersWithIds, getChapterByMangaId(mangaId, applyScanlatorFilter = false), getScanlatorsByMangaId, getScanlatorsByMangaIdAsFlow, getBookmarkedChaptersByMangaId, getChapterById, getChapterByMangaIdAsFlow(mangaId, applyScanlatorFilter = false) (suspend returning Flow), getChapterByUrlAndMangaId.
- `EpisodeRepository`: addAllEpisodes, updateEpisode, updateAllEpisodes, removeEpisodesWithIds, getEpisodeByAnimeId, getBookmarkedEpisodesByAnimeId, getEpisodeById, getEpisodeByAnimeIdAsFlow (suspend returning Flow), getEpisodeByUrlAndAnimeId (no scanlator methods).
- `AnimeRelationRepository`: subscribeRelatedAnime(animeId): Flow<List<AnimeRelationGroup>>, getLastFetchedAt(animeId): Long?, replaceRelations(animeId, groups: List<Pair<String, List<Long>>>, fetchedAt).

### 4.1 `GetLibraryManga.subscribe()` / `GetLibraryAnime.subscribe()`

OBSERVED (`GetLibraryManga.kt:21-33`): `repository.getLibraryMangaAsFlow().retry { cause -> if (cause is NullPointerException) { delay(0.5.seconds); true } else false }.catch { logcat(ERROR, it) }`. So: the library view query can transiently throw an NPE (typically while a restore replaces rows or while a view row is mid-update, area:05-data-repos-entries-items `p1-topic-library-and-season-views.md`); the stream is re-subscribed after 500 ms, without a limit on the number of retries. Any other exception ends the stream silently (logged; the collector sees normal completion and the library screen would keep its last state). Proposal: the Rust subscription yields `Result<Vec<LibraryManga>, DbError>`; the facade maps "transient" errors (SQLite `SQLITE_BUSY`, `SQLITE_LOCKED`, and the row-decode failure that corresponds to the NPE) to a retry after 500 ms with a cap of 20 attempts (10 s) before surfacing a user-visible error toast, and logs everything else with `tracing::error!` and keeps the previous list.

### 4.2 `NetworkToLocalManga.await(manga)` / `NetworkToLocalAnime.await(anime)`

OBSERVED (`NetworkToLocalManga.kt:10-26`): given a manga built from a source listing (id = -1):

```
local = repo.getMangaByUrlAndSourceId(manga.url, manga.source)
if local == null:
    id = repo.insertManga(manga)!!          // `!!`: a null id (insert failure) throws NullPointerException
    return manga.copy(id = id)              // the object the caller passed, with the new id
else if !local.favorite:
    return local.copy(title = manga.title)  // IN MEMORY ONLY: not written to the DB
else:
    return local                            // favourites keep their stored (possibly user-edited) title
```

The comment in source: "if the manga isn't a favorite, set its display title from source; if it later becomes a favorite, updated title will go to db". The `copy(title = ...)` is never persisted by this function: the new title reaches the DB only when `UpdateManga.awaitUpdateFromSource` runs (it also only sets `title` for non-favourites) or when the user adds to the library. `NetworkToLocalAnime` has identical logic plus a constructor parameter `sourceManager: AnimeSourceManager` that is never used (OBSERVED dead dependency). Used by browse (`BrowseAnimeSourceScreenModel.kt:116`), global search (`AnimeSearchScreenModel.kt:164`), `SyncSeasonsWithSource` (child rows created with `parentId` set afterwards), `SyncRelatedAnimeWithSource`, migration. In the seasons case an inserted child comes back with `id` set and the caller then does `.copy(parentId = anime.id, seasonSourceOrder = i)` and persists via `updateAllAnime`.

Concurrency: insert-if-absent is two repository calls (select, then insert) with no surrounding transaction, and `insertManga` performs no uniqueness check on `(source, url)` (area:05-data-repos-entries-items `p1-behavior.md`, `insertManga`). Two concurrent browse pages (or a browse page and a global-search result) that list the same new entry can both see `null` and insert two rows with the same `(source, url)`; the duplicate is invisible until a later `getMangaByUrlAndSourceId` returns one of them arbitrarily. Proposal: Rust performs `network_to_local` as ONE writer-connection transaction (select, insert if absent) and area:05-data-repos-entries-items should add a partial unique index or `INSERT ... WHERE NOT EXISTS`; tracked as T-07a-021.

## 5. Item number recognition

Summary only; exact regexes, rewrite for the Rust `regex` crate and 90+ verified vectors are in p1-topic-item-number-recognition.md.

`parseChapterNumber(mangaTitle, chapterName, chapterNumber: Double? = null): Double`:

1. If `chapterNumber != null && (chapterNumber == -2.0 || chapterNumber > -1.0)` return it unchanged. (So -1.0 and values in (-2, -1) and below -2 fall through; the source's own number wins when it is -2, 0 or any positive value.)
2. `clean = chapterName.lowercase().replace(mangaTitle.lowercase(), "").trim().replace(',', '.').replace('-', '.')`, then remove every whitespace char that is followed by `extra`, `special` or `omake`.
3. All matches of `number` in `clean`: none -> return `chapterNumber ?: -1.0`; more than one -> `name = unwanted.replace(clean, "")`, then the first match of `basic` in `name` if any, else the first match of `number` in `name` (if `name` has no number left, fall through); exactly one -> its value.
4. Fall-through: value of the first `number` match in `clean`.
5. A match's value = group1 as double + (group2 as double if non-empty, else special words in group3: contains `extra` 0.99, `omake` 0.98, `special` 0.97, else if group3 without leading dots is one letter `a`..`i` 0.1..0.9 (`j` or later 0.0), else 0.0).

`EpisodeRecognition.parseEpisodeNumber` adds a tag-stripping loop after step 2 and uses the episode regexes; `SeasonRecognition.parseSeasonNumber` is the episode algorithm with the season regexes. The three are otherwise line-for-line copies; Proposal: one generic function parameterised by a `Recogniser { basic, unwanted, strip_tags }` value.

## 6. Sorting

Details and tie-break rules: p1-topic-sorting-and-filtering.md. Summary of OBSERVED comparators (`ChapterSorter.kt`, `EpisodeSorter.kt`, `SeasonSorter.kt`):

- `getChapterSort(manga, sortDescending = manga.sortDescending())` returns a comparator chosen by `manga.sorting`: SOURCE (`sourceOrder`, direction INVERTED: `sortDescending == true` yields ascending `sourceOrder` because `sourceOrder 0` is the newest), NUMBER (`chapterNumber`), UPLOAD_DATE (`dateUpload`), ALPHABET (`name.compareToWithCollator`, case- and accent-insensitive locale collation, strength PRIMARY). Any other `sorting` value throws `NotImplementedError("Invalid chapter sorting method: ...")` (unreachable for the 2-bit field).
- `getEpisodeSort` identical with the episode fields and `anime.sorting`.
- `getSeasonSortComparator(anime)` returns an ASCENDING comparator over `SeasonAnime` by `anime.seasonSorting` (SOURCE `seasonSourceOrder`, SEASON `seasonNumber`, UPLOAD `latestUpload`, ALPHABET `seasonSortAlphabetically`, COUNT `unseenCount`, LAST_SEEN `lastSeen`, FETCHED `fetchedAt`); the caller applies `.reversed()` when `seasonSortDescending()` and then `.thenComparator(seasonSortAlphabetically)` as the final tie-break (`AnimeScreenModel.kt:1709-1711`). Value 0xE000 throws `NotImplementedError("Invalid season sorting method: ...")` and IS reachable (all three sort-type bits set) from a corrupted word.
- `seasonSortAlphabetically(i1, i2) = i1.anime.title.lowercase().compareToWithCollator(i2.anime.title.lowercase())`.

## 7. Missing items

`List<Double>.missingChaptersCount()` (`MissingChapters.kt:6-38`) and `missingEntriesCount()` (`MissingEpisodes.kt:6-38`; the episode file is the only user for anime AND for season numbers, `AnimeScreen.kt:545-548`): algorithm

```
if list empty -> 0
items = list.filter(it != -1.0).map(toInt  /* truncation toward zero */).distinct().sorted()
if items empty -> 0
missing = 0; previous = 0
for current in items:
    if current > previous + 1: missing += current - previous - 1
    previous = current
return missing
```

Decimals collapse (1.1, 1.5, 1.99 all become 1). The counting starts from 0, so a list starting at 5 reports 4 missing even when the source simply begins at chapter 5 (OBSERVED; the UI shows "Missing N chapters" via `MR.plurals.missing_chapters` = "Missing %1$s chapter" / "Missing %1$s chapters", and for anime `AYMR.plurals.missing_items` = "Missing %1$s item" / "Missing %1$s items"). Only exactly `-1.0` is ignored: -2.0 or -0.5 become `-2`/`0` integers and are counted as numbers (-2 is less than 0+1 so adds nothing; 0 adds nothing).

`calculateChapterGap(higher: Chapter?, lower: Chapter?)`: 0 when either is null or either `!isRecognizedNumber`; else `calculateChapterGap(higher.chapterNumber, lower.chapterNumber)`. `calculateChapterGap(higherNum, lowerNum)`: 0 if either `< 0.0`; else `floor(higher).toInt() - floor(lower).toInt() - 1` (can be negative when the list is not strictly descending, e.g. higher 5.0 lower 7.0 gives -3; callers only show a gap when `> 0`). Callers: the entry list inserts a `MissingCount` separator between consecutive rows (`MangaScreenModel.kt:1170-1190`, `AnimeScreenModel.kt:1640-1660`): for the LAST row of the list (no lower neighbour) the count is `floor(higher.number).toInt() - 1` clamped at 0, which is why a list ending at chapter 5 shows "4 missing" at the bottom, and for all other pairs `calculateChapterGap(higher, lower)`; separators only appear when the count is `> 0`; the separator id is `"{lowerId}-{higherId}"`. The text is `MR.plurals.missing_chapters_warning` = "Skipping %d chapter, either the source is missing it or it has been filtered out" (one) / "Skipping %d chapters, either the source is missing them or they have been filtered out" (other). The reader uses the other overload (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/MissingChapters.kt:7`, on `ReaderChapter`) for transition pages (`ChapterTransition.kt:68,80`) and pager/webtoon adapters (`> 0` check). The Episode twin is `calculateEpisodeGap`.

## 8. "Should the DB row be updated from the source?"

`ShouldUpdateDbChapter.await(db, source)` (`ShouldUpdateDbChapter.kt:7-13`) = `db.scanlator != source.scanlator || db.name != source.name || db.dateUpload != source.dateUpload || db.chapterNumber != source.chapterNumber || db.sourceOrder != source.sourceOrder`. Episode version (`ShouldUpdateDbEpisode.kt:7-17`) adds `|| summary != || fillermark != || previewUrl != || memo !=` (nine comparisons: scanlator, name, dateUpload, episodeNumber, sourceOrder, summary, fillermark, previewUrl, memo). Season version (`ShouldUpdateDbSeason.kt:6-10`) on two `Anime` rows: `title != || seasonNumber != || seasonSourceOrder !=` (memo is NOT compared even though `SyncSeasonsWithSource` copies it when updating). `Double` comparison uses `!=` on primitives (IEEE: `NaN != NaN` is true; irrelevant as numbers come from parsing). Strings compare exactly (no trimming, nullable equality: `null != ""` is true). Quirk Q1: a source that reports `dateUpload = 0` for a chapter always differs from the DB row (the sync stores `now` for new rows), so every refresh marks the row "changed" and bumps `last_update`; see p1-topic-quirks-and-decisions.md.

## 9. Fetch interval ("smart update")

Summary; full algorithm, worked examples and quirks in p1-topic-fetch-interval.md.

`toMangaUpdate(manga, dateTime, window)`:
1. `interval = if (manga.fetchInterval < 0) manga.fetchInterval else calculateInterval(getChaptersByMangaId.await(manga.id, applyScanlatorFilter = true), dateTime.zone)`. (Anime: `getEpisodesByAnimeId.await(anime.id)`.) A negative stored value is a user override and is kept (and passed on as negative).
2. `currentWindow = if (window == (0, 0)) getWindow(ZonedDateTime.now()) else window`.
3. `nextUpdate = calculateNextUpdate(manga, interval, dateTime, currentWindow)`.
4. Return `MangaUpdate(id, nextUpdate, fetchInterval = interval)`.

`getWindow(dateTime)`: `today = dateTime.toLocalDate().atStartOfDay(dateTime.zone)`; `lower = today - 1 day`; `upper = today + 1 day`; returns `(lower.epochSecond * 1000, upper.epochSecond * 1000 - 1)`.

`calculateInterval(items, zone)`: window size `3` if `items.size <= 8` else `10`; take upload dates (items with `dateUpload > 0`, sorted descending, mapped to local calendar day, distinct, first `window`) and fetch dates (all items sorted by `dateFetch` descending, same mapping); if `uploadDates.size >= 3` use `floorDiv(daysBetween(last, first), index(last))`, else if `fetchDates.size >= 3` the same with fetch dates, else 7; clamp to `1..28`.

`calculateNextUpdate`: return `nextUpdate` unchanged if `window.first <= nextUpdate <= window.second + 1`; otherwise `latestDate` = local start of day of `lastUpdate` (or of `now` when `lastUpdate <= 0`), `timeSinceLatest` = whole days from `latestDate` to `dateTime`, `cycle = floorDiv(timeSinceLatest, |interval| if interval < 0 else increaseInterval(interval, timeSinceLatest, 10))`, result `= (latestDate + (cycle + 1) * |interval| days)` as epoch ms using `dateTime`'s UTC offset.

`SyncChaptersWithSource` (area:10-app-domain-layer) calls `awaitUpdateFetchInterval` (a) when nothing changed AND (`manualFetch || manga.fetchInterval == 0 || manga.nextUpdate < fetchWindow.first`), and (b) always after a change, then bumps `lastUpdate = now`. The library updater skips entries whose `nextUpdate > fetchWindowUpperBound` when the restriction `ENTRY_OUTSIDE_RELEASE_PERIOD` ("manga_outside_release_period") is set. User override: the details screen's SetInterval dialog writes `fetchInterval = -interval` (`MangaScreenModel.kt:405-417`, `interval` 0..28 from the wheel; 0 is shown as `MR.strings.label_default` "Default" and, being `-0 = 0`, restores automatic mode).

## 10. Seasons (anime only)

OBSERVED model: an anime with `fetchType == Seasons` is a parent; its children are other `animes` rows whose `parent_id` equals the parent id. `SeasonAnime` is the child plus episode statistics (counts and dates from the `animes` view with episode aggregates, area:05-data-repos-entries-items `p1-topic-library-and-season-views.md`). `SyncSeasonsWithSource` (area:10-app-domain-layer) builds children with `NetworkToLocalAnime` + `parentId` + `seasonSourceOrder`, recognises the season number with `SeasonRecognition.parseSeasonNumber(parentTitle, seasonTitle, seasonNumber)` and decides updates with `ShouldUpdateDbSeason`. Removed seasons are detached with `removeParentIdByIds(ids)` (their `parent_id` becomes NULL; the rows stay, possibly as library entries). `NoSeasonsException` is thrown when the source returns an empty list and the source is not local; the details screen shows `AYMR.strings.no_seasons_error` = "No seasons found" in a snackbar (`AnimeScreenModel.kt:353-355`); `NoEpisodesException` -> `AYMR.strings.no_episodes_error` = "No episodes found"; `NoChaptersException` in `MangaScreenModel.kt:587` -> `MR.strings.no_chapters_error` = "No chapters found". Elsewhere (`ExceptionFormatter.kt:28-30`) `NoChaptersException`/`NoEpisodesException` format as `MR.strings.no_results_found` = "No results found" (library-update error log, global error dialogs); `NoSeasonsException` is not special-cased there and formats as `"NoSeasonsException: null"` (class simple name + message).

`SeasonAnime.toLibraryAnime()` is used to show seasons in the library-style grid component (category -1 = no category).

## 11. Exceptions, messages, side effects summary

| Event | Exception / outcome | User-visible text |
|---|---|---|
| Source returned zero chapters (non-local) | `NoChaptersException` | snackbar "No chapters found"; library update log "No results found" |
| Zero episodes | `NoEpisodesException` | "No episodes found" / "No results found" |
| Zero seasons | `NoSeasonsException` | "No seasons found" |
| Repository failure in `Get*`/`Update*` interactors | swallowed, `logcat(ERROR)` | none |
| `GetChapterByUrlAndMangaId` / `GetEpisodeByUrlAndAnimeId` failure | swallowed silently | none |
| `insertManga` returns null | `NullPointerException` from `!!` | generic error from caller |
| `getSeasonSortComparator` with 0xE000 | `NotImplementedError` | crash in the comparator (unhandled) |

None of the scope's interactors posts notifications, schedules work or writes files. DB writes happen only through the repositories listed in section 4.

## 12. Concurrency and dispatchers

OBSERVED. Interactors are `suspend` and do not switch dispatchers themselves (repository handlers use `Dispatchers.IO`, area:05-data-repos-entries-items). `withNonCancellableContext` (default-flags writers) prevents cancellation of the multi-entry loop. `combine` (`GetMangaWithChapters`, `GetAnimeWithEpisodesAndSeasons`) emits only after every upstream emitted at least once and then on every upstream emission, so the details screen re-renders when ANY of the manga row, chapter rows or season rows change (chapter read/bookmark toggles re-emit the whole list).

Proposal: `tokio` task for repository awaits; GTK widgets are only touched on the main loop through `glib::MainContext::channel`/`spawn_local` bridge from area:01-foundation-app-bootstrap `T-01a-009`; `combine` becomes a task that holds `watch::Receiver`s and emits `Arc<(Manga, Vec<Chapter>)>` into a new `watch` channel (design in p1-rust-design.md section 6).

## 13. Application release check (in scope only through `GetApplicationReleaseTest`)

`GetApplicationRelease.await(arguments)`: unless `arguments.forceCheck`, return `NoNewUpdate` when `now < lastChecked + 3 days` (without calling the service); ask `service.latest(arguments)`; `null` -> `NoNewUpdate`; store `lastChecked = now`; compare versions (`isNewVersion`) and return `NewUpdate(release)` or `NoNewUpdate`. The `OsTooOld` result exists but is never produced. Version compare quirks (`NumberFormatException` on non-numeric tags, `IndexOutOfBoundsException` when the new version has fewer components, "any greater component anywhere wins") are in p1-topic-release-check.md.

## 14. Library flags (in scope only through `LibraryFlagsTest`)

The test exercises `domain/src/main/java/tachiyomi/domain/library/model/Flag.kt` (`Flag`, `Mask`, `FlagWithMask`, `Long.contains`, `Long.plus`, `Flag.plus`), `LibraryDisplayMode` and `MangaLibrarySort`/`AnimeLibrarySort`, which are owned by area:08-domain-library-category-history-updates. The exact numbers that make the test pass are reproduced in p1-edge-cases-and-tests.md section 3.5 so the test can be ported without area 08's files being open.
