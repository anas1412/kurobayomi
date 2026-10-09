# Area 11: the three always-run migrations and their Linux replacement

The three steps registered with `version = Migration.ALWAYS` are the only part of the migration list that stays useful on a fresh profile: they re-register Android WorkManager periodic jobs on every launch (`SetupBackupCreateMigration`, `SetupAnimeLibraryUpdateMigration`, `SetupMangaLibraryUpdateMigration`). Their purpose is to make sure the schedule exists and reflects the current preferences after an app update, a data restore, or a WorkManager database loss. The jobs themselves (what an update or a backup does) belong to area:26-library-update-jobs and area:21-backup-format-and-create; this file defines only the schedule contract and the three set-up calls.

## 1. OBSERVED call sites of `setupTask` (so the Rust `reconfigure` is called from the same places)

| Caller | Purpose |
|---|---|
| `SetupBackupCreateMigration`, `SetupAnimeLibraryUpdateMigration`, `SetupMangaLibraryUpdateMigration` | every launch |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt:237-238` | the user picks a new update interval (`setupTask(context, it)` with the new value for both media types) |
| `SettingsLibraryScreen.kt:255-256` | the user changes the device-restriction multi-select (`setupTask(context)`) |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt:273` | the user picks a backup interval (`BackupCreateJob.setupTask(context, it)`) |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt:45-47` | after restoring preferences: `AnimeLibraryUpdateJob.setupTask`, `MangaLibraryUpdateJob.setupTask`, `BackupCreateJob.setupTask` |
| `MangaLibraryUpdateJob.kt:523`, `AnimeLibraryUpdateJob.kt:535` | inside `stop(context)`: when a running job is cancelled and carries the `-auto` tag, the periodic job is re-enqueued |
| steps 61, 71, 84, 96 | historic migrations (not ported) |

## 2. OBSERVED schedule parameters

| Job | Unique name (tag) | Interval | Flex | Constraints | Backoff | Policy |
|---|---|---|---|---|---|---|
| Backup | `BackupCreator` | `backup_interval` hours (default 12; 0 cancels) | 10 minutes | battery not low | EXPONENTIAL, 10 minutes | UPDATE; input `is_auto_backup = true` |
| Manga library update | `LibraryUpdate-auto` (also tag `LibraryUpdate`) | `pref_library_update_interval_key` hours (default 0; 0 cancels) | 10 minutes | see below | LINEAR, 10 minutes | UPDATE |
| Anime library update | `AnimeLibraryUpdate-auto` (also tag `AnimeLibraryUpdate`) | same preference as manga | 10 minutes | see below | LINEAR, 10 minutes | UPDATE |

Library-update constraints from `library_update_restriction` (string set, default `{"wifi"}`): wifi required if `"wifi"` in set; unmetered required if `"network_not_metered"` in set (then `NetworkType.UNMETERED`, else `CONNECTED`); charging required if `"ac"` in set; battery not low always required.

WorkManager semantics that the Rust scheduler must reproduce in spirit: a periodic job first becomes eligible after one full interval (no initial delay is set), it may run at any moment inside the last 10 minutes ("flex") of each interval, runs are skipped (not queued up) when constraints are unmet and execute as soon as they hold, and `UPDATE` replaces the stored job definition without cancelling a run in progress. `Result.retry()` from the job (used by the library jobs when a manual run is active, `MangaLibraryUpdateJob.kt:95-110`) triggers the backoff delay (10 minutes linear: 10, 20, 30, ...; backup exponential: 10, 20, 40, ...).

## 3. Proposal: schedule contract (crate `kuro-core::sched`)

```rust
pub struct PeriodicSpec {
    pub name: &'static str,                 // "BackupCreator", "LibraryUpdate-auto", "AnimeLibraryUpdate-auto"
    pub interval: Duration,                 // hours * 3600 s
    pub flex: Duration,                     // 10 minutes
    pub constraints: Constraints,
    pub backoff: Backoff,
}
pub struct Constraints {
    pub wifi_only: bool,                    // "wifi" in restrictions
    pub unmetered_only: bool,               // "network_not_metered" in restrictions
    pub requires_ac: bool,                  // "ac" in restrictions
    pub battery_not_low: bool,              // always true for all three
}
pub enum Backoff { Linear(Duration), Exponential(Duration) }

pub trait JobScheduler: Send + Sync {
    /// Idempotent. Replaces the stored definition with the same name; keeps `last_run` and a running job.
    fn enqueue_unique_periodic(&self, spec: PeriodicSpec);
    /// No-op if absent.
    fn cancel_unique(&self, name: &str);
}
```

The three set-up functions are plain functions over typed preferences and a `&dyn JobScheduler`, registered in the migration list as always-run steps (`Version::Always`) and also called from the settings pages and after a preferences restore (the call sites in section 1):

```rust
pub fn setup_backup(prefs: &BackupPrefs, s: &dyn JobScheduler) {
    let hours = prefs.backup_interval.get();                       // default 12
    if hours > 0 { s.enqueue_unique_periodic(PeriodicSpec { name: "BackupCreator",
        interval: hours_to_dur(hours), flex: MIN10,
        constraints: Constraints { battery_not_low: true, ..Constraints::NONE },
        backoff: Backoff::Exponential(MIN10) }) } else { s.cancel_unique("BackupCreator") }
}
pub fn setup_library_update(media: Media, prefs: &LibraryPrefs, s: &dyn JobScheduler, override_hours: Option<u32>);
```

`setup_library_update` mirrors the Kotlin: `hours = override_hours.unwrap_or(prefs.auto_update_interval.get())`; when `hours > 0` build the constraints from `prefs.auto_update_device_restrictions` and enqueue `"LibraryUpdate-auto"` (manga) or `"AnimeLibraryUpdate-auto"` (anime); else `cancel_unique` of that name. Return value of each migration step: `Ok(true)` (as Kotlin, `true` after the call), `Ok(false)` if the scheduler or the typed preference group is not available in the context (tests).

### 3.1 In-process backend (default)

* State per job in `state.toml`: `last_run_ms` (the Kotlin equivalents are `library_update_last_timestamp` and `last_auto_backup_timestamp`, both app-state keys).
* Next eligible time = `last_run_ms + interval` (or `now + interval` when no run has ever happened, matching WorkManager's first run one interval after enqueue). The run time is chosen uniformly in `[next - flex, next]` when scheduled in the future; if the moment has already passed when the app starts (the app was closed for days), run once shortly after startup (after a delay of 60 seconds so the first window is usable) and then continue with `now + interval`. Missed runs are not repeated (WorkManager does the same).
* When constraints are unmet at the chosen moment, wait for a change event from the platform state and re-check; also re-check every 15 minutes as a safety net.
* Failure or `retry`: delay `10 minutes * attempt` (Linear) or `10 minutes * 2^(attempt-1)` (Exponential), maximum 5 hours, as in WorkManager (`MAX_BACKOFF_MILLIS = 5 hours`), attempts reset after a success.
* Single-run guard: a scheduled library update does not start while a manual update is running (it retries like the Kotlin `Result.retry()`), a backup does not start while a manual backup runs.
* The scheduler runs on the tokio runtime; jobs are spawned as tasks, and UI progress goes through the area:35-data-misc-updater-notification-export notification layer. No GTK calls happen in the scheduler.

### 3.2 Platform state for constraints (trait in `kuro-platform`, rule 2 of `plans/addenda/requirements-cross-platform.md`)

```rust
pub trait PowerNetworkState: Send + Sync {
    fn network(&self) -> NetworkInfo;            // { connected: bool, kind: Wifi | Ethernet | Cellular | Unknown, metered: bool }
    fn on_ac(&self) -> bool;                     // true on desktops without a battery
    fn battery_low(&self) -> bool;               // false on desktops without a battery
    fn subscribe(&self) -> tokio::sync::watch::Receiver<u64>;   // bumps when any of the above changes
}
```

Linux implementation: `network()` via NetworkManager D-Bus (`org.freedesktop.NetworkManager` properties `PrimaryConnection`, `Metered`, and the primary connection type `802-11-wireless` -> Wifi, `802-3-ethernet` -> Ethernet, `gsm`/`cdma` -> Cellular) using `zbus`; when NetworkManager is absent fall back to `gio::NetworkMonitor` (`network_available()` and `is_network_metered()`, kind Unknown). `on_ac()` via UPower (`org.freedesktop.UPower` property `OnBattery`, negated); `battery_low()` via the UPower display device `WarningLevel >= 3` (3 low, 4 critical, 5 action; values 0 unknown and 1 none are "not low"). Mapping of constraints to checks: `wifi_only` requires `kind == Wifi`, or `Ethernet` when the machine has no Wi-Fi (Proposal: on a desktop wired connection satisfies "only on Wi-Fi" because the user's intent is "not on mobile data"; document in the settings help text of area:47-settings-screens-general); `unmetered_only` requires `!metered`; `requires_ac` requires `on_ac()`; `battery_not_low` requires `!battery_low()`. Windows implementation later: `GetSystemPowerStatus` and the Network List Manager.

### 3.3 When Kurobayomi is not running

WorkManager runs jobs while the app process is dead; a desktop app does not. Loss: no library update or backup happens while Kurobayomi is closed (they run at the next start, see the catch-up rule). Optional replacement (not part of this area, area:26-library-update-jobs decides): a systemd user timer that launches `kurobayomi --headless-job library-update` through the `SchedulerBackend` trait of the cross-platform addendum.

## 4. Tasks

T-11a-004 (backup), T-11a-005 (manga), T-11a-006 (anime) implement section 3's three functions; T-11a-007 implements 3.2; area:26-library-update-jobs owns 3.1's backend because it also runs the job body.
