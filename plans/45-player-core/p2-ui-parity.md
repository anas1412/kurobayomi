# 45-player-core (part 2 of 2) — UI parity (state-driven surfaces)

This part's files do not render widgets. They publish the state that part 1 widgets render. This document lists every user-visible state this part produces, what the Kotlin UI does with it, and what the Kurobayomi mockup (`src/player.rs`) lacks. Widget layout of the controls belongs to part 1 (`PlayerControls.kt`, `GestureHandler.kt`, sheets) and is not repeated here.

Source of state: `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` (state flows named below). Message keys and English text: `p2-behavior.md` §18.

## 1. Surfaces driven by this part

### 1.1 Toasts / snackbars (one-shot)
| Trigger | Kotlin | Text | Desktop equivalent |
|---|---|---|---|
| Sleep timer ends | `withUIContext { toast(toast_sleep_timer_ended) }` (:362) | "Sleep timer ended" | `pill` overlay (2.5 s), see §3 |
| Previous episode unavailable | `activity.showToast(no_prev_episode)` (:1036) | localized (base file Arabic) | `pill` |
| Next episode unavailable | `showToast(no_next_episode)` (:1041) | "Next Episode not found!" | `pill` |
| Netflix countdown starts | `showToast("Skip Intro: " + "Skipping %s in %d seconds")` (:2082) | "Skip Intro: Skipping <chapter> in 5 seconds" (prefix hard-coded) | `pill` |
| Auto-play toggled | `playerUpdate = ShowTextResource(enable/disable_auto_play)` (:808) | "Auto-play is on/off" | `pill` (already in mockup) |
| Show text (Lua) | `playerUpdate = ShowText(data)` (:863) | arbitrary | `pill` |
| Save / share / set-art result | `Event.SavedImage`, `ShareImage`, `SetArtResult` (eventFlow) | notifier strings (area:43) | `notify` + toast |
| Failed to start hoster load | `ExceptionWithStringResource` → error screen in part 1 | "No available videos", "Hoster list is empty", etc. | error panel with retry |

Behavioural rule: a new one-shot replaces the previous one (Kotlin `playerUpdate` is a single StateFlow value, so two messages in the same frame collapse to the last). Desktop: keep a single pill slot; a newer message replaces the visible one and resets its 2.5 s timer (proposal; Kotlin duration is toast-default, about 2 s).

### 1.2 Skip-intro button (`skipIntroText`)
- Null → hidden.
- Netflix mode, countdown > 0 → label `player_aniskip_dontskip` (base text non-English; localized at runtime).
- Countdown == 0 → seeks immediately (no label).
- Countdown == -1 (cancelled or spent) or manual mode → label `player_skip_action` with chapter type text, e.g. "Skip Opening" (Kotlin base template is non-English; English wording from `ui` mockup "Skip Opening").
- Tapping the button while countdown > 0 cancels the countdown (label then becomes the normal skip label on the next tick).
- Mockup today: a static `Button` labelled "Skip Opening" with `suggested-action pill` classes, always visible. Gap: no visibility binding, no countdown, no cancel behaviour.

### 1.3 Chapter display (`currentChapter`, `chapters`)
- Bottom bar chapter button shows `"<mm:ss> • <chapter name>"` for the current chapter (mockup shows `"00:00 • Opening"` static). Chapter names: mpv `chapter-list/N/title` or AniSkip labels `Opening`, `Ending`, `Recap`, `Mixed-op`; chapter type labels `Opening`, `Ending`, `Recap`, `Mixed opening` (AYMR keys, §18 behavior).
- Empty chapter list → the chapter button is hidden (Kotlin `showCurrentChapter` pref default true controls the button; part 1 reads `pref_show_current_chapter`).
- Seek bar segments: chapters with `index == -2` (external) use colour `0xFFD8BBDF`; `ChapterType.Other` uses unspecified colour.
- Gap in mockup: seek bar marks are hard-coded at 85 and 1330 seconds.

### 1.4 Seek bar, scrub and thumbnails (`pos`, `duration`, `seekPosition`, `isSeeking`, `thumbnailImage`, `seekText`, `doubleTapSeekAmount`, `isSeekingForwards`)
- Position label and remaining label update every whole second (time-pos observer, I64).
- While scrubbing (`isSeeking = true`) show the thumbnail above the thumb: `thumbnailImage` (may be null → show time only). Thumbnail fetch throttled to one request per 2 s; the previous request is cancelled on each new fetch.
- On scrub end: `updateIsSeeking(false)` clears the thumbnail and commits `seekTo(position)`.
- `seekText` overlay: text such as the skip message; shown while non-null, centred near the seek bar.
- `doubleTapSeekAmount`: accumulated seconds shown on the side of a double tap ("+10", "-20"). `0` hides it. Sign from `isSeekingForwards`.
- Duration label toggles between total and remaining (`invertDuration` pref, non-pref default false). Mockup has this toggle; keep, bind to pref.
- Gap: mockup uses a constant 1420 s duration, a 10 s skip pill on tap and no thumbnails.

### 1.5 Controls visibility and locking (`controlsShown`, `seekBarShown`, `areControlsLocked`)
- Controls hidden when `controlsShown == false`; `hideControls` also hides the status bar (desktop: no-op).
- Locked state: only the unlock button is visible (part 1 renders; this part only keeps the flag). Mockup implements this with a local `Cell`-less visibility toggle; move it to `controls_locked`.
- Controls auto-hide delay: `pref_player_time_to_disappear` = 4000 ms (part 1 timer; this part only stores the pref).
- `showControls` is refused while any sheet, panel or dialog is open (exclusivity, behavior §10).

### 1.6 Sheets, panels, dialogs
| State | Source | Desktop rendering |
|---|---|---|
| `sheetShown` (None, PlaybackSpeed, SubtitleTracks, AudioTracks, QualityTracks, Chapters, More, Screenshot) | `showSheet` | existing `sheet("player", id)` helper; gate on `Sheet` change |
| `panelShown` (None, SubtitleSettings, SubtitleDelay, AudioDelay, VideoFilters) | `showPanel` | side panel (existing mockup `long press` maps to panels) |
| `dialogShown` (None, EpisodeList, IntegerPicker) | `showDialog` | `dialog("player", id)`; IntegerPicker is a spin dialog built from `IntegerPicker` fields (title, nameFormat, min, max, step, default, onChange writes mpv property) |
| `dismissSheet` | `dismissSheet()` after quality pick (`onVideoClicked`) | close sheet on request, then reset |

IntegerPicker details: `nameFormat` is a format template with one value slot; `step` snaps values; `onChange` writes the mpv property immediately; dismiss calls `showDialog(None)`.

### 1.7 Quality/hoster sheet (`hosterList`, `hosterState`, `hosterExpandedList`, `selectedHosterVideoIndex`, `isLoadingHosters`)
Per hoster row (behavior §4):
- `Idle` (lazy) → row with "tap to load" affordance; tap → `Loading`.
- `Loading` → spinner row.
- `Ready` → expandable: header with hoster name and video count; body lists videos with per-video state.
- `Error` → error row; tap is a no-op in Kotlin (cannot retry). Proposal: allow retry (behavior §4.4 note, E-08).
- Per video: `Queue` (no marker), `LoadVideo` (spinner), `Ready` (selected check when `selectedHosterVideo` matches), `Error` (greyed, not clickable, `onVideoClicked` returns early).
- Selected video highlighted by `selectedHosterVideoIndex == (hosterIdx, videoIdx)`.
- Hidden rows: `pref_show_failed_hosters` (default false) and `pref_show_empty_hosters` (default false) decide whether Error and empty hosters are listed (part 1 filter; the prefs are owned here, behavior unchanged).
- Initially all hosters are expanded (`hosterExpandedList = all true`).

### 1.8 Audio and subtitle sheets (`subTracks`, `selectedSubtitles`, `audioTracks`, `selectedAudio`, `isLoadingTracks`)
- Audio list: first row "Off" (id -1, name from `MR.strings.off`), then tracks named `title` or `language` fallback (`VideoTrack.name` is the mpv title; language kept separately).
- Subtitle list: "Off" entry is NOT added in the Kotlin code (subtitle list starts with real tracks); the selection shows primary and secondary (dual subs) markers. Proposal: add "Off" row for parity with audio (behavior §5.1 notes only audio gets "Off"; keep Kotlin behavior and mark as E-31 for decision).
- Loading flag: `isLoadingTracks` true until `onFinishLoadingTracks` runs (no visible spinner in Kotlin when there are no external tracks).
- External file add: file chooser → `addAudio`/`addSubtitle`.

### 1.9 Playback speed (`playbackSpeed`)
Speed label shows the current value. Presets from `default_speed_presets` (12 values; behavior/data §4.5). Kotlin `DoubleSpeed` one-shot message is emitted by part 1 on long-press of the speed button. Mockup cycles 0.25 steps up to 2.00 (`1.00x`) — gap: presets and the 12-value list are not used.

### 1.10 Aspect ratio (`playerUpdate = AspectRatio`, `aspectState`)
- Pill text per mode: "Fit to screen", "Stretched to screen", "Cropped to screen" (mockup already has these). Kotlin source strings: `video_fit_screen`, `video_stretch_screen`, `video_crop_screen` (AYMR). Mockup order is Fit → Stretch → Crop, which matches a cycle; Kotlin enum order is Crop, Fit, Stretch (PlayerEnums.kt:52-56). Keep the cycle order of the mockup but make it state-driven from `aspectState`.
- Initial mode = persisted `pref_player_aspect_state` (default Fit).

### 1.11 Auto-play (`autoplayEnabled`) and next/previous
- Switch in the top bar bound to `autoplayEnabled`; toggling shows the auto-play pill (§1.1). The mockup shows the pill with the same texts ("Auto-play is on/off") — keep.
- Previous/next buttons disabled when `hasPreviousEpisode`/`hasNextEpisode` is false (Kotlin buttons read those flags; mockup always enabled — gap).

### 1.12 Volume and brightness sliders (`isVolumeSliderShown`, `isBrightnessSliderShown`, `currentVolume`, `currentMPVVolume`, `currentBrightness`)
- Volume slider appears when mpv volume changes (`setMPVVolume` with a changed value) or on gesture; shows percent 0..(100 + cap). Above 100 shows the boost range (cap default 30).
- Brightness slider appears on gesture (`displayBrightnessSlider`). Range -0.75..1 displayed as percent; a negative value is shown as dimming.
- Hidden after the slider's own timeout (part 1 timer).
- Gap: mockup has no sliders.

### 1.13 Sleep timer (`remainingTime`)
- Shows remaining seconds (integer countdown) when > 0. Ends with pause and the "Sleep timer ended" pill. Starting a new timer cancels the previous one. Setting 0 cancels.
- Note the off-by-one: total displayed countdown runs from N to 0 inclusive, so the pause happens N+1 seconds after start (behavior §11). Desktop should show the same N..0 sequence to match Kotlin.

### 1.14 Custom buttons (`customButtons`, `primaryButton`, `primaryButtonTitle`)
- `customButtons` state: `Loading` → show nothing; `Success(list)` → list in the custom-buttons sheet; `Error(message)` → message row with the error text (fallback "Unable to fetch buttons").
- Primary button: visible when `primaryButton != null`; label `primaryButtonTitle` (defaults to favourite button name, can be overwritten by Lua `set_button_title`); tap → `execute()`, long press → `executeLongPress()`.
- Lua `toggle_button` show/hide/toggle controls visibility (behavior §12).

### 1.15 Loading indicators (`isLoading`, `isLoadingEpisode`, `isLoadingHosters`, `isLoadingTracks`)
- `isLoadingEpisode` true while `loadVideo` runs (during switching or first load): shows the loading circle (`pref_show_loading`, default true) over the video. Cleared by `onFinishLoadingTracks` / failed `loadVideo`.
- `isLoading` is set true at construction and not cleared in this file; part 1 clears it when the first frame renders (proposal: clear on first `FileLoaded` + video params).

### 1.16 Decoder state (`currentDecoder`)
Shown in the statistics/more sheet as the currently active decoder name: `Decoder.title` (`Auto`, `SW`, `HW`, `HW+`). Changes only when the user picks one in the More sheet (writes `hwdec`). Linux mapping in data §2 (HW/HW+ → VA-API/NVDEC copy modes).

### 1.17 Episode list dialog (`currentPlaylist`, `currentEpisode`)
- Rows are the filtered playlist (behavior §2.4). The current episode is highlighted; the selected episode is always present even if filtered out (appended at end).
- Tap a row → `changeEpisode`-equivalent switch (`loadEpisode`).
- Bookmark/fillermark controls on each row call `bookmarkEpisode`/`fillermarkEpisode` (part 1 row UI).

## 2. Interaction and input mapping (desktop)

| Kotlin gesture or control | Desktop mapping (this part's semantics) |
|---|---|
| Double tap left third → `handleLeftDoubleTap` (default Seek) | Double-click left 40% of the video surface (mockup uses 40%/60% — keep); action per `pref_left_double_tap` |
| Double tap centre → `handleCenterDoubleTap` (default PlayPause) | Double-click middle 20% |
| Double tap right → `handleRightDoubleTap` (default Seek) | Double-click right 40% |
| Horizontal drag → seek (`gestureHorizontalSeek`, default true) | Drag on surface → scrub; gated by pref |
| Vertical drag left/right → brightness/volume (`gestureVolumeBrightness` default true; swap pref) | Scroll wheel over surface → volume (up/down) and Shift+scroll → brightness; swap pref inverts |
| Media keys previous/play-pause/next (`pref_media_*`) | MPRIS (`mpris` via `gio::DBus` or `mpris-server` crate): Previous/PlayPause/Next mapped to the three `SingleActionGesture` settings |
| Keyboard arrows | Left/Right ±`skipLengthPreference` (default 10 s), Up/Down volume ±1, Space pause, `[`/`]` speed presets (proposal, not in Kotlin) |
| Long press surface → screenshot sheet (mockup) | Keep, or right click; Kotlin has no direct long press for screenshot in this file |

Keyboard binding is a desktop addition; its conflicts with KDE shortcuts are documented in T-45b-057 acceptance.

## 3. Desktop-specific surfaces (no Kotlin equivalent)

### 3.1 Picture-in-picture (Kotlin `pref_enable_pip`, `pip_replace_with_previous`, `pref_pip_on_exit`, `pref_pip_episode_toasts`)
Android PiP is an OS window mode and has no GTK equivalent. Proposal (T-45b-050):
- "Mini player" = a second `gtk::Window` (small, 480x270 default, resizable) that hosts a second GLArea attached to the same mpv handle is NOT possible (one `vid` per handle); instead the existing render context is re-parented: move the GLArea widget into the mini window, keep the same `Mpv` handle.
- Always-on-top: GTK4 has no portable API. Use a window rule hint via `gtk::Window::set_transient_for` plus document the KDE window rule ("Keep above") for the user; under Wayland the compositor decides.
- Trigger: a button (replaces the Android PiP button, mockup has `view-restore-symbolic`); auto on window minimise if `pref_pip_on_exit`.
- Episode toasts (`pref_pip_episode_toasts`): show the "episode changed" pill in the mini window on episode switch.
- `pip_replace_with_previous`: when enabled, the mini player shows the previous episode instead of the next one on the "switch" gesture (behavior §2 ordering). Document as behaviour-preserving placeholder: if the user never enables it, nothing changes.

### 3.2 Window brightness and system volume
Visible behaviour is covered in §1.12. Failure surface: if no PipeWire/PulseAudio tool is present, the volume slider is shown but disabled with a tooltip "System volume unavailable"; mpv-boost volume still works.

## 4. Gap list (mockup vs. target), concrete

| Area | Mockup `src/player.rs` | Target |
|---|---|---|
| Surface | `GBox` placeholder with `osd` class | `GLArea` + mpv render (part 1 widget, this part's `Mpv`) |
| Duration | constant 1420.0 | `store.model.duration` |
| Seek bar marks | constant 85 & 1330 | chapter list marks (`chapters`) |
| Position | none (slider only) | `pos` from `time-pos` |
| Play/pause icon | swaps icon by local label check | `paused` state |
| Speed | local cycle 0.25 steps to 2.0 | `playback_speed`, presets from pref, long press sheet |
| Chapter button | static "00:00 • Opening" | current chapter, hidden when empty |
| Skip button | always visible "Skip Opening" | `skip_intro_text` binding with countdown and cancel |
| Aspect | local Cell index | `aspectState` + `set_aspect` |
| Auto-play | Switch, pill | Switch bound to pref + pill from `player_update` |
| Prev/next | always enabled | bound to `has_prev`/`has_next` |
| Lock | local visibility toggling | `controls_locked` |
| Seek thumbnail | none | `seek_thumbnail` |
| Sleep timer | none | `remaining_sleep` + sheet "More" |
| Volume/brightness sliders | none | §1.12 |
| Hoster/quality sheet | none | §1.7 |
| Tracks | none | §1.8 |
| Episode list | `dialog("player","episode_list")` only | §1.17 bound to playlist |
| Errors | none | error panel from `PlayerError` |
| PiP | icon only | §3.1 |

## 5. Accessibility and keyboard requirements
- All icon buttons get tooltips (existing `icon_button` helper sets them).
- Skip button, toasts and the countdown text are announced via `gtk::Accessible` labels (`accessible-label`), since they appear without focus change.
- Every mouse-only gesture (double-click seek, scroll volume) has a keyboard equivalent in §2.
- Colour is not the only indicator for hoster/video states: Error uses a text suffix "(failed)"; Loading uses a spinner.
- Text scale follows the GTK font settings; no fixed-pixel label sizes for toasts.

## 6. Edge rules for rendering
- Pill messages are ellipsized at 60 characters; Lua `show_text` can be arbitrary length.
- Thumbnail preview is clamped inside the window; if the crop rectangle exceeds the tile bounds, use the nearest valid crop (E-30).
- The IntegerPicker uses `min`/`max` from the Lua payload; values outside `[min, max]` are clamped to the range on open (Kotlin passes them as-is; E-53 covers malformed payloads).
