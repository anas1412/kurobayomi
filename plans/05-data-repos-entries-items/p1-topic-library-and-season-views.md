# Area 05 — topic: library views, season views, deletable-parent view, row grouping

Paths repo-relative to `/home/blackbox/aniyomi-src`. The views are defined in area:03-db-manga-schema-migrations (libraryView) and area:04-db-anime-schema-migrations (animelibView and friends); this area reads them through `getLibraryManga`, `getLibraryAnime`, `getAnimeSeasonsById`, `getDeletableParentAnime`. The mapper files are `data/src/main/java/tachiyomi/data/entries/manga/MangaMapper.kt` and `data/src/main/java/tachiyomi/data/entries/anime/AnimeMapper.kt`.

## 1. Manga: `libraryView` (data/src/main/sqldelight/view/libraryView.sq)

Result columns: all 24 `mangas` columns (`M.*`, in table order), then `totalCount, readCount, latestUpload, chapterFetchedAt, lastRead, bookmarkCount, category`. `MangaMapper.mapLibraryManga` takes them positionally and produces `LibraryManga(manga, category, totalChapters = totalCount, readCount.toLong(), bookmarkCount.toLong(), latestUpload, chapterFetchedAt, lastRead)`.

Semantics (OBSERVED from the SQL):

1. One output row per (favorite manga, category link). A manga linked to 3 categories appears 3 times with different `category` values; a manga with zero links appears once with `category = 0` (`coalesce(MC.category_id, 0)`), i.e. in the system "Default"/uncategorized category. The consumer (library screen model, area:37-ui-library) groups by `category`.
2. Chapter aggregates come from a sub-select grouped by `manga_id` that excludes chapters whose scanlator is in `excluded_scanlators` for that manga (the LEFT JOIN + `WHERE excluded_scanlators.scanlator IS NULL` anti-join). So `totalCount`, `readCount`, `bookmarkCount`, `latestUpload`, `chapterFetchedAt` and `lastRead` all ignore excluded scanlators. A chapter with a NULL scanlator is never excluded (`chapters.scanlator = excluded_scanlators.scanlator` is NULL for NULL, the join finds no match, and `excluded_scanlators.scanlator IS NULL` is true).
3. `lastRead` = `coalesce(max(history.last_read), 0)` over the joined `history` rows (one `history` row per chapter, `chapter_id` unique). `history.last_read` is epoch milliseconds (DateColumnAdapter).
4. `readCount = sum(read)` and `bookmarkCount = sum(chapters.bookmark)` are SQLite integer sums; SQLDelight types them as `Double`, so the mapper converts with `.toLong()`. A manga with no chapters gets 0 for every aggregate (outer `coalesce(..., 0)`).
5. `latestUpload = coalesce(max(chapters.date_upload), 0)`: the most recent upload timestamp (ms), 0 when no chapters.
6. `WHERE M.favorite = 1` is inside the view; non-library manga never appear.

Proposal (`kuro_core::data::manga::repo::library()`):

```rust
pub struct LibraryManga {
    pub manga: Manga,
    pub category: i64,
    pub total_chapters: i64,
    pub read_count: i64,
    pub bookmark_count: i64,
    pub latest_upload: i64,
    pub chapter_fetched_at: i64,
    pub last_read: i64,
}
```

Read the seven aggregate columns as `f64` then `as i64` (mirrors `Double.toLong()` and tolerates SQLite returning either storage class). `unread_count()` = `total_chapters - read_count`, `has_bookmarks()` = `bookmark_count > 0`, `has_started()` = `read_count > 0`.

Performance (single-user desktop, libraries up to about 5 000 entries and 500 000 chapters): the view re-aggregates all chapters on every call and on every notification of `mangas`, `chapters`, `mangas_categories`, `excluded_scanlators`, `history`. Proposal: the subscription debounces notification bursts by 100 ms (a chapter-mark-read of 200 chapters triggers one re-query, not 200) and runs on the reader connection (WAL: does not block the writer). Do not materialise the view.

## 2. Anime: `animelibView` (data/src/main/sqldelightanime/view/animelibView.sq)

Result columns: all 32 `animes` columns (`M.*`), then `totalCount, seenCount, latestUpload, episodeFetchedAt, lastSeen, bookmarkCount, fillermarkCount, category` (8 columns, total 40), matching `AnimeMapper.mapLibraryAnime`.

Each stat column is `CASE M.fetch_type WHEN 1 THEN <episode stat> WHEN 0 THEN <season stat> END`:

| Column | fetch_type = 1 (Episodes) | fetch_type = 0 (Seasons) |
|---|---|---|
| totalCount | `coalesce(ES.total, 0)` = number of episodes | `coalesce(ASS.child_count, 0)` = number of child season rows (`animes.parent_id = M._id`) |
| seenCount | `coalesce(ES.seenCount, 0)` = `sum(seen)` | `coalesce(ASS.fully_seen_seasons, 0)` |
| latestUpload | `coalesce(ES.latestUpload, 0)` = `max(date_upload)` | `coalesce(ASS.max_latest_upload, 0)` |
| episodeFetchedAt | `coalesce(ES.fetchedAt, 0)` = `max(date_fetch)` | `coalesce(ASS.max_fetched_at, 0)` |
| lastSeen | `coalesce(AHS.lastSeen, 0)` = `max(animehistory.last_seen)` over its episodes | `coalesce(ASS.max_last_seen, 0)` |
| bookmarkCount | `coalesce(ES.bookmarkCount, 0)` = `sum(bookmark)` | `coalesce(ASS.total_bookmarks, 0)` |
| fillermarkCount | `coalesce(ES.fillermarkCount, 0)` = `sum(fillermark)` | `coalesce(ASS.total_fillermarks, 0)` |

`category = coalesce(MC.category_id, 0)` from `animes_categories`, same one-row-per-link grouping as manga. `WHERE M.favorite = 1`.

Helper views:

```sql
CREATE VIEW episodestatsView AS
SELECT episodes.anime_id, count(*) AS total, sum(seen) AS seenCount,
       coalesce(max(episodes.date_upload), 0) AS latestUpload,
       coalesce(max(episodes.date_fetch), 0) AS fetchedAt,
       sum(episodes.bookmark) AS bookmarkCount, sum(episodes.fillermark) AS fillermarkCount
FROM episodes GROUP BY episodes.anime_id;

CREATE VIEW animehistorystatsView AS
SELECT episodes.anime_id, max(ah.last_seen) AS lastSeen
FROM episodes LEFT JOIN animehistory AS ah ON episodes._id = ah.episode_id
GROUP BY episodes.anime_id;

CREATE VIEW animeseasonstatsView AS
SELECT season.parent_id, count(*) AS child_count,
  sum(CASE WHEN season.fetch_type = 1 AND coalesce(ES.total, 0) = coalesce(ES.seenCount, 0)
           THEN 1 ELSE 0 END) AS fully_seen_seasons,
  max(coalesce(ES.latestUpload, 0)) AS max_latest_upload, max(coalesce(ES.fetchedAt, 0)) AS max_fetched_at,
  max(coalesce(AHS.lastSeen, 0)) AS max_last_seen,
  sum(coalesce(ES.bookmarkCount, 0)) AS total_bookmarks, sum(coalesce(ES.fillermarkCount, 0)) AS total_fillermarks
FROM animes season
LEFT JOIN episodestatsView ES ON season._id = ES.anime_id
LEFT JOIN animehistorystatsView AHS ON season._id = AHS.anime_id
WHERE season.parent_id IS NOT NULL GROUP BY season.parent_id;
```

Quirks (OBSERVED, preserve):

1. `fully_seen_seasons` counts a child season as fully seen when `coalesce(total,0) = coalesce(seenCount,0)`. A child with zero episodes (never fetched) has 0 = 0 and therefore counts as fully seen. A child whose own `fetch_type` is 0 (nested seasons) never counts.
2. `animehistorystatsView.lastSeen` is NULL for an anime whose episodes have no history; consumers `coalesce(..., 0)`.
3. The episode stats are not filtered by `excluded_scanlators` (anime has no such table).
4. The CASE has no ELSE: a `fetch_type` other than 0 or 1 yields NULL for every stat column. The Kotlin mapper takes non-null `Long` and would crash on such a row. The column adapter decodes unknown values to `Episodes`, so a corrupt value is read as Episodes by the mapper but as NULL stats by the view. Proposal: wrap each stat in `coalesce(..., 0)` when reading (`row.get::<_, Option<f64>>(i)?.unwrap_or(0.0) as i64`) so a corrupt row degrades to zero counts instead of failing the whole library query.
5. Sums are typed Double by SQLDelight (`seenCount.toLong()`), same as manga.

## 3. Anime: `animeseasonsView` (animeseasonsView.sq)

Same CASE columns as `animelibView` but without `category`, from `animes AS M` with `WHERE M.parent_id IS NOT NULL`. Query: `getAnimeSeasonsById: SELECT * FROM animeseasonsView WHERE parent_id = :parentId`. Result: 32 `animes` columns + `totalCount, seenCount, latestUpload, episodeFetchedAt, lastSeen, bookmarkCount, fillermarkCount` (7), mapped by `AnimeMapper.mapSeasonAnime` into `SeasonAnime` (the 4th stat column is named `episodeFetchedAt` in SQL and `fetchedAt` in the mapper; mapping is positional so only the order matters). The result includes non-favorite children (seasons are not "in the library" individually).

Rust: `async fn seasons_by_parent(parent_id) -> Vec<SeasonAnime>` and a subscription variant. Dependencies: `animes, episodes, animehistory`.

## 4. Anime: `animedeletableView` (animedeletableView.sq)

```sql
CREATE VIEW animedeletableView AS
SELECT M._id, M.source, M.fetch_type FROM animes AS M
WHERE M.favorite = 0 AND M.parent_id IS NULL;
```

Query `getDeletableParentAnime: SELECT * FROM animedeletableView`, mapped by `AnimeMapper.mapDeletableAnime(id, source, fetchType)` into `DeletableAnime(animeId, sourceId, fetchType)`. Consumer: `GetAnimeSourcesWithNonLibraryAnime` (domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetAnimeSourcesWithNonLibraryAnime.kt) used by the clear-database settings screen (app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearAnimeDatabaseScreen.kt), which groups the rows by `sourceId`, recurses into `Seasons` parents with `getChildrenByParentId`, and detaches favorited children with `removeParentIdByIds` (full algorithm in `p1-topic-seasons-and-relations.md` section 3). There is no foreign key on `parent_id`, so deleting a parent row neither cascades to nor nulls its children.

Dependencies: `animes`. Emission: whenever `animes` changes (the view is a plain filter).

## 5. `AnimeMapper.mapRelatedAnime` shape

`getRelatedAnimeByAnimeId` returns `A.*` (32 columns) plus `R.name` (the relation group title). `mapRelatedAnime` yields `Pair<String, Anime>(name, anime)`; `AnimeRelationRepositoryImpl.subscribeRelatedAnime` then groups consecutive rows with `rows.groupBy({ it.first }, { it.second }).map { (name, anime) -> AnimeRelationGroup(name, anime) }`. `groupBy` on a `LinkedHashMap` preserves first-encounter order of the group names, and the SQL `ORDER BY R.sort_order ASC` makes group order = order of the first member of each group. See `p1-topic-seasons-and-relations.md`.

## 6. Column-order contract tests

Because the mappers are positional and the views use `M.*`, adding or reordering a column in `animes`/`mangas` silently shifts every following index. Proposal: the Rust mappers never use numeric indexes for the entity part; they read by column name (`row.get("fetch_type")` via a cached `HashMap<&str, usize>` built from `Statement::column_names()` once per statement). A test per mapper executes the real query against the schema from area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations and asserts the 40/31/39/33 column counts and that each name used exists.
