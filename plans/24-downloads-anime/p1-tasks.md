# 24-downloads-anime: implementation tasks

Each task maps to behaviour sections in `p1-behavior.md` (cited as "behaviour §n"), data in `p1-data-and-config.md` (data §n), design in `p1-rust-design.md` (design §n), UI in `p1-ui-parity.md` (ui §n), and edge cases in `p1-edge-cases-and-tests.md` (E-ids). Source paths are repo-relative to `/home/blackbox/aniyomi-src` (every `Sources:` entry is written in full). Paths under `src/` (`src/screens.rs`, `src/entry.rs`, `src/settings.rs`) are in the Kurobayomi repo `/home/blackbox/kurobayomi`, not in the Aniyomi tree.

Priority: P1 for parity features needed by anime downloads; P2 for polish and long tail; P3 for Android-only or dead code (each P3 task states the replacement or the drop decision).

---

## A. Model, naming, layout

### T-24a-001 — AnimeDownload model and status state machine
- Priority: P1
- Effort: M
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - Rust `DlState` has values 0 NotDownloaded, 1 Queue, 2 Downloading, 3 Downloaded, 4 Error, and `is_active()` is true for 0..=2 only.
  - Transitions match behaviour §1.1, including ERROR→QUEUE on `start()` and DOWNLOADING→QUEUE on pause.
  - Unit test covers every transition listed in the state diagram.

### T-24a-002 — Download progress semantics
- Priority: P1
- Effort: S
- Depends: T-24a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - Progress is an `i32` clamped to 0..=100; 0 means indeterminate in the UI.
  - Percent for ffmpeg = `100 * out_seconds / duration_seconds` with integer truncation, 0 when duration or out time is 0 (behaviour §5.7).
  - Published value changes only when the integer changes.

### T-24a-003 — Drop AnimeDownloadPart (dead HTTP range model)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownloadPart.kt
- Acceptance:
  - The file has zero references in the repo (verified); the port does not create a range-part type.
  - Multi-part range downloads are not offered for anime; users lose nothing (no current behaviour).

### T-24a-004 — Episode, anime and source directory naming, FAT-safe filenames
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - `fat_safe` trims `.` and space at both ends, returns `(invalid)` for empty, replaces `"*/:<>?\|` and 0x00-0x1F, 0x7F with `_`, truncates to 240 characters (behaviour §9.2).
  - Source dir = `fat_safe("<name> (<LANG>)")`; anime dir = `fat_safe(title)`; episode dir = `fat_safe([scanlator_]name)` with blank name → `Episode` and blank scanlator ignored.
  - Golden tests reproduce every example in `p1-edge-cases-and-tests.md` §4.1.

### T-24a-005 — Legacy episode directory name compatibility
- Priority: P1
- Effort: S
- Depends: T-24a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt
- Acceptance:
  - Legacy name = `fat_safe(scanlator != null ? "<scanlator>_<name>" : name)` with no sanitising and no blank check.
  - Lookups check new name first, then legacy name; first existing wins.
  - `is_episode_dir_name_changed` treats blank and null scanlator as equal (behaviour §9.2).

### T-24a-006 — Directory discovery and creation
- Priority: P1
- Effort: S
- Depends: T-24a-004, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - `get_anime_dir` creates source and anime dirs; `find_*` never creates (behaviour §9.3).
  - A missing or unset root yields `DlError::InvalidLocation` with the displayed path, not a panic.
  - `find_episode_dirs` returns `(None, [])` when the anime dir is missing.

### T-24a-007 — Episode directory rename on metadata change
- Priority: P1
- Effort: M
- Depends: T-24a-006, T-24a-035, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt
- Acceptance:
  - `rename_episode` renames the first existing of [new, legacy] to the new name, then updates the cache (remove old, add new).
  - No rename and no error when names already match; error logged when no folder exists.
  - Called only when `is_episode_dir_name_changed` is true (caller in area:05-data-repos-entries-items).

### T-24a-008 — Source folder rename, including case-only change
- Priority: P1
- Effort: S
- Depends: T-24a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - If names differ only by case, rename to `<new>_tmp` first, then to `<new>` (behaviour §6.4).
  - Cache is invalidated after rename (proposal; Kotlin does not, see behaviour §6.4).

---

## B. Queue, persistence, scheduling

### T-24a-009 — Persistent download queue store (SQLite)
- Priority: P1
- Effort: M
- Depends: area:04-db-anime-schema-migrations, T-24a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadStore.kt
- Acceptance:
  - Table `anime_download_queue(episode_id PK, anime_id, ord, changed_at)` created by a migration (data §5.1).
  - `add_all`, `remove`, `remove_all`, `clear`, `restore` behave as data §5.1 operations.
  - Restore keeps rows whose anime exists even if the source is not installed (fixes E-Q02); drops rows whose anime or episode is missing (E-Q03).

### T-24a-010 — Manager API and command channel
- Priority: P1
- Effort: L
- Depends: T-24a-009, T-24a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - Commands `Enqueue`, `AddToStart`, `StartNow`, `Reorder`, `Cancel`, `CancelAnime`, `Start`, `Pause`, `ClearQueue`, `DeleteEpisodes`, `DeleteAnime`, `EnqueueDelete`, `FlushPendingDeletes`, `RenameEpisode`, `RenameSource` exist (design §3).
  - The scheduler task is the only writer of the queue; the GTK side never mutates it.
  - Every public Kotlin method in `AnimeDownloadManager.kt` has a Rust equivalent or a documented drop.

### T-24a-011 — Scheduler: one download per source, at most three sources
- Priority: P1
- Effort: M
- Depends: T-24a-001, T-24a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - `active_set` returns the first active download per source in queue order, limited to 3 sources (behaviour §2.3).
  - An ERROR download frees its slot and the next item in the same source starts.
  - Scheduler recomputes when any active item becomes ERROR, when the queue changes, or when a download finishes.
  - Test with 5 sources × 2 items and a slow fake ffmpeg shows exactly 3 concurrent children.

### T-24a-012 — Enqueue filtering and ordering
- Priority: P1
- Effort: S
- Depends: T-24a-006, T-24a-010, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - Skip episodes already downloaded (new or legacy name); skip episodes already queued by id; skip fillermarked unless `pref_download_fillermarked` (behaviour §2.8, §3.1).
  - Remaining episodes sorted by `source_order` descending and appended to the queue.
  - Non-HTTP sources are ignored with a debug log.
  - `autoStart` starts the runner only when the queue was empty before the call (behaviour §2.8 note).

### T-24a-013 — Queue size warnings
- Priority: P1
- Effort: S
- Depends: T-24a-012, T-24a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - Warning when metered queued items > 20 or any metered source has > 10 items; unmetered sources excluded.
  - Notification body is the `download_queue_size_warning` text; timeout 30 s; action opens the help URL.
  - Shown once per enqueue batch that starts a run.

### T-24a-014 — Best-video resolution through the hoster loader
- Priority: P1
- Effort: M
- Depends: area:45-player-core, area:16-source-api-anime, T-24a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - `HosterResolver::best_video` is called only when `video` is None (behaviour §5.2).
  - Empty hoster list or no best video yields `NoVideo` ("No video found").
  - Resolution is cancellable; cancellation leaves `video` unset.

### T-24a-015 — Retry policy: 4 attempts, delays 2 s, 4 s, 8 s
- Priority: P1
- Effort: S
- Depends: T-24a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - `RETRY_DELAYS_MS = [2000, 4000, 8000]`; total attempts 4 (behaviour §5.5).
  - Cancellation during a delay returns `Cancelled` immediately.
  - Test with paused time verifies the delay sequence.

### T-24a-016 — Staging, remux completion and finalise
- Priority: P1
- Effort: M
- Depends: T-24a-004, T-24a-006, T-24a-035, T-24a-049
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - Staging dir `<episode_dir>_tmp` created before download; partial `.tmp` deleted at attempt start and on failure or cancel (behaviour §5.2, §5.5).
  - Finished `<name>.mkv` found in staging is reused without remux (behaviour §5.3).
  - Finalise: delete `<anime> - <name>_tmp.mkv` if present, rename staging dir to final name, add to cache, create `.nomedia` in the final dir (behaviour §5.9).
  - Rename failure is an error, never a silent success (fixes E-F02).

### T-24a-017 — Free-space precheck (200 MiB)
- Priority: P1
- Effort: S
- Depends: area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - `statvfs` on the anime dir; if available < 209,715,200 bytes, set ERROR and notify "Couldn't download due to low storage space".
  - A statvfs error skips the check (behaviour §5.2).

### T-24a-018 — ffmpeg remux argument builder
- Priority: P1
- Effort: L
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - `build_remux_args` produces exactly the order in design §7 and behaviour §5.7.1 for all combinations (video only, subtitles, audio, both).
  - Input-index arithmetic: audio maps use `i + 1 + subtitle_count`; subtitle maps use `i + 1`.
  - Golden tests pass; argv is passed without a shell.

### T-24a-019 — ffmpeg progress from stats, and ffprobe duration
- Priority: P1
- Effort: M
- Depends: T-24a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - ffprobe args: `-v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 <url>` with the header prefix; output parsed as float then truncated to integer seconds.
  - Progress from `out_time_us` (microseconds) via `-progress pipe:1 -nostats`; published at most every 100 ms.
  - ffprobe failure is a retryable error with ffprobe's first stderr line as message.

### T-24a-020 — HTTP header propagation to ffmpeg inputs
- Priority: P1
- Effort: S
- Depends: T-24a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - Header string is `Name: value\r\n` pairs in order, passed as one argv element before an input only when that input's URL starts with `http` (behaviour §5.7).
  - Video headers used when present, else the source headers.
  - CR and LF inside header values are stripped (fixes E-F13).

### T-24a-021 — Subtitle and audio track muxing and metadata
- Priority: P1
- Effort: M
- Depends: T-24a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt
- Acceptance:
  - Subtitle and audio tracks become extra inputs in that order, followed by `-metadata:s:s:<i> title=<lang>` and `-metadata:s:a:<i> title=<lang>`.
  - `-map 0:a?`, `-map 0:s?`, `-map 0:t?` included exactly as Kotlin.
  - A local-file track gets no header option.

### T-24a-022 — Local HTTP server for localhost video URLs
- Priority: P1
- Effort: M
- Depends: area:12-network-stack, area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt
- Acceptance:
  - `uses_http_server()` matches the regex `http:\/\/localhost:1(?!\d)` on the video URL and every track URL (behaviour data §1.4).
  - Server started before remux, port substituted via `copy_http_server(port)`, server stopped on success, failure and cancel.
  - Port 0 is treated as a failure (fixes E-X07). Start timeout 5 s.

### T-24a-023 — Torrent-server download path
- Priority: P2
- Effort: L
- Depends: area:13-core-util-storage-archive-torrent, T-24a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - `is_torrent` matches `magnet*`, `*.torrent`, or the torrent host prefix (behaviour §5.5).
  - `link=`/`index=` conversion to `magnet:?xt=urn:btih:<hash>&index=<n>` is a pure function with tests.
  - Behind feature `torrent-server`; without it, torrent URLs fail with `TorrentUnavailable` (E-X04).

### T-24a-024 — External downloader handoff: drop Android intents, provide a desktop option
- Priority: P3
- Effort: M
- Depends: T-24a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - Drop 1DM and ADM intent handling and the generic `ACTION_VIEW video/*` fallback. Reason: Android intents have no Linux equivalent; the user loses the ability to hand off to Android apps.
  - Replacement option (off by default): send the video URL and headers to aria2 via JSON-RPC when `aria2` is configured. Not finalised on handoff (fixes E-X01).
  - The `use_external_downloader` and `external_downloader_selection` preferences keep their keys and are shown as "External downloader (aria2)" if enabled.

### T-24a-025 — Copy download location to clipboard on external handoff
- Priority: P2
- Effort: S
- Depends: T-24a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - When the external path runs, the staging-dir path (without `_tmp` suffix) is copied to the GTK clipboard with label `Episode download location`.
  - Clipboard write failure is logged, not fatal.

---

## C. Notifications and lifecycle

### T-24a-026 — Progress and state notifications
- Priority: P1
- Effort: M
- Depends: T-24a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadNotifier.kt
- Acceptance:
  - One progress notification slot (emulating ID -203) with title and body per behaviour §10.2, updated at most once per second and on state changes.
  - Paused notification (behaviour §10.3) and no completion notification (behaviour §10.4).
  - Implemented via `notify-rust` with `replace_id`; sends from a blocking thread.

### T-24a-027 — Warning and error notifications
- Priority: P1
- Effort: S
- Depends: T-24a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadNotifier.kt
- Acceptance:
  - Error text: `"<anime>: <episode>"` as title, `Throwable.message` or "Could not download chapter due to unexpected error" as body (behaviour §10.6).
  - Each call builds a fresh notification (fixes E-U03); errors use a distinct slot from progress (ID -204 equivalent).
  - Errors are summarised when more than one arrives within 5 s (proposal, fixes E-U02).

### T-24a-028 — hide_notification_content behaviour
- Priority: P2
- Effort: S
- Depends: T-24a-026, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadNotifier.kt, core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt
- Acceptance:
  - With `hide_notification_content = true` (default false), notification title is the progress text and body is empty (behaviour §10.2 step 3).

### T-24a-029 — Notification title truncation rules
- Priority: P2
- Effort: S
- Depends: T-24a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadNotifier.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - `chop(15)` on anime title, `chop(30)` on `"<title> - <episode>"`, replacement `…` (U+2026).
  - Episode prefix stripped using the untruncated title, case-insensitive (fixes E-U01).

### T-24a-030 — Runner lifecycle and window-close handling
- Priority: P1
- Effort: M
- Depends: T-24a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - Runner state `Stopped`, `Running`, `Paused` exposed via watch channel to the UI (`isDownloaderRunning`).
  - `start` no-op when running or queue empty; `stop` cancels all work, marks DOWNLOADING as QUEUE (pause) or ERROR (failure).
  - Closing the window while downloads run shows "Downloads will pause. Quit?" unless the user disabled it (proposal; E-N06).

### T-24a-031 — Wi-Fi-only policy mapped to Linux network types
- Priority: P1
- Effort: M
- Depends: T-24a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadJob.kt
- Acceptance:
  - NetworkManager `State` (zbus) gives online/offline; `PrimaryConnection` `Type` `802-11-wireless` = Wi-Fi, `802-3-ethernet` = wired (allowed under Wi-Fi-only, fixes E-N02).
  - Offline → stop with "No network connection available"; Wi-Fi-only and not Wi-Fi → stop with "No Wi-Fi connection available".
  - Fallback when NetworkManager is unavailable: `/sys/class/net/*/operstate` and `/sys/class/net/*/wireless`.
  - Debounced 2 s before stopping (E-N04).

### T-24a-032 — Pending deletion store (remove after watched)
- Priority: P1
- Effort: M
- Depends: T-24a-009, area:04-db-anime-schema-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadPendingDeleter.kt
- Acceptance:
  - Table `anime_download_pending_delete` (data §5.2) with dedupe by episode id.
  - `add_episodes` dedupes within the batch and against existing rows (fixes E-P02).
  - `take_for_installed` returns groups and deletes only rows whose source is installed (fixes E-P01).

### T-24a-033 — Deletion filters: bookmarked and excluded categories
- Priority: P1
- Effort: S
- Depends: T-24a-032, area:08-domain-library-category-history-updates, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - Category rule: if the anime is in a category listed in `remove_exclude_anime_categories`, watched episodes are excluded from deletion; empty category list → category 0 (behaviour §3.2).
  - Bookmark rule: bookmarked episodes excluded unless `pref_remove_bookmarked`.
  - Both rules re-checked at flush time (fixes E-P07).

### T-24a-034 — deleteEpisodes, deleteAnime and source cleanup cascade
- Priority: P1
- Effort: M
- Depends: T-24a-006, T-24a-010, T-24a-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - Delete order: filter, remove queued items for those episodes, delete dirs, update cache, delete anime dir if empty, delete source dir if empty (behaviour §6.2-§6.3).
  - Deleting an anime pauses the runner before removing its queued items and resumes after (fixes E-D02).
  - Deleting a currently DOWNLOADING episode cancels it and deletes partial output (E-D01).

### T-24a-035 — Downloaded-episode index cache
- Priority: P1
- Effort: L
- Depends: T-24a-006, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadCache.kt
- Acceptance:
  - In-memory index keyed by source id, then anime dir name, holding episode names; `_tmp` folders excluded; `.mkv` and `.mp4` files recorded by stem (behaviour §8.5).
  - Folders with no installed source are ignored.
  - Index reads never block on writes; iteration works on a snapshot (fixes E-D07).
  - Snapshot persisted as bincode under `$XDG_CACHE_HOME/kurobayomi/dl_anime_index_v3.bin` (data §4.2).

### T-24a-036 — Cache renewal, invalidation and debounced persistence
- Priority: P1
- Effort: S
- Depends: T-24a-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadCache.kt
- Acceptance:
  - Renewal when older than 1 h; invalidation on storage-root change, deleting the snapshot and renewing.
  - Source wait timeout 30 s; snapshot write debounced 1000 ms and atomic (temp file plus rename).
  - `is_initialising` flag debounced 1000 ms for UI.

### T-24a-037 — Storage counts and sizes
- Priority: P1
- Effort: S
- Depends: T-24a-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadCache.kt
- Acceptance:
  - Total episode count, total size, per-anime count and per-anime size from the index (behaviour §6.1).
  - Size is a recursive sum of file lengths (files of length 0 contribute 0).

### T-24a-038 — Local-source archive counting
- Priority: P2
- Effort: S
- Depends: area:14-source-local-and-metadata, T-24a-037
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - For the local anime source id, count supported archive files in the anime folder rather than the index (behaviour §6.1).

### T-24a-039 — Offline playback of a downloaded episode
- Priority: P1
- Effort: S
- Depends: T-24a-006, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - `build_video` returns a `Video` with `initialized = true`, `status = Ready`, URL `file://<path>` and title `download: <url>`.
  - Picks the first playable file after sorting by name, preferring `.mkv`, then `.mp4`, `.webm`, `.m4v`, `.mov`, `.avi`, `.ts` (behaviour §6.6, data §4.3).
  - No playable file → `NoVideo` ("No video found").

### T-24a-040 — Download queue page: source groups and headers
- Priority: P1
- Effort: M
- Depends: T-24a-010, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHeaderItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHeaderHolder.kt
- Acceptance:
  - Groups by source in first-occurrence order; header text `"<source name> (<count>)"` (ui §1.3–§1.4).
  - Empty queue shows "No downloads" (`information_no_downloads`).
  - Tab title is the Anime label with a count pill equal to the total queued items.

### T-24a-041 — Download item row: progress, text, state
- Priority: P1
- Effort: M
- Depends: T-24a-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadItem.kt
- Acceptance:
  - Row shows anime title (line 1) and episode name (line 2), a progress bar (indeterminate at 0), and text "Downloading…" at 0 or "N%" (ui §1.5).
  - Text "Looking for video…" while hoster resolution is running (fixes E-U05; proposal).
  - Row updates from `watch` receivers without rebinding (rust design §10).

### T-24a-042 — Item menu actions and visibility rules
- Priority: P1
- Effort: M
- Depends: T-24a-041, T-24a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt, app/src/main/res/menu/download_single.xml
- Acceptance:
  - Menu order and labels per ui §1.6 (move to top, move series to top, move to bottom, move series to bottom, cancel, cancel all for series).
  - Single moves stay in the group; series moves cross groups (behaviour §2.11, ui §1.6).
  - "Move to top" hidden for the first item in its own group; "Move to bottom" hidden for the last in its own group (fixes E-U07).

### T-24a-043 — Drag reorder within source groups
- Priority: P1
- Effort: M
- Depends: T-24a-041
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadAdapter.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHeaderHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt
- Acceptance:
  - Drag handle only; drop rejected across source groups (`shouldMove` rule).
  - Release computes the flat order across groups and sends `Reorder` (ui §1.7).
  - Visible drop indicator; no group collapse animation (ui §6).

### T-24a-044 — Footer Stop/Continue action and empty state
- Priority: P1
- Effort: S
- Depends: T-24a-040, T-24a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueTab.kt
- Acceptance:
  - Action shows "Stop" when running and "Continue" when stopped on the anime tab; shown only when the list is non-empty (ui §7).
  - Empty state uses the message from T-24a-040.

### T-24a-045 — Queue sort helper and sort options
- Priority: P1
- Effort: S
- Depends: T-24a-040, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt
- Acceptance:
  - `reorder_queue(selector, reverse)` sorts within groups and sends `Reorder` (ui §1.10).
  - Options: "By upload date" (Newest/Oldest, `date_upload`) and "By episode number" (Ascending/Descending, `episode_number`), as in the spec.

### T-24a-046 — Queue and status event streams to the UI
- Priority: P1
- Effort: S
- Depends: T-24a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt
- Acceptance:
  - Snapshot watch channel updates on every queue change; per-download status and progress receivers feed rows.
  - Status changes emit only after the initial value (behaviour §2.14 semantics).
  - UI rebuilds at most once per 200 ms.

### T-24a-047 — Drop dead ensureSuccessfulAnimeDownload
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - Function is never called (verified); the port does not implement it. Its logic is covered by T-24a-016.

### T-24a-048 — Drop Android background-job, foreground and channel plumbing
- Priority: P3
- Effort: S
- Depends: T-24a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadJob.kt
- Acceptance:
  - WorkManager, `ForegroundInfo`, `FOREGROUND_SERVICE_TYPE_DATA_SYNC`, and notification channel setup are not ported. Replacement: the runner in T-24a-030 and the notifier in T-24a-026. The user loses no visible behaviour.

### T-24a-049 — Fix: isDownloadSuccessful ignores .tmp only by name
- Priority: P1
- Effort: S
- Depends: T-24a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - Success = exactly one regular entry in staging whose name does not end in `.tmp`, after leftover `.tmp` files are removed (fixes E-F06).
  - Test: `.mkv` alone is success; `.mkv` + `.tmp` is success after cleanup; two `.mkv` is failure.

### T-24a-050 — Fix: job busy loop and missing network-resume
- Priority: P1
- Effort: S
- Depends: T-24a-031, T-24a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadJob.kt
- Acceptance:
  - No busy loop; the runner waits on channels only.
  - If the runner stopped because of network loss and the user had started it, it restarts when the network is back (fixes E-N01, E-S06).
  - CPU use idle with an empty queue is 0 (E-N05).

### T-24a-051 — Fix: download identity must not depend on mutable video
- Priority: P1
- Effort: S
- Depends: T-24a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt
- Acceptance:
  - Downloads are keyed by episode id; no `Hash`/`Eq` derived over `video`.
  - Test: setting `video` after insertion into any map keeps lookups working.

### T-24a-052 — Fix: update-queue equality short-circuit
- Priority: P2
- Effort: S
- Depends: T-24a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - Reorder to an identical order is a no-op (the Kotlin comparison is always false, behaviour §2.11).

### T-24a-053 — Fix: pending deleter loses entries
- Priority: P1
- Effort: S
- Depends: T-24a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadPendingDeleter.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - Entries for uninstalled sources stay queued (fixes E-P01).
  - Corrupt entries move to a `corrupt` log table and are not silently cleared (fixes E-P04).

### T-24a-054 — Settings wiring: anime download preference keys and defaults
- Priority: P1
- Effort: M
- Depends: area:25-downloads-shared, area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt
- Acceptance:
  - Keys and defaults exactly as data §3: `pref_download_only_over_wifi_key` true, `use_external_downloader` false, `external_downloader_selection` "", `pref_download_fillermarked` false, `pref_remove_bookmarked` false, `remove_exclude_anime_categories` empty, `hide_notification_content` false.
  - A key-set test fails if any key differs from data §3.

### T-24a-055 — download_slots and download_speed_limit for anime: decide and implement
- Priority: P2
- Effort: S
- Depends: T-24a-054
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - Decision recorded: anime ignores `download_slots` (fixed at 3 sources, 1 per source) and the speed limit is hidden for anime, or implemented with the ffmpeg `-maxrate` only if a setting is added. The chosen option is documented in the settings screen help text.
  - Default `download_slots = 1` is not applied to anime (behaviour §11).

### T-24a-056 — Write .nomedia in the final episode directory
- Priority: P2
- Effort: S
- Depends: T-24a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - `.nomedia` (empty file) exists inside each finished episode dir.
  - Test verifies presence after finalise (behaviour §5.9).

### T-24a-057 — Import an existing Aniyomi download folder
- Priority: P2
- Effort: M
- Depends: T-24a-035, T-24a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadCache.kt
- Acceptance:
  - Existing `<source (LANG)>/<anime>/<episode>` trees are indexed without modification, including legacy names and `.mp4` files.
  - Source folders whose name matches an installed source (case-insensitive) are recognised.

### T-24a-058 — Concurrency model and cancellation semantics
- Priority: P1
- Effort: M
- Depends: T-24a-010, T-24a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadCache.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadPendingDeleter.kt
- Acceptance:
  - One tokio runtime; GTK never awaits the engine (rust design §3).
  - Cancellation token per running download; ffmpeg child killed and waited on cancel (rust design §14).
  - No `runBlocking` on the UI thread (behaviour §17).

### T-24a-059 — Filesystem abstraction, SAF content URIs dropped
- Priority: P1
- Effort: S
- Depends: T-24a-004
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FFmpegUtils.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt
- Acceptance:
  - All paths are `PathBuf`; no `content://` handling. Reason: Android SAF has no Linux equivalent; users keep normal folders. User impact: a SAF-only downloads root must be re-selected as a normal folder.
  - Root selection rejects non-local paths with a message.

### T-24a-060 — Logging with tracing and header redaction
- Priority: P2
- Effort: S
- Depends: T-24a-058
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - `logcat` replaced with `tracing`; ffmpeg messages at warning and above logged at error (behaviour §5.7).
  - Headers, cookies and query strings redacted in logs (edge-cases §6).

### T-24a-061 — Test suite for the download engine
- Priority: P1
- Effort: M
- Depends: T-24a-004, T-24a-011, T-24a-015, T-24a-018, T-24a-032, T-24a-035, T-24a-049
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadPendingDeleter.kt
- Acceptance:
  - All unit tests in edge-cases §4.1 and integration tests in §4.2 pass (`cargo test`).
  - Fixtures in edge-cases §5 are checked in under `tests/fixtures/dl/`.

### T-24a-062 — Startup sweep of orphaned staging dirs and partial files
- Priority: P1
- Effort: S
- Depends: T-24a-009, T-24a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadStore.kt
- Acceptance:
  - At startup, `_tmp` dirs whose episode is not in the restored queue are removed (E-F05). A `.mkv` inside a restored queue item's staging dir is kept for resume.
  - Test with a pre-populated tree.

### T-24a-063 — Drop MediaScanner indexing
- Priority: P3
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - `MediaScannerConnection.scanFile` is not ported. Replacement: `.nomedia` (T-24a-056). User impact: Linux media indexers pick up the files only if they are not hidden by `.nomedia`; this matches the Android hiding behaviour.

### T-24a-064 — Notification actions mapped to desktop actions
- Priority: P2
- Effort: S
- Depends: T-24a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - Progress notification offers "Stop" (pause) and "Show anime"; paused offers "Resume" and "Cancel all"; errors offer "Show anime" when an anime id is known.
  - Actions are handled inside the app (no broadcast receiver).

### T-24a-065 — Notification open-entry and help-link actions
- Priority: P2
- Effort: S
- Depends: T-24a-026, area:39-ui-entry-details-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - "Show anime" opens the anime details page; the queue-size warning opens the help URL in the default browser (`xdg-open`).
  - Notification default action opens the downloads page.

### T-24a-066 — Finalise collision handling
- Priority: P1
- Effort: M
- Depends: T-24a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt
- Acceptance:
  - If the target episode dir exists and is non-empty at finalise, the download becomes ERROR "Episode already exists" and staging is kept (fixes E-F02).
  - Duplicate episode names under one anime get a unique directory name suffix ` (<episode id>)` for new downloads (fixes E-F03; proposal).

### T-24a-067 — ffmpeg presence check and missing-binary error
- Priority: P1
- Effort: S
- Depends: T-24a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - On startup, `ffmpeg` and `ffprobe` are located on `PATH`; if missing, downloads fail with `FfmpegMissing` and Settings shows a link (E-F08).
  - Empty ffmpeg output (size 0) after exit 0 is an error "Error in ffmpeg!" (E-F07).

### T-24a-068 — startDownloadNow guard for already-downloaded episodes
- Priority: P1
- Effort: S
- Depends: T-24a-010, T-24a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - If the episode is already on disk, `StartNow` returns "already downloaded" and does not queue (E-Q06).

### T-24a-069 — Enqueue feedback when the runner is stopped
- Priority: P2
- Effort: S
- Depends: T-24a-012, T-24a-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - When items are queued while the queue was non-empty and the runner is stopped, show a toast "Queued. Press Continue to start." (E-Q07).

### T-24a-070 — Header sanitising for external and ffmpeg inputs
- Priority: P1
- Effort: S
- Depends: T-24a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - Header names and values are validated: names are token characters only; values lose CR, LF and NUL (E-F12, E-F13).
  - Invalid headers are dropped with a debug log.

### T-24a-071 — Rename-failure and cross-device handling
- Priority: P2
- Effort: S
- Depends: T-24a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - `EXDEV` on staging rename produces an error with a clear message; no copy fallback (E-F16).
  - Source and episode rename failures are surfaced as warnings, not silent logs (E-F23).

### T-24a-072 — Long filename byte limit
- Priority: P1
- Effort: S
- Depends: T-24a-004
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - File component (`<title> - <episode>_tmp.mkv` and `<name>.mkv`) limited to 200 UTF-8 bytes on a char boundary (E-F17).

### T-24a-073 — Mockup entry points wired to the engine
- Priority: P1
- Effort: M
- Depends: T-24a-010
- Sources: src/screens.rs, src/entry.rs
- Acceptance:
  - `src/screens.rs:354` `downloads()` builds from the live snapshot instead of "Source A/B" fixtures.
  - `download_button()` (screens.rs:134) items "Start downloading now", "Cancel", "Delete" call `StartNow`, `Cancel`, `DeleteEpisodes`.
  - Entry menu "Download" / "Delete" (entry.rs:171, :196) call `Enqueue` / `DeleteEpisodes`.

### T-24a-074 — i18n keys for new messages
- Priority: P1
- Effort: S
- Depends: area:51-i18n, T-24a-012
- Sources: src/settings.rs
- Acceptance:
  - Keys added for: "Episode already exists", "ffmpeg is not installed", "Torrent server is not available", "Queued. Press Continue to start.", "Finished with %d errors", "Downloads will pause. Quit?", "Looking for video…", "Download location" label.
  - Existing keys from behaviour §14 reused unchanged.
