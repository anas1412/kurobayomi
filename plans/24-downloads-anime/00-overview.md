# 24-downloads-anime: overview

Area lead summary for anime downloads in Aniyomi and the Kurobayomi port. Source paths are repo-relative to `/home/blackbox/aniyomi-src` (Apache-2.0, read-only). Sibling area for manga is area:23-downloads-manga; shared download preferences and the queue tab host are area:25-downloads-shared.

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

Anime downloads let the user save an episode's video to local storage so it plays offline. The engine:

- Keeps a persistent, ordered queue of episodes (`AnimeDownload`), stored across restarts.
- Schedules at most one active download per source, with at most three sources active at once (`AnimeDownloader.kt`, see behaviour section 2.3).
- Resolves the best playable `Video` for an episode through the hoster loader (area:45-player-core), then fetches it over HTTP, over a torrent server, or through an external downloader.
- Remuxes HLS and other streams with ffmpeg (FFmpegKit on Android) into an `.mkv` with subtitles and audio tracks, then finalises the staging directory into the episode folder.
- Maintains an in-memory and on-disk index of downloaded episodes (`AnimeDownloadCache.kt`) used by the library, entry details, player and storage screens.
- Supports deletion (per episode, per anime, per source), rename on metadata change, deferred deletion after watched ("remove after watched"), and offline playback through `buildVideo`.
- Shows a Downloads queue page with source groups, drag reorder, per-item menus, and a footer Stop/Continue action.
- Reports progress and warnings through Android notifications, gated by a Wi-Fi-only preference.

### 1.2 Why it matters for a usable Kurobayomi (PROPOSAL)

- Offline viewing is the main reason a daily user keeps a local anime library. Without downloads the player only streams.
- The downloaded-episode index is read by the player (`EpisodeLoader.kt` calls `isEpisodeDownloaded` and `buildVideo`), by Updates (`AnimeUpdatesScreenModel.kt`), by Storage (`ui/storage/anime/AnimeStorageScreenModel.kt`), and by the episode-sync path when a source renames an episode (`domain/items/episode/interactor/SyncEpisodesWithSource.kt:119,127`). Losing the index would break those screens, not only downloads.
- Priority: this area has no P0 task. Streaming playback is P0 and does not depend on downloads. Every download task is P1, P2 or P3 (section 6).

### 1.3 Scope boundaries

In scope: the 18 files listed in [p1-inventory.md](p1-inventory.md), 3,626 LOC, all under `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/` and `app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/`.

Out of scope (owned elsewhere, referenced only for exact behaviour): the list in section "Cross-area files referenced" of [p1-inventory.md](p1-inventory.md).

Not in scope: manga downloads (area:23-downloads-manga), the shared `DownloadPreferences` and Downloads tab host (area:25-downloads-shared).

## 2. Module map

### 2.1 Kotlin packages and responsibilities (OBSERVED)

| Kotlin package / path | Files (LOC) | Responsibility |
|---|---|---|
| `data/download/anime/` | `AnimeDownloader.kt` (947), `AnimeDownloadManager.kt` (466), `AnimeDownloadCache.kt` (492), `AnimeDownloadStore.kt` (149), `AnimeDownloadPendingDeleter.kt` (187), `AnimeDownloadProvider.kt` (186), `AnimeDownloadJob.kt` (132), `AnimeDownloadNotifier.kt` (237) | Engine (queue, scheduler, pipeline, retry, ffmpeg, torrent, external), public facade, persistence of queue and pending deletes, disk index, directory layout, WorkManager job, notifications |
| `data/download/anime/model/` | `AnimeDownload.kt` (84), `AnimeDownloadPart.kt` (60) | Queue element and its status enum; one dead class |
| `ui/download/anime/` | `AnimeDownloadQueueTab.kt` (44), `AnimeDownloadQueueScreen.kt` (93), `AnimeDownloadQueueScreenModel.kt` (242), `AnimeDownloadAdapter.kt` (27), `AnimeDownloadHeaderHolder.kt` (37), `AnimeDownloadHeaderItem.kt` (64), `AnimeDownloadHolder.kt` (111), `AnimeDownloadItem.kt` (68) | Queue page: tab host, Compose screen, screen model, list adapter, group header and item rows |

Totals: 18 files, 3,626 LOC. Decision counts: Port 5, Rewrite 10, Replace-with-crate 2, Drop 1 (plus one dead function inside `AnimeDownloader.kt`). Details per file in [p1-inventory.md](p1-inventory.md).

### 2.2 Key types (OBSERVED)

- `AnimeDownload` (`data/download/anime/model/AnimeDownload.kt:16-22`): `source`, `anime`, `episode`, `changeDownloader: Boolean = false`, `video: Video? = null`. Implements `ProgressListener`.
- `AnimeDownload.State` (`model/AnimeDownload.kt:62-68`): `NOT_DOWNLOADED(0)`, `QUEUE(1)`, `DOWNLOADING(2)`, `DOWNLOADED(3)`, `ERROR(4)`.
- `AnimeDownloader` (`AnimeDownloader.kt`): owns `_queueState` (`:100`) and the scheduler.
- `AnimeDownloadManager` (`AnimeDownloadManager.kt`): the facade every other area calls.

### 2.3 Rust module map (PROPOSAL)

One crate (`kurobayomi`), no new crate. Engine code under `src/downloads/` must not import `gtk`. The GTK page lives beside it. Full layout with per-file behaviour references is in [p1-rust-design.md](p1-rust-design.md) section 1.

```
src/downloads/
  mod.rs          facade: DownloadManager (public API)
  model.rs        Download, DlState, episode/anime/source refs
  error.rs        DlError (user-facing text)
  store.rs        SQLite anime_download_queue
  pending.rs      SQLite anime_download_pending_delete
  layout.rs       directory naming and lookups (pure)
  cache.rs        downloaded-episode index, renewal, snapshot
  scheduler.rs    per-source scheduling
  engine.rs       per-download pipeline
  retry.rs        2/4/8 s retry schedule, 4 attempts
  ffmpeg.rs       argv builder, ffprobe, progress parser, child driver
  torrent.rs      torrent-server path (feature-gated, P2)
  external.rs     external-handoff (P3)
  lifecycle.rs    runner state, network gating, start/stop
  network.rs      NetworkManager via zbus, sysfs fallback
  notify.rs       notify-rust notifications
src/downloads_page.rs   GTK page: groups, rows, menus, drag and drop, footer action
```

Rust-to-Kotlin mapping:

| Kotlin file | Rust module |
|---|---|
| `AnimeDownloader.kt` | `engine`, `scheduler`, `retry`, `ffmpeg`, `torrent`, `external` |
| `AnimeDownloadManager.kt` | `mod` (facade) |
| `AnimeDownloadCache.kt` | `cache` |
| `AnimeDownloadStore.kt` | `store` |
| `AnimeDownloadPendingDeleter.kt` | `pending` |
| `AnimeDownloadProvider.kt` | `layout` |
| `AnimeDownloadJob.kt` | `lifecycle`, `network` |
| `AnimeDownloadNotifier.kt` | `notify` |
| `model/AnimeDownload.kt` | `model` |
| `model/AnimeDownloadPart.kt` | none (dropped) |
| `ui/download/anime/*` (8 files) | `src/downloads_page.rs` |

## 3. Reading guide

Read in this order: inventory, behaviour, data, rust design, then the topics, UI parity, edge cases, and finally tasks.

- [p1-inventory.md](p1-inventory.md): every in-scope file with LOC, every top-level declaration, port decision, target Rust module and task IDs. Also lists the cross-area files the spec depends on.
- [p1-behavior.md](p1-behavior.md): the exact observable behaviour of the engine, manager, job, provider, cache, pending deleter and notifier, with `path:line` citations, constants, timings, message keys and ordering rules. Sections 1-19.
- [p1-data-and-config.md](p1-data-and-config.md): in-memory and persisted models, enum integer values, preference keys and defaults, disk layout and file formats, SQL tables (`anime_download_queue`, `anime_download_pending_delete`), network endpoints, and the field-to-column mapping.
- [p1-rust-design.md](p1-rust-design.md): Rust module layout, crate choices with reasons, threading and async model, core types, traits to the outside world, scheduler and per-download pipeline design, retry, persistence, GTK integration, error type, pitfalls and testing hooks.
- [p1-topic-ffmpeg-remux.md](p1-topic-ffmpeg-remux.md): why remux instead of re-encode, the header string, the exact ffmpeg argv order, expected output, the ffprobe call, progress parsing, failure and cancel behaviour, and a test checklist.
- [p1-topic-storage-cache-lifecycle.md](p1-topic-storage-cache-lifecycle.md): on-disk states of one episode, naming inside an anime folder, what each operation does to disk, the index's structure and consistency rules, known weak points, and compatibility with an existing Aniyomi download folder.
- [p1-ui-parity.md](p1-ui-parity.md): anatomy of the Download queue page as implemented, screen-model lifecycle, keyboard and mouse equivalents, desktop additions, responsive behaviour, gap analysis against the current mockup, and the strings used.
- [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge cases per area (queue, scheduler, staging, external and torrent, network, pending deletion, deletion cascade, cache, notifications, preferences), concurrency hazards, failure-mode matrix, and the test plan with fixtures.
- [p1-tasks.md](p1-tasks.md): 74 implementation tasks T-24a-001 to T-24a-074 with priority, effort, dependencies, sources and acceptance criteria.

## 4. Key design decisions and open questions

Each item needs the user's call. The recommendation is the default the plan already assumes unless the user says otherwise.

1. **Background operation without a background service.** Aniyomi's `AnimeDownloadJob` is a WorkManager foreground worker. Kurobayomi has no equivalent. Decision: the downloader is a tokio runtime task that lives as long as the app process (T-24a-030, T-24a-048). Recommendation: keep it. Closing the window while downloads run shows "Downloads will pause. Quit?" unless the user disabled that prompt (proposal, E-N06). Open question for the user: do you want downloads to continue with the window hidden to a tray? Recommendation: no tray in this scope, so the answer is moot until a tray exists.

2. **Pause discards partial output.** Kotlin deletes the partial `.tmp` on pause, failure or cancel (behaviour sections 5.2 and 5.5). Resume is not implemented. Recommendation: keep Kotlin behaviour for v1. Changing it means updating T-24a-016 and the edge-case document (see [p1-rust-design.md](p1-rust-design.md) section 14, pitfall 4).

3. **Torrent-server path (T-24a-023, P2, L).** Requires a running torrent server (`TorrentServerService`, area:13-core-util-storage-archive-torrent). Recommendation: defer until the user needs torrent-backed sources. Keep the enum variant and trait hook so it does not change the public API later.

4. **External downloader (T-24a-024, P3, M, and T-24a-025, P2, S).** Android intents for 1DM and ADM have no Linux equivalent, and the user loses them. Replacement proposed: aria2 over JSON-RPC, off by default, shown only when enabled. Recommendation: build only if the user runs aria2. Otherwise drop with the reason stated.

5. **`download_slots` and `download_speed_limit` for anime (T-24a-055, P2, S).** Anime ignores `download_slots` (fixed at 3 sources, 1 per source) and the speed limit is hidden. Alternative: implement the speed limit with ffmpeg `-maxrate` only if a setting is added. Recommendation: hide the speed limit and document the fixed slot policy in the settings help text.

6. **Import an existing Aniyomi download folder (T-24a-057, P2, M).** Only useful if the user copies downloads from an Android device. Recommendation: build it, because the index already has to read the same layout (T-24a-005 and T-24a-006). Ask the user whether they have such a folder.

7. **Wi-Fi-only default (`pref_download_only_over_wifi_key`, default `true`, `DownloadPreferences.kt:9-12`).** On a desktop most connections are wired. Plan: wired ethernet counts as allowed under the Wi-Fi-only preference (T-24a-031, fixes E-N02). Ask the user whether wired connections should also be blocked. Recommendation: keep the planned behaviour.

8. **ffmpeg is an external binary on PATH.** No bundled ffmpeg. Missing binary gives a clear error (T-24a-067). Recommendation: keep. The user must have ffmpeg installed on PATH.

9. **SAF and content URIs dropped (T-24a-059).** Only plain paths. The user loses nothing on desktop because the download root is a normal folder.

10. **Notifications: `notify-rust` over `org.freedesktop.Notifications`.** The `hide_notification_content` behaviour (T-24a-028, P2) hides titles in notifications. Recommendation: keep both the in-app status bar and desktop notifications, because KDE and GNOME both support action buttons.

11. **Dropped Android-only items.** `AnimeDownloadPart` (T-24a-003), `ensureSuccessfulAnimeDownload` (T-24a-047), background-job and channel plumbing (T-24a-048), and MediaScanner indexing (T-24a-063). Nothing the user uses is lost. `.nomedia` is still written for interop (T-24a-056, P2).

12. **Known plan defect: dependency cycle.** `T-24a-016` depends on `T-24a-049` and `T-24a-049` depends on `T-24a-016`. Both are in [p1-tasks.md](p1-tasks.md). Neither task can start while the other is open. Not changed in this pass because the task IDs and dependency intent are the author's call. Recommendation: make `T-24a-049` a predecessor of `T-24a-016` (fix the predicate first, then the finalise step uses it) and drop the reverse edge. Needs the author's decision before scheduling.

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What is needed | Tasks |
|---|---|---|
| area:01-foundation-app-bootstrap | Shared JSON settings (`ignoreUnknownKeys = true; explicitNulls = false`, `AppModule.kt`) reused for cache snapshot and any JSON | T-24a-035 |
| area:02-preferences-system | Preference store for the download keys, `hideNotificationContent` (`SecurityPreferences.kt`) | T-24a-028, T-24a-054 |
| area:04-db-anime-schema-migrations | SQLite migration to add `anime_download_queue` and `anime_download_pending_delete` | T-24a-009, T-24a-032 |
| area:05-data-repos-entries-items | Episode repository and episode name fields for directory rename on metadata change | T-24a-007 |
| area:07-domain-entries-items | Episode fields used for dedupe, naming and deletion filters; sort options for the queue | T-24a-045 |
| area:08-domain-library-category-history-updates | Category membership for the delete-time exclusion filter | T-24a-033 |
| area:12-network-stack | HTTP client for the download request, `ProgressListener` semantics, and a local HTTP server for localhost video URLs | T-24a-022 |
| area:13-core-util-storage-archive-torrent | `buildValidFilename` (DiskUtil), free-space query (replace `StatFs` with `rustix`), `.nomedia` creation, recursive size (`UniFile.size()` replacement), `String.chop`, `FFmpegUtils` argument quoting, torrent server | T-24a-006, T-24a-017, T-24a-023, T-24a-035, T-24a-038 |
| area:14-source-local-and-metadata | Local source archive handling for counting (`T-24a-038`) | T-24a-038 |
| area:16-source-api-anime | `Video` model and fields (`Video.kt:30-46`), hoster resolution contract, `usesHttpServer` | T-24a-014, T-24a-022 |
| area:17-source-app-layer | Source identity (name and language) used for the source-folder rename | T-24a-008 |
| area:19-extensions-anime | Installed-source enumeration for the import and the "Unknown source" storage display | T-24a-057 |
| area:25-downloads-shared | `DownloadPreferences` keys and defaults, the shared Downloads tab host (`DownloadsTab.kt`), queue sort dropdown and the delete entry point (`DeleteEpisodeDownload.kt`) | T-24a-012, T-24a-033, T-24a-040, T-24a-054 |
| area:26-library-update-jobs | The library-update path calls `downloadEpisodes(anime, episodes, false)` and `startDownloads()` (`AnimeLibraryUpdateJob.kt:334,350`). Its trigger contract is owned there | T-24a-012 |
| area:36-ui-shell-navigation-security-webview | Window close and quit confirmation; the shell that hosts the page | T-24a-030 |
| area:39-ui-entry-details-anime | Episode options: download, delete, "Remove after watched" entry points (`EpisodeOptionsDialogScreen.kt` and `AnimeScreenModel.kt` call the manager) | T-24a-065 |
| area:42-ui-updates-history-stats-storage-upcoming | Storage screen reads counts and sizes (`AnimeStorageScreenModel.kt`) | T-24a-037 |
| area:45-player-core | `HosterLoader.getBestVideo`, `EpisodeLoader.getHosters` and `loadHosterVideos`, offline playback via `buildVideo`, and the player's use of `isEpisodeDownloaded` (`PlayerViewModel.kt:1157,1164`) | T-24a-014, T-24a-039 |
| area:51-i18n | Message keys for new strings | T-24a-074 |
| area:53-platform-manifest-intents-workers | Broadcast actions for pause, resume, clear and open-entry (`NotificationReceiver.kt`), replaced by desktop actions | T-24a-064, T-24a-065 |

### 5.2 What others need from this area

| Consumer | What it calls | Source |
|---|---|---|
| area:25-downloads-shared | Manager API: `startDownloads`, `pauseDownloads`, `clearQueue`, `reorderQueue`, `cancelQueuedDownloads`, queue and status streams | `AnimeDownloadManager.kt` |
| area:26-library-update-jobs | `downloadEpisodes(anime, episodes, autoStart)`, `startDownloads()` | `AnimeLibraryUpdateJob.kt:334,350` |
| area:39-ui-entry-details-anime | `downloadEpisodes`, `deleteEpisodes`, `enqueueEpisodesToDelete`, `isEpisodeDownloaded` | `AnimeScreenModel.kt`, `EpisodeOptionsDialogScreen.kt` |
| area:42-ui-updates-history-stats-storage-upcoming | Index counts and sizes; `isEpisodeDownloaded`; the Storage screen's "Unknown source" view | `AnimeStorageScreenModel.kt`, `AnimeStatsScreenModel.kt` |
| area:45-player-core | `isEpisodeDownloaded`, `buildVideo` for offline playback, `addDownloadsToStartOfQueue`, `downloadEpisodes` from the player, `deletePendingEpisodes` | `EpisodeLoader.kt:50,110-112`, `PlayerViewModel.kt:1101,1157,1164,1731,1967,1977` |
| area:07-domain-entries-items | `renameEpisode` is triggered by episode sync when the source renames an episode | `SyncEpisodesWithSource.kt:119,127` |
| area:19-extensions-anime and area:17-source-app-layer | `renameSource` when a source is renamed | `AndroidAnimeSourceManager.kt:112` |
| area:36-ui-shell-navigation-security-webview | Downloads runner state for the close confirmation (`isDownloaderRunning`) | T-24a-030 |

## 6. Task summary

Source: [p1-tasks.md](p1-tasks.md), exported to `plans/TASKS.csv` by `python3 plans/_tools/tasks.py export`. The lint is clean (`python3 plans/_tools/tasks.py lint plans/24-downloads-anime/p1-tasks.md` prints `ok`).

### 6.1 Counts (74 tasks)

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 0 | 0 | 0 | 0 | 0 |
| P1 | 29 | 23 | 3 | 0 | 55 |
| P2 | 12 | 1 | 1 | 0 | 14 |
| P3 | 4 | 1 | 0 | 0 | 5 |
| Total | 45 | 25 | 4 | 0 | 74 |

### 6.2 P0 critical path

No task in this area is P0. The P1 critical path below is the order an implementer should follow to reach a working download-and-play loop. Each step lists its blocking predecessors.

1. `T-24a-001` AnimeDownload model and state machine (no deps). Then in parallel: `T-24a-004` naming, `T-24a-015` retry policy, `T-24a-018` ffmpeg argv builder (no deps), `T-24a-051` identity by episode id.
2. `T-24a-002` progress semantics, after `T-24a-001`.
3. `T-24a-011` scheduler (needs `T-24a-001`, `T-24a-002`).
4. `T-24a-006` directory discovery (needs `T-24a-004`, area:13) and `T-24a-009` SQLite queue store (needs `T-24a-001`, area:04).
5. `T-24a-035` downloaded-episode index (needs `T-24a-006`, area:13), then `T-24a-036` and `T-24a-037`.
6. `T-24a-010` manager API and command channel (needs `T-24a-009`, `T-24a-011`).
7. `T-24a-016` staging, remux completion and finalise (needs `T-24a-004`, `T-24a-006`, `T-24a-035`, `T-24a-049`). Blocked by the cycle in section 4, item 12.
8. `T-24a-014` best-video resolution (needs area:45, area:16, `T-24a-011`), and `T-24a-022` local HTTP server for localhost URLs (needs area:12, area:16).
9. `T-24a-039` offline playback (needs `T-24a-006`, area:45).
10. `T-24a-012` enqueue filtering and ordering (needs `T-24a-006`, `T-24a-010`, area:25).
11. `T-24a-030` runner lifecycle, then `T-24a-031` Wi-Fi-only policy.
12. `T-24a-032` pending-delete store and `T-24a-033` filters, then `T-24a-034` delete cascade.
13. UI: `T-24a-040` queue page, `T-24a-041` item row, `T-24a-042` menu, `T-24a-043` drag reorder, `T-24a-044` footer and empty state, `T-24a-045` sort, `T-24a-046` event streams, `T-24a-073` mockup wiring.
14. Last: `T-24a-061` test suite (needs `T-24a-004`, `T-24a-011`, `T-24a-015`, `T-24a-018`, `T-24a-032`, `T-24a-035`, `T-24a-049`).

Parallel branch for ffmpeg: `T-24a-018` then `T-24a-019` (progress and ffprobe), `T-24a-020` (headers), `T-24a-021` (tracks), `T-24a-067` (missing binary). `T-24a-070` (header sanitising) depends on `T-24a-020`.

### 6.3 Distribution by file

Tasks are in one file, [p1-tasks.md](p1-tasks.md). Tasks T-24a-001 to T-24a-074 cover sections A (model, naming, layout), B (queue, persistence, scheduling), C (notifications, lifecycle, cache, UI, fixes, settings, tests).

### 6.4 Risks

1. **Dependency cycle** `T-24a-016` <-> `T-24a-049`: blocks finalise, the success check, and the test suite. Resolve before scheduling (section 4, item 12).
2. **ffmpeg behaviour parity.** The argv order must match the Kotlin order exactly (see [p1-topic-ffmpeg-remux.md](p1-topic-ffmpeg-remux.md)). Golden tests are required (T-24a-018, T-24a-021).
3. **Filesystem semantics.** Directory rename, case-only rename of a source folder (T-24a-008), and cross-device rename (T-24a-071) are the most likely sources of data loss. Test on a real `btrfs` or `ext4` mount and on a FAT or exFAT mount.
4. **Index drift.** The on-disk snapshot can disagree with the folders when the user edits files by hand. Mitigation: renewal on storage change and on hourly timer, plus atomic snapshot writes (see [p1-rust-design.md](p1-rust-design.md) section 14, pitfall 9).
5. **Cross-area timing.** Area:45 (player) and area:25 (shared download UI) must land their contracts before T-24a-014, T-24a-039 and T-24a-040 can be finished.
6. **Dropped Android features.** External downloader intents and background-job behaviour are dropped (section 4). Users who depend on them need to be told.
7. **Mockup gap.** The current mockup in `/home/blackbox/kurobayomi/src/` has no engine. T-24a-073 wires the existing UI to the engine. Until then the page shows mock data only.

## 7. Definition of done for this area in Kurobayomi

Observable behaviours. Each one must be checkable by running the app.

1. Opening the Downloads page shows one group per source that has queued items. Each group header shows the source name and item count. The page has an empty state when the queue is empty.
2. From an entry's episode options, choosing download adds the episode to the queue as `QUEUE`. The row shows progress text and state, and the item survives an app restart with the same order.
3. At most three sources download at the same time, and only one episode per source runs at once. A fourth source waits in `QUEUE` until a slot is free.
4. Stopping and continuing from the footer moves `DOWNLOADING` items to `QUEUE` (pause) and `ERROR` items back to `QUEUE` (continue). Closing the window while downloads run shows the confirmation dialog unless it was disabled.
5. A failed episode is retried 4 times in total with delays of 2 s, 4 s and 8 s before it is marked `ERROR`. `ERROR` items stay in the queue until the user cancels them or a retry succeeds.
6. A finished episode appears as `<anime> - <episode name>/` inside `<source>/<anime>/` with a `.nomedia` file, and the Storage screen counts it. Playing it from the entry screen uses the local file with no network request.
7. Deleting an episode, an anime, or a source removes the matching folders, updates the index, and removes queued items for them (cascade rules in T-24a-034).
8. "Remove after watched" queues the deletion and runs it once the episode is watched, skipping bookmarked episodes and anime in excluded categories (per area:25 and area:08 settings).
9. With "Only download over Wi-Fi" on (default), a wired connection is allowed, a connection that is neither wired nor Wi-Fi stops the queue with the message "No Wi-Fi connection available", and no network yields "No network connection available" (T-24a-031). Stops are debounced by 2 s.
10. Downloads that need remux produce one `.mkv` with the expected subtitle and audio streams, verified by `ffprobe`. Missing ffmpeg shows a clear error before any download starts.
11. Each queue change is saved to SQLite immediately. After a crash, restart restores the queue and removes orphaned staging folders.
12. Notifications show progress, pause, completion, warning and error states in the desktop notification server, with the replace-id behaviour (one progress notification and one error notification at a time). The "hide notification content" setting hides titles.
13. The unit tests in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 4.1 pass, and the integration tests in section 4.2 pass with a fake ffmpeg.
