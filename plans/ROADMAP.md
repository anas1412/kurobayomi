# Kurobayomi roadmap

Scope: every task in `plans/TASKS.csv` (3801 tasks: P0 1196, P1 1793, P2 503, P3 309). Each task is assigned to exactly one milestone file under `plans/milestones/`. The assignment is produced by `plans/_tools/assign_milestones.py` from `plans/milestones/rules.json`; re-run that script after any task change.

## Reading the numbers

- Effort per task: S = 1 day, M = 3 days, L = 7 days, XL = 15 days. Milestone effort is the plain sum of its tasks. It assumes one person working serially with no parallelism, so it is an upper bound on serial work, not a calendar forecast.
- Cumulative days are the running sum in milestone order.
- The critical path is the longest chain of explicit `T-` dependencies, with each dependency cycle counted once. It is shorter than the sum because chains can run in parallel.

<!-- BEGIN GENERATED: assign_milestones.py -->
| Milestone | Name | Tasks | P0 | P1 | P2 | P3 | Effort (days) | Cumulative (days) |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| M00 | Foundation and platform | 81 | 35 | 44 | 2 | 0 | 131 | 131 |
| M01 | Preferences and databases | 137 | 89 | 48 | 0 | 0 | 335 | 466 |
| M02 | Domain, HTTP and source contract | 341 | 201 | 140 | 0 | 0 | 671 | 1137 |
| M03 | Local reading and playing | 486 | 288 | 185 | 13 | 0 | 1014 | 2151 |
| M04 | Daily-driver shell | 555 | 244 | 300 | 11 | 0 | 1063 | 3214 |
| M05 | First live source and browse | 296 | 143 | 145 | 5 | 3 | 548 | 3762 |
| M06 | Downloads | 253 | 74 | 178 | 1 | 0 | 559 | 4321 |
| M07 | Library updates and notifications | 184 | 43 | 137 | 4 | 0 | 324 | 4645 |
| M08 | Tracking core and AniList | 222 | 48 | 173 | 1 | 0 | 464 | 5109 |
| M09 | Other trackers | 283 | 4 | 278 | 1 | 0 | 505 | 5614 |
| M10 | Backup, restore and migration | 191 | 27 | 161 | 2 | 1 | 369 | 5983 |
| M11 | Long-tail polish (P2) | 469 | 0 | 4 | 463 | 2 | 687 | 6670 |
| M12 | Android-only decisions and replacements (P3) | 303 | 0 | 0 | 0 | 303 | 321 | 6991 |

**M00 Foundation and platform.** Goal: Cargo workspace, Linux paths, logging, crash reports, platform traits, build identity.
- Exit: you can build and launch an empty Kurobayomi window from the workspace
- Exit: a forced panic writes a crash report file under the Linux data directory

**M01 Preferences and databases.** Goal: Preference store with TOML backend, manga and anime SQLite schemas, repositories.
- Exit: you can change a setting and find it again after restart
- Exit: the manga and anime databases are created and migrated on first launch

**M02 Domain, HTTP and source contract.** Goal: Domain models and interactors, HTTP client with rate limits and cookies, source traits, source manager, stubs, frozen ids.
- Exit: domain tests pass headless
- Exit: the HTTP client passes fixture tests with rate limits enabled
- Exit: frozen source id golden vectors pass

**M03 Local reading and playing.** Goal: Local source for manga and anime, image pipeline and cover cache, reader core and viewers, player core and controls on libmpv.
- Exit: you can open a local CBZ or folder and page through it
- Exit: you can play a local video file with mpv and your position is saved

**M04 Daily-driver shell.** Goal: App shell and navigation, library, entry details for manga and anime, updates and history, P0 settings, theme, i18n, desktop integration.
- Exit: you can navigate every P0 route from the sidebar
- Exit: every P0 settings row persists across restart
- Exit: your library shows local entries with categories

**M05 First live source and browse.** Goal: Extension strategy, first built-in live source, extension manager, browse, global search for manga and anime.
- Exit: you can browse, open details, read chapters and watch episodes from the first live source
- Exit: you can search across installed sources

**M06 Downloads.** Goal: Manga and anime downloads, shared queue, download storage layout and notifications.
- Exit: a queued chapter download completes and reads offline
- Exit: a queued episode download completes and plays offline

**M07 Library updates and notifications.** Goal: Library update jobs, scheduling, restrictions, notifications, updater and library export.
- Exit: the library refreshes on schedule and reports new chapters and episodes
- Exit: a desktop notification appears for new items

**M08 Tracking core and AniList.** Goal: Tracker framework, OAuth plumbing, track domain and UI, AniList tracker.
- Exit: you can log in to AniList, bind an entry, and push progress

**M09 Other trackers.** Goal: MyAnimeList, Kitsu, Shikimori, Bangumi, MangaUpdates, Simkl, and enhanced trackers Kavita, Komga, Suwayomi, Jellyfin.
- Exit: each shipped tracker can log in, bind and sync progress

**M10 Backup, restore and migration.** Goal: Backup file format, creators, auto-backup, restore, validators, legacy handling, in-app migration of Aniyomi data.
- Exit: a round trip of a real Aniyomi backup file restores library, categories, history and progress

**M11 Long-tail polish (P2).** Goal: P2 features and polish across all areas.
- Exit: every P2 task is either done or explicitly deferred with a recorded reason

**M12 Android-only decisions and replacements (P3).** Goal: P3 tasks: Android-only obsolete features, their Linux replacement or drop decision, and the Windows spike.
- Exit: every P3 task states its replacement or drop decision and the user-visible loss
<!-- END GENERATED: assign_milestones.py -->

Critical path (explicit task dependencies, longest chain): 40 days.

## Order and rules

1. A task never sits in an earlier milestone than an explicit `T-` task it depends on. Cycles among explicit dependencies are placed in one milestone together.
2. `area:<id>` dependencies are enforced at the home milestone of that area (`area_home` in `rules.json`), not per task. Task-level area edges merged most of the plan into one cycle (2938 tasks), so they were not used. `verify.py` checks only `T-` edges.
3. A task's milestone is never earlier than its area home. A task that an urgent task depends on is pulled to the area home of its dependent's priority, so a P2 helper of a P0 task is not parked in the P2 polish milestone.
4. P2 tasks start no earlier than M11. P3 tasks start no earlier than M12.

## Milestones

### M00 Foundation and platform
- Goal and exit criteria: `plans/milestones/M00-foundation-platform.md`.
- Demo: `cargo build --workspace`; `cargo test --workspace`; launch the app and show the empty library window; trigger the crash hook and open the report file.

### M01 Preferences and databases
- Goal and exit criteria: `plans/milestones/M01-data-foundations.md`.
- Demo: change a preference in the Settings screen, quit, relaunch, and confirm it persisted; delete the database file and watch both schemas recreate.

### M02 Domain, HTTP and source contract
- Goal and exit criteria: `plans/milestones/M02-domain-and-source-contract.md`.
- Demo: run the headless domain test suite; run the HTTP fixture suite with rate limits on; run the frozen id golden vectors.

### M03 Local reading and playing  (first daily-use milestone)
- Goal and exit criteria: `plans/milestones/M03-local-reading-and-playing.md`.
- Demo: add a folder of CBZ files as a local manga entry, open a chapter, page through it in each viewer; add a local video as an anime entry, play it, quit at 1:30, reopen and resume at 1:30.

### M04 Daily-driver shell
- Goal and exit criteria: `plans/milestones/M04-daily-driver-shell.md`.
- Demo: navigate Library, Updates, History, Browse, More from the sidebar; add a category, filter and sort the library; change every P0 setting and confirm it persists; switch the theme to a dark GTK theme and confirm the app follows it.

### M05 First live source and browse
- Goal and exit criteria: `plans/milestones/M05-first-live-source-and-browse.md`.
- Demo: install the first live source from the extension manager; browse it; open a title; read a chapter and watch an episode; run a global search across installed sources.

### M06 Downloads
- Goal and exit criteria: `plans/milestones/M06-downloads.md`.
- Demo: queue five chapters and two episodes; watch progress and the queue; disconnect the network and read or play the downloaded items.

### M07 Library updates and notifications
- Goal and exit criteria: `plans/milestones/M07-library-updates-and-notifications.md`.
- Demo: run a manual library update; confirm new items appear in Updates and a desktop notification is sent; export the library.

### M08 Tracking core and AniList
- Goal and exit criteria: `plans/milestones/M08-tracking-core-and-anilist.md`.
- Demo: log in to AniList in the browser, bind a title, mark an episode watched and see progress reach AniList.

### M09 Other trackers
- Goal and exit criteria: `plans/milestones/M09-other-trackers.md`.
- Demo: repeat the M08 demo for each shipped tracker.

### M10 Backup, restore and migration
- Goal and exit criteria: `plans/milestones/M10-backup-restore-and-migration.md`.
- Demo: create a backup, delete the library, restore the backup and confirm the library, categories, history and progress return; restore a real Aniyomi backup file.

### M11 Long-tail polish (P2)
- Goal and exit criteria: `plans/milestones/M11-long-tail-polish.md`.
- Demo: each P2 task is ticked done or deferred with its recorded reason.

### M12 Android-only decisions and replacements (P3)
- Goal and exit criteria: `plans/milestones/M12-android-only-decisions.md`.
- Demo: each P3 task names its Linux replacement or its drop decision and the user-visible loss.

## Risks

- Scope size. The first usable slice (M00 to M05) sums to 3762 task-days, about 15 working years for one developer at 250 days a year, before any parallelism. The whole plan is 6991 task-days. Cut scope inside M03 to M05 before starting M06 if the calendar matters.
- P0 tags in parity milestones. About 196 tasks tagged P0 sit in M06 to M10 because their area is a parity area (downloads, library updates, tracking, backup). Either retag them to P1 or accept that the daily driver needs them. Decide before M06.
- Area dependencies are enforced at area-home granularity only. A task can sit in a milestone earlier than some task of the area it names. Revisit if this bites.
- Explicit dependency cycles (27 groups, largest 10 tasks) force each group into one milestone. Break the cycles in the task files if they grow.
- Source and extension strategy (area:20) decides how live sources exist at all. Milestones M02 and M05 depend on it.
- Platform risks are in `plans/RISKS-AND-DECISIONS.md`.

## Decisions for the user

1. Accept that the daily-driver slice is M00 to M05, not M00 to M03. M03 is reading and playing local files only.
2. Confirm the P0 retagging question above.
3. Confirm that P3 tasks (Android widgets, Windows port spike) are placed in M12 and not dropped.
