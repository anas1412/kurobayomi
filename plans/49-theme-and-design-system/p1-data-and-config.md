# Area 49 (part 1): data models, preference keys, files, endpoints

Repo-relative paths are under `/home/blackbox/aniyomi-src`.

## 1. Enums

### 1.1 `AppTheme` (`app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt`)

`enum class AppTheme(val titleRes: StringResource?)`. Stored value = enum constant name (String), via `getEnum` (`core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt:27-42`: serializer `it.name`, deserializer `enumValueOf(it)`, falls back to the default on `IllegalArgumentException`).

| Ordinal | Constant | titleRes | Kurobayomi stored value (same string) |
|---|---|---|---|
| 0 | DEFAULT | MR.strings.label_default | `DEFAULT` |
| 1 | MONET | MR.strings.theme_monet | `MONET` |
| 2 | CLOUDFLARE | AYMR.strings.theme_cloudflare | `CLOUDFLARE` |
| 3 | COTTONCANDY | AYMR.strings.theme_cottoncandy | `COTTONCANDY` |
| 4 | DOOM | AYMR.strings.theme_doom | `DOOM` |
| 5 | GREEN_APPLE | MR.strings.theme_greenapple | `GREEN_APPLE` |
| 6 | LAVENDER | MR.strings.theme_lavender | `LAVENDER` |
| 7 | MATRIX | AYMR.strings.theme_matrix | `MATRIX` |
| 8 | MIDNIGHT_DUSK | MR.strings.theme_midnightdusk | `MIDNIGHT_DUSK` |
| 9 | MOCHA | AYMR.strings.theme_mocha | `MOCHA` |
| 10 | SAPPHIRE | AYMR.strings.theme_sapphire | `SAPPHIRE` |
| 11 | NORD | MR.strings.theme_nord | `NORD` |
| 12 | STRAWBERRY_DAIQUIRI | MR.strings.theme_strawberrydaiquiri | `STRAWBERRY_DAIQUIRI` |
| 13 | TAKO | MR.strings.theme_tako | `TAKO` |
| 14 | TEALTURQUOISE | MR.strings.theme_tealturquoise | `TEALTURQUOISE` |
| 15 | TIDAL_WAVE | MR.strings.theme_tidalwave | `TIDAL_WAVE` |
| 16 | YINYANG | MR.strings.theme_yinyang | `YINYANG` |
| 17 | YOTSUBA | MR.strings.theme_yotsuba | `YOTSUBA` |
| 18 | MONOCHROME | MR.strings.theme_monochrome | `MONOCHROME` |
| 19 | DARK_BLUE | null (deprecated, hidden) | `DARK_BLUE` (renders as DEFAULT) |
| 20 | HOT_PINK | null (deprecated, hidden) | `HOT_PINK` (renders as DEFAULT) |
| 21 | BLUE | null (deprecated, hidden) | `BLUE` (renders as DEFAULT) |

Kurobayomi migration rule: keep the exact strings, so a backup or a copied preference store still resolves. Deprecated names map to DEFAULT at read time (the Android code does this implicitly through `getOrDefault`).

### 1.2 `ThemeMode` (`app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt`)

`enum class ThemeMode { LIGHT, DARK, SYSTEM }`. Default `SYSTEM`. Stored as `"LIGHT"`, `"DARK"`, `"SYSTEM"`. Mapping to AppCompat night mode: LIGHT -> `MODE_NIGHT_NO`, DARK -> `MODE_NIGHT_YES`, SYSTEM -> `MODE_NIGHT_FOLLOW_SYSTEM`.

### 1.3 `TriState` (core/common, used by `TriStateItem`)

Values `DISABLED`, `ENABLED_IS`, `ENABLED_NOT`. Not declared in this area; referenced from `SettingsItems.kt:66`. Owner: area 02.

## 2. Preference keys used in this area

Read through `UiPreferences` (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt`).

| Key | Accessor | Type | Default | Read by |
|---|---|---|---|---|
| `pref_app_theme` | `appTheme()` | enum `AppTheme` | `AppTheme.MONET` if `DeviceUtil.isDynamicColorAvailable`, else `AppTheme.DEFAULT` | `TachiyomiTheme.kt:45`, `ThemingDelegate.kt:27`, `ContextExtensions.kt:140`, `AppThemePreferenceWidget.kt`, `SettingsAppearanceScreen.kt:60`, `ThemeStep.kt:25` |
| `pref_theme_dark_amoled_key` | `themeDarkAmoled()` | Boolean | `false` | `TachiyomiTheme.kt:46`, `ThemingDelegate.kt:28`, `ContextExtensions.kt:141`, `SettingsAppearanceScreen.kt:63`, `ThemeStep.kt:28` |
| `pref_theme_mode_key` | `themeMode()` | enum `ThemeMode` | `ThemeMode.SYSTEM` | `ThemeStep.kt:22`, `SettingsAppearanceScreen.kt`, `setAppCompatDelegateThemeMode` |

Notes:
- `DeviceUtil.isDynamicColorAvailable` (`app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt:6-8`) = `DynamicColors.isDynamicColorAvailable() || (DeviceUtil.isSamsung && SDK_INT >= 31)`, lazily evaluated. On Linux it is false (no Material You); the default becomes `DEFAULT`.
- The keys are the Android SharedPreferences names. Keep the same names in Kurobayomi's `prefs` table (area 02) so that importing a backup does not lose the theme.
- Other keys in this area: none. The player ripple and window size classes are not preferences.

## 3. Color scheme data model

Each scheme is `darkScheme` and `lightScheme`, both `androidx.compose.material3.ColorScheme` built with `darkColorScheme(...)` or `lightColorScheme(...)`.

Field set of the Compose `ColorScheme` used by the app (the 36 base roles set by scheme objects; fixed roles are not set by this app and always take library defaults):

| Role | Type | Notes |
|---|---|---|
| primary, onPrimary, primaryContainer, onPrimaryContainer, inversePrimary | Color | |
| secondary, onSecondary, secondaryContainer, onSecondaryContainer | Color | "Unread badge" and selector pill roles in source comments |
| tertiary, onTertiary, tertiaryContainer, onTertiaryContainer | Color | "Downloaded badge" roles in source comments |
| background, onBackground, surface, onSurface | Color | |
| surfaceVariant, onSurfaceVariant | Color | "Navigation bar background (ThemePrefWidget)" comment in several schemes |
| surfaceTint | Color | defaults to `primary` when omitted |
| inverseSurface, inverseOnSurface | Color | |
| error, onError, errorContainer, onErrorContainer | Color | |
| outline, outlineVariant | Color | |
| scrim | Color | `0xFF000000` in most schemes |
| surfaceBright, surfaceDim | Color | |
| surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest | Color | |
| primaryFixed, primaryFixedDim, onPrimaryFixed, onPrimaryFixedVariant, secondaryFixed, secondaryFixedDim, onSecondaryFixed, onSecondaryFixedVariant, tertiaryFixed, tertiaryFixedDim, onTertiaryFixed, onTertiaryFixedVariant | Color | never set; library default |

Exact per-scheme hex values, explicit/omitted lists: `p1-topic-color-token-tables.md`.

Kurobayomi representation (Proposal): `struct SchemeTokens { primary: Rgba, ... }` with 36 fields plus `fn resolve(is_dark, amoled) -> SchemeTokens`. See `p1-rust-design.md` section 4.

## 4. Constants with data meaning

| Name | Type | Value | Location |
|---|---|---|---|
| AMOLED surface container | Color | `0xFF0C0C0C` | `BaseColorScheme.kt:13` |
| AMOLED surface container high | Color | `0xFF131313` | `:14` |
| AMOLED surface container highest | Color | `0xFF1B1B1B` | `:15` |
| Black / White overrides | Color | `0xFF000000` / `0xFFFFFFFF` | `:23-24` |
| Medium window breakpoint | Dp | 600 | `WindowSize.kt:22` |
| Expanded window breakpoint | Dp | 840 | `WindowSize.kt:23` |
| Sheet max width (landscape / portrait) | Dp | 600 / 460 | `AdaptiveSheet.kt:64-68` |
| Ripple alphas | Float | 0.1 (dragged, focused, hovered, pressed) | `TachiyomiTheme.kt:88-91` |
| Disabled alpha | Float | 0.38 | `Constants.kt:9` |
| Secondary alpha | Float | 0.78 | `Constants.kt:10` |
| Spacing scale | Dp | 32, 24, 16, 12, 8, 4 (extraLarge, large, medium, mediumSmall, small, extraSmall) | `Constants.kt:14-24` |
| Settings horizontal / vertical padding | Dp | 24 / 10 | `SettingsItems.kt:76-77` |
| Icon-button state layer | Dp | 40 | `IconButtonTokens.kt:11` |
| Wheel rows | Int | 3 | `WheelPicker.kt:278` |
| Wheel default size | Dp | 128 x 128 | `WheelPicker.kt:62,86,109` |
| Thumb length / thickness | Dp | 48 / 12 | `VerticalFastScroller.kt:440-441` |
| Repeat delays | ms | 750 max, 5 min, decay 0.25 | `SettingsItems.kt:468-470` |
| Dialog min / max width | Dp | 280 / 560 | `AlertDialog.kt:139-140` |

## 5. Files and folders on disk

This area reads and writes no files. Notes:
- `presentation-core` declares no assets, fonts, or raw resources of its own in this part. Its `res/values*/color_*.xml` files belong to part 2 of this area.
- `MonetColorScheme.extractSeedColorFromImage(bitmap)` takes an Android `Bitmap` and has no callers in `app/`. No file is read.
- Kurobayomi: the generated CSS is held in memory and given to `gtk::CssProvider::load_from_string`. No `theme.css` file is written. If a debug dump is wanted, write to `$XDG_CACHE_HOME/kurobayomi/theme.css` (not part of parity).

## 6. Network endpoints

None in this area.

## 7. Database tables

None in this area. Theme prefs live in the preferences store (area 02). Paging (`PagingDataUtil.kt`) has no table; it is a UI helper over repository flows.

Kurobayomi: list-model pagination (Drop of Paging 3) is specified in `p1-rust-design.md` section 8. It reads SQLite with `LIMIT`/`OFFSET` or keyset pages defined by the owning area (e.g., area 37 library), not by this area.

## 8. Dependencies on other areas' data

| Dependency | Area | Used for |
|---|---|---|
| `UiPreferences` | 02-preferences-system | the three theme keys |
| `AppTheme`, `ThemeMode` | 02 (domain/ui/model) | enum types |
| `MR` / `AYMR` strings | 49 part 2 + i18n | scheme titles (see `p1-behavior.md` section 12) |
| `ItemCover.Book.ratio` | 38-39 | theme picker preview cover |
| Backup of preferences | 21-22 | whether `pref_app_theme` is included in backups is decided there; this area defines no backup code |
