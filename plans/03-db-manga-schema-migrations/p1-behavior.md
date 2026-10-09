# Area 03 / part 1 — Behavior spec: manga database (`data/src/main/sqldelight`)

Scope: the 9 `.sq` table files, 3 view files, 32 migrations, `data/build.gradle.kts` and `data/src/main/AndroidManifest.xml`. All paths are repo-relative to `/home/blackbox/aniyomi-src`. "OBSERVED" = what the Kotlin/SQL does (verified by running the SQL where stated). "Proposal:" = what Kurobayomi's Rust must do. Companion files: `p1-topic-baseline-schema.md` (full DDL), `p1-topic-query-catalog.md` (every statement), `p1-topic-triggers-and-versioning.md`, `p1-topic-migration-history.md`, `p1-topic-reactive-runtime.md`, `p1-data-and-config.md` (columns, units, enums), `p1-rust-design.md`, `p1-edge-cases-and-tests.md`, `p1-tasks.md`.

## 1. Database lifecycle

### 1.1 OBSERVED

1. Two independent SQLite files, each with its own SQLDelight database: manga `tachiyomi.db` (`tachiyomi.data.Database`, schema version 33) and anime `tachiyomi.animedb` (`tachiyomi.mi.data.AnimeDatabase`, schema in `data/src/main/sqldelightanime`, owned by area:04-db-anime-schema-migrations). Both are declared in `data/build.gradle.kts` (`create("Database")` with `srcDirs ./src/main/sqldelight`; `create("AnimeDatabase")` with `./src/main/sqldelightanime`; both `dialect(libs.sqldelight.dialects.sql)` = `sqlite-3-38-dialect`).
2. The `categories` table exists in BOTH databases (separate rows, separate ids): the manga library's categories are not shared with the anime library's.
3. Creation of an empty file runs the baseline (`onCreate`); an older file runs `N.sqm` for N in `[old, 33)`; a newer file fails (see `p1-topic-migration-history.md` section 1).
4. Pragmas are applied on every open AFTER create/upgrade: `foreign_keys = ON`, `journal_mode = WAL`, `synchronous = NORMAL`.
5. `data/src/main/AndroidManifest.xml` contains only `<manifest />` (2 lines): the module declares nothing (no permissions, components). `data/build.gradle.kts` also wires `kotlinx-serialization` (JSON, JSON okio, protobuf; used by the backup/release code in other files), depends on `projects.sourceApi`, `projects.domain`, `projects.core.common`, and exports the SQLDelight bundle as `api(libs.bundles.sqldelight)`; compiler opt-in `kotlinx.serialization.ExperimentalSerializationApi`.

### 1.2 Proposal: Rust lifecycle

1. File `manga.db` in the data directory from the `Paths` trait (`plans/addenda/requirements-cross-platform.md`). Never open an Aniyomi-created file directly; import it (`T-03a-023`).
2. Open sequence: open connection with the verified pragma set (`p1-topic-reactive-runtime.md` section 2) -> read `application_id`, `user_version` and the table count -> no tables: run the baseline script in one transaction, set `application_id = 0x4B555230` (ASCII "KUR0") and `user_version = 33` -> tables present and `application_id` differs (an Aniyomi/Mihon file of ANY version, including a version-33 one, has `application_id = 0`): refuse with `LegacyFile` and route to the importer -> `user_version` greater than the app's latest: refuse with `TooNew` -> `user_version` lower than the app's latest: apply Kurobayomi migrations `34..` in order, each in its own transaction, each followed by `PRAGMA foreign_key_check` (must return no rows).
3. Before applying any migration (`user_version` < `SCHEMA_VERSION` on an existing file) copy `manga.db` with `VACUUM INTO 'manga.db.pre-v<from>-<UTC timestamp>.bak'` (WAL-safe); keep the last 3 such backups. The Aniyomi app has no such safety net.
4. After open: run `PRAGMA integrity_check` only when the previous shutdown was not clean (a `manga.db.dirty` marker written on open and removed on clean exit) or on explicit request from Settings > Advanced; a failure moves the file to `manga.db.corrupt-<timestamp>` and offers restore from the newest backup.
5. Startup hook: `UPDATE mangas SET is_syncing = 0 WHERE is_syncing = 1` and the same for chapters (the two `resetIsSyncing` statements that Aniyomi never calls) so a crash during a restore cannot leave rows permanently excluded from version bumping (`T-03a-030`).

## 2. Entities and field semantics (what each column MEANS to the app)

### 2.1 `mangas` (one row per `(source, url)` the app has ever seen, favourite or not)

| Column | Meaning and rules |
|---|---|
| `_id` | rowid alias, no AUTOINCREMENT: the next id is `max(_id) + 1`, so ids of deleted TOP rows get reused |
| `source` | source id (64-bit, Java `Long` hash of name/lang/version for extensions; the local source has a fixed id, see area:14-source-local-and-metadata) |
| `url` | the source-relative URL; together with `source` the logical key (`getMangaByUrlAndSource` LIMIT 1; there is NO unique index, duplicates are possible and tolerated) |
| `title`, `artist`, `author`, `description` | text from the source; `artist/author/description` nullable |
| `genre` | `TEXT`, comma+space separated list (`", "`), see codec in `p1-data-and-config.md` |
| `status` | `SManga` constant 0..6 (UNKNOWN 0, ONGOING 1, COMPLETED 2, LICENSED 3, PUBLISHING_FINISHED 4, CANCELLED 5, ON_HIATUS 6) |
| `thumbnail_url` | cover URL (nullable) |
| `favorite` | 1 = in library |
| `last_update` | ms timestamp of the last time the CHAPTER LIST changed (set by `UpdateManga.awaitUpdateLastUpdate` only when `SyncChaptersWithSource` added/removed/updated chapters) |
| `next_update` | ms timestamp of the next scheduled library update (written by `MangaFetchInterval`); `0` = none; used by the "Upcoming" screen (`getUpcomingManga`) |
| `initialized` | 1 once details were fetched from the source (set true by `awaitUpdateFromSource`; restore sets `initialized = description != null`) |
| `viewer` (Kotlin: `viewerFlags`) | per-entry reader settings bit field (reading mode bits 0-2, orientation bits 3-5; see data doc) |
| `chapter_flags` | per-entry chapter list settings bit field (sort direction, sort key, read/downloaded/bookmark filters, display mode; layout in data doc) |
| `cover_last_modified` | ms timestamp used as cache-busting key for the cover (bumped when the cover URL changes or the user sets a custom cover) |
| `date_added` | ms timestamp when the entry was added to the library; reset to `0` on removal (`UpdateManga.awaitUpdateFavorite`) |
| `update_strategy` | `0 ALWAYS_UPDATE`, `1 ONLY_FETCH_ONCE` (`UpdateStrategy.ordinal`); unknown values decode to `ALWAYS_UPDATE` |
| `calculate_interval` (Kotlin: `fetchInterval`) | days between expected chapter releases, `0` = not computed yet, negative = user-forced fixed interval (see 3.6) |
| `last_modified_at`, `favorite_modified_at`, `version`, `is_syncing` | sync bookkeeping maintained by triggers (`p1-topic-triggers-and-versioning.md`) |

### 2.2 `chapters`

`url` (source-relative, the key within a manga), `name` (sanitised at write time by `ChapterSanitizer`, see area:05-data-repos-entries-items), `scanlator` (nullable; trimmed by migration 28 for old rows and by the sync code for new ones), `read`, `bookmark` (0/1), `last_page_read` (0-based page index; `0` also means "not started"), `chapter_number` (REAL, `-1.0` = unrecognised), `source_order` (position in the source's list: `0` = first/newest), `date_fetch` (ms; ordering key for "Updates"), `date_upload` (ms; `0` replaced at sync time, never stored as 0 for new rows, see 3.4), plus `last_modified_at` (SECONDS), `version`, `is_syncing`. `ON DELETE CASCADE` from `mangas`.

### 2.3 `categories` and `mangas_categories`

`categories._id = 0` is the permanent system category ("Default"): `name = ''`, `sort = -1`, `flags = 0`, protected by trigger T1. `sort` = display order; `flags` = per-category library sort mode (bit layout in the data doc); `hidden` = 1 hides the tab. A manga with no `mangas_categories` row appears in the library under category `0` (the view coalesces the missing join to 0). `mangas_categories` has no unique constraint and no index of its own (both foreign keys cascade).

### 2.4 `history`

One row per chapter ever opened (`chapter_id UNIQUE`). `last_read` = ms timestamp of the last read; `0` = "removed from history" (soft delete: the row and `time_read` stay). `time_read` = accumulated read time in ms across sessions.

### 2.5 `manga_sync`

One row per (manga, tracker) pair (`UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`). `sync_id` = tracker id (1 MyAnimeList, 2 AniList, 3 Kitsu, 4 Shikimori, 5 Bangumi, 6 Komga, 7 MangaUpdates, 8 Kavita, 9 Suwayomi; 101 Simkl and 102 Jellyfin are anime trackers; source: `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt`). `remote_id` = id of the entry on the tracker, `library_id` = the tracker's list-entry id (nullable), `status` = tracker-specific status number, `score` = REAL in the tracker's native scale, `remote_url`, `start_date`/`finish_date` (ms, `0` none), `private` (0/1, AniList/other "private" flag).

### 2.6 `excluded_scanlators`, `sources`, `extension_repos`

`excluded_scanlators(manga_id, scanlator)` = per-entry blacklist of scanlator groups; matching is exact string equality (`NULL` scanlator can never match). `sources(_id, lang, name)` = remembered display data of sources whose extension disappeared ("stub" sources, so the library can still label them). `extension_repos` = user-added extension repositories keyed by `base_url`; `signing_key_fingerprint` is globally UNIQUE (two repos cannot share a key).

## 3. Algorithms that read or write these tables

### 3.1 Network entry -> local row (`domain/src/main/java/tachiyomi/domain/entries/manga/interactor/NetworkToLocalManga.kt`)

1. `getMangaByUrlAndSource(manga.url, manga.source)`.
2. No row: `insert` the entry (`insertManga` returns `last_insert_rowid()` inside a transaction) and return it with the new id.
3. Row exists and `favorite = 0`: return the stored row with `title` replaced by the source's title IN MEMORY ONLY (display; it is persisted only when the entry is later favourited/updated).
4. Row exists and `favorite = 1`: return the stored row unchanged (library entries keep their own title).

### 3.2 Add to / remove from library (`app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt:awaitUpdateFavorite`)

`MangaUpdate(id, favorite = f, dateAdded = if (f) now_ms else 0)` through the coalesce `update`. Side effects by triggers: T2 sets `favorite_modified_at`, T3 sets `last_modified_at`, T4 bumps `version`. Removing from library does NOT delete chapters, history, tracks or category assignments (they stay until "Clear database" or migration).

### 3.3 Refresh details from source (`UpdateManga.awaitUpdateFromSource`)

For one `SManga`: the update writes `author, artist, description, genre (getGenres(): split on ", ", trim, drop blanks, distinct), thumbnailUrl (null when empty), status, updateStrategy, initialized = true`; `title` only when `!favorite && remoteTitle.isNotEmpty()`; `coverLastModified` per this rule: null (unchanged) when the remote thumbnail URL is null/empty; null when not a manual fetch and the URL did not change; `now_ms` when the manga is the local source; null (and cache entry deleted) when the entry has a custom cover; otherwise cache entry deleted and `now_ms`. Because the statement uses `coalesce`, a source returning `null` for author/artist/description/genre never clears the stored value (OBSERVED limitation, kept).

### 3.4 Chapter list synchronisation (`app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt`) — the only writer of new chapter rows

Order of database effects after the in-memory diff (diff is by exact `url` equality; `rawSourceChapters.distinctBy { it.url }`; `source_order` = index in the (deduplicated) source list; empty source list and non-local source -> `NoChaptersException`):

1. Nothing new/removed/changed: only `awaitUpdateFetchInterval` when `manualFetch || fetchInterval == 0 || nextUpdate < fetchWindow.first`; return empty.
2. New chapters get `dateFetch = nowMillis + itemCount--` starting at `newChapters.size` (so the first listed = newest gets the largest value; guarantees a strict order inside one fetch, in ms); a new chapter whose `dateUpload == 0` gets `maxSeenUploadDate` (or `nowMillis` if none yet).
3. Removed chapters (in DB, absent from source): `removeChaptersWithIds(ids)`.
4. `addAllChapters(newChapters)` (one transaction; on failure the list returned is empty and the error is only logged).
5. `updateAllChapters(changed)` for chapters whose name/number/scanlator/order differ (`ShouldUpdateDbChapter`), writing `name, chapterNumber, scanlator, sourceOrder` and `dateUpload` only when the source's value != 0.
6. `awaitUpdateFetchInterval(manga, now, fetchWindow)`, then `awaitUpdateLastUpdate(manga.id)` (= `last_update = now_ms`).
7. Carry-over rules: if a new chapter has the same recognised number as a removed one, it inherits `read` (if the removed one was read), `bookmark` (if it was bookmarked) and the old `date_fetch` (so "Updates" is not polluted); if `markDuplicateReadChapterAsRead` contains `MARK_DUPLICATE_CHAPTER_READ_NEW` and the number is among already-read numbers, the new chapter is stored `read = 1`.
8. Return value (chapters to notify/download) excludes carried-over/duplicate-read chapters and chapters whose scanlator is in `excluded_scanlators`.

All of this is in area:10-app-domain-layer; area 03 requires only the statements and the guarantee that `chapters.insert` + `update` + `removeChaptersWithIds` can run in one transaction.

### 3.5 Reading a chapter list

`getChaptersByMangaId(mangaId, applyScanlatorFilter)` returns rows without ordering; consumers sort in memory (`domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt:getChapterSort`): source order (flag 0x00): for DESC flag semantics `c1.sourceOrder.compareTo(c2.sourceOrder)`, i.e. the sort key for "source" is inverted relative to the others (descending shows source order ascending = newest first); number (0x100) compares `chapterNumber`; upload date (0x200) compares `dateUpload`; alphabet (0x300) compares names with a locale collator (`compareToWithCollator`). `applyScanlatorFilter = true` for UI lists and "next chapter" computations (`GetNextChapters`, `MangaFetchInterval`), `false` for backup.

### 3.6 Fetch-interval columns (`domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt`, tests in `domain/src/test/java/tachiyomi/domain/entries/manga/interactor/MangaFetchIntervalTest.kt`, owned by area:07-domain-entries-items)

Writes `calculate_interval` (days; `MAX_INTERVAL = 28`; range 1..28 for computed values) and `next_update` (ms). `GRACE_PERIOD = 1` day: `getWindow` = [start of yesterday, end of tomorrow]. Computation: take the newest `chapterWindow` distinct upload days (`chapterWindow = 3` when the entry has <= 8 chapters else `10`; only chapters with `dateUpload > 0`); with >= 3 days: `floor(daysBetween(oldest, newest) / indexOf(oldest))`, else the same from `dateFetch` days, else `7`; clamp to 1..28. A negative stored interval is kept as a fixed user value. `calculateNextUpdate` keeps the current `next_update` when it lies inside the grace window; otherwise `latestDate = start of day of lastUpdate (or now when lastUpdate = 0)`, `cycle = floor(daysSinceLatest / (|interval| if interval < 0 else increaseInterval(interval, daysSinceLatest, 10)))`, result `(latestDate + (cycle + 1) * |interval| days)` in ms; `increaseInterval` doubles the interval while `(daysSinceLatest / delta + 1) > 10` and `delta < 28`. Area 03's responsibility: the two columns, `INTEGER NOT NULL DEFAULT 0` (interval) and nullable `INTEGER` (next update), mapped as `lastUpdate ?: 0`, `nextUpdate ?: 0`.

### 3.7 Library composition (`libraryView`, `app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt` in area:37-ui-library)

1. The view returns ONE ROW PER (favourite manga, category assignment): a manga in two categories appears twice with `category` 1 and 2; a manga in none appears once with `category = 0`; a manga with a stale assignment to a deleted category cannot exist (cascade). Consumers group rows by `category`.
2. Counters ignore chapters of excluded scanlators: `totalCount`, `readCount`, `bookmarkCount`, `latestUpload`, `chapterFetchedAt`, `lastRead` are computed over chapters for which `excluded_scanlators.scanlator IS NULL` after a LEFT JOIN on `(manga_id, scanlator)`.
3. `lastRead = coalesce(max(history.last_read), 0)`: reset history (`last_read = 0`) therefore yields 0 = "never".
4. `unreadCount` is not stored: Kotlin computes `totalChapters - readCount` (`LibraryManga.unreadCount`), `hasStarted = readCount > 0`, `hasBookmarks = bookmarkCount > 0`.
5. Performance observation (EXPLAIN and timing): the aggregate subquery scans every chapter of every manga (it is not restricted to favourites before materialisation) and builds an automatic index on `mangas_categories(manga_id)` per execution; 259 ms for 5000 favourites and 300000 chapters (details in `p1-topic-baseline-schema.md` section 5).

### 3.8 Categories

- Create (`domain/src/main/java/tachiyomi/domain/category/manga/interactor/CreateMangaCategoryWithName.kt`): `nextOrder = max(order over ALL categories incl. the system one) + 1` (or `0` when the list is empty, which cannot happen because the system row exists, so the first user category gets `sort = 0`); `flags = sortingMode.type.flag | sortingMode.direction.flag` from preference `library_sorting_mode` (default `ALPHABETICAL,ASCENDING` -> `0b01000000 = 64`); `hidden = 0`. Name uniqueness is NOT enforced by the database or the interactor; the UI checks (message `MR.strings.error_category_exists`, area:37-ui-library).
- Rename: `update(name)` only. Hide toggle (`HideMangaCategory`): `update(hidden = !category.hidden)`.
- Reorder (`ReorderMangaCategory`, guarded by a `Mutex`): load all categories, drop the system category, move the item to `newIndex`, write `sort = index` for every user category in one transaction (`updatePartialMangaCategories`); result `Unchanged` when the id is unknown.
- Delete (`DeleteMangaCategory`): (1) `DELETE FROM categories WHERE _id = ?` (cascade drops its `mangas_categories` rows; entries fall back to the default view); (2) reload ALL categories (the system one included) and renumber `sort = index` for every one: the system category becomes `sort = 0` and the first user category `1` (OBSERVED quirk, harmless because ordering is relative, but it creates ties after the next reorder which writes `0..n-1` to user categories only); (3) if preference `default_category` equals the deleted id it is deleted (back to -1 = "always ask"); (4) the deleted id string is removed from these string-set preferences: `library_update_categories` (listed TWICE in the code, a copy-paste slip: `library_update_categories_exclude` is never cleaned), `remove_exclude_categories`, `download_new_categories`, `download_new_categories_exclude`; (5) the renumbering is written in one transaction. Failure in step 1 returns `InternalError` before any preference is touched.
- Per-category sort (`SetSortModeForMangaCategory`): `flags = (category?.flags ?: 0) + type + direction` with the `FlagWithMask.plus` rule (`this and mask.inv() or (flag and mask)`); picking `Random` stores a new random `Int` seed in preference `library_random_manga_sort_seed`; if the category exists and preference `categorized_display` is true the single category is updated, otherwise the global preference `library_sorting_mode` is set AND `updateAllFlags(flags)` rewrites every category's flags (including the system one).
- `ResetMangaCategoryFlags`: `updateAllFlags(sort.type + sort.direction)` from the global preference.
- Backup restore of categories (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaCategoriesRestorer.kt`): matches by NAME; unmatched ones are inserted with `order = nextOrder++` (nextOrder starts at `max(existing order) + 1`) and the backup's `flags`; afterwards `categorized_display` is set to `(existing + new).distinctBy { flags }.size > 1`.

### 3.9 Manga <-> category assignment

`setMangaCategories(mangaId, categoryIds)` = in one transaction `DELETE FROM mangas_categories WHERE manga_id = ?` then one `INSERT` per id (no duplicate check; passing the same id twice creates two rows). Each insert bumps `mangas.version` (T7). `categoryIds` empty = the entry becomes "uncategorised". The error path of `SetMangaCategories.await` swallows exceptions (logged).

### 3.10 History

- Write (`ReaderViewModel.kt:594-602`): unless incognito: `readAt = Date()`, `sessionReadDuration = readAt.time - chapterReadStartTime` (ms; `0` when no start time) -> `history.upsert(chapterId, readAt_ms, sessionReadDuration)`; on conflict `last_read = readAt`, `time_read = time_read + delta`. `chapterReadStartTime` is reset to null after each upsert; `restartReadTimer()` sets it to now. Errors are logged, not thrown.
- Per-page progress writes `chapters.update(read, last_page_read)` on EVERY page change (`ReaderViewModel.kt:533-552`; `read = true` only on the last page); each such write fires T5/T6 (so `chapters.version` grows once per page that changes `last_page_read`).
- History screen (`historyView` + `history` query): one row per MANGA (the chapter with the greatest `last_read`), `readAt > 0` hides soft-deleted entries, search = case-insensitive substring on the title (`lower(title) LIKE '%' || :query || '%'`, ASCII case folding by SQLite, `%`/`_` in the query act as wildcards). Ties on equal `last_read` inside one manga select an arbitrary one of the tied chapters (SQLite bare-column rule: with a single `MAX()` the bare column comes from a row holding the max; verified).
- "Remove" in the list = `resetHistoryById` (soft); "Remove all for this entry" = `resetHistoryByMangaId` (soft, all chapters of the manga); "Clear history" = `DELETE FROM history` (hard, also loses read time); "Clear database" additionally runs `removeResettedHistory` (hard-deletes soft-deleted rows).
- Resume shortcut: `getLatestHistory` (`readAt > 0 ORDER BY readAt DESC LIMIT 1`) then `GetNextChapters` (`domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetNextChapters.kt`): chapters sorted ascending by the manga's own sort settings; for `onlyUnread = true` returns the unread chapters from the current one; for `false` returns from the current chapter onward, dropping the current one when it is already read.
- Statistics: `getReadDuration` = `coalesce(sum(time_read), 0)` over ALL rows (soft-deleted rows still count; only "Clear history" removes the time).
- Restore merge (`MangaRestorer.restoreHistory`): for each backup history item locate the DB row by chapter URL; new -> insert for the chapter found by URL (`getChapterByUrl`, skipped when the chapter is unknown); existing -> `readAt = max(backup, db)` (0 -> null), `readDuration = max(backup, db) - db.time_read` passed as the DELTA to `upsert` (so the stored total becomes `max`).

### 3.11 Updates feed (`updatesView`)

Rows = chapters of FAVOURITE manga with `date_fetch > date_added` (excludes chapters that existed when the entry was added), view-sorted by `date_fetch DESC`; filters: `dateUpload > :after` (note: upload date, not fetch date) and optional `read = :read`; `LIMIT 500` always (`GetMangaUpdates`). Chapters with `date_upload = 0` cannot appear when `after > 0`.

### 3.12 Tracker rows

Insert goes through `INSERT` and relies on `ON CONFLICT REPLACE`: re-binding a tracker deletes the old row (new `_id`). `delete(mangaId, trackerId)` removes it. Backup restore (`MangaRestorer.restoreTracking`): new -> insert with `id = 0`; existing and different -> update keeping the DB row but taking `remoteId`, `libraryId` from the backup and `lastChapterRead = max(db, backup)`.

### 3.13 Excluded scanlators

`SetExcludedScanlators`: in one transaction read the current set, insert `toAdd = new - current` one by one, then `DELETE FROM excluded_scanlators WHERE manga_id = :mangaId AND scanlator IN :scanlators` (`toRemove = current - new`). Effects visible through `libraryView` counters, `getChaptersByMangaId(applyScanlatorFilter = true)` and the notification filter in 3.4.

### 3.14 Extension repositories (`domain/src/main/java/mihon/domain/extensionrepo/**`)

- Add (`CreateMangaExtensionRepo.await(indexUrl)`): the input must parse as an HTTP URL (`toHttpUrlOrNull().toString()`, which normalises) AND match `^https://.*/index\.min\.json$`, else `InvalidUrl` (message `MR.strings.invalid_repo_name` = "Invalid repo URL"). `baseUrl = url.removeSuffix("/index.min.json")`. Fetch `GET <baseUrl>/repo.json` (JSON `{"meta": {"name", "shortName", "website", "signingKeyFingerprint"}}`; any exception or HTTP failure -> `InvalidUrl`). Insert; on a SQLite constraint failure (PK or fingerprint) look up by `baseUrl` -> `RepoAlreadyExists` (message `error_repo_exists` "This repo already exists!"), else by fingerprint -> `DuplicateFingerprint(old, new)` (dialog with title `action_replace_repo_title` "Signing Key Fingerprint Already Exists", body `action_replace_repo_message`, button `action_replace_repo`), else `Error`.
- Replace (`ReplaceMangaExtensionRepo`): `replace` statement = upsert on the fingerprint conflict (the OLD row's `base_url` is overwritten with the new URL).
- Refresh all (`UpdateMangaExtensionRepo.awaitAll`): concurrently for every repo re-fetch `repo.json`; accept the new details only when the stored fingerprint starts with `NOFINGERPRINT` or equals the fetched one (protection against a repo silently changing its signing key) and then `upsert` by base_url.
- Delete: `DELETE ... WHERE base_url`. Count flow drives the "no repos" hint (`information_empty_repos`). Confirmations: `add_repo_confirmation` "Do you wish to add the repo \"%s\"?", `delete_repo_confirmation` "Do you wish to delete the repo \"%s\"?". Backup restore refuses a backup repo whose URL exists with a different fingerprint (error "Already Exists with different signing key fingerprint") or whose fingerprint exists under another URL (error `"<name> has the same signing key fingerprint"`).

### 3.15 Stub sources (`AndroidMangaSourceManager.kt:100-126`)

For every source of every installed extension the manager compares with the stored stub (`findOne`); if different it `upsert`s `(id, lang, name)` and, when a stub existed, renames the download folders (`downloadManager.renameSource(old, new)`, area:23-downloads-manga). When a source id is requested but not installed: stored stub if present, else extension-store lookup, else an empty stub (`lang = ""`, `name = ""`). A stub renders as `"<name> (<LANG>)"`, or the numeric id when name or lang is blank.

### 3.16 Maintenance actions

- Clear database (Settings > Advanced > "Clear Manga database", `app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearDatabaseScreen.kt`): list = `getSourceIdsWithNonLibraryManga` (per source with count, sorted by source name in the UI; empty -> empty-state `MR.strings.database_clean` "Nothing to clear"); each row shows `MR.strings.clear_database_source_item_count` ("%1$d non-library entries in database"); actions select all / select inverse; delete button enabled when the selection is non-empty -> confirmation dialog text `AYMR.strings.clear_database_confirmation`; on OK (`withNonCancellableContext`): `deleteMangasNotInLibraryBySourceIds(selection)` then `removeResettedHistory()` (two separate statements, not one transaction), clear selection, hide dialog, toast `MR.strings.clear_database_completed` ("Entries deleted"). Deleting manga cascades to chapters, history, tracks, category links and excluded scanlators of THOSE entries.
- Reset per-series reader settings (`pref_reset_viewer_flags`): `UPDATE mangas SET viewer = 0` -> toast `pref_reset_viewer_flags_success` ("All reader settings reset") or on exception `pref_reset_viewer_flags_error` ("Couldn't reset reader settings"); it also rewrites `last_modified_at` of every row (T3).
- Upcoming: `getUpcomingManga(startOfToday_ms, {1, 4})` = favourites with `status IN (ONGOING, PUBLISHING_FINISHED)` and `next_update >= startOfToday` sorted ascending; the domain object exposes `expectedNextUpdate = nextUpdate` unless `status == COMPLETED`.
- Duplicate check on add (`GetDuplicateLibraryManga`): favourites with `LOWER(title) = lowercase(title)` and a different id; the UI then shows `MR.strings.confirm_add_duplicate_manga` ("You have an entry in your library with the same name.\n\nDo you still wish to continue?"). SQLite `LOWER` folds ASCII only while Kotlin `lowercase()` folds Unicode, so titles with non-ASCII capitals (for example "Ärger") never match their own lowercase form (verified: `lower('Ä')` returns `Ä`).

## 4. Concurrency, dispatchers, timings, limits

| Item | Value | Source |
|---|---|---|
| Query dispatcher | `Dispatchers.IO`; one dedicated thread per open transaction | `AndroidMangaDatabaseHandler.kt`, `MangaTransactionContext.kt` |
| Transactions | at most one writer at a time; nested calls join | same |
| WAL + `synchronous = NORMAL` | durable after checkpoint, loses at most the last transactions on power loss, never corrupts | `AppModule.kt:88-95` |
| Updates feed page size | 500 rows | `domain/src/main/java/tachiyomi/domain/updates/manga/interactor/GetMangaUpdates.kt` |
| Interval constants | `MAX_INTERVAL = 28`, `GRACE_PERIOD = 1`, `chapterWindow` 3 or 10 (threshold 8 chapters), `increaseWhenOver = 10`, default interval 7 | `MangaFetchInterval.kt` |
| `date_fetch` stagger | `nowMillis + itemCount--` | `SyncChaptersWithSource.kt` |
| SQLite variable limit | `IN :list` expands to one `?` per element (SQLite default `SQLITE_MAX_VARIABLE_NUMBER` 32766 since 3.32); `removeChaptersWithIds` and `deleteMangasNotInLibraryBySourceIds` fail with "too many SQL variables" above that | SQLDelight codegen |
| Reactive refresh | conflated, no debounce | handler |

## 5. Interactions with other areas

- area:05-data-repos-entries-items, area:06-data-repos-history-track-source-misc: the Kotlin repositories and mappers sit on top of these statements.
- area:07-domain-entries-items, area:08-domain-library-category-history-updates: models/interactors that compute flags and fetch intervals.
- area:11-app-migrations: app-level migrations such as `app/src/main/java/mihon/core/migration/migrations/MigrateRotationViewerValuesMigration.kt` rewrite `viewer_flags` values.
- area:21-backup-format-and-create, area:22-backup-restore: consume `version`, `last_modified_at`, `favorite_modified_at`, `is_syncing`, `excluded_scanlators` and perform merges described above.
- area:26-library-update-jobs: reads `next_update`, `update_strategy`, `calculate_interval`, categories and flags; writes `last_update`.
- area:27-tracking-core: owns `manga_sync` semantics beyond storage (status numbers per tracker).
- area:37-ui-library, area:42-ui-updates-history-stats-storage-upcoming, area:47-settings-screens-general: UI of everything above.
- area:04-db-anime-schema-migrations: the sibling database; shares the runtime design.

## 6. Quirk register (each has a decision)

| # | Quirk | Decision |
|---|---|---|
| Q1 | `coalesce` updates cannot set a column to NULL (descriptions, author, genre, scanlator, thumbnail can never be cleared by the app) | Keep; `Option` patch types |
| Q2 | Description NULL transitions never bump `version` (NULL-blind `!=`) | Keep (in the SQL) |
| Q3 | Mixed units: `last_modified_at`, `favorite_modified_at` in seconds, everything else in ms | Keep; document; codec helpers named `secs`/`millis` |
| Q4 | Migration 10 backfills one `date_added` for all rows | Keep in legacy importer; report |
| Q5 | Migration 15 leaves `history_temp` | Importer drops it |
| Q6 | Upgraded DBs differ from fresh DBs (5 differences) | Baseline = fresh schema with `history.last_read NOT NULL`; importer normalises |
| Q7 | Category delete renumbers the system category and omits `library_update_categories_exclude` from cleanup | Fix in Rust (area:08-domain-library-category-history-updates): renumber only user categories; clean the five string-set preferences `library_update_categories`, `library_update_categories_exclude`, `remove_exclude_categories`, `download_new_categories`, `download_new_categories_exclude` and reset `default_category` to -1 when it equals the deleted id |
| Q8 | `getCategories` has no tiebreak | Rust adds `, _id` |
| Q9 | `getTrackByMangaId` actually queries the track `_id` | Rust names it `get_track_by_id`; no caller needs the manga-id semantics |
| Q10 | `LOWER(title)` ASCII-only duplicate check | Proposal P2: register a Unicode lowercase SQL function and use it on both sides (`T-03a-025`) |
| Q11 | History search treats `%` and `_` as wildcards | Proposal: escape with `ESCAPE '\'` (P2, in `T-03a-013`) |
| Q12 | `getChapterByUrl` full scan (16.8 ms at 300000 chapters) | `chapters_url_index` in migration 34 (0.05 ms; `T-03a-024`) |
| Q13 | `IN :list` variable limit | `json_each` (`T-03a-026`) |
| Q14 | `extension_repos.sq` uses `=: website` | write `= :website` |
| Q15 | `resetIsSyncing` never called | startup hook (`T-03a-030`) |
| Q16 | `getAllManga`, `getMangasWithFavoriteTimestamp`, `getBookmarkedChaptersByMangaId` unused | port as constants, no UI |
| Q17 | Clear database is two non-atomic statements | Rust runs both in one transaction |
| Q18 | One chapter write per page turn | Proposal for area:43-reader-core: buffer progress and flush at most every 2 s and on page-session end; the DB layer stays as is |
| Q19 | Two databases each with `categories` | Keep two files for parity with backups (`BackupCategory` is per medium) |
| Q20 | `libraryView` aggregates the chapters of every manga, including non-library entries (measured 246 ms with 500 favourites among 5000 manga and 300000 chapters) | P2: migration 34 redefines the view with a `chapters.manga_id IN (SELECT _id FROM mangas WHERE favorite = 1)` pre-filter (34 ms, identical rows verified with `EXCEPT` both ways); `T-03a-024` |
