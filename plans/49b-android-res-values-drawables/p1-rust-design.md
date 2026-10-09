# Area 49b (part 1): Rust/GTK design for Android resources (values, drawables, colour selector)

Target: Kurobayomi (`/home/blackbox/kurobayomi`), gtk4-rs 0.11 with the `v4_16` feature (see `Cargo.toml`: `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`), no libadwaita, no tokio in this area. Every Android resource in scope maps to one of: a Rust constant, a CSS rule in a GResource-bundled stylesheet, an exported SVG icon, or a GTK widget/API call. Nothing in this area needs a runtime parser for Android XML.

## 1. Crate and module layout

```
kurobayomi/
  build.rs                      (new) compiles data/kurobayomi.gresource.xml -> $OUT_DIR/kurobayomi.gresource
  data/
    kurobayomi.gresource.xml    (new) bundles style.css, icons/, splash resources
    style.css                   (new) all CSS rules for this area (section 5)
    icons/hicolor/scalable/actions/kb-*-symbolic.svg         (new, 30-odd files, section 4)
    icons/hicolor/scalable/apps/io.github.blackbox.Kurobayomi.svg   (new)
    icons/hicolor/symbolic/apps/io.github.blackbox.Kurobayomi-symbolic.svg (new)
    io.github.blackbox.Kurobayomi.desktop   (new, desktop actions, section 7)
  src/
    main.rs        (existing) calls theme::install and icons::register in build_ui before the window is shown
    theme.rs       (new) AppTheme enum, Palette struct, palette constants, CSS string builders, install()
    icons.rs       (new) Icon enum -> icon name, register(), resolution test helper
    anim.rs        (new) crossfade / rotate / timeline helpers (GTK main loop only)
    splash.rs      (new) splash page and the 500/5000/400 ms state machine
    ui.rs          (existing) cover(), menu_button(), linked(), row(), status() switch to icons::Icon
    player.rs      (existing) reader/player controls switch to icons::Icon and the seek-triangle overlay
    entry.rs       (existing) download/entry icons and the caret expander
    settings.rs    (existing) theme picker uses theme::AppTheme
```

`src/main.rs` currently has no `CssProvider` (verified: `grep -rn CssProvider src` returns nothing), so the theme install call is new.

## 2. Key types

### 2.1 Theme choice (`src/theme.rs`)

```rust
/// Mirrors eu.kanade.domain.ui.model.AppTheme. Only the values that Kurobayomi implements
/// are variants; every other stored value falls back to Default (see section 2.1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppTheme {
    /// Follow the user's GTK theme (no palette overrides). Default on Linux.
    Default,
    /// Android Theme.Tachiyomi.CottonCandy (palette COTTONCANDY_LIGHT / COTTONCANDY_DARK).
    CottonCandy,
    /// Android Theme.Tachiyomi.Mocha (palette MOCHA_LIGHT / MOCHA_DARK).
    Mocha,
}

impl AppTheme {
    /// Maps the stored preference string (Android enum name) to a variant.
    /// Unknown names, MONET, and the deprecated DARK_BLUE / HOT_PINK / BLUE map to Default.
    pub fn from_pref(name: &str) -> Self {
        match name {
            "COTTONCANDY" => Self::CottonCandy,
            "MOCHA" => Self::Mocha,
            _ => Self::Default,
        }
    }
    pub fn pref_name(self) -> &'static str {
        match self {
            Self::Default => "DEFAULT",
            Self::CottonCandy => "COTTONCANDY",
            Self::Mocha => "MOCHA",
        }
    }
}
```

#### 2.1.1 Stored-value compatibility

Android stores `AppTheme` as `name()` (behavior section 3.1). Kurobayomi stores the same strings in its own settings file so that a future import from an Aniyomi backup maps directly. Unknown strings and the 19 unimplemented constants (every `AppTheme` other than `DEFAULT`, `COTTONCANDY`, `MOCHA`, including the deprecated `DARK_BLUE`, `HOT_PINK`, `BLUE`) map to `Default`, the same fallback that `ThemingDelegate.getThemeResIds` uses (`getOrDefault(..., Theme_Tachiyomi)`).

### 2.2 Palette (`src/theme.rs`)

```rust
/// ARGB 0xAARRGGBB, one field per Android colour token used by the scope palettes.
/// Field names match the XML suffixes exactly (behavior section 1.10, data section 1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub primary: u32,
    pub on_primary: u32,
    pub primary_container: u32,
    pub on_primary_container: u32,
    pub secondary: u32,
    pub on_secondary: u32,
    pub secondary_container: u32,
    pub on_secondary_container: u32,
    pub tertiary: u32,
    pub on_tertiary: u32,
    pub tertiary_container: u32,
    pub on_tertiary_container: u32,
    pub background: u32,
    pub on_background: u32,
    pub surface: u32,
    pub on_surface: u32,
    pub surface_variant: u32,
    pub on_surface_variant: u32,
    pub outline: u32,
    pub inverse_on_surface: u32,
    pub inverse_surface: u32,
    pub primary_inverse: u32,
}
```

Palette constants (values taken from the XML by a script run for this plan, with two deliberate corrections that match the Compose `ColorScheme` used by the UI; see section 6):

```rust
pub const COTTONCANDY_LIGHT: Palette = Palette {
    primary: 0xFF9A4058,
    on_primary: 0xFFFFFFFF,
    primary_container: 0xFFFFD9DF,
    on_primary_container: 0xFF3F0017,
    secondary: 0xFF5BCEFA,
    on_secondary: 0xFFFFFFFF,
    secondary_container: 0xFFBCE9FF,
    on_secondary_container: 0xFF001F2A,
    tertiary: 0xFF9A4058,
    on_tertiary: 0xFFFFFFFF,
    tertiary_container: 0xFFFFD9DF,
    on_tertiary_container: 0xFF3F0017,
    background: 0xFFFFFBFF,
    on_background: 0xFF201A1B,
    surface: 0xFFFFFBFF,
    on_surface: 0xFF201A1B,
    surface_variant: 0xFFF3DDE0,
    on_surface_variant: 0xFF524345,
    outline: 0xFF847375,
    inverse_on_surface: 0xFFFAEEEF,
    inverse_surface: 0xFF352F30,
    primary_inverse: 0xFFFFB1C1,
};

pub const COTTONCANDY_DARK: Palette = Palette {
    primary: 0xFFFFB1C1,
    on_primary: 0xFF5F112B,
    primary_container: 0xFF7C2941,
    on_primary_container: 0xFFFFD9DF,
    secondary: 0xFF64D3FF,
    on_secondary: 0xFF003546,
    secondary_container: 0xFF004D63,
    on_secondary_container: 0xFFBCE9FF,
    tertiary: 0xFFFFB1C1,
    on_tertiary: 0xFF5F112B,            // CORRECTION: XML night file lacks this token (resolves to #FFFFFF); Compose uses 0xFF5F112B
    tertiary_container: 0xFF7C2941,
    on_tertiary_container: 0xFFFFD9DF,
    background: 0xFF201A1B,
    on_background: 0xFFECE0E0,
    surface: 0xFF201A1B,
    on_surface: 0xFFECE0E0,
    surface_variant: 0xFF524345,
    on_surface_variant: 0xFFD6C2C4,
    outline: 0xFF9F8C8F,
    inverse_on_surface: 0xFF201A1B,
    inverse_surface: 0xFFECE0E0,
    primary_inverse: 0xFF9A4058,
};

pub const MOCHA_LIGHT: Palette = Palette {
    primary: 0xFF745B00,
    on_primary: 0xFFFFFFFF,
    primary_container: 0xFFFFE08D,
    on_primary_container: 0xFF241A00,
    secondary: 0xFF745B00,
    on_secondary: 0xFFFFFFFF,
    secondary_container: 0xFFFFE08D,
    on_secondary_container: 0xFF241A00,
    tertiary: 0xFF47664A,
    on_tertiary: 0xFFFFFFFF,
    tertiary_container: 0xFFC9ECC8,
    on_tertiary_container: 0xFF04210B,
    background: 0xFFFFFBFF,
    on_background: 0xFF1E1B16,
    surface: 0xFFFFFBFF,
    on_surface: 0xFF1E1B16,
    surface_variant: 0xFFEBE1CF,
    on_surface_variant: 0xFF4C4639,
    outline: 0xFF7E7667,
    inverse_on_surface: 0xFFFBEAEB,     // CORRECTION: XML says FBEEEB; Compose MochaColorScheme says FBEAEB
    inverse_surface: 0xFF362F2D,
    primary_inverse: 0xFFFFB59D,
};

pub const MOCHA_DARK: Palette = Palette {
    primary: 0xFFEBC248,
    on_primary: 0xFF3D2F00,
    primary_container: 0xFF584400,
    on_primary_container: 0xFFFFE08D,
    secondary: 0xFFEBC248,
    on_secondary: 0xFF3D2F00,
    secondary_container: 0xFF584400,
    on_secondary_container: 0xFFFFE08D,
    tertiary: 0xFFADCFAD,
    on_tertiary: 0xFF19361F,
    tertiary_container: 0xFF304D34,
    on_tertiary_container: 0xFFC9ECC8,
    background: 0xFF1E1B16,
    on_background: 0xFFE8E1D9,
    surface: 0xFF1E1B16,
    on_surface: 0xFFE8E1D9,
    surface_variant: 0xFF4C4639,
    on_surface_variant: 0xFFCFC5B4,
    outline: 0xFF989080,
    inverse_on_surface: 0xFF211A18,
    inverse_surface: 0xFFEDE0DD,
    primary_inverse: 0xFFAE3200,
};
```

Note on the source of truth: the XML and the Compose schemes disagree in exactly two tokens (section 6). The design uses the Compose values because that is what the Android UI shows. The XML values are kept in `p1-edge-cases-and-tests.md` for the drift test.

### 2.3 Icon (`src/icons.rs`)

```rust
/// Every Android drawable in scope that Kurobayomi still shows is one variant here.
/// Variants map to either a freedesktop/GTK icon name or a custom kb-* symbolic icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    // Standard names (Replace decisions in the inventory)
    Close, Play, Pause, SkipNext, SkipPrevious, Refresh, Share, Done, Info, Warning,
    Book, Folder, Extension, Photo, Server, UpdateAvailable, Calendar, DragHandle,
    CaretDown, CaretUp, MoreHorizontal, GoBack, HistoryTab, Cover,
    // Custom symbolic icons (Port decisions in the inventory)
    AnimeLibraryOutline, AnimeLibraryFilled, LibraryManga, UpdatesTab, Compass,
    DownloadCircle, DoneMarkPrevious, OverflowVertical, Glasses,
    ReaderDefault, ReaderLtr, ReaderRtl, ReaderVertical, ReaderContinuousVertical, ReaderWebtoon,
    CropOn, CropOff, SeekTriangle, Forward10, TextShadow, FontFamily,
    Logo, LogoSymbolic,
}

impl Icon {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Close => "window-close-symbolic",
            Self::Play => "media-playback-start-symbolic",
            Self::Pause => "media-playback-pause-symbolic",
            Self::SkipNext => "media-skip-forward-symbolic",
            Self::SkipPrevious => "media-skip-backward-symbolic",
            Self::Refresh => "view-refresh-symbolic",
            Self::Share => "document-send-symbolic",
            Self::Done => "object-select-symbolic",
            Self::Info => "dialog-information-symbolic",
            Self::Warning => "dialog-warning-symbolic",
            Self::Book => "accessories-dictionary-symbolic",
            Self::Folder => "folder-symbolic",
            Self::Extension => "application-x-addon-symbolic",
            Self::Photo => "image-x-generic-symbolic",
            Self::Server => "network-server-symbolic",
            Self::UpdateAvailable => "software-update-available-symbolic",
            Self::Calendar => "x-office-calendar-symbolic",
            Self::DragHandle => "list-drag-handle-symbolic",
            Self::CaretDown => "pan-down-symbolic",
            Self::CaretUp => "pan-up-symbolic",
            Self::MoreHorizontal => "view-more-horizontal-symbolic",
            Self::GoBack => "go-previous-symbolic",
            Self::HistoryTab => "document-open-recent-symbolic",
            Self::Cover => "image-x-generic-symbolic",
            Self::AnimeLibraryOutline => "kb-animelibrary-outline-symbolic",
            Self::AnimeLibraryFilled => "kb-animelibrary-filled-symbolic",
            Self::LibraryManga => "kb-library-symbolic",
            Self::UpdatesTab => "kb-updates-outline-symbolic",
            Self::Compass => "kb-compass-symbolic",
            Self::DownloadCircle => "kb-download-circle-symbolic",
            Self::DoneMarkPrevious => "kb-done-previous-symbolic",
            Self::OverflowVertical => "kb-overflow-vertical-symbolic",
            Self::Glasses => "kb-glasses-symbolic",
            Self::ReaderDefault => "kb-reader-default-symbolic",
            Self::ReaderLtr => "kb-reader-ltr-symbolic",
            Self::ReaderRtl => "kb-reader-rtl-symbolic",
            Self::ReaderVertical => "kb-reader-vertical-symbolic",
            Self::ReaderContinuousVertical => "kb-reader-continuous-vertical-symbolic",
            Self::ReaderWebtoon => "kb-reader-webtoon-symbolic",
            Self::CropOn => "kb-crop-symbolic",
            Self::CropOff => "kb-crop-off-symbolic",
            Self::SeekTriangle => "kb-seek-triangle-symbolic",
            Self::Forward10 => "kb-forward-10-symbolic",
            Self::TextShadow => "kb-text-shadow-symbolic",
            Self::FontFamily => "kb-font-family-symbolic",
            Self::Logo => "io.github.blackbox.Kurobayomi",
            Self::LogoSymbolic => "io.github.blackbox.Kurobayomi-symbolic",
        }
    }
}

pub const APP_ID: &str = "io.github.blackbox.Kurobayomi";
pub const ICON_RESOURCE_PATH: &str = "/io/github/blackbox/Kurobayomi/icons";

/// Adds the bundled icon directory to the default display's icon theme.
/// Must run before the first window is shown.
pub fn register(display: &gtk::gdk::Display) {
    gtk::IconTheme::for_display(display).add_resource_path(ICON_RESOURCE_PATH);
}
```

Full mapping rationale (which Android drawable becomes which `Icon` variant) is in the inventory. Icons that are not `Port` are looked up in the active theme, so the user's icon theme (Tokyo Night GTK plus the system icon theme) decides their look. Custom `kb-*` icons are symbolic, so they recolour with the theme foreground.

## 3. Threading and async model

- Everything in this area runs on the GTK main loop. No `tokio` and no worker threads: CSS loading, icon registration, animations and the splash timer all touch widgets.
- Splash timing uses `glib::timeout_add_local` (fires on the main loop) or `gtk::Widget::add_tick_callback` (frame clock). The state machine is driven by `Instant` deltas, not by timer counts, to match the Android `elapsed` logic.
- The reader/player double-tap triangle loop is a `glib::timeout_add_local(16 ms)` that re-computes three opacities from the 150 ms step schedule; it is removed when the overlay is hidden (`glib::ControlFlow::Break`), matching `LaunchedEffect` cancellation.
- No file IO at runtime: the icon and CSS bytes come from the GResource bundle compiled into the binary.

## 4. Icon pipeline (Android VectorDrawable to GTK symbolic SVG)

1. One-time offline export (a script under `tools/`, run by hand, output committed): read each `Port` drawable from `app/src/main/res/drawable/` (paths in the inventory), take `android:viewportWidth`/`Height` as the SVG `viewBox`, copy each `android:pathData` verbatim into a `<path d="...">`, and set `fill="#bebebe"` (the GTK symbolic recolour key; GTK replaces `#bebebe` with the widget foreground). Strip the Android `android:fillColor` and keep `fill-rule` only where `android:fillType="evenOdd"`.
2. Verify each exported file renders at 16 px and 24 px with `rsvg-convert` (or Inkscape) and inspect visually against the Android render.
3. Check: `gtk::IconTheme::has_icon(name)` is true for each `Icon` variant (test in section 8).
4. Launcher icon: `ic_launcher_foreground.xml` is full colour, not symbolic. Export it as `scalable/apps/io.github.blackbox.Kurobayomi.svg` with its original colours (no `#bebebe` recolour). The adaptive background is dropped (behavior 4.2 and inventory).
5. Monochrome launcher `ic_ani_monochrome_launcher.xml` and logo `ic_ani.xml` export as `-symbolic.svg` with `#bebebe` fills (the `ic_ani.xml` second path and first path both fill `#FFFFFF` in Android; in the symbolic export both become `#bebebe`).

Custom symbolic icons added beyond the inventory's own rows (all sourced from the named Android file's base state):
- `kb-compass-symbolic`: base-state `compass` path plus `ring` from `app/src/main/res/drawable/anim_browse_enter.xml` (used as the Browse tab icon).
- `kb-library-symbolic`: base-state `bookmark` path from `app/src/main/res/drawable/anim_library_enter.xml` (viewport 1080; used as the manga library icon). The base `angle` path is included.
- `kb-animelibrary-outline-symbolic` and `kb-animelibrary-filled-symbolic`: from the two selector items (`ic_animelibrary_outline_24dp.xml`, `ic_animelibrary_filled_24dp.xml`).

## 5. Stylesheet (`data/style.css`, bundled, loaded at PRIORITY_APPLICATION)

GTK CSS rules that replace Android style items. All colour references go through GTK theme variables so the user's theme applies. The only literal colours are the ones that are literal in Android: cover placeholder alpha, splash background, and the AMOLED values.

```css
/* Cover placeholder: values/colors via presentation-core cover_placeholder = #1F888888 */
.kb-cover-placeholder {
  background-color: rgba(136, 136, 136, 0.1216);   /* 0x1F / 255 */
}

/* Divider: values/themes.xml android:divider -> divider_default (#1F000000 light) */
.kb-divider, separator.kb-divider {
  min-height: 1px;
  background-color: alpha(@theme_fg_color, 0.12);
}

/* Section header: TextAppearance.Tachiyomi.SectionHeader = labelLarge + textColorSecondary */
.kb-section-header {
  font-size: 14px;
  font-weight: 500;
  color: alpha(@theme_fg_color, 0.7);
}

/* Preference secondary text: TextAppearance.Tachiyomi.Preference.Secondary = bodySmall */
.kb-preference-secondary {
  font-size: 12px;
  color: alpha(@theme_fg_color, 0.7);
}

/* Widget.Button text: TextAppearance.Widget.Button = titleSmall */
.kb-button-text {
  font-size: 14px;
  font-weight: 500;
}

/* Dragged or activated download card: draggable_card_foreground (alpha 0.08 of colorPrimary) */
.kb-draggable.kb-dragged,
.kb-draggable:selected {
  background-image: linear-gradient(alpha(@theme_selected_bg_color, 0.08), alpha(@theme_selected_bg_color, 0.08));
}

/* PROPOSAL. Ripple/highlight: values/themes.xml colorControlHighlight = colorSurfaceVariant (an opaque
   palette colour, not reproducible exactly with GTK defaults). The 0.1 alpha is RIPPLE_*_ALPHA from
   TachiyomiTheme.kt (player ripple). */
.kb-highlight:hover {
  background-color: alpha(@theme_fg_color, 0.1);
}

/* Splash: values/themes.xml windowSplashScreenBackground = splash = #54759E (light), #202125 (dark) */
.kb-splash {
  background-color: #54759E;
}
.kb-splash.kb-dark {
  background-color: #202125;
}

/* AMOLED overlay: values-night/themes.xml ThemeOverlay.Tachiyomi.Amoled (dark only) */
.kb-dark.kb-amoled window,
.kb-dark.kb-amoled .background {
  background-color: #000000;
  color: #FFFFFF;
}
.kb-dark.kb-amoled .card,
.kb-dark.kb-amoled list.boxed-list {
  background-color: #0C0C0C;
}

/* Modal dim: values/themes.xml android:backgroundDimAmount = 0.32 */
.kb-scrim {
  background-color: rgba(0, 0, 0, 0.32);
}

/* Reader background colours are owned by area:44-reader-viewers (not in this scope). Only the class names
   .kb-reader-bg-white / -gray / -black / -auto are defined here, with values supplied by that area. */
```

Notes on the CSS:
- Rotation classes used by `anim::rotate_icon` (add to style.css): `.kb-icon-rot { transition: -gtk-icon-transform 300ms cubic-bezier(0.4, 0, 0.2, 1); }`, `.kb-rot-180 { -gtk-icon-transform: rotate(180deg); }`, `.kb-icon-rot.kb-rot-0-slow { transition-duration: 500ms; -gtk-icon-transform: rotate(0deg); }`. The slow variant starts from `rotate(360deg)` when first applied (anim_history_enter).
- GTK CSS has no `@media (prefers-color-scheme: dark)`. Dark mode is signalled by adding `kb-dark` to the window when `gtk::Settings::is_gtk_application_prefer_dark_theme()` is true or the window background is dark. The AMOLED class `kb-amoled` is added only when `kb-dark` is also present (behavior 1.4: the flag has no effect in light mode).
- `alpha()`, `@theme_*` and `linear-gradient()` are GTK CSS features. `@theme_bg_color`, `@theme_fg_color`, `@theme_base_color`, `@theme_selected_bg_color`, `@theme_selected_fg_color` are defined by GTK's default theme. Use only these named colours in this stylesheet (the user's GTK theme defines them). Do not reference `@accent_bg_color` or `@card_bg_color` unless the verification in section 8 proves the Tokyo Night theme defines them.
- A reference to an undefined `@name` makes GTK drop the whole rule with a parse warning. Test 1 in section 8 guards against that.

### 5.1 Palette override (only when the user picks CottonCandy or Mocha)

Generated at runtime from the `Palette` constants, loaded at PRIORITY_USER so it wins over the theme and over `style.css`:

```rust
pub fn palette_css(p: &Palette) -> String {
    format!(
        "@define-color theme_bg_color #{bg};\n\
         @define-color theme_fg_color #{fg};\n\
         @define-color theme_base_color #{surface};\n\
         @define-color theme_text_color #{on_surface};\n\
         @define-color theme_selected_bg_color #{primary};\n\
         @define-color theme_selected_fg_color #{on_primary};\n\
         @define-color theme_unfocused_bg_color #{bg};\n\
         @define-color theme_unfocused_fg_color #{fg};\n\
         @define-color kb_surface_variant #{surface_variant};\n\
         @define-color kb_on_surface_variant #{on_surface_variant};\n\
         @define-color kb_outline #{outline};\n\
         @define-color kb_inverse_surface #{inverse_surface};\n\
         @define-color kb_inverse_on_surface #{inverse_on_surface};\n\
         @define-color kb_inverse_primary #{primary_inverse};\n\
         @define-color kb_primary_container #{primary_container};\n\
         @define-color kb_on_primary_container #{on_primary_container};\n\
         @define-color kb_tertiary #{tertiary};\n\
         @define-color kb_on_tertiary #{on_tertiary};\n\
         @define-color kb_secondary #{secondary};\n\
         @define-color kb_on_secondary #{on_secondary};\n",
        bg = hex6(p.background),
        fg = hex6(p.on_background),
        surface = hex6(p.surface),
        on_surface = hex6(p.on_surface),
        primary = hex6(p.primary),
        on_primary = hex6(p.on_primary),
        surface_variant = hex6(p.surface_variant),
        on_surface_variant = hex6(p.on_surface_variant),
        outline = hex6(p.outline),
        inverse_surface = hex6(p.inverse_surface),
        inverse_on_surface = hex6(p.inverse_on_surface),
        primary_inverse = hex6(p.primary_inverse),
        primary_container = hex6(p.primary_container),
        on_primary_container = hex6(p.on_primary_container),
        tertiary = hex6(p.tertiary),
        on_tertiary = hex6(p.on_tertiary),
        secondary = hex6(p.secondary),
        on_secondary = hex6(p.on_secondary),
    )
}

/// 0xAARRGGBB -> "RRGGBB"; alpha is dropped (palette colours are opaque).
fn hex6(argb: u32) -> String {
    format!("{:06X}", argb & 0x00FF_FFFF)
}
```

Palette override is a per-theme choice. The default (`AppTheme::Default`) installs no palette CSS, so the user's Tokyo Night theme is untouched (memory: "match the current theme"). Light vs dark for the palette: choose `*_DARK` when the window is dark (`kb-dark`), `*_LIGHT` otherwise, and re-apply on the `gtk::Settings::gtk-application-prefer-dark-theme` and `notify::gtk-theme-name` signals.

### 5.2 Installing the providers

```rust
/// Installs the base stylesheet and the palette provider. AMOLED is not a provider: the caller adds
/// the `kb-amoled` class to the window when `dark` is true and the AMOLED preference is on.
pub fn install(display: &gtk::gdk::Display, theme: AppTheme, dark: bool) -> [gtk::CssProvider; 2] {
    let base = gtk::CssProvider::new();
    base.load_from_resource("/io/github/blackbox/Kurobayomi/style.css");
    gtk::style_context_add_provider_for_display(display, &base, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

    let palette = gtk::CssProvider::new();
    let pal_css = match (theme, dark) {
        (AppTheme::CottonCandy, true) => palette_css(&COTTONCANDY_DARK),
        (AppTheme::CottonCandy, false) => palette_css(&COTTONCANDY_LIGHT),
        (AppTheme::Mocha, true) => palette_css(&MOCHA_DARK),
        (AppTheme::Mocha, false) => palette_css(&MOCHA_LIGHT),
        (AppTheme::Default, _) => String::new(),
    };
    palette.load_from_string(&pal_css);
    gtk::style_context_add_provider_for_display(display, &palette, gtk::STYLE_PROVIDER_PRIORITY_USER);
    [base, palette]
}
```

- `load_from_string` / `load_from_resource` are the gtk4-rs 0.11 `CssProvider` methods (GTK 4.12+ string loader). Keep the two `CssProvider`s alive for the process lifetime (store them in the application struct); dropping a provider removes its rules.
- When the palette changes at runtime, call `palette.load_from_string(new_css)` on the existing provider instead of creating a new one.
- `STYLE_PROVIDER_PRIORITY_APPLICATION` (600) is above the theme (200) and settings (400), below user (800). Palette overrides therefore beat the theme only when a palette is chosen. Verify on Tokyo Night that the override does not leak into `Default`.

## 6. Source-of-truth decisions for colours

- Android has two colour sources for the same palette: the XML files (`app/src/main/res/values*/colors_*.xml`, used by Views) and the Kotlin `ColorScheme` objects (`app/src/main/java/eu/kanade/presentation/theme/colorscheme/*.kt`, used by Compose). They differ in two tokens (verified by a script for this plan):
  - CottonCandy dark `onTertiary`: Compose `#5F112B`; XML has no such token in the night file, so dark mode resolves to the light value `#FFFFFF`.
  - Mocha light `inverseOnSurface`: Compose `#FBEAEB`; XML `#FBEEEB`.
- Decision: Compose values win, because the visible UI is Compose (behavior 1.3). The two corrections are marked in section 2.2. The drift test in section 8 keeps the rest of the XML values equal to the Compose values.
- Other palettes (16 more) are not implemented in this area; their tokens are in `plans/49-theme-and-design-system/p1-topic-color-token-tables.md`.

## 7. Desktop actions (replaces `app/shortcuts.xml`)

`data/io.github.blackbox.Kurobayomi.desktop` (installed to `share/applications/`):

```ini
[Desktop Entry]
Type=Application
Name=Kurobayomi
Exec=kurobayomi %U
Icon=io.github.blackbox.Kurobayomi
Terminal=false
Categories=AudioVideo;Graphics;
Actions=library;animelib;updates;history;browse;

[Desktop Action library]
Name=Library
Icon=kb-library-symbolic
Exec=kurobayomi --action=show-library

[Desktop Action animelib]
Name=Anime library
Icon=kb-animelibrary-filled-symbolic
Exec=kurobayomi --action=show-animelib

[Desktop Action updates]
Name=Recent updates
Icon=kb-updates-outline-symbolic
Exec=kurobayomi --action=show-updates

[Desktop Action history]
Name=Recent manga
Icon=document-open-recent-symbolic
Exec=kurobayomi --action=show-history

[Desktop Action browse]
Name=Browse
Icon=system-search-symbolic
Exec=kurobayomi --action=show-browse
```

- Action names map to the five `MainActivity` routes (`SHOW_LIBRARY`, `SHOW_ANIMELIB`, `SHOW_RECENTLY_UPDATED`, `SHOW_RECENTLY_READ`, `SHOW_CATALOGUES`; behavior section 5). The duplicate-id bug in `shortcuts.xml` does not exist here: each action has a unique name.
- `--action=` is parsed in `gtk::Application::connect_command_line`, which then calls `app.activate-action` with a `show-*` action name; the window routes to the matching stack child (mockup: `ui::open(id)`).
- The `.desktop` `Icon=` entries for the custom `kb-*` names resolve only if the icons are installed in the hicolor theme or the GResource path is registered (section 2.3). Desktop environments that do not read GResource icons will show the fallback; to avoid that, install the SVGs under `share/icons/hicolor/scalable/actions/` as well (Proposal).

## 8. Tests (see also p1-edge-cases-and-tests.md)

- Test 1 (`tests/css_parses.rs`, runs under `xvfb-run -a cargo test`): load `style.css` into a `CssProvider`, connect `parsing-error` and assert no error. Catches undefined `@theme_*` names.
- Test 2 (`tests/icons_resolve.rs`, same display): for each `Icon` variant, `IconTheme::for_display(&display).has_icon(icon.name())` is true after `icons::register`. Catches typos and missing exports.
- Test 3 (`src/theme.rs` unit tests, no display): `AppTheme::from_pref` for every stored name in the Android enum (22 entries, `p1-data-and-config.md` section 2.1); unknown names and `MONET`, `DARK_BLUE`, `HOT_PINK`, `BLUE` map to `Default`.
- Test 4 (`src/theme.rs` unit tests): `palette_css(&MOCHA_LIGHT)` contains `@define-color theme_selected_bg_color #745B00;`; `hex6` drops the alpha byte.
- Test 5 (script, `tools/check_palette.py`, run in CI or by hand): parse the XML colour files and assert that every token equals the Rust constant except the two documented corrections.

## 9. Android to Linux mapping (API level)

| Android API or construct | Where used (scope) | Linux replacement |
|---|---|---|
| `Theme.Material3.DayNight.NoActionBar` and the `colorXxx` attributes | values/themes.xml | GTK theme variables (`@theme_*`) and the optional palette override (section 5.1) |
| `ThemeOverlay` with `Amoled` | values-night/themes.xml | window class `kb-amoled` (dark only) |
| `AnimatedImageVector.animatedVectorResource` + `rememberAnimatedVectorPainter` | 9 anim_*.xml | `anim.rs` helpers (section 10) |
| `painterResource` / `ImageVector.vectorResource` / `rememberResourceBitmapPainter` | drawables | `gtk::Image::from_icon_name(Icon::X.name())` or `gtk::Image::from_resource` for the full-colour logo |
| `layer-list` cover placeholder | cover_error.xml | `Frame` with `.kb-cover-placeholder` and `Image` (`Icon::Cover`), as the mockup's `ui::cover()` does |
| `animated-selector` (checked/unchecked) | ic_animelibrary_selector_24dp.xml | two `gtk::Image`s in a `gtk::Stack` (or `Overlay`), switched by a `ToggleButton` state |
| `SplashScreen` API (`installSplashScreen`, `setKeepOnScreenCondition`) | MainActivity, values/themes.xml | `splash.rs` page in the `gtk::Stack` plus the 500/5000 ms state machine |
| `NotificationCompat.Builder.setSmallIcon` / `setColor` | notifiers | `gio::Notification::set_icon(&gio::ThemedIcon::new(name))`; colour dropped (GNotification has no accent) |
| App shortcuts (`shortcuts.xml`) | app/shortcuts.xml | `.desktop` `[Desktop Action]` groups (section 7) |
| `ContextThemeWrapper` with night override | ContextExtensions.kt | not needed: reader background is a CSS class (behavior 1.5) |
| `dpToPx` (`value * 16.dpToPx`) | MainActivity exit animation | CSS px (1 dp = 1 px at scale 1; GTK scales for HiDPI) |
| `Modifier.size(16.dp, 20.dp)` seek triangle | DoubleTapSeekTriangles.kt | `gtk::Image` with `set_pixel_size` on the symbolic icon, sized 16x20 |
| `RippleConfiguration` | TachiyomiTheme.kt | `:hover` / `:focus` background alpha in CSS (`.kb-highlight`) |
| `windowLightStatusBar`, `navigationBarColor`, `forceDarkAllowed`, `enforce*Contrast`, `elevationOverlayEnabled` | values/themes.xml, bools | dropped (no system bars, no elevation tint, no forced dark on Linux) |
| `DynamicColors` / Monet | values-v31, values-night-v31 | dropped; default follows GTK theme |

## 10. `src/anim.rs` (public helpers)

```rust
use std::time::Duration;
use gtk::prelude::*;

/// Crossfades `stack` to `child_name` over `ms` (300 ms tab/play-pause swaps, behavior 3.2).
pub fn crossfade_to(stack: &gtk::Stack, child_name: &str, ms: u32) {
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(ms);
    stack.set_visible_child_name(child_name);
}

/// Icon rotation endpoints. Each variant is a CSS class defined in style.css with a
/// `transition: -gtk-icon-transform <ms>ms cubic-bezier(0.4, 0, 0.2, 1)` rule, so no per-frame code runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    /// anim_browse_enter: 0 -> 180 over 300 ms (class `kb-rot-180`).
    Half,
    /// anim_history_enter: 360 -> 0 over 500 ms (class `kb-rot-0-slow`).
    FullBackSlow,
}

pub fn rotate_icon(image: &gtk::Image, to: Rotation) {
    let class = match to {
        Rotation::Half => "kb-rot-180",
        Rotation::FullBackSlow => "kb-rot-0-slow",
    };
    image.set_css_classes(&["kb-icon-rot", class]);
}

/// Cubic-bezier easing with the same curve as CSS/Compose FastOutSlowIn (x1=0.4, y1=0.0, x2=0.2, y2=1.0).
/// Used for widget-side tweens that cannot use CSS transitions (double-tap triangles).
pub fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    let bx = |t: f64| 3.0 * (1.0 - t) * (1.0 - t) * t * x1 + 3.0 * (1.0 - t) * t * t * x2 + t * t * t;
    let by = |t: f64| 3.0 * (1.0 - t) * (1.0 - t) * t * y1 + 3.0 * (1.0 - t) * t * t * y2 + t * t * t;
    let dx = |t: f64| 3.0 * (1.0 - t) * (1.0 - t) * x1 + 6.0 * (1.0 - t) * t * (x2 - x1) + 3.0 * t * t * (1.0 - x2);
    let mut t = x.clamp(0.0, 1.0);
    for _ in 0..8 {
        let err = bx(t) - x;
        let d = dx(t);
        if d.abs() < 1e-9 {
            break;
        }
        t = (t - err / d).clamp(0.0, 1.0);
    }
    by(t)
}

/// Double-tap seek triangles (behavior 4.5): step length 150 ms, six steps per cycle (900 ms).
/// Row `s` is the target alpha of the three triangles at the END of step `s`.
/// Step 0 raises triangle 0, step 1 raises 1, step 2 raises 2, step 3 lowers 0, step 4 lowers 1, step 5 lowers 2.
pub const SEEK_STEP_MS: u64 = 150;
pub const SEEK_ROWS: [[f64; 3]; 6] = [
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [1.0, 1.0, 1.0],
    [0.0, 1.0, 1.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, 0.0],
];

/// Opacity of triangle `idx` (0..3) at time `t` since the loop started.
/// Each step is a 150 ms tween with the Compose default easing (FastOutSlowIn), starting from the previous row.
pub fn seek_alpha(idx: usize, t: Duration) -> f64 {
    let ms = t.as_millis() as u64;
    let step = ((ms / SEEK_STEP_MS) % 6) as usize;
    let frac = (ms % SEEK_STEP_MS) as f64 / SEEK_STEP_MS as f64;
    let prev = SEEK_ROWS[(step + 5) % 6][idx];
    let next = SEEK_ROWS[step][idx];
    prev + (next - prev) * cubic_bezier(0.4, 0.0, 0.2, 1.0, frac)
}
```

Checks for `seek_alpha`: at `t = 0` every triangle is `0.0` (previous row is row 5, all zero); at `t = 150 ms` triangle 0 reaches `1.0`; at `t = 900 ms` the cycle restarts from the all-zero row. The table is the behavior-spec schedule in `DoubleTapSeekTriangles.kt:28-36`.

## 11. Pitfalls

- Recolouring: a symbolic SVG without `#bebebe` fills will not follow the theme colour. Check the exported files with `grep -L 'bebebe' data/icons/hicolor/*/actions/kb-*-symbolic.svg` (must print nothing).
- `view-more-horizontal-symbolic` is horizontal. The Android overflow is vertical (`ic_overflow_24dp`); use `kb-overflow-vertical-symbolic` for download rows.
- The mockup uses `edit-cut-symbolic` for the crop button (`src/player.rs:100`). Android's crop button is `ic_crop_24dp` / `ic_crop_off_24dp` (`BottomReaderBar.kt:59`); switch to `Icon::CropOn` / `Icon::CropOff`.
- The mockup uses `bookmark-new-symbolic` and `view-pin-symbolic` for bookmarks/pins (`src/player.rs:69`, `src/entry.rs:292`). Those are not from this area's resources; leave them unless a later area requires otherwise.
- GTK 4 CSS has no media queries and no `:root[data-theme]` selectors. Use classes on the window.
- `-gtk-icon-transform` must be verified on GTK 4.16 for `rotate()` on symbolic icons. If it does not apply to `gtk::Image`, animate with a `gtk::DrawingArea` and cairo `rotate`. Do not use the Android `valueFrom`/`valueTo` values as the GTK animation endpoints unless they are 0/180/360.
- `gio::ThemedIcon` names for notifications must be installed icons; a missing name shows nothing on some desktops. Use the standard names from `Icon::name()` (`accessories-dictionary-symbolic`, `object-select-symbolic`, `view-refresh-symbolic`, `folder-symbolic`, `application-x-addon-symbolic`, `network-server-symbolic`, `software-update-available-symbolic`, `image-x-generic-symbolic`, `dialog-information-symbolic`, `dialog-warning-symbolic`, `window-close-symbolic`, `document-send-symbolic`, `media-playback-start-symbolic`, `media-playback-pause-symbolic`) for notifications, and only the app id for the logo.
- Keep the `CssProvider` instances alive (store them in the application struct); a dropped provider silently removes its rules.
- `ic_ani.xml` has two paths both filled `#FFFFFF`; the symbolic export must not keep white (`#FFFFFF`) because GTK will not recolour it.
- Icon search path: GTK requires the resource directory to follow the icon theme layout (`scalable/actions/`, `scalable/apps/`, `symbolic/apps/`); a flat directory will not resolve.

## 12. Versions and crates

- `gtk4` 0.11.5 (`Cargo.toml`), feature `v4_16`. `gio`, `glib` and `gdk` come with it. Use the `glib-build-tools` release that matches the `glib` version in `Cargo.lock` (run `cargo tree -i glib` to find it) for `build.rs`.
- No new runtime crates. `xvfb-run` (system package) for the display-dependent tests.
- No SVG crate at runtime (icons are shipped as SVG files that GTK loads through its own pixbuf loaders).
- For the offline export script: Python 3 standard library (`xml.etree.ElementTree`) is enough; `rsvg-convert` (librsvg) for the visual check.
