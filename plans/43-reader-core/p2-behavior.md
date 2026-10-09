# 43-reader-core (part 2 of 2) — Behavior spec

Repo root: `/home/blackbox/aniyomi-src`. All paths repo-relative. `OBSERVED` = what the Kotlin does (with file:line). `Proposal:` = what the Rust port should do. Area ids: `area:44-reader-viewers` (pager/webtoon viewers, navigation subclasses, page holders), `area:43-reader-core` part 1 (ReaderActivity, ReaderViewModel, ReaderOrientation, loader, model, presentation/reader), `area:02-preferences-system`, `area:21-backup-format-and-create`, `area:22-backup-restore`, `area:23-downloads-manga`, `area:34-images-covers-cache`, `area:51-i18n`, `area:52-util-app`.

Contents
1. Preference store contract (AndroidPreference semantics)
2. ReaderPreferences: key groups, semantics, ranges
3. Enum behaviors (FlashColor, TappingInvertMode, ReaderHideThreshold, ZoomStart, ImageScaleType, TapZones, ColorFilterMode)
4. ReadingMode: flags, resolution, viewer factory
5. ReaderSettingsScreenModel
6. ViewerConfig: binding semantics, field defaults, one-shot overlay
7. ViewerNavigation: regions, inversion, hit test
8. GestureDetectorWithLongTap: long-press state machine
9. ReaderPageImageView: page display state machine, zoom, pan, landscape auto-zoom, recycle
10. ReaderProgressIndicator
11. ReaderTransitionView and calculateChapterGap
12. ReaderButton
13. Viewer interface contract
14. E-ink flash (DisplayRefreshHost) as consumed from ReaderPreferences
15. Message keys (MR.strings) used in scope
16. Side effects, dispatchers, timings, limits summary

---

## 1. Preference store contract

OBSERVED
- `ReaderPreferences` only holds a `PreferenceStore` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:10-12`). Each getter returns `Preference<T>` created by `preferenceStore.getBoolean/getInt/getEnum(key, default)`.
- `getEnum` (`core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt`) stores `enum.name` as a string (`serializer = { it.name }`). Deserialization uses `enumValueOf(it)`; on `IllegalArgumentException` it returns the default. Unknown names therefore fall back silently.
- `AndroidPreferenceStore.getObject` → `AndroidPreference.Object`: `read` = `preferences.getString(key, null)?.let(deserializer) ?: defaultValue`, wrapped in `try { } catch (Exception) { defaultValue }`. `write` = `putString(key, serializer(value))`.
- `AndroidPreference.get()` (`core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreference.kt:32-41`): `read(...)`; on `ClassCastException` it logs `"Invalid value for $key; deleting"`, calls `delete()`, returns defaultValue. This matters when a key was written with another type.
- `set(value)` → `preferences.edit(write(key,value))`. `delete()` → remove key. `isSet()` → `contains(key)`.
- `changes()` (`AndroidPreference.kt:60-66`): `keyFlow.filter { it == key || it == null }.onStart { emit("ignition") }.map { get() }.conflate()`. A collector gets the current value immediately on subscription, then each new value (conflated, so only the latest value is delivered when producers are faster than the consumer).
- `stateIn(scope)` (`AndroidPreference.kt:68-70`): `changes().stateIn(scope, SharingStarted.Eagerly, get())`.

Proposal (Rust): `ReaderPrefs` backed by a `kv` table (see p2-data-and-config.md). `get` returns the default when the row is missing or unparseable and deletes the row when the stored type is wrong (mirrors `ClassCastException` path). Changes are published on a `glib`-side broadcast (see p2-rust-design.md §Settings change bus), and a new subscriber immediately receives the current value (mirrors `onStart`).

## 2. ReaderPreferences: key groups

Full table with keys, types, defaults and consumers is in `p2-data-and-config.md` §2. Behavioral notes per group:

### 2.1 General (`ReaderPreferences.kt:14-91`)
- `pageTransitions` (`pref_enable_transitions_key`, default true): when on, `ViewerConfig.usePageTransitions` = true (see §6). Consumers in area 44.
- Flash group `flashOnPageChange`, `flashDurationMillis`, `flashPageInterval`, `flashColor`: e-ink flash, see §14.
- `doubleTapAnimSpeed` (`pref_double_tap_anim_speed`, default 500, unit ms): zoom duration for double tap, see §9.5.
- `showPageNumber` (`pref_show_page_number_key`, default true), `showReadingMode` (`pref_show_reading_mode`, default true): UI-only toggles.
- `fullscreen` (`fullscreen`, default true), `cutoutShort` (`cutout_short`, default true), `keepScreenOn` (`pref_keep_screen_on_key`, default true): window behavior. `cutoutShort` is only shown when `screenModel.hasDisplayCutout && fullscreen().get()` (`app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt` around line 68-72).
- `defaultReadingMode` (`pref_default_reading_mode_key`, default `ReadingMode.RIGHT_TO_LEFT.flagValue` = 2): the global default used when a manga has DEFAULT reading mode (resolution is in part 1 ReaderViewModel).
- `defaultOrientationType` (`pref_default_orientation_type_key`, default `ReaderOrientation.FREE.flagValue` = 0x8 = 8): global default orientation (ReaderOrientation is part 1; flag value verified in `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt` lines 14-40, FREE = `0x00000008`).
- `webtoonDoubleTapZoomEnabled` (`pref_enable_double_tap_zoom_webtoon`, default true): webtoon double-tap zoom.
- `imageScaleType` (`pref_image_scale_type_key`, default 1), `zoomStart` (`pref_zoom_start_key`, default 1), `readerTheme` (`pref_reader_theme_key`, default 1), see §3.
- `alwaysShowChapterTransition` (`always_show_chapter_transition`, default true): transition page shown between chapters even when the next chapter is already loaded.
- `preserveReadingPosition` (`pref_preserve_reading_position`, default false): consumer in part 1 ReaderViewModel.
- `cropBorders` (`crop_borders`, default false), `navigateToPan` (`navigate_pan`, default true), `landscapeZoom` (`landscape_zoom`, default true), `cropBordersWebtoon` (`crop_borders_webtoon`, default false): image options; `navigateToPan` and `landscapeZoom` are only enabled for scale type 1 in the settings UI (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt:268`: `enabled = imageScaleType == 1`).
- `webtoonSidePadding` (`webtoon_side_padding`, default `WEBTOON_PADDING_MIN` = 0), range 0..25 (`WEBTOON_PADDING_MAX` = 25). UI shows `NumberFormat.getPercentInstance().format(value / 100f)`.
- `readerHideThreshold` (`reader_hide_threshold`, default LOW), see §3.4 and §2.3.
- `folderPerManga` (`create_folder_per_manga`, default false): download folder option, consumed in area 23.
- `skipRead` (`skip_read`, default false), `skipFiltered` (`skip_filtered`, default true), `skipDupe` (`skip_dupe`, default false): chapter-skipping rules, consumed in part 1 ReaderViewModel (`ReaderViewModel.kt:162` checks `skipRead().get() || skipFiltered().get()`) and in `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt:148, 654`.
- `webtoonDisableZoomOut` (`webtoon_disable_zoom_out`, default false).

### 2.2 Split two page spread (`ReaderPreferences.kt:93-120`)
Eight booleans, all default false: `pref_dual_page_split`, `pref_dual_page_invert`, `pref_dual_page_split_webtoon`, `pref_dual_page_invert_webtoon`, `pref_dual_page_rotate`, `pref_dual_page_rotate_invert`, `pref_dual_page_rotate_webtoon`, `pref_dual_page_rotate_invert_webtoon`. Paged and webtoon variants are separate. Consumers: `ViewerConfig` fields (§6) and the settings UI. Visual behavior is area 44.

Settings UI coupling (verified): enabling "split" resets "rotate" (`SettingsReaderScreen.kt:293`, `388`: `dualPageSplitPref.set(false)` when rotate turned on); `dualPageInvert*` enabled only when split is on (`enabled = dualPageSplit`).

### 2.3 Color filter (`ReaderPreferences.kt:122-138`)
- `customBrightness` (`pref_custom_brightness_key`, false), `customBrightnessValue` (`custom_brightness_value`, int, default 0). Documented range in `ColorFilterPage.kt:` KDoc: "Range is [-75, 100]. From -75 to -1 a semi-transparent black view is shown at the top with the minimum brightness. From 1 to 100 it sets that value as brightness. 0 sets system brightness and hides the overlay."
- `colorFilter` (`pref_color_filter_key`, false), `colorFilterValue` (`color_filter_value`, int, default 0): ARGB overlay color value (stored int; the settings color picker sets it, outside scope).
- `colorFilterMode` (`color_filter_mode`, int, default 0): index into `ColorFilterMode` list (§3.7). `ColorFilterPage.kt:110` sets `colorFilterMode().set(index)`.
- `grayscale` (`pref_grayscale`, false), `invertedColors` (`pref_inverted_colors`, false).

### 2.4 Controls (`ReaderPreferences.kt:140-175`)
- `readWithLongTap` (`reader_long_tap`, default true): enables the long-press action (§8, §5 of part-2 ViewerConfig).
- `readWithVolumeKeys` (`reader_volume_keys`, false), `readWithVolumeKeysInverted` (`reader_volume_keys_inverted`, false).
- `navigationModePager` (`reader_navigation_mode_pager`, int, default 0), `navigationModeWebtoon` (`reader_navigation_mode_webtoon`, int, default 0): tap zone layout index (see §3.6 and topic file).
- `pagerNavInverted` (`reader_tapping_inverted`, enum, default NONE), `webtoonNavInverted` (`reader_tapping_inverted_webtoon`, enum, default NONE).
- `showNavigationOverlayNewUser` (`reader_navigation_overlay_new_user`, default true): one-shot, see §6.3.
- `showNavigationOverlayOnStart` (`reader_navigation_overlay_on_start`, default false).

## 3. Enum behaviors

### 3.1 FlashColor (`ReaderPreferences.kt:177-181`)
Values `BLACK`, `WHITE`, `WHITE_BLACK`. Stored by name. Labels: `pref_flash_style_black` ("Black"), `pref_flash_style_white` ("White"), `pref_flash_style_white_black` ("White and Black") (settings list in `GeneralSettingsPage.kt` and `SettingsReaderScreen.kt:163-170`).

### 3.2 TappingInvertMode (`ReaderPreferences.kt:183-196`)
| Constant | titleRes | shouldInvertHorizontal | shouldInvertVertical |
|---|---|---|---|
| NONE | tapping_inverted_none ("None") | false | false |
| HORIZONTAL | tapping_inverted_horizontal ("Horizontal") | true | false |
| VERTICAL | tapping_inverted_vertical ("Vertical") | false | true |
| BOTH | tapping_inverted_both ("Both") | true | true |

The webtoon settings list (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt`, `getWebtoonGroup`) offers exactly NONE, HORIZONTAL, VERTICAL, BOTH; the paged list uses the same four values (`pagerNavInverted`).

### 3.3 ReaderHideThreshold (`ReaderPreferences.kt:198-203`)
| Constant | threshold (px) | label key |
|---|---|---|
| HIGHEST | 5 | pref_highest ("Highest") |
| HIGH | 13 | pref_high ("High") |
| LOW | 31 (default) | pref_low ("Low") |
| LOWEST | 47 | pref_lowest ("Lowest") |

Used only by the webtoon viewer (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt:75-79`: `private val threshold: Int = Injekt.get<ReaderPreferences>().readerHideThreshold().get().threshold`, read once at viewer construction). Scroll rule: in `onScrolled(dy)` (line 94), if `(dy > threshold || dy < -threshold) && menuVisible` → `hideMenu()`. The threshold is therefore read once per viewer instance; changing it takes effect after the viewer is recreated.

### 3.4 Tap zone list `TapZones` (`ReaderPreferences.kt:211-218`)
Index → label key: 0 `label_default` ("Default"), 1 `l_nav` ("L shaped"), 2 `kindlish_nav` ("Kindle-ish"), 3 `edge_nav` ("Edge"), 4 `right_and_left_nav` ("Right and Left"), 5 `disabled_nav` ("Disabled"). Stored value = index. Mapped to navigators in §7 and area 44 (`PagerConfig.kt:140-150`, `WebtoonConfig.kt:110-120`): 0 → defaultNavigation(), 1 → LNavigation, 2 → KindlishNavigation, 3 → EdgeNavigation, 4 → RightAndLeftNavigation, 5 → DisabledNavigation, else → defaultNavigation().

### 3.5 ImageScaleType `ImageScaleType` (`ReaderPreferences.kt:220-227`)
UI list index i stores value `i + 1` (`ReadingModePage.kt:83-88`: `selected = imageScaleType == index + 1`, `set(index + 1)`).
| Stored value | Label key | Label | SSIV constant the value is passed as (verified in fork `SubsamplingScaleImageView.java` at tachiyomiorg `66e0db195d`) |
|---|---|---|---|
| 1 (default) | scale_type_fit_screen | Fit screen | SCALE_TYPE_CENTER_INSIDE = 1 |
| 2 | scale_type_stretch | Stretch | SCALE_TYPE_CENTER_CROP = 2 (label/behavior mismatch, see §16 quirks) |
| 3 | scale_type_fit_width | Fit width | SCALE_TYPE_FIT_WIDTH = 3 |
| 4 | scale_type_fit_height | Fit height | SCALE_TYPE_FIT_HEIGHT = 4 |
| 5 | scale_type_original_size | Original size | SCALE_TYPE_ORIGINAL_SIZE = 5 |
| 6 | scale_type_smart_fit | Smart fit | SCALE_TYPE_SMART_FIT = 6 |

Pager passes the stored integer straight through: `PagerConfig.imageScaleType` (default 1, `PagerConfig.kt:36`) is set by `register({ imageScaleType = it })` (`PagerConfig.kt:61-62`) and used as `minimumScaleType` (`PagerPageHolder.kt:166`). The fork's `setMinimumScaleType` throws `IllegalArgumentException("Invalid scale type: …")` for values outside `{1,2,3,4,5,6,7}` (SCALE_TYPE_CUSTOM=7 is valid for the fork list `VALID_SCALE_TYPES`, which also omits SCALE_TYPE_START). Stored values outside 1..6 cannot come from the UI.

Webtoon ignores the preference and always uses `SCALE_TYPE_FIT_WIDTH` (`WebtoonPageHolder.kt:201`).

### 3.6 ZoomStart `ZoomStart` (`ReaderPreferences.kt:229-234`)
UI index i stores `i + 1` (`ReadingModePage.kt:94-99`). Values: 1 Automatic (`zoom_start_automatic`), 2 Left (`zoom_start_left`), 3 Right (`zoom_start_right`), 4 Center (`zoom_start_center`). Default 1.
Pager mapping (`PagerConfig.kt:111-125`, `zoomTypeFromPreference`):
- 1 → if viewer is `L2RPagerViewer` → `LEFT`; if `R2LPagerViewer` → `RIGHT`; otherwise → `CENTER`.
- 2 → `LEFT`; 3 → `RIGHT`; any other value (including 4) → `CENTER`.
Note: the `when` uses `else` for CENTER, so values outside 1..4 map to CENTER.

### 3.7 ColorFilterMode `ColorFilterMode` (`ReaderPreferences.kt:236-253`)
Built as list of `(label, BlendMode)` pairs in this order:
- index 0: `label_default` ("Default") → `BlendMode.SrcOver`
- index 1: `filter_mode_multiply` ("Multiply") → `BlendMode.Modulate`
- index 2: `filter_mode_screen` ("Screen") → `BlendMode.Screen`
- if `Build.VERSION.SDK_INT >= Build.VERSION_CODES.P` (API 28) additionally:
  - index 3: `filter_mode_overlay` ("Overlay") → `BlendMode.Overlay`
  - index 4: `filter_mode_lighten` ("Dodge / Lighten") → `BlendMode.Lighten`
  - index 5: `filter_mode_darken` ("Burn / Darken") → `BlendMode.Darken`
Proposal (Linux): always the six entries (no API gate). Map to GSK/cairo blend modes (§p2-rust-design).

### 3.8 ImageScaleType/ZoomStart/TapZones/ColorFilterMode are UI lists, not persisted enums
Their stored form is the integer index or value above, not the label.

## 4. ReadingMode

### 4.1 Entries (`ReadingMode.kt:307-342`)
| Name | stringRes | iconRes | flagValue (hex) | direction | type |
|---|---|---|---|---|---|
| DEFAULT | label_default | ic_reader_default_24dp | 0x00000000 | null | null |
| LEFT_TO_RIGHT | left_to_right_viewer | ic_reader_ltr_24dp | 0x00000001 | Horizontal | Pager |
| RIGHT_TO_LEFT | right_to_left_viewer | ic_reader_rtl_24dp | 0x00000002 | Horizontal | Pager |
| VERTICAL | vertical_viewer | ic_reader_vertical_24dp | 0x00000003 | Vertical | Pager |
| WEBTOON | webtoon_viewer | ic_reader_webtoon_24dp | 0x00000004 | Vertical | Webtoon |
| CONTINUOUS_VERTICAL | vertical_plus_viewer | ic_reader_continuous_vertical_24dp | 0x00000005 | Vertical | Webtoon |

Icons are Android drawables (`app/src/main/res/drawable*` family) → replaced by named icons (see p2-ui-parity).

### 4.2 Flag layout
- `ReadingMode.MASK = 0x00000007` (`ReadingMode.kt:346`).
- Stored on `manga.viewerFlags` bits 0..2: `Manga.readingMode: Long get() = viewerFlags and ReadingMode.MASK.toLong()` (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:17-18`).
- Writes go through `SetMangaViewerFlags` with `viewerFlags.setFlag(flag, ReadingMode.MASK.toLong())` (`app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetMangaViewerFlags.kt:17`), i.e. clear the masked bits then OR in the new value.
- Reader orientation uses bits 3..5 (`ReaderOrientation.MASK = 0x00000038`, part 1).

### 4.3 Resolution and factory
- `fromPreference(preference: Int?)` (`ReadingMode.kt:348`): `entries.find { it.flagValue == preference } ?: DEFAULT`. `null` and unknown values → DEFAULT.
- `isPagerType(preference: Int)` (`ReadingMode.kt:350-353`): true only when the resolved mode's `type is ViewerType.Pager`. DEFAULT → false. So `isPagerType(0)` is false, `isPagerType(1..3)` true, `isPagerType(4..5)` false.
- `toViewer(preference: Int?, activity: ReaderActivity): Viewer` (`ReadingMode.kt:355-366`): the `when` must be exhaustive over resolved values. DEFAULT throws `IllegalStateException("Preference value must be resolved: $preference")`. So the caller must replace DEFAULT with `defaultReadingMode` (part 1) before calling. Mapping: LEFT_TO_RIGHT → `L2RPagerViewer(activity)`, RIGHT_TO_LEFT → `R2LPagerViewer(activity)`, VERTICAL → `VerticalPagerViewer(activity)`, WEBTOON → `WebtoonViewer(activity)`, CONTINUOUS_VERTICAL → `WebtoonViewer(activity, isContinuous = false)`.

Important: `CONTINUOUS_VERTICAL` uses the webtoon viewer with `isContinuous = false` (i.e. chapter gaps shown; the label is "Long strip with gaps"). `WEBTOON` calls the same constructor without the `isContinuous` argument; its default is defined in area 44 (`WebtoonViewer.kt`), not verified here.

## 5. ReaderSettingsScreenModel (`ReaderSettingsScreenModel.kt:269-286`)

- Constructor: `readerState: StateFlow<ReaderViewModel.State>`, `hasDisplayCutout: Boolean`, `onChangeReadingMode: (ReadingMode) -> Unit`, `onChangeOrientation: (ReaderOrientation) -> Unit`, `preferences: ReaderPreferences = Injekt.get()`.
- `viewerFlow`: `readerState.map { it.viewer }.distinctUntilChanged().stateIn(ioCoroutineScope, SharingStarted.Lazily, null)`.
- `mangaFlow`: `readerState.map { it.manga }.distinctUntilChanged().stateIn(ioCoroutineScope, SharingStarted.Lazily, null)`.
- `SharingStarted.Lazily`: upstream starts on first subscriber and is never stopped. The scope is `ioCoroutineScope` from `eu.kanade.presentation.util`.
- Behavior of the settings UI (observed): the "For this series" section shows chips for all six `ReadingMode.entries` (`ReadingModePage.kt:28-40`), selected = `ReadingMode.fromPreference(manga?.readingMode?.toInt())`; clicking calls `onChangeReadingMode(it)`. Then the page shows `WebtoonViewerSettings` if the current viewer is `WebtoonViewer`, else `PagerViewerSettings` (`ReadingModePage.kt:42-45`).
- `ReadingModeSelectDialog` offers "use default" only when the current mode is not DEFAULT (`ReadingModeSelectDialog.kt:64`).

Proposal: the Rust settings model is a struct holding `Rc<RefCell<Option<Manga>>>`-style state plus `glib` signals; it does not need `Lazily` semantics (state is read directly from the GTK main loop).

## 6. ViewerConfig (`ViewerConfig.kt:13-90`)

### 6.1 Field defaults at construction (before any preference emits)
| Field | Initial value | Preference | Preference default |
|---|---|---|---|
| imagePropertyChangedListener | null | — | — |
| navigationModeChangedListener | null | — | — |
| tappingInverted | NONE | pagerNavInverted / webtoonNavInverted (subclass) | NONE |
| longTapEnabled | true | readWithLongTap | true |
| usePageTransitions | **false** | pageTransitions | true |
| doubleTapAnimDuration | 500 | doubleTapAnimSpeed | 500 |
| volumeKeysEnabled | false | readWithVolumeKeys | false |
| volumeKeysInverted | false | readWithVolumeKeysInverted | false |
| alwaysShowChapterTransition | true | alwaysShowChapterTransition | true |
| navigationMode | 0 (protected set) | navigationModePager / navigationModeWebtoon (subclass) | 0 |
| forceNavigationOverlay | false | showNavigationOverlayNewUser (one-shot) | true |
| navigationOverlayOnStart | false | showNavigationOverlayOnStart | false |
| dualPageSplit, dualPageInvert, dualPageRotateToFit, dualPageRotateToFitInvert | false (protected set) | dualPage* (subclass) | false |
| navigator | abstract (`defaultNavigation()` in subclass) | — | — |

Note `usePageTransitions` starts false, while its preference defaults true. The true value arrives asynchronously on the first emission (§6.2), so the first frame after construction uses false.

### 6.2 `register` semantics (`ViewerConfig.kt:80-89`)
```
changes()
  .onEach { valueAssignment(it) }
  .distinctUntilChanged()
  .onEach { onChanged(it) }
  .launchIn(scope)
```
- Runs in the viewer's `scope` (a coroutine scope supplied by the viewer).
- On launch the flow emits the current value once (`onStart` ignition, §1): `valueAssignment(current)` runs, then `onChanged(current)` runs (`distinctUntilChanged` passes the first value). So at construction, every `onChanged` callback fires once with the stored value. Listeners such as `imagePropertyChangedListener` may be null then (no-op).
- Afterwards, `valueAssignment` runs for every new value; `onChanged` runs only when the value differs from the previous one (`distinctUntilChanged` sits between the two `onEach`).
- Because `changes()` conflates, rapid successive changes can collapse to the last value.

Proposal: `ViewerConfig` becomes a plain struct of fields plus a `subscribe` closure per key that is invoked immediately and then on change (equivalent semantics; no coroutines).

### 6.3 Navigation overlay one-shot (`ViewerConfig.kt:67-70`)
```
forceNavigationOverlay = readerPreferences.showNavigationOverlayNewUser().get()
if (forceNavigationOverlay) readerPreferences.showNavigationOverlayNewUser().set(false)
```
Observed: the first viewer constructed after install reads true (default) and the preference is written false. Subsequent viewers see `forceNavigationOverlay == false`. The flag is a plain field read by the viewer when it sets up the overlay (`PagerViewer.kt:144`, `WebtoonViewer.kt:162`: `activity.binding.navigationOverlay.setNavigation(config.navigator, showOnStart)`).
`navigationOverlayOnStart` registers normally (`ViewerConfig.kt:72-73`).

Proposal: `reader_navigation_overlay_new_user` true → show the tap-zone overlay once, then persist false. Both `forceNavigationOverlay` and `navigationOverlayOnStart` feed `showOnStart` (the consumer logic is area 44).

### 6.4 Abstract navigation hooks
- `protected abstract fun defaultNavigation(): ViewerNavigation`.
- `abstract fun updateNavigation(navigationMode: Int)`: subclasses assign `navigator` from `navigationMode` (§3.4 mapping) and then call `navigationModeChangedListener?.invoke()`.
- `abstract var navigator: ViewerNavigation` (protected set). Setters in subclasses also assign `invertMode` from `tappingInverted` (`PagerConfig.kt` navigator setter).

## 7. ViewerNavigation (`ViewerNavigation.kt:101-158`)

### 7.1 Regions (`NavigationRegion`, lines 103-124)
Sealed class with `nameRes: StringResource` and `color: Int` (ARGB via `Color.argb`, alpha always 0xCC = 204):
| Object | nameRes | color (ARGB components) |
|---|---|---|
| MENU | action_menu ("Menu") | A=0xCC R=0x95 G=0x81 B=0x8D |
| PREV | nav_zone_prev ("Prev") | A=0xCC R=0xFF G=0x77 B=0x33 |
| NEXT | nav_zone_next ("Next") | A=0xCC R=0x84 G=0xE2 B=0x96 |
| LEFT | nav_zone_left ("Left") | A=0xCC R=0x7D G=0x11 B=0x28 |
| RIGHT | nav_zone_right ("Right") | A=0xCC R=0xA6 G=0xCF B=0xD5 |

Colors are used by the navigation overlay drawing (part of ReaderNavigationOverlayView, area 44).

### 7.2 Region (`ViewerNavigation.kt:126-136`)
`data class Region(val rectF: RectF, val type: NavigationRegion)` with `invert(invertMode)`: if `invertMode == NONE` return this; else `copy(rectF = rectF.invert(invertMode))`.

### 7.3 Inversion (`app/src/main/java/eu/kanade/tachiyomi/util/lang/RectFExtensions.kt:6-18`)
Rect coordinates are normalized (regions lie in [0,1]; e.g. `constantMenuRegion = RectF(0f, 0f, 1f, 0.05f)`). Rules:
- both horizontal and vertical → `RectF(1 - right, 1 - bottom, 1 - left, 1 - top)`.
- vertical only → `RectF(left, 1 - bottom, right, 1 - top)`.
- horizontal only → `RectF(1 - right, top, 1 - left, bottom)`.
- otherwise → unchanged.
Only the rectangle moves; the region's `type` is unchanged. So an inverted left-zone button still reads "Left" in the overlay, but now sits on the right.

### 7.4 Hit test `getAction(pos: PointF): NavigationRegion` (`ViewerNavigation.kt:149-158`)
1. `regions = getRegions()` = `regionList.map { it.invert(invertMode) }` (computed on each call).
2. `region = regions.find { it.rectF.contains(x, y) }` (first match in list order; `RectF.contains(x, y)` in Android is half-open: `left < right && top < bottom && x >= left && x < right && y >= top && y < bottom`).
3. If region != null → `region.type`.
4. Else → `NavigationRegion.MENU` in both branches (`constantMenuRegion.contains` is evaluated but both branches return MENU, so it has no effect).

Quirk: `constantMenuRegion` is declared as a 0..0.05 top strip but never changes the result. Proposal: drop the dead branch and keep "unmatched → MENU".

`protected abstract var regionList: List<Region>` is provided by the subclasses in area 44 (`navigation/*.kt`). Their geometry is not part of this file.

`invertMode` is a public var set by the owning viewer config.

## 8. GestureDetectorWithLongTap (`GestureDetectorWithLongTap.kt:15-75`)

### 8.1 Constants
- Long-tap delay = `ViewConfiguration.getLongPressTimeout()`: platform value, read once at construction (user-adjustable in Android settings; not a constant in this code).
- Double-tap window = `ViewConfiguration.getDoubleTapTimeout()`: platform value, read once at construction.
- Movement slop = `ViewConfiguration.get(context).scaledTouchSlop`: platform value in pixels, read once at construction.
- Proposal (Linux): read `gtk-long-press-time` (GTK setting, 500 ms default) for the long-press delay, `gtk-double-click-time` for the double-tap window, and a fixed 8 px slop. Final values are fixed in T-43b-012.

### 8.2 Event rules (`onTouchEvent`, lines 35-63)
| actionMasked | Action |
|---|---|
| ACTION_DOWN | `lastDownEvent?.recycle()`; `lastDownEvent = MotionEvent.obtain(ev)`. If `ev.downTime - lastUp > doubleTapTime` → `downX = ev.x`, `downY = ev.y`, schedule `longTapFn` after `longTapTime`. Otherwise (second tap of a double tap) nothing is scheduled. |
| ACTION_MOVE | If `abs(ev.x - downX) > slop || abs(ev.y - downY) > slop` → cancel `longTapFn`. |
| ACTION_UP | `lastUp = ev.eventTime`; cancel `longTapFn`. |
| ACTION_CANCEL, ACTION_POINTER_DOWN | cancel `longTapFn`. |
Then always `return super.onTouchEvent(ev)` so built-in tap, double-tap, scroll and fling detection proceed normally.

`longTapFn` = `Runnable { listener.onLongTapConfirmed(lastDownEvent!!) }`. The stored down event is the one that scheduled the timer.

Observed details:
- `lastUp` starts at 0, so the first press of the session is never blocked by the double-tap guard unless `downTime` is within 300 ms of 0 (not a practical case).
- When the double-tap guard blocks a down, `downX/downY` are not refreshed, but no timer is scheduled either; a later MOVE may still cancel nothing. No functional effect.
- Long press does not fire after the finger has moved beyond slop, and never fires for the second tap of a double tap.
- The built-in detector's own `onLongPress` is not overridden: `Listener` extends `SimpleOnGestureListener` and only adds `onLongTapConfirmed` (`GestureDetectorWithLongTap.kt:68-74`). The pager listener overrides `onLongTapConfirmed` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/Pager.kt:39`, forwarding to `longTapListener` at line 40, which is a `(MotionEvent) -> Boolean` set by the activity).
- Long tap is disabled by `longTapEnabled == false` at the consumer (area 44 `Pager.kt`); this class does not check the setting.

Note on units: `ev.downTime` and `ev.eventTime` are the same uptime-millisecond clock, so `ev.downTime - lastUp` is in ms.

### 8.3 Listener contract
`open class Listener : SimpleOnGestureListener()` with `open fun onLongTapConfirmed(ev: MotionEvent)` (no-op default). `ev` is the initial DOWN event copy.

## 9. ReaderPageImageView (`ReaderPageImageView.kt:55-442`)

This is one page's view. Two rendering back ends:
- **SSIV path** (non-animated): `SubsamplingScaleImageView` (`com.davemorrissey.labs.subscaleview`, tachiyomi fork), or `WebtoonSubsamplingImageView` when `isWebtoon`.
- **Animated path**: `PhotoView` (`com.github.chrisbanes.photoview`, v2.3.0 per `gradle/libs.versions.toml:61`), or `AppCompatImageView` when `isWebtoon`.

### 9.1 Construction and callbacks
- Constructor: `(context, attrs = null, defStyleAttrs = 0, defStyleRes = 0, isWebtoon = false)`. `FrameLayout` subclass.
- `alwaysDecodeLongStripWithSSIV` = `lazy { Injekt.get<BasePreferences>().alwaysDecodeLongStripWithSSIV().get() }` (read once per view; pref `pref_always_decode_long_strip_with_ssiv`, default false; `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:42`).
- Public callbacks (each a nullable lambda plus an open method that invokes it with `@CallSuper`):
  - `onImageLoaded` → sets `background = pageBackground` after invoking the lambda (`ReaderPageImageView.kt:82-85`).
  - `onImageLoadError` → invokes lambda.
  - `onScaleChanged(newScale: Float)` → invokes lambda with the new scale.
  - `onViewClicked()` → invokes lambda (single tap on the page).
- `pageBackground: Drawable?` is applied only when the image loads.

### 9.2 Entry points
- `setImage(drawable: Drawable, config: Config)` (line 163): stores `config`. If `drawable is Animatable` → `prepareAnimatedImageView()` + `setAnimatedImage(drawable, config)`; else `prepareNonAnimatedImageView()` + `setNonAnimatedImage(drawable, config)`.
- `setImage(source: BufferedSource, isAnimated: Boolean, config: Config)` (line 174): same split, using the source.
- `recycle()` (line 185): SSIV → `recycle()`; `AppCompatImageView` (includes PhotoView) → Coil `dispose()`; then `isVisible = false`. The view instance is kept; the next `setImage` reuses it if the type matches.

### 9.3 Non-animated loading (`setNonAnimatedImage`, lines 289-350)
Settings applied to the SSIV before loading: `setDoubleTapZoomDuration(config.zoomDuration.getSystemScaledDuration())`, `setMinimumScaleType(config.minimumScaleType)`, `setMinimumDpi(1)` (lets very small images fit on first load), `setCropBorders(config.cropBorders)` (fork method: crop non-filled border content), and an `OnImageEventListener` whose:
- `onReady()`: `setupZoom(config)`; `if (isVisibleOnScreen()) landscapeZoom(true)`; `this@ReaderPageImageView.onImageLoaded()`.
- `onImageLoadError(e)`: `this@ReaderPageImageView.onImageLoadError()`.

Then by data type:
- `BitmapDrawable` → `setImage(ImageSource.bitmap(data.bitmap))`, `isVisible = true`.
- `BufferedSource`:
  - if `!isWebtoon || alwaysDecodeLongStripWithSSIV` → `setHardwareConfig(ImageUtil.canUseHardwareBitmap(data))`, `setImage(ImageSource.inputStream(data.inputStream()))`, `isVisible = true`, return. `canUseHardwareBitmap(source)` peeks the header (`ImageUtil.kt:594-598`, `peek()` so the stream is not consumed) and returns `max(outWidth, outHeight) <= hardwareBitmapThreshold` (`ImageUtil.kt:330-333`); `hardwareBitmapThreshold` defaults to `GLUtil.SAFE_TEXTURE_LIMIT` = 2048 (`GLUtil.kt:50`) and is overwritten at app start from `pref_hardware_bitmap_threshold` (`App.kt:146`).
  - else (webtoon, not forced) → Coil `ImageRequest`: `data(source)`, `memoryCachePolicy(DISABLED)`, `diskCachePolicy(DISABLED)`, `size(ViewSizeResolver(this))`, `precision(INEXACT)`, `cropBorders(config.cropBorders)`, `customDecoder(true)`, `crossfade(false)`. `onSuccess`: `result as BitmapImage` → `setImage(ImageSource.bitmap(image.bitmap))`, visible. `onError` → `onImageLoadError()`.
- any other type → `throw IllegalArgumentException("Not implemented for class ${data::class.simpleName}")`.

Observed defect: a non-animated `Drawable` that is not a `BitmapDrawable` throws (no fallback). `result as BitmapImage` throws `ClassCastException` if Coil returns a non-bitmap image.

### 9.4 Animated loading (`setAnimatedImage`, lines 391-418)
- `PhotoView.setZoomTransitionDuration(config.zoomDuration.getSystemScaledDuration())` when the view is a PhotoView.
- Coil request: `data(data)`, memory/disk cache DISABLED, `crossfade(false)`. `onSuccess`: `result.asDrawable(context.resources)` → `setImageDrawable`; if `Animatable` → `start()`; `isVisible = true`; `onImageLoaded()`. `onError` → `onImageLoadError()`.
- The `Config` fields other than `zoomDuration` are ignored on this path (KDoc line 425).

### 9.5 Zoom configuration and double tap
Non-animated (`prepareNonAnimatedImageView`, lines 247-274) at view creation:
- `setMaxTileSize(ImageUtil.hardwareBitmapThreshold)` (max tile edge, default 2048).
- `setDoubleTapZoomStyle(SubsamplingScaleImageView.ZOOM_FOCUS_CENTER)` (= 2).
- `setPanLimit(SubsamplingScaleImageView.PAN_LIMIT_INSIDE)` (= 1).
- `setMinimumTileDpi(180)`.
- `OnStateChangedListener`: `onScaleChanged` → callback; `onCenterChanged` ignored.
- Click → `onViewClicked()`.
- `addView(pageView, MATCH_PARENT, MATCH_PARENT)`.

`setupZoom(config)` (called on ready, lines 276-287):
- `maxScale = scale * MAX_ZOOM_SCALE` where `MAX_ZOOM_SCALE = 5F` (file-private const, line 442). So max zoom = 5x the fitted scale.
- `setDoubleTapZoomScale(scale * 2)`.
- Zoom start: `ZoomStartPosition.LEFT` → `setScaleAndCenter(scale, PointF(0,0))`; `RIGHT` → `setScaleAndCenter(scale, PointF(sWidth, 0))`; `CENTER` → `setScaleAndCenter(scale, center)`; `config == null` → no change.

Animated (`prepareAnimatedImageView`, lines 352-389):
- Webtoon → `AppCompatImageView` with `adjustViewBounds = true`, no zoom.
- Otherwise → `PhotoView` with `adjustViewBounds = true`, `setScaleLevels(1F, 2F, MAX_ZOOM_SCALE)` (min 1, medium 2, max 5), and a custom `GestureDetector.SimpleOnGestureListener` set via `setOnDoubleTapListener`:
  - `onDoubleTap(e)`: if `scale > 1F` → `setScale(1F, e.x, e.y, true)` else `setScale(2F, e.x, e.y, true)`; return true.
  - `onSingleTapConfirmed(e)`: `onViewClicked()`; return `super.onSingleTapConfirmed(e)` (false).
- `setOnScaleChangeListener` → `onScaleChanged(scale)`.

Double-tap duration = `zoomDuration` (= `doubleTapAnimDuration`, default 500 ms) scaled by system animation scale (§9.8). For the PhotoView the value also sets `setZoomTransitionDuration`.

### 9.6 Landscape auto-zoom (`landscapeZoom`, lines 125-161)
Trigger preconditions (all must hold):
1. `config != null`
2. `config.landscapeZoom == true`
3. `config.minimumScaleType == SCALE_TYPE_CENTER_INSIDE` (= 1, i.e. "Fit screen")
4. `sWidth > sHeight` (image is landscape)
5. `scale == minScale` (not already zoomed)

Then `handler.postDelayed(500) { … }`:
- point = `when (config.zoomStartPosition)`:
  - LEFT: forward → `PointF(0, 0)`; backward → `PointF(sWidth, 0)`.
  - RIGHT: forward → `PointF(sWidth, 0)`; backward → `PointF(0, 0)`.
  - CENTER → `center`.
- `targetScale = height.toFloat() / sHeight.toFloat()` (fit the image height to the view height).
- `animateScaleAndCenter(targetScale, point)!!.withDuration(500).withEasing(EASE_IN_OUT_QUAD).withInterruptible(true).start()`.

Observed: preconditions are checked before the 500 ms delay and not re-checked after it. The `!!` throws NPE if the fork returns null (when not ready).

Callers: `onReady` of non-animated load (`landscapeZoom(true)` if `isVisibleOnScreen()`), and `onPageSelected(forward)` (§9.7).

### 9.7 onPageSelected(forward) (lines 102-123)
Called by the viewer when this page becomes the selected page (area 44).
- If `pageView` is not an SSIV → nothing.
- If `isReady` → `landscapeZoom(forward)`.
- Else → installs a new `OnImageEventListener`, replacing the one from §9.3:
  - `onReady`: `setupZoom(config)`; `landscapeZoom(forward)`; `this@ReaderPageImageView.onImageLoaded()`.
  - `onImageLoadError(e)`: `onImageLoadError()`.
Observed: the listener from §9.3 (which calls `landscapeZoom(true)` only if visible) is replaced in this case, so `forward` (not `true`) drives the landscape anchor.

### 9.8 System-scaled duration (`getSystemScaledDuration`, lines 420-422)
`(this * context.animatorDurationScale).toInt().coerceAtLeast(1)`. `animatorDurationScale` = `Settings.Global.getFloat(contentResolver, ANIMATOR_DURATION_SCALE, 1f)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/AnimationExtensions.kt:10-13`). Applies to zoom duration only. Landscape zoom (500 ms), pan (250 ms) and their easing are not scaled (observed).

### 9.9 Pan (`canPan`, `panLeft`, `panRight`, `pan`; lines 196-245)
- `canPanLeft()` = `canPan { it.left } ` > 1; `canPanRight()` = `canPan { it.right }` > 1. `canPan` calls SSIV `getPanRemaining(RectF)`, which fills the rect with the remaining pan distance in screen pixels in each direction (`SubsamplingScaleImageView.java:2018-2025`; 0 when the edge is reached). Only applies to SSIV pages; animated pages return false.
- `panLeft()` / `panRight()` → `pan { center, view -> center.x -= view.width / view.scale  }` (or `+=`). Moves by one screen width expressed in source-image pixels.
- `pan(fn)`: `view.center ?: return`, `animateCenter(target)!!.withEasing(EASE_OUT_QUAD).withDuration(250).withInterruptible(true).start()`.

Edge: `view.center` is null before ready → return (no-op). `animateCenter` may return null → NPE (observed `!!`).

### 9.10 Visibility and lifecycle
- `isVisible` toggled true on load success, false on `recycle()`.
- Attaching a new drawable/source does not clear the previous image before loading; `prepare*` removes the old view only when the type changes (`removeView(pageView)`, lines 249, 354).
- `prepareNonAnimatedImageView` returns early if the current view is already an SSIV (line 248), so the same view and fork state (zoom, pan) are reused across loads. Consequence: loading a new page into a reused SSIV keeps its old `minimumScaleType` only until `setMinimumScaleType` is called (it is called on every load, so it is overwritten).

### 9.11 Config (`ReaderPageImageView.kt:427-433`)
`data class Config(val zoomDuration: Int, val minimumScaleType: Int = SCALE_TYPE_CENTER_INSIDE, val cropBorders: Boolean = false, val zoomStartPosition: ZoomStartPosition = ZoomStartPosition.CENTER, val landscapeZoom: Boolean = false)`.
Pager construction (`PagerPageHolder.kt:165-166`): `zoomDuration = viewer.config.doubleTapAnimDuration`, `minimumScaleType = viewer.config.imageScaleType`. Other fields set in the same call are in area 44.
Webtoon construction (`WebtoonPageHolder.kt:200-201`): `zoomDuration = viewer.config.doubleTapAnimDuration`, `minimumScaleType = SubsamplingScaleImageView.SCALE_TYPE_FIT_WIDTH`.

### 9.12 ZoomStartPosition enum (`ReaderPageImageView.kt:435-439`)
`LEFT`, `CENTER`, `RIGHT`. Pager mapping in §3.6.

## 10. ReaderProgressIndicator (`ReaderProgressIndicator.kt:141-174`)

- View wraps a composable `CombinedCircularProgressIndicator(progress = { progress })` in `TachiyomiTheme`. The KDoc states it "always rotates" so the user sees the app is not stuck.
- `progress` is a `mutableFloatStateOf(0f)`.
- `setProgress(progress: Int)` with `@IntRange(from = 0, to = 100)`: `progress = progress / 100f`. No clamping in code; the annotation is the only constraint.
- `show()` → `isVisible = true`; `hide()` → `isVisible = false`.
- Layout: `FrameLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT, Gravity.CENTER)`.
- Composition strategy: `DisposeOnDetachedFromWindowOrReleasedFromPool`.
- Callers: `ReaderActivity.kt:137` (field) and `:559` (created). Visibility/progress values are driven from part 1.

## 11. ReaderTransitionView and calculateChapterGap

### 11.1 calculateChapterGap (`MissingChapters.kt:7-10`)
`fun calculateChapterGap(higherReaderChapter: ReaderChapter?, lowerReaderChapter: ReaderChapter?): Int = domainCalculateChapterGap(higherReaderChapter?.chapter?.toDomainChapter(), lowerReaderChapter?.chapter?.toDomainChapter())`.

Domain implementation (`domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt:40-49`):
1. `higher == null || lower == null` → 0.
2. `!higher.isRecognizedNumber || !lower.isRecognizedNumber` → 0.
3. `calculateChapterGap(higher.chapterNumber, lower.chapterNumber)`:
   - if either < 0.0 → 0;
   - else `floor(higher).toInt() - floor(lower).toInt() - 1`.

Examples (derived from the formula): (10, 8) → 1; (10, 9) → 0; (5.0, 5.5) → **-1**; (3, 2.5) → 0; (-1, 4) → 0.
Consumer `presentation/reader/ChapterTransition.kt:120`: shows the "missing chapters" line only `if (chapterGap > 0)`. So negative values are hidden, not clamped.

### 11.2 ReaderTransitionView (`ReaderTransitionView.kt:195-247`)
- `bind(transition: ChapterTransition, downloadManager: MangaDownloadManager, manga: Manga?)`:
  - `manga == null` → `data = null` (nothing rendered).
  - else:
    - `currChapterDownloaded = transition.from.pageLoader?.isLocal == true`.
    - `goingToChapterDownloaded = manga.isLocal() || (transition.to?.chapter?.let { downloadManager.isChapterDownloaded(chapterName = c.name, chapterScanlator = c.scanlator, mangaTitle = manga.title, sourceId = manga.source, skipCache = true) } ?: false)`.
  - Short-circuit: local manga → true with no download lookup.
  - `skipCache = true` forces the download manager to re-check the filesystem rather than its in-memory cache.
- `Content()`: when `data != null`: `TachiyomiTheme { CompositionLocalProvider(LocalTextStyle provides MaterialTheme.typography.bodySmall, LocalContentColor provides MaterialTheme.colorScheme.onBackground) { ChapterTransition(transition, currChapterDownloaded, goingToChapterDownloaded) } }`.
- The downloaded flags are computed once in `bind`. Download completion while the transition is visible does not update the view until `bind` runs again.
- Layout `LayoutParams(WRAP_CONTENT, WRAP_CONTENT)`.
- Used by `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerTransitionHolder.kt:61`.

Proposal: the Rust transition widget receives the same two booleans; it recomputes them when a download-state event for the target chapter arrives (improvement; see tasks T-43b-032).

## 12. ReaderButton (`ReaderButton.kt:101-116`)

- Subclass of MaterialButton. `var viewer: PagerViewer? = null`.
- `onTouchEvent(event)`: on every event `viewer?.pager?.setGestureDetectorEnabled(false)`; on `ACTION_UP` → `setGestureDetectorEnabled(true)`; return `super.onTouchEvent(event)`.
- Purpose: the pager's gesture detector would otherwise steal the button's tap (taps on a pager child are page-turn taps).
- Used by `PagerTransitionHolder.kt:124` for the retry button on a failed chapter load.
- Note: `ACTION_CANCEL` does not re-enable the pager gesture detector (observed). If a touch is cancelled (e.g. the pointer leaves the view and the parent intercepts), gestures stay disabled until the next `ACTION_UP` on any ReaderButton. Proposal: GTK buttons do not need this; see §12 proposal in p2-rust-design.

## 13. Viewer interface (`Viewer.kt:259-292`)

Contract:
- `getView(): View` — returns the root view of the viewer.
- `destroy()` — default no-op; called when leaving the reader or swapping viewers.
- `setChapters(chapters: ViewerChapters)` — sets the active chapters (model type from part 1).
- `moveToPage(page: ReaderPage)`.
- `handleKeyEvent(event: KeyEvent): Boolean` — called by the activity for every key event; returns true if consumed.
- `handleGenericMotionEvent(event: MotionEvent): Boolean` — called from `ReaderActivity.dispatchGenericMotionEvent` (`ReaderActivity.kt`: `val handled = viewModel.state.value.viewer?.handleGenericMotionEvent(event) ?: false; return handled || super.dispatchGenericMotionEvent(event)`).

Proposal: a Rust trait `Viewer` with `fn widget(&self) -> gtk::Widget`, `fn destroy(&mut self) {}`, `fn set_chapters(&mut self, ViewerChapters)`, `fn move_to_page(&mut self, ReaderPageRef)`, `fn handle_key(&mut self, gdk::Key, gdk::ModifierType) -> bool`, `fn handle_scroll(&mut self, dx: f64, dy: f64) -> bool` (mapping of Android generic motion events to GTK scroll/gesture events).

## 14. E-ink flash (consumer of ReaderPreferences; implementation in `app/src/main/java/eu/kanade/presentation/reader/DisplayRefreshHost.kt`)

Preferences involved:
- `flashOnPageChange` (`pref_reader_flash`, false) gates creation of the host in `ReaderActivity` (`ReaderActivity.kt:433-437`): `if (flashOnPageChange) DisplayRefreshHost(hostState = displayRefreshHost)`.
- `flashDurationMillis` (`pref_reader_flash_duration`, int, default 100 = MILLI_CONVERSION). Stored in ms; the slider stores `value * 100` for value 1..15 (`SettingsReaderScreen.kt:127-141`: `valueRange = 1..15`, `flashMillisPref.set(it * MILLI_CONVERSION)`). Subtitle `pref_flash_duration_summary` = "%1$s ms".
- `flashPageInterval` (`pref_reader_flash_interval`, int, default 1). Slider 1..10 (`valueRange = 1..10`), subtitle plural `pref_pages`.
- `flashColor` (`pref_reader_flash_mode`, enum name, default BLACK).

Algorithm (`DisplayRefreshHost.kt:20-60`):
- `flash()` (called from `ReaderActivity.kt:225` on `ReaderViewModel.Event.PageChanged`): `if (timesCalled % flashInterval == 0) currentDisplayRefresh = true; timesCalled += 1`. Page changes are counted even when not flashed. First page change flashes (0 % n == 0).
- `setInterval(interval)`: stores interval and resets `timesCalled = 0`. Called from `LaunchedEffect(flashInterval)` on change.
- Composable `DisplayRefreshHost`: when `currentDisplayRefresh` becomes true:
  - `half = refreshDuration.milliseconds / 2`.
  - `currentColor = Black` if mode == BLACK else `White`.
  - `delay(half)`.
  - if mode == WHITE_BLACK → `currentColor = Black`.
  - `delay(half)`.
  - `currentDisplayRefresh = false` → `LaunchedEffect` clears `currentColor = null` and the canvas draws nothing.
- The canvas fills the whole view with `drawRect(currentColor)`.

Observed consequences:
- BLACK: full-screen black for `duration` ms. WHITE: full-screen white for `duration`. WHITE_BLACK: white for `duration/2`, then black for `duration/2`.
- A second page change during a flash does not restart it (`currentDisplayRefresh` stays true; the `LaunchedEffect` key does not change).
- `flashInterval` of 0 would divide by zero (`timesCalled % 0`); the UI slider minimum is 1, so this cannot be set from the UI.
- Flash is gated only by `flashOnPageChange`; the color/duration/interval prefs are read live.

## 15. Message keys used in scope

Keys referenced from this scope (`MR.strings.*`), with English text from `i18n/src/commonMain/moko-resources/base/strings.xml`:
| Key | English |
|---|---|
| label_default | Default |
| l_nav | L shaped |
| kindlish_nav | Kindle-ish |
| edge_nav | Edge |
| right_and_left_nav | Right and Left |
| disabled_nav | Disabled |
| scale_type_fit_screen | Fit screen |
| scale_type_stretch | Stretch |
| scale_type_fit_width | Fit width |
| scale_type_fit_height | Fit height |
| scale_type_original_size | Original size |
| scale_type_smart_fit | Smart fit |
| zoom_start_automatic | Automatic |
| zoom_start_left | Left |
| zoom_start_right | Right |
| zoom_start_center | Center |
| filter_mode_multiply | Multiply |
| filter_mode_screen | Screen |
| filter_mode_overlay | Overlay |
| filter_mode_lighten | Dodge / Lighten |
| filter_mode_darken | Burn / Darken |
| tapping_inverted_none | None |
| tapping_inverted_horizontal | Horizontal |
| tapping_inverted_vertical | Vertical |
| tapping_inverted_both | Both |
| nav_zone_prev | Prev |
| nav_zone_next | Next |
| nav_zone_left | Left |
| nav_zone_right | Right |
| action_menu | Menu |
| left_to_right_viewer | Paged (left to right) |
| right_to_left_viewer | Paged (right to left) |
| vertical_viewer | Paged (vertical) |
| webtoon_viewer | Long strip |
| vertical_plus_viewer | Long strip with gaps |
| pref_flash_style_black | Black |
| pref_flash_style_white | White |
| pref_flash_style_white_black | White and Black |
| pref_flash_duration_summary | %1$s ms |
| pref_flash_page | Flash on page change |
| pref_flash_page_summ | Reduces ghosting on e-ink displays |
| pref_flash_duration | Flash duration |
| pref_flash_page_interval | Flash every |
| pref_flash_with | Flash with |
| pref_highest | Highest |
| pref_high | High |
| pref_low | Low |
| pref_lowest | Lowest |
| pref_webtoon_side_padding | Side padding |
| pref_read_with_tapping_inverted | Invert tap zones |
| pref_preserve_reading_position | Preserve reading position on read chapters |
| pref_show_page_number | Show page number |
| pref_double_tap_anim_speed | Double tap animation speed |
| pref_show_reading_mode | Show reading mode |
| pref_flash_page_interval plurals | `pref_pages` plural (not reproduced; plural forms per locale in `i18n/src/commonMain/moko-resources/*/plurals.xml`) |

Also referenced: `black_background`, `gray_background`, `white_background`, `automatic_background` (theme chips, `GeneralSettingsPage.kt:14-19`) and `MR.strings.loading` (ReaderActivity, part 1). `ReaderPreferences` itself uses no other user-visible strings.

Keys present in the locale catalogs but not in English `base/strings.xml` are not part of this scope; translations are area:51-i18n.

## 16. Summary of side effects, dispatchers, timings, limits

| Item | Value | Where |
|---|---|---|
| Preference change dispatch | `changes()` conflated, immediate first emission | AndroidPreference.kt:60-66 |
| ViewerConfig subscribe scope | viewer coroutine scope | ViewerConfig.kt:88 |
| One-shot overlay write | `reader_navigation_overlay_new_user` → false on first viewer | ViewerConfig.kt:67-70 |
| Max zoom | 5 × fitted scale | ReaderPageImageView.kt:278, :364, 442 |
| Double-tap zoom | 2 × fitted scale (non-animated) / levels 1, 2 (animated) | :279, :364, :371-372 |
| Double-tap duration | pref (default 500 ms) × system animator scale, min 1 ms | :293, :396, :420-422 |
| Landscape auto-zoom delay | 500 ms | :132 |
| Landscape auto-zoom duration | 500 ms, EASE_IN_OUT_QUAD | :155-156 |
| Pan animation | 250 ms, EASE_OUT_QUAD | :240-241 |
| Pan step | one view width in source px | :221, :228 |
| Min DPI | 1 (load), tile min DPI 180 | :259, :295 |
| Max tile size | hardwareBitmapThreshold (default 2048) | :256 |
| Hardware bitmap threshold | max(width,height) ≤ threshold, else software | ImageUtil.kt:330-333, GLUtil.kt:50 |
| Long-press delay | platform long-press timeout | GestureDetectorWithLongTap.kt:22, :46 |
| Double-tap guard | platform double-tap timeout | :23, :43 |
| Flash duration | pref ms (default 100), half for each color step | DisplayRefreshHost.kt:52-57 |
| Flash interval | 1..10 pages, default 1 | SettingsReaderScreen.kt:137-146 |
| Webtoon hide-menu thresholds | 5 / 13 / 31 / 47 px | ReaderPreferences.kt:199-202 |
| Webtoon side padding | 0..25 (percent of 100) | ReaderPreferences.kt:206-207 |
| Brightness range | -75..100 | ColorFilterPage.kt KDoc |
| Chapter gap | floor(higher) - floor(lower) - 1, shown when > 0 | MissingChapters.kt, domain MissingChapters.kt:40-49, ChapterTransition.kt:120 |

### 16.1 Quirks and defects (observed, to be preserved or consciously fixed)
1. `ImageScaleType` index 2 "Stretch" maps to `SCALE_TYPE_CENTER_CROP` (crop-to-fill), not a non-uniform stretch. Proposal: keep the stored value 2 meaning crop-to-fill, relabel the Rust UI "Fill (crop)" or keep "Stretch" only if the user asks. Decision required: see p2-edge-cases-and-tests.md E-17.
2. `constantMenuRegion` is dead logic (both branches return MENU).
3. `calculateChapterGap` can be -1 (same integer part); caller hides non-positive values.
4. `ReaderButton` does not re-enable the pager gesture detector on ACTION_CANCEL.
5. `DisplayRefreshHost` flash duration is not restarted when a page change arrives during a flash.
6. `ReaderPageImageView.setNonAnimatedImage` throws for non-bitmap drawables; `result as BitmapImage` can throw ClassCastException.
7. `landscapeZoom` preconditions are not re-checked after the 500 ms delay.
8. `ReadingMode.toViewer` throws for DEFAULT (callers must resolve first).
9. Tap zones index 5 "Disabled" (`DisabledNavigation.kt`, `regionList = emptyList()`) makes every tap return MENU, so page turns happen only through keys, buttons or the page-turn gesture of area 44.
10. Preference-type mismatch deletes the key silently (AndroidPreference.kt:35-38).
