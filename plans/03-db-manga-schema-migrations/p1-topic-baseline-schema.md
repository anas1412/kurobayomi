# Area 03 / part 1 — Topic: canonical baseline schema (version 33) as Rust-ready SQL

Source of truth: the `CREATE ...` statements at the top of every `.sq` file under `data/src/main/sqldelight/` (paths are repo-relative to `/home/blackbox/aniyomi-src`). The generated Kotlin `Database.Schema.create()` executes exactly these statements, and `Database.Schema.version` is `33` because the highest migration is `data/src/main/sqldelight/migrations/32.sqm` (a file `N.sqm` upgrades a database from version N to N+1).

OBSERVED (verified by executing the SQL in SQLite 3.53 with `sqlite3`, and with `rusqlite 0.40.2` + `libsqlite3-sys 0.38.2` bundled = SQLite 3.53.2): the DDL below, executed on an empty database, produces 9 tables, 10 indexes (6 named + 4 `sqlite_autoindex_*`), 7 triggers and 3 views.

## 1. What was changed relative to the verbatim `.sq` text (and why)

Each change is mechanical and does not alter runtime behaviour; they are the only deviations.

| # | `.sq` text | Baseline text | Reason |
|---|---|---|---|
| 1 | `import ...;` lines (`kotlin.Boolean`, `java.util.Date`, `kotlin.String`, `kotlin.collections.List`, `eu.kanade.tachiyomi.source.model.UpdateStrategy`) | removed | SQLDelight-only syntax, not SQL |
| 2 | `INTEGER AS Boolean`, `INTEGER AS Date`, `TEXT AS List<String>`, `INTEGER AS UpdateStrategy` (column type annotations in `chapters.sq`, `history.sq`, `manga_sync.sq`, `mangas.sq`) | plain `INTEGER` / `TEXT` | SQLDelight-only type adapters; Rust uses codecs in `kuro-db::codec` (see `p1-rust-design.md`) |
| 3 | `VALUES (0, "", -1, 0)` and `RAISE(ABORT, "System category can't be deleted")` (double-quoted strings, in `categories.sq` and `migrations/19.sqm`) | `VALUES (0, '', -1, 0)` and `RAISE(ABORT, 'System category can''t be deleted')` | SQLite treats a double-quoted token as an identifier first and only falls back to a string literal when "DQS" (double-quoted string literals) is enabled at compile time. `libsqlite3-sys 0.38.2` bundled keeps DQS on (verified: `INSERT INTO d VALUES ("")` succeeds) but the Arch system `sqlite3` binary has it off (verified: `no such column: ""`). Single quotes are correct under both |
| 4 | `history.last_read INTEGER AS Date` (nullable in fresh `.sq`) | `last_read INTEGER NOT NULL` | Migration `15.sqm` made the column `NOT NULL`; every database that went through migration 15 has `NOT NULL`, only brand-new Aniyomi installs have the nullable variant. `history.upsert` always writes a value, and `historyView` filters `readAt > 0`, so no query depends on NULL. See section 4 |
| 5 | trigger `update_last_favorite_at_mangas` | same name kept | migrated databases name it `update_last_favorited_at_mangas` (migration `26.sqm`); body is identical. See section 4 |

Everything else (column order, defaults, `ON DELETE CASCADE`, partial indexes, trigger bodies, view bodies) is verbatim. Column ORDER matters: `SELECT *` in `mangas.sq`, `chapters.sq`, `libraryView.sq` is mapped positionally in the Kotlin mappers (`data/src/main/java/tachiyomi/data/entries/manga/MangaMapper.kt`, `data/src/main/java/tachiyomi/data/items/chapter/ChapterRepositoryImpl.kt`), and the backup creator relies on the same order.

## 2. The baseline file

Proposal: store this text as `crates/kuro-db/sql/schema_v33.sql`, embed it with `include_str!`, and execute it with `Connection::execute_batch` inside one transaction followed by `PRAGMA user_version = 33`.

```sql
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
    last_read INTEGER NOT NULL,
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
```

After executing: `PRAGMA user_version = 33;`

## 3. Object inventory of the baseline (what a conformance test must assert)

Tables (9): `mangas`, `categories`, `chapters`, `excluded_scanlators`, `history`, `manga_sync`, `mangas_categories`, `sources`, `extension_repos`.

Named indexes (6): `library_favorite_index` (partial, `favorite = 1`), `mangas_url_index`, `chapters_manga_id_index`, `chapters_unread_by_manga_index` (partial, `read = 0`), `excluded_scanlators_manga_id_index`, `history_history_chapter_id_index` (redundant with the UNIQUE autoindex but part of the schema).

Automatic indexes (4): `sqlite_autoindex_history_1` (from `chapter_id INTEGER NOT NULL UNIQUE`), `sqlite_autoindex_manga_sync_1` (from `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`), `sqlite_autoindex_extension_repos_1` (text primary key `base_url`), `sqlite_autoindex_extension_repos_2` (`signing_key_fingerprint` UNIQUE).

Triggers (7): `system_category_delete_trigger`, `update_last_favorite_at_mangas`, `update_last_modified_at_mangas`, `update_manga_version`, `update_last_modified_at_chapters`, `update_chapter_and_manga_version`, `insert_manga_category_update_version`.

Views (3): `historyView`, `libraryView`, `updatesView`.

Seed row: `categories(_id=0, name='', sort=-1, flags=0, hidden=0)`.

## 4. Verified differences between a fresh Aniyomi database and one upgraded through migrations 14..32

Method: a version-13 state was reconstructed from the column lists that `migrations/14.sqm` selects (the version-1 base schema is not in the repository: there is no `.db` snapshot under `data/src/main/sqldelight/`), migrations 14 to 32 were applied in order with `sqlite3`, and `sqlite_master` plus `pragma_table_info` were diffed against the fresh schema from the `.sq` files.

| Object | Fresh install (`.sq`) | Upgraded install | Effect |
|---|---|---|---|
| `history.last_read` | nullable `INTEGER` | `INTEGER NOT NULL` | none at runtime |
| `mangas_categories.last_modified_at` | column absent | `INTEGER NOT NULL DEFAULT 0` (added by `26.sqm`) | `SELECT *` on `mangas_categories` is never used; no effect |
| trigger `update_last_modified_at_mangas_categories` | absent | present (created by `26.sqm`, `AFTER UPDATE ON mangas_categories`) | no code path updates `mangas_categories` rows (only insert and delete), so it never fires |
| trigger `update_last_favorite_at_mangas` vs `update_last_favorited_at_mangas` | `update_last_favorite_at_mangas` | `update_last_favorited_at_mangas` (`26.sqm` drops a non-existent `update_favorite_modified_at_mangas` and creates the "favorited" name) | identical body, different name |
| table `history_temp` (+ `sqlite_autoindex_history_temp_1`) | absent | present: `15.sqm` renames `history` to `history_temp` and never drops it | stale copy of pre-15 rows with `ON DELETE CASCADE` to `chapters`; wasted space, no reader |

Decision (Proposal): the baseline follows the fresh schema except `history.last_read NOT NULL`; it never contains `history_temp`, `mangas_categories.last_modified_at` or the extra trigger. The legacy importer (`T-03a-023`) must tolerate all five differences when reading an Aniyomi `tachiyomi.db` and must `DROP TABLE IF EXISTS history_temp` on its working copy.

## 5. Optional post-33 additions (not part of the baseline)

Proposal, delivered as Kurobayomi migration `34` by `T-03a-024` (not before the first release that needs it). Each item was measured on a synthetic database built with `sqlite3`: 5000 manga, 300000 chapters (every 7th with a scanlator), 50000 history rows, 7000 `mangas_categories` rows, 26 MB, SQLite 3.53.4, warm cache.

```sql
CREATE INDEX chapters_url_index ON chapters(url);

DROP VIEW libraryView;
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
    AND chapters.manga_id IN (SELECT _id FROM mangas WHERE favorite = 1)
    GROUP BY chapters.manga_id
) AS C
ON M._id = C.manga_id
LEFT JOIN mangas_categories AS MC
ON MC.manga_id = M._id
WHERE M.favorite = 1;
```

| Measurement | Baseline | With migration 34 | Conclusion |
|---|---|---|---|
| `SELECT * FROM chapters WHERE url = '/c/299999'` | 16.8 ms (`SCAN chapters`) | 0.05 ms | add `chapters_url_index` (used by `getChapterByUrl`, `getHistoryByChapterUrl` and the backup restorer once per history entry) |
| `libraryView`, all 5000 manga favourites | 259 ms | 251 ms with an extra index `mangas_categories(manga_id)` | that index has no measurable benefit: NOT proposed (nor one on `mangas_categories(category_id)`) |
| `libraryView` with 500 favourites out of 5000 manga (same 300000 chapters) | 246 ms | 34 ms with the `chapters.manga_id IN (SELECT _id FROM mangas WHERE favorite = 1)` pre-filter above | worthwhile: the stock view aggregates the chapters of every non-library entry browsed before; results are identical (the pre-filter only removes groups whose manga is excluded by `WHERE M.favorite = 1` anyway) |
| `historyView WHERE readAt > 0 AND maxReadAtChapterId = chapterId` (5000 rows out) | 27 ms | not changed | fine |
| `updatesView WHERE dateUpload > 0 LIMIT 500` | 122 ms | not changed | acceptable; the view-level `ORDER BY date_fetch DESC` forces a full sort. Optional later: an index on `chapters(date_fetch)` would let SQLite stream in order, not measured |

Consequence for the UI: a library screen subscribed to `libraryView` re-runs the 250 ms query after EVERY commit that touches `mangas`, `chapters`, `excluded_scanlators`, `history` or `mangas_categories` (including the per-page `chapters.last_page_read` writes of the reader). `T-03a-019` therefore requires subscriptions to be paused while their screen is not visible and to be debounced by the UI layer (Proposal: 300 ms trailing) without changing the SQL.
