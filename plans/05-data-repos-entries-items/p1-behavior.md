# Area 05 — behavior specification (part 1 of 1)

Paths repo-relative to `/home/blackbox/aniyomi-src`. Scope files: the 12 listed in `p1-inventory.md`. "OBSERVED" is what the Kotlin/SQL does. "Proposal:" is what Rust should do. Deep dives: `p1-topic-sql-semantics-and-triggers.md`, `p1-topic-library-and-season-views.md`, `p1-topic-seasons-and-relations.md`, `p1-topic-categories.md`, `p1-topic-item-sanitizer.md`.

## 0. Common machinery (what every repository method goes through)

OBSERVED. Each repository holds a `MangaDatabaseHandler` or `AnimeDatabaseHandler` (`data/src/main/java/tachiyomi/data/handlers/{manga,anime}/`). Its operations:

| Handler call | Meaning | Rust equivalent |
|---|---|---|
| `await(inTransaction = false) { block }` | run a write/side-effect block on `Dispatchers.IO` | `writer.call(\|conn\| ...)` |
| `await(inTransaction = true) { block }` | run inside `db.transactionWithResult`; nested calls reuse the transaction through a coroutine context element; the transaction occupies one thread until the block returns | `writer.call(\|conn\| { let tx = conn.transaction()?; ...; tx.commit() })` |
| `awaitList { query }` / `awaitOne` / `awaitOneOrNull` | execute and map rows; `awaitOne` throws when there is no row (SQLDelight `executeAsOne`) | `reader.call(...)` returning `Vec<T>` / `T` (`Err(NotFound)`) / `Option<T>` |
| `awaitOneOrNullExecutable(inTransaction = true) { insert...; selectLastInsertedRowId() }` | insert then fetch id in one transaction | one writer closure, `tx.last_insert_rowid()` |
| `subscribeToList / subscribeToOne / subscribeToOneOrNull { query }` | `Query.asFlow()` + `mapToList/One/OneOrNull(queryDispatcher)`: initial emission, then re-run on each notification of a table the query reads | `Subscription<T>` (section 11) |
| `subscribeToPagingSource` | Paging 3 `QueryPagingSource` | Dropped: not used by any in-scope repository (T-05a-027) |

Dispatchers: all queries on `Dispatchers.IO`; transaction blocks on a thread leased from the same executor. No UI thread access. Error policy differs by method group and is part of the contract:

| Group | On exception |
|---|---|
| `updateManga`, `updateAllManga`, `updateAnime`, `updateAllAnime`, `resetMangaViewerFlags`, `resetAnimeViewerFlags` | catch `Exception`, `logcat(LogPriority.ERROR, e)`, return `false` (success = `true`) |
| `addAllChapters`, `addAllEpisodes` | catch, log, return `emptyList()`; the whole transaction is rolled back |
| `removeChaptersWithIds`, `removeEpisodesWithIds`, `removeParentIdByIds` | catch, log, return Unit |
| `updateChapter`, `updateAllChapters`, `updateEpisode`, `updateAllEpisodes` | NOT caught; exception propagates to the interactor |
| `setMangaCategories`, `setAnimeCategories`, `insertManga`, `insertAnime`, all category repository methods, all reads, relation methods | NOT caught; propagates |

Proposal: Rust methods return `Result<_, DataError>` always. A thin `legacy` adapter in `kuro_core::data::compat` (or the interactor layer, area:10-app-domain-layer) maps `Err` to the Kotlin shape (`false` / empty list / Unit) for exactly the groups in the first three rows, logging with `tracing::error!`. Rationale: the UI contracts of area:10-app-domain-layer depend on those return shapes; hiding errors inside the repository would make failures undiagnosable in Rust tests.

## 1. MangaRepositoryImpl (`data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt`)

All method names below are the Kotlin ones; Rust names are in `p1-rust-design.md` section 3.

### 1.1 Reads

| Method | Query / algorithm | Missing row | Tables read (for subscriptions) |
|---|---|---|---|
| `getMangaById(id): Manga` | `getMangaById` via `awaitOne` | throws (no `Manga?`) | `mangas` |
| `getMangaByIdAsFlow(id): Flow<Manga>` | `subscribeToOne` | the flow errors when the row disappears (SQLDelight `mapToOne` throws on empty) | `mangas` |
| `getMangaByUrlAndSourceId(url, sourceId): Manga?` | `getMangaByUrlAndSource ... LIMIT 1` | null | `mangas` |
| `getMangaByUrlAndSourceIdAsFlow(url, sourceId): Flow<Manga?>` | `subscribeToOneOrNull` | emits null | `mangas` |
| `getMangaFavorites(): List<Manga>` | `WHERE favorite = 1`, unsorted | empty list | `mangas` |
| `getReadMangaNotInLibrary(): List<Manga>` | non-favorites having a chapter with `read = 1 OR last_page_read != 0` (used by `BackupCreator.kt:107-108` when the backup option `readEntries` is on, area:21-backup-format-and-create) | empty list | `mangas, chapters` |
| `getLibraryManga(): List<LibraryManga>` | `SELECT * FROM libraryView`; mapped with `MangaMapper.mapLibraryManga`; one row per (manga, category) | empty | `mangas, chapters, mangas_categories, excluded_scanlators, history` |
| `getLibraryMangaAsFlow()` | subscription of the same | | same |
| `getMangaFavoritesBySourceId(sourceId): Flow<List<Manga>>` | `WHERE favorite = 1 AND source = :sourceId`, subscription only (no suspend variant) | | `mangas` |
| `getDuplicateLibraryManga(id, title)` | `WHERE favorite = 1 AND LOWER(title) = :title AND _id != :id`. The interactor passes `manga.title.lowercase()` (domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetDuplicateLibraryManga.kt: `getDuplicateLibraryManga(manga.id, manga.title.lowercase())`). SQLite `LOWER()` lower-cases ASCII only; Kotlin `lowercase()` lower-cases all Unicode (locale-invariant). Non-ASCII titles with capitals therefore never match: `"Éclair"` lower-cases to `"éclair"` in Kotlin but the column value is lowered by SQLite to `"Éclair"`. | empty list | `mangas` |
| `getUpcomingManga(statuses: Set<Long>): Flow<List<Manga>>` | `epochMillis = LocalDate.now().atStartOfDay(ZoneId.systemDefault()).toEpochSecond() * 1000` computed once when the method is called (suspend function returning a flow: the cut-off does NOT move at midnight while the flow is collected); `WHERE next_update >= :startOfDay AND favorite = 1 AND status IN :statuses ORDER BY next_update ASC`; statuses passed by `GetUpcomingManga` = {ONGOING (1), PUBLISHING_FINISHED (4)} (anime adds UPCOMING 7) | empty | `mangas` |

Proposal for the two flagged defects: (1) duplicates: register a scalar SQL function `lower_unicode(x)` on every connection (`rusqlite::functions`, implemented with `str::to_lowercase`) and query `LOWER_UNICODE(title) = ?` with the title lower-cased in Rust, so both sides use the same Unicode rule (T-05a-019). (2) Upcoming: recompute the cut-off in the subscription closure on every re-execution and also trigger a re-run at local midnight (a `glib::timeout_add_seconds_local` owned by the screen, or an internal timer that publishes a synthetic notification) (T-05a-020).

### 1.2 Writes

`resetMangaViewerFlags(): Boolean`: `UPDATE mangas SET viewer = 0` (every row, library or not). Only caller: the Advanced settings action `MR.strings.pref_reset_viewer_flags` ("Reset per-series reader settings", summary "Resets reading mode and orientation of all series") in app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:325-340, which toasts `MR.strings.pref_reset_viewer_flags_success` ("All reader settings reset") on `true` and `MR.strings.pref_reset_viewer_flags_error` ("Couldn't reset reader settings") on `false` (area:47-settings-screens-general). `resetAnimeViewerFlags` is registered in DomainModule.kt:247 but has no caller in the Kotlin tree; because anime `viewer` also packs skip-intro and airing-episode data (`p1-data-and-config.md` section 8), Proposal: do not expose it in the Rust UI (T-05a-012 implements the repository method only). Notifies `mangas`.

`setMangaCategories(mangaId, categoryIds)`: in one transaction `DELETE FROM mangas_categories WHERE manga_id = :id`, then `INSERT INTO mangas_categories(manga_id, category_id)` for each id in list order (duplicates in the list insert duplicate link rows; ids that do not exist in `categories` raise a foreign-key error and roll back the whole operation). Each inserted link bumps `mangas.version` by 1 via trigger. Empty list = remove all links = entry is in the Default category. Notifies `mangas_categories` (and `mangas` for the version bump if the notifier reports trigger writes). Note: it never inserts a link to category 0; callers strip the system id before calling.

`insertManga(manga): Long?`: one transaction: `INSERT INTO mangas(...)` with these values: `source, url, artist, author, description, genre (StringListColumnAdapter), title, status, thumbnail_url, favorite, last_update, next_update, initialized, viewer = viewerFlags, chapter_flags, cover_last_modified, date_added, update_strategy (ordinal), calculate_interval = fetchInterval, last_modified_at = 0, version`; then `SELECT last_insert_rowid()`. `manga.id` is ignored (the table autogenerates it: `_id INTEGER PRIMARY KEY`, new id = max(_id)+1, or 1 for an empty table). `lastModifiedAt` and `favoriteModifiedAt` of the argument are ignored. No uniqueness check on `(source, url)`; the caller (`NetworkToLocalManga`) checks first with `getMangaByUrlAndSourceId`. Returns the id, or null when the id query returned no row (cannot happen).

`updateManga(update): Boolean` and `updateAllManga(updates): Boolean`: both call `partialUpdateManga(vararg)`: ONE transaction, for each update in list order execute the `update` statement (section 5.1 of `p1-data-and-config.md`) with bindings: `source, url, artist, author, description, genre = genre?.let(StringListColumnAdapter::encode), title, status, thumbnailUrl, favorite, lastUpdate, nextUpdate, calculateInterval = fetchInterval?.toLong(), initialized, viewer = viewerFlags, chapterFlags, coverLastModified, dateAdded, mangaId = id, updateStrategy = updateStrategy?.let(encode), version, isSyncing = 0`. Rows that do not exist are silently not updated (0 rows affected, still `true`). An empty list opens and commits an empty transaction and returns true. Triggers (`p1-topic-sql-semantics-and-triggers.md` section 3) run for each row.

## 2. MangaMapper (`data/src/main/java/tachiyomi/data/entries/manga/MangaMapper.kt`)

`mapManga` (24 inputs): `lastUpdate = lastUpdate ?: 0`, `nextUpdate = nextUpdate ?: 0`, `fetchInterval = calculateInterval.toInt()`, `viewerFlags = viewerFlags`, `chapterFlags = chapterFlags`, `favorite` as Boolean, `genre` decoded list or null, `updateStrategy` decoded enum, `lastModifiedAt`, `favoriteModifiedAt` (nullable), `version`; `isSyncing` is dropped. `mapLibraryManga` (24 + 7): builds `manga = mapManga(first 24)` and `LibraryManga(manga, category, totalChapters = totalCount, readCount = readCount.toLong(), bookmarkCount = bookmarkCount.toLong(), latestUpload, chapterFetchedAt, lastRead)`. Null handling: nullable columns map to Kotlin nullable fields except `last_update`/`next_update` (NULL becomes 0).

## 3. ChapterRepositoryImpl (`data/src/main/java/tachiyomi/data/items/chapter/ChapterRepositoryImpl.kt`)

| Method | Behaviour |
|---|---|
| `addAllChapters(chapters): List<Chapter>` | One transaction; for each chapter in order `INSERT INTO chapters(manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload, last_modified_at = 0, version, is_syncing = 0)` then `last_insert_rowid()`; returns each input with its `id` replaced (same order). Any exception: rollback everything, log, return empty list. No duplicate check on `(manga_id, url)`. `id`, `lastModifiedAt` of inputs ignored. Notifies `chapters` (plus `mangas` only if triggers are reported: inserts do not fire the update triggers, so no version change). |
| `updateChapter(update)` / `updateAllChapters(updates)` | `partialUpdate(vararg)`: one transaction, per update the `update` statement with `chapterId = id`, nullable fields as coalesce, `isSyncing = 0`. Not caught. Triggers: a change to `read`, `bookmark` or `last_page_read` bumps the chapter and manga versions; every update refreshes `last_modified_at`. |
| `removeChaptersWithIds(ids)` | `DELETE FROM chapters WHERE _id IN :ids`; cascades `history` rows (`history.chapter_id` FK in area:06-data-repos-history-track-source-misc); errors logged and swallowed. Rust: chunked, single transaction. |
| `getChapterByMangaId(mangaId, applyScanlatorFilter = false)` | `getChaptersByMangaId` with `:applyScanlatorFilter = if (b) 1 else 0` (`Boolean.toLong()` from `tachiyomi.core.common.util.lang`). With the flag 1, chapters whose `(manga_id, scanlator)` pair is in `excluded_scanlators` are removed (`ES.scanlator IS NULL` after the LEFT JOIN); with 0 the join is irrelevant but still performed, and duplicate `excluded_scanlators` rows multiply the output rows. |
| `getChapterByMangaIdAsFlow(mangaId, applyScanlatorFilter)` | same query; suspend function returning a `Flow` (the function itself does no I/O). Tables `chapters, excluded_scanlators`. |
| `getScanlatorsByMangaId(mangaId): List<String>` | `SELECT scanlator FROM chapters WHERE manga_id = :id` — one entry per chapter (NOT distinct), NULL mapped to `""` (`{ it.orEmpty() }`). The caller `GetAvailableScanlators` (app/src/main/java/eu/kanade/domain/items/chapter/interactor/GetAvailableScanlators.kt) keeps non-blank values and makes a set (`mapNotNull { it.ifBlank { null } }.toSet()`). |
| `getScanlatorsByMangaIdAsFlow(mangaId)` | same as subscription (table `chapters`). |
| `getBookmarkedChaptersByMangaId(mangaId)` | `WHERE bookmark AND manga_id = :id` (bare `bookmark` is truthy for non-zero). |
| `getChapterById(id): Chapter?` | null if missing. |
| `getChapterByUrlAndMangaId(url, mangaId): Chapter?` | exact `url` match scoped to the manga; the SQL has no `LIMIT 1`, so if duplicate `(manga_id, url)` rows exist SQLDelight `executeAsOneOrNull` throws `IllegalStateException` (more than one row). Proposal: Rust returns the row with the smallest `_id` and logs `tracing::warn!` (T-05a-008 acceptance). |

`mapChapter` (15 inputs): drops `isSyncing`; all other columns map one-to-one including `lastModifiedAt` (seconds) and `version`.

## 4. ChapterSanitizer / EpisodeSanitizer

See `p1-topic-item-sanitizer.md`. Pure functions, no I/O, no dispatcher.

## 5. AnimeRepositoryImpl (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt`)

Reads and writes mirror section 1 with these differences and additions:

1. Table/column names: `animes`, `animes_categories`, `episode_flags`, `getWatchedAnimeNotInLibrary` (`seen = 1 OR last_second_seen != 0`).
2. Library: `getLibraryAnime` / `getLibraryAnimeAsFlow` read `animelibView` (see topic doc: stats switch by `fetch_type`; 40 columns) through `AnimeMapper.mapLibraryAnime`. Tables: `animes, episodes, animehistory, animes_categories`.
3. Upcoming: statuses {ONGOING 1, PUBLISHING_FINISHED 4, UPCOMING 7}; same startOfDay computation (`LocalDate.now().atStartOfDay(ZoneId.systemDefault()).toEpochSecond() * 1000`).
4. `insertAnime` binds 29 columns in addition to the manga ones: `fetch_type` (ordinal), `parent_id`, `season_flags`, `season_number`, `season_source_order`, `background_url`, `background_last_modified`, `memo` (JSON bytes). Note the argument names: the Kotlin call passes `calculateInterval = anime.fetchInterval.toLong()` and `viewerFlags`/`episodeFlags`.
5. `partialUpdateAnime` additionally binds `backgroundUrl`, `backgroundLastModified`, `fetchType`, `parentId`, `seasonFlags`, `seasonNumber`, `seasonSourceOrder`, `memo = memo?.let(MemoColumnAdapter::encode)`. `parent_id` and `memo` can only be set to non-null values via update.
6. Season API (section `p1-topic-seasons-and-relations.md`): `getAnimeSeasonsById`, `getAnimeSeasonsByIdAsFlow`, `removeParentIdByIds`, `getDeletableParentAnime`, `getChildrenByParentId`.
7. `setAnimeCategories` is identical to the manga one with `animes_categories`.

## 6. AnimeMapper (`data/src/main/java/tachiyomi/data/entries/anime/AnimeMapper.kt`)

- `mapAnime` (32 inputs): as `mapManga` plus `episodeFlags = chapterFlags` (the parameter is still named `chapterFlags`: it receives the `episode_flags` column), `backgroundLastModified`, `backgroundUrl`, `fetchType`, `parentId`, `seasonFlags`, `seasonNumber`, `seasonSourceOrder`, `memo`.
- `mapLibraryAnime` (32 + 8): `LibraryAnime(anime, category, totalCount, seenCount.toLong(), bookmarkCount.toLong(), fillermarkCount.toLong(), latestUpload, episodeFetchedAt, lastSeen)`.
- `mapSeasonAnime` (32 + 7): `SeasonAnime(anime, totalCount, seenCount.toLong(), bookmarkCount.toLong(), fillermarkCount.toLong(), latestUpload, fetchedAt, lastSeen)`.
- `mapRelatedAnime` (32 + name): `Pair(name, anime)`.
- `mapDeletableAnime(id, source, fetchType)`: `DeletableAnime(animeId = id, sourceId = source, fetchType)`.

## 7. EpisodeRepositoryImpl (`data/src/main/java/tachiyomi/data/items/episode/EpisodeRepositoryImpl.kt`)

Same shape as section 3 with these differences: `addAllEpisodes` binds 17 values (`anime_id, url, name, scanlator, seen, bookmark, last_second_seen, total_seconds, episode_number, source_order, date_fetch, date_upload, version, summary, preview_url, fillermark, memo`); `partialUpdate` also passes `summary`, `previewUrl`, `fillermark`, `memo?.let(encode)`; there is no scanlator filter parameter (`getEpisodeByAnimeId(animeId)` returns all rows) and no scanlator listing method; `getEpisodeByAnimeIdAsFlow` and `getEpisodeByUrlAndAnimeId`, `getBookmarkedEpisodesByAnimeId`, `getEpisodeById`, `removeEpisodesWithIds` as for chapters. The episode update trigger watches `seen`, `bookmark`, `last_second_seen`. `mapEpisode` has 20 inputs and decodes `memo` with `Json.decodeFromString<JsonObject>`; an unparsable BLOB throws `SerializationException` and fails the whole list query (Proposal: Rust falls back to `{}` and logs `tracing::warn!`, since one bad row should not hide an entire episode list; T-05a-003).

## 8. AnimeRelationRepositoryImpl

Full spec in `p1-topic-seasons-and-relations.md` section 4.

## 9. Category repositories

Full spec in `p1-topic-categories.md`.

## 10. Interactions with other areas

| Interaction | Direction | Area |
|---|---|---|
| Schema, triggers, views, migrations create the objects these repositories query | uses | area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations |
| `history` / `animehistory` tables joined by library views; history repositories write them | uses | area:06-data-repos-history-track-source-misc |
| Domain structs, flag masks, `ChapterSorter`, `EpisodeSorter`, `ChapterRecognition`, `ShouldUpdateDb*`, `*FetchInterval` | used by callers | area:07-domain-entries-items |
| Category/library interactors (create, delete, hide, rename, reorder, reset-flags and set-sort interactors) and library sort enums | calls this area | area:08-domain-library-category-history-updates |
| `SyncChaptersWithSource`, `SyncEpisodesWithSource`, `SyncSeasonsWithSource`, `SyncRelatedAnimeWithSource`, `NetworkToLocal*`, `UpdateManga`, `UpdateAnime` | call this area | area:10-app-domain-layer |
| Backup creation/restore reads `getFavorites`, `getReadMangaNotInLibrary`, `getAllMangaSourceAndUrl`, chapter/episode lookups by url, and writes entries through `insert*`/`update*` | calls this area | area:21-backup-format-and-create, area:22-backup-restore |
| Library screen consumes `getLibraryManga/Anime(AsFlow)`; entry screens consume `getMangaByIdAsFlow`, chapter/episode flows, category flows | calls this area | area:37-ui-library, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime |
| Upcoming screen consumes `getUpcoming*` | calls this area | area:42-ui-updates-history-stats-storage-upcoming |
| Clear-database screens consume `getDeletable*`, `getChildrenByParentId`, `removeParentIdByIds` | calls this area | area:47-settings-screens-general |

## 11. Subscriptions (replacement for Kotlin `Flow`)

OBSERVED semantics to reproduce: (1) first emission is the current result; (2) a new emission after every committed write to a table the query reads; (3) results are produced off the UI thread; (4) `subscribeToOne` fails when the row is gone; (5) no coalescing or distinct filtering.

Proposal: `Subscription<T>` is `tokio::sync::watch::Receiver<Arc<T>>` fed by a tokio task per subscription; the task re-runs the query on the reader connection when the notifier reports an intersecting table, compares with the previous value (`PartialEq`), and sends only on change. Bursts are coalesced: after the first notification the task waits 50 ms (library views: 100 ms) for further notifications before re-querying. `subscribe_one` yields `Option<T>` (`None` = row gone) instead of failing, so a details screen can close itself when the entry is deleted from the library cleanup. Dropping the receiver ends the task. The GTK bridge is in `p1-rust-design.md` section 6.
