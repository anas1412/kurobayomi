# Topic — statistics and storage: how the numbers are produced

Deep dive for area 42 part 1. Algorithms are in p1-behavior.md sections 5 and 6; this file records the aggregation rules, their sources, and where the Kotlin is deliberately kept versus improved.

## 1. Shared population: the library

Both pages start from the library, not from all entries:
- Stats: `GetLibraryManga.await()` / `GetLibraryAnime.await()` (one row per entry and category), then `distinctBy { id }`.
- Storage: `GetLibraryManga.subscribe()` / `GetLibraryAnime.subscribe()`, then `distinctBy { id }`, then the category filter.

Exception: manga read duration is `SUM(history.time_read)` over every history row, not over the library (EC-42-S-01). Anime watch time is over library anime (EC-42-S-03). Download counts are over the whole download cache (EC-42-S-12). Engineers must keep these three scopes as they are, or change them deliberately with a test.

## 2. Counting rules

| Metric | Rule | Notes |
|---|---|---|
| In library | number of distinct library entries | |
| Completed | status == 2 AND unread (or unseen) == 0 | an empty entry with status 2 is completed (EC-42-S-04) |
| Started | read/seen count > 0 | |
| Local | source is the local source | area:14-source-local-and-metadata decides `isLocal()` |
| Chapters/Episodes total | sum of `totalChapters` / `totalCount` over distinct library rows | `Int` in Kotlin (`toInt()`) |
| Chapters/Episodes read | sum of `readCount` / `seenCount` | |
| Downloaded | download cache total (all sources) | includes orphan directories |
| Tracked entries | distinct library entries with at least one logged-in tracker link | scored or not |
| Mean score | mean over entries of (mean of the entry's scored tracks' 10-point scores) | tracks with score `<= 0` ignored |
| Trackers used | number of logged-in trackers of the matching kind | |

Mean-of-means is intentional: an entry tracked on three services weighs the same as an entry tracked on one (EC-42-S-08).

## 3. Global update count

This is the number of entries the automatic library update would refresh, approximated by the stats code:
- include list: entries whose category is in `*_update_categories`; empty = all;
- exclude list: entries whose category is in `*_update_categories_exclude`;
- restrictions (`autoUpdateItemRestrictions`): NON_COMPLETED (status 2 excluded), HAS_UNVIEWED (entries with `unreadCount != 0` are excluded), NON_VIEWED (`totalChapters > 0 && !started` excluded);
- OUTSIDE_RELEASE_PERIOD is NOT evaluated here (EC-42-S-07). The updater (area:26-library-update-jobs) evaluates it, so the stats number can be larger than the real update set.

## 4. Durations

- Manga read: `time_read` sum in ms; displayed by `duration_string` (behavior 5.4).
- Anime watched: per library episode, `total_ms` if seen else `last_ms` (ms, column names are misleading, EC-42-U-16).
- Display rules: `1d 1h` for 25 hours, `1h` for 1 h 0 m 5 s, `None` for zero (EC-42-S-11).

## 5. Storage sizes

- Per entry: download cache size (sum of its directories) or the local folder size for local entries (area:23-downloads-manga, area:24-downloads-anime, area:14-source-local-and-metadata).
- Chart: half ring, arcs proportional to size, sorted by size descending, colours per entry (deterministic from the id).
- Text: decimal units (1000), two decimals, `f32` arithmetic. `999_999 bytes` displays as `1000.00 KB` (EC-42-T-10). Keep it; add a test so it is intentional.
- Storage has no empty state (EC-42-T-03).

## 6. Freshness and cost

- Stats are a one-time snapshot per page open (no subscription). The Kotlin issues N+1 queries (anime watch time per entry; tracks per entry). Rust uses one aggregate query per kind (rust design 4.4), with a parity test against the N+1 reference during migration.
- Storage recomputes on every combined emission and calls the cache for each row. Rust computes once per emission and caches sizes by the cache generation (EC-42-T-12, T-13).

## 7. Failure behavior

- Kotlin: no error handling; an exception leaves the page on Loading forever (EC-42-S-06).
- Rust: each section is computed independently; a failure shows "N/A" for that section and logs the error.

## 8. Where the numbers are checked

- Unit tests (pure): `stats::mean_of_entry_means_example`, `stats::mean_zero_scores_excluded`, `stats::mean_none_when_empty`, `stats::global_update_include_exclude_and_restrictions`, `stats::completed_empty_entry_counts`, `storage::size_format_vectors`, `storage::build_snapshot_sort_is_stable`, `storage::build_snapshot_all_drops_hidden_category`, `storage::donut_*` (p1-edge-cases-and-tests.md section 5.1).
- DB tests: `stats::anime_watch_time_uses_library_predicate`, `stats::manga_read_duration_sums_all_history`, `storage::library_rows_dedupe_across_categories` (section 5.2).
- Expected display values for the fixture: section 6.3 of p1-edge-cases-and-tests.md.

## 9. Tasks

- Stats: T-42a-039, T-42a-040, T-42a-041, T-42a-042, T-42a-043, T-42a-044, T-42a-045.
- Storage: T-42a-046, T-42a-047, T-42a-048, T-42a-049, T-42a-050, T-42a-051, T-42a-052, T-42a-053, T-42a-054.
- Shared sizes and formats: T-42a-003, T-42a-004.
