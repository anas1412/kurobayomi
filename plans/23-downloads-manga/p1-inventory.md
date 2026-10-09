# Area 23 — Manga downloads: inventory

Scope: 17 files, 3,397 LOC, all under `app/src/main/java/eu/kanade/tachiyomi/`. Paths are repo-relative to `/home/blackbox/aniyomi-src`. Every file was read in full.

Port decision legend: **Port** (same behaviour, Rust code), **Rewrite** (same behaviour, different mechanism), **Replace-with-crate** (Android API replaced by a named Linux or Rust crate), **Drop** (no equivalent wanted, reason stated).

Priority legend follows the task-format rules: P0 = needed to read downloaded chapters offline and run the queue daily; P1 = parity; P2 = polish; P3 = drop or obsolete.

## Table

| Path (repo-relative) | LOC | Top-level declarations (every name) | Purpose | Port decision and reason | Target Rust module | Priority | Task ids |
|---|---|---|---|---|---|---|---|
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt | 492 | `class MangaDownloadCache`; `private class RootDirectory`; `private class SourceDirectory`; `private class MangaDirectory`; `private object UniFileAsStringSerializer` | In-memory index of the downloads folder (source folder, manga folder, chapter names), rescanned at most hourly, persisted to `dl_index_cache_v3`. Feeds all "is downloaded" badges and counts. | Rewrite. The index idea is kept (scanning is slow), but the UniFile/ProtoBuf plumbing is replaced by plain paths and a versioned binary or JSON snapshot. | `kuro-downloads::index` | P0 | T-23a-006, T-23a-007, T-23a-040, T-23a-045 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadJob.kt | 132 | `class MangaDownloadJob`; companion: `private const val TAG = "MangaDownloader"`, `fun start(context: Context)`, `fun stop(context: Context)`, `fun isRunning(context: Context): Boolean`, `fun isRunningFlow(context: Context): Flow<Boolean>` | WorkManager `CoroutineWorker` that keeps the downloader alive while the device is online, plus a foreground notification. Stops when the network is lost or Wi-Fi is required but missing. | Replace-with-crate. WorkManager has no Linux equivalent. Use a tokio task owned by the app plus a GApplication hold so the process stays alive while downloads run. Network gating moves into `kuro-platform`. | `kuro-downloads::job` plus `kuro-platform::SchedulerBackend` | P0 | T-23a-023, T-23a-035 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt | 459 | `class MangaDownloadManager` (public facade) | Public API used by the rest of the app: enqueue, start, pause, clear, reorder, delete, rename, count, size, status and progress flows. | Port. Becomes the single facade type `DownloadManager` for manga. | `kuro-downloads::manager` | P0 | T-23a-027, T-23a-029, T-23a-030, T-23a-025, T-23a-026 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadNotifier.kt | 233 | `internal class MangaDownloadNotifier` | Progress, paused, complete, warning and error notifications for the downloader, with Pause, Resume, Cancel all and Show manga actions. | Replace-with-crate. Use the `Notifier` trait from `kuro-platform` (gio::Notification or the freedesktop notification spec). Channels become urgency and categories. | `kuro-downloads::notify` plus `kuro-platform::Notifier` | P0 | T-23a-019, T-23a-043 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadPendingDeleter.kt | 187 | `class MangaDownloadPendingDeleter`; `private data class Entry`; `private data class ChapterEntry`; `private data class MangaEntry` | Holds chapters that were read in the reader and must be deleted when the reader closes. Persisted so a crash does not lose the list. | Port, but store in SQLite instead of SharedPreferences `chapters_to_delete` (no JSON blobs). | `kuro-downloads::pending` | P1 | T-23a-028 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadProvider.kt | 176 | `class MangaDownloadProvider` | Maps source, manga and chapter to folder names and paths. Owns the on-disk naming rules. | Port. Path building comes from the `Paths` trait (downloads root), and names pass through one sanitiser (addendum rule 5). | `kuro-downloads::layout` | P0 | T-23a-003, T-23a-004, T-23a-005 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadStore.kt | 149 | `class MangaDownloadStore`; `private data class DownloadObject` | Persists the active queue across restarts. Uses SharedPreferences `active_downloads`, one JSON entry per chapter id. | Port to SQLite table `manga_download_queue`. Fixes the side effect inside a StateFlow update lambda. | `kuro-downloads::store` | P0 | T-23a-009, T-23a-035 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt | 774 | `class MangaDownloader`; `const val TMP_DIR_SUFFIX = "_tmp"` (companion); `const val WARNING_NOTIF_TIMEOUT_MS = 30_000L` (companion); `const val CHAPTERS_PER_SOURCE_QUEUE_WARNING_THRESHOLD = 15` (companion); `private const val DOWNLOADS_QUEUED_WARNING_THRESHOLD = 30` (companion); top-level `private const val MIN_DISK_SPACE = 200L * 1024 * 1024` | The engine: queue state, scheduler, page fetch, retries, throttle, integrity check, ComicInfo, CBZ archiving, finalisation. | Port (core of the area). Split across modules: queue, scheduler, pipeline, images, comicinfo, archive, finalise. | `kuro-downloads::{queue, scheduler, downloader, pipeline, images, comicinfo, archive}` | P0 | T-23a-008, T-23a-010, T-23a-011, T-23a-012, T-23a-013, T-23a-014, T-23a-015, T-23a-016, T-23a-017, T-23a-018, T-23a-020, T-23a-021, T-23a-022, T-23a-024, T-23a-044 |
| app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt | 89 | `data class MangaDownload`; `enum class State` (NOT_DOWNLOADED, QUEUE, DOWNLOADING, DOWNLOADED, ERROR); companion `suspend fun fromChapterId(...)` | The queue item: source, manga, chapter, page list, status, progress flows. | Port as a plain struct with `Arc<Mutex<..>>` status and a `tokio::sync::watch` channel for status and progress. Numeric state values are kept (0..4) because they are stored in the queue table. | `kuro-downloads::model` | P0 | T-23a-002 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadAdapter.kt | 27 | `class MangaDownloadAdapter`; `interface DownloadItemListener` (`onItemReleased`, `onMenuItemClick`) | FlexibleAdapter for the queue list. Blocks moves between groups. | Rewrite. A GTK `ListView` or `ListBox` with drag handles; the same "no cross-group move" rule is kept. | `src/ui/download_queue.rs` | P1 | T-23a-033 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHeaderHolder.kt | 37 | `class MangaDownloadHeaderHolder` | Group header row (source name and count, drag handle, collapse while dragging). | Rewrite as a GTK widget. | `src/ui/download_queue.rs` | P1 | T-23a-033 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHeaderItem.kt | 64 | `data class MangaDownloadHeaderItem` (fields `id`, `name`, `size`; custom `equals`/`hashCode`) | Group model: source id, source name, count. | Port as a model struct. The equality rule (by id, name, size) matters for "move to top" lookups. | `src/ui/download_queue.rs` (model) | P1 | T-23a-032 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHolder.kt | 101 | `class MangaDownloadHolder` | Row widget: title, chapter name, progress bar, "done/total" text, three-dot menu. | Rewrite as GTK. | `src/ui/download_queue.rs` | P1 | T-23a-033, T-23a-034 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadItem.kt | 68 | `class MangaDownloadItem` (equality by `chapter.id`, hash truncates to Int) | Row model wrapping a `MangaDownload`. | Port as a model. Use the full 64-bit chapter id for equality and hash. | `src/ui/download_queue.rs` (model) | P1 | T-23a-032 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreen.kt | 93 | `@Composable fun DownloadQueueScreen(...)` | Compose wrapper that hosts the RecyclerView, applies content padding, and wires status and progress flows. | Rewrite. Plain GTK page inside the Downloads page. | `src/ui/download_queue.rs` | P1 | T-23a-033 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreenModel.kt | 272 | `class MangaDownloadQueueScreenModel` | View model: groups the queue by source, reorder and move actions, cancel actions, sort, progress and status hooks. | Port the logic as a view model that runs on the GTK thread and receives events through `async-channel`. | `src/ui/download_queue_model.rs` | P1 | T-23a-032, T-23a-025, T-23a-026 |
| app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueTab.kt | 44 | `@Composable fun Screen.mangaDownloadTab(...)` | Tab adapter: title "Manga", search disabled, count = total queued items, navigate-up. | Rewrite. A page in the Downloads view with a count badge. | `src/ui/download_queue.rs` | P1 | T-23a-033 |

## Counts

- 17 files, 3,397 LOC (matches `areas.py files 23-downloads-manga 1`).
- Core engine files (MangaDownloader, MangaDownloadCache, MangaDownloadManager, MangaDownloadProvider, MangaDownloadStore, MangaDownloadPendingDeleter, MangaDownload model): 2,305 LOC.
- Platform glue files (MangaDownloadJob, MangaDownloadNotifier): 365 LOC, both replaced.
- UI files (ui/download/manga, 9 files): 727 LOC, all rewritten in GTK.

## Declarations that are not top-level but matter

- `MangaDownloader.init` launches a restore of the queue from the store (see behaviour spec, section "Startup").
- `MangaDownloadCache.renewCache()` and `updateDiskCache()` are private members; their timings are in the behaviour spec.
- `MangaDownloadPendingDeleter` has `@Synchronized` on `addChapters` and `getPendingChapters`.
- `MangaDownloadProvider.isChapterDirNameChanged(...)` is public and has no caller inside this scope (dead inside the scope, used elsewhere per the behaviour spec).

## Cross-area symbols that this scope calls (owned elsewhere)

| Symbol | Owner area | Used in |
|---|---|---|
| `Page` (source model) | area:15-source-api-manga (source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt) | MangaDownload, MangaDownloader |
| `DownloadPreferences` | area:25-downloads-shared (domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt) | Manager, Downloader |
| `DataSaver` | area:26-library-update-jobs (app/src/main/java/aniyomi/util/DataSaver.kt) | Downloader |
| `ChapterCache` | area:34-images-covers-cache (app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt) | Downloader |
| `ImageUtil`, `DiskUtil`, `ZipWriter`, `UniFile` helpers | area:13-core-util-storage-archive-torrent | Downloader, Provider, Cache |
| `ComicInfo`, `getComicInfo` | area:14-source-local-and-metadata (core-metadata/...; app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt is area 10) | Downloader |
| `Notifications` channel and id constants | area:53-platform-manifest-intents-workers (app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt) | Notifier, Job |
| `NotificationReceiver` actions (pause, resume, clear, open manga) | area:53-platform-manifest-intents-workers | Notifier |
| `GetMangaCategories`, `GetMangaTracks`, `GetChapter`, `GetManga` | area:08, area:27, area:05, area:07 | Downloader, Store, Manager |
| `MangaSourceManager` | area:17-source-app-layer | Cache, Downloader, Store |
| `SourcePreferences` | area:02-preferences-system | Downloader |
| `SecurityPreferences` (hide_notification_content) | area:02-preferences-system (core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt) | Notifier |
