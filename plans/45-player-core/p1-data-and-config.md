# Area 45 (player core, part 1): data models, preferences, files, endpoints, tables

Paths are repo-relative to `/home/blackbox/aniyomi-src`. Section 3.3 (subtitle option mapping), section 4 (golden default mpv option list) and section 5 (tracker list) are referenced from `p1-tasks.md`.

---

## 1. Files and folders

### 1.1 Android layout (OBSERVED)

| Purpose | Android path | Source |
|---|---|---|
| mpv runtime directory | `<filesDir>/mpv/` (constant `MPV_DIR = "mpv"`) | `PlayerActivity.kt:176, 427` |
| mpv config file | `<filesDir>/mpv/mpv.conf` | `PlayerActivity.kt:429-430` |
| mpv input file | `<filesDir>/mpv/input.conf` | `PlayerActivity.kt:431-432` |
| bundled Lua bridge | `<filesDir>/mpv/scripts/aniyomi.lua` | `PlayerActivity.kt:483-487` |
| custom buttons script | `<filesDir>/mpv/scripts/custombuttons.lua` | `PlayerActivity.kt:573` |
| bundled subtitle font | `<filesDir>/mpv/subfont.ttf` | `PlayerActivity.kt:492` |
| bundled CA bundle | `<filesDir>/mpv/cacert.pem` | `PlayerActivity.kt:492`, `AniyomiMPVView.kt:135` |
| copied fonts | `<filesDir>/mpv/fonts/` | `PlayerActivity.kt:521` |
| copied scripts | `<filesDir>/mpv/scripts/` | `PlayerActivity.kt:452-467` |
| copied script options | `<filesDir>/mpv/script-opts/` | `PlayerActivity.kt:453, 468-473` |
| copied shaders | `<filesDir>/mpv/shaders/` | `PlayerActivity.kt:454, 474-479` |
| user's mpv config root | `<baseDir>/mpv-config/` (`MPV_CONFIG_PATH = "mpv-config"`) | `domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt:94, 102` |
| user fonts | `<baseDir>/mpv-config/fonts/` (`FONTS_PATH`) | `StorageManager.kt:103, 78-79` |
| user scripts | `<baseDir>/mpv-config/scripts/` (`SCRIPTS_PATH = "scripts"`) | `StorageManager.kt:104, 81-83` |
| user script options | `<baseDir>/mpv-config/script-opts/` (`SCRIPT_OPTS_PATH`) | `StorageManager.kt:105, 85-87` |
| user shaders | `<baseDir>/mpv-config/shaders/` (`SHADERS_PATH = "shaders"`) | `StorageManager.kt:106, 89-91` |
| mpv cache dir | `applicationContext.cacheDir` | `PlayerActivity.kt:443` |
| screenshots | `Environment.DIRECTORY_PICTURES` public dir | `AniyomiMPVView.kt:142-144` |
| torrent engine data path | `filesDir.absolutePath` | `TorrentServerService.kt:73` |

Note: `StorageManager` creates the `mpv-config` folder tree (fonts, scripts, script-opts, shaders) at `StorageManager.kt:44-48` when the base directory is set up.

### 1.2 Linux layout (Proposal)

| Purpose | Linux path |
|---|---|
| app data root | `$XDG_DATA_HOME/kurobayomi/` (fallback `~/.local/share/kurobayomi/`) |
| mpv runtime (regenerated each window open) | `<data>/mpv/` containing `mpv.conf`, `input.conf`, `scripts/`, `script-opts/`, `shaders/`, `fonts/`, `cacert.pem`, `subfont.ttf` |
| user mpv config root (kept as-is for compatibility) | `<data>/mpv-config/{fonts,scripts,script-opts,shaders}` |
| cache | `$XDG_CACHE_HOME/kurobayomi/` (mpv `cache-dir` not set; cache is in memory) |
| screenshots | `$XDG_PICTURES_DIR/Kurobayomi/` (fallback `~/Pictures/Kurobayomi/`) |
| torrent engine data | `<data>/torrserver/` |
| local HTTP server | none on disk; bound to `127.0.0.1` with an ephemeral port |

Proposal: the fonts directory is not copied into `<data>/mpv/fonts`. mpv is pointed at `<data>/mpv-config/fonts` directly (`sub-fonts-dir`, `osd-fonts-dir`), removing the copy hack noted at `PlayerActivity.kt:518`.

### 1.3 File write rules (OBSERVED)

- `mpv.conf`, `input.conf`: created with `createFile` and fully rewritten on every window open (`PlayerActivity.kt:429-432`).
- `aniyomi.lua`: rewritten on every open (`PlayerActivity.kt:483-487`).
- `custombuttons.lua`: rewritten every time `setupCustomButtons` runs (`PlayerActivity.kt:573-576`).
- Copied scripts, script-opts, shaders: the three directories are deleted first (`PlayerActivity.kt:456-458`).
- Assets: copied only when the sizes differ (`PlayerActivity.kt:501`).

---

## 2. Data models

### 2.1 Video (source-api) `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt:30-45`

`data class Video` (all fields, type, default):

| Field | Type | Default |
|---|---|---|
| `videoUrl` | String (var) | `""` |
| `videoTitle` | String | `""` |
| `resolution` | Int? | `null` |
| `bitrate` | Int? | `null` |
| `headers` | okhttp3 `Headers?` | `null` |
| `preferred` | Boolean | `false` |
| `subtitleTracks` | `List<Track>` | `emptyList()` |
| `audioTracks` | `List<Track>` | `emptyList()` |
| `timestamps` | `List<TimeStamp>` | `emptyList()` |
| `mpvArgs` | `List<Pair<String,String>>` | `emptyList()` |
| `ffmpegStreamArgs` | `List<Pair<String,String>>` | `emptyList()` |
| `ffmpegVideoArgs` | `List<Pair<String,String>>` | `emptyList()` |
| `internalData` | String | `""` |
| `initialized` | Boolean | `false` |
| `memo` | `JsonObject` | `JsonObject.EMPTY` |
| `status` (transient, @Volatile) | `Video.State` | `State.QUEUE` |

Non-data members: deprecated `quality` (= `videoTitle`), `url` (= private `videoPageUrl`), and two compatibility constructors (lines 60-107) that are kept for extension library 16.

`Video.State` enum: `QUEUE`, `LOAD_VIDEO`, `READY`, `ERROR`.

Companion constants:

- `MPV_ARGS_TAG = "ANIYOMI_MPV_ARGS"` (`Video.kt:188`).
- `localUrl = Regex("""http:\/\/localhost:1(?!\d)""")` (`Video.kt:189`), used by `usesHttpServer()` and `copyHttpServer(port)`.

Methods (lines 151-178): `usesHttpServer(): Boolean`, `copyHttpServer(port: Int): Video` replacing the regex matches in `videoUrl`, audio track URLs and subtitle URL with `http://localhost:<port>`.

### 2.2 Track, TimeStamp, ChapterType (`Video.kt:10-28`)

- `data class Track(val url: String, val lang: String)` (`@Serializable`).
- `enum class ChapterType { Opening, Ending, Recap, MixedOp, Other }` (`@Serializable`, serial values are the names).
- `data class TimeStamp(val start: Double, val end: Double, val name: String, val type: ChapterType = ChapterType.Other)` (`@Serializable`).

### 2.3 SerializableVideo (`Video.kt:193-260`)

JSON shape used for passing videos between activities (`List<Video>.serialize()` / `String.toVideoList()`):

`videoUrl` (String), `videoTitle` (String), `resolution` (Int?), `bitrate` (Int?), `headers` (`List<Pair<String,String>>?`, from `Headers.toList()`), `preferred` (Boolean), `subtitleTracks` (List<Track>), `audioTracks` (List<Track>), `timestamps` (List<TimeStamp>), `mpvArgs` (List<Pair<String,String>>), `ffmpegStreamArgs`, `ffmpegVideoArgs` (both List<Pair<String,String>>), `internalData` (String), `initialized` (Boolean), `memo` (JsonObject). All with the same defaults as `Video`.

Deserialisation rebuilds `headers` with `Headers.headersOf(*flatPairs)`.

### 2.4 HttpServer (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/HttpServer.kt`)

- `open class HttpServer : NanoHTTPD(0)`.
- `url` = `"http://localhost:$listeningPort"`.
- `isRunning()`; private `@Volatile var isRunning = false`.
- `companion object { const val PLACEHOLDER_URL = "http://localhost:1" }`.

### 2.5 Torrent models (`core/common/src/main/java/aniyomi/core/common/torrent/model/`)

`Torrent.kt`:

| Field (Kotlin) | JSON name | Type | Default |
|---|---|---|---|
| `title` | `title` | String | required |
| `hash` | `hash` | String? | `null` |
| `torrentSize` | `torrent_size` | Long? | `null` |
| `trackers` | `trackers` | `List<String>?` | `null` |
| `fileStats` | `file_stats` | `List<FileStats>?` | `null` |

`FileStats`: `id` (Int?, default null; `id` is the 1-based index from torrent parsing), `path` (String, required), `length` (Long, required). Serialization name for all fields is the Kotlin name, except the `@SerialName` values shown.

`TorrentRequest.kt`:

| Field | JSON name | Type | Default |
|---|---|---|---|
| `action` | `action` | String | required (`"add"` used) |
| `hash` | `hash` | String | `""` |
| `link` | `link` | String | `""` |
| `title` | `title` | String | `""` |
| `poster` | `poster` | String | `""` |
| `data` | `data` | String | `""` |
| `saveToDb` | `save_to_db` | Boolean | `false` |

### 2.6 Episode models

Domain `tachiyomi.domain.items.episode.model.Episode` (`domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt:6-27`):

`id: Long`, `animeId: Long`, `seen: Boolean`, `bookmark: Boolean`, `fillermark: Boolean`, `lastSecondSeen: Long`, `totalSeconds: Long`, `dateFetch: Long`, `sourceOrder: Long`, `url: String`, `name: String`, `dateUpload: Long`, `episodeNumber: Double`, `scanlator: String?`, `summary: String?`, `previewUrl: String?`, `lastModifiedAt: Long`, `version: Long`, `memo: JsonObject`, and derived `isRecognizedNumber: Boolean`.

Legacy `eu.kanade.tachiyomi.data.database.models.anime.Episode` (used by `PlayerViewModel`, part 2): `id: Long?`, `anime_id: Long?`, `seen: Boolean`, `bookmark: Boolean`, `last_second_seen: Long`, `total_seconds: Long`, `date_fetch: Long`, `source_order: Int`, `last_modified: Long`, `version: Long`, plus `episode_number` (used in `PlayerActivity.kt:1394`). Units of `last_second_seen` and `total_seconds` are milliseconds (`PlayerViewModel.onSecondReached` multiplies by 1000, `PlayerViewModel.kt:1665-1666`).

Update payloads used in scope:

- `EpisodeUpdate(id, seen, bookmark, fillermark, lastSecondSeen, totalSeconds)` (`ExternalIntents.kt:469-478`).
- `AnimeHistoryUpdate(episodeId: Long, seenAt: Date)` (`ExternalIntents.kt:443-445`).

### 2.7 Anime episode flags (bit layout)

Source: `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:225-260`. Column `episode_flags INTEGER NOT NULL` in `animes` (`data/src/main/sqldelightanime/dataanime/animes.sq:24`).

| Name | Value | Used here |
|---|---|---|
| `EPISODE_SORTING_SOURCE` | `0x00000000` | yes (sort by `sourceOrder`, descending) |
| `EPISODE_SORTING_NUMBER` | `0x00000200` | yes (ascending by `episodeNumber`) |
| `EPISODE_SORTING_UPLOAD_DATE` | `0x00000400` | yes (ascending by `dateUpload`) |
| `EPISODE_SORTING_ALPHABET` | `0x00000600` | NOT handled: throws `NotImplementedError` (defect) |
| `EPISODE_SORTING_MASK` | `0x00000600` | `anime.sorting = episodeFlags and 0x600` |

Other masks in the same field (not used in scope): unseen `0x6`, downloaded `0x18`, bookmarked `0x60`, fillermarked `0x180`, previews `0x800`, summaries `0x1000`, display `0x100000`.

### 2.8 Custom buttons

`data class CustomButton(id: Long, name: String, isFavorite: Boolean, sortIndex: Long, content: String, longPressContent: String, onStartup: String)` (`domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt:3-11`). Placeholders `$id` and `$isPrimary` are replaced in content, long-press content and on-startup text (`CustomButton.kt:12-25`), with `$isPrimary` = `"true"` when the button id equals the primary button id.

SQL table `custom_buttons` (`data/src/main/sqldelightanime/dataanime/custom_buttons.sq:3-11`): `_id INTEGER NOT NULL PRIMARY KEY`, `name TEXT NOT NULL`, `isFavorite INTEGER AS Boolean NOT NULL`, `sortIndex INTEGER NOT NULL`, `content TEXT NOT NULL`, `longPressContent TEXT NOT NULL`, `onStartup TEXT NOT NULL`.

Seeded default row (`custom_buttons.sq:13-17`): `_id = 1`, `name = '+85 s'`, `isFavorite = 1`, `sortIndex = 0`, content reads `mp.get_property_number("user-data/current-anime/intro-length")` then `aniyomi.right_seek_by(intro_length)`, long-press is `aniyomi.int_picker("Change intro length", "%ds", 0, 255, 1, "user-data/current-anime/intro-length")`, on-startup defines `update_button(_, length)`. The `"%ds"` name format confirms the `int_picker` name_format is a printf-like template.

### 2.9 Player enums and values

| Enum (file) | Values (stored or passed form) |
|---|---|
| `SetAsArt` (`PlayerEnums.kt:29-33`) | `Success`, `AddToLibraryFirst`, `Error` |
| `ArtType` (`PlayerEnums.kt:35-39`) | `Cover`, `Background`, `Thumbnail` |
| `PlayerOrientation` (`PlayerEnums.kt:41-50`) | `Free`, `Video`, `Portrait`, `ReversePortrait`, `SensorPortrait`, `Landscape`, `ReverseLandscape`, `SensorLandscape`. Stored by name (`getEnum`). Default `SensorLandscape` (`PlayerPreferences.kt:16-19`). |
| `VideoAspect` (`PlayerEnums.kt:52-56`) | `Crop`, `Fit`, `Stretch`. Stored by name. Default `Fit` (`PlayerPreferences.kt:81`). |
| `SingleActionGesture` (`PlayerEnums.kt:61-67`) | `None`, `Seek`, `PlayPause`, `Switch`, `Custom`. Stored by name. |
| `CustomKeyCodes` (`PlayerEnums.kt:72-79`) | `DoubleTapLeft="0x10001"`, `DoubleTapCenter="0x10002"`, `DoubleTapRight="0x10003"`, `MediaPrevious="0x10004"`, `MediaPlay="0x10005"`, `MediaNext="0x10006"` |
| `Decoder` (`PlayerEnums.kt:81-87`) | `AutoCopy(title "Auto", value "auto-copy")`, `Auto("Auto","auto")`, `SW("SW","no")`, `HW("HW","mediacodec-copy")`, `HWPlus("HW+","mediacodec")`. Lookup `getDecoderFromValue` = first entry with matching value (throws `NoSuchElementException` if none, `PlayerEnums.kt:89-91`). |
| `Debanding` (`PlayerEnums.kt:93-97`) | `None`, `CPU`, `GPU` |
| `Sheets` (`PlayerEnums.kt:99-108`) | `None`, `PlaybackSpeed`, `SubtitleTracks`, `AudioTracks`, `QualityTracks`, `Chapters`, `More`, `Screenshot` |
| `Panels` (`PlayerEnums.kt:110-116`) | `None`, `SubtitleSettings`, `SubtitleDelay`, `AudioDelay`, `VideoFilters` |
| `Dialogs` (sealed, `PlayerEnums.kt:118-131`) | `None`; `EpisodeList`; `IntegerPicker(defaultValue: Int, minValue: Int, maxValue: Int, step: Int, nameFormat: String, title: String, onChange: (Int)->Unit, onDismissRequest: ()->Unit)` |
| `PlayerUpdates` (sealed, `PlayerEnums.kt:133-139`) | `None`; `DoubleSpeed`; `AspectRatio`; `ShowText(value: String)`; `ShowTextResource(textResource: StringResource)` |
| `VideoFilters` (`PlayerEnums.kt:141-171`) | `BRIGHTNESS` (prop `brightness`, pref `pref_player_filter_brightness`), `SATURATION` (`saturation`, `pref_player_filter_saturation`), `CONTRAST` (`contrast`, `pref_player_filter_contrast`), `GAMMA` (`gamma`, `pref_player_filter_gamma`), `HUE` (`hue`, `pref_player_filter_hue`). All stored as Int with default 0. |
| `AudioChannels` (`ui/player/settings/AudioPreferences.kt:43-49`) | `Auto` (`audio-channels`, `auto-safe`), `AutoSafe` (`audio-channels`, `auto`), `Mono` (`audio-channels`, `mono`), `Stereo` (`audio-channels`, `stereo`), `ReverseStereo` (`af`, `pan=[stereo|c0=c1|c1=c0]`) |
| `SubtitlesBorderStyle` (`ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt:368-375`) | `OutlineAndShadow` (`outline-and-shadow`), `OpaqueBox` (`opaque-box`), `BackgroundBox` (`background-box`) |
| `SubtitleJustification` (`ui/player/settings/SubtitlePreferences.kt:56-64`) | `Left` (`left`), `Center` (`center`), `Right` (`right`), `Auto` (`auto`) |
| `ProxyMode` (`core/common/.../TorrentPreferences.kt:46-51`) | `None` (`tracker`), `Tracker` (`tracker`), `Peers` (`peers`), `Full` (`full`) |
| `ChapterType` | `Opening`, `Ending`, `Recap`, `MixedOp`, `Other` |

Note: `Decoder` values are the mpv `hwdec` option values. `ProxyMode.None` and `Tracker` share the value `tracker`.

### 2.10 Enum storage encoding

`PreferenceStore.getEnum` stores `enum.name` and reads it back with `enumValueOf`, falling back to the default on `IllegalArgumentException` (`core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt:27-45`). An unknown stored value therefore silently resets to the default.

### 2.11 Track ids

mpv track ids are integers. `"no"` means disabled and is represented as `-1` in the app (`AniyomiMPVView.kt:95-112`). `"auto"` is treated as "no change" in observers (`PlayerActivity.kt:723-729`).

---

## 3. Preferences used in scope

Store: `tachiyomi.core.common.preference.PreferenceStore`. `getInt` default is 0 (`PreferenceStore.kt:9`). Keys are case-sensitive. Type column: B = Boolean, I = Int, F = Float, S = String, E = enum (stored as name).

### 3.1 Player and window preferences read by in-scope code

| Key | Type | Default | Read by |
|---|---|---|---|
| `pref_preserve_watching_position` | B | `false` | `PlayerActivity.kt:1074`, `ExternalIntents.kt:174` |
| `pref_progress_preference` | F | `0.85F` | `ExternalIntents.kt:467` |
| `pref_default_player_orientation_type_key` | E `PlayerOrientation` | `SensorLandscape` | `PlayerActivity.kt:837` (dropped) |
| `pref_always_use_external_player` | B | `false` | part 2 (read to choose external path) |
| `external_player_preference` | S | `""` | `ExternalIntents.kt:107` |
| `pref_player_speed` | F | `1f` | `AniyomiMPVView.kt:150` |
| `default_speed_presets` | StringSet | `0.25,0.5,0.75,1.0,1.25,1.5,1.75,2.0,2.5,3.0,3.5,4.0` | part 2 |
| `pref_player_aspect_state` | E `VideoAspect` | `Fit` | `PlayerActivity.kt:652` |
| `pref_auto_play_enabled` | B | `false` | `PlayerActivity.kt:1409` |
| `pref_enable_pip` | B | `true` | `PlayerActivity.kt:141` (Android PiP; mini-player reuses the key) |
| `pref_pip_episode_toasts` | B | `true` | `PlayerActivity.kt:1014` |
| `pref_pip_on_exit` | B | `false` | `PlayerActivity.kt:273, 359, 367, 775` |
| `pip_replace_with_previous` | B | `false` | `PlayerActivity.kt:783` |
| `player_fullscreen` | B | `true` | `PlayerActivity.kt:398` |
| `pref_remember_brightness` | B | `false` | `PlayerActivity.kt:346, 405` (dropped) |
| `player_brightness_value` | F | `-1.0F` | `PlayerActivity.kt:346-347, 406` (dropped) |
| `pref_remember_volume` | B | `false` | part 2 (used by resume rule, T-45a-046) |
| `player_volume_value` | F | `-1.0F` | part 2 |
| `pref_player_statistics_page` | I | `0` | `AniyomiMPVView.kt:163` (advanced prefs) |
| `pref_skip_length_preference` | I | `10` | part 2 (left/right seek amount, media session) |
| `pref_show_loading` | B | `true` | part 2 |

### 3.2 Gesture preferences read by in-scope code (`ui/player/settings/GesturePreferences.kt`)

| Key | Type | Default | Read by |
|---|---|---|---|
| `pref_media_previous` | E `SingleActionGesture` | `Switch` | `PlayerActivity.kt:887` |
| `pref_media_playpause` | E `SingleActionGesture` | `PlayPause` | `PlayerActivity.kt:888` |
| `pref_media_next` | E `SingleActionGesture` | `Switch` | `PlayerActivity.kt:889` |
| `pref_left_double_tap` | E | `Seek` | part 2 (keys 864, 869) |
| `pref_center_double_tap` | E | `PlayPause` | part 2 |
| `pref_right_double_tap` | E | `Seek` | part 2 (keys 865, 870) |
| `pref_skip_length_preference` | I | `10` | part 2 |
| `pref_default_intro_length` | I | `85` | part 2 |

### 3.3 Subtitle preferences and mpv option mapping (`ui/player/settings/SubtitlePreferences.kt`, `AniyomiMPVView.kt:261-289`)

| Preference key | Type | Default (Kotlin) | mpv option | Value sent (default) |
|---|---|---|---|---|
| `pref_subtitles_delay` | I | `0` | `sub-delay` | `(value / 1000.0).toString()` = `"0.0"` |
| `pref_subtitles_speed` | F | `1f` | `sub-speed` | `"1.0"` |
| `pref_subtitles_secondary_delay` | I | `0` | `secondary-sub-delay` | `(value / 1000.0)` = `"0.0"` |
| `pref_subtitle_font` | S | `"Sans Serif"` | `sub-font` | `Sans Serif` |
| `pref_override_subtitles_ass` | B | `false` | `sub-ass-override` = `force` and `sub-ass-justify` = `yes` | not set by default |
| `pref_subtitles_font_size` | I | `55` | `sub-font-size` | `"55"` |
| `pref_bold_subtitles` | B | `false` | `sub-bold` | `"no"` |
| `pref_italic_subtitles` | B | `false` | `sub-italic` | `"no"` |
| `pref_sub_justify` | E `SubtitleJustification` | `Auto` | `sub-justify` | `auto` |
| `pref_text_color_subtitles` | I (ARGB) | `Color.White.toArgb()` = `-1` | `sub-color` | `#FFFFFFFF` |
| `pref_background_color_subtitles` | I (ARGB) | `Color.Transparent.toArgb()` = `0` | `sub-back-color` | `#00000000` |
| `pref_border_color_subtitles` | I (ARGB) | `Color.Black.toArgb()` = `-16777216` | `sub-border-color` | `#FF000000` |
| `pref_sub_border_size` | I | `3` | `sub-border-size` | `"3"` |
| `pref_border_style_subtitles` | E `SubtitlesBorderStyle` | `OutlineAndShadow` | `sub-border-style` | `outline-and-shadow` |
| `sub_shadow_offset` | I | `0` | `sub-shadow-offset` | `"0"` |
| `pref_sub_pos` | I | `100` | `sub-pos` | `"100"` |
| `pref_sub_scale` | F | `1f` | `sub-scale` | `"1.0"` |
| `pref_subtitle_lang` | S | `""` | not an option here (part 2 track selection) | |
| `pref_subtitle_whitelist` | S | `""` | part 2 | |
| `pref_subtitle_blacklist` | S | `""` | part 2 | |
| `pref_screenshot_subtitles` | B | `false` | part 2 (screenshot) | |

Color string format: `Int.toColorHexString()` = `"#" + this.toHexString().uppercase()` (`ui/player/controls/components/panels/SubtitleSettingsColorsCard.kt:156`). Kotlin `Int.toHexString()` gives 8 lowercase hex digits of the two's complement value, so `-1` gives `ffffffff`.

Fixed options (not preference-driven): `sub-ass-force-margins` = `yes`, `sub-use-margins` = `yes` (`PlayerActivity.kt:438-439`), `sub-fonts-dir` and `osd-fonts-dir` (set at runtime, `PlayerActivity.kt:530-531`).

### 3.4 Audio preferences and mpv mapping (`AudioPreferences.kt`, `AniyomiMPVView.kt:254-259`, `PlayerActivity.kt:586`)

| Key | Type | Default | mpv | Value (default) |
|---|---|---|---|---|
| `pref_audio_lang` | S | `""` | `alang` (option) | `""` |
| `pref_audio_pitch_correction` | B | `true` | `audio-pitch-correction` | `"true"` |
| `pref_audio_config` | E `AudioChannels` | `AutoSafe` | `audio-channels` (property) | `auto` |
| `pref_audio_volume_boost_cap` | I | `30` | `volume-max` = value + 100 | `"130"` |
| `pref_audio_delay` | I (ms) | `0` | `audio-delay` = ms / 1000.0 | `"0.0"` |

### 3.5 Decoder and video preferences and mpv mapping (`DecoderPreferences.kt`, `AniyomiMPVView.kt:114-152`)

| Key | Type | Default | mpv effect |
|---|---|---|---|
| `pref_try_hwdec` | B | `true` | `hwdec` = `auto` (true) or `no` (false) |
| `pref_gpu_next` | B | `false` | `vo` = `gpu-next` (true) or `gpu` (false) |
| `pref_video_debanding` | E `Debanding` | `None` | `vf=gradfun=radius=12` (CPU) or `deband=yes` (GPU) |
| `use_yuv420p` | B | `true` | `vf=format=yuv420p` (conflicts with CPU debanding, see behavior 2.2) |
| `pref_player_filter_brightness` | I | `0` | `brightness` option |
| `pref_player_filter_saturation` | I | `0` | `saturation` option |
| `pref_player_filter_contrast` | I | `0` | `contrast` option |
| `pref_player_filter_gamma` | I | `0` | `gamma` option |
| `pref_player_filter_hue` | I | `0` | `hue` option |
| `pref_mpv_conf` | S | `""` | written to `mpv.conf` |
| `pref_mpv_input` | S | `""` | written to `input.conf` |
| `mpv_scripts` | B | `false` | copy user scripts, script-opts and shaders |

### 3.6 Network and base preferences

| Key | Type | Default | Read by |
|---|---|---|---|
| `verbose_logging` | B | `false` (constructor default in `NetworkPreferences`) | `AniyomiMPVView.kt:128`, `PlayerActivity.kt:425`, `TorrentServerService.kt:66` |
| `__APP_STATE_incognito_mode` | B | `false` (`Preference.appStateKey("incognito_mode")`, prefix `__APP_STATE_`, `Preference.kt:43-47`, `BasePreferences.kt:21`) | `ExternalIntents.kt:442, 463` |
| `pref_auto_update_manga_sync_key` | B | `true` (`TrackPreferences.kt:39`) | `ExternalIntents.kt:479, 525` |
| `remove_after_read_slots` | I | `-1` (`DownloadPreferences.kt:28`) | `ExternalIntents.kt:508` |

### 3.7 Torrent preferences (`core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt`)

| Key | Type | Default | Notes |
|---|---|---|---|
| `pref_torrserver_enable` | B | `false` | gate for torrent routing (`PlayerActivity.kt:1093`) |
| `pref_torrserver_shownotice` | B | `false` | UI notice (part 2 / settings) |
| `pref_torrserver_port` | S (string) | `"8090"` | passed as `port` to `TorrServer.startServer` |
| `pref_torrserver_tackers` | S | default tracker list, section 5 | key spelling kept (typo) |
| `pref_torrserver_proxymode` | E `ProxyMode` | `None` | mapped to `value` string |
| `pref_torrserver_proxyurl` | S | `""` | passed only when mode is not None |

### 3.8 Advanced player preferences (`AdvancedPlayerPreferences.kt`)

| Key | Type | Default |
|---|---|---|
| `mpv_scripts` | B | `false` |
| `pref_mpv_conf` | S | `""` |
| `pref_mpv_input` | S | `""` |
| `pref_player_statistics_page` | I | `0` |

---

## 4. Golden default mpv option list (for test `p1-edge-cases-and-tests.md`)

With every preference at its default and a local file (no HTTP headers), the Kotlin issues these calls in this order. Desktop values for paths are placeholders in the golden file only (`<data>` = test temp dir).

Options (`setOptionString`), in order:

1. `vo` = `gpu` (via `setVo`)
2. `hwdec` = `auto`
3. `msg-level` = `all=warn`
4. `vf` = `format=yuv420p`
5. `ytdl` = `no`
6. `tls-verify` = `yes`
7. `tls-ca-file` = `<data>/mpv/cacert.pem`
8. `demuxer-max-bytes` = `67108864`
9. `demuxer-max-back-bytes` = `67108864`
10. `screenshot-directory` = `<Pictures>`
11. `brightness` = `0`
12. `saturation` = `0`
13. `contrast` = `0`
14. `gamma` = `0`
15. `hue` = `0`
16. `speed` = `1.0`
17. `vd-lavc-film-grain` = `cpu`
18. `sub-delay` = `0.0`
19. `sub-speed` = `1.0`
20. `secondary-sub-delay` = `0.0`
21. `sub-font` = `Sans Serif`
22. `sub-font-size` = `55`
23. `sub-bold` = `no`
24. `sub-italic` = `no`
25. `sub-justify` = `auto`
26. `sub-color` = `#FFFFFFFF`
27. `sub-back-color` = `#00000000`
28. `sub-border-color` = `#FF000000`
29. `sub-border-size` = `3`
30. `sub-border-style` = `outline-and-shadow`
31. `sub-shadow-offset` = `0`
32. `sub-pos` = `100`
33. `sub-scale` = `1.0`
34. `alang` = `` (empty)
35. `audio-delay` = `0.0`
36. `audio-pitch-correction` = `true`
37. `volume-max` = `130`
38. `sub-ass-force-margins` = `yes`
39. `sub-use-margins` = `yes`

Properties (`setPropertyBoolean` / `setPropertyString`), in order:

40. `pause` = `yes` (boolean, before options)
41. `keep-open` = `yes`
42. `input-default-bindings` = `yes`
43. `audio-channels` = `auto` (property, from `AutoSafe`)

Note: step 4 is the Kotlin value only when CPU debanding is off. With `pref_video_debanding = CPU` and `use_yuv420p = true`, the Rust port must send `vf` = `gradfun=radius=12,format=yuv420p` (the Kotlin sends only `format=yuv420p`, see behaviour doc 2.2).

Also not sent by default: `http-header-fields` (local file), `ass-override` (default off), `stats/*` script-bindings (page 0), `sub-fonts-dir`, `osd-fonts-dir` (sent at runtime after the font copy).

---

## 5. Default tracker list (`pref_torrserver_tackers`)

OBSERVED (`TorrentPreferences.kt:13-41`). The Kotlin literal is a raw string with newlines and indentation, then `.replace(" ", "")` removes spaces, leaving one tracker per line. The 26 entries, in order:

1. `http://nyaa.tracker.wf:7777/announce`
2. `http://anidex.moe:6969/announce`
3. `http://tracker.anirena.com:80/announce`
4. `udp://tracker.uw0.xyz:6969/announce`
5. `http://share.camoe.cn:8080/announce`
6. `http://t.nyaatracker.com:80/announce`
7. `udp://47.ip-51-68-199.eu:6969/announce`
8. `udp://9.rarbg.me:2940`
9. `udp://9.rarbg.to:2820`
10. `udp://exodus.desync.com:6969/announce`
11. `udp://explodie.org:6969/announce`
12. `udp://ipv4.tracker.harry.lu:80/announce`
13. `udp://open.stealth.si:80/announce`
14. `udp://opentor.org:2710/announce`
15. `udp://opentracker.i2p.rocks:6969/announce`
16. `udp://retracker.lanta-net.ru:2710/announce`
17. `udp://tracker.cyberia.is:6969/announce`
18. `udp://tracker.dler.org:6969/announce`
19. `udp://tracker.ds.is:6969/announce`
20. `udp://tracker.internetwarriors.net:1337`
21. `udp://tracker.openbittorrent.com:6969/announce`
22. `udp://tracker.opentrackr.org:1337/announce`
23. `udp://tracker.tiny-vps.com:6969/announce`
24. `udp://tracker.torrent.eu.org:451/announce`
25. `udp://valakas.rollo.dnsabr.com:2710/announce`
26. `udp://www.torrent.eu.org:451/announce`

Count: the source lines 15-40 hold 26 tracker entries. The task T-45a-076 text is corrected to 26.

Joined form pushed to the server: `TorrentServerUtils.kt:13-15` splits on `\n` and joins with `",\n"`.

---

## 6. Network endpoints

### 6.1 Local TorrServer HTTP API (base `http://127.0.0.1:<port>`, port from `TorrServer.startServer`, `TorrentServerApi.kt`)

| Method | Path | Request | Response | Caller |
|---|---|---|---|---|
| GET | `/echo` | none | body text; empty on error | `TorrentServerApi.echo` |
| POST | `/torrents` | `application/json`, body `TorrentRequest` with `action: "add"` | `Torrent` JSON | `addTorrent` |
| POST | `/torrent/upload` | multipart: `file` (filename = title, `application/x-bittorrent`), `save` (`"true"`/`"false"`), `title` | `Torrent` JSON | `uploadTorrent` |
| GET | `/stream/<urlencoded name>?link=<hash>&index=<n>&play` | none (played by mpv) | media stream | `TorrentServerUtils.getTorrentPlayLink` |

All requests go through `network.client` (OkHttp, the app's shared client, `NetworkHelper`), and `awaitSuccess()` throws on non-2xx.

### 6.2 TorrServer library calls (external `xyz.secozzi.torrserver:torrserver:0.1.1`)

Used signatures (from call sites):

- `TorrServer.startServer(port: String, path: String, proxyMode: String, proxyUrl: String): Int` returns the bound port or `-1`. Call: `TorrServer.startServer(port = torrServerPort().get(), path = filesDir.absolutePath, proxyMode = mode.value, proxyUrl = ...)` (`TorrentServerService.kt:71-76`).
- `TorrServer.stopServer()`.
- `TorrServer.addTrackers(String)` (comma-newline separated).
- `TorrServer.registerLogCallback()`.

Version: `gradle/aniyomi.versions.toml:9`, `torrserver = "0.1.1"`; module `io.github.secozzi:torrserver` (`gradle/aniyomi.versions.toml:20`). The library is Go-based (TorrServer); its source is not in this repository.

### 6.3 mpv network-related options

- `http-header-fields` (string, `Key: value,Key2: value2`, commas inside values escaped `\,`).
- `tls-verify` = `yes`, `tls-ca-file` = `<mpv dir>/cacert.pem`.
- `ytdl` = `no` (yt-dlp disabled).
- `demuxer-max-bytes`, `demuxer-max-back-bytes` = 64 MiB.

### 6.4 Local source HTTP server

`http://localhost:<port>` where port is the ephemeral port of the `HttpServer`/`LocalServer`. The rewrite rule is `localUrl` regex `http:\/\/localhost:1(?!\d)` (section 2.1). Proposal: bind `127.0.0.1` only (Android binds all interfaces by NanoHTTPD default `hostname = null`; Linux port should not expose the source to the LAN).

### 6.5 External player launch (Linux proposal)

Command line only; no network. Preset commands are in `p1-topic-external-players.md`.

---

## 7. Database tables and columns touched

### 7.1 `episodes` (`data/src/main/sqldelightanime/dataanime/episodes.sq:3-26`)

Columns: `_id INTEGER NOT NULL PRIMARY KEY`, `anime_id INTEGER NOT NULL` (FK animes), `url TEXT NOT NULL`, `name TEXT NOT NULL`, `scanlator TEXT`, `seen INTEGER AS Boolean NOT NULL`, `bookmark INTEGER AS Boolean NOT NULL`, `last_second_seen INTEGER NOT NULL` (ms), `total_seconds INTEGER NOT NULL` (ms), `episode_number REAL NOT NULL`, `source_order INTEGER NOT NULL`, `date_fetch INTEGER NOT NULL`, `date_upload INTEGER NOT NULL`, `last_modified_at INTEGER NOT NULL DEFAULT 0`, `version INTEGER NOT NULL DEFAULT 0`, `is_syncing INTEGER NOT NULL DEFAULT 0`, `summary TEXT`, `preview_url TEXT`, `fillermark INTEGER AS Boolean NOT NULL`, `memo BLOB AS JsonObject NOT NULL DEFAULT '{}'`.

Writes in scope: `UpdateEpisode` via `EpisodeUpdate` (`seen`, `bookmark`, `fillermark`, `last_second_seen`, `total_seconds`), issued from `ExternalIntents.saveEpisodeProgress`. The SQL `update` statement compares `last_second_seen` (`episodes.sq:45`), which is the trigger for last-modified bookkeeping (part 04).

### 7.2 `animehistory` (`data/src/main/sqldelightanime/dataanime/animehistory.sq:3-9`)

`_id INTEGER NOT NULL PRIMARY KEY`, `episode_id INTEGER NOT NULL UNIQUE` (FK episodes ON DELETE CASCADE), `last_seen INTEGER AS Date`. Index `animehistory_history_episode_id_index`.

Upsert (`animehistory.sq:59-65`): `INSERT INTO animehistory(episode_id, last_seen) VALUES (:episodeId, :seenAt) ON CONFLICT(episode_id) DO UPDATE SET last_seen = :seenAt WHERE episode_id = :episodeId`. Called by `saveEpisodeHistory` with `Date()`.

### 7.3 `animes` (`animes.sq:8-35`)

`episode_flags INTEGER NOT NULL` (sorting bits, section 2.7). Read by `anime.sorting` in `deleteEpisodeIfNeeded`.

### 7.4 `custom_buttons` (section 2.8).

### 7.5 Tables read but not written here

- Tracking rows (`anime_sync` table `anime_sync.sq:3`), through `GetAnimeTracks`/`InsertAnimeTrack` (area 27).
- Delayed tracking store: `DelayedAnimeTrackingStore` (area 27).

---

## 8. Keys and identifiers in the preference store that look like constants

- `__APP_STATE_` prefix for app-state keys (`Preference.kt:47`): `incognito_mode` becomes `__APP_STATE_incognito_mode`.
- Intent extra names used by player navigation (`PlayerActivity.kt:167-171`): `animeId`, `episodeId`, `hostIndex`, `vidIndex`, `hostList`.
- Intent extras for external players (`ExternalIntents.kt`): `title`, `position`, `return_result`, `secure_uri`, `headers`, `http-header-fields`, `subs`, `subs.name`, `subs.enable`, `subtitles_location`, `subtitle`, `android.media.intent.extra.HTTP_HEADERS`, `end_by`, `duration`, `extra_position`, `extra_duration`.
- Pip broadcast: action `pip_control`, extra `media_control`, codes 1-5 (`PipActions.kt:397-403`).
- MediaSession tag `"PlayerActivity"` (`PlayerActivity.kt:891`).

---

## 9. Message keys used in scope (with English text)

| Key (namespace) | English text | Used in |
|---|---|---|
| `AYMR.strings.http_server_is_running` | `Http server is running` | HttpServerService notification |
| `AYMR.strings.http_server_start_failure` | `Failed to start http server` | PlayerActivity, ExternalIntents, MainActivity |
| `AYMR.strings.torrentserver_is_running` | `Torrent Server is running` | TorrentServerService notification |
| `MR.strings.action_close` | `Close` | HttpServer notification action (see behaviour doc section 18) |
| `MR.strings.app_name` | app name | Torrent notification title |
| `AYMR.strings.action_previous_episode` | `Previous episode` | PIP action label |
| `AYMR.strings.pref_skip_10` | `10s` | PIP skip label (not translatable) |
| `AYMR.strings.action_play` | `Play` | PIP play label |
| `MR.strings.action_pause` | `Pause` | PIP pause label |
| `AYMR.strings.action_next_episode` | `Next episode` | PIP next label |
| `AYMR.strings.no_next_episode` | `Next Episode not found!` | changeEpisode |
| `AYMR.strings.no_hosters` | `Hoster list is empty` | changeEpisode, initial error |
| `AYMR.strings.share_screenshot_info` | `%1$s: %2$s, %3$s` | share message |
| `AYMR.strings.mpv_media_title` | `%1$s - E%2$s - %3$s` | force-media-title |
| `MR.strings.action_share` | `Share` | share chooser |
| `MR.strings.picture_saved` | `Picture saved` | screenshot save toast |
| `MR.strings.cover_updated` | `Cover updated` | set-as-art |
| `AYMR.strings.background_updated` | `Background updated` | set-as-art |
| `AYMR.strings.thumbnail_updated` | `Thumbnail updated` | set-as-art |
| `MR.strings.notification_first_add_to_library` | `Please add the entry to your library before doing this` | set-as-art |
| `MR.strings.notification_cover_update_failed` | `Failed to update cover` | set-as-art |
| `AYMR.strings.rotation_video` | `Video` | orientation label (dropped) |
| `AYMR.strings.rotation_sensor_portrait` | `Sensor portrait` | orientation label (dropped) |
| `AYMR.strings.rotation_reverse_landscape` | `Reverse landscape` | orientation label (dropped) |
| `AYMR.strings.rotation_sensor_landscape` | `Sensor landscape` | orientation label (dropped) |
| `MR.strings.rotation_free` | `Free` | orientation label (dropped) |
| `MR.strings.rotation_portrait` | `Portrait` | orientation label (dropped) |
| `MR.strings.rotation_landscape` | `Landscape` | orientation label (dropped) |
| `AYMR.strings.video_crop_screen` | `Cropped to screen` | aspect label |
| `AYMR.strings.video_fit_screen` | `Fit to screen` | aspect label |
| `AYMR.strings.video_stretch_screen` | `Stretched to screen` | aspect label |
| `AYMR.strings.single_action_none` | `None` | gesture label |
| `AYMR.strings.single_action_seek` | `Seek` | gesture label |
| `AYMR.strings.single_action_playpause` | `Play/Pause` | gesture label |
| `AYMR.strings.single_action_switch` | `Change episode` | gesture label |
| `AYMR.strings.single_action_custom` | `Custom` | gesture label |
| `AYMR.strings.player_sheets_filters_brightness` | (filter label, part 2) | VideoFilters.titleRes |
| `AYMR.strings.pref_player_audio_channels_*` | (channel labels, part 2) | AudioChannels.titleRes |

Literal strings not in the resource catalogue (kept as Kotlin literals): `"Video URL is empty."`, `"Cannot open episode"` (ExternalIntents), `"Error: File ended"`, `"Hoster list is empty"`, `"Unknown error"`, `"Stop"` (torrent notification action, defect), `"PlayerActivity"` (media session tag), `"Error getting links"` (log only).

---

## 10. Interaction with `Video` and transport across activities

- `PlayerActivity.newIntent` passes host list as serialised string (`hostList`), host and video indexes as ints.
- `ExternalIntents.newIntent` receives the `Video` object directly (no serialisation).
- `MainActivity.startPlayerActivity` passes `Video?` by reference from the caller.
- Desktop (Proposal): a `PlayerRequest` struct and `Arc<Mutex<PlayerState>>` replace intent extras; the host list is typed (`Vec<Hoster>`), not serialised.
