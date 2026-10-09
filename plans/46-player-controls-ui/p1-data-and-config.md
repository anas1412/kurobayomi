# 46 Player controls UI, part 1: data models, preferences, files, endpoints

All paths are repo-relative to `/home/blackbox/aniyomi-src`. Defaults are quoted from the Kotlin. Preference keys are read through `tachiyomi.core.common.preference.PreferenceStore`.

---

## 1. Enumerations and their stored values

### 1.1 `Sheets` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt:99-108`)

Ordinal order, not persisted (in-memory `MutableStateFlow`, VM:299):

| Ordinal | Name | Opened from |
|---|---|---|
| 0 | `None` | close |
| 1 | `PlaybackSpeed` | speed long-press (BottomLeft), `PlayerControls` |
| 2 | `SubtitleTracks` | top-right subtitles click |
| 3 | `AudioTracks` | top-right audio click |
| 4 | `QualityTracks` | top-right quality click or long-press |
| 5 | `Chapters` | current-chapter pill click |
| 6 | `More` | top-right more click |
| 7 | `Screenshot` | long press on the video surface (GH:176) |

### 1.2 `Panels` (PlayerEnums.kt:110-116)

| Ordinal | Name | Opened from |
|---|---|---|
| 0 | `None` | close |
| 1 | `SubtitleSettings` | subtitles long-press (TopRight), sub-settings sheet button |
| 2 | `SubtitleDelay` | subtitle tracks sheet button |
| 3 | `AudioDelay` | audio long-press (TopRight), audio sheet button |
| 4 | `VideoFilters` | more long-press (TopRight), more sheet button |

### 1.3 `Dialogs` (PlayerEnums.kt:118-131), sealed

- `None` (data object).
- `EpisodeList` (data object), opened by title click (VM:1209).
- `IntegerPicker(defaultValue: Int, minValue: Int, maxValue: Int, step: Int, nameFormat: String, title: String, onChange: (Int) -> Unit, onDismissRequest: () -> Unit)`, opened by Lua message `launch_int_picker` (VM:904-919).

### 1.4 `PlayerUpdates` (PlayerEnums.kt:133-139), sealed

- `None`, `DoubleSpeed` (no UI), `AspectRatio`, `ShowText(value: String)`, `ShowTextResource(textResource: StringResource)`.

### 1.5 `VideoAspect` (PlayerEnums.kt:52-56)

| Name | `titleRes` | Stored as (`getEnum` serializer = `it.name`, PreferenceStore.kt:34) |
|---|---|---|
| `Crop` | `AYMR.strings.video_crop_screen` ("Cropped to screen") | `"Crop"` |
| `Fit` | `AYMR.strings.video_fit_screen` ("Fit to screen") | `"Fit"` |
| `Stretch` | `AYMR.strings.video_stretch_screen` ("Stretched to screen") | `"Stretch"` |

Preference `pref_player_aspect_state`, default `VideoAspect.Fit` (PlayerPreferences.kt:81). Cycle order in UI: Fit → Stretch → Crop → Fit (PC:512-520).

### 1.6 `SingleActionGesture` (PlayerEnums.kt:61-67)

| Name | `stringRes` | Stored as |
|---|---|---|
| `None` | `single_action_none` ("None") | `"None"` |
| `Seek` | `single_action_seek` ("Seek") | `"Seek"` |
| `PlayPause` | `single_action_playpause` ("Play/Pause") | `"PlayPause"` |
| `Switch` | `single_action_switch` ("Change episode") | `"Switch"` |
| `Custom` | `single_action_custom` ("Custom") | `"Custom"` |

### 1.7 `CustomKeyCodes` (PlayerEnums.kt:72-79)

| Name | `keyCode` (string sent to mpv `keypress`) |
|---|---|
| `DoubleTapLeft` | `"0x10001"` |
| `DoubleTapCenter` | `"0x10002"` |
| `DoubleTapRight` | `"0x10003"` |
| `MediaPrevious` | `"0x10004"` |
| `MediaPlay` | `"0x10005"` |
| `MediaNext` | `"0x10006"` |

Only the three double-tap codes are used in scope. The media codes are used by media-key handling (part 2 or area 45).

### 1.8 `ArtType` and `SetAsArt` (PlayerEnums.kt:29-39)

`SetAsArt { Success, AddToLibraryFirst, Error }`; `ArtType { Cover, Background, Thumbnail }`. Used by the screenshot sheet (part 2), listed here for completeness of the enum set.

### 1.9 `PlayerOrientation` (PlayerEnums.kt:41-50)

`Free(MR.strings.rotation_free)`, `Video(AYMR rotation_video)`, `Portrait(MR rotation_portrait)`, `ReversePortrait(MR rotation_reverse_portrait)`, `SensorPortrait(AYMR rotation_sensor_portrait)`, `Landscape(MR rotation_landscape)`, `ReverseLandscape(AYMR rotation_reverse_landscape)`, `SensorLandscape(AYMR rotation_sensor_landscape)`. Stored as name. Default for `pref_default_player_orientation_type_key` is `SensorLandscape` (PlayerPreferences.kt:16-19). Linux: see T-46a-048.

### 1.10 `Decoder` (PlayerEnums.kt:81-91) — listed because the More sheet (part 2) uses the same enum

| Name | `title` | `value` (mpv `hwdec`) |
|---|---|---|
| `AutoCopy` | "Auto" | `auto-copy` |
| `Auto` | "Auto" | `auto` |
| `SW` | "SW" | `no` |
| `HW` | "HW" | `mediacodec-copy` |
| `HWPlus` | "HW+" | `mediacodec` |

Linux note: `mediacodec*` values are Android-only. Kurobayomi proposal: map to `vaapi-copy`/`nvdec-copy` per detected GPU (area 45). Out of part 1 scope.

### 1.11 `Debanding` (PlayerEnums.kt:93-97): `None`, `CPU`, `GPU` (not used in scope).

### 1.12 `VideoFilters` (PlayerEnums.kt:141-171): `BRIGHTNESS`, `SATURATION`, `CONTRAST`, `GAMMA`, `HUE`, each with a title resource, a preference accessor on `DecoderPreferences`, and an mpv property name (`brightness`, `saturation`, `contrast`, `gamma`, `hue`). Preference keys: `pref_player_filter_brightness`, `pref_player_filter_saturation`, `pref_player_filter_contrast`, `pref_player_filter_gamma`, `pref_player_filter_hue` (DecoderPreferences.kt:17-21), no default given (`getInt(key)` without default: 0 by PreferenceStore Int default). Used by the panel in part 2; listed here because `More` long-press targets the panel.

### 1.13 `ChapterType` (source-api `animesource/model/Video.kt`)

`@Serializable enum class ChapterType { Opening, Ending, Recap, MixedOp, Other }`. JSON form is the enum name. `TimeStamp(start: Double, end: Double, name: String, type: ChapterType = Other)`.

Label map (`app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/ChapterUtils.kt:13-19`): Opening → `player_chapter_type_opening`, Ending → `player_chapter_type_ending`, Recap → `player_chapter_type_recap`, MixedOp → `player_chapter_type_mixedop`, Other → null.

### 1.14 `Anime` display-mode bit field (`domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:245-262`)

| Constant | Value (hex) | Bit meaning |
|---|---|---|
| `EPISODE_SHOW_PREVIEWS` | `0x00000000` | previews on |
| `EPISODE_SHOW_NOT_PREVIEWS` | `0x00000800` | previews off |
| `EPISODE_PREVIEWS_MASK` | `0x00000800` | |
| `EPISODE_SHOW_SUMMARIES` | `0x00000000` | |
| `EPISODE_SHOW_NOT_SUMMARIES` | `0x00001000` | |
| `EPISODE_SUMMARIES_MASK` | `0x00001000` | |
| `EPISODE_DISPLAY_NAME` | `0x00000000` | show `name` |
| `EPISODE_DISPLAY_NUMBER` | `0x00100000` | show "Episode N" |
| `EPISODE_DISPLAY_MASK` | `0x00100000` | bit used for the name/number choice |
| `SEASON_SORT_DESC` | `0x00000000` | |
| `SEASON_SORT_ASC` | `0x00000001` | |
| `SEASON_SORT_DIR_MASK` | `0x00000001` | |
| `SEASON_SHOW_DOWNLOADED` | `0x00000002` | |
| `SEASON_SHOW_NOT_DOWNLOADED` | `0x00000004` | |

Use `(displayMode and EPISODE_DISPLAY_MASK) == EPISODE_DISPLAY_NUMBER` (E-46-033). Kotlin source compares the whole value (EpisodeListDialog.kt:87).

---

## 2. Domain models touched

### 2.1 `Episode` (`domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt:6-28`)

Fields used by the episode list dialog and the player:

| Field | Type | Nullable | Use in scope |
|---|---|---|---|
| `id` | Long | no | row key, bookmark/fillermark/seen calls |
| `animeId` | Long | no | not used in scope |
| `seen` | Boolean | no | dims row text (alpha 0.38) |
| `bookmark` | Boolean | no | bookmark icon state |
| `fillermark` | Boolean | no | fillermark icon state |
| `lastSecondSeen` | Long | no | resume position (area 45) |
| `totalSeconds` | Long | no | (area 45) |
| `dateFetch` | Long | no | not used |
| `sourceOrder` | Long | no | (area 45 playlist ordering) |
| `url` | String | no | (area 45) |
| `name` | String | no | row title when name mode |
| `dateUpload` | Long (epoch millis) | no | date label when > 0 |
| `episodeNumber` | Double | no | "Episode N" label through `formatEpisodeNumber`; `isRecognizedNumber` = `episodeNumber >= 0f` |
| `scanlator` | String? | yes | subtitle row |
| `summary` | String? | yes | not used |
| `previewUrl` | String? | yes | not used in scope |
| `lastModifiedAt`, `version`, `memo` | | | not used |

### 2.2 `IndexedSegment` (SeekBar.kt:53-66), `@Immutable data class`

| Field | Type | Default |
|---|---|---|
| `name` | String | required |
| `start` | Float (seconds) | required |
| `color` | `Color` | `Color.Unspecified` |
| `index` | Int | 0 |
| `chapterType` | `ChapterType` | `ChapterType.Other` |

`companion val Unspecified = IndexedSegment(name = "", start = 0f)`; `fun toSegment(): Segment = Segment(name, start, color)`.

Seeker `Segment(name: String, start: Float, color: Color)` is an external type from `io.github.2307vivek:seeker:1.2.2`.

### 2.3 `Video` / `TimeStamp` (source-api)

`TimeStamp(start: Double, end: Double, name: String, type: ChapterType)` supplies chapter names and skip-intro types (area 16). Converted to `IndexedSegment` by `ChapterUtils.mergeChapters` (`ChapterUtils.kt:21-`).

### 2.4 `CustomButton` (`tachiyomi.domain.custombuttons.model.CustomButton`)

Fields used: `id: Long` (script message name), `name: String` (title). Execute sends `script-message call_button_<id>` (VM:2167-2172). Storage and editing are part 2 and area 45.

### 2.5 Gesture state (in-memory only)

| Name | Type | Initial | Writers |
|---|---|---|---|
| `gestureSeekAmount` | `Pair<Int, Int>?` (`first` = start second, `second` = delta seconds) | null | GH:205-211, 197 |
| `doubleTapSeekAmount` | Int (seconds, signed) | 0 | VM:994-1000, 1002-1015, 1051-1095 |
| `seekText` | `String?` | null | VM:998 and handlers |
| `isSeekingForwards` | Boolean | false | VM |
| `isDoubleTapSeeking` | Boolean (GH local `remember`) | false | GH:94, 132, 136, 159 |
| `isLongPressing` | Boolean (GH local) | false | GH:110, 164, 174 |
| `startingY`, `mpvVolumeStartingY` | Float (GH local, per drag) | 0 | GH:220-221, 240-290 |
| `wasPlayerAlreadyPause` | Boolean (GH local per drag) | | GH:185, 190, 199 |
| `interactionSource` | `MutableInteractionSource` | new | GH (ripple), PC:105 |

### 2.6 Thumbnail model

- `thumbnailInfo: ThumbnailInfo?` (VM, area 45) with `tileInfo: List<TileInfo>` and `imageTileUrls: List<String>`.
- `TileInfo { timeMs: Long, imageIndex: Int, x: Int, y: Int, width: Int, height: Int }` (fields used by `updateSeekPos` and `createThumbnail`, VM:575-609).
- `thumbnailTileCache: LinkedHashMap<Int, Bitmap>` with `removeEldestEntry = size > 3` (VM:257-262).
- `thumbnailImage: MutableStateFlow<ImageBitmap?>` (VM).

---

## 3. Preference keys used in scope

Format: key | type | default | source file:line | owner.

### 3.1 `PlayerPreferences` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt`)

| Key | Type | Default | Line | Read in |
|---|---|---|---|---|
| `pref_allow_gestures_in_panels` | Boolean | false | 23 | GH:87, 142 |
| `pref_show_loading` | Boolean | true | 24 | PC:350 (`showLoadingCircle`) |
| `pref_show_current_chapter` | Boolean | true | 25 | BottomLeft:76 |
| `pref_player_time_to_disappear` | Int (ms) | 4000 | 43 | PC:124 |
| `pref_panel_opacity` | Int (percent) | 60 | 44 | PlayerPanels.kt:87 |
| `player_hide_controls` | Boolean | false | 39 | VM:276, 278 |
| `pref_display_vol_as_per` | Boolean | true | 40 | PC:281 |
| `pref_show_system_status_bar` | Boolean | false | 41 | VM:675 |
| `pref_reduce_motion` | Boolean | false | 42 | PC:195 |
| `pref_enable_skip_intro` | Boolean | true | 48 | VM:2058 |
| `pref_enable_auto_skip_ani_skip` | Boolean | false | 49 | VM:2060 |
| `pref_enable_netflixStyle_aniskip` | Boolean | false | 50-52 | VM:2058 |
| `pref_waiting_time_aniskip` | Int (s) | 5 | 54 | VM:2059 |
| `pref_enable_pip` | Boolean | true | 60 | PlayerActivity:139-142 |
| `pref_player_speed` | Float | 1f | 75 | VM:211, BottomLeft:71 |
| `default_speed_presets` | StringSet | `0.25,0.5,0.75,1.0,1.25,1.5,1.75,2.0,2.5,3.0,3.5,4.0` | 76-79 | part 2 (speed sheet) |
| `invert_duration` | Boolean | false | 80 | PC:387 |
| `pref_player_aspect_state` | enum `VideoAspect` | `Fit` | 81 | PC:292, 514 |
| `pref_auto_play_enabled` | Boolean | false | 85 | PC:444 |
| `pref_default_player_orientation_type_key` | enum `PlayerOrientation` | `SensorLandscape` | 16-19 | VM:839-854 (drop on Linux) |
| `pref_show_failed_hosters` | Boolean | false | 33 | PC:587 (part 2) |
| `pref_show_empty_hosters` | Boolean | false | 34 | PC:588 (part 2) |
| `pref_remember_brightness` | Boolean | false | 26 | part 2/area 45 (not read in scope files) |
| `pref_remember_volume` | Boolean | false | 28 | part 2/area 45 |

### 3.2 `GesturePreferences` (`.../settings/GesturePreferences.kt`)

| Key | Type | Default | Line | Read in |
|---|---|---|---|---|
| `pref_gesture_volume_brightness` | Boolean | true | 11-14 | GH:105, 219 |
| `pref_swap_volume_and_brightness` | Boolean | false | 15 | GH:106, PC:194 |
| `pref_gesture_horizontal_seek` | Boolean | true | 19 | GH:107, 182 |
| `pref_show_seekbar` | Boolean | false | 20 | GH:109, 215; VM:976 |
| `pref_default_intro_length` | Int (s) | 85 | 21 | part 2 (intro length dialog) |
| `pref_skip_length_preference` | Int (s) | 10 | 22 | VM:974 (double-tap seek step) |
| `pref_player_smooth_seek` | Boolean | false | 23 | GH:108, 389; VM:975 |
| `pref_left_double_tap` | enum `SingleActionGesture` | `Seek` | 27 | VM:1052 |
| `pref_center_double_tap` | enum `SingleActionGesture` | `PlayPause` | 28 | VM:1067 |
| `pref_right_double_tap` | enum `SingleActionGesture` | `Seek` | 29 | VM:1082 |
| `pref_media_previous` | enum | `Switch` | 33 | media keys (area 45) |
| `pref_media_playpause` | enum | `PlayPause` | 34 | media keys |
| `pref_media_next` | enum | `Switch` | 35 | media keys |

### 3.3 `AudioPreferences` (`.../settings/AudioPreferences.kt`)

| Key | Type | Default | Line | Read in |
|---|---|---|---|---|
| `pref_audio_lang` | String | `""` | 11 | area 45 |
| `pref_audio_pitch_correction` | Boolean | true | 12 | area 45 |
| `pref_audio_config` | enum `AudioChannels` | `AutoSafe` | 13 | area 45 |
| `pref_audio_volume_boost_cap` | Int (percent above 100) | 30 | 14 | PC:286, GH:114, VM:291; mpv `volume-max` = cap + 100 (AniyomiMPVView.kt:258) |
| `pref_audio_delay` | Int (ms) | 0 | 18 | AudioDelayPanel:71 (written on Apply) |

### 3.4 `SubtitlePreferences` (`.../settings/SubtitlePreferences.kt`)

| Key | Type | Default | Line | Read in |
|---|---|---|---|---|
| `pref_screenshot_subtitles` | Boolean | false | 24 | PC:585, 627 |
| `pref_subtitles_delay` | Int (ms) | 0 | 51 | part 2 (sub delay) |
| `pref_subtitles_secondary_delay` | Int (ms) | 0 | 53 | part 2 |
| `pref_subtitles_speed` | Float | 1f | 52 (SubtitlePreferences.kt:52) | SubtitleDelayPanel.kt:132 (part 2) |

### 3.5 `DecoderPreferences` (`.../settings/DecoderPreferences.kt`)

| Key | Type | Default | Line |
|---|---|---|---|
| `pref_player_filter_brightness` | Int | 0 (PreferenceStore `getInt` default, PreferenceStore.kt:9) | 17 |
| `pref_player_filter_saturation` | Int | 0 | 18 |
| `pref_player_filter_contrast` | Int | 0 | 19 |
| `pref_player_filter_gamma` | Int | 0 | 20 |
| `pref_player_filter_hue` | Int | 0 | 21 |

### 3.6 Values written by scope code

| Key | Written by | When |
|---|---|---|
| `pref_player_speed` | BottomLeftPlayerControls.kt:71 | speed cycle click |
| `pref_auto_play_enabled` | VM:802-810 | auto-play toggle |
| `pref_player_aspect_state` | VM:836 | aspect cycle |
| `pref_default_player_orientation_type_key` | VM:839-854 | rotate (Linux drop) |
| `invert_duration` | PC:407 | duration timer click |
| `pref_audio_delay` | AudioDelayPanel.kt:71 | Apply button |
| `pref_subtitles_delay`, `pref_subtitles_speed` | SubtitleDelayPanel.kt:131-132 (part 2) | Apply |

Storage: `PreferenceStore` (`tachiyomi.core.common.preference`). Enum values stored as `enum.name` (PreferenceStore.kt:34). Unknown names fall back to the default (deserializer try/catch, PreferenceStore.kt:35-40). Kurobayomi: store as TOML/SQLite with the same keys; keep the names so that the Aniyomi backup option mapping (area 21/22) stays aligned where applicable.

---

## 4. mpv properties and commands used in scope

### 4.1 Properties (get/set)

| Property | Type | Set in | Get in | Notes |
|---|---|---|---|---|
| `speed` | double | PC:552 (`setPropertyDouble("speed", it.toDouble())`), PC:620 (`it.toFixed(2).toDouble()`), GH:165 (restore) | VM:211 (via observer) | playback rate |
| `pause` | bool | VM:652-664 (`activity.player.paused`) | VM:269 | |
| `volume` | int (0..volume-max) | PC/VM `changeMPVVolumeTo` (VM:789-791) | VM:293 (`getPropertyInt`) | boost layer, 100 = unity |
| `volume-max` | int | AniyomiMPVView.kt:258 `(cap + 100)` | VM:~295 | |
| `audio-delay` | double seconds | AudioDelayPanel.kt:66 `delay / 1000.0` | AudioDelayPanel.kt:64 `getPropertyDouble * 1000` | |
| `sub-delay`, `secondary-sub-delay` | double seconds | SubtitleDelayPanel (part 2) | same | |
| `sub-speed` | double | SubtitleDelayPanel (part 2) | same | valid write range `0.1..1` (SubtitleDelayPanel.kt:95) |
| `panscan` | double 0..1 | VM:813-837 | | Crop = 1.0, Fit = 0.0, Stretch = 0.0 |
| `video-aspect-override` | double | VM:813-837 | | -1.0 for Crop and Fit; display ratio for Stretch |
| `hwdec` | string | VM:204 | VM:204 | part 2 decoder |
| `audio-delay`, `brightness`, `contrast`, `saturation`, `gamma`, `hue` | | part 2 | | video filter panel |

### 4.2 Commands

| Command | Used in | Purpose |
|---|---|---|
| `seek <sec> relative` / `relative+exact` | VM:983-992 `seekBy` | double-tap relative seek |
| `seek <sec> absolute` / `absolute+keyframes` | VM:747-750 `seekTo` | absolute seek (seekbar, drag end, chapters, skip) |
| `screenshot-to-file <path> video|subtitles` | VM:1830-1845 `takeScreenshot` | screenshot (part 2 sheet) |
| `keypress <code>` | VM:1054, 1070, 1084 | custom double-tap action |
| `script-message call_button_<id>` / `call_button_<id>_long` | VM:2167-2172 | custom button |
| `multiply volume 0.5` / `2` | PlayerActivity.kt:617-619 | volume keys (area 45) |

### 4.3 Shell-visible behaviour of `MPVLib.command` and `setPropertyDouble`

Synchronous from the UI thread in the Kotlin code. Kurobayomi: asynchronous command queue (T-46a-001 acceptance) because `libmpv` `mpv_command` blocks the caller.

---

## 5. Android platform APIs replaced

| Android API | Used in | Linux / Rust replacement |
|---|---|---|
| `Settings.System.SCREEN_BRIGHTNESS` (0..255) read, `normalize` to 0..1 | VM:286-291 | Read backlight `/sys/class/backlight/*/brightness` / `max_brightness` (or `brightnessctl`); if missing, start at 1.0 |
| `Window.attributes.screenBrightness` write (-1 means auto) | VM:752-759 | Hardware: `org.freedesktop.login1.Session.SetBrightness` or `brightnessctl`. Software veil always (section 8.4 of behaviour) |
| `AudioManager.getStreamVolume/SetStreamVolume(STREAM_MUSIC)`, `getStreamMaxVolume` | VM:292, 765, 779-787 | PipeWire default sink via `wpctl` or `libpulse-binding` (`pulse` crate); `maxVolume = 15` steps |
| `WindowInsetsControllerCompat.hide/show(statusBars)` | VM:666-682 | No-op (fullscreen handled by the window manager) |
| `activity.requestedOrientation` | VM:839-854 | Not applicable on desktop; button hidden |
| `enterPictureInPictureMode`, `PictureInPictureParams` | PC:507-511, PlayerActivity.kt:764-790 | Mini-player window (T-46a-046); no Android PiP |
| `HapticFeedback` | GH:173, SliderItem, TintedSliderItem, VerticalSliderItem | None (drop, T-46a-045) |
| `BackHandler` | PlayerSheet.kt:180-183 | GTK Escape shortcut and `close-request` |
| `ActivityResultContracts.OpenDocument` (`*/*`) | PlayerSheets.kt:104-132 | `gtk::FileDialog::open` with no filter (or subtitle/audio filter) |
| `Context.cacheDir` | VM:1830-1845 | `$XDG_CACHE_HOME/kurobayomi/player/` |
| `DisplayMetrics` (real width / height) | VM:820-824 | `gdk::Monitor::geometry()` of the surface's monitor |
| `ViewConfiguration` long-press timeout | GH (Compose) | `gtk::Settings::gtk-long-press-time` (GTK default 500 ms) |
| `WindowInsets.safeGestures` / `systemBars` | GH:120, PlayerSheet.kt:173-175 | None |
| `ContentResolver` / `Uri` | VM:510-545 subtitle/audio addition | `gio::File` |
| `Bitmap`, `ImageBitmap`, `asImageBitmap` | VM:610-613 | `gdk::Texture` (`gdk_memory_texture`), crop by `gtk::Snapshot` clip or `cairo` |
| `Toast` (`showToast`) | VM:1034-1048, 2080 | `adw::Toast` is not used (no libadwaita); use an in-window `gtk::Revealer` label or a `gtk::Overlay` pill (same as the mockup `pill()` in `src/player.rs`) |

---

## 6. On-disk files and folders

| Path (Android) | Written by | Lifecycle | Kurobayomi path |
|---|---|---|---|
| `<cacheDir>/<epochMillis>_mpv_screenshot_tmp.png` | VM:1830-1845 (`screenshot-to-file` target) | renamed immediately | `$XDG_CACHE_HOME/kurobayomi/player/tmp/<ms>_mpv_screenshot_tmp.png` |
| `<cacheDir>/mpv_screenshot.png` | VM:1837-1843 (`newFile`) | replaced on every screenshot | `$XDG_CACHE_HOME/kurobayomi/player/mpv_screenshot.png` |
| `context.cacheImageDir` | VM:1886-1912 `shareImage`: `destDir.deleteRecursively()` then save | deleted on each share | `$XDG_CACHE_HOME/kurobayomi/share/` |
| Pictures directory `Location.Pictures(DiskUtil.buildValidFilename(anime.title))` | VM:1847-1884 `saveImage` | persistent | `$XDG_PICTURES_DIR/Kurobayomi/<sanitised title>/` |
| Generated filename | VM:2013-2030 `generateFilename(anime, seconds)` (part 2 detail) | per save | `<sanitised title> - <HH:MM:SS>.png` (proposal; Kotlin uses `generateFilename`) |

Permissions: none beyond the app sandbox. Kurobayomi: no Android storage permissions exist (desktop).

---

## 7. Database

No table is written directly by the scope files. All persistence goes through the VM:

| Action | VM function | Storage |
|---|---|---|
| Bookmark toggle | `bookmarkEpisode(episodeId, bookmarked)` (VM:1805-1817) | episodes table column `bookmark` (area 04/07) |
| Fillermark toggle | `fillermarkEpisode(episodeId, fillermarked)` (VM:1819-1828) | episodes table column `fillermark` |
| Seen/progress | area 45 (`saveWatchingProgress`) | episodes table `seen`, `last_second_seen`, `total_seconds` |
| Auto-play | preference only | preference |

Kurobayomi: the episodes table already exists in the planned schema (area:04-db-anime-schema-migrations). This scope adds no tables. The bookmark and fillermark updates must run in a transaction together with the mutation of the in-memory `Episode` (the Kotlin mutates `episode.bookmark` directly before the DB write, EpisodeListDialog.kt:155-156, which is a race if the DB write fails; see E-46-035).

---

## 8. Network endpoints

No endpoint is called directly by scope files. Thumbnail tiles come from `AnimeHttpSource.getImageTile(url: String): Bitmap?` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:533`), which uses the source's HTTP client. The `url` values are `thumbInfo.imageTileUrls[index]` (VM:~587), produced by the source's trickplay/thumbnail metadata (area 16 and area 45).

Kurobayomi: the tile URL and the HTTP client come from the source API (area 16 and area 12-network-stack). The player UI only asks the source for a tile and gets a decoded image back.

---

## 9. Layout and style constants (presentation-core)

| Token | Value (dp) | Source |
|---|---|---|
| `padding.extraLarge` | 32 | `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt:14` |
| `padding.large` | 24 | Constants.kt:16 |
| `padding.medium` | 16 | Constants.kt:18 |
| `padding.mediumSmall` | 12 | Constants.kt:20 |
| `padding.small` | 8 | Constants.kt:22 |
| `padding.extraSmall` | 4 | Constants.kt:24 |
| `DISABLED_ALPHA` | 0.38 | Constants.kt:9 |
| `SECONDARY_ALPHA` | 0.78 | Constants.kt:10 |

Scope-local constants:

| Constant | Value | Source |
|---|---|---|
| `CARDS_MAX_WIDTH` | 420 dp | PlayerPanels.kt:81 |
| Control icon default size | 20 dp | ControlsButton.kt:55 |
| Transport icon size | 48 dp (prev/next), 96 dp (play) | MiddlePlayerControls.kt:80, 115 |
| Player update corner radius | 16 dp | PlayerUpdates.kt:42 |
| Player update background | black 40% | PlayerUpdates.kt:43 |
| Chapter pill corner | `RoundedCornerShape(25)` = 25% radius (Compose Int overload) | CurrentChapter.kt:60 |
| Chapter pill background | `colorScheme.background` alpha 0.6 | CurrentChapter.kt:61 |
| Thumbnail card | 220 x 124 dp (or 124*aspect wide), radius 8 dp, elevation 8 dp | ThumbnailPreview.kt:62-65, 112-113 |
| Thumbnail label | black 60%, radius 6 dp, 11 sp | ThumbnailPreview.kt:137-146 |
| Seekbar timer width | 92 dp (thumbnail assumes 96 with 4 dp spacing) | SeekBar.kt:106, 145; ThumbnailPreview.kt:87 |
| Seekbar row height | 48 dp | SeekBar.kt:95 |
| Vertical gauge box | height 120 dp, aspect 0.45, track width 50% | VerticalSliders.kt:105, 110 |
| Vertical slider (control) | 180 x 50 dp | SliderItem.kt:212-213 |
| Brightness/volume rounding | 16 dp radius track | VerticalSliders.kt:112 |
| Dialog width | 80% (episode list), 50% (integer picker) | EpisodeListDialog.kt:69, IntegerPickerDialog.kt:28 |
| Dialog padding | 16 dp, radius per `shapes.large` | PlayerDialog.kt:43-48 |
| Sheet max width | 420 dp portrait, 720 dp landscape | PlayerSheet.kt:87-91 |
| Sheet max height | 95% of screen height | PlayerSheet.kt:92 |
| Sheet corner | `extraLarge`, bottom corners 0 | PlayerSheet.kt:177 |
| Oval width | 40% (2/5) of the surface | GH:324 |
| Ripple alpha | 0.1 for all states | TachiyomiTheme.kt:95-98 (dropped) |

---

## 10. Colour roles used (Material 3 scheme)

| Role | Used by |
|---|---|
| `onSurface` | episode row default text, bookmark icon default |
| `primary` | bookmark tint, seekbar progress and thumb, auto-play on-track |
| `tertiary` | fillermark tint, volume fill, chapter time text |
| `secondaryContainer` | auto-play on-track |
| `errorContainer` | overflow (boost, negative brightness) segment |
| `background` | seekbar track, volume/brightness track, chapter pill (alpha 0.6) |
| `surface` / `surfaceDim` | panel card (alpha from preference) |
| `inversePrimary` | seekbar read-ahead |
| `primaryContainer` | default tint of `TintedSlider` |
| `onBackground` | chapter icon and name |
| `onSurfaceVariant` | subtitle delay type dropdown text (part 2 use) |

Kurobayomi proposal: define these as CSS custom properties in the GTK theme-following stylesheet (`@define-color`), derived from the active GTK theme colours (`window_bg_color`, `accent_color`, `error_bg_color`, `borders`). The user's theme is Tokyo Night (memory: match the current theme), so accent colours come from GTK named colours.

---

## 11. Strings and resources (keys only)

The full list with English text is in `p1-behavior.md` section 14. Resource files:
- `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (AYMR).
- `i18n-aniyomi/src/commonMain/moko-resources/base/plurals.xml` (AYMR plurals).
- `i18n/src/commonMain/moko-resources/base/strings.xml` (MR).

Kurobayomi proposal: gettext `.po` catalogue under `po/` with the same keys as msgid, English text as msgstr, and plural forms preserved (`ngettext` for `seconds`).

---

## 12. Lua-script message contract (used by PlayerViewModel and by mpv scripts)

Script messages handled in VM `handleLuaInvocation` (VM:856-960; part 2 for other messages). Data passed as `property` + `value` pairs. Relevant to scope:

| Property | Value | Effect |
|---|---|---|
| `launch_int_picker` | `title|nameFormat|start|stop|step|property` | opens `IntegerPickerDialog` (VM:904-919) |
| `pause` | `pause`, `unpause`, `pauseunpause` | VM:924-928 |
| `seek_to_with_text` | `seekValue|text` | VM:929-931 |
| `button` / `show_button` | part 2 custom button UI | VM:934- |
| `n`, `p` | next / previous episode | VM:~900 |

Kurobayomi proposal: keep the message names (`launch_int_picker`, `pause`, `seek_to_with_text`, `button`, `show_button`, `n`, `p`) so that existing Aniyomi-compatible mpv scripts keep working. Exact parse rule: `data.split("|")` with destructuring (6 fields, no escape).

---

## 13. Field-level validity rules (inputs)

| Input | Rule | Source |
|---|---|---|
| Audio delay (ms) | Int, no clamp (step 50, min Int.MIN_VALUE, max Int.MAX_VALUE) | AudioDelayPanel.kt, SubtitleDelayPanel.kt:35-37 (shared DelayCard) |
| Sub speed | `0.1..10f` on Apply; mpv write only when `0.1..1` | SubtitleDelayPanel.kt:95, 132 |
| Brightness | `-0.75..1` stored; window `0..1` | VM:752-759 |
| Volume device | `0..maxVolume` | VM:780 |
| Volume boost | `100..cap+100` (mpv) | GH:264 |
| Seek target | `0..duration` (drag, seekbar) | GH:196, PC:404 |
| Seek target absolute | `0..(player duration)` else ignored | VM:748 |
| Speed | cycle values 0.25..2.25 range by 0.25 wrap | BottomLeft:69 |
| Numeric chooser | rule-free in widget; caller decides | OutlinedNumericChooser.kt |

---

## 14. Summary of undeclared-in-Kotlin assumptions

- `Decoder` values for Linux are unknown until area 45 defines the detection step; the scope does not use them.
- `volume-max` is set only if mpv was initialised with the option (AniyomiMPVView.kt:258); Kurobayomi must set it before first playback.
- `maxVolume` is device-dependent on Android; Kurobayomi fixes it to 15 (T-46a-049).
