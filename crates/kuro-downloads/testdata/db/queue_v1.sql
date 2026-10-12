-- Fixture for TST-42 to TST-46: the parent tables of the unified schema reduced to their keys, then the download DDL.
CREATE TABLE entries(_id INTEGER PRIMARY KEY, kind TEXT NOT NULL, source INTEGER NOT NULL, url TEXT NOT NULL, title TEXT NOT NULL, UNIQUE (_id, kind));
CREATE TABLE items(_id INTEGER PRIMARY KEY, entry_id INTEGER NOT NULL REFERENCES entries(_id) ON DELETE CASCADE, url TEXT NOT NULL, name TEXT NOT NULL);
INSERT INTO entries VALUES (1, 'manga', 1, '/manga/1', 'Berserk');
INSERT INTO items VALUES (10, 1, '/chapter/10', 'Chapter 10'), (11, 1, '/chapter/11', 'Chapter 11'), (12, 1, '/chapter/12', 'Chapter 12');
-- Download tables of the unified database. Also exported as `schema_unified::DOWNLOAD_DDL` for the stores' in-memory tests.
CREATE TABLE download_queue(
    unit_id     INTEGER NOT NULL PRIMARY KEY REFERENCES items(_id) ON DELETE CASCADE,
    kind        TEXT    NOT NULL,
    entry_id    INTEGER NOT NULL,
    source      INTEGER NOT NULL,
    position    INTEGER NOT NULL,
    status      INTEGER NOT NULL DEFAULT 1,
    enqueued_at INTEGER NOT NULL,
    FOREIGN KEY (entry_id, kind) REFERENCES entries(_id, kind) ON DELETE CASCADE
);
CREATE INDEX download_queue_position_index ON download_queue(position);
CREATE INDEX download_queue_source_index   ON download_queue(kind, source, position);

CREATE TABLE download_pending_delete(            -- no foreign keys: the item may be gone when the flush runs
    unit_id     INTEGER NOT NULL PRIMARY KEY,
    kind        TEXT    NOT NULL CHECK (kind IN ('manga', 'anime')),
    entry_id    INTEGER NOT NULL,
    entry_url   TEXT    NOT NULL,
    entry_title TEXT    NOT NULL,
    source      INTEGER NOT NULL,
    unit_url    TEXT    NOT NULL,
    unit_name   TEXT    NOT NULL,
    scanlator   TEXT,
    done        INTEGER NOT NULL DEFAULT 0,      -- anime pending rows carried seen/bookmark; manga wrote 0
    bookmark    INTEGER NOT NULL DEFAULT 0,
    seq         INTEGER NOT NULL,                -- insertion order inside an entry (manga used enqueued_at, id)
    enqueued_at INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX download_pending_delete_entry_index ON download_pending_delete(entry_id, seq);

CREATE TABLE download_pending_corrupt(           -- anime_download_pending_corrupt, E-P04: rows that could not be read
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    source_rowid INTEGER NOT NULL,
    reason       TEXT    NOT NULL,
    recorded_at  INTEGER NOT NULL
);

