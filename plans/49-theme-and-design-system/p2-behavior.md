# Area 49 (part 2): behavior specification

Root: `/home/blackbox/aniyomi-src/`. Every path is repo-relative. OBSERVED describes the Kotlin. "Proposal:" describes the Rust behavior. Constants are quoted as they appear in source. Where a value comes from the Android or Compose framework rather than this repo, it is marked **(framework)**.

Related: `p2-inventory.md` (file list), `p2-data-and-config.md` (tokens, keys, values), `p2-rust-design.md`, `p2-ui-parity.md`, `p2-edge-cases-and-tests.md`, `p2-tasks.md`, `p2-topic-theme-tokens.md`, `p2-topic-layout-widgets.md`.

---

## 1. Theme pipeline (OBSERVED)

### 1.1 Entry point: `TachiyomiTheme`
Source: `app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt` (interaction, area 49 part 1).

1. `TachiyomiTheme(appTheme: AppTheme? = null, amoled: Boolean? = null, content)` reads `UiPreferences` from Injekt.
2. `appTheme = appTheme ?: uiPreferences.appTheme().get()`.
3. `isAmoled = amoled ?: uiPreferences.themeDarkAmoled().get()`.
4. `BaseTachiyomiTheme(appTheme, isAmoled, content)` calls `MaterialTheme(colorScheme = getThemeColorScheme(appTheme, isAmoled), content)`.
5. `getThemeColorScheme`:
   - if `appTheme == AppTheme.MONET`: `MonetColorScheme(LocalContext.current)`;
   - else `colorSchemes.getOrDefault(appTheme, TachiyomiColorScheme)`. The map `colorSchemes` has 18 entries (DEFAULT, CLOUDFLARE, COTTONCANDY, DOOM, GREEN_APPLE, LAVENDER, MATRIX, MIDNIGHT_DUSK, MONOCHROME, MOCHA, SAPPHIRE, NORD, STRAWBERRY_DAIQUIRI, TAKO, TEALTURQUOISE, TIDAL_WAVE, YINYANG, YOTSUBA). `DARK_BLUE`, `HOT_PINK` and `BLUE` are not in the map and fall back to `TachiyomiColorScheme`.
   - returns `colorScheme.getColorScheme(isSystemInDarkTheme(), isAmoled)`.
6. `TachiyomiPreviewTheme(appTheme = AppTheme.DEFAULT, isAmoled = false, content)` is the same without the preference reads (used by previews and by the theme picker sample card).

### 1.2 Scheme resolution: `BaseColorScheme.getColorScheme`
Source: `app/src/main/java/eu/kanade/presentation/theme/colorscheme/BaseColorScheme.kt`.

```
fun getColorScheme(isDark: Boolean, isAmoled: Boolean): ColorScheme {
    if (!isDark) return lightScheme
    if (!isAmoled) return darkScheme
    return darkScheme.copy(background = Black, onBackground = White, surface = Black, onSurface = White,
        surfaceVariant = #0C0C0C, surfaceContainerLowest = #0C0C0C, surfaceContainerLow = #0C0C0C,
        surfaceContainer = #0C0C0C, surfaceContainerHigh = #131313, surfaceContainerHighest = #1B1B1B)
}
```

Rules:
- AMOLED is applied only when `isDark` is true. In light mode the flag has no effect.
- The AMOLED override is applied to every scheme, including the Monet scheme.
- `#0C0C0C`, `#131313`, `#1B1B1B` are private constants on `BaseColorScheme` (comment: "Cannot be pure black as there's content scrolling behind it"). Only the background and surface roles become pure black.
- `isSystemInDarkTheme()` reads the effective `uiMode` of the configuration. After the AppCompat night mode is applied (see section 3), it reflects the user's app theme mode, not only the OS setting.

### 1.3 Scheme objects (Compose side)
- Each `AppTheme` has one `object XxxColorScheme : BaseColorScheme()` with `darkScheme = darkColorScheme(<role = Color(...) arguments>)` and `lightScheme = lightColorScheme(<role = Color(...) arguments>)`.
- Only the roles listed in the Kotlin constructor call are set. The rest take the Compose M3 baseline defaults from `darkColorScheme()` / `lightColorScheme()` of the pinned Compose M3 version. Coverage per scheme is listed in `p2-topic-theme-tokens.md`, section 3. Example: Cloudflare, Doom, Matrix, Sapphire, Mocha and Cotton Candy set 23 roles (Kotlin, light and dark); the 13 M3 roles `error`, `onError`, `errorContainer`, `onErrorContainer`, `outlineVariant`, `scrim`, `surfaceBright`, `surfaceDim`, `surfaceContainer`, `surfaceContainerHigh`, `surfaceContainerHighest`, `surfaceContainerLow`, `surfaceContainerLowest` fall back to the baseline (`p2-topic-theme-tokens.md` section 3).
- Most Kotlin scheme objects carry a KDoc header with "Key colors" text copied from the matching XML header comment (for example `TachiyomiColorScheme.kt` says `Neutral #919094` while `values/colors_tachiyomi.xml` says `Neutral #5E5E62`; the headers are not authoritative).

### 1.4 Monet (Dynamic) scheme
Source: `app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonetColorScheme.kt`.
- SDK >= 31 (`Build.VERSION_CODES.S`): `MonetSystemColorScheme` uses `dynamicLightColorScheme(context)` and `dynamicDarkColorScheme(context)`.
- SDK >= 27 (`O_MR1`) and < 31: `WallpaperManager.getInstance(context).getWallpaperColors(WallpaperManager.FLAG_SYSTEM)?.primaryColor?.toArgb()`. If non-null, `MonetCompatColorScheme(context, seed)` builds both schemes with `SchemeContent(Hct.fromInt(seed), dark, contrast)`, where `contrast` is `UiModeManager.contrast` on SDK >= 34 (`UPSIDE_DOWN_CAKE`) else `0.0`. If null, `TachiyomiColorScheme`.
- Otherwise `TachiyomiColorScheme`.
- `extractSeedColorFromImage(bitmap)`: `QuantizerCelebi.quantize(pixels, 128)` then `Score.score(QuantizerCelebi.quantize(bitmapPixels, 128), 1, 0)[0]`, returns null when the result is 0. Annotated `@Suppress("Unused")`: not called from this scope.
- Availability gate: `DeviceUtil.isDynamicColorAvailable` = `DynamicColors.isDynamicColorAvailable() || (DeviceUtil.isSamsung && SDK_INT >= S)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt`).
- Default preference: `pref_app_theme` defaults to `MONET` when the gate is true, else `DEFAULT` (section 2).

### 1.5 Ripple configuration for the player
Source: `app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt`.
- `playerRippleConfiguration` is a composable getter returning `RippleConfiguration(color, rippleAlpha)`.
- `color = if (isSystemInDarkTheme()) Color.White else Color.Black`.
- `RippleAlpha(draggedAlpha = .1f, focusedAlpha = .1f, hoveredAlpha = .1f, pressedAlpha = .1f)` (constants `RIPPLE_*_ALPHA`).
- Used once: `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt` line 318 (`LocalRippleConfiguration provides playerRippleConfiguration`).

---

## 2. AppTheme and preference resolution (OBSERVED)

### 2.1 `AppTheme` enum
Source: `app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt`. Declaration order is significant (it is the picker order):

| Index | Constant | Title key | Scheme object |
|---|---|---|---|
| 0 | DEFAULT | `MR.strings.label_default` | TachiyomiColorScheme |
| 1 | MONET | `MR.strings.theme_monet` ("Dynamic") | MonetColorScheme |
| 2 | CLOUDFLARE | `AYMR.strings.theme_cloudflare` | CloudflareColorScheme |
| 3 | COTTONCANDY | `AYMR.strings.theme_cottoncandy` | CottoncandyColorScheme |
| 4 | DOOM | `AYMR.strings.theme_doom` | DoomColorScheme |
| 5 | GREEN_APPLE | `MR.strings.theme_greenapple` | GreenAppleColorScheme |
| 6 | LAVENDER | `MR.strings.theme_lavender` | LavenderColorScheme |
| 7 | MATRIX | `AYMR.strings.theme_matrix` | MatrixColorScheme |
| 8 | MIDNIGHT_DUSK | `MR.strings.theme_midnightdusk` | MidnightDuskColorScheme |
| 9 | MOCHA | `AYMR.strings.theme_mocha` | MochaColorScheme |
| 10 | SAPPHIRE | `AYMR.strings.theme_sapphire` | SapphireColorScheme |
| 11 | NORD | `MR.strings.theme_nord` | NordColorScheme |
| 12 | STRAWBERRY_DAIQUIRI | `MR.strings.theme_strawberrydaiquiri` | StrawberryColorScheme |
| 13 | TAKO | `MR.strings.theme_tako` | TakoColorScheme |
| 14 | TEALTURQUOISE | `MR.strings.theme_tealturquoise` | TealTurqoiseColorScheme |
| 15 | TIDAL_WAVE | `MR.strings.theme_tidalwave` | TidalWaveColorScheme |
| 16 | YINYANG | `MR.strings.theme_yinyang` | YinYangColorScheme |
| 17 | YOTSUBA | `MR.strings.theme_yotsuba` | YotsubaColorScheme |
| 18 | MONOCHROME | `MR.strings.theme_monochrome` | MonochromeColorScheme |
| 19 | DARK_BLUE | `null` (deprecated) | none (falls back) |
| 20 | HOT_PINK | `null` (deprecated) | none (falls back) |
| 21 | BLUE | `null` (deprecated) | none (falls back) |

- 19 selectable schemes (indices 0 to 18) and 3 deprecated entries. The 19 are: Default, Dynamic, Cloudflare, Cotton Candy, Doom, Green Apple, Lavender, Matrix, Midnight Dusk, Mocha, Sapphire, Nord, Strawberry Daiquiri, Tako, Teal & Turquoise, Tidal Wave, Yin & Yang, Yotsuba, Monochrome.
- `titleRes` is nullable: the three deprecated entries have no title. The picker uses `appTheme.titleRes!!` (`AppThemePreferenceWidget.kt` line 116), so deprecated entries must never reach the picker.
- Strings: `theme_cloudflare` = "Cloudflare", `theme_cottoncandy` = "Cotton Candy", `theme_doom` = "Doom", `theme_matrix` = "Matrix", `theme_mocha` = "Mocha", `theme_sapphire` = "Sapphire" (`i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`); `theme_monet` = "Dynamic", `label_default` = "Default", `theme_greenapple` = "Green Apple", `theme_lavender` = "Lavender", `theme_midnightdusk` = "Midnight Dusk", `theme_monochrome` = "Monochrome", `theme_nord` = "Nord", `theme_strawberrydaiquiri` = "Strawberry Daiquiri", `theme_tako` = "Tako", `theme_tealturquoise` = "Teal &amp; Turquoise", `theme_yinyang` = "Yin &amp; Yang", `theme_yotsuba` = "Yotsuba", `theme_tidalwave` = "Tidal Wave" (`i18n/src/commonMain/moko-resources/base/strings.xml`).

### 2.2 Stored preference values
Source: `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt` lines 20 to 31.
- `themeMode()` = `getEnum("pref_theme_mode_key", ThemeMode.SYSTEM)`.
- `appTheme()` = `getEnum("pref_app_theme", if (DeviceUtil.isDynamicColorAvailable) AppTheme.MONET else AppTheme.DEFAULT)`.
- `themeDarkAmoled()` = `getBoolean("pref_theme_dark_amoled_key", false)`.
- `getEnum` (`core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt` lines 27 to 40) stores `enum.name` and deserializes with `enumValueOf`. An unknown name (for example a persisted `DARK_BLUE` after an update) returns the **default value passed to getEnum** (the device-dependent default above), not `DEFAULT`.

### 2.3 Theme mode to night mode
Source: `app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt`.
- `enum class ThemeMode { LIGHT, DARK, SYSTEM }`.
- `setAppCompatDelegateThemeMode(mode)`: LIGHT to `MODE_NIGHT_NO`, DARK to `MODE_NIGHT_YES`, SYSTEM to `MODE_NIGHT_FOLLOW_SYSTEM`. Called from `SettingsAppearanceScreen.kt` (`onItemClick`) right after the preference is set.
- Pure-black switch: `enabled = themeMode != ThemeMode.LIGHT`. Its `onValueChanged` calls `ActivityCompat.recreate(activity)` and returns `true`.
- Strings: `theme_system` = "System", `theme_light` = "Light", `theme_dark` = "Dark", `pref_dark_theme_pure_black` = "Pure black dark mode".

### 2.4 View-system theme map (for completeness)
Source: `app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/ThemingDelegate.kt`.
- `getThemeResIds(appTheme, isAmoled)` returns `[themeResources.getOrDefault(appTheme, R.style.Theme_Tachiyomi), ThemeOverlay_Tachiyomi_Amoled (if isAmoled)]`.
- `themeResources` maps every `AppTheme` except DEFAULT (`Theme_Tachiyomi`) and the three deprecated entries to `R.style.Theme_Tachiyomi_<Name>`. These styles read the XML colours of section 5 (through `app/src/main/res/values/themes.xml`).

---

## 3. Surface, elevation and tint (OBSERVED)

Source: `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Surface.kt`.

### 3.1 `Surface(onClick, modifier, onLongClick, enabled = true, shape = RectangleShape, color = surface, contentColor = contentColorFor(color), tonalElevation = 0.dp, shadowElevation = 0.dp, border = null, interactionSource = remember { MutableInteractionSource() }, content)`
Steps, in order:
1. `absoluteElevation = LocalAbsoluteTonalElevation.current + tonalElevation`.
2. Provide `LocalContentColor = contentColor` and `LocalAbsoluteTonalElevation = absoluteElevation`.
3. Outer `Box` modifier chain: `modifier.minimumInteractiveComponentSize()` (M3: at least 48 dp touch target, visual size unchanged), then `.surface(shape, backgroundColor, border, shadowElevation)`, then `.combinedClickable(interactionSource, indication = ripple(), enabled, role = Role.Button, onLongClick, onClick)`.
4. `propagateMinConstraints = true`.
5. `Modifier.surface(shape, backgroundColor, border, shadowElevation)` is `shadow(shadowElevation, shape, clip = false)`, then `border(border, shape)` if non-null, then `background(color = backgroundColor, shape = shape)`, then `clip(shape)`.

### 3.2 Tint formula
`surfaceColorAtElevation(color, elevation)`:
- if `color == MaterialTheme.colorScheme.surface`: `ColorScheme.surfaceColorAtElevation(elevation)`; else `color` unchanged (a custom container colour is never tinted).
- `ColorScheme.surfaceColorAtElevation(elevation)`: if `elevation == 0.dp` return `surface`; else `alpha = ((4.5f * ln(elevation.value + 1)) + 2f) / 100f`; return `surfaceTint.copy(alpha = alpha).compositeOver(surface)`.

Worked values (natural log, `elevation.value` in dp):
- 1 dp: `ln(2) = 0.693147`; `4.5 * 0.693147 = 3.119162`; `+ 2 = 5.119162`; alpha = `0.0512`.
- 3 dp: `ln(4) = 1.386294`; `6.238324 + 2 = 8.238324`; alpha = `0.0824`.
- 6 dp: `ln(7) = 1.945910`; `8.756595 + 2 = 10.756595`; alpha = `0.1076`.
- 0 dp: returns `surface` exactly.

`compositeOver` is standard source-over alpha compositing on the `surface` colour (not premultiplied tint over opaque white).

Rust proposal: the same formula on the `Rgba` tokens; `surfaceTint` is the Compose `surfaceTint` role (for schemes that lack it, see topic 3).

### 3.3 `Modifier.surface`, minimal clicks
- `Surface` with `onClick` always has `Role.Button` semantics. `onLongClick = null` means no long-press handler.
- `enabled = false` disables clicks but keeps layout.

### 3.4 `clickableNoIndication` (Modifier.kt)
`combinedClickable(interactionSource = null, indication = null, onLongClick, onClick)`: no ripple, no press state.

---

## 4. Scaffold layout algorithm (OBSERVED)

Source: `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Scaffold.kt` (361 lines).

### 4.1 Public signature and defaults
`Scaffold(modifier, topBarScrollBehavior = TopAppBarDefaults.pinnedScrollBehavior(rememberTopAppBarState()), topBar = {}, bottomBar = {}, startBar = {}, snackbarHost = {}, floatingActionButton = {}, floatingActionButtonPosition = FabPosition.End, containerColor = MaterialTheme.colorScheme.background, contentColor = contentColorFor(containerColor), contentWindowInsets = ScaffoldDefaults.contentWindowInsets, content: (PaddingValues) -> Unit)`.

- Outer `material3.Surface(modifier = Modifier.nestedScroll(topBarScrollBehavior.nestedScrollConnection).onConsumedWindowInsetsChanged { remainingWindowInsets.insets = contentWindowInsets.exclude(it) }.then(modifier), color = containerColor, contentColor)`.
- `remainingWindowInsets = remember { MutableWindowInsets() }`.

### 4.2 Constants
- `FabSpacing = 16.dp` (private).
- `FabPosition.Center` = value 0; `FabPosition.End` = value 1. Default End. `toString()` returns `"FabPosition.Center"` or `"FabPosition.End"`.
- Subcompose slot order (enum `ScaffoldLayoutContent`): `TopBar, MainContent, Snackbar, Fab, BottomBar, StartBar`.

### 4.3 Measurement (in `SubcomposeLayout`)
Let `layoutWidth = constraints.maxWidth`, `layoutHeight = constraints.maxHeight`, `looseConstraints = constraints.copy(minWidth = 0, minHeight = 0)`, `topBarConstraints = looseConstraints.copy(maxHeight = Infinity)` (top bar may be taller than the screen).

1. `leftInset`, `rightInset`, `bottomInset` from `contentWindowInsets` (`getLeft(this, layoutDirection)`, `getRight`, `getBottom`).
2. `startBar` measured with `looseConstraints`; `startBarWidth = max width` (0 if none).
3. `insetLayoutWidth = layoutWidth - leftInset - rightInset - startBarWidth`.
4. `topBar` measured with `topBarConstraints`; `topBarHeight = max height` (0 if none).
5. `snackbar` measured loose; `snackbarHeight`, `snackbarWidth` = max of each.
6. `snackbarLeft = if (snackbar present) (insetLayoutWidth - snackbarWidth) / 2 + leftInset else 0`. Integer division.
7. `fab` measured loose; `fabWidth`, `fabHeight`. `fabPlacement` exists only if `fab` is present and both sizes are non-zero.
   - Position `End`, LTR: `fabLeftOffset = layoutWidth - 16dp - fabWidth - rightInset`.
   - Position `End`, RTL: `fabLeftOffset = 16dp + leftInset`.
   - Position `Center`: `fabLeftOffset = leftInset + (insetLayoutWidth - fabWidth) / 2`.
8. `bottomBar` measured loose; `bottomBarHeight = max height`, or null if 0.
9. `fabOffsetFromBottom = fabPlacement?.let { max(bottomBarHeight ?: 0, bottomInset) + fab.height + 16dp }`.
10. `snackbarOffsetFromBottom = if (snackbarHeight != 0) snackbarHeight + (fabOffsetFromBottom ?: max(bottomBarHeight ?: 0, bottomInset)) else 0`.
11. `MainContent` (the `content` lambda) measured loose with `innerPadding`:
    - `top = if no topBar: insets.top else topBarHeight`;
    - `bottom = if no bottomBar or bottomBarHeight == 0: max(insets.bottom, fabOffsetDp) else max(bottomBarHeight, fabOffsetDp)` (fabOffsetDp = fabOffsetFromBottom or 0 dp);
    - `start = max(insets.start, startBarWidth)`;
    - `end = insets.end`.

### 4.4 Placement and z-order
Placed in this order (later is drawn on top): content at (0,0); startBar at (0,0) relative; topBar at (0,0); snackbar at `(snackbarLeft, layoutHeight - snackbarOffsetFromBottom)`; bottomBar at `(0, layoutHeight - bottomBarHeight)`; FAB at `(fabPlacement.left, layoutHeight - fabOffsetFromBottom)`.

Consequence: the FAB sits above the bottom bar, separated by 16 dp; the content bottom padding already includes the FAB height plus 16 dp so the last row can scroll clear of it.

### 4.5 Known gaps for the Rust version
- The Rust overlay has no insets (no system bars). The insets terms become 0.
- Only `End` and `Center` FAB positions exist.
- The consumed-insets logic is not needed.

---

## 5. Navigation rail and bar (OBSERVED)

### 5.1 `NavigationRail(modifier, containerColor = NavigationRailDefaults.ContainerColor, contentColor = contentColorFor(containerColor), header: (ColumnScope.() -> Unit)? = null, windowInsets = NavigationRailDefaults.windowInsets, content: ColumnScope.() -> Unit)`
Source: `NavigationRail.kt`.
- `material3.Surface(color = containerColor, contentColor, modifier, tonalElevation = 3.dp)`.
- Inside: `Column(Modifier.fillMaxHeight().windowInsetsPadding(windowInsets).widthIn(min = 80.dp).padding(vertical = MaterialTheme.padding.extraSmall /* 4 dp */).selectableGroup(), horizontalAlignment = CenterHorizontally, verticalArrangement = Arrangement.spacedBy(4.dp, alignment = CenterVertically))`.
- If `header != null`: `header()` then `Spacer(height = MaterialTheme.padding.small /* 8 dp */)`.
- Then `content()`.
- Minimum width 80 dp; grows if content is wider.

### 5.2 `NavigationBar(modifier, containerColor = NavigationBarDefaults.containerColor, contentColor = contentColorFor(containerColor), tonalElevation = NavigationBarDefaults.Elevation, windowInsets = NavigationBarDefaults.windowInsets, content: RowScope.() -> Unit)`
Source: `NavigationBar.kt`.
- `material3.Surface(color, contentColor, tonalElevation, modifier)`.
- Inside: `Row(Modifier.fillMaxWidth().windowInsetsPadding(windowInsets).height(80.dp).selectableGroup(), content)`.
- Fixed 80 dp height.

### 5.3 Which one the app uses
Neither composable has a call site outside its own file (grep for `NavigationBar(` and `NavigationRail(` in `*.kt` returns only the definitions). `Scaffold`'s `startBar` slot is documented for the rail but no screen passes one. Both are therefore dead code in this repo. The desktop shell already has its own sidebar (`src/main.rs::sidebar`, mockup), see task T-49b-017.

---

## 6. Slider (OBSERVED)

Source: `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Slider.kt`.

Signature: `Slider(value: Int, onValueChange: (Int) -> Unit, modifier, enabled = true, valueRange: IntProgression = 0..1, steps: Int = (last - first) - 1, onValueChangeFinished: (() -> Unit)? = null, colors = SliderDefaults.colors(), interactionSource, thumb, track)`.

Algorithm:
1. The M3 slider receives `value.toFloat()`, `valueRange = first.toFloat()..last.toFloat()`, `steps` unchanged.
2. Changes are reported as `onValueChange(it.roundToInt())`. `roundToInt()` is `Math.round` semantics (half rounds up toward positive infinity).
3. `steps` default for `0..10` is `9`. M3 `steps` counts the interior stops, so the slider has 11 discrete positions (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10). Every integer is one stop.
4. `0..1` gives `steps = 0`: continuous float, rounded to 0 or 1 on each change.
5. `onValueChangeFinished` is forwarded unchanged.

Usage: 10 call sites (reader and player settings, `ChapterNavigator.kt`, `SliderItem.kt`, `TintedSliderItem.kt`, `SettingsItems.kt`).

Proposal (Rust): `gtk::Scale` with `set_range(first, last)`, `set_increments(1.0, 1.0)`, `set_round_digits(0)`, `set_draw_value(true)`. Report on `value-changed`. Emit the finished event on the `GestureDrag` end or on button release, to mirror `onValueChangeFinished`.

---

## 7. Pull to refresh (OBSERVED)

Source: `PullRefresh.kt`.
- `PullRefresh(refreshing: Boolean, enabled: Boolean, onRefresh: () -> Unit, modifier = Modifier, indicatorPadding: PaddingValues = PaddingValues(0.dp), content)`.
- `state = rememberPullToRefreshState()`.
- Outer `Box(modifier.pullToRefresh(isRefreshing = refreshing, state, enabled, onRefresh))`.
- Content first, then `PullToRefreshDefaults.Indicator(modifier = Modifier.align(TopCenter).padding(indicatorPadding), isRefreshing = refreshing, state, containerColor = surfaceVariant, color = onSurfaceVariant)`.
- `enabled = false` stops the gesture. `refreshing` controls the indicator spinner. The gesture completes at the M3 threshold (framework; not set in this file).
- Note: the component is referenced 9 times by app screens (`PullRefresh(` call count 9).

Proposal: no gesture on desktop. A header refresh button and F5 (Ctrl+R on the same action) call `onRefresh`. When `refreshing` is true, the button is disabled and shows a spinner. `indicatorPadding` maps to the spinner's margin.

---

## 8. Tab text with count pill (OBSERVED)

Source: `Tabs.kt`.
- `TabText(text: String, badgeCount: Int? = null)`.
- `pillAlpha = if (isSystemInDarkTheme()) 0.12f else 0.08f`.
- `Row(verticalAlignment = CenterVertically)`:
  - `Text(text, maxLines = 1, overflow = Ellipsis)`;
  - if `badgeCount != null`: `Pill(text = "$badgeCount", color = onBackground.copy(alpha = pillAlpha), fontSize = 10.sp)`.
- `Pill` (`presentation-core/src/main/java/tachiyomi/presentation/core/components/Pill.kt`, component area) is a `Surface` with `shape = extraLarge`, padding start 4 dp, inner padding `6.dp, 1.dp`, `Text` with `maxLines = 1`, font size 10 sp.
- The count is printed with `$badgeCount` (no grouping separators, no cap at 99).

---

## 9. Empty, loading and info screens (OBSERVED)

### 9.1 `EmptyScreen`
Source: `EmptyScreen.kt`.
- Overloads: `EmptyScreen(stringRes: StringResource, modifier, actions: ImmutableList<EmptyScreenAction>? = null)` resolves `stringResource(stringRes)` and calls the `String` overload. `EmptyScreen(message: String, modifier, actions)` is the implementation.
- `face = remember { getRandomErrorFace() }`: one random face from `ErrorFaces`, chosen once per composition (kept across recompositions, not across navigation).
- `ErrorFaces` (12 strings, in order): `"(･o･;)"`, `"Σ(ಠ_ಠ)"`, `"ಥ_ಥ"`, `"(˘･_･˘)"`, `"(；￣Д￣)"`, `"(･Д･。"`, `"(╬ಠ益ಠ)"`, `"(╥﹏╥)"`, `"(⋟﹏⋞)"`, `"Ò︵Ó"`, `" ˙ᯅ˙)"`, `"(¬_¬)"`. Selection is `Random.nextInt(12)` (uniform). Note the 6th string `"(･Д･。"` has no closing bracket and the 11th string `" ˙ᯅ˙)"` starts with a space; these are copied verbatim.
- Layout: `Column(modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = 24.dp), horizontalAlignment = CenterHorizontally, verticalArrangement = Center)`.
  - Face: forced `LayoutDirection.Ltr`; `Text(face, modifier = secondaryItemAlpha(), style = displayMedium)`.
  - Message: `Text(message, modifier = Modifier.paddingFromBaseline(top = 24.dp).secondaryItemAlpha(), style = bodyMedium, textAlign = Center)`.
  - Actions only if `!actions.isNullOrEmpty()`: `Row(Modifier.padding(top = 24.dp), horizontalArrangement = spacedBy(8.dp))`; each action is `ActionButton(Modifier.weight(1f), title = stringResource(it.stringRes), icon = it.icon, onClick = it.onClick)`, so actions split the width equally.
- `EmptyScreenAction(stringRes: StringResource, icon: ImageVector, onClick: () -> Unit)` data class.
- Scroll: the whole column scrolls if content is taller than the screen (verticalScroll).
- Usage: 40 call sites.

### 9.2 `InfoScreen`
Source: `InfoScreen.kt`.
- Signature: `InfoScreen(icon: ImageVector, headingText: String, subtitleText: String, acceptText: String, onAcceptClick: () -> Unit, canAccept: Boolean = true, rejectText: String? = null, onRejectClick: (() -> Unit)? = null, content: ColumnScope.() -> Unit)`.
- Bottom bar slot of `Scaffold` (`bottomBar` lambda):
  - bottom bar `Column`: background `background`; a top hairline `drawLine` in `outline` colour, width `Dp.Hairline` (drawn at y=0 across the full width); `windowInsetsPadding(NavigationBarDefaults.windowInsets)`; padding horizontal 16 dp (`medium`), vertical 8 dp (`small`).
  - Accept button: `Button(fillMaxWidth, enabled = canAccept, onClick = onAcceptClick)` with `Text(acceptText)`.
  - Reject button: only when `rejectText != null && onRejectClick != null`: `OutlinedButton(fillMaxWidth, onClick = onRejectClick)`.
- Body (`paddingValues` from Scaffold):
  - Status-bar scrim: `Box(Modifier.zIndex(2f).secondaryItemAlpha().background(background).fillMaxWidth().height(paddingValues.calculateTopPadding()))`.
  - Scrollable column: `verticalScroll(rememberScrollState()).fillMaxWidth().padding(paddingValues).padding(top = 48.dp).padding(horizontal = 16.dp)`.
  - Icon: `Icon(icon, contentDescription = null, Modifier.padding(bottom = 8.dp).size(48.dp), tint = primary)`.
  - Heading: `Text(headingText, style = headlineLarge)`.
  - Subtitle: `Text(subtitleText, Modifier.secondaryItemAlpha().padding(vertical = 8.dp), style = titleSmall)`.
  - Then `content()`.
- Usage: 6 call sites.

### 9.3 `LoadingScreen`
Source: `LoadingScreen.kt`. `Box(modifier.fillMaxSize(), contentAlignment = Center) { CircularProgressIndicator() }` (M3 default indeterminate). Usage: 54 call sites. No text, no timeout.

---

## 10. Modifier helpers (OBSERVED)

Source: `Modifier.kt`.

### 10.1 `selectedBackground(isSelected: Boolean)`
- If false: returns `this` (no-op).
- If true: `composed { alpha = if (isSystemInDarkTheme()) 0.16f else 0.22f; color = colorScheme.secondary.copy(alpha); Modifier.drawBehind { drawRect(color) } }`.
- Draws over the full bounds, under the content (drawBehind).

### 10.2 `secondaryItemAlpha()`
`alpha(SECONDARY_ALPHA)`, `SECONDARY_ALPHA = .78f` (`components/material/Constants.kt`).

### 10.3 `runOnEnterKeyPressed(action)`
`onPreviewKeyEvent { when (it.key) { Key.Enter, Key.NumPadEnter -> { action(); true }; else -> false } }`.
- Preview phase: runs before the focused child. Returns `true` so the key is consumed.
- Only physical Enter and keypad Enter. Other keys are not consumed.
- Documented as "Naturally, the TextField should be set to single line only."

### 10.4 `showSoftKeyboard(show: Boolean)`
- If `show == false`: no-op.
- If `show == true`: `composed { focusRequester = remember{}; openKeyboard by rememberSaveable { mutableStateOf(show) }; LaunchedEffect(focusRequester) { if (openKeyboard) { focusRequester.requestFocus(); openKeyboard = false } }; Modifier.focusRequester(focusRequester) }`.
- Effect: the element takes focus once per saved state; it survives configuration change (rememberSaveable) but a second composition does not re-request focus.

### 10.5 `clearFocusOnSoftKeyboardHide(onFocusCleared: (() -> Unit)? = null)`
- `isFocused` set by `onFocusChanged`. On focus gain, `keyboardShowedSinceFocused = false`.
- While focused, `LaunchedEffect(imeVisible)` with `imeVisible = WindowInsets.isImeVisible`: when IME becomes visible, `keyboardShowedSinceFocused = true`; when it becomes hidden after having been shown, call `focusManager.clearFocus()` and `onFocusCleared?.invoke()`.
- Desktop: no soft keyboard. The helper is dropped (task T-49b-032).

### 10.6 `clickableNoIndication(onLongClick, onClick)`
See 3.4.

---

## 11. Preference observation (OBSERVED)

Source: `presentation-core/src/main/java/tachiyomi/presentation/core/util/Preference.kt`.
- `@Composable fun <T> Preference<T>.collectAsState(): State<T>`:
  - `flow = remember(this) { changes() }` (recreated only if the preference object identity changes);
  - `return flow.collectAsState(initial = get())`.
- `Preference<T>` interface (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt`): `key()`, `get()`, `set(value)`, `isSet()`, `delete()`, `defaultValue()`, `changes(): Flow<T>`, `stateIn(scope)`.
- Emits the current value first (StateFlow semantics of `changes()`), then each change.
- Called from about 51 files in the app (grep for `collectAsState()` in files that use `Preference`).

Proposal: `PrefCell<T>` holds the value; `changes` pushes to a `glib::MainContext` channel; widgets subscribe with `glib::spawn_future_local` (single-threaded UI).

---

## 12. Scrollbar (OBSERVED, dropped on desktop)

Source: `presentation-core/src/main/java/tachiyomi/presentation/core/util/Scrollbar.kt`. Derived from an MIT gist (Albert Chang, 2022), modified for contentPadding.

Algorithm for vertical (horizontal analogous):
1. `viewportSize = layoutInfo.viewportSize.height - beforeContentPadding - afterContentPadding`.
2. `items = visibleItemsInfo`; `itemsSize = sum(item.size)`.
3. `showScrollbar = items.size < totalItemsCount || itemsSize > viewportSize`.
4. `estimatedItemSize = if (items empty) 0 else itemsSize / items.size`.
5. `totalSize = estimatedItemSize * totalItemsCount`.
6. `thumbSize = viewportSize / totalSize * viewportSize`. **Edge:** if `totalSize == 0` (empty list), the division yields infinity.
7. `startOffset`: if no items, 0; else first visible item whose key does not start with `STICKY_HEADER_KEY_PREFIX` (`"sticky:"`, defined in `VerticalFastScroller.kt` line 437). `startOffset = startPadding + ((estimatedItemSize * index - offset) / totalSize * viewportSize)`. `startPadding` = `beforeContentPadding`, or `afterContentPadding` when reversed.
8. Draws a rect of `thickness x thumbSize` at `(atEnd ? width - positionOffset - thickness : positionOffset, scrollOffset)` only if `showScrollbar`.
9. Colour: `onSurface.copy(alpha = 0.364f)`.
10. `thickness = ViewConfiguration.get(context).scaledScrollBarSize` **(framework)**.
11. Fade: a `MutableSharedFlow<Unit>(extraBufferCapacity = 1, onBufferOverflow = DROP_OLDEST)` receives a tick from `onPostScroll` when the scroll delta on the axis is non-zero. `scrolled.sample(100)` (ms **(framework: Kotlin Flow sample of 100 ms)**), then `collectLatest { alpha.snapTo(1f); alpha.animateTo(0f, tween(durationMillis = ViewConfiguration.getScrollBarFadeDuration(), delayMillis = ViewConfiguration.getScrollDefaultDelay())) }`. Framework defaults: fade 250 ms, delay 300 ms **(framework)**.
12. Direction: `isLtr`; horizontal reverse unless LTR; vertical `atEnd = isLtr` (thumb on the right in LTR).

Desktop replacement: GTK4 overlay scrollbars. `gtk::ScrolledWindow::set_overlay_scrolling(true)`; GTK fades them out on its own. The user keeps native overlay scrollbars and loses the sticky-header-aware thumb (the Compose thumb skipped sticky headers when computing the start offset). Task T-49b-036.

Preview functions (`LazyListScrollbarPreview`, `LazyListHorizontalScrollbarPreview`) are dev-only.

---

## 13. Elevation animation specs (OBSERVED, ported as CSS)

Source: `presentation-core/src/main/java/tachiyomi/presentation/core/util/Elevation.kt`. Copy of the M3 `ElevationDefaults` (Apache 2.0 header "Copyright 2021 The Android Open Source Project").

`animateElevation(target, from, to)`:
- `to != null` (moving to a new state): spec = `incomingAnimationSpecForInteraction(to)`.
  - PressInteraction.Press, DragInteraction.Start, HoverInteraction.Enter, FocusInteraction.Focus: `DefaultIncomingSpec = TweenSpec(durationMillis = 120, easing = FastOutSlowInEasing)`.
  - Other: null (snap).
- `to == null && from != null` (returning to rest): spec = `outgoingAnimationSpecForInteraction(from)`.
  - Press, Drag.Start, Focus.Focus: `DefaultOutgoingSpec = TweenSpec(durationMillis = 150, easing = OutgoingSpecEasing)`.
  - HoverInteraction.Enter: `HoveredOutgoingSpec = TweenSpec(durationMillis = 120, easing = OutgoingSpecEasing)`.
  - Other: null (snap).
- Neither: snap.
- `OutgoingSpecEasing = CubicBezierEasing(0.40f, 0.00f, 0.60f, 1.00f)`. `FastOutSlowInEasing` is the Compose standard curve (cubic-bezier(0.4, 0, 0.2, 1)) **(framework)**.

Proposal: the Rust CSS uses `transition: box-shadow 120ms cubic-bezier(0.4,0,0.2,1)` on entry, `150ms cubic-bezier(0.4,0,0.6,1)` on exit, `120ms` exit for hover. See topic 2.

---

## 14. Typography and colour helpers (OBSERVED)

### 14.1 `Typography.header`
Source: `presentation-core/src/main/java/tachiyomi/presentation/core/theme/Typography.kt`. `bodyMedium.copy(color = onSurfaceVariant, fontWeight = FontWeight.SemiBold)`. Used 8 times.

### 14.2 `ColorScheme.active`
Source: `presentation-core/src/main/java/tachiyomi/presentation/core/theme/Color.kt`.
- Composable getter. `if (isSystemInDarkTheme()) Color(255, 235, 59) else Color(255, 193, 7)`.
- Values: dark `#FFEB3B` (Material yellow 500), light `#FFC107` (Material amber 500).
- It follows the **system** dark flag as the code reads it (same as `isSystemInDarkTheme()` in section 1.2), so it matches the effective theme mode after AppCompat night mode.
- Used for an active filter tint: `EntryToolbar.kt` line 80 (`if (hasFilters) active else LocalContentColor`), `ChapterSettingsDialog.kt` line 182, `LibraryToolbar.kt`.

---

## 15. Localization helpers (OBSERVED)

Source: `presentation-core/src/main/java/tachiyomi/presentation/core/i18n/Localize.kt`, and `core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt`.
- `stringResource(resource: StringResource): String = LocalContext.current.stringResource(resource)` (`@ReadOnlyComposable`).
- `stringResource(resource, vararg args: Any)` formats with `StringDesc.ResourceFormatted(resource, *args)`.
- `pluralStringResource(resource: PluralsResource, count: Int)` uses `StringDesc.Plural(resource, count)`; the vararg overload uses `StringDesc.PluralFormatted(resource, count, *args)`.
- Every result passes through `private fun String.fixed() = this.replace("\\", "\"")` (`core/common/.../Localize.kt`, line with `replace("""\""", """"""")`). The replacement is **a single backslash replaced by a single double-quote character**, and it runs on every string. This is a workaround for moko-resources issue 337 (comment "janky workaround"). Edge: any genuine backslash in a translated string turns into `"`.
- Keys come from `tachiyomi.i18n.MR` and `tachiyomi.i18n.aniyomi.AYMR` (MOKO generated).

Proposal: see T-49b-023 (gettext) and `p2-rust-design.md` section 6. The backslash rule is reproduced only if a catalog really contains backslash escapes; the Rust port should not replicate it blindly.

---

## 16. Icons (OBSERVED)

Source: `presentation-core/src/main/java/tachiyomi/presentation/core/icons/*.kt`.
- `object CustomIcons` is a namespace with KDoc "Icons imported from https://simpleicons.org using https://github.com/DevSrSouza/svg-to-compose".
- Each icon is `val CustomIcons.<Name>: ImageVector` with a lazily built cache field `private var _<name>: ImageVector?` (not thread-safe; set once on the main thread).
- Build: `ImageVector.Builder(name, defaultWidth = 24.0.dp, defaultHeight = 24.0.dp, viewportWidth = 24.0f, viewportHeight = 24.0f)`; one `path(fill = SolidColor(Color(0xFF000000)), stroke = null, strokeLineWidth = 0.0f, strokeLineCap = Butt, strokeLineJoin = Miter, strokeLineMiter = 4.0f, pathFillType = NonZero)` per icon (`Github.kt` and `Discord.kt`), or `path(fill = SolidColor(...))` with defaults (`Magnet.kt`).
- Shapes: Discord: `moveTo` x3, `curveToRelative` x21, `arcToRelative` x17 (1 path). GitHub: `moveTo` x1, `curveToRelative` x25 (1 path). Magnet: `moveTo` x3, `arcToRelative` x7 (1 path).
- Black fill is replaced by the tint at the use site (the `Icon` `tint` parameter).
- Licences: Magnet is MIT, "Made by Neuicons: https://github.com/neuicons/neu" (comment in `Magnet.kt` line 9). Discord and GitHub are brand marks from simpleicons (trademark caveat: use only for the links they represent).
- Usage: Discord 3, Github 2, Magnet 3 call sites.

---

## 17. Thumbnails of the drawing resources (OBSERVED, colour usage)

- `ThemePreviews` (`ThemePreviews.kt`): annotation combining `@Preview(name = "Light", showBackground = true)` and `@Preview(name = "Dark", showBackground = true, uiMode = Configuration.UI_MODE_NIGHT_YES)`. Dev-only.
- `InfoScaffoldPreview`: a dev preview with `@PreviewLightDark` (`InfoScreen.kt`).
- `LazyListScrollbarPreview` (400x400, `showBackground`) and `LazyListHorizontalScrollbarPreview` (width 400): dev only.

---

## 18. Interaction timings and limits (consolidated)

| Name | Value | Source |
|---|---|---|
| Elevation incoming tween | 120 ms, FastOutSlowIn | Elevation.kt line 112 to 115 |
| Elevation outgoing tween | 150 ms, cubic-bezier(0.40,0,0.60,1) | Elevation.kt line 117 to 120 |
| Hover outgoing tween | 120 ms, same curve | Elevation.kt line 122 to 125 |
| Scrollbar sample | 100 ms | Scrollbar.kt line 216 |
| Scrollbar fade | ViewConfiguration.getScrollBarFadeDuration() (framework default 250 ms) | Scrollbar.kt line 241 to 244 |
| Scrollbar delay | ViewConfiguration.getScrollDefaultDelay() (framework default 300 ms) | Scrollbar.kt line 241 to 244 |
| Ripple alpha (player) | 0.1 for dragged, focused, hovered, pressed | TachiyomiTheme.kt line 89 to 91 (and 88) |
| Selection tint | 0.16 dark, 0.22 light | Modifier.kt line 29 |
| Secondary alpha | 0.78 | Constants.kt line 10 |
| Disabled alpha | 0.38 | Constants.kt line 9 |
| Scrollbar colour alpha | 0.364 | Scrollbar.kt line 233 |
| Pill tint alpha | 0.12 dark, 0.08 light | Tabs.kt line 18 |
| Pill font | 10 sp | Tabs.kt line 32 |
| FAB spacing | 16 dp | Scaffold.kt line 359 |
| Nav rail min width | 80 dp; tonal elevation 3 dp | NavigationRail.kt line 41, 47 |
| Nav bar height | 80 dp | NavigationBar.kt line 43 |
| Empty screen padding | horizontal 24 dp; face to message 24 dp; actions 24 dp top, 8 dp gap | EmptyScreen.kt line 62, 77, 86, 87 |
| Info screen | top 48 dp; horizontal 16 dp; vertical 8 dp; icon 48 dp | InfoScreen.kt lines 63 to 109 |
| Pull-refresh | none (M3 defaults) | PullRefresh.kt |
| Empty face count | 12 | EmptyScreen.kt line 102 to 115 |
| Window size breakpoints | medium > 600 dp, expanded > 840 dp (not in scope; WindowSize.kt) | mihon/core/designsystem/utils/WindowSize.kt lines 22 to 23 |
| Surface min touch size | 48 dp (M3 minimumInteractiveComponentSize) | Surface.kt line 61 |

---

## 19. Interactions with other areas

- area:02-preferences-system: `Preference<T>`, `getEnum`, `getBoolean`, `changes()`. Defaults in section 2.2.
- area:36-ui-shell-navigation-security-webview: the sidebar and navigation shell where the rail equivalent lives.
- area:37-ui-library, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime, area:40-ui-browse-manga, area:41-ui-browse-anime, area:42-ui-updates-history-stats-storage-upcoming: call `EmptyScreen`, `LoadingScreen`, `PullRefresh`, `TabText`, `ColorScheme.active`, `Typography.header`, `selectedBackground`.
- area:43-reader-core, area:44-reader-viewers, area:45-player-core, area:46-player-controls-ui: call `Slider`, `playerRippleConfiguration`, `Scaffold`, `Surface`.
- area:47-settings-screens-general and area:48-settings-player-and-scripts: call `InfoScreen`, `collectAsState`, theme picker (`AppThemePreferenceWidget.kt`, `AppThemeModePreferenceWidget.kt`) and the appearance screen (`SettingsAppearanceScreen.kt`).
- area:34-images-covers-cache: `cover_placeholder` colour (`#1F888888`) is used by `cover_error.xml`.
- area:35-data-misc-updater-notification-export: `accent_blue` is the notification colour (`NotificationExtensions.kt` line 71).
- Part 1 of this area (`p1-*.md`): the Compose scheme objects and `TachiyomiTheme`.

