# Area 06 — Behaviour specification

OBSERVED = what the Kotlin does (with `path:line`). Proposal = what the Rust app should do. Paths are repo-relative to `/home/blackbox/aniyomi-src`. Where a statement comes from a library that is not in the repository (SQLDelight, AndroidX Paging, kotlinx.coroutines, OkHttp, kotlinx.serialization, SQLite itself) it is marked "library behaviour".

Abbreviations used for paths (all under `data/src/main/java/` unless stated):
`MHR` = `tachiyomi/data/history/manga/MangaHistoryRepositoryImpl.kt`; `AHR` = `tachiyomi/data/history/anime/AnimeHistoryRepositoryImpl.kt`; `MUR` = `tachiyomi/data/updates/manga/MangaUpdatesRepositoryImpl.kt`; `AUR` = `tachiyomi/data/updates/anime/AnimeUpdatesRepositoryImpl.kt`; `MTR` = `tachiyomi/data/track/manga/MangaTrackRepositoryImpl.kt`; `ATR` = `tachiyomi/data/track/anime/AnimeTrackRepositoryImpl.kt`; `MSR` = `tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt`; `ASR` = `tachiyomi/data/source/anime/AnimeSourceRepositoryImpl.kt`; `MSP` = `tachiyomi/data/source/manga/MangaSourcePagingSource.kt`; `ASP` = `tachiyomi/data/source/anime/AnimeSourcePagingSource.kt`; `ESR` = `mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt`; `ESS` = `mihon/data/extension/anime/service/AnimeExtensionStoreService.kt`; `NES` = `mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt`; `NLE` = `mihon/data/extension/anime/model/NetworkLegacyAnimeExtension.kt`; `NLR` = `mihon/data/extension/anime/model/NetworkLegacyAnimeExtensionRepo.kt`; `MER` = `mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt`; `CBR` = `tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt`; `RSI` = `tachiyomi/data/release/ReleaseServiceImpl.kt`; `AMH` = `tachiyomi/data/handlers/manga/AndroidMangaDatabaseHandler.kt`; `AAH` = `tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt`. SQL files are under `data/src/main/sqldelight/` (manga DB) and `data/src/main/sqldelightanime/` (anime DB).

Deep dives referenced from here: `p1-topic-db-handle-and-reactive-queries.md` (handler replacement), `p1-topic-history-and-updates-views.md` (the two SQL views), `p1-topic-extension-store-index.md` (store wire formats and algorithms), `p1-topic-source-paging-and-stubs.md`, `p1-topic-custom-buttons.md`, `p1-topic-release-check.md`.

## 1. Database handlers (OBSERVED)

### 1.1 What the handlers are

`AMH:17-22` and `AAH:17-22` are the only way repositories touch SQL. Constructor: `(db, driver, queryDispatcher = Dispatchers.IO, transactionDispatcher = queryDispatcher)`. They are built in `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt` (singletons `MangaDatabaseHandler`, `AnimeDatabaseHandler`). The two databases are separate files with separate generated schemas (`tachiyomi.data.Database` for manga, `tachiyomi.mi.data.AnimeDatabase` for anime); they never share a transaction.

### 1.2 Method semantics

| Method | Behaviour (AMH / AAH line) | Rust equivalent |
|---|---|---|
| `await(inTransaction=false, block)` | `dispatch(inTransaction, block)` (`AMH:26-28`). Runs a suspending block with the database as receiver and returns its value | `Db::write(|tx| ...)` for anything that writes, `Db::read(|conn| ...)` for reads. Kotlin does not distinguish; Rust does (reads use the reader pool) |
| `awaitList(block)` | `block(db).executeAsList()` (`AMH:30-35`) | `read(|c| query_map(...).collect())` |
| `awaitOne(block)` | `executeAsOne()` — throws `NullPointerException`/`IllegalStateException` when 0 rows and `IllegalStateException` when more than one (library behaviour) (`AMH:37-42`) | `query_row` -> `DbError::NotFound` when 0 rows; extra rows are ignored by `query_row` (Proposal: use `query_row` and document, because every call site uses a single-row aggregate) |
| `awaitOneExecutable` | Same for `ExecutableQuery` (INSERT ... then `SELECT last_insert_rowid()`) (`AMH:44-49`); used by `CustomButtonRestorer` with `selectLastInsertedRowId` | `tx.last_insert_rowid()` |
| `awaitOneOrNull`, `awaitOneOrNullExecutable` | `executeAsOneOrNull()` (`AMH:51-63`) | `query_row(...).optional()` |
| `subscribeToList/One/OneOrNull` | `block(db).asFlow().mapToList(queryDispatcher)`, `.mapToOne(queryDispatcher)`, `.mapToOneOrNull(queryDispatcher)` (`AMH:65-75`). Library behaviour: the flow emits the current result immediately on collection, then re-runs the query and emits again whenever SQLDelight notifies a table the query reads. `mapToOne` throws if the re-run returns no row; `mapToOneOrNull` emits null | `Db::watch(tables, f)` (topic file) |
| `subscribeToPagingSource(countQuery, queryProvider)` | Returns a `QueryPagingMangaSource` (`AMH:77-88`) | Dropped (task T-06a-006): never called anywhere else in the repository |
| `dispatch(inTransaction, block)` | If `inTransaction`: `withMangaTransaction { block(db) }`. Else if `driver.currentTransaction() != null` (we are already on the transaction thread): run `block(db)` directly. Else `withContext(getCurrentMangaDatabaseContext())` where the context is the transaction dispatcher if the coroutine carries a `TransactionElement`, otherwise `queryDispatcher` (`AMH:90-104`) | Every `write` is one IMMEDIATE transaction on a blocking-pool thread; no context inheritance exists |

### 1.3 Transaction machinery

`MangaTransactionContext.kt` and `AnimeTransactionContext.kt` (same code, names differ: `TransactionElement` vs `AnimeTransactionElement`):

1. `withMangaTransaction` (`:38-55`): reuse the inherited transaction dispatcher if the coroutine already has the element (allows nested `await(inTransaction = true)` from inside a transaction block); otherwise `createTransactionContext()`.
2. `createTransactionContext` (`:75-90`): creates a `Job` (`controlJob`), registers `coroutineContext[Job]?.invokeOnCompletion { controlJob.cancel() }` so a cancelled caller releases the thread, then `acquireTransactionThread` (`:97-125`) dispatches a `runBlocking { continuation.resume(interceptor); controlJob.join() }` onto `transactionDispatcher` (IO) to *take over one IO thread* and use its `ContinuationInterceptor` as the transaction dispatcher. A `RejectedExecutionException` becomes `IllegalStateException("Unable to acquire a thread to perform the database transaction", ex)`.
3. Inside `withContext(transactionContext)`: `transactionElement.acquire()` (ref-count +1), `db.transactionWithResult { runBlocking(transactionContext) { block() } }`, `finally { transactionElement.release() }`. `release` (`:152-160`) decrements; below 0 throws `IllegalStateException("Transaction was never started or was already released")`; at 0 cancels `controlJob`, freeing the thread.
4. SQLDelight `transactionWithResult` (library behaviour): `BEGIN` ... commit when the lambda returns, rollback when it throws; table-change notifications for queries are deferred until commit. Only one transaction runs at a time on a connection; others queue first-come first-served (stated in the KDoc at `:29-30`).
5. Cancellation (library behaviour): a cancelled coroutine inside the transaction block rolls the transaction back (the KDoc at `:25-27` says "marked as successful unless an exception is thrown ... or the coroutine is cancelled").

Proposal: all of section 1.3 is replaced by "one closure, one thread, one `rusqlite::Transaction`" (`p1-topic-db-handle-and-reactive-queries.md` section 3). Rollback on `Err` and on panic (the `Transaction` guard drops). Because the closure is synchronous, nested "transaction inside transaction" is expressed by plain function calls taking `&Connection`; a nested `Db::write` from inside a write closure is a programming error and returns `DbError::NestedWrite` (thread-local guard).

### 1.4 Connection settings (OBSERVED in `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:78-155`)

Both databases: `PRAGMA foreign_keys = ON`, `PRAGMA journal_mode = WAL`, `PRAGMA synchronous = NORMAL`, set on every open (`onOpen`). The Android build uses the Requery SQLite factory (bundled modern SQLite) except in debug builds on API 30+ (framework SQLite for the inspector). Proposal: rusqlite with feature `bundled` (the app needs `ON CONFLICT ... DO UPDATE` upserts, which need SQLite 3.24+) and the same three PRAGMAs, plus `busy_timeout = 5000` and `PRAGMA temp_store = MEMORY` (not in the source; harmless). The schema and migrations are owned by area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations; this area only requires that `Db::open(path, &dyn Schema)` runs them.

### 1.5 Column adapters used by this area

* `history.last_read` and `animehistory.last_seen`: `DateColumnAdapter` — `java.util.Date` <-> INTEGER epoch milliseconds (`data/src/main/java/tachiyomi/data/DatabaseAdapter.kt:11-14`). NULL decodes to `null`. A "reset" writes the integer 0 (not NULL), which decodes to `Date(0)`.
* Boolean columns declared `INTEGER AS Boolean` (`private`, `isFavorite`, `is_legacy`) store 0/1.

## 2. History

### 2.1 Tables and views (OBSERVED)

`history(_id INTEGER PK, chapter_id INTEGER NOT NULL UNIQUE -> chapters(_id) ON DELETE CASCADE, last_read INTEGER (Date), time_read INTEGER NOT NULL)` + index `history_history_chapter_id_index` (`data/src/main/sqldelight/data/history.sq:3-12`).
`animehistory(_id, episode_id UNIQUE -> episodes(_id) ON DELETE CASCADE, last_seen INTEGER (Date))` — no duration column (`data/src/main/sqldelightanime/dataanime/animehistory.sq:3-11`).
The list is read through `historyView` / `animehistoryView`; full SQL in `p1-data-and-config.md` section 3 and semantics in `p1-topic-history-and-updates-views.md`.

### 2.2 Repository operations

| Operation | Manga (MHR) | Anime (AHR) | Behaviour |
|---|---|---|---|
| list with search | `getMangaHistory(query): Flow<List<MangaHistoryWithRelations>>` (`MHR:16-20`) -> `historyViewQueries.history(query, mapper)` | `getAnimeHistory(query)` (`AHR:16-20`) -> `animehistoryViewQueries.animehistory(query, mapper)` | Reactive. Rows = the single most recently read chapter/episode of every entry that has `last_read > 0`, newest first. Title filter: `lower(title) LIKE ('%' || :query || '%')`. The Kotlin passes the query unmodified (`query ?: ""` in `MangaHistoryScreenModel.kt`), so an empty string matches everything |
| latest | `getLastMangaHistory(): MangaHistoryWithRelations?` (`MHR:22-26`) | `getLastAnimeHistory()` (`AHR:22-26`) | `awaitOneOrNull` of `getLatestHistory`: `WHERE readAt > 0 ORDER BY readAt DESC LIMIT 1`. Consumer: `GetNextChapters.await()` / `GetNextEpisodes.await()` (continue-reading button: first element of the unread-or-current chapters after the last history chapter) |
| total duration | `getTotalReadDuration(): Long` (`MHR:28-30`) | none | `SELECT coalesce(sum(time_read), 0) FROM history` — includes rows whose `last_read` was reset to 0 and excludes rows deleted by "remove everything". No try/catch: failure propagates to `MangaStatsScreenModel` |
| by entry | `getHistoryByMangaId(mangaId)` (`MHR:32-34`) | `getHistoryByAnimeId(animeId)` (`AHR:28-35`) | `SELECT H._id, H.chapter_id, H.last_read, H.time_read FROM history H JOIN chapters C ON H.chapter_id = C._id WHERE C.manga_id = :mangaId AND C._id = H.chapter_id` — includes reset rows (readAt = Date(0)). Exceptions propagate |
| reset one | `resetMangaHistory(historyId)` (`MHR:36-42`) | `resetAnimeHistory(historyId)` (`AHR:37-43`) | `UPDATE history SET last_read = 0 WHERE _id = :historyId`. The row stays; `time_read` is kept. Exceptions are caught and logged at ERROR, nothing is returned. This is the "Remove" action on one list item |
| reset entry | `resetHistoryByMangaId` (`MHR:44-50`) | `resetHistoryByAnimeId` (`AHR:45-51`) | `UPDATE history SET last_read = 0 WHERE _id IN (SELECT H._id FROM mangas M INNER JOIN chapters C ON M._id = C.manga_id INNER JOIN history H ON C._id = H.chapter_id WHERE M._id = :mangaId)`. Same error handling. The "Remove all for this entry" action |
| delete all | `deleteAllMangaHistory(): Boolean` (`MHR:52-60`) | `deleteAllAnimeHistory()` (`AHR:53-61`) | `DELETE FROM history` (all rows, including accumulated `time_read`). Returns true on success, false after logging on exception. The screen model sends `HistoryCleared` only when true |
| upsert | `upsertMangaHistory(MangaHistoryUpdate(chapterId, readAt, sessionReadDuration))` (`MHR:62-74`) | `upsertAnimeHistory(AnimeHistoryUpdate(episodeId, seenAt))` (`AHR:63-74`) | See 2.3. Exceptions are caught and logged |

### 2.3 Upsert semantics (history.sq:61-69, animehistory.sq:58-65)

```sql
INSERT INTO history(chapter_id, last_read, time_read)
VALUES (:chapterId, :readAt, :time_read)
ON CONFLICT(chapter_id)
DO UPDATE SET last_read = :readAt, time_read = time_read + :time_read
WHERE chapter_id = :chapterId;
```

* The Kotlin passes `(chapterId, readAt: Date, sessionReadDuration: Long)`. `:time_read` is one bound parameter used twice. First read of a chapter: `time_read = sessionReadDuration`. Later reads: the stored total is increased by the new session's duration and `last_read` is overwritten.
* Anime: `ON CONFLICT(episode_id) DO UPDATE SET last_seen = :seenAt`.
* `readAt` is always non-null and `> 0`, so an upsert after a reset "revives" the row.
* The unit of `time_read` is milliseconds (`ReaderViewModel.kt:597-601`: `readAt.time - chapterReadStartTime`).

### 2.4 Who records history (OBSERVED, consumer side for interaction)

* Manga reader (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt`): `restartReadTimer()` sets `chapterReadStartTime = Instant.now().toEpochMilli()` (`:579-581`); `flushReadTimer()` launches `updateHistory(currentChapter)` non-cancellably (`:583-589`); `updateHistory` returns immediately in incognito mode, otherwise `readAt = Date()`, `sessionReadDuration = chapterReadStartTime?.let { readAt.time - it } ?: 0`, `upsertHistory.await(MangaHistoryUpdate(chapterId, readAt, sessionReadDuration))`, then `chapterReadStartTime = null` (`:595-603`). Triggers: `ReaderActivity.onPause()` -> `flushReadTimer()` (`ReaderActivity.kt:258-260`), `onResume()` -> `restartReadTimer()` (`:266-270`), and when a new chapter loads: `flushReadTimer(); restartReadTimer()` (`ReaderViewModel.kt:346-347`).
* Player (`PlayerViewModel.kt:1788-1799` and `ExternalIntents.kt:435-446`): `saveEpisodeHistory(episode)` — if incognito do nothing, else `upsertHistory.await(AnimeHistoryUpdate(episodeId, Date()))`. No duration.
* Proposal: a `HistoryRecorder` helper (T-06a-014) with the same state machine: `start()`, `flush(chapter_id)`, `incognito` flag read at flush time, duration `max(0, now - start)` (Kotlin can produce a negative value after a clock change; Rust clamps at 0), `start = None` after a flush.

### 2.5 Consumers and display rules that depend on this area's output (for area:42)

* `MangaHistoryScreenModel` (`ui/history/manga/MangaHistoryScreenModel.kt:77-90`): collects `getHistory.subscribe(query ?: "")` with `distinctUntilChanged()`, maps to UI models, `flowOn(Dispatchers.IO)`; on error logs and sends `Event.InternalError` (string `MR.strings.internal_error`).
* Date headers: `insertSeparators` inserts a `Header(date)` between two items whose `readAt.time.toLocalDate()` differ, and before the first item (`:92-102`).
* Actions: single remove -> `removeHistory.await(history)` (reset by id); "remove all for entry" -> `removeHistory.await(mangaId)`; "remove everything" -> `awaitAll()` then `HistoryCleared` event; strings `clear_history_confirmation` "Are you sure? All history will be lost.", `clear_history_completed` "History deleted".
* Resume: `getNextChapterForManga(mangaId, chapterId)` -> `GetNextChapters.await(mangaId, chapterId, onlyUnread = false)`; first element opens (`Event.OpenChapter(chapter?)`; null means "no next chapter", UI toast in area:42).

## 3. Updates

### 3.1 SQL (OBSERVED)

`updatesView` (`data/src/main/sqldelight/view/updatesView.sq:1-21`): `FROM mangas JOIN chapters ON mangas._id = chapters.manga_id WHERE favorite = 1 AND date_fetch > date_added ORDER BY date_fetch DESC`, exposing `mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, last_page_read, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch`. Anime view adds `fillermark`, `last_second_seen`, `totalSeconds` (`animeupdatesView.sq:1-23`).
Queries (`updatesView.sq:23-35`): `getRecentUpdates(after, limit)` = `SELECT * FROM updatesView WHERE dateUpload > :after LIMIT :limit`; `getUpdatesByReadStatus(read, after, limit)` = `... WHERE read = :read AND dateUpload > :after LIMIT :limit`. The outer queries have no `ORDER BY`; the order comes from the view (SQLite keeps a subquery's ORDER BY when it is not flattened away — not guaranteed). Proposal: add `ORDER BY date_fetch DESC` to the Rust queries (identical result, deterministic).

### 3.2 Semantics

1. Only chapters/episodes of favourited entries, fetched after the entry was added to the library (`date_added` is set to `Instant.now().toEpochMilli()` when favouriting and to 0 when un-favouriting: `app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt:97-103`). `date_fetch` is set at sync time to `nowMillis + itemCount--` so that within one fetch the first (most recent) item has the largest value (`SyncChaptersWithSource.kt:178-185`).
2. The time window is on `dateUpload` (the source-reported upload time), not on `date_fetch`: chapters with `date_upload = 0` (unknown) never appear in Updates. The window start `after` is supplied by the caller: `ZonedDateTime.now().minusMonths(3).toInstant().toEpochMilli()` (`MangaUpdatesScreenModel.kt:75-79`).
3. `limit` is 500 (`GetMangaUpdates.kt`, `GetAnimeUpdates.kt`).
4. Row mapping drops `dateUpload` (it is a view column used only for filtering): the mapper receives 14 (manga) / 16 (anime) positional columns (`MUR:41-74`, `AUR:45-82`) and builds `MangaUpdatesWithRelations`/`AnimeUpdatesWithRelations` with `dateFetch` but without `dateUpload`.
5. `awaitWithRead/awaitWithSeen`, `subscribeWithRead/subscribeWithSeen` (the read-status queries) have no caller outside the domain layer (verified by grep over `app/` and `domain/`): they exist for parity and for the library-update notifier design (unread-only listings).
6. Reactive: `subscribeAllMangaUpdates` re-emits when `mangas`, `chapters` change (and for anime: `animes`, `episodes`). The screen models combine it with the download cache/queue to refresh download state.

## 4. Tracks

### 4.1 Tables (OBSERVED)

`manga_sync(_id PK, manga_id NOT NULL -> mangas(_id) ON DELETE CASCADE, sync_id NOT NULL, remote_id NOT NULL, library_id, title NOT NULL, last_chapter_read REAL NOT NULL, total_chapters INTEGER NOT NULL, status INTEGER NOT NULL, score REAL NOT NULL, remote_url TEXT NOT NULL, start_date INTEGER NOT NULL, finish_date INTEGER NOT NULL, private INTEGER DEFAULT 0 NOT NULL, UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE)` (`manga_sync.sq:3-21`); anime twin `anime_sync` with `anime_id`, `last_episode_seen`, `total_episodes` (`anime_sync.sq:3-21`).

### 4.2 Repository operations

| Operation | Behaviour |
|---|---|
| `getTrackByMangaId(id)` / `getTrackByAnimeId(id)` (`MTR:12-14`, `ATR:12-14`) | Despite the name the argument is the track **row id** (`WHERE _id = :id`; queries `getTrackById` / `getTrackByAnimeId`). Returns null when absent |
| `getTracksByMangaId(mangaId)` / `getTracksByAnimeId` | All tracker rows of one entry, no ordering clause |
| `getMangaTracksAsFlow()` / `getAnimeTracksAsFlow()` | Reactive full table (`SELECT * FROM manga_sync`), grouped by entry in `GetTracksPerManga` / `GetTracksPerAnime` (`groupBy { it.mangaId }`) for the library "tracking" filter |
| `getTracksByMangaIdAsFlow(id)` | Reactive per entry (details screen) |
| `delete(mangaId, trackerId)` | `DELETE FROM manga_sync WHERE manga_id = :mangaId AND sync_id = :syncId` |
| `insertManga(track)` / `insertAllManga(tracks)` | `insertValues(vararg)`: inside ONE transaction (`handler.await(inTransaction = true)`), one `INSERT INTO manga_sync(manga_id, sync_id, remote_id, library_id, title, last_chapter_read, total_chapters, status, score, remote_url, start_date, finish_date, private) VALUES (...)` per track. The `id` of the argument is ignored (not inserted). Because of `UNIQUE(manga_id, sync_id) ON CONFLICT REPLACE`, inserting a track for an entry/tracker pair that already exists deletes the old row and creates a new one with a **new `_id`** (rowid = max + 1) — callers that hold the old id (delayed tracking store, `DelayedMangaTrackingStore`) must re-read |
| error handling | The repository lets exceptions propagate; the interactors `InsertMangaTrack`, `DeleteMangaTrack`, `GetMangaTracks` catch every `Exception`, log at ERROR, and return nothing/null/empty list (`domain/src/main/java/tachiyomi/domain/track/manga/interactor/*.kt`) |

`manga_syncQueries.update(...)` (the coalesce update, `manga_sync.sq:45-61`) is not used by the repository; its only callers are the backup restorers (`MangaRestorer.kt:394-410`, `AnimeRestorer.kt:433-`) which pass all 13 values plus `id`. Binding NULL for a parameter keeps the column (coalesce).

### 4.3 Legacy track models (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/{manga,anime}/*Track*.kt`)

* Fields with defaults (`MangaTrackImpl.kt:5-33`): `id: Long? = null`, `manga_id: Long = 0`, `tracker_id: Long = 0`, `remote_id: Long = 0`, `library_id: Long? = null`, `title` (lateinit, no default), `last_chapter_read: Double = 0.0`, `total_chapters: Long = 0`, `score: Double = 0.0`, `status: Long = 0`, `started_reading_date: Long = 0`, `finished_reading_date: Long = 0`, `tracking_url: String = ""`, `private: Boolean = false`. Anime: `anime_id`, `last_episode_seen`, `total_episodes`, `started_watching_date`, `finished_watching_date`.
* `copyPersonalFrom(other, copyRemotePrivate = true)` copies `last_*_read/seen`, `score`, `status`, `started_*_date`, `finished_*_date`, and `private` only when `copyRemotePrivate` is true (`MangaTrack.kt:37-44`). Used by tracker `refresh` flows (area:27).
* `create(serviceId)` returns an impl with `tracker_id = serviceId` and everything else default (`MangaTrack.kt:46-50`).
* Conversions between domain and legacy objects (`app/src/main/java/eu/kanade/domain/track/manga/model/MangaTrack.kt:1-60`): `toDbTrack()` sets `it.id = id`; `remote_url` <-> `tracking_url`; `toDomainTrack(idRequired = true)` returns **null** when `id == null` and `idRequired`, else uses `-1` when `idRequired == false`. Domain `MangaTrack.copyPersonalFrom(other)` copies `lastChapterRead, score, status, startDate, finishDate, private` (always including private).
* Date fields are epoch milliseconds, 0 = unset. `status` holds the tracker-specific numeric status (area:27).

## 5. Legacy chapter/episode models (OBSERVED)

* `Chapter : SChapter, Serializable` adds `id: Long?`, `manga_id: Long?`, `read`, `bookmark`, `last_page_read: Int`, `date_fetch`, `source_order: Int`, `last_modified`, `version` (`Chapter.kt:9-28`); `ChapterImpl` defaults: nulls, false, 0, `scanlator = null` (`ChapterImpl.kt:5-33`); `chapter_number: Float = 0f`.
* `Chapter.isRecognizedNumber = chapter_number >= 0f` (`Chapter.kt:30-31`); the sources use -1 for "unknown number".
* `toDomainChapter()` returns null when `id == null || manga_id == null`; otherwise copies all fields converting `last_page_read` Int -> Long, `source_order` Int -> Long, `chapter_number` Float -> Double (`Chapter.kt:33-51`). The Float->Double widening is lossless but displays like 12.5 -> 12.5 and 0.1f -> 0.10000000149011612; Proposal: parse chapter numbers as `f32` in the source DTO and widen to `f64` the same way so equality with stored values is identical.
* `equals`: same class, same `url`, same `id` (`ChapterImpl.kt:35-42`); `hashCode = url.hashCode() + id.hashCode()`.
* `Episode` adds `anime_id`, `seen`, `bookmark`, `fillermark` (from SEpisode), `last_second_seen: Long`, `total_seconds: Long`, `date_fetch`, `source_order`, `last_modified`, `version`, `summary`, `preview_url`, `memo: JsonObject = JsonObject.EMPTY` (`EpisodeImpl.kt:8-46`); `toDomainEpisode()` also requires non-null ids and copies `fillermark, summary, previewUrl, memo`.
* Used by `ReaderChapter` (area:43), `ChapterTransition.kt:54-55`, `PlayerViewModel.kt:372`, the tracker implementations (`Anilist`, `Bangumi`, `Jellyfin`, `Kavita`, `Kitsu`, `Komga`, `MangaUpdates`, `MyAnimeList`, `Shikimori`, `Simkl`, `Suwayomi`, which use only the `*Track` legacy models).

## 6. Sources

### 6.1 Source list flows (MSR, ASR)

| Function | Behaviour |
|---|---|
| `getMangaSources()` (`MSR:23-31`) | `sourceManager.catalogueSources` (all installed `CatalogueSource`s, including the Local source) mapped through `mapSourceToDomainSource(...)` then `.copy(supportsLatest = it.supportsLatest)`. `Source(id, lang, name, supportsLatest, isStub = false)` with default `pin = unpinned`, `isUsedLast = false` |
| `getOnlineMangaSources()` (`MSR:33-39`) | Same flow filtered `filterIsInstance<HttpSource>()` (excludes Local); `supportsLatest` is NOT copied (false) |
| `getMangaSourcesWithFavoriteCount()` (`MSR:41-55`) | `combine(db flow of getSourceIdWithFavoriteCount, sourceManager.catalogueSources) { counts, _ -> counts }.map { rows -> rows.map { (sourceId, count) -> sourceManager.getOrStub(sourceId) -> Source(...).copy(isStub = source is StubMangaSource) to count } }`. SQL: `SELECT source, count(*) FROM mangas WHERE favorite = 1 GROUP BY source`. The second flow only triggers re-mapping (so a source installed/removed flips `isStub`); `supportsLatest` is false in these rows. Sources without any favourite are absent |
| `getMangaSourcesWithNonLibraryManga()` (`MSR:57-69`) | `subscribeToList { getSourceIdsWithNonLibraryManga }` = `SELECT source, COUNT(*) AS manga_count FROM mangas WHERE favorite = 0 GROUP BY source`, mapped with `getOrStub` + `isStub` into `MangaSourceWithCount(source, count)`. Does not observe the source manager (a source change does not re-emit). Consumer: "clear database" screen (area:47) |
| `getAnimeSources()` (`ASR:21-29`) | `sourceManager.sources` (ALL anime sources) mapped + `.copy(supportsLatest = it.supportsLatest)` |
| `getOnlineAnimeSources()` (`ASR:31-37`) | `filterIsInstance<AnimeHttpSource>()` |
| `getAnimeSourcesWithFavoriteCount()` (`ASR:39-53`) | As manga with `animesQueries.getAnimeSourceIdWithFavoriteCount` |
| `mapSourceToDomainSource` | Manga: private member (`MSR:90-96`); anime: top-level function (`ASR:72-78`) — both: `supportsLatest = false`, `isStub = false` |

### 6.2 Pager factories (MSR:71-88, ASR:55-70)

| Function | Behaviour |
|---|---|
| `searchManga(sourceId, query, filterList)` | `sourceManager.get(sourceId) as CatalogueSource` then `SourceSearchPagingSource(source, query, filterList)`. **`get` returns null for an uninstalled source, and `null as CatalogueSource` throws `NullPointerException`("null cannot be cast to non-null type")** at the call site (inside the `Pager` factory lambda) |
| `getPopularManga`, `getLatestManga` | Same cast |
| `searchAnime`, `getPopularAnime`, `getLatestAnime` | `sourceManager.getOrStub(sourceId)` (never null); the stub throws `AnimeSourceNotInstalledException` on load, shown as `MR.strings.loader_not_implemented_error` ("Source not found") |
| routing (domain `GetRemoteManga.subscribe`, `GetRemoteAnime.subscribe`) | `query == QUERY_POPULAR` -> popular; `query == QUERY_LATEST` -> latest; anything else -> search. `QUERY_POPULAR = "eu.kanade.domain.source.manga.interactor.POPULAR"`, `QUERY_LATEST = "eu.kanade.domain.source.manga.interactor.LATEST"`; anime uses `...anime.interactor.POPULAR/LATEST`. A user query that equals one of those strings would be treated as a listing request (never happens in practice) |

### 6.3 Paging load algorithm (MSP:37-69, ASP:34-66)

```text
load(params):                       # params.key is the page number, null on first load
  page = params.key ?: 1
  try:
     withIOContext:
        result = requestNextPage(page.toInt())      # popular/latest/search call on the source
        if result.mangas is empty: throw NoChaptersException()   # anime: NoEpisodesException
  catch Exception e:  return LoadResult.Error(e)   # includes CancellationException (quirk)
  return Page(data = result.mangas, prevKey = null,
              nextKey = if result.hasNextPage then page + 1 else null)
getRefreshKey(state) = state.anchorPosition?.let { closestPageToPosition(it) }?.let { prevKey ?: nextKey }
```

`requestNextPage` calls `source.getSearchManga(currentPage, query, filters)`, `source.getPopularManga(currentPage)` or `source.getLatestUpdates(currentPage)` (anime: `getSearchAnime`, `getPopularAnime`, `getLatestUpdates`). The empty-page rule applies to every page: an empty *later* page after `hasNextPage = true` also yields an error (the UI shows the error row at the end of the list) rather than "end of list". Error text: `NoChaptersException`/`NoEpisodesException` -> `MR.strings.no_results_found` ("No results found"); source stub -> `loader_not_implemented_error` ("Source not found"); `HttpException` -> `exception_http` ("HTTP %d, check website in WebView"); `UnknownHostException` -> `exception_offline` ("No Internet connection") when offline else `exception_unknown_host` ("Couldn't reach %s"); otherwise `"$className: $message"` (or just message for plain `Exception`/`IOException`) (`app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:14-39`).
Paging3 configuration at the call site: `Pager(PagingConfig(pageSize = 25))` (`BrowseMangaSourceScreenModel.kt:112`, `BrowseAnimeSourceScreenModel.kt:112`): `prefetchDistance = 25` (default = pageSize), `initialLoadSize = 75` (ignored: the source dictates page size), no `maxSize`. The consumer de-duplicates by mapping each `SManga` to a local DB row (`networkToLocalManga`), so repeated results across pages collapse. `enablePlaceholders` default true (library behaviour; irrelevant because `itemsBefore` is not supplied).

### 6.4 Stub sources (MangaStubSourceRepositoryImpl.kt, AnimeStubSourceRepositoryImpl.kt)

Tables `sources(_id PK, lang NOT NULL, name NOT NULL)` and `animesources` (same). `subscribeAllManga()` = `SELECT * FROM sources`; `getStubMangaSource(id)` = `SELECT * FROM sources WHERE _id = :id` (null if absent); `upsertStubMangaSource(id, lang, name)` = `INSERT ... ON CONFLICT(_id) DO UPDATE SET lang = :lang, name = :name WHERE _id = :id`. Caller (`AndroidMangaSourceManager.kt:57-125`, area:17): when an installed source is registered, it compares `StubMangaSource.from(source)` with the stored row (data-class equality on id/lang/name) and upserts if different, then calls `downloadManager.renameSource(dbSource, source)` when a previous row existed. `createStubSource(id)`: stored row -> else extension-manager data (`extensionManager.getSourceData(id)`, registers it) -> else `StubMangaSource(id, lang = "", name = "")`. `StubMangaSource.toString()` = `"$name (${lang.uppercase()})"` unless name or lang is blank, then the id as a string. Stub methods throw `SourceNotInstalledException`/`AnimeSourceNotInstalledException` (`StubMangaSource.kt`, `StubAnimeSource.kt`); anime stub has `supportsLatest = false`, `supportsRelatedAnime = false`.

## 7. Anime extension stores (OBSERVED; full algorithm in `p1-topic-extension-store-index.md`)

Purpose: the anime side of Aniyomi replaced "extension repos" (URL base + `repo.json`) with "extension stores": an index document (JSON or protobuf, optionally gzip) that carries store metadata and either an inline extension list or the URL of one. Legacy repos are still supported and auto-upgraded when their `repo.json` advertises an `index_v2`.

Key behaviours (each cited in the topic file):

1. `insert(indexUrl)` = `service.fetch(indexUrl).mapCatching { upsert(it) }` (`ESR:19-21`): fetch the document, derive `AnimeExtensionStore`, upsert row keyed by `index_url`. Failure is returned as `Result.failure`; the screen model shows `throwable.message ?: "unknown error"` inside the dialog.
2. `refreshAll()` (`ESR:38-52`): sequentially for every stored row, `service.fetch(store.index_url).mapCatching { upsert }`; a failure is logged per store (`"Failed to refresh extension store '<name> (<url>)'"`) and does not stop the loop; the whole loop is wrapped in a catch-all that logs.
3. `fetchExtensions()` (`ESR:69-88`): loads all stores, runs `service.getExtensions(store)` for all in parallel inside `supervisorScope`, logs each failure (`"Failed to fetch extensions for store '<name> (<indexUrl>)'"`), concatenates successes in store order, returns `emptyList()` on a catch-all failure.
4. `insertFromPreference(indexUrl, name)` (`ESR:23-36`): used only by the one-time migration; inserts `signingKey = "NO_SIGNING_KEY"`, `contactWebsite = indexUrl`, `contactDiscord = null`, `isLegacy = false`, `extensionListUrl = null`, `badgeLabel = name`.
5. `remove(indexUrl)` deletes the row; `getCountAsFlow()` = `SELECT COUNT(*)`; `getAllAsFlow()` = `SELECT * FROM extension_store` (no ORDER BY; the table is an ordinary rowid table with a TEXT primary key, so a full scan returns rows in insertion (rowid) order in practice, which SQLite does not guarantee; Proposal: add `ORDER BY rowid` so the screen order and the order in which duplicate extensions from several stores are listed stay stable).
6. Upsert (`extension_store.sq:27-40`) overwrites all columns on conflict; there is NO signing-key continuity check for stores (contrast manga repos, section 8): a store whose key changes is silently accepted on refresh.
7. Callers: `AnimeExtensionApi.checkForUpdates` (`AnimeExtensionApi.kt:30-45`) rate-limits to once per day via preference `last_ext_check` when `fromAvailableExtensionList`, calls `updateExtensionStores()` (`refreshAll`) first, then `findExtensions()` or the cached available list, then compares versions (`availableExt.versionCode > installedExt.versionCode || availableExt.libVersion > installedExt.libVersion`) and notifies via `ExtensionUpdateNotifier.promptUpdates(names, anime = true)`. The store screen (`AnimeExtensionStoresScreenModel.kt`): after a successful add or a delete it calls `extensionManager.findAvailableExtensions()`; refresh button calls `refreshRepos()` (only `refreshAll`, no re-listing).
8. Backup: `AnimeExtensionStoresBackupCreator` serialises `getExtensionStores.get()` (area:21).

## 8. Manga extension repos (OBSERVED)

Table `extension_repos(base_url PK, name NOT NULL, short_name, website NOT NULL, signing_key_fingerprint TEXT UNIQUE NOT NULL)` (`extension_repos.sq:1-7`). Repository (`MER`):

* `insertRepo(...)`: plain `INSERT`; any `SQLiteException` (PK or UNIQUE violation, or anything else) is rethrown as `SaveExtensionRepoException(ex)` — an `IOException("Error Saving Repository to Database")` (`MER:36-48`).
* `upsertRepo(...)`: `INSERT ... ON CONFLICT(base_url) DO UPDATE SET name, short_name, website, signing_key_fingerprint WHERE base_url = base_url` (the WHERE is a tautology); a conflict on the fingerprint UNIQUE constraint of a *different* row still throws -> `SaveExtensionRepoException` (`MER:50-62`).
* `replaceRepo(newRepo)`: `INSERT ... ON CONFLICT(signing_key_fingerprint) DO UPDATE SET base_url = :base_url, name, short_name, website WHERE signing_key_fingerprint = signing_key_fingerprint` — replaces the row that owns that fingerprint, changing its primary key `base_url`. Not wrapped: a PK conflict with a third row throws a raw `SQLiteException` to the caller (`MER:64-74`).
* `deleteRepo(baseUrl)`, `getRepo(baseUrl)`, `getRepoBySigningKeyFingerprint(fp)`, `getAll`, `subscribeAll` (`SELECT * FROM extension_repos`), `getCount` (flow, `COUNT(*)` mapped to Int).

Interactors (domain; behaviour spec'd here because they define this repository's contract):

* `CreateMangaExtensionRepo.await(indexUrl)`: (1) `indexUrl.toHttpUrlOrNull()?.toString()` — OkHttp parses and re-serialises (lower-cased host, default port dropped, dot segments resolved, percent-encoding normalised); null -> `Result.InvalidUrl`. (2) must match `^https://.*/index\.min\.json$` (plain `http://` is rejected) else `InvalidUrl`. (3) `baseUrl = formatted.removeSuffix("/index.min.json")`. (4) `service.fetchRepoDetails(baseUrl)`: `GET "$baseUrl/repo.json"` parsed as `{"meta": {name, shortName?, website, signingKeyFingerprint}}`; any exception -> null -> `InvalidUrl` (logged `"Failed to fetch repo details"`). (5) `insertRepo`; on `SaveExtensionRepoException` log WARN `"SQL Conflict attempting to add new manga repository <baseUrl>"` then classify: `getRepo(baseUrl)` non-null -> `RepoAlreadyExists`; else `getRepoBySigningKeyFingerprint(fp)` non-null -> `DuplicateFingerprint(oldRepo, newRepo)`; else `Error`. Results map to UI: `InvalidUrl` -> toast/snackbar `MR.strings.invalid_repo_name` ("Invalid repo URL"); `RepoAlreadyExists` -> `MR.strings.error_repo_exists` ("This repo already exists!"); `DuplicateFingerprint` -> conflict dialog (`action_replace_repo_title` "Signing Key Fingerprint Already Exists", `action_replace_repo_message` "Repository %1$s has the same Signing Key Fingerprint as %2$s.\nIf this is expected, %2$s will be replaced, otherwise contact your repo maintainer." with `newRepo.name`, `oldRepo.name`; button `action_replace_repo` "Replace" calls `replaceRepo(newRepo)`); `Success` and `Error` -> nothing visible (an `Error` result is silently ignored by `MangaExtensionReposScreenModel.createRepo`).
* `UpdateMangaExtensionRepo.awaitAll()` (refresh button and extension-list refresh): `coroutineScope { getAll().map { async { await(it) } }.awaitAll() }`; `await(repo)`: fetch details for `repo.baseUrl`; null -> return; write only if `repo.signingKeyFingerprint.startsWith("NOFINGERPRINT")` **or** equals the fetched fingerprint, via `upsertRepo(newRepo)`. A mismatching fingerprint is ignored silently (the repo keeps the old key). An exception from `upsertRepo` (unique conflict) cancels the sibling refreshes and propagates to `launchIO` (no handler: crash in Aniyomi).
* `DeleteMangaExtensionRepo.await(baseUrl)`, `ReplaceMangaExtensionRepo.await(repo)`, `GetMangaExtensionRepo.subscribeAll()/getAll()`, `GetMangaExtensionRepoCount.subscribe()`.
* Migration compat (`TrustExtensionRepositoryMigration.kt:15-52`, version 7f): every legacy preference URL `u` at index `i` (0-based) becomes `upsertRepo(u, "Repo #${i+1}", null, u, "NOFINGERPRINT-${i+1}")`, `SaveExtensionRepoException` logged and skipped; then the preference is deleted. The `NOFINGERPRINT-` prefix is what lets the first successful refresh adopt the real fingerprint.
* Screen strings: `label_extension_repos` "Extension repos", `information_empty_repos` "You have no repos set.", `action_add_repo` "Add repo", `label_add_repo_input` "Repo URL", `action_add_repo_message` (AYMR) text, `delete_repo_confirmation`, `add_repo_confirmation`, `action_open_repo`.

## 9. Custom buttons (OBSERVED; full detail in `p1-topic-custom-buttons.md`)

* Table `custom_buttons(_id PK, name, isFavorite INTEGER AS Boolean, sortIndex, content, longPressContent, onStartup)` in the **anime** DB (`custom_buttons.sq:3-11`), created with one seeded row (`_id = 1`, name `+85 s`, favourite, sortIndex 0 — the "skip intro" button) by `INSERT OR IGNORE` in the same file and in migration 129 (`129.sqm`).
* Repository (`CBR`): `findAll` is `SELECT * FROM custom_buttons ORDER BY sortIndex`; `insertCustomButton(name, sortIndex, content, longPressContent, onStartup)` always inserts `isFavorite = false`, wrapping `SQLiteException` in `SaveCustomButtonException` ("Error Saving Custom Button to Database"); `updatePartialCustomButton(update)` runs the coalesce `UPDATE` where every null field keeps the column; `updatePartialCustomButtons(updates)` does the same for a list in one transaction; `deleteCustomButton(id)`.
* Interactors (all `withNonCancellableContext`, errors logged at ERROR, never thrown): create (next `sortIndex = max + 1` or 0), delete (delete, then renumber the survivors `0..n-1` by their current `sortIndex` order, in one transaction), reorder (under a `Mutex`: remove from the list and re-insert at `newIndex`, renumber all), toggle favourite (if currently favourite: clear only it; else under a `Mutex` set `isFavorite = (id == target)` on every button in one transaction — at most one favourite exists), update.
* Player consumption (`PlayerViewModel.kt:319-345`, `PlayerActivity.kt:535-580`, `PlayerViewModel.kt:2167-2173`): load all buttons, primary = first with `isFavorite`; generate `custombuttons.lua` and `load-script` it; execute with the mpv command `script-message call_button_<id>` (long press `call_button_<id>_long`). Placeholders `$id` and `$isPrimary` are textually replaced in `content`, `longPressContent`, `onStartup`.
* Screen events: any `InternalError` -> `MR.strings.internal_error` ("InternalError: Check crash logs for further information").
* Restore (area:22; `CustomButtonRestorer.kt`): buttons matched by `name`; new ones appended with `sortIndex` continuing from `max + 1`, `isFavorite` kept only if no favourite exists yet.

## 10. Release service (OBSERVED; `p1-topic-release-check.md`)

1. `latest(arguments)`: `GET https://api.github.com/repos/<arguments.repository>/releases/latest` via the shared client with `awaitSuccess()` (non-2xx -> `HttpException`), parsed as `GithubRelease(tag_name, body, html_url, assets[{name, browser_download_url}])` with the app `Json` (`ignoreUnknownKeys`).
2. Download link: group assets by the first element of `BUILD_TYPES = ["arm64-v8a", "armeabi-v7a", "x86_64", "x86"]` contained in the asset name as `"-<type>"` (null when none matches; the later asset wins on a key clash because of `associate`); result = `map[Build.SUPPORTED_ABIS[0]] ?: map[null]`; `null` -> `latest` returns null -> "no new update".
3. Release notes: every GitHub `@mention` is replaced by `[@name](https://github.com/name)` using the regex `\B@([a-z0-9](?:-(?=[a-z0-9])|[a-z0-9]){0,38}(?<=[a-z0-9]))` (case-insensitive).
4. The interactor `GetApplicationRelease` (area:09): 3-day throttle through preference `app_state:last_app_check` unless `forceCheck`; preview builds compare `tag` digits to the commit count; stable builds compare dotted numbers with a quirk (any component greater returns true).
5. Callers: `AppUpdateChecker.checkForUpdate` (main activity on start and the About screen "Check for updates"): messages `update_check_no_new_updates`, `update_check_eol`, notification `update_check_notification_update_available` ("New version available!") with actions `action_download`, `whats_new`; repository names `aniyomiorg/aniyomi-preview` (preview builds) or `aniyomiorg/aniyomi`.

## 11. Concurrency and dispatcher table (OBSERVED -> Proposal)

| Operation | Kotlin dispatcher | Rust |
|---|---|---|
| All `await*` | `Dispatchers.IO` (or the transaction thread) | `tokio::task::spawn_blocking` (reads: reader pool; writes: the single writer) |
| `subscribeTo*` re-queries | `queryDispatcher` (IO) | a dedicated tokio task per subscription calling `spawn_blocking` for each re-run; results forwarded to the GTK main thread through an `async_channel` consumed with `glib::spawn_future_local` |
| Pager `load` | `withIOContext` | `tokio` async (network) |
| Store `fetch/getExtensions` | caller's context (`launchIO`) | tokio tasks; fan-out with `futures::future::join_all` (unbounded in Kotlin; Proposal cap: 8 concurrent stores via `buffer_unordered(8)`) |
| Interactors with `withNonCancellableContext` | non-cancellable | `spawn_blocking` closures always run to completion even if the awaiting future is dropped |
| Mutex-guarded interactors (`ReorderCustomButton`, `ToggleFavoriteCustomButton`) | per-instance `Mutex` (note: the interactors are `addFactory` in `DomainModule.kt:410-415`, i.e. a NEW instance per injection, so the mutex only protects within one instance) | one `tokio::sync::Mutex` owned by `CustomButtonRepo` (shared) — strictly stronger |

## 12. Message key index (strings referenced by this area's behaviour)

| Key | Text | Where used |
|---|---|---|
| `MR.strings.internal_error` | "InternalError: Check crash logs for further information" | History error event, custom button errors |
| `MR.strings.clear_history_confirmation` / `clear_history_completed` | "Are you sure? All history will be lost." / "History deleted" | History clear |
| `MR.strings.information_no_recent` / `information_no_recent_manga` | "No recent updates" / "Nothing read recently" | Empty Updates / History (area:42) |
| `MR.strings.no_results_found` | "No results found" | Empty remote page (`NoChaptersException`, `NoEpisodesException`) |
| `MR.strings.loader_not_implemented_error` | "Source not found" | Stub source called |
| `MR.strings.exception_http` / `exception_offline` / `exception_unknown_host` | "HTTP %d, check website in WebView" / "No Internet connection" / "Couldn't reach %s" | Source errors |
| `MR.strings.invalid_repo_name`, `error_repo_exists`, `action_replace_repo_title`, `action_replace_repo_message`, `action_replace_repo` | see section 8 | Manga repo screen |
| `MR.strings.extensionStores`, `extensionStoresScreen.emptyLabel` "You haven't added an extension store yet", `.addStore.title` "Add extension store", `.addStore.alreadyExists` "This extension store already exists", `.addStore.processing` "Processing…", `.addStoreInput.inputLabel` "Store index URL", `.addStoreDeeplink.bodyText` "Do you wish to add the extension store below?", `.deleteStore.title` "Remove extension store", `.deleteStore.body` "Do you wish to remove the extension store \"%1$s\" (%2$s)?" | Anime store screen (`strings.xml:373-382`) |
| `MR.strings.information_required_plain` | "*required" | Add dialogs |
| `AYMR.strings.pref_player_custom_button*` (`pref_player_custom_button_empty`, `_add`, `_edit`, `_title`, `_error_exists` "Button already exists!", `_content` "Lua code", `_content_long`, `_optional`, `_delete`, `_delete_confirm`, `_startup`, `_id`) | Custom button editor (area:48; `_error_exists` has no producer in this area: names are not unique in the DB) |
| `MR.strings.update_check_no_new_updates`, `update_check_eol`, `update_check_notification_update_available`, `whats_new`, `action_download` | see section 10 | Release check |

## 13. Interactions with other areas

| Other area | Interaction |
|---|---|
| area:03, area:04 (schemas) | Create the tables/views quoted in `p1-data-and-config.md`; `Db::open` calls their `Schema::create/migrate`. The seeded custom button row and `extension_store`/`extension_repos` tables are in area:04/area:03 respectively |
| area:05 (entry/item repos) | Shares `Db`; chapter/episode deletes cascade into `history`/`animehistory` rows (the change notifier must fire for `history` when `chapters` rows are deleted: handled by SQLite's update hook for FK cascades) |
| area:07/08/09 (domain) | Own `MangaHistory*`/`Track` models, interactors (`GetNextChapters`, `GetApplicationRelease`) and `Source`/`Pins`; this area owns only repository-level behaviour |
| area:12 (network) | `NetworkHelper.client` + `GET` + `awaitSuccess` + `parseAs`; store service and release service are its consumers (headers, UA, cache, Cloudflare handling are theirs) |
| area:15/16/17 (sources) | `CatalogueSource`/`AnimeSource` traits and `SourceManager` flows consumed by `SourceRepo`/`RemotePager` |
| area:18/19/20 (extensions) | `AnimeExtension.Available` shape (`NES:93-120` produces it); area:20 decides what the artifact URL points at on Linux |
| area:21/22 (backup) | Serialise/restore stores, repos, custom buttons, tracks, history; `manga_sync.update` coalesce query |
| area:26 (library update jobs) | Writes chapters (`date_fetch`) that populate Updates; reads `GetNextChapters` |
| area:27 (trackers) | Mutates `MangaTrack`/`AnimeTrack` via `copyPersonalFrom`, persists through `InsertMangaTrack` |
| area:42 (history/updates UI), 40/41 (browse UI), 47 (settings), 48/46 (custom buttons editor and strip), 43/45 (reader/player recording history) | UI and recorders |
