# Topic — The eight views, aggregate rules and query plans

Source files: `data/src/main/sqldelightanime/view/*.sq`. This document gives the exact text to embed in `crates/kuro-db/sql/anime/views.sql` (creation order matters only for readability: SQLite resolves view references lazily, but the order below is dependency order), then the column contract, then measured query-plan facts and one performance proposal.

## 1. Texts (embed verbatim)

```sql
CREATE VIEW episodestatsView AS
SELECT
    episodes.anime_id,
    count(*) AS total,
    sum(seen) AS seenCount,
    coalesce(max(episodes.date_upload), 0) AS latestUpload,
    coalesce(max(episodes.date_fetch), 0) AS fetchedAt,
    sum(episodes.bookmark) AS bookmarkCount,
    sum(episodes.fillermark) AS fillermarkCount
FROM episodes
GROUP BY episodes.anime_id;

CREATE VIEW animehistorystatsView AS
SELECT
    episodes.anime_id,
    max(ah.last_seen) AS lastSeen
FROM episodes
LEFT JOIN animehistory AS ah ON episodes._id = ah.episode_id
GROUP BY episodes.anime_id;

CREATE VIEW animeseasonstatsView AS
SELECT
    season.parent_id,
    count(*) AS child_count,
    sum(CASE WHEN season.fetch_type = 1 AND
                 coalesce(ES.total, 0) = coalesce(ES.seenCount, 0)
            THEN 1 ELSE 0 END) AS fully_seen_seasons,
    max(coalesce(ES.latestUpload, 0)) AS max_latest_upload,
    max(coalesce(ES.fetchedAt, 0)) AS max_fetched_at,
    max(coalesce(AHS.lastSeen, 0)) AS max_last_seen,
    sum(coalesce(ES.bookmarkCount, 0)) AS total_bookmarks,
    sum(coalesce(ES.fillermarkCount, 0)) AS total_fillermarks
FROM animes season
LEFT JOIN episodestatsView ES ON season._id = ES.anime_id
LEFT JOIN animehistorystatsView AHS ON season._id = AHS.anime_id
WHERE season.parent_id IS NOT NULL
GROUP BY season.parent_id;

CREATE VIEW animelibView AS
SELECT
    M.*,
    CASE M.fetch_type WHEN 1 THEN coalesce(ES.total, 0)          WHEN 0 THEN coalesce(ASS.child_count, 0)        END AS totalCount,
    CASE M.fetch_type WHEN 1 THEN coalesce(ES.seenCount, 0)      WHEN 0 THEN coalesce(ASS.fully_seen_seasons, 0) END AS seenCount,
    CASE M.fetch_type WHEN 1 THEN coalesce(ES.latestUpload, 0)   WHEN 0 THEN coalesce(ASS.max_latest_upload, 0)  END AS latestUpload,
    CASE M.fetch_type WHEN 1 THEN coalesce(ES.fetchedAt, 0)      WHEN 0 THEN coalesce(ASS.max_fetched_at, 0)     END AS episodeFetchedAt,
    CASE M.fetch_type WHEN 1 THEN coalesce(AHS.lastSeen, 0)      WHEN 0 THEN coalesce(ASS.max_last_seen, 0)      END AS lastSeen,
    CASE M.fetch_type WHEN 1 THEN coalesce(ES.bookmarkCount, 0)  WHEN 0 THEN coalesce(ASS.total_bookmarks, 0)    END AS bookmarkCount,
    CASE M.fetch_type WHEN 1 THEN coalesce(ES.fillermarkCount, 0) WHEN 0 THEN coalesce(ASS.total_fillermarks, 0) END AS fillermarkCount,
    coalesce(MC.category_id, 0) AS category
FROM animes M
LEFT JOIN episodestatsView AS ES ON M._id = ES.anime_id
LEFT JOIN animehistorystatsView AS AHS ON M._id = AHS.anime_id
LEFT JOIN animes_categories AS MC ON MC.anime_id = M._id
LEFT JOIN animeseasonstatsView AS ASS ON M._id = ASS.parent_id
WHERE M.favorite = 1;

CREATE VIEW animeseasonsView AS
SELECT
    M.*,
    CASE M.fetch_type WHEN 1 THEN coalesce(ES.total, 0)          WHEN 0 THEN coalesce(ASS.child_count, 0)        END AS totalCount,
    CASE M.fetch_type WHEN 1 THEN coalesce(ES.seenCount, 0)      WHEN 0 THEN coalesce(ASS.fully_seen_seasons, 0) END AS seenCount,
    CASE M.fetch_type WHEN 1 THEN coalesce(ES.latestUpload, 0)   WHEN 0 THEN coalesce(ASS.max_latest_upload, 0)  END AS latestUpload,
    CASE M.fetch_type WHEN 1 THEN coalesce(ES.fetchedAt, 0)      WHEN 0 THEN coalesce(ASS.max_fetched_at, 0)     END AS episodeFetchedAt,
    CASE M.fetch_type WHEN 1 THEN coalesce(AHS.lastSeen, 0)      WHEN 0 THEN coalesce(ASS.max_last_seen, 0)      END AS lastSeen,
    CASE M.fetch_type WHEN 1 THEN coalesce(ES.bookmarkCount, 0)  WHEN 0 THEN coalesce(ASS.total_bookmarks, 0)    END AS bookmarkCount,
    CASE M.fetch_type WHEN 1 THEN coalesce(ES.fillermarkCount, 0) WHEN 0 THEN coalesce(ASS.total_fillermarks, 0) END AS fillermarkCount
FROM animes AS M
LEFT JOIN episodestatsView AS ES ON M._id = ES.anime_id
LEFT JOIN animehistorystatsView AS AHS ON M._id = AHS.anime_id
LEFT JOIN animeseasonstatsView AS ASS ON M._id = ASS.parent_id
WHERE M.parent_id IS NOT NULL;
```

`animehistoryView`, `animeupdatesView` and `animedeletableView` are quoted in `p1-behavior.md` sections 5.2 and 9 (copy those texts verbatim). The `.sq` files write the `CASE` arms on separate lines; whitespace is not significant, so the single-line form above is equivalent. The SQLDelight statements that sit in the same files (`animelib`, `getAnimeSeasonsById`, `animehistory`, `getLatestAnimeHistory`, `getRecentAnimeUpdates`, `getUpdatesBySeenStatus`, `getDeletableParentAnime`) are NOT part of the view text; they belong in `crates/kuro-db/sql/anime/queries/*.sql`.

## 2. Column contract (exact names, used by name in Rust)

| View | Columns in order |
|---|---|
| `episodestatsView` | `anime_id, total, seenCount, latestUpload, fetchedAt, bookmarkCount, fillermarkCount` |
| `animehistorystatsView` | `anime_id, lastSeen` |
| `animeseasonstatsView` | `parent_id, child_count, fully_seen_seasons, max_latest_upload, max_fetched_at, max_last_seen, total_bookmarks, total_fillermarks` |
| `animelibView` | the 32 `animes` columns, then `totalCount, seenCount, latestUpload, episodeFetchedAt, lastSeen, bookmarkCount, fillermarkCount, category` (40 columns) |
| `animeseasonsView` | the 32 `animes` columns, then `totalCount, seenCount, latestUpload, episodeFetchedAt, lastSeen, bookmarkCount, fillermarkCount` (39 columns) |
| `animehistoryView` | `id, animeId, episodeId, title, thumbnailUrl, source, favorite, cover_last_modified, episodeNumber, seenAt, maxSeenAt, maxSeenAtEpisodeId` |
| `animeupdatesView` | `animeId, animeTitle, episodeId, episodeName, scanlator, seen, bookmark, fillermark, last_second_seen, totalSeconds, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch` |
| `animedeletableView` | `_id, source, fetch_type` |

Note the spelling `datefetch` (lower-case f) in `animeupdatesView` and `fetchedAt` versus `episodeFetchedAt` between the stats view and the library view; `AnimeMapper.mapSeasonAnime` names the corresponding Kotlin parameter `fetchedAt`.

## 3. Aggregate rules (decisions encoded in the SQL)

1. Episode-type entries (`fetch_type = 1`): counts come from their own episodes; an entry with no episodes reports 0 for everything.
2. Season-type entries (`fetch_type = 0`): `totalCount` = number of child rows (any child, favorite or not, any fetch type); `seenCount` = children with `fetch_type = 1` whose episodes are all seen (`total = seenCount`, so a childless-of-episodes season counts as complete); `latestUpload`, `episodeFetchedAt`, `lastSeen` = maxima over children's own episode stats (grand-children are not included: a child of type Seasons contributes 0 because it has no episode rows); `bookmarkCount`, `fillermarkCount` = sums over children.
3. `lastSeen` is the maximum history `last_seen` over the entry's episodes (0 when none); history rows that were reset to 0 do not raise it.
4. The library view multiplies rows by category membership (section 6 of `p1-behavior.md`); UI code groups by `category` and treats `0` as the Default category.
5. All integer sums can come back as REAL in the typed layer (SQLDelight generates `Double` for `seenCount`, `bookmarkCount`, `fillermarkCount`); convert with truncation.

## 4. Query-plan facts (SQLite 3.53.4, `EXPLAIN QUERY PLAN SELECT * FROM animelibView`)

* `episodestatsView` and `animehistorystatsView` and `animeseasonstatsView` are MATERIALIZED once per query (full scan of `episodes` through `episodes_anime_id_index`, a covering scan for history, plus `animes_parent_id` for seasons).
* The joins to those materialised sets use AUTOMATIC COVERING INDEXes built per execution (`ES`, `AHS`, `ASS`), and so does `animes_categories` (`MC`), because `animes_categories` has no index on `anime_id` in the fresh schema.
* `animelib_favorite_index` is used for `favorite = 1`.

Cost: the whole library query is O(episodes + links) per execution and re-runs after every write to a table it reads (progress updates while playing re-run it). For a library of a few thousand entries and tens of thousands of episodes this is milliseconds to tens of milliseconds; acceptable, but the streams are coalesced (50 ms) and the library screen must not subscribe while the player is in the foreground (area:37-ui-library).

Proposal (T-04a-036, P2): a Kurobayomi-native schema step 139 to 140 adds `CREATE INDEX animes_categories_anime_id_index ON animes_categories(anime_id)` and `CREATE INDEX animes_categories_category_id_index ON animes_categories(category_id)` (the second speeds `ON DELETE CASCADE` of a category and `getCategoriesByAnimeId`), so the `MC` automatic index disappears from the plan. These indexes do not change results. Cost: a database at version 140 cannot be opened by Aniyomi on Android (SQLite downgrade refusal); Kurobayomi never needs to hand its raw file back (the backup format is the interchange path).

## 5. Query text that must be added in Rust but is not in a `.sq` file

* Explicit ordering for the updates feed: `... FROM animeupdatesView WHERE dateUpload > :after ORDER BY datefetch DESC LIMIT :limit` (and the `seen = :seen` variant).
* Explicit tie-break for the library query is NOT added (Aniyomi sorts in Kotlin by flags); the UI crate sorts.
