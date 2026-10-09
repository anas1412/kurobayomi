# Topic: chapter loading pipeline (43-reader-core)

Deep dive for the loader stack. Rules are in `p1-behavior.md` sections 2-4; this file adds the timelines, the per-page status table, and the test vectors the implementer needs in one place.

## 1. Who calls what

```
ReaderActivity.onCreate                       app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt:163-187
  -> ReaderViewModel.init(mangaId, chapterId)  ReaderViewModel.kt:274
       -> sourceManager.isInitialized.first    (wait for extensions)
       -> ChapterLoader(...)                   ChapterLoader.kt:25
       -> chapterList (lazy, runBlocking)      ReaderViewModel.kt:154
       -> ChapterLoader.loadChapter(current)   ChapterLoader.kt:39
            -> getPageLoader                   ChapterLoader.kt:81
            -> loader.getPages()
       -> ViewerChapters(current, prev, next)  ReaderViewModel.kt:307-334
  page visible  -> ReaderViewModel.onPageSelected -> loadNewChapter (other chapter)
  nav button    -> ReaderViewModel.loadAdjacent
  viewer edge   -> ReaderViewModel.preload (cross-chapter, no UI event on failure)
```

## 2. Chapter state timeline

```
            loadChapter()                 getPages ok & non-empty
  Wait ------------------------> Loading ------------------------> Loaded(pages)
   ^                               |                                  |
   |                               | any error (incl. cancel!)        | unref -> 0
   |                               v                                  |
   +------- unref -> 0 ------- Error(e) <--- retry: loadChapter/preload
                                  |
                                  +-- Kotlin: cancel also lands here (E-CANCEL-1)
                                  +-- Rust:   cancel lands in Wait instead
```

Side effects per transition (Kotlin, with Rust change where marked):
- `Wait -> Loading`: none.
- `Loading -> Loaded`: `pageLoader` set (Rust: set only after success, E-LEAK-1), `requestedPage` set per the start rule (E-START-1), every page gets `chapter = this`.
- `-> Error`: nothing recycled (Kotlin keeps a loader attached on a failed `getPages`, E-LEAK-1).
- `Loaded/Error -> Wait` via `unref() == 0`: `pageLoader.recycle()`, `pageLoader = null`.

## 3. Per-page status table (HTTP loader)

| From | Trigger | To | Enqueued at | Notes |
|---|---|---|---|---|
| `Queued` | `load_page` on this page | `Queued` (in queue) | priority 1 | `ReaderPage` created by `get_pages` (status default `QUEUE`, `Page.kt:22`) |
| `Queued` | forward preload | `Queued` (in queue) | priority 0 | at most 4 forward pages of the same chapter |
| `Queued` | worker dequeues, `imageUrl` missing | `LoadPage` | - | calls `source.getImageUrl` |
| `LoadPage` or `Queued` | image not cached | `DownloadImage` | - | `source.getImage(page, dataSaver)` |
| `DownloadImage` | bytes written to cache | `Ready` | - | `stream` = cache file |
| any | non-cancellation error | `Error` | - | worker continues |
| `Error` | `load_page` visible | `Queued` | priority 1 | automatic retry |
| `Error` | `retry_page` | `Queued` | priority 2 | manual retry |
| `Ready` | visible, cache entry gone | `Queued` | priority 1 | eviction re-queue (E-HTTP-1) |
| `Ready` | stream open fails (file gone) | `Queued` | priority 1 | E-IMG-3 (Rust only) |
| any queued | viewer cancels guard | `Queued` (removed from queue) | - | E-HTTP-8 |

Worker rule: dequeue, skip if `status != Queued`. Priority order: higher first; equal priority FIFO by enqueue sequence (`RL/HttpPageLoader.kt:224-227`).

## 4. Dequeue examples (test vectors)

Enqueue order and expected dequeue order, with priorities in brackets:

1. `p3[0]`, `p4[0]`, `p5[1]` -> `p5`, `p3`, `p4` (visible before preload, FIFO within preload).
2. `p3[0]`, `p3[1]` (same page, two entries) -> first dequeue fetches; second is skipped because `p3` is already `Ready` (E-HTTP-3).
3. `p7[0]` queued, then `retry(p7)` while `p7` is `Error` -> `p7[2]` dequeued first; `p7` fetched once.
4. `p9` is `Ready` and `retry(p9)` is called -> entry `p9[2]` is dequeued and skipped (status not `Queued`), so no fetch (E-HTTP-2).

## 5. Preload window

For a chapter of N pages and a visible page index i (0-based):
- Queued preload pages are `i+1 .. min(i+4, N-1)` that are `Queued` at the moment of the call.
- If `i == N-1`: no preload.
- Pages `< i` are never preloaded.
- Cross-chapter preload is requested by the viewer near the edge (area:44-reader-viewers) and calls `ReaderViewModel.preload(next)`, which loads the next chapter's PAGE LIST only (its images are not downloaded until the viewer asks for them).

## 6. Archive and directory loader behaviour

- `DirectoryPageLoader` (`RL/DirectoryPageLoader.kt:16-28`): one level only; files that are not directories and pass the image rule; natural sort; index 0..n.
- `ArchivePageLoader` (`RL/ArchivePageLoader.kt:15-26`): entries that are files and pass the image rule; natural sort; index 0..n. The Kotlin page stream re-scans the archive from the start for every page (`ArchiveReader.kt:18-22`); the Rust port builds an entry index once (E-ARC-1).
- Both: empty result -> the chapter loader raises "No pages found" (`page_list_empty_error`).

Test vectors for the natural sort (from `p1-edge-cases-and-tests.md` E-SORT-1):
- `["10.jpg","2.jpg","1.jpg"]` -> `["1.jpg","2.jpg","10.jpg"]`.
- `["Page 2","page 10","PAGE 1"]` -> `["PAGE 1","Page 2","page 10"]`.

## 7. Selection precedence table

| Priority | Condition (evaluated in order) | Loader | Source line |
|---|---|---|---|
| 1 | download found with skip-cache lookup | `DownloadPageLoader` | `RL/ChapterLoader.kt:83-97` |
| 2 | source is `LocalMangaSource`, format directory | `DirectoryPageLoader` | `RL/ChapterLoader.kt:98-100` |
| 2 | ... format archive | `ArchivePageLoader` | `RL/ChapterLoader.kt:101` |
| 2 | ... format epub | `EpubPageLoader` | `RL/ChapterLoader.kt:102` |
| 3 | source is `HttpSource` | `HttpPageLoader` | `RL/ChapterLoader.kt:105` |
| 4 | source is `StubMangaSource` | error "Source not installed: %s" | `RL/ChapterLoader.kt:106-108` |
| 5 | anything else | error "Source not found" | `RL/ChapterLoader.kt:109` |

## 8. Cache keys and layout (Kotlin, reference for the Rust cache)

- Page list: key = md5(`"${mangaId}${url}"`), value = JSON array of `{index, url, imageUrl}` (`ChapterCache.kt:197-199`, `:81-111`).
- Image: key = md5(original image URL), file name `md5 + ".0"` (`ChapterCache.kt:130-134`).
- Capacity: 100 MiB (`ChapterCache.kt:209`).
- Rust (proposal, `p1-rust-design.md` 3.5): SQLite index table `reader_cache(key TEXT PRIMARY KEY, kind TEXT, path TEXT, size INTEGER, last_access INTEGER)` plus files in the cache directory; LRU on `last_access` until the total is at most 100 MiB.

## 9. What the Rust port must not do

- Do not start a worker before the first `load_page` (E-HTTP-7).
- Do not keep a loader attached after a failed `get_pages` (E-LEAK-1).
- Do not turn cancellation into `Error` (E-CANCEL-1).
- Do not rescan an archive per page (E-ARC-1).
- Do not write a page-list file on the UI thread (P-9).
- Do not create `gdk::Texture` off the main thread (P-8).
