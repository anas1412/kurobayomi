# P1 behaviour spec: 48-settings-player-and-scripts

Scope: the player settings tree, the custom button feature (screen, domain, repository, Lua runtime), the mpv script and config editor, the code editor, global UI preference models, and the TorrServer settings page. All paths are repo-relative to `/home/blackbox/aniyomi-src`. Line references are `path:line`.

Labels: **OBSERVED** = what the Kotlin does. **Proposal:** = what the Rust port should do.

Subsystem index:
1. Settings tree and navigation
2. Preference widget semantics
3. Internal player settings page
4. Gestures page
5. Decoder page
6. Subtitle page and language-code validator
7. Audio page
8. Advanced page (mpv.conf, input.conf, user files)
9. mpv runtime directory sync
10. Script editor (file list)
11. Code editor and syntax highlighter
12. Custom buttons (summary; details in `p1-topic-custom-buttons.md`)
13. mpv Lua bridge (summary; details in `p1-topic-custom-buttons.md`)
14. TorrServer settings page (details in `p1-topic-torrent-settings.md`)
15. Global UI preferences (theme, nav, start screen, date)
16. Storage root and directory layout
17. Interactions with other areas

---

## 1. Settings tree and navigation

**OBSERVED**

- Root screen `PlayerSettingsMainScreen(mainSettings: Boolean)` (`app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsMainScreen.kt:56`). Its app bar title is `AYMR.strings.label_player` ("Player") when `mainSettings == true`, otherwise `AYMR.strings.label_player_settings` ("Player settings") (`PlayerSettingsMainScreen.kt:89-95`). The app bar uses a pinned scroll behaviour (`TopAppBarDefaults.pinnedScrollBehavior`, line 86).
- App bar action: one search icon (`Icons.Outlined.Search`) with title `MR.strings.action_search` ("Search") that calls `navigator.navigate(SettingsSearchScreen(true), twoPane)` (lines 97-106).
- Root list items, in order (`PlayerSettingsMainScreen.kt:184-239`). Each row: title, subtitle, icon, target screen.

| # | Title key (English) | Subtitle key | Icon | Target |
|---|---|---|---|---|
| 1 | `AYMR.strings.pref_player_internal` ("Internal player") | `pref_player_internal_summary` ("Progress, controls, orientation") | `Icons.Outlined.PlayCircleOutline` | `PlayerSettingsPlayerScreen` |
| 2 | `pref_player_gestures` ("Gestures") | `pref_player_gestures_summary` ("Seeking, double tap, media controls") | `Icons.Outlined.Gesture` | `PlayerSettingsGesturesScreen` |
| 3 | `pref_player_decoder` ("Decoder") | `pref_player_decoder_summary` ("Hardware decoding, pixel format, debanding") | `Icons.Outlined.Memory` | `PlayerSettingsDecoderScreen` |
| 4 | `pref_player_subtitle` ("Subtitles") | `pref_player_subtitle_summary` ("Preferred languages, whitelist, blacklist") | `Icons.Outlined.Subtitles` | `PlayerSettingsSubtitleScreen` |
| 5 | `pref_player_audio` ("Audio") | `pref_player_audio_summary` ("Preferred languages, pitch correction, audio channels") | `Icons.Outlined.Audiotrack` | `PlayerSettingsAudioScreen` |
| 6 | `pref_player_custom_button` ("Custom buttons") | `pref_player_custom_button_summary` ("Execute lua code with custom buttons") | `Icons.Outlined.Terminal` | `PlayerSettingsCustomButtonScreen` |
| 7 | `pref_player_editor` ("Script editor") | `pref_player_editor_summary` ("Edit and create new scripts and script options") | `Icons.Outlined.EditNote` | `PlayerSettingsEditorScreen` |
| 8 | `pref_player_torrents` ("Torrent") | `pref_player_torrents_summary` ("TorrServer options") | `CustomIcons.Magnet` | `PlayerSettingsTorrentScreen` |
| 9 | `pref_player_advanced` ("Advanced") | `pref_player_advanced_summary` ("User files, mpv.conf, input.conf") | `Icons.Outlined.Code` | `PlayerSettingsAdvancedScreen` |

- Navigation: on phones each item is `push`ed onto the Voyager navigator. In two-pane mode it is `replaceAll` (`PlayerSettingsMainScreen.kt:172-174`).
- Two-pane root (`app/src/main/java/eu/kanade/tachiyomi/ui/setting/PlayerSettingsScreen.kt:44-58`): the start pane is the root list, the end pane is a nested navigator starting at `PlayerSettingsPlayerScreen`. The selected row gets `surfaceVariant` background, rounded 24dp clip, horizontal padding 8dp, and `onSurfaceVariant` content colour (`PlayerSettingsMainScreen.kt:140-156`). On first composition the list scrolls to the row whose screen class equals the current nav item; if that index is > 0 the top bar is lifted (`topBarState.contentOffset = topBarState.heightOffsetLimit`) (lines 113-127).
- Two-pane background colour (`getPalerSurface`, lines 62-76): convert `surface` to HSL; lightness -= 0.05 in dark mode, lightness += 0.02 in light mode; clamp lightness to [0,1].
- Back handling: `LocalBackPress` in the phone path pops the nested navigator if `canPop`, otherwise pops the parent (`PlayerSettingsScreen.kt:31-37`).
- Phone path uses `DefaultNavigatorScreenTransition`; tablet path uses `TwoPanelBox`, with `WindowInsets.systemBars.only(WindowInsetsSides.Horizontal)` consumed.
- `isTabletUi()` decides which path (`PlayerSettingsScreen.kt:27`).

**Proposal:**
- One layout only: a left sidebar list (same nine rows, same order, same title/subtitle keys) and a right stack. Selecting a row replaces the right-hand page. Back navigation is not needed for the sidebar; a header back button is shown only in the narrow layout (window width < 720 px) where the sidebar collapses into a page list.
- Keep the `mainSettings` flag: when `true` the title is "Player" (used as the top-level entry under the main settings), when `false` it is "Player settings".
- Search action pushes the cross-cutting settings search (area:47b-settings-widgets-search-more) pre-scoped to player settings.

---

## 2. Preference widget semantics

These semantics are shared by every screen in scope. They are defined in `app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt` and rendered by `app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt`.

**OBSERVED**

- `StatusWrapper` (`PreferenceItem.kt:50-71`): every item is wrapped in `AnimatedVisibility(visible = item.enabled)` with `expandVertically() + fadeIn()` enter and `shrinkVertically() + fadeOut()` exit. Note: `enabled == false` **hides** the item (it does not grey it out). Items whose `enabled` flag is false are removed from view. Highlight: `item.title == highlightKey` (set by search).
- `SwitchPreference`: value comes from `preference.collectAsState()`. Toggle calls `onValueChanged(newValue)`; if it returns `true`, `preference.set(newValue)` is called (`PreferenceItem.kt:76-94`).
- `SliderPreference`: `BaseSliderItem(label, value, valueRange, valueText = subtitle or value.toString(), steps)`; every change calls `onValueChanged(it)` inside `scope.launch`. **The screens in scope call `pref.set(it)` inside `onValueChanged` and return `true`** (for example `PlayerSettingsPlayerScreen.kt:190-193`). `steps` defaults to `(last - first) - 1` (`Preference.kt:` SliderPreference constructor).
- `ListPreference<T>`: dialog with one row per entry (`ListPreferenceWidget`). Selecting a non-selected row calls `onValueChange(key)`, which calls `onValueChanged` and then `set`; selecting the already selected row does nothing (`DialogRow`, `onClick = { if (!isSelected) onSelected() }`). Dialog has only a Cancel button. Subtitle default is `"%s"` formatted with `entries[value]`. **If the stored value is not a key in `entries`, the subtitle becomes the literal text "null"** (String.format with a null argument).
- `EditTextInfoPreference` (`EditTextPreferenceWidget.kt`): row subtitle `"%s"` formatted with the value (formatSubtitle is true for this item). Dialog: title + optional `dialogSubtitle`, one `OutlinedTextField` (single line unless overridden), trailing icon is an error icon when invalid (`(text.isBlank() && !canBeBlank) || !validate(text)`), otherwise a clear (Cancel icon) button that empties the field. Supporting text shows `errorMessage(text)` when `!validate(text)`. **OK is enabled iff** `text != currentValue && (text.isNotBlank() || canBeBlank) && validate(text)`. On OK: `onConfirm(text)` (which runs `onValueChanged` then `set` if accepted) then the dialog closes.
- `MultiLineEditTextPreference`: same dialog with `singleLine = false`; subtitle is supplied by the caller.
- `MPVConfPreference`: same dialog, `singleLine = false`, `canBeBlank = true` by default, `formatSubtitle = false`. Its default subtitle is the first two lines of the value, joined with `\n`, followed by `\n...` when the value has more than two lines (`Preference.kt` MPVConfPreference, `subtitle` default).
- `TextPreference(onClick)`: plain row; `onClick == null` means not tappable.
- `InfoPreference(title)`: non-interactive text row.
- `PreferenceGroup(title, preferenceItems)`: section header (group title) above items.
- `CustomPreference(content)`: arbitrary composable (not used in scope).

**Proposal:**
- Keep the same semantics, but show disabled items greyed out (not removed). Desktop users expect to see why a setting is unavailable. This is a deliberate deviation; record it in `p1-ui-parity.md`.
- Dialogs become modal `gtk::Window`s with a header bar (no libadwaita). Enter activates OK when enabled; Escape cancels.

---

## 3. Internal player settings page (`PlayerSettingsPlayerScreen`)

Top-level items in order (`PlayerSettingsPlayerScreen.kt:48-82`):

1. **Progress** `ListPreference` (`pref_progress_preference`, Float). Title `pref_progress_mark_as_seen` ("At what point to mark the episode as seen"). Entries (key → label): `1.00F` "100%", `0.95F` "95%", `0.90F` "90%", `0.85F` "85%", `0.80F` "80%", `0.75F` "75%", `0.70F` "70%". Default `0.85F`.
2. **Preserve watch position** switch (`pref_preserve_watching_position`, default `false`). Title `pref_preserve_watching_position` ("Preserve watch position on seen episodes").
3. **Orientation** `ListPreference` (`pref_default_player_orientation_type_key`, enum `PlayerOrientation`, default `SensorLandscape`). Entries in enum order: Free "Free", Video "Video", Portrait "Portrait", ReversePortrait "Reverse portrait", SensorPortrait "Sensor portrait", Landscape "Landscape", ReverseLandscape "Reverse landscape", SensorLandscape "Sensor landscape". Title `pref_category_player_orientation` ("Orientation").
4. Group **Controls** (`pref_category_controls`, "Controls"):
   - `pref_allow_gestures_in_panels` switch, default `false`, title `pref_controls_allow_gestures_in_panels` ("Allow gesture in panels").
   - `pref_show_loading` switch, default `true`, title `pref_controls_show_loading` ("Show loading circle").
   - `pref_show_current_chapter` switch, default `true`, title `pref_controls_show_chapter_indicator` ("Show current chapter indicator"), subtitle `pref_controls_show_chapter_indicator_info` ("Only appears if the video has chapters").
   - `pref_remember_brightness` switch, default `false`, title `pref_remember_brightness` ("Remember and switch to the last used brightness").
   - `pref_remember_volume` switch, default `false`, title `pref_remember_volume` ("Remember and switch to the last used volume"). **OBSERVED: no reader of this key exists in the codebase outside this screen.** The switch has no effect. See edge case E-PS-01.
5. Group **Hosters** (`pref_hosters`, "Hosters"):
   - `pref_show_failed_hosters` switch, default `false`, title `pref_hosters_show_failure` ("Display failed hosters").
   - `pref_show_empty_hosters` switch, default `false`, title `pref_hosters_show_empty` ("Display empty hosters").
6. Group **Display** (`MR.strings.pref_category_display`, "Display"):
   - `player_fullscreen` switch, default `true`, title `pref_player_fullscreen` ("Show content in display cutout"), `enabled = Build.VERSION.SDK_INT >= 28` (Android P). Observed: on Android it is only enabled on API 28+.
   - `player_hide_controls` switch, default `false`, title `pref_player_hide_controls` ("Hide player controls when opening the player").
   - `pref_display_vol_as_per` switch, default `true`, title `pref_controls_display_volume_percentage` ("Display volume as percentage").
   - `pref_show_system_status_bar` switch, default `false`, title `pref_show_system_bar` ("Show system status bar with controls").
   - `pref_reduce_motion` switch, default `false`, title `pref_reduce_motion` ("Reduce player animation").
   - `pref_player_time_to_disappear` `ListPreference<Int>`, default `4000`. Entries: 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, each label `pref_player_time_to_disappear_summary` formatted with the number ("%1$s ms" becomes "500 ms" etc.). Title `pref_player_time_to_disappear` ("Hide player controls time").
   - `pref_panel_opacity` `SliderPreference` 0..100, default `60`, title `pref_panel_opacity` ("Panel opacity"). Subtitle = `NumberFormat.getPercentInstance().format(value / 100f)` (for example "60%"; locale-dependent digits).
7. Group **Intro skip** (`pref_category_intro_skip`, "Intro skip"). Four controls plus AniSkip (`PlayerSettingsPlayerScreen.kt:199-259`):
   - `pref_enable_skip_intro` switch, default `true`, title `pref_enable_intro_skip` ("Enable intro skip").
   - `pref_enable_auto_skip_ani_skip` switch, default `false`, title `pref_enable_auto_skip_ani_skip` ("Enable auto skip"), `enabled = introSkipOn`.
   - `pref_enable_netflixStyle_aniskip` switch, default `false` (key spelled `pref_enable_netflixStyle_aniskip`), title `pref_enable_netflix_style_aniskip` ("Enable Netflix style"), `enabled = introSkipOn`.
   - `pref_waiting_time_aniskip` `ListPreference<Int>`, default `5`, entries 5→"5 seconds", 6→"6 seconds", 7→"7 seconds", 8→"8 seconds", 9→"9 seconds", 10→"10 seconds". Title `pref_waiting_time_aniskip` ("Button timeout"), `enabled = introSkipOn`.
   - `pref_enable_ani_skip` switch, default `false`, title `pref_enable_aniskip` ("Enable AniSkip"), `enabled = introSkipOn`.
   - `pref_disabled_ani_skip_chapters` switch, default `true`, title `pref_disable_aniskip_chapter` ("Disable AniSkip if video already contains chapters"), `enabled = introSkipOn && aniSkipOn`.
   - Info row `pref_category_player_aniskip_info` ("AniSkip requires the anime to be tracked with MAL or Anilist to work"), `enabled = introSkipOn`.
   - `introSkipOn` is the live value of `pref_enable_skip_intro`; `aniSkipOn` the live value of `pref_enable_ani_skip`.
8. Group **Picture-in-Picture** (`pref_category_pip`). Only added when `BasePreferences.deviceHasPip()` is true (`listOfNotNull`, line 77). Items:
   - `pref_enable_pip` switch, default `true`, title `pref_enable_pip` ("Enable the use of PiP mode").
   - `pref_pip_episode_toasts` switch, default `true`, enabled when PiP on.
   - `pref_pip_on_exit` switch, default `false`, enabled when PiP on.
   - `pip_replace_with_previous` switch, default `false` (key is `pip_replace_with_previous`, no `pref_` prefix), enabled when PiP on.
9. Group **External player** (`pref_category_external_player`):
   - `pref_always_use_external_player` switch, default `false`.
   - `external_player_preference` `ListPreference<String>`, default `""`. Entries: `"" → "None"` (literal English, not a string resource) plus every installed package from the fixed list below, mapping package name → application label (`pm.getApplicationLabel`). Computed at composition time from `getInstalledPackages(0)`.
   - Fixed package allowlist `externalPlayers` (`PlayerSettingsPlayerScreen.kt:333-346`): `is.xyz.mpv` (MPV_PLAYER), `com.mxtech.videoplayer` (MX_PLAYER), `com.mxtech.videoplayer.ad` (MX_PLAYER_FREE), `com.mxtech.videoplayer.pro` (MX_PLAYER_PRO), `org.videolan.vlc` (VLC_PLAYER), `live.mehiz.mpvkt` (MPV_KT), `live.mehiz.mpvkt.preview` (MPV_KT_PREVIEW), `com.husudosu.mpvremote` (MPV_REMOTE), `com.brouken.player` (JUST_PLAYER), `dev.anilbeesetti.nextplayer` (NEXT_PLAYER), `video.player.videoplayer` (X_PLAYER), `com.instantbits.cast.webvideo` (WEB_VIDEO_CASTER). Constants are at `app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt:599-610`.

Consumers (verified by grep): `alwaysUseExternalPlayer` is read by `EntryBottomActionMenu.kt`, `AnimeUpdatesScreen.kt`, `AnimeScreenModel.kt`, `AnimeHistoryTab.kt`, `AnimeLibraryTab.kt`, `AnimeUpdatesTab.kt`. `externalPlayerPreference` is read by `ExternalIntents.kt:107` (empty → plain `ACTION_VIEW` intent; otherwise `getIntentForPackage`).

Player-side consumers of the other keys (verified): `PlayerActivity.kt` reads `pip_*`, `enablePip`, `playerFullscreen`, `hideControls`, `rememberPlayerBrightness`, `aniSkipEnabled`, `disableAniSkipOnChapters`, `waitingTimeIntroSkip`, `defaultPlayerOrientationType`; `PlayerViewModel.kt` reads `enableSkipIntro`, `autoSkipIntro`, `enableNetflixStyleIntroSkip`, `waitingTimeIntroSkip`, `progressPreference`, `showSystemStatusBar`, `defaultPlayerOrientationType`; `PlayerControls.kt` reads `showFailedHosters`, `showEmptyHosters`, `showLoadingCircle`, `displayVolPer`, `reduceMotion`, `playerTimeToDisappear`, `hideControls`, `invertDuration`; `PlayerPanels.kt` reads `panelOpacity`; `BottomLeftPlayerControls.kt` reads `showCurrentChapter`.

---

## 4. Gestures page (`PlayerSettingsGesturesScreen`)

Four groups in order: Sliders, Seeking, Double tap, Media controls (`PlayerSettingsGesturesScreen.kt:45-50`).

**Sliders** (`pref_category_player_sliders`):
- `pref_gesture_volume_brightness` switch, default `true`, title `enable_volume_brightness_gestures` ("Enable Volume and Brightness Gestures").
- `pref_swap_volume_and_brightness` switch, default `false`, title `pref_controls_swap_vol_brightness` ("Swap volume and brightness slider").

**Seeking** (`pref_category_player_seeking`):
- `pref_gesture_horizontal_seek` switch, default `true`, title `enable_horizontal_seek_gesture` ("Enable Horizontal Seek Gesture"), subtitle `enable_horizontal_seek_gesture_summary` ("Swipe left or right to rewind or fast-forward").
- `pref_show_seekbar` switch, default `false`, title `pref_show_seekbar` ("Show seekbar when seeking").
- `pref_default_intro_length` (int, default `85`). Shown as `TextPreference` with title `pref_default_intro_length` ("Default skip intro length") and subtitle `"${value}s"`. Click opens `SkipIntroLengthDialog`:
  - Title `pref_intro_length` ("Skip intro length").
  - Body: `WheelTextPicker` over `0..255`, each item `MR.strings.seconds_short` formatted with the integer ("%ds" → "85s"), `startIndex` = current value.
  - OK writes the chosen integer with `defaultIntroLength().set(...)`; Cancel dismisses.
  - Observed detail: the local `newLength` starts at `0`, but `WheelPicker` emits the snapped index on first composition (`snapshotFlow` emits its initial value and `distinctUntilChanged` passes it), so pressing OK without scrolling saves the start value. The Rust port must initialise the selection explicitly to the current value (do not rely on this).
- `pref_skip_length_preference` (int, default `10`), title `pref_skip_length` ("Double tap to skip length"). Entries in order: 30 "30s", 20 "20s", 10 "10s", 5 "5s", 3 "3s", 0 "Disable" (`pref_skip_disable`).
- `pref_player_smooth_seek` switch, default `false`, title `pref_player_smooth_seek` ("Enable precise seeking"), subtitle `pref_player_smooth_seek_summary` ("When enabled, seeking will not focus on keyframes, leading to slower but precise seeking").

**Double tap** (`pref_category_double_tap`):
- `pref_left_double_tap` (`SingleActionGesture`, default `Seek`). Allowed options in order: None, Seek, PlayPause, Switch, Custom. Title `pref_left_double_tap` ("Double tap (left)").
- `pref_center_double_tap` (default `PlayPause`). Allowed: None, PlayPause, Custom. Title "Double tap (center)".
- `pref_right_double_tap` (default `Seek`). Allowed: None, Seek, PlayPause, Switch, Custom. Title "Double tap (right)".
- Info row `pref_double_tap_info`: "When a tap gesture is set to "Custom", it can be bound through input.conf. The key codes are 0x10001 for left, 0x10002 for center, and 0x10003 for right."

**Media controls** (`pref_category_media_controls`):
- `pref_media_previous` (default `Switch`). Allowed: None, Seek, PlayPause, Switch, Custom. Title "Previous".
- `pref_media_playpause` (default `PlayPause`). Allowed: None, PlayPause, Custom. Title "Play/Pause".
- `pref_media_next` (default `Switch`). Allowed: None, Seek, PlayPause, Switch, Custom. Title "Next".
- Info row `pref_media_info`: "When a media control is set to "Custom", it can be bound through input.conf. The key codes are 0x10004 for previous, 0x10005 for play/pause, and 0x10006 for next."

Labels from `SingleActionGesture.stringRes`: None → "None" (`single_action_none`), Seek → "Seek", PlayPause → "Play/Pause", Switch → "Change episode" (`single_action_switch`), Custom → "Custom". (Source: `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt:` SingleActionGesture.)

Runtime dispatch semantics for these values are in `p1-topic-custom-buttons.md` section "Gesture dispatch". Observed: the `Custom` action does not run a Lua function; it only sends an Android key code (`CustomKeyCodes`: DoubleTapLeft `0x10001`, DoubleTapCenter `0x10002`, DoubleTapRight `0x10003`, MediaPrevious `0x10004`, MediaPlay `0x10005`, MediaNext `0x10006`) that the user binds in `input.conf`.

Consumers: `GestureHandler.kt` (`gestureVolumeBrightness`, `swapVolumeBrightness`, `gestureHorizontalSeek`, `showSeekBar`, `playerSmoothSeek`), `PlayerViewModel.kt` (double-tap actions, skip length, smooth seek, show seek bar), `PlayerActivity.kt` (media control actions), `AnimeScreen.kt` (`defaultIntroLength`).

---

## 5. Decoder page (`PlayerSettingsDecoderScreen`)

Items in order (`PlayerSettingsDecoderScreen.kt:31-54`):
1. `pref_try_hwdec` switch, default `true`, title `pref_try_hw` ("Try hardware decoding").
2. `pref_gpu_next` switch, default `false`, title `pref_gpu_next_title` ("Enable gpu-next"), subtitle `pref_gpu_next_subtitle` ("A new video rendering backend").
3. `pref_video_debanding` `ListPreference<Debanding>`, default `Debanding.None`. Entries are the enum constant names as-is (`it.name`), not localised: "None", "CPU", "GPU" (the commented-out `stringResource(it.)` shows the intent was never finished). Title `pref_debanding_title` ("Debanding").
4. `use_yuv420p` switch, default `true`, title `pref_use_yuv420p_title` ("Use YUV420P pixel format"), subtitle `pref_use_yuv420p_subtitle` ("May fix black screens on some video codecs, can also improve performance at the cost of quality").

Application to mpv (`app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:114-131`, function `initOptions`):
- `setVo(if gpuNext "gpu-next" else "gpu")`.
- `setPropertyBoolean("pause", true)`, `setOptionString("profile", "fast")`.
- `setOptionString("hwdec", if tryHW "auto" else "no")`.
- Debanding: `None` → nothing; `CPU` → `setOptionString("vf", "gradfun=radius=12")`; `GPU` → `setOptionString("deband", "yes")`.
- `if useYUV420P: setOptionString("vf", "format=yuv420p")`.
- `msg-level` = `"all=" + (verboseLogging ? "v" : "warn")`.

**OBSERVED BUG (must not be ported):** the `vf` option is assigned twice. With `Debanding.CPU` and `useYUV420P == true` (the default), the second assignment replaces the first, so the CPU debanding filter is silently dropped. Proposal: build the `vf` list from all active filters and join with commas (see `p1-rust-design.md`, `vf::build_chain`).

The `Decoder` enum (`PlayerEnums.kt`: AutoCopy "auto-copy", Auto "auto", SW "no", HW "mediacodec-copy", HWPlus "mediacodec") is not offered by any screen in scope. Dropped for Linux (see `p1-rust-design.md` section "Decoder mapping").

---

## 6. Subtitle page and the language-code validator

Items (`PlayerSettingsSubtitleScreen.kt:30-83`):
1. `pref_subtitle_lang` `EditTextInfoPreference`, default `""`. Title `pref_player_subtitle_lang` ("Preferred languages"). Dialog subtitle `pref_player_subtitle_lang_info` ("Subtitle language(s) to be selected by default on a video with multiple subtitles, Two- or three-letter languages codes work. Multiple values can be delimited by a comma."). Validation: language validator (below). Error message: `pref_player_subtitle_invalid_lang` = `Invalid code: "%s"` with the first invalid code.
2. `pref_subtitle_whitelist`, default `""`. Title `pref_player_subtitle_whitelist` ("Whitelist"). Dialog subtitle `pref_player_subtitle_whitelist_info` ("Whitelist for subtitles. If a whitelist is defined, the first subtitle that contains a whitelisted word will be used. Multiple values can be delimited by a comma."). **No validation.**
3. `pref_subtitle_blacklist`, default `""`. Title `pref_player_subtitle_blacklist` ("Blacklist"). Dialog subtitle `pref_player_subtitle_blacklist_info` ("Blacklist for subtitles. If a blacklist is defined, all subtitles that contains a blacklisted word will be filtered out. Multiple values can be delimited by a comma."). **No validation.**

**Language-code validator (exact algorithm, same for audio and subtitle pages)**
`PlayerSettingsSubtitleScreen.kt:35-82`, `PlayerSettingsAudioScreen.kt:41-76`:

```
langs = pref.split(",").filter(String::isNotEmpty).map(String::trim)
for each code in langs (in order):
    try:
        locale = Locale(code)
        if locale.isO3Language == locale.language && locale.language == locale.getDisplayName(Locale.ENGLISH):
            throw MissingResourceException
    catch MissingResourceException:
        INVALID(code)            // validate -> false; errorMessage -> "Invalid code: \"code\""
VALID                            // validate -> true; errorMessage -> ""
```

Observed consequences:
- `""` (empty) is valid (no items).
- Filter happens before trim, so `"en, "` yields the element `" "` which passes the filter, trims to `""`, and is then **invalid** (`Locale("")` gives an empty 3-letter code and an empty display name, so the equality test passes and the code is rejected). Error text: `Invalid code: ""`.
- Codes are case-insensitive (`Locale` lower-cases the language), so `"EN"` is valid.
- Unknown two-letter codes such as `"xx"` throw `MissingResourceException` from `getISO3Language`, so they are invalid.
- Three-letter codes (`"eng"`) depend on the JDK's English display-name table. Treat this as **unverified**; the Rust port must make a deliberate choice (see `p1-rust-design.md` section "Language codes").
- The error message shows the trimmed `it` value, not the raw input.

**Proposal (Rust):** `lang::validate_list(s: &str) -> Result<(), InvalidCode>` implementing the same split/filter/trim order, accepting ISO 639-1 (two-letter) and ISO 639-2/T (three-letter) codes from a static table, lower-casing before lookup, and rejecting the empty string produced by whitespace-only elements. Returns the first invalid code for the message.

---

## 7. Audio page (`PlayerSettingsAudioScreen`)

Items in order (`PlayerSettingsAudioScreen.kt:36-100`):
1. `pref_audio_lang` `EditTextInfoPreference`, default `""`, title `pref_player_audio_lang` ("Preferred languages"), dialog subtitle `pref_player_audio_lang_info` ("Audio language(s) to be selected by default on a video with multiple audio streams, Two- or three-letter languages codes work. Multiple values can be delimited by a comma."). Same validator as section 6.
2. `pref_audio_pitch_correction` switch, default `true`, title `pref_player_audio_pitch_correction` ("Enable audio pitch correction"), subtitle `pref_player_audio_pitch_correction_summary` ("Prevents the audio from becoming high-pitched at faster speeds and low-pitched at slower speeds").
3. `pref_audio_config` `ListPreference<AudioChannels>`, default `AutoSafe`, title `pref_player_audio_channels` ("Audio channels"). Entries in enum order with labels: `Auto` "Auto", `AutoSafe` "Auto-safe", `Mono` "Mono", `Stereo` "Stereo", `ReverseStereo` "Reverse stereo".
4. `pref_audio_volume_boost_cap` `SliderPreference`, range `0..200`, default `30`, title `pref_player_audio_boost_cap` ("Volume boost cap"), subtitle = the integer value.

Enum values (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt`):

| Constant | Label | Property set | Value set |
|---|---|---|---|
| Auto | Auto | `audio-channels` | `auto-safe` |
| AutoSafe | Auto-safe | `audio-channels` | `auto` |
| Mono | Mono | `audio-channels` | `mono` |
| Stereo | Stereo | `audio-channels` | `stereo` |
| ReverseStereo | Reverse stereo | `af` | `pan=[stereo\|c0=c1\|c1=c0]` |

**OBSERVED:** the labels and mpv values are crossed for `Auto` and `AutoSafe`. The user-facing "Auto" sends mpv `auto-safe`, and "Auto-safe" sends mpv `auto`. The stored key is the constant **name** (see section 16 / data file), so the mapping is fixed by code. Proposal: keep the stored names, but map "Auto" to mpv `auto` and "Auto-safe" to mpv `auto-safe` (the label now matches the value). Record this in the release notes as a deliberate fix.

Application (`AniyomiMPVView.kt:255-258`, `PlayerActivity.kt` `setupPlayerAudio`):
- `alang` = preferred languages string (raw, not validated again).
- `audio-delay` = `audioDelay / 1000.0` (audio delay is a non-preference int, default `0`, key `pref_audio_delay`).
- `audio-pitch-correction` = `true` or `false`.
- `volume-max` = `volumeBoostCap + 100` (so the slider 0..200 maps to volume-max 100..300; default 30 gives 130).
- Audio channels applied at player setup via `setPropertyString(property, value)`.

Audio focus (PlayerActivity, Android-only): `AUDIOFOCUS_GAIN`, usage MEDIA, content type MUSIC. Not ported (see area:45-player-core). Linux: rely on PipeWire/PulseAudio session policy (note in P3 task of area 45).

---

## 8. Advanced page (`PlayerSettingsAdvancedScreen`)

Items (`PlayerSettingsAdvancedScreen.kt:28-52`):
1. `mpv_scripts` switch, default `false`, title `pref_mpv_user_files` ("Use user MPV files"), subtitle `pref_mpv_user_files_summary` ("Use files in mpv-config directory"). Observed: the key name `mpv_scripts` is historical; it controls whether user files (scripts, script-opts, shaders) are copied into the player's config dir.
2. `pref_mpv_conf` `MPVConfPreference`, default `""`, `fileName = "mpv.conf"`, title `pref_mpv_conf` ("Edit MPV configuration file for further player settings").
3. `pref_mpv_input` `MPVConfPreference`, default `""`, `fileName = "input.conf"`, title `pref_mpv_input` ("Edit MPV input file for keyboard mapping configuration").

`MPVConfPreference.onValueChanged` (`Preference.kt` MPVConfPreference):
```
if fileName != null:
    if SDK >= R and Environment.isExternalStorageManager():
        file = storageManager.getMPVConfigDirectory().createFile(fileName)
        write the new text to file (fd "rwt")
    preference.set(value)      // done by the caller after onValueChanged returns true
return true
```
**OBSERVED:** when the all-files permission is absent, the text is still saved to the preference (the caller sets it because the function returns `true`), but no external file is written. When it is present, the external file in `mpv-config/` is written, **but the player never reads that external file**. `PlayerActivity.setupPlayerMPV` writes `filesDir/mpv/mpv.conf` and `input.conf` from the preference values on every player start (lines 428-432). The external copies are a read-only mirror for the user to look at, not an input.

Proposal: the preference (text in the database) is the only source of truth. Expose "Open mpv-config folder" to let the user reach the scripts directory. Do not write a mirror file.

---

## 9. mpv runtime directory sync (`PlayerActivity.setupPlayerMPV`)

Called at player start. Order is observable and must be preserved (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:420-466`, `:519-528`):

1. `mpvDir = filesDir/mpv` (`MPV_DIR = "mpv"`, line 176). Created if missing.
2. Write `mpv.conf` (overwrite) from `pref_mpv_conf`; write `input.conf` (overwrite) from `pref_mpv_input`.
3. `copyUserFiles(mpvDir)`:
   a. Delete `mpvDir/scripts`, `mpvDir/script-opts`, `mpvDir/shaders` recursively (directories are recreated by `createDirectory` calls).
   b. If `mpv_scripts == true`: copy every file in `<base>/mpv-config/scripts` into `mpvDir/scripts`, every file in `<base>/mpv-config/script-opts` into `mpvDir/script-opts`, every file in `<base>/mpv-config/shaders` into `mpvDir/shaders`. Copy is `file.openInputStream().copyTo(outFile.openOutputStream())` with **no close()** on either stream and **no size or mtime check**.
   c. Always: create `mpvDir/scripts/aniyomi.lua` from the bundled asset `app/src/main/assets/aniyomi.lua` (90 lines, the bridge; see topic file).
4. `copyAssets(mpvDir)`: for `subfont.ttf` and `cacert.pem` open the asset with `ACCESS_STREAMING`, create the output file, skip the copy when `outFile.length() == ins.available()`, otherwise copy. `IOException` is logged at ERROR and swallowed.
5. `copyFontsDirectory(mpvDir)` runs **asynchronously** on `Dispatchers.IO` (`launchIO`): creates `mpvDir/fonts`, copies each file from `<base>/mpv-config/fonts`, then sets mpv properties `sub-fonts-dir` and `osd-fonts-dir` to the fonts path. Observed race: the first frame may render before fonts are copied. Comment in source calls this "a bad hack".
6. `setOptionString("sub-ass-force-margins", "yes")` and `setOptionString("sub-use-margins", "yes")`.
7. `player.initialize(configDir = mpvDir.filePath, cacheDir = cacheDir, logLvl = verboseLogging ? "info" : "warn")`.

Then `setupCustomButtons(buttons)` writes `mpvDir/scripts/custombuttons.lua` and runs `load-script` (see topic file).

**Proposal:** the Rust app does not need a copy step. Point mpv at `<data>/mpv-config` directly (`config-dir`, `scripts`, `script-opts`, `shaders`, `sub-fonts-dir`), write only the generated files (`aniyomi.lua`, `custombuttons.lua`, and `mpv.conf` / `input.conf` from the database) into a private runtime directory under `$XDG_RUNTIME_DIR` or the cache directory, and skip the deletion step (it deletes user-visible copies). Keep "user files" as a switch: when off, mpv loads only the generated scripts.

---

## 10. Script editor: file list (`PlayerSettingsEditorScreen`, `PlayerSettingsEditorScreenModel`)

**Types** (`EditorListType`, `PlayerSettingsEditorScreenModel.kt:159-162`):

| Constant | Directory (`SCRIPTS_PATH` / `SCRIPT_OPTS_PATH`) | Title | Extension for new files |
|---|---|---|---|
| SCRIPTS | `scripts` | `pref_player_editor_script` ("Scripts") | `lua` |
| SCRIPTS_OPTS | `script-opts` | `pref_player_editor_script_opts` ("Script options") | `conf` |

Constants: `domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt` (`SCRIPTS_PATH = "scripts"`, `SCRIPT_OPTS_PATH = "script-opts"`).

**Loading** (`getEditorListItems`):
1. Get `<base>/mpv-config/<dir>`, creating it if missing; if the base directory is unavailable, return an empty list.
2. `listFiles()`; skip directories; for each file: `name = file.name ?: ""`, `size = file.size().toSize()` (`StorageUtil.kt:23` sums file lengths for directories; for a file it is its length), `lastModified = file.lastModified()` formatted with `SimpleDateFormat("MMMM d, yyyy HH:mm", Locale.getDefault())` or `null` when the timestamp is `-1`.
3. **No sort** is applied. The order is whatever the filesystem returns (observed).
4. `Toast`/`toSize` formats: `>= 1000^3` → "%.2f GB", `>= 1000^2` → "%.2f MB", `>= 1000` → "%.2f KB", else "<n> B". Decimal units (1000), not 1024.

**State**: `Loading` (initial) → `Success(editorListItems)`. Empty list shows `pref_player_no_items` ("Directory contains no items. Tap the plus button to add a new file.").

**Type switch**: the app-bar title is a dropdown of the two types. Selecting one sets `selectedType` and rebuilds the list (`collectLatest` on the type flow, `launchIO`).

**Row**: leading icon by extension (`lua` → `IntegrationInstructions`, `conf` → `SettingsApplications`, other → `Description`); headline = file name; supporting text = `listOfNotNull(size, lastModified).joinToString(" - ")`; tap → open code editor with path `"<dir>/<name>"`; trailing `MoreHoriz` icon opens a menu with `editor_action_rename` ("Rename file") and `editor_action_delete` ("Delete file").

**FAB**: add, opens the create dialog.

**Create** (`createFile(fileName)`):
- Dialog `FileCreateDialog(initialName = null, fileExtension = selectedType.fileExtension)`: title `editor_create_file` ("Create file"); label `editor_filename` ("Filename"); confirm button `action_add` ("Add"); initial text `"file.<ext>"`, i.e. `file.lua` or `file.conf`; initial selection = `[0, lastIndexOf('.'))`, i.e. `file` selected (if there is no dot the whole text is selected).
- On confirm: `storageManager.getMPVConfigDirectory()?.createDirectory(dir)?.createFile(fileName)`. If any step returns null, toast `editor_create_error` ("Unable to create file") and return. Then reload. The result of `createFile` is not checked further.

**Rename** (`editFile(originalFile, fileName)`):
- Dialog with `initialName = name`, title `editor_edit_file` ("Edit filename"), confirm `action_edit` ("Edit"). Initial text is the current file name and selection covers the base name.
- Implementation: `file = directory.createFile(originalFile)` then `file.renameTo(fileName)`. On `true` reload; otherwise toast `editor_rename_error` ("Unable to rename file").
- **No extension enforcement.** A `.lua` can be renamed to `.txt`.

**Delete** (`deleteFile(name)`): `directory.findFile(name)?.delete() == true` → reload; otherwise toast `editor_delete_error` ("Unable to delete file"). Confirmation dialog: title `editor_delete_file` ("Delete file"), body `editor_delete_file_confirmation` ("Do you wish to delete the file \"%s\"?"), confirm `action_ok`, cancel `action_cancel`.

**Validation** (`isValidName(name, initialName)`), applied live on each keystroke (`remember(fileName.text)`):
1. `names` = current list names, excluding an entry equal to `initialName` (case-sensitive exclusion).
2. If any existing name equals `name` **ignoring case** → Failure `editor_file_already_exists` ("File already exists").
3. If `name != DiskUtil.buildValidFilename(name)` → Failure `editor_invalid_filename` ("Invalid filename").
4. Otherwise Success. The confirm button is enabled only on Success.

`DiskUtil.buildValidFilename` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:109-125`): `trim('.', ' ')` from both ends; empty → `"(invalid)"`; each char in `0x00..0x1f`, `"`, `*`, `/`, `:`, `<`, `>`, `?`, `\`, `|`, `0x7f` is replaced by `_`; result is `take(240)`. Consequences: a leading dot (`.x.lua`) or trailing space makes the name invalid; a slash cannot be typed.

Error text in the dialog is shown in supporting text, and the text field is marked `isError`.

**Dialog focus**: `LaunchedEffect` waits `0.1 s` then requests focus (workaround for Google issue 204502668).

---

## 11. Code editor and syntax highlighter (`CodeEditScreen`, `CodeEditScreenModel`)

**Entry**: `CodeEdit(filePath)`, `filePath = "<dir>/<name>"`. App-bar title = `filePath.substringAfter("/")` (the file name only).

**Load** (`CodeEditScreenModel.init`, `launchIO`):
1. `file = getMPVConfigDirectory()?.findFile(filePath) ?: throw Exception("Unable to read file")`. Note `findFile` is called with a slash-containing path (`"scripts/x.lua"`). Whether this resolves on SAF-backed base directories is **unverified** (UniFile internals not in repo). The Rust port uses real paths and does not have this question.
2. `currentFile = file`.
3. `content = file.openInputStream().use { readBytes().toString(UTF_8) }`. Invalid UTF-8 bytes become U+FFFD, and a later save writes the replacement characters back (silent data change for non-UTF-8 files; edge E-CE-03).
4. State `Success(TextFieldValue(annotatedString = highlight(content)))`.
5. Any exception → state `Error(e)`; the screen shows `EmptyScreen(message = e.message ?: "Unknown exception")`.

**Highlighting** (`highlightText`):
- If `text.length > HIGHLIGHT_MAX_SIZE` (= `15000` characters, `CodeEditScreenModel.kt:133`) → no highlighting (plain `AnnotatedString`).
- Else by the extension after the last dot of `filePath`: `lua` → Lua rules; `conf` → conf rules; anything else → plain.
- Highlighting is re-run on every text change (`onEdit`), with the same limit.

**Editing** (`onEdit(value)`):
- If `value.text != currentText`: set `hasModified = true`, re-highlight, keep `selection` from `value`, `composition = null`.
- Else: keep the old annotated string, update only the selection.
- `hasModified` is never reset except by a successful save.

**Save** (`save()`):
1. `file = currentFile` or toast `editor_save_error` ("Unable to save file") and return.
2. `content = state.content.annotatedString.text` or the same toast and return.
3. `file.openOutputStream()`, truncate the underlying `FileOutputStream` channel to 0, write `content.toByteArray()` (UTF-8, platform default charset is not used because `toByteArray()` defaults to UTF-8 in Kotlin), close.
4. Success → `hasModified = false` and toast `editor_save_success` ("File saved").
5. Failure → `logcat(ERROR, e)` and toast `e.message` or `editor_save_error`.

Save is not atomic: truncate happens before write. A crash mid-write loses the file. Proposal: write to a temporary file in the same directory and `rename` over the target (atomic on Linux).

**Back navigation**: both the app-bar up arrow and the system back (`BackHandler(enabled = hasModified)`) show `UnsavedChangesDialog` when modified. Dialog: title `label_warning` ("Warning"), body `editor_unsaved_progress` ("You have unsaved changes. Do you still wish to go back?"), OK = pop, Cancel = dismiss.

**App bar**: save action with `Icons.Outlined.Save`, title `action_save` ("Save"), `enabled = hasModified`.

**Layout** (`CodeEditorContent`):
- Monospace text style: 14 sp font size, 20 sp line height, colour `onBackground`.
- Two text fields in a `Row` inside a vertical scroll, with `imePadding()`:
  - Left gutter: read-only text of `1..lineCount` joined with `\n`, right aligned, colour `onBackground` at `DISABLED_ALPHA`; width `12.dp * lineCount.toString().length`.
  - Right editor: fills height, horizontal scroll, cursor colour `primary`. `lineCount` is taken from `onTextLayout`.
- Requests focus on first composition.

---

## 12. Custom buttons (summary)

Full detail: `p1-topic-custom-buttons.md`. Essentials:
- Table `custom_buttons` (`data/src/main/sqldelightanime/dataanime/custom_buttons.sq`, created by migration `data/src/main/sqldelightanime/migrations/129.sqm`). Seed row `_id = 1`, name `+85 s`, favourite, sortIndex 0 (the intro-skip button).
- Screen lists buttons ordered by `sortIndex`; drag to reorder; star marks the single favourite (primary) button; edit and delete per row; FAB adds.
- Only the favourite is shown in the player as the primary button; its title is `name`, unless a Lua script sets it.
- Lua code is written to `mpvDir/scripts/custombuttons.lua` at player start for **all** buttons.

## 13. mpv Lua bridge (summary)

Full detail in `p1-topic-custom-buttons.md`, section "Bridge protocol". The bundled `aniyomi.lua` exposes functions that write to mpv user-data properties `user-data/aniyomi/<action>`; the player observes these, dispatches, and clears them by setting the property to `""`.

## 14. TorrServer settings page

Full detail in `p1-topic-torrent-settings.md`. Summary: one enable switch (first enable shows a legal notice once), port (0..65535, default `8090`), trackers (26-line default), proxy mode (None/Tracker/Peers/Full), proxy URL (http, https, socks4, socks4a, socks5, socks5h).

---

## 15. Global UI preferences (`UiPreferences`, enums in `app/src/main/java/eu/kanade/domain/ui/model/`)

Keys and defaults (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:20-41`):

| Accessor | Key | Type | Default | Notes |
|---|---|---|---|---|
| `themeMode()` | `pref_theme_mode_key` | enum `ThemeMode` | `SYSTEM` | LIGHT → `MODE_NIGHT_NO`, DARK → `MODE_NIGHT_YES`, SYSTEM → `MODE_NIGHT_FOLLOW_SYSTEM` (`ThemeMode.kt:11-18`) |
| `appTheme()` | `pref_app_theme` | enum `AppTheme` | `MONET` if dynamic colour is available, else `DEFAULT` | |
| `themeDarkAmoled()` | `pref_theme_dark_amoled_key` | boolean | `false` | pure black dark theme |
| `relativeTime()` | `relative_time_v2` | boolean | `true` | "2 minutes ago" style |
| `dateFormat()` | `app_date_format` | string | `""` | empty = `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)`; otherwise `ofPattern(format, Locale.getDefault())` |
| `tabletUiMode()` | `tablet_ui_mode` | enum `TabletUiMode` | `AUTOMATIC` | dropped on desktop |
| `startScreen()` | `start_screen` | enum `StartScreen` | `ANIME` | |
| `navStyle()` | `bottom_rail_nav_style` | enum `NavStyle` | `MOVE_HISTORY_TO_MORE` | |

Enum constants and labels:
- `ThemeMode`: LIGHT, DARK, SYSTEM (no labels in this file; the screen supplies them).
- `AppTheme` titles: DEFAULT "Default", MONET "Dynamic", CLOUDFLARE "Cloudflare", COTTONCANDY "Cotton Candy", DOOM "Doom", GREEN_APPLE "Green Apple", LAVENDER "Lavender", MATRIX "Matrix", MIDNIGHT_DUSK "Midnight Dusk", MOCHA "Mocha", SAPPHIRE "Sapphire", NORD "Nord", STRAWBERRY_DAIQUIRI "Strawberry Daiquiri", TAKO "Tako", TEALTURQUOISE "Teal & Turquoise", TIDAL_WAVE "Tidal Wave", YINYANG "Yin & Yang", YOTSUBA "Yotsuba", MONOCHROME "Monochrome". DARK_BLUE, HOT_PINK, BLUE are deprecated with no title (`titleRes = null`).
- `StartScreen` titles: ANIME "Anime" (`label_anime`), MANGA "Manga", UPDATES "Updates" (`label_recent_updates`), HISTORY "History" (`label_recent_manga`), BROWSE "Browse".
- `TabletUiMode` titles: AUTOMATIC "Auto" (`automatic_background`), ALWAYS "Always" (`lock_always`), LANDSCAPE "Landscape", NEVER "Never" (`lock_never`).
- `NavStyle` (`titleRes`): MOVE_MANGA_TO_MORE "Move Manga to the More tab"; MOVE_UPDATES_TO_MORE "Move Updates to the More tab"; MOVE_HISTORY_TO_MORE "Move History to the More tab"; MOVE_BROWSE_TO_MORE "Move Browse to the More tab". Moved tab per variant: MangaLibraryTab, UpdatesTab, HistoriesTab, BrowseTab.

`NavStyle.tabs` (`NavStyle.kt:40-50`): start from `[AnimeLibraryTab, MangaLibraryTab, UpdatesTab, HistoriesTab, BrowseTab, MoreTab]`, remove the variant's `moreTab`. The result is the five bottom-bar tabs. `MoreTab` is always present. Icon for the moved tab when it lives in More: MANGA → `Icons.Outlined.CollectionsBookmark`, UPDATES → `R.drawable.ic_updates_outline_24dp`, HISTORY → `Icons.Outlined.History`, BROWSE → `Icons.Outlined.Explore`.

`dateFormat(format)` is used by `app/src/main/java/eu/kanade/presentation/components/DateText.kt:52`.

---

## 16. Storage root and directory layout (`StorageManager`)

`domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt`. The base directory is a user-chosen tree (`baseStorageDirectory` preference). When it changes, the manager creates (`createDirectory`, lazy) these children:
- `autobackup`, `local`, `localanime`, `downloads` (plus a `.nomedia` file via `DiskUtil.createNoMediaFile`), `mpv-config`.
- Inside `mpv-config`: `fonts`, `scripts`, `script-opts`, `shaders`.

Getters create lazily: `getMPVConfigDirectory()` → `<base>/mpv-config`; `getFontsDirectory()`, `getScriptsDirectory()`, `getScriptOptsDirectory()`, `getShadersDirectory()` → the children. Returns `null` when the base is unavailable (`getBaseDir` requires `UniFile.exists()`).

## 17. Interactions with other areas

- area:02-preferences-system: `PreferenceStore` (`core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt`); `getEnum` stores `enum.name` and falls back to the default on unknown names (lines 27-42). `getObject` is used for enums.
- area:04-db-anime-schema-migrations: `custom_buttons` table and migration 129.
- area:11-app-migrations: legacy key migrations that touch keys in this scope: `VideoOrientationMigration` (version 127, maps old int orientation to `PlayerOrientation`), `VideoPlayerPreferenceMigration` (126), `PlayerPreferenceMigration` (92), `MovePlayerPreferencesMigration` (93), `PrefLangMigration` (130, filters stored language lists with the same validator). Desktop does not carry Android preference migrations; see task T-48a-055 notes.
- area:13-core-util-storage-archive-torrent: `aniyomi.core.common.torrent.TorrentPreferences` and `ProxyMode`, `TorrentServerUtils`, `DiskUtil`, `StorageUtil`.
- area:21-backup-format-and-create and area:22-backup-restore: `BackupCustomButtons` (proto fields in `p1-data-and-config.md`), `LegacyBackup` field 109 and `Backup` field 506.
- area:45-player-core: mpv option application, AniSkip network calls, PlayerViewModel behaviour for the Lua bridge, primary button semantics in `PlayerViewModel.kt:315-345, 884-945`.
- area:46-player-controls-ui: panels (`PlayerPanels.kt`), sheets, gesture handler, button rendering.
- area:47b-settings-widgets-search-more: `SearchableSettings`, `PreferenceScaffold`, `SettingsSearchScreen`.
- area:33 is not involved. Trackers are not part of this area.
