-- Anime pending deletes, remove-after-watched (area 24, p1-data-and-config.md section 5.2). Applied idempotently by AnimePendingStore::open.
CREATE TABLE IF NOT EXISTS anime_download_pending_delete (
    episode_id   INTEGER NOT NULL PRIMARY KEY,
    anime_id     INTEGER NOT NULL,
    episode_url  TEXT    NOT NULL,
    episode_name TEXT    NOT NULL,
    scanlator    TEXT,
    seen         INTEGER NOT NULL DEFAULT 0,
    bookmark     INTEGER NOT NULL DEFAULT 0,
    anime_url    TEXT    NOT NULL,
    anime_title  TEXT    NOT NULL,
    anime_source INTEGER NOT NULL,
    seq          INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_anime_download_pending_anime ON anime_download_pending_delete(anime_id, seq);
-- Rows of the pending list that could not be read, kept for inspection instead of being cleared (E-P04, T-24a-053).
CREATE TABLE IF NOT EXISTS anime_download_pending_corrupt (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    source_rowid INTEGER NOT NULL,
    reason       TEXT    NOT NULL,
    recorded_at  INTEGER NOT NULL
);
