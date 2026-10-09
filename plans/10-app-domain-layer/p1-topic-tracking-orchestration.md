# Area 10 topic: tracking orchestration, offline queue, enhanced trackers

Files: everything under `app/src/main/java/eu/kanade/domain/track/` (17 files) plus the callers named below. The tracker implementations (AniList, MyAnimeList, Kitsu, Shikimori, Bangumi, MangaUpdates, Simkl and the enhanced trackers) are area:27 to area:33; this area is only the glue that decides WHEN and WHAT to push or pull.

## 1. When progress is pushed (the callers, because they define the trigger rules)

| Trigger | Condition (OBSERVED) | Calls |
|---|---|---|
| Reader reaches the last page of a chapter (`updateChapterProgressOnComplete`, `ReaderViewModel.kt:553-556`) | not incognito, `trackPreferences.autoUpdateTrack()` (`pref_auto_update_manga_sync_key`, default true), manga known; run in `launchNonCancellable` (`ReaderViewModel.kt:915-928`) | `TrackChapter.await(context, mangaId, chapter_number.toDouble())` |
| Player marks an episode seen | not incognito, `hasTrackers`, `autoUpdateTrack` (`PlayerViewModel.kt:1947-1956`) | `TrackEpisode.await(context, animeId, episode_number.toDouble())` |
| User marks chapters read in the list (`MangaScreenModel.kt:760-796`) | `read == true`, at least one logged-in tracker, `autoTrackState != NEVER`; `maxChapterNumber = chapters.maxOf { chapterNumber }`; prompt only if some track has `lastChapterRead < maxChapterNumber`. `ALWAYS`: call and toast `AYMR.strings.trackers_updated_summary_manga` ("Trackers updated to chapter %d", `maxChapterNumber.toInt()`). `ASK`: snackbar `MR.strings.confirm_tracker_update` ("Update trackers to chapter %d?", action `MR.strings.action_ok`, short duration, dismiss action), call only when the action is pressed | `TrackChapter.await` |
| Anime equivalent (`AnimeScreenModel.kt:875-910`) | same with `confirm_tracker_update_anime` ("Update trackers to episode %d?") and `trackers_updated_summary_anime` ("Trackers updated to episode %d") | `TrackEpisode.await` |
| Entry added to library / opened from history / details loaded | enhanced trackers only | `AddMangaTracks.bindEnhancedTrackers(manga, source)` |
| Track info sheet opened, pull to refresh | all logged-in trackers | `RefreshMangaTracks.await(mangaId)` |
| Tracker search result chosen by the user | `Tracker.register(item, mangaId)` -> `AddMangaTracks.bind(tracker, item, mangaId)` (`MangaTracker.kt:60`, `AnimeTracker.kt:61`) | bind |

Message-key parity matters: the toast shows the INTEGER part of the chapter number (`maxChapterNumber.toInt()`).

## 2. TrackChapter / TrackEpisode state machine

```
for each stored track T of the entry:
  skip if tracker missing || !loggedIn || number <= T.lastRead
  online?  (anime only: context.isOnline();  manga: always attempts)
    yes -> refresh(T) -> set lastRead=number -> tracker.update(T, didRead=true) -> store row -> remove queue entry
           failure: manga -> queue(T.id, number) [+ schedule worker], log INFO
                    anime -> log INFO only (NOT queued)
    no  -> (anime) queue(T.id, number) [+ schedule worker if setupJobOnFailure]
```
All tracks of an entry are processed concurrently; `awaitAll` then logs every failure at INFO (never surfaced to the user). `TrackChapter` has a dead duplicated condition (`if (cond) { if (cond) return }`) - behaviour equals a single check.

Proposal: unify both behaviours: treat any failure as "queue and retry" for BOTH kinds (a single `Result<(), TrackError>`); distinguish `TrackError::Offline` (no retry-scheduling noise) from `TrackError::Auth` (expired token: do not queue, set `trackAuthExpired` via area:27 and show the re-login prompt) and `TrackError::Other` (queue). This removes the anime-only gap where an online-but-failing update is lost.

Connectivity (replaces `Context.isOnline()` from `app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkExtensions.kt:16`, which checks `NetworkCapabilities` transports CELLULAR..LOWPAN): Proposal `Connectivity::is_online() -> bool` in `kuro-platform` using `gio::NetworkMonitor::default().connectivity() >= NetworkConnectivity::Limited` (GIO reads NetworkManager/portals; no extra dependency since gtk4-rs re-exports gio). Treat `Limited`/`Portal` as online-but-uncertain: attempt and rely on the failure path.

## 3. The offline queue

OBSERVED storage: Android `SharedPreferences` `tracking_queue`, key `trackId.toString()`, `putFloat`. Rules:
- `add(trackId, value)`: write only if `value > stored (default 0f)`. So the queue keeps the highest progress per track. Value is truncated to `Float` (32-bit).
- `remove(trackId)`; `getItems()` lists everything.
- Manga and anime stores share the same file (collision bug, see below).

Proposal (`kuro-domain/src/app/track/delayed.rs`):
```rust
pub struct DelayedTrackingStore { path: PathBuf, inner: Mutex<BTreeMap<QueueKey, f32>> }
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct QueueKey { pub kind: EntryKind, pub track_id: i64 }
impl DelayedTrackingStore {
    pub fn add(&self, key: QueueKey, last: f64);            // only if (last as f32) > stored.unwrap_or(0.0)
    pub fn remove(&self, key: QueueKey);
    pub fn items(&self, kind: EntryKind) -> Vec<(i64, f32)>;
}
```
File `<data_dir>/tracking_queue.json`: `{"manga":{"12":150.5},"anime":{"3":7.0}}`. Every mutation rewrites the file atomically (`tempfile` in the same directory then `persist`). Keys are namespaced by kind, which fixes the collision where `DelayedMangaTrackingUpdateJob` would read anime ids (and delete entries whose id does not exist in the manga track table, line `delayedTrackingStore.removeMangaItem(it.trackId)`), and vice versa. Import: if a legacy Android `tracking_queue.xml` is ever restored from a backup it is ignored (the queue is not part of Aniyomi backups).

## 4. The retry worker

OBSERVED (`DelayedMangaTrackingUpdateJob.doWork`, anime twin identical):
1. `runAttemptCount > 3` -> `Result.failure()` (the queue entries stay in the store; they are retried the next time a new offline failure schedules the work).
2. For each queued item: load the track by id (`getTracks.awaitOne`); missing -> delete the entry; else replay with `TrackChapter.await(context, mangaId, lastRead, setupJobOnFailure = false)` (failures re-queue without scheduling another job).
3. `Result.success()` if the queue is empty, otherwise `Result.retry()` -> WorkManager reschedules with exponential backoff starting at 5 minutes (`BackoffPolicy.EXPONENTIAL`, 5.minutes) (the delays between the four runs are 5, 10 and 20 minutes). Attempts 0..3 are executed, i.e. at most four runs.
4. `setupTask`: `enqueueUniqueWork(TAG, ExistingWorkPolicy.REPLACE, request)` with `Constraints(requiredNetworkType = CONNECTED)`; tags `DelayedMangaTrackingUpdate`, `DelayedAnimeTrackingUpdate`. REPLACE resets the attempt counter on every new offline failure.

Proposal: `DelayedTrackingWorker` is a tokio task owned by `Domain` (not an OS scheduler job: the app must be running to push; if it is closed the queue is replayed on next start, see start hook below):
```rust
impl DelayedTrackingWorker {
    pub fn schedule(&self);                       // idempotent: restarts the backoff, like REPLACE
    async fn run(self: Arc<Self>) {               // select! on cancel
        let mut attempt = 0u32;
        loop {
            wait_until_online().await;            // NetworkMonitor::connect_network_changed + notify
            self.replay_all().await;              // calls TrackChapter/TrackEpisode with setup_job_on_failure=false
            if self.store.is_empty() || attempt >= 3 { break; }
            sleep(Duration::from_secs(300 << attempt)).await;   // 5, 10, 20 minutes
            attempt += 1;
        }
    }
}
```
Startup hook: `Domain::start()` calls `schedule()` if the stored queue is non-empty (Kotlin has no such hook; WorkManager persisted the job across app restarts, which this replaces).

## 5. AddTracks.bind: pushing local state at bind time

Algorithm in p1-behavior.md 12.5. Details worth restating for implementers:
- "Latest local read chapter" is the highest number of the contiguous read prefix when chapters are sorted ASCENDING by number, NOT the maximum read number. Example: read {1,2,3,5}, unread 4 -> latest = 3. Unrecognised numbers (-1) sort first; with unread -1 chapters at the front the prefix is empty and the default `-1.0` applies.
- The remote receives `latest.toInt()`: reading chapter 12.5 pushes 12 to the tracker, while the local track row stores 12.5. A later `TrackChapter` with 12.5 passes the `number <= lastRead` skip test and does not repush.
- Start date: the first history entry by `readAt` ascending (the Kotlin sorts nullable `Date?` values with `sortedBy`, nulls first, so `firstOrNull()?.readAt` may be null even when later entries have dates; then no start date is set. Rust must keep this literal: `min_by(readAt)` with `None < Some`.
- Zone conversion: `LocalDateTime.ofInstant(Instant.ofEpochMilli(ms), systemZone).atZone(UTC).toInstant().toEpochMilli()`. In Rust with the `time` crate: `OffsetDateTime::from_unix_timestamp_nanos(ms*1e6).to_offset(local_offset)` then `.replace_offset(UtcOffset::UTC).unix_timestamp()*1000 + millis`. Use the offset in effect at THAT instant (DST aware), `time::UtcOffset::local_offset_at`.
- Order of remote calls: `bind` -> (maybe) `setRemoteLastChapterRead` -> (maybe) `setRemoteStartDate`; each of the setRemote* functions internally calls `updateRemote`, which catches exceptions, logs ERROR and shows a toast with `e.message` (`MangaTracker.kt` `updateRemote`), so a failure in one does not abort the rest.

## 6. Enhanced trackers

`EnhancedMangaTracker` / `EnhancedAnimeTracker` (area:33: Kavita, Komga, Suwayomi, Jellyfin) never ask the user to pick a series: `accept(source)` tests whether the source class is one of `getAcceptedSources()` (compares `source::class.qualifiedName`), `match(manga)` returns zero or one `MangaTrackSearch`. Flow `bindEnhancedTrackers(manga, source)` (p1-behavior.md 12.5) then `SyncChapterProgressWithTrack` merges progress both ways.

Proposal for Linux: extension classes do not exist as JVM classes, so `accept` becomes `accept(source_id, source_pkg)`; area:20 decides how a native source declares compatibility (for example a `tracker_hints: ["komga"]` field in the source manifest). Until then the enhanced trackers are only used with the built-in Komga/Kavita/Suwayomi-style sources if implemented as native sources.

## 7. Sync of progress (`Sync*ProgressWithTrack`), worked example

Local chapters (sorted by number): 1 read, 2 read, 3 unread, 4 unread, 5 read. Remote `lastChapterRead = 4`.
- `chapterUpdates` = unread chapters with `number <= 4` -> chapters 3 and 4 marked read.
- `localLastRead` = contiguous read prefix on the pre-update state = 2.
- `lastRead = max(4, 2) = 4`; `updatedTrack.lastChapterRead = 4`.
- Remote `tracker.update` is called FIRST (value 4); only if it succeeds the local chapter rows are updated and the track row stored. Chapter 5 stays local-only (remote is behind it, but this function never pushes a higher local number because `localLastRead` ignores the gap).
Second scenario: remote 0, local read 1..3: `chapterUpdates` empty, `lastRead = max(0, 3) = 3`, remote updated to 3.

## 8. RefreshTracks

`supervisorScope` + one `async` per logged-in track; return list of `(tracker, throwable)` failures. Proposal: `futures::future::join_all` over per-track futures returning `Result<(), (TrackerId, TrackError)>`; the UI shows a toast listing failed tracker names (area:38/39 `MangaTrackInfoDialog`). The `!!` on `toDomainTrack()` can only fail if the tracker returned a track without an id; in Rust the single `Track` struct always carries `id`, so this becomes a `TrackError::MissingId`.
