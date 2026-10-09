# area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin, part 1: implementation tasks

Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/33-tracker-enhanced-kavita-komga-suwayomi-jellyfin/p1-tasks.md`
Repo root for Sources: `/home/blackbox/aniyomi-src`. Docs: `p1-inventory.md` (files, task ids), `p1-behavior.md` (D-nn defects, algorithms), `p1-data-and-config.md` (DTOs, endpoints, ids), `p1-rust-design.md` (types, decisions in section 10), `p1-edge-cases-and-tests.md` (E-nn), topics `p1-topic-*.md`.

Priority note: these trackers are not needed to read or watch, so the parity tasks are P1. Cosmetic items are P2. Android-only plumbing is P3 with its replacement stated.

## A. Shared framework for the four trackers

### T-33a-001 — Enhanced tracker module skeleton and HTTP helper
- Priority: P1
- Effort: S
- Depends: T-27a-001, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedMangaTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt
- Acceptance:
  - `crates/kuro-track/src/trackers/{mod.rs,enhanced.rs,http.rs,sources.rs}` exist and `cargo check` passes.
  - `http.rs` builds the tracker client from the shared client with system DNS and no HTTP cache (see T-33a-004).
  - Test: `grep -R "unwrap()\|expect(" crates/kuro-track/src/trackers` finds none outside `#[cfg(test)]`.

### T-33a-002 — Source identity generator and regression vectors
- Priority: P1
- Effort: S
- Depends: area:15-source-api-manga, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - `crates/kuro-sources/src/identity.rs::generate_source_id(name, lang, version_id) -> i64` implements `MD5("<lowercase name>/<lang>/<version>")`, first 8 bytes big-endian, `& i64::MAX`.
  - Tests E-103 to E-112 pass: `kavita_1/all/1` = 7266875667263635239; `jellyfin (10)/all/1` = 3559930647551045450; `tachidesk/en/1` = 3100117499901280806.
  - Proptest: result is never negative.

### T-33a-003 — SourceDirectory and SourcePrefs traits
- Priority: P1
- Effort: S
- Depends: T-33a-001, area:17-source-app-layer, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/ConfigurableSource.kt
- Acceptance:
  - `trait SourceDirectory { fn source_prefs(&self, source_id: i64) -> Option<Arc<dyn SourcePrefs>> }` and `trait SourcePrefs { fn get_string(&self, key, default) -> String }` exist.
  - An in-memory implementation exists in `kuro-testkit` (27 helper) for tests.
  - Missing source returns `None`, never panics (D-05, D-17 precondition).

### T-33a-004 — System DNS and no HTTP cache for tracker clients
- Priority: P1
- Effort: S
- Depends: area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt
- Acceptance:
  - All four tracker clients resolve with the system resolver (no DoH) regardless of the DoH preference.
  - Kavita authenticate uses the same client as the other Kavita calls (fixes D-04).
  - Test: with a DoH provider set, the tracker client's resolver is the system one (unit test on the builder config).
  - Tracker requests never use an HTTP response cache (checked by a test that sends two identical GETs and sees two server hits).

### T-33a-005 — EnhancedError and mapping to TrackError
- Priority: P1
- Effort: S
- Depends: T-33a-001, T-27a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinApi.kt
- Acceptance:
  - `EnhancedError` has the variants in `p1-rust-design.md` section 3.3 (Unauthorized, ServerError, SourceNotInstalled, SourceNotConfigured, SearchUnsupported, MissingFragment, ChapterNotOnServer, EpisodeNotFound, Decode, Http).
  - `From<EnhancedError> for TrackError` maps `Unauthorized` to `TrackError::Unauthorised` and `Http` to `TrackError::Http`; others to `TrackError::Message`.
  - Message keys `track.enhanced.*` (six keys in `p1-rust-design.md` section 7.2) are listed for the i18n crate (area 51).

### T-33a-006 — Change EnhancedTracker match signature to EntryRef
- Priority: P1
- Effort: S
- Depends: T-27a-012, T-27a-033, T-27a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedMangaTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt
- Acceptance:
  - `match_entry(&self, entry: &EntryRef)` replaces `(title, entry_id)` in the 27 traits (`p1-rust-design.md` section 3.1). The 27 bind path builds `EntryRef { id, source_id, url, title }`.
  - Owner of area 27 signs off the signature change (noted in the 27 design doc).
  - Test: Kavita `match` uses `entry.url`, not `entry.title`.

### T-33a-007 — accepts, is_track_from and migrate helpers
- Priority: P1
- Effort: S
- Depends: T-33a-001, T-27a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedMangaTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/Suwayomi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt
- Acceptance:
  - `accepts(accepted, identity)` is exact string equality.
  - `is_track_from(remote_url, entry_url, identity, accepted)` = `remote_url == entry_url && accepts(...)`.
  - `migrate_remote_url` returns `Some(new_url)` only for an accepted identity.
  - Tests E-96, E-97, E-98.

### T-33a-008 — Login flag replaces dummy credentials
- Priority: P1
- Effort: S
- Depends: T-27a-003, T-27a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/Suwayomi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt
- Acceptance:
  - `loginNoop()` sets `enhanced_tracker_enabled_<id>` = true; `logout()` clears it. No secret is written (Proposal, `p1-data-and-config.md` section 5.2).
  - `is_logged_in()` reads the flag for the four trackers.
  - Tests E-101, E-102.
  - Backup import: a backup with `pref_mangasync_username_<id>` = "user" still restores logged-in state (checked with area 21/22).

### T-33a-009 — Enhanced services rows in Settings > Tracking
- Priority: P1
- Effort: M
- Depends: T-27a-024, T-27a-044, T-33a-003, T-33a-008
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt, i18n/src/commonMain/moko-resources/base/strings.xml, /home/blackbox/kurobayomi/spec/settings.json
- Acceptance:
  - The "Enhanced trackers" group lists only trackers whose accepted identity has an installed source (`SettingsTrackingScreen.kt:116-135`).
  - Rows not installed are named in an info row using `enhanced_services_not_installed` with names joined by ", ".
  - The info row begins with `enhanced_tracking_info` followed by a blank line.
  - The mockup's hard-coded four rows in `spec/settings.json` are replaced by runtime rows.
  - Test E-99, E-100.

## B. Jellyfin (anime, id 102)

### T-33a-010 — Jellyfin DTOs
- Priority: P1
- Effort: S
- Depends: T-33a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/dto/JFItem.kt
- Acceptance:
  - `JFItem { name: "Name", id: "Id", user_data: "UserData", index_number: "IndexNumber" (Option<i64>) }`, `JFUserData { played: "Played" }`, `JFItemList { items: "Items" }` with serde renames.
  - Test E-86 (movie has no IndexNumber), E-87 (UserData required).

### T-33a-011 — Tracking URL parser (movie, season, missing fragment)
- Priority: P1
- Effort: S
- Depends: T-33a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt
- Acceptance:
  - `TrackingUrl::parse(url)` returns `Movie` for fragment starting "movie", `Season { series_id }` for "seriesId,{s}", `MissingFragment` for no fragment (E-67), `Decode` otherwise.
  - User id = `pathSegments[1]`; item id = last path segment.
  - Test E-67.

### T-33a-012 — Item read for movies and seasons
- Priority: P1
- Effort: S
- Depends: T-33a-011, T-33a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinApi.kt
- Acceptance:
  - `get_track_search` fetches the item (`J1`), builds the search result with `total_episodes = 1`, `last_episode_seen = 1.0` (played) or `0.0`, and status COMPLETED or UNSEEN (behaviour 5.3).
  - Season URLs also run the episode walk (T-33a-015).
  - `tracking_url` is the input URL unchanged.

### T-33a-013 — Jellyfin producer fixture contract
- Priority: P1
- Effort: S
- Depends: T-33a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt
- Acceptance:
  - A fixture set exists for movie, season, and unsorted seasons (`p1-edge-cases-and-tests.md` section 9).
  - The producer's real tracking URL form is confirmed with the Jellyfin extension owner; the chosen form is recorded in `p1-data-and-config.md` section 5.
  - Test E-115 is recorded as closed or open.

### T-33a-014 — Episode query builder and path handling
- Priority: P1
- Effort: M
- Depends: T-33a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinApi.kt
- Acceptance:
  - Request is `GET {scheme}://{host}[prefix]/Shows/{seriesId}/Episodes?seasonId=…&userId=…&Fields=Overview,MediaSources&SortBy=IndexNumber&SortOrder=Ascending` (D-14 fix).
  - A reverse-proxy prefix in the server URL is kept (D-13 fix). Test E-81.
  - Test: a URL with `/jf/Users/U/Items/S` queries `/jf/Shows/…`.

### T-33a-015 — Season progress with the continuous rule
- Priority: P1
- Effort: M
- Depends: T-33a-014, T-33a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinApi.kt
- Acceptance:
  - Implements the three-case table in `p1-topic-jellyfin-progress-model.md` section 3 (UNSEEN / WATCHING / COMPLETED).
  - Total episodes = maximum `IndexNumber` (D-14 fix). Empty list returns `Decode` (E-90).
  - Position-based "episode before first unwatched" with null `IndexNumber` returns `Decode` (E-73).
  - Tests E-70, E-71, E-72, E-74, E-75.

### T-33a-016 — Mark progress: range marking, movie rule, UTC date
- Priority: P1
- Effort: M
- Depends: T-33a-014, T-33a-020, T-33a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinApi.kt
- Acceptance:
  - Season: marks each unplayed episode with `IndexNumber <= last_episode_seen + 0.001` by `POST /Users/{u}/PlayedItems/{id}?DatePlayed=…` (D-11 proposal). Strict mode (`JELLYFIN_MARK_RANGE = false`) marks only the equal index.
  - No matching episode: `Ok`, INFO log, no POST (D-12).
  - Movie: POST only when `last_episode_seen >= 1.0` (D-32).
  - `DatePlayed` is UTC in `yyyy-MM-ddTHH:mm:ss.sssZ` (D-10). Test E-80.
  - POSTs run with concurrency 4 (`buffer_unordered`).
  - Tests E-68, E-69, E-76, E-77, E-78, E-79, E-89.

### T-33a-020 — JellyfinAuth: user id to api key lookup
- Priority: P1
- Effort: S
- Depends: T-33a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinInterceptor.kt
- Acceptance:
  - Scans slots 1..10 with names from `jellyfin_name(n)` (`"jellyfin"` for 1, `"jellyfin (n)"` otherwise).
  - Missing slots are skipped (D-17). Empty `api_key` returns `SourceNotConfigured` and is not cached (D-16).
  - No cache (reads prefs per call), so no shared mutable map (D-15).
  - Tests E-82, E-83, E-84, E-85.

### T-33a-021 — Jellyfin request authentication and User-Agent
- Priority: P1
- Effort: S
- Depends: T-33a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt
- Acceptance:
  - Every request carries `User-Agent: Aniyomi v{VERSION} ({APPLICATION_ID})` (Proposal: the Rust app's own UA string, documented).
  - The key is sent as header `X-Emby-Token` (Proposal, pitfall P-8). If a test server rejects the header, fall back to query `api_key` for that request only.
  - Logs never contain the key.

### T-33a-022 — Missing-login error text
- Priority: P1
- Effort: S
- Depends: T-33a-005, T-33a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinInterceptor.kt
- Acceptance:
  - `SourceNotConfigured` maps to message key `track.enhanced.login_required` ("Please log in through the extension").
  - The English text is shown only through the i18n key, never as a raw exception.

### T-33a-023 — Drop the OkHttp interceptor chain (P3 replacement)
- Priority: P3
- Effort: S
- Depends: T-33a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt
- Acceptance:
  - Decision recorded: the interceptor classes are not ported. Their work is done by explicit request builders that take an auth context (`p1-rust-design.md` section 11).
  - Nothing else is needed; this task closes once no `Interceptor` type exists in `crates/kuro-track`.

### T-33a-030 — JellyfinTracker identity, status constants and labels
- Priority: P1
- Effort: S
- Depends: T-33a-001, T-27a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt
- Acceptance:
  - `UNSEEN = 1`, `WATCHING = 2`, `COMPLETED = 3`, rewatching `-1`, watching `WATCHING`, completion `COMPLETED`.
  - `status_list() = [1, 2, 3]`; labels: 1 → `unseen`, 2 → `watching`, 3 → `completed`; others `None`.
  - Score list empty; `display_score` = "".

### T-33a-031 — Jellyfin logo asset and colour
- Priority: P1
- Effort: S
- Depends: T-33a-100
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt, app/src/main/res/drawable-nodpi/ic_tracker_jellyfin.webp
- Acceptance:
  - Logo colour is `rgb(0, 11, 37)` (`#000B25`).
  - Logo asset is referenced from the GResource bundle (T-33a-100).

### T-33a-032 — Jellyfin update and refresh
- Priority: P1
- Effort: S
- Depends: T-33a-016, T-33a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt
- Acceptance:
  - `update(track, did_watch)` calls `update_progress` and ignores `did_watch` (D-31).
  - `refresh` copies only `last_episode_seen`, `status`, `total_episodes` from the remote. Keeps `score`, `started_watching_date`, `finished_watching_date`, `private` (D-03 fix).
  - Test E-91 (framework) and a Jellyfin-specific test that score survives refresh.

### T-33a-033 — Jellyfin bind and search
- Priority: P1
- Effort: S
- Depends: T-33a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt
- Acceptance:
  - `bind` returns the track unchanged.
  - `search` returns `EnhancedError::SearchUnsupported` (D-21), not `Exception("Not used")`.

### T-33a-034 — Jellyfin match, is_track_from, migrate
- Priority: P1
- Effort: S
- Depends: T-33a-007, T-33a-006, T-33a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt
- Acceptance:
  - `match(entry)` fetches `entry.url`, returns the result, and returns `Err` on failure (D-29: no silent null).
  - `is_track_from` compares `remote_url == entry.url` and accepted identity `jellyfin/all`.
  - `migrate_track` copies `remote_url` for an accepted new source.
  - Test E-94, E-95 for the caller behaviour.

### T-33a-035 — Jellyfin accepted source identity and login
- Priority: P1
- Effort: S
- Depends: T-33a-002, T-33a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt
- Acceptance:
  - Accepted identity is `jellyfin/all`, mapping to Kotlin `eu.kanade.tachiyomi.animeextension.all.jellyfin.Jellyfin`.
  - `login` and `login_noop` set the flag (T-33a-008).

### T-33a-036 — Jellyfin status seeds from the played flag
- Priority: P1
- Effort: S
- Depends: T-33a-012, T-33a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinApi.kt
- Acceptance:
  - Movie: played → COMPLETED with `last = 1.0`; unplayed → UNSEEN with `last = 0.0`.
  - Season: status from the continuous rule (T-33a-015), not from the season's own flag (P-10).
  - Test: a season whose own `Played` flag is true but whose episodes are not watched is UNSEEN.

### T-33a-037 — Jellyfin wire and rule tests
- Priority: P1
- Effort: M
- Depends: T-33a-014, T-33a-015, T-33a-016, T-33a-020, T-33a-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinInterceptor.kt
- Acceptance:
  - All tests E-67 to E-90 pass against `wiremock` with the fixtures of `p1-edge-cases-and-tests.md` section 9.
  - Test count for the Jellyfin module is at least 24.

## C. Kavita (manga, id 8)

### T-33a-040 — KavitaTracker identity, constants and labels
- Priority: P1
- Effort: S
- Depends: T-33a-001, T-27a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt
- Acceptance:
  - `UNREAD = 1`, `READING = 2`, `COMPLETED = 3`, rereading `-1`, reading `READING`, completion `COMPLETED`.
  - Labels: 1 → `unread`, 2 → `reading`, 3 → `completed`.
  - Score list empty; display score "".

### T-33a-041 — Kavita logo asset and colour
- Priority: P1
- Effort: S
- Depends: T-33a-100
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt, app/src/main/res/drawable-nodpi/ic_tracker_kavita.webp
- Acceptance:
  - Colour `rgb(74, 198, 148)` (`#4AC694`).
  - Asset from the GResource bundle.

### T-33a-042 — Kavita update: write, then take the server status
- Priority: P1
- Effort: S
- Depends: T-33a-056, T-33a-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/SyncChapterProgressWithTrack.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt
- Acceptance:
  - The Kotlin local status branch is removed (D-02). `update` writes progress and returns the refreshed track.
  - The caller persists the returned track's `status`, `last_chapter_read` and `total_chapters`.
  - Test E-92: a locally computed READING is overwritten by the server's COMPLETED.

### T-33a-043 — Kavita refresh keeps personal fields
- Priority: P1
- Effort: S
- Depends: T-33a-055, T-33a-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt
- Acceptance:
  - Refresh copies `status`, `last_chapter_read`, `total_chapters` only (D-03 fix).
  - Test E-91.

### T-33a-044 — Kavita match, bind and search
- Priority: P1
- Effort: S
- Depends: T-33a-055, T-33a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt
- Acceptance:
  - `match(entry)` = `get_track_search(entry.url)`; errors returned (D-29).
  - `bind` returns the track unchanged.
  - `search` returns `SearchUnsupported` (D-21), without a panic.
  - Test E-93, E-66-style check for Kavita.

### T-33a-045 — Kavita accepted identity, is_track_from and migrate
- Priority: P1
- Effort: S
- Depends: T-33a-007, T-33a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt
- Acceptance:
  - Accepted identity `kavita/all` for `eu.kanade.tachiyomi.extension.all.kavita.Kavita`.
  - `is_track_from` and `migrate_track` as in T-33a-007.

### T-33a-046 — Kavita login and logout
- Priority: P1
- Effort: S
- Depends: T-33a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt
- Acceptance:
  - `login` and `login_noop` set the enhanced flag. `logout` clears it.
  - Settings row login calls `loginNoop` (T-33a-009).

### T-33a-047 — KavitaAuth: three JWT slots, isolation and invalidation
- Priority: P1
- Effort: M
- Depends: T-33a-003, T-33a-002, T-33a-052
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaModels.kt
- Acceptance:
  - Table of 3 slots `{api_url, jwt}`, loaded once under a `Mutex` single-flight gate (E-08).
  - A failing slot (401, 500, timeout, missing source, empty token) is skipped and does not stop others (D-05 fix, E-02, E-10).
  - `token_for(api_base)` returns the JWT of the slot whose `api_url == api_base`, else `Unauthorized` (E-01, E-12).
  - `invalidate(api_base)` clears a slot; the API retries once after a 401 (D-07 fix, E-09).

### T-33a-048 — Kavita request headers
- Priority: P1
- Effort: S
- Depends: T-33a-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaInterceptor.kt
- Acceptance:
  - Every Kavita request has `Authorization: Bearer <jwt>` and `User-Agent: Aniyomi v{VERSION} ({APPLICATION_ID})`.
  - No header value is built from an absent token (no `Bearer null`).

### T-33a-050 — Kavita URL helpers
- Priority: P1
- Effort: S
- Depends: T-33a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt
- Acceptance:
  - `api_base(url) = url.split("/api/").first() + "/api"` (Kotlin parity, works behind a prefix).
  - `series_id(url)` parses the last non-empty path segment as `i32` (D-30 fix). Tests E-35, E-36, E-37.

### T-33a-051 — Kavita DTOs
- Priority: P1
- Effort: S
- Depends: T-33a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaModels.kt
- Acceptance:
  - `SeriesDto`, `VolumeDto`, `ChapterDto`, `AuthenticationDto` match the field tables in `p1-data-and-config.md` section 2.1, with `#[serde(default)]` only where Kotlin has a default.
  - Test E-66 (`pagesRead` required), E-15 (`thumbnail_url` absent).

### T-33a-052 — Authenticate call
- Priority: P1
- Effort: M
- Depends: T-33a-051, T-33a-050, T-33a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt
- Acceptance:
  - `POST {api}/Plugin/authenticate?apiKey=…&pluginName=Tachiyomi-Kavita` with body `{}` and `Content-Type: application/json; charset=utf-8`. The key is query-encoded (E-11).
  - 200 → token; 401 → `Unauthorized`; 500 → `ServerError`; timeout → `Ok(None)`; other → `Ok(None)`; bad JSON → `Decode`.
  - Log text matches `p1-behavior.md` section 7; no key in logs (E-14).
  - Tests E-03, E-04, E-05, E-06, E-07, E-13.

### T-33a-053 — Total chapters from the volume walk
- Priority: P1
- Effort: M
- Depends: T-33a-051, T-33a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt
- Acceptance:
  - Implements the walk in `p1-behavior.md` section 2.6 with the guards of D-23: empty volume skipped (E-25), non-numeric number is `Decode` (E-26), null number is `-1` (E-27).
  - Truncation of the chapter max matches `Float.toLong()` (E-22).
  - Tests E-21, E-23, E-24.

### T-33a-054 — Latest chapter read
- Priority: P1
- Effort: S
- Depends: T-33a-051, T-33a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt
- Acceptance:
  - 200 → parse with `,` replaced by `.`, clamped to `>= 0.0` (D-22). 204 → 0.0. Any other status → 0.0 silently.
  - Tests E-28, E-29, E-30, E-31.
  - A 204 response never calls `.json()` (P-11).

### T-33a-055 — Series read: get_track_search
- Priority: P1
- Effort: M
- Depends: T-33a-053, T-33a-054, T-33a-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaModels.kt
- Acceptance:
  - Series GET, then `try_join!(total, latest)`.
  - `cover_url` is `""` when absent or null (D-01). Proposal of a derived cover is a P2 follow-up (T-33a-107).
  - Status via `kavita_status`: `pagesRead == 0` → UNREAD first (D-24), then `== pages` → COMPLETED, else READING.
  - Tests E-15, E-16, E-17, E-18, E-19, E-20.

### T-33a-056 — Mark progress: update_progress
- Priority: P1
- Effort: S
- Depends: T-33a-052, T-33a-055
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt
- Acceptance:
  - `POST {api}/Tachiyomi/mark-chapter-until-as-read?seriesId=…&chapterNumber=<Kotlin Double string>` with body `{}` and JSON content type.
  - Double string follows Kotlin `Double.toString()` for `[0, 1e7)` (pitfall P-6).
  - After the POST, the track is re-read with `get_track_search`.
  - Tests E-32, E-33, E-34.

### T-33a-057 — Kavita wire and auth tests
- Priority: P1
- Effort: M
- Depends: T-33a-047, T-33a-048, T-33a-052, T-33a-053, T-33a-054, T-33a-055, T-33a-056
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaInterceptor.kt
- Acceptance:
  - Tests E-01 to E-37 pass with `wiremock` and fixtures from `p1-edge-cases-and-tests.md` section 9.
  - Concurrency test E-08 (32 tasks, one authenticate per slot).

## D. Komga (manga, id 6)

### T-33a-060 — KomgaTracker identity, constants and labels
- Priority: P1
- Effort: S
- Depends: T-33a-001, T-27a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt
- Acceptance:
  - `UNREAD = 1`, `READING = 2`, `COMPLETED = 3`, rereading `-1`, reading `READING`, completion `COMPLETED`.
  - Labels as Kavita. Score list empty.

### T-33a-061 — Komga client
- Priority: P1
- Effort: S
- Depends: T-33a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaApi.kt
- Acceptance:
  - Client uses system DNS (T-33a-004) and sends `User-Agent: Aniyomi v{VERSION} ({APPLICATION_ID})` on every request (`KomgaApi.kt:28-32`).
  - No interceptor is installed.

### T-33a-062 — Komga authentication decision and implementation
- Priority: P1
- Effort: M
- Depends: T-33a-061, T-27a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaApi.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt
- Acceptance:
  - Decision from `p1-rust-design.md` section 10.1 is recorded (recommended: Basic auth with username and password stored in `SecretStore`, entered in the tracker login dialog).
  - Requests send the credentials. A 401 returns `Unauthorized`.
  - Test E-49.
  - Manual check on a live Komga before this task closes (E-114).

### T-33a-063 — Komga update and refresh
- Priority: P1
- Effort: S
- Depends: T-33a-071, T-33a-072, T-33a-060
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt
- Acceptance:
  - Dead status branch removed (D-02); `update` returns the refreshed track.
  - Refresh copies `status`, `last_chapter_read`, `total_chapters` only (D-03 fix).
  - Test E-91-style for Komga, E-92.

### T-33a-064 — Komga match, bind, search, identity, migrate, login
- Priority: P1
- Effort: S
- Depends: T-33a-007, T-33a-008, T-33a-070, T-33a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt
- Acceptance:
  - `match` = `get_track_search(entry.url)`, errors returned (D-29).
  - `bind` no-op. `search` → `SearchUnsupported` (Kotlin `Exception("Not used")`).
  - Accepted identity `komga/all` for `eu.kanade.tachiyomi.extension.all.komga.Komga`.
  - `is_track_from` and `migrate_track` as T-33a-007.
  - `login` and `login_noop` set the flag.

### T-33a-065 — Komga logo asset and colour
- Priority: P1
- Effort: S
- Depends: T-33a-100
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt, app/src/main/res/drawable-nodpi/ic_tracker_komga.webp
- Acceptance:
  - Colour `rgb(51, 37, 50)` (`#332532`), asset from the GResource bundle.

### T-33a-070 — Komga series and readlist dispatch and DTOs
- Priority: P1
- Effort: M
- Depends: T-33a-061
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaModels.kt
- Acceptance:
  - `READLIST_API = "/api/v1/readlists"`; URL containing it is a readlist, otherwise a series.
  - `SeriesDto` and its nested DTOs match `p1-data-and-config.md` section 2.2, with required keys required.
  - Series: title = `metadata.title`, summary = `metadata.summary`, publishing_status = `metadata.status`. Readlist: title = `name`.
  - `cover_url = "{url}/thumbnail"`, `tracking_url = url`.
  - Test E-38 (readlist paths), E-51 (required fields).

### T-33a-071 — Komga progress endpoints with v2 rewrite
- Priority: P1
- Effort: M
- Depends: T-33a-070
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaModels.kt
- Acceptance:
  - Progress URL: `{url with "/api/v1/series/" → "/api/v2/series/"}/read-progress/tachiyomi` (replace all occurrences).
  - Parse V2 (`ReadProgressV2Dto`) when the URL is a v1 or v2 series (D-18 fix, E-40). Parse V1 (`ReadProgressDto`) for readlists and convert with `to_v2()`.
  - `total_chapters = maxNumberSort as i64` (truncation, E-45).
  - Test E-39.

### T-33a-072 — Komga progress update payloads
- Priority: P1
- Effort: M
- Depends: T-33a-071
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaModels.kt
- Acceptance:
  - v1 series URL: `PUT …/api/v2/series/{id}/read-progress/tachiyomi` with `{"lastBookNumberSortRead": <f64>}`, `Content-Type: application/json`.
  - Otherwise: `PUT {url}/read-progress/tachiyomi` with `{"lastBookRead": <i32 truncated>}`.
  - Test E-46 (bytes), E-47 (truncation), E-48 (204 then refresh).

### T-33a-073 — Komga status mapping
- Priority: P1
- Effort: S
- Depends: T-33a-071
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaApi.kt
- Acceptance:
  - `komga_status(books_count, unread, read)`: `books_count == unread` → UNREAD first; `== read` → COMPLETED; else READING (D-20 kept).
  - Tests E-41 to E-44.

### T-33a-074 — Komga wire tests
- Priority: P1
- Effort: M
- Depends: T-33a-070, T-33a-071, T-33a-072, T-33a-073, T-33a-062
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt
- Acceptance:
  - Tests E-38 to E-51 pass with `wiremock` and the Komga fixtures.

## E. Suwayomi (manga, id 9)

### T-33a-080 — SuwayomiTracker identity, constants and labels
- Priority: P1
- Effort: S
- Depends: T-33a-001, T-27a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/Suwayomi.kt
- Acceptance:
  - `UNREAD = 1`, `READING = 2`, `COMPLETED = 3`, rereading `-1`, reading `READING`, completion `COMPLETED`.
  - Labels as Kavita. Score list empty.

### T-33a-081 — Suwayomi update and refresh
- Priority: P1
- Effort: S
- Depends: T-33a-093, T-33a-080
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/Suwayomi.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt
- Acceptance:
  - Dead status branch removed (D-02); `update` returns the refreshed track.
  - Refresh copies only remote-owned fields (D-03 fix).

### T-33a-082 — Suwayomi match, search, is_track_from, migrate, identity
- Priority: P1
- Effort: S
- Depends: T-33a-092, T-33a-007, T-33a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/Suwayomi.kt
- Acceptance:
  - `match(entry)` = `get_track_search(entry.url)`, errors returned (D-29).
  - `search` → `SearchUnsupported` (D-21). The Kotlin TODO is replaced.
  - `is_track_from` compares `remote_url == entry.url` and accepted identity (D-09 fix, E-64).
  - Accepted identity `tachidesk/en` for `eu.kanade.tachiyomi.extension.all.tachidesk.Tachidesk`.
  - `migrate_track` as T-33a-007.

### T-33a-083 — Suwayomi login and logout
- Priority: P1
- Effort: S
- Depends: T-33a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/Suwayomi.kt
- Acceptance:
  - `login` and `login_noop` set the flag. `logout` clears it.

### T-33a-084 — Suwayomi logo asset and colour decision
- Priority: P2
- Effort: S
- Depends: T-33a-100
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/Suwayomi.kt, app/src/main/res/drawable-nodpi/ic_tracker_suwayomi.webp
- Acceptance:
  - Colour is sampled from the logo asset (D-27), replacing the Kotlin `rgb(255, 35, 35)` marked `// TODO`. Until sampled, `#FF2323` is used.

### T-33a-085 — Suwayomi status mapping
- Priority: P1
- Effort: S
- Depends: T-33a-092
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/Suwayomi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt
- Acceptance:
  - `suwayomi_status(unread: Option<i64>, chapters: i64)`: `Some(u)` with `u == chapters` → UNREAD first; `Some(0)` → COMPLETED; otherwise READING (null unread → READING).
  - Tests E-54, E-55, E-56, E-57.

### T-33a-090 — Settings read per call and source id
- Priority: P1
- Effort: S
- Depends: T-33a-003, T-33a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt
- Acceptance:
  - Reads "Server URL Address", "Login (Basic Auth)", "Password (Basic Auth)" from `source_<tachidesk/en/1>` on every call (D-08 fix). Test E-63.
  - Keys are the exact strings with spaces and parentheses.

### T-33a-091 — Suwayomi headers and Basic auth
- Priority: P1
- Effort: S
- Depends: T-33a-090, T-33a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt
- Acceptance:
  - `User-Agent` is the user's default UA preference (area 12).
  - Basic auth only when both login and password are non-empty; header equals `Basic base64(login:password)` (E-62).
  - Charset decision recorded (P-3): UTF-8 is the Proposal. Verify the OkHttp default before shipping.

### T-33a-092 — Suwayomi get_track_search
- Priority: P1
- Effort: M
- Depends: T-33a-090, T-33a-091, T-33a-095
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiModels.kt
- Acceptance:
  - Numeric track URL with empty base URL → `SourceNotConfigured` (D-26, E-52).
  - Non-numeric URL used verbatim (E-53).
  - `GET {url}/full`; `cover_url = {url}/thumbnail`; `summary = description ?? ""`; `total_chapters = chapterCount`; `publishing_status = status`; `last_chapter_read = lastChapterRead?.chapterNumber ?? 0.0`; status by T-33a-085.
  - Tests E-58.

### T-33a-093 — Suwayomi update_progress
- Priority: P1
- Effort: M
- Depends: T-33a-090, T-33a-091, T-33a-096
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiModels.kt
- Acceptance:
  - `GET {url}/chapters`; find the first chapter whose `chapterNumber == last_chapter_read` exactly; missing → `ChapterNotOnServer` (D-25, E-60).
  - `PUT {url}/chapter/{index}` with form `markPrevRead=true`, `read=true` (uses `index`, E-59); first match wins (E-61).
  - Returns `get_track_search(tracking_url)`.

### T-33a-094 — Suwayomi wire tests
- Priority: P1
- Effort: M
- Depends: T-33a-092, T-33a-093, T-33a-085
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/Suwayomi.kt
- Acceptance:
  - Tests E-52 to E-66 pass with `wiremock` and Suwayomi fixtures.

### T-33a-095 — Suwayomi MangaDataClass and SourceDataClass DTOs
- Priority: P1
- Effort: S
- Depends: T-33a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiModels.kt
- Acceptance:
  - `MangaDataClass` (26 fields) and `SourceDataClass` (8 fields) match `p1-data-and-config.md` section 2.3: required and nullable sets as listed.
  - Test E-65 (missing required field fails the parse).

### T-33a-096 — Suwayomi ChapterDataClass DTO and index semantics
- Priority: P1
- Effort: S
- Depends: T-33a-095
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiModels.kt
- Acceptance:
  - `ChapterDataClass` (17 fields) matches section 2.3; `index` starts at 1 (per the source comment at `SuwayomiModels.kt:83-84`).
  - `chapterNumber` is `f64`.

## F. Cross-cutting and assets

### T-33a-100 — Tracker logo assets
- Priority: P1
- Effort: S
- Depends: T-33a-001
- Sources: app/src/main/res/drawable-nodpi/ic_tracker_kavita.webp, app/src/main/res/drawable-nodpi/ic_tracker_komga.webp, app/src/main/res/drawable-nodpi/ic_tracker_suwayomi.webp, app/src/main/res/drawable-nodpi/ic_tracker_jellyfin.webp
- Acceptance:
  - The four logos are converted to PNG or SVG and bundled in the GResource file.
  - Lookup by tracker id returns an asset; a missing asset falls back to a text tile, not a crash.

### T-33a-101 — Framework tests for enhanced trackers
- Priority: P1
- Effort: M
- Depends: T-33a-005, T-33a-006, T-33a-007, T-33a-008, T-33a-009
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedMangaTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt
- Acceptance:
  - Tests E-91 to E-102 pass.
  - Bind path (E-94, E-95) and refresh path (E-91) are tested through `kuro-track` public APIs.

### T-33a-102 — Wire test harness
- Priority: P1
- Effort: S
- Depends: T-33a-001, T-33a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt
- Acceptance:
  - `wiremock` is a dev-dependency of `kuro-track`. A helper starts a mock server per test and returns its base URL.
  - Fixtures live in `crates/kuro-track/tests/fixtures/` (list in `p1-edge-cases-and-tests.md` section 9).

### T-33a-103 — Drop: Injekt, injectLazy and lazy wiring (P3)
- Priority: P3
- Effort: S
- Depends: T-33a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaApi.kt
- Acceptance:
  - Replacement recorded: constructor injection through `TrackerDeps` (`p1-rust-design.md` section 11). No service locator is ported.
  - Closes once no `injectLazy` or `Injekt` appears in `crates/kuro-track`.

### T-33a-104 — Drop: Android resource lookups (P3)
- Priority: P3
- Effort: S
- Depends: T-33a-100
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/Suwayomi.kt
- Acceptance:
  - `R.drawable.*`, `Color.rgb(...)` and `MR.strings.*` are replaced by asset keys, a `u32` colour, and message keys (`p1-rust-design.md` section 11).
  - Closes once none of these appear in `crates/kuro-track`.

### T-33a-105 — Coroutine dispatcher mapping (P3)
- Priority: P3
- Effort: S
- Depends: T-27a-035, T-27a-036
- Sources: app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/KomgaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/SuwayomiApi.kt
- Acceptance:
  - `withIOContext` is dropped (async end to end). `withNonCancellableContext` becomes a detached `tokio::spawn` (27 section 4).
  - Kavita blocking `execute()` calls become `await` (no `spawn_blocking`).

### T-33a-106 — Jellyfin season paging (P2)
- Priority: P2
- Effort: S
- Depends: T-33a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinApi.kt
- Acceptance:
  - Episode requests page with `StartIndex` and `Limit=200` until a short page (Proposal, E-88).
  - Test: 150 episodes are all read.

### T-33a-107 — Kavita derived cover (P2)
- Priority: P2
- Effort: S
- Depends: T-33a-055
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaModels.kt
- Acceptance:
  - After the Kavita cover endpoint is confirmed on a live server, `cover_url` is derived from the series id. Until then `""` (D-01).

### T-33a-108 — Kavita volume-encoding decision (P2)
- Priority: P2
- Effort: S
- Depends: T-33a-054, T-33a-053
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt
- Acceptance:
  - Decision recorded for E-113 (decode `volume * 10000` or keep raw). Default: keep raw (parity). Test E-113 asserts the chosen behaviour.

### T-33a-109 — Kavita token renewal check on a live server (P2)
- Priority: P2
- Effort: S
- Depends: T-33a-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/KavitaInterceptor.kt
- Acceptance:
  - Live check (E-116): an expired JWT returns 401 (not a redirect). The retry-once rule in T-33a-047 is confirmed or changed.

### T-33a-110 — Manual verification runbook (P2)
- Priority: P2
- Effort: S
- Depends: T-33a-057, T-33a-074, T-33a-094, T-33a-037
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/Suwayomi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt
- Acceptance:
  - The four checks in `p1-edge-cases-and-tests.md` section 11 are run and their results recorded in the area notes.

### T-33a-111 — Enhanced trackers: release notes text (P2)
- Priority: P2
- Effort: S
- Depends: T-33a-101
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - The known-difference list (D-02, D-03, D-09, D-10, D-11, D-13, D-14, D-24) is written as user-facing release text for the Rust port's first release, in plain English.
