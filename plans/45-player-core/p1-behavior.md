# Area 45 (player core, part 1): behaviour specification

Every path is repo-relative to `/home/blackbox/aniyomi-src`. "OBSERVED" marks what the Kotlin does. "Proposal:" marks what the Rust port should do. Section numbers 2.2, 3.4 and 5 are referenced from `p1-tasks.md`.

Part-2 identifiers (`PlayerViewModel` methods, loaders, AniSkip, ChapterUtils, preference classes such as `SubtitleSettingsColorsCard.kt`) are named but their internals are specified in the part-2 documents.

---

## 1. Activity lifecycle and state machine

### 1.1 Entry points

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:157-181`):

- `PlayerActivity.newIntent(context, animeId: Long?, episodeId: Long?, hostList: List<Hoster>? = null, hostIndex: Int? = null, vidIndex: Int? = null): Intent`.
- Extras: `animeId` (Long), `episodeId` (Long), `hostIndex` (Int, only if non-null), `vidIndex` (Int, only if non-null), `hostList` (String, `List<Hoster>.serialize()`, only if non-null).
- Flag: `Intent.FLAG_ACTIVITY_CLEAR_TOP`.
- Launchers: `eu.kanade.tachiyomi.ui.main.MainActivity.startPlayerActivity` (outside scope, `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:600-643`). With `extPlayer == true` it first calls `startHttpServerService(context, sourceId)` (5 s timeout), then `ExternalIntents.newIntent`; otherwise `context.startActivity(PlayerActivity.newIntent(...))`.
- Manifest (`app/src/main/AndroidManifest.xml:206-212`): `autoRemoveFromRecents="true"`, `configChanges="orientation|screenLayout|screenSize|smallestScreenSize|keyboardHidden|keyboard|uiMode"`, `exported="false"`, `launchMode="singleTask"`, `resizeableActivity="true"`, `supportsPictureInPicture="true"`.

Proposal: a single GTK window (`PlayerWindow`) receives the same parameters through a Rust `PlayerRequest { anime_id: i64, episode_id: i64, host_list: Option<Vec<Hoster>>, host_index: Option<i32>, video_index: Option<i32> }`. Missing ids (`-1`) close the window (see 1.2).

### 1.2 onNewIntent (the real entry for every episode open)

OBSERVED (`PlayerActivity.kt:183-230`). `onCreate` ends by calling `onNewIntent(this.intent)`, so this is also the first load.

1. Read `animeId` (`extras.getLong`, default -1), `episodeId` (default -1), `hostList` (String, default ""), `hostIndex` (Int, default -1), `vidIndex` (Int, default -1).
2. If `animeId == -1L || episodeId == -1L`: `finish()` and return.
3. `NotificationReceiver.dismissNotification(this, animeId.hashCode(), Notifications.ID_NEW_EPISODES)`.
4. `viewModel.saveCurrentEpisodeWatchingProgress()`.
5. In `lifecycleScope.launchNonCancellable`:
   a. `viewModel.updateIsLoadingEpisode(true)`, `viewModel.updateIsLoadingHosters(true)`.
   b. `initResult = viewModel.init(animeId, episodeId, hostList, hostIndex, vidIndex)` returns a pair `(InitResult, Result<Boolean>)`.
   c. If `initResult.second.getOrDefault(false)` is false: exception = `initResult.second.exceptionOrNull() ?: IllegalStateException("Unknown error")`; on UI context call `setInitialEpisodeError(exception)` (which closes the window, see 13.1).
   d. `viewModel.updateIsLoadingHosters(false)`.
   e. `lifecycleScope.launch { viewModel.loadHosters(source = viewModel.currentSource.value!!, hosterList = initResult.first.hosterList ?: emptyList(), hosterIndex = initResult.first.videoIndex.first, videoIndex = initResult.first.videoIndex.second) }`.
6. `setIntent(intent)`.

Edge: step 5e uses `!!` on `currentSource`; if init failed this may throw. Proposal: skip 5e when init failed.

### 1.3 onCreate

OBSERVED (`PlayerActivity.kt:232-295`), in this order:

1. `enableEdgeToEdge()`, `registerSecureActivity(this)`, `super.onCreate(savedInstanceState)`, `setContentView(binding.root)`.
2. `setupPlayerMPV()` (section 2.1).
3. `setupPlayerAudio()` (section 9).
4. `setupMediaSession()` (section 11).
5. `setupPlayerOrientation()` (section 12, dropped on desktop).
6. `Thread.setDefaultUncaughtExceptionHandler { ... }` (process-wide, closes the activity; Drop, see `p1-edge-cases-and-tests.md`).
7. `viewModel.eventFlow.onEach { ... }.launchIn(lifecycleScope)`: `SavedImage` -> `onSaveImageResult`, `ShareImage` -> `onShareImageResult(uri, seconds)`, `SetArtResult` -> `onSetAsArtResult(result, artType)`.
8. `binding.controls.setContent { TachiyomiTheme { PlayerControls(viewModel, onBackPress = { ... }, modifier = Modifier.onGloballyPositioned { pipRect = boundsInWindow as Rect }) } }`. `onBackPress`: if `isPipSupportedAndEnabled && player.paused == false && pipOnExit` then `enterPictureInPictureMode(createPipParams())`, else `finish()`.
9. `onNewIntent(this.intent)`.

### 1.4 onStart

OBSERVED (`PlayerActivity.kt:379-410`):

1. `super.onStart()`, `setPictureInPictureParams(createPipParams())`.
2. `WindowCompat.setDecorFitsSystemWindows(window, false)`.
3. `window.setFlags(FLAG_LAYOUT_NO_LIMITS, FLAG_LAYOUT_NO_LIMITS)`.
4. `window.addFlags(FLAG_KEEP_SCREEN_ON)`.
5. `binding.root.systemUiVisibility = SYSTEM_UI_FLAG_IMMERSIVE_STICKY | SYSTEM_UI_FLAG_LAYOUT_STABLE | SYSTEM_UI_FLAG_HIDE_NAVIGATION | SYSTEM_UI_FLAG_LOW_PROFILE`.
6. Hide `systemBars()` and `navigationBars()`; behaviour `BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE`; `FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS`.
7. API 28+: `layoutInDisplayCutoutMode = SHORT_EDGES` if `player_fullscreen` (default true) else `NEVER`.
8. If `pref_remember_brightness`: `viewModel.changeBrightnessTo(stored)` when stored != -1f.

Desktop mapping (Proposal): steps 1, 4 and 6 become `gtk::Window::fullscreen()` when `player_fullscreen` is true; step 8 is dropped (see T-45a-045).

### 1.5 onResume

OBSERVED (`PlayerActivity.kt:634-648`):

- If `!player.isExiting`: `super.onResume()` and return.
- Else `player.isExiting = false`, `super.onResume()`, then `viewModel.currentVolume.update { audioManager.getStreamVolume(STREAM_MUSIC).also { if (it < viewModel.maxVolume) viewModel.changeMPVVolumeTo(100) } }`.

### 1.6 onPause

OBSERVED (`PlayerActivity.kt:325-342`):

1. `viewModel.saveCurrentEpisodeWatchingProgress()` (always).
2. If `isInPictureInPictureMode`: `super.onPause()` and return (playback continues).
3. `player.isExiting = true`.
4. If `isFinishing`: `viewModel.deletePendingEpisodes()` and `MPVLib.command(arrayOf("stop"))`.
5. Else `viewModel.pause()`.
6. `super.onPause()`.

Note: because step 3 is unconditional for non-PiP pauses, `isExiting` is true for any pause until `PLAYBACK_RESTART` (section 6.4) or `onResume`.

### 1.7 onStop

OBSERVED (`PlayerActivity.kt:344-356`):

1. If `pref_remember_brightness` and `window.attributes.screenBrightness != -1f`: store it in `player_brightness_value` (float).
2. If `isInPictureInPictureMode && powerManager.isInteractive`: `viewModel.deletePendingEpisodes()`.
3. `super.onStop()`.

### 1.8 onDestroy

OBSERVED (`PlayerActivity.kt:297-323`), in order:

1. `player.isExiting = true`.
2. `httpServer?.stop()`, `httpServer = null`.
3. If audio focus request exists: abandon it (section 9).
4. `mediaSession.isActive = false`, `release()`.
5. If `noisyReceiver.initialized`: unregister and set false.
6. `MPVLib.removeLogObserver(playerObserver)`, `MPVLib.removeObserver(playerObserver)`.
7. `player.destroy()`.
8. `super.onDestroy()`.

### 1.9 onUserLeaveHint and onBackPressed

OBSERVED:

- `onUserLeaveHint` (`PlayerActivity.kt:358-363`): if PiP supported and enabled, not paused (`player.paused == false`) and `pip_on_exit` true: `enterPictureInPictureMode()`.
- `onBackPressed` (`PlayerActivity.kt:365-377`, deprecated): if the same PiP condition holds and no sheet, panel or dialog is open (`Sheets.None`, `Panels.None`, `Dialogs.None`): enter PiP; otherwise `super.onBackPressed()`.

### 1.10 onSaveInstanceState

OBSERVED (`PlayerActivity.kt:990-995`): if not `isChangingConfigurations`, call `viewModel.onSaveInstanceStateNonConfigurationChange()`.

### 1.11 onConfigurationChanged

OBSERVED (`PlayerActivity.kt:650-657`): when not in PiP, `viewModel.changeVideoAspect(playerPreferences.aspectState().get())`; when in PiP, `viewModel.hideControls()`. Then `super`.

### 1.12 Window state machine (Proposal, derived from 1.2-1.11)

States: `Loading` (init or switch in progress), `Playing`, `Paused`, `Exiting` (set by onPause, cleared by PLAYBACK_RESTART or onResume), `Closed`.

- `Loading` -> `Playing` on `FILE_LOADED` followed by `pause=false`.
- `Playing` -> `Paused` on `pause=true` event.
- `Playing|Paused` -> `Exiting` on window hide.
- `Exiting` -> `Playing|Paused` on `onResume` only (PLAYBACK_RESTART is ignored while exiting, section 6.4).
- Any -> `Closed` on close (`isFinishing`) or on `setInitialEpisodeError`.

---

## 2. mpv initialisation

### 2.1 setupPlayerMPV

OBSERVED (`PlayerActivity.kt:424-448`):

1. `logLevel = if (networkPreferences.verboseLogging()) "info" else "warn"`.
2. `mpvDir = filesDir/mpv` (created).
3. Write `mpv.conf` from `advancedPlayerPreferences.mpvConf()` and `input.conf` from `mpvInput()` (truncate-write; empty string writes an empty file).
4. `copyUserFiles(mpvDir)` (section 2.5).
5. `copyAssets(mpvDir)` (section 2.6).
6. `copyFontsDirectory(mpvDir)` (section 2.7, async).
7. `MPVLib.setOptionString("sub-ass-force-margins", "yes")`, `("sub-use-margins", "yes")`.
8. `player.initialize(configDir = mpvDir.filePath, cacheDir = applicationContext.cacheDir.path, logLvl = logLevel)`.
9. `MPVLib.addLogObserver(playerObserver)`, `MPVLib.addObserver(playerObserver)`.

Note: the mpv config directory is `<filesDir>/mpv` (constant `MPV_DIR = "mpv"`). Desktop path: `$XDG_DATA_HOME/kurobayomi/mpv` (data doc).

### 2.2 initOptions (the option table)

OBSERVED (`AniyomiMPVView.kt:114-156`). Order is the source order. `vo` is set by `setVo` (BaseMPVView, external).

| Step | Call | Value | Condition / source preference |
|---|---|---|---|
| 1 | `setVo` | `"gpu-next"` or `"gpu"` | `pref_gpu_next` (default false) selects gpu-next |
| 2 | `MPVLib.setPropertyBoolean("pause", true)` | yes | always |
| 3 | `setOptionString("profile", "fast")` | fast | always |
| 4 | `setOptionString("hwdec", ...)` | `"auto"` or `"no"` | `pref_try_hwdec` (default true) |
| 5 | debanding | see 2.3 | `pref_video_debanding` (default `None`) |
| 6 | `setOptionString("vf", "format=yuv420p")` | | `use_yuv420p` (default true) |
| 7 | `setOptionString("msg-level", "all=" + (v or warn))` | | `verbose_logging` |
| 8 | `setPropertyBoolean("keep-open", true)` | yes | always |
| 9 | `setPropertyBoolean("input-default-bindings", true)` | yes | always |
| 10 | `setOptionString("ytdl", "no")` | no | always |
| 11 | `setOptionString("tls-verify", "yes")` | yes | always |
| 12 | `setOptionString("tls-ca-file", "<filesDir>/mpv/cacert.pem")` | | always |
| 13 | `demuxer-max-bytes`, `demuxer-max-back-bytes` | `"67108864"` | Android SDK >= O_MR1 branch; Linux value is always 64 MiB (see 2.4) |
| 14 | `screenshot-directory` | Pictures dir, `mkdirs()` | always |
| 15 | per `VideoFilters` entry: `mpvProperty` = stored int | `brightness`, `saturation`, `contrast`, `gamma`, `hue` | defaults from stored values |
| 16 | `speed` | `pref_player_speed` as string (default `1.0`) | `playerSpeed()` default `1f` |
| 17 | `vd-lavc-film-grain` | `"cpu"` | always (mpv issue 14651 workaround) |
| 18 | `setupSubtitlesOptions()` | section 4.3 | |
| 19 | `setupAudioOptions()` | section 4.2 | |

Defect (OBSERVED): steps 5 and 6 both call `setOptionString("vf", ...)`. When debanding is CPU and `use_yuv420p` is true, step 6 overwrites the gradfun filter. Proposal: build a single comma-joined `vf` list (`gradfun=radius=12` then `format=yuv420p`).

### 2.3 Debanding

OBSERVED (`AniyomiMPVView.kt:119-123`): `Debanding.None` -> nothing; `Debanding.CPU` -> `vf=gradfun=radius=12`; `Debanding.GPU` -> `deband=yes`. Enum `Debanding{None, CPU, GPU}` (`PlayerEnums.kt:93-97`). The stored enum name is the preference value (the `getEnum` helper, part 2 settings).

### 2.4 Cache limit

OBSERVED (`AniyomiMPVView.kt:137-140`): `cacheMegs = if (SDK >= O_MR1) 64 else 32`; `demuxer-max-bytes = cacheMegs * 1024 * 1024` and `demuxer-max-back-bytes` the same. Linux value: `64 * 1024 * 1024 = 67108864`.

### 2.5 copyUserFiles

OBSERVED (`PlayerActivity.kt:450-488`):

1. Compute `scripts`, `script-opts`, `shaders` subdirectories (created on access).
2. Delete all three directories (`scriptsDir()?.delete()`, `scriptOptsDir()?.delete()`, `shadersDir()?.delete()`).
3. If `mpv_scripts` (default false): copy every file from `storageManager.getScriptsDirectory()` into `scripts`, from `getScriptOptsDirectory()` into `script-opts`, from `getShadersDirectory()` into `shaders` (`openInputStream().copyTo(createFile(...).openOutputStream())`).
4. Always: write the bundled bridge `aniyomi.lua` from assets into `scripts/aniyomi.lua` (bufferedWriter, `readText`).

Proposal: copy 4 is kept only if custom buttons or user scripts exist; the Rust app ships the bridge as a string constant.

### 2.6 copyAssets

OBSERVED (`PlayerActivity.kt:490-515`): files `subfont.ttf`, `cacert.pem`. For each: open asset with `ACCESS_STREAMING`, create target file, if `outFile.length() == ins.available().toLong()` skip (logged VERBOSE `Skipping copy of asset file (exists same size): <name>`), else copy (WARN log `Copied asset file: <name>`). `IOException` logs ERROR `Failed to copy asset file: <name>` and continues. Streams are closed in `finally`.

### 2.7 copyFontsDirectory

OBSERVED (`PlayerActivity.kt:517-533`): on `Dispatchers.IO` (`launchIO`): create `mpv/fonts`; copy every file from the user fonts directory; then `MPVLib.setPropertyString("sub-fonts-dir", path)` and `"osd-fonts-dir"` to the same path. A source comment labels this "a bad hack".

### 2.8 Post-init

OBSERVED (`AniyomiMPVView.kt:162-169`): `postInitOptions()`: if `player_statistics_page` (default 0) != 0: `script-binding stats/display-stats-toggle` and `script-binding stats/display-page-<n>`.

### 2.9 Observed properties registration

OBSERVED (`AniyomiMPVView.kt:158-160`): `observeProperties()` registers every entry of `observedProps` (section 6.1) with `MPVLib.observeProperty(name, format)`.

---

## 3. Loading and playing a video

### 3.1 setVideo(video, position)

OBSERVED (`PlayerActivity.kt:1064-1137`):

1. If `player.isExiting` or `video == null`: return.
2. Stop and null `httpServer`.
3. `setHttpOptions(video)` (3.3).
4. Start position (3.2) via `set start <seconds>`.
5. `videoOptions = video.mpvArgs.joinToString(",") { (option, value) -> "$option=\"$value\"" }` (3.4).
6. If torrent support is enabled and (`videoUrl.startsWith(torrentServerApi.hostUrl)` or `startsWith("magnet")` or `endsWith("torrent")`): in IO, `TorrentServerService.start()` then `torrentLinkHandler(videoUrl, videoTitle, videoOptions)` (3.5).
7. Else in IO:
   a. If `video.usesHttpServer()` and the source is an `AnimeHttpSource`: create and start `httpSource.createHttpServer()`; port = `listeningPort ?: 0`; on exception toast `Failed to start http server` and return. Then `newVideo = video.copyHttpServer(port)`, `videoUrl = newVideo.videoUrl`, `viewModel.updateVideo(newVideo)`.
   b. `MPVLib.command(arrayOf("loadfile", parseVideoUrl(videoUrl), "replace", "0", videoOptions))`.

### 3.2 Start position

OBSERVED (`PlayerActivity.kt:1072-1087`):

- If `viewModel.isLoadingEpisode` is true: `resumePosition = position ?: (if (episode.seen && !preserve_watching_position) 0L else episode.last_second_seen)` (values are milliseconds). Command: `set start "${resumePosition / 1000F}"`.
- Else: if `player.timePos` is non-null: `set start "${player.timePos}"` (seconds, Int).

Note: the `"set"` command with `start` is issued before loadfile, so mpv applies it to the next file.

### 3.3 setHttpOptions

OBSERVED (`PlayerActivity.kt:1204-1223`):

1. If `viewModel.isEpisodeOnline() != true`: return. If `currentSource` is not an `AnimeHttpSource`: return.
2. `headers = (video.headers ?: source.headers).toMultimap().mapValues { it.value.firstOrNull() ?: "" }`.
3. `httpHeaderString = headers.map { key + ": " + value.replace(",", "\\,") }.joinToString(",")`.
4. `MPVLib.setOptionString("http-header-fields", httpHeaderString)`.
5. Disk cache options are commented out ("need to fix the cache").

Edge: `toMultimap()` keeps all values; only the first is used. Duplicate keys produce one header.

### 3.4 loadfile option quoting

OBSERVED: `video.mpvArgs.joinToString(",") { (option, value) -> "$option=\"$value\"" }`. No escaping is applied to `value`, so a value containing `"` breaks the option string. Proposal: pass per-file options as a key/value list to libmpv (`loadfile` with an options map), which avoids quoting entirely.

### 3.5 Torrent routing and torrentLinkHandler

OBSERVED (`PlayerActivity.kt:1153-1197`):

- If the URL starts with `content://`: open the input stream, `torrentServerApi.uploadTorrent(inputStream!!, title, save=false)`, play link `torrentServerUtils.getTorrentPlayLink(torrent, 0)`, `loadfile <link> replace 0 <options>`, return.
- If the URL starts with `magnet`: if it contains `index=`, `index = substringAfter("index=").substringBefore("&").toInt()`; `NumberFormatException` -> 0. Default index 0.
- `currentTorrent = torrentServerApi.addTorrent(videoUrl, title, "", "", save=false)`; `link = getTorrentPlayLink(currentTorrent, index)`; `loadfile <link> replace 0 <options>`.

Edge: `inputStream!!` is null-unsafe for a content URI that cannot be opened.

### 3.6 parseVideoUrl

OBSERVED (`PlayerActivity.kt:1199-1202`): `videoUrl?.toUri()?.resolveUri(this) ?: videoUrl`. Resolution rules are in section 10.1.

### 3.7 Video URL local server rewrite

OBSERVED (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt:151-178`): `usesHttpServer()` is true if the regex `http:\/\/localhost:1(?!\d)` matches the video URL, any audio track URL, or any subtitle URL. `copyHttpServer(port)` replaces every match with `http://localhost:<port>`. Note: the regex matches `localhost:1` followed by a non-digit, so a real port such as `localhost:1234` is not matched.

---

## 4. File loaded orchestration and track handling

### 4.1 Event route

OBSERVED (`PlayerActivity.kt:753-762`): `MPV_EVENT_FILE_LOADED` -> `viewModel.viewModelScope.launchIO { fileLoaded() }`; `MPV_EVENT_SEEK` -> `isLoading = true`; `MPV_EVENT_PLAYBACK_RESTART` -> `player.isExiting = false`.

### 4.2 fileLoaded order

OBSERVED (`PlayerActivity.kt:1280-1308`). Returns immediately when `isExiting`.

1. `setMpvOptions()` (4.3).
2. `setMpvMediaTitle()` (4.4).
3. `setupPlayerOrientation()` (dropped on desktop, section 12).
4. `setupChapters()` (4.5).
5. `setupTracks()` (4.6).
6. `viewModel.waitingSkipIntro = waitingTimeIntroSkip` (default 5).
7. `runBlocking`: if `viewModel.introSkipEnabled && aniSkipEnabled && !(disableAniSkipOnChapters && chapters non-empty)`: `aniSkipResponse(player.duration)` (part 2); if non-null, `updateChapters(ChapterUtils.mergeChapters(currentChapters, stamps, duration))` and `setChapter(pos.value)`.

Note: the comment block at `PlayerActivity.kt:1274-1279` records a past `ConcurrentModificationException` in `fileLoaded` (OBSERVED; the comment says it "may have been fixed").

### 4.3 setMpvOptions (metadata to mpv properties)

OBSERVED (`PlayerActivity.kt:1310-1335`):

1. `video = viewModel.currentVideo.value ?: return`.
2. Only for local videos: `videoUrl` must start with `file`, `content` or `data`; otherwise return.
3. `metadata = Json.decodeFromString<Map<String,String>>(MPVLib.getPropertyString("metadata"))`.
4. `opts = metadata["ANIYOMI_MPV_ARGS"]?.split(";")?.map { it.split("=", limit = 2) } ?: return`.
5. For each `(option, value)`: `MPVLib.setPropertyString(option, value)`.
6. Any exception: log ERROR `Failed to read video metadata`.

Edge: an entry with no `=` yields a single-element list; destructuring `(option, value)` then throws `NoSuchElementException`? Kotlin destructuring of a list with 1 element throws `IndexOutOfBoundsException` on component2; the catch-all logs it and stops processing the remaining entries. Proposal: skip malformed entries and continue.

### 4.4 setMpvMediaTitle

OBSERVED (`PlayerActivity.kt:1386-1406`):

1. Requires current anime and episode.
2. `setPropertyString("user-data/current-anime/episode-title", episode.name)`.
3. `epNumber = number.toInt() if ceil == floor else number`, then `.toString().padStart(2, '0')`.
4. Title = `stringResource(AYMR.strings.mpv_media_title, anime.title, epNumber, episode.name)` = `"%1$s - E%2$s - %3$s"`.
5. `setPropertyString("force-media-title", title)`.

Example: anime `"Show"`, episode number 1.5, name `"Pilot"` gives `Show - E1.5 - Pilot`; number 7 gives `Show - E07 - Pilot`.

### 4.5 setupChapters

OBSERVED (`PlayerActivity.kt:1361-1384`):

1. `timestamps = currentVideo.timestamps` (non-empty) else return.
2. For each stamp: if `name` is empty and `type != ChapterType.Other`: `name = type.getStringRes()?.let(::stringResource) ?: ""`. Otherwise unchanged.
3. `viewModel.updateChapters(ChapterUtils.mergeChapters(currentChapters = viewModel.chapters.value, stamps, duration = player.duration))`.
4. `viewModel.setChapter(viewModel.pos.value)`.

`ChapterType` values: `Opening`, `Ending`, `Recap`, `MixedOp`, `Other` (`source-api/.../model/Video.kt:14-20`).

### 4.6 setupTracks

OBSERVED (`PlayerActivity.kt:1337-1359`):

1. `isLoadingTracks = true`.
2. `audioTracks = currentVideo.audioTracks.takeIf { notEmpty }`; same for `subtitleTracks`.
3. If both are null: `viewModel.onFinishLoadingTracks()` and return.
4. For each audio: `audio-add <url> auto <lang>` (`executeMPVCommand`).
5. For each subtitle: `sub-add <url> auto <lang>`.
6. `isLoadingTracks = false` (set immediately, not after mpv confirms the track list).

Note: `track-list` property changes call `viewModel.loadTracks()` (section 6.2), which is where the real track list is refreshed.

---

## 5. user-data/aniyomi protocol (Lua bridge)

The bridge lets mpv scripts (`aniyomi.lua` and custom-button scripts) drive UI state. Each Lua call sets one mpv user-data property. The Kotlin observes these properties (`AniyomiMPVView.kt:236-249`) and forwards the string value to `viewModel.handleLuaInvocation(property, value)` (`PlayerActivity.kt:740`), whose internals are part 2.

Routing rule (OBSERVED, `PlayerActivity.kt:731-742`): a string property event is routed by `property.substringBeforeLast("/")`; the value `"user-data/aniyomi"` routes to `handleLuaInvocation`. Routing happens only when `isExiting` is false.

| Lua function | mpv property (user-data/aniyomi/...) | Value sent | Value format | Observed forwarding |
|---|---|---|---|---|
| `show_text(text)` | `show_text` | `text` | free text | handleLuaInvocation |
| `hide_ui()` / `show_ui()` / `toggle_ui()` | `toggle_ui` | `hide` / `show` / `toggle` | enum string | handleLuaInvocation |
| `show_subtitle_settings()` | `show_panel` | `subtitle_settings` | panel name | handleLuaInvocation |
| `show_subtitle_delay()` | `show_panel` | `subtitle_delay` | | handleLuaInvocation |
| `show_audio_delay()` | `show_panel` | `audio_delay` | | handleLuaInvocation |
| `show_video_filters()` | `show_panel` | `video_filters` | | handleLuaInvocation |
| `show_software_keyboard()` / `hide_software_keyboard()` / `toggle_software_keyboard()` | `software_keyboard` | `show` / `hide` / `toggle` | | handleLuaInvocation (no keyboard on desktop, section 14) |
| `set_button_title(text)` | `set_button_title` | `text` | | handleLuaInvocation |
| `reset_button_title()` | `reset_button_title` | `unused` | | handleLuaInvocation |
| `hide_button()` / `show_button()` / `toggle_button()` | `toggle_button` | `hide` / `show` / `toggle` | | handleLuaInvocation |
| `previous_episode()` | `switch_episode` | `p` | | handleLuaInvocation |
| `next_episode()` | `switch_episode` | `n` | | handleLuaInvocation |
| `pause()` | `pause` | `pause` | | handleLuaInvocation |
| `unpause()` | `pause` | `unpause` | | handleLuaInvocation |
| `pauseunpause()` | `pause` | `pauseunpause` | | handleLuaInvocation |
| `seek_by(value)` | `seek_by` | `value` | number in seconds | handleLuaInvocation |
| `seek_to(value)` | `seek_to` | `value` | number in seconds | handleLuaInvocation |
| `seek_by_with_text(value, text)` | `seek_by_with_text` | `value .. "|" .. text` | pipe-separated | handleLuaInvocation |
| `seek_to_with_text(value, text)` | `seek_to_with_text` | `value .. "|" .. text` | pipe-separated | handleLuaInvocation |
| `int_picker(title, name_format, start, stop, step, property)` | `launch_int_picker` | `title|name_format|start|stop|step|property` | pipe-separated | handleLuaInvocation |
| `left_seek_by(v)` | (legacy) calls `seek_by(-v)` | | | |
| `right_seek_by(v)` | (legacy) calls `seek_by(v)` | | | |

Also observed (not Lua): `user-data/current-anime/intro-length` (INT64) -> `setAnimeSkipIntroLength(value)`; written by the bridge script, read here (`PlayerActivity.kt:677`).

Note: `set_button_title` / `reset_button_title` / `toggle_button` / `show_button` / `hide_button` / `previous_episode` / `next_episode` are the custom-button API; their state lives in part 2.

Lua bridge loader: `require 'aniyomi'` works because the custom-button script sets `package.path` to include `mp.find_config_file('scripts')` and the app scripts directory (`PlayerActivity.kt:548-552`).

---

## 6. Observers and event dispatch

### 6.1 observedProps (all 34 entries, in source order)

OBSERVED (`AniyomiMPVView.kt:210-252`). Format names are `MPVLib.mpvFormat` constants.

| Property | Format | Handler (`PlayerActivity` unless noted) |
|---|---|---|
| `chapter` | INT64 | commented out (no handler) |
| `chapter-list` | NONE | `loadChapters()`, `updateChapter(0)` |
| `track-list` | NONE | `loadTracks()` |
| `time-pos` | INT64 | `updatePlayBackPos(value.toFloat())`, `setChapter(value.toFloat())` |
| `demuxer-cache-time` | INT64 | `updateReadAhead(value)` |
| `duration` | INT64 | `duration.update { value.toFloat() }` |
| `volume` | INT64 | `setMPVVolume(value.toInt())` |
| `volume-max` | INT64 | `volumeBoostCap = value.toInt() - 100` |
| `sid` | STRING | `updateSubtitle(id, selected.second)` (via `trackId`) |
| `secondary-sid` | STRING | `updateSubtitle(selected.first, id)` (via `trackId`) |
| `aid` | STRING | `updateAudio(id)` (via `trackId`) |
| `speed` | DOUBLE | `playbackSpeed.update { value.toFloat() }` |
| `video-params/aspect` | DOUBLE | if PiP supported: `createPipParams()` |
| `hwdec-current` | STRING | `getDecoder()` |
| `hwdec` | STRING | `getDecoder()` |
| `pause` | FLAG | pause/unpause logic (6.3) |
| `paused-for-cache` | FLAG | `isLoading.update { value }` |
| `seeking` | FLAG | `isLoading.update { value }` |
| `eof-reached` | FLAG | `endFile(value)` |
| `user-data/aniyomi/show_text` | STRING | handleLuaInvocation |
| `user-data/aniyomi/toggle_ui` | STRING | handleLuaInvocation |
| `user-data/aniyomi/show_panel` | STRING | handleLuaInvocation |
| `user-data/aniyomi/software_keyboard` | STRING | handleLuaInvocation |
| `user-data/aniyomi/set_button_title` | STRING | handleLuaInvocation |
| `user-data/aniyomi/reset_button_title` | STRING | handleLuaInvocation |
| `user-data/aniyomi/toggle_button` | STRING | handleLuaInvocation |
| `user-data/aniyomi/switch_episode` | STRING | handleLuaInvocation |
| `user-data/aniyomi/pause` | STRING | handleLuaInvocation |
| `user-data/aniyomi/seek_by` | STRING | handleLuaInvocation |
| `user-data/aniyomi/seek_to` | STRING | handleLuaInvocation |
| `user-data/aniyomi/seek_by_with_text` | STRING | handleLuaInvocation |
| `user-data/aniyomi/seek_to_with_text` | STRING | handleLuaInvocation |
| `user-data/aniyomi/launch_int_picker` | STRING | handleLuaInvocation |
| `user-data/current-anime/intro-length` | INT64 | `setAnimeSkipIntroLength(value)` |

Count check: 34 entries (19 non-user-data properties, 14 `user-data/aniyomi/*` properties, 1 `user-data/current-anime/intro-length`). Keys are unique.

Also observed: `hwdec` and `hwdec-current` are both in the list but only `getDecoder()` runs (no value is sent).

### 6.2 Dispatch rules

OBSERVED (`PlayerObserver.kt`): every mpv callback is posted with `activity.runOnUiThread { ... }` and then calls the matching `PlayerActivity` overload. Each `onObserverEvent` overload returns immediately when `player.isExiting` is true. The `event(eventId)` function also returns immediately when exiting.

### 6.3 pause property handling

OBSERVED (`PlayerActivity.kt:692-721`):

- `value == true` and `player.paused == true`: `viewModel.pause()`, clear `FLAG_KEEP_SCREEN_ON`.
- `value == false` and `player.paused == false`: `viewModel.unpause()`, add `FLAG_KEEP_SCREEN_ON`.
- Then `runCatching { setPictureInPictureParams(createPipParams()) }` (always).

The double check (`player.paused` read again) guards against races between the callback value and the live property.

### 6.4 Event ids

OBSERVED (`PlayerActivity.kt:753-762`):

- `MPV_EVENT_FILE_LOADED`: launch `fileLoaded()` on IO (4.2).
- `MPV_EVENT_SEEK`: `isLoading = true`.
- `MPV_EVENT_PLAYBACK_RESTART`: `player.isExiting = false` (but only reached when `isExiting` is already false, because of the gate above).

Observed consequence: `isExiting` is set in `onPause` (1.6) and the restart event can never clear it, since the gate returns first. The flag is therefore cleared only by `onResume` (1.5). Proposal: handle PLAYBACK_RESTART before the exiting gate, or clear the flag in `onResume` only, and document it.

### 6.5 Property read helpers

OBSERVED (`AniyomiMPVView.kt:52-93`): `getPropertyInt/Boolean/Double/String` call `MPVLib.getPropertyX` and cast with `as T?`. `duration` is `getPropertyInt("duration")`; `timePos` is `getPropertyInt("time-pos")` (setter `setPropertyInt("time-pos", position!!)`); `paused` is `getPropertyBoolean("pause")` (setter `setPropertyBoolean("pause", paused!!)`); `hwdecActive` is `getPropertyString("hwdec-current") ?: "no"`; `videoH` is `getPropertyInt("video-params/h")`.

### 6.6 Track delegates

OBSERVED (`AniyomiMPVView.kt:95-112`):

- Getter: `getPropertyString(name)?.toIntOrNull() ?: -1`. Values `no`, `auto`, or any non-integer give -1.
- Setter: `-1` sets the string `no`; any other int sets the int.
- Properties: `sid`, `secondary-sid`, `aid`.

### 6.7 Video aspect helper

OBSERVED (`AniyomiMPVView.kt:88-93`): `getVideoOutAspect()`: `aspect = video-params/aspect` (double). If null, returns null. If `aspect < 0.001` returns `0.0`. Else if `(video-params/rotate ?: 0) % 180 == 90`, returns `1.0 / aspect`; else `aspect`.

---

## 7. Playback control and episode switching

### 7.1 changeEpisode(episodeId, autoPlay)

OBSERVED (`PlayerActivity.kt:1002-1062`):

1. `sheetShown = Sheets.None`, `panelShown = Panels.None`, `viewModel.pause()`, `isLoading = true`, `viewModel.resetState()`.
2. In `lifecycleScope.launch`:
   a. `updateIsLoadingEpisode(true)`, `updateIsLoadingHosters(true)`, `cancelHosterVideoLinksJob()`.
   b. `pipEpisodeToasts = pip_episode_toasts` (read now, default true).
   c. `switchMethod = viewModel.loadEpisode(episodeId)` (part 2).
   d. `updateIsLoadingHosters(false)`.
   e. When `switchMethod == null`: if `currentAnime != null && !autoPlay` toast `Next Episode not found!` (`no_next_episode`); `isLoading = false`.
   f. Else if `switchMethod.hosterList != null`: if empty, `setInitialEpisodeError(ExceptionWithStringResource("Hoster list is empty", no_hosters))`; else `loadHosters(source, hosterList, hosterIndex = -1, videoIndex = -1)`.
   g. Else log ERROR `Error getting links`.
   h. If `isInPictureInPictureMode && pipEpisodeToasts`: toast the episode title (`switchMethod.episodeTitle`).
3. Synchronously after launching: `updateHasPreviousEpisode(index != 0)`, `updateHasNextEpisode(index != playlist.size - 1)`.

### 7.2 Autoplay

OBSERVED (`PlayerActivity.kt:1408-1412`): `endFile(eofReached)`: if `eofReached && autoplay_enabled` (default false) then `changeEpisode(previous = false, autoPlay = true)`. Note that `PlayerActivity` calls `viewModel.changeEpisode(previous = false, autoPlay = true)` directly (part 2 signature).

PIP key actions (OBSERVED, `PlayerActivity.kt:812-823`): `PIP_NEXT` -> `changeEpisode(false)`; `PIP_PREVIOUS` -> `changeEpisode(true)`.

### 7.3 Media and D-pad mapping

OBSERVED (`PlayerActivity.kt:854-884`), `onKeyDown` (all branches return true):

| Key code | Action |
|---|---|
| `KEYCODE_VOLUME_UP` | `changeVolumeBy(1)`, `displayVolumeSlider()` |
| `KEYCODE_VOLUME_DOWN` | `changeVolumeBy(-1)`, `displayVolumeSlider()` |
| `KEYCODE_DPAD_LEFT` | `handleLeftDoubleTap()` |
| `KEYCODE_DPAD_RIGHT` | `handleRightDoubleTap()` |
| `KEYCODE_SPACE` | `pauseUnpause()` |
| `KEYCODE_MEDIA_STOP` | `finishAndRemoveTask()` |
| `KEYCODE_MEDIA_REWIND` | `handleLeftDoubleTap()` |
| `KEYCODE_MEDIA_FAST_FORWARD` | `handleRightDoubleTap()` |
| else | `player.onKey(event)` then `super.onKeyDown` |

`onKeyUp` (`PlayerActivity.kt:881-884`): `player.onKey(event!!)` returns true or delegates to super.

### 7.4 Media session actions

OBSERVED (`PlayerActivity.kt:886-986`). Mapping per action:

- onPlay: `None`/`Seek`/`Switch` do nothing; `PlayPause` -> `super.onPlay()`, `unpause()`, add keep-awake; `Custom` -> `keypress 0x10005`.
- onPause: `None`/`Seek`/`Switch` nothing; `PlayPause` -> `super.onPause()`, `pause()`, clear keep-awake; `Custom` -> `keypress 0x10005`.
- onSkipToPrevious (`previousAction`): `None` nothing; `Seek` -> `leftSeek()`; `PlayPause` -> `pauseUnpause()`; `Custom` -> `keypress 0x10004`; `Switch` -> `changeEpisode(true)`.
- onSkipToNext (`nextAction`): `None` nothing; `Seek` -> `rightSeek()`; `PlayPause` -> `pauseUnpause()`; `Custom` -> `keypress 0x10006`; `Switch` -> `changeEpisode(false)`.
- onStop: `isActive = false`; `this@PlayerActivity.onStop()`.

Defaults (`GesturePreferences.kt:97-99`): previous = `Switch`, play-pause = `PlayPause`, next = `Switch`.

Playback state advertised: actions PLAY, PAUSE, STOP, SKIP_TO_PREVIOUS, SKIP_TO_NEXT.

---

## 8. Keyboard input (AniyomiMPVView.onKey)

OBSERVED (`AniyomiMPVView.kt:171-208`), algorithm:

1. If `event.action == ACTION_MULTIPLE` or the key is a modifier key: return false.
2. `mapped = KeyMapping.map.get(keyCode)` (external `is.xyz.mpv.KeyMapping`, not in repo).
3. If `mapped == null`:
   a. If the event is not a printing key: if `repeatCount == 0` log DEBUG `Unmapped non-printable key <code>`; return false.
   b. `ch = event.unicodeChar`; if `ch & COMBINING_ACCENT != 0` return false (dead key).
   c. `mapped = ch.toChar().toString()`.
4. If `repeatCount > 0`: return true (consumed, mpv does its own repeat).
5. Build modifiers: `shift`, `ctrl`, `alt`, `meta` in that order when pressed.
6. `action = "keydown"` if `ACTION_DOWN`, else `"keyup"`.
7. `MPVLib.command(arrayOf(action, mods.joinToString("+") + mapped))`; the `+` join is applied to the full list including the key name (so a plain key yields just the name).
8. Return true.

Note: the modifier list is built with `mod.add(mapped)` appended last, then joined with `+`. Example: Ctrl+Left gives `keydown ctrl+LEFT`.

---

## 9. Audio

### 9.1 Output channels

OBSERVED (`PlayerActivity.kt:584-600`): on create, `audioChannels()` (enum `AudioChannels`, default `AutoSafe`) sets `property` = `value`. Enum (`AudioPreferences.kt:43-49`):

- `Auto`: property `audio-channels`, value `auto-safe`.
- `AutoSafe`: property `audio-channels`, value `auto`.
- `Mono`: `audio-channels`, `mono`.
- `Stereo`: `audio-channels`, `stereo`.
- `ReverseStereo`: property `af`, value `pan=[stereo|c0=c1|c1=c0]`.

### 9.2 Audio focus (dropped on desktop)

OBSERVED (`PlayerActivity.kt:588-599`, `602-632`): request `AUDIOFOCUS_GAIN` with usage MEDIA and content type MUSIC. On change:

- `LOSS` or `LOSS_TRANSIENT`: `wasPlayerPaused = player.paused ?: false`; `viewModel.pause()`; restore = old restore then (if not wasPlayerPaused) `unpause()`.
- `LOSS_TRANSIENT_CAN_DUCK`: `multiply volume 0.5`; restore = `multiply volume 2`.
- `GAIN`: run restore, reset to no-op.
- `REQUEST_FAILED`: DEBUG log `didn't get audio focus`.

Decision: dropped (T-45a-042). The volume multiply is not exactly reversible (`x * 0.5 * 2 == x` only for representable values, and any volume change in between is multiplied too).

### 9.3 Audio option defaults

From `AudioPreferences.kt:33-41` (part-2 file, values referenced here): `pref_audio_lang` `""`; `pref_audio_pitch_correction` true; `pref_audio_config` `AutoSafe`; `pref_audio_volume_boost_cap` 30; `pref_audio_delay` 0 (ms).

---

## 10. Source URL resolution

### 10.1 Uri.resolveUri

OBSERVED (`PlayerUtils.kt:500-511`):

- `file` -> `path`.
- `content` -> `openContentFd(context)` (below).
- `data` -> `"data://" + schemeSpecificPart`.
- scheme in `Utils.PROTOCOLS` (external `is.xyz.mpv.Utils`) -> `toString()`.
- else null, log ERROR `unknown scheme: <scheme>`.

### 10.2 openContentFd

OBSERVED (`PlayerUtils.kt:492-498`): open the content URI for read, detach fd; `Utils.findRealPath(fd)` gives a real path (then the fd is closed) or the fallback `fd://<fd>`.

Desktop: not applicable (no content provider). Proposal: `file://` paths are canonicalised and passed as absolute paths; `data:` and protocol strings are passed unchanged.

### 10.3 getFileName

OBSERVED (`PlayerUtils.kt:513-519`): query `OpenableColumns.DISPLAY_NAME`; `moveToFirst()` is not checked, so an empty cursor returns garbage. Desktop: basename of the path.

---

## 11. Media session (MediaSession) and MPRIS

See 7.4 for the action map. Proposal: MPRIS per T-45a-043. Additional observed details:

- The session is created at `onCreate` with tag `"PlayerActivity"`, set active immediately.
- `noisyReceiver` is registered for `ACTION_AUDIO_BECOMING_NOISY` (`PlayerActivity.kt:983-985`), which calls `viewModel.pause()` and clears keep-awake (`147-155`). Desktop replacement: T-45a-082.

---

## 12. Orientation (dropped on desktop)

OBSERVED (`PlayerActivity.kt:835-852`), `setupPlayerOrientation()`: skip when exiting; otherwise `requestedOrientation` from `pref_default_player_orientation_type_key` (default `SensorLandscape`):

| PlayerOrientation | Activity orientation |
|---|---|
| Free | SENSOR |
| Video | SENSOR_LANDSCAPE if `getVideoOutAspect() > 1.0`, else SENSOR_PORTRAIT (null treated as 0.0, so portrait) |
| Portrait | PORTRAIT |
| ReversePortrait | REVERSE_PORTRAIT |
| SensorPortrait | SENSOR_PORTRAIT |
| Landscape | LANDSCAPE |
| ReverseLandscape | REVERSE_PORTRAIT (defect: should be REVERSE_LANDSCAPE) |
| SensorLandscape | SENSOR_LANDSCAPE |

Defect recorded: `ReverseLandscape` maps to a portrait value.

---

## 13. Errors and failure paths

### 13.1 setInitialEpisodeError

OBSERVED (`PlayerActivity.kt:1143-1151`): if the error is `ExceptionWithStringResource` show `error.stringResource` as toast, else show `error.message` (may be null, toast of null); log ERROR; `finish()`.

Messages: `Hoster list is empty` (key `no_hosters` = "Hoster list is empty"), `Unknown error` (fallback text in onNewIntent).

### 13.2 setVideo failures

- HTTP server start failure: toast `Failed to start http server` (key `http_server_start_failure`), return from the coroutine (no load).
- Torrent failures: propagate as exceptions from `TorrentServerApi` (no catch in `torrentLinkHandler`); the launch coroutine fails silently (no toast). Observed gap.

### 13.3 Playback errors

`PlayerObserver.efEvent(err)` (`PlayerObserver.kt:440-450`): message `err ?: "Error: File ended"`; appended `: <httpError>` if an `HTTP error` log line was seen; logged ERROR; toast with `Toast.LENGTH_LONG`.

### 13.4 Screenshot and art results

- Save success: toast `Picture saved` (`picture_saved`). Error: log only.
- Art: see 15.

### 13.5 Metadata and option errors

Metadata JSON failure: log ERROR `Failed to read video metadata` (4.3).

### 13.6 Asset copy failures

`IOException` in `copyAssets`: log ERROR `Failed to copy asset file: <name>`, continue with the next asset.

### 13.7 Thread-level uncaught exception

OBSERVED (`PlayerActivity.kt:244-250`): process-wide handler: `runOnUiThread { toast(throwable.message) }`, log ERROR, `finish()`. Dropped (T-45a-054), replacement is a GTK error dialog.

---

## 14. Keep-awake, fullscreen and window flags

OBSERVED flags:

- `FLAG_KEEP_SCREEN_ON`: added in `onStart` (1.4), on pause=false (6.3), on media play (7.4); cleared on pause=true (6.3), media pause (7.4), and the becoming-noisy receiver (11). `onNewIntent` does not change it.
- `FLAG_LAYOUT_NO_LIMITS`, immersive system UI (1.4): Android only.

Desktop (Proposal, T-45a-041, T-45a-044): idle inhibit via `gtk::Application::inhibit(Some(&window), InhibitFlags::IDLE, "Playing video")` while playing; fullscreen via `gtk::Window::fullscreen`.

Software keyboard: `software_keyboard` bridge calls are accepted and logged unsupported on desktop.

---

## 15. Screenshot, share and set-as-art

### 15.1 Share

OBSERVED (`PlayerActivity.kt:1229-1238`): requires current anime and episode. Message `stringResource(AYMR.strings.share_screenshot_info, anime.title, episode.name, seconds)` = `"%1$s: %2$s, %3$s"`. Share via `uri.toShareIntent(context, message)` in a chooser titled `MR.strings.action_share` ("Share").

### 15.2 Save

OBSERVED (`PlayerActivity.kt:1244-1253`): `SaveImageResult.Success` -> toast `Picture saved`; `SaveImageResult.Error` -> log ERROR with the error.

### 15.3 Set as art

OBSERVED (`PlayerActivity.kt:1259-1272`):

| SetAsArt | ArtType | Message |
|---|---|---|
| Success | Cover | `Cover updated` (MR cover_updated) |
| Success | Background | `Background updated` (AYMR background_updated) |
| Success | Thumbnail | `Thumbnail updated` (AYMR thumbnail_updated) |
| AddToLibraryFirst | any | `Please add the entry to your library before doing this` (MR notification_first_add_to_library) |
| Error | any | `Failed to update cover` (MR notification_cover_update_failed) |

Set-as-art logic itself is in the part-2 view model.

---

## 16. External players (ExternalIntents)

### 16.1 getExternalIntent algorithm

OBSERVED (`ExternalIntents.kt:80-118`):

1. `initAnime(animeId, episodeId)`: loads `getAnime.await(animeId)` (null -> return false), `sourceManager.get(anime.source)` (null -> false), `getEpisodesByAnimeId.await(anime.id).find { it.id == episodeId }` (null -> false). Sets `this.animeId`, `this.episodeId`. Returns true.
2. If init false: return null.
3. `hosters = EpisodeLoader.getHosters(episode, anime, source)`.
4. `video = chosenVideo ?: HosterLoader.getBestVideo(source, hosters) ?: throw Exception("Video list is empty")`.
5. `videoUrl = getVideoUrl(source, context, video) ?: return null`.
6. If `video.usesHttpServer()`: `(success, port) = MainActivity.startHttpServerService(context, source.id)`; on failure toast `Failed to start http server` on UI and return null. Else `videoUrl = getVideoUrl(source, context, video.copyHttpServer(port)) ?: return null`.
7. `pkgName = externalPlayerPreference()` (default empty string).
8. Empty package: `Intent(ACTION_VIEW)` with `setDataAndTypeAndNormalize(videoUrl, getMime(videoUrl))`, `addExtrasAndFlags(false, this)`, `addVideoHeaders(false, video, this)`.
9. Else: `getIntentForPackage(pkgName, context, videoUrl, video)`.

Note: `getMime(videoUrl)` takes a `Uri`, so the mime is based on the URL path extension.

### 16.2 getVideoUrl

OBSERVED (`ExternalIntents.kt:137-168`):

1. `resolved = HosterLoader.getResolvedVideo(source, video)`.
2. If null or `videoUrl` empty: `makeErrorToast(context, Exception("Video URL is empty."))` (shows message `Video URL is empty.`), return null.
3. `uri = videoUrl.toUri()`.
4. `isOnDevice = true` if source is `LocalAnimeSource.ID`; else `downloadManager.isEpisodeDownloaded(episodeName, episodeScanlator, animeTitle, sourceId, skipCache = true)`.
5. If `isOnDevice && uri.scheme != "content"`: `FileProvider.getUriForFile(context, applicationId + ".provider", File(uri.path!!))`. Else `uri`.

Desktop (T-45a-068): step 5 returns the plain file path.

### 16.3 Start position

OBSERVED (`ExternalIntents.kt:173-182`): `preserve = preserve_watching_position`; `isEpisodeWatched = episode.lastSecondSeen == episode.totalSeconds`. If `episode.seen && (!preserve || isEpisodeWatched)` -> `1L`, else `episode.lastSecondSeen`.

Compare with the internal rule (3.2), which returns 0 (not 1) when seen and not preserved. Both are kept (T-45a-062 says 1 ms for external).

### 16.4 Intent extras

OBSERVED:

- `addExtrasAndFlags(isSupportedPlayer, intent)` (`ExternalIntents.kt:280-288`): `title` = `anime.title + " - " + episode.name`; `position` = start (int); `return_result` = true; flag `FLAG_GRANT_READ_URI_PERMISSION`; if supported player: `secure_uri` = true.
- `addVideoHeaders` (`297-310`): headers = `video.headers ?: (source as? AnimeHttpSource)?.headers`; if non-null: `headers` extra = flat array `[name1, value1, name2, value2, ...]`; if not supported player: `http-header-fields` = `headersArray.drop(2).joinToString(": ")` (note: this is a defect, it joins values with ": " after dropping the first pair).

### 16.5 Standard intent for packages

OBSERVED (`ExternalIntents.kt:242-272`):

- If installed: if SDK >= TIRAMISU and package contains `vlc`: `setPackage`; else `component = getComponent(pkg)`.
- `setDataAndType(uri, "video/*")`; `addExtrasAndFlags(true, ...)`; `addVideoHeaders(true, ...)`.
- Subtitles: `langIndex = subtitleTracks.indexOfFirst { lang contains localeName (ignore case) }`; `requestedLanguage = if -1 then 0 else index`; `requestedUrl = subtitleTracks.getOrNull(requestedLanguage)?.url`. Note: when no track matches, the first track is still selected (index 0).
- Extras: `subs` = all subtitle URLs as URI array; `subs.name` = all languages; `subs.enable` = `[requestedUrl]` or empty; `subtitles_location` = requestedUrl (VLC).

Note: `LocaleHelper.getSimpleLocaleDisplayName()` (external util) gives the current locale's simple name.

### 16.6 Component names

OBSERVED (`ExternalIntents.kt:332-347`):

- `is.xyz.mpv` -> `is.xyz.mpv.MPVActivity`.
- `com.mxtech.videoplayer`, `.ad`, `.pro` -> `<pkg>.ActivityScreen`.
- `org.videolan.vlc` -> `org.videolan.vlc.gui.video.VideoPlayerActivity`.
- `live.mehiz.mpvkt`, `.preview` -> `live.mehiz.mpvkt.ui.player.PlayerActivity`.
- `com.husudosu.mpvremote` -> `<pkg>.MainActivity`.
- `com.brouken.player` -> `<pkg>.PlayerActivity`.
- `dev.anilbeesetti.nextplayer` -> `<pkg>.feature.player.PlayerActivity`.
- `video.player.videoplayer` -> `com.inshot.xplayer.activities.PlayerActivity`.
- else null.

### 16.7 Web Video Caster

OBSERVED (`ExternalIntents.kt:209-232`): `setDataAndType(uri, "video/*")`; package set if installed; `addExtrasAndFlags(true)`; `HTTP_HEADERS` bundle from `video.headers`; subtitle: first track whose lang contains the locale name, else first track; extras `subtitle` (URL) and `android.media.intent.extra.HTTP_HEADERS` and `secure_uri` = true. Dropped (T-45a-060).

### 16.8 MIME

OBSERVED (`ExternalIntents.kt:317-324`): by path extension `mp4` -> `video/mp4`, `mkv` -> `video/x-matroska`, `m3u8` -> `application/x-mpegURL`, else `video/any`.

### 16.9 onActivityResult

OBSERVED (`ExternalIntents.kt:371-419`):

1. `data = intent ?: return`; if `animeId == null || episodeId == null` return.
2. `cause = data.getStringExtra("end_by") ?: ""`.
3. If `cause` non-empty: `currentPosition` from extra `position` (Int -> Long, Long as is, else 0); `duration` from extra `duration` the same way.
4. Else: if extra `extra_position` is present: `currentPosition = getLongExtra("extra_position")`, `duration = getLongExtra("extra_duration")`. Else `currentPosition = getIntExtra("position")`, `duration = getIntExtra("duration")` (both as Long).
5. In IO: if `cause == "playback_completion"` or `(currentPosition == duration && duration == 0L)`: save progress with `(totalSeconds, totalSeconds)`; else save with `(currentPosition, duration)`.
6. Then `saveEpisodeHistory(episode)`.

Note: the second clause in step 5 treats an unknown duration of 0 as completion when position is also 0.

### 16.10 saveEpisodeHistory and saveEpisodeProgress

OBSERVED (`ExternalIntents.kt:441-486`):

- History: if incognito return; `upsertHistory.await(AnimeHistoryUpdate(episode.id, Date()))`.
- Progress: if incognito return; `currEp = currentEpisode ?: return`. If `totalSeconds > 0`: `progress = progressPreference()` (default 0.85); `seen = if (!currEp.seen) lastSecondSeen >= totalSeconds * progress else true`; `updateEpisode.await(EpisodeUpdate(id, seen, bookmark, fillermark, lastSecondSeen, totalSeconds))`; if `autoUpdateTrack` and `currEp.seen` (the pre-update value): `updateTrackEpisodeSeen(episodeNumber.toDouble(), anime)`; if `seen`: `deleteEpisodeIfNeeded(currentEpisode, anime)`.

Note: `currEp.seen` is read before the write, so the tracker update is sent only for episodes that were already seen when the save ran. Observed; the Proposal in T-45a-066 keeps this rule and flags it.

### 16.11 deleteEpisodeIfNeeded

OBSERVED (`ExternalIntents.kt:495-515`):

- Sort function by `anime.sorting`: `EPISODE_SORTING_SOURCE` (0x0) -> descending by `sourceOrder`; `EPISODE_SORTING_NUMBER` (0x200) -> ascending by `episodeNumber`; `EPISODE_SORTING_UPLOAD_DATE` (0x400) -> ascending by `dateUpload`; else throw `NotImplementedError("Unknown sorting method")`.
- `episodes = getEpisodesByAnimeId.await(anime.id).sortedWith(sortFunction)`.
- `pos = episodes.indexOf(episode)`; `slots = remove_after_read_slots` (default -1); `toDelete = episodes.getOrNull(pos - slots)`.
- If `slots != -1 && toDelete != null`: `enqueueDeleteSeenEpisodes(toDelete, anime)`.

Edge: `pos == -1` gives `getOrNull(-1 - slots)`; with slots < 0 this can be a valid index; the guard `slots != -1` does not cover slots < -1. Proposal: require `slots >= 0`.

### 16.12 enqueueDeleteSeenEpisodes

OBSERVED (`ExternalIntents.kt:570-579`): if `episode.seen`: IO `downloadManager.enqueueEpisodesToDelete(listOf(episode), anime)`.

### 16.13 updateTrackEpisodeSeen

OBSERVED (`ExternalIntents.kt:524-562`):

1. Return if `autoUpdateTrack` is false.
2. IO: for each `track` in `getTracks.await(anime.id)`: `tracker = trackerManager.get(track.trackerId)`; if tracker non-null, logged in, is `AnimeTracker`, and `episodeNumber > track.lastEpisodeSeen`:
   - `updatedTrack = track.copy(lastEpisodeSeen = episodeNumber)`.
   - `async { runCatching { if online: tracker.animeService.update(updatedTrack.toDbTrack(), true); insertTrack.await(updatedTrack) else: delayedTrackingStore.addAnime(track.animeId, lastEpisodeSeen = episodeNumber); DelayedAnimeTrackingUpdateJob.setupTask(context) } }`.
3. `awaitAll()`; for each failure log INFO.

### 16.14 Exception toasts

`makeErrorToast(context, e)` shows `e.message ?: "Cannot open episode"` (`ExternalIntents.kt:190-192`).

---

## 17. HTTP local server (HttpServerService and HttpServer)

### 17.1 HttpServer

OBSERVED (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/HttpServer.kt:7-35`): `open class HttpServer : NanoHTTPD(0)` (ephemeral port). `url` = `http://localhost:<listeningPort>`. `start()` calls super and sets `isRunning = true`; on exception it only logs DEBUG `Failed to start http server` and does not rethrow. `stop()` calls super and clears `isRunning`. `PLACEHOLDER_URL = "http://localhost:1"`.

Observed defect: because `start()` swallows the exception, `HttpServerService` still sets `_isRunning = true` and `port = listeningPort` (which is 0 when the socket never bound). Proposal: `LocalServer::start` returns `Err` on bind failure.

### 17.2 HttpServerService.onStartCommand

OBSERVED (`HttpServerService.kt:33-98`):

1. `ACTION_STOP`: `stopSelf()`, `START_NOT_STICKY`.
2. If `isRunning`: set false, `httpServer?.stop()`.
3. `sourceId = intent.getLongExtra(EXTRA_SOURCE_ID, -1)`; `server = (sourceManager.get(sourceId) as? AnimeHttpSource)?.createHttpServer()`.
4. If null: `stopSelf()`, `START_NOT_STICKY`.
5. `httpServer.start()`; `port = listeningPort ?: 0`; any exception: log ERROR `Failed to start http server`, `stopSelf()`.
6. `_isRunning = true`.
7. Foreground notification (section 18) with `ID_HTTP_SERVER`, `FOREGROUND_SERVICE_TYPE_DATA_SYNC` on API 29+.
8. `START_STICKY`.

`onDestroy`: `_isRunning = false`, `httpServer?.stop()`.

### 17.3 Start from UI

OBSERVED (`MainActivity.kt:644-660`): `resetIsRunning()`, start the service with the source id extra, wait up to `timeout = 5.seconds` for `isRunning` to become true (`withTimeoutOrNull`), return `(ready == true, HttpServerService.port)`.

Observed nuance: `isRunning.first { it }` is subscribed after `startService`, but `resetIsRunning()` ran before, so a fast service start can complete before the subscription is active; `StateFlow` replays the current value, so this is safe.

Proposal (desktop): the server starts synchronously in the caller on a tokio task with a 5 s timeout, returning `Result<u16>`.

---

## 18. Notifications of the services

OBSERVED:

- HTTP server (`HttpServerService.kt:64-96`): channel `Notifications.CHANNEL_HTTP_SERVER` = `"http_server_channel"` (importance LOW, `Notifications.kt:183`), id `ID_HTTP_SERVER` = `-901`, small icon `R.drawable.ic_dns_24dp`, large icon launcher, action `MR.strings.action_close` ("Close") with a `ACTION_STOP` pending service intent, content text `AYMR.strings.http_server_is_running`, not auto-cancel, ongoing, chronometer on.
- Torrent server (`TorrentServerService.kt:94-138`): channel `CHANNEL_TORRENT_SERVER` = `"torrent_server_channel"` (LOW, `Notifications.kt:179`), id `ID_TORRENT_SERVER` = `-801`, small icon `R.drawable.ic_ani`, content text `AYMR.strings.torrentserver_is_running`, title `MR.strings.app_name`, delete intent = start-again pending service, chronometer on, ongoing, action label **hard-coded** `"Stop"` (not localised, defect).
- New episodes notification id `ID_NEW_EPISODES` = `-1301` (dismissed in onNewIntent).

Desktop: see T-45a-071 and T-45a-079 (GNotification with actions, or an in-window badge).

---

## 19. Torrent server

### 19.1 Constants

OBSERVED (`TorrentServerService.kt:141-142`): `ACTION_START = "start_torrent_server"`, `ACTION_STOP = "stop_torrent_server"`. Preferences: `pref_torrserver_enable` (false), `pref_torrserver_shownotice` (false), `pref_torrserver_port` (`"8090"`), `pref_torrserver_tackers` (default list, data doc), `pref_torrserver_proxymode` (`ProxyMode.None`), `pref_torrserver_proxyurl` (`""`).

### 19.2 Service start

OBSERVED (`TorrentServerService.kt:63-84`):

1. In `serviceScope.launch`: if `api.echo() == ""` (server not up):
   a. If `verbose_logging`: `TorrServer.registerLogCallback()`.
   b. `port = TorrServer.startServer(port = pref_torrserver_port, path = filesDir.absolutePath, proxyMode = mode.value, proxyUrl = if mode == None "" else pref_torrserver_proxyurl)`.
   c. If `port != -1`: `api.setPort(port)`, `wait(10)`, `torrentServerUtils.setTrackersList()`.
2. If `echo` is already non-empty, nothing happens (no re-push of trackers).

`ACTION_START` path: `startServer()` then `notification(...)` then `START_STICKY`.

### 19.3 wait(timeout)

OBSERVED (`TorrentServerService.kt:173-186`):

- `count = 0`; if `timeout < 0`: `count = -20`.
- `while (api.echo() == "")`: `delay(1 s)`; `count++`; if `count > timeout` return false.
- Return true.

Effective budgets: `wait(10)` gives 11 polls (about 11 s). `wait()` (timeout -1) gives 20 polls (about 20 s), since count starts at -20 and returns when count > -1.

### 19.4 Stop

OBSERVED (`TorrentServerService.kt:86-92`): `TorrServer.stopServer()`, cancel notification `ID_TORRENT_SERVER`, `stopSelf()`.

Companion `stop()` and `start()` send intents via `applicationContext.startService`; exceptions log DEBUG and print the stack trace. `start()` also calls `wait(10)`.

### 19.5 Tracker list

OBSERVED (`TorrentServerUtils.kt:13-19`): `animeTrackers = torrServerTrackers().split("\n").joinToString(",\n")`; `setTrackersList()` calls `TorrServer.addTrackers(animeTrackers)`.

### 19.6 Play link

OBSERVED (`TorrentServerUtils.kt:21-27`): file = `torr.fileStats?.firstOrNull { it.id == index }`; `name = File(file.path).name` or `torr.title`; URL `http://127.0.0.1:<port>/stream/<urlencode(name)>?link=<hash>&index=<index>&play` (`URLEncoder.encode(name, "utf8")`).

Note: `torr.hash` is nullable; the interpolation prints `null` if absent (defect).

### 19.7 API

OBSERVED (`TorrentServerApi.kt`):

- `hostUrl = "http://127.0.0.1:" + port` (port is `@Volatile`).
- `echo()`: `GET <host>/echo`, `awaitSuccess()`, returns body string; on any exception logs DEBUG `Error sending echo` and returns `""`.
- `addTorrent(link, title, poster = "", data = "", save)`: `POST <host>/torrents` with JSON `TorrentRequest("add", link, title, poster, data, save_to_db = save)`; decodes `Torrent`.
- `uploadTorrent(file, title, save = false)`: reads all bytes, `POST <host>/torrent/upload` multipart with `file` (filename = title, type `application/x-bittorrent`), `save` (string), `title`; decodes `Torrent`.

`TorrentRequest` JSON (`TorrentRequest.kt`): `action` (String), `hash` (default ""), `link` (""), `title` (""), `poster` (""), `data` (""), `save_to_db` (Boolean, default false). Note: the `@SerialName("save_to_db")` is on `saveToDb`.

`Torrent` JSON (`Torrent.kt`): `title` (String, required), `hash` (String?, default null), `torrent_size` (Long?), `trackers` (List<String>?), `file_stats` (List<FileStats>?). `FileStats`: `id` (Int?), `path` (String), `length` (Long).

### 19.8 Magnet index

OBSERVED (`PlayerActivity.kt:1175-1183`): `index=` in the URL; value up to `&`; parse error gives 0.

### 19.9 Mode mapping

OBSERVED (`TorrentPreferences.kt:46-51`): `ProxyMode(value)`: `None("tracker")`, `Tracker("tracker")`, `Peers("peers")`, `Full("full")`. Default `None`.

---

## 20. Dispatchers, threads and concurrency

| Work | Where it runs (OBSERVED) |
|---|---|
| mpv callbacks | mpv event thread, then `runOnUiThread` (PlayerObserver) |
| `fileLoaded` | `viewModel.viewModelScope.launchIO` (IO) |
| `runBlocking` aniSkip in `fileLoaded` | blocks that IO worker |
| `setVideo` network setup | `launchIO` |
| `copyFontsDirectory` | `CoroutineScope(Dispatchers.IO).launchIO` |
| `setupCustomButtons` | `CoroutineScope(Dispatchers.IO).launchIO` |
| `changeEpisode` | `lifecycleScope.launch` (main) |
| `onNewIntent` init | `lifecycleScope.launchNonCancellable`, then `lifecycleScope.launch` for loadHosters |
| `onActivityResult` (external) | `launchIO` |
| `updateTrackEpisodeSeen` | `withIOContext`, each update in `async` |
| `TorrentServerService.serviceScope` | `CoroutineScope(EmptyCoroutineContext)` (no dispatcher, unscoped) |
| `HttpServerService` | main thread start, server threads inside NanoHTTPD |

Proposal: all mpv calls on the GTK main thread (libmpv calls are not thread-safe except `mpv_wait_event`); IO on tokio; UI updates via `glib::MainContext::spawn_local`.

---

## 21. Timings, debounces and limits

- `setVideo` torrent start wait: 10 (`wait(10)`), about 11 s.
- Torrent `wait()` default: 20 s.
- HTTP server start timeout from UI: 5 s (`MainActivity.kt:647`).
- `demuxer-max-bytes`: 64 MiB; `demuxer-max-back-bytes`: 64 MiB.
- `waitingTimeIntroSkip` default 5 (part 2).
- Progress completion default 0.85 (`pref_progress_preference`).
- Toast length: `Toast.LENGTH_LONG` (`PlayerObserver.efEvent`).
- Volume set to 100 on resume if below max (1.5).
- Debanding radius 12 (gradfun).
- Local server port: ephemeral (0).
- `REMOVE_AFTER_READ_SLOTS` sentinel -1 (disabled).

---

## 22. Interactions with other areas

- Part 2 (`area:45-player-core`): `PlayerViewModel` (init, loadEpisode, loadHosters, changeEpisode, changeVideoAspect, pause/unpause, volume, sheets, panels, dialogs, handleLuaInvocation, saveCurrentEpisodeWatchingProgress, deletePendingEpisodes, onSaveInstanceStateNonConfigurationChange), loader classes, AniSkip, ChapterUtils, TrackSelect, settings preference classes, the subtitle colour helper `toColorHexString` (`controls/components/panels/SubtitleSettingsColorsCard.kt:156`).
- `area:06-data-repos-history-track-source-misc`: `UpsertAnimeHistory`, `UpdateEpisode`, `GetAnime`, `GetEpisodesByAnimeId`, `GetAnimeTracks`, `InsertAnimeTrack`.
- `area:16-source-api-anime`: `AnimeHttpSource.createHttpServer`, `headers`, `HttpServer`, `Video`.
- `area:13-core-util-storage-archive-torrent`: torrent helpers (`TorrentHelpers`), `TorrServer` engine.
- `area:24-downloads-anime`: `AnimeDownloadManager.isEpisodeDownloaded`, `enqueueEpisodesToDelete`.
- `area:27-tracking-core`: `TrackerManager`, `AnimeTracker`, delayed tracking store and job.
- `area:01-foundation-app-bootstrap`: app Application, notifications channels setup (`Notifications.kt`).
- `area:43-reader-core`: unrelated except the shared `Notifications` infrastructure.

---

## 23. Known defects in the Kotlin (recorded, not carried over)

1. `ReverseLandscape` maps to `SCREEN_ORIENTATION_REVERSE_PORTRAIT` (`PlayerActivity.kt:849`).
2. `vf` is overwritten by yuv420p after debanding CPU (`AniyomiMPVView.kt:119-127`).
3. `HttpServer.start()` swallows bind errors (`HttpServer.kt:18-25`), so the service reports running with port 0.
4. `trackId` uses `it.toInt()` which throws on unexpected values (`PlayerActivity.kt:723-729`).
5. `createPipParams` uses `getVideoOutAspect()!!` (`PlayerActivity.kt:791`) and `Rational(height, width.toInt())` where `width` can be 0 (`PlayerActivity.kt:792`).
6. `onKeyUp` uses `event!!` (`PlayerActivity.kt:882`).
7. `setMpvOptions` aborts on the first malformed metadata entry.
8. The torrent notification action label `"Stop"` is hard-coded English.
9. `deleteEpisodeIfNeeded` throws for unknown sorting (`ExternalIntents.kt:501`).
10. `TorrentServerService` companion object eagerly calls `Injekt.get` at class initialisation (`TorrentServerService.kt:143-144`).
11. `http-header-fields` for non-supported players drops the first pair and joins the rest with `: ` (`ExternalIntents.kt:306-307`).
12. `Thread.setDefaultUncaughtExceptionHandler` is process-wide (`PlayerActivity.kt:244`).
13. Typo kept in a key name: `pref_torrserver_tackers` (`TorrentPreferences.kt:14`). The Rust port keeps the stored key for backup compatibility and documents the typo.
14. `HttpServerService.NAME` is `"HttpServerSerivce"` (typo, `HttpServerService.kt:108`); only affects intent action and extra names, which are internal.
15. `copyUserFiles` deletes script directories even when `mpv_scripts` is false (`PlayerActivity.kt:456-458`), which is intended (clean slate), but it also deletes user files copied by hand into the app directory.
