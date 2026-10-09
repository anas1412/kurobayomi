# Area 49b (part 1): behavior spec for Android resources (values, drawables, colour selector)

All paths are repo-relative to `/home/blackbox/aniyomi-src`. Line references are `path:line`. Sections are per feature. "OBSERVED" = what the Kotlin/XML does. "PROPOSAL" = what Kurobayomi should do.

Resources have no runtime behavior of their own. Their behavior is the behavior of the code that resolves them. The sections below therefore describe both the resource and the resolving code.

---

## 1. Theme selection (which style is applied to an Activity)

### 1.1 OBSERVED: preference inputs

- `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:20-31`
  - `appTheme()` = `preferenceStore.getEnum("pref_app_theme", default)` where `default` = `AppTheme.MONET` if `DeviceUtil.isDynamicColorAvailable`, else `AppTheme.DEFAULT`.
  - `themeDarkAmoled()` = `preferenceStore.getBoolean("pref_theme_dark_amoled_key", false)`.
  - `themeMode()` = `preferenceStore.getEnum("pref_theme_mode_key", ThemeMode.SYSTEM)` (`UiPreferences.kt:20`; used only by the reader context, see 1.5).
- `app/src/main/java/eu/kanade/util/system/DeviceUtilExtensions.kt`: `isDynamicColorAvailable = DynamicColors.isDynamicColorAvailable() || (DeviceUtil.isSamsung && SDK_INT >= 31)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt:6-8`).
- `minSdk` = 26, `targetSdk` = 36, `compileSdk` = 36 (`buildSrc/src/main/kotlin/mihon/buildlogic/AndroidConfig.kt:7-9`). Consequence: the `values/` branch (API < 27) is reachable on API 26 only.

### 1.2 OBSERVED: style resolution

`app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/ThemingDelegate.kt`:

1. `getThemeResIds(appTheme, isAmoled)` returns a list of at most 2 style ids (lines 14-21):
   - `themeResources.getOrDefault(appTheme, R.style.Theme_Tachiyomi)`
   - then `R.style.ThemeOverlay_Tachiyomi_Amoled` only if `isAmoled` is true.
2. `ThemingDelegateImpl.applyAppTheme(activity)` reads the two prefs and calls `activity.setTheme(id)` for each id in order (lines 23-33). The overlay is applied second, so its items win.
3. `themeResources` (lines 34-52) maps 18 `AppTheme` constants to styles:

| AppTheme constant | Style |
|---|---|
| MONET | Theme_Tachiyomi_Monet |
| COTTONCANDY | Theme_Tachiyomi_CottonCandy |
| GREEN_APPLE | Theme_Tachiyomi_GreenApple |
| LAVENDER | Theme_Tachiyomi_Lavender |
| MIDNIGHT_DUSK | Theme_Tachiyomi_MidnightDusk |
| MONOCHROME | Theme_Tachiyomi_Monochrome |
| MOCHA | Theme_Tachiyomi_Mocha |
| NORD | Theme_Tachiyomi_Nord |
| STRAWBERRY_DAIQUIRI | Theme_Tachiyomi_StrawberryDaiquiri |
| TAKO | Theme_Tachiyomi_Tako |
| TEALTURQUOISE | Theme_Tachiyomi_TealTurquoise |
| YINYANG | Theme_Tachiyomi_YinYang |
| YOTSUBA | Theme_Tachiyomi_Yotsuba |
| CLOUDFLARE | Theme_Tachiyomi_Cloudflare |
| SAPPHIRE | Theme_Tachiyomi_Sapphire |
| DOOM | Theme_Tachiyomi_Doom |
| MATRIX | Theme_Tachiyomi_Matrix |
| TIDAL_WAVE | Theme_Tachiyomi_TidalWave |

4. Not in the map, so they fall back to `Theme_Tachiyomi`: `DEFAULT`, and the deprecated constants `DARK_BLUE`, `HOT_PINK`, `BLUE` (`app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt:7-30`, `titleRes = null`).
5. Base style for every activity: `android:theme="@style/Theme.Tachiyomi"` (`app/src/main/AndroidManifest.xml:50`, `:214`, `:225`) and `@style/Theme.Tachiyomi.SplashScreen` for the launch activity (`:57`).

### 1.3 OBSERVED: Compose path (does not use the XML themes)

`app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt:37-72`:
- `TachiyomiTheme(appTheme, amoled)` reads `appTheme` and `themeDarkAmoled` (defaults from prefs).
- If `appTheme == MONET`: `MonetColorScheme(LocalContext.current)`; else `colorSchemes.getOrDefault(appTheme, TachiyomiColorScheme)`.
- The result `getColorScheme(isSystemInDarkTheme(), isAmoled)` is passed to `MaterialTheme`.

Consequence: almost all visible UI is Compose and reads the Kotlin `ColorScheme` objects, not `values/colors_*.xml`. The XML themes drive View-based surfaces (legacy preferences, dialogs made with `ContextThemeWrapper`, the splash screen, and notifications). Kurobayomi needs the Kotlin tables as its reference for colour values (see `plans/49-theme-and-design-system/p1-topic-color-token-tables.md`), and this area's XML tables only as a second source.

### 1.4 OBSERVED: AMOLED overlay

`ThemeOverlay.Tachiyomi.Amoled` (`app/src/main/res/values-night/themes.xml:5-14`) sets, in night mode only (the `values-night` qualifier):
- `android:colorBackground` = `@color/amoled_background` = `#000000`
- `colorOnBackground` = `@color/amoled_onBackground` = `#FFFFFF`
- `colorSurface` = `@color/amoled_surface` = `#000000`
- `colorOnSurface` = `@color/amoled_onSurface` = `#FFFFFF`
- `colorSurfaceContainer` = `@color/amoled_surfaceContainer` = `#0C0C0C`
- `colorSurfaceContainerHigh` = `@color/amoled_surfaceContainerHigh` = `#131313`
- `colorSurfaceContainerHighest` = `@color/amoled_surfaceContainerHighest` = `#1B1B1B`

The Compose equivalent is `BaseColorScheme.getColorScheme` (`app/src/main/java/eu/kanade/presentation/theme/colorscheme/BaseColorScheme.kt:17-34`): when `isDark && isAmoled`, it copies the dark scheme with `background = Color.Black`, `onBackground = Color.White`, `surface = Color.Black`, `onSurface = Color.White`, `surfaceVariant = #0C0C0C`, `surfaceContainerLowest = #0C0C0C`, `surfaceContainerLow = #0C0C0C`, `surfaceContainer = #0C0C0C`, `surfaceContainerHigh = #131313`, `surfaceContainerHighest = #1B1B1B`. When not dark, the AMOLED flag has no effect (`if (!isDark) return lightScheme`).

PROPOSAL (Kurobayomi): apply the same values as a CSS overlay class `.kb-amoled` only when the active style is dark. Light mode ignores the flag, exactly as the Kotlin does.

### 1.5 OBSERVED: reader-specific night context

`app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:116-143` (`Context.createReaderThemeContext()`):
- `themeMode` = `UiPreferences.themeMode()`.
- `isDarkBackground` depends on `ReaderPreferences.readerTheme()`:
  - `1, 2` (Black, Gray) = true
  - `3` (automatic background) = `themeMode == SYSTEM` ? `applicationContext.isNightMode()` : `themeMode == DARK`
  - otherwise (White) = false
- `expected` uiMode = `UI_MODE_NIGHT_YES` or `UI_MODE_NIGHT_NO` accordingly.
- If the current configuration's night bit differs from `expected`, build a `ContextThemeWrapper(this, R.style.Theme_Tachiyomi)`, override the configuration's night bits, then apply each style from `ThemingDelegate.getThemeResIds(appTheme, amoled)` with `applyStyle(id, true)`. Otherwise return `this`.

PROPOSAL: the reader's background theme (white, sepia, gray, black) should pick a GTK CSS class on the reader surface (`.kb-reader-bg-white`, `-gray`, `-black`, `-auto`) rather than swapping contexts. The four-way mapping of `readerTheme` values 0-3 is the one defined above.

### 1.6 OBSERVED: qualifier precedence (values, night, API level)

Android resolves a resource by qualifiers, in this precedence for the qualifiers present in this repo: smallest width (`sw720dp`) > UI mode (`night`) > API version (`v27`, `v31`). Concretely for a device on API 31+ in dark mode:
- `bools.xml`: `values-night/bools.xml` defines all three keys, so `values-night` wins for `lightStatusBar`, `lightNavigationBar`, `elevationOverlayEnabled`. `values-v27/bools.xml` never applies to those keys in dark mode.
- `themes.xml`: `values-night-v31/themes.xml` provides `Theme.Tachiyomi.Monet` dark colours. `values-night/themes.xml` provides `ThemeOverlay.Tachiyomi.Amoled` only.
- `dimens.xml`: `values-sw720dp/dimens.xml` overrides `screen_edge_margin` to 24dp on devices with smallest width of at least 720dp (tablets). Phones use 16dp.

### 1.7 OBSERVED: base theme items (values/themes.xml 9-87)

`Base.Theme.Tachiyomi` parent `Theme.Material3.DayNight.NoActionBar`. Items that change behavior (name = value):
- `android:forceDarkAllowed` = false (line 11)
- Palette: `colorPrimary`=`@color/tachiyomi_primary` ... through `colorSurfaceContainerLowest` (lines 14-46). Full token list in section 6.
- `android:divider` = `@color/divider_default`; `android:listDivider` = `@drawable/line_divider` (lines 48-49).
- `colorControlHighlight` = `?attr/colorSurfaceVariant` (line 53): ripple/press highlight colour.
- `android:textAlignment` = `gravity`; `android:textDirection` = `locale` (lines 56-57): RTL-aware alignment.
- `android:windowLightStatusBar` = `@bool/lightStatusBar`; `android:statusBarColor` = transparent; `android:navigationBarColor` = `@color/amoled_surface` (lines 60-62).
- `android:itemTextAppearance` = `@style/TextAppearance.Widget.Menu`; `materialAlertDialogTheme` and `alertDialogTheme` = `@style/ThemeOverlay.Tachiyomi.MaterialAlertDialog`; `textAppearanceButton` = `@style/TextAppearance.Widget.Button` (lines 66-69).
- `android:backgroundDimAmount` = `0.32` (line 71): dim behind dialogs.
- `actionOverflowButtonStyle` = `@style/Theme.Tachiyomi.ActionButton.Overflow`; `actionModeCloseDrawable` = `@drawable/ic_close_24dp` (lines 73-74).
- `preferenceTheme` = `@style/PreferenceThemeOverlay.Tachiyomi`; `textInputStyle` = `Widget.Material3.TextInputLayout.OutlinedBox`; `appBarLayoutStyle` = `Widget.Material3.AppBarLayout`; `toolbarStyle` = `Widget.Material3.Toolbar.Surface`; `switchStyle` = `@style/Widget.Tachiyomi.Switch`; `materialSwitchStyle` = `Widget.Material3.CompoundButton.MaterialSwitch`; `switchPreferenceCompatStyle` = `@style/Widget.Tachiyomi.Switch`; `materialCardViewStyle` = `Widget.Material3.CardView.Elevated` (lines 76-82).
- `android:textAppearanceListItem` = `?attr/textAppearanceBodyLarge`; `android:textAppearanceListItemSecondary` = `@style/TextAppearance.Tachiyomi.Preference.Secondary` (lines 85-86).

### 1.8 OBSERVED: fallback when a palette does not define a token

Several palette styles define fewer tokens than the base. Each palette style has an implicit parent (the name prefix `Theme.Tachiyomi`, see 1.9), and `Theme.Tachiyomi` in turn inherits from `Base.Theme.Tachiyomi`, which defines the `tachiyomi_*` default palette. So a token a palette omits is NOT left at the Material 3 default; it silently takes the default-palette value (`tachiyomi_*`, blue-based, see `presentation-core/src/main/res/values/colors_tachiyomi.xml`). Omitted tokens per palette (from the item matrix checked against the file, `values/themes.xml`):
- CottonCandy, Mocha, Cloudflare, Sapphire, Doom, Matrix: omit `colorSurfaceContainer`, `colorSurfaceContainerLowest`, `colorSurfaceBright`, `colorSurfaceDim`, `colorOutlineVariant`, `colorError`, `scrimBackground`, `elevationOverlayColor`. Note the base also has no `colorSurfaceBright`/`colorSurfaceDim`/`scrimBackground` items of its own; those resolve to the Material 3 parent.
- Monochrome: omits `scrimBackground`, `elevationOverlayColor`.
- Nord: omits `colorSurfaceBright`, `colorSurfaceDim`, `colorError`, `scrimBackground`.
- MidnightDusk, Tako, TealTurquoise, YinYang, Yotsuba, TidalWave: omit `colorSurfaceBright`, `colorSurfaceDim`, `colorOutlineVariant`, `colorError`, `scrimBackground`.
- GreenApple, Lavender, StrawberryDaiquiri: omit only `elevationOverlayColor`.
- Monet: no items in `values/themes.xml`; items come only from the v31 files (section 1.9).

Effect: on CottonCandy and Mocha the error colour is the default-palette `tachiyomi_error` (`#BA1A1A`), and the container surfaces are the default-palette values, so the palette is not self-consistent. This is recorded as a finding in p1-edge-cases-and-tests.md.

### 1.9 OBSERVED: implicit parents

Style items with no `parent` attribute inherit from the name prefix (`Theme.Tachiyomi.Mocha` gets `Theme.Tachiyomi`). `Theme.Tachiyomi` is defined twice: `values/themes.xml:90` (parent `Base.Theme.Tachiyomi`) and `values-v27/themes.xml:8` (parent `Base.V27.Tachiyomi`, which adds `android:navigationBarColor` = `?attr/colorSurface` and `windowLightNavigationBar` = `@bool/lightNavigationBar`). On API 27+ all palettes therefore use the v27 base.

`Theme.Tachiyomi.Monet` has no items in `values/themes.xml:93` and gets its items from `values-v31/themes.xml` (light) or `values-night-v31/themes.xml` (dark) on API 31+.

### 1.10 OBSERVED: palette-to-token map (applies to all 18 palettes)

Every palette style sets the same logical roles (`colorPrimary`, `colorOnPrimary`, `colorPrimaryContainer`, `colorOnPrimaryContainer`, `colorPrimaryInverse`, `colorSecondary`, `colorOnSecondary`, `colorSecondaryContainer`, `colorOnSecondaryContainer`, `colorTertiary`, `colorOnTertiary`, `colorTertiaryContainer`, `colorOnTertiaryContainer`, `android:colorBackground`, `colorOnBackground`, `colorSurface`, `colorOnSurface`, `colorSurfaceVariant`, `colorOnSurfaceVariant`, `elevationOverlayColor`, `colorSurfaceInverse`, `colorOnSurfaceInverse`, `colorError`, `colorOnError`, `colorErrorContainer`, `colorOnErrorContainer`, `colorOutline`, `colorOutlineVariant`, `scrimBackground`, `colorSurfaceBright`, `colorSurfaceDim`, `colorSurfaceContainer`, `colorSurfaceContainerHigh`, `colorSurfaceContainerHighest`, `colorSurfaceContainerLow`, `colorSurfaceContainerLowest`). The colour resource for role R of palette P is `@color/<p>_<R>` where `<p>` is the palette prefix (`tachiyomi`, `cottoncandy`, `greenapple`, `lavender`, `midnightdusk`, `mocha`, `strawberry`, `tako`, `tealturquoise`, `yinyang`, `yotsuba`, `cloudflare`, `sapphire`, `doom`, `matrix`, `tidalwave`, `nord`, `monochrome`). The palette prefix for each style is listed in section 6.

PROPOSAL: Kurobayomi should not expand all 18 palettes. The GTK default follows the user's GTK theme (memory: user runs Tokyo Night everywhere). Palettes are optional accent overrides. Only CottonCandy and Mocha are in this area's scope; the rest belong to area:49-theme-and-design-system.

### 1.11 OBSERVED: which colours are referenced by the scope but defined elsewhere

- `@color/tachiyomi_*`, `@color/amoled_*` (partially), `@color/splash`, `@color/divider_default`, `@color/accent_blue`, `@color/cover_placeholder`, `@color/md_*` are defined in `presentation-core/src/main/res/values/colors.xml`, `values/colors_tachiyomi.xml`, `values-night/colors.xml`, `values-night/colors_tachiyomi.xml`.
- `@color/m3_*` (dynamic colour, text colours), `m3_popupmenu_overlay_color`, `m3_alert_dialog_corner_size` come from the Material Components library (`com.google.android.material:material:1.12.0`, `gradle/libs.versions.toml:59`), not from this repo.
- `@android:color/white`, `@android:color/transparent`, `@android:interpolator/fast_out_slow_in`, `@android:anim/decelerate_interpolator`, `@android:interpolator/...` are framework resources.

---

## 2. Splash screen (launch)

### 2.1 OBSERVED: resources

- Splash theme `Theme.Tachiyomi.SplashScreen` (`app/src/main/res/values/themes.xml:660-668`), parent `Theme.SplashScreen` (AndroidX core-splashscreen):
  - `windowSplashScreenAnimatedIcon` = `@drawable/ic_tachi_splash` (108dp logo, centered, `ic_ani`)
  - `windowSplashScreenBackground` = `@color/splash` (`presentation-core/src/main/res/values/colors.xml`: `splash` = `@color/accent_blue` = `#54759E`; night `#202125`)
  - `postSplashScreenTheme` = `@style/Theme.Tachiyomi`
  - statusBarColor and navigationBarColor transparent; `windowLightStatusBar` false.

### 2.2 OBSERVED: lifecycle (`app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt`)

- `onCreate`: `isLaunch = savedInstanceState == null`; `splashScreen = if (isLaunch) installSplashScreen() else null` (lines 160-163). The comment says it prevents the splash from re-appearing on configuration changes.
- `var ready = false` (line 150). `ready` is set to true by `BrowseTab` in a `LaunchedEffect(Unit)` on first composition (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/BrowseTab.kt:103-104`), and by the extension installers (not relevant to the splash).
- Keep-on-screen condition (lines 316-321): the splash stays while `elapsed <= SPLASH_MIN_DURATION || (!ready && elapsed <= SPLASH_MAX_DURATION)`, where `elapsed = now - startTime` and `startTime` is captured just before the condition is registered.
- Constants (`MainActivity.kt:665-667`): `SPLASH_MIN_DURATION = 500` ms, `SPLASH_MAX_DURATION = 5000` ms, `SPLASH_EXIT_ANIM_DURATION = 400L` ms.

### 2.3 OBSERVED: pre-Android-12 exit animation (`MainActivity.kt:418-456`)

Applies only when `SDK_INT < 31` and `splashScreen != null`:
1. `window.statusBarColor` and `window.navigationBarColor` = transparent.
2. On exit: `iconView.translationY = 0`.
3. `activityAnim`: `ValueAnimator.ofFloat(1, 0)`, `LinearOutSlowInInterpolator`, 400 ms. Each update sets `root.translationY = value * 16.dpToPx`. Result: the content slides up from 16dp below to its place.
4. `splashAnim`: `ValueAnimator.ofFloat(1, 0)`, `FastOutSlowInInterpolator`, 400 ms. Each update sets `splashProvider.view.alpha = value`; on end it calls `splashProvider.remove()`.
5. Both start together.

PROPOSAL (Linux): there is no Android splash window. Show a splash page (logo at 108 px on the `.kb-splash` background) in a `gtk::Stack` until the first browse-tab equivalent signals ready. Keep the same timing contract: show for at least 500 ms, at most 5000 ms, and then cross-fade to the main window over 400 ms (`gtk::Stack` transition `Crossfade`, duration 400). The 16px upward slide is optional (P3).

---

## 3. Animated vector drawables: driving semantics

Compose drives an `AnimatedImageVector` with `rememberAnimatedVectorPainter(image, atEnd)`:
- `atEnd = false` shows the vector's base state (the `<vector>` paths as declared).
- A change of `atEnd` plays the matching `<target>` animators forward (false to true) or in reverse (true to false). The end state is the `valueTo` of each objectAnimator.
- Each objectAnimator runs for its `android:duration` (ms) with its `android:interpolator` (default `@android:interpolator/fast_out_slow_in` for all files that set it; `fast_out_slow_in` is a cubic bezier (0.4, 0, 0.2, 1)).

Call sites and the boolean that drives them:

| Drawable | Call site | Driver (`atEnd`) | Base state | End state |
|---|---|---|---|---|
| anim_caret_down | AnimeInfoHeader.kt:637-639, MangaInfoHeader.kt:605 | `!expanded` | caret pointing up/expanded | caret pointing down (rotation caret_l -45 to 45, caret_r 45 to -45, translate caret01 along a path) |
| anim_play_to_pause | MiddlePlayerControls.kt:86-113 | `!paused` | triangle (play) `M 8 5 L 8 19 L 19 12 Z` | two bars (pause) |
| anim_animelibrary_leave | AnimeLibraryTab.kt:77-83 (`isSelected`) | `isSelected` | per AVD base | per AVD end |
| anim_library_enter | MangaLibraryTab.kt:77 | tab selected state (`isSelected` pattern) | per AVD | per AVD |
| anim_browse_enter | BrowseTab.kt:43 | tab selected state | compass at 0 degrees | compass rotated 180 degrees |
| anim_history_enter | HistoriesTab.kt:35, DownloadsTab.kt:84 | tab selected | arrow at 360 degrees | arrow at 0 degrees |
| anim_more_enter | MoreTab.kt:49-53 | `isSelected` | three dots | three dots at their end positions |
| anim_updates_enter | UpdatesTab.kt:30-40, CategoriesTab.kt:38, StatsTab.kt:28, StorageTab.kt:28 | `isSelected` | per AVD | per AVD |

The tab-selection drivers for the last five are the same `isSelected` pattern in each TabOptions builder (`rememberAnimatedVectorPainter(image, isSelected)`).

### 3.1 Per-file timelines (OBSERVED, exact)

`anim_animelibrary_enter` (39 lines): one target `playbutt`, one objectAnimator, `pathData` morph, duration 300, `valueType=pathType`, interpolator `fast_out_slow_in`.
`anim_animelibrary_leave` (37 lines): same as enter with start and end paths swapped.
`anim_library_enter` (31 lines): target `bookmark`, pathData morph, 300 ms, `fast_out_slow_in`.
`anim_updates_enter` (27 lines): target `updates_filled`, pathData morph, 300 ms, `fast_out_slow_in`.
`anim_play_to_pause` (44 lines): target `path`: pathData 300 ms (from `M 8 5 L 8 12 L 19 12 L 19 12 L 8 5 M 8 12 L 8 19 L 19 12 L 19 12 L 8 12` to `M 5 6 L 5 10 L 19 10 L 19 6 L 5 6 M 5 14 L 5 18 L 19 18 L 19 14 L 5 14`); target `group`: rotation 0 to 90, 300 ms.
`anim_history_enter` (36 lines): target `arrow_head`: rotation 360 to 0, 500 ms, `floatType`.
`anim_caret_down` (85 lines): target `caret01`: 300 ms, `propertyXName=translateX`, `propertyYName=translateY`, path `M 12.0,9.0 c 0.0,0.66667 0.0,5.0 0.0,6.0` (a single cubic segment from (12, 9) to (12, 15)); target `caret_l`: rotation -45.0 to 45.0, 300 ms; target `caret_r`: rotation 45.0 to -45.0, 300 ms. Group base translate: `caret01` translateX 12, translateY 15, rotation 90.
`anim_browse_enter` (104 lines): target `compass`: pathData 300 ms (the compass morphs into a ring with a dot, valueFrom `M 9.99 9.99 ...`, valueTo `M 12 2 C 6.48 2 ...`); target `main_parent`: rotation 0 to 180, 300 ms; target `group` (set, default ordering `together`): scaleX 0 to 0 over 66 ms, then scaleX 0 to 1 over 100 ms starting at offset 66 (`decelerate_interpolator`), and the same for scaleY. Total 166 ms. Base: `ring_parent` scale 0.833.
`anim_more_enter` (201 lines): group `_R_G_L_2_G_D_0_P_0` (the leftmost of three circles, centre x about 270): ordering together, four objectAnimators: 83 ms (offset 0), 100 ms (offset 83), 133 ms (offset 183), 67 ms (offset 317); total 384 ms (there is a 1 ms gap between 316 and 317). Group `_R_G_L_1_G_D_0_P_0`: 150 ms (offset 0), 100 (150), 133 (250), 67 (383); total 450 ms. Group `_R_G_L_0_G_D_0_P_0`: 217 ms (offset 0), 100 (217), 133 (317), 67 (450); total 517 ms. Target `time_group`: translateX 0 to 1 over 833 ms; the `time_group` element has no children (`<group android:name="time_group" />`), so the animator moves nothing. Each dot's interpolator is `pathInterpolator` `M 0.0,0.0 c0.5,0 0.5,1 1.0,1.0` (an ease-in-out cubic).
`anim_updates_enter`: as above.

### 3.2 PROPOSAL (Linux) for each animation

| Animation | GTK approach | Duration | Notes |
|---|---|---|---|
| anim_caret_down | Icon swap between `pan-down-symbolic` and `pan-up-symbolic` plus CSS `-gtk-icon-transform: rotate(180deg)` transition | 300 ms | The path translate is dropped (decorative). |
| anim_play_to_pause | Two `gtk::Image`s stacked in a `gtk::Overlay`, crossfade via `opacity` transition | 300 ms | Keep `media-playback-start-symbolic` and `media-playback-pause-symbolic`. |
| anim_animelibrary_leave / enter | Two SVGs (`kb-animelibrary-outline-symbolic`, `kb-animelibrary-filled-symbolic`), crossfade | 300 ms | This is the two-state selector. |
| anim_library_enter, anim_updates_enter | Crossfade between the static icon and its end state; no path morph | 300 ms | Decorative. |
| anim_browse_enter | Rotate the compass icon 180 degrees via `-gtk-icon-transform` | 300 ms | Interpolator: `fast_out_slow_in` cubic (0.4, 0, 0.2, 1). |
| anim_history_enter | Rotate 360 to 0 via `-gtk-icon-transform` | 500 ms | Keep rotation; drop the clock hand morph. |
| anim_more_enter | Dropped (P3) | n/a | Static `view-more-horizontal-symbolic`. |

PROPOSAL for the interpolator: GTK's CSS `transition-timing-function: cubic-bezier(0.4, 0, 0.2, 1)` matches `fast_out_slow_in`. `decelerate_interpolator` maps to `cubic-bezier(0, 0, 0.2, 1)`.

---

## 4. Icon semantics (where each icon is used, states, and colour)

### 4.1 Tint and colour rules (OBSERVED)

- Most vectors set `android:fillColor="#000"` or `#FFF` and are recoloured at use time by Compose `Icon(tint=...)` or by the View `tint` attribute. The fill colour in the file is therefore not the rendered colour.
- Vectors with `android:tint="?attr/colorControlNormal"` (`ic_arrow_back_24dp`, `ic_calendar_clock_24dp`) are tinted from the theme; `colorControlNormal` is not set by the scope's themes (inherits Material 3 default).
- Vectors with a hard-coded fill (`ic_dns_24dp` `#e3e3e3`, `ic_launcher_foreground` colours, `ic_launcher_background` `#3DDC84`) are not recoloured.
- `ic_pause_circle_filled_24` and `ic_play_circle_filled_24` reference `?attr/colorOnPrimarySurface` and `?attr/colorControlActivated`, which are not defined in this repo (the drawables are unused).

### 4.2 Notification small icons (OBSERVED)

Every notification uses `setSmallIcon(R.drawable.<x>)`. The single status icon used for the app's notifications is `ic_ani` (`BackupNotifier.kt:30,40`; `AnimeLibraryUpdateNotifier.kt:180,220,256`; `MangaLibraryUpdateNotifier.kt:162,202,238`; `TorrentServerService.kt:115`; `AnimeExtensionInstallService.kt:28`; `MangaExtensionInstallService.kt:28`). Other notifications use the per-state icons listed in the inventory (warning, done, refresh, book, close, pause, play, photo, dns, info, folder, extension, system update).

The colour applied to every notification is `accent_blue` (`app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt:70-71`: `NotificationCompat.Builder(this, channelId).setColor(getColor(R.color.accent_blue))`). This is the Android notification accent. GNotification has no accent colour, so it is dropped on Linux.

### 4.3 Entry-list icons

- `ic_download_item_24dp` on each chapter/episode row when downloaded (`ChapterDownloadIndicator.kt`, `EpisodeDownloadIndicator.kt`).
- `ic_done_prev_24dp` in `EntryBottomActionMenu.kt` (the "mark previous as read" bottom action).
- `ic_calendar_clock_24dp` in the anime info header (release-time row).

### 4.4 Reader and player icons

- `ReadingMode.kt` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt`) maps each reading mode to one of the six `ic_reader_*` drawables. Mode-to-drawable mapping is in that file; the drawables are named `default`, `ltr`, `rtl`, `vertical`, `continuous_vertical`, `webtoon`.
- `app/src/main/java/eu/kanade/presentation/reader/appbars/BottomReaderBar.kt:57-60`: the crop-borders button shows `R.drawable.ic_crop_24dp` when `cropEnabled` is true and `R.drawable.ic_crop_off_24dp` when false; its content description is `MR.strings.pref_crop_borders`.
- `DoubleTapSeekTriangles.kt`: the seek triangle overlay (see 4.5).
- `SubtitleSettingsTypographyCard.kt`: `sharp_shadow_24` (line 348) and `outline_brand_family_24` (typography row).
- `PipActions.kt`: picture-in-picture buttons use `ic_play_arrow_24dp`, `ic_pause_24dp`, `ic_skip_next_24dp`, `ic_skip_previous_24dp`, `ic_forward_10_24dp`. PiP does not exist on Linux, so these are reused only as the transport icons of the Linux player (Replace).

### 4.5 Double-tap seek triangles (OBSERVED, exact)

`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/DoubleTapSeekTriangles.kt:21-48`:
- `isForward` selects rotation `0` (forward) or `180` (backward).
- Three copies of `ic_play_seek_triangle` in a `Row`, each drawn at `size(width=16.dp, height=20.dp)`, `tint = Color.White`.
- `animationDuration = 750L`; each of six steps uses `tween((750/5).toInt())` = 150 ms with no explicit easing, so Compose's default `tween` easing applies (`FastOutSlowInEasing`, cubic-bezier 0.4, 0, 0.2, 1). The loop is: alpha1 0 to 1, alpha2 0 to 1, alpha3 0 to 1, alpha1 1 to 0, alpha2 1 to 0, alpha3 1 to 0; repeated forever (6 x 150 = 900 ms per cycle).

PROPOSAL (Linux): three SVG triangles in a `gtk::Box`, each with `opacity` set from the same six-step schedule (150 ms steps) via a 16 ms GTK frame-clock tick or `gtk::Widget::add_tick_callback`; `rotate 180` for backward seeks via `-gtk-icon-transform: rotate(180deg)`.

### 4.6 Covers

`cover_error` is the `error` painter of Coil image loads (`ItemCover.kt:35`; `BrowseMangaIcons.kt:94`; `BrowseAnimeIcons.kt:94`). It is a 1-colour placeholder: background `#1F888888` (12 % grey), icon inset 48dp (the glyph is 24dp on a 96dp frame). Whether a cover failed is decided by the image pipeline (area:34-images-covers-cache).

---

## 5. Launcher shortcuts (`app/shortcuts.xml`)

Five static shortcuts (Android app shortcuts) map each action string to a MainActivity tab. Kotlin routing (`app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:471-489`):

| shortcutId (file) | Icon (file) | Label | Intent action | Routed to |
|---|---|---|---|---|
| show_library | sc_collections_bookmark_48dp | `@string/label_manga_library` | `eu.kanade.tachiyomi.SHOW_LIBRARY` | `HomeScreen.Tab.Library()` |
| show_animelib | sc_collections_baseline_48dp | `@string/label_anime_library` | `eu.kanade.tachiyomi.SHOW_ANIMELIB` | `HomeScreen.Tab.AnimeLib()` |
| show_recently_updated | sc_new_releases_48dp | `@string/label_recent_updates` | `eu.kanade.tachiyomi.SHOW_RECENTLY_UPDATED` | `HomeScreen.Tab.Updates` |
| show_recently_updated (duplicate id) | sc_history_48dp | `@string/label_recent_manga` | `eu.kanade.tachiyomi.SHOW_RECENTLY_READ` | `HomeScreen.Tab.History` |
| show_catalogues | sc_explore_48dp | `@string/browse` | `eu.kanade.tachiyomi.SHOW_CATALOGUES` | `HomeScreen.Tab.Browse(false)` |

- Every shortcut sets `shortcutDisabledMessage="@string/app_not_available"` and `android:enabled="true"`, and targets `eu.kanade.tachiyomi.ui.main.MainActivity`.
- BUG (OBSERVED): the history shortcut reuses `shortcutId="show_recently_updated"` (duplicate of the updates shortcut). Android keeps one of the two. Routing still works because routing is by action, not by id.
- Action strings are defined in `core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt:14-24`: `SHORTCUT_ANIMELIB`, `SHORTCUT_LIBRARY`, `SHORTCUT_ANIME`, `SHORTCUT_MANGA`, `SHORTCUT_UPDATES`, `SHORTCUT_HISTORY`, `SHORTCUT_SOURCES`, `SHORTCUT_ANIMEEXTENSIONS`, `SHORTCUT_EXTENSIONS`, `SHORTCUT_ANIME_DOWNLOADS`, `SHORTCUT_DOWNLOADS`. Shortcuts that exist in Kotlin but not in `shortcuts.xml`: `SHORTCUT_ANIME`, `SHORTCUT_MANGA`, `SHORTCUT_ANIMEEXTENSIONS`, `SHORTCUT_EXTENSIONS`, `SHORTCUT_ANIME_DOWNLOADS`, `SHORTCUT_DOWNLOADS` (dynamic, from notifications).

PROPOSAL (Linux): a `.desktop` file with `[Desktop Action]` groups for the five actions (`Library`, `Anime library`, `Recent updates`, `Recent manga`, `Browse`), `Icon=` set to the icon names in section 4 (`kb-library-symbolic`, `kb-animelibrary-filled-symbolic`, `kb-updates-outline-symbolic`, `document-open-recent-symbolic`, `system-search-symbolic`), and `Exec=kurobayomi --action=<name>`. Actions are handled by `gtk::Application` `command-line` or `app.activate-action`, mapping to the same five screens.

---

## 6. Palette reference (names only; values in section 1.10 and in the palette topic)

Palette prefix per `AppTheme` (for the `@color/<prefix>_<role>` references in `themes.xml`):

| Style | Prefix | Colour file(s) |
|---|---|---|
| Theme.Tachiyomi (base) | tachiyomi | presentation-core/src/main/res/values/colors_tachiyomi.xml |
| Theme.Tachiyomi.CottonCandy | cottoncandy | app/src/main/res/values/colors_cottoncandy.xml, app/src/main/res/values-night/colors_cottoncandy.xml |
| Theme.Tachiyomi.Mocha | mocha | app/src/main/res/values/colors_mocha.xml, app/src/main/res/values-night/colors_mocha.xml |
| Theme.Tachiyomi.GreenApple | greenapple | presentation-core/src/main/res/values*/colors_greenapple.xml |
| Theme.Tachiyomi.Lavender | lavender | presentation-core/src/main/res/values*/color_lavender.xml |
| Theme.Tachiyomi.MidnightDusk | midnightdusk | presentation-core/src/main/res/values*/colors_midnightdusk.xml |
| Theme.Tachiyomi.StrawberryDaiquiri | strawberry | presentation-core/src/main/res/values*/colors_strawberry.xml |
| Theme.Tachiyomi.Tako | tako | presentation-core/src/main/res/values*/colors_tako.xml |
| Theme.Tachiyomi.TealTurquoise | tealturquoise | presentation-core/src/main/res/values*/colors_tealturqoise.xml |
| Theme.Tachiyomi.YinYang | yinyang | presentation-core/src/main/res/values*/colors_yinyang.xml |
| Theme.Tachiyomi.Yotsuba | yotsuba | presentation-core/src/main/res/values*/colors_yotsuba.xml |
| Theme.Tachiyomi.Cloudflare | cloudflare | presentation-core/src/main/res/values*/color_cloudflare.xml |
| Theme.Tachiyomi.Sapphire | sapphire | presentation-core/src/main/res/values*/color_sapphire.xml |
| Theme.Tachiyomi.Doom | doom | presentation-core/src/main/res/values*/color_doom.xml |
| Theme.Tachiyomi.Matrix | matrix | presentation-core/src/main/res/values*/color_matrix.xml |
| Theme.Tachiyomi.TidalWave | tidalwave | presentation-core/src/main/res/values*/colors_tidalwave.xml |
| Theme.Tachiyomi.Nord | nord | presentation-core/src/main/res/values*/colors_nord.xml |
| Theme.Tachiyomi.Monochrome | monochrome | presentation-core/src/main/res/values*/colors_monochrome.xml |
| Theme.Tachiyomi.Monet | (system) `m3_sys_color_dynamic_*` | Material Components library |

The file-to-prefix mapping for the presentation-core files was taken from the file names in `presentation-core/src/main/res/` and the prefixes used in `values/themes.xml`; the prefix for each palette is the one used in its `@color/` references.

---

## 7. Dimensions and integers

- `screen_edge_margin`: 16dp (`values/dimens.xml:2`); 24dp on smallest width at least 720dp (`values-sw720dp/dimens.xml:2`). No Kotlin or XML reference to `R.dimen.screen_edge_margin` exists in the repo (OBSERVED: zero references outside the two files). Its effective use is therefore nil.
- `appwidget_background_radius`: 16dp; `appwidget_inner_radius`: 12dp (`values/dimens.xml:4-5`). Used by `presentation-widget` (`GlanceUtils.kt:10,13`, `appwidget_background.xml`, `appwidget_cover_error.xml`). The `presentation-widget` module has its own `values/dimens.xml` with identical values, and `values-v31/dimens.xml` sets `appwidget_background_radius` to `@android:dimen/system_app_widget_background_radius`. Because the app module's values win over the library's during resource merge, the app's 16dp overrides the v31 system value on API 31+ (an OBSERVED consequence of Android's merge order, relevant only to the widget).
- `player_animation_duration`: 200 (`values/integers.xml:3`). Used as `android:duration="@integer/player_animation_duration"` in 10 files in `app/src/main/res/anim/` (`player_enter_bottom`, `player_enter_left`, `player_enter_right`, `player_enter_top`, `player_exit_bottom`, `player_exit_left`, `player_exit_right`, `player_exit_top`, `player_fade_in`, `player_fade_out`). Units: milliseconds.

## 8. Ids

- `reader_pager` (`values/ids.xml:3`) is assigned at runtime: `pager.id = R.id.reader_pager` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt:88`). Used to find the pager view from the reader layout.

## 9. Attrs (declare-styleable)

- `MinMaxNumberPicker` (`min`, `max`, both integer): read in `app/src/main/java/eu/kanade/tachiyomi/widget/MinMaxNumberPicker.kt:28-31` with default 0. The class is referenced only from its own file (OBSERVED: no layout or Kotlin constructs it), so the styleable is dead.
- `MaterialSpinnerView` (`title`, `android:entries`, `summary`): no class in the repo. Dead.

## 10. Layout-related XML used by styles (outside scope, referenced)

- `@layout/m3_alert_dialog`, `@layout/pref_widget_switch_material` (`app/src/main/res/layout/pref_widget_switch_material.xml` exists; `m3_alert_dialog` comes from the Material library).

## 11. Edge cases and state (the rest is in p1-edge-cases-and-tests.md)

- Unknown or deprecated `AppTheme` value: style falls back to `Theme_Tachiyomi` (no error).
- AMOLED flag set while light mode is active: no visible change (both Compose and XML).
- MONET selected on a device without dynamic colour: `AppThemePreferenceWidget.kt:85` hides the option from the picker (`it == AppTheme.MONET && !DeviceUtil.isDynamicColorAvailable` filtered out). If the stored value is MONET anyway, `getThemeColorScheme` still builds `MonetColorScheme` (which then uses the seed fallback `MonetCompatColorScheme`).
- Dialog context wrap (1.5) rebuilds the theme but does not re-run `setTheme` on the activity.
- Splash `ready` is never reset; configuration changes skip the splash (`isLaunch == false`).

## 12. Message keys referenced by the scope

- `app/src/main/res/menu/main_nav.xml` (not in scope; the menu has no Kotlin reference, so it is dead): `@string/label_anime_library`, `@string/label_manga`, `@string/label_recent_updates`, `@string/browse`, `@string/label_more`.
- `app/shortcuts.xml`: `@string/app_not_available`, `@string/label_manga_library`, `@string/label_anime_library`, `@string/label_recent_updates`, `@string/label_recent_manga`, `@string/browse`. These are Android `strings.xml` entries (moko-resources in `i18n`/`i18n-aniyomi`), outside this scope; the English keys are the ones to reuse for the `.desktop` action names.
- The scope itself defines no user-visible strings.

## 13. Side effects in this area

- None at runtime from resource files. Side effects come from the code paths above: splash timing (MainActivity), theme context wrap (ContextExtensions), notification colour (NotificationExtensions), double-tap triangle loop (DoubleTapSeekTriangles), tab animation drivers (tab TabOptions).
- No file writes, no DB writes, no WorkManager jobs, no network.

## 14. Concurrency and threading

- Compose animations run on the Compose frame clock (main thread). `DoubleTapSeekTriangles` uses a `LaunchedEffect` coroutine loop; it is cancelled when the composable leaves composition.
- The splash `keepOnScreenCondition` lambda is evaluated on each frame of the splash window (main thread).
- Pre-Android-12 exit animation uses `ValueAnimator`, main thread.

PROPOSAL (Linux): all animations on the GTK main loop. Use `glib::timeout_add_local` or `gtk::Widget::add_tick_callback` for frame-driven effects, and `glib::MainContext::spawn_local` for the splash timer (500/5000 ms logic). No tokio involvement.

## 15. Timings and limits (complete list)

| Name | Value | Unit | Source |
|---|---|---|---|
| splash minimum duration | 500 | ms | MainActivity.kt:665 |
| splash maximum duration (waiting for ready) | 5000 | ms | MainActivity.kt:666 |
| splash exit animation duration | 400 | ms | MainActivity.kt:667 |
| splash exit slide distance | 16 | dp | MainActivity.kt:439 (`value * 16.dpToPx`) |
| player animation duration | 200 | ms | values/integers.xml:3 |
| double-tap seek cycle step | 150 (750 / 5) | ms | DoubleTapSeekTriangles.kt:22,30-35 |
| double-tap seek cycle total | 900 (6 steps) | ms | DoubleTapSeekTriangles.kt:28-36 |
| play/pause morph | 300 | ms | anim_play_to_pause.xml |
| tab icon morphs | 300 | ms | anim_*_enter/leave.xml |
| history icon rotation | 500 | ms | anim_history_enter.xml |
| browse compass group | 66 + 100 (offset 66) | ms | anim_browse_enter.xml |
| more-dot morph | 384 / 450 / 517 | ms | anim_more_enter.xml |
| caret expand | 300 | ms | anim_caret_down.xml |
| dialog backgroundDimAmount | 0.32 | fraction | values/themes.xml:71 |
| screen edge margin, phone | 16 | dp | values/dimens.xml:2 |
| screen edge margin, tablet | 24 | dp | values-sw720dp/dimens.xml:2 |
| widget background radius | 16 | dp | values/dimens.xml:4 |
| widget inner radius | 12 | dp | values/dimens.xml:5 |
| cover placeholder alpha | 0x1F (about 12 %) | alpha | colors via presentation-core |
| focus/press ripple alpha | 0.1 | alpha | TachiyomiTheme.kt RIPPLE_*_ALPHA (other area) |
| dragged card overlay alpha | 0.08 | alpha | color/draggable_card_foreground.xml |
| splash logo size | 108 | dp | drawable/ic_tachi_splash.xml |
| logo size in More tab | 64 | dp | LogoHeader.kt (padding vertical 56dp) |
