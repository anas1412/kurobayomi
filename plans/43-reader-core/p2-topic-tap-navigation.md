# Topic: tap navigation model, inversion, long press (43-reader-core part 2)

Scope files: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt`, `app/src/main/java/eu/kanade/tachiyomi/util/lang/RectFExtensions.kt` (helper, read because `ViewerNavigation` calls it), `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/GestureDetectorWithLongTap.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt` (tap-related keys and enums).

Layouts (region geometry for each named zone set) are in area 44: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/` (DisabledNavigation.kt, EdgeNavigation.kt, KindlishNavigation.kt, LNavigation.kt, RightAndLeftNavigation.kt). This topic covers the shared model only; the layout index mapping is in `p2-behavior.md` §3.4.

## 1. Model

```
ViewerNavigation (abstract)
  invertMode: TappingInvertMode                 (public var, set by owner)
  regionList: List<Region>                      (abstract, protected var; provided by subclass)
  getRegions(): regionList.map { it.invert(invertMode) }
  getAction(pos: PointF): NavigationRegion

Region(rectF: RectF, type: NavigationRegion)    rectF normalized to [0,1] x [0,1]
NavigationRegion (sealed): MENU | PREV | NEXT | LEFT | RIGHT
  each: nameRes (StringResource), color (ARGB)
```

Coordinates: `RectF` is normalized. The owner (viewer) passes a `PointF` in the same normalized space. Shipped rectangles in this file: only the constant `constantMenuRegion = RectF(0f, 0f, 1f, 0.05f)` (a top strip, dead logic).

Semantics of a region's `type`: PREV means "go to previous page in reading order", NEXT "go to next page". LEFT and RIGHT are screen-direction zones: the viewer maps them to previous/next according to reading direction (L2R: left = prev; R2L: right = prev; vertical: up = prev). That mapping is in area 44 (pager/webtoon viewers), not here. The model deliberately stores screen-direction zones (`LEFT`, `RIGHT`) separately from logical zones (`PREV`, `NEXT`) so layouts like "Right and Left" can be expressed with screen-direction names.

## 2. Inversion

`TappingInvertMode` (stored per viewer type: `reader_tapping_inverted` for paged, `reader_tapping_inverted_webtoon` for webtoon):

| Mode | horizontal flip | vertical flip |
|---|---|---|
| NONE | no | no |
| HORIZONTAL | yes | no |
| VERTICAL | no | yes |
| BOTH | yes | yes |

Rect transforms (`RectFExtensions.kt:6-18`):
- BOTH: `(1 - right, 1 - bottom, 1 - left, 1 - top)`
- VERTICAL: `(left, 1 - bottom, right, 1 - top)`
- HORIZONTAL: `(1 - right, top, 1 - left, bottom)`
- NONE: unchanged (returns `this`).

Inversion moves the rectangle but not its `type`. Example: with HORIZONTAL inversion, a LEFT zone covering x∈[0, 0.33] moves to x∈[0.67, 1]; a tap on the left third now hits the MENU fallback, and a tap on the right third hits LEFT.

Propagation: `ViewerNavigation.invertMode` is set by the owning config: pager `pagerNavInverted` → `tappingInverted` → `navigator.invertMode` (`PagerConfig.kt:79-84`); webtoon similarly with `webtoonNavInverted`. A change calls `navigationModeChangedListener` for the pager (`PagerConfig.kt:81-84`: `.drop(1)` then the listener).

## 3. Hit test

`getAction(pos)` (`ViewerNavigation.kt:149-158`):
1. Build `regions = getRegions()` (applies inversion to every region on every call).
2. First region in list order whose `rectF.contains(x, y)`.
3. Return its `type`, else MENU.

Android `RectF.contains(x, y)` is half-open: `left < right && top < bottom && left <= x < right && top <= y < bottom`. An empty rect (zero width or height) contains nothing. Edges at exactly 1.0 are not contained, so a tap at x = 1.0 falls through to MENU (the Rust port uses `RectN::contains` with the same rule; see tests in p2-edge-cases-and-tests.md E-05).

The first-match rule matters when zones overlap. Layouts in area 44 are expected to be disjoint; the model does not enforce that.

## 4. Overlay colors

The navigation overlay (area 44, `ReaderNavigationOverlayView`) draws each region with its `color` and label `nameRes`. Constants (ARGB, alpha 0xCC):
| Type | RGB |
|---|---|
| MENU | 95 81 8D |
| PREV | FF 77 33 |
| NEXT | 84 E2 96 |
| LEFT | 7D 11 28 |
| RIGHT | A6 CF D5 |

Rust proposal: keep the same RGB values; the overlay alpha is 0xCC. Under the user's GTK theme the labels use the theme font; colors are fixed (they are semantic zone colors, not theme colors).

## 5. Long press

The long-press action is wired through `GestureDetectorWithLongTap` (`GestureDetectorWithLongTap.kt`), which adds `onLongTapConfirmed(ev)` to Android's `GestureDetector`. Its state machine and constants are specified in `p2-behavior.md` §8. Summary:
- Arm on DOWN unless within `doubleTapTime` of the previous UP.
- Cancel on move beyond slop, on UP, on CANCEL and on POINTER_DOWN.
- Fires `onLongTapConfirmed` with the DOWN event copy.
- Gated by `reader_long_tap` (default true) at the consumer (`ViewerConfig.longTapEnabled`, area 44 `Pager.kt`).

Rust: `src/reader/gesture.rs::LongPressTracker`, timer via `glib::timeout_add_local_once`. Pure logic; unit tests E-08..E-11.

## 6. Why the Rust port keeps "Menu" as the fallback

Taps outside every zone open the menu in Kotlin (both branches). This includes taps in the middle of the page in the Disabled layout (a layout with no zones toggles the menu on any tap). Keep identical behavior; document in the layout help text.

## 7. Dropped or changed

- `constantMenuRegion`: dead branch in Kotlin. Dropped in Rust (no behavior change).
- `setGestureDetectorEnabled` toggling from `ReaderButton` (`ReaderButton.kt:109-115`): replaced by GTK's native claim behavior (topic-level decision in p2-rust-design §9).
