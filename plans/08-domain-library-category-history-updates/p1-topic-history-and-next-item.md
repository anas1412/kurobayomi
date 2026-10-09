# Area 08 — Topic: history recording, history list, "next chapter/episode"

Sources: `domain/src/main/java/tachiyomi/domain/history/**` (17 files), `data/src/main/sqldelight/data/history.sq`, `data/src/main/sqldelight/view/historyView.sq`, `data/src/main/sqldelightanime/dataanime/animehistory.sq`, `data/src/main/sqldelightanime/view/animehistoryView.sq`, `app/src/main/java/eu/kanade/tachiyomi/ui/history/**`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt`.

## 1. Data semantics in one table

| Aspect | Manga (`history`) | Anime (`animehistory`) |
|---|---|---|
| Key | one row per chapter (`chapter_id UNIQUE`) | one row per episode (`episode_id UNIQUE`) |
| Timestamp column | `last_read` (epoch ms, `AS Date`) | `last_seen` |
| Duration | `time_read` ms, accumulated by `+ :time_read` on every upsert | none |
| Upsert effect | `last_read = now`, `time_read += session` | `last_seen = now` |
| Soft reset | `last_read = 0` (row and `time_read` stay) | `last_seen = 0` |
| Hard clear | `DELETE FROM history` (duration lost) | `DELETE FROM animehistory` |
| Cascade | chapter deleted -> row deleted (FK) | episode deleted -> row deleted |
| Visible in list when | `last_read > 0` and the chapter is its manga's most recent (max `last_read`) | `last_seen > 0` and latest per anime |

Consequence for statistics: the stats screen's "total read duration" keeps counting time of soft-reset rows but forgets everything after "Clear history". Rust keeps both behaviours.

## 2. Worked example of the list query

History rows for manga M (chapters c1..c3) and N (n1):

| row | chapter | last_read |
|---|---|---|
| 1 | M.c1 | 1000 |
| 2 | M.c2 | 3000 |
| 3 | M.c3 | 0 (reset) |
| 4 | N.n1 | 2000 |

`max_last_read`: M -> (c2, 3000), N -> (n1, 2000). The list shows two rows ordered by `readAt DESC`: `M.c2 (3000)`, `N.n1 (2000)`. Remove (reset) row 2: M's max becomes `c1` (1000) -> list: `N.n1 (2000)`, `M.c1 (1000)`. Reset row 1 as well: M has max 0 for all rows, `readAt > 0` fails -> M disappears. "Reset all chapters for this manga" (checkbox) resets rows 1-3 at once.

Search `query = "n"` matches titles containing `n` case-insensitively (ASCII). `%` and `_` inside the query act as LIKE wildcards upstream (e.g. typing `100%` matches everything starting with `100`); Proposal: escape `\`, `%`, `_` and add `ESCAPE '\'`.

## 3. Recording policy (Proposal for Kurobayomi, mapped to upstream behaviour)

| Event | Upstream behaviour | Kurobayomi reader/player must |
|---|---|---|
| Reader chapter becomes active (open or next/prev chapter) | `flushReadTimer()` for the previous chapter (upsert with elapsed time), `restartReadTimer()` | on chapter change: `history.record(prev_chapter, now, elapsed)` then start a new timer |
| Reader window loses focus/minimised/closed (`onPause`) | `flushReadTimer()` | on `notify::is-active` false, on close: flush |
| Reader window regains focus (`onResume`) | `restartReadTimer()` | restart timer |
| Incognito on | no upsert | no upsert (also for the timer: nothing to flush) |
| Anime playback, each second | `saveWatchingProgress` -> history upsert + episode progress write each second | Proposal: upsert at most every 5 s while playing plus on pause/seek-end/episode change/stop/close (T-08a-019) |
| External player returns | `ExternalIntents.saveEpisodeHistory` upsert with `Date()` | not applicable on Linux unless an external player is configured (area:45) |

Elapsed time rule for manga: `elapsed = now - chapterReadStartTime` or `0` when the start time is null; after the upsert the start time is set to null (so repeated flushes without a restart add 0 ms but still bump `last_read`). A clock that goes backwards produces a negative `time_read` delta upstream (no guard). Proposal: clamp `elapsed` to `max(0, ...)` and to a sanity ceiling of 12 h per session (prevents a suspended laptop from adding days).

## 4. Next chapter / next episode algorithms with examples

`GetNextChapters` always asks `getChapterSort(manga, sortDescending = false)`, i.e. the **ascending reading order for the manga's configured sort type** (`SOURCE`: descending source order index, because source order 0 is the newest chapter; `NUMBER`: chapter number ascending; `UPLOAD_DATE`: upload date ascending; `ALPHABET`: name ascending with the collator).

Example chapters in ascending reading order with read flags: `c1 read, c2 read, c3 unread, c4 unread, c5 read, c6 unread`.

| Call | Result |
|---|---|
| `await(mangaId)` (onlyUnread = true) | `[c3, c4, c6]` |
| `await(mangaId, onlyUnread = false)` | `[c1, c2, c3, c4, c5, c6]` |
| `await(mangaId, fromChapterId = c4, onlyUnread = true)` | list `[c3, c4, c6]`, index of c4 = 1 -> `[c4, c6]` |
| `await(mangaId, fromChapterId = c2, onlyUnread = true)` | `c2` not in the list -> index -1 -> `max(0, -1) = 0` -> `[c3, c4, c6]` |
| `await(mangaId, fromChapterId = c4, onlyUnread = false)` | index 3 -> `subList(3, 6) = [c4, c5, c6]`; `fromChapter = c4` unread -> return `[c4, c5, c6]` (resume c4) |
| `await(mangaId, fromChapterId = c2, onlyUnread = false)` | index 1 -> `[c2..c6]`; `c2` is read -> `drop(1)` -> `[c3, c4, c5, c6]` (first = c3, the one after c2) |
| `await(mangaId, fromChapterId = <unknown>, onlyUnread = false)` | index -1 -> start 0 -> whole list; `fromChapter = null` -> `drop(1)` -> `[c2..c6]` (edge: skips c1) |
| `await(onlyUnread = false)` with last history entry on c2 | `GetNextChapters.await(manga, c2, false)` as above -> history screen's "resume" opens `first()` = c3 |

Resume button in the history list opens `getNextChapters.await(mangaId, chapterId, onlyUnread = false).firstOrNull()`: so a half-read chapter (unread flag still false because the "read" flag is only set at the last page) reopens at its saved page, and a finished chapter opens the following one. `null` (end of series) -> snackbar "Next chapter not found" (`no_next_chapter`) / "Next Episode not found!" (`no_next_episode`).

Anime differences: no scanlator filter in the repository call; `Episode.seen`; `getEpisodeSort` masks differ; otherwise identical.

Download-ahead use (reader/player): `GetNextChapters.await(manga.id, nextChapter.id!!)` (onlyUnread = true) then optional duplicate removal (`readerPreferences.skipDupe`), `take(downloadAheadAmount)`.

## 5. Date separators (shared by history and updates)

Algorithm (`insertSeparators`): iterate pairs `(before, after)` over the item list with virtual `null` at both ends; emit `Header(afterDate)` before `after` when `afterDate != null && beforeDate != afterDate` where dates are `LocalDate` in the system zone of each item's timestamp (`readAt` for history, `dateFetch` for updates). `readAt == null` (possible only for soft-reset rows, which the query excludes) would give no header.

Header label (UI layer, `relativeDateText`): if `relative_time_v2` is true: difference in days `d = today - date`: `d < -7` -> absolute date; `-7 <= d < 0` -> plural `upcoming_relative_time`; `d < 1` (i.e. today or future-within) -> "Today"; `1 <= d < 7` -> plural `relative_time` ("Yesterday" for 1, "%d days ago" for 2..6); else absolute `DateTimeFormatter.ofLocalizedDate(SHORT)` or the user's `app_date_format` pattern. If `relative_time_v2` is false: always the absolute date.

Rust (`time` crate): `local_date(ms) = OffsetDateTime::from_unix_timestamp_nanos(ms*1_000_000).to_offset(local_offset).date()`; compute `d` with `(today - date).whole_days()`.
