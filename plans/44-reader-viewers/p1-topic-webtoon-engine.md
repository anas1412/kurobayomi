# Topic: the webtoon (long strip) engine

Covers `webtoon/WebtoonViewer.kt`, `webtoon/WebtoonRecyclerView.kt`, `webtoon/WebtoonLayoutManager.kt`, `webtoon/WebtoonAdapter.kt`, `webtoon/WebtoonFrame.kt`, `webtoon/WebtoonPageHolder.kt`, `webtoon/WebtoonTransitionHolder.kt`, `webtoon/WebtoonConfig.kt`, `webtoon/WebtoonBaseHolder.kt`, `webtoon/WebtoonSubsamplingImageView.kt`. Observed rules: p1-behavior.md §5.5, §7, §8.3-8.6, §10.7-10.9. Paths are repo-relative to `/home/blackbox/aniyomi-src`.

Two modes share this engine: `WebtoonViewer(isContinuous = true)` ("Webtoon", no gap) and `WebtoonViewer(isContinuous = false)` ("Long strip with gaps", a 15 dp bottom margin under each page; `WebtoonPageHolder.kt:100-102`).

---

## 1. Geometry

Coordinate system: strip space, y down, origin at the top of the first item. `offsets[i]` is the top of item `i`, `heights[i]` its height. Viewport height `vh`, scroll position `s` (top of the visible region in strip space).

Item heights:
- Page: `height = page_view_width * (decoded_h / decoded_w)` where `page_view_width = widget_width - 2 * margin`, `margin = widget_width * padding_pct / 100` (integer; B§8.6). Before decode: placeholder `height = vh`.
- Transition card: content height (wrap content) plus `128 dp` top and bottom padding (B§11.4).
- Gap mode: pages get `+15 dp` bottom margin (B§8.6).

`extra_layout = vh * 3 / 4` (the webtoon scroll distance, B§7.1). Realised items are those with `offsets[i] + heights[i] > s - extra_layout` and `offsets[i] < s + vh + extra_layout`. Realised items are bound; others are unbound and their decode requests cancelled (B§8.1 unbind rule).

Lookup: `first = offsets.partition_point(|&o| o + h(o) <= s - extra)` style binary search on the prefix sums (`O(log n)`).

Height changes (decode completes) shift `offsets` for every later item. Update the prefix sums in O(n) per change, or keep a Fenwick tree when strips exceed a few thousand items (Proposal; the Kotlin RecyclerView has the same shifting and no special handling, so O(n) per change is the parity baseline).

---

## 2. Current page and events

`last_end_visible` (B§7.3): the bottom-most realised item whose bottom is inside the viewport or whose top is above the viewport top. This item is the reported current page. Reports happen when this item changes (`current_item != new`, B§7.3).

Derived events on each scroll update (B§7.2):
1. Current page → `ViewerEvent::PageSelected` (ReaderPage) or transition handling.
2. `|dy| > threshold` while the menu is visible → hide menu. `threshold` is the `ReaderHideThreshold` value (5/13/31/47). Proposal: read live (B§7.6).
3. `dy < 0` and the first visible item is a `Prev` transition with a destination → `PreloadChapter(destination)`.
4. The last item (bottom-most realised, B§7.2 step 4) is a `Next` transition with no destination → `ShowMenu`.

Preload rule for pages (B§7.3, `onPageSelected`): page in the last five of its chapter and `allow_preload` → `PreloadChapter(transitionChapter)` where `transitionChapter` is the last list item's destination (Next transition `to`) or, when the last item is a page, that page's chapter (observed quirk; the request is for a chapter that may already be loaded, which the view model treats as a no-op).

`check_allow_preload(page)` (B§7.3): true when the page belongs to the current page's chapter or to the chapter of the last list item; `page == None` and `current == None` are true.

---

## 3. Tap, key and scroll handling

- Tap zones: `LNavigation` default or the user's mode (B§2). `MENU` toggles the menu; `NEXT`/`RIGHT` scroll down by `scroll_distance`; `PREV`/`LEFT` scroll up. `scroll_distance = vh * 3 / 4` (integer division, B§7.4). Smooth when `use_page_transitions` (animated `vadjustment`), instant otherwise.
- Tap coordinate normalisation: `y / original_height` where `original_height` is the height at first allocation (B§2.1). Store it once.
- Keys: the table in p1-behavior.md §4.4. Consumed on both edges.
- Pointer scroll: native (`gtk::ScrolledWindow` semantics), not intercepted.

---

## 4. Zoom (strip-level)

Applied as a transform on the strip (`snapshot.translate` + `scale`), never by resizing the widget except for the height change at scale < 1 (B§10.7 step 3).

State: `scale` (1.0), `tx`, `ty`, `half_w`, `half_h`, `orig_h`, `zoom_out_disabled`, `double_tap_enabled`, `is_zooming`, `at_edge_top`, `at_edge_bottom`.

Rules (all from B§10.7-10.8):
- `min = zoom_out_disabled ? 1.0 : 0.5`; `max = 3.0`.
- Pinch: `scale = clamp(scale * factor, min, 3.0)`; recompute height `orig_h / scale` when `scale < 1` else `orig_h`; `half_h = height / 2`; if `scale != 1` clamp `tx`, `ty`; else reset both to 0.
- Clamp x: `scale < 1` → 0; else `±half_w * (scale - 1)`.
- Clamp y: `scale < 1` → `orig_h / 2 - half_h`; else `±half_h * (scale - 1)`.
- Double tap (`double_tap_enabled && !is_zooming`): if `scale != 1` → animate to 1 (200 ms), height back to `orig_h`; else animate to 2 with `tx = (half_w - x) * 1`, `ty = (half_h - y) * 1` (200 ms, decelerate).
- Quick scale: double tap then one-finger drag (B§10.4 formula, `threshold = 20 dp`); the drag consumes the pointer while quick scaling.
- Drag when zoomed: `dx` always applies; `dy` applies only at `at_edge_top || at_edge_bottom` (B§10.7). Otherwise the strip scrolls normally. The first `touchSlop` of drag is subtracted when the drag starts (B§10.7).
- Fling when zoomed: `dx = 0.4 * vx / 2`; `dy` only at edges; animate 400 ms, decelerate, values pass through the clamps.
- `zoom_out_disabled` turned on while `scale < 1`: animate back to 1 over 200 ms (B§10.7 `zoomOutDisabled` setter).
- `onScaleEnd`: animate to `min` when below it (rarely reached; kept for parity).

Edge flags (`at_edge_*`) are recomputed on scroll-state change only, not on every scroll (observed; B§10.7). Proposal: recompute on every scroll; the observed staleness is a bug, not a feature.

---

## 5. Transitions and the list

- List builder: B§5.5. No InsertPages in webtoon. Dual-page split for webtoon is applied inside the page decode (`splitAndMerge`, B§9.2).
- Transition cards: B§11, with the webtoon paddings (B§11.4).
- Retry in the webtoon transition card is a plain button (B§11.3).
- `forceTransition` is `always_show || current_item is transition` (B§5.4).

---

## 6. Holder behaviour

Same status machine as the pager (B§8.1). Differences:
- Placeholder height is the viewport height while loading (B§8.6).
- Decode target width is the view's width (B§8.4), with `Precision.INEXACT` semantics: any size at or above the target is acceptable, so the Rust decoder downsamples to `target_width` and never upsamples.
- No auto background, no landscape zoom, no zoom start (B§8.3).
- Crop borders from `crop_borders_webtoon` (B§9.4).
- Error card height `0.8 * viewport_h` (B§8.5).
- Progress indicator top margin `viewport_h / 4` (B§8.6).

---

## 7. Refresh on settings change

Any image-affecting setting (crop, dual split/rotate, padding, theme for the background) triggers `refreshAdapter`: compute the visible centre item, rebind items in `[centre - 3, centre + 3]` (the Kotlin range is wider by an off-by-one because of the count-as-end argument; B§7.7 quirk). Proposal: exact range.

Theme change (`readerTheme` after first emission): Kotlin recreates the activity (`ActivityCompat.recreate`). Rust: restyle in place (T-44a-043).

---

## 8. Test plan

- Geometry: heights with padding 0 and 25 %, gap mode, placeholder before decode, prefix sums after a height change.
- Current item: `last_end_visible` at top, middle and bottom of a 50-item strip; empty realised set returns `None`.
- Zoom: pinch clamp at 0.5 and 3.0, zoom-out disabled clamp at 1.0, centring at `scale < 1`, double-tap focus point math, quick scale sequence (T-44a-080).
- Menu rules: threshold boundary `|dy| == 31` does not hide, `32` hides.
- Preload: trigger at the 5th last page; transition destination request.
