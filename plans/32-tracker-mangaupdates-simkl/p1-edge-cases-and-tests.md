# Area 32: MangaUpdates + Simkl trackers, part 1 - edge cases and tests

Upstream tests: none of the 22 scope files is a test file, so there is no upstream `*Test.kt` in scope to port. A search of every `*Test*.kt` file in the Aniyomi tree for `simkl` or `mangaupdates` (case-insensitive) found no match. The tests below are new.

Test ids: `E-MU-n` (MangaUpdates edge), `E-SK-n` (Simkl edge), `E-X-n` (cross-cutting), `TST-n` (test to write). Behaviour references are `B-x.y` into `p1-behavior.md`; wire references `D-x.y` into `p1-data-and-config.md`; design references `R-x.y` into `p1-rust-design.md`.

Contents: 1 test strategy and fixtures; 2 MangaUpdates edge cases; 3 Simkl edge cases; 4 cross-cutting edge cases and races; 5 defect regression tests; 6 unit and integration test list; 7 fixtures to record.

---

## 1. Strategy and fixtures

- Pure-logic tests (no network): score table, status mapping, URL normalisation, DTO parsing, body construction, `htmlDecode` parity. Run with `cargo test -p kuro-track`.
- HTTP tests: `wiremock` (dev-dependency, test-only) serving recorded JSON fixtures. Each test asserts the exact request sequence (method, path, query, body key order) against B-2.9..2.12 and B-3.8.
- Live tests: `#[ignore]` by default, run manually with a throwaway account (env `KURO_MU_USER`, `KURO_MU_PASS`, `KURO_SK_CODE`). They verify O-1 (404 vs 200 for unlisted series), O-4 (movie path), O-5 (history semantics).
- No test may hit the network by default. CI runs the non-ignored set only.
- Fixtures live in `crates/kuro-track/tests/fixtures/mangaupdates/` and `.../simkl/`; file names below. Content is the minimal JSON that exercises the field, with invented ids. Do not commit real session tokens or real user ids.

---

## 2. MangaUpdates edge cases

- **E-MU-1 (score index 0)**: `index_to_score(0)` = 0.0 (B-2.2). Value `"-"` maps to 0.0, which then sends DELETE rating (B-2.10). Test TST-1.
- **E-MU-2 (score index 91)**: `"10.0"` maps to 10.0. Index 92 (one past the end) must return `ScoreOutOfRange(92)`, not panic (fix D-MU-7). TST-2.
- **E-MU-3 (score strings never produced)**: no `"0.x"` entries exist (B-2.2). A test asserts `SCORE_LIST.len() == 92`, `SCORE_LIST[1] == "1.0"`, `SCORE_LIST[10] == "1.9"`, `SCORE_LIST[11] == "2.0"`, `SCORE_LIST[91] == "10.0"`, and that no entry starts with `"0."`. TST-3.
- **E-MU-4 (display of 0.0)**: Kotlin shows `"0.0"`; port shows `"-"` (fix D-MU-8). A test pins the port value and documents the change. Display of 7.5 is `"7.5"`, of 10.0 is `"10.0"` (Kotlin `Double.toString`). TST-4.
- **E-MU-5 (chapter truncation)**: `last_chapter_read = 12.9` sends `"chapter": 12` (B-2.10). `0.0` sends `0`. TST-5.
- **E-MU-6 (rating guards)**: score `-1.0` sends no rating request (B-2.10 first guard). Score `7.5` sends PUT with `{"rating":7.5}`. Score `0.0` with "known remote rating absent" sends no DELETE (fix D-MU-3, O-3). Score `0.0` with "remote rating known present" sends DELETE. TST-6 (four cases).
- **E-MU-7 (rating fetch failure)**: M2 returns 500. `RatingFetch::Unknown`. bind keeps the previous score (fix D-MU-3). Kotlin would set 0.0. Regression TST-7.
- **E-MU-8 (bind 404 vs other error)**: M1 returns 404 -> add path (B-2.7 fix D-MU-2). M1 returns 500 -> error, no add request sent. M1 returns 401 -> error, no add. TST-8 (three cases, assert request log).
- **E-MU-9 (bind add with has_read)**: add with `hasReadChapters=true` sends list `0` and, on code 200, sets `last_chapter_read = 1.0`; with false sends list `1` and leaves `last_chapter_read` unchanged (fix D-MU-4 applied). On code 201 status is not written (B-2.9 note; the port treats 201 as success for the write, see O-note in B-2.9; TST asserts the port behaviour). TST-9.
- **E-MU-10 (update status rule)**: status 2 (complete) with `did_read = true` keeps 2. Status 4 with `did_read = true` becomes 0. Status 1 with false stays 1. TST-10.
- **E-MU-11 (search body key order and content)**: body is exactly `{"search":"<q>","filter_types":["drama cd","novel"]}`. Query with quotes and unicode is JSON-escaped. TST-11 (byte-compare).
- **E-MU-12 (search record nulls)**: record with `type: null` and `year: null` gives `publishing_type == ""` and `start_date == ""` (fix D-MU-5). Kotlin gives `"null"`. TST-12.
- **E-MU-13 (search record missing title)**: `title: null` gives `""`. Kotlin also gives `""`. Parity. TST-13.
- **E-MU-14 (htmlDecode parity)**: input `Tom &amp; Jerry <i>x</i>` decodes to `Tom & Jerry x`. Input with malformed `<b>unclosed` gives `unclosed` in Kotlin (parser closes). Port result must match on the fixture set; mismatches are logged as known differences. TST-14 (fixture list of 12 strings in `htmldecode_cases.json`).
- **E-MU-15 (login success)**: response `{"context":{"session_token":"T","uid":42}}` stores username `"42"` and password `"T"`; `is_logged_in` true; the typed username is not stored. TST-15.
- **E-MU-16 (login failure)**: 401 response: no credentials stored, `logout` run, error text `HTTP 401` shown. TST-16.
- **E-MU-17 (login response missing context)**: body `{}` gives a parse error, not a panic. TST-17.
- **E-MU-18 (logout clears token)**: after `logout()`, a subsequent list call fails with `NotAuthenticated("MangaUpdates")` without any request (fix D-MU-1). Kotlin would still send the old token. Regression TST-18.
- **E-MU-19 (delete body)**: body is exactly `[<remote_id>]`. TST-19.
- **E-MU-20 (refresh with rating failure)**: refresh with M2 failing keeps score (fix D-MU-3) and does not error (Kotlin's `refresh` lacks try, but the rating fetch was already swallowed; parity on error propagation for M1 failures: propagate). TST-20.
- **E-MU-21 (list id unknown)**: `list_id: 9` from server maps to status 9 and is displayed through `status_label_key` as None (no label). The picker does not show it; the row shows the raw number (Kotlin behaviour). TST-21.
- **E-MU-22 (progress zero on missing status)**: `status: null` gives `last_chapter_read = 0.0` (B-2.7). TST-22.
- **E-MU-23 (rating double precision)**: score `9.9` is sent as `9.9` (no rounding); `10.0` as `10.0`. TST-23.
- **E-MU-24 (User-Agent)**: header is `Kurobayomi v<ver> (<app id>)` on every authenticated request; `reqwest` default UA is not sent. TST-24.

---

## 3. Simkl edge cases

- **E-SK-1 (TV lookup defect)**: an already-listed TV show (watched returns result true, last_watched and list present, all-items returns it under `tv`) must bind successfully. Kotlin throws `Unknown type: shows` (D-SK-1). Regression TST-25 with a `tv` key fixture.
- **E-SK-2 (search tracking_url normalisation)**: result `url = "https://simkl.com/anime/1234/some-slug"` gives stored `tracking_url = "/anime/1234"` and display `source_url` = the absolute URL (fix D-SK-2). TST-26 (three kinds: anime, tv, movie -> `/movies/<id>`).
- **E-SK-3 (absolute URL with no known segment)**: `url = "https://simkl.com/x/1"` with kind Tv gives `/tv/1` (fallback). TST-27.
- **E-SK-4 (movie add goes to movies)**: after normalisation, a movie search result added sends `"movies"` as the body key (fixes the shows-list bug). TST-28.
- **E-SK-5 (unknown status)**: status 0 (rewatching) or 9 gives `UnknownStatus` before any request is sent (fix D-SK-7). Kotlin throws NotImplementedError after building the body, after the progress calls for update. TST-29: assert zero requests sent for `update` with status 0 (because validation runs first, PROPOSAL in R-5.3).
- **E-SK-6 (from-wire statuses)**: `"dropped"` and `"notinteresting"` both give 4; output of 4 is `"notinteresting"`. `"hold"` gives 3. Unknown `"paused"` gives `UnknownWireStatus`. TST-30.
- **E-SK-7 (movie episodes)**: movie with `status = "completed"` gives `last_episode_seen = 1.0`, `total_episodes = 1`; other statuses give 0.0. TST-31.
- **E-SK-8 (show missing counts)**: `total_episodes_count: null` in a show gives `MissingField("total_episodes_count")` (fix of `!!`), not a panic. TST-32.
- **E-SK-9 (negative score)**: score `-1.0` (search default) reaching `update` sends `rating: -1` in Kotlin (B-3.8.3). Port: `score: None` is never sent; if reached with `Some(-1.0)` the port skips rating and logs a warning. The test asserts the port behaviour and records the Kotlin difference. TST-33.
- **E-SK-10 (progress for zero episodes)**: `last_episode_seen = 0.0` sends `episodes: []` in S8 and the remove in S7 (B-3.8.1). TST-34.
- **E-SK-11 (progress count truncation)**: `last_episode_seen = 12.9` sends episodes 1..12. TST-35.
- **E-SK-12 (progress order)**: S7 then S8 then S6 then S9/S10 in that order (B-3.8). Assert the request log order. TST-36.
- **E-SK-13 (history add failure after remove)**: S8 returns 500 after S7 succeeded. Kotlin leaves history removed (D-SK-5). Port: test asserts the error is returned and the row is NOT updated locally (so the next sync retries). The server-side loss is documented as a known risk until O-5 is decided. TST-37.
- **E-SK-14 (rating zero)**: score `0.0` sends S10 with `rating: 0` (B-3.8.3). Score `7.9` sends S9 with `rating: 7`. TST-38.
- **E-SK-15 (login swallow)**: code exchange returns 400: `login_code` returns Err, the credentials are cleared, `is_logged_in` false (B-3.7). The UI shows the dialog error (PROPOSAL) and not a toast. TST-39.
- **E-SK-16 (user id failure after token)**: S2 ok, S3 fails: the token was stored in memory and in the secret store; the logout clears both (no half-logged-in state). TST-40.
- **E-SK-17 (logout writes no "null")**: after logout the secret store has no `track.101.token` entry (fix D-SK-4). Kotlin stores the literal `"null"`. TST-41.
- **E-SK-18 (stored token "null")**: an imported Aniyomi `track_token_101 = "null"` migrates as absent (B-3.11). TST-42.
- **E-SK-19 (search partial failure)**: the `tv` request fails: whole search returns Err (B-3.14). The UI shows the error text. TST-43.
- **E-SK-20 (search ordering)**: results are concatenated anime, tv, movie (no sort) (B-3.14). TST-44.
- **E-SK-21 (title fallback)**: `title_romaji: null`, `title: null` gives empty title, not panic (fix D-SK-8). TST-45.
- **E-SK-22 (summary prefix)**: `all_titles: []` gives `"All titles:\n"`; `null` gives `""`. TST-46.
- **E-SK-23 (poster URL)**: `poster: "abc"` gives `https://simkl.in/posters/abc_m.webp`. `poster: null` gives `""`. TST-47.
- **E-SK-24 (date_from opacity)**: `last_watched` containing spaces or `+` is percent-encoded once in the query (Uri builder behaviour) and not parsed. TST-48 (fixture with `2024-01-02 03:04:05+00:00`).
- **E-SK-25 (bind keeps library_id)**: remote has no library id; local `library_id = Some(5)` stays 5 (fix D-SK-3). TST-49.
- **E-SK-26 (refresh not found)**: `findLibAnime` returns None: refresh returns the track unchanged, no error (B-3.15). TST-50.
- **E-SK-27 (missing account id)**: S3 without `account` gives a parse error (not default 0). TST-51.
- **E-SK-28 (token JSON shape)**: stored token JSON is exactly `{"access_token":"<t>"}` (compact, no spaces). TST-52.
- **E-SK-29 (client_id in search query)**: search URL contains `client_id=<CLIENT_ID>` and no Authorization header (B-3.14). TST-53.
- **E-SK-30 (interceptor headers)**: authenticated calls send `Authorization: Bearer <t>`, `simkl-api-key: <CLIENT_ID>`, User-Agent. Missing token gives `NotAuthenticated("Simkl")` without a request. TST-54.
- **E-SK-31 (authorise URL)**: `https://simkl.com/oauth/authorize?response_type=code&client_id=<CLIENT_ID>&redirect_uri=aniyomi%3A%2F%2Fsimkl-auth` exactly (byte compare). TST-55.
- **E-SK-32 (paste parsing)**: input `aniyomi://simkl-auth?code=ABC123` gives `ABC123`; bare `ABC123...` (length 20-200, allowed chars) gives itself; `https://example.com/?code=x` gives an error (wrong host). Keep the host check: only `simkl-auth` host accepted. TST-56.
- **E-SK-33 (redirect without code)**: `aniyomi://simkl-auth` (no code) calls logout (B-3.7 / TrackLoginActivity.kt:79-82) and reports nothing. TST-57.
- **E-SK-34 (code not logged)**: `tracing` output for a login attempt contains no code and no token. TST-58 (capture subscriber).
- **E-SK-35 (runBlocking removal)**: `current_user_id` is async and does not block a worker: a test with a single-thread runtime completes a concurrent task while the request is pending. TST-59.

---

## 4. Cross-cutting edge cases and races

- **E-X-1 (concurrent update of the same track)**: two `update` calls for the same track from the reader and from the library job overlap. Each sends its full sequence; server state converges to the last writer. Port: serialise per track id with a `tokio::sync::Mutex` keyed by track id (prevents interleaved S7/S8). TST-60.
- **E-X-2 (window closed mid-sequence)**: dropping the update future between S7 and S8 leaves history removed (E-SK-13 class). Port spawns the pair on the runtime (R-8). TST-61: drop the caller future after S7 is sent; assert S8 is still sent.
- **E-X-3 (token expiry)**: 401 on any authenticated call: the error text is shown; the credentials are not cleared automatically (the Kotlin also does not clear). PROPOSAL: set `auth_expired` flag; UI shows "Session expired". TST-62.
- **E-X-4 (no network)**: `reqwest` connect error: `TrackError::Network`; no state change. TST-63.
- **E-X-5 (secret store absent)**: Secret Service not running: the credential store falls back to prefs and the settings row shows the unencrypted warning (R-7). TST-64.
- **E-X-6 (migration of old prefs)**: old `pref_mangasync_password_7` moves to the secret store once and the old key is deleted. A second start does not re-migrate. TST-65.
- **E-X-7 (logged-in flow)**: `login_state` watch channel emits `true` after login and `false` after logout (both services). TST-66.
- **E-X-8 (cover loading failure)**: 404 on the poster URL gives the placeholder image; no error toast. TST-67.
- **E-X-9 (large history)**: a show with 2000 episodes sends one S8 body of about 2000 objects (no batching); body size under 64 KiB. TST-68 (size check only).
- **E-X-10 (clock)**: no time values are sent by MU or SK; `started/finished` dates stay local. TST-69 (assert no body contains `date` or `time`).
- **E-X-11 (database default)**: a new `MangaTrack` for MU has `private = false` and `remote_url = ""` (never null). TST-70.
- **E-X-12 (search cancel)**: a second search while the first is pending cancels the first future; the UI shows only the last result (race in the list model). TST-71.

---

## 5. Defect regression tests (one per defect in p1-behavior section 8)

| Defect | Test | Assertion |
|---|---|---|
| D-SK-1 TV lookup throws | TST-25 | bind succeeds, `library` item found |
| D-SK-2 absolute tracking_url | TST-26, TST-28 | stored `/type/id`; movie add uses `movies` |
| D-SK-3 library_id cleared | TST-49 | local value kept |
| D-SK-4 "null" token | TST-41 | no `"null"` value anywhere |
| D-SK-5 remove-then-add loss | TST-37 | error returned; local row not updated; documented |
| D-SK-6 rating each update | (behaviour change, optional) TST-72: skip rating when unchanged | request count |
| D-SK-7 unknown status throw | TST-29 | `UnknownStatus` with zero requests |
| D-SK-8 title NPE | TST-45 | empty title, no panic |
| D-SK-9 runBlocking | TST-59 | concurrency |
| D-SK-10 class name | (naming only) | compile check |
| D-SK-11 client_id in query | TST-53 | present (kept, same as Kotlin) |
| D-MU-1 logout keeps token | TST-18 | request refused after logout |
| D-MU-2 bind catch-all | TST-8 | 404 adds, 500 does not |
| D-MU-3 rating 0.0 on fetch fail | TST-6, TST-7 | no DELETE on unknown; score kept |
| D-MU-4 add last_chapter 1.0 | TST-9 | set only with has_read |
| D-MU-5 "null" strings | TST-12 | empty strings |
| D-MU-6 dead Throwable | (naming only) | none |
| D-MU-7 index out of range | TST-2 | `ScoreOutOfRange` |
| D-MU-8 display 0.0 | TST-4 | `-` |
| D-MU-9 toast "null" | TST-73: login failure toast with null message shows "Login failed: HTTP 401" | text |
| D-MU-10 | (kept) | none |

---

## 6. Test list (unit and integration)

Pure unit (kuro-track):
- TST-1 score 0 and `"-"`; TST-2 index bounds; TST-3 score table shape; TST-4 display; TST-5 chapter truncation; TST-23 rating precision.
- TST-29 status validation order; TST-30 from-wire mapping; TST-31 movie episodes; TST-34, TST-35 progress arithmetic.
- TST-26, TST-27 URL normalisation; TST-55 authorise URL; TST-56 paste parsing.
- TST-46, TST-47 summary and poster; TST-45 title fallback; TST-14 htmlDecode parity; TST-22 status null.
- TST-52 token JSON shape; TST-42 "null" migration.

HTTP (wiremock) with request-log assertions:
- TST-6, TST-7, TST-8, TST-9, TST-10, TST-11, TST-15, TST-16, TST-17, TST-18, TST-19, TST-20, TST-21, TST-24.
- TST-25, TST-28, TST-29, TST-33, TST-36, TST-37, TST-38, TST-39, TST-40, TST-43, TST-44, TST-48, TST-49, TST-50, TST-51, TST-53, TST-54.
- TST-61 (cancellation), TST-60 (per-track mutex), TST-62 (401), TST-63 (no network), TST-68 (body size).

Secret store and prefs:
- TST-41 (no "null"), TST-64 (fallback warning), TST-65 (migration once), TST-66 (watch channel), TST-70 (defaults).

Logging and privacy:
- TST-58 (no code or token in logs), TST-69 (no time in bodies).

Live (ignored by default):
- TST-74: MU unlisted series id returns 404 on M1 (O-1). Record the status.
- TST-75: MU rating GET for a series with no rating (O-1 related, D-data 4.1 M2 shape).
- TST-76: SK movie add-to-list with `movies` key then `sync/watched` returns the item (O-4).
- TST-77: SK history remove then add, then `sync/watched` count (O-5).
- TST-78: SK search response for each kind and the `url` format (confirms R-5.7).

---

## 7. Fixtures to record

Directory `crates/kuro-track/tests/fixtures/mangaupdates/`:
- `list_item_reading.json` (`{"series":{"id":101,"title":"Example"},"list_id":0,"status":{"volume":1,"chapter":12},"priority":0}`)
- `list_item_null_status.json` (`status` null)
- `rating_7_5.json` (`{"rating":7.5}`), `rating_null.json` (`{"rating":null}`)
- `login_ok.json` (`{"context":{"session_token":"<fake>","uid":42}}`), `login_missing_context.json` (`{}`)
- `search_ok.json` (two results, one with `type` null and `year` null), `search_empty.json` (`{"results":[]}`)
- `htmldecode_cases.json` (12 strings: entities, tags, malformed, empty, unicode)
- Error bodies: `err_401.json`, `err_404.json`, `err_500.json` (plain text bodies are fine)

Directory `crates/kuro-track/tests/fixtures/simkl/`:
- `oauth_ok.json` (`{"access_token":"<fake>"}`), `oauth_400.json`
- `user_settings.json` (`{"account":{"id":7}}`), `user_settings_missing.json` (`{}`)
- `watched_ok.json` (`[{"result":true,"last_watched":"2024-01-02 03:04:05+00:00","list":"watching"}]`), `watched_empty.json` (`[]`), `watched_result_false.json`
- `all_items_tv.json` (key `tv` with one show: `show.title`, `show.ids.simkl`, `total_episodes_count`, `watched_episodes_count`, `user_rating`), `all_items_movies.json` (key `movies`), `all_items_anime.json` (key `anime`), `all_items_show_null_counts.json`
- `search_anime.json`, `search_tv.json`, `search_movie.json` (each with `ids.simkl_id`, `title_romaji`, `ep_count`, `poster`, `all_titles`, `url`, `status`, `type`, `year`); one record with `title_romaji: null, title: null`; one with `all_titles: []`; one with `poster: null`.
- `search_tv_500.json` (error for partial failure)

Note: all fixture values are invented; the real response keys for `all-items` and `search` are unverified (O-8). Each fixture is replaced by a recorded live response on the first ignored-live run (TST-74..78) and the test is re-run.

---

## 8. Acceptance for this file
- Every E-* item above maps to a TST-* test or to a stated "naming/none" note.
- Every defect in p1-behavior section 8 has a regression entry (section 5).
- Live tests are the only path that resolves O-1, O-4, O-5 and O-8.
