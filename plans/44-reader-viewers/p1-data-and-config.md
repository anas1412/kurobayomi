# 44-reader-viewers: data models, preferences, files, endpoints

Paths are repo-relative to `/home/blackbox/aniyomi-src`. Where a type is owned by another area the owner is named. This document covers only the fields the viewers read or write.

---

## 1. Data models used by the viewers

### 1.1 Page: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt`

Owner: source API (area:15/16). Viewer-relevant members (declared at lines 11-42):

| Member | Type | Nullability | Default | Notes |
|---|---|---|---|---|
| index | Int | non-null | constructor arg | 0-based position in the chapter |
| url | String | non-null | "" | page URL on the source |
| imageUrl | String? | nullable | null | used by "Open in WebView" (only when it starts with http) |
| number | Int (derived) | non-null | `index + 1` | 1-based; used by preload threshold and split detection |
| status | `Page.State` | non-null | `QUEUE` | backed by `MutableStateFlow`; exposed as `statusFlow` |
| statusFlow | `StateFlow<Page.State>` | non-null | — | collected by holders |
| progress | Int | non-null | 0 | 0..100; backed by `progressFlow` |
| progressFlow | `StateFlow<Int>` | non-null | 0 | collected in DOWNLOAD_IMAGE |

`enum class Page.State` (lines 51-57, in declaration order): `QUEUE`, `LOAD_PAGE`, `DOWNLOAD_IMAGE`, `READY`, `ERROR`. No explicit numeric values (stored only in memory, never persisted).

### 1.2 ReaderPage: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ReaderPage.kt`

| Member | Type | Nullability | Default |
|---|---|---|---|
| index | Int (from Page) | non-null | constructor |
| url | String | non-null | "" |
| imageUrl | String? | nullable | null |
| stream | `(() -> InputStream)?` (var) | nullable | null; supplied by the loader |
| chapter | `ReaderChapter` (open lateinit var) | non-null after init | none; throws if read before set |

Equality: identity (`ReaderPage` does not override `equals`).

### 1.3 InsertPage: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/InsertPage.kt`

`class InsertPage(val parent: ReaderPage) : ReaderPage(parent.index, parent.url, parent.imageUrl)`.
- `chapter = parent.chapter` (overrides the lateinit).
- `status = READY` (init block).
- `stream = parent.stream` (init block).
- `index` is the parent's index; `number` is the same as the parent's, which the pager uses to detect split pairs.
- Identity equality.

### 1.4 ReaderChapter: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ReaderChapter.kt`

Owner: area:43-reader-core. Viewer-relevant:

| Member | Type | Default | Notes |
|---|---|---|---|
| chapter | domain `Chapter` (`data class ReaderChapter(val chapter: Chapter)`) | constructor | equality is structural over `chapter` |
| stateFlow | `StateFlow<State>` | `State.Wait` | observed by transitions |
| state | `State` | `Wait` | get/set on stateFlow |
| pages | `List<ReaderPage>?` | null | non-null only when `State.Loaded` |
| pageLoader | `PageLoader?` | null | `isLocal` used by transitions |
| requestedPage | Int | 0 | first-layout target (clamped to last page) |
| references | Int (private) | 0 | `ref()` / `unref()`; at 0 the loader is recycled and state reset to `Wait` |

`sealed interface ReaderChapter.State`:
- `Wait` (data object)
- `Loading` (data object)
- `Error(error: Throwable)` (data class)
- `Loaded(pages: List<ReaderPage>)` (data class)

### 1.5 ChapterTransition: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ChapterTransition.kt`

`sealed class ChapterTransition` with `abstract val from: ReaderChapter` and `abstract val to: ReaderChapter?`.
- `class Prev(from, to?)`
- `class Next(from, to?)`
- `equals`: same instance, or same class-independent unordered pair (`from==other.from && to==other.to`, or `from==other.to && to==other.from`). Note that `other !is ChapterTransition` is the only class check, so `Prev` and `Next` with the same pair compare equal.
- `hashCode = 31 * from.hashCode() + (to?.hashCode() ?: 0)`.
- `toString = "${javaClass.simpleName}(from=${from.chapter.url}, to=${to?.chapter?.url})"`.

### 1.6 ViewerChapters: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ViewerChapters.kt`

`data class ViewerChapters(val currChapter: ReaderChapter, val prevChapter: ReaderChapter?, val nextChapter: ReaderChapter?)`. `ref()` and `unref()` apply to all non-null members.

### 1.7 Navigation model: `V/ViewerNavigation.kt`

- `data class Region(val rectF: RectF, val type: NavigationRegion)`. `RectF` floats in 0..1 (edges as written in the navigation tables of p1-behavior.md §2.3).
- `NavigationRegion` (sealed, each with a `nameRes: StringResource` and `color: Int` ARGB):

| Object | Name key | ARGB |
|---|---|---|
| MENU | action_menu | 0xCC95818D |
| PREV | nav_zone_prev | 0xCCFF7733 |
| NEXT | nav_zone_next | 0xCC84E296 |
| LEFT | nav_zone_left | 0xCC7D1128 |
| RIGHT | nav_zone_right | 0xCCA6CFD5 |

- `invertMode: TappingInvertMode` (default NONE).

### 1.8 Rendering configs

`ReaderPageImageView.Config` (`V/ReaderPageImageView.kt:427-433`):

| Field | Type | Default | Pager value | Webtoon value |
|---|---|---|---|---|
| zoomDuration | Int (ms) | none (required) | `doubleTapAnimDuration` (500) | same |
| minimumScaleType | Int (SSIV constant) | `SCALE_TYPE_CENTER_INSIDE` (1) | `imageScaleType` (pref 1..6) | `SCALE_TYPE_FIT_WIDTH` (3) |
| cropBorders | Boolean | false | `imageCropBorders` | `imageCropBorders` (webtoon pref) |
| zoomStartPosition | `ZoomStartPosition` | CENTER | `imageZoomType` | CENTER (not passed) |
| landscapeZoom | Boolean | false | `landscapeZoom` | false (not passed) |

`enum ReaderPageImageView.ZoomStartPosition { LEFT, CENTER, RIGHT }` (ordinal order as declared).

`PagerConfig.imageZoomType` (default `LEFT` before first emission):
- pref 1 Automatic: L2R → LEFT; R2L → RIGHT; any other viewer (vertical) → CENTER.
- pref 2 Left → LEFT. pref 3 Right → RIGHT. pref 4 (and any other value) Center → CENTER.

### 1.9 Settings values held in memory

| Holder | Field | Type | Default |
|---|---|---|---|
| ViewerConfig | tappingInverted | `ReaderPreferences.TappingInvertMode` | NONE |
| ViewerConfig | longTapEnabled | Boolean | true |
| ViewerConfig | usePageTransitions | Boolean | false (preference default is true; the field is set on first emission) |
| ViewerConfig | doubleTapAnimDuration | Int | 500 |
| ViewerConfig | volumeKeysEnabled / volumeKeysInverted | Boolean | false / false |
| ViewerConfig | alwaysShowChapterTransition | Boolean | true |
| ViewerConfig | navigationMode | Int | 0 |
| ViewerConfig | forceNavigationOverlay | Boolean | false (set from pref on construction) |
| ViewerConfig | navigationOverlayOnStart | Boolean | false |
| ViewerConfig | dualPageSplit / dualPageInvert / dualPageRotateToFit / dualPageRotateToFitInvert | Boolean | false each |
| PagerConfig | theme | Int | `readerTheme()` default 1 |
| PagerConfig | automaticBackground | Boolean | false (true when theme == 3) |
| PagerConfig | imageScaleType | Int | 1 |
| PagerConfig | imageZoomType | `ZoomStartPosition` | LEFT |
| PagerConfig | imageCropBorders | Boolean | false |
| PagerConfig | navigateToPan | Boolean | false (preference default true) |
| PagerConfig | landscapeZoom | Boolean | false (preference default true) |
| WebtoonConfig | theme | Int | `readerTheme()` default 1 (val, read once) |
| WebtoonConfig | imageCropBorders | Boolean | false |
| WebtoonConfig | zoomOutDisabled | Boolean | false |
| WebtoonConfig | sidePadding | Int | 0 |
| WebtoonConfig | doubleTapZoom | Boolean | true |
| WebtoonViewer | threshold | Int | from `ReaderHideThreshold` (LOW = 31) |

Note that a few Kotlin fields are initialised to a value that differs from the preference default (`usePageTransitions`, `navigateToPan`, `landscapeZoom`). The preference always wins once the first emission arrives. The Rust port must initialise from the preference store, not from these fallbacks.

---

## 2. Enums and their stored values

| Enum | Declared in | Stored form | Values (stored) |
|---|---|---|---|
| `ReaderPreferences.TappingInvertMode` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:183-196` | `getEnum` → enum **name** string | NONE (h=0,v=0), HORIZONTAL (h=1,v=0), VERTICAL (h=0,v=1), BOTH (h=1,v=1) |
| `ReaderPreferences.ReaderHideThreshold` | `ReaderPreferences.kt:198-203` | enum name | HIGHEST(5), HIGH(13), LOW(31), LOWEST(47). Default LOW |
| `ReaderPreferences.FlashColor` | `ReaderPreferences.kt:177-181` | enum name | BLACK, WHITE, WHITE_BLACK (used by area:43 flash, not here) |
| `ReadingMode` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt:12-47` | int `flagValue` | DEFAULT 0x0, LEFT_TO_RIGHT 0x1, RIGHT_TO_LEFT 0x2, VERTICAL 0x3, WEBTOON 0x4, CONTINUOUS_VERTICAL 0x5. MASK 0x7 |
| `ReaderPageImageView.ZoomStartPosition` | `ReaderPageImageView.kt:435-439` | not persisted | LEFT, CENTER, RIGHT |
| `ChapterTransition` kinds | `model/ChapterTransition.kt` | not persisted | Prev, Next |
| `Page.State` | `Page.kt:51-57` | not persisted | QUEUE, LOAD_PAGE, DOWNLOAD_IMAGE, READY, ERROR |
| `ReaderChapter.State` | `ReaderChapter.kt:45-50` | not persisted | Wait, Loading, Error, Loaded |
| `ImageUtil.Side` | `core/common/.../ImageUtil.kt:196-199` | not persisted | RIGHT, LEFT |
| `ImageUtil.Format` (decoder) | `tachiyomi.decoder` (dependency) | not persisted | Avif, Gif, Heif, Jpeg, Jxl, Png, Webp |
| `ImageUtil.ImageType` | `ImageUtil.kt:106-114` | not persisted | AVIF("image/avif","avif"), GIF("image/gif","gif"), HEIF("image/heif","heif"), JPEG("image/jpeg","jpg"), JXL("image/jxl","jxl"), PNG("image/png","png"), WEBP("image/webp","webp") |
| SSIV constants | SSIV `66e0db195d` | int | ZOOM_FOCUS_FIXED 1, ZOOM_FOCUS_CENTER 2, ZOOM_FOCUS_CENTER_IMMEDIATE 3; EASE_OUT_QUAD 1, EASE_IN_OUT_QUAD 2; PAN_LIMIT_INSIDE 1, PAN_LIMIT_OUTSIDE 2, PAN_LIMIT_CENTER 3; SCALE_TYPE_CENTER_INSIDE 1, CENTER_CROP 2, FIT_WIDTH 3, FIT_HEIGHT 4, ORIGINAL_SIZE 5, SMART_FIT 6, CUSTOM 7; ORIGIN_ANIM 1, TOUCH 2, FLING 3, DOUBLE_TAP_ZOOM 4 |

---

## 3. Preference keys used by the viewers (with defaults)

Defined in `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt` (class owned by area:43; key list here is complete for viewer consumers). The `getBoolean`/`getInt`/`getEnum` call is quoted.

| Key | Accessor (line) | Type | Default | Consumer(s) |
|---|---|---|---|---|
| `pref_enable_transitions_key` | `pageTransitions()` (16) | Boolean | true | ViewerConfig.usePageTransitions (animates page move and webtoon scroll) |
| `pref_double_tap_anim_speed` | `doubleTapAnimSpeed()` (26) | Int (ms) | 500 | ViewerConfig.doubleTapAnimDuration |
| `reader_long_tap` | `readWithLongTap()` (142) | Boolean | true | ViewerConfig.longTapEnabled |
| `reader_volume_keys` | `readWithVolumeKeys()` (144) | Boolean | false | ViewerConfig.volumeKeysEnabled |
| `reader_volume_keys_inverted` | `readWithVolumeKeysInverted()` (146) | Boolean | false | ViewerConfig.volumeKeysInverted |
| `always_show_chapter_transition` | `alwaysShowChapterTransition()` (59) | Boolean | true | ViewerConfig.alwaysShowChapterTransition |
| `reader_navigation_overlay_new_user` | `showNavigationOverlayNewUser()` (165) | Boolean | true | ViewerConfig.forceNavigationOverlay (set to false after read) |
| `reader_navigation_overlay_on_start` | `showNavigationOverlayOnStart()` (170) | Boolean | false | ViewerConfig.navigationOverlayOnStart |
| `reader_navigation_mode_pager` | `navigationModePager()` (151) | Int | 0 | PagerConfig.navigationMode |
| `reader_navigation_mode_webtoon` | `navigationModeWebtoon()` (153) | Int | 0 | WebtoonConfig.navigationMode |
| `reader_tapping_inverted` | `pagerNavInverted()` (155) | enum TappingInvertMode | NONE | PagerConfig.tappingInverted |
| `reader_tapping_inverted_webtoon` | `webtoonNavInverted()` (160) | enum TappingInvertMode | NONE | WebtoonConfig.tappingInverted |
| `pref_image_scale_type_key` | `imageScaleType()` (53) | Int (1..6) | 1 | PagerConfig.imageScaleType |
| `pref_zoom_start_key` | `zoomStart()` (55) | Int (1..4) | 1 | PagerConfig.imageZoomType |
| `pref_reader_theme_key` | `readerTheme()` (57) | Int (0..3) | 1 | PagerConfig.theme/automaticBackground; WebtoonConfig.theme; createReaderThemeContext |
| `crop_borders` | `cropBorders()` (69) | Boolean | false | PagerConfig.imageCropBorders |
| `crop_borders_webtoon` | `cropBordersWebtoon()` (75) | Boolean | false | WebtoonConfig.imageCropBorders |
| `navigate_pan` | `navigateToPan()` (71) | Boolean | true | PagerConfig.navigateToPan |
| `landscape_zoom` | `landscapeZoom()` (73) | Boolean | true | PagerConfig.landscapeZoom |
| `webtoon_side_padding` | `webtoonSidePadding()` (77) | Int (0..25) | `WEBTOON_PADDING_MIN` = 0 | WebtoonConfig.sidePadding |
| `reader_hide_threshold` | `readerHideThreshold()` (79) | enum ReaderHideThreshold | LOW | WebtoonViewer.threshold |
| `webtoon_disable_zoom_out` | `webtoonDisableZoomOut()` (89) | Boolean | false | WebtoonConfig.zoomOutDisabled |
| `pref_enable_double_tap_zoom_webtoon` | `webtoonDoubleTapZoomEnabled()` (48) | Boolean | true | WebtoonConfig.doubleTapZoom |
| `pref_dual_page_split` | `dualPageSplitPaged()` (95) | Boolean | false | PagerConfig.dualPageSplit |
| `pref_dual_page_invert` | `dualPageInvertPaged()` (97) | Boolean | false | PagerConfig.dualPageInvert |
| `pref_dual_page_rotate` | `dualPageRotateToFit()` (103) | Boolean | false | PagerConfig.dualPageRotateToFit |
| `pref_dual_page_rotate_invert` | `dualPageRotateToFitInvert()` (105) | Boolean | false | PagerConfig.dualPageRotateToFitInvert |
| `pref_dual_page_split_webtoon` | `dualPageSplitWebtoon()` (99) | Boolean | false | WebtoonConfig.dualPageSplit |
| `pref_dual_page_invert_webtoon` | `dualPageInvertWebtoon()` (101) | Boolean | false | WebtoonConfig.dualPageInvert |
| `pref_dual_page_rotate_webtoon` | `dualPageRotateToFitWebtoon()` (110) | Boolean | false | WebtoonConfig.dualPageRotateToFit |
| `pref_dual_page_rotate_invert_webtoon` | `dualPageRotateToFitInvertWebtoon()` (115) | Boolean | false | WebtoonConfig.dualPageRotateToFitInvert |

Keys used by the viewer code through other classes:

| Key | Accessor | Type | Default | Defined in | Consumer |
|---|---|---|---|---|---|
| `pref_hardware_bitmap_threshold` | `BasePreferences.hardwareBitmapThreshold()` (line 40) | Int | `GLUtil.SAFE_TEXTURE_LIMIT` = 2048 | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt` | `ImageUtil.hardwareBitmapThreshold` → SSIV max tile size |
| `pref_always_decode_long_strip_with_ssiv` | `BasePreferences.alwaysDecodeLongStripWithSSIV()` (line 42) | Boolean | false | same | `ReaderPageImageView` webtoon decode path |

Preference keys in `ReaderPreferences` that are **not** read by the viewers (owned by area:43): `pref_show_page_number_key`, `pref_show_reading_mode`, `fullscreen`, `cutout_short`, `pref_keep_screen_on_key`, `pref_default_reading_mode_key`, `pref_default_orientation_type_key`, the colour-filter keys (`pref_custom_brightness_key`, `custom_brightness_value`, `pref_color_filter_key`, `color_filter_value`, `color_filter_mode`, `pref_grayscale`, `pref_inverted_colors`), the flash keys (`pref_reader_flash`, `pref_reader_flash_duration`, `pref_reader_flash_interval`, `pref_reader_flash_mode`), the skip keys (`skip_read`, `skip_filtered`, `skip_dupe`), `create_folder_per_manga` and `pref_preserve_reading_position`.

### 3.1 Defaults that differ from the Kotlin fields (explicit)

| Field | Kotlin initial | Preference default | Effective value after load |
|---|---|---|---|
| ViewerConfig.usePageTransitions | false | true | true |
| PagerConfig.navigateToPan | false | true | true |
| PagerConfig.landscapeZoom | false | true | true |
| PagerConfig.imageZoomType | LEFT | from pref 1 (Automatic) | per direction |

---

## 4. Files, folders and formats on disk

The viewers write nothing to disk. Relevant disk-adjacent facts:
- Page bytes are read from the loader's stream (`ReaderPage.stream`). For downloaded chapters the loader is `DownloadPageLoader` (area:43, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/DownloadPageLoader.kt`). For local files `DirectoryPageLoader`/`ArchivePageLoader` (area:43).
- Split and rotate outputs are in-memory `okio.Buffer`s encoded as JPEG quality 100 (`ImageUtil.kt:145,155,192`). No temp files.
- Coil requests for webtoon and animated images use `memoryCachePolicy(DISABLED)` and `diskCachePolicy(DISABLED)` (`ReaderPageImageView.kt:326-327, 401-402`). No cache files are created by this area.
- Accepted image formats are whatever `tachiyomi.decoder.ImageDecoder.findType` identifies (`ImageUtil.kt:50-65`): AVIF, GIF, HEIF, JPEG, JXL, PNG, WEBP. Extension names in `ImageType`: avif, gif, heif, jpg, jxl, png, webp.

Proposal for Kurobayomi: page bytes come from the area:43 loaders (downloads folder, archive, or network), never from a viewer-owned temp directory. Decoded RGBA pages live in an in-memory LRU (§1 of p1-rust-design.md). No disk cache in this area.

---

## 5. Network endpoints

The viewers make no HTTP request. Page fetching is done by the page loaders (area:43, `HttpPageLoader.kt` and source API, areas 15/16). The only network-adjacent behaviour is:

| Trigger | Action | Input | Notes |
|---|---|---|---|
| Error card "Open in WebView" | `WebViewActivity.newIntent(context, imageUrl)` then `startActivity` | `imageUrl` (page's `imageUrl`) | only when `imageUrl.startsWith("http", ignoreCase = true)`; visible-but-no-op otherwise (`PagerPageHolder.kt:283-289`, `WebtoonPageHolder.kt:290-296`) |

Proposal: open the URL with `gio::AppInfo::launch_default_for_uri` (or `gtk::UriLauncher`) in the default browser. The same http-only gate applies.

---

## 6. Database tables

The viewers read no table directly. Read progress and chapter state are written by `ReaderViewModel` (area:43) through the chapter repository (area:05 and area:43). Tables touched indirectly: `chapters` (read, last page read, bookmarked; area:03 schema). The viewers do not write them.

Proposal: no table in Kurobayomi is owned by this area. The viewer calls the progress callback (`on_page_selected`) and area:43 persists it.

---

## 7. Constants and lookup tables that must survive the port byte-for-byte

These are the non-preference constants. Each is listed once here and referenced from the other documents.

| Name | Value | Source |
|---|---|---|
| Navigation tables | §2.3 of p1-behavior.md | `V/navigation/*.kt` |
| Tap-zone ARGB colours | §1.7 table above | `V/ViewerNavigation.kt:16-32` |
| Preload window | 5 | `PagerViewer.kt:239`, `WebtoonViewer.kt:214` |
| Prev/Next list windows | 2 | `PagerViewerAdapter.kt:60,108`, `WebtoonAdapter.kt:51,78` |
| Webtoon item cache | 4 | `WebtoonViewer.kt:364` |
| MAX_ZOOM_SCALE (pager) | 5 (× fit scale) | `ReaderPageImageView.kt:442` |
| Webtoon zoom min/default/max | 0.5 / 1 / 3 | `WebtoonRecyclerView.kt:347-349` |
| Webtoon zoom anim | 200 ms | `WebtoonRecyclerView.kt:346` |
| Webtoon fling | 400 ms, factor 0.4 | `WebtoonRecyclerView.kt:137,163` |
| Landscape zoom | delay 500, anim 500 | `ReaderPageImageView.kt:132,155` |
| Pan-before-page | 250 ms | `ReaderPageImageView.kt:241` |
| Quick-scale threshold | 20 dp | SSIV |
| Quick-scale multiplier threshold | 0.03 | SSIV |
| Quick-scale span factor | 0.5 | SSIV |
| Pan-remaining threshold | > 1 px | `ReaderPageImageView.kt:211` |
| Webtoon error height factor | 0.8 | `WebtoonPageHolder.kt:281` |
| Webtoon progress top margin divisor | 4 | `WebtoonPageHolder.kt:266` |
| Webtoon gap margin | 15 dp | `WebtoonPageHolder.kt:101` |
| Pager transition side padding | 64 dp | `PagerTransitionHolder.kt:58` |
| Webtoon transition padding | 128 dp vertical, 32 dp horizontal, child margin 16 dp | `WebtoonTransitionHolder.kt:361-365` |
| Transition card max width | 460 dp | `ChapterTransition.kt:100` |
| Transition card spacer | 24 dp | `ChapterTransition.kt:279` |
| Transition title size | 20 sp, maxLines 5 | `ChapterTransition.kt:238-239` |
| Scanlator maxLines | 2 | `ChapterTransition.kt:265` |
| Downloaded icon | 22 sp | `ChapterTransition.kt:245-246` |
| Card padding | 16 dp horizontal, 12 dp vertical, 16 dp spacing | `ChapterTransition.kt:155,183` |
| Border limit | 0.0025 of line length, halved | `image-decoder/borders.h:11`, `borders.cpp:25,72,119,165` |
| Border threshold | 191 (black if < 191), 63 (white if > 63) | `image-decoder/borders.h:16,18` |
| Border sampling stride | 2 | `borders.cpp:31,51,79,99,125,145,172,192` |
| Background minimum | 50 px per side | `ImageUtil.kt:345` |
| Background edge inset | top = 5, bottom = h − 5 | `ImageUtil.kt:349-350` |
| Background left inset | 2.75% of width | `ImageUtil.kt:351` |
| Background offset | 1% of width | `ImageUtil.kt:355` |
| Background row step | h / 25 | `ImageUtil.kt:443` |
| Background dark rule | r,g,b < 40 and a > 200 | `ImageUtil.kt:582-583` |
| Background white rule | r + g + b > 740 | `ImageUtil.kt:588-589` |
| Background close rule | |Δr|,|Δg|,|Δb| < 30 | `ImageUtil.kt:585-586` |
| Background streak thresholds | black/white streak ≥ 14; black streak > 6; white > 22; black count > 22; overall > 9; black count > 18 | `ImageUtil.kt:452,455,466,483,504,515,483` |
| Wide-image rule | width > height (strict) | `ImageUtil.kt:123` |
| Split width | width / 2 (integer) | `ImageUtil.kt:136,173` |
| Split JPEG quality | 100 | `ImageUtil.kt:145,155,192` |
| Gap formula | floor(higher) − floor(lower) − 1 | `domain/.../MissingChapters.kt:48` |
| Scale-type preference mapping | value = index + 1 over `ImageScaleType` list | `ReadingModePage.kt:85-88` |
| Zoom-start preference mapping | value = index + 1 over `ZoomStart` list | `ReadingModePage.kt:96-99` |
| Reader theme ints | White 0, Black 1, Gray 2, Automatic 3 | `ContextExtensions.kt:123-129`, `SettingsReaderScreen.kt:92-97` |
| Navigation mode ints | Default 0, L 1, Kindle-ish 2, Edge 3, Right-Left 4, Disabled 5 | `PagerConfig.kt:141-149`, `WebtoonConfig.kt:340-348` |
| Reading mode flags | 0..5 | `ReadingMode.kt:17-44` |
| Haptic constant | `HapticFeedbackConstants.LONG_PRESS` | `Pager.kt:42` |

---

## 8. Image format support matrix for this area

| Format | Still decode | Animated (observed) | Source of decode |
|---|---|---|---|
| JPEG | yes (SSIV or native decoder) | no | Android BitmapFactory / SSIV tiles / native decoder with crop |
| PNG | yes | no | same |
| WebP (still) | yes | no | same |
| WebP (animated) | yes | yes on Android 9+ (`ImageUtil.kt:79`) | Coil `AnimatedImageDecoder` (GIF module); SSIV not used |
| GIF | yes | yes (all GIFs) | Coil GIF decoder |
| AVIF | yes (Android 12+ system decoder or native) | no | `ImageDecoder` native (`avif` via tachiyomi.decoder) |
| HEIF | yes | yes on Android 11+ (`ImageUtil.kt:81`) | native |
| JXL | yes (native decoder) | no | native (`decoder_jxl.cpp`) |

Proposal for Kurobayomi (see p1-rust-design.md §5): still formats via `image` crate (JPEG, PNG, WebP, AVIF with the `avif` feature, GIF first frame) and `jxl-oxide` for JXL. Animated: GIF and animated WebP only. HEIF is decoded only if libheif is present (optional).

---

## 9. Data-model invariants the port must keep

1. A `ReaderPage` belongs to exactly one `ReaderChapter`. `InsertPage.chapter` is the parent's chapter.
2. An `InsertPage` is never the target of a preload and never triggers `preload` (`PagerViewer.kt:234-236`).
3. `ReaderPage.number` is `index + 1` and is unique within a chapter except for an InsertPage/parent pair.
4. `ChapterTransition` items appear at most twice per adjacent-chapter boundary (once as Prev, once as Next).
5. `ViewerChapters.currChapter` is always non-null. Prev and Next may be null.
6. The item list ends with a `Next` transition whenever the next chapter is absent or not Loaded, or a gap exists, or `forceTransition` is set (which is the default). When none of these holds, the list ends with pages of the next chapter (at most two). The list starts with a page or a `Prev` transition.
7. `requestedPage` is clamped to `[0, pages.lastIndex]` before use.
