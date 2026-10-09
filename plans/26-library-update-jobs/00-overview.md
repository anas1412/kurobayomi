# Area 26 - Library update jobs: overview

Area id: `26-library-update-jobs`. Source root: `/home/blackbox/aniyomi-src` (repo-relative paths throughout). Output of this area: the Rust design in `p1-rust-design.md`, the behaviour spec in `p1-behavior.md`, and the 104 tasks in `p1-tasks.md`.

---

## 1. Purpose and scope

**What Aniyomi does here.** A "library update pass" walks every favourite manga and anime, asks its source for the current chapter or episode list (and, optionally, details), writes the diff to the local database, decides which new items are worth downloading, and tells the user what changed through notifications. Passes run in two ways:

- **Automatic:** a periodic WorkManager job, with an interval chosen in settings (`0, 12, 24, 48, 72, 168` hours; `0` = off) and device restrictions (Wi-Fi, unmetered, charging).
- **Manual:** "Update library" from the Library or Updates screens, optionally limited to one category.

Two more manual jobs refresh library covers and metadata (the "Refresh library covers" action). Supporting pieces: a per-entry fetch-interval model that decides when an entry is next due, a set of pure helpers (next unread, next unseen, duplicate removal, downloaded-only filters), and the image data saver (`aniyomi/util/DataSaver.kt`), which is used by the reader and the downloader as well.

**Why it matters for a usable Kurobayomi.** Without this area the library never gets new chapters or episodes, the Updates tab stays empty, "continue reading" and "continue watching" have no target, and downloads of new items never start. It is P0 for the core (next-unread/unseen helpers, sync, pass runner, scheduler) and P1 for parity features (notifications, downloads-on-update, metadata refresh).

**In scope (12 scope files, 2,592 LOC):** see `p1-inventory.md`.

| Kotlin file | LOC |
|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt` | 528 |
| `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt` | 540 |
| `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt` | 400 |
| `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt` | 413 |
| `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaMetadataUpdateJob.kt` | 207 |
| `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeMetadataUpdateJob.kt` | 199 |
| `app/src/main/java/aniyomi/util/DataSaver.kt` | 170 |
| `app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterGetNextUnread.kt` | 33 |
| `app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterRemoveDuplicates.kt` | 15 |
| `app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterFilterDownloaded.kt` | 27 |
| `app/src/main/java/eu/kanade/tachiyomi/util/episode/EpisodeFilterDownloaded.kt` | 27 |
| `app/src/main/java/eu/kanade/tachiyomi/util/episode/EpsiodeGetNextUnseen.kt` (sic; keep the misspelling) | 33 |

**Out of scope here** (owned elsewhere, listed in section 5): the source API contract, the database schema, download execution, the Updates and Library screens, preference storage, and tracker sync.

**Explicitly dropped or replaced on Linux** (details in `p1-data-and-config.md` section 6 and `p1-rust-design.md` section 8):

| Android element | Linux outcome | What the user loses |
|---|---|---|
| WorkManager periodic job, constraints, backoff | in-process tokio scheduler (mode A) or systemd user timer (mode B) | Missed runs are not replayed; with mode A the app must keep running (portal background permission) |
| Battery-not-low constraint | not enforced | Nothing on mains power; laptops on battery may run passes that Android would defer |
| Foreground service notification (`getForegroundInfo`) | not used; progress is a freedesktop notification | Nothing visible beyond the notification |
| Notification channels and channel importance settings | one settings toggle "Show library update notifications" (default on) | Per-channel importance control |
| Android group summary behaviour | summary notification is independent of child notifications | Dismissing the summary no longer dismisses children |
| `FileProvider` URI for the error log | `gio::File::for_path(p).uri()` opened by `gio::AppInfo::launch_default_for_uri` | Nothing |

---

## 2. Module map

### 2.1 Kotlin packages to responsibilities

| Package or file (repo-relative) | Responsibility |
|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/` | Manga pass worker (`MangaLibraryUpdateJob`), manga notifier, manga metadata worker |
| `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/` | Anime pass worker, anime notifier, anime metadata worker. Structurally a copy of the manga package (duplicated code; the port has one implementation) |
| `app/src/main/java/aniyomi/util/DataSaver.kt` | Image proxy rewriting (Bandwidth Hero, wsrv.nl, reSmush.it) |
| `app/src/main/java/eu/kanade/tachiyomi/util/chapter/`, `.../util/episode/` | Pure list helpers for reader and player (next unread/unseen, dedupe, downloaded filter) |
| `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt` | Keys and defaults for every library-update preference (area:02) |
| `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt`, `.../entries/anime/interactor/AnimeFetchInterval.kt` | Fetch window, interval, next-update algorithm |
| `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt`, `.../items/episode/interactor/SyncEpisodesWithSource.kt` | Diff of source list against database rows; writes |
| `app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt`, `app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt` | Metadata update from source |
| `domain/src/main/java/mihon/domain/items/chapter/interactor/FilterChaptersForDownload.kt`, `.../items/episode/interactor/FilterEpisodesForDownload.kt` | Decides which new items are auto-downloaded |
| `app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt`, `NotificationReceiver.kt` | Channel and id constants; broadcast actions (owned by area:01) |

### 2.2 Proposed Rust module map

Single crate for now (`Cargo.toml` is `kurobayomi`, edition 2024, `gtk4` 0.11.5 with `v4_16`). The `core` tree has no `gtk` imports so it is unit-testable headless (`p1-rust-design.md` section 1).

| Rust path | Replaces | Notes |
|---|---|---|
| `src/core/library_update/queue.rs` | category selection, restrictions, season expansion, sort | pure |
| `src/core/library_update/pass.rs` | `doWork` pass body, per-source grouping, per-entry flow | `run_pass` returns `PassOutcome` |
| `src/core/library_update/sync.rs` | `SyncChaptersWithSource`, `SyncEpisodesWithSource` | pure diff plus rusqlite writes in `transaction()` per group |
| `src/core/library_update/fetch_interval.rs` | `MangaFetchInterval`, `AnimeFetchInterval` | chrono, one implementation for both kinds |
| `src/core/library_update/metadata.rs` | the two metadata workers | manual only |
| `src/core/library_update/error_log.rs` | `writeErrorFile` | |
| `src/core/scheduler.rs` | `setupTask`, `startNow`, `stop`, `cancelAllWorks`, lock | tokio loops per kind, `watch` channels |
| `src/core/notify/text.rs` | notifier text builders | pure, golden-tested |
| `src/core/notify/desktop.rs` | notifier rendering | `notify-rust` |
| `src/core/chapter_util.rs`, `src/core/episode_util.rs` | the five util files | pure |
| `src/core/datasaver.rs` | `DataSaver.kt` | reader and downloader use it too |
| `src/core/net_state.rs` | `isConnectedToWifi`, metered and charging checks | `zbus` to NetworkManager and UPower |
| `src/core/prefs.rs` | typed view over preferences | storage owned by area:02 |
| `src/ui/updates_screen.rs`, `src/ui/settings_library.rs` | in-app surfaces | GTK only |

---

## 3. Reading guide

All files are in this folder.

- [p1-inventory.md](p1-inventory.md): the 12 scope files with LOC, every declared name, port decision and task ids.
- [p1-behavior.md](p1-behavior.md): the Kotlin behaviour to reproduce, in 15 sections: scheduling, pre-checks, queue and restrictions, fetch interval, sync, pass execution, error log, notifications, metadata passes, data saver, helpers, defects P-1..P-18, message keys, constants.
- [p1-data-and-config.md](p1-data-and-config.md): domain models, enums and their stored integers, preference keys with defaults, files, network endpoints, database tables touched, the proposed `library_job_run` table.
- [p1-rust-design.md](p1-rust-design.md): crate layout, Rust type and trait signatures, async model, persistence and locking, error handling, crate choices, Android-to-Linux mapping, implementer pitfalls.
- [p1-topic-scheduling.md](p1-topic-scheduling.md): WorkManager facts, desktop constraints, mode A (resident) versus mode B (systemd timer), loop design, single-instance rule, startup sequence.
- [p1-topic-update-pipeline.md](p1-topic-update-pipeline.md): queue, fetch window, sync, download hand-off, counters, errors, ordering guarantees, deliberate deviations table.
- [p1-topic-notifications.md](p1-topic-notifications.md): freedesktop mapping, action callbacks (`NotifyAction`), hidden content, images, desktop limits.
- [p1-ui-parity.md](p1-ui-parity.md): every notification and in-app surface with its Kotlin source, desktop equivalents for gestures, responsive behaviour, animation, gaps in the existing mockup, strings to add.
- [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): ported fetch-interval tests FI-01..FI-23, queue, sync, pass, scheduler and notification edge cases, failure modes, race conditions, fixtures, performance targets, the manual acceptance checklist.
- [p1-tasks.md](p1-tasks.md): the 104 tasks `T-26a-NNN` in the required format.
- `00-overview.md` (this file).

---

## 4. Key design decisions and open questions

Each entry gives the decision needed, the options, and a recommendation. Items marked **User call** need the user's answer before implementation starts. The others are engineering calls that the recommendation already covers.

| # | Question | Options | Recommendation | Status |
|---|---|---|---|---|
| D1 | How do automatic passes run when the window is closed? | A. resident process with `org.freedesktop.portal.Background.RequestBackground` autostart. B. systemd user timer `kurobayomi-update.timer` running `kurobayomi --library-update=auto`. | A by default; B as an opt-in in settings. A and B never run together. Without the portal, A runs only while the window or process is open, and settings shows "Automatic updates run only while Kurobayomi is open." | **User call** (T-26a-010..T-26a-015 depend on it) |
| D2 | First automatic pass after enabling | run now when constraints allow (WorkManager parity) / wait for the next interval | parity: run as soon as constraints are met | **User call**; default is parity (T-26a-010) |
| D3 | Missed automatic runs while the app was closed | replay / not replay | not replayed; the next run starts at next launch if due (T-26a-011) | Decided |
| D4 | Battery-not-low constraint | enforce via UPower percentage / not enforce | not enforced on desktop (documented deviation; mains-powered desktops have no meaningful signal). `p1-behavior.md` section 14 still lists "battery not low: always required" as the Kotlin value, and that line is correct for Kotlin only | **User call** (T-26a-012) |
| D5 | New anime episodes are never queued for download in Aniyomi (P-1: `downloadEpisodes` is never called) | wire it like manga / keep the gap | wire it with `autoStart = false`, as manga does. The default for `download_new_episode` is `false`, so no user gets surprise downloads | **User call** (T-26a-024) |
| D6 | Anime favourite recheck happens after the sync (P-7) | keep / move before the sync | move the recheck before the sync for both kinds | Decided (T-26a-024) |
| D7 | Concurrency across kinds | per-kind semaphore of 5 (Kotlin) / process-wide per-source permit of 1 added on top | add the process-wide per-source permit so manga and anime passes never hit one source at once | Decided (T-26a-022) |
| D8 | Restrictions also apply to manual passes (`manga_outside_release_period` skips in both) | parity / exempt manual | parity; the settings help text says so | Decided; the help text is the user-visible consequence |
| D9 | Metadata refresh jobs share one unique work name in Kotlin (P-3) | keep shared name / separate names | separate `MetadataUpdate-manga` and `MetadataUpdate-anime`; `distinct` by id (P-13) | Decided (T-26a-050..T-26a-054) |
| D10 | Tapping a new-item notification opens the oldest new item (P-8) | keep / open the highest number | open the highest-numbered new item | Decided (T-26a-034) |
| D11 | reSmush.it is called over plain HTTP with no fallback (P-15) | keep / HTTPS-first with fallback / drop the proxy | HTTPS first, fall back to HTTP only if HTTPS fails, and show a settings note that the image URL goes out in clear text on the fallback path | **User call** on the fallback or drop (T-26a-044) |
| D12 | wsrv.nl URL is not percent-encoded (P-14) | keep / encode | percent-encode the image URL | Decided (T-26a-042) |
| D13 | Notification ids collide between kinds (P-11) | keep / namespace | FNV-1a of `(kind, id)` mapped to `u32`; ids are internal, so exact Java hash reproduction is not needed | Decided |
| D14 | Error log file name and location | Kotlin `aniyomi_update_errors.txt` in the cache dir / Linux name | `$XDG_CACHE_HOME/kurobayomi/update_errors.txt` | Decided (T-26a-095..T-26a-097) |
| D15 | In-app message duration | 3 s / 4 s | 4 s, matching the Material Snackbar short duration (fixed this pass; `p1-rust-design.md` section 4 had both values) | Decided |
| D16 | Summary and per-entry notification urgency | Kotlin default importance / critical | `normal`; never `critical` | Decided (`p1-rust-design.md` 2.6) |
| D17 | GNOME ignores notification action buttons for some apps | rely on buttons / make default action primary | default action on each per-entry notification opens the item; buttons are secondary. Tested target is KDE Plasma 6 | Decided (`p1-topic-notifications.md` section 5) |
| D18 | Scheduler wake-up cadence for constraint checks | poll every 30 s / event-driven | poll every 30 s; log constraint changes at `debug!` once per change | Decided |

Notes that the implementer must not "fix" silently:

- Restriction token `manga_ongoing` means "skip COMPLETED". The name is misleading, but the behaviour is parity (`p1-rust-design.md` pitfall 4).
- Manga and anime both use `chop` with 40 and 45 characters. The exact Kotlin `chop` lives in `eu.kanade.tachiyomi.util.lang` and was not read in this pass. `T-26a-037` compares the Rust version against recorded Kotlin output before release.

---

## 5. Cross-area dependencies

### 5.1 This area needs

| From | What | Used by |
|---|---|---|
| area:01-foundation-app-bootstrap | tokio runtime bootstrap (`T-26a-131`), GTK toast helper (`T-26a-113`), notification action receiver, GApplication single instance | scheduler, pass runner, UI |
| area:02-preferences-system | typed preference storage, keys and defaults from `p1-data-and-config.md` section 3, settings widgets (`T-26a-104`, `T-26a-128`), `hideNotificationContent` | everything |
| area:03-db-manga-schema-migrations | `mangas`, `chapters`, `excluded_scanlators`, `mangas_categories`, `libraryView`, plus the new `library_job_run` table (`T-26a-129`) | queue, sync, lock |
| area:04-db-anime-schema-migrations | `animes`, `episodes`, `animeseasonsView`, `animelibView` | anime queue, sync, season children |
| area:05-data-repos-entries-items | entry and item repository types | model (`T-26a-001`) |
| area:07-domain-entries-items | `UpdateManga`, `UpdateAnimeFromRemote`, `SyncChaptersWithSource`, `SyncEpisodesWithSource` semantics | sync (`T-26a-080`, `T-26a-081`) |
| area:08-domain-library-category-history-updates | `MangaFetchInterval`, `AnimeFetchInterval`, `GetLibraryManga`, `GetLibraryAnime`, next-unread and next-unseen (`T-26a-061`, `T-26a-064`) | fetch interval, queue |
| area:09-domain-source-track-download-release-misc | `FilterChaptersForDownload`, `FilterEpisodesForDownload`, `DownloadPreferences` | download hand-off (`T-26a-092`) |
| area:15-source-api-manga, area:16-source-api-anime, area:17-source-app-layer | source trait, `getOrStub` semantics, `SourceNotInstalledException` | per-entry flow (`T-26a-006`) |
| area:23-downloads-manga, area:24-downloads-anime | `downloadChapters(..., autoStart)`, `startDownloads()`, `removeAfterMarkedAsRead` | hand-off and notification actions |
| area:34-images-covers-cache | cover image cache and resizing for notification icons | notifications |

### 5.2 Other areas need from this one

| Consumer | What they need | Source task(s) |
|---|---|---|
| area:37-ui-library | "Update library" per category; `start_now(kind, Some(category))`; disabled state while running | `T-26a-016`, `T-26a-102` |
| area:42-ui-updates-history-stats-storage-upcoming | `library_update_last_timestamp`, `new_manga_updates_count` and `new_anime_updates_count` counters, library job state, MarkRead handling | `T-26a-025`, `T-26a-026` |
| area:43-reader-core, area:44-reader-viewers | `next_unread` and `dedupe_by_number`, the `DataSaver` proxy | `T-26a-061`, `T-26a-062`, `T-26a-040..T-26a-046` |
| area:45-player-core | `next_unseen` and `filter_downloaded` for episodes | `T-26a-063`, `T-26a-064` |
| area:23-downloads-manga, area:24-downloads-anime | enqueue with `autoStart = false` and one `start()` at pass end | `T-26a-092` |
| area:22-backup-restore | after restore, re-arm the schedule from prefs; restore must not touch `library_update_last_timestamp` (`PreferenceRestorer.kt:45-46`) | `T-26a-014` |
| area:01-foundation-app-bootstrap | `NotifyAction` handlers for MarkRead, OpenItem, Download, CancelPass, OpenErrorLog, OpenFaq | `p1-topic-notifications.md` section 2 |

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then the rows of `TASKS.csv` whose `file` starts with `26-library-update-jobs/`.

**Totals: 104 tasks.** Lint of `p1-tasks.md`: `ok`.

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 22 | 13 | 4 | 0 | 39 |
| P1 | 34 | 9 | 0 | 0 | 43 |
| P2 | 14 | 1 | 0 | 0 | 15 |
| P3 | 7 | 0 | 0 | 0 | 7 |
| **Total** | **77** | **23** | **4** | **0** | **104** |

The four L tasks are `T-26a-005` (SQLite library store), `T-26a-009` (anime per-entry flow with seasons), `T-26a-080` (chapter sync diff) and `T-26a-081` (episode sync diff with sticky fields). All four are P0.

### 6.1 P0 critical path (in dependency order)

1. `T-26a-131` tokio runtime bootstrap (S, `area:01`)
2. `T-26a-129` library job run table and per-kind lock (M, `area:03`)
3. `T-26a-004` verify stored enum and integer mappings (S, `area:03`, `area:04`)
4. `T-26a-001` core entry model and enums (S)
5. `T-26a-070` fetch window (S), `T-26a-071` interval (M), `T-26a-072` next update (M), `T-26a-083` date rules (S), `T-26a-085` interval and lastUpdate recompute (S)
6. `T-26a-005` SQLite library store (L)
7. `T-26a-080` chapter sync diff (L) and `T-26a-081` episode sync diff (L), with `T-26a-087` (empty source list errors)
8. `T-26a-006` source trait and stub semantics (M)
9. `T-26a-007` pass runner skeleton (M)
10. `T-26a-008` manga per-entry flow (M), `T-26a-009` anime per-entry flow (L)
11. `T-26a-092` enqueue paused and start once per pass (S, depends on area:23 and area:24)
12. `T-26a-014` automatic scheduler loop (M), `T-26a-016` manual start (S), `T-26a-017` stop (S)
13. Surfaces that close P0: `T-26a-030`, `T-26a-032`, `T-26a-033` notifications; `T-26a-061`, `T-26a-064` next unread and unseen; `T-26a-102` library refresh buttons; `T-26a-104` interval setting; `T-26a-128` preference view; `T-26a-130` event channel

Step 7 is the longest chain: `T-26a-070` to `T-26a-071` to `T-26a-072` to `T-26a-085` to `T-26a-080` to `T-26a-008`. The sync diff is the riskiest part of the path and gets the most test coverage (`p1-edge-cases-and-tests.md` section 3).

### 6.2 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Source API contract (areas 15, 16, 17) not finalised | per-entry flow and sync cannot be tested against real data | build against the fake source in `p1-edge-cases-and-tests.md` section 9 first |
| Desktop lifetime: automatic passes stop when the window and process are gone | scheduled updates silently do not run | D1 decision; settings note when the portal is missing |
| KDE portal behaviour for background autostart | the permission dialog may be declined | fall back to mode B (timer) and show the state in settings |
| Sync fidelity: a wrong sticky-field rule loses user data (memo, fillermark, read state) | data loss in the database | the sync edge cases in section 3 of `p1-edge-cases-and-tests.md`, plus fixtures |
| Anime episode auto-download (D5) changes behaviour users have never seen | surprise downloads and disk use | default `download_new_episode = false`; visible toggle |
| Interval algorithm bugs around DST and negative overrides | entries checked too early or never | `chrono::NaiveDate` arithmetic; tests FI-01..FI-23 |
| Notification action support varies by desktop | buttons missing on GNOME | default action carries the primary behaviour (D17) |
| Multiple passes writing one SQLite file | lock contention or partial writes | per-kind lock row, single writer mutex with `spawn_blocking`, one transaction per write group |

---

## 7. Definition of done for this area

Observable behaviours, checked in the running app on KDE Plasma 6 with a fake or real source:

1. "Update library" on the Library screen refreshes every favourite manga; the progress notification shows `Updating library… (N%)` and updates in place; its Cancel action stops the pass without an error notification.
2. The same for anime; manga and anime passes can run at the same time with separate locks.
3. A manual pass started from a category refreshes only that category and ignores the include and exclude settings.
4. Automatic passes run at the configured interval (`0, 12, 24, 48, 72, 168` hours) and respect the Wi-Fi, unmetered and charging restrictions. The first pass after enabling runs as soon as the constraints are met. Missed runs are not replayed.
5. Setting the interval to `0` stops automatic passes; a running automatic pass completes.
6. Restrictions skip completed, caught-up, not-started and out-of-release-period entries in both manual and automatic passes. Skip reasons appear only in the debug log.
7. A pass that finds new items updates the Updates badge and writes the summary notification (`New chapters found` or `New episodes found`) with the entry count.
8. Each entry with new items gets a per-entry notification, with at most 5 numbers listed and titles truncated to 45 characters. Its default action opens the highest-numbered new item.
9. Enabling "Hide notification content" removes entry titles from every notification; the summary keeps its count text.
10. An entry whose source fails shows the error notification `N update(s) failed` after the pass; its action opens the error log in the default text viewer. The log lists each failure with the message `No chapters found`, `No episodes found`, or `Source not found`, as applicable.
11. A source that returns zero items records a failure (except local sources), while other entries in the same pass still update.
12. New chapters and episodes are queued for download with `autoStart = false`; `startDownloads()` is called once at pass end, and only if something was queued. This includes new anime episodes (P-1 fixed).
13. Read and seen state, bookmarks, memos, and fillermarks are never changed by a sync (except the explicit carry-over rules in `p1-behavior.md` section 5).
14. The "Refresh library covers" action runs the metadata pass for every library entry with `fetchDetails = true`; it is manual only and runs for manga and anime concurrently.
15. The Worker info screen (`T-26a-110`; replaces the Kotlin `settings_worker_info` dump) lists `library_job_run` rows with kind, trigger, state, start time, and done, total and failed counts.
16. The fetch-interval test suite FI-01..FI-23 passes, and the queue, sync, scheduler and notification-text tests in `p1-edge-cases-and-tests.md` pass.
17. `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint` prints `ok` for `p1-tasks.md`.
