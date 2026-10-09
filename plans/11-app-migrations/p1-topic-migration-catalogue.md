# Area 11: catalogue of the 44 registered migrations

Source of registration order: `app/src/main/java/mihon/core/migration/migrations/Migrations.kt:5-51` (property getter, so a fresh list per access). Run order is "sorted by `version` ascending, stable" (`MigrationJobFactory.kt:16`), so `ALWAYS = -1f` first, then numerically. Two steps share version 64 and keep registration order. Step `TrustExtensionRepositoryMigration` is registered late but has version 7, so when due it sorts before 15.

Every step is a class in `app/src/main/java/mihon/core/migration/migrations/<ClassName>.kt` with `package mihon.core.migration.migrations`. `prefs` below always means `PreferenceManager.getDefaultSharedPreferences(context)` (the single app-wide SharedPreferences file that is also backing `AndroidPreferenceStore`, `core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt:14-16`), and `edit { }` is `androidx.core.content.edit` (applies asynchronously at the end of the block, `commit = true` for synchronous).

Shared Android semantics assumed by many steps:

* Typed reads on a key holding another type throw `ClassCastException` (not caught unless stated).
* `prefs.edit { ... }` stages all operations; reads inside the block still see the pre-edit values.
* `AndroidPreference.get()` catches `ClassCastException`, logs `Invalid value for <key>; deleting`, deletes the key and returns the default (`AndroidPreference.kt:32-40`). `Object<T>.read` (used by every enum and sort preference) instead catches any `Exception` and returns the default without deleting (`AndroidPreference.kt:174-190`). `getEnum` additionally maps an unknown name to the default (`PreferenceStore.kt:27-41`).
* Android `ActivityInfo.SCREEN_ORIENTATION_*` constants: UNSPECIFIED -1, LANDSCAPE 0, PORTRAIT 1, USER 2, BEHIND 3, SENSOR 4, NOSENSOR 5, SENSOR_LANDSCAPE 6, SENSOR_PORTRAIT 7, REVERSE_LANDSCAPE 8, REVERSE_PORTRAIT 9, FULL_SENSOR 10, USER_LANDSCAPE 11, USER_PORTRAIT 12, FULL_USER 13, LOCKED 14.

## 1. Overview table (run order)

Columns: `#` run order; `Reg` index in `Migrations.kt` (1 to 44); idempotent = running it a second time (after a later step crashed and the range reran) gives the same result as running once; disposition codes: PORT = scheduler/helper ported, NORM = pure conversion function for backup import (`p1-topic-legacy-normaliser.md`), DROP = dropped (reason in section 2). Task ids refer to `p1-tasks.md`.

| # | Reg | Version | Class | Returns | Idempotent | Disposition | Task |
|---|---|---|---|---|---|---|---|
| 1 | 1 | ALWAYS | SetupBackupCreateMigration | true | yes | PORT | T-11a-004 |
| 2 | 2 | ALWAYS | SetupAnimeLibraryUpdateMigration | true | yes | PORT | T-11a-006 |
| 3 | 3 | ALWAYS | SetupMangaLibraryUpdateMigration | true | yes | PORT | T-11a-005 |
| 4 | 40 | 7 | TrustExtensionRepositoryMigration | true | yes (source sets are deleted at the end) | NORM (repo URL lists) | T-11a-017 |
| 5 | 4 | 15 | InternalChapterCacheDirMigration | true | yes | DROP | T-11a-020 |
| 6 | 5 | 19 | CoverToExternalFileMigration | true | yes | DROP | T-11a-020 |
| 7 | 6 | 26 | DeleteExternalChapterCacheDirMigration | true | yes | DROP | T-11a-020 |
| 8 | 7 | 44 | ResetSortPreferenceRemovedMigration | true | yes | NORM | T-11a-011 |
| 9 | 8 | 52 | MigrateToTriStateMigration | true | no | NORM | T-11a-011 |
| 10 | 9 | 54 | ForceMALLogOutMigration | true | yes | DROP | T-11a-021 |
| 11 | 10 | 57 | DOHMigration | true | yes | NORM | T-11a-016 |
| 12 | 11 | 59 | ResetRotationMigration | true | yes | NORM | T-11a-013 |
| 13 | 12 | 60 | MigrateRotationViewerValuesMigration | true | no | NORM | T-11a-013 |
| 14 | 13 | 61 | RemoveOneTwoHourUpdateMigration | true | yes | NORM | T-11a-012 |
| 15 | 14 | 64 | SetupBackgroundTasksMigration | true | yes | DROP | T-11a-022 |
| 16 | 15 | 64 | MigrateSortingModeMigration | true | no | NORM | T-11a-011 |
| 17 | 16 | 70 | AddAllLangMigration | true | yes | NORM | T-11a-016 |
| 18 | 17 | 71 | RemoveQuickUpdateMigration | true | yes | NORM | T-11a-012 |
| 19 | 18 | 72 | CombineUpdateRestrictionMigration | true | yes | NORM | T-11a-012 |
| 20 | 19 | 75 | MigrateSecureScreenMigration | true | yes | NORM (secure screen) + DROP (MIUI installer) | T-11a-016, T-11a-024 |
| 21 | 20 | 77 | RemoveReaderTapMigration | true | yes | NORM | T-11a-013 |
| 22 | 21 | 81 | RenameEnumMigration | true | yes | NORM | T-11a-011 |
| 23 | 22 | 82 | MergeSortTypeDirectionMigration | true | no (rerun appends a second direction) | NORM | T-11a-011 |
| 24 | 23 | 84 | EnableAutoBackupMigration | true | yes | NORM | T-11a-012 |
| 25 | 24 | 85 | MoveChapterPreferencesMigration | true | no (throws on rerun) | NORM | T-11a-011 |
| 26 | 25 | 86 | SplitPreferencesMigration | true | yes | NORM | T-11a-016 |
| 27 | 26 | 92 | PlayerPreferenceMigration | true | yes | NORM | T-11a-014 |
| 28 | 27 | 93 | MovePlayerPreferencesMigration | true | no (tracking queue) | NORM (prefs) + DROP (tracking queue) | T-11a-014, T-11a-024 |
| 29 | 28 | 96 | UseWorkManagerMigration | true | yes | DROP | T-11a-022 |
| 30 | 29 | 97 | RemoveBackgroundJobsMigration | true | yes | DROP | T-11a-022 |
| 31 | 30 | 99 | MigrateTriStateMigration | true | no (rerun resets filters to DISABLED) | NORM | T-11a-011 |
| 32 | 31 | 105 | DontRunJobsMigration | true | yes | NORM | T-11a-012 |
| 33 | 32 | 106 | RelativeTimestampMigration | true | yes | NORM | T-11a-016 |
| 34 | 33 | 113 | NoAppStateMigration | true | yes | DROP | T-11a-023 |
| 35 | 34 | 114 | ExternalRepoMigration | true | no (double wrap) | NORM (repo URL lists) | T-11a-017 |
| 36 | 35 | 116 | PrivatePreferenceMigration | true | yes | DROP | T-11a-023 |
| 37 | 36 | 117 | PermaTrustExtensionsMigration | true | yes | DROP | T-11a-023 |
| 38 | 37 | 120 | NavigationOptionsMigration | true | no | NORM | T-11a-016 |
| 39 | 38 | 121 | LogOutMALMigration | true | yes | DROP | T-11a-021 |
| 40 | 39 | 123 | EnumsMigration | true | no (rerun resets debanding, aspect, gpu-next to defaults) | NORM | T-11a-014 |
| 41 | 41 | 126 | VideoPlayerPreferenceMigration | true or false | yes | NORM | T-11a-014 |
| 42 | 42 | 127 | VideoOrientationMigration | true | no | NORM | T-11a-014, T-11a-026 |
| 43 | 43 | 129 | CategoryPreferencesCleanupMigration | true | yes | PORT (helper) | T-11a-008 |
| 44 | 44 | 130 | PrefLangMigration | true | yes | NORM | T-11a-015 |

Registration index (`Reg`, line in `Migrations.kt` = Reg + 6): 1 SetupBackupCreate, 2 SetupAnimeLibraryUpdate, 3 SetupMangaLibraryUpdate, 4 InternalChapterCacheDir, 5 CoverToExternalFile, 6 DeleteExternalChapterCacheDir, 7 ResetSortPreferenceRemoved, 8 MigrateToTriState, 9 ForceMALLogOut, 10 DOH, 11 ResetRotation, 12 MigrateRotationViewerValues, 13 RemoveOneTwoHourUpdate, 14 SetupBackgroundTasks, 15 MigrateSortingMode, 16 AddAllLang, 17 RemoveQuickUpdate, 18 CombineUpdateRestriction, 19 MigrateSecureScreen, 20 RemoveReaderTap, 21 RenameEnum, 22 MergeSortTypeDirection, 23 EnableAutoBackup, 24 MoveChapterPreferences, 25 SplitPreferences, 26 PlayerPreference, 27 MovePlayerPreferences, 28 UseWorkManager, 29 RemoveBackgroundJobs, 30 MigrateTriState, 31 DontRunJobs, 32 RelativeTimestamp, 33 NoAppState, 34 ExternalRepo, 35 PrivatePreference, 36 PermaTrustExtensions, 37 NavigationOptions, 38 LogOutMAL, 39 Enums, 40 TrustExtensionRepository, 41 VideoPlayerPreference, 42 VideoOrientation, 43 CategoryPreferencesCleanup, 44 PrefLang.

## 2. Step-by-step specification

Format per step: file and version; collaborators requested from the container (a missing one returns `false` at once); OBSERVED algorithm with exact keys, types, constants; quirks; Proposal.

### 2.1 Always-run steps (version `Migration.ALWAYS = -1f`)

#### SetupBackupCreateMigration (`SetupBackupCreateMigration.kt:8-18`)
Collaborators: `Application`. Body: `BackupCreateJob.setupTask(context)`; return `true`.
`BackupCreateJob.setupTask(context, prefInterval = null)` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt:90-114`):
1. `interval = backupPreferences.backupInterval().get()`; key `backup_interval`, Int, default `12` (hours), `domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt:10`.
2. If `interval > 0`: `Constraints(requiresBatteryNotLow = true)`; `PeriodicWorkRequestBuilder<BackupCreateJob>(interval hours, flex 10 minutes)`; `setBackoffCriteria(EXPONENTIAL, 10, MINUTES)`; `addTag("BackupCreator")` (`TAG_AUTO`, line 131); `setInputData(is_auto_backup = true)`; `enqueueUniquePeriodicWork("BackupCreator", ExistingPeriodicWorkPolicy.UPDATE, request)`.
3. Else: `cancelUniqueWork("BackupCreator")`.
Effect: after every launch the periodic job is re-registered with current preference values, which is how changes made while the app was closed (for example restored preferences) take effect. Proposal: port as `backup_scheduler.reconfigure()`, see `p1-topic-scheduler-setup.md`. Task T-11a-004.

#### SetupAnimeLibraryUpdateMigration (`SetupAnimeLibraryUpdateMigration.kt:8-18`)
Collaborators: `Application`. Body: `AnimeLibraryUpdateJob.setupTask(context)`; return `true`. `setupTask` (`app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt:452-498`):
1. `interval = libraryPreferences.autoUpdateInterval().get()`; key `pref_library_update_interval_key`, Int hours, default `0` (disabled), shared by anime and manga.
2. If `interval > 0`: `restrictions = autoUpdateDeviceRestrictions().get()` (key `library_update_restriction`, string set, default `{"wifi"}`; constants `DEVICE_ONLY_ON_WIFI = "wifi"`, `DEVICE_NETWORK_NOT_METERED = "network_not_metered"`, `DEVICE_CHARGING = "ac"`, `LibraryPreferences.kt:399-401`). `networkType = UNMETERED` if `"network_not_metered"` in restrictions else `CONNECTED`. `NetworkRequest` gets `TRANSPORT_WIFI` if `"wifi"` in restrictions and `NET_CAPABILITY_NOT_METERED` if `"network_not_metered"` in restrictions. Constraints: `setRequiredNetworkRequest(request, networkType)`, `setRequiresCharging("ac" in restrictions)`, `setRequiresBatteryNotLow(true)`.
3. `PeriodicWorkRequestBuilder<AnimeLibraryUpdateJob>(interval hours, flex 10 minutes)`, tags `"AnimeLibraryUpdate"` (`TAG`, line 435) and `"AnimeLibraryUpdate-auto"` (`WORK_NAME_AUTO`, line 436), backoff `LINEAR 10 minutes`, `enqueueUniquePeriodicWork("AnimeLibraryUpdate-auto", UPDATE, request)`.
4. Else `cancelUniqueWork("AnimeLibraryUpdate-auto")`.
Proposal: `anime_update_scheduler.reconfigure()`. Task T-11a-006.

#### SetupMangaLibraryUpdateMigration (`SetupMangaLibraryUpdateMigration.kt:8-18`)
Identical to the anime step with `MangaLibraryUpdateJob` (`app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt:439-486`), tags `"LibraryUpdate"` (`TAG`, line 423) and `"LibraryUpdate-auto"` (`WORK_NAME_AUTO`, line 424). Manual run name `"LibraryUpdate-manual"`. Task T-11a-005.

### 2.2 Versioned steps, in run order

#### Version 7: TrustExtensionRepositoryMigration (`TrustExtensionRepositoryMigration.kt:13-53`)
Collaborators: `SourcePreferences`, `AnimeExtensionStoreRepository`, `MangaExtensionRepoRepository` (each missing instance returns `false`; the anime preference is already deleted by then if the manga repository is the missing one). Runs in `withIOContext` (`Dispatchers.IO`).
Reachability: version 7 is inside `(old + 1)..new` only when `old <= 6`; `old == 0` selects the Initial strategy that skips it. No real install has `last_version_code` between 1 and 6 (the build number is 132), so the step never runs on a real device. It is the result of the Aniyomi fork numbering; see `p1-topic-upstream-quirks.md` item 2.
Algorithm if it did run:
1. Anime: for `(index, source)` in `sourcePreferences.animeExtensionRepos().get().withIndex()` (key `anime_extension_repos`, string set, default empty; iteration order is the `HashSet` order, not insertion or alphabetical order):
   * `indexUrl = source.removeSuffix("/index.min.json").removeSuffix("/index.json") + "/repo.json"` (applied in that order, so `.../index.min.json` and `.../index.json` both become `.../repo.json`, and a bare base URL gets `/repo.json` appended).
   * `name = "Repo #${index + 1}"`.
   * `animeRepository.insertFromPreference(indexUrl, name)` (`data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt:23-35`): upsert into table `extension_store` (anime database; `data/src/main/sqldelightanime/dataanime/extension_store.sq`) with `index_url = indexUrl`, `name = name`, `badge_label = name`, `signing_key = "NO_SIGNING_KEY"`, `contact_website = indexUrl`, `contact_discord = NULL`, `is_legacy = 0`, `extension_list_url = NULL`. Upsert is `ON CONFLICT(index_url) DO UPDATE`.
   * Any `Exception` is logged at ERROR (`Error Migrating Extension Repo with baseUrl: <source>`) and the loop continues.
   * After the loop `animeExtensionRepos().delete()`.
2. Manga: for `(index, source)` in `mangaExtensionRepos().get().withIndex()` (key `extension_repos`): `mangaExtensionRepositoryRepository.upsertRepo(baseUrl = source, name = "Repo #${index + 1}", shortName = null, website = source, signingKeyFingerprint = "NOFINGERPRINT-${index + 1}")` into table `extension_repos` (`data/src/main/sqldelight/data/extension_repos.sq`: `base_url TEXT NOT NULL PRIMARY KEY`, `name TEXT NOT NULL`, `short_name TEXT`, `website TEXT NOT NULL`, `signing_key_fingerprint TEXT UNIQUE NOT NULL`); an `SQLiteException` is wrapped by the repository into `SaveExtensionRepoException` (`MangaExtensionRepoRepositoryImpl.kt:50-62`), caught here and logged (`Error Migrating Manga Extension Repo with baseUrl: <source>`); then `mangaExtensionRepos().delete()`; return `true`.
Proposal: the URL transformation and the placeholder naming become the pure functions `legacy_anime_repo_to_store_row` and `legacy_manga_repo_to_row` in the normaliser (T-11a-017); persistence belongs to area:20-extension-strategy-redesign.

#### Version 15: InternalChapterCacheDirMigration (`InternalChapterCacheDirMigration.kt:8-19`)
Collaborators: `Application`. `File(context.cacheDir, "chapter_disk_cache").deleteRecursively()`; return `true`. No existence check (deleteRecursively on a missing path returns false harmlessly). Drop (T-11a-020): the Android internal cache directory layout of 2018-era Tachiyomi has no Linux counterpart; Kurobayomi's chapter cache lives under `Paths::cache()` and starts empty. User loses nothing.

#### Version 19: CoverToExternalFileMigration (`CoverToExternalFileMigration.kt:8-27`)
Collaborators: `Application`. `oldDir = File(context.externalCacheDir, "cover_disk_cache")`; if `oldDir.exists()`: `destDir = context.getExternalFilesDir("covers")`; if non-null, for each file in `oldDir.listFiles()` do `it.renameTo(File(destDir, it.name))` (rename failures are ignored; `oldDir` itself is left in place). Return `true`. Edge case: `externalCacheDir == null` makes `File(null as File?, name)` a relative path, almost never existing. Drop (T-11a-020): Android external-storage layout. Kurobayomi stores covers under `Paths::data()/covers` from the start (area:34-images-covers-cache).

#### Version 26: DeleteExternalChapterCacheDirMigration (`DeleteExternalChapterCacheDirMigration.kt:8-25`)
`extCache = context.externalCacheDir`; if non-null and `File(extCache, "chapter_disk_cache").exists()` then `deleteRecursively()`. Return `true`. Drop (T-11a-020).

#### Version 44: ResetSortPreferenceRemovedMigration (`ResetSortPreferenceRemovedMigration.kt:10-43`)
Collaborators: `Application`, `LibraryPreferences`. Reads `prefs.getInt(mangaSortingMode().key(), 0)` where the key is `library_sorting_mode`; if the value is `5` (the removed `SOURCE` sort, per the inline comment) writes Int `0` (`ALPHABETICAL = 0`). The same for `animeSortingMode().key()` = `animelib_sorting_mode`. Return `true`. Typed Int read of a key that is not an Int throws `ClassCastException` (not caught). Proposal: normaliser maps legacy Int sort `5` to `0` as part of the sort family (T-11a-011).

#### Version 52: MigrateToTriStateMigration (`MigrateToTriStateMigration.kt:11-51`)
Collaborators: `Application`, `LibraryPreferences`. In one `prefs.edit`:
* `putInt(filterDownloadedManga().key() = "pref_filter_library_downloaded_v2", convert("pref_filter_downloaded_key"))`, then `remove("pref_filter_downloaded_key")`.
* `putInt("pref_filter_library_unread_v2", convert("pref_filter_unread_key"))`, remove `pref_filter_unread_key`.
* `putInt("pref_filter_library_completed_v2", convert("pref_filter_completed_key"))`, remove `pref_filter_completed_key`.
`convert(key)` = `prefs.getBoolean(key, false)` mapped `true -> 1`, `false -> 0`. The write is unconditional.
Quirk: the destination keys are resolved from today's `LibraryPreferences`, which now expect an enum name String, so the Int written here is later read through `Object.read`, fails with `ClassCastException` caught as `Exception`, and yields `TriState.DISABLED`; any filter state from before build 52 is lost. See quirks item 6. Proposal: normaliser converts the legacy boolean keys to `TriState` (`true -> ENABLED_IS`, `false -> DISABLED`) and writes the correct `_v2` enum name (T-11a-011).

#### Version 54: ForceMALLogOutMigration (`ForceMALLogOutMigration.kt:7-22`)
Collaborators: `TrackerManager`. `if (trackerManager.myAnimeList.isLoggedIn) trackerManager.myAnimeList.logout()`; return `true`. `isLoggedIn` is `getUsername().isNotEmpty() && getPassword().isNotEmpty()` (`app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt:28-33`); `MyAnimeList.logout()` clears the credentials (`setCredentials(this, "", "")`), deletes `trackToken(this)` and `interceptor.setAuth(null)` (`MyAnimeList.kt:262-266`). The comment says: "v52: switched from scraping to WebView; v53: switched from WebView to OAuth". Drop (T-11a-021): a fresh Kurobayomi profile has no MAL credentials; MAL login is OAuth from day one (area:29-tracker-myanimelist). Loss: none.

#### Version 57: DOHMigration (`DOHMigration.kt:11-30`)
Collaborators: `Application`, `NetworkPreferences`. `wasDohEnabled = prefs.getBoolean("enable_doh", false)`; if true, in one edit `putInt(networkPreferences.dohProvider().key() = "doh_provider", PREF_DOH_CLOUDFLARE = 1)` and `remove("enable_doh")`. If false the old key is left untouched. Return `true`. Provider ids (`core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt:12-24`): CLOUDFLARE 1, GOOGLE 2, ADGUARD 3, QUAD9 4, ALIDNS 5, DNSPOD 6, 360 7, QUAD101 8, MULLVAD 9, CONTROLD 10, NJALLA 11, SHECAN 12, LIBREDNS 13; the default of `doh_provider` is `-1` (off). Proposal: normaliser maps `enable_doh = true` to `doh_provider = 1` (T-11a-016).

#### Version 59: ResetRotationMigration (`ResetRotationMigration.kt:9-25`)
If `prefs.contains("pref_rotation_type_key")` then `putInt("pref_rotation_type_key", 1)` (1 = "Free" in the old numbering). Return `true`. Resets any previous "lock" rotation value. Proposal: normaliser (T-11a-013) together with step 60.

#### Version 60: MigrateRotationViewerValuesMigration (`MigrateRotationViewerValuesMigration.kt:10-39`)
Collaborators: `Application`. Reads `prefs.getInt("pref_rotation_type_key", 1)` and maps:

| old value | `ReaderOrientation` | stored `flagValue` (`ReaderOrientation.kt`) |
|---|---|---|
| 1 | FREE | `0x00000008` (8) |
| 2 | PORTRAIT | `0x00000010` (16) |
| 3 | LANDSCAPE | `0x00000018` (24) |
| 4 | LOCKED_PORTRAIT | `0x00000020` (32) |
| 5 | LOCKED_LANDSCAPE | `0x00000028` (40) |
| anything else | FREE | `0x00000008` |

`newReadingMode = prefs.getInt("pref_default_viewer_key", 1)` ("Reading mode flag and prefValue is the same value": 1 = LEFT_TO_RIGHT 0x1, 2 = RIGHT_TO_LEFT 0x2, 3 = VERTICAL 0x3, 4 = WEBTOON 0x4, 5 = CONTINUOUS_VERTICAL 0x5, `ReadingMode.kt`). In one edit: `putInt("pref_default_orientation_type_key", newOrientation)`, `remove("pref_rotation_type_key")`, `putInt("pref_default_reading_mode_key", newReadingMode)`, `remove("pref_default_viewer_key")`. Unconditional writes. Return `true`.
Quirks: the default reading mode today is `RIGHT_TO_LEFT.flagValue` (2) and the default orientation `FREE.flagValue` (8), but the migration writes `1` (left to right) for a user who never set a viewer; a rerun after the old keys are removed resets both values to `1` and `FREE`. `ReaderOrientation.MASK = 0x00000038`; `DEFAULT = 0x00`, `REVERSE_PORTRAIT = 0x30` are not produced here. Proposal: normaliser (T-11a-013) writes the new keys only when the old keys are present (fixing the unconditional write).

#### Version 61: RemoveOneTwoHourUpdateMigration (`RemoveOneTwoHourUpdateMigration.kt:10-27`)
Collaborators: `Application`, `LibraryPreferences`. `interval = autoUpdateInterval().get()`; if `interval == 1 || interval == 2`: set `autoUpdateInterval = 3`, then `MangaLibraryUpdateJob.setupTask(context, 3)` and `AnimeLibraryUpdateJob.setupTask(context, 3)`. Return `true`. Proposal: normaliser maps interval 1 and 2 to 3 (T-11a-012); scheduling is not needed at import time because the always-run set-up re-registers.

#### Version 64 (registered first): SetupBackgroundTasksMigration (`SetupBackgroundTasksMigration.kt:9-21`)
`MangaLibraryUpdateJob.setupTask(context)` then `AnimeLibraryUpdateJob.setupTask(context)`; return `true`. Fully redundant with the always-run steps. Drop (T-11a-022).

#### Version 64 (registered second): MigrateSortingModeMigration (`MigrateSortingModeMigration.kt:10-72`)
Collaborators: `Application`, `LibraryPreferences`. Comment: "Switch to sort per category".
Reads (typed Int): `prefs.getInt("library_sorting_mode", 0)`, `prefs.getInt("animelib_sorting_mode", 0)`, `prefs.getBoolean("library_sorting_ascending", true)`.
Manga mapping (`when`): 0 `"ALPHABETICAL"`, 1 `"LAST_READ"`, 2 `"LAST_CHECKED"`, 3 `"UNREAD"`, 4 `"TOTAL_CHAPTERS"`, 6 `"LATEST_CHAPTER"`, 8 `"DATE_FETCHED"`, 7 `"DATE_ADDED"`, else `"ALPHABETICAL"` (5 = SOURCE was removed by step 44).
Anime mapping: 0 `"ALPHABETICAL"`, 1 `"LAST_SEEN"`, 2 `"LAST_CHECKED"`, 3 `"UNSEEN"`, 4 `"TOTAL_EPISODES"`, 6 `"LATEST_EPISODE"`, 8 `"DATE_FETCHED"`, 7 `"DATE_ADDED"`, else `"ALPHABETICAL"`.
Direction: `true -> "ASCENDING"`, `false -> "DESCENDING"`.
Writes: first `prefs.edit(commit = true)` removing `library_sorting_mode`, `animelib_sorting_mode`, `library_sorting_ascending` (synchronous so the type change is safe), then a second edit with `putString("library_sorting_mode", newManga)`, `putString("animelib_sorting_mode", newAnime)`, `putString("library_sorting_ascending", newDirection)`. Return `true`.
Pipeline with steps 81 and 82 (all three always ran in order 64, 81, 82 on an old install): 64 yields `"<MODE>"` + separate direction key; 81 renames modes; 82 merges to `"<MODE>,<DIRECTION>"`. The final string must be parseable by `MangaLibrarySort.deserialize`/`AnimeLibrarySort.deserialize` (`domain/src/main/java/tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt`, `.../anime/model/AnimeLibrarySortMode.kt`), which accept for manga `ALPHABETICAL, LAST_READ, LAST_MANGA_UPDATE, UNREAD_COUNT, TOTAL_CHAPTERS, LATEST_CHAPTER, CHAPTER_FETCH_DATE, DATE_ADDED, TRACKER_MEAN, RANDOM` and for anime `ALPHABETICAL, LAST_SEEN, LAST_ANIME_UPDATE, UNSEEN_COUNT, TOTAL_EPISODES, LATEST_EPISODE, EPISODE_FETCH_DATE, DATE_ADDED, TRACKER_MEAN, AIRING_TIME, RANDOM`; unknown type names fall back to `ALPHABETICAL`, a direction equal to `"ASCENDING"` is ascending and anything else descending.
Quirk: anime names produced by 64 and 81 (`LAST_CHECKED -> LAST_MANGA_UPDATE`, `UNSEEN`, `DATE_FETCHED -> CHAPTER_FETCH_DATE`) are not names the anime deserializer knows, so an anime library sort migrated from before 64 always ends as `ALPHABETICAL`; see quirks item 7. Proposal: normaliser (T-11a-011) builds the final string directly from the legacy Int and Boolean with correct anime names: anime Int 2 -> `LAST_ANIME_UPDATE`, 3 -> `UNSEEN_COUNT`, 8 -> `EPISODE_FETCH_DATE`.

#### Version 70: AddAllLangMigration (`AddAllLangMigration.kt:8-21`)
Collaborators: `SourcePreferences`. If `enabledLanguages().isSet()` (key `source_languages`, string set; default `setOf("all", "en", Locale.getDefault().language)`, `LocaleHelper.kt:72-74`) then `enabledLanguages() += "all"` (adds `"all"` to the stored set, `Preference.kt:61-63`). Return `true`. Proposal: normaliser adds `"all"` when importing a stored language set that lacks it (T-11a-016).

#### Version 71: RemoveQuickUpdateMigration (`RemoveQuickUpdateMigration.kt:10-27`)
If `autoUpdateInterval().get()` is in `[3, 4, 6, 8]` then set it to `12` and call both `setupTask(context, 12)`. Return `true`. Combined with 61, interval 1 or 2 ends at 12 when both steps ran. Normaliser (T-11a-012): apply 61 then 71 in order, so 1, 2, 3, 4, 6, 8 all become 12.

#### Version 72: CombineUpdateRestrictionMigration (`CombineUpdateRestrictionMigration.kt:11-30`)
Collaborators: `Application`, `LibraryPreferences`. `oldUpdateOngoingOnly = prefs.getBoolean("pref_update_only_non_completed_key", true)`; if false, `autoUpdateItemRestrictions() -= ENTRY_NON_COMPLETED` where the preference (key `library_update_manga_restriction`) has default `{"manga_fully_read", "manga_ongoing", "manga_started", "manga_outside_release_period"}` and `ENTRY_NON_COMPLETED = "manga_ongoing"` (`LibraryPreferences.kt:403-406`); when unset the removal writes the default minus `manga_ongoing` explicitly. The old key is not removed. Return `true`. Normaliser (T-11a-012): if the legacy key is present and false, remove `manga_ongoing` from the restriction set (starting from the default when absent).

#### Version 75: MigrateSecureScreenMigration (`MigrateSecureScreenMigration.kt:11-37`)
Collaborators: `Application`, `SecurityPreferences`, `BasePreferences`. (a) `oldSecureScreen = prefs.getBoolean("secure_screen", false)`; if true: `securityPreferences.secureScreen().set(SecureScreenMode.ALWAYS)` (key `secure_screen_v2`, enum name string; default `INCOGNITO`; values `ALWAYS`, `INCOGNITO`, `NEVER`, `SecurityPreferences.kt:15-32`). (b) If `DeviceUtil.isMiui` and `basePreferences.extensionInstaller().get() == PACKAGEINSTALLER` set the installer to `LEGACY`. Old key `secure_screen` is not removed. Return `true`. Proposal: (a) in the normaliser (T-11a-016); (b) dropped (T-11a-024): MIUI package installers do not exist on Linux and the extension installer enum is dropped by area:02-preferences-system.

#### Version 77: RemoveReaderTapMigration (`RemoveReaderTapMigration.kt:9-26`)
`oldReaderTap = prefs.getBoolean("reader_tap", false)`; if `!oldReaderTap`: `readerPreferences.navigationModePager().set(5)` and `navigationModeWebtoon().set(5)` (keys `reader_navigation_mode_pager`, `reader_navigation_mode_webtoon`, Int, default 0). Index 5 in `ReaderPreferences.TapZones` is `MR.strings.disabled_nav` (list: 0 label_default, 1 l_nav, 2 kindlish_nav, 3 edge_nav, 4 right_and_left_nav, 5 disabled_nav, `ReaderPreferences.kt:211-218`). An absent `reader_tap` key counts as false, so it also disables tap navigation. Return `true`. Normaliser (T-11a-013).

#### Version 81: RenameEnumMigration (`RenameEnumMigration.kt:10-48`)
In one edit reads `prefs.getString("library_sorting_mode", "ALPHABETICAL")` and `prefs.getString("animelib_sorting_mode", "ALPHABETICAL")` and rewrites each with the same table: `"LAST_CHECKED" -> "LAST_MANGA_UPDATE"`, `"UNREAD" -> "UNREAD_COUNT"`, `"DATE_FETCHED" -> "CHAPTER_FETCH_DATE"`, otherwise unchanged. Writes both keys back unconditionally (an unset key becomes the explicit string `ALPHABETICAL`, with no direction suffix). Return `true`. Quirk: anime shares the manga table (item 7). Normaliser (T-11a-011) uses a separate anime table.

#### Version 82: MergeSortTypeDirectionMigration (`MergeSortTypeDirectionMigration.kt:10-36`)
Inside one edit: `mangasort = prefs.getString("library_sorting_mode", null) ?: return@edit` (the `return@edit` leaves the edit block, writing nothing but the step still returns true), `animesort = prefs.getString("animelib_sorting_mode", null) ?: return@edit`, `direction = prefs.getString("library_sorting_ascending", "ASCENDING")!!`; `putString("library_sorting_mode", "$mangasort,$direction")`, `putString("animelib_sorting_mode", "$animesort,$direction")`, `remove("library_sorting_ascending")`. If either mode is unset nothing is written and the direction key stays. Return `true`. Normaliser (T-11a-011): final value `"<TYPE>,<DIRECTION>"`.

#### Version 84: EnableAutoBackupMigration (`EnableAutoBackupMigration.kt:9-24`)
Collaborators: `Application`, `BackupPreferences`. If `backupInterval().get() == 0` then set it to `12` and `BackupCreateJob.setupTask(context)`. Return `true`. The default is 12 today, so only an explicit stored 0 triggers it (a user who had disabled auto-backup before build 84 gets it turned on). Normaliser (T-11a-012): imported `backup_interval` of 0 stays 0 (the area:02-preferences-system note in T-02a-029 states the same: the step only fires on the upgrade path).

#### Version 85: MoveChapterPreferencesMigration (`MoveChapterPreferencesMigration.kt:10-46`)
Collaborators: `Application`, `LibraryPreferences`. For each of these 12 preference objects (several share a key string, see below) `key = preference.key()`, `value = prefs.getInt(key, Int.MIN_VALUE)`; if `value == Int.MIN_VALUE` skip; else `remove(key)` and `putLong(key, value.toLong())`. All inside one edit. Return `true`.
Keys visited (current definitions, `LibraryPreferences.kt:172-224`): `default_chapter_filter_by_read`, `default_chapter_filter_by_downloaded`, `default_chapter_filter_by_bookmarked`, `default_chapter_sort_by_source_or_number`, `default_chapter_display_by_name_or_number`, `default_chapter_sort_by_ascending_or_descending`, `default_episode_filter_by_seen`, `default_episode_filter_by_downloaded`, `default_episode_filter_by_bookmarked`, `default_episode_sort_by_source_or_number`, then `default_chapter_display_by_name_or_number` again (the anime `displayEpisodeByNameOrNumber` uses the same key string as the manga one) and `default_chapter_sort_by_ascending_or_descending` again (anime sort direction shares the manga key). Because the edit is staged, both visits see the old Int and stage the same write.
Quirk: a value stored as Long throws `ClassCastException` (crash). Normaliser (T-11a-011): widen the 10 distinct legacy Int keys to Long; the bit layouts are in `p1-data-and-config.md` section 3.

#### Version 86: SplitPreferencesMigration (`SplitPreferencesMigration.kt:10-28`)
Collaborators: `Application`, `UiPreferences`. If `themeMode().isSet()` (key `pref_theme_mode_key`): `themeMode = prefs.getString(key, null) ?: return@edit`; `putString(key, themeMode.uppercase())`. Values accepted by the enum pref: `LIGHT`, `DARK`, `SYSTEM` (`ThemeMode.kt`, default `SYSTEM`). Return `true`. Normaliser (T-11a-016): uppercase the stored theme mode string.

#### Version 92: PlayerPreferenceMigration (`PlayerPreferenceMigration.kt:10-34`)
Collaborators: `Application`, `PlayerPreferences`. If `progressPreference().isSet()` (key `pref_progress_preference`, Float, default `0.85F`): `progressString = try { prefs.getString(key, null) } catch (ClassCastException) { null } ?: return@edit`; `newProgress = progressString.toFloatOrNull() ?: return@edit`; `putFloat(key, newProgress)`. Return `true`. Normaliser (T-11a-014): a String value that parses as a float becomes a Float; a Float stays.

#### Version 93: MovePlayerPreferencesMigration (`MovePlayerPreferencesMigration.kt:12-51`)
Collaborators: `Application`, `PlayerPreferences`, `GesturePreferences`. For each of `playerPreferences.defaultPlayerOrientationType()` (key `pref_default_player_orientation_type_key`) and `gesturePreferences.skipLengthPreference()` (key `pref_skip_length_preference`, Int, default 10): if `pref.isSet()`: (1) in an edit: `oldString = try { prefs.getString(key, null) } catch (ClassCastException) { null } ?: return@edit`; `newInt = oldString.toIntOrNull() ?: return@edit`; `putInt(key, newInt)`; (2) still inside the `if`, open the `tracking_queue` SharedPreferences file (`context.getSharedPreferences("tracking_queue", MODE_PRIVATE)`) and for every entry: `val (_, lastChapterRead) = it.value.toString().split(":")` then `remove(it.key)` and `putFloat(it.key, lastChapterRead.toFloat())`. Return `true`.
Quirks: the tracking-queue loop runs once per set preference (twice if both are set); the destructuring throws `IndexOutOfBoundsException` for any value without a colon, including the Floats that `DelayedMangaTrackingStore`/`DelayedAnimeTrackingStore` store today (`app/src/main/java/eu/kanade/domain/track/manga/store/DelayedMangaTrackingStore.kt:18-23`, `putFloat(trackId.toString(), ...)`); and it mutates the SharedPreferences while iterating a snapshot. Proposal: normaliser handles the two string-to-int conversions (T-11a-014); the tracking-queue part is dropped (T-11a-024): Kurobayomi's delayed-tracking queue (area:27-tracking-core) starts empty and uses its own format.

#### Version 96: UseWorkManagerMigration (`UseWorkManagerMigration.kt:9-23`)
`MangaLibraryUpdateJob.cancelAllWorks(context)` (`cancelAllWorkByTag("LibraryUpdate")`), `AnimeLibraryUpdateJob.cancelAllWorks(context)` (tag `"AnimeLibraryUpdate"`), then both `setupTask(context)`. Return `true`. Drop (T-11a-022): WorkManager history; the Rust scheduler has a single code path and nothing to cancel.

#### Version 97: RemoveBackgroundJobsMigration (`RemoveBackgroundJobsMigration.kt:10-26`)
`context.workManager.cancelAllWorkByTag("UpdateChecker")`, `cancelAllWorkByTag("ExtensionUpdate")`, `prefs.edit { remove("automatic_ext_updates") }`. Return `true`. Drop (T-11a-022): the background app-update and extension-update jobs were removed in 2021; Kurobayomi has no such jobs to cancel (its extension update check is in-process, area:20-extension-strategy-redesign).

#### Version 99: MigrateTriStateMigration (`MigrateTriStateMigration.kt:13-52`)
Collaborators: `Application`, `TrackerManager`, `PreferenceStore`. Keys: `pref_filter_library_downloaded`, `pref_filter_library_unread`, `pref_filter_library_unseen`, `pref_filter_library_started`, `pref_filter_library_bookmarked`, `pref_filter_library_completed` plus `pref_filter_library_tracked_<tracker.id>` for each tracker in `TrackerManager.trackers`. For each key: `pref = getInt(key, 0)`; edit: `remove(key)`; `newValue` = `1 -> TriState.ENABLED_IS`, `2 -> TriState.ENABLED_NOT`, else `DISABLED`; `getEnum("${key}_v2", DISABLED).set(newValue)` (stored as enum name `DISABLED`/`ENABLED_IS`/`ENABLED_NOT`). Unconditional. Return `true`. `TriState` order: `DISABLED, ENABLED_IS, ENABLED_NOT` (`TriState.kt:3-17`).
Quirk: the anime keys written (`pref_filter_library_unseen_v2`, `pref_filter_library_started_v2`, ...) are not the keys the app reads for anime today (`pref_filter_animelib_unread_v2`, `pref_filter_animelib_started_v2`, `pref_filter_animelib_bookmarked_v2`, `pref_filter_animelib_completed_v2`, `pref_filter_animelib_downloaded_v2`, `pref_filter_animelib_tracked_<id>_v2`), so anime filters from before 99 are not carried over (item 8). Normaliser (T-11a-011) converts into both families with correct names.

#### Version 105: DontRunJobsMigration (`DontRunJobsMigration.kt:8-22`)
`pref = autoUpdateDeviceRestrictions()`; if `pref.isSet() && "battery_not_low" in pref.get()` then `pref.getAndSet { it - "battery_not_low" }`. Return `true`. Normaliser (T-11a-012): drop the `"battery_not_low"` member from an imported `library_update_restriction` set.

#### Version 106: RelativeTimestampMigration (`RelativeTimestampMigration.kt:8-23`)
`pref = preferenceStore.getInt("relative_time", 7)`; if `pref.get() == 0` then `uiPreferences.relativeTime().set(false)` (key `relative_time_v2`, Boolean, default `true`, `UiPreferences.kt:33`). Old key not removed. Return `true`. Normaliser (T-11a-016).

#### Version 113: NoAppStateMigration (`NoAppStateMigration.kt:9-42`)
Collaborators: `Application`, `PreferenceStore`. `replacePreferences` (section 8 of `p1-behavior.md`) for these keys, new key = `"__APP_STATE_" + key`: `pref_download_only`, `incognito_mode`, `last_catalogue_source`, `trusted_signatures`, `last_app_closed`, `library_update_last_timestamp`, `library_unseen_updates_count`, `last_used_category`, `last_app_check`, `last_ext_check`, `last_version_code`, `storage_dir`. Then `context.cacheDir.deleteRecursively()` (comment: "Deleting old download cache index files, but might as well clear it all out"). Return `true`.
Quirks: `last_version_code` is moved to `__APP_STATE_last_version_code`, the key `App.initializeMigrator` reads; a device upgrading directly across this boundary with the old key would read 0 (item 1). `library_unseen_updates_count` is still read un-prefixed by `LibraryPreferences.newAnimeUpdatesCount()` (`LibraryPreferences.kt:149`), so the moved counter is orphaned. Drop (T-11a-023): the profile is new; the importer ignores un-prefixed legacy copies of app-state keys (area:02-preferences-system backup interop: app-state keys never exported).

#### Version 114: ExternalRepoMigration (`ExternalRepoMigration.kt:8-21`)
`sourcePreferences.mangaExtensionRepos().getAndSet { it.map { repo -> "https://raw.githubusercontent.com/$repo/repo" }.toSet() }`, i.e. every stored `user/repo` shorthand becomes `https://raw.githubusercontent.com/user/repo/repo`. Every entry is wrapped, even if it is already a URL (running twice produces `https://raw.githubusercontent.com/https://raw.githubusercontent.com/...`). Return `true`. Normaliser (T-11a-017): wrap only entries that do not start with `http://` or `https://`.

#### Version 116: PrivatePreferenceMigration (`PrivatePreferenceMigration.kt:8-22`)
`replacePreferences(filterPredicate = { it.key.startsWith("pref_mangasync_") || it.key.startsWith("track_token_") }, newKey = { Preference.privateKey(it) })` (prefix `__PRIVATE_`). Return `true`. Drop (T-11a-023): Kurobayomi stores tracker secrets in the OS keyring through `SecretStore` (area:02-preferences-system, area:27-tracking-core) and never in the preference file; the importer ignores legacy `pref_mangasync_*` and `track_token_*` keys.

#### Version 117: PermaTrustExtensionsMigration (`PermaTrustExtensionsMigration.kt:10-24`)
`prefs.edit { remove(Preference.appStateKey("trusted_signatures")) }`; return `true`. Comment: "Allow permanently trusting unofficial extensions by version code + signature". The current trust store is `SourcePreferences.trustedExtensions()` (key `__APP_STATE_trusted_extensions`, entries `"$pkgName:$versionCode:$signatureHash"`, `TrustMangaExtension.kt:20-25`). Drop (T-11a-023).

#### Version 120: NavigationOptionsMigration (`NavigationOptionsMigration.kt:13-42`)
Collaborators: `Application`, `PreferenceStore`. Reads `bottom_nav_style` (Int, default 0), `default_home_tab_library` (Boolean, default false); in one edit removes both; `startScreen = if (isDefaultTabManga) StartScreen.MANGA else StartScreen.ANIME`; `navStyle = when (style) { 0 -> MOVE_HISTORY_TO_MORE; 1 -> MOVE_UPDATES_TO_MORE; else -> MOVE_MANGA_TO_MORE }`; writes `start_screen` (default `ANIME`; values `ANIME, MANGA, UPDATES, HISTORY, BROWSE`) and `bottom_rail_nav_style` (default `MOVE_HISTORY_TO_MORE`; values `MOVE_MANGA_TO_MORE, MOVE_UPDATES_TO_MORE, MOVE_HISTORY_TO_MORE, MOVE_BROWSE_TO_MORE`) as enum names. Unconditional, so a rerun resets the user's choices to defaults. Return `true`. Normaliser (T-11a-016): only when the legacy keys are present.

#### Version 121: LogOutMALMigration (`LogOutMALMigration.kt:7-19`)
Same body as step 54, with no comment. Drop (T-11a-021).

#### Version 123: EnumsMigration (`EnumsMigration.kt:13-44`)
Collaborators: `Application`, `PreferenceStore`. Reads `pref_deband` (Int, default 0), `pref_player_view_mode` (Int, default 1), `gpu_next` (Boolean, default false). In one edit: remove `pref_invert_playback_txt`, `pref_invert_duration_txt`, `pref_hwdec`, `pref_deband`, `pref_player_view_mode`, `gpu_next`; `videoDebanding = Debanding.entries.first { it.ordinal == deband }` with `Debanding` order `None(0), CPU(1), GPU(2)` (`PlayerEnums.kt:93-97`); `aspectState = VideoAspect.entries.first { it.ordinal == viewMode }` with order `Crop(0), Fit(1), Stretch(2)` (`PlayerEnums.kt:52-56`); then `getEnum("pref_video_debanding", None).set(videoDebanding)`, `getEnum("pref_player_aspect_state", Fit).set(aspectState)`, `getBoolean("pref_gpu_next", false).set(gpuNext)`. An ordinal outside 0..2 throws `NoSuchElementException` and crashes startup. Return `true`. The hardware-decoding preference `pref_hwdec` and the two "invert" text toggles are removed without replacement. Normaliser (T-11a-014): out-of-range values fall back to the defaults instead of throwing.

#### Version 126: VideoPlayerPreferenceMigration (`VideoPlayerPreferenceMigration.kt:14-46`)
Collaborators: `Application`, `SubtitlePreferences`, injected `Json` (`ignoreUnknownKeys = true`, `explicitNulls = false`, `AppModule.kt:174-177`). `subtitleConf = prefs.getString("pref_sub_select_conf", "")!!`; `json.decodeFromString<SubConfig>(subtitleConf)` where `SubConfig(lang: List<String> = [], blacklist: List<String> = [], whitelist: List<String> = [])`; a `SerializationException` (including an empty string or malformed JSON) returns `false` without touching anything. Otherwise one edit: `putString("pref_subtitle_lang", lang.joinToString(","))`, `putString("pref_subtitle_whitelist", whitelist.joinToString(","))`, `putString("pref_subtitle_blacklist", blacklist.joinToString(","))`; return `true`. The old key is kept. Normaliser (T-11a-014).

#### Version 127: VideoOrientationMigration (`VideoOrientationMigration.kt:14-59`)
Collaborators: `Application`, `PlayerPreferences`, `PreferenceStore`. `oldPref = prefs.getInt("pref_default_player_orientation_type_key", 10)`; on `ClassCastException` (the key already holds a String such as an enum name): `prefs.edit(commit = true) { remove(key) }` and return `true` (the preference then reads its default `SensorLandscape`). Mapping (`when`, Android constants):

| stored Int | meaning | `PlayerOrientation` written |
|---|---|---|
| 10 (`FULL_SENSOR`, also the default when unset) | sensor | `Free` |
| 1 (`PORTRAIT`) | | `Portrait` |
| 9 (`REVERSE_PORTRAIT`) | | `ReversePortrait` |
| 0 (`LANDSCAPE`) | | `Landscape` |
| 8 (`REVERSE_LANDSCAPE`) | | `ReverseLandscape` |
| 7 (`SENSOR_PORTRAIT`) | | `SensorPortrait` |
| 6 (`SENSOR_LANDSCAPE`) | | `SensorLandscape` |
| any other | | `Free` |

Then `prefs.edit(commit = true) { remove(key) }` and an edit writing `getEnum(key, SensorLandscape).set(newPref)` (stored as the enum name). `PlayerOrientation` entries: `Free, Video, Portrait, ReversePortrait, SensorPortrait, Landscape, ReverseLandscape, SensorLandscape` (`PlayerEnums.kt:41-50`; `Video` is not produced). Quirk: an unset key is read as 10 and written as `Free`, replacing the new default `SensorLandscape` for everybody who never chose an orientation. Return `true`. Normaliser/drop (T-11a-014, T-11a-026): on desktop there is no device rotation; the value is mapped only so backups round-trip.

#### Version 129: CategoryPreferencesCleanupMigration (`CategoryPreferencesCleanupMigration.kt:11-54`)
Collaborators: `LibraryPreferences`, `DownloadPreferences`, `GetAnimeCategories`, `GetMangaCategories`; runs in `withIOContext`. Algorithm:
1. `allAnimeCategories = getAnimeCategories.await().map { it.id.toString() }.toSet()`, `allMangaCategories` likewise (the SQL `getCategories` returns every row of `categories`, including the system row id 0 inserted by `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0)`, `data/src/main/sqldelight/data/categories.sq:10`, so `"0"` is valid).
2. `defaultAnimeCategory().get()` (key `default_anime_category`, Int, default `-1`): if `toString()` is not in `allAnimeCategories` then `delete()`. Same for `defaultMangaCategory()` (key `default_category`) against `allMangaCategories`. (With the default `-1`, the delete is a harmless no-op.)
3. For each of these 10 string-set preferences: `library_update` manga `library_update_categories`, anime `animelib_update_categories`, manga exclude `library_update_categories_exclude`, anime exclude `animelib_update_categories_exclude`, `remove_exclude_categories`, `remove_exclude_anime_categories`, `download_new_categories`, `download_new_anime_categories`, `download_new_categories_exclude`, `download_new_anime_categories_exclude`: `ids = preference.get()`; `garbage = ids - allAnimeCategories - allMangaCategories` (an id is garbage only if it exists in neither database; ids are not checked against the matching media type); if `garbage` is empty skip, else `preference.set(ids - garbage)`.
4. Return `true`.
Reason it exists: `DeleteMangaCategory`/`DeleteAnimeCategory` forget some sets (item 12 of the quirks file). Proposal: port as `prune_dangling_category_ids` (T-11a-008).

#### Version 130: PrefLangMigration (`PrefLangMigration.kt:14-54`)
Collaborators: `Application`, `AudioPreferences`, `SubtitlePreferences`. For `audioPreferences.preferredAudioLanguages()` (key `pref_audio_lang`, String, default `""`) and `subtitlePreferences.preferredSubLanguages()` (key `pref_subtitle_lang`): if `isSet()`: `langs = prefs.getString(key, "")!!.split(",").filter(String::isNotEmpty).map(String::trim)` (note: filter first, then trim, so a blank-only entry becomes an empty string that is kept and later fails validation), `newLangs = langs.filter { it.isValidCode() }.joinToString(",")`, `putString(key, newLangs)` (written even when empty). `isValidCode()`:
```kotlin
try {
    val locale = Locale(this)
    if (locale.isO3Language == locale.language && locale.language == locale.getDisplayName(Locale.ENGLISH)) return false
} catch (_: MissingResourceException) { return false }
return true
```
Meaning: a code is invalid when the JDK cannot produce an ISO 639-2 three-letter code for it (a two-letter code outside ISO 639-1, which throws `MissingResourceException`), or when it is a three-letter code whose English display name is just the code itself (an unknown language). Valid: known two-letter codes (`en`), known three-letter codes (`eng`, `jpn`), any case (the `Locale` constructor lowercases). Invalid: unknown two-letter codes (`xx`), unknown three-letter codes (`zzz`), hyphenated tags (`en-US`), arbitrary words. Return `true`. Proposal: T-11a-015 uses an ISO 639 table (`isolang` crate) for the validity test.
