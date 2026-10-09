# Area 49 (part 2): Rust design for theme, presentation components and utils

Target: `/home/blackbox/kurobayomi` (crate `kurobayomi`, edition 2024, `gtk4` 0.11.5 with feature `v4_16`, `serde`, `serde_json`). The mockup has no CSS provider yet: all styling comes from stock GTK classes (`title`, `heading`, `dim-label`, `caption`, `boxed-list`, `navigation-sidebar`, `linked`, `flat`, `accent`, `caption-heading`, `title-3`) in `src/ui.rs` and `src/main.rs`. This design adds a theme layer on top of that.

Constraints from the request: GTK4 with no libadwaita; the user's GTK theme must still apply; stable Rust; tokio for network work; no emojis in UI.

Decision that needs the user's call (flagged, not silently chosen): the user's standing preference is Tokyo Night everywhere and never reset to stock. Aniyomi's default scheme is a Material blue. Proposal: add a Linux-only option "GTK theme" that applies no colour overrides, make it the default on Linux, and keep the 19 Aniyomi schemes as explicit overrides. The enum mapping in section 3.1 supports this without changing the Android parity list. Task T-49b-006 records the decision.

---

## 1. Module layout (new files)

```
src/
  theme/
    mod.rs          pub use of the items below, plus `Theme` (the runtime state)
    tokens.rs       Rgba, Role (38 variants), Tokens (fixed array), compositing helpers
    schemes.rs      static SCHEMES: generated constant tables (18 entries + Monet slot), fallbacks
    resolve.rs      resolve(app_theme, mode, amoled, system_dark) -> Tokens   (Compose parity)
    elevation.rs    tint_at(surface, surface_tint, dp) -> Rgba               (Surface.kt formula)
    css.rs          Tokens -> GTK CSS text (named colours + component classes)
    apply.rs        CssProvider lifecycle on the default Display
    portal.rs       XDG Settings portal watcher (colour-scheme, accent-colour)
    prefs.rs        AppTheme / ThemeMode / amoled read & write (area 02 store)
  ui/
    mod.rs          (existing helpers move here or stay in ui.rs; keep ui.rs API)
    helpers.rs      selected_background, secondary_alpha, clickable_no_indication,
                    enter_activates, focus_on_open, Insets
    scroll.rs       should_expand_fab(ScrollSnapshot) -> bool
    surface.rs      clickable surface (GestureClick + GestureLongPress)
    empty_state.rs  EmptyScreen equivalent, ERROR_FACES
    info_screen.rs  InfoScreen equivalent
    loading.rs      LoadingScreen equivalent
    slider.rs       IntSlider wrapper over gtk::Scale
    tabs.rs         tab label with count pill
    scaffold.rs     overlay layout: content, top bar, bottom bar, FAB, snackbar
    refresh.rs      refresh button and F5 accelerator (PullRefresh replacement)
    theme_picker.rs settings widget: mode row + 19 preview cards (+ Dynamic/Monet hidden)
  i18n.rs           gettext wrapper: t(), tn(), with the fixed() rule
  icons.rs          GResource-backed SVG icons (discord, github, magnet)
assets/
  icons/discord.svg, github.svg, magnet.svg
  icons.gresource.xml
po/                 gettext catalogs generated from the moko strings.xml keys
build.rs            calls glib_build_tools::compile_resources on assets/icons.gresource.xml
```

Existing files touched: `src/main.rs` (call `theme::Theme::install()` before `build_ui`; sidebar sizing in `sidebar()`), `src/ui.rs` (`row`, `status`, `page`, `icon_button` gain theme classes), `src/settings.rs` (theme rows read/write the prefs module), `src/spec.rs` (no change; the `"App theme"` chips option list stays as in the spec).

---

## 2. Core types

### 2.1 Colour and tokens

```rust
// src/theme/tokens.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }

impl Rgba {
    /// Parse Android-style 0xAARRGGBB (the XML and Kotlin literal form).
    pub const fn argb(v: u32) -> Self {
        Rgba { a: (v >> 24) as u8, r: (v >> 16) as u8, g: (v >> 8) as u8, b: v as u8 }
    }
    /// Alpha-scale this colour (Compose Color.copy(alpha = a)).
    pub fn with_alpha(self, a: f32) -> Self {
        Rgba { a: (a.clamp(0.0, 1.0) * 255.0).round() as u8, ..self }
    }
    /// Source-over compositing (Compose Color.compositeOver), defined below.
    pub fn over(self, dst: Rgba) -> Rgba { over_impl(self, dst) }
}
```

Compositing (exact):
```rust
fn over_impl(src: Rgba, dst: Rgba) -> Rgba {
    let sa = src.a as f32 / 255.0;
    let da = dst.a as f32 / 255.0;
    let oa = sa + da * (1.0 - sa);
    if oa <= 0.0 { return Rgba { r: 0, g: 0, b: 0, a: 0 }; }
    let ch = |s: u8, d: u8| -> u8 {
        let v = (s as f32 * sa + d as f32 * da * (1.0 - sa)) / oa;
        v.round().clamp(0.0, 255.0) as u8
    };
    Rgba { r: ch(src.r, dst.r), g: ch(src.g, dst.g), b: ch(src.b, dst.b), a: (oa * 255.0).round() as u8 }
}
```

The 38 roles (36 M3 + `primaryInverse`, `elevationOverlay`):
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Role {
    Primary, OnPrimary, PrimaryContainer, OnPrimaryContainer, InversePrimary,
    Secondary, OnSecondary, SecondaryContainer, OnSecondaryContainer,
    Tertiary, OnTertiary, TertiaryContainer, OnTertiaryContainer,
    Background, OnBackground, Surface, OnSurface, SurfaceVariant, OnSurfaceVariant,
    SurfaceTint, InverseSurface, InverseOnSurface,
    Error, OnError, ErrorContainer, OnErrorContainer,
    Outline, OutlineVariant, Scrim,
    SurfaceBright, SurfaceDim,
    SurfaceContainer, SurfaceContainerHigh, SurfaceContainerHighest,
    SurfaceContainerLow, SurfaceContainerLowest,
    // non-M3, XML only
    PrimaryInverseAlias, ElevationOverlay,
}
pub const ROLE_COUNT: usize = 38;

#[derive(Clone, Debug, PartialEq)]
pub struct Tokens {
    pub colors: [Rgba; ROLE_COUNT],
    pub amoled: bool,
    pub dark: bool,
}
impl Tokens {
    pub fn get(&self, r: Role) -> Rgba { self.colors[r as usize] }
}
```

`PrimaryInverseAlias` exists so the XML key `primaryInverse` can be stored; `InversePrimary` is the M3 role. Resolution treats the alias as `InversePrimary` when the latter is unset (see section 4). `ElevationOverlay` holds the resolved `primary` (the XML value is a reference), so it is stored as a resolved colour, not a reference.

### 2.2 Scheme tables

```rust
// src/theme/schemes.rs (generated by a script from the XML files; see task T-49b-002)
pub struct SchemeDef {
    pub id: &'static str,                       // "tachiyomi", "monochrome", "yotsuba", "tako", "cloudflare", "doom", "lavender", "matrix", "sapphire", "greenapple", "midnightdusk", "nord", "strawberry", "tealturquoise", "tidalwave", "yinyang", "cottoncandy", "mocha"
    pub light: [Option<Rgba>; ROLE_COUNT],      // None = not declared in the XML
    pub dark:  [Option<Rgba>; ROLE_COUNT],
}
pub static SCHEMES: &[(AppTheme, SchemeDef)] = &[ /* 18 entries, see section 3.1 */ ];
```

Generated literal for the Default scheme (all 38 entries per mode, `None` where the XML key is absent; `elevationOverlay` is always `None` because it is resolved from `Primary`):

```rust
(AppTheme::Default, SchemeDef { id: "tachiyomi",
    light:
    [
        Some(Rgba::argb(0xFF0058CA)), // Primary
        Some(Rgba::argb(0xFFFFFFFF)), // OnPrimary
        Some(Rgba::argb(0xFFD9E2FF)), // PrimaryContainer
        Some(Rgba::argb(0xFF001945)), // OnPrimaryContainer
        Some(Rgba::argb(0xFFB0C6FF)), // InversePrimary
        Some(Rgba::argb(0xFF0058CA)), // Secondary
        Some(Rgba::argb(0xFFFFFFFF)), // OnSecondary
        Some(Rgba::argb(0xFFD9E2FF)), // SecondaryContainer
        Some(Rgba::argb(0xFF001945)), // OnSecondaryContainer
        Some(Rgba::argb(0xFF006E1B)), // Tertiary
        Some(Rgba::argb(0xFFFFFFFF)), // OnTertiary
        Some(Rgba::argb(0xFF95F990)), // TertiaryContainer
        Some(Rgba::argb(0xFF002203)), // OnTertiaryContainer
        Some(Rgba::argb(0xFFFEFBFF)), // Background
        Some(Rgba::argb(0xFF1B1B1F)), // OnBackground
        Some(Rgba::argb(0xFFFEFBFF)), // Surface
        Some(Rgba::argb(0xFF1B1B1F)), // OnSurface
        Some(Rgba::argb(0xFFF3EDF7)), // SurfaceVariant
        Some(Rgba::argb(0xFF44464F)), // OnSurfaceVariant
        Some(Rgba::argb(0xFF0058CA)), // SurfaceTint
        Some(Rgba::argb(0xFF303034)), // InverseSurface
        Some(Rgba::argb(0xFFF2F0F4)), // InverseOnSurface
        Some(Rgba::argb(0xFFBA1A1A)), // Error
        Some(Rgba::argb(0xFFFFFFFF)), // OnError
        Some(Rgba::argb(0xFFFFDAD6)), // ErrorContainer
        Some(Rgba::argb(0xFF410002)), // OnErrorContainer
        Some(Rgba::argb(0xFF757780)), // Outline
        Some(Rgba::argb(0xFFC5C6D0)), // OutlineVariant
        None, // Scrim
        None, // SurfaceBright
        None, // SurfaceDim
        Some(Rgba::argb(0xFFF3EDF7)), // SurfaceContainer
        Some(Rgba::argb(0xFFFCF7FF)), // SurfaceContainerHigh
        Some(Rgba::argb(0xFFFCF7FF)), // SurfaceContainerHighest
        Some(Rgba::argb(0xFFF7F2FA)), // SurfaceContainerLow
        Some(Rgba::argb(0xFFF5F1F8)), // SurfaceContainerLowest
        None, // PrimaryInverseAlias
        None, // ElevationOverlay
    ]
    dark:
    [
        Some(Rgba::argb(0xFFB0C6FF)), // Primary
        Some(Rgba::argb(0xFF002D6E)), // OnPrimary
        Some(Rgba::argb(0xFF00429B)), // PrimaryContainer
        Some(Rgba::argb(0xFFD9E2FF)), // OnPrimaryContainer
        Some(Rgba::argb(0xFF0058CA)), // InversePrimary
        Some(Rgba::argb(0xFFB0C6FF)), // Secondary
        Some(Rgba::argb(0xFF002D6E)), // OnSecondary
        Some(Rgba::argb(0xFF00429B)), // SecondaryContainer
        Some(Rgba::argb(0xFFD9E2FF)), // OnSecondaryContainer
        Some(Rgba::argb(0xFF7ADC77)), // Tertiary
        Some(Rgba::argb(0xFF003909)), // OnTertiary
        Some(Rgba::argb(0xFF005312)), // TertiaryContainer
        Some(Rgba::argb(0xFF95F990)), // OnTertiaryContainer
        Some(Rgba::argb(0xFF1B1B1F)), // Background
        Some(Rgba::argb(0xFFE3E2E6)), // OnBackground
        Some(Rgba::argb(0xFF1B1B1F)), // Surface
        Some(Rgba::argb(0xFFE3E2E6)), // OnSurface
        Some(Rgba::argb(0xFF211F26)), // SurfaceVariant
        Some(Rgba::argb(0xFFC5C6D0)), // OnSurfaceVariant
        Some(Rgba::argb(0xFFB0C6FF)), // SurfaceTint
        Some(Rgba::argb(0xFFE3E2E6)), // InverseSurface
        Some(Rgba::argb(0xFF1B1B1F)), // InverseOnSurface
        Some(Rgba::argb(0xFFFFB4AB)), // Error
        Some(Rgba::argb(0xFF690005)), // OnError
        Some(Rgba::argb(0xFF93000A)), // ErrorContainer
        Some(Rgba::argb(0xFFFFDAD6)), // OnErrorContainer
        Some(Rgba::argb(0xFF8F9099)), // Outline
        Some(Rgba::argb(0xFF44464F)), // OutlineVariant
        None, // Scrim
        None, // SurfaceBright
        None, // SurfaceDim
        Some(Rgba::argb(0xFF211F26)), // SurfaceContainer
        Some(Rgba::argb(0xFF292730)), // SurfaceContainerHigh
        Some(Rgba::argb(0xFF302E38)), // SurfaceContainerHighest
        Some(Rgba::argb(0xFF1E1C22)), // SurfaceContainerLow
        Some(Rgba::argb(0xFF1A181D)), // SurfaceContainerLowest
        None, // PrimaryInverseAlias
        None, // ElevationOverlay
    ]
}),
```

The XML values are written as `#RRGGBB` (6 digits, opaque) or `#AARRGGBB` (8 digits, e.g. `#D849454E`). Parse them as ARGB exactly; do not assume opaque. Two examples are in appendix A of `p2-data-and-config.md`.

### 2.3 Runtime theme state

```rust
// src/theme/mod.rs
pub enum SystemScheme { Light, Dark, Unknown }

pub struct Theme {
    pub app_theme: AppTheme,        // see prefs
    pub mode: ThemeMode,            // Light | Dark | System
    pub amoled: bool,
    pub system: SystemScheme,       // from portal; Unknown until first answer
    pub accent: Option<Rgba>,       // portal accent-color, for the Dynamic option only
}
impl Theme {
    pub fn tokens(&self) -> Tokens { resolve::resolve(self) }
}
```

### 2.4 Enums

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppTheme {
    Default, Monet, Cloudflare, CottonCandy, Doom, GreenApple, Lavender, Matrix,
    MidnightDusk, Mocha, Sapphire, Nord, StrawberryDaiquiri, Tako, TealTurquoise,
    TidalWave, YinYang, Yotsuba, Monochrome,
    // Linux-only addition (decision T-49b-006): no colour overrides, follow the GTK theme
    GtkTheme,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode { Light, Dark, System }
```

`GtkTheme` is not an Aniyomi value. It is listed so the picker can offer it as the first card. Its stored name is `"GTK_THEME"`.

---

## 3. Resolution (Compose parity)

### 3.1 AppTheme to scheme
| `AppTheme` | Scheme source |
|---|---|
| Default | tachiyomi tables |
| Monet | no static table: `portal accent` seed (section 5.3); if no accent, Default |
| Cloudflare, CottonCandy, Doom, GreenApple, Lavender, Matrix, MidnightDusk, Mocha, Sapphire, Nord, StrawberryDaiquiri, Tako, TealTurquoise, TidalWave, YinYang, Yotsuba, Monochrome | their table |
| GtkTheme | no colour overrides (`Tokens` not applied to GTK named colours; only the mode is applied) |
| DARK_BLUE, HOT_PINK, BLUE (stored only) | never constructed: the prefs loader maps them to the default (section 5.1) |

### 3.2 Algorithm `resolve(theme) -> Tokens`
1. `dark = match theme.mode { Light => false, Dark => true, System => match theme.system { Dark => true, Light => false, Unknown => gtk_prefers_dark() } }`.
2. Pick the `SchemeDef` (table above). For Monet, derive a scheme from `accent` (out of scope for exact parity; see 5.3).
3. For each role `r` in 0..38: `value = if dark { def.dark[r] } else { def.light[r] }`; if `None`, use the **fallback** `fallback_for(r, dark)` (section 4).
4. Apply the alias: if `InversePrimary` is `None` and `PrimaryInverseAlias` is set, copy the alias. Set `ElevationOverlay` to the resolved `Primary` when absent.
5. If `dark && theme.amoled`: override `Background`, `Surface` to `#000000`; `OnBackground`, `OnSurface` to `#FFFFFF`; `SurfaceVariant`, `SurfaceContainerLowest`, `SurfaceContainerLow`, `SurfaceContainer` to `#0C0C0C`; `SurfaceContainerHigh` to `#131313`; `SurfaceContainerHighest` to `#1B1B1B`. (Exact copy of `BaseColorScheme.getColorScheme`.) AMOLED never applies in light mode.
6. Return `Tokens { colors, amoled: theme.amoled && dark, dark }`.

### 3.3 Active tint
`active()` returns `Rgba::argb(0xFFFFEB3B)` when `dark`, else `Rgba::argb(0xFFFFC107)`. `dark` is the resolved system/mode flag from step 1, which matches Compose `isSystemInDarkTheme()`. Store it as a token outside the 38 roles (`Tokens::active()`).

---

## 4. Fallback for missing roles

Compose fills any role a scheme does not pass with the M3 baseline (`darkColorScheme()` / `lightColorScheme()` of the pinned Compose M3 version). Which roles are missing per scheme is in `p2-topic-theme-tokens.md`, section 3 (for example Cloudflare, Doom, Matrix, Sapphire, Cotton Candy and Mocha leave 13 M3 roles to the baseline in Compose; the XML leaves a similar set empty).

Proposal: the same fallback table in Rust, generated once from the Compose artifact of the pinned version (task T-49b-004). Until that exists, the fallback is `Role` value taken from the matching `tachiyomi` (Default) table for the same mode. This keeps the palette coherent. It is not Compose-exact and must be labelled as a Linux deviation in the settings notes.

```rust
pub fn fallback_for(r: Role, dark: bool) -> Rgba {
    FALLBACK_BASELINE.get(dark as usize)
        .and_then(|t| t[r as usize])
        .unwrap_or_else(|| tachiyomi_table(dark)[r as usize].expect("default table is complete"))
}
```

The Default table must be complete (33 of 36 M3 roles plus aliases come from XML; `Scrim`, `SurfaceBright`, `SurfaceDim` are absent in XML and must be filled from the baseline). `FALLBACK_BASELINE` is generated with the schemes.

---

## 5. Application to GTK

### 5.1 Preferences (`src/theme/prefs.rs`)
- Read: `pref_app_theme` (enum name). Unknown names return the default, as Compose `getEnum` does. Linux default is `GtkTheme` (recommended, decision T-49b-006); the Android default (`Monet` or `Default`) is not used because `Monet` needs the Android wallpaper API. Deprecated names `DARK_BLUE`, `HOT_PINK`, `BLUE` are read as the default and written back as the default (migration; they never reach the picker).
- `pref_theme_mode_key`: `System` default.
- `pref_theme_dark_amoled_key`: `false` default.
- Writes go through the area 02 store and then call `Theme::reapply()`.

### 5.2 CSS provider lifecycle (`src/theme/apply.rs`)
```rust
pub struct ThemeCss { provider: gtk::CssProvider }
impl ThemeCss {
    pub fn install(display: &gdk::Display) -> Self {
        let provider = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        ThemeCss { provider }
    }
    pub fn set_css(&self, css: &str) { self.provider.load_from_string(css); }
}
```
- Priority `STYLE_PROVIDER_PRIORITY_APPLICATION` (600) overrides the theme (`THEME` = 200) and user CSS (`USER` = 800 is higher; the user's own `gtk.css` still wins, which is the intended behaviour).
- One provider is kept for the process. `load_from_string` replaces its content and GTK re-styles the widgets. Do not add a new provider per change (memory leak and style churn).
- Parse errors are reported by GTK on stderr and leave the previous rules in place; the code must not panic on them. Log the error with `glib::g_warning` and keep the old text.

### 5.3 Dynamic (Monet) on Linux
- The XDG portal `org.freedesktop.appearance` namespace provides `accent-color` (a `(ddd)` triple, 0.0 to 1.0) and `color-scheme` (`0` no preference, `1` dark, `2` light). Read through `ashpd` `Settings::accent_color()` and `Settings::color_scheme()`.
- Seed: if the portal returns an accent colour, derive a Material-like scheme from it. Algorithm: the Monet scheme is not reproducible without the Material Color Utilities (HCT). Proposal: crate `material-colors` or port the HCT path (`Hct::from_int` plus `SchemeContent`) only if the user wants parity. First cut: take the accent as `primary`, `secondary` as the same hue at 40% chroma, `surface*` from a neutral of the same hue. Mark as "approximate Dynamic". Task T-49b-005.
- If no accent, the option behaves as `Default`.

### 5.4 GTK named colour mapping (proposal)
`css.rs` emits `@define-color` for the stock GTK/Adwaita names so that the stock widgets follow the palette. Mapping:

| GTK named colour | Token |
|---|---|
| `window_bg_color` | `Background` |
| `window_fg_color` | `OnBackground` |
| `view_bg_color` | `Surface` |
| `view_fg_color` | `OnSurface` |
| `headerbar_bg_color` | `SurfaceContainer` |
| `headerbar_fg_color` | `OnSurface` |
| `card_bg_color` | `SurfaceContainerLow` |
| `card_fg_color` | `OnSurface` |
| `popover_bg_color` | `SurfaceContainerHigh` |
| `accent_color` | `Primary` |
| `accent_bg_color` | `Primary` |
| `accent_fg_color` | `OnPrimary` |
| `destructive_color` | `Error` |
| `destructive_bg_color` | `Error` |
| `destructive_fg_color` | `OnError` |
| `borders` | `Outline` |
| `dark_fill_color` | `SurfaceVariant` |

Only emitted when `AppTheme != GtkTheme`. For `GtkTheme` the provider is set to an empty string so the user's theme is untouched.

### 5.5 Component classes (generated CSS, appended to the named colours)
```css
/* elevation: a Surface at dp N uses a precomputed tinted background, see elevation.rs */
.kb-surface { transition: background-color 120ms cubic-bezier(0.4, 0, 0.2, 1); }
.kb-surface:active { transition-duration: 150ms; transition-timing-function: cubic-bezier(0.4, 0, 0.6, 1); }
.kb-surface:hover { transition-duration: 120ms; }

/* selected row: secondary at 0.16 (dark) or 0.22 (light) */
.kb-selected { background-color: @SECONDARY_ALPHA_SELECTED; }

/* secondary item alpha 0.78 */
.kb-secondary-item { opacity: 0.78; }

/* header text style: bodyMedium, onSurfaceVariant, 600 */
.kb-header { font-weight: 600; color: @SURFACE_VARIANT_TEXT; }

/* active filter tint */
.kb-active { color: @ACTIVE_TINT; }

/* pill: onBackground at 0.12 dark or 0.08 light, 10pt */
.kb-pill { background-color: @PILL_BG; border-radius: 999px; padding: 1px 6px; font-size: 10pt; margin-start: 4px; }
```
`@SECONDARY_ALPHA_SELECTED`, `@PILL_BG` and similar are literal `rgba(r, g, b, a)` values written by `css.rs` (the generator substitutes them; GTK CSS does not compute them from named colours in a portable way). Names in the output are concrete colours.

Opacity note: GTK `opacity` on a widget affects its whole subtree, which matches `secondaryItemAlpha()` (Compose `alpha` modifier). Use `opacity`, not a colour alpha.

---

## 6. Threading and async model

- GTK is single-threaded. Every widget and `CssProvider` call runs on the main thread (`glib::MainContext::default()`).
- Portal calls (`ashpd`) are async. Run them with `glib::spawn_future_local` on the main loop, since ashpd is executor-agnostic and the result is needed on the main thread anyway. Do not block the main thread on them.
- Network and DB work for the rest of the app uses the tokio runtime (per the request). Theme code never touches tokio.
- Preference change notifications: the prefs store (area 02) publishes changes; the bridge (`ui/observe.rs`, T-49b-035) forwards them into `glib::MainContext::channel`. Widgets subscribe on the main thread.
- `Theme::reapply()` runs on the main thread and is idempotent.

---

## 7. Crates

| Crate | Use | Why |
|---|---|---|
| `gtk4` 0.11.5 (features `v4_16`) | all widgets, `CssProvider`, `GestureLongPress`, `GestureClick`, `Overlay`, `Scale`, `Spinner` | already in `Cargo.toml` |
| `gdk4` (via gtk) | `Display`, `RGBA` helpers | comes with gtk |
| `glib` (via gtk) | `MainContext`, `g_warning`, `spawn_future_local` | comes with gtk |
| `ashpd` (current 0.x line) | XDG Settings portal: `color-scheme`, `accent-color`, change signals | the only portable way to read the desktop colour scheme without libadwaita; works on KDE Plasma |
| `gettext-rs` (0.7 line) | translated strings (`t`, `tn`) | links the system gettext; catalogs from `po/` |
| `glib-build-tools` (matching glib 0.2x) | compile `assets/icons.gresource.xml` in `build.rs` | GResource SVG icons |
| `serde`, `serde_json` | already present; used by the spec loader, not by theme | no new use |

Not used: libadwaita (forbidden), `palette` (not needed: only `over`, `with_alpha` and the 1/4-channel math above), `once_cell` (use `std::sync::OnceLock` for the static fallback tables), `material-colors` (only if T-49b-005 chooses exact Monet).

---

## 8. Error handling
- Scheme lookup cannot fail: `AppTheme` is exhaustive; `SCHEMES` covers all static variants (compile-time check with a `match` in `schemes.rs`).
- Preference parse: unknown enum names fall back to the default (Compose parity, `getEnum`). No error is shown.
- CSS load failure: keep the previous provider text; log once with `glib::g_warning!`.
- Portal failure (no portal running, or the call returns an error): `SystemScheme::Unknown`; `dark` falls back to `gtk::Settings::default().gtk_application_prefer_dark_theme()`. Log at debug level. Never block the UI.
- Missing icon resource: `icons.rs` returns the symbolic fallback `image-missing`.
- Gettext missing catalog: returns the msgid (English source text).

---

## 9. Android to Linux mapping

| Android / Compose API | Linux equivalent here |
|---|---|
| `MaterialTheme(colorScheme)` | `Tokens` plus `ThemeCss` (named colours) |
| `isSystemInDarkTheme()` | portal `color-scheme`, fallback GTK `prefer-dark` |
| `AppCompatDelegate.setDefaultNightMode` | `gtk::Settings::set_gtk_application_prefer_dark_theme(bool)` (only effective for themes that have dark variants) |
| `DynamicColors` / `dynamicLightColorScheme` | portal `accent-color`, approximate scheme (T-49b-005) |
| `WallpaperManager` seed | not available: use the portal accent instead (no wallpaper access on Wayland) |
| `ViewConfiguration.scaledScrollBarSize` | GTK overlay scrollbar (no custom drawing) |
| `ViewConfiguration.getScrollBarFadeDuration()` | GTK internal overlay fade |
| `Modifier.combinedClickable` | `GestureClick` (primary button) and `GestureLongPress` |
| `ripple()` | GTK `:hover`, `:active`, `:focus` CSS states (transitions in CSS) |
| `minimumInteractiveComponentSize()` (48 dp) | `min-width`/`min-height: 48px` CSS on the hit box (Linux pointer targets are smaller) |
| `onPreviewKeyEvent` Enter/NumPadEnter | `gtk::EventControllerKey` in capture phase, keys `Return` and `KP_Enter` |
| `WindowInsets.isImeVisible` | none (no IME); helper dropped |
| `PullToRefreshDefaults.Indicator` | `gtk::Spinner` in the header bar + F5 action |
| `CircularProgressIndicator` | `gtk::Spinner` |
| `Scaffold` / `SubcomposeLayout` | `gtk::Overlay` (content child, overlay children for FAB and snackbar) |
| `SnackbarHost` | `gtk::Revealer` (bottom-centre) over the overlay |
| `R.color.*` in XML (View theme) | `schemes.rs` constants (no XML at runtime) |
| `R.drawable.*` vector icons | `assets/icons/*.svg` in a GResource |
| `moko-resources` `StringDesc` | gettext via `gettext-rs` |
| `DeviceUtil.isDynamicColorAvailable` | "portal returns an accent-color" |
| `Build.VERSION` checks | not applicable |
| `Preference<T>.changes()` | `glib::MainContext::channel` bridge |

---

## 10. Integration with the existing mockup

- `src/main.rs::build_ui` (line 22): after creating the window, call `theme::Theme::install(&display)` and `theme::Theme::load_prefs()`, then `reapply`. Install before `win.present()`.
- `src/main.rs::sidebar` (line 154): use the rail geometry from section 5 of the behavior file (min width 80 px, 4 px vertical padding, 4 px item gap). The mockup sidebar currently has no fixed minimum; add `set_size_request(80, -1)` only if the mockup looks wrong at small widths.
- `src/ui.rs::row`, `status`, `page`: add `kb-header`, `kb-surface` classes where the Compose originals use them (see the UI parity file).
- `src/ui.rs::linked` (segmented control): the theme-mode row (System, Light, Dark) reuses it.
- `spec/settings.json` and `spec/app.json`: the `App theme` chips already list the 19 names. Add `GTK theme` as the first option (decision T-49b-006) and set the default per the decision. The `"Pure black dark mode"` switch stays `"default": false`, and it becomes insensitive when the mode row is Light (the Compose rule).
- `src/settings.rs`: the appearance rows read and write the prefs module, not the spec's `default` (the spec default is only the initial render in the mockup).
- No change to `src/clamp.rs`, `src/entry.rs`, `src/player.rs`.

---

## 11. Pitfalls
- `@define-color` names must not clash with GTK's own names or with the user's `gtk.css`; use the stock names above only, and keep the list in one constant.
- GTK CSS does not support every CSS colour function the same way across 4.16 point releases. Emit `rgba(r, g, b, a)` with 0 to 1 alpha and 0 to 255 channels, never `hsl()` or `color-mix()`.
- `load_from_string` on the same provider re-parses everything. Keep the CSS under ~20 KB; cache the last string and skip identical reloads.
- The Compose `surface` check (`color == MaterialTheme.colorScheme.surface`) means only default containers get tinted. In Rust, pass an explicit `tint: bool` to the surface builder instead of comparing colours.
- AMOLED surfaces are pure black: GTK's dark popovers and menus still come from the GTK theme unless `popover_bg_color` is overridden (it is, in 5.4).
- The Dynamic option must be hidden when the portal gives no accent colour (Compose hides it via the `isDynamicColorAvailable` gate; the picker must not show a dead card).
- `Theme::reapply()` must not run during widget construction (re-entrancy with `set_child`); defer with `glib::idle_add_local_once`.
