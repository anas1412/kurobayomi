-- Manga chapters waiting for the reader to exit before they are deleted (area 23, p1-data-and-config.md section 3.1).
-- No foreign key to chapters: the chapter may already be gone when the flush runs. Applied idempotently by MangaPendingStore::open.
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
