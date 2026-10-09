# Area 29-tracker-myanimelist, part 1: implementation tasks

Priority rule for this area: tracking is P1 parity (area:27-tracking-core). No MAL task is P0. Android-only items are P3 with a drop decision. Polish and optional consumers are P2.

Path prefix used below: `MAL/` means `app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/`.

### T-29a-001 — Tracker module skeleton, constants and tracker id
- Priority: P1
- Effort: S
- Depends: area:27-tracking-core
- Sources: MAL/MyAnimeList.kt, MAL/MyAnimeListApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt
- Acceptance:
  - `kuro-track::mal` exists with the module list in `p1-rust-design.md` section 1 and compiles with no GTK dependency.
  - `MAL_TRACKER_ID` equals 1 and the constants `CLIENT_ID` (from config, not hard-coded), `OAUTH_BASE`, `API_BASE`, `PAGE_SIZE = 250`, `QUERY_MAX = 64` match `p1-behavior.md` section 0.
  - A unit test asserts the id and the two base URLs as strings.

### T-29a-002 — Status codes and wire mapping (manga and anime)
- Priority: P1
- Effort: S
- Depends: T-29a-001
- Sources: MAL/MyAnimeList.kt, MAL/MyAnimeListUtils.kt
- Acceptance:
  - `MalStatus` has discriminants 1, 11, 2, 3, 4, 6, 16, 7, 17 exactly as `p1-data-and-config.md` section 1 lists.
  - `manga_wire` and `anime_wire` return the strings of `p1-behavior.md` section 1.3 for every code, including `None` for codes outside each list and the cross-mappings (anime 1 is `watching`, anime 6 is `plan_to_watch`, manga 7 is `reading`).
  - `from_wire` maps every string in section 1.4 and maps unknown and absent values to 1.
  - Table test `status_*` in `p1-edge-cases-and-tests.md` section 5 passes.

### T-29a-003 — Score model (0 to 10, index, display)
- Priority: P1
- Effort: S
- Depends: T-29a-001
- Sources: MAL/MyAnimeList.kt
- Acceptance:
  - `SCORE_LIST` is `"0"` to `"10"` (11 entries) and `index_to_score(i) == i as f64`.
  - `display_score(8.9)` returns `"8"` (truncation, as `toInt()`), and `display_score(0.0)` returns `"0"`.
  - `score_from_label("11")` returns `None`, not -1 (behaviour D-12).
  - `wire_score(8.0)` returns `"8.0"` by default (D-11 default); a feature switch selects the integer form and is covered by a test.

### T-29a-004 — Credential and token persistence
- Priority: P1
- Effort: M
- Depends: T-29a-001, area:02-preferences-system
- Sources: MAL/MyAnimeList.kt, MAL/MyAnimeListInterceptor.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt, core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt
- Acceptance:
  - The token JSON (`MalToken`, fields in `p1-data-and-config.md` section 3.1) is stored in the Secret Service item with attributes `app=kurobayomi, tracker=myanimelist, kind=oauth-token`, and in the 0600 fallback file only when no secret service is available.
  - The username and the auth-expired flag live in `$XDG_CONFIG_HOME/kurobayomi/tracking/myanimelist.toml`. The token never appears in that file (a test reads the file and asserts no `access_token` substring).
  - Clearing the token removes the item or file. No `"null"` string is ever written (D-07).
  - Fallback path is created with directory mode 0700 and file mode 0600 (test checks mode bits).

### T-29a-005 — PKCE verifier and authorize URL
- Priority: P1
- Effort: S
- Depends: T-29a-001
- Sources: MAL/MyAnimeListApi.kt, app/src/main/java/eu/kanade/tachiyomi/util/PkceUtil.kt
- Acceptance:
  - `new_verifier()` returns 68 characters from the base64url alphabet with no padding, from 50 bytes of `OsRng`.
  - `authorize_url(client_id, verifier)` returns `https://myanimelist.net/v1/oauth2/authorize?client_id=<id>&code_challenge=<verifier>&response_type=code` with keys in this order and no `redirect_uri` or `code_challenge_method` key.
  - Calling it twice produces two different verifiers (E-LOGIN-4 precondition).

### T-29a-006 — Login completion (exchange, user, save, failure rules)
- Priority: P1
- Effort: M
- Depends: T-29a-004, T-29a-005, T-29a-007, T-29a-009, T-29a-010, T-29a-039
- Sources: MAL/MyAnimeList.kt, MAL/MyAnimeListApi.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt
- Acceptance:
  - `login_with_code` POSTs to `https://myanimelist.net/v1/oauth2/token` with form keys `client_id`, `code`, `code_verifier`, `grant_type=authorization_code` in that order, then stores the token, then calls `/v2/users/@me` and stores the username.
  - On success the auth-expired flag is false and `is_logged_in` is true (test with the fixture server).
  - On exchange failure the function returns `Err` and keeps an existing login if one existed; without an existing login it logs out (matches `MyAnimeList.kt:257-259` on the no-prior-login path). Behaviour change D-03 is covered by a test.
  - Empty username from `/users/@me` is rejected with `MAL: User is not authenticated` (E-AUTH-13).

### T-29a-007 — Token model and expiry rule
- Priority: P1
- Effort: S
- Depends: T-29a-001
- Sources: MAL/dto/MALOAuth.kt
- Acceptance:
  - `MalToken` deserialises `token_type`, `refresh_token`, `access_token`, `expires_in`, and an optional `created_at` that defaults to the parse time.
  - `is_expired(now)` is `created_at + max(expires_in - 60, 0)` less than `now`, strictly (test: expires_in 100, created 0, expired at 41 and not at 40).
  - Margin clamp for short lifetimes (D-19): margin is `min(60, expires_in / 2)`, tested with expires_in 30.

### T-29a-008 — Token refresh with single-flight lock and expired flag
- Priority: P1
- Effort: M
- Depends: T-29a-004, T-29a-007
- Sources: MAL/MyAnimeListInterceptor.kt
- Acceptance:
  - `MalAuth::bearer` returns `MAL: Login has expired` when the flag is set and makes no network call.
  - Ten concurrent calls on an expired token issue exactly one refresh POST (fixture server counts requests).
  - Refresh 401 sets the flag and returns `LoginExpired`; refresh 400 or network error returns `RefreshFailed` with the flag unchanged.
  - Logout during a refresh leaves no token after both settle (test `logout_during_refresh`).
  - The refresh request carries `Authorization: Bearer <old access token>` and the form keys `client_id`, `refresh_token`, `grant_type=refresh_token` in order.

### T-29a-009 — Authenticated request path and HTTP client
- Priority: P1
- Effort: S
- Depends: T-29a-008, area:12-network-stack
- Sources: MAL/MyAnimeListInterceptor.kt, MAL/MyAnimeListApi.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt
- Acceptance:
  - Every API call except the token exchange sends `Authorization: Bearer <access>`; the token exchange sends no Authorization header.
  - The client has connect timeout 30 s, read timeout 30 s, total timeout 120 s, no cookie store, and no HTTP cache (assertions on a test server: no `Cookie` header, no `Cache-Control` header on requests).
  - Non-2xx responses surface as `Http(code)` with Display `HTTP error <code>`.

### T-29a-010 — Current user request
- Priority: P1
- Effort: S
- Depends: T-29a-009
- Sources: MAL/MyAnimeListApi.kt, MAL/dto/MALUser.kt
- Acceptance:
  - GET `https://api.myanimelist.net/v2/users/@me` returns `name`; a missing `name` is a decode error.
  - Test fixture `{"name":"fixture_user"}` returns `fixture_user`.

### T-29a-011 — Manga text search
- Priority: P1
- Effort: M
- Depends: T-29a-009, T-29a-016, T-29a-037
- Sources: MAL/MyAnimeListApi.kt, MAL/dto/MALSearch.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt
- Acceptance:
  - GET `/v2/manga` with `q=<query truncated to 64 UTF-16 units>` then `nsfw=true`, in that order.
  - Details are fetched for each hit with bounded concurrency (T-29a-037), and results keep the search order.
  - Results whose `publishing_type` contains `novel` (case-sensitive) are removed (E-SEARCH-10, E-SEARCH-11).
  - A query of 80 characters sends a 64-character `q` (test with an emoji at the boundary).

### T-29a-012 — Anime text search (single q parameter)
- Priority: P1
- Effort: S
- Depends: T-29a-017, T-29a-009
- Sources: MAL/MyAnimeListApi.kt, MAL/dto/MALSearch.kt
- Acceptance:
  - GET `/v2/anime` sends exactly one `q` parameter (64 units, D-09), then `nsfw=true`.
  - No novel filter is applied.
  - A test asserts the query string contains one `q=`.

### T-29a-013 — id: prefix direct lookup
- Priority: P1
- Effort: S
- Depends: T-29a-016, T-29a-017
- Sources: MAL/MyAnimeList.kt
- Acceptance:
  - `id:42` returns exactly one result from the details endpoint for the manga or anime search.
  - `id:abc` and `id:` fall through to text search (E-SEARCH-3, E-SEARCH-4).
  - `id:-7` sends the request and reports `HTTP error 404` (E-SEARCH-5).

### T-29a-014 — my: list search for manga
- Priority: P1
- Effort: M
- Depends: T-29a-016, T-29a-036, T-29a-037
- Sources: MAL/MyAnimeListApi.kt, MAL/dto/MALUserListSearch.kt, MAL/MyAnimeList.kt
- Acceptance:
  - Pages are requested from `/v2/users/@me/mangalist` with `fields=list_status{start_date,finish_date}`, `limit=250`, and `offset` only when greater than 0 (offsets 0 then 250 in the 251-entry fixture).
  - Titles are matched with case-insensitive `contains`; empty query matches all entries.
  - Results are in page order; `paging.next` empty or absent ends the paging.
  - Results carry details only; the row shows no fake status (UI, T-29a-049).

### T-29a-015 — my: list search for anime (endpoint fix)
- Priority: P1
- Effort: S
- Depends: T-29a-014
- Sources: MAL/MyAnimeListApi.kt, MAL/MyAnimeList.kt
- Acceptance:
  - Anime `my:` search requests `/v2/users/@me/animelist` (D-01 fix), with the same parameters and paging as T-29a-014.
  - A fixture test asserts the path. A live check (T-29a-058) confirms the response shape before release.

### T-29a-016 — Manga details and search hit mapping
- Priority: P1
- Effort: S
- Depends: T-29a-009, T-29a-036
- Sources: MAL/MyAnimeListApi.kt, MAL/dto/MALManga.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt
- Acceptance:
  - GET `/v2/manga/{id}` with the exact `fields` string from `p1-data-and-config.md` E7.
  - Mapping: `remote_id`, `title`, `summary` (synopsis), `total_chapters`, `score` (mean, -1.0 when absent), `cover_url` (large or `""`), `tracking_url = https://myanimelist.net/manga/{id}`, `publishing_status` and `publishing_type` with `_` replaced by a space, `start_date` (or `""`).
  - Fixture `manga_details_2.json` yields score -1.0, cover `""`, start date `""`.

### T-29a-017 — Anime details and search hit mapping
- Priority: P1
- Effort: S
- Depends: T-29a-009, T-29a-036
- Sources: MAL/MyAnimeListApi.kt, MAL/dto/MALAnime.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/model/AnimeTrackSearch.kt
- Acceptance:
  - GET `/v2/anime/{id}` with the exact `fields` string from E8.
  - Mapping as T-29a-016 with `total_episodes` from `num_episodes` and `tracking_url = https://myanimelist.net/anime/{id}`.
  - `num_episodes = 0` is kept as 0 (unknown) and disables completion checks (E-STAT-18).

### T-29a-018 — Remote list entry fetch (manga and anime)
- Priority: P1
- Effort: M
- Depends: T-29a-009, T-29a-036
- Sources: MAL/MyAnimeListApi.kt, MAL/dto/MALList.kt
- Acceptance:
  - GET `/v2/manga/{id}?fields=num_chapters,my_list_status{start_date,finish_date}` sets `total_chapters` on the track even when `my_list_status` is absent, and returns `None` in that case.
  - When present, status, `last_chapter_read`, score and dates are copied (`is_rereading` true gives Rereading; otherwise `from_wire`).
  - Anime uses `num_episodes` and `num_episodes_watched` (response key), with `is_rewatching` giving Rewatching.
  - Absent start or finish date in the response leaves the local value unchanged.

### T-29a-019 — Date conversion (local calendar, empty clears, partial dates)
- Priority: P1
- Effort: S
- Depends: T-29a-001
- Sources: MAL/MyAnimeListApi.kt
- Acceptance:
  - `to_mal_date(0)` returns `Some("")`; `to_mal_date(ms)` returns the local `yyyy-MM-dd` (chrono Local).
  - `parse_mal_date` accepts `YYYY-MM-DD`, `YYYY-MM` (day 1), and `YYYY` (January 1) (D-08 fix); `2020-02-30` returns absent with a WARN log (E-DATE-3).
  - A test in a zone that skips local midnight (set `TZ` for a subprocess) returns a valid instant (E-DATE-4).
  - Round trip local date to epoch to local date is the identity for dates in 1970 to 2099.

### T-29a-020 — Manga list upsert (PUT my_list_status)
- Priority: P1
- Effort: M
- Depends: T-29a-002, T-29a-003, T-29a-019, T-29a-009
- Sources: MAL/MyAnimeListApi.kt, MAL/dto/MALList.kt
- Acceptance:
  - PUT `/v2/manga/{remote_id}/my_list_status` form keys in order: `status`, `is_rereading`, `score`, `num_chapters_read`, then `start_date` and `finish_date` only when non-None.
  - `status` defaults to `reading` for unknown codes, but PUT is refused for codes outside the manga list (E-STAT-2, D-23 proposal).
  - `num_chapters_read` for 12.7 is `12`.
  - Response is decoded as `MALListMangaItemStatus` and applied to the track as in `parseMangaItem`.

### T-29a-021 — Anime list upsert (PUT my_list_status)
- Priority: P1
- Effort: S
- Depends: T-29a-020
- Sources: MAL/MyAnimeListApi.kt, MAL/dto/MALList.kt
- Acceptance:
  - PUT `/v2/anime/{remote_id}/my_list_status` form keys in order: `status`, `is_rewatching`, `score`, `num_watched_episodes`, then optional dates.
  - Default status `watching`; code 1 on anime sends `watching` (E-STAT-1).
  - Response key `num_episodes_watched` is read; the form key is `num_watched_episodes` (test asserts both).

### T-29a-022 — Remote delete (manga and anime)
- Priority: P1
- Effort: S
- Depends: T-29a-009
- Sources: MAL/MyAnimeListApi.kt, MAL/MyAnimeList.kt
- Acceptance:
  - DELETE `/v2/manga/{remote_id}/my_list_status` and `/v2/anime/{remote_id}/my_list_status` with Bearer auth.
  - A 404 returns `Http(404)` and the caller logs at WARN without a toast (E-DEL-1).

### T-29a-023 — Progress-driven status transitions in update
- Priority: P1
- Effort: M
- Depends: T-29a-020, T-29a-021
- Sources: MAL/MyAnimeList.kt
- Acceptance:
  - For manga with `did_read = true`, a status other than Completed moves to Completed when `last_chapter_read as i64 == total_chapters && total_chapters > 0` (sets finished date to now), else to Reading unless Rereading (sets started date to now when last read equals 1.0).
  - Anime follows the same rules with Watching, Rewatching, `last_episode_seen`, `total_episodes`, and started and finished watching dates.
  - Table tests in `p1-edge-cases-and-tests.md` section 5 (`update_*`) pass, including the case of a re-read that reaches the last chapter becoming Completed.
  - Per-title PUT serialisation is covered by T-29a-055.

### T-29a-024 — setRemote defaults for MAL
- Priority: P1
- Effort: S
- Depends: T-29a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt
- Acceptance:
  - `set_remote_status(COMPLETED)` with a non-zero total sets progress to the total before PUT.
  - `set_remote_last_chapter_read(n)` sets Reading when progress was 0 and `n > 0` and status is not Rereading, and sets Completed plus finished date when `n == total` (default behaviour of `MangaTracker.kt:74-89`).
  - `set_remote_score("7")` stores 7.0; `set_remote_score("foo")` leaves the score unchanged and returns an error (D-12).
  - Each setter ends with the same PUT path as the update flow.

### T-29a-025 — bind for new and existing titles
- Priority: P1
- Effort: M
- Depends: T-29a-018, T-29a-020, T-29a-021
- Sources: MAL/MyAnimeList.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt
- Acceptance:
  - Not on list: manga status is Reading if local chapters are read else PlanToRead, score 0.0, PUT created. Anime uses Watching or PlanToWatch.
  - On list: status is kept unless it is not Completed and not Rereading/Rewatching and local progress exists, in which case it becomes Reading/Watching (D-16 documented); then PUT with `did_read = false`.
  - No self-copy side effect: the copy step is either removed or proven a no-op by a test (D-02).
  - Test cases `bind_manga_found`, `bind_manga_not_found`, `bind_manga_rereading_kept`, `bind_anime_found_watch` pass.

### T-29a-026 — refresh (find or re-add)
- Priority: P1
- Effort: S
- Depends: T-29a-018, T-29a-020, T-29a-021
- Sources: MAL/MyAnimeList.kt
- Acceptance:
  - `refresh` returns the list state when present, otherwise PUTs the local state and returns it (E-STAT-17).
  - When it re-adds, a toast "Re-added to MyAnimeList" is emitted (D-10 proposal); test asserts the event.

### T-29a-027 — Logout and auth-expired reset
- Priority: P1
- Effort: S
- Depends: T-29a-004, T-29a-008
- Sources: MAL/MyAnimeList.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt
- Acceptance:
  - Logout clears username, token, and the auth-expired flag, and no request is sent during logout.
  - After logout `is_logged_in` is false and API calls fail with `MAL: User is not authenticated`.
  - Login success resets the expired flag to false (test).

### T-29a-028 — Tracker capability flags
- Priority: P1
- Effort: S
- Depends: T-29a-001
- Sources: MAL/MyAnimeList.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt
- Acceptance:
  - `supports_reading_dates()` is true and `supports_private_tracking()` is false.
  - `completion_status()` is 2 and `score_list()` is the 11-entry list.
  - Test asserts all four.

### T-29a-029 — Settings row for MAL login and logout
- Priority: P1
- Effort: S
- Depends: T-29a-006, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt
- Acceptance:
  - Row shows "MyAnimeList" with the MAL logo tile and checked state equal to `is_logged_in`.
  - Logged-out click opens the login flow (T-29a-039); logged-in click opens the logout dialog (T-29a-031 wording is shared).
  - When no client id is configured, the row shows "Set a MyAnimeList client id to log in" and does not start the flow (T-29a-040).

### T-29a-030 — Logo asset and brand colour
- Priority: P2
- Effort: S
- Depends: T-29a-001
- Sources: app/src/main/res/drawable-nodpi/ic_tracker_mal.webp, MAL/MyAnimeList.kt
- Acceptance:
  - A 48 px PNG is bundled as a GResource from `ic_tracker_mal.webp` by the build step (area:01-foundation-app-bootstrap).
  - The logo tile background is `#2E51A2` (test: colour constant equals `Color.rgb(46, 81, 162)`).

### T-29a-031 — Delete dialog option for MAL
- Priority: P1
- Effort: S
- Depends: T-29a-022, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt
- Acceptance:
  - Title "Remove MyAnimeList tracking?", text "This will remove the tracking locally.", checkbox "Also remove from MyAnimeList" default off, buttons "Cancel" and "OK" (order as in source).
  - OK removes the local row, then issues the remote DELETE only when the checkbox is on.
  - A failed remote DELETE is logged at ERROR and does not show a toast.

### T-29a-032 — AniChart airing lookup uses MAL id (consumer)
- Priority: P2
- Effort: S
- Depends: area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt
- Acceptance:
  - For a MAL track the airing lookup posts the AniList GraphQL `Media(idMal:<id>,type: ANIME) { id }` query and uses the result id.
  - A non-numeric response returns 0 (as the Kotlin does at `AniChartApi.kt:72-74`); the parse is done with a JSON parser, not by substring.

### T-29a-033 — AniSkip lookup uses MAL id (consumer)
- Priority: P2
- Effort: S
- Depends: area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - For a MAL track the skip-times lookup uses `remote_id` as the MAL id.
  - The lookup keeps the first matching track's result as the Kotlin does (`PlayerViewModel.kt:2044` returns from inside `map`). Documented as a quirk, with a test that a non-MAL first track returns nothing.

### T-29a-034 — Error messages (English literals, i18n-ready)
- Priority: P1
- Effort: S
- Depends: T-29a-001
- Sources: MAL/MyAnimeListInterceptor.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/HttpException.kt
- Acceptance:
  - `MalError` Display strings equal `MAL: User is not authenticated`, `MAL: Failed to refresh account token`, `MAL: Login has expired`, and `HTTP error <code>` (test `error_messages_exact`).
  - Each message is produced through one constant table so the i18n area can translate them later without code changes.

### T-29a-035 — Logging
- Priority: P1
- Effort: S
- Depends: T-29a-001, area:01-foundation-app-bootstrap
- Sources: MAL/MyAnimeList.kt, MAL/MyAnimeListInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt
- Acceptance:
  - Errors use `tracing::error!` with the message `Failed to update remote track data id=<id>` for progress updates.
  - Token values never appear in log output (test captures logs during login and refresh and asserts no `access_token` or token value).

### T-29a-036 — JSON strictness and DTO fields
- Priority: P1
- Effort: S
- Depends: T-29a-001
- Sources: MAL/dto/MALAnime.kt, MAL/dto/MALList.kt, MAL/dto/MALManga.kt, MAL/dto/MALOAuth.kt, MAL/dto/MALSearch.kt, MAL/dto/MALUser.kt, MAL/dto/MALUserListSearch.kt
- Acceptance:
  - Every DTO field from `p1-data-and-config.md` section 3 exists with the same optionality. Unknown keys are ignored.
  - `mean` absent gives -1.0; `synopsis` absent gives `""`; `main_picture` null is allowed.
  - Missing required keys produce a `Decode` error, not a panic.

### T-29a-037 — Bounded fan-out for detail requests
- Priority: P1
- Effort: S
- Depends: T-29a-011
- Sources: MAL/MyAnimeListApi.kt
- Acceptance:
  - Detail fetches run with `buffered(4)` (D-13): at most 4 requests in flight (test counts concurrent requests on the fixture server).
  - Output order equals input order.

### T-29a-038 — Persist the pending login verifier
- Priority: P2
- Effort: S
- Depends: T-29a-005, T-29a-004
- Sources: MAL/MyAnimeListApi.kt
- Acceptance:
  - The verifier is written to `pending_verifier` in the config file when the login starts and removed after the code is consumed or the login times out.
  - A restart between the two steps still completes the login (test: write, drop the tracker, reload, exchange succeeds).

### T-29a-039 — Redirect strategy: loopback listener with paste fallback
- Priority: P1
- Effort: M
- Depends: T-29a-005
- Sources: MAL/MyAnimeListApi.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt, app/src/main/AndroidManifest.xml
- Acceptance:
  - A tokio listener on `127.0.0.1:<port>` (port from config, default 8765) accepts one request on `/myanimelist-auth`, extracts `code`, answers with a page saying the tab can be closed, and stops. Timeout 5 minutes.
  - If the port cannot be bound, login still works through the paste field (E-LOGIN-6).
  - `parse_pasted` accepts a full redirect URL or a bare code and rejects other input (E-LOGIN-9).
  - Decision recorded: whether MAL accepts `localhost` with a port is answered by T-29a-058; if not, the custom-scheme option in `p1-rust-design.md` section 3.6 is implemented instead.

### T-29a-040 — OAuth client registration and configuration (user decision)
- Priority: P1
- Effort: S
- Depends: none
- Sources: MAL/MyAnimeListApi.kt, MAL/MyAnimeList.kt
- Acceptance:
  - Kurobayomi does not embed the Aniyomi client id `686b980ff4240fccce7f6a654cea07ce`. The id comes from `client_id` in the config file or `KUROBAYOMI_MAL_CLIENT_ID`.
  - Without an id the login flow is disabled with the message "Set a MyAnimeList client id to log in" (E-AUTH-11 family, test).
  - The user registers an application on MyAnimeList and records the redirect URI used by T-29a-039. Recording this is a manual step that the user performs; the plan does not assume the registration exists.

### T-29a-041 — Recorded fixtures and DTO tests
- Priority: P1
- Effort: M
- Depends: T-29a-036, T-29a-040
- Sources: MAL/dto/MALAnime.kt, MAL/dto/MALList.kt, MAL/dto/MALManga.kt, MAL/dto/MALOAuth.kt, MAL/dto/MALSearch.kt, MAL/dto/MALUser.kt, MAL/dto/MALUserListSearch.kt
- Acceptance:
  - `crates/kuro-track/tests/fixtures/mal/` contains every fixture named in `p1-edge-cases-and-tests.md` section 4.
  - The fixtures are replaced by responses recorded from a test account (names and shapes kept), and each file has a header comment in the test code stating the recording date.
  - Every fixture deserialises into its DTO in a test.

### T-29a-042 — Mapping, status, score and date unit tests
- Priority: P1
- Effort: S
- Depends: T-29a-002, T-29a-003, T-29a-019
- Sources: MAL/MyAnimeListUtils.kt, MAL/MyAnimeList.kt, MAL/MyAnimeListApi.kt
- Acceptance:
  - All tests named `status_*`, `score_*`, `date_*`, `pkce_*`, `token_*`, `form_*`, `search_routing_*`, `list_paging_offsets`, `parse_pasted_*` in `p1-edge-cases-and-tests.md` section 5 exist and pass.
  - `cargo test -p kuro-track` runs them with no network.

### T-29a-043 — HTTP integration tests with a fake server
- Priority: P1
- Effort: M
- Depends: T-29a-009, T-29a-008, T-29a-020, T-29a-025
- Sources: MAL/MyAnimeListApi.kt, MAL/MyAnimeListInterceptor.kt, MAL/MyAnimeList.kt
- Acceptance:
  - The integration tests in `p1-edge-cases-and-tests.md` section 5 (`login_*`, `refresh_*`, `logout_during_refresh`, `bind_*`, `update_*`, `refresh_readds_missing`, `delete_remote_404`, `search_*`, `request_headers`, `no_cookie_store`) pass against a local server.
  - The fake server is a local `tokio::net::TcpListener` or `httpmock` as a dev-dependency; no test contacts myanimelist.net.

### T-29a-044 — Android-only: TrackLoginActivity and deep-link intent filter (drop)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt, app/src/main/AndroidManifest.xml
- Acceptance:
  - Decision recorded: the Android activity and the `aniyomi://myanimelist-auth` intent filter are dropped. Replacement is T-29a-039.
  - No Rust code references `aniyomi://`.

### T-29a-045 — Android-only: BaseOAuthLoginActivity loading screen (drop)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt
- Acceptance:
  - Decision recorded: the full-screen `LoadingScreen` and `MainActivity` return are dropped. The Settings row shows a spinner during the exchange (T-29a-047).

### T-29a-046 — Android-only: backup does not carry MAL login (drop)
- Priority: P3
- Effort: S
- Depends: area:22-backup-restore
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - Decision recorded: MAL credentials are excluded from Kurobayomi backups, as the `__PRIVATE_` keys are excluded from Android backups.
  - After restoring a backup the MAL row shows logged out and the user is asked to log in again (test with a restored backup fixture from area:22-backup-restore).

### T-29a-047 — Settings row states: login in progress and expired login
- Priority: P1
- Effort: S
- Depends: T-29a-029, T-29a-027
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - While the loopback login waits, the row shows "Waiting for MyAnimeList..." with a Cancel action.
  - When the token is flagged expired the row shows "Log in again" as a warning and stays checked (E-UI-2).
  - Each state is covered by a widget test or a manual test listed in `p1-edge-cases-and-tests.md` section 7.

### T-29a-048 — MalTracker implements the area:27-tracking-core tracker trait
- Priority: P1
- Effort: M
- Depends: T-29a-006, T-29a-016, T-29a-017, T-29a-023, T-29a-025, T-29a-026, T-29a-022
- Sources: MAL/MyAnimeList.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/DeletableMangaTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/DeletableAnimeTracker.kt
- Acceptance:
  - `MalTracker` implements every method in `p1-rust-design.md` section 3.3 and both deletable traits.
  - The area:27-tracking-core `TrackerManager` lists the MAL tracker with id 1 and `get(1)` returns it.
  - Trait-object test: `Box<dyn TrackerService>` can call every method on a fake transport.

### T-29a-049 — Tracker search card fields for MAL
- Priority: P1
- Effort: S
- Depends: T-29a-016, T-29a-017, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/model/AnimeTrackSearch.kt, spec/entries.json
- Acceptance:
  - The MAL search card shows cover, title, type, status, start date, count, and mean score, and hides the author line (empty for MAL).
  - Rows from the `my:` search show no status and offer "Refresh" (`p1-ui-parity.md` section 7).
  - Replaces the mockup's fixed "Author name" and "Score: 8.4" text in `src/entry.rs` `tracker_search()`.

### T-29a-050 — Hide the private-track action for MAL
- Priority: P1
- Effort: S
- Depends: T-29a-028, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt, src/entry.rs
- Acceptance:
  - The "Track privately" icon button and the "Track privately" / "Track publicly" menu entries are absent for MAL (test in GTK or via the view model).
  - The mockup button in `src/entry.rs` `tracker_search()` is not shown when the selected tracker is MAL.

### T-29a-051 — Toasts for login outcomes
- Priority: P1
- Effort: S
- Depends: T-29a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Success shows no toast for the browser flow (matches source); a failed exchange shows its error text (D-03 proposal); a denied login shows "Login was not completed" and keeps the existing login (D-04).
  - The toast text for logout is "You are now logged out" (`logout_success`).

### T-29a-052 — Loopback and paste login UI
- Priority: P1
- Effort: M
- Depends: T-29a-039, T-29a-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - The GTK login dialog shows the authorise URL with Copy and Open buttons, a text field "Paste the redirect URL or code", and Cancel.
  - Pasting a valid redirect completes the login; invalid input shows "Paste the full redirect URL or the code" (E-LOGIN-9).
  - Cancel removes the pending verifier from the config file (T-29a-038).

### T-29a-053 — Expired-token recovery on API 401
- Priority: P2
- Effort: S
- Depends: T-29a-008, T-29a-009
- Sources: MAL/MyAnimeListInterceptor.kt
- Acceptance:
  - An API call that returns 401 triggers one refresh and one retry (proposal for race 1 in `p1-edge-cases-and-tests.md` section 2).
  - A second 401 after the retry returns `Http(401)` and does not loop.

### T-29a-054 — Refresh backoff after non-401 failure
- Priority: P2
- Effort: S
- Depends: T-29a-008
- Sources: MAL/MyAnimeListInterceptor.kt
- Acceptance:
  - After a refresh failure that is not 401, further refreshes are skipped for 30 s and the call returns `RefreshFailed` immediately (D-06 proposal).
  - Test with a fake clock: no second refresh request within 30 s, one after.

### T-29a-055 — Per-title PUT serialisation
- Priority: P2
- Effort: S
- Depends: T-29a-023
- Sources: MAL/MyAnimeList.kt, MAL/MyAnimeListApi.kt
- Acceptance:
  - Two progress PUTs for the same `remote_id` never run concurrently (test records max concurrency per id = 1).
  - PUTs for different titles still run concurrently.

### T-29a-056 — Skip progress PUT after a local delete
- Priority: P2
- Effort: S
- Depends: T-29a-022
- Sources: MAL/MyAnimeList.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt
- Acceptance:
  - A progress update for a local row that no longer exists does not PUT (race 4 proposal). Test: delete then update yields zero PUT requests.

### T-29a-057 — Gating by shared tracking preferences
- Priority: P1
- Effort: S
- Depends: T-29a-023, area:27-tracking-core, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - With `pref_auto_update_manga_sync_key` false no MAL PUT is sent from reading or watching.
  - `pref_auto_update_manga_on_mark_read` values `ALWAYS`, `ASK`, `NEVER` map to: send, ask, do not send (the ask dialog itself is area:27-tracking-core).
  - Defaults are `true` and `ALWAYS`.

### T-29a-058 — Live verification checklist against a test MAL account
- Priority: P1
- Effort: M
- Depends: T-29a-041, T-29a-040, T-29a-039
- Sources: MAL/MyAnimeListApi.kt, MAL/MyAnimeListInterceptor.kt
- Acceptance:
  - A written checklist is run against a test account and each result is recorded in the tests folder: duplicate `q` behaviour (D-09), partial date responses (D-08), `score` as `8.0` versus `8` (D-11), `score=-1` rejection (D-12), anime `animelist` endpoint shape (D-01), `paging` absent on last page (E-SEARCH-16), 404 on delete of missing entry, token refresh response shape, redirect acceptance of `localhost:<port>`, and rate-limit responses.
  - Each open decision in this list is closed with the observed result before the feature is marked done.

### T-29a-059 — Shared in-memory tracker state for the GTK tracking sheet
- Priority: P1
- Effort: S
- Depends: T-29a-048, T-29a-047
- Sources: src/entry.rs, app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt
- Acceptance:
  - The tracking sheet reads the MAL state from `MalTracker` through one shared handle (no global statics), and refreshes after each action.
  - Two open tracking sheets stay consistent after an action in one (manual test).
