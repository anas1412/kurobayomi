# Topic: progress, completion, history and deletion (43-reader-core)

Deep dive on what is written, when, and in what order when the reader moves. The rules are in `p1-behavior.md` sections 7-9; this file gives the event timeline and the write matrix.

## 1. Event timeline for one page change

Triggered by a viewer reporting a visible page (`ReaderActivity.onPageSelected`, `ReaderActivity.kt:686-688`).

```
viewer: page P becomes visible
  -> ReaderViewModel.onPageSelected(P)                       ReaderViewModel.kt:435
     1. P is InsertPage?            -> stop
     2. chapter C = P.chapter ; pages = C.pages ?: stop
     3. launchNonCancellable:
          updateChapterProgress(C, P)                         :528
            a. state.currentPage = P.index + 1                 (UI label/slider)
            b. C.requestedPage = P.index
            c. chapterPageIndex = P.index                      (saved state, even in incognito)
            d. if !incognito and P.status != Error:
                 C.last_page_read = P.index
                 if P.index == last: updateChapterProgressOnComplete(C)   (section 2)
                 updateChapter(id, read = C.read, last_page_read)
     4. if C != currentChapter:
          loadNewChapter(C)                                    :340
            - launchIO:
                flushReadTimer()   (history for OLD chapter, see section 4)
                restartReadTimer()
                loadChapter(loader, C)                         (section 3 of behavior)
     5. if P.number / pages.size > 0.25: downloadNextChapters()  :462
     6. trySend(PageChanged)                                    (drop-able, E-EVT-1)
```

Ordering consequences:
- The database progress write (3d) is launched BEFORE the chapter switch (4) and is not awaited by it. Both run on IO. Their completion order is not defined.
- The flush of history (4) reads the current chapter synchronously before the switch, so the duration is attributed to the old chapter.

## 2. Completion on the last page (`updateChapterProgressOnComplete`, `:554-577`)

Runs every time the last page becomes visible (also if already read).

```
1. C.read = true
2. updateTrackChapterRead(C)          -> only if !incognito && autoUpdateTrack; trackChapter.await(context, manga.id, C.chapter_number)
3. deleteChapterIfNeeded(C)           -> see section 5
4. if "existing" not in mark_duplicate_read_chapter_read: stop
5. duplicates = chapterList where !read && isRecognizedNumber && chapter_number == C.chapter_number
6. updateChapter.awaitAll(duplicates -> ChapterUpdate(id, read = true))
```

Subtle points:
- Step 5 uses the reader list (`chapterList`), not the whole manga. With skip-duplicates on, duplicates are removed from that list first, so they are not marked (E-DUP-1).
- Chapter numbers compare as `Float` (E-DUP-2).
- Steps 2 and 3 run inside the same coroutine as step 1 but the DB write for `read` happens in the caller (`updateChapterProgress` step 5, after the completion call returns) — the write matrix below lists the effective order.

## 3. Write matrix (what is written, by what, when)

| Table / store | Column or key | Written by | When | Skipped when |
|---|---|---|---|---|
| `chapters` | `last_page_read` | `updateChapter` (ChapterUpdate) | each visible page | incognito; page status Error |
| `chapters` | `read` | same ChapterUpdate | each visible page (value as currently in memory) | incognito; page status Error |
| `chapters` | `read` (true) | `updateChapterProgressOnComplete` (through the in-memory object, then the same ChapterUpdate) | last page | incognito; page status Error |
| `chapters` | `read` = true (duplicates) | `awaitAll` ChapterUpdate | last page, duplicate mode "existing" | incognito is NOT checked here (Kotlin: it only runs inside the progress path, which is gated) |
| `chapters` | `bookmark` | `toggleChapterBookmark` | user action | never gated by incognito (Kotlin) |
| `history` | `last_read`, `time_read` | `upsertHistory` (MangaHistoryUpdate) | flush on pause and chapter switch | incognito |
| `mangas` | `viewer` | `SetMangaViewerFlags` | reading mode / orientation change | never gated |
| tracker tables | track update | `TrackChapter.await` | completion | incognito; autoUpdateTrack off (E-TRACK-1) |
| download queue | pending deletes | `enqueueChaptersToDelete` | completion (read chapters only) | slots -1; chapter not read |
| download queue | queued download restore | `addDownloadsToStartOfQueue` | view model cleared | no pending download |
| download store | deletions | `deletePendingChapters` | leaving the reader | none queued |
| share cache | cleared + written | `shareImage` | share action | none |

Observation: the bookmark write is not gated by incognito (`ReaderViewModel.kt:645-664`), which is consistent with Kotlin but is a privacy question for the user. Recorded as an open point for the Rust port (the tasks keep Kotlin behaviour and the implementer should confirm with the user; if the answer is "gate it", it becomes a deliberate fix).

Also observation: the duplicate-marking write is not gated by incognito at its own call site, but it is reachable only from the completion path, which is gated by the progress path (`updateChapterProgress` checks incognito before calling completion, `:537`). So effectively it is gated. The table states the effective behaviour.

## 4. Read timer and history

```
restartReadTimer():  start = now                          (on resume, and on chapter switch)
flushReadTimer():    C = currentChapter (sync)            (on pause, and on chapter switch BEFORE restart)
                     launchNonCancellable updateHistory(C)
updateHistory(C):    if incognito: stop
                     readAt = now ; session = readAt - start (or 0 when start is null)
                     upsertHistory(chapterId, readAt, session)
                     start = null
```

Rules for the Rust port (E-HIST-1, E-HIST-2):
- The start time is owned by the UI thread. The duration is computed on the UI thread, then sent with the command.
- A flush with no start time records `time_read` unchanged (add 0), so it must not overwrite a larger stored value; `last_read` is updated.
- Pause then resume then pause writes two rows' worth of updates to the same `history` row (one row per chapter, keyed by `chapter_id`).

## 5. Delete-after-read (`deleteChapterIfNeeded`, `:508-522`)

```
slots = remove_after_read_slots           (default -1 = off)
if slots == -1: return                    (chapterToDownload untouched)
pos = index of completed chapter in reader list
target = list[pos - slots]                (slots 0 = completed chapter; slots 1 = previous)
chapterToDownload = null                  (completed chapter is read, no need to re-download)
if target != null: enqueue delete(target) if target.read
```

Deletion itself runs when the reader closes (`onActivityFinish`, `:259-261`, `:950-954`).

Edge rules (E-DEL-1..4):
- Values other than -1 that are negative target chapters AHEAD. Rust: treat any negative value as off.
- Slots 0 targets the just-completed chapter; it is deleted on leaving. This is consistent with Kotlin and is kept.
- A queued download cancelled when the reader opened the chapter is restored when the view model is cleared (`onCleared`, `:245-253`), except when `chapterToDownload` was cleared by completion.

## 6. Download-ahead trigger (`downloadNextChapters`, `:462-491`)

Trigger: page past 25 percent by `number / size` (1-based), current chapter read from a download, next chapter downloaded, amount > 0 (read at open).

```
chaptersToDownload = getNextChapters(manga.id, nextChapter.id)   (unread, from next; see E-DLA-1 fallback)
    .removeDuplicates(next) if skip_dupe
    .take(amount)
downloadChapters(manga, chaptersToDownload)
```

## 7. Incognito matrix

| Feature | Incognito on | Incognito off |
|---|---|---|
| Progress (`last_page_read`, `read`) | not written | written |
| Restore keys (`chapter_id`, `page_index`) | updated in memory (`page_index` is also set in incognito) | written |
| History | not written | written |
| Tracker update | not run | runs if enabled |
| Duplicate marking | not run (reachable only through gated completion) | runs if "existing" |
| Delete-after-read | queued (not gated) | queued |
| Bookmark | written (not gated, see section 3) | written |
| Global incognito toggle off | n/a | closes the reader (`ReaderActivity.kt:192-196`) |

## 8. Ordering invariants the Rust port must keep

1. On a page change of a 1-based number greater than the threshold (25 percent), the next-chapter download check happens after progress is queued, not before.
2. History flush for the old chapter happens before the new chapter loads.
3. Completion (track, delete queue, duplicates) happens after `read = true` is set in memory and before the progress command is flushed, so the progress command carries `read = true`.
4. Nothing in this flow blocks the UI thread (E-UI-2).
5. Incognito gates every database write in this flow except bookmark and mode/orientation (documented exceptions).
