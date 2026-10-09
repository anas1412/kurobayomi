# 46 Player controls UI, part 1: implementation tasks

Ids are `T-46a-NNN`. Every in-scope file and every behaviour in `p1-behavior.md` is covered by at least one task (see `p1-inventory.md`, column "Task ids"). Priorities follow the project rule: P0 is needed for daily reading and watching, P1 is parity most users rely on, P2 is long tail and polish, P3 is an Android-only drop with a stated replacement or reason.

Dependencies on other areas use `area:<id>`. Dependencies inside this area use task ids.

### T-46a-001 — Overlay visibility state machine and auto-hide
- Priority: P0
- Effort: M
- Depends: T-46a-039, T-46a-042, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `ControlsModel` in src/player/model.rs reproduces show_controls, hide_controls, tap, control_clicked, auto_hide_wanted and the host rules of PlayerViewModel.kt:666-741.
  - Auto-hide fires 4000 ms after the last trigger when controls are shown, playback is running and no seek drag is active; pausing cancels it; resume restarts with the full delay (E-46-001, E-46-002, E-46-003).
  - Every control click restarts the countdown through reset_epoch (PlayerControls.kt:158).
  - show_controls returns false and changes nothing while a sheet, panel or dialog is open.
  - mpv pause, speed and seek commands are sent through MpvCmd and never block the GTK thread.

### T-46a-002 — Lock and unlock controls
- Priority: P0
- Effort: S
- Depends: T-46a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomLeftPlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt
- Acceptance:
  - The lock button in the bottom-left row sets locked; while locked every chrome element and the seekbar are hidden (behaviour 1.2 rules).
  - The unlock button (lock icon, top-left) is shown only when locked and controls are shown; tapping the surface toggles it (E-46-005).
  - While locked, double tap, long press and both drags are ignored (GestureHandler.kt:128, 171, 182, 219).
  - Lock state is not persisted; a new player session starts unlocked.

### T-46a-003 — Top-left back button and title block
- Priority: P0
- Effort: S
- Depends: T-46a-058, T-46a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/TopLeftPlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt
- Acceptance:
  - Back button (go-previous) leaves the player.
  - Anime title is 16 pt, episode title is 14 pt italic at 50% white, both single line with ellipsis, with the -4 px overlap of TopLeftPlayerControls.kt:57.
  - Clicking the title block opens the episode list dialog (VM showEpisodeListDialog, requires an anime loaded).
  - Row is fill-to-constraint between the left edge and the top-right row.

### T-46a-004 — Top-right action row with short and long press routing
- Priority: P0
- Effort: M
- Depends: T-46a-005, T-46a-058, T-46a-027
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/TopRightPlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt
- Acceptance:
  - Order and actions: auto-play switch, subtitles (tap: subtitle tracks sheet, long press: subtitle settings panel), audio (tap: audio sheet, long press: audio delay panel), quality (only when the episode is online; tap and long press both open the quality sheet), more (tap: more sheet, long press: video filters panel).
  - Quality button is absent for offline episodes and for unknown online state (TopRightPlayerControls.kt:86).
  - Each button click also resets the auto-hide countdown.

### T-46a-005 — Auto-play switch widget
- Priority: P1
- Effort: S
- Depends: T-46a-058
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/AutoPlaySwitch.kt
- Acceptance:
  - Track 5/6 of width and 2/3 of height with a fully rounded end; PlayCircle thumb on the primary colour when on, PauseCircle in white when off.
  - Thumb slides to the on or off end in 250 ms.
  - Toggling calls setAutoPlay: writes pref_auto_play_enabled and shows "Auto-play is on" or "Auto-play is off" for the pill timeout (VM:802-810).

### T-46a-006 — Centre transport controls
- Priority: P0
- Effort: M
- Depends: T-46a-001, T-46a-053, T-46a-041
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/MiddlePlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt
- Acceptance:
  - Previous and next are 48 px icon buttons spaced 24 px from a 96 px play/pause target; previous is disabled when there is no previous episode (alpha 0.38), next likewise.
  - While loading with pref_show_loading on, a 96 px spinner replaces play/pause; with it off, play/pause stays.
  - Play/pause calls pauseUnpause; previous and next call changeEpisode(true/false).
  - Previous and next are hidden while a gesture seek is active.

### T-46a-007 — Gesture seek indicator text
- Priority: P0
- Effort: S
- Depends: T-46a-040, T-46a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/MiddlePlayerControls.kt
- Acceptance:
  - Text is formatted from player_gesture_seek_indicator as "%c%s\n[%s]" with sign '+' for delta >= 0 else '-', the absolute delta, and the target time (start + delta), using pretty_time.
  - Example: start 85 s, delta +12 renders "+00:12" over "[01:37]".
  - Bold headline style with a 5 px black shadow, centred.

### T-46a-008 — Bottom-left row: lock, rotate, speed cycle, chapter pill gate
- Priority: P0
- Effort: M
- Depends: T-46a-052, T-46a-042, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomLeftPlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Speed label formats as "%.2fx"; a tap sets the next speed by next_speed (+0.25, wrap to 0.25 when >= 2.0) and writes pref_player_speed (E-46-023 rule).
  - Long press opens the playback speed sheet.
  - Rotate button is not shown on Linux (T-46a-048).
  - Chapter pill shows only when a current chapter exists and pref_show_current_chapter is true; tap opens the chapters sheet.

### T-46a-009 — Bottom-right row: primary button slot, PiP gate, aspect cycle
- Priority: P0
- Effort: M
- Depends: T-46a-050, T-46a-051, T-46a-046
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomRightPlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt
- Acceptance:
  - Primary slot shows the skip-intro text when present, otherwise the custom primary button when one exists, otherwise nothing.
  - Aspect button cycles Fit, Stretch, Crop, Fit (next_aspect), sets mpv panscan and video-aspect-override per PlayerViewModel.kt:813-837, persists pref_player_aspect_state, and shows the matching pill text.
  - PiP button is shown only when the mini-player feature is available (T-46a-046); it is never shown on a platform without it.

### T-46a-010 — Seekbar widget with chapter ticks and read-ahead
- Priority: P0
- Effort: L
- Depends: T-46a-040, T-46a-039, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/SeekBar.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt
- Acceptance:
  - SeekBar GObject draws track, progress, read-ahead fill, chapter ticks and thumb; emits value-changed while dragging and seek-committed once on release.
  - Displayed value precedence: gesture seek, then drag value, then playhead (E-46-036).
  - seek_segments emits each chapter once, prepending a synthetic 0 segment only when the first chapter does not start at 0 (E-46-031 fixed), and drops chapters outside 0..duration (E-46-032 included at duration).
  - Commit issues an absolute seek with exact precision when pref_player_smooth_seek is set, else keyframe precision.
  - Duration 0 renders as unknown: no ticks, thumb at 0, drags ignored (E-46-015).

### T-46a-011 — Seekbar time labels and duration toggle
- Priority: P0
- Effort: S
- Depends: T-46a-010, T-46a-040, T-46a-042
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/SeekBar.kt
- Acceptance:
  - Position label is 92 px wide and shows pretty_time(position).
  - Duration label shows total duration, or when inverted pretty_time_signed(position - duration).
  - Clicking the duration label toggles and persists invert_duration (default false).
  - Position label has no click action (no ripple).

### T-46a-012 — Seekbar auto-show mode
- Priority: P1
- Effort: S
- Depends: T-46a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt
- Acceptance:
  - Seekbar is visible when (controls shown or seekbar shown) and not locked (PlayerControls.kt:370).
  - Initial seekbar-shown state is true when pref player_hide_controls is false.
  - When pref pref_show_seekbar is true, gesture seeks show the seekbar; it hides 100 ms after a double-tap clear and at horizontal drag end.
  - showSeekBar is a no-op while a sheet is open (VM:688-691).

### T-46a-013 — Thumbnail preview popup
- Priority: P1
- Effort: M
- Depends: T-46a-010, area:16-source-api-anime, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/ThumbnailPreview.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Card size is 220x124 dp, or 124*aspect by 124 for images narrower than 16:9 (ThumbnailPreview.kt:55-70), radius 8, elevation 8.
  - Card x is centred on the target and clamped 8 px from each edge (behaviour 7.3), via thumb_x.
  - Chapter name shown bottom-left in the 60% black pill only when non-empty (E-46-034 uses reverse list order).
  - Tile cache holds 3 tiles, evicts least recently used (E-46-042); fetch throttle 2 s (E-46-041 retry on drag end).
  - Fetch uses AnimeHttpSource getImageTile on the IO runtime; errors are logged and produce no card (E-46-043).

### T-46a-014 — Single tap gesture
- Priority: P0
- Effort: S
- Depends: T-46a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt
- Acceptance:
  - A single tap on the video toggles controls (tap() in ControlsModel).
  - While a panel is open and pref_allow_gestures_in_panels is false, pressing the video closes the panel first (E-46-006).
  - Taps while locked still toggle controls (E-46-005).

### T-46a-015 — Double-tap zones and per-zone actions
- Priority: P0
- Effort: M
- Depends: T-46a-039, T-46a-042, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Zones: right when x > 0.6 width, left when x < 0.4 width, centre otherwise (E-46-012 boundaries).
  - Action per zone from pref_left_double_tap (default Seek), pref_center_double_tap (default PlayPause), pref_right_double_tap (default Seek).
  - Seek moves by pref_skip_length_preference seconds (default 10) with relative keyframe or exact seek per pref_player_smooth_seek.
  - PlayPause toggles pause; Custom sends keypress 0x10001, 0x10002 or 0x10003; Switch changes episode (left previous, right next); None does nothing.
  - Ignored while locked (GestureHandler.kt:128).

### T-46a-016 — Double-tap accumulator and indicator timing
- Priority: P0
- Effort: M
- Depends: T-46a-015, T-46a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Same-direction taps add the step to the accumulator; reversing resets to 0 first (E-46-046); repeat presses in the same zone during an active seek keep accumulating (E-46-047).
  - Indicator stays 800 ms after the last change, then clears text and hides the seekbar after a further 100 ms; each change restarts the timers (E-46-045).
  - Text is "N seconds" with the plural rule on the signed amount: "-1 second" and "-20 seconds" for negative (E-46-016), no plus sign for forward.
  - When the accumulator is 0 and no text is set, nothing is drawn; a text with amount 0 draws on the left (GH:315).

### T-46a-017 — Double-tap oval shapes and triangle animation
- Priority: P1
- Effort: S
- Depends: T-46a-041
- Sources: app/src/main/java/eu/kanade/presentation/player/components/OvalBox.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/DoubleTapSeekTriangles.kt
- Acceptance:
  - Right oval path: moveTo(w,h), lineTo(w,0), lineTo(w/10,0), cubic to (w/10,h) with control points (w/10,0) and (-30,h/2) (OvalBox.kt:41-53); left oval is its mirror (OvalBox.kt:65-77); both are 40% of the surface width.
  - Fill is white at alpha 0.2 with alpha fading in and out.
  - Three triangles fade in and out in sequence, 150 ms per step, a 900 ms loop, rotated 180 degrees for backward seeks (DoubleTapSeekTriangles.kt:22-47).

### T-46a-018 — Horizontal drag seek
- Priority: P0
- Effort: M
- Depends: T-46a-039, T-46a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Enabled when pref_gesture_horizontal_seek is true (default true) and controls are not locked.
  - Target = start + truncate((x - start_x) * 0.15), clamped to 0..duration; gesture text and seekbar follow the target.
  - Guards: no drag left at position 0, no drag right at the end (GH:202-203).
  - Release seeks to the target (absolute, precision per pref), clears the gesture state, hides the seekbar; paused state is preserved (E-46-044, Kotlin's unpause is a no-op).
  - Start position and release position truncate toward zero (E-46-020).

### T-46a-019 — Vertical drag volume and brightness
- Priority: P1
- Effort: L
- Depends: T-46a-039, T-46a-021, T-46a-022, T-46a-049
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt
- Acceptance:
  - Enabled when pref_gesture_volume_brightness is true (default true) and controls are not locked.
  - Left half (x < w/2) controls brightness, right half controls volume; pref_swap_volume_and_brightness (default false) swaps them.
  - Brightness step sensitivity 0.001 per px, clamped to -0.75..1 in state and 0..1 for the window; volume step sensitivity 0.001 * max_volume per px (Int truncation).
  - When device volume is at max and the drag is upward, mpv boost takes over up to pref_audio_volume_boost_cap (default 30), mpv volume clamped to 100..cap+100; dragging down reverses (behaviour 2.7).
  - Each event shows the matching pill for 2 s after the last event.

### T-46a-020 — Long press pauses and opens the screenshot sheet
- Priority: P1
- Effort: S
- Depends: T-46a-027, T-46a-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerSheets.kt
- Acceptance:
  - Long press (platform timeout, default 500 ms GTK) while not locked pauses playback, sets the Screenshot sheet and gives haptic-free feedback.
  - On release, clears the update pill; does not restore any speed (E-46-011).
  - Dismissing the screenshot sheet unpauses only if the player was playing before the long press (E-46-049 fix).

### T-46a-021 — Volume pill with device and boost display
- Priority: P1
- Effort: M
- Depends: T-46a-037, T-46a-049
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/VerticalSliders.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt
- Acceptance:
  - Pill sits at the start edge (end edge when swapped), vertically centred, 16 px margin; bar 120 px tall with 54 px wide box and a rounded 16 px track.
  - Device text is a percent when pref_display_vol_as_per is true (default), otherwise the integer step; boost text per volume_texts (0 empty, "+N", "-N", else plain) (E-46-026, E-46-052).
  - Icon per volume_icon: Off at 0, Mute 1..30, Down 31..60, Up 61..100 (E-46-053).
  - When max_steps is 0 the pill shows 0 and never divides by zero (E-46-014).

### T-46a-022 — Brightness pill and software veil
- Priority: P1
- Effort: M
- Depends: T-46a-037, T-46a-049
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/VerticalSliders.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/BrightnessOverlay.kt
- Acceptance:
  - Pill text is the integer percent (brightness * 100), no % sign; bar fill 0..1, error-coloured overflow for negative values up to 0.75.
  - Icon per brightness_icon (Night at 0, Low up to 0.3, Medium up to 0.6, High above).
  - Veil is a full-window black layer with alpha = -brightness when brightness is below 0; never receives pointer input (BrightnessOverlay.kt:18-31).
  - Initial brightness reads the backlight; if unavailable, starts at 0 (E-46-059).

### T-46a-023 — Player update pill
- Priority: P1
- Effort: S
- Depends: T-46a-041
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/PlayerUpdates.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt
- Acceptance:
  - Pill is radius 16 px, black at 40%, padding 8 px vertical and 16 px horizontal, positioned at 20% from the top, centred.
  - Clears automatically after 2000 ms; each new update or aspect change restarts the timer; DoubleSpeed and None never start it (PC:293-299).
  - Shows the aspect name, the given text, or the resource text.

### T-46a-024 — Episode list dialog
- Priority: P1
- Effort: L
- Depends: T-46a-025, T-46a-040, area:04-db-anime-schema-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/dialogs/EpisodeListDialog.kt
- Acceptance:
  - Dialog is 80% by 80% of the window, title "Episodes", list shows episodes newest first (reversed playlist) and scrolls so the current episode is at the top.
  - Row title is "Episode N" (formatEpisodeNumber) when the anime display mask bit is number, else the name; mask compare uses (displayMode & 0x00100000) == 0x00100000 (E-46-033).
  - Bookmark toggles primary tint; fillermark toggles tertiary tint; bookmark tint wins over fillermark; watched episodes are dimmed to 38%; current episode is bold italic.
  - Subtitle row: relative date when pref relative time, else short date; dot separator only when date and scanlator are both present (E-46-061).
  - No crash when the current episode is not in the list (E-46-017); clicking a row changes episode and closes the dialog.
  - Bookmark and fillermark changes are persisted; a failed write reverts the row (E-46-035).

### T-46a-025 — Dialog shell
- Priority: P1
- Effort: S
- Depends: T-46a-058
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/dialogs/PlayerDialog.kt
- Acceptance:
  - Modal window, 16 px padding, title in the large title style, optional Cancel (left) and OK (right) row.
  - Escape and outside click dismiss; OK runs the confirm callback then dismisses.
  - Dialog is not decorated, does not use the platform default width.

### T-46a-026 — Integer picker dialog
- Priority: P2
- Effort: M
- Depends: T-46a-025, T-46a-060
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/dialogs/IntegerPickerDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerDialogs.kt
- Acceptance:
  - Opened by the mpv script message launch_int_picker with "title|format|start|stop|step|property"; values are start..stop by step with the last value excluded when not divisible (E-46-028 table).
  - step <= 0 or a malformed message is rejected and logged (E-46-097).
  - Starts on the default value's index or index 0 when absent; the committed value is the displayed item on any dismissal (E-46-027, D-11).
  - Commit writes the property via the mpv command queue.

### T-46a-027 — Sheet and dialog host routing
- Priority: P0
- Effort: M
- Depends: T-46a-029, T-46a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerSheets.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerDialogs.kt
- Acceptance:
  - Each Sheets value opens its sheet; sheet ids map to the spec ids used by the mockup (playback_speed, chapters, more, screenshot, subtitle_tracks, audio_tracks, quality_tracks).
  - Subtitle and audio sheets offer a file picker with no filter (any file) and ignore a cancelled pick.
  - Chapter sheet does nothing and closes state when no chapter exists (E-46-065); chapter selection seeks to the chapter start, dismisses the sheet and unpauses (PC:612-616).
  - Screenshot dismissal closes the sheet and restores the pre-long-press pause state (E-46-049).
  - Dialog routing handles EpisodeList and IntegerPicker; None closes.

### T-46a-028 — Panel host
- Priority: P1
- Effort: M
- Depends: T-46a-041
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerPanels.kt
- Acceptance:
  - Panels are shown right-aligned in a card of at most 420 px width, surface colour at alpha panel_opacity/100 (default 60%).
  - Switching panels slides and fades over 300 ms; None shows nothing.
  - Opening a panel hides controls and closes any sheet or dialog (VM:721-730).

### T-46a-029 — Sheet container
- Priority: P0
- Effort: L
- Depends: T-46a-041
- Sources: app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt
- Acceptance:
  - Sheet is 420 px wide (720 px in landscape), bottom aligned, height up to 95% of the window, top corners rounded, bottom corners square, tonal surface.
  - Scrim is black at 50%, animates in over 350 ms and out on dismiss.
  - Opens with a 350 ms slide; dismiss by scrim tap, Escape, back, or drag down past 56 px or a 125 px/s fling.
  - Dismiss happens once even when the drag and the dismiss event both fire (E-46-064).

### T-46a-030 — Audio delay panel
- Priority: P1
- Effort: M
- Depends: T-46a-031, T-46a-042
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/AudioDelayPanel.kt
- Acceptance:
  - Delay starts from mpv audio-delay (seconds times 1000, truncated) and every change writes mpv audio-delay as delay/1000.
  - Apply writes pref_audio_delay; Reset sets 0 without persisting; closing without Apply keeps the session value in mpv.
  - Card sits at the bottom-right (bias 0.8 vertically).
  - Title "Audio delay" with a close button.

### T-46a-031 — Shared delay card sync algorithm
- Priority: P2
- Effort: M
- Depends: T-46a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/AudioDelayPanel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleDelayPanel.kt
- Acceptance:
  - Audio: "Sound heard" starts the timer with positive direction, "Sound spotted" with negative; the delay changes by plus or minus the elapsed time every 20 ms; the second button stops it and commits the value.
  - Subtitle: "Voice heard" and "Text seen" map to the opposite directions per DelayType (behaviour 11.3).
  - The active button is disabled; Apply and Reset are disabled while running (E-46-030).
  - Shows the live value while running (D-parity note E-46-029 resolved by displaying the running value).

### T-46a-032 — Outlined numeric chooser
- Priority: P1
- Effort: M
- Depends: T-46a-033
- Sources: app/src/main/java/eu/kanade/presentation/player/components/OutlinedNumericChooser.kt
- Acceptance:
  - Minus and plus repeat buttons change the value by step; the entry accepts integer or decimal text per the Int and Float variants.
  - Int: "-" yields 0 and keeps the "-" text while focused (E-46-019); invalid text is ignored (E-46-096).
  - Float: a leading "." is not accepted; "-" yields -0.
  - Error style and labels "Value too big" and "Value too small" when outside min..max; the value is not clamped by the widget.
  - Subtraction saturates at the integer limit (E-46-024).
  - min greater than max is rejected at construction.

### T-46a-033 — Repeating icon button schedule
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/player/components/RepeatingIconButton.kt
- Acceptance:
  - Fires once on press, then repeats with waits 750, 562, 421, 315, 236, 177, 132, 99, 74, 55, 41, 30, 22, 16, 12, 9, 6, 5, 5 ms (RepeatSchedule test, decay 0.25, minimum 5).
  - Release, disable or widget destroy cancels the repeat.
  - Timing jitter is accepted (E-46-021, D-12).

### T-46a-034 — Expandable card
- Priority: P2
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/player/components/ExpandableCard.kt
- Acceptance:
  - Header row toggles expansion on click; the chevron rotates 180 degrees when collapsed and 0 when expanded.
  - Body content is shown only while expanded; size changes animate (200 ms).

### T-46a-035 — Switch row and slider rows
- Priority: P2
- Effort: S
- Depends: T-46a-037
- Sources: app/src/main/java/eu/kanade/presentation/player/components/SwitchPreference.kt, app/src/main/java/eu/kanade/presentation/player/components/SliderItem.kt
- Acceptance:
  - SwitchRow activates on row click and shows a switch reflecting the value; no haptics (T-46a-045).
  - SliderRow shows label, value text and a slider; Int variant uses discrete steps (max minus min) and emits only on change; Float variant is continuous unless steps are given.

### T-46a-036 — Tinted slider colour derivation
- Priority: P2
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/player/components/TintedSliderItem.kt
- Acceptance:
  - Colours derived exactly: darken by 0.2 for inactive track and 0.4 for inactive tick; lighten by 0.2 for active track and 0.4 for active tick; thumb is the tint; disabled variants at alpha 0.5.
  - Lighten is clamped to 1.0 (the Kotlin clamp to 255 is a no-op, documented).
  - Unit test covers one red, one mid grey and one white tint.

### T-46a-037 — Vertical gauge widget and percentage math
- Priority: P1
- Effort: S
- Depends: T-46a-039
- Sources: app/src/main/java/eu/kanade/presentation/player/components/SliderItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/VerticalSliders.kt
- Acceptance:
  - percentage(value, range) clamps to 0..1 and returns 0 for an empty range (E-46-014).
  - Gauge animates the fill height and the overflow height; overflow uses error colour above the main fill.
  - Vertical slider control rotates a horizontal control by 270 degrees with 180 by 50 px dimensions (SliderItem.kt:212-213).

### T-46a-038 — Exposed dropdown widget
- Priority: P2
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/player/components/ExposedTextDropDownMenu.kt
- Acceptance:
  - Read-only entry showing the selected value with a dropdown arrow; choosing an option calls the change callback and closes the list.
  - Implemented with gtk::DropDown and a string list.

### T-46a-039 — Gesture math helpers
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/VerticalSliders.kt
- Acceptance:
  - vertical_step (Int and f32) and horizontal_step (Int) reproduce calculateNewVerticalGestureValue and calculateNewHorizontalGestureValue including Int truncation toward zero.
  - zone() implements 0.4 and 0.6 boundaries exactly; drag_target implements the swapped rule.
  - Sensitivities are the constants 0.15 (seek), 0.001 (brightness), 0.001 times max steps (volume), 0.001 times boost cap (boost).
  - All functions are pure and covered by unit tests (T-46a-056).

### T-46a-040 — Time formatting helpers
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/SeekBar.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/MiddlePlayerControls.kt, app/src/main/java/eu/kanade/presentation/util/ItemNumberFormatter.kt
- Acceptance:
  - pretty_time gives "mm:ss" under one hour and "h:mm:ss" otherwise; pretty_time_signed prefixes "+" or "-" (upstream Utils.prettyTime rules).
  - format_episode_number follows DecimalFormat "#.###" output except the leading zero (D-10): 1.0 gives "1", 12.5 gives "12.5", 1.2345 gives "1.234".
  - format_seconds_plural returns "1 second" and "-1 second", "10 seconds" and "-20 seconds".

### T-46a-041 — Animation timings and reduce motion
- Priority: P1
- Effort: S
- Depends: T-46a-042
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt
- Acceptance:
  - Enter animations 100 ms with ease-out; exit animations 300 ms with fast-out-slow-in; constants are named and tested.
  - pref_reduce_motion (default false) turns every slide into a fade; the GTK setting gtk-enable-animations also disables slides (D-5).
  - A tick-based animator is used for opacity and margin changes, so the code does not depend on CSS transitions.

### T-46a-042 — Preference binding and defaults
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt
- Acceptance:
  - settings.rs PlayerSettings holds every key in p1-data-and-config.md section 3 with the listed default; a unit test compares them.
  - Enum preferences are stored by name; an unknown name falls back to the default per key.
  - Speed, aspect, auto-play and invert-duration write-through on change (D-13).

### T-46a-043 — Keyboard shortcut map
- Priority: P1
- Effort: M
- Depends: T-46a-042
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt
- Acceptance:
  - Shortcut table from p1-ui-parity.md section 10.2 is implemented through a window shortcut controller and is configurable through the player settings file.
  - Space toggles play/pause; Left and Right seek by the skip length; Up and Down change volume by one step; L toggles lock; Escape closes the top sheet, panel or dialog first.
  - Shortcuts are ignored while a text entry has focus.

### T-46a-044 — Pointer equivalents and wheel
- Priority: P1
- Effort: M
- Depends: T-46a-014, T-46a-018, T-46a-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt
- Acceptance:
  - Double-click on the left, centre and right zones act as double tap (GTK double-click time, no hard-coded constant, E-46-072).
  - Press and hold 500 ms opens the screenshot sheet using a timer, not only GestureLongPress (P-3, E-46-073).
  - Mouse wheel over the video changes volume by one step (up is louder) and is ignored while locked (E-46-075); horizontal wheel seeks by the skip length.
  - Right-click on the video opens the More sheet.

### T-46a-045 — Haptic feedback dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt, app/src/main/java/eu/kanade/presentation/player/components/SliderItem.kt, app/src/main/java/eu/kanade/presentation/player/components/TintedSliderItem.kt
- Acceptance:
  - No haptic call exists in the Rust code; the drop is recorded here with reason: desktops have no vibration motor and the feedback has no Linux equivalent.
  - The user loses only the tactile tick; visual feedback (pill, thumb movement) is kept.

### T-46a-046 — PiP button and mini-player window
- Priority: P2
- Effort: M
- Depends: area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomRightPlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - PiP button is shown only when pref_enable_pip is true; it opens a compact always-on-top-where-supported player window that keeps playback running.
  - The button is disabled while a new episode is loading (PC:508).
  - Replacement is documented: Wayland does not guarantee always-on-top; a KWin window rule can keep it above.

### T-46a-047 — Android PiP params and remote actions dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - createPipParams, auto-enter on pause, source rect hints, and the PiP remote action buttons (replace-with-previous, playlist position) are not implemented.
  - Reason recorded: they are Android PictureInPictureParams and have no GTK equivalent; the user loses system PiP auto-enter on home press.

### T-46a-048 — Screen rotation button dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomLeftPlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - The rotate button is absent and cycleScreenRotations is not ported.
  - pref_default_player_orientation_type_key is read and kept in settings but has no effect; reason: desktop windows do not have a device orientation.

### T-46a-049 — System volume and brightness backends
- Priority: P1
- Effort: M
- Depends: area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - SystemVolume trait backed by the default PipeWire sink (wpctl) with 15 steps; read failure leaves the pill at its last value and logs once (E-46-091).
  - SystemBrightness trait backed by the writable sysfs backlight, falling back to logind; if neither works only the software veil is used (E-46-090).
  - mpv volume-max is set to pref boost cap plus 100 before first playback and on every cap change (E-46-025).
  - Blocking commands run on a blocking task, never on the GTK thread.

### T-46a-050 — Skip intro button rendering
- Priority: P1
- Effort: S
- Depends: area:45-player-core, T-46a-009
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomRightPlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Button text is "Skip <type>" for Opening, Ending, Recap, Mixed opening; "Don't skip" during the Netflix-style countdown; hidden for other chapter types.
  - Pressing it seeks to the next chapter start with the "<name> skipped" text; pressing during a countdown cancels it.
  - The countdown toast prefix is a resource string, not a literal (E-46-067).

### T-46a-051 — Custom primary button rendering and execution
- Priority: P2
- Effort: S
- Depends: area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomRightPlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Button is shown in the primary slot when no skip text exists and a primary custom button exists; its label is the button name.
  - Click sends script-message call_button_<id>; long press sends call_button_<id>_long.

### T-46a-052 — Current chapter pill
- Priority: P2
- Effort: S
- Depends: T-46a-058
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/CurrentChapter.kt
- Acceptance:
  - Pill shows a bookmark icon, the chapter start as mm:ss in bold tertiary, a bullet and the chapter name in bold, on a 60% background with fully rounded ends.
  - When the chapter changes forward the new text slides up, backward it slides down.
  - Name truncates with ellipsis; the pill is clickable.

### T-46a-053 — Play/pause icon transition
- Priority: P2
- Effort: S
- Depends: T-46a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/MiddlePlayerControls.kt
- Acceptance:
  - Play and pause icons cross-fade over 150 ms when playback state changes (D-7); no vector morph is required.

### T-46a-054 — Accessibility labels and focus
- Priority: P2
- Effort: S
- Depends: T-46a-058
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/MiddlePlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/ControlsButton.kt
- Acceptance:
  - Every icon-only control has an accessible label from p1-ui-parity.md section 10.3.
  - Each control is reachable by keyboard focus in the order top, centre, bottom.
  - Seekbar exposes the slider role with a label "Seek".

### T-46a-055 — Responsive layout for small windows
- Priority: P1
- Effort: M
- Depends: T-46a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomLeftPlayerControls.kt
- Acceptance:
  - At 640 px wide and 360 px tall every row fits without overlap; the chapter pill hides below 560 px.
  - Title block shrinks with ellipsis before any button is hidden.
  - Sheets are 720 px wide when the window is wider than 720 px, else 420 px.

### T-46a-056 — Test suite for scope logic
- Priority: P0
- Effort: M
- Depends: T-46a-039, T-46a-040, T-46a-042
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/VerticalSliders.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/SeekBar.kt, app/src/main/java/eu/kanade/presentation/player/components/RepeatingIconButton.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/ThumbnailPreview.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/dialogs/EpisodeListDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/dialogs/IntegerPickerDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/AudioDelayPanel.kt
- Acceptance:
  - All 48 numbered unit tests in p1-edge-cases-and-tests.md section 11.1 exist and pass under cargo test.
  - Fixtures from section 11.3 live under tests/fixtures/player/.
  - A manual checklist file records the device checks from p1-ui-parity.md section 19.

### T-46a-057 — Ripple configuration dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/ControlsButton.kt
- Acceptance:
  - The Compose ripple configuration (alpha 0.1 for all states, white or black by theme) is not ported; GTK hover and pressed CSS states replace it.
  - Reason recorded: GTK has no per-composition ripple configuration; the user loses the exact ripple tint, keeps a visible pressed state.

### T-46a-058 — Control button primitives
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/ControlsButton.kt
- Acceptance:
  - Icon button is circular, 20 px icon by default, padding 16 px vertical and horizontal by default; disabled state uses 38% alpha and ignores clicks.
  - Long press is supported on every icon and text button.
  - A filled pill button (Filled) has the suggested-action look and a long-press target.
  - Each click notifies the controls model (reset the auto-hide countdown).

### T-46a-059 — Sheet dismissal: scrim, back and Escape
- Priority: P0
- Effort: S
- Depends: T-46a-029
- Sources: app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt
- Acceptance:
  - Escape and the platform back gesture close the topmost open sheet, then panel, then dialog; the controls are shown after the last one closes (E-46-076).
  - A click on the scrim closes the sheet only when it is fully open (not while animating).

### T-46a-060 — Localised string table
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/ChapterUtils.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/dialogs/EpisodeListDialog.kt
- Acceptance:
  - Every key in p1-behavior.md section 14 exists in one table with the English text, and plural keys use the plural form.
  - The Netflix countdown prefix is a key (E-46-067).
  - A unit test asserts each key used in code is in the table.
