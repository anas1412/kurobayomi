# Area 23 — Manga downloads: edge cases, failure modes, tests

Upstream tests: none. A search of the whole Aniyomi tree for test sources that mention `MangaDownload`, `DownloadCache`, `DownloadProvider`, `ImageUtil`, `DiskUtil` or `buildValidFilename` returns nothing (`find . -path "*/test/*"` plus grep). So every test below is new. Behaviour references point to `p1-behavior.md`, data references to `p1-data-and-config.md`, design to `p1-rust-design.md`.

IDs: `EC-nn` = edge case (what can happen and what the port must do); `FM-nn` = failure mode; `RC-nn` = race; `TST-nn` = test to write.

---

## 1. Queue (`queue.rs`, `manager.rs`)

| ID | Scenario | Kotlin today (behaviour ref) | Required Rust behaviour |
|---|---|---|---|
| EC-01 | Enqueue an empty chapter list | returns (3.3) | `EnqueueOutcome::NothingToDo`, no event |
| EC-02 | Enqueue a chapter already downloaded (folder or `.cbz`) | filtered out (3.3 step 4.1) | filtered by index lookup before the queue |
| EC-03 | Enqueue a chapter already queued | filtered (3.3 step 4.3) | filtered by `chapter.id` |
| EC-04 | Enqueue into a non-empty queue while stopped | added, nothing starts (3.3 note) | added; no auto-start (documented) |
| EC-05 | Enqueue into an empty queue with `auto_start=false` | added, not started (library updater) | added, not started |
| EC-06 | Enqueue 31 chapters from unmetered source vs 31 from metered | warning only counts metered (3.3 step 5) | same counts; warning emitted only on `wasEmpty` and `auto_start` |
| EC-07 | Enqueue 16 chapters from one metered source | warning (max > 15) | warning with 30 s timeout and help URL |
| EC-08 | Source is not an `HttpSource` (local source) | queue refuses silently (3.3 step 2) | `EnqueueOutcome::SourceNotHttp`, UI message only in log |
| EC-09 | `start_now` for a chapter that is queued | moves to index 0, reorders (3.1) | same; ERROR status is reset to QUEUE by reorder in Kotlin, kept in Rust (behaviour 1.1) |
| EC-10 | `start_now` for a chapter that is not queued and whose source is missing | returns (3.1) | returns `DownloadError::Source` with the key text |
| EC-11 | `add_to_front` with empty list | returns (3.1) | no-op |
| EC-12 | `reorder` with an id not in the queue | Kotlin silently rebuilds (3.5) | `QueueError::UnknownId`, queue unchanged |
| EC-13 | `reorder` drops an id (list shorter than queue) | the dropped item is removed from the queue (updateQueue replaces it) | `QueueError::MissingId`, queue unchanged (Proposal) |
| EC-14 | Reorder while running | pause, rebuild, start again (3.5) | same sequence; running chapters go to QUEUE then restart |
| EC-15 | Clear while running | cancel, clear, stop (3.7) | same; events `Dismiss` then `Complete` |
| EC-16 | Pause while nothing runs | `isPaused = true`, stop emits `Paused` if non-empty (3.6) | `Paused` event if queue non-empty; `is_paused` true until next start |
| EC-17 | Stop with a reason while running | warning, no job stop, paused flag not cleared (3.6 step 3) | warning, paused flag cleared, hold released (Proposal fix) |
| EC-18 | Start with empty queue | returns false (3.6) | no-op, no event |
| EC-19 | Start when already running | returns false (3.6) | no-op |
| EC-20 | Start after an ERROR run | all non-DOWNLOADED reset to QUEUE and retried (3.6 step 2) | same; ERROR items retried on explicit start |
| EC-21 | Cancel a chapter that is DOWNLOADING | status NOT_DOWNLOADED, job cancelled, partial files left (4) | same status; tmp folder left (resume later) |
| EC-22 | Cancel all for a manga while a chapter of it is DOWNLOADING | `removeFromQueue(manga)` without pause (7.3, defect) | pause first; then remove; then restart (Rust fix) |
| EC-23 | Position of rows after `add_to_front` when a source group already exists | inserted at queue index 0; group order changes (2) | same |
| EC-24 | Queue order after one "Move to top" with interleaved sources | rebuilt into group order (14.3) | Rust keeps group rebuild rule (Kotlin behaviour) as an explicit option; default: move within group only (Proposal) |

Tests:

| ID | Test | Asserts |
|---|---|---|
| TST-01 | `queue::add_all_sets_queue_and_appends` | status QUEUE, order preserved |
| TST-02 | `queue::remove_resets_downloading_and_queue` | DOWNLOADING and QUEUE become NOT_DOWNLOADED; ERROR and DOWNLOADED untouched |
| TST-03 | `queue::pause_running_sets_queue_and_flag` | DOWNLOADING -> QUEUE, `is_paused` true |
| TST-04 | `queue::stop_running_sets_error` | DOWNLOADING -> ERROR |
| TST-05 | `queue::reset_for_start_keeps_downloaded` | ERROR -> QUEUE, NOT_DOWNLOADED -> QUEUE, DOWNLOADED not in queue |
| TST-06 | `scheduler::pick_active_first_per_source_max_5` | 7 sources, 2 chapters each: 5 ids, first of each, in first-occurrence order |
| TST-07 | `scheduler::pick_active_skips_error` | ERROR first in a source yields the second chapter |
| TST-08 | `scheduler::pick_active_empty_when_all_finished` | all DOWNLOADED/ERROR -> empty, `all_finished` true |
| TST-09 | `queue::reorder_rejects_unknown_and_missing_ids` | EC-12, EC-13 |
| TST-10 | `queue::reorder_keeps_error_status` | Proposal behaviour |
| TST-11 | `manager::enqueue_filters_downloaded_and_queued` | EC-02, EC-03 with a temp directory fixture |
| TST-12 | `manager::enqueue_sorts_by_source_order_desc` | highest `source_order` first (EC behaviour 3.3) |
| TST-13 | `manager::warning_thresholds` | EC-06 and EC-07 boundaries: 30 is not a warning, 31 is; 15 is not a warning, 16 is |

---

## 2. Scheduler and runtime (`downloader.rs`)

| ID | Scenario | Required behaviour |
|---|---|---|
| EC-25 | 6 sources with work | exactly 5 pipelines start; the 6th starts when one finishes |
| EC-26 | One source has 3 chapters | only one runs at a time, the others wait |
| EC-27 | A chapter enters ERROR while others run | the source's next chapter starts; the other sources are not affected |
| EC-28 | The last chapter finishes and others are ERROR | `stop()` emits `Complete`, hold released |
| EC-29 | Panic inside a pipeline | error event with the panic message, actor keeps running |
| EC-30 | Connectivity drops mid-download | all DOWNLOADING chapters become ERROR with the network message; loop ends; no busy spin (behaviour 5.4) |
| EC-31 | Connectivity returns and the user did not pause | Proposal: auto resume; network-only errors go back to QUEUE |
| EC-32 | Connectivity returns after the user paused | nothing starts |
| EC-33 | Metered connection with "Wi-Fi only" on | stop with "No Wi-Fi connection available" |
| EC-34 | Metered connection with "Wi-Fi only" off | continue |
| EC-35 | Connectivity flaps every second | at most one `Warning` per transition; no task leak (JoinSet empty after stop) |
| EC-36 | Shutdown while downloading | tasks aborted; chapters left in QUEUE on restart (store keeps rows) |
| RC-01 | `Pause` racing with a task completion | completion after abort must not set DOWNLOADED; status stays QUEUE (command ordering in the actor) |
| RC-02 | `Cancel` racing with page completion of the same chapter | chapter removed; late page results dropped; no folder created after cancel |
| RC-03 | Two `Enqueue` commands for the same chapter in one tick | one row; second filtered |
| RC-04 | `delete_chapters` while the chapter downloads | pause, remove, delete folders, restart if queue non-empty (7.2) |
| RC-05 | Progress events flood (1000 pages, 2 concurrent) | at most 20 progress events per second per chapter |

Tests:

| ID | Test | Asserts |
|---|---|---|
| TST-14 | `downloader::concurrency_limits` (fake source with a barrier) | max 5 sources, 1 per source, 2 pages per chapter (instrumented counters) |
| TST-15 | `downloader::error_skips_then_continues` | EC-27 |
| TST-16 | `downloader::network_loss_stops_and_marks_error` | EC-30 with a fake `ConnectivityMonitor` |
| TST-17 | `downloader::no_busy_loop` | with `tokio::time::pause()`, a running downloader with no events performs 0 iterations between sleeps (count the actor's wakeups) |
| TST-18 | `downloader::pause_then_completion_race` | RC-01 with a barrier |
| TST-19 | `downloader::cancel_mid_write_leaves_tmp` | EC-21: `_tmp` exists, no `.cbz`, status NOT_DOWNLOADED |
| TST-20 | `downloader::progress_is_coalesced` | RC-05 |

---

## 3. Page pipeline and images (`pipeline.rs`, `images.rs`)

| ID | Scenario | Kotlin today | Required Rust behaviour |
|---|---|---|---|
| EC-37 | Page list empty | "No pages found", chapter ERROR, notification (behaviour 6 step 5) | same key, same status |
| EC-38 | Page list has pages with `imageUrl` set | not refetched (`isNullOrEmpty` check) | same |
| EC-39 | `getImageUrl` fails for one page | page ERROR, no notification at that point; chapter fails integrity later | same; the per-page error goes into the error notification |
| EC-40 | Image 404 on first attempt, 200 on second | retry after 2 s | same schedule (TST-24) |
| EC-41 | Image 500 on all four attempts | ERROR page, notification with message | same |
| EC-42 | Content-Type `image/webp` but body is JPEG | extension from mime: `webp` (wrong) | Proposal: sniff bytes and prefer the sniffed type when they disagree |
| EC-43 | No Content-Type, body PNG | sniffed `png` | same |
| EC-44 | Body unrecognised | `jpg` fallback | same |
| EC-45 | Existing `001.jpg` from an earlier run | reused without validation | reused; validation in Proposal: skip zero-length files |
| EC-46 | Existing zero-length `001.jpg` (partial write by a crash) | reused; chapter completes with a broken page | Proposal: treat zero length as missing |
| EC-47 | Cache hit with recognised type | copied, cache file deleted | same |
| EC-48 | Cache hit with unrecognised type | `.tmp` left, chapter fails (defect) | fallback to network (TST-27) |
| EC-49 | `split_tall_images` off | no split | same |
| EC-50 | Tall page, height exactly `4 * width` | split (`h / w = 4 > 3`) | split |
| EC-51 | Height `3.9 * width` | not split (integer division gives 3) | not split (same as Kotlin, documented) |
| EC-52 | Height exactly `optimal` (2 x display max) | `partCount = 1`, no split | no split |
| EC-53 | Height `optimal + 1` | 2 parts | 2 parts |
| EC-54 | Animated GIF tall | not split | not split |
| EC-55 | Page 1 in a 1000-page chapter | prefix collision with page 10 (defect) | correct file (TST-25) |
| EC-56 | Tall image that fails to decode | error logged, original kept | same; split files removed |
| EC-57 | 1000+ pages folder width | filename width 4 | same width everywhere |
| EC-58 | Download speed limit 0 | unlimited | unlimited (TST-26) |
| EC-59 | Speed limit changed mid-chapter | applies to next chapter (read per download) | same |
| EC-60 | Integrity: stray file in folder | chapter fails | Proposal: count page keys, not files (TST-23) |
| EC-61 | Integrity: `.tmp` file left | not counted, count matches if all pages present | same |
| EC-62 | CBZ archive write fails midway | `.cbz_tmp` left | `.cbz.part` removed; folder kept for resume |
| EC-63 | Folder rename fails (target exists) | ignored, chapter DOWNLOADED | error, chapter ERROR, folder kept |
| EC-64 | ComicInfo with empty categories | `Categories` omitted | same |
| EC-65 | ComicInfo chapter number -1 | `Number` omitted | same |
| EC-66 | ComicInfo chapter number 12.5 | `Number` "12.5" | same |
| EC-67 | ComicInfo manga status 99 | "Unknown" | same |
| EC-68 | Scanlator with spaces only | treated as blank: no prefix | same |
| EC-69 | Chapter name empty | folder name "Chapter" | same |
| EC-70 | Chapter name with `/` | `_` replacement | same |

Tests:

| ID | Test | Asserts |
|---|---|---|
| TST-21 | `images::extension_from_mime_and_sniff` | EC-42 to EC-44 table |
| TST-22 | `images::split_geometry_matches_kotlin` | the example 5000 px / 2000 px gives parts `[0,1666)`, `[1666,3332)`, `[3332,5000)`; table of 10 more heights whose expected parts are computed by hand from the same formula |
| TST-23 | `pipeline::integrity_counts_page_keys_not_files` | EC-60, EC-61 |
| TST-24 | `pipeline::retry_schedule_2_4_8_seconds` | `tokio::time::pause()`; 4 attempts; sleeps exactly 2 s, 4 s, 8 s |
| TST-25 | `images::split_prefix_width_1000_pages` | EC-55, EC-57 with 1234 fake pages |
| TST-26 | `throttle::zero_is_unlimited` and `throttle::rate_is_respected` (1 MiB at 512 KiB/s takes about 2 s) | |
| TST-27 | `pipeline::cache_unknown_type_falls_back_to_network` | EC-48 |
| TST-28 | `images::tall_split_writes_jpeg_q100_parts` | output names `001__001.jpg` and `001__002.jpg`; original removed |
| TST-29 | `images::animated_gif_not_split` | EC-54 |
| TST-30 | `archive::cbz_is_stored_flat_utf8` | open with `zip` crate: method Stored, no directories, UTF-8 name `ページ.jpg` round-trips |
| TST-31 | `comicinfo::element_order_and_namespaces` | exact XML string for a fixed manga/chapter (golden file) |
| TST-32 | `comicinfo::number_formatting` | EC-65, EC-66 |
| TST-33 | `pipeline::resume_keeps_finished_pages` | interrupt after page 3 of 9; second run downloads 4..9 only |

---

## 4. Naming and layout (`sanitize.rs`, `layout.rs`)

| ID | Input | Expected output |
|---|---|---|
| EC-71 | `"  ..Title.. "` | `"Title"` |
| EC-72 | `"   "` | `"(invalid)"` |
| EC-73 | `"a/b:c*d?e\"f<g>h|i\\j"` | `"a_b_c_d_e_f_g_h_i_j"` |
| EC-74 | control char `\u{0007}` | `_` |
| EC-75 | `"😀"` repeated 200 times | truncated to 240 UTF-16 units (120 emoji) |
| EC-76 | `"CON"` (ntfs-names on) | `"CON_"` |
| EC-77 | `"Name (EN)"` source dir | `"Name (EN)"` (parentheses allowed) |
| EC-78 | Title `"Mangá/Ż"` | `"Mangá_Ż"` |
| EC-79 | Two titles that differ only by `:` vs `_` | same folder (collision, documented) |

Tests:

| ID | Test |
|---|---|
| TST-34 | `sanitize::table` covering EC-71 to EC-79 |
| TST-35 | `layout::chapter_dir_name_with_scanlator` (`"Scan_Chapter 1"`, blank scanlator, null scanlator) |
| TST-36 | `layout::valid_names_include_cbz` (order: folder then `.cbz`) |

---

## 5. Index (`index.rs`)

| ID | Scenario | Required |
|---|---|---|
| EC-80 | First start with no index file | scan once; `renewing` shows indicator only if scan > 1 s (debounce 1 s) |
| EC-81 | Index file corrupt | delete, rescan |
| EC-82 | Source gate timeout (30 s) | keep old index, log, retry later (defect fix) |
| EC-83 | Folder for a source not installed | ignored |
| EC-84 | Folder name differs only by case from source dir | matched (lowercase) |
| EC-85 | Folder `_tmp` suffix | skipped |
| EC-86 | `.cbz` renamed by user to `.zip` | not counted |
| EC-87 | Chapter renamed (`rename_chapter`) to a `.cbz` name | index stores bare name (defect fix) |
| EC-88 | Manga folder deleted externally | index entry remains until invalidation (documented, 1 h) |
| EC-89 | `storage` root changed in preferences | invalidate and rescan |

Tests:

| ID | Test |
|---|---|
| TST-37 | `index::scan_builds_source_manga_chapter_tree` (temp tree) |
| TST-38 | `index::renew_interval_one_hour` (with a fake clock) |
| TST-39 | `index::timeout_keeps_previous_state` (EC-82) |
| TST-40 | `index::corrupt_file_is_deleted_and_rebuilt` (EC-81) |
| TST-41 | `index::case_insensitive_source_match` (EC-84) |

---

## 6. Store and pending deletes (`store.rs`, `pending.rs`)

| ID | Scenario | Required |
|---|---|---|
| EC-90 | Add the same chapter twice to pending | one row (`INSERT OR IGNORE`) |
| EC-91 | Add chapters, then `take_all` | rows returned, table empty |
| EC-92 | Crash between read and delete in `take_all` | single transaction: both or neither |
| EC-93 | Pending row for a manga that no longer exists | dropped at flush, no error |
| EC-94 | Bookmarked chapter enqueued with `remove_bookmarked` off | not enqueued (filter at enqueue) |
| EC-95 | Excluded category, read chapter | not enqueued |
| EC-96 | Queue table row whose chapter was deleted | `ON DELETE CASCADE` removes it |
| EC-97 | Queue `position` gaps after a cancel | renumbered densely in the same transaction |

Tests:

| ID | Test |
|---|---|
| TST-42 | `store::replace_all_is_one_transaction` (inject failure at row 3; table unchanged) |
| TST-43 | `store::positions_are_dense` (EC-97) |
| TST-44 | `pending::add_is_idempotent` (EC-90) |
| TST-45 | `pending::take_all_is_atomic` (EC-92, with a fault injection hook) |
| TST-46 | `pending::exclusion_applied_at_enqueue` (EC-94, EC-95) |

---

## 7. Deletion and rename (`manager.rs`)

| ID | Scenario | Required |
|---|---|---|
| EC-98 | Delete the last chapter of a manga | manga folder removed, index updated, source folder removed if empty |
| EC-99 | Delete a CBZ chapter | `.cbz` file deleted |
| EC-100 | Delete a chapter that is queued | removed from queue first (pause, remove, restart) |
| EC-101 | Delete a manga whose source folder has other manga | source folder kept |
| EC-102 | Rename source (case only: `mangadex (en)` to `MangaDex (EN)`) | two-step rename via `_tmp` |
| EC-103 | Rename chapter, CBZ | new name ends `.cbz`; index bare name |
| EC-104 | Rename chapter target name already exists | rename fails, logged, old kept |
| EC-105 | Chapter name change from sync when the chapter is not downloaded | no-op |
| EC-106 | Pending deletes for a chapter that is in a excluded category at flush time | Kotlin: deleted (second pass cannot see read flag). Rust: Proposal re-check the exclusion at flush using the live category list |

Tests:

| ID | Test |
|---|---|
| TST-47 | `manager::delete_last_chapter_removes_manga_and_source_dirs` |
| TST-48 | `manager::rename_source_case_only_two_step` |
| TST-49 | `manager::rename_chapter_cbz_index_bare_name` |
| TST-50 | `manager::delete_while_downloading_pauses_first` |

---

## 8. Notifications (`notify.rs`)

| ID | Scenario | Required |
|---|---|---|
| EC-107 | `hide_notification_content` on | progress title "Downloading (x/y)", no manga or chapter name |
| EC-108 | Manga title 30 characters | title chopped to 14 characters plus "…" (15 total) before the chapter prefix strip (defect kept), or strip first and then chop (Proposal) |
| EC-109 | Chapter name starts with manga title (case-insensitive) | title prefix removed from the chapter part |
| EC-110 | Error with manga but no chapter | text "Title: null" in Kotlin; Rust prints "Title" |
| EC-111 | Warning timeout 30 s | `expire-timeout` set |
| EC-112 | Pause from notification | `Command::Pause` |
| EC-113 | Notification sent with no display server (CI) | no crash; `Notifier` returns `Err` logged at debug |

Tests:

| ID | Test |
|---|---|
| TST-51 | `notify::progress_title_rules` (EC-107 to EC-109) |
| TST-52 | `notify::error_title_without_chapter` (EC-110) |

---

## 9. UI (view model, see ui-parity section 8)

Covered by the seven UI checks in `p1-ui-parity.md` section 8.

---

## 10. Failure modes (what the user sees)

| ID | Failure | Kotlin | Rust |
|---|---|---|---|
| FM-01 | Downloads folder missing or not writable | "Invalid location: <path>" notification, halt run (4 and 6 step 1) | same message key; halt run; the queue stays intact |
| FM-02 | Disk under 200 MiB | "Couldn't download due to low storage space" per chapter, continue with next chapters | same; each chapter fails quickly |
| FM-03 | Source throws on page list | the message (raw) on the error notification | same, raw message |
| FM-04 | HTTP 403 on every image | error notification per page, chapter ERROR | same; one notification per chapter (Proposal, collapse per chapter) |
| FM-05 | App closed while downloading | Android kills the service; next start restores queue | window closed: the hold keeps running; process killed: next start restores queue |
| FM-06 | Power loss mid-write | `.tmp` leftovers, resumed on next run | same |
| FM-07 | Index file corrupt | silently rebuilt | same |
| FM-08 | `hide_notification_content` changed during a run | takes effect next update | same |
| FM-09 | Disk full during archive | `.cbz_tmp` left; chapter ERROR | `.cbz.part` removed; chapter ERROR |
| FM-10 | Two app instances | not handled (Android single process) | single instance from area:01 (`SingleInstance`) |

---

## 11. Race conditions (catalogue)

| ID | Race | Kotlin protection | Rust protection |
|---|---|---|---|
| RC-06 | Queue mutated from UI while job thread reads it | `MutableStateFlow.update` (CAS, side effects repeated on retry) | single actor owns state (rust design 2) |
| RC-07 | `pendingDeleter` read while reader writes | `@Synchronized` | actor-owned table, transaction |
| RC-08 | Index read while scan rewrites it | `Mutex` for writes, unsynchronised reads of the map reference | `RwLock` |
| RC-09 | `isRunning` polled by job while downloader stops | busy loop (defect) | `watch` channel |
| RC-10 | Restore racing with an enqueue from the UI at startup | none (restore is `launchNow` async) | enqueue waits for restore to finish (actor starts with restore) |
| RC-11 | Deleting a manga folder while its chapter writes | none (defect) | pause first (EC-22) |
| RC-12 | Two downloads of the same folder name (collision) | none | detected at enqueue (Proposal) |
| RC-13 | Index updated while folder deleted | none | index update in same actor step |

---

## 12. Fixtures

| Fixture | Content | Used by |
|---|---|---|
| `fixtures/img/tiny.jpg` | 8x8 JPEG, q75 | EC-38 to EC-44, TST-21 |
| `fixtures/img/tiny.png`, `tiny.webp`, `tiny.gif` | 8x8 each | TST-21 |
| `fixtures/img/animated.gif` | 2 frames, 16x16 | TST-29 |
| `fixtures/img/tall_800x4000.png` | 800 x 4000 PNG | TST-22, TST-28 |
| `fixtures/img/broken.jpg` | JPEG header then garbage | EC-56 |
| `fixtures/index/tree/` | source dirs `MangaDex (EN)`, `mangadex (en)`, `Unknown (XX)`; manga dirs with folders and `.cbz` | TST-37, TST-41 |
| `fixtures/index/corrupt.bin` | 16 random bytes | TST-40 |
| `fixtures/comicinfo/golden.xml` | expected XML for manga "Test: Manga/1", chapter 12.5 | TST-31 |
| `fixtures/db/queue_v1.sql` | SQLite with `chapters`, `mangas` (area:03 DDL) and the two new tables | TST-42 to TST-46 |
| FakeSource | in-process HTTP server (`tokio::net::TcpListener`) with scripted routes: `/ok/{n}`, `/status/{code}`, `/slow/{n}` (delays), `/len-unknown`, `/huge-length` | TST-14 to TST-20, TST-24, TST-33 |
| FakeClock | `tokio::time::pause()` | TST-17, TST-24 |
| FakeConnectivity | `watch::Sender<Connectivity>` | TST-16 |

Fixture creation: one script `kuro-downloads/testdata/make_fixtures.rs` (run once, outputs committed). No binary files are generated at test time, except the 1000-page synthetic chapter, which is created in a temp dir with 1000 tiny JPEG copies.

---

## 13. Test list summary

| Group | Count | IDs |
|---|---|---|
| Queue | 13 | TST-01 to TST-13 |
| Runtime | 7 | TST-14 to TST-20 |
| Pipeline and images | 13 | TST-21 to TST-33 |
| Naming | 3 | TST-34 to TST-36 |
| Index | 5 | TST-37 to TST-41 |
| Store and pending | 5 | TST-42 to TST-46 |
| Deletion and rename | 4 | TST-47 to TST-50 |
| Notifications | 2 | TST-51 to TST-52 |

Each test is a `#[test]` or `#[tokio::test]` in the crate that owns the function. Golden outputs live under `testdata/` and are regenerated only by an explicit `UPDATE_GOLDEN=1` run.

## 14. Coverage target

- Pure logic (`queue`, `scheduler`, `sanitize`, `layout`, `images::split_geometry`, `throttle`, `comicinfo`): 90 percent line coverage.
- Pipeline and manager: scenario tests above; no line target.
- The GTK view model is tested through the checks in ui-parity section 8, not in unit tests.
