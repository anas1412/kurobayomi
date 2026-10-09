# 44-reader-viewers: behaviour specification

OBSERVED = what the Kotlin in `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/` does today.
PROPOSAL = what the Rust port should do. Proposals are labelled `Proposal:`.

Paths are repo-relative to `/home/blackbox/aniyomi-src`. `V/` below is shorthand for `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/`.

Viewers in this area are used only for manga. Anime uses the player (area:16/area:19 scope) and does not touch these files.

---

## 0. Viewer selection

OBSERVED. `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt:69-79`:

| ReadingMode | flag value | viewer constructed |
|---|---|---|
| DEFAULT | 0x00000000 | not constructible (throws `IllegalStateException("Preference value must be resolved: ...")`); the caller resolves the default first |
| LEFT_TO_RIGHT | 0x00000001 | `L2RPagerViewer(activity)` |
| RIGHT_TO_LEFT | 0x00000002 | `R2LPagerViewer(activity)` |
| VERTICAL | 0x00000003 | `VerticalPagerViewer(activity)` |
| WEBTOON | 0x00000004 | `WebtoonViewer(activity)` (isContinuous = true) |
| CONTINUOUS_VERTICAL | 0x00000005 | `WebtoonViewer(activity, isContinuous = false)` ("Long strip with gaps") |

`ReadingMode.MASK = 0x00000007`. `ReaderActivity.updateViewer()` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt:533`) chooses the viewer from `viewModel.getMangaReadingMode()`.

Proposal: an enum `ViewerKind { LeftToRight, RightToLeft, Vertical, Webtoon, WebtoonGapped }` with the same flag values, and a factory `fn make_viewer(kind) -> Box<dyn ViewerEngine>`.

---

## 1. Shared configuration and preference propagation

### 1.1 ViewerConfig (common base)

OBSERVED. `V/ViewerConfig.kt`. Constructor `ViewerConfig(readerPreferences, scope)`. Initial values, then the `register` hookup:

| Field | Initial value (Kotlin) | Preference key | Preference default | Registration |
|---|---|---|---|---|
| longTapEnabled | true | `reader_long_tap` | true | `register({ longTapEnabled = it })` |
| usePageTransitions | false | `pref_enable_transitions_key` | true | `register({ usePageTransitions = it })` |
| doubleTapAnimDuration | 500 | `pref_double_tap_anim_speed` | 500 | `register({ doubleTapAnimDuration = it })` |
| volumeKeysEnabled | false | `reader_volume_keys` | false | `register({ volumeKeysEnabled = it })` |
| volumeKeysInverted | false | `reader_volume_keys_inverted` | false | `register({ volumeKeysInverted = it })` |
| alwaysShowChapterTransition | true | `always_show_chapter_transition` | true | `register({ alwaysShowChapterTransition = it })` |
| forceNavigationOverlay | false | `reader_navigation_overlay_new_user` | true | read once at construction; if true, the pref is set to false |
| navigationOverlayOnStart | false | `reader_navigation_overlay_on_start` | false | `register({ navigationOverlayOnStart = it })` |
| tappingInverted | `TappingInvertMode.NONE` | set by subclasses | NONE | subclass |
| navigationMode | 0 | set by subclasses | 0 | subclass |
| dualPageSplit / dualPageInvert / dualPageRotateToFit / dualPageRotateToFitInvert | false | set by subclasses | false | subclass |

`register` (`V/ViewerConfig.kt:80-89`) is `changes().onEach { valueAssignment(it) }.distinctUntilChanged().onEach { onChanged(it) }.launchIn(scope)`. `changes()` emits the current value first. Evidence: `drop(1)` is used wherever a change-only reaction is wanted (`PagerConfig.kt:79-84`, `WebtoonConfig.kt:286-291`, `WebtoonConfig.kt:323-327`).

Proposal: each setting is a `tokio::sync::watch::Receiver<T>`. The consumer reads the current value on creation (this replaces the first emission) and reacts to `changed()` afterwards (this replaces `drop(1)`). `distinctUntilChanged` is the `watch` semantics for equal values, which must be enforced explicitly with a `PartialEq` check before send.

### 1.2 Which setting reacts how (complete table)

OBSERVED, from `PagerConfig.kt` and `WebtoonConfig.kt`. "assign" = assignment only. "refresh" = `imagePropertyChangedListener` (pager: `refreshAdapter()`; webtoon: `refreshAdapter()`). "nav" = `navigationModeChangedListener` (overlay re-applied). "recreate" = `themeChangedListener` (webtoon: `ActivityCompat.recreate`). "zoomprop" = `zoomPropertyChangedListener`. "dtz" = `doubleTapZoomChangedListener`.

| Pref key | Default | Pager reaction | Webtoon reaction |
|---|---|---|---|
| reader_navigation_mode_pager | 0 | assign navigationMode, then `updateNavigation(navigationMode)` (which calls nav) | n/a |
| reader_navigation_mode_webtoon | 0 | n/a | assign, then `updateNavigation(it)` |
| reader_tapping_inverted | NONE | assign tappingInverted; `navigator.invertMode = it`; nav on change (not initial) | n/a |
| reader_tapping_inverted_webtoon | NONE | n/a | assign tappingInverted; `navigator.invertMode`; nav on change (not initial) |
| pref_image_scale_type_key | 1 | assign imageScaleType; refresh | not read (webtoon forces FIT_WIDTH) |
| pref_zoom_start_key | 1 | assign imageZoomType via `zoomTypeFromPreference`; refresh | not read |
| pref_reader_theme_key | 1 (Black) | assign theme and automaticBackground (= theme==3); refresh | assign theme (no refresh); recreate on change (not initial) |
| crop_borders | false | assign imageCropBorders; refresh | not read |
| crop_borders_webtoon | false | not read | assign imageCropBorders; refresh |
| navigate_pan | true | assign navigateToPan | not read |
| landscape_zoom | true | assign landscapeZoom; refresh | not read (stays false) |
| pref_dual_page_split | false | assign dualPageSplit; refresh; on change also `dualPageSplitChangedListener(it)` | not read |
| pref_dual_page_split_webtoon | false | not read | assign dualPageSplit; refresh |
| pref_dual_page_invert | false | assign dualPageInvert; refresh | not read |
| pref_dual_page_invert_webtoon | false | not read | assign dualPageInvert; refresh |
| pref_dual_page_rotate | false | assign dualPageRotateToFit; refresh | not read |
| pref_dual_page_rotate_webtoon | false | not read | assign dualPageRotateToFit; refresh |
| pref_dual_page_rotate_invert | false | assign dualPageRotateToFitInvert; refresh | not read |
| pref_dual_page_rotate_invert_webtoon | false | not read | assign dualPageRotateToFitInvert; refresh |
| webtoon_side_padding | 0 | not read | assign sidePadding; refresh |
| webtoon_disable_zoom_out | false | not read | assign zoomOutDisabled; zoomprop(it) |
| pref_enable_double_tap_zoom_webtoon | true | not read | assign doubleTapZoom; dtz(it) |
| reader_hide_threshold | LOW | not in ViewerConfig; read once in `WebtoonViewer` (see §7.6) | same |

Notes:
- The pager's `register` for `pref_dual_page_split` calls `imagePropertyChangedListener?.invoke()` and then `dualPageSplitChangedListener?.invoke(it)` (`PagerConfig.kt:86-93`). When `it == false`, the pager removes all InsertPages (`PagerViewer.kt:132-136`, `cleanupPageSplit`).
- On the very first emission of `register` (construction time), the listeners are still null in `PagerViewer` init order. In practice the refresh is a no-op at construction. Proposal: the refresh is not triggered until the page list exists.
- `zoomprop` for webtoon uses `frame.zoomOutDisabled` and `recycler.zoomOutDisabled` (`WebtoonViewer.kt:156-158`).

### 1.3 Preference enums and value spaces used here

OBSERVED, `V/ViewerConfig.kt`, `ReaderPreferences.kt`.

- `TappingInvertMode` (enum with `titleRes`, `shouldInvertHorizontal`, `shouldInvertVertical`):
  - `NONE(MR.strings.tapping_inverted_none)` : h=false, v=false
  - `HORIZONTAL(MR.strings.tapping_inverted_horizontal, shouldInvertHorizontal = true)` : h=true, v=false
  - `VERTICAL(MR.strings.tapping_inverted_vertical, shouldInvertVertical = true)` : h=false, v=true
  - `BOTH(MR.strings.tapping_inverted_both, true, true)`
  - Stored as the enum name by `getEnum`.
- `ReaderHideThreshold(threshold: Int)`: `HIGHEST(5)`, `HIGH(13)`, `LOW(31)`, `LOWEST(47)`. Default `LOW`.
- Navigation mode ints (`reader_navigation_mode_*`): 0 = Default, 1 = L shaped, 2 = Kindle-ish, 3 = Edge, 4 = Right and Left, 5 = Disabled. Any other value falls back to `defaultNavigation()`.
- Image scale type (`pref_image_scale_type_key`): stored value = list index + 1 (`SettingsReaderScreen` and `ReadingModePage.kt:85-88` write `index + 1`). Values: 1 Fit screen, 2 Stretch, 3 Fit width, 4 Fit height, 5 Original size, 6 Smart fit. Default 1.
- Zoom start (`pref_zoom_start_key`): stored value = index + 1 (`ReadingModePage.kt:96-99`). 1 Automatic, 2 Left, 3 Right, 4 Center. Default 1.
- Reader theme (`pref_reader_theme_key`): 0 White, 1 Black (default), 2 Gray, 3 Automatic. Source of truth for the menu order is `SettingsReaderScreen.kt:91-97`: 1 Black, 2 Gray, 0 White, 3 Automatic.

### 1.4 Reader background context

OBSERVED. `app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:119-145` (`createReaderThemeContext`):
- `isDarkBackground` = theme 1 or 2 → true; theme 3 → if app ThemeMode is SYSTEM then `isNightMode()`, else `themeMode == ThemeMode.DARK`; anything else (0) → false.
- The context gets a `Configuration.UI_MODE_NIGHT_YES/NO` override and the app theme styles. Page holders and transitions are created with this themed context (`PagerViewerAdapter.kt:40`, `WebtoonAdapter.kt:32`). `refresh()` recomputes it.

Proposal: a `ReaderPalette { background: Rgba, foreground: Rgba, dark: bool }` computed from the same four rules. `dark` is used for the transition card text colour (§11).

---

## 2. Tap zones (navigation regions)

### 2.1 Coordinate space

OBSERVED. Regions are `RectF(left, top, right, bottom)` in normalised view coordinates, 0 to 1, where x = 0 is the left edge of the viewer view and y = 0 is the top edge.

Pager tap point (`PagerViewer.kt:104-112`):
`pos = ((rawX - viewLocationOnScreen.x + viewLocationInWindow.x) / pager.width, (rawY - ... ) / pager.height)`.
Effectively the touch point relative to the pager, divided by its width/height.

Webtoon tap point (`WebtoonViewer.kt:114-122`): same formula relative to `recycler`, but the y divisor is `recycler.originalHeight`, not `recycler.height`. `originalHeight` is the height captured on the first `onMeasure` (`WebtoonRecyclerView.kt:55-61`) and does not change when the strip is zoomed. Observed consequence: zone boundaries stay fixed on screen while zoomed.

### 2.2 Hit test

OBSERVED. `ViewerNavigation.getAction(pos)` (`V/ViewerNavigation.kt:59-68`):
1. `regions = getRegions()` = `regionList.map { it.invert(invertMode) }`.
2. `region = regions.find { it.rectF.contains(x, y) }` (first match in list order; `RectF.contains` is half-open: `left <= x < right` and `top <= y < bottom`, Android semantics).
3. If found, return `region.type`. Otherwise return `MENU`. (The `constantMenuRegion` check at line 65 returns MENU as well, so it has no effect. It is a dead branch.)

Inversion (`RectFExtensions.kt:6-20`):
- neither: unchanged
- horizontal only: `(1 - right, top, 1 - left, bottom)`
- vertical only: `(left, 1 - bottom, right, 1 - top)`
- both: `(1 - right, 1 - bottom, 1 - left, 1 - top)`

### 2.3 Region tables (exact)

All values are `RectF(left, top, right, bottom)`.

**Mode 0 Default** resolves via `defaultNavigation()`:
- Pager vertical (`VerticalPagerViewer`): `LNavigation`.
- Pager L2R / R2L: `RightAndLeftNavigation`.
- Webtoon (both): `LNavigation` (`WebtoonConfig.defaultNavigation()`, `WebtoonConfig.kt:335-337`).

**Mode 1 L shaped** (`navigation/LNavigation.kt:18-35`), in list order:
1. `(0.00, 0.33, 0.33, 0.66)` PREV
2. `(0.00, 0.00, 1.00, 0.33)` PREV
3. `(0.66, 0.33, 1.00, 0.66)` NEXT
4. `(0.00, 0.66, 1.00, 1.00)` NEXT
Everything else: MENU. (Middle-centre block (0.33..0.66, 0.33..0.66) is MENU.)

**Mode 2 Kindle-ish** (`navigation/KindlishNavigation.kt:18-27`):
1. `(0.33, 0.33, 1.00, 1.00)` NEXT
2. `(0.00, 0.33, 0.33, 1.00)` PREV
Everything else: MENU.

**Mode 3 Edge** (`navigation/EdgeNavigation.kt:18-31`):
1. `(0.00, 0.00, 0.33, 1.00)` NEXT
2. `(0.33, 0.66, 0.66, 1.00)` PREV
3. `(0.66, 0.00, 1.00, 1.00)` NEXT
Everything else: MENU.

**Mode 4 Right and Left** (`navigation/RightAndLeftNavigation.kt:18-27`):
1. `(0.00, 0.00, 0.33, 1.00)` LEFT
2. `(0.66, 0.00, 1.00, 1.00)` RIGHT
Everything else: MENU.

**Mode 5 Disabled** (`navigation/DisabledNavigation.kt`): empty list. Every tap is MENU.

### 2.4 Action dispatch per viewer

Pager (`PagerViewer.kt:113-119`):
- MENU → `activity.toggleMenu()`
- NEXT → `moveToNext()`
- PREV → `moveToPrevious()`
- RIGHT → `moveRight()` (absolute, not reading-direction aware)
- LEFT → `moveLeft()` (absolute)

`moveToNext` / `moveToPrevious` are overridden by direction (§6.3).

Webtoon (`WebtoonViewer.kt:123-127`):
- MENU → `activity.toggleMenu()`
- NEXT, RIGHT → `scrollDown()`
- PREV, LEFT → `scrollUp()`

Webtoon has no reading direction, so NEXT and RIGHT are both "down".

### 2.5 Tap zone overlay colours and labels (used by the overlay in area:43-reader-core)

OBSERVED. `ViewerNavigation.NavigationRegion` colours use `Color.argb(0xCC, r, g, b)`:
- MENU `#CC95818D` label `action_menu` = "Menu"
- PREV `#CCFF7733` label `nav_zone_prev` = "Prev"
- NEXT `#CC84E296` label `nav_zone_next` = "Next"
- LEFT `#CC7D1128` label `nav_zone_left` = "Left"
- RIGHT `#CCA6CFD5` label `nav_zone_right` = "Right"

Proposal: keep these exact ARGB values as `const` in `reader::nav` so the overlay (area:43) can reuse them.

---

## 3. Gesture recognition

### 3.1 GestureDetectorWithLongTap algorithm

OBSERVED. `V/GestureDetectorWithLongTap.kt`. Platform constants are read at construction: `slop = ViewConfiguration.scaledTouchSlop`, `longTapTime = ViewConfiguration.getLongPressTimeout()`, `doubleTapTime = ViewConfiguration.getDoubleTapTimeout()`. AOSP defaults are 500 ms (long press), 300 ms (double tap) and 8 dp (touch slop); user and accessibility settings can change them.

On each `onTouchEvent(ev)`:
1. `ACTION_DOWN`:
   - recycle the previous `lastDownEvent`, store a copy.
   - if `ev.downTime - lastUp > doubleTapTime`: record `downX`, `downY`, post `longTapFn` after `longTapTime`.
   - Otherwise (this down is a double tap) no long-tap timer is scheduled.
2. `ACTION_MOVE`: if `|x - downX| > slop` or `|y - downY| > slop`, cancel `longTapFn`. Note that `downX` and `downY` are only set when the timer is scheduled, so the check uses stale coordinates after a double tap.
3. `ACTION_UP`: `lastUp = ev.eventTime`; cancel `longTapFn`.
4. `ACTION_CANCEL` and `ACTION_POINTER_DOWN`: cancel `longTapFn`.
5. Always returns `super.onTouchEvent(ev)` (the Android `GestureDetector` that produces `onSingleTapConfirmed`, `onDoubleTap`, `onFling`, and the other callbacks of the Android framework).

`longTapFn` calls `listener.onLongTapConfirmed(lastDownEvent!!)`.

### 3.2 Pager gestures

OBSERVED. `V/pager/Pager.kt`.
- `dispatchTouchEvent` first calls `super.dispatchTouchEvent(ev)` (the ViewPager, which handles swipes), then, if `isGestureDetectorEnabled`, feeds the same event to `gestureDetector`. The detector therefore sees every touch even while the ViewPager is paging.
- `onSingleTapConfirmed` → `tapListener` (§2.4). Returns true.
- `onLongTapConfirmed` → `longTapListener(ev)`. If it returns true, haptic `LONG_PRESS` is performed.
- Double tap: the Android default `onDoubleTap` does nothing in the pager. A double tap therefore produces no action. (The pager does not zoom on double tap; zoom is in the page image, §10.)
- `setGestureDetectorEnabled(false)` is called by `ReaderButton` while a button inside a page is pressed (`ReaderButton.kt:22-28`). This prevents the page turn when a retry button is pressed.

Pager long tap (`PagerViewer.kt:121-130`):
```
if (activity.viewModel.state.value.menuVisible || config.longTapEnabled) {
    item = adapter.items.getOrNull(pager.currentItem)
    if (item is ReaderPage) { activity.onPageLongTap(item); return true }
}
return false
```
Note the operator: the check passes when the menu is visible even if long tap is disabled.
`onPageLongTap` opens the page actions sheet (`ReaderActivity.kt:694-696`, `viewModel.openPageDialog(page)`, area:43).

### 3.3 Webtoon gestures

OBSERVED. `V/webtoon/WebtoonRecyclerView.kt` (`GestureListener`, `Detector`) and `V/webtoon/WebtoonFrame.kt`.
- `onSingleTapConfirmed` → `tapListener` (§2.4). Returns **false** (so the recycler still sees the tap for its own bookkeeping).
- `onDoubleTap` → sets `detector.isDoubleTapping = true`. Returns false.
- `onDoubleTapConfirmed` (called from `Detector` on `ACTION_UP` when `isDoubleTapping && !isQuickScaling`): if `!isZooming && doubleTapZoom`, run the double-tap zoom (§10.5).
- `onLongTapConfirmed` → `longTapListener(ev)`. If true, haptic.
- Pinch: `WebtoonFrame`'s `ScaleGestureDetector` (§10.4). Fling: `GestureDetector` with `FlingListener.onFling` → `recycler.zoomFling(velocityX.toInt(), velocityY.toInt())` (§10.6).
- `ScaleGestureDetector.isQuickScaleEnabled = doubleTapZoom` (`WebtoonFrame.kt:29-34`), so quick scale is on exactly when double-tap zoom is on.

Webtoon long tap (`WebtoonViewer.kt:129-142`): hit-test `recycler.findChildViewUnder(event.x, event.y)`, then the adapter position, then `items[position] is ReaderPage`. Same `menuVisible || longTapEnabled` gate as the pager.

### 3.4 Proposal: GTK gesture mapping

- Single tap: `gtk::GestureClick` with `n_press == 1` on release, after the double-click timeout (`gtk-double-click-time`, default 400 ms in GTK) when the viewer needs to distinguish double tap (webtoon). Pager: fire immediately on release because double tap has no pager action (Proposal, deviates by up to one double-click-time of latency for taps that are never doubled).
- Double tap: `GestureClick` with `n_press == 2` (webtoon and pager page image zoom).
- Long press: `gtk::GestureLongPress` with delay `gtk-long-press-time` (GTK default 500 ms). A long press cancels a pending single tap. The double-tap window rule of §3.1 step 1 is reproduced by ignoring a long press that starts within `gtk-double-click-time` of the previous release.
- Drag/slop: `gtk-dnd-drag-threshold` (default 8 px) stands in for `scaledTouchSlop`.
- Pinch: `gtk::GestureZoom` (scale factor per update; `scale-changed` signal).
- Quick scale: a `GestureClick` that stays pressed after a double tap, then `GestureDrag` vertical deltas, reproduced with the §10.4 formula.
- Fling: `gtk::GestureSwipe` velocities (px/s) into `zoom_fling`.
- Scroll wheel: `gtk::EventControllerScroll` with `VERTICAL` flags (§4.3).

---

## 4. Keyboard and pointer input

### 4.1 Dispatch order

OBSERVED. `ReaderActivity.dispatchKeyEvent` (`ReaderActivity.kt:322-325`) calls `viewer.handleKeyEvent(event)` first; if not handled it calls `super.dispatchKeyEvent`. `onKeyUp` in the activity handles `KEYCODE_N` (next chapter) and `KEYCODE_P` (previous chapter) (`ReaderActivity.kt:308-316`, area:43). `dispatchGenericMotionEvent` (`ReaderActivity.kt:332`) delegates to `viewer.handleGenericMotionEvent` first.

### 4.2 Pager key table

OBSERVED. `PagerViewer.handleKeyEvent` (`PagerViewer.kt:381-418`). `isUp = event.action == ACTION_UP`. `ctrlPressed = metaState & META_CTRL_ON > 0`. Return value is `true` for every key listed (even on key-down), `false` for unlisted keys, and `false` for volume keys when the volume feature is off or the menu is visible.

| keyCode | action | effect |
|---|---|---|
| KEYCODE_VOLUME_DOWN | if `!volumeKeysEnabled` or menu visible: return false. On UP: `volumeKeysInverted ? moveUp() : moveDown()` | |
| KEYCODE_VOLUME_UP | same gate. On UP: `volumeKeysInverted ? moveDown() : moveUp()` | |
| KEYCODE_DPAD_RIGHT | on UP: `ctrl ? moveToNext() : moveRight()` | |
| KEYCODE_DPAD_LEFT | on UP: `ctrl ? moveToPrevious() : moveLeft()` | |
| KEYCODE_DPAD_DOWN | on UP: `moveDown()` | |
| KEYCODE_DPAD_UP | on UP: `moveUp()` | |
| KEYCODE_PAGE_DOWN | on UP: `moveDown()` | |
| KEYCODE_PAGE_UP | on UP: `moveUp()` | |
| KEYCODE_MENU | on UP: `activity.toggleMenu()` | |
| other | return false | |

Where `moveDown()` = `moveToNext()`, `moveUp()` = `moveToPrevious()`, and for the R2L and vertical viewers these are overridden (§6.3).

Note: the `ctrl` variants do not skip chapters. They use the direction-normalised page move. Physical DPAD_LEFT/RIGHT are not direction-normalised.

### 4.3 Pager pointer scroll

OBSERVED. `PagerViewer.handleGenericMotionEvent` (`PagerViewer.kt:424-438`): only when `event.source and SOURCE_CLASS_POINTER != 0` and `action == ACTION_SCROLL`:
- if `getAxisValue(AXIS_VSCROLL) < 0.0f` → `moveDown()`, else → `moveUp()` (so a value of exactly 0 moves up)
- return true. Otherwise return false.

Android `AXIS_VSCROLL` is positive for scrolling towards the user's up direction. A negative value is wheel-down.

Proposal: GTK `EventControllerScroll` delivers `dy` where positive is scroll down. Map `dy > 0` → next page (down), `dy <= 0` → previous page (up). This keeps "wheel down = next page". The `0` case is treated as previous to keep the observed behaviour.

### 4.4 Webtoon keys

OBSERVED. `WebtoonViewer.handleKeyEvent` (`WebtoonViewer.kt:308-340`):

| keyCode | action | effect |
|---|---|---|
| KEYCODE_VOLUME_DOWN | gate as pager. On UP: `volumeKeysInverted ? scrollUp() : scrollDown()` | |
| KEYCODE_VOLUME_UP | gate as pager. On UP: `volumeKeysInverted ? scrollDown() : scrollUp()` | |
| KEYCODE_MENU | on UP: toggleMenu | |
| KEYCODE_DPAD_LEFT, KEYCODE_DPAD_UP, KEYCODE_PAGE_UP | on UP: `scrollUp()` | consumed on both actions (return true) |
| KEYCODE_DPAD_RIGHT, KEYCODE_DPAD_DOWN, KEYCODE_PAGE_DOWN | on UP: `scrollDown()` | consumed on both actions |
| other | return false | |

`handleGenericMotionEvent` always returns false (`WebtoonViewer.kt:346-348`). The RecyclerView's own wheel handling applies.

Proposal: GTK's native scrolled-window handles wheel and touchpad scrolling. The webtoon viewer keeps the key table. The volume table is kept as an optional feature (§T-44a-015).

---

## 5. Item model: chapter transitions and list building

### 5.1 Model types

OBSERVED.
- `ReaderPage(index, url, imageUrl, stream)`: `number = index + 1` (`source-api/.../Page.kt:18-19`). `stream: (() -> InputStream)?`. `chapter` is `lateinit`. Equality is identity (no `equals` override).
- `InsertPage(parent)`: `index = parent.index`, `chapter = parent.chapter`, `status = READY`, `stream = parent.stream`. Identity equality.
- `ReaderChapter(chapter)` is a data class over the domain chapter, so equality is structural over `chapter`. It has `stateFlow: StateFlow<State>` with `State.Wait`, `State.Loading`, `State.Error(error: Throwable)`, `State.Loaded(pages: List<ReaderPage>)`. `pages` is non-null only when `Loaded`. `requestedPage: Int = 0`.
- `ChapterTransition` (sealed): `Prev(from, to?)` and `Next(from, to?)`. `equals` is symmetric over the unordered pair: `(from == other.from && to == other.to) || (from == other.to && to == other.from)`. `hashCode = from.hashCode()*31 + (to?.hashCode() ?: 0)`. `toString` = `"Prev(from=<url>, to=<url>)"`.
- `ViewerChapters(currChapter, prevChapter?, nextChapter?)`.

Consequence for the port: a `Prev(A, B)` equals `Next(B, A)` under `==`. Lists use `indexOf` and DiffUtil identity, so this matters. Proposal: transitions are keyed by `(from_id, to_id)` ordered as in the variant, and by the unordered pair only where the Kotlin used `==`. Identify each item by a `ItemKey` enum (`Page(chapter_id, index, insert: bool)`, `Transition(kind, from_id, to_id)`), and keep an explicit test (T-44a-079) for the symmetric equality, since equality is observable in `indexOf` for `moveToPage`.

### 5.2 Gap rule

OBSERVED. `calculateChapterGap(higher: ReaderChapter?, lower: ReaderChapter?)` (`V/MissingChapters.kt:7-11`) calls the domain function (`domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt:40-49`):
1. if either is null → 0
2. if either chapter number is not "recognised" → 0 (domain uses `isRecognizedNumber`)
3. `calculateChapterGap(higherNum, lowerNum)`: if either number < 0 → 0; else `floor(higher).toInt() - floor(lower).toInt() - 1`.

The result can be negative (for example, equal integer parts). Callers test `> 0`.

### 5.3 Pager list builder (setChapters)

OBSERVED. `PagerViewerAdapter.setChapters(chapters, forceTransition)` (`V/pager/PagerViewerAdapter.kt:47-127`). Steps in order:

1. `prevHasMissingChapters = calculateChapterGap(curr, prev) > 0` (higher = curr).
2. `nextHasMissingChapters = calculateChapterGap(next, curr) > 0` (higher = next).
3. If `prev != null` and `prev.pages != null`: append `prev.pages.takeLast(2)`.
4. If `prevHasMissingChapters || forceTransition || prev?.state !is Loaded`: append `Prev(curr, prev)`. When `prev == null`, `prev?.state` is null, so this is always true: a Prev transition is always added when there is no previous chapter.
5. If `curr.pages != null`:
   - `pages = curr.pages.toMutableList()`; `lastPage = pages.last()` (throws on an empty list; see §12.4).
   - For each key in `preprocessed.keys` sorted descending: if `lastPage.index == key` set `insertPageLastPage = preprocessed[key]`; then `pages.add(key + 1, preprocessed[key])`.
   - Append `pages`.
6. `currentChapter = curr`.
7. `nextTransition = Next(curr, next)` (always assigned). Append it if `nextHasMissingChapters || forceTransition || next?.state !is Loaded`.
8. If `next != null` and `next.pages != null`: append `next.pages.take(2)`.
9. Remove all `InsertPage`s from the *old* `items` list (`items.removeAll(...)`). This does not affect `newItems`.
10. If `viewer is R2LPagerViewer`: `newItems.reverse()`.
11. `preprocessed = {}`; `items = newItems`; `notifyDataSetChanged()`.
12. If `insertPageLastPage != null`: `viewer.moveToPage(insertPageLastPage)`.

Observed result with default settings (`always_show_chapter_transition = true`), L2R, all chapters loaded, next and previous present:
`[prev_p[-2], prev_p[-1], Prev(curr, prev), curr_p[0..n-1], Next(curr, next), next_p[0], next_p[1]]`.

Observed result at the end of the book (no next): `[..., curr_p[n-1], Next(curr, null)]`. Because `Next(curr, null)` is always added when `next == null`, the terminal transition is always present.

### 5.4 forceTransition

OBSERVED. Pager (`PagerViewer.kt:277-282`): `forceTransition = config.alwaysShowChapterTransition || adapter.items.getOrNull(pager.currentItem) is ChapterTransition`.
Webtoon (`WebtoonViewer.kt:243`): `forceTransition = config.alwaysShowChapterTransition || currentPage is ChapterTransition`.

With the default `always_show_chapter_transition = true`, forceTransition is always true, so transitions are always present between chapters. Turning it off makes transitions appear only when a chapter is missing, not loaded, or when the reader is already on a transition.

### 5.5 Webtoon list builder

OBSERVED. `WebtoonAdapter.setChapters` (`V/webtoon/WebtoonAdapter.kt:38-83`):
1. Same gap computations (lines 42-43).
2. If `prev != null` and pages non-null: append `prev.pages.takeLast(2)`.
3. If `prevHasMissingChapters || forceTransition || prev?.state !is Loaded`: append `Prev(curr, prev)`.
4. If `curr.pages != null`: append all of `curr.pages`. **There is no preprocessed/InsertPage handling in webtoon.**
5. `currentChapter = curr`.
6. If `nextHasMissingChapters || forceTransition || next?.state !is Loaded`: append `Next(curr, next)`.
7. If `next != null` and pages non-null: append `next.pages.take(2)`.
8. `updateItems(newItems)`: `DiffUtil.calculateDiff(Callback(items, newItems))`, `items = newItems`, `dispatchUpdatesTo(this)`.

`Callback`: `areItemsTheSame(a, b) = oldItem == newItem` (uses the `equals` rules of §5.1). `areContentsTheSame = true` always, so changed page objects are not rebound in place by DiffUtil. `getItemViewType`: ReaderPage → 0 (PAGE_VIEW), ChapterTransition → 1 (TRANSITION_VIEW), other → error.

Webtoon has no reversal (no R2L webtoon).

### 5.6 Preprocessed insert pages (dual-page split, pager only)

OBSERVED. `PagerViewerAdapter.onPageSplit(currentPage, newPage)` (`PagerViewerAdapter.kt:162-193`):
1. If `currentPage !is ReaderPage` return.
2. `currentIndex = items.indexOf(currentPage)`.
3. If `currentPage.chapter.chapter.id != currentChapter?.chapter?.id` (the split page belongs to a neighbouring chapter): `preprocessed[newPage.index] = newPage`; return. (Stored so the insert survives the chapter swap.)
4. `placeAtIndex = (L2R or Vertical) ? currentIndex + 1 : currentIndex` (R2L uses currentIndex).
5. R2L guard: if `viewer is R2L` and `placeAtIndex - 1 >= 0` and `items[placeAtIndex-1] is InsertPage`: return (prevents an endless insert cycle).
6. Guard: if `items[placeAtIndex] is InsertPage`: return. (No index bounds check. See §12.4.)
7. `items.add(placeAtIndex, newPage)`; `notifyDataSetChanged()`.

`PagerViewer.onPageSplit` runs the adapter update on the UI thread (`PagerViewer.kt:440-445`, `activity.runOnUiThread`). The comment says inserting off the UI thread makes images go blank.

`cleanupPageSplit()` (`PagerViewerAdapter.kt:195-199`): remove every InsertPage and notify.

Insert page placement in `setChapters`: `preprocessed` entries are added to the current chapter list at `key + 1`. Afterwards `preprocessed` is cleared.

---

## 6. Pager engine

### 6.1 Construction

OBSERVED. `PagerViewer` init (`V/pager/PagerViewer.kt:83-146`):
- `pager.isVisible = false` ("don't layout the pager yet").
- `pager.layoutParams = MATCH_PARENT x MATCH_PARENT`; `isFocusable = false`; `offscreenPageLimit = 1`; `id = R.id.reader_pager`; `adapter = PagerViewerAdapter(this)`.
- ViewPager listener: `onPageSelected(position)`: if `!activity.isScrollingThroughPages` → `activity.hideMenu()`; then `onPageChange(position)`. `onPageScrollStateChanged(state)`: `isIdle = state == SCROLL_STATE_IDLE`.
- Tap, long-tap and dual-page listeners as in §2 and §3.
- `config.navigationModeChangedListener` → `activity.binding.navigationOverlay.setNavigation(config.navigator, showOnStart)` where `showOnStart = navigationOverlayOnStart || forceNavigationOverlay` (area:43 renders it).

`isIdle` setter: when set true and `awaitingIdleViewerChapters != null`, apply them via `setChaptersInternal`, clear them, and if `viewerChapters.currChapter.pages?.size == 1` call `activity.requestPreloadChapter(adapter.nextTransition.to)`.

### 6.2 Setting chapters

OBSERVED. `setChapters(chapters)` (`PagerViewer.kt:266-272`): if `isIdle` → `setChaptersInternal`; else store in `awaitingIdleViewerChapters` (replacing any earlier pending value).

`setChaptersInternal` (`PagerViewer.kt:277-291`):
1. `forceTransition = alwaysShowChapterTransition || items[currentItem] is ChapterTransition`.
2. `adapter.setChapters(chapters, forceTransition)`.
3. If `pager.isGone` (first layout): `pages = chapters.currChapter.pages ?: return`; `moveToPage(pages[min(chapters.currChapter.requestedPage, pages.lastIndex)])`; `pager.isVisible = true`.

Proposal: the first layout uses the same "clamp to last page" rule (`min(requestedPage, lastIndex)`). Negative `requestedPage` cannot occur (`requestedPage` is non-negative in the model).

### 6.3 Direction-normalised moves

OBSERVED. Base (`PagerViewer.kt:313-364`):
- `moveToNext()` → `moveRight()`; `moveToPrevious()` → `moveLeft()`; `moveUp()` → `moveToPrevious()`; `moveDown()` → `moveToNext()`.
- `moveRight()`: if `pager.currentItem != adapter.count - 1`: `holder = (currentPage as? ReaderPage)?.let(getPageHolder)`. If `holder != null && navigateToPan && holder.canPanRight()` → `holder.panRight()`. Else `pager.setCurrentItem(currentItem + 1, usePageTransitions)`.
- `moveLeft()`: symmetrical with `currentItem != 0`, `canPanLeft`, `panLeft`, `currentItem - 1`.

R2L (`V/pager/PagerViewers.kt:31-40`): `moveToNext()` = `moveLeft()`; `moveToPrevious()` = `moveRight()`. Combined with the reversed list (§5.3 step 10), "next" is always the item at a lower index in the R2L list, which is the page on the left.

Vertical: uses the base class (`moveToNext` = `moveRight` = index + 1, which is downward). Vertical pager reads top to bottom.

Edge: at the first or last item, the move does nothing (no wrap, no chapter change). Chapter changes happen only by `moveToPage`, the transition page, or activity-level chapter navigation.

Proposal: keep the same functions as pure index arithmetic: `fn next_index(dir, idx, len) -> Option<usize>`.

### 6.4 Page-change handling (`onPageChange`)

OBSERVED. `PagerViewer.kt:176-200`. Triggered by ViewPager `onPageSelected` and by `moveToPage` when the index does not change.
1. `page = items.getOrNull(position)`; if null or `page == currentPage`: return. (Equality for transitions is the symmetric rule of §5.1.)
2. `allowPreload = checkAllowPreload(page as? ReaderPage)`.
3. `forward`:
   - `currentPage is ReaderPage && page is ReaderPage`: if `page.number == currentPage.number` (split pair: the InsertPage and its parent share a number) → `forward = page is InsertPage` (the insert is always the second in reading order). Otherwise `forward = page.number > currentPage.number`.
   - `currentPage is ChapterTransition.Prev && page is ReaderPage` → `forward = false`.
   - Otherwise `forward = true`.
4. `currentPage = page`.
5. ReaderPage → `onReaderPageSelected(page, allowPreload, forward)`. ChapterTransition → `onTransitionSelected(page)`.

Note on `forward` with a `Next` transition followed by a page of the next chapter: `currentPage is ChapterTransition.Prev` is the only chapter-related case; a `Next` transition followed by a page falls through to `forward = true`, which is correct.

`checkAllowPreload(page)` (`PagerViewer.kt:202-219`):
- `page == null` → true (transition pages always allow preload).
- `currentPage == null` → true (initial opening).
- Otherwise true when `page.chapter == (currentPage as? ChapterTransition.Next)?.to` (going to the next chapter from a Next transition), or `page.chapter == (currentPage as? ReaderPage)?.chapter` (same chapter), or `page.chapter == adapter.nextTransition?.to` (entering the next chapter). Otherwise false.

`onReaderPageSelected(page, allowPreload, forward)` (`PagerViewer.kt:225-244`):
1. `pages = page.chapter.pages ?: return`. (A page whose chapter is not Loaded is ignored.)
2. `activity.onPageSelected(page)` (ReaderViewModel, area:43).
3. `getPageHolder(page)?.onPageSelected(forward)`: the holder's zoom-start/landscape-zoom hook (§10.3).
4. If `page is InsertPage` → return (no preload; "causes unwanted page jumping").
5. `inPreloadRange = pages.size - page.number < 5`. If `inPreloadRange && allowPreload && page.chapter == adapter.currentChapter` and `adapter.nextTransition?.to != null` → `activity.requestPreloadChapter(nextTransition.to)`.

Observed meaning: the last five pages (numbers `size-4` through `size`) of the current chapter trigger the next-chapter preload. Test: size 20 → numbers 16..20.

`onTransitionSelected(transition)` (`PagerViewer.kt:250-260`):
- if `transition.to != null` → `requestPreloadChapter(to)` (for both Prev and Next).
- else if `transition is Next` → `activity.showMenu()`. (A Prev transition with no previous chapter does nothing.)

`getPageHolder(page)` looks up the holder whose `item == page` among `pager.children` (only currently attached holders).

### 6.5 Selection side effects summary

OBSERVED. Each page selection: hides the menu (unless `isScrollingThroughPages`), reports the page to the view model, triggers the holder's zoom hook, and may preload. Each transition selection: preloads the destination, or shows the menu at the end of the book.

Quirk: `isScrollingThroughPages` is set to true in `ReaderActivity.kt:412` when the page slider is moved, and is never reset to false anywhere in the viewer or activity source files read here (`grep -rn isScrollingThroughPages app/src/main/java` shows only lines 139, 412 and `PagerViewer.kt:93`). Observed effect: after the slider is used once, selecting pages no longer hides the menu for the rest of the activity lifetime. Proposal: reset the flag after the next page selection, so the slider-driven move does not hide the menu but later swipes do. This is a deliberate fix (flagged in the task list).

### 6.6 `moveToPage`

OBSERVED. `PagerViewer.kt:296-308`: `position = adapter.items.indexOf(page)`. If found: `currentPosition = currentItem`; `setCurrentItem(position, true)` (**smooth = true**, regardless of `usePageTransitions`). If `currentPosition == position`, call `onPageChange(position)` manually because ViewPager does not fire the listener. If not found: log only.

Proposal: `smooth = usePageTransitions` for consistency with the move commands. This is a small change from the observed `true`; it is listed as an open question in §13.

### 6.7 `refreshAdapter` (image property change)

OBSERVED. `PagerViewer.kt:370-375`: `currentItem = pager.currentItem`; `adapter.refresh()` (recompute themed context); `pager.adapter = adapter`; `setCurrentItem(currentItem, false)`. This re-creates all visible holders, which rebinds and re-decodes pages.

### 6.8 Pager lifecycle

OBSERVED. `destroy()` (`PagerViewer.kt:148-151`): `scope.cancel()` (cancels the MainScope used for the config flows and the viewer's own coroutines). Page holders cancel their loads in `onDetachedFromWindow`. Chapter ref counting is in area:43 (`ViewerChapters.ref/unref`).

---

## 7. Webtoon engine

### 7.1 Construction

OBSERVED. `WebtoonViewer` (`V/webtoon/WebtoonViewer.kt:34-167`).
- `recycler = WebtoonRecyclerView(activity)`; `frame = WebtoonFrame(activity)`.
- `scrollDistance = activity.resources.displayMetrics.heightPixels * 3 / 4` (integer division; computed once at construction).
- `layoutManager = WebtoonLayoutManager(activity, scrollDistance)`: the extra layout space equals `scrollDistance` (the viewport height times 3/4, so about 0.75 screen of pre-layout above and below).
- `recycler.setItemViewCacheSize(4)` (`RECYCLER_VIEW_CACHE_SIZE`; comment: "Double the cache size to reduce rebinds/recycles incurred by the extra layout space").
- `recycler.isVisible = false` until first layout. `isFocusable = false`. `itemAnimator = null` (no item animations).
- `threshold = readerHideThreshold().get().threshold` read once here.
- Scroll listener (below), tap and long-tap listeners, config listeners (`imagePropertyChanged` → `refreshAdapter`, `themeChanged` → `ActivityCompat.recreate(activity)`, `doubleTapZoomChanged` → `frame.doubleTapZoom`, `zoomPropertyChanged` → `frame.zoomOutDisabled`, `navigationModeChanged` → overlay).
- `frame.addView(recycler)`.

### 7.2 Scroll listener

OBSERVED. `WebtoonViewer.kt:89-112`, on every `onScrolled(dx, dy)`:
1. `onScrolled()` (current-page update, §7.3).
2. If `(dy > threshold || dy < -threshold) && menuVisible` → `hideMenu()`. (Scrolls larger than the threshold in one event hide the menu.)
3. If `dy < 0` (scrolling up): `firstItem = items[layoutManager.findFirstVisibleItemPosition()]`. If `firstItem is Prev && firstItem.to != null` → `requestPreloadChapter(firstItem.to)`.
4. `lastIndex = findLastEndVisibleItemPosition()`; `lastItem = items[lastIndex]`. If `lastItem is Next && lastItem.to == null` → `showMenu()`. (The end of the book shows the menu.)

### 7.3 Current page detection

OBSERVED.
- `WebtoonLayoutManager.findLastEndVisibleItemPosition()` (`V/webtoon/WebtoonLayoutManager.kt:142-160`): iterate children from last to first; return the position of the first child (from the bottom) where `childEnd <= parentEnd || childStart < parentStart`. Returns `NO_POSITION` (-1) when no child qualifies (before the first layout).
- `WebtoonViewer.onScrolled(pos)` (`WebtoonViewer.kt:269-280`): `position = pos ?: findLastEndVisibleItemPosition()`; `item = items.getOrNull(position)`; `allowPreload = checkAllowPreload(item as? ReaderPage)`; if `item != null && currentPage != item`: `currentPage = item`; ReaderPage → `onPageSelected(item, allowPreload)`; ChapterTransition → `onTransitionSelected(item)`.

Note the "current page" is the last child whose end is on screen, not the first one. In a long strip this is the page whose bottom edge is visible nearest the bottom of the viewport.

`checkAllowPreload(page)` (`WebtoonViewer.kt:169-187`):
- `page == null` → true.
- `currentPage == null` → true.
- `nextItem = items.last()`; `nextChapter = (nextItem as? Next)?.to ?: (nextItem as? ReaderPage)?.chapter`.
- true if `page.chapter == (currentPage as? ReaderPage)?.chapter` or `page.chapter == nextChapter`. Otherwise false.

`onPageSelected(page, allowPreload)` (`WebtoonViewer.kt:208-224`):
1. `pages = page.chapter.pages ?: return`.
2. `activity.onPageSelected(page)`.
3. If `pages.size - page.number < 5 && allowPreload && page.chapter == adapter.currentChapter`:
   - `nextItem = items.last()`; `transitionChapter = (nextItem as? Next)?.to ?: (nextItem as? ReaderPage)?.chapter`.
   - If non-null → `requestPreloadChapter(transitionChapter)`.

Quirk (observed): when the next chapter's pages are already appended (last item is a ReaderPage of the next chapter), the preload request targets that already-loaded chapter. The call is harmless (the view model no-ops for loaded chapters, area:43) but it is not the intended "next chapter to preload".

`onTransitionSelected(transition)` (`WebtoonViewer.kt:230-237`): if `to != null` → `requestPreloadChapter(to)`. Unlike the pager, a Next transition with no next chapter does **not** show the menu here (the menu is shown by the scroll listener, §7.2 step 4).

### 7.4 Scrolling by taps and keys

OBSERVED. `scrollDown()` / `scrollUp()` (`WebtoonViewer.kt:296-302`, `285-291`): if `usePageTransitions` → `recycler.smoothScrollBy(0, ±scrollDistance)`; else `recycler.scrollBy(0, ±scrollDistance)`.

`scrollDistance` = `heightPixels * 3 / 4` (§7.1). One tap or key press moves 75% of the screen height.

### 7.5 `moveToPage`

OBSERVED. `WebtoonViewer.kt:257-267`: `position = items.indexOf(page)`. If found: `layoutManager.scrollToPositionWithOffset(position, 0)`. Then if `findLastEndVisibleItemPosition() == -1` (not laid out yet) → `onScrolled(pos = position)` to set the current page explicitly. If not found: log only.

`setChapters` (`WebtoonViewer.kt:242-252`): `forceTransition = alwaysShowChapterTransition || currentPage is ChapterTransition`; `adapter.setChapters`; if `recycler.isGone` (first layout): `pages = curr.pages ?: return`; `moveToPage(pages[min(requestedPage, lastIndex)])`; `recycler.isVisible = true`.

Note: `setChapters` on the webtoon is not deferred the way the pager's is. Changes apply immediately (DiffUtil keeps the scroll position stable).

### 7.6 Scroll-hide threshold

OBSERVED. `readerHideThreshold` values (§1.3): HIGHEST 5, HIGH 13, LOW 31, LOWEST 47. Default LOW (31). Read once, so a change in settings takes effect on the next viewer creation.

Proposal: subscribe to the watch channel and read the latest value on each scroll. This is a deliberate change; it is listed in the tasks.

### 7.7 `refreshAdapter`

OBSERVED. `WebtoonViewer.kt:354-361`:
```
position = layoutManager.findLastEndVisibleItemPosition()
adapter.refresh()
adapter.notifyItemRangeChanged(max(0, position - 3), min(position + 3, adapter.itemCount - 1))
```
Quirk: `notifyItemRangeChanged(positionStart, itemCount)` takes a count as the second argument, not an end index. The call therefore marks `min(position + 3, count - 1)` items starting at `position - 3`. The range overshoots and may cover items beyond the intended window. The result is extra rebinds, not a crash, because RecyclerView clamps the range.

Proposal: rebind exactly the items whose index is in `[position-3, position+3]`.

### 7.8 Webtoon lifecycle

OBSERVED. `destroy()` cancels `scope`. Holders cancel loads in `recycle()` (`onViewRecycled`, `WebtoonAdapter.kt:144-149`).

---

## 8. Page holders: loading state machine

### 8.1 Status mapping (both viewers)

OBSERVED. `PagerPageHolder.loadPageAndProcessStatus` (`V/pager/PagerPageHolder.kt:90-112`) and `WebtoonPageHolder.loadPageAndProcessStatus` (`V/webtoon/WebtoonPageHolder.kt:130-152`). The holder starts `loader.loadPage(page)` on an IO dispatcher (fire-and-forget, `launchIO`) and collects `page.statusFlow` with `collectLatest`:

| `Page.State` | Pager action | Webtoon action |
|---|---|---|
| QUEUE | `setQueued()`: create indicator if absent, show it, remove error layout | `progressContainer.isVisible = true`, show indicator, remove error layout |
| LOAD_PAGE | `setLoading()`: same as QUEUE | same as QUEUE |
| DOWNLOAD_IMAGE | `setDownloading()` then collect `page.progressFlow` (0..100) → `indicator.setProgress(value)` | same, progress into the indicator |
| READY | `setImage()` (§8.2) | `setImage()` (§8.3) |
| ERROR | `setError()`: hide indicator, show error layout | `progressContainer.isVisible = false`, `initErrorLayout()` |

`Page.State` declaration order: QUEUE, LOAD_PAGE, DOWNLOAD_IMAGE, READY, ERROR (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt:51-57`). The initial value is QUEUE (`Page.kt:22`).

If `page.chapter.pageLoader == null` the holder returns immediately and shows nothing (blank). Observed.

### 8.2 Pager `setImage` (exact order)

OBSERVED. `PagerPageHolder.setImage()` (`V/pager/PagerPageHolder.kt:144-183`):
1. `progressIndicator?.setProgress(0)`.
2. `streamFn = page.stream ?: return`. (No stream → nothing happens; no error is shown.)
3. In IO context:
   - `source = streamFn().use { process(item, Buffer().readFrom(it)) }`
   - `isAnimated = ImageUtil.isAnimatedAndSupported(source)`
   - `background = if (!isAnimated && viewer.config.automaticBackground) ImageUtil.chooseBackground(context, source.peek().inputStream()) else null`
4. In UI context:
   - `setImage(source, isAnimated, Config(zoomDuration = doubleTapAnimDuration, minimumScaleType = imageScaleType, cropBorders = imageCropBorders, zoomStartPosition = imageZoomType, landscapeZoom = landscapeZoom))`
   - if `!isAnimated`: `pageBackground = background`
   - `removeErrorLayout()`
5. On any Throwable: log, then UI context `setError()`.

Background application: `ReaderPageImageView.onImageLoaded()` sets `background = pageBackground` (`ReaderPageImageView.kt:82-85`). The background is applied only once the image is ready.

### 8.3 Webtoon `setImage` (exact order)

OBSERVED. `WebtoonPageHolder.setImage()` (`V/webtoon/WebtoonPageHolder.kt:184-213`):
1. `progressIndicator.setProgress(0)`; `streamFn = page?.stream ?: return`.
2. IO: `source = streamFn().use { process(Buffer().readFrom(it)) }`; `isAnimated = isAnimatedAndSupported(source)`.
3. UI: `frame.setImage(source, isAnimated, Config(zoomDuration = doubleTapAnimDuration, minimumScaleType = SCALE_TYPE_FIT_WIDTH, cropBorders = imageCropBorders))`; `removeErrorLayout()`.
4. Throwable → `setError()`.

The webtoon config does not pass `zoomStartPosition` (defaults to CENTER) or `landscapeZoom` (defaults to false). Automatic background is not computed for webtoon (`automaticBackground` is only in `PagerConfig`).

### 8.4 Decode-path selection in `ReaderPageImageView.setImage(source, isAnimated, config)`

OBSERVED. `ReaderPageImageView.kt:174-183`: if animated → `prepareAnimatedImageView()` and `setAnimatedImage`; else `prepareNonAnimatedImageView()` and `setNonAnimatedImage`.

Non-animated, pager (`isWebtoon = false`): `SubsamplingScaleImageView` (SSIV). `setNonAnimatedImage` (`ReaderPageImageView.kt:289-350`) for a `BufferedSource`:
- `if (!isWebtoon || alwaysDecodeLongStripWithSSIV)` → `setHardwareConfig(ImageUtil.canUseHardwareBitmap(data))`, `setImage(ImageSource.inputStream(data.inputStream()))`, `isVisible = true`.
- Non-webtoon always takes this branch.

Non-animated, webtoon (`isWebtoon = true`, default `alwaysDecodeLongStripWithSSIV = false`): an `ImageRequest` through Coil:
- `data(source)`, `memoryCachePolicy(DISABLED)`, `diskCachePolicy(DISABLED)`, `size(ViewSizeResolver(this))`, `precision(Precision.INEXACT)`, `cropBorders(config.cropBorders)`, `customDecoder(true)`, `crossfade(false)`.
- success: `setImage(ImageSource.bitmap(image.bitmap))`, `isVisible = true`.
- error: `onImageLoadError()`.
So webtoon strips are decoded at the view's measured size, not full resolution.

Animated (both viewers): an `ImageRequest` with cache disabled and `crossfade(false)`. Success: `setImageDrawable`, `start()` if `Animatable`, `isVisible = true`, `onImageLoaded()`. Error: `onImageLoadError()`. Pager uses `PhotoView` (zoomable); webtoon uses `AppCompatImageView` (not zoomable).

### 8.5 Error layout and retry

OBSERVED.
- Pager (`PagerPageHolder.showErrorLayout`, lines 271-294): inflates `R.layout.reader_error` once. Text `decode_image_error` = "The image couldn't be loaded". `action_retry` (`ReaderButton`, viewer set) → `page.chapter.pageLoader?.retryPage(page)`. `action_open_in_web_view` visible when `page.imageUrl != null`; its click listener is set only when `imageUrl` starts with "http" (case-insensitive). The click starts `WebViewActivity.newIntent(context, imageUrl)`. If the URL does not start with http, the button is visible but has no listener (observed no-op).
- Webtoon (`initErrorLayout`, lines 276-300): same strings. The error root is added to the frame with `MATCH_PARENT x (parentHeight * 0.8).toInt()`. Retry → `page?.chapter?.pageLoader?.retryPage(page)`. Open in WebView: visible if `imageUrl != null`, listener only for http (same quirk).
- Removal: pager `removeErrorLayout` hides the root and nulls the field (the view stays attached). Webtoon removes the root from the frame.

### 8.6 Webtoon progress container

OBSERVED. `WebtoonPageHolder.createProgressIndicator` (lines 260-271): `progressContainer = FrameLayout`, added to `frame` with height `parentHeight` (viewer.recycler.height at construction). `ReaderProgressIndicator` is added with top margin `parentHeight / 4`. Result: each webtoon page occupies at least one viewport height while loading, so the layout does not create extra holders to fill the screen (comment at lines 51-54).

`refreshLayoutParams` (lines 98-108): `FrameLayout.LayoutParams(MATCH_PARENT, WRAP_CONTENT)`, `bottomMargin = 15.dp` when `!isContinuous`, and `marginStart = marginEnd = (widthPixels * (sidePadding / 100f)).toInt()` where `widthPixels` is `Resources.getSystem().displayMetrics.widthPixels` (screen width, not the recycler width).

### 8.7 Pager page placeholders

The pager holder has no fixed placeholder height. The pager page fills the pager (`MATCH_PARENT` in `ReaderPageImageView`'s parent). Progress indicator is added on demand.

---

## 9. Image transforms

### 9.1 Dual-page split (pager, `PagerPageHolder.process`, lines 185-206)

Order:
1. `dualPageRotateToFit` → `rotateDualPage(source)` and return (rotate has priority over split).
2. `!dualPageSplit` → return source.
3. `page is InsertPage` → `splitInHalf(source)` (the second half).
4. `isDoublePage = ImageUtil.isWideImage(source)`; if false → return source.
5. `onPageSplit(page)`: creates `InsertPage(page)`, posts it to the viewer (`viewer.onPageSplit`).
6. return `splitInHalf(source)` (the first half).

`isWideImage` (`ImageUtil.kt:121-124`): `outWidth > outHeight` (strict). Decoded bounds only.

`splitInHalf(source, side)` side selection (`PagerPageHolder.kt:218-235`):

| viewer | page | side (before invert) |
|---|---|---|
| L2R | InsertPage | RIGHT |
| not L2R (R2L, vertical) | InsertPage | LEFT |
| L2R | normal | LEFT |
| not L2R | normal | RIGHT |

If `dualPageInvert`: swap LEFT and RIGHT.

`ImageUtil.splitInHalf(source, side)` (`ImageUtil.kt:129-148`): decode bitmap; `half = createBitmap(width / 2, height)`; `singlePage = Rect(0, 0, width/2, height)`; RIGHT source rect = `Rect(width - width/2, 0, width, height)`; LEFT source rect = `Rect(0, 0, width/2, height)`; draw; JPEG quality 100 into a Buffer. Integer division: for odd widths, the right half is `width - width/2` wide and may be one pixel wider than the left half.

Rotation (`ImageUtil.rotateImage`, lines 150-158 and `rotateDualPage`): `isWideImage` → rotate by `dualPageRotateToFitInvert ? -90 : 90` degrees, JPEG q100; else unchanged.

Animated images and split: `process` runs before the animated check, so an animated wide GIF would be decoded to a JPEG (first frame) and split. Observed behaviour: split applies to animated wide files. (Animated detection is run on the processed source.)

### 9.2 Dual-page split (webtoon, `WebtoonPageHolder.process`, lines 215-229)

1. `dualPageRotateToFit` → rotate (same as pager).
2. `dualPageSplit` and `isWideImage` → `splitAndMerge(source, upperSide)` where `upperSide = if (dualPageInvert) LEFT else RIGHT`.
3. Otherwise source.

`ImageUtil.splitAndMerge` (`ImageUtil.kt:168-194`): output `createBitmap(width / 2, height * 2)`. The `upperSide` half is drawn into the top `width/2 x height` cell, the other half into the bottom cell. Concretely: rightPart = RIGHT ? `Rect(width-width/2, 0, width, height)` : `Rect(0,0,width/2,height)`, drawn to `Rect(0,0,width/2,height)`. leftPart = the complementary rect, drawn to `Rect(0,height,width/2,height*2)`. JPEG q100.

Webtoon splitting has no InsertPage, no preprocessed map and no `dualPageSplitChangedListener`. Turning the setting off triggers `refreshAdapter`, which re-decodes.

### 9.3 Automatic background (`ImageUtil.chooseBackground`)

OBSERVED. Used only by the pager when `theme == 3` and the page is not animated (`PagerPageHolder.kt:153-157`). The full algorithm is in `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:338-580`. It is summarised here as steps, and p1-topic-zoom-and-image.md section 5 restates them as a port-ready algorithm with every constant.

1. Decode with `ImageDecoder`. If decode fails or image is null → white.
2. If `width < 50 || height < 50` → white.
3. Sample positions: `top = 5`, `bot = h - 5`, `left = (w * 0.0275).toInt()`, `right = w - left`, `midX = w/2`, `midY = h/2`, `offsetX = (w * 0.01).toInt()`, `leftOffsetX = left - offsetX`, `rightOffsetX = right + offsetX`.
4. Eight edge pixels: topLeft(left,top), topRight(right,top), midLeft(left,midY), midRight(right,midY), topCenter(midX,top), botLeft(left,bot), bottomCenter(midX,bot), botRight(right,bot). Each has an `isDark` flag.
5. `darkBG` = (topLeftDark && (botLeftDark || botRightDark || topRightDark || midLeftDark || topMidDark)) || (topRightDark && (botRightDark || botLeftDark || midRightDark || topMidDark)).
6. If the six top/bottom pixels (topLeft, topCenter, topRight, botRight, bottomCenter, botLeft) are all non-white and each is "close to" its successor in that cyclic order → return solid topLeft colour.
7. If the number of white corners (topLeft, topRight, botLeft, botRight) > 2 → `darkBG = false`.
8. `blackColor` = first dark of topLeft, topRight, botLeft, botRight, else white.
9. Column scan over x in [left, right, leftOffsetX, rightOffsetX] (break on certain conditions) with rows at `step = h / 25` (integer). Track white/black streaks and counts; `notOffset` for left/right columns only. Rules in order: `blackPixels > 22` → set darkBG true, break; `blackStreak` → darkBG true, possibly break if `blackPixels > 18`; `whiteStreak || whitePixels > 22` → darkBG false.
10. `topIsBlackStreak = topBlackStreak > topWhiteStreak`, `bottomIsBlackStreak = botBlackStreak > botWhiteStreak`. If `overallWhitePixels > 9 && overallWhitePixels > overallBlackPixels` → darkBG false. If both black streaks → darkBG true.
11. Landscape device orientation → solid colour (black if darkBG else white).
12. Gradient decision for portrait. Returns a vertical gradient (TOP_BOTTOM) of `[black, black, white, white]` or `[white, white, black, black]`, or a solid black, or a solid white.

`isDark(c)`: `r < 40 && g < 40 && b < 40 && a > 200`. `isWhite(c)`: `r + g + b > 740`. `isCloseTo(o)`: each channel difference `< 30`.

Proposal: port as `fn choose_background(img: &RgbaImage, landscape: bool) -> Background` (`Solid(Rgba)` or `Gradient2(Rgba, Rgba)`), with golden tests (T-44a-081).

### 9.4 Crop borders (decoder-level, both viewers)

OBSERVED. `cropBorders` is passed to the native decoder (`image-decoder` dependency, tag `41c059e540`, `library/src/main/cpp/borders.cpp`). Constants (`borders.h`):
- `filledRatioLimit = 0.0025`
- `THRESHOLD = 0.75`
- `thresholdForBlack = (uint8_t)(255 * 0.75) = 191`
- `thresholdForWhite = (uint8_t)(255 - 255 * 0.75) = 63`
- `isBlackPixel(p)`: `p < 191`. `isWhitePixel(p)`: `p > 63`. (Names are as in the source; the two ranges overlap between 64 and 190. Keep as is.)

Algorithm (`findBorders`, `borders.cpp:208-215`): `top = findBorderTop`, `bottom = findBorderBottom`, `left = findBorderLeft(top, bottom)`, `right = findBorderRight(top, bottom)`; crop rect = `(left, top, right-left, bottom-top)`.
- `findBorderTop`: `filledLimit = round(width * 0.0025 / 2)`. Sample the first row at x = 0, 2, 4, ... < width: count black and white. If `white > filledLimit && black > filledLimit` → return 0 (mixed fill, no crop). Else if `black > filledLimit` → detect white, else detect black. Scan rows y = 1..height-1, counting detected pixels at x stride 2; return the first y with count > filledLimit. Return 0 if none.
- `findBorderBottom`: symmetric with the last row (y = height-1), scanning y = height-2 down to 1, returning `y + 1`. Return height if none, or on mixed fill.
- `findBorderLeft(top, bottom)`: `filledLimit = round(height * 0.0025 / 2)` (uses full height). Samples column x = 0 for y from top to bottom stride 2. Scan columns x = 1..width-1 counting over y in [top, bottom) stride 2; return first x with count > limit. Return 0 if none or mixed.
- `findBorderRight(top, bottom)`: column x = width-1 for sampling; scan x = width-2 down to 1; return `x + 1`. Return width if none or mixed.

The decoder must apply this to the luminance (grayscale) plane. The comment in `borders.h` says "This only works on bitmaps of a single component (grayscale)".

Proposal: port `find_borders(gray: &[u8], w, h) -> Rect` with the exact integer rounding (`(x as f32 + 0.5) as u32` style is not used; use `f64::round` then cast to u32, the C `round` semantics).

### 9.5 Animated detection

OBSERVED. `ImageUtil.isAnimatedAndSupported(source)` (`ImageUtil.kt:72-87`): reads the first 32 bytes with `ImageDecoder.findType`. Returns true only for:
- GIF (any GIF is treated as animated, even if single-frame), or
- WebP with `type.isAnimated` and Android 9+ (`SDK_INT >= P`), or
- HEIF with `type.isAnimated` and Android 11+ (`SDK_INT >= R`).
Other formats (AVIF, JXL, PNG, JPEG) are not animated-supported.

Proposal: GIF and animated WebP are animated. HEIF/AVIF/JXL animations are not supported (first frame) and this is listed as P2 task T-44a-052.

---

## 10. Zoom and pan

### 10.1 Pager, static images (SubsamplingScaleImageView configuration)

OBSERVED. `ReaderPageImageView.prepareNonAnimatedImageView` (`ReaderPageImageView.kt:247-274`):
- `setMaxTileSize(ImageUtil.hardwareBitmapThreshold)`: default 2048 (`SAFE_TEXTURE_LIMIT`, `GLUtil.kt:50`; preference `pref_hardware_bitmap_threshold`, default `GLUtil.SAFE_TEXTURE_LIMIT`).
- `setDoubleTapZoomStyle(ZOOM_FOCUS_CENTER)` (value 2).
- `setPanLimit(PAN_LIMIT_INSIDE)` (value 1).
- `setMinimumTileDpi(180)`.
- `setOnStateChangedListener`: scale changes → `onScaleChanged(newScale)`; centre changes ignored.
- `setOnClickListener` → `onViewClicked()`.

`setNonAnimatedImage` (`ReaderPageImageView.kt:289-309`):
- `setDoubleTapZoomDuration(config.zoomDuration.getSystemScaledDuration())`.
- `setMinimumScaleType(config.minimumScaleType)`.
- `setMinimumDpi(1)`: SSIV sets `maxScale = averageDpi / 1` (a temporary very large value; setupZoom replaces it).
- `setCropBorders(config.cropBorders)`.
- `setOnImageEventListener(onReady → setupZoom(config); if isVisibleOnScreen() landscapeZoom(true); onImageLoaded(). onImageLoadError → onImageLoadError())`.

`getSystemScaledDuration()` = `(this * context.animatorDurationScale).toInt().coerceAtLeast(1)`, where `animatorDurationScale` = `Settings.Global.ANIMATOR_DURATION_SCALE` (default 1f, `AnimationExtensions.kt:10-11`). A value of 0 (animations off) yields 1 ms.

`setupZoom(config)` (`ReaderPageImageView.kt:276-287`), called on ready:
- `maxScale = scale * 5` (`MAX_ZOOM_SCALE = 5F`), where `scale` is the scale SSIV chose on ready (the minimum scale for the current type).
- `setDoubleTapZoomScale(scale * 2)`.
- Zoom start: `LEFT` → `setScaleAndCenter(scale, PointF(0, 0))`; `RIGHT` → `setScaleAndCenter(scale, PointF(sWidth, 0))`; `CENTER` → `setScaleAndCenter(scale, center)`; `null` → nothing.

### 10.2 Scale types (minimum scale), SSIV at tag `66e0db195d`

Source: `library/src/main/java/com/davemorrissey/labs/subscaleview/SubsamplingScaleImageView.java`. Constants: `SCALE_TYPE_CENTER_INSIDE = 1`, `SCALE_TYPE_CENTER_CROP = 2`, `SCALE_TYPE_FIT_WIDTH = 3`, `SCALE_TYPE_FIT_HEIGHT = 4`, `SCALE_TYPE_ORIGINAL_SIZE = 5`, `SCALE_TYPE_SMART_FIT = 6`, `SCALE_TYPE_CUSTOM = 7`.

`minScale()` (`SubsamplingScaleImageView.java:1907-1941`), with `vPadding/hPadding` = view padding plus extra space when PAN_LIMIT_INSIDE:
- CENTER_INSIDE (and default): `min((viewW - hPad) / sW, (viewH - vPad) / sH)`
- CENTER_CROP: `max(...)` of the same two ratios
- FIT_WIDTH: `(viewW - hPad) / sW`
- FIT_HEIGHT: `(viewH - vPad) / sH`
- ORIGINAL_SIZE: `1`
- SMART_FIT: if `sH > sW` → `(viewW - hPad)/sW` (fit width) else `(viewH - vPad)/sH` (fit height)
- CUSTOM: the `minScale` field

`limitedScale(t) = min(maxScale, max(minScale(), t))`.

Mapping from the preference (§1.3): pref 1 → CENTER_INSIDE "Fit screen"; **pref 2 → CENTER_CROP labelled "Stretch"** (observed: "Stretch" fills and crops rather than distorting); pref 3 → FIT_WIDTH; pref 4 → FIT_HEIGHT; pref 5 → ORIGINAL_SIZE; pref 6 → SMART_FIT.

Proposal: keep the labels from the Kotlin (`ReaderPreferences.ImageScaleType`). Name the 2 case honestly in the UI help text, but keep the behaviour (CENTER_CROP) because it is what users have today. Open question in §13.

### 10.3 Zoom start and landscape zoom

OBSERVED. `ReaderPageImageView.onPageSelected(forward)` (`ReaderPageImageView.kt:102-123`), called by the pager on page selection:
- If the SSIV is ready → `landscapeZoom(forward)`.
- Else install a one-shot listener: on ready → `setupZoom(config)`, `landscapeZoom(forward)`, `onImageLoaded()`; on error → `onImageLoadError()`.

`landscapeZoom(forward)` (`ReaderPageImageView.kt:125-161`) runs only when all hold: `config.landscapeZoom`, `config.minimumScaleType == CENTER_INSIDE`, `sWidth > sHeight` (landscape page), and `scale == minScale`. Then, after a 500 ms `postDelayed`:
- point: zoomStart LEFT → forward ? `(0,0)` : `(sWidth, 0)`; RIGHT → forward ? `(sWidth, 0)` : `(0, 0)`; CENTER → `center`.
- `targetScale = viewHeight / sHeight` (fit the page height).
- `animateScaleAndCenter(targetScale, point)`, duration 500 ms, `EASE_IN_OUT_QUAD`, interruptible.

The `forward` flag comes from §6.4 step 3, so a page reached by moving backward starts at the right edge (for LEFT zoom start).

### 10.4 Pinch, quick scale and pan (SSIV touch model)

OBSERVED. SSIV `onTouchEvent` (`SubsamplingScaleImageView.java`, the `ACTION_MOVE` branch):

Two pointers (pinch):
- `vDistEnd = distance(p0, p1)`; `vCenterEnd = midpoint`.
- Zoom when `zoomEnabled && (centerMoved > 5 || |vDistEnd - vDistStart| > 5 || isPanning)`.
- `scale = min(maxScale, (vDistEnd / vDistStart) * scaleStart)`.
- If `scale <= minScale()`: reset start values (no pan), `scaleStart = minScale()`.
- Else if `panEnabled`: translate so the source point under the pinch centre stays under the moving centre.
- `fitToBounds(true)`.

One pointer in quick-scale mode (`isQuickScaling`, after a double tap is held and dragged):
- `dist = |quickScaleVStart.y - event.y| * 2 + quickScaleThreshold`, where `quickScaleThreshold = 20 dp`.
- `spanDiff = |1 - dist / quickScaleLastDistance| * 0.5`.
- If `spanDiff > 0.03 || quickScaleMoved`: `multiplier = isUpwards ? 1 + spanDiff : 1 - spanDiff`; `scale = clamp(scale * multiplier, minScale(), maxScale)`; translate around `quickScaleSCenter`.
- `isUpwards = event.y > quickScaleVLastPoint.y` (note: a larger y means the finger moved down, and the code treats that as "upwards"; keep the exact comparison).

Long click at 600 ms (`MESSAGE_LONG_CLICK`) is not used by the reader (no long-click listener on the image).

Pan with `PAN_LIMIT_INSIDE`: `fitToBounds` keeps the image inside the view. `getPanRemaining(RectF)` (`SubsamplingScaleImageView.java:2025-2047`), for INSIDE: left = max(0, -vTranslate.x), right = max(0, scaleWidth + vTranslate.x - viewW), top and bottom likewise. `canPanRight() = remaining.right > 1` (`ReaderPageImageView.kt:207-212`, `fn(it) > 1`).

### 10.5 Double tap (pager static image)

OBSERVED. SSIV `onDoubleTapEvent` (around `SubsamplingScaleImageView.java:855-876`):
- `doubleTapZoomScale = min(maxScale, this.doubleTapZoomScale)` (`scale * 2`).
- `zoomIn = scale <= doubleTapZoomScale * 0.9 || scale == minScale`.
- `targetScale = zoomIn ? doubleTapZoomScale : minScale()`.
- With `ZOOM_FOCUS_CENTER` (set by the reader): animate to `targetScale` centred on the touch point (`sCenter`), duration `doubleTapZoomDuration` (pref, scaled), not interruptible, origin DOUBLE_TAP_ZOOM. (When `!zoomIn` or `!panEnabled`, the same animation is used.)

### 10.6 Pager animated images (PhotoView 2.3.0)

OBSERVED. `prepareAnimatedImageView` (`ReaderPageImageView.kt:352-389`), pager: `PhotoView`, `adjustViewBounds = true`, `setScaleLevels(1, 2, 5)`, `setZoomTransitionDuration(zoomDuration scaled)`. Double tap: if `scale > 1` → `setScale(1, x, y, animate = true)`, else `setScale(2, x, y, true)`. Single tap confirmed → `onViewClicked()`. Scale changes → `onScaleChanged(scale)`.

### 10.7 Webtoon zoom model (`WebtoonRecyclerView`)

OBSERVED. The whole RecyclerView is scaled (`scaleX = scaleY = currentScale`) and translated (`x`, `y`).

State: `currentScale` (start 1f), `halfWidth = measuredWidth / 2`, `halfHeight = measuredHeight / 2` (updated on `onScale`), `originalHeight` (first measured height, never changes), `zoomOutDisabled`, `doubleTapZoom`, `isZooming`, `atFirstPosition`, `atLastPosition`.

Constants: `MIN_RATE = 0.5f`, `DEFAULT_RATE = 1f`, `MAX_SCALE_RATE = 3f`, `ANIMATOR_DURATION_TIME = 200` ms.
`minRate = if (zoomOutDisabled) 1f else 0.5f`.

Position clamps:
- `getPositionX(px)`: if `currentScale < 1` → 0. Else clamp to `[-halfWidth*(scale-1), +halfWidth*(scale-1)]`.
- `getPositionY(py)`: if `currentScale < 1` → `originalHeight/2 - halfHeight` (vertically centred). Else clamp to `[-halfHeight*(scale-1), +halfHeight*(scale-1)]`.

`onScale(factor)` (pinch and quick scale, from `WebtoonFrame.ScaleListener.onScale`):
1. `currentScale = clamp(currentScale * factor, minRate, 3)`.
2. `setScaleRate(currentScale)` (scaleX = scaleY).
3. `layoutParams.height = if (currentScale < 1) originalHeight / currentScale (int) else originalHeight`.
4. `halfHeight = layoutParams.height / 2`.
5. If `currentScale != 1`: `x = getPositionX(x)`, `y = getPositionY(y)`. Else `x = 0`, `y = 0`.
6. `requestLayout()`.

So zooming out below 1 makes the strip taller than the viewport (height = originalHeight / scale), which keeps it filling the frame when scaled down.

`onScaleBegin()`: if `detector.isDoubleTapping` → `detector.isQuickScaling = true`.

`onScaleEnd()`: if `scaleX < minRate` → `zoom(currentScale, minRate, x, 0, y, 0)` (animate back to the minimum). Effectively a no-op unless the minimum changed.

`zoom(from, to, fromX, toX, fromY, toY)` (`WebtoonRecyclerView.kt:103-132`): sets `isZooming = true`; three `ValueAnimator`s (x, y, scale) over 200 ms with `DecelerateInterpolator` (factor 1, `f(t) = 1 - (1-t)^2`); `doOnEnd` → `isZooming = false`, `currentScale = to`.

Double-tap zoom (`GestureListener.onDoubleTapConfirmed`, lines 235-249): only if `!isZooming && doubleTapZoom`:
- if `scaleX != 1` → `zoom(currentScale, 1, x, 0, y, 0)`; `layoutParams.height = originalHeight`; `halfHeight = height/2`; `requestLayout()`.
- else → `toScale = 2`; `toX = (halfWidth - ev.x) * (2 - 1)`; `toY = (halfHeight - ev.y) * (2 - 1)`; `zoom(1, 2, 0, toX, 0, toY)`.
So double tap zooms to 2x centred on the touch point (`ev.x`, `ev.y` in recycler coordinates), and a second double tap resets.

Quick scale: `Detector.onTouchEvent` on `ACTION_MOVE` with `isDoubleTapping && isQuickScaling` → return true (consume; the scale comes from ScaleGestureDetector). On `ACTION_UP` with `isDoubleTapping && !isQuickScaling` → double-tap confirm. Flags reset on UP and CANCEL.

Drag while zoomed (`Detector`, `ACTION_MOVE`, lines 283-326):
- `dx = x - downX`; `dy = (atFirstPosition || atLastPosition) ? y - downY : 0`.
- If `!isZoomDragging && currentScale > 1`: if `|dx| > touchSlop` → `dx -= sign(dx) * touchSlop`, `startScroll = true`; same for dy. If `startScroll` → `isZoomDragging = true`.
- If `isZoomDragging` → `zoomScrollBy(dx, dy)` (`x = getPositionX(x + dx)`, `y = getPositionY(y + dy)`).
- `super.onTouchEvent(ev)` always runs, so the RecyclerView also scrolls. Observed: when zoomed and not at the first or last item, `dy` is 0, so vertical drags scroll the list instead of panning the zoom.

`touchSlop = ViewConfiguration.scaledTouchSlop`. `downX/downY` are recorded on ACTION_DOWN and POINTER_DOWN as rounded `(x + 0.5f).toInt()`.

`atFirstPosition = firstVisibleItemPosition == 0` and `atLastPosition = visibleCount > 0 && lastVisible == total - 1`, both updated in `onScrollStateChanged` only (not on every scroll). Quirk: the flags can be stale while a fling is running.

### 10.8 Zoom fling (webtoon)

OBSERVED. `zoomFling(velocityX, velocityY)` (`WebtoonRecyclerView.kt:134-168`): if `currentScale <= 1` return false. `distanceTimeFactor = 0.4f`.
- if `velocityX != 0`: `dx = 0.4 * vx / 2`; `newX = getPositionX(x + dx)`; animate x from `x` to `newX`.
- if `velocityY != 0 && (atFirstPosition || atLastPosition)`: `dy = 0.4 * vy / 2`; `newY = getPositionY(y + dy)`; animate y.
- `duration = 400` ms, `DecelerateInterpolator`. Returns true.
Animated values pass through `getPositionX/Y` on each update.

`zoomScrollBy` and `zoomFling` only run on a zoomed webtoon. At scale 1 the RecyclerView handles flings.

### 10.9 Webtoon `minimumScaleType` and tiling

Webtoon strips use `SCALE_TYPE_FIT_WIDTH` in the image holder, but the strip's zoom uses the RecyclerView transform above, not the image's own zoom. The image inside a webtoon page is not zoomable (`WebtoonSubsamplingImageView.onTouchEvent` returns false, `WebtoonSubsamplingImageView.kt:179-181`), and webtoon animated images are `AppCompatImageView`. Webtoon pages are decoded at view size (§8.4), so no tiling occurs in webtoon.

---

## 11. Chapter transition card

### 11.1 Content rules

OBSERVED. `app/src/main/java/eu/kanade/presentation/reader/ChapterTransition.kt` (area:43, rendered by `ReaderTransitionView` in this area):

For `Prev(from = curr, to = prev)`:
- top: label `transition_previous` ("Previous:"), chapter = `to` (the previous chapter). If `to == null`: fallback notice `transition_no_previous` ("There's no previous chapter").
- bottom: label `transition_current` ("Current:"), chapter = `from` (current).
- Gap warning (`missing_chapters_warning` plural, count = `calculateChapterGap(curr, goingTo)`), shown above the bottom card when `bottomChapter != null && gap > 0`. Note the arguments order is `(currChapter, goingToChapter)`.

For `Next(from = curr, to = next)`:
- top: label `transition_finished` ("Finished:"), chapter = `from`.
- bottom: label `transition_next` ("Next:"), chapter = `to`. If `to == null`: fallback `transition_no_next` ("There's no next chapter").
- Gap warning count = `calculateChapterGap(goingTo, curr)`, shown above the bottom card when `bottomChapter != null && gap > 0`.

Each chapter card (`ChapterText`): header in titleMedium with the label; name in titleLarge at 20 sp, `maxLines = 5`, ellipsis; if downloaded, an inline check icon (22 sp, content description `label_downloaded` = "Downloaded") plus a space precedes the name; scanlator (if non-null) in bodySmall, `maxLines = 2`, ellipsis, secondary alpha, 2 dp top padding. Spacer 24 dp between cards. Text column `widthIn(max = 460.dp)`, `fillMaxWidth`.

Cards (`NoChapterNotification` and `ChapterGapWarning`): outlined card, transparent container, content onSurface; row with 16 dp horizontal and 12 dp vertical padding, 16 dp spacing. Info icon (primary tint) for the notice; Warning icon (error tint) for the gap.

Plural `missing_chapters_warning`: one → "Skipping %d chapter, either the source is missing it or it has been filtered out"; other → "Skipping %d chapters, either the source is missing them or they have been filtered out".

### 11.2 Downloaded flags

OBSERVED. `ReaderTransitionView.bind` (`V/ReaderTransitionView.kt:30-49`), only when `manga != null` (else nothing is shown):
- `currChapterDownloaded = transition.from.pageLoader?.isLocal == true`. This checks the page loader type, not the download index.
- `goingToChapterDownloaded = manga.isLocal() || transition.to?.chapter?.let { downloadManager.isChapterDownloaded(chapterName = name, chapterScanlator = scanlator, mangaTitle = manga.title, sourceId = manga.source, skipCache = true) } ?: false`.

### 11.3 Loading and error states for the adjacent chapter

OBSERVED. Pager holder (`PagerTransitionHolder.observeStatus`, lines 82-97) and webtoon holder (`WebtoonTransitionHolder.observeStatus`, lines 394-410): observe `to.stateFlow` (only if `to != null`). On each state, clear the states container, then:
- `Loading` → indeterminate `CircularProgressIndicator` + text `transition_pages_loading` ("Loading pages…").
- `Error(e)` → text `transition_pages_error` = "Failed to load pages: %1$s" with `e.message ?: ""`, then Retry (`action_retry` = "Retry") → `requestPreloadChapter(to)`.
- `Wait`, `Loaded` → nothing.
- Webtoon additionally sets `pagesContainer.isVisible = pagesContainer.isNotEmpty()`.

Retry in pager: `ReaderButton` (disables the pager gesture while pressed, §3.2). Webtoon: plain `AppCompatButton`.

### 11.4 Layout metrics

OBSERVED.
- Pager (`PagerTransitionHolder.kt:55-68`): the holder is a vertical LinearLayout with 64 dp left and right padding, 0 dp top and bottom, gravity center. Children: the transition view (wrap content) and `pagesContainer` (MATCH_PARENT x WRAP_CONTENT, vertical, center).
- Webtoon (`WebtoonTransitionHolder.kt:356-372`): the holder is `MATCH_PARENT x WRAP_CONTENT`, vertical, centre, with 128 dp top and bottom padding and 32 dp left and right padding. `pagesContainer` has 16 dp top and bottom margins.

---

## 12. Lifecycle, threading, timings

### 12.1 Threads

OBSERVED.
- Viewer and holders run on the main thread (`MainScope`, which is `Dispatchers.Main`).
- Page loading: `launchIO { loader.loadPage(page) }` (`PagerPageHolder.kt:94-96`, `WebtoonPageHolder.kt:134-136`).
- Decoding and transforms: `withIOContext { streamFn().use { process(...) } ... }` then `withUIContext { setImage(...) }`.
- Preload requests: `activity.requestPreloadChapter(chapter)` → `lifecycleScope.launchIO { viewModel.preload(chapter) }` (`ReaderActivity.kt:702-704`).
- Adapter mutation from split: `activity.runOnUiThread` (`PagerViewer.kt:441`).

### 12.2 Timings and limits (complete)

| Name | Value | Where |
|---|---|---|
| Pager preload window | last 5 pages (`pages.size - number < 5`) | `PagerViewer.kt:239`, `WebtoonViewer.kt:214` |
| Prev tail kept in list | 2 pages | `PagerViewerAdapter.kt:60`, `WebtoonAdapter.kt:51` |
| Next head kept in list | 2 pages | `PagerViewerAdapter.kt:108`, `WebtoonAdapter.kt:78` |
| `offscreenPageLimit` (pager) | 1 | `PagerViewer.kt:87` |
| Webtoon item view cache | 4 | `WebtoonViewer.kt:364` |
| Webtoon extra layout space | `heightPixels * 3 / 4` | `WebtoonViewer.kt:53,58` |
| Webtoon tap/key scroll | `heightPixels * 3 / 4` | `WebtoonViewer.kt:53` |
| Double-tap zoom anim (pager SSIV) | pref `pref_double_tap_anim_speed` (default 500) × animator scale, min 1 | `ReaderPageImageView.kt:293,421` |
| Webtoon zoom anim | 200 ms | `WebtoonRecyclerView.kt:346` |
| Webtoon zoom fling | 400 ms | `WebtoonRecyclerView.kt:163` |
| Pager pan-before-page anim | 250 ms, EASE_OUT_QUAD | `ReaderPageImageView.kt:240-242` |
| Landscape zoom delay | 500 ms | `ReaderPageImageView.kt:132` |
| Landscape zoom anim | 500 ms, EASE_IN_OUT_QUAD | `ReaderPageImageView.kt:155-156` |
| Long-tap | platform (AOSP default 500 ms; `ViewConfiguration.getLongPressTimeout`) | `GestureDetectorWithLongTap.kt:22` |
| Double-tap window | platform (AOSP default 300 ms) | `GestureDetectorWithLongTap.kt:23` |
| Touch slop | platform (AOSP default 8 dp) | `GestureDetectorWithLongTap.kt:21` |
| Haptic | `HapticFeedbackConstants.LONG_PRESS` | `Pager.kt:42`, `WebtoonRecyclerView.kt:253` |
| SSIV quick-scale threshold | 20 dp | SSIV source line 304 |
| SSIV long click | 600 ms (unused by reader) | SSIV source |
| SSIV min tile DPI | 180 | `ReaderPageImageView.kt:259` |
| SSIV max tile size | 2048 px (`hardwareBitmapThreshold`) | `ReaderPageImageView.kt:256`, `GLUtil.kt:50` |
| SSIV min dpi (temporary) | 1 | `ReaderPageImageView.kt:295` |
| Max zoom (static pager) | 5 x fit scale | `ReaderPageImageView.kt:364,278` |
| Max zoom (webtoon) | 3.0 | `WebtoonRecyclerView.kt:349` |
| Min zoom (webtoon) | 0.5 (1.0 when zoom-out disabled) | `WebtoonRecyclerView.kt:347` |
| Error card height (webtoon) | 0.8 × recycler height | `WebtoonPageHolder.kt:281` |
| Progress indicator top margin (webtoon) | recycler height / 4 | `WebtoonPageHolder.kt:266` |
| Webtoon gap between pages (gapped mode) | 15 dp bottom margin | `WebtoonPageHolder.kt:101` |
| Webtoon side padding | pref 0..25 (%) of screen width | `ReaderPreferences.kt:206-207`, `WebtoonPageHolder.kt:104` |
| Background sampling | `h / 25` row step | `ImageUtil.kt:443` |
| Background minimum size | 50 px on each side | `ImageUtil.kt:345` |
| Border filled-ratio | 0.0025 | `image-decoder/borders.h:11` |

### 12.3 Concurrency and cancellation

OBSERVED.
- Holder loads are cancelled on `onDetachedFromWindow` (pager) and on `recycle()` / rebind (webtoon: `bind` cancels the previous `loadJob`).
- `PagerTransitionHolder` and `WebtoonTransitionHolder` cancel `stateJob` on detach or recycle.
- `supervisorScope` in holders keeps a failing status collector from cancelling siblings.
- `collectLatest` in status observers: a new status cancels the previous branch (for example, a DOWNLOAD_IMAGE progress collector is cancelled when status changes to READY).
- `PagerViewer.destroy()` cancels `scope`; `WebtoonViewer.destroy()` cancels `scope`.

### 12.4 Known hazards in the observed code (Rust must not copy blindly)

OBSERVED, each with the required behaviour:
1. `PagerViewerAdapter.setChapters` uses `pages.last()` on `curr.pages`. An empty loaded chapter crashes. Proposal: treat an empty list as "no pages" and show the transition only.
2. `PagerViewerAdapter.onPageSplit` indexes `items[placeAtIndex]` without a bounds check. Safe in practice because a trailing transition always exists (§5.3 step 7), but the Rust version must check bounds and return.
3. `PagerViewer.moveToPage` calls `setCurrentItem(position, true)` (smooth) regardless of settings (§6.6).
4. Webtoon `refreshAdapter` range argument is a count, not an end index (§7.7).
5. `PagerViewer` swallows `NullPointerException`, `IndexOutOfBoundsException`, `IllegalArgumentException` in `Pager.onTouchEvent` and `IllegalArgumentException` in `onInterceptTouchEvent`. These guard Android ViewPager bugs. Not needed in Rust.
6. `ReaderPageImageView.onPageSelected` fallback paths replace the earlier `setOnImageEventListener` (§10.3), so only one `onReady` action runs.
7. `WebtoonFrame.dispatchTouchEvent` clamps coordinates into the recycler rectangle. Not needed in Rust (§3.4).

---

## 13. Interactions with other areas

| Interaction | Owner of the other side | Contract |
|---|---|---|
| `ReaderActivity.onPageSelected(page)` | area:43-reader-core | View model updates current page number and progress. Called on every page selection (pager) and every new current page (webtoon). |
| `ReaderActivity.onPageLongTap(page)` | area:43 | Opens the page actions dialog (`viewModel.openPageDialog`). |
| `ReaderActivity.requestPreloadChapter(chapter)` | area:43 | Loads the chapter's page list in the background (`viewModel.preload`). Idempotent for loaded chapters (assumed; confirm in area:43). |
| `ReaderActivity.toggleMenu()` / `showMenu()` / `hideMenu()` | area:43 | Menu visibility; `showMenu` only when hidden, `hideMenu` only when visible. |
| `viewModel.state.value.menuVisible` | area:43 | Read in key handlers and long-tap gates. |
| `viewModel.state.value.viewer` | area:43 | The viewer instance; `dispatchKeyEvent` uses it. |
| `activity.binding.navigationOverlay.setNavigation(navigator, showOnStart)` | area:43 | Renders the tap-zone overlay. |
| `activity.isScrollingThroughPages` | area:43 | Set by the page slider; never reset (§6.5). |
| `activity.viewModel.manga` | area:43 | Used by transitions for `isLocal` and download lookups. |
| `ReaderChapter.pageLoader`, `Page.statusFlow`, `Page.progressFlow`, `Page.stream`, `ReaderChapter.stateFlow` | area:43 (loaders) and source-api | Status and data source for holders. |
| `ReaderChapter.ref()/unref()` | area:43 | Ref counting; `unref` to zero recycles the loader and resets state to Wait. |
| `MangaDownloadManager.isChapterDownloaded` | area:23-downloads-manga | Download index lookup for "Downloaded" icon. |
| `ImageUtil.*` | area:13 | Decode helpers used directly. |
| `ReaderPreferences` keys | area:43 (class) and each consumer here | Defaults in p1-data-and-config.md. |
| `BasePreferences.hardwareBitmapThreshold` | area:02 | Max tile size (default 2048). |
| `BasePreferences.alwaysDecodeLongStripWithSSIV` | area:02 | Webtoon decode path (default false). |
| Coil `customDecoder` / `cropBorders` extras | area:34-images-covers-cache | Native decoder with border crop. |
| `WebViewActivity.newIntent` | area:36 | "Open in WebView" for http image URLs. Replaced by default browser (§T-44a-064). |
| `ActivityCompat.recreate` on theme change | area:43 | Replaced by in-place restyle (§T-44a-043). |

---

## 14. User-visible message keys (complete for this area)

Keys are `MR.strings.*` (`i18n/src/commonMain/moko-resources/base/strings.xml`) unless noted. English values quoted.

| Key | English | Used in |
|---|---|---|
| action_menu | Menu | tap zone overlay label (`ViewerNavigation.kt:15`) |
| nav_zone_prev | Prev | overlay label |
| nav_zone_next | Next | overlay label |
| nav_zone_left | Left | overlay label |
| nav_zone_right | Right | overlay label |
| transition_pages_loading | Loading pages… | `PagerTransitionHolder.kt:108`, `WebtoonTransitionHolder.kt:421` |
| transition_pages_error | Failed to load pages: %1$s | `PagerTransitionHolder.kt:121`, `WebtoonTransitionHolder.kt:434` |
| action_retry | Retry | error buttons (`PagerTransitionHolder.kt:127`, `WebtoonTransitionHolder.kt:439`, `reader_error.xml:21`) |
| transition_previous | Previous: | `ChapterTransition.kt:61` |
| transition_current | Current: | `ChapterTransition.kt:64` |
| transition_no_previous | There's no previous chapter | `ChapterTransition.kt:67` |
| transition_finished | Finished: | `ChapterTransition.kt:73` |
| transition_next | Next: | `ChapterTransition.kt:76` |
| transition_no_next | There's no next chapter | `ChapterTransition.kt:79` |
| missing_chapters_warning (plural) | see §11.1 | `ChapterTransition.kt:195` |
| label_downloaded | Downloaded | check icon description (`ChapterTransition.kt:252`) |
| decode_image_error | The image couldn't be loaded | `reader_error.xml:13` |
| action_open_in_web_view | Open in WebView | `reader_error.xml:28` |
| tapping_inverted_none / horizontal / vertical / both | None / Horizontal / Vertical / Both | `TappingInvertMode.titleRes` (settings UI, area:43) |
| l_nav / kindlish_nav / edge_nav / right_and_left_nav / disabled_nav / label_default | L shaped / Kindle-ish / Edge / Right and Left / Disabled / Default | `ReaderPreferences.TapZones` (settings UI, area:43) |
| scale_type_fit_screen / stretch / fit_width / fit_height / original_size / smart_fit | Fit screen / Stretch / Fit width / Fit height / Original size / Smart fit | `ReaderPreferences.ImageScaleType` |
| zoom_start_automatic / left / right / center | Automatic / Left / Right / Center | `ReaderPreferences.ZoomStart` |

Strings for the reader theme and general settings (`black_background`, `gray_background`, `white_background`, `automatic_background`) belong to area:43.

---

## 15. Open questions (answered by the design, listed here so reviewers can challenge them)

1. Pager `moveToPage` uses smooth scrolling regardless of the transitions setting (observed). Proposal: follow `usePageTransitions`. Needs a decision.
2. "Stretch" maps to CENTER_CROP (observed, §10.2). Proposal: keep CENTER_CROP (user-visible behaviour) and rename the help text to "Fill (crop)". Decision needed.
3. `isScrollingThroughPages` is never reset (observed, §6.5). Proposal: reset after the next selection. Decision needed.
4. Webtoon `threshold` read once (observed, §7.6). Proposal: live. Low risk.
5. Pager double tap does nothing (observed, §3.2). Proposal: keep (no pager double-tap action; pinch and double-tap zoom live on the page image). Confirm.
