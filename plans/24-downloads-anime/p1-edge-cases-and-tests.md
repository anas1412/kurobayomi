# 24-downloads-anime: edge cases, failure modes, races, tests

Notes on evidence:
- "Upstream tests": none exist. A repository-wide search for test files referencing `AnimeDownload`, `AnimeDownloader`, `AnimeDownloadCache`, `AnimeDownloadProvider`, `AnimeDownloadStore`, `AnimeDownloadPendingDeleter` or `AnimeDownloadManager` returned no files. The test plan in §5 is therefore new.
- Severity: H = data loss or user-visible wrong state; M = wrong but recoverable; L = cosmetic or dead code.
- Every "observed" item is a code-reading finding. Items marked **verify** need a runtime check before the port relies on them.

---

## 1. Edge cases by area

### 1.1 Queue and store

| ID | Case | Kotlin behaviour (source) | Expected Rust behaviour | Sev |
|---|---|---|---|---|
| E-Q01 | App restart while downloading | Store restored, all items set to QUEUE, nothing starts until user acts (`AnimeDownloader.kt:129-134`, `:142-153`) | Same. Also clean orphan `_tmp` dirs and partial `.tmp` files at startup (see E-F04). | M |
| E-Q02 | Restored entry whose source is no longer installed | Dropped silently and removed from store (`AnimeDownloader.kt:250`, store cleared at `AnimeDownloadStore.kt:257`) | Keep the row, show as QUEUE with a "source not installed" reason, do not start, retain for later. | H |
| E-Q03 | Restored entry whose episode was deleted from the DB | Dropped silently (`AnimeDownloadStore.kt:251`) | Same drop; this is correct. | L |
| E-Q04 | Queue reorder while DOWNLOADING | `updateQueue` pauses (partial remux deleted), reorders, restarts if it was running (`AnimeDownloader.kt:927-935`) | Same. Users lose progress of the active item. Document in UI tooltip. | M |
| E-Q05 | Same episode enqueued twice (two paths, or `alt` flag) | Filtered by `queue.none { episode.id == id }` (`:302`) | Same, plus dedupe in the store insert. | L |
| E-Q06 | `startDownloadNow` for an episode already on disk | Re-downloads into a new staging dir; final rename fails silently; cache reports downloaded (see E-F02) | Check `layout::find_episode_dir` first; if present, return "already downloaded" and do nothing. | H |
| E-Q07 | `queueEpisodes` with `autoStart = true` while queue non-empty | Job not started (`wasEmpty` gate, `:311`) | Same gate, but show a toast "Queued. Press Resume to start" when the runner is stopped. | M |
| E-Q08 | Queue size warning threshold | `> 20` total metered or `> 10` per metered source (`:320-323`) | Same thresholds; shown once per enqueue batch, not per item. | L |
| E-Q09 | Unmetered source queued items | Not counted toward warnings (`:313`, `:316`) | Same. | L |
| E-Q10 | Enqueue from non-HTTP source | Silently ignored (`:293`) | Same, but log at debug level. | L |
| E-Q11 | Empty queue `start()` | Returns false, nothing starts (`:143`) | Same. | L |
| E-Q12 | `addDownloadsToStartOfQueue` while runner is running | Inserts at head; no start call because the job is running (`AnimeDownloadManager.kt:398`) | Same; flow picks up the new head immediately. | L |
| E-Q13 | Store write fails (DB locked/full) | Not handled; `SharedPreferences.edit{}` is async apply (`AnimeDownloadStore.kt:189`) | Propagate: keep in-memory queue, retry write up to 3 times, then show a warning notification. | M |
| E-Q14 | `ord` collisions after restart | Counter restarts at 0 per process; restore reassigns (`:181`, `:257-258`) | Same semantics; avoid collision by inserting in one transaction. | L |
| E-Q15 | Mutation inside `StateFlow.update` retried | `addAllToQueue`, `removeFromQueue`, `removeFromQueueIf`, `internalClearQueue` have store side effects inside `update {}` (`AnimeDownloader.kt:861-916`) | Store writes outside any retried closure. | M |

### 1.2 Scheduler and status

| ID | Case | Kotlin behaviour | Expected | Sev |
|---|---|---|---|---|
| E-S01 | Errored item at head of its source group | Skipped (status > DOWNLOADING filtered out), next item in the group is scheduled (`:208-213`) | Same. | M |
| E-S02 | All items errored | Downloader stops with `onPaused` notification, not a completion message (`:226`, `:169-171`) | Show "Finished with N errors" (new text) and the error notification. Fix defect. | M |
| E-S03 | Unexpected exception in one download | `launchDownloadJob` catch-all calls `notifier.onError` and `stop()`, stopping all downloads (`:261-266`) | Stop only that download, mark ERROR, keep others running. Log at error level. Proposal: change from Kotlin, because one bad item should not stop the queue. | H |
| E-S04 | `DOWNLOADING` item at `stop()` | Becomes ERROR (`:160-162`) | Becomes QUEUE on pause (user intent), ERROR only on failure. Behaviour §2.5 defect. | M |
| E-S05 | Pause during hoster resolution | `pause` cancels job; resolution cancelled; status set back to QUEUE | Same. The `video` field (if resolved) is kept for the next attempt. | L |
| E-S06 | Network drops mid-download | Remux fails and retries; network-check flow does not re-start after reconnect (see E-N01) | Gate retries on network availability; if offline, pause with "No network connection available" and resume on reconnect when the runner was running (fix). | H |
| E-S07 | Scheduler recompute storm | `transformLatest`+`distinctUntilChanged` (`:205-227`) | Coalesce notifications; recompute at most once per 50 ms. | L |
| E-S08 | `DOWNLOADED` item that remains in queue because `removeFromQueue` failed | Not expected; store remove precedes update | Ensure removal is idempotent. | L |
| E-S09 | Two queue entries same anime different episodes, one source | Serial per source (`:213`) | Same. | L |

### 1.3 Staging, remux and finalise

| ID | Case | Kotlin behaviour | Expected | Sev |
|---|---|---|---|---|
| E-F01 | Pause mid-remux | Partial `.tmp` deleted (`AnimeDownloader.kt:514`) | Same. Progress is lost. | M |
| E-F02 | Target episode dir already exists at finalise | `tmpDir.renameTo(episodeDirname)` return value ignored (`:384`). Cache says downloaded; staging dir with the real file remains as `<name>_tmp`; episode shows as downloaded but is unreadable from its name. | Detect rename failure; if target exists and is non-empty, move the new file into the existing dir under a unique name, or mark ERROR "Episode already exists". Never silently succeed. | H |
| E-F03 | Duplicate episode names in one anime (different URLs) | Same as E-F02 | Same fix. Also make `episodeDirName` unique per episode id when a collision is detected (proposal: append ` (id)` on collision). | H |
| E-F04 | Crash after ffmpeg writes `.mkv`, before finalise | On restart the staging dir contains `<name>.mkv`; resume path picks it up (`:429`) | Same. Good path. | L |
| E-F05 | Crash mid-remux | `.tmp` left in `_tmp` dir; restart re-downloads: `.tmp` deleted at start of `downloadVideo` (`:505`) | Same, plus startup sweep of `_tmp` dirs that are not in the queue (fixes orphans). | M |
| E-F06 | `isDownloadSuccessful` with leftover `.tmp` plus `.mkv` | Counts 2 files, fails (`:817-818`) → ERROR without notification | Count only non-`.tmp` files; leftover `.tmp` removed first (defect fix, behaviour §5.8). | H |
| E-F07 | ffmpeg exits 0 but produces empty output | `isValueSuccess` true; file renamed; finalised as DOWNLOADED with 0-byte `.mkv` | Check output size > 0 and ffprobe duration > 0 before finalise; else error "Error in ffmpeg!". | H |
| E-F08 | ffmpeg not on PATH | `FFmpegKit` always available on Android | Startup check; downloads fail with `FfmpegMissing` and a Settings link. | H |
| E-F09 | `ffprobe` fails (HTTP 403, network) | `getDuration` throws `Exception(it.output)` → retry (`:696`) | Same retry; message is ffprobe stderr trimmed to first line. | M |
| E-F10 | Duration unknown (`N/A`) | `toFloatOrNull` → 0 → no progress (`:610`, `:605`) | Same; bar stays indeterminate. | L |
| E-F11 | Live stream / infinite duration | Duration 0 → indeterminate, runs until end | Same. | L |
| E-F12 | Header value contains a single quote | `-headers 'K: v'` breaks (`:579-581`) | Not an issue with argv. | M |
| E-F13 | Header value contains CRLF | Injected header | Strip CR and LF from header values before building `-headers`. | M |
| E-F14 | Output path contains `"` | Escaped as `\"` (`FFmpegUtils.kt:18`) | Not an issue with argv. | L |
| E-F15 | Finished-file leftover `.nomedia` absent | Possibly never created (see behaviour §5.9) | Create in final dir; verify. | L |
| E-F16 | Rename of staging dir across filesystems | Not possible in SAF semantics | Same-filesystem rename only; error if `EXDEV`. | M |
| E-F17 | Final name > 255 bytes | `take(240)` counts UTF-16 units, not bytes (`DiskUtil.kt:124`); `_tmp.mkv` suffix adds 8 | Truncate to 200 bytes on a char boundary for the file component. | M |
| E-F18 | Source directory name collision after rename (case-insensitive FS) | Two-step rename via `_tmp` (`AnimeDownloadManager.kt:602-609`) | Same. | L |
| E-F19 | Downloads root unset (`null`) | `downloadsDir!!` NPE caught → `invalid_location` exception (`AnimeDownloadProvider.kt:333-343`) | Return `DlError::InvalidLocation(path or "")` before queueing; block enqueue with a Settings link. | H |
| E-F20 | Free space check on SAF or unreadable path | `-1` → check skipped (`DiskUtil.kt:75-82`) | `statvfs` error → skip with a debug log (same). | L |
| E-F21 | Free space exactly 200 MiB | `availSpace < MIN` strict (`:347`) → allowed | Same comparison. | L |
| E-F22 | Anime title changed while download is queued | Download uses `anime.title` captured at enqueue (`AnimeDownload.anime`) | Same; finalise into the old anime dir name. Proposal: re-resolve the anime dir at finalise and warn if it moved. | M |
| E-F23 | Source renamed while download is running | `animeDir` captured at start; `renameSource` renames the parent | The final rename fails; treat as ERROR with "Download folder moved". | M |
| E-F24 | Episode renamed on source while downloading | Finalises under old name; `SyncEpisodesWithSource` later renames the dir | Same. | L |
| E-F25 | External placeholder (`_tmp.mkv`) left behind | Deleted before finalise (`:383`) | Same, but see E-X01 for the bigger external defect. | M |

### 1.4 External downloader and torrent

| ID | Case | Kotlin behaviour | Expected | Sev |
|---|---|---|---|---|
| E-X01 | External downloader handoff marks episode DOWNLOADED | Placeholder created, intent started, returns, finaliser treats the empty dir as downloaded (`:711-805`, `:829-851` not used; `:813-819`) | Do not finalise on handoff. Feature dropped on Linux by default (T-24a-024). | H |
| E-X02 | ADM handoff deletes staging dir, then `isDownloadSuccessful` fails | `tmpDir.delete()` then listFiles null → ERROR silently (`:777-778`, `:813`) | Not applicable after the drop. If kept, mark DOWNLOADED only via the external tool's callback. | H |
| E-X03 | Generic external (no package) | `ACTION_VIEW video/*` opens a random player | Drop. | M |
| E-X04 | Torrent URL with torrent server disabled | `isTorrent` only checked when enabled; magnet goes to ffmpeg, which fails (`:508`) | Reject magnet URLs when torrent support is off, with message "Torrent server disabled". | M |
| E-X05 | Torrent `index=` non-integer | Throws in step 2 (`:548`) → retry | Return `Message` error, not retry (not transient). | L |
| E-X06 | Local HTTP server start timeout | `startHttpServerService` 5 s timeout → `Failed to start server` (`MainActivity.kt:647`, `AnimeDownloader.kt:461`) | Same timeout; error text kept. | L |
| E-X07 | Local HTTP server port 0 | `listeningPort ?: 0` (`:454`) | Treat port 0 as failure. | M |

### 1.5 Network and job lifecycle

| ID | Case | Kotlin behaviour | Expected | Sev |
|---|---|---|---|---|
| E-N01 | Network returns after Wi-Fi-only stop | No auto-resume (job loop unreachable, see behaviour §4.2) | Resume automatically only if the user had the runner started and the stop reason was network. | H |
| E-N02 | Wired connection with Wi-Fi-only on | Treated as no Wi-Fi (`AnimeDownloadJob.kt:89`) | Allow wired (Linux mapping, T-24a-031). | H |
| E-N03 | Metered connection (Linux `Metered` property) | Not considered by Android code | Optional: respect NM `Metered` = yes as Wi-Fi-only. Default off. | L |
| E-N04 | Network state flapping | Each change emits check; `networkCheck` var updated (`:73-75`) | Debounce 2 s before stopping; avoid thrash. | M |
| E-N05 | Busy loop in job | `while (active)` without suspension (`:80-82`) unreachable in practice, but if reached spins | Do not port. Use `tokio::select!` on cancellation. | H |
| E-N06 | Window closed while downloading | Android: WorkManager keeps running | Keep the process alive via a hidden window or close confirmation (T-24a-030). | H |
| E-N07 | Suspend / resume of the machine | Android Doze not applicable | On resume, network state re-checked; running remux may be dead: detect child exit and retry. | M |

### 1.6 Pending deletion

| ID | Case | Kotlin behaviour | Expected | Sev |
|---|---|---|---|---|
| E-P01 | Pending delete for uninstalled source | Entry cleared, not deleted (`AnimeDownloadManager.kt:585` `?: continue` after `getPendingEpisodes` cleared prefs) | Keep rows until the source is installed or the user clears them. | H |
| E-P02 | Duplicate episode in one batch | `episodes.map { toEntry() }` no dedupe for new anime (`AnimeDownloadPendingDeleter.kt:202`) | Dedupe. | M |
| E-P03 | Batch added while another batch for the same anime is being flushed | `@Synchronized` on both methods (`:174`, `:220`) | Same; serialise with the DB transaction. | L |
| E-P04 | Entry JSON corrupt | Skipped silently (`:237-244`) and then cleared with the rest | Move corrupt value to a `corrupt` table and log. | M |
| E-P05 | Deleted episode file already missing | `findEpisodeDirs` returns only existing; no-op | Same. | L |
| E-P06 | User re-watches an episode after pending delete, before flush | Pending delete still removes the file | Same (the feature's intent). Document. | L |
| E-P07 | Episode bookmarked after being queued for pending delete | Bookmark check done at enqueue time only | Re-check bookmark and category rules at flush time too. | M |

### 1.7 Deletion and cascade

| ID | Case | Kotlin behaviour | Expected | Sev |
|---|---|---|---|---|
| E-D01 | Delete episode currently DOWNLOADING | `removeFromDownloadQueue` pauses and restarts (`:552-567`); running coroutine is cancelled by pause; partial `.tmp` deleted | Same. | M |
| E-D02 | Delete anime while its items are DOWNLOADING | `deleteAnime` → `removeFromQueue(anime)` without pause (`:900`); running item keeps going and finalises into deleted dir (fails) | Pause first, then remove, then resume. | H |
| E-D03 | Delete last episode in anime | `deleteEpisodes` removes anime dir when empty (`:523-525`) | Same. | L |
| E-D04 | Delete last anime in source | Source dir removed if empty and cache entry removed (`:544-548`) | Same. | L |
| E-D05 | Delete with filter excluding everything | `getEpisodesToDelete` empty → return (`:509-511`) | Same; return a count of protected items to the UI (proposal). | M |
| E-D06 | Delete while cache renewal in progress | Mutex on cache (`AnimeDownloadCache.kt:259`) | Same. | L |
| E-D07 | Delete during listing (cache iteration) | No mutex on reads (behaviour §8.3) | Clone the snapshot before iterating. | M |
| E-D08 | Source uninstalled and dir remains | Cache ignores folders with no installed source (`:359`) | Same; show in Storage as "Unknown source" (proposal, owned by area:42-ui-updates-history-stats-storage-upcoming). | M |

### 1.8 Cache

| ID | Case | Kotlin behaviour | Expected | Sev |
|---|---|---|---|---|
| E-C01 | First query before any snapshot | `renewCache` starts; returns false until done (`:147`, `:149-159`) | Same, plus "Indexing…" indicator; UI uses `isInitializing` (1 s debounce). | M |
| E-C02 | Snapshot corrupt | Deleted and rebuilt (`:115-117`) | Same. | L |
| E-C03 | Extensions never initialise | 30 s timeout, then uses whatever sources exist (`:342-347`) | Same; after timeout, any folder with no source is ignored (E-D08). | M |
| E-C04 | Folder appears while app is running | Not seen until renewal (1 h) or storage change (`:122-124`) | Same; add a manual "Refresh downloads" action (proposal). | L |
| E-C05 | Cache says downloaded but file deleted externally | Shown as downloaded until renewal | Same. | M |
| E-C06 | Renewal racing with finalise | Finalise calls `addEpisode` under the mutex (`:227`) while renewal swaps root under mutex | Same. | L |
| E-C07 | `_tmp` folders visible in cache | Excluded (`:374`) | Same. | L |
| E-C08 | `.mp4` files counted but never produced | Counted (`:378`) | Same for compatibility. | L |

### 1.9 Notifications and UI

| ID | Case | Kotlin behaviour | Expected | Sev |
|---|---|---|---|---|
| E-U01 | Notification title prefix strip with truncated title | `Pattern.quote(title)` uses chopped title incl. `…` (`AnimeDownloadNotifier.kt:106-109`) | Strip with the untruncated title, then truncate. Defect fix. | L |
| E-U02 | All errors share notification ID -204 | Later error replaces earlier (`:231`) | One error summary notification with the count, or one per download with distinct IDs. | M |
| E-U03 | Warning builder state leaks into error notifications | Shared builder (`:41-44`, `:192`) | Build a new notification per call. | M |
| E-U04 | Progress notification spam | Every 50 ms (`AnimeDownloader.kt:444-446`) | ≤1 Hz. | M |
| E-U05 | Row text blank while video unresolved | `video == null` → text "" (`AnimeDownloadHolder.kt:176-179`) | Show "Looking for video…" during resolution. | L |
| E-U06 | Row shows stale percent after ERROR | Text and bar not reset (§UI parity §1.5) | Reset to "Failed" with glyph. | L |
| E-U07 | "Move to top" shown on first item of second group | `position > 1` (`AnimeDownloadHolder.kt:230`) | Hide when first in own group. | L |
| E-U08 | Series move across groups | Intentional (`AnimeDownloadQueueScreenModel.kt:83-94`) | Same. | L |
| E-U09 | Notification text localised but `R.string.download_notifier_no_network` used | Job uses R strings | Use the same key via i18n. | L |
| E-U10 | Queue count pill counts ERROR items | Yes (`AnimeDownloadQueueTab.kt:361`) | Same; document. | L |

### 1.10 Preferences

| ID | Case | Kotlin behaviour | Expected | Sev |
|---|---|---|---|---|
| E-PR01 | `download_slots` changed by user | No effect on anime | Decide per T-24a-055. Default: ignore for anime and hide the control for anime. | M |
| E-PR02 | `download_speed_limit` changed by user | No effect on anime (ffmpeg has no limiter here) | Implement or hide (T-24a-055). | M |
| E-PR03 | Preference key typo risk | Keys are string literals | Constant table in Rust; test that the key set matches `p1-data-and-config.md` §3. | L |
| E-PR04 | `remove_exclude_anime_categories` contains a stale category id | `map(String::toLong)` throws on non-numeric (`:648`) | Parse with error skip. | L |

---

## 2. Concurrency hazards (checklist for the port)

1. Mutating the queue from two tasks. Use a single owner (scheduler). Enforced by the command channel (rust design §3).
2. Holding a DB lock across an `.await` on ffmpeg. Never hold the store mutex while waiting for the child.
3. ffmpeg child outliving its download task. Use `kill_on_drop` and explicit `wait()` on cancel (rust design §14).
4. Rename of a staging dir while ffmpeg writes into it. Await child exit first (rust design §14).
5. Cache renewal and finalise both writing to the in-memory index. Serialise with one `Mutex` or put the index behind one owner task.
6. Progress publication flood. Coalesce at 10 Hz per download.
7. Notification thread sends. `notify-rust` `show()` blocks on D-Bus; call from a blocking thread.
8. The GTK thread must never await the runtime. All UI updates go through `glib::spawn_future_local` consuming `watch` receivers.
9. Pause/cancel races: cancellation token set, then state set to QUEUE. The download task must check the token before writing state DOWNLOADED.
10. Restart and pause sequencing: on `Pause`, wait for all running tasks to observe cancellation before persisting the queue.

---

## 3. Failure-mode matrix (what the user sees)

| Failure | Retries | Queue result | Notification | Staging result |
|---|---|---|---|---|
| Hoster list empty | none | ERROR | "No video found" (ID -204) | staging dir left empty; removed on next start (E-F05 sweep) |
| Best video not resolved | none | ERROR | "No video found" | as above |
| Low disk space at start | none | ERROR | "Couldn't download due to low storage space" | not created |
| ffprobe failure | 3 retries (2/4/8 s) | ERROR after 4 attempts | ffprobe error text | partial deleted each attempt |
| ffmpeg non-zero exit | 3 retries | ERROR | "Error in ffmpeg!" | partial deleted |
| ffmpeg exit 0, empty output | proposal: none | ERROR | "Error in ffmpeg!" (new check) | partial deleted |
| Cancellation (pause) | none | QUEUE | none (pause notification only) | partial deleted |
| Unexpected exception (outside episode scope) | none | runner stops (Kotlin) / only item ERROR (proposal) | "Downloader" error with message | depends |
| Network lost (Wi-Fi-only) | none | runner stops with warning | "No Wi-Fi connection available" | partial deleted when remux was running |
| Network lost (no network) | retries of remux | ERROR after 4 attempts or pause (proposal) | "No network connection available" | partial deleted |
| Finalise rename fails (target exists) | none | ERROR (proposal; Kotlin: silent DOWNLOADED) | "Episode already exists" (new) | staging kept |

---

## 4. Test plan (new tests, all in `src/downloads/` unless stated)

### 4.1 Unit tests (pure functions)

| Test | Function | Cases |
|---|---|---|
| `fat_safe_*` | `layout::fat_safe` | empty → `(invalid)`; leading/trailing dots and spaces trimmed; each of `"*/:<>?\|` → `_`; control chars 0x00, 0x1F, 0x7F → `_`; 241-char name → 240 chars; multi-byte name > 255 bytes → byte-limited (E-F17) |
| `episode_dir_name_*` | `layout::episode_dir_name` | blank name → `Episode`; scanlator blank → name only; scanlator set → `scanlator_name`; invalid chars in scanlator |
| `legacy_name_*` | `layout::legacy_episode_dir_name` | `scanlator != null` even if blank → `"${scanlator}_$name"`; no sanitise |
| `valid_names_order` | `layout::valid_episode_dir_names` | new name first, legacy second |
| `name_changed_*` | `layout::is_episode_dir_name_changed` | name change; scanlator `null` vs blank equal; scanlator change |
| `active_set_*` | `scheduler::active_set` | empty; all DOWNLOADED; one source many items; 4 sources → 3 chosen in first-occurrence order; ERROR head skipped; mixed |
| `remux_args_*` | `ffmpeg::build_remux_args` | golden order (rust design §7) for: video only; +subs; +audio; +both; header on/off per URL scheme; stream args; video args; CRLF stripped (E-F13) |
| `ffprobe_args_*` | `ffmpeg::build_ffprobe_args` | argument list matches Kotlin with header prefix |
| `progress_*` | `ffmpeg::parse_progress_line` | `out_time_us=…`; `progress=end`; garbage lines ignored |
| `percent_*` | progress computation | duration 0 → 0; out 0 → 0; out > duration → clamp 100; integer truncation matches `100 * out / dur` |
| `retry_schedule` | `retry::RETRY_DELAYS_MS` | `[2000, 4000, 8000]`; attempts = 4 (test with `tokio::time::pause`) |
| `retry_cancel` | `with_retry` | cancellation during sleep returns `Cancelled` immediately |
| `chop_*` | notifier title helper | 15 and 30 limits; `…` length; exact-length input unchanged |
| `notify_title_strip` | notifier title helper | episode prefix stripped using untruncated title (E-U01 fixed) |
| `is_download_successful_*` | engine helper | 0 files false; 1 `.mkv` true; `.mkv` + leftover `.tmp` → true (after removing `.tmp` first) (E-F06 fixed); 2 non-tmp false |
| `torrent_convert_*` | torrent helper | `link=`/`index=` → magnet; missing index → error; index parse failure (E-X05) |
| `pending_dedupe_*` | `pending::add_episodes` | batch duplicates; existing duplicates; new anime |
| `cache_snapshot_*` | `cache::CacheSnapshot` | round-trip with bincode; corrupt bytes → rebuild; `_tmp` names excluded |
| `dlstate_active` | `DlState::is_active` | 0,1,2 true; 3,4 false |

### 4.2 Integration tests (temp dirs, fake ffmpeg)

| Test | Setup | Assertion |
|---|---|---|
| `download_happy_path` | fake `ffmpeg` shell script that writes the output file and prints `out_time_us` lines and `progress=end`; fake hoster resolver returns a video | final dir `<name>/<ep>.mkv` + `.nomedia`; state DOWNLOADED; removed from store; cache includes episode |
| `download_retries_then_error` | fake ffmpeg exits 1 every time | 4 invocations; ERROR; staging `.tmp` absent; notification "Error in ffmpeg!" |
| `download_recovers_on_retry` | ffmpeg fails once then succeeds | DOWNLOADED; total delay ≥ 2 s (use paused time) |
| `pause_discards_partial` | ffmpeg sleeps; pause after first progress | state QUEUE; `.tmp` absent; resume restarts from 0 |
| `resume_finished_remux` | staging dir pre-populated with `<ep>.mkv` | ffmpeg not invoked; finalised |
| `finalise_existing_target` | final dir already exists (E-F02) | state ERROR with "Episode already exists" (after fix); staging preserved |
| `scheduler_three_sources` | 5 sources, 2 items each, fake slow ffmpeg | exactly 3 concurrent children at any time; per-source serial |
| `reorder_while_running` | running item A, reorder | A back to QUEUE, partial removed, restarts in new order |
| `store_restore_roundtrip` | enqueue 3, drop manager, recreate | same order; entries with missing episode dropped; entries with missing source kept (E-Q02 fixed) |
| `pending_flush_missing_source` | pending for source X not installed | rows kept; files kept (E-P01 fixed) |
| `delete_cascade` | episodes in one anime, delete all | anime dir removed; if source empty, source dir removed |
| `delete_while_downloading` | delete the running episode | pause, remove, resume others; no `.tmp` remains |
| `rename_episode` | rename dir and cache | new name in cache, old removed |
| `rename_source_case_only` | `Example (EN)` → `example (en)` on a case-insensitive temp FS (if available) | two-step rename; directory ends with new name |
| `network_gate_wifi_only` | fake NM state: wired / wifi / none | wired allowed (E-N02 fixed); none → "No network connection available" |
| `cache_renewal_snapshot` | create tree, snapshot, add folder | renewal after 1 h (paused time) or invalidation |

### 4.3 GTK tests (manual or `gtk::test` with display)

| Test | Assertion |
|---|---|
| queue page with 0 items | shows "No downloads" |
| 2 sources, 5 items | 2 headers "Name (n)", 5 rows, counts correct |
| row at progress 0 | pulsing bar, text "Downloading…" |
| row at progress 45 | fraction 0.45, text "45%" |
| drag across groups | drop rejected |
| menu visibility | "Move to top" hidden for first overall item; "Move to bottom" hidden for last overall (current Kotlin rule, to be corrected per E-U07) |

---

## 5. Fixtures

- `fixtures/dl/tree_new/` — `Example (EN)/Show/Scan_Episode 1/Episode 1.mkv` and `.nomedia`.
- `fixtures/dl/tree_legacy/` — `Example (EN)/Show/Scan_Episode 2/` (legacy name = name with scanlator prefix, no sanitising).
- `fixtures/dl/tree_tmp/` — `Example (EN)/Show/Episode 3_tmp/Episode 3.tmp`, `Episode 4_tmp/Episode 4.mkv`.
- `fixtures/dl/tree_case/` — `EXAMPLE (en)` and `example (en)` for rename tests.
- `fixtures/ffmpeg/fake_ffmpeg.sh` — writes argv to a log file, emits `out_time_us=` lines, writes a non-empty output path, exits with code from env `FAKE_FFMPEG_EXIT`.
- `fixtures/ffmpeg/fake_ffprobe.sh` — prints duration from env `FAKE_FFPROBE_DURATION` (or empty for N/A).
- `fixtures/queue/store_v1.sqlite` — sample `anime_download_queue` rows (3 items, 2 anime, 2 sources).
- `fixtures/pending/entries.json` — the JSON shape from `p1-data-and-config.md` §1.6.
- `fixtures/cache/snapshot_v3.bin` — bincode snapshot of the tree above.
- `fixtures/video/with_tracks.json` — a `Video` with 2 subtitle tracks and 2 audio tracks, one `http` and one local.
- `fixtures/headers/crlf.json` — header values containing `\r\n` to test E-F13.

---

## 6. Non-functional checks

- Memory: queue of 500 items must stay under 20 MB resident (no per-item file handles kept open).
- Latency: starting a download after Resume must reach `Downloading` within 500 ms when the hoster is already resolved.
- Disk: staging dir must never exceed the final size by more than the remux overhead (≤ 5 %).
- Logs: no secrets. Headers (which can contain cookies) must be redacted in logs (`Cookie`, `Authorization`, `Referer` values replaced by `***`). Kotlin currently logs ffmpeg messages verbatim at error level (`AnimeDownloader.kt:597`), which can include header-bearing URLs; the port must strip query strings and headers from log lines.
- Power: idle runner uses no CPU (no busy loop, E-N05).
