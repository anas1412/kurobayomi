# 43-reader-core: edge cases, failure modes, races, tests

IDs (`E-…`) are referenced from `p1-behavior.md`, `p1-rust-design.md`, `p1-ui-parity.md` and `p1-tasks.md`. Each entry states the Kotlin behaviour with `path:line`, the risk, and the required Rust behaviour. "Keep" means the Kotlin behaviour is preserved on purpose. "Fix" means the Rust port deliberately differs.

Upstream test files: the scope has no `*Test.kt` file of its own. The only related upstream tests are `domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt` (chapter-gap and missing-count helpers used by the transition card) and `domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt` (chapter number parsing, area:07-domain-entries-items). Both are ported in section 3.

---

## 1. Loader and chapter state

### E-LEAK-1 Loader overwritten on retry after a failed page list (Fix)
- Kotlin: `chapter.pageLoader = loader` is assigned BEFORE `getPages()` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ChapterLoader.kt:48-51`). If `getPages()` throws, the loader stays on the chapter with state `Error`. A retry creates a second loader and overwrites the first without `recycle()`. `HttpPageLoader` starts a consumer in `init` (`HttpPageLoader.kt:54-64`), so the old consumer thread remains parked forever.
- Rust: assign the loader only after `get_pages()` succeeds (`p1-rust-design.md` 3.7). On failure, `loader = None`.
- Test: force `get_pages` to fail twice; assert the first loader was dropped (use a drop counter).

### E-LEAK-2 Online loader orphaned on downloaded-chapter preload (Fix)
- Kotlin: `preload` sets `chapter.state = Wait` for an online chapter that is now downloaded (`ReaderViewModel.kt:392-404`) without recycling `pageLoader`. `loader.loadChapter` then replaces `pageLoader` (`ChapterLoader.kt:49`). The old `HttpPageLoader` is orphaned as in E-LEAK-1.
- Rust: `set_loader` always drops (recycles) the previous loader before storing the new one.
- Test: preload an online chapter, mark it downloaded, preload again; assert the first loader was recycled.

### E-CANCEL-1 Cancellation stored as an error (Fix)
- Kotlin: `ChapterLoader.loadChapter` catches `Throwable` (including `CancellationException`) and stores `State.Error(e)` (`ChapterLoader.kt:64-67`).
- Effect: a navigation that is cancelled mid-load leaves the chapter in `Error`, and the UI shows an error for a load the user abandoned.
- Rust: map cancellation to `ChapterState::Wait`, never `Error`.
- Test: cancel a load in progress; assert state is `Wait`.

### E-START-1 Stale requested page for a read chapter (Keep, documented)
- Kotlin: when the chapter is read and `preserve_reading_position` is false, `requestedPage` is not set (`ChapterLoader.kt:59-61`), so it keeps the previous value (default 0). Re-opening a read chapter after the user had scrolled in the same session therefore starts at the old value.
- Rust: same rule (`requested_page` unchanged). Test: read chapter opened twice; second open uses the value from the first (documented, not "fixed").

### E-REF-1 Reference count can go negative (Keep, with assert)
- Kotlin: `unref()` decrements and recycles only at `== 0` (`ReaderChapter.kt:33-43`). A negative count never recycles again (the chapter is never reset).
- Rust: `release` keeps the `== 0` rule, and debug builds `assert!(refs >= 0)`.
- Test: double release in debug build panics; in release build the state is still reset once.

### E-TR-1 ChapterTransition equality and hash (Fix)
- Kotlin: `equals` ignores the subclass (`Prev(a,b) == Next(a,b)`); `hashCode` is not symmetric with the swapped-pair `equals` (`ChapterTransition.kt:18-30`).
- Rust: equality includes the variant and the unordered pair; hash is derived from the same canonical key. Document the change in the release notes (no caller relied on subclass equality in this scope).
- Test: `Prev(a,b) != Next(a,b)`; `Prev(a,b) == Prev(b,a)` is FALSE (direction matters for the card), `Prev(a,b) == Prev(a,b)` true.

Note: the Kotlin swapped-pair equality would make `Prev(a,b) == Prev(b,a)` true. Rust keeps direction as part of the key. This is a deliberate fix (it is a conscious deviation from the Kotlin rule).

### E-START-2 requested page for an unread chapter after restore (Keep)
- Kotlin: `init` block (`ReaderViewModel.kt:228-243`) sets `requestedPage = chapterPageIndex` when a saved index exists, else `last_page_read` when unread. Read chapters get `requestedPage` from the loader rule (E-START-1).
- Rust: same precedence.

---

## 2. HTTP page loader

### E-HTTP-1 Evicted READY page is re-queued only when visited (Keep)
- Kotlin: `loadPage` re-queues a READY page whose image is no longer in the cache (`HttpPageLoader.kt:94-101`). Pages that are not visited are not checked.
- Rust: same. Test: evict an image file, call `load_page`, assert it is re-downloaded.

### E-HTTP-2 Manual retry enqueues even non-error pages (Keep)
- Kotlin: `retryPage` offers the entry at priority 2 regardless of the page status, and the worker drops it unless the status is QUEUE (`HttpPageLoader.kt:128-133`, `:55-63`).
- Rust: same filter at dequeue. Test: retry a READY page; assert no download happens.

### E-HTTP-3 Duplicate queue entries for the same page (Keep)
- Kotlin: a page can be enqueued as priority 1 (visible) and again as priority 0 (preload) by different calls. Only the first dequeue loads it; later dequeues see a non-QUEUE status and drop it (`:55-63`).
- Rust: same. Test: enqueue a page twice; exactly one fetch.

### E-HTTP-4 Cache key uses the original URL, not the data-saver URL (Keep)
- Kotlin: `chapterCache.putImageToCache(imageUrl, ...)` uses the original `imageUrl` local (`HttpPageLoader.kt:194-197`), while the request uses the rewritten URL (`DataSaver.kt:30-41`).
- Rust: same key rule. Test: switching data saver modes does not invalidate cached originals.

### E-HTTP-5 Page list persisted only at recycle (Keep, with improvement note)
- Kotlin: the network page list is written only in `recycle()` (`HttpPageLoader.kt:141-156`). A reader closed by a crash loses it.
- Rust: write the page list as soon as `get_pages` succeeds from the network (Fix: it is a cache write, not a close action). Documented deviation.
- Test: kill after `get_pages`; reopen; no network call for the page list.

### E-HTTP-6 Non-cancellation errors are swallowed (Keep)
- Kotlin: `internalLoadPage` sets `ERROR` and does not rethrow non-cancellation errors (`HttpPageLoader.kt:202-207`). Errors are not logged.
- Rust: keep the status change; log at warn level (no silent failures). Test: a failing image leaves the status `Error` and the worker continues with the next page.

### E-HTTP-7 Worker lifetime (Fix)
- Kotlin: the consumer starts in `init` and lives until `recycle` (`HttpPageLoader.kt:54-64`), even for a chapter that is only queried for its page list.
- Rust: start the worker lazily on the first `load_page` and stop it on `Drop`.
- Test: `get_pages` alone starts no worker task.

### E-HTTP-8 Queue removal semantics (Keep, implemented lazily)
- Kotlin: on cancellation, still-QUEUE entries are removed from the priority queue (`HttpPageLoader.kt:116-120`). The page keeps status QUEUE and will be re-queued by a later `loadPage`.
- Rust: cancellation flag on the entry; the worker skips it. Same observable result.
- Test: visible page cancelled, then visited again; downloaded once.

### E-HTTP-9 Preload only forward, in-chapter (Keep)
- Kotlin: `preloadNextPages` takes at most 4 pages after the current one within the same chapter (`HttpPageLoader.kt:164-178`). Cross-chapter preload is the view model's job (`preload`).
- Rust: same. Test: last page queues no preload; page n-2 of n queues one.

### E-IMG-1 Extension match is case-sensitive; `.jpeg` relies on sniffing (Keep, with stated improvement)
- Kotlin: `ImageUtil.isImage` compares the text after the last dot exactly with `jpg`, `png`, ... (`ImageUtil.kt:39-44`, `:106-114`). `.JPG` and `.jpeg` are not extension matches and are only accepted when the magic bytes match.
- Rust: keep the same list. Improvement (documented): compare case-insensitively and also accept `jpeg`. Tests: `001.JPG` accepted, `001.jpeg` accepted, `001.txt` rejected unless its bytes are an image.

### E-IMG-2 Notification bitmap decode disables memory cache (Keep)
- Kotlin: `memoryCachePolicy(DISABLED)` and `size(720, 1280)` (`SaveImageNotifier.kt:36-44`). Rust: decode once to a 720x1280 bound and drop after the notification is sent.

---

## 3. Sorting, filtering, duplicates, gaps

### E-SORT-1 Natural and collation differences (Fix, documented)
- Kotlin: `CaseInsensitiveSimpleNaturalComparator` (`StringExtensions.kt:37-40`) and `compareToWithCollator` (`ChapterSorter.kt:25-26`).
- Rust: `natord::compare_ignore_case` and ICU collation. They match on ASCII and common Latin input; they can differ on exotic Unicode and on leading zeros in some locales. Test cases below pin the common inputs.
- Test vectors (natural): `["10.jpg","2.jpg","1.jpg"]` -> `["1.jpg","2.jpg","10.jpg"]`; `["Page 2","page 10","PAGE 1"]` -> `["PAGE 1","Page 2","page 10"]`.

### E-SORT-2 SOURCE sort is reversed when sortDescending is false (Keep, named)
- Kotlin: `getChapterSort(manga, false)` for `SOURCE` compares `c2.sourceOrder` against `c1.sourceOrder` (`ChapterSorter.kt:12-14`), i.e. descending `sourceOrder`. The source lists newest first, so the reader shows oldest first.
- Rust: same, documented as "oldest first in reader". Test: `source_order` [0,1,2] (0 = newest) -> reader list [2,1,0].

### E-SORT-3 Invalid sort mode throws (Fix)
- Kotlin: `else -> throw NotImplementedError(...)` (`ChapterSorter.kt:28`) crashes the reader if the stored mode is corrupt (only bits 8..9 are used, so the values 0x000/0x100/0x200/0x300 cover every combination; the throw is unreachable for the current mask but the Kotlin code keeps it).
- Rust: all four values handled; any other value is impossible by masking. Test: each of the 4 modes produces the documented order.

### E-LIST-1 Current chapter re-appended when filtered out (Keep)
- Kotlin: `if (filteredChapters.any { it.id == chapterId }) filteredChapters else filteredChapters + listOf(selectedChapter)` (`ReaderViewModel.kt:196-200`). The appended chapter goes to the END before sorting.
- Rust: same. Test: skip-read on, current chapter read -> still in list, sorted in place.

### E-LIST-2 Downloaded-only removes the requested chapter (Fix)
- Kotlin: `filterDownloadedChapters` (`ChapterFilterDownloaded.kt:13-26`) runs after the current chapter was ensured, so an undownloaded current chapter is removed. `init` then calls `first { chapterId == it.chapter.id }` and throws `NoSuchElementException` (`ReaderViewModel.kt:288`), shown as the raw text "Collection contains no element matching the predicate." (E-INIT-1).
- Rust: `build_reader_list` returns `ListError::CurrentChapterMissing`; the UI shows "The chapter is not downloaded. Turn off downloaded-only mode or download it first." (new message, PROPOSAL, flagged for strings) and offers Retry. Test: downloaded-only with the current chapter undownloaded.

### E-DEDUPE-1 Unknown chapter numbers collapse under skip-dupe (Keep, documented)
- Kotlin: `groupBy { it.chapterNumber }` puts every `-1.0` chapter in one group (`ChapterRemoveDuplicates.kt:8-15`). With skip-duplicates on, only one unnumbered chapter remains in the reader.
- Rust: keep the grouping rule exactly (the user has the option off by default). Warn in the setting's description (PROPOSAL, UI text). Test: three `-1.0` chapters -> one entry.

### E-DUP-1 Duplicate marking ignores chapters removed by skip-dupe (Keep, documented)
- Kotlin: `updateChapterProgressOnComplete` iterates `chapterList`, which already had duplicates removed (`ReaderViewModel.kt:563-575`). Chapters hidden by skip-dupe are never marked read.
- Rust: same. Test: skip-dupe on, two chapters numbered 5, completing one: the other is not marked.

### E-DUP-2 Float equality on chapter numbers (Keep)
- Kotlin: `chapter.chapter_number == readerChapter.chapter.chapter_number` on `Float` (`ReaderViewModel.kt:569`).
- Rust: compare as `f32`. Test: 5.0 and 5.0 equal; 5.0 and 5.000001 not equal (Float precision, documented).

### E-GAP-1 Gap formula (Keep, ported tests)
- Kotlin: `floor(h) - floor(l) - 1` with a `< 0` guard (`MissingChapters.kt:46-48`). A negative result (e.g. 5.0 vs 5.5 gives -1) is returned, and the caller shows the warning only when `> 0` (`P/ChapterTransition.kt:120`).
- Rust: ported vectors from `MissingItemsTest.kt`:
  - `calculateChapterGap(10.0, 9.0) == 0`; `(10.0, 8.0) == 1`; `(10.0, 8.5) == 1`; `(10.0, 1.1) == 8`.
  - `(-1.0, 10.0) == 0`; `(99.0, -1.0) == 0`.
  - `missingChaptersCount([-1.0, 1.0, 2.0, 2.2, 4.0, 6.0, 10.0, 10.0]) == 5`; `[1.0, 1.0, 1.1, 1.5, 1.6, 1.99] == 0`; `[-1,-1,-1] == 0`; `[] == 0`.

### E-GAP-2 Chapter gap with a missing neighbour (Keep)
- Kotlin: `calculateChapterGap(higher: Chapter?, lower: Chapter?)` returns 0 when either is null (`MissingChapters.kt:40-44`). The transition card then shows only the fallback card.
- Rust: same.

---

## 4. Initial load, navigation, preload

### E-INIT-1 Initial failure closes the reader (Fix)
- Kotlin: `setInitialChapterError` logs, `finish()`s and toasts `error.message` (`ReaderActivity.kt:629-633`). The user loses the reader state and must navigate back.
- Rust: stay in the reader with an error card and a Retry button (`p1-ui-parity.md` section 1). The message text is the same.
- Test: failing page list shows the card; Retry calls `load_init` again.

### E-INIT-2 See E-LIST-2 (downloaded-only)

### E-INIT-3 Missing manga returns "Unknown err" (Keep, localise)
- Kotlin: `init` returns `success(false)` for a missing manga, and the activity builds `IllegalStateException("Unknown err")` (`ReaderActivity.kt:178-181`). The literal is not localised.
- Rust: `LoadError::UnknownManga` with the same English text, localised through `strings.rs`.

### E-INIT-4 Requested chapter absent from the list (Keep, message)
- Kotlin: `error("Requested chapter of id $chapterId not found in chapter list")` (`ReaderViewModel.kt:158-159`). The message is developer text and is not localised.
- Rust: `ListError::CurrentChapterMissing` with a generic user message; developer detail goes to the log.

### E-NAV-1 Failed adjacent load then jump to page 0 (Fix)
- Kotlin: `loadAdjacent` swallows errors (`ReaderViewModel.kt:363-381`); `loadNext` returns normally (`:608-611`); the activity then calls `moveToPageIndex(0)` (`ReaderActivity.kt:664-669`), which moves to page 0 of the STILL-CURRENT chapter.
- Rust: `load_adjacent` returns `Err`; the UI shows a toast and does not move. Test: adjacent load failure keeps the current page index.

### E-NAV-2 Previous chapter opens at page 0 (Keep, documented)
- Kotlin: `loadPreviousChapter` also moves to page 0 (`ReaderActivity.kt:675-680`). A user who expects the last page of the previous chapter gets the first.
- Rust: keep the Kotlin default (page 0) and make it a setting in the tasks as a follow-up (PROPOSAL). Test: default behaviour is page 0.

### E-NAV-3 Chapter absent from the list yields odd neighbours (Keep, unreachable normally)
- Kotlin: `indexOf` returns -1; prev = `getOrNull(-2)` = null; next = `getOrNull(0)` (`ReaderViewModel.kt:313-318`).
- Rust: assert the chapter is in the list when building `ViewerChapters`; on absence, return an error.

### E-PRE-1 Preload failure is silent and sends no event (Keep, with retry on next visit)
- Kotlin: `preload` returns silently on error and leaves `Error` (`ReaderViewModel.kt:411-421`). A later load retries because `Error` is retryable.
- Rust: same, plus a debug log. Test: preload fails; visiting the chapter loads it.

### E-EVT-1 Rendezvous channel drops events (Fix)
- Kotlin: `Channel<Event>()` (`ReaderViewModel.kt:111`) with `trySend` (`:421`, `:459`). `trySend` drops the event unless a receiver is suspended at that instant. Dropped `ReloadViewerChapters` means the viewer keeps stale chapters; dropped `PageChanged` skips the flash.
- Rust: unbounded `async_channel`; no drops. Test: send 1000 events while the receiver is busy; all 1000 are received in order.

### E-DLA-1 Download-ahead details (Keep, documented)
- `auto_download_while_reading` is read once at view-model construction (`ReaderViewModel.kt:226`). Changing it while reading has no effect until the next open.
- The trigger uses the 1-based page number (`page.number / pages.size > 0.25`, `:454`).
- `getNextChapters.await(manga.id, nextChapter.id)` returns unread chapters from the start of the manga when the next chapter itself is read (`GetNextChapters.kt`, `indexOfFirst` returns -1 and `max(0, -1) = 0`). The queue can then start far before the reader position.
- Rust: same trigger and threshold; FIX: start from the next chapter even when it is read (documented deviation), with a test.

### E-DLA-2 Download-ahead only from a downloaded current chapter (Keep)
- Kotlin: `if (getCurrentChapter()?.pageLoader !is DownloadPageLoader) return` (`ReaderViewModel.kt:467`).
- Rust: same.

---

## 5. Progress, completion, history, deletion

### E-PROG-1 Error pages do not save progress (Keep)
- Kotlin: `if (!incognitoMode && page.status != Page.State.ERROR)` (`ReaderViewModel.kt:537`). `requestedPage` and `chapterPageIndex` are still updated (`:534-535`).
- Rust: same. Test: page in Error -> no `Progress` command; `requested_page` updated.

### E-PROG-2 Completion re-runs on every visit to the last page (Keep, documented)
- Kotlin: `updateChapterProgressOnComplete` runs whenever the last page is selected, including when already read (`ReaderViewModel.kt:540-542`). Track update, delete-after-read and duplicate marking run again.
- Rust: keep the behaviour (idempotent writes). Test: visiting the last page twice sends the track command twice (documented).

### E-PROG-3 Read flag is never cleared (Keep)
- Kotlin: nothing sets `read = false` in this scope; `ChapterUpdate` sends `read = chapter.read` (`ReaderViewModel.kt:544-550`).
- Rust: same. Mark-as-unread is in the list screens (area:38-ui-entry-details-manga and area:08-domain-library-category-history-updates), not here.

### E-DEL-1 Negative remove-after-read slots other than -1 (Fix)
- Kotlin: `chapterList.getOrNull(currentChapterPosition - removeAfterReadSlots)` (`ReaderViewModel.kt:514`). With `-2` the target is two chapters ahead; only `-1` is "off" (`:510`).
- Rust: any value `< 0` is treated as "off". The Kotlin code only special-cases `-1` (`:510`); whether the settings UI can store other negative values was not checked in this scope, so the Rust rule is the defensive choice.
- Test: `-2` does not delete a forward chapter.

### E-DEL-2 Slot 0 deletes the chapter just read (Keep, documented)
- Kotlin: slots 0 -> target = current chapter, which is read at completion, so it is queued for deletion on leave (`ReaderViewModel.kt:514`, `:934-944`).
- Rust: same. Test: slots 0 -> deletion queued for the current chapter when completed.

### E-DEL-3 Pending download not cleared when slots = -1 (Keep)
- Kotlin: returns before `chapterToDownload = null` (`ReaderViewModel.kt:510` before `:517`). A queued-then-cancelled download (from `cancelQueuedDownloads`, `:497-500`) is restored on `onCleared`.
- Rust: same rule.

### E-DEL-4 Deletion deferred until leaving the reader (Keep)
- Kotlin: queued deletes run in `onActivityFinish` (`ReaderViewModel.kt:259-261`, `:950-954`). A crash before leaving keeps the queue (download manager persists it).
- Rust: same. Test: queue survives restart.

### E-HIST-1 Duplicate history rows from consecutive flushes (Keep)
- Kotlin: `updateHistory` upserts even when `chapterReadStartTime` is null, with duration 0 (`ReaderViewModel.kt:599-603`). `onPause` and `loadNewChapter` can flush twice in a row.
- Rust: the upsert is idempotent by `chapter_id`; the second flush writes duration 0 and must NOT overwrite a non-zero duration (Fix: `time_read = max(old, new)` plus `last_read = new`). Test: pause then page change keeps the total.

### E-HIST-2 Race on the read-start timestamp (Fix)
- Kotlin: `chapterReadStartTime` is written on the main thread (`restartReadTimer`) and read/cleared on IO (`updateHistory`) with no synchronisation (`ReaderViewModel.kt:146`, `:579-603`).
- Rust: single owner on the main thread; the duration is computed before the command is sent.

### E-INC-1 Incognito gates writes but not state keys (Keep)
- Kotlin: `chapterPageIndex` is saved even in incognito (`ReaderViewModel.kt:535`), while DB progress, history and tracking are skipped (`:537`, `:595`, `:919`).
- Rust: same split. The saved restore keys are not persisted across restarts in incognito (Fix: store them in memory only when incognito). Test: incognito session -> no `reader_session` row written.

### E-INC-2 Turning incognito off closes the reader (Keep)
- Kotlin: `preferences.incognitoMode().changes().drop(1).onEach { if (!it) finish() }` (`ReaderActivity.kt:192-196`).
- Rust: close the reader page (return to the list) on the same change. Test: toggling off emits a close event.

### E-TRACK-1 Track auto-update checks only `autoUpdateTrack` (Open question)
- Kotlin: the reader checks `trackPreferences.autoUpdateTrack()` (`ReaderViewModel.kt:920`). `TrackChapter.await` does not check `autoUpdateTrackOnMarkRead` (`app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt:24-35`).
- Consequence: if `autoUpdateTrackOnMarkRead` is meant to gate this path, the reader ignores it. The correct behaviour is not decided in this scope. Owner: area:27-tracking-core. Rust must implement the decision made there; until then implement the Kotlin behaviour and note it in `p1-tasks.md` (task T-43a-030 depends on area:27-tracking-core).

### E-TRACK-2 Track only for chapters above last read (Keep)
- Kotlin: `TrackChapter.await` ignores trackers where `chapterNumber <= track.lastChapterRead` (`TrackChapter.kt:30-31`).
- Rust: area:27-tracking-core implements; the reader passes the chapter number.

---

## 6. Actions: save, share, copy, cover

### E-SAVE-1 Double feedback on save (Keep, flagged for review)
- Kotlin: a notification ("Picture saved", `SaveImageNotifier.kt:69-90`) AND a toast ("Picture saved", `ReaderActivity.kt:757-766`).
- Rust: notification only (desktop shells show notifications reliably). PROPOSAL: documented change.

### E-SAVE-2 Filename collisions (Open)
- Kotlin: the filename is `"<manga> - <chapter> - <page>"` (`ReaderViewModel.kt:754-766`) with no uniqueness check in this scope. The collision behaviour of `ImageSaver` was not read in this session (owner: area:13-core-util-storage-archive-torrent).
- Rust: on collision append ` (2)`, ` (3)`, ... (PROPOSAL). Test: two saves of the same page name produce two files.

### E-SHARE-1 Dead try/catch around share (Fix)
- Kotlin: `shareImage` wraps `launchNonCancellable { ... }` in `try/catch`, which cannot catch exceptions from the coroutine (`ReaderViewModel.kt:852-877`). Exceptions reach the `viewModelScope` handler and crash the app.
- Rust: errors are sent as an event and shown as a toast. Test: a failing save shows an error toast and the app keeps running.

### E-CLIP-1 Clipboard copy of an image (Keep, Linux API)
- Kotlin: `ClipData.newUri(contentResolver, "", uri)` (`ReaderActivity.kt:747-751`). No toast.
- Rust: `gdk::Clipboard::set_texture` (PNG in memory). Test: clipboard holds an image MIME type.

### E-COVER-1 Cover written before the library check (Keep, documented)
- Kotlin: `editCover` runs before the `isLocal() || favorite` check (`ReaderViewModel.kt:889-894`). A non-library manga gets a new cover file and then the "add to library first" message.
- Rust: same order, documented. Test: the file exists after the message.

### E-COVER-2 Only `Exception` is caught (Fix)
- Kotlin: `catch (e: Exception)` (`ReaderViewModel.kt:896-898`). An `Error` (e.g. out-of-memory during a large decode) escapes and crashes the coroutine.
- Rust: convert every failure to `SetCoverResult::Error`. Test: decode failure shows "Failed to update cover".

### E-URL-1 Chapter URL errors are silent (Keep)
- Kotlin: `getChapterUrl` returns null on any exception (`ReaderViewModel.kt:630-640`); the Share and Open entries then do nothing (`ReaderActivity.kt:577-597`).
- Rust: show a toast "Unknown error" instead of doing nothing (PROPOSAL). Test: failing URL builder shows a toast.

### E-PATH-1 Filename truncation (Keep)
- Kotlin: title and chapter truncated to `250 - suffix bytes` before `buildValidFilename` (`ReaderViewModel.kt:754-766`). `takeBytes` cuts on a UTF-8 boundary and removes U+FFFD (`StringExtensions.kt:50-58`).
- Rust: same byte rule. Test: a 300-byte multi-byte title is cut without a broken character.

### E-IMG-3 Page stream after cache eviction (Fix)
- Kotlin: `page.stream = { chapterCache.getImageFile(imageUrl).inputStream() }` (`HttpPageLoader.kt:200`). If the file is evicted after READY, opening the stream throws at display time.
- Rust: `open` returns an error that sets the page to `Queued` and re-enqueues it at priority 1. Test: delete the cache file, open the page; it is re-downloaded.

### E-DL-1 Downloaded directory page with missing URI (Keep, Fix on Linux)
- Kotlin: `openInputStream(page.uri ?: Uri.EMPTY)!!` (`DownloadPageLoader.kt:66-68`) throws if `uri` is null, at display time.
- Rust: path-based; a missing file marks the page `Error` and the viewer shows the retry card. Test: missing file in download dir.

---

## 7. Settings and UI state

### E-UI-1 Current page -1 before the first page (Fix)
- Kotlin: `currentPage` starts at -1 (`ReaderViewModel.kt:962`); the Slider gets a value outside `1..totalPages` (`ChapterNavigator.kt:115-126`).
- Rust: slider hidden until the first page (the UI-parity rule). Test: no slider value update before the first page event.

### E-UI-2 runBlocking on the UI thread (Fix)
- Kotlin: `setMangaReadingMode` runs `runBlocking(Dispatchers.IO)` containing a rendezvous `send` (`ReaderViewModel.kt:681-703`). If the event collector is busy, the UI thread and the collector deadlock.
- Rust: no blocking call on the UI thread. Test: a change while the event queue is busy completes.

### E-UI-3 Split and invert bindings in the paged settings (Fix, real bug)
- Kotlin: "Split wide pages" bound to `dualPageInvertPaged()` (`ReadingModePage.kt:120-124`); "Invert split page placement" bound to `dualPageRotateToFitInvert()` (`:126-131`). The real split key `pref_dual_page_split` is never written from this dialog. The settings screen (`SettingsReaderScreen.kt:213`) reads it, so the two screens disagree.
- Rust: "Split wide pages" -> `dual_page_split_paged`; "Invert split page placement" -> `dual_page_invert_paged`; "Flip orientation" -> `dual_page_rotate_to_fit_invert`. Test: toggling each control changes only its own key.

### E-UI-4 Reading-mode toast condition is inverted (Keep, documented)
- Kotlin: on creation the toast shows when `show_reading_mode` is on (`ReaderActivity.kt:555-557`); on change the toast shows only when it is OFF (`:470-472`).
- Rust: keep both rules (documented in the settings help).

### E-UI-5 Page-actions sheet after a chapter change (Fix)
- Kotlin: `closeDialog()` closes any open dialog when a chapter load finishes (`ReaderViewModel.kt:792-794`, `ReaderActivity.kt:641-647`), including the page-actions sheet opened on a page that is no longer current.
- Rust: close the sheet only when its page is no longer loaded. Test: page-actions open, chapter loads, the sheet stays if its page still exists.

### E-UI-6 Chip and sheet apply semantics differ (Keep)
- Kotlin: the bottom reading-mode and rotation sheets apply on "Apply" (`ReadingModeSelectDialog.kt:44-53`); the settings-tab chips apply immediately (`ReadingModePage.kt:37-60`).
- Rust: same split (documented in the parity file).

### E-UI-7 Icons not mirrored in RTL (Fix, visible)
- Kotlin: left button always `SkipPrevious`, right always `SkipNext`, in RTL too (`ChapterNavigator.kt:83-88`, `:140-145`).
- Rust: mirror the glyph so the icon matches the action. Test: RTL snapshot.

### E-UI-8 Overlay consumes no touches but fades on any touch (Keep)
- Kotlin: `onTouchEvent` calls `performClick()` for every event and returns `super` (`ReaderNavigationOverlayView.kt:109-113`).
- Rust: overlay does not take input (`can_target = false`) and fades on the first page change or any key. Test: overlay does not block clicks.

### E-FLASH-1 Flash flag stale when disabled (Fix)
- Kotlin: `flash()` sets the flag on every `PageChanged` even when the flash feature is off and the composable is absent (`DisplayRefreshHost.kt:36-41`, `ReaderActivity.kt:224-226`, `:433-437`). The flag stays `true`, so enabling the feature later flashes immediately.
- Rust: `on_page_changed` is called only when the feature is on. Test: disable, change pages, enable -> no immediate flash.

### E-FLASH-2 Flash during WHITE mode (Keep)
- Kotlin: WHITE keeps white for the full duration; WHITE_BLACK changes to black after half (`DisplayRefreshHost.kt:61-79`).
- Rust: same. Test: colour at 25 percent and 75 percent of the duration.

### E-BRIGHT-1 Custom brightness subscriptions leak and apply when disabled (Fix)
- Kotlin: each enable launches another `customBrightnessValue().changes().sample(100)` subscription that is never cancelled (`ReaderActivity.kt:937-946`); after disabling, the old subscription still applies new values (`:939-942`).
- Rust: one subscription owned by the session, dropped when the feature is disabled. Test: disable, change the value, brightness is unchanged.

### E-BRIGHT-2 Positive brightness on Linux (Fix, partial)
- Kotlin: sets window brightness (`ReaderActivity.kt:958-965`).
- Rust: sysfs backlight when writable; otherwise toast "Custom brightness is not available on this display" (PROPOSAL text). Test: no writable backlight -> toast and no panic.

### E-ORI-1 Orientation values stored with no effect (Drop, documented)
- Kotlin applies `requestedOrientation` (`ReaderActivity.kt:785-790`).
- Rust: the value is stored and synced; the rotation sheet shows the note "Rotation has no effect on this platform" (PROPOSAL). Test: stored value round-trips through backup.

### E-ORI-2 Orientation only applied when different (Keep)
- Kotlin: `if (newOrientation.flag != requestedOrientation)` (`ReaderActivity.kt:787-789`).
- Rust: not applicable (no orientation).

### E-STATE-1 Side effects inside state-update lambdas (Fix)
- Kotlin: `loadChapter` performs `ref`, `unref`, and `cancelQueuedDownloads` inside `mutableState.update { ... }` (`ReaderViewModel.kt:321-331`). `update` can re-run the lambda under contention, repeating the side effects.
- Rust: compute the new `ViewerChapters` and effects first, then one assignment. Test: concurrent updates produce one `ref` per chapter.

### E-BMK-1 Bookmark optimistic update without rollback (Keep, Fix on failure)
- Kotlin: the in-memory chapter and the UI state change before the DB write; a failed write is only logged (`ReaderViewModel.kt:645-664`).
- Rust: on write failure, revert the UI and show a toast "Unknown error". Test: DB error reverts the icon.

### E-KEY-1 N and P on key up only (Keep)
- Kotlin: `onKeyUp` handles N and P (`ReaderActivity.kt:308-317`); `dispatchKeyEvent` gives the viewer the first look (`:322-325`).
- Rust: handle on key press, after the viewer.

### E-RESTORE-1 Crash restore (Decision recorded)
- Kotlin restores `chapter_id` and `page_index` on process death (`ReaderViewModel.kt:123-136`).
- Rust: restore only when the reader is reopened for the same manga within 24 hours and the setting `restore_after_crash` is on (default on). The 24-hour window is a PROPOSAL; the Kotlin app has no window.

### E-TOAST-1 Null error message (Keep, documented)
- Kotlin: `toast(error.message)` with a nullable message (`ReaderActivity.kt:632`); the helper's null behaviour is outside this scope.
- Rust: always a non-empty message ("Unknown error" when absent).

---

## 8. Concurrency and memory

### E-CONC-1 Background coroutine outlives the page (Fix)
- Kotlin: `load_page` suspends forever until cancelled; the viewer must cancel it (`HttpPageLoader.kt:114-122`). A missed cancel leaks queue entries.
- Rust: the guard is dropped when the widget is unmapped; test: unmap a widget, its queued pages are cancelled.

### E-CONC-2 Image size vs memory (Fix)
- Kotlin: full-size bitmap decode for every page, handled by the viewer (area:44-reader-viewers).
- Rust: decode to viewport size; test: a 6000x4000 page yields a frame no larger than the viewport bound.

### E-CONC-3 Single consumer throughput (Keep)
- Kotlin: one image at a time per chapter (`HttpPageLoader.kt:55-63`). Preloading 4 pages takes 4 sequential downloads.
- Rust: one worker per loader (same), but the tokio runtime may run several chapters' workers in parallel. Test: two chapters do not block each other.

### E-CACHE-1 Eviction during read (Keep, Fix on open)
- Kotlin: DiskLruCache with 100 MiB; eviction of an image in use causes E-IMG-3.
- Rust: E-IMG-3 handling plus pinning of the current chapter's files (PROPOSAL).

---

## 9. Platform mapping risks

| Risk | Where | Mitigation |
|---|---|---|
| `gdk::Texture` created off the main thread | image pipeline | create only in the GTK main loop (P-1, P-8) |
| `glib::spawn_future_local` used from tokio | session | channel from tokio to main loop only (P-2) |
| SQLite on the UI thread | progress | DB actor thread (P-9) |
| RAR library licence/linking | archive | feature flag `rar`, default off until decided |
| No sensor orientation | rotation | stored only (E-ORI-1) |
| Notification action availability | notify | fall back to a plain notification without buttons if the shell does not support actions (checked at runtime) |
| Share target list absent | share | open or show-in-folder instead (documented loss) |

---

## 10. Test plan (new tests to write)

Pure logic (no GTK, `cargo test`):

| Test | Covers |
|---|---|
| `missing_count_empty`, `missing_count_unknown_only`, `missing_count_repeated`, `missing_count_example_5` | MissingItemsTest.kt (`missingChaptersCount`) |
| `chapter_gap_difference`, `chapter_gap_invalid` | MissingItemsTest.kt (`calculateChapterGap`), vectors in E-GAP-1 |
| `sort_source_reversed_when_ascending` | E-SORT-2 |
| `sort_number_unknown_first` | `-1.0` before `0.0` |
| `sort_upload_date_asc` | UPLOAD_DATE |
| `sort_alphabet_collation` | E-SORT-1 vectors |
| `list_skip_read_keeps_current` | E-LIST-1 |
| `list_skip_filtered_unread_only` | filter arms |
| `list_skip_filtered_downloaded_arms` | downloaded arms (both) |
| `list_skip_filtered_bookmark_arms` | bookmark arms (both) |
| `list_dedupe_keeps_current_id_then_scanlator_then_first` | E-DEDUPE-1 and the 3-step rule |
| `list_dedupe_unknown_numbers_collapse` | E-DEDUPE-1 |
| `list_downloaded_only_missing_current` | E-LIST-2 |
| `list_order_is_ascending_for_every_mode` | ascending default |
| `ref_release_zero_crossing_resets_state` | E-REF-1 |
| `ref_negative_asserts_in_debug` | E-REF-1 |
| `viewer_chapters_replace_keeps_shared` | ref-before-unref |
| `transition_equality_direction` | E-TR-1 |
| `mask_set_flag_preserves_other_bits` | bit layout (data file 7.1) |
| `reading_mode_fromPreference_unknown_is_default` | 6 and 7 decode to DEFAULT |
| `orientation_flag_values` | 0x8..0x30 table |
| `progress_skips_error_pages` | E-PROG-1 |
| `progress_completion_idempotent_writes` | E-PROG-2 |
| `delete_slots_negative_other_than_minus_one` | E-DEL-1 |
| `delete_slots_zero_deletes_current` | E-DEL-2 |
| `history_duration_not_overwritten_by_zero` | E-HIST-1 |
| `flash_trigger_interval_counting` | flash rule |
| `flash_colour_white_black_halves` | E-FLASH-2 |
| `prio_queue_order_and_fifo` | E-HTTP-3, priority 2 > 1 > 0 |
| `prio_queue_cancel_skips` | E-HTTP-8 |
| `page_status_filter_on_dequeue` | E-HTTP-2 |
| `natural_sort_filenames` | E-SORT-1 vectors |
| `is_image_name_case_rules` | E-IMG-1 |
| `sniff_jpeg_without_extension` | E-IMG-1 |
| `takeBytes_utf8_boundary` | E-PATH-1 |
| `filename_suffix_and_limit` | E-PATH-1 |

Loader (fixtures in `tests/fixtures/reader/`):

| Fixture | Test |
|---|---|
| `dir_natural/` with `2.jpg`, `10.jpg`, `1.JPG`, `notes.txt`, subfolder | order `1.JPG, 2.jpg, 10.jpg`; txt ignored; subfolder ignored |
| `archive.cbz` (zip with two images and `__MACOSX/` junk) | only images; order natural |
| `empty.cbz` | `PageListEmpty` |
| `book.epub` (spine of 3 pages, one page with 2 images) | image order equals spine order |
| `broken.epub` (missing container.xml) | `Archive` error, message shown |

HTTP loader (mock source with scripted responses):

| Test | Covers |
|---|---|
| `preload_queues_four_forward` | E-HTTP-9 |
| `visible_page_priority_over_preload` | priority 1 before 0 |
| `retry_error_page_once_per_visit` | E-HTTP-2 |
| `evicted_ready_page_requeued_on_visit` | E-HTTP-1 |
| `data_saver_key_is_original_url` | E-HTTP-4 |
| `failed_image_sets_error_and_continues` | E-HTTP-6 |
| `cancel_removes_queued_entries` | E-HTTP-8 |
| `no_worker_until_first_load_page` | E-HTTP-7 |
| `page_list_persisted_after_fetch` | E-HTTP-5 |

Chapter loader state machine:

| Test | Covers |
|---|---|
| `load_idempotent_when_loaded` | `chapterIsReady` |
| `failed_get_pages_leaves_no_loader` | E-LEAK-1 |
| `retry_recycles_previous_loader` | E-LEAK-2 |
| `cancelled_load_returns_to_wait` | E-CANCEL-1 |
| `read_chapter_start_page_rule` | E-START-1 |
| `choose_loader_precedence` | download first, then local, then http, stub, unsupported |

Integration (GTK, run under `xvfb-run` or a headless display):

| Test | Covers |
|---|---|
| `reader_opens_with_real_page_total` | G-01 |
| `slider_moves_viewer_to_index` | G-09 |
| `prev_next_buttons_respect_ends` | G-08 |
| `rtl_navigator_mirrors_buttons` | G-13, E-UI-7 |
| `bookmark_persists_and_reverts_on_error` | E-BMK-1 |
| `page_actions_save_notifies` | E-SAVE-1 (notification only) |
| `set_cover_messages` | E-COVER-1, E-COVER-2 |
| `split_binding_fix` | E-UI-3 |
| `brightness_negative_overlay_alpha` | overlay math |
| `flash_disabled_no_pending` | E-FLASH-1 |
| `keep_awake_follows_pref` | keep screen on |

Manual test script (for the user, daily use):
1. Open a manga with 3 chapters; read to the last page; confirm the chapter is marked read in the list and the next chapter loads with the transition card.
2. Close and reopen mid-chapter; confirm the page restores.
3. Turn on downloaded-only; open a not-downloaded chapter; confirm the message and Retry.
4. Long-press a page; Save; confirm the file exists under the Pictures folder and a notification appears.
5. Set reading mode to Long strip for this manga; confirm the reader switches and the preference persists after restart.
6. Enable custom brightness -75; confirm the overlay darkens; set 0; confirm it clears.

Fixtures needed: a mock `HttpSource` (scripted page list, image URLs, failure injection), the zip/epub fixtures above, and a temporary database with migrated schema (area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations).
