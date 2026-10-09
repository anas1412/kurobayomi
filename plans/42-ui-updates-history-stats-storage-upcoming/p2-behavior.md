# Area 42 (part 2): behaviour spec, Updates and Upcoming

All paths are repo-relative to `/home/blackbox/aniyomi-src`. "OBSERVED" = what the Kotlin does. "Proposal:" = what the Rust port should do. Section numbers are referenced from `p2-inventory.md`, `p2-data-and-config.md`, `p2-ui-parity.md`, `p2-edge-cases-and-tests.md`.

Files: `app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt` (MUSM), `.../ui/updates/anime/AnimeUpdatesScreenModel.kt` (AUSM), `mihon/feature/upcoming/manga/UpcomingMangaScreenModel.kt` (UMSM), `mihon/feature/upcoming/anime/UpcomingAnimeScreenModel.kt` (UASM).

---

## 1. Updates load pipeline (MUSM and AUSM, identical shape)

### 1.1 Constructor and defaults
- MUSM constructor (MangaUpdatesScreenModel.kt): `sourceManager`, `downloadManager`, `downloadCache`, `updateChapter`, `setReadStatus`, `getUpdates: GetMangaUpdates`, `getManga`, `getChapter`, `libraryPreferences`, `snackbarHostState = SnackbarHostState()`. All injected via `Injekt.get()`.
- AUSM constructor adds `downloadPreferences: DownloadPreferences` and stores `val useExternalDownloader = downloadPreferences.useExternalDownloader().get()` (read once at construction, not live).
- State object: `State(isLoading = true, items = persistentListOf(), dialog = null)`. Derived: `selected = items.filter { it.selected }`, `selectionMode = selected.isNotEmpty()`.
- `lastUpdated` = `libraryPreferences.lastUpdatedTimestamp().asState(screenModelScope)` (Long, epoch millis, live).

### 1.2 Subscription (OBSERVED)
Inside `screenModelScope.launchIO { ... }`:
1. `limit = ZonedDateTime.now().minusMonths(3).toInstant()` (computed ONCE when the ScreenModel is constructed, system zone, calendar-month subtraction).
2. `combine( getUpdates.subscribe(limit).distinctUntilChanged(), downloadCache.changes, downloadManager.queueState ) { updates, _, _ -> updates }`.
   - Any change in the download cache or the download queue re-emits the LAST known updates list (the cache/queue values are discarded). This is the only mechanism that re-computes per-row download state after a queue change.
3. `.catch { logcat(ERROR, it); _events.send(Event.InternalError) }`. OBSERVED: `catch` terminates the flow after emitting nothing further. After an error the list stops updating until the screen is recreated.
4. `.collectLatest { updates -> mutableState.update { it.copy(isLoading = false, items = updates.toUpdateItems()) } }`.

`GetMangaUpdates.subscribe(instant)` calls `repository.subscribeAllMangaUpdates(instant.toEpochMilli(), limit = 500)` (domain/src/main/java/tachiyomi/domain/updates/manga/interactor/GetMangaUpdates.kt). AUSM: `GetAnimeUpdates.subscribe(instant)` calls `subscribeAllAnimeUpdates(instant.toEpochMilli(), limit = 500)`.

SQL that runs (OBSERVED, data/src/main/sqldelight/view/updatesView.sq):
```
getRecentUpdates:
SELECT * FROM updatesView WHERE dateUpload > :after LIMIT :limit;
```
The view `updatesView` (see `p2-data-and-config.md` section 3) carries `WHERE favorite = 1 AND date_fetch > date_added ORDER BY date_fetch DESC`.
Consequences:
- Only chapters whose `date_upload` (source-provided upload time, millis) is strictly greater than now-3-months appear. Chapters with `date_upload = 0` (unknown) NEVER appear in the Updates tab.
- Ordering is by `date_fetch DESC` (time the app fetched the chapter), set in the view. SQLite does not guarantee view ORDER BY survives an outer query; Proposal: repeat `ORDER BY date_fetch DESC` in the outer query.
- LIMIT 500 is applied after WHERE and ORDER, so the window is the 500 most recently fetched qualifying rows.

AUSM SQL: `animeupdatesView` and `getRecentAnimeUpdates` with the same filter pattern (`animes.favorite = 1 AND date_fetch > date_added`, `ORDER BY date_fetch DESC`, `WHERE dateUpload > :after LIMIT :limit`).

### 1.3 Building the rows (toUpdateItems, OBSERVED)
For each `MangaUpdatesWithRelations` `update` in list order:
1. `activeDownload = downloadManager.getQueuedDownloadOrNull(update.chapterId)` (null if not queued/downloading).
2. `downloaded = downloadManager.isChapterDownloaded(update.chapterName, update.scanlator, update.mangaTitle, update.sourceId)`. OBSERVED: the on-disk check uses NAMES (chapter name, scanlator, manga title), not ids. Two chapters with the same name and scanlator in one manga map to the same folder check.
3. `downloadState = when { activeDownload != null -> activeDownload.status; downloaded -> DOWNLOADED; else -> NOT_DOWNLOADED }`.
4. Item: `MangaUpdatesItem(update, downloadStateProvider = { downloadState }, downloadProgressProvider = { activeDownload?.progress ?: 0 }, selected = update.chapterId in selectedChapterIds)`.
   - OBSERVED: the state provider is frozen at build time; the progress provider captures the `activeDownload` object and reads its live `progress`.
   - The AUSM equivalent uses `downloadManager.isEpisodeDownloaded(episodeName, scanlator, animeTitle, sourceId)` and `AnimeDownload.State`.
Result is `toPersistentList()` in the same order.

### 1.4 Download-state patch stream (OBSERVED)
A second `screenModelScope.launchIO` collects `merge(downloadManager.statusFlow(), downloadManager.progressFlow())` (`.catch { logcat(ERROR, it) }`, no event) and calls `updateDownloadState(download)`:
- `modifiedIndex = list.indexOfFirst { it.update.chapterId == download.chapter.id }` (AUSM: `download.episode.id` vs `update.episodeId`).
- If `< 0`, nothing changes.
- Else replaces that element with `item.copy(downloadStateProvider = { download.status }, downloadProgressProvider = { download.progress })`.
- Runs on every progress tick for every downloading item. Cost is O(n) per tick (Proposal: index map keyed by chapterId).

### 1.5 Loading and empty
- `isLoading` is `true` until the first successful emission of the subscription. An error before the first emission leaves `isLoading = true` forever (the catch emits only the event). Proposal: set `isLoading = false` on error and show the error banner.
- Empty state (`items.isEmpty()` and not loading) is shown by the presentation layer, see `p2-ui-parity.md`.

---

## 2. Date grouping and separators (OBSERVED, eu/kanade/core/util/CollectionUtils.kt)

`State.getUiModel()` (both models):
```
items.map { Item(it) }.insertSeparators { before, after ->
    beforeDate = before?.item?.update?.dateFetch?.toLocalDate()
    afterDate  = after?.item?.update?.dateFetch?.toLocalDate()
    when {
        beforeDate != afterDate && afterDate != null -> Header(afterDate)
        else -> null
    }
}
```
- Group key is the LOCAL CALENDAR DATE of `dateFetch` (system zone), not `dateUpload`.
- `insertSeparators(generator)`: for `i` in `-1..lastIndex`, add `before` (if non-null), then add `generator(before, after)` if non-null. A header therefore sits immediately before the first item of each date group. The first item always gets a header (before is null).
- Header payload is a `LocalDate`. The presentation calls `relativeDateText(date)` (section 6).

`insertSeparatorsReversed(generator)` (used by Upcoming, section 7): iterates `i` from `size` down to `0`, adds `after` then the separator, then returns `newList.asReversed()`. Effect: the separator for a boundary is emitted once, before the earlier-in-list item (in forward order), and a generator call receives `(before, after)` where `after` is the later item in forward order.

`addOrRemove(value, shouldAdd)` on a HashSet: `add` if true, `remove` otherwise.

---

## 3. Download actions (OBSERVED)

### 3.1 Dispatch
`downloadChapters(items, action)` (MUSM) / `downloadEpisodes(items, action)` (AUSM):
1. If `items.isEmpty()` return immediately.
2. `screenModelScope.launch { when (action) { ... }; toggleAllSelection(false) }`.

Actions (enum `ChapterDownloadAction` = START, START_NOW, CANCEL, DELETE; `EpisodeDownloadAction` adds SHOW_QUALITIES):

| Action | Behaviour | Precondition shortcut |
|---|---|---|
| START | `downloadChapters(items)` (section 3.2). Then if `items.any { it.downloadStateProvider() == ERROR }` call `downloadManager.startDownloads()`. | none |
| START_NOW | `startDownloadNow(chapterId)` | `items.singleOrNull()` else `return@launch` |
| CANCEL | `cancelDownload(chapterId)` | `items.singleOrNull()` else `return@launch` |
| DELETE | `deleteChapters(items)` (section 4.3) | none |
| SHOW_QUALITIES (AUSM only) | `showQualitiesDialog(update)` opens `Dialog.ShowQualities(episodeName, episodeId, animeId, sourceId)` | `items.singleOrNull()` else `return@launch` |

OBSERVED edge: `return@launch` for START_NOW, CANCEL and SHOW_QUALITIES with more than one selected item skips the trailing `toggleAllSelection(false)`. The selection is NOT cleared in that case. Proposal: keep the same (documented in the edge-case file as a known quirk) or clear selection in all cases; decision recorded as task T-42b-009 acceptance.

### 3.2 Enqueue (`downloadChapters(updatesItem)`, OBSERVED)
- Runs in `launchNonCancellable`.
- `groupedUpdates = updatesItem.groupBy { it.update.mangaId }.values` (LinkedHashMap order = first-seen order in the list).
- For each group: `manga = getManga.await(mangaId) ?: continue`; `sourceManager.get(manga.source) ?: continue` (if the source extension is not installed, the group is silently skipped, no error shown); `chapters = updates.mapNotNull { getChapter.await(it.update.chapterId) }`; `downloadManager.downloadChapters(manga, chapters)` (manga signature `(manga, chapters, autoStart = true)`).
- AUSM: `downloadManager.downloadEpisodes(anime, episodes, true, alt)` with `alt = false` from this path (the default parameter). Signature `downloadEpisodes(anime, episodes, autoStart = true, alt = false, video: Video? = null)`.

### 3.3 Start now / cancel
- `startDownloadNow(id)` delegates to `downloadManager.startDownloadNow(id)`.
- `cancelDownload(id)`:
  1. `activeDownload = downloadManager.getQueuedDownloadOrNull(id) ?: return`.
  2. `downloadManager.cancelQueuedDownloads(listOf(activeDownload))`.
  3. `updateDownloadState(activeDownload.apply { status = NOT_DOWNLOADED })`. OBSERVED: mutates the shared download object in place, then patches the row.

### 3.4 Enum values (cited)
`MangaDownload.State` and `AnimeDownload.State` both define `NOT_DOWNLOADED(0), QUEUE(1), DOWNLOADING(2), DOWNLOADED(3), ERROR(4)` (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt:67`, `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt:62`).

---

## 4. Read, seen, bookmark, fillermark, delete (OBSERVED)

### 4.1 Mark read / seen
`markUpdatesRead(updates, read)` (MUSM):
1. `screenModelScope.launchIO { setReadStatus.await(read = read, chapters = updates.mapNotNull { getChapter.await(it.update.chapterId) }.toTypedArray()) }`.
2. `toggleAllSelection(false)` runs immediately (not after the coroutine finishes).
The return value (`Result`) is ignored: `NoChapters` and `InternalError` are silent in the UI.

`SetReadStatus.await(read, vararg chapters)` (`app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt`), runs in `withNonCancellableContext`:
- Filter: `read == true` keeps chapters where `!it.read`; `read == false` keeps chapters where `it.read || it.lastPageRead > 0`.
- If empty, return `Result.NoChapters`.
- `chapterRepository.updateAllChapters(list.map { ChapterUpdate(read = read, lastPageRead = if (!read) 0 else null, id = chapter.id) })`. Exception → logcat ERROR and `Result.InternalError(e)`.
- If `read && downloadPreferences.removeAfterMarkedAsRead().get()`: group the updated chapters by `mangaId`, and for each group `deleteDownload.awaitAll(manga = mangaRepository.getMangaById(mangaId), chapters = ...)`.
- Return `Result.Success`.

AUSM uses `setSeenStatus.await(seen, episodes)` from `app/src/main/java/eu/kanade/domain/items/episode/interactor/SetSeenStatus.kt`. Same shape: filter `seen==true → !it.seen`, `seen==false → it.seen || it.lastSecondSeen > 0`; mapper `EpisodeUpdate(seen = seen, lastSecondSeen = if (!seen) 0 else null, id)`; remove-after-seen when `seen && removeAfterMarkedAsRead`.

### 4.2 Bookmark and fillermark
`bookmarkUpdates(updates, bookmark)` (both): `screenModelScope.launchIO { updates.filterNot { it.update.bookmark == bookmark }.map { ChapterUpdate(id = chapterId, bookmark = bookmark) }` (AUSM: `EpisodeUpdate(id = episodeId, bookmark = bookmark)`) `.let { updateChapter.awaitAll(it) } }`, then `toggleAllSelection(false)`.
`fillermarkUpdates(updates, fillermark)` (AUSM only): same with `it.update.fillermark == fillermark` and `EpisodeUpdate(id, fillermark = fillermark)`.
Only rows whose current flag differs are written.

### 4.3 Delete downloads
- `showConfirmDeleteChapters(items)` / `showConfirmDeleteEpisodes(items)` → `setDialog(Dialog.DeleteConfirmation(items))`.
- Dialog (`app/src/main/java/eu/kanade/presentation/updates/UpdatesDialog.kt`, `UpdatesDeleteConfirmationDialog(onDismissRequest, onConfirm, isManga)`): text = `MR.strings.confirm_delete_chapters` ("Are you sure you want to delete the selected chapters?") when manga, else `AYMR.strings.confirm_delete_episodes` ("Are you sure you want to delete the selected episodes?"). Buttons: confirm = `MR.strings.action_ok` ("OK"), dismiss = `MR.strings.action_cancel` ("Cancel"). Confirm calls `onConfirm()` then `onDismissRequest()`.
- Confirm calls `deleteChapters(dialog.toDelete)`: `launchNonCancellable`, `groupBy { mangaId }`, for each group: `manga = getManga.await(mangaId) ?: return@forEach`; `source = sourceManager.get(manga.source) ?: return@forEach` (silent skip when the source is missing); `chapters = updates.mapNotNull { getChapter.await(it.update.chapterId) }`; `downloadManager.deleteChapters(chapters, manga, source)` (AUSM: `deleteEpisodes(episodes, anime, source)`). Then `toggleAllSelection(false)`.
- OBSERVED: the delete path does NOT filter to downloaded rows. The bottom action "Delete" is only offered when some selected row is DOWNLOADED (section 12.4), but a mixed selection deletes all selected chapters' downloads, and the download manager handles non-downloaded ones.

---

## 5. Selection engine (OBSERVED, exact)

### 5.1 State
- `selectedPositions: Array<Int>(2) = [-1, -1]` (first and last selected index in the list, the "anchor range"). Not persisted.
- `selectedChapterIds: HashSet<Long>` (AUSM: `selectedEpisodeIds`). Kept in sync with each row's `selected` flag.
- Indices are positions in the `items` list (which is the display order, including NO header rows).

### 5.2 toggleSelection(item, selected, userSelected = false, fromLongPress = false)
Executed inside `mutableState.update`:
1. `selectedIndex = items.indexOfFirst { it.update.chapterId == item.update.chapterId }`. If `< 0`, return (no change).
2. `selectedItem = items[selectedIndex]`. If `selectedItem.selected == selected`, return (no change).
3. `firstSelection = none { it.selected }` on the working list, computed BEFORE the flip.
4. `items[selectedIndex] = selectedItem.copy(selected = selected)`; `selectedChapterIds.addOrRemove(id, selected)`.
5. Branch A, `selected && userSelected && fromLongPress` (a long press that selects):
   - If `firstSelection`: `selectedPositions[0] = selectedPositions[1] = selectedIndex`.
   - Else:
     - If `selectedIndex < selectedPositions[0]`: `range = (selectedIndex + 1) until selectedPositions[0]` (exclusive end); `selectedPositions[0] = selectedIndex`.
     - Else if `selectedIndex > selectedPositions[1]`: `range = (selectedPositions[1] + 1) until selectedIndex`; `selectedPositions[1] = selectedIndex`.
     - Else: `range = empty` (the item is inside the existing range).
     - For each `i` in `range`: if `items[i]` not selected → add its id to the set and set `selected = true`.
6. Branch B, `userSelected && !fromLongPress` (a tap while in selection mode):
   - If `!selected` (deselect):
     - If `selectedIndex == selectedPositions[0]`: `selectedPositions[0] = indexOfFirst { it.selected }` (may be `-1`).
     - Else if `selectedIndex == selectedPositions[1]`: `selectedPositions[1] = indexOfLast { it.selected }` (may be `-1`).
   - Else (select):
     - If `selectedIndex < selectedPositions[0]`: `selectedPositions[0] = selectedIndex`.
     - Else if `selectedIndex > selectedPositions[1]`: `selectedPositions[1] = selectedIndex`.
     - NOTE: when both anchors are `-1`, the first tap sets only `selectedPositions[1]`; `selectedPositions[0]` stays `-1`.
7. Otherwise (programmatic, `userSelected = false`): flags only, anchors untouched.
8. Return `state.copy(items = newItems.toPersistentList())`.

Proposal: port the anchor arithmetic with `isize` and `-1` sentinel to match exactly (Rust type in `p2-rust-design.md`). Known oddities preserved on purpose and listed in `p2-edge-cases-and-tests.md`.

Mapping of UI gestures (presentation, `p2-ui-parity.md`):
- Row tap when `selectionMode`: `onUpdateSelected(item, !item.selected, userSelected=true, fromLongPress=false)`.
- Row long press: `onUpdateSelected(item, !item.selected, true, true)`.

### 5.3 toggleAllSelection(selected) and invertSelection()
- `toggleAllSelection(selected)`: every item `selected = selected`, `selectedChapterIds.addOrRemove(id, selected)` for each, then `selectedPositions = [-1, -1]`.
- `invertSelection()`: each item flips; set updated with `!it.selected`; positions reset to `[-1, -1]`.

### 5.4 Selection survival
- `toUpdateItems()` recomputes `selected` from `selectedChapterIds` on each subscription emission, so selection survives list refreshes and download patches.
- OBSERVED: `selectedPositions` is NOT recomputed on refresh. After a refresh, anchors can point to stale indices.
- OBSERVED: ids in `selectedChapterIds` that are no longer in the list are never removed, and `selectionMode` is derived from the visible items, so a stale set with no visible selected rows shows normal mode.

### 5.5 Back and selection mode
- `BackHandler(enabled = state.selectionMode) { onSelectAll(false) }` (presentation). Back = `toggleAllSelection(false)`.
- Bottom navigation hides while selection mode: `LaunchedEffect(state.selectionMode) { HomeScreen.showBottomNav(!state.selectionMode) }` (see section 13).

---

## 6. Date and time formatting (cited, used by scope)

### 6.1 Long.toLocalDate / Instant.toLocalDate
`Long.toLocalDate() = LocalDate.ofInstant(Instant.ofEpochMilli(this), ZoneId.systemDefault())` (`app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt`). Epoch millis to local date.

### 6.2 relativeDateText(LocalDate?) (`app/src/main/java/eu/kanade/presentation/components/DateText.kt`)
- Returns `MR.strings.not_applicable` ("N/A") when the date is null (the Long overload maps `<= 0` to null).
- Else `localDate.toRelativeString(context, relative = relativeTime, dateFormat)` where `relativeTime = UiPreferences.relativeTime()` (default `true`) and `dateFormat = UiPreferences.dateFormat(preferences.dateFormat().get())`.

### 6.3 LocalDate.toRelativeString (OBSERVED, DateExtensions.kt)
If `!relative`: `dateFormat.format(this)`. Else `difference = ChronoUnit.DAYS.between(this, now)` (this = the date; now = `LocalDate.now()`):
- `difference < -7` → formatted date (`dateFormat`).
- `difference < 0` → plural `MR.plurals.upcoming_relative_time` with `abs(difference)`: one → "Tomorrow", other → "In %1$d days".
- `difference < 1` (i.e. 0) → `MR.strings.relative_time_today` ("Today").
- `difference < 7` → plural `MR.plurals.relative_time` with `difference`: one → "Yesterday", other → "%1$d days ago".
- else → formatted date.
Edge: `difference == -7` gives "In 7 days"; `difference == 7` gives the formatted date (7 is not `< 7`).

Default `dateFormat` when the pref is empty: `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)` (the function parameter default), reached through `UiPreferences.dateFormat(format)` (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:44`).

### 6.4 LocalDateTime.toRelativeString (not used by scope; recorded for area owners)
Hours and minutes variants with `AYMR.plurals.upcoming_relative_time_hours`, `upcoming_relative_time_minutes`, `relative_time_minutes`, `relative_time_hours`, `AYMR.strings.relative_time_now`. Used by chapter and episode release times in other areas.

### 6.5 relativeTimeSpanString(epochMillis) (`app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt`)
- `epochMillis <= 0` → "Never" (`MR.strings.relative_time_span_never`).
- `now - epochMillis < 60_000 ms` (1 minute) → "Just now" (`MR.strings.updates_last_update_info_just_now`).
- else `android.text.format.DateUtils.getRelativeTimeSpanString(epochMillis, now, MINUTE_IN_MILLIS)` (Android-only API; output such as "5 minutes ago", "2 hours ago", "Yesterday", "Oct 3, 2026").
Proposal: own implementation in `time_fmt.rs`: under 60 s "Just now"; otherwise `N minutes ago` / `N hours ago` / `N days ago` / absolute date. Exact Android wording is not reproducible without the platform; the key behaviour is the three cases (Never, Just now, relative string) and the minute granularity.

### 6.6 formatProgress(ms) (AUSM, private, `AnimeUpdatesUiItem.kt`)
- If `ms > 3_600_000`: `"%d:%02d:%02d"` with hours = `toHours(ms)`, minutes = `toMinutes(ms) - 60*hours`, seconds = `toSeconds(ms) - 60*toMinutes(ms)`.
- Else: `"%d:%02d"` with minutes = `toMinutes(ms)`, seconds = `toSeconds(ms) - 60*toMinutes(ms)`.
- Examples: `0 -> "0:00"`, `754_000 -> "12:34"`, `3_600_000 -> "60:00"` (the threshold is exclusive, so exactly one hour uses the m:ss form), `3_600_001 -> "1:00:00"`.
- `lastSecondSeen` and `totalSeconds` are passed in as milliseconds (the names say seconds; the formatter treats them as ms).

---

## 7. Upcoming lists (UMSM and UASM, OBSERVED)

### 7.1 Subscription
- `UpcomingMangaScreenModel(getUpcomingManga: GetUpcomingManga = Injekt.get())` (`StateScreenModel<State>(State())`).
- `init { screenModelScope.launch { getUpcomingManga.subscribe().collectLatest { ... } } }`. Every emission rebuilds `items`, `events`, `headerIndexes` in one `state.copy`.
- `GetUpcomingManga.subscribe()` → `mangaRepository.getUpcomingManga(includedStatuses)` where `includedStatuses = setOf(SManga.ONGOING.toLong(), SManga.PUBLISHING_FINISHED.toLong())` = `{1, 4}`.
- `GetUpcomingAnime.subscribe()` → `animeRepository.getUpcomingAnime(includedStatuses)`; `includedStatuses = setOf(SAnime.ONGOING, SAnime.PUBLISHING_FINISHED, SAnime.UPCOMING)` = `{1, 4, 7}`.

### 7.2 Repository query (OBSERVED)
`MangaRepositoryImpl.getUpcomingManga(statuses)`:
```
epochMillis = LocalDate.now().atStartOfDay(ZoneId.systemDefault()).toEpochSecond() * 1000
handler.subscribeToList { mangasQueries.getUpcomingManga(epochMillis, statuses, MangaMapper::mapManga) }
```
SQL (data/src/main/sqldelight/data/mangas.sq:124):
```
SELECT * FROM mangas
WHERE next_update >= :startOfDay AND favorite = 1 AND status IN :statuses
ORDER BY next_update ASC;
```
Anime: `data/src/main/sqldelightanime/dataanime/animes.sq:135`, same shape on `animes`.
- `epochMillis` is computed ONCE when the flow is created. The list does not advance at midnight until the subscription is re-created.
- No upper bound on `next_update`: far-future entries are included.
- Entries whose `next_update` is earlier than today's local midnight (even earlier today) are excluded.
- `next_update` is epoch milliseconds (the `* 1000` converts seconds to ms).

### 7.3 Grouping (OBSERVED)
`toUpcomingMangaUIModels()`:
```
var mangaCount = 0
fastMap { Item(it) }.insertSeparatorsReversed { before, after ->
    if (after != null) mangaCount++
    beforeDate = before?.manga?.expectedNextUpdate?.toLocalDate()
    afterDate  = after?.manga?.expectedNextUpdate?.toLocalDate()
    if (beforeDate != afterDate && afterDate != null) {
        Header(afterDate, mangaCount).also { mangaCount = 0 }
    } else null
}
```
Because iteration is from last to first, the count accumulates the items of one date group (ascending list), and the header emitted for a group carries the count of that group. Then the list is reversed back to ascending order. The result is, in ascending order: `Header(d1, n1), Item, Item, ..., Header(d2, n2), Item, ...`. Headers precede their group.

- `expectedNextUpdate` (`domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:37`): `nextUpdate.takeIf { status != SManga.COMPLETED.toLong() }?.let { Instant.ofEpochMilli(it) }`. COMPLETED = 2 is not in the query set, so the value is always present in practice.
- Dates are the LOCAL DATE of `expectedNextUpdate` (system zone).

### 7.4 Events and header indexes
- `events = filterIsInstance<Header>().associate { it.date to it.mangaCount }`: map from day to count, used for calendar dots.
- `headerIndexes = fastMapIndexedNotNull { index, m -> if (m is Header) m.date to index else null }.toMap()`: index in `items` (the list WITHOUT the calendar row).

### 7.5 Selected month
- `State.selectedYearMonth = YearMonth.now()` at model construction. Not persisted, not re-evaluated.
- `setSelectedYearMonth(ym)` updates state only. Navigating the month does NOT filter the list; the list always shows all upcoming items, and the calendar only highlights days.

### 7.6 Calendar day jump
`onClickDay(date, offset)` (in the content composable): `state.headerIndexes[date]?.let { scope.launch { listState.animateScrollToItem(it + offset) } }`.
- Phone: `offset = 1` (the calendar is item 0 of the same LazyColumn).
- Tablet: `offset = 0` (the calendar is in the left panel, the list is separate).
- Days with no header (no upcoming items on that day) do nothing on click.
- Click is accepted for any day in the displayed month, including past days (past days have no headers, so they do nothing).

---

## 8. Calendar (OBSERVED, mihon/feature/upcoming/components/calendar/)

### 8.1 Grid math (Calendar.kt, CalendarGrid)
- `localeFirstDayOfWeek = WeekFields.of(Locale.getDefault()).firstDayOfWeek.value` (1 = Monday ... 7 = Sunday).
- `weekDays = (0 until 7).map { DayOfWeek.of((localeFirstDayOfWeek - 1 + it) % 7 + 1) }` (`remember`ed).
- `emptyFieldCount = weekDays.indexOf(selectedYearMonth.atDay(1).dayOfWeek)`.
- `daysInMonth = selectedYearMonth.lengthOfMonth()`.
- Grid: 7 fixed columns. Header cells first (`item.getDisplayName(TextStyle.NARROW, Locale.getDefault())`, text 16sp, SemiBold, centered). Then `emptyFieldCount` empty `Box {}` cells. Then one `CalendarDay` per day `1..daysInMonth` with `date = ym.atDay(i)`, `events = events[date] ?: 0`.
- Grid width: if `isMediumWidthWindow() && !isExpandedWidthWindow()` the grid gets `widthIn(max = 360.dp)`, otherwise unconstrained (section 9).
- Column layout: `Column(fillMaxWidth, horizontalAlignment = CenterHorizontally, verticalArrangement = spacedBy(4.dp))`. Header row padding: vertical `padding.small`, start `padding.medium`.

### 8.2 Header (CalendarHeader.kt)
- Row, `SpaceBetween`, centered vertically. Left: `AnimatedContent(targetState = yearMonth, transitionSpec = getAnimation(), label = "Change Month")` with the title text.
- Title format: `DateTimeFormatter.ofPattern("MMMM yyyy", Locale.getDefault())` (full month name then year, e.g. "October 2026" in English). Style `titleLarge`.
- Right: `Row` with two `IconButton`s: left arrow (`KeyboardArrowLeft`, content description `MR.strings.upcoming_calendar_prev` = "Previous Month") calls `setSelectedYearMonth(ym.minusMonths(1))`; right arrow (`KeyboardArrowRight`, `MR.strings.upcoming_calendar_next` = "Next Month") calls `plusMonths(1)`.
- Transition (`getAnimation`, duration `MONTH_YEAR_CHANGE_ANIMATION_DURATION = 200` ms):
  - `movingForward = targetState > initialState` (YearMonth comparison).
  - Enter: `slideInVertically(tween(200)) { h -> if (movingForward) h else -h } + fadeIn(tween(200))`.
  - Exit: `slideOutVertically(tween(200)) { h -> if (movingForward) -h else h } + fadeOut(tween(200))`.
  - `(enter togetherWith exit).using(SizeTransform(clip = false))`.
  - Forward: new title enters from below, old exits upward. Backward: reverse.

### 8.3 Day cell (CalendarDay.kt)
- `today = remember { LocalDate.now() }` (evaluated once per composition, not live at midnight).
- Container: `Box`, if `today == date`, a 1dp `BorderStroke` ring in `colorScheme.onBackground`, `CircleShape`. Then `clip(CircleShape)`, `clickable(onDayClick)`, `circleLayout()`.
- `circleLayout()`: measures the child, `newDiameter = max(width, height)`, lays out a square of that size with the child centred. Result: circular hit area and ring.
- Day number: `date.dayOfMonth.toString()`, 16sp, SemiBold, centred. Colour: `onBackground` at alpha `DISABLED_ALPHA` (0.38) when `date.isBefore(today)`, else `onBackground`.
- Event dots: `Row(Modifier.offset(y = 12.dp))`, `size = min(events, MAX_EVENTS = 3)`; for `index in 0 until size`: `CalendarIndicator(index, size = 56.dp, color = colorScheme.primary)`.

### 8.4 Dot (CalendarIndicator.kt)
- Box with `padding(horizontal = 1.dp)`, `clip(CircleShape)`, `background(color.copy(alpha = (index + 1) * 0.3f))`, `size(56.dp / 12)`.
- Dot diameter = 56 / 12 = 4.666... dp. Alpha by index: 0 → 0.3, 1 → 0.6, 2 → 0.9.
- A day with 5 items shows 3 dots (no "+2" text). The count itself is not displayed anywhere in the calendar.

### 8.5 Today is not highlighted in the list
The list has no "today" marker. Date headers use relative text ("Today", "Tomorrow", "In N days").

---

## 9. Layout breakpoints (OBSERVED)

- `isTabletUi()` (`app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt:20`): `Configuration.smallestScreenWidthDp >= 720` (`TABLET_UI_REQUIRED_SCREEN_WIDTH_DP = 720`, line 12).
- Upcoming: `if (isTabletUi())` → `*LargeImpl` (two panels: `TwoPanelBox` with calendar as `startContent`, list as `endContent`), else `*SmallImpl` (single `FastScrollLazyColumn`, calendar as item 0 with key `"upcoming-calendar"`).
- `isMediumWidthWindow()` = `screenWidthDp > 600` (`MediumWidthWindowSize = 600.dp`). `isExpandedWidthWindow()` = `screenWidthDp > 840` (`ExpandedWidthWindowSize = 840.dp`). Calendar max width 360dp applies when `600 < width <= 840`.
- Proposal (desktop): the Android smallest-width rule does not fit a resizable desktop window. Use the GTK window's width: two panels when the content area is `>= 720` px, single column below. Calendar width cap 360 px when the content width is in `(600, 840]`. Document this as the desktop replacement in the design.

---

## 10. Library update trigger and new-count reset

### 10.1 updateLibrary() (OBSERVED)
- MUSM: `val started = MangaLibraryUpdateJob.startNow(Injekt.get<Application>())`, then `screenModelScope.launch { _events.send(Event.LibraryUpdateTriggered(started)) }`, returns `started`.
- AUSM: same with `AnimeLibraryUpdateJob.startNow`.
- `startNow(context, category = null): Boolean` (`MangaLibraryUpdateJob.kt:488`): if WorkManager reports the TAG as running, return `false` ("Already running either as a scheduled or manual job"). Else enqueue a `OneTimeWorkRequest` unique work `WORK_NAME_MANUAL` with `ExistingWorkPolicy.KEEP` and return `true`.
- Snackbar via `LaunchedEffect(Unit) { screenModel.events.collectLatest { ... } }`:
  - `InternalError` → `context.stringResource(MR.strings.internal_error)` = "InternalError: Check crash logs for further information".
  - `LibraryUpdateTriggered(started = true)` → `MR.strings.updating_library` = "Updating library".
  - `LibraryUpdateTriggered(started = false)` → `MR.strings.update_already_running` = "An update is already running".
  - Presented with `snackbarHostState.showSnackbar(message)`.
- `_events` is `Channel(Int.MAX_VALUE)` (unbounded buffer). Events emitted while no collector is active are queued and shown when the tab next composes (OBSERVED).

### 10.2 Pull-to-refresh (presentation, MangaUpdatesScreen.kt)
- `PullRefresh(refreshing = isRefreshing, enabled = !state.selectionMode, onRefresh = { val started = onUpdateLibrary(); if (!started) return; isRefreshing = true; delay(1 s); isRefreshing = false })`.
- OBSERVED: the spinner is fake and lasts exactly 1 second, regardless of the real job. If `startNow` returns false, no spinner. The snackbar still appears from the event.

### 10.3 Last updated line
- `lastUpdated` = `libraryPreferences.lastUpdatedTimestamp()` (key `Preference.appStateKey("library_update_last_timestamp")`, default `0L`).
- Written by the library update job at the end of a run: `libraryPreferences.lastUpdatedTimestamp().set(Instant.now().toEpochMilli())` (`MangaLibraryUpdateJob.kt:116`, `AnimeLibraryUpdateJob.kt:110`).
- Rendered as the first list row: `MR.strings.updates_last_update_info` = "Library last updated: %s" with `relativeTimeSpanString(lastUpdated)` (section 6.5), `FontStyle.Italic`, padding horizontal `padding.medium`, vertical `padding.small`.

### 10.4 New-updates counter reset
- `DisposableEffect(Unit) { screenModel.resetNewUpdatesCount(); onDispose { screenModel.resetNewUpdatesCount() } }`. The count is reset to 0 both when the tab is entered and when it is left.
- `resetNewUpdatesCount()`: MUSM sets `libraryPreferences.newMangaUpdatesCount()` (key `library_unread_updates_count`) to 0; AUSM sets `newAnimeUpdatesCount()` (key `library_unseen_updates_count`) to 0.
- The count is incremented by the library update job (`MangaLibraryUpdateJob.kt:287`, `AnimeLibraryUpdateJob.kt:302`) and drives the Updates nav badge (`HomeScreen.kt:269-270`, see area:36-ui-shell-navigation-security-webview).

---

## 11. Tab wiring (MangaUpdatesTab.kt, AnimeUpdatesTab.kt)

- `navigator = LocalNavigator.currentOrThrow`; `screenModel = rememberScreenModel { MUSM() }`.
- `navigateUp`: non-null only when `fromMore` (the tab is opened from the More tab under the MOVE_HISTORY_TO_MORE / nav-style setting). Lambda: if `navigator.lastItem == HomeScreen` → `HomeScreen.openTab(HomeScreen.Tab.AnimeLib())`, else `navigator.pop()`. (AUSM is identical.)
- `TabContent(titleRes = AYMR.strings.label_updates | AYMR.strings.label_anime_updates, searchEnabled = false, content, actions, navigateUp)`.
  - Manga title string: `AYMR.strings.label_updates` (value "Manga" in the aniyomi i18n base file, it is the manga tab label because the Updates screen has an Anime/Manga pair). Anime title: `AYMR.strings.label_anime_updates` = "Anime".
- Content callbacks: `onClickCover` → `navigator.push(MangaScreen(item.update.mangaId))` / `AnimeScreen(item.update.animeId)`; `onOpenChapter` → `ReaderActivity.newIntent(context, mangaId, chapterId)` + `startActivity`; `onOpenEpisode` → section 14.
- Toolbar actions:
  - No selection: [`AppBar.Action(MR.strings.action_view_upcoming = "View Upcoming Updates", CalendarMonth icon, navigator.push(UpcomingMangaScreen()) / UpcomingAnimeScreen())`, `AppBar.Action(MR.strings.action_update_library = "Update library", Refresh icon, updateLibrary())`].
  - With selection (`selected.isNotEmpty()`): [`MR.strings.action_select_all = "Select all"` (SelectAll icon, `toggleAllSelection(true)`), `MR.strings.action_select_inverse = "Select inverse"` (FlipToBack icon, `invertSelection()`)].
- Dialog routing: `Dialog.DeleteConfirmation` → `UpdatesDeleteConfirmationDialog(isManga = true|false)`; `Dialog.ShowQualities` (AUSM only) → `NavigatorAdaptiveSheet(EpisodeOptionsDialogScreen(useExternalDownloader, episodeTitle, episodeId, animeId, sourceId))`. `null` → nothing.
- `LaunchedEffect(state.isLoading) { if (!isLoading) (context as? MainActivity)?.ready = true }`: signals the Activity that the first frame has real content (splash gate). See section 14.

---

## 12. Row presentation and gestures (OBSERVED, details in `p2-ui-parity.md`)

### 12.1 Progress text
- Manga: `readProgress = lastPageRead.takeIf { !read && it > 0L }?.let { stringResource(MR.strings.chapter_progress, it + 1) }`. `chapter_progress` = "Page: %1$d". So a stored 0-based `lastPageRead` of 4 shows "Page: 5".
- Anime: `watchProgress = lastSecondSeen.takeIf { !seen && it > 0L }?.let { stringResource(AYMR.strings.episode_progress, formatProgress(it), formatProgress(totalSeconds)) }`. `episode_progress` = "%1$s/%2$s", e.g. "12:34/24:00".
- Shown after a `DotSeparatorText` (a middle dot separator) and at `DISABLED_ALPHA`.

### 12.2 Text alpha and indicators
- `textAlpha = if (read|seen) DISABLED_ALPHA (0.38) else 1f`, applied to title and chapter/episode name.
- Unread dot: `Icons.Filled.Circle`, height 8dp, end padding 4dp, `colorScheme.primary`, shown when `!read` (manga) / `!seen` (anime). Content description `MR.strings.unread` = "Unread".
- Bookmark: `Icons.Filled.Bookmark`, `colorScheme.primary`, shown when `bookmark`, content description `MR.strings.action_filter_bookmarked` = "Bookmarked", followed by 2dp spacer.
- Fillermark (anime only): `Icons.AutoMirrored.Filled.Label`, `colorScheme.tertiary`, shown when `fillermark`, content description `AYMR.strings.action_filter_fillermarked` = "Fillermarked", followed by 2dp spacer.
- Icon max height = text height minus 2dp.

### 12.3 Row geometry and click rules
- Row height 56dp, horizontal padding `padding.medium`, vertically centred. Selected background via `Modifier.selectedBackground(selected)`.
- Cover: `ItemCover.Square`, `padding(vertical = 6dp)`, `fillMaxHeight()`, `onClick = onClickCover` (null when `selectionMode`, so the cover is not clickable in selection mode).
- Middle column: `weight(1f)`, horizontal padding `padding.medium`. Line 1 title `bodyMedium`, 1 line, ellipsis. Line 2 `Row` with icons + name (`bodySmall`, 1 line, ellipsis, `weight(1f, fill = false)`) + optional progress.
- Click (`combinedClickable`): `onClick` → if `selectionMode` toggle (userSelected true, fromLongPress false) else open the update (reader or player). `onLongClick` → toggle with `userSelected = true, fromLongPress = true` plus haptic feedback.
- Download indicator: `ChapterDownloadIndicator` / `EpisodeDownloadIndicator`, `enabled = onDownloadChapter != null` (null when `selectionMode`), start padding 4dp. Click → `onDownloadChapter(listOf(item), action)`.

### 12.4 Bottom action bar (OBSERVED; `EntryBottomActionMenu` in area:38-ui-entry-details-manga, area:39-ui-entry-details-anime)
Visible when `selected.isNotEmpty()`. Each action is included only when its `takeIf` predicate holds. Manga (`MangaUpdatesBottomBar`):
- Bookmark (`onBookmarkClicked`): when `any { !bookmark }`. Action = `bookmarkUpdates(selected, true)`.
- Remove bookmark: when `all { bookmark }`. Action = `bookmarkUpdates(selected, false)`.
- Mark as viewed/read: when `any { !read }`. Action = `markUpdatesRead(selected, true)`.
- Mark as unviewed/unread: when `any { read || lastPageRead > 0 }`. Action = `markUpdatesRead(selected, false)`.
- Download: when `any { state != DOWNLOADED }`. Action = `downloadChapters(selected, START)`.
- Delete: when `any { state == DOWNLOADED }`. Action = `showConfirmDeleteChapters(selected)`.
Anime (`AnimeUpdatesBottomBar`) adds, in this order: Bookmark / Remove bookmark (same rules), Fillermark (when `any { !fillermark }`) / Remove fillermark (when `all { fillermark }`), Mark as viewed (`any { !seen }`) / Mark as unviewed (`any { seen || lastSecondSeen > 0 }`), Download, Delete, Play externally (when `!alwaysUseExternalPlayer && size == 1`), Play internally (when `alwaysUseExternalPlayer && size == 1`). Both play actions call `onOpenEpisode(selected[0], true)`.

### 12.5 Row tap, anime routing (AnimeUpdatesTab.openEpisode, OBSERVED)
```
extPlayer = playerPreferences.alwaysUseExternalPlayer().get() != altPlayer
MainActivity.startPlayerActivity(context, animeId, episodeId, extPlayer, sourceId)
```
- Row tap calls `onClickUpdate(item, false)`. With `alwaysUseExternalPlayer = false` → internal. With `true` → external.
- Bottom "Play externally" and "Play internally" call with `altPlayer = true`, flipping the preference.
- Launched in `scope.launchIO { openEpisode(...) }`.

---

## 13. Navigation and bottom-nav coupling

- `LaunchedEffect(state.selectionMode) { HomeScreen.showBottomNav(!state.selectionMode) }`. OBSERVED: no explicit restore when the tab is left during selection. Proposal: clear selection on tab exit and restore the nav bar.
- Re-selecting the Updates tab in the bottom bar opens the download queue (spec/app.json, area:36-ui-shell-navigation-security-webview). Not implemented in this scope.
- Cover tap pushes the entry screen (area:38-ui-entry-details-manga, area:39-ui-entry-details-anime). Upcoming item tap pushes the entry screen.
- Upcoming title bar: `navigateUp = navigator::pop`. Help button → `uriHandler.openUri(Constants.URL_HELP_UPCOMING)` = `https://aniyomi.org/docs/faq/updates/upcoming`.

---

## 14. Android-only behaviours and their Linux decisions

| Behaviour (OBSERVED) | Android mechanism | Decision | User loss |
|---|---|---|---|
| Long-press haptic feedback on rows | `LocalHapticFeedback` | Drop. Optional: no-op. | No vibration (no vibration motor on desktop). |
| Pull-to-refresh gesture to start library update | `PullRefresh` | Replace with a Refresh toolbar button and `F5` / `Ctrl+R` shortcut. | No pull gesture (touch-only). |
| BackHandler clears selection | `androidx.activity.compose.BackHandler` | Replace with `Escape` key. | Hardware back key replaced by Escape. |
| `MainActivity.ready = true` on first loaded frame | Splash gate in Activity | Drop, or use `gtk::Window` presentation after first data load (keep the "show after data" idea). | None visible. |
| `WorkManager.isRunning(TAG)` | AndroidX WorkManager | Replace with a process-wide `AtomicBool`/state flag in the library-update module (area:26-library-update-jobs). | None. |
| `ExistingWorkPolicy.KEEP` unique work | WorkManager | Replace with a single-flight guard (`Mutex`/`try_lock`) that returns false when a run is active. | None. |
| `android.text.format.DateUtils.getRelativeTimeSpanString` | Android framework | Own formatter (section 6.5). | Exact localised wording of the platform string. |
| `Configuration.smallestScreenWidthDp` tablet rule | Android config | Window-width rule (section 9). | Layout switch point differs on desktop. |
| Voyager `Screen`, `ScreenModel`, `Navigator` | Voyager | Plain Rust structs plus GTK stack push/pop. | None. |
| `Injekt.get()` DI | Injekt | Constructor injection. | None. |
| `SnackbarHostState.showSnackbar` | Compose Material3 | `gtk::Revealer` toast at the bottom of the window, auto-hides after 4 s (Material Short duration), queued if several. | Compose snackbar action slot unused in scope anyway. |
| `Modifier.animateItem` / `animateItemFastScroll` | Compose lazy animations | Drop. | No insert/remove animation. |
| `FastScrollLazyColumn` fast-scroll thumb | presentation-core | GTK overlay scrollbar (kinetic + drag). | Custom thumb replaced by the GTK scrollbar. |
| `TwoPanelBox` | presentation-core | `gtk::Paned` or fixed two-column `Box`. | None. |

---

## 15. Interactions with other areas (summary)

- area:23-downloads-manga / area:24-downloads-anime downloads: queue `statusFlow`/`progressFlow`, `getQueuedDownloadOrNull`, `isChapterDownloaded`/`isEpisodeDownloaded` (name-based), `downloadChapters`/`downloadEpisodes`, `startDownloadNow`, `cancelQueuedDownloads`, `deleteChapters`/`deleteEpisodes`, `startDownloads`.
- area:25-downloads-shared downloads shared: `DownloadCache.changes` used in the combine trigger.
- area:26-library-update-jobs library update jobs: `startNow` semantics, `lastUpdatedTimestamp` write, new-count increments.
- area:05-data-repos-entries-items / area:07-domain-entries-items / area:08-domain-library-category-history-updates data and domain: `GetChapter`, `GetEpisode`, `GetManga`, `GetAnime`, `UpdateChapter`, `UpdateEpisode` (`awaitAll`), `ChapterUpdate`/`EpisodeUpdate` shapes.
- area:02-preferences-system preferences: `asState`, `appStateKey`.
- area:34-images-covers-cache covers: `ItemCover` loading and `MangaCover`/`AnimeCover` objects.
- area:36-ui-shell-navigation-security-webview shell: `HomeScreen.showBottomNav`, `HomeScreen.openTab`, badges, `MainActivity` ready gate.
- area:38-ui-entry-details-manga / area:39-ui-entry-details-anime entry screens and reader/player: `MangaScreen`, `AnimeScreen`, `ReaderActivity.newIntent`, `MainActivity.startPlayerActivity`, `EpisodeOptionsDialogScreen`, `ChapterDownloadIndicator`, `EpisodeDownloadIndicator`, `EntryBottomActionMenu`.
- area:40-ui-browse-manga / area:41-ui-browse-anime browse: not used by this scope.
