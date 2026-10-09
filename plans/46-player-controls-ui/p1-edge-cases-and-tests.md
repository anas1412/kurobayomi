# 46 Player controls UI, part 1: edge cases, failure modes, tests

Upstream tests: none. A search of `app/src/test` and of all `*Test.kt` / `*Spec.kt` files in the clone found no test file for the in-scope player controls (`find` over the clone returned nothing for `player`, `seek`, `gesture`, `ChapterUtils`, `volume`). Every test below is new and must be written for Kurobayomi (T-46a-056).

Edge-case IDs are `E-46-NNN`. Those referenced from `p1-behavior.md`, `p1-data-and-config.md`, `p1-ui-parity.md` and `p1-rust-design.md` keep their numbers: 011, 016, 017, 019, 021, 022, 024, 027, 029, 031, 033, 034, 035, plus 022/033/043 cross-references.

Columns: ID | Where (path:line) | Condition | Observed Kotlin result | Required Kurobayomi result | Test (T-46a-056 unit or manual).

---

## 1. Visibility and auto-hide

| ID | Where | Condition | Observed | Required | Test |
|---|---|---|---|---|---|
| E-46-001 | PC:134-144 | Playback starts with controls initially shown | Auto-hide armed immediately (controls shown and not paused) | Same; countdown 4000 ms from start | unit `auto_hide_wanted` true when shown and playing |
| E-46-002 | PC:134-144 | Pause during countdown | LaunchedEffect cancels; controls stay | Disarm; on resume re-arm with full 4000 ms (not the remainder) | unit: pause→resume gives full delay |
| E-46-003 | PC:134-144 | Seekbar drag begins while controls shown | `isSeeking` key change cancels timer, controls stay until drag ends | Disarm while dragging; re-arm on drag end | unit `auto_hide_wanted` false during seek |
| E-46-004 | VM:666-672 | Tap while a sheet is open | `showControls` refuses; `hideControls` is also called by tap if controls shown (tap toggles off, GH:125) | Tap while sheet is modal does not reach the surface (sheet covers); if it does, `show_controls` refuses | unit `show_controls` false when sheet open |
| E-46-005 | GH:124-126 + PC:322-334 | Locked, controls hidden, tap | `controlsShown` toggles, unlock button appears only when shown | Same | unit `tap` while locked toggles `controls_shown`; unlock visible iff shown && locked |
| E-46-006 | GH:141-147 | Tap on surface while a panel is open | Panel closes (`panelShown = None`) unless `pref_allow_gestures_in_panels` | Same | unit `press_closes_panel(allow=false)` |
| E-46-007 | VM:666-669 | Controls shown when panel open via long press | `showControls` refused (panel open) | Same | unit |
| E-46-008 | VM:688-691 | `showSeekBar` while sheet open | returns, no change | Same | unit |
| E-46-009 | VM:679-682 | `hideControls` hides the status bar on every call | Android-only | No-op on Linux (no status bar in the desktop window) | manual: no side effect |
| E-46-010 | PC:322-335 | Lock with controls shown | Chrome hidden instantly (visible flags false), unlock button visible due to `controlsShown && locked` | Same | manual |

---

## 2. Gestures

| ID | Where | Condition | Observed | Required | Test |
|---|---|---|---|---|---|
| E-46-011 | GH:122, 163-167 | Long press then release after the speed was changed by the screenshot sheet | Release sets mpv `speed` to the value captured at composition | Do not restore speed (no speed change on long press) | unit: release leaves speed unchanged |
| E-46-012 | GH:127-140 | Double-tap exactly at `0.4 * width` | `x < 0.4w` false → centre | Strict comparisons as in `math::zone` | unit `zone(0.4w)=Center`, `zone(0.6w)=Center` |
| E-46-013 | GH:249-282 | Volume reaches max device step, user drags up | Boost branch starts; `startingY` reset to 0 on entry, `originalVolume` re-read | Same anchoring rule; test that first boost event does not jump the mpv volume | unit with sequence of `(y)` values |
| E-46-014 | VerticalSliders.kt:184, 59; Float.roundToInt | `maxVolume == 0` (device reports no steps) | `percentage` divides by zero → NaN; `roundToInt` on NaN throws in Kotlin | Clamp `max_steps` to at least 1; percentage 0 when range empty | unit `percent(0, 0..0) == 0` |
| E-46-015 | SeekBar.kt:109-110, SeekBar | Duration unknown (0 f) | Range `0f..0f` passed to Seeker; no crash observed in code, library behaviour unverified | Widget treats duration 0 as "unknown": no ticks, thumb at 0, drags ignored | unit `seek_segments(&_, 0.0)` empty; manual |
| E-46-016 | GH:337 (`seconds` plural) | Backward seek amount -1 | Plural `one` selected by absolute value → text `-1 second` | Same | unit `format_seconds(-1) == "-1 second"`, `(-20) == "-20 seconds"`, `(1) == "1 second"`, `(10) == "10 seconds"` |
| E-46-018 | GH:202-203 | Drag exactly at `position == 0` moving left | Guard returns; no seek | Same | unit |
| E-46-020 | GH:183, GH:188, PC:404 | Drag start or end at `position` 0.9 s | `position.toInt()` and `seekPosition.toInt()` truncate to 0 | Truncate (not round) to match | unit `start_pos_secs(0.9)==0` |
| E-46-044 | GH:193-199 | Drag while paused | Pause state unchanged; `unpause()` called only when player was not paused, no-op | Paused stays paused | unit model: paused unchanged across drag |
| E-46-045 | GH:95-103 | Second double tap during the 800 ms window | `LaunchedEffect(seekAmount)` restarts the 800 ms delay | Restart the clear timer on each step | unit `TimerSlot::restart` cancels |
| E-46-046 | GH:129-139 | Reversal of direction while accumulator non-zero | `updateSeekAmount(0)` before the new direction | Same | unit `double_tap` state machine |
| E-46-047 | GH:148-157 | Double-tap repeats in the same zone during `isDoubleTapSeeking` | Repeat call `handleRight/LeftDoubleTap` without reset | Same | unit |
| E-46-048 | GH:170-178 | Long press when the controls are locked | Ignored | Same | unit |
| E-46-049 | GH:175, PC:633-636 | Long press on an already paused video, then dismiss screenshot sheet | Long press pauses (no-op, already paused), dismiss calls `unpause()` → video starts playing | Dismiss restores the paused state present before the long press | unit: `paused_before_long_press` stored; manual |

---

## 3. Seekbar and chapters

| ID | Where | Condition | Observed | Required | Test |
|---|---|---|---|---|---|
| E-46-031 | SeekBar.kt:119-129 | Any chapter list | Segments list duplicated (`... + it`), every tick drawn twice | Each segment once; synthetic 0 only when first chapter start != 0 | unit `seek_segments` length == chapters (+1 if synthetic) |
| E-46-032 | SeekBar.kt:120 | Chapter start == duration exactly | Included (`0f..duration` inclusive) | Included | unit |
| E-46-034 | ThumbnailPreview.kt:72 | Unsorted chapter list | `lastOrNull { start <= pos }` returns the last in list order, not the max start | Keep `rev().find` semantics for the label; sorted input makes it equal to max | unit with unsorted list |
| E-46-035 | EpisodeListDialog.kt:155-156 | Bookmark toggle when the DB write fails | `episode.bookmark` mutated in memory first; UI keeps new state | Write DB in a transaction; on failure revert the row widget and show a toast | unit (DB failure mock) |
| E-46-036 | SeekBar.kt:84-91 | Drag then gesture seek concurrently | Gesture value wins (`isGestureSeeking` first) | Same precedence: gesture > drag > playhead | unit `preview_value` precedence |
| E-46-037 | PC:397-405 | Drag end without any value change | Seeks to the same point (absolute seek) | Same, no-op if equal (avoid a needless keyframe seek) | unit |
| E-46-038 | VM:747-750 | Absolute seek when `player.duration` is null/0 | `seekTo` silently returns for positions > 0 | Same result, but log a warning | unit |
| E-46-039 | VM:748 | Seek target exactly `duration` | Allowed (`!in 0..duration` rejects only outside) | Same | unit |
| E-46-040 | VM:2150-2155 | Position before first chapter | `getCurrentChapter` returns null → no pill, no skip | Same | unit `current_chapter(&[], …) == None` and pos < first start |

---

## 4. Thumbnail preview

| ID | Where | Condition | Observed | Required | Test |
|---|---|---|---|---|---|
| E-46-041 | VM:574-608 | Drag stops on an uncached tile within 2 s of the last fetch | No fetch; thumbnail stays stale or hidden until the next `updateSeekPos` | On drag end and on throttle expiry, fetch the tile for the last position | unit: throttle returns `Retry(at)` |
| E-46-042 | VM:257-262 | LRU | `LinkedHashMap` with insertion order (not access-order unless constructor says so; `removeEldestEntry = size > 3`) | Use an access-ordered LRU of 3 entries (documented as a deliberate choice if different) | unit: fourth tile evicts least recently used |
| E-46-043 | VM:~597-603 | Tile fetch returns null | `if (bitmap != null)` → no thumbnail, no error | Same, log | unit |
| E-46-050 | ThumbnailPreview.kt:90-93 | duration 0 | progress 0 | Same | unit |
| E-46-051 | ThumbnailPreview.kt:98-105 | Very narrow window: `previewWidth` greater than the available width | `minX > maxX` → `constrainedX` = minX (first branch wins) | Clamp so the card never leaves the window | unit `thumb_x(...)` with width 200 |

---

## 5. Volume, brightness and overlay

| ID | Where | Condition | Observed | Required | Test |
|---|---|---|---|---|---|
| E-46-021 | RepeatingIconButton.kt:66-76 | Schedule reaches 5 ms | GLib timer resolution is coarser; delays bunch | Accept jitter; keep 5 ms minimum; test only the sequence | unit `RepeatSchedule` sequence |
| E-46-022 | ItemNumberFormatter.kt:9-17 | `formatEpisodeNumber(0.5)` | `".5"` (DecimalFormat `#.###`) | Rust: `"0.5"` (deliberate, D-10) | unit `format_episode_number(0.5) == "0.5"` |
| E-46-023 | BottomLeft:69 | Speed starts at a non-grid value (for example 1.1 from preferences) | Cycle yields 1.35, 1.6, ... (off grid), wraps at >= 2 | Same rule; document | unit `next_speed(1.1) == 1.35` |
| E-46-025 | VM:291, AniyomiMPVView.kt:258 | Boost cap changed in preferences mid-session | mpv `volume-max` keeps the old cap until restart; UI uses the new pref | One source: write `volume-max` on every change | unit/manual |
| E-46-026 | VerticalSliders.kt:249-254 | Boost > 1000 or < -100 | No plus sign (`integer.format(boost)`) | Same text rule | unit `volume_texts` |
| E-46-052 | VerticalSliders.kt:249 | Boost exactly 0 | "" (no label) | Same | unit |
| E-46-053 | VerticalSliders.kt:225-232 | Percent exactly 30 | `VolumeMute` (first match) | Same | unit `volume_icon(30) == Mute` |
| E-46-054 | VerticalSliders.kt:163-169 | Brightness exactly 0.3 or 0.6 | Low / Medium (inclusive upper bound) | Same | unit `brightness_icon(0.3) == Low` |
| E-46-055 | VM:752-759 | Brightness below -0.75 | Clamped to -0.75 in state, veil max 0.75 | Same | unit |
| E-46-056 | VM:752-759 | Brightness to exactly 0 while hardware is at 0 | No veil | Same | unit |
| E-46-057 | VerticalSliders.kt:160 | `(-brightness).coerceIn(0f..0.75f)` with brightness > 0 | 0 → no overflow | Same | unit |
| E-46-058 | VerticalSliders.kt:105, 110 | Box `modifier` applied twice | No visible effect today | Do not replicate | n/a |
| E-46-059 | VM:286-291 | Brightness read fails (no backlight) | `getOrElse { 0f }` → veil logic at 0 | Initial 0.0 when `get()` is None; hardware writes return Err → brightness pill shows the software value only | unit with mock backend |
| E-46-060 | GH:225-227 | Volume device steps change during the drag | `volumeGestureSens` captured at drag start with the max at that time | Same | manual |

---

## 6. Dialogs, panels and sheets

| ID | Where | Condition | Observed | Required | Test |
|---|---|---|---|---|---|
| E-46-017 | EpisodeListDialog.kt:85 | `currentEpisodeIndex == -1` (current episode not in playlist) | `episodeList[-1]` → IndexOutOfBoundsException while composing rows | No highlight; scroll to top; no crash | unit `episode_row_is_current(None)` |
| E-46-024 | OutlinedNumericChooser.kt:60 | Delay at `Int.MIN_VALUE`, press `-` | `value - 50` wraps to a large positive | Saturating subtraction | unit `saturating_sub(i64::MIN)` |
| E-46-019 | OutlinedNumericChooser.kt:77 | Int field text becomes `-` | `onChange(0)`; LaunchedEffect rewrites text to `0` on next value update | Keep `-` text while focused; do not rewrite while the entry has focus | unit/manual |
| E-46-096 | OutlinedNumericChooser.kt:76-80 | Typing `1.5` into Int field | `toInt()` throws, silently ignored, text stays previous | Same (ignore) but show the error style briefly | manual |
| E-46-027 | IntegerPickerDialog.kt:22, 24, 41-42 | Recomposition during scroll | `newValue` local resets to `defaultValue` (plain var) | Keep selection in the widget; commit displayed item | unit `commit_value(selection)` |
| E-46-028 | IntegerPickerDialog.kt:23 | `defaultValue` not in `values` | Wheel starts at index 0 but dismissal commits `defaultValue` | Start at index 0 and commit index 0's value | unit |
| E-46-097 | IntegerPickerDialog.kt:23 | `step <= 0` | Kotlin `step` throws IllegalArgumentException at composition | Reject at parse: log and do not open | unit `parse_picker("…|0")` error |
| E-46-029 | SubtitleDelayPanel.kt:249-262 (shared DelayCard) | Sync running | Field shows the parent's stale value until stop | Show live value | manual |
| E-46-030 | SubtitleDelayPanel.kt:262-263 | Active sync button | Disabled; the other button stops | Same (documented) | unit `sync_button_enabled` table |
| E-46-033 | EpisodeListDialog.kt:87, Anime.kt:253-257 | displayMode has other bits set (for example `EPISODE_SHOW_NOT_PREVIEWS`) | `==` compare fails → episode names shown instead of numbers | Mask compare | unit `is_number_mode(0x00100800) == true` |
| E-46-061 | EpisodeListDialog.kt:212-227 | Empty date and scanlator present | Leading dot (`date != null` is true for `""`) | Show dot only when date non-empty | unit |
| E-46-062 | VM:1187-1189 | `getCurrentEpisodeIndex` used for the scroll index | `-1` → scroll index `size`, out of range for the list | Clamp | unit |
| E-46-063 | VM:1209-1215 | Title click without anime loaded | No dialog | Same | unit |
| E-46-064 | PlayerSheet.kt:192-197 | Sheet dismissed by drag while `dismissEvent` also fires | Double dismissal possible | Single dismissal (guard with a flag) | unit |
| E-46-065 | PlayerSheets.kt:152-155 | Chapter sheet opened with `chapter == null` | Returns; sheet not shown, `sheetShown` stays Chapters (no dismissal) | Close the sheet state when there is no current chapter | unit `open_sheet(Chapters)` with no chapter → state None |
| E-46-066 | PlayerSheets.kt:104-111 | File picker cancelled | `null` ignored | Same | manual |

---

## 7. Skip intro and custom button

| ID | Where | Condition | Observed | Required | Test |
|---|---|---|---|---|---|
| E-46-098 | VM:2058-2100 | Netflix countdown while paused | `setChapter` runs on each position update; paused means no updates → countdown freezes | Same, document | manual |
| E-46-067 | VM:2080-2085 | Netflix toast | Hard-coded English `"Skip Intro: "` prefix | Resource key | unit (strings table) |
| E-46-068 | VM:2063-2102 | Chapter type `Other` | Button cleared, countdown reset | Same | unit |
| E-46-069 | VM:2133-2137 | Skip pressed during countdown | Cancels countdown (`waitingSkipIntro = -1`) and returns without seeking | Same | unit |
| E-46-070 | VM:2150 | Skip with no chapter at position | `getCurrentChapter` null → nothing | Same | unit |
| E-46-071 | VM:2167-2172 | Custom button with id `0` | Script message `call_button_0` | Same | unit |

---

## 8. Keyboard, mouse and timers (Linux specific)

| ID | Condition | Required result | Test |
|---|---|---|---|
| E-46-072 | GTK double-click time 400 ms vs Android 300 ms | Use GTK setting; no hard-coded constant | manual |
| E-46-073 | Mouse long press with GestureLongPress not firing on some backends | Timer-based long press (P-3) | manual on X11 and Wayland |
| E-46-074 | Rapid horizontal drag with GTK drag threshold 8 px | First 8 px ignored by GTK drag-update (begin point used instead, D-1) | unit with offsets |
| E-46-075 | Wheel scroll over surface while locked | Ignored (lock covers volume too) | unit |
| E-46-076 | Keyboard Escape with sheet open | Sheet closes, controls not shown until next tap (sheet close shows controls: VM:709-719 calls showControls on None) | unit `set_sheet(None)` shows controls |
| E-46-077 | Window unfocused during auto-hide | Timer keeps running (Android: app in background pauses coroutines) | Proposal: pause timers when the window is not active (`is-active` notify) and re-arm on focus |
| E-46-078 | Fullscreen toggle | Controls keep state | manual |
| E-46-079 | Monitor scale 2x (HiDPI) | All px values are logical px in GTK; no change | manual |

---

## 9. Race conditions and concurrency

| ID | Race | Mitigation | Test |
|---|---|---|---|
| E-46-080 | Seek drag end and mpv `time-pos` event arriving after `SeekAbsolute` | Ignore position events for 200 ms after commit, or until a position within 1 s of the target arrives | unit with event order |
| E-46-081 | Two `MpvCmd::SetAudioDelay` during fast delay changes | Coalesce to the latest value in the mpv thread | unit (coalescer) |
| E-46-082 | Episode change while thumbnail fetch in flight | Cancel the fetch token on episode change; ignore late results (`resetState`, VM:1023-1031) | unit with token |
| E-46-083 | Screenshot sheet open while episode switches | Sheet closes on episode change (part 2) | manual |
| E-46-084 | Auto-hide timer fires during a tap | Tap restarts the timer by `reset_epoch`; no double hide | unit |
| E-46-085 | Lock toggled during a horizontal drag | Drag handler returns early on next update; in-flight drag ends with a commit | manual |
| E-46-086 | Volume change from device side (other app) while dragging | Read device value at drag start only (Kotlin does) | manual |
| E-46-087 | `SeekAbsolute` queued behind a slow `Query` | Queue order preserved; `Query` only from non-GTK threads | unit ordering |

---

## 10. Failure modes

| ID | Failure | Observed (Kotlin) | Required (Kurobayomi) |
|---|---|---|---|
| E-46-088 | mpv command error (for example `seek` rejected) | `MPVLib` returns error code; not surfaced | log at warn; no UI change |
| E-46-089 | mpv thread dies | Not handled in scope | Show "Player stopped" pill once; disable transport |
| E-46-090 | Backlight not writable | Android always writable | Fall back to software veil, log once, brightness pill still works |
| E-46-091 | PipeWire unavailable (no `wpctl`) | Android always has audio manager | Volume pill shows boost only; device volume read-only; log once |
| E-46-092 | Thumbnail tile decode error | `Failed to fetch thumbnails tiles` logged (VM:~600) | Log; no thumbnail |
| E-46-093 | Screenshot save error | `SaveImageResult.Error` event (VM:1847-1884) | Toast with error text (part 2 message) |
| E-46-094 | Preferences file corrupted | PreferenceStore falls back to default (PreferenceStore.kt:35-40) | Same, per key |
| E-46-095 | Integer picker script gives non-numeric bounds | `toInt()` throws in VM:906-909 (uncaught in handler) | Parse error → ignore request, log |

---

## 11. Test plan

### 11.1 Unit tests (pure Rust, no display)

Module `src/player/model.rs` tests:
1. `show_controls` refuses when sheet, panel or dialog is set (E-46-004, E-46-007).
2. `set_sheet(None)` calls show (E-46-076).
3. `set_sheet(Some)` hides controls and closes panel and dialog.
4. `auto_hide_wanted` truth table over (shown, paused, seeking) (E-46-001..003).
5. `tap` toggles and respects `locked` (E-46-005).
6. `lock`/`unlock` leaves `controls_shown` unchanged.

Module `src/player/math.rs` tests:
7. `zone` boundaries: `0.4w` and `0.6w` are Center; `0.39w` Left; `0.61w` Right (E-46-012).
8. `horizontal_step_i32`: `start=100, new=200, sens=0.15` gives `start+15`; negative truncation toward zero: `(-0.9) as i32 == 0`.
9. `vertical_step_i32`: upward drag increases value; sens 0.001*15 example: 67 px gives +1.
10. `drag_target` table for swapped and not swapped at `w/2` (behaviour 2.7).
11. `next_speed`: 1.0→1.25, 1.75→2.0, 2.0→0.25, 0.25→0.5, 1.1→1.35 (E-46-023).
12. `next_aspect` cycle Fit→Stretch→Crop→Fit.
13. `volume_icon`: 0→Off, 1→Mute, 30→Mute, 31→Down, 60→Down, 61→Up, 100→Up (E-46-053).
14. `brightness_icon`: 0→Night, 0.01→Low, 0.3→Low, 0.31→Medium, 0.6→Medium, 0.61→High.
15. `volume_texts`: (volume 7, mpv 100, pct 47, false) → ("7", ""); (…, mpv 115) → ("7", "+15"); (mpv 85) → ("7", "-15"); mpv 100+1001 → no plus (E-46-026).
16. Percent helper: `percent(0, 0..=0) == 0` (E-46-014).
17. `brightness_veil_alpha(-0.75) == 0.75`, `(0.2) == 0.0` (E-46-057).

Module `src/player/time.rs` tests:
18. `pretty_time(0) == "00:00"`, `(59) == "00:59"`, `(3600) == "1:00:00"`, `(3725) == "1:02:05"`.
19. `pretty_time_signed(0) == "+00:00"`, `(-5) == "-00:05"`, `(65) == "+01:05"`.
20. `format_episode_number`: 1.0→"1", 12.5→"12.5", 1.2345→"1.234" (the binary value of 1.2345 is below the half, so HALF_EVEN and Rust agree), 0.5→"0.5" (E-46-022).
21. `format_seconds_plural`: 1→"1 second", -1→"-1 second", 10→"10 seconds", -20→"-20 seconds" (E-46-016).

Module `src/player/segments.rs` tests:
22. `seek_segments` with chapters `[0, 90, 1330]` and duration 1420 returns the three once (E-46-031).
23. First chapter at 85 s: synthetic segment at 0 added once.
24. Chapter at exactly duration included (E-46-032).
25. Empty chapters returns empty (no synthetic).
26. Duration 0 returns empty (E-46-015).
27. `current_chapter` ties keep first (E-46-034 for the sorted case), and `chapter_label_at` uses reverse list order (E-46-034 unsorted).

Module `src/player/repeat.rs` tests:
28. Sequence of waits from 750: 750, 562, 421, 315, 236, 177, 132, 99, 74, 55, 41, 30, 22, 16, 12, 9, 6, 5, 5 (E-46-021).

Module `src/player/settings.rs` tests:
29. Default values equal `p1-data-and-config.md` section 3 table (serde default test).
30. Unknown enum string falls back to default (E-46-094).

Module `src/player/controls/numeric.rs` pure parts (behind a `parse_int_input(&str) -> Option<i64>` helper):
31. `"-"` → Some(0) (E-46-019); `"1.5"` → None (E-46-020b); `""` → Some(0) with clear flag; `" 12 "` → None or trimmed Some(12): decide and test (trim start only, as Kotlin `trimStart` for the minus check, but `toInt` does not trim: `" 12 "` → None).
32. `saturating_sub(i64::MIN, 50) == i64::MIN` (E-46-024).

Module `src/player/dialogs/picker` pure parts:
33. `parse_picker("t|%d|0|10|3|prop")` yields values 0,3,6,9 (E-46-028 table) and last excluded (10).
34. `parse_picker` with step 0 → Err (E-46-028b).
35. `commit_value(selection_index)` returns `values[index]`; with default not in list starts at index 0 (E-46-028).

Module `src/player/controls/episode.rs` pure parts:
36. `is_number_mode(0x00100800)` true (E-46-033); `is_number_mode(0x0)` false.
37. `date_label(0)` → "" and `scanlator_separator(date="", scanlator=Some)` → false (E-46-061).
38. Current index -1 → no current row (E-46-017).

Module `src/player/sync.rs` (delay sync, shared) pure parts:
39. `sync_value(start, elapsed_ms, positive)` = start ± elapsed (E-46-029 values).
40. `sync_button_enabled(kind, state)` table (E-46-030).

Module `src/player/thumbnail.rs` pure parts:
41. `thumb_x(target, width, card_w, start_inset, end_inset)` never leaves [8, width - card_w - 8] (E-46-051).
42. `TileLru` with capacity 3 evicts least recently used (E-46-042).
43. Throttle returns `Retry` within 2000 ms and `Fetch` after (E-46-041).

Module `src/player/backend.rs` tests:
44. Command coalescing for `SetAudioDelay` keeps only the last (E-46-081).
45. Commands are delivered in order (E-46-087) with a fake mpv.

Gesture state (`src/player/controls/gestures.rs` state, pure part `double_tap.rs`):
46. Reversal resets accumulator (E-46-046).
47. Repeat in same zone does not reset (E-46-047).
48. Long-press release does not change speed (E-46-011); paused-before-long-press remains paused after dismiss (E-46-049).

### 11.2 Manual checks (GTK, target machine)

Run through `p1-ui-parity.md` section 19 on Wayland (KDE) and X11. Record each as pass/fail in the test file `tests/manual/player_controls.md` (T-46a-056).

### 11.3 Fixtures

- `fixtures/chapters_unsorted.json`: chapters `[{"name":"Ending","start":1300.0},{"name":"Opening","start":0.0},{"name":"Part A","start":85.0}]` duration 1420 (E-46-034).
- `fixtures/chapters_no_zero.json`: first chapter at 85.0, duration 1420.
- `fixtures/chapters_at_end.json`: last chapter start 1420.0 equal to duration.
- `fixtures/episode_numbers.json`: values `[1.0, 12.5, 1.2345, 0.5, 100.0]` with expected strings.
- `fixtures/display_modes.json`: `[0, 0x100000, 0x100800, 0x101000]` with expected number-mode flags.
- `fixtures/picker_specs.txt`: the six lines `title|%d|0|10|3|prop`, `title|%d|0|10|0|prop`, `title|%d|5|5|1|prop`, `title|%02d|0|100|25|prop`.

### 11.4 Things not testable without a device

- Actual double-tap and long-press timing under GTK (E-46-072, E-46-073).
- Real drag offsets (E-46-074).
- Brightness and volume backend writes (E-46-090, E-46-091).
- Render of the animations (durations are asserted in unit tests on the constants only).

### 11.5 Known-unverified library behaviour

- Seeker library (`dev.vivvvek.seeker` 1.2.2) with a zero-length range: not verified (E-46-015). Action: Kurobayomi's own widget, so the question is moot after T-46a-010.
- Compose `Slider` with `valueRange` where start == end: not verified. Action: Kurobayomi widget guards.
- Compose `detectTapGestures` order between `onPress` for the second tap and `onDoubleTap` (GH:127-160). Action: verify on device when porting; the Rust model treats the second press as a double-tap (single action per double-click).
- Compose default fade duration (used for overlays without explicit spec): assumed 300 ms; verify against Compose sources before finalising the animation constants (T-46a-041).
