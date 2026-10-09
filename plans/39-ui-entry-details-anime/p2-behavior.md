# 39-ui-entry-details-anime (part 2): behavior specification

All paths are repo-relative to `/home/blackbox/aniyomi-src`. Line numbers are `path:line`. Deep dives are in the topic files listed in section 0.

Topic files:
- `p2-topic-episode-list.md`: episode list pipeline, sort, filter, separators, selection, next-unseen.
- `p2-topic-seasons.md`: season list pipeline, season flags, season settings, season refresh.
- `p2-topic-track-sheet.md`: tracking sheet and all child screens, date and score rules, tracker removal, auto-track on mark-seen.
- `p2-topic-airing-and-related.md`: airing countdown (AniChartApi), viewer-flag codec, related anime sync.

Conventions used below:
- **OBSERVED** = what the Kotlin does. **Proposal:** = what Rust should do. Where OBSERVED behavior is a defect, it is labelled `DEFECT` and the Proposal says whether to keep or fix it.
- `ScreenModel` = `AnimeScreenModel` unless stated otherwise.
- `successState` = `state.value as? State.Success` (AnimeScreenModel.kt:156-157). Every mutator that reads it returns silently when the state is `Loading`.
- "`launchIO`" = `screenModelScope.launchIO` = `launch(Dispatchers.IO)` (core/common/src/main/java/tachiyomi/core/common/util/lang/CoroutinesExtensions.kt:49-50).
- "`launchNonCancellable`" = `launchIO { withContext(NonCancellable, block) }` (CoroutinesExtensions.kt:52-53). Survives screen leave.
- "`withUIContext`" = `withContext(Dispatchers.Main)` (CoroutinesExtensions.kt:55-58).

---

## 0. Map of this document

| Section | Content |
|---|---|
| 1 | External contracts (callees and their semantics) |
| 2 | Construction, init, first load |
| 3 | State model (fields, defaults, invariants) |
| 4 | Library membership: add, remove, duplicates, categories, auto-open tracking |
| 5 | Dialog state machine (every dialog, open trigger, dismiss behavior) |
| 6 | Source refresh (manual and automatic) and error messages |
| 7 | Episode list pipeline (summary; see topic file) |
| 8 | Episode sort, filters, separators (summary; see topic file) |
| 9 | Episode settings sheet actions and "set as default" |
| 10 | Selection model (summary; see topic file) |
| 11 | Episode actions: swipe, bookmark, fillermark, delete, mark seen, mark previous |
| 12 | Mark-seen to tracker prompt (AutoTrackState) |
| 13 | Download actions (per-episode and "next N") |
| 14 | Seasons (summary; see topic file) |
| 15 | Related anime |
| 16 | Tracker summary, airing countdown (summary; see topic file) |
| 17 | Continue watching and opening an episode |
| 18 | Search, genre search, share, WebView, copy URL |
| 19 | User-visible message keys |
| 20 | Side effects (DB, files, network, notifications, toasts) |
| 21 | Concurrency and dispatch map |
| 22 | Timings, limits, constants |
| 23 | Observed defects (catalogue) |

---

## 1. External contracts

Only the contracts this scope relies on are listed. Each contract cites the owning file.

### 1.1 Anime and episode storage

- `GetAnimeWithEpisodesAndSeasons` (domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetAnimeWithEpisodesAndSeasons.kt:11-36):
  - `subscribe(id)`: `combine(animeRepository.getAnimeByIdAsFlow(id), episodeRepository.getEpisodeByAnimeIdAsFlow(id), animeRepository.getAnimeSeasonsByIdAsFlow(id))` into `Triple(anime, episodes, seasons)`.
  - `awaitAnime`, `awaitEpisodes`, `awaitSeasons` are one-shot reads.
- `GetEpisodesByAnimeId` returns the episodes for an anime. The SQL is `SELECT * FROM episodes WHERE anime_id = :animeId;` with no ORDER BY (data/src/main/sqldelightanime/dataanime/episodes.sq:62-65). Ordering is applied in memory by the caller. **Proposal:** the Rust repository must not rely on any DB order.
- Season rows come from the view `animeseasonsView` filtered by `parent_id` (data/src/main/sqldelightanime/view/animeseasonsView.sq:41-44). The view computes `totalCount`, `seenCount`, `latestUpload`, `episodeFetchedAt`, `lastSeen`, `bookmarkCount`, `fillermarkCount` per season, using `fetch_type` (1 = Episodes, 0 = Seasons). Details in `p2-topic-seasons.md`.
- `AnimeRepository.updateAnime(AnimeUpdate)` returns Boolean (used everywhere for flag writes).
- `EpisodeRepository.updateAllEpisodes(list)` applies `EpisodeUpdate` rows; the `update:` SQL (episodes.sq:98) uses `coalesce(:field, field)` for every column, so a null field in `EpisodeUpdate` means "unchanged".

### 1.2 Episode seen state

- `SetSeenStatus.await(seen: Boolean, vararg episodes: Episode): Result` (app/src/main/java/eu/kanade/domain/items/episode/interactor/SetSeenStatus.kt:29-61).
  - Filter: when `seen == true`, keep episodes with `!it.seen`. When `seen == false`, keep episodes with `it.seen || it.lastSecondSeen > 0` (lines 30-35).
  - If the filtered list is empty, return `Result.NoEpisodes` (lines 36-38) and do nothing else.
  - Mapper (lines 21-27): `EpisodeUpdate(seen = read, lastSecondSeen = if (!read) 0 else null, id = episode.id)`. Marking unseen resets the resume position to 0. Marking seen keeps the existing `lastSecondSeen`.
  - Write: `episodeRepository.updateAllEpisodes(...)`; on exception log and return `Result.InternalError(e)` (lines 40-47).
  - Removal of downloads after read: if `seen && downloadPreferences.removeAfterMarkedAsRead().get()`, group the updated episodes by `animeId` and call `deleteDownload.awaitAll(anime, episodes)` (lines 49-58). Default of `removeAfterMarkedAsRead` is `false` (DownloadPreferences.kt:30).
  - Returns `Result.Success` (line 60).
- The ScreenModel ignores the returned `Result` (AnimeScreenModel.kt:872-875).

### 1.3 Other callees (contract summary)

| Callee | Location | Contract used here |
|---|---|---|
| `UpdateAnimeFromRemote.awaitEpisodesUpdate(source, anime, fetchDetails, fetchEpisodes, manualFetch)` | app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt:54-87 | Loads stored episodes sorted by `sourceOrder`, calls `source.getAnimeEpisodeUpdate`, updates the anime row (`awaitUpdateFromSource`), syncs episodes; returns `Result<RemoteAnimeEpisodeUpdate(anime, newEpisodes)>`. Errors are caught and returned as `Result.failure`. |
| `UpdateAnimeFromRemote.awaitSeasonsUpdate(source, anime, fetchDetails, fetchSeasons, manualFetch)` | same file, 107-140 | Same for seasons; returns `RemoteAnimeSeasonUpdate(anime, newSeasons)`. **`newSeasons` is ALL source seasons, not only new ones** (see 6.4). |
| `UpdateAnimeFromRemote.awaitUpdateFromSource` | same file, 142-214 | Title update only when `!localAnime.favorite` and remote title non-empty. Cover and background timestamps refreshed per the rules in 6.5. Writes `initialized = true`. |
| `SyncEpisodesWithSource.await(...)` | app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt:47-240 | Assigns `sourceOrder = index` to source episodes (line 67), returns the **new** episodes only (`updatedToAdd.filterNot { it.url in changedOrDuplicateReadUrls }`, line 240). Comment at line 194: sources must return episodes from most recent to least recent. |
| `SyncSeasonsWithSource.await(...)` | app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt:24-109 | Assigns `seasonSourceOrder = index` (line 41). Returns `sourceSeasons` (ALL seasons) on both return paths (lines 90 and 107). |
| `FilterEpisodesForDownload.await(anime, newEpisodes)` | domain/src/main/java/mihon/domain/items/episode/interactor/FilterEpisodesForDownload.kt:29-44 | Empty when `newEpisodes` empty, `downloadNewEpisodes` false, or anime not eligible. Eligibility (lines 52-67): must be favorite; categories default to `[0]` when none; category include/exclude rules. Optional "unseen only" filter by episode number. |
| `GetDuplicateLibraryAnime.await(anime)` | domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetDuplicateLibraryAnime.kt:10-12 | `animeRepository.getDuplicateLibraryAnime(anime.id, anime.title.lowercase())`. SQL: `SELECT * FROM animes WHERE favorite = 1 AND LOWER(title) = :title AND _id != :id;` (data/src/main/sqldelightanime/dataanime/animes.sq:128-133). |
| `SetAnimeEpisodeFlags` | domain/src/main/java/tachiyomi/domain/entries/anime/interactor/SetAnimeEpisodeFlags.kt | Writes `episodeFlags` via `setFlag`. Full bit layout in `p2-data-and-config.md`. |
| `SetAnimeSeasonFlags` | domain/src/main/java/tachiyomi/domain/entries/anime/interactor/SetAnimeSeasonFlags.kt | Writes `seasonFlags`. |
| `SetAnimeDefaultEpisodeFlags.await(anime)` / `awaitAll()` | domain/src/main/java/tachiyomi/domain/items/episode/interactor/SetAnimeDefaultEpisodeFlags.kt:15-37 | Copies all episode-setting preferences into one anime's `episodeFlags` (`awaitSetAllFlags`); `awaitAll` does it for every favorite. |
| `SetAnimeDefaultSeasonFlags.await(anime)` / `awaitAll()` | domain/src/main/java/tachiyomi/domain/items/season/interactor/SetAnimeDefaultSeasonFlags.kt:15-45 | Same for `seasonFlags`. |
| `SetAnimeViewerFlags` | app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt | Writes skip-intro and airing bits in `viewerFlags`. See `p2-topic-airing-and-related.md`. |
| `GetAnimeCategories.await()` / `await(animeId)` | domain/src/main/java/tachiyomi/domain/category/anime/interactor (not opened; used with the signatures shown in AnimeScreenModel.kt:424, 461, 527) | `await()` = all categories; `await(animeId)` = categories containing the anime. |
| `Category.isSystemCategory` | used at AnimeScreenModel.kt:517 | `getCategories()` returns user categories only (system category excluded). |
| `SetAnimeCategories.await(animeId, categoryIds)` | used at AnimeScreenModel.kt:552 | Replaces the anime's category set. |
| `UpdateAnime.awaitUpdateFavorite(id, Boolean)` | used at 401, 430, 437, 536 | Returns Boolean. |
| `UpdateAnime.awaitUpdateCoverLastModified(id)` | used at 404 | Called only when `removeCovers()` changed the anime. |
| `UpdateAnime.awaitUpdateFetchInterval(anime)` | app/src/main/java/eu/kanade/domain/entries/anime/interactor/UpdateAnime.kt | Computes `nextUpdate` and stores `fetchInterval`; see 6.8. |
| `AddAnimeTracks.bindEnhancedTrackers(anime, source)` | app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt:81-107 | For each logged-in `EnhancedAnimeTracker` that `accept(source)`, runs `match(anime)`; on match binds and syncs progress. Errors logged at WARN. |
| `AddAnimeTracks.bind(tracker, item, animeId)` | same file, 32-79 | Used by `AnimeTracker.register` (AnimeTracker.kt:58-65). Sets `lastEpisodeSeen` to the highest local seen episode number when newer; sets `startDate` from history if unset. |
| `TrackEpisode.await(context, animeId, episodeNumber, setupJobOnFailure = true)` | app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt:25-58 | For each track whose `episodeNumber > track.lastEpisodeSeen` and whose service is logged in: if online, refresh, update `lastEpisodeSeen`, push, insert; if offline, store in `DelayedAnimeTrackingStore` and schedule `DelayedAnimeTrackingUpdateJob`. Errors logged at INFO only. |
| `RefreshAnimeTracks.await(animeId)` | app/src/main/java/eu/kanade/domain/track/anime/interactor/RefreshAnimeTracks.kt:25-45 | Refreshes every logged-in track in parallel (`supervisorScope` + `async`). Returns the list of `(Tracker?, Throwable)` failures. |
| `SyncRelatedAnimeWithSource.await(anime, forceRefresh)` | app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncRelatedAnimeWithSource.kt:18-38 | See 15. |
| `TorrentServerService.start()` / `wait(timeout)` | app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt:146-188 | `start()`: start service intent then `wait(10)`; swallows exceptions. `wait(timeout)`: loops `api.echo() == ""` with `delay(1.seconds)` until `count > timeout`, returns false (no throw). |
| `TorrentServerUtils.setTrackersList()` | core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerUtils.kt:17 | Called after the torrent server start attempt, regardless of whether `wait` succeeded. |
| `AnimeSource.isSourceForTorrents()` | app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt:35-41 | False for null, local or stub sources. Otherwise `installedExtensionsFlow.value.find { ext -> ext.sources.any { it.id == this.id } }!!` then `.isTorrent`. The `!!` can throw (see 23.4). |
| `AniChartApi().loadAiringTime(anime, trackItems, manualFetch)` | app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt:22-49 | See `p2-topic-airing-and-related.md`. |
| `DownloadManager` API used | app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt | `queueState` (StateFlow<List<AnimeDownload>>, line 62); `statusFlow()` (437) and `progressFlow()` (452); `getQueuedDownloadOrNull(episodeId)` (106-108); `startDownloadNow(episodeId)` (110-126); `downloadEpisodes(anime, episodes, autoStart=true, alt=false, video=null)` (139-153); `startDownloads()` (75-86); `cancelQueuedDownloads(downloads)` (258-260); `deleteEpisodes(episodes, anime, source)` (269-297); `deleteAnime(anime, source, removeQueued=true)` (299); `isEpisodeDownloaded(name, scanlator, animeTitle, sourceId, skipCache=false)` (200-217); `getDownloadCount(anime)` (228-238). |
| `AnimeDownloadCache.isEpisodeDownloaded(...)` | app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadCache.kt:135-162 | When `skipCache` is false, calls `renewCache()` then looks up `sourceDirs[sourceId].animeDirs[provider.getAnimeDirName(title)].episodeDirs` for any name from `provider.getValidEpisodeDirNames(name, scanlator)`. |
| `EnhancedAnimeTracker` | app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt:12-41 | `accept(source)` = `source::class.qualifiedName in getAcceptedSources()`; `match(anime)` returns zero or one match. |
| `DeletableAnimeTracker.delete(track)` | app/src/main/java/eu/kanade/tachiyomi/data/track/DeletableAnimeTracker.kt:49-51 | Remote delete. Only some trackers implement it. |
| `AnimeTracker` | app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt:24-123 | Remote setters: `setRemoteAnimeStatus`, `setRemoteLastEpisodeSeen`, `setRemoteScore`, `setRemoteStartDate`, `setRemoteFinishDate`, `setRemotePrivate`. Each mutates the given track object and calls `updateRemote`, which calls `update(track)` then `insertTrack`. Errors become a toast with `e.message`. Details in `p2-topic-track-sheet.md`. |
| `TrackerManager` | app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:17-56 | `loggedInTrackers()`, `loggedInTrackersFlow()` (combine of each `isLoggedInFlow`), `get(id)`. Trackers in order: MyAnimeList(1), AniList(2), Kitsu(3), Shikimori(4), Bangumi(5), Komga(6), MangaUpdates(7), Kavita(8), Suwayomi(9), Simkl(101), Jellyfin(102). |
| `Tracker` (wrapper) | app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt:10-48 | Exposes `name`, `id`, `supportsPrivateTracking`, `isLoggedIn`, `isLoggedInFlow`, `animeService: AnimeTracker`. |
| `AnimeSourceManager.getOrStub(id)` | domain/src/main/java/tachiyomi/domain/source/anime/service/AnimeSourceManager.kt:17 | Returns the source or a `StubAnimeSource` (`isLocalOrStub()` true). |
| `AnimeSource.supportsRelatedAnime` | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt:121 | Default `false`. `StubAnimeSource` and `LocalAnimeSource` return `false`. |
| `FetchType` | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt:10-19 | `enum class FetchType { Seasons, Episodes }` (declaration order = Seasons first). DB stores `fetch_type` INTEGER with default 1 (= Episodes). |
| `SAnime` status constants | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-74 | `UNKNOWN = 0`, `ONGOING = 1`, `COMPLETED = 2`. |
| `UnmeteredSource` | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/UnmeteredSource.kt:8 | Marker interface. Changes season episode-fetch concurrency (6.4). |
| `AnimeFetchInterval` | domain/src/main/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchInterval.kt | `MAX_INTERVAL = 28` (line 126). Negative `fetchInterval` = custom days. Owned by area 07-domain-entries-items. |
| `LibraryPreferences`, `DownloadPreferences`, `TrackPreferences`, `PlayerPreferences`, `GesturePreferences`, `SourcePreferences`, `TorrentPreferences`, `BasePreferences`, `UiPreferences` | see `p2-data-and-config.md` section 4 | Keys and defaults. |

---

## 2. Construction, init, first load

### 2.1 Construction

- Constructed by `AnimeScreen.Content()` through `rememberScreenModel { AnimeScreenModel(context, lifecycleOwner.lifecycle, animeId, fromSource) }` (AnimeScreen.kt:101-102). All other constructor parameters take their default `Injekt.get()` value.
- `snackbarHostState: SnackbarHostState = SnackbarHostState()` is a constructor default (AnimeScreenModel.kt:153).
- The model starts in `State.Loading` (AnimeScreenModel.kt:154). `AnimeScreen.Content()` shows `LoadingScreen()` while Loading (AnimeScreen.kt:106-109).
- Before any of this, `AnimeScreen.Content()` returns `LoadingScreen()` when `ifAnimeSourcesLoaded()` is false (AnimeScreen.kt:91-94).

### 2.2 Per-entry value snapshots taken at construction

These are captured once, when the model is created, and never refreshed while the screen is open (AnimeScreenModel.kt:171-182):

| Field | Source | Default | Note |
|---|---|---|---|
| `episodeSwipeStartAction` | `libraryPreferences.swipeEpisodeEndAction().get()` | `ToggleBookmark` | Name is reversed relative to the key (see 11.1 and 23.9). |
| `episodeSwipeEndAction` | `libraryPreferences.swipeEpisodeStartAction().get()` | `ToggleSeen` | Same. |
| `autoTrackState` (var) | `trackPreferences.autoUpdateTrackOnMarkRead().get()` | `ALWAYS` | Read by `markEpisodesSeen`. |
| `showNextEpisodeAirTime` | `trackPreferences.showNextEpisodeAiringTime().get()` | `true` | |
| `alwaysUseExternalPlayer` | `playerPreferences.alwaysUseExternalPlayer().get()` | `false` | |
| `useExternalDownloader` | `downloadPreferences.useExternalDownloader().get()` | `false` | Passed to the quality dialog. |
| `relatedAnimeDisplayMode` | `sourcePreferences.sourceDisplayMode().get()` | `LibraryDisplayMode.default` | Presentation only. |
| `isUpdateIntervalEnabled` | `LibraryPreferences.ENTRY_OUTSIDE_RELEASE_PERIOD in libraryPreferences.autoUpdateItemRestrictions().get()` | `true` (default set contains `manga_outside_release_period`) | Gates the custom interval wheel (6.8). |

Note that the constants `ENTRY_*` in `LibraryPreferences` are shared between manga and anime (value strings start with `manga_`). This is intentional in the source.

### 2.3 `init` block (AnimeScreenModel.kt:204-283)

Executed once per model instance. In order:

1. **Live subscription (launchIO, lines 205-221).** `combine(getAnimeAndEpisodesAndSeasons.subscribe(animeId).distinctUntilChanged(), downloadCache.changes, downloadManager.queueState) { (anime, episodes, seasons), _, _ -> ... }`. Only the first flow's value is used. The other two are triggers. `flowWithLifecycle(lifecycle)` stops the collection when the lifecycle is below STARTED. `collectLatest` maps each emission to `updateSuccessState { copy(anime, episodes = episodes.toEpisodeListItems(anime), seasons = seasons.toAnimeSeasonItems()) }`.
   - `updateSuccessState` does nothing while the state is still `Loading`, so emissions before step 3 are dropped. Step 3 reads fresh data anyway.
2. **Download observers (line 223)**: `observeDownloads()` (569-593), two collectors (status and progress).
3. **First load (launchIO, lines 225-282)**:
   1. `anime = getAnimeAndEpisodesAndSeasons.awaitAnime(animeId)`.
   2. `source = sourceManager.getOrStub(anime.source)`.
   3. `episodes` = `[]` when `anime.fetchType == FetchType.Seasons`, else `awaitEpisodes(animeId).toEpisodeListItems(anime)` (lines 229-234).
   4. `seasons` = `[]` when `anime.fetchType == FetchType.Episodes`, else `awaitSeasons(animeId).toAnimeSeasonItems()` (lines 236-241).
   5. **If `!anime.favorite`**: `setAnimeDefaultEpisodeFlags.await(anime)` and `setAnimeDefaultSeasonFlags.await(anime)` (lines 243-246). **OBSERVED: this overwrites the per-entry filter, sort and display flags of every non-library entry on every open**, with the current library defaults. See 23.1.
   6. Compute flags (lines 248-250):
      - `needRefreshInfo = !anime.initialized`
      - `needRefreshEpisode = episodes.isEmpty() && anime.fetchType == FetchType.Episodes`
      - `needRefreshSeason = seasons.isEmpty() && anime.fetchType == FetchType.Seasons`
   7. Publish the first Success state (lines 253-263) with `isRefreshingData = needRefreshInfo || needRefreshEpisode || needRefreshSeason`, `dialog = null`, `isFromSource` set from the constructor.
   8. `observeTrackers()` (265): starts two tracker collectors (section 16).
   9. `observeRelatedAnime()` (267) and `syncRelatedAnime()` (268) with default `forceRefresh = false`.
   10. If any `needRefresh*` is true and `screenModelScope.isActive`: `fetchAllFromSource(manualFetch = false, fetchDetails = needRefreshInfo, fetchEpisodes = needRefreshEpisode, fetchSeasons = needRefreshSeason)` (lines 271-278). This call is awaited inside the same launchIO block.
   11. `updateSuccessState { it.copy(isRefreshingData = false) }` (line 281).

**Proposal:** keep the order. Steps 3.5 (defaults for non-library) is the one that must be reconsidered; see 23.1 and the Rust design in `p2-rust-design.md` section 6.

### 2.4 Empty-list refresh rule

On every open of an entry whose episode list (or season list, by fetch type) is empty, a full source refresh runs automatically (`needRefreshEpisode` / `needRefreshSeason`, lines 249-250). An entry that legitimately has zero episodes is therefore re-fetched on every open. **Proposal:** keep the rule but add a `last_fetched_at` guard of 10 minutes to stop repeated fetches (see task T-39b-021 acceptance).

---

## 3. State model

### 3.1 `State` (AnimeScreenModel.kt:1602-1739)

- `State.Loading` (data object, `@Immutable`).
- `State.Success` (data class, `@Immutable`) fields in declaration order with defaults:

| Field | Type | Default |
|---|---|---|
| `anime` | `Anime` | (required) |
| `source` | `AnimeSource` | (required) |
| `isFromSource` | Boolean | (required) |
| `episodes` | `List<EpisodeList.Item>` | (required) |
| `seasons` | `List<AnimeSeasonItem>` | (required) |
| `trackingCount` | Int | 0 |
| `hasLoggedInTrackers` | Boolean | false |
| `relatedAnime` | `List<AnimeRelationGroup>` | empty |
| `isLoadingRelatedAnime` | Boolean | false |
| `isRefreshingData` | Boolean | false |
| `dialog` | `Dialog?` | null |
| `hasPromptedToAddBefore` | Boolean | false |
| `trackItems` | `List<AnimeTrackItem>` | empty. **Never written by any code path in this scope (23.3).** |
| `nextAiringEpisode` | `Pair<Int, Long>` | `Pair(anime.nextEpisodeToAir, anime.nextEpisodeAiringAt)` |

Derived members (all `by lazy` except the getters):

- `processedSeasons` = `seasons.applySeasonFilters(anime).toList()` (1627-1629).
- `processedEpisodes` = `episodes.applyFilters(anime).toList()` (1631-1633). The member extension (1682-1699) is the one used, not the top-level one in `EpisodeFilter.kt`.
- `episodeListItems` = `processedEpisodes.insertSeparators { before, after -> ... }` (1635-1660). Separator rules in `p2-topic-episode-list.md` section 3.
- `trackingAvailable` = `trackItems.isNotEmpty()` (1662-1663). Always false (23.3).
- `airingEpisodeNumber` = `nextAiringEpisode.first.toDouble()` (1665-1666).
- `airingTime` = `nextAiringEpisode.second * 1000L - Calendar.getInstance().timeInMillis` (1668-1671). Not clamped; can be negative.
- `showPreviews` = `anime.showPreviews()` (1672-1673). `showSummaries` = `anime.showSummaries()` (1675-1676).

### 3.2 `EpisodeList` (AnimeScreenModel.kt:1742-1760)

```
@Immutable sealed class EpisodeList
  data class MissingCount(val id: String, val count: Int)
  data class Item(val episode: Episode, val downloadState: AnimeDownload.State, val downloadProgress: Int, val selected: Boolean = false)
      val id = episode.id           // body property, not constructor
      val isDownloaded = downloadState == AnimeDownload.State.DOWNLOADED
```

- `MissingCount.id` = `"${lowerEpisode?.id}-${higherEpisode.id}"` (1655). Note that `lowerEpisode` can be null, so the id may begin with `null-`.
- `Item.id` is the episode id. Selection is keyed by it.

### 3.3 Invariants the Rust model must keep

1. `episodes` holds one `Item` per stored episode of the anime (before filters). The UI list is derived, never stored.
2. `selected` on an `Item` is always equal to `episode.id in selectedEpisodeIds` at the time the list was built (line 637). Toggling mutates both.
3. `isRefreshingData` is true from the start of a refresh until the refresh coroutine finishes (see 6.1).
4. `dialog` holds at most one dialog. Opening a dialog replaces the current one (no stack). The only exception is the track-after-category chaining in 5.4.

**DEFECT (see 23.2):** `toggleSelection` writes `successState.processedEpisodes` (filtered) back into `episodes` (lines 1376, 1429). After one toggle the `episodes` list is the filtered list, not the full list. Full-list state is restored only when the DB flow next emits.

---

## 4. Library membership

### 4.1 Entry points

- `AnimeScreen`: `onAddToLibraryClicked = { screenModel.toggleFavorite(); haptic.performHapticFeedback(LongPress) }` (AnimeScreen.kt:148-151). Haptics are dropped (task 067).
- `DuplicateAnimeDialog` confirm: `toggleFavorite(onRemoved = {}, checkDuplicate = false)` (AnimeScreen.kt:265).
- `startDownload` snackbar action: `toggleFavorite()` (AnimeScreenModel.kt:799-801).

### 4.2 `toggleFavorite()` (no arguments, lines 370-386)

- Calls `toggleFavorite(onRemoved = { ... })` where `onRemoved` runs in `screenModelScope.launch` and does:
  1. `if (!hasDownloads()) return`.
  2. `snackbarHostState.showSnackbar(message = delete_downloads_for_anime, actionLabel = action_delete, withDismissAction = true)`.
  3. If the result is `SnackbarResult.ActionPerformed` then `deleteDownloads()`.
- `hasDownloads()` = `downloadManager.getDownloadCount(anime) > 0` (498-501). For local anime this counts archive files in the anime directory (AnimeDownloadManager.kt:228-238).
- `deleteDownloads()` = `downloadManager.deleteAnime(state.anime, state.source)` (506-509). `removeQueued` defaults to true.

### 4.3 `toggleFavorite(onRemoved, checkDuplicate = true)` (lines 391-456)

Runs in `screenModelScope.launchIO`. `anime = state.anime` (captured at call).

**Remove branch** (`isFavorited` is true, where `isFavorited = anime?.favorite ?: false` reads the current state):

1. `updateAnime.awaitUpdateFavorite(anime.id, false)`. If false, do nothing.
2. `if (anime.removeCovers() != anime) updateAnime.awaitUpdateCoverLastModified(anime.id)` (lines 403-405). `removeCovers()` deletes the cached cover file (area 34); when the returned anime differs, the cover timestamp is bumped.
3. `withUIContext { onRemoved() }`.

**Add branch** (`isFavorited` false):

1. If `checkDuplicate`:
   - `duplicate = getDuplicateLibraryAnime.await(anime).getOrNull(0)`.
   - If non-null: `updateSuccessState { it.copy(dialog = Dialog.DuplicateAnime(anime, duplicate)) }`, then `return@launchIO`. Nothing else happens.
2. `categories = getCategories()` (user categories, system excluded).
3. `defaultCategoryId = libraryPreferences.defaultAnimeCategory().get().toLong()` (default `-1`).
4. `defaultCategory = categories.find { it.id == defaultCategoryId }`.
5. `when`:
   - **`defaultCategory != null`**: `awaitUpdateFavorite(anime.id, true)`; if false return; `moveAnimeToCategory(defaultCategory)` (that is, `setAnimeCategories.await(animeId, listOf(defaultCategory.id))`).
   - **`defaultCategoryId == 0L || categories.isEmpty()`**: `awaitUpdateFavorite(true)`; if false return; `moveAnimeToCategory(null)` which calls `setAnimeCategories.await(animeId, emptyList())`.
   - **else** (choose a category): `isFromChangeCategory = true`; `showChangeCategoryDialog()`. The favorite flag is NOT set here. It is set later by `moveAnimeToCategoriesAndAddToLibrary` (4.5).
6. `addTracks.bindEnhancedTrackers(anime, state.source)` (line 450). Runs in every add branch that reaches this line, including the "choose a category" branch, before the user has confirmed a category.
7. `if (autoOpenTrack) showTrackDialog()` (lines 451-453). See 4.6.

`toggleFavorite` with `checkDuplicate = false` (from the duplicate dialog) skips step 1 and continues at step 2. The `Dialog.DuplicateAnime` is dismissed by the host before the call (AnimeScreen.kt:262-266 does not dismiss explicitly; the call `toggleFavorite(onRemoved = {}, checkDuplicate = false)` replaces `dialog` on the next state write, and `updateSuccessState { it.copy(dialog = Dialog.DuplicateAnime) }` is not re-executed). **OBSERVED:** the duplicate dialog stays visible until the user dismisses it or a later state write clears it. See 23.6.

### 4.4 Category selection dialog content

`showChangeCategoryDialog()` (458-472):

- `anime = successState.anime`; launches `screenModelScope.launch`:
  - `categories = getCategories()` (user categories).
  - `selection = getAnimeCategoryIds(anime)` = `getCategories.await(anime.id).map { it.id }`.
  - `dialog = Dialog.ChangeCategory(anime, initialSelection = categories.mapAsCheckboxState { it.id in selection }.toImmutableList())`.
- The dialog is `ChangeCategoryDialog(initialSelection, onDismissRequest, onEditCategories = { navigator.push(CategoriesTab) }, onConfirm = { include, _ -> screenModel.moveAnimeToCategoriesAndAddToLibrary(dialog.anime, include) })` (AnimeScreen.kt:241-250).

### 4.5 `moveAnimeToCategoriesAndAddToLibrary(anime, categories)` (531-538)

1. `moveAnimeToCategory(categories)`: `screenModelScope.launchIO { setAnimeCategories.await(animeId, categories) }`. Not awaited by the caller.
2. `if (anime.favorite) return`. Uses the `anime` captured in the dialog, so the check can be stale.
3. Otherwise `screenModelScope.launchIO { updateAnime.awaitUpdateFavorite(anime.id, true) }`.

Note: the dialog's `onConfirm` is only wired for `Dialog.ChangeCategory`. Dismissing without confirming leaves the anime not in the library, with no category change.

### 4.6 Track-after-add (auto-open)

- `autoOpenTrack` (internal val, lines 189-190) = `successState?.hasLoggedInTrackers == true && trackPreferences.trackOnAddingToLibrary().get()`.
- `trackOnAddingToLibrary` default is `true` (TrackPreferences.kt:41).
- In the default-category and no-category branches, `showTrackDialog()` runs at step 7 of 4.3 and opens the tracking sheet right away.
- In the choose-category branch, the sheet is requested at step 7 but then replaced when the category dialog's state write lands (5.4). The sheet re-opens when the category dialog is dismissed (AnimeScreen.kt:232-238, see 5.3).

### 4.7 `setFetchInterval(anime, interval)` (481-493)

Covered in 6.8.

---

## 5. Dialog state machine

### 5.1 The union (AnimeScreenModel.kt:1547-1562)

| Variant | Fields | Opened by | Host (AnimeScreen.kt) |
|---|---|---|---|
| `ChangeCategory` | `anime`, `initialSelection: ImmutableList<CheckboxState<Category>>` | `showChangeCategoryDialog()` (from 4.3 choose-branch, and `onEditCategoryClicked` at AnimeScreen.kt:194) | `ChangeCategoryDialog` (239-250) |
| `DeleteEpisodes` | `episodes: List<Episode>` | `showDeleteEpisodeDialog(episodes)` (from `onMultiDeleteClicked`) | `DeleteItemsDialog(isManga = false)` (251-260). Confirm: `toggleAllSelection(false)`, then `deleteEpisodes(dialog.episodes)`. |
| `DuplicateAnime` | `anime`, `duplicate: Anime` | `toggleFavorite` add path (4.3 step 1) | `DuplicateAnimeDialog` (262-271). Confirm: `toggleFavorite(onRemoved={}, checkDuplicate=false)`. Open anime: navigate to `AnimeScreen(duplicate.id)`. Migrate: `showMigrateDialog(duplicate)`. |
| `Migrate` | `newAnime`, `oldAnime` | `showMigrateDialog(duplicate)` (1589-1592) | `MigrateAnimeDialog` (273-283). Click title: push `AnimeScreen(oldAnime.id)`. Seasons: push `MigrateSeasonSelectScreen`. Pop: `navigator.replace(AnimeScreen(newAnime.id))`. |
| `SetAnimeFetchInterval` | `anime` | `showSetAnimeFetchIntervalDialog()` (474-479) | `SetIntervalDialog` (358-367) |
| `ShowQualities` | `episode`, `anime`, `source` | `showQualitiesDialog(episode)` (1598-1600), from download action `SHOW_QUALITIES` | `NavigatorAdaptiveSheet(EpisodeOptionsDialogScreen(...))` (389-409) |
| `ChangeAnimeSkipIntro` | none | `showAnimeSkipIntroDialog()` (1594-1596) | `SkipIntroLengthDialog` (368-388) |
| `EpisodeSettingsSheet` | none | `showSettingsDialog()` when `fetchType == Episodes` (1572-1579) | `EpisodeSettingsDialog` (284-296) |
| `SeasonSettingsSheet` | none | `showSettingsDialog()` when `fetchType == Seasons` | `SeasonSettingsDialog` (297-316) |
| `TrackSheet` | none | `showTrackDialog()` (1581-1583) | `NavigatorAdaptiveSheet(AnimeTrackInfoDialogHomeScreen(...), enableSwipeDismiss = { it.lastItem is AnimeTrackInfoDialogHomeScreen })` (317-327) |
| `FullImages` | none | `showImagesDialog()` (1585-1587), from `onCoverClicked` | `AnimeImagesDialog` with `AnimeImageScreenModel(animeId)` (328-357). `LoadingScreen` while `sm.state` is null. |

Other `dialog` writers that do not use a named Dialog variant:
- `updateSuccessState { it.copy(dialog = null) }` in `dismissDialog()` (1564-1566).

### 5.2 `showSettingsDialog()` branch

`when (it.anime.fetchType) { FetchType.Seasons -> SeasonSettingsSheet; FetchType.Episodes -> EpisodeSettingsSheet }` (1573-1578). Exhaustive over the two `FetchType` values.

### 5.3 Dismiss behavior (`onDismissRequest`, AnimeScreen.kt:232-238)

```
onDismissRequest = {
    screenModel.dismissDialog()
    if (screenModel.autoOpenTrack && screenModel.isFromChangeCategory) {
        screenModel.isFromChangeCategory = false
        screenModel.showTrackDialog()
    }
}
```

So every dialog's dismiss is `dismissDialog()`, and in addition, if the flag `isFromChangeCategory` was set and auto-open is on, the flag is cleared and the tracking sheet opens. The flag is set only in the choose-category branch (4.3 step 5). `isFromChangeCategory` is not cleared when the category dialog is confirmed (the confirm path does not go through `onDismissRequest` in the `ChangeCategoryDialog` call at AnimeScreen.kt:242-249 unless the presentation composable also calls `onDismissRequest` after `onConfirm`; this is a presentation detail owned by part 1). **Proposal:** clear the flag on confirm and on dismiss both.

### 5.4 Track sheet after category (race)

In the choose-category branch, the sequence on the launchIO thread is:
1. `isFromChangeCategory = true`.
2. `showChangeCategoryDialog()` calls `screenModelScope.launch { ... updateSuccessState { dialog = ChangeCategory } }`. This is asynchronous.
3. `addTracks.bindEnhancedTrackers(...)` (blocking within the coroutine).
4. `if (autoOpenTrack) showTrackDialog()` (synchronous `updateSuccessState { dialog = TrackSheet }`).

Step 2's write happens after `getCategories()` returns, so it normally lands after step 4. The visible result is the category dialog. The track sheet opens after the category dialog is dismissed (5.3). **DEFECT (23.5):** the ordering is incidental, not enforced.

### 5.5 Interaction with the Success state update helper

`updateSuccessState(func)` (195-202): `mutableState.update { when (it) { Loading -> it; is Success -> func(it) } }`. It is `inline`, so `func` may not capture a suspend point. Several callers rely on `it.copy(dialog = ...)` being the last write; there is no queue.

---

## 6. Source refresh

### 6.1 Entry points and flags

- Public `fetchAllFromSource(manualFetch: Boolean = true)` (285-297):
  1. `screenModelScope.launch { ... }` (Voyager main-thread scope).
  2. `updateSuccessState { copy(isRefreshingData = true) }`.
  3. `fetchAllFromSource(manualFetch, fetchDetails = true, fetchEpisodes = true, fetchSeasons = true)`.
  4. `updateSuccessState { copy(isRefreshingData = false) }`.
  5. `successState?.let { updateAiringTime(it.anime, it.trackItems, manualFetch) }` (line 295). **This passes the always-empty `trackItems` (23.3).**
- Private suspend `fetchAllFromSource(manualFetch, fetchDetails, fetchEpisodes, fetchSeasons)` (299-366):
  1. `state = successState ?: return`.
  2. `startTorrentServer(state.source)` (307). This happens outside the try block (see 23.4).
  3. `if (fetchDetails) syncRelatedAnime(forceRefresh = true)` (309-311). Fire-and-forget.
  4. `try { withUIContext { when (state.anime.fetchType) { ... } } }`:
     - `FetchType.Episodes`:
       - `update = updateAnimeFromRemote.awaitEpisodesUpdate(source, anime, fetchDetails, fetchEpisodes, manualFetch).getOrThrow()`.
       - If `manualFetch`: `downloadNewEpisodes(update.newEpisodes)` (326-328). Not run on the automatic open-time refresh.
     - `FetchType.Seasons`:
       - `update = updateAnimeFromRemote.awaitSeasonsUpdate(source, anime, fetchDetails, fetchSeasons, manualFetch).getOrThrow()`.
       - If `libraryPreferences.updateSeasonOnRefresh().get()` (default `false`): `fetchEpisodesFromSeasons(update.newSeasons, manualFetch)` (340-342).
  5. Catch:
     - `CancellationException`: ignored (line 346-347).
     - `Exception`: map to a message (6.3), then `screenModelScope.launch { snackbarHostState.showSnackbar(message) }`.

Note: `withUIContext` at line 314 wraps the whole network call. The network work itself runs inside `UpdateAnimeFromRemote` via `withIOContext`. The main thread is therefore held only for the coroutine bookkeeping.

### 6.2 Episode refresh path (what changes)

`UpdateAnimeFromRemote.awaitEpisodesUpdate(source, anime, fetchDetails, fetchEpisodes, manualFetch, fetchWindow = (0, 0))`:
- `episodes = episodeRepository.getEpisodeByAnimeId(anime.id).sortedBy { it.sourceOrder }`.
- `update = source.getAnimeEpisodeUpdate(anime.toSAnime(), episodes.map(toSEpisode), fetchDetails, fetchEpisodes)` on `Dispatchers.IO`.
- `awaitUpdateFromSource(anime, update.anime, manualFetch)` (6.5).
- `newEpisodes = syncEpisodesWithSource.await(rawSourceEpisodes = update.episodes, anime, source, manualFetch, fetchWindow)`.
- `updatedAnime = animeRepository.getAnimeById(anime.id)`; return `RemoteAnimeEpisodeUpdate(updatedAnime, newEpisodes)`.
- Errors: logged at ERROR, `Result.failure(e)`.

`SyncEpisodesWithSource` returns only the newly inserted episodes (SyncEpisodesWithSource.kt:240). Auto-download on manual refresh applies to exactly that list.

### 6.3 Refresh error messages

| Exception | Message | Source |
|---|---|---|
| `NoEpisodesException` | `AYMR.strings.no_episodes_error` = "No episodes found" | AnimeScreenModel.kt:350-352 |
| `NoSeasonsException` | `AYMR.strings.no_seasons_error` = "No seasons found" | 353-355 |
| anything else | `logcat(ERROR, e)`, then `with(context) { e.formattedMessage }` | 356-359 |

The message is shown with `snackbarHostState.showSnackbar(message = message)` (363). No action, no dismiss button, default duration.

Where `NoEpisodesException` / `NoSeasonsException` are thrown is owned by the sync interactors (area 07). The scope only maps them.

### 6.4 Season refresh path (what changes)

`UpdateAnimeFromRemote.awaitSeasonsUpdate` (SeasonsUpdate lines 107-140):
- `seasons = animeRepository.getAnimeSeasonsById(anime.id).sortedBy { it.anime.seasonSourceOrder }`.
- `update = source.getAnimeSeasonUpdate(anime.toSAnime(), seasons.map { it.anime.toSAnime() }, fetchDetails, fetchSeasons)`.
- `awaitUpdateFromSource(anime, update.anime, manualFetch)`.
- `newSeasons = syncSeasonsWithSource.await(rawSourceSeasons = update.seasons, anime, source, manualFetch, fetchWindow)`.

`SyncSeasonsWithSource.await` returns **all** source seasons (lines 90 and 107), so `RemoteAnimeSeasonUpdate.newSeasons` = all seasons. `fetchEpisodesFromSeasons(seasons, manualFetch)` therefore runs over every season, subject to the filter in 6.4.1. The field name is misleading and the Rust port should name it `seasons`.

#### 6.4.1 `fetchEpisodesFromSeasons(seasons, manualFetch)` (AnimeScreenModel.kt:670-702)

- `state = successState ?: return`.
- Per season `s` the fetch runs only when `s.fetchType === FetchType.Episodes && (s.lastUpdate == 0L || s.status.toInt() != SAnime.COMPLETED)` (line 676). `===` is identity equality on the enum; both sides are the same enum object so this is equivalent to `==`.
- Each fetch: `updateAnimeFromRemote.awaitEpisodesUpdate(source = state.source, anime = s, fetchEpisodes = true, manualFetch = manualFetch)` (line 678-683). `fetchDetails` defaults to false. The result is discarded. **New episodes of a season are never auto-downloaded** (23.7).
- Errors per season: `catch (e: Throwable) { logcat(ERROR, e) }`. One failing season does not stop the others.
- Concurrency:
  - If `state.source is UnmeteredSource`: `seasons.map { async(Dispatchers.IO) { fetch(s) } }.awaitAll()` (690-695). All seasons in parallel.
  - Otherwise: `seasons.forEach { ensureActive(); fetch(s) }` (696-701). Sequential, cancelable between seasons.

### 6.5 Cover and background refresh rules (`awaitUpdateFromSource`, UpdateAnimeFromRemote.kt:142-214)

`coverLastModified` (lines 161-175):
1. `remoteAnime.thumbnail_url` null or empty: `null` (keep existing cover).
2. `!manualFetch && localAnime.thumbnailUrl == remoteAnime.thumbnail_url`: `null`.
3. `localAnime.isLocal()`: `Instant.now().toEpochMilli()`.
4. `localAnime.hasCustomCover(coverCache)`: delete cached cover (`deleteFromCache(localAnime, false)`), result `null` (the custom cover is kept).
5. else: delete cached cover, result `now`.

`backgroundLastModified` (177-191): identical structure with `background_url` and `hasCustomBackground(backgroundCache)`.

Title (147-159): remote title used only when the local anime is not a favorite and the remote title is non-empty.

### 6.6 Initial open refresh condition

`needRefreshInfo = !anime.initialized` (line 248). `initialized` is set to `true` by `awaitUpdateFromSource` (UpdateAnimeFromRemote.kt:210). So details are fetched once; episodes are fetched again only when the stored list is empty (2.4).

### 6.7 Refresh button

`onRefresh = screenModel::fetchAllFromSource` (AnimeScreen.kt:175). This is the public overload with `manualFetch = true` (default). Effect: all four fetch flags true, `downloadNewEpisodes` runs, cover/background are re-fetched even when the URL is unchanged (rule 2 of 6.5 is skipped).

### 6.8 Custom update interval (`showSetAnimeFetchIntervalDialog`, `setFetchInterval`)

- `showSetAnimeFetchIntervalDialog()` (474-479): opens `SetAnimeFetchInterval(anime)`.
- Host (AnimeScreen.kt:358-367): `SetIntervalDialog(interval = dialog.anime.fetchInterval, nextUpdate = dialog.anime.expectedNextUpdate, isManga = false, onValueChanged = { interval -> screenModel.setFetchInterval(dialog.anime, interval) }.takeIf { screenModel.isUpdateIntervalEnabled })`.
- `SetIntervalDialog` (app/src/main/java/eu/kanade/presentation/entries/components/ItemsDialogs.kt:70-170):
  - `selectedInterval` starts at `if (interval < 0) -interval else 0` (line 77).
  - Subtitle shows the expected next update in days when `nextUpdate != null && nextUpdateDays >= 0 && interval >= 0` (lines 93-110). Else the "null" text variant.
  - The custom wheel is shown only when `onValueChanged != null && !isReleaseBuildType` (line 126). `isReleaseBuildType` = `BuildConfig.BUILD_TYPE == "release"` (app/src/main/java/eu/kanade/tachiyomi/util/system/BuildConfig.kt:15-16). **In release builds the wheel is hidden and the OK button calls `onValueChanged?.invoke(selectedInterval)` with a null receiver, so the interval cannot change (23.8).**
  - Wheel items: `0..AnimeFetchInterval.MAX_INTERVAL` (28); index 0 shows `MR.strings.label_default` (line 139-148).
  - OK: `onValueChanged?.invoke(selectedInterval); onDismissRequest()` (lines 162-168).
- `setFetchInterval(anime, interval)` (481-493):
  - Launch IO. `updateAnime.awaitUpdateFetchInterval(anime.copy(fetchInterval = -interval))`.
  - Index 0 (Default) gives `-0 = 0` which means smart mode; index N (1..28) gives `-N` which means custom every N days.
  - On success: `animeRepository.getAnimeById(anime.id)` and `updateSuccessState { copy(anime = updatedAnime) }`.
- `AnimeFetchInterval.toAnimeUpdate` (AnimeFetchInterval.kt:40-54): `interval = fetchInterval.takeIf { it < 0 } ?: calculateInterval(episodes, zone)`; `nextUpdate = calculateNextUpdate(anime, interval, dateTime, window)`. Smart mode computes its interval from upload and fetch dates (lines 75-118) and clamps to `1..28`. Default for smart mode when there is not enough data: 7 days.

### 6.9 Torrent server before refresh

`startTorrentServer(source)` (659-665): if `isTorrentEnabled() && source.isSourceForTorrents()` then `TorrentServerService.start()`, `TorrentServerService.wait(10)`, `torrentServerUtils.setTrackersList()`. `isTorrentEnabled()` = `torrentPreferences.torrServerEnable().get()` (default `false`, TorrentPreferences.kt:9).

---

## 7. Episode list pipeline (summary)

Full detail in `p2-topic-episode-list.md`. The order is:

1. DB row (`Episode`) -> `toEpisodeListItems(anime)` (609-640): one `EpisodeList.Item` per episode; `downloadState` from the queue or the cache (section 13); `selected = episode.id in selectedEpisodeIds`.
2. `Success.episodes` (stored, unfiltered, unsorted in memory, but in DB order).
3. `processedEpisodes` = `episodes.applyFilters(anime)` (1682-1699): the four tri-state filters (unseen, bookmarked, fillermarked, downloaded), then sort by `getEpisodeSort(anime)`.
4. `episodeListItems` = `processedEpisodes.insertSeparators { ... }` (1635-1660): inserts `MissingCount` rows between two consecutive items where the gap is positive.

Filter order is fixed (1689-1692): unseen, bookmarked, fillermarked, downloaded. Sort is last (1693-1698). Filters are lazy sequences; the terminal `sortedWith` on a Sequence is a full sort at evaluation time.

---

## 8. Episode sort, filters, separators (summary)

- Sort key set: `Anime.EPISODE_SORTING_*` (domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:239-243). Direction: `Anime.EPISODE_SORT_DESC = 0`, `EPISODE_SORT_ASC = 1`. Source order is 0, so default sorting is SOURCE + DESC.
- `getEpisodeSort(anime, sortDescending)` (domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt:7-30). Full table in the topic file.
- Filter semantics (`applyFilter`, domain/src/main/java/tachiyomi/domain/entries/TriState.kt:5-9): `DISABLED` passes everything; `ENABLED_IS` passes items where the predicate is true; `ENABLED_NOT` passes items where the predicate is false.
- Separator rule (1636-1659): see topic file section 3.

---

## 9. Episode settings sheet actions and "set as default"

All setters follow the same shape: `val anime = successState?.anime ?: return` (no state write), then `screenModelScope.launchNonCancellable { setAnimeEpisodeFlags.await...(anime, flag) }`. No optimistic UI update: the list refreshes when the DB flow re-emits.

| ScreenModel method | Lines | Flag value mapping | Interactor call |
|---|---|---|---|
| `setUnseenFilter(state: TriState)` | 1011-1022 | DISABLED -> `SHOW_ALL` (0), ENABLED_IS -> `EPISODE_SHOW_UNSEEN` (0x2), ENABLED_NOT -> `EPISODE_SHOW_SEEN` (0x4) | `awaitSetUnseenFilter` (mask `EPISODE_UNSEEN_MASK` 0x6) |
| `setDownloadedFilter(state)` | 1028-1040 | DISABLED -> 0, IS -> `EPISODE_SHOW_DOWNLOADED` (0x8), NOT -> `EPISODE_SHOW_NOT_DOWNLOADED` (0x10) | `awaitSetDownloadedFilter` (mask 0x18) |
| `setBookmarkedFilter(state)` | 1046-1058 | IS -> `EPISODE_SHOW_BOOKMARKED` (0x20), NOT -> `EPISODE_SHOW_NOT_BOOKMARKED` (0x40) | `awaitSetBookmarkFilter` (mask 0x60) |
| `setFillermarkedFilter(state)` | 1064-1076 | IS -> `EPISODE_SHOW_FILLERMARKED` (0x80), NOT -> `EPISODE_SHOW_NOT_FILLERMARKED` (0x100) | `awaitSetFillermarkFilter` (mask 0x180) |
| `setSorting(sort: Long)` | 1094-1100 | value is the sort key | `awaitSetSortingModeOrFlipOrder` (see below) |
| `setDisplayMode(mode: Long)` | 1082-1088 | `EPISODE_DISPLAY_NAME` (0) or `EPISODE_DISPLAY_NUMBER` (0x100000) | `awaitSetDisplayMode` (mask 0x100000) |
| `showEpisodePreviews(flag: Long)` | 1106-1112 | `EPISODE_SHOW_PREVIEWS` (0) or `EPISODE_SHOW_NOT_PREVIEWS` (0x800) | `awaitShowEpisodePreviews` (mask 0x800) |
| `showEpisodeSummaries(flag: Long)` | 1118-1124 | `EPISODE_SHOW_SUMMARIES` (0) or `EPISODE_SHOW_NOT_SUMMARIES` (0x1000) | `awaitShowEpisodeSummaries` (mask 0x1000) |

`awaitSetSortingModeOrFlipOrder` (SetAnimeEpisodeFlags.kt:56-79):
- If `anime.sorting == flag`: flip the direction bit. `orderFlag = if (anime.sortDescending()) EPISODE_SORT_ASC else EPISODE_SORT_DESC`.
- Else: set the new sort key and set the direction to `EPISODE_SORT_ASC`. A newly chosen sort key always starts ascending.

The three-state filter setters map `DISABLED` to `Anime.SHOW_ALL = 0`, which clears the bits for that filter.

### 9.1 `setCurrentSettingsAsDefault(applyToExisting)` (1126-1137)

Runs in `launchNonCancellable`:
1. `libraryPreferences.setEpisodeSettingsDefault(anime)` (LibraryPreferences.kt, see below).
2. If `applyToExisting`: `setAnimeDefaultEpisodeFlags.awaitAll()` (copies defaults to every favorite).
3. `snackbarHostState.showSnackbar(message = episode_settings_updated)` (no action).

`setEpisodeSettingsDefault(anime)` (LibraryPreferences.kt:236-247) writes:
- `filterEpisodeBySeen` <- `anime.unseenFilterRaw`
- `filterEpisodeByDownloaded` <- `anime.downloadedFilterRaw`
- `filterEpisodeByBookmarked` <- `anime.bookmarkedFilterRaw`
- `filterEpisodeByFillermarked` <- `anime.fillermarkedFilterRaw`
- `sortEpisodeBySourceOrNumber` <- `anime.sorting`
- `displayEpisodeByNameOrNumber` <- `anime.displayMode`
- `sortEpisodeByAscendingOrDescending` <- `if (anime.sortDescending()) EPISODE_SORT_DESC else EPISODE_SORT_ASC`
- `showEpisodeThumbnailPreviews` <- `anime.showPreviewsRaw`
- `showEpisodeSummaries` <- `anime.showSummariesRaw`

**DEFECT (23.10):** the display and sort-direction preference keys collide with the manga chapter keys (`default_chapter_display_by_name_or_number`, `default_chapter_sort_by_ascending_or_descending`). See `p2-data-and-config.md` section 4.2.

---

## 10. Selection model (summary)

Full algorithm in `p2-topic-episode-list.md` section 5. State: `selectedPositions: Array<Int>` of size 2 (`[first, last]` selected indices into `processedEpisodes`, initial `[-1, -1]`) and `selectedEpisodeIds: HashSet<Long>` (line 184-185).

- `toggleSelection(item, selected, userSelected = false, fromLongPress = false)` (1369-1431).
- `toggleAllSelection(selected)` (1433-1443) operates on `successState.episodes` (the full list).
- `invertSelection()` (1445-1455) operates on `successState.episodes`.
- Each of the three resets `selectedPositions` to `-1, -1` in the `toggleAll` / `invert` cases and mutates `selectedPositions` in `toggleSelection` only as described in topic section 5.

---

## 11. Episode actions

### 11.1 Swipe (`episodeSwipe`, 707-748)

`episodeSwipe(episodeItem, swipeAction)` launches `screenModelScope.launch { executeEpisodeSwipeAction(...) }`.

`executeEpisodeSwipeAction`:
- `ToggleSeen`: `markEpisodesSeen(listOf(episode), !episode.seen)`.
- `ToggleBookmark`: `bookmarkEpisodes(listOf(episode), !episode.bookmark)`.
- `ToggleFillermark`: `fillermarkEpisodes(listOf(episode), !episode.fillermark)`.
- `Download`: derive `EpisodeDownloadAction` from `downloadState`:
  - `ERROR` or `NOT_DOWNLOADED` -> `START_NOW`.
  - `QUEUE` or `DOWNLOADING` -> `CANCEL`.
  - `DOWNLOADED` -> `DELETE`.
  - then `runEpisodeDownloadActions(listOf(episodeItem), action)`.
- `Disabled`: `throw IllegalStateException()` (line 746). A disabled swipe is a programming error.

The enum (`LibraryPreferences.EpisodeSwipeAction`, LibraryPreferences.kt:383-389): `ToggleSeen`, `ToggleBookmark`, `ToggleFillermark`, `Download`, `Disabled`.

Preference keys and defaults (LibraryPreferences.kt:361-368):
- `swipeEpisodeStartAction()` reads key `pref_episode_swipe_end_action`, default `ToggleSeen`.
- `swipeEpisodeEndAction()` reads key `pref_episode_swipe_start_action`, default `ToggleBookmark`.
- The model assigns `episodeSwipeStartAction = swipeEpisodeEndAction()` and `episodeSwipeEndAction = swipeEpisodeStartAction()`, so the model's "start" action is the key named `..._start_action` (default `ToggleBookmark`) and the model's "end" action is the key named `..._end_action` (default `ToggleSeen`). Net effect: the swipe toward the start edge defaults to bookmark, the swipe toward the end edge defaults to toggle-seen. **Proposal:** name the Rust settings by what they do, not by the Kotlin names.

### 11.2 Bookmark and fillermark bulk (951-973)

- `bookmarkEpisodes(episodes, bookmarked)`: `episodes.filterNot { it.bookmark == bookmarked }.map { EpisodeUpdate(id, bookmark = bookmarked) }`, `updateEpisode.awaitAll(list)`, then `toggleAllSelection(false)`. Runs in `launchIO` for the DB write, but the `toggleAllSelection(false)` call is synchronous.
- `fillermarkEpisodes` is the same with `fillermark`.
- Empty filtered list: no DB call, selection still cleared.

### 11.3 Delete episodes (980-994)

- `showDeleteEpisodeDialog(episodes)` opens the confirm dialog (5.1). Confirm calls `toggleAllSelection(false)` then `deleteEpisodes(dialog.episodes)` (AnimeScreen.kt:254-257).
- `deleteEpisodes(episodes)`: `launchNonCancellable { try { successState?.let { downloadManager.deleteEpisodes(episodes, state.anime, state.source) } } catch (e: Throwable) { logcat(ERROR, e) } }`.
- Deletes downloaded files for the given episodes only. It does not delete episode rows.

### 11.4 Mark seen / unseen (`markEpisodesSeen`, 868-910)

Full flow in section 12. Summary: clear selection, call `setSeenStatus`, then possibly prompt for tracker update.

### 11.5 Mark previous as seen (`markPreviousEpisodeSeen(pointer)`, 855-861)

- `anime = successState?.anime ?: return`.
- `episodes = processedEpisodes.orEmpty().map { it.episode }.toList()`. This is the **filtered and sorted** display list.
- `prevEpisodes = if (anime.sortDescending()) episodes.asReversed() else episodes` (ascending reading order).
- `pointerPos = prevEpisodes.indexOf(pointer)`.
- If `pointerPos != -1`: `markEpisodesSeen(prevEpisodes.take(pointerPos), true)`.

Consequence (23.11): with an active filter the "previous" set excludes hidden episodes, so episodes that were filtered out are not marked. `indexOf` uses `Episode` data-class equality (all fields).

---

## 12. Mark-seen to tracker prompt (`markEpisodesSeen`, 868-910)

Exact sequence:

1. `toggleAllSelection(false)` (line 869). Runs before the empty check, so the selection is cleared even when `episodes` is empty.
2. If `episodes.isEmpty()` return.
3. `screenModelScope.launchIO { ... }`:
   1. `setSeenStatus.await(seen = seen, episodes = episodes.toTypedArray())` (872-875). Result ignored.
   2. If `!seen || successState?.hasLoggedInTrackers == false || autoTrackState == AutoTrackState.NEVER` return.
   3. `refreshTrackers()` (881). Private suspend function (912-931): `RefreshAnimeTracks.await(animeId)`, then for each failure `(track, e)` that has a non-null track: `logcat(ERROR)` and `withUIContext { context.toast(stringResource(MR.strings.track_error, track!!.name, e.message ?: "")) }`.
   4. `tracks = getTracks.await(animeId)` (883).
   5. `maxEpisodeNumber = episodes.maxOf { it.episodeNumber }` (884). Computed over the list passed in, including already-seen episodes.
   6. `shouldPromptTrackingUpdate = tracks.any { maxEpisodeNumber > it.lastEpisodeSeen }` (885). If false, return.
   7. If `autoTrackState == AutoTrackState.ALWAYS`:
      - `trackEpisode.await(context, animeId, maxEpisodeNumber)`.
      - `withUIContext { context.toast(stringResource(AYMR.strings.trackers_updated_summary_anime, maxEpisodeNumber.toInt())) }` (unconditional success toast, see 23.12).
      - return.
   8. Otherwise (`ASK`):
      - `snackbarHostState.showSnackbar(message = confirm_tracker_update_anime(maxEpisodeNumber.toInt()), actionLabel = action_ok, duration = SnackbarDuration.Short, withDismissAction = true)`.
      - If `ActionPerformed`: `trackEpisode.await(context, animeId, maxEpisodeNumber)`. No success toast.

`AutoTrackState` values (app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt:6-10): `ALWAYS`, `ASK`, `NEVER`. Default from `TrackPreferences.autoUpdateTrackOnMarkRead()` key `pref_auto_update_manga_on_mark_read` is `ALWAYS` (TrackPreferences.kt:48-51). Note the key says "manga" but the anime model reads the same key.

`trackEpisode.await` only sends an episode number that is greater than the tracker's `lastEpisodeSeen` (TrackEpisode.kt:32). So the prompt and the send are consistent.

Note on step 2: `successState?.hasLoggedInTrackers == false` means the check passes (continues) when `successState` is null or `hasLoggedInTrackers` is true. A null state here means the screen was torn down; the subsequent calls would not touch state, so this is harmless.

---

## 13. Download actions

### 13.1 Per-episode actions (`runEpisodeDownloadActions`, 806-833)

| Action | Behavior |
|---|---|
| `START` | `startDownload(items.map { it.episode }, startNow = false)`. Then if any item has `downloadState == ERROR`, `downloadManager.startDownloads()`. |
| `START_NOW` | `items.singleOrNull()?.episode ?: return`; `startDownload(listOf(episode), startNow = true)`. Multi-select: silent no-op. |
| `CANCEL` | `items.singleOrNull()?.id ?: return`; `cancelDownload(episodeId)`. Multi-select: no-op. |
| `DELETE` | `deleteEpisodes(items.map { it.episode })`. |
| `SHOW_QUALITIES` | `items.singleOrNull()?.episode ?: return`; `showQualitiesDialog(episode)`. |

Enum `EpisodeDownloadAction` (app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt:42-48): `START`, `START_NOW`, `CANCEL`, `DELETE`, `SHOW_QUALITIES`.

`startDownload(episodes, startNow, video = null)` (776-804):
- `successState` captured; if null return.
- `launchNonCancellable`:
  - `startNow`: `episodeId = episodes.singleOrNull()?.id ?: return`; `downloadManager.startDownloadNow(episodeId)`.
  - else: `downloadEpisodes(episodes, alt = false, video)`.
  - Then if `!isFavorited && !successState.hasPromptedToAddBefore`:
    - `updateSuccessState { copy(hasPromptedToAddBefore = true) }`.
    - `snackbarHostState.showSnackbar(message = snack_add_to_anime_library, actionLabel = action_add, withDismissAction = true)`.
    - If `ActionPerformed && !isFavorited`: `toggleFavorite()` (4.3 with its remove/add branch; here `isFavorited` is false so this is the add path).

`downloadEpisodes(episodes, alt, video)` (937-945): `downloadManager.downloadEpisodes(anime, episodes, autoStart = true, alt, video)` then `toggleAllSelection(false)`.

### 13.2 "Next N" and "Unseen" (`runDownloadAction`, 835-847)

`DownloadAction` enum (app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt:3-9): `NEXT_1_ITEM`, `NEXT_5_ITEMS`, `NEXT_10_ITEMS`, `NEXT_25_ITEMS`, `UNVIEWED_ITEMS`.

- `NEXT_1_ITEM` -> `getUnseenEpisodesSorted().take(1)`.
- `NEXT_5_ITEMS` -> `take(5)`, `NEXT_10_ITEMS` -> `take(10)`, `NEXT_25_ITEMS` -> `take(25)`.
- `UNVIEWED_ITEMS` -> `getUnseenEpisodes()` (all, unsorted order).
- If not empty: `startDownload(list, startNow = false)`.

`getUnseenEpisodes()` (762-768): `processedEpisodes.filter { (episode, dlStatus) -> !episode.seen && dlStatus == NOT_DOWNLOADED }.map { it.episode }`. Note it uses `processedEpisodes`, so hidden-by-filter episodes are excluded (23.11).

`getUnseenEpisodesSorted()` (770-774): `episodes = getUnseenEpisodes().sortedWith(getEpisodeSort(anime))`; `if (anime.sortDescending()) episodes.reversed() else episodes`. The double sort (display sort, then sort again then reverse for descending) produces ascending-by-display-order for descending display, i.e. oldest first. Downloads therefore start from the earliest unseen episode in the display order.

### 13.3 Cancel (`cancelDownload`, 849-853)

- `activeDownload = downloadManager.getQueuedDownloadOrNull(episodeId) ?: return`.
- `downloadManager.cancelQueuedDownloads(listOf(activeDownload))`.
- `updateDownloadState(activeDownload.apply { status = AnimeDownload.State.NOT_DOWNLOADED })`. Mutates the shared `AnimeDownload` object (23.13).

### 13.4 Download status observers (`observeDownloads`, 569-593; `updateDownloadState`, 595-607)

- Two collectors: `statusFlow()` and `progressFlow()`, each `filter { it.anime.id == successState?.anime?.id }`, `catch { logcat }`, `flowWithLifecycle(lifecycle)`, and `withUIContext { updateDownloadState(it) }`.
- `updateDownloadState(download)`: finds the index of the item with `it.id == download.episode.id` in `successState.episodes`. If not found, return unchanged. Otherwise replace that element with `copy(downloadState = download.status, downloadProgress = download.progress)`. The list is rebuilt with `toMutableList().apply { removeAt; add(index) }`, so the item stays at the same index.

### 13.5 Download state derivation (`toEpisodeListItems`, 609-640)

For each episode:
- `isLocal = anime.isLocal()` (source id `LocalAnimeSource.ID` = 0L; LocalAnimeSource.kt:365, 375).
- `activeDownload = if (isLocal) null else downloadManager.getQueuedDownloadOrNull(episode.id)`.
- `downloaded = if (isLocal) true else downloadManager.isEpisodeDownloaded(episode.name, episode.scanlator, anime.title, anime.source)`. Four-argument call, so `skipCache = false` (cache path).
- `downloadState = when { activeDownload != null -> activeDownload.status; downloaded -> DOWNLOADED; else -> NOT_DOWNLOADED }`.
- `downloadProgress = activeDownload?.progress ?: 0`.
- `selected = episode.id in selectedEpisodeIds`.

Precedence: an episode in the queue shows its queue status (QUEUE, DOWNLOADING or ERROR) even if the files exist. `DOWNLOADED` only when not queued.

---

## 14. Seasons (summary)

Full detail in `p2-topic-seasons.md`.

- Season list mapping `toAnimeSeasonItems` (642-653): `downloadCount = downloadManager.getDownloadCount(seasonAnime.anime).toLong()`, `unseenCount = seasonAnime.unseenCount`, `isLocal = seasonAnime.anime.isLocal()`, `sourceLanguage = sourceManager.getOrStub(seasonAnime.anime.source).lang`, `showContinueOverlay = false`.
- Season pipeline `applySeasonFilters` (1701-1737): six tri-state filters, then sort, then overlay gating.
- Season setters: 1143-1367. The season settings default bug is 23.9.

---

## 15. Related anime

- `observeRelatedAnime()` (1461-1470): `getRelatedAnime.subscribe(animeId).flowWithLifecycle(lifecycle).distinctUntilChanged().collectLatest { updateSuccessState { copy(relatedAnime = it) } }`.
- `syncRelatedAnime(forceRefresh = false)` (1472-1486):
  1. `state = successState ?: return`; `if (!state.source.supportsRelatedAnime) return`.
  2. `launchIO`: `updateSuccessState { copy(isLoadingRelatedAnime = true) }`; `syncRelatedAnimeWithSource.await(state.anime, forceRefresh)`; catch `Throwable` -> log ERROR "Failed to fetch related anime"; finally `copy(isLoadingRelatedAnime = false)`.
- `SyncRelatedAnimeWithSource.await` (SyncRelatedAnimeWithSource.kt:18-38):
  - `source = sourceManager.get(anime.source)?.takeIf { it.supportsRelatedAnime } ?: return`. Note `get`, not `getOrStub`.
  - If `!forceRefresh`: `lastFetchedAt = relationRepository.getLastFetchedAt(anime.id)`; if non-null and `now - lastFetchedAt < TTL` return. `TTL = 7.days` (`7 * 24 * 3600 * 1000` ms, line 41).
  - `groups = source.getRelatedAnimeList(anime.toSAnime()).map { relation -> relation.name to relation.animes.distinctBy { it.url }.filterNot { it.url == anime.url }.take(MAX_PER_GROUP).map { networkToLocalAnime.await(it.toDomainAnime(anime.source)).id } }.filter { (_, ids) -> ids.isNotEmpty() }`. `MAX_PER_GROUP = 10` (line 42).
  - `relationRepository.replaceRelations(anime.id, groups, now)`.
- Side effect: `networkToLocalAnime.await` inserts each related anime into the `animes` table (as non-favorite rows) if not already present (area 07/05).
- Trigger points: init (`syncRelatedAnime()` with `forceRefresh=false`) and every full refresh (`fetchDetails == true` -> `syncRelatedAnime(forceRefresh = true)`).
- Display groups are presentation (part 1). Long-press opens `GlobalAnimeSearchScreen(relatedAnime.title)` (AnimeScreen.kt:226-228); tap opens `AnimeScreen(relatedAnime.id, fromSource = !relatedAnime.favorite)` (223-225).

---

## 16. Tracker summary and airing (summary)

- `observeTrackers()` (1492-1533) has two collectors:
  1. Summary: `combine(getTracks.subscribe(anime.id).catch { log }, trackerManager.loggedInTrackersFlow())` -> `supportedTrackers = loggedInTrackers.filter { (it as? EnhancedAnimeTracker)?.accept(source!!) ?: true }`, `supportedTrackerTracks = animeTracks.filter { it.trackerId in supportedTrackerIds }`, emits `(size, supportedTrackers.isNotEmpty())`. `distinctUntilChanged`, `collectLatest` -> `copy(trackingCount, hasLoggedInTrackers)`.
  2. Airing: `combine(...)` -> `loggedInTrackers.map { AnimeTrackItem(animeTracks.find { it.trackerId == service.id }, service) }` (no accept filter). `distinctUntilChanged`, `collectLatest { updateAiringTime(anime, trackItems, manualFetch = false) }`.
- `updateAiringTime(anime, trackItems, manualFetch)` (1535-1543): `airingEpisodeData = AniChartApi().loadAiringTime(anime, trackItems, manualFetch)`; `setAnimeViewerFlags.awaitSetNextEpisodeAiring(anime.id, airingEpisodeData)`; `copy(nextAiringEpisode = airingEpisodeData)`.
- Details: `p2-topic-airing-and-related.md`.

---

## 17. Continue watching and opening an episode

### 17.1 Next unseen (`getNextUnseen`)

- `getNextUnseenEpisode(): Episode?` (757-760, non-suspend): `successState.episodes.getNextUnseen(successState.anime)` (the EpisodeList overload in EpsiodeGetNextUnseen.kt:25-31). Uses the full episode list and applies filters itself.
- `getNextUnseenEpisode(anime): Episode?` (750-752, suspend): `getEpisodesByAnimeId.await(anime.id).getNextUnseen(anime, downloadManager)` (EpsiodeGetNextUnseen.kt:12-18). Used for season cards.
- Algorithm for both overloads (EpsiodeGetNextUnseen.kt:12-31): take the filtered and sorted list (`applyFilters`), then `if (anime.sortDescending()) findLast { !seen } else find { !seen }`. The result is the first unseen item in display order.

### 17.2 Continue watching button (`onContinueWatching`, AnimeScreen.kt:176-181)

`scope.launchIO { continueWatching(context, screenModel.getNextUnseenEpisode(), extPlayer = screenModel.alwaysUseExternalPlayer) }`. `continueWatching` opens it only if non-null.

### 17.3 Season continue (`onContinueWatchingClicked`, 215-222)

`scope.launchIO { val episode = screenModel.getNextUnseenEpisode(it.anime); episode?.let { openEpisode(context, it, screenModel.alwaysUseExternalPlayer) } }`.

### 17.4 Opening an episode (`onEpisodeClicked`, AnimeScreen.kt:136-144)

```
onEpisodeClicked = { episode, alt ->
    scope.launchIO {
        if (screenModel.isTorrentEnabled() && successState.source.isSourceForTorrents()) TorrentServerService.start()
        val extPlayer = screenModel.alwaysUseExternalPlayer != alt
        openEpisode(context, episode, extPlayer)
    }
}
```

- `alt` is true for the "other player" action (long-press or secondary action in the presentation). `extPlayer = alwaysUseExternalPlayer XOR alt`.
- `openEpisode` (AnimeScreen.kt:421-430): `withIOContext { MainActivity.startPlayerActivity(context, episode.animeId, episode.id, useExternalPlayer) }`.

Note that `continueWatching` and the season continue path use `alwaysUseExternalPlayer` without the XOR (no alt is available there).

### 17.5 Torrent start on play

Covered in 6.9. Start is attempted before every play of a torrent source, regardless of whether it is already running. `TorrentServerService.start()` starts the service intent every time.

---

## 18. Search, genre search, share, WebView, copy URL

### 18.1 `performSearch(navigator, query, global)` (476-496)

- `global == true`: `navigator.push(GlobalAnimeSearchScreen(query))` and return.
- `navigator.size < 2`: return (no previous screen).
- `previousController = navigator.items[navigator.size - 2]`:
  - `HomeScreen` -> `navigator.pop()`, `AnimeLibraryTab.search(query)`.
  - `BrowseAnimeSourceScreen` -> `navigator.pop()`, `previousController.search(query)`.
  - Other: nothing happens (23.14).
- Callers: `onSearch = { query, global -> scope.launch { performSearch(navigator, query, global) } }` (AnimeScreen.kt:182).

### 18.2 `performGenreSearch(navigator, genreName, source)` (503-519)

- `navigator.size < 2`: return.
- If `previousController is BrowseAnimeSourceScreen && source is AnimeHttpSource`: `navigator.pop()` and `previousController.searchGenre(genreName)`.
- Else: `performSearch(navigator, genreName, global = false)`.
- Caller: `onTagSearch = { scope.launch { performGenreSearch(navigator, it, screenModel.source!!) } }` (AnimeScreen.kt:173). `screenModel.source!!` throws if the state is not Success.

### 18.3 Share (`shareAnime`, 455-469)

- `getAnimeUrl(anime, source)` (432-441): `source as? AnimeHttpSource` else null; `source.getAnimeUrl(anime.toSAnime())` wrapped in try/catch returning null.
- If non-null: `url.toUri().toShareIntent(context, type = "text/plain")`, `Intent.createChooser(intent, stringResource(MR.strings.action_share))`, `context.startActivity(...)`.
- Any exception: `context.toast(e.message)`.
- Shown only when `isAnimeHttpSource` (AnimeScreen.kt:190).

### 18.4 WebView (`openAnimeInWebView`, 443-453)

- URL from `getAnimeUrl`. If non-null: `navigator.push(WebViewScreen(url = url, initialTitle = anime.title, sourceId = source.id))`.
- Shown only when `isAnimeHttpSource` (AnimeScreen.kt:158).
- Long-press (`onWebViewLongClicked`, 159-165) calls `copyAnimeUrl`.

### 18.5 Copy URL (`copyAnimeUrl`, 524-529)

- `anime ?: return`, `source as? AnimeHttpSource ?: return`.
- `url = source.getAnimeUrl(anime.toSAnime())` **without try/catch** (23.15).
- `context.copyToClipboard(url, url)`.

### 18.6 Assist URL (`LaunchedEffect`, AnimeScreen.kt:114-124)

For `AnimeHttpSource` only: computes `getAnimeUrl` on `Dispatchers.IO` and stores it in `assistUrl` for `onProvideAssistUrl()`. Android-only; Dropped (task 066).

---

## 19. User-visible message keys

Each row: key as referenced in code, English text from the string resource, where it is shown, and the trigger.

| Key (as referenced) | English text | Shown by | Trigger |
|---|---|---|---|
| `AYMR.strings.no_episodes_error` | No episodes found | snackbar (no action) | refresh with `NoEpisodesException` (6.3) |
| `AYMR.strings.no_seasons_error` | No seasons found | snackbar | refresh with `NoSeasonsException` |
| `e.formattedMessage` (util) | (exception text) | snackbar | any other refresh failure |
| `AYMR.strings.delete_downloads_for_anime` | Delete downloaded episodes? | snackbar, action `MR.strings.action_delete` ("Delete"), dismiss button | removing from library with downloads present (4.2) |
| `AYMR.strings.snack_add_to_anime_library` | Add anime to library? | snackbar, action `MR.strings.action_add` ("Add"), dismiss button | first download of a non-library entry (13.1) |
| `AYMR.strings.trackers_updated_summary_anime` | Trackers updated to episode %d | toast | ALWAYS auto-track after mark seen (12 step 7) |
| `AYMR.strings.confirm_tracker_update_anime` | Update trackers to episode %d? | snackbar, action `MR.strings.action_ok` ("OK"), short duration, dismiss button | ASK auto-track (12 step 8) |
| `AYMR.strings.episode_settings_updated` | Updated default episode settings | snackbar, no action | `setCurrentSettingsAsDefault` |
| `AYMR.strings.season_settings_updated` | Updated default season settings | snackbar, no action | `setSeasonCurrentSettingsAsDefault` |
| `AYMR.strings.display_mode_episode` | Episode %1$s | episode-dialog title (presentation) | `ShowQualities` title when display mode is number (AnimeScreen.kt:391-395) |
| `MR.strings.track_error` | %1$s error: %2$s | toast | refresh of a tracker failed (`refreshTrackers`, 912-931; and the tracking home sheet, AnimeTrackInfoDialog.kt:240-260) |
| `MR.strings.error_no_match` | No match found | toast | enhanced tracker `match` returned null or threw (AnimeTrackInfoDialog.kt:227-238) |
| `MR.strings.action_share` | Share | chooser title | share (18.3) |
| `MR.strings.track_started_reading_date` | Start date | date-picker title | TrackDateSelector start (AnimeTrackInfoDialog.kt:539-541) |
| `MR.strings.track_finished_reading_date` | Finish date | date-picker title | TrackDateSelector finish |
| `MR.strings.track_remove_date_conf_title` | Remove date? | alert title | date remover |
| `MR.strings.track_remove_start_date_conf_text` | This will remove your previously selected start date from %s | alert text | date remover, start |
| `MR.strings.track_remove_finish_date_conf_text` | This will remove your previously selected finish date from %s | alert text | date remover, finish |
| `MR.strings.action_cancel` | Cancel | button | multiple |
| `MR.strings.action_remove` | Remove | button | date remover |
| `MR.strings.track_delete_title` | Remove %s tracking? | alert title | tracker remove |
| `MR.strings.track_delete_text` | This will remove the tracking locally. | alert text | tracker remove |
| `MR.strings.track_delete_remote_text` | Also remove from %s | checkbox label | tracker remove, only when tracker is deletable |
| `MR.strings.action_ok` | OK | button | several |
| `MR.strings.action_delete` | Delete | button | delete prompt |
| `MR.strings.action_add` | Add | button | add prompt |

Resource files: `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` for `AYMR`, `i18n/src/commonMain/moko-resources/base/strings.xml` for `MR`. The values above were read from those files: lines 501-504, 516-517, 535-536 and 382-384 for AYMR; lines 86, 91, 106, 135-136, 151, 791-806 for MR.

Localized strings for other locales are not in scope; Kurobayomi uses the English base.

---

## 20. Side effects

| Action | DB writes | Files | Network | Notifications / UI |
|---|---|---|---|---|
| Open entry (init) | `episodeFlags`/`seasonFlags` overwritten for non-favorites (2.3.5); `viewerFlags` airing bits (16) | cover/background timestamps may change (6.5) | source details + episode list if needed (2.3.10); related anime (15); AniChart (16) | loading screen; `isRefreshingData` |
| Add to library | `favorite = true`; category set; possibly enhanced-tracker bind (`anime_sync` row) | none | enhanced `match` (may hit tracker API) | category dialog or track sheet |
| Remove from library | `favorite = false`; cover cache deleted if changed | cover file removed | none | snackbar "Delete downloaded episodes?" when downloads exist |
| Refresh | anime row (title if not favorite, cover/background timestamps, author, artist, description, genres, status, initialized); episodes inserted/updated/removed; seasons similarly | cover and background cache deletes (6.5) | source calls | snackbar on error; `isRefreshingData` |
| Mark seen | `episodes.seen`, `episodes.last_second_seen` (reset on unseen) | downloads deleted when `removeAfterMarkedAsRead` and seen | tracker refresh and possibly update (ALWAYS / ASK) | toast or snackbar |
| Bookmark / fillermark | `episodes.bookmark` / `episodes.fillermark` | none | none | selection cleared |
| Delete episodes | none | episode directories deleted; queued downloads removed; empty anime directory removed | none | none |
| Settings change | `episodeFlags` / `seasonFlags` | none | none | sheet remains open; list updates on DB emission |
| Set as default | preference writes; with apply-all, `episodeFlags` for every favorite | none | none | snackbar |
| Download start | queue entries | downloads written later by the downloader | source video lookup in the downloader | optional add-to-library snackbar |
| Cancel download | queue removal | partial files cleaned by downloader | none | item state -> NOT_DOWNLOADED |
| Set fetch interval | `fetchInterval`, `nextUpdate` | none | none | dialog closes |
| Set skip intro | `viewerFlags` (skip-intro bits) | none | none | dialog closes |
| Torrent start | none | torrent server data dir (service) | local echo polling | none |
| Share / open WebView / copy | none | none | none | intent chooser, WebView screen, clipboard |

---

## 21. Concurrency and dispatch map

| Scope | Dispatcher / coroutine | Notes |
|---|---|---|
| `screenModelScope` (Voyager) | main thread (Voyager default) | All `launch` without an explicit dispatcher. `updateSuccessState` is called from here. |
| `launchIO` | `Dispatchers.IO` | DB writes, network. Used for almost every mutator. |
| `launchNonCancellable` | `Dispatchers.IO` + `NonCancellable` | Setters, deletes, download starts, default-setting writes. Work continues after the screen is popped. |
| `withUIContext` | `Dispatchers.Main` | Toasts and `updateDownloadState`. |
| `withIOContext` | `Dispatchers.IO` | Inside interactors and `openEpisode`. |
| `async(Dispatchers.IO)` | season episode refresh for `UnmeteredSource` | Parallel. |
| `supervisorScope` | `RefreshAnimeTracks.await` | One tracker failure does not cancel the others. |
| `collectLatest` | init subscriptions, tracker summary, related anime | Cancels the previous collector body on a new emission. |
| `flowWithLifecycle(lifecycle)` | all long-lived collectors | Collection stops when the lifecycle leaves STARTED and resumes on return. |

Shared mutable state:
- `selectedPositions` and `selectedEpisodeIds` are mutated inside `updateSuccessState` lambdas (1369-1455), which are executed inside `MutableStateFlow.update`. `update` may retry the lambda under contention, so side effects inside it are not idempotent (23.16). Proposal: compute the new selection first, then publish it once.
- `isFromChangeCategory` (var) is written from `launchIO` and read from the UI thread (AnimeScreen.kt:234-235). No synchronization.
- `autoTrackState` (var) is set once at construction and read from `launchIO`.

---

## 22. Timings, limits, constants

| Name | Value | Where | Notes |
|---|---|---|---|
| Torrent server wait | 10 (count of 1-second polls; returns after count > 10) | AnimeScreenModel.kt:662; TorrentServerService.kt:173-188 | Returns false silently after ~11 s. |
| Snackbar duration (auto-track ASK) | `SnackbarDuration.Short` | AnimeScreenModel.kt:902 | Other snackbars use the default. |
| Related anime TTL | 7 days | SyncRelatedAnimeWithSource.kt:41 | Skipped when `!forceRefresh`. |
| Related anime max per group | 10 | SyncRelatedAnimeWithSource.kt:42 | |
| Fetch interval max | 28 days | AnimeFetchInterval.kt:126 | |
| Fetch interval default (smart, no data) | 7 days | AnimeFetchInterval.kt:111-112 (`else -> 7`) | |
| Fetch interval clamp (smart) | 1..28 | AnimeFetchInterval.kt:113 | |
| Fetch interval grace period | 1 day | AnimeFetchInterval.kt:128 (`GRACE_PERIOD = 1L`) | Window of +/- 1 day around now. |
| Skip intro default | 85 seconds | GesturePreferences.kt:21 | |
| Skip intro range | 0..255 seconds | AnimeScreen.kt:369-388 and Anime.kt ANIME_INTRO_MASK | 8 bits. |
| Track episode range (no total) | 0..10000 | AnimeTrackInfoDialog.kt:376-382 | |
| Track episode range (with total) | 0..totalEpisodes inclusive | same | |
| Next-N download counts | 1, 5, 10, 25 | AnimeScreenModel.kt:837-840 | |
| Season grid default size | 0 | LibraryPreferences.kt:295-297 | Presentation interprets. |
| Default season grid mode | CompactGrid (0) | LibraryPreferences.kt:290-293 | |
| Season overlay defaults | download false, unseen true, local true, lang false, continue true | LibraryPreferences.kt:300-322 | |
| Default episode sort | SOURCE, DESC | LibraryPreferences.kt:195-218 | |
| Default season sort | SOURCE, DESC | LibraryPreferences.kt:280-288 | |
| Default display | NAME (0) | LibraryPreferences.kt:205-208 | |
| Default previews / summaries | SHOW (0) / SHOW (0) | LibraryPreferences.kt:225-233 | |
| Default filters | SHOW_ALL (0) | LibraryPreferences.kt:173-192 | |
| Default `hasPromptedToAddBefore` | false | AnimeScreenModel.kt:1619 | Per model instance, not persisted. |
| Missing-count separator minimum | count > 0 | AnimeScreenModel.kt:1652 | |
| Selected list cap | none | | |
| Max episodes sort | none | | |

---

## 23. Observed defects

Each defect is listed with the place and a recommendation. The Rust plan states per defect whether to keep or fix it (`p2-edge-cases-and-tests.md` section 2 repeats the list with test cases).

1. **23.1 Non-library entries lose per-entry view settings on every open.** AnimeScreenModel.kt:243-246. Proposal: fix. Apply defaults only once, at add-to-library time, and for non-library entries use an in-memory override for the session.
2. **23.2 Single toggle replaces the full episode list with the filtered list.** AnimeScreenModel.kt:1376-1377 and 1429: `successState.processedEpisodes` is written to `episodes`. Proposal: fix. Keep the full list in `episodes` and derive the view.
3. **23.3 `trackItems` is never populated.** The field stays at `emptyList()` (AnimeScreenModel.kt:1620), so `trackingAvailable` is always false, and the manual refresh at line 295 calls `updateAiringTime(anime, emptyList(), manualFetch = true)`. `AniChartApi.loadAiringTime` then returns `Pair(1, 0L)` (AniChartApi.kt:35) and this overwrites the stored next-episode airing data with episode 1 and time 0 until the tracker flow re-emits. Proposal: fix by populating from the same tracker collector used in 16.
4. **23.4 Torrent start can throw.** `isSourceForTorrents()` uses `!!` (AnimeSourceExtensions.kt:9). `startTorrentServer` runs outside the `try` block (AnimeScreenModel.kt:307), so an exception propagates out of the refresh coroutine. Proposal: fix. Return false when the extension is not found.
5. **23.5 Track sheet vs category dialog is ordered by timing.** See 5.4. Proposal: fix. Use an explicit "pending track sheet" flag set in the category branch and consumed on dismiss.
6. **23.6 Duplicate dialog can remain visible.** The confirm path (AnimeScreen.kt:265) does not clear `dialog` itself. Proposal: fix. Dismiss first, then call the action.
7. **23.7 Auto-download never applies to season episodes.** `fetchEpisodesFromSeasons` discards the result (AnimeScreenModel.kt:678-683). Proposal: keep the current behavior for Kurobayomi v1 and make it a P2 feature (task 022 acceptance).
8. **23.8 Custom update wheel hidden in release builds.** ItemsDialogs.kt:126 with `isReleaseBuildType`. Kurobayomi has no release/debug split, so the Proposal is to always show the wheel.
9. **23.9 Swipe settings and season defaults use misleading names.** The name/key mismatch in 11.1. Also `setSeasonSettingsDefault` writes `filterSeasonByDownload` from `seasonUnseenFilterRaw` (LibraryPreferences.kt:331). Proposal: fix the season default to use `seasonDownloadedFilterRaw`.
10. **23.10 Preference key collisions between anime episode and manga chapter defaults.** `displayEpisodeByNameOrNumber` uses `default_chapter_display_by_name_or_number` (LibraryPreferences.kt:205-208); `sortEpisodeByAscendingOrDescending` uses `default_chapter_sort_by_ascending_or_descending` (215-218). Writing the anime default changes the manga default. Proposal: fix by using `default_episode_*` keys in Kurobayomi (the library data is new).
11. **23.11 Mark-previous and unseen-download act only on the filtered view.** `markPreviousEpisodeSeen` (855-861) and `getUnseenEpisodes` (762-768) use `processedEpisodes`. Proposal: keep for "previous" (visible selection semantics) but make "download unseen" use the full episode list, documented in the UI.
12. **23.12 Track success toast is unconditional.** The ALWAYS branch shows "Trackers updated" even when `trackEpisode` skipped or failed (AnimeScreenModel.kt:889-896). Proposal: fix; show only when at least one tracker accepted the update.
13. **23.13 `cancelDownload` mutates a shared object.** `activeDownload.apply { status = ... }` (852). Proposal: fix. Build a new value.
14. **23.14 Search from a non-Home, non-Browse previous screen does nothing.** AnimeScreen.kt:486-495. Proposal: fix. Fall back to `GlobalAnimeSearchScreen`.
15. **23.15 Copy URL has no error handling.** AnimeScreen.kt:527 with no try/catch, unlike `getAnimeUrl`. Proposal: fix.
16. **23.16 Side effects inside `MutableStateFlow.update`.** Selection mutation at 1376-1431 (see 21). Proposal: fix.
17. **23.17 Track `lateinit` search field.** `AnimeTrackSearch.tracking_url` is `lateinit` (AnimeTrackSearch.kt:35). `trackingSearch` reads it inside `mutableState.update` (AnimeTrackInfoDialog.kt:750), outside the try. A search result without `tracking_url` throws and crashes the coroutine. Proposal: fix with `Option<String>`.
18. **23.18 Preselection matches null URLs.** `find { it.tracking_url == currentUrl }` with `currentUrl == null` selects any result whose URL is null (AnimeTrackInfoDialog.kt:750). Proposal: fix. Require a non-null `currentUrl`.
19. **23.19 Stale track object written to the remote.** Track sheet child screens capture `track` at navigation time and call `setRemote*` with it (AnimeTrackInfoDialog.kt:331, 364-366, 391-396, 448, 578-580, 670-673). `update(track)` sends the whole object, so fields that changed since the capture are overwritten. Proposal: fix. Re-read the track row before each remote write.
20. **23.20 Score not in list stores -1.** `setRemoteScore` uses `indexOf(scoreString)` (AnimeTracker.kt:90-93). If the display string is not in `getScoreList()`, the score becomes -1.0. Proposal: fix. Reject the write and show an error.
21. **23.21 Date validity uses the wrong zone for the picker.** The picker's UTC-midnight millis are converted to the system zone for the future check (AnimeTrackInfoDialog.kt:468-472), so for zones west of UTC the day shown can differ by one. Proposal: fix by comparing `LocalDate` values in UTC for picker input.
22. **23.22 `refreshTrackers` swallows the exception object for `track == null`.** `track!!` in the forEach (AnimeScreenModel.kt:917-929) is guarded by `filter { it.first != null }`, so this is not reachable. Recorded for completeness.
23. **23.23 Duplicate-check ignores the non-favorite case.** `GetDuplicateLibraryAnime` requires `favorite = 1` (animes.sq:131). Intended. Recorded.
24. **23.24 Empty-list re-fetch loop.** A source that returns zero episodes re-fetches on every open (2.4). Proposal: fix with a 10-minute guard.

---

## 24. Cross-area references

| Concern | Owning area |
|---|---|
| Episode and season DB tables, SQL | area:04-db-anime-schema-migrations, area:05-data-repos-entries-items |
| Domain entities and interactors (`Anime`, `Episode`, flags helpers, `AnimeFetchInterval`, sync) | area:07-domain-entries-items |
| Library categories | area:08-domain-library-category-history-updates |
| Download manager, cache, queue | area:24-downloads-anime, area:25-downloads-shared |
| Tracker services and remote calls | area:27-tracking-core, area:28-tracker-anilist, area:29-tracker-myanimelist, area:30-tracker-kitsu, area:31-tracker-shikimori-bangumi, area:32-tracker-mangaupdates-simkl, area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin |
| Covers and backgrounds (cache) | area:34-images-covers-cache |
| Navigation, WebView, shell | area:36-ui-shell-navigation-security-webview |
| Presentation of this screen (composables, rows, dialogs, settings sheets) | area:39-ui-entry-details-anime part 1 |
| Preference store | area:02-preferences-system |
| Player and playback | area:45-player-core, area:46-player-controls-ui |
| Source API (`AnimeSource`, `SAnime`, `FetchType`) | area:16-source-api-anime |
| Extension management | area:19-extensions-anime |
