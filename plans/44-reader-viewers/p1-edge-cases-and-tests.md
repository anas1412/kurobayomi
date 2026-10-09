# 44-reader-viewers: edge cases, failure modes, tests

IDs `E-nn` are edge cases. Each has an expected result and the Kotlin location it comes from. Test IDs point at the implementation tasks in p1-tasks.md (T-44a-078 to T-44a-082 are the test tasks).

Paths are repo-relative to `/home/blackbox/aniyomi-src` unless noted.

---

## 1. Upstream tests

The scope contains no `*Test.kt` file. Related upstream tests live in the domain module and cover the gap function used by the viewers:

`domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt`:
- `calculateChapterGap returns difference` (lines 37-47): `(10, 9) → 0`, `(10, 8) → 1`, `(10, 8.5) → 1`, `(10, 1.1) → 8`, the same through the `Double` overload.
- `calculateChapterGap returns 0 if either are not valid chapter numbers` (lines 50-56): `(-1, 10) → 0`, `(99, -1) → 0`, and the `Double` overload.

These vectors are ported as Rust tests `reader::model::gap::tests::upstream_vectors` (T-44a-079). The Rust function takes `Option<f64>` pairs and a `recognised` flag, so `-1` maps to "not recognised" as the domain does.

---

## 2. Edge cases by subsystem

### 2.1 Items and transitions

**E-01 Loaded chapter with zero pages.** `PagerViewerAdapter.setChapters` calls `pages.last()` (`PagerViewerAdapter.kt:76`). Kotlin throws `NoSuchElementException`.
- Expected (Rust): no crash. The chapter contributes no page items. Transitions are still computed. The `preprocessed` insert lookup is skipped. The first-layout move is skipped and the pager becomes visible on the transition.

**E-02 `requestedPage` beyond the last page.** Kotlin: `min(requestedPage, pages.lastIndex)` (`PagerViewer.kt:288`, `WebtoonViewer.kt:249`).
- Expected: clamp. Negative values are impossible; assert in debug.

**E-03 Transition equality.** `ChapterTransition.equals` (`model/ChapterTransition.kt:18-24`) makes `Prev(A,B) == Next(B,A)`. A `None` destination never matches.
- Test: `Prev(A, Some(B)) == Next(B, Some(A))` is true (swapped pair); `Prev(A, None) == Next(A, None)` is true (same pair); `Next(A, None) != Next(A, Some(B))`; `Hash` is equal for equal values.

**E-04 Gap edge values.** `calculateChapterGap` returns `floor(h) - floor(l) - 1`, which can be `-1` when the integer parts are equal (`10.0`, `10.5`). Callers test `> 0`.
- Expected: `(10.5, 10.0) → -1`, and "no gap" in the UI.

**E-05 Unrecognised numbers.** A chapter with number `-1` (not recognised) gives gap 0 (`MissingChapters` domain code, lines 40-44).
- Expected: no transition is forced by the gap.

**E-06 No previous chapter.** `prev == null` makes `prev?.state !is Loaded` true, so the Prev transition is always added (B§5.3 step 4). Expected: first item is the Prev transition with `to = None`, showing "There's no previous chapter".

**E-07 `always_show_chapter_transition = false` with loaded neighbours.** Expected: no Prev/Next transition unless a gap or a non-loaded neighbour exists or the reader is on a transition. The list then runs straight from the previous chapter's last two pages into the current chapter, and out to the next chapter's first two pages.

**E-08 Preprocessed insert pages.** A split of a neighbour-chapter page stores the insert in `preprocessed[index]` and it is inserted at `key + 1` when that chapter becomes current. If the last current page has a preprocessed insert, the pager jumps to that insert (`insertPageLastPage`, `PagerViewerAdapter.kt:69-126`), so the insert is not skipped.
- Test: two neighbour inserts; the current chapter's last page has an insert; `apply` returns the insert key as the jump target.

**E-09 R2L insert cycle guard.** `onPageSplit` returns when `items[place - 1]` is an insert (`PagerViewerAdapter.kt:181-183`).
- Test: R2L, current index 5, insert at 4 → no second insert.

**E-10 Insert placement at the end of the list.** Kotlin reads `items[placeAtIndex]` with no check (`PagerViewerAdapter.kt:186`). The list always ends with a Next transition (B§5.3 step 7 in the default setting), so the index is in range in practice.
- Expected (Rust): `place >= len` returns `false` without inserting. Test with a list that ends in a page (`always_show = false`, loaded next chapter).

**E-11 Split pair forward flag.** Split pairs share `number`. `forward = page is InsertPage` when numbers are equal (`PagerViewer.kt:183-186`).
- Test: parent → insert (forward), insert → parent (backward).

**E-12 Transition-to-page direction.** `currentPage is Prev && page is ReaderPage` → `forward = false` (`PagerViewer.kt:190-191`).

### 2.2 Pager navigation

**E-13 Pan before page turn with a narrow remainder.** `canPanRight` needs `remaining.right > 1`. A remainder of exactly 1.0 px does not pan (B§10.4).
- Test: `right = 1.0 → false`, `1.01 → true`.

**E-14 Move at either end.** `moveRight` does nothing at `count - 1`; `moveLeft` at 0 (`PagerViewer.kt:328, 342`). No wrap and no chapter change.

**E-15 Direction-normalised keys.** Ctrl+Right on R2L calls `moveToNext()`, which is `moveLeft()`. Plain Right calls `moveRight()`.
- Test table in `reader::pager::nav::tests`.

**E-16 Tap in a region gap.** Edge mode: left column (x < 0.33) NEXT full height; right column (x >= 0.66) NEXT full height; bottom-centre (0.33 <= x < 0.66, 0.66 <= y < 1) PREV; everything else MENU (`ViewerNavigation.kt:59-68`, `navigation/EdgeNavigation.kt`).
- Test: `(0.5, 0.2)` in Edge → MENU; `(0.5, 0.8)` → PREV; `(0.33, 0.66)` → PREV (half-open left/top edge); `(0.66, 0.5)` → NEXT (right column starts at 0.66).

**E-17 Half-open rectangle boundaries.** `RectF.contains` is `left <= x < right` and `top <= y < bottom`. Test the exact boundary values `0.33`, `0.66`, `1.0`.

**E-18 Inverted zones.** `TappingInvertMode.BOTH` maps `(l,t,r,b)` to `(1-r, 1-b, 1-l, 1-t)`. Test: LNavigation with BOTH; point `(0.1, 0.1)` (originally PREV top) becomes NEXT.

**E-19 Disabled navigation.** Every tap is MENU; the overlay shows no zones (`navigation/DisabledNavigation.kt`).

**E-20 Pager double tap.** No action (B§3.2). Test: two taps within the double-tap window produce no page turn and no menu toggle.

**E-21 Long-tap gate.** `longTapEnabled == false` and menu hidden → no action. Menu visible → action even when disabled (B§3.2 operator observation).

**E-22 Long tap inside the double-tap window.** A down event within `doubleTapTime` of the previous up does not start the long-press timer (`GestureDetectorWithLongTap.kt:43-47`).

**E-23 Stale slop reference after double tap.** `downX/downY` are only refreshed when the timer starts, so after a double tap the slop check compares against the previous press position (`GestureDetectorWithLongTap.kt:44-50`). No timer is pending in that case, so the observable effect is none. Expected: reproduce the same condition; test that a double tap followed by a drag does not trigger a long press.

**E-24 Pointer scroll with dy == 0.** Maps to previous page (`PagerViewer.kt:428-432`).

**E-25 Volume keys off or menu visible.** The pager returns false so the system handles volume (`PagerViewer.kt:387-388`). The webtoon does the same (`WebtoonViewer.kt:313-314`).

**E-26 Key claim and release.** Handled keys return true on both down and up (B§4.2). The Rust controller must claim and swallow the matching release, else the release reaches the window.

### 2.3 Webtoon

**E-27 Menu threshold boundary.** `dy > threshold || dy < -threshold` (`WebtoonViewer.kt:94`). LOW (31): `31` does not hide, `32` hides. `-32` hides.

**E-28 Threshold read once.** Observed: a change in the threshold takes effect on the next viewer creation (`WebtoonViewer.kt:75-79`). Proposal: live. Test: change at runtime, next scroll uses the new value.

**E-29 End of book shows the menu.** Last realised item is a Next transition with no destination → `ShowMenu` (`WebtoonViewer.kt:106-110`).

**E-30 Refresh range.** `notifyItemRangeChanged(max(0, p-3), min(p+3, n-1))` passes a count (B§7.7). Proposal: rebind the exact indices `[p-3, p+3]`. Test: no index outside `[p-3, p+3]` is rebound.

**E-31 Scale below one.** `layoutParams.height = originalHeight / currentScale` (`WebtoonRecyclerView.kt:193-197`). At 0.5 the strip is twice the viewport height. Test: height and `clamp_y` centring value.

**E-32 Zoom out disabled while below 1.** Setting `zoomOutDisabled` animates back to 1 over 200 ms (`WebtoonRecyclerView.kt:38-43`).

**E-33 Double tap during an animation.** `onDoubleTapConfirmed` requires `!isZooming` (`WebtoonRecyclerView.kt:236`). Test: second double tap during the 200 ms animation is ignored.

**E-34 Double tap at scale 1 with focus at the edge.** `toX = (halfWidth - x) * 1` is written straight to `x` by the animation (`WebtoonRecyclerView.kt:103-117`, `zoom`), without `getPositionX`. The clamp applies only on the next pinch or drag. Expected: reproduce the unclamped animation endpoint, and clamp on the first gesture after it. Proposal: clamp every animation frame (visible difference only at strip edges; decision in p1-behavior.md §15 is not needed, this is a bug fix listed in T-44a-039).

**E-35 Quick-scale threshold reset.** After a threshold crossing `dist = 0`, so the next step has `lastDist = 0` and multiplier 1 (`SubsamplingScaleImageView.java`, quick-scale move handling; topic §4.2). Test in `zoom::pinch::tests::quick_scale_sequence`.

**E-36 Tap coordinates while zoomed.** Normalised by `originalHeight`, not current height (`WebtoonViewer.kt:121`). Test: zones unchanged at scale 2.

**E-37 Page padding at the widest setting.** `webtoon_side_padding = 25` gives 25% margins each side (`PagerConfig`/`WebtoonPageHolder.kt:104`), leaving 50% width. Test: page width clamp at 0 for a 0 px window.

**E-38 Gap mode margin.** 15 dp bottom margin only when `!isContinuous` (`WebtoonPageHolder.kt:100-102`). Test: CONTINUOUS_VERTICAL vs WEBTOON heights differ by 15 dp × scale.

**E-39 Placeholder height before the recycler has a height.** `parentHeight` is read when the holder is built (`WebtoonPageHolder.kt:262, 281`). If the recycler is not laid out yet it is 0, so the progress container has no height. Expected (Rust): compute the placeholder lazily from the current viewport at each layout.

### 2.4 Holders and decode

**E-40 Page with no loader.** Holder shows nothing and does not error (`PagerPageHolder.kt:91`, `WebtoonPageHolder.kt:132`). Expected: blank; logged at `warn`.

**E-41 Page with no stream while READY.** `page.stream ?: return` (`PagerPageHolder.kt:147`). Expected: blank, no error card.

**E-42 Decode failure.** `setImage` catch → error card (B§8.2 step 5). Retry calls `loader.retryPage(page)` (`PagerPageHolder.kt:275-277`). Test: decode error → error state; retry → QUEUE again.

**E-43 Late decode result after unbind.** Cancellation in Kotlin is cooperative (`loadJob.cancel()`); a decode in progress may finish. Expected (Rust): the generation check drops the texture.

**E-44 Rapid status changes.** `collectLatest` cancels the previous branch (for example, a download progress collector stops when READY arrives; `PagerPageHolder.kt:97-110`). Test: progress updates after READY do not update the indicator.

**E-45 Truncated or corrupt image.** SSIV reports `onImageLoadError`; Coil reports error (`ReaderPageImageView.kt:305-307, 334-336`). Expected: error card, no crash.

**E-46 Animated GIF with zero delay frames.** Clamp frame delay to ≥ 10 ms (proposal; Coil uses the decoder value). Test: a 0 ms frame plays at 10 ms.

**E-47 Animated AVIF/HEIF/JXL.** `isAnimatedAndSupported` is false (B§9.5), so the still path decodes the first frame. Test: animated AVIF fixture displays frame 0.

**E-48 Animated WebP on old Android.** Not applicable on Linux (no SDK gate). Treat animated WebP as animated on every platform.

**E-49 Huge page.** 1 × 20000 strip: pyramid levels; tile cap 2048; memory budget. Test: tile count and dimensions.

**E-50 Tiny page.** Automatic background returns white for `w < 50 || h < 50` (`ImageUtil.kt:345`).

**E-51 Square and wide images with split disabled.** `isWideImage` is only consulted when split or rotate is on (`PagerPageHolder.kt:185-206`).

**E-52 Split width with odd page width.** Left half `w/2`, right half `w - w/2` columns (`ImageUtil.kt:136-138`). Test: `w = 101` → left 50 cols, right 51 cols (RIGHT source rect starts at 51).

**E-53 Rotate direction.** `dualPageRotateToFitInvert` → −90 else +90 (`PagerPageHolder.kt:211`, `WebtoonPageHolder.kt:234`).

**E-54 Rotate has priority over split.** Pager `process` returns early on rotate (`PagerPageHolder.kt:186-188`). Test: rotate on, split on, wide page → rotated, no insert page.

**E-55 Border crop with no content.** `findBorders` returns the full rect when no line passes the fill rule. Test: all-white fixture → unchanged.

**E-56 Border crop with mixed fill.** Mixed black and white first line → no crop on that side (`borders.cpp:40-42`). Test fixture.

**E-57 Border crop producing zero size.** Degenerate result (for example, the content is one pixel). Expected: return the original and log.

**E-58 Background from edge colours.** Dark edges with light centre: `Gradient2` or `Solid(black)` per the tree in topic §5. Golden fixtures pin outputs.

**E-59 Landscape background.** Widget wider than tall → only `Solid(black)` or `Solid(white)`, never a gradient (B§9.3 step 11).

### 2.5 Zoom and pan

**E-60 Min scale with a zero-sized view.** Before layout the view is 0×0: `min_scale` gives 0 or NaN. Expected: return 1.0 until the view has a non-zero size; test.

**E-61 Stretch maps to CENTER_CROP.** Pref 2 → `max` of ratios (B§10.2). Test: a 400×200 image in a 200×200 view gives scale `max(0.5, 1.0) = 1.0`, while FIT_SCREEN gives `0.5`.

**E-62 Smart fit.** Tall (`sh > sw`): fit width; otherwise fit height. Test: 300×900 in 600×600 → fit width `2.0`; 900×300 → fit height `2.0`.

**E-63 Fit-to-bounds centring.** Image smaller than view: centred (`tx = d/2`). Image larger: clamp to `[vw - sw, 0]`. Test both (topic §3).

**E-64 Double-tap target.** `zoomIn` when scale ≤ 0.9 × dt or at min; otherwise back to min. Test the boundary at `0.9 × dt`.

**E-65 Double-tap scale capped by max.** `dt = min(max, 2 × fit)`. In the reader `max = 5 × fit`, so the cap never applies; keep the `min` for parity. Test: ORIGINAL_SIZE, `fit = 1`, `dt = 2`, `max = 5` gives `dt = 2`.

**E-66 Landscape zoom only for CENTER_INSIDE.** Other scale types skip it (`ReaderPageImageView.kt:128`). Test.

**E-67 Landscape zoom only at min scale.** If the user already zoomed, `scale != minScale`, skip.

**E-68 Zoom start with null config.** `null` → no change (`ReaderPageImageView.kt:285`). Not reachable in practice (config always set), but the Rust type makes it impossible.

**E-69 Pan remaining when not ready.** `getPanRemaining` returns without writing (zero rect) when the image is not ready (`SubsamplingScaleImageView.java:2026-2028`). Expected: zero, so `canPan` false.

### 2.6 Chapter transition card

**E-70 Downloaded flags with manga null.** `bind` does nothing when `manga == null` (`ReaderTransitionView.kt:31-48`). Expected: card not shown until manga is known.

**E-71 Local manga.** `goingToChapterDownloaded = manga.isLocal() || ...` (`ReaderTransitionView.kt:35`). Test: local manga → downloaded icon on the next chapter.

**E-72 Error message empty.** `transition_pages_error` with `error.message ?: ""` → "Failed to load pages: " (trailing space). Expected: same text.

**E-73 Gap warning count 1.** Plural "one" form: "Skipping 1 chapter, …" (`plurals.xml`). Test.

**E-74 Notice slot placement.** Prev with no previous: notice in the top slot; Next with no next: notice in the bottom slot (`ChapterTransition.kt:103-140`).

### 2.7 Lifecycle

**E-75 setChapters while not idle.** The last pending value wins (`awaitingIdleViewerChapters` is overwritten, `PagerViewer.kt:270`). Test: three updates during settling → only the last is applied on idle.

**E-76 Single-page chapter on idle.** Triggers preload of the next transition destination (`PagerViewer.kt:76-78`).

**E-77 Destroy during decode.** Cancels the decode, no texture is created after destroy (B§12.3). Test with a slow decoder fake.

**E-78 Teardown order.** `destroy()` cancels viewer scope; holders cancel on detach. Expected: no `ViewerEvent` is sent after destroy.

**E-79 LRU eviction of visible pages.** Visible and realised pages are pinned. Test: budget smaller than the window → the window still renders; a warning is logged.

**E-80 Status reset on eviction.** Evicted pages revert to `Queue`, not `Error` (proposal, §5.6 of the design).

**E-81 Theme change in webtoon.** Kotlin recreates the activity (`WebtoonViewer.kt:148-150`). Rust restyles; scroll position and current item preserved. Test.

**E-82 `isScrollingThroughPages` never reset.** Observed (`ReaderActivity.kt:412`). Proposal: reset after the next page selection. Test: after one slider move, a swipe hides the menu.

**E-83 Haptics.** None on desktop. No test beyond "no call to a haptic API" (desktop has none).

**E-84 Open in browser for a non-http URL.** Kotlin shows the button with no click listener (`PagerPageHolder.kt:283-289`). Proposal: hide the button for non-http. Test.

**E-85 Preload request for an already loaded chapter.** The webtoon quirk (B§7.3) requests the chapter of the last page. The view model no-ops (area:43). Test: the viewer still sends the event (parity), the fake view model counts zero loads.

---

## 3. Race conditions

| ID | Race | Mitigation |
|---|---|---|
| R-01 | `setChapters` arrives during a drag | defer until idle (E-75) |
| R-02 | Decode result for a page that was rebound | generation counter per `PageKey` (E-43) |
| R-03 | Split insert added during `setChapters` rebuild | `onPageSplit` posts to main thread; `apply` resets `preprocessed` (B§5.6) |
| R-04 | Preload while the same chapter is being evicted | preload is idempotent in area:43; viewer does not hold chapter refs beyond its window |
| R-05 | Progress updates after READY | `collectLatest` cancels the progress branch (E-44) |
| R-06 | Zoom animation finishing after a new gesture starts | the SSIV double-tap animation is not interruptible (B§10.5); the Rust tween keeps that flag, so a pinch that starts during it is ignored until the tween ends |
| R-07 | Theme change during decode | the decode uses the theme captured in the request; the next refresh re-decodes |
| R-08 | Window resize during a webtoon height change | heights recomputed from the current width; offsets rebuilt |

---

## 4. Fixtures needed

Location: `tests/fixtures/reader/` (create in T-44a-081). All images are generated by a small script committed next to them, so the repository holds no copyrighted pages.

| File | Content | Used by |
|---|---|---|
| `spread_2to1.png` | 200×100, left half red, right half blue | E-11, E-52, E-54 |
| `spread_odd_101.png` | 101×50, columns numbered by colour | E-52 (split width) |
| `tall_strip_1x20000.png` | 1 × 20000 gradient | E-49 |
| `tiny_49x200.png` | 49 × 200 | E-50 |
| `square_white.png` | 300×300 white | E-55 |
| `borders_black.png` | 300×400, black 20 px margin around a grey centre | E-55 crop positive |
| `borders_mixed.png` | black on top row, white on second row | E-56 |
| `bg_dark_edges.png` | black border, white centre | E-58 |
| `bg_white_edges.png` | white border, black centre | E-58 |
| `bg_gradient_top_black.png` | black top half, white bottom half | E-58 gradient |
| `anim.gif` | 3 frames, delays 0 ms, 10 ms, 100 ms | E-46 |
| `anim.webp` | 2 frames | animated WebP |
| `anim.avif` | 2 frames | E-47 |
| `corrupt.jpg` | JPEG truncated at 50 % | E-45 |
| `scale_400x200.png` | 400×200 | E-61, E-62 |
| `scale_300x900.png`, `scale_900x300.png` | tall and wide | E-62 |
| `golden/pager_*.json` | expected item lists for the scenarios in p1-topic-pager-engine.md §8 | T-44a-079 |
| `golden/webtoon_*.json` | expected geometry and current-item sequences | T-44a-079 |
| `golden/nav_*.json` | expected hit-test outcomes for the six layouts × four inversions | T-44a-078 |
| `golden/zoom_pinch.json`, `golden/quick_scale.json` | input and output sequences | T-44a-080 |

---

## 5. Test plan (mapped to tasks)

| Test task | Scope | Kind | Pass criteria |
|---|---|---|---|
| T-44a-078 | navigation tables, inversion, hit test | unit, golden | all `golden/nav_*.json` cases equal; E-15..E-19 |
| T-44a-079 | list builders, gap, transitions, split placement, preload | unit, golden | E-01..E-12, upstream vectors, E-27..E-30 |
| T-44a-080 | zoom math: min scale, fit to bounds, double tap, pinch, quick scale, webtoon clamps | unit, golden | E-31..E-36, E-60..E-69 |
| T-44a-081 | decode: border crop, background, transforms, animated detection, fixtures | unit, golden images | E-46..E-59 |
| T-44a-082 | gesture and keyboard integration on a GTK test window (no display: skipped in CI, run manually) | integration | E-20..E-26 and E-22 |

Coverage target: 100 % of the branches in `nav/`, `split.rs`, `webtoon/scroll.rs`, `zoom/scale.rs`, `zoom/pinch.rs`, `zoom/pan.rs`, `decode/borders.rs`, `decode/background.rs`.

Manual checklist (run before each release of the reader): one L2R manga, one R2L manga, one vertical, one webtoon with 300 pages, one gapped strip, one GIF page, one double-page spread in each split mode, one page with a 404 image (error path with Retry), one chapter boundary with a missing chapter (gap warning visible), theme black/gray/white/automatic.
