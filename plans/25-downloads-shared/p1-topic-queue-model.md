# Topic — download queue model (view-side)

Scope: the screen-side model of the download queue as implemented by the two `ScreenModel`s in
- app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt (242 lines)
- app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreenModel.kt (272 lines)

The engine state (`queueState`, status flows, progress flows) is owned by area:23 and area:24. This topic covers only what the view does with it. Behaviour details are in p1-behavior.md sections 3 to 5; the Rust types are in p1-rust-design.md section 2.2.

## 1. Data flow

```
engine.queueState (flat list, queue order)
   │  groupBy(source)  (first-appearance order)
   ▼
state: List<HeaderItem{sourceId, sourceName, count, subItems[]}>
   │  collected on screenModelScope
   ▼
DownloadsTab + page list (counts, FAB visibility, section rendering)
   │  user action
   ▼
reorderQueue / moveTop / moveBottom / moveSeries / cancel / clearQueue
   │  builds flat list of the whole queue
   ▼
engine.reorder(flat) | engine.cancel(list) | engine.clear()
```

Invariant (OBSERVED): the view never mutates engine state directly; every change is a whole-list `reorder`, a `cancel` of a list, or `clear`. The engine applies it and emits a new `queueState`, which rebuilds the view.

## 2. Flat list vs sections

- Source of truth for ordering: the flat list (what the engine downloads, in order).
- Sections are a projection: `groupBy { source }` in first-appearance order.
- Sorting (`reorderQueue`) is applied to each section and then flattened. Sections keep their order, so a sort cannot move items between sources (E-08).
- Moving a series (`move_series`) is applied to the flat list. The regroup can change the order of sections (E-09).
- Drag-and-drop (`onItemReleased`) flattens the adapter in on-screen order (cross-section allowed).

## 3. Operations and their exact list transforms

Notation: `F` = flat list, `S(x)` = section of item x, `ID(x)` = item id.

| Operation | Transform on F | Engine call |
|---|---|---|
| Sort by key k, reverse r | for each section: `stable_sort_by_k` then `reverse if r`; concat sections | `reorder(ids(F'))` |
| Move to top (x) | remove x from S(x), insert at index 0 of S(x); concat sections | `reorder` |
| Move to bottom (x) | remove x from S(x), push to end of S(x); concat sections | `reorder` |
| Move series to top (x) | `sel = {y in F : parent(y)=parent(x)}` in F order; `oth = F - sel`; `F' = sel ++ oth` | `reorder` |
| Move series to bottom (x) | `F' = oth ++ sel` (same partition) | `reorder` |
| Drop (on-screen flat order) | `F' = flatten(adapter, on-screen order)` | `reorder` |
| Cancel (list L) | `F' = F - L`; L's items NOT_DOWNLOADED | `cancel(ids(L))` (engine removes, pauses/resumes if running) |
| Cancel series (x) | `L = {y in F(current page) : parent(y)=parent(x)}`; if L non-empty → cancel | `cancel(ids(L))` |
| Cancel all | queue empty | `clear()` (clear then stop) |

Sort keys:
- anime upload date → `episode.dateUpload`; anime number → `episode.episodeNumber`.
- manga upload date → `chapter.dateUpload`; manga number → `chapter.chapterNumber`.

## 4. Engine calls and their meaning

| View call | Kotlin engine method (anime) | Kotlin engine method (manga) |
|---|---|---|
| reorder(F') | `reorderQueue` → `downloader.updateQueue(F')` (AnimeDownloader.kt:918) | `reorderQueue` → `downloader.updateQueue(F')` (MangaDownloader.kt:747) |
| cancel(L) | `cancelQueuedDownloads` → `removeFromDownloadQueue` | same shape (MangaDownloadManager.kt:253) |
| clear() | `clearQueue` → `downloader.clearQueue(); downloader.stop()` (AnimeDownloadManager.kt:95) | same (MangaDownloadManager.kt:98) |
| start() | `startDownloads` (AnimeDownloadManager.kt:75) | `startDownloads` (MangaDownloadManager.kt:77) |
| pause (FAB) | `pauseDownloads` → `downloader.stop()` (AnimeDownloadManager.kt:88) | `pauseDownloads` → `downloader.pause(); downloader.stop()` (MangaDownloadManager.kt:90) |

Important (E-15, OBSERVED): `updateQueue` replaces the queue. An outdated snapshot can therefore drop newly added items or resurrect removed ones, and an empty list triggers clear+stop. The Rust engine must not reproduce the replace-on-empty behaviour: `reorder([])` is a no-op and ids are an ordering over the live queue.

## 5. Status and progress handling (view side)

State machine per row (driven by `onStatusChange`):

```
            DOWNLOADING                 DOWNLOADED
QUEUE/NOT ───────────────► [progress on] ───────────► [progress off, final update]
                              │  ERROR
                              ▼
                        [progress off, no updates]
```

- Anime: no progress subscription job; updates only through status change callbacks (AnimeDownloadQueueScreenModel.kt:185-202).
- Manga: `launchProgressJob` on DOWNLOADING: poll pages every 50 ms until known, then `combine(page progress flows) → sum → distinctUntilChanged → debounce(50) → collectLatest { update row }` (MangaDownloadQueueScreenModel.kt:215 onward).
- Job map keyed by download object; a new job for the same download cancels the old one.
- On dispose: all jobs cancelled and map cleared.

## 6. Counts and derived values

- Per page: `count = sum(section.subItems.size)`.
- Title pill: anime count only, shown when > 0.
- Tab badge: page count; Kotlin shows 0 as well (E-14; Rust hides at 0).
- FAB visible: page list non-empty.
- Sort and cancel-all visible: page list non-empty.

## 7. Rust state model

```rust
pub struct QueuePage {
    kind: EntryKind,
    model: QueueModel,                 // flat, ordered (p1-rust-design.md 2.2)
    rows: HashMap<i64, RowHandle>,     // item id → widget handles (E-33)
    progress_tasks: HashMap<i64, glib::SourceId>, // or tokio JoinHandle, one per DOWNLOADING row
    running: bool,                     // from engine watch
}
```

Rules:
- `apply_snapshot(flat)`: replace `model`; rebuild sections; reconcile rows (add/remove by item id); cancel progress tasks for rows no longer DOWNLOADING; update counts and FAB.
- Every user action returns a new flat list computed from the current `model`, then sends it to the engine. The model is updated only from the next engine snapshot (no optimistic update), matching Kotlin (the Kotlin view also waits for `queueState`).

Proposal: optimistic local reorder for responsiveness is NOT adopted (keeps behaviour identical); revisit if the engine round trip is visible (>100 ms).

## 8. Tests for this topic

Covered by the test ids in p1-edge-cases-and-tests.md: T-E07, T-E08, T-E09, T-E11, T-E15, T-E16, T-E27, T-E28, T-E31 to T-E34, T-E37.
