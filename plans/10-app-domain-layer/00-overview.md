# Area 10 - App-level domain layer (`eu.kanade.domain`): overview

Repo-relative Kotlin paths are relative to `/home/blackbox/aniyomi-src`. "OBSERVED" = what the Kotlin does. "Proposal:" = what Kurobayomi should do. Other areas are written `area:<id>`.

## 1. Purpose and scope

### 1.1 What Aniyomi does here

`app/src/main/java/eu/kanade/domain/` is the application-level layer between the screen models (UI), the repositories (data layer, area:05 and area:06), the pure domain layer (area:07, area:08, area:09), the source and extension runtime (area:17 to area:20), trackers (area:27 and following), download managers (area:23, area:24, area:25) and the library update jobs. Its callers are the screen models, the reader and player view models, the library update jobs and the backup restorer (see p1-behavior.md section 0 and section 13).

It holds 69 Kotlin files, about 4,000 lines of code (sum of the LOC column in p1-inventory.md). The files are interactors (single-purpose use cases, most with `await` for one-shot calls and `subscribe` for `Flow` queries), model mappers, the DI registry `DomainModule`, and a few stores (delayed-tracking queues).

The main behaviours in scope:

- Reconciling a source's chapter and episode lists with the local database: add, update, remove, carry read and bookmark state, date ordering (`SyncChaptersWithSource`, `SyncEpisodesWithSource`, `SyncSeasonsWithSource`, `SyncRelatedAnimeWithSource`).
- Marking chapters and episodes read or seen, with optional download deletion (`SetReadStatus`, `SetSeenStatus`, `DeleteChapterDownload`, `DeleteEpisodeDownload`).
- Chapter and episode list filtering and sorting.
- Per-entry viewer flags packed into one integer column (`SetMangaViewerFlags`, `SetAnimeViewerFlags`).
- Source listing: enabled sources, language grouping, pins, last-used, incognito, migration sort (`source/manga`, `source/anime`).
- Extension listing and trust (`extension/manga`, `extension/anime`).
- Tracking: binding to tracker entries, pushing progress, queueing offline progress, refreshing, enhanced trackers (`track/manga`, `track/anime`).

### 1.2 Why it matters for a usable Kurobayomi

Every refresh of a details screen, every library update, migration and deep link ends in `SyncChaptersWithSource` or `SyncEpisodesWithSource`. An error there loses read progress (a re-uploaded chapter becomes unread), duplicates notifications, sorts the Updates tab wrongly, or orphans downloads (p1-topic-chapter-episode-sync.md section 1). Read-status writes and source toggles are what the user touches daily. This area is the contract those screens depend on, so the Rust port must reproduce the exact outputs.

### 1.3 Scope boundaries

- In scope: the 69 files listed in p1-inventory.md (4,004 LOC), plus the `DomainModule.kt` and `SYDomainModule.kt` registrations (178 plus 1 registrations; see section 2.3).
- Out of scope: the tracker HTTP implementations, the pure domain models and recognition code (area:07), the data repositories (area:05, area:06), the source API (area:15, area:16), and the GTK screens.

## 2. Module map

### 2.1 Kotlin packages to responsibilities

All under `app/src/main/java/eu/kanade/domain/`.

| Package (Kotlin) | Files | Responsibility |
|---|---|---|
| `DomainModule.kt`, `SYDomainModule.kt` | 2 | Injekt registry: 22 singleton plus 156 factory registrations in `DomainModule`, 1 factory in `SYDomainModule` |
| `download/manga/interactor`, `download/anime/interactor` | 2 | `DeleteChapterDownload`, `DeleteEpisodeDownload` (non-cancellable, via the source lookup) |
| `entries/manga/model`, `entries/anime/model` | 2 | Domain-to-source mappers (`toSManga`, `copyFrom`, `toDomainManga`), filter helpers, `getComicInfo` (manga only), cover-presence checks |
| `entries/manga/interactor`, `entries/anime/interactor` | 8 (manga 4, anime 4) | `UpdateManga`, `UpdateAnime`, viewer-flag setters, excluded scanlators (manga only), `SyncSeasonsWithSource` and `SyncRelatedAnimeWithSource` (anime only) |
| `items/chapter/model`, `items/chapter/interactor` | 5 | Chapter mappers, `ChapterFilter`, `SetReadStatus`, `GetAvailableScanlators`, `SyncChaptersWithSource` |
| `items/episode/model`, `items/episode/interactor` | 4 | Episode mappers, `EpisodeFilter`, `SetSeenStatus`, `SyncEpisodesWithSource` |
| `source/manga/interactor`, `source/anime/interactor` | 15 (manga 8, anime 7) | Enabled sources, languages, favourite-count list, pin/source/language toggles, incognito state, data-saver toggle (manga only) |
| `source/manga/model`, `source/anime/model` | 2 | Extension icon lookup (`Source.icon`, `AnimeSource.icon`) |
| `source/interactor` | 2 | `ToggleLanguage`, `SetMigrateSorting` (enums `Mode`, `Direction`) |
| `source/service` | 1 | `SourcePreferences` (all keys, `DataSaver` enum) |
| `extension/manga/...`, `extension/anime/...` | 5 each side (`interactor`: 4, `model`: 1) | Extensions by type, languages, sources, trust |
| `track/manga/...`, `track/anime/...` | 7 each side (`interactor`: 4, `model`: 1, `service`: 1, `store`: 1) | Track interactors, track mappers, delayed-tracking worker and store |
| `track/model`, `track/service` | 2 | `AutoTrackState`, `TrackPreferences` |

### 2.2 Proposed Rust crate and module map

Crate `kuro-domain` (name reserved by area:01), under `src/app/`, GTK-free (p1-rust-design.md section 1):

| Rust module | Replaces (Kotlin) |
|---|---|
| `app/domain.rs` (`Domain`, `DomainDeps`, `Domain::new`, `Domain::start`) | `DomainModule`, `SYDomainModule` |
| `app/error.rs` | exceptions (`NoChaptersException` and others) |
| `app/rx.rs` (`combine3`, `pref_stream`, `watch_stream`, `distinct_until_changed`) | kotlinx `combine`, `Flow.distinctUntilChanged`, `changes()` |
| `app/flags.rs` | `Set*ViewerFlags` bit masks |
| `app/entries/{manga,anime,manga_model,anime_model,comic_info,anime_seasons,anime_related}.rs` | `entries/*` |
| `app/items/{chapter,chapter_sync,chapter_model,chapter_filter,episode,episode_sync,episode_model,episode_filter}.rs` | `items/*` |
| `app/download/{manga,anime}.rs` | `download/*` |
| `app/source/{mod,prefs,manga,anime,icon}.rs` | `source/*` |
| `app/extension/{mod,manga,anime,trust}.rs` | `extension/*` |
| `app/track/{mod,model,prefs,manga,anime,delayed}.rs` | `track/*` |

Design decision: manga and anime are two near-identical Kotlin trees (about 40 percent duplicated code). Proposal: implement each interactor once, generic over an `EntryKind` marker with associated types, and expose the two public names as type aliases (p1-rust-design.md section 1). Fallback if the borrow checker fights the generic form: two thin modules sharing the pure planner functions. The acceptance tests hold either way.

### 2.3 Registry facts (verified against source)

- `DomainModule.kt` contains 22 `addSingletonFactory` registrations (one of them written without a type argument, line 389) and 156 `addFactory` registrations, total 178. `SYDomainModule.kt` adds 1. Total 179. Verified by counting the registration calls and subtracting the two import lines of each function.
- Some registered classes are declared in other files than their names (`GetMangaExtensionSources.kt` declares `GetExtensionSources`); p1-inventory.md notes this.
- `DelayedAnimeTrackingStore` and `DelayedMangaTrackingStore` are registered in `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:218-219`, not in `DomainModule`. `SourcePreferences` and `TrackPreferences` are in `app/src/main/java/eu/kanade/tachiyomi/di/PreferenceModule.kt:43,76`. These are outside the 178 count.

## 3. Reading guide

| Document | One-line description |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Table of all 69 files: LOC, declarations, port decision (Port, Rewrite, Replace, Drop), target Rust module, priority, task IDs. |
| [p1-behavior.md](p1-behavior.md) | OBSERVED behaviour and Proposal per interactor, section by section (conventions, DI, mappers, sync, filters, sources, extensions, tracking, cross-area contracts). |
| [p1-data-and-config.md](p1-data-and-config.md) | Models, enumerations and stored values, bit-flag layouts with masks, preference keys and defaults, on-disk state, database columns and network endpoints touched. |
| [p1-rust-design.md](p1-rust-design.md) | Crate and module layout, the `Domain` container, Rust signatures for flags, reactive helpers, entry operations, source listing, tracking types, crate choices, async model, error handling, Android-to-Linux mapping, pitfalls. |
| [p1-topic-domainmodule-registry.md](p1-topic-domainmodule-registry.md) | Every one of the 179 registrations, numbered, with kind, class, arity, owner area and Rust counterpart. |
| [p1-topic-chapter-episode-sync.md](p1-topic-chapter-episode-sync.md) | The three sync algorithms: pure planner plus applier split, worked example, ordering rules, episode rules, season recognition, fetch-interval helpers, related anime. |
| [p1-topic-source-and-extension-listing.md](p1-topic-source-and-extension-listing.md) | Ordering primitives (`CASE_INSENSITIVE_ORDER`, `LocaleHelper.comparator`), enabled and language lists, migration list, toggles, incognito, extension grouping, trust, icons. |
| [p1-topic-tracking-orchestration.md](p1-topic-tracking-orchestration.md) | When progress is pushed, the `TrackChapter`/`TrackEpisode` state machine, offline queue, retry worker with backoff, bind-time push, enhanced trackers, progress sync, refresh. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Upstream test coverage, numbered edge cases (EC-01 onward), concurrency and race notes, the full list of new tests to write for `kuro-domain`, and fixtures. |
| [p1-tasks.md](p1-tasks.md) | 37 implementation tasks (T-10a-001 to T-10a-037) with priority, effort, dependencies, sources and acceptance criteria. Lint: ok. |

## 4. Key design decisions and open questions

Each item states the choice the user must make, and a recommendation.

1. **DI container versus explicit struct.** Injekt resolves 178 registrations by type. Proposal: one `Domain` struct built once by `Domain::new(DomainDeps)` with `Arc<dyn Trait>` fields; factories become cheap accessor methods (p1-rust-design.md section 2). Recommendation: accept. No DI crate.

2. **Generic `EntryKind` interactors versus two copies.** Recommendation: generic, with two thin fallback modules if the generic form becomes hard to read. Differences (scanlator exclusion, fillermark, summary and preview fields, queue-on-failure rule, empty-sources fallback) are carried by `Kind::` associated items so no behaviour is lost.

3. **Queue offline tracking progress for anime too.** OBSERVED: `TrackChapter` queues on any failure, but `TrackEpisode` queues only when offline and only logs an online failure (p1-topic-tracking-orchestration.md section 2). This leaves a gap: an online failure for anime is lost. Recommendation: unify on "queue and retry" for both kinds, but separate `TrackError::Offline` (no job noise), `TrackError::Auth` (expired token, do not queue, prompt re-login) and `TrackError::Other` (queue). This is a behaviour change for anime and needs the user's yes.

4. **Single database transaction for sync writes.** OBSERVED: separate writes (delete, insert, update, fetch interval, last update). Proposal: one `Db::write` transaction per sync. Recommendation: accept. It is invisible to users, and it prevents readers from seeing half-synced lists. The planner still runs before the transaction opens.

5. **Chapter numbers: keep `f32` at the source boundary.** OBSERVED: `SChapter.chapter_number` is a 32-bit float; the domain stores it as a double. Recommendation: keep `f32` in the source struct and convert with `as f64`. Otherwise `12.1` and `12.100000381469727` never compare equal, and every sync rewrites the row (p1-topic-chapter-episode-sync.md section 4).

6. **Data-saver image proxies (Bandwidth Hero, wsrv.nl, resmush.it).** OBSERVED: `SourcePreferences.DataSaver` enum has `NONE, BANDWIDTH_HERO, WSRV_NL, RESMUSH_IT`. Decision recorded in p1-tasks.md (T-10a-025): the three proxy modes are stored but not implemented in this area. Recommendation: keep the preference, drop the proxy implementations, and say so in the Settings UI. The user loses image loading through three third-party proxy services (image rewriting to save bandwidth). Confirm.

7. **Legacy Android queue import.** OBSERVED: `DelayedTrackingStore` uses an Android `SharedPreferences` file named `tracking_queue`. Recommendation: do not import it. Kurobayomi has no Android install to migrate from. Confirm.

8. **Queue key collision between manga and anime.** OBSERVED: both stores share one file name, so a manga worker can delete anime entries by id. Proposal: namespace the keys by kind in one JSON file (p1-topic-tracking-orchestration.md section 3). Recommendation: accept. This is a bug fix, not a choice.

9. **Extension trust model.** OBSERVED: trust is a decision on Android package signing fingerprints (`PackageInfo`, repo signing keys). There is no equivalent for native Linux extensions. Depends on area:20 (`PackageIdentity { name, version_code, fingerprints }`). The user must choose the extension strategy first (area:20 open items). Recommendation: this area implements one `TrustExtension` for both kinds that accepts an identity from area:20.

10. **Extension icons.** OBSERVED: `AnimeSource.icon` and `Source.icon` look up the Android app icon as a Compose `ImageBitmap`. Proposal: `SourceIcons` trait returning a cached `gdk::Texture`, with a generic fallback icon. Recommendation: accept. Whether a Linux source ships an icon at all depends on area:20.

11. **Connectivity check.** OBSERVED: `Context.isOnline()` checks `NetworkCapabilities` transports. Proposal: `Connectivity::is_online()` via `gio::NetworkMonitor` (`connectivity() >= Limited` counts as online). Recommendation: accept.

12. **Priority of tasks that P0 tasks depend on.** Three P0 tasks depend on P1 tasks: T-10a-008 (sync) depends on T-10a-016 (excluded scanlators), T-10a-013 (SetReadStatus) depends on T-10a-015 (download deletion), and T-10a-036 (test suite) depends on T-10a-028 (TrackChapter). Recommendation: promote T-10a-015, T-10a-016 and T-10a-028 to P0, or drop the dependency for the first iteration. The user's call, because it changes what "usable daily" means.

13. **Whether the Rust port reproduces Kotlin quirks on purpose.** Examples: `Pins::UNPINNED.contains(Pin::UNPINNED)` is always true; `Pins::minus` is XOR; `coalesce` updates cannot clear nullable columns; `CASE_INSENSITIVE_ORDER` is not Unicode case folding (p1-rust-design.md section 9). Recommendation: keep the quirks wherever they change stored data or ordering, and pin each one with a test. The user should confirm this default.

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What it needs (from p1-behavior.md section 13 and p1-tasks.md dependencies) |
|---|---|
| area:01-foundation-app-bootstrap | `kuro-core` runtime, `kuro-testkit` fixtures, crate name `kuro-domain`, startup hook for `Domain::start` |
| area:02-preferences-system | `Pref<T>` with `get`, `set`, `changes()`; `SourcePreferences`, `TrackPreferences`, `BasePreferences`, `LibraryPreferences`, `DownloadPreferences` registry sections |
| area:03-db-manga-schema-migrations | Manga table columns (`viewer_flags`, `excluded_scanlators`, `hidden_catalogues`, `pinned_catalogues`, `incognito_manga_extensions`, `data_saver_excluded`) |
| area:05-data-repos-entries-items | `MangaRepo`, `AnimeRepo`, `ChapterRepo`, `EpisodeRepo`, `Db::write` transactions, `ChapterSanitizer` and `EpisodeSanitizer` |
| area:06-data-repos-history-track-source-misc | History and track repositories, source repository, extension repo repositories |
| area:07-domain-entries-items | `MangaFetchInterval` and `AnimeFetchInterval` (`get_window`, `to_update`, MAX_INTERVAL 28, GRACE 1), `ChapterRecognition`, `EpisodeRecognition`, `SeasonRecognition`, `Chapter`/`Episode`/`Manga`/`Anime` models, `chapterFlags` and `episodeFlags` setters |
| area:08-domain-library-category-history-updates | `GetMangaHistory` and `GetAnimeHistory` (read-at timestamps), library and category interactors used by `AddTracks` |
| area:09-domain-source-track-download-release-misc | Track and download domain models |
| area:14-source-local-and-metadata | Local source always present in the enabled list; ComicInfo metadata inputs for `getComicInfo` |
| area:15-source-api-manga, area:16-source-api-anime | `SChapter`, `SEpisode`, `SManga`, `SAnime`, `SSeason`, `SAnimeRelated` shapes; `prepareNew*` on HTTP sources |
| area:17-source-app-layer | `SourceCatalog`: `get`, `get_or_stub`, `watch_all`, `watch_online`, `watch_with_favorite_count` |
| area:18-extensions-manga, area:19-extensions-anime | Installed, untrusted and available extension flows; `extension_package(source_id)` |
| area:20-extension-strategy-redesign | `ExtensionCatalog`, `PackageIdentity`, the extension strategy that decides trust and icons |
| area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared | `DownloadFacade`: `is_downloaded`, `is_dir_name_changed`, `rename`, `delete` |
| area:27-tracking-core | `TrackerRegistry`: `get(id)`, `logged_in_trackers()`, `MangaTracker` and `AnimeTracker` service interfaces, `EnhancedTracker`, `trackAuthExpired` |
| area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin | Enhanced-tracker behaviour used by `bindEnhancedTrackers` and `SyncProgressWithTrack` |
| area:34-images-covers-cache | `CoverCache`: `has_custom_cover`, `delete_from_cache`, `has_custom_background`, `delete_background_from_cache` |
| area:51-i18n | Message keys (`no_chapters_error`, `no_episodes_error`, `no_seasons_error`, `confirm_tracker_update`, `trackers_updated_summary_manga`, and others listed in p1-rust-design.md section 6) |
| area:52-util-app | `cmp_ignore_case_java` and `lang_cmp` helpers (`LocaleHelper.comparator`) |
| area:53-platform-manifest-intents-workers | OS scheduling, if any; this area needs none, because the retry worker is a tokio task (p1-rust-design.md section 7) |

### 5.2 What others need from this area

| Area | What it uses from area:10 |
|---|---|
| area:26-library-update-jobs | `SyncChaptersWithSource::run` (with the `(0, 0)` window sentinel), `SyncEpisodesWithSource`, `UpdateManga`, `UpdateAnime`, `TrackChapter`, `TrackEpisode`, `RefreshMangaTracks`, `RefreshAnimeTracks` |
| area:37 to area:42 (UI library, entry details, browse, updates) | Chapter and episode list filters and sort (`apply_filters`), `SetReadStatus`, `SetSeenStatus`, `GetAvailableScanlators`, `SetExcludedScanlators`, source listing streams, extension listing streams, migration sort |
| area:43, area:44 (reader), area:45, area:46 (player) | `SetMangaViewerFlags`, `SetAnimeViewerFlags` (skip-intro length, next-airing), `TrackChapter`, `TrackEpisode`, `UpdateManga` for `last_page_read` coalescing |
| area:47, area:47b, area:48 (settings) | Source and track preference keys (`SourcePreferences`, `TrackPreferences`) through area:02 bindings, `AutoTrackState` labels |
| area:21, area:22 (backup) | `UpdateManga`, `UpdateAnime`, `SyncChaptersWithSource`, and the restorer's use of the entry and track models |
| area:27 (tracking core) | `AddMangaTracks::bind`, `AddAnimeTracks::bind`, `Track` struct and personal-field copy (`copy_personal_from`) |
| area:34 (covers) | `UpdateManga` and `UpdateAnime` call cover invalidation through `CoverCache` |

## 6. Task summary

### 6.1 Counts (from `TASKS.csv`, rows whose file starts with `10-app-domain-layer/`)

Total: 37 tasks (T-10a-001 to T-10a-037), all in `10-app-domain-layer/p1-tasks.md`.

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 9 | 6 | 2 | 0 | 17 |
| P1 | 7 | 8 | 0 | 0 | 15 |
| P2 | 3 | 1 | 0 | 0 | 4 |
| P3 | 1 | 0 | 0 | 0 | 1 |
| **Total** | **20** | **15** | **2** | **0** | **37** |

Upper-bound effort using the effort definitions (S = 1 day, M = 3 days, L = 7.5 working days): P0 about 42 days, P1 about 31 days, P2 about 6 days, P3 about 1 day. Total about 80 working days upper bound. These are upper bounds, not estimates.

Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint p1-tasks.md` prints `ok`.

### 6.2 P0 critical path, in order

1. T-10a-001 `Domain` container (depends on area:01, area:02, area:05).
2. T-10a-002 reactive helpers and T-10a-003 viewer flag helpers (parallel, both after 001).
3. T-10a-004 anime viewer flags (after 003).
4. T-10a-005 entry model mappers (after 001, area:07, area:15, area:16).
5. T-10a-006 chapter and episode mappers (after 005).
6. T-10a-007 `UpdateManga` and `UpdateAnime` (after 005, area:34).
7. T-10a-016 excluded scanlators (P1, after 001 and area:03, area:05). Required by step 8.
8. T-10a-008 `SyncChaptersWithSource` (after 006, 007, 016 and area:23). This is the hardest task (L).
9. T-10a-009 `SyncEpisodesWithSource` (after 008 and area:24). Effort L.
10. T-10a-017 `SourcePreferences` section (after area:02, area:52), then T-10a-018 enabled sources (after 002 and 017), T-10a-019 language ordering, T-10a-020 toggle source, pin and language.
11. T-10a-012 chapter and episode filter and sort (after 005).
12. T-10a-015 download deletion (P1, after 001), then T-10a-013 `SetReadStatus` and T-10a-014 `SetSeenStatus`.
13. T-10a-028 `TrackChapter` and `TrackEpisode` (P1), needed by T-10a-036.
14. T-10a-036 golden and integration test suite (after 008, 009, 012, 018, 028). This gates release of the area.

Tasks 002, 003, 005 and 017 can run in parallel once 001 lands. Tasks 008 and 009 are sequential: the same planner is reused.

### 6.3 Risks

1. **Priority inversions.** P0 tasks T-10a-008, T-10a-013 and T-10a-036 depend on P1 tasks (T-10a-016, T-10a-015, T-10a-028). If the P1 tasks slip, the P0 work stalls (decision 12 above).
2. **Sync correctness.** The sync is the most important code in the area and the Kotlin has no unit test for it (p1-topic-chapter-episode-sync.md section 1). Mitigation: the golden test `sync_chapters::worked_example` and the ordering tests in p1-edge-cases-and-tests.md section 4 must exist before T-10a-008 is accepted.
3. **Float and collation edge cases.** `f32` versus `f64` chapter numbers and `CASE_INSENSITIVE_ORDER` versus Unicode case folding (p1-rust-design.md section 9, pitfalls 1 and 4). Mitigation: vector tests generated from a Java program (p1-edge-cases-and-tests.md section 4).
4. **Tracking data loss on anime.** Decision 3 changes anime behaviour; without the user's yes, the gap stays.
5. **Extension trust depends on area:20.** T-10a-023 and T-10a-024 cannot finish until area:20 fixes the package identity. Risk of late rework.
6. **Unresolved downstream API shape.** Callers in area:26 and the UI areas must port unchanged (the `(0, 0)` window sentinel and the `SetStatusResult` variants). Any change in this area's signatures ripples. Freeze the signatures in p1-rust-design.md before the UI areas start.
7. **Missing task folders.** Many `area:` tokens in p1-tasks.md (areas 14 to 53) name areas whose folders do not yet exist in `plans/`. `tasks.py export` reports them as "unknown area". See section 8.

## 7. Definition of done for this area in Kurobayomi

The area is done when the following are observable in the running app and covered by `cargo test -p kuro-domain`:

1. Opening a manga or anime details screen refreshes the chapter or episode list from its source. New items appear in the order the source lists them, with the newest at the top of the Updates tab (`date_fetch` descending).
2. Re-syncing an unchanged source writes nothing to the database (the no-change test passes).
3. A re-uploaded chapter with a new URL keeps its read and bookmark state. A chapter whose number or name changes keeps its download and is renamed.
4. Marking chapters read or unread persists across restart. With the "delete downloads after read" preference on, the download files are removed, and the operation completes even if the screen is closed mid-way.
5. Marking episodes seen or unseen works the same way; a last-second reset happens when an episode is marked unseen.
6. The chapter and episode list filters (unread, bookmarked, downloaded tri-state, plus fillermark for episodes) and sort direction match the Kotlin for all 27 combinations of unread, bookmarked and downloaded.
7. The sources list shows enabled sources grouped in this order: last-used, pinned, then by language, with the empty language last. Toggling, pinning and language changes persist across restart.
8. The migration source list shows favourite counts and sorts by alphabetical or total, ascending or descending.
9. The extensions page shows updates, installed, available and untrusted sections. The NSFW filter applies to installed and available only. Trusting an extension persists.
10. Viewer flags round-trip: reading mode and orientation for manga; skip-intro length (0 to 255) and the intro-disable bit, next-airing episode and airing time for anime, all stored in the same integer column as Kotlin.
11. Marking a chapter read pushes progress to every logged-in tracker. If offline, the progress is queued and replayed after reconnect, with retry attempts at 0, 5, 15 and 35 minutes, and then stops. Queue keys for manga and anime never collide.
12. Binding a tracker pushes the local read progress and start date. Enhanced trackers bind automatically.
13. The registry parity test (T-10a-037) passes: every one of the 179 Kotlin registrations has either a `Domain` accessor or a documented drop.
14. The golden test suite (T-10a-036) passes, and the Kotlin-reproducing quirks in section 4 decision 13 are covered by tests.
15. No Android-only construct (`androidx.work`, `SharedPreferences`, `PackageInfo`, Injekt, Compose `ImageBitmap`) is referenced in `kuro-domain`. This is a CI grep check (T-10a-035).

## 8. Notes for the orchestrator

- `p1-tasks.md` lints `ok`. The area dependency tokens in it are already canonical ids from `_tools/areas.py` (the registry the tools use to name areas). No token was changed, and no task ID was changed.
- `tasks.py export` reports "unknown area" for tokens such as `area:15-source-api-manga` and `area:23-downloads-manga`. The validator checks only folders that exist under `plans/` (`01-*` to `13-*`, and `10-app-domain-layer`). The tokens will resolve when those areas create their folders. Other areas' task files are affected the same way.
- Last `tasks.py export` run: 372 tasks, 61 errors, all shown as "unknown area". 30 of them are in `10-app-domain-layer/p1-tasks.md`. No other kind of error is reported for this area.
