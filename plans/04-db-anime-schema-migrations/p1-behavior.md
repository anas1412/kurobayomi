# Area 04 — Anime database — behaviour specification

Scope: every statement, trigger, view and migration in `data/src/main/sqldelightanime/` plus the Kotlin code that decides how each statement is bound and sequenced (`data/src/main/java/tachiyomi/data/**/anime/*`, `.../items/episode/*`, `.../custombutton/*`, `mihon/data/extension/anime/*`, and the domain interactors that call them). Paths are repo-relative to `/home/blackbox/aniyomi-src`. "OBSERVED" = what the Kotlin/SQL does. "Proposal:" = what Kurobayomi does. Numeric claims about SQLite behaviour were verified by running the schema in SQLite 3.53.4 (see `p1-edge-cases-and-tests.md` for the harness results).

Section index: 1 SQLDelight conventions, 2 handler/transactions/flows, 3 `animes`, 4 `episodes`, 5 history, 6 library and statistics views, 7 categories, 8 tracks, 9 updates feed, upcoming, deletable, 10 sources/relations/extension store, 11 custom buttons, 12 episode name sanitizer, 13 clear-database operation, 14 restore interplay (`is_syncing`, `version`), 15 concurrency and timing, 16 interactions with other areas, 17 error paths and message keys.

## 1. SQLDelight conventions that change the meaning of the SQL

1. Named statements are written `name:` on its own line followed by one SQL statement ending in `;`. Each becomes a Kotlin method `<fileQueries>.name(params)`; a `SELECT` returns a `Query<T>` (rows mapped by a lambda, or by the generated row class), anything else is an `ExecutableQuery` or a plain execute.
2. `:param` placeholders become method parameters in the order of FIRST appearance in the statement text. A name used twice (for example `:seenAt` in `animehistory.upsert`) is bound once per occurrence from the same argument.
3. `IN :list` expands to `IN (?,?,...)` with one placeholder per element at call time, no chunking. SQLite's default limit is 999 variables before 3.32 and 32766 afterwards; Aniyomi never chunks. Empty list gives `IN ()` which is false.
4. `coalesce(:x, col)` in `UPDATE` means "bind NULL to leave the column unchanged". A nullable column can therefore never be set back to NULL by these statements. This is why `removeParentIdByIds` exists as a separate statement (to clear `parent_id`).
5. `AS Boolean`, `AS Date`, `AS List<String>`, `AS AnimeUpdateStrategy`, `AS FetchType`, `AS JsonObject` are column adapters applied by SQLDelight in generated code (`p1-data-and-config.md` section 2). Column adapters only apply to columns of the table the annotation sits on; a view that selects the column re-exposes the type.
6. `SELECT *` rows are mapped POSITIONALLY by generated code in the declared column order of the `.sq` table. In Rust all mapping is by column NAME (a column added later must not shift anything).
7. `ON CONFLICT(col) DO UPDATE SET ... WHERE col = :x` clauses in the `upsert` statements have tautological `WHERE` parts; they do not filter anything.
8. `INSERT OR REPLACE` / `UNIQUE ... ON CONFLICT REPLACE` delete the conflicting row and insert a new one; the new row gets a NEW `_id` when the statement does not supply one (observed on `anime_sync`).
9. Double-quoted string literals (`""`, `"System category can't be deleted"`) are accepted by SQLite as a legacy fallback; Rust text uses single quotes.
10. A `FOREIGN KEY ... ON DELETE CASCADE` only works when `PRAGMA foreign_keys = ON` is set per connection (default OFF in SQLite); Aniyomi sets it in `Callback.onOpen`.

## 2. Handler, transactions and flows (`AndroidAnimeDatabaseHandler`, `AnimeTransactionContext`)

OBSERVED API (`data/src/main/java/tachiyomi/data/handlers/anime/AnimeDatabaseHandler.kt`):

| Method | Meaning |
|---|---|
| `await(inTransaction=false) { ... }` | run a block against `AnimeDatabase`, return its value |
| `awaitList / awaitOne / awaitOneOrNull` | run a block returning `Query<T>` and execute it as list / exactly one (throws when 0 or more than 1 rows) / zero-or-one |
| `awaitOneExecutable / awaitOneOrNullExecutable` | same for `ExecutableQuery` (used for `insert` followed by `selectLastInsertedRowId`) |
| `subscribeToList / subscribeToOne / subscribeToOneOrNull` | `Query.asFlow()` mapped on `queryDispatcher`: emits the current result at collection start and again whenever SQLDelight reports a change of a table the query reads |
| `subscribeToPagingSource` | Paging 3 source; unused (T-04a-033) |

Dispatch rules (`AndroidAnimeDatabaseHandler.dispatch`):
1. `inTransaction = true`: run the block inside `withAnimeTransaction` (below).
2. otherwise, if the SQL driver already has a current transaction on this thread, run the block directly (no thread hop).
3. otherwise run on `getCurrentAnimeDatabaseContext()`: the transaction dispatcher when the coroutine context carries an `AnimeTransactionElement`, else `queryDispatcher = Dispatchers.IO`.

`withAnimeTransaction`: reuse the inherited transaction context if present (nested calls join the outer transaction), otherwise create one that pins a single thread taken from the IO pool; acquire/release a counter on the element; run `db.transactionWithResult { runBlocking(transactionContext) { block() } }`. The transaction commits unless the block throws or the coroutine is cancelled. SQLDelight runs at most one transaction at a time; later ones queue first-come-first-served. Result: every repository method that calls `await(inTransaction = true)` is atomic, and `handler.await {}` outside a transaction is a single autocommit statement block.

Proposal: `Db::write(|tx| ...)` (T-04a-008) is the single equivalent of `inTransaction = true`; nested write calls are impossible by construction (the closure receives `&Transaction`, so inner code calls plain functions that take `&Connection`), which removes the inherited-context machinery. `Db::read` = `await` without a transaction, on the reader pool.

Flow semantics to reproduce (`p1-topic-reactive-queries.md`): a subscription emits once immediately; re-emits after commits to the tables it reads; `subscribeToOne` on a row that disappears throws in Kotlin (`mapToOne` expects exactly one row), so `AnimeScreenModel` stops observing when the entry is deleted. Proposal: Rust streams yield `Option<T>` for single-row queries and the UI treats `None` as "entry gone".

## 3. Table `animes` — statements, triggers, algorithms

### 3.1 Read statements

| Statement | SQL (verbatim, whitespace folded) | Params | Used by |
|---|---|---|---|
| `getAnimeById` | `SELECT * FROM animes WHERE _id = :id` | id | `AnimeRepositoryImpl.getAnimeById` (`awaitOne`: throws when missing; `GetAnime.await` catches and returns null), `getAnimeByIdAsFlow` (`subscribeToOne`) |
| `getAnimeByUrlAndSource` | `SELECT * FROM animes WHERE url = :url AND source = :source LIMIT 1` | url, source | `NetworkToLocalAnime`, backup restore `findExistingAnime`, deep links (`DeepLinkAnimeScreenModel`), `getAnimeByUrlAndSourceIdAsFlow`. No unique index exists on `(source, url)`, so duplicates are possible and the first row in rowid order wins (a source comment in the file says this "should ideally never" return more than one) |
| `getFavorites` | `SELECT * FROM animes WHERE favorite = 1` | none | `getAnimeFavorites`, `SetAnimeDefaultEpisodeFlags`, migration screens |
| `getWatchedAnimeNotInLibrary` | `SELECT * FROM animes WHERE favorite = 0 AND _id IN (SELECT DISTINCT episodes.anime_id FROM episodes WHERE seen = 1 OR last_second_seen != 0)` | none | `BackupCreator` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt:102`): when the backup option `readEntries` is on, non-library entries with watch progress are added to the backup |
| `getAllAnime` | `SELECT * FROM animes` | none | no caller (Drop, T-04a-035) |
| `getAllAnimeSourceAndUrl` | `SELECT source, url FROM animes` | none | `AnimeRestorer.sortByNew` (groups urls by source to put not-yet-present entries first) |
| `getAnimesWithFavoriteTimestamp` | `SELECT * FROM animes WHERE favorite_modified_at IS NOT NULL` | none | no caller (Drop) |
| `getAnimeSourceIdWithFavoriteCount` | `SELECT source, count(*) FROM animes WHERE favorite = 1 GROUP BY source` | none | `AnimeSourceRepositoryImpl.getAnimeSourcesWithFavoriteCount` (combined with the source manager's source list; consumed by the Migrate screen `MigrateAnimeSourceScreenModel`, which lists sources with the number of library entries each has) |
| `getFavoriteBySourceId` | `SELECT * FROM animes WHERE favorite = 1 AND source = :sourceId` | sourceId | migration entry list (`MigrateAnimeScreenModel`) |
| `getDuplicateLibraryAnime` | `SELECT * FROM animes WHERE favorite = 1 AND LOWER(title) = :title AND _id != :id` | title (already lowercased by Kotlin `String.lowercase()`), id | duplicate warning when adding to library (`AnimeScreenModel.kt:412`, `BrowseAnimeSourceScreenModel`, `AnimeHistoryScreenModel.kt:183`) |
| `getUpcomingAnime` | `SELECT * FROM animes WHERE next_update >= :startOfDay AND favorite = 1 AND status IN :statuses ORDER BY next_update ASC` | startOfDay (ms), statuses | `AnimeRepositoryImpl.getUpcomingAnime`: `startOfDay = LocalDate.now().atStartOfDay(ZoneId.systemDefault()).toEpochSecond() * 1000`; statuses fixed by `GetUpcomingAnime` as `{1 ONGOING, 4 PUBLISHING_FINISHED, 7 UPCOMING}`; result is a flow |
| `getChildrenByParentId` | `SELECT * FROM animes WHERE parent_id = :parentId` | parentId | clear-database recursion |
| `selectLastInsertedRowId` | `SELECT last_insert_rowid()` | none | run right after `insert` in the same transaction/connection |

`getDuplicateLibraryAnime` note (OBSERVED + verified): SQLite's `LOWER()` lowercases ASCII only, so a library title `Éclair` is stored with `É` and `LOWER` leaves it, while Kotlin sends `éclair`; the match is missed. Proposal (T-04a-022): register `kuro_lower()` in Rust and compare `kuro_lower(title) = :title`.

### 3.2 Write statements

`insert` (`animes.sq:166-168`): columns `source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, episode_flags, cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, version, fetch_type, parent_id, season_flags, season_number, season_source_order, background_url, background_last_modified, memo` (29 columns); values are the 28 named parameters `:source, :url, :artist, :author, :description, :genre, :title, :status, :thumbnailUrl, :favorite, :lastUpdate, :nextUpdate, :initialized, :viewerFlags, :episodeFlags, :coverLastModified, :dateAdded, :updateStrategy, :calculateInterval, 0 (literal for last_modified_at), :version, :fetchType, :parentId, :seasonFlags, :seasonNumber, :seasonSourceOrder, :backgroundUrl, :backgroundLastModified, :memo`. `favorite_modified_at` stays NULL, `is_syncing` takes its default 0. No trigger fires on INSERT.

`AnimeRepositoryImpl.insertAnime` (OBSERVED): inside one transaction run `insert` with the domain `Anime` fields (`calculateInterval = anime.fetchInterval.toLong()`, `viewerFlags = anime.viewerFlags`, `memo = anime.memo`), then `selectLastInsertedRowId` and return it as `Long?`.

`update` (`animes.sq:170-201`): `UPDATE animes SET <col> = coalesce(:<param>, <col>)` for exactly these 29 columns in this order: `source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, episode_flags, cover_last_modified, date_added, update_strategy, calculate_interval, version, is_syncing, fetch_type, parent_id, season_flags, season_number, season_source_order, background_url, background_last_modified, memo` `WHERE _id = :animeId`. Not updatable through this statement: `_id`, `last_modified_at`, `favorite_modified_at` (trigger-owned).

`AnimeRepositoryImpl.partialUpdateAnime` (OBSERVED): for each `AnimeUpdate` in ONE transaction call `update` with: `genre = value.genre?.let(StringListColumnAdapter::encode)`, `calculateInterval = value.fetchInterval?.toLong()`, `updateStrategy` and `fetchType` encoded to their ordinal Long, `memo` encoded to bytes, and ALWAYS `isSyncing = 0`. `updateAnime` and `updateAllAnime` wrap this in `try/catch (Exception)`, log with `logcat(LogPriority.ERROR, e)` and return `false`; success returns `true`. Consequence: any normal edit clears a leftover `is_syncing = 1`.

`resetViewerFlags`: `UPDATE animes SET viewer = 0` (every row). Repository wrapper returns `true`, or logs and returns `false`.
`resetIsSyncing`: `UPDATE animes SET is_syncing = 0 WHERE is_syncing = 1` (no caller; Proposal T-04a-024 runs it at startup).
`removeParentIdByIds`: `UPDATE animes SET parent_id = NULL WHERE _id IN :animeIds`; wrapper swallows and logs exceptions.
`deleteAnimesNotInLibraryByAnimeIds`: `DELETE FROM animes WHERE favorite = 0 AND _id IN :animeIds`; cascades to episodes, history, category links, tracks, relations (both directions).

### 3.3 Triggers on `animes` (net effect of one `UPDATE animes`, `recursive_triggers` OFF)

Three AFTER UPDATE triggers: `update_last_favorite_at_animes` (fires when the column `favorite` appears in the SET list, even if its value is unchanged), `update_last_modified_at_animes` (every update), `update_anime_version` (every update; its inner UPDATE has a guard). Measured results (SQLite 3.53.4, fresh schema):

| Statement through `animes.update` | `last_modified_at` | `favorite_modified_at` | `version` |
|---|---|---|---|
| all parameters NULL (only `animeId`) | set to now (s) | set to now (s) | unchanged |
| `favorite = 0` when already 0 | now | now | unchanged |
| `favorite` 0 to 1 | now | now | +1 |
| `description` NULL to `x` | now | now | unchanged (`new.description != old.description` is NULL) |
| `description` `x` to `y` | now | now | +1 |
| `url` changed | now | now | +1 |
| `isSyncing = 1, version = 50` plus a description change | now | now | 50 (explicit assignment; trigger skipped because `new.is_syncing = 1`) |
| the next normal update with a description change after the above | now | now | 51 |

Why `favorite_modified_at` changes on every update: `animes.update` always lists `favorite = coalesce(:favorite, favorite)`. Proposal: keep the statement shape (backups and sync compare these stamps); do not "fix" it.

### 3.4 Algorithms around `animes`

**Add to / remove from library** (`UpdateAnime.awaitUpdateFavorite`): `update(favorite = f, dateAdded = if f then now_ms else 0)`. Side effects through triggers: `favorite_modified_at`, `last_modified_at`, and `version + 1` (favorite changed).
**Touch stamps** (`UpdateAnime`): `awaitUpdateLastUpdate` sets `last_update = now_ms`; `awaitUpdateCoverLastModified` sets `cover_last_modified = now_ms`; `awaitUpdateBackgroundLastModified` sets `background_last_modified = now_ms`.
**Fetch interval** (`UpdateAnime.awaitUpdateFetchInterval` + `AnimeFetchInterval.toAnimeUpdate`): writes `next_update` and `calculate_interval`; algorithm: `interval = anime.fetchInterval if < 0 else calculateInterval(episodes)`; `calculateInterval`: `episodeWindow = 3 if episodes.size <= 8 else 10`; upload days = distinct local dates of `date_upload > 0` episodes newest first, take `episodeWindow`; fetch days likewise from `date_fetch`; if upload days >= 3 use `(first - last in days).floorDiv(index of last)`; else if fetch days >= 3 same with fetch days; else 7; clamp to `1..28`. `getWindow(dateTime) = (today_00:00 - 1 day, today_00:00 + 1 day - 1 ms)` in epoch ms. `calculateNextUpdate`: when `next_update` lies in `[window.first, window.second + 1]` keep it; else `latestDate` = local date of `last_update` (or now when 0); `timeSinceLatest` = days between; `cycle = timeSinceLatest.floorDiv(abs(interval) if interval < 0 else increaseInterval(interval, timeSinceLatest, 10))`; result = `latestDate + (cycle + 1) * abs(interval)` days at local midnight in ms. `increaseInterval(delta, t, over)`: if `delta >= 28` return 28; `cycle = t.floorDiv(delta) + 1`; if `cycle > over` return `increaseInterval(delta * 2, t, over)` else `delta`. (Area owner: area:07-domain-entries-items; stated here because the three columns it writes live in this table.)
**Network-to-local** (`NetworkToLocalAnime.await`): look up `(url, source)`; when absent insert (`id = insertAnime(...)`); when present and not a favorite, return the stored row with `title` replaced by the source title (not persisted); when a favorite, return the stored row unchanged.
**Season sync** (`SyncSeasonsWithSource`, T-04a-020): season children are `animes` rows with `parent_id = parent._id` and `season_source_order = index in the source list`; a season missing from the source list is detached with `removeParentIdByIds` (row kept); `UpdateAnime.awaitUpdateLastUpdate` stamps the parent's `last_update`.

## 4. Table `episodes` — statements, triggers

| Statement | SQL | Notes |
|---|---|---|
| `getEpisodeById` | `SELECT * FROM episodes WHERE _id = :id` | repo uses `awaitOneOrNull` |
| `getEpisodesByAnimeId` | `SELECT * FROM episodes WHERE anime_id = :animeId` | no ORDER BY; callers sort by flags (`getEpisodeSort`); backup creator, reactive list |
| `getBookmarkedEpisodesByAnimeId` | `SELECT * FROM episodes WHERE bookmark AND anime_id = :animeId` | `WHERE bookmark` uses the integer as truth value |
| `getEpisodeByUrl` | `SELECT * FROM episodes WHERE url = :episodeUrl` | restore of history by url; may match episodes of several anime, first row wins |
| `getEpisodeByUrlAndAnimeId` | `... WHERE url = :episodeUrl AND anime_id = :animeId` | sync with source |
| `removeEpisodesWithIds` | `DELETE FROM episodes WHERE _id IN :episodeIds` | repo swallows and logs errors |
| `resetIsSyncing` | `UPDATE episodes SET is_syncing = 0 WHERE is_syncing = 1` | no caller (T-04a-024) |
| `insert` | 19 columns, 17 named parameters `:animeId, :url, :name, :scanlator, :seen, :bookmark, :lastSecondSeen, :totalSeconds, :episodeNumber, :sourceOrder, :dateFetch, :dateUpload, 0, :version, 0, :summary, :previewUrl, :fillermark, :memo` (the literals are `last_modified_at` and `is_syncing`) | |
| `update` | `UPDATE episodes SET anime_id = coalesce(:animeId, anime_id), url, name, scanlator, seen, bookmark, last_second_seen, total_seconds, episode_number, source_order, date_fetch, date_upload, version, is_syncing, summary, preview_url, fillermark, memo (each coalesce) WHERE _id = :episodeId` | 18 coalesced columns; `last_modified_at` is trigger-owned |
| `selectLastInsertedRowId` | `SELECT last_insert_rowid()` | |

`EpisodeRepositoryImpl.addAllEpisodes`: one transaction; for each episode run `insert` then `selectLastInsertedRowId().executeAsOne()` and return `episode.copy(id = lastInsertId)`; any exception is logged and the result is an EMPTY list (the transaction rolled back, so no row is stored). `partialUpdate`: one transaction, each update with `isSyncing = 0`. `removeEpisodesWithIds`, reads as in the table.

Triggers (verified): `update_last_modified_at_episodes` stamps `last_modified_at` (seconds) on every update. `update_episode_and_anime_version` has a `WHEN` clause: `new.is_syncing = 0 AND (new.seen != old.seen OR new.bookmark != old.bookmark OR new.last_second_seen != old.last_second_seen)`; then `episodes.version + 1` and `animes.version + 1` (the latter only if the anime's `is_syncing = 0`). Measured: changing `seen` bumps both; changing `fillermark` or `total_seconds` alone bumps neither; changing `last_second_seen` bumps both; with the anime `is_syncing = 1` only the episode bumps.

Progress write path (player, area:45-player-core): the player updates `seen`, `last_second_seen`, `total_seconds` through `UpdateEpisode` (a partial update) and then upserts history (section 5). The user-visible rule "seen when position reaches the threshold" lives in the player (`PlayerViewModel`), not in SQL.

## 5. History: `animehistory` and `animehistoryView`

### 5.1 Statements on the table

| Statement | SQL | Notes |
|---|---|---|
| `getHistoryByAnimeId` | `SELECT H._id, H.episode_id, H.last_seen FROM animehistory H JOIN episodes EP ON H.episode_id = EP._id WHERE EP.anime_id = :animeId AND EP._id = H.episode_id` | all history rows of one anime (also used to find the first watched date for trackers, `AddAnimeTracks.kt:59`, and by the backup creator) |
| `getHistoryByEpisodeUrl` | same join `WHERE EP.url = :episodeUrl AND EP._id = H.episode_id` | restore |
| `resetAnimeHistoryById` | `UPDATE animehistory SET last_seen = 0 WHERE _id = :historyId` | "remove from history" on one entry; wrapper swallows errors |
| `resetHistoryByAnimeId` | `UPDATE animehistory SET last_seen = 0 WHERE _id IN (SELECT H._id FROM animes M INNER JOIN episodes EP ON M._id = EP.anime_id INNER JOIN animehistory H ON EP._id = H.episode_id WHERE M._id = :animeId)` | "remove all history of this entry" |
| `removeAllHistory` | `DELETE FROM animehistory` | "Clear history"; wrapper returns `true`/`false` |
| `removeResettedHistory` | `DELETE FROM animehistory WHERE last_seen = 0` | physical cleanup (called by clear-database) |
| `upsert` | `INSERT INTO animehistory(episode_id, last_seen) VALUES (:episodeId, :seenAt) ON CONFLICT(episode_id) DO UPDATE SET last_seen = :seenAt WHERE episode_id = :episodeId` | `seenAt` bound through `DateColumnAdapter` (ms). A previously reset row (0) is revived by the next upsert |

`AnimeHistoryRepositoryImpl.upsertAnimeHistory` swallows exceptions (logs). Reads: `getHistoryByAnimeId` returns `AnimeHistory(id, episodeId, seenAt: Date?)`.

### 5.2 `animehistoryView` and the two queries

```sql
CREATE VIEW animehistoryView AS
SELECT animehistory._id AS id, animes._id AS animeId, episodes._id AS episodeId, animes.title,
       animes.thumbnail_url AS thumbnailUrl, animes.source, animes.favorite, animes.cover_last_modified,
       episodes.episode_number AS episodeNumber, animehistory.last_seen AS seenAt,
       max_last_seen.last_seen AS maxSeenAt, max_last_seen.episode_id AS maxSeenAtEpisodeId
FROM animes
JOIN episodes ON animes._id = episodes.anime_id
JOIN animehistory ON episodes._id = animehistory.episode_id
JOIN (SELECT episodes.anime_id, episodes._id AS episode_id, MAX(animehistory.last_seen) AS last_seen
      FROM episodes JOIN animehistory ON episodes._id = animehistory.episode_id
      GROUP BY episodes.anime_id) AS max_last_seen
ON episodes.anime_id = max_last_seen.anime_id;
```

Algorithm: the inner subquery yields, per anime, the maximum `last_seen` and (SQLite bare-column rule) the `episode_id` of the row holding that maximum. The outer join returns one row for EVERY history row of the anime, each carrying the same `maxSeenAtEpisodeId`.

`animehistory` query: `SELECT id, animeId, episodeId, title, thumbnailUrl, source, favorite, cover_last_modified, episodeNumber, seenAt FROM animehistoryView WHERE seenAt > 0 AND maxSeenAtEpisodeId = episodeId AND lower(title) LIKE ('%' || :query || '%') ORDER BY seenAt DESC`. Net rule: one row per anime, the row of its most recently watched episode, only if that time is greater than 0, newest first, with a case-insensitive (ASCII) substring filter on the title. The query text is passed raw (the history screen model passes `query ?: ""`), so a typed `%` or `_` acts as a LIKE wildcard (verified: query `%` matches everything).

`getLatestAnimeHistory`: same columns, `WHERE seenAt > 0 ORDER BY seenAt DESC LIMIT 1` (does not check `maxSeenAtEpisodeId`; the newest row overall is by definition its anime's max). Used by `GetNextEpisodes.await()` ("resume" / "next episode" shortcut).

Quirks (verified): (a) resetting the newest row of an anime (`last_seen = 0`) makes the NEXT newest row of that anime become its history entry, because the subquery maximum is recomputed; the anime does not vanish from history until all its rows are 0. (b) When two episodes of one anime have exactly the same `last_seen`, exactly one of them equals `maxSeenAtEpisodeId`, so one row is shown. (c) On a fresh database `last_seen` is nullable; NULL fails `seenAt > 0`.

Mapping (`AnimeHistoryMapper.mapAnimeHistoryWithRelations`): row to `AnimeHistoryWithRelations(id, episodeId, animeId, title, episodeNumber, seenAt, coverData = AnimeCover(animeId, sourceId = source, isAnimeFavorite = favorite, url = thumbnailUrl, lastModified = cover_last_modified))`. The history screen groups rows by date (area:42-ui-updates-history-stats-storage-upcoming).

History write triggers: none. The player upserts `last_seen = now_ms` when playback of an episode ends or is left (area:45-player-core).

## 6. Library and statistics views

Full text of each view is in `p1-topic-views-and-aggregates.md`. Behaviour:

**`episodestatsView`** (per anime with at least one episode): `total = count(*)`, `seenCount = sum(seen)`, `latestUpload = coalesce(max(date_upload), 0)`, `fetchedAt = coalesce(max(date_fetch), 0)`, `bookmarkCount = sum(bookmark)`, `fillermarkCount = sum(fillermark)`. Anime without episodes have no row (LEFT JOINs then give NULL, which `coalesce` turns into 0).

**`animehistorystatsView`**: per anime with at least one episode, `lastSeen = max(ah.last_seen)` through a LEFT JOIN on history (NULL when never watched).

**`animeseasonstatsView`** (per `parent_id` that has children): `child_count = count(*)`, `fully_seen_seasons = sum(CASE WHEN season.fetch_type = 1 AND coalesce(ES.total,0) = coalesce(ES.seenCount,0) THEN 1 ELSE 0 END)`, `max_latest_upload`, `max_fetched_at`, `max_last_seen` (all `max(coalesce(x,0))`), `total_bookmarks`, `total_fillermarks` (`sum(coalesce(...,0))`). Children are all `animes` rows with that `parent_id`, favorite or not. Quirk: a child season with zero episodes has `0 = 0` and counts as fully seen; a child with `fetch_type = 0` (a season that itself holds seasons) never counts.

**`animelibView`** (WHERE `M.favorite = 1`): `M.*` first, then `totalCount, seenCount, latestUpload, episodeFetchedAt, lastSeen, bookmarkCount, fillermarkCount` each as `CASE M.fetch_type WHEN 1 THEN <episode stat> WHEN 0 THEN <season aggregate> END` with the `coalesce(..., 0)` wrappers (so an entry with `fetch_type` outside {0,1} yields NULL), then `category = coalesce(MC.category_id, 0)` from a LEFT JOIN on `animes_categories`. Because of that join an anime in N categories yields N rows and one in none yields one row with `category = 0`. Measured example: a parent (fetch_type 0) with children S1 (2 episodes, both seen), S2 (2 episodes, 1 seen, 1 bookmarked) and S3 (0 episodes) gives `totalCount = 3`, `seenCount = 2` (S1 and the empty S3), `bookmarkCount = 1`. Query `animelib`: `SELECT * FROM animelibView`. Mapping: `seenCount`, `bookmarkCount`, `fillermarkCount` are read as `Double` (SQLite `sum` of integers can be typed REAL by SQLDelight) and `.toLong()`; others `Long`.

**`animeseasonsView`** (WHERE `M.parent_id IS NOT NULL`): the same computed columns without `category`; query `getAnimeSeasonsById: SELECT * FROM animeseasonsView WHERE parent_id = :parentId` (no ORDER BY; the UI sorts by `season_flags`). Mapping `mapSeasonAnime` reads `fetchedAt` for the column named `episodeFetchedAt`.

## 7. Categories: `categories`, `animes_categories`

### 7.1 Statements

| Statement | SQL | Notes |
|---|---|---|
| `getCategory` | `SELECT * FROM categories WHERE _id = :id LIMIT 1` | |
| `getCategories` | `SELECT _id AS id, name, sort AS \`order\`, flags, hidden FROM categories ORDER BY sort` | includes the system category (sort -1 first) |
| `getVisibleCategories` | same with `WHERE hidden = 0` | |
| `getCategoriesByAnimeId` | `SELECT C._id AS id, C.name, C.sort AS \`order\`, C.flags, C.hidden FROM categories C JOIN animes_categories AC ON C._id = AC.category_id WHERE AC.anime_id = :animeId` | no ORDER BY |
| `getVisibleCategoriesByAnimeId` | same plus `AND C.hidden = 0` | |
| `insert` | `INSERT INTO categories(name, sort, flags) VALUES (:name, :order, :flags)` | `hidden` defaults to 0; id is auto-assigned (rowid max + 1, minimum 1 because id 0 exists) |
| `delete` | `DELETE FROM categories WHERE _id = :categoryId` | blocked for `_id <= 0` by trigger |
| `update` | `UPDATE categories SET name = coalesce(:name, name), sort = coalesce(:order, sort), flags = coalesce(:flags, flags), hidden = coalesce(:hidden, hidden) WHERE _id = :categoryId` | |
| `updateAllFlags` | `UPDATE categories SET flags = coalesce(?, flags)` | positional parameter; NULL leaves flags untouched |
| `selectLastInsertedRowId` | | restore of categories |
| link `insert` | `INSERT INTO animes_categories(anime_id, category_id) VALUES (:animeId, :categoryId)` | trigger bumps the anime's `version` |
| link `deleteAnimeCategoryByAnimeId` | `DELETE FROM animes_categories WHERE anime_id = :animeId` | no trigger |

`AnimeCategoryRepositoryImpl`: `insertAnimeCategory` runs `insert(name, order, flags)` WITHOUT a transaction and ignores `hidden`; `updatePartialAnimeCategory` runs one `update`; `updatePartialAnimeCategories` runs a list in one transaction (`hidden` bool mapped to 1/0); `updateAllAnimeCategoryFlags(flags)` runs `updateAllFlags`; `deleteAnimeCategory(id)` runs `delete`. Mapping `hidden == 1L`.

`AnimeRepositoryImpl.setAnimeCategories(animeId, ids)`: in one transaction `deleteAnimeCategoryByAnimeId` then `insert` per id. The insert trigger fires once per id, so saving `n` categories bumps `animes.version` by `n` (unless `is_syncing = 1`); saving an empty list bumps nothing and leaves the entry in the implicit default category (`category = 0` in the library view).

The system category: id 0, `name = ''`, `sort = -1`, `flags = 0`, created by `INSERT OR IGNORE`; undeletable (trigger `system_category_delete_trigger`, message `System category can't be deleted`, `RAISE(ABORT, ...)`). It is "default" in the UI sense of "uncategorized"; the user-visible name comes from `MR.strings.label_default` (area:51-i18n) when `Category.isSystemCategory` (`id == UNCATEGORIZED_ID = 0`).

### 7.2 Interactor algorithms that decide the stored numbers

* Create (`CreateAnimeCategoryWithName`): `nextOrder = max(order over ALL categories incl. system) + 1`, or 0 when the list is empty; `flags = sort.type.flag or sort.direction.flag` from preference `animelib_sorting_mode`; `hidden = false`. With only the system category present the first user category gets `order = 0` (max is -1, plus 1).
* Rename: partial update of `name`. Hide/unhide (`HideAnimeCategory`): partial update `hidden = !category.hidden`.
* Reorder (`ReorderAnimeCategory`, under a Mutex): take all categories WITHOUT the system category, remove the moved one and re-insert it at `newIndex`, write `order = index` for every listed category in one transaction (system category keeps `-1`).
* Delete (`DeleteAnimeCategory`): (1) `delete(categoryId)`; failure returns `Result.InternalError` and stops. (2) fetch ALL categories (system category included) and write `order = index` to each in list order, so after the first delete the system category's `sort` becomes 0 and user categories become 1..n (OBSERVED quirk: the system row is renumbered because the list includes it). (3) if preference `default_anime_category == categoryId.toInt()`, delete that preference (back to `-1`). (4) remove the id string from each of the five sets `animelib_update_categories`, `animelib_update_categories_exclude`, `remove_exclude_anime_categories`, `download_new_anime_categories`, `download_new_anime_categories_exclude` when present. (5) apply the order updates in one transaction. Failure of (5) returns `InternalError`. The anime-category links of the deleted category disappear by cascade.
* Sort (`SetSortModeForAnimeCategory`): `flags = (category?.flags ?: 0) + type + direction` using `Long.plus(Flag)` (mask-aware replace); when type is Random set `library_random_anime_sort_seed = Random.nextInt()`; if the category exists and preference `categorized_display` is true write only that category's `flags`; otherwise set `animelib_sorting_mode` and write `updateAllFlags(flags)` to every category. `ResetAnimeCategoryFlags` writes `sort.type + sort.direction` to all.
* Restore (`AnimeCategoriesRestorer`): `nextOrder = max(order of DB categories) + 1` (0 when none); backup categories sorted by their `order`; categories whose NAME already exists are reused, others are inserted with the next order and the backup `flags`; afterwards preference `categorized_display` is set to `true` when the union of categories has more than one distinct `flags` value.

## 8. Tracks: `anime_sync`

| Statement | SQL |
|---|---|
| `delete` | `DELETE FROM anime_sync WHERE anime_id = :animeId AND sync_id = :syncId` |
| `getAnimeTracks` | `SELECT * FROM anime_sync` |
| `getTrackByAnimeId` | `SELECT * FROM anime_sync WHERE _id = :id` (misnamed: filters by track row id) |
| `getTracksByAnimeId` | `SELECT * FROM anime_sync WHERE anime_id = :animeId` |
| `insert` | 13 columns `anime_id, sync_id, remote_id, library_id, title, last_episode_seen, total_episodes, status, score, remote_url, start_date, finish_date, private` |
| `update` | coalesce update of those columns plus parameter renames: `:mediaId` for `remote_id`, `:trackingUrl` for `remote_url`; `WHERE _id = :id` |

Behaviour: `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE` makes `insert` an upsert by REPLACING the row, with a fresh `_id` (verified: `_id` 1 became 2). `AnimeTrackRepositoryImpl.insertValues` runs a list in one transaction. Restore (`AnimeRestorer.restoreTracking`): for each backup track find the DB track by `trackerId`; none means insert with `id = 0` and the new `animeId`; equal ignoring `id`/`animeId` means skip; otherwise keep the DB row and take `remoteId`, `libraryId` from the backup and `lastEpisodeSeen = max(db, backup)`; existing ones are written with `update(animeId, trackerId, remoteId, libraryId, title, lastEpisodeSeen, totalEpisodes, status, score, remoteUrl, startDate, finishDate, private, id)`.

## 9. Feeds: updates, upcoming, deletable parents

**`animeupdatesView`**: `SELECT animes._id AS animeId, animes.title AS animeTitle, episodes._id AS episodeId, episodes.name AS episodeName, episodes.scanlator, episodes.seen, episodes.bookmark, episodes.fillermark, episodes.last_second_seen, episodes.total_seconds AS totalSeconds, animes.source, animes.favorite, animes.thumbnail_url AS thumbnailUrl, animes.cover_last_modified AS coverLastModified, episodes.date_upload AS dateUpload, episodes.date_fetch AS datefetch FROM animes JOIN episodes ON animes._id = episodes.anime_id WHERE favorite = 1 AND date_fetch > date_added ORDER BY date_fetch DESC`.
Rule: only library entries; only episodes fetched strictly after the entry was added (`date_fetch > date_added`, so episodes found before adding, or in the same millisecond, never show). Queries: `getRecentAnimeUpdates: SELECT * FROM animeupdatesView WHERE dateUpload > :after LIMIT :limit` and `getUpdatesBySeenStatus: SELECT * FROM animeupdatesView WHERE seen = :seen AND dateUpload > :after LIMIT :limit`. The filter is on `dateUpload` (the source's publish time), while the order is `date_fetch DESC` carried by the view. `GetAnimeUpdates` passes `limit = 500`; `subscribe(instant)` passes `instant.toEpochMilli()` as `after`. The updates screen builds `after` as "now minus 3 months" (area:42-ui-updates-history-stats-storage-upcoming). Verified: SQLite keeps the view's ORDER BY for such outer queries; Proposal: write `ORDER BY datefetch DESC` explicitly in the Rust statements because a view's ORDER BY is not guaranteed.

**Upcoming** (`getUpcomingAnime`, section 3.1): entries with a computed next release time today or later.

**`animedeletableView`**: `SELECT M._id, M.source, M.fetch_type FROM animes AS M WHERE M.favorite = 0 AND M.parent_id IS NULL`; query `getDeletableParentAnime: SELECT * FROM animedeletableView` (a flow). Season children (non-null `parent_id`) are intentionally excluded; the clear-database operation reaches them through `getChildrenByParentId`.

## 10. Small tables

**`animesources`** (`findAll`, `findOne WHERE _id = :id`, `upsert`): `INSERT INTO animesources(_id, lang, name) VALUES (:id, :lang, :name) ON CONFLICT(_id) DO UPDATE SET lang = :lang, name = :name WHERE _id = :id`. `AndroidAnimeSourceManager.registerStubSource` (run for every source of every loaded extension): read the DB row; if equal to the current `StubAnimeSource` do nothing; else upsert; and if a row existed (rename), call `downloadManager.renameSource(old, new)` so download folders follow the new name. `createStubSource(id)`: DB row, else extension data, else `StubAnimeSource(id, lang = "", name = "")`.

**`anime_relations`**: `getRelatedAnimeByAnimeId: SELECT A.*, R.name FROM anime_relations AS R JOIN animes AS A ON A._id = R.related_anime_id WHERE R.anime_id = :animeId ORDER BY R.sort_order ASC`; `getLastFetchedAt: SELECT last_fetched_at FROM anime_relations WHERE anime_id = :animeId LIMIT 1`; `insert: INSERT OR REPLACE INTO anime_relations(...)`; `deleteByAnimeId`. `replaceRelations(animeId, groups, fetchedAt)`: one transaction: `deleteByAnimeId`; `sortOrder = 0`; for each group in order and each related id in order insert `(animeId, relatedId, groupName, sortOrder++, fetchedAt)`. `subscribeRelatedAnime` maps rows to groups with `groupBy` on the name, preserving first-appearance order. The primary key `(anime_id, related_anime_id, name)` means the same related anime can appear in several groups. The related anime rows themselves are `animes` rows created by `NetworkToLocalAnime` (non-favorite).

**`extension_store`**: `upsert` writes all 8 columns and on conflict on `index_url` updates the other 7; `get`, `getAll`, `getCount: SELECT COUNT(*)`, `delete WHERE index_url = :indexUrl`. `AnimeExtensionStoreRepositoryImpl`: `insert(indexUrl)` fetches the store (`AnimeExtensionStoreService.fetch`) then upserts it, returning `Result<Unit>`; `insertFromPreference(indexUrl, name)` upserts a placeholder (`badgeLabel = name`, `signingKey = "NO_SIGNING_KEY"`, `contactWebsite = indexUrl`, `contactDiscord = null`, `isLegacy = false`, `extensionListUrl = null`) used by `TrustExtensionRepositoryMigration` (`name = "Repo #<index+1>"`, url = preference url without a trailing `/index.min.json` or `/index.json` plus `/repo.json`); `refreshAll()` iterates stores and refetches each, logging failures `Failed to refresh extension store '<name> (<index_url>)'`; `fetchExtensions()` fetches the extension list of every store in parallel under `supervisorScope`, concatenating the successes and logging `Failed to fetch extensions for store '<name> (<indexUrl>)'` for failures; `remove(indexUrl)` deletes. A SQLDelight quirk: the source writes `signing_key =: signingKey` in the upsert; SQLDelight still binds it correctly (verified with SQLite: `=:name` parses as `=` and a parameter).

## 11. Custom buttons (`custom_buttons`)

Statements: `findAll: SELECT * FROM custom_buttons ORDER BY sortIndex`, `insert (name, isFavorite, sortIndex, content, longPressContent, onStartup)`, `delete WHERE _id = :customButtonId`, `update` (coalesce on the six columns, `WHERE _id = :customButtonId`), `selectLastInsertedRowId`.

Repository: `insertCustomButton(name, sortIndex, content, longPressContent, onStartup)` always inserts `isFavorite = false`; a `SQLiteException` is converted to `SaveCustomButtonException` (message `Error Saving Custom Button to Database`, an `IOException`). Domain rules (all `withNonCancellableContext`, results `Success` / `InternalError(e)` and the UI shows `MR.strings.internal_error` = "InternalError: Check crash logs for further information"):
* Create: `sortIndex = max(sortIndex) + 1` (0 when empty).
* Delete: delete the row, re-read all, write `sortIndex = index` for each in one transaction.
* Reorder (Mutex): move the item to `newIndex`, rewrite all indexes; `Unchanged` when the id is not found.
* Toggle favorite (Mutex for turning on): if currently favorite set `isFavorite = false` for it only; else set `isFavorite = (id == this id)` for every button in one transaction, so exactly one favorite remains. The favorite is the "primary" button shown on the player's control row (`PlayerViewModel`: first favorite).
* Update: partial update of any subset of the six columns.
* Script expansion (`CustomButton.getButtonContent(primaryId)` and the two siblings): `text.replace("$id", id.toString()).replace("$isPrimary", if (primaryId == id) "true" else "false")` for content, long-press content and startup code respectively; `PlayerActivity.setupCustomButtons` concatenates, for each button, its startup code, `function button<id>() <content> end`, `mp.register_script_message('call_button_<id>', button<id>)`, `function button<id>long() <longPressContent> end`, `mp.register_script_message('call_button_<id>_long', button<id>long)` into the file `custombuttons.lua` in the app's mpv scripts directory (prefixed with a `package.path` setup and `local aniyomi = require 'aniyomi'`).
* Restore (`CustomButtonRestorer`): for backup buttons ordered by `sortIndex`: a DB button with the same NAME is kept as is; otherwise insert with `sortIndex = nextSortIndex++` (`max + 1` of the DB), `isFavorite = backup.isFavorite && !dbHasFavorite`.

## 12. Episode name sanitizer (`EpisodeSanitizer`, `data/src/main/java/tachiyomi/data/items/episode/EpisodeSanitizer.kt`)

`String.sanitize(title)`: `trim()` (Kotlin whitespace), `removePrefix(title)`, then `trim(*EPISODE_TRIM_CHARS)`. `EPISODE_TRIM_CHARS` = the 30 characters: space `U+0020`, `U+0009`, `U+000A`, `U+000B`, `U+000C`, `U+000D`, `U+0020` (a second time), `U+0085`, `U+00A0`, `U+1680`, `U+2000`, `U+2001`, `U+2002`, `U+2003`, `U+2004`, `U+2005`, `U+2006`, `U+2007`, `U+2008`, `U+2009`, `U+200A`, `U+2028`, `U+2029`, `U+202F`, `U+205F`, `U+3000`, then `-`, `_`, `,`, `:` (the list has 30 entries counting the duplicate space). Effect: an episode named `One Piece - 1000` for the anime `One Piece` is stored as `1000`. Applied by the source layer when episodes are created from `SEpisode` (area:07-domain-entries-items), not by SQL.

## 13. Clear anime database (settings > advanced)

OBSERVED (`ClearAnimeDatabaseScreenModel`): subscribe to `getDeletableParentAnime`; group by `source`; for each source collect `ids` (the deletable parents) and, for parents whose `fetch_type` is `Seasons`, recurse through `getChildrenByParentId`: a favorite child goes to `orphaned`, a non-favorite child goes to `ids` and, if its own `fetch_type` is `Seasons`, is recursed into. Items are sorted by source name. Confirmation dialog text `AYMR.strings.clear_database_confirmation` ("Are you sure? Completed episodes and chapters and progress of non-library entries will be lost"), buttons `MR.strings.action_ok` / `MR.strings.action_cancel`; a screen with no deletable rows shows `MR.strings.database_clean` ("Nothing to clear"); each source row shows `MR.strings.clear_database_source_item_count` ("%1$d non-library entries in database"); app bar title `AYMR.strings.pref_clear_anime_database` ("Clear Anime database", summary "Delete history for anime that are not saved in your library"). On confirm (`withNonCancellableContext`): `animeIds = selected.flatMap(ids)`, `orphaned = selected.flatMap(orphaned) minus animeIds`; run `deleteAnimesNotInLibraryByAnimeIds(animeIds)`, `removeParentIdByIds(orphaned)`, `removeResettedHistory()` (three separate statements, no common transaction); toast `MR.strings.clear_database_completed` ("Entries deleted"). Note the third statement deletes ALL reset history rows, also for library entries. Proposal: wrap the three statements in one transaction (T-04a-021).

## 14. Restore interplay: `is_syncing` and `version`

OBSERVED sequence (`AnimeRestorer.restore`, one transaction per anime via `handler.await(inTransaction = true)`):
1. `findExistingAnime` by `(url, source)`.
2. New: insert (`restoreNewAnime`: `initialized = description != null`, `next_update = 0`, `calculate_interval = 0`, version from backup). Existing: if `backup.version > db.version` the DB row is updated from the backup, else from the DB side (`copyFrom`: `favorite = this.favorite || newer.favorite`, author, artist, description, genre, thumbnail, status, `initialized = this.initialized || newer.initialized`, `version = newer.version`, `fetchType`, `parentId`); the update writes `isSyncing = 1`, `nextUpdate = null`, `calculateInterval = null` and the chosen `version`, so the version triggers do not bump it.
3. Seasons: each `backupSeasons` entry is restored the same way with `parentId = restoredAnime.id`.
4. Details: categories (mapped by backup order to DB category by name, then delete+insert links in one transaction; only when at least one maps), episodes (existing ones compared via `forComparison`; `seen` is never un-set, `lastSecondSeen` is never lowered to 0, bookmark/fillermark are OR-ed; new episodes inserted; existing updated with `isSyncing = 0` and the backup `version`), tracks (section 8), history (for each: look up by episode url, `seenAt = max(backup, db)` when it exists else only if the episode exists), then `updateAnime.awaitUpdateFetchInterval` which writes `isSyncing = 0` (the `partialUpdateAnime` default).
Result: during the restore the anime row carries `is_syncing = 1` so the category-link insert trigger and episode trigger do not bump the anime version; the final partial update clears the flag. Episode rows restored through `update` with `isSyncing = 0` and a changed `seen`, `bookmark` or `last_second_seen` DO get `version + 1` on top of the backup value (OBSERVED consequence of the trigger).

## 15. Concurrency, dispatchers, timings, limits

* Dispatcher: `Dispatchers.IO` for queries and flows; transactions on a pinned thread. SQLDelight serialises transactions; SQLite WAL permits concurrent readers.
* Limits: updates feed `limit = 500`; no other row limits. Library queries return every favorite (thousands of rows are normal).
* Timings: none in SQL. Proposal timings: change-bus coalescing window 50 ms, `busy_timeout` 5000 ms, `wal_checkpoint(PASSIVE)` every 1000 committed writes.
* Cancellation: domain interactors that mutate (`DeleteAnimeCategory`, `ReorderAnimeCategory`, custom-button interactors, `ClearAnimeDatabase`) run inside `withNonCancellableContext` so leaving the screen does not abort a half-applied reorder. Proposal: run the equivalent Rust writes in `spawn_blocking` tasks that are not tied to widget lifetime.
* Id generation: `selectLastInsertedRowId` must run on the same connection and in the same transaction as the preceding insert; in Rust use `tx.last_insert_rowid()`.

## 16. Interactions with other areas

| Area | Interaction |
|---|---|
| area:03-db-manga-schema-migrations | twin schema, same pragmas/handler design; shared `kuro-db::core` |
| area:05-data-repos-entries-items, area:06-data-repos-history-track-source-misc | the Kotlin repository classes whose SQL calls are specified here; their Rust ports build on `kuro-db::anime::*` |
| area:07-domain-entries-items, area:08-domain-library-category-history-updates | interactors (`AnimeFetchInterval`, flag setters, category interactors) deciding the values written |
| area:22-backup-restore, area:21-backup-format-and-create | read and write these tables directly (sections 8, 11, 14) |
| area:20-extension-strategy-redesign, area:19-extensions-anime | consume `extension_store` |
| area:27-tracking-core | owns the meaning of `anime_sync.status` and `sync_id` |
| area:45-player-core, area:46-player-controls-ui, area:48-settings-player-and-scripts | write episode progress and history; execute custom-button Lua |
| area:37-ui-library, area:39-ui-entry-details-anime, area:42-ui-updates-history-stats-storage-upcoming | consume the views |
| area:02-preferences-system | the six category-related preference keys |

## 17. Error paths and message keys

| Situation | Behaviour | User-visible text |
|---|---|---|
| Missing row on `getAnimeById` | `awaitOne` throws; `GetAnime.await` logs and returns null | none |
| Update/insert failure in `AnimeRepositoryImpl.updateAnime` | log, return false | caller decides (usually none) |
| `addAllEpisodes` failure | log, return empty list; transaction rolled back | none |
| Category delete of id 0 | trigger aborts with `System category can't be deleted`; `DeleteAnimeCategory` returns `Result.InternalError` | `MR.strings.internal_error` |
| Category interactor failures | `Result.InternalError(e)` | `MR.strings.internal_error` |
| Category name exists | UI-level check | `MR.strings.error_category_exists` |
| Custom button save failure | `SaveCustomButtonException("Error Saving Custom Button to Database")`; interactor returns `InternalError` | `MR.strings.internal_error` |
| Extension store fetch failure | `Result.failure`, log `Failed to add extension store '<url>'` | shown by the extensions UI (area:41-ui-browse-anime) |
| History reset/remove failure | log only | none |
| Memo cell invalid JSON | `Json.decodeFromString` throws and the query fails | none (crash path); Proposal: `DbError::BadMemo` and the row is read with an empty object plus a logged warning |
