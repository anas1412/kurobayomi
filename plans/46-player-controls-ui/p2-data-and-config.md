# 46-player-controls-ui (part 2 of 2): data models, preferences, mpv properties, files, strings

Repo root for every path: `/home/blackbox/aniyomi-src`. All paths are repo-relative and written in full.
Scope files: the 14 files in `p2-inventory.md`. Behavior is in `p2-behavior.md`.

---

## 1. Enumerations (exact names, values, ordinals)

Ordinal = declaration order (Kotlin `entries` order). Stored form is noted per enum.

### 1.1 `Decoder` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt:81-87`)

| Ordinal | Name | `title` (literal, not localized) | `value` (mpv `hwdec`) | Shown in More sheet |
|---|---|---|---|---|
| 0 | `AutoCopy` | `Auto` | `auto-copy` | yes |
| 1 | `Auto` | `Auto` | `auto` | no (filtered out) |
| 2 | `SW` | `SW` | `no` | yes |
| 3 | `HW` | `HW` | `mediacodec-copy` | yes |
| 4 | `HWPlus` | `HW+` | `mediacodec` | yes |

- `getDecoderFromValue(value)` = `Decoder.entries.first { it.value == value }` (throws `NoSuchElementException` on unknown; `PlayerEnums.kt`).
- Linux values for the port (proposal): `AutoCopy` = `auto-copy`, `Auto` = `auto`, `SW` = `no`, `HW` = `vaapi-copy` or `nvdec-copy`, `HWPlus` = `vaapi` or `nvdec`. The final mapping is chosen at startup from the detected GPU (see p2-rust-design.md).

### 1.2 `AudioChannels` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt:21-27`)

Constructor: `(titleRes: StringResource, property: String, value: String)`.

| Ordinal | Name | titleRes key | English label | `property` | `value` |
|---|---|---|---|---|---|
| 0 | `Auto` | `pref_player_audio_channels_auto` | Auto | `audio-channels` | `auto-safe` |
| 1 | `AutoSafe` | `pref_player_audio_channels_auto_safe` | Auto-safe | `audio-channels` | `auto` |
| 2 | `Mono` | `pref_player_audio_channels_mono` | Mono | `audio-channels` | `mono` |
| 3 | `Stereo` | `pref_player_audio_channels_stereo` | Stereo | `audio-channels` | `stereo` |
| 4 | `ReverseStereo` | `pref_player_audio_channels_reverse_stereo` | Reverse stereo | `af` | `pan=[stereo|c0=c1|c1=c0]` |

- Stored form: enum **name** (`getEnum`, `PreferenceStore.kt:27-41`, serializer `it.name`; unknown name falls back to the default).
- **Name/value trap:** `Auto` writes mpv `auto-safe`; `AutoSafe` writes mpv `auto`. Keep labels and values in one mapping table in the port.
- Default: `AutoSafe`.

### 1.3 `SubtitleJustification` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/SubtitlePreferences.kt:56-61`)

| Ordinal | Name | `value` (mpv `sub-justify`) | Icon (Compose material icon) |
|---|---|---|---|
| 0 | `Left` | `left` | `Icons.AutoMirrored.Default.FormatAlignLeft` |
| 1 | `Center` | `center` | `Icons.Default.FormatAlignCenter` |
| 2 | `Right` | `right` | `Icons.AutoMirrored.Default.FormatAlignRight` |
| 3 | `Auto` | `auto` | `Icons.Default.FormatAlignJustify` |

- Stored form: enum name, key `pref_sub_justify`, default `Auto`.
- The typography toggles iterate `entries.minus(Auto)` (Left, Center, Right).

### 1.4 `SubtitlesBorderStyle` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt:368-375`)

| Ordinal | Name | `value` (mpv `sub-border-style`) | titleRes key | English |
|---|---|---|---|---|
| 0 | `OutlineAndShadow` | `outline-and-shadow` | `player_sheets_subtitles_border_style_outline_and_shadow` | Outline and shadow |
| 1 | `OpaqueBox` | `opaque-box` | `player_sheets_subtitles_border_style_opaque_box` | Opaque box |
| 2 | `BackgroundBox` | `background-box` | `player_sheets_subtitles_border_style_background_box` | Background box |

- Stored form: enum name, key `pref_border_style_subtitles`, default `OutlineAndShadow`.

### 1.5 `SubColorType` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsColorsCard.kt:158-178`)

| Ordinal | Name | titleRes key | mpv property | Preference getter | Pref key | Default int | Default meaning |
|---|---|---|---|---|---|---|---|
| 0 | `Text` | `player_sheets_subtitles_color_text` | `sub-color` | `textColorSubtitles` | `pref_text_color_subtitles` | `-1` | `0xFFFFFFFF` opaque white (`Color.White.toArgb()`) |
| 1 | `Border` | `player_sheets_subtitles_color_border` | `sub-border-color` | `borderColorSubtitles` | `pref_border_color_subtitles` | `-16777216` | `0xFF000000` opaque black (`Color.Black.toArgb()`) |
| 2 | `Background` | `player_sheets_subtitles_color_background` | `sub-back-color` | `backgroundColorSubtitles` | `pref_background_color_subtitles` | `0` | `0x00000000` transparent (`Color.Transparent.toArgb()`) |

Stored as signed 32-bit ARGB integers. Alpha is the top byte.

### 1.6 `SubtitleDelayType` (`.../panels/SubtitleDelayPanel.kt:194-200`)

| Ordinal | Name | titleRes key | English |
|---|---|---|---|
| 0 | `Primary` | `player_sheets_sub_delay_subtitle_type_primary` | Primary |
| 1 | `Secondary` | `player_sheets_sub_delay_subtitle_type_secondary` | Secondary |
| 2 | `Both` | `player_sheets_sub_delay_subtitle_type_primary_and_secondary` | Both |

Not persisted (UI state only).

### 1.7 `DelayType` (`.../panels/SubtitleDelayPanel.kt:370-373`)

`Audio` (0), `Subtitle` (1). Not persisted. Decides button labels and sign of runs (behavior §11.5).

### 1.8 `VideoFilters` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt:141-170`)

Constructor `(titleRes, preference: (DecoderPreferences) -> Preference<Int>, mpvProperty: String)`.

| Ordinal | Name | titleRes key | English | Preference | mpv property |
|---|---|---|---|---|---|
| 0 | `BRIGHTNESS` | `player_sheets_filters_brightness` | Brightness | `brightnessFilter()` | `brightness` |
| 1 | `SATURATION` | `player_sheets_filters_Saturation` | Saturation | `saturationFilter()` | `saturation` |
| 2 | `CONTRAST` | `player_sheets_filters_contrast` | Contrast | `contrastFilter()` | `contrast` |
| 3 | `GAMMA` | `player_sheets_filters_gamma` | Gamma | `gammaFilter()` | `gamma` |
| 4 | `HUE` | `player_sheets_filters_hue` | Hue | `hueFilter()` | `hue` |

Slider range for all: `min = -100`, `max = 100`, default `0`.

### 1.9 `Sheets` (`PlayerEnums.kt:99-108`)

Ordinals: `None` 0, `PlaybackSpeed` 1, `SubtitleTracks` 2, `AudioTracks` 3, `QualityTracks` 4, `Chapters` 5, `More` 6, `Screenshot` 7. Not persisted.

### 1.10 `Panels` (`PlayerEnums.kt:110-116`)

Ordinals: `None` 0, `SubtitleSettings` 1, `SubtitleDelay` 2, `AudioDelay` 3, `VideoFilters` 4. Not persisted.

### 1.11 `ArtType` (`PlayerEnums.kt:35-39`)

`Cover` 0, `Background` 1, `Thumbnail` 2. Used by `setAsArt`.

### 1.12 `SetAsArt` (`PlayerEnums.kt:29-33`)

`Success` 0, `AddToLibraryFirst` 1, `Error` 2. Returned by `PlayerViewModel.setAsArt`.

### 1.13 `HosterState` (sealed, `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/QualitySheet.kt:51-60`)

| Subclass | Fields | Meaning |
|---|---|---|
| `Idle(name: String)` | `name` | hoster not loaded; shows "Tap to load videos" |
| `Loading(name: String)` | `name` | loading; spinner |
| `Error(name: String)` | `name` | failed; "Failed to load videos" |
| `Ready(name: String, videoList: List<Video>, videoState: List<Video.State>)` | `name`, `videoList`, `videoState` (parallel lists, same length) | loaded; `videoList` may be empty |

`open val name: String` on the sealed base. Helper `HosterState.Ready.getChangedAt(index: Int, newVideo: Video, newState: Video.State): HosterState.Ready` (`QualitySheet.kt:62-72`).

### 1.14 `Video.State` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt:180-185`)

`QUEUE` 0, `LOAD_VIDEO` 1, `READY` 2, `ERROR` 3.

### 1.15 `Hoster.State` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Hoster.kt:22-27`)

`IDLE` 0, `LOADING` 1, `READY` 2, `ERROR` 3. Not read by this part; `HosterState` is the UI model.

### 1.16 `Hoster.NO_HOSTER_LIST`

`const val NO_HOSTER_LIST = "no_hoster_list"` (`Hoster.kt:57`). Sentinel: the only hoster in the list means "videos without hosters".

---

## 2. Data classes

### 2.1 `PlayerViewModel.VideoTrack` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:473-477`)

```kotlin
@Immutable
data class VideoTrack(
    val id: Int,          // mpv track-list id; -1 = "Off"
    val name: String,     // track title or the "Off" label
    val language: String?,
)
```

Rust: `struct TrackInfo { id: i64, name: String, language: Option<String> }`.

### 2.2 `IndexedSegment` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/SeekBar.kt:54-60`, used by chapters)

```kotlin
data class IndexedSegment(
    val name: String,
    val start: Float,                       // seconds
    val color: Color = Color.Unspecified,
    val index: Int = 0,
    val chapterType: ChapterType = ChapterType.Other,
)
```

Implements `dev.vivvvek.seeker.Segment` (external library). The sheet only reads `name` and `start`. `index` is `0` in `loadChapters()` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:486-490`).

### 2.3 `dev.vivvvek.seeker.Segment` (external)

Interface with `name` and `start`. Source not in repo; contract inferred from usage: `chapter.name` (String) and `chapter.start` (Float, seconds).

### 2.4 `Video` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt:30-40`)

Fields used here: `videoTitle: String = ""`. Other fields (`videoUrl`, `resolution`, `bitrate`, `headers`, `preferred`, ...) are area:16 and not used in this part.

### 2.5 `CustomButton` (`domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt:3-11`)

Used here: `id: Long` (mpv command id), `name: String` (chip label). Other fields (`isFavorite`, `sortIndex`, `content`, `longPressContent`, `onStartup`) belong to area:06 / part 1.

### 2.6 `CustomButton` mpv commands (`PlayerViewModel.kt:2167-2173`)

- Tap: `script-message` with argument `call_button_<id>`.
- Long press: `script-message` with argument `call_button_<id>_long`.

### 2.7 `CardColors` used by panels

Built by `panelCardsColors` (`PlayerPanels.kt:82-92`): container = `surface` with alpha `panelOpacity/100`; disabled container = `surfaceDim` with the same alpha.

### 2.8 Sheet state data (inputs)

- `QualitySheet` inputs: `isLoadingHosters: Boolean`, `hosterState: List<HosterState>`, `expandedState: List<Boolean>`, `selectedVideoIndex: Pair<Int, Int>`, `displayHosters: Pair<Boolean, Boolean>`.
- `ChaptersSheet` inputs: `chapters: ImmutableList<Segment>`, `currentChapter: Segment`, `dismissSheet: Boolean`.
- `SubtitlesSheet` inputs: `tracks: ImmutableList<VideoTrack>`, `selectedTracks: ImmutableList<Int>` (index 0 primary, 1 secondary; each element is a track id).
- `AudioTracksSheet` inputs: `tracks`, `selectedId: Int`.
- `MoreSheet` inputs: `selectedDecoder: Decoder`, `remainingTime: Int` (seconds), `customButtons: ImmutableList<CustomButton>`.
- `PlaybackSpeedSheet` inputs: `speed: Float`.
- `ScreenshotSheet` inputs: `isLocalSource`, `hasSubTracks`, `showSubtitles`, `cachePath: String`.

---

## 3. Preferences used by this part

Every key below is read or written by the 14 scope files, or is read at startup for a value those files write. Store: Android `SharedPreferences` through `PreferenceStore` (area:02). Types are the `PreferenceStore` getter types. "Default" is the getter's default argument.

### 3.1 Subtitle preferences (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/SubtitlePreferences.kt`)

| Accessor | Key | Type | Default | Written by (scope) | Read at startup (AniyomiMPVView.kt line) |
|---|---|---|---|---|---|
| `preferredSubLanguages()` | `pref_subtitle_lang` | String | `""` | not in scope | not in scope |
| `subtitleWhitelist()` | `pref_subtitle_whitelist` | String | `""` | not in scope | not in scope |
| `subtitleBlacklist()` | `pref_subtitle_blacklist` | String | `""` | not in scope | not in scope |
| `screenshotSubtitles()` | `pref_screenshot_subtitles` | Boolean | `false` | not in scope (ScreenshotSheet uses PlayerViewModel state) | not in scope |
| `subtitleFont()` | `pref_subtitle_font` | String | `"Sans Serif"` | Typography font dropdown; reset | 269 |
| `subtitleFontSize()` | `pref_subtitles_font_size` | Int | `55` | Typography font size slider; reset | 274 |
| `subtitleFontScale()` | `pref_sub_scale` | Float | `1f` | Misc card scale slider; reset | 288 |
| `subtitleBorderSize()` | `pref_sub_border_size` | Int | `3` | Typography border size slider; reset | 284 |
| `boldSubtitles()` | `pref_bold_subtitles` | Boolean | `false` | Typography bold toggle; reset | 275 |
| `italicSubtitles()` | `pref_italic_subtitles` | Boolean | `false` | Typography italic toggle; reset | 276 |
| `textColorSubtitles()` | `pref_text_color_subtitles` | Int (ARGB) | `Color.White.toArgb()` = `-1` | Colors card; reset | 278 |
| `borderColorSubtitles()` | `pref_border_color_subtitles` | Int (ARGB) | `Color.Black.toArgb()` = `-16777216` | Colors card; reset | 283 |
| `borderStyleSubtitles()` | `pref_border_style_subtitles` | Enum `SubtitlesBorderStyle` (name) | `OutlineAndShadow` | Typography border style menu; reset | 285 |
| `shadowOffsetSubtitles()` | `sub_shadow_offset` (**no `pref_` prefix**) | Int | `0` | Typography shadow slider; reset | 286 |
| `backgroundColorSubtitles()` | `pref_background_color_subtitles` | Int (ARGB) | `Color.Transparent.toArgb()` = `0` | Colors card; reset | 279-282 |
| `subtitleJustification()` | `pref_sub_justify` | Enum `SubtitleJustification` (name) | `Auto` | Typography justify toggles; reset | 277 |
| `subtitlePos()` | `pref_sub_pos` | Int | `100` | Misc card position slider; reset | 287 |
| `overrideSubsASS()` | `pref_override_subtitles_ass` | Boolean | `false` | Misc card override switch; reset (also used by typography reset for `sub-ass-justify`) | 270-272 (`force` and `sub-ass-justify yes` only when true) |
| `subtitlesDelay()` | `pref_subtitles_delay` | Int (ms) | `0` | Subtitle delay panel "Set as default" | 262 |
| `subtitlesSpeed()` | `pref_subtitles_speed` | Float | `1f` | Subtitle delay panel "Set as default" (only when `0.1..10`) | 263 |
| `subtitlesSecondaryDelay()` | `pref_subtitles_secondary_delay` | Int (ms) | `0` | **none in scope** (gap) | 264-267 |

Note on the enum-typed preferences: `getEnum` serializes `it.name` and deserializes with `enumValueOf`, falling back to the default on `IllegalArgumentException` (`PreferenceStore.kt:27-41`).

### 3.2 Audio preferences (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt`)

| Accessor | Key | Type | Default | Written by (scope) | Read at startup |
|---|---|---|---|---|---|
| `enablePitchCorrection()` | `pref_audio_pitch_correction` | Boolean | `true` | PlaybackSpeedSheet switch | `AniyomiMPVView.kt:257` (`audio-pitch-correction`) |
| `audioChannels()` | `pref_audio_config` | Enum `AudioChannels` (name) | `AutoSafe` | MoreSheet audio channel chips | `PlayerActivity.kt:586` (sets `it.property` = `it.value`) |

### 3.3 Player preferences (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt`)

| Accessor | Key | Type | Default | Written by (scope) | Read at startup |
|---|---|---|---|---|---|
| `playerSpeed()` | `pref_player_speed` | Float | `1f` | PlaybackSpeedSheet "Make default speed"; reset speed deletes it | `AniyomiMPVView.kt:150` (`speed`) |
| `speedPresets()` | `default_speed_presets` | StringSet | `setOf("0.25","0.5","0.75","1.0","1.25","1.5","1.75","2.0","2.5","3.0","3.5","4.0")` | PlaybackSpeedSheet add/remove, reset | not applied at startup (UI only) |
| `panelOpacity()` | `pref_panel_opacity` | Int (0..100) | `60` | (settings screen, part 1) | read live by `panelCardsColors` (`PlayerPanels.kt:87,89`) |

### 3.4 Decoder preferences (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/DecoderPreferences.kt`)

| Accessor | Key | Type | Default | Written by (scope) | Read |
|---|---|---|---|---|---|
| `tryHWDecoding()` | `pref_try_hwdec` | Boolean | `true` | not in scope | `AniyomiMPVView.kt:118` (`hwdec` = `auto` or `no`) |
| `gpuNext()` | `pref_gpu_next` | Boolean | `false` | not in scope | `VideoFiltersPanel.kt` warning (`FiltersCard`, not reactive) |
| `brightnessFilter()` | `pref_player_filter_brightness` | Int | `0` (no default argument) | VideoFiltersPanel slider; reset | (video filter startup, outside scope) |
| `saturationFilter()` | `pref_player_filter_saturation` | Int | `0` | VideoFiltersPanel | (outside scope) |
| `contrastFilter()` | `pref_player_filter_contrast` | Int | `0` | VideoFiltersPanel | (outside scope) |
| `gammaFilter()` | `pref_player_filter_gamma` | Int | `0` | VideoFiltersPanel | (outside scope) |
| `hueFilter()` | `pref_player_filter_hue` | Int | `0` | VideoFiltersPanel | (outside scope) |

### 3.5 Advanced player preferences (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AdvancedPlayerPreferences.kt`)

| Accessor | Key | Type | Default | Written by (scope) | Read at startup |
|---|---|---|---|---|---|
| `playerStatisticsPage()` | `pref_player_statistics_page` | Int (0..5) | `0` | MoreSheet statistics chips | `AniyomiMPVView.kt:163-169` |

### 3.6 Preference API used

- `Preference<T>.get()`, `.set(v)`, `.delete()`, `.defaultValue()`, `.collectAsState()` (Compose state), `deleteAndGet()` (= `delete()` then `get()`; returns the default). Source: `core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:56-59`.
- `Set<T>` preferences: `plusAssign`/`minus` helpers for speed presets (`Preference.kt:61+`).

---

## 4. mpv properties and commands (exact names, types, and where)

Types: `D` = double, `I` = int, `B` = bool (`MPVLib.getPropertyBoolean`/`setPropertyBoolean`), `S` = string, `C` = command.

### 4.1 Subtitle properties

| Property | Type | Read in | Written in | Notes / values |
|---|---|---|---|---|
| `sub-delay` | D (seconds) | SubtitleDelayPanel (lines 89, 115), AniyomiMPVView (262 startup) | SubtitleDelayPanel (lines 100, 103-104) | ms / 1000.0 |
| `secondary-sub-delay` | D (seconds) | SubtitleDelayPanel (91, 110-111) | SubtitleDelayPanel (101, 104) | ms / 1000.0 |
| `sub-speed` | D | SubtitleDelayPanel (93) | SubtitleDelayPanel (95) only when `0.1 <= speed <= 1` | multiplier |
| `sub-ass-override` | S (`force`/`scale`) | Misc card (75) | Misc card (82, 145), AniyomiMPVView (271 when true) | `scale` is mpv's default |
| `sub-scale` | D | Misc card (89) | Misc card (101, 142) | 0..5 slider |
| `sub-pos` | I | Misc card (92) | Misc card (118, 138) | 0..150 slider |
| `sub-bold` | B | Typography (134) | Typography (174, 357) | |
| `sub-italic` | B | Typography (135) | Typography (188, 358) | |
| `sub-justify` | S (`left`/`center`/`right`/`auto`) | Typography (139) | Typography (205, 208, 360) | |
| `sub-ass-justify` | B | (not read) | Typography (202, 359) | toggled by justification toggles; reset sets it from `overrideSubsASS` (bug, behavior §13.8) |
| `sub-font` | S | Typography (143) | Typography (255, 361; line 143 and 222 are reads) | |
| `sub-font-size` | I | Typography (145) | Typography (269, 362) | |
| `sub-border-style` | S | Typography (149) | Typography (311, 365) | `outline-and-shadow` / `opaque-box` / `background-box` |
| `sub-border-size` | I | Typography (154) | Typography (333, 363) | |
| `sub-shadow-offset` | I | Typography (159) | Typography (345, 364) | |
| `sub-color` | S (`#AARRGGBB`) | Colors (203 via `getCurrentMPVColor`) | Colors (141; `resetColors` at lines 180-200) | |
| `sub-border-color` | S | Colors | Colors (187-189) | |
| `sub-back-color` | S | Colors | Colors (194-196) | |

### 4.2 Audio and playback properties

| Property | Type | Read in | Written in | Notes |
|---|---|---|---|---|
| `audio-pitch-correction` | B | — | PlaybackSpeedSheet (140) | |
| `audio-channels` | S | — | MoreSheet (252, 256) | values from `AudioChannels` (section 1.2) |
| `af` | S | — | MoreSheet (254) (set to `""` or the pan string) | clears all audio filters when set to `""` |
| `hwdec` | S | — | PlayerViewModel.kt:389-391 (`updateDecoder`) | `Decoder.value` |
| `hwdec-current` | S | AniyomiMPVView.kt:228 registration / 79-80 (`hwdecActive`) | — | Decoder selected chip; Linux values are not in the enum |
| `chapter-list/count` | I | PlayerViewModel.kt:481 | — | `!!` on read |
| `chapter-list/N/title` | S | PlayerViewModel.kt:483 | — | may be null |
| `chapter-list/N/time` | I | PlayerViewModel.kt:484 | — | seconds, `!!` |
| `track-list/N/lang` | S | PlayerViewModel.kt:393-398 (`getTrackLanguage`) | — | `""` fallback |
| `track-list/N/title` | S | PlayerViewModel.kt:400-405 (`getTrackTitle`) | — | `""` fallback |
| `speed` | D | (startup AniyomiMPVView.kt:150) | (not in this part) | |

### 4.3 Commands

| Command | Arguments | Used by | Notes |
|---|---|---|---|
| `script-binding` | `stats/display-stats-toggle` | MoreSheet (197); AniyomiMPVView.kt:165 (startup) | toggles statistics overlay |
| `script-binding` | `stats/display-page-$page` (page 1..5) | MoreSheet (200); AniyomiMPVView.kt:166 | |
| `screenshot-to-file` | `<path>`, `subtitles` or `video` | PlayerViewModel.kt:1834 (`takeScreenshot`) | writes PNG to temp file |
| `script-message` | `call_button_<id>` | `CustomButton.execute` (PlayerViewModel.kt:2168) | |
| `script-message` | `call_button_<id>_long` | `CustomButton.executeLongPress` (PlayerViewModel.kt:2172) | |

### 4.4 Startup option writes that mirror this part (`AniyomiMPVView.kt:254-288`)

`setOptionString` calls (not properties): `alang`, `audio-delay`, `audio-pitch-correction`, `volume-max` (audio, 254-258), and the subtitle options (261-288) listed in section 3.1. Startup writes use string forms: booleans as `yes`/`no` for `sub-bold` and `sub-italic` (lines 275-276), and `true`/`false` for `audio-pitch-correction` (line 257, `toString()`).

---

## 5. Files and folders on disk

| Path (relative to app base storage) | Created by | Used by | Notes |
|---|---|---|---|
| `mpv-config/` | `StorageManager.getMPVConfigDirectory()` (`domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt:93-95`) | all mpv config | constant `MPV_CONFIG_PATH = "mpv-config"` (line 102) |
| `mpv-config/fonts/` | `StorageManager.getFontsDirectory()` (line 77-79) | Typography card font list | constant `FONTS_PATH = "fonts"` (line 103); created on every call to `getFontsDirectory()` |
| `mpv-config/scripts/` | `getScriptsDirectory()` | mpv scripts (outside scope) | `SCRIPTS_PATH` (line 104) |
| `mpv-config/script-opts/` | `getScriptOptsDirectory()` | mpv script options (outside scope) | `SCRIPT_OPTS_PATH` (line 105) |
| `<cachePath>/<epochMillis>_mpv_screenshot_tmp.png` | ScreenshotSheet via `PlayerViewModel.takeScreenshot` (line 1830) | temp screenshot | `cachePath` is supplied by PlayerSheets; a new name per call |
| `<cachePath>/mpv_screenshot.png` | `PlayerViewModel.takeScreenshot` (lines 1838-1839: delete then rename) | the image handed to share, save, set-as-art | one fixed name, overwritten each call |

Accepted font extensions: regex `.*\.[ot]tf$` (case-insensitive by lowercasing the name), i.e. `.ttf` and `.otf` only. Source: `SubtitleSettingsTypographyCard.kt:354`.

---

## 6. Message keys and English strings (all keys used by this part)

Format: key — English text (source file where the key is used is listed in `p2-behavior.md` by section). `MR.strings.*` = shared module `i18n/src/commonMain/moko-resources/base/strings.xml`; `AYMR.strings.*` = `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`; `AYMR.plurals.*` = `i18n-aniyomi/src/commonMain/moko-resources/base/plurals.xml`.

### 6.1 Track, sheet and panel labels

- `pref_player_audio` — Audio
- `pref_player_subtitle` — Subtitles
- `player_sheets_add_ext_audio` — Add external audio tracks
- `player_sheets_add_ext_sub` — Add external subtitles
- `player_sheets_track_delay` — Delay
- `player_sheets_track_palette` — Palette
- `player_sheets_track_title_wo_lang` — `#%d: %s`
- `player_sheets_track_title_w_lang` — `#%d: %s (%s)`
- `player_sheets_track_lang_wo_title` — `#%d: %s`
- `player_sheets_tracks_off` — Off
- `player_sheets_chapters_title` — Chapters
- `player_sheets_qualities_title` — Qualities
- `player_hoster_tap_to_load` — Tap to load videos
- `player_hoster_failed` — Failed to load videos
- `hoster_video_count` (plural) — one: `%d video`; other: `%d videos`
- `player_sheets_more_title` — More
- `player_hwdec_mode` — Hardware decoding mode
- `player_sheets_stats_page_title` — Default statistics page
- `player_sheets_stats_page_chip` — Page %d
- `player_sheets_custom_buttons_title` — Custom buttons
- `pref_audio_channels` — Audio channels
- `pref_player_audio_channels_auto` — Auto
- `pref_player_audio_channels_auto_safe` — Auto-safe
- `pref_player_audio_channels_mono` — Mono
- `pref_player_audio_channels_stereo` — Stereo
- `pref_player_audio_channels_reverse_stereo` — Reverse stereo
- `player_sheets_filters_title` — Filters
- `player_sheets_filters_warning` — Some filters may not work on your current video driver
- `player_sheets_filters_brightness` — Brightness
- `player_sheets_filters_Saturation` — Saturation (note capital S in the key)
- `player_sheets_filters_contrast` — Contrast
- `player_sheets_filters_gamma` — Gamma
- `player_sheets_filters_hue` — Hue
- `player_speed` — `%.2fx`
- `player_sheets_speed_slider_label` — Speed
- `player_sheets_speed_make_default` — Make default speed
- `pref_audio_pitch_correction_title` — Enable audio pitch correction
- `pref_audio_pitch_correction_summary` — Prevents the audio from becoming high-pitched at faster speeds and low-pitched at slower speeds
- `timer_title` — Sleep Timer
- `timer_remaining` — `%s remaining`
- `timer_picker_enter_timer` — Enter duration
- `timer_picker_pick_time` — Pick duration
- `timer_cancel_timer` — Cancel current timer
- `action_ok` — OK (`MR`)
- `action_reset` — Reset (`MR`)
- `action_share` — Share (`MR`)
- `action_save` — Save (`MR`)
- `set_as_cover` — Set as cover (`MR`)
- `set_as_background` — Set as background (`AYMR`)
- `set_as_thumbnail` — Set as thumbnail (`AYMR`)
- `confirm_set_image_as_cover` — Use this image as cover art? (`MR`)
- `screenshot_show_subs` — Include Subtitles
- `off` — Off (`MR`, used for track id -1 in PlayerViewModel.kt:400+)

### 6.2 Subtitle settings and delay

- `player_sheets_subtitles_settings_title` — Subtitles settings
- `player_sheets_sub_delay_title` — Subtitle delay
- `player_sheets_sub_delay_subtitle_type_primary` — Primary
- `player_sheets_sub_delay_subtitle_type_secondary` — Secondary
- `player_sheets_sub_delay_subtitle_type_primary_and_secondary` — Both
- `player_sheets_sub_delay_audio_sound_heard` — Sound heard
- `player_sheets_sub_delay_sound_sound_spotted` — Sound spotted
- `player_sheets_sub_delay_subtitle_voice_heard` — Voice heard
- `player_sheets_sub_delay_subtitle_text_seen` — Text seen
- `player_sheets_sub_delay_delay` — Delay
- `player_generic_unit_ms` — ms
- `player_sheets_sub_delay_speed` — Speed
- `player_sheets_delay_set_as_default` — Set as default
- `numeric_chooser_value_too_big` — Value too big (presentation)
- `numeric_chooser_value_too_small` — Value too small (presentation)
- `player_sheets_sub_scale` — Scale
- `player_sheets_sub_position` — Position
- `player_sheets_sub_override_ass` — Override ASS/SSA subtitles
- `player_sheets_sub_misc_title` — Miscellaneous
- `player_sheets_sub_typography_title` — Typography
- `player_sheets_sub_typography_font` — Font
- `player_sheets_sub_typography_font_size` — Font size
- `player_sheets_sub_typography_border_style` — Border style
- `player_sheets_sub_typography_border_size` — Border size
- `player_sheets_subtitles_shadow_offset` — Shadow offset
- `player_sheets_subtitles_border_style_outline_and_shadow` — Outline and shadow
- `player_sheets_subtitles_border_style_opaque_box` — Opaque box
- `player_sheets_subtitles_border_style_background_box` — Background box
- `player_sheets_sub_colors_title` — Colors
- `player_sheets_subtitles_color_text` — Text
- `player_sheets_subtitles_color_border` — Border
- `player_sheets_subtitles_color_background` — Background
- `player_sheets_sub_color_red` — Red
- `player_sheets_sub_color_green` — Green
- `player_sheets_sub_color_blue` — Blue
- `player_sheets_sub_color_alpha` — Alpha
- `player_sheets_subtitles_footer_secondary_sid_no_styles` — Secondary subtitles will have no ASS/SSA Styling.
- `player_sheets_audio_delay_title` — Audio delay (used by part 1 AudioDelayPanel)

### 6.3 Keys referenced by the Kotlin but resolved elsewhere

- `R.drawable.outline_brand_family_24`, `R.drawable.sharp_shadow_24` (Android drawables, Typography card icons; replaced by GTK icon names in the port, see p2-rust-design.md).

---

## 7. Network endpoints

None. The 14 scope files make no network calls.

## 8. Database tables and columns

None. The scope reads and writes preferences and mpv state only. Preference storage is Android `SharedPreferences` (area:02); the port uses the settings store defined in area:02.

## 9. Constants and magic numbers (exact)

| Constant | Value | Where |
|---|---|---|
| `CARDS_MAX_WIDTH` | `420.dp` | `PlayerPanels.kt:81` |
| `PlayerSheet` max width portrait / landscape | `420.dp` / `720.dp` | `app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt` (`maxWidth` expression, line 87; `maxHeight` line 92) |
| `PlayerSheet` max height | `screenHeightDp * 0.95f` | same |
| `sheetAnimationSpec` | `tween<Float>(350)` | `app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt`:74 |
| `positionalThreshold` | `56.dp` | `app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt`:107 |
| `velocityThreshold` | `125.dp` | `app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt`:108 |
| Scrim alpha (open) | `0.5f` | `app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt` (`LaunchedEffect(true)`, line 188) |
| Delay tap-sync tick | `20 ms` | `SubtitleDelayPanel.kt:261` |
| Delay Int step | `50` ms | `SubtitleDelayPanel.kt:234` |
| Delay min / max | `Int.MIN_VALUE` / `Int.MAX_VALUE` | `SubtitleDelayPanel.kt:235-236` |
| Sub speed min / max / step | `0.1f` / `10f` / `.01f` | `SubtitleDelayPanel.kt:180-182` |
| Sub speed live-apply range | `0.1f..1f` | `SubtitleDelayPanel.kt:95` |
| Sub speed save range | `0.1f..10f` | `SubtitleDelayPanel.kt:132` |
| Speed slider min / max | `0.01f` / `6f` | `PlaybackSpeedSheet.kt:81-82` |
| Speed rounding (delay panel) | `round(it * 1000) / 1000f` | `SubtitleDelayPanel.kt:127` |
| `toFixed` default precision | `1` (calls use `2`) | `PlaybackSpeedSheet.kt:178-181` |
| Font size slider | `1..100` | `SubtitleSettingsTypographyCard.kt:262-263` |
| Border size slider | `0..100` (min default) | `SubtitleSettingsTypographyCard.kt:335` |
| Shadow offset slider | `0..100` (min default) | `SubtitleSettingsTypographyCard.kt:347` |
| Sub scale slider | `0..5` (min default `0f`) | `SubtitleSettingsMiscellaneousCard.kt:103` |
| Sub position slider | `0..150` (min default `0`) | `SubtitleSettingsMiscellaneousCard.kt:120` |
| Color slider | `0..255` | `SubtitleSettingsColorsCard.kt:219-247` |
| Video filter slider | `-100..100`, steps 200 | `VideoFiltersPanel.kt:130-131` |
| Statistics pages | `0..5` (6 chips) | `MoreSheet.kt:179` |
| Sleep timer granularity | minutes (seconds output) | `MoreSheet.kt:343` |
| `DISABLED_ALPHA` | `.38f` | `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt:9` |
| Hoster spinner size / stroke | `24.dp` / `2.dp` | `QualitySheet.kt:323-324` |
| Video icon size | `28.dp` (no hoster) / `20.dp` (hoster) | `QualitySheet.kt:395` |
| Track row height (`AddTrackRow`) | `48.dp` | `GenericTracksSheet.kt:84` |
| Hoster row height | `32.dp` | `QualitySheet.kt:291` |
| Card padding (track sheets) | `padding.medium` / `padding.small` | various |
| Speed default presets | `0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0` | `PlayerPreferences.kt:76-79` |
| Ms per second conversion | `1000` / `1000.0` | Delay panel, startup |
| Hour and minute split | `/ 3600`, `(s % 3600) / 60` | `MoreSheet.kt:302-303` |
| Sleep timer total | `hour * 3600 + minute * 60` | `MoreSheet.kt:343` |

## 10. Layout constants from the mockup spec that map to this part

These come from `spec/reader_player.json` (`screens[player]`), not from Kotlin. Used by the Rust port for parity (see p2-ui-parity.md).

- Sheets: `subtitle_tracks`, `audio_tracks`, `quality_tracks`, `chapters`, `playback_speed`, `more`, `screenshot`.
- Panels: `panel_subtitle_settings`, `panel_subtitle_delay`, `panel_audio_delay`, `panel_video_filters`.
- Dialogs: `sleep_timer` (title "Enter duration", toggle text "Toggle to clock picker: title becomes \"Pick duration\""), `set_art_confirm`.
