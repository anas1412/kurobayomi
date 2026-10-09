# 43-reader-core (part 2 of 2) — File inventory

Area: **43-reader-core**, part 2 of 2. Scope = the 12 files printed by `python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 43-reader-core 2`.
Repo root for every path: `/home/blackbox/aniyomi-src` (repo-relative paths below).

Part 1 of this area (not documented here) holds `loader/`, `model/`, the root-level `ui/reader/*.kt` files (ReaderActivity, ReaderViewModel, ReaderOrientation.kt, ...) and `presentation/reader`. Names from part 1 that appear below are referenced only where this part's code calls them. Part 1 is documented by the other author of this area.

Port decision legend: **Port** = same behavior, re-expressed in Rust; **Rewrite** = same user-visible behavior, different mechanism; **Replace-with-crate** = library swapped for a named crate; **Drop** = Android-only, no Linux behavior.

Priority legend: P0 usable daily, P1 parity, P2 long-tail, P3 Android-only/obsolete (still has a task).

Declarations column lists every top-level and nested type/function/property in the file. Methods of `ReaderPreferences` are listed by name (every one).

## Summary table

| # | Full repo-relative path | LOC | All declarations in file | Purpose | Port decision | Target Rust module | Priority | Task ids |
|---|---|---|---|---|---|---|---|---|
| 1 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt | 255 | class `ReaderPreferences`; methods `pageTransitions`, `flashOnPageChange`, `flashDurationMillis`, `flashPageInterval`, `flashColor`, `doubleTapAnimSpeed`, `showPageNumber`, `showReadingMode`, `fullscreen`, `cutoutShort`, `keepScreenOn`, `defaultReadingMode`, `defaultOrientationType`, `webtoonDoubleTapZoomEnabled`, `imageScaleType`, `zoomStart`, `readerTheme`, `alwaysShowChapterTransition`, `preserveReadingPosition`, `cropBorders`, `navigateToPan`, `landscapeZoom`, `cropBordersWebtoon`, `webtoonSidePadding`, `readerHideThreshold`, `folderPerManga`, `skipRead`, `skipFiltered`, `skipDupe`, `webtoonDisableZoomOut`, `dualPageSplitPaged`, `dualPageInvertPaged`, `dualPageSplitWebtoon`, `dualPageInvertWebtoon`, `dualPageRotateToFit`, `dualPageRotateToFitInvert`, `dualPageRotateToFitWebtoon`, `dualPageRotateToFitInvertWebtoon`, `customBrightness`, `customBrightnessValue`, `colorFilter`, `colorFilterValue`, `colorFilterMode`, `grayscale`, `invertedColors`, `readWithLongTap`, `readWithVolumeKeys`, `readWithVolumeKeysInverted`, `navigationModePager`, `navigationModeWebtoon`, `pagerNavInverted`, `webtoonNavInverted`, `showNavigationOverlayNewUser`, `showNavigationOverlayOnStart`; enum `FlashColor` (BLACK, WHITE, WHITE_BLACK); enum `TappingInvertMode` (NONE, HORIZONTAL, VERTICAL, BOTH; props `titleRes`, `shouldInvertHorizontal`, `shouldInvertVertical`); enum `ReaderHideThreshold` (HIGHEST, HIGH, LOW, LOWEST; prop `threshold`); companion: `WEBTOON_PADDING_MIN`, `WEBTOON_PADDING_MAX`, `MILLI_CONVERSION`, `TapZones`, `ImageScaleType`, `ZoomStart`, `ColorFilterMode` | Typed access to every reader preference key, with defaults; enums for flash color, tap inversion, hide threshold; UI option lists | Rewrite (SharedPreferences → typed store in SQLite kv) | `src/reader/prefs.rs` (keys, defaults, typed getters), `src/reader/enums.rs` (enums + option lists) | P0 | T-43b-001, T-43b-002, T-43b-003, T-43b-004, T-43b-005, T-43b-006, T-43b-007, T-43b-008, T-43b-039, T-43b-042, T-43b-045, T-43b-047, T-43b-048, T-43b-049, T-43b-050, T-43b-053, T-43b-055, T-43b-056, T-43b-057 |
| 2 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderSettingsScreenModel.kt | 31 | class `ReaderSettingsScreenModel`; properties `viewerFlow`, `mangaFlow`; constructor values `hasDisplayCutout`, `onChangeReadingMode`, `onChangeOrientation`, `preferences` | Settings-sheet state holder: current viewer and manga as StateFlows; callbacks to change reading mode and orientation | Rewrite (Voyager ScreenModel → plain struct with `glib` signals) | `src/reader/settings_model.rs` | P0 | T-43b-030, T-43b-031 |
| 3 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt | 92 | enum `ReadingMode` (DEFAULT, LEFT_TO_RIGHT, RIGHT_TO_LEFT, VERTICAL, WEBTOON, CONTINUOUS_VERTICAL; props `stringRes`, `iconRes`, `flagValue`, `direction`, `type`); companion: `MASK`, `fromPreference`, `isPagerType`, `toViewer`; sealed interface `Direction` (data objects Horizontal, Vertical); sealed interface `ViewerType` (data objects Pager, Webtoon) | Reading-mode enum with flag values, direction, viewer-type, and the factory that creates the viewer | Rewrite | `src/reader/mode.rs` | P0 | T-43b-009, T-43b-010, T-43b-011 |
| 4 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/GestureDetectorWithLongTap.kt | 75 | open class `GestureDetectorWithLongTap` (ctor `context`, `listener`; private: `handler`, `slop`, `longTapTime`, `doubleTapTime`, `downX`, `downY`, `lastUp`, `lastDownEvent`, `longTapFn`; override `onTouchEvent`); open class `Listener` (open `onLongTapConfirmed(ev)`) | Adds a long-press (confirmed) callback to the built-in gesture detector, guarded against double taps | Rewrite (Android GestureDetector + Handler → GTK gesture state machine + glib timeout) | `src/reader/gesture.rs` | P0 | T-43b-012, T-43b-013, T-43b-052 |
| 5 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/MissingChapters.kt | 12 | top-level `fun calculateChapterGap(higherReaderChapter: ReaderChapter?, lowerReaderChapter: ReaderChapter?): Int` (import alias `domainCalculateChapterGap`) | Number of missing chapters between two reader chapters (wraps domain function) | Port | `src/reader/chapter_gap.rs` | P0 | T-43b-027 |
| 6 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderButton.kt | 29 | class `ReaderButton` (ctors `context`, `attrs`, `defStyleAttr`; `var viewer: PagerViewer?`; override `onTouchEvent`) | Clickable button inside the pager that disables the pager's tap gestures while touched | Rewrite (GTK buttons consume their own clicks; no pager-gesture toggling) | `src/reader/overlay_button.rs` | P2 | T-43b-028, T-43b-052 |
| 7 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt | 442 | open class `ReaderPageImageView` (ctor `context`, `attrs`, `defStyleAttrs`, `defStyleRes`, `isWebtoon`; private: `alwaysDecodeLongStripWithSSIV`, `pageView`, `config`, `landscapeZoom`(ext), `setupZoom`(ext), `canPan`, `pan`, `prepareNonAnimatedImageView`, `prepareAnimatedImageView`, `setNonAnimatedImage`, `setAnimatedImage`, `getSystemScaledDuration`; public: `onImageLoaded` (var + fun), `onImageLoadError` (var + fun), `onScaleChanged` (var + fun), `onViewClicked` (var + fun), `pageBackground`, `onPageSelected`, `setImage` (2 overloads), `recycle`, `canPanLeft`, `canPanRight`, `panLeft`, `panRight`); data class `Config` (zoomDuration, minimumScaleType, cropBorders, zoomStartPosition, landscapeZoom); enum `ZoomStartPosition` (LEFT, CENTER, RIGHT); file-private `const val MAX_ZOOM_SCALE = 5F` | Renders one page image: zoom/pan/fit, subsampled large-image view, animated image view, landscape auto-zoom, double tap | Rewrite (SubsamplingScaleImageView + PhotoView + Coil → GTK widget + gdk::Texture + image crate; see topic file) | `src/reader/page_view.rs`, `src/reader/page_zoom.rs`, `src/reader/page_decode.rs` | P0 | T-43b-014, T-43b-015, T-43b-016, T-43b-017, T-43b-018, T-43b-019, T-43b-020, T-43b-021, T-43b-022, T-43b-023, T-43b-024, T-43b-025, T-43b-026, T-43b-051, T-43b-059 |
| 8 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderProgressIndicator.kt | 58 | class `ReaderProgressIndicator` (ctor `context`, `attrs`, `defStyleAttr`; private `progress` state; override `Content`; funs `show`, `hide`, `setProgress(progress: Int)`) | Always-rotating loading spinner shown in the reader while a chapter loads, with optional percentage | Rewrite (AbstractComposeView → gtk::Spinner / DrawingArea) | `src/reader/progress.rs` | P0 | T-43b-029, T-43b-052 |
| 9 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderTransitionView.kt | 73 | class `ReaderTransitionView` (ctor `context`, `attrs`; private `data` state; `init`; fun `bind(transition, downloadManager, manga)`; override `Content`; private data class `Data` (transition, currChapterDownloaded, goingToChapterDownloaded)) | Hosts the chapter-transition panel (previous/next chapter card) and computes the downloaded flags it displays | Rewrite (AbstractComposeView → GTK widget) | `src/reader/transition.rs` | P0 | T-43b-032, T-43b-033, T-43b-052 |
| 10 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/Viewer.kt | 45 | interface `Viewer` (funs `getView`, `destroy` (default empty), `setChapters`, `moveToPage`, `handleKeyEvent`, `handleGenericMotionEvent`) | Contract every reader viewer (pager L2R/R2L/vertical, webtoon) implements | Port (Rust trait) | `src/reader/viewer.rs` | P0 | T-43b-034, T-43b-058 |
| 11 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerConfig.kt | 90 | abstract class `ViewerConfig` (ctor `readerPreferences`, `scope`; vars `imagePropertyChangedListener`, `navigationModeChangedListener`, `tappingInverted`, `longTapEnabled`, `usePageTransitions`, `doubleTapAnimDuration`, `volumeKeysEnabled`, `volumeKeysInverted`, `alwaysShowChapterTransition`, `navigationMode`, `forceNavigationOverlay`, `navigationOverlayOnStart`, `dualPageSplit`, `dualPageInvert`, `dualPageRotateToFit`, `dualPageRotateToFitInvert`, `navigator`; funs `defaultNavigation`, `updateNavigation`, extension `Preference<T>.register`) | Base configuration for all viewers, bound live to preferences | Rewrite (Flow.register → `ReaderPrefs` change subscriptions on the GTK main loop) | `src/reader/viewer_config.rs` | P0 | T-43b-035, T-43b-036, T-43b-041, T-43b-045 |
| 12 | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt | 69 | abstract class `ViewerNavigation`; sealed class `NavigationRegion` (data objects MENU, PREV, NEXT, LEFT, RIGHT; ctor `nameRes`, `color`); data class `Region` (`rectF`, `type`; fun `invert`); vars `constantMenuRegion`, `invertMode`; abstract `regionList`; funs `getRegions`, `getAction` | Tap-zone model: regions in normalized coordinates, inversion, hit-test to an action | Port | `src/reader/nav.rs` | P0 | T-43b-037, T-43b-038, T-43b-039, T-43b-040 |

Note on counts: the table gives one row per scope file; declarations are complete (see column 5 for each file).

## Per-file detail

### 1. ReaderPreferences.kt (255 LOC)

- Declared types: `class ReaderPreferences(private val preferenceStore: PreferenceStore)`; nested `enum class FlashColor`, `enum class TappingInvertMode(val titleRes: StringResource, val shouldInvertHorizontal: Boolean = false, val shouldInvertVertical: Boolean = false)`, `enum class ReaderHideThreshold(val threshold: Int)`; `companion object` with `const val WEBTOON_PADDING_MIN`, `const val WEBTOON_PADDING_MAX`, `const val MILLI_CONVERSION`, `val TapZones`, `val ImageScaleType`, `val ZoomStart`, `val ColorFilterMode`.
- Sections (region markers): General (lines 14-91), Split two page spread (93-120), Color filter (122-138), Controls (140-175), enums (177-203), companion (205-254).
- Every method is a one-line getter returning `Preference<T>` from `preferenceStore`. Key names, types and defaults are in `p2-data-and-config.md`.
- Port decision: keep the exact key strings (they are backup and migration-relevant; see area:21-backup-format-and-create and area:22-backup-restore). Replace `Preference<T>` with a typed handle `Pref<T>` (get/set/subscribe).

### 2. ReaderSettingsScreenModel.kt (31 LOC)

- `class ReaderSettingsScreenModel(readerState: StateFlow<ReaderViewModel.State>, val hasDisplayCutout: Boolean, val onChangeReadingMode: (ReadingMode) -> Unit, val onChangeOrientation: (ReaderOrientation) -> Unit, val preferences: ReaderPreferences = Injekt.get()) : ScreenModel`.
- `val viewerFlow` = `readerState.map { it.viewer }.distinctUntilChanged().stateIn(ioCoroutineScope, SharingStarted.Lazily, null)`.
- `val mangaFlow` = `readerState.map { it.manga }.distinctUntilChanged().stateIn(ioCoroutineScope, SharingStarted.Lazily, null)`.

### 3. ReadingMode.kt (92 LOC)

- Entries (ordinal, name, flagValue): DEFAULT 0x00000000, LEFT_TO_RIGHT 0x00000001, RIGHT_TO_LEFT 0x00000002, VERTICAL 0x00000003, WEBTOON 0x00000004, CONTINUOUS_VERTICAL 0x00000005.
- `MASK = 0x00000007`.
- `fromPreference(preference: Int?)` = `entries.find { it.flagValue == preference } ?: DEFAULT`.
- `isPagerType(preference: Int)` = `fromPreference(preference).type is ViewerType.Pager` (DEFAULT has null type, so false).
- `toViewer(preference: Int?, activity: ReaderActivity): Viewer` maps: LEFT_TO_RIGHT→`L2RPagerViewer(activity)`, RIGHT_TO_LEFT→`R2LPagerViewer(activity)`, VERTICAL→`VerticalPagerViewer(activity)`, WEBTOON→`WebtoonViewer(activity)`, CONTINUOUS_VERTICAL→`WebtoonViewer(activity, isContinuous = false)`, DEFAULT→throws `IllegalStateException("Preference value must be resolved: $preference")`.

### 4. GestureDetectorWithLongTap.kt (75 LOC)

- Subclass of `android.view.GestureDetector`. Fields: `handler = Handler(Looper.getMainLooper())`, `slop = ViewConfiguration.get(context).scaledTouchSlop`, `longTapTime = ViewConfiguration.getLongPressTimeout()`, `doubleTapTime = ViewConfiguration.getDoubleTapTimeout()`, `downX`, `downY`, `lastUp = 0L`, `lastDownEvent: MotionEvent?`.
- `longTapFn = Runnable { listener.onLongTapConfirmed(lastDownEvent!!) }`.
- Touch handling is in `onTouchEvent` (lines 35-63) and always ends with `super.onTouchEvent(ev)`.

### 5. MissingChapters.kt (12 LOC)

- Wrapper that converts two `ReaderChapter` values to domain chapters with `toDomainChapter()` (from `eu.kanade.tachiyomi.data.database.models.manga`) and calls `tachiyomi.domain.items.chapter.service.calculateChapterGap`.

### 6. ReaderButton.kt (29 LOC)

- `class ReaderButton @JvmOverloads constructor(context, attrs = null, defStyleAttr = R.attr.materialButtonStyle) : MaterialButton(...)`; `var viewer: PagerViewer? = null`; `onTouchEvent` disables the pager gesture detector on any event, re-enables it on `ACTION_UP`, and calls `super`.

### 7. ReaderPageImageView.kt (442 LOC)

- Full behavior is in `p2-behavior.md` section "Page image view" and `p2-topic-page-image-view.md`.

### 8. ReaderProgressIndicator.kt (58 LOC)

- `AbstractComposeView` with layout `FrameLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT, Gravity.CENTER)`, composition strategy `DisposeOnDetachedFromWindowOrReleasedFromPool`, body `TachiyomiTheme { CombinedCircularProgressIndicator(progress = { progress }) }`.

### 9. ReaderTransitionView.kt (73 LOC)

- `class ReaderTransitionView @JvmOverloads constructor(context, attrs = null) : AbstractComposeView(context, attrs)`; state `data: Data?`; `layoutParams = LayoutParams(WRAP_CONTENT, WRAP_CONTENT)`.
- Imports two different `ChapterTransition` symbols: `eu.kanade.presentation.reader.ChapterTransition` (a composable function) and `eu.kanade.tachiyomi.ui.reader.model.ChapterTransition` (the model type). Kotlin resolves the function call and the type reference separately.

### 10. Viewer.kt (45 LOC)

- `interface Viewer` with `getView(): View`, `destroy() {}` (default no-op), `setChapters(chapters: ViewerChapters)`, `moveToPage(page: ReaderPage)`, `handleKeyEvent(event: KeyEvent): Boolean`, `handleGenericMotionEvent(event: MotionEvent): Boolean`.

### 11. ViewerConfig.kt (90 LOC)

- Full behavior in `p2-behavior.md` section "ViewerConfig".

### 12. ViewerNavigation.kt (69 LOC)

- Full behavior in `p2-topic-tap-navigation.md`.

## Caller map (outside scope, read to make this spec exact)

| Symbol in scope | Caller outside the scope (repo-relative) |
|---|---|
| `ReaderPreferences` | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt (settings screen), app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt, app/src/main/java/eu/kanade/presentation/reader/settings/ReadingModePage.kt, app/src/main/java/eu/kanade/presentation/reader/settings/ColorFilterPage.kt, app/src/main/java/eu/kanade/presentation/reader/DisplayRefreshHost.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt (skipFiltered), app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt (threshold) |
| `ReaderSettingsScreenModel` | app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt:355, app/src/main/java/eu/kanade/presentation/reader/settings/ReaderSettingsDialog.kt, app/src/main/java/eu/kanade/presentation/reader/ReadingModeSelectDialog.kt, app/src/main/java/eu/kanade/presentation/reader/OrientationSelectDialog.kt |
| `ReadingMode` | app/src/main/java/eu/kanade/presentation/reader/ReadingModeSelectDialog.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt (`readingMode` = `viewerFlags and ReadingMode.MASK`), app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetMangaViewerFlags.kt |
| `GestureDetectorWithLongTap` | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/Pager.kt:33, :50 |
| `calculateChapterGap` (viewer package) | app/src/main/java/eu/kanade/presentation/reader/ChapterTransition.kt:68, :80 (the details screen `MangaScreenModel.kt:1180` uses the domain function directly) |
| `ReaderButton` | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerTransitionHolder.kt:124 (retry button) |
| `ReaderPageImageView` | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt:165 (Config construction), app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt:200, app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaCoverDialog.kt:163, app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeImagesDialog.kt:201, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt:39, :115-122 (`ZoomStartPosition` mapping) |
| `ReaderProgressIndicator` | app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt:137, :559 |
| `ReaderTransitionView` | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerTransitionHolder.kt:61 (only user in this checkout) |
| `Viewer` | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt (`viewer?.handleGenericMotionEvent`) |
| `ViewerConfig` | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt:26, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonConfig.kt |
| `ViewerNavigation` | app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderNavigationOverlayView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/*.kt (DisabledNavigation, EdgeNavigation, KindlishNavigation, LNavigation, RightAndLeftNavigation), app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt:140, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonConfig.kt:110 |

## Inventory self-check

Every path printed by the scope command appears verbatim in the summary table above (see the grep self-check in the task log).
