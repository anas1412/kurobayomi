# Preference keys, chunk 4 of 8

This chunk covers the 46 items returned by `python3 /home/blackbox/kurobayomi/plans/_tools/inventory.py items prefs 4 8`, in the extractor's alphabetical order from `pip_replace_with_previous` (first) to `pref_dual_page_invert_webtoon` (last). Every key was traced to its definition in `/home/blackbox/aniyomi-src` and to every read site. All paths are repo-relative to that directory.

All 46 items are real preference keys or legacy keys. Three of them exist only inside migrations and are not read by any live code: `pref_deband`, `pref_default_viewer_key`, `pref_download_only`. One more, `player_volume_value`, is defined but never read or written by any code (dead). Their rows say so. One item, `pref_downloaded_only`, is the only app-state key in the chunk. No key here is `private`.

Related areas: area:02-preferences-system (owner of the final config schema), area:11-app-migrations, area:21-backup-format-and-create, area:22-backup-restore, area:26-library-update-jobs, area:27-tracking-core, area:24-downloads-anime, area:37-ui-library, area:40-ui-browse-manga, area:41-ui-browse-anime, area:43-reader-core, area:44-reader-viewers, area:45-player-core, area:46-player-controls-ui, area:47-settings-screens-general, area:48-settings-player-and-scripts.

## 0. Conventions

### 0.1 OBSERVED: how the Kotlin store behaves (same facts as the neighbouring chunks, repeated so this file stands alone)

| Fact | Source |
|---|---|
| One Android `SharedPreferences` file (`PreferenceManager.getDefaultSharedPreferences`) holds every key. | `core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt` |
| Typed accessors `getString`, `getLong`, `getInt`, `getFloat`, `getBoolean`, `getStringSet`, `getObject(key, default, serializer, deserializer)`; `getEnum` is an extension that stores `enum.name` as a string and falls back to the default on `IllegalArgumentException` (unknown name). | `core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt` |
| A stored value of the wrong primitive type throws `ClassCastException`; `get()` then deletes the key and returns the default. | `core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreference.kt` |
| `deleteAndGet()` (used by the player's "Reset" buttons) deletes the stored key and returns the default. | `core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt` |
| Store "app-state": stored key is `__APP_STATE_` + key (`Preference.appStateKey`); backups skip it. Store "normal": plain key, included in backups as `IntPreferenceValue`, `FloatPreferenceValue`, `StringPreferenceValue`, `BooleanPreferenceValue`, `StringSetPreferenceValue`. | `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt:60-83` |

### 0.2 Proposal: Rust storage used by the "Rust field" column

Same layout as area:02-preferences-system and chunk 3: `config.toml` (`$XDG_CONFIG_HOME/kurobayomi/config.toml`, tables `[library]`, `[sources]`, `[reader]`, `[player]`, `[tracking]`, `[downloads]`) holds user preferences and is included in backups; `state.toml` (`$XDG_STATE_HOME/kurobayomi/state.toml`, table `[state]`) holds what Kotlin calls app-state and is never backed up. Both are parsed with serde, `#[serde(default)]` on every struct (a missing key yields the default, an unknown key is ignored, never an error), and written atomically (temp file in the same directory, `fsync`, `rename`). Enums are stored as the exact Kotlin constant name so an Aniyomi backup value imports by name; an unknown name yields the default (never an error). Integer-coded enums (reader flags) are stored as the same integer.

Importer rule for every `Int`/`Float` field below: out-of-range values are clamped to the documented range, never rejected.

### 0.3 Path legend (basename used in the "Read / used" columns, expanded here)

| Short name | Full repo-relative path |
|---|---|
| `PlayerPreferences.kt`, `AudioPreferences.kt`, `SubtitlePreferences.kt`, `GesturePreferences.kt`, `DecoderPreferences.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/<name>` |
| `PlayerActivity.kt`, `PlayerViewModel.kt`, `AniyomiMPVView.kt`, `PipActions.kt`, `PlayerEnums.kt`, `ExternalIntents.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/<name>` |
| `PlayerControls.kt`, `GestureHandler.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/<name>` |
| `VerticalSliders.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/VerticalSliders.kt` |
| `MoreSheet.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt` |
| `AudioDelayPanel.kt`, `SubtitleSettingsTypographyCard.kt`, `SubtitleSettingsColorsCard.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/<name>` |
| `TrackSelect.kt`, `AniSkipApi.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/<name>` |
| `PlayerSettingsPlayerScreen.kt`, `PlayerSettingsAudioScreen.kt`, `PlayerSettingsGesturesScreen.kt` | `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/<name>` |
| `SettingsReaderScreen.kt`, `SettingsLibraryScreen.kt`, `SettingsAppearanceScreen.kt`, `SettingsTrackingScreen.kt`, `SettingsDownloadScreen.kt`, `SettingsAdvancedScreen.kt` | `app/src/main/java/eu/kanade/presentation/more/settings/screen/<name>` |
| `ReaderPreferences.kt`, `ReaderOrientation.kt`, `ReadingMode.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/<name>` |
| `ReaderActivity.kt`, `ReaderViewModel.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/<name>` |
| `ViewerConfig.kt`, `ReaderPageImageView.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/<name>` |
| `PagerConfig.kt`, `PagerPageHolder.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/<name>` |
| `WebtoonConfig.kt`, `WebtoonPageHolder.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/<name>` |
| `ColorFilterPage.kt` | `app/src/main/java/eu/kanade/presentation/reader/settings/ColorFilterPage.kt` |
| `ReaderContentOverlay.kt` | `app/src/main/java/eu/kanade/presentation/reader/ReaderContentOverlay.kt` |
| `BasePreferences.kt` | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt` |
| `UiPreferences.kt` | `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt` |
| `SourcePreferences.kt` | `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt` |
| `TrackPreferences.kt` | `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt` |
| `LibraryPreferences.kt` | `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt` |
| `DownloadPreferences.kt` | `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt` |
| `EnumsMigration.kt`, `NoAppStateMigration.kt`, `MigrateRotationViewerValuesMigration.kt`, `VideoOrientationMigration.kt`, `MovePlayerPreferencesMigration.kt`, `MigrationUtils.kt` | `app/src/main/java/mihon/core/migration/migrations/<name>` |

Decision values: Keep, Keep (changed semantics), Keep (state, not config), Replace, Drop. Priority tags (P0..P3) follow the task rules and are repeated in the notes.

## 1. Enum and flag reference used by several rows

### 1.1 `PlayerOrientation` (`PlayerEnums.kt:35-44`), stored as `.name`
`Free`, `Video`, `Portrait`, `ReversePortrait`, `SensorPortrait`, `Landscape`, `ReverseLandscape`, `SensorLandscape`.

### 1.2 `SingleActionGesture` (`PlayerEnums.kt:58-64`), stored as `.name`
`None`, `Seek`, `PlayPause`, `Switch`, `Custom`. Labels: "None", "Seek", "Play/Pause", "Change episode", "Custom" (`single_action_*` in `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml:128-132`).

`CustomKeyCodes` (`PlayerEnums.kt:69-76`): `DoubleTapLeft` = `"0x10001"`, `DoubleTapCenter` = `"0x10002"`, `DoubleTapRight` = `"0x10003"`, `MediaPrevious` = `"0x10004"`, `MediaPlay` = `"0x10005"`, `MediaNext` = `"0x10006"`. `Custom` sends `MPVLib.command(arrayOf("keypress", <code>))`.

### 1.3 `ReadingMode` flag values (`ReadingMode.kt`), `MASK = 0x00000007`
`DEFAULT` 0x0, `LEFT_TO_RIGHT` 0x1, `RIGHT_TO_LEFT` 0x2, `VERTICAL` 0x3, `WEBTOON` 0x4, `CONTINUOUS_VERTICAL` 0x5. `fromPreference(int)` returns the entry whose `flagValue` equals the int, else `DEFAULT`.

### 1.4 `ReaderOrientation` flag values (`ReaderOrientation.kt`), `MASK = 0x00000038`
`DEFAULT` 0x00, `FREE` 0x08, `PORTRAIT` 0x10, `LANDSCAPE` 0x18, `LOCKED_PORTRAIT` 0x20, `LOCKED_LANDSCAPE` 0x28, `REVERSE_PORTRAIT` 0x30. `fromPreference(int)` returns the matching entry else `DEFAULT`. The per-title override lives in `manga.viewer_flags` (`viewerFlags and MASK`); the preference only supplies the default for titles whose override is `DEFAULT` (`ReaderViewModel.kt:667-676`, `706-715`).

### 1.5 `LibraryDisplayMode` serialization (`domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt`)
`CompactGrid` is `"COMPACT_GRID"`, `ComfortableGrid` is `"COMFORTABLE_GRID"`, `CoverOnlyGrid` is `"COVER_ONLY_GRID"`, `List` is `"LIST"`. Any other string deserializes to the default `CompactGrid`.

### 1.6 `AudioChannels` (`AudioPreferences.kt:107-113`), stored as `.name`

| Enum name | Label (string key, English) | mpv property | mpv value |
|---|---|---|---|
| `Auto` | `pref_player_audio_channels_auto` "Auto" | `audio-channels` | `auto-safe` |
| `AutoSafe` | `pref_player_audio_channels_auto_safe` "Auto-safe" | `audio-channels` | `auto` |
| `Mono` | `pref_player_audio_channels_mono` "Mono" | `audio-channels` | `mono` |
| `Stereo` | `pref_player_audio_channels_stereo` "Stereo" | `audio-channels` | `stereo` |
| `ReverseStereo` | `pref_player_audio_channels_reverse_stereo` "Reverse stereo" | `af` | `pan=[stereo\|c0=c1\|c1=c0]` |

The `Auto` and `AutoSafe` rows are swapped relative to their labels: the entry labelled "Auto" sends `auto-safe` and the entry labelled "Auto-safe" sends `auto`. The default entry `AutoSafe` therefore sends mpv `audio-channels=auto`.

### 1.7 `SubtitlesBorderStyle` (`SubtitleSettingsTypographyCard.kt:368-375`), stored as `.name`
`OutlineAndShadow` = mpv `"outline-and-shadow"`, `OpaqueBox` = `"opaque-box"`, `BackgroundBox` = `"background-box"`.

### 1.8 `AutoTrackState` (`app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt:6-10`), stored as `.name`
`ALWAYS` (label `lock_always` "Always"), `ASK` (label `default_category_summary` "Always ask"), `NEVER` (label `lock_never` "Never").

### 1.9 `ChapterSwipeAction` (`LibraryPreferences.kt:391-396`), stored as `.name`
`ToggleRead`, `ToggleBookmark`, `Download`, `Disabled`. Settings list order and labels: `Disabled` "Disabled" (`MR.strings.disabled`), `ToggleBookmark` "Bookmark" (`action_bookmark`), `ToggleRead` "Mark as read" (`action_mark_as_read`), `Download` "Download" (`action_download`).

---

## 2. Player: `PlayerPreferences.kt`

| Key | Store | Kotlin type | Default | Values | Defined | Read / used | Behaviour | UI label | Rust field | Decision |
|---|---|---|---|---|---|---|---|---|---|---|
| `pip_replace_with_previous` | normal | `Boolean` | `false` | `true`/`false` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt:63` | `PlayerActivity.kt:783`; `PipActions.kt:32-80`; `PlayerSettingsPlayerScreen.kt:266,287-291` | In the Android PiP window, the first remote action becomes "Previous episode" (disabled when `playlistPosition == 0`) instead of "Skip 10 seconds" (`PIP_SKIP`). | `pref_pip_replace_with_previous` = `Replaces the "Skip 10 seconds" option with "Previous episode"`; group `pref_category_pip`; enabled only while `pref_enable_pip` is on; whole group shown only if `deviceHasPip()` | none | Drop |
| `player_brightness_value` | normal | `Float` | `-1.0F` | `-1.0` = unset, else window brightness 0.0..1.0 | `PlayerPreferences.kt:27` | `PlayerActivity.kt:342-349` (write in `onStop`), `PlayerActivity.kt:405-408` (read in `onStart`) | Companion value of `pref_remember_brightness`. In `onStop`, if `rememberPlayerBrightness()` is true and `window.attributes.screenBrightness != -1f`, stores it. In `onStart`, if the remember flag is true and the stored value `!= -1f`, calls `viewModel.changeBrightnessTo(value)` which clamps the overlay state to `-0.75..1` and the window attribute to `0..1` (`PlayerViewModel.kt:752-758`). | none (no row; state saved as a side effect) | none | Drop |
| `player_fullscreen` | normal | `Boolean` | `true` | `true`/`false` | `PlayerPreferences.kt:38` | `PlayerActivity.kt:397-402`; `PlayerSettingsPlayerScreen.kt:143,157-161` | Despite the name it is the display-cutout switch: when true the window uses `LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES`, otherwise `LAYOUT_IN_DISPLAY_CUTOUT_MODE_NEVER`; applied only on API 28 and above. The player is always immersive regardless of this key. | `pref_player_fullscreen` = "Show content in display cutout"; group "Display"; the switch is disabled below Android 9 | none | Drop |
| `player_hide_controls` | normal | `Boolean` | `false` | `true`/`false` | `PlayerPreferences.kt:39` | `PlayerViewModel.kt:276-278`; `PlayerSettingsPlayerScreen.kt:144,162-165` | Initial state of the on-screen controls and the seek bar: `_controlsShown = !hideControls`, `_seekBarShown = !hideControls`. Not re-read while a video is playing. (The identically named `PlayerViewModel.hideControls()` method seen in `GestureHandler.kt:125` and `PlayerControls.kt:142` is unrelated.) | `pref_player_hide_controls` = "Hide player controls when opening the player"; group "Display" | `player.hide_controls_on_open: bool = false` | Keep (P2) |
| `player_volume_value` | normal | `Float` | `-1.0F` | `-1.0` = unset | `PlayerPreferences.kt:29` | none: a repo-wide search finds no other reference to `playerVolumeValue()` | Dead. Companion of `pref_remember_volume`, whose switch exists (`PlayerSettingsPlayerScreen.kt:91,113-116`) but nothing saves or restores a volume. | none | `player.volume_value: f32 = -1.0` (same field as chunk 6, key `pref_remember_volume`) | Keep (changed semantics), implement (P2) |
| `pref_allow_gestures_in_panels` | normal | `Boolean` | `false` | `true`/`false` | `PlayerPreferences.kt:23` | `GestureHandler.kt:87,142`; `PlayerSettingsPlayerScreen.kt:87,96-99` | On every press, `if (panelShown != Panels.None && !allowGesturesInPanels) viewModel.panelShown.update { Panels.None }`: with the flag false, touching the video closes an open panel (subtitle settings, subtitle delay, audio delay, video filters); with true the gesture proceeds while the panel stays open. | `pref_controls_allow_gestures_in_panels` = "Allow gesture in panels"; group "Controls" | `player.allow_gestures_in_panels: bool = false` | Keep (P2) |
| `pref_always_use_external_player` | normal | `Boolean` | `false` | `true`/`false` | `PlayerPreferences.kt:67-70` | `AnimeScreenModel.kt:176` and `AnimeScreen.kt:134,141,178,219` (entry page); `AnimeLibraryTab.kt:116`; `AnimeHistoryTab.kt:59`; `AnimeUpdatesTab.kt:65`; `AnimeUpdatesScreen.kt:156,159`; `EntryBottomActionMenu.kt:222,231`; `PlayerSettingsPlayerScreen.kt:301,319-322` (all under `app/src/main/java/eu/kanade/...`) | Chooses the player when an episode is opened: `extPlayer = alwaysUseExternalPlayer.get()`; where an alternate action exists (entry page and updates), `extPlayer = alwaysUseExternalPlayer.get() != altPlayer` (XOR). The multi-select bottom bar shows the "external player" button only when the flag is false and the "internal player" button only when it is true, and only for a single selected episode. | `pref_always_use_external_player` = "Always use external player"; group `pref_category_external_player` | `player.always_use_external: bool = false` | Keep (changed semantics) (P2) |
| `pref_auto_play_enabled` | normal | `Boolean` | `false` | `true`/`false` | `PlayerPreferences.kt:85` (under the comment `// Old`) | `PlayerActivity.kt:1408-1411`; `PlayerControls.kt:444`; `PlayerViewModel.kt:802-810` | Auto-play next episode: on mpv `end-file` with `eofReached == true`, `viewModel.changeEpisode(previous = false, autoPlay = true)`. With no next episode the call toasts `no_next_episode` = "Next Episode not found!" and returns. Toggling shows the overlay text `enable_auto_play` "Auto-play is on" or `disable_auto_play` "Auto-play is off" for 2000 ms. | no settings row; toggle button in the top-right controls of the player (`TopRightPlayerControls`) | `player.autoplay_next: bool = false` | Keep (P1) |
| `pref_default_player_orientation_type_key` | normal | enum `PlayerOrientation`, stored as `.name` | `SensorLandscape` | `Free`, `Video`, `Portrait`, `ReversePortrait`, `SensorPortrait`, `Landscape`, `ReverseLandscape`, `SensorLandscape` | `PlayerPreferences.kt:16-19` | `PlayerActivity.kt:835-851`; `PlayerViewModel.kt:839-855`; `PlayerSettingsPlayerScreen.kt:66-72`; `VideoOrientationMigration.kt`; `MovePlayerPreferencesMigration.kt` | Requested Android screen orientation when a file is loaded. See the mapping table below the section. The rotate button (`cycleScreenRotations`) also writes it: from any landscape orientation to `SensorPortrait`, otherwise to `SensorLandscape`. | `pref_category_player_orientation` = "Orientation"; entries `rotation_free` "Free", `rotation_video` "Video", `rotation_portrait` "Portrait", `rotation_reverse_portrait` "Reverse portrait", `rotation_sensor_portrait` "Sensor portrait", `rotation_landscape` "Landscape", `rotation_reverse_landscape`, `rotation_sensor_landscape` "Sensor landscape" | none | Drop |
| `pref_disabled_ani_skip_chapters` | normal | `Boolean` | `true` | `true`/`false` | `PlayerPreferences.kt:56` | `PlayerActivity.kt:1290-1307`; `PlayerSettingsPlayerScreen.kt:210,248-252` | When AniSkip is enabled the skip times are fetched and merged into the chapter list unless `disableAniSkipOnChapters && viewModel.chapters.value.isNotEmpty()`, i.e. with the flag true AniSkip is skipped for videos that already carry chapters. Evaluated in `fileLoaded()`. | `pref_disable_aniskip_chapter` = "Disable AniSkip if video already contains chapters"; enabled only if both `pref_enable_skip_intro` and `pref_enable_ani_skip` are on | `player.skip_intro.disable_aniskip_on_chapters: bool = true` | Keep (P2) |
| `pref_display_vol_as_per` | normal | `Boolean` | `true` | `true`/`false` | `PlayerPreferences.kt:40` | `PlayerControls.kt:281,287`; `VerticalSliders.kt:182-205,238-246`; `PlayerSettingsPlayerScreen.kt:145,166-169` | Volume slider shows `NumberFormat.getPercentInstance()` of `percentage(volume, range)` when true, else the raw Android stream step (integer from `NumberFormat.getIntegerInstance()`). The slider's value and range switch between `0..100` and `0..maxVolume` accordingly. The boost label is `+n` (for `0 < boost <= 1000`), `-n` (for `-100 <= boost < 0`), otherwise the plain integer; empty when `0`. | `pref_controls_display_volume_percentage` = "Display volume as percentage"; group "Display" | none (always percent) | Replace |

Mapping used by `setupPlayerOrientation` (`PlayerActivity.kt:835-851`), OBSERVED:

| `PlayerOrientation` | Android constant requested |
|---|---|
| `Free` | `SCREEN_ORIENTATION_SENSOR` |
| `Video` | `SCREEN_ORIENTATION_SENSOR_LANDSCAPE` if `player.getVideoOutAspect() > 1.0`, else `SCREEN_ORIENTATION_SENSOR_PORTRAIT` |
| `Portrait` | `SCREEN_ORIENTATION_PORTRAIT` |
| `ReversePortrait` | `SCREEN_ORIENTATION_REVERSE_PORTRAIT` |
| `SensorPortrait` | `SCREEN_ORIENTATION_SENSOR_PORTRAIT` |
| `Landscape` | `SCREEN_ORIENTATION_LANDSCAPE` |
| `ReverseLandscape` | `SCREEN_ORIENTATION_REVERSE_PORTRAIT` (looks like a bug: the reverse-landscape entry requests reverse portrait) |
| `SensorLandscape` | `SCREEN_ORIENTATION_SENSOR_LANDSCAPE` |

### Notes for section 2

#### `pip_replace_with_previous`
- Decision: Drop. Reason: Android PiP (`enterPictureInPictureMode`, `PictureInPictureParams`) has no GTK4 equivalent a client can request; chunk 5 reaches the same decision for `pref_enable_pip`, `pref_pip_episode_toasts`, `pref_pip_on_exit`. The user loses the shrinking PiP window and its Previous-episode button. Replacement: the window manager's "Keep above others" action (KDE Plasma: window menu, More actions, Keep above others) and the player's own previous/next buttons (area:46-player-controls-ui). The importer reads the key and discards it without a warning.

#### `player_brightness_value`
- Decision: Drop (P3), consistent with chunk 6 (`pref_remember_brightness`). A GTK window cannot change the backlight on Wayland or X11 without desktop-specific D-Bus. The user loses restoring a gesture-set brightness. Replacement: mpv's `brightness` property (video filter, range -100..100) if a brightness gesture is built (area:45-player-core). The importer discards the key.

#### `player_fullscreen`
- Decision: Drop. Reason: desktop windows have no display cutouts. What the user loses: nothing on desktop. The label "Show content in display cutout" is not a fullscreen switch; the Rust player's fullscreen toggle (`F` key, double-click) is a runtime action in area:46-player-controls-ui and is not persisted in this key. The importer discards the key.

#### `player_hide_controls`
- Proposal (area:46-player-controls-ui). When opening the player, set `controls_visible = !hide_controls_on_open` and `seekbar_visible = !hide_controls_on_open`. Moving the mouse or any key press shows the controls (the Kotlin equivalent is a tap). Backup import: key `player_hide_controls` (Boolean) maps 1:1.

#### `player_volume_value`
- OBSERVED: dead, see the row. Proposal: implement the missing behaviour as `player.remember_volume: bool = false` plus `player.volume_value: f32 = -1.0` (chunk 6). On player teardown, if `remember_volume` and the mpv `volume` property (0..=100+boost cap) is known, store it; on file load, if the stored value is not `-1.0`, set mpv `volume` to it, clamped to `0.0..=(100 + audio.volume_boost_cap)`. The importer keeps the stored float untouched.

#### `pref_allow_gestures_in_panels`
- Proposal (area:46-player-controls-ui). Click/drag on the video surface while a side panel is open closes the panel unless `allow_gestures_in_panels` is true. Backup import: Boolean, 1:1.

#### `pref_always_use_external_player`
- OBSERVED: every call site computes the launch choice as above; the external launch itself (package, intent extras, result handling) is in `ExternalIntents.kt` and is the subject of the `external_player_preference` row in chunk 2.
- Proposal (area:45-player-core). External players on Linux are launched as a child process: `player.external_player_command: String` (chunk 2) holds the command template; this flag selects it. The XOR with the "alternate" action is kept so the entry page can still offer the other player on one click. If the command string is empty, the flag is ignored and the internal libmpv player opens (never fail to open an episode because of a missing external command). What the user loses: Android intent hand-off with resume position returned to the app; the Rust app can only mark an episode seen from the internal player.

#### `pref_auto_play_enabled`
- OBSERVED: no settings row. The source marks it "Old", yet it is the only auto-play switch and is used. Auto-play happens only on natural end of file (`eofReached`), not on a stop or an error.
- Proposal (area:45-player-core, area:46-player-controls-ui). Connect libmpv `MPV_EVENT_END_FILE` with `reason == MPV_END_FILE_REASON_EOF`; if `autoplay_next` and a next episode exists, load it and start playing (the new file starts unpaused, `autoPlay = true`); otherwise show the toast text `no_next_episode`. Provide the toggle in the top-right controls with the two overlay texts above. Backup import: Boolean, 1:1.

#### `pref_default_player_orientation_type_key`
- Migration history (OBSERVED). `MovePlayerPreferencesMigration` (version 93) re-saved a String-typed value as Int when it parsed. `VideoOrientationMigration` (version 127) converted the old Int `ActivityInfo` constant to the enum name: `SCREEN_ORIENTATION_FULL_SENSOR` (10) to `Free`; `PORTRAIT` (1) to `Portrait`; `REVERSE_PORTRAIT` (9) to `ReversePortrait`; `LANDSCAPE` (0) to `Landscape`; `REVERSE_LANDSCAPE` (8) to `ReverseLandscape`; `SENSOR_PORTRAIT` (7) to `SensorPortrait`; `SENSOR_LANDSCAPE` (6) to `SensorLandscape`; anything else to `Free`; an Int read that throws `ClassCastException` (already an enum string) just removes the old value and returns true. A stored value that is an enum name falls through `getEnum` normally.
- Decision: Drop. Reason: a desktop window has no orientation lock. What the user loses: forced landscape/portrait (irrelevant on a desktop monitor). The importer discards the key and the sibling rotate-button behaviour.

#### `pref_disabled_ani_skip_chapters`
- OBSERVED: AniSkip fetches `https://api.aniskip.com/v2/skip-times/$malId/$episodeNumber?types[]=ed&types[]=mixed-ed&types[]=mixed-op&types[]=op&types[]=recap&episodeLength=$episodeLength` (`AniSkipApi.kt:22-25`). Chapters already present in the container count as "video already contains chapters".
- Proposal (area:45-player-core). Query mpv's `chapter-list/count` after `file-loaded`; skip the AniSkip request when the flag is true and the count is greater than 0. Importer: Boolean, 1:1.

#### `pref_display_vol_as_per`
- Decision: Replace with "always percent". Reason: the integer alternative is the Android `STREAM_MUSIC` step count (0..`getStreamMaxVolume`), which does not exist on desktop; libmpv's `volume` is already a percentage. What the user loses: the toggle. The importer discards the key. The boost label rule above (`+n`/`-n`) is kept for values above 100%.

---

## 3. Audio: `AudioPreferences.kt`

| Key | Store | Kotlin type | Default | Values | Defined | Read / used | Behaviour | UI label | Rust field | Decision |
|---|---|---|---|---|---|---|---|---|---|---|
| `pref_audio_config` | normal | enum `AudioChannels`, stored as `.name` | `AutoSafe` | `Auto`, `AutoSafe`, `Mono`, `Stereo`, `ReverseStereo` (table in 1.6) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt:13` | `PlayerActivity.kt:585-587` (`setupPlayerAudio`); `MoreSheet.kt:241-258`; `PlayerSettingsAudioScreen.kt:32,83-89` | At load: `MPVLib.setPropertyString(entry.property, entry.value)`. When the user selects an entry in the player's "More" sheet: if `ReverseStereo`, first set `audio-channels` to the `AutoSafe` value (`auto`); otherwise first clear `af` with `""`; then set the entry's own property. | Settings: `pref_player_audio_channels` = "Audio channels" (list); player More sheet: `pref_audio_channels` = "Audio channels" (filter chips) | `player.audio.channels: AudioChannels = AutoSafe` | Keep (changed semantics) (P2) |
| `pref_audio_delay` | normal | `Int` (milliseconds) | `0` | any integer, negative allowed | `AudioPreferences.kt:18` | `AniyomiMPVView.kt:256`; `AudioDelayPanel.kt:64-71` | At player init: `MPVLib.setOptionString("audio-delay", (delay / 1000.0).toString())`. The panel edits the live mpv property `audio-delay` in seconds (`delay / 1000.0`) on every change and persists the value only when the user presses Apply; Reset sets the in-panel value to 0 without saving. | no settings row; player panel "Audio delay" (`player_sheets_audio_delay_title`) | `player.audio.delay_ms: i32 = 0` | Keep (P2) |
| `pref_audio_lang` | normal | `String` | `""` | comma-separated language codes, two- or three-letter | `AudioPreferences.kt:11` | `AniyomiMPVView.kt:255`; `TrackSelect.kt:16-60`; `PlayerViewModel.kt:462-465`; `PlayerSettingsAudioScreen.kt:30,40-77`; `PrefLangMigration.kt` (`app/src/main/java/mihon/core/migration/migrations/PrefLangMigration.kt:24`) | Passed verbatim as mpv option `alang`, and parsed by `TrackSelect` to pick the audio track after load. See notes. | `pref_player_audio_lang` = "Preferred languages"; dialog text `pref_player_audio_lang_info`; error `pref_player_subtitle_invalid_lang` = `Invalid code: "%s"` | `player.audio.preferred_languages: String = ""` | Keep (P1) |
| `pref_audio_pitch_correction` | normal | `Boolean` | `true` | `true`/`false` | `AudioPreferences.kt:12` | `AniyomiMPVView.kt:257`; `PlaybackSpeedSheet.kt:135-139`; `PlayerSettingsAudioScreen.kt:31,78-82` | mpv option `audio-pitch-correction` = `"true"`/`"false"` at init; the speed sheet toggle writes the preference. | `pref_player_audio_pitch_correction` = "Enable audio pitch correction", summary `pref_player_audio_pitch_correction_summary` | `player.audio.pitch_correction: bool = true` | Keep (P2) |
| `pref_audio_volume_boost_cap` | normal | `Int` | `30` | slider `0..200` | `AudioPreferences.kt:14` | `AniyomiMPVView.kt:258`; `PlayerControls.kt:280-286`; `GestureHandler.kt:114`; `PlayerViewModel.kt:762-777`; `PlayerSettingsAudioScreen.kt:33,92-100` | mpv option `volume-max = cap + 100`. In the volume gesture logic, boost applies only once the Android stream volume is at its maximum (`currentVolume == maxVolume`) and `cap > 0`; boost range shown on the slider is `0..cap` when `cap > 0`, absent otherwise. | `pref_player_audio_boost_cap` = "Volume boost cap"; subtitle shows the integer | `player.audio.volume_boost_cap: u8 = 30` (range 0..=200) | Keep (changed semantics) (P2) |

### Notes for section 3

#### `pref_audio_config`
- OBSERVED: see the label/value swap in 1.6. Selecting the "Auto" chip sends `auto-safe`; selecting "Auto-safe" sends `auto`.
- Proposal (area:48-settings-player-and-scripts). Keep the five serde names unchanged so imports parse, but fix the mapping: `Auto` sends `auto`, `AutoSafe` sends `auto-safe` (mpv's own default, the safe one on PipeWire/PulseAudio), `Mono` sends `mono`, `Stereo` sends `stereo`, `ReverseStereo` sets property `af` to `pan=[stereo|c0=c1|c1=c0]` after setting `audio-channels` to `auto-safe`. Effect for an imported default: an Aniyomi user who never changed it gets mpv `auto-safe` instead of `auto`, which is the safer choice. Selecting a non-reverse entry clears `af` first, as in Kotlin (only the `pan=` filter this app sets; other user filters from `mpv.conf` are not touched: clear via `af-remove` of that single filter label instead of `af ""`). Apply with libmpv `mpv_set_property_string`.

#### `pref_audio_delay`
- Proposal. Keep the live-apply, save-on-Apply flow in the audio delay panel (area:46-player-controls-ui). Apply the stored value at player init through the libmpv option `audio-delay` (seconds, `ms / 1000.0`).

#### `pref_audio_lang`
- OBSERVED validation (`PlayerSettingsAudioScreen.kt:36-76`): split on `,`, drop empty tokens, trim each; for each token build `Locale(token)`; the token is invalid when `locale.isO3Language == locale.language && locale.language == locale.getDisplayName(Locale.ENGLISH)`, or when `isO3Language` throws `MissingResourceException`; the dialog shows the first invalid token with `pref_player_subtitle_invalid_lang`.
- OBSERVED selection (`TrackSelect.getPreferredTrackIndex(tracks, subtitle = false)`, `TrackSelect.kt:16-60`): token list from `prefLangs`; an empty list means the system default locale; the first locale for which any track matches wins (`containsLang`: track name contains the locale's localized display name or its English display name, case-insensitive, or the track `language` field matches regex `\b<iso3>|<language>\b` case-insensitive); the first track in the chosen-locale list is returned. `PlayerViewModel.onFinishLoadingTracks()` falls back to `audioTracks.getOrNull(1)` when nothing matches (index 0 is the "none" placeholder).
- Proposal (area:45-player-core). Pass the setting to libmpv as option `alang` (mpv supports the same comma list and matches 2- and 3-letter codes itself), and do not re-implement `containsLang`; fall back to mpv's default track when `alang` yields nothing. Validate in the settings dialog with an ISO 639 table (crate `isolang`): accept 2-letter (ISO 639-1) and 3-letter (ISO 639-2/3) codes; show `Invalid code: "<token>"` for the first token that is not in the table. Importer: String, 1:1.

#### `pref_audio_pitch_correction`
- Proposal. Set libmpv option/property `audio-pitch-correction` (`yes`/`no`) at init and when toggled. Importer: Boolean.

#### `pref_audio_volume_boost_cap`
- Proposal. libmpv volume is a single scale: set option `volume-max` = `100 + cap`. The slider range is `0..=100+cap`; the part above 100 is the "boost" shown as `+n`. No stream-volume stage exists, so the "boost only after the device is at maximum" rule is dropped. `cap == 0` means no boost (volume-max 100). Importer clamps to `0..=200`.

---

## 4. Subtitles: `SubtitlePreferences.kt`

Stored colour format (OBSERVED): `Int` ARGB, signed. `Color.White.toArgb()` = `0xFFFFFFFF` (`-1`), `Color.Black.toArgb()` = `0xFF000000` (`-16777216`), `Color.Transparent.toArgb()` = `0x00000000` (`0`). Applied to mpv as `"#" + Int.toHexString().uppercase()` (`SubtitleSettingsColorsCard.kt:156`): eight hex digits `#AARRGGBB`, which is mpv's colour syntax.

| Key | Store | Kotlin type | Default | Values | Defined | Read / used | Behaviour | UI label | Rust field | Decision |
|---|---|---|---|---|---|---|---|---|---|---|
| `pref_background_color_subtitles` | normal | `Int` (ARGB) | `Color.Transparent.toArgb()` = `0` | any 32-bit ARGB | `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/SubtitlePreferences.kt:42-45` | `AniyomiMPVView.kt:279-282`; `SubtitleSettingsColorsCard.kt:196,158-166` | mpv option `sub-back-color` at init; the colour picker (R, G, B, A sliders) sets the preference and the live property `sub-back-color` on every change; Reset calls `deleteAndGet()` and re-applies the default. Only visible when the border style draws a box (`OpaqueBox` / `BackgroundBox`) in mpv. | player subtitle settings panel, Colors card, chip "Background" (`player_sheets_subtitles_color_background`); buttons "Red/Green/Blue/Alpha" | `player.subtitles.background_color: u32 = 0x00000000` | Keep (P2) |
| `pref_bold_subtitles` | normal | `Boolean` | `false` | `true`/`false` | `SubtitlePreferences.kt:30` | `AniyomiMPVView.kt:275`; `SubtitleSettingsTypographyCard.kt:173,357` | mpv option `sub-bold` = `"yes"`/`"no"` at init; the toggle button writes the preference and sets the live property; Reset (`resetTypography`) deletes the key and re-applies. | Typography card, bold toggle (icon `FormatBold`, no text label) | `player.subtitles.bold: bool = false` | Keep (P2) |
| `pref_border_color_subtitles` | normal | `Int` (ARGB) | `Color.Black.toArgb()` = `-16777216` (`0xFF000000`) | any 32-bit ARGB | `SubtitlePreferences.kt:35` | `AniyomiMPVView.kt:283`; `SubtitleSettingsColorsCard.kt:189,155-157` | mpv option `sub-border-color`; same picker/reset flow as above. | Colors card, chip "Border" (`player_sheets_subtitles_color_border`) | `player.subtitles.border_color: u32 = 0xFF000000` | Keep (P2) |
| `pref_border_style_subtitles` | normal | enum `SubtitlesBorderStyle`, stored as `.name` | `OutlineAndShadow` | `OutlineAndShadow` (mpv `outline-and-shadow`), `OpaqueBox` (`opaque-box`), `BackgroundBox` (`background-box`) | `SubtitlePreferences.kt:37-40` | `AniyomiMPVView.kt:285`; `SubtitleSettingsTypographyCard.kt:310,365` | mpv option `sub-border-style` set to `entry.value`; the Typography card shows a drop-down with a check mark on the current entry. | Typography card, "Border style" (`player_sheets_sub_typography_border_style`); entries `player_sheets_subtitles_border_style_outline_and_shadow` "Outline and shadow", `..._opaque_box` "Opaque box", `..._background_box` "Background box" | `player.subtitles.border_style: SubBorderStyle = OutlineAndShadow` | Keep (P2) |

### Notes for section 4
- Proposal (area:46-player-controls-ui, area:48-settings-player-and-scripts). A single `SubtitleStyle` struct under `[player.subtitles]` with the same defaults; apply every field with `mpv_set_property_string` on load and on change. Colours are stored as `u32` ARGB in TOML (hex string `"0xAARRGGBB"` accepted on read); the importer converts a signed `Int` to `u32` by two's-complement reinterpretation (`value as u32`). GTK colour pickers use `gtk::ColorDialog` (GTK 4.10+) with alpha; format with `format!("#{:08X}", argb)` for mpv.
- "Reset" in the UI sets the field back to its default and re-applies; it never deletes the TOML key.
- The sibling keys of the same class (`pref_italic_subtitles`, `pref_text_color_subtitles`, `pref_sub_border_size`, `sub_shadow_offset`, `pref_subtitle_font`, `pref_subtitles_font_size`, `pref_sub_scale`, `pref_sub_pos`, `pref_sub_justify`, `pref_override_subtitles_ass`, `pref_subtitles_delay`, `pref_subtitles_speed`, `pref_subtitles_secondary_delay`, `pref_screenshot_subtitles`) are catalogued in other chunks.

---

## 5. Gestures: `GesturePreferences.kt`

| Key | Store | Kotlin type | Default | Values | Defined | Read / used | Behaviour | UI label | Rust field | Decision |
|---|---|---|---|---|---|---|---|---|---|---|
| `pref_center_double_tap` | normal | enum `SingleActionGesture`, stored as `.name` | `PlayPause` | settings offer `None`, `PlayPause`, `Custom`; enum also has `Seek`, `Switch` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt:28` | `PlayerViewModel.kt:1067-1080`; `GestureHandler.kt:126-140`; `PlayerSettingsGesturesScreen.kt:135,155-165` | Double tap in the middle fifth of the player (`2/5 * width <= x <= 3/5 * width`): `PlayPause` toggles pause, `Custom` sends mpv `keypress 0x10002`, `None`/`Seek`/`Switch` do nothing (so an imported `Seek` or `Switch` is a no-op). | `pref_center_double_tap` = "Double tap (center)"; group `pref_category_double_tap` = "Double tap" | `player.gestures.center_double_tap: SingleActionGesture = PlayPause` | Keep (changed semantics) (P2) |
| `pref_default_intro_length` | normal | `Int` (seconds) | `85` | wheel picker `0..255` | `GesturePreferences.kt:21` | `PlayerViewModel.kt:1982-1991`; `AnimeScreen.kt:378`; `PlayerSettingsGesturesScreen.kt:78,88,227-268` | Default length of the "skip intro" jump. `getAnimeSkipIntroLength()`: if the open anime has `skipIntroDisable` returns `0`; if `anime.skipIntroLength <= 0` returns the preference; else the per-anime value. The per-anime dialog opens with the preference when the anime has no override and is not disabled. | `pref_default_intro_length` = "Default skip intro length"; subtitle `"<n>s"`; dialog title `pref_intro_length`; wheel items `seconds_short` | `player.gestures.default_intro_length_secs: u8 = 85` | Keep (P1) |

### Notes for section 5

#### `pref_center_double_tap`
- OBSERVED zones (`GestureHandler.kt:126-140`): `x > width * 3 / 5` right, `x < width * 2 / 5` left, otherwise centre. Left/right are used for seek actions and the settings list for them is `None`, `Seek`, `PlayPause`, `Switch`, `Custom` (`Switch` on the left = previous episode, on the right = next episode); the centre list omits `Seek` and `Switch`.
- Proposal (area:46-player-controls-ui). A mouse has a native double-click on the video surface: use `gtk::GestureClick` with `n_press == 2`, same 2/5, 1/5, 2/5 zoning, same action enum. `Custom` sends the mpv command `keypress 0x10002` (the user binds it in `input.conf`; libmpv accepts the same key name). Because desktop users expect double-click for fullscreen, the Rust default for the centre zone is the Kotlin default `PlayPause` and the fullscreen toggle is bound to the `F` key and to the fullscreen button only (never to the zones). Importer: unknown name leaves `PlayPause`.

#### `pref_default_intro_length`
- Proposal (area:45-player-core, area:39-ui-entry-details-anime). Keep the picker range 0..=255 (a `gtk::SpinButton` with step 1 replaces the wheel). The skip-intro button seeks `+length` seconds relative to the current position. Importer clamps to `0..=255`.

---

## 6. Reader: `ReaderPreferences.kt` and `BasePreferences.kt`

| Key | Store | Kotlin type | Default | Values | Defined | Read / used | Behaviour | UI label | Rust field | Decision |
|---|---|---|---|---|---|---|---|---|---|---|
| `pref_always_decode_long_strip_with_ssiv` | normal | `Boolean` | `false` | `true`/`false` | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:42` | `ReaderPageImageView.kt:63-65,317`; `SettingsAdvancedScreen.kt:386` | In webtoon mode a `BufferedSource` page is normally decoded by Coil with `TachiyomiImageDecoder` into a bitmap (crop borders applied there); with the flag true, or in a paged viewer, the page goes straight to `SubsamplingScaleImageView.setImage(ImageSource.inputStream)` with `setHardwareConfig(canUseHardwareBitmap)`. It works around blank tall images on GPUs with small texture limits. | `pref_always_decode_long_strip_with_ssiv_2` = "Use legacy decoder for long strip reader"; summary "Affects performance. Only enable if reducing bitmap threshold doesn't fix blank image issues" | none | Drop |
| `pref_color_filter_key` | normal | `Boolean` | `false` | `true`/`false` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:128` | `ReaderActivity.kt:371-376,380-384`; `ColorFilterPage.kt:49-53` | Enables a full-size coloured rectangle drawn above the page: `ReaderContentOverlay(brightness, color = colorOverlay.takeIf { enabled }, colorBlendMode)`. Colour is `color_filter_value` (packed ARGB), blend mode is `color_filter_mode`. | reader sheet, tab "Custom filter": checkbox `pref_custom_color_filter` = "Custom color filter"; sliders `color_filter_r_value`, `g`, `b`, `a`; chip row `pref_color_filter_mode` | `reader.color_filter.enabled: bool = false` | Keep (P2) |
| `pref_custom_brightness_key` | normal | `Boolean` | `false` | `true`/`false` | `ReaderPreferences.kt:124` | `ReaderActivity.kt:865-867,937-946`; `ColorFilterPage.kt:25-45` | Enables the brightness override. When off the code calls `setCustomBrightnessValue(0)`, which restores system brightness (`BRIGHTNESS_OVERRIDE_NONE`) and hides the overlay; when on it follows `custom_brightness_value` sampled every 100 ms. | reader sheet, tab "Custom filter": checkbox `pref_custom_brightness` = "Custom brightness", slider range `-75..100` | `reader.custom_brightness.enabled: bool = false` | Keep (changed semantics) (P2) |
| `pref_default_orientation_type_key` | normal | `Int` (flag value) | `ReaderOrientation.FREE.flagValue` = `0x08` | `0x08`, `0x10`, `0x18`, `0x20`, `0x28`, `0x30` (settings omit `DEFAULT` 0x00) | `ReaderPreferences.kt:43-46` | `ReaderViewModel.kt:706-715`; `SettingsReaderScreen.kt:83-90`; `MigrateRotationViewerValuesMigration.kt:31` | Default rotation lock for titles whose `viewer_flags` orientation bits are `DEFAULT`. | `pref_rotation_type` = "Default rotation"; entries `rotation_free` "Free", `rotation_portrait` "Portrait", `rotation_landscape` "Landscape", `rotation_force_portrait` "Locked portrait", `rotation_force_landscape` "Locked landscape", `rotation_reverse_portrait` "Reverse portrait" | none | Drop |
| `pref_default_reading_mode_key` | normal | `Int` (flag value) | `ReadingMode.RIGHT_TO_LEFT.flagValue` = `0x02` | `0x01`, `0x02`, `0x03`, `0x04`, `0x05` (settings omit `DEFAULT` 0x00) | `ReaderPreferences.kt:38-41` | `ReaderViewModel.kt:667-676`; `SettingsReaderScreen.kt:36-42`; `MigrateRotationViewerValuesMigration.kt:33` | Reading mode for titles whose `viewer_flags` reading-mode bits are `DEFAULT`. `getMangaReadingMode(resolveDefault = true)` returns this value when the title's own mode is `DEFAULT`. | `pref_viewer_type` = "Default reading mode"; entries `left_to_right_viewer`, `right_to_left_viewer`, `vertical_viewer`, `webtoon_viewer`, `vertical_plus_viewer` | `reader.default_reading_mode: ReadingMode = RightToLeft` | Keep (P0) |
| `pref_display_profile_key` | normal | `String` (document URI) | `""` | `""` = none, else a `content://` URI of an ICC profile | `BasePreferences.kt:38` | `ReaderActivity.kt:853-855,885-913`; `SettingsAdvancedScreen.kt:350-359,388-395`; `TachiyomiImageDecoder.kt` (`app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt:24,88`) | Loads the file's bytes and passes them to `SubsamplingScaleImageView.setDisplayProfile` and to `TachiyomiImageDecoder.displayProfile`, so decoded images are colour-converted with that ICC profile. A missing or non-existent file is silently ignored. | `pref_display_profile` = "Custom display profile"; subtitle shows the stored URI; opens a document picker for `*/*` | `reader.display_icc_profile: String = ""` (absolute path) | Keep (P3) |
| `pref_double_tap_anim_speed` | normal | `Int` (milliseconds) | `500` | `1` ("No animation"), `500` ("Normal"), `250` ("Fast") | `ReaderPreferences.kt:26` | `ViewerConfig.kt:55-56`; `PagerPageHolder.kt:165`; `WebtoonPageHolder.kt:200`; `ReaderPageImageView.kt:293,396`; `SettingsReaderScreen.kt:43-52` | Duration of the double-tap zoom animation; passed as `zoomDuration` and scaled by `getSystemScaledDuration()` (the Android animator duration scale). | `pref_double_tap_anim_speed` = "Double tap animation speed"; `double_tap_anim_speed_0` "No animation", `_normal` "Normal", `_fast` "Fast" | `reader.double_tap_anim_ms: u32 = 500` | Keep (P1) |
| `pref_dual_page_invert` | normal | `Boolean` | `false` | `true`/`false` | `ReaderPreferences.kt:97` | `PagerConfig.kt:95-96`; `PagerPageHolder.kt:218-235`; `ReadingModePage.kt:123`; `SettingsReaderScreen.kt:283-288` | Paged viewers: which half of a split wide page is shown first. | `pref_dual_page_invert` = "Invert split page placement"; summary "If the placement of the split wide pages don't match reading direction"; enabled only while `pref_dual_page_split` is on | `reader.dual_page_invert_paged: bool = false` | Keep (P2) |
| `pref_dual_page_invert_webtoon` | normal | `Boolean` | `false` | `true`/`false` | `ReaderPreferences.kt:101` | `WebtoonConfig.kt:67-68`; `WebtoonPageHolder.kt:212-226`; `ReadingModePage.kt:188`; `SettingsReaderScreen.kt:378-383` | Webtoon viewers: which half of a split wide page ends on top. | same label and gating as above, in the "Webtoon" group, gated by `pref_dual_page_split_webtoon` | `reader.dual_page_invert_webtoon: bool = false` | Keep (P2) |

(`ReadingModePage.kt` is `app/src/main/java/eu/kanade/presentation/reader/settings/ReadingModePage.kt`.)

### Notes for section 6

#### `pref_always_decode_long_strip_with_ssiv`
- Decision: Drop. Reason: SSIV vs Coil decoding is an Android renderer detail. GTK4 draws a `gdk::Texture`; GL textures are limited to the GPU's maximum texture size (commonly 16384 px per side), so images taller than that cannot be uploaded as one texture. Replacement (area:44-reader-viewers): the webtoon viewer always slices decoded images into horizontal tiles no taller than `reader.max_texture_tile` (chunk 5, key `pref_hardware_bitmap_threshold`; default derived from the GL `MAX_TEXTURE_SIZE`) and lays the tiles out gap-free. What the user loses: the legacy-decoder escape hatch. The importer discards the key.

#### `pref_color_filter_key`
- OBSERVED value format (`ColorFilterPage.kt:100-160`): `color_filter_value` is packed ARGB; each slider writes one channel with `(new shl shift) or (current and mask.inv())` using `ALPHA_MASK = 0xFF000000` (shift 24), `RED_MASK = 0x00FF0000` (16), `GREEN_MASK = 0x0000FF00` (8), `BLUE_MASK = 0x000000FF` (0). `color_filter_mode` is an index: 0 `SrcOver` ("Default"), 1 `Modulate` ("Multiply"), 2 `Screen`, then on API 28 and above 3 `Overlay`, 4 `Lighten` ("Dodge / Lighten"), 5 `Darken` ("Burn / Darken").
- Proposal (area:44-reader-viewers). Draw the overlay inside a `gtk::Overlay` above the page view with a custom `gtk::Snapshot` (`snapshot.push_blend` with `gsk::BlendMode`: Default, Multiply, Screen, Overlay, Lighten, Darken; the colour as `gdk::RGBA`, alpha from the A channel). Fields `reader.color_filter.rgba: u32` and `reader.color_filter.blend_mode` are in chunk 1. Importer: Boolean 1:1.

#### `pref_custom_brightness_key`
- OBSERVED (`ReaderActivity.kt:954-968`): value `> 0` sets window brightness `value / 100f`; `< 0` sets window brightness `0.01f` and `ReaderContentOverlay` draws a black rectangle at alpha `abs(value) / 100f`; `0` restores system brightness and hides the overlay.
- Proposal: follow chunk 1 (`custom_brightness_value`): negative values keep the black veil with the same alpha; positive values apply a GSK colour-matrix brightening; system brightness is never touched. Importer: Boolean 1:1.

#### `pref_default_orientation_type_key`
- Decision: Drop. Reason: no orientation lock on a desktop window. What the user loses: forced portrait/landscape on tablets. Keep the `manga.viewer_flags` column and its bit layout (area:03-db-manga-schema-migrations) so backups round-trip; the Rust reader ignores the orientation bits. The importer discards the preference.
- Legacy migration (OBSERVED), `MigrateRotationViewerValuesMigration` (version `60f`): reads `pref_rotation_type_key` (Int, default 1): `1` to `FREE` (0x08), `2` to `PORTRAIT` (0x10), `3` to `LANDSCAPE` (0x18), `4` to `LOCKED_PORTRAIT` (0x20), `5` to `LOCKED_LANDSCAPE` (0x28), anything else to `FREE`; writes `pref_default_orientation_type_key` and removes the old key.

#### `pref_default_reading_mode_key`
- Proposal (area:43-reader-core). A Rust enum `ReadingMode { LeftToRight = 1, RightToLeft = 2, Vertical = 3, Webtoon = 4, ContinuousVertical = 5 }` with an extra `Default = 0` used only for the per-title column; the config field never holds `Default`. Written to TOML as the variant name, accepted on import as either the name or the integer (`1..=5`; `0` or anything else becomes `RightToLeft`). The same integer mask `0x7` is used for `manga.viewer_flags`. The same migration as above maps old `pref_default_viewer_key` (Int, default 1) straight to this key ("Reading mode flag and prefValue is the same value").

#### `pref_display_profile_key`
- Proposal (area:44-reader-viewers). Store a filesystem path (a file chooser via `gtk::FileDialog`). When non-empty and readable, convert each decoded image to sRGB with `lcms2` (crate `lcms2`, `Profile::new_file` then `Transform` input = embedded profile or sRGB, output = the chosen profile); otherwise skip. Importer: a `content://` URI cannot be resolved, so the value is discarded with a one-line log message and the field stays empty. What the user loses: nothing for the default case; colour management of GTK4 is not applied automatically.

#### `pref_double_tap_anim_speed`
- Proposal (area:44-reader-viewers). Animate the double-tap zoom over `double_tap_anim_ms` using the widget frame clock; if `gtk-enable-animations` is false use `1`. Settings list keeps the three values.

#### `pref_dual_page_invert`
- OBSERVED side choice for a split wide page (`PagerPageHolder.splitInHalf`, lines 218-235): `L2RPagerViewer`: page not an `InsertPage` shows the LEFT half, an `InsertPage` the RIGHT half. Any other paged viewer (R2L, vertical): the page shows the RIGHT half and the `InsertPage` the LEFT half. If `dualPageInvert` is true, LEFT and RIGHT are swapped.
- Proposal (area:44-reader-viewers). Same table.

#### `pref_dual_page_invert_webtoon`
- OBSERVED (`WebtoonPageHolder.process`, lines 212-226): with `dualPageRotateToFit` false and `dualPageSplit` true, a wide image (`ImageUtil.isWideImage`) is split into two halves stacked vertically with `ImageUtil.splitAndMerge(source, upperSide)`, where `upperSide = LEFT` if `dualPageInvert` else `RIGHT`; the half named `upperSide` goes on top.
- Proposal (area:44-reader-viewers). Same rule.

---

## 7. Legacy keys that exist only in migrations

| Key | Store | Kotlin type | Default | Values | Defined | Read / used | Behaviour | UI label | Rust field | Decision |
|---|---|---|---|---|---|---|---|---|---|---|
| `pref_deband` | normal (legacy; removed after migration) | `Int` | `0` as read by the migration | ordinal of `Debanding`: `0` None, `1` CPU, `2` GPU | `app/src/main/java/mihon/core/migration/migrations/EnumsMigration.kt:22` (read), `:30` (removal) | `EnumsMigration.kt:22,30,34-37` only | Migration `EnumsMigration` (`version = 123f`): maps the ordinal with `Debanding.entries.first { it.ordinal == deband.get() }`, writes enum `pref_video_debanding`; throws `NoSuchElementException` for any ordinal outside `0..=2`. The same migration also removes `pref_invert_playback_txt`, `pref_invert_duration_txt`, `pref_hwdec`, `pref_player_view_mode`, `gpu_next`. | none | `player.decoder.debanding: Debanding = None` (proposed name; the live key `pref_video_debanding` belongs to the decoder-settings keys) | Replace |
| `pref_default_viewer_key` | normal (legacy; removed after migration) | `Int` | `1` as read by the migration | an old viewer id, equal to the `ReadingMode` flag value | `MigrateRotationViewerValuesMigration.kt:28` (read), `:34` (removal) | `MigrateRotationViewerValuesMigration.kt:28,33-34` only | Migration (`version = 60f`) copies the value unchanged into `pref_default_reading_mode_key` and deletes the old key. | none | none (importer maps to `reader.default_reading_mode`) | Replace |
| `pref_download_only` | normal (legacy) | `Boolean` | not read by live code | `true`/`false` | `NoAppStateMigration.kt:18` (one of twelve names in `prefsToReplace`) | `NoAppStateMigration.kt:15-30` only | Migration `NoAppStateMigration` (`version = 113f`) renamed unprefixed keys to `Preference.appStateKey(key)` (for this key `__APP_STATE_pref_download_only`) and deleted the old key (`MigrationUtils.replacePreferences`); it then wiped `context.cacheDir`. The live setting is the differently named key `pref_downloaded_only`, so a value migrated here is orphaned and never read. | none | none | Drop |

### Notes for section 7

#### `pref_deband`
- OBSERVED use of the converted value (`AniyomiMPVView.kt:119-123`): `Debanding.None` no change, `Debanding.CPU` sets option `vf` to `gradfun=radius=12`, `Debanding.GPU` sets option `deband` to `yes`.
- Proposal (area:11-app-migrations, area:21-backup-format-and-create). Migrations from the Android app are not run; the importer handles the legacy key only if it appears in a backup: `0`, `1`, `2` to `None`, `CPU`, `GPU`; every other value to `None` (no failure; this fixes the `NoSuchElementException`). Decision: Replace (importer-only).

#### `pref_default_viewer_key`
- Proposal (area:22-backup-restore). If a backup contains `pref_default_viewer_key` and not `pref_default_reading_mode_key`, import the integer into `reader.default_reading_mode` using the rule above. Same for `pref_rotation_type_key` (ignored in Rust, so nothing to map).

#### `pref_download_only`
- Proposal (area:22-backup-restore). Discard. State lives in `state.downloaded_only` (key `pref_downloaded_only`). Because app-state keys are never included in a backup, no backup written by Aniyomi at version 113 or later contains it; an older backup may, and the importer ignores it.

---

## 8. Library, browse and UI

| Key | Store | Kotlin type | Default | Values | Defined | Read / used | Behaviour | UI label | Rust field | Decision |
|---|---|---|---|---|---|---|---|---|---|---|
| `pref_animelib_columns_landscape_key` | normal | `Int` | `0` | `0` = auto, `1..=10` | `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:105` | `BrowseAnimeSourceScreenModel.kt:130,141`; `MigrateSeasonSelectScreenModel.kt:56`; `AnimeLibraryScreenModel.kt:587`; `AnimeLibrarySettingsDialog.kt:252` (all under `app/src/main/java/eu/kanade/...`) | Grid column count when the device is in landscape. `0` yields `GridCells.Adaptive(128.dp)`, otherwise `GridCells.Fixed(n)` (`BrowseAnimeSourceScreenModel.kt:127-135`, `LazyLibraryGrid.kt:22`). The anime browse and season-migration screens reuse the same key. In `List` display mode the same slider is the list size. | library sheet, Display tab: slider `pref_library_columns` = "Items per row" (grid) or `pref_library_rows` = "List size" (list); value text `label_auto` "Auto" when 0; range `0..10` | `library.anime_columns_landscape: u8 = 0` | Keep (changed semantics) (P0) |
| `pref_animelib_columns_portrait_key` | normal | `Int` | `0` | `0` = auto, `1..=10` | `LibraryPreferences.kt:102` | `AnimeLibraryScreenModel.kt:589`; `AnimeLibrarySettingsDialog.kt:254`; `BrowseAnimeSourceScreenModel.kt:132,143`; `MigrateSeasonSelectScreenModel.kt:58` | Same as above for portrait orientation. | same slider | `library.anime_columns_portrait: u8 = 0` | Keep (changed semantics) (P0) |
| `pref_app_theme` | normal | enum `AppTheme`, stored as `.name` | `MONET` if `DeviceUtil.isDynamicColorAvailable`, else `DEFAULT` | `DEFAULT`, `MONET`, `CLOUDFLARE`, `COTTONCANDY`, `DOOM`, `GREEN_APPLE`, `LAVENDER`, `MATRIX`, `MIDNIGHT_DUSK`, `MOCHA`, `SAPPHIRE`, `NORD`, `STRAWBERRY_DAIQUIRI`, `TAKO`, `TEALTURQUOISE`, `TIDAL_WAVE`, `YINYANG`, `YOTSUBA`, `MONOCHROME`; deprecated, no title: `DARK_BLUE`, `HOT_PINK`, `BLUE` | `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:22-29` | `TachiyomiTheme.kt:45`; `ContextExtensions.kt:140`; `ThemingDelegate.kt:27`; `ThemeStep.kt:25`; `SettingsAppearanceScreen.kt:60-85` (under `app/src/main/java/eu/kanade/...`) | Selects the Material 3 colour scheme (`colorSchemes` map in `TachiyomiTheme.kt`, `MonetColorScheme` for `MONET`) and the matching XML theme (`ThemingDelegate.kt` `themeResources`, with `ThemeOverlay_Tachiyomi_Amoled` added when `pref_theme_dark_amoled_key`). `AppThemePreferenceWidget` lists the entries whose `titleRes != null`, and hides `MONET` where dynamic colour is unavailable. | Settings, Appearance, Theme: `pref_app_theme` = "Theme" preview grid; `MONET` shown as "Dynamic" (`theme_monet`), `DEFAULT` as "Default" (`label_default`) | none | Replace |
| `pref_chapter_swipe_end_action` | normal | enum `ChapterSwipeAction`, stored as `.name` | `ToggleRead` | `ToggleRead`, `ToggleBookmark`, `Download`, `Disabled` | `LibraryPreferences.kt:369-370` (accessor `swipeChapterStartAction()`) | `MangaScreenModel.kt:145` (assigned to `chapterSwipeEndAction`); `SettingsLibraryScreen.kt:329-342`; `MangaChapterListItem.kt:61-90`; `MangaScreenModel.kt:607-642` (under `app/src/main/java/eu/kanade/...`) | Action when a chapter row is swiped to the LEFT (the `endActions` of `SwipeableActionsBox`; the settings row labelled "swipe to left" edits this key). The accessor and key names are crossed twice, but the net effect is consistent: key `..._end_action` is the swipe-to-left action. | `pref_chapter_swipe_start` = "Chapter on swipe to left" (the row titled "start" edits the `..._end_action` key) | `library.swipe.chapter_left: ChapterSwipeAction = ToggleRead` | Keep (changed semantics) (P2) |
| `pref_chapter_swipe_start_action` | normal | enum `ChapterSwipeAction`, stored as `.name` | `ToggleBookmark` | same four values | `LibraryPreferences.kt:372-375` (accessor `swipeChapterEndAction()`) | `MangaScreenModel.kt:144` (assigned to `chapterSwipeStartAction`); `SettingsLibraryScreen.kt:343-357`; `MangaChapterListItem.kt:61-90` | Action when a chapter row is swiped to the RIGHT (`startActions`). | `pref_chapter_swipe_end` = "Chapter on swipe to right" (the row titled "end" edits the `..._start_action` key) | `library.swipe.chapter_right: ChapterSwipeAction = ToggleBookmark` | Keep (changed semantics) (P2) |
| `pref_display_mode_catalogue` | normal | `LibraryDisplayMode` via `getObject` (string) | `LibraryDisplayMode.default` = `CompactGrid` | `COMPACT_GRID`, `COMFORTABLE_GRID`, `COVER_ONLY_GRID`, `LIST` (see 1.5) | `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:16-21` | `BrowseMangaSourceScreenModel.kt:77`; `BrowseAnimeSourceScreenModel.kt:79`; `MigrateSeasonSelectScreenModel.kt:50`; `AnimeScreenModel.kt:179`; `BrowseAnimeSourceToolbar.kt:62-117`; `BrowseAnimeSourceScreen.kt:121-146` (under `app/src/main/java/eu/kanade/...`) | One display mode shared by manga browse, anime browse, season migration and the related-anime rows on the entry page. The browse toolbar menu offers Comfortable, Compact, List only; `CoverOnlyGrid` (only reachable via a backup or the library key) renders as the compact grid in browse. | browse toolbar action `action_display_mode` = "Display mode", radio entries "Comfortable grid", "Compact grid", "List" | `sources.display_mode: DisplayMode = CompactGrid` | Keep (P1) |
| `pref_display_mode_library` | normal | `LibraryDisplayMode` via `getObject` (string) | `CompactGrid` | same four strings | `LibraryPreferences.kt:18-23` | `MangaLibraryScreenModel.kt:566`; `AnimeLibraryScreenModel.kt:581`; `SetMangaDisplayMode.kt:11`; `SetAnimeDisplayMode.kt:11`; `MangaLibrarySettingsDialog.kt:238-242`; `AnimeLibrarySettingsDialog.kt:238-242`; `AnimeLibraryPager.kt:75-120` | One display mode shared by the manga and anime libraries (both screens read and write the same key). `CompactGrid` and `CoverOnlyGrid` use the compact grid, with `showTitle` true only for `CompactGrid`; `ComfortableGrid` shows the title under the cover; `List` is a list. | library sheet, Display tab, chip row `action_display_mode`: "Compact grid" (`action_display_grid`), "Comfortable grid", "Cover-only grid", "List" | `library.display_mode: DisplayMode = CompactGrid` | Keep (P0) |

### Notes for section 8

#### `pref_animelib_columns_landscape_key` and `pref_animelib_columns_portrait_key`
- Proposal (area:37-ui-library, area:41-ui-browse-anime). Keep the two fields so a backup round-trips and for consistency with `library.manga_columns_landscape` / `library.manga_columns_portrait` (chunk 6). Desktop meaning: "landscape" when the window is wider than tall, "portrait" otherwise. Value `0` (auto) = as many columns as fit with a minimum cell width of 128 logical pixels (`columns = max(1, floor(available_width / 128))`, cells then stretch to fill the width; implement with `gtk::GridView` and `min_columns`/`max_columns` set from this formula on each size allocation). Value `n` in `1..=10` forces exactly `n` columns. Importer clamps to `0..=10`. In `List` display mode the value is the list "size"; Proposal: ignore it in list mode (a desktop list is one row per entry) and keep the stored number unchanged.

#### `pref_app_theme`
- Decision: Replace. The app uses the user's GTK theme (vanilla GTK4, no libadwaita); no palette picker. What the user loses: the 18 bundled palettes, Material You dynamic colour, and the pure-black variant tied to this key. Replacement: the GTK theme chosen in KDE system settings (the maintainer runs Tokyo Night; the app follows it) and `gtk-application-prefer-dark-theme`. The importer discards the key; the light/dark choice key `pref_theme_mode_key` is catalogued elsewhere.
- Default expression (OBSERVED): `DeviceUtil.isDynamicColorAvailable` is `DynamicColors.isDynamicColorAvailable() || (DeviceUtil.isSamsung && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt:6-7`).

#### `pref_chapter_swipe_end_action` and `pref_chapter_swipe_start_action`
- OBSERVED swipe behaviours (`MangaScreenModel.executeChapterSwipeAction`, `MangaChapterListItem.getSwipeAction`): `ToggleRead` calls `markChaptersRead(listOf(chapter), !chapter.read)` (icon `Done` when unread, `RemoveDone` when read); `ToggleBookmark` calls `bookmarkChapters(listOf(chapter), !chapter.bookmark)` (icon `BookmarkAdd` / `BookmarkRemove`); `Download` chooses from the chapter's download state: `ERROR` or `NOT_DOWNLOADED` start now (`ChapterDownloadAction.START_NOW`, icon `Download`), `QUEUE` or `DOWNLOADING` cancel (`CANCEL`, icon `FileDownloadOff`), `DOWNLOADED` delete (`DELETE`, icon `Delete`); `Disabled` renders no action (swiping is inert) and `executeChapterSwipeAction` throws `IllegalStateException` if ever called with it. Reveal threshold `swipeActionThreshold = 56.dp`. Marking read also runs the auto-track prompt described under `pref_auto_update_manga_on_mark_read`.
- Anime counterparts are `pref_episode_swipe_end_action` and `pref_episode_swipe_start_action` (chunk 5; Rust `library.swipe.episode_left` and `library.swipe.episode_right`).
- Proposal (area:38-ui-entry-details-manga, area:37-ui-library). Mouse and keyboard users have no swipe, so the two fields drive (1) horizontal two-finger touchpad / touch drags on the row via `gtk::GestureDrag` with the same 56 px reveal threshold, and (2) the first two entries of the row's context menu and hover buttons. The fields keep the Kotlin semantics: left action first `ToggleRead`, right action `ToggleBookmark`. Backup import: key `pref_chapter_swipe_end_action` maps to `chapter_left` and `pref_chapter_swipe_start_action` to `chapter_right`; do not swap them.

#### `pref_display_mode_catalogue` and `pref_display_mode_library`
- Proposal (area:37-ui-library, area:40-ui-browse-manga, area:41-ui-browse-anime). Rust enum `DisplayMode { CompactGrid, ComfortableGrid, CoverOnlyGrid, List }` with serde renames to the exact strings in 1.5; unknown string deserializes to `CompactGrid`. Rendering: `CompactGrid` = cover with title overlaid at the bottom over a gradient, `CoverOnlyGrid` = cover only, `ComfortableGrid` = cover with the title in a label below, `List` = row with a small cover, title and badges. Keep the two keys separate, as in Aniyomi (library vs. browse).

---

## 9. Tracking: `TrackPreferences.kt`

| Key | Store | Kotlin type | Default | Values | Defined | Read / used | Behaviour | UI label | Rust field | Decision |
|---|---|---|---|---|---|---|---|---|---|---|
| `pref_auto_update_manga_on_mark_read` | normal | enum `AutoTrackState`, stored as `.name` | `ALWAYS` | `ALWAYS`, `ASK`, `NEVER` | `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt:48-51` | `MangaScreenModel.kt:146,750-791`; `AnimeScreenModel.kt:173,870-912`; `SettingsTrackingScreen.kt:95,150-156` (under `app/src/main/java/eu/kanade/...`) | What happens to trackers when the user marks chapters read (or episodes seen) manually from the entry page. Algorithm below. | `pref_auto_update_manga_on_mark_read` = "Update progress when marked as read/watched" (list); entries "Always", "Always ask", "Never" | `tracking.auto_update_on_mark_read: AutoTrackState = ALWAYS` | Keep (P1) |
| `pref_auto_update_manga_sync_key` | normal | `Boolean` | `true` | `true`/`false` | `TrackPreferences.kt:39` | `ReaderViewModel.kt:920`; `PlayerViewModel.kt:1949`; `ExternalIntents.kt:479,525`; `SettingsTrackingScreen.kt:140` (under `app/src/main/java/eu/kanade/...`) | Master switch for pushing progress to trackers automatically after reading/watching. | `pref_auto_update_manga_sync` = "Update progress after reading/watching" | `tracking.auto_update_after_reading: bool = true` | Keep (P1) |

### Notes for section 9

#### `pref_auto_update_manga_on_mark_read`
- OBSERVED algorithm (`MangaScreenModel.markChaptersRead`, `AnimeScreenModel.markEpisodesSeen`): set read/seen status first; return immediately if the action was "mark unread", or the entry has no logged-in tracker, or the value is `NEVER`. Otherwise refresh the tracker records (`RefreshMangaTracks` / `RefreshAnimeTracks`), compute `max = max(chapterNumber)` (or `episodeNumber`) over the affected items, and continue only if `tracks.any { max > track.lastChapterRead }` (`lastEpisodeSeen` for anime). `ALWAYS`: call `trackChapter` / `trackEpisode` with `max`, then toast `trackers_updated_summary_manga` "Trackers updated to chapter %d" (anime `trackers_updated_summary_anime` "Trackers updated to episode %d") with `max.toInt()`. `ASK`: show a snackbar (duration Short, dismiss action) with `confirm_tracker_update` "Update trackers to chapter %d?" (anime `confirm_tracker_update_anime` "Update trackers to episode %d?"), action label `action_ok`; only on `ActionPerformed` call the tracker update (no toast).
- Proposal (area:27-tracking-core, area:38-ui-entry-details-manga). Same algorithm; the snackbar becomes a `gtk::Revealer` banner with an "OK" button and a 4-second timeout (Material "Short" is 4000 ms). The setting is read once when the entry page opens (Kotlin reads it into a `var` at screen-model creation); re-read on each page open. Importer: unknown name leaves `ALWAYS`.

#### `pref_auto_update_manga_sync_key`
- OBSERVED conditions: `ReaderViewModel.updateTrackChapterRead` also requires `!incognitoMode`; `PlayerViewModel.updateTrackEpisodeSeen` requires `!basePreferences.incognitoMode().get() && hasTrackers`; the external-player path (`ExternalIntents.kt:479`) additionally requires the episode to be marked seen (`currEp.seen`). Note the per-extension incognito set is not consulted by the anime player (chunk 3, finding 4).
- Proposal. One function `should_push_progress(source_id)` = `auto_update_after_reading && !is_incognito(source_id)`, used by the reader, the internal player and the external-player path. Importer: Boolean 1:1.

---

## 10. Downloads and base state

| Key | Store | Kotlin type | Default | Values | Defined | Read / used | Behaviour | UI label | Rust field | Decision |
|---|---|---|---|---|---|---|---|---|---|---|
| `pref_download_fillermarked` | normal | `Boolean` | `false` | `true`/`false` | `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:37` | `AnimeDownloadManager.kt:146,429-435`; `SettingsDownloadScreen.kt:155` (under `app/src/main/java/eu/kanade/...`) | Anime only. `downloadEpisodes` runs `getEpisodesToDownload(episodes)` before queueing: when the flag is false every fillermarked episode (`episode.fillermark`) is dropped from the batch; when true all are kept. Applies to every path that calls `downloadEpisodes`, including auto-download. | `pref_download_fillermarked_items` = "Allow downloading of fillermarked episodes"; group "Delete downloads / downloads" in Settings, Downloads | `downloads.download_fillermarked: bool = false` | Keep (P1) |
| `pref_download_only_over_wifi_key` | normal | `Boolean` | `true` | `true`/`false` | `DownloadPreferences.kt:9-12` | `MangaDownloadJob.kt:59,72`; `AnimeDownloadJob.kt:59,72`; `SettingsDownloadScreen.kt:80` (under `app/src/main/java/eu/kanade/tachiyomi/data/download/...` and `app/src/main/java/eu/kanade/presentation/more/settings/screen/`) | The download worker starts only if online and (if the flag is true) on Wi-Fi; the flag and the network state are re-evaluated on every change. If offline: `downloaderStop` with `download_notifier_no_network` = "No network connection available". If the flag is true and not on Wi-Fi: `downloaderStop` with `download_notifier_text_only_wifi` = "No Wi-Fi connection available". The worker returns `Result.failure()` when it cannot start. | `connected_to_wifi` = "Only on Wi-Fi" | `downloads.only_unmetered: bool = true` | Keep (changed semantics) (P1) |
| `pref_downloaded_only` | app-state (stored as `__APP_STATE_pref_downloaded_only`) | `Boolean` | `false` | `true`/`false` | `BasePreferences.kt:16-19` | `MainActivity.kt:179,183-187`; `MoreTab.kt:94`; `MoreScreen.kt:36,62-66`; `Manga.kt:25`; `Anime.kt:15,25`; `ReaderViewModel.kt:215`; `PlayerViewModel.kt:1342`; `MangaLibraryScreenModel.kt:180-182,337`; `AnimeLibraryScreenModel.kt:183-185,350`; `MangaLibrarySettingsDialog.kt:83`; `AnimeLibrarySettingsDialog.kt:83-93`; `ChapterSettingsDialog.kt:70`; `EpisodeSettingsDialog.kt:61,90`; `SeasonSettingsDialog.kt:79` (under `app/src/main/java/eu/kanade/...`) | Global "downloaded only" mode. See notes. | More tab: `label_downloaded_only` = "Downloaded only", summary `downloaded_only_summary` = "Filters all entries in your library" | `state.downloaded_only: bool = false` | Keep (state, not config) (P1) |

### Notes for section 10

#### `pref_download_fillermarked`
- Proposal (area:24-downloads-anime). Apply the same filter at the single entry point that queues anime episodes: `episodes.retain(|e| config.download_fillermarked || !e.fillermark)`. If the filter removes every item of a manual request, nothing is queued and no error is shown (Kotlin behaviour). Importer: Boolean 1:1.

#### `pref_download_only_over_wifi_key`
- Decision: Keep with changed semantics. Android distinguishes Wi-Fi from mobile data; a desktop is usually wired, so "Wi-Fi" would wrongly block Ethernet. Proposal: field `downloads.only_unmetered: bool = true` meaning "pause downloads while the active connection is metered". Determine meteredness through the NetworkManager D-Bus property `org.freedesktop.NetworkManager.Metered` (`NM_METERED_UNKNOWN` 0, `YES` 1, `NO` 2, `GUESS_YES` 3, `GUESS_NO` 4; treat 1 and 3 as metered; subscribe to `PropertiesChanged` on the NetworkManager object, crate `zbus`). If NetworkManager is not on the bus, assume unmetered (never block). Offline detection and the two stop messages are unchanged. Importer: Boolean 1:1. What the user loses: a Wi-Fi-versus-Ethernet distinction; a metered tethered phone behaves like mobile data.

#### `pref_downloaded_only`
- OBSERVED effects of the global mode:
  1. Library list: `AnimeLibraryScreenModel`/`MangaLibraryScreenModel` compute `filterDownloaded = if (globalFilterDownloaded) TriState.ENABLED_IS else prefs.filterDownloaded`, so only entries with a downloaded item show; the library sheet row "Downloaded" (`label_downloaded`) shows the state as `ENABLED_IS` and is disabled (`enabled = !downloadedOnly`); the stored per-user filter value is not rewritten.
  2. Entry page: `Manga.downloadedFilter` (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:25`), `Anime.downloadedFilter` and `Anime.seasonDownloadedFilter` (`app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:15,25`) return `TriState.ENABLED_IS` regardless of the stored filter; the filter dialogs pass `onDownloadFilterChanged.takeUnless { downloadedOnly }`, a null callback, so the "Downloaded" row cannot be changed (`ChapterSettingsDialog.kt:70,106`, `EpisodeSettingsDialog.kt:61,90`, `SeasonSettingsDialog.kt:79,108`, all under `app/src/main/java/eu/kanade/presentation/entries/`).
  3. Reader and player: the chapter list built in `ReaderViewModel.kt:205-220` and `PlayerViewModel.initEpisodeList` apply `filterDownloadedChapters(manga)` (`app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterFilterDownloaded.kt:13`) / `filterDownloadedEpisodes(anime)` (`app/src/main/java/eu/kanade/tachiyomi/util/episode/EpisodeFilterDownloaded.kt:13`), so next/previous navigation only sees downloaded items.
  4. Main window: while on it draws a banner background (`DownloadedOnlyBannerBackgroundColor`); priority order of the banner colours is indexing, then downloaded-only, then incognito.
- Proposal (area:36-ui-shell-navigation-security-webview, area:37-ui-library). Keep it in `state.toml` as `state.downloaded_only`, never backed up (same as Kotlin, which excludes `__APP_STATE_` keys). Implement as a single `SourceFilter` consulted by the library query, the entry page filters, and the reader/player playlists. Show the banner as a one-line `gtk::InfoBar`-style strip (a `gtk::Box` with a label; `gtk::InfoBar` is deprecated in GTK 4.10+). Provide the toggle in the sidebar menu and the application menu.

---

## 11. Cross-cutting findings (OBSERVED, with the consequence for Rust)

| # | Finding | Evidence | Consequence for Rust |
|---|---|---|---|
| 1 | `AudioChannels.Auto` and `AudioChannels.AutoSafe` have their mpv values swapped relative to their labels; the default `AutoSafe` sends mpv `auto`. | `AudioPreferences.kt:108-109` | Keep the enum names, fix the mapping; imported defaults land on `auto-safe`. |
| 2 | `PlayerOrientation.ReverseLandscape` requests `SCREEN_ORIENTATION_REVERSE_PORTRAIT`. | `PlayerActivity.kt:849` | Moot: orientation is dropped. |
| 3 | `player_volume_value` and the switch `pref_remember_volume` do nothing in Kotlin. | `PlayerPreferences.kt:28-29`; only `PlayerSettingsPlayerScreen.kt` references them | Implement (P2) rather than port a no-op. |
| 4 | `pref_player_fullscreen` ("player_fullscreen") is only the display-cutout setting. | `PlayerActivity.kt:397-402` | Drop; do not map to window fullscreen. |
| 5 | The chapter and episode swipe key names are crossed with their accessor names; the user-visible direction is consistent (`..._end_action` is swipe left, `..._start_action` is swipe right). | `LibraryPreferences.kt:360-375`; `MangaScreenModel.kt:144-145`; `i18n/src/commonMain/moko-resources/base/strings.xml:310-313` | Rust names fields by direction (`chapter_left`, `chapter_right`) and maps imported keys as in section 8. |
| 6 | `NoAppStateMigration` renames `pref_download_only`, but the live key is `pref_downloaded_only`; the migrated value is orphaned. | `NoAppStateMigration.kt:18`; `BasePreferences.kt:17` | Ignore `pref_download_only` on import. |
| 7 | `EnumsMigration` throws `NoSuchElementException` for a `pref_deband` ordinal outside `0..=2` (and likewise for `pref_player_view_mode` outside `0..=2`). | `EnumsMigration.kt:34-35` | Importer clamps unknown enum values to the default (also chunk 3, finding 8). |
| 8 | Anime library columns (`pref_animelib_columns_*`), the library display mode and the browse display mode are reused by other screens (browse, season migration, related rows). | `BrowseAnimeSourceScreenModel.kt:127-145`; `MigrateSeasonSelectScreenModel.kt:50-60`; `AnimeScreenModel.kt:179` | One `grid_columns` resolver and one `DisplayMode` renderer shared by all list screens (area:37-ui-library). |
| 9 | `pref_default_viewer_key` is migration-only (version 60) and the value equals the `ReadingMode` flag. | `MigrateRotationViewerValuesMigration.kt:27-34` | Importer maps it to `reader.default_reading_mode` when the new key is absent. |
| 10 | The "Auto-track on mark read" setting is read once into a `var` at entry-screen creation, so changing it while the page is open has no effect until reopened. | `MangaScreenModel.kt:146`; `AnimeScreenModel.kt:173` | Rust re-reads on page open; behaviour identical. |
