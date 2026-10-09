# Area 49 (part 1): Rust design

Target: Kurobayomi (`/home/blackbox/kurobayomi`, crate `kurobayomi`, `Cargo.toml` pins `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, glib 0.22 per the licenses screen in `src/settings.rs:256`). GTK4 only, no libadwaita. The user's GTK theme and the KDE/Breeze colors must keep working when no Kurobayomi scheme is chosen.

## 1. Module layout

Keep the flat mockup layout and add a `theme/` module and a `widgets/` module (the mockup has `ui.rs`, `screens.rs`, `entry.rs`, `player.rs`, `settings.rs`, `clamp.rs`; `clamp.rs` is the AdwClamp replacement and stays as is).

```
src/
  main.rs                    (existing) app bootstrap; calls theme::install() after the window is realized
  theme/
    mod.rs                   pub API: ThemeSettings, Resolved, install(), apply()
    tokens.rs                spacing, alpha, durations, radii, breakpoints (section 5)
    scheme.rs                SchemeTokens (36 roles), resolve(is_dark, amoled), BASELINE_DARK/LIGHT
    schemes/
      mod.rs                 SCHEMES: [(AppTheme, SchemeDef)] for the 18 static schemes
      default.rs cloudflare.rs cottoncandy.rs doom.rs green_apple.rs lavender.rs matrix.rs
      midnight_dusk.rs mocha.rs monochrome.rs nord.rs sapphire.rs strawberry.rs tako.rs
      teal_turquoise.rs tidal_wave.rs yin_yang.rs yotsuba.rs
    monet.rs                 seed source (portal / KDE), HCT + SchemeContent port (section 6)
    hct.rs                   Hct, Cam16, tone/hue/chroma math (only if no crate is adopted)
    css.rs                   tokens -> CSS text (section 4.3)
    dark.rs                  system dark detection via portal color-scheme (section 4.4)
    prefs.rs                 typed accessors over area 02 prefs (keys in section 4.1)
  ui/
    window_size.rs           WindowClass from widget width / scale (section 7)
    widgets/                 one file per Compose component (section 8)
      mod.rs
      settings_items.rs      SettingsRow family
      fast_scroller.rs       VerticalFastScroller + grid variant
      wheel_picker.rs
      adaptive_sheet.rs
      progress_ring.rs
      badge.rs
      dialog_content.rs
      button.rs
      extended_fab.rs
      ... (remaining component files as listed in p1-inventory.md)
```

Existing mockup files (`src/ui.rs`, `src/screens.rs`, `src/settings.rs`, `src/entry.rs`, `src/player.rs`) currently use stock GTK style classes (`suggested-action`, `pill`, `osd`, `toolbar`, `circular`, `card`, `boxed-list`, `navigation-sidebar`, `dim-label`, `caption-heading`, `heading`, `title-1..4`). Proposal: keep those class names (they map to the user's theme) and add Kurobayomi-specific classes `.kuro-*` only where a Compose component has no GTK equivalent (badges, pills, section cards, FAB, sheet). Do not restyle existing widgets to a fixed palette.

## 2. Public API

```rust
// src/theme/mod.rs
use crate::theme::scheme::SchemeTokens;

/// Persisted inputs, read from area 02's prefs table (keys in section 4.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeSettings {
    pub app_theme: AppTheme,      // default: Monet if Monet::available() else Default
    pub amoled: bool,             // default false
    pub mode: ThemeMode,          // default ThemeMode::System
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AppTheme {
    Default, Monet, Cloudflare, Cottoncandy, Doom, GreenApple, Lavender, Matrix,
    MidnightDusk, Mocha, Sapphire, Nord, StrawberryDaiquiri, Tako, TealTurquoise,
    TidalWave, YinYang, Yotsuba, Monochrome,
    /// Linux-only addition (section 4.5): no color override; the GTK theme is used as is.
    System,
    // Deprecated Android names (DARK_BLUE, HOT_PINK, BLUE): read as Default
}

impl AppTheme {
    /// Exact Android enum names, for prefs compatibility (section 1.1 of p1-data-and-config.md).
    pub fn from_pref_name(s: &str) -> AppTheme;   // unknown or deprecated -> Default
    pub fn pref_name(self) -> &'static str;       // "MONET", "GREEN_APPLE", ...
    /// None = hidden from the picker (deprecated values).
    pub fn title_key(self) -> Option<&'static str>; // "label_default", "theme_monet", ...
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode { Light, Dark, System }

/// Resolved, ready-to-render result.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub is_dark: bool,
    pub tokens: SchemeTokens,     // after AMOLED override
    pub css: String,              // the generated stylesheet
}

pub fn resolve(settings: &ThemeSettings, system_dark: bool, monet_seed: Option<Rgba>) -> Resolved;

pub fn install(display: &gtk::gdk::Display, resolved: &Resolved) -> gtk::CssProvider;
```

Notes:
- `resolve` is pure (no GTK calls) and is the unit-test surface (section 9).
- `install` removes the previous provider (keep the `CssProvider` handle in a `RefCell` on the main thread and call `gtk::style_context_remove_provider_for_display`) and adds the new one at `gtk::STYLE_PROVIDER_PRIORITY_APPLICATION` (600). GTK4 priorities: FALLBACK 1, THEME 200, SETTINGS 400 (the desktop/GTK settings layer), APPLICATION 600, USER 800 (`~/.config/gtk-4.0/gtk.css`). APPLICATION beats the theme and the settings layer, so the Kurobayomi scheme wins over the GTK theme, and it stays below USER, so the user's own `gtk.css` still wins.

## 3. Types

```rust
// src/theme/scheme.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba(pub u32); // 0xAARRGGBB, as in the Kotlin source; alpha FF for all static schemes

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemeTokens {
    pub primary: Rgba, pub on_primary: Rgba, pub primary_container: Rgba, pub on_primary_container: Rgba,
    pub inverse_primary: Rgba,
    pub secondary: Rgba, pub on_secondary: Rgba, pub secondary_container: Rgba, pub on_secondary_container: Rgba,
    pub tertiary: Rgba, pub on_tertiary: Rgba, pub tertiary_container: Rgba, pub on_tertiary_container: Rgba,
    pub background: Rgba, pub on_background: Rgba, pub surface: Rgba, pub on_surface: Rgba,
    pub surface_variant: Rgba, pub on_surface_variant: Rgba, pub surface_tint: Rgba,
    pub inverse_surface: Rgba, pub inverse_on_surface: Rgba,
    pub error: Rgba, pub on_error: Rgba, pub error_container: Rgba, pub on_error_container: Rgba,
    pub outline: Rgba, pub outline_variant: Rgba, pub scrim: Rgba,
    pub surface_bright: Rgba, pub surface_dim: Rgba,
    pub surface_container: Rgba, pub surface_container_high: Rgba, pub surface_container_highest: Rgba,
    pub surface_container_low: Rgba, pub surface_container_lowest: Rgba,
}

pub struct SchemeDef { pub dark: SchemeTokens, pub light: SchemeTokens }

pub const BASELINE_DARK: SchemeTokens = /* filled from the pinned androidx.compose.material3 ColorDarkTokens */;
pub const BASELINE_LIGHT: SchemeTokens = /* filled from ColorLightTokens */;

pub fn resolve(def: &SchemeDef, is_dark: bool, amoled: bool) -> SchemeTokens;
```

Each static scheme is written as a full struct literal. Omitted Kotlin tokens are filled from `BASELINE_*` at compile time by a builder:

```rust
// src/theme/schemes/cloudflare.rs (pattern)
pub const DEF: SchemeDef = SchemeDef {
    dark: SchemeTokens { primary: Rgba(0xFFF38020), /* ... every token explicit or BASELINE_DARK.x */ ..BASELINE_DARK },
    light: SchemeTokens { primary: Rgba(0xFFF38020), /* ... */ ..BASELINE_LIGHT },
};
```

Rule: explicit values first, then struct update syntax from the baseline, so every omitted token resolves to the baseline value, exactly as Compose does. Values come from `p1-topic-color-token-tables.md` (regenerate it if the source changes).

AMOLED override (`resolve`, when `is_dark && amoled`):
```rust
t.background = Rgba(0xFF000000); t.on_background = Rgba(0xFFFFFFFF);
t.surface = Rgba(0xFF000000); t.on_surface = Rgba(0xFFFFFFFF);
t.surface_variant = Rgba(0xFF0C0C0C);
t.surface_container_lowest = Rgba(0xFF0C0C0C); t.surface_container_low = Rgba(0xFF0C0C0C);
t.surface_container = Rgba(0xFF0C0C0C);
t.surface_container_high = Rgba(0xFF131313); t.surface_container_highest = Rgba(0xFF1B1B1B);
```

## 4. Theme plumbing

### 4.1 Preference keys (area 02 store)

| Key | Type | Default |
|---|---|---|
| `pref_app_theme` | string enum name | `MONET` if `monet::available()`, else `DEFAULT` |
| `pref_theme_dark_amoled_key` | bool | false |
| `pref_theme_mode_key` | string enum name | `SYSTEM` |

Read with area 02's typed `Pref<T>` accessors. Enum decode: `AppTheme::from_pref_name` (unknown returns Default; never an error).

### 4.2 Propagation

- `ThemeSettings` lives in a `Rc<RefCell<..>>` on the GTK thread, or in a `glib::Object` property. Changing a pref (area 47 widgets) calls `theme::apply(&settings)`.
- `apply` recomputes `Resolved`, calls `install`, and then the global CSS redraws all widgets. No widget rebuild is needed.
- Monet seed changes (portal `SettingChanged` signal) call `apply` again.

### 4.3 CSS text (Proposal)

Generated by `css.rs` as one string:

```css
@define-color kuro_primary #F38020;
@define-color kuro_on_primary #1B1B22;
/* ... one @define-color per role, kuro_ prefix, lower-case snake names ... */
```

Then component classes use the tokens (names stable across schemes):

```css
.kuro-badge { background-color: @kuro_secondary; color: @kuro_on_secondary;
  padding: 1px 3px; border-radius: 4px; font-weight: 500; font-size: 0.75em; }
.kuro-badge.downloaded { background-color: @kuro_tertiary; color: @kuro_on_tertiary; }
.kuro-pill { background-color: @kuro_surface_container_high; color: @kuro_on_surface;
  border-radius: 999px; padding: 1px 6px; margin-left: 4px; }
.kuro-section-card { background-color: @kuro_surface_container_low; border-radius: 28px; }
.kuro-sheet { background-color: @kuro_surface_container_high; border-radius: 28px 28px 0 0; }
.kuro-dialog { background-color: @kuro_surface_container_high; border-radius: 28px; }
window { background-color: @kuro_background; color: @kuro_on_background; }
```

Important: `@define-color` names in GTK4 can collide with the theme's own names (`theme_bg_color`, `accent_color`, etc.). Use the `kuro_` prefix only; never redefine `accent_color` (that would hijack GTK widgets).

Mapping of M3 role to the GTK-named colors used by the mockup (Proposal, applied in one extra block so GTK widgets follow the scheme when a Kurobayomi scheme is chosen):

```css
@define-color accent_color @kuro_primary;
@define-color accent_bg_color @kuro_primary;
@define-color accent_fg_color @kuro_on_primary;
@define-color window_bg_color @kuro_background;
@define-color window_fg_color @kuro_on_background;
@define-color view_bg_color @kuro_surface;
@define-color view_fg_color @kuro_on_surface;
@define-color card_bg_color @kuro_surface_container_low;
@define-color card_fg_color @kuro_on_surface;
@define-color headerbar_bg_color @kuro_surface_container;
@define-color headerbar_fg_color @kuro_on_surface;
@define-color popover_bg_color @kuro_surface_container_high;
@define-color dialog_bg_color @kuro_surface_container_high;
@define-color destructive_color @kuro_error;
@define-color destructive_bg_color @kuro_error;
@define-color destructive_fg_color @kuro_on_error;
@define-color borders @kuro_outline_variant;
```

When the selected scheme is "System" (see section 4.5), emit none of these overrides. This keeps the Breeze/Plasma look.

### 4.4 Dark detection

`system_dark` = `org.freedesktop.appearance` `color-scheme` read via the Settings portal (`ashpd::desktop::settings::Settings::color_scheme()`; `1` = prefer dark, `2` = prefer light, `0` = no preference). Fallback when the portal is unavailable: GTK `gtk::Settings::default().gtk_application_prefer_dark_theme()` and then the `gtk-theme-name` suffix `-dark`. The result is the `is_dark` input to `resolve`. `ThemeMode::Dark/Light` overrides it.

Subscribe to `SettingChanged` (namespace `org.freedesktop.appearance`, key `color-scheme`) and re-run `apply`.

### 4.5 System scheme

Proposal: add a Kurobayomi default that does not override GTK at all (`AppTheme::Default` maps to the static Tachiyomi default scheme, but a new "System" entry is needed on Linux). This is a deliberate deviation: Aniyomi has no equivalent. Record it as a P2 addition in `p1-tasks.md` (T-49a-060). Default for new installs on Linux: `System` (not `Monet`, which is unavailable without a seed).

## 5. Tokens (`src/theme/tokens.rs`)

```rust
pub const PAD_EXTRA_LARGE: f32 = 32.0;
pub const PAD_LARGE: f32 = 24.0;
pub const PAD_MEDIUM: f32 = 16.0;
pub const PAD_MEDIUM_SMALL: f32 = 12.0;
pub const PAD_SMALL: f32 = 8.0;
pub const PAD_EXTRA_SMALL: f32 = 4.0;
pub const DISABLED_ALPHA: f32 = 0.38;
pub const SECONDARY_ALPHA: f32 = 0.78;
pub const ICON_STATE_LAYER: f32 = 40.0;
pub const RIPPLE_ALPHA: f32 = 0.10;
pub const SHEET_MAX_WIDTH_LANDSCAPE: f32 = 600.0;
pub const SHEET_MAX_WIDTH_PORTRAIT: f32 = 460.0;
pub const SHEET_ANIM_MS: u32 = 350;
pub const SHEET_POSITIONAL_THRESHOLD: f32 = 56.0;  // dp
pub const SHEET_VELOCITY_THRESHOLD: f32 = 125.0;   // dp/s
pub const THUMB_LENGTH: f32 = 48.0;
pub const THUMB_THICKNESS: f32 = 12.0;
pub const SCROLLBAR_FADE_MS: u32 = 250;
pub const SCROLLBAR_FADE_DELAY_MS: u32 = 2000;
pub const SCROLL_SAMPLE_MS: u32 = 100;
pub const WHEEL_ROWS: u32 = 3;
pub const WHEEL_SIZE: (f32, f32) = (128.0, 128.0);
pub const REPEAT_MAX_MS: u64 = 750;
pub const REPEAT_MIN_MS: u64 = 5;
pub const REPEAT_DECAY: f64 = 0.25;
pub const NUMERIC_MIN_WIDTH: f32 = 140.0;
pub const DIALOG_MIN_WIDTH: f32 = 280.0;
pub const DIALOG_MAX_WIDTH: f32 = 560.0;
pub const DIALOG_PAD: f32 = 24.0;
pub const FAB_COLLAPSED_WIDTH: f32 = 56.0;
pub const FAB_EXPANDED_MIN_WIDTH: f32 = 80.0;
pub const FAB_ICON_PAD: f32 = 12.0;
pub const FAB_TEXT_PAD: f32 = 20.0;
pub const PROGRESS_ROTATION_MS: u32 = 2000;
pub const TWO_PANEL_FIRST_MAX: f32 = 450.0;
pub const SETTINGS_PAD_H: f32 = 24.0;
pub const SETTINGS_PAD_V: f32 = 10.0;
pub const RADIUS_EXTRA_SMALL: f32 = 4.0;
pub const RADIUS_SMALL: f32 = 8.0;
pub const RADIUS_MEDIUM: f32 = 12.0;
pub const RADIUS_LARGE: f32 = 16.0;
pub const RADIUS_EXTRA_LARGE: f32 = 28.0;

pub const EASE_EMPHASIZED: (f32, f32, f32, f32) = (0.2, 0.0, 0.0, 1.0);
pub const EASE_LINEAR: (f32, f32, f32, f32) = (0.0, 0.0, 1.0, 1.0);
```

Units: all Dp in the Kotlin source map to CSS px at scale 1. GTK scales them; do not multiply by scale factor manually.

Typography (Proposal, CSS): body medium `font-size: 0.875em; line-height: 20px` (14/20), label large `font-size: 0.875em; font-weight: 500` (14 medium), title small `font-size: 0.875em; font-weight: 500`, title medium `font-size: 1em; font-weight: 500; line-height: 24px`, headline small `font-size: 1.5em; line-height: 32px`, body small `font-size: 0.75em; line-height: 16px`. The `header` style is `body medium` + `color: @kuro_on_surface_variant` + `font-weight: 600`.

## 6. Monet (`src/theme/monet.rs`)

```rust
pub fn available() -> bool;                       // true iff a seed can be read (section 6.2)
pub async fn read_seed() -> Option<Rgba>;         // portal accent-color or KDE kdeglobals
pub fn generate(seed: Rgba, is_dark: bool, contrast: f64) -> SchemeTokens;
```

### 6.1 Generation

Port of `SchemeContent(Hct.fromInt(seed), isDark, contrast)` plus `MaterialDynamicColors` (`MonetColorScheme.kt:74-123`). Each role maps to one `MaterialDynamicColors` function. The `tertiaryContainer` role must use `tertiaryContainer()`, not `tertiary()` (the Kotlin defect at line 97 is NOT ported; test TEST-49-09).

Contrast: `0.0` default. Read it from the portal `org.freedesktop.appearance` `contrast` key (values: 0 = normal, 1 = high); map 1 to `contrast = 1.0`, else 0.0. Android uses `UiModeManager.contrast` (API 34+), which is -1..1; the Linux mapping uses the portal's coarser values.

Alpha: all generated colors are opaque. `scrim` = `0xFF000000`.

### 6.2 Seed sources, in order

1. Portal: `ashpd` `Settings::accent_color()` returns `(f64, f64, f64)` in 0..1. Seed = `Rgba(0xFF_RR_GG_BB)` with rounding `(x * 255).round()`. Used when the call succeeds and the values are in range.
2. KDE: `$XDG_CONFIG_HOME/kdeglobals` (default `~/.config/kdeglobals`), section `[General]` key `AccentColor` (format `r,g,b` integers), else `[Colors:Header]` key `BackgroundNormal`. Parse with a plain INI reader (no crate needed).
3. None: `available() == false`.

`read_seed` runs on the tokio runtime (portal is async); the result is delivered to the GTK thread through `glib::MainContext::channel` or `glib::spawn_future_local`.

### 6.3 Crate evaluation (open, assumption A-49-1)

Before writing `hct.rs`, evaluate `material-colors` (crates.io) for `Hct`, `SchemeContent`, and `MaterialDynamicColors`. Adopt it when all three exist and match `material-color-utilities` output for the test vectors in `p1-edge-cases-and-tests.md`. Otherwise port (about 600 lines: CAM16, HCT solver, tone mapping, scheme and dynamic color tables).

## 7. Window size classes (`src/ui/window_size.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowClass { Compact, Medium, Expanded }

pub fn classify(width_dp: f32) -> WindowClass {
    if width_dp > 840.0 { WindowClass::Expanded }
    else if width_dp > 600.0 { WindowClass::Medium }
    else { WindowClass::Compact }
}
```

Strict greater-than matches `WindowSize.kt` (`screenWidthDp > MediumWidthWindowSize.value`).

Width source: `window.width()` (content width, logical px) divided by `window.scale_factor()`. Connect `notify::default-width`, `notify::scale-factor`, and `notify::width` on the main window (the last is a widget property; in gtk4 use `widget.connect_map` + a tick callback when width changes, or `gtk::Widget::add_tick_callback` comparing the last value). Emit a `WindowClass` change event only when the class changes.

`isMediumWidthWindow() && !isExpandedWidthWindow()` (Calendar, area 42) is `class == WindowClass::Medium`.

## 8. Component mapping (Compose -> GTK4)

Each widget is a function returning a `gtk::Widget` (no custom GObject subclasses unless needed), following the mockup style (`ui.rs` helpers). All widgets read tokens from CSS classes, not from Rust constants, except geometry needed for layout code (sizes in section 5).

| Compose component | Rust function | GTK widgets | Notes |
|---|---|---|---|
| `ActionButton` | `widgets::action_button(title, icon_name, on_click)` | `gtk::Button` with `gtk::Box` (vertical, spacing 4) of `gtk::Image` + `gtk::Label` | icon has no accessible name (matches `contentDescription = null`) |
| `AdaptiveSheet` | `widgets::adaptive_sheet(parent, is_tablet, enable_swipe, on_dismiss, child)` | tablet: `gtk::Window` modal transient; phone: `gtk::Revealer` (slide up) in an overlay plus `gtk::GestureDrag` | section 9.2 |
| `Badge` / `BadgeGroup` | `widgets::badge(text, color_class)`, `badge_group(children)` | `gtk::Label` with `.kuro-badge`; `gtk::Box` with `.linked`-like radius | |
| `CombinedCircularProgressIndicator` | `widgets::progress_ring(progress_fn)` | `gtk::Spinner` (indeterminate) or `gtk::DrawingArea` | section 9.3 |
| `CollapsibleBox` | `widgets::collapsible(heading, child)` | `gtk::Expander` | |
| `LabeledCheckbox` | `widgets::labeled_checkbox(label, checked, on_toggle, enabled)` | `gtk::CheckButton` with label, full-width row | |
| `LazyColumnWithAction` | `widgets::list_with_action(contents, label, on_action, enabled)` | `gtk::Box` vertical: `gtk::ScrolledWindow` (vexpand) + `gtk::Separator` + `gtk::Button` (`.suggested-action`, full width, margins 16/8) | |
| `FastScrollLazyVerticalGrid` / `FastScrollLazyColumn` / `ScrollbarLazyColumn` | `widgets::fast_scroll::{list, grid}` | `gtk::Overlay` over `gtk::ScrolledWindow` with a custom thumb `gtk::DrawingArea` on the right | section 9.1 |
| `LinkIcon` | `widgets::link_icon(label, icon, url)` | `gtk::Button` (flat) with `gtk::Image`; click calls `gtk::UriLauncher::new(url).launch(...)` | `UriHandler.openUri` equivalent |
| `ListGroupHeader` | `widgets::group_header(text)` | `gtk::Label` with `.kuro-group-header` | |
| `Pill` | `widgets::pill(text, style_class)` | `gtk::Label` with `.kuro-pill` | |
| `SectionCard` | `widgets::section_card(title: Option<&str>, child)` | optional `gtk::Label` (`.heading`), then `gtk::Frame`/`gtk::Box` with `.kuro-section-card`, padding 16 | |
| `SettingsItems` family | `widgets::settings::*` | `gtk::ListBoxRow` rows inside `.boxed-list`; widgets in section 9.4 | |
| `TwoPanelBox` | `widgets::two_panel(start, end)` | `gtk::Paned` with `position` min(width/2, 450 px) | `Paned` lets the user resize; that is a deliberate desktop change (Proposal) |
| `WheelNumberPicker` / `WheelTextPicker` | `widgets::wheel_picker::{number, text}` | `gtk::ScrolledWindow` (no scrollbar) + snapping via `gtk::ScrollableExt` + `gtk::GestureDrag`; background `gtk::Frame` | section 9.5 |
| `AlertDialogContent` | `widgets::dialog_content(icon, title, text, buttons)` | `gtk::Box` (vertical) inside a `gtk::Window` (modal) with `.kuro-dialog` | section 9.6 |
| `TextButton` / `Button` | `widgets::button(label, kind)` where `kind` = Filled, Text | `gtk::Button` with `.suggested-action` (filled) or `.flat` (text) | long-press: `gtk::GestureLongPress` |
| `ExtendedFloatingActionButton` | `widgets::extended_fab(label, icon, expanded)` | `gtk::Button` `.kuro-fab` + `gtk::Revealer` for the label | section 9.7 |
| `IconToggleButton` | `widgets::icon_toggle(label, icon, checked, on_change)` | `gtk::ToggleButton` `.suggested-action` when checked, height 48 | |
| `Slider` (part 2) | used by `SliderItem` | `gtk::Scale` (horizontal, `round-digits 0`, `draw-value false`) | |

## 9. Behavior-bearing designs

### 9.1 Fast scroller

- Thumb: `gtk::DrawingArea`, 12 px wide, 48 px tall, rounded radius 6, color `@kuro_primary` (the Compose default `thumbColor = primary`).
- Overlay position: right edge, `margin-end` = `endContentPadding`, `margin-top` = `topContentPadding`.
- Visibility: `opacity` animated with `gtk::Widget::set_opacity` or a CSS transition on `opacity`. Show on scroll (adjustment `value-changed`), throttle the show signal to 100 ms, keep it visible 2000 ms, fade out over 250 ms. Immediate fade (250 ms, no delay) when `thumb_allowed()` is false.
- Show only when `visible_items < total_items` (list) or the adjustment's `upper - page_size > 0` (GTK equivalent of "content overflows").
- Drag: `gtk::GestureDrag` on the thumb; compute `ratio = (y - top) / track_len`, set the adjustment to `ratio * (upper - page_size)`. Keep the list formulas from the topic file as the reference behavior for the item-index mapping. GTK scrolls by pixels, so the index-to-offset math of the Compose version is not needed; use proportional pixel mapping and record that as an intended simplification (Proposal).
- Sticky headers: Kurobayomi lists without sticky headers ignore `STICKY_HEADER_KEY_PREFIX`. If a list adds sticky headers, use `gtk::ListView` section headers, and keep the rule "first non-header row" for the thumb proportion.

### 9.2 Adaptive sheet

- Tablet (`is_tablet` true): `gtk::Window` with `set_modal(true)`, `set_transient_for(main)`, `set_default_width` = 600 px landscape or 460 px portrait (choose by the window aspect ratio: width > height = landscape), `.kuro-dialog`. Fade in/out 350 ms via `Window::set_opacity` is not available in GTK4 for windows on all compositors; use a CSS `opacity` transition on the content root.
- Phone (`is_tablet` false): `gtk::Overlay` child with a `gtk::Revealer` (`transition-type = SlideUp`, `transition-duration = 350`) anchored to the bottom, width `min(460, window)` px, `.kuro-sheet`. Dismiss: scrim-less tap outside (as observed, no scrim; Proposal: add `rgba(0,0,0,0.0)` backdrop to catch clicks).
- Swipe dismiss (only when `enable_swipe_dismiss`): `gtk::GestureDrag` on the sheet top area (handle). `drag_update` moves the sheet by `dy` when `dy > 0` (downward), clamped at 0 at the top. `drag_end`: close when `offset_y > 56` or `velocity_y > 125` (px/s); otherwise snap back. Use `gtk::GestureDrag::offset` and an estimated velocity from timestamps.
- Back: `gtk::ShortcutController` with `Escape` (and the platform back equivalent, `Alt+Left`, not mapped in Compose; Proposal) calls dismiss.
- Dismiss callback fires once: guard with a `Cell<bool>`.

### 9.3 Progress ring

- Indeterminate (progress == 0): `gtk::Spinner` started.
- Determinate: `gtk::DrawingArea` with a `cairo` arc from -90 degrees, sweep `progress * 360`, stroke width 4, color `@kuro_primary`, plus a 2000 ms rotation (`add_tick_callback`, angle = `frame_time % 2000 / 2000 * 360`). The rotation applies to the whole ring, as in Compose (`Modifier.rotate(rotation)`).
- Progress smoothing: 200 ms ease-out (Proposal, replaces the M3 `ProgressAnimationSpec`).
- Crossfade between the two modes: `gtk::Stack` with `transition-type = Crossfade`, `transition-duration = 200`.

### 9.4 Settings rows

A `SettingsRow` is a `gtk::ListBoxRow` containing a horizontal `gtk::Box` (spacing 24 px, padding 10 px top/bottom, 24 px start/end, `valign center`). The label is `gtk::Label` with `.body-medium`. The trailing widget depends on the variant:

| Variant | Trailing | Activation |
|---|---|---|
| Icon | `gtk::Image` (primary) | row activated |
| Sort | `gtk::Image` `view-sort-descending-symbolic` (true) / `view-sort-ascending-symbolic` (false) / spacer 24 px (null) | row activated |
| Checkbox | `gtk::CheckButton` (insensitive; row activation toggles) | toggles the preference |
| Radio | `gtk::CheckButton` in a group (insensitive) | row activated |
| Slider | a column: label + `Pill` on a row, then `gtk::Scale` below (spacing 2 px); `value-changed` where value != previous calls `on_change` | |
| Select | `gtk::DropDown` (or `gtk::ComboBoxText`-free `gtk::DropDown` with a `StringList`) with the options; `selected` changes call `on_select` | |
| TriState | `gtk::Image` icon (`checkbox-symbolic` for DISABLED, `checkbox-checked-symbolic` for ENABLED_IS, `action-unavailable-symbolic` for ENABLED_NOT) + label; click cycles DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED | disabled when `enabled == false` or no callback |
| Numeric chooser | `gtk::Box` horizontal: minus `gtk::Button` (repeat), `gtk::Entry` (`input-purpose = digits`, min-width 140 px, placeholder, suffix label), plus `gtk::Button` (repeat) | repeat per section 9.8 |
| Text | `gtk::Entry` full width, margins 24 px / 4 px | |
| Chip row | heading + `gtk::FlowBox` (column spacing 8, margins 24/0/24/10) | |
| Icon grid | heading + `gtk::GridView` or `gtk::FlowBox` with min child width 128 px | the grid needs a bounded height: put it in a `gtk::ScrolledWindow` |

Rust signature sketch:

```rust
pub enum SettingsRowKind<'a> {
    Icon { icon: &'a str, on_click: Box<dyn Fn()> },
    Sort { desc: Option<bool>, on_click: Box<dyn Fn()> },
    Checkbox { checked: bool, on_toggle: Box<dyn Fn(bool)> },
    Radio { selected: bool, on_click: Box<dyn Fn()> },
    Slider { value: i32, range: std::ops::RangeInclusive<i32>, steps: Option<u32>, value_text: String, on_change: Box<dyn Fn(i32)> },
    Select { options: Vec<String>, selected: usize, on_select: Box<dyn Fn(usize)> },
    TriState { state: TriState, on_click: Option<Box<dyn Fn(TriState)>>, enabled: bool },
    Numeric { value: i32, step: i32, min: Option<i32>, on_change: Box<dyn Fn(i32)> },
    Text { value: String, on_change: Box<dyn Fn(String)> },
}
```

`steps` default = `(last - first) - 1` as in Compose (`SettingsItems.kt:215`).

### 9.5 Wheel picker

- Three visible rows of `height / 3` (128 / 3 = 42.67 px). Snap on scroll end: snap to the row nearest the center, animate 150 ms (Proposal).
- Alpha per row: the formula from the topic file (1.2 - d/single, else 0.2); GTK: set the row label `opacity` in a `scroll` callback. Clamp to [0, 1] explicitly.
- Selection event: fire `on_selected(index)` when the snapped index changes (not on construction, Proposal; the Compose version fires once, see E-49-09; the Rust default avoids the spurious callback and tests it).
- Manual input for the number variant: double-click or a `gtk::EditableLabel` inside the picker; Enter commits (exact match on `to_string()`; no match closes without change).
- Haptic: none on Linux (no portable haptics); drop with a note in the UI parity doc.

### 9.6 Dialog content

`dialog_content(icon, title, text, buttons)`:
- Window: `gtk::Window` modal, `.kuro-dialog`, width clamped 280..560 px.
- Column: icon (secondary color, centered, bottom margin 16) if present; title (headline small, color on_surface; centered when an icon exists, else start-aligned; bottom margin 16); text block (body medium, color on_surface_variant, margin 24 px horizontal, bottom 24); button row (end-aligned, margins 24 px start/end/bottom, label-large, primary color).
- The text block is a `gtk::ScrolledWindow` with `max-content-height` set to 60% of the window height when long (Proposal for GTK, the Compose one just wraps).

### 9.7 Extended FAB

- Collapsed: min width 56, icon only. Expanded: min width 80, padding start 12, label with padding start 12 end 20.
- Width transition: CSS `transition: min-width 500ms cubic-bezier(0.2, 0, 0, 1)` on `.kuro-fab`. Label `gtk::Revealer` (`transition-type = SlideRight`, `transition-duration = 500`) with an opacity fade of 200 ms, 100 ms delay on expand, and a 100 ms linear fade on collapse.
- Expanded state toggles on scroll direction in the caller (area 37/41), not here.

### 9.8 Repeat button

Implement with `glib::timeout_add_local_once` chains: on `pressed`, call `on_click()` immediately; then schedule the next call after `delay`, where `delay` starts at 750 ms and follows `next = max(5, (delay as f64 - delay as f64 * 0.25) as u64)`. Cancel the source on release, on `leave`, on `cancel`, and on drag beyond the widget (the Compose version cancels on ACTION_MOVE, which is stricter; Proposal: cancel on leave, documented as a change).

Expected click times: 0, 750, 1312, 1733, 2048, 2284, 2461, 2593, 2692, 2766, 2821, 2862, 2892, 2914, 2930, 2942, 2951, 2957, 2962, 2967, ... (cumulative sums of the schedule in `p1-topic-fast-scroll-and-wheel-picker.md` section 9).

Numeric chooser arithmetic: step up/down, clamp to `min` only for button presses (E-49-20, E-49-21). Typed text: strip characters not in `[-0-9.]`, parse; on failure keep the previous value and re-display it. Proposal: clamp typed values too and allow intermediate "-" (fixes E-49-20, E-49-21 and the reverting bug; record the deviation).

## 10. Threading

| Work | Thread | Mechanism |
|---|---|---|
| `resolve`, `css::render`, `classify` | any (pure) | plain functions |
| `install` (CssProvider), widget changes | GTK main thread | direct calls |
| Portal reads (`accent_color`, `color_scheme`, subscriptions) | tokio runtime | `ashpd` futures |
| Result delivery | GTK thread | `glib::spawn_future_local` awaiting a `oneshot`, or `glib::MainContext::channel` |
| Animation ticks | GTK thread | `add_tick_callback`, `timeout_add_local` |

No `Rc` crosses to tokio. Seed and contrast values are `Copy` data.

## 11. Crates

| Crate | Use | Why |
|---|---|---|
| `gtk4` 0.11.5 (features `v4_16`) | UI | already in the project |
| `glib` 0.22 | timeouts, futures, main context | already in the project |
| `ashpd` (latest 0.x compatible with the tokio runtime used by the app; pin at implementation) | Settings portal (accent color, color scheme, contrast) | the stable Linux interface for appearance; falls back gracefully |
| `tokio` | async portal calls | project-wide runtime (area 12) |
| `material-colors` (evaluate, section 6.3) | HCT and scheme generation | avoid porting if it matches |
| `serde` | prefs and tests fixtures | already in the project |

No Android/AndroidX crates. No CSS framework: the CSS is a generated string.

## 12. Android API -> Linux mapping

| Android / Compose | Kurobayomi (Linux) |
|---|---|
| `isSystemInDarkTheme()` | Settings portal `color-scheme` (section 4.4) |
| `AppCompatDelegate.setDefaultNightMode` | a `ThemeMode` pref; the portal value is read, never written |
| `DynamicColors.isDynamicColorAvailable()` | `monet::available()` |
| `dynamicLightColorScheme` / `dynamicDarkColorScheme` (Android 12+) | `monet::generate(seed, dark, contrast)` (section 6) |
| `WallpaperManager.getWallpaperColors` | dropped (no portable API); portal `accent-color` as seed |
| `UiModeManager.contrast` | portal `contrast` key |
| `LocalConfiguration.screenWidthDp` | window width / scale factor (section 7) |
| `LocalContext` / `Activity.setTheme` (XML themes in `ThemingDelegate`) | one CSS provider at APPLICATION priority |
| `RippleConfiguration` / `RippleAlpha` | CSS state layer alpha 0.10 on hover, focus-visible, active (section 4.3) |
| `MaterialTheme.shapes` | CSS radii (section 5) |
| `MaterialTheme.typography` | CSS font sizes (section 5) |
| `HapticFeedback` (`TextHandleMove`) | dropped; no portable haptics on the target desktops |
| `BackHandler` | `gtk::ShortcutController` (Escape), window close request |
| `LocalUriHandler.openUri` | `gtk::UriLauncher` (GTK 4.10+) |
| `systemGestureExclusion` | dropped (no Android gesture navigation) |
| `WindowInsets` / `systemBarsPadding` | dropped (no system bars inside the window); `gtk::Window` decorations are the desktop's |
| `ViewConfiguration.getScrollBarFadeDuration()` | constant 250 ms |
| `Paging 3` | Rust vectors and SQLite pages (area owners) |
| `MonetColorScheme.extractSeedColorFromImage` | dropped (dead code in Aniyomi) |

## 13. Error handling

- Missing portal (no `xdg-desktop-portal`): `system_dark` falls back to the GTK setting; Monet unavailable; no error dialog. Log at `info`.
- Portal call error: log `warn` with the error string; keep the last good value.
- Invalid accent values (NaN, out of range): ignore and keep the fallback.
- Unknown `pref_app_theme` value: Default, no log spam (one `debug` line).
- CSS parse errors (`gtk::CssProvider` `parsing-error` signal): log with the location; the generated CSS is test-validated (section 9 of the edge-case file), so this is a bug path, not a user path.

## 14. Pitfalls

1. `@define-color` names shared with the GTK theme (`accent_color`, `window_bg_color`) must be overwritten only in the "scheme override" block, and only when a Kurobayomi scheme is selected. Otherwise the user's Breeze look is lost.
2. CSS priority: APPLICATION (600) is below USER (800). A user `gtk.css` can override Kurobayomi tokens; that is intended.
3. Do not rebuild the window on theme change; re-render CSS only.
4. The omitted-token behavior (section 3) must use baseline values, not the primary color; a test covers it.
5. AMOLED applies only in dark mode; light mode ignores the flag.
6. The Monet tertiary-container role bug must not be ported.
7. Dark detection must not block the GTK main thread on a portal call.
8. Font metrics differ: keep text `ellipsize` and `max-lines` where Compose used `maxLines`.

## 15. Assumptions

- A-49-1: a Rust HCT/SchemeContent implementation can match `material-color-utilities` (verify with the vectors in the edge-case file).
- A-49-2: `org.freedesktop.appearance` `accent-color` is exposed by the user's Plasma portal. Verify with `busctl --user call org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop org.freedesktop.portal.Settings ReadOne ss org.freedesktop.appearance accent-color` before relying on it.
- A-49-3: the pinned androidx.compose.material3 baseline values can be dumped once and frozen.
