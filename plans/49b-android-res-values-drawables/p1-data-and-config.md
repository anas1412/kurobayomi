# Area 49b (part 1): data models, preference keys, formats and endpoints

All paths are repo-relative to `/home/blackbox/aniyomi-src`. This area has no database tables, no network endpoints and no files written at runtime. The data it defines is: theme style tables, colour values, dimensions, timings, icon path data, and the preference/enum values that select a theme.

---

## 1. Value formats used by the resource files

### 1.1 Colours (`<color>` in `values*/colors*.xml`, `color/*.xml`)

- Formats seen: `#RRGGBB` (for example `#9A4058`, `#FFFFFF`, `#0C0C0C`), `#AARRGGBB` (for example `#1F888888` for `cover_placeholder`, `#1F000000` for `md_black_1000_12`, `#33FFFFFF` for launcher grid lines), and `@android:color/transparent`, `@color/<other>` references.
- Alpha byte is the first two hex digits. `#1F888888` = alpha 0x1F = 31/255 = 0.1216, colour `#888888`.
- Case: values appear in both upper and lower case (`#ebc248` and `#EBC248`); the value is case-insensitive.
- Colour resource names in scope: 22 tokens per palette file x 4 files (`colors_cottoncandy`, `colors_mocha`, each in light and dark), plus `cottoncandy_elevationOverlay` (x2, unused in code).

Token suffix list used by the palettes (exact names, `<p>` = palette prefix):

`<p>_primary`, `<p>_onPrimary`, `<p>_primaryContainer`, `<p>_onPrimaryContainer`, `<p>_secondary`, `<p>_onSecondary`, `<p>_secondaryContainer`, `<p>_onSecondaryContainer`, `<p>_tertiary`, `<p>_onTertiary`, `<p>_tertiaryContainer`, `<p>_onTertiaryContainer`, `<p>_background`, `<p>_onBackground`, `<p>_surface`, `<p>_onSurface`, `<p>_surfaceVariant`, `<p>_onSurfaceVariant`, `<p>_outline`, `<p>_inverseOnSurface`, `<p>_inverseSurface`, `<p>_primaryInverse`. (Mocha and CottonCandy in `app/` use this 22-name set; the base `tachiyomi_*` set in `presentation-core` is larger, see section 6.)

Cross-check of the values against the Compose palettes (`app/src/main/java/eu/kanade/presentation/theme/colorscheme/MochaColorScheme.kt`, `CottoncandyColorScheme.kt`), performed for this plan on the 22 tokens:
- Mocha dark: all 22 match.
- Mocha light: `inverseOnSurface` XML `#FBEEEB` vs Kotlin `#FBEAEB` (differs in one channel).
- CottonCandy light: all 22 match.
- CottonCandy dark: `onTertiary` is `#5F112B` in Kotlin and missing in XML (`values-night/colors_cottoncandy.xml` has no `cottoncandy_onTertiary`), so dark mode resolves the light-file value `#FFFFFF`.

### 1.2 Dimensions (`<dimen>`)

- Units: `dp` only (`16dp`, `24dp`, `12dp`, `16dp`).
- Conversion: px = dp x density / 160. On GTK the equivalent is `logical px` (dp = CSS px). Kurobayomi uses CSS px directly (16 CSS px = 16dp).

### 1.3 Integers (`<integer>`)

- `player_animation_duration` = `200` (milliseconds; used as `android:duration` which is in ms).

### 1.4 Booleans (`<bool>`)

Qualifier-specific values (see behavior section 1.6):

| Name | default (`values/`) | `values-night/` | `values-v27/` |
|---|---|---|---|
| lightStatusBar | true | false | true |
| lightNavigationBar | false | false | true |
| elevationOverlayEnabled | false | true | (not defined) |

### 1.5 Styles and their item types

Each `<item name="...">` value is one of: a colour reference (`@color/...`), a theme attribute reference (`?attr/...`, `?android:attr/...`), a drawable reference (`@drawable/...`), a style reference (`@style/...`), a layout reference (`@layout/...`), a dimen reference (`@dimen/...`), a bool reference (`@bool/...`), a boolean literal (`false`/`true`), a float literal (`0.32`), a keyword (`gravity`, `locale`), or `@null` / `@android:color/transparent`. Full item-name list per style is in the inventory and in behavior section 1.7.

Items with non-colour types in `values/themes.xml`:
- `android:forceDarkAllowed` = `false` (boolean)
- `android:textAlignment` = `gravity` (keyword)
- `android:textDirection` = `locale` (keyword)
- `android:backgroundDimAmount` = `0.32` (float, fraction of black at a modal's scrim)
- `android:windowLightStatusBar` = `@bool/lightStatusBar`
- `windowActionModeOverlay` = `true`
- `android:navigationBarDividerColor` = `@null`
- `android:enforceNavigationBarContrast` = `false`, `android:enforceStatusBarContrast` = `false`
- `windowSplashScreenAnimatedIcon` = `@drawable/ic_tachi_splash`
- `windowSplashScreenBackground` = `@color/splash`
- `postSplashScreenTheme` = `@style/Theme.Tachiyomi`
- `android:windowLightNavigationBar` = `false` (in `Theme.Tachiyomi.SplashScreen`, `tools:targetApi="o_mr1"`)

Attributes in `values/styles.xml` with non-trivial values: `dialogCornerRadius` = `@dimen/m3_alert_dialog_corner_size` (Material library dimen), `android:background` = `@drawable/material_popup_background`, `android:layout` = `@layout/m3_alert_dialog`, `android:widgetLayout` = `@layout/pref_widget_switch_material`, `android:tint` = `?attr/colorOnSurface`, `android:textColor` = `?attr/colorOnBackground` (menu text), `android:textColor` = `?android:attr/textColorSecondary` (section header), `dividerColor` = `@color/divider_default`, `materialThemeOverlay` = `@style/ThemeOverlay.Tachiyomi.Switch`, `elevationOverlayEnabled` = `@bool/elevationOverlayEnabled`, `preferenceCategoryTitleTextAppearance` = `?attr/textAppearanceBodyMedium`, `switchPreferenceCompatStyle` = `@style/PreferenceThemeOverlay.Tachiyomi.SwitchPreferenceCompat`.

### 1.6 Declare-styleable attributes (`values/attrs.xml`)

| Attribute | Format | Used by |
|---|---|---|
| `MinMaxNumberPicker_min` | integer | `app/src/main/java/eu/kanade/tachiyomi/widget/MinMaxNumberPicker.kt:30` |
| `MinMaxNumberPicker_max` | integer | `MinMaxNumberPicker.kt:31` |
| `MaterialSpinnerView_title` | reference or string | none (no class) |
| `MaterialSpinnerView_android:entries` | framework entries | none |
| `MaterialSpinnerView_summary` | reference or string | none |

### 1.7 Drawable path data (VectorDrawable grammar)

- `android:pathData` strings use SVG path syntax: commands `M m L l H h V v C c S s Q q T t A a Z z`, with optional commas. Some files use Android's compact form (`M270.46 450.51 C220.99,450.51 ...`), some use the Material Symbols form (`M80,880L80,240...`).
- `android:viewportWidth` and `android:viewportHeight` define the coordinate space (24, 960, 1080, 108, 56, 512 across the scope). Intrinsic size comes from `android:width`/`android:height` in `dp`.
- Fill: `android:fillColor` (hex or `?attr` or `@color`), `android:fillAlpha`, `android:fillType` (`nonZero` in the animation file), `android:strokeColor`, `android:strokeWidth` (`0.0853242` on two launcher circles), `android:strokeAlpha` (`0` on the browse ring).
- Groups: `android:rotation`, `android:pivotX`, `android:pivotY`, `android:translateX`, `android:translateY`, `android:scaleX`, `android:scaleY`, `android:name`.
- Clip paths: `<clip-path android:pathData=...>` (only `anim_browse_enter.xml`, `dot_mask`).
- Animated vectors add `android:propertyName` (`pathData`, `rotation`, `scaleX`, `scaleY`, `translateX`, `translateY`), `android:valueType` (`pathType` or `floatType`), `android:valueFrom`, `android:valueTo`, `android:duration` (ms, integer), `android:startOffset` (ms), `android:interpolator` (`@android:interpolator/fast_out_slow_in`, `@android:anim/decelerate_interpolator`, or a `<pathInterpolator android:pathData=...>`), `android:ordering` (`together` or `sequentially`).

Path data is the input to the SVG export (section 5 in the icon topic). The Rust side needs no VectorDrawable parser: the export is a one-time offline conversion.

### 1.8 Adaptive icon wrappers

- `<adaptive-icon>` with `<background>` and `<foreground>` (5 shortcut icons; `<monochrome>` in `mipmap/ic_launcher.xml`, outside scope).
- The `tools:keep` attribute is a build-time hint only.

---

## 2. Enums that select the theme

### 2.1 `AppTheme` (`app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt`, enum starts at line 7)

Stored as the enum `name()` string (see 3.1). Values in declaration order, with the MR/AYMR message key used for the picker label:

| # | Constant | Label key | Key library | Style |
|---|---|---|---|---|
| 0 | DEFAULT | `label_default` | MR | Theme.Tachiyomi |
| 1 | MONET | `theme_monet` | MR | Theme.Tachiyomi.Monet |
| 2 | CLOUDFLARE | `theme_cloudflare` | AYMR | Theme.Tachiyomi.Cloudflare |
| 3 | COTTONCANDY | `theme_cottoncandy` | AYMR | Theme.Tachiyomi.CottonCandy |
| 4 | DOOM | `theme_doom` | AYMR | Theme.Tachiyomi.Doom |
| 5 | GREEN_APPLE | `theme_greenapple` | MR | Theme.Tachiyomi.GreenApple |
| 6 | LAVENDER | `theme_lavender` | MR | Theme.Tachiyomi.Lavender |
| 7 | MATRIX | `theme_matrix` | AYMR | Theme.Tachiyomi.Matrix |
| 8 | MIDNIGHT_DUSK | `theme_midnightdusk` | MR | Theme.Tachiyomi.MidnightDusk |
| 9 | MOCHA | `theme_mocha` | AYMR | Theme.Tachiyomi.Mocha |
| 10 | SAPPHIRE | `theme_sapphire` | AYMR | Theme.Tachiyomi.Sapphire |
| 11 | NORD | `theme_nord` | MR | Theme.Tachiyomi.Nord |
| 12 | STRAWBERRY_DAIQUIRI | `theme_strawberrydaiquiri` | MR | Theme.Tachiyomi.StrawberryDaiquiri |
| 13 | TAKO | `theme_tako` | MR | Theme.Tachiyomi.Tako |
| 14 | TEALTURQUOISE | `theme_tealturquoise` | MR | Theme.Tachiyomi.TealTurquoise |
| 15 | TIDAL_WAVE | `theme_tidalwave` | MR | Theme.Tachiyomi.TidalWave |
| 16 | YINYANG | `theme_yinyang` | MR | Theme.Tachiyomi.YinYang |
| 17 | YOTSUBA | `theme_yotsuba` | MR | Theme.Tachiyomi.Yotsuba |
| 18 | MONOCHROME | `theme_monochrome` | MR | Theme.Tachiyomi.Monochrome |
| 19 | DARK_BLUE | (null: hidden) | none | Theme.Tachiyomi (fallback) |
| 20 | HOT_PINK | (null: hidden) | none | Theme.Tachiyomi (fallback) |
| 21 | BLUE | (null: hidden) | none | Theme.Tachiyomi (fallback) |

MR = `tachiyomi.i18n.MR`, AYMR = `tachiyomi.i18n.aniyomi.AYMR` (imports at `AppTheme.kt:4-5`).

The ordinal is not persisted; only `name` is (see 3.1). Ordinals are listed for completeness.

### 2.2 `ThemeMode` (`app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt:5-9`)

`LIGHT`, `DARK`, `SYSTEM`. Mapped to `AppCompatDelegate` modes in `setAppCompatDelegateThemeMode` (`ThemeMode.kt:11-18`): `LIGHT` to `MODE_NIGHT_NO`, `DARK` to `MODE_NIGHT_YES`, `SYSTEM` to `MODE_NIGHT_FOLLOW_SYSTEM`.

---

## 3. Preferences that this area reads

### 3.1 Storage semantics

- Enum preferences use `PreferenceStore.getEnum(key, default)` (`core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt:27-40`): serializer `it.name`; deserializer `enumValueOf(it)`; on `IllegalArgumentException` the default is returned. A stored deprecated name such as `BLUE` still deserializes (the constant still exists), then `ThemingDelegate` maps it to `Theme_Tachiyomi` via `getOrDefault`.
- Boolean preferences: `getBoolean` (Android SharedPreferences key/value).
- The backing store is `AndroidPreferenceStore` (`core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt`), i.e. a SharedPreferences file (file name not in scope).

### 3.2 Keys and defaults

| Key | Type | Default | Reader | Defined at |
|---|---|---|---|---|
| `pref_app_theme` | enum AppTheme | `MONET` if `DeviceUtil.isDynamicColorAvailable`, else `DEFAULT` | `ThemingDelegate.kt:18-21`, `TachiyomiTheme.kt:37-47`, `AppThemePreferenceWidget.kt` | `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:22-29` |
| `pref_theme_dark_amoled_key` | boolean | `false` | `ThemingDelegate.kt:28`, `ContextExtensions.kt:141`, `TachiyomiTheme.kt:46`, `SettingsAppearanceScreen.kt:63`, `ThemeStep.kt:28` | `UiPreferences.kt:31` |
| `pref_theme_mode_key` | enum ThemeMode | `SYSTEM` | `ContextExtensions.kt:122`, `ThemeMode.kt` | `UiPreferences.kt:20` (`themeMode()`) |
| `pref_reader_theme_key` | int | `1` | `ContextExtensions.kt` (`createReaderThemeContext`), `PagerConfig.kt:28`, `ReaderActivity.kt:840` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:57` |

No other preference key is read by the scope's resources. The DYNAMIC-colour check is not a preference.

### 3.3 Reader theme values (for the reader context mapping)

`readerTheme` integer as used by `createReaderThemeContext` (`app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:123-129`): `1` and `2` are dark backgrounds (Black, Gray), `3` is "automatic" (follows `themeMode` or the system), and any other value is treated as white. Default is `1` (`ReaderPreferences.kt:57`).

---

## 4. Files, folders and formats on disk

This area reads only files that ship inside the APK. It writes nothing.

| Path (repo-relative) | Format | Read by |
|---|---|---|
| app/src/main/res/values/*.xml, values-night/*.xml, values-v27/*.xml, values-v31/*.xml, values-night-v31/*.xml, values-sw720dp/dimens.xml | Android resource XML | the Android resource compiler (AAPT2) |
| app/src/main/res/drawable/*.xml | VectorDrawable / AnimatedVectorDrawable / layer-list / shape / adaptive-icon XML | AAPT2 and `ImageVector.vectorResource`, `AnimatedImageVector.animatedVectorResource`, `painterResource`, `rememberResourceBitmapPainter` |
| app/src/main/res/color/draggable_card_foreground.xml | colour state list | Views (`app:cardForegroundColor`) |
| app/shortcuts.xml | Android app-shortcut XML | Android launcher |
| app/src/main/AndroidManifest.xml | manifest (theme refs at lines 50, 57, 214, 225) | Android |

Linux equivalents this plan produces (not present yet):
- `data/icons/hicolor/scalable/apps/io.github.anas1412.Kurobayomi.svg` (full-colour app icon)
- `data/icons/hicolor/symbolic/apps/io.github.anas1412.Kurobayomi-symbolic.svg` (monochrome app icon)
- `data/icons/hicolor/scalable/actions/kb-*-symbolic.svg` (custom symbolic icons; list in the icon topic)
- `data/style.css` (GResource-bundled stylesheet; palette and token rules)
- `data/io.github.anas1412.Kurobayomi.desktop` (desktop actions)

The app id `io.github.anas1412.Kurobayomi` is the one already used by the mockup (`src/main.rs:13`).

---

## 5. Network endpoints

None in this area. The launcher and notification icons are not fetched from the network.

---

## 6. Base palette (outside scope, required by the scope's `themes.xml`)

Keys from `presentation-core/src/main/res/values/colors_tachiyomi.xml` (the `tachiyomi_*` set used as the Base palette, the default look). Values are the defaults that a palette falls back to when it omits a token (behavior section 1.8). Tokens: `tachiyomi_primary` `#0058CA`, `tachiyomi_onPrimary` `#FFFFFF`, `tachiyomi_primaryContainer` `#D9E2FF`, `tachiyomi_onPrimaryContainer` `#001945`, `tachiyomi_inversePrimary` `#B0C6FF`, `tachiyomi_secondary` `#0058CA`, `tachiyomi_onSecondary` `#FFFFFF`, `tachiyomi_secondaryContainer` `#D9E2FF`, `tachiyomi_onSecondaryContainer` `#001945`, `tachiyomi_tertiary` `#006E1B`, `tachiyomi_onTertiary` `#FFFFFF`, `tachiyomi_tertiaryContainer` `#95F990`, `tachiyomi_onTertiaryContainer` `#002203`, `tachiyomi_background` `#FEFBFF`, `tachiyomi_onBackground` `#1B1B1F`, `tachiyomi_surface` `#FEFBFF`, `tachiyomi_onSurface` `#1B1B1F`, `tachiyomi_surfaceVariant` `#F3EDF7`, `tachiyomi_onSurfaceVariant` `#44464F`, `tachiyomi_surfaceTint` `#0058CA`, `tachiyomi_inverseSurface` `#303034`, `tachiyomi_inverseOnSurface` `#F2F0F4`, `tachiyomi_error` `#BA1A1A`, `tachiyomi_onError` `#FFFFFF`, `tachiyomi_errorContainer` `#FFDAD6`, `tachiyomi_onErrorContainer` `#410002`, `tachiyomi_outline` `#757780`, `tachiyomi_outlineVariant` `#C5C6D0`, `tachiyomi_surfaceContainerLowest` `#F5F1F8`, `tachiyomi_surfaceContainerLow` `#F7F2FA`, `tachiyomi_surfaceContainer` `#F3EDF7`, `tachiyomi_surfaceContainerHigh` `#FCF7FF`, `tachiyomi_surfaceContainerHighest` `#FCF7FF`.

Other shared colours (from `presentation-core/src/main/res/values/colors.xml`, `values-night/colors.xml`):
- `splash` = `@color/accent_blue` (light), `#202125` (night)
- `accent_blue` = `#54759E`
- `cover_placeholder` = `#1F888888`
- `divider_default` = `@color/md_black_1000_12` (light) = `#1F000000`; `@color/md_white_1000_12` (night) = `#1FFFFFFF`
- `amoled_background` = `#000000`; `amoled_onBackground` = `#FFFFFF`; `amoled_surface` = `#000000`; `amoled_onSurface` = `#FFFFFF`; `amoled_surfaceContainer` = `#0C0C0C`; `amoled_surfaceContainerHigh` = `#131313`; `amoled_surfaceContainerHighest` = `#1B1B1B`

## 7. Dimensions and timing constants (Kotlin side, consumed with the resources)

| Constant | Value | Location |
|---|---|---|
| `SPLASH_MIN_DURATION` | 500 ms | `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:665` |
| `SPLASH_MAX_DURATION` | 5000 ms | `MainActivity.kt:666` |
| `SPLASH_EXIT_ANIM_DURATION` | 400 ms | `MainActivity.kt:667` |
| splash exit slide | 16 dp | `MainActivity.kt:439` |
| `animationDuration` (double-tap) | 750 ms | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/DoubleTapSeekTriangles.kt:22` |
| RIPPLE_*_ALPHA | 0.1 each | `app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt` (other area) |

## 8. Launcher-shortcut data (`app/shortcuts.xml`)

Fields per `<shortcut>`: `android:enabled` (`true`), `android:icon` (drawable), `android:shortcutDisabledMessage` (`@string/app_not_available`), `android:shortcutId`, `android:shortcutLongLabel`, `android:shortcutShortLabel`, and one `<intent android:action=... android:targetClass="eu.kanade.tachiyomi.ui.main.MainActivity"/>`. Five entries; shortcutIds `show_library`, `show_animelib`, `show_recently_updated`, `show_recently_updated` (duplicate), `show_catalogues`. See behavior section 5.

## 9. Icon path inventory (data shape)

Each vector drawable reduces to: a viewport size, a list of paths (`pathData`, fill colour, optional stroke), and optional transform groups. The Kurobayomi export keeps `pathData` verbatim and writes an SVG `viewBox` equal to the viewport, with `fill="currentColor"` for symbolic icons and the original colours for the launcher icon. Dimensions for symbolic icons: 16 px and 24 px nominal (GTK symbolic icons are 16 px; the mockup uses 96 px images for placeholders only).

## 10. Data not present in this area (explicit)

- No SQLite or other database tables.
- No HTTP requests, no JSON, no protobuf.
- No bit flags or packed layouts (colours are plain ARGB, not packed fields).
- No locale-dependent strings (scope defines none; labels come from MR/AYMR and the Android `strings.xml` files outside scope).
