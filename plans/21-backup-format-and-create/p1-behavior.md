# Area 21: Backup format and create, behaviour spec

Source tree: `/home/blackbox/aniyomi-src`. OBSERVED = what the Kotlin does. PROPOSAL = what Kurobayomi should do.
Section ids `B-nn` are referenced from p1-inventory.md and p1-tasks.md.

---

## B-01 Entry points and job model

OBSERVED

| Trigger | Code | Job type | Input | Notes |
|---|---|---|---|---|
| Settings > Backup and restore > "Create backup", then Create, then file save picker | `CreateBackupScreen.kt` (`app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt`) calls `BackupCreateJob.startNow(context, uri, state.value.options)` | one-time, tag `BackupCreator:manual`, unique name `BackupCreator:manual`, `ExistingWorkPolicy.KEEP` | `is_auto_backup=false`, `location_uri=<SAF uri string>`, `options=<BooleanArray of 12>` | `KEEP` means a second manual request while one is queued/running is silently dropped by WorkManager. The screen prevents this with `isManualJobRunning` and a toast (see B-03). |
| Periodic timer | `BackupCreateJob.setupTask(context, prefInterval)` from `SettingsDataScreen.kt:273`, `EnableAutoBackupMigration.kt`, `SetupBackupCreateMigration.kt`, `PreferenceRestorer.kt:47` | periodic, unique name `BackupCreator` (`TAG_AUTO`), policy `UPDATE` | `is_auto_backup=true` only, no location (uses automatic dir), no options (defaults) | Runs only if interval > 0. |

Consequences:
- Automatic backups always use `BackupOptions()` defaults. `extensions` is false, `privateSettings` is false. Automatic backups never contain extension binaries or tracker tokens.
- Automatic and manual runs share `BackupCreator.backup`. The only branch on `isAutoBackup` is in B-06 and B-07, and in notifications (B-15) and in the `lastAutoBackupTimestamp` write (B-10).

---

## B-02 BackupOptions model

OBSERVED. `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt`

Fields and defaults (index in `asBooleanArray()` / `fromBooleanArray()` in brackets):

| Field | Default | Index | Gated in backup code by |
|---|---|---|---|
| `libraryEntries` | true | 0 | `backupMangas`/`backupAnimes` return empty when false (`BackupCreator.kt:173-183`) |
| `categories` | true | 1 | `backupMangaCategories`/`backupAnimeCategories` return empty when false (`:161-171`); per-entry category links in creators (`options.categories`) |
| `chapters` | true | 2 | `MangaBackupCreator.kt:37` and `AnimeBackupCreator.kt:33` (chapters/episodes) |
| `tracking` | true | 3 | `MangaBackupCreator.kt:58`, `AnimeBackupCreator.kt:53` |
| `history` | true | 4 | `MangaBackupCreator.kt:65`, `AnimeBackupCreator.kt:60` |
| `readEntries` | true | 5 | non-favourite read/watched entries fetched only when true (`BackupCreator.kt:101-111`) |
| `appSettings` | true | 6 | `backupAppPreferences` (`:192`) |
| `extensionStores` | true | 7 | `backupAnimeExtensionStores` (`:198`) AND `backupMangaExtensionRepos` (`:204`). One flag controls both. |
| `customButton` | true | 8 | `backupCustomButtons` (`:210`) |
| `sourceSettings` | true | 9 | `backupSourcePreferences` (`:216`) |
| `privateSettings` | false | 10 | include `__PRIVATE_` keys in app and source prefs (`withPrivatePreferences`) |
| `extensions` | false | 11 | `backupExtensions` (`:222`) |

`canCreate()` = `libraryEntries || categories || appSettings || extensionStores || customButton || sourceSettings`. It does NOT include `chapters`, `tracking`, `history`, `readEntries`, `privateSettings`, `extensions`. A selection of only `extensions` (or only `history`) cannot be created from the UI.

`asBooleanArray()` order = field declaration order (12 booleans). `fromBooleanArray(array)` reads indexes 0..11 unchecked (an array shorter than 12 throws `ArrayIndexOutOfBoundsException`). Only ever called with the array produced by `asBooleanArray()`.

Option label rows (UI order), from `BackupOptions.kt:46-118`:
- `libraryOptions`: `AYMR.strings.entries` ("Library entries") -> libraryEntries; `AYMR.strings.chapters_episodes` ("Chapters and episodes") -> chapters, enabled iff libraryEntries; `MR.strings.track` ("Tracking") -> tracking, enabled iff libraryEntries; `MR.strings.history` ("History") -> history, enabled iff libraryEntries; `MR.strings.categories` ("Categories") -> categories (always enabled); `AYMR.strings.non_library_settings` ("All read/watched entries") -> readEntries, enabled iff libraryEntries.
- `settingsOptions`: `MR.strings.app_settings` ("App settings"); `MR.strings.extensionStores` ("Extension stores"); `AYMR.strings.custom_button_settings` ("Custom buttons"); `MR.strings.source_settings` ("Source settings"); `MR.strings.private_settings` ("Include sensitive settings (e.g., tracker login tokens)") enabled iff `appSettings || sourceSettings`.
- `extensionOptions`: `MR.strings.label_extensions` ("Extensions").

Toggle rule: a setter changes only its own field (`copy(x = enabled)`). Turning `libraryEntries` off does NOT reset `chapters`/`tracking`/`history`/`readEntries`; they keep their values and are only disabled. This matters: re-enabling `libraryEntries` restores them.

---

## B-03 Create Backup screen rules (UI state, in scope because it owns the option model)

OBSERVED. `CreateBackupScreen.kt`.
- State is in memory only (`CreateBackupScreenModel.State(options = BackupOptions())`). Nothing is persisted between visits.
- Create button `actionEnabled = state.options.canCreate()`.
- Click: if `BackupCreateJob.isManualJobRunning(context)` is true, toast `MR.strings.backup_in_progress`. Otherwise open a save dialog `ActivityResultContracts.CreateDocument("application/*")` with suggested name `BackupCreator.getFilename()` (B-07).
- If no activity handles the picker (`ActivityNotFoundException`), toast `MR.strings.file_picker_error`.
- On a chosen URI: `takePersistableUriPermission(READ|WRITE)`, `BackupCreateJob.startNow(...)`, then `navigator.pop()`. The screen closes immediately; progress is shown only through the notification (B-15).
- On MIUI with optimisation disabled (`DeviceUtil.isMiui && DeviceUtil.isMiuiOptimizationDisabled()`), a warning banner `MR.strings.restore_miui_warning` is shown. Not applicable on Linux (see p1-ui-parity.md).

---

## B-04 Automatic backup scheduling (`BackupCreateJob.setupTask`)

OBSERVED. `BackupCreateJob.kt:90-114`.

Input: `prefInterval: Int?`. `interval = prefInterval ?: backupPreferences.backupInterval().get()`.
- If `interval > 0`:
  1. `Constraints(requiresBatteryNotLow = true)`.
  2. `PeriodicWorkRequestBuilder<BackupCreateJob>(interval.toLong(), TimeUnit.HOURS, 10, TimeUnit.MINUTES)` (repeat interval = `interval` hours; flex = 10 minutes).
  3. `.setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 10, TimeUnit.MINUTES)`.
  4. `.addTag("BackupCreator")`, `.setConstraints(...)`, `.setInputData(workDataOf("is_auto_backup" to true))`.
  5. `enqueueUniquePeriodicWork("BackupCreator", ExistingPeriodicWorkPolicy.UPDATE, request)`.
- If `interval == 0`: `cancelUniqueWork("BackupCreator")`.

Allowed UI values (`SettingsDataScreen.kt`, `getBackupAndRestoreGroup`): 0 "Off" (`MR.strings.off`), 6 "Every 6 hours" (`update_6hour`), 12 "Every 12 hours" (`update_12hour`), 24 "Daily" (`update_24hour`), 48 "Every 2 days" (`update_48hour`), 168 "Weekly" (`update_weekly`). Default value 12 (`BackupPreferences.kt:8`). The UI `onValueChanged` calls `setupTask(context, it)` immediately and returns true. Other integer values are accepted by the code but cannot be selected in the UI.

Migrations:
- `EnableAutoBackupMigration` (version `84f`): if the stored interval is 0, set it to 12 and call `setupTask(context)`.
- `SetupBackupCreateMigration` (version `Migration.ALWAYS`): call `setupTask(context)` on every start.
- `PreferenceRestorer.kt:47`: after restoring preferences, `setupTask(context)` is called again.

---

## B-05 Job body (`BackupCreateJob.doWork`)

OBSERVED, in this order (`BackupCreateJob.kt:39-66`):

1. `isAutoBackup = inputData.getBoolean("is_auto_backup", true)`. Absent key means automatic.
2. If `isAutoBackup && BackupRestoreJob.isRunning(context)`: return `Result.retry()`. (Restore and auto-backup never overlap; manual backup does not check this.)
3. `uri = inputData.getString("location_uri")?.toUri() ?: getAutomaticBackupLocation() ?: return Result.failure()`.
   `getAutomaticBackupLocation()` = `StorageManager.getAutomaticBackupsDirectory()?.uri`. That creates the `autobackup` directory under the base storage directory if missing (B-07).
4. `setForegroundSafely()`: calls `setForeground(getForegroundInfo())` then `delay(500)`. Catches `IllegalStateException` (not allowed to run foreground) and logs it.
5. `options = inputData.getBooleanArray("options")?.let { fromBooleanArray(it) } ?: BackupOptions()`.
6. `location = BackupCreator(context, isAutoBackup).backup(uri, options)` (B-06).
7. If not auto: `notifier.showBackupComplete(UniFile.fromUri(context, location.toUri())!!)`.
8. Return `Result.success()`.
9. On any `Exception`: `logcat(ERROR)`; if not auto: `notifier.showBackupError(e.message)`; return `Result.failure()`.
10. `finally`: cancel notification `ID_BACKUP_PROGRESS` (-501).

Edge: step 7 uses `!!`. A `NullPointerException` there is caught by step 9 and reported as "Backup failed" even though the file was written. (Observed; PROPOSAL: do not treat post-write notification failures as backup failures.)

Edge: `Result.failure()` for automatic runs is terminal for that run; there is no retry. The next periodic run happens after the interval. Failures of automatic runs produce no notification (only `logcat`).

`getForegroundInfo()`: `ForegroundInfo(ID_BACKUP_PROGRESS, notifier.showBackupProgress().build(), FOREGROUND_SERVICE_TYPE_DATA_SYNC on API >= 29 else 0)`.

`isManualJobRunning(context)` = WorkManager `isRunning("BackupCreator:manual")` = any work info with that tag in state RUNNING (`WorkManagerExtensions.kt:14-17`). Queued (ENQUEUED) manual jobs are not detected by this check.

---

## B-06 Backup pipeline (`BackupCreator.backup(uri, options)`)

OBSERVED. `BackupCreator.kt:79-159`. Wrapped in try/catch; on any exception: `logcat`, `file?.delete()`, rethrow.

1. Choose the target file:
   - Automatic: `dir = UniFile.fromUri(context, uri)`; prune (B-07); `file = dir?.createFile(getFilename())`.
   - Manual: `file = UniFile.fromUri(context, uri)`.
   - If `file == null || !file.isFile`: throw `IllegalStateException(MR.strings.create_backup_file_error)`.
2. Non-favourite read entries (only if `options.readEntries`):
   - anime: `animeRepository.getWatchedAnimeNotInLibrary()`
   - manga: `mangaRepository.getReadMangaNotInLibrary()`
   Otherwise empty.
3. `backupAnime = backupAnimes(getAnimeFavorites.await() + nonFavoriteAnime, options)` (empty when `!libraryEntries`).
4. `backupManga = backupMangas(getMangaFavorites.await() + nonFavoriteManga, options)`. Order of the two lists: favourites first, then non-favourites, in repository order (no sort in the creators).
5. Assemble `Backup`:
   - `backupManga` (1), `backupCategories` = manga categories (2), `backupSources` = `backupMangaSources(backupManga)` (101), `backupPreferences` = app prefs (104), `backupSourcePreferences` = source prefs (105), `backupMangaExtensionRepo` (106), `isLegacy = false` (500), `backupAnime` (501), `backupAnimeCategories` (502), `backupAnimeSources` = `backupAnimeSources(backupAnime)` (503), `backupExtensions` (504), `backupAnimeExtensionStores` (505), `backupCustomButton` (506).
6. `byteArray = parser.encodeToByteArray(Backup.serializer(), backup)`. If empty, throw `empty_backup_error` ("No library entries to back up"). Note: `isLegacy=false` is always encoded, so this cannot happen in practice.
7. Write: `file.openOutputStream()`, if it is a `FileOutputStream` truncate its channel to 0 ("Force overwrite old file"), wrap in `sink().gzip().buffer()`, write `byteArray`, close.
8. `fileUri = file.uri`. `BackupFileValidator(context).validate(fileUri)` (B-14). This is a decode round-trip. Its `Results` are ignored; only exceptions matter (decode failure throws and deletes the file).
9. If auto: `backupPreferences.lastAutoBackupTimestamp().set(Instant.now().toEpochMilli())` (B-10).
10. Return `fileUri.toString()`.

Creator gating summary (what each option controls, all reached through steps above):
- Manga entries (step 4): requires `libraryEntries`. Inside an entry: excluded scanlators are ALWAYS written (no option); chapters need `chapters`; category links need `categories`; tracking needs `tracking`; history needs `history`.
- Anime entries: same rules with episodes/seen.
- Categories: `categories`.
- Sources: derived from entries, no own option (empty when the entries are empty).
- App preferences: `appSettings`, private keys need `privateSettings` too.
- Source preferences: `sourceSettings`, private keys need `privateSettings`.
- Extension stores (anime) and repos (manga): `extensionStores`.
- Custom buttons: `customButton`.
- Extension binaries: `extensions`.

---

## B-07 File naming, directory and retention

OBSERVED.

- Suggested/manual filename: `getFilename()` = `"${APPLICATION_ID}_" + SimpleDateFormat("yyyy-MM-dd_HH-mm", Locale.ENGLISH).format(Date()) + ".tachibk"`. `APPLICATION_ID` = `xyz.jmir.tachiyomi.mi` (`app/build.gradle.kts:20`); debug and dev builds add suffixes (`.debug`, `.dev`). Local time zone (`Date()` default). Example: `xyz.jmir.tachiyomi.mi_2026-10-08_19-31.tachibk`.
- Automatic directory: `StorageManager.getAutomaticBackupsDirectory()` = `baseDir.createDirectory("autobackup")` (`domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt`). Returns null if the base storage directory does not exist.
- Automatic filename regex (`BackupCreator.kt:230`): `"""${APPLICATION_ID}_\d{4}-\d{2}-\d{2}_\d{2}-\d{2}.tachibk""".toRegex()`. The dots are unescaped (match any character). Only files matching this regex are considered for pruning.
- Retention (`MAX_AUTO_BACKUPS = 4`, `BackupCreator.kt:229`): list matching files, sort by name descending, drop the first `MAX_AUTO_BACKUPS - 1` = 3 (keep the 3 newest), delete the rest. Then create the new file. Net result after a successful run: 4 files.
- Because the name has minute resolution, two automatic runs in the same minute produce the same name. SAF `createFile` on an existing name typically yields a suffixed name (for example `name (1).tachibk`), which does not match the regex and would never be pruned. (Behaviour depends on the storage provider; treat as a known hazard.)
- Writing overwrites: if the file already exists the code truncates it (step B-06.7). This only happens for a manual target the user chose.

PROPOSAL (Kurobayomi):
- Filename `kurobayomi_<yyyy-MM-dd_HH-mm-ss>.tachibk` (seconds added to avoid collisions). Regex `^kurobayomi_\d{4}-\d{2}-\d{2}_\d{2}-\d{2}(-\d{2})?\.tachibk$` (accepts both forms). Escape the dot.
- Retention: create and fsync the new file FIRST (under a temporary name), then rename, then delete all matching files beyond the newest 4 (divergence from B-07 retention order, recorded as edge E-04 in p1-edge-cases-and-tests.md).
- Automatic directory: `$XDG_DATA_HOME/kurobayomi/autobackup` (default `~/.local/share/kurobayomi/autobackup`), configurable base directory.

---

## B-08 Data gathering per category (creators)

OBSERVED. Each creator is a separate class; they run inside the same coroutine, sequentially.

### B-08a Manga entry (`MangaBackupCreator.backupManga`, `MangaBackupCreator.kt:29-79`)

1. `toBackupManga()` (mapping, B-09). Fields copied from `Manga`:
   - `url, title, artist, author, description, genre = genre.orEmpty(), status = status.toInt(), thumbnailUrl, favorite, source, dateAdded`
   - `viewer = viewerFlags.toInt() and ReadingMode.MASK` (`ReadingMode.MASK = 0x00000007`, `ReadingMode.kt:60`)
   - `viewer_flags = viewerFlags.toInt()`
   - `chapterFlags = chapterFlags.toInt()`, `updateStrategy, lastModifiedAt, favoriteModifiedAt, version`
   - NOT copied: `chapters`, `categories`, `tracking`, `history`, `excludedScanlators` (set below).
2. `excludedScanlators = handler.awaitList { excluded_scanlatorsQueries.getExcludedScanlatorsByMangaId(manga.id) }` (always, no option). SQL: `SELECT scanlator FROM excluded_scanlators WHERE manga_id = :mangaId;` (`data/src/main/sqldelight/data/excluded_scanlators.sq:19-22`).
3. If `chapters`: `chaptersQueries.getChaptersByMangaId(mangaId, applyScanlatorFilter = 0, mapper = backupChapterMapper)`. `applyScanlatorFilter = 0` means ALL chapters, including those from excluded scanlators. SQL in `data/src/main/sqldelight/data/chapters.sq:56-69`: `SELECT C.* FROM chapters C LEFT JOIN excluded_scanlators ES ON C.manga_id = ES.manga_id AND C.scanlator = ES.scanlator WHERE C.manga_id = :mangaId AND (:applyScanlatorFilter = 0 OR ES.scanlator IS NULL)`. The list is set only when non-empty.
4. If `categories`: `getCategories.await(manga.id)` (manga categories for this manga). If non-empty, `categories = list.map { it.order }`. The link is stored as the category ORDER, not the id.
5. If `tracking`: `manga_syncQueries.getTracksByMangaId(manga.id, backupMangaTrackMapper)`. SQL: `SELECT * FROM manga_sync WHERE manga_id = :mangaId;`. Set only when non-empty.
6. If `history`: `getHistory.await(manga.id)` (`MangaHistory` rows for chapters of this manga, SQL `data/src/main/sqldelight/data/history.sq:14-23`: join `history` to `chapters` where `C.manga_id = :mangaId`). For each row: `chapter = chaptersQueries.getChapterById(history.chapterId)` (`SELECT * FROM chapters WHERE _id = :id`), then `BackupHistory(url = chapter.url, lastRead = history.readAt?.time ?: 0L, readDuration = history.readDuration)`. Set only when non-empty.

### B-08b Anime entry (`AnimeBackupCreator.backupAnime`, `AnimeBackupCreator.kt:29-74`)

Same structure:
1. `toBackupAnime()` (B-09). Fields copied: `url, title, artist, author, description, genre = genre.orEmpty(), status = status.toInt(), thumbnailUrl, favorite, source, dateAdded, viewer_flags = viewerFlags.toInt(), episodeFlags = episodeFlags.toInt(), updateStrategy, lastModifiedAt, favoriteModifiedAt, version, fetchType, parentId, id, seasonFlags, seasonNumber, seasonSourceOrder, memo = MemoColumnAdapter.encode(memo)`.
   - NOT copied: `backgroundUrl` (always null in backups even though the restore side reads it). Observed gap. PROPOSAL: copy it (Kurobayomi has backgrounds only if area 34 keeps them; otherwise write null explicitly).
   - Anime has no `viewer` (legacy) field: only `viewer_flags`.
2. If `chapters`: `episodesQueries.getEpisodesByAnimeId(animeId, backupEpisodeMapper)`; SQL `SELECT * FROM episodes WHERE anime_id = :animeId;` (`data/src/main/sqldelightanime/dataanime/episodes.sq:62-64`). Set only when non-empty.
3. If `categories`: `getCategories.await(anime.id)` -> `categories = map { it.order }` when non-empty.
4. If `tracking`: `anime_syncQueries.getTracksByAnimeId(anime.id, backupAnimeTrackMapper)`; SQL `SELECT * FROM anime_sync WHERE anime_id = :animeId;`.
5. If `history`: `getHistory.await(anime.id)`; per row `episode = episodesQueries.getEpisodeById(history.episodeId)` (`SELECT * FROM episodes WHERE _id = :id`), then `BackupAnimeHistory(url = episode.url, lastRead = history.seenAt?.time ?: 0L)`. `readDuration` is left at its default 0 (anime history has no duration in the creator).

Anime query list (`AnimeBackupCreator` input): favourites via `GetAnimeFavorites.await()` (`favorite = 1` rows, repository order) plus `getWatchedAnimeNotInLibrary()` SQL `data/src/main/sqldelightanime/dataanime/animes.sq:92-99`:
`SELECT * FROM animes WHERE favorite = 0 AND _id IN (SELECT DISTINCT episodes.anime_id FROM episodes WHERE seen = 1 OR last_second_seen != 0);`

Manga non-favourite query `data/src/main/sqldelight/data/mangas.sq:81-88`:
`SELECT * FROM mangas WHERE favorite = 0 AND _id IN (SELECT DISTINCT chapters.manga_id FROM chapters WHERE read = 1 OR last_page_read != 0);`

Note: non-favourites are fetched whenever `readEntries` is true, but are dropped later if `libraryEntries` is false (the list goes through `backupAnimes`/`backupMangas` which gate on `libraryEntries`). PROPOSAL: skip the query in that case.

### B-08c Categories (`MangaCategoriesBackupCreator`, `AnimeCategoriesBackupCreator`)

`getMangaCategories.await()` / `getAnimeCategories.await()` filtered with `filterNot(Category::isSystemCategory)` (`isSystemCategory = id == 0L`, `Category.kt:13`), then mapped with `backupCategoryMapper` (id, name, order, flags). The default "Uncategorized" (id 0) is never backed up.

### B-08d Sources (`MangaSourcesBackupCreator`, `AnimeSourcesBackupCreator`)

Input: the entries of the backup (not all sources). `asSequence().map(source).distinct().map(getOrStub).map(toBackupSource).toList()`. `distinct()` keeps first-seen order. `getOrStub(id)` returns the installed source or a stub (B-14). Output `BackupSource(name = source.name, sourceId = source.id)`. For a stub the name is `""` when the source record is unknown (`StubAnimeSource(id, lang="", name="")` from `AndroidAnimeSourceManager.createStubSource`, line 125).

### B-08e Extension stores and repos

- Anime: `GetAnimeExtensionStores.get()` (`mihon.domain.extension.anime.interactor`), mapped by `backupAnimeExtensionStoreMapper`: `indexUrl, name, badgeLabel, signingKey, contactWebsite = contact.website, contactDiscord = contact.discord, isLegacy, extensionListUrl`. SQL source table `data/src/main/sqldelightanime/dataanime/extension_store.sq` (`getAll: SELECT * FROM extension_store;`).
- Manga: `GetMangaExtensionRepo.getAll()` mapped by `backupExtensionReposMapper`: `baseUrl, name, shortName, website, signingKeyFingerprint`. SQL source `data/src/main/sqldelight/data/extension_repos.sq`.

No ordering is applied; order is the repository order.

### B-08f Custom buttons

`GetCustomButtons.getAll()` mapped with `backupCustomButtonsMapper`: `name, isFavorite, sortIndex, content, longPressContent, onStartup`. The row `id` is not backed up (restore assigns new ids).

### B-08g Installed extensions (opt-in, `extensions`)

For each installed anime extension (`animeExtensionManager.installedExtensionsFlow.value`) then each installed manga extension: `pkgName`, then `File(context.packageManager.getApplicationInfo(pkgName, GET_META_DATA).publicSourceDir).readBytes()` (the APK). Whole files are read into memory. A package removed between listing and reading throws `NameNotFoundException`, which aborts the entire backup (no per-extension skip). Order: anime first, then manga.

### B-08h App and source preferences (`PreferenceBackupCreator`)

App (`createApp(includePrivatePreferences)`):
- Source: `preferenceStore.getAll()` = the default `SharedPreferences.all` map (`AndroidPreferenceStore.kt:63-65`). Contains every key stored in the app's default preference file.
- `toBackupPreferences()`:
  - drop keys where `Preference.isAppState(key)` (prefix `__APP_STATE_`);
  - map the value by runtime type: `Int -> IntPreferenceValue`, `Long -> LongPreferenceValue`, `Float -> FloatPreferenceValue`, `String -> StringPreferenceValue`, `Boolean -> BooleanPreferenceValue`, `Set<*>` -> `(value as? Set<String>)?.let { StringSetPreferenceValue(it) }` (unchecked cast; any non-String element fails at encode time), anything else -> dropped.
- `withPrivatePreferences(include)`: if `include` false, drop keys with prefix `__PRIVATE_`.

Source (`createSource(includePrivatePreferences)`):
- Anime: `animeSourceManager.getAll().filterIsInstance<ConfigurableAnimeSource>()`, each -> `BackupSourcePreferences(sourceKey = it.preferenceKey(), prefs = it.sourcePreferences().all.toBackupPreferences().withPrivatePreferences(...))`, keep only non-empty `prefs`.
- Manga: `mangaSourceManager.getCatalogueSources().filterIsInstance<ConfigurableSource>()` (same mapping). Asymmetry: anime uses all installed sources, manga only catalogue sources.
- Result: anime list + manga list.
- `preferenceKey()` = `"source_$id"` (`ConfigurableSource.kt:22`, `ConfigurableAnimeSource.kt:22`); the values come from the SharedPreferences file of that name (`ConfigurableSource.kt:25-26`).

Known private keys (`Preference.privateKey(...)`, excluded unless `privateSettings`): `pref_mangasync_username_<trackerId>`, `pref_mangasync_password_<trackerId>`, `pref_tracker_auth_expired_<trackerId>` (`app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt:15-25`), `track_token_<trackerId>` (`TrackPreferences.kt:35`). The UI says these are "tracker login tokens".

Known app-state keys (never backed up): `__APP_STATE_last_auto_backup_timestamp` (this area), `__APP_STATE_library_update_last_timestamp`, `__APP_STATE_storage_dir`, `__APP_STATE_last_app_closed`, `__APP_STATE_last_version_code`, and others listed by `grep appStateKey`.

---

## B-09 Entry mapping rules shared by all creators

OBSERVED. These are the SQL-mapper conversions (the `backup*Mapper` lambdas in the `models/` files). They apply when converting database rows to backup entries; the reverse direction (`get*Impl`) is restore-side.

- Float fields: `chapterNumber`, `episodeNumber` (Double -> Float), `lastChapterRead`, `lastEpisodeSeen` (Double -> Float), `score` (Double -> Float). Precision loss beyond ~7 significant digits is accepted by the format (see edge E-07).
- `totalChapters`, `totalEpisodes`, `status` (Long -> Int). Overflow silently wraps.
- `libraryId` null -> 0 ("forced not null so it's compatible with 1.x backup system").
- `sourceOrder = source_order`.
- `scanlator` nullable kept as null.
- `memo`: `MemoColumnAdapter.encode(JsonObject)` = UTF-8 bytes of the JSON text; empty object is `{}` (`mihon.core.common.extensions.JsonObjectEmptyBytes` = bytes `7B 7D`).
- `genre`: `Manga.genre` is nullable; backup uses `orEmpty()`.
- `updateStrategy`/`fetchType` are enums written as enum values (see the wire topic file).

---

## B-10 Last automatic backup timestamp

OBSERVED. Stored as `Preference.appStateKey("last_auto_backup_timestamp")` = key `__APP_STATE_last_auto_backup_timestamp`, Long, default 0 (`BackupPreferences.kt:12`). Written only on a successful automatic run, after self-validation (B-06.9), with `Instant.now().toEpochMilli()`. Read by `SettingsDataScreen` for the text `last_auto_backup_info` ("Last automatically backed up: %s") with `relativeTimeSpanString(lastAutoBackup)`. Manual runs never update it.

---

## B-11 Decoding and format sniffing (reader, used by validation and by restore)

OBSERVED. `BackupDecoder.decode(uri)`:
1. Open `contentResolver.openInputStream(uri)!!`, `source().buffer()`.
2. `peeked = source.peek().apply { require(2) }`: peek two bytes; `require(2)` throws `EOFException` if fewer than 2 bytes exist (not converted to a user message).
3. `id1id2 = peeked.readShort()` (big-endian signed 16-bit).
4. Dispatch on `id1id2.toInt()`:
   - `0x1f8b`: gzip. `source.gzip().buffer()`.
   - `0x7b7d` (`{}`), `0x7b22` (`{"`), `0x7b0a` (`{\n`): throw `IOException(MR.strings.invalid_backup_file_json)` = "JSON backup not supported".
   - anything else: treat as raw (uncompressed) protobuf.
5. `backupString = (chosen source).use { it.readByteArray() }` (whole file in memory).
6. `if (BackupDetector.isLegacyBackup(backupString))`: decode `LegacyBackup` and `.toBackup()`; else decode `Backup`.
7. `SerializationException` at step 6 -> `IOException(MR.strings.invalid_backup_file_unknown)` = "Backup file is corrupted". Other exceptions (`IOException` from gzip corruption, `EOFException`) propagate unchanged.

Note: a file that is JSON but starts with other bytes (for example leading whitespace) is treated as raw protobuf and fails with "corrupted", not "JSON not supported".

---

## B-12 Legacy detection (`BackupDetector.isLegacyBackup`)

OBSERVED. Decode the bytes with a partial message that declares only field 103 (`backupAnimeSources`, list of `{name=1 (default ""), sourceId=2}`) and field 500 (`isLegacy`, default true). Result: `isLegacy && backupAnimeSources.isNotEmpty()`. Any `SerializationException` -> false.

Effect:
- Legacy file: field 500 absent (default true) and field 103 present and non-empty. The file is decoded as `LegacyBackup` (anime uses field 3, extensions field 106, stores field 107, repos field 108, custom buttons field 109; see p1-data-and-config.md section 1.2).
- Current file: field 500 is always written as `false` (`isLegacy = false`), so detection is false.
- Mihon file: no 103 and no 500 -> false, decoded as current `Backup`.

Known weakness: a legacy file with no anime sources (field 103 empty) is decoded as current `Backup` and its anime fields (field 3) are ignored. PROPOSAL: detect legacy by the presence of field 500 with value false being absent AND any of fields 3/4/103/106..109 present.

---

## B-13 Restore preflight validation (`BackupFileValidator.validate`)

OBSERVED. `BackupFileValidator.kt:23-70`.
1. `backup = BackupDecoder(context).decode(uri)`. Any exception is rethrown as `IllegalStateException(e)`. Its message is `e.toString()`, e.g. `java.io.IOException: Backup file is corrupted`. The restore UI shows `e.message` directly.
2. Manga sources: `sources = backupSources.associate { it.sourceId to it.name }`. Missing = keys where `mangaSourceManager.get(key) == null`. Name for each missing entry: if `name.toLongOrNull()` is null, the stored name is used as is (may be `""`); else `mangaSourceManager.getOrStub(id).toString()` (`StubMangaSource.toString()` = `"$name (${lang.uppercase()})"` when valid, else the id as text). Then `.distinct().sorted()`.
3. Anime sources: same with `backupAnimeSources` and `animeSourceManager`. The two lists are concatenated (manga first, then anime), each sorted.
4. Trackers: `trackers = (anime entries' tracking syncIds + manga entries' tracking syncIds).distinct()`. For each id: `trackerManager.get(id.toLong())` (null for unknown ids, silently skipped), filter `!isLoggedIn`, map `.name`, sort.
5. Return `Results(missingSources, missingTrackers)`.

Results are used by restore to show "Missing sources" and "Trackers not logged into". Creation calls the same function only for its exception side effect.

---

## B-14 Source identity and stub rule

OBSERVED. `AndroidAnimeSourceManager.getOrStub(id)` (and manga twin): returns the installed source; otherwise a cached stub from `createStubSource(id)`, which returns (in order) a stored stub from `sourceRepository.getStubAnimeSource(id)`, the extension data from `extensionManager.getSourceData(id)`, else `StubAnimeSource(id, lang = "", name = "")`. A stub whose name or lang is blank is `isInvalid` and prints as its numeric id.

---

## B-15 Notifications

OBSERVED. Channels, IDs (`app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt:58-64, 165-172`):

| Constant | Value | Channel | Importance |
|---|---|---|---|
| `CHANNEL_BACKUP_RESTORE_PROGRESS` | `backup_restore_progress_channel` | progress | `IMPORTANCE_LOW` |
| `ID_BACKUP_PROGRESS` | -501 | | |
| `ID_BACKUP_COMPLETE` | -502 | | |
| `ID_RESTORE_PROGRESS` | -503 | | |
| `ID_RESTORE_COMPLETE` | -504 | | |
| `CHANNEL_BACKUP_RESTORE_COMPLETE` | `backup_restore_complete_channel_v2` | complete | `IMPORTANCE_HIGH` |
| channel group | `group_backup_restore` | | |

Create-side notifications (all `BackupNotifier`):
- Progress (`showBackupProgress`): content title `MR.strings.creating_backup` = "Creating backup"; indeterminate (`setProgress(0, 0, true)`); `ongoing`, `onlyAlertOnce`, `autoCancel(false)`; small icon `ic_ani`, large icon launcher. Shown at the start of every run (as foreground info) and cancelled at the end.
- Error (`showBackupError(error)`): cancel progress; complete channel; title `MR.strings.creating_backup_error` = "Backup failed"; text = `error` (the exception message, may be null/empty). Only for manual runs.
- Complete (`showBackupComplete(file)`): cancel progress; title `MR.strings.backup_created` = "Backup created"; text = `file.displayablePath`; one action: `R.drawable.ic_share_24dp` + `MR.strings.action_share` = "Share" with `NotificationReceiver.shareBackupPendingBroadcast(context, file.uri)`. Only for manual runs. `autoCancel(false)`.

Restore-side notifications (called from area:22 restore code; implemented in the same class):
- Progress (`showRestoreProgress(content = "", progress = 0, maxAmount = 100, sync = false)`): title `MR.strings.syncing_library` ("Syncing library") when `sync`, else `MR.strings.restoring_backup` ("Restoring backup"); text = `content` unless `SecurityPreferences.hideNotificationContent()` (key `hide_notification_content`, default false); determinate `setProgress(maxAmount, progress, false)`; action `MR.strings.action_cancel` ("Cancel") -> `cancelRestorePendingBroadcast(ID_RESTORE_PROGRESS)`; `onlyAlertOnce`.
- Error (`showRestoreError(error)`): cancel progress; title `MR.strings.restoring_backup_error` ("Restoring backup failed"); text = `error`; posted on `ID_RESTORE_COMPLETE`.
- Complete (`showRestoreComplete(time, errorCount, path, file, sync)`): title `MR.strings.library_sync_complete` ("Library sync complete") if `sync` else `MR.strings.restore_completed` ("Restore completed"); text = `R.plurals.restore_completed_message(errorCount, timeString, errorCount)`; `timeString = MR.strings.restore_duration(minutes, seconds)` where `minutes = MILLISECONDS.toMinutes(time)` and `seconds = MILLISECONDS.toSeconds(time) - MINUTES.toSeconds(minutes)`, format `"%1$02d min, %2$02d sec"`. Plural strings (`i18n/src/commonMain/moko-resources/base/plurals.xml:28-31`): one = "Done in %1$s with %2$s error", other = "Done in %1$s with %2$s errors". If `errorCount > 0` and both `path` and `file` non-empty: `File(path, file)`, content intent and action `MR.strings.action_show_errors` = "Tap to see details" -> `openErrorLogPendingActivity(context, uri)`, icon `ic_folder_24dp`.

Cancel rules: `cancelNotification(id)` is called before each terminal post. The complete notifications are never cancelled by this class.

Permission rule: `Context.notify` silently returns when `POST_NOTIFICATIONS` is not granted on API 33+ (`NotificationExtensions.kt:29-40`). Nothing is logged.

---

## B-16 User-visible message keys (all used by this area)

| Key | English text | Where |
|---|---|---|
| `MR.strings.create_backup_file_error` | Couldn't create a backup file | B-06.1 |
| `MR.strings.empty_backup_error` | No library entries to back up | B-06.6 (effectively unreachable) |
| `MR.strings.backup_in_progress` | Backup is already in progress | B-03 |
| `MR.strings.file_picker_error` | No file picker app found | B-03 |
| `MR.strings.restore_in_progress` | Restore is already in progress | restore UI (area:22), shares the restore-running check |
| `MR.strings.invalid_backup_file_json` | JSON backup not supported | B-11 |
| `MR.strings.invalid_backup_file_unknown` | Backup file is corrupted | B-11 |
| `MR.strings.creating_backup` | Creating backup | B-15 |
| `MR.strings.creating_backup_error` | Backup failed | B-15 |
| `MR.strings.backup_created` | Backup created | B-15 |
| `MR.strings.action_share` | Share | B-15 |
| `MR.strings.pref_backup_interval` | Automatic backup frequency | B-04 |
| `MR.strings.update_6hour` .. `update_weekly` | Every 6 hours / Every 12 hours / Daily / Every 2 days / Weekly | B-04 (`update_24hour` = "Daily") |
| `MR.strings.off` | Off | B-04 |
| `MR.strings.backup_info` | You should keep copies of backups in other places as well. Backups may contain sensitive data including any stored passwords; be careful if sharing. | settings |
| `MR.strings.last_auto_backup_info` | Last automatically backed up: %s | B-10 |
| `MR.strings.pref_storage_location_info` | Used for automatic backups, chapter downloads, and local source. | storage settings |
| `MR.strings.restore_miui_warning` | Backup/restore may not function properly if MIUI Optimization is disabled. | B-03 (dropped on Linux) |
| `MR.strings.private_settings` | Include sensitive settings (e.g., tracker login tokens) | B-02 |

Restore-side keys are listed in area:22 and are not repeated here.

---

## B-17 Concurrency and dispatchers

OBSERVED.
- `BackupCreateJob` is a `CoroutineWorker`: `doWork` runs on WorkManager's default background dispatcher. `setForegroundSafely` must be called from the worker.
- `BackupCreator.backup` is `suspend`. Each `handler.awaitList`/`awaitOne` runs on the database handler's dispatcher (IO). File reads and writes are blocking calls made from the worker coroutine.
- Several queries run one after another (no parallelism, no transaction). The entry list is read, then per entry many queries. A user editing the library during a run gets a mixed snapshot.
- `BackupCreator` for automatic runs and the manual run share no state. Two manual runs cannot overlap (`KEEP` + UI guard, B-01). An automatic run and a manual run CAN overlap: nothing prevents it (the automatic `doWork` checks only restore). Two overlapping automatic runs cannot happen for the same periodic unique work.
- `StorageManager` holds a `CoroutineScope(Dispatchers.IO)` and reacts to base-directory changes; `getAutomaticBackupsDirectory()` is called from the worker.
- `BackupNotifier` builders are created once per notifier instance (one per job instance).

PROPOSAL (Kurobayomi):
- One backup at a time: a single-flight guard (`tokio::sync::Mutex` or `try_lock` on a job slot) shared by manual and automatic runs. A manual request while a run is active is refused with `backup_in_progress`. An automatic tick while a run is active is skipped (not queued).
- Read the database inside one read transaction (consistent snapshot) on a blocking thread.

---

## B-18 Timings, limits and constants

| Name | Value | Where |
|---|---|---|
| Automatic interval | 0 (off), 6, 12 (default), 24, 48, 168 hours | B-04 |
| Periodic flex | 10 minutes | `BackupCreateJob.kt:101-102` |
| Retry backoff | EXPONENTIAL, starting 10 minutes | `BackupCreateJob.kt:104` |
| Retry while restore runs | `Result.retry()`, same backoff | B-05.2 |
| Foreground settle delay | 500 ms after `setForeground` | `WorkManagerExtensions.kt:31` |
| Constraint | `requiresBatteryNotLow = true` | `BackupCreateJob.kt:94-96` |
| Max automatic backups kept | 4 (`MAX_AUTO_BACKUPS`) | `BackupCreator.kt:229` |
| Filename minute pattern | `yyyy-MM-dd_HH-mm` (`Locale.ENGLISH`) | `BackupCreator.kt:233` |
| File extension | `.tachibk` | `BackupCreator.kt:234` |
| Gzip level | `okio.gzip()` default (java.util.zip Deflater default, level 6) | `BackupCreator.kt:141` |
| Restore duration format | `%1$02d min, %2$02d sec` | `i18n/.../strings.xml` `restore_duration` |
| Notification ids | -501, -502, -503, -504 | `Notifications.kt:60-64` |
| Restore progress max | 100 default | `BackupNotifier.kt:92` |
| Default options | 10 true, `privateSettings` false, `extensions` false | B-02 |
| Magic bytes | gzip `1F 8B`; JSON `7B 7D`, `7B 22`, `7B 0A` | B-11 |
| Metadata for empty object | `7B 7D` | `JsonObject.kt:7` |

---

## B-19 Interactions with other areas

- area:22-backup-restore: consumes `BackupDecoder`, `BackupFileValidator`, models' `get*Impl()`, `BackupNotifier.showRestore*`, `BackupRestoreJob.isRunning`. Also owns the restore side of `hide_notification_content`.
- area:02-preferences-system: owns the preference store (`PreferenceStore`, `Preference.isAppState/isPrivate`, key prefixes). The backup dump and restore use its key rules.
- area:03/04 (DB schema): tables `chapters`, `episodes`, `mangas`, `animes`, `manga_sync`, `anime_sync`, `history`, `excluded_scanlators`, extension tables. Backup queries must keep these column semantics.
- area:05/06/07/08 (repositories, categories, history, favourites): the repository functions the creators call (`GetAnimeFavorites`, `GetMangaFavorites`, `GetAnimeCategories`, `GetMangaCategories`, `GetAnimeHistory`, `GetMangaHistory`, `GetCustomButtons`, `GetAnimeExtensionStores`, `GetMangaExtensionRepo`).
- area:09-domain-source-track-download-release-misc and area:27-tracking-core: `TrackerManager` (validator tracker check), track entities.
- area:11-app-migrations: `EnableAutoBackupMigration` and `SetupBackupCreateMigration` (B-04).
- area:13-core-util-storage-archive-torrent: `UniFile`, `StorageManager` directory and `displayablePath`, `getUriCompat`.
- area:17/18/19 (extensions): installed extension lists and binaries (B-08g), stores and repos.
- area:26-library-update-jobs: the same battery and network constraint model; the scheduler design is shared (see p1-topic-autobackup-scheduling.md).
- area:34-images-covers-cache: `backgroundUrl` (B-08b gap).

---

## B-20 Summary of observed defects and gaps (verified in source)

1. `backgroundUrl` is never written by the anime creator (`AnimeBackupCreator.kt:77-103`).
2. `backup_flags` (`BackupPreferences.kt:14-17`) is defined with defaults but never read or written anywhere. Dead preference.
3. Automatic runs always skip extensions and private settings (`BackupOptions()` defaults). Not user-visible; documented here so the Kurobayomi UI can say so.
4. Retention prunes before the new backup is written (`BackupCreator.kt:86-92`). A failure during the write leaves one fewer backup.
5. Minute-resolution filenames can collide (B-07).
6. `ExtensionsBackupCreator` aborts the whole backup if one package disappears mid-run.
7. `BackupFileValidator` error messages in restore are `IllegalStateException(e).message` = `java.io.IOException: ...` strings.
8. The empty-backup check (`empty_backup_error`) cannot fire because `isLegacy=false` is always encoded.
9. Creation self-validation only proves the file decodes; missing sources or trackers never fail creation.
10. `StubMangaSource` blank name -> restore preflight can list an empty string as a missing source (B-13.2).
11. `chapterNumber`, `episodeNumber`, `lastChapterRead`, `lastEpisodeSeen`, `score` are stored as float32 (B-09).
12. Excluded scanlators are written even when `chapters` is off (B-08a.2).
