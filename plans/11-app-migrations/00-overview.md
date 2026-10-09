# Area 11: app-migrations (overview)

Area lead document. Every path is repo-relative to `/home/blackbox/aniyomi-src` unless it starts with `plans/` or `crates/`. Kurobayomi crate paths are the ones fixed by the area documents (`kuro-core`, `kuro-prefs`, `kuro-platform`).

## 1. Purpose and scope

**What Aniyomi does here.** `mihon/core/migration/` is a small in-app framework that runs versioned upgrade steps at process start. The stored build number `__APP_STATE_last_version_code` (Int, default 0) is compared with `BuildConfig.VERSION_CODE` (`versionCode = 132`, `app/build.gradle.kts:22`, `versionName = "0.18.2.1"`). Every step whose version falls in `(old+1)..new`, plus every step marked `ALWAYS = -1f`, runs in version order. The steps are almost entirely SharedPreferences rewrites, Android cache-directory deletions, WorkManager re-registrations and tracker log-outs. There is no SQL migration here; databases migrate through SQLDelight `.sqm` files (area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations).

**Scope size.** 54 files: 7 framework files, 44 registered step classes (one per file), `MigrationUtils.kt`, `Migrations.kt` (the registration list), and one test file `app/src/test/java/mihon/core/migration/MigratorTest.kt` with six `@Test` methods. Counts were re-verified against the tree during this review. Inventory: `plans/11-app-migrations/p1-inventory.md`.

**Why it matters for a usable Kurobayomi.**
1. The framework is P0: `kuro-core::migrate` runs before the first window and records the build number. Without it the app cannot establish a profile version.
2. Three always-run steps are the only part that changes behaviour on a fresh Linux profile. They (re)register the automatic backup job, the manga library-update job and the anime library-update job from current preferences on every launch (`SetupBackupCreateMigration.kt`, `SetupMangaLibraryUpdateMigration.kt`, `SetupAnimeLibraryUpdateMigration.kt`). Without them, "Automatic updates" and "Automatic backup frequency" in settings do nothing.
3. Users who import an old Aniyomi or Tachiyomi backup would silently lose sort modes, filters, reader and player settings. The 29 value-conversion steps therefore survive as a pure "legacy normaliser" (`kuro-prefs::legacy`) that runs on the backup map before import (P2).
4. 11 steps plus 3 key-prefix moves are explicitly dropped (P3) with a stated replacement or loss.

**Out of scope (owned elsewhere).** The job bodies (library update, backup creation) belong to area:26-library-update-jobs and area:21-backup-format-and-create. Backup restore belongs to area:22-backup-restore. Typed settings and the key registry belong to area:02-preferences-system. The `Migrator` framework itself and the startup call site belong to area:01-foundation-app-bootstrap (tasks T-01a-012, T-01a-013, T-01a-043); area 11 owns the migration list contents, the scheduler set-up, the normaliser and the category pruning.

## 2. Module map

### 2.1 Kotlin packages to responsibilities

| Package or file | Responsibility | Files |
|---|---|---|
| `mihon.core.migration` (`app/src/main/java/mihon/core/migration/`) | Framework: `Migration` interface with `version: Float` and `invoke(MigrationContext): Boolean`; `Migrator` singleton holding the result `Deferred<Boolean>` and `Migrator.scope` (`CoroutineScope(Dispatchers.IO + Job())`); `MigrationStrategyFactory.create(old, new)` selects `Initial` (old == 0), `Noop` (old >= new) or `VersionRange` (otherwise); `MigrationStrategy` (`DefaultMigrationStrategy` runs the chain and fires the completion listener when the result is true); `MigrationJobFactory` sorts by version and folds `async` steps with `migration(ctx) \|\| prev`; `MigrationContext(dryrun)` service locator over Injekt; `MigrationCompletedListener` typealias `() -> Unit`. | `Migration.kt`, `MigrationCompletedListener.kt`, `MigrationContext.kt`, `MigrationJobFactory.kt`, `MigrationStrategy.kt`, `MigrationStrategyFactory.kt`, `Migrator.kt` |
| `mihon.core.migration.migrations` (`app/src/main/java/mihon/core/migration/migrations/`) | The 44 steps, the registration list (`Migrations.kt`, a `get()` property that returns 44 fresh instances) and a helper (`MigrationUtils.kt`, `replacePreferences` re-keying by value type). | 46 files |
| `eu.kanade.tachiyomi.App` (`App.kt:166-180`) | Reads `__APP_STATE_last_version_code`, calls `Migrator.initialize`, stores the new code in the completion listener. | outside scope, cited |
| `eu.kanade.tachiyomi.ui.main.MainActivity` (`MainActivity.kt:166`, `:294-312`) | Blocks with `Migrator.awaitAndRelease()` and shows the "Updated to v%1$s" changelog dialog when the result is true and the build is not debug. | outside scope, cited |
| `eu.kanade.tachiyomi.data.backup.create.BackupCreateJob` (`BackupCreateJob.kt:90-114`) | Owns `setupTask`, the periodic `BackupCreator` job. | outside scope, cited |
| `eu.kanade.tachiyomi.data.library.manga.MangaLibraryUpdateJob`, `eu.kanade.tachiyomi.data.library.anime.AnimeLibraryUpdateJob` | Own `setupTask` (`MangaLibraryUpdateJob.kt:439-486`, `AnimeLibraryUpdateJob.kt:452-498`) and `cancelAllWorks`. | outside scope, cited |
| `tachiyomi.core.common.preference` (`core/common/src/main/java/tachiyomi/core/common/preference/`) | `AndroidPreferenceStore` (`:14-16`), `Preference.kt` key prefixes `__APP_STATE_`, `__PRIVATE_` (`:27-48`), `TriState` enum. | outside scope, cited |
| `tachiyomi.domain.*` settings and sort models | `LibraryPreferences.kt` key constants (`:399-406`), `MangaLibrarySortMode.kt`, `AnimeLibrarySortMode.kt`, `Manga.kt:77-103`, `Anime.kt:217-255` flag layouts. | outside scope, cited |

### 2.2 Proposed Rust module map

| Crate and module | Content | Owner |
|---|---|---|
| `kuro-core::app_info` | `VERSION_CODE: u32 = 1` at first release; bumped by +1 for each release that adds a migration. | area:01-foundation-app-bootstrap |
| `kuro-core::migrate` | `Version { Always, At(u32) }` (`Always < At(n)`), `Migration { name, version, run }`, `MigrationContext { dry_run, app }`, `RawPrefStore` trait, `select`, `plan`, `run`, `run_blocking`. | area:01 (framework), contract refined in area:11 |
| `kuro-core::migrate::list` | `static MIGRATIONS: &[Migration]` with the three always-run steps; disposition comment for the 41 legacy steps. | area:11 (T-11a-002) |
| `kuro-core::migrate::steps` | `backup_create`, `anime_library_update`, `manga_library_update` migration bodies that call the scheduler. | area:11 |
| `kuro-core::sched` | `PeriodicSpec`, `Constraints`, `Backoff { Linear, Exponential }`, `JobScheduler` trait (`enqueue_unique_periodic`, `cancel_unique`), `InProcessScheduler` backend. | contract area:11; backend area:26-library-update-jobs |
| `kuro-platform::power_network` | `PowerNetworkState` trait: `network()`, `on_ac()`, `battery_low()`, `subscribe()`. Linux backend: NetworkManager and UPower over `zbus`, fallback `gio::NetworkMonitor`. | contract area:11 (T-11a-007); implementation in `kuro-platform` |
| `kuro-prefs::legacy` | `RawValue`, `RawPrefs`, `NormaliseReport`, `normalise()` running groups `a_library`, `b_update`, `c_reader`, `d_player`, `e_lang`, `f_ui`, `g_repos`, `h_ignored`. | area:02 crate, content area:11 (T-11a-010 to T-11a-017, T-11a-023, T-11a-026) |
| `kuro-prefs::category_prune` | `prune_dangling_category_ids(prefs, manga_ids, anime_ids) -> usize`. | content area:11 (T-11a-008) |

New crates and why (from `p1-rust-design.md` section 1): `isolang` (ISO 639 tables for the language-code rule), `zbus` (used only inside `kuro-platform`), `anyhow` and `thiserror`, `futures`, `tracing` (log-text parity), `rand` (scheduler flex). No GTK dependency is introduced by this area.

## 3. Reading guide

All files are in this folder. Read in this order for an implementer: behaviour, then the catalogue, then the design, then the tasks.

1. [p1-behavior.md](p1-behavior.md): the authoritative behaviour spec of the framework (strategy table, execution chain, `Migrator` lifecycle, log lines, return-value contract) plus the Kurobayomi runtime proposal.
2. [p1-topic-migration-catalogue.md](p1-topic-migration-catalogue.md): the 44 registered steps in run order with the exact keys, values, quirks and dispositions.
3. [p1-topic-upstream-quirks.md](p1-topic-upstream-quirks.md): 20 numbered defects and hazards found in the Kotlin code, and what Kurobayomi does about each.
4. [p1-topic-legacy-normaliser.md](p1-topic-legacy-normaliser.md): the Proposal for the pure conversion functions (groups A to H) that run on imported backups, with the exact rules and table values.
5. [p1-topic-scheduler-setup.md](p1-topic-scheduler-setup.md): the three always-run set-up steps, the WorkManager parameters they reproduce, the `JobScheduler` and `PowerNetworkState` contracts, and the in-process backend rules.
6. [p1-data-and-config.md](p1-data-and-config.md): the preference-key table (every key read or written, its type, default and Kurobayomi disposition), the chapter and episode flag bit layouts, the library sort encodings and all stored enum values.
7. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): the test plan (six upstream tests ported, framework tests, scheduler tests, pruning tests, normaliser fixtures), failure modes and race conditions.
8. [p1-rust-design.md](p1-rust-design.md): the Rust design of types, the registered list, threading, error handling, the Android-to-Linux API map, the normaliser structure, and the hook points into the existing mockup.
9. [p1-inventory.md](p1-inventory.md): the file-by-file inventory of all 54 scope files with port decisions, target modules, priorities and task ids.
10. [p1-tasks.md](p1-tasks.md): the 26 implementation tasks (T-11a-001 to T-11a-029, with 018, 019 and 025 unused), each with priority, effort, dependencies, sources and acceptance criteria.

## 4. Key design decisions and open questions

Each item needs the user's call unless marked as settled. Recommendations are the area lead's.

**Settled (no user call needed).**
- S1. No Android-era step is replayed on a fresh profile. The 41 versioned steps are either converted to pure rules that run only on imported backups (29), dropped with a reason (11), or split (3). Reason: a fresh Linux profile has none of the data these steps transform.
- S2. Every normaliser rule converts only when its legacy key is present and never writes a default (fixes quirks items 4, 5, 9, 14). Reason: the Kotlin steps overwrite user choices on rerun.
- S3. `last_version_code` is never renamed (fixes quirks item 1). A future rename must read the old key as a fallback in the same release.
- S4. The `Migrator` singleton and global mutable result are not reproduced; `run_blocking` returns the bool directly (T-11a-027).
- S5. Filesystem access goes only through `kuro_platform::Paths`; nothing under Android cache or external storage is ported.

**Open questions.**

Q1. Which rule stores `last_version_code`? Kotlin stores it only when the chain result is `true`. Area:01 (`T-01a-013` acceptance and `p1-topic-migrator.md` line 139) stores it explicitly on an empty plan. Area:11 (`T-11a-027`) stores it only on `Ok(true)`. There is also an internal conflict: the always-run steps return `true` in Kotlin even when they cancel a job (interval 0), but `p1-rust-design.md` section 2 rule 7 says `Ok(true)` only if something changed or a schedule was registered, which would make a cancel return `Ok(false)`.
- Recommendation: store the version when `run` completes without error, whatever the bool; keep the changelog gate on `Ok(true)`; always-run steps return `Ok(true)` after a successful call, cancellation included. This removes the empty-plan special case, the cancel problem and the rerun-forever case for a fresh profile. Area:01's `T-01a-013` acceptance and `T-11a-027` must then be amended together.

Q2. What happens on a migration error? Area:01 (`p1-topic-migrator.md` `run_blocking`) says the Rust version panics with the error text. Area:11 (`p1-rust-design.md` section 4) says the crash dialog from area:01 shows the failing migration name and offers `kurobayomi --skip-migrations`, which stores the version code without running anything. The `--skip-migrations` flag is not assigned to any task in any area.
- Recommendation: keep the crash dialog (no panic), add `--skip-migrations` to the CLI parsing of area:01, and assign its behaviour test to `T-11a-027` (already cited in `p1-topic-upstream-quirks.md` item 3). The area:01 `T-01a-012` acceptance wording "panics with the error chain" should change to "returns the error to the crash path".

Q3. Is the backup-import normaliser needed at P2? It matters only if the user imports an Aniyomi or Tachiyomi backup created by an old build. It is P2 because a fresh profile does not need it.
- Recommendation: keep P2 unless the user plans to import existing Aniyomi backups. If he does, raise group A (sort and filters, T-11a-011) and group D (player, T-11a-014) to P1, because those are the settings most likely to be lost.

Q4. Are library updates and backups expected to run while Kurobayomi is closed? WorkManager does; a desktop app does not, unless a helper runs.
- Recommendation: accept "runs only while the app is open, with a catch-up run 60 seconds after the next start" (the in-process backend in `p1-topic-scheduler-setup.md` section 3.1) for the first release. Offer a systemd user timer (`kurobayomi --headless-job library-update`) later through the `SchedulerBackend` trait; area:26-library-update-jobs decides.

Q5. Does "Only on Wi-Fi" accept a wired Ethernet connection?
- Recommendation: yes on desktops (the user's intent is "not on mobile data"). Document it in the settings help text (area:47-settings-screens-general). The rule is in `p1-topic-scheduler-setup.md` section 3.2.

Q6. Anime filter from the legacy manga key. Build 99 wrote `pref_filter_library_unseen` into the manga family, but the anime library reads `pref_filter_animelib_*_v2`. Kotlin loses those anime filters (quirks item 8).
- Recommendation: map `pref_filter_library_unseen` to `pref_filter_animelib_unread_v2` (the normaliser table in `p1-topic-legacy-normaliser.md` section 2 A3). This is a deliberate deviation and should be listed in the import report.

Q7. Tracker log-outs (MAL steps 54 and 121) and the `tracking_queue` rewrite (step 93) are dropped. Loss: a user who imports a backup with a MAL login has to log in again if the token is rejected; a pending tracker push from before the upgrade is rebuilt from history rather than replayed.
- Recommendation: accept. Tracker secrets live in the OS keyring (area:02, area:27-tracking-core).

Q8. Should the import report show dropped keys? `T-11a-029` lists them with reasons in the area:02 import dialog.
- Recommendation: yes, in a collapsed "Not imported" section, so the user can see what was lost.

Q9. Is the Kotlin-era library sort string `"<TYPE>,<DIRECTION>"` kept as the Kurobayomi on-disk form? It comes from area:08 (`T-08a-003` owns the codec).
- Recommendation: keep it for backup interoperability. The normaliser writes exactly this form.

## 5. Cross-area dependencies

### 5.1 What area 11 needs from others

| Area | What is needed | Where it is used |
|---|---|---|
| area:01-foundation-app-bootstrap | `Migrator` framework (`T-01a-012`), the list and bookkeeping hook (`T-01a-013`), the crash dialog (`T-01a-006`, `T-01a-007`), first-frame splash over 300 ms (`T-01a-043`), `Paths`, `AppContext`, the tokio runtime, the logging setup | `T-11a-001`, `T-11a-002`, `T-11a-027`, `T-11a-009` |
| area:02-preferences-system | Raw store with `get_raw`, `set_raw`, `remove`, `all`; `state.toml` for `last_version_code`; typed prefs tree after migrations (`T-02a-034`); the legacy import table that consumes the normaliser (`T-02a-029`) | `T-11a-010`, all groups |
| area:08-domain-library-category-history-updates | `LibrarySort` codec and flag layout (`T-08a-003`); category delete hook that calls `prune_dangling_category_ids` | `T-11a-008`, `T-11a-011` |
| area:21-backup-format-and-create | The backup job body that `setup_backup` schedules | `T-11a-004` |
| area:22-backup-restore | Restore flow that calls the three set-up functions after preferences are restored; `restore_in_progress` flag; import dialog that displays `NormaliseReport` counts | `T-11a-004`, `T-11a-005`, `T-11a-006`, `T-11a-029` |
| area:26-library-update-jobs | `JobScheduler` backend (`InProcessScheduler`) and the library-update job bodies; `cancel_all_works` equivalent | `T-11a-004`, `T-11a-005`, `T-11a-006`, `T-11a-007` |
| area:20-extension-strategy-redesign | Repository tables that receive the converted rows (`LegacyRepoSets`) | `T-11a-017` |
| area:27-tracking-core, area:29-tracker-myanimelist | Owns tracker login state; area 11 only drops the old log-out steps | `T-11a-021` |
| area:12-network-stack | DoH provider ids (`doh_provider`, `1` = Cloudflare) | `T-11a-016` |
| area:36-ui-shell-navigation-security-webview | Secure-screen mode, navigation style and start screen enums; changelog dialog host | `T-11a-016`, `T-11a-009` |
| area:43-reader-core, area:44-reader-viewers | Reader orientation, reading mode and navigation keys | `T-11a-013` |
| area:45-player-core, area:46-player-controls-ui, area:48-settings-player-and-scripts | Player enums and keys: `Debanding`, `VideoAspect`, `PlayerOrientation`, subtitle and progress prefs | `T-11a-014`, `T-11a-026` |
| area:47-settings-screens-general | Settings widgets for "Automatic updates", "Automatic backup frequency", device restrictions; their `on_change` callbacks call the set-up functions | `T-11a-004`, `T-11a-005`, `T-11a-006` |
| area:34-images-covers-cache | Covers directory under `Paths::data()`; the cover-move step is dropped because of it | `T-11a-020` |
| area:35-data-misc-updater-notification-export | Notifications for scheduled runs (the job itself reports through the notification layer) | `T-11a-007` (indirect) |

### 5.2 What others need from area 11

| Consumer | What it gets | Contract |
|---|---|---|
| area:01-foundation-app-bootstrap | The concrete list `MIGRATIONS` and the `Migration` authoring rules (`T-11a-002`), the version-store rule (`T-11a-027`) | `p1-rust-design.md` section 2 |
| area:02-preferences-system | `normalise(&mut RawPrefs) -> NormaliseReport`, group functions, `LegacyRepoSets` | `p1-topic-legacy-normaliser.md` sections 1 and 8 |
| area:08-domain-library-category-history-updates | `prune_dangling_category_ids` (fixes `DeleteMangaCategory` leaving `library_update_categories_exclude` dangling) | `p1-rust-design.md` section 8 |
| area:20-extension-strategy-redesign | Repo rows from `LegacyRepoSets` (manga `base_url`, anime `index_url`) | `p1-topic-legacy-normaliser.md` section 8 |
| area:22-backup-restore | The three `setup_*` functions to call after restore; `normalise` must run on the backup map, not the live store | `p1-rust-design.md` section 5 |
| area:26-library-update-jobs | `PeriodicSpec`, `Constraints`, `Backoff`, `JobScheduler` trait, and the backoff and catch-up rules | `p1-topic-scheduler-setup.md` sections 2 to 3 |
| area:36-ui-shell-navigation-security-webview | `run_blocking` returns the changelog gate as a bool | `p1-behavior.md` section 6 |
| area:47-settings-screens-general | Interval values 0, 12, 24, 48, 72, 168 hours and restriction members `wifi`, `network_not_metered`, `ac` | `p1-rust-design.md` section 5 |

## 6. Task summary

Source: `plans/TASKS.csv` after `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, filtered to rows whose file starts with `11-app-migrations/`. Lint of `p1-tasks.md` prints `ok`.

Totals: 26 tasks. By priority: P0 3, P1 6, P2 11, P3 6. By effort: S 18, M 8, L 0, XL 0.

| Priority | Tasks |
|---|---|
| P0 | T-11a-001 (S), T-11a-002 (S), T-11a-027 (S) |
| P1 | T-11a-003 (M), T-11a-004 (M), T-11a-005 (M), T-11a-006 (M), T-11a-007 (M), T-11a-008 (S) |
| P2 | T-11a-009 (S), T-11a-010 (M), T-11a-011 (M), T-11a-012 (S), T-11a-013 (S), T-11a-014 (M), T-11a-015 (S), T-11a-016 (S), T-11a-017 (S), T-11a-026 (S), T-11a-028 (S) |
| P3 | T-11a-020 (S), T-11a-021 (S), T-11a-022 (S), T-11a-023 (S), T-11a-024 (S), T-11a-029 (S) |

**P0 critical path, in order.**
1. area:01 `T-01a-012` (Migrator framework) and `T-01a-013` (list and bookkeeping). These are not area 11 tasks but gate everything below.
2. `T-11a-001`: port the six Kotlin `MigratorTest` cases to Rust tests of `select`, `plan` and `run`. Verify: `cargo test -p kuro-core migrate` passes all six.
3. `T-11a-002`: `MIGRATIONS` static list with the three always-run entries and the 41-row disposition comment. Verify: the list has three entries, all `Version::Always`, in order backup, anime, manga.
4. `T-11a-027`: store `last_version_code` only on success. Verify: an injected failing step leaves the stored value unchanged, and the next run retries the same range. Depends on the decision in section 4, Q1.

**P1 critical path, in order (after P0).**
1. `T-11a-007`: `PowerNetworkState` and constraint checks in `kuro-platform`. Needs area:26's backend to be runnable.
2. `T-11a-005` and `T-11a-006`: manga and anime library-update set-up. Each verifies with a fake scheduler that interval 6 h plus `wifi` yields one job `LibraryUpdate-auto` with period 6 h and a connected-network constraint.
3. `T-11a-004`: backup set-up, after area:21's job body exists.
4. `T-11a-008`: category pruning. Needs area:08's `T-08a-003`.
5. `T-11a-003`: idempotency harness for every registered step and every legacy rule. Its acceptance refers to rules T-11a-011 to T-11a-017, which are P2 (see risk R4).

**P2 chain.** `T-11a-010` (normaliser core) fans out to groups `T-11a-011` (sort, filters, flags), `T-11a-012`, `T-11a-013`, `T-11a-014`, `T-11a-015`, `T-11a-016`, `T-11a-017` and `T-11a-026`. `T-11a-028` (upstream-defect tests) comes after 011, 014 and 017. `T-11a-009` (changelog gate) needs area:01's `T-01a-043`.

**P3 tasks** are documentation-level drops with tests that assert the key is ignored or no Rust step runs. They can be implemented at any time after `T-11a-010` (for the ignore list in `T-11a-023` and `T-11a-029`).

## 7. Risks

R1. **Fixtures are reconstructed, not real.** The golden fixtures (`tests/fixtures/legacy/*.json`, `p1-edge-cases-and-tests.md` section 5) are built from reading the code, not from real backups. Real Aniyomi backups may carry shapes the code does not predict. Mitigation: ask the user for one real backup from each of the three generations (pre-52, build 64 to 81, build 99) before `T-11a-015` is closed.

R2. **Language-code validity is not verified against the JDK.** Rows for bibliographic codes (`fre`, `ger`, `und`, `mis`, `mul`, `zxx`) are tagged `jdk-unverified`. Mitigation: run the Kotlin predicate in `jshell` once, as the edge-case document describes, before `T-11a-015` is closed. The Rust rule treats every ISO 639-2 code as valid.

R3. **Scheduler behaviour is not WorkManager.** Jobs run only while the app is open (Q4). Flex, backoff and the catch-up rule are reproduced "in spirit", so the first real use may expose timing differences. Mitigation: the scheduler tests in `p1-edge-cases-and-tests.md` section 3 use an injected clock.

R4. **Priority inversion.** `T-11a-003` (P1) verifies rules owned by P2 tasks. If P2 slips, P1 cannot close. Mitigation: either raise `T-11a-011` to `T-11a-017` to P1, or narrow `T-11a-003` to the three always-run steps and `T-11a-001`/`T-11a-002` until P2 lands. This is an owner decision; no priority was changed in this review.

R5. **Fresh-profile testing hides bugs.** A new profile runs only the three always-run steps, so the versioned branches have no real execution path on a real device. Mitigation: the framework tests (`T-11a-001`, `T-11a-027`) and the normaliser fixtures must exercise those branches explicitly.

R6. **Shared key collisions in the Kotlin source.** Quirks item 15 shows that manga and anime chapter display and sort direction share one key string. Area:02 decides the key layout; if it keeps the sharing, the normaliser must read the key once (as written). A wrong choice here silently changes both media types.

R7. **Crash loop on a failing step.** A step that throws on every launch stops startup on every launch (quirks item 3). The Rust design avoids the loop only if the crash dialog offers `--skip-migrations` (Q2) and each step is idempotent (`T-11a-003`).

## 8. Definition of done for area 11

Each item is an observable behaviour in the running Kurobayomi app.

1. A fresh profile starts without a crash. After first start `state.toml` contains `last_version_code` equal to `VERSION_CODE`. A second start logs `Migration from 1 to 1` and runs no step (the Noop strategy).
2. A downgrade (stored value above `VERSION_CODE`) leaves the stored value unchanged.
3. Setting "Automatic backup frequency" to "Every 12 hours" registers one job named `BackupCreator` visible in the scheduler snapshot shown by "Worker info"; setting it to "Off" removes it.
4. Setting "Automatic updates" to "Every 24 hours" with "Only on Wi-Fi" registers one `LibraryUpdate-auto` job and one `AnimeLibraryUpdate-auto` job with a connected-network constraint. Setting it to "Off" removes both.
5. With the app closed for ten days and a 24-hour interval, the next start runs one library update about 60 seconds after the window appears, not ten.
6. Deleting a category removes its id from every category preference set, including the manga "exclude from update" set that Kotlin leaves dangling.
7. Importing an Aniyomi backup that holds anime sort `LAST_CHECKED,DESCENDING` yields `LAST_ANIME_UPDATE,DESCENDING` in the library. The import report shows the number of settings converted, skipped and not imported.
8. The import report lists every dropped legacy key with its reason (cache directories, MAL login, WorkManager tags, `automatic_ext_updates`, MIUI installer, `tracking_queue`, un-prefixed app-state keys, `trusted_signatures`, `pref_mangasync_*`, `track_token_*`).
9. Importing a backup in which a legacy key and its new key are both present keeps the new key's value and reports `DestinationPresent`.
10. An injected failing migration shows the crash dialog naming the migration. Starting once with `--skip-migrations` stores the version code and runs no step.
11. No Android-only key (`extension_installer`, `pref_hwdec`, `pref_invert_playback_txt`, `pref_invert_duration_txt`, `secure_screen`, `enable_doh`, `reader_tap`, `relative_time`) appears in the Kurobayomi settings UI. The last four are legacy names that only the import report mentions.
12. The idempotency harness passes: running every registered step and every legacy rule twice on the same fixture gives byte-identical stores.

## 9. Review notes (this pass)

- Expanded every short `area:NN` token in the area's documents to its full folder id. All area tokens in `plans/11-app-migrations/` now resolve to existing folders. The `_tools/tasks.py` `Depends` fields contained no short tokens.
- Added `area:21-backup-format-and-create` to `T-11a-004`, and `area:26-library-update-jobs` to `T-11a-004`, `T-11a-005`, `T-11a-006` and `T-11a-007`, so the scheduler tasks show their real external dependencies. No task IDs changed.
- Corrected `p1-inventory.md`: the test file has six `@Test` methods, not seven (checked against `app/src/test/java/mihon/core/migration/MigratorTest.kt`).
- Corrected the `--skip-migrations` reference in `p1-topic-upstream-quirks.md` item 3 to point at `T-11a-027`, because no task owned the flag.
- Re-ran `tasks.py lint` on `p1-tasks.md`: `ok`. `tasks.py export` reports 18 errors across the whole plans tree, none in `11-app-migrations/`. Two of those are in area:01 and area:06 (`area:53-platform-manifest-intents-workers` is not a folder).
