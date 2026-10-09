# Area 27-tracking-core, part 1: edge cases, failure modes and tests

Upstream test coverage: none. A search of `app/`, `domain/`, `data/` and `core/` for `*Test.kt` or `*Spec.kt` that mention track finds nothing for the part 1 scope. Every test below is new. Fixtures and fakes are listed in section 8.

Contents: 1 edge cases by function; 2 race conditions; 3 failure modes and recovery; 4 defect regression tests (D1 to D20); 5 rule tests (the setter and progress rules); 6 queue and retry tests; 7 UI tests; 8 fixtures and fakes; 9 properties; 10 migration and backup tests; 11 manual acceptance checklist.

Test IDs: `TT-27-nnn`. Each test names the behaviour section it checks (`B` plus section number, for example B3.3 means `p1-behavior.md` section 3.3).

---

## 1. Edge cases by function

### 1.1 Registry (`TrackerManager`)

| Case | Expected (Kurobayomi) | Source reference |
|---|---|---|
| `get(id)` for an unknown id (for example 999) | `None`, no panic | `TrackerManager.kt:52` returns null |
| `get(id)` for a manga-only tracker asked for anime | `as_anime()` is `None`; the anime sheet never lists it | B1.4 |
| Two trackers with the same id | impossible: the constructor asserts unique ids in debug builds | source has no check |
| `logged_in()` when no tracker has credentials | empty vector | `:44` |
| Registry order changes | the order of `logged_in()` follows `trackers` order, not insertion order from storage | `:39-42` |

### 1.2 Credentials and login state

| Case | Expected | Source reference |
|---|---|---|
| username set, password empty | `is_logged_in == false` | `BaseTracker.kt:32-34` |
| both set to whitespace " " | `is_logged_in == true` (only empty is checked; the source does not trim) | same |
| `logout()` twice | second call is a no-op write of empty strings, no error | `BaseTracker.kt:27-30` |
| `save_credentials` then crash before the second write | on next start `is_logged_in == false` (half-written pair is not a login) | B17.5 (Proposal fix) |
| Secret Service unavailable | fallback file with mode 0600 is written; a warning is logged once | `p1-rust-design.md` section 5.1 |
| Secret Service locked | `get` returns `SecretError::Locked`; `is_logged_in` returns `false` and the UI asks to unlock | Proposal |
| Auth-expired set, then `save_credentials` | flag cleared to false | `TrackPreferences.kt:29-33` |
| MyAnimeList token refused | auth-expired true, login row shows a warning (Proposal) | `MyAnimeList.kt:269-273` |
| `login` fails with an empty message | toast shows "null" in source (D14); Rust shows "Login failed" | `SettingsTrackingScreen.kt:361` |

### 1.3 Remote setters (behaviour section 3)

| Case | Input (status, last, total) | Expected output | Rule |
|---|---|---|---|
| Set episode 1 on a fresh entry | (planned, 0, 12) | status = watching, last = 1 | B3.3 rule 1 |
| Set episode 0 on a fresh entry | (planned, 0, 12) | status unchanged, last = 0 | B3.3 (watching needs a positive number) |
| Set episode 12 with total 12 | (watching, 11, 12) | status = completed, finish = now, last = 12 | B3.3 rule 3 |
| Set episode 11.9 with total 12 | (watching, 11, 12) | last = 11, not completed | `toLong()` truncation |
| Set episode 12 with total 0 (unknown) | (watching, 11, 0) | status unchanged, last = 12 | `total != 0` guard |
| Set episode 5 when status is rewatching | (rewatching, 0, 12) | status stays rewatching | rule 1 guard |
| Set episode 5 when already at 9 (lower) | (completed, 9, 12) | status stays completed, last = 5 | rule 1 needs last == 0 |
| Completed status with total 12 | status = completed | last = 12 | B3.2 |
| Completed status with total 0 | status = completed | last unchanged | B3.2 guard |
| Score string not in list | "x" | score = -1.0 | B3.4 (Rust: `UnknownScore` error, proposal) |
| Score string at index 0 | "0" | score = 0.0 | `indexToScore(0)` |
| Score list empty | any | score = -1.0 | `indexOf` on empty list |
| Start date set to 0 | 0 | start date cleared | B3.5 |
| Finish date set in the past | epoch 1 | stored as given | no validation in source |
| Private true then false | | `private` follows | B3.5 |
| `update` throws | | remote error propagated; local row not written | B3.6 step 1 |
| `update` succeeds, DB insert fails | | error logged (insert swallows), setter returns normally | `InsertAnimeTrack.kt:12-18` |

### 1.4 Dates

| Case | Expected | Source |
|---|---|---|
| Epoch 0 start date | displayed as unset | `AnimeTrackInfoDialogHome.kt:109` |
| Start date 2026-03-01 23:30 local, UTC+2 | converted via `convertEpochMillisZone` keeps wall clock, stored as UTC wall time | B6.1 date conversion |
| Negative epoch (before 1970) | `toLocalDate` works for negative millis | `DateExtensions.kt:39-41` |
| Date picker: future day | not selectable | B14.6 |
| Start picker: day after finish | not selectable | B14.6 |
| Start picker with finish 0 (unset) | every past day selectable | B14.6 |
| Picker day in a zone west of UTC | the chosen day is preserved after conversion | Proposal in B14.6 |
| Year-level check with start in same year | years allowed, days checked by day rule | B14.6 |

### 1.5 Binding (behaviour section 6)

| Case | Expected |
|---|---|
| Entry has no episodes | `has_seen = false`, no progress push, no start date, bind still inserts |
| Entry has seen episodes, latest seen 3.5 | remote gets `3` (truncation), local track keeps 3.5 (B6.1 step 6b) |
| Latest seen episode number is -1 (unrecognised) | `latest = -1.0` is not greater than a non-negative last, nothing pushed |
| Track already has `startDate > 0` | start date not overwritten |
| History empty | no start date |
| History first row has `seenAt = null` | no start date (defect D20; Rust: sort with `None` last, then take first non-null) |
| `bind` throws | error returned to the caller; no insert |
| `bindEnhancedTrackers` with two enhanced trackers, first `match` throws | first error logged as WARN, second still processed |
| `bindEnhancedTrackers` for a source not accepted | tracker skipped (no `match` call) |
| `bindEnhancedTrackers` called twice | second call upserts the same binding (Proposal: skip when bound) |

### 1.6 Refresh (behaviour section 7)

| Case | Expected |
|---|---|
| No bindings | empty failure list |
| One binding logged out | skipped silently, no failure entry |
| Two bindings, one fails | failure list has one entry; the other updates |
| Cancellation mid-refresh (screen closed) | Proposal: refresh runs to completion; the test checks that a write started before close is finished |
| Refresh returns a track with a different `remote_id` | written as returned (no validation in source) |

### 1.7 Progress triggers (behaviour section 9)

| Case | Expected |
|---|---|
| Episode 5 when tracker is at 5 | skipped, nothing queued (`<=` rule) |
| Episode 6 when tracker is at 5, online, refresh works | refresh, update with last = 6, insert, queue entry removed |
| Episode 6, offline (anime) | queued with progress 6, retry scheduled |
| Episode 6, offline (manga chapter) | queued with progress 6, retry scheduled (manga does not check connectivity; refresh fails, queue on failure) |
| Episode 6, online, `update` returns HTTP 500 | queued (Proposal, manga rule); the source's anime path does not queue this case (behaviour 9.1) |
| Two trackers, one logged out | only the logged-in tracker is updated |
| Episode number 6.0 from Float 6.0f | exact |
| Episode number 12.1 from Float | expected `12.1` (f64 from decimal text); source gives `12.100000381469727` (D7). Test asserts the Rust value equals `"12.1".parse::<f64>()` |
| `track_episode` for an entry with no bindings | no-op, no queue entry |
| Mark read with ALWAYS and tracker at 4, marked 5 | `track_chapter(5)` runs and toast "Trackers updated to chapter 5" |
| Mark read with ASK, user dismisses the banner | nothing sent |
| Mark read with NEVER | no refresh, no prompt, no tracker update |
| Mark read with the selection max 3 while tracker at 5 | no prompt (`shouldPrompt` false) |

### 1.8 Queue (behaviour section 10)

| Case | Expected |
|---|---|
| Add 5 then add 3 for the same key | stored value 5 (monotonic) |
| Add 3 then add 5 | stored value 5 |
| Add for anime track 42 and manga track 42 | two separate entries (defect D3 fixed) |
| `list(Anime)` | only anime entries |
| Remove an entry that does not exist | no error |
| Job runs with an item whose binding was deleted | item removed, not retried |
| Job runs with an item whose binding is already at 12 and queued value 12 | item removed (Proposal, defect D5) |
| Four failed runs | items kept; the fifth start does nothing and keeps items |
| Enqueue wakes the sleeping scheduler | scheduler runs within 1 s |
| Network down at start | no run until `wait_online` resolves |

### 1.9 External player (ExternalIntents) path

| Case | Expected |
|---|---|
| Episode 6 seen externally, binding at 5, offline | queue entry keyed by the binding's `(entry_id, tracker_id)`, not the entry id (defect D2 fixed) |
| `autoUpdateTrack` false | nothing happens |
| Binding at 7, episode 6 seen | nothing (`6 > 7` false) |

### 1.10 Settings

| Case | Expected |
|---|---|
| Toggle "Update progress after reading/watching" off, then on | pref persisted, default on at first run |
| Choose "Never" in mark-read list | persisted as `NEVER`; reading unknown stored text returns `ALWAYS` |
| Enhanced tracker source installed later | row appears after the next settings open (source recomputes on composition; Proposal: refresh on source install event) |
| No enhanced tracker, no missing text | info text is only the base sentence |
| Two enhanced trackers missing sources | text joins names with ", " |

### 1.11 Search

| Case | Expected |
|---|---|
| Query "" | Proposal: not sent; results unchanged (defect D11) |
| Query with only spaces | Proposal: trimmed then treated as empty |
| Result list of 0 | "No results found" |
| Tracker throws with a message | message shown; with empty message, "Unknown error" |
| Result with `tracking_url` never set | Rust: `Option` absent; the card's open action disabled (defect D9) |
| Two results with same `remote_id` but different tracker | distinct (keys include tracker) |
| currentUrl matches a result | that result selected on load |
| currentUrl null | no auto-selection. Source `find { it.tracking_url == null }` never matches a non-null String. Rust: no auto-select when `current_url` is `None` |
| Register with `private = true` | `private` set on the result before register |

---

## 2. Race conditions

| ID | Scenario | Risk | Mitigation (Kurobayomi) |
|---|---|---|---|
| R1 | Two `track_episode` calls for the same entry close together (player and external player) | both read the same binding and both update; the last write wins with an older `lastEpisodeSeen` | Serialise per `(entry_id, tracker_id)` with a `tokio::sync::Mutex` keyed map; the update uses `max(current, new)` |
| R2 | Queue `add` concurrent with scheduler `run` for the same key | the scheduler removes an entry while a higher value is being added; the higher value is lost | Scheduler removes with a conditional delete: `DELETE ... WHERE progress <= ?` using the value it processed |
| R3 | Refresh writes a binding while a progress update is in flight | the refresh's older remote copy overwrites the newer progress | Refresh and update share the mutex from R1 |
| R4 | Remote `update` succeeds, local insert fails | remote ahead of local; next refresh fixes it | Accept; retry the local insert once. Test TT-27-120 |
| R5 | Settings toggles auto-update while a progress update runs | update already started completes | Acceptable; read the flag once at the start of each action |
| R6 | Login completes in a second window while the settings dialog is open | dialog shows stale "Log in" | `watch` channel drives the row; the row updates |
| R7 | Logout during a running refresh | refresh writes a logged-in row after logout | Check `is_logged_in` before each write in refresh; skip writes when false |
| R8 | Two OAuth redirects for the same tracker | second overwrites the first token | Reject a redirect whose `state` does not match the pending request (`p1-topic-tracker-oauth.md`) |
| R9 | ASK banner shown, user marks more chapters read meanwhile | banner refers to old max | The action uses the max stored with the banner, not a recomputed value |
| R10 | Two entries bind the same tracker entry (same remote_id) | two local rows point to one remote media | Allowed by the source; Proposal: warn in search when `remote_id` is already bound to another entry |

---

## 3. Failure modes and recovery

| Failure | Observable | Recovery |
|---|---|---|
| Network timeout during `refresh` | per-tracker toast "<tracker> error: timeout" | retry on next open; no queue entry |
| HTTP 401 on `update` | error toast on remote update; progress queued (manga) or dropped (anime, online) | Proposal: mark auth-expired and ask to log in again |
| HTTP 429 | error toast | the scheduler backs off 5 minutes (same schedule as network errors) |
| Corrupt token JSON in `track_token_N` | `serde_json` error on decode | treat as logged out, log error, keep the row |
| SQLite busy | write fails with `SQLITE_BUSY` after busy timeout | `TrackError::Db`; the queue keeps the entry |
| Disk full during queue write | `add` fails | the progress is lost; log at error. Proposal: fall back to an in-memory retry for the session |
| Process killed between remote update and local insert | remote ahead | fixed by the next refresh (R4) |
| Process killed after queue `add` but before the job runs | entry stays queued | the scheduler reads the queue at startup and runs it |
| Tracker removed from the build | binding exists for unknown id | `get` returns `None`; binding ignored in UI; queue entries for it are removed by the scheduler |
| Secret Service unlocked late | login row shows logged out at startup | state updates when the secret store is readable (poll once after 5 s, then on the next settings open) |

---

## 4. Defect regression tests

Each defect in `p1-behavior.md` section 22 has a test that fails on the Kotlin behaviour and passes on Kurobayomi.

| Test | Defect | Setup | Assertion |
|---|---|---|---|
| TT-27-D01 | D1 `_id` churn | insert a binding, record id; upsert it again | id unchanged (`upsert` keeps `_id`) |
| TT-27-D02 | D2 external queue key | fake external player with binding id 7, entry id 3, offline | queue entry has `entry_id = 3`, `tracker_id = binding.tracker_id`, and the scheduler resolves it |
| TT-27-D03 | D3 shared key | queue anime (entry 42, tracker 1, progress 3) and manga (entry 42, tracker 1, progress 9) | both exist; `list(Anime)` has progress 3 |
| TT-27-D04 | D4 entries kept at limit | four failed runs, then a fifth start | items still queued; no panic |
| TT-27-D05 | D5 stale entry | binding at 12, queued 12 | item removed after the run (Proposal) |
| TT-27-D06 | D6 race | 100 concurrent `add` of random values | final value equals the max |
| TT-27-D07 | D7 float | queue `progress = 12.1` | read back `12.1` exactly |
| TT-27-D08 | D8 score index 0 | list `["0","1"]`, current `"0"` | picker starts at index 0 |
| TT-27-D09 | D9 lateinit | search result with no URL | no panic; open action disabled |
| TT-27-D10 | D10 hashing | two results with same `remote_id`, different tracker | not equal, both shown |
| TT-27-D11 | D11 blank query | empty search submit | no tracker call |
| TT-27-D12 | D12 dead return | `to_domain(id_required = false)` with `id = None` | returns the track with `id: None`; no early return path exists |
| TT-27-D13 | D13 auto-open | pref on, no binding, no enhanced match | sheet not opened |
| TT-27-D14 | D14 "null" text | login error with `None` message | toast text is "Login failed", not "null" |
| TT-27-D15 | D15 menu closes | "Open in browser" in result menu | menu closes after the action |
| TT-27-D16 | D16 remove order | remote delete fails | local binding kept |
| TT-27-D17 | D17 unknown host | redirect with host `foo` | activity returns to settings with an error |
| TT-27-D18 | D18 AniList regex | fragment `access_token=abc` (no trailing `&`) | token parsed as `abc` |
| TT-27-D19 | D19 stale copy | enhanced tracker `set_last_episode_seen` then sync | final row has the status from the setter |
| TT-27-D20 | D20 null seenAt | history rows `[null, 2026-01-01]` | start date is 2026-01-01 |

---

## 5. Rule tests (setter and progress rules)

Property-style table tests. Use the exact table from `p1-behavior.md` section 3.

| ID | Rule | Cases |
|---|---|---|
| TT-27-100 | B3.2 completion | completed + total 12 → last 12; completed + total 0 → last unchanged; not completed → unchanged |
| TT-27-101 | B3.3 watching promotion | last 0 + episode 1 + planned → watching; last 0 + episode 0 → no change; last 0 + episode 1 + rewatching → rewatching |
| TT-27-102 | B3.3 completion | last 11, episode 12, total 12 → completed, finish set to `now`; last 11, episode 11, total 12 → not completed |
| TT-27-103 | B3.3 truncation | episode passed as `i32`; decimal progress preserved only in the entry, not in the setter |
| TT-27-104 | B3.4 score mapping | list `[a,b,c]`: `"a"`→0.0, `"c"`→2.0, `"x"`→-1.0 |
| TT-27-105 | B3.5 dates | set start 5 → 5; set start 0 → 0; set finish now → now |
| TT-27-106 | B3.6 order | fake tracker records call order: `update` then `insert` |
| TT-27-107 | B8.1 sync no-op | non-enhanced tracker → no episode updates, no `update` call |
| TT-27-108 | B8.1 enhanced continuous rule | episodes 1..5 with 1,2,4 seen → local last continuous = 2; remote 3 → max(3, 2) = 3; episodes ≤ 3 unseen become seen |
| TT-27-109 | B8.1 non-decreasing | remote 8, local continuous 5 → 8 |
| TT-27-110 | B6.1 remote start date from history | history earliest 2026-02-01 local → stored UTC-wall value; check conversion vector |
| TT-27-111 | B9.1 offline anime | `is_online=false` → queue, no network call |
| TT-27-112 | B9.2 manga failure queues | `refresh` throws → queued, error logged, no panic |
| TT-27-113 | B11.2 ASK | `ASK`, marks 5 with tracker at 4 → banner payload max 5; OK → `track_chapter(5)`; dismiss → none |
| TT-27-114 | B11.3 counts | two bindings, one enhanced not accepting the source → supported count 1 |
| TT-27-115 | B11.4 auto-open | pref on, count 0 → false; count 1 → true; pref off → false |

---

## 6. Queue and retry tests

| ID | Test | Expectation |
|---|---|---|
| TT-27-120 | Local insert fails after remote update succeeds | retry once; no duplicate remote update |
| TT-27-121 | Scheduler backoff sequence | attempts 0..3 wait 0, 5 min, 10 min, 20 min (`INITIAL_BACKOFF * 2^(n-1)`), the fifth start gives up |
| TT-27-122 | Backoff cap | attempt 10 wait equals `MAX_BACKOFF` (5 h) |
| TT-27-123 | Reset on kick | after two failures, `kick` resets attempts to 0 |
| TT-27-124 | Network constraint | `is_online=false` for 1 h simulated; no run; then true → run |
| TT-27-125 | Persistence | queue survives restart (SQLite file reopened) |
| TT-27-126 | Duplicate kick | ten kicks in 100 ms → one run |
| TT-27-127 | Concurrent scheduler runs | only one loop active (singleton) |

Time is simulated with `tokio::time::pause()` in tests; the wall clock is not used.

---

## 7. UI tests

GTK tests are run with a headless display (`GDK_BACKEND=broadway` or `xvfb-run`). Keep them few and focused on logic that is in `src/track/ui/*`.

| ID | Test | Expectation |
|---|---|---|
| TT-27-200 | Info sheet with one bound and one unbound logged-in tracker | two cards; second shows "Add tracking" |
| TT-27-201 | Detail cell placeholder | null status renders placeholder at alpha 0.5 |
| TT-27-202 | Progress text | total 24, last 12 → "12 / 24"; total 0 → "12" |
| TT-27-203 | Score cell hidden | tracker with empty score list → no score cell |
| TT-27-204 | Date row hidden | tracker without reading-date support → no date row |
| TT-27-205 | Private badge | private true → badge with eye-off icon |
| TT-27-206 | Overflow menu order | open, copy, toggle, remove |
| TT-27-207 | Toggle label | private → "Track publicly"; public → "Track privately" |
| TT-27-208 | Search states | loading → spinner; empty → text; error → message; results → list |
| TT-27-209 | Selection | one card selected; bottom bar visible; second click changes selection |
| TT-27-210 | Private button hidden | tracker without private support → no icon button |
| TT-27-211 | Number picker bounds | total 24 → 0..=24; total 0 → 0..=10000 |
| TT-27-212 | Score picker start | index 0 selected when current is "0" (D8 fixed) |
| TT-27-213 | Date picker day rules | future disabled; start after finish disabled |
| TT-27-214 | Settings rows | four rows with defaults true/true/true/Always |
| TT-27-215 | Settings logged-in row | check shown; click → logout dialog |
| TT-27-216 | Login dialog OK disabled | blank password → OK insensitive |
| TT-27-217 | Logout default focus | Cancel focused |
| TT-27-218 | Copy toast | copy link → toast text starts with "Copied to clipboard:" |

---

## 8. Fixtures and fakes (`kuro-testkit`)

| Fixture | Contents |
|---|---|
| `FakeTracker` | implements `AnimeTracker` and `MangaTracker`; scripted responses per call; records call order; configurable score list, status list, completion status (`valCompletionStatus = 2`, `valScoreList = "0".."10"`, `valStatuses = 1..6`, mirrors `DummyTracker` in `app/src/main/java/eu/kanade/test/DummyTracker.kt:14-51`) |
| `FakeTrackRepo` | in-memory `AnimeTrackRepo` / `MangaTrackRepo` with the upsert rule; `_id` assignment by counter; watch channels |
| `FakeEpisodes`, `FakeChapters` | episodes with numbers, `seen`, `Float` numbers (negative allowed) |
| `FakeHistory` | rows with optional `seenAt` / `readAt` (nullable, to test D20) |
| `FakeConnectivity` | `is_online` flag and `wait_online` channel |
| `MemorySecrets` | `SecretStore` in memory, with a `locked` switch |
| `FixedClock` | `now_millis` set by the test |
| `MemoryPrefs` | `kuro-prefs` in-memory store (area 02 testkit) |
| `Sample tracks` | one anime track (id 1, entry 2, tracker 1, remote 4, total 12, last 2, status 1, score 2.0, url `https://example.com`), one manga counterpart, one with private true, one with total 0 |
| `Sample search results` | three results with distinct remote ids and tracker ids; one with no `tracking_url` (exercises D9) |
| `Sample logos` | none (logo bytes not needed for logic tests) |

The `Sample tracks` values are the same as the Kotlin preview data (`AnimeTrackInfoDialogHomePreviewProvider.kt:14-29`), so visual snapshots line up with the upstream previews.

---

## 9. Properties

Use `proptest` (dev-dependency) for these:

| ID | Property |
|---|---|
| TT-27-300 | For any sequence of `set_last_episode_seen` calls, `status == completion` implies `last == total` (when total > 0) |
| TT-27-301 | `set_score` with a string from the list never yields -1.0 |
| TT-27-302 | The queue value for a key never decreases over any sequence of adds |
| TT-27-303 | `copy_personal_from_anime` with `copy_remote_private=false` never changes `private` |
| TT-27-304 | `to_db` then `from_db` is identity for any track with `id = Some(_)` |
| TT-27-305 | `convert_epoch_millis_zone(from, to)` followed by the reverse conversion returns the input for every instant outside a DST gap (the source's `convertEpochMillisZone` has this round-trip property except in gaps) |

---

## 10. Migration and backup tests

| ID | Test | Expectation |
|---|---|---|
| TT-27-400 | Import a backup from Aniyomi with tracks (area 21 fixture) | `anime_sync` rows created with the same `sync_id`, progress, status, dates |
| TT-27-401 | Import a backup with a private track | private flag kept |
| TT-27-402 | Import a backup that carries a password for a tracker | the password is not imported (private keys are excluded from backups, B17.5); the user must log in again |
| TT-27-403 | Preference export of `pref_auto_update_manga_on_mark_read = "ASK"` | imported as `Ask` |
| TT-27-404 | Preference export with unknown value `"SOMETHING"` | imported as `Always` (default) |
| TT-27-405 | Queue entries from the Android device | not imported (the queue is not in backup); documented loss |

---

## 11. Manual acceptance checklist

Run on a real Plasma session with at least one live tracker account (user's own).

1. Log in to AniList in the browser. The redirect returns and the row shows the check within 5 seconds.
2. Log out from AniList. The dialog appears; Cancel leaves it logged in; Log out clears the row.
3. Open an entry, add tracking from search. Results appear; the selected card is highlighted; "Track" adds the binding; the sheet shows the card.
4. Set status, progress (wheel up to total), score, start and finish dates. Each change appears in the tracker's website within 10 seconds.
5. Mark a chapter read with the policy set to ASK. The banner appears; OK updates the tracker.
6. Disconnect Wi-Fi, read a chapter, reconnect. The tracker updates within 5 minutes of reconnecting (first retry).
7. Remove tracking with the remote checkbox on. The entry is removed locally and on the tracker.
8. Close the window during a refresh. Reopen. The progress is consistent with the tracker.
