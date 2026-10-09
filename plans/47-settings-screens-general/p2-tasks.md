# 47-settings-screens-general — part 2 implementation tasks

Task format is enforced by `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint`. Sources are repo-relative to `/home/blackbox/aniyomi-src`. Mockup paths are under `/home/blackbox/kurobayomi/` and named in acceptance criteria.

Priority reminder: P0 = needed for daily use from a working source (extension stores and repos are the source mechanism). P1 = parity (backup UI, storage, job registry, shared toast/clipboard). P2 = long tail and polish. P3 = Android-only or dropped, with the replacement or drop decision.

### T-47b-001 — Settings router: destinations and start page
- Priority: P0
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/SettingsScreen.kt
- Acceptance:
  - `SettingsDestination` has `About = 0`, `DataAndStorage = 1`, `Tracking = 2`, and an unknown id opens the default page.
  - Default page is the settings main list on narrow layouts and Appearance on wide layouts.
  - `SettingsShell::open(Some(About))` shows the About page directly, with no main list pushed underneath on narrow layouts.
  - Mockup: `/home/blackbox/kurobayomi/src/settings.rs` `body("settings")` routes through the shell, not `main_menu()` directly.

### T-47b-002 — Two-pane settings layout at 720 px
- Priority: P1
- Effort: M
- Depends: T-47b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/SettingsScreen.kt, app/src/main/java/eu/kanade/presentation/util/WindowSize.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt
- Acceptance:
  - `two_pane(width)` returns true iff `width >= 720`, and `two_pane(719)` is false.
  - Resizing across the threshold keeps the open sub-page and does not reset the navigation stack.
  - Wide layout puts the main list in the left pane and the page stack in the right pane.

### T-47b-003 — Settings back navigation
- Priority: P0
- Effort: S
- Depends: T-47b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/SettingsScreen.kt
- Acceptance:
  - Back pops the inner page if one exists, otherwise closes the settings shell.
  - Escape, Alt+Left and the mouse back button trigger the same action.
  - Mockup: sub-pages return to the previous page (not always the root) in `/home/blackbox/kurobayomi/src/settings.rs`.

### T-47b-005 — App language: catalog-derived list
- Priority: P2
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt, buildSrc/src/main/kotlin/mihon/buildlogic/tasks/LocalesConfigTask.kt, app/src/main/AndroidManifest.xml
- Acceptance:
  - The list has a "Default" row (value `""`) first, then one row per translation catalog, sorted by native name in code-point order.
  - Catalog folder `base` maps to `en`; `ka-rGE`, `nb-rNO`, `pt-rBR`, `zh-rCN`, `zh-rTW` map to `ka-GE`, `nb-NO`, `pt-BR`, `zh-CN`, `zh-TW`.
  - Tags that produce an empty display name are dropped; duplicate tags are removed.

### T-47b-006 — App language: native display names and fallback
- Priority: P2
- Effort: S
- Depends: T-47b-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt
- Acceptance:
  - Each row shows the native name as headline and the English name as supporting text; the Default row has no supporting text.
  - Unknown tags show the tag string, never "undefined" or an empty label.
  - `in` shows "Indonesian".

### T-47b-007 — App language: persist choice and restart notice
- Priority: P2
- Effort: M
- Depends: T-47b-005
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt
- Acceptance:
  - Selecting a row writes settings key `app_language` (`""` or tag) and moves the check mark immediately.
  - A toast "Restart Kurobayomi to apply the language." is shown once per change; selecting the already-active row shows no toast.
  - On startup the stored tag is applied before any window is built.

### T-47b-008 — App language: picker page UI
- Priority: P2
- Effort: S
- Depends: T-47b-005, T-47b-006
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt
- Acceptance:
  - Page title is "App language"; back button present; rows are keyboard-navigable (Up/Down, Enter).
  - Mockup: `settings_app_language` in `/home/blackbox/kurobayomi/spec/settings.json` lists the real catalog, not the 7 hard-coded languages in `language()` of `/home/blackbox/kurobayomi/src/settings.rs`.

### T-47b-009 — Anime extension stores: DB access and subscription
- Priority: P0
- Effort: M
- Depends: area:04-db-anime-schema-migrations, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt, data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt, data/src/main/sqldelightanime/dataanime/extension_store.sq, domain/src/main/java/mihon/domain/extension/anime/interactor/RemoveAnimeExtensionStore.kt
- Acceptance:
  - Table `extension_store` exists with columns `index_url` (PK), `name`, `badge_label`, `signing_key`, `contact_website`, `contact_discord`, `is_legacy`, `extension_list_url`, exactly as in p2-data-and-config.md section 2.1.
  - Store list updates live after any insert, upsert or delete (watch channel after each write).
  - Mockup: `settings_anime_extension_stores` shows the DB list, not the static "Store A" and "Store B" cards in `/home/blackbox/kurobayomi/src/settings.rs` `cards()`.

### T-47b-010 — Anime extension stores: page state machine
- Priority: P0
- Effort: M
- Depends: T-47b-009
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreen.kt
- Acceptance:
  - States are `Loading` and `Ready { stores, dialog }`; a list update in `Ready` keeps the open dialog.
  - Loading shows a centred spinner; Ready with zero stores shows the empty state.
  - Reducer is a pure function with unit tests for each transition in p2-behavior.md section 3.2.

### T-47b-011 — Anime extension stores: add flow
- Priority: P0
- Effort: M
- Depends: T-47b-010, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt, data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt, data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt
- Acceptance:
  - Add sets `processing` synchronously before the fetch starts; a second click is ignored.
  - Success upserts the row, reloads available extensions, and closes the dialog.
  - Failure stores the error text in the dialog (`throwable.message`, fallback "unknown error") and re-enables Add.
  - Test: fixture server returns JSON, gzip JSON, legacy `[` array with `/index.min.json`, and garbage bytes; each produces the documented result.

### T-47b-012 — Anime extension stores: refresh all
- Priority: P0
- Effort: S
- Depends: T-47b-009
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresScreen.kt
- Acceptance:
  - Refresh runs only in Ready; it re-fetches every stored index; a failing store is logged and skipped without stopping the others.
  - Header shows a spinner while the refresh runs (improvement over Kotlin, documented in p2-ui-parity.md section 3.2).

### T-47b-013 — Anime extension stores: delete flow
- Priority: P0
- Effort: S
- Depends: T-47b-009
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt, domain/src/main/java/mihon/domain/extension/anime/interactor/RemoveAnimeExtensionStore.kt
- Acceptance:
  - Delete removes the row by `index_url` then reloads available extensions.
  - The dialog closes immediately on OK; no error dialog for delete.

### T-47b-014 — Anime extension stores: deep-link confirm (queued)
- Priority: P2
- Effort: S
- Depends: T-47b-010, T-47b-016
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresDialogs.kt
- Acceptance:
  - A deep link received while Loading is held and opened as the Confirm dialog when Ready (fixes the Kotlin race RACE-01).
  - `alreadyExists` is computed from the exact `indexUrl` match; Add disabled when true.

### T-47b-015 — Anime extension stores: card list
- Priority: P0
- Effort: M
- Depends: T-47b-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresContent.kt
- Acceptance:
  - Each card shows the store name with a tag icon, then buttons in order: globe (Open in browser), Discord (only when `contact.discord` is non-empty), copy (copies `indexUrl`, toast), delete.
  - Globe is disabled when `contact.website` is empty.
  - Mockup: `settings_anime_extension_stores` in `/home/blackbox/kurobayomi/spec/settings.json` lists the Discord button only for stores that have one.

### T-47b-016 — Anime extension stores: add dialog
- Priority: P0
- Effort: S
- Depends: T-47b-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresDialogs.kt
- Acceptance:
  - Entry has purpose URL; helper text is "*required", "This extension store already exists", or the error text.
  - Add is disabled when empty, duplicate, or processing; label "Processing…" while processing; Enter submits when enabled; Escape cancels.
  - Mockup: `add_store` in `/home/blackbox/kurobayomi/spec/settings.json` gains the duplicate and processing states.

### T-47b-017 — Anime extension stores: delete and deep-link dialogs
- Priority: P0
- Effort: S
- Depends: T-47b-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresDialogs.kt
- Acceptance:
  - Remove dialog title "Remove extension store", body `Do you wish to remove the extension store "<name>" (<url>)?`, buttons Cancel and OK (destructive).
  - Deep-link dialog has a read-only URL entry and the same helper rules as the add dialog.

### T-47b-018 — Anime extension stores: page chrome
- Priority: P0
- Effort: S
- Depends: T-47b-010
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresScreen.kt, app/src/main/java/eu/kanade/presentation/category/components/CategoryFloatingActionButton.kt, presentation-core/src/main/java/tachiyomi/presentation/core/util/LazyListState.kt
- Acceptance:
  - Title "Extension stores"; header Refresh with tooltip "Refresh"; Extended "Add" FAB that shrinks on forward scroll and expands at top.
  - Empty state text "You haven't added an extension store yet" with the FAB still visible.

### T-47b-019 — Manga extension repos: DB access and subscription reducer
- Priority: P0
- Effort: M
- Depends: area:03-db-manga-schema-migrations, area:18-extensions-manga
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/MangaExtensionReposScreenModel.kt, data/src/main/sqldelight/data/extension_repos.sq, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/GetMangaExtensionRepo.kt
- Acceptance:
  - Table `extension_repos` has `base_url` (PK), `name`, `short_name`, `website`, `signing_key_fingerprint` (UNIQUE NOT NULL).
  - A DB emission updates the list and keeps the open dialog (fixes RACE-02) unless the dialog's target row was deleted.
  - Loading state is shown until the first emission.

### T-47b-020 — Manga extension repos: create flow and result mapping
- Priority: P0
- Effort: M
- Depends: T-47b-019
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/MangaExtensionReposScreenModel.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt, domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoService.kt
- Acceptance:
  - URL check uses regex `^https://.*/index\.min\.json$` after `url` crate normalisation; `http://` and trailing slash are rejected (table in p2-edge-cases-and-tests.md section 3.4).
  - `InvalidUrl` and `RepoAlreadyExists` produce toasts "Invalid repo URL" and "This repo already exists!"; `DuplicateFingerprint` opens the conflict dialog; `Error` shows toast "Could not add the repo" (deliberate deviation from silent Kotlin).
  - Empty signing fingerprint from `repo.json` is treated as invalid (see T-47b-066).

### T-47b-021 — Manga extension repos: fingerprint conflict and replace
- Priority: P1
- Effort: M
- Depends: T-47b-020
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/ExtensionReposDialogs.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/MangaExtensionReposScreenModel.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/ReplaceMangaExtensionRepo.kt
- Acceptance:
  - Dialog title "Signing Key Fingerprint Already Exists"; body names the new and old repo; buttons Cancel and Replace.
  - Replace removes the old row with the same fingerprint and inserts the new one; Cancel changes nothing.
  - Test: two repos with the same fingerprint, different URLs (fixture `manga-repo-conflict.json`).

### T-47b-022 — Manga extension repos: refresh and delete
- Priority: P0
- Effort: S
- Depends: T-47b-019
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/MangaExtensionReposScreenModel.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/DeleteMangaExtensionRepo.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/UpdateMangaExtensionRepo.kt
- Acceptance:
  - Refresh runs in Ready only and calls the update-all interactor; delete removes the row by `base_url`.
  - Delete triggers an extension list reload (Kotlin does not; Kurobayomi must, for consistency with anime, documented as deviation).

### T-47b-023 — Manga extension repos: page chrome and card list
- Priority: P0
- Effort: M
- Depends: T-47b-019
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/manga/MangaExtensionReposScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/manga/MangaExtensionReposContent.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/MangaExtensionReposScreen.kt
- Acceptance:
  - Title "Extension repos"; header Refresh; Extended "Add" FAB; empty text "You have no repos set.".
  - Each card: name with tag icon; buttons open website (`repo.website`), copy `<baseUrl>/index.min.json`, delete `baseUrl`; no Discord button.

### T-47b-024 — Manga extension repos: add dialog
- Priority: P0
- Effort: S
- Depends: T-47b-020
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/ExtensionReposDialogs.kt
- Acceptance:
  - Body text is `action_add_repo_message` with the app name; entry label "Repo URL"; purpose URL.
  - Duplicate hint compares the typed value to base URLs as Kotlin does, but the real duplicate result comes from the domain toast (documented).
  - Add closes the dialog immediately after submit; Enter submits when enabled; Escape cancels.

### T-47b-025 — Manga extension repos: deep-link confirm (queued)
- Priority: P2
- Effort: S
- Depends: T-47b-019, T-47b-024
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/MangaExtensionReposScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/ExtensionReposDialogs.kt
- Acceptance:
  - A deep link received while Loading is queued and opens `Add repo` confirm dialog body `Do you wish to add the repo "<url>"?` when Ready (fixes RACE-01).
  - Confirming an already-present repo shows toast "This repo already exists!".

### T-47b-026 — Repo events as toasts
- Priority: P0
- Effort: S
- Depends: T-47b-052, T-47b-020
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/MangaExtensionReposScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/MangaExtensionReposScreenModel.kt
- Acceptance:
  - Events `InvalidUrl` and `RepoAlreadyExists` are delivered once each through an unbounded channel, shown as 2 s toasts.
  - Events emitted while no page is listening are not lost while the page is open; a closed page drops them without error.

### T-47b-027 — Backup create: options model
- Priority: P1
- Effort: S
- Depends: area:21-backup-format-and-create
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt
- Acceptance:
  - `BackupOptions::default()` matches p2-data-and-config.md section 10: all true except `private_settings` and `extensions` false.
  - `to_array`/`from_array` use the 12-index order; a round-trip test over all 4096 combinations passes.
  - `can_create()` equals `library_entries || categories || app_settings || extension_stores || custom_button || source_settings`.

### T-47b-028 — Backup restore: options model
- Priority: P1
- Effort: S
- Depends: area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/RestoreOptions.kt
- Acceptance:
  - Seven fields with defaults: all true except `extensions` false; index order per section 10.
  - `can_restore()` is true iff any of the seven is true (including `extensions`).

### T-47b-029 — Backup create: page UI and rules
- Priority: P1
- Effort: M
- Depends: T-47b-027
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt
- Acceptance:
  - Three cards "Library", "Settings", "Extensions" with the labels from p2-behavior.md section 5.1, in the order given there.
  - Chapters, Tracking, History and All read/watched entries are insensitive unless Library entries is on; Include sensitive settings is insensitive unless App or Source settings is on.
  - Create is pinned at the bottom and enabled only when `can_create()`; with only Extensions checked it is disabled.
  - Mockup: `settings_create_backup` is wired in `body()` of `/home/blackbox/kurobayomi/src/settings.rs`.

### T-47b-030 — Backup create: save dialog, busy check, job hand-off
- Priority: P1
- Effort: M
- Depends: T-47b-029, T-47b-044, area:21-backup-format-and-create
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt
- Acceptance:
  - If a job tagged `BackupCreator:manual` is running, toast "Backup is already in progress" and no dialog opens.
  - Otherwise `gtk::FileDialog::save` opens with initial name `<applicationId>_yyyy-MM-dd_HH-mm.tachibk`; cancel changes nothing.
  - On a chosen path the job is enqueued with KEEP semantics and the page pops; options array is passed unchanged (including chapters when library entries is off).

### T-47b-031 — Backup restore: page state machine with async validation
- Priority: P1
- Effort: M
- Depends: T-47b-028, area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupFileValidator.kt
- Acceptance:
  - States `Validating`, `Invalid { chain }`, `Missing { sources, trackers }`, `Valid`; validation runs on tokio, never on the GTK thread (Kotlin validates synchronously in `init`).
  - Validating shows a spinner row and disables Restore; the options card shows only for Missing and Valid.
  - Closing the page during validation cancels the task (RACE-07).

### T-47b-032 — Backup restore: missing sources and trackers computation
- Priority: P1
- Effort: M
- Depends: T-47b-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupFileValidator.kt
- Acceptance:
  - Manga missing sources and anime missing sources are each deduped and sorted (code-point order), then concatenated manga-first.
  - Numeric source ids map to the stub display name; non-numeric names are shown raw.
  - Trackers come from both `backupManga[*].tracking` and `backupAnime[*].tracking`, deduped, unknown ids dropped, logged-in trackers dropped, names sorted.
  - Tests T-UT-05, T-UT-06, T-UT-07 pass with fixtures `missing-numeric.tachibk` and `complete.tachibk`.

### T-47b-033 — Backup restore: error card rendering
- Priority: P1
- Effort: S
- Depends: T-47b-031
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt
- Acceptance:
  - Missing: text "You may need to install any missing extensions and log in to tracking services afterwards to use them.", then bold "Missing sources:" and "Trackers not logged into:" sections with `- item` lines.
  - Invalid: bold "Invalid backup file:", path, blank line, bold "Full error:", message (the cause chain).
  - Text is selectable; headers are bold via Pango markup with escaping.

### T-47b-034 — Backup restore: start job and busy check
- Priority: P1
- Effort: S
- Depends: T-47b-031, area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestoreJob.kt
- Acceptance:
  - Restore enqueues the job with the 7 options and pops the page.
  - If a restore job is already running, toast "Backup is already in progress" and the page stays open (fixes RACE-05).

### T-47b-035 — Drop MIUI optimisation banner
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt
- Acceptance:
  - No MIUI check and no `restore_miui_warning` banner exist in Kurobayomi.
  - Decision recorded in p2-behavior.md section 12 (no user loss on Linux).

### T-47b-036 — Backup restore: options card and Restore rules
- Priority: P1
- Effort: S
- Depends: T-47b-028, T-47b-031
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt
- Acceptance:
  - Seven checkboxes with labels "Library", "Categories", "App settings", "Extension stores", "Custom buttons", "Source settings", "Extensions" in that order; no insensitive entries.
  - Restore enabled iff `Valid` or `Missing`, and `can_restore()`.
  - Mockup: `settings_restore_backup` in `/home/blackbox/kurobayomi/spec/settings.json` gains the invalid-file state (no options card).

### T-47b-037 — Storage roots from mount table
- Priority: P1
- Effort: M
- Depends: area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/StorageInfo.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - Roots come from `/proc/self/mountinfo` filtered to fs types `ext2 ext3 ext4 btrfs xfs f2fs vfat exfat ntfs ntfs3 fuseblk` with `/dev/` sources (or fuseblk), plus the data-dir mount; deduped by device id.
  - Fixture `proc/mountinfo-sample` yields exactly the expected roots (test T-UT-16).

### T-47b-038 — Storage usage bars and states
- Priority: P1
- Effort: S
- Depends: T-47b-037
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/StorageInfo.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - Each root shows path header, 12 px progress bar with fraction `(total - available) / total` clamped to 0..1, and caption "Available: <x> / Total: <y>".
  - `total <= 0` or statvfs error hides the bar and shows "Storage size unavailable" (no bar at total 0 or -1, unlike Kotlin).
  - statvfs call has a 2 s timeout.
  - Mockup: the hard-coded "Available: 12.3 GB / Total: 64 GB" line in `/home/blackbox/kurobayomi/spec/settings.json` `settings_data_storage` is replaced by per-root bars.

### T-47b-039 — SI size formatter
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/StorageInfo.kt
- Acceptance:
  - Table in p2-edge-cases-and-tests.md section 3.1 passes (0 -> "0 B", 1500 -> "1.5 kB", 999999 -> "1.0 MB", 12300000000 -> "12 GB").
  - Labels are `B`, `kB`, `MB`, `GB`, `TB`.

### T-47b-040 — Debug info: app info rows
- Priority: P2
- Effort: S
- Depends: area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt
- Acceptance:
  - Rows "Version" ("Stable <version>", "Debug <sha>" in debug builds, "Preview r<count>" for preview builds), "Build time" (date pref format plus short time), "WebView version" replaced by WebKitGTK row (T-47b-048).
  - Group title "App info" and row titles match the English text in p2-behavior.md section 15.2.

### T-47b-041 — Debug info: device rows for Linux
- Priority: P2
- Effort: S
- Depends: T-47b-040
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt
- Acceptance:
  - "Model" from `/sys/class/dmi/id/sys_vendor` and `product_name` (absent files omitted).
  - "OS" from `PRETTY_NAME` in `/etc/os-release`; "Kernel" from uname; "GTK" from gtk version; "Session" from `XDG_SESSION_TYPE` plus `XDG_CURRENT_DESKTOP`.
  - Each row is copyable (proposal, parity with long-press in Android settings is not required).

### T-47b-042 — Drop ART profile compilation row
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt
- Acceptance:
  - No "Profile compilation status" row exists; the status map in p2-behavior.md section 8.1 is not implemented.
  - Decision recorded in p2-behavior.md section 12.

### T-47b-043 — Worker info page
- Priority: P2
- Effort: M
- Depends: T-47b-044
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/WorkerInfoScreen.kt
- Acceptance:
  - Three sections "Enqueued", "Finished", "Running" with the exact text format of p2-behavior.md section 9.1 and fixture test T-UT-20.
  - Updates live while open; copy action copies the concatenation with the clipboard toast.
  - Mockup: `settings_worker_info` in `/home/blackbox/kurobayomi/src/settings.rs` shows real jobs, not the static "Enqueued/Finished/Running" text dump.

### T-47b-044 — Job registry
- Priority: P1
- Effort: M
- Depends: area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestoreJob.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/WorkerInfoScreen.kt
- Acceptance:
  - Registry records id, tags, state (`ENQUEUED`, `RUNNING`, `SUCCEEDED`, `FAILED`, `CANCELLED`, `BLOCKED`), next run, attempts.
  - `enqueue_unique(name, Keep)` returns the existing job when one with the same name is enqueued or running (test T-UT-19).
  - `is_running(tag)` answers the busy checks in T-47b-030 and T-47b-034.

### T-47b-045 — Backup file schema page
- Priority: P2
- Effort: S
- Depends: area:21-backup-format-and-create
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/BackupSchemaScreen.kt
- Acceptance:
  - Page title "Backup file schema" shows the contents of `proto/backup.proto` verbatim (embedded with `include_str!`).
  - A test checks the field numbers of the proto against the decoder's tags (T-UT-26).
  - Copy action copies the schema with the title as label.

### T-47b-046 — Clipboard copy with confirmation toast
- Priority: P1
- Effort: S
- Depends: T-47b-052
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/ToastExtensions.kt
- Acceptance:
  - Blank content is a no-op with no toast (test T-UT-22 / E72).
  - Non-blank content sets the GDK clipboard and shows "Copied to clipboard:\n<truncated to 50 chars, centre ellipsis>".
  - Clipboard unavailable logs an error and does not crash.

### T-47b-047 — Monospace text page layout
- Priority: P2
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/WorkerInfoScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/BackupSchemaScreen.kt
- Acceptance:
  - Worker info: horizontal scroll, no wrapping, 16 px side padding. Schema: vertical scroll, 16 px padding, selectable text.
  - Both use the `.monospace` style class and are selectable.

### T-47b-048 — WebKitGTK version row
- Priority: P2
- Effort: S
- Depends: T-47b-049
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt
- Acceptance:
  - Row "WebView version" shows the WebKitGTK version from `build.rs` (pkg-config).
  - Missing pkg-config shows "not built with WebKitGTK" and the build still succeeds (test E70).

### T-47b-049 — Build metadata via build.rs
- Priority: P2
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt
- Acceptance:
  - `build.rs` exports `KUROBAYOMI_BUILD_TIME` (RFC 3339 UTC, `SOURCE_DATE_EPOCH` honoured), `KUROBAYOMI_COMMIT_SHA` (fallback `unknown`), `KUROBAYOMI_COMMIT_COUNT` (fallback `0`), and the WebKitGTK version.
  - Missing build time shows "unknown" (test T-UT-25).

### T-47b-050 — Date format preference for info rows and job times
- Priority: P2
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt
- Acceptance:
  - Setting `app_date_format` defaults to `""`; empty uses a localised short date plus short time; a non-empty value is a chrono format string.
  - Build time and worker next-run times use the same formatter.

### T-47b-051 — Drop OneUI and MIUI rows
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt
- Acceptance:
  - No "OneUI version" or "MIUI version" rows exist.
  - Decision recorded in p2-behavior.md section 12 (no Linux loss).

### T-47b-052 — In-window toast overlay
- Priority: P0
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/system/ToastExtensions.kt
- Acceptance:
  - A `gtk::Overlay` + `gtk::Revealer` shows one toast at a time, bottom-centre, auto-hide after 2 s; a new toast replaces the visible one (test E73).
  - No libadwaita toast widgets are used.

### T-47b-053 — Pure-logic unit test suite for this area
- Priority: P1
- Effort: M
- Depends: T-47b-027, T-47b-028, T-47b-031, T-47b-032, T-47b-037, T-47b-038, T-47b-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/RestoreOptions.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupFileValidator.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/data/StorageInfo.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - Tests T-UT-01 to T-UT-26 in p2-edge-cases-and-tests.md section 4 are implemented and pass with `cargo test`.
  - Fixtures from section 5 of that file are committed under `tests/fixtures/settings/`.

### T-47b-054 — Drop Android persistable URI permission
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt
- Acceptance:
  - No URI permission call exists; paths are stored as plain paths or `gio::File` URIs.
  - Decision recorded in p2-behavior.md section 12.

### T-47b-055 — File chooser unavailable message
- Priority: P2
- Effort: S
- Depends: T-47b-030
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt
- Acceptance:
  - If `gtk::FileDialog` fails with no chooser available, toast "No file chooser available" (replaces "No file picker app found" wording).
  - The same wrapper is used by the restore file chooser (part 1 entry point).

### T-47b-056 — Drop list item animations
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresContent.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/manga/MangaExtensionReposContent.kt
- Acceptance:
  - No item insert or remove animation is required; lists update instantly.
  - Decision recorded in p2-behavior.md section 12.

### T-47b-057 — Card context menu and keyboard shortcuts for stores and repos
- Priority: P2
- Effort: S
- Depends: T-47b-015, T-47b-023
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresContent.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/manga/MangaExtensionReposContent.kt
- Acceptance:
  - F5 refreshes, Ctrl+N opens Add, right-click on a card shows the same actions as its buttons.
  - Shortcuts appear in p2-ui-parity.md section 12 and do not conflict with the shell back keys.

### T-47b-058 — Mockup wiring for in-scope settings ids
- Priority: P0
- Effort: S
- Depends: T-47b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/SettingsScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/MangaExtensionReposScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt
- Acceptance:
  - `body()` in `/home/blackbox/kurobayomi/src/settings.rs` dispatches `settings_app_language`, `settings_anime_extension_stores`, `settings_manga_extension_repos`, `settings_create_backup`, `settings_restore_backup`, `settings_worker_info`, `settings_backup_schema`, `settings_debug_info` to this area's modules.
  - Part-1 arms (`cards`, `text_dump`, `licenses`, `clear_db`) are kept unchanged for their screens.

### T-47b-059 — Accessibility basics for this area
- Priority: P2
- Effort: S
- Depends: T-47b-018, T-47b-023
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresDialogs.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/ExtensionReposDialogs.kt
- Acceptance:
  - Every icon-only button has a tooltip equal to the Kotlin content description (p2-behavior.md section 15).
  - Dialogs focus the entry on open, have a default response button, and Escape cancels.
  - Error helper text uses the alert role.

### T-47b-060 — Restore validation cancellation on page close
- Priority: P1
- Effort: S
- Depends: T-47b-031
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt
- Acceptance:
  - Closing the restore page while `Validating` aborts the validation task and drops late results (RACE-07).
  - No warning or toast is shown on cancel.

### T-47b-061 — Storage refresh on page show
- Priority: P2
- Effort: S
- Depends: T-47b-038
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/StorageInfo.kt
- Acceptance:
  - Stats are recomputed each time the containing page is shown (Kotlin computes once per composition).
  - Mounts that disappear between reads show "Storage size unavailable" (RACE-09).

### T-47b-062 — Anime store row duplicate normalisation
- Priority: P2
- Effort: S
- Depends: T-47b-011
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresDialogs.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt
- Acceptance:
  - Duplicate check normalises the URL with the `url` crate before comparing (so a trailing slash difference is detected), and the same normalised value is stored.
  - Documented deviation from Kotlin exact match (E14); test T-UT-14 is updated to the normalised rule.

### T-47b-063 — Refresh skips deleted stores (RACE-03)
- Priority: P2
- Effort: S
- Depends: T-47b-012, T-47b-013
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt, data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt
- Acceptance:
  - A refresh that finishes after a delete does not re-insert the deleted row (test T-IT-05).

### T-47b-064 — Restore busy-state and Create double-click guard (RACE-04, RACE-06)
- Priority: P1
- Effort: S
- Depends: T-47b-030
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresDialogs.kt
- Acceptance:
  - Create is disabled while the save dialog is open; the job registry check and enqueue happen under one lock (RACE-06).
  - Add buttons are disabled synchronously on the first click (RACE-04).

### T-47b-065 — Language selection with stale stored tag
- Priority: P2
- Effort: S
- Depends: T-47b-007
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt
- Acceptance:
  - A stored tag not in the catalog (e.g. `fr-CA` when only `fr` exists) selects the nearest parent row for display; if none, no row is checked and the app uses system locale.
  - Test T-UT-21 covers the fallback.

### T-47b-066 — Reject manga repos with empty signing fingerprint
- Priority: P2
- Effort: S
- Depends: T-47b-020
- Sources: domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoService.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt
- Acceptance:
  - A `repo.json` with an empty `signingKeyFingerprint` maps to `InvalidUrl` (prevents `UNIQUE` collisions on empty strings, E36).
  - Test T-IT-02 covers the empty-fingerprint fixture.

### T-47b-067 — Anime store error messages and network wrapper
- Priority: P2
- Effort: S
- Depends: T-47b-011
- Sources: data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt
- Acceptance:
  - Errors keep the Kotlin legacy message "Provided legacy store url is not valid" verbatim.
  - Network and HTTP errors show "Could not fetch the extension store: <detail>" (documented deviation from raw text, E16).

### T-47b-068 — Manga create error toast
- Priority: P1
- Effort: S
- Depends: T-47b-020, T-47b-052
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/MangaExtensionReposScreenModel.kt
- Acceptance:
  - `CreateMangaExtensionRepo.Result.Error` shows toast "Could not add the repo" (Kotlin: silent, E33).
  - Test T-UT-11 covers the mapping.

### T-47b-069 — Card field rules for empty contact values
- Priority: P2
- Effort: S
- Depends: T-47b-015
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresContent.kt
- Acceptance:
  - Discord button shows only for non-empty `contact.discord`; globe is disabled for empty `contact.website`.
  - Documented in p2-behavior.md section 3.5 as deviation from `!= null` (E22, E23).

### T-47b-070 — Shared clipboard truncation
- Priority: P1
- Effort: S
- Depends: T-47b-046
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - The 50-character centre truncation delegates to the area:13-core-util-storage-archive-torrent `truncateCenter` function; test T-UT-22 covers lengths 49, 50 and 51.

### T-47b-071 — Settings search includes the in-scope pages
- Priority: P2
- Effort: S
- Depends: T-47b-058
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/SettingsScreen.kt
- Acceptance:
  - Searching for "repo", "backup", "language", "storage" or "worker" returns entries from this area with breadcrumb "<group> > <item>" (mockup `search()` in `/home/blackbox/kurobayomi/src/settings.rs`).
