-- Manga download queue and pending deletes (area 23, p1-data-and-config.md section 3.1).
-- Applied idempotently by QueueStore::open. Area 03 may move this into its numbered migrations.
CREATE TABLE IF NOT EXISTS manga_download_queue (
    chapter_id   INTEGER NOT NULL PRIMARY KEY REFERENCES chapters(_id) ON DELETE CASCADE,
    manga_id     INTEGER NOT NULL REFERENCES mangas(_id) ON DELETE CASCADE,
    source_id    INTEGER NOT NULL,
    position     INTEGER NOT NULL,
    status       INTEGER NOT NULL DEFAULT 1,
    enqueued_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS manga_download_queue_position ON manga_download_queue(position);
CREATE INDEX IF NOT EXISTS manga_download_queue_source ON manga_download_queue(source_id, position);

CREATE TABLE IF NOT EXISTS manga_pending_delete (
    chapter_id   INTEGER NOT NULL PRIMARY KEY,
    manga_id     INTEGER NOT NULL,
    manga_title  TEXT    NOT NULL,
    manga_url    TEXT    NOT NULL,
    source_id    INTEGER NOT NULL,
    chapter_name TEXT    NOT NULL,
    chapter_url  TEXT    NOT NULL,
    scanlator    TEXT,
    enqueued_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS manga_pending_delete_manga ON manga_pending_delete(manga_id);
