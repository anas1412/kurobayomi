# Area 49 (part 1): implementation tasks

Scope: the 50 files in `p1-inventory.md`. Every task ID starts with `T-49a-`. Source paths are repo-relative to `/home/blackbox/aniyomi-src`. Target: the `kurobayomi` crate (GTK4 via gtk4 0.11.5 with feature `v4_16`, no libadwaita). Behavior references point to `p1-behavior.md`, data to `p1-data-and-config.md`, design to `p1-rust-design.md`, UI to `p1-ui-parity.md`, edge cases and tests to `p1-edge-cases-and-tests.md` (IDs `E-49-*`, `F-49-*`, `R-49-*`, `TEST-49-*`), and the scheme value tables to `p1-topic-color-token-tables.md`.

Dependencies on other areas are written as `area:<id>`. The part 2 tasks are the `T-49b-*` IDs in `p2-tasks.md`; this part references them by task ID.

## A. Theme core

### T-49a-001 — Theme settings model and preference loading
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt, app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt, core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt
- Acceptance:
  - `ThemeSettings { app_theme, amoled, mode }` loads from keys `pref_app_theme`, `pref_theme_dark_amoled_key`, `pref_theme_mode_key` with defaults from `p1-data-and-config.md` section 2.
  - `pref_app_theme` default is `MONET` only when `monet::available()`, otherwise `DEFAULT` (on Linux the default is `System`, see T-49a-060).
  - Unknown or deprecated enum strings read as the default scheme (TEST-49-01, E-49-01, E-49-02).
  - Enum names are stored exactly as the Android names (`GREEN_APPLE`, `STRAWBERRY_DAIQUIRI`, ...), so an imported preference store resolves.

### T-49a-002 — Pure theme resolution entry point
- Priority: P0
- Effort: M
- Depends: T-49a-001, T-49a-003, T-49a-004
- Sources: app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/BaseColorScheme.kt
- Acceptance:
  - `theme::resolve(settings, system_dark, monet_seed) -> Resolved` has no GTK calls and is covered by TEST-49-03.
  - The scheme map has 18 static entries plus Monet, and any other value falls back to the default scheme (`TachiyomiTheme.kt:104-123` behavior).
  - Preview mode (`TachiyomiPreviewTheme` equivalent) takes explicit scheme and AMOLED args and ignores prefs.

### T-49a-003 — Light/dark selection and AMOLED override
- Priority: P0
- Effort: S
- Depends: T-49a-004
- Sources: app/src/main/java/eu/kanade/presentation/theme/colorscheme/BaseColorScheme.kt, app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt
- Acceptance:
  - Light mode ignores AMOLED (TEST-49-05, E-49-04).
  - Dark plus AMOLED sets exactly the 10 overrides in `p1-rust-design.md` section 3 and no other field (TEST-49-04, E-49-05).
  - `ThemeMode::Light/Dark` override the system value; `System` uses the portal value (T-49a-011).

### T-49a-004 — Port the 18 static color schemes as full token tables
- Priority: P1
- Effort: L
- Depends: T-49a-013
- Sources: app/src/main/java/eu/kanade/presentation/theme/colorscheme/CloudflareColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/CottoncandyColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/DoomColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/GreenAppleColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/LavenderColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/MatrixColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/MidnightDuskColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/MochaColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonochromeColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/NordColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/SapphireColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/StrawberryColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/TachiyomiColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/TakoColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/TealTurqoiseColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/TidalWaveColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/YinYangColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/YotsubaColorScheme.kt
- Acceptance:
  - Each scheme in `src/theme/schemes/` has 36 explicit base tokens per mode, built as explicit values plus `..BASELINE_*` for omitted tokens (`p1-rust-design.md` section 3).
  - Every hex value equals the value in `p1-topic-color-token-tables.md` (diff test against the generated table, TEST-49-03).
  - Omitted tokens resolve to the baseline (TEST-49-07, E-49-03). `surface_tint` omitted resolves to the scheme primary.
  - Comments from source (`Navigation bar background`, `Unread badge`, etc.) are kept as role documentation.

### T-49a-005 — Monet seed sources (portal accent, KDE fallback)
- Priority: P1
- Effort: M
- Depends: T-49a-011, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonetColorScheme.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt
- Acceptance:
  - Reads `org.freedesktop.appearance` `accent-color` via ashpd on the tokio runtime; result delivered to the GTK thread (`p1-rust-design.md` section 10).
  - KDE fallback parses `AccentColor=r,g,b` from `kdeglobals` (TEST-49-24).
  - Invalid or out-of-range values are rejected (E-49-40, TEST-49-23).
  - A missing portal and a missing KDE file give `available() == false` without an error dialog (E-49-41).

### T-49a-006 — Monet palette generation (HCT, SchemeContent, dynamic roles)
- Priority: P1
- Effort: L
- Depends: T-49a-005
- Sources: app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonetColorScheme.kt
- Acceptance:
  - `monet::generate(seed, is_dark, contrast)` returns a full 36-role table, matching the `material-color-utilities` reference vectors (TEST-49-08). Crate choice per A-49-1 in `p1-rust-design.md` section 6.3.
  - `tertiary_container` is generated from the `tertiaryContainer` role; the Kotlin defect at `MonetColorScheme.kt:97` is not reproduced (TEST-49-09, E-49-07).
  - AMOLED override applies after generation (same as T-49a-003).
  - Output is opaque; `scrim` = `0xFF000000`.
  - Contrast from the portal is clamped to the supported range 0 or 1 (E-49-08).

### T-49a-007 — Monet availability, picker filter, and fallback
- Priority: P1
- Effort: S
- Depends: T-49a-005, T-49a-062
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemePreferenceWidget.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt
- Acceptance:
  - `monet::available()` gates the picker entry and the default (E-49-06).
  - A stored `MONET` with Monet unavailable resolves to `System` (Linux) and the picker shows the System card selected.

### T-49a-010 — Player ripple state layer (CSS)
- Priority: P1
- Effort: S
- Depends: T-49a-012, area:46-player-controls-ui
- Sources: app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt
- Acceptance:
  - Player controls show a state layer of alpha 0.10 on hover, focus-visible, active, and drag, colored white in system dark and black in system light (not the app scheme) (`p1-behavior.md` 1.4, TEST MANUAL-49-07).
  - The four alpha constants are one shared constant (`RIPPLE_ALPHA = 0.10`).

### T-49a-011 — System dark detection and live updates
- Priority: P0
- Effort: S
- Depends: T-49a-001, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt, app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt
- Acceptance:
  - `color-scheme` from the Settings portal (`1` dark, `2` light, `0` or unknown no preference) feeds `system_dark` (F-49-04).
  - The GTK settings fallback is used when the portal is missing (E-49-41).
  - A `SettingChanged` signal re-runs `apply` without restart (MANUAL-49-01, E-49-43).
  - Re-entrant `apply` calls are guarded; a second portal signal during `apply` is queued (R-49-02).

### T-49a-012 — CSS generation and provider install with priorities
- Priority: P0
- Effort: M
- Depends: T-49a-002
- Sources: app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt
- Acceptance:
  - `css::render` emits `@define-color kuro_*` for every role, the `.kuro-*` component classes, and the GTK-name override block only for non-System schemes (`p1-rust-design.md` 4.3).
  - The provider is added at `STYLE_PROVIDER_PRIORITY_APPLICATION` (600), and the previous provider is removed in the same main-thread call (R-49-01).
  - A user `gtk.css` setting `@define-color` still wins (USER 800) (MANUAL check in the parity doc).
  - A parse failure keeps the previous provider (F-49-01).

### T-49a-013 — Freeze androidx Material 3 baseline color values
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/theme/colorscheme/BaseColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/TachiyomiColorScheme.kt
- Acceptance:
  - `BASELINE_DARK` and `BASELINE_LIGHT` are dumped once from the androidx.compose.material3 version pinned by the Compose BOM of this clone (`ColorDarkTokens` and `ColorLightTokens`), recorded with the version string in the source header comment (assumption A-49-3).
  - The dump includes all 36 base roles (TEST-49-07 uses it).

### T-49a-021 — Window size classes
- Priority: P1
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: app/src/main/java/mihon/core/designsystem/utils/WindowSize.kt, app/src/main/java/mihon/feature/upcoming/components/calendar/Calendar.kt
- Acceptance:
  - `classify` uses strict greater-than at 600 and 840 dp (TEST-49-10, E-49-13).
  - Width in dp is `width / scale_factor`, recomputed on `notify::default-width` and `notify::scale-factor` (`p1-rust-design.md` 7).
  - Only class changes emit events; a zero width at startup is ignored (F-49-05).
  - Area 42 calendar uses `class == Medium` in place of `isMediumWidthWindow() && !isExpandedWidthWindow()`.

### T-49a-051 — Spacing, alpha, duration, and radius tokens
- Priority: P0
- Effort: S
- Depends: none
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/material/IconButtonTokens.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/AdaptiveSheet.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/VerticalFastScroller.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/material/FloatingActionButton.kt
- Acceptance:
  - `src/theme/tokens.rs` contains every constant listed in `p1-behavior.md` section 25 with the same value (TEST-49-21 covers the FAB ones).
  - Dp values are used as CSS px at scale 1 (no manual scale multiply).
  - Typography mapping from `p1-rust-design.md` section 5 is a CSS block generated with the tokens.

### T-49a-060 — Linux "System" scheme (no color override)
- Priority: P2
- Effort: S
- Depends: T-49a-012
- Sources: app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt
- Acceptance:
  - `AppTheme::System` exists only in Kurobayomi; the picker lists it first; it emits no GTK-name override (`p1-rust-design.md` 4.5).
  - It is the Linux default for new installs; the Android pref names are unchanged.
  - Decision recorded: this is an addition beyond Aniyomi parity.

### T-49a-062 — Theme picker preview cards
- Priority: P1
- Effort: M
- Depends: T-49a-002, T-49a-024, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemePreferenceWidget.kt
- Acceptance:
  - Horizontal row of cards, width 114 px, 9:16 preview, 4 px border (primary when selected), 17 px outer and 13 px inner radius, check icon when selected (`p1-ui-parity.md` 1).
  - Each card renders its own scheme via `resolve` (not the live scheme).
  - Selecting a card applies immediately (no restart) (E-49-42).
  - Label is two lines high, alpha 0.78, text from `label` keys (`p1-behavior.md` 12).
  - Card list keys are the stable `AppTheme` names so reordering does not reset selection (E-49-39).

### T-49a-063 — Optional bundled Tokyo Night scheme (addition)
- Priority: P2
- Effort: S
- Depends: T-49a-004
- Sources: app/src/main/java/eu/kanade/presentation/theme/colorscheme/TachiyomiColorScheme.kt
- Acceptance:
  - Added only as a Kurobayomi scheme, not an Aniyomi `AppTheme` (it has no Android name); stored as `TOKYO_NIGHT`.
  - Tokens are documented in a new table; TEST-49-03 extends to it.
  - Skip this task if the author decides parity-only.

## B. Components

### T-49a-022 — ActionButton
- Priority: P1
- Effort: S
- Depends: T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/ActionButton.kt
- Acceptance:
  - Vertical icon over label, 4 px gap, centered, text button style.
  - Icon has no accessible name (matches `contentDescription = null`); the label is the accessible name.

### T-49a-023 — AdaptiveSheet (tablet dialog and phone bottom sheet)
- Priority: P1
- Effort: L
- Depends: T-49a-051, T-49a-021
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/AdaptiveSheet.kt
- Acceptance:
  - Tablet branch: centered modal, 350 ms fade in and out, max width 600 landscape / 460 portrait, outside click and Escape dismiss (E-49-28).
  - Phone branch: bottom sheet, 350 ms slide, swipe dismiss only when enabled; close when drag > 56 dp or velocity > 125 dp/s (TEST-49-19, E-49-29, E-49-30).
  - `on_dismiss` runs once (TEST-49-20, E-49-31).
  - No scrim is drawn (matches source); a tap outside still dismisses.
  - Callbacks hold weak references to the window; a close after destroy is a no-op (R-49-03).

### T-49a-024 — Badges and badge groups
- Priority: P0
- Effort: S
- Depends: T-49a-012
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/Badges.kt
- Acceptance:
  - Text badge: 3 px horizontal, 1 px vertical padding, 12 px font, weight 500, one line with ellipsis (E-49-37).
  - Icon badge: icon inline at 12 px, same colors as the text badge.
  - Default colors are `secondary` / `onSecondary`; the downloaded variant uses `tertiary` / `onTertiary`.

### T-49a-025 — Progress ring (indeterminate and determinate)
- Priority: P0
- Effort: S
- Depends: T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/CircularProgressIndicator.kt
- Acceptance:
  - Progress exactly 0 shows the spinner; progress in (0, 1] shows an arc rotating once per 2000 ms (TEST-49-25 for input handling, E-49-36).
  - Crossfade between modes (E-49-35); tick callback removed on unmap (R-49-07).
  - Reduced-motion setting stops the rotation (T-49a-057).

### T-49a-026 — Collapsible box
- Priority: P2
- Effort: S
- Depends: T-49a-033
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/CollapsibleBox.kt
- Acceptance:
  - Collapsed by default; header with 24/12 px padding; chevron swaps up and down.
  - Built on `gtk::Expander`; keyboard Enter or Space toggles.

### T-49a-027 — Labeled checkbox row
- Priority: P1
- Effort: S
- Depends: T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/LabeledCheckbox.kt
- Acceptance:
  - Full-width row, min height 48 px, 8 px gap, whole row toggles (role checkbox).
  - Disabled row does not toggle and is insensitive (fixes the source quirk noted in `p1-behavior.md` 9).

### T-49a-028 — List with bottom action button
- Priority: P1
- Effort: S
- Depends: T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/LazyColumnWithAction.kt
- Acceptance:
  - List fills the space; separator; full-width action button with 16/8 px margins; disabled state respected.

### T-49a-029 — Scrollable list and grid constructors with fast scroll option
- Priority: P1
- Effort: M
- Depends: T-49a-031, T-49a-047
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/LazyGrid.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/LazyList.kt
- Acceptance:
  - `fast_scroll::grid` takes column layout, thumb predicate, thumb color, and edge paddings with the source defaults (`Dp.Hairline` = 0.5 px).
  - Grid column width requires bounded width; unbounded width is a programming error reported at build time (not a runtime panic).
  - Callers can pick passive scrollbar (`ScrollbarLazyColumn`) or fast scroll (`FastScrollLazyColumn`).

### T-49a-031 — Fast scroll list thumb
- Priority: P1
- Effort: M
- Depends: T-49a-047
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/VerticalFastScroller.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/LazyList.kt
- Acceptance:
  - Thumb 12 x 48 px, radius 6, primary color, 8 px end inset plus `endContentPadding`; hidden when the list fits (E-49-17).
  - Show on scroll, hold 2 s, fade 250 ms; immediate fade when `thumb_allowed` is false (`p1-topic-fast-scroll-and-wheel-picker.md` 5).
  - Dragging maps proportionally to the scroll range and clamps at ends (E-49-18); sticky headers do not break the thumb (E-49-14, TEST-49-12).

### T-49a-032 — Link icon button
- Priority: P1
- Effort: S
- Depends: T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/LinkIcon.kt
- Acceptance:
  - Flat icon button, primary tint, accessible name = label, opens URL via `gtk::UriLauncher`.
  - Launch failure logs a warning and shows no crash.

### T-49a-033 — List group header
- Priority: P0
- Effort: S
- Depends: T-49a-012
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/ListGroupHeader.kt
- Acceptance:
  - Body medium, SemiBold, `on_surface_variant`, padding 16/8 px.

### T-49a-034 — Pill value chip
- Priority: P1
- Effort: S
- Depends: T-49a-012
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/Pill.kt
- Acceptance:
  - Single line, 6/1 px padding, 4 px start margin, pill radius, default color `surface_container_high`.
  - Second overload behavior is the first with `body medium` and given font size; unused local removed (E-49-38).

### T-49a-035 — Section card
- Priority: P0
- Effort: S
- Depends: T-49a-012
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SectionCard.kt
- Acceptance:
  - Optional title at 32 px side inset (title small); card with 16 px margins horizontally, 8 px vertically, 16 px inner padding, 28 px radius.

### T-49a-036 — Settings rows: headings, icon, sort
- Priority: P0
- Effort: S
- Depends: T-49a-045
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - `HeadingItem` (header style, 24/10 px) and `IconItem` (primary icon) and `SortItem` (down arrow for true, up arrow for false, blank 24 px spacer for null) match `p1-behavior.md` 19.
  - All rows are activatable with Enter and Space (`p1-ui-parity.md` 2).

### T-49a-037 — Settings rows: checkbox and radio
- Priority: P0
- Effort: S
- Depends: T-49a-045
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - Checkbox row toggles the bound preference (`CheckboxItem(label, pref)` calls `toggle`) and reflects external changes (`collectAsState` equivalent subscription).
  - Radio row selects on activation; the check control is insensitive.

### T-49a-038 — Settings rows: slider with value pill
- Priority: P0
- Effort: M
- Depends: T-49a-045, T-49b-020
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Slider.kt
- Acceptance:
  - Steps default `(last - first) - 1` and never negative (E-49-32, TEST-49-22).
  - Value pill shows `value_text` (default `value.to_string()`); pill color default `surface_container_high`.
  - `on_change` is not called when the value is unchanged (E-49-25).
  - Arrow keys step by 1, Page keys by 10 (GTK defaults).

### T-49a-039 — Settings rows: select (dropdown)
- Priority: P0
- Effort: M
- Depends: T-49a-045
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - Two source overloads map to one Rust API (`options: Vec<String>` plus a `to_string` callback).
  - Out-of-range `selected` shows an empty value and logs a warning; no panic (E-49-22).
  - Selection callback fires once per change.

### T-49a-040 — Settings rows: tri-state
- Priority: P1
- Effort: S
- Depends: T-49a-045
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - Click cycles DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED (TEST-style unit test in the Rust module).
  - Row inert when callback is None; disabled tint (E-49-26).
  - Icons per `p1-rust-design.md` 9.4.

### T-49a-041 — Repeating icon button
- Priority: P1
- Effort: M
- Depends: T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - Click fires on press, then repeats with the schedule 750, 562, 421, 315, 236, 177, 132, 99, 74, 55, 41, 30, 22, 16, 12, 9, 6, 5 (TEST-49-14).
  - Release, leave, cancel stop the timer (TEST-49-15, E-49-24, E-49-23).
  - Timer cancelled on destroy (R-49-06).

### T-49a-042 — Numeric chooser
- Priority: P1
- Effort: M
- Depends: T-49a-041, T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - Buttons step by `step` and clamp to `min` (E-49-19).
  - Field min width 140 px, numeric input purpose, suffix and placeholder shown.
  - Typed input strips invalid characters, allows intermediate "-", and clamps to min on commit (E-49-20, E-49-21, TEST-49-16). Deviation from source recorded in the task PR.

### T-49a-043 — Settings rows: text entry
- Priority: P1
- Effort: S
- Depends: T-49a-045
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - Full-width entry with 24/4 px margins, single line, label above.

### T-49a-044 — Settings rows: chip row and icon grid
- Priority: P1
- Effort: M
- Depends: T-49a-045
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - Chip row wraps with 8 px spacing; padding 24/0/24/10 px.
  - Icon grid uses adaptive 128 px columns with 8/4 px gaps and a bounded scroller (E-49-33).

### T-49a-045 — Settings row base and paddings
- Priority: P0
- Effort: S
- Depends: T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - Shared row: 24 px side and 10 px vertical padding, 24 px gap, body medium label, activation on click.
  - Rows are `gtk::ListBoxRow` inside `.boxed-list` (keeps the mockup look).

### T-49a-046 — Two-panel layout
- Priority: P1
- Effort: S
- Depends: T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/TwoPanelBox.kt
- Acceptance:
  - Start panel initial width is `min(width/2, 450)`; end fills the rest.
  - Paned resize is allowed (desktop change; minimum start width 280 px, Proposal in `p1-ui-parity.md` 13).

### T-49a-047 — Fast scroll shared logic
- Priority: P1
- Effort: M
- Depends: T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/VerticalFastScroller.kt
- Acceptance:
  - Proportional mapping implemented as pure functions with TEST-49-11 and TEST-49-13.
  - Grid proportional mapping replaces the source column arithmetic (E-49-16 documented).
  - `dragging` flag prevents feedback loops (R-49-05).
  - Grid column count is recomputed when the column layout changes (E-49-15, not the source's keyless `remember`).

### T-49a-048 — Wheel picker (number and text)
- Priority: P2
- Effort: M
- Depends: T-49a-051, T-49a-024
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/WheelPicker.kt
- Acceptance:
  - Three visible rows; default size 128 x 128 px; selected row highlighted with radius 16 and 1 px border at primary 20%.
  - Alpha formula per TEST-49-17; snapped index is max-alpha row, ties pick the first (TEST-49-18).
  - Selection callback does not fire at construction (E-49-09); `start_index` read once (E-49-10).
  - Number variant supports manual entry on double-click; no match closes without change (E-49-11, E-49-12).
  - A drag in progress cancels a pending snap; snap starts only after drag end (R-49-04).

### T-49a-049 — Dialog content layout
- Priority: P0
- Effort: S
- Depends: T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/AlertDialog.kt
- Acceptance:
  - Width clamped 280..560 px; 24 px paddings; icon (secondary, bottom 16), title (headline small, centered with icon), text (body medium, on_surface_variant, bottom 24), buttons end-aligned with 24 px padding.
  - Long text scrolls at 60% window height (E-49-27).

### T-49a-050 — Buttons (filled, text, long-press) and elevation model
- Priority: P1
- Effort: M
- Depends: T-49a-012
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Button.kt
- Acceptance:
  - Filled: primary container, on-primary text, min 58 x 40 px, pill radius; disabled 12% container and 38% text.
  - Text: transparent, primary text, 12 px side padding, disabled 38% text.
  - Long-press callback via `gtk::GestureLongPress`.
  - Hover adds the 1 dp hover elevation as a CSS shadow (Proposal for the tonal model; the tonal elevation function in source is unused and not ported).

### T-49a-052 — Extended floating action button
- Priority: P1
- Effort: S
- Depends: T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/FloatingActionButton.kt
- Acceptance:
  - Collapsed 56 px, expanded min 80 px; label padding 12/20 px.
  - Width transition 500 ms with emphasized easing; start padding 300 ms (expand) and 900 ms (collapse); label fade and expand timings per `p1-behavior.md` 22 (TEST-49-21).
  - Expanded/collapsed toggles mid-animation restart from the current width (E-49-34).

### T-49a-053 — Icon toggle button
- Priority: P2
- Effort: S
- Depends: T-49a-051
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/material/IconToggleButton.kt
- Acceptance:
  - Height 48 px; icon and up to two title lines with ellipsis; filled when checked.

### T-49a-054 — Build and packaging files (dropped)
- Priority: P3
- Effort: S
- Depends: none
- Sources: presentation-core/build.gradle.kts, presentation-core/consumer-rules.pro, presentation-core/proguard-rules.pro, presentation-core/src/main/AndroidManifest.xml
- Acceptance:
  - Dropped: Gradle, ProGuard, and manifest have no Rust equivalent. Their dependency list is replaced by the crate list in `p1-rust-design.md` section 11.
  - The decision is recorded in `Cargo.toml` comments only if needed (no file is created for this).

## C. Cross-cutting

### T-49a-030 — Paging helper (dropped; list model replacement)
- Priority: P3
- Effort: S
- Depends: none
- Sources: presentation-core/src/main/java/mihon/presentation/core/util/PagingDataUtil.kt
- Acceptance:
  - Dropped: Paging 3 is Android-only. Lists use Rust vectors from SQLite queries; keyset paging is added by the owning area only when a list exceeds 10 000 rows.
  - Drop decision recorded in the list model module doc comment.

### T-49a-055 — Keyboard and focus behavior for all widgets
- Priority: P1
- Effort: M
- Depends: T-49a-045, T-49a-041
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/AdaptiveSheet.kt
- Acceptance:
  - Every interactive widget is reachable by Tab and activatable by Enter or Space (MANUAL-49-03).
  - Focus ring comes from the user GTK theme (not overridden).
  - Sheet and dialog trap focus; Escape closes.

### T-49a-056 — Haptics (dropped)
- Priority: P3
- Effort: S
- Depends: none
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/WheelPicker.kt
- Acceptance:
  - Dropped: `HapticFeedbackType.TextHandleMove` has no portable desktop equivalent. Call sites become no-ops; the decision is in `p1-ui-parity.md` 8.

### T-49a-057 — Reduced motion
- Priority: P2
- Effort: S
- Depends: T-49a-025
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/CircularProgressIndicator.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/AdaptiveSheet.kt
- Acceptance:
  - When `gtk-enable-animations` is false, transitions are instant and the progress ring does not spin (Proposal, `p1-ui-parity.md` 10).

### T-49a-058 — Window integration of theme and size class
- Priority: P1
- Effort: S
- Depends: T-49a-021, T-49a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/ThemingDelegate.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - `main.rs` installs the theme after the window is realized and subscribes to size class changes.
  - The Android per-activity theme overrides (`ThemingDelegate`, `ContextExtensions.kt`) have no Linux equivalent; the reader background override is covered by area 44.

### T-49a-059 — Test suite for theme and components
- Priority: P0
- Effort: M
- Depends: T-49a-004, T-49a-006, T-49a-012, T-49a-048
- Sources: app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonetColorScheme.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/VerticalFastScroller.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/WheelPicker.kt
- Acceptance:
  - All TEST-49-01..25 exist and pass with `cargo test`; MANUAL-49-01..07 are listed in a checklist file kept with the app docs.
  - Fixtures in `p1-edge-cases-and-tests.md` section 5 are checked in.
  - TEST-49-02 (title keys) is included; MANUAL-49-02, MANUAL-49-04, MANUAL-49-05, MANUAL-49-06 are listed in the checklist with pass criteria.

### T-49a-061 — CSS validity check
- Priority: P0
- Effort: S
- Depends: T-49a-012
- Sources: app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt
- Acceptance:
  - TEST-49-06 parses the generated CSS for all 19 schemes and both modes and asserts every referenced `@kuro_*` name is defined.
  - Runs in CI without a display (parse only).
  - Missing-token failures are reported with the offending class name (F-49-02).

### T-49a-064 — Logging and error policy
- Priority: P0
- Effort: S
- Depends: T-49a-011, T-49a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt
- Acceptance:
  - Portal missing: info log, fallback used. Portal error: warn log, last good value kept (F-49-03, F-49-04).
  - Unknown theme value: one debug line.
  - No user-facing error dialog in this area.

### T-49a-065 — Scheme preview and picker keyboard navigation
- Priority: P2
- Effort: S
- Depends: T-49a-062, T-49a-055
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemePreferenceWidget.kt
- Acceptance:
  - Arrow keys move between preview cards; Enter selects; focus ring visible (`p1-ui-parity.md` 1).

### T-49a-066 — Responsive settings widths
- Priority: P2
- Effort: S
- Depends: T-49a-021
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt
- Acceptance:
  - Settings columns keep the 24 px inset in every window class and never exceed 560 px line length in the compact layout (Proposal; no source equivalent).

<!-- Self-check (author): every inventory task id appears above; every E-49/R-49/F-49/TEST-49 id in p1-edge-cases-and-tests.md maps to at least one task. -->
