# Area 49 (part 1): edge cases, failure modes, tests

Upstream tests: none. `find` over `/home/blackbox/aniyomi-src` for `*Test*.kt` matching theme, color, presentation/core, scroll, wheel, or window names returns nothing, and there is no `src/test` for these packages. All tests below are new and are Rust tests (`cargo test`), plus two manual checks marked MANUAL.

Notation: `E-49-NN` = edge case; `TEST-49-NN` = test. Task ids are in `p1-tasks.md`.

## 1. Edge cases: theme resolution

| ID | Case | Expected behavior | Source evidence |
|---|---|---|---|
| E-49-01 | `pref_app_theme` holds an unknown string (e.g., `"FOO"`) | `AppTheme::Default`; no crash | `PreferenceStore.getEnum` catch (`core/common/.../PreferenceStore.kt:34-40`) |
| E-49-02 | Deprecated value `DARK_BLUE`, `HOT_PINK`, or `BLUE` stored | Renders as Default scheme; hidden from picker | `TachiyomiTheme.kt:80` (`getOrDefault`), `AppTheme.kt` (titleRes null) |
| E-49-03 | A scheme omits tokens (e.g., Cloudflare omits `outlineVariant`, `scrim`, `surfaceContainer*`, `error*`) | Omitted tokens equal the M3 baseline values for that mode, not the primary color | `p1-behavior.md` 1.2; `p1-topic-color-token-tables.md` |
| E-49-04 | AMOLED flag on, light mode | Light scheme returned unchanged; AMOLED ignored | `BaseColorScheme.kt:18` |
| E-49-05 | AMOLED on, dark mode | surface/background black; containers 0C0C0C / 131313 / 1B1B1B; other roles untouched | `BaseColorScheme.kt:22-33` |
| E-49-06 | Monet selected but unavailable (no portal accent, no KDE seed) | Picker hides Monet; a stored `MONET` falls back to `System` on Linux (Proposal) | `AppThemePreferenceWidget.kt:85` (filter) |
| E-49-07 | Monet `tertiaryContainer` role | Must use the `tertiaryContainer` role. The Android source uses `tertiary()` (defect at `MonetColorScheme.kt:97`); the port must not copy it | `MonetColorScheme.kt:97` |
| E-49-08 | Monet contrast value out of range from portal (e.g., 2) | Clamp to the supported value (0 or 1) | `MonetColorScheme.kt:76-81` (contrast -1..1 on Android) |
| E-49-09 | Wheel picker first composition | Source fires `onSelectionChanged` once immediately; Kurobayomi does not (Proposal), test that the first real change is reported | `WheelPicker.kt:126-133` |
| E-49-10 | Wheel picker `startIndex` changed after first composition | Ignored (source uses `rememberLazyListState(startIndex)`); Kurobayomi uses the initial value only and documents it | `WheelPicker.kt:118` |
| E-49-11 | Manual number entry with no matching item | Field closes, selection unchanged | `WheelPicker.kt:152-163` |
| E-49-12 | Manual number entry matches an item | Selects and scrolls to that index | `WheelPicker.kt:154-160` |
| E-49-13 | Window width exactly 600 dp or 840 dp | 600 -> Compact; 840 -> Medium (strict greater-than) | `WindowSize.kt:12,19` |
| E-49-14 | Fast scroller: all visible list items are sticky headers | Source throws NPE (`!!`) in `computeScrollOffset/Range(LazyListState)`; Kurobayomi must not panic: fall back to the first item | `VerticalFastScroller.kt:413,429` |
| E-49-15 | Grid fast scroller: column count changes at runtime | Source keeps the first `columnCount` (`remember` without keys); Kurobayomi recomputes on change | `VerticalFastScroller.kt:277` |
| E-49-16 | Grid fast scroller: fraction within an item when `scrollItemWhole > 0` | Source computes `scrollItem % scrollItemWhole` (not `scrollItem - scrollItemWhole`). Kurobayomi uses proportional pixel mapping (`p1-rust-design.md` 9.1), so this formula is not ported; test the expected item index mapping, not the oddity | `VerticalFastScroller.kt:287` |
| E-49-17 | Fast scroller: list fits in viewport | No thumb; no fade | `VerticalFastScroller.kt:84` |
| E-49-18 | Fast scroller: thumb dragged to bottom | Clamped to track end; index within range | `VerticalFastScroller.kt:159-162` |
| E-49-19 | Numeric chooser: minus pressed below min | Clamped to min | `SettingsItems.kt:520` |
| E-49-20 | Numeric chooser: typed text "-" or "" | Source reverts to last valid integer; Kurobayomi allows an intermediate state and commits on blur/Enter | `SettingsItems.kt:536-537` |
| E-49-21 | Numeric chooser: typed value below min | Source does not clamp typed input; Kurobayomi clamps on commit (Proposal) | `SettingsItems.kt:515-538` |
| E-49-22 | `SelectItem` with `selectedIndex` out of range | Source throws; Kurobayomi shows an empty value and logs a warning | `SettingsItems.kt:323,432` |
| E-49-23 | Repeat button: pointer moves off the button | Source stops repeating on ACTION_MOVE; Kurobayomi stops on leave (Proposal) | `SettingsItems.kt:477-485` |
| E-49-24 | Repeat button: release after many repeats | Timer cancelled; no further clicks | `SettingsItems.kt:492-502` |
| E-49-25 | Slider value set to the same value | `onChange` not called | `SettingsItems.kt:274` |
| E-49-26 | TriState with `onClick == null` | Row not clickable; icon uses disabled tint | `SettingsItems.kt:363-366,393-395` |
| E-49-27 | Dialog text longer than the window | Source wraps and grows; Kurobayomi scrolls the text block at 60% window height (Proposal) | `AlertDialog.kt:38-47` |
| E-49-28 | Sheet swipe-dismiss disabled | Drag does nothing; outside tap and Escape still close | `AdaptiveSheet.kt:181-185,191-194` |
| E-49-29 | Sheet dragged less than 56 dp and released | Snaps back open | `AdaptiveSheet.kt:122` |
| E-49-30 | Sheet fling down faster than 125 dp/s | Closes | `AdaptiveSheet.kt:123,236-245` |
| E-49-31 | Sheet dismiss requested twice | `onDismissRequest` called once | `AdaptiveSheet.kt:203-206` (collectLatest on settled 1) |
| E-49-32 | `SliderItem` with `valueRange` of one element (`0..0`) | steps = -1 in source; Kurobayomi uses max(0, steps) | `SettingsItems.kt:215` |
| E-49-33 | `SettingsIconGrid` in an unbounded parent | Compose throws; Kurobayomi requires a `ScrolledWindow` with bounded height | `SettingsItems.kt:592-608`, callers `OrientationSelectDialog.kt:69` |
| E-49-34 | ExtendedFAB toggled quickly | Animations restart from current value (Compose `animateDpAsState`); Kurobayomi CSS transition restarts too | `FloatingActionButton.kt:58-73` |
| E-49-35 | Circular progress progress goes 0 -> 0.15 | Indeterminate to determinate crossfade, no flicker | `CircularProgressIndicator.kt:44-75` |
| E-49-36 | Progress value above 1 or NaN | Clamp to [0, 1]; NaN treated as 0 (indeterminate) | Source does not clamp; Kurobayomi defines it (Proposal) |
| E-49-37 | Badge text longer than width | One line, ellipsis | `Badges.kt:53` (`maxLines = 1`) |
| E-49-38 | Pill second overload | Same as first with `bodyMedium` and given font size; unused `LocalTextStyle` local removed | `Pill.kt:54` |
| E-49-39 | Theme preview cards when theme list changes order | Stable keys by `AppTheme` name | `AppThemePreferenceWidget.kt:103` (`key = { it.name }`) |
| E-49-40 | Accent color from portal with alpha channel or out-of-range values | Use RGB only; reject NaN or outside [0, 1] and fall back | Proposal (portal `(ddd)` spec) |
| E-49-41 | Portal not running | Dark detection falls back to GTK settings; Monet unavailable; no error UI | Proposal (`p1-rust-design.md` 13) |
| E-49-42 | Theme changed while a dialog is open | Dialog re-renders through the global CSS; no restart (Compose recreates the Activity) | `AppThemePreferenceWidget.kt:88-95` |
| E-49-43 | Light/dark switch while system dark changes | Re-resolve on `SettingChanged` signal; no restart | Proposal (`p1-rust-design.md` 4.4) |

## 2. Failure modes

| ID | Failure | Behavior |
|---|---|---|
| F-49-01 | CSS parse error from generated tokens | Log error with the failing line; keep the previous provider (do not remove the old one until the new one parses) |
| F-49-02 | `gtk::CssProvider` lacks a `@define-color` name used by a class | Silent fallback in GTK; CI test ensures every class-referenced token is defined (TEST-49-06) |
| F-49-03 | Monet seed read times out | Treat as unavailable after 2 s; continue with the non-Monet scheme |
| F-49-04 | Portal returns an unknown `color-scheme` integer | Treat as "no preference" (0) |
| F-49-05 | Window width reported as 0 during startup | Do not emit a class change for 0; keep Compact |

## 3. Race conditions

| ID | Race | Mitigation |
|---|---|---|
| R-49-01 | Two theme changes in quick succession (user taps two cards) | Apply is idempotent; the last `ThemeSettings` wins; remove-then-add of the provider runs on the GTK thread in one call |
| R-49-02 | Portal signal arrives during `apply` | Signals are queued onto the GTK thread; `apply` is not re-entrant (guard with a `Cell<bool>`) |
| R-49-03 | Sheet close callback runs after the widget is destroyed | Check that the window is still alive (`weak` ref) before calling `on_dismiss` |
| R-49-04 | Wheel snap animation vs. user drag | A drag cancels the snap; the snap starts only after `drag-end` |
| R-49-05 | Fast scroller thumb drag vs. list scroll | When dragging, ignore the adjustment's `value-changed` coming from the thumb itself (use a `dragging` flag) |
| R-49-06 | Repeat button timer vs. widget destroy | Cancel the source in `unrealize`/`destroy` |
| R-49-07 | Progress ring tick callback continues after hide | Remove the tick callback on `unmap` |

## 4. Tests

Pure Rust unit tests (no display needed) unless marked.

| ID | Test | What it checks | Fixture |
|---|---|---|---|
| TEST-49-01 | `AppTheme::from_pref_name` round trip for all 22 Android names | names, deprecated mapping (E-49-01, E-49-02) | literal list from `p1-data-and-config.md` 1.1 |
| TEST-49-02 | `AppTheme::title_key` for all entries | keys match `p1-behavior.md` 12 (MR/AYMR split) | literal table |
| TEST-49-03 | `resolve(Dark, amoled=false)` equals the scheme's dark table for all 18 schemes | no accidental mutation | generated scheme tables |
| TEST-49-04 | `resolve(Dark, amoled=true)` applies exactly the 10 overrides in `p1-rust-design.md` 3 and changes no other field | field-by-field diff against the dark table | |
| TEST-49-05 | `resolve(Light, amoled=true)` equals the light table | E-49-04 | |
| TEST-49-06 | CSS: every `@kuro_*` name used in a class is defined in `:root`, and each class parses under `gtk::CssProvider` (GTK test, MANUAL-ish: run in CI with a display or `GDK_BACKEND=broadway`) | F-49-02 | rendered CSS |
| TEST-49-07 | Omitted-token resolution equals `BASELINE_*` for Cloudflare `outline_variant`, `scrim`, `surface_dim`, `surface_container_high` (dark), and `surface_tint` = scheme primary for GreenApple | E-49-03 | scheme definitions |
| TEST-49-08 | Monet generation for seed `0xFF1E88E5` (Sapphire blue) dark and light, contrast 0 and 1, compares to `material-color-utilities` reference outputs (vectors generated once from the Java/Kotlin library and frozen) | A-49-1 | reference vectors (to be generated by the author from `com.google.android.material:material` 1.12.0, which is in `gradle/libs.versions.toml:59`) |
| TEST-49-09 | Monet `tertiary_container` equals the `tertiaryContainer()` reference (not `tertiary()`) | E-49-07 | same vectors |
| TEST-49-10 | `classify`: 0, 599, 600, 601, 840, 841, 2000 dp | E-49-13 | literal |
| TEST-49-11 | Fast scroller proportion math: list `scroll_offset / (range - height) * track + top` for offset 0, mid, max; clamped | E-49-18 | synthetic `ListLayout` struct |
| TEST-49-12 | Fast scroller `computeScrollOffset` with all-sticky visible items returns the fallback (no panic) | E-49-14 | synthetic items with `sticky:` keys |
| TEST-49-13 | Grid mapping: given `columns = 3`, `total = 30`, drag ratio 0.5 -> item index = 15 (proportional mapping) | E-49-16 | synthetic |
| TEST-49-14 | Repeat schedule: first 20 delays equal `750, 562, 421, 315, 236, 177, 132, 99, 74, 55, 41, 30, 22, 16, 12, 9, 6, 5, 5, 5` | `p1-topic-fast-scroll-and-wheel-picker.md` 9 | literal |
| TEST-49-15 | Repeat: release at t = 1500 ms cancels, total clicks = 3 (0, 750, 1312) | E-49-24 | timer fake (`glib` with virtual time in test, or a pure state machine) |
| TEST-49-16 | Numeric chooser: `step=1, min=0` at 0 minus -> 0; typed "abc1-2" -> 12 (E-49-20 proposal behavior: strip, parse) | E-49-19, E-49-20 | literal |
| TEST-49-17 | Wheel alpha: distance 0 -> 1.0 (clamped from 1.2), distance = single -> 0.2 boundary, distance > single -> 0.2 | `calculateAnimatedAlpha` formula | synthetic viewport 300 px |
| TEST-49-18 | Wheel snapped index = row with max alpha; ties pick the first | `maxBy` tie behavior (Kotlin `maxBy` returns the first max) | synthetic |
| TEST-49-19 | Sheet: drag 55 dp down then release -> open; 57 dp -> closed; fling 130 dp/s -> closed; 120 dp/s -> open | E-49-29, E-49-30 thresholds | literal |
| TEST-49-20 | Sheet: dismiss callback count is 1 after close | E-49-31 | state machine |
| TEST-49-21 | ExtendedFAB timings constants equal `p1-behavior.md` 22 | constants | literal |
| TEST-49-22 | Settings row defaults: steps = (last - first) - 1; `steps` never negative (E-49-32) | | |
| TEST-49-23 | Portal parse: `accent-color` `(0.1176, 0.5333, 0.8980)` -> `0xFF1E88E5` (rounding) | E-49-40 | literal |
| TEST-49-24 | KDE parse: `AccentColor=30,136,229` -> `0xFF1E88E5`; missing -> `None` | §6.2 | fixture kdeglobals text |
| TEST-49-25 | Progress: values 0, 0.5, 1.0, 1.2, -0.1, NaN -> indeterminate, 0.5, 1.0, 1.0, 0.0, indeterminate | E-49-36 | literal |
| MANUAL-49-01 | Light/dark follows the system: toggle in KDE System Settings, app updates without restart | E-49-43 | Plasma 6 session |
| MANUAL-49-02 | Choose each of the 19 schemes and the System option; each applies live; text remains readable (contrast check by eye for Matrix, Nord, Yin & Yang) | | |
| MANUAL-49-03 | Settings rows: keyboard operation (Tab, Space, arrows) on every variant | | |
| MANUAL-49-04 | Fast scroller: drag thumb over a 5000-item list, check no skipping at both ends | R-49-05 | a generated list |
| MANUAL-49-05 | Numeric chooser hold: repeat acceleration matches the schedule within 20 ms | TEST-49-14 | a stopwatch or log timestamps |
| MANUAL-49-06 | Sheet phone width (narrow window) and tablet width (wide window) branches | E-49-28 | resizing the window |
| MANUAL-49-07 | Player ripple color: white in system dark, black in light | `p1-behavior.md` 1.4 | player screen (area 46) |

## 5. Fixtures

- `fixtures/schemes/*.json`: the 18 schemes as JSON (dark/light, 36 base tokens each), generated from `p1-topic-color-token-tables.md`. The generator is `49-theme-and-design-system/p1-tool-scheme_tables.py` (its output is the source of truth for the topic file).
- `fixtures/monet/vectors.json`: seed, dark, contrast, and the 36 reference role values (generated once with the Android library; checked in).
- `fixtures/kdeglobals/*.ini`: AccentColor present; present but malformed; absent.
- `fixtures/portal/*.json`: accent-color tuples, color-scheme values 0/1/2/3.

## 6. Coverage of edge cases by tasks

Every E-49 and R-49 and F-49 ID maps to at least one task in `p1-tasks.md` (verified by the self-check table in that file's footer comment).
