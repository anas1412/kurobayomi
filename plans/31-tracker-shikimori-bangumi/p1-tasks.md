# Area 31-tracker-shikimori-bangumi, part 1: implementation tasks

Task ids are `T-31a-001` to `T-31a-049`. Each task is one reviewable unit. Acceptance criteria name the test ids in p1-edge-cases-and-tests.md (`TS-*`) where a test exists. Sources are repo-relative and cite the Kotlin that the task reproduces or replaces. Behaviour details are in p1-behavior.md, data in p1-data-and-config.md, the Rust layout in p1-rust-design.md.

Priority meaning in this area: P1 = tracker parity for Shikimori and Bangumi (users rely on both); P2 = improvements over parity that change behaviour on purpose; P3 = Android-only plumbing with a stated replacement.

Coverage: every file in p1-inventory.md maps to at least one task (see the file-to-task table at the end). Every defect in p1-behavior.md section 8 (D1 to D22) is assigned to a task.

---

### T-31a-001 — Shikimori tracker type, identity and registration
- Priority: P1
- Effort: M
- Depends: area:27-tracking-core, T-31a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt
- Acceptance:
  - `ShikimoriTracker` exists in `kuro-track/src/shikimori/mod.rs`, with `id() == 4`, `name() == "Shikimori"`, `supports_private_tracking() == false`, `supports_reading_dates() == false`.
  - `completion_status() == 2`, `reading_status() == watching_status() == 1`, `rereading_status() == rewatching_status() == 6` (Shikimori.kt:211-219).
  - `status_list()` for manga and anime returns `[1,2,3,4,5,6]` in that order (Shikimori.kt:183-189).
  - `logo_color()` returns `0xFF282828` (Shikimori.kt:181).
  - Registered in the tracker registry at index 3 of `trackers()` (TrackerManager.kt:39-42), and `get(4)` returns it.

### T-31a-002 — Shikimori status enum and API strings
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriUtils.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt
- Acceptance:
  - `ShikiStatus` has discriminants 1..6 exactly as in p1-behavior.md 1.1 (`Reading=1`, `Completed=2`, `OnHold=3`, `Dropped=4`, `PlanToRead=5`, `Rereading=6`).
  - `api_str()` gives `watching`, `completed`, `on_hold`, `dropped`, `planned`, `rewatching` (ShikimoriUtils.kt:6-24). `from_api` is the inverse (ShikimoriUtils.kt:26-34).
  - `from_api("nope")` returns `TrackError::UnknownStatus`, never panics (TS-S-01, TS-S-17).
  - `from_internal(0)` and `from_internal(7)` return `None`.

### T-31a-003 — Shikimori score model
- Priority: P1
- Effort: S
- Depends: T-31a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt
- Acceptance:
  - Score list is `"0"` through `"10"`, 11 entries (Shikimori.kt:44-46), using the shared `SCORE_LIST_0_10` from area 27.
  - `index_to_score(i) == i as f64`; `display_score(t) == (t.score as i64).to_string()` (Shikimori.kt:57-67).
  - Write path clamps the score to `0..=10` with negatives written as `0`. Kotlin does not clamp (D8). Test EC-S-23 and EC-S-24 (TS-S-13).

### T-31a-004 — Shikimori wire DTOs
- Priority: P1
- Effort: S
- Depends: T-31a-043
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMEntry.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMUserListEntry.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMAddEntryResponse.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMUser.kt
- Acceptance:
  - Structs `SmEntry`, `SmCover`, `SmUserListEntry`, `SmAddEntryResponse`, `SmUser` in `shikimori/dto.rs` with the fields and optionality in p1-data-and-config.md section 1.1.
  - `SmEntry` decodes with `chapters`, `episodes` and `aired_on` absent (EC-S-13; TS-S-12).
  - `SmUserListEntry.score` is an integer; `chapters` and `episodes` are floats (p1-data-and-config.md 1.1).

### T-31a-005 — Shikimori token type, expiry and single-flight session
- Priority: P1
- Effort: M
- Depends: T-31a-004, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMOAuth.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriInterceptor.kt
- Acceptance:
  - `SmOAuth` decodes the token response; `refresh_token` optional (p1-data-and-config.md 1.1).
  - `is_expired(t, now)` is `now > created_at + expires_in - 3600`, strict (EC-S-05, EC-S-06; TS-S-02).
  - `ShikimoriSession::bearer()` holds a `tokio::sync::Mutex` across the refresh; ten concurrent callers cause one refresh request (RC-01; TS-S-04).
  - A refresh rejected with 400 or 401 sets the auth-expired flag and returns `AuthExpired` without sending the old token (D1, EC-S-03; TS-S-05).
  - A refresh failing with 5xx keeps the token and returns `Http(status)` (EC-S-04; TS-S-06).
  - Missing `refresh_token` on an expired token returns `AuthExpired` (EC-S-08; TS-S-07). Missing refresh token on a fresh token works (EC-S-07).

### T-31a-006 — Shikimori login by OAuth code
- Priority: P1
- Effort: M
- Depends: T-31a-005, T-31a-007, T-31a-039, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt
- Acceptance:
  - `authUrl()` is `https://shikimori.one/oauth/authorize?client_id=<client_id>&redirect_uri=<redirect_uri>&response_type=code` with that parameter order and no `scope` (p1-data-and-config.md section 8).
  - Token exchange sends form keys `grant_type`, `client_id`, `client_secret`, `code`, `redirect_uri` in that order (TS-S-08).
  - After a successful exchange, `GET /api/users/whoami` gives the id; username is the decimal id string, password the access token (EC-S-11).
  - A failed exchange or a failed `whoami` returns an error to the caller. Credentials stay cleared, and the token is not stored unless `whoami` succeeded (EC-S-09, EC-S-10; TS-S-08). Kotlin's silent `logout()` is replaced by a returned error (p1-behavior.md section 5).
  - A second login attempt while the first is in progress does not clear a successful login (RC-06; TS-S-08).

### T-31a-007 — Shikimori credential and token persistence
- Priority: P1
- Effort: S
- Depends: T-31a-005, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - Token JSON is stored in the secret store under service `kurobayomi.track`, account `4.token`, and restored in the constructor (not lazily). A stored value that fails to decode yields logged out (p1-behavior.md 2.5).
  - Username is stored under `4.username` and password (access token) under `4.password`. `is_logged_in()` is true only when both are non-empty (BaseTracker.kt:32-34).
  - A missing secret store service falls back to the area 27 file store (FM-04).
  - No value is written to the preference file (D11).

### T-31a-008 — Shikimori request authentication and headers
- Priority: P1
- Effort: S
- Depends: T-31a-005, T-31a-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriInterceptor.kt
- Acceptance:
  - Every authenticated call sends `Authorization: Bearer <access token>` and the Kurobayomi User-Agent (`user_agent()`), never a query-string token (p1-data-and-config.md section 7).
  - When logged out, an authenticated call returns `NotAuthenticated("Shikimori")` before any network I/O (EC-S-02; TS-S-10).
  - The refresh request is sent through the raw client, not through the authenticated request path (D16).
  - A fresh token is never refreshed (EC-S-05).

### T-31a-009 — Shikimori catalogue search
- Priority: P1
- Effort: S
- Depends: T-31a-008, T-31a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt
- Acceptance:
  - Manga search sends `GET /api/mangas?order=popularity&search=<q>&limit=20`, anime search `GET /api/animes?order=popularity&search=<q>&limit=20` with the same parameter order (ShikimoriApi.kt:124-154).
  - Results are mapped by T-31a-017. Empty array gives an empty list.
  - Logged-out search follows the anonymous rule in T-31a-045 (P2). Until then it fails with `NotAuthenticated` (D9).

### T-31a-010 — Shikimori library lookup
- Priority: P1
- Effort: S
- Depends: T-31a-008, T-31a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMUserListEntry.kt
- Acceptance:
  - Two sequential requests: catalogue entry (`/api/mangas/{id}` or `/api/animes/{id}`), then `GET /api/v2/user_rates?user_id=<uid>&target_id=<rid>&target_type=Manga` (or `Anime`) (p1-behavior.md 2.9).
  - Catalogue 404 returns `HttpStatus(404)` (EC-S-17). Two rows return `TooManyEntries` (EC-S-15). Empty array returns `None` (EC-S-16). Both with the same message for anime and manga (D13 fixed by key, not text).
  - `remote_id` stays the catalogue id; `library_id` is set to the user-rate id (D17).
  - The user-rates GET sends `Cache-Control: no-cache` (D12; T-31a-040).

### T-31a-011 — Shikimori add and update through the upsert
- Priority: P1
- Effort: S
- Depends: T-31a-008, T-31a-003, T-31a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt
- Acceptance:
  - One function serves add and update: `POST /api/v2/user_rates` with `Content-Type: application/json; charset=utf-8` and the body in p1-data-and-config.md 7.3 (key order preserved, `preserve_order` enabled).
  - Manga bodies use `"chapters"`; anime bodies use `"episodes"`; `target_type` is `Manga` or `Anime`.
  - Response `{id}` sets `library_id` (TS-S-18, TS-S-19).
  - Returns the same `TrackError` family for HTTP and transport failures (T-31a-041).

### T-31a-012 — Shikimori status transitions on read
- Priority: P1
- Effort: S
- Depends: T-31a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt
- Acceptance:
  - `next_status_after_read` reproduces Shikimori.kt:77-89 and 91-103 exactly: no change when `Completed` or `did_read == false`; `Completed` when `last as i64 == total && total > 0`; otherwise `Reading`, except `Rereading` stays (EC-S-19, EC-S-20, EC-S-21; TS-S-11).
  - The update path that calls this function is only used by the mark-read flows (TrackChapter, TrackEpisode in area 27).

### T-31a-013 — Shikimori bind
- Priority: P1
- Effort: M
- Depends: T-31a-010, T-31a-011, T-31a-012, T-31a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt
- Acceptance:
  - Found: copy progress, score and status from the remote entry only (`copy_shiki_personal`, p1-rust-design.md 3.5). Dates and `private` are not copied (D22, EC-S-14; TS-S-14).
  - Found and `has_read` true and status not `Completed` and not `Rereading`: status becomes `Reading` (Shikimori.kt:119-122).
  - Then one upsert (T-31a-011).
  - Not found: status is `Reading` if `has_read` else `PlanToRead`; score `0.0`; upsert (EC-S-18; TS-S-10).
  - A catalogue entry with null `chapters` (manga) or `episodes` (anime) fails bind with `Decode` instead of a null-pointer failure (EC-S-12).

### T-31a-014 — Shikimori refresh
- Priority: P1
- Effort: S
- Depends: T-31a-010, T-31a-041
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt
- Acceptance:
  - Looks up the remote entry (T-31a-010). Copies progress, score, status and total chapters, and `library_id` (Shikimori.kt:161-168). No write is sent.
  - Not found returns `NotFound("manga")` or `NotFound("anime")` (Shikimori.kt:166, 175).
  - Dates are not cleared (D22).

### T-31a-015 — Shikimori delete remote entry
- Priority: P1
- Effort: S
- Depends: T-31a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - `DELETE /api/v2/user_rates/{library_id}` with bearer (ShikimoriApi.kt:75-81).
  - Missing `library_id` returns `NoLibraryId` and sends nothing (D18, EC-S-25; TS-S-16).
  - A 404 response returns `Ok(())` (proposal, OQ-2; EC-S-26).
  - Implements `DeletableMangaTracker` and `DeletableAnimeTracker` (only Shikimori among these two).

### T-31a-016 — Shikimori logout
- Priority: P1
- Effort: S
- Depends: T-31a-007, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt
- Acceptance:
  - Order: clear username and password, clear auth-expired flag and publish logged-out; then delete `4.token` and clear the in-memory token (p1-rust-design.md 5.4).
  - No `"null"` string is ever written to the token secret (D10; TS-S-15).
  - After logout, `is_logged_in()` is false and `restore()` returns none (TS-S-15, EC-S-30).

### T-31a-017 — Shikimori catalogue result mapping
- Priority: P1
- Effort: S
- Depends: T-31a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMEntry.kt
- Acceptance:
  - Manga result: `title = name`, `remote_id = id`, `total_chapters = chapters` (required for manga; missing becomes a skipped element or `Decode`, OQ-1), `cover_url = "https://shikimori.one" + image.preview`, `summary = ""`, `score = score`, `tracking_url = "https://shikimori.one" + url`, `publishing_status = status`, `publishing_type = kind`, `start_date = aired_on ?? ""` (SMEntry.kt:23-36; TS-S-12).
  - Anime result: same with `total_episodes = episodes` (SMEntry.kt:38-51).
  - Unicode and unknown `kind` values pass through (EC-S-28, EC-S-32).

### T-31a-018 — Shikimori status labels and lists per kind
- Priority: P1
- Effort: S
- Depends: T-31a-002, T-31a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt
- Acceptance:
  - Manga labels: 1→`MR.strings.reading`, 5→`MR.strings.plan_to_read`, 2→`MR.strings.completed`, 3→`MR.strings.on_hold`, 4→`MR.strings.dropped`, 6→`MR.strings.repeating` (Shikimori.kt:191-199).
  - Anime labels: 1→`AYMR.strings.watching`, 5→`AYMR.strings.plan_to_watch`, 2/3/4 as manga, 6→`AYMR.strings.repeating_anime` (Shikimori.kt:201-209).
  - Unknown values return no label.
  - Keys resolve to the English values in p1-behavior.md section 2.14 when rendered (area:51-i18n checks the strings).

### T-31a-019 — Shikimori test suite
- Priority: P1
- Effort: M
- Depends: T-31a-005, T-31a-006, T-31a-009, T-31a-010, T-31a-011, T-31a-012, T-31a-013, T-31a-014, T-31a-015, T-31a-016, T-31a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriInterceptor.kt
- Acceptance:
  - Tests TS-S-01 to TS-S-19 in `crates/kuro-track/tests/shikimori_*.rs` pass with `cargo test -p kuro-track`.
  - Fixtures from p1-edge-cases-and-tests.md section 7 (Shikimori rows) are present under `crates/kuro-track/tests/fixtures/shikimori/`.
  - No test contacts the network (wiremock only).

### T-31a-020 — Bangumi tracker type, identity and registration
- Priority: P1
- Effort: M
- Depends: area:27-tracking-core, T-31a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/Bangumi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt
- Acceptance:
  - `BangumiTracker` with `id() == 5`, `name() == "Bangumi"`, `supports_private_tracking() == true` (Bangumi.kt:32), `supports_reading_dates() == false`.
  - `completion_status() == 2`, `reading_status() == watching_status() == 3`, `rereading_status() == rewatching_status() == -1` (Bangumi.kt:178-186).
  - `status_list()` for manga and anime returns `[3,2,4,5,1]` (Bangumi.kt:152-158).
  - `logo_color()` returns `0xFFF09199` (Bangumi.kt:150). Registered at index 4 of the registry (TrackerManager.kt:39-42).
  - Does not implement the Deletable traits.

### T-31a-021 — Bangumi status enum, collection type and labels
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiUtils.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/dto/BGMCollectionResponse.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/Bangumi.kt
- Acceptance:
  - `BgmStatus` discriminants `PlanToRead=1, Completed=2, Reading=3, OnHold=4, Dropped=5` (Bangumi.kt:223-227).
  - `collection_type()` equals the discriminant (BangumiUtils.kt:6-22). `from_collection_type(None)` and `(Some(6))` return `UnknownStatus` (EC-B-08, EC-B-09; TS-B-01, TS-B-08).
  - Labels: manga 3→`MR.strings.reading`, 1→`MR.strings.plan_to_read`, 2→`MR.strings.completed`, 4→`MR.strings.on_hold`, 5→`MR.strings.dropped`; anime 3→`AYMR.strings.watching`, 1→`AYMR.strings.plan_to_watch`, others as manga; `Rereading` never exists (Bangumi.kt:160-176).

### T-31a-022 — Bangumi score coercion and display
- Priority: P1
- Effort: S
- Depends: T-31a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/Bangumi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt
- Acceptance:
  - Score list `"0".."10"`; index equals score; display `score as i64` (Bangumi.kt:34-46).
  - Write `rate = (score as i64).clamp(0, 10)` (BangumiApi.kt:48, 66, 84, 109). `-1` is written as `0` (EC-B-19), `10.9` as `10` (EC-B-20; TS-B-11).

### T-31a-023 — Bangumi collection and slim-subject DTOs
- Priority: P1
- Effort: S
- Depends: T-31a-043
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/dto/BGMCollectionResponse.kt
- Acceptance:
  - `BgmCollection` with `rate`, `type` (serde rename), `ep_status`, `vol_status`, `private`, `subject` with defaults and optionality of p1-data-and-config.md 1.2 (EC-B-10, EC-B-11, EC-B-33).
  - `private` is decoded and not mapped to the track (D14).

### T-31a-024 — Bangumi search DTOs and result mapping
- Priority: P1
- Effort: M
- Depends: T-31a-043
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/dto/BGMSearch.kt
- Acceptance:
  - Structs `BgmSearchResult`, `BgmSubject`, `BgmImages`, `BgmRating` with the fields and defaults of p1-data-and-config.md 1.2.
  - Title: `name_cn` if not blank, else `name` (EC-B-25).
  - Summary: with non-blank `name_cn`, `"作品原名：" + name` plus `"\n" + summary.trim()` when summary is non-null; otherwise `summary.trim()` or empty (EC-B-25, EC-B-26, EC-B-27; TS-B-02).
  - Score `rating.score` or `-1.0` (EC-B-28). `start_date = date ?? ""` (EC-B-29). `total = eps` (EC-B-30). Tracking URL `https://bangumi.tv/subject/<id>`.

### T-31a-025 — Bangumi token type, expiry and created_at default
- Priority: P1
- Effort: S
- Depends: T-31a-043
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/dto/BGMOAuth.kt
- Acceptance:
  - `BgmOAuth` with `created_at` set to the decode time when absent (EC-B-04), `user_id` optional and never read (EC-B-05).
  - `is_expired` uses the same strict formula as Shikimori with `expires_in` 604800 (TS-B-04).
  - Stored JSON round-trips all six keys.

### T-31a-026 — Bangumi login by OAuth code and username
- Priority: P1
- Effort: M
- Depends: T-31a-025, T-31a-034, T-31a-039, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/Bangumi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/dto/BGMUser.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt
- Acceptance:
  - `authUrl()` = `https://bgm.tv/oauth/authorize?client_id=<client_id>&response_type=code&redirect_uri=<redirect_uri>` (order as in p1-data-and-config.md 8).
  - Token exchange: form keys `grant_type`, `client_id`, `client_secret`, `code`, `redirect_uri` in order; the host is `bgm.tv`, not `api.bgm.tv` (TS-B-04).
  - Username from `GET https://api.bgm.tv/v0/me`, the login name string (not the nickname). Stored with the access token only after `/v0/me` succeeds (EC-B-05 context).
  - Errors are returned, not silently turned into logout (p1-behavior.md section 5).

### T-31a-027 — Bangumi request authentication, refresh and UA
- Priority: P1
- Effort: M
- Depends: T-31a-025, T-31a-040, T-31a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt
- Acceptance:
  - Bearer header and Kurobayomi UA on every authenticated call; logged-out call returns `NotAuthenticated("Bangumi")` before I/O (EC-B-32; TS-B-15).
  - Refresh form keys `grant_type`, `client_id`, `client_secret`, `refresh_token`, `redirect_uri` in that order (EC-B-03; TS-B-06).
  - Refresh rejected: `AuthExpired` with flag set (EC-B-02; TS-B-05). Refresh 5xx: `Http`, token kept.
  - Refresh decodes the documented body without `user_id` (EC-B-12; TS-B-05).
  - Refresh runs on the raw client (D16).

### T-31a-028 — Bangumi search (manga and anime)
- Priority: P1
- Effort: S
- Depends: T-31a-027, T-31a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt
- Acceptance:
  - `POST https://api.bgm.tv/v0/search/subjects?limit=20` with `Content-Type: application/json` and body `{"keyword","sort":"match","filter":{"type":[1]}}` for manga, `[2]` for anime (BangumiApi.kt:129-184; p1-data-and-config.md 7.3).
  - Manga keeps results whose `platform` is null or `"漫画"`; anime keeps all (EC-B-23, EC-B-24; TS-B-03).
  - Logged-out search follows T-31a-045 (P2). Until then it fails with `NotAuthenticated("Bangumi")` (D9).

### T-31a-029 — Bangumi collection lookup
- Priority: P1
- Effort: M
- Depends: T-31a-023, T-31a-027, T-31a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt
- Acceptance:
  - `GET https://api.bgm.tv/v0/users/{username}/collections/{subject_id}` with `Cache-Control: no-cache` (FORCE_NETWORK, BangumiApi.kt:191).
  - 404 returns `None` (EC-B-06). 403 and other statuses return `HttpStatus` (EC-B-07; TS-B-07).
  - Returns a new `BgmLibEntry` value. The caller's track is not mutated (D5).
  - Missing `rate`, `ep_status`, `subject` default to `0` (EC-B-10, EC-B-11, EC-B-33).
  - Unknown or absent `type` returns `UnknownStatus` (EC-B-08, EC-B-09).
  - Username is percent-encoded as a path segment (proposal P-enc; EC-B-31). Empty username is refused before the request (EC-B-32).

### T-31a-030 — Bangumi add (POST 202) and update (PATCH 204)
- Priority: P1
- Effort: S
- Depends: T-31a-027, T-31a-022, T-31a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt
- Acceptance:
  - Add: `POST https://api.bgm.tv/v0/users/-/collections/{subject_id}`, body `{"type","rate","ep_status","private"}` in order, 202 success, body not parsed, `library_id` unchanged (EC-B-18; TS-B-10).
  - Update: `PATCH` same URL and body, 204 success (EC-B-17; TS-B-09).
  - `ep_status` is `last_chapter_read as i64` for manga and `last_episode_seen as i64` for anime (EC-B-34, D7 parity).
  - `private` is sent on every write from the local value (EC-B-21; TS-B-12).

### T-31a-031 — Bangumi status transitions on read
- Priority: P1
- Effort: S
- Depends: T-31a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/Bangumi.kt
- Acceptance:
  - `bgm_next_status_after_read` reproduces Bangumi.kt:56-68 and 70-82: no change when `Completed` or not read; `Completed` when `last == total && total > 0`; otherwise `Reading` (so `PlanToRead`, `OnHold` and `Dropped` become `Reading`) (TS-B-09).

### T-31a-032 — Bangumi bind
- Priority: P1
- Effort: M
- Depends: T-31a-029, T-31a-030, T-31a-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/Bangumi.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt
- Acceptance:
  - Found: copy progress, score, total and status from the lookup entry (`copy_bgm_personal`); `private` is kept (EC-B-22; TS-B-13).
  - Found and `has_read` true and status not `Completed`: status becomes `Reading`; otherwise remote status kept (Bangumi.kt:93-95). Then one PATCH (T-31a-030).
  - Not found: status `Reading` if `has_read` else `PlanToRead`; score `0`; POST (EC-B-16; TS-B-10).
  - Dates are not copied (EC-S-14 rule applies to Bangumi too, since Bangumi has no dates).

### T-31a-033 — Bangumi refresh
- Priority: P1
- Effort: S
- Depends: T-31a-029
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/Bangumi.kt
- Acceptance:
  - Looks up the collection and copies progress, score, status and total (`copy_bgm_personal`). No write. Not found returns `NotFound("manga")` or `NotFound("anime")` (Bangumi.kt:137, 143).
  - `library_id` and `private` are untouched (TS-B-13, TS-B-14 for refresh).

### T-31a-034 — Bangumi token persistence and logout
- Priority: P1
- Effort: S
- Depends: T-31a-025, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/Bangumi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiInterceptor.kt
- Acceptance:
  - Token JSON under `5.token`, username under `5.username`, access token under `5.password` (p1-rust-design.md 5.5). Restored in the constructor.
  - Logout order as T-31a-016: clear credentials, clear flag, publish logged-out, delete `5.token`, clear in-memory token. No `"null"` written (D10).
  - Persisted token includes `user_id` when present, omitted when absent (round trip).

### T-31a-035 — Bangumi private flag and local-only dates
- Priority: P1
- Effort: S
- Depends: T-31a-030, T-31a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/Bangumi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt
- Acceptance:
  - Bangumi `supports_private_tracking() == true`; the search sheet shows the private button only then (area 27 UI, EC-B-21).
  - Shikimori `supports_private_tracking() == false`; `private` is never sent to Shikimori.
  - Start and finish dates are never sent to either tracker; `setRemoteStartDate` and `setRemoteFinishDate` only update local state (p1-behavior.md 3.14).

### T-31a-036 — Bangumi test suite
- Priority: P1
- Effort: M
- Depends: T-31a-022, T-31a-024, T-31a-025, T-31a-026, T-31a-027, T-31a-028, T-31a-029, T-31a-030, T-31a-031, T-31a-032, T-31a-033, T-31a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/dto/BGMSearch.kt
- Acceptance:
  - Tests TS-B-01 to TS-B-14 in `crates/kuro-track/tests/bangumi_*.rs` pass.
  - Fixtures from p1-edge-cases-and-tests.md section 7 (Bangumi rows) are present under `crates/kuro-track/tests/fixtures/bangumi/`.
  - The documented refresh example decodes (TS-B-05).

### T-31a-037 — Tracker registry rows and login UI wiring for both trackers
- Priority: P1
- Effort: S
- Depends: T-31a-001, T-31a-020, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt
- Acceptance:
  - The Tracking settings page shows a Shikimori row and a Bangumi row (SettingsTrackingScreen.kt:191-219), each with Login (opens the authorise URL in the default browser) and Logout (confirmation dialog), per area 27.
  - When the client is not configured (T-31a-039), the login action is disabled and says why.
  - Both rows appear in the registry order given in p1-rust-design.md section 11.

### T-31a-038 — OAuth redirect parameters shared with area 27
- Priority: P1
- Effort: S
- Depends: area:27-tracking-core, T-31a-006, T-31a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt
- Acceptance:
  - The Shikimori and Bangumi authorise builders accept `redirect_uri` as a parameter, so area 27 chooses the scheme once.
  - A redirect with `code` calls the matching login; a redirect without `code` logs out (TrackLoginActivity.kt:33-44, 59-70; FM-06).
  - Return-to-settings behaviour is area 27's (BaseOAuthLoginActivity.kt:29-36).

### T-31a-039 — OAuth client configuration and the ownership decision
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt
- Acceptance:
  - `TrackClientConfig { shikimori, bangumi }` loads from the user config (`[track.clients]`); absent entries disable login with `NotConfigured` (FM-05; TS-X-01 context).
  - The Aniyomi client id and secret values are not present in any Kurobayomi source file (a literal grep for the Aniyomi client id and secret values returns nothing).
  - The decision in p1-behavior.md D-OPEN-1 is recorded by the user before release (open question).

### T-31a-040 — Shared HTTP client usage, headers and cache policy
- Priority: P1
- Effort: S
- Depends: area:12-network-stack
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiInterceptor.kt
- Acceptance:
  - Both trackers use the single `reqwest::Client` from area 12 and do not create their own.
  - Every GET sends `Cache-Control: no-cache` (D12). Kotlin's default 10-minute cache (Requests.kt:16) is not used.
  - `user_agent()` returns `Kurobayomi/<version> (linux) (<repository>)` (TS-X-11).
  - Logs contain no bearer or refresh token (TS-X-12).

### T-31a-041 — Tracker error model and message mapping
- Priority: P1
- Effort: M
- Depends: area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/HttpException.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt
- Acceptance:
  - `TrackError` gains `NotAuthenticated`, `AuthExpired`, `HttpStatus`, `TooManyEntries`, `NotFound`, `UnknownStatus`, `NoLibraryId`, `NotConfigured`, `Decode` (p1-rust-design.md section 10).
  - Message mapping: `HttpStatus(401)` on an authenticated session → auth-expired key; `HttpStatus(n)` → `MR.strings.exception_http`; `AuthExpired` → `track.err.auth_expired` with a log-in action (D1); other variants → `track_error` with a detail key (p1-rust-design.md section 10).
  - No variant text is shown raw (the Kotlin shows raw English; this is a deliberate change).
  - `UnknownStatus` never panics (D4; TS-S-17).

### T-31a-042 — Remove-tracking remote option for Shikimori
- Priority: P1
- Effort: S
- Depends: T-31a-015, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - The "Also remove from Shikimori" checkbox is shown only for Shikimori (`is DeletableMangaTracker`), not for Bangumi (AnimeTrackInfoDialog.kt:858-860).
  - The remote call is fire-and-forget; failures are logged (AnimeTrackInfoDialog.kt:862-868). Kurobayomi shows a toast on failure (proposal, consistent with T-31a-041).

### T-31a-043 — Lenient decoding rules for both trackers
- Priority: P1
- Effort: S
- Depends: area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/dto/BGMSearch.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/dto/SMEntry.kt
- Acceptance:
  - Decoder configuration: unknown keys ignored; absent nullable keys decode as `None`; absent non-null keys without defaults fail (matches `ignoreUnknownKeys = true, explicitNulls = false`, AppModule.kt:173-177).
  - `NaN` and infinity are rejected after decoding (p1-rust-design.md 4.3).
  - Test: a JSON with an extra key and a missing optional key decodes for every DTO (TS-X-09 context).

### T-31a-044 — Bangumi anime progress through the episode endpoints (P2, D7)
- Priority: P2
- Effort: M
- Depends: T-31a-030, T-31a-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/Bangumi.kt
- Acceptance:
  - Behind a setting (default off until verified on a live account, T-31a-046), anime progress is written through `PATCH /v0/users/-/collections/{subject_id}/episodes` with `episode_id` (array of ids) and `type: 2` (done) after resolving ids with `GET /v0/episodes?subject_id=<subject_id>`.
  - With the setting off, behaviour is the parity behaviour in T-31a-030.
  - UI shows "unverified" next to the Bangumi anime progress while the setting is off (D7).

### T-31a-045 — Anonymous search for both trackers (P2, D9)
- Priority: P2
- Effort: S
- Depends: T-31a-009, T-31a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt
- Acceptance:
  - When logged out, search uses the plain client with no `Authorization` header (TS-S-03; OQ-5 accepted).
  - When logged in, the bearer is still sent, so account-specific filters keep working.
  - Parity behaviour is recorded in the test as the baseline (logged-out search fails in the Kotlin, EC-S-01, EC-B-01).

### T-31a-046 — Live verification of remote behaviour before release
- Priority: P1
- Effort: S
- Depends: T-31a-019, T-31a-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiInterceptor.kt
- Acceptance:
  - On a test Shikimori account: a POST to user_rates on an existing entry returns the existing id (upsert, verified server-side already, confirm once); a refresh returns a new refresh token and the old one stops working (OQ-6).
  - On a test Bangumi account: the refresh response includes or omits `user_id` and the decode still succeeds (OQ-7); a PATCH with `ep_status` on an anime subject is recorded as success or failure (D7 evidence for T-31a-044).
  - Results are recorded in p1-topic-remote-api-verification.md section 6 with the date and the account type used.
  - Requires the user's account credentials; this task is not done by an agent.

### T-31a-047 — Logo assets for both trackers
- Priority: P2
- Effort: S
- Depends: T-31a-037
- Sources: app/src/main/res/drawable-nodpi/ic_tracker_shikimori.webp, app/src/main/res/drawable-nodpi/ic_tracker_bangumi.webp
- Acceptance:
  - 48 px PNG versions are embedded through `include_bytes!` (p1-rust-design.md section 7).
  - Before shipping, the user decides whether the provider marks may be redistributed (OQ-3). If not, a text tile with the initial (as the mockup does) is used.

### T-31a-048 — Backup and restore compatibility for tracker ids and status numbers
- Priority: P1
- Effort: S
- Depends: T-31a-001, T-31a-020, area:21-backup-format-and-create, area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupTracking.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnimeTracking.kt
- Acceptance:
  - A backup written by Aniyomi with `syncId` 4 or 5 restores to Shikimori or Bangumi (p1-data-and-config.md section 6).
  - Status numbers in backups are restored unchanged (Shikimori 1..6, Bangumi 1..5).
  - Tokens and usernames are not in backups; restore reports both trackers as "log in again" (area 22).
  - Test fixture: an Aniyomi backup with one Shikimori and one Bangumi track restores with `remote_id`, `library_id` (Shikimori), `status`, `score`, dates and `private`.

### T-31a-049 — Android-only plumbing in scope: drop list and replacements (P3)
- Priority: P3
- Effort: S
- Depends: T-31a-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/Bangumi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiInterceptor.kt
- Acceptance:
  - The Android-only mechanisms in p1-rust-design.md section 9 are replaced, not ported: `BuildConfig` (replaced by `env!`), `android.graphics.Color` (replaced by a `u32` constant), moko `StringResource` (replaced by `MsgKey`), injekt `injectLazy` (replaced by constructor injection), `withIOContext` (tokio), `logcat` (`tracing`).
  - The Kotlin `jmir1/Aniyomi` user-agent string is not reproduced (T-31a-040 replaces it).
  - No behaviour is dropped by this task. The list of dropped behaviour is empty for this area.

---

## File-to-task coverage

| File (repo-relative, under app/src/main/java/eu/kanade/tachiyomi/data/track/) | Tasks |
|---|---|
| bangumi/Bangumi.kt | T-31a-020, 021, 022, 031, 032, 033, 034, 035, 037, 049 |
| bangumi/BangumiApi.kt | T-31a-022, 026, 027, 028, 029, 030, 036, 040, 044 |
| bangumi/BangumiInterceptor.kt | T-31a-025, 027, 034, 040, 049 |
| bangumi/BangumiUtils.kt | T-31a-021 |
| bangumi/dto/BGMCollectionResponse.kt | T-31a-021, 023 |
| bangumi/dto/BGMOAuth.kt | T-31a-025 |
| bangumi/dto/BGMSearch.kt | T-31a-024, 036, 043 |
| bangumi/dto/BGMUser.kt | T-31a-026 |
| shikimori/Shikimori.kt | T-31a-001, 002, 003, 011, 012, 013, 014, 015, 016, 018, 035, 049 |
| shikimori/ShikimoriApi.kt | T-31a-003, 006, 009, 010, 011, 015, 019, 039, 045 |
| shikimori/ShikimoriInterceptor.kt | T-31a-005, 008, 040, 049 |
| shikimori/ShikimoriUtils.kt | T-31a-002 |
| shikimori/dto/SMAddEntryResponse.kt | T-31a-004, 011 |
| shikimori/dto/SMEntry.kt | T-31a-004, 017, 043 |
| shikimori/dto/SMOAuth.kt | T-31a-005 |
| shikimori/dto/SMUser.kt | T-31a-004, 006 |
| shikimori/dto/SMUserListEntry.kt | T-31a-004, 010 |

Cross-cutting: TrackerManager registration (T-31a-001, 020, 037), settings and login rows (T-31a-037), redirect handling (T-31a-038), defects D1 to D22 (T-31a-005, 008, 016, 017, 029, 040, 041, 043, 009, 011, 013, 033, 034, 045, 044, 046, 035, 014, 049, 022, 003, 042). D6 is withdrawn after verification (p1-behavior.md section 8), so no task. D21 and D20 are covered by T-31a-043 and T-31a-017, T-31a-024.
