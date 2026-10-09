# area:27-tracking-core part 2 — behavior spec (track dialogs, track items, tracker login callback)

Repo root: `/home/blackbox/aniyomi-src`. All paths repo-relative. `OBSERVED` = what the Kotlin does (with file:line). `Proposal:` = what Kurobayomi (Rust + GTK4) should do. Part-1 behavior (`Tracker`/`AnimeTracker` update rules, `AddAnimeTracks`, `RefreshAnimeTracks` internals, presentation composables) is in `p1-behavior.md`; it is referenced here only where a dialog depends on it.

Files in this spec: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt` (A), `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt` (M; line numbers identical to A for every shared construct), `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackItem.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackItem.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt`.

---

## 0. Object graph (OBSERVED)

- Entry point for the whole family: `AnimeTrackInfoDialogHomeScreen(animeId: Long, animeTitle: String, sourceId: Long)` (A:92-96). Manga: `MangaTrackInfoDialogHomeScreen(mangaId, mangaTitle, sourceId)` (M:92-96). Opened from the entry details "Tracking" action (`app/src/main/java/eu/kanade/presentation/track/...` part 1 callers).
- Every sub-screen is a Voyager `Screen` pushed onto the same `LocalNavigator` (A:99, 294, 349, 413, 525, 600, 688, 780). `navigator.pop()` / `popUntil { it is AnimeTrackInfoDialogHomeScreen }` (A:645) closes them.
- Each screen has a `ScreenModel` (`StateScreenModel<State>` for status, episode, score, search; plain `ScreenModel` for date selector, date remover, remove-tracking). The `ScreenModel` is created with `rememberScreenModel { Model(...) }`, so it lives as long as the screen.
- Tracker lookups: `Injekt.get<TrackerManager>().get(serviceId)!!` (A:298, 353, 416, 528, 603, 694, 785). `get` returns `Tracker?` (`TrackerManager.kt:332`). The `!!` throws `NullPointerException` if the id is not one of the 11 registered trackers (`TrackerManager.kt:307-317`). OBSERVED; reachable only with a stale id.
- Services used by the Model classes: `GetAnimeTracks` (subscription), `RefreshAnimeTracks`, `GetAnime`, `AnimeSourceManager`, `DeleteAnimeTrack`, `TrackerManager`, `Application` (toast), `UiPreferences` (date format). OBSERVED from the `Injekt.get` calls.

## 1. Track row fields used by these dialogs

Domain model `tachiyomi.domain.track.anime.model.AnimeTrack` (`domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt:5-20`): `id: Long`, `animeId: Long`, `trackerId: Long`, `remoteId: Long`, `libraryId: Long?`, `title: String`, `lastEpisodeSeen: Double`, `totalEpisodes: Long`, `status: Long`, `score: Double`, `remoteUrl: String`, `startDate: Long`, `finishDate: Long`, `private: Boolean`.

Manga domain model `tachiyomi.domain.track.manga.model.MangaTrack` (`domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt`): same shape with `mangaId`, `lastChapterRead: Double`, `totalChapters: Long`.

Note: the Kotlin dialogs import the domain class under the alias `DbAnimeTrack` / `DbMangaTrack` (A:90, M:90). Both names refer to the domain class. Do not confuse with the legacy class `eu.kanade.tachiyomi.data.database.models.anime.AnimeTrack` that `toDbTrack()` produces.

Conversion used before every remote call (OBSERVED, `app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt:17-31`):
- `toDbTrack()` builds `DbAnimeTrack.create(trackerId)` and copies `id`, `anime_id`, `remote_id`, `library_id`, `title`, `last_episode_seen`, `total_episodes`, `status`, `score`, `tracking_url` (from `remoteUrl`), `started_watching_date` (from `startDate`), `finished_watching_date` (from `finishDate`), `private`.
- Manga: `app/src/main/java/eu/kanade/domain/track/manga/model/MangaTrack.kt` with `last_chapter_read`, `total_chapters`, `started_reading_date`, `finished_reading_date`.

Remote update path after every setter (part 1, `AnimeTracker.updateRemote`, `app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt:215-225`): calls `update(track)` on the remote, then `track.toDomainTrack(idRequired = false)` and `insertTrack.await(...)`. On exception: logs `"Failed to update remote track data id=${track.id}"` and toasts `e.message`. So a dialog's remote write persists locally only if the remote call succeeded.

## 2. Anime versus manga: the only differences (OBSERVED, diff of A and M)

| Concern | Anime (A) | Manga (M) |
|---|---|---|
| Home params | `animeId`, `animeTitle` | `mangaId`, `mangaTitle` |
| Progress action | `onEpisodeClick` → `TrackEpisodeSelectorScreen` | `onChapterClick` → `TrackChapterSelectorScreen` |
| Progress field | `track.lastEpisodeSeen.toInt()` (A:374) | `track.lastChapterRead.toInt()` (M:374) |
| Total field | `track.totalEpisodes` (A:377-378) | `track.totalChapters` (M:377-378) |
| Progress setter | `setRemoteLastEpisodeSeen(track, Int)` | `setRemoteLastChapterRead(track, Int)` |
| Progress selector label | `isManga = false` (AYMR "Episodes") (A:367) | `isManga = true` (MR "Chapters") (M:367) |
| Enhanced check | `EnhancedAnimeTracker` | `EnhancedMangaTracker` |
| Source/entry lookups | `GetAnime`, `AnimeSourceManager`, `getOrStub` | `GetManga`, `MangaSourceManager`, `getOrStub` |
| Status list | `animeService.getStatusListAnime()`, `getStatusForAnime` | `mangaService.getStatusListManga()`, `getStatusForManga` |
| Remote delete | `DeletableAnimeTracker.delete(track)` ; log `"Failed to delete anime entry from service"` (A:865) | `DeletableMangaTracker.delete(track)` ; log `"Failed to delete manga entry from service"` (M:865) |
| Refresh error log | `"Failed to refresh track data mangaId=$animeId for service ${track!!.id}"` (A:248) | `"... mangaId=$mangaId for service ${track!!.name}"` (M:248) |
| Start/finish date fields | `startDate`/`finishDate` mapped to `started_watching_date`/`finished_watching_date` | mapped to `started_reading_date`/`finished_reading_date` |
| Date picker title | `track_started_reading_date` / `track_finished_reading_date` (shared "reading" wording for anime too, A:540-542) | same |
| Remove screen name | `TrackerAnimeRemoveScreen` | `TrackerMangaRemoveScreen` |

OBSERVED defect: the anime refresh log message says `mangaId=` (A:248). Kept as a divergence in Rust (task T-27b-034).

Proposal: one Rust implementation parameterised by `EntryKind` (see `p2-rust-design.md` section 3). Labels, field selectors and service calls switch on the kind.

---

## 3. Home screen: `AnimeTrackInfoDialogHomeScreen` (A:92-285; manga M:92-285)

### 3.1 Construction (OBSERVED, A:101-108)
- `dateFormat = remember { UiPreferences.dateFormat(Injekt.get<UiPreferences>().dateFormat().get()) }`. Preference `app_date_format`, default `""` (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:35`). `UiPreferences.dateFormat(format)` (UiPreferences.kt:44-47): `""` → `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)`; otherwise `DateTimeFormatter.ofPattern(format, Locale.getDefault())`.
- `state` is collected from `screenModel.state` (`collectAsState`).

### 3.2 Model init (OBSERVED, A:207-225)
Two concurrent `screenModelScope.launch` blocks, not sequenced:
1. `refreshTrackers()` (A:209). Runs on every construction of the screen model, i.e. every time the dialog is opened.
2. Subscription (A:212-224):
   - `getTracks.subscribe(animeId)` (Flow of `List<AnimeTrack>`, backed by `AnimeTrackRepository.getTracksByAnimeIdAsFlow`).
   - `.catch { logcat(LogPriority.ERROR, it) }` (swallowed; the flow completes, nothing is shown).
   - `.distinctUntilChanged()`.
   - `.map { it.mapToTrackItem() }`.
   - `.collectLatest { trackItems → mutableState.update { it.copy(trackItems = trackItems) } }`.

### 3.3 `refreshTrackers()` (OBSERVED, A:240-260)
1. `refreshTracks.await(animeId)` → `List<Pair<Tracker?, Throwable>>` (failed updates only; see `RefreshAnimeTracks.kt:25-44`: only trackers where `isLoggedIn == true` are refreshed, each in its own `async`, all awaited with `supervisorScope`; the per-tracker step is `service.animeService.refresh(track.toDbTrack()).toDomainTrack()!!` → `insertTrack.await` → `syncEpisodeProgressWithTrack.await(animeId, updatedTrack, service.animeService)`; any Throwable is captured as a failure).
2. For each failure `(track, e)` where `track != null` (filter at A:245): logs `ERROR` with the message in section 2 (table row "Refresh error log"), then `withUIContext { context.toast(context.stringResource(MR.strings.track_error, track!!.name, e.message ?: "")) }`.
   - Message key `track_error` = `"%1$s error: %2$s"` (i18n/src/commonMain/moko-resources/base/strings.xml:800). Example: `AniList error: Unauthorized`. A null exception message yields `"AniList error: "` (empty second part).
   - Note: `track!!.name` for the message (tracker display name). The log uses `track!!.id` (anime) or `track!!.name` (manga).
3. No success toast. No retry. No progress indicator. Refresh results do not appear in the dialog except through the subscription (step 3.2) because `syncEpisodeProgressWithTrack` and `insertTrack` write to the DB.
4. Refresh is not run for trackers that are not logged in (filter at `RefreshAnimeTracks.kt:29`), and not for trackers that have no local track row (it iterates over `getTracks.await(animeId)`, i.e. existing rows only).

### 3.4 `mapToTrackItem()` (OBSERVED, A:268-278)
Input: the `List<AnimeTrack>` (domain rows for this anime).
1. `loggedInTrackers = TrackerManager.loggedInTrackers().filter { it is AnimeTracker }` (`TrackerManager.kt:324` = `trackers.filter { it.isLoggedIn }`). `isLoggedIn` = `getUsername().isNotEmpty() && getPassword().isNotEmpty()` (`BaseTracker.kt:85-87`). The snapshot is taken at each emission, not reactively (see defect D-4).
2. `source = AnimeSourceManager.getOrStub(sourceId)` (returns a stub source when the id is unknown).
3. `.map { service -> AnimeTrackItem(find { it.trackerId == service.id }, service) }`. So there is exactly one item per logged-in anime tracker. `track` is the row whose `trackerId` equals the tracker id, or null.
4. `.filter { (it.tracker as? EnhancedAnimeTracker)?.accept(source) ?: true }`. Non-enhanced trackers always pass. Enhanced trackers pass only when `accept(source)` is true. Default `accept` (`EnhancedAnimeTracker.kt:244-246`): `source::class.qualifiedName in getAcceptedSources()`. For a stub source, the qualified name is not in the list, so enhanced trackers are hidden for entries whose source is not installed. (Derived from code, not run.)
5. Result order = the order of `TrackerManager.trackers` (`TrackerManager.kt:319-322`): MyAnimeList(1), AniList(2), Kitsu(3), Shikimori(4), Bangumi(5), Komga(6), MangaUpdates(7), Kavita(8), Suwayomi(9), Simkl(101), Jellyfin(102), filtered by logged-in and kind.

Manga difference: the same steps with `is MangaTracker`, `EnhancedMangaTracker`, `MangaSourceManager`. The comment at M:276 reads "this manga's source".

### 3.5 Row actions (OBSERVED, A:110-181)
Each callback takes the `AnimeTrackItem` `it` for the row.

| Callback (Home) | Behavior | Navigation |
|---|---|---|
| `onStatusClick` (A:113-120) | `it.track!!` (NPE if null; row hides this action when track is null, see section 11) | push `TrackStatusSelectorScreen(track = it.track!!, serviceId = it.tracker.id)` |
| `onEpisodeClick` (A:121-128) | same | push `TrackEpisodeSelectorScreen(track, serviceId)` |
| `onScoreClick` (A:129-136) | same | push `TrackScoreSelectorScreen(track, serviceId)` |
| `onStartDateEdit` (A:137-145) | same | push `TrackDateSelectorScreen(track, serviceId, start = true)` |
| `onEndDateEdit` (A:146-154) | same | push `TrackDateSelectorScreen(track, serviceId, start = false)` |
| `onNewSearch` (A:155-168) | if `it.tracker is EnhancedAnimeTracker` → `screenModel.registerEnhancedTracking(it)` (no navigation); else push `TrackServiceSearchScreen(animeId, initialQuery = it.track?.title ?: animeTitle, currentUrl = it.track?.remoteUrl, serviceId = it.tracker.id)` | section 4 / 10 |
| `onOpenInBrowser` (A:169) | `openTrackerInBrowser(context, it)` | section 3.6 |
| `onRemoved` (A:170-178) | push `TrackerAnimeRemoveScreen(animeId, track = it.track!!, serviceId)` | section 10 |
| `onCopyLink` (A:179) | `context.copyTrackerLink(it)` | section 3.7 |
| `onTogglePrivate` (A:180) | `screenModel::togglePrivate` | section 3.8 |

Note that `onNewSearch` on a row that already has a track still opens the search with the current title and URL, so re-matching is the same flow as first-time binding.

### 3.6 `openTrackerInBrowser` (OBSERVED, A:187-192)
`url = trackItem.track?.remoteUrl ?: return`. If `url.isNotBlank()` → `context.openInBrowser(url)` (`ContextExtensions.kt:71-83`: `Intent(ACTION_VIEW, uri)`, `startActivity`; the `forceDefaultBrowser` flag is not used here). If the row has no track or the URL is blank, nothing happens and no message is shown.

### 3.7 `copyTrackerLink` (OBSERVED, A:194-199)
Same URL guard. Then `copyToClipboard(url, url)` (`ContextExtensions.kt:39-52`): returns if content is blank; sets clipboard with label = url and content = url; on Android API ≤ 32 (`S_V2`) shows toast `copied_to_clipboard` (`"Copied to clipboard:\n%1$s"`, i18n strings.xml:728) with the URL truncated by `truncateCenter(50)` (`core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt:24`, `replacement = "..."`). On newer Android the system shows its own confirmation.

### 3.8 `togglePrivate(item)` (OBSERVED, A:262-266)
`screenModelScope.launchNonCancellable { (item.tracker as? AnimeTracker)?.setRemotePrivate(item.track!!.toDbTrack(), !item.track.private) }`.
- Writes `private` remotely through `AnimeTracker.setRemotePrivate` (part 1: sets field, calls `updateRemote`, so the local row is updated only after the remote call succeeds).
- No optimistic UI update. The row changes when the subscription emits.
- `item.track!!` throws if the row has no track. The Home action is shown only when a track exists (see section 11).
- Action visible only when `supportsPrivateTracking` (section 11).

### 3.9 `registerEnhancedTracking(item)` (OBSERVED, A:227-238)
Called for enhanced trackers from `onNewSearch`. Runs `screenModelScope.launchNonCancellable`.
1. `item.tracker as EnhancedAnimeTracker` (cast; no-op if not that type).
2. `anime = Injekt.get<GetAnime>().await(animeId) ?: return@launchNonCancellable` (silent return if the entry is gone).
3. `try { matchResult = item.tracker.match(anime) ?: throw Exception(); item.tracker.animeService.register(matchResult, animeId) } catch (e: Exception) { withUIContext { toast(MR.strings.error_no_match) } }`.
   - `match` returns zero or one `AnimeTrackSearch` (`EnhancedAnimeTracker.kt:255-258`).
   - `register` (part 1, `AnimeTracker.kt:163-170`) catches its own `Throwable`s and toasts `e.message`; so the outer catch only sees exceptions from `match`.
   - Message `error_no_match` = `"No match found"` (strings.xml:799).
4. There is no loading state and no check that the tracker is logged in (the row is only shown for logged-in trackers, so practically it is).

Manga: identical with `EnhancedMangaTracker`, `GetManga`, `mangaService.register`.

---

## 4. Remote-title search: `TrackServiceSearchScreen(animeId, initialQuery, currentUrl, serviceId)` (A:679-770)

### 4.1 Construction and first search (OBSERVED, A:718-732)
- `supportsPrivateTracking = tracker.supportsPrivateTracking` (A:725).
- `init { if (initialQuery.isNotBlank()) trackingSearch(initialQuery) }` (A:727-732). So the search starts immediately when the screen opens, using `track?.title ?: animeTitle` from the Home (A:162).
- The Compose text field is seeded with the same string (`rememberTextFieldState(initialQuery)`, A:700) and nothing else.

### 4.2 `trackingSearch(query)` (OBSERVED, A:734-754)
1. `screenModelScope.launch` (not cancellable, not cancelled on new searches: defect D-6).
2. Immediately sets `queryResult = null` and `selected = null` (loading state).
3. `result = withIOContext { try { Result.success(tracker.animeService.searchAnime(query)) } catch (e: Throwable) { Result.failure(e) } }`. `searchAnime` is part 1 (`AnimeTracker.searchAnime(query: String): List<AnimeTrackSearch>`).
4. Sets `queryResult = result` and `selected = result.getOrNull()?.find { it.tracking_url == currentUrl }`.
   - Pre-selection matches on the tracking URL string, not on remote id. When `currentUrl` is null (no existing track), nothing matches because `tracking_url` is non-null.
5. Failure: `queryResult = Result.failure(e)`; the UI shows the error (see `p2-ui-parity.md`). The error text is the raw `Throwable` message. No `track_error` wrapping here (contrast with section 3.3).

Empty query: `onDispatchQuery` calls `trackingSearch(textFieldState.text.toString())` (A:703) with no blank check in this screen, so a blank query performs a search with an empty string (the remote decides). OBSERVED.

### 4.3 Confirm (OBSERVED, A:707-712)
`onConfirmSelection = f@{ private: Boolean -> val selected = state.selected ?: return@f; selected.private = private; screenModel.registerTracking(selected); navigator.pop() }`.
- If nothing is selected, the lambda returns early and the screen stays open.
- The `private` flag is written onto the search object before registration. Only honored when `supportsPrivateTracking` (the UI passes it; see `AnimeTrackerSearch.kt` in part 1).
- `registerTracking(item)` = `screenModelScope.launchNonCancellable { tracker.animeService.register(item, animeId) }` (A:756-758). `register` sets `item.anime_id = animeId` and calls `AddAnimeTracks.bind(this, item, animeId)` (part 1 `AnimeTracker.kt:163-170`). Errors are toasted by `register`.
- The screen pops immediately, before the bind finishes.

### 4.4 Selection update
`updateSelection(selected)` sets `selected` (A:760-762). Used by the list tap in the Compose UI.

---

## 5. Status selector: `TrackStatusSelectorScreen(track, serviceId)` (A:287-340)

- Model state: `State(selection: Long = track.status)` (A:317). Initial selection = current stored status id, even if it is not in the tracker's list.
- Options: `getSelections(): Map<Long, StringResource?>` (A:319-323) = `tracker.animeService.getStatusListAnime().associateWith { (tracker as? AnimeTracker)?.getStatusForAnime(it) }`. Keys are the tracker's numeric status ids, in the order `getStatusListAnime()` returns them. Labels are `StringResource?` (null label renders as empty text, `TrackInfoDialogSelector.kt:80`).
- The `selections` map is computed once: `remember { screenModel.getSelections() }` (A:305).
- `setSelection(selection: Long)` only updates state (A:325-327). Nothing is written until OK.
- Confirm (A:306-309): `screenModel.setStatus()` then `navigator.pop()`.
- `setStatus()` (A:329-333): `screenModelScope.launchNonCancellable { tracker.animeService.setRemoteAnimeStatus(track.toDbTrack(), state.value.selection) }`.
  - Part 1 `setRemoteAnimeStatus` (`AnimeTracker.kt:172-178`): sets `track.status = status`; if `status == getCompletionStatus()` and `track.total_episodes != 0L`, sets `last_episode_seen = total_episodes.toDouble()`; then `updateRemote`. So choosing "Completed" sets progress to the total when total is known. This also happens for manga through the manga counterpart (part 1 `MangaTracker`).
- Cancel: `onDismissRequest = navigator::pop` (A:310); nothing is written.
- Note: the status dialog does not validate the choice; the Compose radio list has no "none" entry.

Manga: `getStatusListManga()`, `getStatusForManga(it)`, `mangaService.setRemoteMangaStatus`.

## 6. Progress selector: `TrackEpisodeSelectorScreen` (A:342-403) and `TrackChapterSelectorScreen` (M:342-403)

- Initial state: `State(selection: Int = track.lastEpisodeSeen.toInt())` (A:374). `Double.toInt()` truncates toward zero (12.9 → 12). Fractional progress stored remotely is shown truncated and writing back an unchanged value stores the truncated integer (defect D-7).
- Range (OBSERVED, A:376-383):
  ```
  endRange = if (track.totalEpisodes > 0) track.totalEpisodes else 10000
  return 0..endRange.toInt()        // inclusive upper bound
  ```
  So with an unknown total the wheel offers 0..10000 (10 001 values). With total N it offers 0..N (N+1 values, the 0 is included). Unit: the integer count of episodes/chapters seen.
- The range is computed once: `remember { screenModel.getRange() }` (A:361).
- Wheel UI: `TrackItemSelector(selection, onSelectionChange, range, onConfirm, onDismissRequest, isManga)` (part 1, `TrackInfoDialogSelector.kt:97-118`): `WheelNumberPicker(items = range.toImmutableList(), startIndex = selection, ...)`. Because the range starts at 0, index == value.
- Title: `MR.strings.chapters` for manga (`"Chapters"`) or `AYMR.strings.episodes` for anime (`"Episodes"`, `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml:408`).
- Confirm: `setEpisode()` then pop. `setEpisode()` (A:389-396) = `setRemoteLastEpisodeSeen(track.toDbTrack(), state.value.selection)` in a non-cancellable scope.
  - Part 1 `AnimeTracker.setRemoteLastEpisodeSeen` (`AnimeTracker.kt:180-193`), exact logic:
    1. If `track.last_episode_seen == 0.0 && track.last_episode_seen < episodeNumber && track.status != getRewatchingStatus()` → `track.status = getWatchingStatus()`. (First progress on a zero-progress row moves it to "watching" unless it is rewatching.) The first clause is always true when the second is true, so the effective condition is "progress was 0 and new progress > 0 and status is not rewatching".
    2. `track.last_episode_seen = episodeNumber.toDouble()`.
    3. If `track.total_episodes != 0L && track.last_episode_seen.toLong() == track.total_episodes` → `track.status = getCompletionStatus()` and `track.finished_watching_date = System.currentTimeMillis()`.
    4. `updateRemote(track)`.
  - Manga counterpart `setRemoteLastChapterRead` is in part 1 (`MangaTracker.kt`).
- Cancel: pop, nothing written.
- Selecting a value below the current progress is allowed; no validation. Proposal keeps this (no rule in source).

## 7. Score selector: `TrackScoreSelectorScreen` (A:405-457)

- Initial state: `State(selection: String = tracker.animeService.displayScore(track))` (A:436). `displayScore` is part 1 (tracker-specific formatting; e.g. `"8.0"` for the default 10-point scale in the mockup). For an unscored row the value is whatever `displayScore` returns for score `-1.0` (part 1).
- Options: `getSelections(): ImmutableList<String> = tracker.animeService.getScoreList()` (A:438-440). Option strings are tracker-specific (part 1: `Tracker.getScoreList()`).
- `setSelection(String)` only updates state (A:442-444).
- `setScore()` (A:446-450): `setRemoteScore(track.toDbTrack(), state.value.selection)`.
  - Part 1 `AnimeTracker.setRemoteScore` (`AnimeTracker.kt:195-198`): `track.score = indexToScore(getScoreList().indexOf(scoreString))` then `updateRemote`. `indexToScore(index: Int): Double = index.toDouble()` by default (`AnimeTracker.kt:136-138`). A string not in the list gives `indexOf = -1` and score `-1.0` (defect D-8: an unknown string silently unsets the score).
  - Scores are stored as the list index, not as the displayed number. The mapping is tracker-specific; several trackers override `indexToScore` / `get10PointScore` (part 1).
- Wheel UI: `TrackScoreSelector` (part 1 `TrackInfoDialogSelector.kt:121-141`): `startIndex = selections.indexOf(selection).takeIf { it > 0 } ?: (selections.size / 2)`. Index 0 falls back to the middle (defect D-9, see section 18).
- Confirm → `setScore()` → pop. Cancel → pop.

## 8. Start/finish date selector: `TrackDateSelectorScreen(track, serviceId, start: Boolean)` (A:459-589)

### 8.1 Title and remove availability (OBSERVED, A:533-551)
- Title: `MR.strings.track_started_reading_date` = `"Start date"` if `start`, else `MR.strings.track_finished_reading_date` = `"Finish date"` (strings.xml:791-792).
- `canRemove = if (start) track.startDate > 0 else track.finishDate > 0` (A:533-537). `onRemove = { screenModel.confirmRemoveDate(navigator) }.takeIf { canRemove }` (A:550). So the "Remove" button shows only when a date is set (strictly greater than 0).

### 8.2 Initial selection (OBSERVED, A:561-569)
```
millis = (if (start) track.startDate else track.finishDate).takeIf { it != 0L } ?: Instant.now().toEpochMilli()
return millis.convertEpochMillisZone(ZoneOffset.systemDefault(), ZoneOffset.UTC)
```
- `convertEpochMillisZone(from, to)` (`app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:29-37`): `LocalDateTime.ofInstant(Instant.ofEpochMilli(this), from).atZone(to).toInstant().toEpochMilli()`. It re-reads the local wall-clock time in `from` and re-interprets it in `to`.
- So the picker is seeded with UTC midnight of the local calendar date that is stored (when the stored value is local midnight). For a date set without a time part this yields the same calendar day.
- Note: `ZoneOffset.systemDefault()` is inherited from `ZoneId.systemDefault()` in Java; it returns the JVM default zone.

### 8.3 Selectable dates (OBSERVED, A:465-520)
Anonymous `SelectableDates`:
- `isSelectableDate(utcTimeMillis)`:
  1. `dateToCheck = Instant.ofEpochMilli(utcTimeMillis).atZone(ZoneOffset.systemDefault()).toLocalDate()` (A:468-470).
  2. If `dateToCheck > LocalDate.now()` → false (no future dates; comment "Disallow future dates", A:472-475).
  3. If `start && track.finishDate > 0`: `dateFinished = Instant.ofEpochMilli(track.finishDate).atZone(ZoneId.systemDefault()).toLocalDate()`; return `dateToCheck <= dateFinished` (A:477-482).
  4. Else if `!start && track.startDate > 0`: `dateStarted` likewise; return `dateToCheck >= dateStarted` (A:483-488).
  5. Else true.
- `isSelectableYear(year)` (A:495-519): `year > LocalDate.now().year` → false; `start && finishDate > 0` → `year <= finishYear`; `!start && startDate > 0` → `year >= startYear`; else true.
- Observed rule set: start ≤ finish and finish ≥ start, neither in the future. Equal dates are allowed.
- Quirk (derived from code, defect D-10): the picker value is UTC midnight. `atZone(systemDefault())` converts it to local time before `toLocalDate()`. For users west of UTC (negative offset) UTC midnight is the previous local day, so the "future" check and the start/finish bounds are off by one day. Proposal: convert the picker value with `Instant.ofEpochMilli(x).atZone(ZoneOffset.UTC).toLocalDate()` (the picker's own zone).

### 8.4 Confirm (OBSERVED, A:546-549, Compose `TrackDateSelector` part 1 `TrackInfoDialogSelector.kt:186`)
- The Compose OK button calls `onConfirm(pickerState.selectedDateMillis!!)` (non-null assertion, defect D-11). The picker's `selectedDateMillis` is initialised from `initialSelection`, so it is normally non-null.
- `onConfirm = { screenModel.setDate(it); navigator.pop() }`.
- `setDate(millis)` (A:572-583): `localMillis = millis.convertEpochMillisZone(ZoneOffset.UTC, ZoneOffset.systemDefault())`, then in non-cancellable scope `setRemoteStartDate(track.toDbTrack(), localMillis)` if `start`, else `setRemoteFinishDate(...)`.
  - Stored value = local midnight of the chosen calendar day (`LocalDateTime` from UTC midnight re-read in local zone). Epoch millis, local-time based. This is the value the Home displays via `toLocalDate()`.
- Cancel: pop.
- `Remove` → `confirmRemoveDate(navigator)` → `navigator.push(TrackDateRemoverScreen(track, tracker.id, start))` (A:585-587). The date picker stays on the stack under the confirmation.

## 9. Date remover: `TrackDateRemoverScreen(track, serviceId, start)` (A:591-677)

- Dialog (Compose `AlertDialogContent`, A:607-656): icon `Icons.Default.Delete`; title `MR.strings.track_remove_date_conf_title` = `"Remove date?"` (strings.xml:801); text:
  - start: `MR.strings.track_remove_start_date_conf_text` = `"This will remove your previously selected start date from %s"` with the tracker name (strings.xml:802).
  - finish: `MR.strings.track_remove_finish_date_conf_text` = `"This will remove your previously selected finish date from %s"` (strings.xml:803).
- Buttons: `TextButton` "Cancel" (`action_cancel`, strings.xml:135) → pop; `FilledTonalButton` with `errorContainer` colours labelled `MR.strings.action_remove` = `"Remove"` (strings.xml:106) → `screenModel.removeDate()` then `navigator.popUntil { it is AnimeTrackInfoDialogHomeScreen }` (A:645). So confirming closes both the remover and the date picker and returns to Home.
- `removeDate()` (A:667-675): non-cancellable `setRemoteStartDate(track.toDbTrack(), 0)` or `setRemoteFinishDate(track.toDbTrack(), 0)`. Zero = "unset" (the Home shows the date only when `!= 0`).
- The remover does not check `supportsReadingDates`; the date rows are hidden for such trackers in Home, so the screen is unreachable for them.

## 10. Remove tracking: `TrackerAnimeRemoveScreen(animeId, track, serviceId)` (A:772-874)

### 10.1 Dialog (OBSERVED, A:778-847)
- `serviceName = tracker.name`. `removeRemoteTrack` = `remember { mutableStateOf(false) }` (A:789).
- Title: `MR.strings.track_delete_title` = `"Remove %s tracking?"` with the tracker name (strings.xml:804).
- Text: `MR.strings.track_delete_text` = `"This will remove the tracking locally."` (strings.xml:805). Arguments include the service name but the string has no placeholder (harmless).
- If `isDeletable()` (`tracker is DeletableAnimeTracker`, A:858): a `LabeledCheckbox` with label `MR.strings.track_delete_remote_text` = `"Also remove from %s"` (strings.xml:806), bound to `removeRemoteTrack`, default unchecked. Otherwise no checkbox.
- Buttons: "Cancel" (`action_cancel`) → pop; `FilledTonalButton` (errorContainer) with label `MR.strings.action_ok` = `"OK"` (strings.xml:136), not "Remove" (observed label inconsistency; Proposal keeps "OK" for parity, or switch to "Remove" in Rust; the spec is "Remove" per the mockup).
- Click order (A:832-835): `screenModel.unregisterTracking(serviceId)`; `if (removeRemoteTrack) screenModel.deleteAnimeFromService()`; `navigator.pop()`.

### 10.2 Model
- `isDeletable() = tracker is DeletableAnimeTracker` (A:858).
- `deleteAnimeFromService()` (A:860-868): `screenModelScope.launchNonCancellable { try { (tracker as DeletableAnimeTracker).delete(track) } catch (e: Exception) { logcat(ERROR, e) { "Failed to delete anime entry from service" } } }`. Errors are logged only, no toast.
- `unregisterTracking(serviceId)` (A:870-872): `screenModelScope.launchNonCancellable { deleteTrack.await(animeId, serviceId) }`.
- `DeleteAnimeTrack.await(animeId, trackerId)` (`domain/src/main/java/tachiyomi/domain/track/anime/interactor/DeleteAnimeTrack.kt:11-17`): `trackRepository.delete(animeId, trackerId)`; any `Exception` is logged and swallowed.

Effective ordering: the local delete and the remote delete are launched as two independent non-cancellable coroutines. The local delete is started first but the two can complete in either order. A remote failure leaves the local row deleted. A local failure does not stop the remote delete.

### 10.3 Which trackers are deletable (OBSERVED from class declarations)
- `DeletableAnimeTracker` implemented by: Anilist (`anilist/Anilist.kt:34-36`), Kitsu (`kitsu/Kitsu.kt:34-35`), MyAnimeList (`myanimelist/MyAnimeList.kt:32-33`), Shikimori (`shikimori/Shikimori.kt:33-34`). Their `delete(track)` implementations are at Anilist.kt:271, Kitsu.kt:164, MyAnimeList.kt:165, Shikimori.kt (see part 1).
- Not implemented for anime: Bangumi, Simkl, Jellyfin.
- `DeletableMangaTracker`: the four above plus MangaUpdates (`mangaupdates/MangaUpdates.kt:20`, `delete` at :83).

## 11. Gating matrix for actions (driven by Home, OBSERVED from `presentation/track/anime/AnimeTrackInfoDialogHome.kt:81-127`, part 1 owns the composable)

| Action | Shown when | Source |
|---|---|---|
| Whole row body (status, progress, score, dates) | `item.track != null` (AnimeTrackInfoDialogHome.kt:81) | Home |
| Status chip / click | row has track | Home (the status click handler `it.track!!`) |
| Progress (episodes/chapters) click | row has track | Home |
| Score row | `supportsScoring` and `item.track.score != 0.0` shows the text; the click is shown when `supportsScoring` (lines 99-102) | Home |
| Start date row | `supportsReadingDates && startDate != 0L` (line 109); click when `supportsReadingDates` (lines 110-111) | Home |
| Finish date row | `supportsReadingDates && finishDate != 0L` (line 113); click when `supportsReadingDates` (lines 114-115) | Home |
| Private toggle | `supportsPrivateTracking` (line 121) | Home |
| New search / bind | always (no track required) | Home |
| Remove | `item.track != null` (via `onRemoved`) | Home |

Note the score text is hidden when score is exactly `0.0`, but the score is `-1.0` for "unset" in search objects (`AnimeTrackSearch.score = -1.0`). Part 1 decides the stored default (domain rows use the DB default).

`supportsScoring` is defined in the Home composable (part 1, not in scope). `supportsReadingDates` default false (`BaseTracker.kt:76`). Trackers with `supportsReadingDates = true`: AniList (`anilist/Anilist.kt:62`), Kitsu (`kitsu/Kitsu.kt:47`), MyAnimeList (`myanimelist/MyAnimeList.kt:59`). `supportsPrivateTracking = true`: AniList (`anilist/Anilist.kt:64`), Bangumi (`bangumi/Bangumi.kt:32`), Kitsu (`kitsu/Kitsu.kt:49`). All others use the default false (`BaseTracker.kt:78`).

---

## 12. OAuth callback routing: `TrackLoginActivity` (OBSERVED)

### 12.1 Activity lifecycle (OBSERVED, `BaseOAuthLoginActivity.kt:31-39`)
1. `super.onCreate`.
2. `setComposeContent { LoadingScreen() }` (a spinner).
3. `handleResult(intent.data)` synchronously on the main thread, with the URI that launched the activity (null if none).
4. The activity stays on the spinner until a branch calls `returnToSettings()`.

### 12.2 Manifest (OBSERVED, `app/src/main/AndroidManifest.xml:243-261`)
- Activity `.ui.setting.track.TrackLoginActivity`, `android:exported="true"`, label `@string/track_activity_name` ("Tracker login", strings.xml:538).
- Intent filter: action `android.intent.action.VIEW`; categories DEFAULT and BROWSABLE; `data scheme="aniyomi"`; hosts `myanimelist-auth`, `anilist-auth`, `bangumi-auth`, `shikimori-auth`, `simkl-auth`.
- Note: `kitsu`, `mangaupdates`, `kavita`, `komga`, `suwayomi`, `jellyfin` are not in the filter (they use user/password login).

### 12.3 Routing table (OBSERVED, `TrackLoginActivity.kt:58-66`)
| `data.host` | Handler | Tracker (id) |
|---|---|---|
| `anilist-auth` | `handleAnilist(data)` | `trackerManager.aniList` (2) |
| `bangumi-auth` | `handleBangumi(data)` | `trackerManager.bangumi` (5) |
| `myanimelist-auth` | `handleMyAnimeList(data)` | `trackerManager.myAnimeList` (1) |
| `shikimori-auth` | `handleShikimori(data)` | `trackerManager.shikimori` (4) |
| `simkl-auth` | `handleSimkl(data)` (accepts null) | `trackerManager.simkl` (101) |
| any other host, or null | no branch | none: the spinner stays, the activity is never finished (defect D-1) |

Scheme is not checked in code (the intent filter enforces `aniyomi`).

### 12.4 AniList (implicit grant) (OBSERVED, `TrackLoginActivity.kt:68-80`)
1. `regex = "(?:access_token=)(.*?)(?:&)".toRegex()`. The regex requires a `&` after the token (a token at the very end of the fragment does not match).
2. `matchResult = regex.find(data.fragment.toString())`. The fragment is the part after `#`. If the fragment is null, `toString()` gives `"null"`, which does not match.
3. If `matchResult?.groups?.get(1) != null`: `lifecycleScope.launchIO { trackerManager.aniList.login(token); returnToSettings() }`. The token is group 1.
4. Else: `trackerManager.aniList.logout()` and `returnToSettings()` on the main thread.

Anilist `login(token)` (`anilist/Anilist.kt:346-356`): `oauth = api.createOAuth(token)`; `interceptor.setAuth(oauth)`; `(username, scoreType) = api.getCurrentUser()`; `scorePreference.set(scoreType)`; `saveCredentials(username.toString(), oauth.accessToken)`. Any Throwable → `logout()`. Note that `login` itself never rethrows, so `returnToSettings()` always runs.

`createOAuth(token)` (`anilist/AnilistApi.kt:515-517`) = `ALOAuth(token, "Bearer", System.currentTimeMillis() + 31536000000, 31536000000)`. The implicit flow's `expires_in` in the fragment is ignored; expiry is hard-coded to 365 days (defect D-12).

`getCurrentUser` is a GraphQL `Viewer` query returning `id`, `mediaListOptions.scoreFormat` (`anilist/AnilistApi.kt:519-527`).

### 12.5 Authorization-code trackers (Bangumi, MyAnimeList, Shikimori, Simkl) (OBSERVED)
Each handler: `code = data.getQueryParameter("code")` (Simkl uses `data?.getQueryParameter`). If `code != null`: `launchIO { tracker.login(code); returnToSettings() }`. Else `tracker.logout(); returnToSettings()`.

Login code path per tracker:
- Bangumi (`bangumi/Bangumi.kt:190-202`): `api.accessToken(code)` → `interceptor.newAuth(oauth)` → `api.getUsername()` (`GET https://api.bgm.tv/v0/me`, field `username`) → `saveCredentials(username, oauth.accessToken)`. Failure → `logout()`.
- MyAnimeList (`myanimelist/MyAnimeList.kt:251-259`): `api.getAccessToken(authCode)` (uses the PKCE verifier stored in the API object, see 13.2) → `interceptor.setAuth(oauth)` (which calls `myanimelist.saveOAuth`, `MyAnimeListInterceptor.kt:45-48`) → `api.getCurrentUser()` (`GET https://api.myanimelist.net/v2/users/@me`, string username) → `saveCredentials(username, oauth.accessToken)`. Failure → `logout()`.
- Shikimori (`shikimori/Shikimori.kt:223-232`): `api.accessToken(code)` → `interceptor.newAuth(oauth)` → `api.getCurrentUser()` (`GET .../api/users/whoami`, field `id` Int) → `saveCredentials(user.toString(), accessToken)`. Failure → `logout()`.
- Simkl (`simkl/Simkl.kt:122-131`): `api.accessToken(code)` → `interceptor.newAuth(oauth)` → `api.getCurrentUser()` (blocking `runBlocking` call to `GET https://api.simkl.com/users/settings`, field `account.id`; `simkl/SimklApi.kt:204-213`) → `saveCredentials(user.toString(), accessToken)`. Failure → `logout()`.

### 12.6 Logout (OBSERVED)
- `BaseTracker.logout()` (`BaseTracker.kt:80-83`, `@CallSuper`) = `trackPreferences.setCredentials(this, "", "")`. `setCredentials` (part 1 `TrackPreferences.kt:29-33`) writes username "", password "", and clears `auth_expired` to false.
- Each tracker's `logout()` then deletes the token preference (`trackToken(this).delete()`) and clears the interceptor auth (`interceptor.setAuth(null)` / `newAuth(null)`). Examples: `anilist/Anilist.kt:358-362`, `myanimelist/MyAnimeList.kt:262-266`, `bangumi/Bangumi.kt:216-220`, `shikimori/Shikimori.kt:246-250`, `simkl/Simkl.kt:145-149`.
- Logout clears everything for that tracker. It is also what `TrackLoginActivity` calls when the callback has no token or code. So a denied or cancelled authorization in the browser logs the user out of that tracker (defect D-2).

### 12.7 Completion
`returnToSettings()` (`BaseOAuthLoginActivity.kt:41-48`): `finish()`, then `startActivity(Intent(this, MainActivity::class.java).apply { addFlags(FLAG_ACTIVITY_CLEAR_TOP or FLAG_ACTIVITY_SINGLE_TOP) })`. The main app reopens at its root and the tracker settings are shown (part 47 owns the settings screen).
- Note: in the login branches `returnToSettings()` runs inside `launchIO` (background thread), which is a wrong-thread UI call on Android; the Linux port must marshal to the GTK main thread (section 14).

### 12.8 Browser launch side (part 1, for context)
Each tracker's `authUrl()` is opened with the system browser (part 47 Settings tracker row). Exact URLs are in `p2-data-and-config.md` section 5.

---

## 13. Messages (user-visible keys used by these files, exact English text)

| Key (MR.strings.*) | English | Where used (file:line) |
|---|---|---|
| `error_no_match` | "No match found" | A:235, M:235 (enhanced match failure toast) |
| `track_error` | "%1$s error: %2$s" | A:251-257, M:251-257 (refresh failure toast) |
| `track_started_reading_date` | "Start date" | A:540, A:625 |
| `track_finished_reading_date` | "Finish date" | A:542 |
| `track_remove_date_conf_title` | "Remove date?" | A:617 |
| `track_remove_start_date_conf_text` | "This will remove your previously selected start date from %s" | A:625 |
| `track_remove_finish_date_conf_text` | "This will remove your previously selected finish date from %s" | A:627 |
| `action_cancel` | "Cancel" | A:640, A:829 |
| `action_remove` | "Remove" | A:652 |
| `action_ok` | "OK" | A:842 |
| `track_delete_title` | "Remove %s tracking?" | A:800 |
| `track_delete_text` | "This will remove the tracking locally." | A:809 |
| `track_delete_remote_text` | "Also remove from %s" | A:813 |
| `status` (presentation) | "Status" | `TrackInfoDialogSelector.kt:57` |
| `score` (presentation) | "Score" | `TrackInfoDialogSelector.kt:130` |
| `chapters` (manga progress title) | "Chapters" | `TrackInfoDialogSelector.kt:105` |
| `AYMR.strings.episodes` (anime progress title) | "Episodes" | `TrackInfoDialogSelector.kt:105`, `i18n-aniyomi/.../strings.xml:408` |
| `copied_to_clipboard` (via copyToClipboard, API ≤ 32) | "Copied to clipboard:\n%1$s" | `ContextExtensions.kt:49` |
| `track_activity_name` | "Tracker login" | `AndroidManifest.xml:246` |

Toasts from part-1 code that the dialogs can trigger (listed for completeness): `AnimeTracker.register` failure toasts `e.message` raw (`AnimeTracker.kt:168`); `updateRemote` failure toasts `e.message` raw (`AnimeTracker.kt:223`). These are not localized; the message is whatever the tracker threw.

No dialog in this scope has a success message.

---

## 14. Concurrency and dispatch (OBSERVED)

| Operation | Scope | Dispatcher / cancellation |
|---|---|---|
| Home init refresh | `screenModelScope.launch` | Voyager ScreenModel scope (Main dispatcher); `RefreshAnimeTracks.await` uses `supervisorScope` + `async` per tracker, so one failed tracker does not cancel the others (RefreshAnimeTracks.kt:26, 31-40). Cancelled when the screen model is disposed. |
| Home subscription | `screenModelScope.launch` + `collectLatest` | cancelled with the screen model. `catch` logs the error and completes. |
| Toast in refresh | `withUIContext` | Main. |
| Search | `screenModelScope.launch` + `withIOContext` | Not cancelled when a new query starts (defect D-6). |
| Register / match / setters / delete | `screenModelScope.launchNonCancellable` | Continue after the screen is popped (intended: pop happens immediately). `launchNonCancellable` runs in a `NonCancellable` context so the network call completes even if the screen model is cleared. |
| Enhanced match | `launchNonCancellable` | as above. |
| Tracker login | `lifecycleScope.launchIO` | IO; then `returnToSettings` runs on that IO thread (defect-level wrong thread on Android). |
| Simkl identity | `runBlocking` inside `getCurrentUser()` | Blocks the calling IO thread (simkl/SimklApi.kt:204). |

Multiple dialogs can be open at once only through navigation (stack); there is no parallel dialog.

## 15. Timings, limits and constants (OBSERVED)
- Progress wheel max when total unknown: 10000 (A:380).
- Progress minimum: 0 (A:382, `0..endRange`).
- AniList token fake lifetime: 31 536 000 000 ms = 365 days (AnilistApi.kt:516).
- Toast truncation for clipboard message: `truncateCenter(50)` with `"..."` (ContextExtensions.kt:49).
- PKCE verifier: 50 random bytes, base64url without padding (PkceUtil.kt:8-14) → 67 characters.
- Clock: none in the dialogs. `System.currentTimeMillis()` sets `finished_watching_date` in part 1 when progress reaches total.
- No debounce on search; no max-length on the query; no pagination in this screen (part 1 `searchAnime` decides).
- No retry logic anywhere in these files.

## 16. Anime versus manga and the one shared implementation (Proposal)
Rust: one `TrackDialogs<K: EntryKind>`-like object. Labels, progress field, progress title, status list, and the remote method family are selected via `EntryKind`. Do not duplicate the 874-line file.

## 17. Observed defects (for reference, each has a task)
- D-1: Unknown or missing `data.host` leaves the activity on the spinner forever and never finishes it (TrackLoginActivity.kt:58-66 has no else branch; BaseOAuthLoginActivity.kt:31-39). Task T-27b-023.
- D-2: A denied or cancelled OAuth in the browser (callback without `code`/`access_token`) calls `logout()` for that tracker and wipes its saved token (TrackLoginActivity.kt:77-78, 90-91, 103-104, 116-117, 129-130). Task T-27b-024 (Proposal: keep the token and show "authorization cancelled").
- D-3: Login failures are swallowed (`catch (e: Throwable) { logout() }` in every login function). The user sees a settings screen with no error. Task T-27b-024.
- D-4: Logged-in tracker list in Home is a snapshot per emission, so logging in or out in another screen does not update an open dialog until the track rows change. Task T-27b-004.
- D-5: The refresh failure log in anime says `mangaId=` (A:248). Task T-27b-034.
- D-6: Search has no cancellation: a slow earlier request can overwrite a later query's result (A:734-754). Task T-27b-012.
- D-7: Progress uses `toInt()` truncation (A:374, 376-383). Fractional progress is lost on confirm. Task T-27b-037.
- D-8: `setRemoteScore` maps an unknown label to index -1 → score -1.0 (AnimeTracker.kt:196). Task T-27b-009.
- D-9: Score wheel initial index treats index 0 as "unset" (`takeIf { it > 0 }`, TrackInfoDialogSelector.kt:135). Task T-27b-035.
- D-10: Date picker bounds use local zone on a UTC picker value, so the day check is off by one for negative UTC offsets (A:468-470 and A:477-488). Task T-27b-036.
- D-11: Date OK uses `selectedDateMillis!!` (TrackInfoDialogSelector.kt:186). Task T-27b-010.
- D-12: AniList token expiry hard-coded to 365 days, ignoring the implicit grant's `expires_in` (AnilistApi.kt:515-517). Task T-27b-038.
- D-13: Remote delete failure after a successful local delete leaves the remote entry and only logs the error (A:860-868). Task T-27b-014 (Proposal: toast the failure).
- D-14: Remove button label is "OK" (A:842), while the date remover uses "Remove". Task T-27b-014.
- D-15: `!!` on `track`, `TrackerManager.get(serviceId)`, and `selectedDateMillis` can throw (A:116, 298, 353, 416, 528, 603, 694, 785; M same). Task T-27b-018 (Rust returns typed errors instead).
- D-16: Anime-specific dialogs use "reading" date titles (A:540-542). Keep for parity (Proposal: keep).
- D-17: `copyToClipboard` shows a toast only for API ≤ 32 (ContextExtensions.kt:48). Not applicable on Linux; the Rust toast always shows.

## 18. Interactions with other areas
- `area:27-tracking-core` part 1: `Tracker`, `AnimeTracker`, `EnhancedAnimeTracker`, `DeletableAnimeTracker`, `TrackerManager`, `RefreshAnimeTracks`, `AddAnimeTracks`, `TrackPreferences`, presentation composables.
- `area:28-tracker-anilist`, `area:29-tracker-myanimelist`, `area:30-tracker-kitsu`, `area:31-tracker-shikimori-bangumi`, `area:32-tracker-mangaupdates-simkl`, `area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin`: per-tracker `login`, `logout`, `refresh`, `delete`, `match`, score and status lists.
- `area:05-data-repos-entries-items` / `area:06-data-repos-history-track-source-misc` (track repository `delete(animeId, trackerId)`, `getTracksByAnimeIdAsFlow`).
- `area:02-preferences-system` (`app_date_format`, `UiPreferences`).
- `area:36-ui-shell-navigation-security-webview` (deep links; `MainActivity` relaunch).
- `area:47-settings-screens-general` (tracker rows that start login, `SettingsTrackingScreen`).
- `area:21-backup-format-and-create` and `area:22-backup-restore`: private preference keys (`__PRIVATE_` prefix) are not in backups (`PreferenceBackupCreator.kt:82`), so tokens and usernames are not backed up. Restoring a backup therefore requires logging in again.
- `area:01-foundation-app-bootstrap`: the Injekt replacement; Rust uses the app context (see rust design).
