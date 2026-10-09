# Area 42 (part 2): edge cases, failure modes, races, tests

Upstream tests: none. No `*Test.kt` file exists in the scope list (`areas.py files 42-ui-updates-history-stats-storage-upcoming 2`), so there is nothing to port directly. The tests below are new, written for the Rust port, and each one encodes an observed Aniyomi behaviour from `p2-behavior.md`.

Cross-references: B§ = `p2-behavior.md`, U§ = `p2-ui-parity.md`, D§ = `p2-data-and-config.md`, R§ = `p2-rust-design.md`.

---

## 1. Edge cases: updates feed

E-U1. Chapter with `date_upload = 0` (unknown upload date). Excluded by `dateUpload > after` (D§3.1). Expected: not shown in Updates even if fetched today. Keep (observed). Test: `updates_excludes_unknown_upload_date`.

E-U2. Chapter fetched before the manga was added to the library (`date_fetch <= date_added`). Excluded by the view filter `date_fetch > date_added` (D§3.1). Expected: not shown. Test: `updates_excludes_backfill`.

E-U3. Manga removed from library (`favorite = 0`) after its chapters were fetched. Excluded by the view. Expected: its updates disappear from the feed on the next query. Test: `updates_drop_when_unfavorited`.

E-U4. Window boundary: `date_upload` exactly equal to the cut-off. Filter is strict `>`. Expected: excluded. Test: `updates_window_is_strict`.

E-U5. More than 500 qualifying rows. LIMIT 500 applies after ORDER BY `date_fetch DESC`, so the 500 most recently fetched are kept; older ones vanish from the list even though inside the window. Expected: at most 500 rows. Test: `updates_limit_500_by_fetch_order`.

E-U7. Chapter name collision for download-state check: `isChapterDownloaded(name, scanlator, mangaTitle, sourceId)` is name based (B§1.3). Two chapters in one manga with the same name and scanlator share the on-disk check, so one may show DOWNLOADED while the other is not. Expected (observed). Proposal: keep the name-based check for parity and add a test documenting the collision. Test: `download_state_name_collision_is_observed`.

E-U8. Empty manga title or chapter name: rendered as empty text, no placeholder. Expected: row still rendered at 56 px. Test: `row_renders_with_empty_title`.

E-U9. Extremely long title: 1 line ellipsis (U§1.4). Test is visual; add a snapshot-free check that `ellipsize = End` and `lines = 1` are set.

E-U10. Read and progress interaction: a read row never shows progress (`!read && lastPageRead > 0`). An unread row with `lastPageRead = 0` shows no progress. Test: `progress_only_when_unread_and_positive`.

E-U11. Manga progress uses `lastPageRead + 1` (Page: 1-based). `lastPageRead = 0` never shows (filtered). `lastPageRead = 4` shows "Page: 5". Test: `manga_progress_is_one_based`.

E-U12. Anime progress with `lastSecondSeen = 3_600_000` → "60:00" (exclusive threshold, D§/B§6.6). `3_600_001` → "1:00:00". Test: `format_progress_thresholds`.

E-U13. Anime progress with `totalSeconds = 0` (unknown duration). Format gives "0:00" for the total. Expected: "12:34/0:00". Observed, keep. Test: `anime_progress_with_zero_total`.

E-U14. Relative date boundaries: difference -7 gives "In 7 days", -8 gives an absolute date, +6 gives "6 days ago", +7 gives absolute. Test: `relative_date_boundaries` (B§6.3).

E-U15. Relative date `today` computed in local time at midnight rollover. Expected: headers update on next refresh only (Aniyomi). Proposal: recompute on each render. Test: `relative_date_uses_passed_today` (pure function test).

E-U16. DST day with 23 or 25 hours. `local_date` converts ms to local date per instant; grouping stays correct. Test: `grouping_across_dst_change` with a fixture on a known DST date in `America/New_York` (use `TZ` env in the test).

E-U17. Midnight boundary: an update at 23:59:59.999 local and one at 00:00:00.001 next day produce two headers. Test: `grouping_splits_at_local_midnight`.

E-U18. Timezone change while the app runs: groups computed with the new zone only after a refresh. Expected: same as Aniyomi. Note only.

E-U19. Empty feed (`items.isEmpty()`) after loading: empty state text "No recent updates". Test: `empty_state_when_no_rows`.

E-U20. Feed error before first emission: Aniyomi keeps `isLoading = true` forever (B§1.5). Rust port: show error toast, leave the list empty and not loading. Test: `error_clears_loading_state` (deviation, recorded).

E-U21. Feed error after first emission: Aniyomi stops updating (B§1.2). Rust: keep looping and retry on the next change signal. Test: `error_does_not_stop_feed` (deviation, recorded).

E-U22. Download progress tick for a row not in the list (chapter removed by refresh). Aniyomi: `indexOfFirst` returns -1, no-op. Rust: `position` returns None, no-op. Test: `progress_for_missing_row_is_noop`.


E-U24. Download status ERROR on a selected row followed by START: `startDownloads()` is called after enqueue (B§3.1). Test: `start_triggers_restart_when_any_error`.

E-U25. START with a mixed selection where some rows' manga source is missing: those groups are skipped silently (B§3.2). Test: `enqueue_skips_missing_source_group`.

E-U26. START with rows from many manga: one `downloadChapters` call per manga, in first-seen order (B§3.2). Test: `enqueue_groups_in_first_seen_order`.

E-U27. CANCEL on a row that is not queued: `getQueuedDownloadOrNull` returns null, no-op. Test: `cancel_missing_is_noop`.

E-U28. Cancel mutates the shared download object (B§3.3). Rust port: never mutate shared state from the UI; send a cancel command and let the download module emit NotDownloaded. Test: `cancel_emits_not_downloaded_patch` (deviation: no mutation).

E-U29. Delete with a mix of downloaded and non-downloaded selected rows: all selected chapters are passed to the download manager (B§4.3). Test: `delete_passes_full_selection`.

E-U30. Delete for a manga whose source is no longer installed: silently skipped (B§4.3). Downloads remain on disk. Expected (observed). Proposal: `tracing::warn!` and a toast "Source not installed, downloads were not deleted". Recorded as deviation. Test: `delete_skips_missing_source_with_warning`.

E-U31. Mark read on an already read row: filtered out, `NoChapters` result (B§4.1). Test: `read_targets_skip_already_read`.

E-U32. Mark unread on a row with `read = false, lastPageRead > 0`: included (progress reset), `lastPageRead` set to 0. Test: `unread_resets_progress`.

E-U33. Mark read with `removeAfterMarkedAsRead = true`: downloads of the newly read chapters are deleted, grouped per manga. Chapters already read are not deleted. Test: `remove_after_read_only_newly_read`.

E-U34. Bookmark toggle on a mixed selection: only rows with the wrong flag are written (B§4.2). Test: `flag_targets_only_changed_rows`.

E-U35. Anime play routing with `alwaysUseExternalPlayer` both values and alt flag (B§12.5). Table test of four combinations:
- pref false, alt false → internal
- pref false, alt true → external
- pref true, alt false → external
- pref true, alt true → internal
Test: `player_route_truth_table`.

E-U36. Anime row tap when the source is missing: `startPlayerActivity` looks up the source. Out of scope for this area (area:36-ui-shell-navigation-security-webview, area:24-downloads-anime). Note only.

E-U37. Library update already running: `startNow` returns false, snackbar "An update is already running" (B§10.1). Test: `library_trigger_reports_already_running`.

E-U38. Library update trigger rapid double click: second call returns false (already running) once the first is enqueued. Test: `library_trigger_single_flight`.

E-U39. Snackbar events emitted while the tab is not visible: queued and shown later (B§10.1). Rust: `async_channel::unbounded` plus the page consumes on show. Test: `events_buffered_until_page_visible`.

E-U40. Pull-to-refresh while selecting: disabled (U§1.8). Test: `refresh_disabled_in_selection` (UI test, manual).

E-U41. "Last updated" value 0: "Never" (B§6.5). Test: `last_updated_never`.

E-U42. "Last updated" within the last minute: "Just now". Test: `last_updated_just_now`.

E-U43. New-count reset: entering and leaving the tab sets the counter to 0 (B§10.4). Test: `counter_reset_on_enter_and_leave`.

E-U44. Anchor semantics (selection, B§5.2). Test vectors are in section 3 below.

E-U45. Selection survives feed refresh (B§5.4). Test: `selection_survives_refresh`.

E-U46. Stale anchors after refresh (B§5.4). Observed: anchors not recomputed. Test: `stale_anchor_after_refresh_is_preserved` (documents the quirk).

E-U47. Selection mode derived from visible items (B§5.4). Set non-empty but no visible selected rows → normal mode. Test: `selection_mode_from_visible_rows`.

E-U48. Clearing selection with the Escape key while not selecting: pop the page (U§1.11). Test: manual.

E-U49. Selection-clear skipped for multi-select START_NOW/CANCEL/SHOW_QUALITIES (B§3.1). Test: `dispatch_multi_startnow_keeps_selection` (uses `dispatch_download_precondition`).

E-U50. Long-press range select when the anchor is not set (both -1): first selection becomes the anchor (B§5.2 step 5). Test: `long_press_first_sets_anchor`.

E-U51. Long-press range select downward and upward. Test vectors in section 3.

E-U52. Tap-deselect of the first anchor: anchor moves to the first selected row. Test vectors in section 3.

E-U53. Tap-select when no anchors exist: only anchor[1] is set; anchor[0] stays -1 (B§5.2 step 6 note). Test: `tap_select_sets_only_upper_anchor`.

E-U54. Select all then invert: invert of all-selected gives none-selected, anchors -1 (B§5.3). Test: `invert_all_clears`.

---

## 2. Edge cases: upcoming

E-C1. Entry with `next_update` earlier today (e.g. 00:30 today) is included (>= local midnight). Entry with `next_update` at 23:59 yesterday is excluded. Test: `upcoming_includes_today_from_midnight`.

E-C2. Entry whose `next_update` is very far in the future (year 2100): included, grouped under that date, no upper bound. Test: `upcoming_no_upper_bound`.

E-C3. Entry with `status = COMPLETED (2)`: never returned by the query (not in set). Test: `upcoming_excludes_completed`.

E-C4. Entry with `status = UPCOMING (7)` in anime: included. In manga, status 7 does not exist; manga set excludes 7 by design. Test: `anime_upcoming_status_included`.

E-C5. Entry `favorite = 0` with a future `next_update`: excluded. Test: `upcoming_requires_favorite`.

E-C6. Two entries on the same local date: one header with count 2, ascending order by `next_update` then by query tie order (SQLite unspecified). Expected: order among equal `next_update` is unspecified; Proposal: add `, _id` as the tie-breaker for determinism. Test: `upcoming_tie_break_by_id` (Proposal; deviates in order only).

E-C7. Events map: day with 5 entries → count 5 in `events`, 3 dots drawn (B§8.4). Test: `dots_capped_at_three`.

E-C8. Header index mapping for day jump: `headerIndexes[date]` is the index of the header in the `items` list without the calendar row (B§7.6). Phone scroll index = header index + 1. Test: `day_jump_index_phone_offset`.

E-C9. Day click for a day with no header: no scroll (B§7.6). Test: `day_click_without_header_is_noop`.

E-C10. Month navigation does not filter the list (B§7.5). Test: `month_change_keeps_list`.

E-C11. Month navigation across year boundary: December → January increments year. Test: `shift_month_year_boundary` (calendar::shift_month).

E-C12. Leading blanks for each first weekday, Monday-first and Sunday-first. Test vectors in section 3.

E-C13. Locale first day: `en_US` Sunday, `en_GB` Monday, `ja_JP` Sunday, `de_DE` Monday, `ar_SA` Sunday, `fa_IR` Saturday. Test: `week_start_by_locale`.

E-C14. Leap year February: 29 days in 2028, 28 in 2026. Test: `days_in_month_leap`.

E-C15. Calendar today ring when today is in a month that is not displayed: no ring. Test: manual.

E-C16. Past day dimming uses `isBefore(today)`. Today is not dimmed. Test: `past_days_dimmed_today_not_dimmed`.

E-C17. Upcoming list ordering when `next_update` values are equal across manga and anime: separate pages, no interaction.

E-C18. Upcoming page open across midnight: start-of-day is computed at page creation (B§7.2, R§10 pitfall 8). Expected: list stale until reopened. Proposal: re-query when the page is shown again or on the first focus after midnight. Test: `upcoming_refreshes_after_midnight` (Proposal).

E-C19. Locale month name in title: "MMMM yyyy" uses the default locale. Rust: chrono `%B %Y` uses English names. Deviation: English month names unless locale support is added. Test: `month_title_english_fallback`.

E-C20. Calendar weekday narrow names: `NARROW` gives one letter. In Rust: first letter of the chrono short weekday name. Test: `weekday_narrow_names` (for English: M T W T F S S).

---

## 3. Test vectors (pure functions)

### 3.1 Selection (Selection::toggle), rows = 8 items with ids 1..8, index = id - 1

| Step | Call | Result `selected` flags (idx 0..7) | anchors |
|---|---|---|---|
| 1 | long-press select id 3 (idx 2) | 0,0,1,0,0,0,0,0 | [2, 2] |
| 2 | long-press select id 6 (idx 5) | 0,0,1,1,1,1,0,0 | [2, 5] |
| 3 | long-press select id 1 (idx 0) | 1,1,1,1,1,1,0,0 | [0, 5] |
| 4 | tap deselect id 1 (idx 0) | 0,1,1,1,1,1,0,0 | anchors[0] == 0 → first selected = idx 1: [1, 5] |
| 5 | tap select id 8 (idx 7) | 0,1,1,1,1,1,0,1 | [1, 7] |
| 6 | tap deselect id 8 (idx 7) | 0,1,1,1,1,1,0,0 | anchors[1] == 7 → last selected = idx 5: [1, 5] |

Check: step 3 range for idx 0 with anchors [2,5]: `i < anchors[0]` → range `1..2` (exclusive) = idx 1 only; but expected row says idx 1 becomes selected through the range; and idx 0 itself is selected by the toggle. So the flags after step 3 are 1,1,1,1,1,1,0,0. Confirmed.

Anchors in step 2: `i > anchors[1]` with anchors [2,2] → range (2+1)..5 = idx 3,4 selected; anchors [2,5]. Flags 0,0,1,1,1,1,0,0. Confirmed.

### 3.2 Selection anchor quirks

| Case | Input | Expected anchors |
|---|---|---|
| Tap-select first item when nothing selected | select idx 4 (tap) | [-1, 4] (anchor[0] stays -1) |
| Tap-select then long-press next | tap idx 4, then long-press idx 6 | range `(4+1)..6` → idx 5 selected; anchors [-1, 6] |
| Long-press at -1 anchors but another item selected | selected idx 2 via programmatic toggle, then long-press idx 5 | firstSelection = false; `i > anchors[1] (-1)` → range `0..5` → idx 0,1,3,4 selected; anchors [-1, 5] |

The third row is a quirk (B§5.2). It is preserved in the port and documented in the test name `quirk_long_press_with_stale_anchors`.

### 3.3 Calendar leading blanks

| Month | First weekday | Monday-first leading | Sunday-first leading |
|---|---|---|---|
| 2026-10 | Thursday | 3 | 4 |
| 2026-02 | Sunday | 6 | 0 |
| 2026-03 | Sunday | 6 | 0 |
| 2027-01 | Friday | 4 | 5 |

Derivation check (Monday-first, 2026-10): Thu is index 3 in [Mon,Tue,Wed,Thu,Fri,Sat,Sun]. Correct. Sunday-first, 2026-10: [Sun,Mon,Tue,Wed,Thu,Fri,Sat], Thu index 4. Correct.

### 3.4 Relative date (`time_fmt::relative_date`, today = 2026-10-08)

| Date | Output (relative = true) |
|---|---|
| 2026-10-08 | Today |
| 2026-10-07 | Yesterday |
| 2026-10-02 (diff 6) | 6 days ago |
| 2026-10-01 (diff 7) | absolute |
| 2026-10-09 (diff -1) | Tomorrow |
| 2026-10-15 (diff -7) | In 7 days |
| 2026-10-16 (diff -8) | absolute |
| any with relative = false | absolute |

### 3.5 Upcoming grouping (`upcoming::model::build_upcoming`)

Input (ascending next_update, local dates): A(10-08), B(10-08), C(10-09), D(10-15).
Output rows: `Header(10-08, 2), Item(0), Item(1), Header(10-09, 1), Item(2), Header(10-15, 1), Item(3)`.
events: {10-08: 2, 10-09: 1, 10-15: 1}. header_index: {10-08: 0, 10-09: 3, 10-15: 5}.

### 3.6 Progress formatting (`updates::fmt::format_progress`)

| ms | output |
|---|---|
| 0 | 0:00 |
| 754000 | 12:34 |
| 3600000 | 60:00 |
| 3600001 | 1:00:00 |
| 4500000 | 1:15:00 |

### 3.7 Last updated (`time_fmt::relative_time_span`, now = 1_000_000_000_000)

| epoch_ms | output |
|---|---|
| 0 | Never |
| now - 30_000 | Just now |
| now - 60_000 | 1 minute ago (Proposal: singular form; the Android string is not reproduced) |
| now - 3_600_000 | 1 hour ago |

Proposal note: the plural forms above are Proposal wording; the Android platform string is unavailable. Tests assert the Proposal wording.

---

## 4. Race conditions and concurrency

R1. Feed refresh vs download patch. A download progress patch arrives while a DB re-query is in flight. Rust port: patches apply to the current rows; the in-flight result replaces the rows and then re-applies `Selection::apply_to`. Download state is re-derived from the download manager at build time (B§1.3), so the post-query row has the newest state. Test: `patch_then_refresh_consistent`.

R2. Two change signals during a query: coalesce into one follow-up query (R§3.10). Test: `coalesce_change_signals`.

R3. Selection change while a bulk write runs. Write uses the snapshot of selected rows taken at click time (B§4). Changing selection afterward does not affect the in-flight write. Test: `write_uses_click_time_snapshot`.

R4. Page closed during a write. Writes are non-cancellable (R§6). Test: `write_completes_after_page_drop`.

R5. Delete while a download is starting for the same chapter. The download module owns the race (area:23-downloads-manga, area:24-downloads-anime); this area only dispatches. Note.

R6. Library update job finishes during the 1 s fake spinner: spinner still hides after 1 s; feed updates from the change signal. Test: manual.

R7. Two Updates pages open (manga and anime) share the DB. Reads only; no conflict. Writes target different tables.

R8. `Selection` is not thread-safe and must be used only on the GTK thread. Enforced by `Rc`/`RefCell` (not `Send`). Compile-time check.

---

## 5. Failure modes

F1. DB locked (busy). Retry with `busy_timeout(5000 ms)` on the connection (Proposal). Surface an error after 5 s as the internal error toast.

F2. Corrupt or missing view (old DB without `updatesView`). The app's migrations create it (area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations). Query error → internal error toast; no crash.

F3. Source extension uninstalled: enqueue skips, delete skips (B§3.2, B§4.3). Show warning only (E-U30).

F4. Download manager not running: `startDownloadNow` and `startDownloads` are queued calls; no error path in this area.

F5. Library update trigger fails to start (WorkManager not available on Android; desktop: scheduler error). Map to `started = false` and the "already running" message is wrong text for that case. Proposal: distinct message "Could not start library update" (key not in MR; add an Aniyomi-compatible key only if translation support is required). Recorded as deviation.

F6. Huge download progress updates (many per second). Coalesce progress patches per row at 10 Hz for UI. Proposal.

F7. Cover image missing: placeholder cover (area:34-images-covers-cache handles). Row still renders.

F8. Invalid `date_fetch` (0 or negative): `local_date` returns 1970-01-01 fallback; grouping shows an old header. Aniyomi would produce 1970 too. Expected: observed parity; add warning log.

F9. Clock skew: `now` in the future relative to `date_fetch`: relative date returns "In N days" for updates, which is wrong for an update. Aniyomi shows the same. Note only.

---

## 6. New test list (Rust)

Location: `src/updates/*.rs` and `src/upcoming/*.rs` as `#[cfg(test)] mod tests`. No display needed for model and selection tests. UI smoke test with `gtk::init` gated behind an env var `KUROBAYOMI_UI_TESTS=1`.

Unit (pure):
1. `selection::tests::long_press_range_*` (3.1 table), `tap_deselect_moves_anchor`, `quirk_long_press_with_stale_anchors`, `tap_select_sets_only_upper_anchor`, `set_all_resets_anchors`, `invert_all_clears`.
2. `model::tests::group_by_day_*` (E-U16, E-U17, first item always header, empty input).
3. `upcoming::model::tests::build_upcoming_*` (3.5 vector, dots cap, ties).
4. `upcoming::calendar::tests::leading_blanks_*` (3.3 vectors), `days_in_month_leap`, `shift_month_year_boundary`, `week_start_by_locale`, `week_days_sunday_first`.
5. `time_fmt::tests::relative_date_boundaries` (3.4), `relative_time_span_cases` (3.7), `relative_date_uses_passed_today`.
6. `updates::fmt::tests::format_progress_thresholds` (3.6), `manga_progress_is_one_based`, `progress_only_when_unread_and_positive`.
7. `updates::actions::tests::dispatch_*` (E-U49), `plan_enqueue_*` (E-U25, E-U26), `read_targets_*` (E-U31, E-U32), `remove_after_read_only_newly_read`, `flag_targets_only_changed_rows`, `player_route_truth_table` (E-U35).
8. `upcoming::store::tests::start_of_today_ms_local_midnight` with `TZ=America/New_York` set in the test process via `std::env::set_var` before chrono reads it (chrono reads TZ on each `Local` call on Unix; use a fixed fixture date).

Integration (rusqlite, in-memory DB with the Kurobayomi schema):
9. `sql::updates_*` (E-U1..E-U5) using fixtures in section 7.
10. `sql::upcoming_*` (E-C1..E-C5) using fixtures.
11. `sql::set_read_writes_and_remove_after_read` (E-U33) with a fake download module trait.

UI smoke (env-gated):
12. `ui::updates_page_builds_with_empty_state`, `ui::updates_page_builds_with_rows`, `ui::upcoming_page_two_panel_at_720`, `ui::calendar_grid_cells_match_month`.

Manual checklist (not automated): Ctrl/Shift select, Escape, F5, toast timing, month slide animation, day-jump scroll.

---

## 7. Fixtures

Stored as SQL under `tests/fixtures/updates_upcoming.sql` (Kurobayomi repo):
- Tables: `mangas`, `chapters`, `animes`, `episodes` with the columns listed in D§3, plus `date_added`, `date_fetch`, `date_upload`, `next_update`, `status`, `favorite`.
- Rows:
  - M1 favorite, date_added = 1_700_000_000_000; chapters: C1 date_fetch 1_760_000_000_000 date_upload 1_759_000_000_000 read false; C2 date_upload 0 (E-U1); C3 date_fetch 1_600_000_000_000 (E-U2, before date_added? set date_added later than date_fetch to exclude).
  - M2 favorite 0 with a recent chapter (E-U3).
  - M3 favorite 1, status 1, next_update = local midnight today + 3 h; M4 status 2 (COMPLETED) next_update future (E-C3); M5 next_update = yesterday 23:59 local (E-C1); M6 next_update year 2100 (E-C2); M7 favorite 0 with future next_update (E-C5).
  - A1 anime status 7 next_update future (E-C4).
- Fixture times use a fixed `now` injected into the code (the window and start-of-day functions take parameters, so tests do not depend on the wall clock).

## 8. Acceptance for the port (summary)

- Every unit and integration case above passes with `cargo test`.
- The UI smoke tests pass under `xvfb-run` on CI.
- Manual checklist complete on KDE Plasma Wayland and X11.
