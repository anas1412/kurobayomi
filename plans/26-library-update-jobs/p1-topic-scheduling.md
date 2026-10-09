# Topic: scheduling and background running (area 26)

Deep dive on how library passes are started, constrained, retried and stopped. Kotlin facts are in `p1-behavior.md` sections 1-2; this file decides the desktop design.

## 1. What Android gives (OBSERVED)

- `MangaLibraryUpdateJob.setupTask` / `AnimeLibraryUpdateJob.setupTask` enqueue a `PeriodicWorkRequest` (interval hours, flex 10 min) with `ExistingPeriodicWorkPolicy.UPDATE`. Constraints: network type `UNMETERED` or `CONNECTED`, network request (Wi-Fi transport and/or not-metered capability on API 28+), charging when `ac` is set, battery-not-low always.
- Backoff `LINEAR` 10 min. Pre-checks in `doWork` return `Result.retry()` for the Wi-Fi check on API < 28 and when a manual pass is running (manga only).
- One-time manual work: `ExistingWorkPolicy.KEEP` with unique name `LibraryUpdate-manual` / `AnimeLibraryUpdate-manual`.
- WorkManager persists the schedule across reboots and process death, runs under the OS job scheduler, and does not replay missed periods.

## 2. Desktop constraints

- A GTK app is not running when the user closes the window unless it is told to stay alive. Scheduled passes therefore need either a resident process or an OS timer.
- There is no Doze; there is no metered-network API besides NetworkManager.
- Charging state comes from UPower, not from a platform job API.

## 3. Decision (Proposal)

Two modes, user-selectable in the Library settings (area:02-preferences-system owns the switch; this area owns the behaviour):

| Mode | Mechanism | When it runs | Default |
|---|---|---|---|
| A. Resident scheduler | in-process tokio loops (`core::scheduler`), background portal `org.freedesktop.portal.Background.RequestBackground` with autostart | while the process lives; the window may be closed | on, if the portal grants it |
| B. systemd user timer | `~/.config/systemd/user/kurobayomi-update.timer` (`OnUnitActiveSec=` = interval hours, `Persistent=true`, `RandomizedDelaySec=600`) running `kurobayomi --library-update=auto` (a headless run of `run_pass` for each kind, then exit) | regardless of GUI | off |

Rules:
- Mode A and B must not both be active. Enabling B disables A's autostart.
- In mode B the headless run takes the same per-kind lock in SQLite (`library_job_run`); if the GUI already runs a pass, the headless run exits 0 with "already running".
- The headless run sends notifications through `notify-rust` like the GUI run.
- When the portal is unavailable (no sandbox or portal missing), mode A works only while the process runs; the settings page shows a note: "Automatic updates run only while Kurobayomi is open." (proposal text).

## 4. Loop design (mode A)

```
per kind (manga, anime) one task:
  loop:
    cfg = watch.borrow()                       // interval_hours, restrictions
    if cfg.interval_hours == 0: changed().await; continue
    wait_until_constraints(cfg.restrictions)   // poll net_state every 30 s; cancellable by config change
    if lock_is_held_by_manual(kind): sleep(backoff(attempt)); attempt += 1; continue
    outcome = run_pass(Auto, ...)
    attempt = 0 on success
    sleep(interval - rand(0..=10 min))         // flex window: next run within [interval-10min, interval]
```
- `backoff(n) = min(10 min * n, 5 h)` (LINEAR). Only used for `Retry` results.
- Config changes call `watch.send_replace`; the loop wakes via `changed()` in a `select!` with the sleep. This mirrors `ExistingPeriodicWorkPolicy.UPDATE`.
- `stop(kind)` cancels the running pass token; the loop continues (re-armed), like `stop()` re-enqueuing the auto work.

## 5. Parity decisions to confirm (open)

| Question | WorkManager behaviour | Proposal | Task |
|---|---|---|---|
| First auto run after enabling | runs as soon as constraints are met | same (parity); the user sees a pass right after enabling | T-26a-010 |
| Missed runs while closed | not replayed | not replayed; next run at next start if due | T-26a-011 |
| Constraint "battery not low" | enforced | not enforced on desktop (no meaningful signal on mains power); documented deviation | T-26a-012 |
| Manual pass during auto | auto returns retry | same | T-26a-015 |
| Metadata passes scheduled? | manual only | manual only | T-26a-050 |

## 6. Startup sequence

1. Load prefs (area:02-preferences-system).
2. Mark stale `library_job_run` rows `running` as `cancelled` (crash recovery).
3. Arm loops for kinds with interval > 0.
4. If mode B is configured, verify the timer unit exists and is enabled (`systemctl --user is-enabled`); show a settings warning if not. Do not auto-install units without the user's action.

## 7. Single-instance

GApplication with `ApplicationFlags::HANDLES_COMMAND_LINE`: a second `kurobayomi --library-update=manual` forwards to the primary instance (action `start-update`) instead of running a second pass. This keeps one scheduler and one lock holder.

## 8. Notes for the engineer

- Use monotonic `tokio::time` for waits; persist only wall-clock timestamps.
- Never block the GTK thread on D-Bus; `zbus` calls run on the tokio runtime.
- Log constraint decisions at `debug!` once per change, not per poll.
