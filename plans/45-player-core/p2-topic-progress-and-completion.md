# Topic: progress, completion, history and downloads (45-player-core part 2)

Deep dive for what happens to the database and the download queue while an episode plays. Full algorithms: `p2-behavior.md` §7. Edge cases: E-13 to E-17, E-29, E-46 to E-50. Tasks: T-45b-026 to T-45b-030, T-45b-040, T-45b-056, T-45b-027, T-45b-028, T-45b-029.

## 1. Clock

mpv `time-pos` is observed as INT64, so the progress code runs once per whole second of playback. The position is passed as seconds; `duration` is whole seconds. Everything below is in that cadence. Changing the observed format to float would multiply DB writes and change the skip countdown (E-10 and pitfall 1 in the Rust design).

## 2. What runs on each tick (`onSecondReached`)

```
guards:  not loading episode, episode present, episode id != -1, duration != 0
set in memory:  last_second_seen = pos*1000, total_seconds = duration*1000, episode_position = pos*1000
threshold:      if pos*1000 >= total*1000 * progress_preference (default 0.85) AND (not incognito OR has trackers)
                  -> completion (once, E-14 rule) in NonCancellable
progress save:  save_progress(episode) + save_history(episode)           (every tick)
download range: if pos/duration > 0.35 -> download_next_episodes()        (every tick, throttled by E-13 rule)
```

Rust rule summary:
- Completion runs once per episode and sets `seen = true` in memory first.
- Progress and history writes are coalesced: progress every tick is fine (small row update), history every 5 s plus pause and close (E-47).
- The episode row is captured by value at call time (race 3 in the E doc).

## 3. Writes, field by field

| Write | Fields | Gate | Table |
|---|---|---|---|
| Progress | `seen`, `bookmark`, `fillermark`, `last_second_seen`, `total_seconds` (Kotlin) → Rust writes only changed fields (E-46) | `!incognito_source || has_trackers` | `episodes` |
| History | `episode_id`, `last_seen = now` (upsert) | `!incognito_source` | `animehistory` |
| Completion | `seen = true` + tracker update + delete-after-read + optional duplicate marking | completion threshold | `episodes`, tracker, download queue |
| Bookmark / filler | single flag | user action | `episodes` |

Triggers (data §8.1): any change to `seen`, `bookmark` or `last_second_seen` bumps `episodes.version` (and the anime version), so sync sees progress changes.

## 4. Incognito matrix (E-16, E-17)

| Setting | Progress saved | History saved | Tracker sync |
|---|---|---|---|
| per-source incognito off, global off | yes | yes | yes (if trackers and auto-update) |
| per-source incognito on, no tracker | no | no | no |
| per-source incognito on, has tracker | yes | no | depends on global flag only |
| global incognito on | depends on per-source rule | depends on per-source rule | no (`basePreferences.incognitoMode()`) |

Note the asymmetry in the last row: the tracker path checks the global flag, while progress and history check the per-source flag. This is the Kotlin behaviour; the port keeps both flags and names them (`incognito_source`, `incognito_global`) so the code is explicit.

`incognito_source` is recomputed on each episode switch (E-16), not cached for the whole ViewModel lifetime.

## 5. Completion side effects (in order)

1. `seen = true` in memory (before any save, E-14).
2. Tracker update through `TrackEpisode` (only if not global incognito, has trackers, and `pref_auto_update_manga_sync_key` is true, default true).
3. Delete-after-read: `remove_after_read_slots` (default -1 = off). Index rule: the episode at `index - slots` of the playlist is queued for delete when it is seen; with slots = 0 the completed episode itself is queued.
4. Duplicate marking: when `mark_duplicate_seen_episode_seen` contains `existing_episode`, every unseen recognised episode with the same episode number in the FILTERED playlist is marked seen (E-50).

## 6. Download-ahead

- Starts only past 35 % of duration (`0.35` at PlayerViewModel.kt:1682).
- Only when the current and the next episode are already downloaded (to avoid jank, PlayerViewModel.kt:1716-1728).
- Takes `auto_download_while_watching` episodes after the next one (default 0 = off, read once at ViewModel construction).
- Kotlin calls it every tick past the threshold (E-13). Rust: once per episode per session (`download_ahead_started` set), and the download manager de-duplicates as well (area:24).

## 7. Shutdown

- On close the current episode is saved synchronously with a 2 s deadline (E-29); the Kotlin `onCleared` saves on the ViewModel scope and may be interrupted.
- `episodeToDownload` re-queue on close is dead code in the Kotlin (E-18) and is dropped.
- Deferred deletion (`deletePendingEpisodes`) runs when the player exits (PlayerActivity.kt:335, 352).

## 8. Failure handling

- `UpdateEpisode.await/awaitAll` swallow exceptions (UpdateEpisode.kt:12-27). Port: keep swallow-and-log for progress, but after 3 consecutive failures show a non-blocking warning (E-49).
- Tracker failure is logged; no retry (behavior §19 interactions, area:27).
- History upsert failure: logged; the next tick retries naturally.

## 9. Tests (E doc §4.11, §4.8 T56-T62)

- T53 threshold exactly at 85 %.
- T54/T55 completion once and `seen` set before save.
- T56/T57 incognito combinations.
- T58 history payload shape.
- T59 delete-after-read slot rules.
- T60/T61 download-ahead once and range.
- T62 bookmark updates memory.
- T46-T49 are volume, not this topic.

## 10. Open decisions

- Whether `pref_preserve_watching_position` (default false) should resume at the last position. Kotlin stores `episode_position` in savedState for process death only; the pref's exact effect is in part 1. Port: keep the pref and implement resume only when the flag is true, flagged for confirmation.
- Whether history should coalesce to every 5 s (E-47). Default in the port: yes, with a flush on pause and close.
