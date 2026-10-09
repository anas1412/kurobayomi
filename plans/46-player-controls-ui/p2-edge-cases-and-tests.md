# 46-player-controls-ui (part 2 of 2): edge cases, failure modes, races, tests

Scope: the 14 files in `p2-inventory.md`. Rules are in `p2-behavior.md`; Rust design in `p2-rust-design.md`.

## 0. Upstream tests in scope

- Repo search (`find . -name "*Test*.kt"` filtered to player, sheet, panel, delay, track paths): **no upstream tests exist** for these 14 files or for their helper functions (`toFixed`, `getTrackTitle`, `copyAsArgb`, `toColorHexString`, `resetColors`, `resetTypography`, `HosterState.getChangedAt`). The `app/src/test` tree has no player tests in this scope.
- Therefore there are no upstream test cases to port. All tests below are **new**, derived from the Kotlin behavior.

---

## 1. Edge cases by file

Each entry: trigger → observed Kotlin result → required port behavior (decision in brackets: **parity**, **fix**, or **decision needed**).

### 1.1 `SubtitleDelayPanel.kt`

E1. Delay Int overflow. `OutlinedNumericChooser` minus button calls `onChange(value - step)` with Kotlin `Int` arithmetic. At `value = Int.MIN_VALUE` the step `50` wraps to a large positive value. → **fix**: saturating arithmetic (`saturating_sub`/`saturating_add`) in the port.

E2. Delay Int range is `Int.MIN_VALUE..Int.MAX_VALUE`. mpv seconds are a double; `value / 1000.0` is exact for values below 2^53 ms, so no loss. **parity**.

E3. Sub-speed live apply only in `0.1..1` (`SubtitleDelayPanel.kt:95`). Speeds above 1 are shown but not sent to mpv until the panel is reopened or the app restarts (`AniyomiMPVView.kt:263`). → **fix**: send every accepted value (`0.1..10`) live. Release-note item.

E4. Save speed range `0.1..10` (`SubtitleDelayPanel.kt:132`) vs live range `0.1..1`. The two checks disagree. → **fix** with E3.

E5. Secondary delay is never saved (`onApply` at lines 130-133 saves only `delay` and `speed`). Restart loses it. → **decision needed**: proposal persist `pref_subtitles_secondary_delay` on "Set as default" when the target is Secondary or Both.

E6. Target `Both`: the effect writes `delay` (primary state) to both mpv properties. Changing secondary in `Both` mode is not possible (the card edits `delay` only). **parity**; document in UI.

E7. Rounding mismatch: initial read uses `roundToInt()` (line 89, 91), the `LaunchedEffect(affectedSubtitle)` read uses `.toInt()` truncation (lines 109-115). A stored value of `0.0999999` becomes `100` at start and `99` after switching target. → **fix**: `round` everywhere.

E8. Tap-sync: run started and the panel closes mid-run. The Kotlin `LaunchedEffect` is cancelled, so the final value is **not** delivered (lost). → **decision needed**: proposal on close during a run: apply the current `tick` value (keeps user input); parity alternative: discard. Default in the port: apply.

E9. Tap-sync: display frozen during the run (Kotlin shows the start value). Port default keeps parity; a live display is a flagged proposal (p2-ui-parity §11.3).

E10. Tap-sync: `Set as default` and `Reset` disabled while running; the target dropdown in the title is **not** disabled (switching target mid-run keeps the old run). → **fix**: disable the target dropdown while a run is active.

E11. Tap-sync: the numeric value is updated only at stop (`onDelayChange(finalDelay)`), but `startingDelay` is read from `delay` at start. If the user edits the field during a run (the field is still editable in Kotlin), the edit is overwritten on stop. → **decision needed**: proposal disable the field during a run.

E12. Speed control hidden for Secondary/Both, but the value still applies through `LaunchedEffect(speed)`. A user who set speed on Primary and switches to Secondary sees no speed control but `sub-speed` is unchanged. **parity** (documented).

E13. `onSpeedChange` rounds with `round(it * 1000) / 1000f` using `kotlin.math.round` (round-half-even). Port uses `f32::round_ties_even`. **parity**.

### 1.2 `SubtitleSettingsColorsCard.kt`

E14. `getCurrentMPVColor` uses `!!` on `getPropertyString` (line 205). mpv returns null before a file is loaded → NPE. → **fix**: fall back to the stored preference.

E15. `Color.parseColor` throws `IllegalArgumentException` for strings other than `#RRGGBB` / `#AARRGGBB` / named colors (for example mpv returning `#FF0000FF` is valid, but a user-typed `sub-color=red` in `mpv.conf` would be parsed through mpv's own normalization). → **fix**: `parse_mpv_color` returns `None`; keep the stored preference.

E16. Alpha 0 text is valid and invisible. **parity**.

E17. Color writes on every slider change with no throttle. → **fix** (proposal): coalesce to 50 ms.

E18. `resetColors` deletes the preference and writes the default, but the panel re-reads mpv only through `currentColor = getCurrentMPVColor(...)`, which may throw (E14). → covered by E14.

### 1.3 `SubtitleSettingsMiscellaneousCard.kt`

E19. Debug `println(it)` on each composition (line 75). → **fix**: remove.

E20. `sub-scale = 0` hides subtitles (no guard). → **parity** with a hint label (p2-ui-parity §10.2).

E21. Reset writes `sub-ass-override` as `scale` regardless of the stored value; with default `false` this is consistent. **parity**.

E22. The Override switch state is read from mpv (line 74-76), while `overrideSubsASS` pref is written. Startup writes `force` only when the pref is true (`AniyomiMPVView.kt:270-272`). If mpv was changed by the user in `mpv.conf`, the switch can show `force` while the pref says `false`. → **decision needed**: read the pref at open, write both, or keep mpv as the source. Proposal: mpv is the source for the switch; pref is for next start.

### 1.4 `SubtitleSettingsTypographyCard.kt`

E23. `SubtitleJustification.entries.first { it.value == getPropertyString("sub-justify") }` throws on any unexpected mpv string (lines 137-141, 220-222). → **fix**: map unknown to `Auto`.

E24. `SubtitlesBorderStyle.entries.first { ... }` same throw (lines 147-151, 224-226). → **fix**: unknown → `OutlineAndShadow`.

E25. Justification toggle: unchecking leaves the toggle visually checked (`justify = justification` regardless of `it`, line 201). → **fix**: set `justify` to `Auto` when `it == false`.

E26. `resetTypography` sets `sub-ass-justify` from `overrideSubsASS` (line 359). → **fix**: reset to `false` (the justify default) and leave ASS override alone.

E27. Startup applies `sub-justify` but sets `sub-ass-justify yes` only if ASS override is true (`AniyomiMPVView.kt:270-272`). A user who chose Left justification without ASS override gets left-justified non-ASS subtitles but unjustified ASS. → **decision needed**: proposal set `sub-ass-justify yes` whenever justify ≠ Auto.

E28. Font list mutated in place inside `mutableStateOf`; no recomposition on add (lines 93-117). → **fix**: Rust uses an explicit model update that triggers the dropdown rebuild.

E29. Font scan on every composition call creates `mpv-config/fonts` (`getFontsDirectory()` is called each composition, line 92). → **fix**: create once (area:13 helper).

E30. Font scan result includes `Sans Serif` twice if a font file reports that family (`distinct()` does not compare with the default). → **fix**: dedupe against the default.

E31. Unreadable or non-TTF file: `runCatching` returns null, file skipped silently. **parity** (log at debug).

E32. Font file over 64 MiB: Kotlin reads it fully. → **fix**: skip (proposal, `MAX_FONT_BYTES`).

E33. `.ttc` (collections) are skipped by the regex. **parity**; documented limitation (Linux users often have `.ttc` system fonts, but the mpv fonts dir is user-managed).

E34. Font file is added while the panel is open: not picked up until reopen (Kotlin scans on composition start). → **fix** (proposal): rescan on panel open only (unchanged) and a refresh button is not added.

E35. `font` displayed from mpv (line 143), while the preference `subtitleFont` may differ after a reset or a restart. **parity**.

### 1.5 `SubtitleSettingsPanel.kt`

E36. `BackHandler` closes the panel. Port: Escape. **parity**.

E37. The portrait pager uses `screenWidthDp * 0.9f` for page size (line 102); on desktop the stack layout replaces it (no page size). Documented.

### 1.6 `VideoFiltersPanel.kt`

E38. Reset writes each filter through `deleteAndGet()`; the stored key for an unset filter is missing so the default is `0`. **parity**.

E39. `gpuNext()` is not collected (line 135), so the warning does not update when the setting changes while the panel is open. **fix**: observe.

E40. The Int slider range is `-100..100`, steps `200`. The Kotlin Int `SliderItem` truncates float positions toward zero (`toInt()`), so a drag from `0` to `-0.4` lands on `0`. **parity**.

### 1.7 Track sheets (`AudioTracksSheet.kt`, `SubtitleTracksSheet.kt`, `GenericTracksSheet.kt`)

E41. `getTrackTitle` with `language == null` and blank name yields `#id: ` (empty name). **parity** (rule 5). Improvement proposal: `#id` alone.

E42. Off track id `-1` with a name that is not `Off` (for example `track.name` set by the view model): rule 1 returns the name verbatim. **parity**.

E43. Subtitle selection: `selectedTracks.indexOf(track.id)` gives `-1`, `0`, or `1`. If the view model returns a list with more than two entries, the `#3` label would show (observed; view-model constraint). → **fix**: cap at two slots in the Rust model.

E44. The modifier is applied to both `GenericTracksSheet` and the footer in `SubtitlesSheet` (lines 99, 109). Cosmetic: padding applied twice. → **fix**: apply once.

E45. Audio/subtitle file picker returns `null` on cancel (`PlayerSheets.kt`); the callback returns without action. **parity**.

E46. Picker MIME `*/*` accepts any file. **parity**; the port may filter by extension (`srt`, `ass`, `ssa`, `vtt`, `sub`, `mka`, `m4a`, `aac`, `ac3`, `flac`, `mp3`, `opus`, `dts`, `eac3`) as a proposal, with an "All files" fallback.

### 1.8 `ChaptersSheet.kt`

E47. `chapter == null` → sheet not rendered (`PlayerSheets.kt`). Desktop: show the sheet with an empty list? → **parity**: do not show.

E48. Chapter name `null` from mpv (`chapter-list/N/title` absent) → `null` in label (`#1: null`). → **fix**: label `#N` with an empty name, not the literal text `null`.

E49. Two equal chapters share `indexOf` (E-by-equality). → **fix**: ordinal stored at load (behavior §4.4).

E50. Chapter time `start.toInt()` truncates fractional seconds: `59.9` → `59`. **parity**.

E51. Chapter list not sorted by the sheet; `loadChapters` sorts by `start` (`PlayerViewModel.kt:493`). Two chapters with the same start keep original order (stable sort). **parity**.

### 1.9 `QualitySheet.kt`

E52. `hoster.videoState[videoIdx]` assumes equal list lengths (line 268). Mismatch throws `IndexOutOfBoundsException`. → **fix**: `get(idx)` with default `Video.State::QUEUE`.

E53. `expandedState.getOrNull(hosterIdx) ?: false`: expansion state can be shorter than hosters. **parity**.

E54. Hoster clicks during `Loading` are accepted. → **decision needed**: proposal ignore.

E55. `selectedVideoIndex` uses hoster **index**. If the hoster list changes (a hoster is added or removed while the sheet is open), the selection points at a different hoster. → **fix**: identify hosters by `name` (stable) for selection; keep index only for layout.

E56. Flat list hard-codes hoster index `0` (line 178). If the flat-mode check changes while selected, selection mismatches. → **fix**: flat mode uses the same key scheme.

E57. Empty hoster list (zero entries) → blank sheet with spinner off (observed; no empty text). **parity** (decision: keep blank).

E58. Error hoster shown only when `displayHosters.first` is true; if false, failed hosters are invisible although they exist (observed). The caller decides; port keeps the flags.

E59. `HosterState.Ready` with `videoList` empty but expanded: an empty block renders (observed). **parity**.

E60. `getChangedAt` with index out of range: `mapIndexed` keeps all entries unchanged (no error). **parity**.

### 1.10 `PlaybackSpeedSheet.kt`

E61. Presets: stored strings that are not canonical (`"1.50"`, `" 2.0"`, `"abc"`). `toFloat()` on `"abc"` throws `NumberFormatException` (line 103). → **fix**: skip invalid entries with a log; `" 2.0"` is trimmed.

E62. Remove uses `it.toFixed(2).toString()`; a stored `"1.50"` never matches `"1.5"`, so the chip cannot be removed. → **fix**: remove by parsed float equality.

E63. Duplicate keys: the set may contain `"1"` and `"1.0"` (both parse to `1.0f`). `items(key = { it })` then crashes with a duplicate-key exception. → **fix**: dedupe by float value when parsing.

E64. Current speed not equal to any preset (for example `1.2345`) shows no selected chip. Proposal: nearest preset within 0.005 is highlighted.

E65. Speed slider continuous: a drag can produce `1.2345`. "Make default speed" stores the exact float; the chip for `1.23` is not selected. **parity**.

E66. `Add` stores `toFixed(2)` of the speed (half-up). Speed `0.125` → `0.13`. The added preset differs from the speed. **parity**.

E67. Slider minimum `0.01f`. mpv accepts `0.01`. **parity**.

E68. Pitch correction switch writes mpv immediately; mpv may reject the property before a file is loaded. Failure is not reported. **parity** (log only).

E69. `RestartAlt` (Make default area) resets speed to `1f` and deletes the preference; if the preference was never set, `delete()` is a no-op. **parity**.

### 1.11 `MoreSheet.kt`

E70. Decoder chip click writes `hwdec` only (no preference). After restart, `hwdec` = `auto`/`no` from `pref_try_hwdec`. → **parity** (documented); proposal: add an opt-in "remember decoder" later.

E71. `getDecoderFromValue` throws for unknown `hwdec-current` (Linux values). → **fix** (`from_hwdec_current` returns None, §8 of design).

E72. Statistics page desync if the overlay is toggled by keyboard (the chip reads the preference). → **decision needed** (P2 task).

E73. Statistics: page `n` chip after page `m` (both > 0) sends `display-page-n` only (no toggle). If the overlay was hidden by the user (keyboard), the page command shows it again? Observed mpv behavior: `display-page-N` shows the overlay; **parity**.

E74. Custom button tap and long press share the same `interactionSource`; long press on the `FilterChip` does not show the press indication for the overlay. Cosmetic. **parity**.

E75. Custom buttons without any `call_button` script defined: `script-message` is sent and ignored by mpv. Silent. **parity**.

E76. Audio channel: choosing `ReverseStereo` then `Auto-safe`: `af=""` clears the pan filter (correct), but also clears any user `af` filter (E77). → see E77.

E77. `af=""` clears all audio filters, including user `mpv.conf` filters and any Lua-added filters. → **fix** (remove only the pan filter).

E78. Audio channels enum name trap (`Auto` writes `auto-safe`). The stored name is still the enum name, so existing settings keep working. → **parity** with the unit test in §3.

E79. Sleep timer `remainingTime` > 86399 s (more than 23:59:59): picker initial hour > 23 is clamped by Material, not by Kotlin code; the port spin button allows up to 99 h. → **parity** for display, OK.

E80. Sleep timer `00:00` + OK → `onStartTimer(0)` = cancel (observed). **parity**; documented.

E81. Sleep timer does not persist across restart (view model; part 1). **parity**.

### 1.12 `ScreenshotSheet.kt`

E82. `takeScreenshot(...)!!` throws when mpv did not write the temp file (lines 83, 91, 118-122). → **fix**: show `player_screenshot_failed` (new string, p2-ui-parity §5.3).

E83. Share and Save each call `takeScreenshot` again (a new temp file each time, but the same final `mpv_screenshot.png`). Concurrent use can read the file while it is renamed. → **fix**: unique final name per capture.

E84. "Include Subtitles" toggles the next screenshot only; the toggle is in the sheet and the value is held by the caller. **parity**.

E85. `isLocalSource == false` hides the Thumbnail tile; `setAsArt(Thumbnail)` is still reachable through code only. **parity**.

E86. Confirm dialog text is the same for every art type (`confirm_set_image_as_cover`). **decision needed**: new per-type keys (p2-ui-parity §17). Port default: parity text.

E87. `SetAsArt.AddToLibraryFirst`: the anime is not local and not favorite; the cover is not saved. Result is shown by part 1. Covered by area:45 tests.

### 1.13 `SubtitleTracksSheet.kt`

E88. Two-slot model (E43). Selecting a third track: the view model enforces the maximum; the UI does not. → **fix**: the Rust model enforces two slots.

E89. Checkbox click toggles through `onClick` (`onCheckedChange = { _ -> onClick() }`). Clicking a checked primary removes it, leaving the secondary in slot 0? The view model reorders the list (out of scope). Observed: slot labels shift. **decision needed**: keep slots stable (proposal: removing primary promotes secondary to primary, same as the view model).

### 1.14 `ChaptersSheet` cross-file

E90. The chapter list belongs to the loaded file; switching episodes while the sheet is open keeps the old list until the sheet closes (view model). **parity**.

---

## 2. Failure modes (system level)

F1. **mpv handle missing or not initialized.** All `get_*` return `None`; the UI shows preference values. All `set_*` return an error and are logged. The sheet still opens. Test: `MpvProps` fake that returns `None` everywhere.

F2. **mpv property rejected** (`set_*` error, for example `sub-ass-override` with an invalid value). Logged; the model keeps the new value so the UI does not flicker; the preference is written anyway (Kotlin writes preference first, then mpv).

F3. **Preference store write failure.** Store trait returns nothing; the port logs and continues (Kotlin ignores).

F4. **Font directory unreadable** (permissions). `scan_fonts_dir` returns an empty list; dropdown shows `Sans Serif`. Test: directory with mode 000 (Unix test).

F5. **Corrupt font file.** `family_name` returns `None`; file skipped. Test: random bytes fixture.

F6. **Screenshot temp file missing.** Port returns an error and shows the new message; no panic. Test: fake `MpvProps` whose `command("screenshot-to-file", …)` does not write the file.

F7. **Hoster list empty and not loading.** Blank sheet (parity).

F8. **Speed preset string corrupt.** Skipped with log.

F9. **Decoder unknown.** `from_hwdec_current` → None; no chip selected.

F10. **Clock skew during tap-sync.** Uses `Instant` (monotonic); no negative elapsed. Test: `Instant` arithmetic with `saturating_duration_since`.

F11. **Very long tap-sync run** (for example 10 min): value = start ± 600000 ms, fits `i64`. Fine.

F12. **Window closed during a font scan.** Result arrives after the widget is dropped; the callback must upgrade a `glib::WeakRef` and do nothing if gone.

---

## 3. Race conditions and ordering

R1. **Panel close during a tap-sync run.** See E8 (decision: apply the current value on close; implemented by calling `on_change` in `SheetHost::close` before dropping the panel).

R2. **Two writes to the same mpv property in one frame** (for example `sub-delay` from the delay field and from tap-sync stop). Port: model update and mpv write happen in one function; no double write.

R3. **Speed slider drag vs preset chip click.** Both update the same speed model; last write wins; no lock needed on the GTK thread.

R4. **Quality sheet selection by index with live hoster updates** (E55). Use hoster name as key.

R5. **Font scan completes after the typography panel was closed and reopened.** Two scans may run; the second scan's result wins. Port: keep a generation counter; drop stale results.

R6. **Screenshot Save and Share pressed quickly.** Two captures produce two unique temp files; no shared final name (E83).

R7. **Stats page toggle vs chip click.** The chip sends commands then writes the preference. If mpv applies `display-stats-toggle` asynchronously, a second click may see the old state. Port: compute actions from the model's `statistics_page`, not from mpv (matches Kotlin), and keep the action list (`logic::stats_page`).

R8. **Language change while a sheet is open.** Labels are fixed at build time (Kotlin recomposes). Port: rebuild the sheet on locale change (area:36).

R9. **Chapter click while the chapter list is re-sorted.** Index passed to `onSeekToChapter` is computed from the list at click time (Kotlin `chapters.indexOf(it)`). Port: pass the chapter ordinal, not the list position at render time.

---

## 4. Upstream behaviors that must stay identical (parity contract)

P1. `getTrackTitle` five-row table (behavior §1.4).
P2. `toFixed` half-up rounding to 2 places for presets.
P3. Delay tap-sync truth table and `±elapsed ms` arithmetic (behavior §11.5).
P4. Hoster grouping order and display flags (behavior §5.4).
P5. Statistics toggle xor table (behavior §7.2).
P6. Audio channel mpv mapping with swapped names (behavior §7.3).
P7. Speed presets default set (behavior §6.1).
P8. Subtitle color hex format `#AARRGGBB` uppercase (behavior §14.1).
P9. Font filename regex `.*\.[ot]tf$` (case-insensitive).
P10. Sleep timer seconds = `hour*3600 + minute*60`.
P11. Preference key spellings, including `sub_shadow_offset` (no `pref_` prefix) and `pref_subtitles_font_size` (plural "subtitles").
P12. Enum stored form = Kotlin enum name.

---

## 5. New tests to write (Rust)

Each test is a single `#[test]` in the module named in brackets. Expected values are exact.

### 5.1 `logic::track_label` (`src/player/logic/track_label.rs`)

- T1 `track_label(-1, "Off", None)` → `"Off"`.
- T2 `track_label(1, "Japanese", None)` → `"#1: Japanese"`.
- T3 `track_label(2, "Commentary", Some("eng"))` → `"#2: Commentary (eng)"`.
- T4 `track_label(3, "", Some("eng"))` → `"#3: eng"`.
- T5 `track_label(4, "  ", Some(" "))` → `"#4: "` (blank language and name, rule 5; name is the blank string).
- T6 `track_label(5, "", None)` → `"#5: "`.

### 5.2 `logic::number`

- N1 `to_fixed(1.2345, 2)` → `1.23`.
- N2 `to_fixed(1.75, 2)` → `1.75`.
- N3 `to_fixed(0.25, 2)` → `0.25`.
- N4 `preset_string(1.0)` → `"1.0"`; `preset_string(0.25)` → `"0.25"`; `preset_string(3.0)` → `"3.0"`.
- N5 `round3_half_even` rule: `round_ties_even(2.5) == 2.0`, `round_ties_even(3.5) == 4.0` (the Kotlin `round` contract).
- N6 `format_elapsed(0)` → `"00:00"`; `(65)` → `"01:05"`; `(3599)` → `"59:59"`; `(3600)` → `"1:00:00"`; `(3723)` → `"1:02:03"`.
- N7 `pretty_time` — same shape as N6 (port definition, §11.1 of the design); mark as a port-defined function.

### 5.3 `logic::color`

- C1 `color_hex(-1)` → `"#FFFFFFFF"`.
- C2 `color_hex(-16777216)` → `"#FF000000"`.
- C3 `color_hex(0)` → `"#00000000"`.
- C4 `copy_argb` with `red=Some(255)` on `0x00000000` → `0x00FF0000` as i32.
- C5 `parse_mpv_color("#ff0000")` → `Some(0xFFFF0000u32 as i32)`.
- C6 `parse_mpv_color("#80FF0000")` → `Some(0x80FF0000u32 as i32)`.
- C7 `parse_mpv_color("red")` → `None`.
- C8 `parse_mpv_color("#GG0000")` → `None`.

### 5.4 `logic::delay_measure`

- D1 Audio: idle, press A at t0, tick at t0+120 ms with start 100 → tick value `220`; press A again (disabled, Positive run) → `Ignored`; press B → `Stopped(220)`.
  - Precisely: press A at t0 gives `Started`. enabled(A) is false while Positive (state == start of A). So press A is `Ignored`. press B: enabled(B) = run.sign(Positive) != start_sign(B)=Negative → true → stops with value `start + elapsed`.
- D2 Audio: press B from idle → Negative run; value decreases with elapsed.
- D3 Subtitle: press A from idle → Negative run; press B → Stopped at value `start - elapsed`.
- D4 Subtitle: press B from idle → Positive run; press A stops it at `start + elapsed`.
- D5 `enabled` truth table: for kind Audio: idle → (A true, B true); Positive → (A false, B true); Negative → (A true, B false). For kind Subtitle: idle → (true, true); Negative → (A false, B true)… check: Subtitle A start Negative: enabled(A) = run.sign != Negative → false when Negative; enabled(B) = run.sign != Positive → true when Negative. So Negative → (A false, B true). Positive → (A true, B false).
- D6 `final_ms` after stop equals the stop value; a second stop without a run returns `Ignored` (no `Stopped` without `Started`).
- D7 Wall clock: elapsed uses `Instant` difference; a tick earlier than the start yields the start value (`saturating_duration_since`).

### 5.5 `logic::stats_page`

- S1 `stats_actions(0, 0)` → toggle false, show None.
- S2 `stats_actions(0, 3)` → toggle true, show Some(3).
- S3 `stats_actions(3, 0)` → toggle true, show None.
- S4 `stats_actions(2, 3)` → toggle false, show Some(3).
- S5 `stats_actions(3, 3)` → toggle false, show Some(3).

### 5.6 `logic::audio_channels`

- A1 `mpv_write(Auto)` → `("audio-channels", "auto-safe")`.
- A2 `mpv_write(AutoSafe)` → `("audio-channels", "auto")`.
- A3 `mpv_write(ReverseStereo)` → `("af", "pan=[stereo|c0=c1|c1=c0]")`.
- A4 `from_stored("Bogus")` → `AutoSafe`.
- A5 `from_stored("ReverseStereo")` → `ReverseStereo`.
- A6 `label(Auto)` → `"Auto"` and `label(AutoSafe)` → `"Auto-safe"` (the swapped-name check).

### 5.7 `logic::speed_presets`

- P1 `parse_sorted(["1.0","0.25","x","1.0"])` → `[0.25, 1.0]` (dedupe by float, invalid skipped).
- P2 `add(["1.0"], 1.0)` → unchanged; `add(["1.0"], 1.5)` → `["1.0","1.5"]`.
- P3 `remove(["1.0","1.5"], 1.5)` → `["1.0"]`.
- P4 `remove(["1.50"], 1.5)` → `["1.50"]` removed → `[]` (canonical match via float equality).
- P5 `DEFAULT_PRESETS` length 12, first `"0.25"`, last `"4.0"`.

### 5.8 `logic::hoster_groups`

- H1 `[Idle, Loading, Ready(videos>0), Error, Ready(videos=0)]` with show_failed=true, show_empty=true → valid = [0,1,2], failed = [3], empty = [4].
- H2 same with show_failed=false, show_empty=false → failed and empty lists returned empty (the caller decides display; test that the flags are honored in the view, not in grouping: grouping returns all, view filters). **Decide**: grouping function returns groups unconditionally; the flags are applied in the view. Test asserts that.
- H3 empty input → all groups empty.

### 5.9 `logic::decoder`

- DC1 `from_hwdec_current("no")` → `Some(Sw)`.
- DC2 `from_hwdec_current("vaapi-copy")` → `Some(Hw)`.
- DC3 `from_hwdec_current("vaapi")` → `Some(HwPlus)`.
- DC4 `from_hwdec_current("bogus")` → `None` (no panic).
- DC5 `CHIPS` = `[AutoCopy, Sw, Hw, HwPlus]` (Auto hidden).
- DC6 `linux_value(Hw, Nvidia)` → `"nvdec-copy"`.

### 5.10 `logic::fonts`

- F1 `is_font_file("A.TTF")` → true; `"a.otf"` → true; `"a.ttc"` → false; `"a.ttf.bak"` → false; `"ttf"` → false.
- F2 `family_name(sample_bytes)` on the fixture `tests/fixtures/fonts/Sample-Regular.ttf` → `"Kurobayomi Sample"`.
- F3 `family_name(&[0u8; 16])` → None.
- F4 `scan_fonts_dir(fixture_dir)` with `Sample-Regular.ttf` twice (renamed copy `Sample-Copy.ttf`) → `["Kurobayomi Sample"]` (dedupe keeps first).
- F5 `scan_fonts_dir` on missing dir → `[]`.
- F6 file over `MAX_FONT_BYTES` (create a 65 MiB sparse file in a temp dir) → skipped.

### 5.11 `logic::delay` integer edge (E1)

- X1 `saturating_sub(i64::MIN, 50)` → `i64::MIN` (the port's i64 model must not wrap). Use the port's delay type (`i64` ms).

### 5.12 `settings_view` (Store fake)

- SV1 Enum stored form: `set_str_set` not used; `stored_name(AudioChannels::AutoSafe)` → `"AutoSafe"`.
- SV2 `reset_typography` with `MpvProps` fake → recorded writes exactly: `sub-bold=false`, `sub-italic=false`, `sub-ass-justify=false` (bug fix), `sub-justify=auto`, `sub-font=Sans Serif`, `sub-font-size=55`, `sub-border-size=3`, `sub-shadow-offset=0`, `sub-border-style=outline-and-shadow`.
- SV3 `reset_misc` writes `sub-pos=100`, `sub-scale=1`, `sub-ass-override=scale`.
- SV4 `reset_colors(Text)` writes `sub-color=#FFFFFFFF`.

### 5.13 `mpvprops`

- M1 `set_audio_channels(Auto)` writes `audio-channels=auto-safe`, never writes `af=""`.
- M2 `set_audio_channels(Stereo)` writes `audio-channels=stereo`, and removes only the pan filter (for example `af remove @pan` proposal form; the fake records the command).
- M3 `stats_toggle` sends `["script-binding", "stats/display-stats-toggle"]`.
- M4 `custom_button(7, true)` sends `["script-message", "call_button_7_long"]`.
- M5 `screenshot_to_file` sends `["screenshot-to-file", path, "subtitles"]` when `with_subs`.

---

## 6. Fixtures needed

| Fixture | Path (proposal) | Content / how to make |
|---|---|---|
| Sample TTF with family `Kurobayomi Sample` | `tests/fixtures/fonts/Sample-Regular.ttf` | generate with fontTools (`fontTools.fontBuilder`) with name id 1 = `Kurobayomi Sample`; licence CC0 for the file |
| Corrupt font | `tests/fixtures/fonts/Broken.ttf` | 16 bytes of `0x00` |
| Sample `hwdec` strings | inline in tests | `"no"`, `"vaapi-copy"`, `"vaapi"`, `"nvdec-copy"`, `"bogus"` |
| Track list | inline | `TrackInfo { id: -1, name: "Off", language: None }`, `{ id: 1, name: "Japanese", language: Some("jpn") }`, `{ id: 2, name: "", language: Some("eng") }`, `{ id: 3, name: "Commentary", language: None }` |
| Hoster list | inline | `Idle("A")`, `Loading("B")`, `Ready("C", 2 videos)`, `Error("D")`, `Ready("E", 0 videos)` |
| Chapter list | inline | three chapters with starts `0.0`, `59.9`, `3600.0` |
| `MpvProps` fake | `src/player/mpvprops.rs` test module | `RefCell<Vec<(String, String)>>` recorder; `get_*` from a `HashMap` |
| `Store` fake | `src/player/settings_view.rs` test module | `RefCell<HashMap<String, String>>` |

---

## 7. Manual verification checklist (desktop)

These are run by the developer on the real app after the port lands.

M-1. Open a video, open Subtitles → checkboxes, `#1`/`#2` labels, footer text visible.
M-2. Add an external subtitle, pick a file → track appears, `Off` remains.
M-3. Open Audio → radio rows; select a track; the playing track changes.
M-4. Chapters → click a chapter; playback jumps; current chapter italic/bold.
M-5. Qualities → hoster rows expand; `Tap to load` loads; error row shows red icon.
M-6. Speed → slider changes playback; add preset; remove preset; `Make default` persists after restart.
M-7. More → decoder chip changes `hwdec` (check with `mpv` stats); audio channel `Reverse stereo` and back to `Auto-safe` restores normal audio without losing a user filter.
M-8. More → sleep timer 00:01 → playback pauses after 60 s (part 1 timer); `Cancel current timer` appears only while running.
M-9. Screenshot → Save writes a PNG; Share opens the portal; set-as-background confirm works; thumbnail tile absent for online sources.
M-10. Subtitle delay → tap-sync: press `Voice heard`, wait 1 s, press `Text seen` → delay becomes about `-1000` ms (subtitle, start Negative); the value sticks.
M-11. Subtitle settings → font dropdown lists fonts from `mpv-config/fonts`; bold/italic/justify update the rendered subtitles.
M-12. Colors → change text color; alpha 0 hides text; reset restores white.
M-13. Video filters → sliders change the picture; reset zeroes all five.
M-14. Escape closes every sheet and panel; scrim click closes; focus returns to the opener.
