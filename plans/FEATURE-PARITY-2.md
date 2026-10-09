# Feature parity matrix 2/3: playback, sources, data services

Synthesis document for Kurobayomi. It maps every user-visible capability of Aniyomi in these areas to one of three outcomes: planned port or rewrite in Rust, named Linux-native replacement, or explicit drop with the user loss stated.

Areas covered: player (`area:45-player-core`, `area:46-player-controls-ui`, `area:48-settings-player-and-scripts`, `area:47b-settings-widgets-search-more` where a row points at it), downloads (`area:23-downloads-manga`, `area:24-downloads-anime`, `area:25-downloads-shared`), tracking (`area:27-tracking-core`, `area:28-tracker-anilist`, `area:29-tracker-myanimelist`, `area:30-tracker-kitsu`, `area:31-tracker-shikimori-bangumi`, `area:32-tracker-mangaupdates-simkl`, `area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin`), backup and restore (`area:21-backup-format-and-create`, `area:22-backup-restore`), library update jobs (`area:26-library-update-jobs`), sources and extensions (`area:17-source-app-layer`, `area:18-extensions-manga`, `area:19-extensions-anime`, `area:20-extension-strategy-redesign`), and the local source (`area:14-source-local-and-metadata`).

Repo root for every Kotlin path: `/home/blackbox/aniyomi-src`. Repo-relative paths are given from the repository root, for example `app/src/main/java/eu/kanade/tachiyomi/App.kt`. Kurobayomi root: `/home/blackbox/kurobayomi`. Plans root: `/home/blackbox/kurobayomi/plans/`.

---

## 0. How to read this document

### 0.1 Row format

Every matrix row has these columns:

| Column | Meaning |
|---|---|
| Row | Stable row id, prefix by section (PB = playback core, PC = player controls UI, PS = player settings and scripts, DM = manga downloads, DA = anime downloads, DS = shared downloads, LU = library update jobs, TK = tracking core, AL/MU/KI/SK/MD/SI/EN = trackers, BK = backup create, BR = backup restore, SR = source registry and stubs, EM = manga extensions, EA = anime extensions, ES = extension strategy, LS = local source). |
| Feature | What the user sees or can do. |
| Aniyomi evidence | Repo-relative Kotlin path (from `/home/blackbox/aniyomi-src`), with `:line` where the owning overview gives one. |
| Pri | P0, P1, P2 or P3 as defined in the project brief. |
| Disposition | `PORT` (same behaviour, Rust), `REWRITE` (same user behaviour, new internals), `REPLACE:<native>` (named Linux equivalent), `DROP` (with the user loss in section 4), `DEFER` (planned, not in the current phase). |
| Tasks | Task ids from `plans/TASKS.csv`. Ranges mean the listed prefix and the tasks named in the owning area. |

### 0.2 Priority of a row

The `Pri` column is the priority of the user-visible behaviour in this matrix. The priority of each task is in `plans/TASKS.csv`, and a row can cite tasks with other priorities (for example a P1 row can cite a P0 task that it needs). Where the two disagree in a way that changes scheduling, section 2 lists it.

### 0.3 Labels

- OBSERVED: what the Kotlin does. Stated as fact in the owning area document and spot-checked where marked.
- Proposal: what the Rust port should do. Every row whose Rust behaviour differs from Kotlin says so in the Disposition column.

### 0.4 Priorities (project brief)

- P0: needed so Kurobayomi is usable daily for reading manga and watching anime from one working source.
- P1: parity features most Aniyomi users rely on.
- P2: long tail and polish.
- P3: Android-only or obsolete, with a stated replacement or drop.

### 0.5 Task counts for the areas in this document

Computed from `plans/TASKS.csv` (3801 rows in total). Each count is priority per area, in P0, P1, P2, P3 order.

| Area | Tasks | P0 | P1 | P2 | P3 | Task id prefix |
|---|---|---|---|---|---|---|
| 14-source-local-and-metadata | 58 | 25 | 23 | 8 | 2 | T-14a |
| 17-source-app-layer | 50 | 33 | 10 | 3 | 4 | T-17a |
| 18-extensions-manga | 54 | 27 | 16 | 7 | 4 | T-18a |
| 19-extensions-anime | 75 | 19 | 33 | 9 | 14 | T-19a |
| 20-extension-strategy-redesign | 28 | 14 | 10 | 2 | 2 | T-20a |
| 21-backup-format-and-create | 44 | 0 | 40 | 2 | 2 | T-21a |
| 22-backup-restore | 53 | 0 | 40 | 9 | 4 | T-22a |
| 23-downloads-manga | 45 | 23 | 16 | 4 | 2 | T-23a |
| 24-downloads-anime | 74 | 0 | 55 | 14 | 5 | T-24a |
| 25-downloads-shared | 35 | 1 | 27 | 6 | 1 | T-25a |
| 26-library-update-jobs | 104 | 39 | 43 | 15 | 7 | T-26a |
| 27-tracking-core | 123 | 0 | 112 | 8 | 3 | T-27a, T-27b |
| 28-tracker-anilist | 58 | 35 | 16 | 6 | 1 | T-28a |
| 29-tracker-myanimelist | 59 | 0 | 48 | 8 | 3 | T-29a |
| 30-tracker-kitsu | 35 | 0 | 28 | 5 | 2 | T-30a |
| 31-tracker-shikimori-bangumi | 49 | 0 | 45 | 3 | 1 | T-31a |
| 32-tracker-mangaupdates-simkl | 55 | 0 | 50 | 4 | 1 | T-32a |
| 33-tracker-enhanced-kavita-komga-suwayomi-jellyfin | 81 | 0 | 70 | 7 | 4 | T-33a |
| 45-player-core | 164 | 73 | 60 | 20 | 11 | T-45a, T-45b |
| 46-player-controls-ui | 96 | 39 | 37 | 15 | 5 | T-46a, T-46b |
| 47b-settings-widgets-search-more | 49 | 32 | 10 | 5 | 2 | T-47ba |
| 48-settings-player-and-scripts | 58 | 20 | 25 | 7 | 6 | T-48a |

Task ids are not contiguous in every area (for example area 33 has 81 tasks with ids up to `T-33a-111`). The authoritative list is `plans/TASKS.csv`. Export with `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`.

---

## 1. Summary of the matrix

| Group | Matrix rows | Section |
|---|---|---|
| Playback core (player) | 43 | 3.1 |
| Player controls UI | 42 | 3.2 |
| Player settings, mpv, scripts, custom buttons, torrent settings | 30 | 3.3 |
| Manga downloads | 31 | 3.4 |
| Anime downloads | 13 | 3.5 |
| Shared download queue rules | 10 | 3.6 |
| Library update jobs | 28 | 3.7 |
| Source registry and stubs | 10 | 3.8 |
| Manga extensions | 17 | 3.9 |
| Anime extensions | 20 | 3.10 |
| Extension strategy | 8 | 3.11 |
| Local source | 10 | 3.12 |
| Tracking core | 16 | 3.13 |
| AniList | 25 | 3.14 |
| MyAnimeList | 6 | 3.15 |
| Kitsu | 4 | 3.16 |
| Shikimori and Bangumi | 9 | 3.17 |
| MangaUpdates and Simkl | 9 | 3.18 |
| Enhanced trackers | 8 | 3.19 |
| Backup create | 22 | 3.20 |
| Backup restore | 32 | 3.21 |
| **Total** | **393** | |

Each row is one user-visible behaviour, one capability or one decision. Rows marked DROP or DEFER are also listed in section 4 with the user loss. Each row appears in exactly one section.

---

## 2. Cross-area conflicts that block implementation

These are contradictions between the area documents, found while writing this synthesis. Each needs one decision before the affected tasks start. The recommendation is the one the owning overview gives, unless the item says otherwise.

| # | Topic | Conflict | Recommendation | Affected tasks |
|---|---|---|---|---|
| X-01 | Torrent engine | `area:45-player-core` D-2 recommends `librqbit` in process, with TorrServer as fallback. `area:48-settings-player-and-scripts` DD-6 says TorrServer runs as a child process from a configured path, and that the child is stopped on switch-off and on exit. | Pick one. The 48 document models a process (matches the user settings page). The 45 document models an in-process crate. Decide with the user before `T-45a-072` and `T-48a` torrent tasks start. | T-45a-072 to T-45a-079, torrent settings tasks in T-48a |
| X-02 | Local HTTP server | `area:45-player-core` keeps `LocalServer` on `127.0.0.1` with an ephemeral port (T-45a-069 to T-45a-071). `area:20-extension-strategy-redesign` Q10 and item 2 of section 4.1 drop `createHttpServer` and recommend dropping the local proxy. | Keep only if a source the user needs depends on it. Ask the user; the 20 document says no current source requires it. | T-45a-069 to T-45a-071 |
| X-03 | WASM host priority | `area:20-extension-strategy-redesign` T-20a-001 is P0 (L). Its own topic document puts WASM in phase 2. Its Q3 recommends P1. | Built-in sources in P0. WASM loader P1, split from the built-in loader. | T-20a-001, T-18a-006 |
| X-04 | Extension ABI version | `area:19-extensions-anime` Q4 accepts `"1.0"`. `area:18-extensions-manga` section 7.3 uses `"abi": "1.4"`. `area:20-extension-strategy-redesign` names the WIT world `kurobayomi:source@1.0.0`. | `1.0`, matching the WIT package version. Change the manga example. | T-19a-002, T-18a-002, T-20a-001 |
| X-05 | Package manifest format | `area:18-extensions-manga` and `area:19-extensions-anime` use `.kuroext` zip with `manifest.json`. `area:20-extension-strategy-redesign` D8 uses a directory package with `manifest.toml`. | `.kuroext` zip with `manifest.json`. Rewrite T-20a-001 to T-20a-100 D7 and D8. | T-18a-004, T-19a-004, T-20a-004 |
| X-06 | Signer record | `area:19` Q5 writes a sidecar `<pkg>.signer`. `area:18` section 7.3 rewrites `signerPub` into the installed `manifest.json`. | Sidecar. Rewriting the manifest changes the bytes that were verified. | T-19a-005, T-18a-047 |
| X-07 | Maximum `source.wasm` size | `area:18` pitfall 6 caps at 16 MiB. `area:18` section 7.3 caps at 32 MiB. | 32 MiB for both kinds. | T-18a-006 |
| X-08 | Unsigned versus untrusted | `area:19` data 8.2 calls missing signature data `Unsigned`. `area:18` section 16.2 calls the same case `Untrusted`. | `Unsigned` for missing or malformed signature data. `Untrusted` only for a valid signature from a signer not yet trusted. | T-19a-009, T-18a-012 |
| X-09 | Duplicate package names across repos | Kotlin `associateBy` (last wins) for the available map and `find` (first wins) for reconciliation. | First repo in table order wins for both (`area:18` D4). | T-18a-014, T-18a-015 |
| X-10 | Torrent priority in P0 chain | `area:45-player-core` says all torrent tasks are P2. `area:48` lists torrent settings with P1 and P2 tasks. | Torrent is P2 for daily use. The settings page is P2. | T-48a-034 |
| X-11 | Download concurrency default | `area:23-downloads-manga` says `download_slots` default is 1 (`DownloadPreferences.kt:69`). Its D-DL-02 proposes default 5 with a new key name `downloads.max_parallel_chapters`. | Keep default 1 for the renamed key until the user confirms. Proposal only. | T-23a-037 |
| X-12 | Error notification importance | `area:23` T-23a-019 and rust design section 5.3 say errors stay at low importance, as in Kotlin. `area:23` D-DL-03 and defect D-27 recommend default importance for errors. | Default importance for errors and warnings, low for progress and paused. | T-23a-019 |
| X-13 | Battery threshold for automatic jobs | `area:21-backup-format-and-create` proposes 15 percent on battery. `area:26-library-update-jobs` proposes 20. | One shared constant, 20 percent (`area:21` Q3). | T-21a-026, T-26a-104 |
| X-14 | Priority inversions in the download chain | `area:23` T-23a-022 (finalisation) and T-23a-021 (ComicInfo) are P1, but P0 test T-23a-036 depends on them. T-23a-026 (cancel API, P1) is a dependency of P0 T-23a-027. | Promote T-23a-021, T-23a-022 and T-23a-026 to P0. | T-23a-021, T-23a-022, T-23a-026, T-23a-036 |
| X-15 | Anime download folder rules | `area:24-downloads-anime` D-6 proposes importing existing Aniyomi download folders (T-24a-057, P2). `area:25` says the index reads the same layout. | Keep the layout reader. Import is P2 and asks the user. | T-24a-005, T-24a-006, T-24a-057 |
| X-16 | Library job restriction ids | `area:26` D8 applies `manga_outside_release_period` to manual passes too. Kotlin does the same. The help text must say so. | Parity. Help text states it. | T-26a-003 |
| X-17 | Tracker secret fallback | `area:27-tracking-core` D-G part 1 writes `secrets.json` mode 0600 when no Secret Service runs. Part 2 `T-27b-027` reports "Secret storage is unavailable" and stores nothing. | Part 2. A plaintext token file is not acceptable. | T-27b-027 |
| X-18 | Tracker trait name | `area:27` part 1 uses `Tracker`, `AnimeTracker`, `MangaTracker`. Part 2 uses `TrackerService`. | `Tracker` (area 27 D-J). | T-27a, T-27b |
| X-19 | Tracking date crate | `area:27` D-H: part 1 suggests `time`, part 2 uses `chrono`. | `chrono`. | T-27a, T-27b |
| X-20 | Tracking PKCE randomness | `area:27` D-I: `rand` plus `sha2` plus `base64` (part 1) versus `getrandom` 0.2 plus `base64` (part 2). | `getrandom` 0.2, `base64` URL-safe no-pad, `sha2` for S256. 50 random bytes, as `PkceUtil.kt:9`. | T-27a, T-27b |
| X-21 | Tracking priority | `area:27` inventory shows six contract files as P0. Task files show P1. | P1 for every file and task. | T-27a, T-27b |
| X-22 | Player sheet container | `area:46` D-19 uses modal windows with scrim. `area:46` part 2 `T-46b-001` uses an in-window `SheetHost` overlay. | In-window overlay (`SheetHost`). Fold T-46a-029 into T-46b-001. | T-46a-029, T-46b-001 |
| X-23 | Player controls priority inversions | Several P0 tasks depend on P1 or P2 tasks: T-46a-004 to T-46a-005, T-46a-006 to T-46a-041, T-46a-006 to T-46a-053, T-46a-009 to T-46a-046, T-46a-009 to T-46a-050, T-46a-009 to T-46a-051, T-46a-008 to T-46a-052, T-46a-016 to T-46a-017, T-46a-029 to T-46a-041, T-46b-045 to T-46b-008. | Promote the dependencies or demote the dependents. Decide per edge. | listed |
| X-24 | Player controls cycles | `area:46` lists dependency cycles T-46a-006 and T-46a-053, and T-46a-009 and T-46a-050. | Drop the parent-to-child edge in each pair. | T-46a-006, T-46a-053, T-46a-009, T-46a-050 |
| X-25 | Hoster retry and resume defaults | `area:45` D-6 (no retry on ERROR) and D-8 (`pref_preserve_watching_position` default false). Both need user confirmation. | Kotlin defaults until the user answers. | T-45b-014, T-45a-012 |
| X-26 | Subtitle Off entry | `area:45` D-7 recommends an explicit Off entry (`sid=no`). Not in Kotlin. | Add it in the settings pass. | T-45b-016, T-45a-015 |
| X-27 | Lua bridge priority | `area:48-settings-player-and-scripts` makes the bridge P0 (`T-48a-040`, L). `area:45-player-core` makes the user-data dispatcher P1 (`T-45b-035`) and custom-button Lua P2 (`T-45a-021`). | P0 bridge, P2 custom buttons. | T-48a-040, T-45b-035, T-45a-021 |
| X-28 | Long-press on the video | `T-46a-020` says long press opens the screenshot sheet. `area:46-player-controls-ui` section 7 item 4 says long press starts speed boost. | Speed boost (the DoD item is the user-facing contract). Confirm with the user (U-15). | T-46a-020, T-46a-039 |
| X-29 | Hoster click while Loading | `area:46` E54 recommends ignoring clicks on a hoster in Loading. | Ignore. Decision, not conflict. | T-46b-014 |
| X-30 | Subtitle delay field during tap-to-sync | `area:46` E11 recommends disabling the field while a run is active. | Disable. Decision, not conflict. | T-46b-030 |
| X-31 | Secondary subtitle delay on "Set as default" | `area:46` E5 recommends also persisting `pref_subtitles_secondary_delay`. | Persist. Decision, not conflict. | T-46b-038 |
| X-32 | New-episode auto-download default | `area:26` D5 wires anime new-episode downloads with `autoStart = false`, default `download_new_episode` false. | As stated. Decision, not conflict. | T-26a-024 |
| X-33 | Backup preference tags | `area:21` D10 and Q1: the nested tags of the six `PreferenceValue` subclasses are not in the Kotlin text. Real backup files and the debug schema text are needed. | Ask the user for fixtures (U-19). Do not guess tags. | T-21a-034, T-21a-006, T-21a-020, T-21a-021 |
| X-34 | Restore metadata when the backup is newer | `area:22` section 4.2 item 1: Kotlin keeps DB metadata when the backup is newer and overwrites when older or equal. | Keep parity by default; user call (U-20). | T-22a-006 |

Items X-01, X-02, X-03, X-05, X-11, X-12, X-13, X-22, X-28, X-33 and X-34 change user-visible behaviour, scope or data. Each needs an answer from the user, listed in section 5.

---

## 3. Feature rows

Each subsection lists the rows for one area. The disposition uses the codes in section 0.1.

### 3.1 Playback core (`area:45-player-core`)

Scope of the Kotlin code: `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt` (1413 lines), `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt` (290 lines), `app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt` (610 lines), `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` (2177 lines), `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt`, `app/src/main/assets/aniyomi.lua`. Engine: libmpv through the `BaseMPVView` class from the external `com.github.aniyomiorg:aniyomi-mpv-lib:1.18.n` library (not in this repository).

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| PB-01 | Play one episode in an mpv surface, with video and audio | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt` | P0 | REWRITE: libmpv through `libmpv2`, render API into `gtk::GLArea` (`src/player/mpv.rs`). Single handle per window (area 45 D-3). | T-45a-001, T-45a-002, T-45a-003 |
| PB-02 | Startup mpv option table (decoder, cache, TLS, log level) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt` `initOptions` | P0 | PORT: pure function `options.rs` from preferences to `Vec<MpvSetting>`. | T-45a-003, T-45a-004, T-45a-005 |
| PB-03 | Property observation (34 observed properties) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt` `observedProps` | P0 | PORT: observer table in `observer.rs`. Whole-second channel for the seek bar. | T-45a-006, T-45a-007, T-45b-073 |
| PB-04 | Playback error reporting | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerObserver.kt` | P0 | PORT: `PlayerError { kind, message_key }`. | T-45a-009 |
| PB-05 | Source URI resolution (file, content, data, protocol strings) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerUtils.kt` | P0 | REWRITE: pure `source_url.rs`. Android `content://` is dropped; plain paths and URLs only. | T-45a-010 |
| PB-06 | Load episode with start position, HTTP headers, media title | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt` `setVideo`, `fileLoaded` | P0 | PORT: `loadfile` with start, header passthrough, title. | T-45a-011, T-45a-012, T-45a-014, T-45a-018, T-45a-084 |
| PB-07 | Resume position rule | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` (episode position in saved state) | P0 | PORT. Resume only when `pref_preserve_watching_position` is true (area 45 D-8, default false). | T-45a-012, T-45b-026 |
| PB-08 | Hoster list per episode kind (downloaded, HTTP source, local) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt` | P0 | PORT: `loader/episode.rs`. | T-45b-007, T-45b-008, T-45b-064 |
| PB-09 | Hoster selection and video (quality) list, lazy URL resolution, sorting | `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt` `selectBestVideo`, `getBestVideo`, `getResolvedVideo` | P0 | PORT: `HosterState` enum (Idle, Loading, Error, Ready). Concurrency per `load_hosters`. | T-45b-009, T-45b-010, T-45b-012, T-45b-013, T-45b-014, T-45b-015 |
| PB-10 | Hoster in Error: retry | `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt` | P1 | Kotlin: no retry. Area 45 D-6 adds `hoster_retry` flag, off by default. | T-45b-014 |
| PB-11 | Audio track selection (default by language, whitelist, blacklist) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/TrackSelect.kt` | P0 | PORT. Display names from `icu_displaynames` (REPLACE for `Locale.getDisplayName`, area 45 D-13). | T-45b-016, T-45b-017 |
| PB-12 | Subtitle track selection and Off entry | `app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/TrackSelect.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` | P0 | PORT. Explicit Off (`sid=no`) added (X-26). | T-45b-016, T-45b-017 |
| PB-13 | Chapters from the source, current chapter, chapter seek | `app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/ChapterUtils.kt` | P0 | PORT. | T-45b-018 |
| PB-14 | AniSkip skip-intro button and timestamps | `app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/AniSkipApi.kt` | P1 | PORT: `reqwest` client from `area:12-network-stack`. MAL id mapping. Off by default (area 45 D-10). | T-45b-019, T-45b-020, T-45b-021, T-45b-022 (`p2-topic-skip-intro-and-chapters.md`) |
| PB-15 | Skip-intro state machine (show during intro, skip) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` | P1 | PORT: `SkipIntroMachine`. | T-45b-022 |
| PB-16 | Progress save on a one-second tick, coalesced | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` `onSecondReached` | P0 | REWRITE: coalesced to every 5 s, with flush on pause and close (area 45 D-9). | T-45b-026, T-45a-040 |
| PB-17 | Completion threshold marks episode seen | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` | P0 | PORT. Threshold per `p2-topic-progress-and-completion.md` section 4. | T-45a-064, T-45b-027 |
| PB-18 | Duplicate-episode marking after completion (preference) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` | P1 | PORT. Uses `MARK_DUPLICATE_EPISODE_SEEN_EXISTING` from `area:08-domain-library-category-history-updates`. | T-45b-027 |
| PB-19 | History write with incognito gate | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt` | P0 | PORT. Incognito matrix in area 45 `p2-topic-progress-and-completion.md` section 4: with incognito on and no tracker, neither progress nor history is written; with a tracker, progress is written and history is not. | T-45a-063, T-45b-028 |
| PB-20 | Process-restart snapshot (episode, position, quality index) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` | P0 | PORT: `session_file.rs`. Restores after a crash. | T-45b-030 |
| PB-21 | Next and previous episode, autoplay at end of file | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` | P0 | PORT. Messages from area 45 T-45a-037. | T-45a-036, T-45a-037, T-45b-006 |
| PB-22 | Download-ahead of next episodes | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` | P1 | PORT with the shared queue (`area:25-downloads-shared`). | T-45b-029, plus the queue tasks in `area:24-downloads-anime` |
| PB-23 | Transport: play, pause, seek events | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt` | P0 | PORT. | T-45a-019 |
| PB-24 | Lifecycle: stop mpv, save progress, release handle on close | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt` `onPause`, `onStop`, `onDestroy` | P0 | PORT: `isExiting` gate and shutdown order, without the Kotlin flag defect. | T-45a-039, T-45a-040 |
| PB-25 | Keyboard to mpv key names and window key bindings | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt` `onKey` | P0 | REWRITE: `keys.rs` maps gdk keyval and modifiers to mpv key strings. New bindings are in area 46 T-46a-043. | T-45a-024, T-45a-025 |
| PB-26 | Media keys (play, pause, next, previous) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt` media session | P1 | REPLACE: MPRIS2 on the session bus (`mpris` crate or `zbus`) instead of Android MediaSession. | T-45a-043, T-45b-057, T-48a-024 |
| PB-27 | Audio focus | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt` | P3 | DROP. KDE and PipeWire handle the audio stream. No UI remains. | T-45a-042 |
| PB-28 | Picture-in-picture | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt` | P3 | REPLACE with a mini-player window (area 46 D-16, P2). Transport only. | T-45a-047 (P3), T-45a-048, T-45b-050 |
| PB-29 | Orientation lock | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt` `PlayerOrientation` | P3 | DROP. No device rotation on Linux. | T-45a-035, T-45b-058, T-48a-019 |
| PB-30 | Keep screen awake during playback | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt` | P1 | REPLACE: `org.freedesktop.ScreenSaver` `Inhibit` over D-Bus while playing, released on pause. | T-45a-041 |
| PB-31 | Sleep timer (fires in player) | Sleep timer in `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt`, action in `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` | P1 | PORT. Action when the timer fires is defined in area 45. Timer values: seconds = hour * 3600 + minute * 60. | T-46b-018 |
| PB-32 | Screenshot to the configured folder, with notification | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` `takeScreenshot` | P1 | PORT. Notification pattern from `area:43-reader-core`. | T-45a-049, T-45b-039, T-46b-021 |
| PB-33 | Share screenshot | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` | P2 | REPLACE: `gio` file share through the desktop portal (`org.freedesktop.portal.FileChooser` / `Email` or copy to clipboard). | T-45a-050 |
| PB-34 | Set as art (cover, background, thumbnail) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` `setAsArt`, `ArtType`, `SetAsArt` | P2 | PORT using `area:34-images-covers-cache` targets. Confirm text is one string for every art type (area 46 E86). | T-45a-051, T-46b-021 |
| PB-35 | External player hand-off (mpv, vlc, celluloid, custom, desktop default) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt` (610 lines) | P1 | REPLACE: `std::process::Command` argv builder (pure) plus `gio` default-app launch for the empty preset. Preset table in section 3.1.1. | T-45a-055 to T-45a-062, T-45b-049 |
| PB-36 | External player resume position report back | `app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt` | P1 | REWRITE: mpv `--input-ipc-server` socket poll for the external mpv preset. Other presets do not report back. | T-45a-057, T-45a-058, T-45a-061 |
| PB-37 | Source local HTTP server for `createHttpServer` sources | `app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt` (NanoHTTPD) | P1 | REPLACE: `LocalServer` on `127.0.0.1` with an ephemeral port. Conflict X-02: may be dropped. | T-45a-069 to T-45a-071 |
| PB-38 | Torrent playback through TorrServer | `app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt` (`xyz.secozzi.torrserver`, Go library) | P2 | DEFER. Engine choice in conflict X-01. | T-45a-072 to T-45a-079 |
| PB-39 | Lua bridge: 28 `aniyomi.*` functions, `user-data/aniyomi/*` properties | `app/src/main/assets/aniyomi.lua` | P0 | REWRITE: `bridge.rs` parses the protocol. Dispatch clears the property after read (area 45 D-5). | T-45b-035, T-45b-071, T-48a-040, T-45a-021 |
| PB-40 | Stats overlay (statistics page) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt`, controls | P2 | PORT. Toggle truth table in `stats_page.rs`. | T-45a-053 |
| PB-41 | Mini-player | not in Kotlin (PiP replacement) | P2 | DEFER. Transport controls only (area 45 D-11). | T-45a-048, T-45b-050, T-46a-046 |
| PB-42 | MPRIS metadata (title, art, position) | not in Kotlin | P1 | REPLACE: MPRIS2 `Metadata`, `PlaybackStatus`, `Position`. | T-45a-043 |
| PB-43 | Playback speed presets and long-press speed boost | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt`, controls | P0 | PORT. Boost released on pointer-up, speed not restored (area 46 D-4). | T-45b-038, T-45a-033, T-46a-039 |

#### 3.1.1 External player preset table (Linux)

Source: area 45 `p1-topic-external-players.md` section 3. Ship these presets on Linux, per area 45 D-14:

| Preference value | Program | Argv pattern | Position | Subtitle | Headers |
|---|---|---|---|---|---|
| `""` (empty) | desktop default for the URI | `gio open <file-or-url>` through `gio::AppInfo::launch_default_for_uri` | none | none | none |
| `mpv` | `mpv` | `mpv --title=<title> --start=<seconds> --http-header-fields=<h> --sub-file=<url> --input-ipc-server=<sock> <url>` | `--start=<s>` | `--sub-file=<url>` | `--http-header-fields` |
| `vlc` | `vlc` | `vlc --meta-title=<title> --start-time=<s> --http-referrer=<referer> --sub-file=<url> <url>` | `--start-time=<s>` | `--sub-file=<url>` | `User-Agent` and `Referer` only |
| `celluloid` | `celluloid` | `celluloid --mpv-options=start=<s>,sub-file=<url>,http-header-fields=<h> <url>` | `start=<s>` in mpv options | `--mpv-options=sub-file=<url>` | `--mpv-options=http-header-fields=<h>` |
| `custom` | user template | user-defined with placeholders `{url}`, `{title}`, `{position}`, `{subtitle}`, `{headers}` | `{position}` in seconds | `{subtitle}` | `{headers}` as `Key: value` pairs joined by commas |

Rules: positions are seconds, `f64` with three decimals, internal value is milliseconds. Each title is one argv element. No shell. A missing binary gives `Player not found: <program>` and nothing is spawned (EC-85).

Dropped external players (Android packages, no Linux build): `com.mxtech.videoplayer`, `com.mxtech.videoplayer.ad`, `com.mxtech.videoplayer.pro` (MX Player), Just Player, Next Player, X Player, the mpv remote (`com.husudosu.mpvremote`) and Web Video Caster. The Android packages `is.xyz.mpv` and `live.mehiz.mpvkt` are replaced by the `mpv` preset, and `org.videolan.vlc` by the `vlc` preset.

### 3.2 Player controls UI (`area:46-player-controls-ui`)

Scope: 46 Kotlin files, 7781 LOC (part 1 is 32 files, 4717 LOC; part 2 is 14 files, 3064 LOC). Root: `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/`. Gesture surface: `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt`. Root overlay: `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt`. Sheets: `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/` (directory). Panels: `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/` (directory). Dialogs: `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/dialogs/` (directory). Leaf widgets: `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/` (directory).

Timings and constants quoted from the owning overview and part documents:
- Overlay auto-hide after `playerTimeToDisappear`, default 4000 ms, only while playing and not seeking.
- Double-tap accumulator resets to 0 after 800 ms with no tap; seek text hides 100 ms after the reset.
- Device volume uses 15 steps (`max_steps`), about 67 px of vertical drag per step.
- Repeating button schedule: first repeat at 750 ms, decay factor 0.25, floor 5 ms.
- Sleep timer: seconds = hour * 3600 + minute * 60. Remaining time shown as `MM:SS` or `H:MM:SS`. Setting 00:00 equals cancel.
- Alpha constants: `DISABLED_ALPHA = .38f`, `SECONDARY_ALPHA = .78f` from `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt` (owner area:36).

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| PC-01 | Tap the video to show or hide controls; auto-hide while playing | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt` | P0 | REWRITE: `gtk::Overlay` with visibility state machine (`src/player/controls/overlay.rs`). Pure rules in `logic/model.rs`. | T-46a-001, T-46a-014 |
| PC-02 | Lock button hides every control except the lock; tap the lock to show | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt` | P0 | REWRITE | T-46a-002 |
| PC-03 | Top-left back button closes the player; title block shows anime title and episode name and opens the episode list | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/TopLeftPlayerControls.kt` | P0 | REWRITE | T-46a-003, T-46a-024 |
| PC-04 | Top-right row: subtitle, audio, quality (online sources only), more | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/TopRightPlayerControls.kt` | P0 | REWRITE. Quality is hidden for local sources. Long-press on subtitle opens subtitle settings; long-press on audio opens audio delay (46 overview section 7 item 8). | T-46a-004, T-46a-005 |
| PC-05 | Auto-play toggle persists its value | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/AutoPlaySwitch.kt` | P1 | PORT. Stored in the typed settings store (area:02). | T-46a-005 |
| PC-06 | Centre transport: previous, play/pause, next. Spinner while loading. Icon cross-fades (not morph) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/MiddlePlayerControls.kt` | P0 | REWRITE: cross-fade is decision D-7 in area 46. | T-46a-006, T-46a-007 |
| PC-07 | Bottom-left row: lock, speed cycle on tap, speed sheet on long-press, chapter pill when chapters exist. Rotate removed on Linux (D-15) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomLeftPlayerControls.kt` | P0 | REWRITE. Rotate is DROP (section 4). | T-46a-008, T-46a-052 |
| PC-08 | Bottom-right row: skip-intro button when intro chapter is current, else the custom primary button when configured; PiP gate; aspect cycle | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomRightPlayerControls.kt` | P0 | REWRITE. PiP becomes the mini-player (PB-28). | T-46a-009, T-46a-050 |
| PC-09 | Aspect button cycles crop, fit and stretch | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomRightPlayerControls.kt` | P0 | PORT. | T-45b-033, T-46a-009 |
| PC-10 | Seekbar with chapter ticks, read-ahead region, elapsed time left, duration or remaining right (toggle) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/SeekBar.kt` | P0 | REPLACE: custom GObject `SeekBar`, snapshot-drawn. Replaces the `dev.vivvvek.seeker` 1.2.2 library. | T-46a-010, T-46a-011 |
| PC-11 | Seek preview thumbnail above seekbar with chapter label | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/ThumbnailPreview.kt` | P1 | PORT: tile LRU of 3, 2 s throttle, crop (`src/player/controls/thumbnail.rs`). Source tiles from `getImageTile`. | T-46a-013 |
| PC-12 | Horizontal drag on the video seeks; seek text shows target time; release seeks | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt` | P0 | REWRITE: `GestureDrag` on overlay child. | T-46a-018 |
| PC-13 | Double-tap left or right third seeks by the preference amount; accumulated amount shown | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/DoubleTapSeekTriangles.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt` | P0 | REWRITE: `GestureClick` with n_press 2. Triangle animation drawn in cairo (`src/player/controls/oval.rs`). | T-46a-015, T-46a-016 |
| PC-14 | Long-press on the video starts speed boost; release on pointer-up does not restore speed | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt` | P0 | REWRITE: `GestureLongPress`. Area 46 D-4. | T-46a-039 |
| PC-15 | Long-press action on the video: task T-46a-020 says it opens the screenshot sheet; area 46 section 7 item 4 says speed boost. Conflict X-28. | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt` | P1 | DECISION NEEDED (X-28) | T-46a-020 |
| PC-16 | Vertical drag on left half changes brightness; right half changes volume | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/VerticalSliders.kt` | P1 | REWRITE. Brightness and volume backends in PC-20 and PC-21. | T-46a-019 |
| PC-17 | Volume pill: device volume in steps; boost value above device maximum | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/VerticalSliders.kt` | P1 | REWRITE: `src/player/controls/volume.rs` with `SystemVolume` trait. Device backend `wpctl set-volume @DEFAULT_AUDIO_SINK@`, 15 steps. Boost through mpv `volume` above 100, capped by `pref_audio_volume_boost_cap`, `volume-max` = cap + 100. | T-46a-021, T-46a-049, T-45b-031 |
| PC-18 | Brightness pill; negative brightness dims with a black veil and never reaches hardware | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/BrightnessOverlay.kt` | P1 | REPLACE: logind `SetBrightness` first, then `/sys/class/backlight` write, then software veil. Order is unverified on target (area 46 section 4.2 item 12). | T-46a-022, T-46a-049 |
| PC-19 | Brightness veil overlay | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/BrightnessOverlay.kt` | P1 | PORT: `src/player/controls/veil.rs`. | T-46a-022 |
| PC-20 | System volume and brightness backends | not in Kotlin (Android APIs) | P1 | REPLACE: PipeWire `wpctl` for volume; logind or sysfs for brightness. | T-46a-049 |
| PC-21 | Player update toast (pill) for messages | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/PlayerUpdates.kt` | P1 | REWRITE: in-window pill, not OS notification (area 46 D-8). | T-46a-029, T-46b-046 |
| PC-22 | Episode list dialog: current episode highlighted, bookmark and fillermark toggles | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/dialogs/EpisodeListDialog.kt` | P1 | REWRITE: `ui/dialogs/episode_list.rs`. | T-46a-024, T-45b-081 |
| PC-23 | Integer picker dialog (list, commits displayed item; not a wheel, D-6, D-11) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/dialogs/IntegerPickerDialog.kt` | P1 | REWRITE: `ui/dialogs/integer_picker.rs`. | T-46b-002 |
| PC-24 | Dialog shell | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/dialogs/PlayerDialog.kt` | P1 | REWRITE | T-46a-027 (dialog dispatch only, per area 46 section 4.2 item 6) |
| PC-25 | Sheet host: one sheet open at a time, scrim, Escape and close button dismiss, drag-to-dismiss dropped | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerSheets.kt` | P0 | REWRITE: in-window `SheetHost` with `gtk::Revealer` and scrim (`ui/host.rs`). Conflict X-22. Drag dismissal is DROP. | T-46b-001, T-46a-029, T-46a-059 |
| PC-26 | Audio tracks sheet (two selection slots: primary and secondary) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/AudioTracksSheet.kt`, `GenericTracksSheet.kt` | P0 | REWRITE | T-46b-011 |
| PC-27 | Subtitle tracks sheet (two slots) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/SubtitleTracksSheet.kt` | P0 | REWRITE | T-46b-012 |
| PC-28 | Chapters sheet | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/ChaptersSheet.kt` | P0 | REWRITE | T-46b-013 |
| PC-29 | Quality sheet: hosters, expansion, per-video state, online sources only | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/QualitySheet.kt` | P0 | REWRITE: hoster partition in `logic/hoster_groups.rs`, state in `logic/hoster_state.rs`. Clicks on a hoster in Loading are ignored (X-29). | T-46b-007, T-46b-014 |
| PC-30 | Playback speed sheet with presets | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/PlaybackSpeedSheet.kt` | P0 | REWRITE: preset parse, format, sort, add, remove in `logic/speed_presets.rs`. | T-46b-015 |
| PC-31 | More sheet: sleep timer, screenshot, other chips | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt` | P0 | REWRITE | T-46b-016, T-46b-017 |
| PC-32 | Sleep timer dialog: hours (0 to 99) and minutes (0 to 59) spin buttons; clock dial dropped | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt` | P1 | REWRITE: two `gtk::SpinButton`. Clock dial is DROP (section 4). | T-46b-018 |
| PC-33 | Screenshot sheet: set as cover, background, thumbnail; share; save | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/ScreenshotSheet.kt` | P1 | REWRITE | T-46b-021 |
| PC-34 | Subtitle settings panel (right edge): typography, colours, miscellaneous cards | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsPanel.kt` and three cards | P1 | REWRITE: writes mpv subtitle properties live. | T-46b-032 |
| PC-35 | Subtitle delay panel with tap-to-sync (measurement applies on stop) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleDelayPanel.kt` | P1 | REWRITE: tap-to-sync state machine in `logic/delay_measure.rs`. Field disabled while a run is active (X-30). | T-46b-030, T-46b-031, T-46a-031 |
| PC-36 | Audio delay panel (shares `DelayCard` with subtitle delay) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/AudioDelayPanel.kt` | P1 | REWRITE | T-46b-031 |
| PC-37 | Video filters panel | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/VideoFiltersPanel.kt` | P2 | REWRITE | T-46b-001 to T-46b-046 |
| PC-38 | Current chapter pill | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/CurrentChapter.kt` | P2 | REWRITE | T-46a-052 |
| PC-39 | Keyboard shortcut map (new on Linux, not in Kotlin) | not in Kotlin | P1 | NEW (D-14). Approve map in area 46 `p1-ui-parity.md` section 10.2 before implementation. | T-46a-043 |
| PC-40 | Mouse wheel and right-click (new on Linux) | not in Kotlin | P1 | NEW | T-46a-044 |
| PC-41 | Mini-player window replacing PiP | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/TopRightPlayerControls.kt` PiP gate | P2 | REPLACE (D-16). User must confirm the feature is wanted. | T-46a-046, T-45b-050 |
| PC-42 | PiP remote actions and params | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt` | P3 | DROP | T-46a-047 |

### 3.3 Player settings, mpv configuration, scripts, custom buttons, torrent settings (`area:48-settings-player-and-scripts`)

Scope: 40 Kotlin files, 3944 LOC. Root: `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/`. Nine root rows: Internal player, Gestures, Decoder, Subtitles, Audio, Custom buttons, Script editor, Torrent, Advanced.

Storage decision (48 DD-1): preferences live in a SQLite `prefs` table with an in-memory cache. Enum values stored by constant name; unknown names fall back to the default silently, as Kotlin does. Custom buttons: one `custom_buttons` table with the same columns as Kotlin and the same seed row (`_id` 1, `+85 s`, favourite, sortIndex 0).

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| PS-01 | Typed preference store with exact keys and defaults | `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/` and `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/` (PlayerPreferences.kt, AudioPreferences.kt, DecoderPreferences.kt, GesturePreferences.kt, SubtitlePreferences.kt, AdvancedPlayerPreferences.kt) | P0 | PORT. SQLite kv with cache. | T-48a-001, T-45b-048 |
| PS-02 | Player settings root with nine rows; sidebar and narrow layouts | `app/src/main/java/eu/kanade/tachiyomi/ui/setting/PlayerSettingsScreen.kt` | P0 | REWRITE: GTK list page. | T-48a-010 |
| PS-03 | Preference row kit: switch, list, slider, text, edit, multi-line, info | `presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt` | P0 | REWRITE: GTK rows (`src/player/controls/widgets.rs` shared with area 46). | T-48a-012 |
| PS-04 | Controls and Hosters groups | settings screen | P0 | PORT | T-48a-013 |
| PS-05 | Display group | settings screen | P0 | PORT | T-48a-014 |
| PS-06 | Progress threshold and preserve-watching-position | settings screen | P0 | PORT. `pref_preserve_watching_position` default false (area 45 D-8). | T-48a-015 |
| PS-07 | Seeking group: horizontal seek, seek bar, intro length, skip length, precise seek | settings screen | P0 | PORT | T-48a-022 |
| PS-08 | Double-tap action settings | settings screen | P0 | PORT | T-48a-023 |
| PS-09 | Single-action gesture dispatch and custom key names | settings screen | P0 | PORT | T-48a-025 |
| PS-10 | Intro skip group and AniSkip settings | settings screen | P1 | PORT (off by default, area 45 D-10) | T-48a-016 |
| PS-11 | Mini-player group (replaces PiP group) | settings screen | P2 | REPLACE | T-48a-017 |
| PS-12 | External player group (MIME-based) | settings screen | P1 | REPLACE: presets in PB-35. Allowlist of packages is DROP (T-48a-053). | T-48a-018, T-48a-053 |
| PS-13 | Orientation setting | settings screen | P3 | DROP | T-48a-019 |
| PS-14 | Gesture sliders group: volume and brightness | settings screen | P1 | PORT | T-48a-021 |
| PS-15 | Media controls: MPRIS and keyboard media keys | settings screen | P1 | REPLACE: MPRIS2 | T-48a-024 |
| PS-16 | Subtitle page: preferred languages, whitelist, blacklist | settings screen | P1 | PORT | T-48a-029, T-48a-030 |
| PS-17 | Language list validator | settings screen | P0 | PORT | T-48a-030 |
| PS-18 | mpv runtime directory and player-start sync (`mpv.conf`, `input.conf`, scripts) | `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/` and `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt` | P0 | PORT. Path `storage_dir/mpv-config/`. | T-48a-036, T-48a-051, T-45a-026 |
| PS-19 | Script editor: list, create, rename, delete files in `mpv-config/scripts` and `mpv-config/script-opts` | script editor screens | P1 | PORT | T-48a-037, T-48a-038 |
| PS-20 | Full-file text editor with Lua and conf syntax highlighter | code editor screens | P1 | REWRITE: one highlighter with `fancy-regex` (DD-5). 15000-character limit only on full-file editor. | T-48a-039 |
| PS-21 | Custom buttons: list, create, edit, delete, reorder, favourite (one primary button) | custom button screens, table `custom_buttons` | P2 | PORT. Delete promotes no other button (DD-3). | T-48a-041, T-48a-042, T-48a-044 |
| PS-22 | Custom button Lua snippets (tap, long press, on-startup) compiled to `custombuttons.lua`, each wrapped in its own `pcall` | custom button screens | P2 | REWRITE (DD-4): one failing body does not break others. | T-48a-043, T-45a-021 |
| PS-23 | mpv Lua bridge (`user-data/aniyomi`) | `app/src/main/assets/aniyomi.lua` | P0 | PORT. Task T-48a-040 is P0 in area 48 but P2 in area 45 (conflict X-27). | T-48a-040 |
| PS-24 | Torrent settings page: enable switch, legal notice, port, trackers list, proxy mode and URL | torrent settings screens | P2 | PORT. Port and proxy validation in T-48a-034. Engine choice is X-01. | T-48a-034, T-45a-076, T-45a-077 |
| PS-25 | Remember volume and brightness decision | not a Kotlin row; decision row | P2 | DECISION (T-48a-052) | T-48a-052, T-45a-083 |
| PS-26 | Android device gates (PiP support, fullscreen SDK gate) | Android API checks | P3 | DROP | T-48a-054 |
| PS-27 | Storage root and mpv-config layout | `app/src/main/java/eu/kanade/presentation/more/settings/screen/` storage pages | P0 | PORT | T-48a-051 |
| PS-28 | Start screen and UI preference model (theme, nav, date format) | settings screens | P0 | PORT. Theme colours follow area 49 tokens. | T-48a-002, T-48a-005 |
| PS-29 | Syntax highlighter test cases and rule sets for Lua and conf | highlighter | P1 | PORT | T-48a-039 |
| PS-30 | Player settings unit and integration tests | none | P0 | NEW | T-48a-056 |

Note on PS-23: area 48 lists the Lua bridge as P0 (`T-48a-040`, L). Area 45 lists the Lua user-data dispatcher as P1 (`T-45b-035`) and the custom-button Lua compile as P2 (`T-45a-021`). Choose the priority once; this matrix shows P0 for the bridge and P2 for custom buttons, because the bridge is needed by the mpv config that is always generated.

### 3.4 Manga downloads (`area:23-downloads-manga`)

Scope: manga download manager, chapter pipeline, index, queue, notifications. Replacement for the Android WorkManager job and the foreground notification is `REPLACE:gio-notification` (GNotification through `gio::Notification`), with the in-window `gtk::ProgressBar` for the Downloads page. Storage: SQLite queue and pending-delete tables replace SharedPreferences (area 23 section 4.1).

Fixed for compatibility (area 23 K-01 to K-04, K-14, K-15): folder layout, file names, CBZ format (ZIP, STORE, UTF-8 names, flat entries), ComicInfo.xml fields.

Constants (quoted from area 23 section 4.1):
- At most 5 sources at once, one chapter per source, two pages per chapter (K-06).
- Retry schedule for page fetch: 2, 4, 8 seconds (K-07).
- Minimum free space: 200 MiB = `209715200` bytes (K-08).
- Warning thresholds: 30 queued items and 15 per source (K-11).

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| DM-01 | Download chapter pages into `source/manga/chapter/` folder layout | `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt` (area 23 inventory) | P0 | PORT: same folder names, bare chapter names in index, never `.cbz` | T-23a-004, T-23a-005, T-23a-014 |
| DM-02 | Chapter output as CBZ (ZIP, STORE, UTF-8, flat) | T-23a-001 to T-23a-045 | P1 | PORT (fixed for compatibility) | T-23a-022 |
| DM-03 | Chapter output as a plain folder | T-23a-022 | P1 | PORT | T-23a-022 |
| DM-04 | ComicInfo.xml written into CBZ | area 23 section 4.1 | P1 | PORT (fields fixed). Read-only parity for legacy files in LS-05. | T-23a-021 |
| DM-05 | Queue with states and status invariants | T-23a-008, T-23a-009 | P0 | REWRITE: `queue state machine` with invariants checked in tests. | T-23a-008, T-23a-009 |
| DM-06 | Per-source scheduler: 5 sources, one chapter each, ERROR sources skipped | area 23 K-06 | P0 | PORT | T-23a-010 |
| DM-07 | Pages per chapter: 2 concurrent page fetches | area 23 K-06 | P0 | PORT | T-23a-013 |
| DM-08 | Page fetch retries 2, 4, 8 seconds; 404 and 410 fail at once (D-DL-11) | area 23 K-07, D-DL-11 | P0 | PORT with the 404/410 rule (decision, default yes) | T-23a-013 |
| DM-09 | Start, pause, stop, clear queue; warning stop | Kotlin downloader lifecycle | P0 | PORT | T-23a-011 |
| DM-10 | Pause discards partial output | area 23 section 4 (kept from Kotlin) | P0 | PORT (Kotlin behaviour). Resume is not implemented. | T-23a-012 |
| DM-11 | Notifications: progress, paused, complete, warning, error | `MangaDownloadNotifier` (area 23) | P1 | REPLACE:gio-notification. Error importance conflict X-12. | T-23a-019 |
| DM-12 | Background running while the window is closed | Android foreground worker | P1 | REPLACE: process kept alive with `GApplication::hold` while work exists (D-DL-07). | T-23a-023, T-23a-011 |
| DM-13 | "Download only over Wi-Fi" | area 23 D-DL-04 | P1 | REPLACE: "Not on a metered connection" through GIO `network_metered`. Settings label says "metered". | T-23a-023 |
| DM-14 | Resume after network returns, unless the user paused | area 23 D-DL-06 | P1 | PORT | T-23a-023 |
| DM-15 | Enqueue with filters, source order, auto-start, warnings | T-23a-024 | P0 | PORT | T-23a-024 |
| DM-16 | Two chapters map to one folder name: warn at enqueue | area 23 D-DL-08 | P1 | PORT (decision default yes) | T-23a-024 |
| DM-17 | Move to top in the queue (within the source group) | area 23 D-DL-05 | P2 | PORT | T-23a-025 |
| DM-18 | Delete chapters, manga, and clean empty folders | T-23a-027 | P0 | PORT | T-23a-027 |
| DM-19 | Pending deletes flushed on reader exit and at startup (D-DL-14) | area 23 | P1 | PORT with the startup flush | T-23a-028 |
| DM-20 | Download index: scan, hourly renew, disk snapshot | T-23a-006, T-23a-007 | P0 | PORT | T-23a-006, T-23a-007 |
| DM-21 | Startup restore of queue, gated on sources; no silent drops | T-23a-035 | P0 | PORT | T-23a-035 |
| DM-22 | Reserved Windows names rejected (CON, PRN, AUX, NUL, COM1-9, LPT1-9) with `_` suffix (`ntfs-names` on) | area 23 D-DL-01 | P0 | PORT | T-23a-003 |
| DM-23 | Single page-number width everywhere (fixes 1000+ prefix collision) | area 23 D-05 | P0 | PORT | T-23a-014 |
| DM-24 | Tall images above 256 MiB decoded: split skipped with a log line (D-DL-13) | area 23 D-DL-13 | P1 | PORT (limitation) | T-23a-018 |
| DM-25 | AVIF, HEIF, JXL pages recognised and stored; split skipped (D-DL-12) | area 23 D-DL-12 | P2 | PORT. Decode is in `area:43-reader-core`. | T-23a-014, T-23a-018 |
| DM-26 | Reader page list for a downloaded chapter | T-23a-031 | P0 | PORT | T-23a-031 |
| DM-27 | Maximum downloads control (`download_slots`, default 1) | `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:69` | P1 | PORT under the new key name proposed in area 23 D-DL-02. Default stays 1 until the user confirms (X-11). | T-23a-037 |
| DM-28 | resmush.it image size reduction | area 23 D-DL-09 | P1 | DROP. Plain-HTTP third-party service. User loses data-saver compression on download. | T-23a-017 |
| DM-29 | `.nomedia` at downloads root | area 23 D-DL-10 | P3 | DROP. Desktop gallery indexers are not a target. Nothing lost. | T-23a-022 |
| DM-30 | Download test suite | area 23 | P0 | NEW | T-23a-036 |
| DM-31 | Filename sanitiser (`buildValidFilename` port) | area 23 | P0 | PORT | T-23a-003 |

### 3.5 Anime downloads (`area:24-downloads-anime`)

Scope: `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/`. Download engine uses ffmpeg (Kotlin invokes ffmpeg for stream remux) and a torrent path through `TorrentServerService` (area 13). Replacement: ffmpeg on PATH (no bundled binary, decision 8 in area 24 section 4).

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| DA-01 | Download episode video (HTTP and HLS sources) | `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt` (area 24) | P1 | REWRITE: reqwest streaming for direct files, ffmpeg for HLS and remux. | T-24a-001 to T-24a-030 |
| DA-02 | Remux stream with ffmpeg | area 24 `p1-topic-ffmpeg-remux.md` | P1 | REPLACE: ffmpeg on PATH. Missing binary gives a clear error. | T-24a-067 |
| DA-03 | Anime concurrency: 3 sources, one episode per source (fixed) | area 24 D-5 | P1 | PORT. `download_slots` hidden for anime (T-24a-055 P2). | T-24a-055 |
| DA-04 | Pause deletes partial `.tmp`; no resume | area 24 Q2 (kept from Kotlin) | P1 | PORT. Changing it updates T-24a-016. | T-24a-016 |
| DA-05 | Background download as a process-lifetime tokio task (no WorkManager) | area 24 Q1 | P1 | REPLACE: tokio runtime task, kept alive by the app process. | T-24a-030, T-24a-046 |
| DA-06 | Wi-Fi-only default `pref_download_only_over_wifi_key` default `true` (`domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:9-12`) | area 24 Q7 | P1 | PORT. Wired ethernet counts as allowed (E-N02 fix, T-24a-031). | T-24a-031 |
| DA-07 | Torrent-backed downloads | area 24 Q3, `TorrentServerService` | P2 | DEFER (X-01). Keep the enum variant and trait hook. | T-24a-023 |
| DA-08 | External downloader 1DM and ADM | area 24 Q4 | P3 | DROP. User loses 1DM and ADM hand-off. Aria2 over JSON-RPC proposed as a P2 replacement, off by default. | T-24a-024, T-24a-025 |
| DA-09 | Import an existing Aniyomi download folder | area 24 Q6 | P2 | PORT as a reader; the user is asked first (X-15) | T-24a-057 |
| DA-10 | SAF and content URIs for the download folder | area 24 T-24a-059 | P1 | DROP. Only plain paths. Nothing is lost on Linux. | T-24a-059 |
| DA-11 | Anime download index reads the same folder layout | area 24 index | P1 | PORT | T-24a-005, T-24a-006 |
| DA-12 | Storage and cache lifecycle | area 24 `p1-topic-storage-cache-lifecycle.md` | P1 | PORT | T-24a-001 to T-24a-074 |
| DA-13 | Anime speed limit | area 24 Q5 | P2 | DEFER: hidden in Kotlin; ffmpeg `-maxrate` only if the user asks | T-24a-055 |

### 3.6 Shared download queue rules (`area:25-downloads-shared`)

Scope: queue model, preference keys and defaults, sort and reorder rules, deletion. Kotlin preference keys are kept 1:1 so backups restore (area 25 D3).

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| DS-01 | Typed download preference store with exact keys and defaults | area 25 section 3 | P0 | PORT | T-25a-001 |
| DS-02 | Reorder: `reorder(ids)` treats the argument as an ordering over the live queue; unknown ids ignored; `reorder([])` no-op | area 25 D2 (Kotlin `updateQueue([])` clears) | P1 | REWRITE (defect fix E-15) | T-25a-001 to T-25a-040 |
| DS-03 | Sort: stable sort then reverse; equal keys reversed for "Newest" and "Descending" | area 25 D4 (E-07) | P1 | PORT (kept quirk) | T-25a-001 to T-25a-040 |
| DS-04 | Sort never moves items across sources; series moves act on the flat list | area 25 D5 (E-08, E-09) | P1 | PORT | T-25a-001 to T-25a-040 |
| DS-05 | Permanent deletion, no trash | area 25 D6 | P1 | PORT | T-25a-001 to T-25a-040 |
| DS-06 | Preference `pref_remove_bookmarked` default `false` (inverted meaning kept) | area 25 D3 | P1 | PORT | T-25a-001 |
| DS-07 | Tab badge hidden at 0 (Kotlin shows 0) | area 25 D7 (E-14, T-25a-022) | P1 | PORT (deliberate change) | T-25a-022 |
| DS-08 | Move to top and bottom visibility uses flat queue index (E-10) | area 25 D8 (T-25a-027) | P1 | REWRITE (defect fix) | T-25a-027 |
| DS-09 | Manga progress polling stops after 600 ticks of 50 ms (30 s) with warning | area 25 D9 (E-13, T-25a-029) | P1 | REWRITE (defect fix: Kotlin polls forever) | T-25a-029 |
| DS-10 | Per-series async lock keyed by (kind, entry id) for removal steps | area 25 D10 (E-22, T-25a-013) | P1 | PORT | T-25a-013 |

Note: area 25 D1 makes `await_items` wait for file removal (Kotlin does not). This is a behaviour change, recorded here.

### 3.7 Library update jobs (`area:26-library-update-jobs`)

Scope: scheduled and manual library refresh for manga and anime, fetch interval algorithm, chapter and episode sync diff, new-item notifications. Kotlin runs these as WorkManager jobs; Kurobayomi runs an in-process scheduler (`area:21` D3, `area:26` D1).

Automatic runs: option A (resident process with the `org.freedesktop.portal.Background` autostart portal) or option B (systemd user timer `kurobayomi-update.timer` running `kurobayomi --library-update=auto`). Area 26 recommends A by default.

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| LU-01 | Automatic library update on an interval, with flex and backoff | `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt` (area 26) | P0 | REPLACE:WorkManager with in-process scheduler | T-26a-014, T-26a-104 |
| LU-02 | Manual library refresh, all or per category, with toast feedback | T-26a-016 | P0 | PORT | T-26a-016, T-26a-102, T-26a-113 |
| LU-03 | Stop semantics: cancel and re-arm | area 26 | P0 | PORT | T-26a-017 |
| LU-04 | Category selection for automatic and manual passes | area 26 | P0 | PORT | T-26a-002 |
| LU-05 | Item restrictions: skip reasons, title ordering, release period (`manga_outside_release_period`) | area 26 section 4 D8 | P0 | PORT, applies to manual passes too (parity; help text states it) (X-16) | T-26a-003 |
| LU-06 | Cancellation is not a failure | area 26 | P0 | PORT | T-26a-021 |
| LU-07 | Per-entry error collection and failure outcome | area 26 | P0 | PORT | T-26a-023 |
| LU-08 | Manga per-entry flow | area 26 | P0 | PORT | T-26a-008 |
| LU-09 | Anime per-entry flow with seasons | area 26 | P0 | PORT | T-26a-009 |
| LU-10 | Chapter sync diff: new, updated, removed | area 26 | P0 | PORT | T-26a-080 |
| LU-11 | Episode sync diff with sticky fields | area 26 | P0 | PORT | T-26a-081 |
| LU-12 | Fetch interval algorithm and interval increase; test suite ported from upstream | T-26a-071, T-26a-072, T-26a-074 | P0 | PORT | T-26a-071, T-26a-072, T-26a-074 |
| LU-13 | Fetch window: yesterday to tomorrow, local zone | area 26 | P0 | PORT | T-26a-070 |
| LU-14 | New-item download: manga enqueued, anime wired with `autoStart = false`; `download_new_episode` default false | area 26 D5 (P-1) | P0 | PORT (anime gap closed) | T-26a-024 |
| LU-15 | Favourite recheck runs before sync for both kinds | area 26 D6 (P-7) | P1 | PORT | T-26a-024 |
| LU-16 | Concurrency: per-kind semaphore of 5, plus process-wide per-source permit of 1 | area 26 D7 | P1 | PORT (Proposal added) | T-26a-022 |
| LU-17 | Restrictions apply to manual passes too | area 26 D8 | P1 | PORT | T-26a-003 |
| LU-18 | Metadata refresh jobs: separate names `MetadataUpdate-manga` and `MetadataUpdate-anime`, de-duplicated by id | area 26 D9 (P-3, P-13) | P1 | REWRITE | T-26a-050 to T-26a-054 |
| LU-19 | New-item notifications: progress, summary, per-entry description | T-26a-030, T-26a-032, T-26a-033 | P1 | REPLACE: GNotification | T-26a-030, T-26a-032, T-26a-033 |
| LU-20 | Tapping a new-item notification opens the highest-numbered new item | area 26 D10 (P-8) | P1 | REWRITE (changed from oldest) | T-26a-034 |
| LU-21 | Battery-not-low constraint | area 26 D4 | P2 | DROP on desktop (documented deviation). Shared 20 percent constant with backup (X-13) for backup only. | none (deviation recorded in area 26 D4) |
| LU-22 | Missed automatic runs while app closed | area 26 D3 | P2 | DECIDED: not replayed; next run starts at next launch if due | T-26a-011 |
| LU-23 | Next unread chapter and next unseen episode for "continue reading" and "continue watching" | area 26 | P1 | PORT | T-26a-061, T-26a-064 |
| LU-24 | Last-updated timestamp and date assignment rules (dateUpload, dateFetch) | area 26 | P0 | PORT | T-26a-026, T-26a-083 |
| LU-25 | Interval and lastUpdate recompute | area 26 | P0 | PORT | T-26a-085 |
| LU-26 | Library job run table and per-kind lock | area 26 | P0 | PORT | T-26a-129 |
| LU-27 | Sources proxy for reSmush.it in metadata (plain HTTP) | T-26a-001 to T-26a-131 | P2 | DECISION: drop the proxy call, same as DM-28 | area 26 D11 |
| LU-28 | Automatic backups and library updates: one battery constant | T-21a-026 | P1 | PORT (20 percent) | T-21a-041 |

### 3.8 Sources, source registry and stubs (`area:17-source-app-layer`)

Scope: source manager, enable and pin state, language grouping, stub sources for missing extensions. Kotlin: `app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt`, `source/anime/AndroidAnimeSourceManager.kt`, `source/manga/MangaSourceExtensions.kt`, `source/anime/AnimeSourceExtensions.kt`.

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| SR-01 | Source registry: live map of installed sources, lookups, lists | T-17a-001 to T-17a-050 | P0 | REWRITE: `SourceRegistry` (`area:17`) | T-17a-005, T-17a-006 |
| SR-02 | Stub source for an uninstalled extension: keeps entry, chapters, history; disables reader, search, refresh; message `source_not_installed` | T-20a-001 to T-20a-100 | P0 | PORT | T-17a-003, T-17a-004, T-17a-007, T-17a-008 |
| SR-03 | Stub fallback for unknown ids | T-17a-001 to T-17a-050 | P0 | PORT | T-17a-008 |
| SR-04 | Source enable and disable, bulk set, pins | T-17a-001 to T-17a-050 | P0 | PORT. `PinFlags` bit model. | T-17a-002, T-17a-019 |
| SR-05 | Language toggle and default enabled languages from system locale | T-17a-001 to T-17a-050 | P0 | PORT: locale from `kuro-platform` (`LANG`) | T-17a-018, T-17a-020 |
| SR-06 | Language grouping and ordering | T-17a-001 to T-17a-050 | P0 | PORT | T-17a-017 |
| SR-07 | Display names and collator | T-17a-001 to T-17a-050 | P0 | REPLACE: `icu_displaynames` and `icu_locale` (T-17a-045) | T-17a-045 |
| SR-08 | Cover and background cache rules and timestamps | T-17a-001 to T-17a-050 | P1 | PORT | T-17a-028 |
| SR-09 | Episode refresh orchestration | T-17a-001 to T-17a-050 | P0 | PORT | T-17a-027 |
| SR-10 | Typed errors instead of casts | T-17a-001 to T-17a-050 | P0 | REWRITE | T-17a-013 |

### 3.9 Manga extensions (`area:18-extensions-manga`)

Scope: 19 Kotlin files, 2387 LOC. Packages: `app/src/main/java/eu/kanade/domain/extension/manga/interactor/`, `app/src/main/java/eu/kanade/tachiyomi/extension/manga/`, `app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/`, `app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/`, `app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/`. Lib version gate: `LIB_VERSION_MIN = 1.4`, `LIB_VERSION_MAX = 1.5` at `app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt:56-57` (spot-checked). Update gate: `1.days` at `app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt:75` (spot-checked). Name prefix stripped: `"Tachiyomi: "` at `MangaExtensionLoader.kt:253` (spot-checked).

Kurobayomi package format: `.kuroext` zip with `manifest.json`, `source.wasm`, optional `icon.png`. Read in memory, never extracted (area 18 section 7.3). WASM sandbox: 256 MiB per instance, 30 s per call (300 epoch ticks of 100 ms), `allowed_hosts`. Conflicts: X-03, X-04, X-05, X-07.

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| EM-01 | Extension browse: Updates pending (with "Update all"), Installed (obsolete first, then lowercase name), Untrusted, Available grouped by language | `app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionsByType.kt` | P0 | REWRITE: pure `views.rs` (`by_type`). Verified by views tests V1 to V14. | T-18a-030 |
| EM-02 | NSFW rows hidden unless `show_nsfw_source` is on | `MangaExtensionLoader.kt` | P0 | PORT. Read per load (D5 in area 18). | T-18a-049 |
| EM-03 | Add a repo: fetch `<base>/repo.json`, store fingerprint, list repo | `app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt` (area 18) | P0 | PORT: `extension_repos` table | T-18a-014, T-18a-020 |
| EM-04 | Repo fingerprint rule: a second repo with the same fingerprint is refused (X19); `NOFINGERPRINT-<n>` takes the real fingerprint on refresh (X16 to X18) | T-18a-014, T-18a-015 | P0 | PORT | T-18a-015 |
| EM-05 | Repo refresh isolation: one failing repo does not cancel siblings | T-18a-036 | P0 | REWRITE: `join_all` with isolation | T-18a-015 |
| EM-06 | Install steps shown: Pending, Downloading, Installing, Installed (also Idle, Error) | `app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt` | P0 | PORT | T-18a-035, T-18a-036 |
| EM-07 | Install failure: digest, signature or fingerprint mismatch ends in Error, no `.part` file, row stays Available | T-18a-034, T-18a-047 | P0 | PORT | T-18a-034, T-18a-047 |
| EM-08 | Cancel during download sets Idle, removes `.part` | T-18a-036, I18 | P0 | PORT (Kotlin left the step unchanged) | T-18a-036 |
| EM-09 | Untrusted package: appears under Untrusted with `UNTRUSTED` tag; Trust dialog shows "Untrusted extension" | `app/src/main/java/eu/kanade/domain/extension/manga/interactor/TrustMangaExtension.kt` | P0 | PORT. Trust rule per version (T2). | T-18a-012, T-18a-013 |
| EM-10 | Uninstall confirm dialog: `Do you really want to remove "<name>" extension?` | T-18a-033 | P1 | PORT | T-18a-033 |
| EM-11 | Downgrade refused: `Installed extension version is higher. Downgrading is not allowed.`; signer change: `Installed extension signature is not matched.` | T-18a-037 | P0 | PORT | T-18a-037 |
| EM-12 | Update check at most once per 24 h; notification `Extension update available` (one) or `%d extension updates available` (more); withdrawn at zero | T-18a-022, `MangaExtensionApi.kt:75` | P1 | PORT (update count in one call; badge count written, deviation) | T-18a-022 |
| EM-13 | Non-blocking startup: window opens before packages finish loading | T-18a-049 | P0 | PORT | T-18a-049 |
| EM-14 | Load order: lib gate, signed, trusted, NSFW gate, instantiate. Untrusted NSFW is Untrusted, not Error | T-18a-008 | P0 | PORT | T-18a-006, T-18a-008 |
| EM-15 | APK-only index entries show `Not runnable on Linux`, install disabled | T-18a-021 | P0 | PORT (D3 in area 18: show, do not hide) | T-18a-021 |
| EM-16 | Android installers, install foreground service, install Activity, MIUI workaround, `extension_installer` preference | `app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/` (InstallerManga.kt, PackageInstallerInstallerManga.kt, ShizukuInstallerManga.kt), `app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallService.kt`, `MangaExtensionInstallActivity.kt` | P3 | DROP. Nothing is lost on Linux. | T-18a-001 to T-18a-054 (inventory drop rows) |
| EM-17 | Downloads capped at 3 concurrent permits (Kotlin has no cap) | T-18a-036 | P1 | REWRITE (Proposal) | T-18a-034 |

### 3.10 Anime extensions (`area:19-extensions-anime`)

Scope: 75 tasks; Kotlin files under `app/src/main/java/eu/kanade/tachiyomi/extension/anime/` and `domain/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeExtension.kt`. Installed packages: `$XDG_DATA_HOME/kurobayomi/extensions/anime/<pkg>.kuroext`, mode `0444`, written by `.part`, fsync, then rename (T-19a-005). Trust key: `"<pkg>:<versionCode>:<signer digest>"` (D4). Update gate: 86,400,000 ms, key `anime_ext_last_check` (D11). Store redirect depth 4 (D7). Store fetch concurrency 4 (D14). Download cap 64 MiB (Q11).

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| EA-01 | Anime extension browse buckets with NSFW filter and sorting | `app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionsByType.kt` | P0 | REWRITE: pure function | T-19a-025 |
| EA-02 | Anime languages filter list | `GetAnimeExtensionLanguages.kt` | P1 | PORT (ICU-free comparator, `"all"` first, D13) | T-19a-001 to T-19a-075 |
| EA-03 | Source enable state and label | `GetAnimeExtensionSources.kt` | P1 | PORT | T-19a-001 to T-19a-075 |
| EA-04 | Store JSON fetch and format detection (JSON only in P0) | `data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt` | P0 | REWRITE: reqwest, flate2, serde. Protobuf store (`0x0A`-style else branch) is P2, T-19a-014. | T-19a-013 |
| EA-05 | Store fetch: JSON array (`0x5B`), JSON object with `indexV2` (`0x7B`), protobuf (else), gzip body | T-19a-013, T-19a-014 | P2 | PORT (JSON first). Legacy repo.json rejected with message. | T-19a-014, T-06a-050 |
| EA-06 | Store add: `Store index URL`, duplicate refused with `This extension store already exists` | T-19a-021 | P1 | PORT | T-19a-021 |
| EA-07 | Deep link `aniyomi://add-repo?url=` and `aniyomi://extension-store?url=` opens Confirm dialog; non-http(s) shows `This link is not an extension store address` | T-19a-021 | P2 | REWRITE: `deeplink.rs`, scheme handler for `aniyomi://` kept for compatibility | T-19a-001 to T-19a-075 |
| EA-08 | Store refresh: reload every store document and extension list; per-store banner `Could not update store "<name>"` | T-19a-021 | P1 | PORT | T-19a-021 |
| EA-09 | Copy index URL to clipboard with banner `Copied to clipboard:` | T-19a-021 | P2 | PORT (`gdk::Clipboard`) | T-19a-001 to T-19a-075 |
| EA-10 | Store website button hidden when website empty | T-19a-021 | P3 | PORT | area 19 |
| EA-11 | Installed package verification: sha256, publicKey 32 bytes, SHA-256 of publicKey equals store `signingKey`, ed25519 `verify_strict`, manifest validation, `pkg` equals `packageName` | T-19a-011 | P0 | PORT. `ed25519-dalek`. | T-19a-011 |
| EA-12 | Downgrade refused; reinstall of same version allowed | T-19a-033 | P0 | PORT | T-19a-033 |
| EA-13 | Anime install queue: FIFO, one commit at a time, cancel emits Idle | area 19 `InstallerAnime.kt` (170 LOC) | P1 | REWRITE (fixes stuck row) | T-19a-032 |
| EA-14 | Aniyomi APK extensions and PackageInstaller, Shizuku, DownloadManager, broadcast receiver, foreground service, install Activity | `app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/PackageInstallerInstallerAnime.kt`, `ShizukuInstallerAnime.kt`, `util/AnimeExtensionInstallReceiver.kt`, `util/AnimeExtensionInstallService.kt`, `util/AnimeExtensionInstallActivity.kt` | P3 | DROP. Every existing Aniyomi anime extension is an APK and will not run (section 4). | T-19a-001 to T-19a-075 |
| EA-15 | Trust (`TrustAnimeExtension.kt`): digest source changes from APK certificate to ed25519 publisher key | `domain/src/main/java/eu/kanade/tachiyomi/extension/anime/` (`TrustAnimeExtension.kt` 32 LOC) | P1 | REWRITE | T-19a-009 |
| EA-16 | Revoke all trust | T-19a-009 | P2 | PORT | T-19a-009 |
| EA-17 | Store add, get, count, remove, update via `extension_store` table | `data/src/main/sqldelightanime/dataanime/extension_store.sq` | P1 | REWRITE on rusqlite; SQL kept as constants | T-19a-012 |
| EA-18 | Stores page: header buttons `Add` and `Refresh` (FAB replaced) | `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreen.kt` | P1 | REWRITE (K-1) | T-19a-001 to T-19a-075 |
| EA-19 | Store backup export and restore (BackupExtensionStore, protobuf tags 1 to 8) | `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeExtensionStoresBackupCreator.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtensionStore.kt` | P1 | PORT (T-21a-001 to T-21a-044/22) | T-19a-027, T-21a-001 to T-21a-044 |
| EA-20 | Anime update check on start, at most once per 24 h; one notification | T-19a-021, T-19a-024 | P1 | PORT | T-19a-013, T-19a-021, T-19a-024 |

### 3.11 Extension strategy (`area:20-extension-strategy-redesign`)

Decision row set. The strategy is three source kinds in this order: built-in Rust sources, declarative TOML scrapers (`kb-source-scrape`, scraper crate), WASM components (`kb-ext-wasm`, wasmtime). Rejected: JVM bridge, Lua, JS as primary, sidecar processes (T-20a-001 to T-20a-100 section 4.1 item 1). Source: `area:20-extension-strategy-redesign/p1-topic-option-matrix.md`.

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| ES-01 | Aniyomi APK extensions do not load | 84-file extension inventory in T-20a-001 to T-20a-100 | P0 | DROP. Library carries over by source id, not by binary. | T-20a-001 to T-20a-100 |
| ES-02 | Source ids frozen: `md5("<name.lowercase()>/<lang>/<versionId>")`, first 8 bytes big-endian, AND `i64::MAX`; default `versionId = 1` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:87-90`, `HttpSource.kt:45` (spot-checked in T-20a-001 to T-20a-100) | P0 | PORT: `kb-source::id` with golden vectors produced by running the Kotlin `generateId` | T-20a-011 |
| ES-03 | Backup source-id resolution | T-20a-001 to T-20a-100 | P0 | PORT | T-20a-094 |
| ES-04 | Missing or unloaded source behaviour | T-20a-001 to T-20a-100 | P0 | PORT | T-20a-095 |
| ES-05 | Anime hoster and video resolution | T-20a-001 to T-20a-100 | P0 | PORT | T-20a-096 |
| ES-06 | Anime episode and season update | T-20a-001 to T-20a-100 | P0 | PORT | T-20a-097 |
| ES-07 | ffmpegStreamArgs, ffmpegVideoArgs, createHttpServer | `animesource/online/AnimeHttpSource.kt` | P3 | DROP. Source-specified ffmpeg tweaks and sources that depend on the local proxy stop working (T-20a-001 to T-20a-100 Q10). | T-20a-001 to T-20a-100 |
| ES-08 | Torrent-based anime sources | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/torrentutils/` | P2 | DEFER: `kb-torrent` (T-20a-001 to T-20a-100 Q9) | T-20a-070 |

### 3.12 Local source (`area:14-source-local-and-metadata`)

Scope: `source-local/src/commonMain/kotlin/tachiyomi/source/local/` and `data/` local helpers. Roots: `local/` and `localanime/` under `storage_dir` (default `$XDG_DATA_HOME/kurobayomi`, folder names kept for Aniyomi compatibility, T-14a-001 to T-14a-058 decision 4). Local source id `0`.

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| LS-01 | Local source identity, id 0, registration and `isLocal` | `source-local/src/commonMain/kotlin/tachiyomi/source/local/` (directory) (T-14a-001 to T-14a-058) | P0 | PORT | T-14a-001, T-14a-002 |
| LS-02 | Local storage roots `local` and `localanime` | T-14a-003 | P0 | PORT | T-14a-003 |
| LS-03 | Manga browse listing (popular, latest, search) and sort | T-14a-001 to T-14a-058 | P0 | PORT | T-14a-012, T-14a-013 |
| LS-04 | Manga chapter list and reader format resolution (CBZ, EPUB, folder) | T-14a-001 to T-14a-058 | P0 | PORT | T-14a-014, T-14a-016, T-14a-017 |
| LS-05 | ComicInfo.xml read and write | T-14a-030, T-14a-034 | P1 | PORT. Kept both read and write (user call, decision 1). | T-14a-030, T-14a-034 |
| LS-06 | Legacy `details.json`, `chapters.json`, `episodes.json` read-only | T-14a-037 to T-14a-040 | P2 | PORT read-only | T-14a-037 to T-14a-040 |
| LS-07 | Anime browse listing, fetch type, episode list, playback hoster for a local file | T-14a-001 to T-14a-058 | P0 | PORT | T-14a-018, T-14a-019, T-14a-023, T-14a-025 |
| LS-08 | Episode thumbnail through ffmpeg or ffprobe | T-14a-001 to T-14a-058 | P2 | REPLACE: `ffmpeg` on PATH, optional. Missing binary disables thumbnails with a visible message. | T-14a-001 to T-14a-058 |
| LS-09 | Dropped unused hooks `getPageList`, `getVideoList` | T-14a-001 to T-14a-058 | P3 | DROP. Both throw `Unused`. Nothing lost. | T-14a-001 to T-14a-058 |
| LS-10 | Local cover manager and natural order sort | T-14a-001 to T-14a-058 | P0 | PORT | T-14a-009, T-14a-020 |

### 3.13 Tracking core (`area:27-tracking-core`)

Scope: the tracker contract, track record model, retry queue, OAuth and PKCE, credential storage, the enhanced-tracker framework and the tracking UI. Task ids: `T-27a-001` to `T-27a-080` (part 1 files) and `T-27b-001` to `T-27b-041` (part 2 files). Every tracking task is P1; no tracking task is needed to read or watch (area 27 X-21).

Tracker ids (spot-checked, `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:20-36`): MyAnimeList `1L` (line 27), AniList `2L` (line 20), Kitsu `3L` (line 21), Shikimori `4L` (line 30), Bangumi `5L` (line 31), Komga `6L` (line 32), MangaUpdates `7L` (line 33), Kavita `8L` (line 22), Suwayomi `9L` (line 35), Simkl `101L` (line 23), Jellyfin `102L` (line 24).

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| TK-01 | Tracker registry with stable ids (table above), identity and capability flags | `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt` | P1 | PORT (ids kept so backups restore) | T-27a-001 to T-27a-080 (tracker contract group) |
| TK-02 | Track record: status, score, progress, start and finish dates, remote id, private flag | `app/src/main/java/eu/kanade/tachiyomi/data/track/model/` | P1 | PORT. Trackers store integers; labels from i18n keys. | T-27a-010 |
| TK-03 | Bind (link local entry to a remote record), add, update, refresh, delete, logout | `app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt`, `BaseTracker.kt`, `DeletableAnimeTracker.kt`, `DeletableMangaTracker.kt` | P1 | PORT. Delete only where the tracker supports it (Shikimori, MangaUpdates). | T-27a-010, T-27a-018 |
| TK-04 | Retry queue for failed remote updates | `app/src/main/java/eu/kanade/tachiyomi/data/track/` queue (SharedPreferences `tracking_queue` in Kotlin) | P1 | REWRITE: SQLite table `track_retry_queue` with a `kind` column (area 27 D-C). Keyed by track id (`_id`) (D-D). Fixes Kotlin row-identity defects D1 to D3. | T-27a-025, T-27a-026, T-27a-029, T-27a-030 |
| TK-05 | Per-binding mutex and conditional queue delete | area 27 | P1 | PORT | T-27a-080 |
| TK-06 | Auto-update gating for reader and player | area 27 | P1 | PORT | T-27a-040, T-27a-018 |
| TK-07 | Auto-open track sheet after adding to library | area 27 | P1 | PORT | T-27a-041 |
| TK-08 | Private tracking: badge, toggle from the row menu, semantics | area 27 | P1 | PORT | T-27a-063, T-27b-017 |
| TK-09 | Credential and token storage | T-27a-001 to T-27a-082 | P1 | REPLACE: Secret Service through `keyring`, service `kurobayomi.track`, accounts `tracker/<id>/oauth`, `tracker/<id>/account`, `tracker/<id>/expired` (area 27 D-F). No plaintext fallback: report "Secret storage is unavailable" and store nothing (D-G, conflict X-17). | T-27a-020, T-27b-027 |
| TK-10 | OAuth request builder, PKCE, redirect router and pending login | T-27a-001 to T-27a-082, `PkceUtil.kt:9` | P1 | REWRITE: 50 random bytes (`getrandom` 0.2), `base64` URL-safe no-pad, `sha2` S256 challenge (conflict X-20). PKCE verifier per login attempt. | T-27a-021, T-27a-022, T-27b-025 |
| TK-11 | Redirect scheme `aniyomi://<tracker>-auth` kept | Kotlin `TrackLoginActivity` | P1 | REPLACE: desktop file with `MimeType=x-scheme-handler/aniyomi;` plus a loopback fallback. Keep the scheme (area 27 D-A). | T-27a-022, T-27b-029 |
| TK-12 | Manual paste fallback for the redirect link | T-27b-001 to T-27b-041 | P1 | PORT (used when the scheme handler is not registered) | T-27b-029 |
| TK-13 | Remote search page: auto-search, loading, results, empty, error states | T-27b-001 to T-27b-041 | P1 | REWRITE | T-27b-012 |
| TK-14 | Enhanced tracker auto-match on "Add tracking" | T-27b-001 to T-27b-041 | P1 | PORT | T-27b-013 |
| TK-15 | Progress and read-state mapping for mark-read decisions | area 27 | P1 | PORT | T-27a-018 |
| TK-16 | Tracking date handling: `chrono` (not `time`) | area 27 D-H | P1 | PORT (decision) | T-27a-010 |

### 3.14 AniList (`area:28-tracker-anilist`)

Constants (T-28a-001 to T-28a-058): GraphQL endpoint `https://graphql.anilist.co/`, client id `5338`, `https://anilist.co/api/v2/`, `https://anilist.co/manga/`, `https://anilist.co/anime/`.

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| AL-01 | Login: browser opens authorize URL, redirect `aniyomi://anilist-auth#access_token=<token>&`, token extracted | `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt`, `TrackLoginActivity` | P1 | REWRITE: same URL and parse (`T-28a-008`), fragment parsed by the redirect router | T-28a-008, T-28a-011, T-28a-040 |
| AL-02 | Token expiry in seconds, legacy milliseconds migration, expiry on 401 | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-009 |
| AL-03 | Auth header, User-Agent, not-authenticated gate | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-010 |
| AL-04 | Login fetches Viewer, persists, surfaces errors | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-011 |
| AL-05 | Logout semantics | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-012 |
| AL-06 | GraphQL transport with 85-per-minute rate limiter | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-014 |
| AL-07 | GraphQL error envelope and HTTP status mapping | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-015 |
| AL-08 | Manga and anime search, result mapping and DTO conversion | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-016, T-28a-017, T-28a-018 |
| AL-09 | Find entry and user-list mapping | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-019 |
| AL-10 | Add entry mutation, capture of MediaList id | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-020 |
| AL-11 | Update entry mutation payload | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-021 |
| AL-12 | Fuzzy dates and create-date encoding | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-023 |
| AL-13 | Progress-driven status transitions | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-024 |
| AL-14 | Bind (register) for manga and anime | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-025 |
| AL-15 | Refresh from remote; lazy library id resolution before write | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-026, T-28a-027 |
| AL-16 | Status mapping to MediaListStatus strings | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-007, T-28a-003, T-28a-004 |
| AL-17 | Score formats `POINT_100`, `POINT_10_DECIMAL`, `POINT_10`, `POINT_5`, `POINT_3` read from the account `mediaListOptions.scoreFormat` | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-005, T-28a-006 |
| AL-18 | Score picker per format | T-28a-001 to T-28a-058 | P1 | REWRITE | T-28a-054 |
| AL-19 | Status picker dialog | T-28a-001 to T-28a-058 | P1 | REWRITE | T-28a-053 |
| AL-20 | Info panel rows and AniList flags; description HTML to plain text | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-031, T-28a-029 |
| AL-21 | Track row persistence and per-track write serialisation | T-28a-001 to T-28a-058 | P1 | PORT | T-28a-039 |
| AL-22 | Search screen replacing the mock | T-28a-001 to T-28a-058 | P1 | REWRITE | T-28a-057 |
| AL-23 | Settings row: AniList login and logout | T-28a-001 to T-28a-058 | P1 | REWRITE | T-28a-047 |
| AL-24 | Serde DTOs for every AniList response | T-28a-001 to T-28a-058 | P1 | REWRITE (serde) | T-28a-037 |
| AL-25 | Test suite | T-28a-001 to T-28a-058 | P1 | NEW | T-28a-038 |

### 3.15 MyAnimeList (`area:29-tracker-myanimelist`)

Scope: 11 Kotlin files, 1014 LOC: `app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeList.kt`, `MyAnimeListApi.kt`, `MyAnimeListInterceptor.kt` (Bearer header, token refresh under a lock, "login expired" flag), `MyAnimeListUtils.kt`, and eight `dto/*.kt` files. MAL id is the key used by the AniSkip lookup (`area:45-player-core` T-45b-021) and by AniChart.

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| MY-01 | OAuth login with PKCE | `MyAnimeListApi.kt` | P1 | REWRITE (TK-10) | T-29a-001 to T-29a-059 (T-29a-001 to T-29a-059); T-27b-025 |
| MY-02 | Bearer header and token refresh under a lock; "login expired" flag | `MyAnimeListInterceptor.kt` | P1 | PORT | T-29a-001 to T-29a-059 |
| MY-03 | Search and list | `MyAnimeListApi.kt` | P1 | PORT | T-29a-001 to T-29a-059 |
| MY-04 | Progress sync and status mapping | `MyAnimeListUtils.kt` (status to wire string and back) | P1 | PORT | T-29a-001 to T-29a-059 |
| MY-05 | Remote id lifecycle | area 29 | P1 | PORT | T-29a-001 to T-29a-059 |
| MY-06 | Date parsing | `MyAnimeListApi.kt` | P1 | PORT (chrono) | T-29a-001 to T-29a-059 |

### 3.16 Kitsu (`area:30-tracker-kitsu`)

Scope: 11 Kotlin files under `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/` and its `dto/` subpackage. OAuth password grant, JSON:API library-entry CRUD, Algolia-backed title search. Login and logout dialogs live in `app/src/main/java/eu/kanade/presentation/more/settings/` (area 30 scope note). Kitsu is P1 parity only.

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| KI-01 | Login: OAuth password grant | `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/` | P1 | REWRITE: the password is sent once and never stored; only the token is kept in Secret Service (TK-09) | T-30a-001 to T-30a-035 (T-30a-001 to T-30a-035); T-30a-001 to T-30a-035 |
| KI-02 | Library entry CRUD over JSON:API | area 30 | P1 | PORT | T-30a-001 to T-30a-035 |
| KI-03 | Algolia title search | area 30 | P1 | PORT | T-30a-001 to T-30a-035 |
| KI-04 | Session refresh and logout | area 30 | P1 | PORT | T-30a-001 to T-30a-035 |

### 3.17 Shikimori and Bangumi (`area:31-tracker-shikimori-bangumi`)

Endpoints: Shikimori `https://shikimori.one` (registry id 4). Bangumi `https://bgm.tv` (registry id 5). Login is authorisation code only, through `ShikimoriApi.authUrl()` and `BangumiApi.authUrl()`.

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| SK-01 | Authorisation-code login for Shikimori | `app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/` | P1 | REWRITE (TK-10, TK-11) | T-31a-001 to T-31a-049 (T-31a-001 to T-31a-049); T-31a-001 to T-31a-049 |
| SK-02 | Catalogue search for manga and anime | area 31 | P1 | PORT | T-31a-001 to T-31a-049 |
| SK-03 | Bind: find or create remote entry, copy personal fields | area 31 | P1 | PORT | T-31a-001 to T-31a-049 |
| SK-04 | Add, update progress, status, score; refresh | area 31 | P1 | PORT | T-31a-001 to T-31a-049 |
| SK-05 | Delete remote entry (Shikimori only) | area 31 | P1 | PORT | T-31a-001 to T-31a-049 |
| SK-06 | Status list, labels, score list | area 31 | P1 | PORT | T-31a-001 to T-31a-049 |
| SK-07 | Logout deletes stored credentials | area 31 | P1 | PORT (Secret Service delete) | T-31a-001 to T-31a-049 |
| BG-01 | Authorisation-code login for Bangumi with `https://bgm.tv` and the alternative `http` host listed in the source | `app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/` | P1 | REWRITE | T-31a-001 to T-31a-049 |
| BG-02 | Bangumi search, bind, update, refresh | area 31 | P1 | PORT | T-31a-001 to T-31a-049 |

Remote API verification for both services is listed in T-31a-001 to T-31a-049. Shikimori and Bangumi use tracker ids 4 and 5, which backups store as integers.

### 3.18 MangaUpdates and Simkl (`area:32-tracker-mangaupdates-simkl`)

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| MU-01 | MangaUpdates manga tracker, id 7 | `app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/` | P1 | PORT | T-32a-001 to T-32a-072 (T-32a-001 to T-32a-072) |
| MU-02 | MangaUpdates list states: Reading, Wish, Complete, Unfinished, On hold | area 32 section 1.1 | P1 | PORT | T-32a-001 to T-32a-072 |
| MU-03 | MangaUpdates chapter number and rating 0.0 to 10.0 in steps of 0.1 | area 32 section 1.1 | P1 | PORT | T-32a-001 to T-32a-072 |
| MU-04 | MangaUpdates remote delete | area 32 section 1.1 | P1 | PORT | T-32a-001 to T-32a-072 |
| SI-01 | Simkl anime, TV and movies, id 101 | `app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/` | P1 | PORT | T-32a-001 to T-32a-072 |
| SI-02 | Simkl statuses: Watching, Completed, On hold, Not interesting, Plan to watch | area 32 section 1.1 | P1 | PORT | T-32a-001 to T-32a-072 |
| SI-03 | Simkl episode count and integer rating 0 to 10 | area 32 section 1.1 | P1 | PORT | T-32a-001 to T-32a-072 |
| SI-04 | Simkl OAuth login | area 32 section 1.1 | P1 | REWRITE (TK-10) | T-32a-001 to T-32a-072 |
| SI-05 | Simkl has no remote delete | area 32 section 1.1 | P1 | PORT (no delete) | T-32a-001 to T-32a-072 |

### 3.19 Enhanced trackers (`area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin`)

Enhanced trackers never show a search dialog. They bind automatically to entries that come from one extension source, and they talk to the server that source already uses. Bound source classes (T-33a-001 to T-33a-111): Kavita `eu.kanade.tachiyomi.extension.all.kavita.Kavita` (`Kavita.kt:99`); Komga `eu.kanade.tachiyomi.extension.all.komga.Komga` (`Komga.kt:97`); Suwayomi `eu.kanade.tachiyomi.extension.all.tachidesk.Tachidesk` (`Suwayomi.kt:89`); Jellyfin `eu.kanade.tachiyomi.animeextension.all.jellyfin.Jellyfin` (`Jellyfin.kt:86`).

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| EN-01 | Kavita chapter progress through `mark-chapter-until-as-read` (tracker 8) | `app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/` | P1 | PORT. Requires the Kavita source (ES-01). | T-33a-001 to T-33a-111 (T-33a-001 to T-33a-111) |
| EN-02 | Komga read progress through Mihon-specific v2 endpoints (tracker 6) | `app/src/main/java/eu/kanade/tachiyomi/data/track/komga/` | P1 | PORT | T-33a-001 to T-33a-111 |
| EN-03 | Suwayomi chapter read flags through ``$url/chapter/$lastChapterIndex` (`app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt:91`)` (tracker 9) | `app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/` | P1 | PORT | T-33a-001 to T-33a-111 |
| EN-04 | Jellyfin "played" flags through a POST to a path ending in `PlayedItems` (`app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinApi.kt:148`) (tracker 102, anime) | `app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/` | P1 | PORT. Progress model in `p1-topic-jellyfin-progress-model.md`. | T-33a-001 to T-33a-111 |
| EN-05 | Tracking URL parser for movie, season and missing fragment | area 33 | P1 | PORT | T-33a-011 |
| EN-06 | Item read for movies and seasons | area 33 | P1 | PORT | T-33a-012 |
| EN-07 | Season progress with the continuous rule | area 33 | P1 | PORT | T-33a-015 |
| EN-08 | Enhanced tracker auto-match when the entry is added | area 33 | P1 | PORT | T-27b-013 |

### 3.20 Backup create (`area:21-backup-format-and-create`)

Scope: `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/` (creators, `BackupCreator.kt`), `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/`. Output `.tachibk`. Task ids: `T-21a-001` to `T-21a-044`: 40 P1, 2 P2 (T-21a-019 extensions artifact is one of them) and 2 P3 (T-21a-033 dead code removal is one of them), see section 0.4.

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| BK-01 | Create a backup file with the selected sections | `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt` | P1 | PORT | T-21a-010 (options), T-21a-040 (tests) |
| BK-02 | Hand-written protobuf codec, no prost or protobuf crate | T-21a-001 to T-21a-044 | P1 | REWRITE (kotlinx ProtoBuf differs from proto3 defaults; repeated scalars unpacked, sealed `PreferenceValue`, enums are ordinals) | T-21a-002 |
| BK-03 | Gzip with level 6 (`Compression::default()`) | T-21a-001 to T-21a-044 | P1 | REPLACE: `flate2` | T-21a-001 to T-21a-044 (protobuf topic) |
| BK-04 | File name `kurobayomi_yyyy-MM-dd_HH-mm-ss.tachibk`, collision suffix `-2`, `-3` | T-21a-001 to T-21a-044 (Aniyomi minute resolution, B-07) | P1 | REWRITE | T-21a-001 to T-21a-044 |
| BK-05 | Write `.part`, fsync, rename; self-check decodes the written file | T-21a-001 to T-21a-044 | P1 | REWRITE | T-21a-001 to T-21a-044 |
| BK-06 | Retention runs after a successful write, not before | T-21a-025 (Kotlin prunes first, `BackupCreator.kt:89`) | P1 | REWRITE (defect E-04 fixed) | T-21a-025 |
| BK-07 | `is_legacy` written as `false` | T-21a-001 to T-21a-044 | P1 | PORT | T-21a-001 to T-21a-044 `p1-data-and-config.md` section 10 |
| BK-08 | Preference values encoded only after the sealed `PrefValue` tags are verified from user fixtures | T-21a-001 to T-21a-044, Q1, T-21a-034 | P1 | DECISION NEEDED (fixtures from the user, X-33) | T-21a-034, T-21a-006, T-21a-020, T-21a-021 |
| BK-09 | Automatic backups: folder, retention, startup cleanup | `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt` and job | P1 | PORT | T-21a-025 |
| BK-10 | Automatic backup scheduler: interval, due rule, deferrals, backoff | T-21a-001 to T-21a-044 | P1 | REPLACE:WorkManager with in-process loop and persisted last-run timestamp (D3) | T-21a-026 (P1 scheduler); T-21a-041 (P2 systemd timer) |
| BK-11 | Single-flight slot shared by manual and automatic runs | T-21a-001 to T-21a-044 | P1 | REWRITE | T-21a-001 to T-21a-044 |
| BK-12 | Automatic run on battery under 20 percent is deferred (shared constant) | T-21a-026 | P1 | PORT (20 percent; X-13) | T-21a-026 |
| BK-13 | Automatic backups exclude extensions and private settings (`__PRIVATE_*`) | T-21a-044 | P1 | PORT (no secrets unattended) | T-21a-044 |
| BK-14 | Manual backup with private settings after explicit toggle | T-21a-044 | P1 | PORT | T-21a-044 |
| BK-15 | Extensions option (APK bytes in Kotlin) | `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/ExtensionsBackupCreator.kt` | P2 | DROP for P1 (Q5 recommendation). Revisit with T-21a-019 once `area:20` fixes the installed artifact. | T-21a-019 |
| BK-16 | Last automatic backup timestamp and info text | T-21a-001 to T-21a-044 | P1 | PORT | T-21a-031 |
| BK-17 | Automatic backup interval setting | T-21a-001 to T-21a-044 | P1 | PORT | T-21a-032 |
| BK-18 | Storage base directory dependency | T-21a-001 to T-21a-044 | P1 | PORT | T-21a-037 |
| BK-19 | Migration equivalents for automatic backup setup | T-21a-001 to T-21a-044 | P1 | PORT | T-21a-039 |
| BK-20 | Legacy (pre-1.0) backup detection on read | T-21a-001 to T-21a-044 | P1 | PORT | T-21a-008, T-21a-009 |
| BK-21 | JSON import and export | Kotlin rejects JSON by magic bytes (D11) | P3 | DROP. Matches Aniyomi. Nothing lost. | none (T-21a-001 to T-21a-044) |
| BK-22 | Dead code: `full/models/*`, `BackupExtensionPreferences`, both `Broken*` variants, `backup_flags` | T-21a-001 to T-21a-044 | P3 | DROP. No references. Nothing lost. | T-21a-033 |

### 3.21 Backup restore (`area:22-backup-restore`)

Scope: `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/` and `restore/restorers/`. Restorers: categories, library entries (manga and anime), sources, preferences, extension repositories, extension stores, custom buttons, seasons.

| Row | Feature | Aniyomi evidence | Pri | Disposition | Tasks |
|---|---|---|---|---|---|
| BR-01 | Restore from `.tachibk` (magic sniffing, legacy detection) | `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt` | P1 | PORT | T-22a-003 |
| BR-02 | Sections run in order: categories before entries (defect D3 fixed) | T-22a-001 to T-22a-053 | P1 | REWRITE | T-22a-001 to T-22a-053 |
| BR-03 | Progress amount matches each emitted step (defect D2 fixed) | area 22 D2 | P1 | REWRITE | T-22a-006 |
| BR-04 | Cancel is not an entry error: `RestoreError::Cancelled` re-raised; error log on every exit path | area 22 D4, D17 | P1 | REWRITE | T-22a-001 to T-22a-053 |
| BR-05 | Anime source names shown in errors (defect D1 fixed) | `BackupRestorer.kt:87`, `:89`, `:173`, `:193`, `:196` | P1 | REWRITE | T-22a-001 to T-22a-053 |
| BR-06 | Seasons: pre-pass; children inside parent transaction; orphans get `parent_id = NULL` | area 22 seasons-and-legacy section 1.4 (D7, D18) | P1 | REWRITE | T-22a-023 |
| BR-07 | `initialized` persists the computed value, not the backup value | area 22 D6 | P1 | REWRITE | T-22a-014 |
| BR-08 | Custom button favourite: one favourite per restore | area 22 D10 | P1 | REWRITE (count fixed) | T-22a-028 |
| BR-09 | Manga repo: identical repo is a no-op, not an error | area 22 D11 | P1 | REWRITE | T-22a-027 |
| BR-10 | Category string sets replaced on every restore (not appended) | area 22 D12 | P1 | REWRITE | T-22a-009 |
| BR-11 | Error log write failure logs a warning instead of silence | area 22 D15 | P1 | REWRITE | T-22a-001 to T-22a-053 |
| BR-12 | Error text shows the inner message only (no `null`, no `java.lang.IllegalStateException:` prefix) | area 22 D13, D19 | P2 | REWRITE | T-22a-043 |
| BR-13 | Storage bar progress clamped to 0.0 to 1.0 | area 22 D14 | P1 | REWRITE | T-22a-001 to T-22a-053 |
| BR-14 | Extension section label is "Extensions" | area 22 D24 | P1 | REWRITE | T-22a-001 to T-22a-053 |
| BR-15 | Dead sync mode removed | area 22 D21 | P1 | DROP (dead code). Nothing lost. | T-22a-045 |
| BR-16 | Restore notification: throttled progress, at most one update every 500 ms plus final 100 percent | `NotificationCompat` in Kotlin | P1 | REPLACE:gio-notification with an in-window `gtk::ProgressBar` | T-22a-001 to T-22a-053 |
| BR-17 | Metadata merge when the backup is newer (keep DB title, viewer flags, chapter flags, update strategy, date added, last update) and overwrite when older or equal | area 22 section 4.2 item 1 (user call, D5) | P1 | DECISION NEEDED (X-34). Parity is the default. | T-22a-001 to T-22a-053 |
| BR-18 | Progress regression: restore takes the maximum of DB and backup for `lastPageRead` and `lastSecondSeen` | area 22 section 4.2 item 2 (E-29, E-30) | P1 | PROPOSAL: maximum (user call) | T-22a-001 to T-22a-053 |
| BR-19 | Duplicate category names merged by name | area 22 section 4.2 item 3 (E-16, Q-10) | P1 | PROPOSAL: merge (user call) | T-22a-001 to T-22a-053 |
| BR-20 | Unknown and internal preference keys: allowlist; others counted as skipped | area 22 section 4.2 item 4 (E-60) | P2 | PROPOSAL: allowlist (T-22a-052) | T-22a-052 |
| BR-21 | Non-protobuf input rejected with "Invalid backup file" | area 22 section 4.2 item 5 (Q-08, Q-09) | P1 | PROPOSAL: reject (user call) | T-22a-003 |
| BR-22 | Legacy 0.x backups converted | area 22 section 4.2 item 8 | P1 | PORT (user call: support) | T-22a-050 |
| BR-23 | Close window during a restore: confirmation with "Keep running" and "Cancel restore and close" | area 22 section 4.2 item 7 (T-22a-046) | P2 | PROPOSAL | T-22a-046 |
| BR-24 | Restore job not persisted across app restart | area 22 section 4.3 | P1 | DROP (accepted loss): restore is interactive and not resumable | none (T-22a-001 to T-22a-053) |
| BR-25 | APK extension payloads installed through Android installer | `ExtensionsRestorer.kt:82-99` | P3 | DROP. Kurobayomi extensions are not APKs (T-20a-001 to T-20a-100). Stored extension repos and trust restore instead. | T-22a-029 |
| BR-26 | MIUI warning | Android device helper | P3 | DROP. Nothing lost. | T-22a-039 |
| BR-27 | Restore of extension repos and stores, trust entries; trust keys not carried into backups | T-18a-012 | P1 | PORT | T-22a-027, T-18a-012 |
| BR-28 | Restore of custom buttons | area 22 | P1 | PORT | T-22a-028 |
| BR-29 | Restore of anime and manga sources by id, stubs for missing sources | T-20a-094, T-20a-095 | P0 | PORT | T-20a-094, T-20a-095 |
| BR-30 | Restore of preferences by typed value | area 22 | P1 | PORT (after fixtures, X-33) | T-22a-001 to T-22a-053 |
| BR-31 | Restore of categories | area 22 | P1 | PORT | T-22a-009 |
| BR-32 | Restore storage check (free space before write) | T-22a-001 to T-22a-053 | P1 | PORT | T-22a-001 to T-22a-053 |

## 4. Dropped features and what the user loses

Every DROP row in section 3 is listed here once, with the loss stated. A DROP is a decision for this plan; items marked "user call" are not final until the user answers section 5.

| Dropped item | Rows | User loss |
|---|---|---|
| Installing every existing Aniyomi anime and manga extension (APK) | ES-01, EM-16, EA-14 | Every Aniyomi extension binary stops working. Library entries keep their source ids (`source_id` is frozen, ES-02). Entries whose source is not ported show a stub with `source_not_installed` (SR-02). |
| Android installers: PackageInstaller, Shizuku, `pm install-*`, DownloadManager, install broadcast receiver, foreground install service, install Activity, MIUI workaround, `extension_installer` preference | EM-16, EA-14, BR-26 | None beyond the extension loss above. |
| Picture-in-picture (Android) | PB-28, PC-42 | Video does not float over other windows. Mini-player (PC-41, P2) gives transport controls only. |
| Orientation lock and rotate button | PB-29, PC-07, PS-13 | No device rotation control. Video rotate is handled by mpv (`T-45a-035`). |
| Audio focus policy UI | PB-27 | None: KDE and PipeWire handle the audio stream. |
| Haptics | area 46 D-17 | No vibration feedback on touch input. |
| Clock dial in the sleep timer | PC-32 | Timer is set with two spin buttons (hours, minutes). Same values. |
| Drag-to-dismiss on sheets | PC-25 | Sheets close by scrim, Escape, close button, or back. |
| External players: MX Player, Just Player, Next Player, X Player, mpv remote, Web Video Caster | PB-35, PS-12, PS-24 (no Android package) | Hand-off to these apps is unavailable. `mpv`, `vlc`, `celluloid`, `custom` and the desktop default remain. |
| 1DM and ADM external downloaders | DA-08 | Download hand-off to those apps is unavailable. aria2 (P2) is the proposed replacement and is off by default. |
| SAF and content URIs for downloads | DA-10 | Downloads go only to plain paths. Nothing lost on Linux. |
| resmush.it image size reduction | DM-28, LU-27 | Downloaded pages are not compressed by the third-party data-saver. |
| `.nomedia` marker at the downloads root | DM-29 | None. Desktop gallery indexers are not a target. |
| Resumable pause of anime and manga downloads | DM-10, DA-04 | Pause discards partial output (Kotlin behaviour kept). Resume is a P2 item if the user asks. |
| Battery-not-low constraint for library updates | LU-21 | Library updates run on battery regardless of charge (desktop has no reliable signal). Backup keeps its 20 percent deferral (BK-12). |
| Restore job survives an app restart | BR-24 | A restore that is interrupted by a crash or a closed window must be started again from the file. |
| APK extension payloads in backups, and their restore | BK-15, BR-25 | Extension binaries are not in backups. Extension repos, trust rows and stores are restored instead (BR-27). User call on the P2 variant (X-36). |
| JSON backup import and export | BK-21 | None: Aniyomi also rejects JSON by magic bytes. |
| Dead backup code: `full/models/*`, `BackupExtensionPreferences`, both `Broken*` variants, `backup_flags` | BK-22 | None: no references in the repository. |
| Dead restore sync mode | BR-15 | None: `isSync` is never set to true. |
| Android device gates (PiP support, fullscreen SDK gate) | PS-26 | None. |
| Android package allowlist for external players | PS-12 (`T-48a-053`) | Replaced by MIME lookup. |
| Local source hooks `getPageList` and `getVideoList` | LS-09 | None: both throw `Unused` and are never called. |
| Source-specified `ffmpegStreamArgs`, `ffmpegVideoArgs`, and `createHttpServer` | ES-07 | Sources that depend on ffmpeg arguments from the source, or on the local proxy, stop working. Conflict X-02 may keep the local server. |
| Tracker tokens stored in a plaintext file when no Secret Service runs | TK-09 (`T-27b-027`) | Tracker login is unavailable without a Secret Service (for example a bare window manager). The user gets the error "Secret storage is unavailable". |
| Aniyomi anime torrent path with the Go TorrServer library | PB-38, DA-07 | Torrent-backed episodes do not play until the P2 engine exists (conflict X-01). |
| Legacy `repo.json` store format (`indexV2`) | EA-05 | Legacy repos are rejected with a message. Protobuf store is P2 (`T-19a-014`). |

## 5. Decisions needed from the user

Each item lists the choice and the recommendation the plan currently assumes. The plan is blocked on each item only where the matching tasks are P0 or P1.

| # | Decision | Recommendation | Blocks |
|---|---|---|---|
| U-01 | Is torrent playback wanted at all? If yes, which engine: in-process `librqbit` or a TorrServer child process? | Not for daily use. Defer. If wanted: TorrServer child process (X-01). | PB-38, DA-07, T-45a-072 to T-45a-079, T-48a-034 |
| U-02 | Keep the source-side local HTTP server (`createHttpServer` replacement)? | Keep as P2 only if a source needs it. | PB-37, X-02 |
| U-03 | Is WASM needed in the first usable release? Are built-in sources enough? | Built-in sources first (P0). WASM P1 (X-03). | ES-01 to ES-08, EM-03 to EM-15, EA-04 to EA-13 |
| U-04 | Accept the `.kuroext` zip with `manifest.json` as the only installable package format? | Yes (X-05). | EM-07, EA-11 |
| U-05 | Concurrent download slot default for manga: keep 1, or move to 5 under a renamed key? | Keep 1 until the user decides (X-11). | DM-27 |
| U-06 | Error and warning notifications: default importance or low importance? | Default importance for errors and warnings (X-12). | DM-11 |
| U-07 | Battery threshold for automatic runs | 20 percent, shared by backup and library updates (X-13). | BK-12, LU-21 |
| U-08 | Sheet container: in-window overlay or modal windows | In-window overlay (X-22). | PC-25 |
| U-09 | Hoster in ERROR: allow retry | No, Kotlin default. Add a hidden `hoster_retry` flag off by default (area 45 D-6). | PB-10 |
| U-10 | Resume at the saved position on reopen (`pref_preserve_watching_position`) | Off by default (area 45 D-8). | PB-07, PS-06 |
| U-11 | Subtitle "Off" entry in the subtitle track sheet | Add it (`sid=no`) (X-26). | PB-12 |
| U-12 | AniSkip on by default | Off (area 45 D-10). | PB-14, PS-10 |
| U-13 | Lua custom buttons: keep working for existing user scripts, or a native custom-button editor only? | Native editor first; Lua scripts imported as P2 (area 45 section 4.2 item 5). | PS-21, PS-22, PB-39 |
| U-14 | Lua bridge priority (X-27) | P0 bridge, P2 custom buttons. | PB-39, PS-23 |
| U-15 | Long-press action on the video (X-28): speed boost or screenshot sheet | Speed boost (overview DoD item 4). | PC-14, PC-15 |
| U-16 | Brightness control order: logind, then sysfs, then veil | As written (area 46 section 4.2 item 12). Test on the target first. | PC-18, PC-20 |
| U-17 | Mini-player window (PiP replacement) wanted at all | Only if the user asks; P2 (area 46 section 4.2 item 11). | PB-41, PC-41 |
| U-18 | Keyboard shortcut map (PC-39) | Approve the map in area 46 `p1-ui-parity.md` section 10.2 before implementation. | PC-39 |
| U-19 | Preference encoding for backups needs a real Aniyomi backup file and debug schema text | Provide `current-all.tachibk` and `current-default.tachibk` (X-33). | BK-08, BR-30 |
| U-20 | Restore: metadata when the backup is newer (X-34), progress regression (take maximum), duplicate categories (merge by name), unknown preference keys (allowlist), non-protobuf input (reject), legacy 0.x backups (support), close during a restore (confirm) | As listed in section 3.21 (BR-17 to BR-23). | BR-17 to BR-23 |
| U-21 | Automatic backups exclude extensions and private settings | Yes, exclude (BK-13). | BK-13, BK-14 |
| U-22 | Import an existing Aniyomi download folder (X-15) | Build the layout reader (P1); import is P2 and asks first. | DA-09 |
| U-23 | Legacy `details.json`, `chapters.json` and `episodes.json` for local source (area 14 decision 2) | Read-only parity (P2). | LS-06 |
| U-24 | Torrent settings page in the first release | Not needed for daily use (X-10). | PS-24 |

## 6. Additional conflicts found while writing this document

These are conflicts found in the owning area overviews. Each one is a fact the owning doc must correct.

| # | Area | Finding | Action |
|---|---|---|---|
| X-35 | `area:22-backup-restore` | Section 4.4 of the overview says the defect citations need a line audit. `BackupRestorer.kt:196` (D1) and `MangaExtensionRepoRestorer.kt:122-125` (D11) do not match the files (the file is 38 lines long; the error is at `MangaExtensionRepoRestorer.kt:25`). `StorageInfo.kt:214` (D14) is outside a 75-line file. | Re-derive every defect citation in `p1-behavior.md` section 13 before implementation. |
| X-36 | `area:21-backup-format-and-create` | Q5 proposes P2 extension payloads (T-21a-019). Whether an installed `.kuroext` artifact belongs in a backup depends on `area:20` deciding what an installed extension artifact is. | Decide with the user (U-20 covers the restore options; the extension option is BK-15). Keep the option off until then. |
| X-37 | `area:19-extensions-anime` and `area:18-extensions-manga` | Both define the same store and install pipeline with different task sets. Area 20 section 4.3 item 8 notes the duplicate task areas. | Pick one owner per task; this matrix uses area 19 for anime store rows and area 18 for manga repo rows. |
| X-38 | `area:46-player-controls-ui` | Priority inversions and two dependency cycles are reported in its overview section 6.2 and are not changed there. | Decide per edge (X-23, X-24). |
| X-39 | `area:45-player-core` and `area:48-settings-player-and-scripts` | Module names and settings module names differ (`settings.rs` versus `settings_view.rs` in area 46; `prefs.rs` in area 45). | Use one `settings.rs` (area 46 section 4.2 item 3). |

## 7. Verification log

- Task ids: every `T-` id cited in this document was checked against `plans/TASKS.csv` by script (result in section 8).
- Repo paths: every backtick path that starts with `app/`, `data/`, `domain/`, `source-api/`, `core/`, `app/src/main/java/eu/kanade/presentation/` or `presentation-core/` was checked for existence under `/home/blackbox/aniyomi-src` (result in section 8).
- Spot checks that passed in this pass: `TrackerManager.kt` tracker ids (`app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:20-36`); `PlayerActivity.kt` line count (1413); torrent service file present at `app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt`; manga extension loader constants and update gate, as stated in area 18 section 8.
- Not verified in this pass: every row's Kotlin behaviour. The owning area documents are the evidence. Where a row cites an area section, that section is the evidence.
- Placeholder scan: run after assembly; see section 8.

## 8. Checks run on this file

| Check | Method | Result |
|---|---|---|
| Task ids exist | Every `T-<area><letter>-<nnn>` token (449 distinct ids) compared with `plans/TASKS.csv` | 0 missing. All 179 range endpoints exist. |
| Kotlin and resource paths exist | Every backtick path starting with `app/`, `data/`, `domain/`, `source-api/`, `core/`, `presentation/`, `presentation-core/` or `source-local/` (115 distinct) checked under `/home/blackbox/aniyomi-src` | 0 missing. |
| Markdown tables | Column count of every table row compared with its header | 0 mismatches. |
| Placeholder words | Search for TBD, TODO, FIXME, "etc.", "and so on", "see source", "similar to", "family", "0xx", "..." | 0 found. |
| Matrix row count | Rows with ids PB-01 to LS-10 style | 393 rows across 21 groups (section 1). |

Rust module paths (`src/player/...`, `src/...`) are proposals and are not checked against the disk, because the Kurobayomi crate does not yet contain them.
