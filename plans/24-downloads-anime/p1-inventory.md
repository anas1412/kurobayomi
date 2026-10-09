# 24-downloads-anime: file inventory

Scope: every file printed by `areas.py files 24-downloads-anime 1` (18 files, 3,626 LOC total). All paths are repo-relative to `/home/blackbox/aniyomi-src`. No `*Test.kt` file exists in scope or in the downloads packages (verified: no test file references `AnimeDownload`).

Port decision legend:
- **Port**: behaviour is reproduced in Rust with the same semantics.
- **Rewrite**: behaviour reproduced, implementation replaced (different data structures or concurrency).
- **Replace-with-crate**: Android API replaced by a named Linux crate or system service.
- **Drop**: removed, with the reason and user impact stated in the behaviour and tasks docs.

Priority legend follows the project rules (P0 usable-daily core, P1 parity, P2 polish, P3 Android-only or dead).

## Summary counts

| Decision | Files |
|---|---|
| Port | 5 (`model/AnimeDownload.kt`, `AnimeDownloadManager.kt` (API), `AnimeDownloadStore.kt`, `AnimeDownloadPendingDeleter.kt`, `AnimeDownloadProvider.kt`) |
| Rewrite | 10 (`AnimeDownloader.kt`, `AnimeDownloadCache.kt`, and the 8 UI files under `ui/download/anime/`) |
| Replace-with-crate | 2 (`AnimeDownloadJob.kt`, `AnimeDownloadNotifier.kt`) |
| Drop | 1 file (`model/AnimeDownloadPart.kt`, dead code). Also dead function `ensureSuccessfulAnimeDownload` inside `AnimeDownloader.kt` (T-24a-047) |

Total 18 = 5 + 10 + 2 + 1.

## Per-file inventory

Columns: path | LOC | every top-level declaration (classes, objects, interfaces, enums, typealiases, top-level functions, composables, and top-level constants) | purpose | port decision with reason | target Rust module | priority | task ids.

| Path | LOC | Top-level declarations | Purpose | Port decision | Target Rust module | Priority | Task ids |
|---|---|---|---|---|---|---|---|
| app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadCache.kt | 492 | `class AnimeDownloadCache`; `private class RootDirectory`; `private class SourceDirectory`; `private class AnimeDirectory`; `private object UniFileAsStringSerializer` | In-memory and protobuf-on-disk index of downloaded episode directories per source and anime, renewed hourly or on storage change. Answers `isEpisodeDownloaded`, counts, sizes. | Rewrite. Index semantics are kept. Protobuf disk file replaced by serde + bincode snapshot; UniFile replaced by `PathBuf`. Reason: UniFile/SAF and kotlinx ProtoBuf have no Rust equivalent. | `downloads::cache` | P1 | T-24a-035, T-24a-036, T-24a-037, T-24a-038 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadJob.kt | 132 | `class AnimeDownloadJob`; companion `private const val TAG`, `fun start`, `fun stop`, `fun isRunning`, `fun isRunningFlow` | WorkManager worker that keeps the downloader alive as a foreground data-sync job and gates on network and Wi-Fi state. | Replace-with-crate. WorkManager has no Linux equivalent; the worker becomes a tokio task owned by the downloader plus NetworkManager gating over zbus. Reason: no background-job system on desktop; lifetime follows the app process. | `downloads::lifecycle` | P1 | T-24a-030, T-24a-031, T-24a-048, T-24a-050 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt | 466 | `class AnimeDownloadManager` | Public facade: enqueue, start/pause/clear, reorder, start-now, delete episodes/anime/source, rename, pending delete, counts, sizes, built video for local playback, flows for UI. | Port (API preserved, implementation Rewrite of threading to tokio). | `downloads::manager` | P1 | T-24a-010, T-24a-012, T-24a-032, T-24a-033, T-24a-034, T-24a-037, T-24a-039, T-24a-046 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadNotifier.kt | 237 | `internal class AnimeDownloadNotifier` | Builds the progress, paused, warning and error notifications (IDs -203 and -204) with action buttons. | Replace-with-crate: Android NotificationCompat replaced by `notify-rust` over org.freedesktop.Notifications D-Bus, plus in-app status bar. Reason: no NotificationCompat on Linux; actions are supported by KDE and GNOME servers. | `downloads::notify` | P1 | T-24a-026, T-24a-027, T-24a-028, T-24a-029, T-24a-064, T-24a-065 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadPendingDeleter.kt | 187 | `class AnimeDownloadPendingDeleter`; `private data class Entry`; `private data class EpisodeEntry`; `private data class AnimeEntry` | Deferred-deletion list (remove-after-watched) stored in SharedPreferences `episodes_to_delete` as JSON per anime id. | Port into SQLite table, same semantics (dedupe by episode id, clear-on-read). Reason: SharedPreferences replaced by the app database. | `downloads::pending_delete` | P1 | T-24a-032, T-24a-033, T-24a-053 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt | 186 | `class AnimeDownloadProvider` | Path scheme `<downloads root>/<source dir>/<anime dir>/<episode dir>`, name sanitising, legacy episode-dir name lookup, rename detection. | Port. Pure string logic transfers exactly; UniFile lookups replaced by `std::fs`. | `downloads::layout` | P1 | T-24a-004, T-24a-005, T-24a-006, T-24a-007, T-24a-008 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadStore.kt | 149 | `class AnimeDownloadStore`; `private data class AnimeDownloadObject` | Persists queue entries (animeId, episodeId, order) across restarts in SharedPreferences `active_downloads`, keyed by episode id. | Port into SQLite table. Reason: SharedPreferences replaced by the app database (same keyed, ordered semantics). | `downloads::store` | P1 | T-24a-009 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt | 947 | `class AnimeDownloader`; `private const val MIN_DISK_SPACE` (200 MiB); companion `const val TMP_DIR_SUFFIX`, `const val WARNING_NOTIF_TIMEOUT_MS`, `const val EPISODES_PER_SOURCE_QUEUE_WARNING_THRESHOLD`, `private const val DOWNLOADS_QUEUED_WARNING_THRESHOLD` | Core engine: queue, scheduler (per-source concurrency), per-episode staging, hoster resolution, retry, ffmpeg remux, torrent path, external-downloader path, finalisation, queue mutation. | Rewrite. Scheduler becomes tokio tasks plus a `Semaphore`-free per-source map; ffmpeg via `Command` with `-progress`; external handoff dropped to a desktop equivalent. Reason: FFmpegKit and Android intents have no direct equivalent. | `downloads::engine`, `downloads::ffmpeg`, `downloads::retry`, `downloads::torrent_path` | P1 | T-24a-011, T-24a-012, T-24a-013, T-24a-014, T-24a-015, T-24a-016, T-24a-017, T-24a-018, T-24a-019, T-24a-020, T-24a-021, T-24a-022, T-24a-023, T-24a-024, T-24a-025, T-24a-047, T-24a-049, T-24a-051, T-24a-052, T-24a-055 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt | 84 | `data class AnimeDownload`; `enum class State` (nested: `NOT_DOWNLOADED(0)`, `QUEUE(1)`, `DOWNLOADING(2)`, `DOWNLOADED(3)`, `ERROR(4)`) | Queue element: source, anime, episode, changeDownloader flag, optional resolved video, observable status and progress. Companion `fromEpisodeId` builds one from ids. | Port with change: key by episode id, not structural equality (hash over mutable `video`). Reason: data-class hash is unstable during download. | `downloads::model` | P1 | T-24a-001, T-24a-002, T-24a-051 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownloadPart.kt | 60 | `class AnimeDownloadPart` | HTTP range-part holder for multi-part downloads. No reference anywhere in the repo. | Drop. Reason: dead code, never instantiated; multi-part range downloads are not implemented for anime. User loses nothing. | none | P3 | T-24a-003 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadAdapter.kt | 27 | `class AnimeDownloadAdapter`; `interface DownloadItemListener` (`onItemReleased`, `onMenuItemClick`) | FlexibleAdapter subclass that forbids moving items across headers (`shouldMove`). | Rewrite as GTK4 `ListView` with a drag controller that rejects drops across source groups. | `ui::downloads` (new) | P1 | T-24a-043 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHeaderHolder.kt | 37 | `class AnimeDownloadHeaderHolder` | Source group header view holder; drag handle; collapse-all during drag, expand-all on release. | Rewrite (GTK header row). | `ui::downloads` | P1 | T-24a-040, T-24a-043 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHeaderItem.kt | 64 | `data class AnimeDownloadHeaderItem` | Group model: source id, name, count; expanded by default; not selectable; equality ignores sub-item contents except identity. | Rewrite as Rust struct `SourceGroup`. | `ui::downloads` | P1 | T-24a-040 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt | 111 | `class AnimeDownloadHolder` | Item row: title, anime title, progress bar (indeterminate at 0), progress text, popup menu with visibility rules. | Rewrite (GTK row). | `ui::downloads` | P1 | T-24a-041, T-24a-042 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadItem.kt | 68 | `class AnimeDownloadItem` | Sectionable item wrapping an `AnimeDownload`; draggable; equality by episode id; hash = `episode.id.toInt()`. | Rewrite; equality by episode id is kept. | `ui::downloads` | P1 | T-24a-041, T-24a-043 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreen.kt | 93 | `@Composable fun AnimeDownloadQueueScreen(...)` | Compose wrapper that hosts the RecyclerView; empty state; contentPadding handling. | Rewrite (GTK page with empty state). | `ui::downloads` | P1 | T-24a-044 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt | 242 | `class AnimeDownloadQueueScreenModel` | Screen model: builds groups from queue, handles menu actions (move top/bottom, series moves, cancel), reorder with selector, progress/status callbacks, flows. | Rewrite as GObject/Rust model owned by the GTK page. | `ui::downloads` | P1 | T-24a-040, T-24a-042, T-24a-045, T-24a-046 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueTab.kt | 44 | `@Composable fun Screen.animeDownloadTab(...)` (returns `TabContent`) | Tab host: title `AYMR.strings.label_anime`, search disabled, count = sum of sub-items, navigateUp. | Rewrite as one sub-tab of the mockup downloads page. | `ui::downloads` | P1 | T-24a-040, T-24a-044 |

## Cross-area files referenced (not in scope; read only for exact behaviour)

| Path | Owner area | Why referenced |
|---|---|---|
| domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt | area:25-downloads-shared | Every download preference key and default used by the anime engine. |
| app/src/main/java/eu/kanade/domain/download/anime/interactor/DeleteEpisodeDownload.kt | area:25-downloads-shared | UI entry point into `deleteEpisodes`. |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt | area:25-downloads-shared | Hosts `animeDownloadTab`; sort dropdown UI for the queue. |
| app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt | area:45-player-core | `getBestVideo` used by `downloadEpisode`. |
| app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt | area:45-player-core | `getHosters`, `loadHosterVideos`, `buildVideo` call site. |
| app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt | area:13-core-util-storage-archive-torrent | Starts the torrent server before torrent downloads. |
| core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt | area:13-core-util-storage-archive-torrent | `buildValidFilename`, `getAvailableStorageSpace`, `createNoMediaFile`. |
| core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FFmpegUtils.kt | area:13-core-util-storage-archive-torrent | `toFFmpegString` (SAF content URI handling, dropped). |
| core/common/src/main/java/eu/kanade/tachiyomi/network/ProgressListener.kt | area:12-network-stack | Interface implemented by `AnimeDownload` (never attached to an HTTP call in this scope). |
| source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt | area:16-source-api-anime | `Video` fields, `State`, `usesHttpServer`, `copyHttpServer`. |
| domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt | area:07-domain-entries-items | Episode fields used for dedupe, naming and deletion filters. |
| app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt | area:53-platform-manifest-intents-workers | Pause, resume, clear and open-entry broadcast actions bound to notifications. |
| app/src/main/java/eu/kanade/tachiyomi/util/StorageUtil.kt | area:13-core-util-storage-archive-torrent | `UniFile.size()` recursive size used by cache. |
| core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt | area:13-core-util-storage-archive-torrent | `String.chop(count)` used by notifier. |
| core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt | area:02-preferences-system | `hideNotificationContent()`. |
| app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt | area:01-foundation-app-bootstrap | Injected `Json { ignoreUnknownKeys = true; explicitNulls = false }`. |
| app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt | area:36-ui-shell-navigation-security-webview | `startHttpServerService` used by external-downloader path (dropped). |
