# Topic: tracking sheet and its child screens

Scope files: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt` (874 lines, all read), `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackItem.kt` (6 lines), and the tracking parts of `AnimeScreenModel.kt` (1492-1533 observers, 912-931 refresh, 868-910 mark-seen prompt) and `AnimeScreen.kt` (166-172, 317-327).

Callees (contracts only): `app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/track/DeletableAnimeTracker.kt`, `app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt`, `app/src/main/java/eu/kanade/domain/track/anime/interactor/RefreshAnimeTracks.kt`, `app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt`, `app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt`, `domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt`, `app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt`, `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`, `app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt`.

Repo-relative paths; `/home/blackbox/aniyomi-src` is the root. The presentation composables (`eu/kanade/presentation/track/*`, `eu/kanade/presentation/track/anime/*`) belong to the tracking area and are named, not specified, here.

---

## 1. Data

### 1.1 Domain track (domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt:5-20)

| Field | Type | Meaning |
|---|---|---|
| `id` | Long | row id |
| `animeId` | Long | owning anime |
| `trackerId` | Long | tracker id (see 1.3) |
| `remoteId` | Long | id on the remote service |
| `libraryId` | Long? | remote list entry id (nullable) |
| `title` | String | title on the remote |
| `lastEpisodeSeen` | Double | last episode number recorded on the remote |
| `totalEpisodes` | Long | remote total (0 = unknown) |
| `status` | Long | tracker-specific status code (see 1.4) |
| `score` | Double | tracker-specific score (see 5.3) |
| `remoteUrl` | String | URL on the remote (may be empty) |
| `startDate` | Long | epoch millis, 0 = unset (interpreted in the system zone wall clock, see 6.2) |
| `finishDate` | Long | epoch millis, 0 = unset |
| `private` | Boolean | tracked privately on the remote |

The DB model is `DbAnimeTrack` (`eu.kanade.tachiyomi.data.database.models.anime.AnimeTrack`). `AnimeTrack.toDbTrack()` (app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt:17-31) copies every field and renames: `remoteId -> remote_id`, `libraryId -> library_id`, `lastEpisodeSeen -> last_episode_seen`, `totalEpisodes -> total_episodes`, `remoteUrl -> tracking_url`, `startDate -> started_watching_date`, `finishDate -> finished_watching_date`, `animeId -> anime_id`, `trackerId -> tracker_id` (via `create(trackerId)`), and `private -> private`.

`toDomainTrack(idRequired)` (AnimeTrack.kt:33) is the inverse; returns null when `id` is null and `idRequired` is true.

Important: the child screens capture the **domain** track object at navigation time and convert it with `toDbTrack()` at write time (see 8, defect D-1).

### 1.2 AnimeTrackItem (AnimeTrackItem.kt:6)

`data class AnimeTrackItem(val track: AnimeTrack?, val tracker: Tracker)`. `track` is null when the user has logged into the service but has not bound this anime to it.

### 1.3 Tracker ids (TrackerManager.kt:20-37)

`ANILIST = 2L`, `KITSU = 3L`, `KAVITA = 8L`, `SIMKL = 101L`, `JELLYFIN = 102L`. Others: MyAnimeList `1L`, Shikimori `4L`, Bangumi `5L`, Komga `6L`, MangaUpdates `7L`, Suwayomi `9L`.

### 1.4 Status codes

Status is a tracker-specific Long. The list of selectable statuses comes from `tracker.animeService.getStatusListAnime()`, and labels from `getStatusForAnime(status): StringResource?`. The per-tracker codes are in each tracker file (areas 28-33) and are not restated here.

Generic semantics implemented by `AnimeTracker` (AnimeTracker.kt:27-40): `getCompletionStatus()`, `getWatchingStatus()`, `getRewatchingStatus()`.

### 1.5 Score

- `getScoreList(): ImmutableList<String>` is the list of selectable score labels.
- `indexToScore(index: Int): Double = index.toDouble()` (AnimeTracker.kt:31-33), overridable by trackers.
- `displayScore(track): String` gives the label for the stored score.
- `get10PointScore(track)` exists for the 10-point normalization (AnimeTracker.kt:43-45).

---

## 2. Home sheet (`AnimeTrackInfoDialogHomeScreen`, lines 92-285)

### 2.1 Construction

`AnimeTrackInfoDialogHomeScreen(animeId: Long, animeTitle: String, sourceId: Long)`. Pushed inside the sheet host by `NavigatorAdaptiveSheet(screen = AnimeTrackInfoDialogHomeScreen(animeId = successState.anime.id, animeTitle = successState.anime.title, sourceId = successState.source.id), enableSwipeDismiss = { it.lastItem is AnimeTrackInfoDialogHomeScreen }, onDismissRequest = onDismissRequest)` (AnimeScreen.kt:317-327).

### 2.2 Content

- `screenModel = rememberScreenModel { Model(animeId, sourceId) }`.
- `dateFormat = remember { UiPreferences.dateFormat(Injekt<UiPreferences>().dateFormat().get()) }` (lines 103-107). The date format preference is owned by area 02.
- `state` collected; `AnimeTrackInfoDialogHome(trackItems = state.trackItems, dateFormat, onStatusClick, onEpisodeClick, onScoreClick, onStartDateEdit, onEndDateEdit, onNewSearch, onOpenInBrowser, onRemoved, onCopyLink, onTogglePrivate)`.

Callbacks and their effects:

| Callback | Argument | Effect |
|---|---|---|
| `onStatusClick` | `AnimeTrackItem` | push `TrackStatusSelectorScreen(track = it.track!!, serviceId = it.tracker.id)` |
| `onEpisodeClick` | item | push `TrackEpisodeSelectorScreen(track!!, serviceId)` |
| `onScoreClick` | item | push `TrackScoreSelectorScreen(track!!, serviceId)` |
| `onStartDateEdit` | item | push `TrackDateSelectorScreen(track!!, serviceId, start = true)` |
| `onEndDateEdit` | item | push `TrackDateSelectorScreen(track!!, serviceId, start = false)` |
| `onNewSearch` | item | if `item.tracker is EnhancedAnimeTracker`: `screenModel.registerEnhancedTracking(item)`; else push `TrackServiceSearchScreen(animeId, initialQuery = it.track?.title ?: animeTitle, currentUrl = it.track?.remoteUrl, serviceId = it.tracker.id)` |
| `onOpenInBrowser` | item | `openTrackerInBrowser(context, item)`: `url = track?.remoteUrl ?: return`; if not blank: `context.openInBrowser(url)` |
| `onRemoved` | item | push `TrackerAnimeRemoveScreen(animeId, track = it.track!!, serviceId)` |
| `onCopyLink` | item | `context.copyTrackerLink(item)`: same URL rule, `copyToClipboard(url, url)` |
| `onTogglePrivate` | item | `screenModel.togglePrivate(item)` |

`it.track!!` is evaluated for status, episode, score, both dates and remove. The presentation shows these only for bound rows (part 1). Calling them for an unbound row throws `NullPointerException` (see D-7).

### 2.3 Model init (lines 207-225)

Two launches on the screen model scope:
1. `refreshTrackers()` (240-260).
2. `getTracks.subscribe(animeId).catch { logcat(ERROR, it) }.distinctUntilChanged().map { it.mapToTrackItem() }.collectLatest { trackItems -> mutableState.update { it.copy(trackItems = trackItems) } }`.

### 2.4 `refreshTrackers()` (240-260)

- `RefreshAnimeTracks.await(animeId)` (see 2.5 below for semantics).
- `.filter { it.first != null }` then `forEach { (track, e) -> logcat(ERROR, e) { "Failed to refresh track data mangaId=$animeId for service ${track!!.id}" }; withUIContext { context.toast(context.stringResource(MR.strings.track_error, track!!.name, e.message ?: "")) } }`.
- Note: the log text says `mangaId` in an anime file (copy-paste; keep the meaning, correct the word).

### 2.5 `RefreshAnimeTracks.await(animeId)` (app/src/main/java/eu/kanade/domain/track/anime/interactor/RefreshAnimeTracks.kt:25-45)

- `supervisorScope`. `getTracks.await(animeId)` mapped to `(track, trackerManager.get(track.trackerId))`, filtered to `service?.isLoggedIn == true`.
- Each in parallel (`async`): `updatedTrack = service.animeService.refresh(track.toDbTrack()).toDomainTrack()!!`; `insertTrack.await(updatedTrack)`; `syncEpisodeProgressWithTrack.await(animeId, updatedTrack, service.animeService)`; returns null on success.
- On `Throwable`: returns `service to e`.
- Result: `awaitAll().filterNotNull()`. Failures only.

### 2.6 `registerEnhancedTracking(item)` (227-238)

- `item.tracker as EnhancedAnimeTracker` (cast, smart-casts the item's tracker).
- `launchNonCancellable`:
  - `anime = Injekt.get<GetAnime>().await(animeId) ?: return@launchNonCancellable`.
  - `try { matchResult = item.tracker.match(anime) ?: throw Exception(); item.tracker.animeService.register(matchResult, animeId) } catch (e: Exception) { withUIContext { Injekt.get<Application>().toast(MR.strings.error_no_match) } }`.
- Note the toast uses the Application context from Injekt, not the screen context.
- `match(anime)` contract (EnhancedAnimeTracker.kt:30): "similar to Tracker.search, but only return zero or one match".
- The `accept(source)` check is not repeated here; the list only shows accepted enhanced trackers (2.8).

### 2.7 `togglePrivate(item)` (262-266)

`launchNonCancellable { (item.tracker as? AnimeTracker)?.setRemotePrivate(item.track!!.toDbTrack(), !item.track.private) }`.

`setRemotePrivate(track, private)` (AnimeTracker.kt:105-108): sets `track.private` and calls `updateRemote(track)`.

### 2.8 `mapToTrackItem()` (268-278)

```
loggedInTrackers = TrackerManager.loggedInTrackers().filter { it is AnimeTracker }
source = AnimeSourceManager.getOrStub(sourceId)
loggedInTrackers
  .map { service -> AnimeTrackItem(find { it.trackerId == service.id }, service) }
  .filter { (it.tracker as? EnhancedAnimeTracker)?.accept(source) ?: true }
```

- Non-enhanced trackers always pass. Enhanced trackers pass only when they accept the source (`accept` = `source::class.qualifiedName in getAcceptedSources()`, EnhancedAnimeTracker.kt:16-18).
- Difference from the summary collector in the ScreenModel (`p2-behavior.md` section 16): that collector does not filter by `AnimeTracker` and does not reuse this list. The two can disagree on `trackItems` vs `trackingCount`. **Proposal:** one shared function.

### 2.9 State

`@Immutable data class State(val trackItems: List<AnimeTrackItem> = emptyList())` (280-283).

---

## 3. Status selector (`TrackStatusSelectorScreen`, lines 287-340)

- Constructed with `track: DbAnimeTrack` and `serviceId`.
- `Content()`: `screenModel = Model(track, tracker = TrackerManager.get(serviceId)!!)`. The `!!` throws if the service is not registered (D-6).
- `TrackStatusSelector(selection = state.selection, onSelectionChange = setSelection, selections = remember { getSelections() }, onConfirm = { setStatus(); navigator.pop() }, onDismissRequest = navigator::pop)`.
- Model state: `State(selection: Long)`, initialized to `track.status`.
- `getSelections(): Map<Long, StringResource?>` = `tracker.animeService.getStatusListAnime().associateWith { (tracker as? AnimeTracker)?.getStatusForAnime(it) }`.
- `setSelection(selection: Long)`: copies state.
- `setStatus()`: `launchNonCancellable { tracker.animeService.setRemoteAnimeStatus(track.toDbTrack(), state.value.selection) }`.

`setRemoteAnimeStatus(track, status)` (AnimeTracker.kt:67-73):
```
track.status = status
if (track.status == getCompletionStatus() && track.total_episodes != 0L) {
    track.last_episode_seen = track.total_episodes.toDouble()
}
updateRemote(track)
```

So choosing the "completed" status on a tracker with a known total forces the last-seen episode to the total.

---

## 4. Episode selector (`TrackEpisodeSelectorScreen`, lines 342-403)

- `TrackItemSelector(selection = state.selection, onSelectionChange, range = remember { getRange() }, onConfirm = { setEpisode(); pop }, onDismissRequest = pop, isManga = false)`.
- Model state: `State(selection: Int)` initialized to `track.lastEpisodeSeen.toInt()` (Double truncated toward zero).
- `getRange(): Iterable<Int>`:
  - `endRange = if (track.totalEpisodes > 0) track.totalEpisodes else 10000`.
  - Returns `0..endRange.toInt()`, inclusive on both ends.
  - Edge: `totalEpisodes` is Long; `toInt()` truncates values above `Int.MAX_VALUE`.
- `setEpisode()`: `launchNonCancellable { tracker.animeService.setRemoteLastEpisodeSeen(track.toDbTrack(), state.value.selection) }`.

`setRemoteLastEpisodeSeen(track, episodeNumber: Int)` (AnimeTracker.kt:75-88):
```
if (track.last_episode_seen == 0.0 &&
    track.last_episode_seen < episodeNumber &&
    track.status != getRewatchingStatus()) {
    track.status = getWatchingStatus()
}
track.last_episode_seen = episodeNumber.toDouble()
if (track.total_episodes != 0L && track.last_episode_seen.toLong() == track.total_episodes) {
    track.status = getCompletionStatus()
    track.finished_watching_date = System.currentTimeMillis()
}
updateRemote(track)
```

Observed rules:
- A bound track with `lastEpisodeSeen == 0` that receives a positive episode moves to the watching status (unless rewatching).
- Reaching the total episode count moves to completed and stamps the finish date with the current time (not the picker value). The finish date is therefore not user-chosen in this path.
- Setting the value to 0 keeps the status unchanged (no reverse transition).
- `last_episode_seen == 0.0 && last_episode_seen < episodeNumber` is equivalent to `episodeNumber > 0` at this point.

---

## 5. Score selector (`TrackScoreSelectorScreen`, lines 405-457)

- `TrackScoreSelector(selection: String, onSelectionChange, selections = remember { getSelections() }, onConfirm = { setScore(); pop }, onDismissRequest = pop)`.
- Model state: `State(selection: String)` initialized to `tracker.animeService.displayScore(track)`.
- `getSelections(): ImmutableList<String> = tracker.animeService.getScoreList()`.
- `setScore()`: `launchNonCancellable { tracker.animeService.setRemoteScore(track.toDbTrack(), state.value.selection) }`.

`setRemoteScore(track, scoreString)` (AnimeTracker.kt:90-93): `track.score = indexToScore(getScoreList().indexOf(scoreString)); updateRemote(track)`.

Consequence (D-4): when the stored display string is not in the score list, `indexOf` returns -1 and the score becomes `indexToScore(-1)`. For the default `indexToScore` that is `-1.0`, which is written to the remote. Kurobayomi must refuse the write and show an error in that case.

---

## 6. Date selectors (`TrackDateSelectorScreen`, lines 459-589; `TrackDateRemoverScreen`, lines 591-677)

### 6.1 Picker validity (`selectableDates`, lines 465-520)

An anonymous `SelectableDates` (Material3 interface) with two methods.

`isSelectableDate(utcTimeMillis: Long): Boolean`:
```
dateToCheck = Instant.ofEpochMilli(utcTimeMillis).atZone(ZoneOffset.systemDefault()).toLocalDate()
if dateToCheck > LocalDate.now(): return false                  // no future dates
if start && track.finishDate > 0:
    dateFinished = Instant.ofEpochMilli(track.finishDate).atZone(ZoneId.systemDefault()).toLocalDate()
    return dateToCheck <= dateFinished                          // start <= finish
else if !start && track.startDate > 0:
    dateStarted = Instant.ofEpochMilli(track.startDate).atZone(ZoneId.systemDefault()).toLocalDate()
    return dateToCheck >= dateStarted                           // finish >= start
else: return true
```

`isSelectableYear(year: Int): Boolean`:
```
if year > LocalDate.now().year: return false
if start && track.finishDate > 0: return year <= finishDate.year (system zone)
else if !start && track.startDate > 0: return year >= startDate.year (system zone)
else: return true
```

Note D-5: `utcTimeMillis` is UTC midnight for the picked day. Converting it to the system zone shifts the date back one day for zones west of UTC. The future check and the bound check then compare the shifted date (see D-5 and the Rust rule in section 6.3).

### 6.2 Initial value and writes (lines 561-583)

- `initialSelection: Long` getter (UTC-based):
  ```
  millis = (if start track.startDate else track.finishDate).takeIf { it != 0L } ?: Instant.now().toEpochMilli()
  return millis.convertEpochMillisZone(ZoneOffset.systemDefault(), ZoneOffset.UTC)
  ```
- `setDate(millis: Long)` (UTC millis from the picker):
  ```
  localMillis = millis.convertEpochMillisZone(ZoneOffset.UTC, ZoneOffset.systemDefault())
  launchNonCancellable { if start setRemoteStartDate(track.toDbTrack(), localMillis) else setRemoteFinishDate(track.toDbTrack(), localMillis) }
  ```
- `Long.convertEpochMillisZone(from, to)` (app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:29-36): `LocalDateTime.ofInstant(Instant.ofEpochMilli(this), from).atZone(to).toInstant().toEpochMilli()`. It keeps the **wall-clock** time and changes only the zone interpretation. So a local midnight stored as a system-zone value becomes the same wall clock in UTC for the picker, and back again on write.

`setRemoteStartDate(track, epochMillis)` (AnimeTracker.kt:95-98) and `setRemoteFinishDate` (100-103): set the field and `updateRemote(track)`.

### 6.3 Rust rule for the picker

Input from the picker is a calendar date. Store it as a **local midnight** in the system zone (matching the observed write, which keeps wall clock). Validity compares `LocalDate` values directly, with no zone conversion of the picker value. This fixes D-5 and gives the same result as the observed behavior in every zone.

### 6.4 Date writes from the UI (`onConfirm`)

Both selectors call `setDate(it)` then `navigator.pop()`. `onRemove` is `takeIf { canRemove }`, where `canRemove = if (start) track.startDate > 0 else track.finishDate > 0` (lines 533-537).

### 6.5 Date remover (`TrackDateRemoverScreen`)

- `AlertDialogContent`, icon `Icons.Default.Delete`, title `MR.strings.track_remove_date_conf_title` ("Remove date?").
- Text: `serviceName = tracker.name`; `track_remove_start_date_conf_text` or `track_remove_finish_date_conf_text` with `%s` = service name.
- Buttons: `TextButton(Cancel, pop)`; `FilledTonalButton(Remove, errorContainer)` which runs `screenModel.removeDate()` then `navigator.popUntil { it is AnimeTrackInfoDialogHomeScreen }`.
- `removeDate()`: `launchNonCancellable { if (start) setRemoteStartDate(track.toDbTrack(), 0) else setRemoteFinishDate(track.toDbTrack(), 0) }`.

---

## 7. Track search and bind (`TrackServiceSearchScreen`, lines 679-770)

### 7.1 Screen

- `TrackServiceSearchScreen(animeId, initialQuery, currentUrl: String?, serviceId)` (public data class).
- `Content()`: `screenModel = Model(animeId, currentUrl, initialQuery, tracker = TrackerManager.get(serviceId)!!)`. `textFieldState = rememberTextFieldState(initialQuery)`.
- `AnimeTrackerSearch(state = textFieldState, onDispatchQuery = { screenModel.trackingSearch(textFieldState.text.toString()) }, queryResult = state.queryResult, selected = state.selected, onSelectedChange = screenModel::updateSelection, onConfirmSelection = { private -> val selected = state.selected ?: return; selected.private = private; screenModel.registerTracking(selected); navigator.pop() }, onDismissRequest = navigator::pop, supportsPrivateTracking = screenModel.supportsPrivateTracking)`.

### 7.2 Model

- `supportsPrivateTracking = tracker.supportsPrivateTracking` (Tracker.kt:21).
- `init`: `if (initialQuery.isNotBlank()) trackingSearch(initialQuery)` (lines 727-732). The first search runs on open.
- `trackingSearch(query)` (734-754), in `screenModelScope.launch`:
  1. `mutableState.update { copy(queryResult = null, selected = null) }` (loading state).
  2. `result = withIOContext { try { Result.success(tracker.animeService.searchAnime(query)) } catch (e: Throwable) { Result.failure(e) } }`.
  3. `mutableState.update { old -> old.copy(queryResult = result, selected = result.getOrNull()?.find { it.tracking_url == currentUrl }) }`.
- `registerTracking(item)`: `launchNonCancellable { tracker.animeService.register(item, animeId) }`.
- `updateSelection(selected)`: copy.

### 7.3 `AnimeTracker.register(item, animeId)` (AnimeTracker.kt:58-65)

```
item.anime_id = animeId
try { addTracks.bind(this, item, animeId) } catch (e: Throwable) { withUIContext { toast(e.message) } }
```

### 7.4 `AddAnimeTracks.bind(tracker, item, animeId)` (app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt:32-79)

Runs under `withNonCancellableContext` and `withIOContext`:
1. `allChapters = getEpisodesByAnimeId.await(animeId)`; `hasSeenEpisodes = allChapters.any { it.seen }`.
2. `tracker.bind(item, hasSeenEpisodes)` (remote call, returns the bound track).
3. `track = item.toDomainTrack(idRequired = false) ?: return`; `insertTrack.await(track)`.
4. If `hasSeenEpisodes`:
   1. `latestLocalReadChapterNumber = allChapters.sortedBy { it.episodeNumber }.takeWhile { it.seen }.lastOrNull()?.episodeNumber ?: -1.0`. This is the highest episode of the **leading run** of seen episodes in ascending order, so a gap stops the run.
   2. If it is greater than `track.lastEpisodeSeen`: set `lastEpisodeSeen` and `tracker.setRemoteLastEpisodeSeen(track.toDbTrack(), latestLocalReadChapterNumber.toInt())`.
   3. If `track.startDate <= 0`: first history entry by `seenAt` (`GetAnimeHistory.await(animeId).sortedBy { it.seenAt }.firstOrNull()?.seenAt`). If present: `startDate = seenAt.time.convertEpochMillisZone(systemDefault, UTC)`; `tracker.setRemoteStartDate(track.toDbTrack(), startDate)`.
5. `syncChapterProgressWithTrack.await(animeId, track, tracker)`.

### 7.5 Preselection and confirm

- On each search result `selected = results.find { it.tracking_url == currentUrl }`. This is how the currently bound entry is pre-highlighted when the user re-searches from an existing binding.
- `currentUrl` is `track?.remoteUrl` from the home item (2.2). For a new bind it is null, see D-3.
- Confirm: `selected.private = private`, `registerTracking(selected)`, pop.

### 7.6 Enhanced path

Enhanced trackers never reach this screen from the home sheet (2.2 `onNewSearch`); they register by match (2.6).

---

## 8. Tracker removal (`TrackerAnimeRemoveScreen`, lines 772-874)

- Constructed with `animeId`, domain `AnimeTrack` (not DB), `serviceId`.
- `Content()`: `serviceName = screenModel.getName()` (= `tracker.name`); `removeRemoteTrack` remembered state, default false.
- `AlertDialogContent`, icon Delete, title `MR.strings.track_delete_title` ("Remove %s tracking?" with service name), text Column:
  - `Text(MR.strings.track_delete_text)` ("This will remove the tracking locally.").
  - If `screenModel.isDeletable()` (`tracker is DeletableAnimeTracker`): `LabeledCheckbox(label = MR.strings.track_delete_remote_text ("Also remove from %s"), checked = removeRemoteTrack, onCheckedChange)`.
- Buttons: `TextButton(Cancel, pop)`; `FilledTonalButton(OK, errorContainer)`:
  1. `screenModel.unregisterTracking(serviceId)`.
  2. `if (removeRemoteTrack) screenModel.deleteAnimeFromService()`.
  3. `navigator.pop()`.
- `unregisterTracking(serviceId)`: `launchNonCancellable { deleteTrack.await(animeId, serviceId) }` (DeleteAnimeTrack, domain/src/main/java/tachiyomi/domain/track/anime/interactor/DeleteAnimeTrack.kt). Removes the local row only.
- `deleteAnimeFromService()`: `launchNonCancellable { try { (tracker as DeletableAnimeTracker).delete(track) } catch (e: Exception) { logcat(ERROR) } }`. The remote delete uses the captured domain `track`. The local and remote operations run in two separate coroutines with no ordering.

`DeletableAnimeTracker.delete(track)` (DeletableAnimeTracker.kt:49-51): suspend, implemented only by trackers that support remote deletion.

---

## 9. Auto-track on mark seen and on add (cross-reference)

- Mark seen prompt (AnimeScreenModel.kt:868-910): `behavior` section 12. Uses `AutoTrackState` (`ALWAYS`, `ASK`, `NEVER`), default `ALWAYS`, from preference key `pref_auto_update_manga_on_mark_read` (TrackPreferences.kt:48-51).
- Track sheet after library add: `behavior` section 4.6. Preference `track_on_adding_to_library`, default true (TrackPreferences.kt:41).
- `TrackEpisode.await(context, animeId, episodeNumber, setupJobOnFailure = true)` (app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt:25-58): updates only trackers whose `lastEpisodeSeen < episodeNumber` and that are logged in. Online: `refresh`, `update(track, true)`, insert, remove from delayed store. Offline: add to `DelayedAnimeTrackingStore` and schedule `DelayedAnimeTrackingUpdateJob` when `setupJobOnFailure`.

---

## 10. Sheet navigation graph

```
[Tracking home sheet] (AnimeTrackInfoDialogHomeScreen)
  |-- status         -> [TrackStatusSelector]           pop on confirm/dismiss
  |-- episodes       -> [TrackEpisodeSelector]          pop on confirm/dismiss
  |-- score          -> [TrackScoreSelector]            pop on confirm/dismiss
  |-- start date     -> [TrackDateSelector(start)]
  |     '-- remove   -> [TrackDateRemover] --popUntil--> [home]
  |-- finish date    -> [TrackDateSelector(finish)]
  |     '-- remove   -> [TrackDateRemover] --popUntil--> [home]
  |-- new search     -> [TrackServiceSearch]            pop on bind
  |-- remove         -> [TrackerAnimeRemove]            pop on OK
  '-- (enhanced)     -> registerEnhancedTracking        no navigation
```

Swipe-to-dismiss is enabled only when the top screen is the home (AnimeScreen.kt:324). Dropped in Kurobayomi (task 068).

---

## 11. Defects and Proposals (summary, cross-referenced to `p2-behavior.md` section 23 and `p2-edge-cases-and-tests.md`)

| ID | Defect | Where | Proposal |
|---|---|---|---|
| D-1 | Child screens write a captured (stale) track object; `update(track)` sends the whole object | lines 331, 364-366, 391-396, 448, 578-580, 670-673, 831-833 | Re-read the row by `(anime_id, tracker_id)` before each write |
| D-2 | `!!` on `track` and `tracker` (`it.track!!`, `TrackerManager.get(serviceId)!!`) | 116-135, 174, 298, 353, 416, 528, 785 | Make these `Option`s and disable the actions when absent |
| D-3 | Preselection matches `null` URLs when `currentUrl` is null | 750 | Require `currentUrl` to be `Some` |
| D-4 | Score not found in list stores -1 | AnimeTracker.kt:90-93 | Reject the write |
| D-5 | Date picker validity uses system zone on UTC millis, off by a day west of UTC | 468-471, 497-499 | Compare `LocalDate` values without a zone shift |
| D-6 | Unknown `serviceId` throws (`!!` on `TrackerManager.get`) | 298, 353, 416, 528, 785 | Return an error screen |
| D-7 | Actions on unbound items throw | 116-178 | Only expose actions for bound rows |
| D-8 | `tracking_url` is `lateinit`; a search result without it throws inside `mutableState.update` | 750; AnimeTrackSearch.kt:35 | `Option<String>` |
| D-9 | Log message names `mangaId` in the anime sheet | 248 | Fix wording |
| D-10 | Home list filters by `AnimeTracker` and accept, summary collector does not | 269-277 vs ScreenModel 1501-1505 | One shared function |
| D-11 | Remote and local delete are not ordered | 863-873 | Local delete after remote succeeds, or report both results |
| D-12 | Track date selector stores UTC-wall-clock values as system-zone values, relying on `convertEpochMillisZone` | 568, 574-575 | Store local midnight in system zone; compare `LocalDate` |

---

## 12. Rust mapping

| Kotlin | Rust |
|---|---|
| `AnimeTrack` (domain) | `struct TrackRow { id, anime_id, tracker_id, remote_id, library_id: Option<i64>, title, last_episode_seen: f64, total_episodes: i64, status: i64, score: f64, remote_url, start_date: i64, finish_date: i64, private: bool }` |
| `AnimeTrackItem` | `struct TrackItem { track: Option<TrackRow>, tracker: TrackerRef }` |
| `AnimeTracker` remote setters | `trait AnimeTrackerService { async fn update(&self, track: &mut TrackRow, did_watch: bool) -> Result<TrackRow>; ... }` with default methods `set_status`, `set_last_episode_seen`, `set_score`, `set_start_date`, `set_finish_date`, `set_private` implemented exactly as in 3-6 |
| `EnhancedAnimeTracker` | `trait EnhancedTracker { fn accept(&self, source: &SourceRef) -> bool; async fn match_anime(&self, anime: &AnimeRef) -> Result<Option<TrackSearch>>; }` |
| `DeletableAnimeTracker` | `trait DeletableTracker { async fn delete(&self, track: &TrackRow) -> Result<()>; }` |
| `TrackEpisode` | `async fn track_episode(ctx, anime_id, episode_number: f64, offline_store: &DelayedStore) -> ()` |
| `AddAnimeTracks.bind` | `async fn bind_track(&self, tracker, item, anime_id) -> Result<()>` with the leading-run rule from 7.4 |
| `convertEpochMillisZone` | `fn wall_clock_to_zone(ms: i64, from: Tz, to: Tz) -> i64` using `chrono-tz` or `jiff` |
| Tracking sheet navigation | `enum TrackPage { Home, Status(TrackId), Episode(TrackId), Score(TrackId), Date { track, start }, DateRemove { track, start }, Search { anime_id, query, current_url, tracker }, Remove { anime_id, track, tracker } }` held as a `Vec<TrackPage>` stack |

---

## 13. Tests (summary)

- Episode setter: `setRemoteLastEpisodeSeen` table (0 -> n transitions, rewatching, total reached, zero).
- Status setter: completion with known total copies total into last-seen.
- Score: index lookup; missing string rejected.
- Date validity: start <= finish, finish >= start, no future, year checks, UTC-west zone regression (D-5).
- Date remover: popUntil target is the home page.
- Search: preselect by URL; null URL not preselected (D-3); result without URL becomes an error, not a crash (D-8).
- Bind: leading-run rule (gap stops the run); start date from first history entry only when unset.
- Remove: local removal always; remote delete only when the tracker is deletable and the checkbox is set.
