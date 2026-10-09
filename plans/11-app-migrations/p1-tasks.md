# Area 11 app-migrations, part 1: implementation tasks

Every scope file and every one of the 44 registered steps has at least one task (mapping in `p1-inventory.md`). The dispositions follow `p1-topic-migration-catalogue.md`: 3 always-run steps are ported as scheduler set-up, 1 step as a helper, 29 steps become legacy-normaliser rules (priority P2, because they only matter when an old Aniyomi backup is imported), and 11 steps are dropped (P3) with a stated replacement or loss.

Priority rule used here: P0 is the framework that lets the app start and record its version; P1 is scheduler set-up and category hygiene (features most users rely on); P2 is legacy-backup normalisation; P3 is the explicit drop register. The numbers 018, 019 and 025 are skipped and carry no task.

Dependencies: `T-01a-*` ids are area:01-foundation-app-bootstrap framework tasks (`plans/01-foundation-app-bootstrap/p1-tasks.md`), `T-02a-034` is the typed prefs tree in area:02-preferences-system, `T-08a-003` is the library sort codec in area:08-domain-library-category-history-updates, `T-01a-043` is the first-frame behaviour in area:01-foundation-app-bootstrap.

### T-11a-001 — Framework conformance tests ported from MigratorTest

- Priority: P0
- Effort: S
- Depends: T-01a-012
- Sources: app/src/test/java/mihon/core/migration/MigratorTest.kt, app/src/main/java/mihon/core/migration/Migration.kt, app/src/main/java/mihon/core/migration/MigrationJobFactory.kt, app/src/main/java/mihon/core/migration/MigrationStrategy.kt, app/src/main/java/mihon/core/migration/MigrationStrategyFactory.kt, app/src/main/java/mihon/core/migration/Migrator.kt
- Acceptance:
    - `select(0, 1)` returns `Initial` and the plan contains only `Always` steps (the Kotlin `initialVersion` case expects exactly one step out of two).
    - `select(1, 1)` returns `Noop`; `run` returns `Ok(false)` and makes zero migration calls (`sameVersion`).
    - `select(1, 2)` with an empty list returns `Ok(false)` and does not call `on_complete` (`noMigrations`).
    - `plan(Range(2..=10))` over 1 `Always` plus steps 2..10 keeps all 10 (`largeMigration`), and drops version 3 when `new = 2` (`withinRangeMigration`).
    - `MigrationContext.dry_run = true` logs the `(Dry-run)` prefix, calls no step body, and still returns `Ok(true)` (`p1-behavior.md` section 4).

### T-11a-002 — MIGRATIONS list and authoring rules

- Priority: P0
- Effort: S
- Depends: T-01a-013
- Sources: app/src/main/java/mihon/core/migration/migrations/Migrations.kt, app/src/main/java/mihon/core/migration/Migration.kt
- Acceptance:
    - `kuro-core::migrate::list::MIGRATIONS` contains exactly three entries in order `SetupBackupCreateMigration`, `SetupAnimeLibraryUpdateMigration`, `SetupMangaLibraryUpdateMigration`, all with `Version::Always`.
    - A comment block lists the 41 disposition rows (class, version, reason), with the same count as `p1-topic-migration-catalogue.md` section 1.
    - The seven authoring rules of `p1-rust-design.md` section 2 are in `kuro-core/src/migrate/README.md`.

### T-11a-003 — Idempotency harness for every registered step

- Priority: P1
- Effort: M
- Depends: T-11a-001, T-11a-002
- Sources: app/src/main/java/mihon/core/migration/MigrationJobFactory.kt, app/src/main/java/mihon/core/migration/migrations/Migrations.kt, app/src/test/java/mihon/core/migration/MigratorTest.kt
- Acceptance:
    - For each entry of `MIGRATIONS` and for each legacy rule of T-11a-011 to T-11a-017, running it twice on the same fixture store gives the same store byte for byte (the test fails on the first differing key).
    - Fixtures are listed in `p1-edge-cases-and-tests.md` section 2. The Kotlin reruns that were not idempotent (rows marked "no" in the catalogue table) pass here because each Rust rule writes only when its legacy key exists.

### T-11a-004 — Automatic backup scheduler set-up (SetupBackupCreateMigration)

- Priority: P1
- Effort: M
- Depends: T-11a-002, area:21-backup-format-and-create, area:26-library-update-jobs
- Sources: app/src/main/java/mihon/core/migration/migrations/SetupBackupCreateMigration.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt
- Acceptance:
    - Every launch calls `setup_backup(prefs, sched)`, which registers a unique periodic job named `BackupCreator` with the interval from `backup_interval` (hours: 0 means off).
    - The periodic job is replaced, not duplicated, when called twice (`enqueue_unique_periodic` with a replace policy).
    - `backup_interval = 0` cancels the unique job; the job is not registered.
    - Test: starting with `backup_interval = 12` gives exactly one registered job named `BackupCreator` with period 12 h (`p1-topic-scheduler-setup.md` sections 2 and 3).

### T-11a-005 — Manga library update scheduler set-up (SetupMangaLibraryUpdateMigration)

- Priority: P1
- Effort: M
- Depends: T-11a-007, area:26-library-update-jobs
- Sources: app/src/main/java/mihon/core/migration/migrations/SetupMangaLibraryUpdateMigration.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
    - Every launch calls `setup_library_update(Manga, prefs, sched, None)`; interval `pref_library_update_interval_key` in hours (0 means off, so nothing is registered).
    - Constraints come from `library_update_restriction`: `wifi` requires a connection (`CONNECTED`), `network_not_metered` requires an unmetered network (`UNMETERED`), `ac` requires power (T-11a-007).
    - Test: interval 6 h with `wifi` produces one job named `LibraryUpdate` for manga with period 6 h and a connected-network constraint.

### T-11a-006 — Anime library update scheduler set-up (SetupAnimeLibraryUpdateMigration)

- Priority: P1
- Effort: M
- Depends: T-11a-007, area:26-library-update-jobs
- Sources: app/src/main/java/mihon/core/migration/migrations/SetupAnimeLibraryUpdateMigration.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
    - Same contract as T-11a-005 for anime, using the same interval and restriction keys, registered under a separate unique name (`AnimeLibraryUpdate`).
    - Changing one media type's job never cancels the other (test: cancel anime, manga job still present).

### T-11a-007 — Scheduler constraints, power/network state and overdue rule

- Priority: P1
- Effort: M
- Depends: T-11a-002, T-01a-012, area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
    - `PowerNetworkState` (kuro-platform, Linux via NetworkManager and UPower over `zbus`, fallback `gio::NetworkMonitor`) exposes `is_metered()`, `is_connected()`, `on_ac_power()` and a `watch::Receiver<u64>` for changes.
    - A job whose constraints are unmet is deferred and rechecked when the watch value changes; it never runs in the GTK thread.
    - Overdue rule (`p1-topic-scheduler-setup.md` section 3.2): a job that missed its window by more than one period runs once shortly after startup unless `restore_in_progress` is set.

### T-11a-008 — Dangling category id pruning (CategoryPreferencesCleanupMigration)

- Priority: P1
- Effort: S
- Depends: T-08a-003
- Sources: app/src/main/java/mihon/core/migration/migrations/CategoryPreferencesCleanupMigration.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/DeleteMangaCategory.kt
- Acceptance:
    - `prune_dangling_category_ids(prefs, manga_ids, anime_ids)` deletes `default_category` and `default_anime_category` when their id is in neither set (reset to `-1`).
    - Each of the 10 category-id sets loses exactly `ids - manga_ids - anime_ids`; sets without dangling ids are not written.
    - Called after category deletion and after restore, not at startup.
    - Test: deleting category `7` removes `7` from every set that held it and leaves `0` (the system category) in place.

### T-11a-009 — Post-upgrade changelog gate

- Priority: P2
- Effort: S
- Depends: T-01a-043
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
    - The changelog is shown once after a run that returned `Ok(true)` and never on a fresh install (`old == 0`) or in debug builds (`p1-behavior.md` section 5).
    - The gate is a boolean returned by `run_blocking`; no separate preference is written.

### T-11a-010 — Legacy normaliser core (RawPrefs, RawValue, NormaliseReport, driver)

- Priority: P2
- Effort: M
- Depends: T-02a-034, T-11a-002
- Sources: app/src/main/java/mihon/core/migration/migrations/MigrationUtils.kt, core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt, core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt
- Acceptance:
    - `normalise(&mut RawPrefs) -> NormaliseReport` runs groups A, B, C, D, E, F, G, H in that order, and never fails.
    - `take!(raw, "key", RawValue::Int(v))` returns `Some(v)` for a matching kind and records `WrongType` otherwise.
    - The report lists `converted`, `skipped` (with reason `DestinationPresent`, `WrongType`, `Invalid`) and `repo_sets`, as used by the import dialog of area:02-preferences-system.
    - Running it on the backup's own map leaves the live store untouched (test: a failing group does not modify the live store).

### T-11a-011 — Group A: library sort, filters and chapter/episode flags

- Priority: P2
- Effort: M
- Depends: T-11a-010, T-08a-003
- Sources: app/src/main/java/mihon/core/migration/migrations/MigrateSortingModeMigration.kt, app/src/main/java/mihon/core/migration/migrations/ResetSortPreferenceRemovedMigration.kt, app/src/main/java/mihon/core/migration/migrations/MergeSortTypeDirectionMigration.kt, app/src/main/java/mihon/core/migration/migrations/RenameEnumMigration.kt, app/src/main/java/mihon/core/migration/migrations/MigrateToTriStateMigration.kt, app/src/main/java/mihon/core/migration/migrations/MigrateTriStateMigration.kt, app/src/main/java/mihon/core/migration/migrations/MoveChapterPreferencesMigration.kt, domain/src/main/java/tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt, domain/src/main/java/tachiyomi/domain/library/anime/model/AnimeLibrarySortMode.kt
- Acceptance:
    - Sort ints 0..8 (manga and anime separately) map per `MigrateSortingModeMigration.kt:29-51`; any other value maps to `ALPHABETICAL`; direction `true` becomes `ASCENDING`, absent or `false` becomes `DESCENDING` only when the legacy key exists.
    - Anime sort `LAST_CHECKED` becomes `LAST_ANIME_UPDATE`, `UNSEEN` and `UNREAD` become `UNSEEN_COUNT`, `DATE_FETCHED` becomes `EPISODE_FETCH_DATE` (the original produced names the anime deserialiser does not read; see T-11a-028).
    - Filters `pref_filter_library_*` (manga) and `pref_filter_animelib_*` (anime) are converted to `TriState` enum names under `${key}_v2`; the values written by build 52 are not overwritten.
    - Test: `MergeSortTypeDirection` is a no-op when either sort string is absent, and the legacy `library_sorting_ascending` key is kept in that case.

### T-11a-012 — Group B: update scheduling, restrictions and backup interval

- Priority: P2
- Effort: S
- Depends: T-11a-010
- Sources: app/src/main/java/mihon/core/migration/migrations/RemoveOneTwoHourUpdateMigration.kt, app/src/main/java/mihon/core/migration/migrations/RemoveQuickUpdateMigration.kt, app/src/main/java/mihon/core/migration/migrations/CombineUpdateRestrictionMigration.kt, app/src/main/java/mihon/core/migration/migrations/DontRunJobsMigration.kt, app/src/main/java/mihon/core/migration/migrations/EnableAutoBackupMigration.kt
- Acceptance:
    - Interval `1` or `2` becomes `3`; `3`, `4`, `6`, `8` become `12`; all others unchanged (`pref_library_update_interval_key`).
    - Legacy `pref_update_only_non_completed_key = false` removes `manga_ongoing` from `library_update_manga_restriction`.
    - `battery_not_low` is removed from `library_update_restriction`.
    - `backup_interval = 0` becomes `12`; scheduling is left to T-11a-004 (the always-run step).

### T-11a-013 — Group C: reader orientation, reading mode and navigation

- Priority: P2
- Effort: S
- Depends: T-11a-010
- Sources: app/src/main/java/mihon/core/migration/migrations/MigrateRotationViewerValuesMigration.kt, app/src/main/java/mihon/core/migration/migrations/ResetRotationMigration.kt, app/src/main/java/mihon/core/migration/migrations/RemoveReaderTapMigration.kt
- Acceptance:
    - `pref_rotation_type_key` 1..5 map to the `ReaderOrientation` flag values (1 FREE, 2 PORTRAIT, 3 LANDSCAPE, 4 LOCKED_PORTRAIT, 5 LOCKED_LANDSCAPE, else FREE); the new key is written only when the legacy key exists.
    - `pref_default_viewer_key` becomes `pref_default_reading_mode_key` only when present; no default value is written (quirk 4 fixed).
    - `reader_tap = false` sets both navigation modes to 5; `reader_tap = true` changes nothing.

### T-11a-014 — Group D: player settings (orientation, debanding, aspect, subtitles, skip length, progress)

- Priority: P2
- Effort: M
- Depends: T-11a-010
- Sources: app/src/main/java/mihon/core/migration/migrations/EnumsMigration.kt, app/src/main/java/mihon/core/migration/migrations/PlayerPreferenceMigration.kt, app/src/main/java/mihon/core/migration/migrations/MovePlayerPreferencesMigration.kt, app/src/main/java/mihon/core/migration/migrations/VideoPlayerPreferenceMigration.kt, app/src/main/java/mihon/core/migration/migrations/VideoOrientationMigration.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
    - `pref_deband` 0..2 maps to `Debanding` (`None`, `CPU`, `GPU`); any other value keeps `None` (the Kotlin throw is not reproduced).
    - `pref_player_view_mode` 0..2 maps to `VideoAspect` (`Crop`, `Fit`, `Stretch`); other values keep `Fit`.
    - `gpu_next = true` becomes `pref_gpu_next = true`.
    - `pref_sub_select_conf` JSON with `lang`, `whitelist`, `blacklist` lists (`#[serde(default)]`) becomes three comma-joined strings; malformed JSON is skipped and reported.
    - `pref_progress_preference` String parses to Float; `pref_skip_length_preference` String parses to Int; a non-numeric value is skipped.

### T-11a-015 — Group E: language codes

- Priority: P2
- Effort: S
- Depends: T-11a-010
- Sources: app/src/main/java/mihon/core/migration/migrations/PrefLangMigration.kt
- Acceptance:
    - `pref_audio_lang` and `pref_subtitle_lang` are split on `,`, trimmed, empty items dropped, and each item kept only if `isolang` recognises it as an ISO 639-1/2/3 code.
    - Test: `"eng,jpn"` stays; `"en, ja"` becomes `"en,ja"`; `",,en,,"` becomes `"en"`; `"xx,en"` becomes `"en"`; `"zzz"` becomes `""`.

### T-11a-016 — Group F: UI, security, DoH, navigation and miscellaneous settings

- Priority: P2
- Effort: S
- Depends: T-11a-010
- Sources: app/src/main/java/mihon/core/migration/migrations/AddAllLangMigration.kt, app/src/main/java/mihon/core/migration/migrations/DOHMigration.kt, app/src/main/java/mihon/core/migration/migrations/MigrateSecureScreenMigration.kt, app/src/main/java/mihon/core/migration/migrations/NavigationOptionsMigration.kt, app/src/main/java/mihon/core/migration/migrations/RelativeTimestampMigration.kt, app/src/main/java/mihon/core/migration/migrations/SplitPreferencesMigration.kt
- Acceptance:
    - `source_languages` gains `all` only when the set is already set.
    - `enable_doh = true` sets the DoH provider to `1` (Cloudflare) and removes `enable_doh`.
    - `secure_screen = true` and `secure_screen_v2` absent gives `secure_screen_v2 = "ALWAYS"`.
    - `bottom_nav_style` 0, 1 and other map to `MOVE_HISTORY_TO_MORE`, `MOVE_UPDATES_TO_MORE`, `MOVE_MANGA_TO_MORE`; `default_home_tab_library` true gives `start_screen = MANGA`, otherwise `ANIME`.
    - `relative_time = 0` sets `relative_time_v2 = false`; `pref_theme_mode_key` is uppercased.

### T-11a-017 — Group G: extension repository URL lists

- Priority: P2
- Effort: S
- Depends: T-11a-010
- Sources: app/src/main/java/mihon/core/migration/migrations/TrustExtensionRepositoryMigration.kt, app/src/main/java/mihon/core/migration/migrations/ExternalRepoMigration.kt
- Acceptance:
    - Anime repo URL `X` maps to `X` with a trailing `/index.min.json` or `/index.json` removed and `/repo.json` appended, named `Repo #N` (N is 1-based after sorting).
    - Manga repo entries that are already `https://` URLs are kept as-is; `user/repo` shorthand becomes `https://raw.githubusercontent.com/user/repo/repo`.
    - Both legacy sets are removed from `state` only after the repo rows are written; the sort is lexicographic (quirk 16).

### T-11a-020 — Drop: Android cache-directory and cover-file steps

- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/mihon/core/migration/migrations/InternalChapterCacheDirMigration.kt, app/src/main/java/mihon/core/migration/migrations/CoverToExternalFileMigration.kt, app/src/main/java/mihon/core/migration/migrations/DeleteExternalChapterCacheDirMigration.kt
- Acceptance:
    - No Rust code path moves or deletes Android `cache_disk_cache`, `chapter_disk_cache` or `cover_disk_cache` directories.
    - Loss documented: none. Covers and chapter cache re-download on demand; the cache directory under `Paths::cache()` is created fresh.

### T-11a-021 — Drop: MyAnimeList logout steps

- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/mihon/core/migration/migrations/ForceMALLogOutMigration.kt, app/src/main/java/mihon/core/migration/migrations/LogOutMALMigration.kt
- Acceptance:
    - No Rust step logs MyAnimeList out; login state comes only from the keyring-backed tracker store (area:06-data-repos-history-track-source-misc).
    - Loss documented: an Aniyomi user who imports a backup containing a MAL token must log in again if the token is rejected by the new flow (the same effect the Kotlin steps forced).

### T-11a-022 — Drop: WorkManager and background-job cleanup steps

- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/mihon/core/migration/migrations/SetupBackgroundTasksMigration.kt, app/src/main/java/mihon/core/migration/migrations/UseWorkManagerMigration.kt, app/src/main/java/mihon/core/migration/migrations/RemoveBackgroundJobsMigration.kt
- Acceptance:
    - No Rust step cancels WorkManager tags (`UpdateChecker`, `ExtensionUpdate`); the in-process scheduler (T-11a-005, T-11a-006) owns library jobs.
    - `automatic_ext_updates` is ignored by the normaliser (Kurobayomi has no APK updater).

### T-11a-023 — Drop: app-state, private-key and trust re-key steps; ignored legacy keys

- Priority: P3
- Effort: S
- Depends: T-11a-010
- Sources: app/src/main/java/mihon/core/migration/migrations/NoAppStateMigration.kt, app/src/main/java/mihon/core/migration/migrations/PrivatePreferenceMigration.kt, app/src/main/java/mihon/core/migration/migrations/PermaTrustExtensionsMigration.kt, app/src/main/java/mihon/core/migration/migrations/MigrationUtils.kt
- Acceptance:
    - The normaliser ignores the un-prefixed app-state keys (`pref_download_only`, `incognito_mode`, `last_catalogue_source`, `trusted_signatures`, `last_app_closed`, `library_update_last_timestamp`, `library_unseen_updates_count`, `last_used_category`, `last_app_check`, `last_ext_check`, `last_version_code`, `storage_dir`) and reports each as ignored.
    - `replacePreferences` has no Rust counterpart; no re-keying is performed.

### T-11a-024 — Drop: MIUI installer fallback and tracking-queue rewrite

- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/mihon/core/migration/migrations/MigrateSecureScreenMigration.kt, app/src/main/java/mihon/core/migration/migrations/MovePlayerPreferencesMigration.kt
- Acceptance:
    - `DeviceUtil.isMiui` has no Rust equivalent; `extension_installer` is never set to `LEGACY` by a migration.
    - The Android `tracking_queue` SharedPreferences file is not read. Loss documented: a pending tracker push from before this build is rebuilt from history, not replayed.

### T-11a-026 — Video orientation table (Android ActivityInfo to PlayerOrientation)

- Priority: P2
- Effort: S
- Depends: T-11a-010
- Sources: app/src/main/java/mihon/core/migration/migrations/VideoOrientationMigration.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
    - Int to enum table: 10 Free, 1 Portrait, 9 ReversePortrait, 0 Landscape, 8 ReverseLandscape, 7 SensorPortrait, 6 SensorLandscape, every other value Free.
    - A String value under the key is removed and skipped (the `ClassCastException` branch); default when absent is `SensorLandscape`.

### T-11a-027 — Version bookkeeping: store last_version_code only on success

- Priority: P0
- Effort: S
- Depends: T-01a-013
- Sources: app/src/main/java/mihon/core/migration/Migrator.kt, app/src/main/java/mihon/core/migration/MigrationCompletedListener.kt, app/src/main/java/mihon/core/migration/MigrationStrategy.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
    - `last_version_code` (app-state key) is written only when `run` returns `Ok(true)` and `dry_run` is false; `Ok(false)` and `Err` leave it unchanged (test: a failing step reruns on next launch).
    - `old >= new` (downgrade or same build) never writes the key.
    - `run_blocking` is called once from the GTK `startup` handler before any window is built; no global mutable result (the Kotlin `Migrator.result` singleton is not reproduced).
    - Test: a crash in the middle of a range does not store the new version.

### T-11a-028 — Non-reproduction tests for upstream defects

- Priority: P2
- Effort: S
- Depends: T-11a-011, T-11a-014, T-11a-017
- Sources: app/src/main/java/mihon/core/migration/migrations/RenameEnumMigration.kt, app/src/main/java/mihon/core/migration/migrations/MigrateToTriStateMigration.kt, app/src/main/java/mihon/core/migration/migrations/MigrateTriStateMigration.kt, app/src/main/java/mihon/core/migration/migrations/EnumsMigration.kt, app/src/main/java/mihon/core/migration/migrations/ExternalRepoMigration.kt, app/src/main/java/mihon/core/migration/migrations/MoveChapterPreferencesMigration.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
    - Anime sort fixtures (`LAST_CHECKED`, `UNSEEN`, `DATE_FETCHED`) reach the anime deserialiser with the anime names; the Kotlin fallback to `ALPHABETICAL` does not occur.
    - A boolean `pref_filter_library_downloaded_key = true` survives the migration as `ENABLED_IS` (the Kotlin build 52 then 99 path reset it to `DISABLED`).
    - `ExternalRepoMigration` applied to an already-wrapped URL leaves it unchanged (Kotlin double-wraps it).
    - `EnumsMigration` with `pref_deband = 5` keeps `None` instead of throwing.
    - Test list lives in `p1-topic-upstream-quirks.md`; each item has a fixture.

### T-11a-029 — Drop register: user-visible list of settings not carried over

- Priority: P3
- Effort: S
- Depends: T-11a-010
- Sources: app/src/main/java/mihon/core/migration/migrations/Migrations.kt
- Acceptance:
    - The import report (area:02-preferences-system dialog) lists, by stable key, every legacy key that is dropped with a reason: cache-directory keys, MAL login keys, WorkManager tags, `automatic_ext_updates`, MIUI installer, `tracking_queue`, un-prefixed app-state keys, `trusted_signatures`, `pref_mangasync_*` and `track_token_*`.
    - The list is generated from the same table as the normaliser, so the two cannot drift (test: every legacy key in `p1-data-and-config.md` section 2 is either converted or listed).
