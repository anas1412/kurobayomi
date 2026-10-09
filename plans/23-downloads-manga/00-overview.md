# Area 23: Manga downloads, overview

Area lead overview for the manga download subsystem of Aniyomi, as it is rewritten for Kurobayomi (Rust, gtk4-rs 0.11 without libadwaita, tokio, rusqlite). Paths in this folder are relative to `/home/blackbox/aniyomi-src` (Apache-2.0), unless stated otherwise. Format: **OBSERVED** is what the Kotlin does (path:line). **Proposal:** is what the Rust port should do.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here

Manga downloads let a reader save chapters to local storage and read them offline. The subsystem:

- queues chapters from the browse, details, library, updates and reader screens;
- runs a scheduler that fetches page images for up to 5 sources at once, one chapter per source, two pages per chapter;
- retries failed pages with a 2, 4, 8 second back-off;
- checks each chapter for completeness, writes ComicInfo.xml, and stores the result as a `.cbz` archive (default) or as a folder;
- indexes the downloads folder so every "is downloaded" badge and size/count shows without rescanning;
- deletes downloaded chapters after reading (with bookmark and category exclusions), and lets the user rename folders when a source or chapter is renamed;
- shows a Downloads page with a queue (grouped by source, drag to reorder, sort, pause, resume, cancel) and progress notifications;
- keeps the queue across restarts and pauses when the network goes away or Wi-Fi is required but not present.

### 1.2 Why it matters for a usable Kurobayomi

- Offline reading is a core reason to use a manga reader on a laptop with unreliable connectivity.
- The reader (area:43) reads downloaded chapters from disk, so the on-disk layout is a shared contract. The folder names and CBZ format must stay compatible with Aniyomi so a copied Aniyomi download folder works on first run.
- The Downloads page is where a user notices failed chapters, so error visibility is a P0 concern even though the queue UI itself is P1.

### 1.3 In scope and out of scope

- In scope: everything under `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/` and `app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/`, plus the manga-only helpers they call (see section 2.1).
- Out of scope, owned elsewhere: anime downloads (area:24-downloads-anime), shared download preferences and the Downloads tab chrome (area:25-downloads-shared), reader page cache (area:34-images-covers-cache), source HTTP and page model (area:15-source-api-manga), notification channels and receiver (area:53-platform-manifest-intents-workers).
- Scope size: 17 files, 3,397 LOC (`p1-inventory.md`). Per-file LOC was re-checked against `wc -l` and the total matches.

### 1.4 Inventory counts (corrected)

`p1-inventory.md` gives "Core engine 2,305 LOC" and "UI files (9 files) 727 LOC". Re-counting the same files gives:

| Group | Files | LOC |
|---|---|---|
| Core engine and model (Downloader, Cache, Manager, Provider, Store, PendingDeleter, model/MangaDownload) | 7 | 2,326 |
| Platform glue (Job, Notifier) | 2 | 365 |
| UI (ui/download/manga) | 8 | 706 |
| Total | 17 | 3,397 |

The total agrees with the inventory. The split in the inventory (2,305 / 727 and "9 files") does not. The UI group has 8 files, not 9. This overview uses the corrected split. Fixing `p1-inventory.md` is left to its author.

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities

| Kotlin location (repo-relative) | LOC | Responsibility | Rust home |
|---|---|---|---|
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt | 774 | Engine: queue state, scheduler loop, page fetch, retries, throttle, integrity, ComicInfo, CBZ, finalise | `kuro-downloads::{queue, scheduler, downloader, pipeline, images, comicinfo, archive}` |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt | 492 | In-memory index of the downloads folder, hourly renew, snapshot `dl_index_cache_v3` | `kuro-downloads::index` |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt | 459 | Public facade used by other areas (enqueue, pause, clear, reorder, delete, rename, counts, sizes, flows) | `kuro-downloads::manager` |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadProvider.kt | 176 | Folder and file naming rules, path building, chapter lookup | `kuro-downloads::layout` (+ `sanitize`) |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadStore.kt | 149 | Persist the queue (SharedPreferences `active_downloads`) | `kuro-downloads::store` (SQLite `manga_download_queue`) |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadPendingDeleter.kt | 187 | Read chapters that wait for reader exit before deletion (`chapters_to_delete`) | `kuro-downloads::pending` (SQLite `manga_pending_delete`) |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt | 89 | Queue item and state enum (values 0 to 4) | `kuro-downloads::model` |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadJob.kt | 132 | WorkManager worker that keeps downloads alive and gates on network | replaced: `kuro-downloads` actor + `SchedulerBackend::hold` + `kuro-platform` connectivity |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadNotifier.kt | 233 | Progress, paused, complete, warning, error notifications | replaced: `kuro-platform::Notifier` (gio::Notification) |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreenModel.kt | 272 | View model: groups, reorder, row menu, sort, progress hooks | `src/download_queue_model.rs` |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreen.kt | 93 | Compose page wrapper | `src/download_queue.rs` |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueTab.kt | 44 | Tab title "Manga", count | `src/download_queue.rs` |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadAdapter.kt | 27 | FlexibleAdapter, blocks moves between groups | dropped as a mechanism; the rule is kept in `src/download_queue.rs` |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHeaderHolder.kt | 37 | Group header row widget | `src/download_queue.rs` |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHeaderItem.kt | 64 | Group model (id, name, size) | `src/download_queue.rs` (model) |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHolder.kt | 101 | Row widget (title, chapter, progress, menu) | `src/download_queue.rs` |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadItem.kt | 68 | Row model, equality by chapter id | `src/download_queue.rs` (model) |

Cross-package code this area calls (owned elsewhere, listed in `p1-inventory.md`): `Page` (source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt), `DownloadPreferences` (domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt), `DataSaver` (app/src/main/java/aniyomi/util/DataSaver.kt), `ChapterCache` (app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt), `ImageUtil` (core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt), `DiskUtil` (core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt), `ZipWriter` (core/archive/src/main/kotlin/mihon/core/archive/ZipWriter.kt), `Notifications` (app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt), `SecurityPreferences` (core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt).

### 2.2 Proposed Rust crate and module map

New crate `crates/kuro-downloads` (no GTK, no OS code except through `kuro-platform`):

| Module | Port of | Responsibility |
|---|---|---|
| `lib.rs` | none | Re-exports `DownloadManager`, `DownloadEvent`, `DownloadState`, `DownloadError` |
| `model.rs` | MangaDownload.kt | `DownloadState` (`#[repr(u8)]` 0 to 4), `PageState`, `Page`, `QueuedDownload`, `ChapterRef`, `MangaRef`, `SourceRef` |
| `error.rs` | none | `DownloadError` (thiserror) mapped to message keys |
| `layout.rs` | MangaDownloadProvider.kt | Folder and file names, path building |
| `sanitize.rs` | DiskUtil.buildValidFilename | Name sanitiser and reserved-name rule |
| `index.rs` | MangaDownloadCache.kt | Scan, renew, snapshot, query |
| `store.rs` | MangaDownloadStore.kt, PendingDeleter tables | SQLite queue and pending delete store |
| `queue.rs` | MangaDownloader state | Pure in-memory state machine, no I/O |
| `scheduler.rs` | MangaDownloader loop | Per-source choice, at most 5 sources, one chapter each |
| `downloader.rs` | MangaDownloader actor | Owns `QueueState`, runs the scheduler, emits events |
| `pipeline.rs` | MangaDownloader.downloadChapter | One chapter: precheck, page list, fetch, integrity, finalise |
| `images.rs` | ImageUtil, copy from cache, extension | Image type, tall-image split |
| `throttle.rs` | okio Throttler | Shared bytes-per-second limiter |
| `comicinfo.rs` | createComicInfoFile | ComicInfo.xml writer (quick-xml) |
| `archive.rs` | ZipWriter | CBZ writer (zip crate, Stored) |
| `manager.rs` | MangaDownloadManager | Public facade `DownloadManager` |
| `pending.rs` | MangaDownloadPendingDeleter | Pending delete logic |
| `events.rs` | Kotlin StateFlows | `DownloadEvent` enum |
| `testing.rs` | none | Fake source, fake clock, temp trees (feature `testkit`) |

Other crates touched: `kuro-platform` gets `SchedulerBackend::hold` and `ConnectivityMonitor` (GIO) and `Notifier`; the GUI crate (`kurobayomi`, in `src/`) gets `download_queue.rs`, `download_queue_model.rs`, and optional `download_notice.rs`. Dependency direction: GUI → `kuro-downloads` → `kuro-core`, `kuro-platform`, and the domain traits of areas 03, 05, 07, 08, 15, 17, 27.

### 2.3 Threading model (summary of `p1-rust-design.md` section 2)

- GTK main thread owns widgets and the queue view model. It receives `DownloadEvent`s through `async_channel` in `glib::spawn_future_local`.
- The tokio runtime owns the `Downloader` actor, page tasks and the index scanner.
- `spawn_blocking` runs filesystem scans, image decoding, zip writing and rusqlite calls.
- Only the actor mutates `QueueState`. Store writes happen in the same actor step as the in-memory change.

---

## 3. Reading guide

All documents are in this folder. Read in the order shown for a first pass.

1. [00-overview.md](00-overview.md): this document.
2. [p1-inventory.md](p1-inventory.md): the 17 Kotlin files, their declarations, port decisions, target Rust modules, and cross-area symbols.
3. [p1-behavior.md](p1-behavior.md): the observed behaviour of every path (state transitions, scheduler, job, pipeline, deletion, rename, notifications, queue screen), with file:line citations and Proposal notes; section 15 lists user-visible message keys and section 16 lists all timings and limits.
4. [p1-data-and-config.md](p1-data-and-config.md): in-memory models, persisted records (SharedPreferences, `dl_index_cache_v3`, ComicInfo.xml, CBZ), the Rust SQLite tables, on-disk layout, preference keys and defaults, network endpoints, constants, and data invariants.
5. [p1-rust-design.md](p1-rust-design.md): crate and module layout, threading, key types and traits, actor loop, connectivity and notifications, page pipeline, public facade, pitfalls, Android-to-Linux API mapping, and the plan for the existing mockup.
6. [p1-ui-parity.md](p1-ui-parity.md): screen anatomy, sort menu, row menu, footer action, empty and error states, interaction and keyboard rules, responsive rules, strings, mockup gaps, and the view model contract.
7. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): 114 edge-case ids (numbered up to EC-113) by module, failure modes, race catalogue, test fixtures, and the TST-01 to TST-52 test list.
8. [p1-topic-defect-register.md](p1-topic-defect-register.md): 38 verified defects (D-01 to D-38) with the Rust fix and task, plus 15 deliberate behaviours to keep (K-01 to K-15).
9. [p1-topic-decision-log.md](p1-topic-decision-log.md): 14 open decisions (D-DL-01 to D-DL-14) with options and the recommendation the plan assumes.
10. [p1-tasks.md](p1-tasks.md): 45 implementation tasks (T-23a-001 to T-23a-045) in the required task format.

---

## 4. Key design decisions and open questions

Each item below needs the user's yes or no before the matching task starts. The plan assumes the recommendation until the user answers. Full text is in `p1-topic-decision-log.md`.

| ID | Question | Recommendation (assumed) | Tasks |
|---|---|---|---|
| D-DL-01 | Reserved Windows device names (CON, PRN, AUX, NUL, COM1-9, LPT1-9) rejected on Linux too? | Yes. Append `_` (`ntfs-names` feature, default on), so copied folders stay valid on the Windows port | T-23a-003 |
| D-DL-02 | The Kotlin "Maximum downloads" setting (`download_slots`) does nothing for manga. What should Rust do? | Keep as "Maximum downloads", rename to `downloads.max_parallel_chapters`, default 5, cap 5 | T-23a-037 |
| D-DL-03 | Importance of failure notifications | Errors and warnings at default importance (sound and banner); progress and paused at low | T-23a-019 |
| D-DL-04 | What is "Download only over Wi-Fi" on Linux? | "Not on a metered connection" (GIO `network_metered`). The settings label must say "metered" | T-23a-023 |
| D-DL-05 | "Move to top" in the queue | Within the item's source group only | T-23a-025 |
| D-DL-06 | Resume automatically after the network returns? | Yes, unless the user paused. Network-only errors return to QUEUE | T-23a-023 |
| D-DL-07 | Keep downloads running when the window is closed? | Yes. Process kept alive with a GApplication hold while work exists | T-23a-023, T-23a-011 |
| D-DL-08 | Two chapters map to the same folder name | Warn at enqueue. Do not silently treat the second as downloaded | T-23a-024 |
| D-DL-09 | resmush.it data-saver service | Drop. It is a plain-HTTP third-party service and the Kotlin parses JSON by string search. Final call belongs to area:26-library-update-jobs | T-23a-017 |
| D-DL-10 | Write `.nomedia` at the downloads root | Never. Desktop gallery indexers are not a target | T-23a-022 |
| D-DL-11 | Retry HTTP 404 and 410 | Fail at once. Retrying a missing page wastes about 14 s per page | T-23a-013 |
| D-DL-12 | AVIF, HEIF and JXL pages in the Rust build | Recognise and store; the reader decodes them (area:43-reader-core). The split step skips them | T-23a-014, T-23a-018 |
| D-DL-13 | Tall images above 256 MiB decoded | Skip the split with a log line in the first release | T-23a-018 |
| D-DL-14 | Pending deletes flushed on reader exit only, or also at startup? | Also at startup. A crash on reader exit would otherwise keep read chapters forever | T-23a-028 |

### 4.1 Design decisions already fixed (not open)

- Folder layout, file names, CBZ format (ZIP, STORE, UTF-8 names, flat entries) and ComicInfo.xml fields are fixed for compatibility (K-01 to K-04, K-14, K-15).
- Concurrency: at most 5 sources, one chapter per source, two pages per chapter (K-06). The retry schedule 2, 4, 8 seconds (K-07), minimum free space 200 MiB = 209715200 bytes (K-08), and warning thresholds 30 queued and 15 per source (K-11) are kept.
- The SQLite queue and pending-delete tables replace SharedPreferences. The downloaded folders are reused as they are (T-23a-039).
- Single owner for queue state: the actor (rust design section 2 rule 1).
- Index: stores bare chapter names, never `.cbz`. Case-insensitive source folder matching (TST-41).
- Page file numbering uses one width everywhere. This fixes the prefix collision for pages 1000 and above (D-05).

### 4.2 Inconsistencies found while writing this overview (need a decision or a fix)

1. Error notification importance. `p1-tasks.md` T-23a-019 and `p1-rust-design.md` section 5.3 say error notifications stay at low importance "like Kotlin". `p1-topic-decision-log.md` D-DL-03 and `p1-topic-defect-register.md` D-27 recommend default importance for errors. Recommendation: follow D-DL-03 and update T-23a-019 and rust design 5.3 once the user confirms.
2. Priority inversion. T-23a-022 (finalisation: CBZ or folder) and T-23a-021 (ComicInfo.xml) are P1, but without them a downloaded chapter cannot be produced as a readable file, and P0 test task T-23a-036 depends on both. T-23a-026 (cancel API, P1) is a dependency of P0 task T-23a-027 (delete API). Recommendation: promote T-23a-022 and T-23a-026 to P0. Promote T-23a-021 to P0 too, or make it optional so that 022 does not depend on it.
3. Cycle fixed in this pass. T-23a-011 depended on T-23a-019 and T-23a-019 depended on T-23a-011. T-23a-011 now depends on T-23a-001 and T-23a-010. The notifier consumes the events that the lifecycle emits, so the direction is correct.
4. Bare area tokens fixed in this pass. Four prose references used `area:03`, `area:26`, `area:34` and `area:51`. They are now full ids (`area:03-db-manga-schema-migrations`, `area:26-library-update-jobs`, `area:34-images-covers-cache`, `area:51-i18n`).
5. Inventory split. See section 1.4.
6. `download_slots` default. `p1-data-and-config.md` section 5 and T-23a-037 give default 1, as in DownloadPreferences.kt:69. Decision D-DL-02 proposes default 5 for the renamed key. This is a new default, not a port, so it is marked as a Proposal.

### 4.3 Items for the user (not for the plan to decide)

- Whether Kurobayomi should keep the "Maximum downloads" control (D-DL-02). A working control changes the user-visible behaviour of Aniyomi.
- Whether to keep resmush.it (D-DL-09). Final call with area:26-library-update-jobs.
- Whether a Windows-first port of the queue file is planned (decision log, notes). If yes, T-23a-039 needs revisiting.

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What is needed | Tasks |
|---|---|---|
| area:01-foundation-app-bootstrap | tokio runtime, `kuro-core` bootstrap, `kuro-platform` with `SchedulerBackend::hold`, `tracing`, `rustix` | T-23a-001, T-23a-023, T-23a-041 |
| area:02-preferences-system | Typed settings with the keys and defaults in `p1-data-and-config.md` section 5, `SecurityPreferences` for `hide_notification_content` | T-23a-006, T-23a-015, T-23a-037, T-23a-043 |
| area:03-db-manga-schema-migrations | Migration step that creates `manga_download_queue` and `manga_pending_delete` | T-23a-009 |
| area:05-data-repos-entries-items | Chapter and manga repositories used by the store and the enqueue API | T-23a-009 |
| area:08-domain-library-category-history-updates | Library categories for exclusions and ComicInfo categories | T-23a-021 |
| area:10-app-domain-layer | Chapter rename callers and `renameSource`, app-level domain services | T-23a-027, T-23a-029 |
| area:11-app-migrations | Confirms no migration of Android download state (dropped) | T-23a-039 |
| area:12-network-stack | HTTP client and streamed bodies with progress, `Content-Length` | T-23a-012, T-23a-013 |
| area:13-core-util-storage-archive-torrent | `ImageUtil` (split, extension sniffing), `DiskUtil` (filename rules), zip, file watcher | T-23a-003, T-23a-014, T-23a-018, T-23a-022, T-23a-045 |
| area:14-source-local-and-metadata | ComicInfo model, local source counts | T-23a-021, T-23a-042 |
| area:15-source-api-manga | `Page`, `ChapterSource` trait (page list, image URL, image bytes, chapter URL) | T-23a-002, T-23a-004, T-23a-012, T-23a-024 |
| area:16-source-api-anime | none directly; shared source gate concepts only | none |
| area:17-source-app-layer | Source manager and the source-ready gate used by the index timeout and startup restore | T-23a-006, T-23a-035 |
| area:25-downloads-shared | `DownloadPreferences` keys, `DeleteChapterDownload` interactor, Downloads tab chrome | T-23a-027, T-23a-033, T-23a-037 |
| area:26-library-update-jobs | `downloadChapters(autoStart = false)` from library updates, DataSaver decision | T-23a-017, T-23a-024 |
| area:27-tracking-core | Tracker remote URLs for ComicInfo `Web` | T-23a-021 |
| area:34-images-covers-cache | `PageImageCache` trait for the reader page cache reuse | T-23a-016 |
| area:36-ui-shell-navigation-security-webview | Downloads page placement, badges, shell chrome | T-23a-033, T-23a-040 |
| area:43-reader-core | Reader calls `buildPageList`, `flush_pending_deletes` on exit, enqueue for auto-download | T-23a-028, T-23a-031 |
| area:51-i18n | Base strings such as `download_notifier_cache_renewal` | T-23a-040 |
| area:53-platform-manifest-intents-workers | Notification channels, notification action routing (Pause, Resume, Cancel all, Show manga) | T-23a-019, T-23a-023, T-23a-038 |

### 5.2 What other areas need from this one

| Consumer area | What they call | Notes |
|---|---|---|
| area:42-ui-updates-history-stats-storage-upcoming | `DownloadManager` size and count (storage screen), `queued(chapter_id)` (update row indicator) | T-23a-030, T-23a-042 |
| area:43-reader-core | `buildPageList`, `flush_pending_deletes`, enqueue for auto-download and "download next", queue pending deletes | T-23a-028, T-23a-030, T-23a-031 |
| area:26-library-update-jobs | `downloadChapters(autoStart = false)` and the DataSaver decision | T-23a-017, T-23a-024 |
| area:10-app-domain-layer | `renameChapter` and `renameSource` from chapter sync and source manager | T-23a-029 |
| area:47-settings-screens-general | Download settings screen reads and writes the keys of section 5 | T-23a-037 |
| area:36-ui-shell-navigation-security-webview | Downloads page entry and count | T-23a-033 |
| area:53-platform-manifest-intents-workers | Notification actions call `DownloadManager::{start, pause, clear, open}` | T-23a-019, T-23a-023 |
| area:24-downloads-anime | Shared patterns (queue layout, index idea) only; no code shared | none |

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, which rewrote `/home/blackbox/kurobayomi/plans/TASKS.csv`. Rows whose file starts with `23-downloads-manga/`: 45.

### 6.1 Counts

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 12 | 11 | 0 | 0 | 23 |
| P1 | 9 | 6 | 1 | 0 | 16 |
| P2 | 4 | 0 | 0 | 0 | 4 |
| P3 | 2 | 0 | 0 | 0 | 2 |
| Total | 27 | 17 | 1 | 0 | 45 |

Tasks: P0 are T-23a-001, 002, 003, 004, 005, 006, 007, 008, 009, 010, 011, 012, 013, 014, 019, 020, 023, 024, 027, 030, 031, 035, 036. P1 are 015 to 018, 021, 022, 025, 026, 028, 029, 032 to 034, 037, 044, 045. P2 are 040 to 043. P3 are 038 and 039.

### 6.2 P0 critical path (in order)

Estimate uses effort ceilings (S at most 1 day, M at most 3 days). It is an upper bound for one engineer.

1. T-23a-001 crate skeleton (S), then T-23a-002 model types (S).
2. T-23a-008 queue state machine (M), then T-23a-010 per-source scheduler (M).
3. T-23a-011 lifecycle (M), which needs the events from T-23a-001.
4. Pipeline: T-23a-012 chapter pipeline (M), T-23a-013 page fetch (M), T-23a-014 image naming and resume (S), T-23a-020 integrity check (S).
5. Finalisation: T-23a-021 ComicInfo (M), T-23a-022 CBZ or folder (M) (see section 4.2 item 2; these are P1 today).
6. T-23a-036 test suite (M) closes the core.
7. In parallel after step 3: T-23a-019 notifications (M), T-23a-023 background running and network gating (M), T-23a-024 enqueue API (M), T-23a-030 manager facade (S).
8. In parallel, the index and naming chain: T-23a-003 (S), T-23a-004 (S), T-23a-005 (S), T-23a-006 index (M), T-23a-007 index mutations (S), then T-23a-027 deletion API (M) and T-23a-031 reader page list (S).
9. Persistence and restart: T-23a-009 SQLite queue store (S) and T-23a-035 restore gated on sources (S).

Ceiling for the longest chain of steps 1 to 6 is about 28 working days. The index and deletion chain and the persistence chain run alongside, so the P0 set is about 5 to 6 weeks for one engineer at the ceilings, less in practice. These are effort ceilings, not measured estimates.

### 6.3 Risks

1. Source contract drift. Page list, image URL and image bytes semantics come from area:15-source-api-manga. Until that contract is final, the pipeline uses `FakeSource` scripted responses (rust design section 12).
2. Memory on tall images. A 50 000 px RGBA page needs about 800 MB to decode in full (rust design pitfall 10). Mitigation: skip the split above 256 MiB decoded (D-DL-13).
3. Image decoding support. AVIF decode may need `dav1d` at build time (rust design section 13). If so, AVIF is recognised but not split.
4. Metered is not Wi-Fi on Linux. GIO reports metered, not interface type. The label must say "metered". Users who expect a Wi-Fi check will see different behaviour (D-DL-04).
5. Folder compatibility with Aniyomi. Any change to the naming rules (D-05 width fix, D-28 240-byte cap, D-DL-01 reserved names) changes names only for affected titles. This must be checked against an Aniyomi fixture (TST-37, T-23a-039 acceptance).
6. Migration ordering. The two SQLite tables are created by a migration that area:03 numbers. If area:03 lands later, the queue store is blocked (T-23a-009).
7. Notification fidelity. `gio::Notification` replaces by id and has no progress bar. The progress text uses the body (rust design 5.3). Desktop environments vary in how they show default-priority notifications.
8. Filesystem watcher reliability. The `notify` crate watcher (T-23a-045) can fail on some network mounts. The hourly renew remains as fallback.
9. Cross-area timing. Reader (area:43) and the Downloads page (area:36) depend on the manager facade (T-23a-030). Any change to the facade API affects those areas.
10. Dependency cycle risk. Fixed in this pass for T-23a-011 and T-23a-019 (section 4.2 item 3). Re-run the cycle check after any task change.

---

## 7. Definition of done (observable in Kurobayomi)

Each item is checked on a Linux desktop with the window open and with the window closed.

1. A copied Aniyomi downloads folder (with `<Source> (LANG)/<Manga>/` folders holding `<scanlator>_<chapter>` folders or `.cbz` files) shows the chapters as downloaded after the first index scan. Badges and counts match.
2. Queuing chapters from a details page adds rows grouped by source, with the source count in the group header. Rows show progress as "done/total" and a bar, and ERROR rows show an error label.
3. The queue runs at most 5 sources and 1 chapter per source, with 2 pages per chapter. Failed pages retry after 2, 4 and 8 seconds. A 404 or 410 fails at once (D-DL-11).
4. Downloads continue after the window is closed, and the process exits when the queue is empty. The idle process shows zero wakeups (TST-17).
5. Network loss stops the queue with the text "No network connection available". On a metered connection with the setting on, the text is "No Wi-Fi connection available". When the network returns and the user has not paused, the queue restarts by itself.
6. Pause, Resume, Cancel, Cancel all, Start now, Move to top (within group), Move to bottom, and the four sort options behave as in `p1-ui-parity.md` section 1.
7. A chapter is ERROR, not DOWNLOADED, when the integrity check fails. The file set is never left half-written: a failed CBZ leaves no `.cbz.part`.
8. Downloaded chapters open in the reader from the CBZ or the folder, in name order. Empty chapters show "No pages found".
9. Deleting chapters with the bookmark and category exclusions applied removes the files, updates the badges, and removes the manga folder when empty. A running chapter is paused before removal. Read chapters are deleted when the reader closes, and after a crash at next start (D-DL-14).
10. Renaming a source or a chapter keeps the badges right. A case-only source rename works (TST-48).
11. Restarting the app restores the queue in order. Rows for sources not yet loaded are kept. DOWNLOADING is never restored.
12. Notifications: progress "Downloading (x/y)" with the chapter title cut to 30 characters; paused with Resume and Cancel all; error with "<manga>: <chapter>" and never the text "null" (EC-110); "Show manga" opens the manga.
13. Hiding notification content (`hide_notification_content`) removes names from notifications on the next update (EC-107).
14. `cargo test -p kuro-downloads` passes all 52 tests TST-01 to TST-52, without network and without a display. The crate has no gtk4 or glib dependency (`cargo tree -p kuro-downloads`).
15. Every dropped item (T-23a-038, T-23a-039, and the resmush.it drop if chosen) is written up in the crate `lib.rs` and in the PR description, with what the user loses.
