# Area 49 (part 2): data models, preference keys, resources and config

Root: `/home/blackbox/aniyomi-src/`. Repo-relative paths. No network endpoints and no database tables are touched by this scope. The only persisted state is three preference keys (section 2) and the enum names they store.

## 1. Data models

### 1.1 `AppTheme` (enum)
Source: `app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt`. Constructor `(val titleRes: StringResource?)`.

| Constant | Stored name (`enum.name`) | `titleRes` key | Selectable |
|---|---|---|---|
| DEFAULT | `DEFAULT` | `MR.strings.label_default` | yes |
| MONET | `MONET` | `MR.strings.theme_monet` | yes (only if dynamic colour available, else not shown on Linux) |
| CLOUDFLARE | `CLOUDFLARE` | `AYMR.strings.theme_cloudflare` | yes |
| COTTONCANDY | `COTTONCANDY` | `AYMR.strings.theme_cottoncandy` | yes |
| DOOM | `DOOM` | `AYMR.strings.theme_doom` | yes |
| GREEN_APPLE | `GREEN_APPLE` | `MR.strings.theme_greenapple` | yes |
| LAVENDER | `LAVENDER` | `MR.strings.theme_lavender` | yes |
| MATRIX | `MATRIX` | `AYMR.strings.theme_matrix` | yes |
| MIDNIGHT_DUSK | `MIDNIGHT_DUSK` | `MR.strings.theme_midnightdusk` | yes |
| MOCHA | `MOCHA` | `AYMR.strings.theme_mocha` | yes |
| SAPPHIRE | `SAPPHIRE` | `AYMR.strings.theme_sapphire` | yes |
| NORD | `NORD` | `MR.strings.theme_nord` | yes |
| STRAWBERRY_DAIQUIRI | `STRAWBERRY_DAIQUIRI` | `MR.strings.theme_strawberrydaiquiri` | yes |
| TAKO | `TAKO` | `MR.strings.theme_tako` | yes |
| TEALTURQUOISE | `TEALTURQUOISE` | `MR.strings.theme_tealturquoise` | yes |
| TIDAL_WAVE | `TIDAL_WAVE` | `MR.strings.theme_tidalwave` | yes |
| YINYANG | `YINYANG` | `MR.strings.theme_yinyang` | yes |
| YOTSUBA | `YOTSUBA` | `MR.strings.theme_yotsuba` | yes |
| MONOCHROME | `MONOCHROME` | `MR.strings.theme_monochrome` | yes |
| DARK_BLUE | `DARK_BLUE` | `null` | no (deprecated) |
| HOT_PINK | `HOT_PINK` | `null` | no (deprecated) |
| BLUE | `BLUE` | `null` | no (deprecated) |

Count: 22 constants; 19 selectable (the Dynamic row is hidden on devices without dynamic colour, see section 2.1 for the Linux decision).

### 1.2 `ThemeMode` (enum)
Source: `app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt`. Constants `LIGHT`, `DARK`, `SYSTEM`. Stored as `"LIGHT"`, `"DARK"`, `"SYSTEM"`. Default `SYSTEM`.

### 1.3 `EmptyScreenAction` (data class)
Source: `presentation-core/src/main/java/tachiyomi/presentation/core/screens/EmptyScreen.kt`.
- `stringRes: StringResource` (non-null, moko).
- `icon: ImageVector` (non-null).
- `onClick: () -> Unit` (non-null).
- Passed as `ImmutableList<EmptyScreenAction>?`; null or empty means no action row.

### 1.4 `FabPosition` (value class over Int)
Source: `Scaffold.kt` line 320. `Center = FabPosition(0)`, `End = FabPosition(1)`. Default `End`. Internal constructor.

### 1.5 `FabPlacement` (internal class)
Source: `Scaffold.kt` line 352. `left: Int` (px, offset from left edge of the layout, RTL-adjusted), `width: Int` (px), `height: Int` (px). `@Immutable`.

### 1.6 `ScaffoldLayoutContent` (private enum)
Source: `Scaffold.kt` line 361. Values in order: `TopBar`, `MainContent`, `Snackbar`, `Fab`, `BottomBar`, `StartBar`. Used only as subcompose slot keys.

### 1.7 `Padding` (class) and `MaterialTheme.padding`
Source: `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt` (component area; used by this scope).

| Field | dp |
|---|---|
| extraLarge | 32 |
| large | 24 |
| medium | 16 |
| mediumSmall | 12 |
| small | 8 |
| extraSmall | 4 |

Other constants in the same file: `DISABLED_ALPHA = .38f`, `SECONDARY_ALPHA = .78f`, `topSmallPaddingValues = PaddingValues(top = MaterialTheme.padding.small)`.

### 1.8 `ColorScheme` role set (Compose M3, 36 roles)
The roles that the schemes address, in the order used in `p2-topic-theme-tokens.md`:
`primary, onPrimary, primaryContainer, onPrimaryContainer, inversePrimary, secondary, onSecondary, secondaryContainer, onSecondaryContainer, tertiary, onTertiary, tertiaryContainer, onTertiaryContainer, background, onBackground, surface, onSurface, surfaceVariant, onSurfaceVariant, surfaceTint, inverseSurface, inverseOnSurface, error, onError, errorContainer, onErrorContainer, outline, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest`.

Two non-M3 keys appear in the XML only: `primaryInverse` (same meaning as `inversePrimary`; used by `cloudflare`, `doom`, `matrix`, `sapphire`, `cottoncandy`, `mocha` XML and read by the View theme) and `elevationOverlay` (value is a reference to the scheme's `primary`, for example `@color/doom_primary`).

### 1.9 Elevation spec (private, Elevation.kt)
| Name | Duration ms | Easing |
|---|---|---|
| DefaultIncomingSpec | 120 | FastOutSlowInEasing |
| DefaultOutgoingSpec | 150 | CubicBezierEasing(0.40, 0.00, 0.60, 1.00) |
| HoveredOutgoingSpec | 120 | CubicBezierEasing(0.40, 0.00, 0.60, 1.00) |

### 1.10 Ripple configuration
`RippleConfiguration(color: Color, rippleAlpha: RippleAlpha)` with `RippleAlpha(draggedAlpha = .1f, focusedAlpha = .1f, hoveredAlpha = .1f, pressedAlpha = .1f)`. Colour `Color.White` in dark system mode, `Color.Black` otherwise (`TachiyomiTheme.kt` lines 93 to 102).

### 1.11 Icon definitions
`ImageVector` with `defaultWidth = 24.0.dp`, `defaultHeight = 24.0.dp`, `viewportWidth = 24.0f`, `viewportHeight = 24.0f`, one path each, black fill `0xFF000000`, Discord and GitHub use `NonZero`, `Butt` cap, `Miter` join, `strokeLineMiter = 4.0f`.

### 1.12 Error faces
`private val ErrorFaces: List<String>` (12 entries, exact strings in `p2-behavior.md` section 9.1).

### 1.13 Window size constants (interaction)
`MediumWidthWindowSize = 600.dp`, `ExpandedWidthWindowSize = 840.dp`; comparisons are strict `>` on `LocalConfiguration.current.screenWidthDp` (`mihon/core/designsystem/utils/WindowSize.kt`).

---

## 2. Preference keys used here

Owner: `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt`. Store: `PreferenceStore` (area 02).

| Key | Accessor | Type | Default | Stored form | Notes |
|---|---|---|---|---|---|
| `pref_app_theme` | `appTheme()` | enum `AppTheme` | `MONET` if `DeviceUtil.isDynamicColorAvailable`, else `DEFAULT` | enum name, e.g. `"TEALTURQUOISE"` | Unknown name returns the default. Deprecated names `DARK_BLUE`, `HOT_PINK`, `BLUE` therefore resolve to the default, not to DEFAULT. |
| `pref_theme_mode_key` | `themeMode()` | enum `ThemeMode` | `SYSTEM` | `"SYSTEM"`, `"LIGHT"`, `"DARK"` | Also drives the AppCompat night mode (`setAppCompatDelegateThemeMode`). |
| `pref_theme_dark_amoled_key` | `themeDarkAmoled()` | Boolean | `false` | `true` or `false` | The key has the odd `_key` suffix; keep it for migration. |

Also read in this scope via other areas: `DeviceUtil.isDynamicColorAvailable` (not a preference; a lazy boolean in `app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt`).

Proposal (Linux): the same three keys, same defaults, stored in the Kurobayomi settings store (area 02 Rust side). On Linux `DeviceUtil.isDynamicColorAvailable` is replaced by "the XDG portal reports an accent colour" (task T-49b-005), so the default is `DEFAULT` unless that is true.

---

## 3. Strings used by this scope

Keys are moko keys (`MR.strings.*` in `i18n/src/commonMain/moko-resources/base/strings.xml`, `AYMR.strings.*` in `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`).

| Key | English value | Used by |
|---|---|---|
| `label_default` | Default | `AppTheme.DEFAULT` title |
| `theme_monet` | Dynamic | `AppTheme.MONET` title |
| `theme_greenapple` | Green Apple | |
| `theme_lavender` | Lavender | |
| `theme_midnightdusk` | Midnight Dusk | |
| `theme_monochrome` | Monochrome | |
| `theme_nord` | Nord | |
| `theme_strawberrydaiquiri` | Strawberry Daiquiri | |
| `theme_tako` | Tako | |
| `theme_tealturquoise` | Teal &amp; Turquoise | (XML entity; displays "Teal & Turquoise") |
| `theme_yinyang` | Yin &amp; Yang | |
| `theme_yotsuba` | Yotsuba | |
| `theme_tidalwave` | Tidal Wave | |
| `theme_cloudflare` (AYMR) | Cloudflare | |
| `theme_cottoncandy` (AYMR) | Cotton Candy | |
| `theme_doom` (AYMR) | Doom | |
| `theme_matrix` (AYMR) | Matrix | |
| `theme_mocha` (AYMR) | Mocha | |
| `theme_sapphire` (AYMR) | Sapphire | |
| `theme_system` | System | theme mode row |
| `theme_light` | Light | theme mode row |
| `theme_dark` | Dark | theme mode row |
| `pref_dark_theme_pure_black` | Pure black dark mode | AMOLED switch |
| `pref_category_theme` | (Theme category header in settings) | `SettingsAppearanceScreen.kt` |
| `pref_app_theme` | (App theme row title) | `SettingsAppearanceScreen.kt` |

The Kurobayomi mockup `spec/settings.json` lines 116 to 146 and `spec/app.json` lines 975 to 1002 hard-code these English titles. Their `default` differs: `spec/settings.json` says `"Dynamic"`, `spec/app.json` says `"Default"`. The Android default on a non-dynamic device is `DEFAULT`. The Rust default is `Default` (see T-49b-006).

---

## 4. Android colour resources (all keys, names only)

The complete list of names per file is in `p2-inventory.md` (part B). Values per scheme are in appendix A below. Resource files in scope, grouped:

- Shared: `presentation-core/src/main/res/values/colors.xml` (`splash`, `cover_placeholder`, `divider_default`, `accent_blue`, `amoled_background`, `amoled_onBackground`, `amoled_surface`, `amoled_onSurface`, `amoled_surfaceContainer`, `amoled_surfaceContainerHigh`, `amoled_surfaceContainerHighest`, `md_black_1000_12`, `md_white_1000_12`); `presentation-core/src/main/res/values-night/colors.xml` (`splash` = `#202125`, `divider_default` = `@color/md_white_1000_12`).
- Per scheme (16 in scope): tachiyomi, monochrome, yotsuba, tako, cloudflare, doom, lavender, matrix, sapphire, greenapple, midnightdusk, nord, strawberry, tealturquoise (file spelled `colors_tealturqoise.xml`), tidalwave, yinyang. Each has `values/` (light) and `values-night/` (dark) files. The two schemes cottoncandy and mocha are in `app/src/main/res/` (not in scope) and are listed in appendix A for completeness.

Known oddities (observed):
- `values/color_cloudflare.xml`, `color_doom.xml`, `color_lavender.xml`, `color_matrix.xml`, `color_sapphire.xml` use the singular `color_` prefix; the other scheme files use `colors_`. The Kotlin counterpart names are not affected.
- `values/color_matrix.xml` header says "Colors for Doom theme" (copy-paste). Same for the light and dark matrix header comments in `values-night/color_matrix.xml`.
- `values-night/colors_tidalwave.xml` header says `Primary #004152` while its values use `#5ed4fc`. The XML comments are not authoritative.
- The `on_surface_variant` colours of cloudflare, doom, matrix and sapphire in the light folder are 8-digit `#D849454E` (ARGB with alpha `0xD8` = 216/255 = 0.847). The dark folder uses `#D8FFFFFF`. The Compose Kotlin uses the same values (for example `CloudflareColorScheme.kt` line 65: `Color(0xFFD849454E)`, and line 39: `Color(0xFFD8FFFFFF)`).
- `tealturquoise` file name typo: `colors_tealturqoise.xml`; Kotlin class is `TealTurqoiseColorScheme`. Keep the typo only in migration notes.
- `values/colors_tidalwave.xml` has `#edf4fA` (mixed case). Harmless.

---

## 5. Files on disk

Inside this scope:
- `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/` (7 Kotlin files)
- `presentation-core/src/main/java/tachiyomi/presentation/core/i18n/Localize.kt`
- `presentation-core/src/main/java/tachiyomi/presentation/core/icons/` (4 Kotlin files: `CustomIcons.kt`, `Discord.kt`, `Github.kt`, `Magnet.kt`)
- `presentation-core/src/main/java/tachiyomi/presentation/core/screens/` (3 Kotlin files)
- `presentation-core/src/main/java/tachiyomi/presentation/core/theme/` (`Color.kt`, `Typography.kt`)
- `presentation-core/src/main/java/tachiyomi/presentation/core/util/` (7 Kotlin files)
- `presentation-core/src/main/res/values/` and `values-night/` (34 colour XML files)

Outside (interaction): `app/src/main/res/values/themes.xml`, `app/src/main/res/values-night/themes.xml`, `app/src/main/res/values-v31/themes.xml`, `app/src/main/res/values-night-v31/themes.xml` (View themes; they reference the XML colours), `app/src/main/res/values/colors_cottoncandy.xml`, `app/src/main/res/values/colors_mocha.xml` (+ night), `app/src/main/res/drawable/cover_error.xml` (`@color/cover_placeholder`), `app/src/main/res/drawable/sc_*_48dp.xml` (`@color/accent_blue` background), `app/src/main/res/values/styles.xml` (`dividerColor`), `presentation-widget/src/main/res/drawable/appwidget_cover_error.xml` (`@color/cover_placeholder`).

Rust target (proposal): no resource files at run time. Tokens are `const` data in Rust (`src/theme/schemes.rs`), icons are a GResource bundle (`assets/icons/*.svg`, compiled by `glib-build-tools`), and gettext catalogs live in `po/` (see `p2-rust-design.md`).

---

## 6. Network endpoints, DB tables, bit flags
- Network: none in this scope.
- DB tables: none in this scope.
- Bit-flag layouts: none. The only numeric encodings are ARGB hex literals (`0xAARRGGBB` in XML and Kotlin, with alpha in the top byte, e.g. `#D8FFFFFF` is alpha 0xD8).

---

## 7. Compose-side numeric constants that must match

| Constant | Value | Where |
|---|---|---|
| AMOLED container tones | `#0C0C0C`, `#131313`, `#1B1B1B` | BaseColorScheme.kt lines 13 to 15 |
| Pure black | `Color.Black` (`#000000`) for background and surface | BaseColorScheme.kt lines 23, 25 |
| Pure white for AMOLED text | `Color.White` | BaseColorScheme.kt lines 24, 26 |
| Active tint dark | `#FFEB3B` (255,235,59) | theme/Color.kt line 11 |
| Active tint light | `#FFC107` (255,193,7) | theme/Color.kt line 11 |
| Surface tint alpha | `((4.5 * ln(dp + 1)) + 2) / 100` | Surface.kt line 111 |
| Scrollbar alpha | `0.364f` | Scrollbar.kt line 233 |
| Splash dark | `#202125` | values-night/colors.xml |
| Accent blue | `#54759E` | values/colors.xml |
| Cover placeholder | `#1F888888` (alpha 0x1F = 31/255 = 12.2%) | values/colors.xml |
| Divider light | `#1F000000` (= `md_black_1000_12`) | values/colors.xml |
| Divider dark | `#1FFFFFFF` (= `md_white_1000_12`) | values/colors.xml (`md_white_1000_12`) and values-night/colors.xml |

---

## Appendix A: colour tokens per scheme (generated from the XML files)

Generated by reading every `<color name="...">` element of the 32 scheme XML files in scope (16 schemes in two modes; the two shared `colors.xml` files are in section 4) plus the four app-level files for Cotton Candy and Mocha that the View theme references. Layout: one row per role (the XML key is `<scheme>_<role>`, scheme prefix omitted here), one column per scheme. Cell format: `light / dark` hex as written in `values/` and `values-night/`. `-` means the key is not declared in that scheme's XML (the Compose Kotlin scheme may still define the role; see `p2-topic-theme-tokens.md`, section 3). Column keys: Default is `tachiyomi`; Teal & Turquoise is `tealturquoise` (file `colors_tealturqoise.xml`); Cotton Candy and Mocha are in `app/src/main/res/`.

| Role | Default | Monochrome | Yotsuba | Tako | Cloudflare | Doom | Lavender | Matrix | Sapphire | Green Apple | Midnight Dusk | Nord | Strawberry Daiquiri | Teal & Turquoise | Tidal Wave | Yin & Yang | Cotton Candy | Mocha |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| primary | #0058CA / #B0C6FF | #000000 / #FFFFFF | #AE3200 / #FFB59D | #66577E / #F3B375 | #F38020 / #F38020 | #FF0000 / #FF0000 | #6D41C8 / #A177FF | #00FF00 / #00FF00 | #1E88E5 / #1E88E5 | #005927 / #7ADB8F | #BB0054 / #F02475 | #5E81AC / #88C0D0 | #A10833 / #FFB2B8 | #008080 / #40E0D0 | #006780 / #5ed4fc | #000000 / #FFFFFF | #9A4058 / #FFB1C1 | #745b00 / #ebc248 |
| onPrimary | #FFFFFF / #002D6E | #FFFFFF / #000000 | #FFFFFF / #5F1600 | #F3B375 / #38294E | #EFF2F5 / #1B1B22 | #FFFFFF / #FAFAFA | #FFFFFF / #3D0090 | #000000 / #FAFAFA | #FFFFFF / #FAFAFA | #FFFFFF / #003917 | #FFFFFF / #FFFFFF | #000000 / #2E3440 | #FFFFFF / #67001D | #FFFFFF / #000000 | #ffffff / #003544 | #FFFFFF / #5A5A5A | #FFFFFF / #5F112B | #ffffff / #3d2f00 |
| primaryContainer | #D9E2FF / #00429B | #000000 / #FFFFFF | #FFDBCF / #862200 | #66577E / #F3B375 | #F38020 / #F38020 | #FF0000 / #FF0000 | #7B46AF / #A177FF | #00FF00 / #00FF00 | #1E88E5 / #1E88E5 | #188140 / #017737 | #FFD9E1 / #BD1C5C | #5E81AC / #88C0D0 | #D53855 / #D53855 | #008080 / #40E0D0 | #B4D4DF / #004d61 | #000000 / #FFFFFF | #FFD9DF / #7C2941 | #ffe08d / #584400 |
| onPrimaryContainer | #001945 / #D9E2FF | #FFFFFF / #000000 | #3B0A00 / #FFDBCF | #F3B375 / #38294E | #EFF2F5 / #1B1B22 | #FFFFFF / #FAFAFA | #130038 / #FFFFFF | #000000 / #FAFAFA | #FFFFFF / #FAFAFA | #FFFFFF / #FFFFFF | #3F0017 / #FFFFFF | #000000 / #2E3440 | #FFFFFF / #FFFFFF | #FFFFFF / #000000 | #001f28 / #b8eaff | #FFFFFF / #000000 | #3F0017 / #FFD9DF | #241a00 / #ffe08d |
| inversePrimary | #B0C6FF / #0058CA | #FFFFFF / #000000 | #FFB59D / #AE3200 | #D6BAFF / #84531E | - / - | - / - | #A177FF / #6D41C8 | - / - | - / - | #7ADB8F / #006D32 | #FFB1C4 / #F02475 | #8CA8CD / #397E91 | #FFB2B8 / #B61F40 | #40E0D0 / #008080 | #ff987f / #a12b03 | #A6A6A6 / #CECECE | - / - | - / - |
| secondary | #0058CA / #B0C6FF | #000000 / #FFFFFF | #AE3200 / #FFB59D | #66577E / #F3B375 | #F38020 / #F38020 | #FF0000 / #FF0000 | #7B46AF / #A177FF | #00FF00 / #00FF00 | #1E88E5 / #1E88E5 | #005927 / #7ADB8F | #BB0054 / #F02475 | #81A1C1 / #81A1C1 | #A10833 / #ED4A65 | #008080 / #40E0D0 | #006780 / #5ed4fc | #000000 / #FFFFFF | #5bcefa / #64D3FF | #745b00 / #ebc248 |
| onSecondary | #FFFFFF / #002D6E | #FFFFFF / #000000 | #FFFFFF / #5F1600 | #F3B375 / #38294E | #EFF2F5 / #1B1B22 | #FFFFFF / #FAFAFA | #EDE2FF / #FFFFFF | #000000 / #FAFAFA | #FFFFFF / #FAFAFA | #FFFFFF / #003917 | #FFFFFF / #16151D | #2E3440 / #2E3440 | #FFFFFF / #201A1A | #FFFFFF / #000000 | #ffffff / #003544 | #FFFFFF / #5A5A5A | #FFFFFF / #003546 | #ffffff / #3d2f00 |
| secondaryContainer | #D9E2FF / #00429B | #888888 / #777777 | #EBCDC2 / #862200 | #C8BED0 / #5C4D4B | #F38020 / #F38020 | #FF0000 / #FF0000 | #C9B0E6 / #423271 | #00FF00 / #00FF00 | #1E88E5 / #1E88E5 | #97f7a9 / #017737 | #EFBAD4 / #66183C | #91B4D7 / #506275 | #D53855 / #91002A | #CFE5E4 / #18544E | #9AE1FF / #004d61 | #DDDDDD / #717171 | #BCE9FF / #004D63 | #ffe08d / #584400 |
| onSecondaryContainer | #001945 / #D9E2FF | #FFFFFF / #000000 | #3B0A00 / #FFDBCF | #66577E / #F3B375 | #EFF2F5 / #1B1B22 | #FFFFFF / #FAFAFA | #7B46AF / #A177FF | #000000 / #FAFAFA | #FFFFFF / #FAFAFA | #000000 / #FFFFFF | #D1377C / #F02475 | #2E3440 / #88C0D0 | #F6EAED / #FFFFFF | #008080 / #40E0D0 | #001f28 / #b8eaff | #0C0C0C / #E4E4E4 | #001F2A / #BCE9FF | #241a00 / #ffe08d |
| tertiary | #006E1B / #7ADC77 | #888888 / #777777 | #6B5E2F / #D7C68D | #F3B375 / #66577E | #EFF2F5 / #1B1B22 | #BFBFBF / #BFBFBF | #EDE2FF / #CDBDFF | #000000 / #FFFFFF | #E1F5FE / #212121 | #9D0012 / #FFB3AC | #006638 / #55971C | #88C0D0 / #5E81AC | #5F441D / #E8C08E | #FF7F7F / #BF1F2F | #92f7bc / #92f7bc | #FFFFFF / #000000 | #9A4058 / #FFB1C1 | #47664a / #adcfad |
| onTertiary | #FFFFFF / #003909 | #FFFFFF / #FFFFFF | #FFFFFF / #3A2F05 | #574360 / #F3B375 | #F38020 / #F38020 | #FF0000 / #FF0000 | #7B46AF / #360096 | #00FF00 / #00FF00 | #1E88E5 / #1E88E5 | #FFFFFF / #680008 | #FFFFFF / #16151D | #2E3440 / #000000 | #FFFFFF / #201A1A | #000000 / #FFFFFF | #001c3b / #001c3b | #000000 / #FFFFFF | #FFFFFF / - | #ffffff / #19361f |
| tertiaryContainer | #95F990 / #005312 | #000000 / #FFFFFF | #F5E2A7 / #524619 | #FDD6B0 / #4E4065 | #EFF2F5 / #1B1B22 | #BFBFBF / #BFBFBF | #6D3BF0 / #5512D8 | #000000 / #FFFFFF | #E1F5FE / #212121 | #D33131 / #C7282A | #00894b / #386412 | #88C0D0 / #5E81AC | #87683D / #775930 | #2A1616 / #200508 | #c3fada / #c3fada | #D8E2FF / #00419E | #FFD9DF / #7C2941 | #c9ecc8 / #304d34 |
| onTertiaryContainer | #002203 / #95F990 | #FFFFFF / #000000 | #231B00 / #F5E2A7 | #221437 / #EDDCFF | #F38020 / #F38020 | #FF0000 / #FF0000 | #FFFFFF / #EFE6FF | #00FF00 / #00FF00 | #1E88E5 / #1E88E5 | #FFFFFF / #FFFFFF | #2D1600 / #E5E1E5 | #2E3440 / #000000 | #FFFFFF / #FFF7F1 | #FF7F7F / #BF1F2F | #78ffd6 / #78ffd6 | #001947 / #D8E2FF | #3F0017 / #FFD9DF | #04210b / #c9ecc8 |
| background | #FEFBFF / #1B1B1F | #FFFFFF / #000000 | #FCFCFC / #211A18 | #F7F5FF / #21212E | #EFF2F5 / #1B1B22 | #212121 / #1B1B1B | #EDE2FF / #111129 | #000000 / #111111 | #FFFFFF / #212121 | #F6FBF2 / #0F1510 | #FFFBFF / #16151D | #ECEFF4 / #2E3440 | #FAFAFA / #201A1A | #FAFAFA / #202125 | #fdfbff / #001c3b | #FDFDFD / #1E1E1E | #FFFBFF / #201A1B | #fffbff / #1e1b16 |
| onBackground | #1B1B1F / #E3E2E6 | #000000 / #FFFFFF | #211A18 / #EDE0DD | #1B1B22 / #E3E0F2 | #1B1B22 / #EFF2F5 | #FFFFFF / #FFFFFF | #1D1A22 / #E7E0EC | #FFFFFF / #FFFFFF | #212121 / #FFFFFF | #181D18 / #DFE4DB | #1C1B1F / #E5E1E5 | #2E3440 / #ECEFF4 | #261819 / #F7DCDD | #050505 / #DFDEDA | #001c3b / #d5e3ff | #222222 / #E6E6E6 | #201A1B / #ECE0E0 | #1e1b16 / #e8e1d9 |
| surface | #FEFBFF / #1B1B1F | #FFFFFF / #000000 | #FCFCFC / #211A18 | #F7F5FF / #21212E | #EFF2F5 / #1B1B22 | #212121 / #1B1B1B | #EDE2FF / #111129 | #000000 / #111111 | #FFFFFF / #212121 | #F6FBF2 / #0F1510 | #FFFBFF / #16151D | #E5E9F0 / #2E3440 | #FAFAFA / #201A1A | #FAFAFA / #202125 | #fdfbff / #001c3b | #FDFDFD / #1E1E1E | #FFFBFF / #201A1B | #fffbff / #1e1b16 |
| onSurface | #1B1B1F / #E3E2E6 | #000000 / #FFFFFF | #211A18 / #EDE0DD | #1B1B22 / #E3E0F2 | #1B1B22 / #EFF2F5 | #FFFFFF / #FFFFFF | #1D1A22 / #E7E0EC | #FFFFFF / #FFFFFF | #212121 / #FFFFFF | #181D18 / #DFE4DB | #1C1B1F / #E5E1E5 | #2E3440 / #ECEFF4 | #261819 / #F7DCDD | #050505 / #DFDEDA | #001c3b / #d5e3ff | #222222 / #E6E6E6 | #201A1B / #ECE0E0 | #1e1b16 / #e8e1d9 |
| surfaceVariant | #F3EDF7 / #211F26 | #FFFFFF / #000000 | #F6EBE7 / #332723 | #E8E0EB / #2A2A3C | #B9B0CC / #3F3F46 | #4D4D4D / #303030 | #E4D5F8 / #3D2F6B | #111111 / #212121 | #B3E5FC / #424242 | #DAE6D7 / #3F493F | #F9E6F1 / #281624 | #DAE0EA / #414C5C | #F6EAED / #322727 | #EBF3F1 / #233133 | #e8eff5 / #082b4b | #E8E8E8 / #313131 | #F3DDE0 / #524345 | #ebe1cf / #4c4639 |
| onSurfaceVariant | #44464F / #C5C6D0 | #000000 / #FFFFFF | #53433F / #D8C2BC | #49454E / #CBC4CE | #D849454E / #D8FFFFFF | #D849454E / #D8FFFFFF | #4A4453 / #CBC3D6 | #D849454E / #D8FFFFFF | #D849454E / #D8FFFFFF | #3F493F / #BECABC | #524346 / #D6C1C4 | #2E3440 / #ECEFF4 | #594042 / #E1BEC0 | #050505 / #DFDEDA | #40484c / #bfc8cc | #515151 / #D1D1D1 | #524345 / #D6C2C4 | #4c4639 / #cfc5b4 |
| surfaceTint | #0058CA / #B0C6FF | - / - | #AE3200 / #FFB59D | #66577E / #66577E | - / - | - / - | - / - | - / - | - / - | - / - | #BB0054 / #F02475 | #5E81AC / #88C0D0 | - / - | #BFDFDF / #40E0D0 | #006780 / #5ed4fc | #000000 / #FFFFFF | - / - | - / - |
| inverseSurface | #303034 / #E3E2E6 | #000000 / #FFFFFF | #362F2D / #EDE0DD | #313033 / #E5E1E6 | #313033 / #F3EFF4 | #424242 / #FAFAFA | #322F38 / #E7E0EC | #424242 / #FAFAFA | #424242 / #FAFAFA | #2C322C / #DFE4DB | #313033 / #333043 | #3B4252 / #D8DEE9 | #3D2C2D / #F7DCDD | #050505 / #DFDEDA | #020400 / #ffe3c4 | #333333 / #E6E6E6 | #352F30 / #ECE0E0 | #362F2D / #EDE0DD |
| inverseOnSurface | #F2F0F4 / #1B1B1F | #FFFFFF / #000000 | #FBEEEB / #211A18 | #F3EFF4 / #1B1B1E | #F3EFF4 / #313033 | #FAFAFA / #313131 | #F5EEFA / #322F38 | #FAFAFA / #313131 | #FAFAFA / #313131 | #EDF2E9 / #2C322C | #F4F0F4 / #FFFFFF | #ECEFF4 / #2E3440 | #FFECED / #3D2C2D | #FAFAFA / #202125 | #ffe3c4 / #001c3b | #F4F4F4 / #1E1E1E | #FAEEEF / #201A1B | #FBEEEB / #211A18 |
| error | #BA1A1A / #FFB4AB | #000000 / #FFFFFF | - / - | - / - | - / - | - / - | #BA1A1A / #FFB4AB | - / - | - / - | #BA1A1A / #FFB4AB | - / - | - / - | #BA1A1A / #FFB4AB | - / - | - / - | - / - | - / - | - / - |
| onError | #FFFFFF / #690005 | #FFFFFF / #000000 | - / - | - / - | - / - | - / - | #FFFFFF / #690005 | - / - | - / - | #FFFFFF / #690005 | - / - | #ECEFF4 / #2E3440 | #FFFFFF / #690005 | - / - | - / - | - / - | - / - | - / - |
| errorContainer | #FFDAD6 / #93000A | #000000 / #FFFFFF | - / - | - / - | - / - | - / - | #FFDAD6 / #93000A | - / - | - / - | #FFDAD6 / #93000A | - / - | #BF616A / #BF616A | #FFDAD6 / #93000A | - / - | - / - | - / - | - / - | - / - |
| onErrorContainer | #410002 / #FFDAD6 | #FFFFFF / #000000 | - / - | - / - | - / - | - / - | #410002 / #FFDAD6 | - / - | - / - | #410002 / #FFDAD6 | - / - | #000000 / #000000 | #410002 / #FFDAD6 | - / - | - / - | - / - | - / - | - / - |
| outline | #757780 / #8F9099 | #000000 / #FFFFFF | #85736E / #A08C87 | #7A757E / #958F99 | #F38020 / #F38020 | #FF0000 / #FF0000 | #7B7485 / #958E9F | #00FF00 / #00FF00 | #1E88E5 / #1E88E5 | #6F7A6E / #889487 | #847376 / #9F8C8F | #2E3440 / #6d717b | #8D7071 / #A9898B | #6F7977 / #899391 | #70787c / #8a9296 | #838383 / #999999 | #847375 / #9F8C8F | #7e7667 / #989080 |
| outlineVariant | #C5C6D0 / #44464F | #000000 / #FFFFFF | - / - | - / - | - / - | - / - | #CBC3D6 / #4A4453 | - / - | - / - | #BECABC / #3F493F | - / - | - / #90939a | #E1BEC0 / #594042 | - / - | - / - | - / - | - / - | - / - |
| scrim | - / - | #000000 / #000000 | - / - | - / - | - / - | - / - | #000000 / #000000 | - / - | - / - | #000000 / #000000 | - / - | - / - | #000000 / #000000 | - / - | - / - | - / - | - / - | - / - |
| surfaceBright | - / - | #FFFFFF / #FFFFFF | - / - | - / - | - / - | - / - | #EDE2FF / #3B3841 | - / - | - / - | #F6FBF2 / #353B35 | - / - | - / - | #FFF8F7 / #463536 | - / - | - / - | - / - | - / - | - / - |
| surfaceDim | - / - | #000000 / #000000 | - / - | - / - | - / - | - / - | #DED7E3 / #111129 | - / - | - / - | #D6DCD3 / #0F1510 | - / - | - / - | #EED4D5 / #1D1011 | - / - | - / - | - / - | - / - | - / - |
| surfaceContainer | #F3EDF7 / #211F26 | #FFFFFF / #000000 | #F6EBE7 / #332723 | #E8E0EB / #2A2A3C | - / - | - / - | #E4D5F8 / #1D193B | - / - | - / - | #EAEFE6 / #1C211C | #F9E6F1 / #281624 | #DAE0EA / #414C5C | #F6EAED / #322727 | #EBF3F1 / #233133 | #e8eff5 / #082b4b | #E8E8E8 / #313131 | - / - | - / - |
| surfaceContainerHigh | #FCF7FF / #292730 | #FFFFFF / #000000 | #FAF4F2 / #413531 | #EEE6F1 / #303044 | - / - | - / - | #EADCFD / #241f41 | - / - | - / - | #E4EAE1 / #262B26 | #FCF3F8 / #2D1C2A | #E9EDF3 / #4E5766 | #FFF0F0 / #3C2F2F | #F0F8F6 / #28383A | #edf4fA / #093257 | #ECECEC / #383838 | - / - | - / - |
| surfaceContainerHighest | #FCF7FF / #302E38 | #FFFFFF / #000000 | #FBF6F4 / #4C403D | #F7EEFA / #36364D | - / - | - / - | #EEE2FF / #282446 | - / - | - / - | #DFE4DB / #313630 | #FEF9FC / #2F1F2C | #F2F4F8 / #505968 | #FFFFFF / #463737 | #F7FFFD / #2F4244 | #f5faff / #0A3861 | #EFEFEF / #3F3F3F | - / - | - / - |
| surfaceContainerLow | #F7F2FA / #1E1C22 | #FFFFFF / #000000 | #F1E7E4 / #312521 | #DFD8E2 / #262636 | - / - | - / - | #DED0F1 / #171531 | - / - | - / - | #F0F5EC / #181D18 | #E8D1DD / #251522 | #D6DCE6 / #3E4756 | #FDE2E3 / #302525 | #E6EEEC / #222F31 | #e5ecf1 / #072947 | #DADADA / #2D2D2D | - / - | - / - |
| surfaceContainerLowest | #F5F1F8 / #1A181D | #FFFFFF / #000000 | #ECE3E0 / #2E221F | #D7D0DA / #20202E | - / - | - / - | #DACCEC / #15132d | - / - | - / - | #FFFFFF / #0A0F0B | #DAC0CD / #221320 | #D1D7E0 / #373F4D | #F7DCDD / #2C2222 | #E1E9E7 / #202C2E | #e2e8ec / #072642 | #CFCFCF / #2A2A2A | - / - | - / - |
| primaryInverse | - / - | - / - | - / - | - / - | #D6BAFF / #D6BAFF | #6D0D0B / #6D0D0B | - / - | #007700 / #007700 | #2979FF / #2979FF | - / - | - / - | - / - | - / - | - / - | - / - | - / - | #FFB1C1 / #9A4058 | #FFB59D / #AE3200 |
| elevationOverlay | - / - | - / - | - / - | - / - | @color/cloudflare_primary / @color/cloudflare_primary | @color/doom_primary / @color/doom_primary | - / - | @color/matrix_primary / @color/matrix_primary | @color/sapphire_primary / @color/sapphire_primary | - / - | - / - | - / - | - / - | - / - | - / - | - / - | @color/cottoncandy_primary / @color/cottoncandy_primary | - / - |

## Appendix B: XML role coverage per scheme

Counts include the two non-M3 keys (`primaryInverse`, `elevationOverlay`). "Missing" lists M3 roles absent from the light XML; dark XML coverage is the same except where stated by the counts.

| Scheme (key) | Light roles present (of 36 M3 + 2 extra) | Dark roles present | Missing in light XML (M3 roles) |
|---|---|---|---|
| Default (`tachiyomi`) | 33 | 33 | scrim, surfaceBright, surfaceDim |
| Monochrome (`monochrome`) | 35 | 35 | surfaceTint |
| Yotsuba (`yotsuba`) | 28 | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Tako (`tako`) | 28 | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Cloudflare (`cloudflare`) | 23 | 23 | inversePrimary, surfaceTint, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Doom (`doom`) | 23 | 23 | inversePrimary, surfaceTint, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Lavender (`lavender`) | 35 | 35 | surfaceTint |
| Matrix (`matrix`) | 23 | 23 | inversePrimary, surfaceTint, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Sapphire (`sapphire`) | 23 | 23 | inversePrimary, surfaceTint, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Green Apple (`greenapple`) | 35 | 35 | surfaceTint |
| Midnight Dusk (`midnightdusk`) | 28 | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Nord (`nord`) | 31 | 32 | error, outlineVariant, scrim, surfaceBright, surfaceDim |
| Strawberry Daiquiri (`strawberry`) | 35 | 35 | surfaceTint |
| Teal & Turquoise (`tealturquoise`) | 28 | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Tidal Wave (`tidalwave`) | 28 | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Yin & Yang (`yinyang`) | 28 | 28 | error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim |
| Cotton Candy (`cottoncandy`) | 23 | 22 | inversePrimary, surfaceTint, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
| Mocha (`mocha`) | 22 | 22 | inversePrimary, surfaceTint, error, onError, errorContainer, onErrorContainer, outlineVariant, scrim, surfaceBright, surfaceDim, surfaceContainer, surfaceContainerHigh, surfaceContainerHighest, surfaceContainerLow, surfaceContainerLowest |
