# Area 11 app-migrations, part 1: behavior specification

Scope: the Mihon/Aniyomi in-app migration framework (`app/src/main/java/mihon/core/migration/*.kt`), the 44 registered migration steps (`app/src/main/java/mihon/core/migration/migrations/*.kt`) and the framework test (`app/src/test/java/mihon/core/migration/MigratorTest.kt`). Per-step detail lives in `p1-topic-migration-catalogue.md`; known upstream defects in `p1-topic-upstream-quirks.md`; Linux scheduling for the three always-run steps in `p1-topic-scheduler-setup.md`; old-preference handling for backup import in `p1-topic-legacy-normaliser.md`. The framework itself is also summarised in `plans/01-foundation-app-bootstrap/p1-topic-migrator.md` (area:01-foundation-app-bootstrap owns the startup call site and tasks T-01a-012 and T-01a-013); this file is the authoritative, more detailed statement for the migration code and does not contradict it.

Everything labelled OBSERVED is what the Kotlin does. Everything labelled "Proposal:" is what Kurobayomi should do.

## 1. What "migration" means here

OBSERVED: a migration is a one-shot, idempotent-by-intent upgrade step run at process start when the stored app build number is lower than the running build number. Aniyomi has no migration of SQL data in this framework (the databases migrate through SQLDelight `.sqm` files, see area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations). All 41 versioned steps rewrite Android `SharedPreferences` (the default shared-preferences file), delete Android cache or files directories, cancel or schedule WorkManager jobs, log trackers out, or insert extension-repository rows. There are no user-visible strings, dialogs, notifications or toasts: the only observable outputs are `logcat` lines and the changes they make.

Proposal: Kurobayomi has its own build-number counter starting at 1 (area:01-foundation-app-bootstrap, T-01a-013). The 41 Android-era steps are not replayed on a fresh Linux profile, because a fresh profile has none of the data they transform. Their knowledge is preserved in two ways: (a) the three always-run steps are ported as scheduler set-up; (b) the value conversions that can still matter when a user imports an old Aniyomi/Tachiyomi backup are ported as pure functions (the "legacy normaliser", `p1-topic-legacy-normaliser.md`). Everything else is an explicit drop with a reason (tasks T-11a-020 to T-11a-029).

## 2. Types (OBSERVED)

`app/src/main/java/mihon/core/migration/Migration.kt:3-22`

| Member | Value |
|---|---|
| `val version: Float` | build number at which the step becomes due; `-1f` means "every launch" |
| `suspend operator fun invoke(migrationContext: MigrationContext): Boolean` | the step; return `true` = "applied/changed something", `false` = "could not run or nothing to do" |
| `val isAlways: Boolean` | `version == ALWAYS` |
| `Migration.ALWAYS` | `const val ALWAYS = -1f` |
| `Migration.of(version, action)` | factory wrapping a lambda in an anonymous `Migration`; used only by tests |

`MigrationContext.kt:5-10`: `class MigrationContext(val dryrun: Boolean)` with `inline fun <reified T> get(): T? = Injekt.getInstanceOrNull(T::class.java)`. Every migration pulls its collaborators through `get<T>()` and returns `false` immediately when one is missing (`?: return false`). In production every requested type is registered, so that early return is never taken; it exists so tests can run a step with a partial container.

`MigrationCompletedListener.kt:3`: `typealias MigrationCompletedListener = () -> Unit`.

## 3. Strategy selection (OBSERVED, `MigrationStrategyFactory.kt:8-20`)

`create(old: Int, new: Int)`:

| Condition (checked top to bottom) | Strategy | Migrations passed to the job chain |
|---|---|---|
| `old == 0` | `InitialMigrationStrategy(DefaultMigrationStrategy)` | only those with `isAlways` |
| `old >= new` | `NoopMigrationStrategy(false)` | none; the result is a completed `false`; the listener is never called |
| otherwise | `VersionRangeMigrationStrategy(versions = (old + 1)..new, DefaultMigrationStrategy)` | those with `isAlways || it.version.toInt() in versions` |

Consequences, all of which are covered by tests in `p1-edge-cases-and-tests.md`:
1. `old == 0` (fresh install, or the stored value was lost or never written) skips every versioned step, including ones that would be due. This is correct for a fresh install and wrong for an install whose `last_version_code` key was renamed (see the hazard in `p1-topic-upstream-quirks.md`, item 1).
2. The range is an inclusive Kotlin `IntRange` of the lower bound `old + 1` to the upper bound `new`. A step whose version equals the old stored value does not run again; a step whose version equals `new` runs.
3. `Float.toInt()` truncates toward zero, so a hypothetical version `64.5f` would be treated as 64.
4. `old > new` (downgrade) is the same as `old == new`: nothing runs and nothing is written, so the stored value stays at the higher number.
5. With `old != 0` and a range containing no registered step (for example `old = 130, new = 132` today), the list passed to `DefaultMigrationStrategy` still contains the always-run steps, so they run every launch; the listener then fires because those steps return `true`, and `last_version_code` is written. If instead the filtered list is empty, `DefaultMigrationStrategy` returns a completed `false` and no listener call happens; the stored value is not updated (this cannot happen in Aniyomi because three always-steps exist).

## 4. Execution chain (OBSERVED, `MigrationJobFactory.kt:15-33`, `MigrationStrategy.kt:12-31`)

`DefaultMigrationStrategy.invoke(migrations)`:
1. If `migrations.isEmpty()` return `CompletableDeferred(false)`.
2. `chain = migrationJobFactory.create(migrations)`.
3. `launch { if (chain.await()) migrationCompletedListener() }.start()` on `Migrator.scope` (the `.start()` is redundant because `launch` starts eagerly).
4. Return `chain`.

`MigrationJobFactory.create(migrations)`:
1. `migrations.sortedBy { it.version }`: ascending by float, `-1f` first, stable (equal versions keep registration order).
2. Fold starting with `CompletableDeferred(true)`. For each migration:
   * If `migrationContext.dryrun` is false: `logcat { "Running migration: { name = ${migration::class.simpleName}, version = ${migration.version} }" }` then `async(start = CoroutineStart.UNDISPATCHED) { val prev = acc.await(); migration(migrationContext) || prev }`.
   * If dry run: log `"(Dry-run) Running migration: { name = ..., version = ... }"` and use `CompletableDeferred(true)` (the body is never called).
3. The deferred returned by the last fold step is the chain result.

Semantics derived from that code:
* Sequential. Step N starts only after step N-1 completed (`acc.await()`).
* The result is `true` when at least one step returned `true`. `migration(ctx) || prev` evaluates the step first (no short-circuit skipping), so every step always runs even when an earlier step already returned `true`.
* A `false` return does not stop the chain.
* Dry run returns `true` for the whole chain (the fold seed is `true` and every element is `true`), so in dry-run mode the completion listener still fires; no caller passes `dryrun = true` in the app (`Migrator.initialize` default is `false`; only the parameter exists).
* Failure path (kotlinx.coroutines semantics, not written in this repository): an exception thrown by a step fails that `async`; the next element throws the same exception from `acc.await()`; the exception also cancels `Migrator.scope`'s plain `Job()` (so the scope is dead for the rest of the process); the `launch` in item 3 above receives it from `chain.await()` and, with no `CoroutineExceptionHandler`, it goes to the thread's uncaught-exception handler (the app's `GlobalExceptionHandler`, which opens the crash screen); `MainActivity` calling `Migrator.awaitAndRelease()` rethrows the same exception on the main thread. `last_version_code` is not written, so the whole range is retried on the next launch. Steps that already finished are not rolled back, so each step must tolerate being run twice (see the idempotency column in the catalogue).
* Threading: `Migrator.scope = CoroutineScope(Dispatchers.IO + Job())`. `CoroutineStart.UNDISPATCHED` starts each `async` body immediately on the calling thread; because almost no step suspends and awaiting an already-completed deferred does not suspend, the whole chain normally executes synchronously inside `Migrator.initialize` on the Android main thread (`App.onCreate`). The two steps that call `withIOContext` (`CategoryPreferencesCleanupMigration`, `TrustExtensionRepositoryMigration`) hop to `Dispatchers.IO`; later steps then continue on IO threads. The listener always runs on an IO thread of `Migrator.scope`.

## 5. Migrator lifecycle (OBSERVED, `Migrator.kt:10-41`, `App.kt:166-180`, `MainActivity.kt:166`)

State machine of `object Migrator`:

| Call | Effect |
|---|---|
| `initialize(old, new, migrations, dryrun=false, onMigrationComplete)` | builds `MigrationContext`, `MigrationJobFactory`, `MigrationStrategyFactory`; `result = strategy(migrations)` |
| `await()` | `(result ?: CompletableDeferred(false)).await()` |
| `release()` | `result = null` |
| `awaitAndRelease()` | `runBlocking { await().also { release() } }`; blocks the calling thread |

Startup sequence (App.kt):
1. `initializeMigrator()` is the last statement of `App.onCreate`.
2. `preference = preferenceStore.getInt(Preference.appStateKey("last_version_code"), 0)`; the stored key is therefore `__APP_STATE_last_version_code`.
3. Log `"Migration from ${preference.get()} to ${BuildConfig.VERSION_CODE}"`.
4. `Migrator.initialize(old = preference.get(), new = BuildConfig.VERSION_CODE, migrations = migrations, onMigrationComplete = { logcat { "Updating last version to ${BuildConfig.VERSION_CODE}" }; preference.set(BuildConfig.VERSION_CODE) })`. `BuildConfig.VERSION_CODE` is `versionCode = 132` in `app/build.gradle.kts:22` (`versionName = "0.18.2.1"`).
5. `MainActivity.onCreate` calls `val didMigration = Migrator.awaitAndRelease()` right after `super.onCreate`, before `setComposeContent`. It blocks the UI thread until the chain finishes, then clears the result. `didMigration` is the chain result (`true` when at least one step returned `true`). It drives the post-upgrade dialog described in section 6; a second activity creation (rotation, recreation) returns `false` because `result` is null.

`migrations` (Migrations.kt:5-51) is a property with a getter, so the 44 migration objects are re-created on every access.

Timing: no timeout, no progress reporting, no cancellation. A hung step hangs startup.

## 6. Message and log inventory

User-visible message keys: none are referenced by the scope files. The only user-visible consequence of the framework is in its consumer, `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:294-312` (outside the scope, owned by area:36-ui-shell-navigation-security-webview):

* `var showChangelog by remember { mutableStateOf(didMigration && !BuildConfig.DEBUG) }`. `didMigration` is false only for the Noop strategy (`old >= new`) and for an empty plan; the Initial and Range strategies always include the three always-run steps, which return true. So the dialog shows once on first install and once after each version-code bump, never in debug builds, and not on ordinary relaunches.
* AlertDialog with no body, title `MR.strings.updated_version` = "Updated to v%1$s" formatted with `BuildConfig.VERSION_NAME` (`0.18.2.1`), dismiss button `MR.strings.whats_new` = "What's new" (opens `RELEASE_URL` in the browser: `https://github.com/$GITHUB_REPO/releases/tag/$RELEASE_TAG`, tag `v<versionName>` for release builds, `r<commitCount>` for preview builds, repo `aniyomiorg/aniyomi` or `aniyomiorg/aniyomi-preview`, `AppUpdateChecker.kt:45-59`), confirm button `MR.strings.action_ok` = "OK". Dismissing (outside tap, back, OK) hides it for the session.

Log lines (logcat, default priority DEBUG unless stated):

| Source | Text |
|---|---|
| `App.kt:168` | `Migration from <old> to <new>` |
| `App.kt:176` | `Updating last version to <new>` |
| `MigrationJobFactory.kt:20` | `Running migration: { name = <SimpleName>, version = <float> }` |
| `MigrationJobFactory.kt:28` | `(Dry-run) Running migration: { name = <SimpleName>, version = <float> }` |
| `TrustExtensionRepositoryMigration.kt:27` (ERROR) | `Error Migrating Extension Repo with baseUrl: <source>` (anime) |
| `TrustExtensionRepositoryMigration.kt:44` (ERROR) | `Error Migrating Manga Extension Repo with baseUrl: <source>` |

`<float>` prints through Kotlin's `Float.toString()`: `-1.0`, `7.0`, `64.0`, `130.0`.

Proposal: Rust uses `tracing` with the same message text at `debug` level (`error` for the two repo lines) so logs can be diffed against Aniyomi's.

## 7. Return-value contract and the "version stored" rule

Because the listener fires only when the chain result is `true`, the stored build number advances only when at least one executed step returned `true`. A step that returns `false` because a precondition was missing (for example `VideoPlayerPreferenceMigration` when the old JSON key is empty or malformed) does not by itself prevent the build number from advancing when another step in the same run returns `true`. When all steps return `false` the build number is not written and the same range reruns next launch. Today the always-run steps return `true` whenever the DI lookup succeeds, so the build number always advances after a clean run.

## 8. Shared helper: `replacePreferences` (OBSERVED, `MigrationUtils.kt:5-41`)

Signature: `replacePreferences(preferenceStore, filterPredicate: (Map.Entry<String, Any?>) -> Boolean, newKey: (String) -> String)`.
Algorithm: `preferenceStore.getAll()` (a snapshot map of every stored key to its raw value); keep entries matching `filterPredicate`; for each entry choose by runtime type of the value:

| Value type | Action |
|---|---|
| `Int` | `getInt(newKey(key)).set(value)`; `getInt(key).delete()` |
| `Long` | same with `getLong` |
| `Float` | same with `getFloat` |
| `String` | same with `getString` |
| `Boolean` | same with `getBoolean` |
| `Set<*>` | if the cast to `Set<String>` succeeds: `getStringSet(newKey(key)).set(value)`; `getStringSet(key).delete()`; otherwise nothing |
| anything else (including null) | ignored |

Edge cases: when `newKey(key) == key` the value is written and then deleted, which loses it (not used that way); the destination key is overwritten without checking whether it already exists; the preference store's typed defaults are irrelevant because the type comes from the stored value.

Used by `NoAppStateMigration` (prefix `__APP_STATE_`) and `PrivatePreferenceMigration` (prefix `__PRIVATE_`). Key-prefix semantics (`Preference.kt:27-48`): `appStateKey(k) = "__APP_STATE_" + k`, `privateKey(k) = "__PRIVATE_" + k`, `isAppState`/`isPrivate` are `startsWith` tests. App-state keys are excluded from backups; private keys are excluded unless the user opts in.

## 9. Interactions with other areas

| Area | Interaction |
|---|---|
| area:01-foundation-app-bootstrap | owns `App.onCreate` ordering, the blocking wait before the first window (T-01a-012, T-01a-013, T-01a-043) |
| area:02-preferences-system | provides raw typed store access (`get_raw`, `set_raw`, `remove`), `last_version_code` in `state.toml`, and the namespaced key prefixes; `legacy.rs` table (T-02a-029) is where import-time conversions live; `p1-topic-legacy-normaliser.md` supplies the exact rules |
| area:21-backup-format-and-create | `BackupCreateJob.setupTask`; Rust equivalent is the backup scheduler (T-11a-004) |
| area:26-library-update-jobs | `MangaLibraryUpdateJob.setupTask`, `AnimeLibraryUpdateJob.setupTask`, `cancelAllWorks` (T-11a-005, T-11a-006) |
| area:20-extension-strategy-redesign and area:18-extensions-manga, area:19-extensions-anime | repository tables targeted by `TrustExtensionRepositoryMigration` and `ExternalRepoMigration` conversions (T-11a-015) |
| area:22-backup-restore | `PreferenceRestorer.restoreCategoriesPreference` remaps category ids at restore; the dangling-id cleanup (T-11a-008) is the safety net |
| area:08-domain-library-category-history-updates | `DeleteMangaCategory`/`DeleteAnimeCategory` prune category preference sets; they forget the "exclude" set (see quirks item 12), which is the reason `CategoryPreferencesCleanupMigration` exists |
| area:27-tracking-core, area:29-tracker-myanimelist | `ForceMALLogOutMigration` and `LogOutMALMigration` call `TrackerManager.myAnimeList.logout()`; `DelayedMangaTrackingStore`/`DelayedAnimeTrackingStore` use the `tracking_queue` file that step 93 rewrites |
| area:12-network-stack | DNS-over-HTTPS provider ids (`PREF_DOH_*`, `doh_provider`) |
| area:45-player-core, area:46-player-controls-ui, area:48-settings-player-and-scripts | player preference keys and enums that steps 92, 93, 123, 126, 127, 130 rewrite |
| area:43-reader-core, area:44-reader-viewers | reader orientation/reading-mode/navigation-mode keys that steps 59, 60, 77 rewrite |
| area:36-ui-shell-navigation-security-webview | `SecurityPreferences.SecureScreenMode`, nav styles, start screen |

## 10. Proposal: Kurobayomi runtime behaviour

Version bookkeeping (extends T-01a-013):
1. `VERSION_CODE: u32` constant in `kuro_core::app_info`, starting at `1`. It is a build/schema number, not the release version. Every release that adds a migration bumps it by exactly 1 and registers the new migration with `Version::At(VERSION_CODE)`.
2. Stored in `state.toml` as `last_version_code` (area:02-preferences-system key `__APP_STATE_last_version_code` mapped by its registry).
3. Strategy rules are identical to section 3 (Initial, Noop, Range). Keep `old == 0` as "fresh profile".
4. Never rename the `last_version_code` key (the hazard in quirks item 1); if a rename is unavoidable, the new code must read the old key as a fallback in the same release.

Execution:
1. `kuro_core::migrate::run_blocking` is called once from the GTK `startup` handler before any window is created (area:01-foundation-app-bootstrap). It owns a short-lived current-thread tokio runtime or uses the app runtime handle; it is the only permitted `block_on` on the GTK thread.
2. Each migration is an `async fn(&MigrationContext) -> anyhow::Result<bool>`; the runner awaits them one after another in sorted order. Sorting uses `(Version, registration index)` so ties are stable.
3. An `Err` aborts the run, writes nothing, and raises the crash dialog of area:01-foundation-app-bootstrap. Because there is no rollback, each Kurobayomi migration must be idempotent and must write its results atomically (single TOML flush or single SQLite transaction).
4. `Ok(true)`/`Ok(false)` semantics are identical to Kotlin. A missing optional service (`ctx.service::<T>()` returns `None`) yields `Ok(false)` with a debug log, mirroring `?: return false`.
5. Preferences are accessed through the raw store API (`get_raw`, `set_raw`, `remove`, `all`) so that a migration can rewrite keys whose typed definition no longer exists. The typed `Prefs` tree is constructed after migrations complete (T-02a-034).
6. Filesystem and path access goes only through `kuro_platform::Paths` (cross-platform rule 4); tests inject a temp-dir `Paths`.

Dry run: `MigrationContext { dry_run: true }` logs `(Dry-run) Running migration: ...` and does not call the body; the chain result is `true` and the completion callback fires, exactly like Kotlin. Only unit tests use it.

The 44 steps map as follows (details and per-step acceptance in the catalogue and in `p1-tasks.md`):

| Class of step | Count | Rust decision |
|---|---|---|
| always-run scheduler set-up (`SetupBackupCreate`, `SetupAnimeLibraryUpdate`, `SetupMangaLibraryUpdate`) | 3 | Port as `setup_*` functions called at startup through the migration list |
| `CategoryPreferencesCleanupMigration` (129) | 1 | Port as reusable `prune_dangling_category_ids`, called after restore and after category deletion (not at startup; a later schema change may register it as a versioned migration) |
| preference value rewrites that can still matter when an old backup is imported (versions 7, 44, 52, 57, 59, 60, 61, 64 sort, 70, 71, 72, 75, 77, 81, 82, 84, 85, 86, 92, 93, 99, 105, 106, 114, 120, 123, 126, 127, 130) | 29 | Port as pure functions in `kuro_prefs::legacy::normalise` (P2) |
| key-prefix moves (113 `NoAppState`, 116 `PrivatePreference`, 117 `PermaTrustExtensions`) | 3 | Drop as migrations; the importer ignores the old un-prefixed keys (P3) |
| Android filesystem, WorkManager history, tracker log-outs (15, 19, 26, 54, 64 `SetupBackgroundTasks`, 96, 97, 121) | 8 | Drop (P3) with stated replacement or loss |

(3 + 1 + 29 + 3 + 8 = 44.) The exact membership of each class is the "Rust disposition" column of the catalogue table in `p1-topic-migration-catalogue.md`.
