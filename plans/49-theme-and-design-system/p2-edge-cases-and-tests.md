# Area 49 (part 2): edge cases, failure modes, races and tests

Upstream tests: none. `find /home/blackbox/aniyomi-src -path "*src/test*" -name "*.kt"` returns 8 files, and none of them reference the theme, colour schemes, `Surface`, `Slider`, `Localize`, `EmptyScreen`, `Modifier` helpers, `Scrollbar` or `LazyListState`. There are no `*Test.kt` files under `presentation-core/`. Everything in section 4 is new.

Root for citations: `/home/blackbox/aniyomi-src/`.

---

## 1. Edge cases (observed in the Kotlin, with the Rust rule)

### 1.1 Theme preference and scheme resolution
| # | Input or state | Kotlin behaviour (source) | Required Rust behaviour |
|---|---|---|---|
| E1 | `pref_app_theme` holds a name not in the enum (`"FOO"`) | `getEnum` returns the default passed by the caller (`UiPreferences.kt` line 22 to 28) | same: return the Linux default (`GtkTheme`) and do not write back |
| E2 | `pref_app_theme` holds deprecated `DARK_BLUE`, `HOT_PINK` or `BLUE` | enum exists, `titleRes` null, not in `colorSchemes` map, so `TachiyomiColorScheme` is used (`TachiyomiTheme.kt` line 80) | map to the Linux default at load; write back the default once so the picker never sees a deprecated value |
| E3 | `pref_app_theme = MONET` but device has no dynamic colour | Kotlin still renders the Monet branch: `MonetColorScheme` falls to `TachiyomiColorScheme` when no wallpaper seed on SDK 27 to 30, and the picker hides the card (`AppThemePreferenceWidget.kt` line 83 to 86) | Rust: if the portal returns no accent, render the default palette; hide the card |
| E4 | AMOLED enabled, mode Light | `getColorScheme(isDark = false, isAmoled)` returns `lightScheme` untouched (`BaseColorScheme.kt` line 18) | same: no override in light, even if the switch is on |
| E5 | AMOLED enabled, mode System, system dark | darkScheme plus pure black overrides | same |
| E6 | AMOLED toggled while the app is running | `ActivityCompat.recreate` (`SettingsAppearanceScreen.kt` line 93) | Rust: `Theme::reapply()`; no restart. Behaviour change, documented as an improvement |
| E7 | Mode changed to Light while AMOLED is on | switch is disabled (`enabled = themeMode != ThemeMode.LIGHT`), value kept | same; keep the stored value |
| E8 | Scheme XML omits roles (for example `error`) | Compose Kotlin passes its own values for most roles; roles absent in Kotlin take M3 baseline | Rust: fallback from the baseline table (T-49b-004); until then from the Default table. Test that every role has a colour after fallback |
| E9 | 8-digit XML colours (`#D849454E`, `#D8FFFFFF`) | `Color(0xFFD849454E)` in Kotlin (ARGB) | parse as `0xAARRGGBB`; a test asserts alpha 0xD8 |
| E10 | Name typo `tealturqoise` vs `TealTurquoise` | file name only | rename in Rust tables; keep the stored enum name `TEALTURQUOISE` |
| E11 | `values/color_matrix.xml` header comment says Doom | comment only | none (comments are not data) |
| E12 | `Monochrome` `secondaryContainer` `#888888` with `onSecondaryContainer` `#FFFFFF` | observed contrast about 3.5:1 for white on that grey | keep the palette; the contrast test (section 4.3) reports it as a known warning, not a failure |
| E13 | Dynamic (Monet) on Linux without accent, and the user picked it | n/a | fall back to the Default palette; keep the stored value so a later accent change applies |
| E14 | Portal not running (no `xdg-desktop-portal`) | n/a | `SystemScheme::Unknown`; fall back to `gtk::Settings::gtk_application_prefer_dark_theme` |
| E15 | Portal `color-scheme` returns `0` (no preference) | n/a | treat as `Light` unless GTK prefers dark (same as `Unknown`) |
| E16 | `GtkTheme` selected | n/a | provider text is empty; no `@define-color` rules; only the dark preference is applied |

### 1.2 Colour math
| # | Case | Kotlin | Rust rule |
|---|---|---|---|
| C1 | Tint at elevation 0 | returns `surface` exactly (`Surface.kt` line 110) | `tint_at(_, _, 0.0) == surface` (bit-exact, no compositing) |
| C2 | Tint for a custom container colour | not tinted (`surfaceColorAtElevation` checks `color == surface`, line 100) | pass an explicit `tint: bool`; never compare colours |
| C3 | Elevation very large (e.g. 24 dp) | `ln(25) = 3.2189`, alpha `0.1648` | no clamp needed for real elevations; the alpha reaches 1.0 only near 2.9e9 dp |
| C4 | Negative elevation | not possible in M3 (Dp >= 0); `ln(0)` would be -inf | clamp elevation to >= 0 in Rust |
| C5 | `with_alpha(1.5)` | not applicable | clamp to [0,1] |
| C6 | Compositing a translucent surface over translucent | `compositeOver` handles it | `over` implements source-over with `oa = sa + da*(1-sa)`; when `oa == 0` return transparent black |
| C7 | Rounding of composite channels | Compose uses float and rounds on conversion to 8-bit | round-half-up on the float; test with the vectors in section 4.2 |

### 1.3 Slider
| # | Case | Kotlin | Rust rule |
|---|---|---|---|
| S1 | `valueRange` has one element (`5..5`) | `steps = (last - first) - 1 = -1`; M3 requires `steps >= 0` | clamp `steps` to >= 0; a single-value slider is disabled |
| S2 | `valueRange` empty (`5..4`) | `steps = -2` | same as S1; log a debug line |
| S3 | Negative half values | `roundToInt()` on `-2.5` returns `-2` (Math.round, toward +infinity) | Rust: `(x + 0.5).floor() as i32`, NOT `f32::round` (which gives `-3`) |
| S4 | Value outside the range | M3 clamps the float; `roundToInt` of the clamped value | clamp to `[first, last]` before emit |
| S5 | `onValueChangeFinished` without a change | forwarded on every release | emit only if the value changed since press (proposal; avoids a spurious write) |
| S6 | Keyboard step | M3 discrete slider moves by one stop | step 1 |

### 1.4 Scaffold and FAB
| # | Case | Kotlin | Rust rule |
|---|---|---|---|
| F1 | FAB with width or height 0 | `fabPlacement = null` (`Scaffold.kt` line 220); no FAB offset applied | do not reserve FAB space when the button is hidden |
| F2 | FAB present, bottom bar absent | offset = `max(0, bottomInset) + fabHeight + 16` | same with bottom inset 0 on desktop |
| F3 | Snackbar present, FAB absent | snackbar offset = `snackbarHeight + max(bottomBarHeight, bottomInset)` | same |
| F4 | Top bar taller than the screen (expanded app bar) | top bar measured with infinite max height; content top padding = its full height | same; content scrolls under the bar |
| F5 | `startBar` wider than the content | content start padding = start bar width (`max` with inset) | same; the rail never overlaps content |
| F6 | Snackbar wider than the content | `snackbarLeft = (insetWidth - snackbarWidth) / 2 + leftInset` can be negative | clamp to 0 |
| F7 | RTL layout | FAB end offset flips (`Scaffold.kt` line 223 to 228) | GTK handles RTL through `gtk::Widget::direction`; use `halign = end` |

### 1.5 Empty and info screens
| # | Case | Kotlin | Rust rule |
|---|---|---|---|
| EM1 | Empty actions list `[]` | no row (`isNullOrEmpty`) | no button box |
| EM2 | Many actions (> 3) | equal weights, buttons get narrow | cap at 3 in the desktop layout and wrap the rest to a second row (proposal) |
| EM3 | Long message | wraps (centred) | wrap at 60 chars |
| EM4 | Face chosen per recomposition | `remember` keeps it while on screen | choose once per widget; do not reroll on refresh |
| EM5 | `InfoScreen` `canAccept = false` | accept button disabled | sensitive = false |
| EM6 | `rejectText` set but `onRejectClick` null | reject row hidden (`&&`) | same rule |
| EM7 | Content taller than screen | column scrolls | `ScrolledWindow` |

### 1.6 Modifier and focus helpers
| # | Case | Kotlin | Rust rule |
|---|---|---|---|
| M1 | `selectedBackground(true)` in light, system light | alpha 0.22 | alpha from the resolved dark flag, not the theme mode |
| M2 | Enter pressed in a multi-line field | the preview handler consumes Enter (returns `true`) before the field sees it | use the helper only on single-line entries (as the Kotlin doc says) |
| M3 | `showSoftKeyboard(true)` on a screen reopened | `rememberSaveable` keeps `openKeyboard`; first effect requests focus then sets it false | Rust: `grab_focus` once per dialog open |
| M4 | `clearFocusOnSoftKeyboardHide` on desktop | no IME | dropped (T-49b-032) |
| M5 | `secondaryItemAlpha` applied to a disabled item | both apply (`alpha` stacking) | GTK `opacity` multiplies with `:disabled` opacity; set one only |

### 1.7 Lists, scrollbars, FAB expansion
| # | Case | Kotlin | Rust rule |
|---|---|---|---|
| L1 | Empty list in `drawVerticalScrollbar` | `totalSize = 0`, `thumbSize = viewport / 0 * viewport = Infinity` (`Scrollbar.kt` line 120); not drawn because `showScrollbar` is false (items 0 < total 0 is false, itemsSize 0 > viewport false) | no custom bar; GTK handles it |
| L2 | Sticky header is the first visible item | start offset skips items whose key starts with `"sticky:"` (line 124 to 125) | GTK handles it |
| L3 | Reverse direction (RTL) | `reverseDirection` flips horizontal scroll and thumb origin | GTK handles it |
| L4 | FAB rule at exactly index 0, offset 0 | expanded | `value <= 0` expanded |
| L5 | FAB rule after scrolling back a little | `lastScrolledBackward` true, expanded | track previous adjustment value; expanded when it decreased |
| L6 | List cannot scroll | `!canScrollForward` true, expanded | `value + page >= upper - epsilon` |
| L7 | Scroll events of 0 delta | ignored by the scrollbar fade (`delta != 0f`) | ignore 0 deltas in any fade logic |

### 1.8 Localization
| # | Case | Kotlin | Rust rule |
|---|---|---|---|
| I1 | Translated string containing a backslash | `fixed()` replaces each `\` with `"` (`core/common/.../Localize.kt`) | do not replicate; gettext strings are unescaped by gettext itself |
| I2 | Plural with count 0 | `StringDesc.Plural` picks the locale's plural form | `ngettext` with the same count |
| I3 | Missing translation | moko falls back to base strings | gettext returns the msgid (English) |
| I4 | Format args with `%` | moko format | use `gettext` with `format!` on the Rust side; never pass user text as a format string |

### 1.9 Preference observation
| # | Case | Kotlin | Rust rule |
|---|---|---|---|
| P1 | Widget created before the first value | `collectAsState(initial = get())` shows the current value immediately | the Rust subscription emits the current value on subscribe |
| P2 | Preference object identity changes | `remember(this)` recreates the flow | re-subscribe on identity change |
| P3 | Change arrives while the widget is being destroyed | Compose cancels the collector | drop the glib receiver on widget destroy; never call into a dead widget |

### 1.10 Icons and resources
| # | Case | Kotlin | Rust rule |
|---|---|---|---|
| R1 | Icon accessed from two threads | `_discord` cache is not synchronized (Compose main thread only) | GResource icons are loaded on the main thread; `OnceLock` if shared |
| R2 | Icon tint | black fill replaced by `Icon(tint)` | use symbolic SVG (single colour) and `gtk::Image` with CSS `color` |
| R3 | Missing icon file | n/a | `image-missing` fallback icon, logged once |

### 1.11 Theme switch while UI is open
| # | Case | Required Rust behaviour |
|---|---|---|
| T1 | Theme changed while a dialog is open | dialogs inherit the display provider; no reopen needed |
| T2 | Theme changed during an animation | `load_from_string` restyles; transitions continue with new colours |
| T3 | Theme changed while the player is in fullscreen | the player overlay re-reads tokens on the next frame; the mpv OSD colours are not part of the theme (area 45) |
| T4 | Two rapid changes (picker clicks) | `reapply` coalesces through `glib::idle_add_local_once` (single pending call) |
| T5 | Preview cards recompute | each card has its own token set; compute 19 once at picker open, not per frame |

---

## 2. Failure modes and what the user sees

| Failure | Detection | Result |
|---|---|---|
| Malformed CSS generated (bug in `css.rs`) | GTK parse error on stderr | previous CSS stays; UI keeps old colours; log line |
| Portal call hangs | timeout 2 s on the first call (proposal) | `Unknown`; GTK prefer-dark used |
| Portal reports accent but no colour scheme | `color-scheme` missing | treat as `Unknown` |
| gettext catalog missing for the locale | `gettext` returns msgid | English shown |
| Font lacks kaomoji glyphs | Pango check at start (proposal) | ASCII fallback face `:(` |
| Preference store unreadable | area 02 error path | theme defaults used; no crash |

---

## 3. Race conditions

| Race | Mitigation |
|---|---|
| Portal change event arrives during `reapply` | `reapply` is idempotent; the last resolved `Tokens` wins (single `RefCell<Theme>` on main thread) |
| Two `load_from_string` calls in one frame | coalesced by `idle_add_local_once` (T4) |
| Preference write from settings while the portal changes | both go through the main thread; the write then `reapply`s with the latest values |
| Widget destroyed during a portal future | the future holds a weak reference; upgrade fails and returns |
| AMOLED toggled while the picker preview is open | each card recomputes from the current `amoled` flag on change |

Kotlin counterpart: the Compose preview cards recompose on each preference emission; no explicit locking. Rust: single thread, no locks needed.

---

## 4. Tests to write (new)

All are unit or golden tests in the crate (`cargo test`), except the snapshot in 4.6.

### 4.1 Token completeness (golden)
- For each of the 18 `SCHEMES` entries and each mode (light, dark): after fallback, all 38 roles have a colour. Assert `Tokens::colors` has no zero-length entries.
- For each scheme: the set of roles declared in XML equals the list in `p2-topic-theme-tokens.md` section 3. Fails if a table is edited without updating the list.
- Parse test: every XML value in `schemes.rs` is an 8-digit or 6-digit hex; the 8-digit ones are exactly `#D849454E` (light cloudflare, doom, matrix, sapphire) and `#D8FFFFFF` (dark ones), and `#D8FFFFFF` and `#D849454E` as listed in the matrix. Counting: cloudflare, doom, matrix, sapphire light `onSurfaceVariant` = `D849454E`; dark = `D8FFFFFF`.

### 4.2 Colour math vectors
Computed from the Compose formula (these are the expected values; Compose itself is the reference if available on the desktop test runner):

| Case | Surface | Tint | dp | alpha | Expected result |
|---|---|---|---|---|---|
| Default light, 1 dp | `#FEFBFF` | `#0058CA` | 1 | 0.051192 | `#F1F3FC` |
| Default light, 3 dp | `#FEFBFF` | `#0058CA` | 3 | 0.082383 | `#E9EEFB` |
| Default light, 6 dp | `#FEFBFF` | `#0058CA` | 6 | 0.107566 | `#E3E9F9` |
| Default dark, 3 dp | `#1B1B1F` | `#B0C6FF` | 3 | 0.082383 | `#272931` |
| Default dark, 6 dp | `#1B1B1F` | `#B0C6FF` | 6 | 0.107566 | `#2B2D37` |
| Elevation 0 | `#FEFBFF` | `#0058CA` | 0 | n/a | `#FEFBFF` exactly |

Additional assertions: `over` with alpha 0 returns the destination exactly; `over` with alpha 255 returns the source exactly; `with_alpha(0.22)` of `#0058CA` gives alpha byte 56 (0.22 * 255 = 56.1, rounded to 56).

### 4.3 Contrast report (non-failing)
For each scheme: contrast of `onPrimary` on `primary`, `onSurface` on `surface`, `onSurfaceVariant` on `surface`, `onSecondaryContainer` on `secondaryContainer`, `onTertiary` on `tertiary`, and text on `background`. Report pairs below WCAG AA 4.5:1 for normal text as warnings (known: `Monochrome` secondaryContainer `#888888`, see E12). Not a failure; a list for the owner.

### 4.4 AMOLED rule
- Dark + AMOLED: background, surface are `#000000`; onBackground, onSurface `#FFFFFF`; the five container roles `#0C0C0C`, `#131313`, `#1B1B1B` as listed in the behavior file.
- Light + AMOLED: tokens equal the light tokens (no change).
- Dark + no AMOLED: tokens equal the dark table.

### 4.5 Preference parsing
- `"DARK_BLUE"`, `"HOT_PINK"`, `"BLUE"`, `"FOO"`, `""` map to the Linux default.
- `"TEALTURQUOISE"` maps to `AppTheme::TealTurquoise`.
- `"SYSTEM"`, `"LIGHT"`, `"DARK"` map to the three `ThemeMode` values; `"system"` (lower case) maps to the default (case-sensitive, like `enumValueOf`).

### 4.6 CSS snapshot
- Generate CSS for `Default` light, `Default` dark, `Doom` dark with AMOLED, `GtkTheme`. Compare to checked-in golden files. `GtkTheme` must be an empty string.
- Assert CSS length < 20 KB for every scheme.

### 4.7 Component logic
- Slider: `round_half_up(-2.5) == -2`, `round_half_up(2.5) == 3`, `round_half_up(0.49) == 0`; `steps` clamp for `5..5`.
- FAB expansion truth table (3 inputs: at top, scrolled backward, can scroll forward) -> expanded = `at_top || backward || !can_forward`. 8 rows.
- Empty face: list length 12; strings verbatim (including the leading space in the 11th and the missing bracket in the 6th).
- Localize rule: none (Rust does not replicate `fixed()`); test that a string with a backslash is returned unchanged.
- Scrollbar: none (dropped).
- Icons: each SVG parses and has `viewBox="0 0 24 24"`.

### 4.8 Integration (manual, on the user's machine)
- KDE Plasma (Wayland) and X11: switch system light/dark; the app follows within 1 s.
- Tokyo Night GTK theme with `GtkTheme` selected: no visible change from the stock look.
- A custom scheme (Doom) with the portal unavailable (stop `xdg-desktop-portal`): mode System falls back to GTK prefer-dark.
- Font without kaomoji: ASCII fallback face shown.

---

## 5. Fixtures needed
- `tests/fixtures/xml/*.xml`: copies of the 34 XML files (or the generated `schemes.rs` plus a checker script reading the XML at test time when `ANIYOMI_SRC` is set). Prefer copies, so tests run without the Android tree.
- `tests/fixtures/css/*.css`: golden CSS outputs (section 4.6).
- `tests/fixtures/vectors.json`: the colour vectors of section 4.2.
- A fake portal: none needed; the portal reader is behind a trait `SystemSchemeSource` with a test implementation.
