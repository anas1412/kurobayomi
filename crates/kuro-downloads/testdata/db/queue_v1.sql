-- Fixture for TST-42 to TST-46: the parent tables of area 03 reduced to their keys, then the queue DDL.
CREATE TABLE mangas(_id INTEGER PRIMARY KEY, source INTEGER NOT NULL, url TEXT NOT NULL, title TEXT NOT NULL);
CREATE TABLE chapters(_id INTEGER PRIMARY KEY, manga_id INTEGER NOT NULL REFERENCES mangas(_id) ON DELETE CASCADE, url TEXT NOT NULL, name TEXT NOT NULL);
INSERT INTO mangas VALUES (1, 1, '/manga/1', 'Berserk');
INSERT INTO chapters VALUES (10, 1, '/chapter/10', 'Chapter 10'), (11, 1, '/chapter/11', 'Chapter 11'), (12, 1, '/chapter/12', 'Chapter 12');
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
