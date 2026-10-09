# Topic: the paged viewer engine (L2R, R2L, vertical)

Covers `pager/Pager.kt`, `pager/PagerViewer.kt`, `pager/PagerViewerAdapter.kt`, `pager/PagerViewers.kt`, `pager/PagerConfig.kt`, `pager/PagerPageHolder.kt`, `pager/PagerTransitionHolder.kt`. Observed rules are in p1-behavior.md §5.3, §5.4, §6. This note adds the state machine, the invariants, and the port-level decisions. Paths are repo-relative to `/home/blackbox/aniyomi-src`.

---

## 1. State

| State | Kotlin location | Meaning | Rust home |
|---|---|---|---|
| `items: MutableList<Any>` | `PagerViewerAdapter.kt:23` | flat list of `ReaderPage` / `ChapterTransition` (+ `InsertPage`) | `PagerController.items` |
| `preprocessed: Map<Int, InsertPage>` | `PagerViewerAdapter.kt:29` | split pages for a non-current chapter, keyed by parent index | `PagerController.preprocessed` |
| `nextTransition` | `PagerViewerAdapter.kt:31` | last computed `Next` transition, always assigned (even when not in the list) | `PagerController.next_transition` |
| `currentChapter` | `PagerViewerAdapter.kt:34` | chapter whose pages are in the list | `PagerController.current_chapter` |
| `currentPage: Any?` | `PagerViewer.kt:57` | the active item (page or transition) | `PagerController.current_page` |
| `isIdle` | `PagerViewer.kt:69` | ViewPager not settling or dragging | `PagerController.is_idle` |
| `awaitingIdleViewerChapters` | `PagerViewer.kt:63` | chapters received while not idle; applied on the next idle | `PagerController.awaiting_idle` |
| `pager.isGone` | Android view state | "first layout not done" | `PagerView::laid_out` flag |

---

## 2. Events and transitions

```
                 set_chapters(c)
  +-------+  ------------------------>  is_idle? ---no---> awaiting_idle = c
  | idle  |                               |yes
  +-------+ <-----------------------------+  apply(c): build_items, reset preprocessed,
      ^                                       first layout -> move_to_page(requested) + show
      | on_scroll_state(IDLE)  (applies awaiting_idle, prefetch if single-page chapter)
      |
  +-----------+  on_scroll_state(SETTLING|DRAGGING) -> is_idle = false
  | settling  |
  +-----------+  on_index_changed(i) (fired by ViewPager when the page settles)
```

Index change handling (`onPageChange`, B§6.4):
1. Resolve `page = items[i]`; no-op when `page == current_page`.
2. Compute `allow_preload` (B§6.4 `checkAllowPreload`).
3. Compute `forward` (B§6.4 step 3).
4. `current_page = page`.
5. Dispatch: `ReaderPage` → `on_reader_page_selected` (hide menu unless scrolling-through flag, report page, holder hook, preload check). `ChapterTransition` → `on_transition_selected` (preload `to`; show menu for terminal `Next`).

`move_to_page(key)`: find index by `ItemKey`; `set_current(i, smooth)`; if the index did not change, call `on_index_changed(i)` manually (B§6.6).

---

## 3. Invariants

1. `items` always contains the current chapter's pages, in order, with `InsertPage`s placed according to §5 of p1-behavior.md.
2. Within a chapter, `items` never contains two `InsertPage`s with the same `parent_index`.
3. The first item is the previous chapter's second-last page, last page, or a `Prev` transition. The last item is a `Next` transition unless the forceTransition rule is off and the next chapter is loaded with no gap (then it is the second page of the next chapter).
4. `next_transition` is always set after `build_items`, even when it is not in `items`. Preload requests use it.
5. R2L: `items` is reversed; `move_next` decreases the index.
6. `current_page` is a key into `items`, never an index, so reordering keeps identity.

---

## 4. Algorithms (port-ready summary; full steps in p1-behavior.md)

`build_pager_items(c, force, preprocessed, kind) -> (Vec<ListItem>, Option<PageKey>)` — B§5.3 steps 1-12:
- Gap flags from `calculate_chapter_gap`.
- Prev tail (2), Prev transition gate, current pages with preprocessed inserts, current chapter, Next transition gate, Next head (2), R2L reverse.
- Returns the list and the insert page to jump to (the `insertPageLastPage` rule: when the last current page has a preprocessed insert, jump to that insert so it is not skipped).

`place_insert(items, current_idx, kind, new_page) -> bool` — B§5.6:
- `place = if kind == RightToLeft { cur } else { cur + 1 }`.
- If `kind == RightToLeft && place >= 1 && items[place - 1] is Insert` → false.
- If `items.get(place)` is `Insert` → false (also false when `place >= len`, which replaces the Kotlin out-of-bounds read; see B§12.4).
- Otherwise insert and return true.

`next_index(kind, idx, len) -> Option<usize>` and `prev_index`: direction mapping of B§6.3, `None` at the bounds (no wrap).

`pan_or_turn(kind_dir, page_holder)`: if `navigate_pan` and the holder can pan in that direction (`remaining > 1`), pan 1 viewport width (250 ms); else turn to the adjacent index with `smooth = use_page_transitions`.

---

## 5. Widget behaviour (`PagerView`)

- Realised window: `current ± 1` (`offscreenPageLimit = 1`).
- Swipe: horizontal (or vertical for `Vertical`). Commit to the neighbour when the drag passes half the extent or the release velocity exceeds the threshold; otherwise snap back. Velocity threshold: Proposal 300 px/s; the Android ViewPager value is platform-defined.
- Tap and long-press are delivered by the viewer widget's gesture controllers (p1-behavior.md §3.2), not by the page widgets. Child buttons (retry, open-in-browser) claim their own events.
- Scroll wheel: `dy > 0` → `move_next`; otherwise `move_prev` (B§4.3 with the GTK sign).
- A page widget that is outside the realised window is unbound, which cancels its load token. Its decoded pixels stay in the LRU.

---

## 6. Holder behaviour (`PagerPageHolder` → `PageView` in Rust)

Status-driven, identical to the webtoon holder except for the image path (B§8.1-8.5):
- QUEUE / LOAD_PAGE / DOWNLOAD_IMAGE → overlay spinner or arc, error card hidden.
- READY → decode request with `PageTransform` from §5 of p1-behavior.md, auto-background when `theme == 3`, crop borders per `crop_borders` pref. Apply texture on the main thread. Call `on_image_loaded` (sets background).
- ERROR, or decode failure → error card: message, Retry (`loader.retry_page`), Open in browser (http only).
- `on_page_selected(forward)` → zoom start / landscape zoom (B§10.3). Only when the page is ready; otherwise the one-shot ready callback performs it.
- Scale change → hide menu (B§6.5 / B§8.4).

Dual-page split request: when a wide non-insert page is decoded and split is on, the holder sends `ViewerEvent::SplitPage { parent }`. The controller creates an `InsertPage` for that parent and calls `place_insert` (B§5.6). The insert page's own decode uses `PageTransform::SplitRight` or `SplitLeft` (B§9.1 table) and does not check wideness.

---

## 7. Preload policy

- Trigger 1: a page in the last five of its chapter (`pages.len() - number < 5`) when the chapter is the current chapter and `allow_preload` → `PreloadChapter(next_transition.to)`.
- Trigger 2: selecting a transition with a destination → `PreloadChapter(to)`.
- Trigger 3: returning from idle with a single-page chapter (B§6.1 isIdle setter) → `PreloadChapter(next.to)`.
- Insert pages never trigger preload (B§6.4 step 4).

Deduplication is not done in the viewer (the activity layer decides; B§13). The Rust `ViewerEvent::PreloadChapter` is sent every time the trigger fires, matching Kotlin.

---

## 8. Test plan (see p1-edge-cases-and-tests.md for IDs)

- Golden item lists: prev/next present; prev missing (`Prev` transition forced); next missing; end of book; `always_show_chapter_transition = false` with loaded neighbours; R2L reversal; preprocessed insert placement; insert cycle guard (R2L and L2R); first-layout clamp.
- Index math: next/prev at both ends; R2L mapping; vertical mapping.
- Preload range: numbers 16..20 of a 20-page chapter trigger; 15 does not.
- Forward flag: split pair (`number` equal, insert is forward); prev-transition-to-page is backward.
