# 43-reader-core (part 2 of 2) — Data models, preference keys, storage, endpoints

Repo root: `/home/blackbox/aniyomi-src`. Scope files are listed in `p2-inventory.md`. Behavior is in `p2-behavior.md`.

Contents
1. In-memory models (fields, types, nullability, defaults)
2. Preference keys: complete catalog (type, default, consumer, Linux storage)
3. Enum stored values and bit layouts
4. Persistent storage touched (SharedPreferences, manga row, no files)
5. Network endpoints (none owned here; page bytes come from part 1 loader / sources)
6. Database tables and columns touched
7. Linux storage plan

---

## 1. In-memory models

### 1.1 ReaderPreferences (class, `ReaderPreferences.kt:10`)
- Holds `preferenceStore: PreferenceStore` (private). No other state.

### 1.2 ReadingMode (enum, `ReadingMode.kt:300-378`)
| Field | Type | Nullable | Notes |
|---|---|---|---|
| stringRes | `dev.icerock.moko.resources.StringResource` | no | label key |
| iconRes | `@DrawableRes Int` | no | Android drawable id (replaced on Linux) |
| flagValue | Int | no | stored value (see §3.1) |
| direction | `ReadingMode.Direction?` | yes, default null | Horizontal / Vertical; null only for DEFAULT |
| type | `ReadingMode.ViewerType?` | yes, default null | Pager / Webtoon; null only for DEFAULT |

`Direction` sealed interface: data objects `Horizontal`, `Vertical`. `ViewerType` sealed interface: data objects `Pager`, `Webtoon`.

Entries (ordinal order used by the settings chips): DEFAULT, LEFT_TO_RIGHT, RIGHT_TO_LEFT, VERTICAL, WEBTOON, CONTINUOUS_VERTICAL.

### 1.3 ReaderPreferences.TappingInvertMode (enum)
| Field | Type | Default |
|---|---|---|
| titleRes | StringResource | required |
| shouldInvertHorizontal | Boolean | false |
| shouldInvertVertical | Boolean | false |

### 1.4 ReaderPreferences.FlashColor (enum, no fields), ReaderHideThreshold (enum, field `threshold: Int`).

### 1.5 ReaderPageImageView.Config (data class, `ReaderPageImageView.kt:427-433`)
| Field | Type | Nullable | Default |
|---|---|---|---|
| zoomDuration | Int (ms, unscaled) | no | none (required) |
| minimumScaleType | Int (SSIV constant) | no | `SCALE_TYPE_CENTER_INSIDE` (1) |
| cropBorders | Boolean | no | false |
| zoomStartPosition | `ReaderPageImageView.ZoomStartPosition` | no | `CENTER` |
| landscapeZoom | Boolean | no | false |

`ZoomStartPosition` enum: LEFT, CENTER, RIGHT (no field).

### 1.6 ViewerNavigation (abstract class, `ViewerNavigation.kt:101-158`)
- `NavigationRegion` sealed class: MENU, PREV, NEXT, LEFT, RIGHT, each with `nameRes: StringResource` and `color: Int` (ARGB Int from `android.graphics.Color.argb`).
- `Region` data class: `rectF: RectF` (normalized floats in [0,1] for both axes in all shipped layouts), `type: NavigationRegion`.
- `constantMenuRegion: RectF` = `RectF(0f, 0f, 1f, 0.05f)` (private, dead, see behavior §7.4).
- `invertMode: ReaderPreferences.TappingInvertMode` = NONE (public var).
- `regionList: List<Region>` abstract protected var (subclass geometry in area 44).

### 1.7 ViewerConfig (abstract class, `ViewerConfig.kt:13-90`)
| Field | Type | Initial | Source preference |
|---|---|---|---|
| imagePropertyChangedListener | `(() -> Unit)?` | null | — |
| navigationModeChangedListener | `(() -> Unit)?` | null | — |
| tappingInverted | TappingInvertMode | NONE | subclass |
| longTapEnabled | Boolean | true | readWithLongTap |
| usePageTransitions | Boolean | false | pageTransitions |
| doubleTapAnimDuration | Int | 500 | doubleTapAnimSpeed |
| volumeKeysEnabled | Boolean | false | readWithVolumeKeys |
| volumeKeysInverted | Boolean | false | readWithVolumeKeysInverted |
| alwaysShowChapterTransition | Boolean | true | alwaysShowChapterTransition |
| navigationMode | Int | 0 | subclass (pager/webtoon) |
| forceNavigationOverlay | Boolean | false, then from pref | showNavigationOverlayNewUser (written false) |
| navigationOverlayOnStart | Boolean | false | showNavigationOverlayOnStart |
| dualPageSplit | Boolean | false | subclass |
| dualPageInvert | Boolean | false | subclass |
| dualPageRotateToFit | Boolean | false | subclass |
| dualPageRotateToFitInvert | Boolean | false | subclass |
| navigator | ViewerNavigation | abstract | subclass |
| `scope` | CoroutineScope (ctor, private) | — | — |

### 1.8 ReaderSettingsScreenModel (class, `ReaderSettingsScreenModel.kt:14-25`)
| Field | Type | Nullable |
|---|---|---|
| readerState | StateFlow<ReaderViewModel.State> (ctor) | no |
| hasDisplayCutout | Boolean | no |
| onChangeReadingMode | (ReadingMode) -> Unit | no |
| onChangeOrientation | (ReaderOrientation) -> Unit | no |
| preferences | ReaderPreferences (default `Injekt.get()`) | no |
| viewerFlow | StateFlow<Viewer?> (initial null) | yes (value) |
| mangaFlow | StateFlow<Manga?> (initial null) | yes (value) |

`ReaderViewModel.State` fields used here: `viewer` (Viewer?), `manga` (Manga?) (part 1).

### 1.9 GestureDetectorWithLongTap (class, `GestureDetectorWithLongTap.kt`)
| Field | Type | Initial |
|---|---|---|
| handler | Handler (main looper) | new |
| slop | Int (px) | `scaledTouchSlop` |
| longTapTime | Long (ms) | `getLongPressTimeout()` |
| doubleTapTime | Long (ms) | `getDoubleTapTimeout()` |
| downX, downY | Float | 0f |
| lastUp | Long (eventTime ms) | 0L |
| lastDownEvent | MotionEvent? (copy, recycled on next DOWN) | null |

### 1.10 Chapter inputs to calculateChapterGap
- `ReaderChapter` (part 1 model) → `.chapter` (data Chapter) → `toDomainChapter()`.
- Domain `Chapter.chapterNumber: Double` (`domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt:14`); `isRecognizedNumber: Boolean get() = chapterNumber >= 0f` (line 19-20). Unrecognized chapters have negative numbers (the sentinel is -1 in practice).

### 1.11 ReaderTransitionView.Data (private data class)
`transition: ChapterTransition` (model from part 1), `currChapterDownloaded: Boolean`, `goingToChapterDownloaded: Boolean`. Both booleans non-null, default not applicable (always computed in `bind`).

### 1.12 DisplayRefreshHost state (presentation; consumes this scope's prefs)
`currentDisplayRefresh: Boolean` (mutableState), `flashInterval: Int` (from pref, min 1 in UI), `timesCalled: Int` (counter, reset by setInterval), `flashMillis: Flow<Int>`, `flashMode: Flow<FlashColor>`, `flashIntervalPref`. Full algorithm in behavior §14.

---

## 2. Preference keys: complete catalog

Storage: `PreferenceStore` (Android SharedPreferences in the app; `core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt`). Type column shows the Kotlin accessor type. Default column is the literal default in code. Consumer column lists files that read the key (within or outside the scope). "Kurobayomi key" is the planned name in the Rust store (same string, to keep backup compatibility; see §7).

### 2.1 Booleans (ReaderPreferences)
| Key | Accessor | Default | Consumers (repo-relative) |
|---|---|---|---|
| pref_enable_transitions_key | pageTransitions | true | ViewerConfig.kt:52 (usePageTransitions), presentation/more/settings/screen/SettingsReaderScreen.kt:63, presentation/reader/settings/GeneralSettingsPage.kt:91 |
| pref_reader_flash | flashOnPageChange | false | ReaderActivity.kt:369, :433; DisplayRefreshHost.kt; SettingsReaderScreen.kt:139 |
| pref_show_page_number_key | showPageNumber | true | GeneralSettingsPage.kt (checkbox), reader UI (part 1) |
| pref_show_reading_mode | showReadingMode | true | reader UI (part 1) |
| fullscreen | fullscreen | true | GeneralSettingsPage.kt (checkbox), ReaderActivity (part 1) |
| cutout_short | cutoutShort | true | GeneralSettingsPage.kt (only when hasDisplayCutout && fullscreen), ReaderActivity.kt:857 (changes) |
| pref_keep_screen_on_key | keepScreenOn | true | GeneralSettingsPage.kt:75 (label pref_keep_screen_on), ReaderActivity (part 1) |
| pref_enable_double_tap_zoom_webtoon | webtoonDoubleTapZoomEnabled | true | ReadingModePage.kt:207, SettingsReaderScreen.kt:398 |
| always_show_chapter_transition | alwaysShowChapterTransition | true | ViewerConfig.kt:64 |
| pref_preserve_reading_position | preserveReadingPosition | false | settings (SettingsReaderScreen.kt:202); consumer in part 1 ReaderViewModel |
| crop_borders | cropBorders | false | ReadingModePage.kt:107; ReaderActivity.kt:378 (cropBorderPaged) |
| navigate_pan | navigateToPan | true | ReadingModePage.kt:117; PagerConfig.kt:70 |
| landscape_zoom | landscapeZoom | true | ReadingModePage.kt:112; PagerConfig.kt:73 |
| crop_borders_webtoon | cropBordersWebtoon | false | ReaderActivity.kt:379; ReadingModePage.kt:176 |
| create_folder_per_manga | folderPerManga | false | SettingsReaderScreen.kt:439 (downloads, area 23) |
| skip_read | skipRead | false | ReaderViewModel.kt:162 (part 1), SettingsReaderScreen.kt:186 |
| skip_filtered | skipFiltered | true | MangaScreenModel.kt:148, :654; SettingsReaderScreen.kt:190 |
| skip_dupe | skipDupe | false | SettingsReaderScreen.kt:194 |
| webtoon_disable_zoom_out | webtoonDisableZoomOut | false | ReadingModePage.kt:211; SettingsReaderScreen.kt:402 |
| pref_dual_page_split | dualPageSplitPaged | false | PagerConfig.kt:86; ReadingModePage.kt:120; SettingsReaderScreen.kt:276 |
| pref_dual_page_invert | dualPageInvertPaged | false | PagerConfig.kt:95; ReadingModePage.kt:123; SettingsReaderScreen.kt:284 |
| pref_dual_page_split_webtoon | dualPageSplitWebtoon | false | PagerConfig/WebtoonConfig (area 44); ReadingModePage.kt:179; SettingsReaderScreen.kt:371 |
| pref_dual_page_invert_webtoon | dualPageInvertWebtoon | false | ReadingModePage.kt:188; SettingsReaderScreen.kt:379 |
| pref_dual_page_rotate | dualPageRotateToFit | false | PagerConfig.kt:98; ReadingModePage.kt:133; SettingsReaderScreen.kt:298 (via rotateToFitPref) |
| pref_dual_page_rotate_invert | dualPageRotateToFitInvert | false | PagerConfig.kt (init block); ReadingModePage.kt:129 |
| pref_dual_page_rotate_webtoon | dualPageRotateToFitWebtoon | false | ReadingModePage.kt:192 |
| pref_dual_page_rotate_invert_webtoon | dualPageRotateToFitInvertWebtoon | false | ReadingModePage.kt:201 |
| pref_custom_brightness_key | customBrightness | false | ColorFilterPage.kt:25-28; ReaderActivity.kt:865 |
| pref_color_filter_key | colorFilter | false | ReaderActivity (color overlay, area 44/part 1) |
| pref_grayscale | grayscale | false | ReaderActivity / color filter (part 1, area 44) |
| pref_inverted_colors | invertedColors | false | ReaderActivity / color filter (part 1, area 44) |
| reader_long_tap | readWithLongTap | true | ViewerConfig.kt:49 (longTapEnabled) |
| reader_volume_keys | readWithVolumeKeys | false | ViewerConfig.kt:58 (volumeKeysEnabled) |
| reader_volume_keys_inverted | readWithVolumeKeysInverted | false | ViewerConfig.kt:61 (volumeKeysInverted) |
| reader_navigation_overlay_new_user | showNavigationOverlayNewUser | true | ViewerConfig.kt:67-70 (written false) |
| reader_navigation_overlay_on_start | showNavigationOverlayOnStart | false | ViewerConfig.kt:72 (navigationOverlayOnStart) |

### 2.2 Integers (ReaderPreferences)
| Key | Accessor | Default | Range / meaning | Consumers |
|---|---|---|---|---|
| pref_reader_flash_duration | flashDurationMillis | MILLI_CONVERSION = 100 | ms; slider stores value*100 for value 1..15 (100..1500 ms) | DisplayRefreshHost.kt; SettingsReaderScreen.kt:127-141 |
| pref_reader_flash_interval | flashPageInterval | 1 | pages; slider 1..10 | DisplayRefreshHost.kt; SettingsReaderScreen.kt |
| pref_double_tap_anim_speed | doubleTapAnimSpeed | 500 | ms (unscaled base) | ViewerConfig.kt:55 (doubleTapAnimDuration) |
| pref_default_reading_mode_key | defaultReadingMode | `ReadingMode.RIGHT_TO_LEFT.flagValue` = 2 | ReadingMode flagValue | part 1 ReaderViewModel (resolution) |
| pref_default_orientation_type_key | defaultOrientationType | `ReaderOrientation.FREE.flagValue` = 8 (0x8) | ReaderOrientation flag (bits 3..5) | part 1 |
| pref_image_scale_type_key | imageScaleType | 1 | 1..6 (see behavior §3.5) | PagerConfig.kt:36-62; ReadingModePage.kt:83-88; SettingsReaderScreen.kt:212 |
| pref_zoom_start_key | zoomStart | 1 | 1..4 (Automatic/Left/Right/Center) | PagerConfig.kt:64; ReadingModePage.kt:94-99 |
| pref_reader_theme_key | readerTheme | 1 | 0 white, 1 black (default), 2 gray, 3 automatic | ReaderActivity.kt:840; PagerConfig.kt (init block); presentation/reader/settings/GeneralSettingsPage.kt (`themes` list, line 19) |
| webtoon_side_padding | webtoonSidePadding | WEBTOON_PADDING_MIN = 0 | 0..25 (WEBTOON_PADDING_MAX) | SettingsReaderScreen.kt:313-352; ReadingModePage.kt:162-169 |
| custom_brightness_value | customBrightnessValue | 0 | -75..100 | ColorFilterPage.kt:38-44; ReaderActivity.kt:939 |
| color_filter_value | colorFilterValue | 0 | ARGB int, set by the color picker | ReaderActivity (part 1 / area 44) |
| color_filter_mode | colorFilterMode | 0 | index into ColorFilterMode (0..2 pre-API 28, 0..5 with API 28 entries) | ColorFilterPage.kt:105-110; ReaderActivity.kt:373 |
| reader_navigation_mode_pager | navigationModePager | 0 | 0..5 tap-zone index | PagerConfig.kt:76-77; SettingsReaderScreen.kt:226 |
| reader_navigation_mode_webtoon | navigationModeWebtoon | 0 | 0..5 tap-zone index | WebtoonConfig.kt:55; ReadingModePage.kt:76 (TapZonesItems) |

### 2.3 Enum-valued keys (stored as enum NAME strings)
| Key | Accessor | Default | Allowed stored names | Consumers |
|---|---|---|---|---|
| pref_reader_flash_mode | flashColor | FlashColor.BLACK | BLACK, WHITE, WHITE_BLACK | DisplayRefreshHost.kt; GeneralSettingsPage.kt:118-121; SettingsReaderScreen.kt:163-170 |
| reader_hide_threshold | readerHideThreshold | ReaderHideThreshold.LOW | HIGHEST, HIGH, LOW, LOWEST | WebtoonViewer.kt:75-79; SettingsReaderScreen.kt:357 |
| reader_tapping_inverted | pagerNavInverted | TappingInvertMode.NONE | NONE, HORIZONTAL, VERTICAL, BOTH | PagerConfig.kt:79-84; pager settings |
| reader_tapping_inverted_webtoon | webtoonNavInverted | TappingInvertMode.NONE | NONE, HORIZONTAL, VERTICAL, BOTH | SettingsReaderScreen.kt (webtoon group, ListPreference) |

Unknown names fall back to the default silently (`getEnum` deserializer, `PreferenceStore.kt`).

### 2.4 Keys owned by sibling BasePreferences (read from this scope)
| Key | Accessor | Default | Consumer in scope |
|---|---|---|---|
| pref_always_decode_long_strip_with_ssiv | BasePreferences.alwaysDecodeLongStripWithSSIV | false | ReaderPageImageView.kt:63-65, :317 |
| pref_hardware_bitmap_threshold | BasePreferences.hardwareBitmapThreshold | `GLUtil.SAFE_TEXTURE_LIMIT` = 2048 | App.kt:146 → ImageUtil.hardwareBitmapThreshold → ReaderPageImageView.kt:256, ImageUtil.kt:330-333 |

Source: `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:40, 42`.

### 2.5 Count
`grep -c` over `ReaderPreferences.kt` gives 36 `getBoolean(` + 14 `getInt(` + 4 `getEnum(` = **54 keyed preferences**. §2.1 has 36 rows, §2.2 has 14 rows, §2.3 has 4 rows; every row is one call. Plus 2 BasePreferences keys read by this scope (§2.4). Note that `getString`, `getLong`, `getFloat`, `getStringSet` and `getObject` are not used by this file (enums use `getEnum`, which is `getObject` underneath).

---

## 3. Enum stored values and bit layouts

### 3.1 ReadingMode flags (stored in manga row, bits 0..2)
| Name | flagValue | Binary |
|---|---|---|
| DEFAULT | 0x0 | 000 |
| LEFT_TO_RIGHT | 0x1 | 001 |
| RIGHT_TO_LEFT | 0x2 | 010 |
| VERTICAL | 0x3 | 011 |
| WEBTOON | 0x4 | 100 |
| CONTINUOUS_VERTICAL | 0x5 | 101 |
MASK 0x7 (binary 111). Values 6 and 7 are not defined: `fromPreference` returns DEFAULT for them.

Manga-level: `Manga.readingMode = viewerFlags and 0x7` (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:17-18`). Writes: `viewerFlags.setFlag(flag, 0x7)` (`SetMangaViewerFlags.kt:17`).

### 3.2 ReaderOrientation flags (reference, part 1; bits 3..5)
FREE = 0x08, PORTRAIT = 0x10, LANDSCAPE = 0x18 (values 0x20, 0x28 exist in part 1 list), DEFAULT = 0x00; `MASK = 0x38` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt:14-65`). Included here because `defaultOrientationType` in this scope stores the FREE flag value.

### 3.3 FlashColor (stored name)
BLACK → Color.Black full-screen; WHITE → Color.White; WHITE_BLACK → White then Black (behavior §14).

### 3.4 TappingInvertMode (stored name) → inversion bits
| Name | horizontal | vertical |
|---|---|---|
| NONE | no | no |
| HORIZONTAL | yes | no |
| VERTICAL | no | yes |
| BOTH | yes | yes |

### 3.5 ReaderHideThreshold (stored name) → px
HIGHEST 5, HIGH 13, LOW 31 (default), LOWEST 47.

### 3.6 Integer enumerations
- imageScaleType: 1 CENTER_INSIDE (default), 2 CENTER_CROP, 3 FIT_WIDTH, 4 FIT_HEIGHT, 5 ORIGINAL_SIZE, 6 SMART_FIT (SSIV constant values, `SubsamplingScaleImageView.java:102-110`).
- zoomStart: 1 Automatic (default), 2 Left, 3 Right, 4 Center.
- readerTheme: 0 white, 1 black (default), 2 gray, 3 automatic (`presentation/reader/settings/GeneralSettingsPage.kt`, `themes` list at line 19).
- navigation mode (pager & webtoon): 0 default, 1 L shaped, 2 Kindle-ish, 3 Edge, 4 Right and Left, 5 Disabled.
- colorFilterMode index: 0 SrcOver (Default), 1 Modulate (Multiply), 2 Screen, 3 Overlay (API 28+), 4 Lighten (API 28+), 5 Darken (API 28+).

### 3.7 SSIV constants used (fork `tachiyomiorg/subsampling-scale-image-view@66e0db195d`)
ZOOM_FOCUS_CENTER = 2; EASE_OUT_QUAD = 1; EASE_IN_OUT_QUAD = 2; PAN_LIMIT_INSIDE = 1; SCALE_TYPE_CENTER_INSIDE = 1; SCALE_TYPE_CENTER_CROP = 2; SCALE_TYPE_FIT_WIDTH = 3; SCALE_TYPE_FIT_HEIGHT = 4; SCALE_TYPE_ORIGINAL_SIZE = 5; SCALE_TYPE_SMART_FIT = 6; SCALE_TYPE_CUSTOM = 7; ORIGIN_* not used in scope.

---

## 4. Persistent storage touched

| Storage | Location (Android) | Written by scope | Read by scope | Notes |
|---|---|---|---|---|
| SharedPreferences (app preference file) | app data dir, default `SharedPreferences` for the app | all setters in ReaderPreferences (via Preference.set) | all getters | Typed; mismatched type deletes the key (AndroidPreference.kt:35-38) |
| Manga row viewer flags | SQLite `mangas.viewer` column | not directly; via SetMangaViewerFlags (part 1/area 05) | `Manga.readingMode` | Bits 0..2 reading mode; 3..5 orientation |
| Page images | none (Coil caches disabled for page requests, `ReaderPageImageView.kt:326-327, 401-402`) | — | — | No disk cache for reader pages in this scope |
| Chapter download check | download folder (area 23) | — | `MangaDownloadManager.isChapterDownloaded(skipCache = true)` in ReaderTransitionView | Filesystem lookup each bind |
| Files on disk | none written by this scope | — | — | — |

Preference file name is the app default (not overridden in scope).

---

## 5. Network endpoints

None in this scope. Page bytes arrive as `BufferedSource` (non-animated) or `Any` data (animated, a `Drawable` or a source-bound request) from the part 1 loader (`ReaderViewModel` → page loader → source HTTP/ local). `ReaderPageImageView` only issues Coil requests with in-memory data. No HTTP headers, cookies or auth are set here (they live in area 12 and the source layer).

`alwaysDecodeLongStripWithSSIV` and hardware threshold do not hit the network.

---

## 6. Database tables and columns touched

| Table | Column | Access | Owner |
|---|---|---|---|
| mangas | viewer (INTEGER NOT NULL; Kotlin field `viewerFlags: Long`) | read via `Manga.readingMode` mask 0x7; written by `SetMangaViewerFlags` | area:03-db-manga-schema-migrations / area:05-data-repos-entries-items (`data/src/main/sqldelight/data/mangas.sq:21`) |

No other table is touched by this scope. Chapter numbers come from the `chapters` table through part 1 (area:05-data-repos-entries-items and area:07-domain-entries-items).

---

## 7. Linux storage plan (Proposal)

- Replace SharedPreferences with a `prefs` table in the Kurobayomi SQLite database: `CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL) STRICT;`. Values are stored as text: integers as decimal, booleans as `true`/`false`, enums as names (matches Aniyomi's string-enum storage), so backups can carry the same key/value pairs.
- Keep every key string identical to §2 (all 54 keys of §2.1 to §2.3 plus the two BasePreferences keys of §2.4) so that imported Aniyomi backup preference entries map one-to-one (area:21-backup-format-and-create and area:22-backup-restore decides the import mapping).
- Mismatched-type handling mirrors AndroidPreference: parse failure → delete row, return default.
- Manga viewer flags: keep the 0x7 / 0x38 layout unchanged in the Kurobayomi `manga.viewer_flags` INTEGER column, so the same mask helpers work and a future Aniyomi backup import preserves the value.
- Page images: no disk cache for reader pages (matches Coil DISABLED); decoded `gdk::Texture` lives only while the page is in the viewer window.
- Config file: none. Everything is in the DB, so backup covers it.
