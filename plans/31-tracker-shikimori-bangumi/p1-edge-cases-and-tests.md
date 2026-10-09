# Area 31-tracker-shikimori-bangumi, part 1: edge cases, failure modes, tests

Upstream tests: none. A search of the whole Aniyomi tree for `shikimori` or `bangumi` in any `*Test*.kt`, `app/src/test` or `domain/src/test` returns no files. The scope has no `*Test.kt`. Every test in this document is new and written for Kurobayomi. Expected values come from the Kotlin behaviour in p1-behavior.md and from the server facts in p1-topic-remote-api-verification.md.

Test ids: `TS-n` (test), `EC-S-nn` (Shikimori edge case), `EC-B-nn` (Bangumi edge case), `EC-X-nn` (shared), `FM-nn` (failure mode), `RC-nn` (race). Open questions: `OQ-n`.

Contents: 1 edge cases, Shikimori; 2 edge cases, Bangumi; 3 shared edge cases; 4 failure modes; 5 race conditions; 6 tests to write; 7 fixtures; 8 open questions.

---

## 1. Edge cases, Shikimori

| ID | Input or state | Kotlin result (p1-behavior.md) | Required Rust result | Test |
|---|---|---|---|---|
| EC-S-01 | Logged out, search | `Exception("Not authenticated with Shikimori")` (D9) | Search without bearer returns results (proposal P-anon) when the server allows it; with the bearer disabled the call is public | TS-S-03 |
| EC-S-02 | Logged out, bind or refresh | interceptor throws `Not authenticated with Shikimori` before any request | `TrackError::NotAuthenticated("Shikimori")` before any request | TS-S-10 |
| EC-S-03 | Token expired, refresh returns 400 | request sent with old token, server 401 → `HTTP error 401` | `AuthExpired`, flag set, no request with the old token | TS-S-05 |
| EC-S-04 | Token expired, refresh returns 500 | same as EC-S-03 | `Http(500)` from refresh, token kept, next call retries refresh | TS-S-06 |
| EC-S-05 | Token expires exactly at `created_at + expires_in - 3600` | not refreshed (`>` is strict) | not refreshed | TS-S-02 |
| EC-S-06 | Token expires at `created_at + expires_in - 3599` | refreshed | refreshed | TS-S-02 |
| EC-S-07 | `refresh_token` absent, token not expired | works (NPE only happens when `refreshToken!!` is evaluated, which is every request: D3) | works; refresh is not needed | TS-S-07 |
| EC-S-08 | `refresh_token` absent, token expired | `NullPointerException` on every request | `AuthExpired`, flag set | TS-S-07 |
| EC-S-09 | Login with a bad code (token endpoint 400) | caught, `logout()`, no message | `Err(TrackError::Http(400))` returned to the GUI, which shows a toast; credentials stay cleared | TS-S-08 |
| EC-S-10 | Login with a good code, `whoami` 500 | caught, `logout()` (token stored then deleted) | error returned; token not stored (store only after `whoami` succeeds) | TS-S-08 |
| EC-S-11 | `whoami` returns id `1234` | username `"1234"` | username `"1234"` (string, not number) | TS-S-08 |
| EC-S-12 | Search response element with `chapters: null` (manga search) | `NullPointerException` (`!!`) fails the whole search | the element is skipped with a `tracing::warn!`, or the search fails with `Decode` (choose one; see OQ-1) | TS-S-12 |
| EC-S-13 | Search response with `aired_on: null` | `start_date = ""` | `start_date = ""` | TS-S-12 |
| EC-S-14 | Bind finds the entry; local start date is 1700000000000 | start date becomes `0` (D22); bind restores from reading history if `startDate <= 0` | local start date kept; history restore not needed; finish date kept | TS-S-14 |
| EC-S-15 | Bind finds entry with `user_rates` list of 2 rows | `Exception("Too many manga in response")` | `TrackError::TooManyEntries` | TS-S-09 |
| EC-S-16 | Bind finds entry with `user_rates` empty | `null` → not-found path: `add` | not-found path | TS-S-10 |
| EC-S-17 | Catalogue `GET /api/mangas/{id}` returns 404 | `HttpException(404)` (lookup aborts before user_rates) | `HttpStatus(404)` | TS-S-09 |
| EC-S-18 | Not-found bind with `hasReadChapters = true` | `READING`, score `0.0`, POST with local progress | same | TS-S-10 |
| EC-S-19 | Update with `didRead = true`, `last == total`, `total > 0` | `COMPLETED` | `Completed` | TS-S-11 |
| EC-S-20 | Update with `didRead = true`, `status = Rereading`, `last < total` | stays `REREADING` | stays `Rereading` | TS-S-11 |
| EC-S-21 | Update with `didRead = true`, `status = Dropped` | `READING` (dropped becomes reading on read) | `Reading` | TS-S-11 |
| EC-S-22 | Logout while a refresh holds the session lock | Kotlin: race; token may be re-saved after logout | Rust: after logout, the secret is deleted; next start restores nothing; the in-memory token is not reused after the refresh completes because the refresh result is only stored when the tracker is still logged in (check `base.is_logged_in()` before `store`) | TS-S-15 |
| EC-S-23 | Score `-1.0` written (`setRemoteScore` not used, but a stale search-result score reaches update) | sends `-1` (server reverts) | clamped to `0`; no `-1` on the wire | TS-S-13 |
| EC-S-24 | Score `10.7` | sends `10` (`toInt`) | sends `10` | TS-S-13 |
| EC-S-25 | Delete with `libraryId == null` | `DELETE /api/v2/user_rates/null` → 404 → logged | `TrackError::NoLibraryId`, no request | TS-S-16 |
| EC-S-26 | Delete returns 404 (already removed remotely) | logged, nothing shown | returns `Ok(())` and logs at info (treat as already deleted) — proposal, see OQ-2 | TS-S-16 |
| EC-S-27 | `status` on the list is an unknown string (`"rewatching_again"`) | `NotImplementedError("Unknown status: <s>")` escapes `catch(Exception)` (D4) | `TrackError::UnknownStatus` returned; no panic | TS-S-17 |
| EC-S-28 | `kind = "manhwa"` | passed through as `publishing_type` | passed through | TS-S-12 |
| EC-S-29 | Two bind calls in parallel for the same entry | both POST; second wins (upsert) | both POST; second wins; no duplicate row | TS-S-18 |
| EC-S-30 | Logout then login again | `track_token_4` written `"null"` then overwritten | secret deleted, then overwritten | TS-S-15 |
| EC-S-31 | `user_id` in POST body is a string | JSON string | JSON string | TS-S-19 |
| EC-S-32 | Unicode title with emoji in `name` | passed through | passed through (UTF-8) | TS-S-12 |

---

## 2. Edge cases, Bangumi

| ID | Input or state | Kotlin result | Required Rust result | Test |
|---|---|---|---|---|
| EC-B-01 | Logged out, search | `Exception("Not authenticated with Bangumi")` (D9) | search without bearer (proposal P-anon); the server does not require auth | TS-B-03 |
| EC-B-02 | Token expired, refresh returns 400 | request sent with expired token → 401 | `AuthExpired`, flag set | TS-B-05 |
| EC-B-03 | Refresh body | `grant_type`, `client_id`, `client_secret`, `refresh_token`, `redirect_uri` (order as built) | same order | TS-B-06 |
| EC-B-04 | Token response without `created_at` | `createdAt` = decode time | `created_at` = `clock.now_unix()` at decode | TS-B-04 |
| EC-B-05 | Token response with `user_id: 12345` | stored in the raw object; in-memory copy keeps the previous `userId` (D15) | stored; never read | TS-B-04 |
| EC-B-06 | Collection GET returns 404 | `null` (not collected) | `Ok(None)` | TS-B-07 |
| EC-B-07 | Collection GET returns 403 (private collection for another user, or token lacks scope) | `HttpException(403)` rethrown | `HttpStatus(403)` | TS-B-07 |
| EC-B-08 | Collection with `type: 6` (not in the enum) | `NotImplementedError("Unknown status: 6")` | `UnknownStatus` | TS-B-08 |
| EC-B-09 | Collection with `type` absent | `NotImplementedError("Unknown status: null")` | `UnknownStatus` | TS-B-08 |
| EC-B-10 | Collection with `rate` absent | `score = 0.0` | `score = 0.0` | TS-B-07 |
| EC-B-11 | Collection with `ep_status` absent | `last = 0.0` | `progress = 0.0` | TS-B-07 |
| EC-B-12 | Refresh response exactly as in the documented refresh example (no `user_id`) | decodes (`explicitNulls = false`) | decodes | TS-B-05 |
| EC-B-13 | Bind found, `hasReadChapters = false`, remote `PLAN_TO_READ` | status `PLAN_TO_READ`; PATCH with `type: 1` | same | TS-B-09 |
| EC-B-14 | Bind found, `hasReadChapters = true`, remote `ON_HOLD` | `READING`; PATCH `type: 3` | same | TS-B-09 |
| EC-B-15 | Bind found, `hasReadChapters = true`, remote `COMPLETED` | stays `COMPLETED` | same | TS-B-09 |
| EC-B-16 | Bind not found, `hasReadChapters = false` | `PLAN_TO_READ`, score 0, POST `type: 1`, `rate: 0` | same | TS-B-10 |
| EC-B-17 | PATCH returns 204 | success | success | TS-B-09 |
| EC-B-18 | POST returns 202 with no body | success, `library_id` unchanged (null) | success; no body parse | TS-B-10 |
| EC-B-19 | Score `-1` (search default) written | `rate: 0` (coerce) | `rate: 0` | TS-B-11 |
| EC-B-20 | Score `10.9` | `rate: 10` (`toInt` then coerce) | `rate: 10` | TS-B-11 |
| EC-B-21 | `private: true` on a track | sent as `"private": true` on every write; never read back | same | TS-B-12 |
| EC-B-22 | Refresh with `private` local `true` and remote `private: false` | local kept (self-copy; statusTrack is the same object) | local kept | TS-B-13 |
| EC-B-23 | Search, manga, `platform: "小说"` (novel) | filtered out (only null and "漫画" kept) | filtered out | TS-B-03 |
| EC-B-24 | Search, anime, same element | kept (no platform filter) | kept | TS-B-03 |
| EC-B-25 | Search `name_cn: ""`, `name: "Bleach"` | title `Bleach`; summary `summary.trim()` (no 作品原名 line) | same | TS-B-02 |
| EC-B-26 | Search `name_cn: "死神"`, `name: "Bleach"`, `summary: null` | summary `"作品原名：Bleach"` | same | TS-B-02 |
| EC-B-27 | Search `name_cn: "死神"`, `summary: "  hi  "` | `"作品原名：Bleach\nhi"` | same | TS-B-02 |
| EC-B-28 | Search `rating: null` | `score = -1.0` | `score = -1.0`, displayed as `-` | TS-B-02 |
| EC-B-29 | Search `date: null` | `start_date = ""` | `""` | TS-B-02 |
| EC-B-30 | Search `eps: 0` for manga (unknown count) | `total_chapters = 0` (no progress bar) | `total = 0` | TS-B-02 |
| EC-B-31 | Username with characters outside `[A-Za-z0-9_-]` in URL path | inserted unencoded | percent-encoded path segment (proposal P-enc) | TS-B-07 |
| EC-B-32 | Username empty (logged out, status lookup) | request sent with empty segment `/v0/users//collections/<subject_id>` → 404 → `null` | refused before the request: `NotAuthenticated` | TS-B-10 |
| EC-B-33 | Collection JSON with `subject` absent | `total_*` = 0 | `total = 0` | TS-B-07 |
| EC-B-34 | Anime progress, `ep_status` write | sent (D7) | sent (parity, D7); flagged in UI | TS-B-09 |

---

## 3. Shared edge cases (both trackers)

| ID | Situation | Required behaviour | Test |
|---|---|---|---|
| EC-X-01 | `status` internal value `0` (never set) reaches a write | Kotlin: `NotImplementedError` (escapes). Rust: `TrackError::UnknownStatus("0")` | TS-X-02 |
| EC-X-02 | Internal status `-1` (Bangumi rereading id) reaches a write | Kotlin: `NotImplementedError`. Rust: `UnknownStatus` | TS-X-02 |
| EC-X-03 | Shikimori internal `6` written for Bangumi | Bangumi enum has no 6: `UnknownStatus` | TS-X-03 |
| EC-X-04 | Bangumi internal `1` written for Shikimori | Shikimori `1` = READING → `watching` (different meaning, same number) | TS-X-03 |
| EC-X-05 | Remote `score` is `NaN` (cannot come from JSON) | not reachable; Rust still guards before storing | TS-X-04 |
| EC-X-06 | Progress `12.9` | `chapters: 12`, status compare uses `last as i64 == total` → 12 | TS-X-05 |
| EC-X-07 | Progress `12.0` and `total = 12` | `COMPLETED` on read | TS-X-05 |
| EC-X-08 | Total `0` and progress equal `0` | no `COMPLETED` (rule requires `total > 0`) | TS-X-05 |
| EC-X-09 | HTTP 401 on any call while the token is not expired | Kotlin: `HTTP error 401` toast, no flag. Rust: `AuthExpired` (token revoked server-side) | TS-X-06 |
| EC-X-10 | HTTP 429 | Kotlin: `HTTP error 429`. Rust: `HttpStatus(429)`, no retry in this area (area 27 queue retries) | TS-X-07 |
| EC-X-11 | Connection refused / DNS failure | `reqwest::Error` → `Http`/`Offline` as area 27 maps it | TS-X-08 |
| EC-X-12 | Response is HTML (captcha page or maintenance) with HTTP 200 | `serde_json` error → `Decode` with first 200 bytes in the log | TS-X-09 |
| EC-X-13 | Response is an empty body with 200 where JSON expected | `Decode("EOF")` | TS-X-09 |
| EC-X-14 | Very large search response (>1 MB) | read fully; no streaming (limit 20 keeps it small) | TS-X-10 |
| EC-X-15 | Non-UTF-8 bytes in a title | serde_json error → `Decode`; the whole search fails (same as Kotlin) | TS-X-09 |
| EC-X-16 | Clock skew: `now` earlier than `created_at` | `is_expired` false until `created_at + expires_in - 3600` | TS-X-02 |
| EC-X-17 | `expires_in = 0` | expired immediately → refresh every call (guard: refresh at most once per 60 s; OQ-4) | TS-X-02 |

---

## 4. Failure modes

| ID | Failure | Detection | User-visible result (Kurobayomi) | Recovery |
|---|---|---|---|---|
| FM-01 | Provider down (5xx) during bind | `HttpStatus(5xx)` | `track.err.http` with the status | user retries; area 27 queue for updates |
| FM-02 | Token revoked by user on the provider site | 401 on a fresh token, or refresh 400 | `AuthExpired` → "log in again" action | login flow |
| FM-03 | Refresh succeeds but secret store write fails | `SecretStore::set` error | token kept in memory; warning logged; token lost on restart → `AuthExpired` on next start | user logs in again (D-OPEN-2 decides whether to surface a dialog) |
| FM-04 | Secret Service absent | `keyring` error | area 27 fallback file (`secrets.json`, mode 0600) with a one-time warning | none needed |
| FM-05 | OAuth client not configured | `NotConfigured` on login | login row disabled with explanation | user adds client to config (D-OPEN-1) |
| FM-06 | Redirect with no `code` | area 27 calls logout | no login, credentials cleared | user retries login |
| FM-07 | `whoami` fails after token exchange | error in login | no token stored; error toast | retry |
| FM-08 | Bangumi username changed on the site | the stored username no longer matches the collection URL | lookup returns 404 → treated as not collected → bind POSTs the subject, which upserts under the account that owns the token; the old username is not used again | none; documented |
| FM-09 | Shikimori user id changed | not possible in practice | n/a | n/a |
| FM-10 | Catalogue entry deleted on the provider | `/api/mangas/{id}` 404 | `NotFound` shown on bind; refresh shows `Could not find manga` | user removes the tracking entry locally |
| FM-11 | `user_rates` duplicate rows (server bug) | size > 1 | `TooManyEntries` | user fixes the list on the site |
| FM-12 | Provider changes the status vocabulary | unknown string | `UnknownStatus`, no panic | app update needed |
| FM-13 | Provider changes Bangumi `type` values | unknown int | `UnknownStatus` | app update needed |

---

## 5. Race conditions

| ID | Race | Kotlin behaviour | Rust requirement | Test |
|---|---|---|---|---|
| RC-01 | Two calls refresh at the same time (Doorkeeper rotates the refresh token) | both refresh; the second refresh uses a token that the first revoked → failure, logged out in effect | single flight: one `Mutex` holds across the refresh; the second caller re-reads the new token | TS-S-04 |
| RC-02 | Refresh completes after logout | token re-saved after logout (Kotlin `newAuth` runs after `newAuth(null)`) | the store step checks `is_logged_in()` first; if logged out, discard the new token and return `NotAuthenticated` | TS-S-15 |
| RC-03 | Logout during bind | bind continues, writes `track_token_4` after logout | bind returns `NotAuthenticated` at the next request; no write after logout | TS-S-15 |
| RC-04 | Two tracks updated in parallel for the same entry | upsert; last write wins | same; no duplicate | TS-S-18 |
| RC-05 | `setRemoteLastChapterRead` while `TrackChapter` runs | area 27 concern | not in this area; the tracker's `update` is safe to call concurrently (state is per request) | TS-S-18 |
| RC-06 | Two logins in quick succession (double redirect) | both exchange the same code; the second exchange fails (codes are one-use), second `logout()` clears a valid login | the second exchange error must not clear a successful login: login error path clears credentials only if the login started this call and no valid token is stored | TS-S-08 |
| RC-07 | Bangumi collection GET races with PATCH | FORCE_NETWORK; read may see pre-PATCH state | after a PATCH, the caller uses the PATCH result, not a re-read | TS-B-09 |

RC-06 is a new requirement. Kotlin would log the user out on a double redirect; Rust must not.

---

## 6. Tests to write

Each test is a `#[tokio::test]` or `#[test]` in `crates/kuro-track/tests/`. Client is `reqwest::Client::new()`. Tracker constructors take base URLs, so `wiremock::MockServer::uri()` is used. Clock is a fixed `FakeClock`. Secret store is an in-memory `MemorySecrets`.

### 6.1 Shikimori

| ID | Name | Setup | Assertions |
|---|---|---|---|
| TS-S-01 | `status_round_trip` | all six internal values | `from_internal(v).api_str()` gives the six strings; `from_api` inverts; `from_internal(0)` is `None` |
| TS-S-02 | `expiry_boundary` | `created_at = 1000`, `expires_in = 86400`, now = `1000+86400-3600` and `-3599` | strict `>` (EC-S-05, EC-S-06) |
| TS-S-03 | `search_anonymous` (proposal P-anon) | logged out, `GET /api/mangas` mocked | request has no `Authorization`; results mapped |
| TS-S-04 | `single_flight_refresh` | token expired, 10 concurrent `bearer()` calls, refresh mock counts hits | refresh mock hit exactly once; all callers get the new token (RC-01) |
| TS-S-05 | `refresh_rejected_400` | refresh returns 400 | `AuthExpired`; flag set; no bearer request sent (EC-S-03) |
| TS-S-06 | `refresh_transport_500` | refresh returns 500 | `Http(500)`; token kept; second call retries (EC-S-04) |
| TS-S-07 | `refresh_token_absent` | token with `refresh_token: None`, not expired then expired | not expired: works; expired: `AuthExpired` (EC-S-07, EC-S-08) |
| TS-S-08 | `login_code_flow` | token endpoint 200 + `whoami` 200 | username `"1234"`, password = token, secret stored under `4.token`; second case: token 400 → no secrets stored (EC-S-09); `whoami` 500 → no token stored (EC-S-10); double redirect (RC-06) |
| TS-S-09 | `find_lib_errors` | catalogue 404; user_rates with 2 rows; user_rates empty | `HttpStatus(404)`, `TooManyEntries`, `Ok(None)` (EC-S-15, EC-S-16, EC-S-17) |
| TS-S-10 | `bind_not_found_and_logged_out` | user_rates empty, POST returns `{"id": 77}` | status `Reading` if local chapters read else `PlanToRead`, score 0, library id 77; logged out: no request sent (EC-S-02, EC-S-16, EC-S-18) |
| TS-S-11 | `update_transitions` | table of (status, did_read, last, total) | expected status (EC-S-19, EC-S-20, EC-S-21) |
| TS-S-12 | `search_mapping` | fixture array with `chapters: null`, `aired_on: null`, `kind: "manhwa"`, unicode name | fields as in EC-S-12, EC-S-13, EC-S-28, EC-S-32 |
| TS-S-13 | `score_clamp_on_write` | scores `-1.0`, `10.7` | wire scores `0`, `10` (EC-S-23, EC-S-24) |
| TS-S-14 | `bind_keeps_dates` | local start `1700000000000`, finish `1700000100000`; remote found with dates 0 | after bind, local start and finish unchanged (EC-S-14, D22) |
| TS-S-15 | `logout_then_stale_refresh` | start refresh with a slow mock; call logout while it waits; release mock | after logout: secret `4.token` absent; refreshed token not stored; credentials empty (EC-S-22, EC-S-30, RC-02, RC-03) |
| TS-S-16 | `delete_remote` | `libraryId = 99`, DELETE returns 204; second case `libraryId = null`; third case DELETE returns 404 | DELETE path `/api/v2/user_rates/99`; no request for null (`NoLibraryId`); 404 returns Ok (EC-S-25, EC-S-26) |
| TS-S-17 | `unknown_status_no_panic` | user_rates row `"status": "rewatching_again"` | `UnknownStatus`; no panic (EC-S-27) |
| TS-S-18 | `parallel_upserts` | two parallel `update` calls for one entry | both POST; final library id from the response (EC-S-29, RC-04) |
| TS-S-19 | `body_key_order` | build body for manga and anime | serialised JSON equals the expected string byte for byte (EC-S-31) |

### 6.2 Bangumi

| ID | Name | Setup | Assertions |
|---|---|---|---|
| TS-B-01 | `status_round_trip` | five internal values | `collection_type` identity; `from_collection_type(6)` error (EC-B-08) |
| TS-B-02 | `search_mapping` | fixtures for EC-B-25 to EC-B-30 | title, summary text, score `-1.0`, start date, total |
| TS-B-03 | `search_filters` | manga search with `platform` 漫画, 小说 and null; anime search with same | manga keeps 漫画 and null; anime keeps all; body has `type: [1]` and `[2]` (EC-B-23, EC-B-24, EC-B-01) |
| TS-B-04 | `token_decode_fields` | token response with and without `created_at`, with `user_id` | `created_at` set; `user_id` stored (EC-B-04, EC-B-05) |
| TS-B-05 | `refresh_documented_body` | refresh response exactly as in `How-to-Auth.md` (no `user_id`) | decodes; new token stored; expired-and-rejected returns `AuthExpired` (EC-B-02, EC-B-12) |
| TS-B-06 | `refresh_request_shape` | refresh mock | form keys in the order of EC-B-03 |
| TS-B-07 | `collection_lookup` | 404; 403; 200 with rate absent, ep_status absent, subject absent; username with special characters | `Ok(None)`, `HttpStatus(403)`, defaults, percent-encoded path (EC-B-06, EC-B-07, EC-B-10, EC-B-11, EC-B-31, EC-B-33) |
| TS-B-08 | `unknown_type` | `type: 6` and absent | `UnknownStatus` (EC-B-08, EC-B-09) |
| TS-B-09 | `bind_and_update` | collection 200 then PATCH 204; hasRead variants | statuses per EC-B-13 to EC-B-15; PATCH body keys order; anime ep_status sent (EC-B-17, EC-B-34, RC-07) |
| TS-B-10 | `bind_not_found_post` | collection 404, POST 202 | POST body `type 1`, `rate 0`, `ep_status 0`, `private false`; library id unchanged (EC-B-16, EC-B-18, EC-B-32) |
| TS-B-11 | `rate_coercion` | scores `-1`, `10.9` | `rate` 0 and 10 (EC-B-19, EC-B-20) |
| TS-B-12 | `private_write_only` | `private: true` local; collection says `private: false` | write sends `true`; local stays `true` after bind (EC-B-21) |
| TS-B-13 | `refresh_keeps_private` | local private true, remote false | local stays true (EC-B-22) |
| TS-B-14 | `logged_out_guard` | no token | `NotAuthenticated("Bangumi")`, no request |

### 6.3 Shared

| ID | Name | Setup | Assertions |
|---|---|---|---|
| TS-X-01 | `tracker_ids` | constructors | Shikimori id 4, Bangumi id 5; `TrackerManager` order |
| TS-X-02 | `internal_status_guard` | statuses 0, -1, 99 | `UnknownStatus` for writes (EC-X-01, EC-X-02, EC-X-16, EC-X-17) |
| TS-X-03 | `cross_tracker_numbers` | Shikimori 6 to Bangumi; Bangumi 1 to Shikimori | errors or distinct meanings (EC-X-03, EC-X-04) |
| TS-X-04 | `non_finite_guard` | construct `f64::NAN` on read path | rejected before store |
| TS-X-05 | `progress_truncation` | progress 12.9, total 12 and 0 | `chapters: 12`; completion rules (EC-X-06, EC-X-07, EC-X-08) |
| TS-X-06 | `http_401_fresh_token` | token fresh, server 401 | `AuthExpired` / auth-expired key (EC-X-09) |
| TS-X-07 | `http_429` | server 429 | `HttpStatus(429)` (EC-X-10) |
| TS-X-08 | `connect_refused` | closed port | transport error (EC-X-11) |
| TS-X-09 | `bad_bodies` | HTML 200, empty 200, invalid UTF-8 | `Decode`; log contains no token (EC-X-12, EC-X-13, EC-X-15) |
| TS-X-10 | `large_search` | 1 MB search body | decodes (EC-X-14) |
| TS-X-11 | `user_agent_format` | build UA | starts with `Kurobayomi/`, contains `(linux)` and the repository URL |
| TS-X-12 | `no_token_in_logs` | run a failing refresh with tracing capture | no access or refresh token text in any log line |

---

## 7. Fixtures

Stored under `crates/kuro-track/tests/fixtures/`. Each is hand-written from the DTO definitions and the server schemas. None are live account data.

| File | Content |
|---|---|
| `shikimori/oauth_token.json` | `{"access_token":"fx-access-1","token_type":"Bearer","created_at":1700000000,"expires_in":86400,"refresh_token":"fx-refresh-1"}` |
| `shikimori/oauth_token_no_refresh.json` | same with no `refresh_token` key |
| `shikimori/whoami.json` | `{"id": 1234}` plus extra keys (`nickname`) to exercise `ignoreUnknownKeys` |
| `shikimori/mangas_search.json` | three entries: normal, `chapters: null`, `aired_on: null`, `kind: "manhwa"` |
| `shikimori/mangas_1.json` | one entry with `chapters: 12` |
| `shikimori/user_rates_one.json` | one row: `status: "watching"`, `chapters: 3.0`, `episodes: 0.0`, `score: 8` |
| `shikimori/user_rates_two.json` | two rows (TooManyEntries) |
| `shikimori/user_rates_empty.json` | `[]` |
| `shikimori/user_rate_create.json` | `{"id": 77, "user_id": 1234, "target_id": 42, "status": "watching"}` |
| `bangumi/oauth_token.json` | `{"access_token":"fx-b-access","expires_in":604800,"token_type":"Bearer","scope":null,"refresh_token":"fx-b-refresh","user_id":1}` |
| `bangumi/oauth_refresh_documented.json` | the refresh example from `How-to-Auth.md` (no `user_id`, `scope: null`) |
| `bangumi/me.json` | `{"username": "fixture_user", "id": 1, "nickname": "x"}` |
| `bangumi/search_manga.json` | three subjects: 漫画 with `name_cn`, empty `name_cn`, 小说 (filtered out) |
| `bangumi/search_anime.json` | one subject with `rating: null` and `date: null` |
| `bangumi/collection_doing.json` | `{"rate": 8, "type": 3, "ep_status": 5, "vol_status": 0, "private": false, "subject": {"volumes": 0, "eps": 24}}` |
| `bangumi/collection_unknown_type.json` | `type: 6` |
| `bangumi/collection_no_rate.json` | `type: 1`, no `rate`, no `ep_status`, no `subject` |

---

## 8. Open questions

| ID | Question | Recommendation | Owner |
|---|---|---|---|
| OQ-1 | On a search element with a missing `chapters`, skip the element or fail the whole search? Kotlin fails the whole search. | Skip the element with a warning. A search with one broken entry should still show the rest. | user |
| OQ-2 | On delete with HTTP 404, is the entry "already deleted" (success) or an error? Kotlin logs and shows nothing. | Success: the goal of delete is that the entry is gone. | user |
| OQ-3 | Are the Shikimori and Bangumi logos (copied into Aniyomi at `ic_tracker_*.webp`) allowed to ship in Kurobayomi? | Ask the providers or use neutral text tiles (initials) like the mockup does. | user |
| OQ-4 | Guard against refresh loops when `expires_in` is 0 or the clock is wrong: refresh at most once every 60 seconds. | Yes. Add the guard. Kotlin has no guard. | this area |
| OQ-5 | Anonymous search (P-anon): is it acceptable to call the public catalogue without the user's token? | Yes. The endpoints are public and the request carries no user data. | user |
| OQ-6 | Refresh-token rotation on Shikimori: is the old refresh token revoked on refresh? | Assume yes and persist the new pair (safe in both cases). Verify on a live account before release. | this area, verification step T-31a-046 |
| OQ-7 | Bangumi `user_id` absent in refresh responses (documented example). | Already handled: `Option` (EC-B-12). Verify on a live refresh. | this area, verification step T-31a-046 |
| OQ-8 | Should Kurobayomi show a "not configured" message or hide the tracker rows when no OAuth client is configured? | Show the rows disabled with an explanation. | user |
