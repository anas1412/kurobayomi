-- Anime download queue (area 24, p1-data-and-config.md section 5.1). Applied idempotently by AnimeQueueStore::open.
CREATE TABLE IF NOT EXISTS anime_download_queue (
    episode_id   INTEGER NOT NULL PRIMARY KEY,
    anime_id     INTEGER NOT NULL,
    ord          INTEGER NOT NULL,
    changed_at   INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);
CREATE INDEX IF NOT EXISTS idx_anime_download_queue_ord ON anime_download_queue(ord);
