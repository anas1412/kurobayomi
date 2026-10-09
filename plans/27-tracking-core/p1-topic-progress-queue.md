# Topic: progress retry queue and scheduler

Scope: the offline and failure buffer for tracker progress, and the background job that drains it. Source: `app/src/main/java/eu/kanade/domain/track/anime/store/DelayedAnimeTrackingStore.kt`, `.../manga/store/DelayedMangaTrackingStore.kt`, `.../anime/service/DelayedAnimeTrackingUpdateJob.kt`, `.../manga/service/DelayedMangaTrackingUpdateJob.kt`, and the callers `TrackEpisode.kt`, `TrackChapter.kt`, `ui/player/ExternalIntents.kt:524-560`. Behaviour is `p1-behavior.md` sections 9, 10 and 22. Rust design is `p1-rust-design.md` sections 3.12 and 6.

## 1. Why a queue exists

A progress update (for example "episode 6 seen") must not be lost when the device is offline or the tracker is down. The reader and player fire the update and move on. The queue stores the intent, and a background job replays it later.

## 2. Source behaviour in one page (OBSERVED)

| Element | Source | Notes |
|---|---|---|
| Container | SharedPreferences `tracking_queue` | shared by both kinds (D3) |
| Key | `trackId.toString()` | track `_id`, which changes on writes (D1) |
| Value | `Float` | precision loss (D7) |
| Insert rule | write only if new value > stored value | keeps the max; not atomic (D6) |
| Remove | on success of the online path, or when the binding no longer exists | `TrackEpisode.kt:44`, `DelayedAnimeTrackingUpdateJob.kt:40-42` |
| Job trigger | `enqueueUniqueWork(TAG, REPLACE, request)` from the queueing path | `DelayedAnimeTrackingUpdateJob.kt:76` |
| Constraint | network connected | `:67` |
| Backoff | exponential from 5 minutes | `:72` |
| Attempts | body runs while `runAttemptCount <= 3` | `:27` |
| Done | queue empty → success; items left → retry | `:59` |

## 3. Data model (Proposal)

Table `track_retry_queue` (defined in `p1-data-and-config.md` section 7.2):

```
(kind, entry_id, tracker_id) -> (progress REAL, queued_at INTEGER, attempts INTEGER)
PRIMARY KEY (kind, entry_id, tracker_id)
```

Why this key:
- `entry_id` is the entry the user is reading. The binding is looked up at run time by `(entry_id, tracker_id)`, which is stable. The track `_id` is not used (D1).
- `kind` separates anime and manga (D3).
- `tracker_id` is the binding's service, so one entry can have several queued trackers.

## 4. State machine per item

```
            add(progress)                    run ok (updated)
  [none] ───────────────────▶ [queued a=0] ─────────────────────▶ [removed]
                                   │
                                   │ run: binding missing
                                   ├──────────────────────────────▶ [removed]
                                   │
                                   │ run: skipped (remote already >= progress)
                                   ├──────────────────────────────▶ [removed]  (Proposal, D5)
                                   │
                                   │ run: failed, a < MAX_ATTEMPTS
                                   ▼
                              [queued a+1] ──wait backoff──▶ (run again when online)
                                   │
                                   │ run: failed, a == MAX_ATTEMPTS
                                   ▼
                              [parked]  (kept; restarted by next add or kick)
```

Transitions:
- `add(progress)` when an item exists: `progress = max(old, new)`. `attempts` is not reset by add (the source resets by REPLACE of the job; see section 6).
- `kick()` (called on every queueing event and at start): resets `attempts` to 0 for all items and wakes the scheduler. This matches `ExistingWorkPolicy.REPLACE`.
- `remove` happens only in the three removal rows above.

## 5. Scheduler loop (Proposal, mirrors the Kotlin job)

```
loop {
    wait for kick() OR startup
    if queue empty: continue waiting
    wait until connectivity.is_online()              // NetworkType.CONNECTED
    for item in queue.list():                        // anime then manga, in insertion order
        binding = tracks.get_for(item.entry_id, item.tracker_id)
        if binding is None:  remove(item); continue
        match run_item(item).await {
            Ok(true)  => remove(item)                // updated or skipped-at-remote
            Ok(false) => remove(item)                // Proposal: skipped
            Err(e)    => keep(item, attempts + 1); log warn
        }
    if queue has items with attempts < MAX_ATTEMPTS:
        sleep(backoff(max attempts))                 // 5 min * 2^(n-1), cap 5 h
        continue
    else: park; log warn; wait for kick()
}
```

Notes:
- The Kotlin job runs the whole list in one `doWork`. The Rust loop processes per item so one bad tracker does not block the others (Proposal).
- `run_item` is the same logic as `TrackEpisode`/`TrackChapter` for one binding: refresh, copy with progress, update, insert, remove. The Kotlin job reuses `TrackEpisode.await(..., setupJobOnFailure = false)`, so it also checks the `<=` skip rule. That skip rule is the cause of defect D5.

## 6. Attempt and backoff arithmetic

| Attempt index n (after failure) | Delay before next run | Source |
|---|---|---|
| 1 | 5 min | `BackoffPolicy.EXPONENTIAL`, initial 5 min |
| 2 | 10 min | exponential doubling (WorkManager `Math.scalb` formula, verify) |
| 3 | 20 min | |
| 4 | 40 min | the body has run four times; the fifth start is refused |
| cap | 5 h | WorkManager maximum backoff |

The attempt index is the number of runs that have already failed. The body runs at most four times per `kick`. After that the items are parked until the next `kick`.

`kick()` resets the counter. In practice a user who keeps reading offline keeps resetting it (each reader event calls `setupTask`). The queue therefore does not give up while the user is still active. Parking happens only when the user stops producing events. This is the observed behaviour and the Proposal keeps it.

## 7. Startup

On process start the scheduler:
1. Reads `track_retry_queue`.
2. If non-empty, calls `kick()` once.
3. Waits for connectivity and runs.

This replaces WorkManager's persistence across reboots. The Kotlin job also survives process death through WorkManager; the Rust scheduler survives it through SQLite.

## 8. Concurrency

- `add` is one `INSERT ... ON CONFLICT ... DO UPDATE SET progress = MAX(progress, excluded.progress)` statement (defect D6 fixed).
- `remove` is conditional: `DELETE FROM track_retry_queue WHERE kind = ? AND entry_id = ? AND tracker_id = ? AND progress <= ?`, passing the progress value the run used. A newer `add` that happened during the run is therefore kept (race R2 in `p1-edge-cases-and-tests.md`).
- Progress updates for the same binding are serialised by a per-binding mutex (race R1). The scheduler and the live path use the same mutex.
- The scheduler is a single task. Enqueue wakes it through a `tokio::sync::Notify`.

## 9. Relationship to the live path

| Live path | When it queues | When it does not queue |
|---|---|---|
| anime `TrackEpisode` (behaviour 9.1) | offline only | any online error (source) |
| manga `TrackChapter` (behaviour 9.2) | any failure | none |
| external player (`ExternalIntents.kt:524`) | offline only | online errors (source does `runCatching`) |

**Proposal:** all three use the same rule: queue on any failure, and check connectivity first to avoid a request that is known to fail. One function (`track_progress(kind, entry_id, progress, policy)`) serves all three. This removes the difference described in behaviour sections 9.1 and 9.2.

## 10. Edge cases

| Case | Behaviour |
|---|---|
| Binding deleted by the user while queued | removed on next run (source behaviour) |
| Tracker logged out while queued | the binding is skipped by the tracker check, so the item is kept. Proposal: remove when the tracker is not logged in and the item is older than 30 days (kept otherwise, so re-login resumes it) |
| Remote is ahead of the queued progress | skipped and removed (Proposal; source keeps it, D5) |
| Queue grows large (1000 items after months offline) | processed in insertion order; each run takes one request per item, rate limited per tracker (`p1-rust-design.md` section 4) |
| Clock jumps backwards | `queued_at` is informational only; backoff uses the scheduler's monotonic clock |
| App closed during backoff | the queue persists; startup `kick` restarts the cycle immediately |

## 11. Tests

See `p1-edge-cases-and-tests.md`: TT-27-D02, D03, D04, D05, D06, D07, TT-27-121 to TT-27-127, and R1, R2.

## 12. Open questions for the user

1. Should a parked queue (after four failed runs) show a toast on the next entry open? Proposal: yes, one line "N tracking updates pending".
2. Is a 30-day expiry acceptable for queue items of logged-out trackers? Alternative: keep forever until removed by the user.
