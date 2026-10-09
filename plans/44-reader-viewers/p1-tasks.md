# 44-reader-viewers: implementation tasks

Format: see `plans/_tools/tasks.py`. Every feature, setting, edge case and file in the scope maps to at least one task. Source paths are repo-relative to `/home/blackbox/aniyomi-src`. Design references: p1-rust-design.md (RD), p1-behavior.md (B), p1-data-and-config.md (D), p1-ui-parity.md (UI), p1-edge-cases-and-tests.md (E), topics (TP = p1-topic-pager-engine.md, TW = p1-topic-webtoon-engine.md, TZ = p1-topic-zoom-and-image.md).

Shared sources used by many tasks (cited per task below): `V/` = `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/`.

---

## A. Foundations and model

### T-44a-001 — Viewer engine trait, viewer factory and lifecycle
- Priority: P0
- Effort: M
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/Viewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt
- Acceptance:
  - `ViewerEngine` trait exposes `kind`, `widget`, `set_chapters`, `move_to_page`, `handle_key`, `handle_scroll`, `destroy` (RD §3.3, B §0, §6.8).
  - `make_viewer(kind)` maps the five reading modes to viewers exactly as B §0 (flag values 1..5; 0 is rejected with an error).
  - `destroy()` cancels every pending load and stops emitting `ViewerEvent`s (E-78).

### T-44a-002 — Viewer item model: pages, insert pages, transitions and equality
- Priority: P0
- Effort: S
- Depends: T-44a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ChapterTransition.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/InsertPage.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ReaderPage.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ViewerChapters.kt
- Acceptance:
  - `PageRef.number() == index + 1`; `PageKind::Insert` carries `parent_index` (B §5.1, RD §3.2).
  - `ChapterTransition` equality is the unordered pair rule; E-03 passes.
  - `ItemKey` identifies pages and transitions for diffing and `move_to_page`.

### T-44a-003 — Chapter gap calculation
- Priority: P0
- Effort: S
- Depends: area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/MissingChapters.kt, domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt
- Acceptance:
  - `calculate_chapter_gap` returns `floor(higher) - floor(lower) - 1`, 0 for null, unrecognised (`< 0`) numbers (B §5.2).
  - Upstream vectors from domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt pass in Rust (E-04, E-05).

### T-44a-004 — Viewer settings store with the observed defaults
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Every key in D §3 (viewer consumers) exists with the default listed there (for example `pref_double_tap_anim_speed` = 500, `reader_hide_threshold` = LOW).
  - The Rust initial values come from the preference store, not from the Kotlin field initialisers (D §3.1: `usePageTransitions`, `navigateToPan`, `landscapeZoom`).
  - Enum values parse from stored names or ints, with unknown values falling back to the default (B §1.3).

### T-44a-005 — Live settings propagation
- Priority: P0
- Effort: M
- Depends: T-44a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerConfig.kt
- Acceptance:
  - Each setting reaction in B §1.2 is implemented: assign, refresh pages, re-apply navigation, or restyle.
  - A change event is sent only when the value differs (distinctUntilChanged, B §1.1).
  - Change-only reactions (Kotlin `drop(1)`) do not fire on the initial value.

### T-44a-006 — Page and chapter state observation
- Priority: P0
- Effort: M
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ReaderChapter.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt
- Acceptance:
  - `Page.State` values QUEUE, LOAD_PAGE, DOWNLOAD_IMAGE (with progress 0..100), READY, ERROR are observed through `watch` receivers (RD §3.1, B §8.1).
  - Only the latest status branch runs (collectLatest semantics, E-44).
  - Chapter `Wait`/`Loading`/`Error`/`Loaded` is observed for transitions (B §11.3).

### T-44a-007 — In-place refresh of page images when a display setting changes
- Priority: P0
- Effort: S
- Depends: T-44a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonAdapter.kt
- Acceptance:
  - Pager refresh keeps the current index and recreates the visible pages (B §6.7).
  - Webtoon refresh rebinds exactly the indices `[centre-3, centre+3]` (B §7.7 quirk fixed; E-30).

---

## B. Tap zones, gestures and input

### T-44a-008 — Tap zone tables for the six navigation modes
- Priority: P0
- Effort: S
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/LNavigation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/KindlishNavigation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/EdgeNavigation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/RightAndLeftNavigation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/DisabledNavigation.kt
- Acceptance:
  - Region lists equal B §2.3 exactly (values and order).
  - Mode mapping 0 (default per viewer), 1 L, 2 Kindle-ish, 3 Edge, 4 Right and Left, 5 Disabled, other values fall back to the default (D §7).
  - Golden test `golden/nav_*.json` passes (T-44a-078).

### T-44a-009 — Tap zone inversion
- Priority: P0
- Effort: S
- Depends: T-44a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/lang/RectFExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt
- Acceptance:
  - Horizontal, vertical and both inversions transform rectangles as B §2.2 (E-18).
  - Pager and webtoon keep separate inversion settings (`reader_tapping_inverted` vs `reader_tapping_inverted_webtoon`).

### T-44a-010 — Tap hit test and per-viewer action dispatch
- Priority: P0
- Effort: M
- Depends: T-44a-008, T-44a-009
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt
- Acceptance:
  - Hit test is half-open and falls back to MENU (E-16, E-17).
  - Pager: MENU toggles, NEXT/PREV use reading direction, RIGHT/LEFT are absolute (B §2.4).
  - Webtoon: NEXT and RIGHT scroll down, PREV and LEFT scroll up, MENU toggles (B §2.4).
  - Tap coordinates are normalised by the widget's original height for the webtoon (B §2.1).

### T-44a-011 — Single tap, double tap and drag recognition
- Priority: P0
- Effort: M
- Depends: T-44a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/GestureDetectorWithLongTap.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/Pager.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonRecyclerView.kt
- Acceptance:
  - Pager: single tap acts on release; a double tap produces no action (E-20).
  - Webtoon: single tap waits for the double-click window before acting; double tap zooms (T-44a-039).
  - Gesture timings come from GTK settings (`gtk-double-click-time`, `gtk-long-press-time`, `gtk-dnd-drag-threshold`) (RD §11).

### T-44a-012 — Long-tap page actions
- Priority: P1
- Effort: S
- Depends: T-44a-011, area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Long press on a page (not a transition) opens the page actions (B §3.2, §3.3).
  - Gate: `reader_long_tap` or menu visible (E-21).
  - A long press starting within the double-tap window does not start (E-22).

### T-44a-013 — Keyboard paging for the pager
- Priority: P0
- Effort: M
- Depends: T-44a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Key table of B §4.2 is implemented (Left, Right, Up, Down, PageUp, PageDown, Menu, Ctrl+Left, Ctrl+Right).
  - Ctrl variants use direction-normalised moves; plain Left/Right are physical (E-15).
  - Keys are claimed on press and the matching release is swallowed (E-26).

### T-44a-014 — Keyboard scrolling for the webtoon
- Priority: P0
- Effort: S
- Depends: T-44a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt
- Acceptance:
  - Table of B §4.4: Up, Left, PageUp scroll up; Down, Right, PageDown scroll down by three quarters of the viewport.
  - Keys are consumed on both edges.

### T-44a-015 — Volume key paging (optional)
- Priority: P2
- Effort: S
- Depends: T-44a-013, T-44a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt
- Acceptance:
  - Off by default (`reader_volume_keys` = false); when on, XF86AudioRaiseVolume and XF86AudioLowerVolume page through, inverted by `reader_volume_keys_inverted` (B §4.2, §4.4).
  - When off, or when the menu is visible, the keys are not claimed (E-25).

### T-44a-016 — Scroll wheel paging
- Priority: P1
- Effort: S
- Depends: T-44a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt
- Acceptance:
  - Pager: wheel down (GTK dy > 0) = next page; otherwise previous, including dy == 0 (E-24).
  - Webtoon: wheel is handled by the scrolled window and not intercepted (B §4.4).

### T-44a-017 — Haptic feedback on long press (dropped)
- Priority: P3
- Effort: S
- Depends: T-44a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/Pager.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonRecyclerView.kt
- Acceptance:
  - Decision: drop. GTK has no haptic API. The user loses the vibration on long press. The long press itself still works (T-44a-012).
  - No haptic call exists in the Rust code (grep check in review).

### T-44a-018 — Live navigation mode switching
- Priority: P1
- Effort: S
- Depends: T-44a-005, T-44a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonConfig.kt
- Acceptance:
  - Changing `reader_navigation_mode_pager` or `reader_navigation_mode_webtoon` replaces the active table immediately, without recreating the viewer (B §1.2).

### T-44a-019 — Navigation overlay trigger rules
- Priority: P1
- Effort: S
- Depends: T-44a-005, area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt
- Acceptance:
  - On viewer creation, `reader_navigation_overlay_new_user` (default true) forces the overlay once and then sets the pref to false (D §3, B §1.1).
  - The overlay shows on start when `reader_navigation_overlay_on_start` is true or the forced flag is set.

### T-44a-020 — Tap zone overlay visual
- Priority: P1
- Effort: S
- Depends: T-44a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt
- Acceptance:
  - Five region colours are the ARGB constants of B §2.5 and D §1.7 (`0xCC95818D`, `0xCCFF7733`, `0xCC84E296`, `0xCC7D1128`, `0xCCA6CFD5`), with labels Menu, Prev, Next, Left, Right.
  - The overlay (rendered in area:43) reads the constants from `nav::colors`.

### T-44a-085 — Tap zone overlay labels and localisation keys
- Priority: P1
- Effort: S
- Depends: T-44a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt, i18n/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - Labels use the keys action_menu, nav_zone_prev, nav_zone_next, nav_zone_left, nav_zone_right (B §14) with the English text.
  - Missing translations fall back to English.

---

## C. Pager engine

### T-44a-021 — Pager item list builder
- Priority: P0
- Effort: L
- Depends: T-44a-002, T-44a-003, T-44a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewerAdapter.kt
- Acceptance:
  - `build_pager_items` reproduces B §5.3 steps 1-12 (prev tail 2, gated transitions, current pages with preprocessed inserts, next head 2).
  - Golden lists in TP §8 pass (T-44a-079).
  - An empty loaded chapter does not panic (E-01).

### T-44a-022 — Right-to-left reversal and direction mapping
- Priority: P0
- Effort: M
- Depends: T-44a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewers.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewerAdapter.kt
- Acceptance:
  - R2L reverses the list and maps next to decreasing index; L2R and vertical map next to increasing index (B §6.3).
  - Boundaries do not wrap (E-14).

### T-44a-023 — Pager paging widget
- Priority: P0
- Effort: L
- Depends: T-44a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/Pager.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt
- Acceptance:
  - Horizontal and vertical strip; realised window is current ±1 (RD §6.1).
  - Swipe commits at half the extent or on release velocity above the threshold, otherwise snaps back (TP §5).
  - Child widgets (retry, open-in-browser) receive their own clicks.

### T-44a-024 — Pager move commands and transition animation
- Priority: P0
- Effort: M
- Depends: T-44a-023, T-44a-022
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt
- Acceptance:
  - Move animation is smooth when `pref_enable_transitions_key` is true, instant otherwise (B §6.3).
  - `move_to_page` moves with `smooth = use_page_transitions` (B §6.6 proposal, open question 1).
  - Calling move at the boundary is a no-op (E-14).

### T-44a-025 — Deferred chapter application and first layout
- Priority: P0
- Effort: M
- Depends: T-44a-021, T-44a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt
- Acceptance:
  - `set_chapters` while settling is stored and applied once idle; the last value wins (E-75).
  - First layout moves to `min(requested_page, last)` and then shows the pager (E-02, E-57).
  - Single-page chapter on idle triggers the next preload (E-76).

### T-44a-026 — Selection side effects
- Priority: P0
- Effort: M
- Depends: T-44a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Selecting a page emits `PageSelected`, hides the menu unless the slider flag is set, and calls the holder's zoom-start hook (B §6.4, §6.5).
  - `forward` follows B §6.4 step 3, including split pairs (E-11) and transition-to-page (E-12).
  - Slider flag reset is a proposal (E-82, open question 3).

### T-44a-027 — Preload policy
- Priority: P0
- Effort: M
- Depends: T-44a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt
- Acceptance:
  - The last five pages of the current chapter (`size - number < 5`) emit one `PreloadChapter` for the next transition destination when `checkAllowPreload` holds (B §6.4).
  - Insert pages never preload (E-12 neighbour test).
  - A transition with a destination emits `PreloadChapter(to)`.

### T-44a-028 — End-of-book menu rule (pager)
- Priority: P0
- Effort: S
- Depends: T-44a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt
- Acceptance:
  - Selecting a Next transition with no destination emits `ShowMenu`; a Prev transition with no destination does nothing (B §6.4).

### T-44a-029 — Pan wide images before turning the page
- Priority: P1
- Effort: M
- Depends: T-44a-024, T-44a-049
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - With `navigate_pan` on, a move pans the image by one viewport width (250 ms, ease out) when the remaining pan is greater than 1 px, otherwise turns the page (B §10.4, E-13).
  - Pan direction follows the move direction for the reading mode.

### T-44a-030 — Chapter transition gating
- Priority: P0
- Effort: M
- Depends: T-44a-002, T-44a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewerAdapter.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerConfig.kt
- Acceptance:
  - A transition is inserted when there is a gap, the neighbour is not loaded, the neighbour is absent, or `forceTransition` (`always_show_chapter_transition` or the current item being a transition) (B §5.3, §5.4, E-06, E-07).
  - The default `always_show_chapter_transition` is true, so transitions are always present by default.

---

## D. Webtoon engine

### T-44a-031 — Webtoon item list builder
- Priority: P0
- Effort: M
- Depends: T-44a-002, T-44a-003, T-44a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonAdapter.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt
- Acceptance:
  - `build_webtoon_items` reproduces B §5.5 (no insert pages; the same gating as the pager).
  - Keyed diff uses `ItemKey`; content comparison is always "same" (B §5.5).

### T-44a-032 — Webtoon virtual vertical list
- Priority: P0
- Effort: L
- Depends: T-44a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonLayoutManager.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonRecyclerView.kt
- Acceptance:
  - Prefix-sum offsets and a realised range with extra space of `viewport * 3 / 4` on both sides (TW §1, B §7.1).
  - `last_end_visible` implements B §7.3 and returns `None` before layout.
  - `scroll_to(index)` aligns the item top with the viewport top (B §7.5).
  - Placeholder height is the viewport height until decode (B §8.6).

### T-44a-033 — Webtoon geometry: side padding and gap mode
- Priority: P1
- Effort: M
- Depends: T-44a-032, T-44a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonConfig.kt
- Acceptance:
  - Side margin is `width * padding / 100` (integer) on both sides, padding 0..25 (B §8.6, D §3).
  - Gapped mode adds 15 dp below each page; continuous mode adds none (E-38).
  - Margins are computed from the widget width (proposal, UI §6).

### T-44a-034 — Webtoon current page detection and page events
- Priority: P0
- Effort: M
- Depends: T-44a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt
- Acceptance:
  - The current item is the bottom-most realised item whose end is visible, reported when it changes (B §7.3).
  - `PageSelected` is emitted for pages; the preload range and `check_allow_preload` follow B §7.3.

### T-44a-035 — Webtoon preload and end-of-book rules
- Priority: P0
- Effort: S
- Depends: T-44a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt
- Acceptance:
  - Scrolling up onto a Prev transition with a destination emits `PreloadChapter` (B §7.2 step 3).
  - The last visible Next transition without a destination emits `ShowMenu` (B §7.2 step 4, E-29).
  - Quirk of B §7.3 (preload target when the last item is a page) is reproduced, documented in code.

### T-44a-036 — Webtoon menu hide on scroll
- Priority: P0
- Effort: S
- Depends: T-44a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - A scroll with `|dy| > threshold` hides a visible menu (E-27).
  - Threshold is `reader_hide_threshold` (HIGHEST 5, HIGH 13, LOW 31 default, LOWEST 47), read live (proposal, E-28).

### T-44a-037 — Webtoon tap and key scroll
- Priority: P0
- Effort: S
- Depends: T-44a-010, T-44a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt
- Acceptance:
  - Scroll distance is `viewport * 3 / 4` (integer) for taps and keys (B §7.4).
  - Smooth when `pref_enable_transitions_key` is on, instant otherwise.

### T-44a-038 — Webtoon zoom model
- Priority: P0
- Effort: L
- Depends: T-44a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonRecyclerView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonFrame.kt
- Acceptance:
  - Scale clamps to `[0.5, 3]` (or `[1, 3]` when zoom-out is disabled), height becomes `orig / scale` below 1, clamps and centring follow B §10.7.
  - Pinch uses the GTK zoom gesture; a test pinch sequence matches the golden file (T-44a-080).
  - Drag pans in x always and in y only at strip edges (B §10.7).

### T-44a-039 — Webtoon double-tap zoom
- Priority: P0
- Effort: M
- Depends: T-44a-038, T-44a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonRecyclerView.kt
- Acceptance:
  - Double tap toggles between scale 1 and 2 centred on the touch point, 200 ms decelerate, ignored while animating (B §10.7, E-33).
  - Enabled by `pref_enable_double_tap_zoom_webtoon` (default true).
  - Animation values are clamped on every frame (fix of E-34; proposal, labelled in B §10.7).

### T-44a-040 — Webtoon quick scale
- Priority: P1
- Effort: M
- Depends: T-44a-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonRecyclerView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonFrame.kt
- Acceptance:
  - Double tap, hold and drag vertically scales with the formula of TZ §4.2 (20 dp threshold, 0.03 span limit, 0.5 span factor).
  - Enabled only when double-tap zoom is enabled (B §10.7).

### T-44a-041 — Webtoon zoom fling
- Priority: P1
- Effort: S
- Depends: T-44a-038
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonRecyclerView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonFrame.kt
- Acceptance:
  - Fling while zoomed moves x by `0.4 * vx / 2`, and y only at strip edges, over 400 ms decelerate, clamped (B §10.8).
  - No effect at scale 1 (the list scrolls natively).

### T-44a-042 — Webtoon zoom-out disable
- Priority: P1
- Effort: S
- Depends: T-44a-038
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonRecyclerView.kt
- Acceptance:
  - `webtoon_disable_zoom_out` sets the minimum to 1; turning it on while below 1 animates back to 1 over 200 ms (E-32).

### T-44a-043 — Theme change without restarting the reader
- Priority: P1
- Effort: S
- Depends: T-44a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonConfig.kt
- Acceptance:
  - A theme change restyles the viewer in place and keeps scroll position and current item (E-81). The Kotlin activity recreate is not reproduced (RD §11, dropped).

---

## E. Page image widget, zoom and decode

### T-44a-044 — Zoomable page image widget core
- Priority: P0
- Effort: L
- Depends: T-44a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - State is `ZoomState` (scale, centre, min, max, double-tap scale) per TZ §1-§3.
  - Max zoom is 5 × fit, double-tap scale 2 × fit, pan limit INSIDE (TZ §1, §3).
  - `on_scale_changed` hides the menu (B §8.4).

### T-44a-045 — Scale types
- Priority: P0
- Effort: M
- Depends: T-44a-044, T-44a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - Preference values 1..6 map to CENTER_INSIDE, CENTER_CROP (labelled Stretch), FIT_WIDTH, FIT_HEIGHT, ORIGINAL_SIZE, SMART_FIT (B §10.2).
  - Minimum scale formulas of TZ §2.1 are implemented with f32 arithmetic.
  - Tests E-61, E-62 pass.

### T-44a-046 — Zoom start position
- Priority: P0
- Effort: S
- Depends: T-44a-044
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - `pref_zoom_start_key` 1 Automatic resolves to LEFT for L2R, RIGHT for R2L, CENTER for vertical; 2 Left, 3 Right, 4 Center (D §1.8).
  - Applied after ready (TZ §2.5).

### T-44a-047 — Landscape auto zoom
- Priority: P1
- Effort: M
- Depends: T-44a-045, T-44a-046
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - Applies only for landscape pages at minimum scale with CENTER_INSIDE, after a 500 ms delay, animating to fit height in 500 ms ease in-out (B §10.3, E-66, E-67).
  - Direction of the start point follows `forward` (B §10.3).

### T-44a-048 — Pager double tap and pinch zoom
- Priority: P0
- Effort: M
- Depends: T-44a-044
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - Double tap follows TZ §2.4 (zoom in to 2 × fit about the touch point, or back to fit), duration from `pref_double_tap_anim_speed` (E-64, E-65).
  - Pinch follows TZ §4.1 and stays within [fit, 5 × fit].

### T-44a-049 — Tiled rendering and texture size cap
- Priority: P0
- Effort: L
- Depends: T-44a-044
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/GLUtil.kt
- Acceptance:
  - Tiles are at most 2048 px per edge (`pref_hardware_bitmap_threshold`), with a pyramid for large images (TZ §4.3).
  - Only visible tiles are uploaded; tiles off-screen are released.
  - A 1×20000 strip renders without a texture-size error (E-49).

### T-44a-050 — Decode pipeline
- Priority: P0
- Effort: L
- Depends: T-44a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt
- Acceptance:
  - Decode on a worker, transform per `PageTransform`, texture created on the GTK thread (RD §4, §5).
  - Webtoon pages decode at view width (never upsampled) (B §8.4).
  - Decode failure sets the error state (E-42, E-45).
  - Optional `zune-jpeg` fast path behind a benchmark gate (RD §15).

### T-44a-051 — Animated GIF and animated WebP
- Priority: P1
- Effort: M
- Depends: T-44a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt
- Acceptance:
  - GIF and animated WebP play their frames with their delays, looping by the file's loop count (TZ §7).
  - Pager animated pages zoom with levels 1, 2, 5 (B §10.6); webtoon animated pages do not zoom (B §8.4).
  - Frame delay clamped to at least 10 ms (E-46).

### T-44a-052 — Unsupported animated formats fall back to the first frame
- Priority: P2
- Effort: S
- Depends: T-44a-051
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt
- Acceptance:
  - Animated AVIF, HEIF and JXL decode as a still first frame (B §9.5, E-47).
  - No animation is attempted for these formats.

### T-44a-053 — Crop borders
- Priority: P1
- Effort: M
- Depends: T-44a-050, T-44a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonConfig.kt
- Acceptance:
  - `crop_borders` (pager) and `crop_borders_webtoon` (webtoon) enable the border crop of TZ §6, with the exact constants and rules (E-55 to E-57).
  - Golden fixtures pass (T-44a-081).

### T-44a-054 — Automatic background colour
- Priority: P1
- Effort: L
- Depends: T-44a-050
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt
- Acceptance:
  - Used only by the pager, theme 3, for static pages; applied after ready (B §8.2, §9.3).
  - Algorithm of TZ §5 reproduces the Kotlin output on the fixtures in E-58 and E-59 (T-44a-081).
  - Landscape window returns only solid colours.

### T-44a-055 — Dual-page split for the pager
- Priority: P1
- Effort: L
- Depends: T-44a-021, T-44a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewerAdapter.kt
- Acceptance:
  - A wide page (`w > h`) with split on creates an InsertPage after it (L2R, vertical) or at its own index (R2L) (B §5.6).
  - Side choice and invert follow the table of B §9.1 (E-52).
  - Insert placement guards of B §5.6 are implemented with bounds checks (E-08, E-09, E-10).

### T-44a-056 — Dual-page split for the webtoon
- Priority: P1
- Effort: S
- Depends: T-44a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt
- Acceptance:
  - A wide page is stacked vertically into two halves of the right then left (or left then right when inverted) (B §9.2).
  - Output size is `(w/2, 2h)`; no insert pages are created.

### T-44a-057 — Rotate-to-fit for wide pages
- Priority: P1
- Effort: S
- Depends: T-44a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt
- Acceptance:
  - Wide pages rotate by +90, or by -90 when the invert pref is on, both pager and webtoon (B §9.1, E-53).
  - Rotation takes precedence over split (E-54).

### T-44a-058 — Insert page bookkeeping on setting changes
- Priority: P1
- Effort: S
- Depends: T-44a-055
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewerAdapter.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt
- Acceptance:
  - Turning split off removes every InsertPage (B §5.6 `cleanupPageSplit`).
  - Inserts for a neighbour chapter are kept in `preprocessed` and applied when that chapter becomes current (E-08).

### T-44a-059 — Reader background themes
- Priority: P0
- Effort: S
- Depends: T-44a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Theme values 0 White, 1 Black (default), 2 Gray, 3 Automatic map to backgrounds and to the dark/light text flag as in B §1.4.
  - Pages, cards and error states use the same palette.

### T-44a-060 — Maximum tile size setting (advanced)
- Priority: P2
- Effort: S
- Depends: T-44a-049
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - Key `pref_hardware_bitmap_threshold` (default 2048) sets the maximum tile edge; values below 256 are rejected with the default (proposal).
  - Changes apply to pages decoded after the change.

### T-44a-061 — Always decode long strip with SSIV (dropped)
- Priority: P3
- Effort: S
- Depends: T-44a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - Decision: drop the preference `pref_always_decode_long_strip_with_ssiv`. The Rust decoder path is the single path for all viewers (RD §11). Users lose the option to force the tiled decoder for webtoon strips; the webtoon path already decodes at view width.
  - Existing backup values are ignored on import.

---

## F. Loading states, errors and transitions

### T-44a-062 — Page status visuals
- Priority: P0
- Effort: M
- Depends: T-44a-006, T-44a-063
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt
- Acceptance:
  - QUEUE and LOAD_PAGE show the spinner; DOWNLOAD_IMAGE shows the arc with progress; READY hides it; ERROR shows the error card (UI §1.1).
  - A page with no loader stays blank with no error (E-40).

### T-44a-063 — Progress indicator
- Priority: P0
- Effort: S
- Depends: T-44a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderProgressIndicator.kt
- Acceptance:
  - Indeterminate spinner for queued and loading; determinate arc for 0..100 downloading (UI §1.1).
  - Spins continuously so the reader never looks frozen (B §8.1 comment in ReaderProgressIndicator.kt).

### T-44a-064 — Page error card
- Priority: P0
- Effort: S
- Depends: T-44a-062
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderButton.kt, app/src/main/res/layout/reader_error.xml
- Acceptance:
  - Text "The image couldn't be loaded", Retry (calls the loader's retry for the page), and Open in browser (only when the image URL is set).
  - Open in browser uses the default browser for http or https only; non-http hides the button (proposal, E-84).
  - Webtoon card height is 0.8 × viewport (B §8.5).
  - ReaderButton's gesture gate is dropped (RD §11).

### T-44a-065 — Decode failure path
- Priority: P0
- Effort: S
- Depends: T-44a-050, T-44a-064
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt
- Acceptance:
  - Any decode error sets the page to the error state; Retry re-queues the page (E-42).
  - Corrupt or truncated files do not crash the reader (E-45).

### T-44a-066 — Chapter transition card
- Priority: P0
- Effort: M
- Depends: T-44a-002, area:43-reader-core, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderTransitionView.kt, app/src/main/java/eu/kanade/presentation/reader/ChapterTransition.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerTransitionHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonTransitionHolder.kt
- Acceptance:
  - Slot order and texts of B §11.1 and UI §2 (E-74).
  - Downloaded icon for the current chapter from its loader (`isLocal`) and for the destination from the download index (E-70, E-71).
  - Gap warning plural forms (E-73); max width 460 dp.

### T-44a-067 — Adjacent chapter loading and error inside the card
- Priority: P0
- Effort: S
- Depends: T-44a-066, T-44a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerTransitionHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonTransitionHolder.kt
- Acceptance:
  - Loading shows the spinner and "Loading pages…"; error shows "Failed to load pages: {message}" and Retry, which requests the preload again (B §11.3, E-72).

### T-44a-068 — Transition layout metrics
- Priority: P0
- Effort: S
- Depends: T-44a-066
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerTransitionHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonTransitionHolder.kt
- Acceptance:
  - Pager: 64 dp side padding, centred. Webtoon: 128 dp vertical and 32 dp horizontal padding, 16 dp margins around the status container (B §11.4).
  - Scaled by the GTK scale factor.

### T-44a-069 — Memory window, release and recycle
- Priority: P0
- Effort: M
- Depends: T-44a-023, T-44a-032, T-44a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonBaseHolder.kt
- Acceptance:
  - Pages outside the realised window release their widgets and cancel loads; decoded RGBA stays in the byte-budget LRU (RD §5.6).
  - Visible pages are never evicted; evicted pages return to Queue, not Error (E-79, E-80).

### T-44a-070 — Viewer teardown
- Priority: P0
- Effort: S
- Depends: T-44a-001, T-44a-069
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt
- Acceptance:
  - Destroy cancels loads, drops textures and emits nothing afterwards (E-77, E-78).
  - Switching reading mode destroys the previous viewer before the new one loads.

### T-44a-071 — Menu hide on zoom gesture
- Priority: P0
- Effort: S
- Depends: T-44a-044, T-44a-038
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonFrame.kt
- Acceptance:
  - Any scale change on a page hides the menu (B §6.5, §8.4).

---

## G. Platform replacements and drops

### T-44a-072 — Replace the DirectionalViewPager library (dropped)
- Priority: P3
- Effort: S
- Depends: T-44a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/Pager.kt, gradle/libs.versions.toml
- Acceptance:
  - Decision: drop `com.github.tachiyomiorg:DirectionalViewPager:1.0.0` (`gradle/libs.versions.toml:62`). Replacement is `widgets::PagerView` (RD §11). No user-visible loss.

### T-44a-073 — Drop Android view plumbing (dropped)
- Priority: P3
- Effort: S
- Depends: T-44a-023, T-44a-038
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonFrame.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonSubsamplingImageView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonBaseHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/Pager.kt
- Acceptance:
  - Decision: drop the touch-coordinate translation of `WebtoonFrame`, the touch-ignoring SSIV subclass, the holder base class, and the ViewPager exception guards. No user-visible loss; the behaviour they protect is re-created by the widgets (RD §10, B §12.4 item 5).

### T-44a-074 — Reduced motion from the desktop setting
- Priority: P2
- Effort: S
- Depends: T-44a-043
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/AnimationExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - When `gtk-enable-animations` is false, every animation duration in UI §5 becomes one frame; the Android `ANIMATOR_DURATION_SCALE` is replaced (RD §11).

### T-44a-075 — Platform gesture timings from GTK settings
- Priority: P1
- Effort: S
- Depends: T-44a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/GestureDetectorWithLongTap.kt
- Acceptance:
  - Long-press, double-click window and drag threshold come from `gtk-long-press-time`, `gtk-double-click-time` and `gtk-dnd-drag-threshold` (RD §11), so the user's desktop settings apply.

### T-44a-076 — Viewport metrics recomputed on resize
- Priority: P0
- Effort: S
- Depends: T-44a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt
- Acceptance:
  - Scroll distance, placeholder height, error height and progress margins are recomputed on every allocation change instead of once at construction (B §7.1 quirk, E-39).

### T-44a-077 — Automatic background from the desktop colour scheme
- Priority: P1
- Effort: S
- Depends: T-44a-059
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt
- Acceptance:
  - Theme 3 uses the portal `color-scheme` (dark or light) when the app theme is "system", and the app's own dark setting otherwise (B §1.4).

---

## H. Tests and quality

### T-44a-078 — Unit tests: navigation tables and inversion
- Priority: P0
- Effort: S
- Depends: T-44a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/LNavigation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/EdgeNavigation.kt
- Acceptance:
  - All `golden/nav_*.json` cases pass for six layouts times four inversions; E-15 to E-19 covered.

### T-44a-079 — Unit tests: list builders, gap, transitions and split placement
- Priority: P0
- Effort: M
- Depends: T-44a-021, T-44a-031, T-44a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewerAdapter.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonAdapter.kt, domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt
- Acceptance:
  - Upstream gap vectors from MissingItemsTest pass; E-01 to E-12; webtoon scenarios of TW §8 pass.
  - Golden item lists in `tests/fixtures/reader/golden/pager_*.json` and `webtoon_*.json` match exactly.

### T-44a-080 — Unit tests: zoom mathematics
- Priority: P0
- Effort: M
- Depends: T-44a-044, T-44a-038
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonRecyclerView.kt
- Acceptance:
  - Minimum scale per type, fit-to-bounds centring and clamps, double-tap target boundary (0.9 × dt), pinch and quick-scale sequences, webtoon clamps and height at scale < 1 (E-31 to E-36, E-60 to E-69).
  - Quick-scale golden sequence matches `golden/quick_scale.json`.

### T-44a-081 — Golden tests: decode transforms, borders and background
- Priority: P1
- Effort: M
- Depends: T-44a-053, T-44a-054, T-44a-055
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, library/src/main/cpp/borders.cpp
- Acceptance:
  - The fixtures listed in E §4 are generated by the committed script; split width, rotation, crop rectangles and background outputs match the expected values (E-50 to E-59).
  - Border results match the Kotlin decoder output on the same fixtures (captured once from the Android build).

### T-44a-082 — Gesture and keyboard integration checks
- Priority: P1
- Effort: M
- Depends: T-44a-013, T-44a-014, T-44a-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt
- Acceptance:
  - A scripted run (GTK test display or manual checklist in E §5) covers E-20 to E-26 and the manual checklist of E §5.

### T-44a-083 — Logging through tracing
- Priority: P2
- Effort: S
- Depends: T-44a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt
- Acceptance:
  - Each `logcat` call maps to a `tracing` event with the same message at debug level; no output at info level by default.

### T-44a-084 — Keyboard and accessibility basics
- Priority: P2
- Effort: S
- Depends: T-44a-013, T-44a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt
- Acceptance:
  - Every action in UI §4 has a keyboard path; the page image has the accessible label "Page {n} of {total}" (UI §9, proposal).
