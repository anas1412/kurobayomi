-- The reconstructed "version 13" state: the database that 14.sqm upgrades (PRAGMA user_version = 14).
-- Shapes follow the 18-column SELECT of 14.sqm (p1-topic-migration-history.md section 2). Rows exercise the
-- importer tests: a favourite and a library manga with chapters, an untrimmed scanlator, a MangaUpdates score of -1,
-- an orphan chapter (manga 99) and an orphan history row (chapter 98).
CREATE TABLE mangas(
    _id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    source INTEGER NOT NULL,
    url TEXT NOT NULL,
    artist TEXT,
    author TEXT,
    description TEXT,
    genre TEXT,
    title TEXT NOT NULL,
    status INTEGER NOT NULL,
    thumbnail_url TEXT,
    favorite INTEGER NOT NULL,
    last_update INTEGER,
    next_update INTEGER DEFAULT 0,
    initialized INTEGER NOT NULL,
    viewer INTEGER NOT NULL,
    chapter_flags INTEGER NOT NULL,
    cover_last_modified INTEGER NOT NULL DEFAULT 0,
    date_added INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX library_favorite_index ON mangas(favorite) WHERE favorite = 1;
CREATE INDEX mangas_url_index ON mangas(url);

CREATE TABLE chapters(
    _id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    manga_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    name TEXT NOT NULL,
    scanlator TEXT DEFAULT NULL,
    read INTEGER NOT NULL,
    bookmark INTEGER DEFAULT 0,
    last_page_read INTEGER NOT NULL,
    chapter_number REAL NOT NULL,
    source_order INTEGER DEFAULT 0,
    date_fetch INTEGER NOT NULL,
    date_upload INTEGER NOT NULL,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE
);
CREATE INDEX chapters_manga_id_index ON chapters(manga_id);
CREATE INDEX chapters_unread_by_manga_index ON chapters(manga_id, read) WHERE read = 0;

CREATE TABLE history(
    history_id INTEGER NOT NULL PRIMARY KEY,
    history_chapter_id INTEGER NOT NULL UNIQUE,
    history_last_read INTEGER,
    history_time_read INTEGER,
    FOREIGN KEY(history_chapter_id) REFERENCES chapters (_id) ON DELETE CASCADE
);
CREATE INDEX history_history_chapter_id_index ON history(history_chapter_id);

CREATE TABLE categories(
    _id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    sort INTEGER NOT NULL,
    flags INTEGER NOT NULL
);

CREATE TABLE mangas_categories(
    _id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    manga_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL,
    FOREIGN KEY(category_id) REFERENCES categories (_id) ON DELETE CASCADE,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE
);

CREATE TABLE manga_sync(
    _id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    manga_id INTEGER NOT NULL,
    sync_id INTEGER NOT NULL,
    remote_id INTEGER NOT NULL,
    library_id INTEGER,
    title TEXT NOT NULL,
    last_chapter_read REAL NOT NULL,
    total_chapters INTEGER NOT NULL,
    status INTEGER NOT NULL,
    score REAL NOT NULL,
    remote_url TEXT DEFAULT '',
    start_date INTEGER NOT NULL DEFAULT 0,
    finish_date INTEGER NOT NULL DEFAULT 0,
    UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id) ON DELETE CASCADE
);

INSERT INTO mangas(_id, source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, chapter_flags, cover_last_modified, date_added)
VALUES
    (1, 10, '/one', 'Art', 'Auth', 'Desc', 'Action, Drama', 'One', 1, 'http://x/1.jpg', 1, 1700000000000, 1700500000000, 1, 2, 3, 1699000000000, 1698000000000),
    (2, 10, '/two', NULL, NULL, NULL, NULL, 'Two', 0, NULL, 0, NULL, 0, 0, 0, 0, 0, 0),
    (3, 11, '/three', NULL, NULL, NULL, NULL, 'Three', 2, NULL, 1, 1600000000000, 0, 1, 0, 0, 0, 1597000000000);

INSERT INTO chapters(_id, manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload)
VALUES
    (1, 1, '/one/1', 'Chapter 1', '  Group ', 1, 0, 10, 1.0, 1, 500, 0),
    (2, 1, '/one/2', 'Chapter 2', NULL, 0, 1, 3, 2.0, 0, 400, 300),
    (3, 3, '/three/1', 'Chapter 1', 'Solo', 0, 0, 0, 1.0, 0, 100, 0),
    (4, 99, '/gone/1', 'Orphan', NULL, 0, 0, 0, 1.0, 0, 0, 0);

INSERT INTO history(history_id, history_chapter_id, history_last_read, history_time_read)
VALUES (1, 1, 1700000100000, 5000), (2, 98, 1, 1), (3, 2, NULL, NULL);

INSERT INTO categories(_id, name, sort, flags) VALUES (1, 'Reading', 0, 8), (2, 'Later', 1, 0);
INSERT INTO mangas_categories(_id, manga_id, category_id) VALUES (1, 1, 1), (2, 3, 2);

INSERT INTO manga_sync(_id, manga_id, sync_id, remote_id, library_id, title, last_chapter_read, total_chapters, status, score, remote_url, start_date, finish_date)
VALUES
    (1, 1, 1, 100, 5, 'One', 1.0, 20, 1, 8.0, 'http://t/1', 0, 0),
    (2, 3, 7, 200, NULL, 'Three', 0.0, 0, 1, -1.0, '', 0, 0);
