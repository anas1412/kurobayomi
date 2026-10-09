# Area 26 - Library update jobs: implementation tasks (part 1)

Task ids are `T-26a-NNN`. Blocks: 001-029 passes and scheduler; 030-037 notifications; 040-046 data saver; 050-054 metadata passes; 060-064 chapter/episode helpers; 070-075 fetch interval; 080-089 sync; 090-094 download hand-off; 095-097 error log; 100-113 UI; 120-127 dropped or replaced Android-only items; 128-131 shared plumbing.

Conventions: Priority per the plan rules (P0 = needed for daily use; P1 = parity most users rely on; P2 = long tail; P3 = Android-only drop with replacement). Effort S <= 1 day, M <= 3 days, L <= 1.5 weeks.

---

### T-26a-001 — Core entry model and enums for library passes
- Priority: P0
- Effort: S
- Depends: T-26a-004, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/library/manga/LibraryManga.kt, domain/src/main/java/tachiyomi/domain/library/anime/LibraryAnime.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt
- Acceptance:
  - `LibraryEntry`, `EntryKind`, `UpdateStrategy`, `FetchType` exist in `src/core/library_update/queue.rs` with the fields in `p1-rust-design.md` section 2.2.
  - `unread_or_unseen` equals `total - read` for every row fixture (unit test).
  - `FetchType::Seasons` maps from stored integer 0 and `Episodes` from 1 (unit test).

### T-26a-002 — Category selection for automatic and manual passes
- Priority: P0
- Effort: M
- Depends: T-26a-001, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - Manual run with a category id returns only entries with that category and ignores include/exclude prefs (test Q-04, Q-05).
  - Automatic run: include list empty means all; exclude wins over include; `distinct` by id keeps one row (tests Q-02, Q-03).
  - Anime keys `animelib_update_categories` and `animelib_update_categories_exclude` are used for anime; manga keys `library_update_categories` and `library_update_categories_exclude` for manga.

### T-26a-003 — Item restrictions, skip reasons and title ordering
- Priority: P0
- Effort: M
- Depends: T-26a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - Five checks run in the order of `p1-behavior.md` 3.3; first match skips; "not always update" is unconditional.
  - Token mapping: `manga_ongoing` skips COMPLETED (status 2); `manga_fully_read` uses `!= 0`; `manga_started` requires `total > 0`; `manga_outside_release_period` compares with `window.second` (tests Q-06..Q-12).
  - Default restriction set is `{manga_fully_read, manga_ongoing, manga_started, manga_outside_release_period}` when the pref is absent.
  - Output sorted by title; skip reasons logged at debug level only.
  - Manual passes apply the same restrictions (test Q-12).

### T-26a-004 — Verify stored enum and integer mappings from the database
- Priority: P0
- Effort: S
- Depends: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations
- Sources: data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/sqldelightanime/view/animelibView.sq, data/src/main/java/tachiyomi/data/entries/anime/AnimeMapper.kt, data/src/main/java/tachiyomi/data/entries/manga/MangaMapper.kt
- Acceptance:
  - Ordinal mapping for `update_strategy` (0 ALWAYS_UPDATE, 1 ONLY_FETCH_ONCE) and `fetch_type` (0 Seasons, 1 Episodes) confirmed from the SQLDelight column adapters (the adapter source is not in scope; find it under `data/src/main/java/tachiyomi/data/`) and recorded in `p1-data-and-config.md` section 2.
  - `calculate_interval` column maps to `fetchInterval` (both kinds), verified by a round-trip test writing -3 and reading -3.

### T-26a-005 — SQLite library store: library, season children, favourite checks
- Priority: P0
- Effort: L
- Depends: T-26a-001, T-26a-004, area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations
- Sources: data/src/main/sqldelight/view/libraryView.sq, data/src/main/sqldelightanime/view/animelibView.sq, data/src/main/sqldelightanime/view/animeseasonsView.sq, domain/src/main/java/tachiyomi/domain/items/season/interactor/GetAnimeSeasonsByParentId.kt
- Acceptance:
  - `library()` returns one row per (favourite entry, category) with the same counts as the views (fixture `library_small.json`).
  - `season_children(parent)` returns `[]` and logs on query error (test Q-16).
  - `is_favorite(id)` reads `favorite` for manga and anime; anime season children are identified by `parent_id IS NOT NULL`.
  - Read queries run in `spawn_blocking`; no GTK calls.

### T-26a-006 — Source trait and not-installed stub semantics
- Priority: P0
- Effort: M
- Depends: area:17-source-app-layer, area:15-source-api-manga, area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt, domain/src/main/java/tachiyomi/domain/source/manga/service/MangaSourceManager.kt
- Acceptance:
  - `get_or_stub(id)` on a missing source returns a stub whose calls yield `UpdateError::SourceNotInstalled` (test P-05).
  - `get(id)` returns `None` for metadata passes and they skip silently (test in T-26a-053).
  - `Source::is_unmetered()` is the `UnmeteredSource` marker and exempts the source from the size warning (test Q-18).

### T-26a-007 — Pass runner skeleton: lock, timestamp, events, outcome
- Priority: P0
- Effort: M
- Depends: T-26a-001, T-26a-129, T-26a-131
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - `run_pass` takes the per-kind lock (T-26a-129) and returns `Skipped` when held.
  - `library_update_last_timestamp` is written at pass start even when the queue is empty (test P-19).
  - `PassEvent::Started`, `EntryStarted`, `EntryFinished`, `Finished` are emitted in order; `done` is monotonic.
  - Empty library ends with `success` and removes the progress notification (test Q-01).

### T-26a-008 — Manga per-entry flow
- Priority: P0
- Effort: M
- Depends: T-26a-007, T-26a-080, T-26a-070, T-26a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - Order: favourite gate (skip if not favourite) -> optional details (when `auto_update_metadata`) -> chapter list -> favourite re-read (return empty if removed) -> sync -> sort by `source_order` descending (test P-03, P-04, P-14).
  - `NoChapters` maps to "No chapters found"; `SourceNotInstalled` to "Source not found" (test P-05).
  - `new_items.len()` increments `new_manga_updates_count` (test P-18).

### T-26a-009 — Anime per-entry flow with seasons
- Priority: P0
- Effort: L
- Depends: T-26a-007, T-26a-081, T-26a-070, T-26a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt, mihon/domain/source/interactor/UpdateAnimeFromRemote.kt
- Acceptance:
  - Season expansion (behaviour 3.4) happens before restrictions; Seasons-type parents are never queued directly; nested Seasons children are ignored (tests Q-13, Q-14, Q-15).
  - Season children skip the favourite gate (`parent_id != null`), top-level anime do not (test).
  - Favourite re-check happens before the sync (deliberate change P-7, see T-26a-024).
  - `update.anime.favorite` false -> no new items returned.

### T-26a-010 — Decision: first automatic run after enabling
- Priority: P1
- Effort: S
- Depends: T-26a-014, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt
- Acceptance:
  - Decision recorded in `p1-topic-scheduling.md` section 5: parity (run as soon as constraints are met) unless the user rejects it.
  - Settings page text states when the first run happens.

### T-26a-011 — Decision: missed runs are not replayed
- Priority: P2
- Effort: S
- Depends: T-26a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - Scheduler does not replay missed periods after sleep or closed app (test SC-13, SC-14).
  - Startup arms the next run from the current time.

### T-26a-012 — Deviation: battery-not-low constraint not enforced on desktop
- Priority: P2
- Effort: S
- Depends: T-26a-028, T-26a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - Deviation recorded in `p1-topic-scheduling.md` and in the settings help text.
  - No code path reads battery percentage for the constraint.

### T-26a-013 — Queue ordering and byte-order title comparison
- Priority: P1
- Effort: S
- Depends: T-26a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - Queue sorted by title ascending with `String::cmp` (test Q-17).
  - Non-ASCII ordering difference from Kotlin documented in the test file for supplementary characters.

### T-26a-014 — Automatic scheduler loop (interval, flex, backoff)
- Priority: P0
- Effort: M
- Depends: T-26a-129, T-26a-027, T-26a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - Interval 0 cancels the loop; interval N hours runs with jitter in [N h - 10 min, N h] (test SC-01, SC-02).
  - Retry backoff is linear 10 min steps capped at 5 h and reset on success (test SC-08).
  - Config change replaces the loop without cancelling a running pass (test SC-03).
  - Manga and anime loops are independent (test SC-12).

### T-26a-015 — Automatic pass yields to a running manual pass
- Priority: P1
- Effort: S
- Depends: T-26a-014, T-26a-129
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - Automatic pass returns retry while a manual pass of the same kind holds the lock (test P-10).
  - Applies to anime as well (deliberate fix of P-5, which Kotlin lacks for anime).

### T-26a-016 — Manual start semantics (startNow)
- Priority: P0
- Effort: S
- Depends: T-26a-129, T-26a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - `start_now(kind, category)` returns false if a pass of that kind is running (test P-09, R-01), true otherwise.
  - Optional category id is passed through (test Q-04).
  - Both library tabs show "Updating category" or "An update is already running" accordingly (see T-26a-102).

### T-26a-017 — Stop semantics (cancel and re-arm)
- Priority: P0
- Effort: S
- Depends: T-26a-016, T-26a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - `stop(kind)` cancels the running pass token and sets state `cancelled` (test P-08).
  - When the stopped pass was automatic, the loop re-arms (test SC-09); manual stop does not re-arm.
  - Metadata stop does not re-arm (test SC-11).

### T-26a-018 — Crash recovery of stale running rows
- Priority: P1
- Effort: S
- Depends: T-26a-129
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - On startup any `library_job_run` row with state `running` becomes `cancelled` (test P-20).

### T-26a-019 — Background running via the portal (mode A)
- Priority: P1
- Effort: M
- Depends: T-26a-014, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - When automatic updates are on, the app requests background running with autostart via `org.freedesktop.portal.Background`; the request result is stored.
  - Without the portal the settings page shows "Automatic updates run only while Kurobayomi is open." and the loops still run while the process lives.
  - Turning automatic updates off withdraws autostart.

### T-26a-020 — Headless scheduled run via systemd user timer (mode B)
- Priority: P2
- Effort: M
- Depends: T-26a-014, T-26a-007, T-26a-131
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - `kurobayomi --library-update=auto` runs one pass per kind and exits 0 (or exits 0 with "already running").
  - Timer unit written only after explicit user action; mode A and B are mutually exclusive (test in topic file).

### T-26a-021 — Cancellation is not a failure
- Priority: P0
- Effort: S
- Depends: T-26a-008, T-26a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - Cancellation observed inside an entry is not recorded in the failure list (fix P-10; test P-08).
  - Outcome of a cancelled pass has `cancelled = true` and no error notification is shown for it.

### T-26a-022 — Process-wide per-source politeness permit
- Priority: P1
- Effort: S
- Depends: T-26a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - Within one pass: at most 5 source groups run concurrently and entries of one source are sequential (tests P-01, P-02).
  - Across manga and anime passes: a single source never has two concurrent requests (process-wide map of 1-permit semaphores).
  - Deliberate improvement recorded in `p1-topic-update-pipeline.md`.

### T-26a-023 — Per-entry error collection and failure outcome
- Priority: P0
- Effort: S
- Depends: T-26a-008, T-26a-009
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - Entry error messages are never empty (test P-07); null messages become the Debug text.
  - Other entries continue after an error (test P-06).
  - Pass outcome is `success` unless the queue build or the final notification step fails (test F-02).

### T-26a-024 — Anime new-episode download wiring and favourite gate
- Priority: P0
- Effort: S
- Depends: T-26a-009, T-26a-092, T-26a-091
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt, mihon/domain/items/episode/interactor/FilterEpisodesForDownload.kt
- Acceptance:
  - New episodes that pass `FilterEpisodesForDownload` are enqueued with autostart=false (fixes P-1; Kotlin's `downloadEpisodes` helper is dead code).
  - Anime favourite is re-checked before sync (fixes P-7).
  - Test P-13 passes: with download-new on, one pass enqueues the expected episodes and starts downloads once.

### T-26a-025 — New-item counters
- Priority: P0
- Effort: S
- Depends: T-26a-008, T-26a-009, T-26a-129
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - Manga adds to `library_unread_updates_count`, anime to `library_unseen_updates_count`, by the number of new items per entry (test P-18).
  - Increment is atomic with respect to concurrent entries.

### T-26a-026 — Last-updated timestamp
- Priority: P0
- Effort: S
- Depends: T-26a-007, T-26a-131
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Key `library_update_last_timestamp` (app state) holds the ms timestamp of the last pass start (test P-19).
  - Updates screen reads it (T-26a-100).

### T-26a-027 — Network constraints from NetworkManager
- Priority: P1
- Effort: M
- Depends: T-26a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkExtensions.kt
- Acceptance:
  - Wi-Fi-only blocks passes when `PrimaryConnectionType` is not `802-11-wireless` (test SC-04).
  - Unmetered-only treats `NMMetered` values 0 and 3 as metered (test SC-05).
  - Missing NetworkManager falls back to "constraints satisfied" (test SC-07).

### T-26a-028 — Charging constraint from UPower
- Priority: P2
- Effort: S
- Depends: T-26a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - When-charging blocks passes while UPower reports `OnBattery == true` (test SC-06).
  - Missing UPower falls back to "constraints satisfied".

### T-26a-029 — Single-instance forwarding of start-update requests
- Priority: P2
- Effort: S
- Depends: T-26a-016, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - Second process started with `--library-update=manual` forwards to the primary instance and exits 0 (GApplication command line).
  - Only one scheduler and one lock holder exist (test F-11).

### T-26a-030 — Progress notification text and percent helper
- Priority: P0
- Effort: S
- Depends: T-26a-036, T-26a-037
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt
- Acceptance:
  - Title "Updating library… (N%)" with floor percent (tests N-10, N-11, N-12).
  - Body lists in-progress titles chopped to 40 chars, omitted when content is hidden.
  - Progress updates after each entry starts and finishes; cancel action present (topic notifications).

### T-26a-031 — Large-queue size warning
- Priority: P1
- Effort: S
- Depends: T-26a-006, T-26a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt
- Acceptance:
  - Shown when any non-unmetered source has more than 60 queued entries (test Q-18).
  - Timeout 30 s; tap opens the FAQ URL; not suppressed by hidden content.

### T-26a-032 — Summary notification (new items found)
- Priority: P0
- Effort: M
- Depends: T-26a-030, T-26a-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt
- Acceptance:
  - One notification per kind; one entry shows its title (chop 45), multiple show "For N entries" or "For N titles" (tests N-17, P-15).
  - Hidden content shows the generic count only.
  - Posted before downloads start (order from the Kotlin pass end).

### T-26a-033 — Per-entry description text (number formatting and truncation)
- Priority: P0
- Effort: M
- Depends: T-26a-037
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt
- Acceptance:
  - All branches of `getNewChaptersDescription` and `getNewEpisodesDescription` produce the exact strings in `p1-behavior.md` 8.6 (tests N-01..N-09).
  - Number formatting "2.5", "3" (no trailing zero) (test N-09).
  - Duplicate numbers collapse after formatting (test N-08).

### T-26a-034 — Per-entry notification: actions and tap target
- Priority: P1
- Effort: M
- Depends: T-26a-033, T-26a-093, T-26a-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - Actions: Mark as read / Mark as watched, View chapters / View episodes, Download only when item count <= 15 (manga) or <= 10 (anime) (test).
  - Default (tap) action opens the newest new item (deliberate change P-8), not the oldest.
  - Hidden content suppresses the notification entirely.

### T-26a-035 — Error notification and error log opener
- Priority: P1
- Effort: S
- Depends: T-26a-095, T-26a-096
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt
- Acceptance:
  - Title "N update(s) failed", text "Tap to see details", urgency low, shown only when N > 0 (test P-16).
  - Opening the action launches the log in the default text viewer; if the file is missing the notification has no action.

### T-26a-036 — Notification id namespace and hidden-content policy
- Priority: P1
- Effort: S
- Depends: T-26a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt
- Acceptance:
  - Per-entry ids derived from (kind, entry id) with FNV-1a into `u32`; no collision between manga and anime in fixture (fixes P-11).
  - Progress, error and warning ids are distinct per kind where Kotlin shares them (documented).
  - `hide_notification_content` (area:02-preferences-system) controls titles in every text listed in `p1-ui-parity.md`.

### T-26a-037 — Chop semantics for titles and UTF-8 safety
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt
- Acceptance:
  - `chop(s, n)` truncates on character boundaries (test N-15) and appends "..." only when cut; the exact Kotlin `chop` (eu/kanade/tachiyomi/util/lang, not read in this pass) is compared against a recorded sample before release.
  - Lengths 40 (progress) and 45 (summary, per-entry list) used consistently.

### T-26a-040 — Data saver factory, modes and exclusions
- Priority: P1
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/aniyomi/util/DataSaver.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Modes `None`, `BandwidthHero`, `WsrvNl`, `ReSmushIt` stored by name; default `NONE`.
  - Source id in `data_saver_excluded` yields the no-op compressor when mode is not None (test).

### T-26a-041 — Bandwidth Hero URL builder
- Priority: P1
- Effort: S
- Depends: T-26a-040
- Sources: app/src/main/java/aniyomi/util/DataSaver.kt
- Acceptance:
  - Template `$server/?jpg=$format&l=$quality&bw=$colorBW&url=$escaped` with `1`/`0` flags and form encoding (golden test with a space and `&`).
  - Empty server returns the original URL; trailing slash trimmed.
  - jpg/jpeg respects `ignore_jpeg`; gif respects `ignore_gif` (default true).

### T-26a-042 — wsrv.nl URL builder
- Priority: P1
- Effort: S
- Depends: T-26a-040
- Sources: app/src/main/java/aniyomi/util/DataSaver.kt
- Acceptance:
  - All four suffix branches of `WsrvNlDataSaver.getUrl` reproduced (golden tests).
  - Image URL percent-encoded (deliberate fix P-14); expected output recorded in `datasaver_urls.txt`.

### T-26a-043 — reSmush.it async compressor with JSON parsing and fallback
- Priority: P1
- Effort: M
- Depends: T-26a-040, T-26a-044
- Sources: app/src/main/java/aniyomi/util/DataSaver.kt
- Acceptance:
  - Request is async with a 10 s timeout; response parsed as JSON field `dest` (not substring search).
  - Any error or missing field returns the original URL (fix P-15).
  - Does not block the reader loader thread (test: runtime worker not blocked for 2 s with a slow mock).

### T-26a-044 — Decision: reSmush.it transport and encoding
- Priority: P2
- Effort: S
- Depends: none
- Sources: app/src/main/java/aniyomi/util/DataSaver.kt
- Acceptance:
  - Recorded decision: use HTTPS when the service answers over HTTPS; otherwise keep HTTP and show a settings note that the image URL is sent in clear text.
  - Default recorded in `p1-data-and-config.md` section 5.2.

### T-26a-045 — Page image fetch without shared mutation
- Priority: P1
- Effort: S
- Depends: T-26a-041, T-26a-042, T-26a-043
- Sources: app/src/main/java/aniyomi/util/DataSaver.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - The wrapper clones the page and never mutates the shared page URL (replaces the Kotlin save/restore in `getImage`).
  - Original URL is used for cache keys; compressed URL only for the request (test with concurrent requests on one page).

### T-26a-046 — Golden tests for every proxy
- Priority: P1
- Effort: S
- Depends: T-26a-041, T-26a-042, T-26a-043
- Sources: aniyomi-src data saver file (app/src/main/java/aniyomi/util/DataSaver.kt)
- Acceptance:
  - `fixtures/datasaver_urls.txt` has 6 inputs per proxy; all match.
  - Test covers jpg, jpeg with ignore, gif with ignore false, webp, png, URL with space and `&`.

### T-26a-050 — Manual-only metadata passes (refresh covers)
- Priority: P1
- Effort: S
- Depends: T-26a-016, T-26a-054
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaMetadataUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeMetadataUpdateJob.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - Metadata passes are only started manually (settings action T-26a-109); no schedule.
  - Each pass is refused if a metadata pass is already running.

### T-26a-051 — Shared lock fix and de-duplication for metadata passes
- Priority: P1
- Effort: S
- Depends: T-26a-050, T-26a-129
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaMetadataUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeMetadataUpdateJob.kt
- Acceptance:
  - Manga and anime metadata passes use distinct lock names (fixes P-3); starting one does not refuse the other.
  - Queue is distinct by entry id (fixes P-13; test in `p1-edge-cases-and-tests.md`).

### T-26a-052 — Anime metadata pass: details-only semantics
- Priority: P1
- Effort: S
- Depends: T-26a-050, T-26a-081
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeMetadataUpdateJob.kt, mihon/domain/source/interactor/UpdateAnimeFromRemote.kt
- Acceptance:
  - Calls the anime update with `fetch_details = true` and `fetch_episodes = false` (test with fake source: details called, episode list endpoint not called).
  - Covers and titles update per behaviour 5.3.

### T-26a-053 — Manga metadata pass: silently skip missing sources
- Priority: P1
- Effort: S
- Depends: T-26a-006, T-26a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaMetadataUpdateJob.kt
- Acceptance:
  - Missing source is skipped with no error recorded (test).
  - Errors per entry are logged and ignored; the pass completes.

### T-26a-054 — Metadata pass progress and cancel
- Priority: P2
- Effort: S
- Depends: T-26a-030, T-26a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaMetadataUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeMetadataUpdateJob.kt
- Acceptance:
  - Progress notification shown with the same text as updates; cancel stops the pass with no re-arm (test SC-11).

### T-26a-060 — Filter chapters to downloaded ones
- Priority: P2
- Effort: S
- Depends: area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterFilterDownloaded.kt
- Acceptance:
  - Local manga returns the input unchanged; otherwise keeps chapters present in the download cache (test with fixture).

### T-26a-061 — Next unread chapter for "continue reading"
- Priority: P0
- Effort: S
- Depends: area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterGetNextUnread.kt
- Acceptance:
  - Descending sort returns the last unread; ascending returns the first unread (`rev().find` for descending).
  - Returns `None` when all are read (unit test).

### T-26a-062 — Collapse duplicate-number chapters in the reader
- Priority: P1
- Effort: S
- Depends: area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterRemoveDuplicates.kt
- Acceptance:
  - Groups by chapter number with `-0.0` mapped to `0.0`; representative = current id, else same scanlator, else first (unit test).
  - Output order is first-appearance group order.

### T-26a-063 — Filter episodes to downloaded ones
- Priority: P2
- Effort: S
- Depends: area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/episode/EpisodeFilterDownloaded.kt
- Acceptance:
  - Local anime returns the input unchanged; otherwise keeps downloaded episodes (test with fixture).

### T-26a-064 — Next unseen episode for "continue watching"
- Priority: P0
- Effort: S
- Depends: area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/episode/EpsiodeGetNextUnseen.kt
- Acceptance:
  - Descending sort returns the last unseen; ascending the first unseen (unit test).
  - Returns `None` when all are seen.

### T-26a-070 — Fetch window (yesterday to tomorrow, local zone)
- Priority: P0
- Effort: S
- Depends: none
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchInterval.kt
- Acceptance:
  - `window_for(now)` returns `(yesterday 00:00 ms, tomorrow 00:00 ms - 1)` in the device zone (test FI-14 family).
  - Computed once per pass and reused (test).

### T-26a-071 — Interval calculation from upload and fetch dates
- Priority: P0
- Effort: M
- Depends: T-26a-070
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchInterval.kt
- Acceptance:
  - Window 3 for <= 8 items else 10; upload branch needs >= 3 distinct dates, fetch branch likewise, default 7; clamp 1..=28 (tests FI-01..FI-13).
  - Floor division for the average gap.

### T-26a-072 — Next update computation and interval increase
- Priority: P0
- Effort: M
- Depends: T-26a-071, T-26a-070
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt
- Acceptance:
  - Kept when inside the inclusive window (FI-18, FI-19, FI-20, FI-20b).
  - Recomputed otherwise from `last_update` (or now when 0) with the increase rule (FI-17 table).
  - Interval never exceeds 28 by increase.

### T-26a-073 — Manual fetch-interval override (negative values)
- Priority: P1
- Effort: S
- Depends: T-26a-072
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt
- Acceptance:
  - Stored negative value is kept and means fixed abs(value) days (FI-15).
  - Negative values are never clamped.

### T-26a-074 — Fetch interval test suite ported from upstream
- Priority: P0
- Effort: M
- Depends: T-26a-071, T-26a-072
- Sources: domain/src/test/java/tachiyomi/domain/entries/manga/interactor/MangaFetchIntervalTest.kt, domain/src/test/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchIntervalTest.kt
- Acceptance:
  - All ten upstream scenarios (FI-01..FI-10) pass with the expected values in `p1-edge-cases-and-tests.md` section 1.
  - Single implementation covers manga and anime (no duplication).

### T-26a-075 — Local-zone and DST correctness
- Priority: P1
- Effort: S
- Depends: T-26a-072
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt
- Acceptance:
  - Day arithmetic uses calendar dates in the zone; DST-day test FI-22 passes for Europe/Paris and America/New_York.
  - Negative day differences (future last_update) are handled (FI-23).

### T-26a-080 — Chapter sync diff: new, updated, removed
- Priority: P0
- Effort: L
- Depends: T-26a-005, T-26a-083, T-26a-085, T-26a-087
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt
- Acceptance:
  - Steps 1-19 of behaviour 5.4 reproduced; diff keyed by URL.
  - Writes only the fields listed in `p1-topic-update-pipeline.md` section 4 for existing rows (test R-05).
  - Returned list is the new items after the carry-over and excluded-scanlator filter (tests S-03, S-07..S-10).

### T-26a-081 — Episode sync diff with sticky fields
- Priority: P0
- Effort: L
- Depends: T-26a-005, T-26a-083, T-26a-085, T-26a-087
- Sources: app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt
- Acceptance:
  - Same diff as chapters with summary, memo, fillermark (sticky true), preview URL (sticky non-blank) rules (tests S-18, S-19).
  - Returned list excludes changed or duplicate-read URLs (no excluded-scanlator filter).

### T-26a-082 — Carry-over rules for removed read or bookmarked items
- Priority: P1
- Effort: M
- Depends: T-26a-080, T-26a-081
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt, app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt
- Acceptance:
  - A removed read item's number makes a re-added item with the same number read and not new (tests S-07, S-08).
  - `dateFetch` taken from the oldest removed entry with that number (test).

### T-26a-083 — Date assignment rules (dateUpload and dateFetch)
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt, app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt
- Acceptance:
  - Zero upload date becomes `max_seen_upload` or now (tests S-04, S-05).
  - New items get `dateFetch = now + N` ... `now + 1`, newest first (test S-06).

### T-26a-084 — Duplicate-read and excluded-scanlator filters
- Priority: P1
- Effort: S
- Depends: T-26a-080
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt, app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt
- Acceptance:
  - `mark_duplicate_read_chapter_read` containing `new` marks matching new items read and excludes them (test S-09).
  - Excluded scanlators stay in the DB but not in the returned list (test S-10).

### T-26a-085 — Interval and lastUpdate recompute rules
- Priority: P0
- Effort: S
- Depends: T-26a-072
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt, app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt
- Acceptance:
  - No change: recompute only when manual, interval 0, or next update before window start (tests S-11, S-12).
  - Changes present: recompute and set `lastUpdate = now` (test S-13).

### T-26a-086 — Rename downloaded folders on name change
- Priority: P1
- Effort: S
- Depends: area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt, app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt
- Acceptance:
  - Rename only when the directory name changed and the old item is downloaded (test S-17).

### T-26a-087 — Empty source list errors
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt, app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt
- Acceptance:
  - Empty non-local source list raises `NoChapters` / `NoEpisodes` (tests S-01, S-02).

### T-26a-088 — Sync performance and transactions
- Priority: P1
- Effort: S
- Depends: T-26a-080, T-26a-081
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt, app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt
- Acceptance:
  - Diff uses hash sets; 500-item sync under 100 ms (PERF-02).
  - Each write group runs in one transaction; a failure rolls back that group only.

### T-26a-089 — Sync fixture tests
- Priority: P1
- Effort: M
- Depends: T-26a-080, T-26a-081, T-26a-082
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt
- Acceptance:
  - Fixture `chapters_newest_first.json` covers duplicates, unparseable numbers, zero upload dates (tests S-03, S-14, S-16).

### T-26a-090 — Chapter download filter for new items
- Priority: P1
- Effort: M
- Depends: area:09-domain-source-track-download-release-misc, area:23-downloads-manga
- Sources: domain/src/main/java/mihon/domain/items/chapter/interactor/FilterChaptersForDownload.kt
- Acceptance:
  - Global toggle `download_new` (default false), unread-only `download_new_unread_chapters_only` (default false) and category include/exclude with default category 0 reproduced (tests).
  - Non-favourite entries never enqueue.

### T-26a-091 — Episode download filter for new items
- Priority: P1
- Effort: M
- Depends: area:09-domain-source-track-download-release-misc, area:24-downloads-anime
- Sources: domain/src/main/java/mihon/domain/items/episode/interactor/FilterEpisodesForDownload.kt
- Acceptance:
  - Same semantics as T-26a-090 with `download_new_episode` and `download_new_unread_episodes_only`.
  - Category keys confirmed in DownloadPreferences (not read in this pass) and recorded in `p1-data-and-config.md` section 3.

### T-26a-092 — Enqueue paused, start once per pass
- Priority: P0
- Effort: S
- Depends: T-26a-090, T-26a-091, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - Enqueue uses autostart=false; `start()` called once at pass end and only when something was enqueued (test P-12).
  - Downloads do not start during the pass (test).

### T-26a-093 — Notification "Download" and "Mark as read" actions
- Priority: P1
- Effort: S
- Depends: T-26a-092, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - Mark-as-read writes read/seen for the listed ids and removes downloads when "remove after marked as read" is on.
  - Download enqueues with autostart true; missing entry is a silent no-op.

### T-26a-094 — Download threshold constants
- Priority: P2
- Effort: S
- Depends: T-26a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - Constants 15 (chapters) and 10 (episodes) in one module, used by notification and tests.

### T-26a-095 — Error log writer in the Kotlin format
- Priority: P1
- Effort: S
- Depends: T-26a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - Output equals `fixtures/errorlog_expected.txt` (test P-17): header with help URL, `! message`, `  # source`, `    - title`.
  - Written only when failures exist (test P-16).

### T-26a-096 — Error log location and opener
- Priority: P1
- Effort: S
- Depends: T-26a-095
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - File at `$XDG_CACHE_HOME/kurobayomi/update_errors.txt`, overwritten per pass.
  - Opened with the default `text/plain` application via gio.

### T-26a-097 — Null error message handling
- Priority: P2
- Effort: S
- Depends: T-26a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - Messages are never printed as "null" (fix P-9); test with a Throwable with no message.

### T-26a-100 — Updates screen: live last-updated text
- Priority: P1
- Effort: S
- Depends: T-26a-026
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Replaces the static "Library last updated: 5 minutes ago" text in `src/screens.rs` (updates screen) with relative time from `library_update_last_timestamp`.
  - Shows nothing when the value is 0.

### T-26a-101 — Updates badge with live counts
- Priority: P1
- Effort: S
- Depends: T-26a-025
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Sidebar badge in `src/main.rs` shows manga plus anime counts; hidden at zero; placeholder "8" removed.
  - Counts reset when the user opens the Updates tab (behaviour owned by Updates area; this task wires the badge).

### T-26a-102 — Library refresh button per category with toast feedback
- Priority: P0
- Effort: M
- Depends: T-26a-016, T-26a-113
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt
- Acceptance:
  - Button starts a pass for that category; toast "Updating category" when started, "An update is already running" when refused.
  - Spinner or disabled state while a pass of that kind runs.

### T-26a-103 — Updates tab refresh button
- Priority: P1
- Effort: S
- Depends: T-26a-016, T-26a-113
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt
- Acceptance:
  - Starts a global pass of the selected kind; same two toasts as T-26a-102.

### T-26a-104 — Settings: automatic updates interval
- Priority: P0
- Effort: S
- Depends: T-26a-014, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt
- Acceptance:
  - Choices "Off", "Every 12 hours", "Daily", "Every 2 days", "Every 3 days", "Weekly" stored as 0, 12, 24, 48, 72, 168 under key `pref_library_update_interval_key`.
  - Change reschedules both kinds immediately (test SC-03).
  - Default "Off".

### T-26a-105 — Settings: automatic updates device restrictions
- Priority: P1
- Effort: S
- Depends: T-26a-104, T-26a-027, T-26a-028
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt
- Acceptance:
  - Multi-select with labels "Only on Wi-Fi" (`wifi`), "Only on unmetered network" (`network_not_metered`), "When charging" (`ac`); default `{wifi}`.
  - Disabled when interval is Off; subtitle "Restrictions: %s".

### T-26a-106 — Settings: entry restrictions
- Priority: P1
- Effort: S
- Depends: T-26a-003
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Four tokens selectable with the defaults from `p1-data-and-config.md` 3; stored under `library_update_manga_restriction`.
  - Help text states that manual updates also apply these restrictions.

### T-26a-107 — Settings: category include and exclude dialog (tri-state)
- Priority: P1
- Effort: M
- Depends: T-26a-002, T-26a-113
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt
- Acceptance:
  - Dialog "Manga categories" and anime equivalent with included, excluded and neutral states per category; writes the two StringSet keys per kind.
  - Scrolls within 720 px height.

### T-26a-108 — Settings: season children and metadata toggles
- Priority: P1
- Effort: S
- Depends: T-26a-009
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Toggles `pref_update_season_on_library_update` (default false) and `auto_update_metadata` (default false) with their labels from `SettingsLibraryScreen.kt`.
  - Each toggle affects the next pass (test).

### T-26a-109 — Settings: refresh library covers action
- Priority: P1
- Effort: S
- Depends: T-26a-050
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - Starts the four jobs in order (anime pass, manga pass, anime metadata, manga metadata); silent, no toast.

### T-26a-110 — Worker info screen from the job-run table
- Priority: P2
- Effort: S
- Depends: T-26a-129
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - Replaces the static dump in `src/settings.rs` (screen id `settings_worker_info`) with the last 50 `library_job_run` rows.

### T-26a-111 — Library header progress indicator
- Priority: P2
- Effort: S
- Depends: T-26a-007, T-26a-102
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt
- Acceptance:
  - Determinate progress bar while a pass runs; hidden when idle.

### T-26a-112 — Desktop shortcuts for refresh
- Priority: P2
- Effort: S
- Depends: T-26a-102
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt
- Acceptance:
  - Ctrl+R refreshes the visible category; Ctrl+Shift+U starts a global pass of the visible kind (documented in the keybindings list).

### T-26a-113 — Toast helper in the GTK UI
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt
- Acceptance:
  - `toast(overlay, text)` shows a bottom-centre label for 4 s, replacing a visible toast; no libadwaita.
  - Used by T-26a-102, T-26a-103 and T-26a-107.

### T-26a-120 — Drop: Android foreground service declaration
- Priority: P3
- Effort: S
- Depends: T-26a-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - Drop reason recorded: no Android foreground service; the resident process or the systemd timer replaces it.
  - User loses nothing beyond background running when the app is closed in mode A without the portal (documented).

### T-26a-121 — Drop: Android notification channels and deprecated channel cleanup
- Priority: P3
- Effort: S
- Depends: T-26a-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt
- Acceptance:
  - Drop reason: freedesktop notifications have no channels; per-kind importance replaced by urgency. Single toggle for notifications instead (T-26a-036).

### T-26a-122 — Drop: Wi-Fi pre-check for API < 28
- Priority: P3
- Effort: S
- Depends: T-26a-027
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - Drop reason: Android version branch; replaced by the NetworkManager constraint (T-26a-027) for all cases.

### T-26a-123 — Replace: FileProvider URIs
- Priority: P3
- Effort: S
- Depends: T-26a-096
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - Replaced by plain `file://` URIs from gio (T-26a-096); no content provider.

### T-26a-124 — Replace: Injekt dependency lookup
- Priority: P3
- Effort: S
- Depends: T-26a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - Replaced by explicit `PassDeps` construction at startup; no global service locator.

### T-26a-125 — Drop: WorkManager migrations
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/mihon/core/migration/migrations/UseWorkManagerMigration.kt, app/src/main/java/mihon/core/migration/migrations/RemoveQuickUpdateMigration.kt, app/src/main/java/mihon/core/migration/migrations/RemoveOneTwoHourUpdateMigration.kt, app/src/main/java/mihon/core/migration/migrations/SetupBackgroundTasksMigration.kt, app/src/main/java/mihon/core/migration/migrations/SetupMangaLibraryUpdateMigration.kt, app/src/main/java/mihon/core/migration/migrations/SetupAnimeLibraryUpdateMigration.kt
- Acceptance:
  - Drop reason: a fresh Rust install has no Android WorkManager state to migrate; the stored interval value is validated instead (only 0, 12, 24, 48, 72, 168 accepted; other values snap to the nearest choice).

### T-26a-126 — Drop: Android-only notification bitmap decode
- Priority: P3
- Effort: S
- Depends: T-26a-127
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt
- Acceptance:
  - Drop reason: `BitmapFactory.decodeResource` of the launcher icon is replaced by the freedesktop app icon name (`image-path` not needed for the app icon).

### T-26a-127 — Replace: coil cover loading for notification icons
- Priority: P2
- Effort: S
- Depends: T-26a-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt
- Acceptance:
  - Cover loaded from the local cover cache with gdk-pixbuf, circle-cropped to 192 px, stored as PNG in the notification cache; failure omits the image hint.

### T-26a-128 — Preference view with defaults and token parsing
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Typed `Prefs` trait returns defaults from `p1-data-and-config.md` section 3 when the key is missing.
  - Unknown StringSet tokens are ignored (test).

### T-26a-129 — Library job run table and per-kind lock
- Priority: P0
- Effort: M
- Depends: area:03-db-manga-schema-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - Table `library_job_run` from `p1-data-and-config.md` 7.3 created by a migration.
  - Lock acquired in one transaction; second start of the same kind returns refused (test R-01).
  - Pass sets total, done, failed, new_items as it runs.

### T-26a-130 — Progress event channel into the GTK main loop
- Priority: P0
- Effort: S
- Depends: T-26a-007, T-26a-131
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - `PassEvent` values received by a `glib::spawn_future_local` consumer; no widget touched from worker threads (test by construction).
  - Dropping the receiver does not panic the pass.

### T-26a-131 — Tokio runtime bootstrap
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - One multi-thread runtime (4 workers) created in `main` and its handle passed to the scheduler and passes.
  - Runtime shuts down cleanly when the window closes unless background running is active.
