# 44-reader-viewers: area overview

Status: area lead summary of the nine detail documents in this folder. Paths are repo-relative to `/home/blackbox/aniyomi-src` unless marked as Kurobayomi paths (`/home/blackbox/kurobayomi`). Labels: **OBSERVED** = what the Kotlin does; **Proposal:** = what the Rust port should do.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here

The viewer package `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/` (31 Kotlin files, 4,152 LOC, verified by summing the LOC column of [p1-inventory.md](p1-inventory.md)) turns a list of manga chapters into something the user can read. It owns:

- **Five reading modes** selected by `ReadingMode` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt:69-79`): Left to Right pager, Right to Left pager, Vertical pager, Webtoon, and Webtoon "Long strip with gaps" (`isContinuous = false`).
- **Tap zones**: six navigation modes (Default, L shaped, Kindle-ish, Edge, Right and Left, Disabled) with inversion and a hit test.
- **Gestures and keys**: tap, double tap, long tap, drag, pinch, fling, keyboard, volume keys and scroll wheel.
- **Item model**: pages, dual-page insert pages, and chapter transition cards with the chapter gap rule.
- **Page image widget**: zoom, pan, scale types, zoom start position, landscape zoom, tiling, animated GIF/WebP, crop borders, dual-page split, rotate-to-fit and automatic background.
- **Page status**: queued, loading, downloading, error, with a progress indicator and an error card with Retry and Open in browser.

Anime does not use these files. Anime playback is area:45-player-core.

### 1.2 Why it matters for a usable Kurobayomi

The reader is the first thing a manga user does. Without a working viewer, Kurobayomi cannot be used daily. Every P0 reading path in the task list depends on this area: pager reading, webtoon reading, tap zones, page states, chapter transitions and the decode pipeline. The current mock (`src/player.rs` `reader()`, lines 49-147, with `spec/reader_player.json`) shows a 420x600 placeholder and fixed thirds for taps. See [p1-ui-parity.md](p1-ui-parity.md) §8 for the full gap list.

### 1.3 Scope boundaries

- In scope: everything under `V/` (`V/` = `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/`), the Rust viewer engine, its widgets, the decode pipeline and the zoom mathematics.
- Owned elsewhere, referenced only: reader chrome, loaders, view model, `ReaderActivity`, reader preferences screen (area:43-reader-core); preference storage (area:02-preferences-system); chapter gap domain function (area:07-domain-entries-items); download state (area:23-downloads-manga); image decoder and cover cache (area:34-images-covers-cache). See §5.

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities

All paths below are under `V/`. LOC values are from [p1-inventory.md](p1-inventory.md).

| Package | Files | LOC | Responsibility |
|---|---|---|---|
| `V/` (root) | 9 | 893 | Shared contract and widgets: `Viewer.kt` (interface), `ViewerConfig.kt` (shared settings), `ViewerNavigation.kt` (tap-zone model), `ReaderPageImageView.kt` (zoomable page image, 442 LOC, the largest file), `ReaderProgressIndicator.kt`, `ReaderTransitionView.kt`, `ReaderButton.kt`, `GestureDetectorWithLongTap.kt`, `MissingChapters.kt` |
| `V/navigation/` | 5 | 142 | Concrete tap-zone layouts: `DisabledNavigation.kt`, `EdgeNavigation.kt`, `KindlishNavigation.kt`, `LNavigation.kt`, `RightAndLeftNavigation.kt` |
| `V/pager/` | 7 | 1,419 | Paged viewer (L2R, R2L, vertical): `Pager.kt`, `PagerConfig.kt`, `PagerPageHolder.kt`, `PagerTransitionHolder.kt`, `PagerViewer.kt` (450 LOC controller), `PagerViewerAdapter.kt` (item list), `PagerViewers.kt` (three direction subclasses) |
| `V/webtoon/` | 10 | 1,698 | Continuous strip viewer: `WebtoonViewer.kt` (controller), `WebtoonAdapter.kt` (item list), `WebtoonRecyclerView.kt` (zoom and gestures), `WebtoonLayoutManager.kt`, `WebtoonPageHolder.kt`, `WebtoonTransitionHolder.kt`, `WebtoonBaseHolder.kt`, `WebtoonConfig.kt`, `WebtoonFrame.kt`, `WebtoonSubsamplingImageView.kt` |

Total: 9 + 5 + 7 + 10 = 31 files; 893 + 142 + 1,419 + 1,698 = 4,152 LOC.

### 2.2 Proposed Rust module map

Crate: the existing Kurobayomi crate `kurobayomi` (Rust 2024 edition, `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, from `/home/blackbox/kurobayomi/Cargo.toml`). All new code lives under `src/reader/`. The layout is defined in [p1-rust-design.md](p1-rust-design.md) §2; the table below maps each Kotlin package to its target.

| Kotlin source | Rust module (proposal) | Kind |
|---|---|---|
| `Viewer.kt` | `reader::viewer` (trait `ViewerEngine`, `ViewerEvent`, `ViewerKind`) | Port |
| `ViewerConfig.kt`, preference wiring in `PagerConfig.kt` and `WebtoonConfig.kt` | `reader::settings`, `reader::viewer::pager::config`, `reader::viewer::webtoon::config` | Port |
| `ViewerNavigation.kt`, `navigation/*.kt`, `RectFExtensions.kt` | `reader::nav` (`mod.rs`, `layouts.rs`, `colors.rs`) | Port |
| `GestureDetectorWithLongTap.kt`, gesture parts of `Pager.kt` and `WebtoonRecyclerView.kt` | `reader::input::gestures`, `reader::input::keys` | Rewrite |
| `MissingChapters.kt` | `reader::model::gap` | Port |
| `PagerViewer.kt`, `PagerViewers.kt` | `reader::viewer::pager` (controller) | Port |
| `PagerViewerAdapter.kt` | `reader::viewer::pager::items` | Port |
| `Pager.kt` | `reader::widgets::pager` (`PagerView`) | Rewrite |
| `PagerPageHolder.kt`, `ReaderPageImageView.kt` | `reader::widgets::pager_page`, `reader::widgets::page_image` | Rewrite |
| `PagerTransitionHolder.kt`, `WebtoonTransitionHolder.kt`, `ReaderTransitionView.kt` | `reader::widgets::transition` | Rewrite |
| `WebtoonViewer.kt` | `reader::viewer::webtoon` (controller) | Port |
| `WebtoonAdapter.kt` | `reader::viewer::webtoon::items` | Port |
| `WebtoonRecyclerView.kt`, `WebtoonLayoutManager.kt` | `reader::widgets::webtoon_view`, `reader::widgets::webtoon::zoom`, `reader::widgets::webtoon::layout` | Rewrite |
| `WebtoonPageHolder.kt` | `reader::widgets::webtoon::page` | Rewrite |
| `ReaderProgressIndicator.kt` | `reader::widgets::status` (GTK `gtk::Spinner` plus a `DrawingArea` arc) | Replace |
| Image transforms (`ImageUtil.kt` as used by the holders) | `reader::decode::transform`, `reader::decode::borders`, `reader::decode::background`, `reader::split` | Port |
| `SubsamplingScaleImageView` and `PhotoView` (external libraries used by `ReaderPageImageView`) | `reader::zoom` (pure math), `reader::widgets::page_image` (GTK snapshot) | Rewrite |
| `ReaderButton.kt`, `WebtoonBaseHolder.kt`, `WebtoonSubsamplingImageView.kt`, `WebtoonFrame.kt` (touch translation) | none (dropped, see [p1-tasks.md](p1-tasks.md) §G) | Drop |

Layering rule (from [p1-rust-design.md](p1-rust-design.md) §1): pure modules (`nav`, `model`, `split`, `zoom`, `decode`, `viewer::*::items`, `viewer::*::controller` logic) hold all decisions and must not import `gtk::`. Widgets only draw and forward input.

---

## 3. Reading guide

Read in this order: inventory, then behaviour, then Rust design, then tasks. The topic documents are summaries of the behaviour and design documents for individual engines.

- [p1-inventory.md](p1-inventory.md): every one of the 31 Kotlin files with LOC, declared names, port decision, target Rust module and the task IDs that cover it. Start here to confirm coverage.
- [p1-behavior.md](p1-behavior.md): the full observed behaviour specification (sections §0 to §15): viewer selection, preference propagation table, tap-zone coordinates and region tables, gestures, key tables, list builders, pager and webtoon engines, the page-holder state machine, image transforms, zoom, background and border algorithms, and open questions (§15).
- [p1-data-and-config.md](p1-data-and-config.md): data models (`Page`, `ReaderPage`, `InsertPage`, `ReaderChapter`, `ChapterTransition`, `ViewerChapters`), enum values, preference keys with defaults, files and formats, network behaviour (none), database touch points (none), and constants that must survive the port.
- [p1-rust-design.md](p1-rust-design.md): crate and module layout, core Rust types and the `ViewerEngine` trait, threading model, decode pipeline, memory budget, pager and webtoon controllers and widgets, zoom model, settings integration, input wiring, Android-to-Linux API mapping, error handling, pitfalls, and the crate decision list.
- [p1-topic-pager-engine.md](p1-topic-pager-engine.md): summary of the paged engine: state, events, invariants, algorithms, widget, holder, preload policy and test plan.
- [p1-topic-webtoon-engine.md](p1-topic-webtoon-engine.md): summary of the webtoon engine: geometry, current-page detection, tap and key handling, strip zoom, transitions, holder behaviour, settings refresh and test plan.
- [p1-topic-zoom-and-image.md](p1-topic-zoom-and-image.md): zoom and pan math as the Kotlin asks SSIV to do it (minimum scale, limits, translation clamping, pinch, quick scale, tiling), plus the port-ready automatic background and border-crop algorithms and open items (§8).
- [p1-ui-parity.md](p1-ui-parity.md): what the user sees and does (page states, error layout, transition cards, menus and overlays, gestures and their desktop equivalents, animations, responsive behaviour, boundary states, the gap list against the current mockup, and accessibility).
- [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge cases `E-nn` with expected results and Kotlin origins, race conditions, fixtures and the test plan mapped to tasks T-44a-078 to T-44a-082.
- [p1-tasks.md](p1-tasks.md): 85 implementation tasks (`T-44a-001` to `T-44a-085`) in sections A to H, each with priority, effort, dependencies, sources and acceptance criteria.

---

## 4. Key design decisions and open questions

### 4.1 Design decisions (recommendations)

| # | Decision | Recommendation | Basis |
|---|---|---|---|
| D1 | Pager widget: third-party paging library or own widget | Own `PagerView` widget (`T-44a-023`). No mature GTK paging library exists. The Android `DirectionalViewPager` is dropped (`T-44a-072`). | [p1-rust-design.md](p1-rust-design.md) §15 "Avoid" list |
| D2 | Where decisions live | Pure Rust logic for every decision; GTK widgets only draw and forward input (`T-44a-001`, `T-44a-021`, `T-44a-031`). | [p1-rust-design.md](p1-rust-design.md) §1 |
| D3 | Webtoon scrolling and zoom | Custom virtual vertical list with a `gtk::Scrollable` vertical adjustment. Zoom is a `translate(x, y) scale(s)` transform applied in `snapshot`; the widget is not scaled. Pointer coordinates are inverted through the transform. | [p1-rust-design.md](p1-rust-design.md) §7.1, `T-44a-032`, `T-44a-038` |
| D4 | Decode formats | `image` 0.25 with features `jpeg`, `png`, `webp`, `gif`, `avif`; `jxl-oxide` for JXL; `libheif-rs` only behind feature `heif` (off by default). Verify that the chosen `image` feature set decodes AVIF before committing (`T-44a-050`). | [p1-rust-design.md](p1-rust-design.md) §15, [p1-data-and-config.md](p1-data-and-config.md) §4 |
| D5 | Decoded page cache | Byte-budgeted LRU of decoded RGBA pages, default 512 MiB, configurable. Evicted pages go back to `Queue`, not `Error`. | [p1-rust-design.md](p1-rust-design.md) §5.6 |
| D6 | Viewer persistence | None. Progress goes upward through `on_page_selected` to area:43-reader-core, which persists it. | [p1-data-and-config.md](p1-data-and-config.md) §6 |
| D7 | Long-strip decode path | Drop the "Always decode long strip with SSIV" option (`T-44a-061`, P3). The Rust decode path is the only path. | [p1-tasks.md](p1-tasks.md) §E |
| D8 | Haptics | Drop haptic feedback on long press (`T-44a-017`, P3). | [p1-tasks.md](p1-tasks.md) §B |
| D9 | Theme change | Change theme in place, no viewer recreate. The Kotlin recreates the activity (`themeChangedListener`). | `T-44a-043` |
| D10 | Volume keys | Optional and off by default (`T-44a-015`, P2). Keyboard and scroll wheel are P0/P1. | [p1-ui-parity.md](p1-ui-parity.md) §8 |

### 4.2 Open questions that need the user's call

These are observed behaviours or ambiguous rules where the Kotlin and the desirable Linux behaviour may differ. Each has a recommendation. The numbering follows [p1-behavior.md](p1-behavior.md) §15, plus the two open items from the zoom topic.

| # | Question | Observed | Recommendation |
|---|---|---|---|
| Q1 | Should pager `moveToPage` scroll smoothly regardless of the transitions setting? | Always smooth (B§6.6). | Follow `usePageTransitions` (`smooth = use_page_transitions`, `T-44a-024`). |
| Q2 | "Stretch" label for scale type | The label "Stretch" maps to `CENTER_CROP` (B§10.2). | Keep `CENTER_CROP` behaviour and rename the help text to "Fill (crop)". Needs the user's decision. |
| Q3 | Reset of `isScrollingThroughPages` | Never reset (B§6.5). | Reset after the next selection (`T-44a-026`). Needs the user's decision. |
| Q4 | Webtoon scroll-hide threshold | Read once at construction (B§7.6). | Live read (low risk). |
| Q5 | Pager double tap | Does nothing (B§3.2). | Keep it a no-op; pinch and double-tap zoom live on the page image (`T-44a-048`). Confirm. |
| Q6 | Webtoon side padding base | Percentage of screen width (`Resources.getSystem().displayMetrics`). Referenced as "§15 open question 6" in [p1-ui-parity.md](p1-ui-parity.md) §6 and [p1-rust-design.md](p1-rust-design.md) §11, but this question is not defined in [p1-behavior.md](p1-behavior.md) §15. This overview defines it here. | Percentage of the widget width (`T-44a-033`). Small visual change. |
| Q7 | Tiling: pyramid or per-tile region decode (zoom topic open item 1) | Not decided. | Pyramid is the portable baseline. Decide after measuring the largest real page in the user's library (`T-44a-049`). |
| Q8 | Crop borders: full resolution or downscale (zoom topic open item 2) | Full resolution by default. | Keep full resolution; decide the downscale optimisation with `T-44a-081` fixtures. |
| Q9 | Background gradient stop placement (zoom topic open item 3) | Four stops at 0, 1/3, 2/3, 1. | Keep; confirm visually against a Kotlin screenshot of the same page (`T-44a-054`). |

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What is needed | Tasks that need it |
|---|---|---|
| area:43-reader-core | Loaders that produce page bytes (`DownloadPageLoader`, `HttpPageLoader`, directory loader); `ReaderPage` state stream; `ReaderChapter`, `ChapterTransition` and `ViewerChapters` models; reader view model that calls the viewer; progress persistence; menu and overlay chrome; reading-mode resolution from `ReadingMode`; the page-number label. | `T-44a-001`, `T-44a-006`, `T-44a-008`, `T-44a-019`, `T-44a-066`, `T-44a-012` |
| area:02-preferences-system | Preference storage and change notification (`Preference<T>.register` equivalent with a watch channel). Settings keys and defaults in [p1-data-and-config.md](p1-data-and-config.md) §3. | `T-44a-004`, `T-44a-005`, `T-44a-045`, `T-44a-053`, `T-44a-033` |
| area:07-domain-entries-items | Domain `calculateChapterGap` semantics (`MissingItems` rules); the Rust function must match the upstream test vectors. | `T-44a-003`, `T-44a-030`, `T-44a-079` |
| area:23-downloads-manga | Download state of a chapter (`currChapterDownloaded`, `goingToChapterDownloaded`) and page states queued and downloading. | `T-44a-066`, `T-44a-062` |
| area:15-source-api-manga | `Page` model and its `State` enum (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt`). | `T-44a-002`, `T-44a-062` |
| area:13-core-util-storage-archive-torrent | Reference only: Kotlin `ImageUtil` functions (`isWideImage` line 121, `splitInHalf` line 129, `splitAndMerge` line 168, `rotateImage` line 150, `chooseBackground` lines 338-580) and `GLUtil.SAFE_TEXTURE_LIMIT = 2048` (line 50). The Rust port is owned by this area under `reader::decode`. | `T-44a-049`, `T-44a-053`, `T-44a-054`, `T-44a-055`, `T-44a-057` |
| area:34-images-covers-cache | Reference only: the Kotlin decoder `TachiyomiImageDecoder.kt` and the native `image-decoder` used for crop borders. Covers may later reuse the Rust decode module (proposal). | `T-44a-050`, `T-44a-053` |
| area:36-ui-shell-navigation-security-webview | Reference only: `ViewPagerAdapter` base, `isVisibleOnScreen`, `animatorDurationScale` (for reduced motion), `createReaderThemeContext`. Opening a URL in the browser (replaces `WebViewActivity`) is a shell service (proposal: `gtk::UriLauncher`). | `T-44a-064`, `T-44a-074`, `T-44a-077` |
| area:49-theme-and-design-system | Reader background colours and tokens for the overlay (proposal; `T-44a-059` and `T-44a-077` define the colour behaviour). | `T-44a-059`, `T-44a-077`, `T-44a-020` |

Reference-only dependencies (area:16-source-api-anime and area:19-extensions-anime) appear only as prose in the behaviour documents. No task depends on them.

### 5.2 What others need from this area

| Consumer | What it needs | Source |
|---|---|---|
| area:43-reader-core | `ViewerEngine` trait and `ViewerEvent` (page selected, chapter changed, menu toggle requests, long tap); `nav::layouts` and `nav::colors` for the tap-zone overlay (`T-44a-020`, `T-44a-085`); the viewer factory `make_viewer(kind)`. | [p1-rust-design.md](p1-rust-design.md) §3.3, §12 |
| area:47-settings-screens-general and area:48-settings-player-and-scripts | Reader setting keys, value spaces and labels, in Kotlin menu order (`SettingsReaderScreen.kt:92-97` for background colour). | [p1-ui-parity.md](p1-ui-parity.md) §8 |
| area:34-images-covers-cache | Proposal: the Rust decode module (`reader::decode`) can serve cover decoding. | this overview §5.1 |

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then the 85 rows of `/home/blackbox/kurobayomi/plans/TASKS.csv` whose `file` starts with `44-reader-viewers/`.

### 6.1 Counts

| Priority | Tasks | Effort S | Effort M | Effort L | Effort XL |
|---|---|---|---|---|---|
| P0 | 51 | 21 | 23 | 7 | 0 |
| P1 | 24 | 14 | 8 | 2 | 0 |
| P2 | 6 | 6 | 0 | 0 | 0 |
| P3 | 4 | 4 | 0 | 0 | 0 |
| **Total** | **85** | **45** | **31** | **9** | **0** |

Rough upper bound for P0 by the convention (S at most 1 day, M at most 3 days, L at most 1.5 weeks): about 21 + 69 + 52 days, so about 142 engineer-days.

Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/44-reader-viewers/p1-tasks.md` prints `ok`. All `area:` dependency tokens in the tasks file use full area IDs that exist (for example `area:43-reader-core`, `area:02-preferences-system`, `area:07-domain-entries-items`, `area:23-downloads-manga`). No task IDs were changed.

### 6.2 P0 critical path

Order of dependency, from the `Depends` fields of [p1-tasks.md](p1-tasks.md):

1. Foundations: `T-44a-001` viewer trait (M, needs area:43-reader-core). `T-44a-002` item model (S) and `T-44a-003` gap (S, area:07-domain-entries-items) in parallel.
2. Settings: `T-44a-004` settings store (M, area:02-preferences-system), then `T-44a-005` live propagation (M), then `T-44a-059` background themes (S).
3. Pager list: `T-44a-030` transition gating (M), then `T-44a-021` pager item list (L), then `T-44a-022` right-to-left mapping (M).
4. Pager widget and moves: `T-44a-023` pager paging widget (L, needs only `T-44a-001`), then `T-44a-024` move commands (M), `T-44a-025` deferred chapter application and first layout (M), `T-44a-026` selection side effects (M), `T-44a-027` preload policy (M), `T-44a-028` end-of-book menu rule (S).
5. Image path: `T-44a-044` zoomable page widget core (L), `T-44a-050` decode pipeline (L, needs `T-44a-006`), `T-44a-062` page status visuals (M), `T-44a-063` progress indicator (S), `T-44a-064` page error card (S), `T-44a-065` decode failure path (S).
6. Input: `T-44a-008` tap tables (S), `T-44a-009` inversion (S), `T-44a-010` hit test and dispatch (M), `T-44a-011` gesture recognition (M), `T-44a-013` keyboard paging (M), `T-44a-014` keyboard scrolling (S).
7. Webtoon: `T-44a-031` item list (M), `T-44a-032` virtual list (L), `T-44a-034` current page detection (M), `T-44a-035` preload and end rules (S), `T-44a-036` menu hide on scroll (S), `T-44a-037` tap and key scroll (S), `T-44a-038` zoom model (L), `T-44a-039` double-tap zoom (M).
8. Transitions and memory: `T-44a-066` transition card (M), `T-44a-067` loading and error inside the card (S), `T-44a-068` layout metrics (S), `T-44a-069` memory window and recycle (M), `T-44a-070` teardown (S).
9. Tests: `T-44a-078` (S), `T-44a-079` (M), `T-44a-080` (M).

Longest serial chain by effort: `T-44a-001` (M) then `T-44a-023` (L) then `T-44a-024` (M), `T-44a-025` (M), `T-44a-026` (M), `T-44a-027` (M). Start `T-44a-023` as soon as `T-44a-001` lands. The image chain `T-44a-001` then `T-44a-044` (L) and `T-44a-050` (L) runs in parallel with it and joins at `T-44a-069`.

### 6.3 Risks

1. **Own pager widget (L, `T-44a-023`).** No mature GTK paging library. Highest schedule risk. Mitigation: keep the controller pure (`T-44a-024`, `T-44a-025`) so the widget can be swapped.
2. **Zoom math ported from SSIV.** The Kotlin relies on `SubsamplingScaleImageView`, which is not in the repository. Behaviour is re-derived from its source at commit `66e0db195d` ([p1-topic-zoom-and-image.md](p1-topic-zoom-and-image.md)). Drift is likely. Mitigation: golden tests `T-44a-080`, `T-44a-081`.
3. **Format coverage.** AVIF, HEIF and JXL decode depend on crate choices (D4). The support matrix is in [p1-data-and-config.md](p1-data-and-config.md) §8. Mitigation: verify AVIF before `T-44a-050` closes.
4. **Memory on long strips.** A 300-page webtoon can exceed the budget. Mitigation: byte-budgeted LRU (D5) and window eviction (`T-44a-069`).
5. **Coupling to area:43-reader-core.** The viewer contract (`T-44a-001`) and the loader state stream (`T-44a-006`) are owned there. Contract churn would stall this area. Mitigation: freeze the `ViewerEngine` and `ViewerEvent` signatures in [p1-rust-design.md](p1-rust-design.md) §3.3 before implementing the widgets.
6. **Race conditions.** [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) §3 lists them (chapter set while a holder is loading, moves during transitions, resize during zoom). Mitigation: cancellation tokens on unbind (`tokio-util`) and the single-owner rule (D2).
7. **Unresolved open questions** (§4.2). Q2 and Q3 change visible behaviour. Mitigation: decide before `T-44a-024` and `T-44a-026`.
8. **Dangling reference.** "§15 open question 6" is cited in two documents but not defined in [p1-behavior.md](p1-behavior.md). This overview defines it as Q6. The behaviour document should be updated.

---

## 7. Definition of done for this area in Kurobayomi

Each item is observable by a user or by a test.

1. Opening a manga chapter in Left to Right mode shows real decoded pages (not a placeholder). Clicking the right third turns to the next page and the left third turns to the previous page. The keys listed in [p1-behavior.md](p1-behavior.md) §4.2 work.
2. Right to Left mode mirrors the tap zones and key actions. Vertical mode uses the L shaped zones by default.
3. Webtoon mode shows one continuous strip with a placeholder of at least one viewport height for undecoded pages. Scrolling, pinch zoom (maximum 3x, `MAX_SCALE_RATE = 3f`), double-tap zoom, quick scale and fling work. Zoom out to 0.5x (`MIN_RATE = 0.5f`) is possible unless "zoom out disabled" is set.
4. Changing reading mode in the reader recreates the viewer (the mode switch is listed as a viewer re-creation in [p1-ui-parity.md](p1-ui-parity.md) §8).
5. Changing navigation mode, inversion, background, scale type, zoom start, crop borders, dual-page split, rotate-to-fit or side padding updates the open reader without reopening the chapter (`T-44a-005`, `T-44a-007`, `T-44a-018`).
6. A chapter boundary shows a transition card with "Previous", "Current", "Finished" and "Next" notices. The card shows loading and error states for the adjacent chapter, and Retry works.
7. A missing chapter between two chapters (per the gap rule) shows the gap notice. The gap function matches the upstream vectors in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) §1.
8. A page that fails to decode shows the error card with Retry. "Open in browser" is shown only for URLs that start with `http` (case-insensitive) and opens the default browser.
9. Dual-page split, rotate-to-fit, crop borders and automatic background work on the fixtures in `tests/fixtures/reader/` (golden tests `T-44a-081`).
10. GIF and animated WebP pages play. Unsupported animated formats show the first frame.
11. Reading progress reaches area:43-reader-core through the page-selected event, and the viewer writes no files and no database rows.
12. Pure modules (`reader::nav`, `reader::model`, `reader::split`, `reader::zoom`, `reader::decode::borders`, `reader::decode::background`, `reader::viewer::*::items`) compile without `gtk::` imports. `cargo test` passes for them, with coverage of every branch in `nav/`, `split.rs`, `webtoon/scroll.rs`, `zoom/scale.rs`, `zoom/pinch.rs`, `zoom/pan.rs`, `decode/borders.rs` and `decode/background.rs` (target stated in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) §5).
13. The manual checklist in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) §5 passes: one Left to Right manga, one Right to Left manga, one vertical, one 300-page webtoon, one gapped strip, one GIF page, a double-page spread in each split mode, one 404 image, and one chapter boundary with a missing chapter.
14. The reader keeps the decoded-page cache under the configured byte budget (default 512 MiB) during a 300-page webtoon read, and evicted pages reload on demand.
