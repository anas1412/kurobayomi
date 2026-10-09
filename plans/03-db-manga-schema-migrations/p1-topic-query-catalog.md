# Area 03 / part 1 — Topic: every named statement of the manga database

Format per statement: verbatim SQL, SQLDelight parameter binding (`:name` or positional `?`), result columns, who calls it in Kotlin (repo-relative paths), tables it reads/writes (the "reactive dependency set" that `T-03a-019` encodes), and the Rust constant/function (`kuro_db::sql::<module>::<NAME>`). "Unused" means no Kotlin caller exists in `app/`, `domain/`, `data/` or `source-local/` (verified by grep).

Rust conventions (full design in `p1-rust-design.md`): every statement is a `pub const NAME: &str` (SQL text identical to the `.sq` text except the mechanical fixes of the baseline doc, and with SQLDelight `:name` placeholders kept because rusqlite supports named parameters `:name` natively). Reads return owned row structs. `IN :list` parameters are NOT expanded to `(?,?,?)`; they use `IN (SELECT value FROM json_each(:ids))` with the ids bound as one JSON text (JSON1 is compiled into `libsqlite3-sys` 0.38.2 bundled: `-DSQLITE_ENABLE_JSON1`), which avoids `SQLITE_MAX_VARIABLE_NUMBER`.

Timestamps/units: see `p1-data-and-config.md` section 4 (milliseconds unless stated).

---

## A. `data/src/main/sqldelight/data/mangas.sq` (module `sql::mangas`)

Row struct `MangaRow` has the 24 columns of the table in table order (`_id, source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, chapter_flags, cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, favorite_modified_at, version, is_syncing`).

| Statement | SQL (verbatim) | Params | Result | Kotlin callers | Reads | Writes |
|---|---|---|---|---|---|---|
| `getMangaById` | `SELECT * FROM mangas WHERE _id = :id;` | `id: i64` | 0..1 `MangaRow` | `data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt:getMangaById` (`awaitOne`: throws when absent), `getMangaByIdAsFlow` (`subscribeToOne`: throws when the row disappears) | mangas | - |
| `getMangaByUrlAndSource` | `SELECT * FROM mangas WHERE url = :url AND source = :source LIMIT 1;` (the `.sq` carries a developer comment stating that this should ideally never have more than 1 result) | `url: &str, source: i64` | 0..1 `MangaRow` | `MangaRepositoryImpl.getMangaByUrlAndSourceId` and `getMangaByUrlAndSourceIdAsFlow`; `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/NetworkToLocalManga.kt`; `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt:findExistingManga` | mangas (uses `mangas_url_index`) | - |
| `getFavorites` | `SELECT * FROM mangas WHERE favorite = 1;` | - | `Vec<MangaRow>` | `MangaRepositoryImpl.getMangaFavorites` (via `GetMangaFavorites`, `SetMangaDefaultChapterFlags`, backup creator) | mangas (`library_favorite_index`) | - |
| `getReadMangaNotInLibrary` | `SELECT * FROM mangas WHERE favorite = 0 AND _id IN (SELECT DISTINCT chapters.manga_id FROM chapters WHERE read = 1 OR last_page_read != 0);` | - | `Vec<MangaRow>` | `MangaRepositoryImpl.getReadMangaNotInLibrary` <- `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt:108` (only when option `readEntries`) | mangas, chapters | - |
| `getAllManga` | `SELECT * FROM mangas;` | - | `Vec<MangaRow>` | unused | mangas | - |
| `getAllMangaSourceAndUrl` | `SELECT source, url FROM mangas;` | - | `Vec<(i64, String)>` | `MangaRestorer.sortByNew` | mangas | - |
| `getMangasWithFavoriteTimestamp` | `SELECT * FROM mangas WHERE favorite_modified_at IS NOT NULL;` | - | `Vec<MangaRow>` | unused | mangas | - |
| `getSourceIdWithFavoriteCount` | `SELECT source, count(*) FROM mangas WHERE favorite = 1 GROUP BY source;` | - | `Vec<(i64 source, i64 count)>` | `data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt:getMangaSourcesWithFavoriteCount` (combined with the live source list; `isStub` true when the source is not installed) | mangas | - |
| `getFavoriteBySourceId` | `SELECT * FROM mangas WHERE favorite = 1 AND source = :sourceId;` | `source_id: i64` | `Vec<MangaRow>` | `MangaRepositoryImpl.getMangaFavoritesBySourceId` <- `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/manga/MigrateMangaScreenModel.kt:41` | mangas | - |
| `getDuplicateLibraryManga` | `SELECT * FROM mangas WHERE favorite = 1 AND LOWER(title) = :title AND _id != :id;` | `title: &str` (ALREADY lowercased by the caller: `GetDuplicateLibraryManga` passes `manga.title.lowercase()`), `id: i64` | `Vec<MangaRow>` | `MangaRepositoryImpl.getDuplicateLibraryManga` <- `GetDuplicateLibraryManga`; UI: `BrowseMangaSourceScreenModel`, `MangaScreenModel`, `MangaHistoryScreenModel` (shows message `MR.strings.confirm_add_duplicate_manga`) | mangas | - |
| `getUpcomingManga` | `SELECT * FROM mangas WHERE next_update >= :startOfDay AND favorite = 1 AND status IN :statuses ORDER BY next_update ASC;` | `start_of_day: i64` (ms; caller computes `LocalDate.now().atStartOfDay(ZoneId.systemDefault()).toEpochSecond() * 1000`), `statuses: &[i64]` (`GetUpcomingManga` passes `{SManga.ONGOING = 1, SManga.PUBLISHING_FINISHED = 4}`) | `Vec<MangaRow>` | `MangaRepositoryImpl.getUpcomingManga` <- `domain/src/main/java/mihon/domain/upcoming/manga/interactor/GetUpcomingManga.kt` <- `app/src/main/java/mihon/feature/upcoming/manga/UpcomingMangaScreenModel.kt` | mangas | - |
| `resetViewerFlags` | `UPDATE mangas SET viewer = 0;` | - | rows affected | `MangaRepositoryImpl.resetMangaViewerFlags` <- `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/ResetMangaViewerFlags.kt` <- Settings > Advanced "Reset per-series reader settings" (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:328`) | - | mangas (fires the three `AFTER UPDATE ON mangas` triggers for every row) |
| `resetIsSyncing` | `UPDATE mangas SET is_syncing = 0 WHERE is_syncing = 1;` | - | rows affected | unused | - | mangas |
| `getSourceIdsWithNonLibraryManga` | `SELECT source, COUNT(*) AS manga_count FROM mangas WHERE favorite = 0 GROUP BY source;` | - | `Vec<(i64 source, i64 manga_count)>` | `MangaSourceRepositoryImpl.getMangaSourcesWithNonLibraryManga` <- `ClearDatabaseScreen` | mangas | - |
| `deleteMangasNotInLibraryBySourceIds` | `DELETE FROM mangas WHERE favorite = 0 AND source IN :sourceIds;` | `source_ids: &[i64]` | rows affected | `app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearDatabaseScreen.kt:212` (followed by `history.removeResettedHistory`) | - | mangas; cascades to chapters -> history, mangas_categories, manga_sync, excluded_scanlators |
| `insert` | `INSERT INTO mangas(source, url, artist, author, description, genre, title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer, chapter_flags, cover_last_modified, date_added, update_strategy, calculate_interval, last_modified_at, version) VALUES (:source, :url, :artist, :author, :description, :genre, :title, :status, :thumbnailUrl, :favorite, :lastUpdate, :nextUpdate, :initialized, :viewerFlags, :chapterFlags, :coverLastModified, :dateAdded, :updateStrategy, :calculateInterval, 0, :version);` | 20 bound values (`last_modified_at` literal `0`; `favorite_modified_at` NULL; `is_syncing` default 0) | `last_insert_rowid()` through `selectLastInsertedRowId` | `MangaRepositoryImpl.insertManga` (inside a transaction together with `SELECT last_insert_rowid()`), `MangaRestorer.insertManga` (restore passes `nextUpdate = 0L`, `calculateInterval = 0L`) | - | mangas |
| `update` | `UPDATE mangas SET source = coalesce(:source, source), url = coalesce(:url, url), artist = coalesce(:artist, artist), author = coalesce(:author, author), description = coalesce(:description, description), genre = coalesce(:genre, genre), title = coalesce(:title, title), status = coalesce(:status, status), thumbnail_url = coalesce(:thumbnailUrl, thumbnail_url), favorite = coalesce(:favorite, favorite), last_update = coalesce(:lastUpdate, last_update), next_update = coalesce(:nextUpdate, next_update), initialized = coalesce(:initialized, initialized), viewer = coalesce(:viewer, viewer), chapter_flags = coalesce(:chapterFlags, chapter_flags), cover_last_modified = coalesce(:coverLastModified, cover_last_modified), date_added = coalesce(:dateAdded, date_added), update_strategy = coalesce(:updateStrategy, update_strategy), calculate_interval = coalesce(:calculateInterval, calculate_interval), version = coalesce(:version, version), is_syncing = coalesce(:isSyncing, is_syncing) WHERE _id = :mangaId;` | every column nullable = "leave unchanged"; there is NO way to set a nullable column back to NULL | rows affected (0 when the id does not exist: no error) | `MangaRepositoryImpl.partialUpdateManga` (always passes `isSyncing = 0`; `genre` encoded through `StringListColumnAdapter` first, `updateStrategy` through `MangaUpdateStrategyColumnAdapter`), `MangaRestorer.updateManga` (passes `isSyncing = 1`, `nextUpdate = null`, `calculateInterval = null` = unchanged) | - | mangas |
| `selectLastInsertedRowId` | `SELECT last_insert_rowid();` | - | `i64` | after every insert (`MangaRepositoryImpl.insertManga`, `MangaRestorer.insertManga`) | - | - |

Notes on `update`: because of `coalesce`, `MangaUpdate.description = null` (Kotlin) means "do not touch", not "clear". `UpdateManga.awaitUpdateFromSource` therefore can never blank an existing description/author/artist/genre/thumbnail when the source returns NULL; it can only overwrite with a non-null value (and with an empty string, since `''` is non-null). Proposal: keep these exact semantics; Rust `MangaPatch` uses `Option<T>` for each field.

## B. `data/src/main/sqldelight/data/chapters.sq` (module `sql::chapters`)

Row struct `ChapterRow`: `_id, manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload, last_modified_at, version, is_syncing` (15 columns).

| Statement | SQL (verbatim) | Params | Result | Kotlin callers | Reads | Writes |
|---|---|---|---|---|---|---|
| `getChapterById` | `SELECT * FROM chapters WHERE _id = :id;` | `id` | 0..1 | `data/src/main/java/tachiyomi/data/items/chapter/ChapterRepositoryImpl.kt:getChapterById`; backup creator `MangaBackupCreator.kt:69` (`awaitOne`, assumes the row exists) | chapters | - |
| `getChaptersByMangaId` | `SELECT C.* FROM chapters C LEFT JOIN excluded_scanlators ES ON C.manga_id = ES.manga_id AND C.scanlator = ES.scanlator WHERE C.manga_id = :mangaId AND (:applyScanlatorFilter = 0 OR ES.scanlator IS NULL);` | `manga_id`, `apply_scanlator_filter: 0|1` (Kotlin `Boolean.toLong()`) | `Vec<ChapterRow>` in UNSPECIFIED order (no `ORDER BY`; SQLite returns by `chapters_manga_id_index` then rowid, i.e. ascending `_id` within one manga, but callers sort in memory with `getChapterSort`) | `ChapterRepositoryImpl.getChapterByMangaId` and `getChapterByMangaIdAsFlow`; backup creator passes `applyScanlatorFilter = 0` (backs up ALL chapters) | chapters, excluded_scanlators | - |
| `getScanlatorsByMangaId` | `SELECT scanlator FROM chapters WHERE manga_id = :mangaId;` | `manga_id` | `Vec<Option<String>>` mapped to `Vec<String>` with NULL -> `""` (NOT distinct, includes blanks) | `ChapterRepositoryImpl.getScanlatorsByMangaId` and `getScanlatorsByMangaIdAsFlow` <- `app/src/main/java/eu/kanade/domain/items/chapter/interactor/GetAvailableScanlators.kt` which drops blanks and builds a `Set` | chapters | - |
| `getBookmarkedChaptersByMangaId` | `SELECT * FROM chapters WHERE bookmark AND manga_id = :mangaId;` | `manga_id` | `Vec<ChapterRow>` | declared in `ChapterRepository` and implemented, but no caller | chapters | - |
| `getChapterByUrl` | `SELECT * FROM chapters WHERE url = :chapterUrl;` | `url` | 0..n (the restorer uses `awaitOneOrNull` = first row) | `MangaRestorer.restoreHistory` (line 327) | chapters (full table scan, see baseline doc section 5) | - |
| `getChapterByUrlAndMangaId` | `SELECT * FROM chapters WHERE url = :chapterUrl AND manga_id = :mangaId;` | `url`, `manga_id` | 0..1 | `ChapterRepositoryImpl.getChapterByUrlAndMangaId` <- `GetChapterByUrlAndMangaId`, `GetChapter`, `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreenModel.kt` | chapters | - |
| `removeChaptersWithIds` | `DELETE FROM chapters WHERE _id IN :chapterIds;` | `ids: &[i64]` | rows affected | `ChapterRepositoryImpl.removeChaptersWithIds` (errors are logged, swallowed) <- `SyncChaptersWithSource` | - | chapters; cascades to history |
| `resetIsSyncing` | `UPDATE chapters SET is_syncing = 0 WHERE is_syncing = 1;` | - | rows | unused | - | chapters |
| `insert` | `INSERT INTO chapters(manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order,date_fetch, date_upload, last_modified_at, version, is_syncing) VALUES (:mangaId, :url, :name, :scanlator, :read, :bookmark, :lastPageRead, :chapterNumber, :sourceOrder, :dateFetch, :dateUpload, 0, :version, 0);` | 12 bound values; `last_modified_at` literal `0`; `is_syncing` literal `0` | rowid via `selectLastInsertedRowId` | `ChapterRepositoryImpl.addAllChapters` (one transaction for the whole list; on any exception the transaction rolls back, the error is logged and an EMPTY list is returned), `MangaRestorer.insertNewChapters` | - | chapters |
| `update` | `UPDATE chapters SET manga_id = coalesce(:mangaId, manga_id), url = coalesce(:url, url), name = coalesce(:name, name), scanlator = coalesce(:scanlator, scanlator), read = coalesce(:read, read), bookmark = coalesce(:bookmark, bookmark), last_page_read = coalesce(:lastPageRead, last_page_read), chapter_number = coalesce(:chapterNumber, chapter_number), source_order = coalesce(:sourceOrder, source_order), date_fetch = coalesce(:dateFetch, date_fetch), date_upload = coalesce(:dateUpload, date_upload), version = coalesce(:version, version), is_syncing = coalesce(:isSyncing, is_syncing) WHERE _id = :chapterId;` | same coalesce rule (cannot set `scanlator` back to NULL) | rows | `ChapterRepositoryImpl.partialUpdate` (always `isSyncing = 0`, one transaction for the batch), `MangaRestorer.updateExistingChapters` | - | chapters (fires `update_last_modified_at_chapters`; fires `update_chapter_and_manga_version` when read/bookmark/last_page_read changed) |
| `selectLastInsertedRowId` | `SELECT last_insert_rowid();` | - | i64 | `addAllChapters` after each insert | - | - |

## C. `data/src/main/sqldelight/data/categories.sq` (module `sql::categories`)

Row struct `CategoryRow { id: i64 (_id), name: String, order: i64 (sort), flags: i64, hidden: bool }`. The query aliases are `_id AS id`, ``sort AS `order` ``, so column names in results are `id, name, order, flags, hidden`.

| Statement | SQL (verbatim) | Params | Result | Kotlin callers | Reads | Writes |
|---|---|---|---|---|---|---|
| `getCategory` | `SELECT * FROM categories WHERE _id = :id LIMIT 1;` | `id` | 0..1 row with the RAW table column names `_id, name, sort, flags, hidden` (mapped by position) | `data/src/main/java/tachiyomi/data/category/manga/MangaCategoryRepositoryImpl.kt:getMangaCategory`; storage screens `app/src/main/java/eu/kanade/tachiyomi/ui/storage/CommonStorageScreenModel.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/storage/manga/MangaStorageScreenModel.kt` | categories | - |
| `getCategories` | ``SELECT _id AS id, name, sort AS `order`, flags, hidden FROM categories ORDER BY sort;`` | - | all categories INCLUDING the system category (id 0, sort -1, name `''`), ordered by `sort` only (ties unordered) | `getAllMangaCategories`, `getAllMangaCategoriesAsFlow` | categories | - |
| `getVisibleCategories` | ``SELECT _id AS id, name, sort AS `order`, flags, hidden FROM categories WHERE hidden = 0 ORDER BY sort;`` | - | visible categories incl. system category | `getAllVisibleMangaCategories`, `getAllVisibleMangaCategoriesAsFlow` | categories | - |
| `getCategoriesByMangaId` | ``SELECT C._id AS id, C.name, C.sort AS `order`, C.flags, C.hidden FROM categories C JOIN mangas_categories MC ON C._id = MC.category_id WHERE MC.manga_id = :mangaId;`` | `manga_id` | categories assigned to the manga (no ORDER BY; never contains the system category unless a `mangas_categories` row points at id 0) | `getCategoriesByMangaId`, `getCategoriesByMangaIdAsFlow` | categories, mangas_categories | - |
| `getVisibleCategoriesByMangaId` | ``SELECT C._id AS id, C.name, C.sort AS `order`, C.flags, C.hidden FROM categories C JOIN mangas_categories MC ON C._id = MC.category_id WHERE MC.manga_id = :mangaId AND C.hidden = 0;`` | `manga_id` | visible assigned categories | `getVisibleCategoriesByMangaId`, `getVisibleCategoriesByMangaIdAsFlow` | categories, mangas_categories | - |
| `insert` | `INSERT INTO categories(name, sort, flags) VALUES (:name, :order, :flags);` | `name, order, flags` (`hidden` defaults 0; `_id` auto-assigned = max(rowid)+1 because there is no AUTOINCREMENT) | - | `insertMangaCategory` <- `CreateMangaCategoryWithName`; backup restore `MangaCategoriesRestorer.kt:29` (then `selectLastInsertedRowId`) | - | categories |
| `delete` | `DELETE FROM categories WHERE _id = :categoryId;` | `category_id` | - | `deleteMangaCategory` <- `DeleteMangaCategory`. Id <= 0 aborts with `System category can't be deleted` (trigger). Cascades remove all `mangas_categories` rows of that category | - | categories, mangas_categories |
| `update` | `UPDATE categories SET name = coalesce(:name, name), sort = coalesce(:order, sort), flags = coalesce(:flags, flags), hidden = coalesce(:hidden, hidden) WHERE _id = :categoryId;` | nullable = unchanged; `hidden` passed as `1L`/`0L` by the repo | - | `updatePartialMangaCategory`, `updatePartialMangaCategories` (one transaction) | - | categories |
| `updateAllFlags` | `UPDATE categories SET flags = coalesce(?, flags);` | one positional `flags: Option<i64>` (NO `WHERE`: includes the system category) | - | `updateAllMangaCategoryFlags` <- `ResetMangaCategoryFlags`, `SetSortModeForMangaCategory` (global branch) | - | categories |
| `selectLastInsertedRowId` | `SELECT last_insert_rowid();` | - | i64 | `MangaCategoriesRestorer` | - | - |

## D. `data/src/main/sqldelight/data/mangas_categories.sq` (module `sql::mangas_categories`)

| Statement | SQL | Callers | Notes |
|---|---|---|---|
| `insert` | `INSERT INTO mangas_categories(manga_id, category_id) VALUES (:mangaId, :categoryId);` | `MangaRepositoryImpl.setMangaCategories`, `MangaRestorer.restoreCategories` | each insert fires `insert_manga_category_update_version`, adding +1 to `mangas.version` per row (when the manga is not `is_syncing`); no UNIQUE constraint: duplicate pairs are possible and produce duplicate rows in `libraryView` |
| `deleteMangaCategoryByMangaId` | `DELETE FROM mangas_categories WHERE manga_id = :mangaId;` | same two callers | `setMangaCategories` = delete-all then insert-each inside one transaction (`handler.await(inTransaction = true)`) |

## E. `data/src/main/sqldelight/data/history.sq` (module `sql::history`)

Row struct `HistoryRow { id, chapter_id, last_read: Option<i64>, time_read: i64 }` (nullable because the `.sq` declared the column nullable; the baseline makes it NOT NULL so the Rust type may be `i64`; keep `Option` only in the importer).

| Statement | SQL (verbatim) | Params | Callers | Reads | Writes |
|---|---|---|---|---|---|
| `getHistoryByMangaId` | `SELECT H._id, H.chapter_id, H.last_read, H.time_read FROM history H JOIN chapters C ON H.chapter_id = C._id WHERE C.manga_id = :mangaId AND C._id = H.chapter_id;` | `manga_id` | `data/src/main/java/tachiyomi/data/history/manga/MangaHistoryRepositoryImpl.kt:getHistoryByMangaId` <- `GetMangaHistory.await`, backup creator | history, chapters | - |
| `getHistoryByChapterUrl` | `SELECT H._id, H.chapter_id, H.last_read, H.time_read FROM history H JOIN chapters C ON H.chapter_id = C._id WHERE C.url = :chapterUrl AND C._id = H.chapter_id;` | `url` | `MangaRestorer.restoreHistory` (`awaitOneOrNull`) | history, chapters | - |
| `resetHistoryById` | `UPDATE history SET last_read = 0 WHERE _id = :historyId;` | `history_id` | `resetMangaHistory` <- `RemoveMangaHistory.await(history)` ("remove" in the history list = reset, row kept, `time_read` kept) | - | history |
| `resetHistoryByMangaId` | `UPDATE history SET last_read = 0 WHERE _id IN (SELECT H._id FROM mangas M INNER JOIN chapters C ON M._id = C.manga_id INNER JOIN history H ON C._id = H.chapter_id WHERE M._id = :mangaId);` | `manga_id` | `resetHistoryByMangaId` <- `RemoveMangaHistory.await(mangaId)` | - | history |
| `removeAllHistory` | `DELETE FROM history;` | - | `deleteAllMangaHistory` <- `RemoveMangaHistory.awaitAll` (history "Clear history" action; returns true/false; DESTROYS `time_read`, which feeds the reading-time statistic) | - | history |
| `removeResettedHistory` | `DELETE FROM history WHERE last_read = 0;` | - | `ClearDatabaseScreen.kt:213` after deleting non-library manga | - | history |
| `upsert` | `INSERT INTO history(chapter_id, last_read, time_read) VALUES (:chapterId, :readAt, :time_read) ON CONFLICT(chapter_id) DO UPDATE SET last_read = :readAt, time_read = time_read + :time_read WHERE chapter_id = :chapterId;` | `chapter_id`, `read_at: i64 ms`, `time_read: i64 ms` | `upsertMangaHistory` <- `UpsertMangaHistory` <- `ReaderViewModel.kt:601` (`MangaHistoryUpdate(chapterId, readAt, sessionReadDuration)`), `MangaRestorer.restoreHistory` (passes `readDuration = max(item, db) - db.time_read`, i.e. the DELTA) | - | history |
| `getReadDuration` | `SELECT coalesce(sum(time_read), 0) FROM history;` | - | `getTotalReadDuration` <- `GetTotalReadDuration`, `app/src/main/java/eu/kanade/tachiyomi/ui/stats/manga/MangaStatsScreenModel.kt` | history | - |

Important: `upsert`'s second `:time_read` binding is the same named parameter used twice (SQLDelight binds positional index 3 and 4 separately; rusqlite named parameters bind both occurrences from one value).

## F. `data/src/main/sqldelight/data/manga_sync.sq` (module `sql::manga_sync`) — manga tracker bindings

Row struct `TrackRow { id, manga_id, sync_id (tracker id), remote_id, library_id: Option<i64>, title, last_chapter_read: f64, total_chapters: i64, status: i64, score: f64, remote_url, start_date: i64, finish_date: i64, private: bool }`.

| Statement | SQL | Params | Callers |
|---|---|---|---|
| `delete` | `DELETE FROM manga_sync WHERE manga_id = :mangaId AND sync_id = :syncId;` | `manga_id, sync_id` | `data/src/main/java/tachiyomi/data/track/manga/MangaTrackRepositoryImpl.kt:delete(mangaId, trackerId)` <- `DeleteMangaTrack` |
| `getTracks` | `SELECT * FROM manga_sync;` | - | `getMangaTracksAsFlow` <- `GetTracksPerManga` (library sort by tracker mean, tracker filters) |
| `getTrackById` | `SELECT * FROM manga_sync WHERE _id = :id;` | `id` | `getTrackByMangaId(id)` — NAME BUG: the repository method is called `getTrackByMangaId` but executes `getTrackById`, i.e. it looks up by the TRACK row id; it is reached through `GetMangaTracks.awaitOne(id)` |
| `getTracksByMangaId` | `SELECT * FROM manga_sync WHERE manga_id = :mangaId;` | `manga_id` | `getTracksByMangaId`, `getTracksByMangaIdAsFlow`, backup creator, `MangaRestorer.restoreTracking` |
| `insert` | `INSERT INTO manga_sync(manga_id,sync_id,remote_id,library_id,title,last_chapter_read,total_chapters,status,score,remote_url,start_date,finish_date,private) VALUES (:mangaId,:syncId,:remoteId,:libraryId,:title,:lastChapterRead,:totalChapters,:status,:score,:remoteUrl,:startDate,:finishDate,:private);` | 13 values | `insertValues` (one transaction for the list) <- `InsertMangaTrack`. Because of `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`, inserting a second track for the same (manga, tracker) DELETES the old row and inserts a new one with a NEW `_id` |
| `update` | `UPDATE manga_sync SET manga_id = coalesce(:mangaId, manga_id), sync_id = coalesce(:syncId, sync_id), remote_id = coalesce(:mediaId, remote_id), library_id = coalesce(:libraryId, library_id), title = coalesce(:title, title), last_chapter_read = coalesce(:lastChapterRead, last_chapter_read), total_chapters = coalesce(:totalChapter, total_chapters), status = coalesce(:status, status), score = coalesce(:score, score), remote_url = coalesce(:trackingUrl, remote_url), start_date = coalesce(:startDate, start_date), finish_date = coalesce(:finishDate, finish_date), private = coalesce(:private, private) WHERE _id = :id;` | the restorer calls it positionally (`MangaRestorer.kt:394`) | `MangaRestorer.restoreTracking` only (the regular tracking flow re-inserts and relies on REPLACE) |

## G. `data/src/main/sqldelight/data/excluded_scanlators.sq` (module `sql::excluded_scanlators`)

| Statement | SQL | Callers |
|---|---|---|
| `insert` | `INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (:mangaId, :scanlator);` | `app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetExcludedScanlators.kt`, `MangaRestorer.restoreExcludedScanlators` |
| `remove` | `DELETE FROM excluded_scanlators WHERE manga_id = :mangaId AND scanlator IN :scanlators;` | `SetExcludedScanlators` (with an empty set SQLDelight emits `IN ()` which SQLite accepts and matches nothing) |
| `getExcludedScanlatorsByMangaId` | `SELECT scanlator FROM excluded_scanlators WHERE manga_id = :mangaId;` | `GetExcludedScanlators` (await + subscribe), `SetExcludedScanlators`, backup creator `MangaBackupCreator.kt:34`, `MangaRestorer.restoreExcludedScanlators`, `SyncChaptersWithSource` (to drop notifications for excluded scanlators) |

No unique constraint: duplicates are possible; `SetExcludedScanlators` and the restorer avoid inserting existing values by reading first.

## H. `data/src/main/sqldelight/data/extension_repos.sq` (module `sql::extension_repos`)

| Statement | SQL (verbatim; note the two typos) | Callers |
|---|---|---|
| `findOne` | `SELECT * FROM extension_repos WHERE base_url = :base_url;` | `mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt:getRepo` |
| `findOneBySigningKeyFingerprint` | `SELECT * FROM extension_repos WHERE signing_key_fingerprint = :fingerprint;` | `getRepoBySigningKeyFingerprint` |
| `findAll` | `SELECT * FROM extension_repos;` | `subscribeAll`, `getAll` (no ORDER BY: table order = `base_url` autoindex order is NOT guaranteed; SQLite scans the rowid b-tree in insertion order) |
| `count` | `SELECT COUNT(*) FROM extension_repos;` | `getCount` (`subscribeToOne`, `.map { it.toInt() }`) |
| `insert` | `INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) VALUES (:base_url, :name, :short_name, :website, :fingerprint);` | `insertRepo` (SQLite constraint failure -> `SaveExtensionRepoException`), `MangaExtensionRepoRestorer` |
| `upsert` | `INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) VALUES (:base_url, :name, :short_name, :website, :fingerprint) ON CONFLICT(base_url) DO UPDATE SET name = :name, short_name = :short_name, website =: website, signing_key_fingerprint = :fingerprint WHERE base_url = base_url;` | `upsertRepo` (constraint failure, e.g. the new fingerprint already belongs to ANOTHER base_url -> `SaveExtensionRepoException`) |
| `replace` | `INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) VALUES (:base_url, :name, :short_name, :website, :fingerprint) ON CONFLICT(signing_key_fingerprint) DO UPDATE SET base_url = :base_url, name = :name, short_name = :short_name, website =: website WHERE signing_key_fingerprint = signing_key_fingerprint;` | `replaceRepo` (NOT wrapped: a PK conflict on `base_url` propagates the raw SQLite exception) |
| `delete` | `DELETE FROM extension_repos WHERE base_url = :base_url;` | `deleteRepo` |

SQLDelight accepts `website =: website` (a colon, a space, then the name); plain SQLite rejects it (verified: `unrecognized token: ":"`). The Rust constants MUST be written `website = :website`. The `WHERE base_url = base_url` / `WHERE signing_key_fingerprint = signing_key_fingerprint` clauses compare a column with itself and are always true for non-NULL columns; they are kept verbatim.

## I. `data/src/main/sqldelight/data/sources.sq` (module `sql::sources`) — stub sources (names of sources whose extension is gone)

| Statement | SQL | Callers |
|---|---|---|
| `findAll` | `SELECT * FROM sources;` | `data/src/main/java/tachiyomi/data/source/manga/MangaStubSourceRepositoryImpl.kt:subscribeAllManga` <- `app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt:75` |
| `findOne` | `SELECT * FROM sources WHERE _id = :id;` | `getStubMangaSource` (`AndroidMangaSourceManager.registerStubSource`, `createStubSource`) |
| `upsert` | `INSERT INTO sources(_id, lang, name) VALUES (:id, :lang, :name) ON CONFLICT(_id) DO UPDATE SET lang = :lang, name = :name WHERE _id = :id;` | `upsertStubMangaSource` |

## J. Views and their queries

### `data/src/main/sqldelight/view/libraryView.sq`

`library:` `SELECT * FROM libraryView;` — result = the 24 `mangas` columns (`M.*`) followed by `totalCount, readCount, latestUpload, chapterFetchedAt, lastRead, bookmarkCount, category` (31 columns; Kotlin mapper `MangaMapper.mapLibraryManga` takes `readCount: Double`, `bookmarkCount: Double` because `sum()` is typed REAL by SQLDelight; the values are integral). Callers: `MangaRepositoryImpl.getLibraryManga` and `getLibraryMangaAsFlow`. Reads: mangas, chapters, excluded_scanlators, history, mangas_categories.

### `data/src/main/sqldelight/view/historyView.sq`

`history:` `SELECT id, mangaId, chapterId, title, thumbnailUrl, source, favorite, cover_last_modified, chapterNumber, readAt, readDuration FROM historyView WHERE historyView.readAt > 0 AND maxReadAtChapterId = historyView.chapterId AND lower(historyView.title) LIKE ('%' || :query || '%') ORDER BY readAt DESC;` — param `query: &str` passed RAW (`MangaHistoryScreenModel` passes `query ?: ""`, no lowercasing, no escaping of `%` and `_`). Caller: `MangaHistoryRepositoryImpl.getMangaHistory` (`subscribeToList`), UI applies `distinctUntilChanged`.

`getLatestHistory:` same projection and `WHERE historyView.readAt > 0 ORDER BY readAt DESC LIMIT 1;` — caller `getLastMangaHistory` <- `GetNextChapters` ("resume reading" shortcut).

Reads: mangas, chapters, history.

### `data/src/main/sqldelight/view/updatesView.sq`

`getRecentUpdates:` `SELECT * FROM updatesView WHERE dateUpload > :after LIMIT :limit;` `getUpdatesByReadStatus:` `SELECT * FROM updatesView WHERE read = :read AND dateUpload > :after LIMIT :limit;` (the `.sq` has a blank line before `LIMIT`; irrelevant). Params: `after: i64 ms` (the Updates screen passes start-of-window; `GetMangaUpdates` always passes `limit = 500`), `read: bool`, `limit: i64`. Result columns (14): `mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, last_page_read, source, favorite, thumbnailUrl, coverLastModified, dateUpload, datefetch`. Order: the view's own `ORDER BY date_fetch DESC` is preserved by SQLite when the outer query has a LIMIT (verified: rows came back 500, 300, 200, 150 for date_fetch values). Callers: `data/src/main/java/tachiyomi/data/updates/manga/MangaUpdatesRepositoryImpl.kt` (`awaitWithRead`, `subscribeAllMangaUpdates`, `subscribeWithRead`). Reads: mangas, chapters.

Important filter pair: the view keeps only chapters with `date_fetch > date_added` (fetched after the manga was added to the library) and the outer query filters on `dateUpload > :after` (NOT on date_fetch).

## K. Statement count

mangas.sq 18 + chapters.sq 11 + categories.sq 10 + mangas_categories.sq 2 + history.sq 8 + manga_sync.sq 6 + excluded_scanlators.sq 3 + extension_repos.sq 8 + sources.sq 3 + libraryView.sq 1 + historyView.sq 2 + updatesView.sq 2 = 74 named statements. Five have no Kotlin caller: `mangas.getAllManga`, `mangas.getMangasWithFavoriteTimestamp`, `mangas.resetIsSyncing`, `chapters.resetIsSyncing`, `chapters.getBookmarkedChaptersByMangaId`. Proposal: port all 74 (the unused ones are one-line constants and `resetIsSyncing` is used by the startup hook of `T-03a-030`).
