# area:28-tracker-anilist, part 1 — implementation tasks

Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/28-tracker-anilist/p1-tasks.md`
Repo root for Sources: `/home/blackbox/aniyomi-src`. Design: `p1-rust-design.md`. Behaviour: `p1-behavior.md`. Data: `p1-data-and-config.md`. Edge cases: `p1-edge-cases-and-tests.md` (E-nn). UI: `p1-ui-parity.md`.

Shorthand for sources used below:
- `ANI` = `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt`
- `API` = `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt`
- `INT` = `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistInterceptor.kt`
- `UTL` = `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistUtils.kt`
- `DTO` = `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/`

## A. Core model and mapping

### T-28a-001 — Tracker module skeleton and dependencies
- Priority: P0
- Effort: S
- Depends: area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt
- Acceptance:
  - `src/tracker/anilist/{mod,status,score,auth,api,queries,dto,convert,service,html,date,error}.rs` exist and `cargo check` passes with the mockup crate.
  - Cargo.toml gains reqwest (json, rustls-tls), tokio (rt-multi-thread, sync, time, macros), chrono, url, thiserror, tracing, ammonia, html-escape, bytes, async-channel, keyring; `cargo tree` shows no OpenSSL.
  - No `unwrap()` or `expect()` in `src/tracker/` (grep check in CI).

### T-28a-002 — Tracker identity and capability flags
- Priority: P0
- Effort: S
- Depends: T-28a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt
- Acceptance:
  - `AniList::TRACKER_ID == 2`, `NAME == "AniList"`, `LOGO_RGB == (18, 25, 35)`.
  - `supports_reading_dates() == true`, `supports_private_tracking() == true`, `is_deletable() == true`.
  - Unit test asserts all five constants.

### T-28a-003 — Typed status enums with stored integers
- Priority: P0
- Effort: S
- Depends: T-28a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt
- Acceptance:
  - `MangaStatus` maps 1,2,3,4,5,6 and `AnimeStatus` maps 2,3,4,11,15,16 (p1-data §3); `from_db` returns None for any other integer.
  - Test: `MangaStatus::from_db(11)` is None and `AnimeStatus::from_db(1)` is None (E-63).
  - Stored integers unchanged versus the Kotlin constants.

### T-28a-004 — Status option lists and i18n label keys
- Priority: P0
- Effort: S
- Depends: T-28a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - Manga order: Reading, Plan to read, Completed, Rereading, Paused, Dropped; anime order: Watching, Plan to watch, Completed, Rewatching, Paused, Dropped (p1-behavior §2.2).
  - Label English text equals the base strings (Reading, Plan to read, Completed, Rereading, Paused, Dropped, Watching, Plan to watch, Rewatching).
  - Test: list lengths 6 and 6, labels exact.

### T-28a-005 — Score formats and picker option lists
- Priority: P0
- Effort: M
- Depends: T-28a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - `ScoreFormat::from_api` accepts POINT_100, POINT_10, POINT_10_DECIMAL, POINT_5, POINT_3; other strings return `UnknownScoreFormat` (E-15).
  - `options()` counts: 11, 101, 6, 4, 101 for POINT_10, POINT_100, POINT_5, POINT_3, POINT_10_DECIMAL; POINT_3 is exactly `["-","😦","😐","😊"]`; POINT_5 first `"0 ★"` last `"5 ★"`; POINT_10_DECIMAL first `"0.0"` last `"10.0"`.
  - Unknown format at load does not panic; falls back to POINT_10 and logs a warning.

### T-28a-006 — Score conversions: index, stored, display, API raw
- Priority: P0
- Effort: M
- Depends: T-28a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistUtils.kt
- Acceptance:
  - `index_to_score` table exactly as p1-behavior §3.4 (e.g. POINT_3 index 3 → 85.0, POINT_5 index 5 → 90.0, POINT_10_DECIMAL index 85 → 85.0).
  - `display` table exactly as p1-data §11 rows (e.g. POINT_5 at 35 → "2 ★", POINT_3 at 36 → "😐").
  - Out-of-range index returns `ScoreOutOfRange` (E-14); negative values never produced.
  - `to_score_raw` clamps to 0..=100 (E-14).

### T-28a-007 — Local status ↔ AniList MediaListStatus strings
- Priority: P0
- Effort: S
- Depends: T-28a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistUtils.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALManga.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALAnime.kt
- Acceptance:
  - Outgoing mapping per p1-behavior §2.5 (CURRENT, COMPLETED, PAUSED, DROPPED, PLANNING, REPEATING).
  - Incoming mapping per §2.6 for both kinds; unknown string → `UnknownStatus` (E-16), not panic.
  - Test table covers all twelve pairs.

### T-28a-008 — Authorize URL and redirect parsing
- Priority: P0
- Effort: S
- Depends: T-28a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt, app/src/main/AndroidManifest.xml
- Acceptance:
  - `authorize_url()` string equals `https://anilist.co/api/v2/oauth/authorize?client_id=5338&response_type=token`.
  - `parse_redirect` reads `access_token` from the URI fragment with no trailing-`&` requirement (E-02); `Denied` when missing.
  - Token redacted in Debug output.

### T-28a-009 — Token expiry in seconds, legacy ms migration, expiry on 401
- Priority: P0
- Effort: M
- Depends: T-28a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALOAuth.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt
- Acceptance:
  - Stored `expires_at` is unix seconds; `now > expires_at` means expired (same comparison as `isExpired`).
  - Stored values > 10^12 are treated as milliseconds and divided by 1000 on load (migration).
  - A 401 from any authenticated call sets state Expired and clears the token (E-03, E-49).
  - Test: a token stored with expiry in the past is not used for any request (no HTTP call made).

### T-28a-010 — Auth header, User-Agent, and not-authenticated gate
- Priority: P0
- Effort: S
- Depends: T-28a-009
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistInterceptor.kt
- Acceptance:
  - Authenticated calls send `Authorization: Bearer <token>` and `User-Agent: Kurobayomi v<version> (<app id>)`.
  - With no token, `execute(Auth::Required)` returns `NotAuthenticated` before any HTTP request (test with fake transport counts zero calls).
  - Null-stored OAuth with token present returns `NotAuthenticated` and does not panic (E-04).

### T-28a-011 — Login: token exchange, Viewer fetch, persistence, error surfacing
- Priority: P0
- Effort: M
- Depends: T-28a-010, T-28a-037
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALUser.kt
- Acceptance:
  - Login runs the `User` Viewer query, stores viewer id (i64) and score format; on failure returns `Err` and leaves state LoggedOut (E-01 fixed, toast shown).
  - Test: Viewer fixture `viewer_point10.json` sets viewer 4242 and POINT_10.
  - Score format is re-read on every successful login (E-50).

### T-28a-012 — Logout semantics
- Priority: P0
- Effort: S
- Depends: T-28a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - Logout clears token, viewer id, score format, auth-expired flag; afterwards `is_logged_in()` is false.
  - Token row is deleted, not set to the string `"null"` (E-05).
  - Local track rows are not deleted by logout (E-48).

### T-28a-013 — Score format refresh on settings migration (old int preference)
- Priority: P1
- Effort: S
- Depends: T-28a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - If a legacy integer value is found for the score key, the tracker logs the user out once and removes the key, with a toast "AniList login must be renewed" (new key `anilist_relogin_required`).
  - Test: integer legacy value triggers logout; string value does not.

## B. Transport and data

### T-28a-014 — GraphQL transport with 85-per-minute rate limiter
- Priority: P0
- Effort: M
- Depends: T-28a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/RateLimitInterceptor.kt
- Acceptance:
  - POST to `https://graphql.anilist.co/` with JSON body `{query, variables}`; `variables` key omitted for the Viewer query.
  - Rate limiter: 85 requests per rolling 60 s; test with fake clock (task T-28a-038 test 17/18).
  - One limiter instance shared by all tracker calls.

### T-28a-015 — GraphQL error envelope and HTTP status mapping
- Priority: P0
- Effort: M
- Depends: T-28a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/HttpException.kt
- Acceptance:
  - Every response is parsed as `{data?, errors?}`; non-empty `errors` → `GraphQl(messages)` even on HTTP 200 (E-22).
  - 401 → `AuthExpired`; 429 → `RateLimited`; 5xx → `Transient`; other non-2xx → `Http(code)` with message `HTTP error <code>`.
  - Fixtures `errors_http200.json`, `http_401.txt`, `http_429.txt`, `http_500.txt` pass.

### T-28a-016 — Manga search query and result mapping
- Priority: P0
- Effort: S
- Depends: T-28a-015, T-28a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALSearch.kt
- Acceptance:
  - Query text equals p1-data §4.7 (`Page (perPage: 50)`, `media(search: $query, type: MANGA, format_not_in: [NOVEL])`).
  - Test: `search_manga_ok.json` yields 3 results; NOVEL excluded by server filter (request assertion).
  - Search requires login (E-34) unless the live check described in p1-rust-design §14 is recorded and approved.

### T-28a-017 — Anime search query and result mapping
- Priority: P0
- Effort: S
- Depends: T-28a-015, T-28a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALSearch.kt
- Acceptance:
  - Query text equals p1-data §4.8 (`type: ANIME`, no format filter).
  - Test: `search_anime_ok.json` yields results with authors = main studios, cap 3 (E-08 with `search_anime_no_studios.json` returns empty authors, no panic).

### T-28a-018 — Search DTO conversion rules
- Priority: P0
- Effort: M
- Depends: T-28a-037, T-28a-023, T-28a-029
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALManga.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALAnime.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALSearchItem.kt
- Acceptance:
  - `format` `_`→`-`; `status` raw; `averageScore` null → -1; `start_date` `yyyy-MM-dd` only when fuzzy date non-zero (partial date → "" E-28).
  - Manga authors from roles containing "Story", artists from roles containing "Art" (both for "Story & Art"); names via userPreferred ?? full ?? native (E-10).
  - `tracking_url` = `https://anilist.co/manga/<id>` or `/anime/<id>`.
  - Null staff/studios do not panic (E-07, E-08).

### T-28a-019 — Find entry queries and user-list mapping
- Priority: P0
- Effort: M
- Depends: T-28a-037, T-28a-007, T-28a-006, T-28a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALUserList.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALManga.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALAnime.kt
- Acceptance:
  - Query text equals p1-data §4.9 and §4.10 (`scoreRaw: score(format: POINT_100)`).
  - `find_manga_found.json` maps status REPEATING → REREADING, score 85.0, progress 12, dates per year-only rule, private true, library_id from `id`.
  - Empty list → `None`.

### T-28a-020 — Add entry mutation and capture of MediaList id
- Priority: P0
- Effort: S
- Depends: T-28a-019, T-28a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALAddEntry.kt
- Acceptance:
  - Mutation `AddManga`/`AddAnime` variables exactly `mediaId, progress, status, private` (p1-data §4.1, §4.4).
  - Returns the new `SaveMediaListEntry.id`; the caller persists it as `library_id` in the same write (E-11, test 43).
  - Fixture `save_entry_ok.json` → id 9001.

### T-28a-021 — Update entry mutation payload
- Priority: P0
- Effort: M
- Depends: T-28a-015, T-28a-023, T-28a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt
- Acceptance:
  - Variables: `listId, progress (Int), status, private, score (scoreRaw 0..=100), startedAt, completedAt` per p1-data §4.2.
  - Date fields are omitted (not null) when the local value is 0 (E-29 proposal); decision recorded in a test.
  - Response `errors` checked (E-22); integer progress (E-33).

### T-28a-022 — Delete entry mutation
- Priority: P1
- Effort: S
- Depends: T-28a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt
- Acceptance:
  - Mutation `DeleteMediaListEntry(id: $listId) { deleted }` with `listId` = MediaList id (never the local row id).
  - Test: request body has `listId` equal to the id passed in; GraphQL errors surface.

### T-28a-023 — Fuzzy dates and create-date encoding
- Priority: P0
- Effort: S
- Depends: T-28a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALFuzzyDate.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt
- Acceptance:
  - `to_epoch_ms`: local midnight of Y-M-D; year-only or invalid date → 0 (tests 25, 26 under two TZ values).
  - `create_date`: 0 → no date; otherwise local Y, M, D integers (tests 27, 28).
  - Stored ms values unchanged in DB.

### T-28a-024 — Progress-driven status transitions
- Priority: P0
- Effort: M
- Depends: T-28a-003, T-28a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt
- Acceptance:
  - Read event (manga) rules per p1-behavior §10.1: final chapter → COMPLETED + finished now; REREADING kept; otherwise READING; progress 1.0 → started now.
  - Anime equivalent with WATCHING/REWATCHING.
  - Tests 33–37 pass, including E-25 (PAUSED becomes WATCHING on event) and E-24.

### T-28a-025 — Bind (register) for manga and anime
- Priority: P0
- Effort: M
- Depends: T-28a-019, T-28a-020, T-28a-021, T-28a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt
- Acceptance:
  - Found: copies progress, score, status, dates from remote, keeps local private (E-18 rule documented), sets library_id, status rules of §10.3, then update.
  - Not found: status READING (if has progress) else PLAN_TO_READ / PLANNING; score 0; add (test 42).
  - Tests 40–43 pass.

### T-28a-026 — Refresh from remote
- Priority: P0
- Effort: S
- Depends: T-28a-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/RefreshAnimeTracks.kt
- Acceptance:
  - Copies personal fields (incl. private, since copyRemotePrivate defaults true), title, total; keeps library_id and tracking_url (test 44).
  - Missing entry → `NotInList` (test 45).

### T-28a-027 — Lazy library_id resolution before write
- Priority: P0
- Effort: S
- Depends: T-28a-019, T-28a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt
- Acceptance:
  - When library_id is None or 0, update first resolves it via find; not found → `NotInList`, no save call (test 38, 39).
  - Resolved id is written back to the row before returning.

### T-28a-028 — Remote delete with correct MediaList id
- Priority: P1
- Effort: S
- Depends: T-28a-022, T-28a-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt
- Acceptance:
  - Delete passes resolved `library_id` (fixes E-20; test 46 asserts request body).
  - Entry not found remotely → Ok with info log (E-21, test 47).
  - Order: remote delete first, local unregister only after remote success or confirmed absence; failure toast shown (E-19, E-47).

### T-28a-029 — Description HTML to plain text
- Priority: P0
- Effort: S
- Depends: T-28a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALManga.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - `<br>`, `<br/>`, `<br />` → `\n`; tags stripped; entities decoded (`&amp;`, `&#39;`); trimmed (test 29, fixture `search_description_html.json`).
  - Documented differences from Android `Html.fromHtml` in the tracker module doc comment.

### T-28a-030 — Search result display rules
- Priority: P1
- Effort: S
- Depends: T-28a-018
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt, app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackerSearch.kt
- Acceptance:
  - Type text = lowercase then capitalise; status same (E-37 underscores replaced with spaces in Rust); score row hidden when -1.
  - Score shown as integer (deliberate change from Double text, E-56).
  - Summary max 4 lines; title max 2 lines.

### T-28a-031 — Info panel rows and AniList flags
- Priority: P0
- Effort: M
- Depends: T-28a-002, T-28a-026, T-28a-053, T-28a-057
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHome.kt, app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackInfoDialogHome.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - Rows: status, progress ("12 / 24" or "12"), score (only if != 0), start date (only if set), end date (only if set), private toggle, open in browser, copy link, search again, remove.
  - Empty bound state shows "Add tracking".
  - Replaces the mock grid in `src/entry.rs:509-520`.

### T-28a-032 — Index-based score picker
- Priority: P1
- Effort: S
- Depends: T-28a-006, T-28a-054
- Sources: app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - Picker stores an index; initial index from `score_to_index` (unscored POINT_3 opens at 0; E-13 regression test 8).
  - No negative scores possible through the UI (E-14).

### T-28a-033 — Progress picker range
- Priority: P1
- Effort: S
- Depends: T-28a-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt
- Acceptance:
  - Range 0..=total when total > 0; 0..=10000 when unknown (E-61).
  - Initial value = integer part of current progress.

### T-28a-034 — AniSkip MAL id lookup via AniList
- Priority: P1
- Effort: M
- Depends: T-28a-014, T-28a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/AniSkipApi.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `anilist_to_mal_id(media_id)` sends `query{ Media(id:$id){idMal} }` without auth (Auth::None), parses with serde, returns `None` on any failure (E-43, E-44).
  - Player uses `None` to skip AniSkip, not 0.
  - Test with fixture for `idMal: 16498` and `idMal: null`.

### T-28a-035 — AniChart next-airing lookup via AniList
- Priority: P2
- Effort: M
- Depends: T-28a-014, T-28a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt
- Acceptance:
  - `next_airing(media_id)` sends `Media(id:$id){ nextAiringEpisode { episode airingAt } }` (Auth::None), serde parse, returns `None` when absent.
  - Fallback episode 1 / airingAt 0 documented (E-45); no substring parsing.
  - Test with `nextAiringEpisode: null` and a populated fixture.

### T-28a-036 — Private tracking semantics
- Priority: P1
- Effort: S
- Depends: T-28a-025, T-28a-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt
- Acceptance:
  - "Track privately" sends `private: true` in the add/update mutation (test 52).
  - Toggling private on an existing bound track sends an update with the new value (framework `setRemotePrivate`).
  - Bind rule documented: local private choice wins over remote (E-18), surfaced in the bind confirmation text.

### T-28a-037 — Serde DTOs for every AniList response
- Priority: P0
- Effort: M
- Depends: T-28a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALAddEntry.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALSearch.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALSearchItem.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALUser.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALUserList.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALOAuth.kt
- Acceptance:
  - Field names and nullability as p1-data §5; optional where Kotlin used `!!` (E-07–E-10).
  - Fixtures in `tests/fixtures/anilist/` parse (test 31, 32).
  - `#[serde(rename = ...)]` for `Page`, `Viewer`, `SaveMediaListEntry`.

### T-28a-038 — Unit and behaviour test suite
- Priority: P0
- Effort: M
- Depends: T-28a-037, T-28a-014, T-28a-024, T-28a-025, T-28a-026, T-28a-027, T-28a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt
- Acceptance:
  - All 52 unit and behaviour tests in p1-edge-cases-and-tests §6 that are not marked integration exist and pass with `cargo test`.
  - Fake `Transport` and fixed clock make the suite offline and deterministic.
  - Fixtures listed in p1-edge-cases §4 exist.

### T-28a-039 — Track row persistence and per-track write serialisation
- Priority: P0
- Effort: M
- Depends: T-28a-025, T-28a-027, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt, data/src/main/sqldelightanime/dataanime/anime_sync.sq, data/src/main/sqldelight/data/manga_sync.sq
- Acceptance:
  - After every successful remote write the row is persisted with `library_id`, `remote_id`, `remote_url` (set on every write, E-39), and all personal fields.
  - Writes for the same `(kind, track_id)` are serialised with an async mutex (R-1, R-2).
  - Concurrent bind of the same media for the same local id results in one row (R-3 test).

### T-28a-040 — Redirect scheme handler and token landing
- Priority: P0
- Effort: M
- Depends: T-28a-008, T-28a-011
- Sources: app/src/main/AndroidManifest.xml, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt
- Acceptance:
  - Installed `.desktop` file declares `MimeType=x-scheme-handler/kurobayomi;` and `Exec=kurobayomi %u`; the app receives `kurobayomi://anilist-auth#...` whole (fragment preserved).
  - Redirect received while app already running is forwarded to the running instance (GApplication `open` or single-instance argv).
  - Duplicate redirect with the same token is ignored (R-5).

### T-28a-041 — Android-only pieces: replacement or drop record
- Priority: P3
- Effort: S
- Depends: T-28a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkExtensions.kt
- Acceptance:
  - Table in p1-rust-design §12 reviewed; each Android API (OkHttp, injekt, SharedPreferences, Activity intent filter, ConnectivityManager, WorkManager, java.time, Html.fromHtml) has a named replacement or drop reason in the design doc.
  - No code work; closes the inventory item.

## C. Offline, safety, and surfaces around the tracker

### T-28a-042 — Offline and retry queue behaviour for AniList updates
- Priority: P1
- Effort: M
- Depends: area:27-tracking-core, T-28a-015, T-28a-039
- Sources: app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt, app/src/main/java/eu/kanade/domain/track/anime/service/DelayedAnimeTrackingUpdateJob.kt
- Acceptance:
  - Offline → queued (no HTTP call). Transient, rate-limited, or offline errors → queued. NotInList and AuthExpired → not queued; toast shown (E-31 unified).
  - Queue keeps the max progress per track (as `DelayedAnimeTrackingStore.addAnime` does, `DelayedAnimeTrackingStore.kt:94-102`).
  - Retry with exponential backoff starting 5 min; give up after 4 attempts (`runAttemptCount > 3`).

### T-28a-043 — External player path refreshes before update
- Priority: P2
- Effort: S
- Depends: T-28a-026, T-28a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - External-player episode update calls `refresh` before `update(…, true)`, same as `TrackEpisode` (E-30).
  - Test: remote score unchanged after an external-player update event.

### T-28a-044 — Logout must not persist the literal "null"
- Priority: P2
- Effort: S
- Depends: T-28a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt
- Acceptance:
  - After logout, the token storage has no row (or empty), never the string `null` (E-05). Covered by T-28a-012 test; this task is the migration that clears any existing `"null"` value on startup.

### T-28a-045 — Localised AniList error messages
- Priority: P1
- Effort: S
- Depends: T-28a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt, app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt
- Acceptance:
  - New keys `anilist_err_not_authenticated`, `anilist_err_session_expired`, `anilist_err_not_in_list`, `anilist_err_rate_limited`, `anilist_err_offline`, `anilist_err_unknown_status`, `anilist_err_login_failed`, `anilist_err_remote_delete_failed` added to the Kurobayomi string table.
  - No raw Kotlin `toString()` or Rust `Debug` output ever shown (E-58; p1-behavior §18).

### T-28a-046 — Keyring-based token storage with fallback
- Priority: P1
- Effort: M
- Depends: T-28a-011, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistInterceptor.kt
- Acceptance:
  - Token stored in the Secret Service via `keyring` (service `kurobayomi`, user `anilist-token`); viewer id and score format in settings.
  - If no secret service is available, the token is kept in a 0600 file with a visible warning in the tracking settings.
  - Tokens are never written to logs (test: tracing output scan).

### T-28a-047 — Settings row: AniList login and logout
- Priority: P0
- Effort: S
- Depends: T-28a-011, T-28a-040
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt, spec/settings.json
- Acceptance:
  - Row "AniList" in the "Trackers" group with a switch reflecting `is_logged_in()`; switch on → open authorize URL in the default browser; switch off → logout confirmation dialog (title "Log out from AniList?", Cancel + destructive confirm).
  - Logout toast "You are now logged out".
  - Row updates after redirect without restart.

### T-28a-048 — Login progress and session-expired state
- Priority: P1
- Effort: S
- Depends: T-28a-040, T-28a-009
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt
- Acceptance:
  - While the Viewer query runs, the AniList row shows "Logging in…" with a spinner.
  - Expired session shows row subtitle "Session expired, log in again" and a single toast.
  - Login success shows toast "Logged in to AniList" (new key); failure shows the localized error (T-28a-045).

### T-28a-049 — Backup and restore fields for AniList tracks
- Priority: P1
- Effort: S
- Depends: area:21-backup-format-and-create, area:22-backup-restore, T-28a-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupTracking.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnimeTracking.kt
- Acceptance:
  - `library_id`, `remote_id`, `remote_url`, `private`, dates, status, score round-trip through backup and restore for tracker id 2.
  - Restored AniList track with library_id missing triggers lazy resolution on first update (no error).
  - Test: backup → restore preserves all fields (fixture).

### T-28a-050 — Date round-trip tests under two timezones
- Priority: P2
- Effort: S
- Depends: T-28a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALFuzzyDate.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt
- Acceptance:
  - Run the date tests with `TZ=America/New_York` and `TZ=Asia/Tokyo`; picking a date and reading it back yields the same local calendar date.
  - Documented: time-of-day is lost (E-27).

### T-28a-051 — Search row polish
- Priority: P2
- Effort: S
- Depends: T-28a-030
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt
- Acceptance:
  - Authors/artists list shows distinct names joined by ", " on one line with ellipsis.
  - Copy-to-clipboard and open-in-browser available from context menu and keyboard shortcut (Shift+F10 / Menu key).

### T-28a-052 — Keyboard navigation in tracker search
- Priority: P2
- Effort: S
- Depends: T-28a-057
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt
- Acceptance:
  - Enter in the field searches; Up/Down move the selection; Enter on a selected row triggers "Track".
  - Escape closes the search.

### T-28a-053 — Status picker dialog
- Priority: P0
- Effort: M
- Depends: T-28a-004, T-28a-025
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt
- Acceptance:
  - Radio list in the order of T-28a-004; OK saves via `set_status` (COMPLETED sets progress to total when total > 0).
  - Cancel changes nothing; the write runs detached so closing does not abort it.

### T-28a-054 — Score picker dialog (format-specific widgets)
- Priority: P0
- Effort: M
- Depends: T-28a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt
- Acceptance:
  - POINT_3 → four radio buttons; POINT_5 → six radio buttons; other formats → spin button with decimal display where applicable.
  - OK saves via `set_score` with an integer scoreRaw 0..=100.

### T-28a-055 — Progress picker dialog
- Priority: P1
- Effort: S
- Depends: T-28a-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - SpinButton with the range from T-28a-033; OK saves via set_last_episode_seen / set_last_chapter_read.

### T-28a-056 — Date picker dialog with date constraints
- Priority: P1
- Effort: M
- Depends: T-28a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - No future dates; start ≤ finish and finish ≥ start when both set (E-59).
  - Remove button only when the date is set; confirm dialog text per p1-ui-parity §7.
  - Removal sends an unset date (E-60).

### T-28a-057 — Tracker search screen (replace mock)
- Priority: P0
- Effort: M
- Depends: T-28a-016, T-28a-017, T-28a-025
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - Real results replace the four fake rows in `src/entry.rs:546-553`; loading, empty ("No results found"), and error states per p1-ui-parity §8.
  - Initial query = bound title or entry title; preselect the result whose `remote_id` equals the bound remote id (E-57).
  - "Track" and "Track privately" call bind with the chosen private flag; both buttons disabled while the write is in flight.

### T-28a-058 — Auto-update setting gates (mark-read behaviour)
- Priority: P1
- Effort: S
- Depends: T-28a-024, T-28a-042
- Sources: app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - Settings: `pref_auto_update_manga_sync_key` (default true) and `pref_auto_update_manga_on_mark_read` (enum ALWAYS default, ASK, NEVER) persisted.
  - ALWAYS → update trackers silently and toast "Trackers updated to episode %d"; ASK → snackbar "Update trackers to episode %d?" with OK; NEVER → nothing.
  - Only fires for mark-as-seen (not un-seen).
