# Topic: episode quality and options dialog (EpisodeOptionsDialogScreen)

Deep dive on `app/src/main/java/eu/kanade/presentation/entries/anime/EpisodeOptionsDialogScreen.kt` (696 LOC). Shared types used by the dialog live in `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/QualitySheet.kt` (HosterState, getChangedAt), `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt` (selectBestVideo, getResolvedVideo) and `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt` (getHosters, loadHosterVideos). Paths are repo-relative to `/home/blackbox/aniyomi-src`. Player internals belong to the player area (not yet planned); this file specifies only what this dialog depends on.

## 1. Entry and inputs

- Constructor: `EpisodeOptionsDialogScreen(useExternalDownloader: Boolean, episodeTitle: String, episodeId: Long, animeId: Long, sourceId: Long)`.
- Opened from `AnimeScreenModel` as `Dialog.ShowQualities(episode, anime, source)` (`AnimeScreenModel.kt:1598-1600`) when the user long-presses a not-downloaded episode (`EpisodeDownloadAction.SHOW_QUALITIES`, `AnimeScreenModel.kt:828-830`). Also opened from the updates and history screens (`app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesTab.kt` references `EpisodeOptionsDialogScreen(`; not in this scope).
- `useExternalDownloader` = `DownloadPreferences.useExternalDownloader()` (default false, key `use_external_downloader`).

## 2. State (ScreenModel)

Kotlin `EpisodeOptionsDialogScreenModel` (lines 138-391) holds:

| Flow | Type | Initial | Meaning |
|---|---|---|---|
| `_hosterState` | `Result<List<HosterState>>?` | null | null = loading; failure = dialog error; success = per-hoster state |
| `_hosterExpandedList` | `List<Boolean>` | empty | expansion per hoster (true after load) |
| `_selectedHosterVideoIndex` | `Pair<Int, Int>` | (-1, -1) | selected (hoster, video); (-1,-1) = none |
| `_currentVideo` | `Video?` | null | the video that will be played or downloaded |
| `_episode`, `_anime` | `Episode?`, `Anime?` | null | loaded from DB |
| `_hosterList` | `List<Hoster>` | empty | raw hosters from the source (hoster videos may be null) |
| `_source` | `AnimeSource?` | null | resolved source (`sourceManager.getOrStub(sourceId)`) |
| `_showAllQualities` | `Boolean` | false | which of the two panels is shown |
| `hasFoundPreferredVideo` | `AtomicBoolean` | false (local to init) | race guard for preferred video |

## 3. Load algorithm (init, lines 168-260)

```
launchIO:
  episode  = GetEpisode.await(episodeId)!!        # D-5: crash path
  anime    = GetAnime.await(animeId)!!            # D-5
  source   = sourceManager.getOrStub(sourceId)
  set _episode, _anime, _source

  hostersResult = withIOContext { try success(EpisodeLoader.getHosters(episode, anime, source)) catch e: failure(e) }
  if failure: _hosterState = failure(e); return

  hosters = hostersResult.value
  _hosterList = hosters
  _hosterExpandedList = [true] * hosters.size
  initial = hosters.map { h -> if h.videoList == null: Loading(h.hosterName)
                               else Ready(h.hosterName, h.videoList, [LOAD_VIDEO] * n) }
  _hosterState = success(initial)

  try:
    hosters.mapIndexed { idx, hoster -> async {
        hs = EpisodeLoader.loadHosterVideos(source, hoster)
        _hosterState[idx] = hs
        if hs is Ready:
          p = hs.videoList.indexOfFirst { it.preferred }
          if p != -1 and hasFound.compareAndSet(false, true):
             ok = loadVideo(source, hs.videoList[p], idx, p)
             if !ok: hasFound.set(false)
    } }.awaitAll()

    if hasFound.compareAndSet(false, true):          # true only if no preferred video was loaded
      if _selectedHosterVideoIndex == (-1, -1):
        list = _hosterState.getOrThrow()
        (h, v) = selectBestVideo(list)
        if h == -1: _hosterState = failure(NoSuchElementException("No available videos")); return
        loadVideo(source, (list[h] as Ready).videoList[v], h, v)
  catch CancellationException:
    _hosterState = success(hosters.map { Idle(it.hosterName) })
    rethrow
```

Notes:
- The final `compareAndSet(false, true)` both tests and sets the flag. After it, the flag is always true. This is intentional: it makes the "select best" step run once.
- If the preferred video's `loadVideo` fails, the flag is reset to false, so a later hoster with a preferred video can win the race. If none wins, the fallback runs.
- The fallback runs only when `_selectedHosterVideoIndex == (-1, -1)`, so a user click that happened during loading is not overwritten.
- A non-cancellation exception thrown inside `loadHosterVideos` propagates out of `awaitAll` and is not caught by the `try` (it only catches `CancellationException`). Proposal: `loadHosterVideos` should return `HosterState.Error` for failures (its signature returns `HosterState`); the Rust port converts any unexpected error into `Error` for that hoster and never aborts the others.

## 4. `loadVideo` algorithm (lines 262-316)

Signature: `suspend fun loadVideo(source, video, hosterIndex, videoIndex): Boolean`.

```
selected = _hosterState.getOrThrow()[hosterIndex] as? Ready ?: return false
old = _selectedHosterVideoIndex
_selectedHosterVideoIndex = (hosterIndex, videoIndex)
_hosterState[hosterIndex] = selected.getChangedAt(videoIndex, video, LOAD_VIDEO)

resolved = if selected.videoState[videoIndex] != READY
             then HosterLoader.getResolvedVideo(source, video)
             else video

if resolved == null or resolved.videoUrl.isEmpty():
   if _currentVideo == null:                                        # first auto-selection
      _hosterState[hosterIndex] = selected.getChangedAt(videoIndex, video, ERROR)   # stale 'selected'
      list = _hosterState.getOrNull() ?: return false
      (nh, nv) = selectBestVideo(list)
      if nh == -1: _hosterState = failure("No available videos"); return false
      return loadVideo(source, list[nh].videoList[nv], nh, nv)     # recursion
   else:                                                            # user chose a bad video
      _selectedHosterVideoIndex = old
      _hosterState[hosterIndex] = selected.getChangedAt(videoIndex, video, ERROR)
      return false

_hosterState[hosterIndex] = selected.getChangedAt(videoIndex, resolved, READY)
_currentVideo = resolved
return true
```

Rust port rules:
- Replace the recursion with a loop that re-reads the current state each iteration, with a cap of `total_videos` iterations (each failing iteration marks one more video `Error`, so the loop ends).
- Re-read the hoster state at each step instead of using the stale `selected` snapshot (fixes R-10).
- Keep the two branches exactly: auto-selection failure falls through to the next best; user-initiated failure restores the previous selection and returns false.

## 5. Other entry points

`onClickHoster(hosterIndex)` (lines 334-359):
- Ready: toggle `expanded[idx]`.
- Error or Idle: set `Loading(name)`, then `launchIO { new = loadHosterVideos(_source, _hosterList[idx]); updateAt(idx, new) }`.
- Loading: no-op.

`onClickVideo(hosterIndex, videoIndex)` (lines 361-373): reads the Ready hoster's video at `videoIndex` (no-op otherwise), then `launchIO { ok = loadVideo(...); if ok: _showAllQualities = false }`. So a successful click returns to the summary panel.

`onShowAllQualities(value)`: sets `_showAllQualities`.

`getHosterList()` (lines 375-390): returns null when `hosterState` is null or failed; otherwise maps `_hosterList` so that every Ready hoster carries its `videoList` from the state. Fields copied: `hosterName`, `hosterUrl`, `videoList`, `internalData`, `memo`. Not copied: `lazy` (defaults to false). Observed; Rust must copy `lazy` too.

`updateAt(index, value)` (lines 318-328): replaces one element of the Ready list by index (copy-on-write list update). If the current value is null or failed, it is a no-op.

## 6. UI (EpisodeOptionsDialog, lines 393-469; VideoList 471-596; QualityOptions 598-651; ClickableRow 653-696)

Layout (Column with `animateContentSize`, `verticalScroll`, padding vertical 8 dp, `windowInsetsPadding(systemBars)`, spacing `MaterialTheme.padding.small` = 8):
1. Title: `episodeTitle`, `titleSmall`, bold, one line, horizontal padding 24 dp.
2. Subtitle: `AYMR.strings.choose_video_quality` ("Choose video quality:"), `bodyMedium`, italic.
3. Body:
   - If `resultList` is failure: `onError()`.
   - If `resultList == null || episode == null || anime == null || currentVideo == null`: `LoadingScreen()`.
   - Else if `hosterStateList` non-empty: `VideoList(...)`.
   - Else: `onError()`.

`onError` (lines 436-440): logs `"Error getting links"` at ERROR, toasts `"No available videos"`, then `EpisodeOptionsDialogScreen.onDismissDialog()`. It is called from composition (D-16 below).

VideoList, panel A (`AnimatedVisibility(visible = !showAllQualities, slideInHorizontally() / slideOutHorizontally())`):
- If `currentVideo.videoUrl` is not empty and not showing all: `ClickableRow(text = currentVideo.videoTitle, icon = null, onClick = { onShowAllQualities(true) }, showDropdownArrow = true)`.
- `QualityOptions` (five rows, below).

VideoList, panel B (`AnimatedVisibility(visible = showAllQualities, slideInHorizontally(initialOffsetX = { it / 2 }) / slideOutHorizontally(targetOffsetX = { it / 2 }))`):
- `Box(fillMaxWidth, padding horizontal 24 dp, heightIn(max = 600.dp))`.
- If there is exactly one hoster and its name is `Hoster.NO_HOSTER_LIST` ("no_hoster_list") and it is Ready: `QualitySheetVideoContent(videoList, videoState, selectedVideoIndex = selected.second, onClickVideo)`.
- Else: `QualitySheetHosterContent(hosterState, expandedState, selectedVideoIndex = selected, onClickHoster, onClickVideo, displayHosters = Pair(false, false))`.

QualityOptions rows (in this order; each `ClickableRow` has a 20 dp icon, a 8 dp gap, text `bodyMedium`, vertical padding 8 dp when icon present, horizontal padding 24 dp):

| # | Label key (English) | Icon | Action | Closes dialog |
|---|---|---|---|---|
| 1 | `MR.strings.copy` "Copy" | `ContentCopy` | `clipboard.setText(currentVideo.videoUrl)`; `launch { if currentVideo.usesHttpServer() startHttpServerService(context, anime.source); toast(copiedString) }` | no |
| 2 | `AYMR.strings.action_start_download_internally` "Use internal downloader" | `Download` | `downloadEpisode(useExternalDownloader)` | yes |
| 3 | `AYMR.strings.action_start_download_externally` "Use external downloader" | `SystemUpdateAlt` | `downloadEpisode(!useExternalDownloader)` | yes |
| 4 | `AYMR.strings.action_play_externally` "Play externally" | `OpenInNew` | `startPlayerActivity(context, anime.id, episode.id, external = true, anime.source, currentVideo)` | yes |
| 5 | `AYMR.strings.action_play_internally` "Play internally" | `Input` | `startPlayerActivity(context, anime.id, episode.id, external = false, anime.source, currentVideo, selectedHosterVideoIndex.first, .second, getHosterList())` | yes |

`downloadEpisode(alt)` = `downloadManager.downloadEpisodes(anime, listOf(episode), autoStart = true, alt = alt, video = currentVideo)`.

Meaning of `alt`: it flips the user's default downloader. With `use_external_downloader = false`, row 2 passes `alt = false` (internal) and row 3 passes `alt = true` (external). With `use_external_downloader = true`, row 2 passes `alt = true` (which selects internal) and row 3 passes `alt = false` (external). The Rust port must keep the same parameter values so that the labelled downloader is used in both preference states. Test T-Q15.

Copy row: `usesHttpServer()` is a `Video` method (player area); when true, `MainActivity.startHttpServerService(context, anime.source)` runs first. The Rust port calls the player's local-server starter; if it is not available yet, copy still works and the toast still shows.

## 7. Video and hoster row states (sheet components)

From `QualitySheet.kt`:
- Hoster row: `HosterState.Idle` (not loaded yet, clickable to load), `Loading` (spinner), `Error` (error icon, clickable to retry), `Ready` with empty list (shown as "empty" in the list, not clickable), `Ready` with videos (expand arrow; expansion from `expandedState`).
- Video row: `Video.State.QUEUE`, `LOAD_VIDEO` (spinner), `READY` (normal), `ERROR` (error styling). Selected video is marked by `selectedVideoIndex`.
- Filter in the hoster list: hosters that are Idle, Loading, or Ready with videos are shown; Error hosters and empty Ready hosters are listed in separate groups (`QualitySheet.kt:196-205`). Rust: keep the same grouping.

## 8. Selection rules (`HosterLoader.selectBestVideo`, `HosterLoader.kt:27-67`)

Usable video: `state == READY || state == QUEUE`.

Pass 1 (preferred): the first Ready hoster (in list order) containing a video with `preferred && usable`; inside it, the first such video. Return `(hosterIndex, videoIndex)`.

Pass 2 (any): the first Ready hoster containing a video with `videoUrl.isNotEmpty() && usable`; inside it, the first such video.

Otherwise `(-1, -1)`.

Note: pass 1 does not require a non-empty URL. A preferred video with an empty URL can be selected in pass 1 and then fail in `loadVideo`, which then tries the next best. That is the intended recursion path.

## 9. Defects and observed quirks

- D-16: `onError()` is invoked inside the composable body (lines 441-443, 464-466), which launches a toast and dismisses the screen during composition. Rust: raise the event from the model, not from the view builder.
- D-5: `!!` on `GetEpisode.await` and `GetAnime.await`, `_hosterState.value!!`, `_source.value!!` (see p1-edge-cases-and-tests.md).
- R-10: `loadVideo` uses a stale snapshot `selected` after it has written `LOAD_VIDEO` (lines 268-271 versus 279-306). Concurrent updates to other videos of the same hoster can be reverted. Fixed by re-reading (section 4).
- R-11: `getHosterList` drops `lazy` (section 5).
- Global callback: `EpisodeOptionsDialogScreen.onDismissDialog` is a companion `var` (line 134). Two dialogs share it (R-6). Rust: pass a closure to the window.
- `loadHosterVideos(source, hoster, force: Boolean = false)` has a `force` parameter; this dialog never passes `force = true`. The Rust port keeps the parameter for the player's retry path.

## 10. Rust state machine

```rust
pub enum HosterStatus { Idle, Loading, Error, Ready { videos: Vec<Video>, states: Vec<VideoState> } }

pub enum QualityEvent {
    HostersLoaded(Result<Vec<Hoster>, QualityError>),
    HosterLoaded { idx: usize, state: HosterStatus },   // from loadHosterVideos
    VideoResolved { hoster: usize, video: usize, result: Option<Video> },
    UserClickedHoster(usize),
    UserClickedVideo { hoster: usize, video: usize },
    ShowAllQualities(bool),
    Cancelled,
}
```

Transitions (table):

| From | Event | Guard | To | Side effects |
|---|---|---|---|---|
| Loading | HostersLoaded(Err) | | Failed | toast "No available videos", close |
| Loading | HostersLoaded(Ok(hs)) | | Listing | create per-hoster Loading or Ready(LOAD_VIDEO) |
| Listing | HosterLoaded(i, s) | | Listing | store s; if Ready with preferred and no winner yet, start load of preferred |
| Listing | all hosters done | no winner and no selection | Selecting | selectBestVideo; if none: Failed |
| Selecting | VideoResolved(ok) | | Playing(current) | set current |
| Selecting | VideoResolved(fail) | current none | Selecting | mark ERROR, select next best or Failed |
| Playing | UserClickedVideo(ok) | | Playing | set selection and current; hide all-qualities |
| Playing | UserClickedVideo(fail) | | Playing | restore previous selection; mark ERROR |
| any | UserClickedHoster(Idle or Error) | | same | set Loading; start loadHosterVideos |
| any | UserClickedHoster(Ready) | | same | toggle expansion |
| any | Cancelled | | Closed | Idle for unfinished hosters; no further events |

## 11. Tests

- T-Q1..T-Q5: selection passes and usable states (p1-edge-cases-and-tests.md).
- T-Q6: load loop restore/fallback.
- T-Q7: no panics on missing state.
- T-Q8, T-Q9: Copy stays open, other four close.
- T-Q10: failure toast once.
- T-Q11: cancellation leaves Idle and no events.
- T-Q12: preferred race (two hosters, first fails, second wins).
- T-Q13: two windows with separate closures.
- T-Q14: `getHosterList` substitution and `lazy` copy (R-11).
- T-Q15: `alt` parameter table in section 6 for both preference values.
- T-Q16: stale-snapshot regression (R-10): two concurrent updates to videos of one hoster both survive.

## 12. Open items

- OD-6: the hoster loading uses `EpisodeLoader.loadHosterVideos`, which belongs to the player area. Confirm its contract (returns `Error` instead of throwing) when the player area is planned.
- OD-7: the Copy action on hosters that need the local HTTP server (`usesHttpServer`) depends on the player's server; confirm the Linux server start method.
