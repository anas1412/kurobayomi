# area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin: edge cases and tests

Source root: `/home/blackbox/aniyomi-src`. Behaviour references point to `p1-behavior.md` (sections and defect ids D-nn). Each edge case has an id `E-nn`, the input, the expected Kotlin result, the expected Rust result, and the test that proves it.

**Upstream tests in scope:** none. The scope contains no `*Test.kt` file (checked: all 14 files in `areas.py files … 1`). Upstream server tests exist in the Kavita repository (`Kavita.Services.Tests/TachiyomiServiceTests.cs`, located by read-only search). They describe server behaviour and are used as a reference for E-63 only. They are not ported.

---

## 1. Kavita authentication (behaviour 2.2 to 2.4)

| Id | Input | Kotlin result | Rust result (expected) | Test |
|---|---|---|---|---|
| E-01 | No Kavita source installed or configured (all 3 slots empty) | `authentications` = 3 empty slots; every request sends `Bearer null` | `EnhancedError::Unauthorized` before the request, no network call | `kavita_auth::no_slots_fails_fast` |
| E-02 | Slot 1 authenticate returns 401, slot 2 returns 200 with token `t2` | `IOException` aborts `loadOAuth`; slot 2 never loaded; all Kavita requests fail (D-05) | Slot 1 empty (WARN), slot 2 = `t2`; request to slot 2's base uses `Bearer t2` | `kavita_auth::unauthorized_slot_does_not_block_others` |
| E-03 | Authenticate times out (`SocketTimeoutException`) | `null`, slot skipped | `Ok(None)`, slot skipped | `kavita_auth::timeout_skips_slot` |
| E-04 | Authenticate returns 500 | Wrapped `IOException("Error fetching JWT token")` as `java.io.IOException: …` (D-06), aborts loop | `ServerError`, slot skipped, loop continues | `kavita_auth::server_error_skips_slot` |
| E-05 | Authenticate returns 403 | falls to `return null`, slot skipped | same | `kavita_auth::other_status_skips_slot` |
| E-06 | Authenticate returns 200 with `"token": ""` | `isNullOrEmpty` → skip | empty token → slot skipped (Rust checks `jwt.is_empty()`) | `kavita_auth::empty_token_skips` |
| E-07 | `APIURL` = `http://h:5000` (no `/api`) | authenticate posts to `http://h:5000/Plugin/authenticate` → 404 → skipped | same URL → `Ok(None)` | `kavita_auth::url_without_api_is_skipped` |
| E-08 | Two concurrent requests on a fresh tracker | `loadOAuth` may run twice (no lock) | single-flight: one `authenticate` per slot | `kavita_auth::concurrent_first_use_loads_once` (wiremock counts 3 calls) |
| E-09 | API returns 401 after the token expired | error returned; no renewal (D-07) | `invalidate` the slot, re-authenticate once, retry the request once | `kavita_auth::expired_jwt_reauthenticates_once` |
| E-10 | Slot 2 source not installed, slot 3 configured | slot 2 `get` returns null → NPE/ClassCast → abort (D-05) | slot 2 `SourceNotInstalled` (WARN), slot 3 loaded | `kavita_auth::missing_source_slot_is_skipped` |
| E-11 | `APIKEY` = `a&b=c d` | sent unencoded in the query: `apiKey=a&b=c d&pluginName=…` (wrong) | encoded: `apiKey=a%26b%3Dc+d` | `kavita_auth::api_key_is_query_encoded` |
| E-12 | `Bearer` header for a URL whose `/api/` prefix differs from the stored `apiUrl` | `getToken` returns null → `Bearer null` | `Unauthorized` (fail fast) | `kavita_auth::unmatched_base_fails_fast` |
| E-13 | Authenticate returns 200 but body is not JSON | `IOException(e)` (D-06 wrapper) | `Decode` error, slot skipped | `kavita_auth::bad_json_skips_slot` |
| E-14 | Authenticate log line for an empty key | "Api key is empty: true" | same wording, key never logged | `kavita_auth::log_never_contains_key` |

## 2. Kavita series, volumes, latest chapter, mark (behaviour 2.5 to 2.8)

| Id | Input | Kotlin result | Rust result (expected) | Test |
|---|---|---|---|---|
| E-15 | Series JSON without `thumbnail_url` | `cover_url` = `""` (default) | `""` | `kavita_series::missing_thumbnail_is_empty` |
| E-16 | `thumbnail_url: null` in JSON | `"null"` string (D-01) | `""` | `kavita_series::null_thumbnail_is_empty` |
| E-17 | `pages = 0`, `pagesRead = 0` | COMPLETED (first branch) | UNREAD (D-24, documented change) | `kavita_series::empty_series_is_unread` |
| E-18 | `pages = 10`, `pagesRead = 10` | COMPLETED | COMPLETED | `kavita_series::fully_read_is_completed` |
| E-19 | `pages = 10`, `pagesRead = 12` (server anomaly) | READING (`else`) | READING (`==` check, kept) | `kavita_series::read_over_pages_is_reading` |
| E-20 | `pages = 10`, `pagesRead = 4` | READING | READING | `kavita_series::partial_is_reading` |
| E-21 | Volumes `[{chapters:[{number:"0"}]}]` | volumeNumber = 1 → total 1 | 1 | `kavita_total::single_zero_volume_counts_one` |
| E-22 | Volumes `[{chapters:[1,2,10.5]}]` | max 10.5 → `toLong` 10 → total 10 | 10 | `kavita_total::fractional_truncated` |
| E-23 | Volumes: two zero-volumes and one with max 5 | max(5, 2) = 5 | 5 | `kavita_total::chapter_max_beats_volume_count` |
| E-24 | Volumes: four zero-volumes and max 2 | max(2, 4) = 4 | 4 | `kavita_total::volume_count_beats_small_max` |
| E-25 | A volume with `chapters: []` | `maxOf` on empty → `NoSuchElementException`, whole tracker fails (D-23) | volume skipped, others counted | `kavita_total::empty_volume_skipped` |
| E-26 | A chapter `number: "abc"` | `NumberFormatException` → fails (D-23) | `Decode` error for the call (not skipped) | `kavita_total::non_numeric_number_is_error` |
| E-27 | A chapter `number: null` | `!!` → `NullPointerException` | parsed as `-1` (documented difference) | `kavita_total::null_number_is_minus_one` |
| E-28 | `latest-chapter` 200 with `"number": "12,5"` | 12.5 | 12.5 | `kavita_latest::comma_decimal` |
| E-29 | `latest-chapter` 204 | 0.0 | 0.0 (no body parse) | `kavita_latest::no_content_is_zero` (P-11) |
| E-30 | `latest-chapter` 200 with `"number": "-1"` | -1.0 stored as `last_chapter_read` (D-22) | 0.0 (clamped) | `kavita_latest::negative_is_clamped` |
| E-31 | `latest-chapter` 500 | 0.0 silently | 0.0 silently | `kavita_latest::server_error_is_zero` |
| E-32 | `mark-chapter-until-as-read` with `last_chapter_read = 12.0` | `chapterNumber=12.0` | `chapterNumber=12.0` | `kavita_mark::double_formatting` |
| E-33 | `last_chapter_read = 0.5` | `chapterNumber=0.5` (server treats `< 1.0` as volume, upstream) | same string | `kavita_mark::fractional_passed_verbatim` |
| E-34 | Mark succeeds, then refresh | returns `getTrackSearch` result, status server-driven | same | `kavita_mark::returns_server_state` |
| E-35 | `getIdFromUrl` with `…/api/series/12?x=1` | `"12?x=1".toInt()` → NumberFormatException (D-30) | id parsed from the path segment only → 12 | `kavita_url::query_string_ignored` |
| E-36 | URL with trailing slash `…/api/series/12/` | `substringAfterLast("/")` = `""` → exception | last non-empty segment = 12 | `kavita_url::trailing_slash` |
| E-37 | Reverse-proxy prefix `https://h/kavita/api/series/12` | `api = https://h/kavita/api` (split on `/api/`) | same | `kavita_url::prefix_preserved` |

## 3. Komga (behaviour 3)

| Id | Input | Kotlin result | Rust result (expected) | Test |
|---|---|---|---|---|
| E-38 | Readlist tracking URL `…/api/v1/readlists/5` | item = ReadListDto; progress V1 at `…/api/v1/readlists/5/read-progress/tachiyomi` (not rewritten) | same paths | `komga_api::readlist_paths` |
| E-39 | Series URL `…/api/v1/series/9` | progress V2 at `…/api/v2/series/9/read-progress/tachiyomi` | same | `komga_api::series_v2_rewrite` |
| E-40 | Stored URL `…/api/v2/series/9` (already v2) | progress parsed as V1 → fails (D-18) | parsed as V2 | `komga_api::stored_v2_url_parses` |
| E-41 | `booksCount=0, unread=0, read=0` | UNREAD (first branch) | UNREAD (D-20 kept) | `komga_status::empty_is_unread` |
| E-42 | `booksCount=10, unread=10, read=0` | UNREAD | UNREAD | `komga_status::all_unread` |
| E-43 | `booksCount=10, unread=4, read=6` | READING | READING | `komga_status::in_progress` |
| E-44 | `booksCount=10, unread=0, read=10` | COMPLETED (`booksCount == read`) | COMPLETED | `komga_status::all_read` |
| E-45 | `maxNumberSort = 12.9` | `total = 12` | 12 | `komga_total::truncates` |
| E-46 | Series update with `last_chapter_read = 12.0` | body `{"lastBookNumberSortRead":12.0}` `application/json` | identical bytes | `komga_api::v2_body_bytes` |
| E-47 | Readlist update with `last_chapter_read = 2.7` | body `{"lastBookRead":2}` | `2` (truncation) | `komga_api::readlist_body_truncates` |
| E-48 | Progress PUT returns 204 | success, then refresh | same | `komga_api::put_204_then_refresh` |
| E-49 | No cookie present (auth missing) | 401 → `awaitSuccess` throws `HttpException` | `Unauthorized` (after decision 10.1) | `komga_api::unauthenticated_is_unauthorized` |
| E-50 | Readlist progress endpoint 404 (readlist hidden by content restriction) | exception | `NotFound` error | `komga_api::readlist_404` |
| E-51 | `ReadProgressV2Dto.maxNumberSort` missing | parse error | parse error | `komga_dto::v2_requires_max` |

## 4. Suwayomi (behaviour 4)

| Id | Input | Kotlin result | Rust result (expected) | Test |
|---|---|---|---|---|
| E-52 | Numeric track URL `"123"`, server address empty | URL `/api/v1/manga/123/full` (relative) → OkHttp throws → `match` null (D-26) | `SourceNotConfigured` | `suwayomi_api::numeric_without_base_fails` |
| E-53 | Non-numeric track URL `https://s/api/v1/manga/123` | used verbatim, `/full` appended | same | `suwayomi_api::url_used_verbatim` |
| E-54 | `unreadCount = null`, `chapterCount = 10` | READING (no branch matches) | READING | `suwayomi_status::null_unread_is_reading` |
| E-55 | `unreadCount = 0`, `chapterCount = 0` | UNREAD (first branch, `0 == 0`) | UNREAD | `suwayomi_status::empty_manga_is_unread` |
| E-56 | `unreadCount = 0`, `chapterCount = 10` | COMPLETED | COMPLETED | `suwayomi_status::all_read` |
| E-57 | `unreadCount = 10`, `chapterCount = 10` | UNREAD | UNREAD | `suwayomi_status::all_unread` |
| E-58 | `lastChapterRead = null` | `0.0` | `0.0` | `suwayomi_search::no_last_read_is_zero` |
| E-59 | Update to `last_chapter_read = 5.0`, chapters `[{chapterNumber:5.0,index:7}]` | `PUT …/chapter/7` with `markPrevRead=true&read=true` | same | `suwayomi_api::uses_index_not_number` |
| E-60 | Update to `0.0`, no chapter numbered 0 | `NoSuchElementException` (D-25) | `ChapterNotOnServer("0.0")` | `suwayomi_api::missing_chapter_error` |
| E-61 | Two `chapterNumber == 5.0` entries | `first {}` → the first in list order | the first in list order (same) | `suwayomi_api::first_match_wins` |
| E-62 | Basic auth login `a`, password `b` | `Authorization: Basic YTpi` | same header | `suwayomi_api::basic_header` |
| E-63 | Settings changed after first call | stale values until restart (D-08) | next call sees new values | `suwayomi_api::settings_reread_per_call` |
| E-64 | `isTrackFrom` with a different `remoteUrl`, same source | true (D-09) | false | `suwayomi_match::track_from_compares_url` |
| E-65 | Server returns `MangaDataClass` without `initialized` | parse error (required) | parse error (required set kept) | `suwayomi_dto::required_fields` |
| E-66 | Search (`searchManga`) | `TODO` → `NotImplementedError` (D-21) | `SearchUnsupported` | `suwayomi_search::unsupported` |

## 5. Jellyfin (behaviour 5)

| Id | Input | Kotlin result | Rust result (expected) | Test |
|---|---|---|---|---|
| E-67 | Tracking URL without fragment | `NullPointerException` from `fragment!!` (D-28) | `MissingFragment` | `jellyfin_url::missing_fragment` |
| E-68 | Fragment `movie` and `last_episode_seen = 0.0` | POST played anyway (D-32) | no POST | `jellyfin_update::movie_zero_does_not_mark` |
| E-69 | Fragment `movie` and `last_episode_seen = 1.0` | POST played | POST played | `jellyfin_update::movie_one_marks` |
| E-70 | Season, played flags `[T,T,F,F]`, indexes `[1,2,3,4]` | `last_episode_seen = episodes[1].indexNumber = 2`, WATCHING | 2, WATCHING | `jellyfin_progress::continuous_prefix` |
| E-71 | Season, flags `[F,F]` | 0.0, UNSEEN | 0.0, UNSEEN | `jellyfin_progress::none_played` |
| E-72 | Season, flags `[T,T]` with indexes `[1,2]` | total 2, `last = 2.0`, COMPLETED | same | `jellyfin_progress::all_played` |
| E-73 | Season, `indexNumber` null at `firstUnwatched - 1` | NPE | `Decode` error | `jellyfin_progress::null_index_is_error` |
| E-74 | Season, list order `[3,1,2]` (unsorted server response) | position rule gives wrong value (D-14) | sorted by `SortBy=IndexNumber` in the request, test with a mock that ignores the sort (client sorts too) | `jellyfin_progress::sorts_by_index` |
| E-75 | Season, total with indexes `[1,2,5]` and last item index 2 | total = `episodes.last()` = 2 (D-14) | total = max index = 5 | `jellyfin_progress::total_is_max_index` |
| E-76 | Update target `3.0`, episodes 1..5, none played | POST only episode index 3 (one POST, others stay unplayed) | POST episodes 1, 2, 3 (D-11 proposal) | `jellyfin_update::marks_continuous_range` |
| E-77 | Same, `JELLYFIN_MARK_RANGE = false` | one POST (index 3) | one POST | `jellyfin_update::strict_parity_mode` |
| E-78 | Update target `3.5`, no episode with that index | no POST, no error (D-12) | no POST, INFO log, `Ok` | `jellyfin_update::no_matching_episode_is_ok` |
| E-79 | Target `3.0`, episode 2 already played | Kotlin posts index 3 only | range mode posts 1 and 3 only (skips played 2) | `jellyfin_update::skips_played` |
| E-80 | `DatePlayed` with local TZ `Asia/Riyadh`, time 14:03 local | `…T14:03:21.123Z` (local time, wrong) (D-10) | `…T11:03:21.123Z` (UTC) | `jellyfin_update::date_is_utc` |
| E-81 | Tracking URL with prefix `https://h/jf/Users/U/Items/S` | path rewritten from root: `/Users/U/PlayedItems/…` (drops `/jf`) (D-13) | `/jf/Users/U/PlayedItems/…` | `jellyfin_url::prefix_kept` |
| E-82 | `api_key` missing for `userId` | `IOException("Please log in through the extension")` | `SourceNotConfigured` | `jellyfin_auth::missing_key_error` |
| E-83 | Key configured but empty string | `""` cached, sent as `api_key=` (D-16) | `SourceNotConfigured`, not cached | `jellyfin_auth::empty_key_not_cached` |
| E-84 | Slot 3 uninstalled, user belongs to slot 4 | loop reaches slot 3 → NPE, aborts (D-17) | slot 3 skipped, slot 4 found | `jellyfin_auth::missing_slot_skipped` |
| E-85 | 32 concurrent requests for the same new user | unsynchronised map (D-15) | no cache, no race | `jellyfin_auth::concurrent_lookup` |
| E-86 | Movie with `IndexNumber` absent | fine | fine | `jellyfin_dto::movie_has_no_index` |
| E-87 | `JFItem` without `UserData` | parse error | parse error | `jellyfin_dto::userdata_required` |
| E-88 | Season with 150 episodes | Kotlin sends no `Limit` or `StartIndex`; whether the server returns all items was not verified | Proposal: page with `StartIndex` and `Limit=200` until a short page; test with 150 episodes | `jellyfin_progress::large_season_not_truncated` |
| E-89 | `POST …/PlayedItems/{id}` returns 500 | error propagates, track not updated | error → `update` fails → retried by queue (27) | `jellyfin_update::post_failure_propagates` |
| E-90 | Season `refresh` with 0 episodes | `episodes.last()` throws `NoSuchElementException` | `Decode("no episodes")`, track left unchanged | `jellyfin_progress::empty_season` |

## 6. Shared framework and cross-tracker (behaviour 1, D-02, D-03, D-21, D-29)

| Id | Input | Kotlin result | Rust result (expected) | Test |
|---|---|---|---|---|
| E-91 | Chapter read via `TrackChapter`, local score 8.0, remote score default -1.0 | score becomes -1.0 and dates reset (D-03) | score 8.0 and dates kept | `framework_refresh::keeps_personal_fields` |
| E-92 | `update` computes READING locally, server says COMPLETED | local status READING stays (D-02) | caller persists server's COMPLETED | `framework_update::status_comes_from_server` |
| E-93 | `match` gets a 500 from the server | `null` (swallowed, D-29) | `Err` returned; `bind_enhanced` logs and continues (27) | `framework_match::error_is_logged_not_swallowed` |
| E-94 | `bind_enhanced` for an entry whose source is accepted but not logged in | tracker skipped (`loggedInTrackers`) | same | `framework_bind::logged_out_skipped` |
| E-95 | `bind_enhanced` for a source not in the accepted list | tracker skipped by `accept` | same | `framework_bind::unaccepted_skipped` |
| E-96 | Source identity `kavita/all` versus accepted `komga/all` | false | false | `framework_accept::mismatch` |
| E-97 | Migration: old track `remoteUrl` = old URL, new entry URL = new | copy `remoteUrl` to new URL when new source accepted | same | `framework_migrate::copies_url` |
| E-98 | Migration to a non-accepted source | `null` | `None` | `framework_migrate::unaccepted_is_none` |
| E-99 | Settings group, no Kavita source installed | row hidden from the "available" list, listed under "not installed" text | same | `framework_settings::not_installed_listed` |
| E-100 | Settings group, 3 of 4 installed | 3 rows + "Available but source not installed: Jellyfin" | same | `framework_settings::partial_install` |
| E-101 | Logout of Kavita | `saveCredentials("", "")`; `isLoggedIn` false | flag cleared (Proposal, section 5.2 of data doc) | `framework_login::logout_clears` |
| E-102 | Login via `loginNoop` | `"user"` / `"pass"` saved | flag set; no secret stored (Proposal) | `framework_login::noop_sets_flag` |

## 7. Source identity (behaviour 2, 3, 4, 5; data doc section 4)

| Id | Key | Expected id | Test |
|---|---|---|---|
| E-103 | `kavita_1/all/1` | 7266875667263635239 | `identity::kavita_1` |
| E-104 | `kavita_2/all/1` | 3764816291200082561 | `identity::kavita_2` |
| E-105 | `kavita_3/all/1` | 778637366235559582 | `identity::kavita_3` |
| E-106 | `jellyfin/all/1` | 1100359934660540567 | `identity::jellyfin_1` |
| E-107 | `jellyfin (2)/all/1` | 5716273013801838310 | `identity::jellyfin_2` |
| E-108 | `jellyfin (3)/all/1` | 7066619062139039107 | `identity::jellyfin_3` |
| E-109 | `jellyfin (10)/all/1` | 3559930647551045450 | `identity::jellyfin_10` |
| E-110 | `tachidesk/en/1` | 3100117499901280806 | `identity::tachidesk` |
| E-111 | Result is never negative | `id >= 0` for 10,000 random names | `identity::non_negative` (proptest) |
| E-112 | Name `Kavita_1` (uppercase) | lowercased before hashing, same as `kavita_1` | `identity::lowercases_name` |

## 8. Known risks not covered by a port decision

| Id | Risk | Why it stays | Test or check |
|---|---|---|---|
| E-113 | Kavita `latest-chapter` can return `volumeNumber * 10000` for volume-only series (upstream `TachiyomiService.cs:98-104`), and the Kotlin does not decode it. | Parity. Decoding changes what `last_chapter_read` means for these series. Needs a product decision. | `kavita_latest::volume_encoding_passed_through` (asserts raw value) |
| E-114 | Komga readlist endpoints were not exercised against a live Komga instance. | Verified by reading the upstream controller only. | Manual check on a live Komga before T-33a-062 closes |
| E-115 | The Jellyfin tracking URL producer is in the external extension. Its exact shape (fragment spelling, trailing slash) is inferred. | Outside this repo. | Fixture from the extension (T-33a-013 requires one) |
| E-116 | Kavita JWT expiry behaviour (401 vs redirect) is not verified. | Needs a live server. | Manual check (open decision 10.2) |

---

## 9. Fixtures needed

Directory: `crates/kuro-track/tests/fixtures/` (JSON, UTF-8, one file per response).

| File | Content | Used by |
|---|---|---|
| `kavita/series_ok.json` | `SeriesDto` with `id, name, pages, pagesRead, format, libraryId` and `thumbnail_url` absent | E-15, E-18, E-20 |
| `kavita/series_thumbnail_null.json` | same with `"thumbnail_url": null` | E-16 |
| `kavita/series_empty.json` | `pages: 0, pagesRead: 0` | E-17 |
| `kavita/volumes_mixed.json` | volumes with chapters `[0]`, `[1,2,10.5]`, `[]` | E-21, E-22, E-25 |
| `kavita/volumes_bad_number.json` | chapter `number: "abc"` | E-26 |
| `kavita/volumes_null_number.json` | chapter `number: null` | E-27 |
| `kavita/authenticate_200.json` | `{"username":"u","token":"tok","apiKey":"k"}` | E-02, E-08 |
| `kavita/authenticate_200_empty_token.json` | `"token": ""` | E-06 |
| `komga/series_v2_progress.json` | `ReadProgressV2Dto` sample | E-39, E-41 to E-44 |
| `komga/series_v1_progress.json` | `ReadProgressDto` sample | E-38 |
| `komga/readlist.json` | `ReadListDto` sample | E-38 |
| `komga/series_full.json` | full `SeriesDto` with all required fields | E-39 |
| `suwayomi/manga_full.json` | full `MangaDataClass` with `lastChapterRead` object | E-53, E-58 |
| `suwayomi/manga_unread_null.json` | `unreadCount: null` | E-54 |
| `suwayomi/chapters.json` | chapters with `chapterNumber` 5.0 at index 7, plus a duplicate | E-59, E-61 |
| `jellyfin/item_movie.json` | `JFItem` with `UserData.Played` | E-68, E-69 |
| `jellyfin/episodes_mixed.json` | episodes with indexes 1..5 and played flags `[T,T,F,F,F]` | E-70, E-76 |
| `jellyfin/episodes_unsorted.json` | indexes `[3,1,2]` | E-74 |
| `jellyfin/episodes_150.json` | 150 episodes | E-88 |

Wire-level tests use `wiremock` (dev-dependency) with recorded request assertions: method, path, query (decoded), headers, and body bytes.

---

## 10. New tests to write (summary)

1. Unit: `kavita_status`, `komga_status`, `suwayomi_status`, `kotlin_double`, `jellyfin_name`, `generate_source_id` (E-103 to E-112).
2. Unit: `TrackingUrl::parse` (E-67, E-81).
3. Unit: `KavitaAuth` state machine with a fake `SourceDirectory` (E-01 to E-14).
4. Wire: Kavita series and volume walk with fixtures (E-15 to E-27).
5. Wire: Kavita mark and latest (E-28 to E-34).
6. Wire: Komga v1, v2 and readlist paths and bodies (E-38 to E-51).
7. Wire: Suwayomi paths, headers and error paths (E-52 to E-66).
8. Wire: Jellyfin episode query, marking and date format (E-68 to E-90).
9. Framework: refresh, update, match, bind, accept, migrate, settings and login (E-91 to E-102).
10. Property test: `generate_source_id` non-negative (E-111).
11. Concurrency: E-08 and E-85 run 32 tasks against a `tokio::sync::Barrier`.

Each test runs without a network: `wiremock` serves the fixtures and `SourceDirectory` is an in-memory map.

---

## 11. Manual checks before release

1. Kavita: connect to a real server, confirm `thumbnail_url` absence (E-15), confirm JWT expiry behaviour (E-116).
2. Komga: a v1 series and a readlist on a live server with and without the cookie (E-49, E-114).
3. Suwayomi: change the server address in settings and confirm the next call uses it (E-63).
4. Jellyfin: season and movie on a server with a reverse-proxy prefix (E-81), and a timezone other than UTC (E-80).
