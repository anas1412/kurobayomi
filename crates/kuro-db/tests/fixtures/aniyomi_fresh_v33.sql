-- Kurobayomi manga database, baseline schema version 33 (== Aniyomi data/src/main/sqldelight at HEAD)
CREATE TABLE mangas(
    _id INTEGER NOT NULL PRIMARY KEY,
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
    next_update INTEGER,
    initialized INTEGER NOT NULL,
    viewer INTEGER NOT NULL,
    chapter_flags INTEGER NOT NULL,
    cover_last_modified INTEGER NOT NULL,
    date_added INTEGER NOT NULL,
    update_strategy INTEGER NOT NULL DEFAULT 0,
    calculate_interval INTEGER DEFAULT 0 NOT NULL,
    last_modified_at INTEGER NOT NULL DEFAULT 0,
    favorite_modified_at INTEGER,
    version INTEGER NOT NULL DEFAULT 0,
    is_syncing INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX library_favorite_index ON mangas(favorite) WHERE favorite = 1;
CREATE INDEX mangas_url_index ON mangas(url);
CREATE TRIGGER update_last_favorite_at_mangas
AFTER UPDATE OF favorite ON mangas
BEGIN
  UPDATE mangas
  SET favorite_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;
CREATE TRIGGER update_last_modified_at_mangas
AFTER UPDATE ON mangas
FOR EACH ROW
BEGIN
  UPDATE mangas
  SET last_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;
CREATE TRIGGER update_manga_version AFTER UPDATE ON mangas
BEGIN
    UPDATE mangas SET version = version + 1
    WHERE _id = new._id AND new.is_syncing = 0 AND (
        new.url != old.url OR
        new.description != old.description OR
        new.favorite != old.favorite
    );
END;

CREATE TABLE categories(
    _id INTEGER NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    sort INTEGER NOT NULL,
    flags INTEGER NOT NULL,
    hidden INTEGER NOT NULL DEFAULT 0
);
INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, '', -1, 0);
CREATE TRIGGER IF NOT EXISTS system_category_delete_trigger BEFORE DELETE
ON categories
BEGIN SELECT CASE
    WHEN old._id <= 0 THEN
        RAISE(ABORT, 'System category can''t be deleted')
    END;
END;

CREATE TABLE chapters(
    _id INTEGER NOT NULL PRIMARY KEY,
    manga_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    name TEXT NOT NULL,
    scanlator TEXT,
    read INTEGER NOT NULL,
    bookmark INTEGER NOT NULL,
    last_page_read INTEGER NOT NULL,
    chapter_number REAL NOT NULL,
    source_order INTEGER NOT NULL,
    date_fetch INTEGER NOT NULL,
    date_upload INTEGER NOT NULL,
    last_modified_at INTEGER NOT NULL DEFAULT 0,
    version INTEGER NOT NULL DEFAULT 0,
    is_syncing INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id)
    ON DELETE CASCADE
);
CREATE INDEX chapters_manga_id_index ON chapters(manga_id);
CREATE INDEX chapters_unread_by_manga_index ON chapters(manga_id, read) WHERE read = 0;
CREATE TRIGGER update_last_modified_at_chapters
AFTER UPDATE ON chapters
FOR EACH ROW
BEGIN
  UPDATE chapters
  SET last_modified_at = strftime('%s', 'now')
  WHERE _id = new._id;
END;
CREATE TRIGGER update_chapter_and_manga_version AFTER UPDATE ON chapters
WHEN new.is_syncing = 0 AND (
    new.read != old.read OR
    new.bookmark != old.bookmark OR
    new.last_page_read != old.last_page_read
)
BEGIN
    -- Update the chapter version
    UPDATE chapters SET version = version + 1
    WHERE _id = new._id;

    -- Update the manga version
    UPDATE mangas SET version = version + 1
    WHERE _id = new.manga_id AND (SELECT is_syncing FROM mangas WHERE _id = new.manga_id) = 0;
END;

CREATE TABLE excluded_scanlators(
    manga_id INTEGER NOT NULL,
    scanlator TEXT NOT NULL,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id)
    ON DELETE CASCADE
);
CREATE INDEX excluded_scanlators_manga_id_index ON excluded_scanlators(manga_id);

CREATE TABLE history(
    _id INTEGER NOT NULL PRIMARY KEY,
    chapter_id INTEGER NOT NULL UNIQUE,
    last_read INTEGER,
    time_read INTEGER NOT NULL,
    FOREIGN KEY(chapter_id) REFERENCES chapters (_id)
    ON DELETE CASCADE
);
CREATE INDEX history_history_chapter_id_index ON history(chapter_id);

CREATE TABLE manga_sync(
    _id INTEGER NOT NULL PRIMARY KEY,
    manga_id INTEGER NOT NULL,
    sync_id INTEGER NOT NULL,
    remote_id INTEGER NOT NULL,
    library_id INTEGER,
    title TEXT NOT NULL,
    last_chapter_read REAL NOT NULL,
    total_chapters INTEGER NOT NULL,
    status INTEGER NOT NULL,
    score REAL NOT NULL,
    remote_url TEXT NOT NULL,
    start_date INTEGER NOT NULL,
    finish_date INTEGER NOT NULL,
    private INTEGER DEFAULT 0 NOT NULL,
    UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id)
    ON DELETE CASCADE
);

CREATE TABLE mangas_categories(
    _id INTEGER NOT NULL PRIMARY KEY,
    manga_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL,
    FOREIGN KEY(category_id) REFERENCES categories (_id)
    ON DELETE CASCADE,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id)
    ON DELETE CASCADE
);
CREATE TRIGGER insert_manga_category_update_version AFTER INSERT ON mangas_categories
BEGIN
    UPDATE mangas
    SET version = version + 1
    WHERE _id = new.manga_id AND (SELECT is_syncing FROM mangas WHERE _id = new.manga_id) = 0;
END;

CREATE TABLE sources(
    _id INTEGER NOT NULL PRIMARY KEY,
    lang TEXT NOT NULL,
    name TEXT NOT NULL
);

CREATE TABLE extension_repos (
    base_url TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    short_name TEXT,
    website TEXT NOT NULL,
    signing_key_fingerprint TEXT UNIQUE NOT NULL
);

CREATE VIEW historyView AS
SELECT
    history._id AS id,
    mangas._id AS mangaId,
    chapters._id AS chapterId,
    mangas.title,
    mangas.thumbnail_url AS thumbnailUrl,
    mangas.source,
    mangas.favorite,
    mangas.cover_last_modified,
    chapters.chapter_number AS chapterNumber,
    history.last_read AS readAt,
    history.time_read AS readDuration,
    max_last_read.last_read AS maxReadAt,
    max_last_read.chapter_id AS maxReadAtChapterId
FROM mangas
JOIN chapters
ON mangas._id = chapters.manga_id
JOIN history
ON chapters._id = history.chapter_id
JOIN (
    SELECT chapters.manga_id,chapters._id AS chapter_id, MAX(history.last_read) AS last_read
    FROM chapters JOIN history
    ON chapters._id = history.chapter_id
    GROUP BY chapters.manga_id
) AS max_last_read
ON chapters.manga_id = max_last_read.manga_id;

CREATE VIEW libraryView AS
SELECT
    M.*,
    coalesce(C.total, 0) AS totalCount,
    coalesce(C.readCount, 0) AS readCount,
    coalesce(C.latestUpload, 0) AS latestUpload,
    coalesce(C.fetchedAt, 0) AS chapterFetchedAt,
    coalesce(C.lastRead, 0) AS lastRead,
    coalesce(C.bookmarkCount, 0) AS bookmarkCount,
    coalesce(MC.category_id, 0) AS category
FROM mangas M
LEFT JOIN(
    SELECT
        chapters.manga_id,
        count(*) AS total,
        sum(read) AS readCount,
        coalesce(max(chapters.date_upload), 0) AS latestUpload,
        coalesce(max(history.last_read), 0) AS lastRead,
        coalesce(max(chapters.date_fetch), 0) AS fetchedAt,
        sum(chapters.bookmark) AS bookmarkCount
    FROM chapters
    LEFT JOIN excluded_scanlators
    ON chapters.manga_id = excluded_scanlators.manga_id
    AND chapters.scanlator = excluded_scanlators.scanlator
    LEFT JOIN history
    ON chapters._id = history.chapter_id
    WHERE excluded_scanlators.scanlator IS NULL
    GROUP BY chapters.manga_id
) AS C
ON M._id = C.manga_id
LEFT JOIN mangas_categories AS MC
ON MC.manga_id = M._id
WHERE M.favorite = 1;

CREATE VIEW updatesView AS
SELECT
    mangas._id AS mangaId,
    mangas.title AS mangaTitle,
    chapters._id AS chapterId,
    chapters.name AS chapterName,
    chapters.scanlator,
    chapters.read,
    chapters.bookmark,
    chapters.last_page_read,
    mangas.source,
    mangas.favorite,
    mangas.thumbnail_url AS thumbnailUrl,
    mangas.cover_last_modified AS coverLastModified,
    chapters.date_upload AS dateUpload,
    chapters.date_fetch AS datefetch
FROM mangas JOIN chapters
ON mangas._id = chapters.manga_id
WHERE favorite = 1
AND date_fetch > date_added
ORDER BY date_fetch DESC;
PRAGMA user_version = 33;
