# area:28-tracker-anilist, part 1 — edge cases, failure modes, tests

Repo root `/home/blackbox/aniyomi-src`. Each edge case has an ID (E-nn) referenced by tasks. Status column: **BUG** = defect in the Kotlin (keep the Rust behaviour correct); **QUIRK** = behaviour that must be reproduced or consciously changed; **RISK** = not a defect today, but fragile.

## 1. Edge cases

### 1.1 Authentication and tokens

| ID | Status | Scenario | Kotlin behaviour (path:line) | Required Rust behaviour |
|---|---|---|---|---|
| E-01 | BUG | Login (token exchange or Viewer query) fails | `login(token)` catches `Throwable` and calls `logout()`; no toast, no error to caller (`ANI:353-355`) | Return `Err`, show toast `anilist_err_login_failed` with the message; state remains LoggedOut |
| E-02 | RISK | Redirect fragment has `access_token` as last parameter | Regex `(?:access_token=)(.*?)(?:&)` needs a trailing `&`; no match → silent logout (`TrackLoginActivity.kt:20-30`) | Parse fragment as form data; no delimiter requirement |
| E-03 | BUG | Token expiry | `expires = now_ms + 31536000000` then setter `*1000 - 60000` → year ~57000; `isExpired` never true (`API:516`, `INT:18-21`, `DTO/ALOAuth.kt:17`) | Store absolute expiry in seconds; check before request; treat 401 as expiry |
| E-04 | BUG | Stored OAuth missing/corrupt while token pref non-empty | `if (oauth!!.isExpired())` at `INT:33` throws NPE before the null check at `INT:39`; message `null`, toast shows `NullPointerException: null` | Return `NotAuthenticated`; clear the token |
| E-05 | QUIRK | Logout stores the text `null` in `track_token_2` | `saveOAuth(null)` encodes `null` (`ANI:364-366`, `ANI:358-362`) | Delete the row; never store the literal `null` |
| E-06 | RISK | Username not numeric | `getUsername().toInt()` (`ANI:215,241,264,273,281,301,329,337`) throws `NumberFormatException` | Store viewer id as i64; no string parse |
| E-07 | RISK | Manga search result with no staff | `staff = staff!!` in `ALSearchItem.toALManga` (`DTO/ALSearchItem.kt:30`) → NPE | Staff optional; empty author list |
| E-08 | RISK | Anime search/user-list media without studios | `studios!!` in `toALAnime` (`DTO/ALSearchItem.kt:43`) → NPE | Optional; empty list |
| E-09 | RISK | `title`, `coverImage`, `startDate` assumed non-null | DTO non-null types (`DTO/ALSearchItem.kt:8-13`) → kotlinx fails if AniList omits them | `Option` for all but `id`; defaults documented |
| E-10 | RISK | Staff name has all three name fields null | `ALStaffName.invoke()` returns null → entry skipped (`DTO/ALManga.kt:43`) | Same: skip the name |
| E-11 | RISK | `library_id` not persisted after bind-add | `addLibX` discards the returned id (`API:73-76,167-169`); the framework inserts the track before any update | Capture id from `SaveMediaListEntry` and persist it in the same write |
| E-12 | QUIRK | Extra find query on first update after add | Consequence of E-11 | Avoid when E-11 fixed |
| E-13 | BUG | POINT_3 unscored shows "0"; picker list has "-" | `displayScore` returns `"0"` (`ANI:176-177`); `indexOf("0")` = -1 → wheel starts at middle (`TrackInfoDialogSelector.kt:135`) → confirm stores 60 | Index-based picker; unscored opens at index 0 |
| E-14 | BUG | Picker value not found in list | `indexToScore(-1)`: POINT_10 → -10.0, POINT_5 → -30.0, POINT_3 → -15.0 (`ANI:146-166`) → sent as negative `scoreRaw` | Index-based; `index_to_score` returns `Err(ScoreOutOfRange)` |
| E-15 | RISK | Unknown score format from AniList | `Exception("Unknown score type")` on list/index/display (`ANI:132,164`, `UTL:55`) | Return `UnknownScoreFormat`; fall back to POINT_10 with warning |
| E-16 | RISK | Unknown MediaListStatus | `NotImplementedError("Unknown status: …")` (`DTO/ALManga.kt:80`, `DTO/ALAnime.kt:81`) → track fails to load | Return `UnknownStatus`; keep the row, show label "Unknown" |
| E-17 | QUIRK | `bind` overwrites local progress with remote progress | `copyPersonalFrom(remote, false)` copies `last_*_read` (`MangaTrack.kt:38-41` DB version) | Same; then the framework pushes local-ahead progress (area:27) |
| E-18 | BUG | Bind keeps local `private` on the local row, but `update` sends local `private` to AniList | `copyRemotePrivate=false` (`ANI:283`) then `updateLibManga` sends `track.private` (`API:108`) | Decide: local choice wins (document) or remote wins. Proposal: local choice wins, explicitly, with a note in the bind dialog |
| E-19 | QUIRK | Remote delete failure invisible | `deleteAnimeFromService` catches and logs at ERROR, no toast (`AnimeTrackInfoDialog.kt:860-868`) | Toast `anilist_err_remote_delete_failed` |
| E-20 | BUG | Delete when `libraryId` null/0 sends `listId: null` | `track.copy(id = libAnime.library_id!!)` sets the local row id; `deleteLibAnime` reads `track.libraryId` (`ANI:271-278`, `ANI:262-269`, `API:223`) | `copy(library_id = Some(id))`; assert non-null before mutation |
| E-21 | QUIRK | Delete when entry not found remotely | `findLib… ?: return` → silent success (`ANI:264,273`) | Return `Ok(())` with info log "already absent on AniList" |
| E-22 | BUG | Mutation errors ignored | `updateLibX`, `deleteLibX` do not read body (`API:111-113,133-135,204-206,226-228`); GraphQL `errors` in HTTP 200 are success | Parse `errors` for every mutation |
| E-23 | QUIRK | Started date reset on every read event at progress 1.0 | `if (last_chapter_read == 1.0) started = now` (`ANI:227-229`, `ANI:252-254`) | Reproduce, or only set when the stored date is 0 (proposal; document change) |
| E-24 | QUIRK | Reread in progress | `status != REREADING` guard keeps REREADING on non-final read events (`ANI:225`) | Same |
| E-25 | QUIRK | Read/watch event moves ON_HOLD, DROPPED, PLAN_TO_* to READING/WATCHING | `update(track, didRead=true)` sets READING unless REREADING/completed (`ANI:225-226`, `ANI:250-251`) | Reproduce (user expectation: progress implies active). Mention in settings subtitle? (no change) |
| E-26 | QUIRK | Manual episode set to total while REWATCHING | `setRemoteLastEpisodeSeen` sets COMPLETED when total reached without checking rewatching (`AnimeTracker.kt:83-86`) | Reproduce |
| E-27 | RISK | Date time-of-day and timezone | `createDate` sends local Y-M-D only (`API:561-566`); read back as local midnight (`ALFuzzyDate.kt:15-17`). Changing timezone shifts displayed date | Store local midnight; document; use chrono `Local` |
| E-28 | QUIRK | Partial fuzzy date (year only) | `toEpochMilli` → 0 (`ALFuzzyDate.kt:18-19`) → display blank; sending it back clears the date (E-29) | Keep partial date as a separate value in DB? Proposal: store 0 and do not send nulls for an unset-but-remote-set date; document the loss |
| E-29 | BUG | Update clears remote dates when local date 0 | `createDate(0)` → nulls (`API:553-558`) sent in every UpdateX, so if local start date is 0 but remote has a start date, it is erased | Send the remote value when local is 0 and the remote is known; or omit the field (Option) when unset. Proposal: omit when 0 |
| E-30 | BUG | External-player path updates without refresh | `ExternalIntents.kt:539-546`: `track.copy(lastEpisodeSeen=…)` then `update(…, true)` — overwrites remote score/status/dates with stale local values | Use `refresh` first (same as `TrackEpisode`) |
| E-31 | QUIRK | Offline queue differs anime vs manga | Anime queues only when offline; API errors are swallowed (`TrackEpisode.kt:191-205`). Manga queues on any exception (`TrackChapter.kt:46-51`) | Unify: queue on transient/offline/rate-limited; drop on NotInList/AuthExpired (area:27) |
| E-32 | BUG | Auto-track of fractional episode | `setRemoteLastEpisodeSeen(track, latest.toInt())` truncates 12.5 → 12 (`AddAnimeTracks.kt:55`) | Local progress stays Double; remote progress is sent as Int (`toInt()`), so truncation toward zero is the defined behaviour; document |
| E-33 | QUIRK | Non-integer episode/chapter numbers sent as `progress` | `progress = last_episode_seen.toInt()` (`API:153,196`) | Same; document |
| E-34 | QUIRK | Search needs login | Interceptor throws `Not authenticated with Anilist` (`INT:26-28`) for `search`/`searchAnime` too | Keep; see design §14 |
| E-35 | QUIRK | Search results capped at 50 | `Page(perPage: 50)` (`API:236,297`), no pagination | Same; no "load more" |
| E-36 | RISK | HTML decode differences | Android `Html.fromHtml` vs Rust (p1-behavior §9) | Accept minor whitespace differences; test with fixtures |
| E-37 | QUIRK | Status text keeps underscores in search | `publishing_status` raw (`DTO/ALSearchItem.kt:26`) and display lowercases+capitalises (`AnimeTrackerSearch.kt:247`) → "Not_yet_released" | Replace `_` with space in display only |
| E-38 | QUIRK | Format `_` → `-` | `format?.replace("_", "-")` (`DTO/ALSearchItem.kt:25,38`) → "One-shot" | Same |
| E-39 | RISK | `tracking_url` not refreshed | `refresh` keeps existing `remote_url`; `findLib…` maps do not set it (`DTO/ALManga.kt:60-71`) | Set URL from `remote_id` on every write (proposal) |
| E-40 | RISK | Rate-limit constant | Source uses 85 per 60 s (`API:42`). AniList's own documented limit was not checked in this area (docs unreachable) | Keep 85 as configurable constant; revisit with docs |
| E-41 | RISK | Race on lazy OAuth load | `oauth` var not synchronized (`INT:18`); parallel `async` refreshes | Use `RwLock`; load once |
| E-42 | QUIRK | Dead branches in `toApiScore` | POINT_5/POINT_3 branches unreachable from `displayScore` (`UTL:38-52`) | Not ported |
| E-43 | RISK | AniSkip MAL id lookup | `getMalIdFromAL` returns 0 on failure; caller calls AniSkip with 0 (`AniSkipApi.kt:46-65`, `PlayerViewModel.kt:2045-2050`) | Return `None` on failure; skip AniSkip |
| E-44 | RISK | AniSkip/AniChart parse by substring | `substringAfter("idMal\":")` etc. (`AniSkipApi.kt:63`, `AniChartApi.kt:72,102-103`) | serde parse |
| E-45 | QUIRK | AniChart defaults | Missing `nextAiringEpisode` → episode 1, airingAt 0 (`AniChartApi.kt:102-103`) | Same default; show nothing when airingAt 0 |
| E-46 | QUIRK | Event with episode ≤ last seen | `TrackEpisode` skips (`TrackEpisode.kt:186`) | Same |
| E-47 | RISK | Remote delete after local delete | `unregisterTracking` and `deleteAnimeFromService` launched independently (`AnimeTrackInfoDialog.kt:833-834,870-872`) | Sequence: remote first, then local; if remote fails, keep local and toast |
| E-48 | QUIRK | Logout does not remove local tracks | Framework behaviour (area:27) | Same |
| E-49 | RISK | No handling of HTTP 401 | `HttpException(401)` surfaces as "HTTP error 401" (`OkHttpExtensions.kt:112-115`) | Map to `AuthExpired` (E-03 fix) |
| E-50 | QUIRK | Score format stored at login only | `anilist_score_type` refreshed only on login (`ANI:351`) | Refresh on each Viewer fetch; re-login after changing AniList score format |
| E-51 | RISK | Stored score scale after format change | Stored score is always 0–100 (`API:105`, `ANI:136-144`) → no migration needed | Same |
| E-52 | RISK | ClassCastException on old int preference | `init` logs out silently (`ANI:70-75`) | Not needed in Rust (new store); migration only if importing a v1 backup |
| E-53 | QUIRK | Dates from DB: `started_watching_date` vs `started_reading_date` naming | Anime/manga field names differ | Use one `Dates { started, finished }` struct per kind |
| E-54 | BUG | `search` and `searchAnime` throw before network when logged out | Same as E-34 | see E-34 |
| E-55 | RISK | `Track privately` on search applies to remote | `selected.private = private` then `register` (`AnimeTrackInfoDialog.kt:707-711`) | Same; verified by fixture test that `private: true` is sent |
| E-56 | RISK | Score of search results | `score = averageScore` (0–100) shown as Double text (`AnimeTrackerSearch.kt:335-339`) | Show as integer; keep value |
| E-57 | QUIRK | Track search preselect by URL | `tracking_url == currentUrl` (`AnimeTrackInfoDialog.kt:750`) | Compare `remote_id` instead (more robust) |
| E-58 | BUG | `updateRemote` toasts `e.message` for `NotImplementedError` | e.g. "Unknown status: 0" (`UTL:17`) | Localized messages |
| E-59 | QUIRK | Track date picker min/max | Start ≤ finish, finish ≥ start, nothing in future (`AnimeTrackInfoDialog.kt:477-518`) | Same |
| E-60 | QUIRK | Remove date sends 0 | `setRemoteStartDate(…, 0)` (`AnimeTrackInfoDialog.kt:670-672`) → `createDate(0)` → nulls sent (E-29) | Same semantics: unset |
| E-61 | RISK | Progress picker upper bound unknown total | `0..10000` (`AnimeTrackInfoDialog.kt:377-381`) | Same |
| E-62 | QUIRK | COMPLETED from picker sets progress to total | `setRemoteAnimeStatus` (`AnimeTracker.kt:67-73`) | Same |
| E-63 | RISK | Manga/anime status ints overlap | COMPLETED = 2 for both; READING=1 vs WATCHING=11 | Typed enums per kind; DB ints unchanged |
| E-64 | RISK | `private` default for search items | `false` (`AnimeTrackSearch.kt:33`) | Same |
| E-65 | QUIRK | Score "0" in list but unscored | Stored 0.0 means unscored everywhere (`AnimeTrackInfoDialogHome.kt:101`) | Same |
| E-66 | BUG | `AnimeTrackSearch.artists` exists for anime but never set | `artists` only assigned for manga (`ALManga.kt:45`) | Drop field for anime |
| E-67 | RISK | Auth header on search uses bearer | Same as E-34 | see E-34 |
| E-68 | RISK | Network offline during login | `login` → catch → logout (E-01) | see E-01 |
| E-69 | RISK | Two windows open editing the same track | Last write wins; `ON CONFLICT REPLACE` changes `_id` | Serialize writes per track in Rust (Mutex keyed by track id) |
| E-70 | QUIRK | Title updated by refresh | `track.title = remote.title` (`ANI:331`) | Same |

## 2. Failure modes (per operation)

| Operation | Failure | Observed Kotlin outcome | Rust outcome |
|---|---|---|---|
| search | not logged in | exception message "Not authenticated with Anilist" shown in result area | `NotAuthenticated` → same text localized |
| search | HTTP 500 | "HTTP error 500" | "AniList server error" + retry button |
| search | offline | `UnknownHostException` → "No Internet connection" via formatter when shown through `formattedMessage` | `Offline` → `anilist_err_offline` |
| bind (add) | duplicate on AniList (already in list) | AniList returns error; `addLibX` `parseAs` fails (no `data`) → serialization exception shown | `GraphQl(message)` shown |
| update | library id not found | `"$track not found on user library"` | `NotInList` "entry not found on your AniList list" |
| update | rate limited (429) | `HttpException(429)` → "HTTP error 429" | `RateLimited`, retried once after `retry_after` or 60 s |
| refresh | entry deleted on AniList | "Could not find anime" | `NotInList` |
| delete | entry missing | silent | info log |
| login | Viewer fails | silent logout | toast |

## 3. Race conditions and ordering

- **R-1 Concurrent progress events:** player and external player can fire `update(…, true)` for two episodes in quick succession. Each `TrackEpisode.await` refreshes first, so the second sees the first's remote state only if the first finished. Rust: serialize per-track operations with a `tokio::sync::Mutex` keyed by `(kind, track_id)`.
- **R-2 Refresh during update:** `RefreshAnimeTracks` may overwrite local progress set moments earlier. Rust: refresh must not run while a write for the same track is in flight.
- **R-3 Register while track exists:** `insert ... ON CONFLICT REPLACE` on `(anime_id, sync_id)` replaces the row; if two binds race, the last wins. Rust: check for existing binding inside the same transaction.
- **R-4 Auth change during request:** a logout while a request is in flight: the request already has the bearer header and succeeds or fails independently. Rust: capture the token at request start; on success after logout, discard the result (do not re-save track data for a logged-out tracker).
- **R-5 Redirect arriving twice:** the browser may send the redirect twice (double-open). Rust: ignore a redirect when the state is already LoggedIn and the token equals the stored one.

## 4. Fixtures needed (Rust tests)

Directory: `tests/fixtures/anilist/` (JSON response bodies, verbatim shapes from `dto/*.kt`).

| File | Content | Used by |
|---|---|---|
| `viewer_point10.json` | `{"data":{"Viewer":{"id":4242,"mediaListOptions":{"scoreFormat":"POINT_10"}}}}` | login |
| `viewer_point3.json` | same, `POINT_3` | score list |
| `viewer_point_decimal.json` | `POINT_10_DECIMAL` | score list |
| `viewer_unknown_format.json` | `"SOMETHING_NEW"` | E-15 |
| `search_manga_ok.json` | 3 media: one with full staff (Story + Art + "Story & Art"), one with null `averageScore`, one with `format: "ONE_SHOT"`, `status: "NOT_YET_RELEASED"`, `startDate {year:2024, month:null, day:null}` | search mapping, E-07, E-09, E-37, E-38, E-28 |
| `search_manga_no_staff.json` | `staff: null` | E-07 |
| `search_anime_ok.json` | `studios` with two `isMain: true` and one `false`; `episodes: null` | anime mapping, E-08 |
| `search_anime_no_studios.json` | `studios: null` | E-08 |
| `search_description_html.json` | description with `<br>`, `<i>`, `&amp;`, `&#39;` | E-36, html tests |
| `find_manga_found.json` | one mediaList entry, status `REPEATING`, score 85, progress 12, startedAt year-only, completedAt full, private true | mapping, bind, refresh |
| `find_manga_empty.json` | `mediaList: []` | bind add path, NotInList |
| `find_anime_found.json` | status `PAUSED`, `scoreRaw: 0`, dates all null | mapping |
| `save_entry_ok.json` | `{"data":{"SaveMediaListEntry":{"id":9001,"status":"CURRENT"}}}` | add returns id (E-11) |
| `update_entry_ok.json` | `{"data":{"SaveMediaListEntry":{"id":9001,"status":"CURRENT","progress":3}}}` | update |
| `errors_http200.json` | `{"errors":[{"message":"Invalid id","status":400}],"data":null}` | E-22 |
| `errors_http200_partial.json` | `data` with null field and `errors` | E-22 |
| `delete_ok.json` | `{"data":{"DeleteMediaListEntry":{"deleted":true}}}` | delete |
| `http_401.txt` (status only) | 401 with body `{"errors":[{"message":"Invalid token"}]}` | E-03 / E-49 |
| `http_429.txt` | 429 | rate limit |
| `http_500.txt` | 500 | transient |

All fixtures must be created by the implementer by hand from the DTO definitions (no live capture in this area). Mark them `"_fixture": true` only if a wrapper is needed; prefer raw bodies.

## 5. Upstream tests to port

None. There are no `*Test.kt` files for AniList or the track package in the repository (`app/src/test/java/mihon/core/migration/MigratorTest.kt` is unrelated; checked).

## 6. New tests to write (Rust)

Unit (pure, no network):
1. **status_roundtrip_manga** — each `MangaStatus` → `ApiStatus` → back is identity for the six values; `from_db(7)` is None.
2. **status_roundtrip_anime** — same for anime; WATCHING(11) maps CURRENT; REWATCHING(16) maps REPEATING.
3. **status_unknown_api_string** — `ApiStatus::from_str("FOO")` is `UnknownStatus`.
4. **score_options_per_format** — counts 11 (POINT_10), 101 (POINT_100), 6 (POINT_5), 4 (POINT_3), 101 (POINT_10_DECIMAL); exact strings for POINT_3 `["-","😦","😐","😊"]`, POINT_10_DECIMAL first `"0.0"` and last `"10.0"`, POINT_5 `"0 ★"`.
5. **index_to_score_table** — the table in p1-behavior §3.4 for every format and index; out-of-range → `ScoreOutOfRange`.
6. **display_score_table** — the rows in p1-data §11 for every format.
7. **score_index_roundtrip** — for every format and every stored value in the table, `index_to_score(score_to_index(v))` produces a value whose display equals display(v).
8. **point3_unscored_opens_at_zero** (regression for E-13): `score_to_index(0.0, POINT_3) == 0`.
9. **to_score_raw_clamps** — -10 → 0, 150 → 100, 85.4 → 85 (Kotlin `toInt()` truncates; match truncation for positives).
10. **authorize_url_exact** — equals `https://anilist.co/api/v2/oauth/authorize?client_id=5338&response_type=token`.
11. **parse_redirect_token_last** (E-02) — fragment `access_token=abc` only → token "abc".
12. **parse_redirect_denied** — fragment without token → `Denied`.
13. **parse_redirect_expires_in** — `expires_in=31536000` parsed.
14. **token_redacted_debug** — `format!("{:?}", token)` contains no token text.
15. **expiry_seconds** (E-03) — with stored expiry in the past → `Expired`; in the future → `LoggedIn`; boundary equal → `Expired` (use `>` consistent with Kotlin: `now > expires` is expired).
16. **legacy_ms_expiry_migration** — stored ms value converted to seconds.
17. **rate_limiter_85_per_minute** — fake clock: 85 acquires at t=0 succeed without waiting; 86th waits until t=60 s.
18. **rate_limiter_window_slides** — after 60 s, capacity restored.
19. **graphql_envelope_errors_on_200** (E-22) — `errors_http200.json` → `GraphQl("Invalid id")`.
20. **graphql_partial_errors** — `errors_http200_partial.json` → error, not partial success.
21. **http_status_mapping** — 401 → AuthExpired (and state Expired), 429 → RateLimited, 500 → Transient, 404 → Http(404).
22. **search_manga_mapping** — `search_manga_ok.json`: authors/artists per p1-behavior §7.3, format `ONE-SHOT`, start_date `""` for year-only date (E-28), score -1 for null averageScore, tracking_url `https://anilist.co/manga/<id>`.
23. **search_anime_authors_cap** — main studios first, cap 3.
24. **search_null_staff_no_panic** (E-07), **search_null_studios_no_panic** (E-08).
25. **fuzzy_date_zero_on_partial** — year-only → 0; Feb 30 → 0.
26. **fuzzy_date_local_midnight** — run under two `TZ` values set by the test runner (for example `TZ=America/New_York` and `TZ=Asia/Tokyo`); assert expected epoch values for each. No extra timezone crate needed.
27. **create_date_zero_omits** (E-29) — `Option::None` serialises to omitted field.
28. **create_date_local_ymd** — ms → `{year, month, day}` in local zone.
29. **html_decode_fixture** — `<br>` → `\n`, `&amp;` → `&`, `&#39;` → `'`, tags removed.
30. **user_agent_format** — `Kurobayomi v<version> (<app id>)`.
31. **dto_user_list_media_optional** — `media.staff` null accepted.
32. **dto_viewer_unknown_format** — parses, `ScoreFormat::from_api` errors (E-15).

Behaviour (with fake transport, table-driven):
33. **update_manga_read_last_chapter_completes** — total 12, last 12, didRead → status COMPLETED, finished date now (inject clock).
34. **update_manga_read_1_sets_started** — last 1.0, didRead, start 0 → start = now; with start already set → E-23 behaviour (decide; test the chosen rule).
35. **update_manga_rereading_stays** (E-24).
36. **update_anime_paused_becomes_watching** (E-25) — PAUSED + episode event → WATCHING.
37. **update_anime_completed_no_change** — COMPLETED stays, dates untouched.
38. **update_resolves_library_id** — library_id None → find called, id set, save called with it.
39. **update_not_in_list_error** — find returns empty → `NotInList`, no save call.
40. **bind_found_keeps_local_private** (E-18) — copies progress/score/status/dates, keeps local private.
41. **bind_found_rereading_kept** — REREADING remote with progress → status stays REREADING.
42. **bind_not_found_sets_planning** — hasRead false → PLAN_TO_READ; true → READING; score 0.
43. **bind_add_persists_library_id** (E-11) — `save_entry_ok.json` id 9001 → track.library_id == 9001 after bind.
44. **refresh_copies_personal_not_url** — refresh keeps tracking_url, updates title/total.
45. **refresh_missing_entry** → NotInList.
46. **delete_uses_library_id** (E-20) — find returns id 9001 → mutation body `listId` = 9001 (assert on request JSON).
47. **delete_not_found_is_ok** (E-21).
48. **set_status_completed_sets_progress_total** (framework parity, area:27 test but AniList data-side): COMPLETED with total 24 → progress 24.
49. **set_episode_completes_even_when_rewatching** (E-26) — documents the quirk.
50. **score_set_by_index_roundtrip_all_formats** — index → stored → request `scoreRaw` integer in 0..=100.
51. **request_body_shapes** — for each mutation, the variables object has exactly the keys listed in p1-data §4 (no extra, no missing).
52. **private_flag_sent** (E-55) — `private: true` in variables when chosen.

Integration (opt-in, `#[ignore]`, needs `ANILIST_TEST_TOKEN` env, disposable account):
53. login with a real token (manual only), Viewer returns id and scoreFormat.
54. search "Bocchi" returns ≥1 manga and ≥1 anime with non-empty titles.
55. add → update → delete round-trip on a disposable entry (remote state verified via find).
56. rate limiter: 90 rapid calls complete without HTTP 429 (verify the 85 setting empirically).

UI (manual checklist in p1-ui-parity; automated GTK tests not planned).

## 7. Fixtures for behaviour parity checks against the Kotlin

Optional but recommended: a small harness that runs the same JSON through the Kotlin DTO classes and the Rust DTOs and compares the resulting `*Track` values. Only needed if the team wants proof of parity; not required for the tasks.

## 8. Test data invariants

- `score` stored values in tests must be in 0..=100 or exactly -1.0 (search placeholder).
- `status` ints must be from the typed enums; tests must not use raw literals other than the constants.
- Dates in tests use explicit epoch values computed with a fixed TZ.
