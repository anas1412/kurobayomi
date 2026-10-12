-- Kurobayomi unified database v1 (docs/design/unified-db.md section 1): manga and anime in one file.
-- ===== entries: manga and anime in one table ==================================================
CREATE TABLE entries(
    _id                      INTEGER NOT NULL PRIMARY KEY,
    kind                     TEXT    NOT NULL CHECK (kind IN ('manga', 'anime')),
    source                   INTEGER NOT NULL,
    url                      TEXT    NOT NULL,
    title                    TEXT    NOT NULL,
    artist                   TEXT,
    author                   TEXT,
    description              TEXT,
    genre                    TEXT,
    status                   INTEGER NOT NULL,
    thumbnail_url            TEXT,
    favorite                 INTEGER NOT NULL,
    last_update              INTEGER,
    next_update              INTEGER,
    initialized              INTEGER NOT NULL,
    viewer                   INTEGER NOT NULL,   -- kind-native bits (reader modes / intro+airing), see 3.4
    item_flags               INTEGER NOT NULL,   -- anime superset layout for both kinds, see 3.4
    cover_last_modified      INTEGER NOT NULL,
    date_added               INTEGER NOT NULL,
    update_strategy          INTEGER NOT NULL DEFAULT 0,
    calculate_interval       INTEGER NOT NULL DEFAULT 0,
    last_modified_at         INTEGER NOT NULL DEFAULT 0,
    favorite_modified_at     INTEGER,
    version                  INTEGER NOT NULL DEFAULT 0,
    -- anime only: NULL for manga (checked below)
    fetch_type               INTEGER,            -- 1 = has episodes, 0 = container of seasons
    parent_id                INTEGER,            -- no foreign key, as before (orphans are handled in code)
    season_flags             INTEGER,
    season_number            REAL,
    season_source_order      INTEGER,
    background_url           TEXT,
    background_last_modified INTEGER,
    memo                     BLOB,               -- JSON object bytes
    UNIQUE (kind, source, url),
    UNIQUE (_id, kind),                          -- parent key for the composite foreign keys below
    CHECK (kind = 'anime' OR (fetch_type IS NULL AND parent_id IS NULL AND season_flags IS NULL
                              AND season_number IS NULL AND season_source_order IS NULL
                              AND background_url IS NULL AND background_last_modified IS NULL AND memo IS NULL)),
    CHECK (kind = 'manga' OR (fetch_type IS NOT NULL AND season_flags IS NOT NULL AND season_number IS NOT NULL
                              AND season_source_order IS NOT NULL AND background_last_modified IS NOT NULL
                              AND memo IS NOT NULL))
);
CREATE INDEX entries_favorite_index ON entries(kind) WHERE favorite = 1;
CREATE INDEX entries_parent_index   ON entries(parent_id) WHERE parent_id IS NOT NULL;

CREATE TRIGGER update_last_favorite_at_entries
AFTER UPDATE OF favorite ON entries
BEGIN
    UPDATE entries SET favorite_modified_at = strftime('%s', 'now') WHERE _id = new._id;
END;
CREATE TRIGGER update_last_modified_at_entries
AFTER UPDATE ON entries
FOR EACH ROW
BEGIN
    UPDATE entries SET last_modified_at = strftime('%s', 'now') WHERE _id = new._id;
END;
CREATE TRIGGER update_entry_version AFTER UPDATE ON entries
BEGIN
    UPDATE entries SET version = version + 1
    WHERE _id = new._id AND (new.url != old.url OR new.description != old.description OR new.favorite != old.favorite);
END;

-- ===== items: chapters and episodes ============================================================
CREATE TABLE items(
    _id              INTEGER NOT NULL PRIMARY KEY,
    entry_id         INTEGER NOT NULL REFERENCES entries(_id) ON DELETE CASCADE,
    url              TEXT    NOT NULL,
    name             TEXT    NOT NULL,
    scanlator        TEXT,
    done             INTEGER NOT NULL,           -- chapters.read / episodes.seen
    bookmark         INTEGER NOT NULL,
    progress         INTEGER NOT NULL,           -- chapters.last_page_read / episodes.last_second_seen
    total_seconds    INTEGER,                    -- episodes only (NULL for chapters)
    number           REAL    NOT NULL,           -- chapter_number / episode_number
    source_order     INTEGER NOT NULL,
    date_fetch       INTEGER NOT NULL,
    date_upload      INTEGER NOT NULL,
    last_modified_at INTEGER NOT NULL DEFAULT 0,
    version          INTEGER NOT NULL DEFAULT 0,
    fillermark       INTEGER NOT NULL DEFAULT 0, -- episodes only (0 for chapters)
    summary          TEXT,                       -- episodes only
    preview_url      TEXT,                       -- episodes only
    memo             BLOB                        -- episodes only
);
CREATE INDEX items_entry_index  ON items(entry_id);
CREATE INDEX items_undone_index ON items(entry_id, done) WHERE done = 0;
CREATE INDEX items_url_index    ON items(url);   -- episodes did not have it; chapters got it in migration 34

CREATE TRIGGER update_last_modified_at_items
AFTER UPDATE ON items
FOR EACH ROW
BEGIN
    UPDATE items SET last_modified_at = strftime('%s', 'now') WHERE _id = new._id;
END;
CREATE TRIGGER update_item_and_entry_version AFTER UPDATE ON items
WHEN new.done != old.done OR new.bookmark != old.bookmark OR new.progress != old.progress
BEGIN
    UPDATE items   SET version = version + 1 WHERE _id = new._id;
    UPDATE entries SET version = version + 1 WHERE _id = new.entry_id;
END;

CREATE TABLE excluded_scanlators(                -- manga-only today; entry ids are unique so no kind column
    entry_id  INTEGER NOT NULL REFERENCES entries(_id) ON DELETE CASCADE,
    scanlator TEXT    NOT NULL
);
CREATE INDEX excluded_scanlators_entry_index ON excluded_scanlators(entry_id);

-- ===== history =================================================================================
CREATE TABLE history(
    _id         INTEGER NOT NULL PRIMARY KEY,
    item_id     INTEGER NOT NULL UNIQUE REFERENCES items(_id) ON DELETE CASCADE,
    last_at     INTEGER NOT NULL DEFAULT 0,      -- ms; 0 = reset (anime last_seen NULL reads as 0)
    duration_ms INTEGER NOT NULL DEFAULT 0       -- chapters.time_read; anime writes 0 until step 4
);

-- ===== categories (ids are per kind) ===========================================================
CREATE TABLE categories(
    kind   TEXT    NOT NULL CHECK (kind IN ('manga', 'anime')),
    _id    INTEGER NOT NULL,
    name   TEXT    NOT NULL,
    sort   INTEGER NOT NULL,
    flags  INTEGER NOT NULL,
    hidden INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (kind, _id)
);
INSERT INTO categories(kind, _id, name, sort, flags) VALUES ('manga', 0, '', -1, 64), ('anime', 0, '', -1, 64);
CREATE TRIGGER system_category_delete_trigger BEFORE DELETE ON categories
BEGIN SELECT CASE WHEN old._id <= 0 THEN RAISE(ABORT, 'System category can''t be deleted') END; END;

CREATE TABLE entry_categories(                   -- NOT "WITHOUT ROWID": SQLite's update hook skips those tables
    entry_id    INTEGER NOT NULL,
    kind        TEXT    NOT NULL,
    category_id INTEGER NOT NULL,
    PRIMARY KEY (entry_id, category_id),
    FOREIGN KEY (entry_id, kind)    REFERENCES entries(_id, kind)    ON DELETE CASCADE,
    FOREIGN KEY (kind, category_id) REFERENCES categories(kind, _id) ON DELETE CASCADE
);
CREATE INDEX entry_categories_category_index ON entry_categories(kind, category_id);
CREATE TRIGGER insert_entry_category_update_version AFTER INSERT ON entry_categories
BEGIN
    UPDATE entries SET version = version + 1 WHERE _id = new.entry_id;
END;

-- ===== tracks (manga_sync + anime_sync) ========================================================
CREATE TABLE tracks(
    _id         INTEGER NOT NULL PRIMARY KEY,
    entry_id    INTEGER NOT NULL REFERENCES entries(_id) ON DELETE CASCADE,
    tracker_id  INTEGER NOT NULL,                -- sync_id
    remote_id   INTEGER NOT NULL,
    library_id  INTEGER,
    title       TEXT    NOT NULL,
    last_item   REAL    NOT NULL,                -- last_chapter_read / last_episode_seen
    total_items INTEGER NOT NULL,                -- total_chapters / total_episodes
    status      INTEGER NOT NULL,
    score       REAL    NOT NULL,
    remote_url  TEXT    NOT NULL,
    start_date  INTEGER NOT NULL,
    finish_date INTEGER NOT NULL,
    private     INTEGER NOT NULL DEFAULT 0,
    UNIQUE (entry_id, tracker_id) ON CONFLICT REPLACE
);

-- ===== sources and relations ====================================================================
CREATE TABLE stub_sources(                       -- sources + animesources; id 0 (local) exists in both kinds
    kind TEXT    NOT NULL CHECK (kind IN ('manga', 'anime')),
    id   INTEGER NOT NULL,
    lang TEXT    NOT NULL,
    name TEXT    NOT NULL,
    PRIMARY KEY (kind, id)
);

CREATE TABLE anime_relations(
    entry_id      INTEGER NOT NULL REFERENCES entries(_id) ON DELETE CASCADE,
    related_id    INTEGER NOT NULL REFERENCES entries(_id) ON DELETE CASCADE,
    name          TEXT    NOT NULL,
    sort_order    INTEGER NOT NULL,
    last_fetched_at INTEGER NOT NULL,
    PRIMARY KEY (entry_id, related_id, name)
);
CREATE INDEX anime_relations_related_index ON anime_relations(related_id);

-- ===== tracker retry (the download tables are in downloads.sql) ===============================
CREATE TABLE track_retry_queue(                  -- verbatim from kuro-track/src/queue/store.rs
    kind       TEXT    NOT NULL CHECK (kind IN ('anime', 'manga')),
    entry_id   INTEGER NOT NULL,
    tracker_id INTEGER NOT NULL,
    progress   REAL    NOT NULL,
    queued_at  INTEGER NOT NULL,
    attempts   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (kind, entry_id, tracker_id)
);
