# Area 05 — topic: anime seasons (parent/child entries) and related-anime groups

Paths repo-relative to `/home/blackbox/aniyomi-src`. Manga has neither concept. Everything here is anime-only and sits in `AnimeRepositoryImpl.kt`, `AnimeMapper.kt`, `AnimeRelationRepositoryImpl.kt` (all under `data/src/main/java/tachiyomi/data/entries/anime/`).

## 1. The season model (OBSERVED)

- An anime row has `fetch_type`: `Episodes` (1, the default, the row has an episode list) or `Seasons` (0, the row is a container whose "chapters" are other anime rows). The doc comment in `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt` says the type does not change after the entry is initialised.
- A child season is an ordinary `animes` row with `parent_id = <parent _id>`, `season_number` (REAL, default -1.0 in `Anime.create()`), `season_source_order` (INTEGER, position in the source's season list), and its own `fetch_type` (a season may itself be of type Seasons, giving nesting).
- `parent_id` has no FOREIGN KEY, but it has an index (`animes_parent_id`). Deleting a parent leaves its children with a dangling `parent_id`.
- `animelibView` filters only on `favorite = 1`; whether a child season appears in the library depends on whether it is favorited and on UI filters (area:37-ui-library). The repository does not filter on `parent_id`.

## 2. Repository operations

| Method | SQL | Notes |
|---|---|---|
| `getAnimeSeasonsById(parentId): List<SeasonAnime>` | `SELECT * FROM animeseasonsView WHERE parent_id = :parentId` | Includes aggregated stats per child; see `p1-topic-library-and-season-views.md` section 3. No ORDER BY: callers sort with `SeasonSorter` (area:07-domain-entries-items) using `seasonFlags`. |
| `getAnimeSeasonsByIdAsFlow(parentId)` | same, subscription | depends on `animes, episodes, animehistory` |
| `getChildrenByParentId(parentId): List<Anime>` | `SELECT * FROM animes WHERE parent_id = :parentId` | plain rows, used by the clear-database screen and nested deletion |
| `removeParentIdByIds(animeIds)` | `UPDATE animes SET parent_id = NULL WHERE _id IN :animeIds` | exceptions are caught and logged (`logcat(LogPriority.ERROR, e)`), returns Unit; used by `SyncSeasonsWithSource` for seasons that disappeared from the source and by the clear-database screen for favorited children |
| `getDeletableParentAnime(): Flow<List<DeletableAnime>>` | `SELECT * FROM animedeletableView` | top-level non-favorite anime |

`SyncSeasonsWithSource` (app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt, area:10-app-domain-layer) is the writer: for each source season it calls `networkToLocalAnime.await(...)` (insert-if-missing by `(url, source)`), then `.copy(parentId = anime.id, seasonSourceOrder = i)`; new/changed seasons are persisted with `updateAnime.awaitAll(listOf(AnimeUpdate…))` where `AnimeUpdate.parentId` is set (the coalesce update can set a parent, never clear one); seasons that vanished get `removeParentIdByIds`. Changed means `ShouldUpdateDbSeason`: `title`, `seasonNumber` or `seasonSourceOrder` differ (domain/src/main/java/tachiyomi/domain/items/season/interactor/ShouldUpdateDbSeason.kt).

## 3. Clear-database flow that depends on these queries

OBSERVED in app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearAnimeDatabaseScreen.kt:215-300:

1. Subscribe to `getDeletableParentAnime()`; group by `sourceId`.
2. For each deletable row, add its id to the source's delete list; if its `fetchType == Seasons`, recurse over `getChildrenByParentId`: a child that is favorite is added to `orphaned` (it must survive, so its `parent_id` will be nulled), a non-favorite child is added to the delete list and, if it is itself of type Seasons, recursed into.
3. On confirm: `deleteAnimesNotInLibraryByAnimeIds(animeIds)` (`DELETE FROM animes WHERE favorite = 0 AND _id IN :animeIds`, cascades to episodes, categories links, relations), `removeParentIdByIds(orphaned.filterNot { it in animeIds })`, then `animehistoryQueries.removeResettedHistory()`.

Proposal: expose these as repository methods on `AnimeRepo` so the Rust clear-database screen (area:47-settings-screens-general) does not run raw SQL: `delete_non_library_by_ids(ids)` (same `favorite = 0` guard in the SQL) and the existing `remove_parent_ids`. Tracked by T-05a-031.

## 4. Related-anime relations (`anime_relations`)

### 4.1 Table semantics

Primary key `(anime_id, related_anime_id, name)`; `name` is the group title the source returned (for example "Sequels"); `sort_order` is a single running counter across all groups of one anime (0, 1, 2, ... in group order then item order); `last_fetched_at` is the same epoch-millis value on every row of one replacement. Both foreign keys cascade on delete, so deleting either anime removes its relation rows.

### 4.2 Methods

`getLastFetchedAt(animeId): Long?` = `SELECT last_fetched_at FROM anime_relations WHERE anime_id = :animeId LIMIT 1` (any row; all rows of one anime share the value; null when the anime has never been fetched or the last fetch returned no groups, because `replaceRelations` with zero groups deletes everything and inserts nothing).

`replaceRelations(animeId, groups: List<Pair<String, List<Long>>>, fetchedAt)` runs in one transaction (`handler.await(inTransaction = true)`):

1. `DELETE FROM anime_relations WHERE anime_id = :animeId`.
2. `sortOrder = 0`; for each group `(name, relatedIds)` in list order, for each `relatedId` in list order: `INSERT OR REPLACE INTO anime_relations(anime_id, related_anime_id, name, sort_order, last_fetched_at) VALUES (animeId, relatedId, name, sortOrder++, fetchedAt)`.
3. `INSERT OR REPLACE` with the composite PRIMARY KEY means a repeated `(relatedId, name)` pair inside one group replaces the earlier row, keeping the later `sort_order` (the earlier counter value is then skipped, leaving a gap, which is harmless for `ORDER BY`). The caller already de-duplicates by url within a group.

`subscribeRelatedAnime(animeId): Flow<List<AnimeRelationGroup>>`: subscribes to `getRelatedAnimeByAnimeId` (join to `animes`, `ORDER BY R.sort_order ASC`), maps each row through `AnimeMapper.mapRelatedAnime` to `(name, Anime)`, then `groupBy(name)` preserving encounter order into `AnimeRelationGroup(name, anime)`. Tables read: `anime_relations, animes`. Group order is the order in which each name first appears. If the same name were stored for two non-adjacent runs, `groupBy` merges them into the first position (cannot happen because one source call yields each group once).

### 4.3 Caller contract (SyncRelatedAnimeWithSource, app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncRelatedAnimeWithSource.kt)

- Skip entirely when the anime's source is missing or `supportsRelatedAnime` is false.
- TTL: `7.days.inWholeMilliseconds` = 604 800 000 ms. If `forceRefresh` is false and `lastFetchedAt != null` and `now - lastFetchedAt < TTL`, return without fetching.
- Per group: `relation.animes.distinctBy { it.url }.filterNot { it.url == anime.url }.take(MAX_PER_GROUP)` with `MAX_PER_GROUP = 10`, each converted through `NetworkToLocalAnime` (inserts missing rows as non-favorite anime), keeping group order; groups with no ids are dropped before `replaceRelations`.
- `now = Instant.now().toEpochMilli()` is passed as `fetchedAt`.

Rust API (`AnimeRelationRepo`): `last_fetched_at(anime_id) -> Option<i64>`, `replace_relations(anime_id, groups: &[(String, Vec<i64>)], fetched_at: i64)`, `subscribe_related(anime_id) -> Subscription<Vec<AnimeRelationGroup>>`.

Data-volume note: every fetch inserts up to (groups x 10) non-favorite anime rows that are never cleaned up except by the clear-database screen. Proposal: nothing automatic; list in `p1-edge-cases-and-tests.md` EC-14.
