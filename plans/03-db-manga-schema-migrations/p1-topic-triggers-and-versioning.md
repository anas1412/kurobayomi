# Area 03 / part 1 — Topic: triggers, `version`, `is_syncing`, `last_modified_at`, `favorite_modified_at`

Source: trigger definitions in `data/src/main/sqldelight/data/mangas.sq:36-61`, `chapters.sq:26-49`, `mangas_categories.sq:11-16`, `categories.sq:11-18`. Every number below was produced by executing the baseline schema in SQLite (3.53) and, for the hook behaviour, in `rusqlite 0.40.2` (bundled SQLite 3.53.2).

## 1. Purpose

The columns exist for Aniyomi's cross-device sync feature (a conflict resolver that compares `version` and `last_modified_at`). Within this repository only the backup code consumes them:

- `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/MangaBackupCreator.kt:99-101` writes `lastModifiedAt`, `favoriteModifiedAt`, `version` into the backup (`app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupManga.kt`: `@ProtoNumber(106) lastModifiedAt`, `@ProtoNumber(107) favoriteModifiedAt`, `@ProtoNumber(109) version`; `BackupChapter.kt`: `@ProtoNumber(11) lastModifiedAt`).
- `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt:45-54` (`sortByNew`) restores entries not yet in the database first and orders by `lastModifiedAt` descending; `restoreExistingManga` (line 85) compares `manga.version > dbManga.version` to decide which side's metadata wins; `Chapter.forComparison()` (line 189) zeroes `id, mangaId, dateFetch, dateUpload, lastModifiedAt, version` before equality checks.

No UI shows these columns. Kurobayomi keeps them because (a) backups interoperate with Aniyomi (area:21-backup-format-and-create, area:22-backup-restore) and (b) the triggers cost nothing.

## 2. Trigger catalogue (7)

| # | Trigger | Event / condition | Body (effect) |
|---|---|---|---|
| T1 | `system_category_delete_trigger` | `BEFORE DELETE ON categories` (any row) | `SELECT CASE WHEN old._id <= 0 THEN RAISE(ABORT, 'System category can''t be deleted') END;` -> deleting `_id` 0 (or any negative id) aborts the statement with that message; user ids (>= 1) pass |
| T2 | `update_last_favorite_at_mangas` | `AFTER UPDATE OF favorite ON mangas` (fires when `favorite` appears in the `SET` list, even if the value is unchanged) | `UPDATE mangas SET favorite_modified_at = strftime('%s', 'now') WHERE _id = new._id;` |
| T3 | `update_last_modified_at_mangas` | `AFTER UPDATE ON mangas FOR EACH ROW` | `UPDATE mangas SET last_modified_at = strftime('%s', 'now') WHERE _id = new._id;` |
| T4 | `update_manga_version` | `AFTER UPDATE ON mangas` | `UPDATE mangas SET version = version + 1 WHERE _id = new._id AND new.is_syncing = 0 AND (new.url != old.url OR new.description != old.description OR new.favorite != old.favorite);` |
| T5 | `update_last_modified_at_chapters` | `AFTER UPDATE ON chapters FOR EACH ROW` | `UPDATE chapters SET last_modified_at = strftime('%s', 'now') WHERE _id = new._id;` |
| T6 | `update_chapter_and_manga_version` | `AFTER UPDATE ON chapters WHEN new.is_syncing = 0 AND (new.read != old.read OR new.bookmark != old.bookmark OR new.last_page_read != old.last_page_read)` | (a) `UPDATE chapters SET version = version + 1 WHERE _id = new._id;` (b) `UPDATE mangas SET version = version + 1 WHERE _id = new.manga_id AND (SELECT is_syncing FROM mangas WHERE _id = new.manga_id) = 0;` |
| T7 | `insert_manga_category_update_version` | `AFTER INSERT ON mangas_categories` | `UPDATE mangas SET version = version + 1 WHERE _id = new.manga_id AND (SELECT is_syncing FROM mangas WHERE _id = new.manga_id) = 0;` |

## 3. Semantics that surprise (all verified)

1. Units. `strftime('%s', 'now')` returns TEXT of UNIX SECONDS; because the destination columns are `INTEGER`, SQLite converts it to an integer (`typeof` = `integer`). `mangas.last_modified_at`, `mangas.favorite_modified_at`, `chapters.last_modified_at` are therefore SECONDS, while every other timestamp in the schema is MILLISECONDS. The `insert` statements write literal `0` into `last_modified_at`.
2. Recursion is off. `PRAGMA recursive_triggers` is 0 by default in SQLite and in `libsqlite3-sys` bundled (verified: `recursive_triggers = 0`), so T3 (an UPDATE on `mangas` inside a trigger on `mangas`) does not recurse infinitely. A trigger program is skipped only when the SAME trigger is already on the call stack; different triggers still nest (a chapter update fires T5, T6, and through T6 fires T3/T4 on `mangas`). Kurobayomi MUST NOT enable `recursive_triggers`.
3. NULL-blind description check. In T4 `new.description != old.description` is NULL (false) when either side is NULL. Verified: description `NULL -> 'x'` leaves `version` at 0, `'d' -> NULL` leaves `version` at 0, `'d' -> 'y'` bumps to 1. The first fetch of metadata for an entry (description NULL -> text) therefore never counts as a change. Keep verbatim (do not wrap in `IS NOT`).
4. `is_syncing` is read from the NEW row of the same `UPDATE`. Kotlin repositories always write `is_syncing = 0` in `mangas.update` and `chapters.update` (`isSyncing = 0` in `MangaRepositoryImpl.partialUpdateManga` and `ChapterRepositoryImpl.partialUpdate`), so ordinary edits are "counted". Only `MangaRestorer.updateManga` writes `is_syncing = 1` (restore from backup must not bump versions); the flag stays 1 until the next ordinary `mangas.update` for that row resets it (`resetIsSyncing` queries exist but have no caller).
5. T6/T7 consult the PARENT manga's current `is_syncing` through a sub-select; a manga with `is_syncing = 1` does not bump its version when its chapters change, but the chapter's own `version` still bumps (verified: chapter 2 -> 3 while manga stayed 6).
6. Multiple categories multiply the bump. `setMangaCategories` inserts one `mangas_categories` row per category, each firing T7: assigning 3 categories raised `mangas.version` from 3 to 6 (verified). Deleting `mangas_categories` rows does not fire any trigger. Deleting a category (cascade) does not bump either.
7. `favorite` toggles fire T2 + T3 + T4: `favorite_modified_at` set, `last_modified_at` set, `version + 1` (once, even if `url` also changed in the same statement; verified `fav+url` raised version by exactly 1).
8. Every `UPDATE` on `mangas`, including `resetViewerFlags` (`UPDATE mangas SET viewer = 0` with no WHERE) fires T3 for every row: it rewrites `last_modified_at` of the whole table. Same for `chapters` via T5.
9. Name differences of the favourite trigger between fresh and upgraded databases are documented in `p1-topic-baseline-schema.md` section 4.

## 4. Expected-value test matrix (to be turned into Rust tests by `T-03a-020`)

Start: manga 1 (`favorite = 0`, `description = 'd'`, `is_syncing = 0`, `version = 0`), chapter 1 (`read = 0`, `bookmark = 0`, `last_page_read = 0`, `version = 0`), categories 1..3 present.

| Step (SQL executed) | manga.version after | chapter.version after | Other assertions |
|---|---|---|---|
| `UPDATE chapters SET read = 1 WHERE _id = 1` | 1 | 1 | `chapters.last_modified_at > 0` |
| `UPDATE chapters SET name = 'n2' WHERE _id = 1` | 1 | 1 | `last_modified_at` refreshed |
| `UPDATE chapters SET last_page_read = 5, bookmark = 1 WHERE _id = 1` | 2 | 2 | one bump for two changed columns |
| `UPDATE mangas SET favorite = 1, url = '/a9' WHERE _id = 1` | 3 | 2 | `favorite_modified_at > 0` |
| `UPDATE mangas SET title = 'T2' WHERE _id = 1` | 3 | 2 | `last_modified_at` refreshed only |
| `INSERT INTO mangas_categories(manga_id, category_id) VALUES (1,1),(1,2),(1,3)` | 6 | 2 | three bumps |
| `UPDATE mangas SET is_syncing = 1 WHERE _id = 1` | 6 | 2 | - |
| `UPDATE chapters SET read = 0, is_syncing = 0 WHERE _id = 1` | 6 | 3 | manga did not bump (syncing) |
| `UPDATE chapters SET read = 1, is_syncing = 1 WHERE _id = 1` | 6 | 3 | statement-level `is_syncing = 1` suppresses T6 |
| `UPDATE mangas SET favorite = 0, is_syncing = 0 WHERE _id = 1` | 7 | 3 | flag reset and counted in the same statement |
| `DELETE FROM categories WHERE _id = 0` | - | - | statement fails with message `System category can't be deleted`; row still present |
| Description tests: `NULL -> 'x'`, `'d' -> NULL`, `'d' -> 'y'` | 0, 0, 1 (starting from 0 each) | - | NULL-blind rule |

## 5. How the Rust app must preserve the behaviour

1. The triggers live in the baseline SQL (`p1-topic-baseline-schema.md`); no Rust code replicates them. The only Rust obligations: (a) never set `PRAGMA recursive_triggers = ON`; (b) always send `is_syncing` explicitly (0) in ordinary updates exactly like Kotlin, via the single `coalesce` update statement; (c) bulk backup/sync restores that must not bump versions set `is_syncing = 1` on the row in the same statement (as `MangaRestorer.updateManga` does) or insert before the triggers exist (legacy importer).
2. Table-change notification (the Rust replacement for SQLDelight's query listeners) must treat trigger-driven writes as changes; see `p1-topic-reactive-runtime.md`.
3. `last_modified_at`/`favorite_modified_at` are written by SQLite's own clock (UTC seconds), not by Rust. Tests that assert timestamps must compare with `>= start_of_test_seconds`, not equality.
