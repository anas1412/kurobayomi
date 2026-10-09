# Area 23 — Defect and quirk register

Every item below was verified by reading the Kotlin at the cited location. "Impact" is what a user can observe. "Rust fix" is the behaviour the port should have; the task that implements it is listed. Items marked **Keep** are deliberate behaviours that the port should preserve, listed here so nobody "fixes" them by accident.

| # | Location (repo-relative, line) | Observed behaviour | Impact | Rust fix | Task |
|---|---|---|---|---|---|
| D-01 | app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadJob.kt (`doWork` loop) | `while (active) { active = ... }` has no suspension or delay | CPU spin for the whole download run | `watch` channels, no polling | T-23a-023 |
| D-02 | MangaDownloadJob.kt (`checkNetworkState`, `doWork`) | Network loss stops the run with ERROR and the job ends with success; nothing restarts it | Downloads silently stop until the user presses Resume | Auto-resume when the user did not pause; network errors return to QUEUE | T-23a-023 |
| D-03 | app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadStore.kt (`restore`) | Rows whose source is not yet registered are dropped and deleted from the store | Lost queue entries after a restart when extensions load late | Gate restore on the source gate; keep rows | T-23a-035 |
| D-04 | MangaDownloadCache.kt (`renewCache`, `withTimeoutOrNull(30.seconds)`) | On timeout, `sources` stays empty, so the index is replaced with an empty map | All downloaded badges disappear for up to an hour | Keep the previous index on timeout | T-23a-006 |
| D-05 | MangaDownloader.kt (`downloadChapter`, `getOrDownloadImage`, `splitTallImageIfNeeded`) | Filename width is `max(3, digits)` (4 for 1000+ pages) but the split prefix is always `%03d`; the prefix `001` matches file `0010` | Wrong page split or deleted for chapters of 1000 or more pages; pages 1000+ never split | One width everywhere; exact prefix match | T-23a-018 |
| D-06 | ImageUtil.kt (`isTallImage`) | `(outHeight / outWidth) > 3` with Int division | Effective threshold is h >= 4w, not the documented 3:1 ratio | Keep the effective rule, document it | T-23a-018 |
| D-07 | MangaDownloader.kt (`copyImageFromCache`) | Unknown cache file type returns the `.tmp` file, which is never renamed | Chapter fails integrity check with no clear message | Fall back to the network | T-23a-016 |
| D-08 | MangaDownloader.kt (`isDownloadSuccessful`) | Counts stray files; a leftover file from another page list fails the chapter forever | Chapter stuck in ERROR | Count page keys | T-23a-020 |
| D-09 | MangaDownloader.kt (`archiveChapter`) | `.cbz_tmp` is left if writing fails; `createFile` on retry is ambiguous | Duplicate or stale archive parts | `.cbz.part` removed on failure, cleanup at startup | T-23a-022, T-23a-044 |
| D-10 | MangaDownloader.kt (`downloadChapter` finalise, `tmpDir.renameTo(chapterDirname)`) | Rename result ignored; chapter marked DOWNLOADED while files stay in `_tmp` | Chapter shows as downloaded but folder is missing | Check result; fail the chapter | T-23a-022 |
| D-11 | MangaDownloader.kt (`downloadChapter`, `DiskUtil.createNoMediaFile(tmpDir, context)`) | Runs after the folder was removed or renamed | `.nomedia` never written | Decide once at the root (data file 4.2) | T-23a-022 |
| D-12 | MangaDownloader.kt (`stop(reason)` early return, lines 168-171) | Skips `isPaused = false` and `MangaDownloadJob.stop` | Later stop shows the wrong notification; job state inconsistent | Always clear | T-23a-011 |
| D-13 | MangaDownloader.kt (`updateQueue`, `addAllToQueue`) | Every reorder sets all queued items to QUEUE, including ERROR | Errors silently retried on the next reorder | Keep ERROR on reorder | T-23a-008 |
| D-14 | MangaDownloader.kt (`addAllToQueue`, `removeFromQueue`, `internalClearQueue`) | Store writes inside `MutableStateFlow.update` lambdas | Side effects repeat on CAS retry | Single actor owns state | T-23a-011 |
| D-15 | MangaDownloader.kt (`launchDownloadJob` on non-cancel exception, lines 264-269) | Any exception outside `downloadChapter`'s try (for example invalid location, `createDirectory(...)!!`) stops the whole run | One bad chapter halts every queued chapter | Fail only that chapter, keep running | T-23a-012 |
| D-16 | MangaDownloader.kt (`downloadImage`, `retryWhen`) | Every exception is retried, including 403 and 404 | Wasted time (about 14 s per failing page) | Do not retry 404 and 410 | T-23a-013 |
| D-17 | MangaDownloader.kt (`throttler` field) | One throttler shared by all downloads, speed read from prefs each time | Limit is global, correct for the setting; but its state is mutated on every call | Keep global limiter; set once per chapter | T-23a-015 |
| D-18 | MangaDownloadManager.kt (`deleteManga`, `removeFromQueue`) | No pause before removing the queue entries of a running manga | Running chapter writes into a deleted folder | Pause, remove, restart | T-23a-027 |
| D-19 | MangaDownloader.kt (`removeFromQueueIf`) and MangaDownloadManager.kt (`removeFromDownloadQueue`) | Removing a running chapter does not pause | Job cancelled mid-write, status NOT_DOWNLOADED, partial files left | Pause first in all removal paths | T-23a-026 |
| D-20 | MangaDownloadManager.kt (`renameChapter`) | Index gets the name with `.cbz` for renamed archives | Index entry does not match scanned name | Store bare names | T-23a-029 |
| D-21 | MangaDownloadManager.kt (`renameSource`) | Index not updated after folder rename | Stale sizes and paths until next invalidation | Rename in index too | T-23a-029 |
| D-22 | MangaDownloadPendingDeleter.kt (`getPendingChapters`) | Prefs cleared before deletion runs; crash loses the entries | Read chapters survive, never retried | Delete rows only after folders are gone | T-23a-028 |
| D-23 | MangaDownloadPendingDeleter.kt (`toModel`) and MangaDownloadManager.kt (`getChaptersToDelete`) | Second filter pass sees read=false and bookmark=false | Settings changed between enqueue and flush are ignored | Re-check at flush with live settings (Proposal) | T-23a-028 |
| D-24 | MangaDownloadNotifier.kt (`onProgressChange`, regex from `chop(15)` title) | Chapter prefix strip uses the truncated title | Long titles keep the manga name in the chapter part | Strip first, then chop (Proposal) | T-23a-019 |
| D-25 | MangaDownloadNotifier.kt (`onError`) | `"$mangaTitle: $chapter"` prints `null` when chapter is null | "Title: null" in the notification | Omit the chapter part | T-23a-019 |
| D-26 | MangaDownloadNotifier.kt (`onError`) | Every page error replaces the previous one (same id -202) | User sees only the last error | Collapse per chapter with a count | T-23a-019 |
| D-27 | MangaDownloadNotifier.kt, Notifications.kt (channels) | Errors on LOW importance | No sound or peek for failures | Keep low for progress; use default for errors (Proposal, decision D-DL-03 in the decision log) | T-23a-019 |
| D-28 | MangaDownloadProvider.kt and DiskUtil.kt (`buildValidFilename`) | `take(240)` counts UTF-16 units, not bytes; MAX_FILE_NAME_BYTES is unused | Names can exceed 255 bytes for some scripts | Cap at 240 bytes, on char boundary (Proposal) | T-23a-003 |
| D-29 | MangaDownloadQueueScreenModel.kt (`onMenuItemClick`, move to top) | Rebuilds the whole queue from header order | A single move reorders all sources | Move within group only (Proposal, decision D-DL-05) | T-23a-025 |
| D-30 | MangaDownloadHolder.kt (`bind`) | Empty progress text before pages are known; ERROR not shown | Blank row while working; errors look like queued items | Show "Downloading…" and an error label (ui-parity 1.3, 1.8) | T-23a-033 |
| D-31 | MangaDownloadItem.kt (`hashCode`) | `chapter.id.toInt()` truncation | Hash collisions for large ids | Full 64-bit id | T-23a-032 |
| D-32 | MangaDownload.kt (`progressFlow`, `progress`) | Average versus sum of page progress across the code | Two numbers for one thing; queue uses the sum | Use the sum everywhere | T-23a-034 |
| D-33 | MangaDownloader.kt (`downloadChapter`, `getOrDownloadImage` line 458) | `getImageUrl` may return an empty string; only `null` is checked, so the empty URL is used as-is | Fails later in the HTTP layer with a generic message | Treat empty URL as an error for that page | T-23a-013 |
| D-34 | MangaDownloader.kt (`getOrDownloadImage`, `it.name!!`) | Force unwrap of file names | Crash on a file with no name | Treat missing name as not matching | T-23a-014 |
| D-35 | MangaDownloader.kt (companion `CHAPTERS_PER_SOURCE_QUEUE_WARNING_THRESHOLD`) and `download_slots` (DownloadPreferences.kt:69) | `download_slots` is not used by the manga downloader; the limit is hard-coded to 5 sources x 1 | Settings screen control has no effect | Decision D-DL-02 | T-23a-037 |
| D-36 | MangaDownloadManager.kt (`startDownloadNow`, `addDownloadsToStartOfQueue`, `Store.restore`) | `runBlocking` on callers, which can be the main thread | UI stalls on slow storage | Async commands | T-23a-030 |
| D-37 | MangaDownloadCache.kt (`UniFileAsStringSerializer`) | Stores the URI string and reopens with `UniFile.fromUri` on load | Index breaks when the storage grant is revoked | Store plain path; check existence on load | T-23a-006 |
| D-38 | MangaDownloadCache.kt (`renewCache` completion handler) | `lastRenew` is set in `invokeOnCompletion`, even when cancelled | A cancelled scan counts as fresh for an hour | Set only after a successful scan | T-23a-006 |

## Keep (deliberate, must survive the port)

| # | Behaviour | Location | Why it stays |
|---|---|---|---|
| K-01 | Folder name = `"<scanlator>_<name>"` sanitised | MangaDownloadProvider.kt:134-142 | Folder compatibility with Aniyomi downloads |
| K-02 | Source folder = `"<name> (<LANG>)"` | HttpSource.kt:103 and MangaDownloadProvider.kt:115 | Same as above |
| K-03 | `"_tmp"` suffix for in-progress folders and `.tmp` for page files | MangaDownloader.kt:766, 382 | Resume compatibility |
| K-04 | Page files named by zero-padded number, extension from type | MangaDownloader.kt:462-463, 579-582 | Reader and folder compatibility |
| K-05 | Chapter order: descending sourceOrder (oldest first in practice) | MangaDownloader.kt:296 | Kept as is; the source has no rationale comment (inferred: oldest first) |
| K-06 | At most 5 sources, 1 chapter per source, 2 pages per chapter | MangaDownloader.kt:219, 390 | User-visible throughput |
| K-07 | Retry schedule 2, 4, 8 seconds | MangaDownloader.kt:539-547 | No rationale in the source; keep for parity |
| K-08 | Min free space 200 MiB | MangaDownloader.kt:773-774 | Source comment: "Arbitrary minimum required space" |
| K-09 | Bookmarked chapters are not deleted unless the setting is on | MangaDownloadManager.kt:424-428 | User data |
| K-10 | Read chapters in excluded categories are not auto-deleted | MangaDownloadManager.kt:418-421 | User data |
| K-11 | Queue size warnings at 30 total and 15 per source | MangaDownloader.kt:768-769 | The warning text says large bulk downloads may slow or block sources |
| K-12 | Errors stay in the queue and are retried on the next explicit start | MangaDownloader.kt:149-150 | Retry without re-adding |
| K-13 | Duplicate chapters in the queue are not added twice | MangaDownloader.kt:298 | Queue sanity |
| K-14 | CBZ archive is STORE (no compression) with UTF-8 names | ZipWriter.kt | No rationale in the source; keep for format parity |
| K-15 | ComicInfo.xml is written before archiving, so it is inside the CBZ | MangaDownloader.kt:417-422 | Metadata for comic readers |
