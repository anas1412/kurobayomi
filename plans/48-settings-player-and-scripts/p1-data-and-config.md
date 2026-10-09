# P1 data models, preferences, files, endpoints and tables: 48-settings-player-and-scripts

Repo-relative paths are relative to `/home/blackbox/aniyomi-src`. Storage backend: `PreferenceStore` (`core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt`) backed by Android SharedPreferences. Enum values are stored by constant **name** (`serializer = { it.name }`, `PreferenceStore.kt:27-42`); an unknown name falls back to the default value silently.

Legend for "Store": `pref` = preference store key; `db` = SQLite table; `file` = file on disk; `none` = not persisted.

---

## 1. Preference keys used in scope

### 1.1 UI preferences (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt`)

| Key | Type | Default | Enum / values | Used by |
|---|---|---|---|---|
| `pref_theme_mode_key` | enum `ThemeMode` | `SYSTEM` | `LIGHT`, `DARK`, `SYSTEM` | `setAppCompatDelegateThemeMode` |
| `pref_app_theme` | enum `AppTheme` | `MONET` if dynamic colour available, else `DEFAULT` | see section 3.1 | theme code (area:36) |
| `pref_theme_dark_amoled_key` | Boolean | `false` | | theme code |
| `relative_time_v2` | Boolean | `true` | | date text |
| `app_date_format` | String | `""` | `""` = localised short date; else a Java `DateTimeFormatter` pattern (`Locale.getDefault()`) | `DateText.kt` |
| `tablet_ui_mode` | enum `TabletUiMode` | `AUTOMATIC` | `AUTOMATIC`, `ALWAYS`, `LANDSCAPE`, `NEVER` | dropped on desktop |
| `start_screen` | enum `StartScreen` | `ANIME` | `ANIME`, `MANGA`, `UPDATES`, `HISTORY`, `BROWSE` | shell (area:36) |
| `bottom_rail_nav_style` | enum `NavStyle` | `MOVE_HISTORY_TO_MORE` | `MOVE_MANGA_TO_MORE`, `MOVE_UPDATES_TO_MORE`, `MOVE_HISTORY_TO_MORE`, `MOVE_BROWSE_TO_MORE` | shell (area:36) |

### 1.2 Player core preferences read by scope screens (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt`)

| Key | Type | Default | Constraints / entries |
|---|---|---|---|
| `pref_progress_preference` | Float | `0.85F` | entries 1.00, 0.95, 0.90, 0.85, 0.80, 0.75, 0.70 |
| `pref_preserve_watching_position` | Boolean | `false` | |
| `pref_default_player_orientation_type_key` | enum `PlayerOrientation` | `SensorLandscape` | 8 constants (section 3.2) |
| `pref_allow_gestures_in_panels` | Boolean | `false` | |
| `pref_show_loading` | Boolean | `true` | |
| `pref_show_current_chapter` | Boolean | `true` | |
| `pref_remember_brightness` | Boolean | `false` | read by `PlayerActivity` |
| `player_brightness_value` | Float | `-1.0F` | non-pref; written by player |
| `pref_remember_volume` | Boolean | `false` | **no reader** (dead) |
| `player_volume_value` | Float | `-1.0F` | non-pref; **no reader** |
| `pref_show_failed_hosters` | Boolean | `false` | |
| `pref_show_empty_hosters` | Boolean | `false` | |
| `player_fullscreen` | Boolean | `true` | Android API 28+ gate in settings |
| `player_hide_controls` | Boolean | `false` | |
| `pref_display_vol_as_per` | Boolean | `true` | |
| `pref_show_system_status_bar` | Boolean | `false` | Android-only, dropped |
| `pref_reduce_motion` | Boolean | `false` | |
| `pref_player_time_to_disappear` | Int | `4000` | entries 500..5000 step 500 |
| `pref_panel_opacity` | Int | `60` | range 0..100 |
| `pref_enable_skip_intro` | Boolean | `true` | |
| `pref_enable_auto_skip_ani_skip` | Boolean | `false` | |
| `pref_enable_netflixStyle_aniskip` | Boolean | `false` | key spelling kept |
| `pref_waiting_time_aniskip` | Int | `5` | entries 5..10 |
| `pref_enable_ani_skip` | Boolean | `false` | |
| `pref_disabled_ani_skip_chapters` | Boolean | `true` | |
| `pref_enable_pip` | Boolean | `true` | Android PiP gate in settings |
| `pref_pip_episode_toasts` | Boolean | `true` | |
| `pref_pip_on_exit` | Boolean | `false` | |
| `pip_replace_with_previous` | Boolean | `false` | key has no `pref_` prefix |
| `pref_always_use_external_player` | Boolean | `false` | |
| `external_player_preference` | String | `""` | package name, `""` = system chooser |
| `pref_player_speed` | Float | `1f` | non-pref |
| `default_speed_presets` | String set | `0.25,0.5,0.75,1.0,1.25,1.5,1.75,2.0,2.5,3.0,3.5,4.0` | non-pref; area:46 |
| `invert_duration` | Boolean | `false` | non-pref |
| `pref_player_aspect_state` | enum `VideoAspect` | `Fit` | `Crop`, `Fit`, `Stretch` (not in scope) |
| `pref_auto_play_enabled` | Boolean | `false` | legacy |

### 1.3 Gesture preferences (`GesturePreferences.kt`)

| Key | Type | Default | Notes |
|---|---|---|---|
| `pref_gesture_volume_brightness` | Boolean | `true` | |
| `pref_swap_volume_and_brightness` | Boolean | `false` | |
| `pref_gesture_horizontal_seek` | Boolean | `true` | |
| `pref_show_seekbar` | Boolean | `false` | |
| `pref_default_intro_length` | Int | `85` | seconds; range 0..255 in dialog |
| `pref_skip_length_preference` | Int | `10` | seconds; entries 30, 20, 10, 5, 3, 0 |
| `pref_player_smooth_seek` | Boolean | `false` | |
| `pref_left_double_tap` | enum `SingleActionGesture` | `Seek` | |
| `pref_center_double_tap` | enum `SingleActionGesture` | `PlayPause` | |
| `pref_right_double_tap` | enum `SingleActionGesture` | `Seek` | |
| `pref_media_previous` | enum `SingleActionGesture` | `Switch` | |
| `pref_media_playpause` | enum `SingleActionGesture` | `PlayPause` | |
| `pref_media_next` | enum `SingleActionGesture` | `Switch` | |

### 1.4 Decoder preferences (`DecoderPreferences.kt`)

| Key | Type | Default |
|---|---|---|
| `pref_try_hwdec` | Boolean | `true` |
| `pref_gpu_next` | Boolean | `false` |
| `pref_video_debanding` | enum `Debanding` | `None` |
| `use_yuv420p` | Boolean | `true` |
| `pref_player_filter_brightness` | Int (no default; unset = null) | — (video filter sheet; area:46) |
| `pref_player_filter_saturation` | Int | — |
| `pref_player_filter_contrast` | Int | — |
| `pref_player_filter_gamma` | Int | — |
| `pref_player_filter_hue` | Int | — |

### 1.5 Audio preferences (`AudioPreferences.kt`)

| Key | Type | Default | Notes |
|---|---|---|---|
| `pref_audio_lang` | String | `""` | comma-separated language codes |
| `pref_audio_pitch_correction` | Boolean | `true` | |
| `pref_audio_config` | enum `AudioChannels` | `AutoSafe` | section 3.3 |
| `pref_audio_volume_boost_cap` | Int | `30` | range 0..200 |
| `pref_audio_delay` | Int | `0` | ms; non-pref |

### 1.6 Subtitle preferences (`SubtitlePreferences.kt`; scope reads three keys, the rest are read by area:46)

| Key | Type | Default | Scope use |
|---|---|---|---|
| `pref_subtitle_lang` | String | `""` | subtitle page |
| `pref_subtitle_whitelist` | String | `""` | subtitle page |
| `pref_subtitle_blacklist` | String | `""` | subtitle page |
| `pref_subtitle_font` | String | `"Sans Serif"` | area:46 |
| `pref_subtitles_font_size` | Int | `55` | area:46 |
| `pref_sub_scale` | Float | `1f` | area:46 |
| `pref_sub_border_size` | Int | `3` | area:46 |
| `pref_bold_subtitles` | Boolean | `false` | area:46 |
| `pref_italic_subtitles` | Boolean | `false` | area:46 |
| `pref_text_color_subtitles` | Int (ARGB) | `Color.White.toArgb()` = `-1` | area:46 |
| `pref_border_color_subtitles` | Int (ARGB) | `Color.Black.toArgb()` = `-16777216` | area:46 |
| `pref_border_style_subtitles` | enum `SubtitlesBorderStyle` | `OutlineAndShadow` | area:46 |
| `sub_shadow_offset` | Int | `0` | area:46 |
| `pref_background_color_subtitles` | Int (ARGB) | `Color.Transparent.toArgb()` = `0` | area:46 |
| `pref_sub_justify` | enum `SubtitleJustification` | `Auto` | values `left`, `center`, `right`, `auto` |
| `pref_sub_pos` | Int | `100` | area:46 |
| `pref_override_subtitles_ass` | Boolean | `false` | area:46 |
| `pref_subtitles_delay` | Int | `0` | ms |
| `pref_subtitles_speed` | Float | `1f` | |
| `pref_subtitles_secondary_delay` | Int | `0` | ms |
| `pref_screenshot_subtitles` | Boolean | `false` | area:46 |

### 1.7 Advanced preferences (`AdvancedPlayerPreferences.kt`)

| Key | Type | Default | Notes |
|---|---|---|---|
| `mpv_scripts` | Boolean | `false` | "Use user MPV files" |
| `pref_mpv_conf` | String | `""` | text of `mpv.conf` |
| `pref_mpv_input` | String | `""` | text of `input.conf` |
| `pref_player_statistics_page` | Int | `0` | non-pref; area:46 |

### 1.8 Torrent preferences (`TorrentPreferences.kt`)

| Key | Type | Default |
|---|---|---|
| `pref_torrserver_enable` | Boolean | `false` |
| `pref_torrserver_shownotice` | Boolean | `false` |
| `pref_torrserver_port` | String | `"8090"` |
| `pref_torrserver_tackers` (sic) | String | 26-line list, `p1-topic-torrent-settings.md` section 3 |
| `pref_torrserver_proxymode` | enum `ProxyMode` | `None` |
| `pref_torrserver_proxyurl` | String | `""` |

### 1.9 Language-list migration keys (legacy, read by migrations only)

`pref_subtitle_lang`, `pref_audio_lang` (migration 130, `PrefLangMigration`). Other legacy keys (`pref_sub_select_conf`, old orientation int, `tracking_queue` shared-prefs file) are read by migrations 92-127 (area:11). Not needed on desktop.

---

## 2. Custom button database (`db`)

Table `custom_buttons` (anime database, `data/src/main/sqldelightanime/dataanime/custom_buttons.sq`, created by migration 129):

| Column | SQL type | Null | Default | Notes |
|---|---|---|---|---|
| `_id` | INTEGER | NOT NULL | — | PRIMARY KEY (rowid alias); auto-assigned |
| `name` | TEXT | NOT NULL | — | title; no uniqueness constraint |
| `isFavorite` | INTEGER (Boolean) | NOT NULL | — | 0/1; at most one 1 by convention, not by constraint |
| `sortIndex` | INTEGER | NOT NULL | — | 0-based contiguous after delete/reorder |
| `content` | TEXT | NOT NULL | — | Lua body (tap) |
| `longPressContent` | TEXT | NOT NULL | — | Lua body (long press); may be `""` |
| `onStartup` | TEXT | NOT NULL | — | Lua body run at load; may be `""` |

Seed row: `_id = 1`, `name = '+85 s'`, `isFavorite = 1`, `sortIndex = 0`, `content`, `longPressContent`, `onStartup` as given in `p1-topic-custom-buttons.md` section 3. Inserted with `INSERT OR IGNORE`.

Indices: none. Foreign keys: none. Triggers: none.

Queries: see `p1-topic-custom-buttons.md` section 3.

---

## 3. Enums (stored name, labels, numeric/string values)

### 3.1 `AppTheme` (`app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt`)
Stored by name. Ordinal order = declaration order. Titles in `p1-behavior.md` section 15.
DEFAULT, MONET, CLOUDFLARE, COTTONCANDY, DOOM, GREEN_APPLE, LAVENDER, MATRIX, MIDNIGHT_DUSK, MOCHA, SAPPHIRE, NORD, STRAWBERRY_DAIQUIRI, TAKO, TEALTURQUOISE, TIDAL_WAVE, YINYANG, YOTSUBA, MONOCHROME, DARK_BLUE (deprecated), HOT_PINK (deprecated), BLUE (deprecated).

### 3.2 `PlayerOrientation` (`PlayerEnums.kt`)
Stored by name. Free, Video, Portrait, ReversePortrait, SensorPortrait, Landscape, ReverseLandscape, SensorLandscape (default).
Android mapping used by `PlayerActivity.setupPlayerOrientation` (`:834-852`): Free → `SCREEN_ORIENTATION_SENSOR`; Video → `SENSOR_LANDSCAPE` if video aspect > 1.0 else `SENSOR_PORTRAIT`; Portrait → `PORTRAIT`; ReversePortrait → `REVERSE_PORTRAIT`; SensorPortrait → `SENSOR_PORTRAIT`; Landscape → `LANDSCAPE`; **ReverseLandscape → `REVERSE_PORTRAIT` (bug; should be `REVERSE_LANDSCAPE`)**; SensorLandscape → `SENSOR_LANDSCAPE`. Desktop: dropped.
Legacy int mapping (migration 127, `VideoOrientationMigration.kt`): old int → `SCREEN_ORIENTATION_FULL_SENSOR`→Free, PORTRAIT→Portrait, REVERSE_PORTRAIT→ReversePortrait, LANDSCAPE→Landscape, REVERSE_LANDSCAPE→ReverseLandscape, SENSOR_PORTRAIT→SensorPortrait, SENSOR_LANDSCAPE→SensorLandscape, else→Free. Default int when absent: `10`.

### 3.3 `AudioChannels` (`AudioPreferences.kt`)
Stored by name. Fields `(titleRes, property, value)`:

| Name | Title | property | value |
|---|---|---|---|
| Auto | Auto | `audio-channels` | `auto-safe` |
| AutoSafe | Auto-safe | `audio-channels` | `auto` |
| Mono | Mono | `audio-channels` | `mono` |
| Stereo | Stereo | `audio-channels` | `stereo` |
| ReverseStereo | Reverse stereo | `af` | `pan=[stereo\|c0=c1\|c1=c0]` |

Default `AutoSafe`. Labels and values are crossed for Auto/AutoSafe (observed; see `p1-behavior.md` section 7).

### 3.4 `SingleActionGesture` (`PlayerEnums.kt`)
Stored by name. None ("None"), Seek ("Seek"), PlayPause ("Play/Pause"), Switch ("Change episode"), Custom ("Custom").

### 3.5 `CustomKeyCodes` (`PlayerEnums.kt`)
`keyCode` string values: DoubleTapLeft `"0x10001"`, DoubleTapCenter `"0x10002"`, DoubleTapRight `"0x10003"`, MediaPrevious `"0x10004"`, MediaPlay `"0x10005"`, MediaNext `"0x10006"`. Not stored; used when a `Custom` action fires.

### 3.6 `Debanding` (`PlayerEnums.kt`)
Stored by name: `None`, `CPU`, `GPU`. Labels are the raw names.

### 3.7 `Decoder` (`PlayerEnums.kt`, not in scope screens)
Constant (title, value): AutoCopy ("Auto", `auto-copy`), Auto ("Auto", `auto`), SW ("SW", `no`), HW ("HW", `mediacodec-copy`), HWPlus ("HW+", `mediacodec`). Mapping lookup `getDecoderFromValue(value)` returns the first entry with that value (duplicates resolve to the first). Not user-selectable in scope; the mpv application uses `hwdec` `auto`/`no` only (`AniyomiMPVView.kt:118`).

### 3.8 `ProxyMode` (`TorrentPreferences.kt`)
Stored by name. Constant → value: None → `tracker` (**duplicate, observed quirk**), Tracker → `tracker`, Peers → `peers`, Full → `full`.

### 3.9 `ThemeMode` (`ThemeMode.kt`)
Stored by name: LIGHT, DARK, SYSTEM. Mapping to `AppCompatDelegate` night modes: LIGHT → `MODE_NIGHT_NO`, DARK → `MODE_NIGHT_YES`, SYSTEM → `MODE_NIGHT_FOLLOW_SYSTEM`.

### 3.10 `TabletUiMode`, `StartScreen`, `NavStyle` (`app/src/main/java/eu/kanade/domain/ui/model/`)
Stored by name. Values in section 1.1.

### 3.11 `EditorListType` (`PlayerSettingsEditorScreenModel.kt`)
Not persisted. `SCRIPTS(directoryName = "scripts", fileExtension = "lua")`, `SCRIPTS_OPTS(directoryName = "script-opts", fileExtension = "conf")`.

### 3.12 `SubtitleJustification` (`SubtitlePreferences.kt`, area:46)
Stored by name. `value` strings: Left `left`, Center `center`, Right `right`, Auto `auto`.

### 3.13 Bit-flag layouts
None exist in scope. Booleans are single preference keys, not packed bits.

---

## 4. Files, folders and formats on disk

### 4.1 Base directory (user-chosen, `baseStorageDirectory` preference, a content URI string on Android)
Created children (see `p1-behavior.md` section 16):
- `autobackup/`, `local/`, `localanime/`, `downloads/` (+ `.nomedia`), `mpv-config/`.
- `mpv-config/fonts/`, `mpv-config/scripts/`, `mpv-config/script-opts/`, `mpv-config/shaders/`.

User-visible contents:
- `mpv-config/scripts/*.lua`: UTF-8 Lua source, created by the editor as `file.lua` by default.
- `mpv-config/script-opts/*.conf`: UTF-8 key=value text, default name `file.conf`.
- `mpv-config/mpv.conf`, `mpv-config/input.conf`: mirror only (see behaviour section 8).
- `mpv-config/shaders/*`, `mpv-config/fonts/*`: copied verbatim.

### 4.2 Player runtime directory (`filesDir/mpv/`, Android internal; Linux equivalent proposed)
- `mpv.conf` (text of `pref_mpv_conf`, overwritten each start).
- `input.conf` (text of `pref_mpv_input`, overwritten each start).
- `scripts/aniyomi.lua` (asset copy, overwritten each start).
- `scripts/custombuttons.lua` (generated, overwritten each player start).
- `scripts/` (user copies, only when `mpv_scripts`), `script-opts/`, `shaders/` (same).
- `fonts/` (copied from `mpv-config/fonts`), `subfont.ttf`, `cacert.pem` (assets; copied when size differs).

### 4.3 Bundled assets (`app/src/main/assets/`)
- `aniyomi.lua`: 90 lines, the Lua bridge (`p1-topic-custom-buttons.md` section 8).
- `subfont.ttf`: subtitle fallback font (binary, not read by this area).
- `cacert.pem`: CA bundle for mpv TLS (binary, not read by this area).

### 4.4 Text encodings and line endings
- All editor files: read as UTF-8 (lossy), written as UTF-8, LF line endings as typed (no conversion).
- Preference strings: Kotlin `String` (UTF-16 in memory, UTF-8 in SharedPreferences XML).
- Custom button Lua: UTF-8.

---

## 5. Generated Lua (format)

`custombuttons.lua` (see `p1-topic-custom-buttons.md` section 7 for the exact template). Starts with a `package.path` setup that appends `filesDir/mpv/scripts/?.lua` and `?/init.lua`, then `local aniyomi = require 'aniyomi'`, then one block per button. Written with `trimIndent` (common leading whitespace removed from the template only).

---

## 6. Network endpoints

**None are made by this area's code.** Checked:
- Settings screens perform no HTTP requests.
- The help button opens `https://aniyomi.org/docs/guides/player-settings/custom-buttons` through `uriHandler.openUri` (`PlayerSettingsCustomButtonScreen.kt:44`). Method GET, no auth, no parameters, no response processing.
- TorrServer HTTP API: the service calls `api.echo()`, `api.setPort(port)` on the local server (`TorrentServerService.kt`). Endpoints, paths and response shapes are owned by area:13 and are not reproduced here (not verified in this area).
- Legacy AniSkip requests are made by the player (area:45), not from these screens.

---

## 7. Database tables touched

| Table | Touched by | Operations |
|---|---|---|
| `custom_buttons` | `CustomButtonRepositoryImpl`, `CustomButtonBackupCreator`, `CustomButtonRestorer` | select all (ordered), insert, update (partial), delete, `last_insert_rowid` |

No other tables are read or written by this area.

---

## 8. Backup format (protobuf, owned by area:21/22)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupCustomButtons.kt` (`@Serializable`, kotlinx protobuf):

| Field number | Kotlin name | Type | Source column |
|---|---|---|---|
| 1 | `name` | String | `name` |
| 2 | `isFavorite` | Boolean | `isFavorite` |
| 3 | `sortIndex` | Long | `sortIndex` |
| 4 | `content` | String | `content` |
| 5 | `longPressContent` | String | `longPressContent` |
| 6 | `onStartup` | String | `onStartup` |

Containers:
- `LegacyBackup` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/Backup.kt`, class starting at line 7): `@ProtoNumber(109) var backupCustomButton: List<BackupCustomButtons> = emptyList()` (line 22).
- `Backup` (class starting at line 45): `@ProtoNumber(506) var backupCustomButton: List<BackupCustomButtons> = emptyList()` (line 61).

Restore options: `RestoreOptions.kt` and `BackupOptions.kt` carry a `customButtons` flag; restore runs only when set (`BackupRestorer.kt:103, 133-134, 277`).

---

## 9. Defaults at a glance (for first-run tests)

- Player: progress 0.85; orientation SensorLandscape; double tap left Seek, centre PlayPause, right Seek; media previous Switch, play/pause PlayPause, next Switch; intro length 85 s; skip length 10 s; hide-controls time 4000 ms; panel opacity 60; intro skip on; AniSkip off; PiP on (Android); external player "" (none).
- Decoder: hwdec on (`auto`); gpu-next off; debanding None; YUV420P on.
- Audio: no languages; pitch correction on; channels AutoSafe (mpv `auto`); boost cap 30 (mpv `volume-max` 130).
- Subtitles: no languages, no whitelist, no blacklist.
- Advanced: user files off; mpv.conf and input.conf empty.
- Torrent: off; port 8090; 26 default trackers; proxy None; proxy URL empty.
- UI: theme SYSTEM; app theme DEFAULT on desktop (MONET on Android 12+); AMOLED off; relative time on; date format localised short; start screen Anime; nav style MOVE_HISTORY_TO_MORE.
- Custom buttons: one row (`+85 s`, favourite, sortIndex 0).

---

## 10. mpv properties written by this area's settings (for the Linux port)

| Source | Android call | Linux port value |
|---|---|---|
| audio channels | `setPropertyString(property, value)` (`PlayerActivity.setupPlayerAudio`) | same property/value pairs (fix Auto/AutoSafe mapping) |
| language | `setOptionString("alang", csv)` | same |
| audio delay | `setOptionString("audio-delay", ms / 1000.0)` | same |
| pitch | `setOptionString("audio-pitch-correction", "true"/"false")` | same |
| volume cap | `setOptionString("volume-max", cap + 100)` | same |
| hwdec | `setOptionString("hwdec", "auto"/"no")` | `auto` or `no`, plus optional `vaapi`/`nvdec` choice (proposal, `p1-rust-design.md`) |
| vo | `setVo("gpu"/"gpu-next")` | same |
| debanding | `vf=gradfun=radius=12` or `deband=yes` | build vf chain (fix overwrite) |
| YUV420P | `vf=format=yuv420p` | append to vf chain |
| sub fonts | `setPropertyString("sub-fonts-dir", path)`, `osd-fonts-dir` | same |
| sub margins | `sub-ass-force-margins=yes`, `sub-use-margins=yes` | same |
| log level | `msg-level=all=warn|v` | same |
| config dir | `initialize(configDir = filesDir/mpv, cacheDir, logLvl = info|warn)` | `config-dir` pointing at the runtime dir |
| profile | `setOptionString("profile", "fast")` | same |
| keep open | `setPropertyBoolean("keep-open", true)` | same |
| default bindings | `setPropertyBoolean("input-default-bindings", true)` | same |

Scripts loaded: `load-script <filesDir/mpv/scripts/custombuttons.lua>`.
