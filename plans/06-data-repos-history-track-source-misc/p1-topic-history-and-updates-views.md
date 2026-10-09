# Topic: History and Updates queries (views, ordering, search, worked examples)

Sources: `data/src/main/sqldelight/view/historyView.sq`, `data/src/main/sqldelightanime/view/animehistoryView.sq`, `data/src/main/sqldelight/view/updatesView.sq`, `data/src/main/sqldelightanime/view/animeupdatesView.sq`, `data/src/main/sqldelight/data/history.sq`, `data/src/main/sqldelightanime/dataanime/animehistory.sq`, repositories `MangaHistoryRepositoryImpl.kt`, `AnimeHistoryRepositoryImpl.kt`, `MangaUpdatesRepositoryImpl.kt`, `AnimeUpdatesRepositoryImpl.kt`. The views themselves are created by the schema (area:03, area:04); this area owns the list queries and their Rust implementation.

## 1. How `historyView` + `history` select rows

`historyView` has one row per **history row** (joined to its chapter and manga) and attaches, to every row, the identity of the chapter with the greatest `last_read` of the same manga (`maxReadAtChapterId`). SQLite rule (SQLite documentation, "Bare columns in an aggregate query"): when `MAX()` is the only aggregate, bare columns in the same SELECT (`chapters._id AS chapter_id` in the subquery) take their value from the row that holds the maximum. The `history` query keeps only the row where `maxReadAtChapterId = chapterId`, so each manga appears **at most once**, represented by its most recently read chapter, and only if that chapter's `last_read > 0`.

Consequences (tested in section 6):

| Situation | Result |
|---|---|
| manga M, chapters c1 read at 100, c2 read at 300 | one list row: c2 (readAt 300) |
| user removes that list row (`resetMangaHistory(history.id)` -> `last_read = 0` on c2) | the list now shows c1 (readAt 100), because the max over {100, 0} is c1. Removing "this chapter entry" reveals the previous chapter of the same manga; to hide the manga completely use "remove all for this manga" (`resetHistoryByMangaId`) |
| all history rows of M reset to 0 | M disappears (max = 0 -> row filtered by `readAt > 0`) but the rows (and their `time_read`) remain |
| two chapters with identical `last_read` (possible when two upserts get the same millisecond) | SQLite picks one of the tied rows as "the max"; exactly one row is listed; which one is unspecified (in practice the first scanned). Proposal: none needed |
| chapter deleted (`DELETE FROM chapters`, e.g. after a source drops it and library sync removes it) | `ON DELETE CASCADE` removes its history row and `time_read` with it; the list falls back to the previous chapter |
| manga removed from the library but not deleted from the DB | still listed (the view does not filter on `favorite`); `favorite` is exposed so the UI can offer "add to library" |

List ordering: `ORDER BY readAt DESC`. Ties are listed in an unspecified order; Proposal: add `, id DESC` as a secondary key (strictly refines the Kotlin ordering, prevents flicker when two entries share a timestamp).

`getLatestHistory` does **not** apply the `maxReadAtChapterId` filter: it takes the single row with the greatest `readAt > 0` across all history; the result equals the first row of the list.

## 2. Rust implementation (module `kuro_core::data::history`)

```rust
const HISTORY_LIST_SQL: &str = "
SELECT id, mangaId, chapterId, title, thumbnailUrl, source, favorite, cover_last_modified,
       chapterNumber, readAt, readDuration
FROM historyView
WHERE historyView.readAt > 0
  AND maxReadAtChapterId = historyView.chapterId
  AND lower(historyView.title) LIKE ('%' || ?1 || '%')
ORDER BY readAt DESC, id DESC";

const HISTORY_LATEST_SQL: &str = "
SELECT id, mangaId, chapterId, title, thumbnailUrl, source, favorite, cover_last_modified,
       chapterNumber, readAt, readDuration
FROM historyView WHERE historyView.readAt > 0 ORDER BY readAt DESC LIMIT 1";

fn map_with_relations(r: &Row) -> rusqlite::Result<MangaHistoryWithRelations> {
    Ok(MangaHistoryWithRelations {
        id: r.get(0)?, manga_id: r.get(1)?, chapter_id: r.get(2)?, title: r.get(3)?,
        cover: MangaCover { manga_id: r.get(1)?, source_id: r.get(5)?, is_manga_favorite: r.get(6)?,
                            url: r.get(4)?, last_modified: r.get(7)? },
        chapter_number: r.get(8)?, read_at: r.get(9)?, read_duration: r.get(10)?,
    })
}
```

Tables for `watch`: `TableSet::of(&[Table::History, Table::Chapters, Table::Mangas])` (anime: `AnimeHistory, Episodes, Animes`). `favorite` is read as `bool` (INTEGER 0/1: rusqlite converts `i64` to `bool` with `FromSql` for bool — nonzero is true).

`history_by_manga(c, manga_id)` implements `getHistoryByMangaId` (join, includes reset rows). `reset_by_id`, `reset_by_manga`, `delete_all` (`DELETE FROM history WHERE 1`), `upsert` (SQL from `p1-data-and-config.md` 3.1; `time_read = time_read + ?3` and `last_read = ?2` bound by position, `?3` reused through numbered parameters), `total_read_duration` (`SELECT coalesce(sum(time_read), 0) FROM history`).

Search text: parity passes the raw text. SQLite's `LIKE` is case-insensitive for ASCII only; `lower()` is ASCII-only unless the ICU extension is loaded, so "é" vs "É" do not match. `%` and `_` typed by the user act as wildcards (Kotlin does not escape). Proposal (flagged improvement, task T-06a-012 acceptance item, default ON because it only broadens/fixes matches): register a scalar function `kuro_fold(text)` (`str::to_lowercase`, deterministic) on every reader connection (`rusqlite` feature `functions`), use `kuro_fold(title) LIKE '%' || kuro_fold(?1) || '%' ESCAPE '\'` and escape `\`, `%`, `_` in the user text. Behaviour is identical to Kotlin for plain ASCII text without wildcards.

Error handling parity: `reset_*`, `delete_all`, `upsert` return `()`/`bool` and log failures (`tracing::error!`); `history_by_manga`, `latest`, `total_read_duration`, list watch return `Result` (Kotlin lets these propagate).

## 3. Upsert accumulation, worked example

Rows start empty. Reader session A on chapter 10 from t=1000 to t=61000 (60 s) -> `upsert(10, readAt=61000, dur=60000)`: row (chapter 10, last_read 61000, time_read 60000). Session B on chapter 10 for 30 s ending t=200000 -> `time_read = 90000`, `last_read = 200000`. User resets chapter 10 -> `last_read = 0`, `time_read` stays 90000; stats total includes it. User reads chapter 10 again for 5 s ending t=300000 -> `last_read = 300000`, `time_read = 95000`. Anime: only `last_seen` changes, there is no duration.

## 4. Updates view and queries

```sql
CREATE VIEW updatesView AS
SELECT mangas._id AS mangaId, mangas.title AS mangaTitle, chapters._id AS chapterId, chapters.name AS chapterName,
       chapters.scanlator, chapters.read, chapters.bookmark, chapters.last_page_read, mangas.source, mangas.favorite,
       mangas.thumbnail_url AS thumbnailUrl, mangas.cover_last_modified AS coverLastModified,
       chapters.date_upload AS dateUpload, chapters.date_fetch AS datefetch
FROM mangas JOIN chapters ON mangas._id = chapters.manga_id
WHERE favorite = 1 AND date_fetch > date_added
ORDER BY date_fetch DESC;

getRecentUpdates:       SELECT * FROM updatesView WHERE dateUpload > :after LIMIT :limit;
getUpdatesByReadStatus: SELECT * FROM updatesView WHERE read = :read AND dateUpload > :after LIMIT :limit;
```

Anime view `animeupdatesView` exposes `animeId, animeTitle, episodeId, episodeName, scanlator, seen, bookmark, fillermark, last_second_seen, totalSeconds, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch`; queries `getRecentAnimeUpdates` and `getUpdatesBySeenStatus` (`seen = :seen`).

Rust queries (explicit order; same column list as `SELECT *` so the positional mapper of `p1-data-and-config.md` 1.2 applies unchanged):

```sql
SELECT * FROM updatesView WHERE dateUpload > ?1 ORDER BY datefetch DESC, chapterId DESC LIMIT ?2
SELECT * FROM updatesView WHERE read = ?1 AND dateUpload > ?2 ORDER BY datefetch DESC, chapterId DESC LIMIT ?3
```

Parameters: `after` = `now - 3 months` in ms computed by the caller (`chrono::Local::now() - Months::new(3)`, calendar months, not 90 days), `limit` = 500 (`pub const UPDATES_LIMIT: i64 = 500`).

Semantics worth stating for the UI (area:42):
* Because the window is on `dateUpload`, a chapter with `date_upload = 0` never shows; a chapter whose source reports an old upload date (back-catalogue added later) never shows either, even though `date_fetch` is new. Parity decision: keep.
* `date_fetch > date_added`: when a series is added to the library, its existing chapters (they were fetched when the details screen first synced the chapter list, which normally happens before the user favourites the series, so `date_fetch <= date_added`) are excluded; chapters discovered by later library updates are included. If a user un-favourites and re-favourites, `date_added` moves forward and older chapters vanish from Updates.
* `date_fetch` values inside one sync are `now + n` for the first new chapter in source order, then one millisecond less for each following one, down to `now + 1` for the last (`SyncChaptersWithSource.kt:178-185`), which is why `ORDER BY date_fetch DESC` reproduces the source's order inside one batch. The extra `chapterId DESC` key only disambiguates identical values.
* The UI groups by local date of `dateFetch` (`MangaUpdatesScreenModel`, `toLocalDate()`), shows a "last updated" line from preference `__APP_STATE_library_update_last_timestamp`, and refreshes download state by combining with the download cache change flow (area:23/24).

`watch` tables: manga `{Mangas, Chapters}`; anime `{Animes, Episodes}`.

## 5. Interaction with deletion and sync

* Library sync that re-inserts a chapter under a new `_id` loses its history (cascade). `SyncChaptersWithSource` preserves `date_fetch` of removed-then-readded chapters, but history rows are not migrated in Kotlin either (area:10).
* Updating `chapters.read` through the reader triggers `Chapters` notifications, so the Updates list re-runs and the item turns "read" instantly.

## 6. Tests to write (fixtures in `p1-edge-cases-and-tests.md` section 3)

1. History latest-per-manga with ties, resets and cascade deletes (table in section 1).
2. `upsert` accumulation (section 3 worked example) with exact numbers.
3. `delete_all` fires a `History` notification (truncate-optimisation guard).
4. Updates window: rows with `date_upload` 0, below/above `after`, `date_fetch == date_added`, unfavourite/refavourite.
5. Search: ASCII case-insensitive, wildcard literal (`100%`), non-ASCII fold (`Éclair` found by `éclair`), empty string returns everything.
