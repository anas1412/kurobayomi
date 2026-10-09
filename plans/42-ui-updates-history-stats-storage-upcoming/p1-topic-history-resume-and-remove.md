# Topic — history: what a row means, resume, remove, clear

Deep dive for area 42 part 1. Full behavior is in p1-behavior.md sections 1.1–1.12; this file explains the model so an engineer can reason about edge cases without reopening Kotlin.

## 1. What a history row is

- A history row is the record "the user read/watched this unit at time T" (`history.last_read` / `animehistory.last_seen`), one per unit (`UNIQUE(chapter_id)` / `UNIQUE(episode_id)`).
- The list shows only ONE row per entry: the row whose timestamp equals the entry's maximum (`maxReadAtChapterId = chapterId`). So a manga read through chapters 1–40 appears once, as chapter 40 (or whichever unit was read last).
- Rows with timestamp 0 are "reset" and are invisible (`readAt > 0`). A reset is how "remove" works; rows are never deleted by the per-row remove.
- Manga rows also carry `time_read` (milliseconds accumulated by the reader; `upsert` adds `:time_read`). Anime rows carry no duration.

Consequences (OBSERVED):

| Action | Visible list | `history` row | `time_read` | Stats effect |
|---|---|---|---|---|
| Read a unit | row appears/moves to top | upsert `last_read = now`, `time_read += session` | accumulates | manga read duration grows |
| Remove one (confirm without checkbox) | row disappears | `last_read = 0` (kept) | kept | none |
| Remove entry (checkbox) | all rows of that entry disappear | all `last_read = 0` (kept) | kept | none |
| Clear all | list empty | `DELETE` (gone) | gone | manga read duration falls to 0 |

## 2. Date grouping

- Rows are ordered `readAt DESC`. Headers are inserted when the LOCAL calendar date of the row differs from the previous row (`insertSeparators`, `MangaHistoryScreenModel.kt:93-104`).
- Header text: `relativeDateText(date)`: Today / N days ago / upcoming / absolute, governed by `relative_time_v2` and `app_date_format` (p1-behavior.md 1.6).
- Row subtitle shows time only (`toTimestampString`), so the date is only in the header.

## 3. Resume (open the next unit)

Resume is a function of (entry, unit), not of the row:

1. Load the entry's units, sorted by the entry's own unit sort (area:07-domain-entries-items), scanlator-filtered.
2. Find the unit id of the history row. Let `tail` = units from that point.
3. If the unit is unread (or, for anime, unseen): open it. If it is read: open the one after it.
4. If nothing is left: snackbar "Next chapter not found" / "Next Episode not found!".

"Resume last" (reselect on the History tab) runs the same algorithm for the newest row overall (`getLastMangaHistory`, not restricted to the per-entry max).

Known defects, kept or fixed (see EC-42-H-05 to H-08 in p1-edge-cases-and-tests.md):
- If the history unit is missing from the sorted list, Aniyomi opens the second unit (`drop(1)` of the whole list). Fix: first unit.
- Reselect on the History tab always resumes the ANIME list (the manga resume channel is never sent to). Fix: resume the visible sub-tab.

## 4. Remove

- Per-row remove and per-entry remove are the same reset, differing only in scope.
- The dialog's checkbox is the only switch between them (`removeEverything`, default false).
- Failures are swallowed in Aniyomi; Kurobayomi shows a toast (rust design section 5).

## 5. Clear all

- Hard delete: `DELETE FROM history` / `DELETE FROM animehistory`. This also drops `time_read`. The confirm text ("All history will be lost.") is the only guard, and it is accurate.
- Success shows "History deleted"; failure is silent in Aniyomi (fix: toast).

## 6. Add to library from history

- Duplicate check, then default-category logic, then (maybe) a change-category dialog, then tracker binding. The favorite flag is set only after the category decision on the change-category path. Full order: p1-behavior.md 1.10.
- Asymmetries kept as observed: the anime "edit categories" path does not switch to the anime category list; the change-category confirm path does not bind trackers; the migrate dialog replaces the stack with the NEW manga but the OLD anime (EC-42-H-17).

## 7. Search

- Substring match on the title. Wildcards in user input are not escaped in Aniyomi (EC-42-H-02); Kurobayomi uses `instr`.
- Search is per sub-tab. The empty-state text switches to "No results found" when a query is non-empty (EC-42-H-26).

## 8. Implementation checklist (maps to tasks)

- Queries: T-42a-011, T-42a-012.
- Model and rows: T-42a-013, T-42a-014, T-42a-005, T-42a-022.
- Resume: T-42a-015, T-42a-016, T-42a-021.
- Remove and clear: T-42a-017, T-42a-018, T-42a-024.
- Favorite and migrate: T-42a-019, T-42a-020.
- Search: T-42a-023.
- Tests: T-42a-060 (sections 5.1–5.4 of p1-edge-cases-and-tests.md).
