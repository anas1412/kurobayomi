# Topic: automatic backup scheduling on Linux

Scope: replacing WorkManager periodic work for automatic backups (`BackupCreateJob.setupTask`, B-04, B-05) and deciding when a run happens, what defers it and how it retries. Shared scheduler design with area:26-library-update-jobs is noted in section 7.

## 1. What Aniyomi does (observed)

- Periodic unique work named `BackupCreator`, interval = `backup_interval` hours (0 = off; UI values 6, 12, 24, 48, 168; default 12). Flex 10 minutes. Exponential backoff base 10 minutes. Constraint `requiresBatteryNotLow`. Policy UPDATE on each `setupTask`. Cancelled when the interval is 0.
- Retry only when a restore is running (`Result.retry()`), otherwise failure is terminal for that run.
- Relies on the OS to wake the app: WorkManager runs the job even when the app process is dead (within Doze limits).

## 2. What Kurobayomi does (proposal)

Two parts:

1. In-process loop, runs while the GUI is open. Ticks every 10 minutes (`TICK`), evaluates `decide(...)` (pure function, p1-rust-design.md section 8). Keeps state in one persisted timestamp `state.backup.last_auto_epoch_ms`.
2. Optional background run (P2, T-21a-041): `systemd --user` timer that starts `kurobayomi --run-backup` for users who keep the app closed. The GUI is not started in that mode.

### 2.1 Due rule

```
due = last_auto_epoch_ms + interval_hours * 3_600_000
if last_auto_epoch_ms == 0 { due = enabled_at_epoch_ms + interval_hours * 3_600_000 }   // E-08
if last_auto_epoch_ms > now { last = now }                                             // E-31
run when now >= due
```
`enabled_at_epoch_ms` is written when the interval changes from 0 to a non-zero value (stored in `state.backup.enabled_at_epoch_ms`, app state). This avoids an immediate backup right after enabling.

Catch-up: after the app was closed for longer than the interval, the first tick after start runs the backup (WorkManager may or may not do this; Kurobayomi does so deliberately, and the first tick is delayed 60 seconds after startup so that the library has loaded).

### 2.2 Deferrals

| Reason | Check | Action | Retry |
|---|---|---|---|
| Restore running | restore job slot held (area:22) | defer | 10 min |
| Manual backup running | job slot held | skip this tick (no queue) | next tick |
| On battery and charge below threshold | UPower: `org.freedesktop.UPower` `OnBattery` true and `DisplayDevice.Percentage` < 15 | defer | 10 min |
| Battery state unknown (no UPower) | D-Bus error | treat as not on battery | none |

Battery threshold: 15% (Proposal). Reason: Android's "battery not low" condition uses the low-battery level (commonly 15%). area:26 proposes 20% for the library update job. The two MUST be reconciled in the cross-area review; this topic uses 15% for backups, which are heavier to skip. Config key: `backup.min_battery_percent_on_battery: u8 = 15` (allowed 5..=50).

### 2.3 Retry and backoff

- After a failed automatic run: `delay = 10 min * 2^(attempt - 1)`, capped at the interval, attempt counter starts at 1 and resets on success. Replaces Kotlin EXPONENTIAL backoff with base 10 minutes.
- The failed run does not advance `last_auto_epoch_ms`, so the due rule keeps the run pending (differs from Kotlin, which waits a full period after a failure; see E-10).
- Deferrals (restore, battery) use a fixed 10-minute retry and do not count as failures.
- After 3 consecutive failures a single warning notification is shown (E-10, p1-ui-parity.md section 3).

### 2.4 Single-flight

One `tokio::sync::Mutex<()>` job slot shared with manual runs (job.rs). Auto tick: `try_lock`; if held, defer (skip). Manual: `try_lock_owned`; if held, refuse with `InProgress`.

### 2.5 Applying a settings change

`backup.interval_hours` changes publish a new `AutoConfig` through `tokio::sync::watch`. The loop wakes on the watch (`tokio::select!`) and re-evaluates immediately, not on the next tick. Setting 0 cancels the pending run. Setting a new non-zero value resets `enabled_at_epoch_ms` only when the previous value was 0.

## 3. Retention and files

Automatic files live in `<automatic dir>`; retention = newest 4 files that match the Kurobayomi name pattern (B-07, T-21a-025). Pruning happens after the new file is verified and renamed (E-04). The directory is created on demand. A stale `*.tachibk.part` is deleted at startup.

## 4. Failure cases

| Case | Behaviour |
|---|---|
| Automatic directory unavailable (unmounted disk) | run fails with `Io`; retry with backoff; warning after 3 failures (E-21) |
| Disk full | run fails; `.part` removed; previous files untouched |
| App crashes mid-run | `.part` left behind; removed at next startup; `last_auto` not advanced, so the run is due again at the next tick |
| Clock change | E-31 rule |
| Two instances of the app running | advisory lock file `autobackup/.lock` (flock). Second instance skips automatic ticks. Manual runs in the second instance proceed (they write to a user-chosen path) |
| `--run-backup` runs while GUI is open | flock prevents two automatic runs; the GUI's loop sees the new `last_auto` on its next tick (re-read the timestamp from the config on each tick) |

## 5. systemd user units (P2, T-21a-041)

These are the exact files to ship (installed by the package, not by the app). Paths are Proposal.

`~/.config/systemd/user/kurobayomi-backup.service` (no `ConditionACPower`: the in-app battery rule is used instead, so `--run-backup` applies the same UPower threshold as the GUI loop):

```ini
[Unit]
Description=Kurobayomi automatic backup

[Service]
Type=oneshot
ExecStart=%h/.local/bin/kurobayomi --run-backup
Nice=10
IOSchedulingClass=idle
```

`~/.config/systemd/user/kurobayomi-backup.timer`:

```ini
[Unit]
Description=Run Kurobayomi automatic backup check every 30 minutes

[Timer]
OnBootSec=5min
OnUnitActiveSec=30min
RandomizedDelaySec=2min
Persistent=true

[Install]
WantedBy=timers.target
```

Behaviour: the timer checks every 30 minutes; `--run-backup` runs `decide(...)` and performs at most one run. The `interval_hours` value is read from the same config file. `Persistent=true` fires once after the machine resumes if a tick was missed. Installing: `systemctl --user enable --now kurobayomi-backup.timer`. The timer is not enabled by the app; the settings page offers a button "Run in background when Kurobayomi is closed" that only writes the unit files and prints the enable command (P2 UX; no privileged action).

## 6. Clock and time zone

- Timestamps are epoch milliseconds (UTC). File names are local time (B-07).
- Due arithmetic uses UTC milliseconds only; DST changes do not shift due times.

## 7. Shared design with area:26-library-update-jobs

The library update job has the same shape (interval, constraints, backoff). Shared pieces to put in one module (proposed `src/jobs/schedule.rs`, owned by area:26):
- `TICK` and `RETRY_BASE` constants (both 10 minutes).
- The power check (`PowerState` from UPower) and the battery threshold (one value for both jobs: resolve 15 vs 20).
- The single-flight slot abstraction.
- The "due" arithmetic with the first-run rule (E-08).

Backup-specific: retention, file names, restore deferral.
Library-specific: network restrictions (`WifiOnly`, `NotMetered`, `Charging`; the set is defined by area:26).

## 8. Test cases

See p1-edge-cases-and-tests.md section 4.6 (decision table) and 4.9 (single-flight). Additional manual test: set interval to 6, close the app for 7 hours, reopen: exactly one run within 2 minutes of startup (catch-up rule), then none for the next 6 hours.
