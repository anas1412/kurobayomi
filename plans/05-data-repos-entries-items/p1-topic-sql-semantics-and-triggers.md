# Area 05 — topic: SQL semantics the repositories depend on (coalesce updates, triggers, versions, notifications)

Paths repo-relative to `/home/blackbox/aniyomi-src`. The repositories look thin (every method is one SQL call) but the behaviour lives in the SQL. A Rust port that reimplements the repositories with "natural" SQL (dynamic `SET`, no triggers, `INSERT OR REPLACE`) will silently diverge from what backups, restore-merging and the UI rely on. This document lists each such dependency. Schema ownership: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations.

## 1. Partial update = `coalesce(:param, column)`

OBSERVED. Every `update` statement (mangas.sq, animes.sq, chapters.sq, episodes.sq, categories.sq) assigns each column as `col = coalesce(:param, col)`. The Kotlin `*Update` classes have every field nullable with default `null`, so "field omitted" and "set to NULL" are the same thing and mean "leave unchanged".

Consequences (all OBSERVED, all must be preserved):

1. A nullable column can never be set back to NULL through `update`. This is how the app gets its behaviour for: artist, author, description, genre, thumbnail_url, background_url, scanlator (an `EpisodeUpdate`/`ChapterUpdate` with `scanlator = null` keeps the old scanlator), summary, preview_url, parent_id (hence the dedicated `removeParentIdByIds` statement), `last_update`, `next_update`.
2. A column can be set to an empty string (`""`) or `0`; those are real values and replace the old one.
3. Boolean parameters bind `1`/`0`/NULL (SQLDelight stores Boolean as INTEGER 0/1).
4. `categories.update` uses the same pattern, so `CategoryUpdate(id, order = 3)` changes only `sort`.
5. `categories.updateAllFlags` is `UPDATE categories SET flags = coalesce(?, flags)` with no `WHERE`: it updates every row including the system category (id 0) and is a no-op when the bound value is NULL.

Proposal: the Rust update structs keep `Option<T>` fields with the same meaning (`None` = keep). Generate the SQL once per table as the static coalesce statement and use `Connection::prepare_cached` so the statement is compiled once. Do not build a dynamic `SET` list: with a dynamic list the `AFTER UPDATE OF favorite` trigger (section 3.2) would stop firing on every update, which changes `favorite_modified_at` behaviour that restore-merging reads (section 4).

## 2. `is_syncing` is always reset to 0

OBSERVED. `MangaRepositoryImpl.partialUpdateManga`, `AnimeRepositoryImpl.partialUpdateAnime`, `ChapterRepositoryImpl.partialUpdate`, `EpisodeRepositoryImpl.partialUpdate` all pass `isSyncing = 0`, so every repository update ends with `is_syncing = coalesce(0, is_syncing) = 0`. The column is only ever non-zero if something else writes it (the SyncYomi sync feature, which is dropped, see T-05a-029). The triggers test `new.is_syncing = 0`, which is therefore always true for repository writes.

Proposal: keep the column and keep writing 0 (schema compatibility with area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations); do not expose it on the Rust update structs. The `resetIsSyncing` statements in mangas.sq, animes.sq, chapters.sq, episodes.sq are never called anywhere in the Kotlin tree (searched `app`, `data`, `domain`); no Rust method is needed.

## 3. Triggers (full list and exact behaviour)

### 3.1 `last_modified_at` bookkeeping

`update_last_modified_at_mangas` / `_animes` / `_chapters` / `_episodes`: `AFTER UPDATE ... FOR EACH ROW BEGIN UPDATE <table> SET last_modified_at = strftime('%s', 'now') WHERE _id = new._id; END`. `strftime('%s','now')` is the current time in epoch SECONDS (as TEXT, stored into an INTEGER column and converted by affinity). All other timestamps in these tables (`date_fetch`, `date_upload`, `date_added`, `last_update`, `next_update`, `cover_last_modified`) are epoch MILLISECONDS. `INSERT` statements write `last_modified_at = 0`; the trigger only fires on UPDATE, so a freshly inserted row has `last_modified_at = 0` until its first update.

Because the trigger's inner `UPDATE` is itself an UPDATE, SQLite would recurse; it does not because `PRAGMA recursive_triggers` is OFF by default (a trigger never re-fires itself). The inner update does fire the other AFTER UPDATE triggers on the table (version trigger), which find no relevant change.

### 3.2 `favorite_modified_at`

`update_last_favorite_at_mangas` / `_animes`: `AFTER UPDATE OF favorite ON mangas BEGIN UPDATE mangas SET favorite_modified_at = strftime('%s', 'now') WHERE _id = new._id; END`. SQLite fires an `UPDATE OF col` trigger whenever `col` appears in the statement's SET list, regardless of whether the value changed. The repositories' `update` statement always lists `favorite = coalesce(:favorite, favorite)`. Therefore EVERY `updateManga`/`updateAnime` call refreshes `favorite_modified_at` (seconds), also when only the title changed. `insert` never sets it (NULL until the first update). `favorite_modified_at` is exported in backups (`MangaBackupCreator.kt:100`, `AnimeBackupCreator.kt:94`) and was designed for sync conflict resolution.

Proposal: preserve (static statement, trigger unchanged). Document the quirk in code comments; do not "fix" it, because restoring an Aniyomi backup and then restoring again would flip merge decisions.

### 3.3 Version counters (`version`)

`update_manga_version` / `update_anime_version`: `AFTER UPDATE ON mangas BEGIN UPDATE mangas SET version = version + 1 WHERE _id = new._id AND new.is_syncing = 0 AND (new.url != old.url OR new.description != old.description OR new.favorite != old.favorite); END`.

SQL three-valued logic matters: `new.description != old.description` is NULL (treated as false) when either side is NULL. So changing description from NULL to text, or text to NULL, does NOT bump the version; changing text to different text does. `url` and `favorite` are NOT NULL columns so they compare normally.

`update_chapter_and_manga_version`: `AFTER UPDATE ON chapters WHEN new.is_syncing = 0 AND (new.read != old.read OR new.bookmark != old.bookmark OR new.last_page_read != old.last_page_read)` then `UPDATE chapters SET version = version + 1 WHERE _id = new._id;` and `UPDATE mangas SET version = version + 1 WHERE _id = new.manga_id AND (SELECT is_syncing FROM mangas WHERE _id = new.manga_id) = 0;`. So marking N chapters read in one transaction raises `mangas.version` by N. The episode counterpart `update_episode_and_anime_version` watches `seen`, `bookmark`, `last_second_seen` (not `fillermark`, not `total_seconds`).

`insert_manga_category_update_version` / `insert_anime_category_update_version`: `AFTER INSERT ON mangas_categories` (or `animes_categories`) adds 1 to the parent's version per inserted link row; the `DELETE` of old links does not bump. So `setMangaCategories(id, [a, b])` adds 2; `setMangaCategories(id, [])` adds 0.

Passing `version = x` explicitly in an update sets the column to `x` first (`coalesce(:version, version)`), then the trigger may add 1 on top. The restore code uses this (AnimeRestorer.kt:100-167 compares `anime.version > dbAnime.version` and copies the newer one).

Chapter `version` default on insert: `addAllChapters` binds `chapter.version` (domain `Chapter.create()` sets 1; the DDL default of 0 applies only to direct SQL inserts).

### 3.4 System category guard

`system_category_delete_trigger BEFORE DELETE ON categories ... RAISE(ABORT, "System category can't be deleted") WHEN old._id <= 0`. `deleteMangaCategory(0)` therefore throws a database error. The seed row is `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0)` executed at schema creation (note the double-quoted literals; SQLite accepts them as strings when no such column exists; in a Rust `execute_batch` use single quotes: `VALUES (0, '', -1, 0)` which is identical in effect).

## 4. Why versions and timestamps matter outside this area

- Backup creation exports `lastModifiedAt`, `favoriteModifiedAt`, `version` for manga, anime, chapters and episodes (area:21-backup-format-and-create). Backup restore picks the newer record by `version` and orders chapter restoration by `lastModifiedAt` (MangaRestorer.kt:52 `compareByDescending { it.lastModifiedAt }`, AnimeRestorer.kt:100) (area:22-backup-restore).
- Restore resets `lastModifiedAt = 0L, version = 0L` on freshly inserted chapter copies (MangaRestorer.kt:190).
- Two timestamp units coexist: seconds (`last_modified_at`, `favorite_modified_at`) and milliseconds (everything else). The Rust structs must name the unit in the doc comment, and `kuro_core::data::time` must expose `fn now_secs()` and `fn now_millis()` so nobody passes the wrong one.

## 5. Row order of unsorted queries

`getFavorites`, `getFavoriteBySourceId`, `getChaptersByMangaId`, `getEpisodesByAnimeId`, `getCategoriesBy*Id`, `getChildrenByParentId` have no `ORDER BY`; SQLite returns rows in the plan's natural order (rowid for a table scan, index order when an index is used). Callers sort in memory: chapter lists are sorted in `ChapterSorter`/`getChapterSort` (area:07-domain-entries-items), library lists in the library screen model (area:37-ui-library). Proposal: repositories keep returning unsorted results; where a Rust test compares lists it must sort first or use `ORDER BY _id` only inside the test helper.

`getCategories` and `getVisibleCategories` use `ORDER BY sort` only. Proposal: append `, _id` as a tie-breaker (deterministic; matches the practical SQLite order because the system row has the smallest rowid). This is the one deliberate SQL deviation in the area, see `p1-topic-categories.md` section 4.

## 6. `IN :list` parameters

SQLDelight expands `IN :ids` to `IN (?, ?, ...)` with one placeholder per element, built at call time. Affected statements: `removeChaptersWithIds`, `removeEpisodesWithIds`, `removeParentIdByIds`, `getUpcomingManga/Anime` (`status IN :statuses`). An empty list yields `IN ()`, which SQLite accepts and which matches nothing.

Proposal: a helper `fn in_clause(n: usize) -> String` (returns `"(?,?,?)"`, or `"()"` for 0) and `chunks(500)` over the id list for the two DELETE statements and `removeParentIdByIds`, all inside one transaction so partial deletion cannot be observed. SQLite's default `SQLITE_MAX_VARIABLE_NUMBER` is 32766 since 3.32 and 999 before; a library with tens of thousands of chapters can exceed 999 in one `removeChaptersWithIds` call, and chunking keeps the Rust code correct on whichever SQLite the system provides.

## 7. Insert-and-return-id

OBSERVED. `insertManga`/`insertAnime` run the INSERT then `SELECT last_insert_rowid()` inside one transaction (`awaitOneOrNullExecutable(inTransaction = true)`) and return `Long?`; `addAllChapters`/`addAllEpisodes` do the same per row inside one transaction and return the input list with `id` replaced by the new row id, in input order. Proposal: `Transaction::last_insert_rowid()` right after each `execute` (same connection, same transaction, so no race).

## 8. Change notification (replacement for SQLDelight `asFlow`)

OBSERVED. SQLDelight generated insert/update/delete statements call `notifyQueries(...)` for the table they write; `Query.asFlow()` re-runs the query on each notification of any table the query reads (for a view, the tables the view reads). Writes made by triggers and foreign-key cascades are not notified (SQLDelight only sees the statement it generated). Notifications are emitted when the surrounding transaction ends. No `distinctUntilChanged` is applied.

Proposal (`kuro_core::data::db::Notifier`): per database a `tokio::sync::broadcast::Sender<TableSet>` where `TableSet` is a small bitset of table ids. The writer connection installs `update_hook` (collects table names touched, including trigger and cascade writes if SQLite reports them) and `commit_hook` (flushes the collected set) and `rollback_hook` (drops it). Each subscription registers the static set of tables its query reads (listed per repository method in `p1-behavior.md`) and re-executes only when the intersection is non-empty. As a safety net for hook gaps, each repository write method additionally publishes the superset of tables it logically affects (for example `delete manga` publishes `mangas, chapters, mangas_categories, excluded_scanlators, history`). A test (`T-05a-002` acceptance) deletes a manga with chapters and asserts that a chapter-list subscription re-emits. Subscriptions compare the new result with the previous one (`PartialEq`) and drop duplicates, which is an intentional improvement over SQLDelight (fewer GTK model updates).
