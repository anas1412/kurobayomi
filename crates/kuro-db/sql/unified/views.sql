-- Kurobayomi unified views (docs/design/unified-db.md section 1.1).
CREATE VIEW historyView AS
SELECT h._id AS id, e.kind, e._id AS entryId, i._id AS itemId, e.title, e.thumbnail_url AS thumbnailUrl,
       e.source, e.favorite, e.cover_last_modified, i.number AS itemNumber,
       h.last_at AS lastAt, h.duration_ms AS durationMs,
       m.last_at AS maxLastAt, m.item_id AS maxLastAtItemId
FROM entries e
JOIN items   i ON i.entry_id = e._id
JOIN history h ON h.item_id  = i._id
JOIN (SELECT i2.entry_id, i2._id AS item_id, MAX(h2.last_at) AS last_at       -- SQLite bare-column-with-MAX: the row of the max
      FROM items i2 JOIN history h2 ON h2.item_id = i2._id GROUP BY i2.entry_id) m
  ON m.entry_id = e._id;

CREATE VIEW updatesView AS
SELECT e.kind, e._id AS entryId, e.title AS entryTitle, i._id AS itemId, i.name AS itemName, i.scanlator,
       i.done, i.bookmark, i.fillermark, i.progress, i.total_seconds AS totalSeconds,
       e.source, e.favorite, e.thumbnail_url AS thumbnailUrl, e.cover_last_modified AS coverLastModified,
       i.date_upload AS dateUpload, i.date_fetch AS datefetch
FROM entries e JOIN items i ON i.entry_id = e._id
WHERE e.favorite = 1 AND i.date_fetch > e.date_added;      -- no ORDER BY inside the view; statements order

CREATE VIEW libraryView AS
WITH scope(entry_id) AS (                         -- the favourites prefilter (migration 34) for BOTH kinds
    SELECT _id FROM entries WHERE favorite = 1
    UNION
    SELECT _id FROM entries WHERE parent_id IN (SELECT _id FROM entries WHERE favorite = 1)
),
item_stats AS (
    SELECT i.entry_id,
           count(*)                        AS total,
           sum(i.done)                     AS doneCount,
           coalesce(max(i.date_upload), 0) AS latestUpload,
           coalesce(max(i.date_fetch), 0)  AS fetchedAt,
           sum(i.bookmark)                 AS bookmarkCount,
           sum(i.fillermark)               AS fillermarkCount,
           coalesce(max(h.last_at), 0)     AS lastActivity
    FROM items i
    LEFT JOIN excluded_scanlators x ON x.entry_id = i.entry_id AND x.scanlator = i.scanlator
    LEFT JOIN history h             ON h.item_id = i._id
    WHERE x.scanlator IS NULL AND i.entry_id IN (SELECT entry_id FROM scope)
    GROUP BY i.entry_id
),
season_stats AS (                                 -- anime containers (fetch_type = 0) roll up their children
    SELECT s.parent_id,
           count(*) AS child_count,
           sum(CASE WHEN s.fetch_type = 1 AND coalesce(t.total, 0) = coalesce(t.doneCount, 0) THEN 1 ELSE 0 END) AS fully_done,
           max(coalesce(t.latestUpload, 0))  AS latestUpload,
           max(coalesce(t.fetchedAt, 0))     AS fetchedAt,
           max(coalesce(t.lastActivity, 0))  AS lastActivity,
           sum(coalesce(t.bookmarkCount, 0)) AS bookmarkCount,
           sum(coalesce(t.fillermarkCount, 0)) AS fillermarkCount
    FROM entries s LEFT JOIN item_stats t ON t.entry_id = s._id
    WHERE s.parent_id IN (SELECT _id FROM entries WHERE favorite = 1)
    GROUP BY s.parent_id
)
SELECT e.*,
       CASE WHEN e.fetch_type = 0 THEN coalesce(ss.child_count, 0)     ELSE coalesce(st.total, 0)           END AS totalCount,
       CASE WHEN e.fetch_type = 0 THEN coalesce(ss.fully_done, 0)      ELSE coalesce(st.doneCount, 0)       END AS doneCount,
       CASE WHEN e.fetch_type = 0 THEN coalesce(ss.latestUpload, 0)    ELSE coalesce(st.latestUpload, 0)    END AS latestUpload,
       CASE WHEN e.fetch_type = 0 THEN coalesce(ss.fetchedAt, 0)       ELSE coalesce(st.fetchedAt, 0)       END AS itemFetchedAt,
       CASE WHEN e.fetch_type = 0 THEN coalesce(ss.lastActivity, 0)    ELSE coalesce(st.lastActivity, 0)    END AS lastActivity,
       CASE WHEN e.fetch_type = 0 THEN coalesce(ss.bookmarkCount, 0)   ELSE coalesce(st.bookmarkCount, 0)   END AS bookmarkCount,
       CASE WHEN e.fetch_type = 0 THEN coalesce(ss.fillermarkCount, 0) ELSE coalesce(st.fillermarkCount, 0) END AS fillermarkCount,
       coalesce(ec.category_id, 0) AS category
FROM entries e
LEFT JOIN item_stats st       ON st.entry_id  = e._id
LEFT JOIN season_stats ss     ON ss.parent_id = e._id
LEFT JOIN entry_categories ec ON ec.entry_id  = e._id
WHERE e.favorite = 1;
