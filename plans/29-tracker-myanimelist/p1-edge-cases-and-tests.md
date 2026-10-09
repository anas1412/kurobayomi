# Area 29-tracker-myanimelist, part 1: edge cases, failure modes, and tests

## 0. Upstream tests

No test file in the Aniyomi source covers the MAL code. The search `grep -rli myanimelist` over `*Test*.kt` returns nothing. There are no `*Test.kt` files in the scope (`p1-inventory.md`). Nothing is ported from upstream tests. All tests below are new.

## 1. Edge cases

Each row: trigger, what the Kotlin does (source), what Kurobayomi does. Where Kurobayomi differs, the difference is a proposal (`p1-behavior.md` section 13, D-nn).

### 1.1 Authentication and tokens

| ID | Trigger | Kotlin behaviour | Kurobayomi behaviour |
|---|---|---|---|
| E-AUTH-1 | Token expires during a list update | `intercept` refreshes once under lock, then proceeds (`MyAnimeListInterceptor.kt:24-26`) | `bearer()` refreshes under the mutex, then the request proceeds |
| E-AUTH-2 | Two requests find the token expired at once | Second waits on `synchronized(this)`, sees the new token (`:52`) | Second waits on the tokio mutex and re-checks the expiry under the lock |
| E-AUTH-3 | Refresh returns 401 | Sets auth-expired flag, throws `MAL: Login has expired` (`:60-63`) | Same. Flag persisted. Login row shows a warning (proposal) |
| E-AUTH-4 | Refresh returns 400 `invalid_grant` | Throws `MAL: Failed to refresh account token`, no flag (`:65-78`) | Same. No backoff in Kotlin (D-06). Proposal: 30 s backoff |
| E-AUTH-5 | Refresh network error | `catch (_: Throwable)` throws `MAL: Failed to refresh account token` (`:54-58`) | Same message. Transport error logged at WARN |
| E-AUTH-6 | Token lifetime shorter than 60 s | `adjustedExpiresIn` negative, always expired, refresh before every request (`MALOAuth.kt:22`) | Proposal D-19: margin = min(60, expires_in / 2) |
| E-AUTH-7 | Clock skew between device and MAL | `isExpired` uses local seconds (`MALOAuth.kt:24`) | Same. A skew of more than 60 s causes early or late refresh. Accept. |
| E-AUTH-8 | `created_at` absent in refresh response | Defaults to now at parse time (`MALOAuth.kt:18-19`) | Same, via `#[serde(default = "now_secs")]` |
| E-AUTH-9 | Token file holds `"null"` after logout | `loadOAuth` returns null (`MyAnimeList.kt:280-286`) | Kurobayomi never writes `"null"` (D-07). Missing secret means no token |
| E-AUTH-10 | Token JSON corrupt | `loadOAuth` returns null, user appears logged in but every call fails with `MAL: User is not authenticated` | Log at ERROR, treat as not authenticated. Show "Log in again" |
| E-AUTH-11 | Secret store unavailable | Not applicable (Android prefs) | Fall back to 0600 file. Log a WARN once. Never write the token to the config TOML |
| E-AUTH-12 | Password pref holds access token and token is refreshed | Password pref keeps the old token; `isLoggedIn` checks non-empty only (`MyAnimeList.kt:256`, `BaseTracker.kt:32-34`) | Kurobayomi keeps no password field. Username only |
| E-AUTH-13 | Username is empty after login | `saveCredentials("", token)` stores empty username; `isLoggedIn` returns false though the token works (`BaseTracker.kt:32-34`) | Reject login with "MAL: User is not authenticated" if username empty (proposal, same message as `NotAuthenticated`) |

### 1.2 Login flow

| ID | Trigger | Kotlin | Kurobayomi |
|---|---|---|---|
| E-LOGIN-1 | User denies consent at MAL | Redirect has no `code`, `logout()` clears any existing login (`TrackLoginActivity.kt:53-55`) | Keep existing login. Toast "Login was not completed" (D-04) |
| E-LOGIN-2 | Code exchange returns 400 (expired or reused code) | `login` swallows error, `logout()` (`MyAnimeList.kt:257-259`) | Toast "HTTP error 400". Logout only when there was no previous login. Proposal D-03 |
| E-LOGIN-3 | App restarted between "open browser" and redirect | Verifier is `""` after restart, exchange fails (`MyAnimeListApi.kt:390`) | Verifier persisted in config until consumed (T-29a-038). Restart keeps the flow |
| E-LOGIN-4 | User starts login twice | Second `authUrl()` overwrites verifier, first redirect fails (`MyAnimeListApi.kt:425-428`) | Keep only the latest verifier. Old redirect fails with "no pending login" (`MalError::NoPendingLogin`) |
| E-LOGIN-5 | Redirect arrives after timeout (5 min) | Not applicable | Listener closed. Redirect page shows "Login timed out. Start again in Kurobayomi." Paste fallback still works |
| E-LOGIN-6 | Loopback port in use | Not applicable | Show paste dialog only. Log WARN |
| E-LOGIN-7 | Browser cannot be launched | `openInBrowser` fails silently | Show the authorise URL in the dialog with a Copy button |
| E-LOGIN-8 | `user/@me` returns no `name` | `MALUser` decode fails, `logout()` (`MyAnimeList.kt:255-259`) | Same as E-AUTH-13 |
| E-LOGIN-9 | Pasted input is not a URL and not a code | Not applicable | `parse_pasted` returns error "Paste the full redirect URL or the code" |
| E-LOGIN-10 | Pasted URL has `error=access_denied` | Not applicable (handled by missing code) | Treat as denial (E-LOGIN-1) |

### 1.3 Status, score and progress

| ID | Trigger | Kotlin | Kurobayomi |
|---|---|---|---|
| E-STAT-1 | Anime row with manga code READING (1) | Sent as `watching` (`MyAnimeListUtils.kt:18`) | Same, in `anime_wire` |
| E-STAT-2 | Manga row with anime-only code (WATCHING 11) | `toMyAnimeListStatus` returns null, PUT uses `reading` (`MyAnimeListApi.kt:184`) | Refuse PUT, error "Unknown status 11 for manga" (D-23) |
| E-STAT-3 | Remote returns unknown status string `"considering"` | Mapped to READING (`MyAnimeListUtils.kt:37`) | Same (D-17), with WARN log |
| E-STAT-4 | Remote `is_rereading: true` with status `completed` | Local REREADING (flag wins, `MyAnimeListApi.kt:347`) | Same. Remote completed plus reread flag is an odd state; flag wins as in Kotlin |
| E-STAT-5 | Chapter read beyond total | `last_chapter_read.toLong() == total` only, so 12 of 10 never completes (`MyAnimeList.kt:128`) | Same |
| E-STAT-6 | Total chapters 0 (unknown, ongoing manga) | Never auto-completes (`total_chapters > 0` check, `MyAnimeList.kt:128`) | Same |
| E-STAT-7 | Read chapter 1.0 on a fresh title | `started_reading_date = now` (`MyAnimeList.kt:133-134`) | Same |
| E-STAT-8 | Chapter 1.5 (fractional) | `num_chapters_read=1` sent (`toInt`), local keeps 1.5 | Same truncation. Note the local and remote values differ until next refresh |
| E-STAT-9 | Score 0 | Sent `"0.0"`; displayed "0" (`MyAnimeList.kt:110`) | Sent per D-11 decision; displayed "0" |
| E-STAT-10 | Score -1 (search result) set by mistake | Sent `"-1.0"`, MAL rejects (400 likely) | Refuse (D-12) |
| E-STAT-11 | Score label not in list | Score -1.0 (`MangaTracker.kt:91-94`) | Error, no change |
| E-STAT-12 | Status COMPLETED then user reads more | `update` skips all logic (`MyAnimeList.kt:126`) | Same |
| E-STAT-13 | Completed via setRemoteLastChapterRead (chapter == total) | `finished_reading_date = now` then PUT (`MangaTracker.kt:85-86`) | Same |
| E-STAT-14 | `bind` on an ON_HOLD title with local reads | Forced to READING (`MyAnimeList.kt:177`) | Same, documented (D-16) |
| E-STAT-15 | `bind` on a COMPLETED title | Status unchanged, progress unchanged, PUT (`:175`) | Same |
| E-STAT-16 | Anime `bind` with `hasSeenEpisodes` and remote `plan_to_watch` | READING check uses WATCHING (`:197`), becomes WATCHING | Same |
| E-STAT-17 | `refresh` on a title removed on the website | Re-added silently (`MyAnimeList.kt:242`) | Same, plus toast (D-10 proposal) |
| E-STAT-18 | Total episodes from MAL is 0 (unaired or unknown) | `total_episodes != 0` guard (`AnimeTracker.kt:83`) | Same |

### 1.4 Dates

| ID | Trigger | Kotlin | Kurobayomi |
|---|---|---|---|
| E-DATE-1 | Start date 0 locally | `start_date=` sent, clears on MAL (`MyAnimeListApi.kt:370-372`) | Same |
| E-DATE-2 | MAL returns `start_date: "2020-05"` (partial) | `ParseException` propagates, call fails (`:366-368`) | Accept partial dates (D-08). YYYY-MM -> day 1, YYYY -> Jan 1 |
| E-DATE-3 | MAL returns `start_date: "2020-02-30"` | Lenient parse normalises to Mar 1 (`SimpleDateFormat` default lenient) | Chrono rejects, treated as absent (warn). Behaviour differs; document |
| E-DATE-4 | DST day in local zone | Local midnight may not exist (for example some zones skip 00:00) | `chrono::Local` picks the earliest valid time. Use `and_hms_opt(0,0,0)` then `earliest()`. Test in a zone that skips midnight |
| E-DATE-5 | Timezone changes between save and read | Epoch stays; calendar date may shift | Same. Documented |
| E-DATE-6 | Completion timestamp `now` | Not midnight, date is the local day of that instant | Same |
| E-DATE-7 | Date before 1970 | Negative epoch, formats fine | Same |
| E-DATE-8 | Year beyond 9999 from remote | Parse fails | Treat as absent |

### 1.5 Search

| ID | Trigger | Kotlin | Kurobayomi |
|---|---|---|---|
| E-SEARCH-1 | Query longer than 64 characters (manga) | `take(64)` (`MyAnimeListApi.kt:85`) | Same, counts UTF-16 units to match. Test with an emoji |
| E-SEARCH-2 | Query longer than 64 characters (anime) | Two `q` params, second is full (`:104-105`) | Single `q=take(64)` (D-09) |
| E-SEARCH-3 | `id:abc` | Falls through to text search (`MyAnimeList.kt:211`) | Same |
| E-SEARCH-4 | `id:` with empty id | Falls through to text search | Same |
| E-SEARCH-5 | `id:-7` | Negative int accepted, 404 from MAL | Same. Show "HTTP error 404" |
| E-SEARCH-6 | `my:` with empty title | Every entry matches, all details fetched (`MyAnimeListApi.kt:292`) | Same, capped in UI to first 200 (proposal). Show progress |
| E-SEARCH-7 | `my:` with title matching nothing | Empty list, no detail call | Same |
| E-SEARCH-8 | Anime `my:` | Uses manga list endpoint, 404 or wrong anime (D-01) | Fix to `animelist`. Verify against live account first |
| E-SEARCH-9 | One detail call fails in a search of 100 | `awaitAll` throws, whole search fails (`:93-94`) | Proposal: keep successful hits, show a warning count. Tests must cover both |
| E-SEARCH-10 | Manga search returns only novels | Empty list after filter (`:95`) | Same |
| E-SEARCH-11 | `publishing_type` contains "novel" in other spelling ("Novel") | Filter is case-sensitive, `"Novel"` is not filtered | Same, case-sensitive |
| E-SEARCH-12 | `q` with `&` or `#` | `Uri.Builder` encodes | `url::Url::query_pairs_mut` encodes. Test |
| E-SEARCH-13 | Search with no network | HTTP fails, `IOException` surfaces | Toast with error text. Retry button |
| E-SEARCH-14 | `my:` pagination with 251 entries | Two pages, `offset=250` second (`:298`) | Same. Test with fixture |
| E-SEARCH-15 | `paging.next` present but empty string | Treated as no next page (`isNullOrBlank`, `:297`) | Same |
| E-SEARCH-16 | `paging` key missing from response | Decode error (required field) | Proposal: `#[serde(default)]` on `paging`, treat as last page. Verify live |

### 1.6 Delete

| ID | Trigger | Kotlin | Kurobayomi |
|---|---|---|---|
| E-DEL-1 | Remote delete of a title not on the list | HTTP 404, logged, local still removed (`AnimeTrackInfoDialog.kt:863-866`) | Same. Log WARN, do not show toast |
| E-DEL-2 | Delete with checkbox off | Only local row removed | Same |
| E-DEL-3 | Remote delete and local delete race | `launchNonCancellable` in parallel (`:833-834`) | Same. Both independent |
| E-DEL-4 | Delete when logged out | `IOException` "not authenticated" logged | Same |

### 1.7 Network and concurrency

| ID | Trigger | Kotlin | Kurobayomi |
|---|---|---|---|
| E-NET-1 | Read timeout 30 s | `IOException` | Same |
| E-NET-2 | Call timeout 2 min | `IOException` | Same (`timeout(120s)`) |
| E-NET-3 | Response is 200 with HTML (captive portal) | JSON decode error | Map to `Decode`. Toast "unexpected response" |
| E-NET-4 | Rate limit 429 | `HTTP error 429` | Same. Proposal: no retry (MAL rate limit not documented here). Verify |
| E-NET-5 | Concurrent progress updates on the same title | Two PUTs, last wins | Same. Proposal: per-title mutex to serialise PUTs |
| E-NET-6 | GET cached by OkHttp | `max-age=600` hint (`Requests.kt:16`) | No HTTP cache in Kurobayomi for MAL. Reads are always fresh (Rust design section 8, client row) |
| E-NET-7 | Proxy or DoH setting | Shared client honours app DoH (`NetworkHelper.kt`) | Area:12 decides. MAL client uses system DNS unless area:12-network-stack provides its resolver |

### 1.8 UI-related

| ID | Trigger | Kotlin | Kurobayomi |
|---|---|---|---|
| E-UI-1 | Private-track button shown for MAL | Not verified in scope: `supportsPrivateTracking = false` (`BaseTracker.kt:25`) | Hide for MAL (see `p1-ui-parity.md` section 8). Test: MAL row has no private button |
| E-UI-2 | Logged-in state with expired flag | Row checked (`BaseTracker.kt:32-34`) | Row shows warning (proposal) |
| E-UI-3 | Logout while a PUT is in flight | Logout clears token; the PUT already sent completes | Same. The in-flight PUT completes with the old token. New calls fail |
| E-UI-4 | Settings page open in two windows | Not applicable | Config file read on each change via file watcher (proposal) or on window focus. Tested manually |

## 2. Race conditions and ordering

1. Refresh vs in-flight calls. Calls started before refresh keep the old access token. MAL accepts a token until it is revoked. On a 401 from an API call (not refresh), the call fails with `HTTP error 401` and the flag is not set (observed; `p1-behavior.md` section 4.8). Proposal: on a 401 from an API call, try one refresh and retry once. Test with a fake server that returns 401 once.
2. Login vs refresh. A login writes a new token while a refresh is pending. Kotlin has no guard. Kurobayomi: `set_token` takes the same mutex, so the later write wins. Test: login during refresh.
3. Logout vs refresh. Logout clears the token while refresh is running. Kotlin: refresh then writes the new token after logout (the logout is lost). Kurobayomi: `logout` takes the mutex. Test: logout during refresh leaves no token.
4. Progress update vs delete. A progress PUT lands after a DELETE, which re-creates the entry on MAL (MAL PUT upserts). Kotlin does the same. Proposal: skip PUT when the local row was deleted (check the row before PUT).
5. Two rapid chapter reads. `TrackChapter` runs one `async` per track. Two PUTs in flight for the same title can finish out of order. Last response wins for local row. Proposal: per-title mutex (E-NET-5).
6. Search cancelled by typing. The Kotlin search has no cancel. Kurobayomi: the older search future is dropped when a newer one starts (GTK side). Test: a late response for an old query is ignored.
7. Login verifier overwritten by a second "Log in" click (E-LOGIN-4). The first browser tab's redirect fails. Test as described.

## 3. Failure modes and what the user sees

| Failure | Kotlin sees | Kurobayomi shows |
|---|---|---|
| Wrong client id | HTTP error 400 or 401 at exchange, silent logout | Toast "HTTP error 400" in Settings. Row stays logged out |
| No network at login | `IOException` swallowed | Toast "network error: ..." |
| Token revoked on MAL website | HTTP 401 on next API call, error toast | Toast "HTTP error 401" and an inline "Log in again" action |
| MAL down | HTTP 5xx toast | Same with the status code |
| Secret store locked | Not applicable | Dialog "Unlock your keyring, then try again" |
| Corrupt token file | Silent "not authenticated" | Same, plus log |

## 4. Fixtures (synthetic, must be replaced by recorded responses)

The fixtures below are written from the DTO shapes in `p1-data-and-config.md` section 3, not from a live MAL response. Record real responses from a test account before relying on them (task T-29a-041). Store under `crates/kuro-track/tests/fixtures/mal/`.

- `oauth_token.json`: `{"token_type":"Bearer","expires_in":2592000,"access_token":"fixture-access","refresh_token":"fixture-refresh"}` (no `created_at`, as the token endpoint returns it).
- `oauth_token_with_created.json`: same plus `"created_at": 1700000000`.
- `user_me.json`: `{"name":"fixture_user"}`.
- `manga_search.json`: `{"data":[{"node":{"id":1}},{"node":{"id":2}}],"paging":{"next":"https://api.myanimelist.net/v2/manga?offset=100"}}`.
- `manga_details_1.json`: `{"id":1,"title":"Fixture Title","synopsis":"Text","num_chapters":0,"mean":8.27,"main_picture":{"large":"https://example.invalid/c.jpg"},"status":"finished","media_type":"light_novel","start_date":"2000-01-01"}`. Used to test the novel filter.
- `manga_details_2.json`: `{"id":2,"title":"Fixture Manga","synopsis":"","num_chapters":10,"status":"currently_publishing","media_type":"manga"}` (no mean, no main_picture, no start_date). Tests the defaults `-1.0`, `""`, `null`.
- `anime_details_1.json`: `{"id":10,"title":"Fixture Show","synopsis":"","num_episodes":12,"mean":7.5,"main_picture":null,"status":"finished_airing","media_type":"tv","start_date":"2020-04"}`. Tests partial date (E-DATE-2).
- `manga_list_entry_present.json`: `{"num_chapters":10,"my_list_status":{"is_rereading":false,"status":"reading","num_chapters_read":3.0,"score":7,"start_date":"2021-01-05","finish_date":null}}`.
- `manga_list_entry_absent.json`: `{"num_chapters":10}` (no `my_list_status`).
- `anime_list_entry_present.json`: `{"num_episodes":12,"my_list_status":{"is_rewatching":true,"status":"watching","num_episodes_watched":4.0,"score":0,"start_date":null,"finish_date":null}}`. Note the response key `num_episodes_watched`.
- `mangalist_page1.json`: 250 entries with `node{id,title}` and `paging.next` set, for `offset` tests.
- `mangalist_page2.json`: fewer entries, `paging:{}` with `next` absent (tests E-SEARCH-16).
- `put_manga_response.json`: `{"is_rereading":false,"status":"completed","num_chapters_read":10.0,"score":9,"start_date":"2021-01-05","finish_date":"2021-03-01"}`.
- `error_401.json`, `error_400_invalid_grant.json`: MAL error bodies, not parsed, recorded for logs.

## 5. Tests to write

Unit tests (`crates/kuro-track/src/mal/`), no network:

| Test | Input | Expected |
|---|---|---|
| `status_manga_wire_roundtrip` | each manga code in `MANGA_LIST` | `from_wire(manga_wire(s))` equals `s` for Reading, Completed, OnHold, Dropped, PlanToRead. Rereading maps to `reading` (not round-trip, flag carries it) |
| `status_anime_wire_roundtrip` | each anime code | same for Watching, Completed, OnHold, Dropped, PlanToWatch |
| `status_anime_reading_is_watching` | code 1 on anime | `anime_wire` = `watching` (E-STAT-1) |
| `status_anime_plan_to_read_is_plan_to_watch` | code 6 on anime | `plan_to_watch` |
| `status_unknown_wire_is_reading` | `"considering"`, `None`, `""` | 1 |
| `status_manga_unknown_code_has_no_wire` | 11 on manga | `None` (E-STAT-2) |
| `status_labels` | every code in both lists plus 1 on anime | labels as listed in `p1-behavior.md` section 1.5, anime Reading has none |
| `score_index_roundtrip` | "0".."10" | `index_to_score(i)` == i, `display_score(i)` == "i" |
| `score_unknown_label` | "11", "" | `None` (not -1) (E-STAT-11) |
| `score_wire_default` | 8.0, 0.0 | "8.0", "0.0" (D-11 default) |
| `score_display_truncates` | 7.9 | "7" |
| `date_zero_is_empty_clear` | 0 | `Some("")` (E-DATE-1) |
| `date_roundtrip_local` | 2021-01-05 local midnight | "2021-01-05" both ways |
| `date_partial_year_month` | "2020-05" | 2020-05-01 (after D-08 fix) |
| `date_partial_year` | "2020" | 2020-01-01 |
| `date_invalid_is_absent` | "2020-02-30" | absent + WARN (E-DATE-3, behaviour documented) |
| `date_dst_gap_midnight` | a zone where 00:00 does not exist on the day | a valid instant on that day (E-DATE-4), using a fixed zone in test via `TZ` env in a subprocess |
| `pkce_verifier_shape` | `new_verifier()` | length 68, base64url alphabet, no padding |
| `pkce_verifier_unique` | 1000 calls | no duplicates |
| `authorize_url_order` | verifier "abc" | `https://myanimelist.net/v1/oauth2/authorize?client_id=...&code_challenge=abc&response_type=code` in this order, no `redirect_uri` |
| `token_expiry_margin` | expires_in 100, created_at 0 | margin 40 s: not expired when now = 40, expired when now = 41 (strict `<`, `MALOAuth.kt:24`) |
| `token_expiry_short_lifetime` | expires_in 30 | margin clamped (D-19 fix) |
| `token_created_default` | JSON without `created_at` | created_at set to parse time |
| `error_messages_exact` | each `MalError` variant | Display equals the Kotlin strings in `p1-behavior.md` section 8.1 |
| `form_order_manga_put` | a `MangaPut` with dates | field names in order status, is_rereading, score, num_chapters_read, start_date, finish_date |
| `form_order_anime_put` | `AnimePut` | status, is_rewatching, score, num_watched_episodes, start_date, finish_date |
| `form_omits_dates_on_none` | `start_date: None` | no `start_date` key |
| `form_empty_date_present` | `start_date: Some("")` | `start_date=` present |
| `search_routing_id` | `id:42`, `id:abc`, `id:` | single detail call, text search, text search |
| `search_routing_my` | `my:` and `my:Title` | list routing, empty title matches all |
| `search_anime_single_q` | 100-character query | one `q` param of 64 characters (D-09) |
| `search_manga_novel_filter` | fixture with `light_novel` and `manga` | only `manga` kept |
| `list_paging_offsets` | 251 entries across two pages | offsets 0 then 250, results in page order |
| `parse_pasted_url` | a full redirect URL with `code=` | the code |
| `parse_pasted_bare` | a bare code | the code |
| `parse_pasted_bad` | "hello" | error (E-LOGIN-9) |
| `token_store_no_null_string` | logout | secret store empty, never the string `"null"` (D-07) |

Integration tests with a fake HTTP server (`wiremock`-style, implemented with a local `tokio::net::TcpListener` to avoid a new dependency, or the `httpmock` crate as a dev-dependency):

| Test | Scenario | Expected |
|---|---|---|
| `login_happy_path` | exchange 200, user 200 | token stored, username stored, auth-expired false |
| `login_denied` | redirect without code | existing login kept, toast text (E-LOGIN-1) |
| `login_exchange_400` | exchange 400 | error returned, previous login kept (D-03 fixed) |
| `refresh_success` | expired token, refresh 200 | new token stored, request uses new bearer |
| `refresh_401_sets_flag` | refresh 401 | `LoginExpired`, flag true, next call fails without network |
| `refresh_400_no_flag` | refresh 400 | `RefreshFailed`, flag false, next call retries (D-06 documented) |
| `refresh_single_flight` | 10 concurrent calls, expired token | one refresh request |
| `logout_during_refresh` | logout while refresh pending | no token after both settle |
| `bind_manga_found` | list entry present, has_read true, status PlanToRead | status Reading, PUT with `status=reading`, `num_chapters_read` from remote |
| `bind_manga_not_found` | list entry absent | status PlanToRead (no reads) or Reading (reads), score 0.0, PUT created |
| `bind_manga_rereading_kept` | remote `is_rereading: true`, has_read true | status stays Rereading |
| `bind_anime_found_watch` | remote watching, has_seen true | status Watching |
| `update_completes_on_last_chapter` | update(did_read=true), last == total | status Completed, finished date set, PUT |
| `update_rereading_no_status_change` | Rereading, did_read=true, not last | status stays Rereading, no started date |
| `update_started_date_on_chapter_one` | READING, last 1.0, did_read | started date = now |
| `refresh_readds_missing` | list entry absent | PUT issued, returned track has local values (E-STAT-17) |
| `delete_remote_404` | DELETE 404 | error returned, caller logs (E-DEL-1) |
| `search_manga_parallel_order` | three ids, details return out of order | results in id order from search |
| `search_one_detail_fails` | details 404 for one id | whole call fails today; with D-09 proposal, two hits and a warning |
| `anime_my_search_uses_animelist` | after D-01 fix | request path `/v2/users/@me/animelist` |
| `put_score_integer_proposal` | with the D-11 switch on | body `score=8` |
| `request_headers` | any api call | `Authorization: Bearer ...`, no `Cache-Control` header |
| `no_cookie_store` | two calls | no `Cookie` header set by client |

Property tests (proptest is NOT added; use fixed tables): the status tables in both directions.

## 6. Fixtures for UI tests (GTK)

Not required for the core crate. The UI tests use the same JSON fixtures through the `MalTracker` mock (section 3.3 of the Rust design) and check the labels, the picker order, the dialog button order and the checkbox default. Run with `GDK_BACKEND=broadway` in CI if a display is needed, or skip (manual test list in section 7).

## 7. Manual test list (desktop, needs a real MAL account)

1. Log in with the loopback flow. Check the Settings row becomes checked and the username shows.
2. Log in with the paste flow. Same result.
3. Deny consent. Check the existing login is kept.
4. Set status, score, progress on a manga. Check the MAL website shows the same values within 10 seconds.
5. Set a start date and a finish date. Check the website shows the same calendar dates in the local timezone.
6. Mark the last chapter read. Check the status becomes Completed on the website.
7. Search `id:<known id>`. Check one result with the correct title.
8. Search `my:<title>` for a title on the list. Check the result count and the absence of a fake status (E-SEARCH-6 and section 7 of the UI doc).
9. Anime `my:` search after the D-01 fix. Check it returns anime entries.
10. Remove tracking with "Also remove from MyAnimeList" on. Check the title is gone on the website.
11. Expire the token by editing the stored expiry to the past. Check the next call refreshes.
12. Revoke the app on the MAL website. Check the next call shows the "Log in again" state (E-UI-2).
13. Turn off network. Check the progress update is queued (area:27-tracking-core delayed store) and re-sent when the network returns.

## 8. Known gaps in this plan

- The MAL API documentation at `https://myanimelist.net/apiconfig/references/api/v2` could not be read during planning (the page returned only its title). Every statement about MAL's own behaviour (search limits, status and score validation, duplicate query parameters, token endpoint details, rate limits) comes from the Kotlin source, not from MAL's docs. The tests above that depend on those statements are listed with their decision points (D-01, D-09, D-11, D-12, D-19, E-SEARCH-16, E-NET-4). T-29a-041 requires recording live responses before those decisions are final.
- Whether MAL accepts a redirect to `http://localhost:<port>` is not known. T-29a-039 and T-29a-040 decide it.
