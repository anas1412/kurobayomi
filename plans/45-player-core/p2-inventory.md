# 45-player-core (part 2 of 2) — File inventory

Area: **45-player-core**, part 2 of 2. Scope = the 12 files printed by `python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 45-player-core 2`.
Repo root for every path: `/home/blackbox/aniyomi-src` (all paths below are repo-relative).

Part 1 of this area (documented by the other author, not here) holds `PlayerActivity.kt`, `PlayerEnums.kt`, `AniyomiMPVView.kt`, `ExternalIntents.kt`, `PipActions.kt`, `PlayerObserver.kt`, `PlayerUtils.kt`, `HttpServerService.kt`, `TorrentServerService.kt`, `app/src/main/assets/aniyomi.lua`. Names that part 1 defines and this part uses are referenced here by name only (`Decoder`, `getDecoderFromValue`, `Debanding`, `Sheets`, `Panels`, `Dialogs`, `PlayerUpdates`, `SingleActionGesture`, `CustomKeyCodes`, `VideoAspect`, `PlayerOrientation`, `ArtType`, `SetAsArt`, `VideoFilters`). Their values are quoted in `p2-data-and-config.md` section 2 for completeness because the port of this part depends on them.

Other symbols used by this part but owned by other areas: `HosterState` and `getChangedAt` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/QualitySheet.kt`, area:45-player-core part 1 UI sheets), `IndexedSegment` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/SeekBar.kt`, UI), `Video`/`Hoster`/`TimeStamp`/`ChapterType`/`ThumbnailInfo`/`TileInfo` (`source-api/...`, area:16-source-api-anime), `getEpisodeSort` (`domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt`, area:07-domain-entries-items), `Anime` filter/viewer constants (`domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt`, area:07), `AnimeDownloadManager` (area:24-downloads-anime), `UpdateEpisode`/`GetEpisodesByAnimeId`/`UpsertAnimeHistory`/`GetNextEpisodes` (area:05/06/07), `TrackEpisode`/`GetAnimeTracks`/`TrackerManager` (area:27-tracking-core, area:28-tracker-anilist, area:29-tracker-myanimelist), `SaveImageNotifier` (area:43-reader-core), `ImageSaver` (area:13), `DiskUtil` (area:13), `AnimeSourceManager` (area:17), `CustomButton` (area:45-player-core part 1 settings), `LibraryPreferences`/`BasePreferences`/`TrackPreferences`/`UiPreferences`/`DownloadPreferences` (area:02, area:08, area:09).

Port decision legend: **Port** = same behavior, re-expressed in Rust. **Rewrite** = same user-visible behavior, different mechanism. **Replace-with-crate** = library swapped for a named crate. **Drop** = Android-only, no Linux behavior.

Priority legend: P0 usable daily, P1 parity, P2 long-tail, P3 Android-only/obsolete (still has a task).

Task ids are `T-45b-NNN` and are defined in `p2-tasks.md`.

## Summary table

| # | Full repo-relative path | LOC | All declarations in file | Purpose | Port decision | Target Rust module | Priority | Task ids |
|---|---|---|---|---|---|---|---|---|
| 1 | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt | 2177 | see table A below (every member) | Central player state holder (flows), episode/hoster/video init, mpv property bridge, seek/volume/brightness/aspect, progress saving, history, downloads hooks, screenshots, skip-intro state machine, Lua bridge, sleep timer | Rewrite (MVU-style state store on GTK main loop; Android Compose StateFlow replaced) | `src/player/model.rs`, `src/player/state.rs`, `src/player/actions.rs`, `src/player/progress.rs`, `src/player/skip.rs`, `src/player/seek.rs`, `src/player/lua_bridge.rs` | P0 (most), P1, P2 | T-45b-002, 003, 004, 005, 006, 013, 014, 015, 016, 018, 022, 023, 024, 025, 026, 027, 028, 029, 030, 031, 032, 033, 035, 036, 037, 038, 039, 040, 041, 042, 043, 053, 054, 055, 056, 059, 060, 068, 069, 071, 072 |
| 2 | app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt | 206 | `class EpisodeLoader`; companion object: `getHosters`, `isDownload`, `checkHasHosters` (private), `getHostersOnHttp` (private), `getHostersOnDownloaded` (private), `getHostersOnLocal` (private), `getVideos` (private), `getVideosOnHttp` (private), `List<Video>.parseVideoUrls` (private extension), `loadHosterVideos` | Resolves the hoster list for an episode (downloaded file, HTTP source, local source) and per-hoster videos; lazy hoster loading | Port | `src/player/loader/episode.rs` | P0 | T-45b-007, 008, 009, 012, 063, 064 |
| 3 | app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt | 170 | `class HosterLoader`; companion object: `selectBestVideo`, `class EarlyReturnException(val video: Video) : Exception` (nested), `getBestVideo`, `getResolvedVideo` | Picks the best video across hosters (preferred flag, then first non-empty URL); parallel resolution race for external-player path | Port | `src/player/loader/hoster.rs` | P0 | T-45b-010, 011, 012 |
| 4 | app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AdvancedPlayerPreferences.kt | 15 | `class AdvancedPlayerPreferences`; methods `mpvUserFiles`, `mpvConf`, `mpvInput`, `playerStatisticsPage` | mpv user scripts toggle, mpv.conf text, input.conf text, statistics page index | Port | `src/settings/player_advanced.rs` | P1 | T-45b-048, 034 |
| 5 | app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt | 27 | `class AudioPreferences`; methods `preferredAudioLanguages`, `enablePitchCorrection`, `audioChannels`, `volumeBoostCap`, `audioDelay`; `enum class AudioChannels(titleRes, property, value)` with `Auto`, `AutoSafe`, `Mono`, `Stereo`, `ReverseStereo` | Audio language preference, pitch correction, channel layout, volume boost cap, audio delay | Port | `src/settings/player_audio.rs`, `src/player/audio.rs` | P1 | T-45b-045, 048 |
| 6 | app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/DecoderPreferences.kt | 22 | `class DecoderPreferences`; methods `tryHWDecoding`, `gpuNext`, `videoDebanding`, `useYUV420P`, `brightnessFilter`, `saturationFilter`, `contrastFilter`, `gammaFilter`, `hueFilter` | Hardware decode toggle, gpu-next, debanding, yuv420p, persisted video filter values | Port | `src/settings/player_decoder.rs`, `src/player/video_filters.rs` | P1 | T-45b-046, 048 |
| 7 | app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt | 36 | `class GesturePreferences`; methods `gestureVolumeBrightness`, `swapVolumeBrightness`, `gestureHorizontalSeek`, `showSeekBar`, `defaultIntroLength`, `skipLengthPreference`, `playerSmoothSeek`, `leftDoubleTapGesture`, `centerDoubleTapGesture`, `rightDoubleTapGesture`, `mediaPreviousGesture`, `mediaPlayPauseGesture`, `mediaNextGesture` | Gesture bindings (double tap, media keys), seek length, intro length default | Port | `src/settings/player_gesture.rs` | P0 | T-45b-047, 048, 024 |
| 8 | app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt | 86 | `class PlayerPreferences`; methods `preserveWatchingPosition`, `progressPreference`, `defaultPlayerOrientationType`, `allowGestures`, `showLoadingCircle`, `showCurrentChapter`, `rememberPlayerBrightness`, `playerBrightnessValue`, `rememberPlayerVolume`, `playerVolumeValue`, `showFailedHosters`, `showEmptyHosters`, `playerFullscreen`, `hideControls`, `displayVolPer`, `showSystemStatusBar`, `reduceMotion`, `playerTimeToDisappear`, `panelOpacity`, `enableSkipIntro`, `autoSkipIntro`, `enableNetflixStyleIntroSkip`, `waitingTimeIntroSkip`, `aniSkipEnabled`, `disableAniSkipOnChapters`, `enablePip`, `pipEpisodeToasts`, `pipOnExit`, `pipReplaceWithPrevious`, `alwaysUseExternalPlayer`, `externalPlayerPreference`, `playerSpeed`, `speedPresets`, `invertDuration`, `aspectState`, `autoplayEnabled` | Central player preference catalogue (controls, hoster visibility, intro skip, PiP, external player, speed, aspect) | Port (keys preserved in `settings.json`-style store) ; PiP methods Rewrite (see T-45b-050) ; orientation Drop (T-45b-058) | `src/settings/player.rs` | P0 (catalogue, skip, speed, aspect), P1 (external, hoster visibility, autoplay), P2 (PiP, panel opacity, reduce motion), P3 (orientation) | T-45b-048, 049, 050, 051, 052, 053, 054, 055, 056, 057, 058, 059 |
| 9 | app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/SubtitlePreferences.kt | 64 | `class SubtitlePreferences`; methods `preferredSubLanguages`, `subtitleWhitelist`, `subtitleBlacklist`, `screenshotSubtitles`, `subtitleFont`, `subtitleFontSize`, `subtitleFontScale`, `subtitleBorderSize`, `boldSubtitles`, `italicSubtitles`, `textColorSubtitles`, `borderColorSubtitles`, `borderStyleSubtitles`, `shadowOffsetSubtitles`, `backgroundColorSubtitles`, `subtitleJustification`, `subtitlePos`, `overrideSubsASS`, `subtitlesDelay`, `subtitlesSpeed`, `subtitlesSecondaryDelay`; `enum class SubtitleJustification(value, icon)` with `Left`, `Center`, `Right`, `Auto` | Subtitle language/whitelist/blacklist, style, delay, position | Port (style to mpv `sub-*` options; icons Drop) | `src/settings/player_subtitle.rs`, `src/player/subtitle_style.rs` | P0 (style, delay, pos), P1 (language, lists) | T-45b-044, 017, 048 |
| 10 | app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/AniSkipApi.kt | 117 | `class AniSkipApi` (members `client`, `json`, `getResult`, `getMalIdFromAL`); `data class AniSkipResponse(found, results)`; `data class Stamp(interval, skipType)`; `enum class SkipType` with `OP`, `ED`, `RECAP`, `MIXED_OP`, members `getString`, `toChapterType`; `data class AniSkipInterval(startTime, endTime)` | AniSkip (MAL id based) skip-times lookup and AniList id to MAL id resolution | Replace-with-crate (`reqwest` + `serde`; `reqwest` replaces OkHttp; the non-app client is intentionally replaced by the shared Kurobayomi HTTP client) | `src/player/skip/aniskip.rs` | P1 | T-45b-020, 021 |
| 11 | app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/ChapterUtils.kt | 105 | `class ChapterUtils`; companion object: `ChapterType.getStringRes` (extension), `mergeChapters` | Converts AniSkip/extension timestamps into chapter segments merged with mpv's own chapters; maps ChapterType to labels | Port | `src/player/chapters.rs` | P1 | T-45b-018, 019, 069 |
| 12 | app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/TrackSelect.kt | 65 | `class TrackSelect`; methods `getPreferredTrackIndex`, `containsLang` (private) | Chooses the preferred subtitle/audio track from language lists, black/white lists | Port (regex quirk preserved, see behavior) | `src/player/track_select.rs` | P0 | T-45b-017 |

## Table A — every member of PlayerViewModel.kt (app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt)

Line numbers are the declaration line. "priv" = private. "st" = `MutableStateFlow` exposed as `StateFlow` via `asStateFlow()`. Rust target is the field/method in `src/player/model.rs` unless stated.

Top-level declarations in the file: `class PlayerViewModelProviderFactory` (145), `class PlayerViewModel` (153), `fun CustomButton.execute()` (2167), `fun CustomButton.executeLongPress()` (2171), `fun Float.normalize(inMin, inMax, outMin, outMax)` (2175).

Constructor (153–176): `activity: PlayerActivity` (priv), `savedState: SavedStateHandle` (priv), `sourceManager: AnimeSourceManager`, `downloadManager: AnimeDownloadManager`, `imageSaver: ImageSaver`, `downloadPreferences: DownloadPreferences`, `trackPreferences: TrackPreferences`, `trackEpisode: TrackEpisode`, `getAnime: GetAnime`, `getNextEpisodes: GetNextEpisodes`, `getEpisodesByAnimeId: GetEpisodesByAnimeId`, `getAnimeCategories: GetAnimeCategories`, `getTracks: GetAnimeTracks`, `upsertHistory: UpsertAnimeHistory`, `updateEpisode: UpdateEpisode`, `setAnimeViewerFlags: SetAnimeViewerFlags`, `playerPreferences: PlayerPreferences` (internal), `gesturePreferences: GesturePreferences` (internal), `basePreferences: BasePreferences`, `getCustomButtons: GetCustomButtons`, `trackSelect: TrackSelect`, `getIncognitoState: GetAnimeIncognitoState`, `libraryPreferences: LibraryPreferences`, `uiPreferences: UiPreferences` (not a property; constructor parameter used in init-time property initialisers at 1114–1115).

| Line | Member | Kind | Purpose (one line) |
|---|---|---|---|
| 180–181 | `_currentPlaylist` / `currentPlaylist` | st `List<Episode>` | Episodes shown in the player's episode list after filtering |
| 183–184 | `_hasPreviousEpisode` / `hasPreviousEpisode` | st Boolean | Whether a previous episode exists in playlist |
| 186–187 | `_hasNextEpisode` / `hasNextEpisode` | st Boolean | Whether a next episode exists |
| 189–190 | `_currentEpisode` / `currentEpisode` | st `Episode?` | Episode being played |
| 192–193 | `_currentAnime` / `currentAnime` | st `Anime?` | Anime being played |
| 195–196 | `_currentSource` / `currentSource` | st `AnimeSource?` | Resolved source |
| 198–199 | `_isEpisodeOnline` / `isEpisodeOnline` | st Boolean | Online (non-downloaded) flag |
| 201–202 | `_isLoadingEpisode` / `isLoadingEpisode` | st Boolean | Episode switch in progress |
| 204–205 | `_currentDecoder` / `currentDecoder` | st `Decoder` | Current hwdec decoder |
| 207 | `mediaTitle` | st String | Episode name (media title) |
| 208 | `animeTitle` | st String | Anime title |
| 210 | `isLoading` | st Boolean (init true) | Generic loading flag |
| 211 | `playbackSpeed` | st Float | Initialised from `playerSpeed()` |
| 213–214 | `_subtitleTracks` / `subtitleTracks` | st `List<VideoTrack>` | Subtitle tracks from mpv |
| 215–216 | `_selectedSubtitles` / `selectedSubtitles` | st `Pair<Int,Int>` (init -1,-1) | (primary sid, secondary sid) |
| 218–219 | `_audioTracks` / `audioTracks` | st `List<VideoTrack>` | Audio tracks (first entry is "Off") |
| 220–221 | `_selectedAudio` / `selectedAudio` | st Int (init -1) | Selected audio track id |
| 223 | `isLoadingTracks` | st Boolean (init true) | Track loading flag |
| 225–226 | `_hosterList` / `hosterList` | st `List<Hoster>` | Hoster list for the episode |
| 227–228 | `_isLoadingHosters` / `isLoadingHosters` | st Boolean (init true) | Hoster loading flag |
| 229–230 | `_hosterState` / `hosterState` | st `List<HosterState>` | Per hoster state |
| 231–232 | `_hosterExpandedList` / `hosterExpandedList` | st `List<Boolean>` | Expanded flags for hoster accordion |
| 233–234 | `_selectedHosterVideoIndex` / `selectedHosterVideoIndex` | st `Pair<Int,Int>` (-1,-1) | (hosterIndex, videoIndex) |
| 235–236 | `_currentVideo` / `currentVideo` | st `Video?` | Currently loaded video |
| 238–239 | `_chapters` / `chapters` | st `List<IndexedSegment>` | Chapters (mpv + external) |
| 240–241 | `_currentChapter` / `currentChapter` | st `IndexedSegment?` | Chapter containing current position |
| 242–243 | `_skipIntroText` / `skipIntroText` | st `String?` | Skip-intro button label |
| 245–246 | `_pos` / `pos` | st Float | Playback position (seconds) |
| 248–249 | `_seekPosition` / `seekPosition` | st Float | Scrub target position |
| 251–252 | `_isSeeking` / `isSeeking` | st Boolean | Scrubbing in progress |
| 254–255 | `_thumbnailImage` / `thumbnailImage` | st `ImageBitmap?` | Seek preview thumbnail |
| 257 | `thumbnailInfo` | priv st `ThumbnailInfo?` | Tile metadata for seek preview |
| 258–261 | `thumbnailTileCache` | priv `LinkedHashMap<Int,Bitmap>(4, 0.75f, true)` anonymous subclass, `removeEldestEntry` = `size > 3` | LRU cache of tile bitmaps (max 3) |
| 262 | `thumbnailFetchJob` | priv Job? | Current tile fetch |
| 264 | `duration` | st Float | Duration (seconds) |
| 266–267 | `_readAhead` / `readAhead` | st Float | demuxer-cache-time |
| 269–270 | `_paused` / `paused` | st Boolean | Pause state |
| 273–274 | `_pausedState` / `pausedState` | st `Boolean?` (init false) | Pending pause state to apply after load |
| 276–277 | `_controlsShown` / `controlsShown` | st Boolean (init `!hideControls()`) | Controls visibility |
| 278–279 | `_seekBarShown` / `seekBarShown` | st Boolean (init `!hideControls()`) | Seek bar visibility |
| 280–281 | `_areControlsLocked` / `areControlsLocked` | st Boolean | Lock state |
| 283 | `playerUpdate` | st `PlayerUpdates` | One-shot UI update message (None, DoubleSpeed, AspectRatio, ShowText, ShowTextResource) |
| 284 | `isBrightnessSliderShown` | st Boolean | Brightness slider visibility |
| 285 | `isVolumeSliderShown` | st Boolean | Volume slider visibility |
| 286–291 | `currentBrightness` | st Float | Initial value = `Settings.System.SCREEN_BRIGHTNESS` normalised 0..255 → 0..1, fallback 0f |
| 292 | `currentVolume` | st Int | System music stream volume |
| 293 | `currentMPVVolume` | st Int? | mpv `volume` |
| 294 | `volumeBoostCap` | var Int | mpv `volume-max` read at construction; later set from observer as `value - 100` (PlayerActivity.kt:676) |
| 297 | `gestureSeekAmount` | st `Pair<Int,Int>?` | (starting position, seek amount) for gesture seeking |
| 299 | `sheetShown` | st `Sheets` | Open bottom sheet |
| 300 | `panelShown` | st `Panels` | Open panel |
| 301 | `dialogShown` | st `Dialogs` | Open dialog |
| 303–304 | `_dismissSheet` / `dismissSheet` | st Boolean | Request to dismiss sheet |
| 306–307 | `_seekText` / `seekText` | st `String?` | Text overlay for seek (e.g. skip messages) |
| 308–309 | `_doubleTapSeekAmount` / `doubleTapSeekAmount` | st Int | Accumulated double-tap seek amount (seconds) |
| 310–311 | `_isSeekingForwards` / `isSeekingForwards` | st Boolean | Direction of last seek |
| 313 | `timerJob` | priv Job? | Sleep timer job |
| 314–315 | `_remainingTime` / `remainingTime` | st Int | Sleep timer remaining seconds |
| 317 | `cachePath` | val String | `activity.cacheDir.path` |
| 319–320 | `_customButtons` / `customButtons` | st `CustomButtonFetchState` (init Loading) | Custom buttons fetch state |
| 322–323 | `_primaryButtonTitle` / `primaryButtonTitle` | st String | Label for primary custom button |
| 325–326 | `_primaryButton` / `primaryButton` | st `CustomButton?` | Visible primary custom button |
| 328–347 | `init {}` block | — | Loads custom buttons (see behavior) |
| 352 | `startTimer(seconds: Int)` | fun | Sleep timer |
| 366 | `isEpisodeOnline(): Boolean?` | fun | Online-ness check |
| 377 | `updateIsLoadingEpisode(value)` | fun | Sets `_isLoadingEpisode` |
| 381 | `updateEpisodeList(episodeList)` | priv fun | Applies `filterEpisodeList` to playlist |
| 385 | `getDecoder()` | fun | Reads `hwdecActive` |
| 389 | `updateDecoder(decoder: Decoder)` | fun | Writes mpv `hwdec` |
| 393 | `getTrackLanguage: (Int)->String` | val lambda | mpv `track-list/$it/lang` or "Off" |
| 400 | `getTrackTitle: (Int)->String` | val lambda | mpv `track-list/$it/title` or "Off" |
| 407 | `getTrackMPVId: (Int)->Int` | val lambda | mpv `track-list/$it/id` or -1 |
| 414 | `getTrackType: (Int)->String?` | val lambda | mpv `track-list/$it/type` |
| 418 | `trackLoadingJob` | priv Job? | Track load job |
| 419 | `loadTracks()` | fun | Builds audio/subtitle lists from mpv track-list |
| 455 | `onFinishLoadingTracks()` | fun | Applies preferred tracks |
| 473–477 | `data class VideoTrack(id: Int, name: String, language: String?)` (annotated `@Immutable`) | nested data class | Track DTO |
| 479 | `loadChapters()` | fun | Reads mpv chapter-list |
| 496 | `updateChapters(chapters)` | fun | Sets chapter list |
| 500 | `selectChapter(index)` | fun | Seeks to chapter start |
| 505 | `updateChapter(index: Long)` | fun | Sets current chapter from index |
| 510 | `addAudio(uri: Uri)` | fun | mpv `audio-add` |
| 523 | `selectAudio(id: Int)` | fun | Sets mpv `aid` |
| 527 | `updateAudio(id: Int)` | fun | Sets `_selectedAudio` |
| 531 | `addSubtitle(uri: Uri)` | fun | mpv `sub-add` |
| 544 | `selectSub(id: Int)` | fun | Primary/secondary subtitle toggle |
| 563 | `updateSubtitle(sid, secondarySid)` | fun | Sets `_selectedSubtitles` |
| 567 | `updatePlayBackPos(pos: Float)` | fun | Calls `onSecondReached` and sets `_pos` |
| 572 | `lastThumbnailFetch` | priv var Long | Throttle timestamp |
| 574 | `updateSeekPos(pos: Float)` | fun | Scrub update + thumbnail |
| 610 | `createThumbnail(tileBitmap, tileInfo)` | priv fun | Crops tile |
| 615 | `updateIsSeeking(value)` | fun | Scrub flag, clears thumbnail on end |
| 622 | `updateReadAhead(value: Long)` | fun | Sets readAhead |
| 626 | `updatePausedState()` | priv fun | Captures pause state before load |
| 632 | `setPausedState()` | priv fun | Applies pending pause state |
| 644 | `pauseUnpause()` | fun | Toggles |
| 652 | `pause()` | fun | Sets mpv paused, updates PiP params |
| 660 | `unpause()` | fun | Clears mpv paused |
| 665 | `showStatusBar` | priv val Boolean | `showSystemStatusBar()` snapshot |
| 666 | `showControls()` | fun | Shows controls unless sheet/panel/dialog open |
| 679 | `hideControls()` | fun | Hides status bar, controls |
| 684 | `hideSeekBar()` | fun | Hides seek bar |
| 688 | `showSeekBar()` | fun | Shows seek bar unless sheet open |
| 693 | `lockControls()` | fun | Locks |
| 697 | `unlockControls()` | fun | Unlocks |
| 701 | `dismissSheet()` | fun | Sets `_dismissSheet` true |
| 705 | `resetDismissSheet()` | priv fun | Sets `_dismissSheet` false |
| 709 | `showSheet(sheet: Sheets)` | fun | Sheet exclusivity |
| 721 | `showPanel(panel: Panels)` | fun | Panel exclusivity |
| 732 | `showDialog(dialog: Dialogs)` | fun | Dialog exclusivity |
| 743 | `seekBy(offset: Int, precise: Boolean = false)` | fun | mpv `seek` relative |
| 747 | `seekTo(position: Int, precise: Boolean = true)` | fun | mpv `seek` absolute with range check |
| 752 | `changeBrightnessTo(brightness: Float)` | fun | Window brightness |
| 761 | `displayBrightnessSlider()` | fun | Shows brightness slider |
| 765 | `maxVolume` | val Int | `getStreamMaxVolume(STREAM_MUSIC)` |
| 766 | `changeVolumeBy(change: Int)` | fun | Volume with boost |
| 779 | `changeVolumeTo(volume: Int)` | fun | System volume |
| 789 | `changeMPVVolumeTo(volume: Int)` | fun | mpv volume |
| 793 | `setMPVVolume(volume: Int)` | fun | Sets currentMPVVolume and shows slider on change |
| 798 | `displayVolumeSlider()` | fun | Shows volume slider |
| 802 | `setAutoPlay(value: Boolean)` | fun | Shows text and persists auto-play |
| 813 | `changeVideoAspect(aspect: VideoAspect)` | fun | Aspect modes |
| 839 | `cycleScreenRotations()` | fun | Android orientation cycle |
| 856 | `handleLuaInvocation(property: String, value: String)` | fun | Lua `user-data/aniyomi` dispatcher |
| 963 | `component6` | priv operator extension on `List<T>` returning `get(5)` | Member extension that shadows the stdlib `component6` inside the class; used by the 6-way destructuring of the `launch_int_picker` payload (line 901). Behaviour identical to stdlib (throws IndexOutOfBounds on short input) |
| 965 | `inputMethodManager` | priv val | Android IME service |
| 966 | `forceShowSoftwareKeyboard()` | priv fun | `toggleSoftInput(SHOW_FORCED, 0)` |
| 970 | `forceHideSoftwareKeyboard()` | priv fun | `toggleSoftInput(SHOW_IMPLICIT, 0)` |
| 974 | `doubleTapToSeekDuration` | priv val Int | `skipLengthPreference()` snapshot |
| 975 | `preciseSeek` | priv val Boolean | `playerSmoothSeek()` snapshot |
| 976 | `showSeekBar` | priv val Boolean | `showSeekBar()` snapshot |
| 978 | `seekToWithText(seekValue, text)` | priv fun | Seek with overlay text |
| 986 | `seekByWithText(value, text)` | priv fun | Relative seek with overlay text |
| 994 | `updateSeekAmount(amount)` | fun | Sets double-tap amount |
| 998 | `updateSeekText(value)` | fun | Sets seek text |
| 1002 | `leftSeek()` | fun | Left double-tap seek |
| 1011 | `rightSeek()` | fun | Right double-tap seek |
| 1023 | `resetState()` | fun | Reset when switching episode |
| 1034 | `changeEpisode(previous, autoPlay = false)` | fun | Prev/next |
| 1051 | `handleLeftDoubleTap()` | fun | Dispatch |
| 1067 | `handleCenterDoubleTap()` | fun | Dispatch |
| 1081 | `handleRightDoubleTap()` | fun | Dispatch |
| 1097 | `onCleared()` (override) | fun | Final save |
| 1108–1109 | `eventChannel` (priv `Channel<Event>()`) / `eventFlow` | val | One-shot events |
| 1111 | `incognitoMode` | priv val by lazy Boolean | `getIncognitoState.await(currentAnime.source)` evaluated once |
| 1112 | `downloadAheadAmount` | priv val Int | `autoDownloadWhileWatching()` snapshot |
| 1114 | `relativeTime` | internal val Boolean | `relative_time_v2` snapshot |
| 1115 | `dateFormat` | internal val DateTimeFormatter | `UiPreferences.dateFormat(...)` snapshot |
| 1120–1124 | `episodePosition` | priv var Long via `savedState["episode_position"]` | Restore-from-process-death position (ms) |
| 1129–1133 | `qualityIndex` | priv var Pair<Int,Int> via `savedState["quality_index"]` | Restore selected hoster/video |
| 1138–1142 | `episodeId` | priv var Long via `savedState["episode_id"]` (default -1) | Current episode id |
| 1144 | `episodeToDownload` | priv var `AnimeDownload?` | Never assigned non-null in this file (see edge cases) |
| 1146 | `filterEpisodeList(episodes)` | priv fun | Playlist filtering (see behavior) |
| 1187 | `getCurrentEpisodeIndex()` | fun Int | Index in playlist |
| 1191 | `getAdjacentEpisodeId(previous)` | priv fun Long | Prev/next id or -1 |
| 1201 | `updateHasNextEpisode(value)` | fun | Sets flag |
| 1205 | `updateHasPreviousEpisode(value)` | fun | Sets flag |
| 1209 | `showEpisodeListDialog()` | fun | Opens EpisodeList dialog |
| 1219 | `onSaveInstanceStateNonConfigurationChange()` | fun | Saves progress on activity save |
| 1228 | `updateIsLoadingHosters(value)` | fun | Sets flag |
| 1235 | `needsInit(animeId, episodeId)` | priv fun Boolean | Init check |
| 1239–1243 | `data class InitResult(hosterList: List<Hoster>?, videoIndex: Pair<Int,Int>, position: Long?)` | nested | Init result |
| 1245 | `currentHosterList` | priv var `List<Hoster>?` | Cached hoster list |
| 1247–1250 | `class ExceptionWithStringResource(message: String, stringResource: StringResource) : Exception` | nested | User-facing error with string key |
| 1252 | `init(animeId, initialEpisodeId, hostList, hostIndex, vidIndex): Pair<InitResult, Result<Boolean>>` | suspend fun | Entry point for player activity |
| 1330 | `updateEpisode(episode)` | priv fun | Sets mediaTitle, isEpisodeOnline, mpv episode-number |
| 1336 | `initEpisodeList(anime)` | priv fun | Sorted/filtered list |
| 1351 | `hasTrackers` | priv var Boolean | Tracker presence |
| 1352 | `checkTrackers: (Anime)->Unit` | priv val lambda | Sets hasTrackers |
| 1357 | `getHosterVideoLinksJob` | priv var Job? | Hoster loading job |
| 1359 | `cancelHosterVideoLinksJob()` | fun | Cancels |
| 1366 | `loadHosters(source, hosterList, hosterIndex, videoIndex)` | fun | Starts per-hoster loading |
| 1459 | `loadVideo(source, video, hosterIndex, videoIndex): Boolean` | priv suspend fun | Resolves and sets a video |
| 1528 | `updateVideo(video)` | fun | Sets currentVideo |
| 1532 | `onVideoClicked(hosterIndex, videoIndex)` | fun | User picked a video |
| 1558 | `onHosterClicked(index)` | fun | Expand or lazily load hoster |
| 1580 | `loadThumbnails(video, source)` | suspend fun | Thumbnail metadata + first 2 tiles |
| 1607 | `MutableStateFlow<List<T>>.updateAt(index, newValue)` | priv extension | Immutable list set |
| 1615–1619 | `data class EpisodeLoadResult(hosterList, episodeTitle, source)` | nested | Result of loadEpisode |
| 1621 | `loadEpisode(episodeId: Long?): EpisodeLoadResult?` | suspend fun | Switch episode |
| 1658 | `onSecondReached(position, duration)` | priv fun | Per-second hook |
| 1688 | `updateEpisodeProgressOnComplete(currentEp)` | priv suspend fun | Mark seen side effects |
| 1712 | `downloadNextEpisodes()` | priv fun | Download-ahead |
| 1740 | `deleteEpisodeIfNeeded(chosenEpisode)` | priv fun | Queue delete of read episode |
| 1756 | `saveCurrentEpisodeWatchingProgress()` | fun | Explicit save |
| 1763 | `saveWatchingProgress(episode)` | priv fun | Progress + history |
| 1774 | `saveEpisodeProgress(episode)` | priv suspend fun | DB update |
| 1792 | `saveEpisodeHistory(episode)` | priv suspend fun | History upsert |
| 1805 | `bookmarkEpisode(episodeId, bookmarked)` | fun | Bookmark |
| 1819 | `fillermarkEpisode(episodeId, fillermarked)` | fun | Fillermark |
| 1830 | `takeScreenshot(cachePath, showSubtitles): InputStream?` | fun | mpv screenshot-to-file |
| 1847 | `saveImage(imageStream, timePos)` | fun | Save to Pictures |
| 1886 | `shareImage(imageStream, timePos)` | fun | Share via cache |
| 1915 | `setAsArt(artType, imageStream)` | fun | Set cover/background/thumbnail |
| 1942–1945 | `sealed class SaveImageResult` with `class Success(uri: Uri)`, `class Error(error: Throwable)` | nested | Save result |
| 1947 | `updateTrackEpisodeSeen(episode)` | priv fun | Tracker sync on completion |
| 1963 | `enqueueDeleteSeenEpisodes(episode)` | priv fun | Enqueue delete |
| 1975 | `deletePendingEpisodes()` | fun | Flush delete queue |
| 1984 | `getAnimeSkipIntroLength(): Int` | fun | Per-anime or default intro length |
| 1999 | `setAnimeSkipIntroLength(skipIntroLength: Long)` | fun | Persist per-anime intro length |
| 2013 | `generateFilename(anime, timePos)` | priv fun String? | Screenshot filename |
| 2031 | `aniSkipResponse(playerDuration: Int?): List<TimeStamp>?` | suspend fun | AniSkip lookup |
| 2056 | `introSkipEnabled` | val Boolean | `enableSkipIntro()` snapshot |
| 2057 | `autoSkip` | priv val Boolean | `autoSkipIntro()` snapshot |
| 2058 | `netflixStyle` | priv val Boolean | `enableNetflixStyleIntroSkip()` snapshot |
| 2060 | `defaultWaitingTime` | priv val Int | `waitingTimeIntroSkip()` snapshot |
| 2061 | `waitingSkipIntro` | var Int | Countdown (-1 = cancelled) |
| 2063 | `setChapter(position: Float)` | fun | Per-second skip-intro state machine |
| 2104 | `updateSkipIntroButton(chapterType)` | priv fun | Sets skip-intro label |
| 2117 | `showSkipIntroButton(chapter, nextChapterPos, waitingTime)` | priv fun | Netflix countdown display/seek |
| 2133 | `onSkipIntro()` | fun | Manual skip / cancel countdown |
| 2150 | `getCurrentChapter(position: Float? = null)` | priv fun IndexedValue? | Chapter at position |
| 2156 | `setPrimaryCustomButtonTitle(button)` | fun | Title from button name |
| 2160–2164 | `sealed class Event` with `SetArtResult(result: SetAsArt, artType: ArtType)`, `SavedImage(result: SaveImageResult)`, `ShareImage(uri: Uri, seconds: String)` | nested | One-shot events |

Companion objects: none declared in `PlayerViewModel`.

## Table B — other scope files, declarations

- `EpisodeLoader.kt`: `class EpisodeLoader` — `companion object` members as listed in the summary row 2. Body is entirely within the companion object.
- `HosterLoader.kt`: `class HosterLoader` — `companion object` members as listed in row 3. `class EarlyReturnException(val video: Video) : Exception()` is nested in the companion.
- `AdvancedPlayerPreferences.kt`: `class AdvancedPlayerPreferences(preferenceStore)`; funs `mpvUserFiles()`, `mpvConf()`, `mpvInput()`, `playerStatisticsPage()`.
- `AudioPreferences.kt`: `class AudioPreferences`; funs `preferredAudioLanguages()`, `enablePitchCorrection()`, `audioChannels()`, `volumeBoostCap()`, `audioDelay()`; `enum class AudioChannels(titleRes: StringResource, property: String, value: String)` with entries `Auto`, `AutoSafe`, `Mono`, `Stereo`, `ReverseStereo`.
- `DecoderPreferences.kt`: `class DecoderPreferences`; funs `tryHWDecoding()`, `gpuNext()`, `videoDebanding()`, `useYUV420P()`, `brightnessFilter()`, `saturationFilter()`, `contrastFilter()`, `gammaFilter()`, `hueFilter()`.
- `GesturePreferences.kt`: `class GesturePreferences`; funs `gestureVolumeBrightness()`, `swapVolumeBrightness()`, `gestureHorizontalSeek()`, `showSeekBar()`, `defaultIntroLength()`, `skipLengthPreference()`, `playerSmoothSeek()`, `leftDoubleTapGesture()`, `centerDoubleTapGesture()`, `rightDoubleTapGesture()`, `mediaPreviousGesture()`, `mediaPlayPauseGesture()`, `mediaNextGesture()`.
- `PlayerPreferences.kt`: `class PlayerPreferences`; 35 funs as listed in row 8.
- `SubtitlePreferences.kt`: `class SubtitlePreferences`; 21 funs as listed in row 9; `enum class SubtitleJustification(value: String, icon: ImageVector)` with `Left("left")`, `Center("center")`, `Right("right")`, `Auto("auto")`.
- `AniSkipApi.kt`: `class AniSkipApi` (private `client: OkHttpClient`, private `json: Json` via `injectLazy()`, `fun getResult(malId: Int, episodeNumber: Int, episodeLength: Long): List<TimeStamp>?`, `fun getMalIdFromAL(id: Long): Long`); `@Serializable data class AniSkipResponse(found: Boolean, results: List<Stamp>?)`; `@Serializable data class Stamp(interval: AniSkipInterval, skipType: SkipType)`; `@Serializable enum class SkipType { OP, ED, RECAP, MIXED_OP; fun getString(): String; fun toChapterType(): ChapterType }`; `@Serializable data class AniSkipInterval(startTime: Double, endTime: Double)`.
- `ChapterUtils.kt`: `class ChapterUtils`; `companion object` with `fun ChapterType.getStringRes(): StringResource?` and `fun mergeChapters(currentChapters: List<IndexedSegment>, stamps: List<TimeStamp>, duration: Int? = null): List<IndexedSegment>`.
- `TrackSelect.kt`: `class TrackSelect(subtitlePreferences, audioPreferences)`; `fun getPreferredTrackIndex(tracks: List<VideoTrack>, subtitle: Boolean = true): VideoTrack?`; `private fun containsLang(track: VideoTrack, locale: Locale): Boolean`.

## Notes on port decisions

- `AniSkipApi` is Replace-with-crate: OkHttp+kotlinx.serialization becomes reqwest+serde. The Kotlin code deliberately builds its own `OkHttpClient()` (not the app client); the Rust port uses the shared client, which is a behaviour change recorded as T-45b-020 acceptance.
- `PlayerViewModel` is a Rewrite: Kotlin `MutableStateFlow` + `viewModelScope` + `Injekt` singletons + `SavedStateHandle` become one `PlayerModel` owned by the GTK main thread, with IO work on tokio and results delivered back over a `glib` channel. Persisted "process-death" values (`episodePosition`, `qualityIndex`, `episodeId`) move to a small on-disk session file under `$XDG_STATE_HOME/kurobayomi/player-session.json` (T-45b-030).
- Android-only members (`AudioManager`, `WindowInsetsCompat`, `InputMethodManager`, `Settings.System.SCREEN_BRIGHTNESS`, `ActivityInfo` orientation, `DisplayMetrics`, `Bitmap`/`ImageBitmap`, `Uri.openContentFd`) are Dropped or replaced as described in `p2-rust-design.md` section 7.
- Table A row `component6` is only the destructuring helper for the `launch_int_picker` payload; the Rust port parses that payload with an explicit length check (edge case E-53).
