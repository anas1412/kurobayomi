# area:27-tracking-core part 2 — edge cases, failure modes, races, tests

Repo root for citations: `/home/blackbox/aniyomi-src`. Behavior references: `p2-behavior.md` (B§n). Data: `p2-data-and-config.md` (D§n). Rust: `p2-rust-design.md` (R§n).

## 1. Upstream tests
- No `*Test.kt` file exists for any scope file (searched the whole tree for Test/Spec files matching track, oauth, pkce, login).
- The only related source is a test helper inside main code: `app/src/main/java/eu/kanade/test/DummyTracker.kt` (51 lines). It is a preview/test double of `Tracker` with defaults: `valStatuses = (1L..6L)`, `valCompletionStatus = 2`, `valScoreList = (0..10) as strings`, `val10PointScore = 5.4`, `username = "username"`, `password = "passw0rd"`. It is outside this area's scope file list (part 1 owns it), but the Rust fixtures in section 7 reuse its values so tests mirror the Kotlin previews.

Conclusion: every test in this document is NEW (written for Kurobayomi), derived from the Kotlin behavior cited.

## 2. Edge cases (numbered; each has an expected Rust outcome)

### 2.1 Dialog and home
- E-01 Tracker logged out while the home is open. Kotlin: the list is not re-filtered until a track row emits (B§3.4 step 1, defect D-4). Rust: the store notifies on login/logout too (`TrackerLoginState` event); home re-assembles. Test T-07 (section 6).
- E-02 Entry has no rows and no tracker is logged in. Kotlin: empty list. Rust: empty-state line (ui parity 2.5).
- E-03 Row exists for a tracker that is now logged out. Kotlin: that tracker is not in `loggedInTrackers`, so the row disappears (B§3.4 step 1-3). Rust: same; the local row stays in DB (not deleted by logout). Document: logging out does NOT delete local rows (Kotlin `logout` only clears credentials, `BaseTracker.kt:80-83`).
- E-04 Refresh returns for a tracker whose row was removed mid-refresh. Kotlin: `insertTrack.await(updatedTrack)` re-inserts with the same `(anime_id, sync_id)` via `ON CONFLICT REPLACE` (new `_id`). Rust: the upsert by `(entry_id, tracker_id)` keeps the row; test T-16.
- E-05 `track == null` and the user presses a row action. Kotlin: NPE via `it.track!!` (B§3.5). Rust: the action is not rendered without a record (ui parity 2.1); if a stale click arrives, `TrackError::UnknownTracker` path returns and shows "No tracking data for this tracker" (new message key Proposal).
- E-06 Enhanced tracker, source not installed. Kotlin: hidden (stub source fails `accept`, B§3.4 step 4). Rust: hidden. Test T-08.
- E-07 Enhanced tracker auto-match fails. Kotlin: toast `error_no_match` (B§3.9). Rust: same text (`MsgKey::ErrorNoMatch`).
- E-08 Enhanced tracker match succeeds but register fails. Kotlin: `register` catches and toasts `e.message` raw (B§3.9 step 3). Rust: `TrackError::Remote(msg)` → toast `"<name> error: <msg>"`. Divergence: prefix added for consistency (Proposal).
- E-09 Entry removed from library while dialog open. Kotlin: enhanced path silently returns (`GetAnime.await ?: return`, A:230). Rust: `EntryRef` lookup returns `None` → return with no toast (same).
- E-10 Refresh with zero logged-in trackers. Kotlin: `RefreshAnimeTracks` returns empty (filter at `RefreshAnimeTracks.kt:29`). Rust: no network call.

### 2.2 Status, progress, score
- E-11 Set status to the completion status with total known. Kotlin sets progress to total (`AnimeTracker.kt:174-176`, condition `track.total_episodes != 0L`). Rust: same rule in part 1 `set_status`. With total 0: progress unchanged. Test T-10.
- E-12 Set status to completion with total unknown (0). Progress unchanged (no 0 → total). Test T-10.
- E-13 Progress set from 0 to N>0 while status is not rewatching. Kotlin: status becomes `getWatchingStatus()` (`AnimeTracker.kt:181-186`). Rust: same. Test T-11.
- E-14 Progress set from 0 to N while status equals rewatching. Status unchanged. Test T-11.
- E-15 Progress reaches total. Kotlin: status = completion, `finished_watching_date = now` (`AnimeTracker.kt:188-191`). Rust: same, `now` from `chrono::Utc::now()` in epoch millis. Test T-11.
- E-16 Progress set above total. Kotlin: no clamp; the `== total` check fails, status unchanged (`toLong() == total`). Rust: the spin button clamps at max (ui parity 4), so this cannot be chosen in the UI; the model does not clamp (matches Kotlin for API callers).
- E-17 Progress fractional (12.9). Kotlin shows 12 and writes 12.0 on OK even if unchanged (defect D-7). Rust: if `selection == floor(progress)` and the user did not change the spin value, the model skips the write (Proposal, divergence). Test T-12.
- E-18 Progress total change from remote refresh while dialog open. Kotlin: the range was computed once (`remember`). Rust: range computed at push time; the spin max does not update until the dialog reopens (matches).
- E-19 Score string not in the list (tracker changed options). Kotlin: index -1 → score -1.0 silently (D-8). Rust: `TrackError::UnknownScore` and toast; record unchanged. Test T-13.
- E-20 Score list index 0 is the current score. Kotlin wheel starts in the middle (`TrackInfoDialogSelector.kt:135`, D-9). Rust starts at 0. Test T-14.
- E-21 Score display for unset (`-1.0` search object or `0.0` row). Row hides score text when `score == 0.0` (`AnimeTrackInfoDialogHome.kt:98-100`); `-1.0` shows the display string of the tracker. Rust: the row hides both `0.0` and unset (divergence documented; part 1 `display_score` handles `-1.0`). Test T-14.
- E-22 Status list contains the current status id missing from the list (tracker removed that status). Kotlin: `RadioButton` list has no selected row. Rust: no radio selected; OK writes the selected id (none) → no write (Proposal).

### 2.3 Dates
- E-23 Start date set after finish date. Blocked by the picker (`isSelectableDate` rule 3). Rust: Calendar clamp (ui parity 6.1). Test T-15.
- E-24 Finish date before start date. Blocked (rule 4). Test T-15.
- E-25 Future date in a UTC-negative zone (New York at 09:00 local on 2026-10-08, picker Oct 9). Kotlin allows Oct 9 (D-10: the picker value converts to Oct 8 local). Rust: uses the picker's UTC date → Oct 9 → rejected. Test T-16 (see section 3).
- E-26 Date remover when the date is 0. Remove button not shown (`canRemove`, B§8.1). Rust: same.
- E-27 Date confirm with no selection. Kotlin: `selectedDateMillis!!` NPE (D-11). Rust: `selected_utc_day` is always seeded; if `None`, the OK is disabled.
- E-28 Date set, then tracker reports 0 on refresh. The stored value may become 0 (refresh writes the remote value). Rust: same (data from tracker wins). Display shows placeholder.
- E-29 DST gap at local midnight (zones whose DST change happens at 00:00, so local midnight does not exist that day). `atZone` in Java moves to the first valid instant after the gap. Rust: `chrono::Local::from_local_datetime(...).earliest()` returns `None` in a gap; use `.single()` → fall back to `.earliest()` and then 01:00 local, documented (Proposal). Test T-16.
- E-30 Start/finish date displayed by Home uses `toLocalDate()` on stored millis (`AnimeTrackInfoDialogHome.kt:101-111`). Stored values are instants of local midnight in the zone at write time. After a large zone change the displayed day can differ by one. Proposal: accepted, no migration.

### 2.4 Search
- E-31 Blank query. Kotlin runs search with "" (B§4.2). Rust: no remote call; hint (ui parity 8.2 Idle).
- E-32 Two searches in quick succession. Kotlin: second search can be overwritten by a slower first response (defect D-6). Rust: `generation` counter; only the latest generation writes results. Test T-17.
- E-33 Close page while loading. Kotlin: response writes into a dead model (harmless). Rust: the page drops; the result is discarded by generation check. Test T-17.
- E-34 Search response with an existing track pre-selection (`tracking_url` equal to `remoteUrl`). Kotlin: pre-select (B§4.2 step 4). Rust: same. Test T-18.
- E-35 `tracking_url` empty on both sides. Kotlin: `"" == ""` → matches the first result with empty URL (currentUrl is `""` when remoteUrl is empty, since `it.track?.remoteUrl` is `""`, not null). Rust: pre-select only when `current_url` is non-empty (divergence: prevents a false pre-select). Test T-18.
- E-36 Confirm with private toggled on for a tracker without private support. Button hidden (`supportsPrivateTracking`). Rust: same; the `private` argument is `false`.
- E-37 Search error (network). Kotlin: raw message in the empty screen (`AnimeTrackerSearch.kt:228-232`). Rust: same text, plus Retry.

### 2.5 Remove tracking
- E-38 Local delete succeeds, remote delete fails. Kotlin: remote error logged, no toast, local row gone (B§10.2). Rust: toast `"<name> error: <msg>"` and local row stays deleted (Proposal: explicit toast; D-13). Test T-19.
- E-39 Remote delete succeeds, local delete fails. Kotlin: local error logged only. Rust: toast "Could not remove the local tracking" (Proposal). Test T-19.
- E-40 Checkbox on, tracker not deletable. Checkbox not shown (B§10.1). Rust: same. Test T-19.
- E-41 The two deletes are concurrent in Kotlin. Rust: sequential (local first, then remote), so a remote failure never leaves a local row pointing at a deleted remote entry that could be re-bound silently. Test T-19. Divergence (Proposal).
- E-42 Remove while a status write for the same row is queued. Serialization lock (R§6) runs the write first, then the delete. Test T-20.

### 2.6 Open, copy, private
- E-43 Blank `remoteUrl`. Kotlin no-op (B§3.6). Rust: toast "No link stored for this entry" (Proposal, new key `NoTrackingLink`). Test T-21.
- E-44 Copy with blank URL: no-op (`copyToClipboard` returns early on blank content, `ContextExtensions.kt:40`). Rust: same as E-43.
- E-45 Long URL in copy toast. Kotlin truncates to 50 chars center with `...` (`truncateCenter(50)`). Rust: same algorithm (`truncate_center(s, 50, "...")`). Test T-22.
- E-46 Private toggle remote failure. Kotlin: local row unchanged (remote-first), toast raw message (B§3.8). Rust: same, toast prefixed. Test T-23.
- E-47 Private toggle on a row whose `track` changed between render and click. Rust: the write uses the record as read at click time; stale data risk is handled by the serialization lock plus a re-read before write (Proposal). Test T-23.

### 2.7 OAuth callback and login
- E-48 Callback for an unknown host. Kotlin: no branch, spinner forever (D-1). Rust: `CallbackOutcome::UnknownHost`, toast. Test T-24.
- E-49 Callback with no fragment or query. Kotlin logs the tracker out (B§12.4, B§12.5). Rust: `Denied` (no logout). Test T-24.
- E-50 AniList fragment token without a trailing `&`. Kotlin regex requires `&` → logout (B§12.4 step 1). Rust: accepts the token up to the end of the fragment or the next `&` (Proposal, divergence). Test T-25.
- E-51 AniList fragment with `access_token` appearing as a substring of another key (`xaccess_token=`). Kotlin: `find` matches inside (no anchor). Rust: anchored match on `access_token=` at start or after `&` (divergence; Proposal). Test T-25.
- E-52 Percent-encoded code (`code=a%2Bb`). Kotlin `getQueryParameter` decodes to `a+b` (Android `Uri` does not map `+` to space). Rust: the `url` crate's `query_pairs()` maps `+` to a space. Rust must use `percent_encoding::percent_decode_str` on the raw query value. Test T-26. Pitfall added to R§11.
- E-53 `code=` empty. Kotlin: `""` is not null → exchange attempted → fails → logout. Rust: `Malformed` (no network call). Test T-24.
- E-54 Multiple `code` params. Kotlin: first value. Rust: first value. Test T-26.
- E-55 AniList token is not percent-decoded by Kotlin (raw substring). Rust: same raw substring (no decode) so the value matches byte for byte. Test T-25.
- E-56 MAL login started twice. Kotlin: `authUrl()` regenerates the static verifier on every call (`MyAnimeListApi.kt:425-427`), so the first browser tab's code is exchanged with the newer verifier and fails; the failure logs the tracker out (D-3). MAL's callback does not echo a `state` value in the source, so attempts cannot be told apart. Rust (Proposal): one pending PKCE attempt per tracker, last attempt wins; a callback that matches no pending attempt gives the message `LoginExpired` and no logout. Test T-27.
- E-57 MAL verifier lost on process restart between browser open and callback. Kotlin: static var reset → exchange sends the new empty verifier → fails → logout. Rust: persist pending attempt in memory with TTL 5 minutes; after restart the callback has no pending session → `Denied`-like outcome "Login expired, start again" (new message key `LoginExpired`, Proposal). Test T-27.
- E-58 Login success for a tracker whose row existed with a different username. Kotlin overwrites username and token (`saveCredentials`). Rust: same.
- E-59 Logout when the tracker is not logged in. Kotlin: idempotent (clears empty values). Rust: idempotent.
- E-60 Denied callback for a logged-in tracker. Kotlin: logs out (token wiped, D-2). Rust: token kept; toast "login cancelled" (ui parity 9). Test T-24.
- E-61 Callback delivered before the main window exists (cold start by URI). Rust: create window, then route URI (topic file section 4). Test T-28 (manual).
- E-62 Two callbacks for the same tracker arrive together. Rust: the `login_*` calls are serialized per tracker (tokio Mutex). Test T-27.
- E-63 Missing client secret for Bangumi/Shikimori/Simkl. Rust: login button disabled with "not configured" toast. Test T-29.
- E-64 Simkl identity call blocks (`runBlocking`) in Kotlin. Rust: async, no block. Test T-29 (timing).
- E-65 Identity call fails after token exchange succeeded (e.g. Shikimori whoami 500). Kotlin: logout, which also discards the token it just received (B§12.5, `logout()` in catch). Rust: keep the token only if identity succeeds; otherwise delete it and report "login failed". Same end state as Kotlin, explicit message. Test T-24.
- E-66 Token expiry. AniList token is treated as 365 days (D-12). Rust: honor `expires_in` when present in the fragment; default 365 days only when absent (Proposal). Test T-30.

### 2.8 Concurrency and lifecycle
- E-67 Dialog closed while a detached write runs. Kotlin: the write continues (launchNonCancellable). Rust: the write continues (tokio task not aborted). Test T-20.
- E-68 App quit during a write. Rust: the runtime is shut down with a 2-second grace period. Unfinished remote writes are not retried automatically (Kotlin has no retry either). The local DB write happens only after remote success, so no half-written state exists. Test T-31 (kill during write).
- E-69 Refresh started on open, dialog closed before it finishes. Kotlin: `screenModelScope` cancels the refresh (the refresh is inside the screen model's launch). Rust: the refresh future is dropped with the window; each per-tracker task is also dropped, not `tokio::spawn`ed. (Divergence: Kotlin refresh is cancellable, so use drop semantics.) Test T-32.
- E-70 Toast fired after the window closed. Rust: toasts go to the app-level overlay, not the dialog. Test T-33.

### 2.9 Network and data
- E-71 Tracker returns a success with an empty body. Part 1 decides parse errors; the dialog shows `TrackError::Remote("")` → "<name> error: " (matches Kotlin empty text).
- E-72 Offline. Search: raw error text (reqwest error). Refresh: one toast per logged-in tracker. Writes: toast, DB unchanged.
- E-73 Track row FK disappears (entry deleted). Kotlin: `ON DELETE CASCADE` removes rows; open dialog keeps stale data until closed. Rust: write fails with `TrackError::Store` ("entry no longer exists"), toast. Test T-34.
- E-74 Backup restore removes tokens (`__PRIVATE_` not backed up). After restore, every tracker shows as logged out. Rust: same; the Settings row says "Log in again". Test T-35.

## 3. Date conversion reference (worked examples)

Notation: `D` = calendar day. "stored" = epoch millis of local midnight of D in the system zone (B§8.2-8.4). "picker" = epoch millis of 00:00 UTC of D (what the Material picker returns).

| Zone (Oct 2026, DST in effect unless noted) | stored(D = 2026-10-08) | picker(D) | Kotlin day for picker (bug path) | Fixed day for picker |
|---|---|---|---|---|
| UTC | 2026-10-08T00:00Z | 2026-10-08T00:00Z | 2026-10-08 | 2026-10-08 |
| Europe/Berlin (UTC+2) | 2026-10-07T22:00Z | 2026-10-08T00:00Z | 2026-10-08 (02:00 local) | 2026-10-08 |
| Asia/Kolkata (UTC+5:30) | 2026-10-07T18:30Z | 2026-10-08T00:00Z | 2026-10-08 (05:30 local) | 2026-10-08 |
| Pacific/Auckland (UTC+13 NZDT) | 2026-10-07T11:00Z | 2026-10-08T00:00Z | 2026-10-08 (13:00 local) | 2026-10-08 |
| America/New_York (UTC-4 EDT) | 2026-10-08T04:00Z | 2026-10-08T00:00Z | 2026-10-07 (20:00 local) | 2026-10-08 |

Stored value examples are the instants of local midnight; they are correct for "stored = local midnight" (B§8.4). The picker column is what the Material picker produces for D.

Check for E-25 (New York, today = 2026-10-08 09:00 EDT, future rule):
- Picker value for D = 2026-10-09 is `2026-10-09T00:00Z`.
- Kotlin: `Instant(...).atZone(systemDefault).toLocalDate()` = 2026-10-08 20:00 EDT → 2026-10-08 → `> LocalDate.now()` (2026-10-08) is false → SELECTABLE (bug).
- Rust: UTC date = 2026-10-09 > 2026-10-08 → NOT selectable (fix).

Year bounds check (isSelectableYear): `year > LocalDate.now().year` → false. Example: now = 2026; year 2027 not selectable.

Month-end and leap day: D = 2028-02-29 stored and picker round-trip must keep the day. Test T-16 includes it.

## 4. Progress and score worked examples
- `progress_range(total = 24)` = `0..=24` (25 values, Kotlin `0..24`, `TrackEpisodeSelectorScreen` A:382).
- `progress_range(total = 0)` = `0..=10000` (10 001 values).
- `progress_range(total = -1)`: Kotlin `if (track.totalEpisodes > 0)` → else branch → 10000. Rust same (negative totals count as unknown).
- Truncation: `12.9` → `12`; `-0.5` → `0` (Kotlin `toInt` truncates toward zero; the value is never negative in practice).
- Score list `["0", "1", …, "10"]`: `index_of("7") = 7`; `index_to_score(7) = 7.0` (default `indexToScore`, `AnimeTracker.kt:136-138`); `index_of("11") = None` → `UnknownScore`.

## 5. Completion and status rules (part 1 functions used by the dialogs; fixture for T-10/T-11)
Fixture: fake tracker with statuses {1: Watching, 2: Completed, 3: On hold, 4: Dropped, 5: Plan to watch, 6: Rewatching}, `watching = 1`, `rewatching = 6`, `completion = 2` (values from `DummyTracker.kt` defaults, one of which is `valStatuses = 1..6`).

| Start record (progress, total, status) | Action | Expected record |
|---|---|---|
| (0, 24, 5) | set progress 3 | (3, 24, 1) watching |
| (0, 24, 6) | set progress 3 | (3, 24, 6) rewatching unchanged |
| (3, 24, 1) | set progress 24 | (24, 24, 2), finish date = now |
| (3, 24, 6) | set progress 24 | (24, 24, 2), finish = now (the completion rule has no rewatching guard) |
| (5, 24, 1) | set status 2 (completion) | (24, 24, 2) |
| (5, 0, 1) | set status 2 (completion) | (5, 0, 2) |
| (5, 24, 1) | set progress 5 (same value) | (5, 24, 1) and remote write still performed (Kotlin always writes) |

Note the row with rewatching: the rule in step 1 excludes `status == rewatching` only for the zero → n transition (B§6 steps 1 and 3). The completion rule (step 3) has no rewatching guard. Both behaviors are kept.

## 6. Test list (new, Rust, in `crates/kuro-track/tests/` unless stated)

| ID | Name | File | Covers | Input | Expected |
|---|---|---|---|---|---|
| T-01 | parse_callback_table | `tests/oauth_callback.rs` | B§12, E-48..E-55 | table in section 7.1 | outcomes listed |
| T-02 | pkce_verifier_shape | `tests/pkce.rs` | D§5.1 | 1000 generations | len 67, charset `A-Za-z0-9-_`, all distinct, challenge == verifier |
| T-03 | progress_range_bounds | `src/progress.rs` unit | B§6, §4 | totals 0, -1, 1, 24 | ranges 0..=10000, 0..=10000, 0..=1, 0..=24 |
| T-04 | progress_truncate | `src/progress.rs` unit | E-17 | 12.9, 0.0, 3.99 | 12, 0, 3 |
| T-05 | score_lookup | `src/score.rs` unit | B§7 | "7", "11", "" | Some(7), None, None |
| T-06 | date_selectable_rules | `src/dates.rs` unit | B§8.3 | rule table (start/finish/future, same day, leap day) | per rule |
| T-07 | assemble_items_logged_out | `src/item.rs` unit | E-01, E-03 | logged-in list changes | rows filtered |
| T-08 | assemble_items_enhanced_source | `src/item.rs` unit | E-06 | enhanced tracker, accepted and not accepted sources | hidden when not accepted |
| T-09 | assemble_items_order | `src/item.rs` unit | B§3.4 step 5 | shuffled tracker list | TrackerManager order |
| T-10 | completion_status_rule | `tests/track_rules.rs` | E-11, E-12 | section 5 table rows 5-6 | expected |
| T-11 | progress_rules | `tests/track_rules.rs` | E-13..E-15 | section 5 table rows 1-4 | expected |
| T-12 | progress_no_op_fractional | `tests/track_rules.rs` | E-17 | progress 12.9, confirm 12 | no write |
| T-13 | score_unknown_label | `tests/track_rules.rs` | E-19 | "11" | `UnknownScore`, record unchanged |
| T-14 | score_initial_index | `src/track/score.rs` (GUI-free model) | E-20, E-21 | index 0 and unset | selection = current; hidden text for 0.0 |
| T-15 | date_clamp | `src/track/dates.rs` model | E-23, E-24 | selecting outside bounds | clamped to nearest allowed |
| T-16 | date_roundtrip_zones | `tests/dates_zones.rs` (dev-dep `chrono-tz`) | E-25, E-29, section 3 | zones UTC, Europe/Berlin, Asia/Kolkata, Pacific/Auckland, America/New_York; days 2026-10-08, 2028-02-29, 2026-03-29 (Europe/Berlin DST change day, which happens at 02:00 and not at midnight) | stored and picker values as table; no panic on a midnight gap (use a synthetic zone with a midnight gap for this case) |
| T-17 | search_generation | `src/track/search.rs` model | E-32, E-33 | start A, start B, complete B, complete A | results = B only |
| T-18 | search_preselect | `src/track/search.rs` model | E-34, E-35 | current url "x", results with "x"; current url "" | preselect first only in first case |
| T-19 | remove_order_and_errors | `tests/remove.rs` (fake tracker) | E-38..E-41 | remote fails / local fails / checkbox off | local first; errors surfaced |
| T-20 | write_serialization | `tests/locks.rs` | E-42, E-62, E-67 | two writes same row, close dialog | ordered, both complete |
| T-21 | blank_link_no_op_toast | `src/track/sheet.rs` model | E-43 | url "" | toast text, no open call |
| T-22 | truncate_center_50 | `src/track/messages.rs` unit | E-45 | 120-char url | 50 chars with `...` centred |
| T-23 | private_toggle_failure | `tests/track_rules.rs` | E-46, E-47 | remote error | local unchanged, toast |
| T-24 | login_outcomes | `tests/login_flow.rs` (fake tracker) | E-48, E-49, E-53, E-60, E-65 | callback variants | state of token store as stated |
| T-25 | anilist_fragment_tokens | `tests/oauth_callback.rs` | E-50, E-51, E-55 | fragments: with `&`, without `&`, `xaccess_token=`, raw token with `%` | raw substring rules |
| T-26 | query_percent_decode | `tests/oauth_callback.rs` | E-52, E-54 | `code=a%2Bb`, `code=a+b`, two `code` params `code=x&code=y` | `a+b`, `a+b` (plus stays literal), `x` (first value) |
| T-27 | pkce_pending_attempts | `tests/login_flow.rs` | E-56, E-57, E-62 | two authorize calls, then a callback; a callback after restart | the callback uses the last attempt's verifier; a callback with no pending attempt gives `LoginExpired` and no token change |
| T-28 | cold_start_uri | manual + `tests/platform_uri.rs` (feature `platform-linux`) | E-61 | `xdg-open aniyomi://bangumi-auth?code=x` with app closed | window created then login job runs |
| T-29 | client_secret_missing | `tests/config.rs` | E-63 | empty config | disabled, toast text |
| T-30 | anilist_expiry | `tests/oauth_callback.rs` | E-66 | fragment with `expires_in=3600` | expiry = now + 3600 s; absent → 365 days |
| T-31 | kill_during_write | manual | E-68 | SIGKILL during remote call | DB unchanged for that row |
| T-32 | refresh_drop | `tests/refresh.rs` | E-69 | drop window during refresh | no further calls after drop |
| T-33 | toast_after_close | `src/track/toast.rs` | E-70 | toast after window closed | shown on app overlay |
| T-34 | fk_missing_entry | `tests/track_rules.rs` | E-73 | write for deleted entry | `TrackError::Store`, toast |
| T-35 | restore_drops_tokens | `tests/backup_tokens.rs` | E-74 | backup + restore | token store empty |

## 7. Fixtures

### 7.1 OAuth callback table (T-01, T-25, T-26)
| Input URI | Expected `CallbackOutcome` | Kotlin result | Note |
|---|---|---|---|
| `aniyomi://anilist-auth#access_token=abc123&token_type=Bearer&expires_in=31536000` | `AniListToken("abc123", expires_in=Some(31536000))` | login | normal |
| `aniyomi://anilist-auth#access_token=abc123` | `AniListToken("abc123", None)` | logout | divergence E-50 |
| `aniyomi://anilist-auth#error=access_denied` | `Denied(AniList)` | logout | divergence: token kept |
| `aniyomi://anilist-auth` | `Denied(AniList)` | logout | |
| `aniyomi://bangumi-auth?code=XYZ` | `Code(Bangumi, "XYZ", pkce=None)` | login | |
| `aniyomi://bangumi-auth?error=access_denied` | `Denied(Bangumi)` | logout | |
| `aniyomi://bangumi-auth?code=` | `Malformed` | login attempt then logout | E-53 |
| `aniyomi://myanimelist-auth?code=a%2Bb` | `Code(MyAnimeList, "a+b", pkce=<pending or None>)` | login | E-52 |
| `aniyomi://shikimori-auth?code=x&code=y` | `Code(Shikimori, "x")` | login | E-54 |
| `aniyomi://simkl-auth?code=k` | `Code(Simkl, "k")` | login | |
| `aniyomi://kitsu-auth?code=x` | `UnknownHost("kitsu-auth")` | spinner forever | E-48 |
| `aniyomi://` | `UnknownHost("")` | spinner forever | E-48 |
| `https://bgm.tv/oauth?code=x` | `Malformed` | not delivered by intent filter | |

### 7.2 Remote payload samples (for part 1 parsers and login)
Record with a throwaway account on each tracker and store under `tests/fixtures/track/`: `mal_users_me.json` (`{"name": ...}`), `bgm_me.json` (`{"username": ...}`), `shiki_whoami.json` (`{"id": ...}`), `simkl_settings.json` (`{"account": {"id": ...}}`), `anilist_viewer.json` (`{"data": {"Viewer": {"id": ..., "mediaListOptions": {"scoreFormat": "POINT_10"}}}}`). The shapes are from D§5.3. Do not commit real account data.

### 7.3 Clock and zone
Tests take `now` and the zone as parameters (no `Local::now()` inside pure functions). Integration tests set `TZ` per test process.

## 8. Failure-mode matrix (summary)

| Failure | Kotlin | Rust |
|---|---|---|
| Search network error | raw text in empty state | raw text + Retry |
| Refresh error | toast `track_error` | same, one per tracker |
| Remote write error | toast raw message, local unchanged | toast `track_error`, local unchanged |
| Remote delete error | log only | toast |
| Local delete error | log only | toast |
| Login exchange error | logout | keep previous state, toast |
| Callback denied | logout | keep state, toast |
| Callback unknown | spinner forever | toast |
| Missing client secret | (not applicable; secrets are in source) | disabled, toast |
| Null track in row action | NPE crash | not rendered; toast if stale |
| Stale tracker id | NPE (`!!`) | `UnknownTracker`, toast |

## 9. Items deliberately not tested here
- Visual layout of the Compose dialogs (part 1 and ui parity doc).
- Per-tracker payload mapping (areas 28-33).
- Notification and scheduler behavior (areas 26, 35).
