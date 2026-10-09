# 22-backup-restore — Part 1 implementation tasks

All tasks target `/home/blackbox/kurobayomi`. Behaviour references are to `p1-behavior.md` (§ numbers), data to `p1-data-and-config.md`, design to `p1-rust-design.md`, edge cases to `p1-edge-cases-and-tests.md` (E-, X-, F-, R-), defects to `p1-behavior.md` §13 (D1..D24). Dependencies on other plan areas use `area:<id>`; the backup wire format (decode, models, validator, notifier, create side) is area:21-backup-format-and-create.

Coverage: every scope file in `p1-inventory.md` has at least one task. Mapping is in the inventory table.

---

### T-22a-001 — Restore page entry: file selection and route
- Priority: P1
- Effort: M
- Depends: T-22a-002, T-22a-004, area:21-backup-format-and-create
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt, spec/settings.json
- Acceptance:
    - Activating the "Restore backup" action in Settings > Data opens a file dialog titled "Select backup file" (`MR.strings.file_select_backup`) with filters "Aniyomi backup" (`*.tachibk`) and "All files".
    - If a restore is running, the dialog is not opened and the banner text "Restore is already in progress" is shown (`restore_in_progress`).
    - Cancelling the dialog does nothing; a dialog error (not cancel) shows "No file selected" (`file_null_uri_error`).
    - A selected file opens the restore page for that path; the page is reachable with keyboard only.

### T-22a-002 — RestoreOptions model (seven flags, defaults, labels, bit array)
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/RestoreOptions.kt
- Acceptance:
    - `RestoreOptions::default()` is `{library_entries: true, categories: true, app_settings: true, extension_stores: true, custom_buttons: true, source_settings: true, extensions: false}`.
    - `to_bits()` / `from_bits()` round-trip all 128 combinations, index order library, categories, app, stores, buttons, source, extensions.
    - `can_restore()` is false only when all seven are false.
    - Labels are exactly "Library", "Categories", "App settings", "Extension stores", "Custom buttons", "Source settings", "Extensions" in that order.

### T-22a-003 — Decode pipeline used by restore (magic sniffing, legacy detection, error mapping)
- Priority: P1
- Effort: M
- Depends: area:21-backup-format-and-create
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDetector.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/Backup.kt
- Acceptance:
    - Gzip (`1f 8b`), raw protobuf, and the three JSON signatures (`7b 7d`, `7b 22`, `7b 0a`) are handled as in p1-behavior.md §3.1.
    - JSON signatures produce "JSON backup not supported"; any protobuf decode error produces "Backup file is corrupted".
    - Legacy detection matches p1-topic-seasons-and-legacy.md §2.1 on fixtures Q-01, Q-02, Q-03.
    - Empty and one-byte files produce a decode error, never a panic.

### T-22a-004 — Validation: missing sources and trackers
- Priority: P1
- Effort: S
- Depends: T-22a-003, area:17-source-app-layer, area:09-domain-source-track-download-release-misc, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupFileValidator.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt
- Acceptance:
    - Missing manga source names are distinct and sorted; missing anime source names are distinct and sorted; the two lists are concatenated manga first (E-10, X-09).
    - A numeric-only source name is replaced by the stub label from the source layer (X-08).
    - Tracker ids not registered are ignored; registered but logged-out trackers are listed by name, sorted (X-10).
    - A decode failure yields the invalid state with the error text, not "java.lang.IllegalStateException" (D19 fixed).
    - Restore remains enabled when only missing sources or trackers exist (E-12, X-11).

### T-22a-005 — Restore job runner (thread, single-job guard, events)
- Priority: P1
- Effort: M
- Depends: T-22a-006, T-22a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestoreJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt
- Acceptance:
    - `JobSlot::start` runs the restore on a dedicated thread and returns `AlreadyRunning` while another job is active (R-04, X-93).
    - The GTK side receives every `RestoreEvent` through `async_channel` and clears the slot on Finished, Canceled or Failed.
    - Start with an options set of all false is refused with a visible message (E-14, X-13).
    - The job never runs on the GTK main thread (verified by thread name in a debug log).

### T-22a-006 — Progress amounts and step events
- Priority: P1
- Effort: S
- Depends: T-22a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt
- Acceptance:
    - Amount formula matches p1-behavior.md §1.4 step 3, except categories count 2 and emit 2 steps (D2 fixed).
    - Final progress equals amount for every option combination (X-72) and with seasons (X-73).
    - Amount zero produces fraction 1.0 and no division by zero (E-72, X-71).
    - Each step emits one `Progress` event with the section label or entry title (behaviour §1.4 titles).

### T-22a-007 — Ordered section execution (fixes D3)
- Priority: P1
- Effort: M
- Depends: T-22a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt
- Acceptance:
    - Sections run strictly in this order: categories, app settings, source settings, anime entries, manga entries, anime stores, manga repos, custom buttons, extensions.
    - Category membership of entries and the default-category mapping always see the created categories (R-01, R-02, X-90, X-91 pass 200/100 iterations).
    - Cancel is checked before each section and each entry (F-02, X-75).

### T-22a-008 — Anime and manga category restore
- Priority: P1
- Effort: S
- Depends: T-22a-007, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeCategoriesRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaCategoriesRestorer.kt
- Acceptance:
    - Existing categories are matched by exact name and keep their DB flags.
    - New categories get order max+1 (or 0 when empty), in backup order, with backup flags (E-18, X-17).
    - Duplicate names in the backup create one category each only if documented; default PROPOSAL is one (E-16, X-15, decision in T-22a-048).
    - `categorized_display` is set to `(db + created).distinctBy(flags).size > 1` (D23 kept, documented).
    - Returned map backup order to DB id is used by entry restore.

### T-22a-009 — Preference restorer (type gating, category remap, string sets)
- Priority: P1
- Effort: M
- Depends: T-22a-008, area:02-preferences-system, T-22a-051
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
    - A key whose stored type differs from the backup type is not written; an absent key is written (E-53, X-52).
    - `default_category` and `default_anime_category` are remapped backup id to name to DB id; unmapped ids (including -1) leave the setting unchanged (E-54, E-55, X-53, X-54).
    - The twelve category keys are remapped per id; when no id maps, the stored set is not changed (E-57, X-56).
    - Category string-set writes replace the set (D12 fixed; X-55).
    - A failure on one key is logged and does not stop others (F-07, X-80).
    - Source preferences go to the store named `source_<sourceId>` for each backup source entry (E-52, X-51).

### T-22a-010 — Re-arm library and auto-backup schedulers after app prefs restore
- Priority: P1
- Effort: S
- Depends: T-22a-009, area:08-domain-library-category-history-updates, area:21-backup-format-and-create
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
    - After an app-prefs section, `rearm_library_updates()` and `rearm_auto_backup()` are called exactly once each (X-57).
    - An interval of 0 disables the matching timer; a non-zero interval enables it with the restored value.
    - The scheduler trait is defined in p1-rust-design.md §9; no-op implementation is allowed only until the owning areas land.

### T-22a-011 — Manga entry ordering (sort by new)
- Priority: P1
- Effort: S
- Depends: T-22a-007, area:03-db-manga-schema-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, data/src/main/sqldelight/data/mangas.sq
- Acceptance:
    - Entries whose `(source, url)` is not in the DB sort before existing ones; within each group, descending `lastModifiedAt` (p1-behavior.md §4.1).
    - Ordering uses one snapshot query of `(source, url)` per section (no per-entry query).
    - Ties keep backup order (stable sort).

### T-22a-012 — Existing manga merge (metadata direction, favourite OR)
- Priority: P1
- Effort: M
- Depends: T-22a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt
- Acceptance:
    - B newer (`B.version > D.version`): metadata from B, title/viewer/chapter flags/update strategy/date added/last update from D (X-22, E-23). Decision on D5 recorded in T-22a-048.
    - B older or equal: metadata from D, the other fields from B (X-22).
    - `favorite` is always the OR (E-21, X-20); `initialized` is the OR.
    - The merged row keeps the DB `id` and `source` and `url`.

### T-22a-013 — Manga row UPDATE semantics
- Priority: P1
- Effort: M
- Depends: T-22a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, data/src/main/sqldelight/data/mangas.sq
- Acceptance:
    - UPDATE uses coalesce semantics: null arguments keep stored values (X-100 integration, mangas.sq:156-179).
    - `next_update` and `calculate_interval` are not changed by the entry update (fetch interval task T-22a-021 sets them).
    - Genre is written as the comma-space join; empty list writes "" (E-51, X-50).
    - `is_syncing` is written as 1 by the entry update; the decision to reset it is recorded in T-22a-048 (E-25, X-24).

### T-22a-014 — Manga row INSERT and initialized persistence (fix D6)
- Priority: P1
- Effort: S
- Depends: T-22a-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt
- Acceptance:
    - INSERT writes `next_update = 0`, `calculate_interval = 0`, and the `initialized` value that the in-memory object computes (`description != null`) (D6 fixed, X-23).
    - Inserted row has `version` from the backup and `date_added` from the backup.
    - `last_insert_rowid` is read in the same transaction.

### T-22a-015 — Chapter merge rules and forComparison
- Priority: P1
- Effort: M
- Depends: T-22a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt
- Acceptance:
    - Chapter lookup is by url within the manga only (not global).
    - Same state (all fields except id, mangaId, dateFetch, dateUpload, lastModifiedAt, version) skips the write.
    - Existing chapter: name, url, number, upload date and scanlator come from the DB row (E-34, X-33).
    - Read flag never reverts; position follows the decision table in p1-topic-merge-rules.md §2 (E-31, X-30).
    - Bookmark is OR (E-32, X-31). Position regression is kept for parity, decided in T-22a-048 (E-29, X-28).

### T-22a-016 — Chapter writes and float chapter numbers
- Priority: P1
- Effort: S
- Depends: T-22a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, data/src/main/sqldelight/data/chapters.sq
- Acceptance:
    - New chapters INSERT 12 columns (mangaId, url, name, scanlator, read, bookmark, lastPageRead, chapterNumber, sourceOrder, dateFetch, dateUpload, version).
    - Existing chapters UPDATE only read, bookmark, lastPageRead, version, and set is_syncing = 0.
    - Chapter numbers are rounded to six decimal places on restore (E-27 PROPOSAL, X-26 with Q-13: 1.1 stored as 1.1).

### T-22a-017 — Manga category membership
- Priority: P1
- Effort: S
- Depends: T-22a-008, T-22a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, data/src/main/sqldelight/data/mangas_categories.sq
- Acceptance:
    - Backup category orders are mapped to DB categories through names (p1-topic-merge-rules.md §6).
    - If at least one pair maps: all memberships for the entry are deleted, then the mapped pairs inserted; if none map, memberships are untouched (X-14, E-15).
    - Categories off: no membership writes.

### T-22a-018 — Manga history restore (chapter lookup scope)
- Priority: P1
- Effort: M
- Depends: T-22a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, data/src/main/sqldelight/data/history.sq
- Acceptance:
    - Existing history row: `last_read = max(backup, db)` (null when both zero), `time_read = max(backup, db)` through the delta upsert (X-39, E-40, E-41).
    - New history: the chapter is looked up in the current manga only (D9 fixed, X-38, Q-17); entries whose chapter is absent are skipped without error (X-37).
    - Re-running the restore does not change `time_read` or `last_read` (idempotency, X-102).

### T-22a-019 — Manga track restore (merge scope)
- Priority: P1
- Effort: M
- Depends: T-22a-014, area:06-data-repos-history-track-source-misc, area:09-domain-source-track-download-release-misc
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupTracking.kt, data/src/main/sqldelight/data/manga_sync.sq
- Acceptance:
    - New tracks: inserted with the entry id (X-36 for unknown tracker id: kept).
    - Existing tracks: only remoteId, libraryId and lastChapterRead (max) change; status, score, dates, title, totalChapters, private, remoteUrl keep DB values (E-36, X-35).
    - `remoteId` is `mediaIdInt` when non-zero, else `mediaId` (p1-data-and-config.md §2.6).
    - Score and lastChapterRead widen Float to Double as in the Kotlin mapping; re-restore writes zero rows (X-27).

### T-22a-020 — Excluded scanlators restore
- Priority: P1
- Effort: S
- Depends: T-22a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, data/src/main/sqldelight/data/excluded_scanlators.sq
- Acceptance:
    - Only scanlators not already stored for the manga are inserted (X-48).
    - Empty backup list performs no query beyond nothing (no writes).

### T-22a-021 — Fetch interval recomputation after entry restore
- Priority: P1
- Effort: S
- Depends: T-22a-015, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt, app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt
- Acceptance:
    - The fetch window is captured once per run (`now`, local start of day minus and plus one day) (E-47, X-46).
    - `calculate_interval` uses chapter count threshold 8 and the upload-date then fetch-date rules; clamp 1..28 (X-45).
    - DST days use local midnight arithmetic (X-47).
    - Only `next_update` and `fetch_interval` are written by this step.

### T-22a-022 — Anime entry ordering and merge
- Priority: P1
- Effort: M
- Depends: T-22a-012, area:04-db-anime-schema-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt, data/src/main/sqldelightanime/dataanime/animes.sq
- Acceptance:
    - Ordering and existing-merge rules equal the manga rules (T-22a-011, T-22a-012) with `fetch_type`, `parent_id`, `season_flags`, `season_number`, `season_source_order`, `background_url`, `background_last_modified`, `memo` added to INSERT and UPDATE (p1-data-and-config.md §5.4).
    - `parent_id` on UPDATE is set by the season logic (T-22a-023), never by the plain merge (D7 scope).
    - `memo` decodes from the backup `memo` bytes (`{}` default) and is re-encoded on write.

### T-22a-023 — Season restore (fixes D7 and D18)
- Priority: P1
- Effort: M
- Depends: T-22a-022, T-22a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt
- Acceptance:
    - Season rows (backup `parent_id != null`) are not processed as top-level entries; they are restored under their parent with `parent_id` set to the parent's DB id (p1-topic-seasons-and-legacy.md §1.4).
    - Season `parent_id` is correct for both sort orders (Q-11, Q-12, X-42, X-104).
    - A season whose parent id is not in the file becomes a top-level row with `parent_id = NULL` (X-43).
    - Season episodes, history and tracks are restored with the season (X-101 counts).
    - Progress counts each row once (X-73).

### T-22a-024 — Episode merge and writes
- Priority: P1
- Effort: M
- Depends: T-22a-022, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt, data/src/main/sqldelightanime/dataanime/episodes.sq, domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt
- Acceptance:
    - Episode lookup by url within the anime (X-18 style), same-state skip, existing merge (`seen` monotonic, `lastSecondSeen` rule, bookmark OR, fillermark OR) per p1-topic-merge-rules.md §2 (X-30..X-33).
    - Existing episode keeps DB name, url, number, upload date, fillermark, scanlator, summary, preview url (E-34 for episodes).
    - INSERT 17 columns including summary, preview url, fillermark and memo; UPDATE per p1-data-and-config.md §5.3.
    - Position regression kept for parity; decision in T-22a-048 (E-30, X-29).

### T-22a-025 — Anime history, anime tracks, anime categories
- Priority: P1
- Effort: S
- Depends: T-22a-024, area:06-data-repos-history-track-source-misc
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt, data/src/main/sqldelightanime/dataanime/animehistory.sq, data/src/main/sqldelightanime/dataanime/anime_sync.sq, data/src/main/sqldelightanime/dataanime/animes_categories.sq
- Acceptance:
    - Anime history: lookup by episode url (global, D9 fixed scope), `last_seen = max(backup, db)`; `readDuration` is dropped with an informational count (E-42).
    - Anime tracks: same rules as manga tracks with `last_episode_seen` (X-35 variants).
    - Anime categories: membership rules identical to manga (T-22a-017), table `animes_categories`.

### T-22a-026 — Anime extension store restore
- Priority: P1
- Effort: S
- Depends: T-22a-007, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeExtensionStoreRestorer.kt, data/src/main/sqldelightanime/dataanime/extension_store.sq
- Acceptance:
    - UPSERT on `index_url` writes name, badge label (`badgeLabel ?: name`), signing key, contact website, contact discord, is_legacy (`isLegacy ?: true`), extension list url (X-60, X-61).
    - Never reports a duplicate error.
    - Order is backup list order.

### T-22a-027 — Manga extension repo restore (fixes D11)
- Priority: P1
- Effort: S
- Depends: T-22a-007, area:18-extensions-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaExtensionRepoRestorer.kt, data/src/main/sqldelight/data/extension_repos.sq
- Acceptance:
    - Same URL and same fingerprint: success, no insert, no error (D11 fixed, X-62).
    - Same URL and different fingerprint: error "Already Exists with different signing key fingerprint" (X-63).
    - Same fingerprint and different URL: error "<name> has the same signing key fingerprint" (X-64).
    - Errors are recorded as "Error Adding Manga Repo: <name> : <message>".

### T-22a-028 — Custom button restore (fixes D10 and E-17)
- Priority: P1
- Effort: S
- Depends: T-22a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/CustomButtonRestorer.kt
- Acceptance:
    - Name match skips the backup button (no update).
    - New buttons get `sort_index` from max+1 in backup `sortIndex` order.
    - At most one button becomes favourite: the first inserted favourite is kept and the DB already having a favourite blocks all (D10 fixed, X-65).
    - Duplicate names inside the backup create one row only (E-17, X-16).

### T-22a-029 — APK extension payloads: skipped counter (drop)
- Priority: P3
- Effort: S
- Depends: T-22a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/ExtensionsRestorer.kt
- Acceptance:
    - Drop decision recorded: APK payloads are not installed on Linux (p1-behavior.md §10).
    - The summary reports the count of skipped APK entries (X-67) and the completion text does not count them as errors.
    - No file is written to the cache directory.

### T-22a-030 — Error collection and error log file
- Priority: P1
- Effort: S
- Depends: T-22a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt
- Acceptance:
    - Entry errors use "<title> [<source name>]: <message>" with the source name from the backup source list for both manga and anime (D1 fixed).
    - Log file `aniyomi_restore_error.txt` is written in `glib::user_cache_dir()/kurobayomi/`, overwritten each run, only when errors exist (E-70, X-69).
    - Line format `[yyyy-MM-dd HH:mm:ss.SSS] message` with ASCII digits and local time (X-68 covers newline escaping).
    - A write failure is logged as a warning, not swallowed (D15 fixed, X-70 creates the directory).

### T-22a-031 — Completion notification and error-log action
- Priority: P1
- Effort: S
- Depends: T-22a-030, T-22a-046
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt
- Acceptance:
    - Completion text is "Done in {mm} min, {ss} sec" plus the plural "with 1 error" / "with N errors" (behaviour §8.2).
    - With errors, the notification has a "Tap to see details" button that opens the log in the default text viewer (`app.open-error-log`).
    - Failure and cancel notifications use the titles and bodies in design §8.
    - Notification ids and priority follow design §8 (no Android channels).

### T-22a-032 — Cancel semantics (fixes D4 and D17)
- Priority: P1
- Effort: S
- Depends: T-22a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestoreJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt
- Acceptance:
    - Cancel mid-entry rolls the entry back and records no error line (F-01, X-74).
    - Cancel writes the error log of errors collected so far and emits `Canceled { error_log }` (F-03, X-76).
    - Cancel stops after at most one entry (verified by timing test).
    - Cancel notification text "Canceled restore" under title "Restoring backup failed" (behaviour §8.3).

### T-22a-033 — Restore page UI (states, cards, options, action, progress banner)
- Priority: P1
- Effort: M
- Depends: T-22a-001, T-22a-002, T-22a-004, T-22a-040
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt
- Acceptance:
    - States Validating, Invalid, Ready (no missing), Ready (missing) render as in p1-ui-parity.md §1.3.
    - Card texts match X-203 exactly (missing sources and trackers lists, invalid file text with "Full error:").
    - Seven checkboxes with defaults; the Restore button enabled iff `can_restore && options.can_restore()` (X-204).
    - Restore starts the job, closes the page, and shows the progress banner with a Cancel button.
    - Error text is selectable.
    - Keyboard: Tab order is checkboxes then Restore; Space toggles; Enter on Restore starts.

### T-22a-034 — Create backup page (options, enable rules, save dialog)
- Priority: P1
- Effort: M
- Depends: area:21-backup-format-and-create, T-22a-044
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt
- Acceptance:
    - Three cards "Library", "Settings", "Extensions" with the checkbox labels and defaults in p1-ui-parity.md §2.1.
    - Enable rules: Chapters, Tracking, History and All read/watched disabled unless Library entries on; Include sensitive disabled unless App settings or Source settings on (X-200).
    - Create enabled iff `canCreate()` (X-201).
    - Click while a manual create job runs shows "Backup is already in progress" (X-202); otherwise a save dialog with name `kurobayomi_yyyy-MM-dd_HH-mm.tachibk`.
    - Cancelled dialog keeps the page; a successful choice starts the create job and closes the page.

### T-22a-035 — Storage info rows
- Priority: P2
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/StorageInfo.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
    - One row per mounted storage root with path heading, usage bar and caption "Available: X / Total: Y" (X-206).
    - Fraction is `1 - available/total`, clamped to 0.0..=1.0 (X-205); bar hidden when total is unknown or zero (D14 fixed).
    - Sizes use `glib::format_size`.

### T-22a-036 — ComicInfo model and XML (de)serialization
- Priority: P2
- Effort: M
- Depends: area:14-source-local-and-metadata
- Sources: core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt
- Acceptance:
    - Root element `ComicInfo` with attributes `xmlns:xsd` and `xmlns:xsi`; element order as in the Kotlin class (X-210).
    - `PublishingStatusTachiyomi`, `Categories` (prefix ty) and `SourceAniyomi` (prefix ay) are written in the XMLSchema namespace as in the source.
    - Null optional elements are omitted; empty string values are written as empty elements.
    - Parsing a ComicInfo v2.0 file with unknown elements succeeds.

### T-22a-037 — ComicInfo mapping and publishing status
- Priority: P2
- Effort: S
- Depends: T-22a-036
- Sources: core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt
- Acceptance:
    - `getComicInfo` mapping: Series from title, Summary from description, Writer from author, Penciller from artist, Genre from genre, PublishingStatusTachiyomi from status (X-210).
    - `copyFromComicInfo`: genre from genre, tags and categories (distinct, trimmed, ", " join) (X-211); artist from penciller, inker, colorist, letterer, cover artist split on ", " (X-211); status via the enum (X-212).
    - Duplicate-after-trim values stay duplicated (X-213, parity).
    - Status mapping: ONGOING 1, COMPLETED 2, LICENSED 3, PUBLISHING_FINISHED 4, CANCELLED 5, ON_HIATUS 6, UNKNOWN 0 (X-212).

### T-22a-038 — details.json models (local source metadata)
- Priority: P2
- Effort: S
- Depends: area:14-source-local-and-metadata
- Sources: core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/MangaDetails.kt, core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/AnimeDetails.kt, core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/ChapterDetails.kt, core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/EpisodeDetails.kt
- Acceptance:
    - serde structs with the JSON names `chapter_number`, `date_upload`, `preview_url`, `episode_number`, `fillermark` (default false) as in the Kotlin classes.
    - Optional fields are `Option`; `chapter_number` and `episode_number` are required.
    - Round trip (X-216) of fixtures for all four types.

### T-22a-039 — MIUI warning (drop)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt
- Acceptance:
    - Drop recorded: MIUI detection and the warning text `restore_miui_warning` are not implemented; the user loses nothing on Linux.
    - No code path shows the MIUI text (grep test in CI).

### T-22a-040 — In-progress guards (restore and create)
- Priority: P1
- Effort: S
- Depends: T-22a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestoreJob.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt
- Acceptance:
    - `is_running()` is true only while the restore thread is active.
    - Starting a restore while one runs shows "Restore is already in progress" (X-93).
    - The auto-backup timer skips its tick while a restore runs (R-06, X-95) instead of failing.
    - Stop request sets the cancel flag and does not block the GTK thread.

### T-22a-041 — Decode once and reuse between validation and job
- Priority: P2
- Effort: S
- Depends: T-22a-003, T-22a-033
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt
- Acceptance:
    - The validation step and the job share one decoded backup (E-13, X-12): decode count is 1 for a restore started from the page.
    - Memory is released when the page closes without a restore.

### T-22a-042 — Test suite and fixtures
- Priority: P1
- Effort: M
- Depends: T-22a-003, T-22a-007, T-22a-012, T-22a-015, T-22a-022, T-22a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt
- Acceptance:
    - Fixtures Q-01..Q-21 are generated by a Rust generator and each has `expected.json` (p1-edge-cases-and-tests.md §7).
    - Unit tests X-01..X-96 exist (or are recorded as manual with reason) and pass with `cargo test`.
    - Integration tests X-100..X-113 run against an in-memory SQLite created from the areas 03/04 schema.
    - Restore of Q-03 twice produces zero entry writes on the second run (X-102).
    - Manual golden comparison for Q-01..Q-04 is recorded once with the Kotlin app (T-22a-051 provides the bytes).

### T-22a-043 — Localize restore error and status strings
- Priority: P2
- Effort: S
- Depends: T-22a-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaExtensionRepoRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt
- Acceptance:
    - The hard-coded English strings in p1-behavior.md §12 are Kurobayomi message keys with the same English default.
    - Log file text stays English for parity (documented); UI text uses the keys.

### T-22a-044 — Mockup spec wiring for restore and create
- Priority: P1
- Effort: S
- Depends: none
- Sources: spec/settings.json, src/ui.rs, src/settings.rs
- Acceptance:
    - `settings_restore_backup` and `settings_create_backup` use check-button items for their options (p1-ui-parity.md §1.5, §2.5).
    - The settings action "Restore backup" (spec line 2412) starts the file dialog (T-22a-001).
    - Create and restore pages render the data-driven states without placeholder text.

### T-22a-045 — Remove the dead sync mode
- Priority: P3
- Effort: S
- Depends: T-22a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestoreJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt
- Acceptance:
    - Drop recorded: no caller sets `sync = true` (D21); sync titles "Syncing library" and "Library sync complete" are not implemented.
    - The restore has no `isSync` parameter in Rust; no sync strings appear in the codebase.

### T-22a-046 — Close-while-running confirmation and banner behaviour
- Priority: P2
- Effort: S
- Depends: T-22a-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
    - Closing the main window during a restore shows the two-choice dialog ("Keep running", "Cancel restore and close") (design §11).
    - Choosing cancel waits for the next check point, not longer than one entry (verified in X-74 timing).
    - Notification "Cancel" button and banner Cancel both call the same stop path (behaviour §8.1).

### T-22a-047 — Restore file acceptance and content sniffing
- Priority: P1
- Effort: S
- Depends: T-22a-003, T-22a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt
- Acceptance:
    - Any file name is accepted; content decides (Q-21 with a renamed file restores correctly).
    - Non-UTF-8 and space-containing paths work (X-07).
    - Paths are `PathBuf`, not `String`, end to end.

### T-22a-048 — Decision record for behaviour-changing defects
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt
- Acceptance:
    - A decision line exists in this task's acceptance for each of: D5 (metadata direction, keep or fix), D8 (position regression, keep or max), D10/E-17 (custom button duplicates), D16 (chapter metadata, keep), D22 (favourite default, keep), D23 (categorized display, keep), E-25 (is_syncing reset), E-26 (lastModifiedAt and favoriteModifiedAt write), E-16 (duplicate category names), E-04 (zero-content backups rejected).
    - Each decision names the test that encodes it (X-22, X-28, X-65, X-34, X-21, X-24, X-25, X-103, X-101).
    - Decisions are recorded in the task list before implementation of T-22a-012, T-22a-015, T-22a-024, T-22a-028 starts.

### T-22a-049 — Hide notification content preference wiring
- Priority: P2
- Effort: S
- Depends: T-22a-031, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt
- Acceptance:
    - When the preference is true, progress notifications and the banner do not show the current item title (X-203 variant, UI check).
    - Default value and key follow area:02 (`hideNotificationContent`).

### T-22a-050 — Legacy 0.x conversion
- Priority: P1
- Effort: S
- Depends: T-22a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/models/Backup.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDetector.kt
- Acceptance:
    - LegacyBackup field numbers 3, 4, 103 map to anime, anime categories, anime sources; 100 and 102 are not read (p1-topic-seasons-and-legacy.md §2.2).
    - Converted legacy file (Q-02) restores with the counts in expected.json (X-06).
    - Legacy files contain no seasons after conversion (`parent_id` none, X-06 check).

### T-22a-051 — Wire fixtures: sealed PreferenceValue and enum encodings
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupPreference.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt
- Acceptance:
    - `fixtures/restore/prefs_wire.md` records tag numbers and wire types for each of the six PreferenceValue variants and the polymorphic envelope, from a real backup byte dump.
    - Enum ordinals (UpdateStrategy 0/1, FetchType Seasons 0/Episodes 1) are confirmed from the same dump.
    - Unknown field numbers are shown to be skipped (X-03 fixture).
    - prefs decoding (T-22a-009) is not merged until this task is done.

### T-22a-052 — Preference key allowlist on restore (PROPOSAL, security)
- Priority: P2
- Effort: S
- Depends: T-22a-009, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
    - Keys outside the Kurobayomi preference schema, and any key starting with the app-state prefix, are skipped and counted in the summary (E-60, X-59).
    - The skip count is shown in the completion text only when non-zero (PROPOSAL wording).
    - Restoring a backup with only known keys has identical results to the unfiltered restore (X-58 and X-59 variants).

### T-22a-053 — core-metadata build plumbing (drop)
- Priority: P3
- Effort: S
- Depends: none
- Sources: core-metadata/build.gradle.kts, core-metadata/consumer-rules.pro, core-metadata/proguard-rules.pro, core-metadata/src/main/AndroidManifest.xml
- Acceptance:
    - Drop recorded: Gradle, R8 consumer rules, ProGuard template and empty Android manifest have no Linux equivalent; the models live in the `metadata` module of the Cargo crate.
    - The Cargo crate builds without any of these files.
    - Dependency needs (serde, quick-xml) are declared in Cargo.toml only.
