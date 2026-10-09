# 38 part 2: behavior specification (manga details ScreenModel, tracking sub-screens, chapter utilities)

All citations are repo-relative to `/home/blackbox/aniyomi-src`. `MSM` = `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt`. `MTID` = `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt`.

OBSERVED = what the Kotlin does today. PROPOSAL = what the Rust port does. Proposals are labelled "Proposal:".

Mihon/Aniyomi terminology: "chapter" here is the manga item. The screen model is shared by the details page, the chapter list, and the chapter settings sheet.

---

## 1. Constants and enums (exact values)

### 1.1 Manga chapter flags (domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:77-103)

| Constant | Value | Mask |
|---|---|---|
| `SHOW_ALL` | `0x00000000L` | (generic "no filter") |
| `CHAPTER_SORT_DESC` | `0x00000000L` | `CHAPTER_SORT_DIR_MASK = 0x00000001L` |
| `CHAPTER_SORT_ASC` | `0x00000001L` | |
| `CHAPTER_SHOW_UNREAD` | `0x00000002L` | `CHAPTER_UNREAD_MASK = 0x00000006L` |
| `CHAPTER_SHOW_READ` | `0x00000004L` | |
| `CHAPTER_SHOW_DOWNLOADED` | `0x00000008L` | `CHAPTER_DOWNLOADED_MASK = 0x00000018L` |
| `CHAPTER_SHOW_NOT_DOWNLOADED` | `0x00000010L` | |
| `CHAPTER_SHOW_BOOKMARKED` | `0x00000020L` | `CHAPTER_BOOKMARKED_MASK = 0x00000060L` |
| `CHAPTER_SHOW_NOT_BOOKMARKED` | `0x00000040L` | |
| `CHAPTER_SORTING_SOURCE` | `0x00000000L` | `CHAPTER_SORTING_MASK = 0x00000300L` |
| `CHAPTER_SORTING_NUMBER` | `0x00000100L` | |
| `CHAPTER_SORTING_UPLOAD_DATE` | `0x00000200L` | |
| `CHAPTER_SORTING_ALPHABET` | `0x00000300L` | |
| `CHAPTER_DISPLAY_NAME` | `0x00000000L` | `CHAPTER_DISPLAY_MASK = 0x00100000L` |
| `CHAPTER_DISPLAY_NUMBER` | `0x00100000L` | |

Default value of `chapterFlags` for `Manga.create()`: `0L` (sort = SOURCE, direction = DESC, no filters, display = NAME).

Derived properties (Manga.kt:37-73):
- `sorting = chapterFlags and 0x300`
- `displayMode = chapterFlags and 0x100000`
- `unreadFilterRaw = chapterFlags and 0x6`, `downloadedFilterRaw = chapterFlags and 0x18`, `bookmarkedFilterRaw = chapterFlags and 0x60`
- `unreadFilter`: raw 0x2 -> `ENABLED_IS`; raw 0x4 -> `ENABLED_NOT`; else `DISABLED`
- `bookmarkedFilter`: raw 0x20 -> `ENABLED_IS`; raw 0x40 -> `ENABLED_NOT`; else `DISABLED`
- `sortDescending()`: `chapterFlags and 0x1 == 0x0` (i.e. `CHAPTER_SORT_DESC`).
- `expectedNextUpdate`: not used here.

### 1.2 Downloaded filter (app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:23-31)

```
val Manga.downloadedFilter: TriState
  if (BasePreferences.downloadedOnly().get()) return ENABLED_IS      // forced, regardless of flags
  when (downloadedFilterRaw) { 0x8 -> ENABLED_IS; 0x10 -> ENABLED_NOT; else -> DISABLED }
```
`BasePreferences.downloadedOnly` key `Preference.appStateKey("pref_downloaded_only")`, default `false` (app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:16-19). The effective preference key string is the app-state key form produced by `Preference.appStateKey`; see p2-data-and-config.md.

`Manga.chaptersFiltered()` (Manga.kt:32-36) = `unreadFilter != DISABLED || downloadedFilter != DISABLED || bookmarkedFilter != DISABLED`. Note it uses the forced `downloadedFilter`.

### 1.3 TriState (domain/src/main/java/tachiyomi/domain/entries/TriState.kt; core: core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt)

`enum TriState { DISABLED, ENABLED_IS, ENABLED_NOT }` with `next()`: DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED.

`applyFilter(filter, predicate)` (TriState.kt:5-9):
- DISABLED -> `true` (keep)
- ENABLED_IS -> `predicate()`
- ENABLED_NOT -> `!predicate()`

### 1.4 ChapterSwipeAction (domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:391-396)

`enum ChapterSwipeAction { ToggleRead, ToggleBookmark, Download, Disabled }`

### 1.5 AutoTrackState (app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt:6-10)

| Enum | titleRes |
|---|---|
| `ALWAYS` | `MR.strings.lock_always` ("Always") |
| `ASK` | `MR.strings.default_category_summary` ("Always ask") |
| `NEVER` | `MR.strings.lock_never` ("Never") |

### 1.6 DownloadAction (app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt:3-9)

`enum DownloadAction { NEXT_1_ITEM, NEXT_5_ITEMS, NEXT_10_ITEMS, NEXT_25_ITEMS, UNVIEWED_ITEMS }`

### 1.7 ChapterDownloadAction (app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt:42-47)

`enum ChapterDownloadAction { START, START_NOW, CANCEL, DELETE }`

### 1.8 MangaDownload.State (app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt:67-73)

`enum State(val value: Int) { NOT_DOWNLOADED(0), QUEUE(1), DOWNLOADING(2), DOWNLOADED(3), ERROR(4) }`

### 1.9 Dialog kinds (MSM:1094-1106)

`sealed interface Dialog`:
- `ChangeCategory(manga: Manga, initialSelection: ImmutableList<CheckboxState<Category>>)`
- `DeleteChapters(chapters: List<Chapter>)`
- `DuplicateManga(manga: Manga, duplicate: Manga)`
- `Migrate(newManga: Manga, oldManga: Manga)`
- `SetMangaFetchInterval(manga: Manga)`
- `SettingsSheet` (data object)
- `TrackSheet` (data object)
- `FullCover` (data object)

Rendering of each dialog is in MangaScreen.kt (part 1 owns the UI).

---

## 2. Constructor dependencies and captured preferences (MSM:94-151)

Constructor parameters (all Injekt-resolved with defaults): `context`, `lifecycle`, `mangaId: Long`, `isFromSource: Boolean`, `libraryPreferences`, `trackPreferences`, `readerPreferences`, `trackerManager`, `trackChapter`, `downloadManager`, `downloadCache`, `getMangaAndChapters`, `getDuplicateLibraryManga`, `getAvailableScanlators`, `getExcludedScanlators`, `setExcludedScanlators`, `setMangaChapterFlags`, `setMangaDefaultChapterFlags`, `setReadStatus`, `updateChapter`, `updateManga`, `syncChaptersWithSource`, `getCategories`, `getTracks`, `addTracks`, `setMangaCategories`, `mangaRepository`, `filterChaptersForDownload`, `snackbarHostState` (default `SnackbarHostState()`).

Captured once at construction (not reactive):
- `chapterSwipeStartAction = libraryPreferences.swipeChapterEndAction().get()` (MSM:144). Note the naming inversion, see section 2.1.
- `chapterSwipeEndAction = libraryPreferences.swipeChapterStartAction().get()` (MSM:145).
- `autoTrackState = trackPreferences.autoUpdateTrackOnMarkRead().get()` (MSM:146). It is a `var`, never reassigned in this file.
- `isUpdateIntervalEnabled = ENTRY_OUTSIDE_RELEASE_PERIOD in libraryPreferences.autoUpdateItemRestrictions().get()` (MSM:150-151). Value is a `Boolean`.

Reactive:
- `skipFiltered` = `readerPreferences.skipFiltered().asState(screenModelScope)` (MSM:148). Used only by `getUnreadChapters()`.

### 2.1 Swipe-action preference key inversion (OBSERVED)

LibraryPreferences.kt:369-375:
- `swipeChapterStartAction()` reads key `pref_chapter_swipe_end_action`, default `ToggleRead`.
- `swipeChapterEndAction()` reads key `pref_chapter_swipe_start_action`, default `ToggleBookmark`.

MSM:144-145 assigns `chapterSwipeStartAction = swipeChapterEndAction()` (key `pref_chapter_swipe_start_action`, default `ToggleBookmark`) and `chapterSwipeEndAction = swipeChapterStartAction()` (key `pref_chapter_swipe_end_action`, default `ToggleRead`). The model's names match the swipe direction (start = swipe from start edge), so the keys are consistently named from the opposite edge. The Rust port must keep the stored keys and map by swipe direction, not by key name. See p2-data-and-config.md.

---

## 3. Lifecycle of the model (init, load, refresh)

### 3.1 init (MSM:173-256)

1. Launch IO: `combine(getMangaAndChapters.subscribe(mangaId, applyScanlatorFilter = true).distinctUntilChanged(), downloadCache.changes, downloadManager.queueState) { mangaAndChapters, _, _ -> mangaAndChapters }`, `flowWithLifecycle(lifecycle)`, `collectLatest { (manga, chapters) -> updateSuccessState { copy(manga, chapters = chapters.toChapterListItems(manga)) } }` (MSM:175-189). Each emission rebuilds ALL `ChapterList.Item`s (download state, selection) from the DB list.
   - `subscribe` is `combine(mangaRepository.getMangaByIdAsFlow(id), chapterRepository.getChapterByMangaIdAsFlow(id, applyScanlatorFilter))` (domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetMangaWithChapters.kt:15-21).
2. Launch IO: excluded scanlators flow -> `copy(excludedScanlators)` (MSM:191-200).
3. Launch IO: available scanlators flow -> `copy(availableScanlators)` (MSM:202-211).
4. `observeDownloads()` (MSM:213).
5. Launch IO initial load (MSM:215-255):
   - `manga = getMangaAndChapters.awaitManga(mangaId)` (repository `getMangaById`; GetMangaWithChapters.kt:24-26).
   - `chapters = getMangaAndChapters.awaitChapters(mangaId, applyScanlatorFilter = true).toChapterListItems(manga)`.
   - If `!manga.favorite`: `setMangaDefaultChapterFlags.await(manga)` (MSM:220-222). OBSERVED: every open of a non-library manga overwrites its chapter flags (filters, sort, display) with the saved library defaults. See section 7.4.
   - `needRefreshInfo = !manga.initialized`; `needRefreshChapter = chapters.isEmpty()` (MSM:224-225).
   - `mutableState.update { State.Success(<the fields listed below>) }` (MSM:228-239) with `source = Injekt<MangaSourceManager>().getOrStub(manga.source)`, `isFromSource`, `chapters`, `availableScanlators = getAvailableScanlators.await(mangaId)`, `excludedScanlators = getExcludedScanlators.await(mangaId)`, `isRefreshingData = needRefreshInfo || needRefreshChapter`, `dialog = null`. Defaults: `trackingCount = 0`, `hasLoggedInTrackers = false`, `hasPromptedToAddBefore = false`.
   - `observeTrackers()` (MSM:242). Tracking flows start only after Success exists.
   - If `screenModelScope.isActive`: `awaitAll(async { if (needRefreshInfo) fetchMangaFromSource() }, async { if (needRefreshChapter) fetchChaptersFromSource() })` (MSM:245-251). Both use the default `manualFetch = false`.
   - `updateSuccessState { copy(isRefreshingData = false) }` (MSM:254).

OBSERVED consequences:
- `observeDownloads()` runs before Success exists. `updateSuccessState` is a no-op in `Loading`, and the download flows filter on `successState?.manga?.id`, so early download events are dropped. This is safe because the first `toChapterListItems` pass reads the current queue (`getQueuedDownloadOrNull`) and the download cache.
- `chapters` is computed before `setMangaDefaultChapterFlags.await(manga)`. The state `manga` is therefore the pre-default copy until the first DB emission arrives. The filters/sort applied on first paint can differ from the flags in DB for one frame.
- If `getMangaById` throws (manga row missing), the coroutine fails inside `launchIO` and the state stays `Loading` forever. There is no error state. See p2-edge-cases-and-tests.md E-LOAD-1.

### 3.2 fetchAllFromSource (MSM:258-268)

`fetchAllFromSource(manualFetch: Boolean = true)`:
1. `launch` (main scope, not IO): `isRefreshingData = true`.
2. `awaitAll(async { fetchMangaFromSource(manualFetch) }, async { fetchChaptersFromSource(manualFetch) })`.
3. `isRefreshingData = false`.

Note the default is `manualFetch = true` here, unlike init (`false`). That default is what enables auto-download of new chapters (section 6.3).

### 3.3 fetchMangaFromSource (MSM:275-291)

`private suspend fun fetchMangaFromSource(manualFetch: Boolean = false)`:
1. `state = successState ?: return`.
2. `withIOContext { networkManga = state.source.getMangaDetails(state.manga.toSManga()); updateManga.awaitUpdateFromSource(state.manga, networkManga, manualFetch) }`.
3. On any `Throwable e`:
   - If `e is HttpException && e.code == 103` return silently. Comment: "Ignore early hints errors that aren't handled by OkHttp". HTTP 103 is "Early Hints", not an error.
   - Otherwise `logcat(ERROR, e)` and `screenModelScope.launch { snackbarHostState.showSnackbar(message = with(context) { e.formattedMessage }) }` (no action label, so default duration, see 9.1).

`toSManga()` copies `url, title, artist, author, description, genre = genre.orEmpty().joinToString()`, `status.toInt()`, `thumbnail_url`, `initialized` (app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:38-48). Note `genre` becomes one comma-joined string, which is then split by `SManga.getGenres()` on the way back (see section 6.1 in the data doc).

`UpdateManga.awaitUpdateFromSource` behavior is area:07 (app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt:29-77). Summary for porting references: title is only overwritten when the manga is NOT a favorite and the remote title is non-empty; cover is re-fetched when the thumbnail URL changes or on manual fetch; `initialized = true` is always written.

### 3.4 fetchChaptersFromSource (MSM:569-600)

`private suspend fun fetchChaptersFromSource(manualFetch: Boolean = false)`:
1. `state = successState ?: return`.
2. `withIOContext`:
   - `chapters = state.source.getChapterList(state.manga.toSManga())`.
   - `newChapters = syncChaptersWithSource.await(chapters, state.manga, state.source, manualFetch)`.
   - If `manualFetch`: `downloadNewChapters(newChapters)` (section 6.3).
3. On `Throwable e`:
   - Message = `context.stringResource(MR.strings.no_chapters_error)` ("No chapters found") if `e is NoChaptersException`; otherwise `logcat(ERROR, e)` and `with(context) { e.formattedMessage }`.
   - `screenModelScope.launch { snackbarHostState.showSnackbar(message) }`.
   - `newManga = mangaRepository.getMangaById(mangaId)`; `updateSuccessState { copy(manga = newManga, isRefreshingData = false) }`.

`NoChaptersException` is thrown by sync when the source returns an empty list and the source is not local (SyncChaptersWithSource.kt:17-19). For a local source, empty is valid.

OBSERVED: on success, `isRefreshingData` is not reset here, but both callers reset it afterwards.

---

## 4. Favorite / library membership

### 4.1 toggleFavorite() (MSM:293-309)

Entry point from the UI button. Calls `toggleFavorite(onRemoved = <callback described below>)` with `checkDuplicate = true`. The callback is the banner logic of the next paragraph.

`onRemoved` (called only after successful removal from library):
1. `launch { if (!hasDownloads()) return; result = snackbarHostState.showSnackbar(message = MR.strings.delete_downloads_for_manga, actionLabel = MR.strings.action_delete, withDismissAction = true); if (result == ActionPerformed) deleteDownloads() }`.
   - Strings: "Delete downloaded chapters?" / "Delete". Snackbar has an action so default duration is `Indefinite` (see 9.1).

### 4.2 toggleFavorite(onRemoved, checkDuplicate) (MSM:314-380)

`val state = successState ?: return`. Then `launchIO`:

**Removing (isFavorited == manga.favorite == true)** (MSM:322-330):
1. `updateManga.awaitUpdateFavorite(manga.id, false)`. This writes `favorite = false` and `dateAdded = 0` (UpdateManga.kt:97-105).
2. If true:
   - `if (manga.removeCovers() != manga) updateManga.awaitUpdateCoverLastModified(manga.id)`.
   - `removeCovers()` (app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt:43-51): if local -> unchanged. Otherwise `coverCache.deleteFromCache(this, true) > 0` deletes cached cover files (side effect) and returns a copy with `coverLastModified = now()`; if nothing was deleted, returns `this`.
   - `withUIContext { onRemoved() }`.
3. If `awaitUpdateFavorite` returned false: nothing else happens (no snackbar, no callback).

**Adding (isFavorited == false)** (MSM:331-378):
1. If `checkDuplicate`:
   - `duplicate = getDuplicateLibraryManga.await(manga).getOrNull(0)` (list is ordered by the SQL result order; index 0 only).
   - SQL (data/src/main/sqldelight/data/mangas.sq:117-122): `SELECT * FROM mangas WHERE favorite = 1 AND LOWER(title) = :title AND _id != :id`, with `:title = manga.title.lowercase()` (Kotlin `lowercase()`, locale-invariant). Duplicates are matched across sources.
   - If found: `updateSuccessState { copy(dialog = Dialog.DuplicateManga(manga, duplicate)) }` and `return@launchIO`. Nothing is written to the DB. The user is then offered a choice (DuplicateMangaDialog, part 1).
2. `categories = getCategories()` (MSM:440-442, returns `getCategories.await().filterNot { it.isSystemCategory }`). `isSystemCategory = id == 0L` (Category.kt:13, `UNCATEGORIZED_ID = 0L`). Virtual "Default" category is therefore excluded from this list.
3. `defaultCategoryId = libraryPreferences.defaultMangaCategory().get().toLong()` (key `default_category`, Int default `-1`, LibraryPreferences.kt:154, companion :413).
4. `defaultCategory = categories.find { it.id == defaultCategoryId }`.
5. Routing (MSM:351-371), first match wins:
   - **A. `defaultCategory != null`** (a real category is the default): `awaitUpdateFavorite(manga.id, true)`; if false -> return; `moveMangaToCategory(defaultCategory)` (sets membership to exactly `[defaultCategory.id]`).
   - **B. `defaultCategoryId == 0L || categories.isEmpty()`** (Default/Uncategorized, or no categories exist): `awaitUpdateFavorite(true)`; if false -> return; `moveMangaToCategory(null)`, which calls `moveMangaToCategories(emptyList())` (membership cleared).
   - **C. otherwise** (default id points at a deleted category, or no default set and categories exist): `isFromChangeCategory = true`; `showChangeCategoryDialog()`. Favorite is NOT written yet. It is written only if the user confirms (see 4.4).
6. `addTracks.bindEnhancedTrackers(manga, state.source)` (MSM:374). Runs in all three branches (in branch C before the user has picked categories). See section 5.5.
7. `if (autoOpenTrack) showTrackDialog()` (MSM:375-377). `autoOpenTrack = successState?.trackingAvailable == true && trackPreferences.trackOnAddingToLibrary().get()` (MSM:158-159). `trackOnAddingToLibrary` key `track_on_adding_to_library`, default `true` (TrackPreferences.kt:41). `trackingAvailable = trackingCount > 0` (MSM:1198-1199). OBSERVED: `trackingCount` is the count of already-linked, logged-in tracks, and it is updated asynchronously by `observeTrackers`, so this check can read a stale count right after the add.

Note on branch C: `isFromChangeCategory` is set to `true` here (MSM:368) and reset by the UI. MangaScreen.kt:176-184 `onDismissRequest` runs `dismissDialog()` and then, if `autoOpenTrack && isFromChangeCategory`, resets the flag and calls `showTrackDialog()`. So the tracking sheet opens right after the category dialog closes, but only for a first-time add that went through branch C and only when trackers are linked and `track_on_adding_to_library` is on. The same flag is also read by AnimeScreen.kt:234 for the anime side. Proposal: model this as a one-shot "open tracker after category dialog" intent in the Rust state machine (`PendingAfterCategoryDialog::OpenTracker`), not as a shared mutable flag.

### 4.3 showChangeCategoryDialog() (MSM:382-396)

`manga = successState?.manga ?: return`. `launch`:
- `categories = getCategories()` (non-system categories).
- `selection = getMangaCategoryIds(manga)` = `getCategories.await(manga.id).map { it.id }` (MSM:450-453). The KDoc says "if none returns default id", but the code returns an empty list when the manga has no categories. OBSERVED: KDoc mismatch, behavior is "empty list".
- `updateSuccessState { copy(dialog = Dialog.ChangeCategory(manga, initialSelection = categories.mapAsCheckboxState { it.id in selection }.toImmutableList())) }`.
- `mapAsCheckboxState` (core/common, CheckboxState.kt): each category becomes `CheckboxState.State.Checked` if in selection, else `CheckboxState.State.None` (two-state checkbox model: `next()` flips Checked and None; the tri-state `CheckboxState.TriState` is not used here; the user toggles in the dialog).

### 4.4 Confirming categories (moveMangaToCategoriesAndAddToLibrary, MSM:455-462)

`moveMangaToCategoriesAndAddToLibrary(manga: Manga, categories: List<Long>)`:
1. `moveMangaToCategory(categories)` (private overload for `List<Long>`, MSM:474-478): `launchIO { setMangaCategories.await(mangaId, categoryIds) }`.
2. `if (manga.favorite) return`.
3. `launchIO { updateManga.awaitUpdateFavorite(manga.id, true) }`.

OBSERVED race: steps 1 and 3 launch two independent coroutines on IO. Their DB writes are not ordered. Proposal: perform both writes in one transaction (see p2-rust-design.md section 5.4).

The MangaScreen dialog that calls this (part 1) sends the selected IDs. The dialog can also be dismissed (`dismissDialog`), in which case no DB write happens and the manga stays non-favorite (branch C).

### 4.5 setFetchInterval (MSM:405-417)

`showSetMangaFetchIntervalDialog()` sets `Dialog.SetMangaFetchInterval(manga)` (MSM:398-403).

`setFetchInterval(manga: Manga, interval: Int)` (called by the dialog):
- `launchIO`: `updateManga.awaitUpdateFetchInterval(manga.copy(fetchInterval = -interval))`.
  - Custom intervals are stored NEGATIVE: comment "Custom intervals are negative" (MSM:409).
  - `awaitUpdateFetchInterval` -> `MangaFetchInterval.toMangaUpdate` (domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt:17-34): `interval = manga.fetchInterval.takeIf { it < 0 } ?: calculateInterval(chapters, zone)`. So a negative value is kept as given; a value of `0` (when `interval == 0`) is NOT negative and falls through to automatic calculation.
- If the update returned true: `updatedManga = mangaRepository.getMangaById(manga.id)`; `updateSuccessState { copy(manga = updatedManga) }`.

Unit of `interval`: the dialog passes days (see MangaFetchInterval.calculateInterval, area:07). OBSERVED: `interval = 0` is a silent reset to automatic.

---

## 5. Tracking sheet state (inside the details screen)

### 5.1 observeTrackers() (MSM:1063-1090)

`manga = successState?.manga ?: return`. `launchIO`:
- `combine(getTracks.subscribe(manga.id).catch { logcat(ERROR, it) }, trackerManager.loggedInTrackersFlow())`:
  - `supportedTrackers = loggedInTrackers.filter { (it as? EnhancedMangaTracker)?.accept(source!!) ?: true }`.
  - `supportedTrackerIds = supportedTrackers.map { it.id }.toHashSet()`.
  - `supportedTrackerTracks = mangaTracks.filter { it.trackerId in supportedTrackerIds }`.
  - emits `supportedTrackerTracks.size to supportedTrackers.isNotEmpty()`.
- `flowWithLifecycle(lifecycle)`, `distinctUntilChanged()`, `collectLatest { (trackingCount, hasLoggedInTrackers) -> copy(trackingCount, hasLoggedInTrackers) }`.

`loggedInTrackersFlow()` = combine of `isLoggedInFlow` per tracker (TrackerManager.kt:46-51). `accept(source)` is the enhanced-tracker source filter (EnhancedMangaTracker.kt:16-...).

### 5.2 Tracker-side calls used by the details screen

| Call site (MSM) | Function | Behavior |
|---|---|---|
| `refreshTrackers()` (MSM:797-816) | `RefreshMangaTracks.await(mangaId)` | Returns `List<Pair<Tracker?, Throwable>>` of FAILED refreshes only (RefreshMangaTracks.kt:25-45). For each failure: `logcat(ERROR)` with `track!!.id`; toast `MR.strings.track_error` with `(track!!.name, e.message ?: "")` -> "%1$s error: %2$s". |
| `markChaptersRead` | `TrackChapter.await(context, mangaId, maxChapterNumber)` | Section 6.5. |
| `addTracks.bindEnhancedTrackers` | AddMangaTracks | Section 4.2 step 6. |

Note `refreshTrackers()` in MSM has a default parameter `refreshTracks: RefreshMangaTracks = Injekt.get()` (MSM:797-799). The MTID version (`MTID:240-260`) has the same behavior but with `track!!.name` in the log message and `Injekt.get<Application>()` as the toast context.

---

## 6. Chapter list: items, sorting, filtering, grouping

### 6.1 toChapterListItems(manga) (MSM:533-564)

For each `Chapter` in the DB order received (no sorting here):
- `isLocal = manga.isLocal()` (`source == LocalMangaSource.ID`, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:385).
- If `isLocal`: `activeDownload = null`, `downloaded = true`.
- Else:
  - `activeDownload = downloadManager.getQueuedDownloadOrNull(chapter.id)` (first queue entry with that chapter id; MangaDownloadManager.kt:109-111).
  - `downloaded = downloadManager.isChapterDownloaded(chapter.name, chapter.scanlator, manga.title, manga.source)`. `skipCache` defaults to `false` (cache-backed check; MangaDownloadCache.kt:136-...).
- `downloadState`:
  1. `activeDownload != null` -> `activeDownload.status` (one of QUEUE, DOWNLOADING, ERROR, NOT_DOWNLOADED can appear here)
  2. else `downloaded` -> `DOWNLOADED`
  3. else -> `NOT_DOWNLOADED`
- `downloadProgress = activeDownload?.progress ?: 0`.
- `selected = chapter.id in selectedChapterIds`.

Local manga therefore always reports `DOWNLOADED` and cannot be in queue.

### 6.2 Live download updates (MSM:493-531)

`observeDownloads()` launches two IO collectors:
- `downloadManager.statusFlow()`, `filter { it.manga.id == successState?.manga?.id }`, `catch` logs, `flowWithLifecycle`, `collect { withUIContext { updateDownloadState(it) } }`.
- `downloadManager.progressFlow()`, same shape.

`updateDownloadState(download)` (MSM:519-531): finds the index of `download.chapter.id` in `chapters`. If not found, no change. Otherwise removes the item and re-inserts `copy(downloadState = download.status, downloadProgress = download.progress)` at the SAME index. Other fields (including `selected`) are preserved. This mutates the UI list only, not the DB.

`MangaDownloadManager.statusFlow()` (MangaDownloadManager.kt:431-443): flatMapLatest over the queue; emits each download's status changes (`drop(1)`), plus an initial emission of every download whose status is DOWNLOADING.

`progressFlow()` (MangaDownloadManager.kt:445): same shape as `statusFlow()` but uses `progressFlow` of each download, with `drop(1)` and the same `onStart` seed of DOWNLOADING items.

### 6.3 Filtering pipeline: applyFilters (private, MSM:1205-1220)

`List<ChapterList.Item>.applyFilters(manga): Sequence<ChapterList.Item>`, in this exact order:
1. `isLocalManga = manga.isLocal()`.
2. `unreadFilter = manga.unreadFilter`, `downloadedFilter = manga.downloadedFilter` (forced if `downloadedOnly`), `bookmarkedFilter = manga.bookmarkedFilter`.
3. `filter { applyFilter(unreadFilter) { !chapter.read } }`.
4. `filter { applyFilter(bookmarkedFilter) { chapter.bookmark } }`.
5. `filter { applyFilter(downloadedFilter) { it.isDownloaded || isLocalManga } }`, where `isDownloaded = downloadState == DOWNLOADED` (ChapterList.kt:1241). Note: a chapter that is QUEUE/DOWNLOADING/ERROR is NOT "downloaded" for this filter.
6. `sortedWith { (c1), (c2) -> getChapterSort(manga).invoke(c1, c2) }`.

`State.Success.processedChapters` (MSM:1157-1159) = `chapters.applyFilters(manga).toList()` (lazy).

The public duplicate `List<ChapterList.Item>.applyFilters(manga)` in app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt:42-52 is byte-for-byte the same logic. Port once.

The `List<Chapter>` variant in the same file (ChapterFilter.kt:16-36) uses `downloadManager.isChapterDownloaded(...)` instead of `downloadState`, and is used by the library and by `getNextUnread` callers outside this file.

### 6.4 Sorting: getChapterSort (domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt:7-30)

`getChapterSort(manga, sortDescending = manga.sortDescending())` returns a comparator by `manga.sorting`:

| sorting | sortDescending == true | sortDescending == false |
|---|---|---|
| `CHAPTER_SORTING_SOURCE` (0x000) | `c1.sourceOrder.compareTo(c2.sourceOrder)` (ASCENDING by sourceOrder) | `c2.sourceOrder.compareTo(c1.sourceOrder)` (DESCENDING by sourceOrder) |
| `CHAPTER_SORTING_NUMBER` (0x100) | `c2.chapterNumber.compareTo(c1.chapterNumber)` | `c1.chapterNumber.compareTo(c2.chapterNumber)` |
| `CHAPTER_SORTING_UPLOAD_DATE` (0x200) | `c2.dateUpload.compareTo(c1.dateUpload)` | `c1.dateUpload.compareTo(c2.dateUpload)` |
| `CHAPTER_SORTING_ALPHABET` (0x300) | `c2.name.compareToWithCollator(c1.name)` | `c1.name.compareToWithCollator(c2.name)` |
| anything else | throws `NotImplementedError("Invalid chapter sorting method: ${manga.sorting}")` | same |

Important: the SOURCE branch looks inverted. With the default `sortDescending == true`, the comparator sorts `sourceOrder` ASCENDING. This is intentional: `sourceOrder` is the index in the source's list (SyncChaptersWithSource.kt:24-31, `mapIndexed { i, ... -> sourceOrder = i }`). Sources list newest first, so index 0 = newest, and ascending sourceOrder = newest first. So "Source, descending" displays newest first.

`compareToWithCollator` (core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt:1-15): `java.text.Collator.getInstance(Locale.getDefault())` with `strength = Collator.PRIMARY` (case-insensitive, accent-insensitive, lazily created once).

`Double.compareTo` uses IEEE total order on Kotlin Double (NaN last). Chapter numbers in DB are never NaN (`-1.0` means unknown).

Mixing of `chapterNumber == -1.0` (unrecognized): NUMBER sort places them first in ascending order and last in descending order. Test vector in section 11.

### 6.5 The read/unread/bookmark filter setters (MSM:878-925)

Each builds a flag from TriState and calls `setMangaChapterFlags.awaitSet*Filter(manga, flag)`, which calls `mangaRepository.updateManga(MangaUpdate(id, chapterFlags = manga.chapterFlags.setFlag(flag, MASK)))` (SetMangaChapterFlags.kt:11-36). `setFlag` is `this and mask.inv() or (flag and mask)` (SetMangaChapterFlags.kt:94-96).

| Setter | Mapping | Mask |
|---|---|---|
| `setUnreadFilter(state)` | DISABLED->`SHOW_ALL` (0); ENABLED_IS->`CHAPTER_SHOW_UNREAD` (0x2); ENABLED_NOT->`CHAPTER_SHOW_READ` (0x4) | `CHAPTER_UNREAD_MASK` 0x6 |
| `setDownloadedFilter(state)` | DISABLED->0; ENABLED_IS->`CHAPTER_SHOW_DOWNLOADED` (0x8); ENABLED_NOT->`CHAPTER_SHOW_NOT_DOWNLOADED` (0x10) | 0x18 |
| `setBookmarkedFilter(state)` | DISABLED->0; ENABLED_IS->`CHAPTER_SHOW_BOOKMARKED` (0x20); ENABLED_NOT->`CHAPTER_SHOW_NOT_BOOKMARKED` (0x40) | 0x60 |

All three run in `launchNonCancellable` and capture `manga` from `successState` at call time (MSM:886-924).

OBSERVED race (bug): `awaitSet*Filter` derives the new `chapterFlags` from the CAPTURED `manga.chapterFlags`, not from the latest DB row. Two filter changes in quick succession (before the DB flow re-emits and updates `successState.manga`) each write a full copy of the stale flags, so the earlier change is lost. Example: user sets Unread filter, then immediately Downloaded filter. The second write restores the unread bits to their old value. Fix in Rust: write only the masked bits in SQL (`UPDATE mangas SET chapter_flags = (chapter_flags & ~:mask) | (:flag & :mask) WHERE _id = :id`) or re-read the row inside the transaction. See E-FLAG-1.

### 6.6 Sorting / display setters (MSM:931-949)

`setDisplayMode(mode: Long)`: `awaitSetDisplayMode(manga, mode)` with mask `CHAPTER_DISPLAY_MASK` 0x100000 (no-op-looking mapping; mode is passed straight through, values 0 or 0x100000).

`setSorting(sort: Long)`: `awaitSetSortingModeOrFlipOrder(manga, sort)` (SetMangaChapterFlags.kt:47-70):
- If `manga.sorting == sort`: flip direction. If currently descending -> set `CHAPTER_SORT_ASC`; if ascending -> set `CHAPTER_SORT_DESC`. Mask `0x1`.
- Else: set sorting bits to `sort` (mask 0x300) AND set direction to `CHAPTER_SORT_ASC` (0x1). A newly chosen sort key always starts ASCENDING.

OBSERVED: because flip uses the captured `manga`, rapid double taps have the same race as section 6.5.

### 6.7 Chapter-to-chapter gap separators: chapterListItems (MSM:1165-1190)

`State.Success.chapterListItems` (lazy) = `processedChapters.insertSeparators { before, after -> <rule below> }` (generic helper, app/src/main/java/eu/kanade/core/util/CollectionUtils.kt:8-21). The helper iterates `i` from `-1` to `lastIndex`, emitting `before` (if not null), then the separator generated for `(before, after)`:

```
(lowerChapter, higherChapter) = if manga.sortDescending() (after, before) else (before, after)
if higherChapter == null -> no separator
if lowerChapter == null:
    missing = floor(higherChapter.chapter.chapterNumber).toInt() - 1, coerced to >= 0
else:
    missing = calculateChapterGap(higherChapter.chapter, lowerChapter.chapter)   // domain MissingChapters.kt:40-49
if missing > 0 -> ChapterList.MissingCount(id = "${lowerChapter?.id}-${higherChapter.id}", count = missing)
```

Interpretation:
- Ascending list: a separator at the very top shows how many chapters are missing BEFORE the first displayed chapter (`floor(first) - 1`, e.g. first = 5 gives 4 missing, chapters 1 to 4).
- Descending list: the top separator never appears (higher is null there). The separator at the BOTTOM shows the chapters below the last item.
- Between two items: `calculateChapterGap` = `floor(higher) - floor(lower) - 1`. Returns `0` if either number is negative (unrecognized) (MissingChapters.kt:40-49).
- `ChapterList.MissingCount` is a separate row type. Count is an Int.

`MissingCount` id uses `null` text when lower is null: `"null-<higherId>"`. Use an enum-based id in Rust.

### 6.8 Selection (MSM:971-1057)

Selection state is kept in two places:
- `selectedChapterIds: HashSet<Long>` (MSM:154), a model-level set.
- `selectedPositions: Array<Int>` of size 2 `[-1, -1]` (MSM:153): "first and last selected index in list" (index into the processed list at the time of the action).
- `Item.selected: Boolean` in the list (copied from the set on each DB emission, section 6.1).

**toggleSelection(item, selected, userSelected = false, fromLongPress = false)** (MSM:971-1033). Runs `updateSuccessState` (note the lambda is executed inside `MutableStateFlow.update`, which may run it more than once on contention; the side effects below are not idempotent, see E-SEL-2):
1. `newChapters = successState.processedChapters.toMutableList()` (OBSERVED BUG, see 6.8.1).
2. `selectedIndex = processedChapters.indexOfFirst { it.id == item.chapter.id }`. If `< 0` return (no change).
3. `selectedItem = list[selectedIndex]`. If `(selectedItem.selected && selected) || (!selectedItem.selected && !selected)` return (no change).
4. `firstSelection = none { it.selected }` (evaluated BEFORE setting).
5. `set(selectedIndex, selectedItem.copy(selected = selected))`; `selectedChapterIds.addOrRemove(item.id, selected)`.
6. Branch A: `selected && userSelected && fromLongPress` (range select):
   - If `firstSelection`: `selectedPositions = [selectedIndex, selectedIndex]`.
   - Else:
     - If `selectedIndex < selectedPositions[0]`: `range = (selectedIndex + 1) ..< selectedPositions[0]`; `selectedPositions[0] = selectedIndex`.
     - Else if `selectedIndex > selectedPositions[1]`: `range = (selectedPositions[1] + 1) ..< selectedIndex`; `selectedPositions[1] = selectedIndex`.
     - Else: `range = IntRange.EMPTY`.
     - For each `i` in `range`: if not selected, add to `selectedChapterIds` and set `selected = true`.
7. Branch B: `userSelected && !fromLongPress` (plain tap or checkbox):
   - If `!selected`:
     - if `selectedIndex == selectedPositions[0]`: `selectedPositions[0] = indexOfFirst { it.selected }` (may be -1).
     - else if `selectedIndex == selectedPositions[1]`: `selectedPositions[1] = indexOfLast { it.selected }` (may be -1).
   - Else (selected):
     - if `selectedIndex < selectedPositions[0]`: `selectedPositions[0] = selectedIndex`.
     - else if `selectedIndex > selectedPositions[1]`: `selectedPositions[1] = selectedIndex`.
8. Branch C: programmatic (`userSelected = false`): no position change.
9. Return `copy(chapters = newChapters)`.

Subtle behavior of Branch B: with `selectedPositions == [-1, -1]` a first tap sets only `[1]` (because `selectedIndex < -1` is never true). OBSERVED: `[0]` stays `-1`, so a later long press before the tapped item cannot extend a range. Keep the positions the way they are in the port (it is the upstream behavior users know), or document the change. See E-SEL-3.

**toggleAllSelection(selected: Boolean)** (MSM:1035-1045): over `successState.chapters` (the RAW list, not processed): `selectedChapterIds.addOrRemove(it.id, selected)`; `copy(selected = selected)`. Resets `selectedPositions` to `[-1, -1]`.

**invertSelection()** (MSM:1047-1057): same over the raw list, each item flipped: `addOrRemove(it.id, !it.selected)`; `copy(selected = !it.selected)`. Resets positions.

OBSERVED consequence: select-all and invert operate on the raw list, so they include chapters hidden by the active filters. Bulk actions (section 6.9) then act on those hidden chapters too. This is the same as the upstream UI, so keep it, but document it to the user in the UI spec (p2-ui-parity.md).

#### 6.8.1 OBSERVED BUG: toggleSelection overwrites raw chapters with processed chapters

MSM:977-1031: `toggleSelection` builds `newChapters` from `successState.processedChapters` (filtered and sorted), then calls `successState.copy(chapters = newChapters)`. The raw `chapters` list (what `allChapters` returns, used by `getUnreadChapters()` when `skipFiltered` is false, and by `toggleAllSelection`/`invertSelection`) is replaced by the filtered list. Effects until the next DB emission:
- Hidden chapters disappear from `chapters`. Select-all after one tap no longer selects hidden chapters (inconsistent with a fresh open).
- `getUnreadChapters()` with `skipFiltered == false` sees the filtered list.
- The list shown in the UI is still correct because `processedChapters` is recomputed from the new `chapters`.

Proposal: the Rust selection engine keeps the raw list as the source of truth and computes the processed view from it. Document the upstream difference in the release notes. See E-SEL-1 and T-38b-008.

### 6.9 Chapter bulk actions and swipe

#### 6.9.1 chapterSwipe / executeChapterSwipeAction (MSM:605-643)

`chapterSwipe(item, swipeAction)` -> `launch { executeChapterSwipeAction(item, swipeAction) }`.

`executeChapterSwipeAction`:
- `ToggleRead` -> `markChaptersRead(listOf(chapter), !chapter.read)`.
- `ToggleBookmark` -> `bookmarkChapters(listOf(chapter), !chapter.bookmark)`.
- `Download` -> map `downloadState`:
  - `ERROR` or `NOT_DOWNLOADED` -> `START_NOW`
  - `QUEUE` or `DOWNLOADING` -> `CANCEL`
  - `DOWNLOADED` -> `DELETE`
  - then `runChapterDownloadActions(listOf(item), action)`.
- `Disabled` -> `throw IllegalStateException()` (caller must not dispatch it).

#### 6.9.2 runChapterDownloadActions (MSM:696-719)

- `START`: `startDownload(items.map { chapter }, startNow = false)`; then if any item has `downloadState == ERROR`, `downloadManager.startDownloads()`.
- `START_NOW`: `chapter = items.singleOrNull()?.chapter ?: return`; `startDownload(listOf(chapter), startNow = true)`.
- `CANCEL`: `chapterId = items.singleOrNull()?.id ?: return` -> `cancelDownload(chapterId)`. OBSERVED: CANCEL with several items does nothing (silent).
- `DELETE`: `deleteChapters(items.map { chapter })`.

#### 6.9.3 startDownload(chapters, startNow) (MSM:666-694)

`successState` captured (`?: return`). `launchNonCancellable`:
1. If `startNow`: `chapterId = chapters.singleOrNull()?.id ?: return@launchNonCancellable`; `downloadManager.startDownloadNow(chapterId)`. Multi-chapter START_NOW is silently ignored.
   - `startDownloadNow` (MangaDownloadManager.kt:113-124): if already queued, move it to the front; else build a `MangaDownload.fromChapterId` and put it at index 0; then `reorderQueue` and `startDownloads()`.
2. Else: `downloadChapters(chapters)` (section 6.9.4).
3. Add-to-library prompt (MSM:680-692): `if (!isFavorited && !successState.hasPromptedToAddBefore)`:
   - `updateSuccessState { copy(hasPromptedToAddBefore = true) }` (only in memory, not persisted; resets on every screen open).
   - `result = snackbarHostState.showSnackbar(message = AYMR.strings.snack_add_to_manga_library ("Add manga to library?"), actionLabel = MR.strings.action_add ("Add"), withDismissAction = true)`.
   - If `result == ActionPerformed && !isFavorited` -> `toggleFavorite()` (full routing from section 4.2 with duplicate check).

Note: the prompt is only shown after a successful queue call and only when the manga is not a favorite. Startnow with multiple chapters returns before the prompt.

#### 6.9.4 downloadChapters(chapters) (MSM:822-826)

`manga = successState?.manga ?: return`; `downloadManager.downloadChapters(manga, chapters)` (autoStart default true, MangaDownloadManager.kt:141-143); then `toggleAllSelection(false)`.

#### 6.9.5 runDownloadAction(action: DownloadAction) (MSM:721-733)

| Action | Chapters |
|---|---|
| `NEXT_1_ITEM` | `getUnreadChaptersSorted().take(1)` |
| `NEXT_5_ITEMS` | `take(5)` |
| `NEXT_10_ITEMS` | `take(10)` |
| `NEXT_25_ITEMS` | `take(25)` |
| `UNVIEWED_ITEMS` | `getUnreadChapters()` (NOT sorted, list order of the source chosen by section 6.9.6) |

If non-empty: `startDownload(list, startNow = false)`.

#### 6.9.6 getUnreadChapters / getUnreadChaptersSorted (MSM:653-664)

`getUnreadChapters()`:
- `chapterItems = if (skipFiltered) filteredChapters.orEmpty() else allChapters.orEmpty()`. `skipFiltered` is the reader preference `skip_filtered` (default `true`, ReaderPreferences.kt:85). So by default the view filters apply.
- Keep items where `!chapter.read && downloadState == NOT_DOWNLOADED` (exact match: QUEUE, DOWNLOADING, ERROR and DOWNLOADED are excluded).
- Map to `chapter`.

`getUnreadChaptersSorted()`:
- `manga = successState?.manga ?: return emptyList()`.
- `getUnreadChapters().sortedWith(getChapterSort(manga))`.
- If `manga.sortDescending()` then `.reversed()`.

Double sort is intentional: the comparator already honors the direction, and the reverse flips it. For a descending manga the result is oldest-first (lowest number or highest index first, see table in 6.4). For NUMBER descending: comparator gives high-to-low, reverse gives low-to-high. For SOURCE descending: comparator gives sourceOrder ascending (newest first), reverse gives oldest first. Net: "next N" always downloads the OLDEST unread chapters first.

#### 6.9.7 getNextUnreadChapter (MSM:648-651) and getNextUnread (ChapterGetNextUnread.kt)

`getNextUnreadChapter()` = `successState.chapters.getNextUnread(manga)`, which is the `List<ChapterList.Item>` overload. It calls the public `applyFilters(manga)` (section 6.3) on the RAW `chapters`, so the view filters apply and then the sort. Then:
- `manga.sortDescending()` -> `findLast { !it.chapter.read }`
- else -> `find { !it.chapter.read }`

Result is `Chapter?` (null when everything matching is read). In descending mode the "last" item in the sorted list is the OLDEST unread (see 6.9.6). Net: "continue reading" always returns the oldest unread chapter that passes the view filters. Ports to `kuro-chapters::nextunread`.

The `List<Chapter>` overload (ChapterGetNextUnread.kt:12-22) takes a `MangaDownloadManager` and uses `applyFilters(manga, downloadManager)` (ChapterFilter.kt:16-36), with the same find/findLast logic. Used by the library screen (MangaLibraryScreenModel.kt:437), not by this file.

#### 6.9.8 markPreviousChapterRead(pointer) (MSM:741-747)

- `chapters = filteredChapters.orEmpty().map { chapter }` (processed view order).
- `prevChapters = if manga.sortDescending() chapters.asReversed() else chapters` (so `prevChapters` is always chronological).
- `pointerPos = prevChapters.indexOf(pointer)` (uses data-class equality, so the pointer must match ALL fields of the current chapter; a stale copy returns -1 and nothing happens).
- If `pointerPos != -1`: `markChaptersRead(prevChapters.take(pointerPos), read = true)`.

Note `prevChapters.take(pointerPos)` is the chapters strictly before the pointer. The pointer itself is not marked.

#### 6.9.9 markChaptersRead(chapters, read) (MSM:754-795)

1. `toggleAllSelection(false)` (immediately, even when the list is empty; clears the selection).
2. If `chapters.isEmpty()` return.
3. `launchIO`:
   a. `setReadStatus.await(read = read, chapters = chapters.toTypedArray())` (SetReadStatus.kt:29-61):
      - `read = true`: only chapters where `!it.read` are written; `read = false`: chapters where `it.read || it.lastPageRead > 0`. Nothing written if the filtered list is empty (`Result.NoChapters`).
      - `ChapterUpdate(read = read, lastPageRead = if (!read) 0 else null, id)`, so marking unread also resets the page position; marking read leaves `lastPageRead` unchanged.
      - Write via `chapterRepository.updateAllChapters`. Failure -> `Result.InternalError`, logged.
      - If `read && downloadPreferences.removeAfterMarkedAsRead()` (key `pref_remove_after_marked_as_read_key`, default `false`): `deleteDownload.awaitAll(manga, chapters)` for each manga group.
   b. Stop here unless ALL are true: `read == true`, `successState?.hasLoggedInTrackers != false`, `autoTrackState != NEVER`. (OBSERVED: `hasLoggedInTrackers == false` stops; the `?.` is only for a null successState.)
   c. `refreshTrackers()` (section 5.2).
   d. `tracks = getTracks.await(mangaId)`; `maxChapterNumber = chapters.maxOf { it.chapterNumber }` (uses the PASSED list, including chapters that were already read); `shouldPromptTrackingUpdate = tracks.any { maxChapterNumber > track.lastChapterRead }`.
   e. If not: return.
   f. If `autoTrackState == ALWAYS`: `trackChapter.await(context, mangaId, maxChapterNumber)`; then `withUIContext { toast(AYMR.strings.trackers_updated_summary_manga, maxChapterNumber.toInt()) }` ("Trackers updated to chapter %d"); return.
   g. Else (ASK): `snackbarHostState.showSnackbar(message = MR.strings.confirm_tracker_update(maxChapterNumber.toInt()) ("Update trackers to chapter %d?"), actionLabel = MR.strings.action_ok ("OK"), duration = SnackbarDuration.Short, withDismissAction = true)`. If `ActionPerformed` -> `trackChapter.await(context, mangaId, maxChapterNumber)`.

OBSERVED: the tracker prompt is evaluated even if `setReadStatus` wrote nothing (already-read chapters). Re-marking read can therefore trigger the tracker prompt again.
OBSERVED: `toInt()` truncates. A chapter 12.5 shows "12".

`TrackChapter.await` (app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt:24-60): for each track whose service is logged in and where `chapterNumber > track.lastChapterRead`, it refreshes the remote track, sets `lastChapterRead = chapterNumber`, pushes the update, stores the track, and removes the delayed-update entry. On failure it stores a delayed update (`delayedTrackingStore.addManga`) and schedules `DelayedMangaTrackingUpdateJob` (`setupJobOnFailure = true`). Failures are logged at INFO and do not surface to the user.

### 6.10 bookmarkChapters (MSM:832-840)

`launchIO`: `chapters.filterNot { it.bookmark == bookmarked }.map { ChapterUpdate(id, bookmark = bookmarked) }` then `updateChapter.awaitAll(it)`. Then `toggleAllSelection(false)` (immediately, not after the write).

### 6.11 deleteChapters (MSM:847-861)

`launchNonCancellable { try { successState?.let { downloadManager.deleteChapters(chapters, manga, source) } } catch (e: Throwable) { logcat(ERROR, e) } }`.

`MangaDownloadManager.deleteChapters` (MangaDownloadManager.kt:264-289) is `launchIO`, so the try/catch here catches only synchronous errors. Filters chapters (`getChaptersToDelete`), removes them from the queue, deletes the chapter directories, removes them from the cache, and deletes the manga directory if it is now empty (then `deleteManga(removeQueued = false)`).

Note: `deleteChapters` does NOT clear the selection. The caller (MangaScreen dialog, part 1) does.

### 6.12 downloadNewChapters(chapters) (MSM:863-872)

Only reached from `fetchChaptersFromSource` when `manualFetch == true` (section 3.4).

`launchNonCancellable`: `manga`; `chaptersToDownload = filterChaptersForDownload.await(manga, chapters)`; if non-empty, `downloadChapters(chaptersToDownload)` (section 6.9.4, which also clears the selection).

`FilterChaptersForDownload.await` (domain/src/main/java/mihon/domain/items/chapter/interactor/FilterChaptersForDownload.kt:29-44):
1. Return `[]` if `newChapters` is empty, or `downloadNewChapters` pref (`download_new`, default `false`) is false, or `!manga.shouldDownloadNewChapters()`.
2. If `download_new_unread_chapters_only` (default `false`) is false: return all `newChapters`.
3. Else `readChapterNumbers = getChaptersByMangaId(manga.id).filter { read && isRecognizedNumber }.map { chapterNumber }.toSet()`; return `newChapters.filterNot { chapterNumber in readChapterNumbers }`.

`shouldDownloadNewChapters` (lines 52-67):
- `!favorite` -> false.
- `categories = getCategories.await(id).map { id }.ifEmpty { [0] }`.
- `includedCategories = download_new_categories` (set of strings -> Long), `excludedCategories = download_new_categories_exclude`.
- `includedCategories.isEmpty() && excludedCategories.isEmpty()` -> true.
- `categories.any { in excludedCategories }` -> false.
- `includedCategories.isEmpty()` -> true.
- else `categories.any { in includedCategories }`.

### 6.13 Settings / defaults (MSM:951-969)

`setCurrentSettingsAsDefault(applyToExisting: Boolean)` (`launchNonCancellable`):
1. `libraryPreferences.setChapterSettingsDefault(manga)` (LibraryPreferences.kt:249-258) writes six keys from the current manga:
   - `default_chapter_filter_by_read` = `unreadFilterRaw`
   - `default_chapter_filter_by_downloaded` = `downloadedFilterRaw`
   - `default_chapter_filter_by_bookmarked` = `bookmarkedFilterRaw`
   - `default_chapter_sort_by_source_or_number` = `sorting` (masked 0x300 value)
   - `default_chapter_display_by_name_or_number` = `displayMode`
   - `default_chapter_sort_by_ascending_or_descending` = `CHAPTER_SORT_DESC` (0) if `sortDescending()` else `CHAPTER_SORT_ASC` (1)
2. If `applyToExisting`: `setMangaDefaultChapterFlags.awaitAll()` applies the defaults to EVERY favorite manga (SetMangaDefaultChapterFlags.kt:31-35).
3. `snackbarHostState.showSnackbar(message = MR.strings.chapter_settings_updated)` ("Updated default chapter settings"), no action, default duration.

`resetToDefaultSettings()`: `setMangaDefaultChapterFlags.await(manga)` (section 7.4).

### 6.14 Dialog routing (MSM:1108-1137)

- `dismissDialog()` -> `dialog = null`.
- `showDeleteChapterDialog(chapters)` -> `DeleteChapters(chapters)`.
- `showSettingsDialog()` -> `SettingsSheet`.
- `showTrackDialog()` -> `TrackSheet`.
- `showCoverDialog()` -> `FullCover`.
- `showMigrateDialog(duplicate)` -> `manga = successState?.manga ?: return`; `Migrate(newManga = manga, oldManga = duplicate)`. Naming: `newManga` is the manga currently open, `oldManga` is the library duplicate it would replace. Migration itself is in the browse/migration area.

---

## 7. Persistence-facing behaviors

### 7.1 setExcludedScanlators(set) (MSM:1133-1137)

`launchIO { setExcludedScanlators.await(mangaId, set) }`. `SetExcludedScanlators.await` (app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetExcludedScanlators.kt:8-27), in one transaction: reads current excluded set; inserts `set - current`; removes `current - set` (`DELETE ... WHERE manga_id = :mangaId AND scanlator IN :scanlators`). Table `excluded_scanlators(manga_id, scanlator)` with cascade delete on mangas.

Scanlator filter effect (for the chapter query, `applyScanlatorFilter = true` everywhere in this file): chapters whose `scanlator` matches an excluded row are removed in SQL. NULL scanlators are never excluded (`C.scanlator = ES.scanlator` is NULL when NULL). See data doc.

`scanlatorFilterActive = excludedScanlators.intersect(availableScanlators).isNotEmpty()` (MSM:1192-1193). `filterActive = scanlatorFilterActive || manga.chaptersFiltered()` (MSM:1195-1196).

`availableScanlators` = `getScanlatorsByMangaIdAsFlow(mangaId)` mapped to `mapNotNull { it.ifBlank { null } }.toSet()` (GetAvailableScanlators.kt:8-22). Blank scanlators are treated as absent.

### 7.2 Stored chapter fields touched here

- `read`, `lastPageRead`, `bookmark` via `ChapterUpdate` (domain ChapterUpdate).
- `Manga.chapterFlags` via `MangaUpdate`.
- `Manga.favorite`, `Manga.dateAdded` (`awaitUpdateFavorite`).
- `Manga.coverLastModified` (cover cache invalidation).
- `Manga.fetchInterval`, `nextUpdate`.
- Category membership (`setMangaCategories`).
- Excluded scanlators.
- Tracks via tracker services.

### 7.3 Chapter `sourceOrder`, `dateUpload`, `chapterNumber` semantics (for sort/gap)

- `sourceOrder` = index in the source list after `distinctBy { url }` (SyncChaptersWithSource.kt:24-31).
- `dateUpload` default `-1` on `Chapter.create()` (Chapter.kt:43); sync may set it to `now` or the previous max for new chapters with `dateUpload == 0` (SyncChaptersWithSource.kt:68-75).
- `chapterNumber` default `-1.0` = unrecognized (Chapter.kt:44). `isRecognizedNumber = chapterNumber >= 0f` (Chapter.kt:19-20). Note: `>= 0f` compares a Double with a Float literal, which Kotlin widens; `0.0` IS recognized.

### 7.4 Non-favorite default reset on open (MSM:220-222)

`if (!manga.favorite) setMangaDefaultChapterFlags.await(manga)`. `await` writes `awaitSetAllFlags(...)` with the six default keys (SetMangaDefaultChapterFlags.kt:15-29). Those writes start from `0L`, so any previous flags on the row are discarded (only the six defaults survive). OBSERVED: a user who sets a filter on a non-favorite manga loses it the next time the manga is opened. Keep this unless the user approves the change (see p2-ui-parity.md, "Behavior decisions for the user").

---

## 8. Track sub-screens (MTID)

All sub-screens are pushed on a navigator and popped on confirm/cancel. In the Rust port they are dialogs or pages (p2-ui-parity.md).

### 8.1 Tracking home list: MangaTrackInfoDialogHomeScreen (MTID:92-285)

Model init (MTID:207-225):
1. `launch { refreshTrackers() }` (MTID:240-260): `RefreshMangaTracks.await(mangaId).filter { it.first != null }.forEach { (track, e) -> logcat(...); withUIContext { context.toast(stringResource(MR.strings.track_error, track!!.name, e.message ?: "")) } }`.
2. `launch { getTracks.subscribe(mangaId).catch { logcat(ERROR, it) }.distinctUntilChanged().map { it.mapToTrackItem() }.collectLatest { trackItems = it } }`.

`mapToTrackItem()` (MTID:268-278):
- `loggedInTrackers = trackerManager.loggedInTrackers().filter { it is MangaTracker }`.
- `source = MangaSourceManager.getOrStub(sourceId)`.
- `map { service -> MangaTrackItem(find { it.trackerId == service.id }, service) }` (one item per logged-in manga tracker, linked track or null).
- `filter { (it.tracker as? EnhancedMangaTracker)?.accept(source) ?: true }`.

Per row actions (MTID:113-180):
- Status click -> push `TrackStatusSelectorScreen(track = it.track!!, serviceId = it.tracker.id)`.
- Chapter click -> push `TrackChapterSelectorScreen`.
- Score click -> push `TrackScoreSelectorScreen`.
- Start date edit -> push `TrackDateSelectorScreen(start = true)`.
- End date edit -> push `TrackDateSelectorScreen(start = false)`.
- New search: if `it.tracker is EnhancedMangaTracker` -> `registerEnhancedTracking(it)` (no UI); else push `TrackServiceSearchScreen(mangaId, initialQuery = it.track?.title ?: mangaTitle, currentUrl = it.track?.remoteUrl, serviceId)`.
- Open in browser -> `openTrackerInBrowser` (MTID:187-192): `url = track?.remoteUrl ?: return`; if not blank, `context.openInBrowser(url)`.
- Remove -> push `TrackerMangaRemoveScreen(mangaId, track = it.track!!, serviceId)`.
- Copy link -> `copyTrackerLink` (MTID:194-199): if not blank `copyToClipboard(url, url)` (label = url).
- Toggle private -> `togglePrivate(item)` (MTID:262-266): `launchNonCancellable { (item.tracker as? MangaTracker)?.setRemotePrivate(item.track!!.toDbTrack(), !item.track.private) }`.

OBSERVED: `togglePrivate` dereferences `item.track!!` without a null check. A row with no linked track must not expose this action, or the call crashes.

`registerEnhancedTracking(item)` (MTID:227-238): `item.tracker as EnhancedMangaTracker` (throws if not); `launchNonCancellable { manga = GetManga.await(mangaId) ?: return; try { matchResult = tracker.match(manga) ?: throw Exception(); tracker.mangaService.register(matchResult, mangaId) } catch (e: Exception) { withUIContext { toast(MR.strings.error_no_match) } } }`.
- OBSERVED: any exception (including network failure) shows "No match found".

### 8.2 TrackStatusSelectorScreen (MTID:287-340)

- Initial selection: `track.status` (Long).
- Options: `tracker.mangaService.getStatusListManga()` (ordered list of Long ids); labels: `(tracker as? MangaTracker)?.getStatusForManga(id)` (StringResource?).
- Confirm: `setRemoteMangaStatus(track.toDbTrack(), selection)` in `launchNonCancellable`, then pop. Cancel: pop without write.

### 8.3 TrackChapterSelectorScreen (MTID:342-403)

- Initial selection: `track.lastChapterRead.toInt()`.
- Range: `0..endRange` inclusive, where `endRange = track.totalChapters` if `> 0`, else `10000`. (`totalChapters` is a Long; `toLong()` used in range, cast from Int.)
- Confirm: `setRemoteLastChapterRead(track.toDbTrack(), selection)`.
- The picker is `TrackItemSelector(..., isManga = true)`.

### 8.4 TrackScoreSelectorScreen (MTID:405-457)

- Initial selection: `tracker.mangaService.displayScore(track)` (String).
- Options: `tracker.mangaService.getScoreList()` (ImmutableList<String>).
- Confirm: `setRemoteScore(track.toDbTrack(), selection)` (String, the tracker's own score format).

### 8.5 TrackDateSelectorScreen (MTID:459-589)

`start: Boolean` picks the start or finish date.

Constraints (`SelectableDates`, MTID:465-520), all in the system default zone (`ZoneOffset.systemDefault()` for dates, `ZoneId.systemDefault()` for instants):
- Day check `isSelectableDate(utcMillis)`: convert to local date; if `> LocalDate.now()` -> false (no future dates).
  - If `start && track.finishDate > 0`: must be `<= date(finishDate)`.
  - Else if `!start && track.startDate > 0`: must be `>= date(startDate)`.
  - Else true.
- Year check `isSelectableYear(year)`: if `year > LocalDate.now().year` -> false; then the same ordering check on the year component.

`canRemove = if (start) startDate > 0 else finishDate > 0` (MTID:533-537). Remove button only when `canRemove`.

Title: `MR.strings.track_started_reading_date` ("Start date") or `track_finished_reading_date` ("Finish date").

`initialSelection` (MTID:562-569): `(if start startDate else finishDate).takeIf { != 0L } ?: Instant.now().toEpochMilli()`, converted from system zone to UTC millis (`convertEpochMillisZone(systemDefault, UTC)`), so a date picker that works in UTC midnight shows the right day.

`setDate(millis)` (MTID:572-583): converts UTC millis to system zone (`convertEpochMillisZone(UTC, systemDefault)`), then `launchNonCancellable` -> `setRemoteStartDate` / `setRemoteFinishDate(track.toDbTrack(), localMillis)`. Then pop.

`confirmRemoveDate` pushes `TrackDateRemoverScreen(track, serviceId, start)`.

### 8.6 TrackDateRemoverScreen (MTID:591-677)

Confirmation dialog: icon Delete; title `MR.strings.track_remove_date_conf_title` ("Remove date?"); text `track_remove_start_date_conf_text` or `track_remove_finish_date_conf_text` with `tracker.name` ("This will remove your previously selected start date from %s" / "...finish date from %s"). Buttons: Cancel (`action_cancel`, pop) and Remove (`action_remove`, errorContainer colors): `removeDate()` (`setRemoteStartDate(track, 0)` or `setRemoteFinishDate(track, 0)` in `launchNonCancellable`), then `popUntil { it is MangaTrackInfoDialogHomeScreen }`.

### 8.7 TrackServiceSearchScreen (MTID:679-770)

Public `data class`. Model init (MTID:727-732): `if (initialQuery.isNotBlank()) trackingSearch(initialQuery)`.

`trackingSearch(query)` (MTID:734-754): `launch`:
1. `state.copy(queryResult = null, selected = null)` (loading).
2. `result = withIOContext { try { Result.success(tracker.mangaService.searchManga(query)) } catch (e: Throwable) { Result.failure(e) } }`.
3. `state.copy(queryResult = result, selected = result.getOrNull()?.find { it.tracking_url == currentUrl })`. Preselects the item whose URL matches the current link.

OBSERVED: `catch (e: Throwable)` also catches cancellation.

`updateSelection(selected)` sets `selected`. `registerTracking(item)` = `launchNonCancellable { tracker.mangaService.register(item, mangaId) }` (MTID:756-758).

Screen: `MangaTrackerSearch(state = textFieldState, onDispatchQuery = { trackingSearch(textFieldState.text.toString()) }, queryResult, selected, onSelectedChange, onConfirmSelection = { private -> selected.private = private; registerTracking(selected); pop }, onDismissRequest, supportsPrivateTracking = tracker.supportsPrivateTracking)` (MTID:700-715). The private flag is written onto the search result object before registering.

### 8.8 TrackerMangaRemoveScreen (MTID:772-874)

Dialog: title `track_delete_title` ("Remove %s tracking?"); text `track_delete_text` ("This will remove the tracking locally."); if `tracker is DeletableMangaTracker`, a `LabeledCheckbox` `track_delete_remote_text` ("Also remove from %s"), default unchecked. Buttons: Cancel (pop), OK (`action_ok`, errorContainer): `unregisterTracking(serviceId)` (`deleteTrack.await(mangaId, serviceId)` in `launchNonCancellable`), then `if (removeRemoteTrack) deleteMangaFromService()` (`(tracker as DeletableMangaTracker).delete(track)` in try/catch, logged with "Failed to delete manga entry from service"), then pop.

OBSERVED: the remote delete is fire-and-forget. A remote failure is only logged; the user sees no error, but the local row is already gone.

---

## 9. Messages, snackbars, toasts, and timing

### 9.1 Snackbar durations (Compose M3 defaults, not in scope files)

`SnackbarHostState.showSnackbar(message, actionLabel, withDismissAction, duration)`: the code in scope passes `duration` explicitly only for two calls.
- Explicit `SnackbarDuration.Short`: `confirm_tracker_update` (MSM:788).
- Other calls use the default. From Compose material3 (not verified in this repo): `duration = if (actionLabel == null) Short else Indefinite`. Calls with an action label (`delete_downloads_for_manga`, `snack_add_to_manga_library`) therefore stay until dismissed.

Proposal: Short = 4000 ms, Indefinite = until dismissed or replaced; the action is shown as a button; `withDismissAction` adds an X button. Document in p2-ui-parity.md.

### 9.2 Toasts

`context.toast(message)` (eu/kanade/tachiyomi/util/system, Android `Toast`). Used for: tracker refresh error (`track_error`), tracker-updated summary (`trackers_updated_summary_manga`), enhanced-match failure (`error_no_match`), remote-track update failure (outside scope, TrackAndMangaExtensions). Android Toast is non-interactive and short.

Proposal: in-app toast (bottom of window, auto-hide 2500 ms, not focus-stealing) for these three. Replace with `Notifier`-free in-window toast; no desktop notification for these.

### 9.3 Message keys used in scope (English text from base strings.xml)

MR (i18n/src/commonMain/moko-resources/base/strings.xml):
- `delete_downloads_for_manga` = "Delete downloaded chapters?"
- `action_delete` = "Delete"
- `action_add` = "Add"
- `action_ok` = "OK"
- `action_cancel` = "Cancel"
- `action_remove` = "Remove"
- `confirm_tracker_update` = "Update trackers to chapter %d?"
- `no_chapters_error` = "No chapters found"
- `chapter_settings_updated` = "Updated default chapter settings"
- `track_error` = "%1$s error: %2$s"
- `error_no_match` = "No match found"
- `track_started_reading_date` = "Start date"
- `track_finished_reading_date` = "Finish date"
- `track_remove_date_conf_title` = "Remove date?"
- `track_remove_start_date_conf_text` = "This will remove your previously selected start date from %s"
- `track_remove_finish_date_conf_text` = "This will remove your previously selected finish date from %s"
- `track_delete_title` = "Remove %s tracking?"
- `track_delete_text` = "This will remove the tracking locally."
- `track_delete_remote_text` = "Also remove from %s"
- `lock_always` = "Always" (AutoTrackState.ALWAYS label)
- `default_category_summary` = "Always ask" (AutoTrackState.ASK label)
- `lock_never` = "Never" (AutoTrackState.NEVER label)

AYMR (i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml):
- `snack_add_to_manga_library` = "Add manga to library?"
- `trackers_updated_summary_manga` = "Trackers updated to chapter %d"

Errors shown through `formattedMessage` (app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:16-38): `HttpException` -> `MR.strings.exception_http` with code; `UnknownHostException` -> `exception_offline` or `exception_unknown_host`; `NoChaptersException` -> `no_results_found`; `SourceNotInstalledException` -> `loader_not_implemented_error`; other exceptions -> `"Exception"`/`"IOException"` gives `message ?: className`, else `"$className: $message"`. Owned by area:36 but used here.

Other strings used in part 1 of this area and referenced in the data doc: none in this part.

---

## 10. Concurrency model (as it exists)

- `screenModelScope` = Voyager's ScreenModel scope (Main dispatcher, cancelled on dispose). Used for `launch`.
- `launchIO` (tachiyomi.core.common.util.lang): IO dispatcher. `withIOContext`, `withUIContext` switch dispatchers.
- `launchNonCancellable`: continues even if the screen is disposed (used for every write, so a user who navigates away still finishes the write).
- `flowWithLifecycle(lifecycle)`: collection stops when the lifecycle is below STARTED.
- `MutableStateFlow.update` on `mutableState`: a CAS loop that may re-run its lambda (see E-SEL-2).
- `selectedChapterIds` (HashSet) and `selectedPositions` (Array) are mutated from inside `update` lambdas, i.e. from any thread that calls `toggleSelection`. No synchronization.
- `combine(...)` for chapters uses `collectLatest`, which cancels the previous block when a new emission arrives.

---

## 11. Algorithm test vectors (from upstream tests and derived from code)

Upstream tests in scope families:
- `domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt`:
  - `missingChaptersCount(emptyList) == 0`
  - `missingChaptersCount([-1, -1, -1]) == 0`
  - `missingChaptersCount([1.0, 1.0, 1.1, 1.5, 1.6, 1.99]) == 0`
  - `missingChaptersCount([-1, 1, 2, 2.2, 4, 6, 10, 10]) == 5`
  - `calculateChapterGap(10, 9) == 0`, `(10, 8) == 1`, `(10, 8.5) == 1`, `(10, 1.1) == 8`
  - `calculateChapterGap(-1, 10) == 0`, `(99, -1) == 0`

Derived vectors for the separator generator (section 6.7):
- Asc list [1, 2, 4] -> separator before 4 with count 1; no top separator (floor(1)-1 = 0).
- Asc list [5, 6] -> top separator count 4 (chapters 1-4).
- Desc list [6, 5, 2] -> no top separator; between 5 and 2 a separator with count `floor(5) - floor(2) - 1 = 2` (chapters 3 and 4); bottom separator (after 2) with count `floor(2) - 1 = 1` (chapter 1).
- Desc list [10.5, 10] -> gap = floor(10) - floor(10)... for pair (10.5, 10) in desc order: higher = 10.5, lower = 10 -> 10 - 10 - 1 = -1 -> no separator.

Derived vectors for sort (section 6.4), SOURCE sorting, default desc, input sourceOrder = [2, 0, 1]: output order sourceOrder [0, 1, 2].

Derived vectors for next-unread (section 6.9.7): items (number, read) sorted by NUMBER desc = [5 read, 4 unread, 3 unread, 2 read, 1 read]; `findLast { !read }` -> chapter 3. Not chapter 4.

Derived vectors for NEXT_N download (section 6.9.6): NUMBER desc, unread not-downloaded chapters {1, 2, 3, 7} -> NEXT_1 = [1], NEXT_5 = [1, 2, 3, 7].

Derived vector for markPreviousChapterRead (section 6.9.8): processed view NUMBER desc [9, 8, 7, 6], pointer 7 -> chronological [6, 7, 8, 9], pointerPos 1 -> mark [6].

Derived vector for `setFlag`: `0x1A` (0b11010) setFlag(0x8, 0x18) = `(0x1A and 0xE7) or (0x8 and 0x18)` = `0x02 or 0x08` = `0x0A`.

---

## 12. Interactions with other areas

| Interaction | Other area | Contract |
|---|---|---|
| Chapter rows and counts | area:05, area:07 | `Chapter` and `Manga` models, SQL with excluded-scanlator join |
| Source fetch | area:15-source-api-manga | `getMangaDetails`, `getChapterList`, error types |
| Downloads | area:23-downloads-manga, area:25-downloads-shared | queue, status flows, startDownloadNow, deleteChapters, cache |
| Tracking | area:27-tracking-core, 28-33 | `MangaTracker`, `EnhancedMangaTracker`, `DeletableMangaTracker`, `RefreshMangaTracks`, `TrackChapter`, `AddMangaTracks`, `setRemote*` |
| Reader filter | area:36 / reader | `skip_filtered` preference |
| Library default category | area:08, area:37-ui-library | `default_category`, categories |
| Migration | area:38 part 1 / browse migration | `Dialog.Migrate` |
| Cover cache | area:34-images-covers-cache | `removeCovers`, `deleteFromCache` |
| Local source | area:14-source-local-and-metadata | `isLocal()` |
| Preferences | area:02-preferences-system | keys, defaults |
| Error text | area:36 | `formattedMessage` |
