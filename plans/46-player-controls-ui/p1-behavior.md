# 46 Player controls UI, part 1: behaviour spec

All paths are repo-relative to `/home/blackbox/aniyomi-src`. `OBSERVED` marks what the Kotlin does. `Proposal:` marks what Kurobayomi should do. Task ids refer to `p1-tasks.md`. Data types and preference keys are in `p1-data-and-config.md`.

Abbreviations:
- VM = `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt`
- PC = `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt`
- GH = `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt`

---

## 1. Root overlay (PC)

### 1.1 Composition and z-order (OBSERVED, PC:151-155 and PC:156-671)

Order of composition, back to front:
1. `GestureHandler(viewModel, interactionSource)` (PC:151), a full-size transparent input surface (GH:117-302).
2. `DoubleTapToSeekOvals(doubleTapSeekAmount, seekText, interactionSource)` (PC:155), full-size, draws only the oval feedback (GH:305-347).
3. `ConstraintLayout` (PC:164-569) with the veil background, containing every control.
4. The sheet host `PlayerSheets` (PC:590-640), the panel host `PlayerPanels` (PC:642-645), the dialog host `PlayerDialogs` (PC:652-666).
5. `BrightnessOverlay(currentBrightness)` (PC:668-670), a full-size black veil.

Consequence: controls receive pointer events before the gesture surface; the gesture surface only gets events the controls do not consume. Proposal: keep the same z-order in GTK (`gtk::Overlay`: gesture surface as child, controls as overlays, sheets as separate modal windows, brightness veil as a non-input overlay).

Composition locals set around the controls (PC:156-163):
- `LocalRippleConfiguration` = `playerRippleConfiguration` (ripple colour white in dark theme, black otherwise, alpha 0.1 for all four states; defined in `app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt`, constants `RIPPLE_*_ALPHA = .1f`). Drop, see T-46a-057.
- `LocalPlayerButtonsClickEvent` = `{ resetControls = !resetControls }` (PC:158). Every control button calls it on click (ControlsButton GH/ControlsButton.kt:68, AutoPlaySwitch.kt:71, SeekBar.kt:103). Flipping `resetControls` restarts the auto-hide timer (see 1.3).
- `LocalContentColor` = `Color.White` (PC:159).
- `LocalLayoutDirection` = `LayoutDirection.Ltr` (PC:162). The overlay is always left-to-right even under RTL locales. Proposal: keep LTR for the transport/seekbar geometry, but use the same left/right semantics (see 1.4).

### 1.2 Visibility inputs (OBSERVED)

State read from VM (PC:107-122):

| Name | Source | Meaning |
|---|---|---|
| `controlsShown` | `VM.controlsShown` (VM:276, initial `!hideControls()`, default `hideControls` = false, so initially true) | Controls chrome visible |
| `areControlsLocked` | `VM.areControlsLocked` (VM:280, initial false) | Lock mode |
| `seekBarShown` | `VM.seekBarShown` (VM:278, initial `!hideControls()`) | Seekbar alone visible |
| `isLoading` | `VM.isLoading` (VM:210, initial true) | Initial load spinner |
| `isLoadingEpisode` | `VM.isLoadingEpisode` (VM:201, initial false) | Episode switch spinner |
| `paused` | `VM.paused` (VM:269, initial false) | |
| `gestureSeekAmount` | `VM.gestureSeekAmount` (VM:~289, `Pair<startingPos, delta>?`, null when idle) | Horizontal drag in progress |
| `doubleTapSeekAmount` | `VM._doubleTapSeekAmount` (VM:308, Int, initial 0) | Accumulated double-tap seconds |
| `seekText` | `VM._seekText` (VM:306, `String?`, initial null) | Override text for the oval |
| `isSeeking` | `VM._isSeeking` (VM:251, initial false) | Seekbar being dragged |
| `seekPosition` | `VM._seekPosition` (VM:248, initial 0f) | Position under finger/drag |
| `pos` | `VM._pos` (VM:245, initial 0f) | Current playback position (seconds, Float) |
| `duration` | `VM.duration` (VM:264, initial 0f) | Seconds |
| `currentChapter` / `chapters` | VM | |
| `currentBrightness` | `VM.currentBrightness` (VM:286) | Range -0.75..1 |

Derived `visible` flags used by the overlay:
- Chrome (top-left, top-right, bottom-left, bottom-right, unlock button, veil): `controlsShown && !areControlsLocked` (PC:416, 447, 482, 526, 370 for seekbar uses different rule, see 6.1).
- Centre transport outer container (PC:336-342): `(controlsShown && !areControlsLocked || gestureSeekAmount != null) || isLoading || isLoadingEpisode`.
- Seekbar (PC:369-371): `(controlsShown || seekBarShown) && !areControlsLocked`.
- Unlock button (PC:322-324): `controlsShown && areControlsLocked`.
- Veil alpha target (PC:146-150): `if (controlsShown && !areControlsLocked) .8f else 0f`.

### 1.3 Auto-hide timer (OBSERVED, PC:134-144)

```
LaunchedEffect(controlsShown, paused, isSeeking, resetControls) {
    if (controlsShown && !paused && !isSeeking) {
        delay(playerTimeToDisappear.toLong())   // default 4000 ms
        viewModel.hideControls()
    }
}
```

Rules:
- The timer runs only when controls are shown, playback is running, and no seekbar drag is in progress.
- Any change to one of the four keys cancels and restarts the timer. `resetControls` flips on every control click (1.1), so every button press restarts the 4000 ms countdown.
- Pausing cancels the timer: controls stay visible while paused.
- Pure gesture events (tap-to-toggle, double-tap, drag) do not flip `resetControls`; they change `controlsShown` or `seekPosition`. A tap that shows controls starts the timer because `controlsShown` changes.
- `playerTimeToDisappear` default: `pref_player_time_to_disappear` = 4000 (`PlayerPreferences.kt:43`). Stored as Int milliseconds.

`VM.hideControls()` (VM:679-682, OBSERVED): hides the status bar (`windowInsetsController.hide(statusBars)`) and sets `controlsShown = false`.
`VM.showControls()` (VM:666-677, OBSERVED): returns without change if any of `sheetShown != None`, `panelShown != None`, `dialogShown != None`. Otherwise shows the status bar only if `pref_show_system_status_bar` (default false, cached at VM construction, VM:675 `showStatusBar`) and sets `controlsShown = true`.

Proposal: same state machine. Store the status-bar behaviour as a no-op on Linux (the window is always decorated by the compositor or fullscreen by the user). Keep the `showControls` refusal rule because it prevents controls from fighting an open sheet.

### 1.4 Veil (OBSERVED, PC:167-175)

A vertical linear gradient is painted behind the ConstraintLayout, with stops:
- 0.0 black, 0.2 transparent, 0.7 transparent, 1.0 black.
- Whole brush alpha = `transparentOverlay`, animated with `playerControlsExitAnimationSpec()` (300 ms, FastOutSlowIn) in both directions (PC:146-150).
- Padding: horizontal `MaterialTheme.padding.medium` = 16 dp (PC:176).

Proposal: CSS `linear-gradient(to bottom, rgba(0,0,0,1) 0%, rgba(0,0,0,0) 20%, rgba(0,0,0,0) 70%, rgba(0,0,0,1) 100%)` on the overlay child, with `opacity` transition 300 ms, opacity 0.8 when controls shown.

### 1.5 Placement table (OBSERVED, PC:177-568)

ConstraintLayout references: `topLeftControls`, `topRightControls`, `volumeSlider`, `brightnessSlider`, `unlockControlsButton`, `bottomRightControls`, `bottomLeftControls`, `centerControls`, `thumbnail`, `seekbar`, `playerUpdates`.

| Element | Constraints | Width rule |
|---|---|---|
| Brightness slider (PC:229-237) | `top`/`bottom` to parent (centred vertically). If `swapVolumeAndBrightness` then `start` to parent start + 16 dp, else `end` to parent end + 16 dp | Wrap |
| Volume slider (PC:270-278) | `top`/`bottom` to parent. If swapped then `end` + 16 dp, else `start` + 16 dp | Wrap |
| Player update pill (PC:304-307) | `linkTo(parent.start, parent.end)` horizontally centred; vertical `linkTo(parent.top, parent.bottom, bias = 0.2f)` | Wrap |
| Unlock button (PC:326-329) | top = parent top + 16, start = parent start + 16 | Wrap |
| Centre transport (PC:343-348) | `start`/`end` = absolute left/right, `top`/`bottom` = parent | Fill parent width (centred by Row) |
| Seekbar (PC:383-385) | `bottom` = parent bottom + 16 dp | Fill parent width |
| Top-left (PC:429-434) | top = parent top + 16; start = parent start; `end` = top-right start; width = fillToConstraints | Fill to constraint |
| Top-right (PC:460-463) | top = parent top + 16; end = parent end | Wrap |
| Bottom-right (PC:495-498) | bottom = seekbar top; end = seekbar end | Wrap |
| Bottom-left (PC:539-544) | bottom = seekbar top; start = seekbar start; `end` = bottom-right start; width = fillToConstraints | Fill to constraint |
| Thumbnail (PC:565-567) | bottom = seekbar top + 16 dp | Fill parent width (content sized inside) |

Slide and fade directions (all durations from 1.6):
- Brightness slider: enter `slideInHorizontally { if swapped -it else it }` plus fadeIn; exit mirrored. When `reduceMotion` is true, fade only (PC:207-228).
- Volume slider: enter `slideInHorizontally { if swapped it else -it }`, exit mirrored (PC:248-269).
- Top-left: enter from `-it` (left), exit to left (PC:418-428).
- Top-right: enter from `it` (right) (PC:448-459).
- Bottom-right: enter from right (PC:483-494).
- Bottom-left: enter from left (PC:527-538).
- Seekbar: slide vertically from bottom `slideInVertically { it }` (PC:372-382).
- Unlock, transport: plain fadeIn/fadeOut with spec (PC:322-335, 336-348).
- When `reduceMotion` is true, every slide becomes a fade.

### 1.6 Animation timings (OBSERVED, PC:674-682)

```
playerControlsExitAnimationSpec<T>() = tween(durationMillis = 300, easing = FastOutSlowInEasing)
playerControlsEnterAnimationSpec<T>() = tween(durationMillis = 100, easing = LinearOutSlowInEasing)
```

Note the enter is shorter (100 ms) than the exit (300 ms). Keep these exact values in Rust (T-46a-041).

Other animation constants found in scope files:
- Volume/brightness/player-update auto-hide delay: 2000 ms (PC:198, 202, 297).
- Brightness slider `alpha` not animated, the overlay veil uses `graphicsLayer.alpha` directly.
- Double-tap oval alpha: `animateFloatAsState(if (amount == 0) 0f else 0.2f)` (GH:312), default spec.
- Sheet background alpha: `tween(350)` (PlayerSheet.kt:74).

### 1.7 Lock mode (OBSERVED)

- Lock button: `ControlsButton(Icons.Default.LockOpen)` at bottom-left (BottomLeftPlayerControls.kt:58-61) calls `VM.lockControls()` (VM:693-695): sets `areControlsLocked = true`. The icon is `LockOpen` (drawn as an open padlock, meaning "lock").
- Locked state: all chrome hidden (1.2). Only the unlock button is shown, at top-left, using `Icons.Filled.Lock` (PC:331-334) when `controlsShown && areControlsLocked`.
- Tapping the unlock button calls `VM.unlockControls()` (VM:697-699). The unlock button is visible only when `controlsShown` is true, so the user must tap the surface to reveal it (GH:125 toggles `controlsShown`).
- Gesture handling while locked:
  - Single tap still toggles `controlsShown` (GH:124-126 has no lock check). The unlock button therefore appears and disappears with taps.
  - Double tap is ignored (GH:128 `if (areControlsLocked || isDoubleTapSeeking) return`).
  - Long press is ignored (GH:171).
  - Horizontal drag is disabled (GH:182 `if (!seekGesture || areControlsLocked) return`).
  - Vertical drag is disabled (GH:219).
  - Drag pointer inputs are keyed on `areControlsLocked` so they are rebuilt on lock change (GH:181, 218).
- Lock is session state. It is not persisted (VM field only), so a new episode or activity start is unlocked.

Proposal: keep the same. In GTK, the unlock button is the only child with pointer activity while locked. Add the keyboard shortcut from T-46a-043 for unlock.

---

## 2. Gesture surface (GH)

### 2.1 Pointer structure (OBSERVED, GH:117-302)

The gesture surface is a `Box` with `fillMaxSize()` and `windowInsetsPadding(WindowInsets.safeGestures)` (GH:117-120). Three `pointerInput` coroutines are attached in order:
1. Tap, double-tap, press, long-press block (GH:121-180), keyed on `Unit`.
2. Horizontal drag block (GH:181-217), keyed on `areControlsLocked`.
3. Vertical drag block (GH:218-301), keyed on `areControlsLocked`.

`safeGestures` insets: the surface avoids system gesture areas (Android-only). Proposal: no equivalent needed; leave a margin for the GTK window edge resize handles only (none in fullscreen).

### 2.2 Tap (OBSERVED, GH:124-126)

`onTap`: `if (controlsShown) viewModel.hideControls() else viewModel.showControls()`.

Consequences:
- Tap while controls are shown hides them immediately (no timer).
- `showControls` refuses if a sheet, panel or dialog is open (VM:666-669). In that case the tap does nothing visible.

### 2.3 Press (OBSERVED, GH:141-169)

`onPress` runs on each pointer down. Sequence:
1. If `panelShown != Panels.None && !allowGesturesInPanels`, set `panelShown = Panels.None` (VM `panelShown` flow). `pref_allow_gestures_in_panels` default false (`PlayerPreferences.kt:23`). So a tap on the video while a panel is open closes the panel, unless the preference is on.
2. Build `PressInteraction.Press` at the point. The x is shifted when the press is in the right zone: `x' = if (x > width*3/5) x - width*0.6 else x` (GH:145-147). This aligns the ripple with the right oval, which begins at 0.6 width.
3. If not locked and `isDoubleTapSeeking && seekAmount != 0`, route the press as a repeat seek using the same zone logic as `onDoubleTap` (GH:148-157). Otherwise set `isDoubleTapSeeking = false` (GH:158-160).
4. `interactionSource.emit(press)` (GH:161).
5. `tryAwaitRelease()` (GH:162).
6. On release: if `isLongPressing` then `isLongPressing = false`, `MPVLib.setPropertyDouble("speed", originalSpeed)`, `VM.playerUpdate = None` (GH:163-167). `originalSpeed` is captured once when this `pointerInput(Unit)` coroutine starts (GH:122: `val originalSpeed = viewModel.playbackSpeed.value`).
7. `interactionSource.emit(PressInteraction.Release(press))` (GH:168).

Quirk (OBSERVED): `originalSpeed` is captured at composition, not at long-press. Release therefore sets mpv speed back to the speed the player had when the surface was created. If the user changed the speed while holding (the screenshot sheet covers the screen, so this is unlikely but possible via the sheet's speed control), the release overwrites it. Proposal: do not restore speed at all, because the long-press no longer changes speed (see 2.5). Keep only the `playerUpdate = None` side effect.

### 2.4 Double tap (OBSERVED, GH:127-140)

`onDoubleTap(position)`:
- If `areControlsLocked || isDoubleTapSeeking` return.
- Zone by x:
  - `x > width * 3/5` (right zone, x > 60%): if `!isSeekingForwards` then `VM.updateSeekAmount(0)`; `VM.handleRightDoubleTap()`; `isDoubleTapSeeking = true`.
  - `x < width * 2/5` (left zone, x < 40%): if `isSeekingForwards` then `VM.updateSeekAmount(0)`; `VM.handleLeftDoubleTap()`; `isDoubleTapSeeking = true`.
  - otherwise (centre, 40% to 60%): `VM.handleCenterDoubleTap()` only. `isDoubleTapSeeking` is not set.

The `updateSeekAmount(0)` before a direction change resets the accumulator so that reversing direction starts from zero (VM `seekByWithText` logic, VM:985-992).

Handlers (VM:1051-1095, OBSERVED): the action comes from preference by zone:

| Zone | Preference (default) | VM handler |
|---|---|---|
| Left | `pref_left_double_tap` (`SingleActionGesture.Seek`) | `handleLeftDoubleTap` |
| Centre | `pref_center_double_tap` (`SingleActionGesture.PlayPause`) | `handleCenterDoubleTap` |
| Right | `pref_right_double_tap` (`SingleActionGesture.Seek`) | `handleRightDoubleTap` |

Action mapping (VM:1051-1095):
- `Seek`: left calls `leftSeek()`, right calls `rightSeek()`; centre does nothing.
- `PlayPause`: `pauseUnpause()`; centre default.
- `Custom`: sends `MPVLib.command(arrayOf("keypress", <CustomKeyCodes.DoubleTapLeft|Center|Right>.keyCode))`, codes `"0x10001"`, `"0x10002"`, `"0x10003"`.
- `None`: nothing.
- `Switch`: left = `changeEpisode(true)` (previous), right = `changeEpisode(false)` (next); centre does nothing.

Seek semantics (VM:1001-1020, OBSERVED):
- `leftSeek()`: if `pos > 0` then `_doubleTapSeekAmount -= doubleTapToSeekDuration`. Set `_isSeekingForwards = false`. `seekBy(-doubleTapToSeekDuration, preciseSeek)`. If `showSeekBar` pref then `showSeekBar()`.
- `rightSeek()`: if `pos < duration` then `_doubleTapSeekAmount += doubleTapToSeekDuration`. `_isSeekingForwards = true`. `seekBy(+doubleTapToSeekDuration, preciseSeek)`. Show seekbar if pref.
- `doubleTapToSeekDuration` = `gesturePreferences.skipLengthPreference()` = `pref_skip_length_preference`, default 10 (VM:974, GesturePreferences.kt:22).
- `preciseSeek` = `pref_player_smooth_seek`, default false (VM:975). When false, mpv gets `relative` (keyframe seek); when true, `relative+exact` (VM:983-986 for `seekBy`).

Note: the double-tap seek is relative (`seekBy`) and does not go through the `seekToWithText` absolute path.

### 2.5 Long press (OBSERVED, GH:170-178)

`onLongPress`:
- If `areControlsLocked` return.
- If not already `isLongPressing`: haptic LongPress, `isLongPressing = true`, `VM.pause()`, `VM.sheetShown = Sheets.Screenshot`.

The long-press opens the screenshot sheet and pauses. It does not change speed. The sheet is `ScreenshotSheet` (part 2 body). On dismiss, `PlayerSheets` calls `showSheet(None)` and `unpause()` (PC:633-636). Long press timeout is the platform long-press timeout (`ViewConfiguration`), which Compose uses for `detectTapGestures`. Proposal: `GestureLongPress` default delay (GTK default 500 ms; verify against the desktop setting in GTK settings `gtk-long-press-time`, which GTK exposes through `gtk::Settings`).

### 2.6 Horizontal drag (seek) (OBSERVED, GH:181-217)

Enabled when `pref_gesture_horizontal_seek` is true (default true, GesturePreferences.kt:19) and not locked.

`onDragStart(offset)`:
- `startingPosition = position.toInt()` (seconds, truncated).
- `startingX = offset.x`.
- `wasPlayerAlreadyPause = VM.paused.value`.
- `VM.updateIsSeeking(true)`.

`onDrag(change, dragAmount)`:
1. If `position <= 0f && dragAmount < 0` return (cannot seek before start).
2. If `position >= duration && dragAmount > 0` return (cannot seek past end).
3. `newPos = calculateNewHorizontalGestureValue(startingPosition, startingX, change.position.x, 0.15f)`:
   `newPos = startingPosition + ((x - startingX) * 0.15f).toInt()` (GH:357-359).
   Sensitivity 0.15 means 1 px = 0.15 s; a 100 px drag = 15 s (Int truncation toward zero).
4. `gestureSeekAmount = Pair(startingPosition, (newPos - startingPosition).coerceIn(0 - startingPosition, (duration - startingPosition).toInt()))`.
5. `VM.updateSeekPos(newPos.toFloat().coerceIn(0f, duration))`.
6. If `pref_show_seekbar` then `VM.showSeekBar()`.

`onDragEnd`:
1. `VM.updatePlayBackPos(VM.seekPosition.value)`.
2. `VM.updateIsSeeking(false)`.
3. `VM.seekTo(seekPosition.coerceIn(0f, duration).toInt(), preciseSeeking)`, absolute seek; `preciseSeeking` = `pref_player_smooth_seek`.
4. `gestureSeekAmount = null`.
5. `VM.hideSeekBar()`.
6. `if (!wasPlayerAlreadyPause) VM.unpause()`.

Observed quirk: the drag never pauses the player, so step 6 only re-issues unpause on an already-playing player (no effect). Proposal: drop the unpause line; preserve the paused state.

Seek bar follow-up: `updateIsSeeking(false)` clears the thumbnail (VM:615-620).

### 2.7 Vertical drag (volume and brightness) (OBSERVED, GH:218-301)

Enabled when `pref_gesture_volume_brightness` is true (default true, GesturePreferences.kt:11-14) and not locked.

Constants captured at pointer start (GH:225-227):
- `brightnessGestureSens = 0.001f`
- `volumeGestureSens = 0.001f * viewModel.maxVolume` (`maxVolume` = Android `STREAM_MUSIC` max, typically 15 on phones; VM:765).
- `mpvVolumeGestureSens = 0.001f * volumeBoostingCap` (`pref_audio_volume_boost_cap`, default 30, AudioPreferences.kt:14).

Booleans:
- `isIncreasingVolumeBoost(dy)` = `cap > 0 && currentVolume == maxVolume && (currentMPVVolume - 100) < cap && dy < 0`.
- `isDecreasingVolumeBoost(dy)` = `cap > 0 && currentVolume == maxVolume && (currentMPVVolume - 100) in 1..cap && dy > 0`.

Sign convention: `amount` is the vertical delta in pixels, positive when the finger moves down. Upward drags have negative amounts and increase the value.

`onDragStart`: reset `startingY = 0`, `mpvVolumeStartingY = 0`, snapshot `originalVolume`, `originalMPVVolume`, `originalBrightness`.

Per event, the handler chooses brightness or volume by zone (GH:295-299):
- If `swapVolumeBrightness` (default false): `x > width/2` → brightness, else volume.
- Otherwise: `x < width/2` → brightness, else volume.

Volume branch (GH:249-282, 250-282):
- If boost condition (`isIncreasingVolumeBoost(amount) || isDecreasingVolumeBoost(amount)`):
  - On first boost event (`mpvVolumeStartingY == 0`): `startingY = 0`, `originalVolume = currentVolume`, `mpvVolumeStartingY = y`.
  - `VM.changeMPVVolumeTo(calc(originalMPVVolume, mpvVolumeStartingY, y, mpvVolumeGestureSens).coerceIn(100..cap + 100))`. Calc is Int: `orig + ((startY - y) * sens).toInt()`.
- Else:
  - On first non-boost event (`startingY == 0`): `mpvVolumeStartingY = 0`, `originalMPVVolume = currentMPVVolume`, `startingY = y`.
  - `VM.changeVolumeTo(calc(originalVolume, startingY, y, volumeGestureSens))`, Int calc. `VM.changeVolumeTo` coerces to `0..maxVolume` and writes the Android stream volume (VM:779-787).
- Always: `VM.displayVolumeSlider()` (VM:798-800) sets `isVolumeSliderShown = true`.

Brightness branch (GH:283-294):
- `if (startingY == 0) startingY = y`.
- `VM.changeBrightnessTo(calcFloat(originalBrightness, startingY, y, 0.001f))`, float calc: `orig + ((startY - y) * sens)`.
- `VM.displayBrightnessSlider()` sets `isBrightnessSliderShown = true`.

Brightness storage (VM:752-759): `currentBrightness = brightness.coerceIn(-0.75f, 1f)`; window brightness `= brightness.coerceIn(0f, 1f)`. So values below 0 do not change hardware brightness; they darken the software veil (see section 8).

Concrete numbers: a full-height drag of 1000 px changes brightness by 1.0 (0.001 per px). Volume with maxVolume 15 changes by 15 * 0.001 * 1000 = 15 steps, i.e., one step per ~67 px (Int truncation).

Quirks (OBSERVED):
- Boost branch: `startingY` is reset to 0 when entering boost (GH:253), so the first boost event re-anchors. Then `originalVolume = currentVolume` is re-read. Fine but subtle.
- `changeVolumeTo` is applied during boost too (GH:257-265 do not change device volume) only via `changeMPVVolumeTo`, so device volume stays at max while boosting. Correct.
- The boost cap is read at pointer start from the preference, while `VM.volumeBoostCap` (VM field, initialised from mpv `volume-max` at VM:~291) is what mpv actually enforces. `volume-max` is set to `pref + 100` at mpv init (`app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:258`). If the preference changes mid-session the two can disagree. Proposal: a single source of truth for the cap (the preference), applied to mpv `volume-max` on every change.

### 2.8 Drag and tap interaction with sheets (OBSERVED)

- `showSeekBar()` returns early if a sheet is open (VM:688-691).
- `VM.showControls()` returns early if any sheet, panel, dialog is open (VM:666-672).
- The gesture surface is covered by sheets (sheets are modal), so pointer events normally do not reach it while a sheet is open. Only the panel host allows gesture pass-through when `pref_allow_gestures_in_panels` is true.

---

## 3. Double-tap feedback (GH:305-347 and components/DoubleTapSeekTriangles.kt)

### 3.1 Accumulator and indicator timing (OBSERVED)

`LaunchedEffect(seekAmount)` in GH:96-103 runs every time `doubleTapSeekAmount` changes:
1. `delay(800)`.
2. `isDoubleTapSeeking = false`.
3. `VM.updateSeekAmount(0)`.
4. `VM.updateSeekText(null)`.
5. `delay(100)`.
6. `VM.hideSeekBar()`.

So the oval stays up 800 ms after the last seek step, then fades (alpha animation, default spec) and the seekbar hides 100 ms later (unless it is also in `pref_show_seekbar` mode, which re-shows it on the next seek).

Accumulation: VM `_doubleTapSeekAmount` changes in `leftSeek`/`rightSeek` by `±doubleTapToSeekDuration`, and is reset to 0 when the direction flips (GH:130, 134, 150, 153). Each double tap in the same direction adds 10 by default. The text is the plural `seconds` (`%d second` / `%d seconds`, `i18n-aniyomi/src/commonMain/moko-resources/base/plurals.xml:51-54`) formatted with the signed amount: for -20 the text is `-20 seconds`, for +10 `10 seconds`. Plural selection uses the absolute value (ICU operand `n = |x|`), so -1 selects `one` and renders `-1 second`. Edge case E-46-016.

Oval display rule (GH:320-344):
- Shown when `amount != 0 || text != null`.
- Side: `Alignment.CenterEnd` when `amount > 0`, else `CenterStart` (GH:315). Box width `fillMaxWidth(0.4f)` (2/5), full height.
- Shape: `RightSideOvalShape` if `amount > 0`, else `LeftSideOvalShape` (GH:330).
- Fill: `Color.White.copy(alpha)` with `alpha` animating to 0.2 when `amount != 0` (GH:312).
- Ripple: `indication(interactionSource, ripple())` (GH:332).
- Content: `DoubleTapSeekTriangles(isForward = amount > 0)` above a 12 sp white text, centred (GH:334-342).
- Text: `text ?: pluralStringResource(AYMR.plurals.seconds, amount, amount)`.

Note: `amount == 0` with `text != null` (for example the aniskip "Skipped" message) shows the oval on the left side because `amount > 0` is false (GH:315). Preserve this.

### 3.2 Triangles (OBSERVED, components/DoubleTapSeekTriangles.kt:20-62)

- `animationDuration = 750L`; step = `750 / 5 = 150` ms.
- Infinite loop: alpha1 to 1, alpha2 to 1, alpha3 to 1, alpha1 to 0, alpha2 to 0, alpha3 to 0. Each step is a sequential `animateTo` with `tween(150)`. Full cycle 900 ms (six 150 ms steps).
- `rotation = if (isForward) 0 else 180` (GH rotates the Row; Left arrows point left).
- Each arrow: drawable `R.drawable.ic_play_seek_triangle`, size 16 dp wide by 20 dp high, tint white, alpha from its channel.

Proposal: same numbers; use a single 150 ms `glib::timeout_add_local` ticking a phase counter 0..5, with each arrow alpha = `phase`-based fade. Keep total cycle 900 ms.

### 3.3 Oval geometry (OBSERVED, components/OvalBox.kt)

`RightSideOvalShape.createOutline(size)` (OvalBox.kt:35-57):
```
moveTo(w, h); lineTo(w, 0); lineTo(w/10, 0);
cubicTo(w/10, 0, -30, h/2, w/10, h); close()
```
The control point x = `-30` is an absolute pixel value (not scaled). The curve bulges left past the left edge by up to about 0.75 of the bulge, which gives the characteristic half-oval look, clipped by the parent box only on the outside.

`LeftSideOvalShape.createOutline(size)` (OvalBox.kt:59-81):
```
moveTo(0,0); lineTo(0,h); lineTo(w - w/10, h);
cubicTo(w - w/10, h, w, h/2, w - w/10, 0); close()
```
Mirror image, control point at `w` (scaled).

Proposal: cairo path; use scaled control point `-w/10` (or the absolute `-30` px as the Compose code does; pick one and document). Acceptance: visually matches the Compose result at 1920x1080 window width.

---

## 4. Centre transport (MiddlePlayerControls.kt)

### 4.1 Layout (OBSERVED, MiddlePlayerControls.kt:66-143)

`Row` with `spacedBy(MaterialTheme.padding.large)` = 24 dp, vertically centred. Children in order:
1. Previous `ControlsButton(Icons.Filled.SkipPrevious, iconSize = 48.dp, enabled = hasPrevious)` inside an `AnimatedVisibility(controlsShown && !areControlsLocked)`, shown only when `gestureSeekAmount == null` (MiddlePlayerControls.kt:71-84).
2. Centre content, one of three (MiddlePlayerControls.kt:88-127):
   - If `gestureSeekAmount != null`: gesture seek text (section 4.2).
   - Else if `(isLoading || isLoadingEpisode) && showLoadingCircle`: `CircularProgressIndicator(Modifier.size(96.dp))`.
   - Else: an `AnimatedVisibility(controlsShown && !areControlsLocked)` wrapping a 96 dp play/pause image, clip circle, ripple, padding 16 dp (MiddlePlayerControls.kt:107-125).
3. Next `ControlsButton(Icons.Filled.SkipNext, iconSize = 48.dp, enabled = hasNext)` with the same visibility rule as previous.

The play/pause image is `AnimatedImageVector.animatedVectorResource(R.drawable.anim_play_to_pause)` painted with `rememberAnimatedVectorPainter(icon, !paused)` (MiddlePlayerControls.kt:86, 113): `atEnd = !paused`, so the icon morphs between play and pause.

Click on play/pause: `onPlayPauseClick = viewModel::pauseUnpause` (PC:363).
Click on previous: `viewModel.changeEpisode(true)`; next: `viewModel.changeEpisode(false)` (PC:353-355).

Enabling (OBSERVED): `hasPreviousEpisode` / `hasNextEpisode` flows (VM:183, 186). `changeEpisode` also shows a toast when called without an adjacent episode (`AYMR.strings.no_prev_episode` = "Previous Episode not found!", `no_next_episode` = "Next Episode not found!", VM:1034-1048). Disabled buttons never call it.

### 4.2 Gesture seek indicator (OBSERVED, MiddlePlayerControls.kt:89-103)

Text = `stringResource(AYMR.strings.player_gesture_seek_indicator, sign, prettyTime(abs(delta)), prettyTime(start + delta))` where:
- `sign = if (delta >= 0) '+' else '-'`.
- `delta = gestureSeekAmount.second`, `start = gestureSeekAmount.first`.
- The resource is `%c%s\n[%s]` (`i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`, `player_gesture_seek_indicator`, translatable=false). Result example: `+00:12` on line 1 and `[01:37]` on line 2 (the target time).

Style: `headlineMedium` with shadow `Shadow(Color.Black, blurRadius = 5f)`, `FontWeight.Bold`, centred.

`prettyTime(d: Int, sign = false)` formats (see section 13.2 and T-46a-040):
- `hours = d / 3600`, `minutes = d % 3600 / 60`, `seconds = d % 60`.
- If hours == 0: `"%02d:%02d"` (minutes, seconds). Else `"%d:%02d:%02d"`.
- Sign variant: `(if (d >= 0) "+" else "-") + prettyTime(abs(d))`.

Source: upstream `mpv-android/app/src/main/java/is/xyz/mpv/Utils.kt:127-137`, used by Aniyomi through `is.xyz.mpv.Utils`.

### 4.3 Loading rules (OBSERVED)

- Initial load: `isLoading` true until the first frame/state (VM:210). Shows the spinner when `showLoadingCircle` (`pref_show_loading`, default true). When the preference is false, the play/pause icon is shown instead while loading (MiddlePlayerControls.kt:106-127).
- Episode switch: `isLoadingEpisode` (VM:201, toggled by `updateIsLoadingEpisode`, VM:377).
- The centre container is shown while loading even if controls are hidden (PC:336-341), so the spinner is visible without a tap.

---

## 5. Corner controls

### 5.1 Top-left (TopLeftPlayerControls.kt:47-80)

Row with `spacedBy(padding.mediumSmall)` = 12 dp, vertically centred:
1. `ControlsButton(icon = Icons.AutoMirrored.Default.ArrowBack, onClick = onBackPress)`: back. Calls `onBackClick = onBackPress` from the activity (PC:440).
2. Column with `spacedBy(-padding.extraSmall)` = -4 dp (titles overlap slightly), clickable for the whole column:
   - Anime title: 16 sp, `bodyLarge`, `maxLines = 1`, ellipsis, white.
   - Episode/media title: 14 sp, `bodyLarge`, italic, `Color.White.copy(alpha = 0.5f)`, `maxLines = 1`, ellipsis.
   - Click on the column: `onTitleClick` = `viewModel.showEpisodeListDialog()` (PC:439).

`showEpisodeListDialog()` (VM:1209-1215): opens `Dialogs.EpisodeList` only if `currentAnime != null`.

Titles come from VM `animeTitle` and `mediaTitle` (VM:207-208; area 45).

### 5.2 Top-right (TopRightPlayerControls.kt:62-100)

Row with `Arrangement.Absolute.SpaceEvenly`, vertically centred. Children:
1. `AutoPlaySwitch(isChecked = autoPlayEnabled, onToggleAutoPlay, modifier = padding(vertical 16, horizontal 12).size(48 x 24))`. `autoPlayEnabled` from `pref_auto_play_enabled` default false (PlayerPreferences.kt:85). Toggle calls `VM.setAutoPlay(value)` (VM:802-810), which sets `playerUpdate = ShowTextResource(enable_auto_play | disable_auto_play)` and writes the preference.
2. Subtitles: `ControlsButton(Icons.Default.Subtitles, onClick = showSheet(SubtitleTracks), onLongClick = showPanel(SubtitleSettings), horizontalSpacing = 12 dp)`.
3. Audio: `ControlsButton(Icons.Default.Audiotrack, onClick = showSheet(AudioTracks), onLongClick = showPanel(AudioDelay), horizontalSpacing = 12 dp)`.
4. Quality, only when `isEpisodeOnline == true` (TopRightPlayerControls.kt:86): `ControlsButton(Icons.Default.HighQuality, onClick = showSheet(QualityTracks), onLongClick = showSheet(QualityTracks))`. Long click is also the quality sheet (no separate panel).
5. More: `ControlsButton(Icons.Default.MoreVert, onClick = showSheet(More), onLongClick = showPanel(VideoFilters))`.

`isEpisodeOnline` is `VM._isEpisodeOnline` (VM:198), a `Boolean?` in the composable signature. `null` and `false` both hide the quality button.

Icons: all default `iconSize = 20.dp` (ControlsButton default), tint white (`color = Color.White`).

Sheet/panel map: `showSheet` (VM:709-719) hides controls, closes panel and dialog. `showPanel` (VM:721-730) hides controls, closes sheet and dialog. `showDialog` (VM:732-741) hides controls, closes sheet and panel.

### 5.3 Bottom-left (BottomLeftPlayerControls.kt:52-85)

Row (fillMaxWidth, vertically centred):
1. Lock: `ControlsButton(Icons.Default.LockOpen, onClick = onLockControls)` (see 1.7).
2. Rotate: `ControlsButton(Icons.Default.ScreenRotation, onClick = onCycleRotation)`. `cycleScreenRotations()` (VM:839-854): if currently landscape (`SCREEN_ORIENTATION_LANDSCAPE`, `REVERSE_LANDSCAPE`, `SENSOR_LANDSCAPE`), set preference `defaultPlayerOrientationType = SensorPortrait` and request `SENSOR_PORTRAIT`; otherwise set `SensorLandscape` and request `SENSOR_LANDSCAPE`. Linux has no orientation: see T-46a-048.
3. Speed text: `ControlsButton(text = stringResource(AYMR.strings.player_speed, playbackSpeed), onClick = cycle, onLongClick = showSheet(PlaybackSpeed))`. Text format `%.2fx` (`player_speed`, strings.xml). Example "1.00x".
   - Cycle (BottomLeftPlayerControls.kt:68-72, OBSERVED): `newSpeed = if (playbackSpeed >= 2) 0.25f else playbackSpeed + 0.25f`; `onPlaybackSpeedChange(newSpeed)` (PC:551-553 calls `MPVLib.setPropertyDouble("speed", it.toDouble())`); `playerPreferences.playerSpeed().set(newSpeed)`.
   - Sequence from 1.0: 1.25, 1.5, 1.75, 2.0, then 2.0 >= 2 so next is 0.25, 0.5, 0.75, 1.0, ...
   - Preference `pref_player_speed` default 1f (PlayerPreferences.kt:75).
   - Long-press: `onOpenSheet(Sheets.PlaybackSpeed)`.
4. Chapter pill (AnimatedVisibility with fadeIn/fadeOut defaults): visible when `currentChapter != null && pref_show_current_chapter` (default true, PlayerPreferences.kt:25). Read with `.get()`, not as a flow (BottomLeftPlayerControls.kt:76), so a preference change only applies on the next recomposition. Click: `onOpenSheet(Sheets.Chapters)`.

`playbackSpeed` (VM:211) is initialised from `pref_player_speed`.

### 5.4 Bottom-right (BottomRightPlayerControls.kt:43-69)

Row, children in order:
1. Primary button, exactly one of:
   - If `skipIntroButton != null` (PC:502-505): `FilledControlsButton(text = skipIntroButton, onClick = viewModel::onSkipIntro, onLongClick = {})`.
   - Else if `customButton != null`: `FilledControlsButton(text = customButtonTitle, onClick = customButton::execute, onLongClick = customButton::executeLongPress)`.
   - Otherwise nothing.
2. PiP: only if `isPipAvailable` (`activity.isPipSupportedAndEnabled`, PlayerActivity.kt:139-142: system PiP feature and `pref_enable_pip` default true). `ControlsButton(Icons.Default.PictureInPictureAlt, onClick = onPipClick)`.
   - `onPipClick` (PC:507-511): if not `isLoadingEpisode`, `activity.enterPictureInPictureMode(activity.createPipParams())`.
3. Aspect: `ControlsButton(Icons.Default.AspectRatio, onClick = onAspectClick)`.
   - Cycle (PC:512-520): `Fit -> Stretch -> Crop -> Fit` based on the current `aspectRatio` preference value (`pref_player_aspect_state`, default `VideoAspect.Fit`, PlayerPreferences.kt:81).
   - `VM.changeVideoAspect(aspect)` (VM:813-837):
     - Crop: `panscan = 1.0`, `video-aspect-override = -1.0`.
     - Fit: `panscan = 0.0`, `video-aspect-override = -1.0`.
     - Stretch: `ratio = displayWidthPx / displayHeightPx` (real display metrics), `panscan = 0.0`, `video-aspect-override = ratio`.
     - Then saves preference and sets `playerUpdate = PlayerUpdates.AspectRatio`.
   - The pill text is the title of the new aspect: `Fit to screen`, `Stretched to screen`, `Cropped to screen` (`video_fit_screen`, `video_stretch_screen`, `video_crop_screen`).

Skip-intro text (VM:242, updated by `updateSkipIntroButton` and `showSkipIntroButton`): see section 5.5.

### 5.5 Skip-intro button text rules (OBSERVED, VM:2104-2150 and 2058-2100)

Text states (`_skipIntroText`, VM:242):
- `null`: no button.
- `Skip <ChapterTypeName>`: `player_skip_action = "Skip %1$s"` with the chapter type name from `ChapterType.getStringRes()` (`ChapterUtils.kt:13-19`): Opening "Opening", Ending "Ending", Recap "Recap", MixedOp "Mixed opening", Other null (no button).
- `Don't skip`: `player_aniskip_dontskip` (only in Netflix-style countdown).
- `<name> skipped`: `player_aniskip_skip = "%s skipped"`, used as the text of the seek-with-text message after the skip.

`setChapter(position)` (VM:2063-2102) runs on each position update:
- Finds the chapter with the greatest start <= position (`getCurrentChapter`, VM:2150-2155).
- Sets `currentChapter` if changed.
- If `pref_enable_skip_intro` (default true, PlayerPreferences.kt:48) is false, return.
- If chapter type is `Other`: clear the button and reset the countdown.
- Else compute `nextChapterPos = chapters[index+1].start` or current position when last.
  - If Netflix-style (`pref_enable_netflixStyle_aniskip`, default false): countdown. Shows a toast `"Skip Intro: " + player_aniskip_dontskip_toast` (`"Skipping %s in %d seconds"`) the first time, then shows button text `Don't skip` while counting down from `waitingSkipIntro` (`pref_waiting_time_aniskip` default 5), then auto-seeks when it reaches 0.
  - Else if `pref_enable_auto_skip_ani_skip` (default false): seek immediately with the text `player_intro_skipped` (`"%1$s skipped"`).
  - Else: show the button text `Skip <type>`.

`onSkipIntro()` (VM:2133-2148): if in Netflix countdown (`waitingSkipIntro > 0 && netflixStyle`) cancel (`waitingSkipIntro = -1`) and return. Else seek to the next chapter start (or current position) with text `player_aniskip_skip`.

Note: the toast prefix `"Skip Intro: "` is a hard-coded English literal in the Kotlin (VM:2080-2085), not a resource. Proposal: a resource.

### 5.6 Custom primary button (OBSERVED)

`CustomButton.execute()` (VM:2167-2169) sends `MPVLib.command(arrayOf("script-message", "call_button_$id"))`. `executeLongPress()` sends `"call_button_${id}_long"`. The button title is `primaryButtonTitle` (VM:322, set by `setPrimaryCustomButtonTitle`, VM:2156-2158 from `button.name`). The `primaryButton` flow (VM:325) holds which custom button is the primary one. Custom button storage is area 45 / part 2.

---

## 6. Seekbar (SeekBar.kt, PC:369-411)

### 6.1 Visibility (OBSERVED)

`SeekbarWithTimers` is inside `AnimatedVisibility((controlsShown || seekBarShown) && !areControlsLocked)`. So the seekbar shows whenever controls are shown, or when `seekBarShown` is set by gestures (`showSeekBar()`), even if controls are hidden. `seekBarShown` initial value = `!pref_hide_controls` = `true` (default `hideControls` is false, `PlayerPreferences.kt:39`, VM:278). Proposal: keep initial true.

Hide: `hideSeekBar()` after horizontal drag end and at the end of double-tap feedback (GH:100-102, 198).
Show: `showSeekBar()` on gesture seek when `pref_show_seekbar` (default false) (GH:215, VM:991, VM:1008, VM:1017).

### 6.2 Value shown (OBSERVED, SeekBar.kt:84-91)

```
position = if (isGestureSeeking) seekPosition
           else if (isSeeking) internalSeekPosition
           else playerPosition
```
`internalSeekPosition` is the last value dragged by the user (set in onValueChange, SeekBar.kt:112).

The slider value is `position.coerceIn(0f, duration)`, range `0f..duration` (SeekBar.kt:109-110).

### 6.3 Interaction (OBSERVED, PC:397-409)

- `onValueChange(v)`: `VM.updateSeekPos(v)` and `VM.updateIsSeeking(true)`. Thumbnail refresh happens here (VM:574-608).
- `onValueChangeFinished(v)`: `VM.updatePlayBackPos(v)`, `VM.updateIsSeeking(false)`, then `VM.seekTo(v.toInt(), preciseSeeking)`.
  - `seekTo` (VM:747-750): ignores positions outside `0..duration(int)`, else `seek <pos> absolute` or `absolute+keyframes` when not precise.
- `readAheadValue` = VM `_readAhead` (VM:266, updated from mpv demuxer cache by area 45): drawn as a secondary fill (SeekBar.kt:118, colour `inversePrimary`).

Seeker colours (SeekBar.kt:131-136): progress primary, thumb primary, track `colorScheme.background`, read-ahead `colorScheme.inversePrimary`. Source of defaults: Seeker library `SeekerDefaults.seekerColors` (external).

### 6.4 Chapter ticks (OBSERVED, SeekBar.kt:119-129, with a defect)

```
segments = chapters
    .filter { it.start in 0f..duration }
    .let {
        if (it.isNotEmpty() && it[0].start != 0f) persistentListOf(Segment("", 0f)) + it
        else it
    } + it
```

Rules:
1. Keep chapters whose start is within `0..duration` (inclusive).
2. If the list is non-empty and the first chapter does not start at 0, prepend a synthetic unnamed segment at 0.
3. Then, by the Kotlin expression as written, the result is concatenated with the filtered list a second time (`... + it`).

Defect (OBSERVED, likely unintended): every chapter appears twice in the segments list passed to Seeker. The comment only describes step 2. Visually this draws duplicate ticks at identical positions; it may also cause the Seeker library to double-count segments (external library not verified). Proposal: emit each segment once, prepend the synthetic 0 segment only when needed. Recorded as E-46-031 and acceptance test in T-46a-010.

The `chapters` list passed to `ChaptersSheet` and to `selectChapter(index)` is the un-duplicated `VM.chapters` (PC:610-611 and VM:500-503), so the index mapping is correct; only the seekbar copy is wrong.

`chapters` is built with `remember(indexedChapters) { indexedChapters.map { it.toSegment() }.toImmutableList() }` (PC:130-132). `IndexedSegment.toSegment()` = `Segment(name, start, color)` (SeekBar.kt:65).

### 6.5 Timers (OBSERVED, SeekBar.kt:138-171)

Left timer (position): `VideoTimer(value = position, isInverted = timersInverted.first (false), onClick = positionTimerOnClick (no-op), width 92 dp)`. Text = `Utils.prettyTime(value.toInt(), false)`.

Right timer (duration/remaining): `VideoTimer(value = if (timersInverted.second) position - duration else duration, isInverted = timersInverted.second, onClick = durationTimerOnCLick, width 92 dp)`.
- Non-inverted: total duration, plain `mm:ss` or `h:mm:ss`.
- Inverted: `position - duration` (negative until the end), formatted with sign: `-mm:ss` (because the value is negative), and `+mm:ss` when zero or positive.
- Click toggles the persisted preference: `playerPreferences.invertDuration().set(!invertDuration)` (PC:407). `pref_invert_duration`... the key is `invert_duration` (no prefix), default false (PlayerPreferences.kt:80).

`VideoTimer` text: `Utils.prettyTime(value.toInt(), isInverted)`, white, centred, `fillMaxHeight`, `ripple()` indication (SeekBar.kt:150-171).

Row: height 48 dp, `spacedBy(4 dp)`, vertically centred (SeekBar.kt:94-98). Seeker weight 1.

Note: the position timer's click is wired to a no-op lambda at the root (PC:408 `positionTimerOnClick = {}`), but the `VideoTimer` still shows a ripple. Proposal: drop the ripple for the position timer; keep duration toggle.

---

## 7. Thumbnail preview (ThumbnailPreview.kt, PC:558-568)

### 7.1 Visibility (OBSERVED)

Visible when `isSeeking && image != null` (ThumbnailPreview.kt:78-83), fade in and out with default `fadeIn()` / `fadeOut()`.

The image comes from `VM.thumbnailImage` (VM:~260s, a `MutableStateFlow<ImageBitmap?>`), created by `createThumbnail` cropping a tile (VM:610-613) and cleared by `updateIsSeeking(false)` (VM:615-620).

### 7.2 Size (OBSERVED, ThumbnailPreview.kt:55-70)

`remember(image)`:
- If image is non-null: `aspect = width / height`.
  - If `aspect < 16/9`: size = `(124.dp * aspect, 124.dp)`.
  - Else: size = `(220.dp, 124.dp)`.
- If image is null: `(220.dp, 124.dp)` (unreachable while visible).

### 7.3 Position (OBSERVED, ThumbnailPreview.kt:85-105)

- `screenWidth` = width of the BoxWithConstraints (the row width inside the overlay).
- `timerWidth = 96.dp` (comment: width 92 + padding 4).
- `seekbarWidth = screenWidth - 96 - 96`.
- `progress = (positionS / durationS).coerceIn(0, 1)` if duration > 0, else 0. `positionS = seekPosition.toLong()`.
- `targetX = startInset + 96 + seekbarWidth * progress`, `startInset` = safe-drawing start inset (0 on desktop).
- `minX = startInset + 8`, `maxX = screenWidth - previewWidth - endInset - 8`.
- `constrainedX`:
  - if `targetX - previewWidth/2 < minX` then `minX`,
  - else if `targetX + previewWidth/2 > screenWidth - endInset - 8` then `maxX`,
  - else `targetX - previewWidth/2`.

Note: these numbers are fixed Android layout values. Proposal: derive from the seekbar's real allocation (the bounding box of the seek scale) instead of the constants 96/96. The result is a card centred over the playhead and clamped 8 px from the edges.

### 7.4 Chapter label (OBSERVED, ThumbnailPreview.kt:123-152)

`seekingChapter = chapters.lastOrNull { it.start <= positionS }`. Shown only when `name` is non-empty (synthetic segment name is `""`, so no label before the first chapter). Pill: background black 60%, radius 6 dp, padding h 8 dp v 4 dp, text 11 sp bold white, single line ellipsis. Position: bottom-start with padding 8 dp inside the card. Transition: fade 200 ms in and out.

### 7.5 Card (OBSERVED, ThumbnailPreview.kt:107-114)

Card: container black, shape radius 8 dp, elevation 8 dp (`defaultElevation`), size `(previewWidth, previewHeight)`, content `Image(ContentScale.Crop)` filling the card.

### 7.6 Tile fetch and cache (OBSERVED, `updateSeekPos` at VM:574-608, cache at VM:257-262)

`updateSeekPos(pos)` steps:
1. `thumbInfo = thumbnailInfo.value ?: return`. Thumbnail info comes from the source's trickplay metadata (area 45).
2. `info = thumbInfo.tileInfo.lastOrNull { it.timeMs <= pos * 1000 }`. `tileInfo` entries carry `imageIndex`, `x`, `y`, `width`, `height`.
3. If `thumbnailTileCache[info.imageIndex]` exists, `createThumbnail` immediately.
4. Otherwise, rate limit: if `now - lastThumbnailFetch < 2 s` return. Set `lastThumbnailFetch = now`. Cancel previous fetch job. Launch IO job: `source = currentSource as? AnimeHttpSource ?: return`; `bitmap = source.getImageTile(thumbInfo.imageTileUrls[imageIndex])`; on success store in cache and `createThumbnail` on UI.
5. Exceptions other than `CancellationException` are logged (`"Failed to fetch thumbnails tiles"`).

Cache: `LinkedHashMap` with `removeEldestEntry = size > 3` (VM:257-262), so at most 3 tiles are kept (LRU by insertion order, not access order, because `accessOrder` is not enabled; verify the constructor argument in VM:257-262 if precision matters). `resetState()` clears the cache on episode change (VM:1023-1031).

`createThumbnail(tile, info)` crops `Bitmap.createBitmap(tile, x, y, width, height)` and publishes `asImageBitmap()`.

Proposal: decode tiles with the `image` crate or `gdk::Texture` from bytes; crop via `gdk::Texture` subsetting (`gtk::Snapshot`) or `cairo` `ImageSurface`. Max 3 tiles in memory, 2 s fetch throttle. Keep.

---

## 8. Volume, brightness, overlay

### 8.1 Volume pill (OBSERVED, VerticalSliders.kt:175-257, PC:270-289)

Inputs: `volume` = `VM.currentVolume` (device stream level Int), `mpvVolume` = `VM.currentMPVVolume` (mpv `volume` Int, normally 0..100, up to 100 + cap while boosting), `range = 0..VM.maxVolume`, `boostRange = if (cap > 0) 0..cap else null`, `displayAsPercentage = pref_display_vol_as_per` (default true, PlayerPreferences.kt:40).

Derived:
- `percentage = roundToInt(percentage(volume, range) * 100)` (0..100), where `percentage(v, r) = ((v - start) / (end - start)).coerceIn(0, 1)`.
- `boostVolume = mpvVolume - 100` (≤ 0 means not boosting, values > 0 mean boosted).

Texts (`getVolumeSliderText`, VerticalSliders.kt:237-257):
- Device line: if displayAsPercentage then `NumberFormat.getPercentInstance().format(percentage/100)` (locale percent e.g. "45%"), else `NumberFormat.getIntegerInstance().format(volume)`.
- Boost line:
  - `0` → `""`.
  - `0..1000` → `"+" + integer(boost)`.
  - `-100..-1` → `"-" + integer(-boost)`.
  - else → `integer(boost)`.

Bar: `VerticalSlider(value = if displayAsPercentage percentage else volume, range = if displayAsPercentage 0..100 else range, overflowValue = boostVolume, overflowRange = boostRange)`. Overflow segment is drawn with `errorContainer` colour above the main fill (VerticalSliders.kt:123-134). Overflow fraction = `percentage(boostVolume, 0..cap)`. For negative boost, the overflow is `percentage(negative, range)` which clamps to 0.

Icon thresholds (VerticalSliders.kt:224-233), evaluated on the rounded percentage in order:
- `0` → `VolumeOff`.
- `0..30` → `VolumeMute` (so 30 is mute-icon).
- `30..60` → `VolumeDown` (30 already matched above; so 31..60).
- `60..100` → `VolumeUp` (61..100).
- otherwise `VolumeOff`.

Net effect: 0 → off; 1..30 → mute; 31..60 → down; 61..100 → up. Note: in Kotlin, `in 0..30` is checked before `in 30..60`, so 30 is mute.

Bar geometry (VerticalSliders.kt:104-136): outer box height 120 dp, aspect 0.45 (width = 54 dp). Inner track fillMaxWidth(0.5) of that, rounded 16 dp, background `colorScheme.background`. Fill height animated with `animateFloatAsState(percentage)` default spec, colour `colorScheme.tertiary`. Overflow height animated similarly.

Note the bug-like detail: `VerticalSliderInternal` uses `modifier` on both the outer and inner Box (VerticalSliders.kt:109 `modifier = modifier`). Modifier applied twice, e.g., `.padding` passed by caller would double. Caller passes no modifier, so no effect today.

### 8.2 Volume gesture to display (OBSERVED)

Each vertical drag event calls `displayVolumeSlider()` (GH:281), which sets `isVolumeSliderShown = true`. A `LaunchedEffect(volume, mpvVolume, isVolumeSliderShown)` (PC:197-200) waits 2000 ms and sets it false if still true. Any change restarts the wait.

Brightness is the same with `(brightness, isBrightnessSliderShown)` (PC:201-204).

### 8.3 Brightness pill (OBSERVED, VerticalSliders.kt:139-173)

- Text above the bar: `(brightness * 100).toInt()` as a plain integer, no % sign. Range can be negative, e.g., `-40`.
- Bar: `VerticalSlider(value = brightness.coerceIn(0,1), range = positiveRange (0..1), overflowRange = negativeRange (0..0.75), overflowValue = (-brightness).coerceIn(0..0.75))`. So negative brightness appears as an error-coloured overflow up to 75%.
- Icon by `percentage(brightness, positiveRange)` (clamped 0..1), first match wins:
  - `-1..0` (i.e., exactly 0) → `ModeNight`.
  - `0..0.3` → `BrightnessLow` (0.3 itself also goes here, since this branch is first).
  - `0.3..0.6` → `BrightnessMedium` (0.6 itself goes to Medium).
  - `0.6..1` → `BrightnessHigh`.
  - otherwise `BrightnessMedium`.

Initial brightness (VM:286-291): `Settings.System.SCREEN_BRIGHTNESS` (0..255) normalised to 0..1 via `Float.normalize(0,255,0,1)`; falls back to 0 on exception. Android-specific. Linux: see 8.5.

### 8.4 Brightness overlay (OBSERVED, BrightnessOverlay.kt:14-32)

`if (brightness < 0)`: full-size `Canvas` filled black with `graphicsLayer.alpha = abs(brightness)`. So `-0.75` gives alpha 0.75 (maximum). When brightness >= 0 nothing is drawn.

Placed as the last child of the root (PC:668-670), so it is above controls and below nothing. It is not touch-sensitive.

### 8.5 System backends (Linux proposal; Android behaviour above)

- Device volume: Android `AudioManager.setStreamVolume(STREAM_MUSIC, v, 0)` with `v` in `0..maxVolume` (VM:779-787). Linux replacement: PipeWire/PulseAudio default sink volume. Proposal: `wpctl set-volume @DEFAULT_AUDIO_SINK@ <p>%` via `std::process::Command` or the `libpulse-binding` crate with a 1 to 15 step mapping. Use `maxVolume = 15` as the step count, equal to Android default, so the same gesture sensitivity applies.
- Screen brightness: Android `window.attributes.screenBrightness` (VM:752-759). Linux: no per-window brightness. Proposal: (a) hardware backlight via `org.freedesktop.login1.Session.SetBrightness` or `brightnessctl`, only for values 0..1 (verify on this machine, see T-46a-049); (b) for negative values use the software veil from 8.4 (always available). If no backlight device exists, positive values become a no-op and only the veil works.
- Boost volume: mpv `volume` property, cap = `pref_audio_volume_boost_cap`, mpv `volume-max` set to `cap + 100` at init.

---

## 9. Player update pill (PlayerUpdates.kt, PC:291-320)

State: `VM.playerUpdate` (MutableStateFlow of `PlayerUpdates`, VM:298). Variants (PlayerEnums.kt:133-139): `None`, `DoubleSpeed` (no UI, commented out in PC:310), `AspectRatio`, `ShowText(value: String)`, `ShowTextResource(textResource)`.

Rules (OBSERVED, PC:293-299):
- A `LaunchedEffect(currentPlayerUpdate, aspectRatio)`: if the update is `DoubleSpeed` or `None`, return. Otherwise `delay(2000)` and set `playerUpdate = None`.
- Restarted on any change of the update or of the aspect preference, so a second update within 2 s extends the display.
- Visible when `currentPlayerUpdate !is None` (PC:300-305). Fade in with `playerControlsEnterAnimationSpec` (100 ms), fade out with exit (300 ms).

Content (PC:309-319):
- `AspectRatio` → `stringResource(aspectRatio.titleRes)` where `aspectRatio` is the live preference (so the pill shows the new aspect).
- `ShowText` → the given string.
- `ShowTextResource` → resolved resource.

Pill (PlayerUpdates.kt:35-47): radius 16 dp, background black 40%, padding v 8 dp h 16 dp, `animateContentSize`, centred content, default text colour (white via `LocalContentColor`).

Position: horizontally centred, vertical bias 0.2 (20% from the top of the overlay).

Who sets it: aspect change (VM:836), auto-play toggle (VM:805-807), and gesture release for the long-press reset (GH:166). The long-press release sets `playerUpdate = None` immediately, which cancels any pill.

---

## 10. Dialogs

### 10.1 Host (PlayerDialogs.kt:26-53)

`when (dialogShown)`:
- `Dialogs.None` → nothing.
- `Dialogs.EpisodeList` → `EpisodeListDialog(...)`.
- `Dialogs.IntegerPicker(...)` → `IntegerPickerDialog(defaultValue, minValue, maxValue, step, nameFormat, title, onChange, onDismissRequest)`.

`onEpisodeClicked(id)` closes the dialog and calls `activity.changeEpisode(id)` (PC:661-664).
`onBookmarkClicked = viewModel::bookmarkEpisode`, `onFillermarkClicked = viewModel::fillermarkEpisode` (PC:659-660). Their DB effects are area 45.

### 10.2 Episode list (EpisodeListDialog.kt:51-244)

Inputs: `displayMode: Long?` (anime display mode, `anime?.displayMode`, PC:654), `currentEpisodeIndex` (`getCurrentEpisodeIndex()`), `episodeList` (`VM.currentPlaylist`), `dateRelativeTime` (`VM.relativeTime`), `dateFormat` (`VM.dateFormat`).

Shell: `PlayerDialog(title = stringResource(AYMR.strings.episodes) = "Episodes", modifier = fillMaxHeight(0.8) x fillMaxWidth(0.8), onDismissRequest)` (EpisodeListDialog.kt:67-71). No OK/Cancel (onConfirmRequest null).

Display-mode test (OBSERVED, EpisodeListDialog.kt:87): `displayMode == Anime.EPISODE_DISPLAY_NUMBER` compares the whole Long. `displayMode` is a bit field (`Anime.kt:253-257`): `EPISODE_DISPLAY_MASK = 0x00100000L` is the only display bit, but other flags (`EPISODE_SHOW_NOT_PREVIEWS = 0x800`, `EPISODE_SHOW_NOT_SUMMARIES = 0x1000`, season flags) live in the same Long. Any anime with those bits set shows episode names instead of numbers. Proposal: `(displayMode & EPISODE_DISPLAY_MASK) == EPISODE_DISPLAY_NUMBER`. Edge E-46-033.

Scroll position: `itemScrollIndex = (episodeList.size - currentEpisodeIndex) - 1` (EpisodeListDialog.kt:64). Because the list is shown reversed (newest first), this places the current episode at the top of the visible area. `rememberLazyListState(initialFirstVisibleItemIndex = itemScrollIndex)`.

Ordering: `items(episodeList.reversed(), key = "episode-${id}", contentType = "episode")` (EpisodeListDialog.kt:79-82). So the list is the reverse of `currentPlaylist`. The playlist order is area 45 (sorted by episode number in the VM).

Scrollbar: `VerticalFastScroller(listState = episodeListState)` (presentation-core `VerticalFastScroller.kt:66`).

Per-row title (EpisodeListDialog.kt:87-94):
- If `displayMode == Anime.EPISODE_DISPLAY_NUMBER` (`0x00100000L`): `"Episode " + formatEpisodeNumber(episode_number)` using `AYMR.strings.display_mode_episode = "Episode %1$s"`.
- Else `episode.name`.

`formatEpisodeNumber(d)` = `DecimalFormat("#.###")` with `.` decimal separator (`app/src/main/java/eu/kanade/presentation/util/ItemNumberFormatter.kt:15-17`). Examples: 1.0 → `1`; 12.5 → `12.5`; 1.2345 → `1.234` (HALF_EVEN); 0.5 → `.5` (no leading zero because the pattern has no `0`). Edge E-46-022.

Date (EpisodeListDialog.kt:96-107): if `dateUpload > 0`, `LocalDate.ofInstant(ofEpochMilli(v), systemDefault()).toRelativeString(context, relative = dateRelativeTime, dateFormat)`; else `""`.
`toRelativeString` (`app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:47-72`):
- `relative == false` → `dateFormat.format(date)` (default pattern `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)`).
- Else difference = days from the date to today (`ChronoUnit.DAYS.between(this, now)`):
  - `< -7` → formatted date.
  - `< 0` → `upcoming_relative_time` plural with |d|.
  - `< 1` → `relative_time_today`.
  - `< 7` → `relative_time` plural with d.
  - else → formatted date.

Row (EpisodeListItem, EpisodeListDialog.kt:124-244):
- Local mutable copies `isBookmarked`, `isFillermarked` initialised from the episode (lines 134-135).
- Bookmark button (`IconButton`): icon `Bookmark`, tint `colorScheme.primary` when bookmarked else `onSurface`, alpha 1 when bookmarked else `DISABLED_ALPHA` (0.38). Icon height capped at `textHeight - 2 dp`.
- Fillermark button: icon `AutoMirrored.Filled.Label`, tint `colorScheme.tertiary` when marked else `onSurface`, alpha as above.
- Click on bookmark toggles: `episode.bookmark = b` (mutates the model object), `isBookmarked = b`, `onBookmarkClicked(episode.id, b)`. Fillermark same with `fillermark`.
- Spacer 2 dp.
- Text colour: bookmarked → primary; else fillermarked → tertiary; else onSurface. (Bookmark takes precedence over fillermark, EpisodeListDialog.kt:143-149.)
- `textAlpha = if (episode.seen) DISABLED_ALPHA else 1f` (watched episodes are dimmed).
- Current episode: `FontWeight.Bold` and `FontStyle.Italic` for both title and subtitle (EpisodeListDialog.kt:151-152). Otherwise Normal.
- Title: `bodyMedium`, `maxLines = 1`, ellipsis; measured height stored for icon sizing (`onTextLayout`).
- Subtitle row: date (`labelMedium`, one line), then `DotSeparatorText` if scanlator is non-null, then scanlator (`labelMedium`).

Defect (OBSERVED, EpisodeListDialog.kt:212-227): the date is passed as `""` when absent, and the check is `if (date != null)`. So when there is no upload date but a scanlator exists, a leading separator dot is shown before the scanlator. Proposal: check `!date.isEmpty()`.

Click on a row: `onEpisodeClicked(episode.id)` (the whole row, EpisodeListDialog.kt:169). Closes dialog and changes episode.

Crash risk (OBSERVED, EpisodeListDialog.kt:85): `episodeList[currentEpisodeIndex]` is evaluated per row. If `currentEpisodeIndex == -1` (current episode not in the playlist; VM:1187-1189 returns `indexOfFirst` which is -1 when not found), this throws `IndexOutOfBoundsException`. Proposal: compare against `current_episode_id` instead of indexing (E-46-017).

### 10.3 Integer picker (IntegerPickerDialog.kt:12-46)

Inputs come from a Lua script message `launch_int_picker` (VM:904-919): data `"title|nameFormat|start|stop|step|property"`. Default value = `MPVLib.getPropertyInt(property)`. `onChange = setPropertyInt(property, value)`. `onDismissRequest = showDialog(None)`.

Behaviour (OBSERVED):
- `values = (minValue..maxValue step step).toList()`. If `step <= 0` Kotlin throws `IllegalArgumentException`. If `(max - min) % step != 0`, the last value is excluded (e.g., 0..10 step 3 gives 0,3,6,9).
- `items = values.map { String.format(nameFormat, it) }`; `nameFormat` is a Java format string from the script.
- Dialog: `PlayerDialog(title, modifier = fillMaxWidth(0.5), onConfirmRequest = null, onDismissRequest = { onChange(newValue); onDismissRequest() })`. So every dismissal (back, outside tap) commits `newValue`, no Cancel.
- Picker: `WheelTextPicker(items, startIndex = values.indexOfFirst { it == defaultValue }.coerceAtLeast(0), onSelectionChanged = { newValue = values[it] })`, size default 128 dp x 128 dp.

Defects (OBSERVED):
- `var newValue = defaultValue` is a plain local, not `remember`ed (IntegerPickerDialog.kt:22). If the composable recomposes while the user scrolls, `newValue` resets to `defaultValue` even though the wheel keeps its own index. A dismissal can then commit the wrong value.
- If `defaultValue` is not in `values`, the wheel starts at index 0, but `newValue` remains `defaultValue`, so dismissal commits a value that is not shown.
Proposal: keep state in a `Cell`/GObject property, initialise it from the displayed index, and commit only the displayed item. E-46-027.

### 10.4 PlayerDialog shell (PlayerDialog.kt:20-74)

`BasicAlertDialog` with `DialogProperties(dismissOnBackPress = true, dismissOnClickOutside = true, usePlatformDefaultWidth = false, decorFitsSystemWindows = false)`.
Surface: `MaterialTheme.shapes.large`, `fillMaxWidth`, tonal elevation 1 dp.
Column padding 16 dp: title (`titleLarge`, `onSurface`), content slot, optional buttons row.
Buttons (only if `onConfirmRequest != null`): row `SpaceBetween`, `TextButton(Cancel = MR.strings.action_cancel "Cancel")` on the left, `TextButton(OK = MR.strings.action_ok "OK")` on the right.
`onConfirm` = call `onConfirmRequest?.invoke()` then `onDismissRequest()`.

Proposal: `gtk::Window` modal with `gtk::Box` (title label, content, optional action row), Escape closes, click outside (on the dimmed parent) closes.

---

## 11. Panels

### 11.1 Host (PlayerPanels.kt:45-79)

`AnimatedContent(targetState = panelShown, contentKey = name, contentAlignment = CenterEnd, transitionSpec = fadeIn() + slideInHorizontally { it/3 } togetherWith fadeOut() + slideOutHorizontally { it/2 })`.
Content:
- `None` → empty box, fill height.
- `SubtitleSettings` → `SubtitleSettingsPanel(onDismiss)` (part 2).
- `SubtitleDelay` → `SubtitleDelayPanel(onDismiss)` (part 2).
- `AudioDelay` → `AudioDelayPanel(onDismiss)` (this part).
- `VideoFilters` → `VideoFiltersPanel(onDismiss)` (part 2).

Panel card (`panelCardsColors`, PlayerPanels.kt:81-92): `CARDS_MAX_WIDTH = 420.dp`. Container = `colorScheme.surface` with alpha `pref_panel_opacity / 100` (default 60 → 0.6). Disabled container = `surfaceDim` with the same alpha.

Panels are right-aligned; the panel host covers the full screen and the card is at the end. Panel show/hide also hides controls (VM:721-730).

### 11.2 Audio delay panel (AudioDelayPanel.kt:50-105)

Layout: `ConstraintLayout` fillMaxSize, padding 16 dp. Card `DelayCard` linked `top` to `bottom` with bias 0.8 and `end` to parent end (AudioDelayPanel.kt:75-78). So the card sits near the bottom-right.

State: `delay: Int` in milliseconds (AudioDelayPanel.kt:64), initial `(MPVLib.getPropertyDouble("audio-delay") * 1000).toInt()`. `LaunchedEffect(delay) { MPVLib.setPropertyDouble("audio-delay", delay / 1000.0) }` (AudioDelayPanel.kt:65-67), so mpv is updated on every change, including first composition (mpv may have a different value only briefly).

Buttons passed to DelayCard:
- `onDelayChange = { delay = it }`.
- `onApply = { preferences.audioDelay().set(delay) }`: persists `pref_audio_delay` (default 0). Not persisted otherwise.
- `onReset = { delay = 0 }`.
- `title = AudioDelayCardTitle(onClose)`.
- `delayType = DelayType.Audio`.

Title (AudioDelayPanel.kt:84-105): row, `SpaceBetween`, text `AYMR.strings.player_sheets_audio_delay_title = "Audio delay"` in `headlineMedium`, and a close `IconButton` with `Close` icon 32 dp.

### 11.3 Shared DelayCard (SubtitleDelayPanel.kt:202-319, owner part 2; behaviour needed here)

Card: `widthIn(max = 420.dp)`, `animateContentSize`, colours `panelCardsColors()`. Column scrollable (`verticalScroll`), padding h 16 v 8, spacing 8 dp.

Children:
1. Title slot.
2. `OutlinedNumericChooser(label = "Delay", value = delay, step = 50, min = Int.MIN_VALUE, max = Int.MAX_VALUE, suffix = "ms")`.
3. Extra settings slot (empty for audio).
4. Sync row: two buttons `weight(1)` each.
5. Action row: "Set as default" (`onApply`, enabled when not syncing) and a `FilledIconButton(Refresh)` (`onReset`, enabled when not syncing).

Sync-by-tapping algorithm (SubtitleDelayPanel.kt:242-263, OBSERVED):
- `isDirectionPositive: Boolean?` = null (idle), true, or false.
- Button 1 onClick: `if null → (delayType == Audio)` else null. So for Audio, button 1 starts with `true`. For Subtitle it starts with `false`.
- Button 2 onClick: `if null → (delayType != Audio)` else null. For Audio it starts with `false`; for Subtitle with `true`.
- Button 1 enabled = `isDirectionPositive != (delayType == Audio)`. Audio: enabled when not `true`. Subtitle: enabled when not `false`.
- Button 2 enabled = `isDirectionPositive != (delayType == Subtitle)`. Audio: enabled when not `false`. Subtitle: enabled when not `true`.
- So the active button is disabled and the other button stops the sync (pressing the other button sets state to null).
- Labels: Audio: button 1 "Sound heard", button 2 "Sound spotted". Subtitle: button 1 "Voice heard", button 2 "Text seen".
- When running (state non-null): `finalDelay = startingDelay + sign * elapsedMs`, updated every 20 ms (the comment says "Arbitrary delay of 20ms"). `sign = +1` when `isDirectionPositive`, else -1. `startingDelay` = delay at start.
- When stopped (null): `onDelayChange(finalDelay)`.
- While running, the numeric field still shows the old `delay` value (the parent state only updates on stop). Proposal: show the live value while running. Note E-46-029.

Meaning: "Sound heard" starts a timer as the user hears the sound and "Sound spotted" stops it, so delay grows with the time between the two presses. Direction semantics are as the comments state: `true` = heard to spotted.

Note: `OutlinedNumericChooser` for `Int.MIN_VALUE..Int.MAX_VALUE` range; the decrement `value - 50` wraps around at Int.MIN_VALUE (E-46-024).

### 11.4 Subtitle equivalents (out of part 1)

`SubtitleDelayPanel` also applies `sub-delay` and `secondary-sub-delay` and `sub-speed` (range 0.1..1 on the mpv side). It is part 2 and described there.

---

## 12. Sheet container and sheet host

### 12.1 PlayerSheet (PlayerSheet.kt:76-200)

Parameters: `onDismissRequest`, `modifier`, `tonalElevation = 1 dp`, `dismissEvent = false`, `content`.

Dimensions:
- `maxWidth = 720 dp` when the configuration is landscape, else `420 dp` (PlayerSheet.kt:87-91).
- `maxHeight = screenHeightDp * 0.95`.
- Surface: `sizeIn(maxWidth, maxHeight)`, shape `extraLarge` with bottom corners zero (anchored to the bottom edge), tonal elevation 1 dp.
- Padding: `WindowInsets.systemBars` only top and horizontal sides.

Scrim: full-size `Box` with black at `backgroundAlpha`, which animates 0 → 0.5 with `tween(350)` on first composition (PlayerSheet.kt:188-190, 94-99). Tapping the scrim calls `internalOnDismissRequest` (only when the sheet is open, value == 0).

Drag (AnchoredDraggableState, PlayerSheet.kt:101-110, 146-172):
- Anchors: `0 at 0 px` (open, top) and `1 at height px` (closed, bottom). Updated on size change (PlayerSheet.kt:137-143).
- `initialValue = 1` (closed), then `animateTo(0)` on first composition (PlayerSheet.kt:192-193): opens.
- Snap animation `tween(350)`. Decay `rememberSplineBasedDecay`.
- `positionalThreshold = 56 dp`, `velocityThreshold = 125 dp` (converted to px).
- `Orientation.Vertical`.

Nested scroll (PlayerSheet.kt:202-250): a connection that forwards user pre-scroll deltas with negative y (upwards) into the drag state, and post-scroll deltas to the drag state; and on fling: upward flings settle when offset > min anchor; downward flings > 0 settle (settle = animateTo nearest anchor by velocity).

Dismissal:
- `dismissEvent` true → `backgroundAlpha = 0`, `animateTo(1)`, `onDismissRequest()` (PlayerSheet.kt:112-118).
- Back press: `BackHandler(enabled = targetValue == 0, onBack = internalOnDismissRequest)` (PlayerSheet.kt:180-183).
- `internalOnDismissRequest` (PlayerSheet.kt:120-127): if `currentValue == 0` then animate scrim to 0 and sheet to 1. Then `snapshotFlow(currentValue).drop(1).filter(== 1).collectLatest(onDismissRequest)` (PlayerSheet.kt:192-197) calls the caller's dismissal once the sheet reaches the closed anchor.

Interaction with sheets: `PlayerSheets` passes `dismissSheet` so the sheet animates out before the VM clears `sheetShown` (VM:701-705 sets `dismissSheet = true`; `resetDismissSheet` on sheet none).

Proposal: GTK modal `gtk::Window` 420 px wide (720 px landscape) bottom-aligned, with `gtk::GestureDrag` for drag-down-to-close and Escape for back. The 350 ms show and hide, the 56 px / 125 px thresholds, the 0.5 scrim alpha and the 95% height cap carry over.

### 12.2 Sheet host (PlayerSheets.kt:101-197)

`when (sheetShown)`:
- `None` → nothing.
- `SubtitleTracks` → `SubtitlesSheet(tracks, selectedTracks, onSelect = selectSub, onAddSubtitle = picker("*/*"), onOpenSubtitleSettings = showPanel(SubtitleSettings), onOpenSubtitleDelay = showPanel(SubtitleDelay), onDismissRequest)`. Picker: `ActivityResultContracts.OpenDocument()` with MIME `*/*`; `null` result is ignored; otherwise `addSubtitle(uri)` (PlayerSheets.kt:104-119).
- `AudioTracks` → `AudioTracksSheet(tracks, selectedId, onSelect = selectAudio, onAddAudioTrack = picker("*/*") → addAudio, onOpenDelayPanel = showPanel(AudioDelay))` (PlayerSheets.kt:121-136).
- `QualityTracks` → `QualitySheet(isLoadingHosters, hosterState, expandedState, selectedVideoIndex, onClickHoster, onClickVideo, displayHosters = (showFailed, showEmpty), dismissSheet)` (PlayerSheets.kt:138-150).
- `Chapters` → if `chapter == null` return (sheet not shown), else `ChaptersSheet(chapters, currentChapter, onClick = onSeekToChapter(indexOf(it)), dismissSheet)` (PlayerSheets.kt:152-161). `onSeekToChapter` (PC:612-616): `VM.selectChapter(index)` (seek to `chapters[index].start`, absolute, precise), `dismissSheet()`, `unpause()`.
- `More` → `MoreSheet(selectedDecoder, onSelectDecoder = updateDecoder, remainingTime = sleepTimerRemaining, onStartTimer = startTimer, onEnterFiltersPanel = showPanel(VideoFilters), customButtons)` (PlayerSheets.kt:163-173).
- `PlaybackSpeed` → `PlaybackSpeedSheet(speed, onSpeedChange = mpv speed = toFixed(2), onDismissRequest)` (PlayerSheets.kt:175-181, PC:620).
- `Screenshot` → `ScreenshotSheet(isLocalSource, hasSubTracks, showSubtitles, onToggleShowSubtitles, cachePath, onSetAsArt, onShare, onSave, takeScreenshot, onDismissRequest = onDismissScreenshot)` (PlayerSheets.kt:183-196). `onDismissScreenshot` = `showSheet(None)` then `unpause()` (PC:633-636).

Bodies of these sheets are part 2 of this area.

Sheet opening rules (VM:709-719): `showSheet(s)` for `s != None` hides controls, closes panel and dialog. `showSheet(None)` resets dismiss and shows controls.

---

## 13. Generic widgets (presentation/player/components)

### 13.1 ExpandableCard (ExpandableCard.kt:53-91)

- Card with `modifier.animateContentSize()`, colours/shape/border/elevation parameters.
- Header `Row` clickable: `onExpand(!isExpanded)`, padding start 16 dp, vertically centred. Contains `title(isExpanded)`, `Spacer(weight 1)`, and an `IconButton` with `ArrowDropDown` icon rotated: `animateFloatAsState(if (isExpanded) 0f else 180f)`. So collapsed shows the arrow up-pointing (180 degrees from down) and expanded shows it down.
- Body: `Box(animateContentSize) { if (isExpanded) content() }`.

Proposal: `gtk::Expander` or a `gtk::Revealer` with `slide-down` and a rotated image. Keep the 180/0 degrees.

### 13.2 OutlinedNumericChooser (OutlinedNumericChooser.kt:43-151)

Two overloads: `Int` (value, onChange(Int), max, step, min = 0, suffix, label) and `Float` (same with Float).

Row, spacing 8 dp, vertically centred:
1. `RepeatingIconButton(onClick = onChange(value - step))` with `RemoveCircle` icon.
2. `OutlinedTextField`, `weight(1)`, label slot, suffix slot, number keyboard. `valueString` is local state.
3. `RepeatingIconButton(onClick = onChange(value + step))` with `AddCircle` icon.

Assertion: `assert(max > min)` with message `min can't be larger than max (min > max)` (only with `-ea`; Kotlin `assert` is off by default in production builds, so not enforced at runtime). Proposal: enforce.

Int variant text handling (OutlinedNumericChooser.kt:63-81):
- `LaunchedEffect(value)`: if `valueString` is blank and `value == 0` return; else `valueString = value.toString()`.
- `onValueChange(newValue)`:
  - If blank: `valueString = ""` and `onChange(0)`.
  - `runCatching`: `intValue = if (newValue.trimStart() == "-") 0 else newValue.toInt()`; `onChange(intValue)`; `valueString = newValue`. Failures (e.g., `"abc"`, `"1.5"`) are swallowed silently and leave `valueString` unchanged (the field refuses the character). Note: `"-"` maps to `-0` (zero) and the LaunchedEffect then rewrites the field to `"0"` on the next value change (E-46-019).

Float variant (OutlinedNumericChooser.kt:117-135):
- `LaunchedEffect(value)`: `valueString = value.toString().dropLastWhile('0').dropLastWhile('.')` (e.g., 1.50 → `1.5`, 2.0 → `2`).
- Typing: starting with `.` is rejected (`return@runCatching`), so `.5` cannot be typed; `-` maps to `-0f`; blank → `onChange(0f)`.
- Validity: `isError = value > max || value < min`; supporting text: `"Value too big"` (`numeric_chooser_value_too_big`) when above max; `"Value too small"` (`numeric_chooser_value_too_small`) when below min. Both can show if (impossible) both conditions held; text is only shown, the value is NOT clamped (the caller decides).

Note: the Float variant's `maxLines = 1`.

Proposal: `gtk::SpinButton` with range, digits, and an error CSS class and a label; or keep the `Entry` + two repeat buttons. Clamp input in the caller, not in the widget (same as now).

### 13.3 RepeatingIconButton (RepeatingIconButton.kt:37-77)

Defaults: `maxDelayMillis = 750`, `minDelayMillis = 5`, `delayDecayFactor = 0.25`.

`pointerInteropFilter`: `ACTION_DOWN` sets pressed = true; any other action sets false; always consumes the event. The `IconButton` `onClick` is an empty lambda, so the normal click never fires. Only the repeat loop calls `onClick`.

Loop (LaunchedEffect(pressed, enabled)):
```
currentDelay = maxDelayMillis
while (enabled && pressed) {
    currentClickListener()          // fires immediately on press
    delay(currentDelay)
    currentDelay = (currentDelay - currentDelay * decay).toLong().coerceAtLeast(minDelayMillis)
}
```
Schedule (computed from the formula; first press fires at t=0):
- Fire at t = 0, then waits 750, 562, 421, 315, 236, 177, 132, 99, 74, 55, 41, 30, 22, 16, 12, 9, 6, 5, 5, ...

Each `currentClickListener()` is `rememberUpdatedState(onClick)`, so the latest lambda is used.

Proposal: `GestureClick` press → immediate call → `glib::timeout_add_local` chained with the same delay formula; cancel on release, on `enabled=false`, or on widget destroy. Note: `glib` timeouts are millisecond precision; the 5 ms minimum is not reliable with the default GLib main loop (about 1-4 ms granularity). Proposal: keep 5 ms but expect jitter; or use a 16 ms minimum at 60 Hz. Mark as E-46-021.

### 13.4 SliderItem (SliderItem.kt)

Two overloads (Int with `min`, `max`, `steps = max - min`; Float with explicit `steps`). Row padding h 16 v 8, spacing 24 dp, vertically centred:
- `icon()`, column weight 0.5 with label (`bodyMedium`) and value text, `Slider` weight 1.5.
- Int overload: changes only if `newValue != value`, then `onChange(newValue)` and haptic `TextHandleMove` (dropped on Linux, see T-46a-045). `steps = max - min` means discrete stops.
- Float overload: `steps` default 0 (continuous).

`VerticalSliderItem` (SliderItem.kt:141-183): column, `fillMaxHeight`, padding h 16 v 8, spacing 32 dp, icon, `VerticalSlider(weight 1)`, label/value column.

`VerticalSlider(value: Int, min, max, onValueChange, modifier)` (SliderItem.kt:185-218): material `Slider` rotated by 270 degrees with `graphicsLayer { rotationZ = 270; transformOrigin = (0,0) }` and a custom `layout` swapping constraints; size 180 x 50 dp; emits `it.toInt()` on change (truncation).

Proposal: `gtk::Scale` with `set_orientation(Vertical)`, `set_inverted(true)` so the top is max, `set_round_digits(0)` for Int steps, and `add_mark` for steps.

### 13.5 SwitchPreference (SwitchPreference.kt:33-58)

Row with `toggleable(value, role = Switch)`, padding h 24 v 8, full width, `SpaceBetween`, vertically centred. Children: `content()` and `Switch(checked = value, onCheckedChange = null)`. Click anywhere on row toggles. Proposal: `gtk::ListBoxRow` activatable with `gtk::Switch` suffix; `activate` toggles.

### 13.6 TintedSliderItem and generateSliderColors (TintedSliderItem.kt:140-169)

`TintedSlider` = `Slider` with `colors = generateSliderColors(tint)`. Default tint in `TintedSlider`: `MaterialTheme.colorScheme.primaryContainer` (TintedSliderItem.kt:101). The `TintedSliderItem` requires a `tint` argument.

Colour derivation (OBSERVED):
- `darken(c, f) = Color(max(c.r*f, 0), max(c.g*f, 0), max(c.b*f, 0), c.alpha)`, factor 0.2 for inactive track and 0.4 for inactive tick.
- `lighten(c, f) = Color(min(c.r + (1-c.r)*f, 255), min(...), min(...), c.alpha)`, factor 0.2 for active track and 0.4 for active tick.
- Thumb = base colour. Disabled variants: thumb alpha 0.5, others `.copy(alpha = 0.5f)`.

Defect (OBSERVED): `lighten` clamps to `255f`, but Compose colour components are in the range 0..1, so the clamp never applies. Result is still correct for 0..1 inputs (values never exceed 1 because `c + (1-c)*f <= 1`). Harmless but misleading. Proposal: clamp to 1.0.

Proposal for GTK: compute the four colours in Rust (same formulas, clamp 0..1), emit a per-widget CSS class with `@define-color`-style custom properties on a `gtk::Scale`.

### 13.7 ExposedTextDropDownMenu (ExposedTextDropDownMenu.kt:36-79)

Read-only `OutlinedTextField` with value `selectedValue`, label, trailing `ExposedDropdownMenuDefaults.TrailingIcon(expanded)`, optional leading icon, `fillMaxWidth`. Clicking toggles `expanded`. Menu items: `DropdownMenuItem(text = option)`; click sets `expanded = false` and `onValueChangedEvent(option)`. Dismiss sets `expanded = false`.

Proposal: `gtk::DropDown` with a `gtk::StringList`, `connect_selected_notify` calls `onValueChangedEvent(selected string)`.

### 13.8 OvalBox shapes: see 3.3.

### 13.9 Previews (`@Preview`) are compile-time only. Drop (T-46a-057 covers the ripple config; previews are not user-visible).

---

## 14. User-visible messages (keys and text)

Task T-46a-060 owns the string table. Every string resource used in scope, with the English base text (from `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` unless stated). Kurobayomi must use the same wording, or a deliberately changed wording listed in `p1-ui-parity.md`.

| Key | Text | Used at |
|---|---|---|
| `player_gesture_seek_indicator` | `%c%s\n[%s]` (not translatable) | MiddlePlayerControls.kt:91-96 |
| `player_speed` | `%.2fx` (not translatable) | BottomLeftPlayerControls.kt:67 |
| `seconds` (plural) | `%d second` / `%d seconds` (plurals.xml:51-54) | GH:337 |
| `numeric_chooser_value_too_big` | `Value too big` | OutlinedNumericChooser.kt:84,139 |
| `numeric_chooser_value_too_small` | `Value too small` | OutlinedNumericChooser.kt:85,140 |
| `display_mode_episode` | `Episode %1$s` | EpisodeListDialog.kt:89 |
| `episodes` | `Episodes` | EpisodeListDialog.kt:68 |
| `player_sheets_audio_delay_title` | `Audio delay` | AudioDelayPanel.kt:94 |
| `enable_auto_play` | `Auto-play is on` | VM:802-810 |
| `disable_auto_play` | `Auto-play is off` | VM:802-810 |
| `rotation_video` | `Video` | PlayerEnums.kt:43 (rotation enum, Linux drop) |
| `rotation_sensor_portrait` | `Sensor portrait` | PlayerEnums.kt:46 |
| `rotation_reverse_landscape` | `Reverse landscape` | PlayerEnums.kt:48 |
| `rotation_sensor_landscape` | `Sensor landscape` | PlayerEnums.kt:49 |
| `video_crop_screen` | `Cropped to screen` | PlayerEnums.kt:53 |
| `video_fit_screen` | `Fit to screen` | PlayerEnums.kt:54 |
| `video_stretch_screen` | `Stretched to screen` | PlayerEnums.kt:55 |
| `player_skip_action` | `Skip %1$s` | VM:2104-2114 |
| `player_aniskip_skip` | `%s skipped` | VM:2118-2131, 2148 |
| `player_aniskip_dontskip` | `Don't skip` | VM:2120 |
| `player_aniskip_dontskip_toast` | `Skipping %s in %d seconds` | VM:2079-2086 |
| `player_intro_skipped` | `%1$s skipped` | VM:2097-2099 |
| `player_chapter_type_opening` | `Opening` | ChapterUtils.kt:14 |
| `player_chapter_type_ending` | `Ending` | ChapterUtils.kt:15 |
| `player_chapter_type_recap` | `Recap` | ChapterUtils.kt:16 |
| `player_chapter_type_mixedop` | `Mixed opening` | ChapterUtils.kt:17 |
| `no_prev_episode` | `Previous Episode not found!` | VM:1034-1040 |
| `no_next_episode` | `Next Episode not found!` | VM:1034-1048 |
| `single_action_none` | `None` | PlayerEnums.kt:62 (settings, part 2) |
| `single_action_seek` | `Seek` | PlayerEnums.kt:63 |
| `single_action_playpause` | `Play/Pause` | PlayerEnums.kt:64 |
| `single_action_switch` | `Change episode` | PlayerEnums.kt:65 |
| `single_action_custom` | `Custom` | PlayerEnums.kt:66 |
| `player_sheets_filters_brightness` | `Brightness` | PlayerEnums.kt:147 (panel, part 2) |
| `player_sheets_sub_delay_title` | `Subtitle delay` | SubtitleDelayPanel.kt (DelayCard title, part 2) |
| `player_sheets_sub_delay_delay` | `Delay` | DelayCard label |
| `player_generic_unit_ms` | `ms` | DelayCard suffix |
| `player_sheets_delay_set_as_default` | `Set as default` | DelayCard apply |
| `player_sheets_sub_delay_audio_sound_heard` | `Sound heard` | DelayCard, Audio |
| `player_sheets_sub_delay_sound_sound_spotted` | `Sound spotted` | DelayCard, Audio |
| `player_sheets_sub_delay_subtitle_voice_heard` | `Voice heard` | DelayCard, Subtitle |
| `player_sheets_sub_delay_subtitle_text_seen` | `Text seen` | DelayCard, Subtitle |
| `action_cancel` (MR) | `Cancel` | PlayerDialog.kt:63 |
| `action_ok` (MR) | `OK` | PlayerDialog.kt:67 |
| `relative_time_today`, `relative_time`, `upcoming_relative_time` (MR) | Date relative strings (not read here, DateExtensions.kt:57-70) | EpisodeListDialog date |
| `toast_sleep_timer_ended` | `Sleep timer ended` | VM:359 (part 2 timer) |

Hard-coded English not in resources:
- `"Skip Intro: "` prefix in the netflix-style toast (VM:2080-2085). Proposal: move to a resource.
- `"Skip Intro"` is not otherwise hard-coded.
- The `Icon(..., contentDescription = null)` everywhere (accessibility). Proposal in T-46a-054.

Toasts (`activity.showToast`) used in scope: `no_prev_episode`, `no_next_episode`, Netflix countdown toast. Long-press pill texts use `PlayerUpdates`.

---

## 15. Side effects

| Action | Side effects |
|---|---|
| Tap | `controlsShown` toggles; status bar hide or show (no-op on Linux); auto-hide timer restarts when shown |
| Control button click | `resetControls` flip (auto-hide restart) |
| Play/pause | mpv `pause` property via `VM.pause()` / `unpause()` (VM:644-664); PiP params refresh on pause (VM:652-658) |
| Speed cycle | mpv `speed`; preference `pref_player_speed` write |
| Aspect cycle | mpv `panscan`, `video-aspect-override`; preference `pref_player_aspect_state`; `playerUpdate = AspectRatio` |
| Rotate | Preference `pref_default_player_orientation_type_key`; Activity orientation request (drop on Linux) |
| Lock | VM flag only |
| Auto-play toggle | Preference `pref_auto_play_enabled`; `playerUpdate` pill |
| Horizontal drag end | mpv absolute seek; `updatePlayBackPos` (`onSecondReached`, area 45 side effects) |
| Volume drag | Device volume write (Android) or mpv `volume` (boost) |
| Brightness drag | Window brightness write (Android); VM `currentBrightness` |
| Double tap | mpv relative seek or pause or keypress or episode change depending on preference |
| Long press | Pause; screenshot sheet open |
| Screenshot | `screenshot-to-file` mpv command into cache dir (VM:1830-1845) |
| Bookmark / fillermark | DB write via VM (area 45) |
| Episode list select | Episode change (activity) |
| Integer picker commit | mpv `setPropertyInt(property, value)` (script-defined) |
| Audio delay change | mpv `audio-delay` immediately; preference `pref_audio_delay` only on Apply |
| Sync-by-tapping stop | Delay value committed to the numeric field (no mpv write until the LaunchedEffect triggers) |

Work scheduling: none (no WorkManager). Timers are coroutine delays (see section 16).

---

## 16. Concurrency and timing

Coroutines in scope (all on the Android main/UI dispatcher unless noted):

| Where | Mechanism | Period / delay | Cancel rule |
|---|---|---|---|
| PC:134-144 auto-hide | `LaunchedEffect` + `delay(playerTimeToDisappear)` | 4000 ms default | Restart on key change |
| PC:197-204 volume and brightness auto-hide | `LaunchedEffect` + `delay(2000)` | 2000 ms | Restart on key change |
| PC:293-299 player update auto-clear | `LaunchedEffect` + `delay(2000)` | 2000 ms | Restart on update change |
| GH:96-103 double-tap clear | `LaunchedEffect(seekAmount)` + `delay(800)` then `delay(100)` | 800 + 100 ms | Restart on each seek step |
| GH:121-180 long press and tap | `pointerInput(Unit)` + `detectTapGestures` | system long-press timeout | Per gesture |
| GH:181-217 horizontal drag | `pointerInput(areControlsLocked)` | per event | Per gesture |
| GH:218-301 vertical drag | `pointerInput(areControlsLocked)` | per event | Per gesture |
| DoubleTapSeekTriangles.kt:28-36 | `LaunchedEffect` infinite loop of `animateTo` | 150 ms steps (900 ms cycle) | Leaving composition |
| RepeatingIconButton.kt:66-76 | `LaunchedEffect(pressed, enabled)` loop | 750 → 5 ms decay | Release or disable |
| DelayCard sync loop (SubtitleDelayPanel.kt:249-262) | `LaunchedEffect(isDirectionPositive)` loop | 20 ms | Stop button |
| VM `startTimer` (sleep) | `viewModelScope.launch` + `delay(1000)` per second | 1 s | Cancel job |
| VM thumbnail fetch | `viewModelScope.launchIO` | 2 s throttle | Cancel previous job |
| VM saveImage / shareImage / setAsArt | `launchNonCancellable` / `launchIO` | once | Not cancelled |
| ViewModel flows | `MutableStateFlow` collected with `collectAsState` | per emission | |

Dispatchers: UI composition on Main; mpv commands are synchronous calls from the UI thread (for example `MPVLib.command` in PC:552, GH:165). Blocking risk: `MPVLib.setPropertyDouble` on every audio-delay change (AudioDelayPanel.kt:66) runs on Main. Proposal: send mpv commands through an async channel to the mpv thread.

Threading in Rust: all GTK widgets on the main thread; mpv event loop on its own thread forwards property changes via `glib::MainContext::channel`; thumbnail fetch and decode on tokio blocking pool, result delivered via a channel; timers via `glib::timeout_add_local`.

---

## 17. Interactions with other areas

- area:45-player-core: VM state, mpv bridge, episode list data, custom buttons execution, skip-intro chapter logic, PiP params, screenshot file IO, thumbnail info and tile fetch, playlist order.
- area:02-preferences-system: all `PreferenceStore` keys listed in `p1-data-and-config.md`; defaults must match the table there.
- area:36-ui-shell-navigation-security-webview: window and navigation shell, theme colour roles (`surface`, `surfaceDim`, `tertiary`, `errorContainer`, `inversePrimary`).
- area:39-ui-entry-details-anime: episode number formatting and date display conventions.
- area:07-domain-entries-items: `Episode` fields (`seen`, `bookmark`, `fillermark`, `dateUpload`, `episodeNumber`, `scanlator`), `Anime.EPISODE_DISPLAY_NUMBER`.
- area:16-source-api-anime: `ChapterType`, `TimeStamp`, `AnimeHttpSource.getImageTile`.
- area:04-db-anime-schema-migrations: bookmark and fillermark storage.
- area:46-player-controls-ui part 2: sheet and panel bodies, DelayCard owner, subtitle settings, video filters, screenshot sheet, custom button editor.
