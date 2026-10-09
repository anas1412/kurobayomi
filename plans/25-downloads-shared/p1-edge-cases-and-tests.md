# 25-downloads-shared — edge cases, failure modes, tests

Edge-case ids (E-nn) are referenced from p1-behavior.md, p1-rust-design.md, p1-ui-parity.md and p1-tasks.md.

Source references are repo-relative to /home/blackbox/aniyomi-src/.

---

## 1. Edge cases — delete path

### E-01 Empty input
- Input: `awaitAll(anime)` with no episodes (empty vararg), or all candidates filtered out.
- Kotlin: `getEpisodesToDelete` returns empty → `deleteEpisodes` returns inside `launchIO` (AnimeDownloadManager.kt:271-273). No queue change, no FS change.
- Rust: `await_items` returns `Ok(0)` before any engine call (rust-design section 5 step 4).

### E-02 Source not resolvable (silent no-op)
- Input: `anime.source` id not present in `AnimeSourceManager` (extension uninstalled, not loaded).
- Kotlin: `sourceManager.get(anime.source)?.let { ... }` (DeleteEpisodeDownload.kt:15) → skipped. No deletion, no log, no error to the user.
- Consequence: downloaded files stay on disk after the user marks the episode seen. The UI still shows "seen".
- Rust (decision): keep the no-op (same user outcome) but log `tracing::warn!` with entry id and source id so the user can find leftovers, and return `Ok(0)`. Ceiling: leftovers remain until the source is reinstalled. Upgrade path: a "Clean orphaned downloads" maintenance action (P2, area:42 storage screen).

### E-03 Fire-and-forget deletion
- Kotlin: `awaitAll` returns before FS work completes (deleteEpisodes/deleteChapters are `launchIO`, AnimeDownloadManager.kt:270, MangaDownloadManager.kt:265).
- Consequence: the caller's `Result.Success` is not a guarantee the files are gone.
- Rust: `await_items` awaits the removal (rust-design section 5). The caller UI keeps running via `glib::spawn_future_local`. Deviation recorded here and in p1-behavior.md section 9.

### E-04 Entry has no categories
- `getCategories.await(id).map { it.id }.ifEmpty { listOf(0) }` (AnimeDownloadManager.kt:413-415, MangaDownloadManager.kt:415-417).
- Rust: `if entry_categories.is_empty() { &[0] }` in `select_for_delete`.
- Test: T-E04 in section 5.

### E-05 Non-numeric category id in preference set
- Kotlin: `map(String::toLong)` throws `NumberFormatException` on bad input; the exception propagates out of `getEpisodesToDelete` into the `launchIO` coroutine (uncaught → crash or lost delete, depending on the handler).
- Rust: `parse_category_ids` returns `DeleteError::BadCategoryId { key, value }`, the item deletion is aborted for that call and the error is logged; the other preference keys are unaffected. Proposal: the settings screen (area:47) should never write non-numeric ids; the migration (area:11) removes stale ids.

### E-06 Mark-all-read where some items were already read
- `SetSeenStatus.await` filters to `episodesToUpdate` (items actually changing) before deleting (SetSeenStatus.kt:30-35, 50-57).
- Items already seen before are not passed to the delete path, so their downloads survive a later "mark all read" (only when `remove_after_marked_as_read` is on).
- Test: T-E06 in section 5.
- Edge: an item marked seen while its `lastSecondSeen > 0` (partially watched, `seen=false`) IS included in the update (the filter is `it.seen || it.lastSecondSeen > 0` for the unread branch; for `seen=true` the filter is `!it.seen`). So partially watched items are updated and then deleted when seen becomes true. Test T-E06b.

### E-07 Reverse sort reverses equal keys
- `sortedBy(selector).toMutableList().apply { if (reverse) reverse() }` (AnimeDownloadQueueScreenModel.kt:170-173). Stable sort then reverse → equal dates (common: many episodes with the same `dateUpload`) come out in reverse of their queue order.
- Rust: stable `sort_by` then `reverse()` (queue.rs `sort_within_sections`). Test T-E07.
- Ceiling: this is the observable behaviour; keep it (it is what users see now).

### E-08 Sort is per section
- Sorting never moves an item across source sections (per-header `subItems` sort, AnimeDownloadQueueScreenModel.kt:168-176).
- Test T-E08: two sources A and B interleaved in the flat queue; sort by number asc; A's items sorted among themselves, B's among themselves, section order A then B (first appearance).

### E-09 Series move can reorder sections
- `move_series` builds a flat partition (AnimeDownloadQueueScreenModel.kt:83-94) and reorders the flat list. The next `queueState` emission regroups by source (`groupBy` first-appearance) so a series from source B moved to top can move B's section above A.
- Test T-E09: flat `[A1(series X), B1(series Y), A2(series X), B2(series Y)]` where A* come from source A and B* from source B. Move series X to top → flat `[A1, A2, B1, B2]`; sections `[A, B]`. Then move series Y to top → flat `[B1, B2, A1, A2]`; sections become `[B, A]`.
- Decision: keep (matches Kotlin). Proposal to revisit in P2.

### E-10 Move-to-top visibility quirk
- `bindingAdapterPosition > 1` is the flat adapter position including headers (AnimeDownloadHolder.kt:102). In the first section the first download sits at position 1 (header at 0), so "Move to top" is hidden there. In the second section the first download's position is larger than 1 and "Move to top" is shown although it is already at index 0 of its section.
- Rust: visibility by flat index (`index > 0`, `index < len-1`). Documented improvement.
- Test T-E10: index 0 in second section → "Move to top" hidden in Rust (shown in Kotlin).

### E-11 Cancel series with no matches
- `cancel_series` on an item whose series has no other items in `adapter.currentItems` → list empty → no call (AnimeDownloadQueueScreenModel.kt:99-105).
- Rust: `remove_series` returns empty Vec; `cancel(vec![])` is not sent (guard in tab.rs).

### E-12 `download_slots` has no reader
- `numberOfDownloads()` = `download_slots`, default 1 (DownloadPreferences.kt:107). The settings screen lets the user choose 1..5 (SettingsDownloadScreen.kt:102-106, description `download_slots_info`: "Will only download concurrently from self-hosted or unmetered sources"). No code in the tree reads the value (grep over all `*.kt` for `numberOfDownloads`/`download_slots` returns only these two places).
- Consequence: the setting has no effect in the Kotlin app as shipped in this tree. Possibly the engine reads it through another mechanism not in this tree; treat as unverified until area:23/24 confirm.
- Rust: store the key (T-25a-003). Engine decision (area:23): either honour it as the concurrency limit (default 1) or hide the setting. Proposal: honour it as the parallel download count, clamped to 1..5, because the UI promises it.

### E-13 Manga progress job never ends if pages never arrive
- `while (download.pages == null) delay(50)` (MangaDownloadQueueScreenModel.kt, `launchProgressJob`, line 215) polls forever until the job is cancelled (DOWNLOADED/ERROR or dispose).
- Failure mode: a DOWNLOADING download whose page list fails to load and never flips to ERROR keeps a coroutine polling every 50 ms.
- Rust: poll with `tokio::time::interval(50 ms)` and stop after 600 ticks (30 s) with a `tracing::warn!`; the row then shows progress 0. Proposal, recorded as a deviation.

### E-14 Tab badge shows 0
- `TabText(text, badgeCount)` shows a pill for any non-null count (presentation-core Tabs.kt:28); DownloadsTab passes non-null Ints (DownloadsTab.kt:250, 261). With no downloads the badges read "0".
- Rust: hide the badge at 0 (T-25a-021). Deviation recorded.

### E-15 Stale snapshot in reorder
- `reorderQueue` builds the new list from `adapter.headerItems` (the view's snapshot) and calls `downloader.updateQueue(list)` which REPLACES the queue (AnimeDownloader.kt:918-936; MangaDownloader.kt:747-767).
- Failure mode: an item that the engine started, finished or removed after the snapshot was taken is re-added (if it was in the snapshot) or dropped (if it was added after). An empty list calls `clearQueue()` + `stop()`.
- Rust: `QueueEngine::reorder(ids)` must treat the argument as an ordering: ids not in the live queue are ignored; live items not in the argument are appended in their existing order (no silent drop). Test T-E15. This is a deliberate deviation from `updateQueue` replace semantics; it is safer and matches what the user sees.
- Edge: `reorder(vec![])` in Rust must NOT clear the queue (it is a no-op). Clearing is only `clear()`.

### E-16 Cancel-all while running
- `clearQueue()` = `downloader.clearQueue(); downloader.stop()` (anime AnimeDownloadManager.kt:95-98, manga MangaDownloadManager.kt:98-101). The running download is removed; its status is set to NOT_DOWNLOADED by `removeFromQueueIf`/`internalClearQueue` (AnimeDownloader.kt:880-893).
- Test T-E16: 3 items, running, cancel all → queue empty, running false, no exception.

### E-17 Delete a queued or running item
- `removeFromDownloadQueue` pauses if running, removes the ids, then `stop()` if the queue is empty or `start()` otherwise (AnimeDownloadManager.kt:315-327). Items removed while DOWNLOADING or QUEUE are set to NOT_DOWNLOADED (AnimeDownloader.kt:870-878).
- Test T-E17: delete one running item out of 2 → the other item starts; deleted item NOT_DOWNLOADED; no partial directory for the deleted item is left behind (the directory is removed after queue removal).

### E-18 Excluded category protects watched/read items
- Entry in `remove_exclude_*` category: only unwatched items are deleted (AnimeDownloadManager.kt:416-420, MangaDownloadManager.kt:418-422).
- Test T-E18 (anime) and T-E18b (manga).

### E-19 Bookmarked items
- `pref_remove_bookmarked = false` (default): bookmarked items are kept (AnimeDownloadManager.kt:422-426, MangaDownloadManager.kt:424-428).
- `true`: bookmarked items are deleted.
- Test T-E19 both values.

### E-20 Empty-directory cascade
- After item removal: if the series dir `listFiles()` is empty → series dir deleted; if the source dir is then empty → deleted and `cache.removeSource`.
- `listFiles()` null (not a directory or IO error) → no cascade (`?.isEmpty() == true`).
- Test T-E20: series with one item only → series dir and source dir removed. Series with a second item → series dir kept.

### E-21 Item directory not found
- `findEpisodeDirs` uses `mapNotNull` over candidate names (AnimeDownloadProvider.kt:101-110). A missing item dir is skipped silently; the item still leaves the queue and the cache.
- Test T-E21: item with no directory → no error; cache entry removed; queue updated.

### E-22 Concurrent deletes on the same series
- Two `await_items` calls for the same series (e.g. "mark all read" with two rapid clicks) can race: one removes the series dir when empty while the other is still removing items.
- Kotlin: no lock (launchIO on IO dispatcher; cascade unguarded).
- Rust: per-series `tokio::sync::Mutex` keyed by `(kind, entry.id)` (HashMap of mutexes) around steps 5-9 of the algorithm. Ceiling: one map entry per series for the app lifetime; prune entries with no waiters.

### E-23 App quit mid-delete
- Kotlin: `launchIO` is cancelled with the process; directories may be half-removed.
- Consequence: a series can have some item dirs removed and others left; the cache may disagree.
- Rust: removal is per item directory and idempotent (missing dir = success). A later delete of the same items completes it. The cache is rebuilt from disk at startup by area:23/24 (their concern). Test T-E23: remove an item dir manually before delete → no error.

### E-24 Remove-after-read slots (consumer contract)
- `remove_after_read_slots = -1` disables (ReaderViewModel.kt:510). Otherwise the chapter at `index(current) - n` is enqueued for deletion (`getOrNull`, so out-of-range → null → nothing).
- `n = 0` → deletes the current chapter (after it is read).
- Test T-E24: list `[c0..c4]`, current index 4, n=2 → deletes c2; n=-1 → nothing; n=5 → index -1 → nothing.
- Owned by area:43/45 consumer; this area supplies the pref.

### E-25 Speed limit value 0
- `download_speed_limit` default 0 (DownloadPreferences.kt:108). Engine converts `value * 1024` bytes/s (MangaDownloader.kt:525). Zero semantics live in `bytesPerSecond` (area:23).
- Test (area:23) T-E25 is referenced, not owned here.

---

## 2. Edge cases — queue tab

### E-26 Count pill vs manga
- Title pill shows anime count only (DownloadsTab.kt:145-154). A non-empty manga queue with an empty anime queue shows no pill.
- Test T-E26: manga has 3 items, anime 0 → no title pill; manga tab badge "3".

### E-27 FAB visibility per page
- FAB is hidden when the current page has no items, even if the other page has items (DownloadsTab.kt:169-173).
- Test T-E27.

### E-28 FAB state on page switch
- The FAB label/icon read the running flag of the current page's kind (DownloadsTab.kt:177-225). Switching pages changes the label immediately.
- Test T-E28: anime running, manga stopped → anime page FAB "Stop", manga page "Resume".

### E-29 Pause semantics differ by kind
- Anime FAB pause = `stop()`; manga FAB pause = `pause(); stop()` (AnimeDownloadManager.kt:88-90 vs MangaDownloadManager.kt:90-93). The extra `pause()` call's effect is defined in the downloader (area:23), not in this scope.
- Rust: the engine exposes `stop()` and `pause_and_stop()`; the UI uses `pause_and_stop()` for manga and `stop()` for anime (T-25a-022). Test T-E29 (manga resume after pause continues from queue).

### E-30 Start when job not running
- `startDownloads()`: if `downloader.isRunning` return; if job running → `downloader.start()`; else → `Job.start(context)` (AnimeDownloadManager.kt:75-85).
- Test T-E30: start with no job → job started; start twice → second is a no-op.

### E-31 Progress for ERROR rows
- ERROR cancels the progress job (manga) and does nothing (anime). A row in ERROR never receives progress updates.
- Test T-E31.

### E-32 Progress flood
- Manga progress updates are debounced by 50 ms and `collectLatest`. Rust: coalesce into at most one repaint per 50 ms per row.
- Test T-E32: 200 progress events in 100 ms → at most 3 row repaints.

### E-33 Row lookup after rebuild
- Kotlin looks up holders by item id (`findViewHolderForItemId`), returns null if unbound (no-op). Rust: `HashMap<i64, RowHandle>` rebuilt per snapshot; missing → no-op.
- Test T-E33: progress for an item not in the snapshot → no panic.

### E-34 Drag into another section
- Adapter allows dropping into another section; `onItemReleased` flattens in on-screen order and sends the whole list (AnimeDownloadQueueScreenModel.kt:46-54). The item keeps the new position; its `source` is unchanged (the header is recalculated on the next snapshot).
- Test T-E34: drag A1 into section B → flat order reflects it; on next snapshot, A1 is shown in section A again (grouping by `source`). Expected: drag across sections is effectively a reorder within the flat queue only. Proposal: refuse cross-section drops with a visual cue (P2, T-25a-027 note).

### E-35 Sort menu when list empty
- The Sort action is not composed when the list is empty (DownloadsTab.kt:299, 383). No sort happens on empty.

### E-36 Duplicate screen-model instances
- DownloadsTab creates a model (lines 96-97) and each page creates another (AnimeDownloadQueueTab.kt:23). Two instances subscribe to `queueState`; the page's actions use the page-level model (DownloadsTab's FAB uses the tab-level model for `isDownloaderRunning`; the list uses the page-level one).
- Rust: one model per kind (T-25a-032).

### E-37 Source name collision
- Sections are keyed by source id, not name (groupBy source object). Two sources with the same display name appear as two sections with the same header text.
- Test T-E37: two sources, same name → two sections.

### E-38 Title / entry deleted while queued
- If the entry is deleted from the library while items are queued: `removeFromQueue(anime)` (AnimeDownloader.kt:900) removes all items of that anime. Owned by entry area; this area's reorder/cancel do not resurrect items because of E-15.

---

## 3. Race conditions summary

| Race | Where | Mitigation |
|---|---|---|
| Stale snapshot in reorder | E-15 | ids-as-ordering |
| Two deletes on one series | E-22 | per-series mutex |
| Progress event after row removal | E-33 | lookup miss = no-op |
| Engine state change during menu open | rebuild menu on open | menu uses current index at open |
| Start/stop double tap | `startDownloads` guard `isRunning` | same guard in Rust |

---

## 4. Failure modes

| Failure | Observable | Handling (Kotlin) | Handling (Rust) |
|---|---|---|---|
| FS delete fails (permission, busy file) | file stays; an exception from `it.delete()` propagates out of the `launchIO` block, so `cache.removeEpisodes` / `cache.removeChapters` is not reached | exception in launchIO | per-dir error logged; cache removal for successful dirs only (Proposal) |
| Source missing | leftovers | silent | warn log (E-02) |
| Bad category id | delete aborted | exception | error logged, delete aborted for that call (E-05) |
| DB update fails in SetSeenStatus | `Result.InternalError(e)`, no delete | logcat | same |
| Queue engine not running | start() spawns job | Job.start | same |

---

## 5. Test plan (all new tests; Kotlin upstream has no *Test.kt in this scope)

Upstream tests: none. Verified by `find . -name '*Test*.kt'` over the source tree: no test file references `DeleteEpisodeDownload`, `DeleteChapterDownload`, `DownloadsTab` or `DownloadPreferences`, and no download-related `*Test*.kt` exists.

Rust tests (unit, pure modules, no GTK):

| Test id | File | Assertion |
|---|---|---|
| T-E01 | policy.rs | empty candidates → empty selection |
| T-E04 | policy.rs | empty categories → treated as [0]; exclude set {0} protects watched |
| T-E06 | delete.rs (integration with fake engine) | only changed items deleted |
| T-E06b | delete.rs | partially watched (`lastSecondSeen>0`) included in update path |
| T-E07 | queue.rs | equal keys reversed for reverse=true; order preserved for reverse=false |
| T-E08 | queue.rs | sort per section, section order = first appearance |
| T-E09 | queue.rs | series move examples from E-09 |
| T-E10 | row.rs (logic fn `can_move_top(index)`) | index 0 → false; index 1 → true |
| T-E11 | queue.rs | remove_series with no match → empty |
| T-E15 | engine fake | reorder keeps live items not in argument; ignores unknown ids; reorder([]) no-op |
| T-E16 | engine fake | clear empties queue and sets running=false |
| T-E17 | engine fake | removal while running: pause, remove, resume if non-empty |
| T-E18 | policy.rs | excluded category + watched → not selected |
| T-E18b | policy.rs (manga read) | same |
| T-E19 | policy.rs | bookmarked kept when remove_bookmarked=false; deleted when true |
| T-E20 | store (tempdir) | single item → series and source dirs removed; two items → series kept |
| T-E21 | store (tempdir) | missing item dir → no error, other items removed |
| T-E22 | delete.rs | two concurrent calls, same series → no panic, no lost removal |
| T-E23 | store (tempdir) | missing item dir during delete → Ok |
| T-E24 | policy or consumer | slots n=0 → current; n=-1 → none; n=5 with index 4 → none |
| T-E26 | tab.rs (logic) | title pill shown iff anime count > 0 |
| T-E27 | tab.rs (logic) | FAB visible iff current page non-empty |
| T-E28 | tab.rs (logic) | FAB label per kind and running flag |
| T-E29 | engine fake | manga pause_and_stop vs anime stop |
| T-E30 | engine fake | start guard |
| T-E31 | row.rs (logic) | ERROR stops progress updates |
| T-E32 | row.rs | 200 events in 100 ms → at most 3 repaints (uses a fake clock) |
| T-E33 | tab.rs (logic) | missing row id → no panic |
| T-E34 | queue.rs | cross-section drop keeps flat order; sections regroup |
| T-E37 | queue.rs | two sources, same name → two sections |

Manual GUI tests (run against the mockup build, `cargo run`):
- M-01: open Download queue with empty queue → "No downloads", no FAB, no title pill, badges "0" (or hidden after T-25a-021).
- M-02: queue 3 anime + 2 manga → title pill "3", tab badges "3" and "2".
- M-03: move row 3 to top → row order updates; engine order matches (check with the engine debug dump).
- M-04: scroll the list up → FAB collapses; scroll down → expands.
- M-05: click Sort → By upload date → Newest; list reorders within sections only.
- M-06: Cancel all → queue empty; FAB and tabs update.
- M-07: mark an episode seen with "remove after marked as read" on → its download row disappears from the queue and its directory is removed (observe the directory).

Fixtures needed:
- F-01: a fake `QueueEngine` (in-memory Vec, running flag, command log).
- F-02: a temp directory tree: `<root>/<source>/<series>/<item dirs>`; helper to create item dirs with names from the provider rule.
- F-03: fake `CategoryLookup` (entry id → Vec<i64>).
- F-04: fake `SourceResolver` (set of installed source ids).
- F-05: queue snapshots covering 2 sources, 3 items each, with equal `date_upload` for two items.

Expected run commands (for the implementer): `cargo test -p kurobayomi download::` (single crate, module filter) and `cargo clippy --all-targets -- -D warnings`.
