# Area 49: Themes and design system (overview)

Area id: `49-theme-and-design-system`. Source root: `/home/blackbox/aniyomi-src` (repo-relative paths below). Target: the `kurobayomi` crate (`/home/blackbox/kurobayomi`, gtk4 0.11.5 with feature `v4_16`, no libadwaita, stable Rust, edition 2024).

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

- Theme selection and resolution. `TachiyomiTheme` (`app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt`, lines 37-48) reads two preferences from `UiPreferences` (`appTheme`, `themeDarkAmoled`) and hands them to `BaseTachiyomiTheme` (same file, line 59).
- 19 selectable `AppTheme` values (`app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt`): `DEFAULT`, `MONET` (dynamic, runtime-generated), and 17 static schemes (`CLOUDFLARE`, `COTTONCANDY`, `DOOM`, `GREEN_APPLE`, `LAVENDER`, `MATRIX`, `MIDNIGHT_DUSK`, `MOCHA`, `SAPPHIRE`, `NORD`, `STRAWBERRY_DAIQUIRI`, `TAKO`, `TEALTURQUOISE`, `TIDAL_WAVE`, `YINYANG`, `YOTSUBA`, `MONOCHROME`). Three deprecated constants (`DARK_BLUE`, `HOT_PINK`, `BLUE`) have no title and are hidden. The 18 static schemes are `DEFAULT` plus the 17 above.
- Material 3 color roles (36 base roles) per scheme, in Kotlin under `app/src/main/java/eu/kanade/presentation/theme/colorscheme/` (20 files: `BaseColorScheme.kt`, `MonetColorScheme.kt` and 18 scheme objects).
- Color values also exist as Android XML under `presentation-core/src/main/res/values*/colors_*.xml` and `app/src/main/res/values*/` (34 XML files in part 2 scope).
- Presentation components (`presentation-core/src/main/java/tachiyomi/presentation/core/components/`): settings rows, dialogs, sheets, fast scrollers, wheel pickers, badges, pills, FAB, progress indicators, empty and loading screens, sections. Material wrappers live in `components/material/`.
- Window size classes (`app/src/main/java/mihon/core/designsystem/utils/WindowSize.kt`) and a paging helper (`presentation-core/src/main/java/mihon/presentation/core/util/PagingDataUtil.kt`).
- Icons and localization helpers (`presentation-core/src/main/java/tachiyomi/presentation/core/i18n/Localize.kt`, `presentation-core/src/main/java/tachiyomi/presentation/core/icons/`).

### 1.2 Why it matters for a usable Kurobayomi

- Every other screen is built from these components. Without the theme engine, the settings rows, the empty and loading states, the badges and the fast scroller, the other areas have nothing to call.
- The user runs Tokyo Night everywhere (see the user's memory note "Match the current theme"). The Linux port must therefore keep the user's GTK theme in control by default. Aniyomi's own palettes remain available as explicit overrides.
- Scope split: part 1 (`p1-*.md`, 50 scope files) covers the theme engine, color schemes, window classes and the Compose component library. Part 2 (`p2-*.md`, 58 scope files) covers the presentation-core Material wrappers, layout widgets, icons, screens, utilities and the XML color resources. The two parts together are 108 scope files.

### 1.3 Out of scope here

- Screens that consume these components (owned by areas 37 to 48).
- Preference storage (area 02) and app bootstrap (area 01).
- Monet color generation is documented here; the Android wallpaper API is not ported.

---

## 2. Module maps

### 2.1 Kotlin module map (packages to responsibilities)

| Package or directory (repo-relative) | Responsibility |
|---|---|
| `app/src/main/java/eu/kanade/presentation/theme/` | `TachiyomiTheme`, `BaseTachiyomiTheme`, the theme entry point and AMOLED handling |
| `app/src/main/java/eu/kanade/presentation/theme/colorscheme/` | 18 static `ColorScheme` objects, `BaseColorScheme` (shared builder and AMOLED override), `MonetColorScheme` (dynamic) |
| `app/src/main/java/eu/kanade/domain/ui/model/` | `AppTheme` enum (19 selectable, 3 deprecated) and its title resource keys |
| `app/src/main/java/mihon/core/designsystem/utils/` | `WindowSize.kt`: `isMediumWidthWindow()` |
| `presentation-core/src/main/java/tachiyomi/presentation/core/components/` | Application-level components: settings items, dialogs, sheets, fast scroller, wheel picker, badges, pills, FAB wrappers, progress, sections, list headers |
| `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/` | Material 3 wrappers: `AlertDialog`, `Button`, `FloatingActionButton`, `IconButtonTokens`, `IconToggleButton`, `Constants`, `Scaffold`, `Surface`, `Slider`, `Tabs`, `NavigationBar`, `NavigationRail`, `PullRefresh` |
| `presentation-core/src/main/java/tachiyomi/presentation/core/util/` | Modifiers, elevation, scrollbar, `LazyListState` helpers, paddings, `Typography` helpers |
| `presentation-core/src/main/java/tachiyomi/presentation/core/i18n/` | `Localize.kt`: string resource helpers |
| `presentation-core/src/main/java/tachiyomi/presentation/core/icons/` | Brand and extension icon vectors (Discord, GitHub, magnet and similar) |
| `presentation-core/src/main/java/tachiyomi/presentation/core/screens/` | `EmptyScreen`, `LoadingScreen`, `InfoScreen` |
| `presentation-core/src/main/res/values*/color_*.xml`, `colors_*.xml` | Android color tokens for the 18 static schemes, Monochrome and the shared colors |

### 2.2 Proposed Rust module map (Kurobayomi)

Two part documents propose slightly different layouts. This section records the reconciled map that the overview recommends. Each part document must be aligned to it (see section 4, decision D-6).

```
src/
  main.rs                 (existing) calls theme::Theme::install() after the window is realized
  theme/
    mod.rs                public items: Theme (runtime state), ThemeSettings, install(), apply()
    tokens.rs             Rgba, Role (36 M3 roles plus XML-only aliases), Tokens, compositing helpers, spacing, alpha, durations, radii, breakpoints
    schemes.rs            generated SCHEMES table for the 18 static schemes (generator in tools/, output byte-identical on re-run)
    resolve.rs            resolve(app_theme, mode, amoled, system_dark) -> Tokens, Compose parity including AMOLED override
    monet.rs              seed source (portal accent or KDE fallback), HCT and SchemeContent port or a vetted crate
    elevation.rs          tint_at(surface, surface_tint, dp) -> Rgba (Surface.kt formula)
    css.rs                Tokens -> GTK CSS text (named colors and component classes)
    apply.rs              single CssProvider lifecycle on the default Display, priorities
    portal.rs             XDG Settings portal watcher (color-scheme, accent-colour)
    prefs.rs              typed read and write of AppTheme, ThemeMode and amoled over the area 02 store
  ui/
    mod.rs                re-exports; existing ui.rs API kept
    helpers.rs            selected_background, secondary_alpha, clickable_no_indication, enter_activates, focus_on_open, Insets
    scroll.rs             should_expand_fab(ScrollSnapshot) -> bool
    surface.rs            clickable surface (GestureClick, GestureLongPress)
    empty_state.rs        EmptyScreen equivalent, ERROR_FACES
    info_screen.rs        InfoScreen equivalent
    loading.rs            LoadingScreen equivalent
    slider.rs             IntSlider over gtk::Scale
    tabs.rs               tab label with count pill
    scaffold.rs           gtk::Overlay layout: content, top bar, bottom bar, FAB, snackbar
    refresh.rs            refresh button and F5 and Ctrl+R accelerators (PullRefresh replacement)
    theme_picker.rs       settings widget: mode row and the 19 preview cards
    widgets/              one file per remaining Compose component (settings_items.rs, fast_scroller.rs, wheel_picker.rs, adaptive_sheet.rs, progress_ring.rs, badge.rs, dialog_content.rs, button.rs, extended_fab.rs and the rest listed in p1-inventory.md)
  i18n.rs                 gettext wrapper: t(), tn(), fixed() rule
  icons.rs                GResource-backed SVG icons (discord, github, magnet)
assets/icons/             discord.svg, github.svg, magnet.svg; icons.gresource.xml
po/                       gettext catalogs generated from the Aniyomi strings keys
build.rs                  glib_build_tools::compile_resources for assets/icons.gresource.xml
```

Existing mockup files that stay: `src/clamp.rs` (AdwClamp replacement, unchanged), `src/ui.rs`, `src/screens.rs`, `src/settings.rs`, `src/entry.rs`, `src/player.rs`, `src/spec.rs`. They keep the stock GTK style classes (`suggested-action`, `pill`, `osd`, `toolbar`, `circular`, `card`, `boxed-list`, `navigation-sidebar`, `dim-label`, `caption-heading`, `heading`, `title-1..4`, `accent`, `monospace`, `linked`). Proposal: add `.kb-*` classes only where no GTK equivalent exists (badges, pills, section cards, FAB, sheet). Do not restyle existing widgets to a fixed palette.

---

## 3. Reading guide

Every document is in `/home/blackbox/kurobayomi/plans/49-theme-and-design-system/`.

| Document | One-line description |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Part 1 file inventory (theme engine, window classes, paging, components, Material wrappers, build files) with port decisions and task IDs |
| [p1-behavior.md](p1-behavior.md) | Part 1 observed behavior: theme resolution, color role fallback, Monet generation, component algorithms |
| [p1-data-and-config.md](p1-data-and-config.md) | Part 1 enums, preference keys, storage (`getEnum`), constants and config |
| [p1-rust-design.md](p1-rust-design.md) | Part 1 Rust design: module layout, public API, core types, CSS generation |
| [p1-ui-parity.md](p1-ui-parity.md) | Part 1 visible behavior of the theme picker and components, state by state, with the mockup gap |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Part 1 edge cases (`E-49-NN`, `F-49-*`, `R-49-*`) and tests (`TEST-49-NN`) |
| [p1-tasks.md](p1-tasks.md) | Part 1 implementation tasks (`T-49a-*`) |
| [p1-topic-color-token-tables.md](p1-topic-color-token-tables.md) | Exact `Color(0xAARRGGBB)` values for the 18 static schemes, generated from Kotlin |
| [p1-topic-fast-scroll-and-wheel-picker.md](p1-topic-fast-scroll-and-wheel-picker.md) | Fast scroller and wheel picker algorithms, thumb geometry and value math |
| [p1-tool-scheme_tables.py](p1-tool-scheme_tables.py) | Script that generates `p1-topic-color-token-tables.md` from the Kotlin sources (run from `plans/`) |
| [p2-inventory.md](p2-inventory.md) | Part 2 file inventory (Kotlin Material wrappers, icons, screens, utils, 34 XML color files) with port decisions |
| [p2-behavior.md](p2-behavior.md) | Part 2 observed behavior: theme pipeline, Scaffold, slider, surface, refresh, scrollbar, localization |
| [p2-data-and-config.md](p2-data-and-config.md) | Part 2 data models: `AppTheme`, the 36 M3 color roles, preference keys, resource and config values |
| [p2-rust-design.md](p2-rust-design.md) | Part 2 Rust design: theme tokens, resolve, elevation tint, CSS provider, portal watcher |
| [p2-ui-parity.md](p2-ui-parity.md) | Part 2 visible behavior of screens and components, with desktop equivalents |
| [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md) | Part 2 edge cases, races and tests (contrast test, XML drift check) |
| [p2-tasks.md](p2-tasks.md) | Part 2 implementation tasks (`T-49b-*`) |
| [p2-topic-theme-tokens.md](p2-topic-theme-tokens.md) | Token model, XML versus Kotlin drift, scheme coverage gaps and recorded decisions |
| [p2-topic-layout-widgets.md](p2-topic-layout-widgets.md) | GTK4 designs for Scaffold, FAB, sidebar, surface, slider, refresh and scrollbar replacements |

---

## 4. Key design decisions and open questions

Each item needs the user's call unless marked "recorded". The recommendation is the default if the user does not object.

| ID | Question | Options | Recommendation |
|---|---|---|---|
| D-1 | Linux default theme | (a) Aniyomi `Default` scheme (Material blue); (b) `Monet` (needs Android wallpaper API, not available); (c) new Linux-only `GtkTheme` (no color overrides, follows the GTK theme) | (c) `GtkTheme`, because the user runs Tokyo Night everywhere. Recorded as decision T-49b-006. The 18 static schemes stay as explicit overrides. Needs the user's confirmation. |
| D-2 | Dynamic (Monet) source on Linux | (a) XDG Settings portal `accent-colour`; (b) KDE color scheme read from `kdeglobals`; (c) hide the option | (a) with (b) as fallback when the portal is absent. Hide the option when neither is available (matches Aniyomi, which hides Monet without dynamic color). Tasks T-49a-005, T-49b-005. |
| D-3 | HCT and SchemeContent implementation | (a) vetted crate (evaluate `material-colors` first, per p1-behavior section 3); (b) port `material-color-utilities` by hand | (a) if the crate matches the reference outputs in the test vectors; otherwise (b). The decision is part of T-49a-006. |
| D-4 | Monochrome light `surfaceDim` | XML value versus Kotlin `#FFFFFF` | Use the Kotlin value `#FFFFFF`, because that is what Android users see today. Recorded in p2-topic-theme-tokens.md section 6 (task T-49b-041). |
| D-5 | Low-contrast palettes | Keep Aniyomi palettes unchanged, or adjust | Keep unchanged. The contrast test reports them and does not fix them. Changing a palette is a product decision (p2-topic-theme-tokens.md section 5). |
| D-6 | Rust module layout | The two part documents disagree (p1-rust-design section 1 names `scheme.rs`, `schemes/`, `dark.rs`, `hct.rs`; p2-rust-design section 1 names `schemes.rs`, `resolve.rs`, `elevation.rs`, `apply.rs`, `portal.rs`) | Adopt the reconciled map in section 2.2 of this overview. Both part documents must be updated before implementation starts. This is a documentation fix, not a product decision. |
| D-7 | Android-only colors and assets (cover placeholder `#1F888888`, notification accent `accent_blue`, AMOLED values) | (a) port all; (b) port only what a Linux surface uses | (b). Keep the values that are used (T-49b-040). The notification accent is reused by the desktop notification path (area 35). |
| D-8 | Pull-to-refresh | Replace with a refresh button and accelerators, or drop | Replace (T-49b-019): a refresh button, F5, Ctrl+R. Nothing is lost except the touch gesture. |
| D-9 | Scrollbar and fast scroller | GTK overlay scrolling only, or a custom fast scroller | GTK overlay scrolling for all lists (T-49b-036 drops the Compose scrollbar). Whether the Aniyomi fast scroller (`VerticalFastScroller`, p1-topic-fast-scroll-and-wheel-picker.md) is ported is a P1 task decision; default is to port it for grids only. |
| D-10 | Bottom navigation bar (`NavigationBar`) | Drop, or keep as a bottom bar | Drop (T-49b-018). Phone layout only; the desktop uses the sidebar (T-49b-017). |
| D-11 | Tab count pill, badges and the other visual extras | Keep as CSS classes | Keep. They are small and map to `.kb-*` classes. |

### 4.1 Task-graph inconsistencies to resolve before implementation

Both tasks files lint clean (`tasks.py lint` prints `ok`). The following graph issues are not lint errors but must be decided:

- Duplicate work. `T-49a-004` (P1, L, "Port the 18 static color schemes as full token tables") and `T-49b-002` (P0, M, "Port the 18 static scheme tables from the XML") port the same tables. Recommendation: keep `T-49b-002` as the single owner and fold the `T-49a-004` acceptance into it.
- Duplicate preference work. `T-49a-001` (P0) and `T-49b-006` (P0) both own preference loading and defaults. Recommendation: keep `T-49b-006` for keys and defaults and make `T-49a-001` a typed accessor that depends on it.
- Duplicate Monet work. `T-49a-005`/`T-49a-006` (P1) and `T-49b-005` (P1) cover the same seed and palette. Recommendation: merge into `T-49a-005`/`T-49a-006` and drop the separate Linux-specific wording in `T-49b-005`.
- Priority inversions (P0 depends on P1 or P2). These are the P0 tasks that cannot finish until a lower-priority task is done:
  - `T-49a-002`, `T-49a-003` and `T-49a-059` depend on `T-49a-004` (P1).
  - `T-49a-038` depends on `T-49b-020` (P1, integer slider over `gtk::Scale`).
  - `T-49a-059` also depends on `T-49a-006` (P1) and `T-49a-048` (P2, wheel picker).
  - `T-49a-064` depends on `T-49a-005` (P1).
  - `T-49b-025` and `T-49b-043` depend on `T-49b-023` (P1, gettext with plural support).
  Recommendation: promote those prerequisites to P0 (they are on the path to the theme engine and the empty and loading states), or demote the dependents. Either is the user's call; the default is to promote.

Already fixed in this pass: `T-49a-038` previously depended on its own area (`area:49-theme-and-design-system`). It now depends on `T-49b-020`. The part 1 prose line that named the part 2 area id now refers to the `T-49b-*` IDs.

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | Used for | Tasks in this area that need it |
|---|---|---|
| `area:01-foundation-app-bootstrap` | App init, window realization, logging hook | T-49a-011 (system dark detection and live updates), T-49a-005 (Monet seed sources), T-49a-021 (window size classes) |
| `area:02-preferences-system` | Typed preferences, `getEnum` semantics, `changes()` stream | T-49a-001, T-49b-006, T-49b-035 (preference observation bridge) |
| `area:46-player-controls-ui` | Player control surface that the ripple state layer styles | T-49a-010 (player ripple state layer, CSS) |
| `area:47-settings-screens-general` | Appearance screen that hosts the theme picker | T-49a-062 (theme picker preview cards) |

Areas 46 and 47 appear here because the tasks listed above depend on them; the links point at the screens the tasks style or host.

### 5.2 What others need from this area

- Every screen area (37 to 48) calls the components documented here: `EmptyScreen`, `LoadingScreen`, `InfoScreen`, `SettingsItems` rows, `ListGroupHeader`, `SectionCard`, `Pill`, `BadgeGroup`, `FastScrollLazyVerticalGrid`, `AdaptiveSheet`, `WheelNumberPicker`, `ExtendedFloatingActionButton`, `Scaffold`, `Surface`, `Slider`. Tasks: T-49a-024, T-49a-025, T-49a-033, T-49a-035, T-49a-036 to T-49a-039, T-49a-045, T-49a-049, T-49b-016, T-49b-020, T-49b-021, T-49b-025, T-49b-027.
- Area 34 (images and covers) needs the cover placeholder color (`#1F888888`, T-49b-040; callers list at the end of `p2-behavior.md`).
- Area 35 (notifications) needs the `accent_blue` notification color (callers list at the end of `p2-behavior.md`).
- Area 36 (shell and navigation) needs the sidebar geometry (T-49b-017) and the window breakpoints (T-49b-042, medium 600, expanded 840).
- Areas 43 to 46 (reader and player) need `Slider`, the player ripple configuration (T-49b-013) and `Surface`.

---

## 6. Task summary

### 6.1 Counts (from `TASKS.csv`, rows whose file starts with `49-theme-and-design-system/`)

Total: 100 tasks (part 1 `T-49a-*`: 57; part 2 `T-49b-*`: 43).

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 25 | 7 | 0 | 0 | 32 |
| P1 | 24 | 17 | 3 | 0 | 44 |
| P2 | 16 | 1 | 0 | 0 | 17 |
| P3 | 7 | 0 | 0 | 0 | 7 |
| **Total** | **72** | **25** | **3** | **0** | **100** |

Part split by priority: part 1 has 20 P0, 26 P1, 8 P2, 3 P3 (57). Part 2 has 12 P0, 18 P1, 9 P2, 4 P3 (43).

### 6.2 P0 critical path (in order)

Part 2 owns the spine. Tasks in brackets can run in parallel where marked.

1. `area:02-preferences-system` (preference store), needed by `T-49b-006`.
2. `T-49b-001` Theme token model (S, no dependencies).
3. `T-49b-002` Port the 18 static scheme tables (M). Needs T-49b-001.
4. `T-49b-003` Scheme resolution, dark flag and AMOLED override (S). Needs T-49b-001 and T-49b-002.
5. `T-49b-006` Preference keys, defaults and the Linux default decision (S). Needs area 02. Decision D-1 is recorded here.
6. `T-49b-007` Theme mode on GTK and the portal (S). Needs T-49b-006.
7. `T-49b-008` Generated GTK CSS and one `CssProvider` lifecycle (M). Needs T-49b-001, T-49b-003 and T-49b-007. This is the central task.
8. In parallel after T-49b-008: `T-49b-027` loading page (S), `T-49b-028` opacity and padding constants (S), `T-49b-025` empty state (S, also needs T-49b-023, a P1 task; see section 4.1), and `T-49b-010` active filter tint (S, needs T-49b-003), `T-49b-029` selected row tint (S, needs T-49b-003).
9. `T-49b-043` `AppTheme` enum port (S). Needs T-49b-023 (P1) and T-49b-006.

Part 1 spine, also P0:

1. `T-49a-013` freeze M3 baseline values and `T-49a-051` spacing, alpha, duration and radius tokens (both S, no dependencies, parallel).
2. `T-49a-045` settings row base and paddings (S). Needs T-49a-051.
3. In parallel: `T-49a-036` headings, icon and sort rows; `T-49a-037` checkbox and radio rows; `T-49a-039` select (M); `T-49a-038` slider with value pill (M, needs T-49b-020).
4. `T-49a-001` theme settings model (S, needs area 02), `T-49a-011` system dark detection (S, needs area 01), `T-49a-002` pure theme resolution entry (M, needs T-49a-004 which is P1, see section 4.1), `T-49a-012` CSS generation and provider install (M, needs T-49a-002), `T-49a-061` CSS validity check (S).
5. `T-49a-059` test suite for theme and components (M). Needs T-49a-004, T-49a-006, T-49a-012 and T-49a-048.

Longest in-area P0 dependency chain: `T-49b-001` to `T-49b-002` to `T-49b-003` to `T-49b-008` to `T-49b-025` (five tasks).

### 6.3 Risks

- Duplicated tasks (section 4.1): scheme tables, preferences, Monet. Risk of two implementations diverging.
- Priority inversions: a P0 spine waits on P1 tasks (T-49a-004, T-49b-023, T-49a-005, T-49b-020). Schedule slips if they stay P1.
- Document layout disagreement (D-6). Two Rust layouts would produce two `theme/` modules if both are followed.
- Scheme contrast (D-5). Some schemes (Cloudflare, Doom, Matrix, Sapphire, Cotton Candy, Mocha) leave 13 M3 roles to the baseline, including the error colors. Behavior is kept as observed, so those schemes look as they do on Android.
- Monet on Linux depends on the portal `accent-colour` value. Not every desktop exposes it; the fallback path (D-2) is the only tested route on KDE without the portal.
- Font and icon assets: GResource build step (`build.rs`) and gettext extraction must work in the build (area 54).
- The GTK theme default (D-1) changes what the user sees on first run. It needs the user's confirmation before the default is written.

---

## 7. Definition of done for this area in Kurobayomi

Each item is observable. Tests named in the part documents (`TEST-49-*`) back these items.

1. On first run with no stored preference, the window follows the user's GTK theme with no Kurobayomi color overrides applied (`GtkTheme` default, D-1). No `CssProvider` from this area sets a color on the window.
2. The settings Appearance page lists the Linux `GtkTheme` option first, then the 19 Aniyomi values in `AppTheme.entries` order minus hidden entries. Monet is hidden when neither the portal accent nor the KDE fallback is available.
3. Selecting each of the 18 static schemes changes the window to that scheme's token values. Each value matches the XML or Kotlin value byte for byte (test reads the fixtures).
4. Mode row (System, Light, Dark) and the AMOLED switch behave as in `TachiyomiTheme`: AMOLED only applies in dark mode; the switch is insensitive when mode is Light.
5. Switching the system dark or light setting while mode is System updates the window without restart (portal `color-scheme` watcher).
6. An unknown stored `AppTheme` name falls back to the default without a crash. Deprecated names `DARK_BLUE`, `HOT_PINK` and `BLUE` render as the default scheme and are not shown in the picker.
7. Empty state, loading state, info screen and list group headers render with the active scheme, with text in the scheme's `onSurfaceVariant` and `onSurface` roles, and with GTK style classes where the mockup already uses them.
8. Settings rows (headings, checkbox and radio, slider with value pill, select, sort) match the mockup layout and respond to keyboard activation (Enter on single-line entries, Space on checkboxes).
9. Fast scroller (grid variant) and wheel picker: the number picker clamps to its range and steps by `(last - first) - 1` as specified in the part 1 edge-case document.
10. Badges, pills, progress rings and section cards render with the token colors; their contrast is reported by the contrast test without a crash.
11. Window size classes: width at or above 600 is medium and at or above 840 is expanded (T-49b-042); the sidebar collapses below these thresholds.
12. The whole theme and component test suite (`cargo test`, T-49a-059 and T-49b-038) passes, and the XML-versus-Kotlin drift checker (T-49b-039) reports no differences.
13. Changing the theme or AMOLED setting takes effect within one frame of the preference change, with no restart and no leftover provider from the previous theme (T-49b-008 lifecycle).
14. Nothing in this area uses Android APIs at runtime: no Compose, no Android resources, no `Injekt`. The Linux-only replacements listed in the part 2 inventory are the only substitutes.
