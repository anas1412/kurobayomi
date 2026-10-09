# 43-reader-core: area overview

Area lead document. Every path is repo-relative to `/home/blackbox/aniyomi-src` unless it starts with `/home/blackbox/kurobayomi`. Short names used below: `R/` = `app/src/main/java/eu/kanade/tachiyomi/ui/reader/`, `P/` = `app/src/main/java/eu/kanade/presentation/reader/`.

Status of this document: OBSERVED facts cite the Kotlin source. Lines marked "Proposal:" are Rust decisions. Task files were linted (`p1-tasks.md` and `p2-tasks.md` print `ok`) and the global export reports `errors=0`.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here

The reader is the manga reading surface. It covers:

- Opening a chapter from the library, the details screen, history or updates (`R/ReaderActivity.kt`, `R/ReaderViewModel.kt`).
- Resolving the chapter's page list through one of several loaders: local directory, zip/cbz archive, epub, a downloaded chapter, or an HTTP source (`R/loader/`).
- Presenting pages in a viewer chosen per manga or globally: left-to-right, right-to-left, vertical pager, webtoon, or continuous vertical (`R/setting/ReadingMode.kt`). The viewers themselves belong to area:44-reader-viewers.
- Saving progress, completion, history and read-timer data as the user moves (`R/ReaderViewModel.kt`).
- Downloading the next N chapters while reading and optionally deleting chapters after they are read.
- Reader chrome: top and bottom bars, chapter navigator, page indicator, transition card, page actions (save, share, copy, set cover), settings window.
- Visual overlays: brightness, colour filter, e-ink flash, tap-zone labels.

Scope size: 44 Kotlin files, 6,210 lines (verified against the source for every file path and line count).

- Part 1 (`p1-*`): 32 files, 4,939 lines. Models, loaders, `ReaderViewModel`, `ReaderActivity`, presentation.
- Part 2 (`p2-*`): 12 files, 1,271 lines. Preferences, reading mode enum, viewer layer (page image view, tap navigation, progress indicator, transition view, flash).

### 1.2 Why this matters for a usable Kurobayomi

The reader is the first end-to-end P0 path. Without it there is no daily manga reading. The reader also owns the progress and history writes that the library, history, updates and tracking areas depend on. A mistake here (a lost progress write, a wrong chapter order, a stale page after a chapter switch) is visible to the user immediately and damages their reading state, so the port has to be exact on ordering and on write timing.

### 1.3 Out of scope for this area

- The viewer layouts, pager and webtoon page holders, and navigation subclasses: area:44-reader-viewers.
- Anime player: area:45-player-core and area:46-player-controls-ui.
- Preference store implementation and the settings screens: area:02-preferences-system and area:47-settings-screens-general. This area defines the reader keys and their semantics only.
- Source API calls (`HttpSource`), download manager internals, image saver internals: owned by area:15-source-api-manga, area:23-downloads-manga, area:25-downloads-shared, area:13-core-util-storage-archive-torrent. This area calls them.

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities

| Package (repo-relative) | Files in scope | Responsibility |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/ui/reader/` | `ReaderActivity.kt` (974 LOC), `ReaderViewModel.kt` (997), `ReaderNavigationOverlayView.kt` (116), `SaveImageNotifier.kt` (96) | Activity host and lifecycle, state machine and writes, tap-zone label overlay, save-image notification |
| `.../ui/reader/loader/` | `ChapterLoader.kt`, `PageLoader.kt`, `DirectoryPageLoader.kt`, `ArchivePageLoader.kt`, `EpubPageLoader.kt`, `DownloadPageLoader.kt`, `HttpPageLoader.kt` (7 files) | Loader selection, page list and page image production per source type |
| `.../ui/reader/model/` | `ReaderChapter.kt`, `ReaderPage.kt`, `InsertPage.kt`, `ViewerChapters.kt`, `ChapterTransition.kt` (5 files) | Chapter and page model, chapter reference set, transition value |
| `.../ui/reader/setting/` | `ReaderOrientation.kt` (part 1); `ReaderPreferences.kt`, `ReaderSettingsScreenModel.kt`, `ReadingMode.kt` (part 2) | Preference keys and typed accessors, enums with stored int values |
| `.../ui/reader/viewer/` | `Viewer.kt`, `ViewerConfig.kt`, `ViewerNavigation.kt`, `GestureDetectorWithLongTap.kt`, `MissingChapters.kt`, `ReaderButton.kt`, `ReaderPageImageView.kt`, `ReaderProgressIndicator.kt`, `ReaderTransitionView.kt` (9 files, part 2) | Viewer contract, per-viewer configuration, tap hit testing, long press, page image view with zoom, spinner, transition host, gap calculation |
| `app/src/main/java/eu/kanade/presentation/reader/` | 15 files including `ChapterTransition.kt`, `DisplayRefreshHost.kt`, `ReaderPageActionsDialog.kt`, `ReadingModeSelectDialog.kt`, `OrientationSelectDialog.kt`, `PageIndicatorText.kt`, `ReaderContentOverlay.kt`; subfolders `appbars/`, `components/`, `settings/` | Compose presentation of the chrome, sheets, dialogs and the settings window |

Rows and file counts come from [p1-inventory.md](p1-inventory.md) (32 files, one row each) and [p2-inventory.md](p2-inventory.md) (12 files, one row each). Port decisions per file are in those inventories.

Outside the scope but read for exact behaviour: `domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt` (chapter sort), `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/` (layouts, owned by area:44-reader-viewers), `core/archive/src/main/kotlin/mihon/core/archive/EpubReader.kt` (epub spine), and the upstream tests `domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt` and `domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt` (ported in T-43a-021 and the test tasks).

### 2.2 Rust module map (merged)

Both part authors proposed a `src/reader/` tree. The two lists collide in four places (`progress.rs`, `settings.rs` vs `prefs.rs`, `image.rs` vs `page_decode.rs`, `sheets.rs` vs `settings_model.rs`). The merged map below is the proposal the implementer should use. Each collision is resolved and marked.

Proposal: one tree under `/home/blackbox/kurobayomi/src/reader/`, added as `mod reader;` in `src/main.rs` next to `mod player;` (see [p1-rust-design.md](p1-rust-design.md) section 1).

```
src/reader/
  mod.rs                 ReaderSession, open(), ReaderView factory, ui::push wiring        (p1 mod.rs + p2 mod.rs)
  model.rs               ChapterKey, ReaderChapter, ReaderPage, PageStatus, ChapterState,
                         ViewerChapters, ChapterTransition, InsertPage                    (p1)
  prefs.rs               ReaderPrefs: typed getters/setters, keys, defaults, Pref<T> handle (p2)
                         RESOLVED: p1 "settings.rs" (ReaderSettings) is folded into prefs.rs
  enums.rs               FlashColor, TappingInvertMode, ReaderHideThreshold, ZoomStart,
                         ImageScaleType, ColorFilterMode, TapZoneLayout, labels            (p2)
  mode.rs                ReadingMode (flag values, MASK, direction, viewer kind), resolve  (p2)
  orientation.rs         ReaderOrientation flag values and mask helpers                    (p1)
  chapters.rs            list build: skip-read, filter, dedupe, downloaded-only; sort comparators (p1)
  loader/mod.rs          enum Loader (enum dispatch), ChapterLoader::load_chapter, choose_loader (p1)
  loader/dir.rs          DirectoryLoader (natural sort, image filter)                      (p1)
  loader/archive.rs      ArchiveLoader (zip/cbz; rar behind feature), entry index built once (p1)
  loader/epub.rs         EpubLoader (container.xml -> OPF -> spine -> xhtml img list)      (p1)
  loader/download.rs     DownloadLoader (dir or .cbz from the download store)             (p1)
  loader/http.rs         HttpLoader: priority queue, single worker, preload 4, retry      (p1)
  loader/queue.rs        PriorityQueue with seq tie-break and cancellation tokens         (p1)
  image.rs               ImageType detection, decode to RGBA off-thread, downscale        (p1)
  page_decode.rs         decision logic only: tiling/size threshold, crop borders, dual-page split math (p2)
  cache.rs               page-list + image cache (SQLite index + files, 100 MiB LRU)      (p1)
  progress_writer.rs     DB actor: last_page_read, read, bookmark, history                (p1)
                         RESOLVED: p1 "progress.rs" is renamed; the name progress.rs is
                         taken by p2 for the loading indicator
  loading.rs             LoadingIndicator widget (spinner + clamped percentage)           (p2)
                         RESOLVED: p2 "progress.rs" is renamed to loading.rs
  nav.rs                 NavRegion (MENU, PREV, NEXT, LEFT, RIGHT), invert(), action_at() (p2)
  gesture.rs             LongPressTracker (pure state machine, no GTK types)             (p2)
  chapter_gap.rs         calculate_chapter_gap(higher, lower) -> i32                      (p2)
  viewer.rs              trait ReaderViewer (the Viewer contract)                         (p2)
  viewer_config.rs       ViewerConfig: fields and pref subscriptions                      (p2)
  page_zoom.rs           ZoomModel: fit scale per ImageScaleType, min/max, zoom anchors, landscape auto-zoom, pan math (p2)
  page_view.rs           PageView (glib subclass of gtk::Widget): snapshot, gestures, load state (p2)
  overlay_button.rs      ReaderButton equivalent (gtk::Button, no pager gesture coupling) (p2)
  flash.rs               full-window flash state machine + drawing                        (p1 + p2 merged)
  overlay.rs             tap-zone label overlay, brightness and colour layer drawing      (p1)
  chrome.rs              top bar, bottom bar, chapter navigator, page indicator           (p1)
  sheets.rs              reading-mode / orientation / page-actions sheets, settings window shell (p1)
  settings_model.rs      ReaderSettingsModel: viewer kind + manga flags + glib signals    (p2)
                         RESOLVED: p1 sheets.rs owns windows; settings_model.rs owns model state only
  transition.rs          ChapterTransitionView widget (binds the transition model)         (p1 + p2 merged)
  actions.rs             save image, open/share, copy, set as cover (no Android APIs)      (p1)
  notify.rs              gio::Notification wrappers (saved image)                          (p1)
  strings.rs             English strings keyed by Aniyomi message keys                     (p1)
  error.rs               LoadError (thiserror), user_message() -> key                      (p1)
  tests/                 unit tests (see section 4 and p1-edge-cases-and-tests.md section 10)
```

Rule kept from part 2: pure logic lives in modules without `gtk` imports (`prefs`, `enums`, `mode`, `orientation`, `nav`, `gesture`, `chapter_gap`, `page_zoom`, the decision parts of `page_decode`, `chapters`, `cache` index, `progress_writer` logic). GTK code is in `page_view`, `loading`, `transition`, `overlay_button`, `flash` drawing, `sheets`, `chrome`, `settings_model` glue.

Proposal: the existing mockup function `src/player.rs:49-149` (`reader()`) is replaced by `reader::open(...)`. The spec-driven settings sheet `spec/reader_player.json` keeps its labels but its items read and write `ReaderPrefs` (task T-43b-031).

### 2.3 Threading model (merged)

- GTK main thread owns `ReaderSession` and every widget. Single owner, no shared mutable state across threads.
- One tokio runtime (multi-thread, 2 workers, started once in `main`; [p1-rust-design.md](p1-rust-design.md) section 2) runs HTTP page fetches, the loader queue worker and the DB writer actor.
- Image decode runs off the GTK thread and returns RGBA8 to the main thread through a glib channel. Stale results are discarded by a decode session guard (T-43b-059).
- Events from the session to viewers flow over an unbounded ordered channel (T-43a-086).

---

## 3. Reading guide

Read in this order: this file, then `p1-behavior.md` and `p1-rust-design.md` for the core, then the topic files when you implement the matching part.

| Document | What it covers |
|---|---|
| [00-overview.md](00-overview.md) | This file: scope, module map, decisions, dependencies, task summary, definition of done. |
| [p1-inventory.md](p1-inventory.md) | The 32 part-1 files: LOC, declarations, purpose, port decision and target Rust module for each. |
| [p1-behavior.md](p1-behavior.md) | Part-1 behaviour spec (23 sections): chapter model and state machine, loader selection, page loaders, ReaderViewModel state, chapter ordering, loading, progress, download-ahead, reading mode, page actions, dialogs, ReaderActivity lifecycle, chrome, overlays, brightness and colour math, message keys, timings and limits, concurrency. |
| [p1-data-and-config.md](p1-data-and-config.md) | Part-1 data: in-memory models, domain models, preference keys with defaults and types, files on disk, network endpoints, database tables and columns, bit layouts, enum stored values, constants table, equality and serialisation. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Part-1 edge cases with IDs (E-...), failure modes, races, platform risks, and the test plan. Includes the open question E-TRACK-1. |
| [p1-rust-design.md](p1-rust-design.md) | Part-1 Rust design: crate and module layout, threading, key Rust types and signatures, settings view, session, widgets, error handling, Android-to-Linux mapping, flash and brightness overlays, strings, tests, pitfalls, dropped items, implementer checklist. |
| [p1-tasks.md](p1-tasks.md) | Part-1 implementation tasks T-43a-NNN (94 tasks). |
| [p1-topic-chapter-loading.md](p1-topic-chapter-loading.md) | Deep dive: who calls the loader, chapter state timeline, per-page status table, dequeue test vectors, preload window, archive and directory behaviour, selection precedence, cache keys. |
| [p1-topic-progress-and-completion.md](p1-topic-progress-and-completion.md) | Deep dive: event timeline for one page change, completion on last page, write matrix, read timer and history, delete-after-read, download-ahead trigger, incognito matrix, ordering invariants. |
| [p1-ui-parity.md](p1-ui-parity.md) | Part-1 UI parity: reader screen states, app bars, navigator and bottom bar, sheets and dialogs, page indicator, chapter transition card, navigation overlay, colours, gestures and keyboard, timings, toasts, mockup gap list G-01 to G-35, accessibility, responsive behaviour, dismissal rules. |
| [p2-inventory.md](p2-inventory.md) | The 12 part-2 files: LOC, declarations, purpose, port decision, target Rust module, task IDs. Includes the caller map. |
| [p2-behavior.md](p2-behavior.md) | Part-2 behaviour spec (16 sections): preference store contract, ReaderPreferences key groups, enum behaviours, reading mode, settings model, ViewerConfig, tap navigation, long press, page image view, progress indicator, transition view and chapter gap, button, Viewer interface, e-ink flash, message keys, side effects and limits. |
| [p2-data-and-config.md](p2-data-and-config.md) | Part-2 data: model fields, complete preference catalog with defaults and consumers, enum stored values and bit layouts, storage, endpoints, tables, and the Linux storage plan (Proposal). |
| [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md) | Part-2 upstream tests to port, test IDs E-01 to E-43, edge cases by file, failure modes, races, decisions D-1 to D-11, fixtures, test layout. |
| [p2-rust-design.md](p2-rust-design.md) | Part-2 Rust design: mockup state, module layout, core types and trait signatures, threading, settings store, page view and zoom model, decoding, navigation and input, overlays, crates, error handling, Android-to-Linux mapping, pitfalls, mockup integration points. |
| [p2-tasks.md](p2-tasks.md) | Part-2 implementation tasks T-43b-NNN (59 tasks). |
| [p2-topic-page-image-view.md](p2-topic-page-image-view.md) | Deep dive on `ReaderPageImageView`: load paths, SubsamplingScaleImageView configuration, zoom start and landscape auto-zoom, PhotoView child for animated images, decode and tiling decision, callbacks, constants, Rust mapping. |
| [p2-topic-tap-navigation.md](p2-topic-tap-navigation.md) | Deep dive on the tap model: regions, inversion, hit test, overlay colours, long press, why "Menu" is the fallback, dropped or changed items. |
| [p2-topic-flash-and-overlays.md](p2-topic-flash-and-overlays.md) | Deep dive on the e-ink flash, loading indicator, transition host, chapter gap, retry button, and their constants. |
| [p2-ui-parity.md](p2-ui-parity.md) | Part-2 UI parity: page image view states, loading indicator, transition panel and retry, flash overlay, settings controls bound to ReaderPreferences, responsive behaviour, keyboard and mouse, mockup gap list. |

---

## 4. Key design decisions and open questions

Each row is a decision the user (or the area lead, where marked) must confirm. The recommendation is what the task files already assume.

### 4.1 Architecture decisions (recommended; already reflected in the tasks)

| ID | Decision | Recommendation and reason |
|---|---|---|
| A-1 | Loader dispatch | Enum dispatch (`enum Loader`), not trait objects. Five variants, no plugin point needed. Source: [p1-rust-design.md](p1-rust-design.md) section 3. |
| A-2 | Preference key names | Keep every Aniyomi key string byte-identical (for example `pref_reader_flash_duration`, `pref_reader_flash_interval`) so backup preference entries map one-to-one. Reason: backup compatibility with area:21-backup-format-and-create and area:22-backup-restore. |
| A-3 | Pure logic separated from GTK | Yes. Gives `cargo test` coverage for ordering, gaps, flags, zoom and long press without a display. |
| A-4 | Progress and history writes | One DB writer actor (`progress_writer.rs`) fed by an ordered channel. Keeps writes ordered and off the GTK thread. |
| A-5 | Decode | Off-thread decode to RGBA8, downscale to a texture limit, stale results dropped by session guard (T-43b-059). |
| A-6 | Chapter page-list and image cache | SQLite index plus files, 100 MiB LRU (`cache.rs`). Matches the Kotlin cache size. |
| A-7 | Overlays | Brightness and colour filter are software layers over the page area only, never over the chrome. Matches `R/ReaderActivity.kt` and `P/ReaderContentOverlay.kt`. |

### 4.2 Open questions for the user

| ID | Question | Recommended default (source) |
|---|---|---|
| D-1 | Scale type stored value 2 is "Stretch" in the UI but maps to `SCALE_TYPE_CENTER_CROP` (crop-to-fill). Keep the behaviour or implement a real non-uniform stretch? | Keep crop-to-fill for backup compatibility; relabel "Fill (crop)"; keep stored value 2 ([p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md) section 6, D-1). |
| D-2 | Invalid stored scale-type value: throw (Kotlin) or fall back? | Fall back to the Fit screen, never crash (D-2). |
| D-3 | Landscape auto-zoom after the user already zoomed within 500 ms | Cancel the pending auto-zoom on any user zoom or pan (improvement over Kotlin) (D-3). |
| D-4 | Global default is itself `DEFAULT` (corrupt preference) | Use `RIGHT_TO_LEFT`, the Kotlin constant default (D-4). |
| D-5 | Progress above 100 | Clamp to 100 (D-5). |
| D-6 | Display cutout setting on Linux | Hidden; no Linux equivalent (drop, P3). Task T-43a-069 (D-6). |
| D-7 | Stored webtoon side padding outside 0..25 | Clamp on read (D-7). |
| D-8 | Negative chapter gaps on the details screen | Hide, same as the reader (D-8). |
| D-9 | Maximum texture size before downscale | 16384 px on either side; verify at runtime with the GL context where available (D-9). |
| D-10 | Long press during an animation | Ignore while an animation runs (D-10). |
| D-11 | Re-apply scale type live to the current page | Yes (D-11). |
| Q-1 | Track auto-update on mark-as-read: Kotlin checks only `autoUpdateTrack`, not `autoUpdateTrackOnMarkRead` (E-TRACK-1, [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 5) | Owner is area:27-tracking-core. Until it decides, mirror the Kotlin behaviour (check only `autoUpdateTrack`). The user should confirm the intended behaviour before T-43a-030 (tracker auto-update on completion) closes. |
| Q-2 | rar (`.cbr`) support through libunrar (native, licensing) | Feature flag `rar`, off by default ([p1-rust-design.md](p1-rust-design.md) section 16). |
| Q-3 | Screen orientation lock and auto-rotate (`ReaderOrientation`) | Drop (P3). Values are stored and synced, not applied. The user loses per-manga rotation (section 4.4). |
| Q-4 | System share sheet | Replace (P2): open the file or show it in the file manager. The user loses sharing to arbitrary apps. |
| Q-5 | Hardware brightness above 0 | Partial: sysfs backlight if writable. On laptops without a writable backlight, values above 0 do nothing. |
| Q-6 | Owner of `DataSaver` (`app/src/main/java/aniyomi/util/DataSaver.kt`) | Not decided by this area. Closest candidate area:12-network-stack. Confirm with that area lead. |
| Q-7 | Test-gating sequence: T-43a-040 (top bar) depends on T-43a-084 (HTTP loader tests) | Recommend removing the test dependency so the chrome is not blocked by test work. Confirm before implementation. |

Dependency cycles found in this pass were removed from the task files; see section 6.4.

### 4.3 Priority inversions (P0 task depends on P1 or P2 task)

These need a decision: either promote the dependency to P0 or demote the dependent. The area lead recommends promotion, since each P0 task needs them to run.

| P0 or P1 task | Depends on | Dependency priority |
|---|---|---|
| T-43a-014 (HTTP page resolution) | T-43a-017 | P1 |
| T-43a-018 (chapter list) | T-43a-020 | P1 |
| T-43a-025 (chapter switch) | T-43a-033 | P1 |
| T-43a-026 (adjacent chapter nav) | T-43a-053 | P1 |
| T-43a-078 (mockup gap closure) | T-43a-035, T-43a-059 | P1 |
| T-43a-082 (pure-logic tests) | T-43a-020 | P1 |
| T-43a-083 (loader tests) | T-43a-010 | P1 |
| T-43a-045 (page actions sheet, P1) | T-43a-056 | P2 |
| T-43a-048 (custom filter tab, P1) | T-43a-065 | P2 |

### 4.4 Dropped and degraded items (user-visible loss)

From [p1-rust-design.md](p1-rust-design.md) section 15 and [p2-topic-tap-navigation.md](p2-topic-tap-navigation.md) section 7:

- Per-manga screen rotation lock: no effect on desktop (Q-3).
- Display cutout mode: nothing on desktop (D-6).
- Haptics on the slider: nothing.
- Volume keys to turn pages: use arrow keys, Page Up/Down, mouse wheel (P3).
- System share sheet: replaced by open or show in file manager (Q-4).
- Hardware brightness above 0: partial (Q-5).
- Big-picture notification expansion: thumbnail shows as the icon only on most shells.
- ICC display profile: no colour management until implemented (P2).
- Activity transition animations: stack slide used instead (P3).

---

## 5. Cross-area dependencies

Source: the `Depends:` lines of the 153 area tasks, checked against [TASKS.csv](../TASKS.csv). Prose mentions of other areas without a `Depends:` edge are listed separately so the lead can decide whether they need one.

### 5.1 What this area needs from others (Depends edges)

| Area | Tasks that depend on it (Depends edge) |
|---|---|
| area:02-preferences-system | T-43a-081 (reader settings store), T-43b-001 (preference store) |
| area:05-data-repos-entries-items | T-43b-011 (per-series reading mode and orientation in manga viewer flags) |
| area:07-domain-entries-items | T-43b-027 (`calculate_chapter_gap` with upstream tests) |
| area:09-domain-source-track-download-release-misc | T-43a-032 (delete-after-read queue) |
| area:21-backup-format-and-create, area:22-backup-restore | T-43b-055 (backup and restore of reader preferences) |
| area:23-downloads-manga | T-43a-011 (downloaded chapter loader), T-43a-034 (download-ahead), T-43b-032 (transition downloaded flags), T-43b-056 (create folder per manga) |
| area:25-downloads-shared | T-43a-011 (downloaded chapter loader), T-43a-034 (download-ahead) |
| area:27-tracking-core | T-43a-030 (tracker auto-update on completion) |
| area:34-images-covers-cache | T-43a-058 (set page as manga cover), T-43b-020 (crop borders for paged and webtoon) |
| area:36-ui-shell-navigation-security-webview | T-43a-059 (open chapter in browser, WebView or similar) |
| area:38-ui-entry-details-manga | T-43a-060 (open the manga from the reader title) |
| area:44-reader-viewers | T-43a-087 (viewer contract used by the reader core) |
| area:47-settings-screens-general | T-43b-031 (reader settings sheet bound to ReaderPrefs) |
| area:51-i18n | T-43b-057 (localisation keys for scope strings) |

### 5.2 Referenced in prose only (no Depends edge; verify before implementation)

- area:12-network-stack: HTTP client, toast and share helpers (location not confirmed by the source docs). Owner of `DataSaver` is open (Q-6 in section 4.2).
- area:13-core-util-storage-archive-torrent: archive and epub readers, `ImageUtil`, `DiskUtil`, natural sort, image saver. Loader tasks (T-43a-009, T-43a-010, T-43a-012) need these in practice; add Depends edges if they are required.
- area:15-source-api-manga, area:16-source-api-anime, area:17-source-app-layer: `HttpSource` page list and image calls, `MangaSourceManager.getOrStub`. The HTTP loader tasks (T-43a-013, T-43a-014) need these.
- area:08-domain-library-category-history-updates: history upsert semantics (`UpsertMangaHistory`, one row per chapter).
- area:52-util-app: utility helpers used by the reader.

### 5.3 What others need from this area

| Dependent | Tasks that depend on area:43-reader-core |
|---|---|
| area:44-reader-viewers | T-44a-001, T-44a-006, T-44a-008, T-44a-012, T-44a-019, T-44a-066 |
| area:47-settings-screens-general | T-47a-031, T-47a-034, T-47a-035, T-47a-036, T-47a-080, T-47a-081 |
| area:15-source-api-manga | T-15a-027 |
| area:23-downloads-manga | T-23a-028, T-23a-031 |
| area:25-downloads-shared | T-25a-036 |
| area:35-data-misc-updater-notification-export | T-35a-014 |
| area:42-ui-updates-history-stats-storage-upcoming | T-42a-016 |
| area:46-player-controls-ui | T-46b-021 |
| area:52-util-app | T-52a-031, T-52a-041 |
| area:53-platform-manifest-intents-workers | T-53a-015, T-53a-057, T-53a-065 |
| area:01-foundation-app-bootstrap | T-01a-022 |
| area:49b-android-res-values-drawables | T-49ba-036 |

### 5.4 Self-references to fix

Twelve tasks in this area list `area:43-reader-core` in their own `Depends:`. It lint-passes, but it gives no ordering. Each needs a concrete `T-43a-NNN` or `T-43b-NNN` instead. Left unchanged in this pass because the right target is a per-task judgement call: T-43b-008, T-43b-009, T-43b-012, T-43b-028, T-43b-029, T-43b-030, T-43b-034, T-43b-037, T-43b-050, T-43b-052, T-43b-053, T-43b-059. Example: T-43b-009 (ReadingMode enum) probably means T-43a-004 (reading-mode and orientation flags).

---

## 6. Task summary

### 6.1 Counts (153 tasks; source: [TASKS.csv](../TASKS.csv) rows whose file starts with `43-reader-core/`)

| File | Tasks | P0 | P1 | P2 | P3 | S | M | L | XL |
|---|---|---|---|---|---|---|---|---|---|
| [p1-tasks.md](p1-tasks.md) | 94 | 55 | 28 | 5 | 6 | 50 | 41 | 3 | 0 |
| [p2-tasks.md](p2-tasks.md) | 59 | 31 | 21 | 4 | 3 | 40 | 16 | 3 | 0 |
| Total | 153 | 86 | 49 | 9 | 9 | 90 | 57 | 6 | 0 |

Every task has Priority, Effort, Depends, Sources, and at least one acceptance criterion; `tasks.py lint` passes on both files.

### 6.2 P0 critical path

The longest chain of P0 tasks, in order:

1. T-43a-001 Reader domain model (M)
2. T-43a-005 Page loader abstraction (M)
3. T-43a-013 HTTP page loader: priority queue and single worker (L)
4. T-43a-014 HTTP page loader: page resolution, image fetch and status (M)
5. T-43a-015 HTTP page loader: retry and eviction re-queue (S)
6. T-43a-084 HTTP loader tests with a scripted mock source (M)
7. T-43a-040 Reader top bar (M)
8. T-43a-024 Initial load error card with Retry (S)
9. T-43a-074 Downloaded-only blocked-chapter message and retry (S)

Parallel P0 streams that join the chain:

- Settings: T-43a-081 (after area:02-preferences-system) feeds T-43a-046 (reading-mode tab, L) and T-43a-080 (change propagation).
- Viewer path: T-43b-001 (preference store) to T-43b-035 (ViewerConfig), T-43b-014 (PageView, L), T-43b-016 (zoom) and T-43b-054 (test suite).
- Session path: T-43a-086 (event channel) to T-43a-075 (session state machine) to T-43a-077 (replace the placeholder reader page).
- Progress path: T-43a-088 (DB writer) to T-43a-028 (page-change save) to T-43a-029 (completion).

Priority-inversion tasks in section 4.3 sit on this path; resolve them before the path can run at P0.

### 6.3 Risks

1. Viewer contract churn: the reader core and area:44 both change `Viewer`, `ViewerChapters` and events. Freeze T-43a-087 and T-43a-093 before either side starts (area leads of 43 and 44 must agree).
2. Preference key drift: any renamed key breaks backup import. Mitigated by A-2 and the key catalog test (T-43b-002, T-43b-003).
3. Page-decode memory: webtoon pages can be very tall. Mitigated by the 16384 px cap (D-9) and tiling decision, but only verified at runtime.
4. GTK 4.16 API surface: several gtk4-rs 0.11 signatures in the design docs are proposals, not verified (checklist in [p1-rust-design.md](p1-rust-design.md) section 16). Verify each before its task starts.
5. Progress correctness: ordering of history flush before chapter switch (T-43a-025) and the incognito matrix. A regression here loses reading progress silently. Covered by tests in T-43a-082 and T-43b-054.
6. Long P0 chains: the critical path above runs through the HTTP loader. A slip there blocks the reader top bar, not only loading.

### 6.4 Lint and dependency fixes applied in this pass

- Dependency cycles removed (the lint did not catch them). Each fix keeps the producer before the consumer:
  - T-43a-045 no longer depends on T-43a-058. T-43a-058 (set as cover) depends on the sheet, and the sheet's "Set as cover" entry is wired by T-43a-058.
  - T-43a-047 no longer depends on T-43a-052. The general tab hosts the background row; T-43a-052 supplies the values and depends on the tab.
  - T-43a-064 and T-43a-066 now depend on T-43a-081 (the reader settings store) instead of T-43a-048. The tab (T-43a-048) is the editor for these values, not their owner.
  - T-43b-032 no longer depends on T-43b-033. The downloaded-flag lookup is the producer; the transition host consumes it.
- Prose `area:` tokens were normalised across the folder to full area ids (for example `area:44` to `area:44-reader-viewers`, `area:21/22` to `area:21-backup-format-and-create and area:22-backup-restore`).
- Task IDs were not changed.

---

## 7. Definition of done for this area in Kurobayomi

Each item is observable in the running app with a library entry from one working source.

1. Open a chapter from the library, the details screen, history or updates. The first page appears, and the reader title shows the manga and chapter.
2. Pages load from all five sources: a local folder, a `.zip`/`.cbz` file, an `.epub`, a downloaded chapter (directory or `.cbz`), and an HTTP source. Loader precedence matches [p1-topic-chapter-loading.md](p1-topic-chapter-loading.md) section 7.
3. An HTTP page that fails shows the error state with Retry and recovers on Retry. A downloaded-only chapter that is not downloaded shows the blocked-chapter message.
4. Next and previous chapter work from the navigator, the bottom bar and the N/P keys. Crossing a chapter boundary shows the transition card with the previous, current and next labels.
5. Every page change saves the last page read. Reaching the last page marks the chapter read once (idempotent). History is updated per the write matrix. Incognito writes nothing.
6. Reading mode, rotation and crop borders can be set per manga and globally. The per-manga choice wins. Changes apply to the open reader without reopening.
7. The reader settings window has its three tabs. Each control reads and writes the Aniyomi preference key of the same name, and changes apply live.
8. Brightness (negative values darken), colour filter with six blend modes, grayscale, invert, and the e-ink flash all work on the page area only.
9. Long press or right-click on a page opens the actions sheet. Save writes the image to the configured folder and shows the saved notification. Copy puts the image on the clipboard. Share opens the file. Set as cover updates the manga cover, with the messages in [p1-behavior.md](p1-behavior.md) section 20.
10. Download-ahead requests the configured number of following chapters. Delete-after-read removes a chapter once it is read, when the setting is on.
11. Every user-visible message key in [p1-behavior.md](p1-behavior.md) section 20 and [p2-behavior.md](p2-behavior.md) section 15 is shown with its English text.
12. `cargo test` passes for the pure-logic modules: chapter list, sort, dedupe, gap, flags, references, transitions, long press, zoom model, nav regions, preference defaults and ranges, and the HTTP loader with a scripted mock.
13. The mockup gap list (G-01 to G-35 in [p1-ui-parity.md](p1-ui-parity.md) section 12) is closed or explicitly assigned to area:44-reader-viewers with a reference.
14. Dropped items in section 4.4 behave as described there: hidden where there is no Linux equivalent (cutout), no-op where the value is only stored (screen rotation lock), and the replacement where one is defined (open or show in file manager instead of share).
