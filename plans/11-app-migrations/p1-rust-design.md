# Area 11 app-migrations, part 1: Rust design

Everything here is Proposal. It refines `plans/01-foundation-app-bootstrap/p1-topic-migrator.md` section 5 (area:01-foundation-app-bootstrap owns `kuro_core::migrate` and tasks T-01a-012, T-01a-013) and plugs the area-11 content into it. It follows the cross-platform rules of `plans/addenda/requirements-cross-platform.md` (no `cfg(target_os)` outside `kuro-platform`; all paths from `Paths`).

## 1. Crates and modules

| Crate / module | Content (area-11 part) | Owner of the crate |
|---|---|---|
| `kuro-core::migrate` | `Version`, `Migration`, `MigrationContext`, `Strategy`, `plan`, `run`, `run_blocking` (framework, area:01-foundation-app-bootstrap) | area:01-foundation-app-bootstrap |
| `kuro-core::migrate::list` | `static MIGRATIONS: &[Migration]` with the three always-run steps and the disposition comment block for the 41 legacy steps | area:11-app-migrations (this area) |
| `kuro-core::migrate::steps` | `setup_backup`, `setup_library_update(Media)` bodies, thin wrappers over `kuro_core::sched` | area:11-app-migrations |
| `kuro-core::sched` | `PeriodicSpec`, `Constraints`, `Backoff`, `JobScheduler`, `InProcessScheduler` | area:26-library-update-jobs (backend), contract here |
| `kuro-platform::power_network` | `PowerNetworkState` trait + Linux implementation (NetworkManager, UPower via `zbus`; `gio::NetworkMonitor` fallback) | area:11-app-migrations contract, `kuro-platform` implementation |
| `kuro-prefs::legacy` | `normalise(&mut RawPrefs) -> NormaliseReport` with groups `a_library`, `b_update`, `c_reader`, `d_player`, `e_lang`, `f_ui`, `g_repos`, `h_ignored` | area:02-preferences-system crate, area:11-app-migrations content |
| `kuro-prefs::category_prune` | `prune_dangling_category_ids(prefs, manga_ids, anime_ids)` | area:11-app-migrations content |

New dependencies and why: `isolang` (ISO 639 tables for E1; tiny, MIT), `zbus` (already required by notifications and MPRIS elsewhere; used here only inside `kuro-platform`), `anyhow` and `thiserror` (error handling; already workspace dependencies), `futures` (`BoxFuture`), `tracing` (log text parity), `rand` (flex jitter in the scheduler, small). No new GTK dependency: nothing in this area touches a widget except the post-upgrade dialog (T-11a-009), which lives in area:36-ui-shell-navigation-security-webview.

## 2. Framework types (refinement of area:01-foundation-app-bootstrap)

```rust
// kuro-core/src/migrate/mod.rs
use std::{future::Future, pin::Pin, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Version { Always, At(u32) }              // derive order: Always < At(0) < At(1) ...

impl Version {
    pub fn is_always(self) -> bool { matches!(self, Version::Always) }
    /// Kotlin printed Float.toString(): "-1.0", "64.0".
    pub fn log_text(self) -> String { match self { Version::Always => "-1.0".into(), Version::At(n) => format!("{n}.0") } }
}

pub type MigrationFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send + 'a>>;

pub struct Migration {
    pub name: &'static str,                       // Kotlin class simple name, used in the log line
    pub version: Version,
    pub run: for<'a> fn(&'a MigrationContext) -> MigrationFuture<'a>,
}
impl Migration {
    /// Test helper mirroring `Migration.of(version) { ... }`.
    pub const fn of(name: &'static str, version: Version,
                    run: for<'a> fn(&'a MigrationContext) -> MigrationFuture<'a>) -> Migration { Migration { name, version, run } }
}

pub struct MigrationContext {
    pub dry_run: bool,
    pub app: Arc<AppContext>,                     // area:01-foundation-app-bootstrap; all services are plain fields
}
impl MigrationContext {
    /// Raw, untyped access, usable before the typed Prefs tree exists (T-02a-034).
    pub fn raw_prefs(&self) -> &dyn RawPrefStore;
    pub fn paths(&self) -> &dyn kuro_platform::Paths;
    pub fn scheduler(&self) -> Option<&dyn kuro_core::sched::JobScheduler>;   // None in tests that do not provide one
}

pub trait RawPrefStore {
    fn get(&self, key: &str) -> Option<kuro_prefs::legacy::RawValue>;
    fn set(&self, key: &str, v: kuro_prefs::legacy::RawValue);
    fn remove(&self, key: &str);
    fn all(&self) -> std::collections::BTreeMap<String, kuro_prefs::legacy::RawValue>;
}
```

Selection and execution functions (`select`, `plan`, `run`, `run_blocking`) are exactly those of area:01-foundation-app-bootstrap section 5; the area-11 requirements on them are:
* `plan(strategy, all)` returns migrations `isAlways || (At(n) with n in from..=to)` stable-sorted by `Version` (ties by slice index).
* `run` logs `Running migration: { name = {name}, version = {version.log_text()} }` before each call (prefix `(Dry-run) ` and no call when `ctx.dry_run`), accumulates `any |= result`, calls `on_complete` only if `any` and `!ctx.dry_run`, and returns `Err` unchanged on the first failure (no further steps run).
* A migration that needs an absent optional service returns `Ok(false)` after a `debug!` line (parity with `?: return false`).

Registered list:

```rust
// kuro-core/src/migrate/list.rs
pub static MIGRATIONS: &[Migration] = &[
    Migration::of("SetupBackupCreateMigration",       Version::Always, steps::backup_create),
    Migration::of("SetupAnimeLibraryUpdateMigration", Version::Always, steps::anime_library_update),
    Migration::of("SetupMangaLibraryUpdateMigration", Version::Always, steps::manga_library_update),
];
// Dropped on purpose, one comment line per legacy step (class, version, reason); the full table lives in
// plans/11-app-migrations/p1-topic-migration-catalogue.md section 1.
```

The order matches `Migrations.kt` (backup, anime, manga), though all three have the same version so only the stable order matters.

Authoring rules for future migrations (T-11a-002), enforced by review checklist in `kuro-core/src/migrate/README` comment and by the idempotency harness (T-11a-003):
1. Version is `VERSION_CODE` of the release that introduces the step; bump `VERSION_CODE` in the same change.
2. Never edit a released step; add a new one.
3. Conversions read raw keys, convert only when the legacy key is present, never write defaults (quirks items 4, 9, 14).
4. Write results atomically: one `RawPrefStore` flush or one SQLite transaction. Do not mix.
5. Idempotent: running twice equals running once (the harness runs each migration twice on the same fixture).
6. No GTK calls, no blocking network access, no sleeps.
7. Return `Ok(true)` only if something changed or a schedule was (re)registered.

## 3. Threading model

* GTK main thread: `connect_startup` in `main.rs` calls `migrate::run_blocking(&app_ctx, VERSION_CODE, MIGRATIONS)` before any window is built; it is the only `Handle::block_on` on the GTK thread (area:01-foundation-app-bootstrap). With three scheduler-registering steps the call completes in well under 10 ms; if a future step exceeds 300 ms the splash of T-01a-043 appears.
* The three steps only call `JobScheduler::enqueue_unique_periodic`/`cancel_unique`, which are non-blocking (they store the spec and wake the scheduler task through a `tokio::sync::mpsc` sender). No `.await` that waits for a job run happens inside a migration.
* `InProcessScheduler` runs on the app's multi-thread tokio runtime (area:01-foundation-app-bootstrap runtime). Job bodies are `tokio::spawn`ed; they report UI state through the app event bus (area:01-foundation-app-bootstrap) and never touch GTK directly.
* `PowerNetworkState::subscribe` returns a `tokio::sync::watch::Receiver<u64>`; `kuro-platform`'s Linux implementation owns a `zbus` connection on the same runtime.

## 4. Error handling

* `anyhow::Result<bool>` for migrations, wrapped by the runner into `MigrationError { name, version, source }` (`thiserror`).
* On `Err`: nothing stored, `error!("Migration failed: {name} (version {version})")`, then the area:01-foundation-app-bootstrap crash dialog with the text "Migration <name> failed: <message>. Start Kurobayomi once with --skip-migrations to continue without it." `--skip-migrations` (area:01-foundation-app-bootstrap CLI parsing) stores `last_version_code = VERSION_CODE` without running anything and logs a warning.
* `normalise` never fails: every problem becomes a `Skipped` entry in the report that the import dialog of area:22-backup-restore shows ("N settings converted, M skipped") and the log records.

## 5. How this plugs into the existing mockup

The mockup (`src/main.rs`, `src/ui.rs`, `src/screens.rs`, `src/settings.rs`, `src/spec.rs`, `spec/*.json`) has no startup logic and no data layer, so area-11 hooks are small:

| Mockup element | Hook |
|---|---|
| `src/main.rs` `main()` creates `gtk::Application` and calls `app.connect_activate(build_ui)` | add `app.connect_startup(|_| migrate::run_blocking(...))` (area:01-foundation-app-bootstrap task T-01a-013); no change to `build_ui` |
| `spec/settings.json` list "Automatic updates" (line 294, options Off, Every 12 hours, Daily, Every 2 days, Every 3 days, Weekly) | stored values 0, 12, 24, 48, 72, 168 hours; the `on_change` callback calls `setup_library_update(Manga, prefs, sched, Some(new))` and the anime variant, after the new value is stored (Kotlin passes the new value because its callback runs before the preference store updates, `SettingsLibraryScreen.kt:236-238`; Rust stores first, then calls with `None`) |
| `spec/settings.json` multi "Automatic updates device restrictions" (line 307, options Only on Wi-Fi, Only on unmetered network, When charging) | stored members `wifi`, `network_not_metered`, `ac`; callback calls both `setup_library_update(..., None)` after storing (Kotlin defers with `getMainExecutor().execute`, `SettingsLibraryScreen.kt:251-257`, for the same reason) |
| `spec/settings.json` list "Automatic backup frequency" (line 2416, options Off, Every 6 hours, Every 12 hours, Daily, Every 2 days, Weekly; default Every 12 hours) | stored values 0, 6, 12, 24, 48, 168 hours; callback calls `setup_backup` |
| `src/settings.rs` `"settings_worker_info" => text_dump(id, "Enqueued\n-\n\nFinished\nId: 1a2b\nTags: LibraryUpdate\nState: SUCCEEDED\n\nRunning\n-")` | replace the static text by `JobScheduler::snapshot()` formatted with the same three headings (Enqueued, Finished, Running) listing job name, tags, state, next run time; "Copy to clipboard" copies that text. The three registered periodic jobs are what appears under "Enqueued" |
| restore flow (area:22-backup-restore) | after preferences are restored call `setup_backup`, `setup_library_update(Manga, ..)`, `setup_library_update(Anime, ..)` (Kotlin `PreferenceRestorer.kt:45-47`) |
| category deletion (area:08-domain-library-category-history-updates, `spec` category dialogs) | call `prune_dangling_category_ids` for the affected media type |
| backup import dialog (area:22-backup-restore UI) | show the `NormaliseReport` counts |

## 6. Android API to Linux mapping (APIs touched by this scope)

| Android / Kotlin | Linux / Rust replacement |
|---|---|
| `Injekt.getInstanceOrNull<T>()` (`MigrationContext.get`) | fields on `AppContext` (area:01-foundation-app-bootstrap), `Option` only for services that tests may omit |
| `kotlinx.coroutines` (`Deferred`, `async`, `CompletableDeferred`, `runBlocking`) | `async fn` chain awaited in order by `run`; one `block_on` in `run_blocking` |
| `Dispatchers.IO` / `withIOContext` | tokio runtime (`spawn_blocking` for file or SQLite work inside a step) |
| `SharedPreferences` + `PreferenceManager.getDefaultSharedPreferences` | `RawPrefStore` over `kuro-prefs` TOML backend (`config.toml`, `state.toml`) |
| `androidx.core.content.edit` staged edit, `commit = true` | one atomic TOML flush per migration |
| `WorkManager` (`PeriodicWorkRequestBuilder`, `enqueueUniquePeriodicWork`, `cancelUniqueWork`, `cancelAllWorkByTag`, constraints, backoff) | `JobScheduler` contract + `InProcessScheduler`, constraints via `PowerNetworkState` |
| `Context.cacheDir`, `externalCacheDir`, `getExternalFilesDir` | `kuro_platform::Paths::cache()`, `data()`; not used by any ported step |
| `Application` | `AppContext` |
| `logcat { }` | `tracing::debug!` / `error!` |
| `DeviceUtil.isMiui` | dropped |
| `TrackerManager.myAnimeList.logout()` | dropped (area:27-tracking-core, area:29-tracker-myanimelist own logout) |
| `kotlinx.serialization.json.Json` (`SubConfig`) | `serde_json` in the normaliser (`#[serde(default)]` on the three lists, unknown fields ignored) |
| `java.util.Locale` language validation (`PrefLangMigration`) | `isolang` tables |
| `ActivityInfo.SCREEN_ORIENTATION_*` | integer constants inside the normaliser only |

## 7. Normaliser structure

```rust
// kuro-prefs/src/legacy/mod.rs
pub fn normalise(raw: &mut RawPrefs) -> NormaliseReport {
    let mut rep = NormaliseReport::default();
    a_library::run(raw, &mut rep);   // sort A1, filters A2/A3, flags A4
    b_update::run(raw, &mut rep);    // B1..B3
    c_reader::run(raw, &mut rep);    // C1..C3
    d_player::run(raw, &mut rep);    // D1..D5
    e_lang::run(raw, &mut rep);      // E1
    f_ui::run(raw, &mut rep);        // F1..F6
    g_repos::run(raw, &mut rep);     // G (moves the two sets into rep.repo_sets)
    h_ignored::run(raw, &mut rep);   // H
    rep
}
```

Each rule is a small private function `fn rule_xx(raw: &mut RawPrefs, rep: &mut NormaliseReport)` with the shape: read legacy key with the legacy type; `if dest.is_some() { rep.skip(DestinationPresent); return }`; compute; `raw.insert(dest, value)`; `raw.remove(legacy)`; `rep.converted(...)`. A helper macro `take!(raw, "key", RawValue::Int(v))` returns `Some(v)` and reports `WrongType` for a key of another kind.

## 8. Category pruning

```rust
// kuro-prefs/src/category_prune.rs
pub struct CategoryPrefs<'a> {
    pub default_manga: &'a Pref<i32>,  pub default_anime: &'a Pref<i32>,       // default_category, default_anime_category; default -1
    pub sets: Vec<&'a Pref<BTreeSet<String>>>,                                 // the 10 sets of the catalogue entry for version 129
}
/// Removes ids that exist in neither id set. Returns how many ids were removed.
pub fn prune_dangling_category_ids(p: &CategoryPrefs, manga_ids: &BTreeSet<String>, anime_ids: &BTreeSet<String>) -> usize;
```
Rules: default categories whose id string is in neither id set are deleted (set back to default `-1`); each set loses `ids - manga_ids - anime_ids`; unchanged sets are not written; ids are decimal strings of `i64`; id `0` (system category) is valid because the category tables contain a row `_id = 0`. Callers: after restore (area:22-backup-restore) and at the end of `DeleteCategory` for both media types (fixing quirks item 12). It is not run at startup; if a later schema change leaves dangling ids it can be registered as a versioned migration.

## 9. Pitfalls

* Do not store `VERSION_CODE` before the migration list finished; a crash between would hide the failed run.
* Do not run migrations before `Paths` exist and the TOML backend is loaded; the order in area:01-foundation-app-bootstrap is paths, logging, prefs, runtime, context, then migrations.
* `last_version_code` must be an app-state key (excluded from backups); restoring a backup must never lower it.
* `normalise` must run on the backup's own map, not on the live store, so a failed import leaves the live preferences untouched.
* The scheduler's "run once shortly after startup when overdue" must not fire while a backup restore is in progress (area:22-backup-restore holds a global `restore_in_progress` flag).
* Time and locale: `"all"` language set and default `source_languages` use the system language through `sys-locale`; not part of the migrations but part of F6.
* GTK: none of the area-11 code may be called from a GTK signal handler synchronously except `run_blocking` at startup; settings callbacks call the scheduler functions, which are non-blocking.
