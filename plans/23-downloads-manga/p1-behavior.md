# Area 23 — Manga downloads: behaviour specification

Format: **OBSERVED** = what the Kotlin does (cited path:line). **Proposal:** = what Rust should do. Paths are repo-relative to `/home/blackbox/aniyomi-src`. Line numbers refer to the files as read for this area.

Abbreviations: `Dl` = `MangaDownloader` (app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt), `Mgr` = `MangaDownloadManager` (MangaDownloadManager.kt), `Cache` = `MangaDownloadCache` (MangaDownloadCache.kt), `Prov` = `MangaDownloadProvider` (MangaDownloadProvider.kt), `Store` = `MangaDownloadStore` (MangaDownloadStore.kt), `Del` = `MangaDownloadPendingDeleter` (MangaDownloadPendingDeleter.kt), `Job` = `MangaDownloadJob` (MangaDownloadJob.kt), `Ntf` = `MangaDownloadNotifier` (MangaDownloadNotifier.kt), `QVM` = `MangaDownloadQueueScreenModel` (ui/download/manga/MangaDownloadQueueScreenModel.kt).

---

## 1. Core state: `MangaDownload.State`

File: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt

| Name | Value | Meaning |
|---|---|---|
| NOT_DOWNLOADED | 0 | Not in the queue (transient after removal) |
| QUEUE | 1 | Waiting in the queue, or paused |
| DOWNLOADING | 2 | Page work in progress |
| DOWNLOADED | 3 | Finished; removed from the queue right after |
| ERROR | 4 | Failed; stays in the queue, skipped for the rest of the run |

"Active" for scheduling means `status.value <= DOWNLOADING.value`, that is states 0, 1 and 2 (Dl.kt:216, Dl.kt:686).

### 1.1 Transitions (complete list, from every writer)

| Writer (file:line) | From | To | Condition |
|---|---|---|---|
| Dl.addAllToQueue (Dl.kt:689-697) | any | QUEUE | every added download, unconditionally |
| Dl.start (Dl.kt:149-150) | NOT_DOWNLOADED, QUEUE, DOWNLOADING, ERROR | QUEUE | every queue item whose status is not DOWNLOADED and not already QUEUE |
| Dl.downloadChapter (Dl.kt:339) | QUEUE | ERROR | free space known and below MIN_DISK_SPACE; returns normally, no exception |
| Dl.downloadChapter (Dl.kt:385) | QUEUE | DOWNLOADING | after the tmp dir exists and page list is known |
| Dl.downloadChapter (Dl.kt:413) | DOWNLOADING | ERROR | `isDownloadSuccessful` is false; returns normally |
| Dl.downloadChapter (Dl.kt:434) | DOWNLOADING | DOWNLOADED | after finalisation |
| Dl.downloadChapter (Dl.kt:439) | any | ERROR | any non-cancellation exception in the try block |
| Dl.stop (Dl.kt:166) | DOWNLOADING | ERROR | the downloader is stopped (without a reason string) while a chapter is downloading |
| Dl.pause (Dl.kt:191) | DOWNLOADING | QUEUE | pause |
| Dl.removeFromQueue(download) and removeFromQueueIf (Dl.kt:699-722) | DOWNLOADING, QUEUE | NOT_DOWNLOADED | removed from the queue |
| Dl.internalClearQueue (Dl.kt:737-741) | DOWNLOADING, QUEUE | NOT_DOWNLOADED | queue cleared |
| Dl.launchDownloadJob (Dl.kt:258-260) | DOWNLOADED | (removed) | after a successful download, `removeFromQueue` is called |

Important side effects of the table above:

- **OBSERVED:** `updateQueue` (Dl.kt:747-763) calls `addAllToQueue`, which sets every queued download to QUEUE. Any reorder therefore converts ERROR items back to QUEUE, and if the downloader was running they are retried in the same run. Proposal: keep ERROR on reorder; only the explicit "retry" path resets it.
- **OBSERVED:** `stop()` converts DOWNLOADING to ERROR (Dl.kt:164-166) and `pause()` converts DOWNLOADING to QUEUE (Dl.kt:189-191). `pauseDownloads` in Mgr calls `pause()` before `stop()` (Mgr.kt:90-93), so paused items end in QUEUE.

---

## 2. Startup and restore

### 2.1 Downloader construction (Dl.kt:131-136)

```
init {
    launchNow {
        val chapters = async { store.restore() }
        addAllToQueue(chapters.await())
    }
}
```

- `launchNow` is the app's coroutine helper from `core/common/src/main/java/tachiyomi/core/common/util/lang/CoroutinesExtensions.kt` (imported at Dl.kt:52). `store.restore()` is a blocking call inside `async`.
- Restored items go through `addAllToQueue`, so they start as QUEUE and get new `order` values through `Store.serialize` (Store.kt, `counter++`).
- **OBSERVED race:** the restore is not gated on sources being initialised. `Store.restore()` (Store.kt, `restore`) calls `sourceManager.get(manga.source) as? HttpSource ?: continue` and then `runBlocking { getChapter.await(chapterId) } ?: continue`. A download whose extension is still loading at startup is **silently dropped**, and `clear()` runs before the re-add, so the row is deleted from the store as well. Proposal: wait for the source gate (`kuro-core::sources_gate::SourceGate`, area:17) before restoring, and keep unresolved rows instead of deleting them.
- **OBSERVED:** `restore()` does not de-duplicate. Two rows with the same chapter id would both be added. The UI tolerates this only because `MangaDownloadItem.equals` compares chapter id. Proposal: primary key on `chapter_id`.

### 2.2 Store.restore (Store.kt)

1. Read all SharedPreferences values that are strings (`preferences.all`), decode each as `DownloadObject(mangaId: Long, chapterId: Long, order: Int)`, skip undecodable values silently.
2. Sort by `order` ascending.
3. Cache manga lookups per `mangaId` (`cachedManga`). For each row: manga must exist (`getManga.await`), source must be an `HttpSource`, chapter must exist. Otherwise skip.
4. `clear()` the store, then return the list. The caller re-adds them, which writes new rows with `order` reset from 0 in restored order.

### 2.3 Disk index on startup (Cache.kt, `init`)

1. Launch on IO: under `rootDownloadsDirMutex`, if `cacheDir/dl_index_cache_v3` exists, decode it (ProtoBuf) into the root directory and set `lastRenew = now`. On failure, log and delete the file.
2. Subscribe to `storageManager.changes` and call `invalidateCache()` on each change.

So after a successful load, the index is trusted for one hour without any filesystem scan.

---

## 3. Queue operations (Mgr and Dl)

### 3.1 Manager facade operations

| Function (Mgr.kt) | Lines | Exact behaviour |
|---|---|---|
| `startDownloads()` | 77-85 | If `downloader.isRunning` return. If `Job.isRunning(context)` (exactly one WorkManager work item RUNNING) then `downloader.start()`, else `Job.start(context)`. |
| `pauseDownloads()` | 90-93 | `downloader.pause()` then `downloader.stop()` (no reason). |
| `clearQueue()` | 98-101 | `downloader.clearQueue()` then `downloader.stop()`. |
| `getQueuedDownloadOrNull(chapterId)` | 109-111 | `queueState.value.find { it.chapter.id == chapterId }`. |
| `startDownloadNow(chapterId)` | 113-123 | existing = queued item, or `MangaDownload.fromChapterId` (runBlocking). If none, return. Remove existing, insert at index 0, `reorderQueue`, then `startDownloads()`. |
| `reorderQueue(downloads)` | 130-132 | `downloader.updateQueue(downloads)`. |
| `downloadChapters(manga, chapters, autoStart=true)` | 141-143 | `downloader.queueChapters`. |
| `addDownloadsToStartOfQueue(downloads)` | 150-157 | Return if empty. Prepend the list, `reorderQueue`, then `if (!Job.isRunning(context)) startDownloads()`. |
| `buildPageList(source, manga, chapter)` | 167-184 | See section 9. |
| `isChapterDownloaded(...)` | 195-209 | Delegates to `Cache.isChapterDownloaded`. |
| `getDownloadCount()` / `getDownloadCount(manga)` | 214-230 | Total from Cache. For `LocalMangaSource.ID`, count of entries in the local manga directory that are directories or archives (`ArchiveManga.isSupported`). Otherwise Cache. |
| `getDownloadSize()` / `getDownloadSize(manga)` | 235-251 | Total from Cache. For the local source, recursive size of the local manga directory. |
| `cancelQueuedDownloads(downloads)` | 253-255 | `removeFromDownloadQueue(downloads.map { it.chapter })`. |
| `deleteChapters(chapters, manga, source)` | 264-282 | See section 7. |
| `deleteManga(manga, source, removeQueued=true)` | 291-306 | See section 7. |
| `enqueueChaptersToDelete(chapters, manga)` | 331-333 | `pendingDeleter.addChapters(getChaptersToDelete(chapters, manga), manga)`. |
| `deletePendingChapters()` | 338-344 | For each `(manga, chapters)` from `pendingDeleter.getPendingChapters()`: source = `sourceManager.get(manga.source) ?: continue`, then `deleteChapters`. |
| `renameSource(oldSource, newSource)` | 352-370 | See section 8. |
| `renameChapter(source, manga, oldChapter, newChapter)` | 380-407 | See section 8. |
| `statusFlow()` / `progressFlow()` | 431-458 | See section 3.4. |

### 3.2 `removeFromDownloadQueue` (Mgr.kt:308-323)

```
wasRunning = downloader.isRunning
if wasRunning: downloader.pause()
downloader.removeFromQueue(chapters)
if wasRunning:
    if queue empty: downloader.stop()
    else if queue non-empty: downloader.start()
```

Used by `deleteChapters` and `cancelQueuedDownloads`. It is the only path that pauses before removing a chapter, which avoids the race described in section 7.3.

### 3.3 `Dl.queueChapters(manga, chapters, autoStart)` (Dl.kt:287-327)

Exact order of operations:

1. Return if `chapters` empty.
2. `source = sourceManager.get(manga.source) as? HttpSource ?: return`. Non-HTTP sources can never be queued.
3. `wasEmpty = queueState.value.isEmpty()`.
4. Build the list lazily from the input sequence in this order:
   1. `filter { provider.findChapterDir(name, scanlator, title, source) == null }` (skip already downloaded, folder or `.cbz`).
   2. `sortedByDescending { it.sourceOrder }` (highest source order first).
   3. `filter { chapter -> queueState.value.none { it.chapter.id == chapter.id } }` (skip already queued).
   4. `map { MangaDownload(source, manga, it) }`.
5. If the list is non-empty: `addAllToQueue`. Then if `autoStart && wasEmpty`:
   - `queuedDownloads = queue.count { it.source !is UnmeteredSource }`.
   - `maxDownloadsFromSource = queue.groupBy { it.source }.filterKeys { it !is UnmeteredSource }.maxOfOrNull { it.value.size } ?: 0`.
   - If `queuedDownloads > 30` (DOWNLOADS_QUEUED_WARNING_THRESHOLD) or `maxDownloadsFromSource > 15` (CHAPTERS_PER_SOURCE_QUEUE_WARNING_THRESHOLD): `notifier.onWarning(text = AYMR download_queue_size_warning, timeout = 30_000 ms, contentIntent = openUrl(MangaLibraryUpdateNotifier.HELP_WARNING_URL))`.
   - `Job.start(context)`.

Notes:

- **OBSERVED:** `sourceOrder` is the index of the chapter in the source's list (`SyncChaptersWithSource.kt:69`, `copy(sourceOrder = i.toLong())`). Sources list chapters newest-first by convention, so a higher index is an older chapter and `sortedByDescending` queues oldest first. This is an inference from the convention; the engine does not check it.
- **OBSERVED:** when the queue was not empty (for example paused, or stopped after a network loss), new chapters are added but nothing starts. Only `wasEmpty` triggers `Job.start`.
- **OBSERVED:** `autoStart=false` is used by the library updater (MangaLibraryUpdateJob.kt:335) so it never starts a download on its own.
- `HELP_WARNING_URL` = `https://aniyomi.org/docs/faq/library#why-am-i-warned-about-large-bulk-updates-and-downloads` (MangaLibraryUpdateNotifier.kt:392-393, with a comment in the source saying the URL will change when the website has the page).

### 3.4 Status and progress streams (Mgr.kt:431-458)

`statusFlow()`: `queueState.flatMapLatest { merge(download.statusFlow.drop(1).map { download }) }`, with `onStart` that emits every download whose status is DOWNLOADING right now. `progressFlow()` is identical but uses `progressFlow`.

- `statusFlow` emits once per status change after the initial value, so it does not replay the current status.
- `MangaDownload.progressFlow` (model file) is `flow { ... combine(page progress) average ... }.distinctUntilChanged().debounce(50)`. It waits in 50 ms steps until `pages != null`. The queue UI does not use its values; it only uses it as a trigger.

### 3.5 `updateQueue(downloads)` (Dl.kt:747-763)

```
wasRunning = isRunning
if downloads.isEmpty(): clearQueue(); stop(); return
pause()                 // isPaused = true, cancels job, DOWNLOADING->QUEUE
internalClearQueue()    // DOWNLOADING/QUEUE -> NOT_DOWNLOADED, store.clear()
addAllToQueue(downloads)// everything -> QUEUE, store.addAll (new order values)
if wasRunning: start()
```

- **OBSERVED:** when not running, `isPaused` stays `true` after `updateQueue`. A later `stop()` then shows the paused notification instead of the complete one (Dl.kt:173-177). Proposal: only set paused when the caller asked for pause.

### 3.6 `Dl.start()` (Dl.kt:144-157) and `Dl.stop(reason)` (Dl.kt:162-182)

`start()`:
1. Return false if `isRunning` or queue is empty.
2. `pending = queue.filter { status != DOWNLOADED }`; every pending item that is not QUEUE is set to QUEUE.
3. `isPaused = false`; `launchDownloaderJob()`.
4. Return `pending.isNotEmpty()`.

`stop(reason: String? = null)`:
1. `cancelDownloaderJob()`.
2. Every DOWNLOADING item becomes ERROR.
3. If `reason != null`: `notifier.onWarning(reason)` and **return early**. The early return skips `isPaused = false` and `Job.stop(context)` (observed, Dl.kt:168-171). Proposal: always clear the paused flag and stop the job.
4. Else if `isPaused && queue non-empty`: `notifier.onPaused()`; otherwise `notifier.onComplete()`.
5. `isPaused = false`.
6. `Job.stop(context)` (cancels the unique WorkManager work).

### 3.7 `Dl.clearQueue()` (Dl.kt:198-203)

`cancelDownloaderJob()`, `internalClearQueue()`, `notifier.dismissProgress()`. `Mgr.clearQueue` then calls `stop()`, which calls `onComplete()` and dismisses the progress notification again.

---

## 4. Scheduler and downloader loop (Dl.kt:208-270)

`launchDownloaderJob()` (returns immediately if `isRunning`):

```
downloaderJob = scope.launch {                       // scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
  activeDownloadsFlow = queueState.transformLatest { queue ->
      while (true) {
          activeDownloads = queue
              .filter { status.value <= DOWNLOADING.value }
              .groupBy { it.source }                  // LinkedHashMap, first occurrence order
              .toList().take(5)                       // at most 5 sources at once
              .map { (_, downloads) -> downloads.first() }  // one chapter per source
          emit(activeDownloads)
          if (activeDownloads.isEmpty()) break
          // wait until one active download becomes ERROR
          combine(activeDownloads.map(statusFlow)) { states -> states.contains(ERROR) }
              .filter { it }.first()
      }
  }.distinctUntilChanged()
  supervisorScope {
      downloadJobs = mutableMapOf<MangaDownload, Job>()
      activeDownloadsFlow.collectLatest { active ->
          for each (download, job) in downloadJobs where download !in active: job.cancel(); remove
          for each download in active where download !in downloadJobs: downloadJobs[download] = launchDownloadJob(download)
      }
  }
}
```

Facts:

- **Concurrency limits, exact:** at most 5 different sources are active; each source has at most 1 chapter in DOWNLOADING at a time; each chapter fetches at most 2 pages at a time (section 6.1). Maximum in-flight page requests = 10 plus retries.
- **OBSERVED:** `numberOfDownloads()` (preference `download_slots`, default 1, DownloadPreferences.kt:69) is **not read** by the manga downloader. It appears only in the settings screen. The concurrency above is hard-coded. See section 12.
- **OBSERVED:** the queue is ordered by insertion, so the first QUEUE item of each source is what runs. Moving an item to the top of its source group therefore makes it run next.
- **OBSERVED:** when a download becomes ERROR, the inner wait ends, the active list is recalculated, and the next chapter of that source starts. ERROR items remain in the queue.
- **OBSERVED cancellation edge:** a job removed from `activeDownloads` is cancelled (`job.cancel()`), but its `status` is not reset by the scheduler. Every current mutation path that could remove an active job first calls `pause()` (section 3.2, 3.5), except `removeFromQueueIf` used by `Mgr.deleteManga(removeQueued=true)` and `Dl.removeFromQueue(manga)`, which do not pause. Those two leave the status at NOT_DOWNLOADED and the job is cancelled mid-write. Proposal: pause first in those paths too.

`launchDownloadJob(download)` (Dl.kt:253-270):

```
try {
    downloadChapter(download)
    if (download.status == DOWNLOADED) removeFromQueue(download)
    if (areAllDownloadsFinished()) stop()      // no item is NOT_DOWNLOADED/QUEUE/DOWNLOADING
} catch (e: Throwable) {
    if (e is CancellationException) throw e
    logcat(ERROR, e)
    notifier.onError(e.message)                // no chapter, no manga id
    stop()
}
```

- `areAllDownloadsFinished()` = `queue.none { status.value <= DOWNLOADING.value }` (Dl.kt:685-687). So the run ends when all remaining items are DOWNLOADED or ERROR, and ERROR items stay in the queue.
- **OBSERVED:** an exception thrown outside `downloadChapter`'s try block (for example from `provider.getMangaDir` or the `createDirectory(...)!!` at Dl.kt:335 and 350) propagates here, shows an error without chapter context, and calls `stop()`. The whole run halts; the chapter stays QUEUE. This is the only path that stops the whole run because of one chapter.

---

## 5. Job lifecycle and network gating (MangaDownloadJob.kt)

### 5.1 `doWork()` (summary of the file's logic)

1. `networkCheck = checkNetworkState(activeNetworkState(), downloadOnlyOverWifi().get())`.
2. `active = networkCheck && downloadManager.downloaderStart()`. If not active: return `Result.failure()`.
3. `setForegroundSafely()` with the foreground notification (section 5.3).
4. Launch a collector: `combineTransform(networkStateFlow(), downloadOnlyOverWifi().changes()) { checkNetworkState(a, b) }` and `onEach { networkCheck = it }`.
5. Spin: `while (active) { active = !isStopped && downloadManager.isRunning && networkCheck }`.
6. Return `Result.success()`.

### 5.2 `checkNetworkState(state, requireWifi)` (MangaDownloadJob.kt)

- If `state.isOnline`:
  - `noWifi = requireWifi && !state.isWifi`.
  - If `noWifi`: `downloaderStop(text: "No Wi-Fi connection available")`.
  - return `!noWifi`.
- Else: `downloaderStop(text: "No network connection available")`, return false.

`downloaderStop(reason)` shows a warning notification (no timeout, `autoCancel` true) and returns without clearing the paused flag or stopping the job (section 3.6).

### 5.3 Foreground notification

`getForegroundInfo()`: channel `downloader_progress_channel`, content title "Downloader" (`download_notifier_downloader_title`), small icon `android.R.drawable.stat_sys_download`, notification id `ID_DOWNLOAD_CHAPTER_PROGRESS` (-201), foreground service type `DATA_SYNC` on Android Q and later.

### 5.4 Defects that the Rust version must not copy

- **OBSERVED busy-wait:** the `while (active)` loop has no suspension point or delay. While downloads run, this loop spins a worker thread. Proposal: wait on a `tokio::sync::watch` for `(running, online, wifi_ok)` changes.
- **OBSERVED no auto-resume:** when the network drops, `downloaderStop` turns active chapters to ERROR (not QUEUE), the loop exits, and the job ends with `Result.success`. Nothing restarts it when the network returns. The user must press Resume, or a new enqueue must happen on an empty queue. Proposal: on network regain, if the user had not paused, restart and reset ERROR chapters that failed only because of the network to QUEUE.
- **OBSERVED:** `checkNetworkState` runs inside the collector, so a network change during `stop()` can call `downloaderStop` again and show a second warning.

### 5.5 Start and stop of the job

- `start(context)`: `OneTimeWorkRequestBuilder<MangaDownloadJob>().addTag("MangaDownloader")`, enqueued with `enqueueUniqueWork(TAG, ExistingWorkPolicy.REPLACE, request)`.
- `stop(context)`: `cancelUniqueWork(TAG)`.
- `isRunning(context)`: `getWorkInfosForUniqueWork(TAG).get()` has exactly one item in state RUNNING.

---

## 6. Chapter pipeline (Dl.kt:334-442)

`downloadChapter(download)`:

**Step A (outside try).**
1. `mangaDir = provider.getMangaDir(manga.title, source)`. Creates `<downloads>/<sourceDir>/<mangaDir>`. On any failure: throws `Exception("Invalid location: <downloads dir path>")` (MangaDownloadProvider.kt:45-50), which propagates to `launchDownloadJob` (section 4).
2. `availSpace = DiskUtil.getAvailableStorageSpace(mangaDir)`. If `availSpace != -1` and `availSpace < MIN_DISK_SPACE` (200 * 1024 * 1024 bytes = 200 MiB): `status = ERROR`, `notifier.onError("Couldn't download due to low storage space", chapter.name, manga.title, manga.id)`, return. -1 means the free space is unknown, and the check is skipped.
3. `chapterDirname = provider.getChapterDirName(chapter.name, chapter.scanlator)` (section 8).
4. `tmpDir = mangaDir.createDirectory(chapterDirname + "_tmp")!!`.

**Step B (try).**
5. Page list: `download.pages ?: run { ... }`. If pages are already known (resume within the same process), they are reused. Otherwise:
   - `pages = source.getPageList(chapter.toSChapter())`.
   - If empty: throw `Exception("No pages found")`.
   - Re-index: `reIndexedPages = pages.mapIndexed { i, p -> Page(i, p.url, p.imageUrl, p.uri) }` ("Don't trust index from source"). `download.pages = reIndexedPages`.
6. `dataSaver = if (sourcePreferences.dataSaverDownloader().get()) DataSaver(source, sourcePreferences) else DataSaver.NoOp`. `dataSaverDownloader` defaults to `true` (SourcePreferences.kt:116); the effective mode is still NONE by default (`data_saver`, SourcePreferences.kt:97), so this is a no-op unless the user chose a data-saver service.
7. Delete every `*.tmp` file in `tmpDir` (`it.extension == "tmp"`). Finished image files are kept (resume).
8. `status = DOWNLOADING`.
9. Pages: `pageList.asFlow().flatMapMerge(concurrency = 2) { page -> flow { ... }.flowOn(IO) }.collect { notifier.onProgressChange(download) }`. Inside each page flow:
   - If `page.imageUrl.isNullOrEmpty()`: `page.status = LOAD_PAGE`; `page.imageUrl = source.getImageUrl(page)`. On exception: `page.status = ERROR` (no notification here).
   - `withIOContext { getOrDownloadImage(page, download, tmpDir, dataSaver) }`.
   - `emit(page)`.
10. `if (!isDownloadSuccessful(download, tmpDir)) { status = ERROR; return }` (no notification in this path; the per-page errors were already notified).
11. `createComicInfoFile(tmpDir, manga, chapter, source)` (always, even for folder output).
12. If `downloadPreferences.saveChaptersAsCBZ()` (default `true`): `archiveChapter(mangaDir, chapterDirname, tmpDir)`. Else `tmpDir.renameTo(chapterDirname)` (the return value is ignored).
13. `cache.addChapter(chapterDirname, mangaDir, manga)`.
14. `DiskUtil.createNoMediaFile(tmpDir, context)`. **OBSERVED:** this runs after step 12 has removed or renamed `tmpDir`. `createNoMediaFile` checks `dir.exists()`, so it is a no-op. The `.nomedia` file is effectively never created on the download path. Proposal: decide explicitly (see section 8.4).
15. `status = DOWNLOADED`.

**Catch (non-cancellation).** `status = ERROR`; `notifier.onError(error.message, chapter.name, manga.title, manga.id)`. The chapter stays in the queue.

**OBSERVED leftovers:** if step 5 or 9 fails, the `<chapter>_tmp` directory remains with its finished pages, and a `.cbz_tmp` file may remain after a failed archive (archiveChapter has no cleanup). They are reused on retry. Proposal: keep the resume behaviour, and clean stray `_tmp` directories and `.cbz_tmp` files at startup (task T-23a-044).

### 6.1 Page concurrency and ordering

- At most 2 pages at a time per chapter (`flatMapMerge(concurrency = 2)`, Dl.kt:390). Completion order is not page order. The notifier is invoked once per emitted page (Dl.kt:406-409), not once per byte.
- The per-page `imageUrl` lookup happens inside the same concurrent flow.

---

## 7. Deletion (Mgr.kt:264-306, 409-429; Dl.kt:707-745)

### 7.1 `getChaptersToDelete(chapters, manga)` (Mgr.kt:409-429)

1. `categoriesToExclude = downloadPreferences.removeExcludeCategories().get().map(String::toLong)`.
2. `categoriesForManga = getCategories.await(manga.id).map { it.id }.ifEmpty { listOf(0) }`. An uncategorised manga is treated as category 0.
3. If `categoriesForManga` intersects `categoriesToExclude`: keep only chapters where `!read`. Otherwise keep all.
4. If `!removeBookmarkedChapters()` (default `false`): drop bookmarked chapters. Otherwise keep all.

So with the default settings, bookmarked chapters are never deleted by the automatic path, and chapters in excluded categories are never deleted when they are read (the enqueue path only passes read chapters).

### 7.2 `deleteChapters(chapters, manga, source)` (Mgr.kt:264-282, launched on IO)

1. `filtered = getChaptersToDelete(chapters, manga)`; return if empty.
2. `removeFromDownloadQueue(filtered)` (section 3.2).
3. `(mangaDir, chapterDirs) = provider.findChapterDirs(filtered, manga, source)`; `chapterDirs.forEach { it.delete() }` (folder recursive delete, or `.cbz` file delete; return values ignored).
4. `cache.removeChapters(filtered, manga)`.
5. If `mangaDir?.listFiles()?.isEmpty() == true`: `deleteManga(manga, source, removeQueued = false)`.

### 7.3 `deleteManga(manga, source, removeQueued = true)` (Mgr.kt:291-306, launched on IO)

1. If `removeQueued`: `downloader.removeFromQueue(manga)` (removes by `manga.id`, no pause; see the race in section 4).
2. `provider.findMangaDir(manga.title, source)?.delete()` (recursive).
3. `cache.removeManga(manga)`.
4. `sourceDir = provider.findSourceDir(source)`. If `sourceDir?.listFiles()?.isEmpty() == true`: `sourceDir.delete()` and `cache.removeSource(source)`.

**OBSERVED:** `deleteManga` does not `pause()`, so an active chapter of that manga keeps writing into a directory that is being deleted. Proposal: use the same pause, remove, restart sequence as `removeFromDownloadQueue`.

### 7.4 Pending deletion (Del.kt, Mgr.kt:331-344, ReaderViewModel.kt:934-955)

- `enqueueDeleteReadChapters(chapter)` in the reader returns early unless `chapter.read`, then calls `enqueueChaptersToDelete(listOf(chapter), manga)` on `launchNonCancellable` (ReaderViewModel.kt:934-946).
- `enqueueChaptersToDelete` filters with `getChaptersToDelete` first (Mgr.kt:332), so exclusions and bookmarks are applied at enqueue time.
- `onActivityFinish()` (ReaderViewModel.kt:259-261) calls `deletePendingChapters()`, which calls `Mgr.deletePendingChapters()` (ReaderViewModel.kt:950-954).
- `Del.addChapters(chapters, manga)` (Del.kt:43-80), `@Synchronized`:
  1. If `lastAddedEntry.manga.id == manga.id`: `newChapters = lastEntry.chapters.addUniqueById(chapters)`.
  2. Else if prefs key `manga.id.toString()` exists: decode it and `addUniqueById`.
  3. Else new `Entry(chapters.map { it.toEntry() }, manga.toEntry())`.
  4. If no chapter was added: return (no write).
  5. Write JSON to the prefs key; `lastAddedEntry = newEntry`.
- `addUniqueById` (Del.kt:117-125) skips chapter ids already present.
- `Del.getPendingChapters()` (Del.kt:89-99), `@Synchronized`: decode all entries (undecodable ones are skipped silently), `clear()` the prefs file **first**, `lastAddedEntry = null`, then return `Map<Manga, List<Chapter>>` built with `toModel()`.
- **OBSERVED:** `toModel()` builds `Chapter.create().copy(id, url, name, scanlator)` and `Manga.create().copy(url, title, source, id)`. `read` and `bookmark` are reset to defaults (false). The second `getChaptersToDelete` pass in `deleteChapters` therefore cannot see the read flag. The exclusion decisions taken at enqueue time are final; changes to the exclusion settings between the two moments are ignored.
- **OBSERVED:** the list is cleared before deletion starts, so a crash during deletion loses the remaining entries and never retries. Proposal: delete the rows in the same SQLite transaction as the directory deletes, or keep them until each chapter is gone.

---

## 8. Renames and naming hooks (Mgr.kt:352-407, Prov.kt, Cache.kt)

### 8.1 `renameSource(oldSource, newSource)` (Mgr.kt:352-370)

1. `oldFolder = provider.findSourceDir(oldSource) ?: return`.
2. `newName = provider.getSourceDirName(newSource)` (`"<name> (<LANG>)"`, section 8.2).
3. If `oldFolder.name == newName` return.
4. `capitalizationChanged = oldFolder.name.equals(newName, ignoreCase = true)`.
5. If so: rename to `newName + "_tmp"` first. If that fails, log `"Failed to rename source download folder: <old>"` and return.
6. Rename to `newName`. On failure, log the same message. No cache update (**OBSERVED:** the index keeps the old folder reference until the next rescan or invalidation).

Called from the source managers when a source changes name (AndroidMangaSourceManager.kt:110, area:17).

### 8.2 Directory naming (Prov.kt)

All names pass through `DiskUtil.buildValidFilename` (section 8.3).

| Function (Prov.kt) | Lines | Result |
|---|---|---|
| `getSourceDirName(source)` | 115-117 | `buildValidFilename(source.toString())`. `HttpSource.toString()` is `"$name (${lang.uppercase()})"` (source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:103). Example: `MangaDex (EN)`. |
| `getMangaDirName(title)` | 124-126 | `buildValidFilename(title)`. |
| `getChapterDirName(name, scanlator)` | 134-142 | `newName = name.ifBlank { "Chapter" }`. If `scanlator` is non-null and not blank: `buildValidFilename("${scanlator}_$newName")`. Else `buildValidFilename(newName)`. |
| `isChapterDirNameChanged(old, new)` | 155-158 | True if `old.name != new.name`, or if the scanlators differ after treating blank as null. Public, with no caller in this area. |
| `getValidChapterDirNames(name, scanlator)` | 166-175 | `[chapterDirName, chapterDirName + ".cbz"]`. |
| `findChapterDir(name, scanlator, title, source)` | 82-92 | First of the valid names found in the manga folder, else null. |
| `findChapterDirs(chapters, manga, source)` | 101-108 | `(mangaDir, list)`. If no manga folder: `(null, emptyList())`. |

Edge: two chapters whose sanitised names collide (same scanlator and name after sanitising) share one folder. The second one is then reported as downloaded and is skipped by `queueChapters`.

### 8.3 `buildValidFilename(origName)` (core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:109-125)

1. `name = origName.trim('.', ' ')` (removes leading and trailing dots and spaces).
2. If empty: return `"(invalid)"`.
3. Each character: keep unless it is in `0x00..0x1f`, `0x7f`, or one of `" * / : < > ? \ |`. Replace those with `_`.
4. Return `take(240)` (240 UTF-16 code units, not bytes).

Proposal (addendum rule 5, `addenda/requirements-cross-platform.md`): keep this exact rule for Linux so folders stay compatible with an Aniyomi download folder copied over, and add the NTFS-only rules (reserved device names CON, PRN, AUX, NUL, COM1-9, LPT1-9 get a trailing `_`) in a second function used when writing to a path that the Windows port will read. Decision point, recorded as T-23a-003.

### 8.4 `.nomedia`

Constant `DiskUtil.NOMEDIA_FILE = ".nomedia"` (DiskUtil.kt:140). **OBSERVED:** because of the order in section 6 step 14, the file is not written on the download path. Whether the user wants it hidden from gallery apps is a Linux question; Proposal: write `.nomedia` into the downloads root once, not per chapter.

### 8.5 Chapter rename (Mgr.kt:380-407)

1. `oldNames = provider.getValidChapterDirNames(oldChapter.name, oldChapter.scanlator)`.
2. `mangaDir = provider.getMangaDir(manga.title, source)` (creates the folder if missing; may throw invalid location).
3. `oldDownload = oldNames.asSequence().mapNotNull { mangaDir.findFile(it) }.firstOrNull() ?: return`.
4. `newName = provider.getChapterDirName(newChapter.name, newChapter.scanlator)`. If `oldDownload` is a file with extension `cbz`: `newName += ".cbz"`.
5. If `oldDownload.name == newName` return.
6. `renameTo(newName)`. On success: `cache.removeChapter(oldChapter, manga)` and `cache.addChapter(newName, mangaDir, manga)`. On failure: log `"Could not rename downloaded chapter: <old names>"`.

**OBSERVED:** step 6 adds the name **with** `.cbz` to the index, while scans store names **without** the suffix (Cache.kt, `renewCache`, `nameWithoutExtension`). `isChapterDownloaded` still matches because `getValidChapterDirNames` also yields `"<name>.cbz"`. Proposal: store the bare name in the index.

Called from `SyncChaptersWithSource.kt:129` (area:10) when a chapter's name or scanlator changes after a source sync.

---

## 9. Reading downloaded chapters: `buildPageList` (Mgr.kt:167-184)

1. `chapterDir = provider.findChapterDir(chapter.name, chapter.scanlator, manga.title, source)`.
2. `files = chapterDir?.listFiles().orEmpty().filter { it.isFile && ImageUtil.isImage(it.name) { it.openInputStream() } }`.
   - `ImageUtil.isImage(name, openStream)`: true if the extension is one of `avif, gif, heif, jpg, jxl, png, webp` (the `ImageType` extensions, ImageUtil.kt:104-113), or if the bytes sniff as an image.
   - Note `ImageType.JPEG.extension` is `"jpg"`, so `.jpeg` files are only recognised by sniffing.
3. If empty: throw `Exception("No pages found")`.
4. `files.sortedBy { it.name }` (string order, so `10.jpg` sorts before `2.jpg`; the downloader's zero padding of at least 3 digits prevents this for downloaded chapters). Each gets `Page(i, uri = file.uri)` with status READY.

Note: for a CBZ-stored chapter, `findChapterDir` returns the `.cbz` file, and `listFiles()` on a file returns null, so `buildPageList` throws "No pages found". **OBSERVED:** the reader must open CBZ chapters through an archive reader (area:43/13), and `buildPageList` does not support them. Proposal: `buildPageList` returns an archive-backed page source for `.cbz`.

---

## 10. Per-page image fetch (Dl.kt:451-550)

### 10.1 `getOrDownloadImage(page, download, tmpDir, dataSaver)`

1. If `page.imageUrl == null`: return silently (page keeps its status).
2. `digitCount = max(3, (download.pages?.size ?: 0).toString().length)`. `filename = String.format("%0${digitCount}d", page.number)` where `page.number = index + 1`. Examples: 9 pages gives `001`; 1234 pages gives `0001`.
3. `tmpFile = tmpDir.findFile("$filename.tmp")`; delete it if present.
4. `imageFile = tmpDir.listFiles().firstOrNull { name startsWith "$filename." or startsWith "${filename}__001" }`.
5. In try:
   - `file = when { imageFile != null -> imageFile; chapterCache.isImageInCache(imageUrl) -> copyImageFromCache(...); else -> downloadImage(...) }`.
   - `splitTallImageIfNeeded(page, tmpDir)`.
   - `page.uri = file.uri`, `page.progress = 100`, `page.status = READY`.
6. Catch (non-cancellation): `page.progress = 0`, `page.status = ERROR`, `notifier.onError(e.message, chapter.name, manga.title, manga.id)`.

Observed consequences:
- A page that already has a finished file is never re-validated (its size or decodability is not checked).
- **OBSERVED:** each failed page calls `onError`, and every call uses the same error notification id (-202), so only the last error stays visible.

### 10.2 `downloadImage(page, source, tmpDir, filename, dataSaver)` (Dl.kt:511-550)

- `page.status = DOWNLOAD_IMAGE`, `page.progress = 0`.
- The flow body:
  1. `response = source.getImage(page, dataSaver)` (the HttpSource API; the data-saver swap is in DataSaver.getImage, aniyomi/util/DataSaver.kt).
  2. `file = tmpDir.createFile("$filename.tmp")!!`.
  3. Try:
     - `throttler.bytesPerSecond(downloadPreferences.downloadSpeedLimit().get().toLong() * 1024)`. Shared by all concurrent downloads of this downloader.
     - `throttledSource = throttler.source(response.body.source()).buffer()`; `throttledSource.saveTo(file.openOutputStream())`; `close()`.
     - `extension = getImageExtension(response, file)`; `file.renameTo("$filename.$extension")`.
  4. Catch: `response.close()`, `file.delete()`, rethrow.
  5. `emit(file)`.
- `.retryWhen { _, attempt -> if (attempt < 3) { delay((2L shl attempt.toInt()) * 1000); true } else false }`.
  - Attempts counted from 0. Retries happen after the 1st, 2nd and 3rd failures, waiting **2 s, 4 s and 8 s**. So at most 4 attempts in total.
  - `retryWhen` does not check the cause. Every exception from the body is retried, including HTTP 403 and 404.
- `.flowOn(Dispatchers.IO).first()`.

Speed limit: `download_speed_limit` is in KiB/s; the code multiplies by 1024 to get bytes per second. `0` means no limit (the UI hint says "Set to 0 to disable the speed limit.", string `download_speed_limit_hint`).

### 10.3 `getImageExtension(response, file)` (Dl.kt:579-582)

- `mime = response.body.contentType()?.run { if (type == "image") "image/$subtype" else null }`.
- `ImageUtil.getExtensionFromMimeType(mime) { file.openInputStream() }`:
  - If `mime` matches an `ImageType.mime` (table below), use its extension.
  - Else sniff the bytes with `findImageType`.
  - Else `"jpg"`.

| ImageType | mime | extension |
|---|---|---|
| AVIF | image/avif | avif |
| GIF | image/gif | gif |
| HEIF | image/heif | heif |
| JPEG | image/jpeg | jpg |
| JXL | image/jxl | jxl |
| PNG | image/png | png |
| WEBP | image/webp | webp |

### 10.4 `copyImageFromCache(cacheFile, tmpDir, filename)` (Dl.kt:559-570)

1. `tmpFile = tmpDir.createFile("$filename.tmp")!!`; copy the cache file into it (streams are closed).
2. `extension = ImageUtil.findImageType(cacheFile.inputStream()) ?: return tmpFile`.
3. `tmpFile.renameTo("$filename.${extension.extension}")`; `cacheFile.delete()`.

**OBSERVED defect:** when the type cannot be recognised, step 2 returns the `.tmp` file. The page is then marked READY, and `isDownloadSuccessful` ignores `.tmp` files, so the chapter fails with ERROR. Proposal: fall back to downloading from the network, or rename with `jpg` as `getExtensionFromMimeType` does.

Source of the cache: `ChapterCache.isImageInCache` / `getImageFile` (app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt, area:34). The key is `DiskUtil.hashKeyForDisk(imageUrl) + ".0"`.

### 10.5 `splitTallImageIfNeeded(page, tmpDir)` (Dl.kt:584-599)

1. If `split_tall_images` (default `true`, DownloadPreferences.kt:23) is false: return.
2. In try:
   - `prefix = String.format("%03d", page.number)` (always at least 3 digits).
   - `imageFile = tmpDir.listFiles().firstOrNull { name startsWith prefix } ?: error("Page <n> not found while splitting")`.
   - If `imageFile.name startsWith "${prefix}__"`: return (already split).
   - `ImageUtil.splitTallImage(tmpDir, imageFile, prefix)` (return value ignored).
3. Catch Exception: log only. No user notification.

**OBSERVED defect (page numbers of 1000 or more):** the file name uses `digitCount` (4 digits when there are 1000 or more pages), but the prefix is always 3 digits. For page 1, prefix `001` matches file `0010.jpg` (page 10), so the wrong file can be split and deleted, and for pages 1000 and above no file matches (error logged, page not split). Proposal: use the same digit count everywhere, and match with `filename.` exact prefix. Covered by T-23a-018.

### 10.6 Tall image splitting (core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt)

- `isTallImage(imageSource)`: `(outHeight / outWidth) > 3` with **Int division** (ImageUtil.kt:206-209). So the effective rule is `outHeight >= 4 * outWidth`. The doc comment says "height:width ratio is greater than 3", which is not what the code does.
- `splitTallImage(tmpDir, imageFile, filenamePrefix)`:
  1. Read all bytes. If `isAnimatedAndSupported` or not tall: return true (no change).
  2. `BitmapRegionDecoder` from the bytes; null means false.
  3. `splitData` per section 10.7.
  4. For each `SplitData`: name = `"${prefix}__${%03d(index+1)}.jpg"`; delete an existing file with that name; create; decode region `Rect(0, topOffset, splitWidth, bottomOffset)` with `inJustDecodeBounds = false`; `Bitmap.compress(JPEG, 100)`; recycle.
  5. On success: delete the original `imageFile`, return true.
  6. On exception: delete any split files created, keep the original, return false.
- `optimalImageHeight = getDisplayMaxHeightInPx * 2`, where `getDisplayMaxHeightInPx = max(displayMetrics.heightPixels, widthPixels)` of the system display (ImageUtil.kt:609, 724-726). On Linux: the primary monitor's larger pixel dimension times 2 (Proposal, see T-23a-018).

### 10.7 Split geometry (ImageUtil.kt, `splitData`)

```
partCount        = (imageHeight - 1) / optimalImageHeight + 1
optimalSplitHeight = imageHeight / partCount
for index in 0 until partCount:
    if list nonempty and imageHeight <= last().bottomOffset: break
    topOffset   = index * optimalSplitHeight
    splitHeight = min(optimalSplitHeight, imageHeight - topOffset)
    if index == partCount - 1:
        splitHeight += imageHeight - (topOffset + splitHeight)     // absorb the remainder
    add SplitData(index, topOffset, splitHeight, imageWidth)
SplitData.bottomOffset = topOffset + splitHeight
```

Example: imageHeight 5000, optimalImageHeight 2000 gives partCount = 4999/2000 + 1 = 3, optimalSplitHeight = 1666. Parts: [0,1666), [1666,3332), [3332,5000).

### 10.8 Integrity check: `isDownloadSuccessful(download, tmpDir)` (Dl.kt:607-629)

1. `downloadPageCount = download.pages?.size ?: return false`.
2. If `download.downloadedImages != downloadPageCount` (count of pages with status READY): false.
3. Count entries in `tmpDir` that are **not**: `ComicInfo.xml`, `.nomedia`, names ending `.tmp`, names containing `__` unless they end with `__001.jpg`.
4. Return `count == downloadPageCount`.

Observed: stray files in the folder (from an earlier, different page list of the same chapter) make the count wrong and fail the chapter every time. Proposal: count the page keys (`NNN` or `NNN__001`) rather than files.

### 10.9 `createComicInfoFile(dir, manga, chapter, source)` (Dl.kt:652-680)

1. `categories = getCategories.await(manga.id).map { it.name.trim() }.takeUnless { it.isEmpty() }` (null when none).
2. `urls = getMangaTracks.await(manga.id).mapNotNull { it.remoteUrl.takeUnless { blank }?.trim() }.plus(source.getChapterUrl(chapter.toSChapter()).trim()).distinct()`.
3. `comicInfo = getComicInfo(manga, chapter, urls, categories, source.name)` (fields in section 10.10).
4. Delete an existing `ComicInfo.xml`, create it, and write `xml.encodeToString(ComicInfo.serializer(), comicInfo)` as UTF-8 bytes.

### 10.10 `getComicInfo(...)` (app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:95-125)

| Field | Value |
|---|---|
| title | `chapter.name` |
| series | `manga.title` |
| number | `chapter.chapterNumber` if `>= 0`; integer string if whole (`rem(1) == 0.0`), else `Double.toString()`; omitted if negative |
| web | `urls.joinToString(" ")` (always present, may be empty) |
| summary | `manga.description` if non-null |
| writer | `manga.author` if non-null |
| penciller | `manga.artist` if non-null |
| translator | `chapter.scanlator` if non-null |
| genre | `manga.genre.joinToString()` (separator `", "`) if non-null |
| publishingStatus (tachiyomi ns) | `ComicInfoPublishingStatus.toComicInfoValue(manga.status)` |
| categories (tachiyomi ns) | `categories.joinToString()` if non-null |
| source (aniyomi ns) | `source.name` |
| inker, colorist, letterer, coverArtist, tags | null (omitted) |

Status mapping (`ComicInfoPublishingStatus`, core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt:165-190): status value to text: 1 "Ongoing", 2 "Completed", 3 "Licensed", 4 "Publishing finished", 5 "Cancelled", 6 "On hiatus", anything else (including 0) "Unknown". Values match `SManga.ONGOING`, `SManga.COMPLETED`, `SManga.LICENSED`, `SManga.PUBLISHING_FINISHED`, `SManga.CANCELLED`, `SManga.ON_HIATUS` and `SManga.UNKNOWN` (source-api/.../source/model/SManga.kt; SAnime.kt:72-78 has the same numbers).

XML: root `ComicInfo` with namespace attributes `xmlns:xsd="http://www.w3.org/2001/XMLSchema"` and `xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"`. The element order follows the data class: Title, Series, Number, Summary, Writer, Penciller, Inker, Colorist, Letterer, CoverArtist, Translator, Genre, Tags, Web, PublishingStatusTachiyomi, Categories, SourceAniyomi (the last three use namespace prefixes `ty` and `ay`, and the URI `http://www.w3.org/2001/XMLSchema`).

---

## 11. Finalisation: archive or folder

### 11.1 `archiveChapter(mangaDir, dirname, tmpDir)` (Dl.kt:634-647)

1. `zip = mangaDir.createFile("$dirname.cbz_tmp")!!`.
2. `ZipWriter(context, zip).use { writer -> tmpDir.listFiles()?.forEach { writer.write(it) } }`.
3. `zip.renameTo("$dirname.cbz")`.
4. `tmpDir.delete()`.

ZipWriter (core/archive/src/main/kotlin/mihon/core/archive/ZipWriter.kt): libarchive; format ZIP; compression **store** (no compression); charset UTF-8; each file is written as a flat entry named `file.name` with the file's stat (mode, mtime) copied; 8 KiB buffer. Entry order is the directory listing order (unsorted). ComicInfo.xml is included. There is no `.nomedia` at this point.

**OBSERVED:** the `.cbz_tmp` file is not deleted when writing fails. Exceptions propagate to the chapter catch (section 6), and the stale file makes the next attempt's `createFile` ambiguous. Proposal: write to `<name>.cbz.part`, rename on success, delete the part on failure.

### 11.2 Folder output

`tmpDir.renameTo(chapterDirname)`. Return value ignored (**OBSERVED**). If a folder with that name already exists, the rename fails silently and the files stay in `_tmp` while the chapter is marked DOWNLOADED. `queueChapters` prevents this in normal flows.

---

## 12. Settings consumed here (all default values from DownloadPreferences.kt and SourcePreferences.kt)

| Key | Default | Read at | Effect |
|---|---|---|---|
| `pref_download_only_over_wifi_key` | `true` | Job.checkNetworkState | Downloads stop on a metered or non-Wi-Fi link |
| `save_chapter_as_cbz` | `true` | Dl step 12 | CBZ or folder output |
| `split_tall_images` | `true` | Dl.splitTallImageIfNeeded | Split tall pages |
| `download_speed_limit` | `0` | Dl.downloadImage | KiB/s cap, shared by all downloads; 0 = unlimited |
| `download_slots` | `1` | **Not read by the manga downloader** (UI only) | Dead for manga; see below |
| `remove_exclude_categories` | empty set | Mgr.getChaptersToDelete | Categories whose read chapters are kept |
| `pref_remove_bookmarked` | `false` | Mgr.getChaptersToDelete | Allow deleting bookmarked chapters |
| `data_saver_downloader` | `true` (SourcePreferences.kt:116) | Dl step 6 | Use the data-saver proxy in the downloader |
| `data_saver` | `NONE` (SourcePreferences.kt:97) | aniyomi/util/DataSaver.kt | Which proxy |
| `hide_notification_content` | `false` (core/common/.../SecurityPreferences.kt:19) | Ntf.onProgressChange | Hide manga and chapter names in progress notifications |

`download_slots` (`DownloadPreferences.numberOfDownloads`, DownloadPreferences.kt:69): **OBSERVED** no reader in the manga downloader (only SettingsDownloadScreen.kt:102 writes it). The manga queue is limited to 5 sources times 1 chapter, regardless of this value. Proposal: either honour it (as the maximum number of concurrently downloading chapters, capped at the scheduler limit) or remove the control. Decision in T-23a-037.

---

## 13. Notifications (MangaDownloadNotifier.kt)

Channels and ids (from app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt):

| Constant | Value |
|---|---|
| CHANNEL_DOWNLOADER_PROGRESS | `downloader_progress_channel` (importance LOW, group `group_downloader`, badge off, name "Progress") |
| CHANNEL_DOWNLOADER_ERROR | `downloader_error_channel` (importance LOW, group `group_downloader`, name "Errors") |
| ID_DOWNLOAD_CHAPTER_PROGRESS | -201 |
| ID_DOWNLOAD_CHAPTER_ERROR | -202 |
| group name | `downloader_title` = "Downloader" |

### 13.1 Progress: `onProgressChange(download)` (Ntf.kt:73-119)

- First call while not already downloading (`isDownloading == false`):
  - small icon `stat_sys_download`; clear actions; content intent opens the download manager.
  - Action 1: "Pause" (`MR.action_pause`) broadcasts pause-downloads.
  - Action 2: "Show manga" (`AYMR.action_show_manga`) opens the manga entry.
  - `isDownloading = true`.
- `progressText = "Downloading (<downloadedImages>/<pages.size>)"` (`chapter_downloading_progress`, `%1$d/%2$d`).
- If `hide_notification_content`: title = progressText, text = null.
- Else:
  - `title = manga.title.chop(15)`.
  - `quoted = Pattern.quote(title)`; `chapter = chapter.name.replaceFirst(Regex("$quoted[\\s]*[-]*[\\s]*", IGNORE_CASE), "")`.
  - contentTitle = `"$title - $chapter".chop(30)`; contentText = progressText.
- `setProgress(max = pages.size, value = downloadedImages, indeterminate = false)`; `setOngoing(true)`; show id -201.

`chop(count, replacement = "…")` (core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt:12-18): if `length > count`, return `take(count - replacement.length) + replacement`, else the string unchanged.

**OBSERVED:** the regex is built from the already chopped title (15 characters), so for longer titles the chapter prefix is not removed.

### 13.2 Paused: `onPaused()` (Ntf.kt:124-152)

- Title "Paused" (`AYMR.download_paused`); text "Chapter download paused" (`AYMR.download_notifier_download_paused_chapters`).
- Icon: pause; progress (0, 0, indeterminate false); ongoing false; content intent download manager.
- Actions: "Resume" (`MR.action_resume`) and "Cancel all" (`MR.action_cancel_all`).
- Show -201; `isDownloading = false`.

### 13.3 Complete: `onComplete()` (Ntf.kt:157-162)

`dismissProgress()` (cancel -201) and `isDownloading = false`.

### 13.4 Warning: `onWarning(reason, timeout?, contentIntent?, mangaId?)` (Ntf.kt:172-196)

- Channel error (-202). Title "Downloader". BigTextStyle with `reason`. Icon: warning. `autoCancel(true)`. Clear actions. Content intent: download manager (or `contentIntent` if given).
- If `mangaId != null`: action "Show manga".
- Progress (0, 0). If `timeout != null`: `setTimeoutAfter(timeout)`.
- `isDownloading = false`.
- Used with: no timeout for network and Wi-Fi reasons; 30 s timeout and help URL for the queue-size warning.

### 13.5 Error: `onError(error?, chapter?, mangaTitle?, mangaId?)` (Ntf.kt:206-232)

- Title: if `mangaTitle != null` then `"$mangaTitle: $chapter"` (the chapter is printed even when null, giving `"Title: null"`); else "Downloader".
- Text: `error ?: "Could not download chapter due to unexpected error"`.
- Icon: warning; `autoCancel` stays **false**; clear actions; content intent download manager; "Show manga" if `mangaId != null`; progress (0, 0). Show -202. `isDownloading = false`.

**OBSERVED:** errors are posted on the LOW-importance channel, so they make no sound.

Ntf also keeps `isDownloading` as its only state, so after an error the next progress update re-adds the Pause and Show manga actions.

---

## 14. Queue screen model (QVM, ui/download/manga/MangaDownloadQueueScreenModel.kt)

### 14.1 State (QVM.kt:118-132)

`state` = queue grouped by source: for each `(source, downloads)` in `queue.groupBy { it.source }` (first-occurrence order), a `MangaDownloadHeaderItem(id = source.id, name = source.name, size = downloads.size)` with items `MangaDownloadItem(download, header)` in queue order.

`countInTab` = sum of subItems sizes (DownloadsTab uses this as the tab badge through `numberTitle`, MangaDownloadQueueTab.kt:406).

### 14.2 Drag reorder: `onItemReleased(position)` (QVM.kt:52-60)

Flatten all headers' subitems in header order, then `reorder(downloads)` (= `Mgr.reorderQueue`). **OBSERVED:** the adapter's `shouldMove` blocks moves across groups (MangaDownloadAdapter.kt:18-21), so the queue cannot interleave sources after a drag, but the flattening rewrites the whole queue into group order.

### 14.3 Row menu: `onMenuItemClick(position, menuItem)` (QVM.kt:68-115)

| Menu id (res/menu/download_single.xml) | Label key | Behaviour |
|---|---|---|
| `move_to_top` | `action_move_to_top` "Move to top" | In its group, remove and insert at index 0; rebuild the flat queue from header order; `reorder`. Visible when `bindingAdapterPosition > 1`. |
| `move_to_bottom` | `action_move_to_bottom` "Move to bottom" | In its group, remove and append; rebuild and reorder. Visible when not the last adapter position. |
| `move_to_top_series` | `action_move_to_top_all_for_series` "Move series to top" | Queue = (items of the same manga, in current order) + (other items, in current order). Always visible. |
| `move_to_bottom_series` | `action_move_to_bottom_all_for_series` "Move series to bottom" | Queue = (others) + (same manga). Always visible. |
| `cancel_download` | `action_cancel` "Cancel" | `cancel([download])`. |
| `cancel_series` | `cancel_all_for_series` "Cancel all for this series" | `cancel(all items with the same manga.id in current adapter items)`. |

**OBSERVED:** `move_to_top` and `move_to_bottom` rebuild the flat list from all headers, so a single move also re-sorts the global queue into source groups in header order.

### 14.4 Sort: `reorderQueue(selector, reverse)` (QVM.kt:168-184)

For each header: `subItems = subItems.sortedBy(selector)`, then `reverse()` if `reverse`. Flatten in header order; `reorder`. Menu wiring (DownloadsTab.kt:384-455):

| Menu | Selector | reverse |
|---|---|---|
| By upload date > Newest | `chapter.dateUpload` | true |
| By upload date > Oldest | `chapter.dateUpload` | false |
| By chapter number > Ascending | `chapter.chapterNumber` | false |
| By chapter number > Descending | `chapter.chapterNumber` | true |

Labels: "Sort" (`action_sort`), "By upload date" (`action_order_by_upload_date`), "Newest", "Oldest", "By chapter number" (`action_order_by_chapter_number`), "Ascending", "Descending".

### 14.5 Overflow: "Cancel all" (`action_cancel_all`) calls `clearQueue()` (DownloadsTab.kt:452-455).

### 14.6 Status and progress hooks (QVM.kt:191-261)

`onStatusChange(download)`:
- DOWNLOADING: launch the progress job; `onUpdateDownloadedPages`.
- DOWNLOADED: cancel the progress job; `onUpdateProgress`; `onUpdateDownloadedPages`.
- ERROR: cancel the progress job.
- Others: nothing.

`launchProgressJob(download)`:
1. Poll every 50 ms until `download.pages != null`.
2. `combine(page progress flows, Array<Int>::sum).distinctUntilChanged().debounce(50).collectLatest { onUpdateProgress(download) }`.
3. Store the job in `progressJobs[download]` (replacing and cancelling any older job).

`onUpdateProgress` and `onUpdateDownloadedPages` look up the bound row (`findViewHolderForItemId(chapter.id)`) and call `notifyProgress` or `notifyDownloadedPages`. If the row is not bound, nothing happens.

Row progress (Holder.kt): `progressBar.max = pages.size * 100`; value = `download.totalProgress` (sum over pages of `Page.progress`); text "`<downloadedImages>/<pages.size>`". `totalProgress` is a SUM (Model.kt, `totalProgress`), while `MangaDownload.progress` is an AVERAGE. The row uses the sum, so `pages.size * 100` is the correct maximum.

`isDownloaderRunning`: `Mgr.isDownloaderRunning.stateIn(WhileSubscribed(5000), false)`.

`onDispose()`: cancel all progress jobs, clear them, `adapter = null`.

### 14.7 Empty and tab (QueueScreen.kt:309-315, QueueTab.kt:391-408)

- Empty list: `EmptyScreen(stringRes = MR.strings.information_no_downloads)` ("No downloads"), padding applied.
- Tab: `titleRes = AYMR.strings.label_manga` ("Manga"), `searchEnabled = false`, `numberTitle = downloadCount`, `navigateUp = navigator::pop`.

---

## 15. Error message keys (every user-visible string in this scope)

| Key | Module | English text | Emitted at |
|---|---|---|---|
| page_list_empty_error | MR | "No pages found" | Mgr.buildPageList (Mgr.kt:178), Dl.downloadChapter (Dl.kt:359) |
| invalid_location | MR | "Invalid location: %s" | Prov.getMangaDir (Prov.kt:45-50) |
| download_notifier_downloader_title | MR | "Downloader" | Job foreground, Ntf.onWarning, Ntf.onError, group name |
| download_notifier_text_only_wifi | MR | "No Wi-Fi connection available" | Job.checkNetworkState |
| download_notifier_no_network | MR | "No network connection available" | Job.checkNetworkState |
| download_notifier_unknown_error | MR | "Could not download chapter due to unexpected error" | Ntf.onError fallback |
| download_notifier_split_page_not_found | MR | "Page %d not found while splitting" | Dl.splitTallImageIfNeeded (logged only) |
| chapter_downloading_progress | MR | "Downloading (%1$d/%2$d)" | Ntf.onProgressChange |
| action_pause | MR | "Pause" | Ntf progress action |
| action_resume | MR | "Resume" | Ntf paused action |
| action_cancel_all | MR | "Cancel all" | Ntf paused action; queue overflow |
| information_no_downloads | MR | "No downloads" | QueueScreen empty |
| channel_progress | MR | "Progress" | channel name |
| channel_errors | MR | "Errors" | channel name |
| label_manga | AYMR | "Manga" | QueueTab title |
| action_show_manga | AYMR | "Show manga" | Ntf actions |
| download_paused | AYMR | "Paused" | Ntf.onPaused |
| download_notifier_download_paused_chapters | AYMR | "Chapter download paused" | Ntf.onPaused |
| download_insufficient_space | AYMR | "Couldn't download due to low storage space" | Dl step 2 |
| download_queue_size_warning | AYMR | "Warning: large bulk downloads may lead to sources becoming slower and/or blocking Aniyomi. Tap to learn more." | Dl.queueChapters |
| action_move_to_top | MR | "Move to top" | row menu |
| action_move_to_top_all_for_series | MR | "Move series to top" | row menu |
| action_move_to_bottom | MR | "Move to bottom" | row menu |
| action_move_to_bottom_all_for_series | MR | "Move series to bottom" | row menu |
| action_cancel | MR | "Cancel" | row menu |
| cancel_all_for_series | MR | "Cancel all for this series" | row menu |
| label_download_queue | MR | "Download queue" | tab label (Downloads page) |
| action_sort | MR | "Sort" | sort action |
| action_order_by_upload_date | MR | "By upload date" | sort menu |
| action_order_by_chapter_number | MR | "By chapter number" | sort menu |
| action_newest / action_oldest | MR | "Newest" / "Oldest" | sort menu |
| action_asc / action_desc | MR | "Ascending" / "Descending" | sort menu |

Errors that carry raw exception text: any `e.message` from the source, the network, or the file system is shown verbatim in the error notification (Ntf.onError). Proposal: map known errors to keys and keep the raw text in the log.

---

## 16. Timings, debounces and limits (complete)

| Name | Value | Where |
|---|---|---|
| Index renew interval | 1 hour | Cache.kt renewInterval |
| Index load wait for sources | 30 s (`withTimeoutOrNull`) | Cache.renewCache |
| Index "indexing" indicator debounce | 1000 ms | Cache.isInitializing |
| Index disk write debounce | 1000 ms | Cache.updateDiskCache |
| Index disk file name | `dl_index_cache_v3` in the app cache dir | Cache.diskCacheFile |
| Page retry | 3 retries; waits 2 s, 4 s, 8 s | Dl.downloadImage |
| Page concurrency per chapter | 2 | Dl flatMapMerge |
| Source concurrency | 5 sources, 1 chapter each | Dl.launchDownloaderJob |
| Min free disk space | 200 MiB (`200L * 1024 * 1024`) | Dl MIN_DISK_SPACE |
| Warning timeout | 30 s | Dl.WARNING_NOTIF_TIMEOUT_MS |
| Queue-size warning thresholds | more than 30 queued non-unmetered; more than 15 for one source | Dl.queueChapters |
| Progress poll while page list is null | 50 ms | MangaDownload.progressFlow, QVM.launchProgressJob |
| Progress debounce | 50 ms | MangaDownload.progressFlow, QVM.launchProgressJob |
| Status-to-UI stateIn timeout | 5000 ms | QVM.isDownloaderRunning |
| Speed limit unit | KiB/s (x1024 bytes/s), 0 = unlimited | Dl.downloadImage |
| Tall split: optimal height | 2 x max display dimension | ImageUtil.optimalImageHeight |
| Tall split: tall rule | `h / w > 3` with Int division (h >= 4w) | ImageUtil.isTallImage |
| Split JPEG quality | 100 | ImageUtil.splitTallImage |
| Name length cap | 240 UTF-16 code units | DiskUtil.buildValidFilename |
| Notification title caps | 15 chars (manga), 30 chars (line) | Ntf.onProgressChange |
| Pending deletes | no timer; flushed on reader activity finish | ReaderViewModel.onActivityFinish |

---

## 17. Concurrency and dispatchers

- Downloader scope: `CoroutineScope(SupervisorJob() + Dispatchers.IO)` (Dl.kt:116). Each download runs as a child of a `supervisorScope` inside the downloader job, so one failing chapter does not cancel the others.
- Queue state: `MutableStateFlow` updated with `update { }`. **OBSERVED:** `addAllToQueue` and `removeFromQueue` put side effects (store writes and status mutation) inside the `update` lambda (Dl.kt:689-707). A CAS retry would repeat them. Proposal: mutate under one mutex, then publish a snapshot.
- `Dl.isPaused` is `@Volatile`.
- Cache: a `Mutex` for the root directory, `Channel.UNLIMITED` for change notifications, `shareIn(Lazily, replay 1)`.
- `Mgr.deleteChapters`, `deleteManga` run on `launchIO`. `Mgr.deletePendingChapters` is called from `launchNonCancellable` in the reader.
- `Del` methods are `@Synchronized` (JVM monitor). Proposal: a single `Mutex` owned by the pending store.
- `runBlocking` is used inside `Store.restore`, `Mgr.startDownloadNow` (`MangaDownload.fromChapterId`), which can block the main thread. Proposal: async.

---

## 18. Interactions with other areas (summary)

| Area | Interaction | Direction |
|---|---|---|
| area:08 | Library categories for exclusion filters and ComicInfo categories | Downloads read |
| area:10 | `SyncChaptersWithSource` calls `renameChapter`; `AndroidMangaSourceManager` calls `renameSource` | Callers into this area |
| area:12 | HTTP client, image response body and progress reporting into `Page` | Downloads read |
| area:13 | `ImageUtil`, `DiskUtil`, `ZipWriter`, UniFile helpers | Downloads read |
| area:14 | ComicInfo model and local source | Shared |
| area:15 | `Page`, `HttpSource.getPageList`, `getImageUrl`, `getImage`, `getChapterUrl` | Downloads call |
| area:17 | `MangaSourceManager`, source initialisation gate | Downloads read |
| area:25 | `DownloadPreferences`, `DeleteChapterDownload` interactor, Downloads tab | Shared |
| area:26 | `MangaLibraryUpdateJob` calls `downloadChapters(autoStart = false)`; `DataSaver` | Callers into this area |
| area:27 | `GetMangaTracks` for ComicInfo URLs | Downloads read |
| area:34 | `ChapterCache` (reader page cache reused by downloads) | Downloads read |
| area:36 | Downloads page, badge and tab chrome | Shared |
| area:43 | Reader: enqueue deletes, `addDownloadsToStartOfQueue`, `downloadChapters`, `buildPageList` consumers | Callers into this area |
| area:53 | Notification receiver actions (pause, resume, clear, open manga, delete chapter); channel setup | Shared |
| area:42 | Storage screen uses `getDownloadSize` and `getDownloadCount` | Callers into this area |
| area:47 | Download settings screen | Callers into this area |
