# Topic: update pipeline (queue, fetch interval, sync, download hand-off)

Deep dive for the per-entry pipeline. Facts: `p1-behavior.md` sections 3-6. This file records the design decisions and the places where the port must differ from or fix the Kotlin.

## 1. Stages

```
library rows ──► build_queue ──► group by source ──► [per source, sequential] ──► per entry:
                                                          gate (favourite) ─► fetch details? ─► fetch list
                                                          ─► sync (diff + writes) ─► new items
                                                          ─► download filter ─► enqueue (paused)
                                                          ─► counters
                                    ──► pass end: summary notification ─► start downloads ─► error log
```

## 2. Queue (`build_queue`)

Decisions:
- Category selection exactly as behaviour 3.2. Manual category runs ignore include/exclude (OBSERVED, keep).
- Restrictions exactly as behaviour 3.3, including the unconditional "not always update" check.
- Restrictions apply to manual passes too (OBSERVED). Keep. The user-facing consequence (manual "Update" skips completed series by default) is documented in the settings help text.
- Season expansion for anime before restrictions (behaviour 3.4).
- Output sorted by title. The title sort is only cosmetic for progress order because sources run in parallel, but it determines group order (first appearance), which determines which source starts first.

## 3. Fetch window and next-update (`fetch_interval.rs`)

Summary: a window of "yesterday 00:00 .. tomorrow 00:00" in the local zone decides whether an entry is due. Entries due today are checked; entries checked too early or too late are rescheduled by `next_update`.

Key facts to keep:
- Interval is the median-free "average gap over the last 3 or 10 distinct days" computed with floor division. It never leaves 1..=28 unless the user sets a negative manual override.
- The increase rule doubles the interval after more than 10 missed cycles, capped at 28.
- The window is computed once per pass (`updateChapterList` line 256) and reused for all entries; `(0,0)` means "compute now" (only used by direct calls).

Port notes:
- Use `chrono` naive dates with a single zone conversion per timestamp. Tests FI-01..FI-23 in `p1-edge-cases-and-tests.md`.
- The Kotlin code is duplicated for manga and anime. The port has one implementation.

## 4. Sync (`sync.rs`)

The sync is the only place where the library database changes during a pass. The port keeps the Kotlin diff semantics and changes three implementation details:

| Kotlin | Port | Reason |
|---|---|---|
| `removedChapters = dbChapters.filterNot { sourceChapters.any { url == } }` (O(n*m)) | `HashSet` of source urls | performance (PERF-02) |
| `deletedChapterNumberDateFetchMap` built from a sorted list with `associate` (last wins) | same semantics via explicit loop, comment kept | correctness documented |
| `TreeSet<Double>` for numbers | `BTreeMap<OrderedFloat<f64>, ..>` or `Vec` sorted + dedupe; NaN rejected at parse | NaN safety (S-15) |

Writes performed (in this order, behaviour 5.4): remove, add, update, interval, last-update. Each group in one transaction (`rusqlite` `transaction()`); a failure rolls back that group only and the entry fails.

Sync must not change `read`, `bookmark` of existing rows except through the explicit carry-over rules (S-07, S-08, S-09). Updating existing rows writes only: name, chapter number, scanlator, source order, date upload (when non-zero) for chapters; plus summary, memo, fillermark (sticky), preview url (sticky) for episodes (R-05).

## 5. Fix list for the pipeline (deliberate deviations)

| Id | Kotlin | Port | Ticket |
|---|---|---|---|
| P-1 | anime new episodes never enqueued | enqueue with autostart=false like manga | T-26a-024 |
| P-7 | anime syncs before favourite recheck | recheck favourite before sync for both kinds | T-26a-024 |
| P-8 | notification tap opens `chapters.first()` (oldest in the sorted list) | open the newest new item (highest number) | T-26a-034 |
| P-10 | cancellation recorded as failure | cancellation not a failure | T-26a-021 |
| P-13 | metadata passes process duplicates | distinct by id | T-26a-051 |
| — | per-source semaphore only within one kind | add a process-wide per-source permit (1) so manga and anime passes never hit the same source at once | T-26a-022 |

## 6. Download hand-off

- Manga: `downloads.enqueue(entry, items, autostart=false)` then one `downloads.start()` at pass end, only if at least one item was enqueued (OBSERVED).
- Anime: same call shape once P-1 is fixed.
- Filters: `FilterChaptersForDownload` / `FilterEpisodesForDownload` semantics (behaviour 6.3): global toggle, optional unread-only with number matching, category include/exclude with the default category `0` when a manga has no category.
- Enqueue is idempotent per item id (R-06).

## 7. Counters

`new_manga_updates_count` and `new_anime_updates_count` are incremented once per entry by the number of new items after the sync filter. They reset when the user opens the corresponding Updates tab (UI behaviour owned by area of the Updates screen; this area only increments). Keep the exact key names from `p1-data-and-config.md` section 3 so backup/restore of preferences keeps counters.

## 8. Errors

- Per-entry error = `UpdateError` (section 2.1 of the Rust design). The message text is stored in the failure list and written to the error log.
- A pass never fails because of an entry. The pass fails only when the queue cannot be built (DB error) or when the notification/log step throws (logged, pass still marked failure).

## 9. Ordering guarantees (for tests)

- Within one source: entries are processed in queue order (title sorted).
- Within one entry: fetch details (if enabled) before chapters.
- New items passed to notifications: summary by number ascending, per-entry description by number ascending, tap target = highest number (after P-8 fix).
- Downloads start after the summary notification is posted (Kotlin order at `MangaLibraryUpdateJob.kt:316-321`).
