# Area 49 (part 1): behavior spec

All paths are repo-relative to `/home/blackbox/aniyomi-src`. "OBSERVED" = what the Kotlin does. "Proposal:" = what Kurobayomi should do. Line numbers are from the files as present in the clone.

## 1. Theme resolution (OBSERVED)

Source: `app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt:37-86`.

Algorithm for `TachiyomiTheme(appTheme: AppTheme? = null, amoled: Boolean? = null, content)`:
1. `uiPreferences = Injekt.get<UiPreferences>()`.
2. `appTheme = appTheme ?: uiPreferences.appTheme().get()`.
3. `isAmoled = amoled ?: uiPreferences.themeDarkAmoled().get()`.
4. `BaseTachiyomiTheme(appTheme, isAmoled, content)` calls `MaterialTheme(colorScheme = getThemeColorScheme(appTheme, isAmoled), content)`.
5. `getThemeColorScheme`:
   - If `appTheme == AppTheme.MONET`, `colorScheme = MonetColorScheme(LocalContext.current)`.
   - Else `colorScheme = colorSchemes.getOrDefault(appTheme, TachiyomiColorScheme)`.
   - Returns `colorScheme.getColorScheme(isSystemInDarkTheme(), isAmoled)`.
   - The dark/light choice therefore follows the system (`isSystemInDarkTheme()`), which itself follows the AppCompat night mode (see section 2).
6. `TachiyomiPreviewTheme(appTheme = AppTheme.DEFAULT, isAmoled = false, content)` skips the preferences entirely. It is used by previews and by some dialogs in other areas (58 call sites under `app/`).

Map `colorSchemes` (`TachiyomiTheme.kt:104-123`), 18 entries, any other key (including the deprecated `DARK_BLUE`, `HOT_PINK`, `BLUE`) falls back to `TachiyomiColorScheme`:

| AppTheme | Scheme object | AppTheme title key (`titleRes`) |
|---|---|---|
| DEFAULT | TachiyomiColorScheme | `MR.strings.label_default` |
| CLOUDFLARE | CloudflareColorScheme | `AYMR.strings.theme_cloudflare` |
| COTTONCANDY | CottoncandyColorScheme | `AYMR.strings.theme_cottoncandy` |
| DOOM | DoomColorScheme | `AYMR.strings.theme_doom` |
| GREEN_APPLE | GreenAppleColorScheme | `MR.strings.theme_greenapple` |
| LAVENDER | LavenderColorScheme | `MR.strings.theme_lavender` |
| MATRIX | MatrixColorScheme | `AYMR.strings.theme_matrix` |
| MIDNIGHT_DUSK | MidnightDuskColorScheme | `MR.strings.theme_midnightdusk` |
| MONOCHROME | MonochromeColorScheme | `MR.strings.theme_monochrome` |
| MOCHA | MochaColorScheme | `AYMR.strings.theme_mocha` |
| SAPPHIRE | SapphireColorScheme | `AYMR.strings.theme_sapphire` |
| NORD | NordColorScheme | `MR.strings.theme_nord` |
| STRAWBERRY_DAIQUIRI | StrawberryColorScheme | `MR.strings.theme_strawberrydaiquiri` |
| TAKO | TakoColorScheme | `MR.strings.theme_tako` |
| TEALTURQUOISE | TealTurqoiseColorScheme | `MR.strings.theme_tealturquoise` |
| TIDAL_WAVE | TidalWaveColorScheme | `MR.strings.theme_tidalwave` |
| YINYANG | YinYangColorScheme | `MR.strings.theme_yinyang` |
| YOTSUBA | YotsubaColorScheme | `MR.strings.theme_yotsuba` |
| MONET | MonetColorScheme (special-cased, not in map) | `MR.strings.theme_monet` |

`AppTheme` is declared in `app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt` (enum constants in order: DEFAULT, MONET, CLOUDFLARE, COTTONCANDY, DOOM, GREEN_APPLE, LAVENDER, MATRIX, MIDNIGHT_DUSK, MOCHA, SAPPHIRE, NORD, STRAWBERRY_DAIQUIRI, TAKO, TEALTURQUOISE, TIDAL_WAVE, YINYANG, YOTSUBA, MONOCHROME, DARK_BLUE(null), HOT_PINK(null), BLUE(null)). Entries with `titleRes == null` are hidden from the picker (`AppThemePreferenceWidget.kt`, `filterNot { it.titleRes == null || (it == MONET && !DeviceUtil.isDynamicColorAvailable) }`).

### 1.1 AMOLED override (OBSERVED)

`BaseColorScheme.getColorScheme(isDark, isAmoled)` (`BaseColorScheme.kt:17-34`):
- `!isDark` returns `lightScheme` unchanged (AMOLED flag ignored in light mode).
- `isDark && !isAmoled` returns `darkScheme`.
- `isDark && isAmoled` returns `darkScheme.copy(...)` with these overrides:
  - `background = Color.Black` (0xFF000000)
  - `onBackground = Color.White` (0xFFFFFFFF)
  - `surface = Color.Black`
  - `onSurface = Color.White`
  - `surfaceVariant = surfaceContainer` (0xFF0C0C0C)
  - `surfaceContainerLowest = surfaceContainer` (0xFF0C0C0C)
  - `surfaceContainerLow = surfaceContainer` (0xFF0C0C0C)
  - `surfaceContainer = surfaceContainer` (0xFF0C0C0C)
  - `surfaceContainerHigh = surfaceContainerHigh` (0xFF131313)
  - `surfaceContainerHighest = surfaceContainerHighest` (0xFF1B1B1B)
- Not overridden: `surfaceDim`, `surfaceBright`, `inverseSurface`, `primary*`, `secondary*`, `tertiary*`, `error*`, `outline*`, `surfaceTint`, `inversePrimary`.
- Comment in source: "Cannot be pure black as there's content scrolling behind it" (navigation bar, M3 guideline link in the source).
- Note that Monet (dynamic) goes through the same override, so Monet AMOLED also replaces the surface tokens above.

### 1.2 Token omission (OBSERVED, important)

Kotlin `darkColorScheme(...)` and `lightColorScheme(...)` take every `ColorScheme` parameter with a default. Any token a scheme does not list is the androidx.compose.material3 default for that token, which is the M3 baseline palette, not a value derived from the scheme. The per-scheme counts and the exact omitted token names are in `p1-topic-color-token-tables.md`. The base set is 36 tokens (the 36 non-fixed `ColorScheme` roles used by this app). Explicit counts per mode: GreenApple, Lavender, Monochrome and Strawberry 35 (1 omitted, `surfaceTint`); Tachiyomi 33; Nord 32 dark / 31 light; MidnightDusk, Tako, TealTurqoise, TidalWave, YinYang and Yotsuba 28; Cottoncandy and Mocha 23; Cloudflare, Doom, Matrix and Sapphire 22. The 22-23 group omits 13-14 tokens per mode (for example `outlineVariant`, `scrim`, `surfaceDim`, `surfaceBright`, `surfaceContainer*`, `error*` in Cloudflare).

Proposal: Kurobayomi must not inherit unknown defaults. Each scheme in Rust is a full table of 36 base tokens (Section 4 of `p1-rust-design.md`). For omitted tokens, the Rust port copies the values the pinned androidx.compose.material3 library uses (the M3 baseline `ColorDarkTokens` / `ColorLightTokens`). The engineer must dump those values once from the library (not vendored in this repo) and freeze them into `src/theme/schemes/mod.rs` as `BASELINE_DARK` and `BASELINE_LIGHT`. Acceptance test in `p1-edge-cases-and-tests.md` (TEST-49-07).

Note: the `darkColorScheme` default for `surfaceTint` is `primary` (the scheme's own primary), so omitted `surfaceTint` equals the scheme's explicit primary in both modes.

### 1.3 Shapes, typography, and defaults not set here (OBSERVED)

`TachiyomiTheme` passes only `colorScheme` to `MaterialTheme`. Therefore `MaterialTheme.shapes` and `MaterialTheme.typography` are the M3 library defaults. Values that the rest of the app relies on (library constants, not in this repo):
- Shapes (M3 defaults): extraSmall 4 dp, small 8 dp, medium 12 dp, large 16 dp, extraLarge 28 dp.
- Typography styles referenced in this area: bodySmall (12 sp / 16 sp line), bodyMedium (14 sp / 20 sp), labelLarge (14 sp, weight Medium), titleSmall (14 sp, weight Medium), titleMedium (16 sp / 24 sp, weight Medium), headlineSmall (24 sp / 32 sp, weight Normal). These are the M3 type scale values.
- The repo's own `header` style is defined in part 2 (`presentation-core/src/main/java/tachiyomi/presentation/core/theme/Typography.kt:9-14`): `bodyMedium.copy(color = onSurfaceVariant, fontWeight = SemiBold)`.

Proposal: Kurobayomi uses the same numbers as CSS: `--kuro-text-body-medium: 14px/20px`, etc. (Section 5 of `p1-rust-design.md`).

### 1.4 Player ripple configuration (OBSERVED)

`playerRippleConfiguration` (`TachiyomiTheme.kt:93-102`) is a composable getter returning `RippleConfiguration(color, rippleAlpha)`:
- `color = if (isSystemInDarkTheme()) Color.White else Color.Black`. It uses the system dark flag, not the app scheme.
- `RippleAlpha(draggedAlpha = .1f, focusedAlpha = .1f, hoveredAlpha = .1f, pressedAlpha = .1f)`.
- Used at `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt:318` and `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt:157` via `LocalRippleConfiguration provides`.

Proposal: the player overlay in Kurobayomi (area 46) uses a CSS state layer: `background-color: alpha(<fg>, 0.10)` on `:hover`, `:focus-visible`, `:active`, and a drag state, where `<fg>` is white when the resolved theme is dark and black when light. Match the source exactly: system dark flag, not the app scheme.

### 1.5 Behavior summary of theme inputs

| Input | Source | Default | Effect |
|---|---|---|---|
| `pref_app_theme` | `UiPreferences.appTheme()` | MONET if `DeviceUtil.isDynamicColorAvailable`, else DEFAULT | Selects the scheme (Section 1) |
| `pref_theme_dark_amoled_key` | `UiPreferences.themeDarkAmoled()` | false | AMOLED override (Section 1.1) |
| `pref_theme_mode_key` | `UiPreferences.themeMode()` | `ThemeMode.SYSTEM` | Light/dark/system; applied via `setAppCompatDelegateThemeMode` (Section 2) |

## 2. Light/dark mode application (OBSERVED)

- `app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt`: `enum class ThemeMode { LIGHT, DARK, SYSTEM }`; `setAppCompatDelegateThemeMode(themeMode)` maps LIGHT to `AppCompatDelegate.MODE_NIGHT_NO`, DARK to `MODE_NIGHT_YES`, SYSTEM to `MODE_NIGHT_FOLLOW_SYSTEM`.
- The Compose layer does not read `ThemeMode`. `isSystemInDarkTheme()` in `getThemeColorScheme` reads the activity configuration `uiMode`, which AppCompat rewrites when the night mode changes.
- `ContextExtensions.kt:125-150` (`app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt`) builds a `ContextThemeWrapper` with an overridden `UI_MODE_NIGHT` when the reader background setting differs from the app mode. This is area 44/43 territory; it reads `appTheme()` and `themeDarkAmoled()` through `ThemingDelegate.getThemeResIds`.

Proposal: Kurobayomi has one `ThemeMode` (System/Light/Dark) preference. System mode reads the `org.freedesktop.appearance color-scheme` portal value (1 = dark, 2 = light, 0 = no preference, treated as light). Light and Dark force the value. The resolved boolean is the single input to the scheme choice.

## 3. Monet / dynamic colors (OBSERVED)

Source: `MonetColorScheme.kt:21-57`.

Constructor selection (evaluated once per `MonetColorScheme(context)` instance, which is created on every `TachiyomiTheme` composition with `appTheme == MONET`):
1. `Build.VERSION.SDK_INT >= 31 (S)`: `monet = MonetSystemColorScheme(context)`, which uses `dynamicLightColorScheme(context)` and `dynamicDarkColorScheme(context)`.
2. Else `Build.VERSION.SDK_INT >= 27 (O_MR1)`: read `WallpaperManager.getInstance(context).getWallpaperColors(WallpaperManager.FLAG_SYSTEM)?.primaryColor?.toArgb()`. If non-null, `monet = MonetCompatColorScheme(context, seed)`; otherwise `TachiyomiColorScheme`.
3. Else `TachiyomiColorScheme`.

`darkScheme` and `lightScheme` are getters delegating to `monet`.

`MonetCompatColorScheme.generateColorSchemeFromSeed(context, seed, dark)` (`MonetColorScheme.kt:74-123`):
- `scheme = SchemeContent(Hct.fromInt(seed), dark, contrast)` where `contrast = UiModeManager.contrast.toDouble()` on API 34+ (`Build.VERSION_CODES.UPSIDE_DOWN_CAKE`), else `0.0`.
- Each `ColorScheme` field is `MaterialDynamicColors().<token>().getArgb(scheme)`, wrapped as Compose `Color(argb)`.
- Fixed values: `scrim = Color.Black`.
- Observed defect: `tertiaryContainer = dynamicColors.tertiary().getArgb(scheme)` (line 97). It uses the `tertiary` role, not `tertiaryContainer`. Kurobayomi must keep the correct role (see T-49a-006 acceptance) and must not copy the defect.

Seed extraction helper `MonetColorScheme.extractSeedColorFromImage(bitmap)` (`MonetColorScheme.kt:48-55`): reads all pixels, `QuantizerCelebi.quantize(pixels, 128)` then `Score.score(colors, 1, 0)[0]`, returns `null` when the result is 0. It has no caller in `app/` (grep found only the definition), so it is dead code in this clone.

### 3.1 Linux Monet (Proposal)

Android provides the system palette. Linux has no equivalent public API that gives a full M3 palette. Proposal, in order of preference (the first that is available wins):
1. `org.freedesktop.portal.Settings` key `org.freedesktop.appearance` / `accent-color` (type `(ddd)` RGB in 0.0-1.0). Read with the `ashpd` crate (`ashpd::desktop::settings::Settings::accent_color()`). Use it as the Monet seed. Check the value is present on the user's Plasma session before relying on it (`busctl --user call org.freedesktop.portal.Desktop ...`).
2. KDE Plasma colors: `~/.config/kdeglobals` `[Colors:Header] BackgroundNormal` or `[General] AccentColor`. Parsed only when the portal key is absent.
3. Fallback: the `DEFAULT` scheme. Monet is hidden from the picker when neither source gives a seed (mirrors `isDynamicColorAvailable`).

The seed becomes the HCT source color. The palette is generated with `SchemeContent` semantics (section 3 above) using a Rust port of `material-color-utilities` (`Hct`, `SchemeContent`, `MaterialDynamicColors`). Crate choice is an open question for the author: evaluate `material-colors` on crates.io first; if it lacks `SchemeContent` and `MaterialDynamicColors`, port those ~600 lines. This is recorded as an assumption in `p1-rust-design.md` section 7.

Wallpaper seed (the compat path via `QuantizerCelebi`) is dropped on Linux: there is no portable wallpaper-colors API. Reason: KDE/GNOME do not expose the wallpaper palette over a stable interface. Users lose "color from my wallpaper" unless the accent-color portal reflects it.

## 4. Window size classes (OBSERVED)

Source: `app/src/main/java/mihon/core/designsystem/utils/WindowSize.kt`.
- `isMediumWidthWindow()` = `LocalConfiguration.current.screenWidthDp > 600`.
- `isExpandedWidthWindow()` = `screenWidthDp > 840`.
- Both use strict greater-than, so exactly 600 dp is "compact" and exactly 840 dp is "medium".
- Callers in this clone: `app/src/main/java/mihon/feature/upcoming/components/calendar/Calendar.kt:85` (`isMediumWidthWindow() && !isExpandedWidthWindow()` chooses a layout modifier). Area 42 owns that layout.

Proposal: the Rust window size is the GTK window's content width in logical pixels, which equals `screenWidthDp` only when the scale factor is 1. Compute `width_dp = widget.width() as f64 / scale_factor` where `scale_factor = widget.scale_factor()`. Recompute on `notify::default-width` and `notify::scale-factor`. Expose `WindowClass::{Compact, Medium, Expanded}` with thresholds 600 and 840 (strict >). Section 5 of `p1-rust-design.md`.

## 5. AdaptiveSheet (OBSERVED)

Source: `presentation-core/src/main/java/tachiyomi/presentation/core/components/AdaptiveSheet.kt`.

Inputs: `isTabletUi` (caller decides, typically a width check in the caller), `enableSwipeDismiss`, `onDismissRequest`, `modifier`, `content`.

Constants:
- `maxWidth = 600.dp` when orientation is landscape (`ORIENTATION_LANDSCAPE`), else `460.dp`.
- `sheetAnimationSpec = tween<Float>(durationMillis = 350)`.
- Tablet branch: the `alpha` animation is `animateFloatAsState(targetValue = targetAlpha, sheetAnimationSpec, label "alpha")`.
- Bottom-sheet branch: `positionalThreshold = 56.dp`, `velocityThreshold = 125.dp` (converted with `LocalDensity`), `snapAnimationSpec = sheetAnimationSpec`, `decayAnimationSpec = rememberSplineBasedDecay<Float>()`.

### 5.1 Tablet branch (`isTabletUi == true`)
1. Full-size `Box` with `clickable(interactionSource = null, indication = null, onClick = internalOnDismissRequest)`, `fillMaxSize`, `alpha(alpha)`, `contentAlignment = Center`. The Box has no background color, so there is no scrim (OBSERVED: no dimming layer).
2. `internalOnDismissRequest`: launch a coroutine that sets `targetAlpha = 0f` and then calls `onDismissRequest()`. The sheet fades out over 350 ms before the caller removes it.
3. Inside: `Surface(modifier = requiredWidthIn(max = maxWidth).clickable(no-op).systemBarsPadding().padding(vertical = 16.dp).then(modifier), shape = MaterialTheme.shapes.extraLarge, color = surfaceContainerHigh, content)`. The inner click is consumed so taps on the sheet do not dismiss.
4. `BackHandler(enabled = alpha > 0f, onBack = internalOnDismissRequest)`.
5. `LaunchedEffect(Unit) { targetAlpha = 1f }` fades in on first composition.

### 5.2 Bottom-sheet branch (`isTabletUi == false`)
Anchored drag model with two anchors: `0` at offset 0 px (open) and `1` at `it.height` px (closed), where `it` is the full box height (`onSizeChanged`).
- `initialValue = 1` (closed), then `LaunchedEffect(anchoredDraggableState)` calls `animateTo(0)` to open.
- Outer `Box`: `clickable(no-op indication)` to dismiss, `fillMaxSize`, `onSizeChanged { updateAnchors }`, `contentAlignment = BottomCenter`.
- `internalOnDismissRequest`: if `settledValue == 0` (open), `animateTo(1)` in a coroutine. When the sheet finishes at 1, `snapshotFlow { settledValue }.drop(1).filter { it == 1 }.collectLatest { onDismissRequest() }`.
- `Surface`: `widthIn(max = maxWidth)`, inner click no-op, `nestedScroll` only when `enableSwipeDismiss` (see 5.3), then `offset { IntOffset(0, offset.roundToInt() or 0 if not finite) }`, `anchoredDraggable(orientation = Vertical, enabled = enableSwipeDismiss)`, `navigationBarsPadding()`, `statusBarsPadding()`, `shape = extraLarge`, `color = surfaceContainerHigh`.
- `BackHandler(enabled = targetValue == 0, onBack = internalOnDismissRequest)`.

### 5.3 Nested-scroll connection for swipe dismiss (OBSERVED, `preUpPostDownNestedScrollConnection`)
- `onPreScroll(available, source)`: if `delta < 0` (finger moves up) and source is `UserInput`, consume via `dispatchRawDelta(delta)`. Otherwise consume nothing. This means a downward drag is consumed only by the sheet.
- `onPostScroll(consumed, available, source)`: when source is `UserInput`, `dispatchRawDelta(available.y)`.
- `onPreFling(velocity)`: if `velocity.y < 0` and `offset > anchors.minPosition()`, call `onFling(velocity)` (settle) and consume all velocity. Otherwise `Velocity.Zero`.
- `onPostFling(consumed, available)`: `onFling(available.y)`, return `available`.

Settling: `settle(velocity)` decides between anchors using position threshold 56 dp and velocity threshold 125 dp, then animates with `sheetAnimationSpec` (350 ms tween). Velocity uses only `y`.

### 5.4 Sheet state machine (summary)

| State | Trigger | Next |
|---|---|---|
| Closed (anchor 1) | composition start | Opening (animate to 0) |
| Opening | animation complete | Open (anchor 0) |
| Open | drag down past 56 dp or fling down over 125 dp/s, or back, or outside tap (bottom branch only if `settledValue == 0`) | Closing (animate to 1) |
| Closing | settled at 1 | `onDismissRequest()` called once |
| Tablet Hidden (alpha 0) | composition start | Showing (alpha 1 over 350 ms) |
| Tablet Showing | outside tap or back | Hiding (alpha 0 over 350 ms), then `onDismissRequest()` |

Tablet has no drag. Outside tap on the bottom branch is ignored unless the sheet is open.

Proposal (GTK): replace the anchored drag with a `gtk::Revealer`-like slide plus a `gtk::GestureDrag` on the handle or the sheet top. Snap rule: close when `final_y > 56 dp` or `velocity_y > 125 dp/s`, else open. Keep the 350 ms duration (`transition-duration: 350ms` in CSS). Use a centered `gtk::Window` (modal, transient) for the tablet branch. Keep a non-dimmed backdrop because OBSERVED has no scrim; Proposal adds an optional scrim `rgba(0,0,0,0)` to match.

## 6. Badges (OBSERVED)

Source: `Badges.kt`.

- `BadgeGroup(modifier, shape = MaterialTheme.shapes.extraSmall, content: RowScope)`: `Row(modifier.clip(shape))`. Extra small shape = 4 dp.
- `Badge(text, modifier, color = colorScheme.secondary, textColor = colorScheme.onSecondary, shape = RectangleShape)`:
  - `Text` with `clip(shape)`, `background(color)`, `padding(horizontal = 3.dp, vertical = 1.dp)`, `color = textColor`, `fontWeight = FontWeight.Medium`, `maxLines = 1`, `style = bodySmall`.
- `Badge(imageVector, modifier, color = secondary, iconColor = onSecondary, shape = RectangleShape)`:
  - Inline content placeholder id `"[icon]"`, placeholder `width = height = bodySmall.fontSize` (12 sp), `PlaceholderVerticalAlign.Center`.
  - Renders `Icon(imageVector, tint = iconColor, contentDescription = null)` inline in a `Text`, `color = iconColor`, same padding/weight/style as text badge.
- Callers: 33 `Badge(` call sites. Colors are normally the theme's `secondary` (unread) and `tertiary` (downloaded) per the scheme comments (e.g., `GreenAppleColorScheme.kt:25-30`: "Unread badge", "Downloaded badge").

Proposal: GTK `gtk::Label` with CSS classes `.kuro-badge` (padding 1px 3px, font-weight 500, font-size 0.75em) and an optional `gtk::Image` child. Colors come from the `@define-color` tokens (Section 4 of `p1-rust-design.md`).

## 7. Progress indicator (OBSERVED)

Source: `CircularProgressIndicator.kt:39-76`.

`CombinedCircularProgressIndicator(progress: () -> Float, modifier)`:
- `AnimatedContent(targetState = progress() == 0f, transitionSpec = fadeIn() togetherWith fadeOut(), label "progressState")`.
- `indeterminate == true` (progress exactly 0f): `CircularProgressIndicator()` (M3 indeterminate spinner).
- Otherwise (determinate):
  - `rotation` from an infinite transition: `initialValue 0f`, `targetValue 360f`, `infiniteRepeatable(tween(2000, easing = LinearEasing), RepeatMode.Restart)`. One full turn per 2000 ms, linear, restart.
  - `animatedProgress = animateFloatAsState(progress(), ProgressIndicatorDefaults.ProgressAnimationSpec)` (M3 library spec).
  - `CircularProgressIndicator(progress = { animatedProgress }, modifier = Modifier.rotate(rotation))`. The determinate arc itself rotates. Its comment says the rotation exists so the user knows the app is not stuck.
- Preview (`CombinedCircularProgressIndicatorPreview`) cycles 0 -> 0.15 -> 0.25 -> 0.5 -> 0.75 -> 0.95 -> 0. Preview only.

Proposal: `gtk::Spinner` for indeterminate. For determinate, `gtk::DrawingArea` drawing an arc of `progress * 360deg` with a 2000 ms rotation tick (`glib::timeout_add_local` at 16 ms or `FrameClock`). Use the CSS color `@accent_color`. Progress smoothing: first-order low-pass with the M3 spec replaced by a 200 ms ease-out; this is a deliberate change, record it in the task acceptance.

## 8. Collapsible box (OBSERVED)

`CollapsibleBox(heading, content)`:
- `expanded = remember { mutableStateOf(false) }`: default collapsed, not saved across recomposition loss (no `rememberSaveable`).
- Header row: `fillMaxWidth`, `clickable { expanded = !expanded }`, `padding(horizontal = 24.dp, vertical = 12.dp)`. Text with `MaterialTheme.typography.header`; `Spacer(weight 1f)`; `Icon(ExpandLess if expanded else ExpandMore, contentDescription = null)`.
- Body: `AnimatedVisibility(visible = expanded) { content() }` (default M3/AnimatedVisibility fade plus expand).

Proposal: `gtk::Expander` with `expanded` default false. Keep the 24/12 padding and the header style; persist expansion only if a caller asks (none in this clone).

## 9. LabeledCheckbox (OBSERVED)

`LabeledCheckbox(label, checked, onCheckedChange, modifier, enabled = true)`:
- Row: `clip(MaterialTheme.shapes.small)` (8 dp), `fillMaxWidth`, `heightIn(min = 48.dp)`, `clickable(role = Role.Checkbox) { if (enabled) onCheckedChange(!checked) }`, `verticalAlignment = CenterVertically`, `spacedBy(MaterialTheme.padding.small)` = 8 dp.
- `Checkbox(checked, onCheckedChange = null, enabled)`; `Text(label)`.
- Note the clickable is always enabled; the guard is inside the lambda, so a disabled row still shows the ripple (OBSERVED quirk). Kurobayomi: make the row insensitive when disabled.

## 10. LazyColumnWithAction (OBSERVED)

- `Column(modifier.padding(contentPadding).fillMaxSize())`.
- `LazyColumn(modifier = Modifier.weight(1f), content)`.
- `HorizontalDivider()` (M3 default 1 dp).
- `Button(modifier = padding(horizontal = 16.dp, vertical = 8.dp).fillMaxWidth(), enabled = actionEnabled, onClick)`, `Text(actionLabel, color = colorScheme.onPrimary)`. The explicit `onPrimary` duplicates the Button's default content color.
- Callers: 4 (see `p1-inventory.md`).

## 11. Cross-area callees (part 2 and other areas)

| Symbol | Defined in | Behavior needed by part 1 | Owner |
|---|---|---|---|
| `Slider(value: Int, onValueChange: (Int) -> Unit, valueRange: IntProgression, steps, ...)` | `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Slider.kt:15-49` | Wraps M3 float slider; rounds float to Int with `roundToInt()`; `valueRange` converted to float range | area 49 part 2 |
| `Surface` (custom, with onClick/onLongClick) | `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Surface.kt` | Used by `Button` wrapper | area 49 part 2 |
| `animateElevation` | `presentation-core/src/main/java/tachiyomi/presentation/core/util/Elevation.kt` | Used by `ButtonElevation.animateElevation` | area 49 part 2 |
| `clickableNoIndication(onLongClick?, onClick)`, `showSoftKeyboard(show)`, `clearFocusOnSoftKeyboardHide(onFocusCleared)`, `secondaryItemAlpha()` (alpha 0.78) | `presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt:39-118` | `WheelPicker` manual input; `clickableNoIndication` in WheelPicker and others | area 49 part 2 |
| `Modifier.drawVerticalScrollbar(state, reverseScrolling, positionOffsetPx)` | `presentation-core/src/main/java/tachiyomi/presentation/core/util/Scrollbar.kt:91-96` | `ScrollbarLazyColumn` | area 49 part 2 |
| `Typography.header` | `presentation-core/src/main/java/tachiyomi/presentation/core/theme/Typography.kt:9-14` | Headings in SettingsItems, CollapsibleBox | area 49 part 2 |
| `Preference<T>.collectAsState()` | `presentation-core/src/main/java/tachiyomi/presentation/core/util/Preference.kt:10-13` | `CheckboxItem(label, pref)` | area 49 part 2 (uses `changes()` and `get()` from core/common Preference) |
| `stringResource(StringResource)` | `presentation-core/src/main/java/tachiyomi/presentation/core/i18n/Localize.kt` | every string in part 1 | area 49 part 2 |
| `TriState` enum (`DISABLED`, `ENABLED_IS`, `ENABLED_NOT`) and `Preference<T>.toggle()` | `core/common/src/main/java/tachiyomi/core/common/preference/` | `TriStateItem`, `CheckboxItem` | area 02-preferences-system |
| `UiPreferences.appTheme()`, `themeDarkAmoled()`, `themeMode()` | `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:20-31` | theme inputs | area 02 / area 47 |
| `DeviceUtil.isDynamicColorAvailable` | `app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt:6-8` | Monet default and picker filter. Value: `DynamicColors.isDynamicColorAvailable() || (DeviceUtil.isSamsung && SDK_INT >= 31)` (lazy) | area 36 / part 1 consumer |
| `ItemCover.Book.ratio`, `ItemCover` | `app/src/main/java/eu/kanade/presentation/entries/components/` | `AppThemePreviewItem` preview cover | area 38-39 |

## 12. Message keys used by this area (OBSERVED, English text)

Source files: `i18n/src/commonMain/moko-resources/base/strings.xml` (MR) and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (AYMR).

| Key | Module | English text | Used in |
|---|---|---|---|
| `label_default` | MR | Default | AppTheme.DEFAULT title |
| `theme_monet` | MR | Dynamic | AppTheme.MONET title |
| `theme_greenapple` | MR | Green Apple | GREEN_APPLE |
| `theme_midnightdusk` | MR | Midnight Dusk | MIDNIGHT_DUSK |
| `theme_nord` | MR | Nord | NORD |
| `theme_strawberrydaiquiri` | MR | Strawberry Daiquiri | STRAWBERRY_DAIQUIRI |
| `theme_tako` | MR | Tako | TAKO |
| `theme_tealturquoise` | MR | Teal &amp; Turquoise (XML escape; display "Teal & Turquoise") | TEALTURQUOISE |
| `theme_tidalwave` | MR | Tidal Wave | TIDAL_WAVE |
| `theme_yinyang` | MR | Yin &amp; Yang (display "Yin & Yang") | YINYANG |
| `theme_yotsuba` | MR | Yotsuba | YOTSUBA |
| `theme_monochrome` | MR | Monochrome | MONOCHROME |
| `theme_lavender` | MR | Lavender | LAVENDER |
| `theme_cloudflare` | AYMR | Cloudflare | CLOUDFLARE |
| `theme_cottoncandy` | AYMR | Cotton Candy | COTTONCANDY |
| `theme_doom` | AYMR | Doom | DOOM |
| `theme_matrix` | AYMR | Matrix | MATRIX |
| `theme_mocha` | AYMR | Mocha | MOCHA |
| `theme_sapphire` | AYMR | Sapphire | SAPPHIRE |
| `selected` | MR | Selected | content description of the check icon on the theme preview (`AppThemePreferenceWidget.kt`) |

No other user-visible strings are defined in this part. The text-free components take caller-provided strings.

## 13. Layout and responsiveness (OBSERVED)

- `TwoPanelBox(startContent, endContent, modifier, contentWindowInsets = WindowInsets(0))`:
  - `padding = contentWindowInsets.asPaddingValues()`; `startPadding = calculateStartPadding(direction)`; `endPadding = calculateEndPadding(direction)`.
  - Inside `BoxWithConstraints(modifier.fillMaxSize())`: `width = maxWidth - startPadding - endPadding`; `firstWidth = (width / 2).coerceAtMost(450.dp)`; `secondWidth = width - firstWidth`.
  - Start box `align(TopStart).width(firstWidth + startPadding)`; end box `align(TopEnd).width(secondWidth + endPadding)`.
  - Result: on wide windows the first panel is capped at 450 dp (plus start inset), the rest goes to the second panel.
- `FastScrollLazyVerticalGrid` and `VerticalGridFastScroller` require bounded width (`require(constraints.maxWidth != Constraints.Infinity) { "LazyVerticalGrid's width should be bound by parent" }`).
- `SettingsIconGrid` uses `GridCells.Adaptive(128.dp)`; `SettingsChipRow` uses `FlowRow`.

## 14. Fast scroll (OBSERVED, detail in `p1-topic-fast-scroll-and-wheel-picker.md`)

Summary of constants:
- Thumb length `ThumbLength = 48.dp`, thumb thickness `ThumbThickness = 12.dp`, shape `RoundedCornerShape(6 dp)`.
- Fade: `FadeOutAnimationSpec = tween(durationMillis = ViewConfiguration.getScrollBarFadeDuration(), delayMillis = 2000)`; `ImmediateFadeOutAnimationSpec = tween(durationMillis = ViewConfiguration.getScrollBarFadeDuration())`. `getScrollBarFadeDuration()` is the Android platform constant (250 ms default).
- Emission sample: `scrolled.sample(100)` (100 ms).
- Thumb visible when `alpha > 0`. Scroller shown only when `visibleItemsInfo.size < totalItemsCount`.
- `STICKY_HEADER_KEY_PREFIX = "sticky:"`. Sticky header items (key string starting with this prefix) are skipped when computing the scroll offset for `LazyListState`.

## 15. Wheel picker (OBSERVED, detail in the topic file)

- `ROW_COUNT = 3`. Default `size = DpSize(128.dp, 128.dp)`. Item height = `size.height / 3` = 42.67 dp. Visible rows = 3, centered item is the selected one.
- `contentPadding.vertical = size.height / 3 * ((3 - 1) / 2)` = one item height at top and bottom.
- `flingBehavior = rememberSnapFlingBehavior(lazyListState)`.
- Selected index changes are detected with `snapshotFlow { firstVisibleItemScrollOffset }.map { calculateSnappedItemIndex }.distinctUntilChanged().collectLatest { haptic(TextHandleMove); internalOnSelectionChanged(it) }`. Note: the first emission fires at composition, so `onSelectionChanged(startIndex-snapped)` runs once immediately.
- Manual input (only when `manualInputType != null`, i.e., `WheelNumberPicker`): tap the list -> a `BasicTextField` replaces it with `KeyboardType.Number`, `ImeAction.Done`, initial text `items[internalIndex].toString()`, cursor at end. On Done or keyboard hide (`clearFocusOnSoftKeyboardHide`), find the first item whose `toString()` equals the text. If found: `internalOnSelectionChanged(index)`, `scrollToItem(index)`. Always hide the field.
- Alpha animation: `calculateAnimatedAlpha(state, index)`: `distance = |distanceToSnapForIndex(index)|`; `viewport = viewportSize.height`; `single = viewport / 3`; if `distance in 0..single.toInt()` returns `1.2 - distance / single`, else `0.2`. Values above 1 at the center are passed to the alpha modifier (Compose clamps to [0, 1]; the clamp is library behavior, not verified here).

## 16. RepeatingIconButton (OBSERVED)

Parameters: `maxDelayMillis = 750`, `minDelayMillis = 5`, `delayDecayFactor = .25f`.

Gesture: `pointerInteropFilter`: `ACTION_DOWN` sets `pressed = true`; any other action (including `ACTION_MOVE`, `ACTION_UP`, `ACTION_CANCEL`) sets `pressed = false`. The filter returns `true` (consumes the event). The IconButton's own `onClick` is a no-op.

Loop (`LaunchedEffect(pressed, enabled)`): while `enabled && pressed`: call `currentClickListener()`, `delay(currentDelayMillis)`, then `currentDelayMillis = (current - current * 0.25f).toLong().coerceAtLeast(5)`.

Exact delay schedule (computed, Python float-to-long truncation): 750, 562, 421, 315, 236, 177, 132, 99, 74, 55, 41, 30, 22, 16, 12, 9, 6, 5, 5, 5, ... (first click happens at press time, before the first delay).

Note: a move of the finger off the button (ACTION_MOVE) stops the repeat (OBSERVED), it does not cancel the click already fired.

## 17. OutlinedNumericChooser (OBSERVED)

- `var currentValue = value` is a local, not remembered. The displayed value is `"%d".format(currentValue)`.
- `updateValue(isIncrement: Boolean)`: `currentValue += if (it) step else -step`; if `min != null` and `currentValue < min`, set `min`; then `onValueChanged(currentValue)`. There is no max.
- Typed text: `onValueChange`: `currentValue = it.trim().replace(Regex("[^-\\d.]"), "").toIntOrNull() ?: currentValue`; then `onValueChanged(currentValue)` (also when unchanged). The minimum is NOT applied to typed input (OBSERVED). Intermediate input such as "-" or "." reverts to the last valid integer on the next recomposition, because the field is driven by `currentValue`.
- Layout: `Row(verticalAlignment = Center)`: `RepeatingIconButton` with `Icons.Outlined.RemoveCircle`, `OutlinedTextField(value, modifier = widthIn(min = 140.dp), label, placeholder, suffix, singleLine = true, keyboardOptions = KeyboardType.Number)`, `RepeatingIconButton` with `Icons.Outlined.AddCircle`.

## 18. TriStateItem (OBSERVED)

- Click cycle: `DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED` (calls `onClick` with the next state; no-op when `onClick == null`).
- Row: `clickable(enabled = enabled && onClick != null)`, `fillMaxWidth`, `padding(horizontal = 24.dp, vertical = 10.dp)`, `spacedBy(MaterialTheme.padding.large)` = 24 dp.
- Icons: DISABLED = `CheckBoxOutlineBlank` (rounded), ENABLED_IS = `CheckBox` (rounded), ENABLED_NOT = `DisabledByDefault` (rounded).
- `stateAlpha = if (enabled && onClick != null) 1f else DISABLED_ALPHA (.38f)`.
- Icon tint: if `!enabled || state == DISABLED`: `onSurfaceVariant.copy(alpha = stateAlpha)`. Else when `onClick == null`: `onSurface.copy(alpha = DISABLED_ALPHA)`; otherwise `primary`.
- Label: `onSurface.copy(alpha = stateAlpha)`, `bodyMedium`.

## 19. Settings rows (OBSERVED)

Shared constants: `SettingsItemsPaddings.Horizontal = 24.dp`, `Vertical = 10.dp`.

| Widget | Layout | Interaction | Notes |
|---|---|---|---|
| `HeadingItem(text or labelRes)` | `Text` `header` style, `fillMaxWidth`, padding 24/10 | none | |
| `BaseSettingsItem` (private) | Row, `clickable(onClick)`, `fillMaxWidth`, padding 24/10, `spacedBy(24.dp)`, `CenterVertically`; widget then `Text(label, bodyMedium)` | whole row | Ripple from M3 `clickable` |
| `IconItem(label, icon, onClick)` | widget `Icon(icon, tint = primary)` | row | |
| `SortItem(label, sortDescending: Boolean?, onClick)` | arrow `ArrowDownward` for true, `ArrowUpward` for false, none for null | row | Delegates to `BaseSortItem` |
| `BaseSortItem(label, icon?, onClick)` | when icon null: `Spacer(size 24.dp)` (keeps alignment) | row | |
| `CheckboxItem(label, pref: Preference<Boolean>)` | `pref.collectAsState()`; click calls `pref.toggle()` | row | |
| `CheckboxItem(label, checked, onClick)` | `Checkbox(checked, onCheckedChange = null)` | row | |
| `RadioItem(label, selected, onClick)` | `RadioButton(selected, onClick = null)` | row | |
| `SliderItem(value, valueRange, label, onChange, steps = (last - first) - 1, valueText = value.toString(), labelStyle = bodyMedium, pillColor = surfaceContainerHigh)` | `BaseSliderItem` with padding 24/10 | slider | |
| `BaseSliderItem(...)` | Column `fillMaxWidth`, `spacedBy(2.dp)`; Row: label (`weight(1f)`), `Pill(valueText, style = bodyMedium, color = pillColor)`; then `Slider` | onValueChange: if `it == value` return; `onChange(it)`; haptic `TextHandleMove` | Haptic happens after `onChange` |
| `SelectItem(label, options: Array<out Any?>, selectedIndex, onSelect)` | `ExposedDropdownMenuBox` + `OutlinedTextField(readOnly, singleLine, label, value = options[selectedIndex].toString(), trailingIcon = TrailingIcon(expanded), colors = textFieldColors())`; `padding(24/10)`, `menuAnchor(PrimaryNotEditable)`, `fillMaxWidth` | menu items `DropdownMenuItem(text, contentPadding = ItemContentPadding)` | Crashes when `selectedIndex` out of range (OBSERVED; the generic overload has the same issue) |
| `SelectItem<T>(label, options: Array<T>, selectedIndex, modifier, onSelect, toString = { it.toString() })` | same as above, with `modifier` on the box and `DropdownMenuItem` without explicit content padding | | |
| `TriStateItem` | see section 18 | | |
| `RepeatingIconButton` | see section 16 | | |
| `OutlinedNumericChooser` | see section 17 | | |
| `TextItem(label, value, onChange)` | `OutlinedTextField(fillMaxWidth, padding(horizontal = 24.dp, vertical = 4.dp), label, singleLine = true)` | typing | |
| `SettingsChipRow(labelRes, content: FlowRowScope)` | `Column { HeadingItem; FlowRow(padding(start 24, top 0, end 24, bottom 10), spacedBy(8.dp)) }` | | |
| `SettingsIconGrid(labelRes, content: LazyGridScope)` | `Column { HeadingItem; LazyVerticalGrid(GridCells.Adaptive(128.dp), padding(start 24, end 24, bottom 10), vertical spacing 4 dp, horizontal spacing 8 dp) }` | | A `LazyVerticalGrid` inside a `Column` needs a bounded height from its parent; callers are dialogs (`OrientationSelectDialog.kt:69`, `ReadingModeSelectDialog.kt:67`). |

## 20. Buttons and elevation (OBSERVED)

- `ButtonDefaults.buttonColors(containerColor = primary, contentColor = onPrimary, disabledContainerColor = onSurface.copy(alpha = 0.12f), disabledContentColor = onSurface.copy(alpha = DISABLED_ALPHA))`.
- `ButtonDefaults.buttonElevation(defaultElevation = 0.dp, pressedElevation = 0.dp, focusedElevation = 0.dp, hoveredElevation = 1.dp, disabledElevation = 0.dp)`.
- Elevation target: disabled -> `disabledElevation`; else last interaction in a list (press, hover enter, focus) maps to pressed/hovered/focused, default `defaultElevation`. Animated through `animateElevation` (part 2 `Elevation.kt`).
- `Button` wrapper: `containerColor` and `contentColor` are `rememberUpdatedState` (no animation); `shadowElevation` from elevation state; `shape = M3ButtonDefaults.textShape` (full pill); `contentPadding = M3ButtonDefaults.ContentPadding` (library default); inside: `defaultMinSize(minWidth = M3ButtonDefaults.MinWidth, minHeight = M3ButtonDefaults.MinHeight)`, `padding(contentPadding)`, `Row(horizontalArrangement = Center, verticalAlignment = CenterVertically)`, `ProvideTextStyle(labelLarge)`.
- `TextButton` wrapper: colors `containerColor = Transparent`, `contentColor = primary`, `disabledContainerColor = Transparent`, `disabledContentColor = onSurface.copy(alpha = .38)`; `elevation = null` (so shadow 0); `contentPadding = M3ButtonDefaults.TextButtonContentPadding`.
- `ButtonElevation.tonalElevation(...)` is defined but never called inside this file (OBSERVED dead code).
- Library defaults used (not vendored): `M3ButtonDefaults.ContentPadding`, `TextButtonContentPadding`, `MinWidth`, `MinHeight`, `textShape`. Pin them when the Compose BOM version is known; Kurobayomi defines its own values (`p1-rust-design.md` section 5.4).

## 21. AlertDialogContent layout (OBSERVED)

Two overloads.

Overload A (`buttons`, `icon?`, `title?`, `text` required):
- Outer: `AlertDialogContent(modifier, icon, title, content = { ... })` (overload B).
- Body column: a `Column` where the text block is `CompositionLocalProvider(LocalContentColor = onSurfaceVariant) { ProvideTextStyle(bodyMedium) { Box(weight(1f, fill = false).padding(horizontal = 24.dp).padding(bottom = 24.dp).align(Start)) { text() } } }`.
- Buttons: `Box(padding(start = 24.dp, end = 24.dp, bottom = 24.dp).align(End))` with `LocalContentColor = primary` and `ProvideTextStyle(labelLarge)`.

Overload B (`modifier`, `icon?`, `title?`, `content: ColumnScope?`):
- Outer `Column(modifier.sizeIn(minWidth = 280.dp, maxWidth = 560.dp))`.
- If icon or title present: `Column(padding(start = 24, top = 24, end = 24).fillMaxWidth())`:
  - icon: `LocalContentColor = secondary`; `Box(padding(bottom = 16.dp).align(CenterHorizontally))`.
  - title: `LocalContentColor = onSurface`; `ProvideTextStyle(headlineSmall)`; `Box(padding(bottom = 16.dp).align(if icon == null then Start else CenterHorizontally))`.
- Then `content?.invoke(this)`.

Dialog frame (rounded corners, scrim, dismiss) is not in this file. The caller wraps it in an M3 `AlertDialog`/`Dialog` (other areas). Kurobayomi: `gtk::Window` modal with `.kuro-dialog` CSS, corner radius 28 px-equivalent.

## 22. ExtendedFloatingActionButton (OBSERVED)

- Wrapper around M3 `FloatingActionButton(shape = large (16 dp), containerColor = primaryContainer, contentColor = contentColorFor(containerColor), elevation = FloatingActionButtonDefaults.elevation())`.
- `minWidth` animated: `targetValue = if expanded 80 dp else 56 dp`, `tween(500 ms, EasingEmphasizedCubicBezier)`.
- `startPadding` animated: `targetValue = if expanded 12 dp else 0`, `tween(durationMillis = if expanded 300 else 900, EasingEmphasizedCubicBezier)`.
- Row: `sizeIn(minWidth = minWidth).padding(start = startPadding)`, `CenterVertically`, `Arrangement.Center`.
- `icon()` always shown.
- `AnimatedVisibility(visible = expanded, enter = ExpandAnimation, exit = CollapseAnimation) { Box(padding(start = 12.dp, end = 20.dp)) { text() } }`.
- Expand: `fadeIn(tween(200 ms, delay 100 ms, linear)) + expandHorizontally(tween(500 ms, emphasized), expandFrom = Start)`.
- Collapse: `fadeOut(tween(100 ms, linear)) + shrinkHorizontally(tween(500 ms, emphasized), shrinkTowards = Start)`.
- Easings: `EasingLinearCubicBezier = CubicBezierEasing(0,0,1,1)`, `EasingEmphasizedCubicBezier = CubicBezierEasing(0.2f, 0f, 0f, 1f)`.

Proposal (GTK): `gtk::Button` with `.kuro-fab` class; collapse by toggling a `gtk::Revealer` (`transition-type = SlideRight`, `transition-duration` 500 ms) around the label; width transitions via CSS `min-width` transition (500 ms cubic-bezier(0.2, 0, 0, 1)). Timings identical.

## 23. ActionButton, LinkIcon, ListGroupHeader, SectionCard, Pill, IconToggleButton (OBSERVED)

- `ActionButton(title, icon, onClick, modifier)`: `TextButton(modifier, onClick)` containing `Column(spacedBy(4.dp), CenterHorizontally) { Icon(icon, contentDescription = null); Text(title, textAlign = Center) }`.
- `LinkIcon(label, icon, url, modifier)`: `IconButton(modifier.padding(4.dp), onClick = { uriHandler.openUri(url) })`, `Icon(icon, tint = primary, contentDescription = label)`.
- `ListGroupHeader(text, modifier)`: `Text` with `padding(horizontal = 16.dp, vertical = 8.dp)`, `color = onSurfaceVariant`, `fontWeight = SemiBold`, `bodyMedium`.
- `SectionCard(titleRes?, content)` (LazyItemScope receiver): optional `Text(stringResource(titleRes), titleSmall, padding(horizontal = 32.dp))`; `ElevatedCard(fillMaxWidth, padding(horizontal = 16.dp, vertical = 8.dp), shape = extraLarge (28 dp))`; inner `Column(padding(16.dp))`.
- `Pill(text, modifier, color = surfaceContainerHigh, contentColor = onSurface, style = LocalTextStyle.current)`: `Surface(modifier.padding(start = 4.dp), shape = extraLarge, color, contentColor)` -> `Box(padding(horizontal = 6.dp, vertical = 1.dp), Center)` -> `Text(maxLines = 1, style)`.
  - Second overload `Pill(..., fontSize: TextUnit = LocalTextStyle.current.fontSize)` calls the first with `style = bodyMedium.copy(fontSize = fontSize)`. It declares `val style = LocalTextStyle.current` and never uses it (OBSERVED dead code).
- `IconToggleButton(checked, onCheckedChange, imageVector, title, modifier)`: `FilledIconToggleButton(modifier.height(48.dp))` -> `Row(spacedBy(8.dp), CenterVertically, fillMaxWidth, padding(8.dp))` -> `Icon(imageVector, contentDescription = null)` and `Text(title, maxLines = 2, overflow = Ellipsis)`.

## 24. Paging helper (OBSERVED)

`StateFlow<Flow<PagingData<T>>>.collectAsLazyPagingItems()` = `collectAsState()` then `collectAsLazyPagingItems()`. 6 callers. Drop on Linux; see `p1-data-and-config.md` section 7 for how Kurobayomi pages data.

## 25. Constants index

| Constant | Value | Location |
|---|---|---|
| `DISABLED_ALPHA` | 0.38f | `components/material/Constants.kt:9` |
| `SECONDARY_ALPHA` | 0.78f | `Constants.kt:10` |
| `Padding.extraLarge` | 32 dp | `Constants.kt:14` |
| `Padding.large` | 24 dp | `Constants.kt:16` |
| `Padding.medium` | 16 dp | `Constants.kt:18` |
| `Padding.mediumSmall` | 12 dp | `Constants.kt:20` |
| `Padding.small` | 8 dp | `Constants.kt:22` |
| `Padding.extraSmall` | 4 dp | `Constants.kt:24` |
| `IconButtonTokens.StateLayerSize` | 40 dp | `material/IconButtonTokens.kt:11` |
| `MediumWidthWindowSize` | 600 dp | `WindowSize.kt:22` |
| `ExpandedWidthWindowSize` | 840 dp | `WindowSize.kt:23` |
| Sheet maxWidth (landscape / portrait) | 600 / 460 dp | `AdaptiveSheet.kt:64-68` |
| Sheet animation | 350 ms tween | `AdaptiveSheet.kt:52` |
| Sheet positional threshold | 56 dp | `AdaptiveSheet.kt:122` |
| Sheet velocity threshold | 125 dp | `AdaptiveSheet.kt:123` |
| Thumb length / thickness | 48 dp / 12 dp | `VerticalFastScroller.kt:440-441` |
| Fast scroller fade delay | 2000 ms | `VerticalFastScroller.kt:445` |
| Fast scroller sample | 100 ms | `VerticalFastScroller.kt:137` |
| Wheel `ROW_COUNT` | 3 | `WheelPicker.kt:278` |
| Wheel default size | 128 x 128 dp | `WheelPicker.kt:62` |
| Repeat delays | 750 / 5 ms, decay .25 | `SettingsItems.kt:468-470` |
| Numeric chooser min width | 140 dp | `SettingsItems.kt:532` |
| Dialog min / max width | 280 / 560 dp | `AlertDialog.kt:139-140` |
| Dialog padding | 24 dp; icon/title bottom 16 dp; text bottom 24 dp | `AlertDialog.kt:134-137` |
| ExtendedFab minimum width (expanded) | 80 dp; collapsed 56 dp | `FloatingActionButton.kt:99,133` |
| ExtendedFab icon padding / text padding | 12 dp / 20 dp | `FloatingActionButton.kt:101-102` |
| ExtendedFab durations | 500 ms (width), 300/900 ms (start pad), expand 200 ms (+100 delay), collapse 100 ms, expand/collapse size 500 ms | `FloatingActionButton.kt:58-131` |
| Circular progress rotation | 2000 ms linear, restart | `CircularProgressIndicator.kt:60` |
| Ripple alphas | .1 each | `TachiyomiTheme.kt:88-91` |
| Surface container (AMOLED) | 0xFF0C0C0C / 0xFF131313 / 0xFF1B1B1B | `BaseColorScheme.kt:13-15` |

## 26. Edge cases observed in this area (summary; full list in `p1-edge-cases-and-tests.md`)

1. Unknown or deprecated `AppTheme` values silently map to the default scheme.
2. Schemes omit tokens (Section 1.2); the rendered color is the library default, not the scheme's intent.
3. Monet `tertiaryContainer` uses the `tertiary` role.
4. `SelectItem` crashes on an out-of-range `selectedIndex`.
5. `OutlinedNumericChooser` ignores `min` for typed input and cannot display an intermediate "-".
6. `RepeatingIconButton` stops on ACTION_MOVE.
7. `SettingsIconGrid` needs a bounded parent height.
8. `VerticalFastScroller.computeScrollOffset(LazyListState)` uses `!!` on the first non-sticky item; a list whose visible items are all sticky headers throws.
9. `WheelPicker` fires `onSelectionChanged` once at composition with the snapped index, and `startIndex` changes after the first composition are ignored (`rememberLazyListState(startIndex)`).
10. Player ripple color follows the system dark flag, not the app scheme.
