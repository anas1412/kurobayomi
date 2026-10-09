# Area 26 - Library update jobs: edge cases, failure modes, tests

Upstream tests in scope: **none** (no `*Test.kt` file lives under the scope paths). Related upstream tests that define the fetch-interval contract and are ported below:

- `domain/src/test/java/tachiyomi/domain/entries/manga/interactor/MangaFetchIntervalTest.kt` (10 tests, `@Test` count verified)
- `domain/src/test/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchIntervalTest.kt` (10 tests, same scenarios with episodes)

Test runner in Rust: `cargo test` with built-in `#[test]` / `#[tokio::test]`. Fixtures: in-memory SQLite (`rusqlite::Connection::open_in_memory`) with the schema from `p1-data-and-config.md`, and a `FakeSource` implementing the `Source` trait. Test file locations are proposals: `src/core/library_update/fetch_interval.rs` (`#[cfg(test)] mod tests`), `tests/library_update_pass.rs` (integration).

---

## 1. Upstream fetch-interval tests, ported

All times are UTC (`ZoneOffset.UTC` in the tests). Base time `testTime = 2020-01-01T00:00:00Z`. "day N" below means `testTime + N days`. The Rust port must use the same zone in tests and must pass the same expectations.

| # | Test name (verbatim) | Input (chapters / episodes) | Expected interval | Port id |
|---|---|---|---|---|
| 1 | returns default interval of 7 days when not enough distinct days | 50 items all at day 1 (`1.days`); then same with `dateUpload = 0` | 7 (both) | FI-01 |
| 2 | returns interval based on more recent chapters | old: day 7,14,21,28,35; new: 10 items at (last old + 1..10 days) = day 36..45 | 1 | FI-02 |
| 3 | returns interval based on smaller subset of recent chapters if very few chapters | old: day 7,14,21; new: day 21+365+7, +14, +21 (i.e. 393, 400, 407) | 7 | FI-03 |
| 4 | returns interval of 7 days when multiple chapters in 1 day | 10 items at 10 hours | 7 | FI-04 |
| 5 | returns interval of 7 days when multiple chapters in 2 days | 2 items at day 1, 5 items at day 2 | 7 | FI-05 |
| 6 | returns interval of 1 day when chapters are released every 1 day | 20 items at day 1..20 | 1 | FI-06 |
| 7 | returns interval of 1 day when delta is less than 1 day | 20 items at 15h * i (i=1..20) | 1 | FI-07 |
| 8 | returns interval of 2 days when chapters are released every 2 days | 20 items at 2i days | 2 | FI-08 |
| 9 | returns interval with floored value when interval is decimal | 5 items at 25h * i (i=1..5), with and without `dateUpload` | 1 (both) | FI-09 |
| 10 | returns interval of 1 day when chapters are released just below every 2 days | 20 items at 43h * i | 1 | FI-10 |

Hand verification of the algorithm for the non-trivial cases (so the port author can trust the table):
- FI-02: 15 items > 8 so window = 10. Newest 10 distinct upload days are day 45 down to day 36 (10 distinct values). `delta = 45 - 36 = 9`, `period = size - 1 = 9`, `interval = 1`.
- FI-09: 5 items at 25h, 50h, 75h, 100h, 125h give start-of-day dates 1,2,3,4,5. Window 3 (5 <= 8), newest 3 distinct dates 5,4,3. `delta = 2`, `period = 2`, `2/2 = 1`. The name refers to `floorDiv` on a non-exact division.

Port requirement: both the manga and anime variants of the Rust function are the same function; test it once for `Item` slices and call it from both paths. The Kotlin duplicates the code per type; the port must not.

Additional Rust-only tests (new):

| Id | Scenario | Expected |
|---|---|---|
| FI-11 | `calculate_interval` with empty slice | 7 |
| FI-12 | items with `date_upload = 0` but valid `date_fetch` every day for 5 days | uses fetch dates: 1 (delta 4 / period 4) |
| FI-13 | `date_upload` valid for 2 distinct days, `date_fetch` valid for 3 days | falls to fetch branch: result from fetch |
| FI-14 | window `(0,0)` passed to `next_update` | equals `next_update` computed with `window_for(now)` |
| FI-15 | stored `fetch_interval = -3` | returned interval -3; `next_update` steps by 3 days; no recalculation |
| FI-16 | stored `fetch_interval = 0` | recalculated |
| FI-17 | `increase_interval` table from `p1-rust-design.md` 2.3 (5 rows) | as listed |
| FI-18 | stored `next_update` inside window (lower..=upper) | returned unchanged |
| FI-19 | stored `next_update` = tomorrow 00:00 exactly (`upper_exclusive`) | kept (inside inclusive range `[lower, upper_exclusive]`) |
| FI-20 | stored `next_update` = tomorrow 00:00 + 1 ms | recomputed (outside range) |
| FI-20b | stored `next_update` = yesterday 00:00 - 1 ms | recomputed (outside range) |
| FI-21 | `last_update = 0` | `latest` = start of today (uses `Instant.now()`); test with injected clock |
| FI-22 | DST day (Europe/Paris 2026-03-29 02:00 skip) | start-of-day of 2026-03-29 is 00:00 CET; `days_since` counts calendar days, not 24h multiples |
| FI-23 | `days_since` negative (last_update in the future) | `div_euclid` with positive divisor; result is a date >= today; document (Kotlin floorDiv identical for positive divisor) |

Window boundary (verified against `MangaFetchInterval.kt:95`): the Kotlin condition is `manga.nextUpdate in window.first.rangeTo(window.second + 1)`. `window.second` is tomorrow 00:00 ms minus 1, so `window.second + 1` is exactly tomorrow 00:00 ms. The kept range is `[yesterday 00:00 ms, tomorrow 00:00 ms]`, both ends inclusive. Use these exact values in the port.

---

## 2. Queue and restriction edge cases

| Id | Scenario | Expected behaviour (Kotlin-compatible) | Test |
|---|---|---|---|
| Q-01 | Library empty | queue empty; pass ends success; progress removed; `library_update_last_timestamp` updated | pass_empty_library |
| Q-02 | Manga in two categories, include contains both | appears once (distinctBy id) | queue_distinct |
| Q-03 | Manga in included cat A and excluded cat B | excluded (exclusion wins) | queue_exclude_wins |
| Q-04 | Manual per-category run for category 0 | only manga with `category == 0` (uncategorised, via the coalesce) | queue_category_zero |
| Q-05 | Manual per-category run ignores include/exclude prefs | yes | queue_manual_ignores_prefs |
| Q-06 | Restriction `manga_fully_read` and `unreadCount == 0` with `totalChapters == 0` | not skipped (`!= 0` rule) | restriction_unread_zero_total |
| Q-07 | `manga_started` and `totalChapters == 0` | not skipped (gated by `totalChapters > 0`) | restriction_started_zero_total |
| Q-08 | `updateStrategy == ONLY_FETCH_ONCE` | skipped regardless of restriction set | strategy_always |
| Q-09 | `status == 2` and `manga_ongoing` absent | not skipped (completed is allowed) | completed_allowed |
| Q-10 | `nextUpdate` = tomorrow 00:00 exactly with `manga_outside_release_period` | skipped: the check is `nextUpdate > window.second` and `window.second` is tomorrow 00:00 minus 1 ms | release_boundary |
| Q-11 | `nextUpdate` = tomorrow 00:00 minus 1 ms with `manga_outside_release_period` | kept | release_boundary_inside |
| Q-12 | Manual pass with `manga_outside_release_period` | still skipped (not exempt) | manual_release_applies |
| Q-13 | Anime: category contains a Seasons-type parent, `include_season_children = false` | parent produces no queue entry | seasons_parent_dropped |
| Q-14 | Anime: season child list contains a child with `favorite = true` | child excluded from expansion | season_child_fav_excluded |
| Q-15 | Anime: season child with `fetch_type = Seasons` | excluded from expansion | season_nested_ignored |
| Q-16 | Anime: `getAnimeSeasonsById` fails | empty list, error logged, pass continues | season_query_error_empty |
| Q-17 | Queue sort | by title ascending (byte order) | queue_sorted_by_title |
| Q-18 | Queue warning threshold | 60 entries from one source -> warning; 60 -> no warning; 61 with source `UnmeteredSource` -> no warning | warning_threshold |

Q-10 wording fix: Kotlin check is `it.manga.nextUpdate > fetchWindowUpperBound`, where `fetchWindowUpperBound = getWindow(now).second`, which is **tomorrow 00:00 minus 1 ms**. So `nextUpdate == tomorrow 00:00` (exactly) is greater than the bound and is skipped. Q-11 (`tomorrow 00:00 - 1 ms`) is kept. Port exactly this comparison.

---

## 3. Sync edge cases (chapters; episodes mirror)

| Id | Scenario | Expected | Test |
|---|---|---|---|
| S-01 | Source returns zero chapters, source not local | `NoChaptersException` -> failure "No chapters found" | sync_empty_nonlocal |
| S-02 | Source returns zero episodes, source local | no throw; nothing added | sync_empty_local |
| S-03 | Duplicate URLs in source list | first occurrence kept (`distinctBy url`) | sync_dedupe_url |
| S-04 | New chapter with `dateUpload == 0` and no earlier valid upload in this batch | `dateUpload = nowMillis` | sync_upload_now |
| S-05 | New chapters with `dateUpload == 0` after one with valid upload `T` | take `maxSeenUploadDate` (= max over earlier valid uploads) | sync_upload_max |
| S-06 | `dateFetch` of new items | `nowMillis + N ... nowMillis + 1` with first item = `nowMillis + N` (newest first) | sync_datefetch_order |
| S-07 | Removed chapter that was read; a new chapter with the same number arrives | new chapter gets `read = true`, `dateFetch` from removed entry, and is NOT returned as new | sync_removed_read_carry |
| S-08 | Removed chapter bookmarked; same number | new chapter `bookmark = true` | sync_removed_bookmark_carry |
| S-09 | `mark_duplicate_read_chapter_read` contains `new`; new chapter number already read in DB | new chapter `read = true`, excluded from returned list | sync_mark_dup_read |
| S-10 | Chapter in excluded scanlator | excluded from returned list (but still inserted) | sync_excluded_scanlator |
| S-11 | `shouldUpdateDbChapter` false for all existing | no update writes; if no add/remove/update, return empty and maybe update interval | sync_no_change |
| S-12 | No changes and `manual == false` and `fetchInterval != 0` and `nextUpdate >= window.first` | interval NOT recalculated; `lastUpdate` NOT touched | sync_interval_skip |
| S-13 | Changes present | interval recalculated; `lastUpdate = now` | sync_interval_always |
| S-14 | Chapter number unparseable (`-1`) | `isRecognizedNumber == false`; never counted for duplicates; name shown as "N new chapters" in notifications | sync_unrecognized |
| S-15 | Removed chapter with number NaN | not expected; `TreeSet<Double>` ordering in Kotlin; Rust uses `total_cmp` and rejects NaN at parse | sync_nan_guard |
| S-16 | Source order changes (reordering only) | `sourceOrder` updated via update path; no new items | sync_reorder |
| S-17 | Rename of downloaded chapter folder needed | `renameChapter` invoked only when the old folder was downloaded and dir name changed | sync_rename |
| S-18 | Anime: episode `fillermark` local true | stays true even if source says false | sync_fillermark_sticky |
| S-19 | Anime: `previewUrl` local non-blank | not overwritten | sync_preview_sticky |

---

## 4. Pass execution edge cases

| Id | Scenario | Expected | Test |
|---|---|---|---|
| P-01 | Two sources, 3 entries each | 2 groups in parallel; entries of a group sequential (check start/end ordering in event log) | pass_group_order |
| P-02 | 7 sources | at most 5 groups active at once (max concurrent counter in fake source = 5) | pass_concurrency_5 |
| P-03 | Entry removed from library during pass (before its turn) | skipped; no source call (fake source call count 0) | pass_removed_skip |
| P-04 | Entry removed during its network call | `updateManga` returns empty; no sync writes | pass_removed_midflight |
| P-05 | Source not installed (`get_or_stub`) | `SourceNotInstalled` -> failure "Source not found" | pass_source_missing |
| P-06 | Source fetch returns error "timeout" | failure message "timeout" recorded; other entries continue | pass_error_continue |
| P-07 | Error message empty | Rust `Other("")` not allowed: message becomes Debug text | pass_empty_message |
| P-08 | Cancel mid-pass | remaining entries not started; running entry finishes or is interrupted; cancelled entries are NOT in failures; lock released; row state `cancelled` | pass_cancel |
| P-09 | Manual start while auto running | `start_now` returns false; UI toast "An update is already running" | start_now_refused |
| P-10 | Auto scheduled tick while manual running | auto returns Retry, backoff 10 min | auto_retry_manual |
| P-11 | `new_items` ordering for notification tap | sorted desc by `source_order` (oldest first on source order list), per P-8 | pass_new_order |
| P-12 | Downloads enqueued with autostart false, then `start()` once at end | assert start called once and only if enqueued > 0 | pass_download_start_once |
| P-13 | Anime pass with new episodes and `download_new_episode = true` | after fix: enqueued (fails in Kotlin: P-1) | pass_anime_download_wired |
| P-14 | `auto_update_metadata = true` | details fetched before chapters for manga | pass_metadata_first |
| P-15 | Summary notification count | equals number of entries with new items (not number of items) | notif_summary_count |
| P-16 | Error log written when failures > 0 only | file absent when none | errorlog_absent |
| P-17 | Error log format | matches `p1-behavior.md` section 7 exactly | errorlog_format |
| P-18 | Counter increments | `new_manga_updates_count += new_items.len()` per entry | counter_increment |
| P-19 | `library_update_last_timestamp` set even for empty queue | set | timestamp_empty |
| P-20 | Crash mid-pass leaves `library_job_run` state running | startup sets `cancelled` | recovery_running_row |

---

## 5. Scheduler edge cases

| Id | Scenario | Expected | Test |
|---|---|---|---|
| SC-01 | interval 0 | no auto loop; existing loop cancelled | sched_off |
| SC-02 | interval 12 set at startup | loop armed; first pass after constraints satisfied | sched_arm |
| SC-03 | interval changed while running | loop replaced (UPDATE policy); running pass continues | sched_replace |
| SC-04 | `wifi_only` and metered/mobile link | constraint false; poll every 30 s; no pass | sched_constraint_wifi |
| SC-05 | `unmetered_only` and `NMMetered` = guess-yes | treated as metered; no pass | sched_metered_guess |
| SC-06 | `when_charging` and UPower OnBattery true | no pass | sched_charging |
| SC-07 | NetworkManager absent | constraints satisfied (fallback) | sched_no_nm |
| SC-08 | Retry backoff sequence | 10 min, 20 min, 30 min ... capped at 5 h; reset on success | sched_backoff |
| SC-09 | `stop(kind)` on auto pass | pass cancelled; loop re-armed | sched_stop_rearm |
| SC-10 | `stop(kind)` on manual pass | pass cancelled; no re-arm needed | sched_stop_manual |
| SC-11 | Metadata pass `stop` | cancelled; no re-arm | sched_stop_meta |
| SC-12 | Two kinds scheduled | independent loops and locks | sched_two_kinds |
| SC-13 | Clock jump forward by 2 days | next run after the wall-clock jump; no replay of missed runs | sched_clock_jump |
| SC-14 | App closed when scheduled time passes | runs at next start (only if background portal granted), no replay | sched_closed |

---

## 6. Notification text edge cases

| Id | Input | Expected output | Test |
|---|---|---|---|
| N-01 | 0 numbers, 1 item | "1 new chapter" | desc_generic_one |
| N-02 | 0 numbers, 5 items | "5 new chapters" | desc_generic_many |
| N-03 | numbers [2.5], 1 item | "Chapter 2.5" | desc_single |
| N-04 | numbers [2.5], 10 items total (9 unparseable) | "Chapter 2.5 and 9 more" | desc_single_more |
| N-05 | numbers [1,2,3] | "Chapters 1, 2, 3" | desc_multi |
| N-06 | numbers [1..=6] (6 distinct) | "Chapters 1, 2, 3, 4, 5 and 1 more" | desc_multi_truncate_one |
| N-07 | numbers [1..=7] | "Chapters 1, 2, 3, 4, 5 and 2 more" | desc_multi_truncate_other |
| N-08 | numbers [1, 1, 2] duplicates | displayable set {1, 2}: "Chapters 1, 2" (dedupe after formatting) | desc_dedupe |
| N-09 | numbers [2.0] vs [2.5] formatting | "2" and "2.5" (no trailing zero) | desc_format_number |
| N-10 | percent(0, 0) | "0%" (guard) | percent_zero_total |
| N-11 | percent(1, 3) | "33%" (DOWN) | percent_down |
| N-12 | percent(2, 3) | "66%" | percent_down2 |
| N-13 | chop of 40-char title in progress | first 40 characters with ellipsis when cut | chop_progress |
| N-14 | chop of 45 in summary | | chop_summary |
| N-15 | chop with multibyte (Japanese title, 50 chars) | cut on char boundary, not byte | chop_utf8 |
| N-16 | hideNotificationContent true | no per-entry notifications; summary generic; progress no list; error and warning still shown | notif_hidden |
| N-17 | summary with 1 entry and content hidden | "For 1 entry" | summary_hidden_one |

---

## 7. Failure modes and recovery

| Id | Failure | Kotlin behaviour | Rust behaviour |
|---|---|---|---|
| F-01 | DB write fails in sync | exception -> entry failure | same; pass continues |
| F-02 | DB unavailable at pass start | `getLibraryManga` throws; `doWork` catch -> failure | run_pass returns failure outcome; lock released |
| F-03 | Notification post fails (D-Bus gone) | `notify` throws? (Android never fails); | log warn, continue |
| F-04 | Error log write fails | `writeErrorFile` returns `File("")` | error notification without action; log warn |
| F-05 | Cover download fails in notification icon | null bitmap; no large icon | same |
| F-06 | Source throws `CancellationException`-like on stop | treated as failure in per-entry catch (P-10) | cancellation not a failure |
| F-07 | Device offline mid-pass | each entry fails with network error | each entry fails with `Other(msg)`; pass ends with error notification |
| F-08 | Source rate-limited (HTTP 429) | error recorded | error recorded; no retry within pass (parity) |
| F-09 | Process killed mid-pass | WorkManager reschedules? (WorkManager may retry depending on state) | row recovered as cancelled; next scheduled run proceeds |
| F-10 | Lock stuck (crash without recovery) | WorkManager handles | startup recovery marks `running` rows as `cancelled` |
| F-11 | Two instances of app | not applicable on Android | single-instance via GApplication `unique`; second instance forwards "start now" action |

---

## 8. Race conditions to test

| Id | Race | Mitigation | Test |
|---|---|---|---|
| R-01 | `startNow` twice quickly | lock row insert in one transaction | race_start_twice |
| R-02 | `stop` while queue building | cancellation token checked before first entry | race_stop_early |
| R-03 | Entry favourited/un-favourited during pass | re-check before sync (both kinds; P-7 fix) | race_unfav |
| R-04 | Category edited during pass | queue snapshot at start (Kotlin behaviour) | race_category_edit |
| R-05 | Manga sync and reader write same chapter (read flag) | `chapter.read` writes are per-row; sync uses `update` path that only writes changed fields (name, number, scanlator, sourceOrder, dateUpload) so read flag preserved | race_read_preserved |
| R-06 | Download completes during notification "Download" action | enqueue idempotent per chapter id | race_download_twice |
| R-07 | Two kinds write `library_update_last_timestamp` | single key; last-writer wins (same as Kotlin) | race_timestamp |
| R-08 | Semaphore starvation | per-group permit held for whole group; a long source blocks its own group only | race_starvation |

---

## 9. Fixtures needed

| Fixture | Content |
|---|---|
| `fixtures/library_small.json` | 12 manga across 3 categories, 2 excluded, 1 completed, 1 ONLY_FETCH_ONCE, 1 with 0 chapters, 1 in two categories |
| `fixtures/library_anime_seasons.json` | 3 anime: one Episodes type, one Seasons type with 4 children (2 favourite, 1 nested Seasons, 1 normal), one Seasons type with no children |
| `fixtures/chapters_newest_first.json` | 30 chapters with dates spanning 60 days, with 2 duplicate numbers, 1 unparseable, 1 with dateUpload 0 |
| `fixtures/episodes_seasons.json` | episode lists for season children |
| `fixtures/source_responses/*.json` | recorded `SChapter` lists per source id (ids: 1001 manga, 2001 anime) |
| `fixtures/notif_expected.txt` | expected description strings for N-01..N-09 |
| `fixtures/errorlog_expected.txt` | expected error-log output for 2 messages, 3 sources |
| `fixtures/datasaver_urls.txt` | golden proxy URLs for 6 inputs per proxy (jpg, jpeg with ignore, gif with ignore false, webp, png, URL with space and `&`) |

Golden data-saver expectations (derived from the Kotlin templates):
- Bandwidth Hero, server `https://bh.example`, quality 80, format false, bw false, input `https://img.example/a b.jpg`: `https://bh.example/?jpg=0&l=80&bw=0&url=https%3A%2F%2Fimg.example%2Fa+b.jpg` (Java `URLEncoder` encodes space as `+`).
- wsrv.nl, quality 80, format false, input `https://img.example/a.webp`: `https://wsrv.nl/?url=https://img.example/a.webp&q=80&n=-1` (Kotlin raw; Rust fixed version: `https://wsrv.nl/?url=https%3A%2F%2Fimg.example%2Fa.webp&q=80&n=-1`).
- wsrv.nl, format true, input `https://img.example/a.png`: `https://wsrv.nl/?url=https://img.example/a.png&output=jpg&q=80`.

---

## 10. Performance tests

| Id | Scenario | Target |
|---|---|---|
| PERF-01 | 2,000 library entries, 40 sources | queue build < 50 ms; pass overhead excluding network < 2 s on fake source with 0 ms latency |
| PERF-02 | 500 chapters per entry sync | sync diff < 100 ms (HashMap by url instead of Kotlin's O(n^2) `any`) |
| PERF-03 | notification description for 10,000 numbers | < 5 ms |

Note: Kotlin `removedChapters = dbChapters.filterNot { sourceChapters.any { ... } }` is O(n*m). The Rust port uses `HashSet<&str>` of source urls. This is an intentional improvement; behaviour identical.

---

## 11. Manual acceptance checklist (for the user)

1. Enable "Automatic updates: Daily" with Wi-Fi restriction; confirm the first pass runs when on Wi-Fi.
2. Trigger "Update library" from the Updates tab with the library having a source with 61 entries: warning notification appears once; progress shows "Updating library… (N%)".
3. Force a source error: error notification shows "1 update(s) failed"; opening it shows the log file in the default text viewer.
4. With "Download new episodes" enabled, a new episode appears after a pass and is queued (after P-1 fix).
5. With "Hide notification content" enabled, no entry titles appear in notifications.
6. Cancel from the progress notification: pass stops; no failure notification.
7. Start an anime pass and a manga pass at the same time: both run (independent locks); starting a second manga pass shows "An update is already running".
