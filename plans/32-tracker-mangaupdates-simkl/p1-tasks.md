# Area 32: MangaUpdates + Simkl trackers, part 1 - implementation tasks

Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/32-tracker-mangaupdates-simkl/p1-tasks.md`

Repo root for Sources: `/home/blackbox/aniyomi-src`. Behaviour refs `B-x.y` = `p1-behavior.md`; wire refs `D-x.y` = `p1-data-and-config.md`; design refs `R-x.y` = `p1-rust-design.md`; UI refs `U-x` = `p1-ui-parity.md`; tests `TST-n` = `p1-edge-cases-and-tests.md`.

Shared prefixes used in the Sources lists below (all relative to the repo root):
- MU = `app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/`
- SK = `app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/`
- SET = `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt`

Sections: A shared (001-004); B MangaUpdates (010-026); C Simkl (030-049); D cross-cutting (050-072).

## A. Shared

### T-32a-001 — Tracker id constants and module skeleton
- Priority: P1
- Effort: S
- Depends: area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt
- Acceptance:
  - `crates/kuro-track/src/trackers/{mod.rs,mangaupdates/mod.rs,simkl/mod.rs}` exist and `cargo check -p kuro-track` passes.
  - `TrackerId::MANGAUPDATES` is 7 and `TrackerId::SIMKL` is 101 (R-1, B-1.1).
  - `all_trackers(ctx)` returns both services in registry order used by `TrackerManager.trackers` (MangaUpdates before Simkl).

### T-32a-002 — Shared credential and login-state behaviour for both trackers
- Priority: P1
- Effort: M
- Depends: T-32a-001, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - `is_logged_in` is true iff username and password are both non-empty (B-1.2).
  - `save_credentials` and `logout` set `auth_expired` to false (B-1.2, TrackPreferences.kt:29-33).
  - `login_state()` emits true after successful login and false after logout (TST-66).

### T-32a-003 — Score table type and index-to-score helper
- Priority: P1
- Effort: S
- Depends: T-32a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt
- Acceptance:
  - `ScoreList` is built once per tracker and returned by reference from `score_list()` (R-12 pitfall 1).
  - Out-of-range index returns `TrackError::ScoreOutOfRange` in the module helper (TST-2).

### T-32a-004 — Track model mapping for both services (DB and domain)
- Priority: P1
- Effort: S
- Depends: T-32a-001, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/track/manga/model/MangaTrack.kt, app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt
- Acceptance:
  - `copy_personal` copies the six fields listed in D-1.1 and does not copy `total_*`, `title`, `remote_url` (matches `copyPersonalFrom`).
  - `remote_url` is `""` (never null) for both trackers (TST-70).
  - Conversions to and from the domain model round-trip all fields (unit test).

## B. MangaUpdates

### T-32a-010 — MangaUpdates list ids, status order and labels
- Priority: P1
- Effort: S
- Depends: T-32a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt
- Acceptance:
  - Constants 0, 1, 2, 3, 4 and rereading -1 exactly as B-2.1 and D-3.1.
  - `STATUS_ORDER` is `[0, 2, 4, 3, 1]` (Reading, Complete, On hold, Unfinished, Wish).
  - Labels resolve to keys `reading_list`, `wish_list`, `complete_list`, `on_hold_list`, `unfinished_list`; unknown ids give none.
  - Logo colour is `0xFF92A0AD` (B-2.1).

### T-32a-011 — MangaUpdates HTTP client with bearer auth and user agent
- Priority: P1
- Effort: S
- Depends: T-32a-001, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt
- Acceptance:
  - Authenticated calls send `Authorization: Bearer <token>` and the Kurobayomi User-Agent (B-2.6, TST-24).
  - Missing token returns `NotAuthenticated("MangaUpdates")` before any request (TST-18).
  - Unauthenticated calls (search, login) do not send the Authorization header.
  - No HTTP cache is used for these hosts (B-6.3).

### T-32a-012 — MangaUpdates wire DTOs
- Priority: P1
- Effort: S
- Depends: T-32a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUContext.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUImage.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUListItem.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MULoginResponse.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MURating.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MURecord.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUSearch.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUSeries.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUStatus.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUUrl.kt
- Acceptance:
  - Field names and nullability match D-4.1 (`list_id`, `session_token`, `series_id`, `bayesian_rating`, `rating_votes`, `latest_chapter`, `type`).
  - Missing nullable keys parse as `None` (`#[serde(default)]`); `MULoginResponse` without `context` gives a parse error (TST-17).
  - Fixture round-trip tests pass for every fixture in p1-edge-cases-and-tests.md section 7.

### T-32a-013 — MangaUpdates login and credential storage
- Priority: P1
- Effort: M
- Depends: T-32a-002, T-32a-012, T-32a-065
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt
- Acceptance:
  - PUT `/v1/account/login` with `{"username","password"}` (D-4.1 M9).
  - On success the username stored is the uid as decimal string and the password is the session token; the typed password is not stored (TST-15).
  - On failure no credentials are stored and the error text is `HTTP <code>` (TST-16).

### T-32a-014 — MangaUpdates session restore and logout clears the token
- Priority: P1
- Effort: S
- Depends: T-32a-011, T-32a-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesInterceptor.kt
- Acceptance:
  - Token is restored from the secret store at startup (R-7 key `track.7.session`).
  - `logout()` clears the in-memory token; the next list call fails without a request (fixes D-MU-1, TST-18).
  - `logout()` callable from the GTK thread (uses a `std::sync::RwLock`, R-4.7).

### T-32a-015 — MangaUpdates bind with list lookup and add fallback
- Priority: P1
- Effort: M
- Depends: T-32a-011, T-32a-012, T-32a-016, T-32a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt
- Acceptance:
  - Bind copies status (`list_id` or 0), `last_chapter_read` (`status.chapter` or 0.0), and score (B-2.7, `MUListItem.copyTo`).
  - Only HTTP 404 on the list-item GET leads to the add path; other errors propagate with no add request (fix D-MU-2, TST-8).
  - Score is set to the remote rating when known, kept when the rating fetch failed (TST-7).

### T-32a-016 — MangaUpdates add to list
- Priority: P1
- Effort: S
- Depends: T-32a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt
- Acceptance:
  - POST `/v1/lists/series` with body `[{"series":{"id":<id>},"list_id":<0|1>}]`, byte-compared in TST-9.
  - List is 0 when has-read is true, 1 otherwise.
  - `status` is written only on success code 200 (B-2.9); `last_chapter_read = 1.0` only when has-read is true (fix D-MU-4).

### T-32a-017 — MangaUpdates update list item and rating sync
- Priority: P1
- Effort: M
- Depends: T-32a-011, T-32a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt
- Acceptance:
  - POST `/v1/lists/series/update` body has keys in order `series`, `list_id`, `status.chapter` with truncated chapter (TST-5).
  - Rating sequence after the list update: skip when score < 0; PUT `{"rating":<score>}` when score is non-zero; DELETE when score is 0.0 and remote rating is known to exist (TST-6).
  - Any non-2xx response stops the sequence and returns the error.

### T-32a-018 — MangaUpdates rating tri-state (fix for score 0.0 on fetch failure)
- Priority: P1
- Effort: S
- Depends: T-32a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt
- Acceptance:
  - `RatingFetch` has `Known(Option<f64>)` and `Unknown` (R-4.1).
  - A failed rating GET gives `Unknown`, never 0.0 (TST-7).
  - DELETE is not sent when the last known remote rating is absent (TST-6, decision O-3 recorded in the row).

### T-32a-019 — MangaUpdates delete from list (DeletableMangaTracker)
- Priority: P1
- Effort: S
- Depends: T-32a-011, T-32a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/DeletableMangaTracker.kt
- Acceptance:
  - POST `/v1/lists/series/delete` with body `[<remote_id>]` (TST-19).
  - Errors propagate; no local state is changed by this function.

### T-32a-020 — MangaUpdates refresh
- Priority: P1
- Effort: S
- Depends: T-32a-015, T-32a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt
- Acceptance:
  - Refresh copies status, last chapter and score from remote and keeps title, remote id and URL (B-2.13).
  - List-item failure propagates; rating failure keeps the previous score (TST-20).

### T-32a-021 — MangaUpdates search and result mapping
- Priority: P1
- Effort: M
- Depends: T-32a-011, T-32a-012, T-32a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MURecord.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt
- Acceptance:
  - POST `/v1/series/search` body is exactly `{"search":"<q>","filter_types":["drama cd","novel"]}` (TST-11).
  - Record mapping per B-2.14: `total_chapters = 0`, cover from `image.url.original`, `tracking_url = url`, `score = None`.
  - Null `type` and `year` give empty strings, not `"null"` (fix D-MU-5, TST-12).
  - Search runs on the unauthenticated client.

### T-32a-022 — MangaUpdates update rule and generic setter integration
- Priority: P1
- Effort: S
- Depends: T-32a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt
- Acceptance:
  - `update(track, did_read)` sets status to reading only when status is not complete and did_read is true (TST-10).
  - Generic `setRemoteLastChapterRead` and `setRemoteScore` work through this update (B-1.3); `setRemoteScore` with an unknown string is rejected before any request.

### T-32a-023 — MangaUpdates HTML decoding parity for title and description
- Priority: P1
- Effort: S
- Depends: T-32a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MURecord.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - Tags are removed and entities decoded per R-2 (`ammonia` + `html-escape`).
  - All 12 cases in the `htmldecode_cases.json` fixture match the Kotlin output or are listed as known differences (TST-14).

### T-32a-024 — MangaUpdates login dialog (username and password)
- Priority: P1
- Effort: M
- Depends: T-32a-013, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Dialog matches U-2: title with service name, username and password entries, Login button enabled only when both are non-blank, spinner while processing.
  - Success toast `login_success` and close; failure keeps dialog open with error text and no `"null"` toast (fix D-MU-9).
  - Password entry starts masked with a show/hide toggle; Enter in password submits; Escape closes.

### T-32a-025 — MangaUpdates logo asset
- Priority: P2
- Effort: S
- Depends: T-32a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt
- Acceptance:
  - Embedded SVG for `ic_manga_updates` is shown at 24 px in the settings row and 48 px in search (U-1, U-6).

### T-32a-026 — MangaUpdates score display and picker
- Priority: P1
- Effort: S
- Depends: T-32a-003, T-32a-072
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt
- Acceptance:
  - Picker has 92 entries in the order of B-2.2.
  - Displayed score is `-` for 0.0 and `Double` text otherwise (`7.5`, `10.0`) (TST-4).
  - Picker index to score uses the table; index 0 gives 0.0.

## C. Simkl

### T-32a-030 — Simkl status ids and wire mapping
- Priority: P1
- Effort: S
- Depends: T-32a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklUtils.kt
- Acceptance:
  - Ids 1 to 5 and wire strings per D-3.2; `to_wire` for 0 or unknown gives `UnknownStatus` (TST-29).
  - `from_wire` maps `dropped` and `notinteresting` to 4, and unknown strings give an error (TST-30).
  - Labels resolve per B-3.1 (watching, completed, on hold, not interesting, plan to watch).

### T-32a-031 — Simkl authorise URL and Linux redirect handling
- Priority: P1
- Effort: M
- Depends: T-32a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt, app/src/main/AndroidManifest.xml
- Acceptance:
  - Authorise URL is byte-equal to `https://simkl.com/oauth/authorize?response_type=code&client_id=<id>&redirect_uri=aniyomi%3A%2F%2Fsimkl-auth` (TST-55).
  - `.desktop` file declares `MimeType=x-scheme-handler/aniyomi;` and the app accepts `aniyomi://simkl-auth?code=...` at startup or via single-instance activation (R-6.3).
  - Redirect without `code` triggers logout and no other action (TST-57).
  - Open decision O-6 is recorded in the README of the area before release.

### T-32a-032 — Simkl token exchange
- Priority: P1
- Effort: S
- Depends: T-32a-011, T-32a-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklOAuth.kt
- Acceptance:
  - POST `https://api.simkl.com/oauth/token` with JSON keys in order `code`, `client_id`, `client_secret`, `redirect_uri`, `grant_type` (D-4.2 S2).
  - Response `access_token` parsed; non-2xx gives `Http` error (TST-39).

### T-32a-033 — Simkl login completion and error handling
- Priority: P1
- Effort: S
- Depends: T-32a-032, T-32a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklUser.kt
- Acceptance:
  - Order: token exchange, store token, fetch user id (GET `/users/settings`), store username (decimal id) and password (token) (B-3.7).
  - Any failure clears all credentials and returns the error to the caller (TST-39, TST-40).
  - Missing `account` gives parse error, not id 0 (TST-51).

### T-32a-034 — Simkl token persistence
- Priority: P1
- Effort: S
- Depends: T-32a-002, T-32a-065
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklOAuth.kt
- Acceptance:
  - Token is stored in the secret store under `track.101.token` as compact JSON `{"access_token":"<t>"}` (TST-52).
  - Stored `"null"` or empty values read as absent (TST-42).

### T-32a-035 — Simkl request headers and missing-token behaviour
- Priority: P1
- Effort: S
- Depends: T-32a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklInterceptor.kt
- Acceptance:
  - Authenticated calls send `Authorization: Bearer <token>`, `simkl-api-key: <client id>`, and the Kurobayomi User-Agent (TST-54).
  - Missing token returns `NotAuthenticated("Simkl")` with no request.
  - Search and token exchange do not send the bearer header (TST-53).

### T-32a-036 — Simkl search over three types
- Priority: P1
- Effort: M
- Depends: T-32a-035, T-32a-037
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt
- Acceptance:
  - Three GET requests in order `anime`, `tv`, `movie` with query `q`, `extended=full`, `client_id` (B-3.14).
  - Results are concatenated without de-duplication (TST-44).
  - Any failing request fails the whole search (TST-43).

### T-32a-037 — Simkl search result mapping
- Priority: P1
- Effort: S
- Depends: T-32a-038
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklSearch.kt
- Acceptance:
  - Field mapping per B-3.14: `total_episodes = ep_count or 1`, poster URL with `_m.webp`, summary prefix `All titles:\n`, `publishing_status` default `ended`, `start_date` from year.
  - Missing both titles gives an empty title, no panic (TST-45).
  - Summary and poster edge cases match TST-46 and TST-47.

### T-32a-038 — Simkl tracking_url normalisation (fix for absolute search URLs)
- Priority: P1
- Effort: M
- Depends: T-32a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklUtils.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklSearch.kt
- Acceptance:
  - Search results store `tracking_url` as `/<anime|tv|movies>/<simkl id>` and keep the absolute URL in `source_url` for "open in browser" (R-5.2, R-5.7).
  - Movie results store `/movies/<id>` and add to the movies list (TST-26, TST-28).
  - Fallback segment is used when the URL has no known segment (TST-27).
  - Decision O-4 is confirmed on a live account (TST-76) before release.

### T-32a-039 — Simkl find in list (sync/watched and all-items), with TV fix
- Priority: P1
- Effort: M
- Depends: T-32a-038, T-32a-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklSyncItem.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklSyncWatched.kt
- Acceptance:
  - POST `/sync/watched` with `[{"simkl": id}]`; not-found gives None when result is not true or fields are missing (B-3.5 steps 1-2).
  - URL path uses `shows` for tv; the response is parsed by the response key `tv` (fix D-SK-1, TST-25).
  - Unknown path type is an error before any all-items request (no `//` in the URL).
  - `date_from` is passed unchanged and percent-encoded once (TST-48).

### T-32a-040 — Simkl add to list
- Priority: P1
- Effort: S
- Depends: T-32a-030, T-32a-035, T-32a-038
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt
- Acceptance:
  - POST `/sync/add-to-list` with `{"shows"|"movies": [{"ids":{"simkl":id},"to":"<wire>"}]}` (B-3.8.2).
  - Status is validated before the request (TST-29).
  - Only add-to-list is sent on add: no progress and no rating (B-3.9).

### T-32a-041 — Simkl update status rule
- Priority: P1
- Effort: S
- Depends: T-32a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt
- Acceptance:
  - When status is not completed and the episode was watched: completed if `last_episode_seen` equals a positive `total_episodes`, else watching (B-3.3).
  - Total of zero never auto-completes.
  - Status completed is never changed by update.

### T-32a-042 — Simkl progress sync (history remove then add)
- Priority: P1
- Effort: M
- Depends: T-32a-035, T-32a-041
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt
- Acceptance:
  - Shows only: POST `/sync/history/remove` then POST `/sync/history` with episodes 1..N (B-3.8.1).
  - Season number is always 1; N is truncated last episode seen; N=0 sends an empty list (TST-34, TST-35).
  - Movies send no progress (TST-36).
  - A failed add after remove returns an error and leaves the local row unchanged (TST-37); decision O-5 recorded.

### T-32a-043 — Simkl rating sync
- Priority: P1
- Effort: S
- Depends: T-32a-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt
- Acceptance:
  - Score 0.0 sends POST `/sync/ratings/remove` with rating 0; otherwise POST `/sync/ratings` with the truncated integer (TST-38).
  - Sent after add-to-list in the update sequence (TST-36).
  - Score -1.0 is not sent as a rating (TST-33).

### T-32a-044 — Simkl bind
- Priority: P1
- Effort: M
- Depends: T-32a-039, T-32a-040, T-32a-042, T-32a-043, T-32a-041
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklSyncItem.kt
- Acceptance:
  - Found: copy personal fields, keep local library id when remote has none (TST-49), keep status unless the item is not completed and has seen episodes, then run update (B-3.4).
  - Not found: status is watching when episodes are seen else plan to watch, score 0.0, add only (B-3.4).
  - Sync item mapping per B-3.6: `!!` replaced by `MissingField` errors (TST-32), movie counts per TST-31.

### T-32a-045 — Simkl refresh
- Priority: P1
- Effort: S
- Depends: T-32a-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt
- Acceptance:
  - When found: copy personal fields and total episodes; when not found: return the track unchanged with no error (TST-50).
  - Errors from the lookup propagate.

### T-32a-046 — Simkl logout
- Priority: P1
- Effort: S
- Depends: T-32a-034, T-32a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt
- Acceptance:
  - Clears username, password and the token key; in-memory token cleared (B-3.12).
  - No `"null"` value is written to any key (fix D-SK-4, TST-41).

### T-32a-047 — Simkl score display and status labels
- Priority: P2
- Effort: S
- Depends: T-32a-030, T-32a-072
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt
- Acceptance:
  - Score list is the integers 0 to 10 as text; display is the truncated integer (B-3.2).
  - Status picker order is watching, completed, on hold, not interesting, plan to watch (B-3.1).

### T-32a-048 — Simkl login UI (browser, waiting state, paste fallback)
- Priority: P1
- Effort: M
- Depends: T-32a-031, T-32a-033, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt
- Acceptance:
  - Clicking Login opens the default browser at the authorise URL (U-3).
  - Dialog offers a paste field that accepts a redirect URL or a bare code; host must be `simkl-auth` (TST-56).
  - A delivered redirect closes the dialog and logs in automatically.
  - Login errors are shown in the dialog text (deliberate change from the source, U-3 step 4).

### T-32a-049 — Simkl client constants, secret handling and log redaction
- Priority: P1
- Effort: S
- Depends: T-32a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt
- Acceptance:
  - Client id and client secret values are the constants in B-3.10 (byte-equal).
  - Logs for login never contain the code, the token or the secret (TST-58).
  - Decision O-7 (client secret in binary) is recorded in the release notes.

## D. Cross-cutting

### T-32a-050 — Logout confirmation dialog for both services
- Priority: P1
- Effort: S
- Depends: T-32a-014, T-32a-046
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Dialog title uses the service name; Cancel and destructive Log out buttons; Escape cancels (U-4).
  - Body text states that only the local link is removed (U-4).
  - Logout toast `logout_success` is shown after the action.

### T-32a-051 — Simkl logo asset
- Priority: P2
- Effort: S
- Depends: T-32a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt
- Acceptance:
  - Embedded SVG for `ic_tracker_simkl` is shown in the settings row and the search list.
  - Logo colour is black `0xFF000000` (B-3.1).

### T-32a-060 — HTTP error policy and JSON read helper for both services
- Priority: P1
- Effort: S
- Depends: T-32a-011, area:12-network-stack
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt
- Acceptance:
  - `read_json` returns `Http{status, body_snippet}` for non-2xx (512-byte snippet, no tokens) and `Parse` for bad bodies (R-9).
  - Error text shown to users is `HTTP <code>` for HTTP errors (matching Kotlin HttpException).
  - Auth failures 401 and 403 set the `auth_expired` UI state (U-9).

### T-32a-061 — Unit and fixture test suite for both trackers
- Priority: P1
- Effort: M
- Depends: T-32a-022, T-32a-044, T-32a-060
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklUtils.kt
- Acceptance:
  - All non-live TST items in p1-edge-cases-and-tests.md section 6 pass under `cargo test -p kuro-track`.
  - Wiremock tests assert request order, method, path, query and body bytes.
  - No test performs network access by default.

### T-32a-062 — Android-only plumbing: replacement decisions (P3)
- Priority: P3
- Effort: S
- Depends: T-32a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MURating.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt
- Acceptance:
  - Replacement recorded for `android.net.Uri`, `android.graphics.Color`, injekt, `runBlocking`, moko `StringResource` (R-11).
  - `MURating.copyTo` (dead code) is dropped and the drop is recorded in B-9.
  - `Context.openInBrowser(forceDefaultBrowser)` replacement recorded: no per-browser override on Linux.

### T-32a-064 — Settings > Tracking page rows for both services
- Priority: P1
- Effort: S
- Depends: T-32a-024, T-32a-048, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Rows for MangaUpdates and Simkl appear in the services group in the order of U-1.
  - Logged-in state is shown in the row and the action switches between Login and Logout.
  - Mockup `tracking_sheet` and `tracker_search` entries are wired to real data (U-12).

### T-32a-065 — Secret store migration for tracker tokens
- Priority: P1
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - On first start, `pref_mangasync_password_7`, `pref_mangasync_password_101` and `track_token_101` move to the secret store and the old keys are deleted (R-7, TST-65).
  - A value of `"null"` is treated as absent (TST-42).
  - If the secret store is unavailable, prefs fallback with a visible warning (TST-64).

### T-32a-066 — Per-track serialisation and cancellation safety for multi-request writes
- Priority: P1
- Effort: M
- Depends: T-32a-017, T-32a-042
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt
- Acceptance:
  - Concurrent updates for the same track id are serialised by a keyed mutex (TST-60).
  - Dropping the caller after the first write does not skip the second write (TST-61); pairs run on a spawned task.

### T-32a-067 — Poster and cover loading for search results
- Priority: P2
- Effort: S
- Depends: T-32a-037, T-32a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklSearch.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MURecord.kt
- Acceptance:
  - Covers load asynchronously with a placeholder; a 404 shows the placeholder without a toast (TST-67).
  - Covers are cached by the image cache of area 12, not by the tracker.

### T-32a-068 — Remember last MangaUpdates login name (no pre-filled password)
- Priority: P1
- Effort: S
- Depends: T-32a-024
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - The username entry is pre-filled from a new preference `track_login_name_7` (default empty), not from the stored uid (U-2).
  - The password entry is never pre-filled with the stored session token.

### T-32a-069 — Live verification tests and fixture recording
- Priority: P1
- Effort: S
- Depends: T-32a-061
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt
- Acceptance:
  - TST-74 to TST-78 exist as `#[ignore]` tests that run with a throwaway account.
  - Results resolve decisions O-1, O-4, O-5 and O-8; fixtures are replaced with recorded responses.
  - Recorded fixtures contain no real tokens or user ids.

### T-32a-070 — Remove-tracking dialog for both services
- Priority: P1
- Effort: S
- Depends: T-32a-019, T-32a-050
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt
- Acceptance:
  - MangaUpdates shows the checkbox "Also remove from my MangaUpdates list" (default off) that triggers the remote delete (U-8).
  - Simkl shows only the local-link text, with no remote delete (U-8, B-3.16).
  - Confirm is destructive; Cancel is the default.

### T-32a-071 — Tracker search screen wiring for both services
- Priority: P1
- Effort: M
- Depends: T-32a-021, T-32a-036, T-32a-064
- Sources: app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackerSearch.kt, app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt
- Acceptance:
  - Search has loading, empty ("No results found") and error states with the error text (U-5, U-9).
  - Result rows show the title, type, start date, status and score lines only when non-blank (U-5).
  - "Track privately" is hidden for both services (U-5, mockup gap 6).
  - Open in browser uses `source_url` for Simkl and `tracking_url` for MangaUpdates.

### T-32a-072 — Status and score pickers wired to tracker lists
- Priority: P1
- Effort: S
- Depends: T-32a-010, T-32a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt
- Acceptance:
  - Status picker shows the service list in the documented order with labels (U-7).
  - MangaUpdates score picker has 92 entries; Simkl has 0 to 10 (U-7).
  - Unknown status values in data show the raw number and no label (TST-21).
