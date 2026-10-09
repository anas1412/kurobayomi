# Area 23 — Manga downloads: implementation tasks

Every task traces to behaviour sections in `p1-behavior.md`, data sections in `p1-data-and-config.md`, the design in `p1-rust-design.md`, UI in `p1-ui-parity.md`, and tests in `p1-edge-cases-and-tests.md`. Paths in Sources are repo-relative to `/home/blackbox/aniyomi-src`. Priorities follow the project rules: P0 = needed to read downloaded manga offline and run the queue daily; P1 = download parity; P2 = polish; P3 = dropped or obsolete (each still has a task).

Shorthand used in Sources: `DL` = `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt`, `MGR` = `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt`, `CACHE` = `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt`, `PROV` = `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadProvider.kt`, `STORE` = `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadStore.kt`, `DEL` = `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadPendingDeleter.kt`, `JOB` = `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadJob.kt`, `NTF` = `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadNotifier.kt`, `MODEL` = `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt`, `QVM` = `app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreenModel.kt`.

---

## Foundation

### T-23a-001 — Crate skeleton for kuro-downloads and module layout
- Priority: P0
- Effort: S
- Depends: T-01a-001, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt
- Acceptance:
  - `crates/kuro-downloads` is a workspace member and builds with no GTK dependency (`cargo tree -p kuro-downloads` shows no gtk4 or glib).
  - Modules listed in `p1-rust-design.md` section 1 exist as files with doc comments and compile.
  - The crate re-exports `DownloadManager`, `DownloadEvent`, `DownloadState`, `DownloadError`.

### T-23a-002 — Download model types: state, page state, page, queued download
- Priority: P0
- Effort: S
- Depends: T-23a-001, area:15-source-api-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt
- Acceptance:
  - `DownloadState` has `#[repr(u8)]` values NotDownloaded=0, Queue=1, Downloading=2, Downloaded=3, Error=4 (unit test asserts each value).
  - `is_active()` is true exactly for 0, 1, 2.
  - `PageState` variants are Queue, LoadPage, DownloadImage, Ready, Error; `total_progress` is a sum and `downloaded_images` counts Ready pages.

### T-23a-003 — Name sanitiser: buildValidFilename port and reserved-name rule
- Priority: P0
- Effort: S
- Depends: T-23a-001, area:13-core-util-storage-archive-torrent
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt, addenda/requirements-cross-platform.md
- Acceptance:
  - Cases EC-71 to EC-75 in `p1-edge-cases-and-tests.md` produce the listed outputs (TST-34).
  - Output for every title without reserved names equals the Kotlin output byte for byte.
  - Decision recorded in the crate README: the `ntfs-names` feature (default on) appends `_` to CON, PRN, AUX, NUL, COM1-9, LPT1-9 (EC-76).

### T-23a-004 — Directory naming: source, manga, chapter, valid chapter names
- Priority: P0
- Effort: S
- Depends: T-23a-003, area:15-source-api-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadProvider.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - Source folder name is `"<name> (<LANG>)"` sanitised, for example `MangaDex (EN)`.
  - Chapter folder name is `"<scanlator>_<name>"` when the scanlator is non-blank, else `"<name>"`, blank name becomes `Chapter`, sanitised (TST-35).
  - `valid_chapter_names` returns the folder name then `<folder>.cbz` (TST-36).

### T-23a-005 — Local lookup of manga, source and chapter folders
- Priority: P0
- Effort: S
- Depends: T-23a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadProvider.kt
- Acceptance:
  - `find_chapter` returns the folder or the `.cbz` file, folder first; returns None when the manga folder is missing.
  - `find_chapter_dirs` for a list of chapters returns the manga folder and the found entries, and `(None, [])` when the manga folder does not exist.
  - Lookup does not create folders (creation is only in `manga_dir`).

### T-23a-006 — Download index: scan, hourly renew, disk snapshot, invalidation
- Priority: P0
- Effort: M
- Depends: T-23a-005, area:17-source-app-layer, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt
- Acceptance:
  - A scan of a fixture tree (TST-37) produces source, manga and chapter names; folders ending `_tmp` are skipped; `.cbz` contributes its stem; unknown source folders are ignored.
  - Renew happens at most once per hour (TST-38); `invalidate` forces a rescan.
  - Source folder matching is case-insensitive (TST-41).
  - The snapshot file is written 1 s after the last change, is `download-index-v1.bin` in the cache directory, and a corrupt file is deleted and rebuilt (TST-40).
  - When the source gate times out after 30 s, the previous index is kept (TST-39). Kotlin replaces it with an empty index (defect).

### T-23a-007 — Index mutations and index-backed queries
- Priority: P0
- Effort: S
- Depends: T-23a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt
- Acceptance:
  - `add_chapter`, `remove_chapter`, `remove_manga`, `remove_source` update the tree without a rescan.
  - `is_chapter_downloaded` matches folder or `.cbz` by any valid chapter name.
  - Counts: total chapters, chapters per manga, total size (recursive), size per manga (recursive, spawn_blocking).
  - Index entries store bare names (no `.cbz`), including after a rename (TST-49).

### T-23a-008 — Queue state machine with status invariants
- Priority: P0
- Effort: M
- Depends: T-23a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - `add_all`, `remove_chapters`, `remove_manga`, `clear`, `pause_running`, `stop_running`, `reset_for_start` implement the transition table in `p1-behavior.md` section 1.1 (TST-01 to TST-05).
  - `reorder` validates that the id list equals the queue (TST-09) and keeps ERROR (TST-10, Proposal change).
  - All mutations are pure functions on `QueueState` with no I/O.

### T-23a-009 — Persistent queue store (SQLite) with restore order
- Priority: P0
- Effort: S
- Depends: T-23a-008, area:03-db-manga-schema-migrations, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadStore.kt
- Acceptance:
  - Tables `manga_download_queue` and `manga_pending_delete` from `p1-data-and-config.md` section 3.1 are created by a migration that area:03-db-manga-schema-migrations numbers.
  - `replace_all` is one transaction (TST-42); `position` values stay dense (TST-43).
  - Rows are loaded `ORDER BY position`; no SharedPreferences are read.

### T-23a-010 — Per-source scheduler: 5 sources, one chapter per source, skip ERROR
- Priority: P0
- Effort: M
- Depends: T-23a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - `pick_active` returns at most 5 chapters, one per source, first-occurrence order (TST-06).
  - An ERROR chapter is skipped and its source's next chapter becomes active (TST-07).
  - `reconcile` aborts tasks that leave the active set (with status fixed, see T-23a-011) and starts tasks for new ones.

### T-23a-011 — Downloader lifecycle: start, pause, stop, clear, warning stop
- Priority: P0
- Effort: M
- Depends: T-23a-001, T-23a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt
- Acceptance:
  - Command table in `p1-rust-design.md` section 4.1 is implemented; `pause` is `pause_running` then `stop`.
  - `stop(reason)` clears the paused flag and releases the hold, unlike Kotlin (TST-18 covers the race; EC-17 covers the reason path).
  - Start on an empty or running queue is a no-op (EC-18, EC-19).
  - After a completed run, `Complete` is emitted once; after a pause with items, `Paused` is emitted.

### T-23a-012 — Chapter pipeline: precheck, tmp folder, page list, finalise hooks
- Priority: P0
- Effort: M
- Depends: T-23a-011, area:12-network-stack, area:15-source-api-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - Free space below 200 MiB (209715200 bytes) sets ERROR with "Couldn't download due to low storage space" and does not throw (FM-02; behaviour section 6 step 2).
  - Empty page list raises "No pages found" and sets ERROR (EC-37).
  - Page indices are reassigned from 0 (`Page(i, url, imageUrl, uri)`).
  - `*.tmp` files in the `_tmp` folder are deleted before fetching; finished images are kept (TST-33).
  - The `_tmp` folder is created before the page list is fetched (Kotlin order, observable in the file system).

### T-23a-013 — Page image fetch: concurrency 2, retries 2/4/8 s, progress
- Priority: P0
- Effort: M
- Depends: T-23a-012, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - At most 2 pages per chapter are in flight (TST-14).
  - A failing request is retried after 2 s, then 4 s, then 8 s; the fourth failure is final (TST-24, EC-40, EC-41).
  - Progress is `100 * read / content_length`, `-1` when the length is unknown, reset to 0 at start and 100 on READY.
  - Decision: HTTP 404 and 410 are not retried (Proposal; recorded in the code comment with the Kotlin reference).

### T-23a-014 — Image file naming, extension resolution and resume
- Priority: P0
- Effort: S
- Depends: T-23a-013, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt
- Acceptance:
  - File stem is the page number zero-padded to `max(3, digits(page count))` (TST-25).
  - Extension: Content-Type mapping, then byte sniffing, then `jpg` (EC-42 to EC-44, TST-21). Sniffing wins when it disagrees with the header (Proposal).
  - Existing finished file `stem.*` or `stem__001*` is reused without download (EC-45).
  - Zero-length files are treated as missing (EC-46, Proposal).

### T-23a-015 — Shared download speed limit
- Priority: P1
- Effort: S
- Depends: T-23a-013, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - Setting `download_speed_limit` (KiB/s, default 0) is read at each chapter start and multiplied by 1024.
  - One limiter is shared by all concurrent page downloads of the downloader (TST-26).
  - 0 means unlimited; 1 MiB at 512 KiB/s takes about 2 s (TST-26).

### T-23a-016 — Reuse of reader page cache images
- Priority: P1
- Effort: S
- Depends: T-23a-013, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt
- Acceptance:
  - `PageImageCache` trait is implemented by area:34-images-covers-cache; a hit copies the file and deletes the cache entry after a successful rename (EC-47).
  - A hit with an unrecognised type falls back to the network instead of leaving `.tmp` (EC-48, TST-27).

### T-23a-017 — Data-saver proxy in the downloader
- Priority: P1
- Effort: S
- Depends: T-23a-013, area:26-library-update-jobs, area:17-source-app-layer
- Sources: app/src/main/java/aniyomi/util/DataSaver.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Data saver is used only when `data_saver_downloader` (default true) is on and `data_saver` is not NONE (default NONE) and the source is not in `data_saver_excluded`.
  - The three services (Bandwidth Hero, wsrv.nl, resmush.it) build the URLs in `p1-data-and-config.md` section 7.
  - resmush.it parses JSON with `serde_json` over HTTPS or is removed; the decision is recorded (area:26-library-update-jobs owns the final call).

### T-23a-018 — Tall-image split with prefix fix and integer threshold
- Priority: P1
- Effort: M
- Depends: T-23a-014, area:13-core-util-storage-archive-torrent
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - Split only when `h / w > 3` with integer division (so h >= 4w) and the image is not animated (EC-50, EC-51, EC-54).
  - `split_geometry` matches the formula in behaviour 10.7 (TST-22).
  - Parts are `prefix__NNN.jpg` with JPEG quality 100, the original is removed only after all parts are written (TST-28).
  - The prefix uses the same width as the page file name; page 1 in a 1000-page chapter never matches page 10 (TST-25, EC-55).
  - Optimal height is `2 * max(monitor width, height)` from GDK, default 4096 without a display (Proposal).
  - Memory: images whose decoded size exceeds 256 MiB are not split and a debug log line says so (Proposal, pitfall 10).

### T-23a-019 — Downloader notifications: progress, paused, complete, warning, error
- Priority: P0
- Effort: M
- Depends: T-23a-011, area:53-platform-manifest-intents-workers
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt
- Acceptance:
  - Progress notification text "Downloading (x/y)" and title "<manga> - <chapter>" chopped to 30 characters (TST-51); manga title chopped to 15 before the chapter prefix strip (defect kept or fixed as recorded in TST-51).
  - Paused notification has title "Paused", text "Chapter download paused", actions Resume and Cancel all.
  - Warning and error use the low-importance class (no sound) and error text falls back to "Could not download chapter due to unexpected error" (TST-52).
  - Error notification title "<manga>: <chapter>" also when chapter is missing without printing "null" (EC-110).
  - Replacement by id: progress -> paused -> complete keeps one progress notification (pitfall 15).

### T-23a-020 — Integrity check and failure classification
- Priority: P0
- Effort: S
- Depends: T-23a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - A chapter is complete when every page is READY and the folder holds exactly one page key per page (`NNN` or `NNN__001`), ignoring ComicInfo.xml, `.nomedia`, `.tmp` (TST-23, EC-60, EC-61).
  - Failure after the check leaves the chapter ERROR, not DOWNLOADED, and emits one error event per failed page (EC-39, FM-04).

### T-23a-021 — ComicInfo.xml writer
- Priority: P1
- Effort: M
- Depends: T-23a-012, area:14-source-local-and-metadata, area:27-tracking-core, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt, core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt
- Acceptance:
  - Element order and namespaces match `p1-data-and-config.md` section 2.4; golden file test passes (TST-31).
  - Web is tracker remote URLs (non-blank, trimmed) plus the chapter URL, distinct, joined by a space.
  - Number formatting and status mapping match (TST-32, EC-67).
  - Empty categories omit `Categories` (EC-64).

### T-23a-022 — Finalisation: CBZ writer or folder rename, index add
- Priority: P1
- Effort: M
- Depends: T-23a-020, T-23a-021, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, core/archive/src/main/kotlin/mihon/core/archive/ZipWriter.kt
- Acceptance:
  - CBZ is ZIP with STORE, UTF-8 names, flat entries, written to `<name>.cbz.part`, renamed to `<name>.cbz`, and the `_tmp` folder removed (TST-30).
  - Folder mode renames `_tmp` to the chapter folder and checks the result; a failed rename fails the chapter (EC-63).
  - Index gets the bare name (TST-49 covers rename; this task covers add).
  - Setting `save_chapter_as_cbz` (default true) selects the mode.

### T-23a-023 — Background running, network gating, no busy-wait
- Priority: P0
- Effort: M
- Depends: T-23a-011, area:01-foundation-app-bootstrap, area:53-platform-manifest-intents-workers
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadJob.kt
- Acceptance:
  - Downloads run while the window is closed via `SchedulerBackend::hold` (GApplication hold) and stop when the queue ends.
  - Connectivity uses `gio::NetworkMonitor`; metered counts as "not Wi-Fi" when `pref_download_only_over_wifi_key` (default true) is on (table in rust design 5.2).
  - No polling loop: TST-17 shows zero wakeups while idle.
  - Offline stops with "No network connection available"; metered with the preference on stops with "No Wi-Fi connection available" (EC-33, EC-34).
  - Regaining connectivity restarts automatically only when the user did not pause (EC-31, EC-32, Proposal).

### T-23a-024 — Enqueue API: filters, source order, auto-start, warnings
- Priority: P0
- Effort: M
- Depends: T-23a-008, T-23a-019, area:15-source-api-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - Filters: already downloaded (folder or cbz), already queued, non-HTTP source (EC-02, EC-03, EC-08).
  - Order: descending `source_order` (TST-12).
  - Auto-start only when the queue was empty and `auto_start` is true (EC-04, EC-05).
  - Warnings at more than 30 queued (non-unmetered) or more than 15 from one source, with 30 s timeout and help URL (EC-06, EC-07, TST-13).

### T-23a-025 — Queue ordering API: start now, add to front, reorder, move, sort
- Priority: P1
- Effort: M
- Depends: T-23a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreenModel.kt
- Acceptance:
  - `start_now` moves a queued chapter to index 0 or inserts a new one at index 0 and starts (EC-09, EC-10).
  - `add_to_front` prepends and starts when not running (EC-11).
  - Move to top / bottom within group, move series to top / bottom, and the four sort options rewrite the order (ui-parity 1.4 and 1.5).
  - Drag rules: only within the same group (ui-parity 2).
  - Reorder while running pauses and restarts (EC-14).

### T-23a-026 — Queue cancellation API
- Priority: P1
- Effort: S
- Depends: T-23a-008, T-23a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - `cancel` removes chapters, sets DOWNLOADING or QUEUE to NOT_DOWNLOADED, and leaves the `_tmp` folder (EC-21, TST-19).
  - `cancel_manga` pauses first when the manga has a running chapter (EC-22, RC-11).
  - Cancel all for a series removes exactly the rows with the same manga id.

### T-23a-027 — Deletion API: chapters, manga, empty-folder cleanup
- Priority: P0
- Effort: M
- Depends: T-23a-007, T-23a-026, area:25-downloads-shared, area:10-app-domain-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt
- Acceptance:
  - `delete_chapters` applies the bookmark and excluded-category filters (EC-94, EC-95), removes from queue (pause, remove, restart), deletes folders or `.cbz` files, updates the index, and deletes the manga folder when empty (EC-98, EC-99).
  - `delete_manga` deletes the manga folder, updates the index, and removes the source folder when empty (EC-101).
  - Both pause first when a running chapter is affected (RC-04, EC-100).
  - Filter settings: `remove_exclude_categories` (default empty), `pref_remove_bookmarked` (default false).

### T-23a-028 — Pending deferred deletion on reader exit
- Priority: P1
- Effort: M
- Depends: T-23a-027, area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadPendingDeleter.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - `add` uses `INSERT OR IGNORE` and returns the number of new rows (TST-44, EC-90).
  - `take_all` reads and deletes in one transaction (TST-45, EC-92).
  - Filtering happens at enqueue time (TST-46); the flush re-checks exclusions against live categories (Proposal, EC-106).
  - The reader calls `flush_pending_deletes` on exit; rows for missing manga are dropped (EC-93).

### T-23a-029 — Rename API: source folder, chapter folder
- Priority: P1
- Effort: S
- Depends: T-23a-007, area:10-app-domain-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt
- Acceptance:
  - Case-only source rename goes through `_tmp` (EC-102, TST-48).
  - Chapter rename updates the index with the bare name and keeps `.cbz` on the file (EC-103, TST-49).
  - A target that exists fails the rename and keeps the old folder (EC-104).

### T-23a-030 — Manager facade for other areas
- Priority: P0
- Effort: S
- Depends: T-23a-011, T-23a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt
- Acceptance:
  - Public API matches `p1-rust-design.md` section 7 (enqueue, start, pause, clear, queued lookup, snapshot, events, counts, sizes, deletes, renames, page list).
  - `queued(chapter_id)` returns the queued item or None without blocking (replaces `runBlocking`).
  - Events are delivered on a broadcast channel with capacity 64; a slow subscriber gets `Lagged` and resynchronises from `snapshot()`.

### T-23a-031 — Reading downloaded chapters: build the page list
- Priority: P0
- Effort: S
- Depends: T-23a-005, T-23a-014, area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt
- Acceptance:
  - Lists image files in the chapter folder, filters by extension or sniffing, sorts by name, and returns pages with READY status (behaviour section 9).
  - Empty result raises "No pages found" (EC-37 message).
  - A `.cbz` chapter returns an archive-backed page list (Proposal; Kotlin throws, see behaviour 9).

---

## Queue screen

### T-23a-032 — Queue view model: groups, counts, actions, subscriptions
- Priority: P1
- Effort: M
- Depends: T-23a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHeaderItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadItem.kt
- Acceptance:
  - Groups by source in first-occurrence order with counts; rows keep queue order (ui-parity 1.3).
  - Row equality uses the full 64-bit chapter id (behaviour 1.6, Proposal).
  - Progress updates are coalesced to 50 ms (RC-05); status changes update the row without a full rebuild.
  - Actions call the manager; the view waits for `QueueChanged` before redrawing (ui-parity 7).

### T-23a-033 — Queue screen UI in GTK
- Priority: P1
- Effort: L
- Depends: T-23a-032, area:36-ui-shell-navigation-security-webview, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadAdapter.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHeaderHolder.kt
- Acceptance:
  - Replaces `fn downloads()` manga branch in `src/screens.rs` (ui-parity section 6 gaps 1 to 9 closed).
  - Empty state text "No downloads" (`information_no_downloads`); top bar sort and cancel-all hidden when empty.
  - Row layout: drag handle, title, chapter, progress text, bar, menu; header "<name> (<count>)" with drag handle.
  - FAB reads Pause or Resume from the manager state (ui-parity 1.6).
  - Narrow layout rules from ui-parity section 4 pass the check at 360 px width.

### T-23a-034 — Progress and status throttling for the UI
- Priority: P1
- Effort: S
- Depends: T-23a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt
- Acceptance:
  - Progress events per chapter are at most one per 50 ms (TST-20).
  - Row progress bar uses the sum of page progress with maximum `pages * 100` (behaviour 14.6).
  - Progress job is cancelled on DOWNLOADED and ERROR (EC behaviour 14.6).

---

## Settings and startup

### T-23a-035 — Startup restore gated on sources; no silent drops
- Priority: P0
- Effort: S
- Depends: T-23a-009, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadStore.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - Restore runs after the source gate opens (RC-10).
  - Rows whose source is not yet registered are kept, not deleted (defect fix, behaviour 2.1).
  - Rows whose manga or chapter is gone are deleted and logged at info.
  - Restored rows are QUEUE; DOWNLOADING is never restored.

### T-23a-036 — Test suite for the manga download core
- Priority: P0
- Effort: M
- Depends: T-23a-006, T-23a-008, T-23a-010, T-23a-018, T-23a-020, T-23a-021, T-23a-022
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadProvider.kt
- Acceptance:
  - Every TST id in `p1-edge-cases-and-tests.md` section 13 exists and passes (TST-01 to TST-52).
  - Fixtures listed in section 12 are committed under `crates/kuro-downloads/testdata/`.
  - `cargo test -p kuro-downloads` runs without network access and without a display.

### T-23a-037 — Download settings consumed here and dead setting resolution
- Priority: P1
- Effort: S
- Depends: T-23a-015, area:02-preferences-system, area:25-downloads-shared
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt
- Acceptance:
  - Every key and default in `p1-data-and-config.md` section 5 is registered in the Rust settings with the same default.
  - `download_slots` (default 1) is not read by the manga downloader in Kotlin: the Rust decision is recorded. Proposal: rename to `downloads.max_parallel_chapters` (default 5), capped at the scheduler limit, or remove the control.
  - Settings UI labels reuse the keys "Maximum downloads" (`pref_download_slots`) and "Chapter download speed limit" (`download_speed_limit`).

### T-23a-038 — Drop: Android-only machinery
- Priority: P3
- Effort: S
- Depends: T-23a-023, area:53-platform-manifest-intents-workers
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadAdapter.kt
- Acceptance:
  - Dropped with reason: WorkManager, ForegroundInfo and foreground service types, FlexibleAdapter, Voyager ScreenModel, Compose AndroidView, MediaScannerConnection, SAF or UniFile document trees. What the user loses: nothing visible, because the replacements are in T-23a-023 and T-23a-033.
  - The decision is written as a comment in the crate's `lib.rs` and in this task's PR description.

### T-23a-039 — Drop: migration of Android download state
- Priority: P3
- Effort: S
- Depends: T-23a-009, area:11-app-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadStore.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadPendingDeleter.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt
- Acceptance:
  - Dropped: SharedPreferences `active_downloads`, `chapters_to_delete`, and `dl_index_cache_v3`. What the user loses: the Android queue and pending deletes (they are short-lived). Kept: the downloaded folders themselves are reused (data file section 6).
  - A Linux install with a copied Aniyomi download folder shows those chapters as downloaded after the first index scan (TST-37 with an Aniyomi fixture).

---

## Polish and long tail

### T-23a-040 — Index indexing indicator
- Priority: P2
- Effort: S
- Depends: T-23a-006, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - A status-bar or page-subtitle text "Checking downloads" (`download_notifier_cache_renewal`, base strings of area:51-i18n) appears only when a scan lasts longer than 1 s (EC-80).
  - The indicator is hidden when the scan ends.

### T-23a-041 — Logging with tracing targets
- Priority: P2
- Effort: S
- Depends: T-23a-001, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt
- Acceptance:
  - Errors are logged with target `kuro_downloads` at error level with the full chain; split failures at error; index scan at debug.
  - No user-visible text is taken from log output.

### T-23a-042 — Local source counts and sizes
- Priority: P2
- Effort: S
- Depends: T-23a-007, area:14-source-local-and-metadata
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt
- Acceptance:
  - For the local source, `chapter_count_for_manga` counts directories and archives in the local manga folder (behaviour 3.1); size is recursive.
  - Other sources use the index.

### T-23a-043 — Hide notification content setting
- Priority: P2
- Effort: S
- Depends: T-23a-019, area:02-preferences-system
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadNotifier.kt
- Acceptance:
  - Setting `hide_notification_content` (default false) switches the progress title to "Downloading (x/y)" with no manga or chapter names (EC-107).
  - Change applies to the next notification update.

### T-23a-044 — Clean stray tmp folders and archive parts at startup
- Priority: P1
- Effort: S
- Depends: T-23a-012, T-23a-022
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - At startup, `_tmp` folders with no queue row, and `.cbz.part` files, are removed; `_tmp` folders for queued chapters are kept for resume (EC-62, FM-06).
  - Behaviour is logged at info with the count removed.

### T-23a-045 — Index invalidation on external file changes
- Priority: P1
- Effort: S
- Depends: T-23a-006, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt
- Acceptance:
  - A `notify` watcher on the downloads root invalidates the index after a 2 s quiet period (Proposal; Kotlin reacts to the storage-root change only, not to folder changes).
  - The hourly renew remains as the fallback when the watcher cannot start (EC-89 covers root changes).
