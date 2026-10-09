# Topic: startup migrator (Migrator, strategies, job chain)

Sources (all repo-relative): `app/src/main/java/mihon/core/migration/Migrator.kt`, `MigrationStrategy.kt`, `MigrationStrategyFactory.kt`, `MigrationJobFactory.kt`, `MigrationContext.kt`, `MigrationCompletedListener.kt`, `Migration.kt`, `migrations/Migrations.kt`, call site `app/src/main/java/eu/kanade/tachiyomi/App.kt:164-180`, blocking consumer `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:166`, test `app/src/test/java/mihon/core/migration/MigratorTest.kt`. The individual migrations belong to area:11-app-migrations; this file covers the framework that area:01 owns because `App.onCreate` drives it.

## 1. Observed contract

```kotlin
interface Migration {
    val version: Float
    suspend operator fun invoke(migrationContext: MigrationContext): Boolean
    val isAlways: Boolean get() = version == ALWAYS          // ALWAYS = -1f
    companion object { fun of(version: Float, action: suspend (MigrationContext) -> Boolean): Migration }
}
class MigrationContext(val dryrun: Boolean) { inline fun <reified T> get(): T? = Injekt.getInstanceOrNull(T::class.java) }
typealias MigrationCompletedListener = () -> Unit
object Migrator {
    private var result: Deferred<Boolean>? = null
    val scope = CoroutineScope(Dispatchers.IO + Job())
    fun initialize(old: Int, new: Int, migrations: List<Migration>, dryrun: Boolean = false, onMigrationComplete: () -> Unit)
    suspend fun await(): Boolean            // result ?: CompletableDeferred(false) .await()
    fun release()                           // result = null
    fun awaitAndRelease(): Boolean          // runBlocking { await().also { release() } }
}
```
Return value of a migration: `true` = "I changed something" (the chain result becomes true), `false` = "nothing to do or could not run". A migration returning `false` does not abort the chain.

## 2. Observed algorithm

1. `initialize` creates `MigrationContext(dryrun)`, `MigrationJobFactory(context, scope)`, `MigrationStrategyFactory(jobFactory, onMigrationComplete)` and `strategy = factory.create(old, new)`; `result = strategy(migrations)`.
2. `create(old, new)`:
   * `old == 0` → `InitialMigrationStrategy(DefaultMigrationStrategy(...))`; invoked with the list filtered to `it.isAlways`.
   * `old >= new` → `NoopMigrationStrategy(false)` → completed deferred `false`; no listener call.
   * otherwise → `VersionRangeMigrationStrategy((old + 1)..new, DefaultMigrationStrategy(...))`; list filtered to `it.isAlways || it.version.toInt() in versions` (inclusive range, `Float.toInt()` truncates toward zero).
3. `DefaultMigrationStrategy(list)`: if `list.isEmpty()` return `CompletableDeferred(false)`. Else `chain = jobFactory.create(list)`; `launch { if (chain.await()) migrationCompletedListener() }.start()` on `Migrator.scope`; return `chain`.
4. `MigrationJobFactory.create(list)`: `list.sortedBy { it.version }` (stable sort, `ALWAYS = -1f` first, then ascending), fold from `CompletableDeferred(true)`:
   * real run: log `"Running migration: { name = ${migration::class.simpleName}, version = ${migration.version} }"`, element = `async(start = CoroutineStart.UNDISPATCHED) { val prev = acc.await(); migration(ctx) || prev }`.
   * dry run: log `"(Dry-run) Running migration: { name = ..., version = ... }"`, element = `CompletableDeferred(true)` (migration body is never called).
   The final deferred is the chain result.
5. Consequences:
   * Strictly sequential: element N awaits element N-1 before it starts its migration.
   * `UNDISPATCHED` runs the first element's body on the calling thread (the Android main thread in `App.onCreate`) until its first real suspension; later elements resume on `Dispatchers.IO` threads of `Migrator.scope`.
   * Result is true if any migration returned true. The listener (which stores `last_version_code`) runs only when the result is true; with a non-empty list where every migration returned false the version code is NOT stored, so the same migrations run again on the next launch.
   * Failure path (kotlinx.coroutines semantics, not written in the repository): a migration that throws fails its `async`; later elements fail at `acc.await()`; the child failure cancels the scope's plain `Job()`; the `launch` in step 3 receives the exception from `chain.await()` and, because the scope has no `CoroutineExceptionHandler`, reports it to the thread's uncaught-exception handler, which is `GlobalExceptionHandler` and shows the crash screen. `last_version_code` is not stored, so the migration retries next launch. `MainActivity`'s `awaitAndRelease()` also rethrows the same exception on the main thread, so the crash happens on whichever sees it first.
6. `MainActivity.onCreate` (`MainActivity.kt:166`): `val didMigration = Migrator.awaitAndRelease()` before composing UI; `awaitAndRelease` blocks the main thread (`runBlocking`) until the chain finishes, then clears `result` so a second call (activity recreation) gets `false`.
7. The `onMigrationComplete` lambda in `App.initializeMigrator`: `logcat { "Updating last version to ${BuildConfig.VERSION_CODE}" }; preference.set(BuildConfig.VERSION_CODE)`.

## 3. Upstream test (port as is, section 7 of `p1-edge-cases-and-tests.md`)
`MigratorTest` has six tests: `initialVersion` (0→1: strategy is Initial; only the ALWAYS migration is passed to `create`; listener called), `sameVersion` (1→1: Noop; result false; `create` never called), `noMigrations` (1→2 with an empty list: VersionRange; result false; `create` never called), `smallMigration` (1→2 with ALWAYS and 2f: both migrations passed; listener called), `largeMigration` (1→10 with ALWAYS and 2f..10f: ten migrations passed; listener called), `withinRangeMigration` (1→2 with ALWAYS, 2f, 3f: only ALWAYS and 2f passed; 3f excluded; listener called).

## 4. Registered migrations (registration order of `Migrations.kt`, with their `version`)

Sorted order at run time is by version; this table keeps source order. "Kind" is how the Rust app treats it.

| # | Class | Version | Kind in Kurobayomi |
|---|---|---|---|
| 1 | `SetupBackupCreateMigration` | ALWAYS | Port: call `backup_scheduler.setup()` (area:21-backup-format-and-create). Body: `BackupCreateJob.setupTask(context)` which, when `backup_interval` (hours) > 0, enqueues unique periodic work `TAG_AUTO` (interval hours, flex 10 minutes, requires battery not low, exponential backoff starting at 10 minutes, input `IS_AUTO_BACKUP_KEY = true`, policy UPDATE), else cancels it |
| 2 | `SetupAnimeLibraryUpdateMigration` | ALWAYS | Port: `anime_update_scheduler.setup()` (area:26-library-update-jobs); body `AnimeLibraryUpdateJob.setupTask(context)` |
| 3 | `SetupMangaLibraryUpdateMigration` | ALWAYS | Port: `manga_update_scheduler.setup()`; body `MangaLibraryUpdateJob.setupTask(context)` |
| 4 | `InternalChapterCacheDirMigration` | 15 | Drop: Android cache dir layout |
| 5 | `CoverToExternalFileMigration` | 19 | Drop |
| 6 | `DeleteExternalChapterCacheDirMigration` | 26 | Drop |
| 7 | `ResetSortPreferenceRemovedMigration` | 44 | Drop |
| 8 | `MigrateToTriStateMigration` | 52 | Drop |
| 9 | `ForceMALLogOutMigration` | 54 | Drop |
| 10 | `DOHMigration` | 57 | Drop |
| 11 | `ResetRotationMigration` | 59 | Drop |
| 12 | `MigrateRotationViewerValuesMigration` | 60 | Drop |
| 13 | `RemoveOneTwoHourUpdateMigration` | 61 | Drop |
| 14 | `SetupBackgroundTasksMigration` | 64 | Drop |
| 15 | `MigrateSortingModeMigration` | 64 | Drop |
| 16 | `AddAllLangMigration` | 70 | Drop |
| 17 | `RemoveQuickUpdateMigration` | 71 | Drop |
| 18 | `CombineUpdateRestrictionMigration` | 72 | Drop |
| 19 | `MigrateSecureScreenMigration` | 75 | Drop |
| 20 | `RemoveReaderTapMigration` | 77 | Drop |
| 21 | `RenameEnumMigration` | 81 | Drop |
| 22 | `MergeSortTypeDirectionMigration` | 82 | Drop |
| 23 | `EnableAutoBackupMigration` | 84 | Drop |
| 24 | `MoveChapterPreferencesMigration` | 85 | Drop |
| 25 | `SplitPreferencesMigration` | 86 | Drop |
| 26 | `PlayerPreferenceMigration` | 92 | Drop |
| 27 | `MovePlayerPreferencesMigration` | 93 | Drop |
| 28 | `UseWorkManagerMigration` | 96 | Drop |
| 29 | `RemoveBackgroundJobsMigration` | 97 | Drop |
| 30 | `MigrateTriStateMigration` | 99 | Drop |
| 31 | `DontRunJobsMigration` | 105 | Drop |
| 32 | `RelativeTimestampMigration` | 106 | Drop |
| 33 | `NoAppStateMigration` | 113 | Drop |
| 34 | `ExternalRepoMigration` | 114 | Drop |
| 35 | `PrivatePreferenceMigration` | 116 | Drop |
| 36 | `PermaTrustExtensionsMigration` | 117 | Drop |
| 37 | `NavigationOptionsMigration` | 120 | Drop |
| 38 | `LogOutMALMigration` | 121 | Drop |
| 39 | `EnumsMigration` | 123 | Drop |
| 40 | `TrustExtensionRepositoryMigration` | 7 | Drop |
| 41 | `VideoPlayerPreferenceMigration` | 126 | Drop |
| 42 | `VideoOrientationMigration` | 127 | Drop |
| 43 | `CategoryPreferencesCleanupMigration` | 129 | Drop |
| 44 | `PrefLangMigration` | 130 | Drop |

The list in `Migrations.kt` has 44 entries. `SetupBackgroundTasksMigration` and `MigrateSortingModeMigration` both have version 64, so after the stable sort they keep their registration order.

Why "Drop": every non-ALWAYS migration rewrites keys or values inside an Android SharedPreferences file from an older Aniyomi/Tachiyomi/Mihon release, or moves Android cache directories. A fresh Linux install has none of that data. Importing an Aniyomi backup is a different path (area:22-backup-restore maps backed-up preference keys to settings fields at restore time). The user loses nothing.

## 5. Rust design

Module `kuro_core::migrate`:

```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Version { Always, At(u32) }          // Always sorts before every At(_), like -1f

pub struct MigrationContext { pub dry_run: bool, pub app: Arc<AppContext> }

pub type MigrationFuture<'a> = BoxFuture<'a, anyhow::Result<bool>>;
pub struct Migration {
    pub name: &'static str,
    pub version: Version,
    pub run: for<'a> fn(&'a MigrationContext) -> MigrationFuture<'a>,
}

pub enum Strategy { Initial, Noop, Range { from: u32, to: u32 } }   // Range is inclusive: (old+1)..=new

pub fn select(old: u32, new: u32) -> Strategy {
    if old == 0 { Strategy::Initial }
    else if old >= new { Strategy::Noop }
    else { Strategy::Range { from: old + 1, to: new } }
}
pub fn plan<'a>(s: &Strategy, all: &'a [Migration]) -> Vec<&'a Migration>;   // filter, then stable sort by version

pub async fn run(ctx: &MigrationContext, s: Strategy, all: &[Migration], on_complete: impl FnOnce()) -> anyhow::Result<bool>;
pub fn run_blocking(app: &Arc<AppContext>, new: u32, all: &[Migration]) -> anyhow::Result<bool>;
```
Behaviour:
* `run`: `plan`; empty plan returns `Ok(false)` without calling `on_complete`; non-empty: for each migration in order log `Running migration: { name = {name}, version = {version} }` (dry run: `(Dry-run) Running migration: ...` and skip the call, count as true); call `run`; `Err(e)` aborts the whole run and propagates (parity with the chain failing); accumulate `any_true |= result`; at the end if `any_true` call `on_complete()` and return `Ok(any_true)`.
* `run_blocking` reads `old` from `state.last_version_code`, builds the strategy, calls `rt.block_on(run(...))` with `on_complete` writing `state.last_version_code = new` and logging `Updating last version to {new}`; logs `Migration from {old} to {new}` first. On `Err` it panics with the error text (so the crash window shows it) — equivalent to Kotlin's uncaught exception path. Must be called on the main thread before any window exists; it is the only permitted `block_on` on the GTK thread.
* Registered list: `pub static MIGRATIONS: &[Migration] = &[SETUP_BACKUP_CREATE, SETUP_ANIME_LIBRARY_UPDATE, SETUP_MANGA_LIBRARY_UPDATE];` in module `kuro_core::migrate::list`; new migrations append and bump `VERSION_CODE` (`kuro_core::app_info`).
* Version code of Kurobayomi starts at `1`, so a first install (`old == 0`, strategy Initial) runs the three always-migrations and stores `1`. A second launch (`old == new == 1`, Noop) runs nothing, as in Aniyomi.
* The three setup migrations require the scheduler traits (`SchedulerBackend` of the cross-platform addendum) that area:26-library-update-jobs and area:21-backup-format-and-create define; until they exist the list is empty and the first install simply stores the version code (`Initial` with an empty plan returns false, so nothing is stored; T-01a-013 stores the version explicitly when the plan is empty, a deliberate deviation recorded here: Aniyomi cannot hit it because its list always contains the always-migrations).

## 6. Dry run
`dryrun = true` exists in the Kotlin signature but no caller passes it (only tests could). Port it for unit tests (`Migrator::run` with `dry_run = true` verifies selection and logging without side effects).
