# Area 49b (part 1): edge cases, failure modes, races and tests

All paths repo-relative to `/home/blackbox/aniyomi-src` unless prefixed `/home/blackbox/kurobayomi/`.

## 1. Upstream tests

No `*Test*.kt` file in the repo exercises these resources. Checked with `find . -name "*Test*.kt"` filtered for theme/colour/icon/drawable/resource names (no result), and `grep` for `AppTheme`, `ThemingDelegate`, `ColorScheme`, `getThemeResIds`, `createReaderThemeContext` under `*Test*.kt` and `app/src/test` (no result). Nothing to port. The tests in section 6 are new.

## 2. Edge-case catalogue

Each row: trigger, what the Android code does (source), what Kurobayomi must do.

### 2.1 Theme preference

| # | Trigger | Android behaviour (source) | Kurobayomi behaviour |
|---|---|---|---|
| E1 | Stored `pref_app_theme` is an unknown name (not an `AppTheme` constant) | `getEnum` catches `IllegalArgumentException` and returns the default (`PreferenceStore.kt:35-40`); default is MONET on dynamic-colour devices, else DEFAULT (`UiPreferences.kt:22-29`) | `AppTheme::from_pref` returns `Default`; the stored value is not rewritten (rewrite only on user change). |
| E2 | Stored value is `BLUE`, `HOT_PINK` or `DARK_BLUE` | Deserialises (constants still exist); `themeResources.getOrDefault` returns `Theme_Tachiyomi`; picker hides them (`titleRes == null`) (`AppTheme.kt:29-31`, `AppThemePreferenceWidget.kt:85`) | Same visible result: `Default`. The picker must not list them. |
| E3 | Stored value is `MONET` on Linux (copied from an Android backup) | Android picker hides MONET only when dynamic colour is unavailable; the style is still applied if stored (`TachiyomiTheme.kt:77-78` builds `MonetColorScheme`) | Kurobayomi maps MONET to `Default` (no wallpaper colour exists on Linux). Log once at info level. |
| E4 | AMOLED flag on, light mode | `getColorScheme(isDark=false)` returns the light scheme (`BaseColorScheme.kt:17-19`); XML overlay applies only under `values-night` | `kb-amoled` class applied only when `dark` is true. |
| E5 | AMOLED flag on, dark mode, palette Default | Overlay replaces background and surface with pure black and container greys (`values-night/themes.xml:5-14`) | Same overlay values, via `.kb-dark.kb-amoled` in `style.css`. |
| E6 | Palette chosen, system switches light/dark at runtime | Compose re-reads `isSystemInDarkTheme()` on recomposition (`TachiyomiTheme.kt:77-82`) | GTK: re-run `palette_css` for the new mode on `notify::gtk-application-prefer-dark-theme` and on theme-name change. |
| E7 | Window is dark but `gtk-application-prefer-dark-theme` is false (dark GTK theme such as Tokyo Night without the flag) | Not applicable on Android | Detect dark by luminance of the window background (`theme_bg_color` luminance < 0.5) rather than by the flag alone. Test T5 below. |
| E8 | Reader background set to "automatic" with theme mode SYSTEM | `createReaderThemeContext` uses `applicationContext.isNightMode()` (`ContextExtensions.kt:125-127`) | Reader background class is chosen from the window's dark state at open time. |

### 2.2 Palette data

Token presence per palette (from the XML style items, `values/themes.xml`; "omits" means the style item is absent so the value is inherited from `Base.Theme.Tachiyomi`, which defines `tachiyomi_*` and not `surfaceBright`/`surfaceDim`/`scrimBackground`, which fall through to the Material 3 parent):

| Palette | Items | Omits (inherited value source) |
|---|---|---|
| Base (`Theme.Tachiyomi`, via `Base.Theme.Tachiyomi`) | 76 distinct item names across the whole file; the base style sets 33 colour-role items (`values/themes.xml:14-46`) | sets every role token the palettes use except `colorSurfaceBright`, `colorSurfaceDim`, `scrimBackground` (those fall through to the Material 3 parent) |
| CottonCandy | 22 | `colorSurfaceContainer`, `colorSurfaceContainerLowest`, `colorSurfaceBright`, `colorSurfaceDim`, `colorOutlineVariant`, `colorError`, `scrimBackground`, `elevationOverlayColor` |
| Mocha | 22 | same as CottonCandy |
| Cloudflare | 22 | same as CottonCandy |
| Sapphire | 22 | same as CottonCandy |
| Doom | 22 | same as CottonCandy |
| Matrix | 22 | same as CottonCandy |
| GreenApple | 35 | `elevationOverlayColor` |
| Lavender | 35 | `elevationOverlayColor` |
| StrawberryDaiquiri | 35 | `elevationOverlayColor` |
| MidnightDusk | 28 | `colorSurfaceBright`, `colorSurfaceDim`, `colorOutlineVariant`, `colorError`, `scrimBackground` |
| Tako | 28 | same as MidnightDusk |
| TealTurquoise | 28 | same as MidnightDusk |
| YinYang | 28 | same as MidnightDusk |
| Yotsuba | 28 | same as MidnightDusk |
| TidalWave | 28 | same as MidnightDusk |
| Nord | 32 | `colorSurfaceBright`, `colorSurfaceDim`, `colorError`, `scrimBackground` |
| Monochrome | 34 | `scrimBackground`, `elevationOverlayColor` |
| Monet | 0 (base file) | all (items come from `values-v31`/`values-night-v31` only) |

Edge effects:
- E9: On CottonCandy and Mocha the error colour is the base `tachiyomi_error` (`#BA1A1A`) and the surface containers are the base values. The palette does not look self-consistent (dark CottonCandy error tokens are blue-based). Kurobayomi's `CottonCandy` and `Mocha` palettes define every token in `Palette` (section 2.2 of the design), so this inheritance does not carry over.
- E10: Kurobayomi's `Palette` has no `error`, `surface_container*`, `scrim` fields. The GTK palette override does not set `error`; GTK's own `@error_color` stays in effect (this matches the Compose schemes, which define `error` only via the default `ColorScheme`). Confirm visually when the palette is enabled.

### 2.3 Colour data drift (verified for this plan)

| # | Token | XML (authoritative for Views) | Kotlin Compose | Resolved value on device | Kurobayomi decision |
|---|---|---|---|---|---|
| D1 | `cottoncandy_onTertiary`, dark | not defined in `values-night/colors_cottoncandy.xml`; the light file value `#FFFFFF` applies | `#5F112B` (`CottoncandyColorScheme.kt`, dark `onTertiary`, comment "onTertiary color is assumed") | Views: `#FFFFFF`; Compose: `#5F112B` | `#5F112B` (Compose). Log as a bug in Aniyomi (Views and Compose disagree). |
| D2 | `mocha_inverseOnSurface`, light | `#FBEEEB` (`values/colors_mocha.xml:33`) | `#FBEAEB` (`MochaColorScheme.kt`, light `inverseOnSurface`) | Views: `#FBEEEB`; Compose: `#FBEAEB` | `#FBEAEB` (Compose). |
| D3 | All other 20 tokens of MOCHA_LIGHT, MOCHA_DARK, COTTONCANDY_LIGHT, COTTONCANDY_DARK | equal | equal | equal | match (test T6). |
| D4 | `cottoncandy_elevationOverlay` (both files) | defined, no reference (`grep` finds only the two colour files) | none | unused | drop (no Kurobayomi token). |
| D5 | `cottoncandy_primaryInverse` | defined; used by `values/themes.xml` (`Theme.Tachiyomi.CottonCandy` `colorPrimaryInverse`) | used as `inversePrimary` | equal | keep as `primary_inverse`. |

### 2.4 Values files and qualifiers

| # | Situation | Behaviour | Note |
|---|---|---|---|
| Q1 | API 26 device (minSdk) | `values/` branch: `Theme.Tachiyomi` parent `Base.Theme.Tachiyomi` with `navigationBarColor` = `amoled_surface` (black); `values-v27` not applied | Irrelevant on Linux (no nav bar). Recorded so the drop is deliberate. |
| Q2 | Dark mode on API 27+ | `values-night` wins for `lightStatusBar`, `lightNavigationBar`, `elevationOverlayEnabled` (`values-night/bools.xml`) over `values-v27/bools.xml` | `lightNavigationBar` false in dark (not true from v27). Dropped with the nav bar. |
| Q3 | Tablet (smallest width at least 720 dp) | `screen_edge_margin` 24 dp (`values-sw720dp/dimens.xml:2`) | Kurobayomi: 24 px when window width at least 720 px. Edge: the threshold is the window width, not the screen; a 700 px window on a 1440 px monitor uses 16 px (Proposal, documented). |
| Q4 | `screen_edge_margin` referenced anywhere | no reference in Kotlin or XML (OBSERVED) | the value is still implemented as the layout margin constant (the margin itself is a layout decision, not dead code in Kurobayomi). |
| Q5 | Widget radius on API 31+ | `presentation-widget/src/main/res/values-v31/dimens.xml` sets `appwidget_background_radius` to the system value, but the app module's `values/dimens.xml:4` (16dp) overrides it at merge time | Widget only; out of Linux scope. Recorded because it is a real merge-order defect. |
| Q6 | `values/dimens.xml` widget radii duplicate `presentation-widget/src/main/res/values/dimens.xml` | identical values (16, 12) | duplicate; Kurobayomi drops both (no widget). |

### 2.5 Animated vectors

| # | Situation | Android behaviour | Kurobayomi behaviour |
|---|---|---|---|
| A1 | Rapid toggling of `atEnd` (for example expand/collapse twice within 300 ms) | Compose reverses from the current interpolated state | GTK `Stack` crossfade interrupts the running transition and starts from the current opacity (GTK behaviour); CSS transitions also reverse from the current value. Same visible result. |
| A2 | `anim_more_enter` `time_group` has no children | the animator runs but moves nothing | Dropped (P3). |
| A3 | Animation with `valueFrom == valueTo` (none found in scope with identical endpoints, checked for all objectAnimators; `anim_browse_enter` scales are 0 to 0 then 0 to 1, intentional) | n/a | n/a |
| A4 | `anim_more_enter` dot schedules leave a 1 ms gap (316 to 317 ms) | visually none | not reproduced (the morph is dropped). |
| A5 | Play/pause morph while the `paused` state flips during the 300 ms animation | Compose plays the reverse | Crossfade reverses; no stuck state. |
| A6 | Reduced-motion users | Android does not check `ANIMATOR_DURATION_SCALE` in these files | Proposal: honour GTK's `gtk-enable-animations` setting; when false, set transition durations to 0 (behaviour unchanged otherwise). |

### 2.6 Icons

| # | Situation | Behaviour | Kurobayomi behaviour |
|---|---|---|---|
| I1 | Icon name not found in the theme | GTK shows the missing-icon placeholder | `icons::register` runs before the window; test T2 asserts every `Icon` resolves. |
| I2 | Symbolic SVG without `#bebebe` fill | GTK does not recolour; icon stays black on dark themes | Export script asserts `#bebebe` presence (T7). |
| I3 | Icon in RTL layout | Android: `drawable` not mirrored unless `autoMirrored` (none set in scope) | Verify on GTK 4.16 how directional icons mirror (GTK uses a `-rtl` icon variant when the theme provides one). Reader LTR/RTL icons (`kb-reader-ltr`, `kb-reader-rtl`) are semantic and must never be mirrored, so they carry no mirroring flag. |
| I4 | Full-colour launcher icon used as a symbolic icon | n/a | Keep full colour for the app icon; use `LogoSymbolic` for in-window logo. |
| I5 | Monochrome launcher icon | Android 13 themed icons only | Kurobayomi uses the symbolic logo only; the monochrome launcher is not installed. |
| I6 | Unused drawable referenced by a Compose `painterResource` in code outside this scope | no such reference found (`grep` for each of the 13 dead drawables returned zero matches in `*.kt`, `*.java`, `*.xml`) | Dropped. |
| I7 | Two Android drawables referenced by the same notification slot | n/a | Use the `Icon` enum to pick exactly one name per call site. |

### 2.7 Splash timing

| # | Situation | Behaviour | Kurobayomi behaviour |
|---|---|---|---|
| S1 | `ready` becomes true at 300 ms | keep-on-screen returns false at 500 ms (the minimum) | splash page hides at 500 ms, not 300 ms |
| S2 | `ready` never becomes true | keep-on-screen returns false at 5000 ms | splash hides at 5000 ms even without the ready signal. |
| S3 | `ready` is produced only by the Browse tab first composition, and Browse is not composed at launch (for example if the start tab is another tab) | `ready` stays false and the splash waits for `SPLASH_MAX_DURATION` (5000 ms) | Kurobayomi's ready signal must come from the first main-screen build, not from one tab, so this cannot happen. |
| S4 | Activity recreated (rotation) | `isLaunch` false, no splash | GTK window recreation does not re-show the splash (the splash is a page in the main window, shown once per process). |
| S5 | Pre-Android-12 exit animation runs while `root.translationY` is changing | content slides from +16 dp | Kurobayomi: optional (P3). |
| S6 | An action from `--action=` arrives during the splash | Android: `onNewIntent` route handled after `ready` (the intent is consumed by the nav, which requires composition) | Kurobayomi: queue the action until the main page is visible, then `activate-action`. |
| S7 | Splash colour in dark mode | `values-night/colors.xml`: `splash` = `#202125` | `.kb-splash.kb-dark` class. |

### 2.8 Double-tap seek triangles

| # | Situation | Behaviour | Kurobayomi behaviour |
|---|---|---|---|
| T-a | Double tap on the left then the right edge within the cycle | Two composables with `isForward` differing; each runs its own loop | Two overlays; each owns its tick callback; removing one does not affect the other. |
| T-b | Overlay removed mid-cycle | `LaunchedEffect` cancels the coroutine; alpha values are discarded | `ControlFlow::Break` removes the tick; the next show starts at `t = 0`. |
| T-c | Rotation for backward seek | `Modifier.rotate(180f)` | `-gtk-icon-transform: rotate(180deg)` on the container (or the three images). |

### 2.9 Shortcuts and desktop actions

| # | Situation | Behaviour (source) | Kurobayomi behaviour |
|---|---|---|---|
| H1 | `shortcutId` duplicated (`show_recently_updated` for the history shortcut, `app/shortcuts.xml:27` and `:38`) | Android keeps one shortcut per id; the history entry can be replaced | Each `.desktop` action has a unique name; no collision. |
| H2 | Dynamic shortcuts (`SHORTCUT_MANGA`, `SHORTCUT_ANIME`, downloads) are not in `shortcuts.xml` | routed in `MainActivity.kt:472-489` from notifications | Kurobayomi handles `--action=show-manga <id>` and `--action=show-anime <id>` (Proposal) with the same routes; the desktop file lists only the five static actions. |
| H3 | Intent action unknown | `when` falls through to other branches (not in scope) | Unknown `--action` value: print usage to stderr, exit code 2. |
| H4 | Action arrives while the app is already running | Android delivers via `onNewIntent` | `gtk::Application` `command-line` signal forwards the action to the primary instance (Proposal). |

### 2.10 Colour arithmetic

| # | Situation | Behaviour | Kurobayomi behaviour |
|---|---|---|---|
| C1 | `#1F888888` cover placeholder alpha | 0x1F = 31; 31/255 = 0.12157 | CSS `rgba(136, 136, 136, 0.1216)`; test T8 checks the rounding. |
| C2 | `#1F000000` divider | alpha 0.12157 black | CSS `alpha(@theme_fg_color, 0.12)` (theme-relative, not black; intentional). |
| C3 | `alpha 0.08` drag overlay | from `draggable_card_foreground.xml` | `alpha(@theme_selected_bg_color, 0.08)`. |
| C4 | `backgroundDimAmount` 0.32 | fraction 0.32 of black | `rgba(0,0,0,0.32)`. |
| C5 | ARGB parsing of `#AARRGGBB` in palette constants | `Palette` fields are full ARGB; alpha is ignored by `hex6` | Test T4. |

## 3. Failure modes and error handling

- CSS parse error (undefined `@name`, bad syntax): GTK emits `parsing-error` and drops the offending rule. Kurobayomi connects `parsing-error` on each `CssProvider` and prints the location to stderr; startup continues with the rest of the stylesheet (no panic). Test T1 makes this a failing test.
- Missing GResource file: `load_from_resource` logs and leaves the provider empty; the app still runs unstyled. Test T1 also asserts the resource path exists (`gio::resources_lookup_data`).
- Icon registration failure (path not added): GTK falls back to the theme's own icons (`image-missing`); test T2.
- `palette_css` for `Default` produces an empty string; `load_from_string("")` is valid.
- Splash timer: `glib::timeout_add_local` returns a `SourceId`; the state machine clears it on hide. Dropping the window before the timer fires must not panic: store the id and remove it in the window's `close-request` handler.
- Reader double-tap timer: the tick callback returns `Break` when the overlay is unmapped (checked in the callback), so no leak.

## 4. Races and ordering

- Palette and AMOLED: `theme::install` must run before the first `present()`; otherwise the first frame uses the theme without the palette (flash of default colours). Order in `build_ui`: `icons::register`, `theme::install`, then build widgets, then `present`.
- Dark-state detection vs palette: when both change in one event (switching GTK theme changes both the background and the dark flag), apply the palette once, after reading the final background (avoid two reloads).
- Splash vs `--action`: see S6. Action is queued in a `RefCell<Option<Action>>` on the application; the main page drains it once.
- Stack transitions: calling `set_visible_child_name` during a running crossfade is allowed (GTK interrupts); no lock needed.
- Thread safety: all of this runs on the GTK main thread; `CssProvider` and `Image` are not `Send`. No `tokio` task touches them.

## 5. Fixtures needed

| Fixture | Purpose | Location (proposed) |
|---|---|---|
| The 4 scope colour files (`colors_cottoncandy.xml`, `colors_mocha.xml`, both qualifiers) copied verbatim | source for `tools/check_palette.py` (test T6) | `tests/fixtures/android/` |
| `values/themes.xml` (copy) | palette-token matrix test (T9) | `tests/fixtures/android/themes.xml` |
| The 18 drawable files that are exported to SVG | export test (T7) | `tests/fixtures/android/drawable/` |
| A dark GTK theme directory (Tokyo Night) | detection test (T5); not committed (user's own theme) | local only |
| An Xvfb display (`xvfb-run -a`) | display-dependent tests T1, T2 | CI or local |
| Compose scheme dump (Kotlin → text) of `MochaColorScheme`, `CottoncandyColorScheme` | drift test (T6) | `tests/fixtures/android/colorscheme/` |

## 6. Tests to write (new)

- T1 (`tests/css_parses.rs`, Xvfb): load `data/style.css` and `palette_css` for each palette; `parsing-error` never fires; the resource `/io/github/anas1412/Kurobayomi/style.css` exists.
- T2 (`tests/icons_resolve.rs`, Xvfb): after `icons::register`, every `Icon` variant's `name()` has `IconTheme::has_icon == true`. Fails if an export is missing or misnamed.
- T3 (`src/theme.rs`, unit): `AppTheme::from_pref` for all 22 Android enum names (`p1-data-and-config.md` 2.1): `DEFAULT`→Default, `COTTONCANDY`→CottonCandy, `MOCHA`→Mocha, every other name (including `MONET`, `BLUE`, `HOT_PINK`, `DARK_BLUE`, empty string, lowercase `mocha`)→Default. Unknown names must not panic.
- T4 (`src/theme.rs`, unit): `hex6(0xFF745B00) == "745B00"`; `hex6(0x1F888888) == "888888"`.
- T5 (`src/theme.rs`, unit with a fake colour): dark detection returns true for background `#1E1B16` and false for `#FFFBFF`, independent of the prefer-dark flag.
- T6 (`tools/check_palette.py`, Python 3 stdlib): parse the four XML colour files; compare each token to the Rust constant text in `src/theme.rs`; assert equality except the two documented corrections (D1 `cottoncandy_dark.on_tertiary`, D2 `mocha_light.inverse_on_surface`); assert the XML-vs-Compose list in section 2.3 is exactly those two.
- T7 (`tools/check_svg.py`): every `data/icons/**/kb-*-symbolic.svg` contains `fill="#bebebe"` and no `fill="#FFF`; the viewBox is present.
- T8 (`src/theme.rs`, unit): cover placeholder alpha: `round(0x1F / 255 * 10000) / 10000 == 0.1216`.
- T9 (`tools/check_themes.py`): from `values/themes.xml`, the set of palette names that define `colorSurfaceContainer` equals the list in section 2.2 (catches a changed inheritance table).
- T10 (`src/anim.rs`, unit): `seek_alpha` at t=0, 149, 150, 299, 300, 449, 450, 600, 750, 899, 900 ms for triangle 0 equals the schedule (row 0 end at 150 ms is `1.0`; at 900 ms the cycle restarts at `0.0`). Also `cubic_bezier(0.4,0,0.2,1,0.5)` is between 0.5 and 1.0.
- T11 (`src/splash.rs`, unit with injected clock): ready at 300 ms → hide at 500 ms; ready never → hide at 5000 ms; ready at 6000 ms → hidden at 5000 ms; `elapsed == 500` boundary keeps the splash (condition is `<=`).
- T12 (`src/icons.rs`, unit): no two `Icon` variants share a `name()` unless intended (the table in the design doc lists the only shared name `image-x-generic-symbolic` for `Photo` and `Cover`).
- T13 (`tests/desktop_file.rs`): `data/io.github.anas1412.Kurobayomi.desktop` passes `desktop-file-validate` (system tool) and has five unique `[Desktop Action]` names.
- T14 (manual, documented): on Tokyo Night, confirm the default theme shows no palette override (`theme_bg_color` equals the Tokyo Night value); with CottonCandy/Mocha the window background equals the palette `background`.

## 7. Acceptance checks that are manual (screenshots)

- Compare the Mocha palette dark and light against an Android screenshot of the same screen (library, entry details, reader). Only the two documented token corrections may differ.
- Compare the splash timing with a stopwatch: about 500 ms when the library is ready, 5000 ms worst case.
- Compare the double-tap triangle cycle against a 30 fps capture: six steps of 150 ms.
