# 43-reader-core: implementation tasks

Format per `plans/_tools/tasks.py`. Paths are repo-relative to `/home/blackbox/aniyomi-src` (Kotlin reference). Rust targets are under `/home/blackbox/kurobayomi/src/reader/` (see `p1-rust-design.md` section 1). Priorities: P0 needed for daily reading of manga from one working source; P1 parity features; P2 polish; P3 Android-only with a replacement or drop decision.

Shorthands in the Sources lines are NOT used; every path is written out.

## Foundations (models, flags, loaders)

### T-43a-001 — Reader domain model (chapter, page, viewer chapters, inserted page)
- Priority: P0
- Effort: M
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ReaderChapter.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ReaderPage.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/InsertPage.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ViewerChapters.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt
- Acceptance:
  - `ChapterState` has exactly `Wait`, `Loading`, `Error(e)`, `Loaded(pages)`; `pages` is non-null only in `Loaded`.
  - `PageStatus` has exactly `Queued, LoadPage, DownloadImage, Ready, Error` in that order; the initial value is `Queued`.
  - `page.number` equals `index + 1` in every UI label; `index` is 0-based and contiguous after loading.
  - An `InsertPage` created from a `Ready` parent is `Ready` and shares its stream; it never writes progress.

### T-43a-002 — Chapter reference counting and atomic chapter-set replacement
- Priority: P0
- Effort: S
- Depends: T-43a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ReaderChapter.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ViewerChapters.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - `release()` resets the state to `Wait` and recycles the loader only when the count reaches exactly zero.
  - Replacing the chapter set adds references for the new set before releasing the old set; a chapter present in both sets is never recycled (test `viewer_chapters_replace_keeps_shared`).
  - A negative count asserts in debug builds (E-REF-1).

### T-43a-003 — Chapter transition model with direction-aware equality
- Priority: P0
- Effort: S
- Depends: T-43a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/model/ChapterTransition.kt
- Acceptance:
  - `Prev` and `Next` with the same pair are not equal to each other (E-TR-1).
  - `Prev(a,b) == Prev(a,b)` and the hash is consistent with equality.
  - The transition carries `from` (non-null) and `to` (nullable) as in the Kotlin model.

### T-43a-004 — Reading-mode and orientation flags with mask-merge writes
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt, app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetMangaViewerFlags.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt, domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt
- Acceptance:
  - Reading mode values 0..5 with mask `0x07`; orientation values `0x00,0x08,0x10,0x18,0x20,0x28,0x30` with mask `0x38`.
  - Decoding an unknown value (for example `0x06`, `0x38`) yields `DEFAULT` (test `reading_mode_fromPreference_unknown_is_default`).
  - A write keeps all bits outside the mask (test `mask_set_flag_preserves_other_bits`).
  - Orientation enum keeps `stringRes` keys and the `flagValue` table of `p1-behavior.md` section 1.6, even though the Linux port does not apply the lock (T-43a-068).

### T-43a-005 — Page loader abstraction (enum dispatch and recycle-on-drop)
- Priority: P0
- Effort: M
- Depends: T-43a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/PageLoader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ArchivePageLoader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/DirectoryPageLoader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/EpubPageLoader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/DownloadPageLoader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt
- Acceptance:
  - `Loader` is an enum with `Directory, Archive, Epub, Download, Http`; `is_local` is false only for `Http`.
  - `Drop` performs the recycle side effects once; double recycle is a no-op.
  - `load_page` returns a guard; dropping the guard cancels only the entries it queued that are still `Queued`.

### T-43a-006 — Chapter loading state machine
- Priority: P0
- Effort: M
- Depends: T-43a-005, T-43a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ChapterLoader.kt
- Acceptance:
  - A loaded chapter with a loader is a no-op (`chapterIsReady`).
  - A failed page list leaves no loader attached (E-LEAK-1); a retry drops nothing that is still attached (E-LEAK-2).
  - Cancellation returns the chapter to `Wait`, not `Error` (E-CANCEL-1).
  - An empty page list yields the message "No pages found" (`page_list_empty_error`).
  - Start page: unread chapter or preserve-position on uses `last_page_read`; otherwise the previous requested page is kept (E-START-1).

### T-43a-007 — Loader selection precedence
- Priority: P0
- Effort: S
- Depends: T-43a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ChapterLoader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt
- Acceptance:
  - Precedence is download (checked with skip-cache), then local format (directory, archive, epub), then HTTP source, then stub (error "Source not installed: %s"), then unsupported (error "Source not found").
  - A stub source produces `source_not_installed`; any other unknown source produces `loader_not_implemented_error`.

### T-43a-008 — Directory page loader (natural order, image filter, one level)
- Priority: P0
- Effort: S
- Depends: T-43a-005, T-43a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/DirectoryPageLoader.kt, core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - Subdirectories are ignored; only files passing the image rule are included.
  - Order is natural and case-insensitive on the file name (`10.jpg` after `2.jpg`).
  - A missing or unreadable directory yields an empty list, which becomes "No pages found".

### T-43a-009 — Archive page loader (zip/cbz with one-time entry index)
- Priority: P0
- Effort: M
- Depends: T-43a-005, T-43a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ArchivePageLoader.kt, core/archive/src/main/kotlin/mihon/core/archive/ArchiveReader.kt, core/archive/src/main/kotlin/mihon/core/archive/UniFileExtensions.kt
- Acceptance:
  - Entries are indexed once; opening a page does not rescan the archive from the beginning (E-ARC-1).
  - Only file entries that pass the image rule are listed, naturally sorted by name.
  - Opening a page after recycle fails with a typed error (Kotlin `check(!isRecycled)`).

### T-43a-010 — EPUB page loader (spine order)
- Priority: P1
- Effort: M
- Depends: T-43a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/EpubPageLoader.kt, core/archive/src/main/kotlin/mihon/core/archive/EpubReader.kt
- Acceptance:
  - Pages follow the spine order of the package document, one image per page entry, index contiguous.
  - A broken container (missing `META-INF/container.xml`) produces an archive error with a visible message.
  - Image extraction selectors match `EpubReader.kt:76-80` (verified by a fixture with two images on one page).

### T-43a-011 — Downloaded chapter loader (directory or cbz from the download store)
- Priority: P1
- Effort: M
- Depends: T-43a-005, T-43a-009, area:23-downloads-manga, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/DownloadPageLoader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadProvider.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt
- Acceptance:
  - A `.cbz` download is read through the archive loader; a directory download uses the directory semantics.
  - A missing file marks that page `Error` and does not crash the viewer (E-DL-1).
  - Download-ahead and the download-only filter use the same lookup (`skip_cache` false in lists, true in the loader).

### T-43a-012 — Image type detection, image file rule and natural name sort
- Priority: P0
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - Extensions `avif, gif, heif, jpg, jxl, png, webp` are accepted; `jpeg` and uppercase extensions are accepted only through the magic-byte check, or case-insensitively if the documented improvement is taken (E-IMG-1).
  - Natural order: `2.jpg < 10.jpg`; `PAGE 1 < Page 2 < page 10` ignoring case.
  - Magic-byte detection identifies JPEG, PNG, WebP, GIF, AVIF, HEIF, JXL without relying on the extension.

### T-43a-013 — HTTP page loader: priority queue and single worker
- Priority: P0
- Effort: L
- Depends: T-43a-005, T-43a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt
- Acceptance:
  - Priority order is higher priority first (retry 2, visible 1, preload 0) and FIFO within a priority (E-HTTP-3); test `prio_queue_order_and_fifo`.
  - One worker task processes one page at a time; the worker starts on the first `load_page`, not on `get_pages` (E-HTTP-7).
  - Preload queues at most 4 forward pages of the same chapter with priority 0; the last page queues nothing.
  - Cancelling the guard removes still-queued entries (E-HTTP-8).

### T-43a-014 — HTTP page loader: page resolution, image fetch and status transitions
- Priority: P0
- Effort: M
- Depends: T-43a-013, T-43a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt, app/src/main/java/aniyomi/util/DataSaver.kt
- Acceptance:
  - A page without an image URL goes `LoadPage` (calls the source), then `DownloadImage` only when not cached, then `Ready`.
  - The cache key is the original image URL even when the data saver rewrites the request (E-HTTP-4).
  - A non-cancellation failure sets `Error`, is logged, and the worker continues (E-HTTP-6).
  - `get_pages` first reads the page-list cache and falls back to the source; indices are rebuilt 0..n.

### T-43a-015 — HTTP page loader: retry, eviction re-queue and error recovery
- Priority: P0
- Effort: S
- Depends: T-43a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt
- Acceptance:
  - Visiting a `Ready` page whose cached image is gone re-downloads it (E-HTTP-1).
  - Visiting an `Error` page re-queues it; a manual retry on a non-error page does nothing (E-HTTP-2).
  - Opening a page stream whose cache file vanished sets the page back to `Queued` and re-enqueues it (E-IMG-3).

### T-43a-016 — Page-list and image disk cache (100 MiB, md5 keys, LRU)
- Priority: P0
- Effort: M
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FileExtensions.kt
- Acceptance:
  - Page-list key is md5 of `mangaId + url`; image key is md5 of the image URL; the cache holds at most 100 MiB and evicts least recently used first.
  - Page list is written as soon as it is fetched from the network (documented improvement over write-at-close, E-HTTP-5).
  - Writing an image uses a temp file and an atomic rename, so a crash never leaves a partial image marked as cached.

### T-43a-017 — Data saver URL rewriting (bandwidth hero, wsrv.nl, resmush.it)
- Priority: P1
- Effort: M
- Depends: none
- Sources: app/src/main/java/aniyomi/util/DataSaver.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Mode `NONE`, or a source in the excluded list, yields the original URL.
  - Bandwidth hero URL is `"$server/?jpg=$format&l=$quality&bw=$colorBW&url=<urlencoded>"`; wsrv.nl and resmush URLs follow `p1-data-and-config.md` section 5.1 exactly.
  - JPEG and GIF are skipped according to `ignore_jpeg` and `ignore_gif`.

## Chapter list, ordering and gaps

### T-43a-018 — Reader chapter list (filters, current chapter re-append, dedupe, downloaded-only)
- Priority: P0
- Effort: M
- Depends: T-43a-019, T-43a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterFilterDownloaded.kt, app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterRemoveDuplicates.kt
- Acceptance:
  - Three-arm predicate: `skip_read && read` removes; else with `skip_filtered` the manga's unread, downloaded and bookmarked filters remove; else keep (test `list_skip_filtered_*`).
  - The current chapter is re-appended before sorting when filtered out (E-LIST-1).
  - Downloaded-only removes the requested chapter when it is not downloaded and returns the "current chapter missing" error (E-LIST-2).

### T-43a-019 — Chapter sort comparators (source reversal, number, upload date, collation)
- Priority: P0
- Effort: S
- Depends: none
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt, domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt
- Acceptance:
  - Reader order is ascending for NUMBER, UPLOAD_DATE and ALPHABET; SOURCE is descending by `source_order` (E-SORT-2).
  - Unknown chapter numbers (`-1`) sort before known ones in NUMBER order.
  - ALPHABET uses locale-aware collation (ICU), not byte order (E-SORT-1).

### T-43a-020 — Skip-duplicates rule (group by chapter number)
- Priority: P1
- Effort: S
- Depends: T-43a-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterRemoveDuplicates.kt
- Acceptance:
  - Groups by chapter number; keeps the current chapter, else the same scanlator, else the first (test `list_dedupe_keeps_current_id_then_scanlator_then_first`).
  - All unknown-number chapters collapse into one entry (E-DEDUPE-1), documented in the setting's description.

### T-43a-021 — Missing chapter gap computation (ported upstream tests)
- Priority: P0
- Effort: S
- Depends: none
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt, domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt
- Acceptance:
  - All vectors from `MissingItemsTest.kt` pass: `(10,9)=0`, `(10,8)=1`, `(10,8.5)=1`, `(10,1.1)=8`, invalid numbers give 0, `missingChaptersCount([-1,1,2,2.2,4,6,10,10])=5`.
  - The card shows the warning only when the gap is greater than 0 (E-GAP-1).

### T-43a-022 — Chapter transition card (layout and strings)
- Priority: P0
- Effort: M
- Depends: T-43a-021, T-43a-061
- Sources: app/src/main/java/eu/kanade/presentation/reader/ChapterTransition.kt
- Acceptance:
  - Prev shows "Previous:" over the target and "Current:" under; Next shows "Finished:" over the current and "Next:" under the target.
  - Missing target shows "There's no previous chapter" / "There's no next chapter" in the matching slot.
  - Names are limited to 5 lines with ellipsis; scanlator to 2 lines; a downloaded chapter shows the check icon with the "Downloaded" accessible name; card max width 460 px.

## Initial load, navigation, preload, events

### T-43a-023 — Reader open: manga and chapter initialisation
- Priority: P0
- Effort: M
- Depends: T-43a-018, T-43a-006, T-43a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Opening waits for the source manager to finish initialising, then loads the manga and the requested chapter (E-INIT-1 precondition).
  - A missing manga reports "Unknown err" (E-INIT-3); a missing requested chapter reports the generic message (E-INIT-4).
  - The explicit chapter id from the caller wins over a restored id unless a crash restore applies (T-43a-039).

### T-43a-024 — Initial load error card with Retry (replaces close-and-toast)
- Priority: P0
- Effort: S
- Depends: T-43a-023, T-43a-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - A failed initial load shows the card "Failed to load pages: <message>" with a Retry button and a Close button; the reader does not close by itself (E-INIT-1).
  - Retry repeats the initial load and clears the card while loading.
  - The message text is taken from `LoadError::user_message` (no null messages, E-TOAST-1).

### T-43a-025 — Chapter switch on page change (load the new current chapter, flush history first)
- Priority: P0
- Effort: M
- Depends: T-43a-023, T-43a-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - When a page of another chapter becomes current, the previous chapter's history is flushed before the new chapter is loaded.
  - A failed switch is logged and the current chapter stays current.
  - Neighbour chapters are recalculated by position in the reader list; an unknown position yields the Kotlin neighbour rule (E-NAV-3 asserted).

### T-43a-026 — Adjacent chapter navigation (buttons and N/P keys)
- Priority: P0
- Effort: S
- Depends: T-43a-025, T-43a-053
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Next and previous are disabled at the ends and enabled otherwise.
  - A failed adjacent load shows a toast and keeps the current page (E-NAV-1); success moves to page 0 of the new chapter (E-NAV-2 default).
  - Only one adjacent load runs at a time; the navigator shows a spinner, no modal dialog.

### T-43a-027 — Cross-chapter preload request from viewers
- Priority: P0
- Effort: M
- Depends: T-43a-086, T-43a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Preload skips chapters that are loaded or loading; a downloaded online chapter is reloaded from its download (state set to `Wait` and the online loader recycled first, E-LEAK-2).
  - Preload failure is silent and leaves the chapter in `Error`, which the next visit retries (E-PRE-1).
  - On success a reload event is sent through the unbounded channel (E-EVT-1).

### T-43a-086 — Reader event channel (unbounded, ordered, UI-owned)
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Events `ReloadViewerChapters`, `PageChanged`, `SetOrientation`, `SetCoverResult`, `SavedImage`, `ShareImage`, `CopyImage` are delivered in order and never dropped (E-EVT-1).
  - The UI is the only consumer; sending never blocks the UI thread (E-UI-2).

### T-43a-087 — Viewer contract used by the reader core
- Priority: P0
- Effort: M
- Depends: area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - The core calls the viewer only through `set_chapters`, `move_to_page`, `destroy`, `handle_key`, and `handle_motion`; viewer events come back as `page_selected`, `toggle_menu`, `show_menu`, `hide_menu`, `request_preload`, `long_tap`.
  - Viewer swap destroys the previous viewer and clears the container before the new one is attached.
  - Ownership of the viewer code stays with area:44-reader-viewers; this task only defines the contract.

## Progress, completion, history, deletion, tracking

### T-43a-088 — Database writer actor (progress, bookmark, history, viewer flags)
- Priority: P0
- Effort: M
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, data/src/main/sqldelight/data/chapters.sq, data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelight/data/history.sq
- Acceptance:
  - One thread owns the writer connection; commands are coalesced per chapter; no write runs on the UI thread.
  - Viewer-flag writes read the old value and merge with the mask inside one transaction.
  - Pragmas: WAL and a busy timeout are set at open.

### T-43a-028 — Page-change progress save (last page read, error skip, incognito)
- Priority: P0
- Effort: M
- Depends: T-43a-088, T-43a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - Each visible page updates the in-memory `current_page` (1-based) and `requested_page` (0-based) and writes `last_page_read` (0-based).
  - A page in `Error` writes nothing to the database (E-PROG-1).
  - Incognito writes nothing to progress, history or track tables (E-INC-1).

### T-43a-029 — Completion on the last page (read flag, idempotent)
- Priority: P0
- Effort: S
- Depends: T-43a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - Landing on the last page sets `read = true` and writes it; visiting the last page again repeats the write without harm (E-PROG-2).
  - Nothing sets `read = false` in the reader (E-PROG-3).
  - The completion hook order is: track update, delete-after-read, duplicate marking (T-43a-030, T-43a-031, T-43a-032).

### T-43a-030 — Tracker auto-update on completion
- Priority: P1
- Effort: S
- Depends: T-43a-029, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - Runs only when `pref_auto_update_manga_sync_key` is on and the session is not incognito (E-TRACK-1 reader side).
  - Passes the chapter number as a double; trackers at or ahead of that number are skipped by the tracker (E-TRACK-2).
  - Whether `autoUpdateTrackOnMarkRead` also gates this path is decided in area:27-tracking-core before this task is closed (open question recorded in the edge file).

### T-43a-031 — Mark duplicate chapters as read on completion
- Priority: P1
- Effort: S
- Depends: T-43a-029
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Runs only when the preference set `mark_duplicate_read_chapter_read` contains `existing`.
  - Marks unread chapters with the same recognised chapter number, including only chapters still in the reader list (E-DUP-1).
  - Comparison of chapter numbers uses float precision as in Kotlin (E-DUP-2).

### T-43a-032 — Delete-after-read queue (slots rule, deferred to leaving the reader)
- Priority: P1
- Effort: M
- Depends: T-43a-029, area:09-domain-source-track-download-release-misc
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - `remove_after_read_slots = -1` disables the feature; any other negative value is also treated as disabled (E-DEL-1).
  - Slot value `n >= 0` targets the chapter `n` positions before the completed one; `0` targets the completed chapter (E-DEL-2).
  - Only read chapters are queued; the queue is flushed when the reader closes and survives a restart (E-DEL-4).

### T-43a-033 — History upsert with session duration
- Priority: P1
- Effort: M
- Depends: T-43a-088
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, data/src/main/sqldelight/data/history.sq
- Acceptance:
  - A flush on pause or chapter switch writes `last_read` and the session duration in milliseconds.
  - A second flush with no start time keeps the existing non-zero duration (E-HIST-1).
  - Start time is owned by the UI thread only (E-HIST-2); incognito writes nothing.

### T-43a-034 — Download-ahead (next chapters while reading a downloaded chapter)
- Priority: P1
- Effort: M
- Depends: T-43a-025, area:23-downloads-manga, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetNextChapters.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - Runs only when the amount `auto_download_while_reading` is greater than zero, the current chapter is downloaded, the next chapter is downloaded, and the page is past 25 percent by the 1-based rule (E-DLA-1, E-DLA-2).
  - Queues at most `amount` chapters starting from the next chapter; when the next chapter is read the list starts from it (documented fix of the Kotlin fallback).
  - The amount is read when the reader opens.

### T-43a-035 — Bookmark toggle with persistence and revert on failure
- Priority: P1
- Effort: S
- Depends: T-43a-088, T-43a-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, data/src/main/sqldelight/data/chapters.sq
- Acceptance:
  - The icon, label and stored flag change together; the initial state comes from the chapter.
  - A failed write reverts the icon and shows "Unknown error" (E-BMK-1).
  - The label is "Bookmark chapter" or "Unbookmark chapter" (`action_bookmark`, `action_remove_bookmark`).

### T-43a-038 — Incognito session behaviour
- Priority: P1
- Effort: S
- Depends: T-43a-028, T-43a-033, T-43a-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - Incognito blocks progress, history, tracker and duplicate writes; the in-memory restore keys are kept but not saved (E-INC-1).
  - Turning the global incognito flag off closes the reader (E-INC-2).
  - The flag is read when the session starts, as the Kotlin lazy does.

### T-43a-039 — Crash restore of chapter id and page index
- Priority: P1
- Effort: S
- Depends: T-43a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - The chapter id and page index are saved on each change and restored on reopen of the same manga within 24 hours (E-RESTORE-1 decision).
  - An explicit chapter from the library always wins over a saved one.
  - A `restore_after_crash` setting, default on, disables the restore.

## Per-manga settings and reader controls

### T-43a-036 — Per-manga reading mode and orientation (resolve default, set, reload)
- Priority: P0
- Effort: M
- Depends: T-43a-004, T-43a-088, T-43a-081
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Resolved mode is the manga value, or the default preference (2 = right-to-left) when the manga value is 0; the raw value drives the bottom bar icon.
  - Changing the mode saves the current page, reloads the chapters and recreates the viewer; no UI-thread blocking (E-UI-2).
  - Orientation defaults to the preference value (8 = free) and is stored only on Linux (T-43a-068).

### T-43a-037 — Crop borders toggle (per viewer type) with On/Off toast
- Priority: P0
- Effort: S
- Depends: T-43a-036, T-43a-081
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Pager modes toggle `crop_borders`; webtoon modes toggle `crop_borders_webtoon`; both default to off.
  - The toast reads "On" or "Off" every time, and a previous toast is replaced.
  - The button icon reflects the state (crop versus crop-off).

## Reader chrome and sheets

### T-43a-040 — Reader top bar (back, title block, bookmark, source-dependent overflow)
- Priority: P0
- Effort: M
- Depends: T-43a-084
- Sources: app/src/main/java/eu/kanade/presentation/reader/appbars/ReaderAppBars.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Line 1 is the manga title, line 2 the chapter name; both come from the session.
  - Overflow items "Open in WebView", "Open in browser", "Share" appear only for HTTP sources.
  - The title block opens the manga details screen (area:38-ui-entry-details-manga route).
  - Slides in and out in 200 ms.

### T-43a-041 — Chapter navigator (buttons, page slider, RTL mirror)
- Priority: P0
- Effort: M
- Depends: T-43a-040, T-43a-026
- Sources: app/src/main/java/eu/kanade/presentation/reader/components/ChapterNavigator.kt
- Acceptance:
  - Slider range is 1..N, step 1; moving it calls the viewer's move-to-page with index minus one; equal values do nothing.
  - Slider is hidden when N is 1 or less; before the first page it does not show an out-of-range value (E-UI-1).
  - In RTL the pill is mirrored and the buttons swap action and enablement; icons are mirrored too (E-UI-7).
  - Tablet-width padding is 24 px, otherwise 8 px.

### T-43a-042 — Reader bottom bar (reading mode, rotation, crop, settings)
- Priority: P0
- Effort: S
- Depends: T-43a-040, T-43a-036, T-43a-037
- Sources: app/src/main/java/eu/kanade/presentation/reader/appbars/BottomReaderBar.kt
- Acceptance:
  - Four buttons in order with the content descriptions `viewer`, `rotation_type`, `pref_crop_borders`, `action_settings`.
  - The reading-mode and rotation icons show the raw manga value (Default shows the Default icon).
  - Buttons are evenly spaced and never wrap at small widths.

### T-43a-043 — Reading-mode sheet (select, apply, revert)
- Priority: P0
- Effort: M
- Depends: T-43a-084, T-43a-036
- Sources: app/src/main/java/eu/kanade/presentation/reader/ReadingModeSelectDialog.kt, app/src/main/java/eu/kanade/presentation/reader/components/ModeSelectionDialog.kt
- Acceptance:
  - Shows the five modes (no Default) with icons and labels; selection is local until Apply.
  - Revert is shown only when the manga value is not Default; Apply shows the mode label as a toast only when `show_reading_mode` is off (E-UI-4).
  - Keyboard: arrows move, Space selects, Enter applies, Escape dismisses.

### T-43a-044 — Rotation sheet (select, apply, no-effect note)
- Priority: P1
- Effort: S
- Depends: T-43a-084, T-43a-036
- Sources: app/src/main/java/eu/kanade/presentation/reader/OrientationSelectDialog.kt
- Acceptance:
  - Shows Free, Portrait, Landscape, Locked portrait, Locked landscape, Reverse portrait; Apply always toasts the label.
  - The sheet states "Rotation has no effect on this platform" (E-ORI-1).
  - The stored value round-trips through backup.

### T-43a-045 — Page actions sheet, long press and right-click
- Priority: P1
- Effort: S
- Depends: T-43a-084, T-43a-054, T-43a-057, T-43a-056
- Sources: app/src/main/java/eu/kanade/presentation/reader/ReaderPageActionsDialog.kt
- Acceptance:
  - Opens on long press and on right-click of a page (and Shift+F10 keyboard).
  - Four actions in order: Set as cover, Copy to clipboard, Share, Save; Copy, Share and Save close the sheet; Set as cover keeps it open until the result toast (E-UI-5).
  - Actions do nothing for pages not in `Ready` state (checked again when the action runs).

### T-43a-046 — Reader settings window: reading-mode tab (pager and webtoon blocks)
- Priority: P0
- Effort: L
- Depends: T-43a-084, T-43a-081, T-43a-036
- Sources: app/src/main/java/eu/kanade/presentation/reader/settings/ReaderSettingsDialog.kt, app/src/main/java/eu/kanade/presentation/reader/settings/ReadingModePage.kt
- Acceptance:
  - Tab order and titles: "Reading mode", "General", "Custom filter"; the dialog caps at 75 percent of the window height.
  - Pager block controls and defaults exactly as in `p1-ui-parity.md` section 4.5; webtoon block only for the webtoon and continuous modes.
  - "Split wide pages" writes `pref_dual_page_split` and "Invert split page placement" writes `pref_dual_page_invert` (E-UI-3 fix); each control writes only its own key.
  - The mode and rotation chips apply immediately.

### T-43a-047 — Reader settings window: general tab
- Priority: P0
- Effort: M
- Depends: T-43a-046, T-43a-051
- Sources: app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt
- Acceptance:
  - Controls and defaults: background (black default), show page number, fullscreen, keep screen on, long-tap actions, always show transition, page transitions, flash (off default; duration 100-1500 ms in 100 ms steps; interval 1-10; colour black/white/white-black).
  - Cutout setting is hidden on Linux (T-43a-069).
  - Sliders show the duration in milliseconds and the interval in pages with plural text.

### T-43a-048 — Reader settings window: custom filter tab
- Priority: P1
- Effort: M
- Depends: T-43a-066, T-43a-064, T-43a-065
- Sources: app/src/main/java/eu/kanade/presentation/reader/settings/ColorFilterPage.kt
- Acceptance:
  - Brightness slider -75..100 shown only when custom brightness is on; colour sliders 0..255 for red, green, blue and alpha shown only when the colour filter is on.
  - Blend-mode chips show the six modes in order (default, multiply, screen, overlay, lighten, darken).
  - Byte-masked channel writes match `getColorValue` (test `colour_channel_mask_write`).

### T-43a-049 — Page indicator
- Priority: P0
- Effort: S
- Depends: T-43a-028, T-43a-084
- Sources: app/src/main/java/eu/kanade/presentation/reader/PageIndicatorText.kt
- Acceptance:
  - Shows "n / total" only when the menu is hidden, the page number preference is on, and both values are greater than zero.
  - Uses the bold 235-grey text with a 2 px dark outline, bottom centre.

### T-43a-050 — Chrome visibility, fullscreen and key escape
- Priority: P0
- Effort: M
- Depends: T-43a-040, T-43a-041, T-43a-042
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Menu visibility controls the top bar, navigator and bottom bar with a 200 ms slide; a viewer tap toggles it (area:44-reader-viewers call).
  - With the fullscreen preference on, hiding the menu enters window fullscreen; showing it leaves fullscreen.
  - F11 toggles fullscreen; Escape leaves fullscreen first, then leaves the reader.

### T-43a-051 — Keep screen on while reading
- Priority: P0
- Effort: S
- Depends: T-43a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Preference `pref_keep_screen_on_key` (default on) inhibits idle while the reader is open and releases the inhibit on close.
  - Changing the preference takes effect without reopening the reader.

### T-43a-052 — Reader background theme choices
- Priority: P0
- Effort: S
- Depends: T-43a-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt
- Acceptance:
  - Values: 0 white, 1 black (default), 2 gray `#202125`, 3 automatic (gray when the system is dark, else white).
  - The change applies immediately to the reader background.
  - Chrome colours come from the active GTK theme, not these values (ui-parity section 8).

### T-43a-053 — Reader keyboard shortcuts (N, P, and desktop shortcuts)
- Priority: P1
- Effort: S
- Depends: T-43a-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - N = next chapter, P = previous chapter, handled after the viewer gets the key (E-KEY-1).
  - Ctrl+D bookmark, Ctrl+C copy page, Ctrl+S save page, F11 fullscreen; all listed in the in-app help (area:36-ui-shell-navigation-security-webview).
  - Shortcuts are inactive while a text field has focus.

### T-43a-054 — Save page to the Pictures folder
- Priority: P1
- Effort: M
- Depends: T-43a-088
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/tachiyomi/data/saver/ImageSaver.kt
- Acceptance:
  - File name is `"<manga> - <chapter> - <page>"` truncated to 250 bytes minus the suffix, sanitised (test `filename_suffix_and_limit`).
  - Folder per manga when `create_folder_per_manga` is on; otherwise the Pictures root.
  - A name collision appends " (2)", " (3)", ... (E-SAVE-2 proposal).
  - Errors are shown as an error notification and a toast; the app never crashes (E-SHARE-1).

### T-43a-055 — Saved-picture notification
- Priority: P1
- Effort: S
- Depends: T-43a-054
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt
- Acceptance:
  - Title "Picture saved" with a thumbnail decoded to at most 720 by 1280; default action opens the file; "Share" button shares it.
  - Saving again replaces the previous notification (fixed id).
  - Error variant shows "Error" and the message or "Unknown error".

### T-43a-056 — Share page or open it in the file manager (replacement for the Android share sheet)
- Priority: P2
- Effort: M
- Depends: T-43a-054
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - The shared image is written to a cache folder that is emptied before each share, so only the last share is kept.
  - The share message text is "<manga>: <chapter>, page <n>" (`share_page_info`).
  - Share opens the file with the default handler and offers "Show in file manager" through the file manager interface; the loss of the system share list is documented.

### T-43a-057 — Copy page to clipboard
- Priority: P1
- Effort: S
- Depends: T-43a-054
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - The clipboard holds the page as an image (PNG) and no toast is shown (E-CLIP-1).
  - Copy is disabled for pages not in `Ready` state.

### T-43a-058 — Set page as manga cover
- Priority: P1
- Effort: M
- Depends: T-43a-045, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/presentation/reader/ReaderPageActionsDialog.kt
- Acceptance:
  - Confirm dialog "Use this image as cover art?" with OK and Cancel.
  - Cover is written before the library check; a manga not in the library reports "Please add the entry to your library before doing this" (E-COVER-1).
  - Any failure reports "Failed to update cover" (all failure types caught, E-COVER-2); success reports "Cover updated".

### T-43a-059 — Open chapter in browser, WebView or share the chapter link
- Priority: P1
- Effort: S
- Depends: T-43a-040, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - The chapter URL comes from the source; if the source fails to build it, a toast "Unknown error" is shown (E-URL-1).
  - Open in browser uses the desktop default browser; Open in WebView opens the in-app WebView (area:36-ui-shell-navigation-security-webview) with the source id and manga title.
  - Share copies the chapter URL to the clipboard as the Linux replacement for the share sheet.

### T-43a-060 — Open the manga from the reader title
- Priority: P0
- Effort: S
- Depends: T-43a-040, area:38-ui-entry-details-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Clicking the title block opens the manga details for the manga in the session.
  - Opening the details does not close the reader state; returning to the reader resumes the same page.

## Overlays, brightness, colour and flash

### T-43a-062 — Navigation overlay with tap-zone labels (fade, pass-through)
- Priority: P2
- Effort: S
- Depends: T-43a-087
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderNavigationOverlayView.kt
- Acceptance:
  - Shown on first launch only when `reader_navigation_overlay_on_start` is on; hidden for the disabled tap layout.
  - Fades in and out over 1000 ms; any input fades it out.
  - The overlay never takes pointer input (E-UI-8).

### T-43a-063 — Flash on page change overlay
- Priority: P2
- Effort: M
- Depends: T-43a-047, T-43a-086
- Sources: app/src/main/java/eu/kanade/presentation/reader/DisplayRefreshHost.kt
- Acceptance:
  - Flash runs on every `flash_page_interval`-th page change (counting from the first call) and only while the feature is on (E-FLASH-1).
  - Duration is the preference value (default 100 ms); black or white for the whole duration, or white then black halves.
  - The overlay is a solid full-window colour and does not block input.

### T-43a-064 — Custom brightness as a software overlay (negative values)
- Priority: P1
- Effort: S
- Depends: T-43a-081
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/presentation/reader/ReaderContentOverlay.kt
- Acceptance:
  - Values -75..-1 darken the reader with black at alpha |v|/100; 0 removes the overlay.
  - Only one value subscription exists; disabling the feature removes it (E-BRIGHT-1).
  - The overlay is below the chrome and does not change chrome colours.

### T-43a-065 — Positive brightness through the display backlight (with unsupported fallback)
- Priority: P2
- Effort: M
- Depends: T-43a-064
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Values 1..100 map to the backlight when a writable backlight exists; the mapping is documented.
  - No writable backlight shows "Custom brightness is not available on this display" once per session (E-BRIGHT-2).
  - The system brightness is restored when the feature is turned off.

### T-43a-066 — Colour filter, grayscale and invert layers
- Priority: P1
- Effort: M
- Depends: T-43a-081
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/presentation/reader/ReaderContentOverlay.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Packed ARGB colour from four 0..255 channels with masks and shifts from `ColorFilterPage.kt`; mode index 0..5 maps to default, multiply, screen, overlay, lighten, darken.
  - Grayscale is applied before invert; both are off by default.
  - The layer is applied to the page area only (not the chrome).

### T-43a-067 — ICC display profile for page colour
- Priority: P2
- Effort: L
- Depends: T-43a-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - A profile file chosen in the settings is read and used for page colour transforms (lcms2).
  - A missing or unreadable file falls back to no transform and logs a warning, not a crash.
  - Until implemented, the loss is documented in the settings help.

## Platform replacements and drops (Android-only)

### T-43a-068 — Screen orientation lock and auto-rotate (drop, values preserved)
- Priority: P3
- Effort: S
- Depends: T-43a-004, T-43a-044
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Requested-orientation calls are not ported; the per-manga value is kept and synced.
  - The rotation sheet says the setting has no effect here (E-ORI-1).
  - Backup and restore keep the orientation values unchanged.

### T-43a-069 — Display cutout short-edge mode (drop, hide setting)
- Priority: P3
- Effort: S
- Depends: T-43a-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt
- Acceptance:
  - The "Show content in cutout area" setting is not shown on Linux; the stored value is kept.
  - No cutout-related layout code is ported.

### T-43a-070 — Haptic feedback on slider drag (drop)
- Priority: P3
- Effort: S
- Depends: T-43a-041
- Sources: app/src/main/java/eu/kanade/presentation/reader/components/ChapterNavigator.kt
- Acceptance:
  - No haptic call is made on Linux; the slider behaves otherwise the same.
  - The loss is recorded in the feature list as "no haptic feedback on desktop".

### T-43a-071 — Volume keys page turning (drop, preference kept)
- Priority: P3
- Effort: S
- Depends: T-43a-053
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Volume keys are not used for page turning; the preference keys remain in the store for backup compatibility.
  - Arrow keys, Page Up and Page Down remain the keyboard page turn (area:44-reader-viewers).

### T-43a-072 — Activity transitions (replace with stack slide)
- Priority: P3
- Effort: S
- Depends: T-43a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Opening and closing the reader slides the stack page in and out; no shared-element transition is ported.

### T-43a-073 — Android platform plumbing (drop)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/presentation/reader/appbars/ReaderAppBars.kt
- Acceptance:
  - Window insets, system bar colours, the assist content hook, secure-activity registration and the notification channel setup are not ported; their replacements are GTK fullscreen and the desktop notification service.
  - Any of these that is still needed by a feature is covered by the feature's own task.

## Reader core: chapter, page and strings

### T-43a-061 — Reader strings table (English keys)
- Priority: P0
- Effort: S
- Depends: none
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n/src/commonMain/moko-resources/base/plurals.xml, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ChapterLoader.kt, app/src/main/java/eu/kanade/presentation/reader/ChapterTransition.kt
- Acceptance:
  - Every key in `p1-behavior.md` section 20 exists in `src/reader/strings.rs` with the English text shown there.
  - The plural `missing_chapters_warning` has one and other forms.
  - Strings marked "text not re-read" are verified before release (color_filter_*_value, vertical_plus_viewer, right_to_left_viewer, vertical_viewer).

### T-43a-074 — Downloaded-only blocked-chapter message and retry
- Priority: P0
- Effort: S
- Depends: T-43a-018, T-43a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterFilterDownloaded.kt
- Acceptance:
  - When the requested chapter is not downloaded in downloaded-only mode, the card reads "The chapter is not downloaded. Turn off downloaded-only mode or download it first." with Retry (E-LIST-2 proposal text).
  - Retry reopens with the same chapter.

### T-43a-075 — Reader session state machine (main-thread owner)
- Priority: P0
- Effort: M
- Depends: T-43a-001, T-43a-086
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - All state changes happen on the UI thread through small functions that compute the new value before assigning it (E-STATE-1).
  - Dialog state has one of `Loading, Settings, ReadingModeSelect, OrientationSelect, PageActions` at a time; closing a chapter load does not close a sheet whose page is still loaded (E-UI-5).
  - Brightness overlay value stays in -100..100.

### T-43a-076 — Chapter load and transition visual states (spinner, error card)
- Priority: P0
- Effort: S
- Depends: T-43a-075, T-43a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Initial load shows a spinner; adjacent load shows a spinner inside the navigator with buttons disabled; background load shows a spinner over the page area.
  - Error states show the card from T-43a-024; no modal blocking dialog is used.

## Mockup replacement and wiring

### T-43a-077 — Replace the placeholder reader with the real reader page
- Priority: P0
- Effort: M
- Depends: T-43a-075, T-43a-087, T-43a-084
- Sources: src/player.rs, src/screens.rs, src/main.rs, spec/reader_player.json
- Acceptance:
  - `player::reader` is replaced by `reader::open(manga_id, chapter_id)`; the `screen:reader` widget name stays so the sidebar hiding still works.
  - The fixed 20-page slider, the placeholder cover art, and the fake tap regions are removed (gaps G-01, G-02, G-19).
  - The spec-driven sheets are replaced by the real sheets; only the strings from the spec are reused where they match the Kotlin strings table.

### T-43a-078 — Mockup gap list closure (titles, bookmark, menu filter, real data)
- Priority: P0
- Effort: S
- Depends: T-43a-077, T-43a-035, T-43a-059
- Sources: src/player.rs, src/screens.rs, spec/reader_player.json
- Acceptance:
  - Titles come from the manga and chapter (G-03); the bookmark state is persisted (G-06); overflow items are filtered by source kind (G-07).
  - Each gap G-01 to G-35 in `p1-ui-parity.md` section 12 is closed or assigned to area:44-reader-viewers with a reference.

### T-43a-079 — GTK sheet and dialog framework for the reader
- Priority: P0
- Effort: M
- Depends: none
- Sources: src/ui.rs, src/main.rs
- Acceptance:
  - A bottom revealer sheet and a modal settings window exist for reuse by the reading-mode, rotation, page-action and settings screens.
  - Escape and outside click dismiss sheets; the confirm dialog uses `gtk::AlertDialog`.
  - No libadwaita dependency is introduced.

## Settings, persistence, defaults

### T-43a-081 — Reader settings store (Aniyomi key names, defaults, ranges)
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - Every reader key in `p1-data-and-config.md` section 3.1 is stored under its Aniyomi name with the listed default and type.
  - Enum values are stored by name; invalid stored values fall back to the default.
  - Writes go through the writer actor (T-43a-088); the in-memory cache is refreshed after each acknowledged write.

### T-43a-080 — Settings change propagation to the open reader
- Priority: P0
- Effort: S
- Depends: T-43a-081, T-43a-075
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Theme, page number, fullscreen, keep-awake, flash, brightness and colour filter changes apply while the reader is open.
  - The download-ahead amount and remove-after-read slots are read at open (their own preferences), as in Kotlin.

## Tests

### T-43a-082 — Pure-logic tests (list, sort, dedupe, gap, flags, refs, transitions)
- Priority: P0
- Effort: M
- Depends: T-43a-018, T-43a-019, T-43a-020, T-43a-021, T-43a-002, T-43a-003, T-43a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt
- Acceptance:
  - All tests listed in the pure-logic table of `p1-edge-cases-and-tests.md` section 10 exist and pass.
  - `cargo test` runs without a display.

### T-43a-083 — Loader fixtures and tests (directory, archive, epub)
- Priority: P0
- Effort: M
- Depends: T-43a-008, T-43a-009, T-43a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/DirectoryPageLoader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ArchivePageLoader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/EpubPageLoader.kt
- Acceptance:
  - Fixtures `dir_natural`, `archive.cbz`, `empty.cbz`, `book.epub`, `broken.epub` exist under `tests/fixtures/reader/`.
  - Each loader test in section 10 passes, including the natural order and the empty-result message.

### T-43a-084 — HTTP loader tests with a scripted mock source
- Priority: P0
- Effort: M
- Depends: T-43a-013, T-43a-014, T-43a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt
- Acceptance:
  - Preload, priority, retry, eviction re-queue, data-saver key, failure isolation, cancellation and lazy worker tests all pass (section 10).
  - The page list is persisted after a fetch and not refetched after restart.

### T-43a-085 — Integration tests for the reader chrome (headless GTK)
- Priority: P1
- Effort: M
- Depends: T-43a-040, T-43a-041, T-43a-043, T-43a-046
- Sources: src/player.rs, src/main.rs
- Acceptance:
  - Navigator, bookmark, bottom bar, reading-mode sheet, settings split binding and the RTL mirror pass under a headless display (section 10 integration table).
  - Test runs in CI with `xvfb-run` or equivalent.

## Replacement of remaining areas

### T-43a-089 — Viewer-independent reader keyboard focus and accessibility labels
- Priority: P1
- Effort: S
- Depends: T-43a-040, T-43a-042
- Sources: src/player.rs
- Acceptance:
  - Every icon-only control has the tooltip and accessible name from `p1-ui-parity.md` section 13.
  - Focus order is top bar, navigator, bottom bar; toggled state is exposed for bookmark and crop.

### T-43a-090 — Release notes for intentional deviations
- Priority: P1
- Effort: S
- Depends: T-43a-082, T-43a-084
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - The notes list every item marked "Fix" in `p1-edge-cases-and-tests.md` (E-LEAK-1, E-LEAK-2, E-CANCEL-1, E-TR-1, E-HTTP-5, E-HTTP-7, E-SORT-3, E-LIST-2, E-NAV-1, E-EVT-1, E-DEL-1, E-HIST-1, E-HIST-2, E-INC-1, E-SHARE-1, E-COVER-2, E-IMG-3, E-DL-1, E-UI-2, E-UI-3, E-UI-5, E-UI-7, E-FLASH-1, E-BRIGHT-1, E-STATE-1) and each "Drop" or "Open" item.

### T-43a-091 — Crash-safe write of the reader session row
- Priority: P1
- Effort: S
- Depends: T-43a-088, T-43a-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - The `(chapter_id, page_index)` row is written through the writer actor on each change and read on reopen.
  - A crash between a page change and the write loses at most the last page change.

### T-43a-092 — Chapter page-list reuse after restart
- Priority: P0
- Effort: S
- Depends: T-43a-016, T-43a-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt
- Acceptance:
  - Reopening an online chapter uses the cached page list when present, with no source call for the list.
  - A stale cache is replaced when the source returns a different list (documented).

### T-43a-093 — Viewer-facing reader events for area:44-reader-viewers (page selected, menu toggles, transition requests)
- Priority: P0
- Effort: S
- Depends: T-43a-087, T-43a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - `on_page_selected` ignores inserted pages and runs progress, chapter switch, download-ahead and flash in that order.
  - `toggle_menu`, `show_menu`, `hide_menu`, `request_preload` and `long_tap` are routed to the session and the chrome.

### T-43a-094 — Reader strings for new proposal messages
- Priority: P1
- Effort: S
- Depends: T-43a-061
- Sources: src/reader/strings.rs
- Acceptance:
  - New messages (downloaded-only card text, brightness unsupported, rotation no-effect note, "Unknown error" toast, collision suffix) are in the strings table and marked as proposals.
