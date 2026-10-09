# 22-backup-restore — Part 1 behaviour spec

Conventions:
- **OBSERVED** = what the Kotlin code does (cited path:line).
- **PROPOSAL** = what Kurobayomi should do.
- Defects are numbered D1..D23 and are referenced from the other files. A defect that Kurobayomi must not reproduce is marked "fix". A defect kept for parity is marked "keep".
- Message keys are `MR.strings.*` (tachiyomi i18n, file `i18n/src/commonMain/moko-resources/base/strings.xml`) or `AYMR.strings.*` (aniyomi i18n, file `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`). Plurals are in `i18n/src/commonMain/moko-resources/base/plurals.xml`.

---

## 1. End-to-end flow

### 1.1 Entry point (OBSERVED)
1. Settings > Data > "Backup and restore" row (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt:191-258`).
2. The "Restore backup" segment (`MR.strings.pref_restore_backup`, "Restore backup") runs, in order:
   - If `BackupRestoreJob.isRunning(context)` is true: toast `MR.strings.restore_in_progress` ("Restore is already in progress") and stop.
   - Else, if `DeviceUtil.isMiui && DeviceUtil.isMiuiOptimizationDisabled()`: toast `MR.strings.restore_miui_warning` (Android only, see D-drop in p1-tasks T-22a-039).
   - Launch `GetContent` with MIME `*/*` and chooser title `MR.strings.file_select_backup` ("Select backup file").
   - Null result: toast `MR.strings.file_null_uri_error` ("No file selected") and stop.
   - Non-null: push `RestoreBackupScreen(uri.toString())`.
3. The "Create backup" segment pushes `CreateBackupScreen()`.

### 1.2 Restore screen lifecycle (OBSERVED)
1. `RestoreBackupScreenModel` is created with the URI. `init` calls `validate(uri.toUri())` synchronously on the caller's thread (Compose `rememberScreenModel`, i.e. the composition thread; `RestoreBackupScreen.kt:51,172-174`). This is a full decode of the file (see §3).
2. Initial state: `State(error = null, canRestore = false, options = RestoreOptions())` (`RestoreBackupScreen.kt:223-228`). Defaults: all options true except `extensions = false` (see §2).
3. Validation result sets `(error, canRestore)`:
   - Decode/validator exception: `error = InvalidRestore(uri, e.message.toString())`, `canRestore = false` (`RestoreBackupScreen.kt:192-201`). (D13: when `e.message` is null the text becomes the literal `"null"`.)
   - Missing sources or trackers non-empty: `error = MissingRestoreComponents(uri, sources, trackers)`, `canRestore = true` (`RestoreBackupScreen.kt:203-209`).
   - Otherwise `error = null`, `canRestore = true` (`RestoreBackupScreen.kt:211`).
4. The option checkbox block is shown only when `state.canRestore` (`RestoreBackupScreen.kt:78`). The error card is always shown when `state.error != null` (`RestoreBackupScreen.kt:94-96`).
5. The action button "Restore" (`MR.strings.action_restore`, "Restore") is enabled iff `state.canRestore && state.options.canRestore()` (`RestoreBackupScreen.kt:65-66`).
6. Clicking the action calls `model.startRestore()` and then `navigator.pop()` (`RestoreBackupScreen.kt:67-70`). The screen closes at once; progress is only in notifications.
7. `startRestore()` enqueues the job with the current `options` and the same URI (`RestoreBackupScreen.kt:184-190`).

### 1.3 Job lifecycle (OBSERVED, `BackupRestoreJob.kt`)
1. `start(context, uri, options, sync=false)` builds input data: `location_uri` (String, `uri.toString()`), `sync` (Boolean), `options` (BooleanArray from `RestoreOptions.asBooleanArray()`), and enqueues a `OneTimeWorkRequest` tagged `"BackupRestore"` with unique work name `"BackupRestore"` and `ExistingWorkPolicy.KEEP` (`BackupRestoreJob.kt:80-96`). `KEEP` means a second start while one is pending or running is silently dropped.
2. `stop(context)` cancels the unique work `"BackupRestore"` (`BackupRestoreJob.kt:98-100`).
3. `isRunning(context)` is true iff a work item with tag `"BackupRestore"` is in state `RUNNING` (`WorkManagerExtensions.kt:14-17`). A queued but not running job is not reported as running.
4. `doWork()` (`BackupRestoreJob.kt:30-61`):
   a. Parse `location_uri`; parse `options` from the BooleanArray (must be length 7; index map in §2). If either is missing: `Result.failure()` with no notification.
   b. Read `sync` (default false).
   c. `setForeground(getForegroundInfo())`. An `IllegalStateException` is logged at ERROR with message `"Not allowed to run on foreground service"` and the job continues.
   d. Run `BackupRestorer(context, notifier, isSync).restore(uri, options)`. On success: `Result.success()`.
   e. On `CancellationException`: `notifier.showRestoreError(MR.strings.restoring_backup_canceled)` ("Canceled restore") and `Result.success()`.
   f. On any other `Exception`: log at ERROR, `notifier.showRestoreError(e.message)` (raw English message text), `Result.failure()`.
   g. `finally`: cancel notification id `-503` (`Notifications.ID_RESTORE_PROGRESS`).
5. `getForegroundInfo()` (`BackupRestoreJob.kt:63-73`): notification id `-503` built by `notifier.showRestoreProgress()`; foreground service type `DATA_SYNC` on API ≥ 29, else 0.

### 1.4 Restore core (OBSERVED, `BackupRestorer.kt`)
`restore(uri, options)` (`BackupRestorer.kt:173-189`):
1. `startTime = System.currentTimeMillis()`.
2. `restoreFromFile(uri, options)`.
3. `time = now - startTime`.
4. `logFile = writeErrorLog()` (§7).
5. `notifier.showRestoreComplete(time, errors.size, logFile.parent, logFile.name, isSync)` (§8).
If step 2 throws (decode failure or cancel), steps 3-5 are skipped: no error log, no completion notification (D17).

`restoreFromFile(uri, options)` (`BackupRestorer.kt:191-251`):
1. `backup = BackupDecoder(context).decode(uri)` (§3).
2. Source-name mappings (see D1): `mangaSourceMapping` is built from `backup.backupSources` (`:197-198`); the anime mapping `animeSourceMapping` is never assigned (`:196` writes `mangaSourceMapping`). Result: anime error messages always show the numeric source id.
3. Amount (`restoreAmount`), exactly as coded (`BackupRestorer.kt:200-220`):
   - `libraryEntries`: `+= backup.backupManga.size + backup.backupAnime.size` (seasons included).
   - `categories`: `+= 2` (one for anime categories, one for manga categories).
   - `appSettings`: `+= 1`.
   - `extensionStores`: `+= backup.backupAnimeExtensionStores.size + backup.backupMangaExtensionRepo.size`.
   - `customButtons`: `+= 1`.
   - `sourceSettings`: `+= 1`.
   - `extensions`: `+= 1`.
4. Sections run inside one `coroutineScope { }` as `launch` children, in this textual order (`:222-250`): categories, app settings, source settings, library (anime then manga), extension stores, custom buttons, extensions. There is no join between them, so the coroutines run concurrently on the worker's dispatcher (D3).
5. Per-section arguments:
   - categories: `restoreCategories(backupAnimeCategories = backup.backupAnimeCategories, backupMangaCategories = backup.backupCategories)`.
   - app settings: `restoreAppPreferences(backup.backupPreferences, backup.backupCategories.takeIf { options.categories })`. When categories is off the category list is null, so category-keyed prefs are skipped (§5.3).
   - source settings: `restoreSourcePreferences(backup.backupSourcePreferences)`.
   - library: `restoreAnime(backup.backupAnime, if (options.categories) backup.backupAnimeCategories else emptyList())`; `restoreManga(backup.backupManga, if (options.categories) backup.backupCategories else emptyList())`.
   - extension stores: `restoreExtensionStores(backup.backupAnimeExtensionStores, backup.backupMangaExtensionRepo)`.
   - custom buttons: `restoreCustomButtons(backup.backupCustomButton)`.
   - extensions: `restoreExtensions(backup.backupExtensions)`.
6. A source comment (`:249`) records an unimplemented optional online library and tracker update: no online refresh happens after restore.

Per-section progress: each section increments `restoreProgress` by 1 after it finishes (categories: once for both anime and manga together; app settings once; source settings once; anime and manga once per entry; extension stores once per store or repo; custom buttons once; extensions once). Each increment calls `notifier.showRestoreProgress(title, restoreProgress, restoreAmount, isSync)` where `title` is the entry title, or the section's label.

Progress titles per section (OBSERVED):
- categories: `MR.strings.categories` ("Categories")
- app settings: `MR.strings.app_settings` ("App settings")
- source settings: `MR.strings.source_settings` ("Source settings")
- anime and manga entries: the entry title
- anime extension stores: `MR.strings.extensionStores` ("Extension stores")
- manga extension repos: `MR.strings.extensionRepo_settings` ("Extension repos")
- custom buttons: `AYMR.strings.custom_button_settings` ("Custom buttons")
- extensions: `MR.strings.source_settings` ("Source settings") (copy-paste label, D24, keep)

D2 (OBSERVED, fix): with categories enabled, `restoreAmount` gets `+2` but only one progress increment happens, so the final progress is `restoreAmount - 1` and the bar never reaches 100 percent. Fix: increment by 2 in the categories section (or amount +1).

---

## 2. RestoreOptions

Data (OBSERVED, `RestoreOptions.kt:8-15`):

| Field | Default | Label key (English) | Bit index in BooleanArray | Notes |
|---|---|---|---|---|
| `libraryEntries` | true | `MR.strings.label_library` ("Library") | 0 | Anime and manga entries with chapters/episodes, tracks, history, excluded scanlators |
| `categories` | true | `MR.strings.categories` ("Categories") | 1 | Anime and manga categories + membership |
| `appSettings` | true | `MR.strings.app_settings` ("App settings") | 2 | Application preference keys |
| `extensionStores` | true | `MR.strings.extensionStores` ("Extension stores") | 3 | Anime extension stores and manga extension repos |
| `customButtons` | true | `AYMR.strings.custom_button_settings` ("Custom buttons") | 4 | Player custom buttons |
| `sourceSettings` | true | `MR.strings.source_settings` ("Source settings") | 5 | Per-source preference files |
| `extensions` | false | `MR.strings.label_extensions` ("Extensions") | 6 | APK payloads (Android only) |

- `asBooleanArray()` returns indices in the table order above.
- `fromBooleanArray(array)` reads `array[0]..array[6]` in the same order. A shorter array throws `ArrayIndexOutOfBoundsException` (inside `doWork`, which is caught as a generic failure).
- `canRestore()` = any field true. The UI disables the Restore action when false.
- The UI entry order (`RestoreOptions.options`) is the table order above.

Dependencies: none between options (unlike backup-create). All seven are independently toggleable on the restore screen.

PROPOSAL: keep the seven flags and their order. Store them as a struct, not a BooleanArray; keep a bit-array encoder only if the job needs a serialized form.

---

## 3. Decoding and validation (what restore depends on)

These files are owned by area:21-backup-format-and-create. Restore calls them in exactly these ways.

### 3.1 Decode (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt:95-129`)
1. Open the URI stream; `source.peek().require(2)` (throws `EOFException` for a file under 2 bytes; this exception is not wrapped; the job reports its raw message).
2. Read 2 bytes as a short `id1id2`:
   - `0x1f8b` (gzip magic): wrap with gzip source.
   - `0x7b7d` (`{}`), `0x7b22` (`{"`), `0x7b0a` (`{` newline): throw `IOException(MR.strings.invalid_backup_file_json)` ("JSON backup not supported").
   - Else: raw bytes.
3. `readByteArray()` (whole file in memory).
4. `BackupDetector.isLegacyBackup(bytes)`: decodes only fields 103 (repeated anime sources) and 500 (isLegacy, default true). Legacy iff `isLegacy && backupAnimeSources.isNotEmpty()`. A Mihon backup has no field 103, so not legacy. A new Aniyomi backup sets 500=false, so not legacy.
5. Legacy: decode `LegacyBackup` and convert with `toBackup()`. Else: decode `Backup`.
6. `SerializationException` (any protobuf decode error) becomes `IOException(MR.strings.invalid_backup_file_unknown)` ("Backup file is corrupted").

### 3.2 Validation (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupFileValidator.kt:23-71`)
1. Decode. Any exception is re-thrown as `IllegalStateException(e)`. Its `message` is `e.toString()`, e.g. `"java.io.IOException: Backup file is corrupted"` (D19).
2. Manga sources: `sources = backupSources.associate { sourceId to name }`. Missing = those where `mangaSourceManager.get(id) == null`. Each missing value: if `name.toLongOrNull()` is null, use the name; else `mangaSourceManager.getOrStub(id).toString()`. Then `distinct().sorted()`.
3. Anime sources: same with `backupAnimeSources` and `animeSourceManager`. Result list is manga-missing then anime-missing, concatenated, NOT re-sorted.
4. Trackers: `(backupAnime.flatMap{tracking} + backupManga.flatMap{tracking}).map{syncId}.distinct()`. Keep ids where `trackerManager.get(id.toLong())` exists and `!isLoggedIn`; map to `name`; `sorted()`.
5. Returns `Results(missingSources, missingTrackers)`.

Note: the validator does not check that the backup has any entries (`MR.strings.invalid_backup_file_missing_manga` is defined but unused, D20).

---

## 4. Restorer algorithms

### 4.1 Common: sort by new (OBSERVED, `MangaRestorer.kt:45-54` and `AnimeRestorer.kt:47-56`)
```
urlsBySource = SELECT source, url FROM mangas (or animes), grouped by source -> list of url
sorted = backupList.sortedWith(
    compareBy { it.url in urlsBySource[it.source].orEmpty() }   // false (not in DB) first
    .then(compareByDescending { it.lastModifiedAt })            // newer lastModifiedAt first
)
```
The lookup is by `(source, url)` in the entry table, not by id. Entries that already exist sort after new ones.

### 4.2 Categories (OBSERVED, `AnimeCategoriesRestorer.kt:171-196`, `MangaCategoriesRestorer.kt:213-238`)
Algorithm (identical for anime and manga; only the DB handle differs):
```
if backupCategories is empty: return (no preference write)
dbCategories = GetXCategories.await()             // all categories
dbByName = dbCategories.associateBy { name }      // last wins on duplicate names
nextOrder = (dbCategories.maxOf order) + 1, or 0 when empty
created = backupCategories.sortedBy { order }.map {
    existing = dbByName[it.name]; if existing != null -> existing   (flags of the DB row kept)
    order = nextOrder++
    id = INSERT INTO categories(name, order, flags) VALUES (it.name, order, it.flags); selectLastInsertedRowId
    Category(id, name=it.name, flags=it.flags, order=order, hidden=false)
}
libraryPreferences.categorizedDisplaySettings = (dbCategories + created).distinctBy{flags}.size > 1
```
Notes:
- Matching is by exact name. Category flags of an existing DB row are never updated from the backup.
- New categories get `order = max + 1, +2, ...` in backup-order; backup `order` values are not kept.
- The categorized-display flag is a heuristic (D23, keep): true iff existing plus new categories have at least two distinct `flags` values. The list double-counts existing rows, which does not change the result.
- Preference key: `categorized_display` (boolean, default false) in `LibraryPreferences` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:70`).

The mapping from backup category to DB category used by entries is by `order` (see §4.4 step 4), so this step must finish before any membership is written (D3).

### 4.3 Manga entry (OBSERVED, `MangaRestorer.kt:56-79`)
`restore(backupManga, backupCategories)` runs inside `handler.await(inTransaction = true)`: one database transaction per manga (all its chapters, categories, history, tracks, scanlators commit together, or all roll back). Steps:
1. `dbManga = findExistingManga` = `SELECT * FROM mangas WHERE url = :url AND source = :source LIMIT 1` (`data/src/main/sqldelight/data/mangas.sq:70-74`).
2. `manga = backupManga.getMangaImpl()` (field mapping in p1-data-and-config.md §3).
3. If `dbManga == null`: `restoredManga = restoreNewManga(manga)`. Else `restoredManga = restoreExistingManga(manga, dbManga)`.
4. `restoreMangaDetails(restoredManga, chapters, categories, backupCategories, history, tracking, excludedScanlators)`:
   1. `restoreCategories` (§4.5)
   2. `restoreChapters` (§4.6)
   3. `restoreTracking` (§4.8)
   4. `restoreHistory` (§4.7)
   5. `restoreExcludedScanlators` (§4.9)
   6. `updateManga.awaitUpdateFetchInterval(manga, now, currentFetchWindow)` (§4.10)

Errors thrown inside the transaction roll the whole manga back. The caller (`BackupRestorer.restoreManga`) catches `Exception` per entry and records an error (§7).

#### 4.3.1 Existing manga merge (OBSERVED, `MangaRestorer.kt:85-105`)
```
restoreExistingManga(manga /*backup*/, dbManga):
  if manga.version > dbManga.version:
      updateManga( dbManga.copyFrom(manga).copy(id = dbManga.id) )
  else:
      updateManga( manga.copyFrom(dbManga).copy(id = dbManga.id) )

Manga.copyFrom(newer):
  favorite      = this.favorite || newer.favorite
  author        = newer.author
  artist        = newer.artist
  description   = newer.description
  genre         = newer.genre
  thumbnailUrl  = newer.thumbnailUrl
  status        = newer.status
  initialized   = this.initialized || newer.initialized
  version       = newer.version
  (all other fields: taken from `this`)
```
Consequence (D5, keep for parity, PROPOSAL to decide): the "newer" side wins for metadata only (author, artist, description, genre, thumbnail, status, version). Fields not in `copyFrom` (title, viewerFlags, chapterFlags, updateStrategy, dateAdded, lastUpdate, coverLastModified) come from `this`:
- backup is newer: the DB row keeps its own title, viewer, chapter flags, update strategy, date added, last update.
- backup is older or equal: the backup values for those fields overwrite the DB row.

`favorite` is always the OR, so a restore never removes a library entry.

#### 4.3.2 New manga insert (OBSERVED, `MangaRestorer.kt:137-145,241-267`)
`restoreNewManga(manga)` returns `manga.copy(initialized = manga.description != null, id = insertManga(manga), version = manga.version)`. The `insertManga` argument is the original, uncopied `manga`, so the `initialized` value written to the row is the backup value (always `false` for Mihon/Aniyomi backups, since `BackupManga` has no initialized field and `Manga.create()` sets `initialized = false`; `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt`). The returned object carries the computed value, but that value is not persisted (D6, fix).

Insert SQL: `INSERT` into `mangas` with the 21 named args of `mangasQueries.insert(...)` (`MangaRestorer.kt:243-264`), `nextUpdate = 0`, `calculateInterval = 0`, `updateStrategy` passed as the enum object (encoded by the column adapter), then `SELECT last_insert_rowid()` (`selectLastInsertedRowId`).

#### 4.3.3 Manga row update (OBSERVED, `MangaRestorer.kt:107-135` and `mangas.sq:156-179`)
`updateManga(manga)` calls `mangasQueries.update(...)` with `isSyncing = 1`. Every column in the SQL is `coalesce(:param, column)`, so a null argument keeps the existing value. `nextUpdate = null` and `calculateInterval = null` keep the existing values here. `genre` is written as `joinToString(", ")` of the list (null when null). `viewer` gets `viewerFlags`; `chapter_flags` gets `chapterFlags`; `update_strategy` is encoded by `MangaUpdateStrategyColumnAdapter.encode`.

#### 4.3.4 Manga categories (OBSERVED, `MangaRestorer.kt:293-319`)
```
dbCategories = GetMangaCategories.await()
dbByName = dbCategories.associateBy { name }
backupByOrder = backupCategories.associateBy { order }
pairs = entry.categories (List<Long> of backup category order values)
   .mapNotNull { o -> backupByOrder[o]?.let { bc -> dbByName[bc.name]?.let { dc -> (manga.id, dc.id) } } }
if pairs not empty (inside one transaction):
    DELETE FROM mangas_categories WHERE manga_id = :mangaId          (all memberships cleared)
    for each pair: INSERT INTO mangas_categories(manga_id, category_id)
```
If `pairs` is empty (for example because categories are off, or none match), memberships are left untouched. Unknown categories are silently dropped.

#### 4.3.5 Chapters (OBSERVED, `MangaRestorer.kt:147-234`)
```
dbByUrl = GetChaptersByMangaId(manga.id).associateBy { url }    // all chapters of this manga
for each backupChapter b:
    c = b.toChapterImpl().copy(mangaId = manga.id)               // id = -1
    d = dbByUrl[c.url]
    if d == null: keep c as NEW (id -1 -> insert)
    else if c.forComparison() == d.forComparison(): SKIP (no write)
    else:
        u = c.copyFrom(d).copy(id = d.id, bookmark = c.bookmark || d.bookmark)
        if d.read && !u.read:          u = u.copy(read = true, lastPageRead = d.lastPageRead)
        else if u.lastPageRead == 0 && d.lastPageRead != 0:
                                       u = u.copy(lastPageRead = d.lastPageRead)
        keep u as EXISTING (id > 0 -> update)
insert NEW chapters; update EXISTING chapters
```
`Chapter.forComparison()` (`MangaRestorer.kt:189-190`) = `copy(id=0, mangaId=0, dateFetch=0, dateUpload=0, lastModifiedAt=0, version=0)`; the comparison is full structural equality on all other fields, including name, url, chapterNumber, scanlator, read, bookmark, lastPageRead, sourceOrder.

`Chapter.copyFrom(other)` (`domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt:22-30`): takes `name`, `url`, `dateUpload`, `chapterNumber`, `scanlator` (blank becomes null) from `other` (the DB row). So for an existing chapter, the DB row's name, number, upload date and scanlator win over the backup; the backup's values for those fields are ignored. Other fields (sourceOrder, lastModifiedAt, version, dateFetch, read, lastPageRead, bookmark) come from the backup object before the rules above.

Insert (`insertNewChapters`, `MangaRestorer.kt:192-211`): `chaptersQueries.insert(mangaId, url, name, scanlator, read, bookmark, lastPageRead, chapterNumber, sourceOrder, dateFetch, dateUpload, version)`.

Update (`updateExistingChapters`, `MangaRestorer.kt:213-234`): `chaptersQueries.update(mangaId=null, url=null, name=null, scanlator=null, read, bookmark, lastPageRead, chapterNumber=null, sourceOrder=null, dateFetch=null, dateUpload=null, chapterId=id, version, isSyncing=0)`. Only read, bookmark, lastPageRead and version change on existing rows.

Regression (D8, keep for parity, PROPOSAL: take max): when both sides have a non-zero `lastPageRead` and the backup value is smaller, the backup value overwrites the DB value.

#### 4.3.6 History (OBSERVED, `MangaRestorer.kt:321-359`)
For each `BackupHistory h` (url, lastRead, readDuration):
```
dbH = historyQueries.getHistoryByChapterUrl(h.url)     // query at data/src/main/sqldelight/data/history.sq:25-31, matches history rows by chapter url
item = MangaHistory(readAt = Date(h.lastRead), readDuration = h.readDuration)
if dbH == null:
    ch = SELECT * FROM chapters WHERE url = :url   (getChapterByUrl: GLOBAL, not scoped to manga; D9)
    if ch == null: SKIP entry
    else: item.chapterId = ch._id                  // new history row
else:
    item.id = dbH._id; item.chapterId = dbH.chapter_id
    item.readAt = max(item.readAt, dbH.last_read) if > 0 else null
    item.readDuration = max(item.readDuration, dbH.time_read) - dbH.time_read   // delta
upsert all items:
    INSERT INTO history(chapter_id, last_read, time_read) VALUES (chapterId, readAt, time_read)
    ON CONFLICT(chapter_id) DO UPDATE SET last_read = :readAt, time_read = time_read + :time_read
    WHERE chapter_id = :chapterId
```
Effect: `time_read` ends up as `max(backup, db)`; `last_read` is the later of the two timestamps (or null when both are zero, which sets the column to NULL on conflict). Source: `data/src/main/sqldelight/data/history.sq:61-69`.

D9 (keep/PROPOSAL fix): `getChapterByUrl` ignores the manga, so two manga with a chapter of the same URL can attach history to the wrong chapter. PROPOSAL: resolve the chapter by `(manga_id, url)` through the current manga's chapter map built in §4.3.5.

#### 4.3.7 Tracks (OBSERVED, `MangaRestorer.kt:361-413`)
```
dbByTracker = GetMangaTracks(manga.id).associateBy { trackerId }
for each BackupTracking t:
    track = t.getTrackImpl()        // id=-1, mangaId=-1, trackerId=syncId, remoteId = mediaIdInt if != 0 else mediaId
    d = dbByTracker[track.trackerId]
    if d == null: NEW = track.copy(id = 0, mangaId = manga.id)
    else if track.forComparison() == d.forComparison(): SKIP
    else: EXISTING = d.copy(remoteId = track.remoteId, libraryId = track.libraryId,
                            lastChapterRead = max(d.lastChapterRead, track.lastChapterRead))
insert NEW via InsertMangaTrack.awaitAll
update EXISTING: manga_sync UPDATE with d's title, totalChapters, status, score, remoteUrl, startDate, finishDate, private (the DB values), plus remoteId, libraryId, lastChapterRead from the merged object
```
Important (D: observed merge scope): for an existing track only `remoteId`, `libraryId` and `lastChapterRead` (max) come from the backup. `status`, `score`, `startDate`, `finishDate`, `title`, `totalChapters`, `private`, `remoteUrl` keep the DB values.

`MangaTrack.forComparison() = copy(id = 0, mangaId = 0)` (`MangaRestorer.kt:415`).

#### 4.3.8 Excluded scanlators (OBSERVED, `MangaRestorer.kt:423-436`)
If the backup list is empty: return. Else `existing = SELECT scanlator FROM excluded_scanlators WHERE manga_id = :mangaId`; insert `s` for each backup `s` not in `existing` (`INSERT INTO excluded_scanlators(manga_id, scanlator)`). Duplicates in the backup list are inserted once each unless the list repeats them, which the table may reject (no unique constraint in the query; behaviour as the DB enforces).

#### 4.3.9 Fetch interval (OBSERVED, `MangaRestorer.kt:37-43,283`; `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt`)
- `now` and `currentFetchWindow = getWindow(now)` are captured when the restorer object is created, once per restore run. `getWindow(now)`: local midnight of `now`'s day, minus 1 day (lower) and plus 1 day (upper), as epoch milliseconds: `lower = start_of_today - 1d`, `upper = start_of_today + 1d - 1ms`.
- `awaitUpdateFetchInterval(manga, now, window)` computes `interval = manga.fetchInterval if < 0, else calculateInterval(chapters, zone)` (chapters from `getChaptersByMangaId(id, applyScanlatorFilter = true)`), then `nextUpdate` (kept if already inside the window; else the next cycle), and writes `nextUpdate` and `fetchInterval` only (`UpdateManga.kt:79-87`).
- `calculateInterval` (`MangaFetchInterval.kt:43-87`): `chapterWindow = 3 if chapters.size <= 8 else 10`; upload dates (distinct local dates, newest first, `take(chapterWindow)`) when `>= 3`: `interval = floorDiv(delta(last..first) days, indexOf(last))`; else fetch dates likewise; else 7. Then `coerceIn(1, MAX_INTERVAL = 28)`.
- Anime does the same with `AnimeFetchInterval` (`domain/src/main/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchInterval.kt`).
- Anime seasons get the interval only at their top-level turn (their own `restoreAnimeDetails`); nested season rows do not (see p1-topic-seasons-and-legacy.md §1.3).

### 4.4 Anime entry (OBSERVED, `AnimeRestorer.kt:58-93`)
Same structure as manga, with these differences.

#### 4.4.1 Lookup and merge
- Lookup: `getAnimeByUrlAndSourceId` (same shape as manga, against `animes`).
- `restoreExistingAnime(anime, dbAnime)`:
```
if anime.version > dbAnime.version:
    updateAnime( dbAnime.copyFrom(anime).copy(id = dbAnime.id, parentId = anime.parentId) )
else:
    updateAnime( anime.copyFrom(dbAnime).copy(id = dbAnime.id, parentId = anime.parentId) )
Anime.copyFrom(newer) = same as Manga.copyFrom plus:
    fetchType = newer.fetchType
    parentId  = newer.parentId
```
`parentId` is then forced to `anime.parentId` (the backup's raw value) in both branches (D7).

- `updateAnime` writes `animesQueries.update(...)` with `isSyncing = 1`, `fetchType` (encoded `FetchTypeColumnAdapter.encode`), `parentId`, `seasonFlags`, `seasonNumber`, `seasonSourceOrder`, `backgroundUrl`, `backgroundLastModified`, `memo` (encoded `MemoColumnAdapter`), `episodeFlags`, `viewer`. All other parameters are coalesce-null.
- `insertAnime` writes all of the above with `nextUpdate = 0`, `calculateInterval = 0`, and the `initialized` value of the original (uncopied) anime (same defect as D6).

#### 4.4.2 Seasons (OBSERVED, `AnimeRestorer.kt:72-82`; `BackupRestorer.kt:270-289`)
- `BackupRestorer.restoreAnime` iterates the full `backupAnimes` (after `sortByNew`), including season rows (rows with `parentId != null`). For each row `it` it computes `seasons = backupAnimes.filter { s.parentId == it.id }`, then calls `restore(it, categories, seasons)`.
- Inside `restore`, each season `bs` is restored as `bs.getAnimeImpl().copy(parentId = restoredAnime.id)` (the DB id of the parent). This is correct.
- But the season row is also processed as its own top-level entry in the same loop. That top-level pass uses `bs.getAnimeImpl()` whose `parentId` is the backup-side parent id (the source device's DB id), and `restoreExistingAnime` writes that raw value (D7).
- Result depends on order: if the parent is processed before the season's own top-level turn, the season's parent pointer ends up as the raw source id. If the season is processed first, the parent pass later rewrites it correctly.

PROPOSAL (fix D7): build `backupId -> dbId` while restoring parents; skip top-level rows that have a non-null backup `parentId` (they are restored under their parent); remap `parentId` through the map.

#### 4.4.3 Episodes (OBSERVED, `AnimeRestorer.kt:171-269`)
Same algorithm as chapters (§4.3.5) with these field names and rules:
- `forComparison()` = `copy(id=0, animeId=0, dateFetch=0, dateUpload=0, lastModifiedAt=0, version=0)`.
- Existing-episode merge:
```
u = episode.copyFrom(d).copy(id = d.id, bookmark = episode.bookmark || d.bookmark, fillermark = episode.fillermark || d.fillermark)
if d.seen && !u.seen:             u = u.copy(seen = true, lastSecondSeen = d.lastSecondSeen)
else if u.lastSecondSeen == 0 && d.lastSecondSeen != 0:
                                  u = u.copy(lastSecondSeen = d.lastSecondSeen)
```
- `Episode.copyFrom(other)` (`domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt:30-41`) takes `name`, `url`, `dateUpload`, `episodeNumber`, `fillermark`, `scanlator` (blank to null), `summary` (blank to null), `previewUrl` (blank to null) from the DB row.
- Insert: `episodesQueries.insert(animeId, url, name, scanlator, seen, bookmark, lastSecondSeen, totalSeconds, episodeNumber, sourceOrder, dateFetch, dateUpload, version, summary, previewUrl, fillermark, memo)`.
- Update: `episodesQueries.update(... seen, bookmark, fillermark, lastSecondSeen, totalSeconds, version, isSyncing = 0, memo)`; null for name, url, scanlator, summary, previewUrl, episodeNumber, sourceOrder, dateFetch, dateUpload, animeId.
- Regression (D8 for episodes): `lastSecondSeen` can drop when both sides are seen (backup smaller).

#### 4.4.4 Anime history (OBSERVED, `AnimeRestorer.kt:362-398`)
`BackupAnimeHistory(url, lastRead, readDuration)`: note `readDuration` is ignored for anime (`AnimeHistory` has only `seenAt`).
```
dbH = animehistoryQueries.getHistoryByEpisodeUrl(url)   // JOIN episodes ON url
item = AnimeHistory(seenAt = Date(lastRead))
if dbH == null:  ep = episodesQueries.getEpisodeByUrl(url)   // GLOBAL by url
                 skip if null, else item.episodeId = ep._id
else: item.id = dbH._id; item.episodeId = dbH.episode_id; item.seenAt = max(lastRead, dbH.last_seen) if > 0 else null
upsert: INSERT INTO animehistory(episode_id, last_seen) VALUES (:episodeId, :seenAt)
        ON CONFLICT(episode_id) DO UPDATE SET last_seen = :seenAt WHERE episode_id = :episodeId
```
Source: `data/src/main/sqldelightanime/dataanime/animehistory.sq:58-65`. Note: for anime, the existing row gets the max timestamp (not an additive time).

#### 4.4.5 Anime tracks (OBSERVED, `AnimeRestorer.kt:400-452`)
Identical to manga tracks (§4.3.7), with `lastEpisodeSeen` and `anime_sync`; existing rows merge only `remoteId`, `libraryId`, `lastEpisodeSeen = max`.

#### 4.4.6 Anime categories
Same as manga membership (§4.3.4) with `animes_categories`.

### 4.5 Manga and anime categories link (summary)
Both are matched by `order` (backup side) and then by `name` (DB side). See §4.2 and §4.3.4.

### 4.6 Custom buttons (OBSERVED, `CustomButtonRestorer.kt:38-66`)
```
if backup list empty: return
dbAll = GetCustomButtons.getAll()
dbByName = dbAll.associateBy { name }
nextSortIndex = max(sortIndex) + 1, or 0
dbHasFavorite = any db button isFavorite           // computed ONCE
for each backup b sorted by sortIndex:
    if dbByName[b.name] != null: skip (no update, no insert)
    sortIndex = nextSortIndex++
    INSERT custom_buttons(name, isFavorite = b.isFavorite && !dbHasFavorite, sortIndex, content, longPressContent, onStartup)
```
D10 (fix): because `dbHasFavorite` is never updated inside the loop, every backup button with `isFavorite = true` is inserted as favourite when the DB has none. PROPOSAL: track `hasFavorite` as a mutable flag set on the first insert.

Source columns: `custom_buttons.sq` in `data/src/main/sqldelightanime/dataanime/custom_buttons.sq`.

### 4.7 Anime and manga extension stores and repos
Anime extension store (OBSERVED, `AnimeExtensionStoreRestorer.kt:8-24`; `extension_store.sq:25-45`):
```
UPSERT extension_store(index_url, name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url)
  VALUES (indexUrl, name, badgeLabel ?: name, signingKey, contactWebsite, contactDiscord, isLegacy ?: true, extensionListUrl)
  ON CONFLICT(index_url) DO UPDATE SET name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url
```
Never errors on duplicates; overwrites the existing row's metadata. Store order is backup list order (no sort).

Manga extension repo (OBSERVED, `MangaExtensionRepoRestorer.kt:114-137`):
```
dbRepos = getAll()
urlExists = dbRepos by baseUrl; shaExists = dbRepos by signingKeyFingerprint
if urlExists != null && urlExists.fingerprint != backup.fingerprint: error("Already Exists with different signing key fingerprint")
else if shaExists != null: error("${shaExists.name} has the same signing key fingerprint")
else: INSERT extension_repos(base_url, name, short_name, website, signing_key_fingerprint)
```
D11 (fix): an identical repo (same URL and fingerprint) always hits the second branch because the DB row also matches the fingerprint, so re-restoring a repo always records an error. PROPOSAL: treat `urlExists` with the same fingerprint as success (no error, no insert).

Error strings (English, not localized; kept as in code):
- `Already Exists with different signing key fingerprint`
- `<name> has the same signing key fingerprint`

Each restore error is recorded as `"Error Adding Manga Repo: ${it.name} : ${e.message}"` (`BackupRestorer.kt:373`) or `"Error Adding Anime Repo: ..."` for anime stores (`BackupRestorer.kt:354`).

### 4.8 Extensions APK (OBSERVED, `ExtensionsRestorer.kt:82-99`) — see §10 (drop).

### 4.9 Preferences (OBSERVED, `PreferenceRestorer.kt`)
See §5.

---

## 5. Preference restore

### 5.1 Entry points
- `restoreApp(preferences, backupCategories)`: run `restorePreferences(preferences, preferenceStore, backupCategories)`, then `AnimeLibraryUpdateJob.setupTask(context)`, `MangaLibraryUpdateJob.setupTask(context)`, `BackupCreateJob.setupTask(context)` (`PreferenceRestorer.kt:35-48`). These re-arm the periodic jobs from the new preference values (§5.4).
- `restoreSource(preferences: List<BackupSourcePreferences>)`: for each source, `store = AndroidPreferenceStore(context, sourcePreferences(sourceKey))` (shared-preference file named by the source key, `"source_<sourceId>"`, e.g. `source_1234`), then `restorePreferences(it.prefs, store)` with `backupCategories = null` (`PreferenceRestorer.kt:50-55`).

### 5.2 Core loop (OBSERVED, `PreferenceRestorer.kt:57-126`)
```
allManga = if backupCategories != null then GetMangaCategories.await() else empty
allAnime = if backupCategories != null then GetAnimeCategories.await() else empty
mangaByName = allManga.associateBy { name }
animeByName = allAnime.associateBy { name }
backupById  = backupCategories?.associateBy { id.toString() }.orEmpty()   // keyed by SOURCE-device category id
prefs = store.getAll()        // snapshot taken once, before the loop
for (key, value) in toRestore:
    try:
        when value:
          Int -> if prefs[key] is Int? (absent or Int):
                    newValue = 
                      if key == "default_category":       backupById[value.toString()]?.let { mangaByName[it.name]?.id?.toInt() }
                      elif key == "default_anime_category": backupById[value.toString()]?.let { animeByName[it.name]?.id?.toInt() }
                      else value
                    newValue?.let { store.getInt(key).set(it) }        // null -> skip
          Long -> if prefs[key] is Long? : set
          Float -> if prefs[key] is Float? : set
          String -> if prefs[key] is String? : set
          Boolean -> if prefs[key] is Boolean? : set
          StringSet -> if prefs[key] is Set<*>? :
                         restored = restoreCategoriesPreference(...)
                         if !restored: store.getStringSet(key).set(value)     // whole-set replace
    catch Exception: Log.e("PreferenceRestorer", "Failed to restore preference <$key>", e); continue
```
Type gating: a key with a different existing type is silently skipped, never converted. An absent key is written.

Keys with special handling:
- `default_category` (manga default category, `LibraryPreferences.DEFAULT_MANGA_CATEGORY_PREF_KEY`) and `default_anime_category` (`DEFAULT_ANIME_CATEGORY_PREF_KEY`): the backup stores a source category id; mapped by id -> name -> new id. Unmapped: skipped (the existing default is kept). Defaults if absent: `-1` (`LibraryPreferences.kt:153-154`).

### 5.3 Category string-set preferences (OBSERVED, `PreferenceRestorer.kt:128-155`)
Applies to keys in `LibraryPreferences.categoryPreferenceKeys` plus `DownloadPreferences.categoryPreferenceKeys` (exact key list in p1-data-and-config.md §5). Algorithm:
```
if key not in categoryKeys: return false                        // falls through to whole-set replace
ids = value.flatMap { oldId ->
        listOf(
          backupById[oldId]?.name?.let { mangaByName[it]?.id?.toString() },
          backupById[oldId]?.name?.let { animeByName[it]?.id?.toString() })
      }.filterNotNull()
if ids not empty: store.getStringSet(key) += ids     // APPEND to the existing set (D12)
return true                                          // even when ids is empty -> no write
```
Consequences:
- With categories off (`backupCategories == null`), the map is empty, so ids are empty; nothing is written.
- Each old id yields up to two new ids (the same name in manga and anime tables), so the set can contain ids of both kinds.
- The append (`+=`) means repeated restores accumulate ids (D12, fix: replace the set with the mapped ids, or union then de-duplicate; PROPOSAL: replace).

### 5.4 Scheduled jobs re-armed after app-prefs restore (OBSERVED)
- `AnimeLibraryUpdateJob.setupTask` / `MangaLibraryUpdateJob.setupTask` (`app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt:452` and the manga equivalent): read `autoUpdateInterval` (hours); if > 0 enqueue a periodic job with network and charging constraints from `autoUpdateDeviceRestrictions`; else cancel.
- `BackupCreateJob.setupTask` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt:90-113`): read `backup_interval` (hours, default 12); if > 0 enqueue a periodic auto-backup with `requiresBatteryNotLow = true` and exponential backoff of 10 minutes; else cancel the `"BackupCreator"` unique work.

PROPOSAL: after a restore that touched app preferences, call the Linux scheduler's `rearm()` for library update and auto-backup timers (see p1-rust-design.md §9).

### 5.5 Security note (PROPOSAL)
Restore writes any key in the file with a matching type, including internal keys (for example `__APP_STATE_*`, which the code uses for app state such as `last_auto_backup_timestamp`). PROPOSAL: restore only keys known to the Kurobayomi preference schema (area:02-preferences-system); log and count unknown keys in the error log as "skipped".

---

## 6. Concurrency and transactions (OBSERVED)

- Every entry (one manga or one anime with seasons) is one database transaction: `handler.await(inTransaction = true)` (`MangaRestorer.kt:60`, `AnimeRestorer.kt:63`). Nested `handler.await(true)` calls inside reuse it. An exception rolls the whole entry back.
- Transactions run through `withMangaTransaction` / `withAnimeTransaction` on a transaction dispatcher; SQLite serializes writers (`data/src/main/java/tachiyomi/data/handlers/manga/MangaTransactionContext.kt:38-50`, `AndroidMangaDatabaseHandler.kt:17-22`).
- Sections (§1.4) run as concurrent `launch` children with no join (D3). Concurrency-sensitive dependencies:
  - category membership needs categories created first (`restoreCategories` must finish before `restoreAnime`/`restoreManga` use `getCategories`);
  - app-prefs default-category mapping needs categories first;
  - progress counters (`restoreProgress += 1`) are plain `var`s updated from several coroutines (benign on JVM for display, but not atomic).
- The coroutine dispatcher is `Dispatchers.Default` for `CoroutineWorker` (library default), and each `handler.await` moves to `Dispatchers.IO` or the transaction context.
- `ensureActive()` is called before each item and each section. Cancellation therefore takes effect at the next item boundary (D4 notes an exception to this).

PROPOSAL: run sections in order (categories, then app prefs and source prefs, then extension stores, custom buttons, extensions, then library entries). Run entries one after another on one DB writer thread. Check the cancel flag before each entry. This removes D3 at the cost of some speed (the restore is I/O bound on SQLite anyway).

---

## 7. Errors and the error log

### 7.1 What is collected (OBSERVED)
- Entry error (manga): `errors.add(Date() to "${title} [${sourceName}]: ${e.message}")` with `sourceName = mangaSourceMapping[source] ?: source.toString()` (`BackupRestorer.kt:299-303`).
- Entry error (anime): same format; `sourceName` is always the numeric source id because of D1 (`BackupRestorer.kt:281-283`).
- Season errors are part of the parent's error (the parent's transaction covers its seasons).
- Anime extension store: `"Error Adding Anime Repo: ${name} : ${e.message}"` (`BackupRestorer.kt:354`).
- Manga extension repo: `"Error Adding Manga Repo: ${name} : ${e.message}"` (`BackupRestorer.kt:373`).
- Errors thrown outside the per-entry try blocks (decode, unexpected exceptions in a section such as preference writes) are not collected; they abort the restore (§1.3 step 4f).
- Preference write errors are logged to logcat and not collected (`PreferenceRestorer.kt:122-124`).

### 7.2 Error log file (OBSERVED, `BackupRestorer.kt:412-429`)
- Written only when `errors.isNotEmpty()`.
- Path: `context.createFileInCacheDir("aniyomi_restore_error.txt")` (`ContextExtensions.kt:104-111`): in `externalCacheDir`; an existing file of that name is deleted first; then created.
- Line format: `"[" + SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.getDefault()).format(date) + "] " + message + "\n"`, one per error, in collection order.
- Any exception while writing is swallowed (empty catch, D15). The function then returns `File("")`, whose `parent` is null, so the completion notification has no "show errors" action.
- Only the latest error log exists; a new restore overwrites it.

### 7.3 Cancel and error-log interaction (OBSERVED)
Because `writeErrorLog` is skipped on exception, a cancelled restore never writes the log even if errors were collected (D17).

---

## 8. Notifications (OBSERVED)

Channels (`app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt:59-64,165-170`):
- `backup_restore_progress_channel` (importance LOW), used for progress.
- `backup_restore_complete_channel_v2` (importance HIGH), used for completion and errors.

Ids: progress `-503` (`ID_RESTORE_PROGRESS`), completion/error `-504` (`ID_RESTORE_COMPLETE`).

### 8.1 Progress notification (`BackupNotifier.kt:225-260`)
- Ongoing, not auto-cancel, only alert once, progress determinate: max = `restoreAmount`, progress = `restoreProgress`.
- Title: `MR.strings.restoring_backup` ("Restoring backup") normally; `MR.strings.syncing_library` ("Syncing library") when `isSync`.
- Text: the current item title, unless the preference `hideNotificationContent` (SecurityPreferences) is true, in which case no text. (Preference key is owned by area:02-preferences-system.)
- Action: `MR.strings.action_cancel` ("Cancel"), broadcast `NotificationReceiver.cancelRestorePendingBroadcast` for id `-503`, which calls `BackupRestoreJob.stop` (`NotificationReceiver.kt:261-262`).
- Re-posted (same id) on every progress increment; no throttle.

### 8.2 Completion notification (`BackupNotifier.kt:273-324`)
- Cancels the progress notification first.
- Title: `MR.strings.restore_completed` ("Restore completed") normally; `MR.strings.library_sync_complete` ("Library sync complete") when sync.
- Text: plural `restore_completed_message` with args `(timeString, errorCount)`:
  - `one`: "Done in %1$s with %2$s error"
  - `other`: "Done in %1$s with %2$s errors"
  - `%1$s` = `restore_duration` = "%1$02d min, %2$02d sec" with minutes and seconds from the elapsed milliseconds (minutes `= ms / 60000`, seconds `= ms/1000 - minutes*60`).
- If `errorCount > 0` and the log path and file name are non-empty: content intent and action `MR.strings.action_show_errors` ("Tap to see details") that opens the log file (`ACTION_VIEW`, MIME `text/plain`, `FileProvider` URI, `NotificationReceiver.kt:970-978`).

### 8.3 Failure notification (`BackupNotifier.kt:262-271`)
- Cancels the progress notification.
- Title: `MR.strings.restoring_backup_error` ("Restoring backup failed"), for both the cancel case and real failures.
- Text: the message passed in: "Canceled restore" (`restoring_backup_canceled`) for cancel; `e.message` (raw English) for failure; empty when `e.message` is null.

### 8.4 Other
- Foreground service notification on Android (`getForegroundInfo`): same as progress, with `DATA_SYNC` type (Android only; no Linux equivalent needed).

---

## 9. Create screen (in scope file CreateBackupScreen.kt; behaviour only as it interacts with restore)

Option table (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt`, `CreateBackupScreen.kt:90-106`):
- Card "Library" (`MR.strings.label_library`): `libraryEntries` ("Library entries", `AYMR.strings.entries`, default true); `chapters` ("Chapters and episodes", enabled iff `libraryEntries`, default true); `tracking` ("Tracking", `MR.strings.track`, enabled iff `libraryEntries`, default true); `history` ("History", enabled iff `libraryEntries`, default true); `categories` ("Categories", default true); `readEntries` ("All read/watched entries", `AYMR.strings.non_library_settings`, enabled iff `libraryEntries`, default true).
- Card "Settings" (`MR.strings.label_settings`): `appSettings` ("App settings", default true); `extensionStores` ("Extension stores", default true); `customButton` ("Custom buttons", default true); `sourceSettings` ("Source settings", default true); `privateSettings` ("Include sensitive settings (e.g., tracker login tokens)", `MR.strings.private_settings`, enabled iff `appSettings || sourceSettings`, default false).
- Card "Extensions" (`MR.strings.label_extensions`): `extensions` ("Extensions", default false).
- Create enabled iff `canCreate()` = `libraryEntries || categories || appSettings || extensionStores || customButton || sourceSettings` (note: `chapters`, `tracking`, `history`, `readEntries`, `privateSettings`, `extensions` alone do not enable it).

Click (`CreateBackupScreen.kt:72-82`):
- If `BackupCreateJob.isManualJobRunning` is false: open the save chooser with suggested name `BuildConfig.APPLICATION_ID + "_" + yyyy-MM-dd_HH-mm + ".tachibk"` (`BackupCreator.getFilename`, `BackupCreator.kt:232-235`). If no picker app exists (`ActivityNotFoundException`): toast `MR.strings.file_picker_error` ("No file picker app found").
- Else: toast `MR.strings.backup_in_progress` ("Backup is already in progress").
- Chooser result null (cancelled): nothing.
- Chooser result URI: take persistable read+write permission; `BackupCreateJob.startNow(context, uri, options)`; pop the screen.

Warning banner (Android MIUI only): `MR.strings.restore_miui_warning` ("Backup/restore may not function properly if MIUI Optimization is disabled.") shown when `DeviceUtil.isMiui && isMiuiOptimizationDisabled()` (drop on Linux, T-22a-039).

### 9.1 Shared text in Settings Data (out of scope but shown on the same screen)
- `MR.strings.backup_info`: "You should keep copies of backups in other places as well. Backups may contain sensitive data including any stored passwords; be careful if sharing."
- `MR.strings.last_auto_backup_info`: "Last automatically backed up: %s" with the relative time of `last_auto_backup_timestamp`.
- `MR.strings.pref_backup_interval` ("Automatic backup frequency") with entries 0 Off, 6 "Every 6 hours", 12 "Every 12 hours", 24 "Daily", 48 "Every 2 days", 168 "Weekly" (`SettingsDataScreen.kt:263-269`). Default 12.

---

## 10. Extensions (APK) restore — Android only (OBSERVED, `ExtensionsRestorer.kt:82-99`)
For each `BackupExtension(pkgName, apk)`: if `packageManager.getInstalledPackages(0)` has no package named `pkgName`, write `apk` to `cacheDir/<pkgName>.apk`, then start an `ACTION_VIEW` intent for the APK with MIME `application/vnd.android.package-archive` (installer dialog). No error is recorded if the user declines. Progress is counted once per section.

Drop decision: see p1-tasks T-22a-029. Kurobayomi extensions are not Android APKs (see area:20-extension-strategy-redesign, area:18/19). The user loses the ability to install extension payloads from a backup. The backup's APK entries are counted as "skipped" in the summary, not as errors.

---

## 11. Restore StorageInfo (in scope file StorageInfo.kt)

OBSERVED (`app/src/main/java/eu/kanade/presentation/more/settings/screen/data/StorageInfo.kt:173-223`; `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:18-70`):
- Roots: `ContextCompat.getExternalFilesDirs` non-null entries, each truncated at `"/Android/"`, kept only if the storage state is `MEDIA_MOUNTED` or `MEDIA_MOUNTED_READ_ONLY`.
- Per root: header = absolute path (`MaterialTheme.typography.header`); bar (12 dp high, clip to small shape, full width) with `progress = 1 - available / total.toFloat()`; caption `MR.strings.available_disk_space_info` = "Available: %1$s / Total: %2$s" with `Formatter.formatFileSize` for both values.
- `getTotalStorageSpace` and `getAvailableStorageSpace` return `-1` on any exception (`DiskUtil.kt:51-70`). With `total == -1` the bar value is not clamped (D14); `total == 0` gives infinity or NaN.

PROPOSAL: compute `used = (total - available) / total` only when `total > 0 && available >= 0`; else hide the bar and show only the text with "unknown". Clamp to 0.0..1.0. Format sizes with GLib's SI formatter (`glib::format_size`, decimal SI units, same as `Formatter.formatFileSize`).

---

## 12. Message keys used by this area (complete list)

Every key below appears in a file in scope. English text is taken from the base strings file cited.

| Key | English text | Where used (path:line) |
|---|---|---|
| `MR.strings.pref_restore_backup` | Restore backup | RestoreBackupScreen.kt:57; SettingsDataScreen.kt:253 |
| `MR.strings.action_restore` | Restore | RestoreBackupScreen.kt:65 |
| `MR.strings.file_select_backup` | Select backup file | SettingsDataScreen.kt:201 |
| `MR.strings.file_null_uri_error` | No file selected | SettingsDataScreen.kt:~200 |
| `MR.strings.restore_in_progress` | Restore is already in progress | SettingsDataScreen.kt:248 |
| `MR.strings.restore_miui_warning` | Backup/restore may not function properly if MIUI Optimization is disabled. | RestoreBackupScreen.kt:74; SettingsDataScreen.kt:242; CreateBackupScreen.kt:86 |
| `MR.strings.label_library` | Library | RestoreOptions.kt:469 |
| `MR.strings.categories` | Categories | RestoreOptions.kt:474; BackupRestorer.kt:263 |
| `MR.strings.app_settings` | App settings | RestoreOptions.kt:479; BackupRestorer.kt:323 |
| `MR.strings.extensionStores` | Extension stores | RestoreOptions.kt:484; BackupRestorer.kt:359 |
| `AYMR.strings.custom_button_settings` | Custom buttons | RestoreOptions.kt:489; BackupRestorer.kt:392 |
| `MR.strings.source_settings` | Source settings | RestoreOptions.kt:494; BackupRestorer.kt:336, 405 |
| `MR.strings.label_extensions` | Extensions | RestoreOptions.kt:499 |
| `MR.strings.extensionRepo_settings` | Extension repos | BackupRestorer.kt:378 |
| `MR.strings.backup_restore_content_full` | You may need to install any missing extensions and log in to tracking services afterwards to use them. | RestoreBackupScreen.kt:113 |
| `MR.strings.backup_restore_missing_sources` | Missing sources: | RestoreBackupScreen.kt:117 |
| `MR.strings.backup_restore_missing_trackers` | Trackers not logged into: | RestoreBackupScreen.kt:128 |
| `MR.strings.invalid_backup_file` | Invalid backup file: | RestoreBackupScreen.kt:140 |
| `MR.strings.invalid_backup_file_error` | Full error: | RestoreBackupScreen.kt:147 |
| `MR.strings.invalid_backup_file_json` | JSON backup not supported | BackupDecoder.kt:113 |
| `MR.strings.invalid_backup_file_unknown` | Backup file is corrupted | BackupDecoder.kt:126 |
| `MR.strings.invalid_backup_file_missing_manga` | Backup does not contain any library entries. | UNUSED (D20) |
| `MR.strings.restoring_backup` | Restoring backup | BackupNotifier.kt:235 |
| `MR.strings.syncing_library` | Syncing library | BackupNotifier.kt:233 (dead, isSync never true, D21) |
| `MR.strings.restoring_backup_error` | Restoring backup failed | BackupNotifier.kt:266 |
| `MR.strings.restoring_backup_canceled` | Canceled restore | BackupRestoreJob.kt:51 |
| `MR.strings.restore_completed` | Restore completed | BackupNotifier.kt:283 |
| `MR.strings.library_sync_complete` | Library sync complete | BackupNotifier.kt:281 (dead, D21) |
| `MR.strings.restore_duration` | %1$02d min, %2$02d sec | BackupNotifier.kt:289 |
| `R.plurals.restore_completed_message` | one: Done in %1$s with %2$s error; other: Done in %1$s with %2$s errors | BackupNotifier.kt:300 (plurals.xml base) |
| `MR.strings.action_show_errors` | Tap to see details | BackupNotifier.kt:317 |
| `MR.strings.action_cancel` | Cancel | BackupNotifier.kt:249 |
| `MR.strings.available_disk_space_info` | Available: %1$s / Total: %2$s | StorageInfo.kt:218 |
| `MR.strings.pref_create_backup` | Create backup | CreateBackupScreen.kt:62; SettingsDataScreen.kt:234 |
| `MR.strings.action_create` | Create | CreateBackupScreen.kt:70 |
| `MR.strings.file_picker_error` | No file picker app found | CreateBackupScreen.kt:77 |
| `MR.strings.backup_in_progress` | Backup is already in progress | CreateBackupScreen.kt:80 |
| `AYMR.strings.entries` | Library entries | BackupOptions.kt:48 |
| `AYMR.strings.chapters_episodes` | Chapters and episodes | BackupOptions.kt:53 |
| `MR.strings.track` | Tracking | BackupOptions.kt:59 |
| `MR.strings.history` | History | BackupOptions.kt:65 |
| `AYMR.strings.non_library_settings` | All read/watched entries | BackupOptions.kt:76 |
| `MR.strings.private_settings` | Include sensitive settings (e.g., tracker login tokens) | BackupOptions.kt:105 |
| `MR.strings.label_settings` | Settings | CreateBackupScreen.kt:97 |
| `MR.strings.backup_info` | (see §9.1) | SettingsDataScreen.kt:278 |
| `MR.strings.last_auto_backup_info` | Last automatically backed up: %s | SettingsDataScreen.kt:280 |
| `MR.strings.off` / `update_6hour` / `update_12hour` / `update_24hour` / `update_48hour` / `update_weekly` | Off / Every 6 hours / Every 12 hours / Daily / Every 2 days / Weekly | SettingsDataScreen.kt:264-269 |
| `MR.strings.pref_backup_interval` | Automatic backup frequency | SettingsDataScreen.kt:271 |
| `MR.strings.file_select_backup` | Select backup file | SettingsDataScreen.kt:201 |
| `MR.strings.creating_backup` / `creating_backup_error` / `backup_created` | (create side, area:21) | BackupNotifier.kt:186,200,211 |

Hard-coded English strings in the restore path (not in any resource, kept verbatim as parity; PROPOSAL to localize as keys in Kurobayomi):
- `"Error Adding Anime Repo: "`, `"Error Adding Manga Repo: "`, `" : "` separator (BackupRestorer.kt:354, 373).
- `"Already Exists with different signing key fingerprint"`, `"%s has the same signing key fingerprint"` (MangaExtensionRepoRestorer.kt:123,125).
- `"Not allowed to run on foreground service"` (log only, BackupRestoreJob.kt:43).
- Log tag `"PreferenceRestorer"` and message `"Failed to restore preference <$key>"` (log only).

---

## 13. Observed defects (complete list)

| ID | Location | Behaviour | Action |
|---|---|---|---|
| D1 | BackupRestorer.kt:196 | Anime source-name map written into `mangaSourceMapping`; `animeSourceMapping` never set. Anime errors show numeric source id. | fix |
| D2 | BackupRestorer.kt:204, 261 | Categories add 2 to the amount but progress increments once. Final progress is total minus 1. | fix |
| D3 | BackupRestorer.kt:222-250 | Sections are unjoined `launch` children. Categories race with entry membership and default-category mapping. | fix (sequence) |
| D4 | BackupRestorer.kt:281-283, 301-303 | `catch (e: Exception)` around per-entry restore also catches `CancellationException`: a cancel mid-entry is recorded as an entry error, and the entry is rolled back. | fix |
| D5 | MangaRestorer.kt:85-90, 93-105 | Asymmetric merge: DB keeps title, viewer, chapter flags, update strategy, date added, last update when the backup is newer; the backup overwrites them when it is older or equal. | keep (decide in PROPOSAL) |
| D6 | MangaRestorer.kt:137-145 (+ AnimeRestorer.kt:161-169) | New row persists backup `initialized` (false), not the computed value. | fix |
| D7 | AnimeRestorer.kt:101, 103; BackupRestorer.kt:270-289 | Seasons restored twice; the top-level pass forces raw source `parentId`, order-dependent. | fix |
| D8 | MangaRestorer.kt:171-180; AnimeRestorer.kt:196-205 | Progress (`lastPageRead` / `lastSecondSeen`) can move backwards when both sides are non-zero. | keep (PROPOSAL: max) |
| D9 | MangaRestorer.kt:327; AnimeRestorer.kt:368 | History chapter/episode lookup by URL is global, not scoped to the manga/anime. | fix |
| D10 | CustomButtonRestorer.kt:45, 54 | `dbHasFavorite` computed once; multiple backup favourites all become favourite. | fix |
| D11 | MangaExtensionRepoRestorer.kt:122-125 | Identical repo (same URL and fingerprint) always errors "has the same signing key fingerprint". | fix |
| D12 | PreferenceRestorer.kt:151 | Category string-set prefs are appended (`+=`), so repeated restores accumulate ids. | fix (replace) |
| D13 | RestoreBackupScreen.kt:197 | `e.message.toString()` yields the literal text `null` when the exception has no message. | fix |
| D14 | StorageInfo.kt:214 | Progress not clamped; `-1` sentinel from DiskUtil gives out-of-range or non-finite bar values. | fix |
| D15 | BackupRestorer.kt:425-427 | Error-log write failures are swallowed silently. | fix (log a warning) |
| D16 | MangaRestorer.kt:164-166 (chapter `copyFrom`); domain `Chapter.kt:22-30` | Backup chapter name, number, upload date and scanlator are ignored for existing chapters (DB wins). | keep (documented) |
| D17 | BackupRestorer.kt:180 | On cancel, the error log and completion notification are skipped even when errors were collected. | fix (write log in `finally`) |
| D18 | BackupRestorer.kt:270-289 | Season rows are restored once under their parent and once more as top-level entries. `restoreAmount` counts each row once, so progress stays consistent, but the work is done twice and the second pass overwrites the parent pointer (D7). | fix with D7 |
| D19 | BackupFileValidator.kt:26-27 | Validator error text is `"java.lang.IllegalStateException: ..."`, shown to the user. | fix (show the inner message) |
| D20 | i18n strings `invalid_backup_file_missing_manga` | Defined, unused. | keep unused or drop |
| D21 | BackupRestoreJob.kt:38; BackupRestorer.kt:187 | `sync` flag is never set true by any caller; sync titles are dead code. | drop (T-22a-045) |
| D22 | BackupModels (`BackupManga.favorite`, proto 100 default `true`) | Missing field decodes as favourite = true. | keep (decoder detail, area:21) |
| D23 | AnimeCategoriesRestorer.kt:190-194; MangaCategoriesRestorer.kt:232-236 | Categorized-display flag is a distinct-`flags` heuristic over db+new categories. | keep (documented) |
| D24 | BackupRestorer.kt:404-407 | Extension-section progress label reuses `MR.strings.source_settings`. | fix (use "Extensions") |

---

## 14. Sources outside scope read for this spec

- Restore dispatch and defaults: `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt` (`setupTask`, `startNow`, `isManualJobRunning`, lines 86-132).
- Notification ids and channels: `app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt:59-64,165-170`.
- Cancel broadcast: `app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt:95,257-262,385,970-993`.
- Settings entry: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt:191-283,305`.
- Storage helpers: `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:18-82`.
- Context helper: `app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:104-111`.
- WorkManager helpers: `app/src/main/java/eu/kanade/tachiyomi/util/system/WorkManagerExtensions.kt:11-17`.
- Device helpers: `core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt:13-40`.
- Preference-store snapshot: `core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt:63-65`.
- Shared-pref name for sources: `"source_$id"` (`app/src/main/java/eu/kanade/tachiyomi/source/manga/MangaSourceExtensions.kt:14`, `app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt:14`).
- Backup preferences: `domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt:10-17`.
- Library preferences: `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:70,153-154,413-424`.
- Download preferences: `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:76-88`.
- Domain defaults: `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:105-128`; `domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt:32-47`; `domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt:30-41,43-58`.
- Enums: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt:15,22`; `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt:15,22`; `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt:14,19`.
- Status constants: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:48-54`.
- JSON empty memo bytes: `core/common/src/main/java/mihon/core/common/extensions/JsonObject.kt:7` (`byteArrayOf(0x7B, 0x7D)`, i.e. `{}`).
- SQL: `data/src/main/sqldelight/data/mangas.sq:64-74,156-179`; `data/src/main/sqldelight/data/history.sq:25-69`; `data/src/main/sqldelight/data/chapters.sq:79-103`; `data/src/main/sqldelight/data/excluded_scanlators.sq:10-22`; `data/src/main/sqldelight/data/extension_repos.sq:20-45`; `data/src/main/sqldelightanime/dataanime/animes.sq:98-107`; `data/src/main/sqldelightanime/dataanime/episodes.sq:74-106`; `data/src/main/sqldelightanime/dataanime/animehistory.sq:23-65`; `data/src/main/sqldelightanime/dataanime/extension_store.sq:25-45`.
- Handler: `data/src/main/java/tachiyomi/data/handlers/manga/AndroidMangaDatabaseHandler.kt:17-105`; `data/src/main/java/tachiyomi/data/handlers/manga/MangaTransactionContext.kt:20-50`.
- Repository lookups: `data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt:28-36`; `domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetMangaByUrlAndSourceId.kt`.
- Detector and decoder dependencies: `app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDetector.kt:14-34`; `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/Backup.kt:24-62` (LegacyBackup and Backup field numbers).
- Backup filename: `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt:232-235`.
- Options: `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt:8-142`.
- Export of season ids: `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeBackupCreator.kt:97-98` (`parentId = this.parentId`, `id = this.id`).
- Spec mockup: `/home/blackbox/kurobayomi/spec/settings.json` (ids `settings_create_backup` line 2507, `settings_restore_backup` line 2604; the "Restore backup" action at line 2412 has no target).
