# 25-downloads-shared — behaviour spec

Notation: `OBSERVED` = what the Kotlin does (file:line cited). `Proposal:` = what the Rust port should do.

---

## 1. Delete interactors (anime and manga)

### 1.1 OBSERVED — DeleteEpisodeDownload

File: app/src/main/java/eu/kanade/domain/download/anime/interactor/DeleteEpisodeDownload.kt:9-18

```kotlin
class DeleteEpisodeDownload(
    private val sourceManager: AnimeSourceManager,
    private val downloadManager: AnimeDownloadManager,
) {
    suspend fun awaitAll(anime: Anime, vararg episodes: Episode) = withNonCancellableContext {
        sourceManager.get(anime.source)?.let { source ->
            downloadManager.deleteEpisodes(episodes.toList(), anime, source)
        }
    }
}
```

Steps:
1. Run inside `withNonCancellableContext` (cancellation cannot interrupt the body; `tachiyomi.core.common.util.lang`).
2. Resolve the source with `sourceManager.get(anime.source)` (`anime.source` is a Long source id).
3. If the source is null (not installed / not loaded), the `let` is skipped. Nothing is deleted, nothing is logged, no error is returned. The call returns `null`/`Unit`.
4. Otherwise call `AnimeDownloadManager.deleteEpisodes(episodes.toList(), anime, source)` (app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt:269).
5. `awaitAll` returns as soon as `deleteEpisodes` returns. `deleteEpisodes` immediately does `launchIO { ... }` (AnimeDownloadManager.kt:270), so the **filesystem deletion is fire-and-forget**: the caller does not wait for it.

Consequence (OBSERVED): callers (`SetSeenStatus`) return `Result.Success` before the files are gone.

### 1.2 OBSERVED — DeleteChapterDownload

File: app/src/main/java/eu/kanade/domain/download/manga/interactor/DeleteChapterDownload.kt:9-18. Identical shape with `MangaSourceManager`, `MangaDownloadManager`, `Manga`, `Chapter`, and `downloadManager.deleteChapters(chapters.toList(), manga, source)` (MangaDownloadManager.kt:264). The `deleteChapters` body is also `launchIO { ... }` (MangaDownloadManager.kt:265).

### 1.3 Callers (OBSERVED)

- SetSeenStatus.await (app/src/main/java/eu/kanade/domain/items/episode/interactor/SetSeenStatus.kt:29-61):
  1. `episodesToUpdate = episodes.filter { if (seen) !it.seen else (it.seen || it.lastSecondSeen > 0) }`.
  2. If empty → `Result.NoEpisodes`, stop.
  3. `episodeRepository.updateAllEpisodes(episodesToUpdate.map { EpisodeUpdate(seen = seen, lastSecondSeen = if (!seen) 0 else null, id = it.id) })`. Exception → logcat ERROR, `Result.InternalError(e)`.
  4. If `seen && downloadPreferences.removeAfterMarkedAsRead().get()`: group `episodesToUpdate` by `animeId`, and for each group call `deleteDownload.awaitAll(anime = animeRepository.getAnimeById(animeId), episodes = group)`.
  5. Return `Result.Success`.
- SetReadStatus.await (app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt:29-61): same structure. Filter `if (read) !it.read else (it.read || it.lastPageRead > 0)`; mapper `ChapterUpdate(read, lastPageRead = if (!read) 0 else null, id)`; delete trigger `read && removeAfterMarkedAsRead`; group by `mangaId`; `Result.NoChapters`, `Result.InternalError(e)`, `Result.Success`.
- Note the trigger is "marked as read/seen" only. Marking unread never deletes. Only the newly-changed items (`episodesToUpdate`) are passed, not the full list.

### 1.4 OBSERVED — delete filter (the "which items are actually deleted" rule)

Anime: AnimeDownloadManager.kt:408-427 `getEpisodesToDelete(episodes, anime)`:

```kotlin
val categoriesToExclude = downloadPreferences.removeExcludeAnimeCategories().get().map(String::toLong)
val categoriesForAnime = getCategories.await(anime.id).map { it.id }.ifEmpty { listOf(0) }
val filteredCategoryAnime = if (categoriesForAnime.intersect(categoriesToExclude).isNotEmpty()) {
    episodes.filterNot { it.seen }
} else {
    episodes
}
return if (!downloadPreferences.removeBookmarkedChapters().get()) {
    filteredCategoryAnime.filterNot { it.bookmark }
} else {
    filteredCategoryAnime
}
```

Manga: MangaDownloadManager.kt:409-429 `getChaptersToDelete(chapters, manga)`: identical except `removeExcludeCategories()`, `filterNot { it.read }`, `it.bookmark`.

Ordered rules (both kinds), applied to the input list:
1. Let `excl` = set of category ids (Long) from the `remove_exclude_*categories` string set (parsed with `String::toLong`; a non-numeric entry throws `NumberFormatException` — Proposal: skip and log instead).
2. Let `cats` = category ids of the entry from `getCategories.await(entryId)`. If that list is empty, use `[0]` (the implicit "default" category id 0).
3. If `cats ∩ excl` is non-empty: keep only items that are NOT watched/read (`!seen` / `!read`). Watched/read items are protected in excluded categories.
4. If `removeBookmarkedChapters` is **false** (default): drop bookmarked items. If true: bookmarked items are kept in the list (deleted).

Quirk (OBSERVED): the key name `pref_remove_bookmarked` with default false means "bookmarked items are protected". The Settings label is the authority for the user-visible meaning; the Rust port keeps the same stored key and semantics.

Edge cases:
- Empty input → empty output → `deleteEpisodes`/`deleteChapters` returns without doing anything (AnimeDownloadManager.kt:271-273, MangaDownloadManager.kt:266-268).
- Entry has no categories → treated as category 0 for the exclude check.

### 1.5 OBSERVED — delete execution (anime; manga is identical with chapter/manga names)

AnimeDownloadManager.kt:269-291 `deleteEpisodes(episodes, anime, source)` inside `launchIO`:
1. `filteredEpisodes = getEpisodesToDelete(episodes, anime)`; if empty, return.
2. `removeFromDownloadQueue(filteredEpisodes)` (AnimeDownloadManager.kt:315-327, see 1.6).
3. `(animeDir, episodeDirs) = provider.findEpisodeDirs(filteredEpisodes, anime, source)`.
4. `episodeDirs.forEach { it.delete() }` (plain directory delete; non-recursive semantics are whatever `UniFile.delete()` does for the provider — Proposal: recursive remove of the item directory).
5. `cache.removeEpisodes(filteredEpisodes, anime)` (in-memory and on-disk cache index update; owned by area:24).
6. If `animeDir?.listFiles()?.isEmpty() == true` → `deleteAnime(anime, source, removeQueued = false)` (AnimeDownloadManager.kt:299-313): deletes the anime directory, then if the source directory is now empty deletes it and calls `cache.removeSource(source)`.

Manga: MangaDownloadManager.kt:264-282 and `deleteManga` at :291.

### 1.6 OBSERVED — queue handling during delete

AnimeDownloadManager.kt:315-327 `removeFromDownloadQueue(episodes)`:
```
wasRunning = downloader.isRunning
if wasRunning: downloader.pause()
downloader.removeFromQueue(episodes)     // AnimeDownloader.kt:895
if wasRunning:
    if queueState.value.isEmpty(): downloader.stop()
    else if queueState.value.isNotEmpty(): downloader.start()
```
Note the second branch is `else if (isNotEmpty())`, which is always true when reached. Equivalent to `else`.

AnimeDownloader.removeFromQueue (AnimeDownloader.kt:895-897) → removeFromQueueIf (AnimeDownloader.kt:880-893): removes all matching queue items; any removed item whose status is `DOWNLOADING` or `QUEUE` is set to `NOT_DOWNLOADED`; the store is updated (`store.removeAll`).

Manga counterpart: MangaDownloadManager.kt:308 `removeFromDownloadQueue(chapters)` and MangaDownloader.kt:724 `removeFromQueue(chapters)`.

Note: `cancelQueuedDownloads(downloads)` (AnimeDownloadManager.kt:258-260, MangaDownloadManager.kt:253-255) calls the same `removeFromDownloadQueue` (no file deletion).

---

## 2. Delete triggers (summary of the whole feature)

| Trigger | Entry point | Preference gate | Filter applied |
|---|---|---|---|
| User marks item seen/read, one or many | SetSeenStatus / SetReadStatus | `pref_remove_after_marked_as_read_key` = true | 1.4 |
| Reader/player "remove after read N slots" | ReaderViewModel.kt:508-520, PlayerViewModel.kt:1743, ExternalIntents.kt:508 (owned by other areas) | `remove_after_read_slots` != -1 | enqueue path (`enqueueDeleteReadChapters`) → `deleteChapters`/pending deleter |
| Manual "delete download" from entry screen / download menu | owned by entry-details areas (38/39) | none | 1.4 |
| Pending deletion (deferred) | `enqueueChaptersToDelete` (MangaDownloadManager.kt:331-333) and `MangaDownloadPendingDeleter` | none | 1.4 |

This area owns the immediate path (`DeleteChapterDownload`, `DeleteEpisodeDownload`, `deleteChapters`, `deleteEpisodes`). The deferred path is in area:23 and area:24.

---

## 3. Queue tab page (DownloadsTab)

Source: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt.

### 3.1 Tab registration (OBSERVED, lines 78-90)
- `TabOptions.index = 6u`.
- `title = stringResource(MR.strings.label_download_queue)` → "Download queue".
- `icon = rememberAnimatedVectorPainter(AnimatedImageVector.animatedVectorResource(R.drawable.anim_history_enter), isSelected)` where `isSelected = LocalTabNavigator.current.current.key == key`. The icon is the history-enter animation (observed; a copy/paste artefact upstream).

### 3.2 Screen models (OBSERVED, lines 96-97)
`rememberScreenModel { AnimeDownloadQueueScreenModel() }` and `MangaDownloadQueueScreenModel()`. The pager page wrappers `animeDownloadTab(...)`/`mangaDownloadTab(...)` each call `rememberScreenModel` again so there are two model instances per kind in the Kotlin (AnimeDownloadQueueTab.kt:23, MangaDownloadQueueTab.kt, same pattern). Proposal: one model per kind, owned by the page.

### 3.3 Counts (OBSERVED, lines 98-105)
- `animeDownloadCount = animeDownloadList.sumOf { it.subItems.size }`; same for manga. These are the total number of queued downloads across all sources of that kind.

### 3.4 Pager (OBSERVED, lines 107, 269-289)
- `rememberPagerState { 2 }`: page 0 = anime, page 1 = manga. Tab row `PrimaryTabRow(selectedTabIndex = state.currentPage, modifier = zIndex(1f))`.
- Tab 0: label `AYMR.strings.label_anime` ("Anime"), badge `animeDownloadCount`. Tab 1: label `AYMR.strings.manga` ("Manga"), badge `mangaDownloadCount`. Tap → `animateScrollToPage(n)`.
- Badge: `TabText(text, badgeCount: Int?)` draws a `Pill` whenever `badgeCount != null`. Since counts are non-null Ints, the badge is shown even when the count is `0` (OBSERVED, Tabs.kt:28). Proposal: hide the badge at 0 (small deliberate deviation, recorded in T-25a-021).

### 3.5 Top app bar (OBSERVED, lines 134-166)
- Title: `Text(stringResource(MR.strings.label_download_queue), maxLines = 1, overflow = Ellipsis)` with `weight(1f, false)`.
- Pill beside title only when `animeDownloadCount > 0` (ONLY anime; manga count is not shown in the title). Pill text = count; padding start 4 dp; colour `onBackground` with alpha 0.12 in dark theme, 0.08 in light (`isSystemInDarkTheme()`); font size 14 sp.
- `navigateUp = navigator::pop`.
- `actions`: page 0 → `AnimeActions`; page 1 → `MangaActions`.
- `scrollBehavior = TopAppBarDefaults.pinnedScrollBehavior(...)`.

### 3.6 Floating action button (OBSERVED, lines 167-230)
- Visible when: page 0 → `animeDownloadList.isNotEmpty()`; page 1 → `mangaDownloadList.isNotEmpty()`; otherwise false. Enter `fadeIn()`, exit `fadeOut()`.
- Running flag: `animeScreenModel.isDownloaderRunning` / `mangaScreenModel.isDownloaderRunning` (collected as state).
- Label:
  - Anime: running → `AYMR.strings.action_stop` ("Stop"); not running → `AYMR.strings.action_continue` ("Continue").
  - Manga: running → `MR.strings.action_pause` ("Pause"); not running → `MR.strings.action_resume` ("Resume").
- Icon: running → `Icons.Outlined.Pause`; not running → `Icons.Filled.PlayArrow` (both kinds).
- Click:
  - Anime: running → `animeScreenModel.pauseDownloads()`; not running → `startDownloads()`.
  - Manga: running → `mangaScreenModel.pauseDownloads()`; not running → `startDownloads()`.
- `expanded = fabExpanded`.
- Unreachable branch `else -> MR.strings.action_pause` (pager is fixed at 2 pages).

Pause semantics differ by kind (OBSERVED):
- Anime `pauseDownloads()` = `downloader.stop()` (AnimeDownloadManager.kt:88-90).
- Manga `pauseDownloads()` = `downloader.pause()` then `downloader.stop()` (MangaDownloadManager.kt:90-93).
- Both `startDownloads()`: if `downloader.isRunning` return; if the job is running → `downloader.start()`; else → `Job.start(context)` (AnimeDownloadManager.kt:75-85, MangaDownloadManager.kt:77-88).

### 3.7 FAB collapse on scroll (OBSERVED, lines 111-132)
- `fabExpanded` starts `true`.
- `NestedScrollConnection.onPreScroll(available, source)`: `fabExpanded = available.y >= 0`, then delegates to `scrollBehavior`.
- Meaning: finger/wheel moving content upward (`available.y < 0`) collapses the FAB to icon-only; scrolling back down or any non-negative movement expands it. Post-scroll and fling are delegated unchanged.
- Proposal (GTK): `EventControllerScroll` on the list; `dy > 0` (content moves up) → collapse; `dy <= 0` → expand. Initial expanded.

### 3.8 Page body (OBSERVED, lines 232-290)
- Column padded top/start/end from the Scaffold content padding; the pager child gets `PaddingValues(bottom = contentPadding.calculateBottomPadding())`.
- Each pager page renders its own list and receives the shared `snackbarHostState` (`SnackbarHostState()` created once, line 108). Messages shown through it are owned by the per-kind screens (not in scope).

### 3.9 Sort and cancel actions — shared shape (OBSERVED)

`AnimeActions` (lines 294-376) and `MangaActions` (lines 378-460). Both render nothing when the list is empty (`if (list.isNotEmpty())`).

Overflow-style dropdown (`DropdownMenu`) with two nested menus:

| Menu row | Child rows | Anime action | Manga action |
|---|---|---|---|
| "By upload date" (`MR.strings.action_order_by_upload_date`) | "Newest" (`action_newest`) → `reorderQueue({ it.download.episode.dateUpload }, reverse = true)`; "Oldest" (`action_oldest`) → `reorderQueue({ it.download.episode.dateUpload }, reverse = false)` | yes | `chapter.dateUpload` |
| "By episode number" (`AYMR.strings.action_order_by_episode_number`) / "By chapter number" (`MR.strings.action_order_by_chapter_number`) | "Ascending" (`action_asc`) → `reverse = false`; "Descending" (`action_desc`) → `reverse = true` | `episode.episodeNumber` | `chapter.chapterNumber` |

Each child closes the menu (`closeMenu()`) after calling `reorderQueue`.

Note: the anime episode-number menu label uses `AYMR.strings.action_order_by_episode_number` while the upload-date label uses `MR.strings.action_order_by_upload_date`. Both exist in the sources; the rendered text is the English base string.

Toolbar actions (`AppBarActions`):
1. `AppBar.Action(title = MR.strings.action_sort, icon = Icons.AutoMirrored.Outlined.Sort, onClick = open sort menu)`.
2. `AppBar.OverflowAction(title = MR.strings.action_cancel_all, onClick = clearQueue())` (anime: AnimeDownloadQueueScreenModel.clearQueue → AnimeDownloadManager.clearQueue; manga likewise).

`clearQueue` semantics (OBSERVED):
- Anime: `AnimeDownloadManager.clearQueue()` = `downloader.clearQueue(); downloader.stop()` (AnimeDownloadManager.kt:95-98).
- Manga: `MangaDownloadManager.clearQueue()` = `downloader.clearQueue(); downloader.stop()` (MangaDownloadManager.kt:98-101). No pause call.
- Note: "Cancel all" is not confirmed by a dialog (OBSERVED: no confirmation code in DownloadsTab.kt).

---

## 4. Queue model — state, grouping, ordering

Anime model: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt. Manga model: app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreenModel.kt (same shape, see diff notes in 4.8).

### 4.1 Grouping (OBSERVED, anime lines 112-126)
```
downloadManager.queueState
  .map { downloads ->
     downloads.groupBy { it.source }                     // key = Source object
              .map { entry ->
                  AnimeDownloadHeaderItem(entry.key.id, entry.key.name, entry.value.size)
                      .apply { addSubItems(0, entry.value.map { AnimeDownloadItem(it, this) }) }
              }
  }
  .collect { _state.update { it } }
```
- Section order = first-appearance order of each source in the queue (Kotlin `groupBy` keeps insertion order).
- Within a section, item order = queue order. Note `addSubItems(0, ...)` inserts at index 0 of an empty section, so the order is preserved.
- Header holds `(sourceId, sourceName, count)`.

### 4.2 Flattened queue (OBSERVED)
The "queue order" sent to the engine is the concatenation of section subItems in header order. Every reorder operation rebuilds this flat list and calls `downloadManager.reorderQueue(flat)` (AnimeDownloadManager.kt:127 → `downloader.updateQueue(downloads)`).

### 4.3 reorderQueue(selector, reverse) (OBSERVED, anime lines 162-178)
```
for each header in adapter.headerItems:
    header.subItems = header.subItems.sortedBy(selector).toMutableList().apply { if (reverse) reverse() }
    flat += header.subItems.map { it.download }
reorder(flat)
```
Rules:
- Sorting is per section (per source); items never move between sections through this action.
- `sortedBy` is stable. `reverse == true` reverses the stable-sorted list, so **equal keys are also reversed** (edge case E-07 in p1-edge-cases-and-tests.md).
- `selector` for anime: `it.download.episode.dateUpload` or `it.download.episode.episodeNumber`. Manga: `it.download.chapter.dateUpload` or `it.download.chapter.chapterNumber`. Comparison uses the field's own `Comparable` type (exact types in p1-data-and-config.md section 2).
- Early return if `adapter == null` (view not bound) — nothing changes.

### 4.4 Drag reorder (OBSERVED, anime lines 46-54)
`onItemReleased(position)`: flatten `adapter.headerItems` → `getSectionItems(header)` into `AnimeDownloadItem.download` in on-screen order; call `reorder(flat)`. This is the only path that can move an item across sections (dropping into another section is allowed by the adapter; the next `queueState` emission regroups by source).

### 4.5 Move to top/bottom and series (OBSERVED, anime lines 62-94)
Menu ids (from download_single.xml): `move_to_top`, `move_to_top_series`, `move_to_bottom`, `move_to_bottom_series`, `cancel_download`, `cancel_series`.

- `move_to_top` / `move_to_bottom` (single item): inside the item's own section only, remove the item and re-insert at index 0 (`move_to_top`) or at the end (`move_to_bottom`). Then rebuild flat list over all sections and `reorder(flat)`.
- `move_to_top_series` / `move_to_bottom_series`: take `adapter.currentItems` filtered to download items, partition by `item.download.anime.id == it.anime.id` (anime, AnimeDownloadQueueScreenModel.kt) or `item.download.manga.id == it.manga.id` (manga, MangaDownloadQueueScreenModel.kt:93), then:
  - top: `reorder(selectedSeries + otherSeries)`.
  - bottom: `reorder(otherSeries + selectedSeries)`.
  - The result is a flat list passed straight to the engine; section regrouping happens on the next `queueState` emission (OBSERVED: can change section order; see E-09).
- Partition keeps the relative order inside each group (Kotlin `partition`).

### 4.6 Cancel (OBSERVED, anime lines 95-106)
- `cancel_download` → `cancel(listOf(item.download))` → `downloadManager.cancelQueuedDownloads(list)` → `removeFromDownloadQueue(episodes)` (see 1.6). No file deletion.
- `cancel_series` → all download items in `adapter.currentItems` whose anime id equals the item's anime id (manga: `item.download.manga.id == it.download.manga.id`, MangaDownloadQueueScreenModel.kt:107); if the list is non-empty → cancel(list). If empty, nothing.

### 4.7 Live status and progress (OBSERVED)

Anime `onStatusChange(download)` (AnimeDownloadQueueScreenModel.kt:185-202):
| State | Action |
|---|---|
| `DOWNLOADING` | `onUpdateProgress(d)` (notify progress and downloaded pages on holder), `onUpdateDownloadedPages(d)` |
| `DOWNLOADED` | `cancelProgressJob(d)`, `onUpdateProgress(d)`, `onUpdateDownloadedPages(d)` |
| `ERROR` | `cancelProgressJob(d)` |
| other | no-op |

Anime has no progress subscription job (`progressJobs` is declared but never populated in the anime model; the `progressJobs` map at line 38 is only cleared in `onDispose`). Progress for the anime row is driven by `onStatusChange` callbacks only.

Manga `onStatusChange` (MangaDownloadQueueScreenModel.kt:191-…):
| State | Action |
|---|---|
| `DOWNLOADING` | `launchProgressJob(d)`; `onUpdateDownloadedPages(d)` |
| `DOWNLOADED` | `cancelProgressJob(d)`; `onUpdateProgress(d)`; `onUpdateDownloadedPages(d)` |
| `ERROR` | `cancelProgressJob(d)` |
| other | no-op |

`launchProgressJob(download)` (manga, ≈line 215):
```
job = screenModelScope.launch {
    while (download.pages == null) delay(50)              // wait until pages are known
    progressFlows = download.pages!!.map(Page::progressFlow)
    combine(progressFlows, Array<Int>::sum)               // sum of per-page progress
        .distinctUntilChanged()
        .debounce(50)
        .collectLatest { onUpdateProgress(download) }
}
progressJobs.remove(download)?.cancel()                     // avoid leaking
progressJobs[download] = job
```
Timings: poll 50 ms until pages are known; debounce 50 ms on the summed progress; `collectLatest` cancels a stale update.

`onUpdateProgress(d)` = `getHolder(d)?.notifyProgress(); getHolder(d)?.notifyDownloadedPages()`.
`onUpdateDownloadedPages(d)` = `getHolder(d)?.notifyDownloadedPages(); getHolder(d)?.notifyProgress()`.
`getHolder(d)` = `controllerBinding.root.findViewHolderForItemId(d.episode.id / d.chapter.id) as? Holder` — null if the row is not bound (no-op). This is the Android RecyclerView lookup; Proposal: a `HashMap<id, RowHandle>` in the GTK list.

`onDispose()`: cancel all `progressJobs`, clear, `adapter = null`.

`isDownloaderRunning` = `downloadManager.isDownloaderRunning.stateIn(screenModelScope, WhileSubscribed(5000), false)` (AnimeDownloadQueueScreenModel.kt:136-137). Proposal: a `watch::Receiver<bool>` with the same 5 s grace period before the FAB state is dropped when no page is visible.

### 4.8 Manga vs anime model differences (OBSERVED, from diff)
1. Manga has `launchProgressJob` and calls it on DOWNLOADING; anime does not. Anime's row progress updates come from `statusFlow`/`progressFlow` only through the holder; check area:24 for the progress flow. In the port both kinds use the same `launchProgressJob` (Proposal, see T-25a-028).
2. Manga "cancel series" uses `allDownloadsForSeries` (naming only).
3. Manga `onStatusChange` DOWNLOADING calls `launchProgressJob` before `onUpdateDownloadedPages`; no immediate `onUpdateProgress` call. Anime calls `onUpdateProgress` and `onUpdateDownloadedPages`.
4. Entry id used for holder lookup: anime `episode.id`, manga `chapter.id`.
5. Manga "series" key is `download.manga.id` (MangaDownloadQueueScreenModel.kt:93, 107).

---

## 5. Item row menu (OBSERVED)

Resource: app/src/main/res/menu/download_single.xml (ids and titles):

| Order | id | Title (MR/AYMR key → English) |
|---|---|---|
| 1 | `move_to_top` | `action_move_to_top` → "Move to top" |
| 2 | `move_to_top_series` | `action_move_to_top_all_for_series` → "Move series to top" |
| 3 | `move_to_bottom` | `action_move_to_bottom` → "Move to bottom" |
| 4 | `move_to_bottom_series` | `action_move_to_bottom_all_for_series` → "Move series to bottom" |
| 5 | `cancel_download` | `action_cancel` → "Cancel" |
| 6 | `cancel_series` | `cancel_all_for_series` → "Cancel all for this series" |

Visibility (AnimeDownloadHolder.kt:100-106; MangaDownloadHolder.kt:90-95):
- `move_to_top` visible iff `bindingAdapterPosition > 1`.
- `move_to_bottom` visible iff `bindingAdapterPosition != adapter.itemCount - 1`.
- Other items always visible.

Note (OBSERVED): `bindingAdapterPosition` is the flat adapter position, which includes section headers. The rule therefore hides "Move to top" for the first row of the first section (position 1) and the header position 0, but shows it for the first row of the second section. Proposal: compute "is first download in the flat queue" and "is last download in the flat queue" (index 0 / len-1), which is the intended behaviour.

---

## 6. Mockup per-entry download menu (for reference, not this area)

`src/screens.rs:134-136` (`download_button`) lists "Start downloading now", "Cancel", "Delete". That menu belongs to entry lists (areas 37-41) and is listed for completeness only.

---

## 7. Preferences semantics (summary; full table in p1-data-and-config.md)

- Every accessor returns a `Preference` object built by `preferenceStore.getBoolean/getInt/getString/getStringSet(key, default)`; the class holds no cached values, reads and writes go through the store.
- `categoryPreferenceKeys` lists the six category-set keys; `PreferenceRestorer.kt:136` merges them with `LibraryPreferences.categoryPreferenceKeys` during backup restore.
- `CategoryPreferencesCleanupMigration.kt:16` (app/src/main/java/mihon/core/migration/migrations/) reads these category-set prefs through `DownloadPreferences` in a migration (purpose owned by area:11); `DeleteMangaCategory.kt` and `DeleteAnimeCategory.kt` read the category-set prefs as well (area:08).
- Consumers per accessor (grep over the tree, OBSERVED):

| Accessor | Read by |
|---|---|
| downloadOnlyOverWifi | AnimeDownloadJob.kt, MangaDownloadJob.kt |
| useExternalDownloader | AnimeDownloader.kt, AnimeScreenModel.kt, AnimeUpdatesScreenModel.kt |
| externalDownloaderSelection | AnimeDownloader.kt |
| saveChaptersAsCBZ | MangaDownloader.kt |
| splitTallImages | MangaDownloader.kt |
| autoDownloadWhileReading | ReaderViewModel.kt |
| autoDownloadWhileWatching | PlayerViewModel.kt |
| removeAfterReadSlots | ReaderViewModel.kt:509, PlayerViewModel.kt:1743, ExternalIntents.kt:508 |
| removeAfterMarkedAsRead | SetReadStatus.kt:49, SetSeenStatus.kt:49, NotificationReceiver.kt |
| removeBookmarkedChapters | AnimeDownloadManager.kt:422, MangaDownloadManager.kt:424 |
| downloadFillermarkedItems | AnimeDownloadManager.kt |
| removeExcludeCategories | MangaDownloadManager.kt:411, DeleteMangaCategory.kt |
| removeExcludeAnimeCategories | AnimeDownloadManager.kt:411, DeleteAnimeCategory.kt |
| downloadNewChapters | FilterChaptersForDownload.kt |
| downloadNewEpisodes | FilterEpisodesForDownload.kt |
| downloadNewChapterCategories / …Exclude | FilterChaptersForDownload.kt, DeleteMangaCategory.kt |
| downloadNewEpisodeCategories / …Exclude | FilterEpisodesForDownload.kt, DeleteAnimeCategory.kt |
| numberOfDownloads | **no reader in the tree** (only SettingsDownloadScreen.kt:102 and DownloadPreferences.kt:69). See E-12. |
| downloadSpeedLimit | MangaDownloader.kt:525 (`bytesPerSecond(value * 1024)`); no anime reader found |
| downloadNewUnreadChaptersOnly | FilterChaptersForDownload.kt |
| downloadNewUnseenEpisodesOnly | FilterEpisodesForDownload.kt |

(The grep was for `accessor()` call sites; paths are file names only; full paths in the Kotlin tree are under `app/src/main/java/...` or `domain/src/main/java/...`.)

---

## 8. Messages (user-visible strings used in scope)

Resolved English text from the base string files:

| Key (object) | Text | Where used |
|---|---|---|
| `label_download_queue` (MR) | Download queue | tab title, top bar |
| `action_order_by_upload_date` (MR) | By upload date | sort menu |
| `action_newest` (MR) | Newest | sort menu |
| `action_oldest` (MR) | Oldest | sort menu |
| `action_order_by_chapter_number` (MR) | By chapter number | manga sort menu |
| `action_order_by_episode_number` (AYMR) | By episode number | anime sort menu |
| `action_asc` (MR) | Ascending | sort menu |
| `action_desc` (MR) | Descending | sort menu |
| `action_sort` (MR) | Sort | toolbar action |
| `action_cancel_all` (MR) | Cancel all | overflow action |
| `action_pause` (MR) | Pause | manga FAB while running |
| `action_resume` (MR) | Resume | manga FAB while paused |
| `action_stop` (AYMR) | Stop | anime FAB while running |
| `action_continue` (AYMR) | Continue | anime FAB while stopped |
| `label_anime` (AYMR) | Anime | pager tab 0 |
| `manga` (AYMR) | Manga | pager tab 1 |
| `information_no_downloads` (MR) | No downloads | EmptyScreen in both queue screens |
| `action_move_to_top` (MR) | Move to top | row menu |
| `action_move_to_top_all_for_series` (MR) | Move series to top | row menu |
| `action_move_to_bottom` (MR) | Move to bottom | row menu |
| `action_move_to_bottom_all_for_series` (MR) | Move series to bottom | row menu |
| `action_cancel` (MR) | Cancel | row menu |
| `cancel_all_for_series` (MR) | Cancel all for this series | row menu |

Note: `MR.strings.manga` is "Library entries" in i18n; the queue uses `AYMR.strings.manga` ("Manga"). Use `AYMR` for the pager tab.

Logging: `logcat(LogPriority.ERROR, e)` in SetSeenStatus/SetReadStatus on update failure (no user-visible toast; the caller decides).

No snackbar or toast is emitted by DownloadsTab itself. Errors from deletes are swallowed inside the manager's `launchIO` (no try/catch in deleteEpisodes/deleteChapters; an exception in the coroutine propagates to the manager scope's handler — owned by area:24/23).

---

## 9. Concurrency and dispatchers (OBSERVED)

| Code | Dispatcher / scope | Non-cancellable | Notes |
|---|---|---|---|
| `DeleteEpisodeDownload.awaitAll` / `DeleteChapterDownload.awaitAll` | caller's | yes (`withNonCancellableContext`) | returns before FS work (fire-and-forget) |
| `deleteEpisodes` / `deleteChapters` | `launchIO` (IO dispatcher) | no | `launchIO` from `tachiyomi.core.common.util.lang` |
| Queue models | `screenModelScope` (Voyager) | no | `WhileSubscribed(5000)` for `isDownloaderRunning` |
| Progress job (manga) | `screenModelScope` | no | `delay(50)`, `debounce(50)`, `collectLatest` |
| Queue reorder | direct call from UI thread → `downloader.updateQueue` | — | engine side in area:23/24 |

Proposal (Rust):
- Deletion runs on `tokio::task::spawn_blocking` (directory removal is blocking IO).
- `awaitAll` in Rust **awaits** the removal result (deviation from Kotlin, justified in E-03 so the UI can refresh the row state deterministically).
- UI updates are posted to the GTK main loop via `glib::MainContext::channel` / `async_channel` with the GTK main context.

---

## 10. Interactions with other areas

- area:23 / area:24 own the engine (`downloader`, `queueState`, `statusFlow`, `progressFlow`, `isRunning`, `start`, `pause`, `stop`, `clearQueue`, `updateQueue`, `removeFromQueue`). This area only calls them.
- area:07 owns `SetSeenStatus`/`SetReadStatus` (the consumers of the delete interactors).
- area:08 owns category deletion (`DeleteMangaCategory.kt`, `DeleteAnimeCategory.kt`), which reads the category-set download prefs.
- area:47 owns SettingsDownloadScreen (all UI for the preferences).
- area:02 owns `PreferenceStore`.
- area:21 / area:22 own backup/restore of preferences (PreferenceRestorer).
- area:36/42 own the top-level navigation that hosts `DownloadsTab` (tab index 6).
