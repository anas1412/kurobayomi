# Area 49 (part 2): implementation tasks

Every task ID starts with `T-49b-`. Sources are repo-relative under `/home/blackbox/aniyomi-src/` unless they name a Kurobayomi path (`src/`, `spec/`, `assets/`, `po/`). Design references: `p2-rust-design.md`, `p2-behavior.md`, `p2-data-and-config.md`, `p2-topic-theme-tokens.md`, `p2-topic-layout-widgets.md`, `p2-ui-parity.md`, `p2-edge-cases-and-tests.md`.

Priorities: P0 is needed to use Kurobayomi daily (theme applies, empty and loading states, selection and focus helpers, constants). P1 is parity most users rely on. P2 is polish. P3 is an Android-only drop with a stated replacement or reason.

## Theme core

### T-49b-001 — Theme token model (roles, ARGB parsing, aliases)
- Priority: P0
- Effort: S
- Depends: none
- Sources: presentation-core/src/main/res/values/colors_tachiyomi.xml, presentation-core/src/main/res/values/color_cloudflare.xml, app/src/main/java/eu/kanade/presentation/theme/colorscheme/BaseColorScheme.kt
- Acceptance:
  - `src/theme/tokens.rs` defines `Rgba` (`argb(u32)` parses `0xAARRGGBB`), `Role` with 38 variants (36 M3 roles plus `PrimaryInverseAlias`, `ElevationOverlay`), and `Tokens { colors: [Rgba; 38], amoled, dark }`.
  - Unit test: `Rgba::argb(0xD849454E)` has alpha 216, r 0x49, g 0x45, b 0x4E; `#RRGGBB` values parse as opaque.
  - `InversePrimary` resolves to `PrimaryInverseAlias` when absent; `ElevationOverlay` resolves to `Primary` when absent (test).

### T-49b-002 — Port the 18 static scheme tables from the XML (generated constants)
- Priority: P0
- Effort: M
- Depends: T-49b-001
- Sources: presentation-core/src/main/res/values/colors_tachiyomi.xml, presentation-core/src/main/res/values-night/colors_tachiyomi.xml, presentation-core/src/main/res/values/colors_monochrome.xml, presentation-core/src/main/res/values-night/colors_monochrome.xml, presentation-core/src/main/res/values/colors_yotsuba.xml, presentation-core/src/main/res/values-night/colors_yotsuba.xml, presentation-core/src/main/res/values/colors_tako.xml, presentation-core/src/main/res/values-night/colors_tako.xml, presentation-core/src/main/res/values/color_cloudflare.xml, presentation-core/src/main/res/values-night/color_cloudflare.xml, presentation-core/src/main/res/values/color_doom.xml, presentation-core/src/main/res/values-night/color_doom.xml, presentation-core/src/main/res/values/color_lavender.xml, presentation-core/src/main/res/values-night/color_lavender.xml, presentation-core/src/main/res/values/color_matrix.xml, presentation-core/src/main/res/values-night/color_matrix.xml, presentation-core/src/main/res/values/color_sapphire.xml, presentation-core/src/main/res/values-night/color_sapphire.xml, presentation-core/src/main/res/values/colors_greenapple.xml, presentation-core/src/main/res/values-night/colors_greenapple.xml, presentation-core/src/main/res/values/colors_midnightdusk.xml, presentation-core/src/main/res/values-night/colors_midnightdusk.xml, presentation-core/src/main/res/values/colors_nord.xml, presentation-core/src/main/res/values-night/colors_nord.xml, presentation-core/src/main/res/values/colors_strawberry.xml, presentation-core/src/main/res/values-night/colors_strawberry.xml, presentation-core/src/main/res/values/colors_tealturqoise.xml, presentation-core/src/main/res/values-night/colors_tealturqoise.xml, presentation-core/src/main/res/values/colors_tidalwave.xml, presentation-core/src/main/res/values-night/colors_tidalwave.xml, presentation-core/src/main/res/values/colors_yinyang.xml, presentation-core/src/main/res/values-night/colors_yinyang.xml, app/src/main/res/values/colors_cottoncandy.xml, app/src/main/res/values-night/colors_cottoncandy.xml, app/src/main/res/values/colors_mocha.xml, app/src/main/res/values-night/colors_mocha.xml
- Acceptance:
  - A generator script (kept in `tools/`, not in the app) writes `src/theme/schemes.rs` with `SCHEMES` for 18 static entries (Default, Monochrome, Yotsuba, Tako, Cloudflare, Doom, Lavender, Matrix, Sapphire, Green Apple, Midnight Dusk, Nord, Strawberry Daiquiri, Teal & Turquoise, Tidal Wave, Yin & Yang, Cotton Candy, Mocha), each with light and dark `[Option<Rgba>; 38]`.
  - Running the generator twice produces byte-identical output.
  - Every value equals the XML value (test reads the XML fixtures and compares all declared keys).
  - Table completeness per 4.1 of `p2-edge-cases-and-tests.md`.

### T-49b-003 — Scheme resolution, dark flag and AMOLED override
- Priority: P0
- Effort: S
- Depends: T-49b-001, T-49b-002
- Sources: app/src/main/java/eu/kanade/presentation/theme/colorscheme/BaseColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt, presentation-core/src/main/res/values/colors.xml, presentation-core/src/main/res/values-night/colors.xml
- Acceptance:
  - `src/theme/resolve.rs::resolve(theme) -> Tokens` follows `p2-rust-design.md` section 3.2 steps 1 to 6.
  - AMOLED applies only when the resolved mode is dark: `background` and `surface` are `#000000`; `onBackground` and `onSurface` are `#FFFFFF`; `surfaceVariant`, `surfaceContainerLowest`, `surfaceContainerLow`, `surfaceContainer` are `#0C0C0C`; `surfaceContainerHigh` is `#131313`; `surfaceContainerHighest` is `#1B1B1B`.
  - Test 4.4 passes (dark plus AMOLED, light plus AMOLED unchanged, dark without AMOLED equals the table).
  - Unknown or deprecated `AppTheme` values resolve to Default tokens (test).

### T-49b-004 — Fallback for roles a scheme does not declare (M3 baseline)
- Priority: P1
- Effort: M
- Depends: T-49b-002
- Sources: app/src/main/java/eu/kanade/presentation/theme/colorscheme/CloudflareColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/DoomColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/MatrixColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/SapphireColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/MochaColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/CottoncandyColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/MidnightDuskColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/NordColorScheme.kt
- Acceptance:
  - `FALLBACK_BASELINE` (light and dark) is generated from the M3 baseline of the Compose version pinned by the Android project (values read from the Compose material3 artifact at that version), not invented.
  - For Cloudflare, Doom, Matrix, Sapphire, Mocha and Cotton Candy the 13 roles listed in `p2-topic-theme-tokens.md` section 3 take the baseline values in both modes; test asserts `error` equals the baseline value.
  - The decision "replicate Compose, do not improve" is recorded in a code comment on `fallback_for`.

### T-49b-005 — Dynamic (Monet) option on Linux from the XDG portal accent colour
- Priority: P1
- Effort: M
- Depends: T-49b-001, T-49b-006
- Sources: app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonetColorScheme.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt
- Acceptance:
  - `src/theme/portal.rs` reads `org.freedesktop.appearance` `accent-color` through ashpd; a value is stored in `Theme.accent`.
  - With an accent, Monet tokens are derived (approximate Dynamic: primary from the accent, secondary and tertiary from the same hue at lower chroma; surfaces from a neutral of the same hue) and the card appears in the picker.
  - Without an accent, the Monet card is absent and a stored `MONET` value renders as Default tokens (test E13).
  - `extractSeedColorFromImage` (`MonetColorScheme.kt`, companion object) is not ported: it has no caller in the repository (documented drop, no user-visible loss).

### T-49b-006 — Preference keys, defaults, migration and the Linux default decision
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt, app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt, app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt, spec/settings.json, spec/app.json
- Acceptance:
  - `src/theme/prefs.rs` reads and writes `pref_app_theme` (enum name), `pref_theme_mode_key` (default `SYSTEM`), `pref_theme_dark_amoled_key` (default `false`) through the area 02 store.
  - Unknown enum names return the Linux default; `DARK_BLUE`, `HOT_PINK`, `BLUE` are rewritten to the default once (test 4.5).
  - Decision recorded (Linux default `GtkTheme` versus Aniyomi's `Default`); the chosen value is the value written on first run, and `spec/settings.json` (`"Dynamic"`) and `spec/app.json` (`"Default"`) are changed to the same default.

### T-49b-007 — Theme mode (System, Light, Dark) on GTK and the portal
- Priority: P0
- Effort: S
- Depends: T-49b-006
- Sources: app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt
- Acceptance:
  - `Light` forces light tokens and `gtk::Settings::set_gtk_application_prefer_dark_theme(false)`; `Dark` forces dark tokens and `true`; `System` follows the portal `color-scheme` (`1` dark, `2` light, `0` or unavailable: GTK preference).
  - Changing the portal scheme while in System updates the window within one second (manual test on KDE Plasma).

### T-49b-008 — Generated GTK CSS and a single CssProvider lifecycle
- Priority: P0
- Effort: M
- Depends: T-49b-001, T-49b-003, T-49b-007
- Sources: app/src/main/java/eu/kanade/presentation/theme/colorscheme/BaseColorScheme.kt, presentation-core/src/main/java/tachiyomi/presentation/core/util/Surface.kt, presentation-core/src/main/java/tachiyomi/presentation/core/theme/Typography.kt, presentation-core/src/main/java/tachiyomi/presentation/core/theme/Color.kt
- Acceptance:
  - `src/theme/css.rs` emits `@define-color` for the named colours of `p2-rust-design.md` section 5.4 and the classes of section 5.5; `src/theme/apply.rs` installs one `gtk::CssProvider` at priority `STYLE_PROVIDER_PRIORITY_APPLICATION` on the default display.
  - `reapply()` replaces the provider text (no second provider is ever added; test by counting providers on the display).
  - Golden CSS tests (4.6) pass; output is below 20 KB for every scheme.
  - A CSS parse error keeps the previous text and logs once (manual: insert an invalid rule in a debug build).
  - `GtkTheme` produces empty CSS, leaving the user's theme unchanged (manual check with Tokyo Night).

### T-49b-009 — Tonal elevation tint (surfaceColorAtElevation)
- Priority: P1
- Effort: S
- Depends: T-49b-001
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Surface.kt
- Acceptance:
  - `src/theme/elevation.rs::tint_at(surface, surface_tint, dp)` implements `alpha = ((4.5 * ln(dp + 1)) + 2) / 100` and source-over compositing.
  - Vectors of 4.2 pass exactly (for example `#FEFBFF` with tint `#0058CA` at 3 dp gives `#E9EEFB`; at 0 dp gives `#FEFBFF`).
  - A `tint: bool` argument replaces the Compose colour comparison (`color == surface`); a custom colour is never tinted (test C2).
  - Negative input is clamped to 0.

### T-49b-010 — Active filter tint (ColorScheme.active)
- Priority: P0
- Effort: S
- Depends: T-49b-003
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/theme/Color.kt
- Acceptance:
  - `Tokens::active()` returns `#FFEB3B` when the resolved dark flag is true and `#FFC107` otherwise (test both).
  - The flag used is the resolved system or mode flag, not the app theme (test: Light mode with a dark system returns the dark value, as Compose `isSystemInDarkTheme()` does when the mode is applied through night mode; confirm on KDE).
  - Filter icon in the library header uses `.kb-active` when a filter is set.

### T-49b-011 — Section header text style (Typography.header)
- Priority: P1
- Effort: S
- Depends: T-49b-008
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/theme/Typography.kt
- Acceptance:
  - CSS class `.kb-header` sets font weight 600 and colour `onSurfaceVariant`, size body medium.
  - Group headers in the mockup use `.kb-header` when a palette is active and keep the stock `heading` class under `GtkTheme`.

### T-49b-012 — Interaction timings as CSS transitions (Elevation.kt)
- Priority: P2
- Effort: S
- Depends: T-49b-008
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/util/Elevation.kt
- Acceptance:
  - Generated CSS contains `120ms` with `cubic-bezier(0.4, 0.0, 0.2, 1.0)` for incoming state, `150ms` with `cubic-bezier(0.4, 0.0, 0.6, 1.0)` for outgoing, and `120ms` for hover exit.
  - No Kotlin-style animation API is ported; the rule is documented in the CSS header comment.
  - Manual check: pressing a `.kb-surface` changes background in about 120 ms.

### T-49b-013 — Player ripple configuration (playerRippleConfiguration)
- Priority: P1
- Effort: S
- Depends: T-49b-003
- Sources: app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt
- Acceptance:
  - Player overlay controls use `.kb-player-ripple`: colour `#FFFFFF` when dark, `#000000` when light, with alpha 0.1 for pressed, hovered, focused and dragged states.
  - Test: the four alpha constants equal `.1`.

## Theme UI

### T-49b-014 — Theme picker: mode row, theme cards, selected state
- Priority: P1
- Effort: M
- Depends: T-49b-006, T-49b-007, T-49b-008
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemePreferenceWidget.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemeModePreferenceWidget.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt, spec/settings.json, src/settings.rs
- Acceptance:
  - Mode row shows System, Light, Dark (`linked` segmented control) and applies immediately.
  - Theme row shows one card per visible theme: `GtkTheme` first, then the 18 static Aniyomi schemes in `AppTheme` order, then Dynamic only if an accent exists. Each card is 114 px wide with a 9:16 preview, border 4 px (primary when selected), outer radius 17 px, inner 13 px, and a two-line label.
  - The selected card shows a check-circle icon in its preview bar; clicking a card applies it at once (no restart) and the mockup's `chips` row is replaced.
  - Card previews use each theme's own tokens (manual check: the card background differs per theme).

### T-49b-015 — Pure-black (AMOLED) switch with the Light gate
- Priority: P1
- Effort: S
- Depends: T-49b-006, T-49b-003
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/BaseColorScheme.kt
- Acceptance:
  - Switch "Pure black dark mode" is insensitive when the mode row is Light, and keeps its stored value.
  - Toggling applies immediately (no restart), which is an intentional change from the Android `ActivityCompat.recreate`.
  - Test E4 and E7.

## Layout and widgets

### T-49b-016 — Scaffold as a GTK overlay (FAB, snackbar, bars)
- Priority: P1
- Effort: M
- Depends: T-49b-008
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Scaffold.kt
- Acceptance:
  - `src/ui/scaffold.rs` exposes `Scaffold` with `relayout(bottom_inset)` implementing: FAB bottom margin `max(bottomBarHeight, bottomInset) + 16`; FAB end margin 16; snackbar bottom margin = FAB offset when a visible FAB exists, otherwise `max(bottomBarHeight, bottomInset)`.
  - A hidden FAB reserves no space (test F1).
  - Snackbar is horizontally centred; FAB is at the end edge.
  - Manual check: snackbar never overlaps the FAB.

### T-49b-017 — Sidebar geometry (NavigationRail equivalent)
- Priority: P1
- Effort: S
- Depends: T-49b-008
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/NavigationRail.kt, src/main.rs
- Acceptance:
  - The sidebar in `src/main.rs::sidebar` has a minimum width of 80 px, 4 px vertical padding, 4 px item spacing, and an 8 px gap after the header slot.
  - Below a 600 px window width the labels hide and icons remain (rule in `p2-ui-parity.md` section 3).
  - No call site outside the rail's own file exists in the app, so the port is a geometry reference only.

### T-49b-018 — NavigationBar dropped (phone bottom bar)
- Priority: P3
- Effort: S
- Depends: T-49b-017
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/NavigationBar.kt
- Acceptance:
  - No bottom navigation bar is created in Kurobayomi; the sidebar is the only navigation.
  - Documented drop: the component has no call site in the Android app, the 80 px height and tonal elevation are not ported, and no desktop user loses a feature.
  - Pages still accept a bottom bar slot (T-49b-016).

### T-49b-019 — Pull-to-refresh replacement (refresh button, F5, Ctrl+R)
- Priority: P2
- Effort: S
- Depends: T-49b-016
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/PullRefresh.kt
- Acceptance:
  - `src/ui/refresh.rs` registers action `win.refresh` with accelerators `F5` and `<Control>r`.
  - The header refresh button is insensitive and shows a `gtk::Spinner` while refreshing; it is re-enabled after the refresh completes (also on error).
  - No overscroll gesture is added; documented as a loss (touch-only).

### T-49b-020 — Integer slider over gtk::Scale
- Priority: P1
- Effort: S
- Depends: none
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Slider.kt
- Acceptance:
  - `src/ui/slider.rs::IntSlider::new(first, last, value)` clamps `value` to the range, uses step 1 and `round-digits` 0, and shows the integer value.
  - `round_half_up(-2.5) == -2`, `round_half_up(2.5) == 3`, `round_half_up(0.49) == 0` (test S3).
  - Single-value or empty ranges give `steps` 0 and a disabled slider (test S1, S2).
  - Finished event fires on release only when the value changed.
  - Works for all 5 range shapes used by the app: 0..10, 1..15, 1..10, 0..200, 0..100.

### T-49b-021 — Clickable surface and clickableNoIndication
- Priority: P1
- Effort: S
- Depends: T-49b-008, T-49b-009
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Surface.kt, presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt
- Acceptance:
  - `src/ui/surface.rs` builds a surface with `GestureClick` (primary) and `GestureLongPress`; the long press fires once and suppresses the click that follows it.
  - Hit target is at least 48 px square (CSS `min-width` and `min-height`), visual size unchanged.
  - `Surface` with tonal elevation applies `tint_at` (T-49b-009); a surface without a tint never does.
  - The no-indication variant has no hover or press feedback (class `.kb-noindication`).

### T-49b-022 — Tab label with count pill
- Priority: P1
- Effort: S
- Depends: T-49b-008
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Tabs.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/Pill.kt
- Acceptance:
  - `src/ui/tabs.rs::tab_label(text, count: Option<i64>)` shows one line with ellipsis; the pill shows when the count is `Some`, including `0`.
  - Pill text is 10 pt, background is the onBackground token at 12% in dark and 8% in light, 1 px vertical and 6 px horizontal padding, 4 px start margin.
  - Mockup tab strip uses it for library and browse tabs.

### T-49b-023 — Localization (gettext) with plural support
- Priority: P1
- Effort: M
- Depends: none
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/i18n/Localize.kt, core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt, i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - `src/i18n.rs` provides `t(msgid)` and `tn(singular, plural, n)` on `gettext-rs`, with `bindtextdomain` on the `po/` directory.
  - Catalog template generated from the moko keys used by this area (list in `p2-data-and-config.md` section 3).
  - Missing translation returns the English msgid.
  - Backslashes are not rewritten (test I1); the moko `fixed()` rule is not replicated.

### T-49b-024 — Brand and extension icons as SVG resources
- Priority: P2
- Effort: S
- Depends: none
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/icons/CustomIcons.kt, presentation-core/src/main/java/tachiyomi/presentation/core/icons/Discord.kt, presentation-core/src/main/java/tachiyomi/presentation/core/icons/Github.kt, presentation-core/src/main/java/tachiyomi/presentation/core/icons/Magnet.kt
- Acceptance:
  - `assets/icons/discord.svg`, `github.svg`, `magnet.svg` exist with `viewBox="0 0 24 24"` and a single fill (tintable by CSS `color`).
  - `src/icons.rs` loads them from the GResource bundle declared in `assets/icons.gresource.xml`.
  - The Magnet MIT notice (Neuicons) is in the repository's third-party notices; the brand marks are used only for their links.

## Screens

### T-49b-025 — Empty state (kaomoji face, message, action buttons)
- Priority: P0
- Effort: S
- Depends: T-49b-008, T-49b-023
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/screens/EmptyScreen.kt
- Acceptance:
  - `src/ui/empty_state.rs` holds `ERROR_FACES` with the 12 strings verbatim from `p2-behavior.md` section 9.1 (test 4.7).
  - One face is chosen per widget instance and kept on refresh.
  - Actions: equal-width buttons in a row with 8 px gaps; no row when the list is empty; more than three actions wrap (EM2).
  - The face is shown left to right; the whole content scrolls when taller than the page.
  - ASCII fallback `:(` is shown when the face glyphs are missing (Pango check at start).

### T-49b-026 — Info screen (full page with accept and reject buttons)
- Priority: P1
- Effort: S
- Depends: T-49b-008, T-49b-016
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/screens/InfoScreen.kt
- Acceptance:
  - `src/ui/info_screen.rs` shows: 48 px top gap, 48 px icon in primary, headline, subtitle at 78% opacity with 8 px vertical padding, then content.
  - Bottom bar: 1 px top border in `borders`, accept button full width and insensitive when `can_accept` is false; reject button only when both reject text and handler are given.
  - Horizontal padding 16 px.

### T-49b-027 — Loading spinner page
- Priority: P0
- Effort: S
- Depends: T-49b-008
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/screens/LoadingScreen.kt
- Acceptance:
  - `src/ui/loading.rs` returns a centred `gtk::Spinner` filling its parent, spinning while visible.
  - The spinner is shown only if loading lasts 300 ms or more (proposal; avoids flicker). Manual check on a fast load: no spinner shown.

## Constants and helpers

### T-49b-028 — Opacity and padding constants
- Priority: P0
- Effort: S
- Depends: T-49b-008
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt, presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt
- Acceptance:
  - `src/ui/consts.rs` defines `SECONDARY_ALPHA = 0.78`, `DISABLED_ALPHA = 0.38`, and the padding scale 32, 24, 16, 12, 8, 4.
  - `.kb-secondary-item` applies `opacity: 0.78`; disabled items use 0.38 (not both at once, test M5).
  - `topSmallPaddingValues` (top equals `small`) is expressed as a helper, not a constant.

### T-49b-029 — Selected row tint (selectedBackground)
- Priority: P0
- Effort: S
- Depends: T-49b-003
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt
- Acceptance:
  - `.kb-selected` background is the secondary token at alpha 0.16 when the resolved dark flag is true and 0.22 otherwise (test M1: follows system and mode, not the theme name).
  - Applied to selected rows in the library, browse and updates lists (mockup `boxed_list` rows).
  - Unselected rows get no background.

### T-49b-030 — Enter-key activation for single-line entries
- Priority: P1
- Effort: S
- Depends: none
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt
- Acceptance:
  - `src/ui/helpers.rs::enter_activates(entry, action)` adds a capture-phase `gtk::EventControllerKey` that runs on `Return` and `KP_Enter` and stops propagation; other keys are not consumed.
  - Used only on single-line `gtk::Entry` widgets (test M2).

### T-49b-031 — Focus on open (showSoftKeyboard)
- Priority: P2
- Effort: S
- Depends: none
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt
- Acceptance:
  - `src/ui/helpers.rs::focus_on_open(widget)` calls `grab_focus()` once, when the dialog or page is first shown; re-rendering does not re-focus (test M3).

### T-49b-032 — Clear focus on IME hide (dropped)
- Priority: P3
- Effort: S
- Depends: none
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt
- Acceptance:
  - No Rust code for `clearFocusOnSoftKeyboardHide`. Documented drop: it reacts to the Android soft keyboard, which does not exist on a desktop; the user loses nothing.

### T-49b-033 — FAB expansion rule (shouldExpandFAB)
- Priority: P1
- Effort: S
- Depends: T-49b-016
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/util/LazyListState.kt
- Acceptance:
  - `src/ui/scroll.rs::should_expand_fab(now, prev_value)` returns `value <= 0 || value < prev || value + page >= upper - 0.5`.
  - Truth-table test over the 8 combinations of the three inputs (4.7).
  - The page tracks `prev_value` on `value-changed`.

### T-49b-034 — Insets addition (PaddingValues plus)
- Priority: P2
- Effort: S
- Depends: none
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/util/PaddingValues.kt
- Acceptance:
  - `src/ui/insets.rs::Insets` with `start`, `end`, `top`, `bottom` in px and `Add` (component-wise sum). LTR only; test associativity and zero identity.

### T-49b-035 — Preference observation bridge (collectAsState)
- Priority: P1
- Effort: M
- Depends: area:02-preferences-system
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/util/Preference.kt, core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt
- Acceptance:
  - `src/prefs/observe.rs` returns a value plus a glib receiver; the current value is delivered first on subscribe.
  - A change reaches the widget within one main-loop iteration (test with a fake store).
  - Dropping the widget drops the receiver; no callback runs on a destroyed widget (P3 in the edge file).

### T-49b-036 — Scrollbar: dropped in favour of GTK overlay scrolling
- Priority: P3
- Effort: S
- Depends: none
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/util/Scrollbar.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/VerticalFastScroller.kt
- Acceptance:
  - Every scrollable page uses `gtk::ScrolledWindow` with overlay scrolling (the `ui::scrolled` helper in `src/ui.rs` sets it).
  - No custom thumb or fade code is written. Documented loss: the Compose thumb ignored sticky headers when placing itself; the GTK thumb does not know about sticky headers either.
  - The `"sticky:"` key prefix (`Scroller.STICKY_HEADER_KEY_PREFIX`) is not ported unless a list in the port uses sticky headers (then T-49b-036 is revisited with area 37 and 38).

### T-49b-037 — ThemePreviews annotation dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/util/ThemePreviews.kt
- Acceptance:
  - No runtime code. Replacement: the dev gallery (one page per component, one row per theme) behind a debug flag, and the snapshot harness in `plans/_tools/snap`. Documented drop (Android Studio preview only).

### T-49b-038 — Test suite for area 49
- Priority: P1
- Effort: M
- Depends: T-49b-001, T-49b-002, T-49b-003, T-49b-008, T-49b-009, T-49b-020, T-49b-025, T-49b-033
- Sources: app/src/main/java/eu/kanade/presentation/theme/colorscheme/BaseColorScheme.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Surface.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Slider.kt, presentation-core/src/main/java/tachiyomi/presentation/core/util/LazyListState.kt, presentation-core/src/main/java/tachiyomi/presentation/core/screens/EmptyScreen.kt, presentation-core/src/main/java/tachiyomi/presentation/core/i18n/Localize.kt
- Acceptance:
  - `cargo test` runs sections 4.1 to 4.7 of `p2-edge-cases-and-tests.md`; all pass.
  - Fixtures from `tests/fixtures/` (XML copies, golden CSS, `vectors.json`) are checked in; tests do not need the Android tree.
  - Contrast report (4.3) prints the known warnings and does not fail.

## Data and drift checks

### T-49b-039 — XML versus Kotlin drift checker and Color(Long) verification
- Priority: P2
- Effort: S
- Depends: T-49b-002
- Sources: presentation-core/src/main/res/values/colors_tachiyomi.xml, app/src/main/java/eu/kanade/presentation/theme/colorscheme/CloudflareColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonochromeColorScheme.kt
- Acceptance:
  - A checker (tools/, run by hand) reports mismatches between the XML and the Kotlin for all declared roles; after T-49b-041 it reports 0 mismatches for the 18 static schemes (1000 comparisons on the current tree, 2 known mismatches).
  - The 8-digit `onSurfaceVariant` values (Cloudflare, Doom, Matrix, Sapphire, light `0xFFD849454E`, dark `0xFFD8FFFFFF`) are compared by rendering: the checker records how Compose's `Color(Long)` reads the stray high byte (the result must be recorded in `p2-topic-theme-tokens.md` section 2).

### T-49b-040 — Android-only and shared colours (cover placeholder, divider, notification accent, amoled values)
- Priority: P2
- Effort: S
- Depends: T-49b-002
- Sources: presentation-core/src/main/res/values/colors.xml, presentation-core/src/main/res/values-night/colors.xml
- Acceptance:
  - Tokens exist for `cover_placeholder` (`#1F888888`), `divider` (`#1F000000` light, `#1FFFFFFF` dark), `accent_blue` (`#54759E`), and the amoled values (used by T-49b-003).
  - `splash` (`#54759E` in light, `#202125` in dark, consumed by the Android splash theme) is dropped: documented, no desktop equivalent.
  - Notifications (area 35) take `accent_blue` from the token table.

### T-49b-041 — Colour value and naming cleanup
- Priority: P2
- Effort: S
- Depends: T-49b-002, T-49b-039
- Sources: presentation-core/src/main/res/values/color_cloudflare.xml, presentation-core/src/main/res/values/color_doom.xml, presentation-core/src/main/res/values/color_lavender.xml, presentation-core/src/main/res/values/color_matrix.xml, presentation-core/src/main/res/values/color_sapphire.xml, presentation-core/src/main/res/values/colors_tealturqoise.xml, presentation-core/src/main/res/values/colors_monochrome.xml, presentation-core/src/main/res/values-night/color_matrix.xml
- Acceptance:
  - Monochrome light `surfaceDim` is `#FFFFFF` (Compose value) in the Rust table; Mocha light `inverseOnSurface` is `#FBEAEB`.
  - Table keys use the `TEALTURQUOISE` and `MATRIX` names without the file spelling; the typo `tealturqoise` is not in any Rust identifier.
  - Header comments copied from another theme (matrix says Doom) are corrected in the generated table's comments.
  - Stored enum names are unchanged (test 4.5).

## Geometry, breakpoints

### T-49b-042 — Window breakpoints (medium 600, expanded 840)
- Priority: P2
- Effort: S
- Depends: none
- Sources: app/src/main/java/mihon/core/designsystem/utils/WindowSize.kt
- Acceptance:
  - `src/ui/breakpoints.rs` provides `is_medium_width(w) = w > 600` and `is_expanded_width(w) = w > 840` (strict, as Compose).
  - Used by the sidebar label rule (T-49b-017) and the layout of two-pane pages (area 37 and 38).

### T-49b-043 — AppTheme enum port (titles from gettext, deprecated entries)
- Priority: P0
- Effort: S
- Depends: T-49b-023, T-49b-006
- Sources: app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt, app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/ThemingDelegate.kt, app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt
- Acceptance:
  - `src/theme/mod.rs::AppTheme` has the 19 selectable variants in the declaration order of `AppTheme.kt` (Default first, Monochrome last) plus `GtkTheme` (Linux-only, first in the picker) and the three deprecated names kept only for parsing (`DARK_BLUE`, `HOT_PINK`, `BLUE`).
  - Titles come from `t()` with the moko keys in `p2-data-and-config.md` section 3; the deprecated variants have no title and are never shown.
  - Mapping of each variant to its scheme matches `p2-rust-design.md` section 3.1 (test: every selectable variant has a scheme or is Monet or GtkTheme).
