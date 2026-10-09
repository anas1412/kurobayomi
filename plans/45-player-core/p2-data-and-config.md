# 45-player-core (part 2 of 2) — Data models, preferences, files, endpoints, DB

Repo root: `/home/blackbox/aniyomi-src`. All paths repo-relative. Enum values are quoted from source. Sections 2 quotes part-1 enums that this part's code uses, so the port has every value in one place.

## 1. Data models

### 1.1 `Video` — `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt:30-46`
Primary constructor fields (types, defaults):

| Field | Type | Default | Notes |
|---|---|---|---|
| `videoUrl` | `String` (var) | `""` | The literal `"null"` means "resolve lazily via `getVideoUrl`" (Video.kt:69, parseVideoUrls in EpisodeLoader.kt:180-187) |
| `videoTitle` | `String` | `""` | Display title (quality label) |
| `resolution` | `Int?` | `null` | |
| `bitrate` | `Int?` | `null` | |
| `headers` | `okhttp3.Headers?` | `null` | Serialized as `List<Pair<String,String>>` |
| `preferred` | `Boolean` | `false` | Drives HosterLoader.selectBestVideo |
| `subtitleTracks` | `List<Track>` | `[]` | `Track(url: String, lang: String)` (Video.kt:11) |
| `audioTracks` | `List<Track>` | `[]` | same |
| `timestamps` | `List<TimeStamp>` | `[]` | |
| `mpvArgs` | `List<Pair<String,String>>` | `[]` | |
| `ffmpegStreamArgs` | `List<Pair<String,String>>` | `[]` | |
| `ffmpegVideoArgs` | `List<Pair<String,String>>` | `[]` | |
| `internalData` | `String` | `""` | |
| `initialized` | `Boolean` | `false` | Set true by `getResolvedVideo` |
| `memo` | `JsonObject` | `JsonObject.EMPTY` | |

Transient, not serialized: `status: State` (default `State.QUEUE`).

`Video.State` (Video.kt:180-185): `QUEUE`, `LOAD_VIDEO`, `READY`, `ERROR`.

Constants: `Video.MPV_ARGS_TAG = "ANIYOMI_MPV_ARGS"` (Video.kt:188). Regex `localUrl = """http:\/\/localhost:1(?!\d)"""` (Video.kt:189) used by `usesHttpServer()` / `copyHttpServer(port)` (part 1 HTTP server).

Legacy constructor `Video(url, quality, videoUrl: String?, headers, subtitleTracks, audioTracks)` sets `videoTitle = quality`, `videoUrl = videoUrl ?: "null"`, and the page URL in a private field exposed as `url` (Video.kt:60-75).

### 1.2 `Hoster` — `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Hoster.kt:10-70`
| Field | Type | Default |
|---|---|---|
| `hosterUrl` | String | `""` |
| `hosterName` | String | `""` |
| `videoList` | `List<Video>?` | `null` (null = not loaded yet) |
| `internalData` | String | `""` |
| `lazy` | Boolean | `false` (lazy hosters are loaded only on user click) |
| `memo` | JsonObject | `JsonObject.EMPTY` |

Transient `status: Hoster.State` default `IDLE`. `Hoster.State`: `IDLE`, `LOADING`, `READY`, `ERROR`.
`Hoster.NO_HOSTER_LIST = "no_hoster_list"` (Hoster.kt:57). `List<Video>.toHosterList()` wraps videos in one hoster with `hosterUrl = ""`, `hosterName = "no_hoster_list"`, `memo = EMPTY`.
`SerializableHoster(hosterUrl, hosterName, videoList: String?, internalData: String, lazy, memo)` is the JSON wire form (Hoster.kt:73-80); `videoList` is itself a JSON string of `SerializableVideo` (Video.kt:193-260).

### 1.3 `HosterState` — `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/QualitySheet.kt:51-60` (UI state used by this scope)
```
sealed class HosterState(open val name: String)
  Idle(name)                       // not loaded, lazy
  Loading(name)                    // fetching
  Error(name)                      // failed
  Ready(name, videoList: List<Video>, videoState: List<Video.State>)
```
`HosterState.Ready.getChangedAt(index, newVideo, newState)` (QualitySheet.kt:62-72) returns a copy with one slot replaced in both lists.

### 1.4 `ChapterType`, `TimeStamp`, `ThumbnailInfo`, `TileInfo` — source-api
- `enum class ChapterType { Opening, Ending, Recap, MixedOp, Other }` (`Video.kt:14-20`). Serializable, default names as written.
- `data class TimeStamp(start: Double, end: Double, name: String, type: ChapterType = ChapterType.Other)` (`Video.kt:23-28`).
- `open class ThumbnailInfo(tileInfo: List<TileInfo>, imageTileUrls: List<String>)` (`ThumbnailInfo.kt:9-12`).
- `data class TileInfo(imageIndex: Int, timeMs: Long, x: Int, y: Int, width: Int, height: Int)` (`ThumbnailInfo.kt:24-31`). Units: `timeMs` milliseconds; `x,y,width,height` pixels inside the tile image.

### 1.5 `AniSkip` models — `app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/AniSkipApi.kt:68-117`
```
@Serializable data class AniSkipResponse(found: Boolean, results: List<Stamp>?)
@Serializable data class Stamp(interval: AniSkipInterval, skipType: SkipType)
@Serializable enum class SkipType { @SerialName("op") OP, @SerialName("ed") ED, @SerialName("recap") RECAP, @SerialName("mixed-op") MIXED_OP }
@Serializable data class AniSkipInterval(startTime: Double, endTime: Double)
```
Live response (fetched during this task, `https://api.aniskip.com/v2/skip-times/21/1?types[]=ed&types[]=mixed-ed&types[]=mixed-op&types[]=op&types[]=recap&episodeLength=1500`):
```json
{"found":true,"results":[{"interval":{"startTime":1387.996,"endTime":1500},"skipType":"ed","skipId":"b60b5bf4-7fc9-40e9-b178-71d2a4f0e6c9","episodeLength":1500.146},{"interval":{"startTime":28.783,"endTime":118.783},"skipType":"op","skipId":"bf98fb07-c8f8-4399-baa4-1579f03b7e90","episodeLength":1500.04}],"message":"Successfully found skip times","statusCode":200}
```
Fields the Kotlin ignores (`ignoreUnknownKeys = true`): `skipId`, `episodeLength`, `message`, `statusCode`. The `mixed-ed` value was requested but did not occur in this sample. If it occurs, the `SkipType` decode fails for the whole response (no enum coercion is configured) and the result is null (edge case E-04).

`SkipType.getString()` labels: `OP → "Opening"`, `ED → "Ending"`, `RECAP → "Recap"`, `MIXED_OP → "Mixed-op"`. `toChapterType()`: `OP → ChapterType.Opening`, `ED → Ending`, `RECAP → Recap`, `MIXED_OP → MixedOp`.

AniList response consumed by `getMalIdFromAL`: `{"data":{"Media":{"idMal":<number|null>}}}` (the Kotlin uses substring search, not JSON parsing; see behavior §6.6).

### 1.6 Domain/DB models used here
- `tachiyomi.domain.items.episode.model.Episode` (`domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt:7-27`): `id: Long`, `animeId: Long`, `seen: Boolean`, `bookmark: Boolean`, `fillermark: Boolean`, `lastSecondSeen: Long`, `totalSeconds: Long`, `dateFetch: Long`, `sourceOrder: Long`, `url: String`, `name: String`, `dateUpload: Long`, `episodeNumber: Double`, `scanlator: String?`, `summary: String?`, `previewUrl: String?`, `lastModifiedAt: Long`, `version: Long`, `memo: JsonObject`, computed `isRecognizedNumber: Boolean`.
- `EpisodeUpdate` (`domain/.../items/episode/model/EpisodeUpdate.kt:5-…`): `id: Long` required; all others nullable with `null` = "do not change": `animeId, seen, bookmark, fillermark, lastSecondSeen, totalSeconds, dateFetch, sourceOrder, url, name, dateUpload, episodeNumber, scanlator, …`. Used with partial values in this scope: `EpisodeUpdate(id, seen=true)`, `EpisodeUpdate(id, bookmark)`, `EpisodeUpdate(id, fillermark)`, and the full progress update (`seen, bookmark, fillermark, lastSecondSeen, totalSeconds`).
- `AnimeHistoryUpdate(episodeId: Long, seenAt: Date)` (`domain/.../history/anime/model/AnimeHistoryUpdate.kt:5-8`).
- `Anime` fields used: `id`, `source: Long`, `title`, `favorite`, `sorting` (= episodeFlags & 0x600), `unseenFilterRaw`, `downloadedFilterRaw`, `bookmarkedFilterRaw`, `fillermarkedFilterRaw`, `skipIntroLength: Int` (viewer bits 0-7), `skipIntroDisable: Boolean` (viewer bit 56). Definitions: `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt` (area:07).
- Legacy DB model `eu.kanade.tachiyomi.data.database.models.anime.Episode` (used by the player's playlist): mutable fields `last_second_seen`, `total_seconds`, `seen`, `bookmark`, `fillermark`, `episode_number`, `name`, `scanlator`, `url`, `id` (nullable). Converted with `toDbEpisode()` / `toDomainEpisode()!!`.

### 1.7 `IndexedSegment` — `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/SeekBar.kt:53-63`
`data class IndexedSegment(name: String, start: Float, color: Color = Color.Unspecified, index: Int = 0, chapterType: ChapterType = ChapterType.Other)`. Index conventions: `-2` = external (AniSkip/extension) chapter start; `-1` = external end marker or synthetic start at 0; `>= 0` = mpv chapter index. `IndexedSegment.Unspecified = IndexedSegment(name = "", start = 0f)`.

### 1.8 ViewModel-local types (PlayerViewModel.kt)
- `VideoTrack(id: Int, name: String, language: String?)`, `@Immutable` (PlayerViewModel.kt:473-477). `id = -1` = "Off".
- `InitResult(hosterList: List<Hoster>?, videoIndex: Pair<Int,Int>, position: Long?)` (1239-1243).
- `ExceptionWithStringResource(message: String, stringResource: StringResource) : Exception` (1247-1250).
- `EpisodeLoadResult(hosterList: List<Hoster>?, episodeTitle: String, source: AnimeSource)` (1615-1619).
- `SaveImageResult`: `Success(uri: Uri)`, `Error(error: Throwable)` (1942-1945).
- `Event`: `SetArtResult(result: SetAsArt, artType: ArtType)`, `SavedImage(result: SaveImageResult)`, `ShareImage(uri: Uri, seconds: String)` (2160-2164).

## 2. Enums and fixed values (quoted; part 1 owns the file, values used here)

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt` (part 1), plus the others noted.

| Enum | Values (order, stored string/int, title key) | Used by |
|---|---|---|
| `Decoder(title, value)` | `AutoCopy("Auto","auto-copy")`, `Auto("Auto","auto")`, `SW("SW","no")`, `HW("HW","mediacodec-copy")`, `HWPlus("HW+","mediacodec")` (PlayerEnums.kt:81-87); `getDecoderFromValue` = `Decoder.entries.first { it.value == value }` (throws if no match, :89-91) | PlayerViewModel `_currentDecoder`, `updateDecoder` |
| `Debanding` | `None`, `CPU`, `GPU` (PlayerEnums.kt:93-97). Pref `pref_video_debanding` (getEnum by name). Applied in AniyomiMPVView.kt:120-124: `CPU → vf gradfun=radius=12`, `GPU → deband=yes` | DecoderPreferences |
| `Sheets` | `None, PlaybackSpeed, SubtitleTracks, AudioTracks, QualityTracks, Chapters, More, Screenshot` (PlayerEnums.kt:99-108) | `sheetShown` |
| `Panels` | `None, SubtitleSettings, SubtitleDelay, AudioDelay, VideoFilters` (:110-116) | `panelShown` |
| `Dialogs` | `None`, `EpisodeList`, `IntegerPicker(defaultValue, minValue, maxValue, step, nameFormat, title, onChange, onDismissRequest)` (:118-131) | `dialogShown` |
| `PlayerUpdates` | `None`, `DoubleSpeed`, `AspectRatio`, `ShowText(value)`, `ShowTextResource(textResource)` (:133-139) | `playerUpdate` |
| `SingleActionGesture(stringRes)` | `None`, `Seek`, `PlayPause`, `Switch`, `Custom` (:61-67) | gesture prefs |
| `CustomKeyCodes(keyCode)` | `DoubleTapLeft "0x10001"`, `DoubleTapCenter "0x10002"`, `DoubleTapRight "0x10003"`, `MediaPrevious "0x10004"`, `MediaPlay "0x10005"`, `MediaNext "0x10006"` (:72-79) | `keypress` commands |
| `VideoAspect(titleRes)` | `Crop`, `Fit`, `Stretch` (:52-56). Default `Fit` | `aspectState` |
| `PlayerOrientation(titleRes)` | `Free, Video, Portrait, ReversePortrait, SensorPortrait, Landscape, ReverseLandscape, SensorLandscape` (:41-50). Default pref `SensorLandscape` | orientation (Drop) |
| `ArtType` | `Cover, Background, Thumbnail` (:35-39) | `setAsArt` |
| `SetAsArt` | `Success, AddToLibraryFirst, Error` (:29-33) | `setAsArt` result |
| `VideoFilters(titleRes, preference, mpvProperty)` | `BRIGHTNESS → "brightness"`, `SATURATION → "saturation"`, `CONTRAST → "contrast"`, `GAMMA → "gamma"`, `HUE → "hue"` (:141-170). Persisted through `DecoderPreferences` keys `pref_player_filter_{brightness,saturation,contrast,gamma,hue}` | video filter panel |

From this part's own files:

| Enum | Values | File |
|---|---|---|
| `AudioChannels(titleRes, property, value)` | `Auto("audio-channels","auto-safe")`, `AutoSafe("audio-channels","auto")` (default), `Mono("audio-channels","mono")`, `Stereo("audio-channels","stereo")`, `ReverseStereo("af","pan=[stereo|c0=c1|c1=c0]")` | AudioPreferences.kt:36-42 |
| `SubtitleJustification(value, icon)` | `Left("left")`, `Center("center")`, `Right("right")`, `Auto("auto")` (default) | SubtitlePreferences.kt:242-250 |
| `SubtitlesBorderStyle(value, titleRes)` | `OutlineAndShadow("outline-and-shadow")` (default), `OpaqueBox("opaque-box")`, `BackgroundBox("background-box")` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt:368-375` |
| `SkipType` | `OP("op")`, `ED("ed")`, `RECAP("recap")`, `MIXED_OP("mixed-op")` | AniSkipApi.kt:81-92 |
| `ChapterType` | `Opening, Ending, Recap, MixedOp, Other` | source-api Video.kt:14-20 |
| `Video.State` | `QUEUE, LOAD_VIDEO, READY, ERROR` | Video.kt:180-185 |
| `HosterState` | `Idle, Loading, Error, Ready` | QualitySheet.kt:51-60 |

Default VideoFilters values are unset in the preference getters (no default argument), so the store default applies (area:02).

## 3. Anime and episode flag layouts

### 3.1 `animes.episode_flags` (INTEGER NOT NULL, `data/src/main/sqldelightanime/dataanime/animes.sq:24`)
Constants from `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:223-250`:
| Bits | Constant | Value |
|---|---|---|
| sort direction | `EPISODE_SORT_DESC` | `0x0` |
| | `EPISODE_SORT_ASC` | `0x1` |
| | `EPISODE_SORT_DIR_MASK` | `0x1` |
| unseen filter | `EPISODE_SHOW_UNSEEN` | `0x2` |
| | `EPISODE_SHOW_SEEN` | `0x4` |
| | `EPISODE_UNSEEN_MASK` | `0x6` |
| downloaded filter | `EPISODE_SHOW_DOWNLOADED` | `0x8` |
| | `EPISODE_SHOW_NOT_DOWNLOADED` | `0x10` |
| | `EPISODE_DOWNLOADED_MASK` | `0x18` |
| bookmark filter | `EPISODE_SHOW_BOOKMARKED` | `0x20` |
| | `EPISODE_SHOW_NOT_BOOKMARKED` | `0x40` |
| | `EPISODE_BOOKMARKED_MASK` | `0x60` |
| filler filter | `EPISODE_SHOW_FILLERMARKED` | `0x80` |
| | `EPISODE_SHOW_NOT_FILLERMARKED` | `0x100` |
| | `EPISODE_FILLERMARKED_MASK` | `0x180` |
| sorting | `EPISODE_SORTING_SOURCE` | `0x0` |
| | `EPISODE_SORTING_NUMBER` | `0x200` |
| | `EPISODE_SORTING_UPLOAD_DATE` | `0x400` |
| | `EPISODE_SORTING_ALPHABET` | `0x600` |
| | `EPISODE_SORTING_MASK` | `0x600` |
| previews | `EPISODE_SHOW_PREVIEWS` / `EPISODE_SHOW_NOT_PREVIEWS` | `0x0` / `0x800` |
| summaries | `EPISODE_SHOW_SUMMARIES` / `EPISODE_SHOW_NOT_SUMMARIES` | `0x0` / `0x1000` |

Filter raw getters: `unseenFilterRaw = flags & 0x6`, `downloadedFilterRaw = flags & 0x18`, `bookmarkedFilterRaw = flags & 0x60`, `fillermarkedFilterRaw = flags & 0x180`. Value 0 = no filter.

### 3.2 `animes.viewer` column (`viewer_flags` in domain, INTEGER NOT NULL, `animes.sq:23`) — skip-intro bits
From `Anime.kt:310-313`:
| Constant | Value | Bits | Meaning |
|---|---|---|---|
| `ANIME_INTRO_MASK` | `0x00000000000000FF` | 0-7 | `skipIntroLength` in seconds, 0..255 |
| `ANIME_AIRING_EPISODE_MASK` | `0x000000000FFFF00` | 8-23 | next airing episode (`removeHexZeros(2)`) — part of area:07 |
| `ANIME_AIRING_TIME_MASK` | (area:07) | — | airing time |
| `ANIME_INTRO_DISABLE_MASK` | `0x100000000000000` | 56 | skip intro disabled |

Writer (`app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt:13-24`):
```
viewerFlags = anime.viewerFlags
  .setFlag(flag, ANIME_INTRO_MASK)                               // flag and mask
  .setFlag((flag == 0L).toLong().addHexZeros(14), ANIME_INTRO_DISABLE_MASK)
```
`setFlag(flag, mask) = this and mask.inv() or (flag and mask)` (SetAnimeViewerFlags.kt:~line 49-51). `addHexZeros(n) = this * 16^n` (so `addHexZeros(14) = 2^56`). Consequence: any `flag` value is masked to its low 8 bits without error (E-27 in edge cases), and `flag == 0` sets the disable bit.

Reader: `skipIntroLength = (viewer & 0xFF).toInt()`; `skipIntroDisable = (viewer & (1 shl 56)) == 1 shl 56`.

### 3.3 `episodes` and `animehistory` tables (see §7)

## 4. Preference keys with defaults

All preferences are read through `PreferenceStore` (area:02). "non-pref" = marked `// Non-preference(s)` in source; still persisted. Keys prefixed `__APP_STATE_` come from `Preference.appStateKey(key)` = `"__APP_STATE_" + key` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-47`).

### 4.1 AdvancedPlayerPreferences (`settings/AdvancedPlayerPreferences.kt`)
| Key | Type | Default | Method |
|---|---|---|---|
| `mpv_scripts` | Boolean | `false` | `mpvUserFiles()` |
| `pref_mpv_conf` | String | `""` | `mpvConf()` |
| `pref_mpv_input` | String | `""` | `mpvInput()` |
| `pref_player_statistics_page` (non-pref) | Int | `0` | `playerStatisticsPage()` |

### 4.2 AudioPreferences (`settings/AudioPreferences.kt`)
| Key | Type | Default | Method |
|---|---|---|---|
| `pref_audio_lang` | String | `""` | `preferredAudioLanguages()` |
| `pref_audio_pitch_correction` | Boolean | `true` | `enablePitchCorrection()` |
| `pref_audio_config` | enum AudioChannels | `AutoSafe` | `audioChannels()` |
| `pref_audio_volume_boost_cap` | Int | `30` | `volumeBoostCap()` |
| `pref_audio_delay` (non-pref) | Int | `0` | `audioDelay()` |

### 4.3 DecoderPreferences (`settings/DecoderPreferences.kt`)
| Key | Type | Default | Method |
|---|---|---|---|
| `pref_try_hwdec` | Boolean | `true` | `tryHWDecoding()` |
| `pref_gpu_next` | Boolean | `false` | `gpuNext()` |
| `pref_video_debanding` | enum Debanding | `Debanding.None` | `videoDebanding()` |
| `use_yuv420p` | Boolean | `true` | `useYUV420P()` |
| `pref_player_filter_brightness` (non-pref) | Int | no default in source (store default) | `brightnessFilter()` |
| `pref_player_filter_saturation` | Int | store default | `saturationFilter()` |
| `pref_player_filter_contrast` | Int | store default | `contrastFilter()` |
| `pref_player_filter_gamma` | Int | store default | `gammaFilter()` |
| `pref_player_filter_hue` | Int | store default | `hueFilter()` |

Downstream effect (part 1, AniyomiMPVView.kt:113-129 initOptions): `vo = gpu-next` if `gpu_next` else `gpu`; `hwdec = auto` if `try_hwdec` else `no`; `profile = fast`; `keep-open = yes`; `msg-level all=v|warn` by network verbose logging; `use_yuv420p` writes `vf format=yuv420p`, which replaces (not appends to) any `vf` written earlier by debanding `CPU` (`gradfun=radius=12`). Observed conflict, edge case E-26.

### 4.4 GesturePreferences (`settings/GesturePreferences.kt`)
| Key | Type | Default | Method |
|---|---|---|---|
| `pref_gesture_volume_brightness` | Boolean | `true` | `gestureVolumeBrightness()` |
| `pref_swap_volume_and_brightness` | Boolean | `false` | `swapVolumeBrightness()` |
| `pref_gesture_horizontal_seek` | Boolean | `true` | `gestureHorizontalSeek()` |
| `pref_show_seekbar` | Boolean | `false` | `showSeekBar()` |
| `pref_default_intro_length` | Int | `85` | `defaultIntroLength()` |
| `pref_skip_length_preference` | Int | `10` | `skipLengthPreference()` |
| `pref_player_smooth_seek` | Boolean | `false` | `playerSmoothSeek()` |
| `pref_left_double_tap` | enum SingleActionGesture | `Seek` | `leftDoubleTapGesture()` |
| `pref_center_double_tap` | enum | `PlayPause` | `centerDoubleTapGesture()` |
| `pref_right_double_tap` | enum | `Seek` | `rightDoubleTapGesture()` |
| `pref_media_previous` | enum | `Switch` | `mediaPreviousGesture()` |
| `pref_media_playpause` | enum | `PlayPause` | `mediaPlayPauseGesture()` |
| `pref_media_next` | enum | `Switch` | `mediaNextGesture()` |

### 4.5 PlayerPreferences (`settings/PlayerPreferences.kt`)
| Key | Type | Default | Method |
|---|---|---|---|
| `pref_preserve_watching_position` | Boolean | `false` | `preserveWatchingPosition()` |
| `pref_progress_preference` | Float | `0.85F` | `progressPreference()` |
| `pref_default_player_orientation_type_key` | enum PlayerOrientation | `SensorLandscape` | `defaultPlayerOrientationType()` |
| `pref_allow_gestures_in_panels` | Boolean | `false` | `allowGestures()` |
| `pref_show_loading` | Boolean | `true` | `showLoadingCircle()` |
| `pref_show_current_chapter` | Boolean | `true` | `showCurrentChapter()` |
| `pref_remember_brightness` | Boolean | `false` | `rememberPlayerBrightness()` |
| `player_brightness_value` | Float | `-1.0F` | `playerBrightnessValue()` |
| `pref_remember_volume` | Boolean | `false` | `rememberPlayerVolume()` |
| `player_volume_value` | Float | `-1.0F` | `playerVolumeValue()` |
| `pref_show_failed_hosters` | Boolean | `false` | `showFailedHosters()` |
| `pref_show_empty_hosters` | Boolean | `false` | `showEmptyHosters()` |
| `player_fullscreen` | Boolean | `true` | `playerFullscreen()` |
| `player_hide_controls` | Boolean | `false` | `hideControls()` |
| `pref_display_vol_as_per` | Boolean | `true` | `displayVolPer()` |
| `pref_show_system_status_bar` | Boolean | `false` | `showSystemStatusBar()` |
| `pref_reduce_motion` | Boolean | `false` | `reduceMotion()` |
| `pref_player_time_to_disappear` | Int | `4000` | `playerTimeToDisappear()` |
| `pref_panel_opacity` | Int | `60` | `panelOpacity()` |
| `pref_enable_skip_intro` | Boolean | `true` | `enableSkipIntro()` |
| `pref_enable_auto_skip_ani_skip` | Boolean | `false` | `autoSkipIntro()` |
| `pref_enable_netflixStyle_aniskip` | Boolean | `false` | `enableNetflixStyleIntroSkip()` |
| `pref_waiting_time_aniskip` | Int | `5` | `waitingTimeIntroSkip()` |
| `pref_enable_ani_skip` | Boolean | `false` | `aniSkipEnabled()` |
| `pref_disabled_ani_skip_chapters` | Boolean | `true` | `disableAniSkipOnChapters()` |
| `pref_enable_pip` | Boolean | `true` | `enablePip()` |
| `pref_pip_episode_toasts` | Boolean | `true` | `pipEpisodeToasts()` |
| `pref_pip_on_exit` | Boolean | `false` | `pipOnExit()` |
| `pip_replace_with_previous` | Boolean | `false` | `pipReplaceWithPrevious()` |
| `pref_always_use_external_player` | Boolean | `false` | `alwaysUseExternalPlayer()` |
| `external_player_preference` | String | `""` | `externalPlayerPreference()` |
| `pref_player_speed` (non-pref) | Float | `1f` | `playerSpeed()` |
| `default_speed_presets` (non-pref) | StringSet | `{"0.25","0.5","0.75","1.0","1.25","1.5","1.75","2.0","2.5","3.0","3.5","4.0"}` (12 values) | `speedPresets()` |
| `invert_duration` (non-pref) | Boolean | `false` | `invertDuration()` |
| `pref_player_aspect_state` (non-pref) | enum VideoAspect | `Fit` | `aspectState()` |
| `pref_auto_play_enabled` (Old) | Boolean | `false` | `autoplayEnabled()` |

### 4.6 SubtitlePreferences (`settings/SubtitlePreferences.kt`)
| Key | Type | Default | Method |
|---|---|---|---|
| `pref_subtitle_lang` | String | `""` | `preferredSubLanguages()` |
| `pref_subtitle_whitelist` | String | `""` | `subtitleWhitelist()` |
| `pref_subtitle_blacklist` | String | `""` | `subtitleBlacklist()` |
| `pref_screenshot_subtitles` (non-pref) | Boolean | `false` | `screenshotSubtitles()` |
| `pref_subtitle_font` (non-pref) | String | `"Sans Serif"` | `subtitleFont()` |
| `pref_subtitles_font_size` | Int | `55` | `subtitleFontSize()` |
| `pref_sub_scale` | Float | `1f` | `subtitleFontScale()` |
| `pref_sub_border_size` | Int | `3` | `subtitleBorderSize()` |
| `pref_bold_subtitles` | Boolean | `false` | `boldSubtitles()` |
| `pref_italic_subtitles` | Boolean | `false` | `italicSubtitles()` |
| `pref_text_color_subtitles` | Int (ARGB) | `Color.White.toArgb()` = `0xFFFFFFFF` (Int `-1`) | `textColorSubtitles()` |
| `pref_border_color_subtitles` | Int (ARGB) | `Color.Black.toArgb()` = `0xFF000000` (Int `-16777216`) | `borderColorSubtitles()` |
| `pref_border_style_subtitles` | enum SubtitlesBorderStyle | `OutlineAndShadow` | `borderStyleSubtitles()` |
| `sub_shadow_offset` | Int | `0` | `shadowOffsetSubtitles()` |
| `pref_background_color_subtitles` | Int (ARGB) | `Color.Transparent.toArgb()` = `0x00000000` (Int `0`) | `backgroundColorSubtitles()` |
| `pref_sub_justify` | enum SubtitleJustification | `Auto` | `subtitleJustification()` |
| `pref_sub_pos` | Int | `100` | `subtitlePos()` |
| `pref_override_subtitles_ass` | Boolean | `false` | `overrideSubsASS()` |
| `pref_subtitles_delay` | Int (ms) | `0` | `subtitlesDelay()` |
| `pref_subtitles_speed` | Float | `1f` | `subtitlesSpeed()` |
| `pref_subtitles_secondary_delay` | Int (ms) | `0` | `subtitlesSecondaryDelay()` |

Preference keys read outside these files by this scope:
| Key | Type | Default | Read in |
|---|---|---|---|
| `__APP_STATE_pref_downloaded_only` | Boolean | `false` | `BasePreferences.downloadedOnly()` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:16-19`) |
| `__APP_STATE_incognito_mode` | Boolean | `false` | `BasePreferences.incognitoMode()` (:21) |
| `pref_auto_update_manga_sync_key` | Boolean | `true` | `TrackPreferences.autoUpdateTrack()` (`app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt:39`) |
| `relative_time_v2` | Boolean | `true` | `UiPreferences.relativeTime()` (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:33`) |
| `app_date_format` | String | `""` | `UiPreferences.dateFormat()` (:35) |
| `auto_download_while_watching` | Int | `0` | `DownloadPreferences.autoDownloadWhileWatching()` (`domain/.../download/service/DownloadPreferences.kt:26`) |
| `remove_after_read_slots` | Int | `-1` | `DownloadPreferences.removeAfterReadSlots()` (:28) |
| `mark_duplicate_seen_episode_seen` | StringSet | `{}` | `LibraryPreferences.markDuplicateSeenEpisodeAsSeen()` (`domain/.../library/service/LibraryPreferences.kt:379`); marker value `MARK_DUPLICATE_EPISODE_SEEN_EXISTING = "existing_episode"` (:411) |

## 5. Network endpoints

### 5.1 AniSkip skip-times (GET)
- URL template: `https://api.aniskip.com/v2/skip-times/{malId}/{episodeNumber}?types[]=ed&types[]=mixed-ed&types[]=mixed-op&types[]=op&types[]=recap&episodeLength={episodeLength}` (AniSkipApi.kt:23-25).
- `malId: Int`, `episodeNumber: Int` (truncated from `Double`), `episodeLength: Long` (player duration in whole seconds).
- Method GET. Headers: from `GET(url)` in `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt:20-30`: `Headers` default `Headers.Builder().build()` (no extra headers), and `Cache-Control: max-age=600` (`CacheControl.Builder().maxAge(10, MINUTES)`, Requests.kt:16). No auth.
- Client: a bare `OkHttpClient()` created per `AniSkipApi` instance (AniSkipApi.kt:18). Not the app-wide client.
- Response: JSON, shape in §1.5. Success: `found = true`, `results` non-null. `found = false` → null result.
- Errors: any exception → null (no retry, no user message).

### 5.2 AniList id→MAL id lookup (POST)
- URL: `https://graphql.anilist.co`.
- Body: JSON `{"query": "query{\n Media(id:<ID>){idMal}\n }"}` built with `buildJsonObject { put("query", query) }` where `query` is a raw string with `trimMargin()`; `<ID>` is the AniList id as a decimal Long, interpolated (no GraphQL variables).
- Content-Type: `application/json; charset=utf-8` (`jsonMime`, `core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:22`).
- Headers: `POST(url, body=...)` default headers (empty) and default `Cache-Control` as in 5.1 (POST helper has the same defaults; Requests.kt, see `DEFAULT_HEADERS`).
- Response: `{"data":{"Media":{"idMal":<number|null>}}}`. Kotlin parses it with `substringAfter("idMal\":").substringBefore("}")`, then `toLongOrNull() ?: 0`.
- Errors: exception → `0`.

### 5.3 Other network use
None in this scope. The thumbnail tiles and video URLs are fetched through `AnimeHttpSource` methods (`getImageTile`, `getVideoThumbnails`, `getVideoUrl`, `resolveVideo`), owned by area:16/17.

## 6. Files, folders and formats

| Path | Writer | Format | Lifetime |
|---|---|---|---|
| `${activity.cacheDir}/<epochMillis>_mpv_screenshot_tmp.png` | `takeScreenshot` (PlayerViewModel.kt:1831) via `screenshot-to-file` | PNG (mpv) | renamed immediately |
| `${cacheDir}/mpv_screenshot.png` | `takeScreenshot` (:1836) | PNG; overwritten each capture | until next capture |
| `context.cacheImageDir` (recursively deleted before each share, :1897) | `shareImage` through `ImageSaver` with `Location.Cache` | PNG, name = generated filename | only the last shared image is kept |
| `Pictures/<DiskUtil.buildValidFilename(anime.title)>/<generateFilename>` | `saveImage` through `ImageSaver` with `Location.Pictures(relativePath)` | PNG; extension appended by ImageSaver (area:13) | permanent |
| Art targets (cover, background, thumbnail) | `setAsArt` via `editCover` / `editBackground` / `editThumbnail` (area:34) | image | permanent |
| `generateFilename` output | `DiskUtil.buildValidFilename(takeBytes("<title> - <episode name>", 250 - byteSize(" - <time>")))` + `" - <time>"` | string | — |
| Session restore (Rust proposal) | `$XDG_STATE_HOME/kurobayomi/player-session.json` (defaults to `~/.local/state/...`) | JSON `{episode_id, episode_position_ms, quality_index:[h,v]}` | replaced on each `onSecondReached` tick (throttled) |

`DiskUtil.MAX_FILE_NAME_BYTES = 250` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:143`).

Process-death savedState keys (SavedStateHandle, PlayerViewModel.kt:1120-1142): `"episode_position"` (Long ms), `"quality_index"` (Pair<Int,Int>), `"episode_id"` (Long, default -1).

## 7. mpv properties and commands

### 7.1 Properties read
| Property | Read at | Type call |
|---|---|---|
| `hwdec` | PlayerViewModel.kt:204 | String |
| `track-list/count` | :428 | Int? |
| `track-list/$i/type`, `/lang`, `/title`, `/id` | :395-415, 430-434 | String / String / String / Int |
| `chapter-list/count` | :481 | Int (`!!`) |
| `chapter-list/$i/title`, `/time` | :483-484 | String / Int (`!!`) |
| `volume` | :293, :767 | Int? |
| `volume-max` | :294 | Int? |
| `user-data/aniyomi/<name>` | observed (PlayerActivity.kt:740) | String |
| `time-pos`, `duration`, `demuxer-cache-time`, `pause`, `sid`, `secondary-sid`, `aid`, `speed`, `video-params/aspect`, `hwdec-current`, `paused-for-cache` | observed in AniyomiMPVView.kt:210-230 | per format list |

### 7.2 Properties written
| Property | Written at | Value |
|---|---|---|
| `hwdec` | :390 | `Decoder.value` |
| `user-data/current-anime/anime-title` | :1285 | anime title |
| `user-data/current-anime/intro-length` | :1286 | `getAnimeSkipIntroLength()` Int |
| `user-data/current-anime/category` | :1287-1292 | `categories.joinToString { name }` (separator `", "`) |
| `user-data/current-anime/episode-number` | :1333 | `episode_number` Double |
| `volume` | :790 | Int 0..(100+cap) |
| `panscan` | :823, :833 | Double 0.0 or 1.0 |
| `video-aspect-override` | :834 | Double -1.0 or display ratio |
| `user-data/aniyomi/<name>` | :960 | `""` (clears the request) |
| `user-data/aniyomi` values via `launch_int_picker` | :911 | `setPropertyInt(pickerProperty, it)` |
| `brightness`, etc. (mpv video filters) | part 1 | from DecoderPreferences filters |
| `sid`, `secondary-sid`, `aid`, `paused` | via `activity.player` setters | Int / Boolean |

### 7.3 Commands
| Command | Form | Site |
|---|---|---|
| `seek` | `["seek", offset, "relative"|"relative+exact"]` | :744 |
| `seek` | `["seek", pos, "absolute"|"absolute+keyframes"]` | :749 |
| `audio-add` | `["audio-add", path, "cached"[, title]]` | :517, :519 |
| `sub-add` | `["sub-add", path, "cached"[, title]]` | :538, :540 |
| `screenshot-to-file` | `["screenshot-to-file", filename, "subtitles"|"video"]` | :1834 |
| `keypress` | `["keypress", "0x1000x"]` | :1060, :1073, :1090 |
| `script-message` | `["script-message", "call_button_<id>"]` / `"call_button_<id>_long"` | :2168, :2172 |

## 8. Database tables and columns touched

### 8.1 `episodes` (`data/src/main/sqldelightanime/dataanime/episodes.sq`)
Columns read/written through `EpisodeUpdate`/`toDbEpisode`: `_id`, `anime_id`, `url`, `name`, `scanlator`, `seen` (INTEGER AS Boolean), `bookmark` (INTEGER AS Boolean), `last_second_seen` (INTEGER NOT NULL), `total_seconds` (INTEGER NOT NULL), `episode_number`, `source_order`, `date_fetch`, `date_upload`, `last_modified_at`, `version`, `is_syncing`, `summary`, `preview_url`, `fillermark` (INTEGER AS Boolean), `memo`.

Triggers relevant to playback:
- `update_episode_and_anime_version` (AFTER UPDATE, episodes.sq ~line 41): when `new.is_syncing = 0 AND (seen != old.seen OR bookmark != old.bookmark OR last_second_seen != old.last_second_seen)` → `UPDATE episodes SET version = version + 1 WHERE _id = new._id`, then the anime version update.
- A second trigger writes `last_modified_at = strftime('%s','now')` on update of the row (episodes.sq ~lines 30-38).

Indexes: `episodes_unseen_by_anime_index ON episodes(anime_id, seen) WHERE seen = 0` (episodes.sq:30).

Progress write shape (PlayerViewModel.kt:1776-1785): `UPDATE episodes SET seen=?, bookmark=?, fillermark=?, last_second_seen=?, total_seconds=? WHERE _id=?` (partial-column update semantics per interactor, area:05).

### 8.2 `animehistory` (`data/src/main/sqldelightanime/dataanime/animehistory.sq`)
Table: `_id INTEGER NOT NULL PRIMARY KEY`, `episode_id INTEGER NOT NULL UNIQUE`, `last_seen INTEGER AS Date`, FK `episode_id → episodes(_id) ON DELETE CASCADE`. Index `animehistory_history_episode_id_index`.
Upsert (animehistory.sq:58-65):
```
INSERT INTO animehistory(episode_id, last_seen)
VALUES (:episodeId, :seenAt)
ON CONFLICT(episode_id)
DO UPDATE
SET last_seen = :seenAt
WHERE episode_id = :episodeId;
```
Used by `saveEpisodeHistory` (PlayerViewModel.kt:1796). Reset query `resetAnimeHistoryById`/`resetHistoryByAnimeId` sets `last_seen = 0` (area:06).

### 8.3 `animes` (`animes.sq`)
`viewer INTEGER NOT NULL` (domain `viewerFlags`, column is named `viewer` in SQL; `UPDATE ... viewer = coalesce(:viewer, viewer)` at line 185), `episode_flags INTEGER NOT NULL` (line 24). Skip-intro length is stored in `viewer` bits 0-7 (§3.2).

### 8.4 Read-only in scope
- `episodes` by anime id (`GetEpisodesByAnimeId`), next episodes (`GetNextEpisodes`), `animes` by id (`GetAnime`), categories (`GetAnimeCategories`), tracks (`GetAnimeTracks`), custom buttons (`GetCustomButtons` — `tachiyomi.domain.custombuttons`, area:45 part 1), incognito state (`GetAnimeIncognitoState`).

## 9. Process-local state (not persisted by Kotlin; decision for Rust in brackets)
- `thumbnailTileCache` (LRU 3 bitmaps) [in-memory, same].
- `waitingSkipIntro` countdown [in-memory].
- `lastThumbnailFetch`, `hasTrackers`, `incognitoMode` (lazy), `downloadAheadAmount`, `introSkipEnabled`, `autoSkip`, `netflixStyle`, `defaultWaitingTime`, `doubleTapToSeekDuration`, `preciseSeek`, `showSeekBar` setting snapshot [in-memory, snapshotted at session start, same].
- `currentHosterList`, `qualityIndex`, `episodeId`, `episodePosition` [persisted per §6 Rust proposal].
- `episodeToDownload` [always null in this file; see E-18].
