# Area 30-tracker-kitsu: edge cases, failure modes, tests

Repo root: `/home/blackbox/aniyomi-src`. The scope contains no `*Test.kt` file (checked: none of the 11 scope files is under a test source set). Upstream test cases to port are therefore zero; every test below is new, derived from the Kotlin source (file:line cited). Test ids are `KT-nn` (Kitsu test).

---

## 1. Edge cases and failure modes

Each row: ID | where | trigger | observed result | required Rust result | task.

### 1.1 Authentication and session

| ID | Where | Trigger | Observed (Kotlin) | Required (Rust) | Task |
|---|---|---|---|---|---|
| E-01 | KitsuInterceptor.kt:23 | Any authed call with no token | throws `Exception("Not authenticated with Kitsu")` before network | `KitsuError::NotAuthenticated`, no request sent | T-30a-006 |
| E-02 | KitsuInterceptor.kt:25 | Token present, `refresh_token` null, token NOT expired | NPE from `!!` (refresh token read before expiry check) → every request fails | Allowed. Refresh needed only when expired | T-30a-006 |
| E-03 | KitsuInterceptor.kt:28-35 | Token expired, refresh returns non-2xx | response closed; old expired token used → server 401 → `HTTP error 401` | 4xx refresh → `ReauthRequired` (flagged deviation); network error → stale token kept | T-30a-006 |
| E-04 | KitsuInterceptor.kt:31 | Refresh 2xx but JSON lacks `refresh_token` | stored token with null refresh → next expiry NPE (E-02) | keep previous refresh token | T-30a-006 |
| E-05 | KitsuInterceptor.kt:31 | Refresh 2xx with malformed JSON | `SerializationException` from `json.decodeFromString` (not caught) | `KitsuError::Decode`; old token kept | T-30a-006 |
| E-06 | KitsuInterceptor.kt:18 | App start with stored token JSON | token restored once at interceptor creation | token restored at `AuthSession::restore` | T-30a-005 |
| E-07 | Kitsu.kt:246-252 | Stored value `""` (never set) | decode fails → `null` (no crash) | `None` | T-30a-005 |
| E-08 | Kitsu.kt:246-252 | Stored value `"null"` (after logout) | decode of non-nullable type fails → `null` | `None`; logout removes the entry | T-30a-005 |
| E-09 | Kitsu.kt:242-244 | `saveToken(null)` | writes the 4-char string `null` | Proposal: delete the secret entry | T-30a-005 |
| E-10 | KitsuApi.kt:395-410 | Wrong password | `POST` 400/401 → `HTTP error <code>` toast; dialog stays open, both fields red | same texts; dialog stays open, error shown inline | T-30a-008 |
| E-11 | KitsuApi.kt:412-425 + Kitsu.kt:226-231 | Token saved, `users` call fails | token persisted, credentials not saved (partial state) | token rolled back; no partial state | T-30a-007 |
| E-12 | KitsuApi.kt:421 | `users` returns `data: []` | `IndexOutOfBoundsException` (uncaught in Kitsu; toast shows its message) | `KitsuError::NoUser`; token rolled back | T-30a-007 |
| E-13 | KitsuApi.kt:395-410 | Login with blank fields | UI disables Login button; API never called | same | T-30a-008 |
| E-14 | SettingsTrackingScreen.kt:349-364 | Login failure | `tracker.logout()` runs, clearing credentials even if previously logged in | Proposal: a failed re-login keeps the previous session (flagged) | T-30a-008 |
| E-15 | Kitsu.kt:233-236 | Logout | no server call; token not revoked | same; token deleted locally | T-30a-009 |
| E-16 | Kitsu.kt:226-231 | password stored | password never persisted; user id in password slot | user id in its own key (`track_user_id_3`) | T-30a-007 |
| E-17 | BaseTracker.kt:32-34 | `isLoggedIn` | username and password non-empty | username and user id non-empty | T-30a-007 |
| E-18 | Kitsu.kt:124-140 | Two concurrent requests, both see expired token | both POST refresh (no serialisation) | single refresh under mutex | T-30a-027 |

### 1.2 Requests and responses

| ID | Where | Trigger | Observed | Required | Task |
|---|---|---|---|---|---|
| E-20 | KitsuApi.kt:135-165 | PATCH with `remote_id = 0` (unbound) | sent; 404 `HTTP error 404` | local `NotFound` before sending (flagged) | T-30a-010 |
| E-21 | KitsuApi.kt:199-209 | DELETE with `remote_id = 0` | sent; 404 | local error before sending (flagged) | T-30a-011 |
| E-22 | KitsuApi.kt:135-165 | Library entry deleted remotely | PATCH 404 → `updateRemote` toast `HTTP error 404`, DB row unchanged | same texts; local row unchanged | T-30a-010 |
| E-23 | KitsuApi.kt:311-330 | Find: `data` non-empty but `included` empty | returns `null` → bind treats as unbound (adds a second entry) | Proposal: return error when `data` non-empty and `included` empty; flagged | T-30a-012 |
| E-24 | KitsuApi.kt:353-372 | Get by entry id: nothing found | `Exception("Could not find manga")` | `NotFound("manga")` | T-30a-013 |
| E-25 | KitsuApi.kt:374-393 | Get anime by entry id: nothing found | `Exception("Could not find manga")` (copy-paste) | `NotFound("anime")` text "Could not find anime" | T-30a-024 |
| E-26 | KitsuListSearch.kt:17-18 | `firstToMangaTrack` with empty data | `IllegalArgumentException("Missing User data from Kitsu")` | `MissingUserData` | T-30a-014 |
| E-27 | KitsuListSearch.kt:52 | `firstToAnimeTrack` with empty included | message says "Missing Manga data from Kitsu" | "Missing Anime data from Kitsu" | T-30a-024 |
| E-28 | KitsuListSearch.kt:42,76 | status string not in table (e.g. `"completed_v2"`) | `Exception("Unknown status")` → bind/refresh fail | `UnknownStatus` (fatal, same as Aniyomi) | T-30a-015 |
| E-29 | KitsuDateHelper.kt:21 | Remote date missing milliseconds, e.g. `2024-05-01T00:00:00Z` | `ParseException` → bind/refresh fail | Proposal: accept with/without millis and `+00:00`; flagged | T-30a-016 |
| E-30 | KitsuDateHelper.kt:21 | Remote date `""` | `ParseException` | `BadDate` (Aniyomi-compatible), or treat `""` as unset (flagged) | T-30a-016 |
| E-31 | KitsuListSearch.kt:44 | `ratingTwenty = 1` | 0.5 | 0.5 | T-30a-003 |
| E-32 | KitsuUtils.kt:24-30 | score 0.5 → wire | `(0.5*2).toInt()` = `"1"` | `"1"` | T-30a-003 |
| E-33 | KitsuUtils.kt:25 | score 0.0 → wire | `null` → JSON null (clears rating) | JSON null | T-30a-010 |
| E-34 | KitsuApi.kt:145-146 | start/finish date 0 → wire | JSON null (clears remote date) | JSON null | T-30a-010 |
| E-35 | KitsuApi.kt:52,97,143,175 | progress `12.7` → wire | `toInt()` = 12 | 12 | T-30a-010 |
| E-36 | KitsuListSearch.kt:45 | remote progress read | `progress.toDouble()` → integer-valued | same | T-30a-014 |
| E-37 | Kitsu.kt:127 | `last_chapter_read = 12.5`, total 12 | `toLong()` = 12 → COMPLETED | same | T-30a-020 |
| E-38 | Kitsu.kt:127 | total 0 (unknown) | never completes automatically | same | T-30a-020 |
| E-39 | Kitsu.kt:132 | progress exactly 1.0 on read | started date reset to now | same; later reads at 1.0 also reset | T-30a-020 |
| E-40 | Kitsu.kt:124-136 | DROPPED/ON_HOLD + read event | becomes READING | same | T-30a-020 |
| E-41 | Kitsu.kt:174-176 | bind, remote COMPLETED, has-read true | status stays COMPLETED | same | T-30a-019 |
| E-42 | Kitsu.kt:171 | bind, remote private true | local private kept (copyRemotePrivate=false) | same | T-30a-019 |
| E-43 | Kitsu.kt:212-217 | refresh, remote private true | copied (copyRemotePrivate default true) | same | T-30a-021 |
| E-44 | Kitsu.kt:215 | refresh, remote chapterCount null | local total set to 0 | same (flagged as surprising) | T-30a-021 |
| E-45 | KitsuListSearch.kt:33-35 | `startedAt` present but unparseable | fatal | see E-29 | T-30a-016 |
| E-46 | KitsuApi.kt:402-405 | login form field order | username, password, grant_type, client_id, client_secret | same order (byte compatibility) | T-30a-007 |
| E-47 | KitsuApi.kt:254 | query `a b&c~d*e` | `a+b%26c%7Ed*e` | same | T-30a-017 |
| E-48 | KitsuApi.kt:249-278 | Algolia hit subtype `novel` | filtered out | same | T-30a-017 |
| E-49 | KitsuApi.kt:274 | Algolia hit subtype null | kept | same | T-30a-017 |
| E-50 | KitsuApi.kt:223-234 | search while logged out | `Not authenticated with Kitsu` (key fetch is authed) | same | T-30a-017 |
| E-51 | KitsuApi.kt:226 | Algolia key fetched every search | one extra GET per search | cached per session; refetch on 403 (flagged) | T-30a-028 |
| E-52 | KitsuSearch.kt:45,62 | Algolia `chapterCount` null | total 0 | same | T-30a-018 |
| E-53 | KitsuSearch.kt:44 | search `averageRating` null | score -1.0 | same | T-30a-018 |
| E-54 | KitsuSearch.kt:49 | `endDate` null | "Publishing" | same | T-30a-018 |
| E-55 | KitsuSearch.kt:51-54 | `startDate` = 0 seconds | "1970-01-01" in local zone | same | T-30a-018 |
| E-56 | KitsuSearch.kt:51-54 | `startDate` = 1714564800 in UTC-5 | "2024-05-01" (local) but could show previous day for UTC-midnight values in negative zones | local display kept (flagged note) | T-30a-018 |
| E-57 | KitsuApi.kt:262-272 | Algolia hit missing required `canonicalTitle` | whole search fails | skip that hit, log (flagged) | T-30a-018 |
| E-58 | KitsuApi.kt:454-460 | `mangaUrl(libraryId)` | wrong URL after find (library id) — latent, not persisted | build from media id (flagged) | T-30a-029 |

### 1.3 Display and locale

| ID | Where | Trigger | Observed | Required | Task |
|---|---|---|---|---|---|
| E-60 | Kitsu.kt:97-100 | German locale score list | `"1,5"`, `"7,5"` | always `.` (flagged) | T-30a-035 |
| E-61 | Kitsu.kt:106-114 | `displayScore(0.0)` | `"0"` | `"0"` | T-30a-003 |
| E-62 | Kitsu.kt:106-114 | `displayScore(7.0)` | `"7"` | `"7"` | T-30a-003 |
| E-63 | Kitsu.kt:106-114 | `displayScore(-1.0)` | `"-1"` | `"-1"` (not reachable from list) | T-30a-003 |
| E-64 | Kitsu.kt:102-104 | `indexToScore(0)` | 0.0 | 0.0 | T-30a-003 |
| E-65 | SettingsTrackingScreen.kt:91 | score not in list (`indexOf` = -1) | `indexToScore(-1)` = 0.0 | `unwrap_or(0.0)` | T-30a-003 |
| E-66 | Kitsu.kt:61-67 | status list order | fixed order | same | T-30a-002 |
| E-67 | Kitsu.kt:69-85 | `getStatusForManga(11)` | null (no label) | None | T-30a-002 |

### 1.4 Concurrency and time

| ID | Where | Trigger | Observed | Required | Task |
|---|---|---|---|---|---|
| E-70 | KitsuDateHelper.kt:10 | concurrent `convert` | shared SimpleDateFormat → corrupt strings possible | stateless formatting | T-30a-016 |
| E-71 | KitsuInterceptor.kt:18,31,39 | concurrent requests during token write | unsynchronised `var oauth` | mutex | T-30a-027 |
| E-72 | KitsuApi.kt:45-88 | UI cancel during add | `withIOContext`, `withNonCancellableContext` in interactor | cancellation of bind is not propagated mid-PATCH in Rust either; document | T-30a-026 |
| E-73 | KitsuApi.kt:197, 221, 88 | response body not read | OkHttp connection not released until GC | `drop` response (reqwest) | T-30a-025 |
| E-74 | Requests.kt:12 + Kitsu GETs | authed GET within 10 min of a PATCH | may be served from OkHttp cache if server marks cacheable | `Cache-Control: no-cache` | T-30a-025 |
| E-75 | NetworkHelper.kt:23-25 | slow server | 30 s connect/read, 2 min call | same numbers in reqwest | T-30a-025 |

### 1.5 Delete and removal flows

| ID | Where | Trigger | Observed | Required | Task |
|---|---|---|---|---|---|
| E-80 | AnimeTrackInfoDialog.kt:811-840 | "Also remove from Kitsu" ticked, remote fails | local row already deleted; error logged, no toast | local-first order kept; remote failure shown as a toast (flagged improvement) | T-30a-022 |
| E-81 | AnimeTrackInfoDialog.kt:811 | checkbox shown only if `DeletableAnimeTracker` | Kitsu implements it → shown | same | T-30a-022 |
| E-82 | Kitsu.kt:160-166 | delete for unbound track | DELETE library-entries/0 → 404 (logged) | local error (see E-21) | T-30a-011 |

---

## 2. Upstream test cases to port

None. The scope has no `*Test.kt` file. The closest existing behaviour checks are the Anilist/MangaUpdates tracker tests, which are outside this scope and are not ported here.

---

## 3. New tests to write (Rust)

Each test is one function in `src/track/kitsu/tests/` (integration, `wiremock`) or `#[cfg(test)]` in the module. Test names use the `kt_` prefix and map to the edge ids above.

### 3.1 Mapping and score (pure, no network)
- `KT-01 kt_status_api_roundtrip`: for every (kind, status) in the allowed tables, `local_status(kind, api_status(kind, s)?) == s`. Covers 2.4.
- `KT-02 kt_status_cross_kind_rejected`: `api_status(Manga, WATCHING)`, `api_status(Manga, PLAN_TO_WATCH)`, `api_status(Anime, READING)`, `api_status(Anime, PLAN_TO_READ)` → `UnknownStatus`. (E-28 family.)
- `KT-03 kt_status_unknown_wire`: `local_status(Manga, "completed_v2")` → `UnknownStatus`.
- `KT-04 kt_score_list_exact`: `score_list()` equals the 20-entry array; length 20.
- `KT-05 kt_index_to_score`: index 0→0.0, 1→1.0, 2→1.5, 19→10.0.
- `KT-06 kt_score_wire`: 0.0→None; 0.5→"1"; 1.0→"2"; 1.5→"3"; 10.0→"20"; 7.5→"15". (E-32, E-33.)
- `KT-07 kt_rating_read`: `from_api_rating(None)`=0.0; `Some(1)`=0.5; `Some(20)`=10.0.
- `KT-08 kt_display_score`: 0.0→"0"; 7.0→"7"; 7.5→"7.5"; 10.0→"10"; -1.0→"-1". (E-61..63.)
- `KT-09 kt_display_score_no_locale`: run with `LC_ALL=de_DE.UTF-8` in the environment; output still "7.5". (E-60.)
- `KT-10 kt_status_labels`: manga 1→"Currently reading", 5→"Want to read"; anime 11→"Currently watching", 15→"Want to watch"; 2→"Completed"; 3→"On hold"; 4→"Dropped"; manga 11→None.

### 3.2 Dates
- `KT-11 kt_wire_utc`: `to_wire(1714564800000)` == `"2024-05-01T00:00:00.000Z"`; run once with `TZ=Asia/Tokyo` and once with `TZ=America/New_York`; same output (proves UTC; Aniyomi would differ). Flagged deviation test.
- `KT-12 kt_wire_zero_null`: `to_wire(0)` == None.
- `KT-13 kt_wire_roundtrip_millis`: `from_wire(to_wire(ms))` == ms for ms = 1714564800123.
- `KT-14 kt_wire_bad`: `from_wire(Some(""))` → BadDate; `from_wire(Some("2024-05-01"))` → BadDate; `from_wire(None)` → 0.
- `KT-15 kt_wire_no_millis_accepted` (proposal): `from_wire(Some("2024-05-01T00:00:00Z"))` → Ok (flagged; Aniyomi fails). Decision recorded in T-30a-016.

### 3.3 Auth and session
- `KT-20 kt_token_expiry_boundary`: `is_expired(created + expires - 3600)` is false; `+1` is true. (E-03 margin.)
- `KT-21 kt_token_restore_variants`: `""`, `"null"`, `"{garbage"`, valid JSON → None, None, None, Some.
- `KT-22 kt_login_form_order`: wiremock matches the request body string `username=<email>&password=<password>&grant_type=password&client_id=<CLIENT_ID>&client_secret=<CLIENT_SECRET>` exactly. (E-46.)
- `KT-23 kt_login_flow_success`: token POST 200 → users GET 200 with `{"data":[{"id":"42"}]}` → `is_logged_in()` true; stored username equals email; stored user id "42"; password never stored (assert no key holds the password). (E-16.)
- `KT-24 kt_login_rollback_on_no_user`: users returns `{"data":[]}` → Err NoUser; token store empty afterwards. (E-11, E-12.)
- `KT-25 kt_login_wrong_password`: token POST 401 → Err Http(401), message `HTTP error 401`; no credentials saved. (E-10.)
- `KT-26 kt_refresh_when_expired`: stored token expired; token POST (grant refresh_token) 200 new token; a subsequent `bind` sends the new bearer. Assert form body order `grant_type, refresh_token, client_id, client_secret`. (E-03 happy path.)
- `KT-27 kt_refresh_keeps_refresh_token`: refresh response without `refresh_token` → stored token keeps the old refresh token. (E-04.)
- `KT-28 kt_refresh_4xx_reauth`: refresh 400 → Err ReauthRequired; no request with the old bearer is made (wiremock asserts zero calls to library-entries). (E-03 proposal.)
- `KT-29 kt_refresh_network_error_keeps_stale`: refresh connection refused → request is sent with the old bearer (asserted by header), server returns 401 → Err Http(401). (E-03 Aniyomi-compatible branch.)
- `KT-30 kt_refresh_missing_token_no_request`: token has no refresh token and is expired → Err ReauthRequired; if not expired → request proceeds (E-02 deviation).
- `KT-31 kt_refresh_single_flight`: 20 concurrent `bearer()` calls with an expired token → exactly 1 refresh POST (wiremock `expect(1)`). (E-18.)
- `KT-32 kt_logout_clears`: after logout: `is_logged_in()` false; token store empty; no network call made. (E-15.)
- `KT-33 kt_no_token_not_authenticated`: no token → Err NotAuthenticated and zero HTTP requests. (E-01.)

### 3.4 Library entries (wiremock)
- `KT-40 kt_add_manga_body`: `bind` with no remote entry → POST body equals the fixture `add_manga.json` (exact JSON, numeric media id, string user id, progress int, private bool, no dates). Response `{"data":{"id":555}}` → `remote_id == 555`. (E-46 family, 6.1.)
- `KT-41 kt_add_anime_body`: same for anime (`type: "anime"`, progress from episodes, status `planned` for PLAN_TO_WATCH).
- `KT-42 kt_bind_found_copies_personal`: find returns entry (status current, progress 7, rating 16, startedAt set, private true) → bound track: progress 7, score 8.0, status READING (if has_progress), private kept as local (false), remote_id = entry id, PATCH sent. (7, E-42.)
- `KT-43 kt_bind_found_completed_kept`: remote completed + has_progress=true → status stays COMPLETED. (E-41.)
- `KT-44 kt_bind_found_dropped_has_progress`: remote dropped, has_progress=true → READING. (7.)
- `KT-45 kt_bind_found_dropped_no_progress`: remote dropped, has_progress=false → DROPPED stays. (7.)
- `KT-46 kt_bind_no_entry_plan`: no entry, has_progress=false → status planned (PLAN_TO_READ) and score 0 → POST has no ratingTwenty. (6.1.)
- `KT-47 kt_update_body_exact`: PATCH body equals fixture `update_manga.json` for score 8.0, started set, finished 0 → `ratingTwenty:"16"`, `finishedAt:null`, progress int. (E-33, E-34.)
- `KT-48 kt_update_zero_score_null`: score 0.0 → `ratingTwenty: null`. (E-33.)
- `KT-49 kt_update_progress_truncation`: progress 12.7 → `progress: 12`. (E-35.)
- `KT-50 kt_update_unbound_rejected`: `remote_id = 0` → NotFound locally; zero HTTP requests (flagged E-20).
- `KT-51 kt_update_404_surfaces`: PATCH 404 → Err Http(404), message `HTTP error 404`; caller-visible track unchanged. (E-22.)
- `KT-52 kt_read_event_completes`: total 12, progress 12.5 with `progress_event=true` → status COMPLETED, finished_ms set. (E-37.)
- `KT-53 kt_read_event_total_zero`: total 0, progress 12 → READING, never COMPLETED. (E-38.)
- `KT-54 kt_read_event_started_on_first`: progress 1.0 → started_ms set to now (inject clock). (E-39.)
- `KT-55 kt_read_event_drop_to_reading`: status DROPPED + progress event → READING. (E-40.)
- `KT-56 kt_read_event_no_flag_no_status`: progress_event=false → status unchanged, only PATCH. (8.)
- `KT-57 kt_delete_entry`: DELETE `library-entries/<id>` 204 → Ok; wiremock asserts method DELETE, empty body, `Content-Type: application/vnd.api+json`. (6.4.)
- `KT-58 kt_delete_unbound_rejected`: remote_id 0 → local error, no request (flagged E-21).
- `KT-59 kt_refresh_copies_and_total`: get entry → status, progress, score, private copied; total from `chapterCount`; null count → total 0. (E-43, E-44.)
- `KT-60 kt_refresh_missing_entry`: get returns `data: []` → NotFound("manga"); anime → NotFound("anime") (E-24/E-25.)
- `KT-61 kt_find_empty_is_none`: find returns `data: []` → Ok(None) → bind goes to add path. (6.5.)
- `KT-62 kt_find_data_without_included`: find returns `data:[x]` and `included:[]` → proposal: error (E-23 flagged). Aniyomi returns None; test documents the deliberate difference.
- `KT-63 kt_missing_user_data_message`: list response with empty data inside `firstTo*` → text "Missing User data from Kitsu". (E-26.)
- `KT-64 kt_missing_media_message_anime`: anime path with empty included → "Missing Anime data from Kitsu". (E-27.)
- `KT-65 kt_unknown_status_fails`: list entry status "weird" → UnknownStatus; bind fails and no DB write (caller). (E-28.)
- `KT-66 kt_tracking_url_media_id` (flagged): search hit URL uses media id; list-entry-derived URL is not used by bind/refresh (checked by asserting `tracking_url` unchanged after bind). (E-58.)

### 3.5 Search
- `KT-70 kt_algolia_key_flow`: key GET `{"media":{"key":"k1"}}` then Algolia POST with headers `X-Algolia-Application-Id: AWQO5J657S`, `X-Algolia-API-Key: k1`, `Content-Type: application/json; charset=utf-8`; body `{"params":"query=one+piece&facetFilters=%5B%22kind%3Amanga%22%5D&attributesToRetrieve=<verbatim ALGOLIA_FILTER for manga>"}` byte-equal to the fixture. (11.1-11.2.)
- `KT-71 kt_algolia_filter_constants`: `FILTER_MANGA` and `FILTER_ANIME` equal the spec strings byte for byte (compare with fixtures `algolia_filter_manga.txt`, `algolia_filter_anime.txt`).
- `KT-72 kt_search_novel_filtered`: hits with subtype `novel` removed; `null` subtype kept; order preserved.
- `KT-73 kt_search_mapping_fields`: one manga hit fixture → `SearchHit` fields per table in behaviour section 12 (publishing "Publishing"/"Finished", start_date "2020-01-01" in test zone UTC, tracking_url `https://kitsu.app/manga/<id>`).
- `KT-74 kt_search_empty_query`: query "" → `query=` in params; no client-side guard.
- `KT-75 kt_search_requires_login`: no token → NotAuthenticated; zero Algolia requests.
- `KT-76 kt_search_key_cached`: two searches → one key GET (proposal, flagged).
- `KT-77 kt_search_key_rotation`: Algolia 403 once → key refetched and retried once → Ok. Second 403 → Http(403). (flagged)
- `KT-78 kt_encode_query`: `a b&c~d*e` → `a+b%26c%7Ed*e`; unicode "حلقة" percent-encodes as UTF-8 bytes. (E-47.)
- `KT-79 kt_search_bad_hit_skipped` (flagged): one hit missing `canonicalTitle`, two good hits → Ok with two results. (E-57.)
- `KT-80 kt_search_anime_fields`: anime hit uses `episodeCount` for total and `tracking_url` `https://kitsu.app/anime/<id>`.

### 3.6 Decode robustness
- `KT-85 kt_ids_string_or_number`: `"id":"123"` and `"id":123` both decode to 123 in list items, add response, Algolia hits, and users (string). (2.8.)
- `KT-86 kt_unknown_fields_ignored`: extra keys in token (`scope`), list item, Algolia hit → decode succeeds. (AppModule Json config.)
- `KT-87 kt_nullable_fields`: list attributes with `startedAt: null`, `finishedAt: null`, `ratingTwenty: null`, included `chapterCount: null` → defaults applied (dates 0, score 0.0, total 0).
- `KT-88 kt_included_missing_status_fails`: included item without `status` → Decode error (DTO requires it; matches Aniyomi).

### 3.7 HTTP details
- `KT-90 kt_headers_on_authed`: every authed request has `Authorization: Bearer <token>`, `Accept`, `Content-Type` = `application/vnd.api+json`, `User-Agent` starting with `Kurobayomi v`, and `Cache-Control: no-cache`.
- `KT-91 kt_login_no_bearer`: login POST has no `Authorization` header.
- `KT-92 kt_algolia_no_bearer`: Algolia POST has no `Authorization` header.
- `KT-93 kt_timeouts`: a server that delays 31 s on read fails with a timeout error (reqwest read_timeout); a 121 s total delay fails with the call timeout. Use `tokio::time::pause` where applicable. (E-75.)

---

## 4. Fixtures needed

Location: `src/track/kitsu/tests/fixtures/` (JSON unless noted). Every fixture is copied from a real Kitsu response shape or written by hand from the DTO definitions. No live credentials.

| File | Contents |
|---|---|
| `token_ok.json` | `{"access_token":"a1","token_type":"bearer","created_at":1700000000,"expires_in":2592000,"refresh_token":"r1","scope":"public"}` (values chosen for the test; `expires_in` is a fixture value, not a Kitsu constant) |
| `token_no_refresh.json` | same without `refresh_token` |
| `token_expired.json` | `created_at` 1 (1970), `expires_in` 10 |
| `users_ok.json` | `{"data":[{"id":"42","type":"users","attributes":{}}]}` |
| `users_empty.json` | `{"data":[]}` |
| `key_ok.json` | `{"media":{"key":"k1"}}` |
| `key_rotated_403.json` | Algolia 403 body `{"message":"Invalid API key"}` (shape only; not verified against Algolia) |
| `algolia_manga_hits.json` | two manga hits (one `subtype: "manga"` with `endDate: null`, one `subtype: "novel"`) |
| `algolia_anime_hits.json` | one anime hit (`episodeCount: 12`) |
| `list_manga_found.json` | `data:[{id:"9001", attributes:{status:"current", startedAt:"2024-05-01T00:00:00.000Z", finishedAt:null, ratingTwenty:16, progress:7, private:true}}]`, `included:[{id:"77", attributes:{canonicalTitle:"Fixture", chapterCount:null, mangaType:"manga", posterImage:{original:"https://example.invalid/c.jpg"}, synopsis:"", startDate:"2020-01-01", status:"current"}}]` |
| `list_manga_empty.json` | `{"data":[],"included":[]}` |
| `list_anime_found.json` | anime analogue with `showType`, `episodeCount`, `status` |
| `add_manga_expected.json` | exact POST body for `remote_id=77`, status `current`, progress 0, private false |
| `add_anime_expected.json` | exact POST body for anime planned |
| `update_manga_expected.json` | exact PATCH body for score 8.0, started date set, finished 0 |
| `add_response.json` | `{"data":{"id":"555","type":"libraryEntries"}}` |
| `algolia_filter_manga.txt` | the `ALGOLIA_FILTER` string, one line |
| `algolia_filter_anime.txt` | the `ALGOLIA_FILTER_ANIME` string, one line |

Test clock: `Kitsu` takes `clock: fn() -> (i64 /*unix secs*/, i64 /*ms*/)` in tests, returning fixed values (`1714564800`, `1714564800000`).

---

## 5. Race conditions to test explicitly

1. Refresh single-flight (KT-31).
2. Bind and read-event update on the same entry (proposal per-entry lock; test: two tasks, assert the PATCH bodies are serialised and the final status is the last writer's). Test name `KT-95 kt_entry_lock_serialises`.
3. Search while login in progress: search returns NotAuthenticated while token store is empty; after login completes, search succeeds. Test `KT-96 kt_search_after_login`.
4. Logout during an in-flight refresh: the refresh result must not re-store a token after logout. Test `KT-97 kt_logout_wins_over_refresh`: logout sets token None while refresh is awaiting; the refresh result is discarded (guard: compare a generation counter). Flagged proposal, needs a generation counter in `AuthSession`.

---

## 6. Manual verification against the live service (one-time, before enabling by default)

Read-only where possible. Use a throwaway Kitsu account. No secrets in logs.

1. Login with a test account; confirm `users?filter[self]=true` returns the id string.
2. Search "one piece" (manga) and "naruto" (anime); confirm Algolia returns hits and `novel` is absent.
3. Add a library entry (planned), PATCH progress and rating, GET it back, DELETE it. Record the status codes.
4. Confirm the PATCH with numeric `data.id` is accepted (`KT-85` assumption). If rejected, switch to string id in Rust only.
5. Confirm the date wire format: PATCH `startedAt` once with a UTC time, read it back, and compare with the device zone. Documents the local-zone defect in production.
6. Force a refresh (set `created_at` in a copied token to the past in a test store, not the live account) and confirm the refresh response shape and whether `refresh_token` rotates. Record the answer in T-30a-027.
7. Check whether authed GET responses are cacheable (`Cache-Control` header). Record in T-30a-025.

Results go into the `Evidence` field of the relevant task before implementation starts.

---

## 7. Failure-mode checklist (must all be covered by tests or a flagged decision)

- No network: every method returns `Transport` and changes no local state. (Add test `KT-98 kt_offline_no_state_change`.)
- Server 5xx on add: no entry created; track keeps `remote_id` = media id; retry is the caller's job (area 27 delayed-update store).
- Server 5xx on PATCH: local row unchanged; caller delays.
- Server 429 (unknown whether Kitsu rate-limits; not in source): treated as `Http(429)` with no retry in the Kitsu layer. Test `KT-99 kt_429_surfaces`.
- Partial JSON (truncated body): `Decode` error, no state change.
- Clock skew: a token that looks valid to the client but is rejected → 401 → `Http(401)`; no automatic re-login (password not stored). Surface as `ReauthRequired` in the UI (T-30a-031).
