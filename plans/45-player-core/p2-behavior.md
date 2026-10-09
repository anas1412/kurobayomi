# 45-player-core (part 2 of 2) — Behavior spec

Repo root: `/home/blackbox/aniyomi-src`. Every path is repo-relative. `OBSERVED` = what the Kotlin does. `Proposal:` = what the Rust port should do. Cross-references: `area:<id>`.

Scope files covered here: `PlayerViewModel.kt`, `loader/EpisodeLoader.kt`, `loader/HosterLoader.kt`, `settings/*Preferences.kt` (6 files), `utils/AniSkipApi.kt`, `utils/ChapterUtils.kt`, `utils/TrackSelect.kt`. Values from part 1 that this code relies on are quoted in `p2-data-and-config.md` section 2.

## 0. Global facts that shape every algorithm

- OBSERVED: The ViewModel is created by `PlayerViewModelProviderFactory` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:145-151`) with `extras.createSavedStateHandle()`. Only `SavedStateHandle` survives process death; everything else is re-derived by `init()`.
- OBSERVED: mpv property observation is driven by `AniyomiMPVView.kt:210-215` with `MPV_FORMAT_INT64` for `time-pos`, `duration`, `volume`, `volume-max`, `demuxer-cache-time`, `chapter`. Therefore `time-pos` events fire once per whole second of playback (integer change), not every frame. Every whole-second event triggers, in order: `updatePlayBackPos(value)` then `setChapter(value)` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:668-671`).
- OBSERVED: `volume-max` observer stores `volumeBoostCap = value - 100` (`PlayerActivity.kt:676`). mpv's `volume-max` is `100 + boost cap`.
- OBSERVED: `user-data/current-anime/intro-length` (mpv property written by `init`, PlayerViewModel.kt:1286) is observed back and forwarded to `setAnimeSkipIntroLength` (`PlayerActivity.kt:677`). This is the bridge that lets a Lua script change the skip length.
- OBSERVED: `track-list` and `chapter-list` (mpv observed with `MPV_FORMAT_NONE`) trigger `loadTracks()` and `loadChapters()` + `updateChapter(0)` (`PlayerActivity.kt:683-688`).
- OBSERVED: `user-data/aniyomi` property changes are forwarded to `handleLuaInvocation(property, value)` (`PlayerActivity.kt:740`).
- Proposal: keep the same event shape. libmpv's client API delivers property-change events on its own thread; the Rust port forwards them to the GTK main thread through a `glib` channel (see `p2-rust-design.md` section 4). Integer-format observation is preserved by requesting `MPV_FORMAT_INT64` for those properties.

## 1. ViewModel construction and the custom-button init block

OBSERVED (`PlayerViewModel.kt:153-347`):
1. Dependencies are resolved from Injekt at construction. Preference objects are singletons; `sourceManager`, `downloadManager`, etc. are interactors.
2. Initial state:
   - `playbackSpeed = playerPreferences.playerSpeed().get()` (default 1f).
   - `_controlsShown = !playerPreferences.hideControls().get()` (default `hideControls` = false → shown).
   - `_seekBarShown = !hideControls().get()`.
   - `currentBrightness`: `Settings.System.getFloat(contentResolver, SCREEN_BRIGHTNESS).normalize(0f, 255f, 0f, 1f)` wrapped in `runCatching {...}.getOrElse { 0f }`.
   - `currentVolume = audioManager.getStreamVolume(STREAM_MUSIC)`.
   - `currentMPVVolume = MPVLib.getPropertyInt("volume")` (nullable).
   - `volumeBoostCap = MPVLib.getPropertyInt("volume-max")` (nullable Int from property; the field is declared non-null `Int`, so a null at construction is a runtime error in Kotlin. Proposal: treat null as 100).
   - `_currentDecoder = getDecoderFromValue(MPVLib.getPropertyString("hwdec"))` — `getDecoderFromValue` does `Decoder.entries.first { it.value == value }` and therefore throws `NoSuchElementException` for any string not in `{auto-copy, auto, no, mediacodec-copy, mediacodec}`. Proposal: fall back to `Decoder::Auto` (see E-07).
   - `_pausedState = false` (comment: "False because the video shouldn't start paused").
   - `_customButtons = CustomButtonFetchState.Loading`.
3. `init {}` (328-347) runs in `viewModelScope.launchIO`:
   - `buttons = getCustomButtons.getAll()`.
   - If any button has `isFavorite`, set `_primaryButton` to the first such button. If `_primaryButtonTitle` is empty, call `setPrimaryCustomButtonTitle(button)` (title = `button.name`). A Lua script that already set a title is not overridden (comment at 334-335).
   - `activity.setupCustomButtons(buttons)` (part 1 function).
   - `_customButtons = Success(buttons.toImmutableList())`.
   - On any exception: log at ERROR, `_customButtons = Error(e.message ?: "Unable to fetch buttons")`.

## 2. Episode identity, playlist, and navigation

### 2.1 `needsInit(animeId, episodeId)` (1235-1237)
`currentAnime.value?.id != animeId || currentEpisode.value?.id != episodeId`. Returns true when either differs (note: when `currentAnime` is null, `null != animeId` is true).

### 2.2 `init(animeId, initialEpisodeId, hostList, hostIndex, vidIndex)` (1252-1328)
Returns `Pair<InitResult, Result<Boolean>>`.

Algorithm:
1. `defaultResult = InitResult(currentHosterList, qualityIndex, null)`.
2. If `!needsInit(animeId, initialEpisodeId)` → return `(defaultResult, success(true))`. This is the idempotent fast path (activity recreation).
3. `try`:
   a. `anime = getAnime.await(animeId)`.
   b. If `anime != null`:
      - `_currentAnime = anime`; `animeTitle = anime.title`.
      - `sourceManager.isInitialized.first { it }` — suspends until the source manager finished loading extensions.
      - `episodeId = initialEpisodeId` (persisted in savedState).
      - `checkTrackers(anime)`: `runBlocking { getTracks.await(anime.id) }`; `hasTrackers = tracks.isNotEmpty()`. (Blocking call on the calling thread; see §16.)
      - `updateEpisodeList(initEpisodeList(anime))` (see 2.3, 2.4).
      - `episode = currentPlaylist.first { it.id == episodeId }` — throws `NoSuchElementException` if not present (the playlist filter normally keeps the selected episode, see 2.4).
      - `source = sourceManager.getOrStub(anime.source)` (stub for uninstalled extension).
      - `_currentEpisode = episode`; `_currentSource = source`.
      - `updateEpisode(episode)` (2.5).
      - `_hasPreviousEpisode = getCurrentEpisodeIndex() != 0`.
      - `_hasNextEpisode = getCurrentEpisodeIndex() != currentPlaylist.size - 1`.
      - mpv user-data writes (for Lua scripts):
        - `user-data/current-anime/anime-title` = `anime.title` (string).
        - `user-data/current-anime/intro-length` = `getAnimeSkipIntroLength()` (int).
        - `user-data/current-anime/category` = `getAnimeCategories.await(anime.id).joinToString { it.name }` (default separator `", "`).
      - `currentEp = currentEpisode.value ?: throw ExceptionWithStringResource("No episode loaded", AYMR.strings.no_episode_loaded)`.
      - If `hostList` non-blank:
        - `currentHosterList = hostList.toHosterList()` (`SerializableHoster` JSON decode, see area:16). If empty → `currentHosterList = null` and throw `ExceptionWithStringResource("Hoster selected from empty list", AYMR.strings.select_hoster_from_empty_list)`.
        - `qualityIndex = Pair(hostIndex, vidIndex)`.
      - Else: `EpisodeLoader.getHosters(currentEp.toDomainEpisode()!!, anime, source)`; if empty → `currentHosterList = null` and throw `ExceptionWithStringResource("Hoster list is empty", AYMR.strings.no_hosters)`; else `currentHosterList = it`.
      - Return `(InitResult(hosterList=currentHosterList, videoIndex=qualityIndex, position=episodePosition), success(true))`.
   c. Else (anime null): `(defaultResult, success(false))`.
4. `catch (e: Throwable)`: `(defaultResult, failure(e))`.

Note the caller (PlayerActivity.kt:207) inspects the `Result`. `ExceptionWithStringResource` carries a `StringResource` for a user-facing message; all others are generic.

### 2.3 `initEpisodeList(anime)` (1336-1349)
1. `episodes = runBlocking { getEpisodesByAnimeId.await(anime.id) }`.
2. `.sortedWith(getEpisodeSort(anime, sortDescending = false))`. Note `sortDescending = false` is passed explicitly, so the player's order does NOT follow the anime's saved direction. Sorting key comes from `anime.sorting` (see §2.6).
3. If `basePreferences.downloadedOnly()` (key `pref_downloaded_only` via `Preference.appStateKey`, default false): `filterDownloadedEpisodes(anime)` (utility in area:13/24). Else keep.
4. `.map { it.toDbEpisode() }` — converts domain to the legacy DB model used by the player (`Episode` from `eu.kanade.tachiyomi.data.database.models.anime`).

### 2.4 `updateEpisodeList` → `filterEpisodeList(episodes)` (1146-1185)
OBSERVED algorithm:
1. `anime = currentAnime.value ?: return episodes` (unfiltered when no anime).
2. `selectedEpisode = episodes.find { it.id == episodeId } ?: error("Requested episode of id $episodeId not found in episode list")` — throws `IllegalStateException`. This propagates out of `init`'s try (→ `Result.failure`). From `loadEpisode` it is not reached (see 2.8).
3. `episodesForPlayer = episodes.filterNot { ... }` where the predicate is the OR of six clauses (exact code order):
   - `anime.unseenFilterRaw == EPISODE_SHOW_SEEN && !it.seen`
   - `anime.unseenFilterRaw == EPISODE_SHOW_UNSEEN && it.seen`
   - `anime.downloadedFilterRaw == EPISODE_SHOW_DOWNLOADED && !downloadManager.isEpisodeDownloaded(name, scanlator, anime.title, anime.source)`
   - `anime.downloadedFilterRaw == EPISODE_SHOW_NOT_DOWNLOADED && downloadManager.isEpisodeDownloaded(name, scanlator, anime.title, anime.source)`
   - `anime.bookmarkedFilterRaw == EPISODE_SHOW_BOOKMARKED && !it.bookmark`
   - `anime.bookmarkedFilterRaw == EPISODE_SHOW_NOT_BOOKMARKED && it.bookmark`
   - `anime.fillermarkedFilterRaw == EPISODE_SHOW_FILLERMARKED && !it.fillermark`
   - `anime.fillermarkedFilterRaw == EPISODE_SHOW_NOT_FILLERMARKED && it.fillermark`
   (Kotlin `&&` binds tighter than `||`, so each clause is a conjunction.)
   Filter raw values come from the anime row's `episode_flags` bits (see data file §4.2). `unseenFilterRaw = episodeFlags and EPISODE_UNSEEN_MASK (0x6)`; `downloadedFilterRaw = episodeFlags and 0x18`; `bookmarkedFilterRaw = and 0x60`; `fillermarkedFilterRaw = and 0x180` (see `Anime.kt`, area:07).
4. `.toMutableList()`.
5. If no episode in the filtered list has `id == episodeId`: append `selectedEpisode` at the end. (The selected episode is always kept so playback can continue even if the filter would hide it.)
6. Return.

Filter-value meaning: `EPISODE_SHOW_UNSEEN=0x2`, `EPISODE_SHOW_SEEN=0x4`, `EPISODE_SHOW_DOWNLOADED=0x8`, `EPISODE_SHOW_NOT_DOWNLOADED=0x10`, `EPISODE_SHOW_BOOKMARKED=0x20`, `EPISODE_SHOW_NOT_BOOKMARKED=0x40`, `EPISODE_SHOW_FILLERMARKED=0x80`, `EPISODE_SHOW_NOT_FILLERMARKED=0x100`. Value 0 = show all.

Proposal: the same predicate, written as a pure Rust function `fn player_visible(ep: &Episode, flags: u64, is_downloaded: impl Fn(&Episode)->bool) -> bool` plus the always-keep-selected rule (T-45b-005).

### 2.5 `updateEpisode(episode)` (1330-1334)
`mediaTitle = episode.name`; `_isEpisodeOnline = isEpisodeOnline() == true`; mpv `user-data/current-anime/episode-number` = `episode.episode_number.toDouble()`.

### 2.6 Episode sorting used for the playlist (domain code, quoted for exactness)
From `domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt:7-30`, `getEpisodeSort(anime, sortDescending)`:
- `EPISODE_SORTING_SOURCE (0x0)`: `sortDescending=true` → `e1.sourceOrder.compareTo(e2.sourceOrder)`; `false` → `e2.sourceOrder.compareTo(e1.sourceOrder)`. **Observed quirk:** with `sortDescending=false` (used by the player) source order is REVERSED relative to the name, so the player shows the source list in descending source order.
- `EPISODE_SORTING_NUMBER (0x200)`: true → `e2.episodeNumber.compareTo(e1.episodeNumber)`; false → `e1.episodeNumber.compareTo(e2.episodeNumber)`.
- `EPISODE_SORTING_UPLOAD_DATE (0x400)`: true → `e2.dateUpload.compareTo(e1.dateUpload)`; false → ascending `dateUpload`.
- `EPISODE_SORTING_ALPHABET (0x600)`: true → `e2.name.compareToWithCollator(e1.name)`; false → `e1.name.compareToWithCollator(e2.name)`.
- Any other value → `NotImplementedError("Invalid episode sorting method: ...")`.
- `Anime.sorting` = `episodeFlags and EPISODE_SORTING_MASK (0x600)`.

Proposal: replicate exactly, including the SOURCE inversion, and mark it with a `// compat:` comment and a settings-level override if the user complains (T-45b-004 acceptance).

### 2.7 `getCurrentEpisodeIndex()` (1187-1189) and `getAdjacentEpisodeId(previous)` (1191-1199)
- `getCurrentEpisodeIndex() = currentPlaylist.indexOfFirst { currentEpisode.id == it.id }` → -1 if not found.
- `getAdjacentEpisodeId(previous)`:
  - `newIndex = previous ? index-1 : index+1` (using the current index, which may be -1).
  - If `previous && index == 0` → `-1L`.
  - Else if `!previous && lastIndex == index` → `-1L`.
  - Else `playlist.getOrNull(newIndex)?.id ?: -1L`.
  - Note: when `index == -1` and `previous=false`: `lastIndex == -1` only if list is empty; otherwise `getOrNull(0)` returns the first episode.

### 2.8 `changeEpisode(previous, autoPlay = false)` (1034-1049)
1. If `previous && !hasPreviousEpisode.value` → toast `AYMR.strings.no_prev_episode`; return.
2. If `!previous && !hasNextEpisode.value` → toast `AYMR.strings.no_next_episode`; return.
3. `activity.changeEpisode(episodeId = getAdjacentEpisodeId(previous), autoPlay = autoPlay)` (part 1 PlayerActivity).

### 2.9 `loadEpisode(episodeId: Long?): EpisodeLoadResult?` (1621-1652)
1. `anime = currentAnime.value ?: return null`; `source = sourceManager.getOrStub(anime.source)`.
2. `chosenEpisode = currentPlaylist.firstOrNull { it.id == episodeId } ?: return null`.
3. `_currentEpisode = chosenEpisode`; `updateEpisode(chosenEpisode)`.
4. In `withIOContext`:
   - `try`: `currentHosterList = EpisodeLoader.getHosters(currentEpisode.toDomainEpisode()!!, anime, source)`; then `episodeId = currentEpisode.id!!`.
   - `catch (Exception)`: log ERROR "Error getting links". Note: on failure `currentHosterList` is left as the previous episode's list (it is NOT reset). Observed bug; see E-12.
   - Return `EpisodeLoadResult(currentHosterList, episodeTitle = anime.title + " - " + chosenEpisode.name, source)`.

### 2.10 `resetState()` (1023-1032)
Sets `_pausedState = false`; clears `_hosterState`, `_hosterList`, `_hosterExpandedList`; `_selectedHosterVideoIndex = (-1,-1)`; `thumbnailTileCache.clear()`; cancels `thumbnailFetchJob`; `lastThumbnailFetch = 0`. Note: it does NOT clear `_currentVideo`, `_chapters`, `_subtitleTracks`, or `_audioTracks`; those are overwritten by mpv events on the next file load.

### 2.11 Other episode-state setters
- `updateHasNextEpisode(v)`, `updateHasPreviousEpisode(v)`: direct sets.
- `showEpisodeListDialog()`: if `currentAnime != null` → `showDialog(Dialogs.EpisodeList)`.
- `onSaveInstanceStateNonConfigurationChange()` (1219-1224): if `currentEpisode` non-null → `viewModelScope.launchNonCancellable { saveEpisodeProgress(currentEpisode) }`. Note: saves progress only (not history).

## 3. Hoster and video resolution

### 3.1 `EpisodeLoader.getHosters(episode, anime, source)` (EpisodeLoader.kt:33-41)
Dispatch order (first match wins):
1. `isDownload(episode, anime)` → `getHostersOnDownloaded(episode, anime, source)`.
2. `source is AnimeHttpSource` → `getHostersOnHttp(episode, source)`.
3. `source is LocalAnimeSource` → `getHostersOnLocal(episode)`.
4. else `error("source not supported")` (IllegalStateException).

### 3.2 `EpisodeLoader.isDownload(episode, anime)` (49-58)
`AnimeDownloadManager.isEpisodeDownloaded(episode.name, episode.scanlator, anime.title, anime.source, skipCache = true)`. `skipCache = true` means a fresh disk check (no in-memory cache). Keyed by name, scanlator, title, and source id.

### 3.3 `getHostersOnHttp(episode, source)` (86-96)
1. `checkHasHosters(source)`:
   - Walks `source.javaClass` up the superclass chain. If it reaches `ParsedAnimeHttpSource`, `AnimeHttpSource`, or `AnimeSource` → return false.
   - If `current.declaredMethods` contains a method named `getHosterList`, `hosterListRequest`, or `hosterListParse` → return true.
   - Else `current = current.superclass ?: return false`.
2. If true: `source.getHosterList(episode.toSEpisode())` then `source.run { it.sortHosters() }`.
3. Else (legacy extension library < 1.6): `source.getVideoList(episode.toSEpisode())` → `source.run { it.sortVideos() }` → `.toHosterList()`.
   - Proposal (Rust): the reflection check becomes a trait method: `fn has_hoster_list(&self) -> bool { false }` overridden by sources that implement hoster listing. Behaviour equals the Kotlin for all compiled-in sources.

### 3.4 `getHostersOnDownloaded(episode, anime, source)` (105-117)
`video = downloadManager.buildVideo(source, anime, episode)`; return `listOf(video).toHosterList()`. Any `Throwable` → `emptyList()` (silent).

### 3.5 `getHostersOnLocal(episode)` (124-143)
1. `(animeDirName, episodeName) = episode.url.split('/', limit = 2)`.
2. `fileSystem = Injekt.get<LocalAnimeSourceFileSystem>()`; `videoFile = fileSystem.getBaseDirectory()?.findFile(animeDirName)?.findFile(episodeName)`.
3. `videoUri = videoFile!!.uri` (NPE if missing).
4. `Video(videoUri.toString(), "Local source: ${episode.url}")` — note argument order: first arg is `url` (page URL), second is `quality` → the legacy constructor sets `videoUrl = videoUri` (since `videoUrl` param is passed as `null`? No: legacy constructor is `(url, quality, videoUrl: String?, headers, ...)`. The call passes only two arguments, so it resolves to the `(videoUrl: String = "", videoTitle: String = "", ...)` primary constructor. Therefore `videoUrl = videoUri.toString()` and `videoTitle = "Local source: <episode.url>"`. OBSERVED.
5. Return `listOf(video).toHosterList()`. Any `Exception` → `emptyList()`.

### 3.6 `getVideos(source, hoster)` (153-166) and `getVideosOnHttp` (174-177)
- If `hoster.videoList != null && source is AnimeHttpSource` → `videoList.parseVideoUrls(source)`.
- Else if `hoster.videoList != null` → `hoster.videoList!!`.
- Else if `source is AnimeHttpSource` → `getVideosOnHttp(source, hoster)` = `source.getVideoList(hoster).parseVideoUrls(source)`.
- Else `error("source not supported")`.
- Final: if `source is AnimeHttpSource` → `source.run { videos.sortVideos() }`; else unchanged.

### 3.7 `parseVideoUrls` (180-187)
For each video: if `video.videoUrl != "null"` keep; else `video.copy(videoUrl = source.getVideoUrl(video))`. The literal string `"null"` is the sentinel meaning "URL must be resolved lazily". It is produced by the legacy `Video(url, quality, videoUrl = null)` constructor (`source-api/.../Video.kt:69`).

### 3.8 `loadHosterVideos(source, hoster, force = false): HosterState` (189-204)
1. If `!force && hoster.lazy` → `HosterState.Idle(hoster.hosterName)`.
2. `try`: `videos = getVideos(source, hoster)`; return `HosterState.Ready(name, videos, List(videos.size) { Video.State.QUEUE })`.
3. `catch (Exception)`: rethrow `CancellationException`; else `HosterState.Error(hoster.hosterName)`.

### 3.9 `HosterLoader.selectBestVideo(hosterState: List<HosterState>): Pair<Int,Int>` (27-67)
Input filtered to `Ready` states, retaining original indices.
1. Preferred pass: `isPreferred(v, s) = v.preferred && (s == READY || s == QUEUE)`. Find the first hoster (in original order) containing any preferred video; return `(hosterIdx, indexOfFirst preferred within that hoster)`.
2. First-valid pass: `firstValid(v, s) = v.videoUrl.isNotEmpty() && (s == READY || s == QUEUE)`. Same search.
3. Else `(-1, -1)`.
Note: `ERROR` and `LOAD_VIDEO` states are excluded from selection. `videoUrl == "null"` (unresolved) is non-empty, so it is selectable and will be resolved when loaded.

### 3.10 `HosterLoader.getResolvedVideo(source, video)` (152-168)
1. If `source is AnimeHttpSource && !video.initialized`: `source.resolveVideo(video)`; on `Exception` (not Cancellation) → `null`.
2. Else `video` itself.
3. Return `resolvedVideo?.copy(initialized = true)`.
Note: for non-HTTP sources `initialized` is set true on the returned copy, even when it was false.

### 3.11 `HosterLoader.getBestVideo(source, hosterList)` (78-150) — used by the external-player path (`ExternalIntents.kt:90`)
Purpose: pick and resolve one playable video, racing hosters in parallel.

Algorithm:
1. `hosterStates = MutableList(n) { Idle("") }`.
2. `withContext(Dispatchers.IO)`:
   a. For every hoster index in parallel (`async`):
      - `state = loadHosterVideos(source, hoster)` (not forced; lazy hosters yield Idle); store.
      - If Ready: `prefIndex = videoList.indexOfFirst { it.preferred && !it.initialized }`. If found:
        - mark that video `LOAD_VIDEO`; `resolved = getResolvedVideo(source, video)`.
        - If `resolved?.videoUrl` non-empty → `coroutineContext.cancelChildren()` and `throw EarlyReturnException(resolved)`.
        - Else mark `ERROR`.
   b. `awaitAll()`. (An `EarlyReturnException` thrown in a child propagates and cancels siblings.)
   c. Fallback loop: `(h, v) = selectBestVideo(states)`; while `h != -1`: mark `LOAD_VIDEO`; `resolved = getResolvedVideo`; if non-empty URL → cancel children and `return@withContext resolved`; else mark `ERROR`; reselect.
   d. `cancelChildren()`; return null.
3. `catch (EarlyReturnException) → e.video`.
Observed quirk: step 2a uses `!it.initialized` but the fallback in 2c does not check `initialized`. Also the early-return path returns the resolved video, while a failure in 2c returns null and the caller throws `"Video list is empty"` (ExternalIntents.kt:92).

## 4. Loading hosters and videos into the player

### 4.1 `loadHosters(source, hosterList, hosterIndex, videoIndex)` (1366-1457)
Called from PlayerActivity with the resolved list and the saved (hosterIndex, videoIndex) (PlayerActivity.kt:220, 1037). Algorithm:
1. `hasFoundPreferredVideo = AtomicBoolean(false)`.
2. `_hosterList = hosterList`; `_hosterExpandedList = List(n) { true }` (all expanded).
3. Cancel previous `getHosterVideoLinksJob`; start `launchIO`:
   a. `_hosterState = hosterList.map { hoster -> when { hoster.lazy → Idle(name); hoster.videoList == null → Loading(name); else → Ready(name, videoList, List(size){QUEUE}) } }`.
   b. `coroutineScope { hosterList.mapIndexed { hosterIdx, hoster -> async { ... } }.awaitAll(); <post-step> }`. Per-hoster body:
      - `state = EpisodeLoader.loadHosterVideos(source, hoster)` (not forced); `_hosterState[hosterIdx] = state`.
      - If Ready:
        - If `hosterIdx == hosterIndex` (the saved hoster): `videoList.getOrNull(videoIndex)?.let { hasFoundPreferredVideo = true; success = loadVideo(source, it, hosterIndex, videoIndex); if (!success) hasFoundPreferredVideo = false }`.
        - `prefIndex = videoList.indexOfFirst { it.preferred }`. If `prefIndex != -1 && hosterIndex == -1` (no saved selection): if `hasFoundPreferredVideo.compareAndSet(false, true)` and `selectedHosterVideoIndex == (-1,-1)`: `loadVideo(source, videoList[prefIndex], hosterIdx, prefIndex)`; on failure reset flag to false.
   c. After all hosters: if `hasFoundPreferredVideo.compareAndSet(false, true)` and `selectedHosterVideoIndex == (-1,-1)`: `(h, v) = selectBestVideo(_hosterState.value)`; if `h == -1` → throw `ExceptionWithStringResource("No available videos", AYMR.strings.no_available_videos)`; else `loadVideo(source, videoList[v], h, v)`.
4. `catch (CancellationException)`: `_hosterState = hosterList.map { Idle(name) }`; rethrow.

Observed subtleties:
- Step 3b-1 uses the saved `hosterIndex`/`videoIndex` only if that hoster resolves to Ready and the video index exists.
- The `compareAndSet(false, true)` pattern means the first successful load wins; later hosters only load if the flag was reset to false by a failed load.
- Step 3c throws `No available videos` (surfaced as an error screen by the activity) only when no video is selected at all and no preferred one was loaded.

### 4.2 `loadVideo(source, video, hosterIndex, videoIndex): Boolean` (1459-1526)
1. `selectedHosterState = _hosterState[hosterIndex] as? Ready ?: return false`.
2. `updateIsLoadingEpisode(true)`.
3. `oldSelectedIndex = _selectedHosterVideoIndex`; set it to `(hosterIndex, videoIndex)`.
4. `_hosterState[hosterIndex] = selectedHosterState.getChangedAt(videoIndex, video, LOAD_VIDEO)`.
5. `updatePausedState()` (captures current `paused` into `_pausedState` only if `_pausedState` is null) then `pause()` (mpv paused = true, `_paused = true`, PiP params refresh).
6. `resolvedVideo = if (selectedHosterState.videoState[videoIndex] != READY) HosterLoader.getResolvedVideo(source, video) else video`.
   Note: the comparison uses the state captured before step 4.
7. If `resolvedVideo == null || resolvedVideo.videoUrl.isEmpty()`:
   - If `currentVideo.value == null` (nothing playing yet):
     - Mark `ERROR` on the failed video.
     - `(nh, nv) = selectBestVideo(_hosterState.value)`.
     - If `nh == -1`: if any hoster state is `Loading` → `_selectedHosterVideoIndex = (-1,-1)`; return false. Else throw `ExceptionWithStringResource("No available videos", AYMR.strings.no_available_videos)`.
     - Else recurse `loadVideo(source, newVideo, nh, nv)`.
   - Else (something already playing):
     - `_selectedHosterVideoIndex = oldSelectedIndex`; mark the failed video `ERROR`; return false.
8. Else: mark `READY` with `resolvedVideo`; `_currentVideo = resolvedVideo`; `qualityIndex = (hosterIndex, videoIndex)` (persisted); `viewModelScope.launchIO { loadThumbnails(resolvedVideo, source) }`; `activity.setVideo(resolvedVideo)` (part 1 loads it into mpv). Return true.

Termination: each recursive failure marks one `(hoster, video)` as ERROR, so `selectBestVideo` eventually returns -1.

### 4.3 `onVideoClicked(hosterIndex, videoIndex)` (1532-1556)
1. `hosterState = _hosterState[hosterIndex] as? Ready`; `video = videoList.getOrNull(videoIndex) ?: return`.
2. `videoState = hosterState.videoState.getOrNull(videoIndex) ?: return`.
3. If `videoState == ERROR` → return (silently ignored).
4. `launchIO`: `success = loadVideo(currentSource.value, video, hosterIndex, videoIndex)`. If success and `sheetShown == QualityTracks` → `dismissSheet()`. If not success → `updateIsLoadingEpisode(false)`.

### 4.4 `onHosterClicked(index)` (1558-1578)
- `Ready` → toggle `_hosterExpandedList[index]` (the accordion).
- `Idle` → set `Loading(name)`; `launchIO`: `state = loadHosterVideos(currentSource!!, hosterList[index], force = true)`; `_hosterState[index] = state`.
- `Loading`, `Error` → no-op. **Observed:** an `Error` hoster cannot be retried from here.

### 4.5 `cancelHosterVideoLinksJob()` (1359-1361)
Cancels the `loadHosters` job. Caller: part 1 when the episode changes.

### 4.6 `updateVideo(video)` (1528-1530)
`_currentVideo = video`. Caller: PlayerActivity.kt:1123 (external update).

### 4.7 `loadThumbnails(video, source)` (1580-1605)
1. Only for `source is AnimeHttpSource`.
2. `thumbInfo = source.getVideoThumbnails(video)`; if non-null:
   - `thumbnailInfo = ThumbnailInfo(tileInfo = thumbInfo.tileInfo.sortedBy { it.timeMs }, imageTileUrls = thumbInfo.imageTileUrls)`.
   - Preload the first 2 tile images: `imageTileUrls.take(2).forEachIndexed { index, url -> bitmap = source.getImageTile(url); if (bitmap != null) thumbnailTileCache[index] = bitmap }`. Note: cache key is the list index, which equals `imageIndex` only for these first two tiles.
3. `catch (Exception)`: rethrow Cancellation; log ERROR "Failed to fetch thumbnails".

## 5. Audio and subtitle tracks

### 5.1 `loadTracks()` (419-449)
1. Cancel previous `trackLoadingJob`; `launch` (Main dispatcher):
2. `possibleTrackTypes = ["audio", "sub"]`; `subTracks = []`; `audioTracks = [VideoTrack(-1, "Off", null)]` (the "Off" entry is `activity.stringResource(MR.strings.off)` = "Off").
3. `try`: `count = getPropertyInt("track-list/count") ?: 0`; for `i in 0..<count`:
   - `type = getTrackType(i)`; skip if not in list or null.
   - `"sub"` → `subTracks += VideoTrack(getTrackMPVId(i), getTrackTitle(i), getTrackLanguage(i))`.
   - `"audio"` → `audioTracks += VideoTrack(...)`.
   - `else -> error("Unrecognized track type")` (unreachable).
4. `catch (NullPointerException)`: log "Couldn't load tracks, probably cause mpv was destroyed"; return (state unchanged).
5. `_subtitleTracks = subTracks`; `_audioTracks = audioTracks`.
6. If `!isLoadingTracks.value` → `onFinishLoadingTracks()`.

Accessor semantics:
- `getTrackLanguage(it)`: `it != -1` → `getPropertyString("track-list/$it/lang") ?: ""`; `-1` → "Off".
- `getTrackTitle(it)`: `track-list/$it/title` or "" ; `-1` → "Off".
- `getTrackMPVId(it)`: `track-list/$it/id` (Int?); `-1` → -1.
- `getTrackType(it)`: `track-list/$it/type` (no default).

### 5.2 `onFinishLoadingTracks()` (455-470)
1. `preferredSubtitle = trackSelect.getPreferredTrackIndex(subtitleTracks.value)`.
   `(preferredSubtitle ?: subtitleTracks.firstOrNull())?.let { player.sid = it.id; player.secondarySid = -1 }`.
   Note: if no preferred match, the FIRST subtitle track is selected (not "Off"). When there are no subtitle tracks the list is empty and nothing changes.
2. `preferredAudio = trackSelect.getPreferredTrackIndex(audioTracks.value, subtitle = false)`.
   `(preferredAudio ?: audioTracks.getOrNull(1))?.let { player.aid = it.id }`. Index 1 = first real audio track (index 0 is "Off").
3. `isLoadingTracks = true`; `updateIsLoadingEpisode(false)`; `setPausedState()` (applies the pending pause/unpause captured in `updatePausedState`).

### 5.3 `TrackSelect.getPreferredTrackIndex(tracks, subtitle = true): VideoTrack?` (TrackSelect.kt:238-276)
Inputs by mode:
- Subtitle mode (`subtitle = true`): `prefLangs = pref_subtitle_lang`; `whitelist = pref_subtitle_whitelist`; `blacklist = pref_subtitle_blacklist`.
- Audio mode: `prefLangs = pref_audio_lang`; whitelist and blacklist are the empty string (not read).
- Each string: `.split(",").filter(String::isNotEmpty).map(String::trim)`. Note: `"  "` (spaces only) is NOT filtered out before trimming, so it becomes an empty-string token. Empty-string tokens match everything in `contains("", true)` = true. Observed bug: a pref value like `"en, "` yields an empty token and blacklist entries of `""` exclude every track. See E-21.

Algorithm:
1. `locales = prefLangs.map(::Locale)`; if empty → `listOf(LocaleListCompat.getDefault()[0]!!)` (system default locale). `Locale(String)` is a Java constructor (no validation).
2. `chosenLocale = locales.firstOrNull { loc -> tracks.any { containsLang(it, loc) } } ?: return null`. (The first preferred language that matches at least one track, in preference order.)
3. `filtered = tracks.withIndex().filterNot { blacklist.any { name.contains(it, true) } }.filter { containsLang(track, chosenLocale) }`.
4. Return `filtered.firstOrNull { whitelist.any { name.contains(it, true) } }?.value ?: filtered.getOrNull(0)?.value`.
   So: whitelist match wins; else first remaining track in the chosen language; else null.

`containsLang(track, locale)` (278-286):
- `localName = locale.getDisplayName(locale)` (e.g. "Deutsch" for de).
- `englishName = locale.getDisplayName(Locale.ENGLISH).substringBefore(" (")`.
- `langRegex = Regex("""\b${locale.isO3Language}|${locale.language}\b""", IGNORE_CASE)`.
- Return `track.name.contains(localName, true) || track.name.contains(englishName, true) || track.language?.let { langRegex.find(it) != null } == true`.

**Observed regex bug:** the alternation is not grouped. The pattern is `\b<iso3>` OR `<iso2>\b`. For `locale = en` (iso3 `eng`), the pattern matches `eng` at a word start anywhere in `language`, OR `en` at a word end anywhere. So a language tag such as `"xen"` matches via `en\b`. Preserve for compatibility or fix (decision recorded in E-22; default proposal: anchor both alternatives, `^(?:<iso3>|<iso2>)$` on the normalised tag).

`locale.isO3Language` can throw `MissingResourceException` for locales with no ISO-639-2 code (not caught). Proposal: catch and fall back to the 2-letter code.

### 5.4 `addAudio(uri)` (510-521) and `addSubtitle(uri)` (531-542)
1. `url = uri.toString()`; `isContentUri = url.startsWith("content://")`.
2. `path = if (isContentUri) uri.openContentFd(activity) else url`; if null → return.
3. `name = if (isContentUri) uri.getFileName(activity) else null`.
4. `MPVLib.command(arrayOf("audio-add" | "sub-add", path, "cached"[, name]))`. Title argument is included only when the name is non-null.
Linux proposal: the file chooser yields a path; always pass `path` and `"cached"`, with title = file name (T-45b-072).

### 5.5 `selectAudio(id)` (523-525), `updateAudio(id)` (527-529), `updateSubtitle(sid, secondarySid)` (563-565)
- `selectAudio`: `player.aid = id` (mpv write).
- `updateAudio`: `_selectedAudio = id` (state only; the mpv observer `aid` in part 1 is the source of truth).
- `updateSubtitle`: `_selectedSubtitles = (sid, secondarySid)`.

### 5.6 `selectSub(id)` (544-561)
Current pair `(p, s) = selectedSubtitles`. New value:
- `id == p` → `(s, -1)` (primary removed; secondary is promoted to primary).
- `id == s` → `(p, -1)` (secondary removed).
- else if `p != -1` → `(p, id)` (set as secondary).
- else → `(id, -1)` (set as primary).
Then `player.secondarySid = pair.second`, `player.sid = pair.first`.
Note: when `p == -1` and `s == -1` and `id == -1`, `id == p` → `(s, -1)` = `(-1, -1)`; harmless.

## 6. Chapters, AniSkip and the skip-intro state machine

### 6.1 `loadChapters()` (479-494)
`count = getPropertyInt("chapter-list/count")!!` (NPE if absent; caller only calls after chapter-list observed). For `i in 0 until count`: `title = chapter-list/$i/title` (nullable String, passed as `name`), `time = chapter-list/$i/time` (`!!`, seconds as Int, converted to Float `start`). Builds `IndexedSegment(name = title, start = time.toFloat(), index = 0)`, then `updateChapters(list.sortedBy { it.start })`.
Note: `name` may be null here and `IndexedSegment.name` is a non-null String → Kotlin would reject; the platform type from `MPVLib` makes it compile. Proposal: empty string for missing titles.

### 6.2 `updateChapters(chapters)` (496-498), `selectChapter(index)` (500-503)
`selectChapter` → `seekTo(chapters[index].start.toInt())` with the default `precise = true`.

### 6.3 `updateChapter(index: Long)` (505-508)
If `chapters` empty or `index == -1L` → return. Else `_currentChapter = chapters.getOrNull(index.toInt()) ?: return`. Called with `0` after chapter-list (PlayerActivity.kt:686).

### 6.4 `ChapterUtils.mergeChapters(currentChapters, stamps, duration)` (ChapterUtils.kt:138-220)
Inputs: mpv chapters (`currentChapters`, index 0 by default); AniSkip (or extension) `TimeStamp`s; `duration` in whole seconds (`player.duration`, may be null).

Algorithm (exact order):
1. `sortedStamps = stamps.sortedBy { it.start }`.
2. `chapters = sortedStamps.mapIndexed { i, it -> ... }.flatten()`: for each stamp:
   - `startTime = if (i == 0 && it.start < 1.0) 0.0 else it.start`.
   - `startChapter = IndexedSegment(index = -2, name = it.name, start = startTime.toFloat(), color = if (type == Other) Color.Unspecified else Color(0xFFD8BBDF), chapterType = it.type)`.
   - `nextStart = sortedStamps.getOrNull(i+1)?.start`.
   - `isNotLastChapter = abs(it.end - (duration?.toDouble() ?: -2.0)) > 1.0`.
   - `isNotAdjacent = nextStart == null || abs(it.end - nextStart) > 1.0`.
   - If both → also emit `endChapter = IndexedSegment(index = -1, name = "", start = it.end.toFloat())`; else only `startChapter`.
   Note: when `duration` is null, `-2.0` makes `isNotLastChapter` true for any `end` (|end+2| > 1 for end >= 0).
3. `playerChapters = currentChapters.filter { pc -> chapters.none { c -> abs(c.start - pc.start) < 1.0 && c.index == -2 } }.map { IndexedSegment(name, start, color, chapterType) }.sortedBy { start }.mapIndexed { i, it -> if (i == 0 && start < 1.0) IndexedSegment(name, 0.0f, index = it.index, chapterType) else it }`.
   (Mpv chapters are dropped when an external stamp starts within 1 s of them.)
4. `filteredChapters = chapters.filter { c -> playerChapters.none { pc -> abs(c.start - pc.start) < 1.0 && c.index != -2 } }`.
   (Index -2 is an external start; index -1 is an external end marker.)
5. `startChapter = if ((playerChapters + filteredChapters).isNotEmpty() && playerChapters.none { start == 0.0f } && filteredChapters.none { start == 0.0f }) listOf(IndexedSegment(index=-1, name="", start=0.0f)) else emptyList()`.
6. `combined = (startChapter + playerChapters + filteredChapters).sortedBy { start }` (stable sort).
7. Remove adjacent empty-name duplicates: keep index 0; for `index > 0` drop segment when `segment.name.isEmpty() && combined[index-1].name.isEmpty()` (compares against the ORIGINAL previous element, not the last kept one).
8. Return.

Observed: step 3 keeps the `index` of player chapters only in the first-element rewrite; other player chapters get default `index = 0` (from the constructor default). Step 7 is applied to `combined` (pre-filter neighbours).

### 6.5 `aniSkipResponse(playerDuration: Int?): List<TimeStamp>?` (PlayerViewModel.kt:2031-2054)
Observed algorithm (note the loop bug):
1. `animeId = currentAnime.id ?: return null`; `episodeNumber = currentEpisode.episode_number.toInt() ?: return null` (truncates; 12.5 → 12).
2. `if (getTracks.await(animeId).isEmpty()) { log "AniSkip: No tracks found for anime $animeId"; return null }`.
3. `getTracks.await(animeId).map { track -> ... }`:
   - `tracker = trackerManager.get(track.trackerId)`.
   - `malId = when (tracker) { is MyAnimeList → track.remoteId; is Anilist → AniSkipApi().getMalIdFromAL(track.remoteId); else → null }`.
   - `duration = playerDuration ?: return null`.
   - `return malId?.let { AniSkipApi().getResult(it.toInt(), episodeNumber, duration.toLong()) }`.
   The `return` inside the lambda is a NON-LOCAL return from `aniSkipResponse` (the `map` is inline). Therefore only the FIRST track is ever evaluated; the function returns on the first iteration with that track's result (or null if its tracker is not MAL/AniList).
4. After the loop (unreachable in practice): `return null`.

Consequences (OBSERVED, see E-03):
- A Kitsu/Shikimori/Bangumi first track yields null even when a MAL track exists second.
- AniList first track: `getMalIdFromAL` returns `0` on failure (not null), so `getResult(0, …)` is still called and will return null or garbage from AniSkip.
- `getTracks` is queried twice.

Proposal (T-45b-021): choose the first track whose tracker maps to a MAL id (MAL remote id, or AniList→MAL lookup), skip `0`, and query AniSkip once. This is a behaviour change and is recorded as an intentional fix.

### 6.6 AniSkip client (AniSkipApi.kt)
`getResult(malId: Int, episodeNumber: Int, episodeLength: Long): List<TimeStamp>?` (22-44):
1. URL: `https://api.aniskip.com/v2/skip-times/$malId/$episodeNumber?types[]=ed&types[]=mixed-ed&types[]=mixed-op&types[]=op&types[]=recap&episodeLength=$episodeLength`.
2. `client.newCall(GET(url)).execute().body.string()`; decode `AniSkipResponse` with the app `Json` (ignoreUnknownKeys = true, explicitNulls = false; see data file §5).
3. If `res.found` → `res.results?.map { TimeStamp(start = interval.startTime, end = interval.endTime, name = skipType.getString(), type = skipType.toChapterType()) }`; else null.
4. Any `Exception` → null.

Consequences:
- The request asks for `mixed-ed`, but `SkipType` has no `mixed-ed` constant. The Json has `ignoreUnknownKeys` only; enum constants are NOT coerced. If the API returns `skipType: "mixed-ed"` the whole decode throws and the result is null (E-04). Analysis; confirm against live API before the port (T-45b-020 acceptance).
- `body.string()` is never closed (resource leak in Kotlin, E-05).

`SkipType` mapping:
- `"op"` → `OP` → name `"Opening"` → `ChapterType.Opening`.
- `"ed"` → `ED` → `"Ending"` → `ChapterType.Ending`.
- `"recap"` → `RECAP` → `"Recap"` → `ChapterType.Recap`.
- `"mixed-op"` → `MIXED_OP` → `"Mixed-op"` → `ChapterType.MixedOp`.

`getMalIdFromAL(id: Long): Long` (46-65):
1. POST `https://graphql.anilist.co` with JSON body `{"query": "query{\n Media(id:$id){idMal}\n }"}` (the query text is produced with `trimMargin()`; the literal `id` is interpolated, not a GraphQL variable).
2. Exception → `0`.
3. `response.body.string().substringAfter("idMal\":").substringBefore("}").toLongOrNull() ?: 0`.
Observed: this is string parsing, not JSON. For `{"data":{"Media":{"idMal":null}}}` the substring is `null` → 0. For a number it works. Proposal: parse with serde (`data.Media.idMal`), returning `Option<u64>`.

### 6.7 `ChapterType.getStringRes()` (ChapterUtils.kt:130-136)
Opening → `AYMR.strings.player_chapter_type_opening`; Ending → `player_chapter_type_ending`; Recap → `player_chapter_type_recap`; MixedOp → `player_chapter_type_mixedop`; Other → null.

### 6.8 Skip-intro state machine (`setChapter`, `onSkipIntro`, `showSkipIntroButton`, `updateSkipIntroButton`)

Inputs read at construction (per ViewModel instance):
- `introSkipEnabled = enableSkipIntro()` (default true).
- `autoSkip = autoSkipIntro()` (default false).
- `netflixStyle = enableNetflixStyleIntroSkip()` (default false).
- `defaultWaitingTime = waitingTimeIntroSkip()` (default 5).
- `waitingSkipIntro` (var) = `defaultWaitingTime`; PlayerActivity re-sets it at 1290 (`viewModel.waitingSkipIntro = playerPreferences.waitingTimeIntroSkip().get()`).

`setChapter(position: Float)` (2063-2102), called on every whole-second `time-pos` event and once after AniSkip/loadTracks merges:
1. `(chapterIndex, chapter) = getCurrentChapter(position)`; if null → return.
2. If `currentChapter.value != chapter` → `_currentChapter = chapter`.
3. If `!introSkipEnabled` → return.
4. If `chapter.chapterType == Other`: `_skipIntroText = null`; `waitingSkipIntro = defaultWaitingTime`. Return path ends here.
5. Else `nextChapterPos = chapters.getOrNull(chapterIndex + 1)?.start ?: pos.value`.
   - If `netflixStyle`:
     - If `waitingSkipIntro == defaultWaitingTime` → toast `"Skip Intro: " + stringResource(AYMR.strings.player_aniskip_dontskip_toast, chapter.name, waitingSkipIntro)`. (The literal prefix "Skip Intro: " is hard-coded English, not a string resource.)
     - `showSkipIntroButton(chapter, nextChapterPos, waitingSkipIntro)`.
     - `waitingSkipIntro--` (decrements on EVERY call, i.e. once per whole second of playback while in an intro chapter).
   - Else if `autoSkip` → `seekToWithText(seekValue = nextChapterPos.toInt(), text = stringResource(AYMR.strings.player_intro_skipped, chapter.name))`.
   - Else → `updateSkipIntroButton(chapter.chapterType)`.

`showSkipIntroButton(chapter, nextChapterPos, waitingTime)` (2117-2131):
- If `waitingTime > -1`:
  - If `waitingTime > 0` → `_skipIntroText = stringResource(AYMR.strings.player_aniskip_dontskip)`.
  - Else (`== 0`) → `seekToWithText(nextChapterPos.toInt(), stringResource(AYMR.strings.player_aniskip_skip, chapter.name))`.
- Else (`-1`, user cancelled) → `updateSkipIntroButton(chapter.chapterType)`.

Countdown timeline (netflix style, default 5): call k (k=0..) sees waiting = 5-k. Calls with 5,4,3,2,1 show the "don't skip" text. At 0 it seeks. Because `waitingSkipIntro--` runs after `showSkipIntroButton`, the seek happens on the call where the value read is 0, which is the 6th call (k=5). With the per-second cadence that is 5 seconds after the chapter starts (first call at t=0 reads 5, call at t=1 reads 4, …, call at t=5 reads 0 → seek). Precisely: `waiting` is read before decrement; calls at t=0..5 read 5,4,3,2,1,0.

`updateSkipIntroButton(chapterType)` (2104-2115): `label = chapterType.getStringRes()?.let { stringResource(AYMR.strings.player_skip_action, stringResource(it)) }`; `_skipIntroText = label` (null for Other, which never reaches here).

`onSkipIntro()` (2133-2148) — user taps the skip button:
1. `getCurrentChapter()` (at current pos); if null return.
2. If `waitingSkipIntro > 0 && netflixStyle` → `waitingSkipIntro = -1`; return (cancel countdown; the button then shows the normal label on the next tick).
3. Else `seekToWithText(nextChapterPos.toInt(), stringResource(AYMR.strings.player_aniskip_skip, chapter.name))` where `nextChapterPos = chapters[chapterIndex+1].start ?: pos`.

`getCurrentChapter(position: Float? = null)` (2150-2154): `chapters.withIndex().filter { start <= (position ?: pos) }.maxByOrNull { start }`. Returns the last chapter whose start is at or before the position (ties: `maxByOrNull` returns the first max element).

Observed (E-09): `waitingSkipIntro` is not reset when the chapter changes from an intro to another intro without passing through an `Other` chapter. After a countdown ends (`0` then decremented to `-1`), the next intro chapter shows only the button label (no countdown) until an `Other` chapter resets it. Proposal: reset the counter on every chapter change.

Observed (E-10): the countdown is per-call, and `setChapter` is also called from PlayerActivity at 1304 and 1383 (not per second). Those extra calls decrement the counter as well.

## 7. Progress, history, completion and downloads

### 7.1 `onSecondReached(position: Int, duration: Int)` (1658-1686) — called from `updatePlayBackPos(pos)` on every whole-second event
1. If `isLoadingEpisode` → return. If `currentEpisode` null → return. If `episodeId == -1L` → return. If `duration == 0` → return.
2. `seconds = position * 1000L`; `totalSeconds = duration * 1000L`.
3. `currentEp.last_second_seen = seconds`; `currentEp.total_seconds = totalSeconds` (in-memory object).
4. `episodePosition = seconds` (persisted in savedState, ms).
5. `progress = progressPreference()` (float, default 0.85).
6. `shouldTrack = !incognitoMode || hasTrackers`.
7. If `seconds >= totalSeconds * progress && shouldTrack` → `launchNonCancellable { updateEpisodeProgressOnComplete(currentEp) }`.
   Note: this runs on EVERY second after the threshold, not once. `updateEpisodeProgressOnComplete` is idempotent in effect but repeats DB writes and tracker calls (see E-14).
8. `saveWatchingProgress(currentEp)` (every second: progress row + history row, subject to incognito).
9. `inDownloadRange = seconds.toDouble() / totalSeconds > 0.35`; if true → `downloadNextEpisodes()`.

Note the ORDER: step 7 (completion) runs before step 8 (save), and the `currentEp.seen` flag is set inside step 7's coroutine (`currentEp.seen = true`) which happens asynchronously; step 8 may save `seen = false` on the same tick (E-14).

### 7.2 `updateEpisodeProgressOnComplete(currentEp)` (1688-1710)
1. `currentEp.seen = true`.
2. `updateTrackEpisodeSeen(currentEp)` (7.5).
3. `deleteEpisodeIfNeeded(currentEp)` (7.6).
4. `markDuplicateAsSeen = libraryPreferences.markDuplicateSeenEpisodeAsSeen().get().contains(LibraryPreferences.MARK_DUPLICATE_EPISODE_SEEN_EXISTING)`. The pref is a string set; the marker is `"existing_episode"`. Default empty set (no duplicate marking).
5. If false → return.
6. `duplicateUnseenEpisodes = currentPlaylist.mapNotNull { ep -> if (!ep.seen && ep.isRecognizedNumber && ep.episode_number == currentEp.episode_number) EpisodeUpdate(id = ep.id!!, seen = true) else null }`.
   Note: `currentPlaylist` is the FILTERED playlist, so duplicates hidden by the filter are not updated.
7. `updateEpisode.awaitAll(duplicateUnseenEpisodes)`.

`isRecognizedNumber` is an extension in the DB model file (`eu.kanade.tachiyomi.data.database.models.anime.Episode`, area:05); it is false for episodes whose number could not be parsed (value -1 semantics, owned by area:05).

### 7.3 `saveWatchingProgress(episode)` (1763-1768)
`launchNonCancellable { saveEpisodeProgress(episode); saveEpisodeHistory(episode) }`.

### 7.4 `saveEpisodeProgress(episode)` (1774-1787) and `saveEpisodeHistory(episode)` (1792-1800)
- Progress: if `!incognitoMode || hasTrackers` → `updateEpisode.await(EpisodeUpdate(id = episode.id!!, seen = episode.seen, bookmark = episode.bookmark, fillermark = episode.fillermark, lastSecondSeen = episode.last_second_seen, totalSeconds = episode.total_seconds))`.
  Note: the update writes `seen`, `bookmark`, `fillermark` from the in-memory object even when nothing changed.
- History: if `!incognitoMode` → `upsertHistory.await(AnimeHistoryUpdate(episodeId = episode.id!!, seenAt = Date()))`.

`incognitoMode` = `getIncognitoState.await(currentAnime.value?.source)`, lazily evaluated once and cached for the ViewModel lifetime (line 1111). Note: a change of anime within the same ViewModel keeps the first value (E-16).

### 7.5 `updateTrackEpisodeSeen(episode)` (1947-1957)
Return unless `basePreferences.incognitoMode()` is false AND `hasTrackers` AND `trackPreferences.autoUpdateTrack()` (key `pref_auto_update_manga_sync_key`, default true). Then `launchNonCancellable { trackEpisode.await(context, anime.id, episode.episode_number.toDouble()) }`.
Note: this uses the GLOBAL `basePreferences.incognitoMode()` (app-state key) rather than `incognitoMode` (per-source). Observed inconsistency (E-17).

### 7.6 `deleteEpisodeIfNeeded(chosenEpisode)` (1740-1754)
1. `pos = currentPlaylist.indexOf(chosenEpisode)` (identity/equality on data class).
2. `slots = downloadPreferences.removeAfterReadSlots()` (int, default -1).
3. `episodeToDelete = currentPlaylist.getOrNull(pos - slots)`.
4. `episodeToDownload = null`.
5. If `slots != -1 && episodeToDelete != null` → `enqueueDeleteSeenEpisodes(episodeToDelete)`.
Semantics: with `slots = 0` the episode being completed is itself queued for deletion. With `slots = 1` the episode before it is deleted (after it is completed, the previous one goes). With `pos - slots < 0` nothing is deleted.

`enqueueDeleteSeenEpisodes(episode)` (1963-1969): return if `!episode.seen`; `launchNonCancellable { downloadManager.enqueueEpisodesToDelete(listOf(episode.toDomainEpisode()!!), anime) }`.

`deletePendingEpisodes()` (1975-1979): `launchNonCancellable { downloadManager.deletePendingEpisodes() }`. Called on exit (PlayerActivity.kt:335, 352). Errors ignored by the download manager (area:24).

### 7.7 `downloadNextEpisodes()` (1712-1733)
1. `downloadAheadAmount == 0` → return (`downloadPreferences.autoDownloadWhileWatching()`, default 0, read once at construction).
2. `anime = currentAnime ?: return`; `currentEpisode ?: return`.
3. If `getCurrentEpisodeIndex() == currentPlaylist.lastIndex` → return (no next episode).
4. `nextEpisode = currentPlaylist[index+1]`.
5. `episodesAreDownloaded = isDownload(current) && isDownload(next)` (both must be downloaded already, "to avoid jank").
6. `launchIO`: if `!episodesAreDownloaded` return; else `episodesToDownload = getNextEpisodes.await(anime.id, nextEpisode.id!!).take(downloadAheadAmount)`; `downloadManager.downloadEpisodes(anime, episodesToDownload)`.
Observed: `onSecondReached` calls this every second once past 35 %, so `downloadEpisodes` is invoked repeatedly for the same set (E-13). The download manager is expected to de-duplicate (area:24).

### 7.8 `saveCurrentEpisodeWatchingProgress()` (1756-1758)
`currentEpisode?.let { saveWatchingProgress(it) }`. Called on pause and exit (PlayerActivity.kt:201, 326).

### 7.9 `onCleared()` (1097-1104)
If `currentEpisode != null`: `saveWatchingProgress(currentEpisode)`, then `episodeToDownload?.let { downloadManager.addDownloadsToStartOfQueue(listOf(it)) }`.
Observed: `episodeToDownload` is never assigned non-null in this file, so the second statement never runs (E-18).

### 7.10 Bookmark and fillermark (1805-1828)
`bookmarkEpisode(episodeId: Long?, bookmarked)` → `launchNonCancellable { updateEpisode.await(EpisodeUpdate(id = episodeId!!, bookmark = bookmarked)) }`. Same shape for `fillermarkEpisode`. Note: `episodeId!!` throws if null (the callers pass the current id).
These do not update the in-memory `currentEpisode.bookmark`, so `saveEpisodeProgress` on the next tick (which writes `episode.bookmark`) may overwrite the new value with the stale one (E-15). Proposal: update the in-memory episode too.

## 8. Playback primitives, seeking and gestures

### 8.1 Pause (644-663)
- `pauseUnpause()`: `if (paused) unpause() else pause()`.
- `pause()`: `player.paused = true`; `_paused = true`; `runCatching { setPictureInPictureParams(createPipParams()) }`.
- `unpause()`: `player.paused = false`; `_paused = false`.
- `updatePausedState()` (626-630): if `_pausedState == null` → `_pausedState = paused.value`.
- `setPausedState()` (632-642): if `_pausedState != null` → call `pause()` or `unpause()` per its value, then set `_pausedState = null`.

### 8.2 Seek primitives (743-750, 986-992, 1002-1018, 978-984)
- `seekBy(offset: Int, precise = false)`: `MPVLib.command(["seek", offset.toString(), precise ? "relative+exact" : "relative"])`.
- `seekTo(position: Int, precise = true)`: returns silently if `position !in 0..(player.duration ?: 0)`. Else `MPVLib.command(["seek", position.toString(), precise ? "absolute" : "absolute+keyframes"])`.
- `seekToWithText(seekValue, text)` (978-984): `_isSeekingForwards = seekValue > 0`; `_doubleTapSeekAmount = seekValue - pos.toInt()`; `_seekText = text`; `seekTo(seekValue, preciseSeek)`; `if (showSeekBar) showSeekBar()`.
- `seekByWithText(value, text)` (986-992): `_doubleTapSeekAmount = if ((value < 0 && amount < 0) || pos + value > duration) 0 else amount + value`; `_seekText = text`; `_isSeekingForwards = value > 0`; `seekBy(value, preciseSeek)`; `if (showSeekBar) showSeekBar()`.
  Observed: the amount accumulator resets to 0 when a backward seek follows a backward seek (`value < 0 && amount < 0`) — the NEW value is not added, so repeated back-taps show "0". Also resets to 0 when the target exceeds duration. Preserve or document (E-19).
- `leftSeek()` (1002-1009): `if (pos > 0) _doubleTapSeekAmount -= doubleTapToSeekDuration`; `_isSeekingForwards = false`; `seekBy(-doubleTapToSeekDuration, preciseSeek)`; `if (showSeekBar) showSeekBar()`.
- `rightSeek()` (1011-1018): `if (pos < duration) _doubleTapSeekAmount += doubleTapToSeekDuration`; `_isSeekingForwards = true`; `seekBy(+doubleTapToSeekDuration, preciseSeek)`; show seek bar if configured.
- `doubleTapToSeekDuration = gesturePreferences.skipLengthPreference()` (default 10), `preciseSeek = playerSmoothSeek()` (default false → keyframe-based relative seek is NOT used; `relative` without `+exact`), `showSeekBar = showSeekBar()` (default false). All three are snapshotted at construction (974-976), so changes need a new player session.

Precision mapping (OBSERVED, the flag strings are passed explicitly, so the Rust port passes the same strings and does not depend on mpv's default for `hr-seek`):
- `seekBy(offset, precise=false)` → `relative`; `seekBy(offset, preciseSeek)` with `playerSmoothSeek() == true` → `relative+exact`. Double-tap and skip-by paths use `preciseSeek`, so the default (smooth seek off) is the plain `relative` flag.
- `seekTo(position, precise=true)` (default) → `absolute`; `precise=false` → `absolute+keyframes`. Chapter jumps (`selectChapter`) and skip-intro seeks (`seekToWithText(..., preciseSeek)`) go through `seekTo`. Note that `seekToWithText` passes `preciseSeek`, so with smooth seek off, skip-intro seeks use `absolute+keyframes`, while `selectChapter` always uses the default `precise = true`. This asymmetry is observed; keep it and document it in the settings help text (E-25).

### 8.3 Double-tap dispatch (1051-1095)
Per side, the action is read at each tap from `gesturePreferences` (`left/center/rightDoubleTapGesture`). Defaults: left = `Seek`, center = `PlayPause`, right = `Seek`.
- Left (`handleLeftDoubleTap`): `Seek → leftSeek()`; `PlayPause → pauseUnpause()`; `Custom → MPVLib.command(["keypress", "0x10001"])`; `None → {}`; `Switch → changeEpisode(previous = true)`.
- Center (`handleCenterDoubleTap`): `PlayPause → pauseUnpause()`; `Custom → keypress "0x10002"`; `Seek → {}`; `None → {}`; `Switch → {}`. (Seek and Switch are NOT offered for center; the `when` has explicit no-ops.)
- Right (`handleRightDoubleTap`): `Seek → rightSeek()`; `PlayPause → pauseUnpause()`; `Custom → keypress "0x10003"`; `None → {}`; `Switch → changeEpisode(previous = false)`.
`CustomKeyCodes` values are defined in part 1 (`PlayerEnums.kt:72-79`).

### 8.4 Seek preview thumbnails (`updateSeekPos`, `createThumbnail`, `updateIsSeeking`) (574-620)
`updateSeekPos(pos: Float)`:
1. `_seekPosition = pos`.
2. `thumbInfo = thumbnailInfo ?: return`.
3. `info = thumbInfo.tileInfo.lastOrNull { it.timeMs <= pos * 1000L }`; if null → return.
4. If `thumbnailTileCache[info.imageIndex]` is cached → `createThumbnail(bitmap, info)` (crop, sets `_thumbnailImage`).
5. Else:
   - `now = currentTimeMillis()`; if `now - lastThumbnailFetch < 2000` → return (throttle).
   - `lastThumbnailFetch = now`; cancel previous `thumbnailFetchJob`.
   - `launchIO`: `source = currentSource as? AnimeHttpSource ?: return`; `tileUrl = thumbInfo.imageTileUrls[info.imageIndex]`; `bitmap = source.getImageTile(tileUrl)`; if non-null → on UI: `thumbnailTileCache[imageIndex] = bitmap; createThumbnail(bitmap, info)`.
   - Errors: rethrow Cancellation; log "Failed to fetch thumbnails tiles".

`createThumbnail(tile, info)` (610-613): `Bitmap.createBitmap(tile, info.x, info.y, info.width, info.height)` → `_thumbnailImage`. No bounds check (out-of-range rectangles throw in Android; Proposal: clamp).

`updateIsSeeking(value)` (615-620): `_isSeeking = value`; if `value == false` → `_thumbnailImage = null`.

Observed (E-20): because the 2-second throttle drops requests without retry, the preview for the final scrub position may never be fetched if the user stops within 2 s of the last fetch.

### 8.5 `updatePlayBackPos(pos)` (567-570) and `updateReadAhead(value)` (622-624)
`updatePlayBackPos`: `onSecondReached(pos.toInt(), duration.toInt())` then `_pos = pos`. Note the call order: progress logic first, the position flow is updated after.
`updateReadAhead(value: Long)`: `_readAhead = value.toFloat()` (seconds of demuxer cache).

### 8.6 Gesture seek amount, seek text, and speed
- `updateSeekAmount(amount)`, `updateSeekText(value)`: direct state sets.
- `gestureSeekAmount` (`Pair<Int,Int>?`): declared; not written anywhere in this file (writes occur in part 1 gesture handler).
- `playbackSpeed` (float, initialised from `player_speed` default 1f). Not updated in this file; the speed setter is in part 1 (PlayerActivity / sheets). `DoubleSpeed` in `PlayerUpdates` is emitted by part 1.

## 9. Volume, brightness, aspect, orientation, auto-play, decoder

### 9.1 Volume (`changeVolumeBy`, `changeVolumeTo`, `changeMPVVolumeTo`, `setMPVVolume`, `displayVolumeSlider`) (766-800)
`changeVolumeBy(change: Int)` (766-777):
1. `mpvVolume = getPropertyInt("volume")`.
2. If `volumeBoostCap > 0 && currentVolume == maxVolume`:
   a. If `mpvVolume == 100 && change < 0` → `changeVolumeTo(currentVolume + change)` (decreases system volume, does NOT return).
   b. `finalMPVVolume = (mpvVolume + change).coerceAtLeast(100)`.
   c. If `finalMPVVolume in 100..(volumeBoostCap + 100)` → `changeMPVVolumeTo(finalMPVVolume)`; return.
3. `changeVolumeTo(currentVolume + change)`.

Observed (E-06): step 2a and 2c both run when mpv is at 100 and the user lowers volume while at system max: system volume drops by one step AND mpv stays at 100. Keep for parity or simplify: when mpv > 100 lower mpv first; when mpv == 100 lower system.

`changeVolumeTo(volume)` (779-787): `newVolume = volume.coerceIn(0..maxVolume)`; `audioManager.setStreamVolume(STREAM_MUSIC, newVolume, 0)` (flags 0 = no UI); `currentVolume = newVolume`.
`changeMPVVolumeTo(volume)` (789-791): `setPropertyInt("volume", volume)`.
`setMPVVolume(volume)` (793-796): `if (volume != currentMPVVolume) displayVolumeSlider()`; `currentMPVVolume = volume`. (Triggered by mpv `volume` events.)
`displayVolumeSlider()` (798-800): `isVolumeSliderShown = true`.

Linux proposal: system volume through PipeWire/PulseAudio (`wpctl set-volume @DEFAULT_AUDIO_SINK@` or the `pipewire` crate, or `libpulse-binding`). Keep the boost math on mpv `volume` (0..100+cap). Volume boost cap default: `pref_audio_volume_boost_cap` = 30 (AudioPreferences.kt:29).

### 9.2 Brightness (`changeBrightnessTo`, `displayBrightnessSlider`) (752-763)
`changeBrightnessTo(brightness: Float)`: `currentBrightness = brightness.coerceIn(-0.75f, 1f)`; `window.attributes.screenBrightness = brightness.coerceIn(0f, 1f)`.
Observed: state range is -0.75..1 (negative values allowed for a dim overlay in UI), window range is 0..1. `displayBrightnessSlider()` sets `isBrightnessSliderShown = true`.
Linux (T-45b-032): there is no window brightness API. Proposal: map to mpv `brightness` video filter in -100..100 (value = round((b - 0) * 100) when b in [-0.75, 1]), and optionally to the backlight via `brightnessctl`-style sysfs writes behind a preference (P2).

### 9.3 Aspect (`changeVideoAspect`) (813-837)
| Aspect | `ratio` (`video-aspect-override`) | `pan` (`panscan`) |
|---|---|---|
| `Crop` | -1.0 | 1.0 |
| `Fit` | -1.0 | 0.0 |
| `Stretch` | displayWidthPx / displayHeightPx (real metrics of the default display) | 0.0 |
Sequence: for Fit, `panscan` 0.0 is also set explicitly before the common calls; then always `setPropertyDouble("panscan", pan)`, `setPropertyDouble("video-aspect-override", ratio)`, `aspectState().set(aspect)`, `playerUpdate = AspectRatio`.
Default `aspectState` = `Fit` (PlayerPreferences.kt:181).
Linux: the real-metrics step uses the monitor geometry of the window (GDK `Gdk::Monitor::geometry()`), not `DisplayMetrics`. Note the Android code uses `getRealMetrics` (full screen including nav bar); the Linux port uses the monitor size.

### 9.4 Orientation (`cycleScreenRotations`) (839-854) — Drop (T-45b-058)
If the current requested orientation is landscape / reverse landscape / sensor landscape → set preference `defaultPlayerOrientationType = SensorPortrait` and request `SCREEN_ORIENTATION_SENSOR_PORTRAIT`. Otherwise → `SensorLandscape` and `SCREEN_ORIENTATION_SENSOR_LANDSCAPE`. Persisted default: `SensorLandscape` (PlayerPreferences.kt:116-119).

### 9.5 Auto-play (`setAutoPlay`) (802-810)
`textRes = enable_auto_play ("Auto-play is on") or disable_auto_play ("Auto-play is off")`; `playerUpdate = ShowTextResource(textRes)`; `autoplayEnabled().set(value)`. Default `pref_auto_play_enabled` = false (marked "Old" in PlayerPreferences.kt:183-185).

### 9.6 Decoder (`getDecoder`, `updateDecoder`) (385-391)
`getDecoder()`: `currentDecoder = getDecoderFromValue(player.hwdecActive)`. `updateDecoder(decoder)`: `setPropertyString("hwdec", decoder.value)`.
Decoder values (part 1, PlayerEnums.kt:81-91): `AutoCopy → "auto-copy"`, `Auto → "auto"`, `SW → "no"`, `HW → "mediacodec-copy"`, `HWPlus → "mediacodec"`. On Linux these map to mpv `hwdec` values `auto-copy`, `auto`, `no`, and the VA-API/VDPAU/NVDEC copy variants (T-45b-046). `HW`/`HWPlus` (`mediacodec*`) are Android-only: Drop with a Linux replacement list.
Preference: `pref_try_hwdec` (default true) decides the initial hwdec in part 1.

### 9.7 Auto-hide and status bar on show/hide (see §10).

## 10. Control visibility and exclusivity

- `showControls()` (666-677): return if any of `sheetShown != None`, `panelShown != None`, `dialogShown != None`. If `showSystemStatusBar()` → show status bars. `_controlsShown = true`.
- `hideControls()` (679-682): hide status bars; `_controlsShown = false`.
- `hideSeekBar()` (684-686): `_seekBarShown = false`.
- `showSeekBar()` (688-691): return if `sheetShown != None`; `_seekBarShown = true`.
- `lockControls()` / `unlockControls()` (693-699): set `_areControlsLocked`.
- `dismissSheet()` (701-703): `_dismissSheet = true`. `resetDismissSheet()` (705-707): `false`.
- `showSheet(sheet)` (709-719): `sheetShown = sheet`. If `sheet == None` → `resetDismissSheet()`; `showControls()`. Else → `hideControls()`; `panelShown = None`; `dialogShown = None`.
- `showPanel(panel)` (721-730): `panelShown = panel`. None → `showControls()`. Else `hideControls()`, `sheetShown = None`, `dialogShown = None`.
- `showDialog(dialog)` (732-741): `dialogShown = dialog`. None → `showControls()`. Else `hideControls()`, `sheetShown = None`, `panelShown = None`.
Exclusivity invariant: at most one of {sheet, panel, dialog} is non-None at a time (each setter clears the other two). `showControls` refuses to show chrome while any is open.

Default `_controlsShown` = `!hideControls()` = true; `hide_controls` pref default false. `pref_player_time_to_disappear` = 4000 (auto-hide delay, consumed by part 1 UI).

## 11. Sleep timer (`startTimer`) (352-364)
1. `timerJob?.cancel()`; `_remainingTime = seconds`.
2. If `seconds < 1` → return (cancel only).
3. `timerJob = viewModelScope.launch { for (time in seconds downTo 0) { _remainingTime = time; delay(1000) }; pause(); withUIContext { toast(AYMR.strings.toast_sleep_timer_ended) } }`.
Total wall time = `(seconds + 1) * 1000 ms`. The timer does not stop on its own when the user resumes; it pauses at the end regardless. `pause()` is called from the Main context.
Callers: `PlayerControls.kt:622` (`onStartSleepTimer = viewModel::startTimer`). Preset values are in the UI (part 1).

## 12. Lua bridge (`handleLuaInvocation`) (856-961)

Entry: mpv property `user-data/aniyomi/<name>` with value string `value`. Name = `property.substringAfterLast("/")`.

Preprocessing: `data = value.removePrefix("\"").removeSuffix("\"").ifEmpty { return }` — an empty payload returns WITHOUT clearing the property.

Post-processing: after the `when` (unless returned early), `MPVLib.setPropertyString(property, "")` clears the property so the same command can be sent again.

Dispatch table (exact):
| name | data | effect |
|---|---|---|
| `show_text` | any | `playerUpdate = ShowText(data)` |
| `toggle_ui` | `show` | `showControls()` |
| `toggle_ui` | `toggle` | if `controlsShown` → `hideControls()` else `showControls()` |
| `toggle_ui` | `hide` | clear sheet, panel, dialog to None; `hideControls()` |
| `toggle_ui` | other | ignored |
| `show_panel` | `subtitle_settings` | `showPanel(SubtitleSettings)` |
| `show_panel` | `subtitle_delay` | `showPanel(SubtitleDelay)` |
| `show_panel` | `audio_delay` | `showPanel(AudioDelay)` |
| `show_panel` | `video_filters` | `showPanel(VideoFilters)` |
| `show_panel` | other | ignored |
| `set_button_title` | any | `_primaryButtonTitle = data` |
| `reset_button_title` | any | title = name of favourite button in `customButtons` (if loaded) |
| `switch_episode` | `n` | `changeEpisode(previous = false)` |
| `switch_episode` | `p` | `changeEpisode(previous = true)` |
| `switch_episode` | other | ignored |
| `launch_int_picker` | `title\|nameFormat\|start\|stop\|step\|pickerProperty` | open `Dialogs.IntegerPicker(defaultValue = getPropertyInt(pickerProperty), minValue = start.toInt(), maxValue = stop.toInt(), step = step.toInt(), nameFormat, title, onChange = setPropertyInt(pickerProperty, it), onDismissRequest = showDialog(None))` |
| `pause` | `pause` / `unpause` / `pauseunpause` | `pause()` / `unpause()` / `pauseUnpause()` |
| `seek_to_with_text` | `value\|text` (split limit 2) | `seekToWithText(value.toInt(), text)` |
| `seek_by_with_text` | `value\|text` | `seekByWithText(value.toInt(), text)` |
| `seek_by` | `value` | `seekByWithText(value.toInt(), null)` |
| `seek_to` | `value` | `seekToWithText(value.toInt(), null)` |
| `toggle_button` | `show` | show primary button (set from favourite if null) |
| `toggle_button` | `hide` | `_primaryButton = null` |
| `toggle_button` | `toggle` | show if null else hide |
| `software_keyboard` | `show` / `hide` / `toggle` | IME force show / implicit hide / toggle by `isActive` (Drop on Linux, T-45b-065) |

Errors: `toInt()` and the picker's destructuring throw on malformed input; the exception propagates into the mpv observer thread (no catch). Proposal: catch, log, and clear the property (E-53).

## 13. Custom buttons (`CustomButton.execute`, `executeLongPress`, `setPrimaryCustomButtonTitle`) (2156-2173)
- `CustomButton.execute()`: `MPVLib.command(["script-message", "call_button_$id"])`.
- `CustomButton.executeLongPress()`: `["script-message", "call_button_${id}_long"]`.
- `setPrimaryCustomButtonTitle(button)`: `_primaryButtonTitle = button.name`.
Data model of `CustomButton` is in area:45-player-core part 1 (`GetCustomButtons`, domain `tachiyomi.domain.custombuttons`). `CustomButtonFetchState` is defined in `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/PlayerSettingsCustomButtonScreenModel.kt` (`getButtons()` at line 161 returns the Success list or empty).

## 14. Screenshots, save, share, set-as-art (1830-1945, 2013-2025)

### 14.1 `takeScreenshot(cachePath, showSubtitles): InputStream?` (1830-1841)
1. `filename = "$cachePath/${currentTimeMillis()}_mpv_screenshot_tmp.png"`.
2. `subtitleFlag = if (showSubtitles) "subtitles" else "video"`.
3. `MPVLib.command(["screenshot-to-file", filename, subtitleFlag])`.
4. `tempFile = File(filename).takeIf { exists } ?: return null`.
5. `newFile = File("$cachePath/mpv_screenshot.png")`; `newFile.delete()`; `tempFile.renameTo(newFile)`.
6. Return `newFile.takeIf { exists }?.inputStream()`.
Observed: the fixed name `mpv_screenshot.png` is overwritten on every capture; the stream is never closed by this function (callers close it).

### 14.2 `generateFilename(anime, timePos: String)` (2013-2025)
`filenameSuffix = " - $timePos"`; `base = "${anime.title} - ${episode.name}"` truncated to `MAX_FILE_NAME_BYTES - suffix.byteSize()` bytes (`takeBytes`) and passed through `DiskUtil.buildValidFilename`; result = `base + suffix`. `MAX_FILE_NAME_BYTES = 250` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:143`). `timePos` = `Utils.prettyTime(seconds)` (mpv Utils, format in §16).

### 14.3 `saveImage(imageStream, timePos: Int?)` (1847-1877)
1. `anime = currentAnime ?: return`.
2. `notifier = SaveImageNotifier(context)`; `notifier.onClear()`.
3. `seconds = timePos?.let { Utils.prettyTime(it) } ?: return`; `filename = generateFilename(...) ?: return`.
4. `relativePath = DiskUtil.buildValidFilename(anime.title)`.
5. `launchNonCancellable`: `uri = imageSaver.save(Image.Page(inputStream, name = filename, location = Location.Pictures(relativePath)))`; `notifier.onComplete(uri)`; send `Event.SavedImage(Success(uri))`. On `Throwable`: `notifier.onError(message)`; send `Event.SavedImage(Error(e))`.
Observed: `onClear()` runs before the null checks, so a failed filename clears any previous notification.

### 14.4 `shareImage(imageStream, timePos)` (1886-1910)
1. `destDir = context.cacheImageDir`.
2. Same validation and filename as 14.3.
3. `try { launchIO { destDir.deleteRecursively(); uri = imageSaver.save(Image.Page(..., location = Location.Cache)); send Event.ShareImage(uri, seconds) } } catch → log`.
Observed: the cache directory is wiped before every share so only the last shared image is kept.

### 14.5 `setAsArt(artType, imageStream)` (1915-1937)
`anime`/`episode` must be non-null. `launchNonCancellable`: per `ArtType` (`Cover` → `anime.editCover(imageSaver's context, stream)`, `Background` → `editBackground`, `Thumbnail` → `episode.editThumbnail(anime, …)`); then `SetAsArt.Success` if `anime.isLocal() || anime.favorite` else `SetAsArt.AddToLibraryFirst`. Any exception → `SetAsArt.Error`. Result sent as `Event.SetArtResult(result, artType)`.

## 15. Per-anime skip-intro length (`getAnimeSkipIntroLength`, `setAnimeSkipIntroLength`) (1984-2008)
- `getAnimeSkipIntroLength()`: `default = defaultIntroLength()` (default 85). If no anime → default. `skipIntroDisable` → 0. `skipIntroLength <= 0` → default. Else `anime.skipIntroLength` (0..255 from viewer flags bits 0-7).
- `setAnimeSkipIntroLength(skipIntroLength: Long)`: return if no anime, or `!anime.favorite`, or `skipIntroLength == getAnimeSkipIntroLength().toLong()`. Else `launchIO { setAnimeViewerFlags.awaitSetSkipIntroLength(anime.id, skipIntroLength); _currentAnime = getAnime.await(anime.id) }`.
Bit layout and the 0-clamp are in `p2-data-and-config.md` §4.4.
Observed (E-24): non-favourite anime cannot store a per-anime length; the value stays in mpv only for the session.

## 16. Concurrency, dispatchers and blocking calls

| Location | Mechanism | Notes |
|---|---|---|
| All `MutableStateFlow` writes | `update { }` (atomic CAS) | Single-threaded in the Rust port (GTK main loop). |
| `viewModelScope.launch` | Main dispatcher | Used for timers, track loading, progress steps. |
| `viewModelScope.launchIO` | `Dispatchers.IO` | Hoster loading, tile fetching, download-ahead, set-as-skip-length. |
| `viewModelScope.launchNonCancellable` | `NonCancellable` context | Progress saves, deletes, screenshot save, art set, bookmark/fillermark. Survives `onCleared`. |
| `runBlocking` in `initEpisodeList`, `checkTrackers` | Blocks caller | Called from `init` on the IO/Main path (PlayerActivity). Proposal: await. |
| `withIOContext` in `loadEpisode` | IO | Hoster resolution on switch. |
| `withUIContext` | Main | Toasts, cache writes after tile fetch. |
| `withContext(Dispatchers.IO)` in `getBestVideo` | IO fan-out with `async` and `EarlyReturnException` | Cancels siblings with `cancelChildren()`. |
| `coroutineScope` in `loadHosters` | Structured fan-out | `awaitAll`; cancellation rethrown after resetting hoster states. |
| `Channel<Event>()` | Rendezvous channel | `send` suspends until collected (E-23). |

Cancellation rules: `CancellationException` is always rethrown (never swallowed) in loaders and tile fetches; other exceptions are converted to `Error` states or logged.

## 17. Timings, limits and magic numbers (complete list in this scope)

| Value | Where | Meaning |
|---|---|---|
| 1000 ms | PlayerViewModel.kt:359 | sleep timer tick |
| 2 s (`2.seconds`) | :585 | min interval between network tile fetches |
| 3 (`size > 3`) | :260 | tile LRU capacity |
| 2 | :1593 | tile maps preloaded at episode start |
| 100 | :770, :771 | mpv volume floor / boost base |
| -0.75 .. 1 | :755 | brightness state clamp |
| 0 .. 1 | :757 | window brightness clamp |
| 1.0 s | ChapterUtils.kt:158,159,173,191 | adjacency / start tolerance |
| 1.0 (<1.0) | ChapterUtils.kt:145,178 | first chapter snaps to 0 |
| -2.0 | ChapterUtils.kt:158 | duration sentinel when unknown |
| 0xFFD8BBDF | ChapterUtils.kt:154 | colour of external chapter segments |
| 0.35 | PlayerViewModel.kt:1682 | fraction after which download-ahead starts |
| 0.85 default | PlayerPreferences.kt:115 | fraction of duration that marks episode seen |
| 0 .. 255 | AniSkip interval / viewer flags | skip intro length range (bits 0-7) |
| 85 | GesturePreferences.kt:85 | default intro length in seconds |
| 10 | GesturePreferences.kt:86 | default double-tap seek seconds |
| 5 | PlayerPreferences.kt:154 | netflix countdown seconds |
| 30 | AudioPreferences.kt:29 | volume boost cap (percent over 100) |
| 4000 ms | PlayerPreferences.kt:143 | control auto-hide delay (consumed by part 1 UI) |
| 60 | PlayerPreferences.kt:144 | panel opacity percent |
| 55 | SubtitlePreferences.kt:213 | subtitle font size |
| 100 | SubtitlePreferences.kt:233 | subtitle position percent |
| 3 | SubtitlePreferences.kt:215 | subtitle border size |
| 250 | DiskUtil.kt:143 | max filename bytes |
| 12 | PlayerPreferences.kt:178 | number of default speed presets (see p2-data-and-config) |

## 18. User-visible message keys (exact)

Each key is `AYMR.strings.<name>` (`i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`) unless marked MR. English base text as found in the base file; keys whose base text is not English are quoted verbatim and flagged.

| Key | Base text | Shown by |
|---|---|---|
| `toast_sleep_timer_ended` | "Sleep timer ended" | sleep timer end (toast) |
| `no_prev_episode` | "الحلقة السابقة غير موجودة!" (base file is not English; quoted verbatim) | `changeEpisode(previous=true)` when no previous (toast) |
| `no_next_episode` | "Next Episode not found!" | `changeEpisode` when no next (toast) |
| `player_aniskip_dontskip` | "এৰি নিদিব" (non-English base text; verbatim) | skip button label during netflix countdown (waiting > 0) |
| `player_aniskip_skip` | "%s saltats" (non-English; verbatim) | seek text after countdown / manual skip; `%s` = chapter name |
| `player_intro_skipped` | "%1$s skipped" | seek text after auto-skip; `%1$s` = chapter name |
| `player_skip_action` | "تخطّي %1$s" (non-English; verbatim) | skip button label `%1$s` = chapter type label |
| `player_aniskip_dontskip_toast` | "Skipping %s in %d seconds" | netflix toast `%s` chapter name, `%d` countdown; prefixed with hard-coded "Skip Intro: " |
| `no_episode_loaded` | "No episode loaded" | `init` and `loadEpisode` failure |
| `select_hoster_from_empty_list` | "Hoster selected from empty list" | `init` with hostList parse empty |
| `no_hosters` | "Hoster list is empty" | `init` when resolved list empty |
| `no_available_videos` | "No available videos" | `loadHosters`/`loadVideo` exhausted |
| `enable_auto_play` | "Auto-play is on" | `setAutoPlay(true)` |
| `disable_auto_play` | "Auto-play is off" | `setAutoPlay(false)` |
| `player_chapter_type_opening` | "Opening" | chapter type label |
| `player_chapter_type_ending` | "Ending" | chapter type label |
| `player_chapter_type_recap` | "Recap" | chapter type label |
| `player_chapter_type_mixedop` | "Mixed opening" | chapter type label |
| `MR.strings.off` | "Off" | track list "Off" entry, language/title getters for id -1 |
| `AYMR.strings.rotation_*`, `single_action_*`, `video_*_screen`, `player_sheets_filters_*` | in part 1 (enums) | used only as titles of part 1 enums |
| Event `SavedImage`/`ShareImage` results | (notifier strings in area:43) | save/share toasts |

Hard-coded English strings in this scope (must become resources in Rust):
- `"Skip Intro: "` prefix (PlayerViewModel.kt:2083).
- `"Local source: ${episode.url}"` (EpisodeLoader.kt:137) — internal title, not displayed as UI text by this code path; shown in the quality sheet.
- `"No episode loaded"`, `"Hoster selected from empty list"`, `"Hoster list is empty"`, `"No available videos"` appear as exception messages AND as resource keys; the message is for logs.
- `"Unable to fetch buttons"` (PlayerViewModel.kt:344) — fallback error text for custom buttons.
- Track title default `"Off"` and `"Local source: …"`.

## 19. Interactions with other areas

- area:24-downloads-anime: `isEpisodeDownloaded(name, scanlator, animeTitle, source, skipCache)`, `buildVideo(source, anime, episode)`, `downloadEpisodes`, `enqueueEpisodesToDelete`, `deletePendingEpisodes`, `addDownloadsToStartOfQueue`.
- area:25-downloads-shared: `filterDownloadedEpisodes`, download-ahead de-duplication.
- area:16-source-api-anime and area:17-source-app-layer: `getHosterList`, `getVideoList`, `getVideoUrl`, `resolveVideo`, `getVideoThumbnails`, `getImageTile`, `sortHosters`, `sortVideos`, `getOrStub`, `isInitialized`.
- area:05/06/07 (data/domain): `GetAnime`, `GetEpisodesByAnimeId`, `UpdateEpisode`, `UpsertAnimeHistory`, `GetNextEpisodes`, `SetAnimeViewerFlags`, `GetAnimeIncognitoState`.
- area:27/28/29 (tracking): `GetAnimeTracks`, `TrackEpisode`, `TrackerManager`, `MyAnimeList`, `Anilist`.
- area:08 (library preferences): `markDuplicateSeenEpisodeAsSeen` (`mark_duplicate_seen_episode_seen`), `LibraryPreferences.MARK_DUPLICATE_EPISODE_SEEN_EXISTING`.
- area:43-reader-core: `SaveImageNotifier`.
- area:13: `DiskUtil`, `cacheImageDir`, `editCover`/`editBackground`/`editThumbnail` (area:34 for covers).
- area:45-player-core part 1: `PlayerActivity` (calls listed in §0), `ExternalIntents` (calls `getBestVideo`), `PlayerEnums` (enum values in `p2-data-and-config.md` §2), `PlayerControls`/sheets (UI), `AniyomiMPVView` (mpv property observation).
