-- Kurobayomi anime views in dependency order (Aniyomi data/src/main/sqldelightanime/view/*.sq).

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
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.total, 0)
        WHEN 0 THEN coalesce(ASS.child_count, 0)
    END AS totalCount,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.seenCount, 0)
        WHEN 0 THEN coalesce(ASS.fully_seen_seasons, 0)
    END AS seenCount,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.latestUpload, 0)
        WHEN 0 THEN coalesce(ASS.max_latest_upload, 0)
    END AS latestUpload,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.fetchedAt, 0)
        WHEN 0 THEN coalesce(ASS.max_fetched_at, 0)
    END AS episodeFetchedAt,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(AHS.lastSeen, 0)
        WHEN 0 THEN coalesce(ASS.max_last_seen, 0)
    END AS lastSeen,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.bookmarkCount, 0)
        WHEN 0 THEN coalesce(ASS.total_bookmarks, 0)
    END AS bookmarkCount,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.fillermarkCount, 0)
        WHEN 0 THEN coalesce(ASS.total_fillermarks, 0)
    END AS fillermarkCount,
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
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.total, 0)
        WHEN 0 THEN coalesce(ASS.child_count, 0)
    END AS totalCount,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.seenCount, 0)
        WHEN 0 THEN coalesce(ASS.fully_seen_seasons, 0)
    END AS seenCount,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.latestUpload, 0)
        WHEN 0 THEN coalesce(ASS.max_latest_upload, 0)
    END AS latestUpload,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.fetchedAt, 0)
        WHEN 0 THEN coalesce(ASS.max_fetched_at, 0)
    END AS episodeFetchedAt,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(AHS.lastSeen, 0)
        WHEN 0 THEN coalesce(ASS.max_last_seen, 0)
    END AS lastSeen,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.bookmarkCount, 0)
        WHEN 0 THEN coalesce(ASS.total_bookmarks, 0)
    END AS bookmarkCount,
    CASE M.fetch_type
        WHEN 1 THEN coalesce(ES.fillermarkCount, 0)
        WHEN 0 THEN coalesce(ASS.total_fillermarks, 0)
    END AS fillermarkCount
FROM animes AS M
LEFT JOIN episodestatsView AS ES
ON M._id = ES.anime_id
LEFT JOIN animehistorystatsView AS AHS
ON M._id = AHS.anime_id
LEFT JOIN animeseasonstatsView AS ASS
ON M._id = ASS.parent_id
WHERE M.parent_id IS NOT NULL;

CREATE VIEW animehistoryView AS
SELECT
    animehistory._id AS id,
    animes._id AS animeId,
    episodes._id AS episodeId,
    animes.title,
    animes.thumbnail_url AS thumbnailUrl,
    animes.source,
    animes.favorite,
    animes.cover_last_modified,
    episodes.episode_number AS episodeNumber,
    animehistory.last_seen AS seenAt,
    max_last_seen.last_seen AS maxSeenAt,
    max_last_seen.episode_id AS maxSeenAtEpisodeId
FROM animes
JOIN episodes
ON animes._id = episodes.anime_id
JOIN animehistory
ON episodes._id = animehistory.episode_id
JOIN (
    SELECT episodes.anime_id,episodes._id AS episode_id, MAX(animehistory.last_seen) AS last_seen
    FROM episodes JOIN animehistory
    ON episodes._id = animehistory.episode_id
    GROUP BY episodes.anime_id
) AS max_last_seen
ON episodes.anime_id = max_last_seen.anime_id;

CREATE VIEW animeupdatesView AS
SELECT
    animes._id AS animeId,
    animes.title AS animeTitle,
    episodes._id AS episodeId,
    episodes.name AS episodeName,
    episodes.scanlator,
    episodes.seen,
    episodes.bookmark,
    episodes.fillermark,
    episodes.last_second_seen,
    episodes.total_seconds AS totalSeconds,
    animes.source,
    animes.favorite,
    animes.thumbnail_url AS thumbnailUrl,
    animes.cover_last_modified AS coverLastModified,
    episodes.date_upload AS dateUpload,
    episodes.date_fetch AS datefetch
FROM animes JOIN episodes
ON animes._id = episodes.anime_id
WHERE favorite = 1
AND date_fetch > date_added
ORDER BY date_fetch DESC;

CREATE VIEW animedeletableView AS
SELECT
    M._id,
    M.source,
    M.fetch_type
FROM animes AS M
WHERE M.favorite = 0
AND M.parent_id IS NULL;
