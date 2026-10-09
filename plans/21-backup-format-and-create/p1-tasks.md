# Area 21: Backup format and create, implementation tasks

Every scope file (38 in total, see p1-inventory.md) is covered by at least one task. Priorities follow the project rule: backup create, format and automatic backup are parity features (P1); Android-only or dead code is P3; the optional background run and the Android-only extension binaries are P2.

Shared-owner notes: the restore side is area:22-backup-restore. Scheduling is shared with area:26-library-update-jobs (see p1-topic-autobackup-scheduling.md section 7).

### T-21a-001 — Protobuf wire codec (varint, fixed, length-delimited, unknown skip)
- Priority: P1
- Effort: M
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/Backup.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDetector.kt
- Acceptance:
  - `src/backup/wire.rs` implements `Writer`, `Reader`, `WireType`, `WireError` and the `Message` trait exactly as p1-rust-design.md section 3.
  - Unit tests in p1-edge-cases-and-tests.md 4.1 all pass, including the 10-byte negative int64 case and the fixed32/fixed64 little-endian cases.
  - A payload with an unknown field number (999) decodes identically to the same payload without it.
  - Field 0, a varint longer than 10 bytes and a length prefix past the buffer return errors without panicking (fuzzed with at least 10,000 random inputs).
  - A bad enum ordinal returns `BadEnum`; a missing required field returns `MissingField`.

### T-21a-002 — Container schema: Backup and LegacyBackup message types
- Priority: P1
- Effort: M
- Depends: T-21a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/models/Backup.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt
- Acceptance:
  - `schema.rs` defines `Backup` with exactly the field numbers 1, 2, 101, 104, 105, 106, 500, 501, 502, 503, 504, 505, 506 and `LegacyBackup` with 1, 2, 3, 4, 101, 103, 104, 105, 106, 107, 108, 109 (p1-data-and-config.md sections 1.1 and 1.2).
  - Encoding a default `Backup` writes `is_legacy` (500) as false.
  - Golden test: the `Backup` field-number table printed by the code equals the table in p1-data-and-config.md section 5.

### T-21a-003 — Manga entry schema and SQL row mappers
- Priority: P1
- Effort: M
- Depends: T-21a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupManga.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupChapter.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupHistory.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupTracking.kt
- Acceptance:
  - `BackupManga` (1..109), `BackupChapter` (1..12), `BackupHistory` (1..3), `BackupTracking` (1..100) match p1-data-and-config.md sections 2.1-2.4 field for field, including defaults and nullability.
  - Decoding `viewer_flags` (103) takes precedence over `viewer` (14) when present; encoding writes `viewer = viewer_flags & 0x7`.
  - Tracking decode uses `mediaIdInt` only when non-zero, else `mediaId` (test with both).
  - Float fields (chapterNumber, lastChapterRead, score) are float32 on the wire.
  - Golden bytes of p1-topic-protobuf-wire-format.md section 4 are reproduced exactly for a `BackupManga` with defaults (under the assumption confirmed in T-21a-034; both variants stored as fixtures).

### T-21a-004 — Anime entry schema and SQL row mappers
- Priority: P1
- Effort: M
- Depends: T-21a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnime.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupEpisode.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnimeHistory.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnimeTracking.kt
- Acceptance:
  - `BackupAnime` (1..507 as listed in p1-data-and-config.md 2.5), `BackupEpisode` (1..503), `BackupAnimeHistory` (1..3), `BackupAnimeTracking` (1..100) match the tables field for field.
  - `seasonNumber` defaults to -1.0 and is fixed64 on the wire; `memo` defaults to bytes `7B 7D`.
  - `background_url` (500) is written when the anime has a background (E-26); a test with an anime that has a background round-trips it.
  - Malformed memo JSON in the database writes `{}` and logs a warning; the backup succeeds (E-28).
  - `episodeNumber` and `lastEpisodeSeen` are float32; `totalEpisodes` overflow fails the backup (E-14).

### T-21a-005 — Category, source, store, repo, custom button and extension schema
- Priority: P1
- Effort: S
- Depends: T-21a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupCategory.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupSource.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnimeSource.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtensionRepos.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtensionStore.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtension.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupCustomButtons.kt
- Acceptance:
  - `BackupCategory` (1 name, 2 order, 3 id, 100 flags), `BackupSource` (1, 2), `BackupAnimeSource` (1, 2), `BackupExtensionRepos` (1..5), `BackupExtensionStore` (1..8), `BackupExtension` (1, 2), `BackupCustomButtons` (1..6) match p1-data-and-config.md section 3.
  - `BackupExtensionStore.isLegacy` is nullable (absent when None).
  - `BackupCustomButtons` with a missing field fails decode with `MissingField`.
  - Byte-equal `pkgName`+`apk` values compare equal (custom equality as in `BackupExtension.kt:11-27`).

### T-21a-006 — Preference schema: BackupPreference, PrefValue and source preferences
- Priority: P1
- Effort: S
- Depends: T-21a-001, T-21a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupPreference.kt
- Acceptance:
  - `BackupPreference` (1 key, 2 value) and `BackupSourcePreferences` (1 sourceKey, 2 prefs) match p1-data-and-config.md section 4.
  - `PrefValue` has exactly six variants (Int, Long, Float, String, Boolean, StringSet) and each round-trips.
  - The nested tags of the six variants come from the fixture-verified constants (T-21a-034); until verified, encoding a preference returns a clear error and the backup is not produced (no guessed numbers ship).

### T-21a-007 — Format sniffing and gzip container read
- Priority: P1
- Effort: S
- Depends: T-21a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt
- Acceptance:
  - `sniff` returns `Gzip` for `1F 8B`, `JsonForbidden` for `7B 7D`, `7B 22`, `7B 0A`, `Raw` otherwise, and `Truncated` for fewer than two bytes (test table in p1-edge-cases-and-tests.md 4.2).
  - Reading a gzip file uses `MultiGzDecoder` and returns the complete payload; a truncated gzip stream returns an error (not a panic).
  - A JSON file (`{"`) is rejected with the key text "JSON backup not supported" and is never parsed.
  - Reading a raw protobuf file (no gzip) succeeds.

### T-21a-008 — Legacy (pre-1.0) detection
- Priority: P1
- Effort: S
- Depends: T-21a-001, T-21a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDetector.kt
- Acceptance:
  - A partial decode reads only fields 500 and 103 and returns `is_legacy = (500 absent or true) and count(103) > 0`.
  - The table in p1-edge-cases-and-tests.md 4.2 (`legacy_detect_*`) passes.
  - A current Kurobayomi file (500 = false) is never classified as legacy.
  - Malformed input returns "not legacy" (matches `SerializationException -> false`), not an error.

### T-21a-009 — LegacyBackup to Backup conversion
- Priority: P1
- Effort: S
- Depends: T-21a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/models/Backup.kt
- Acceptance:
  - `LegacyBackup::into_current` copies every list and sets `is_legacy = false`, with the field mapping of p1-data-and-config.md 1.2 (legacy 3 -> anime, 106 -> extensions, 107 -> anime stores, 108 -> manga repos, 109 -> custom buttons).
  - A legacy fixture with one anime, one extension and one custom button yields the same counts after conversion.
  - Test: a legacy payload that also contains field 106 decodes its extensions into `extensions`, not into `manga_extension_repos`.

### T-21a-010 — BackupOptions model (defaults, bool array, canCreate, disable rules, private reset)
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt
- Acceptance:
  - `BackupOptions::default()` has the 10 true values and `private_settings = false`, `extensions = false` (B-02).
  - `as_bool_array` order matches the index table in p1-behavior.md B-02; `from_bool_array` round-trips all 4096 combinations; a short array returns an error.
  - `can_create` matches the 64-combination test in p1-edge-cases-and-tests.md 4.7 (extensions alone is not enough).
  - Setting `library_entries = false` does not reset `chapters`, `tracking`, `history`, `read_entries` (toggle keeps children).
  - Setting both `app_settings` and `source_settings` to false resets `private_settings` to false (E-27).

### T-21a-011 — Create Backup screen (options, gating, save dialog, toast)
- Priority: P1
- Effort: M
- Depends: T-21a-010, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt
- Acceptance:
  - Three section cards (Library 6 rows, Settings 5 rows, Extensions 1 row) with the labels and defaults of p1-ui-parity.md section 2.1.
  - The enable rules in p1-ui-parity.md section 2.1 are applied live (Chapters/Tracking/History/All read-watched disabled when Library entries is off; sensitive settings disabled unless App or Source settings is on).
  - The Create button is disabled unless `can_create()`; Ctrl+S and Enter activate it when enabled.
  - Clicking Create while a manual job is running shows "Backup is already in progress" and opens no dialog.
  - Choosing a file starts the job and closes the screen; cancelling the dialog keeps the screen open with the same selection.
  - The screen does not persist the selection between visits.

### T-21a-012 — Backup build pipeline (BackupCreator.backup)
- Priority: P1
- Effort: L
- Depends: T-21a-013, T-21a-014, T-21a-015, T-21a-016, T-21a-017, T-21a-018, T-21a-020, T-21a-021, area:05-data-repos-entries-items, area:06-data-repos-history-track-source-misc
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt
- Acceptance:
  - `build_backup` runs all reads inside one read transaction (E-05), and does not run the non-favourite queries when `library_entries` is false (E-09).
  - Option matrix test (p1-edge-cases-and-tests.md 4.3 `option_matrix_gating`) passes: each option alone produces exactly the sections of B-06 "Creator gating summary".
  - Output `Backup` has `is_legacy = false` and the source lists derived from the entries (first-seen distinct).
  - Integer overflow in any converted field fails with an error naming the row id (E-14).
  - Creating from a 10,000-manga seeded database completes; peak memory is recorded in the test report (E-41 documentation).

### T-21a-013 — Manga entries creator (excluded scanlators, chapters, categories, tracking, history)
- Priority: P1
- Effort: M
- Depends: T-21a-003, area:05-data-repos-entries-items, area:06-data-repos-history-track-source-misc
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/MangaBackupCreator.kt
- Acceptance:
  - Excluded scanlators are written regardless of the `chapters` option (E-25).
  - Chapters use the unfiltered query (`applyScanlatorFilter = 0`) and are omitted when empty.
  - Category links are category `order` values, never ids; the system category (id 0) is never linked.
  - History rows carry the chapter URL and `readDuration`; rows whose `last_read` is null are written with time 0.
  - Test `history_uses_urls` and `category_link_is_order` from p1-edge-cases-and-tests.md 4.3 pass.

### T-21a-014 — Anime entries creator (episodes, categories, tracking, history, background)
- Priority: P1
- Effort: M
- Depends: T-21a-004, area:05-data-repos-entries-items, area:06-data-repos-history-track-source-misc, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeBackupCreator.kt
- Acceptance:
  - Writes every field listed in B-08b.1 including `parent_id`, `id`, `season_flags`, `season_number`, `season_source_order`, `fetch_type` and `memo`.
  - Writes `background_url` (E-26) when the anime has a background; leaves it absent otherwise.
  - Episode history writes `url` and `lastRead` only (`readDuration` stays 0).
  - Non-favourite watched anime are included only when `read_entries` and `library_entries` are both on.

### T-21a-015 — Category creators (manga and anime)
- Priority: P1
- Effort: S
- Depends: T-21a-005, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/MangaCategoriesBackupCreator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeCategoriesBackupCreator.kt
- Acceptance:
  - Categories with id 0 are excluded; all others are written with name, order, id and flags.
  - Duplicate `order` values produce a warning in the log and are still written (E-15).
  - Empty category list writes an empty repeated field (no error).

### T-21a-016 — Source creators and stub name rule
- Priority: P1
- Effort: S
- Depends: T-21a-005, area:09-domain-source-track-download-release-misc
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/MangaSourcesBackupCreator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeSourcesBackupCreator.kt
- Acceptance:
  - Only sources referenced by written entries are listed, distinct, in first-seen order (test `source_list_distinct_first_seen`).
  - An unknown source id is written with name "" (E-17) and the restore preflight shows the numeric id instead.
  - Installed sources use their display name.

### T-21a-017 — Extension store and repo creators
- Priority: P1
- Effort: S
- Depends: T-21a-005, area:18-extensions-manga, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeExtensionStoresBackupCreator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/MangaExtensionRepoBackupCreator.kt
- Acceptance:
  - Anime stores map `index_url, name, badge_label, signing_key, contact.website, contact.discord, is_legacy, extension_list_url` to the fields 1..8 listed in p1-data-and-config.md 3.5.
  - Manga repos map `base_url, name, short_name, website, signing_key_fingerprint` to fields 1..5.
  - Both are gated by the single `extension_stores` option (B-02, E-related gating note).

### T-21a-018 — Custom buttons creator
- Priority: P1
- Effort: S
- Depends: T-21a-005, area:06-data-repos-history-track-source-misc
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/CustomButtonBackupCreator.kt
- Acceptance:
  - Writes name, is_favorite, sort_index, content, long_press_content, on_startup; never writes the row id.
  - Gated by `custom_button`.
  - Round-trip test: 3 buttons with empty `on_startup` decode back with `on_startup = ""`.

### T-21a-019 — Installed extension artifacts creator (opt-in)
- Priority: P2
- Effort: M
- Depends: T-21a-005, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/ExtensionsBackupCreator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtension.kt
- Acceptance:
  - Off by default (`extensions` option false). When on, one `BackupExtension` per installed extension, anime first then manga, with the extension id as `pkgName` and the artifact bytes as `apk`.
  - A removed artifact is skipped with a warning and the rest of the backup is written (E-16). The completion notification mentions the number skipped.
  - Documented replacement: Android APKs are replaced by Kurobayomi extension artifacts; the user loses Android APK packages (stated in the settings info text).

### T-21a-020 — App preferences creator with key rules
- Priority: P1
- Effort: M
- Depends: T-21a-006, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt
- Acceptance:
  - Keys with prefix `__APP_STATE_` are never written (test `app_prefs_exclude_app_state_and_private`).
  - Keys with prefix `__PRIVATE_` are written only when `private_settings` is on.
  - Value variants are preserved (Int, Long, Float, String, Boolean, StringSet); unsupported types cannot occur in Rust (E-43).
  - The backup includes `backup_interval` as a normal preference (Aniyomi parity).

### T-21a-021 — Source preferences creator
- Priority: P1
- Effort: S
- Depends: T-21a-006, T-21a-020, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt
- Acceptance:
  - Writes one `BackupSourcePreferences` per configurable source with at least one non-app-state preference, `source_key` = `source_<id>`.
  - Sources with zero preferences are omitted (test `source_prefs_only_non_empty`).
  - Gated by `source_settings`; private keys by `private_settings` (same rule as T-21a-020).

### T-21a-022 — Output file write (.part, fsync, rename, overwrite)
- Priority: P1
- Effort: S
- Depends: T-21a-012, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt
- Acceptance:
  - Writes `<name>.part` in the destination directory, `sync_all`, then renames to the final name; no final file exists before the rename (test `write_uses_part_then_rename`).
  - A manual target that exists is replaced completely (no trailing bytes from a longer old file).
  - Any failure removes the `.part` file (test `failure_removes_part`).
  - GTK dialog results without a local path (non-file URIs) are written through `gio::FileOutputStream` (documented fallback).

### T-21a-023 — Post-write self-check
- Priority: P1
- Effort: S
- Depends: T-21a-007, T-21a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupFileValidator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt
- Acceptance:
  - After writing, the `.part` file is decoded with the same reader as restore; a decode error deletes the file and fails the backup (test `self_check_failure_deletes`).
  - Missing sources and trackers do NOT fail creation (only decode errors do), matching Aniyomi.
  - Self-check runs before the rename (so a bad file never gets the final name).

### T-21a-024 — Restore preflight: missing sources and logged-out trackers
- Priority: P1
- Effort: M
- Depends: T-21a-007, T-21a-016, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupFileValidator.kt
- Acceptance:
  - `preflight` returns manga sources first then anime sources, each distinct and sorted; stub label rule per B-13.2 (tests `preflight_*` in p1-edge-cases-and-tests.md 4.5).
  - Trackers with `logged_in = false` are listed; unknown tracker ids are ignored.
  - Decode failures return the localized key text, not `java.io.IOException: ...` (E-12).
  - Empty-string source names never appear in the output; the numeric id is shown instead (E-17).

### T-21a-025 — Automatic folder, retention and startup cleanup
- Priority: P1
- Effort: M
- Depends: T-21a-022, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt
- Acceptance:
  - Automatic files are named per T-21a-036 and pruned after the new file is renamed (E-04); after a run the directory holds exactly 4 matching files, the newest 4 (test `prune_keeps_four_newest`, `prune_after_write`).
  - Files that do not match the pattern are never touched (test `prune_ignores_non_matching`).
  - The directory is created on demand; if the base folder is unavailable the run fails with `Io` and one warning is shown (E-21).
  - At startup, `*.tachibk.part` files in the automatic directory are deleted (E-30).

### T-21a-026 — Automatic backup scheduler (interval, due rule, deferrals, backoff)
- Priority: P1
- Effort: M
- Depends: T-21a-027, T-21a-032, area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt
- Acceptance:
  - `decide(...)` matches the decision table in p1-edge-cases-and-tests.md 4.6 (all 14 rows).
  - First run after enabling is due at enable time plus the interval (E-08); a backup is run at most once per due time.
  - Deferred by restore (10 min retry), battery below 15% on battery power, and manual run in progress.
  - Failed runs retry with 10 min * 2^(n-1) capped at the interval; `last_auto_epoch_ms` does not advance on failure (E-10).
  - A clock in the future is treated as now (E-31).
  - Interval 0 cancels the loop's pending run (no further runs until a non-zero value is set).

### T-21a-027 — Manual job and single-flight slot
- Priority: P1
- Effort: M
- Depends: T-21a-012, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt
- Acceptance:
  - One `JobSlot` shared by manual and automatic runs; a second manual start returns `InProgress` immediately (test `second_manual_refused`).
  - An automatic tick is skipped (not queued) while the slot is held (test `auto_tick_skipped_during_manual`).
  - The slot is released on success, on error and on panic (tests `slot_released_*`).
  - Progress events reach the GTK thread through `async_channel` and never touch GTK from another thread.

### T-21a-028 — Create notifications (progress, complete, error) on freedesktop notifications
- Priority: P1
- Effort: S
- Depends: T-21a-027
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt
- Acceptance:
  - Progress notification "Creating backup" is shown at the start, updated in place by id, and closed on completion or failure.
  - Manual success shows "Backup created" with the display path and "Show in folder"; manual failure shows "Backup failed" with the reason.
  - Automatic success is silent; automatic failure shows the warning only after 3 consecutive failures (p1-ui-parity.md section 3).
  - If the notification daemon is absent, the error is logged at warn level and ignored (E-22).

### T-21a-029 — Restore notifications used by the restore side
- Priority: P1
- Effort: S
- Depends: T-21a-028, area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt
- Acceptance:
  - Restore progress uses "Restoring backup" or "Syncing library", determinate bar, and a "Cancel" action.
  - Restore completion text uses the plural rule "Done in %1$s with %2$s error(s)" and the duration format `%1$02d min, %2$02d sec` (test `restore_duration_format` and `plural_errors`).
  - "Tap to see details" is added only when errors > 0 and a log path exists.

### T-21a-030 — Honour hide_notification_content for restore text only
- Priority: P1
- Effort: S
- Depends: T-21a-029, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt
- Acceptance:
  - With `hide_notification_content = true`, restore progress body is empty; backup texts are unaffected (test `hide_content_restore_only`).
  - The default is false.

### T-21a-031 — Last automatic backup timestamp and info text
- Priority: P1
- Effort: S
- Depends: T-21a-012, T-21a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt, domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt
- Acceptance:
  - `state.backup.last_auto_epoch_ms` is written only after a verified automatic run (B-10), never by manual runs.
  - The Backup and restore group shows "Last automatically backed up: <relative time>" and "Never" when the value is 0 (p1-ui-parity.md section 6).
  - The value is an app-state key and is excluded from backups (T-21a-020).

### T-21a-032 — Automatic backup interval setting
- Priority: P1
- Effort: S
- Depends: T-21a-026, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt, domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt
- Acceptance:
  - `backup.interval_hours` accepts only {0, 6, 12, 24, 48, 168}; default 12; any other stored value loads as 12 (E-13).
  - Changing it updates the loop through `tokio::sync::watch` and takes effect without restart.
  - Changing from 0 to non-zero sets `enabled_at_epoch_ms` (E-08); changing to 0 cancels the pending run.
  - Labels are "Off", "Every 6 hours", "Every 12 hours", "Daily", "Every 2 days", "Weekly" (B-04).

### T-21a-033 — Decision record: drop dead backup code and dead preference
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/full/models/Backup.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/full/models/BackupPreference.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtensionPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnimeSource.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupSource.kt, domain/src/main/java/tachiyomi/domain/backup/service/PreferenceValues.kt
- Acceptance:
  - The `full/models` containers, `BackupExtensionPreferences`, `BrokenBackupSource` and `BrokenBackupAnimeSource` are not implemented in Kurobayomi; the decision is recorded in p1-inventory.md rows 19, 20, 24, 31, 37.
  - The `backup_flags` preference is not created in Kurobayomi config.
  - A grep of the Kurobayomi tree for `BackupExtensionPreferences` and `backup_flags` returns nothing.

### T-21a-034 — Fixtures and wire-compatibility verification (sealed PrefValue, enum ordinals, defaults)
- Priority: P1
- Effort: M
- Depends: T-21a-001, T-21a-002, T-21a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupPreference.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupAnime.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupManga.kt
- Acceptance:
  - Fixtures listed in p1-edge-cases-and-tests.md 4.10 are committed with a MANIFEST (app version, date, option set). The Aniyomi-produced files come from the user; this task is blocked until they are supplied.
  - The sealed `PrefValue` tags (K-9) and the enum ordinal rule (K-4) and the default-encoding rule (K-6) are confirmed from the fixtures and the schema dump, and recorded as constants in `schema.rs`.
  - `golden_decode_current_all` passes: entry counts, tracking remote ids, source names, preference variants and enum values equal the manifest.
  - Kurobayomi-produced `current-all` backup re-decoded by the reader equals the in-memory structure.
  - The open items of p1-topic-protobuf-wire-format.md section 5 are closed or explicitly listed as known differences.

### T-21a-035 — Backup schema text for the debug screen
- Priority: P3
- Effort: S
- Depends: T-21a-002, T-21a-034
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/BackupSchemaScreen.kt
- Acceptance:
  - `schema_text::TEXT` lists all 13 fields of `Backup` with the correct numbers (the mockup's three-field text is replaced; p1-ui-parity.md section 4).
  - A test compares the field numbers in `TEXT` with the table in p1-data-and-config.md section 1.1.
  - The screen is reachable from the debug settings and offers a Copy action.

### T-21a-036 — File naming and collision suffix
- Priority: P1
- Effort: S
- Depends: T-21a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt
- Acceptance:
  - Automatic names are `kurobayomi_<yyyy-MM-dd_HH-mm-ss>.tachibk` in local time; the hand-written pattern check accepts the seconds form and the `-2`, `-3` collision form (test `regex_dot_escaped`, `name_collision_suffix`).
  - Manual suggested name is `kurobayomi_<yyyy-MM-dd_HH-mm-ss>.tachibk`.
  - Two runs with a fixed clock in the same second produce distinct files.

### T-21a-037 — Storage base directory dependency for automatic backups
- Priority: P1
- Effort: S
- Depends: T-21a-025, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt, domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt
- Acceptance:
  - Default automatic directory is `$XDG_DATA_HOME/kurobayomi/autobackup` (fallback `~/.local/share/kurobayomi/autobackup`); a user-set base directory moves it to `<base>/autobackup` (matching `StorageManager.AUTOMATIC_BACKUPS_PATH`).
  - A missing or unreadable base directory fails the run with the folder-unavailable warning (E-21).
  - The settings page displays the resolved path (p1-ui-parity.md section 1, row 5 text is extended with the folder path, Proposal).

### T-21a-038 — Error mapping and message keys
- Priority: P1
- Effort: S
- Depends: T-21a-007, T-21a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupFileValidator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt
- Acceptance:
  - Every `BackupError` variant maps to exactly one key in p1-behavior.md B-16 (test enumerates the variants).
  - Truncated files, corrupt gzip and wire errors all map to "Backup file is corrupted" (E-01, E-11).
  - No user-visible text is produced from `Display` of an internal error (`thiserror` strings are logs only).

### T-21a-039 — Migration equivalents for automatic backup setup
- Priority: P1
- Effort: S
- Depends: T-21a-026, area:11-app-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt, app/src/main/java/mihon/core/migration/migrations/EnableAutoBackupMigration.kt, app/src/main/java/mihon/core/migration/migrations/SetupBackupCreateMigration.kt
- Acceptance:
  - On the first start of a Kurobayomi version that adds the feature, an interval of 0 becomes 12 (Aniyomi migration `84f` rule, applied to Kurobayomi's config only if the key is absent).
  - Each start re-applies the loop configuration (the `ALWAYS` behaviour of `SetupBackupCreateMigration`).
  - Restoring a backup re-applies the interval (`PreferenceRestorer` parity; restore side in area:22-backup-restore).

### T-21a-040 — Test suite for wire, creator, files, scheduler and fixtures
- Priority: P1
- Effort: M
- Depends: T-21a-001, T-21a-012, T-21a-026, T-21a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt
- Acceptance:
  - `cargo test` runs the unit tests of p1-edge-cases-and-tests.md 4.1-4.9 and the integration tests 4.11; all pass on CI with no GTK display and no D-Bus session (pure functions and fakes).
  - Unicode, long-title and float-precision tests (E-07, E-29) pass.
  - The test list in p1-edge-cases-and-tests.md section 5 is fully implemented (no skipped tests).

### T-21a-041 — Background run via systemd user timer and --run-backup entry point
- Priority: P2
- Effort: M
- Depends: T-21a-026, area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt
- Acceptance:
  - `kurobayomi --run-backup` performs at most one automatic run using the same `decide(...)` and config, then exits with status 0 (run or not due) or 1 (failed).
  - A `flock` on `autobackup/.lock` prevents overlap with the GUI loop (R-07); a locked directory skips the run.
  - Unit files of p1-topic-autobackup-scheduling.md section 5 are installed by the package; the app does not enable them itself.
  - Test: with a clock past the due time, the entry point writes exactly one file.

### T-21a-042 — "Show in folder" action through FileManager1
- Priority: P1
- Effort: S
- Depends: T-21a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt
- Acceptance:
  - The action calls `org.freedesktop.FileManager1.ShowItems` with the `file://` URI of the backup file on the session bus.
  - If the call fails, the parent folder opens through `gio::AppInfo::launch_default_for_uri`.
  - The action runs off the GTK thread and its result reaches GTK through `async_channel`.

### T-21a-043 — Read Mihon-shaped backups as current format
- Priority: P1
- Effort: S
- Depends: T-21a-007, T-21a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupManga.kt
- Acceptance:
  - A Mihon backup fixture (manga only, fields 1, 2, 101, 104) decodes with the current `Backup` message, with manga entries, categories and sources intact.
  - `viewer` (14) is used when `viewer_flags` (103) is absent.
  - Preflight of the Mihon fixture reports no anime sources and no anime trackers.

### T-21a-044 — Sensitive settings policy (private settings)
- Priority: P1
- Effort: S
- Depends: T-21a-010, T-21a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt
- Acceptance:
  - The "Include sensitive settings (e.g., tracker login tokens)" option is off by default and enabled only when App settings or Source settings is on.
  - With it on, the keys `__PRIVATE_*` (tracker tokens, usernames and passwords, auth-expired flags listed in p1-behavior.md B-08h) are written; with it off, none are written.
  - The Create screen and the manual backup text state that the file contains credentials when this option is on (Proposal string: "This backup contains login tokens. Keep it private.").
