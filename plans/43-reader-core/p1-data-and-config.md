# 43-reader-core: data models, preferences, files, endpoints, tables

All paths are repo-relative to `/home/blackbox/aniyomi-src`. Sections 1-2 describe in-memory models; 3 lists preference keys; 4 files on disk; 5 network; 6 database; 7 bit layouts.

---

## 1. In-memory models

### 1.1 `ReaderChapter` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ReaderChapter.kt:9-51`)

| Field | Type | Nullability | Default | Notes |
|---|---|---|---|---|
| `chapter` | `eu.kanade.tachiyomi.data.database.models.manga.Chapter` (interface, impl `ChapterImpl`) | non-null | constructor arg | database model; equality = `ChapterImpl.equals` (class + url + id) |
| `stateFlow` | `MutableStateFlow<ReaderChapter.State>` | non-null | `State.Wait` | source of truth for `state` |
| `state` | `ReaderChapter.State` (get/set through `stateFlow.value`) | non-null | `Wait` | |
| `pages` | `List<ReaderPage>?` (computed) | nullable | null unless `Loaded` | |
| `pageLoader` | `PageLoader?` | nullable | null | |
| `requestedPage` | Int | non-null | 0 | page index the viewer should show first |
| `references` | Int (private) | non-null | 0 | ref count |

`ReaderChapter.State` variants: `Wait` (data object), `Loading` (data object), `Error(error: Throwable)` (data class), `Loaded(pages: List<ReaderPage>)` (data class).

### 1.2 `Page` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt:10-58`)

| Field | Type | Nullability | Default | Serialised |
|---|---|---|---|---|
| `index` | Int | non-null | constructor | yes (`@Serializable`) |
| `url` | String | non-null | `""` | yes |
| `imageUrl` | String? (var) | nullable | null | yes |
| `uri` | `android.net.Uri?` (var) | nullable | null | `@Transient` (deprecated, kept for extensions) |
| `number` | Int (computed) | non-null | `index + 1` | no |
| `status` | `Page.State` (var, backed by `MutableStateFlow`) | non-null | `QUEUE` | `@Transient` |
| `progress` | Int (var, backed by `MutableStateFlow`) | non-null | 0 | `@Transient` |

`Page.State` ordinal values: `QUEUE` (0), `LOAD_PAGE` (1), `DOWNLOAD_IMAGE` (2), `READY` (3), `ERROR` (4). The ordinal is not persisted in this scope.

`progress` after `update(bytesRead, contentLength, done)`: `100 * bytesRead / contentLength` (integer division) when `contentLength > 0`, else `-1`.

### 1.3 `ReaderPage` (`RM/ReaderPage.kt:6-14`)

Extends `Page`. Adds:
- `stream: (() -> InputStream)?` (var). Null until the page is READY. For HTTP pages it is `{ chapterCache.getImageFile(imageUrl).inputStream() }`. For local pages it re-opens the file or archive entry on each call.
- `chapter: ReaderChapter` (`open lateinit var`). Must be set before any access; throws `UninitializedPropertyAccessException` otherwise.

### 1.4 `InsertPage` (`RM/InsertPage.kt:3-11`)

Extends `ReaderPage`. Constructor `(parent: ReaderPage)`. Copies `index`, `url`, `imageUrl`, `chapter` (`parent.chapter`), `stream` (`parent.stream`). `status = READY` at construction.

### 1.5 `ViewerChapters` (`RM/ViewerChapters.kt:3-20`)

`data class ViewerChapters(val currChapter: ReaderChapter, val prevChapter: ReaderChapter?, val nextChapter: ReaderChapter?)`. No defaults.

### 1.6 `ChapterTransition` (`RM/ChapterTransition.kt:3-35`)

`sealed class` with `abstract val from: ReaderChapter` (non-null) and `abstract val to: ReaderChapter?` (nullable). Subclasses `Prev(from, to)` and `Next(from, to)`. No defaults. Equality rules in `p1-behavior.md` section 1.5.

### 1.7 `ReaderViewModel.State` (`R/ReaderViewModel.kt:956-977`)

See `p1-behavior.md` section 4.3 for the field table.

### 1.8 `ReaderViewModel.Dialog`, `Event`, `SetAsCoverResult`, `SaveImageResult` (`R/ReaderViewModel.kt:903-996`)

- `Dialog`: `Loading`, `Settings`, `ReadingModeSelect`, `OrientationModeSelect` (data objects); `PageActions(page: ReaderPage)`.
- `Event`: `ReloadViewerChapters`, `PageChanged` (data objects); `SetOrientation(orientation: Int)`, `SetCoverResult(result: SetAsCoverResult)`, `SavedImage(result: SaveImageResult)`, `ShareImage(uri: Uri, page: ReaderPage)`, `CopyImage(uri: Uri)`.
- `SetAsCoverResult` enum: `Success`, `AddToLibraryFirst`, `Error`.
- `SaveImageResult` sealed: `Success(uri: Uri)`, `Error(error: Throwable)`.

### 1.9 `ReaderOrientation` (`RS/ReaderOrientation.kt:14-69`)

Enum with fields `flag: Int` (Android `ActivityInfo.SCREEN_ORIENTATION_*`), `stringRes: StringResource`, `icon: ImageVector`, `flagValue: Int` (stored). Values in `p1-behavior.md` section 1.6. Companion: `MASK = 0x00000038`, `fromPreference(Int?)`.

Android `ActivityInfo` numeric values used as `flag` (only relevant to the Android port; the Linux port drops orientation locking, see `p1-rust-design.md`): `SCREEN_ORIENTATION_UNSPECIFIED`, `SCREEN_ORIENTATION_SENSOR_PORTRAIT`, `SCREEN_ORIENTATION_SENSOR_LANDSCAPE`, `SCREEN_ORIENTATION_PORTRAIT`, `SCREEN_ORIENTATION_LANDSCAPE`, `SCREEN_ORIENTATION_REVERSE_PORTRAIT`.

### 1.10 `ReadingMode` (cross-area, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt`)

| Name | `flagValue` | `direction` | `type` | `iconRes` | `stringRes` |
|---|---|---|---|---|---|
| `DEFAULT` | `0x00000000` | null | null | `ic_reader_default_24dp` | `label_default` |
| `LEFT_TO_RIGHT` | `0x00000001` | Horizontal | Pager | `ic_reader_ltr_24dp` | `left_to_right_viewer` |
| `RIGHT_TO_LEFT` | `0x00000002` | Horizontal | Pager | `ic_reader_rtl_24dp` | `right_to_left_viewer` |
| `VERTICAL` | `0x00000003` | Vertical | Pager | `ic_reader_vertical_24dp` | `vertical_viewer` |
| `WEBTOON` | `0x00000004` | Vertical | Webtoon | `ic_reader_webtoon_24dp` | `webtoon_viewer` |
| `CONTINUOUS_VERTICAL` | `0x00000005` | Vertical | Webtoon | `ic_reader_continuous_vertical_24dp` | `vertical_plus_viewer` |

`MASK = 0x00000007`. Viewer construction: `toViewer` maps `LEFT_TO_RIGHT` -> `L2RPagerViewer`, `RIGHT_TO_LEFT` -> `R2LPagerViewer`, `VERTICAL` -> `VerticalPagerViewer`, `WEBTOON` -> `WebtoonViewer`, `CONTINUOUS_VERTICAL` -> `WebtoonViewer(isContinuous = false)` (`ReadingMode.kt:69-80`).

### 1.11 `ChapterLoader` constructor (`RL/ChapterLoader.kt:25-31`)

`ChapterLoader(context: Context, downloadManager: MangaDownloadManager, downloadProvider: MangaDownloadProvider, manga: Manga, source: MangaSource)`. Field `readerPreferences: ReaderPreferences` (lazy inject).

### 1.12 `HttpPageLoader` constructor (`RL/HttpPageLoader.kt:32-39`)

`HttpPageLoader(chapter: ReaderChapter, source: HttpSource, chapterCache: ChapterCache = Injekt.get(), sourcePreferences: SourcePreferences = Injekt.get())`. Internal state: `scope`, `queue: PriorityBlockingQueue<PriorityPage>`, `preloadSize = 4`, `dataSaver`.

`PriorityPage(page: ReaderPage, priority: Int)`: `identifier` from a process-wide `AtomicInteger` (`companion`). Comparison in `p1-behavior.md` section 3.6.

### 1.13 `ArchivePageLoader`, `DirectoryPageLoader`, `EpubPageLoader`, `DownloadPageLoader`, `PageLoader`

Constructors: `ArchivePageLoader(reader: ArchiveReader)`, `DirectoryPageLoader(file: UniFile)` (`val file`), `EpubPageLoader(reader: EpubReader)`, `DownloadPageLoader(chapter, manga, source, downloadManager, downloadProvider)`, base `PageLoader` (no constructor args). `isLocal` is `true` for the first three and for `DownloadPageLoader`, `false` for `HttpPageLoader`.

### 1.14 `SaveImageNotifier` (`R/SaveImageNotifier.kt:25`)

`SaveImageNotifier(context: Context)`. Fields: `notificationBuilder` (from `Notifications.CHANNEL_COMMON`), `notificationId = Notifications.ID_DOWNLOAD_IMAGE`.

### 1.15 `ReaderPreferences.FlashColor` and `TappingInvertMode` (`RS/ReaderPreferences.kt`, cross-area)

- `FlashColor` enum: `BLACK`, `WHITE`, `WHITE_BLACK`. Stored by `getEnum` (name-based; area:02-preferences-system).
- `TappingInvertMode(titleRes, shouldInvertHorizontal, shouldInvertVertical)`: `NONE(tapping_inverted_none, false, false)`, `HORIZONTAL(tapping_inverted_horizontal, true, false)`, `VERTICAL(tapping_inverted_vertical, false, true)`, `BOTH(tapping_inverted_both, true, true)`.
- `ReaderHideThreshold(threshold: Int)`: `HIGHEST(5)`, `HIGH(13)`, `LOW(31)`, `LOWEST(47)`. Default `LOW` (`ReaderPreferences.kt:79`). Used by the viewer (area:44-reader-viewers), listed for completeness.
- Constants: `WEBTOON_PADDING_MIN = 0`, `WEBTOON_PADDING_MAX = 25`, `MILLI_CONVERSION = 100` (`ReaderPreferences.kt:206-209`).

---

## 2. Domain models touched

### 2.1 `Manga` (domain, `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt`)

Relevant members: `viewerFlags: Long` (stored in `mangas.viewer`), `chapterFlags: Long` (stored in `mangas.chapter_flags`), `title`, `source`, `id`, `favorite`. Computed (`Manga.kt:40-80`):
- `sorting = chapterFlags and 0x300`
- `displayMode = chapterFlags and 0x00100000`
- `unreadFilterRaw = chapterFlags and 0x6`
- `downloadedFilterRaw = chapterFlags and 0x18`
- `bookmarkedFilterRaw = chapterFlags and 0x60`
- `sortDescending() = (chapterFlags and 0x1) == 0x0`

App extensions (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:17-21`):
- `Manga.readingMode: Long = viewerFlags and 0x7`
- `Manga.readerOrientation: Long = viewerFlags and 0x38`

`Manga.create()` defaults: `id = -1L`, `source = -1L`, `favorite = false` (`Manga.kt:82+`).

### 2.2 `Chapter` (domain, `domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt`)

Fields: `id: Long`, `mangaId: Long`, `read: Boolean`, `bookmark: Boolean`, `lastPageRead: Long`, `dateFetch: Long`, `sourceOrder: Long`, `url: String`, `name: String`, `dateUpload: Long`, `chapterNumber: Double`, `scanlator: String?`, `lastModifiedAt: Long`, `version: Long`, computed `isRecognizedNumber: Boolean` (line 19).

### 2.3 `Chapter` (database model, `app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/Chapter.kt`)

Interface with `id: Long?`, `manga_id: Long?`, `read: Boolean`, `bookmark: Boolean`, `last_page_read: Int`, `date_fetch: Long`, `source_order: Int`, `last_modified: Long`, `version: Long` (`:9-29`), plus `SChapter` fields (`name`, `url`, `scanlator`, `chapter_number: Float`, `date_upload`). `isRecognizedNumber: chapter_number >= 0f` (`:30-31`). Implementation `ChapterImpl` (`ChapterImpl.kt`): `last_page_read: Int = 0`, `chapter_number: Float = 0f`.

### 2.4 `ChapterUpdate` (domain, `domain/src/main/java/tachiyomi/domain/items/chapter/model/ChapterUpdate.kt`, area:07-domain-entries-items)

Partial update with nullable fields: `id: Long` (required), `read: Boolean?`, `lastPageRead: Long?`, `bookmark: Boolean?`. Null fields are not written. This scope uses:
- progress: `id`, `read`, `lastPageRead`;
- duplicates: `id`, `read = true`;
- bookmark: `id`, `bookmark`.

### 2.5 `MangaHistoryUpdate` (domain, area:08-domain-library-category-history-updates)

Constructed as `MangaHistoryUpdate(chapterId: Long, readAt: Date, sessionReadDuration: Long)`. `sessionReadDuration` is milliseconds.

### 2.6 `SChapter.chapter_number` and `source_order`

`chapter_number: Float` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SChapter.kt:15`). `source_order: Int` in the database model, `sourceOrder: Long` in the domain model.

---

## 3. Preference keys used by this scope (with defaults and types)

Owner in parentheses. All keys are read through `PreferenceStore` (area:02-preferences-system). `getBoolean`/`getInt`/`getString`/`getEnum`/`getStringSet`/`getLong` are the store methods.

### 3.1 `ReaderPreferences` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt`, area:02-preferences-system owns the file; keys and defaults listed here because this scope reads them)

| Method | Key | Type | Default | Line |
|---|---|---|---|---|
| `pageTransitions()` | `pref_enable_transitions_key` | Boolean | true | 16 |
| `flashOnPageChange()` | `pref_reader_flash` | Boolean | false | 18 |
| `flashDurationMillis()` | `pref_reader_flash_duration` | Int (ms) | `MILLI_CONVERSION` = 100 | 20 |
| `flashPageInterval()` | `pref_reader_flash_interval` | Int | 1 | 22 |
| `flashColor()` | `pref_reader_flash_mode` | enum `FlashColor` | `BLACK` | 24 |
| `doubleTapAnimSpeed()` | `pref_double_tap_anim_speed` | Int | 500 | 26 (viewer) |
| `showPageNumber()` | `pref_show_page_number_key` | Boolean | true | 28 |
| `showReadingMode()` | `pref_show_reading_mode` | Boolean | true | 30 |
| `fullscreen()` | `fullscreen` | Boolean | true | 32 |
| `cutoutShort()` | `cutout_short` | Boolean | true | 34 |
| `keepScreenOn()` | `pref_keep_screen_on_key` | Boolean | true | 36 |
| `defaultReadingMode()` | `pref_default_reading_mode_key` | Int | `RIGHT_TO_LEFT.flagValue` = 2 | 38-41 |
| `defaultOrientationType()` | `pref_default_orientation_type_key` | Int | `FREE.flagValue` = 8 | 43-46 |
| `webtoonDoubleTapZoomEnabled()` | `pref_enable_double_tap_zoom_webtoon` | Boolean | true | 48-51 |
| `imageScaleType()` | `pref_image_scale_type_key` | Int (1..6) | 1 | 53 |
| `zoomStart()` | `pref_zoom_start_key` | Int (1..4) | 1 | 55 |
| `readerTheme()` | `pref_reader_theme_key` | Int (0,1,2,3) | 1 | 57 |
| `alwaysShowChapterTransition()` | `always_show_chapter_transition` | Boolean | true | 59-62 |
| `preserveReadingPosition()` | `pref_preserve_reading_position` | Boolean | false | 64-67 |
| `cropBorders()` | `crop_borders` | Boolean | false | 69 |
| `navigateToPan()` | `navigate_pan` | Boolean | true | 71 |
| `landscapeZoom()` | `landscape_zoom` | Boolean | true | 73 |
| `cropBordersWebtoon()` | `crop_borders_webtoon` | Boolean | false | 75 |
| `webtoonSidePadding()` | `webtoon_side_padding` | Int (0..25) | `WEBTOON_PADDING_MIN` = 0 | 77 |
| `readerHideThreshold()` | `reader_hide_threshold` | enum `ReaderHideThreshold` | `LOW` | 79 |
| `folderPerManga()` | `create_folder_per_manga` | Boolean | false | 81 |
| `skipRead()` | `skip_read` | Boolean | false | 83 |
| `skipFiltered()` | `skip_filtered` | Boolean | true | 85 |
| `skipDupe()` | `skip_dupe` | Boolean | false | 87 |
| `webtoonDisableZoomOut()` | `webtoon_disable_zoom_out` | Boolean | false | 89 |
| `dualPageSplitPaged()` | `pref_dual_page_split` | Boolean | false | 95 |
| `dualPageInvertPaged()` | `pref_dual_page_invert` | Boolean | false | 97 |
| `dualPageSplitWebtoon()` | `pref_dual_page_split_webtoon` | Boolean | false | 99 |
| `dualPageInvertWebtoon()` | `pref_dual_page_invert_webtoon` | Boolean | false | 101 |
| `dualPageRotateToFit()` | `pref_dual_page_rotate` | Boolean | false | 103 |
| `dualPageRotateToFitInvert()` | `pref_dual_page_rotate_invert` | Boolean | false | 105-108 |
| `dualPageRotateToFitWebtoon()` | `pref_dual_page_rotate_webtoon` | Boolean | false | 110-113 |
| `dualPageRotateToFitInvertWebtoon()` | `pref_dual_page_rotate_invert_webtoon` | Boolean | false | 115-118 |
| `customBrightness()` | `pref_custom_brightness_key` | Boolean | false | 124 |
| `customBrightnessValue()` | `custom_brightness_value` | Int (-75..100) | 0 | 126 |
| `colorFilter()` | `pref_color_filter_key` | Boolean | false | 128 |
| `colorFilterValue()` | `color_filter_value` | Int (packed ARGB) | 0 | 130 |
| `colorFilterMode()` | `color_filter_mode` | Int (0..5) | 0 | 132 |
| `grayscale()` | `pref_grayscale` | Boolean | false | 134 |
| `invertedColors()` | `pref_inverted_colors` | Boolean | false | 136 |
| `readWithLongTap()` | `reader_long_tap` | Boolean | true | 142 |
| `readWithVolumeKeys()` | `reader_volume_keys` | Boolean | false | 144 (viewer) |
| `readWithVolumeKeysInverted()` | `reader_volume_keys_inverted` | Boolean | false | 146-149 (viewer) |
| `navigationModePager()` | `reader_navigation_mode_pager` | Int (0..5) | 0 | 151 |
| `navigationModeWebtoon()` | `reader_navigation_mode_webtoon` | Int (0..5) | 0 | 153 |
| `pagerNavInverted()` | `reader_tapping_inverted` | enum `TappingInvertMode` | `NONE` | 155-158 |
| `webtoonNavInverted()` | `reader_tapping_inverted_webtoon` | enum `TappingInvertMode` | `NONE` | 160-163 |
| `showNavigationOverlayNewUser()` | `reader_navigation_overlay_new_user` | Boolean | true | 165-168 |
| `showNavigationOverlayOnStart()` | `reader_navigation_overlay_on_start` | Boolean | false | 170-173 |

### 3.2 `BasePreferences` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt`, area:02-preferences-system)

| Method | Key | Type | Default | Line |
|---|---|---|---|---|
| `downloadedOnly()` | `Preference.appStateKey("pref_downloaded_only")` | Boolean | false | 16-19 |
| `incognitoMode()` | `Preference.appStateKey("incognito_mode")` | Boolean | false | 21 |
| `displayProfile()` | `pref_display_profile_key` | String (file URI) | `""` | 38 |

`Preference.appStateKey(x)` marks app-state (not user-setting) keys; it is a naming helper from `core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt`.

### 3.3 `DownloadPreferences` (`domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt`, area:09-domain-source-track-download-release-misc)

| Method | Key | Type | Default | Line |
|---|---|---|---|---|
| `autoDownloadWhileReading()` | `auto_download_while_reading` | Int | 0 | 25 |
| `removeAfterReadSlots()` | `remove_after_read_slots` | Int | -1 | 28 |
| `saveChaptersAsCBZ()` | `save_chapter_as_cbz` | Boolean | true | (not read here) |
| `splitTallImages()` | `split_tall_images` | Boolean | true | (not read here) |

### 3.4 `LibraryPreferences` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt`, area:08-domain-library-category-history-updates)

| Method | Key | Type | Default | Line |
|---|---|---|---|---|
| `markDuplicateReadChapterAsRead()` | `mark_duplicate_read_chapter_read` | StringSet | empty | 377 |

Accepted values (constants, `:408-409`): `MARK_DUPLICATE_CHAPTER_READ_NEW = "new"`, `MARK_DUPLICATE_CHAPTER_READ_EXISTING = "existing"`. The reader checks only `"existing"`.

### 3.5 `TrackPreferences` (`app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`, area:27-tracking-core)

| Method | Key | Type | Default | Line |
|---|---|---|---|---|
| `autoUpdateTrack()` | `pref_auto_update_manga_sync_key` | Boolean | true | 39 |
| `autoUpdateTrackOnMarkRead()` | `getEnum(...)` (key and type not re-read here) | enum | not re-read | 48 |

The reader checks only `autoUpdateTrack()` before calling `TrackChapter.await` (`R/ReaderViewModel.kt:920`). `TrackChapter.await` itself does not check `autoUpdateTrackOnMarkRead`. It returns early when the manga has no tracks, and it only updates trackers where `chapterNumber > track.lastChapterRead` (`app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt:24-35`). Whether `autoUpdateTrackOnMarkRead` gates the reader path is therefore NOT decided in this scope; it is an open point for area:27-tracking-core and is listed in `p1-edge-cases-and-tests.md`.

### 3.6 `SourcePreferences` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt`, area:17-source-app-layer and area:12-network-stack)

Used only through `DataSaver`:

| Method | Key | Type | Default | Line |
|---|---|---|---|---|
| `dataSaver()` | `data_saver` | enum `DataSaver` (NONE, BANDWIDTH_HERO, WSRV_NL, RESMUSH_IT) | `NONE` | 97 |
| `dataSaverIgnoreJpeg()` | `ignore_jpeg` | Boolean | false | 99 |
| `dataSaverIgnoreGif()` | `ignore_gif` | Boolean | true | 101 |
| `dataSaverImageQuality()` | `data_saver_image_quality` | Int | 80 | 103 |
| `dataSaverImageFormatJpeg()` | `data_saver_image_format_jpeg` | Boolean | false | 105-108 |
| `dataSaverServer()` | `data_saver_server` | String | `""` | 110 |
| `dataSaverColorBW()` | `data_saver_color_bw` | Boolean | false | 112 |
| `dataSaverExcludedSources()` | `data_saver_excluded` | StringSet | empty | 114 |
| `dataSaverDownloader()` | `data_saver_downloader` | Boolean | true | 116 (downloader area:23-downloads-manga) |

### 3.7 Summary of the keys this scope WRITES

| Key | Written by | When |
|---|---|---|
| `chapter_id` (SavedState) | `ReaderViewModel.chapterId` | every chapter change |
| `page_index` (SavedState) | `ReaderViewModel.chapterPageIndex` | every page change |
| `pref_reader_theme_key`, `pref_show_page_number_key`, `fullscreen`, `pref_keep_screen_on_key`, `pref_reader_flash*`, `custom_brightness_value`, `color_filter_value`, `color_filter_mode`, `pref_grayscale`, `pref_inverted_colors`, `pref_dual_page_*` (3.1) | `ReaderSettingsDialog` pages (`P/settings/*`) | user changes a control in the reader settings sheet |
| `reader_navigation_mode_pager`, `reader_navigation_mode_webtoon`, `reader_tapping_inverted*` (3.1) | `TapZonesItems` (`P/settings/ReadingModePage.kt:215-243`) | user changes a tap-zone chip |
| `mangas.viewer` (DB) | `SetMangaViewerFlags.awaitSetReadingMode` / `awaitSetOrientation` (`app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetMangaViewerFlags.kt`) | reading mode or orientation chosen for this manga |
| `chapters.read`, `chapters.last_page_read`, `chapters.bookmark` (DB) | `UpdateChapter` with `ChapterUpdate` (section 2.4) | page change, completion, duplicate marking, bookmark toggle |
| `history` (DB) | `UpsertMangaHistory` with `MangaHistoryUpdate` (section 2.5) | pause, chapter change |
| `mark_duplicate_read_chapter_read` (3.4) | not written here | |

---

## 4. Files and directories on disk

| Path | Purpose | Created by | Source |
|---|---|---|---|
| `context.cacheDir/chapter_disk_cache/` | `DiskLruCache` directory for page lists and images. App version 1, value count 1, max size 100 MiB | `ChapterCache` init | `app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt:31-39`, `:203-209` |
| `<key>.0` inside the cache dir | One entry per md5 key; holds the page-list JSON or image bytes | `DiskLruCache` | `ChapterCache.kt:116-134`, `:144-160` |
| `context.cacheDir/shared_image/` | Last shared page image, cleared before each share (`deleteRecursively`) | `cacheImageDir` | `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FileExtensions.kt:8-9`, `R/ReaderViewModel.kt:862-864` |
| Pictures directory (`Location.Pictures`), optional sub folder `buildValidFilename(manga.title)` when `create_folder_per_manga` is on | Saved pages | `ImageSaver` (area:13-core-util-storage-archive-torrent and area:35-data-misc-updater-notification-export) | `R/ReaderViewModel.kt:816-831` |
| Download folder `<downloads>/<source>/<manga>/<chapter>/` or `<chapter>.cbz` | Downloaded chapters read by `DownloadPageLoader` | `MangaDownloadProvider` / `MangaDownloadCache` | `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadProvider.kt:82-92`, `MangaDownloadCache.kt:380` |
| ICC display profile | Path stored in `pref_display_profile_key` | user | `R/ReaderActivity.kt:895-909` |

Cache key derivations (exact):
- Page-list key: `DiskUtil.hashKeyForDisk("${chapter.mangaId}${chapter.url}")` (`ChapterCache.kt:197-199`).
- Image key: `DiskUtil.hashKeyForDisk(imageUrl)` (`ChapterCache.kt:130-134`); file name is that md5 + `".0"`.
- `DiskUtil.hashKeyForDisk(key) = Hash.md5(key)` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:32-34`).

Filename rules for saved pages:
- Base: `"${manga.title} - ${chapter.name}"` truncated to `MAX_FILE_NAME_BYTES (250) - suffix.byteSize()` bytes (`DiskUtil.kt:143`), then `DiskUtil.buildValidFilename`, then suffix `" - ${page.number}"`.

Image file extension rule for saves: the extension is taken from the image MIME type when it matches an `ImageType`, else from magic-byte detection, else `"jpg"` (`core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:65-69`). The helper function name at that location was not re-read.

---

## 5. Network endpoints touched by this scope

The reader does not define its own HTTP endpoints. Source requests are performed by the source implementation (area:15-source-api-manga and area:16-source-api-anime) through `HttpSource`. The reader calls these methods:

| Call site | Method (Kotlin) | Source-level signature | HTTP | Owner |
|---|---|---|---|---|
| `HttpPageLoader.getPages` (`RL/HttpPageLoader.kt:79`) | `source.getPageList(chapter)` | `HttpSource.getPageList(SChapter): List<Page>` | defined per source | area:15-source-api-manga |
| `HttpPageLoader.internalLoadPage` (`:190`) | `source.getImageUrl(page)` | `HttpSource.getImageUrl(Page): String` | defined per source | area:15-source-api-manga |
| `HttpPageLoader.internalLoadPage` (`:196`) | `source.getImage(page, dataSaver)` | `HttpSource.getImage(Page): Response` (extension in `aniyomi/util/DataSaver.kt:30-41`) | defined per source (usually GET of `page.imageUrl`) | area:15-source-api-manga |
| `ReaderViewModel.getChapterUrl` (`R/ReaderViewModel.kt:635`) | `source.getChapterUrl(SChapter)` | `HttpSource.getChapterUrl(SChapter): String` | none (string builder) | area:15-source-api-manga |

### 5.1 Data saver (`app/src/main/java/aniyomi/util/DataSaver.kt`)

Selected in `fun DataSaver(source, preferences)` (`:43-54`): if the mode is not `NONE` and the source id is in `dataSaverExcludedSources`, use `NoOp`; otherwise by mode.

`DataSaver.NoOp.compress(url) = url`.

**BANDWIDTH_HERO** (`:56-96`):
- Precondition: `dataSaverServer` trimmed of trailing `/` is not blank, and the image URL does not already contain that server.
- JPEG (`.jpeg` or `.jpg`, case-insensitive): if `ignore_jpeg` then original, else proxy. GIF (`.gif`): if `ignore_gif` then original, else proxy. Other: proxy.
- Proxy URL: `"$server/?jpg=$format&l=$quality&bw=$colorBW&url=$escaped"`, where `format` = `"1"` if `data_saver_image_format_jpeg` else `"0"`, `quality` = `data_saver_image_quality`, `colorBW` = `"1"`/`"0"` from `data_saver_color_bw`, `escaped = URLEncoder.encode(url, "utf-8")`.
- The proxied URL is then passed to the source's normal `getImage` path (`DataSaver.kt:30-41` swaps `page.imageUrl` for the call and restores it afterwards). The HTTP verb is whatever the source's `getImage` uses.

**WSRV_NL** (`:98-140`):
- Proxy base `https://wsrv.nl/?url=$imageUrl` (the URL is NOT encoded in this path).
- If URL contains `.webp` or `.gif` (case-insensitive): if `format` (JPEG conversion) is off -> `&q=$quality&n=-1`; else `&output=jpg&q=$quality&n=-1`.
- Otherwise: if `format` on -> `&output=jpg&q=$quality`; else `&output=webp&q=$quality`.
- JPEG/GIF ignore rules as above.

**RESMUSH_IT** (`:142-170`):
- Request: `GET http://api.resmush.it/ws.php?img=$imageUrl&qlty=$quality` (plain HTTP, the URL is not encoded). Executed synchronously with the app's OkHttp client.
- Response parsing: `body.string().substringAfter("\"dest\":\"").substringBefore("\",")`. The compressed URL is the value of the `dest` JSON field (string search, not a JSON parser).
- JPEG/GIF ignore rules as above.

Errors: none caught in `compress`; an exception propagates into `internalLoadPage`, which sets the page to `ERROR`.

### 5.2 Page-list cache JSON

`chapterCache.putPageListToCache` serialises `List<Page>` with the injected `kotlinx.serialization.json.Json` instance (`ChapterCache.kt:81-111`). Only the constructor properties `index`, `url`, `imageUrl` are persistent (the `uri`, `statusFlow`, `progressFlow`, `status`, `progress` members are `@Transient`, `Page.kt:10-30`). The shape is a JSON array of objects with those three keys. Whether `null` `imageUrl` is written or omitted depends on the `Json` configuration (`encodeDefaults`), which was not re-read here. The Rust port uses its own format (section 9 of `p1-rust-design.md`), so this detail matters only for reading an existing Android cache, which is out of scope.

---

## 6. Database tables and columns touched

Schema files: `data/src/main/sqldelight/data/chapters.sq`, `mangas.sq`, `history.sq`.

### 6.1 `chapters` (`data/src/main/sqldelight/data/chapters.sq:3-21`)

| Column | Type | Null | Default | Written by this scope |
|---|---|---|---|---|
| `_id` | INTEGER PK | no | | identity |
| `manga_id` | INTEGER FK -> `mangas(_id)` ON DELETE CASCADE | no | | read |
| `url` | TEXT | no | | read (page-list cache key, chapter URL) |
| `name` | TEXT | no | | read (filenames, download lookup) |
| `scanlator` | TEXT | yes | | read (download lookup, dedupe) |
| `read` | INTEGER AS Boolean | no | | written (completion, duplicates) |
| `bookmark` | INTEGER AS Boolean | no | | written (toggle) |
| `last_page_read` | INTEGER | no | | written (every page change, except ERROR pages, not in incognito) |
| `chapter_number` | REAL | no | | read (duplicates, gap, track) |
| `source_order` | INTEGER | no | | read (SOURCE sort) |
| `date_fetch` | INTEGER | no | | read via domain |
| `date_upload` | INTEGER | no | | read (UPLOAD_DATE sort) |
| `last_modified_at` | INTEGER | no | 0 | not written here |
| `version` | INTEGER | no | 0 | not written here |
| `is_syncing` | INTEGER | no | 0 | not written here |

Writes are issued through `UpdateChapter` with `ChapterUpdate` (section 2.4). The exact SQL is in area:05-data-repos-entries-items.

### 6.2 `mangas` (`data/src/main/sqldelight/data/mangas.sq:3-22`)

| Column | Type | Null | Written by this scope |
|---|---|---|---|
| `viewer` (Kotlin `viewerFlags`) | INTEGER | no | yes, `SetMangaViewerFlags` (only the bits of the mask being changed) |
| `chapter_flags` | INTEGER | no | read (sorting, filters) |

Update query used for both flag writes (`mangas.sq:171`): `viewer = coalesce(:viewer, viewer)`. The write computes the full new `viewer` value on the domain side (section 7.1), so the coalesce keeps the other bits.

### 6.3 `history` (`data/src/main/sqldelight/data/history.sq:1-12`)

| Column | Type | Null | Notes |
|---|---|---|---|
| `_id` | INTEGER PK | no | |
| `chapter_id` | INTEGER UNIQUE FK -> `chapters(_id)` ON DELETE CASCADE | no | one history row per chapter (the UNIQUE constraint) |
| `last_read` | INTEGER AS Date | yes | `readAt` timestamp, milliseconds |
| `time_read` | INTEGER | no | session duration accumulated, milliseconds |

Upsert semantics come from `UpsertMangaHistory` (area:08-domain-library-category-history-updates and area:06-data-repos-history-track-source-misc); the UNIQUE constraint on `chapter_id` means one row per chapter, updated on each flush.

### 6.4 Other tables the cascade touches (not written here)

- `manga_sync` (tracks), read by `TrackChapter.await` through `getTracks` (area:27-tracking-core).
- `history` is also read by the global "last history" query used by `GetNextChapters.await(onlyUnread)` (domain layer, area:08-domain-library-category-history-updates).

---

## 7. Bit layouts

### 7.1 `mangas.viewer` = `Manga.viewerFlags` (Long, low 6 bits meaningful)

```
bit:   7 6 5 4 3 2 1 0
mask:            0x38 0x07
field:  .  .  orientation(3)  reading_mode(3)
```

- Reading mode (`0x07`): `0 DEFAULT`, `1 LEFT_TO_RIGHT`, `2 RIGHT_TO_LEFT`, `3 VERTICAL`, `4 WEBTOON`, `5 CONTINUOUS_VERTICAL`. Values `6` and `7` are unused and decode to `DEFAULT` via `fromPreference`.
- Orientation (`0x38`): `0x0 DEFAULT`, `0x8 FREE`, `0x10 PORTRAIT`, `0x18 LANDSCAPE`, `0x20 LOCKED_PORTRAIT`, `0x28 LOCKED_LANDSCAPE`, `0x30 REVERSE_PORTRAIT`. Value `0x38` is unused and decodes to `DEFAULT`.
- Writes: `new = (old and mask.inv()) or (flag and mask)` (`app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetMangaViewerFlags.kt:24-29`). Both values are in-place; bits outside `0x3F` are preserved.
- Note: `ReaderOrientation.flagValue` values are already positioned in bits 3..5. Writing them with the mask `0x38` is correct. `ReadingMode.flagValue` values are positioned in bits 0..2 with mask `0x07`.

### 7.2 `mangas.chapter_flags` = `Manga.chapterFlags` (Long)

| Bits | Mask | Meaning | Values |
|---|---|---|---|
| bit 0 | `0x00000001` | sort direction | `0x0` = descending (`CHAPTER_SORT_DESC`), `0x1` = ascending (`CHAPTER_SORT_ASC`) |
| bits 1-2 | `0x00000006` | unread filter | `0x0` none, `0x2` `CHAPTER_SHOW_UNREAD`, `0x4` `CHAPTER_SHOW_READ` |
| bits 3-4 | `0x00000018` | downloaded filter | `0x0` none, `0x8` `CHAPTER_SHOW_DOWNLOADED`, `0x10` `CHAPTER_SHOW_NOT_DOWNLOADED` |
| bits 5-6 | `0x00000060` | bookmark filter | `0x0` none, `0x20` `CHAPTER_SHOW_BOOKMARKED`, `0x40` `CHAPTER_SHOW_NOT_BOOKMARKED` |
| bits 8-9 | `0x00000300` | sort mode | `0x0` SOURCE, `0x100` NUMBER, `0x200` UPLOAD_DATE, `0x300` ALPHABET |
| bit 20 | `0x00100000` | display mode | `0x0` NAME, `0x00100000` NUMBER |

Source: `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:83-110`. `SHOW_ALL = 0x0`.

Important: `sortDescending()` (`Manga.kt:71-73`) is `chapterFlags and 0x1 == 0x0`, i.e. bit 0 CLEAR means descending. The reader ignores it (always `false`) and uses the `sortDescending = false` comparators (`p1-behavior.md` section 5.1).

### 7.3 Chapter flags unused in this scope

`CHAPTER_DISPLAY_*` (bit 20) affects the chapter list display only (area:37-ui-library and area:38-ui-entry-details-manga).

### 7.4 Reader orientation and reading mode defaults as stored integers

| Preference | Stored value | Meaning |
|---|---|---|
| `pref_default_reading_mode_key` default | `2` | RIGHT_TO_LEFT |
| `pref_default_orientation_type_key` default | `8` | FREE |
| `pref_reader_theme_key` default | `1` | black background |
| `pref_image_scale_type_key` default | `1` | fit screen |
| `pref_zoom_start_key` default | `1` | automatic |

---

## 8. Stored value translations for enum-like integers used by this scope

| Setting | Stored int | Label key |
|---|---|---|
| Reader theme | 0 | `white_background` |
| Reader theme | 1 | `black_background` (default) |
| Reader theme | 2 | `gray_background` |
| Reader theme | 3 | `automatic_background` |
| Tap zone (`reader_navigation_mode_*`) | 0 | `label_default` |
| Tap zone | 1 | `l_nav` |
| Tap zone | 2 | `kindlish_nav` |
| Tap zone | 3 | `edge_nav` |
| Tap zone | 4 | `right_and_left_nav` |
| Tap zone | 5 | `disabled_nav` (hides the inversion row) |
| Image scale type | 1..6 | `scale_type_fit_screen`, `scale_type_stretch`, `scale_type_fit_width`, `scale_type_fit_height`, `scale_type_original_size`, `scale_type_smart_fit` |
| Zoom start | 1..4 | `zoom_start_automatic`, `zoom_start_left`, `zoom_start_right`, `zoom_start_center` |
| Colour filter mode | 0..5 | see `p1-behavior.md` section 18 |
| Flash colour | enum name | `BLACK` / `WHITE` / `WHITE_BLACK` |
| Tapping invert | enum name | `NONE` / `HORIZONTAL` / `VERTICAL` / `BOTH` |

Note: the page-indicator stored `currentPage` in state is 1-based, the DB `last_page_read` is 0-based (`ReaderViewModel.kt:532` vs `:538`).

---

## 9. Constants table (exact values)

| Constant | Value | Source |
|---|---|---|
| `HttpPageLoader.preloadSize` | 4 | `RL/HttpPageLoader.kt:48` |
| `PriorityPage` priorities | 0 preload, 1 visible, 2 retry | `RL/HttpPageLoader.kt:110`, `:132`, `:173` |
| `FADE_DURATION` | 1000L | `R/ReaderNavigationOverlayView.kt:116` |
| `grayBackgroundColor` | `Color.rgb(0x20, 0x21, 0x25)` | `R/ReaderActivity.kt:834` |
| Toolbar alpha | 230 (dark) / 242 (light) | `R/ReaderActivity.kt:498-501` |
| Bar alpha | 0.9f dark / 0.95f light | `P/appbars/ReaderAppBars.kt:72`, `P/components/ChapterNavigator.kt:64` |
| `animationSpec` | `tween<IntOffset>(200)` | `P/appbars/ReaderAppBars.kt:35` |
| Notification image size | 720 x 1280 | `R/SaveImageNotifier.kt:39` |
| Text sizes (overlay) | 64f (fill and stroke), stroke width 8f | `R/ReaderNavigationOverlayView.kt:54-62` |
| Page indicator | `Color(235,235,235)` fill, `Color(45,45,45)` outline, outline width 4f, `letterSpacing 1.sp` | `P/PageIndicatorText.kt:26-35` |
| Overlay colours | `Color.Black` (brightness), `Color(color)` (filter) | `P/ReaderContentOverlay.kt:34`, `:44` |
| Colour matrix (invert) | `[-1 0 0 0 255; 0 -1 0 0 255; 0 0 -1 0 255; 0 0 0 1 0]` | `R/ReaderActivity.kt:818-827` |
| Colour matrix (grayscale) | `setSaturation(0f)` | `R/ReaderActivity.kt:813-815` |
| Brightness sample | 100 ms | `R/ReaderActivity.kt:940` |
| Brightness minimum | 0.01f | `R/ReaderActivity.kt:961` |
| Brightness override none | `WindowManager.LayoutParams.BRIGHTNESS_OVERRIDE_NONE` (-1) | `R/ReaderActivity.kt:963` |
| Cutout short edges | `LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES` / `_NEVER` | `R/ReaderActivity.kt:915-916` |
| Keys | `KEYCODE_N` next, `KEYCODE_P` previous | `R/ReaderActivity.kt:309-313` |
| Download-ahead threshold | `> 0.25` | `R/ReaderViewModel.kt:454` |
| `MAX_FILE_NAME_BYTES` | 250 | `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:143` |
| Chapter cache size | `100L * 1024 * 1024` | `ChapterCache.kt:209` |
| Chapter cache app version / value count | 1 / 1 | `ChapterCache.kt:203`, `:206` |
| Unknown chapter number | `-1` (`isRecognizedNumber` false) | `app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/Chapter.kt:30-31` |
| Webtoon padding range | 0..25 | `ReaderPreferences.kt:206-207` |
| Brightness slider range | -75..100 | `P/settings/ColorFilterPage.kt:41` |
| Colour slider range | 0..255 | `P/settings/ColorFilterPage.kt:58`, `:82`, `:94` |
| Flash duration slider | 1..15 (x100 ms) | `P/settings/GeneralSettingsPage.kt:101` |
| Flash interval slider | 1..10 | `P/settings/GeneralSettingsPage.kt:109` |
| Tablet padding | 24 dp / 8 dp | `P/components/ChapterNavigator.kt:57` |
| Chapter transition card width | 460 dp | `P/ChapterTransition.kt:100` |
| Chapter transition name lines | 5 | `P/ChapterTransition.kt:239` |
| Scanlator lines | 2 | `P/ChapterTransition.kt:264` |
| Gap warning threshold | `> 0` | `P/ChapterTransition.kt:120` |
| Transition spacer | 24 dp | `P/ChapterTransition.kt:279` |
| Dialog max height | 0.75 x max height | `P/settings/ReaderSettingsDialog.kt:38` |
| Dialog dim | 0.5f / 0f (custom filter tab) | `P/settings/ReaderSettingsDialog.kt:49-53` |

---

## 10. Data classes equality and hashing (for the Rust port)

| Type | Kotlin equality | Rust equivalent required |
|---|---|---|
| `ReaderChapter` | data class over `chapter` (`ChapterImpl.equals`: class + url + id) | identity by chapter id AND url |
| `ViewerChapters` | data class over three `ReaderChapter?` | structural over ids |
| `ChapterTransition` | custom (section 1.5 of behavior) | symmetric by unordered pair + direction |
| `Manga` | data class (domain) | structural (not used as a key here) |
| `Page` | class (default identity) | identity |
| `ReaderOrientation`/`ReadingMode` | enum identity | enum |

---

## 11. Serialisation and persistence summary for this scope

| Data | Persisted where | Survives restart | Survives process death |
|---|---|---|---|
| Current chapter id | `SavedStateHandle["chapter_id"]` | per session | yes (saved-state bundle) |
| Current page index | `SavedStateHandle["page_index"]` | per session | yes |
| Last read page, read flag | `chapters` table | yes | yes |
| Bookmark | `chapters.bookmark` | yes | yes |
| Reading mode, orientation per manga | `mangas.viewer` | yes | yes |
| Page list of an online chapter | disk cache (`chapter_disk_cache`) | until evicted (100 MiB LRU) | yes |
| Downloaded images | disk cache | until evicted | yes |
| Pending chapter deletions | download manager's pending deleter (area:09-domain-source-track-download-release-misc) | yes | yes |
| Share image | `shared_image/` cache dir | until next share | yes |
| Reading history | `history` table | yes | yes |
