# Topic: library and backup scheduling constraints (settings side)

Covers T-47a-023, T-47a-024, T-47a-090, and the decision in `p1-rust-design.md` section 6. Sources: `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt` (`setupTask` lines 439-486, `startNow` 488-509, `stop` 511-526), `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt` (`setupTask` lines 452-498, `startNow` 500-521), `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt` (`setupTask` lines 90-114, `startNow` 116-127), `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt` (lines 226-258), `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt` (lines 261-276).

## 1. What the settings screens write

| Setting | Key | Effect on the scheduler |
|---|---|---|
| Automatic updates interval | `pref_library_update_interval_key` (default 0) | `setupTask(context, value)` for manga AND anime |
| Device restrictions | `library_update_restriction` (default {wifi}) | constraints of the periodic work (both jobs), rescheduled after the value is stored |
| Backup frequency | `backup_interval` (default 12) | `BackupCreateJob.setupTask(context, value)` |

Interval 0 → cancel the unique periodic work (`cancelUniqueWork(WORK_NAME_AUTO)` / backup `TAG_AUTO`).

## 2. Periodic work definition (observed)

Library (manga, `MangaLibraryUpdateJob.kt:447-476`; anime has the same structure at `AnimeLibraryUpdateJob.kt:452-498`):
```
interval   = pref value (hours)
restrictions = pref set
networkType = restrictions ∋ network_not_metered ? UNMETERED : CONNECTED
networkRequest = NetworkRequest.Builder()
    + TRANSPORT_WIFI            if restrictions ∋ wifi
    + NET_CAPABILITY_NOT_METERED if restrictions ∋ network_not_metered
constraints = requiredNetworkRequest(networkRequest, networkType)
            + requiresCharging(restrictions ∋ ac)
            + requiresBatteryNotLow(true)
periodic   = every `interval` hours, flex 10 minutes
backoff    = LINEAR, 10 minutes
policy     = enqueueUniquePeriodicWork(WORK_NAME_AUTO, UPDATE, request)
```

Backup (`BackupCreateJob.kt:94-110`):
```
periodic   = every `interval` hours, flex 10 minutes
backoff    = EXPONENTIAL, 10 minutes
constraints = requiresBatteryNotLow(true)           (no network constraint)
input      = is_auto_backup = true
policy     = enqueueUniquePeriodicWork(TAG_AUTO, UPDATE, request)
```

Manual run: library `startNow` uses a one-time request with `ExistingWorkPolicy.KEEP` and returns false when `isRunning(TAG)` (lines 492-509). Backup manual run uses `KEEP` too (`BackupCreateJob.kt:126`).

## 3. Mapping to the Linux scheduler (Proposal, option A from the design doc)

Scheduler state (SQLite table `job_schedule`, area 26): `name TEXT PRIMARY KEY, period_secs INTEGER, next_run_at INTEGER, last_result TEXT, attempts INTEGER`.

```
on set interval N (hours):
    if N == 0: DELETE job_schedule WHERE name IN ('library-manga','library-anime')
    else:      upsert (name, period = N*3600, next_run_at = now + period)

scheduler tick (every 60 s):
    for job in due jobs (next_run_at <= now):
        if !constraints_ok(job): next_run_at = now + 5 min (defer, no attempt increment); continue
        run job
        on success: attempts = 0; next_run_at = now + period (+ jitter ≤ 10 min)
        on failure: attempts += 1; backoff = 10 min × attempts (LINEAR) [library]
                    or 10 min × 2^(attempts-1) (EXPONENTIAL) [backup]
                    next_run_at = now + backoff
```

Constraint checks:

| Constraint | Check | If unknown |
|---|---|---|
| Unmetered | `gio::NetworkMonitor::default().is_network_metered() == false` | allow |
| Wi-Fi only | NetworkManager active connection type `802-11-wireless` (zbus `org.freedesktop.NetworkManager`) | allow + warn |
| Charging | UPower `OnBattery == false` (zbus `org.freedesktop.UPower`) | allow + warn |
| Battery not low | UPower `Percentage >= 15` (Proposal) | allow + warn |

Open decision: whether the scheduler runs only while the app is open (option A) or as a `systemd --user` timer that starts the app headless (option B). Option B changes nothing in the settings screens; only the runtime differs. The user must choose.

## 4. Restriction changes race (Android behaviour and Rust rule)

Android posts the reschedule to the main executor so that the preference set is committed first (`SettingsLibraryScreen.kt:254-257`). Rust does the same by calling `reschedule()` after `Pref::set()` returns (the store updates the cache synchronously, so no executor hop is needed).

## 5. Toggle semantics

- Interval change to the same value: Android re-enqueues with `ExistingPeriodicWorkPolicy.UPDATE` (`MangaLibraryUpdateJob.kt:478-482`). Rust (Proposal): skip the reschedule when the value is unchanged.
- Restriction row is hidden when interval is 0 (behaviour §3.2). Restriction changes made while hidden are not possible.
- Backup interval 0 cancels only the auto backup; running manual backups continue.

## 6. Test vectors

| Input | Expected schedule |
|---|---|
| interval 12, restrictions {wifi} | period 12 h; constraint wifi; no charging; battery ok |
| interval 24, restrictions {network_not_metered, ac} | period 24 h; unmetered; charging required |
| interval 0 | no row |
| library job fails 3 times | next_run = now + 10, 20, 30 min |
| backup job fails 3 times | next_run = now + 10, 20, 40 min |
| restriction change while job running | running job continues; next run uses new constraints |
