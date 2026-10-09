# Area 23 — Manga downloads: UI parity

This area renders one screen: the manga half of the Download queue. The screen lives inside the Downloads page (area:25 owns `app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt`, which hosts the Anime/Manga switch, the sort dropdown and the overflow "Cancel all"). Files in scope are the 9 files under `app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/`.

Sources read for this document:
- Kotlin: MangaDownloadQueueTab.kt, MangaDownloadQueueScreen.kt, MangaDownloadQueueScreenModel.kt, MangaDownloadAdapter.kt, MangaDownloadHeaderHolder.kt, MangaDownloadHeaderItem.kt, MangaDownloadHolder.kt, MangaDownloadItem.kt.
- Layout resources: app/src/main/res/layout/download_item.xml, download_header.xml, download_list.xml; menu app/src/main/res/menu/download_single.xml.
- Spec written earlier from Aniyomi: `spec/app.json`, entry `download_queue` (lines 732-752), More row subtitle (line 729), Updates rows (line 539), and the Downloads page (line 681-683).
- Mockup: `src/screens.rs` (`fn downloads()`, lines 354-397) and `src/ui.rs` (`menu_button` at 202, `list_row`, `boxed_list`, `heading`, `fab`).

Labels below are the English strings with their keys. Behaviour details are in `p1-behavior.md` sections 13-14.

---

## 1. Screen anatomy (manga tab)

### 1.1 Top app bar

| Element | Source | Behaviour | Text or icon |
|---|---|---|---|
| Back arrow | Spec `download_queue` | Pops to More or the Downloads root | `navigateUp = navigator::pop` (MangaDownloadQueueTab.kt:407) |
| Title | `label_download_queue` | Static | "Download queue" |
| Count pill | Spec | Number of queued items in the current tab (sum of group sizes) | `numberTitle = downloadCount` (QueueTab.kt:406) |
| Sort action | `action_sort` | Shown only when the queue is not empty. Opens the dropdown in 1.4 | icon: sort (AutoMirrored.Outlined.Sort in Kotlin); GTK icon `view-sort-ascending-symbolic` (mockup) |
| Overflow "Cancel all" | `action_cancel_all` | Shown only when the queue is not empty. Calls `clearQueue()` | "Cancel all" |

The Kotlin tab sets `searchEnabled = false` (QueueTab.kt:398). The Rust view has no search field.

### 1.2 Tabs

Anime | Manga switch as a `PrimaryTabRow`, each tab with a count badge (spec line 681-683). Owned by area:25. This area provides the Manga content and the count.

### 1.3 List

- Groups by source in first-occurrence order (`groupBy { it.source }`, QVM.kt:122-127).
- **Group header row** (layout `download_header.xml`, Kotlin `MangaDownloadHeaderHolder`):
  - Text `"<source name> (<count>)"` (MangaDownloadHeaderHolder.kt:47).
  - Drag handle at the start (`reorder` view). The whole group can be dragged (spec line 752 "whole group draggable").
  - While the header is being dragged: the group collapses (`collapseAll`) and the container is marked dragged. On release: `expandAll` and the reorder is committed (MangaDownloadHeaderHolder.kt:50-63).
- **Item row** (layout `download_item.xml`, Kotlin `MangaDownloadHolder`):
  - Drag handle at the start (`reorder`).
  - Line 1: manga title (`manga_full_title`).
  - Line 2: chapter name (`chapter_title`).
  - Right side: progress text `download_progress_text`, format `"<downloadedImages>/<pages.size>"` (MangaDownloadHolder.kt:200). Before the page list is known: empty string (MangaDownloadHolder.kt:176).
  - Progress bar `download_progress` along the row (`max = pages.size * 100`, `value = totalProgress`, animated with `setProgressCompat(value, true)`). Before pages are known: `progress = 0`, `max = 1`.
  - Trailing menu button `menu` (opens the popup in 1.5).
- **Row tap:** no action in the Kotlin (no click listener). Proposal: tap opens the entry's details page (area:38), since "Show manga" is in the notifications anyway.

Spec-versus-code difference: spec line 752 says the right side shows "Downloading…" while indeterminate. The Kotlin code shows an empty string until the page list exists (MangaDownloadHolder.kt:176). Proposal: follow the code, but show "Downloading…" from the moment the chapter enters DOWNLOADING, so the row is never blank when work is in progress. Flagged for the reviewer.

### 1.4 Sort dropdown (from DownloadsTab.kt:384-455)

| Item | Submenu item | Action |
|---|---|---|
| By upload date | Newest | sort each group by `chapter.dateUpload`, reversed (newest first) |
| By upload date | Oldest | sort each group by `chapter.dateUpload`, ascending |
| By chapter number | Ascending | sort each group by `chapter.chapterNumber`, ascending |
| By chapter number | Descending | sort each group by `chapter.chapterNumber`, reversed |

Sorting is per group; the groups keep their order (behaviour 14.4). Each choice rewrites the queue order (persisted).

### 1.5 Row popup menu (res/menu/download_single.xml, QVM.kt:68-115)

| Item (id) | Label key | Visible when | Action |
|---|---|---|---|
| Move to top (`move_to_top`) | `action_move_to_top` "Move to top" | adapter position > 1 (not the first row) | move row to top of its group |
| Move series to top (`move_to_top_series`) | `action_move_to_top_all_for_series` "Move series to top" | always | the series' rows go first in the queue |
| Move to bottom (`move_to_bottom`) | `action_move_to_bottom` "Move to bottom" | not the last adapter position | move row to bottom of its group |
| Move series to bottom (`move_to_bottom_series`) | `action_move_to_bottom_all_for_series` "Move series to bottom" | always | the series' rows go last |
| Cancel (`cancel_download`) | `action_cancel` "Cancel" | always | cancel this chapter |
| Cancel all for this series (`cancel_series`) | `cancel_all_for_series` "Cancel all for this series" | always | cancel every row with the same manga id |

Menu order in the Kotlin file: Move to top, Move series to top, Move to bottom, Move series to bottom, Cancel, Cancel all for this series (download_single.xml order). Keep this order.

### 1.6 Footer action (extended FAB)

- Bottom-end, extended FAB, shown only when the list is not empty. Collapses to an icon on scroll (spec line 752).
- Manga tab: label "Pause" with the pause icon when running; "Resume" with the play icon when paused (spec line 752; Kotlin `pauseDownloads` / `startDownloads`).
- Anime tab uses "Stop" and "Continue" (area:24).
- The label must come from the downloader state (`DownloadManager::running()`, `is_paused`), not from the button's own text. The mockup reads its own label back (`lbl.label() == b`, screens.rs:376-383), which is a shortcut to remove.

### 1.7 Empty state

- Empty queue: centred text "No downloads" (`information_no_downloads`), with the standard empty-screen layout (EmptyScreen in Kotlin).
- No icon is specified in the Kotlin code; Proposal: use `folder-download-symbolic` with the same text, matching the mockup `status()` helper (ui.rs:166).

### 1.8 Loading and error states

- **Loading:** none in the Kotlin. The queue is in memory from start, so no spinner. Proposal: none.
- **Errors per chapter:** the Kotlin row does not show ERROR at all. An errored chapter looks like a queued chapter with its last progress. Errors are only shown as notifications. Proposal: an error label on the row ("Failed: <message>" in error colour, from `Error` events) and a "Retry" row action that sets the chapter to QUEUE (area:43 has the same need for the reader).
- **Downloader-level errors:** the Kotlin shows them as notifications only; no in-app banner. Proposal: keep notifications, and show the last error for the queue in the page subtitle until the next start.

---

## 2. Interaction and animation

| Interaction | Kotlin | Proposal (GTK) |
|---|---|---|
| Drag a row within its group | RecyclerView drag with drag handle; `shouldMove` blocks other groups | `GtkDragSource` on the handle, `GtkDropTarget` on rows; drop only inside the same group |
| Drag a group header | Whole group draggable; the group collapses during the drag and expands on drop | `GtkDragSource` on the header handle; drag preview is the header; collapse by hiding its rows during the drag |
| Drop indicator | RecyclerView default | 2 px accent line between rows (`accent_color` from the theme, not hard-coded) |
| Progress update | `setProgressCompat(value, animate=true)` | `gtk::ProgressBar::set_fraction`; no animation needed, updates are at most every 50 ms |
| Row enter or leave | RecyclerView default item animator | GTK `ListView` default; no custom animation |
| Scroll | nested scroll with the bottom app bar and FAB collapse | `ScrolledWindow` with the FAB overlaid; collapse when the scroll position is not at the top |
| Expand and collapse of a group | Header is an expandable header (`isExpanded = true` at init) | Click the header to collapse its rows; state kept per session |

Animations used by the Kotlin screen: progress bar animation, RecyclerView item animator, FAB extend/shrink on scroll. Keep the progress animation out (fraction changes are cheap), keep the FAB shrink/extend (a CSS transition of 150 ms).

---

## 3. Keyboard and mouse (desktop equivalents)

Android has none of these; they are proposals for the Linux build.

| Input | Action |
|---|---|
| Up / Down | move focus between rows and headers |
| Enter | open the focused row's manga (Show manga) |
| Space | pause or resume the queue (same as the FAB) |
| Delete | cancel the focused row |
| Ctrl+Shift+Delete | Cancel all (Proposal: confirmation dialog; the Kotlin overflow item has no confirmation) |
| Ctrl+Up / Ctrl+Down | move the focused row up or down one place inside its group (keyboard reorder, no drag needed) |
| Ctrl+Home / Ctrl+End | move the focused row to the top or bottom of its group |
| Shift+Ctrl+Up / Shift+Ctrl+Down | move the focused series to the top or bottom |
| Right click or Menu key | open the row popup (1.5) |
| Long press (touch screens) | open the row popup |
| Double click | open the manga (Proposal) |
| Mouse wheel over the list | scroll |

Accessibility: each row exposes `accessible-label` = "<title>, <chapter>, <progress text>", and the menu button has the tooltip "More" (`view-more-symbolic`). Focus rings come from the GTK theme.

---

## 4. Responsive behaviour

The Kotlin screen has one layout for phone and tablet. The desktop window can be narrow (tiling window managers, split views).

| Window width | Layout |
|---|---|
| 720 px and wider | Title on the left, progress text and menu on the right, bar along the bottom |
| 480 to 719 px | Progress text moves to the line under the chapter name, right-aligned |
| below 480 px | Chapter name is hidden behind a tooltip; progress text moves under the title; the count pill is hidden |

Implementation (Proposal): GTK 4.16 has no built-in breakpoints (those are libadwaita's), so use the window's `default-width` notifier (`connect_notify_local("default-width")` or a `size-allocate` handler) to toggle CSS classes `narrow` and `very-narrow`. Padding: 16 px side gutter. No horizontal scroll: the title ellipsises (`ellipsize = End`).

---

## 5. Strings

Every visible string uses its key from `i18n/src/commonMain/moko-resources/base/strings.xml` (MR) or `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (AYMR). Keys used on this screen:

| Where | Key | English |
|---|---|---|
| tab title | `label_manga` (AYMR) | Manga |
| page title | `label_download_queue` (MR) | Download queue |
| sort action | `action_sort` (MR) | Sort |
| sort menu | `action_order_by_upload_date`, `action_newest`, `action_oldest`, `action_order_by_chapter_number`, `action_asc`, `action_desc` (MR) | By upload date / Newest / Oldest / By chapter number / Ascending / Descending |
| overflow | `action_cancel_all` (MR) | Cancel all |
| empty | `information_no_downloads` (MR) | No downloads |
| row progress | `chapter_downloading_progress` (MR) | Downloading (%1$d/%2$d) (used by notification; the row uses the bare "x/y") |
| row menu | `action_move_to_top`, `action_move_to_top_all_for_series`, `action_move_to_bottom`, `action_move_to_bottom_all_for_series`, `action_cancel`, `cancel_all_for_series` (MR) | see 1.5 |
| FAB | `action_pause`, `action_resume` (MR) | Pause / Resume |
| paused subtitle | `download_paused` (AYMR) | Paused |
| header | `"<name> (<count>)"` (no key; format string) | |

Strings the mockup uses that have no key here: "Start downloading now", "Cancel", "Delete" on the Updates and Browse menus. Those belong to other areas (area:37, area:38).

---

## 6. What the existing mockup lacks

Mockup file: `src/screens.rs`, function `downloads()` (lines 354-397), and the helpers in `src/ui.rs`.

| # | Gap | Where in the mockup |
|---|---|---|
| 1 | Rows and groups are hard-coded ("Source A", "Source B", counts 3 and 2, chapter 1..n). No data. | screens.rs:359-365 |
| 2 | Progress is a constant 0.45 and labels are "12/30" or "45%". No per-chapter progress, no ERROR state, no indeterminate state. | screens.rs:372-375 |
| 3 | Header text is hard-coded "(n)" from the loop, not the group count. | screens.rs:360 |
| 4 | Row menu entries exist but their callbacks are `\|_\| {}`. No move, no cancel. | screens.rs:369 |
| 5 | Sort menu callbacks are `\|_\| {}`. | screens.rs:378, 382 (sort menu) |
| 6 | Overflow "Cancel all" is not in the mockup (the top bar has only the sort and the menu). | screens.rs:398 onwards (toolbar) |
| 7 | FAB label is read back from the label widget to decide the state (`lbl.label() == b`). The state is not the downloader's. | screens.rs:376-383 |
| 8 | No drag and drop; the Kotlin uses drag handles on rows and headers. | screens.rs:362-366 (handle icon only) |
| 9 | No empty state (the count pill and the list always show). | screens.rs:354-397 |
| 10 | No queue subtitle showing the last error or paused state. | screens.rs:354-397 |
| 11 | No "Show manga" action. Kotlin has it only on notifications; Proposal: add it to the row menu as well. | screens.rs:369 |
| 12 | Tab counts are not linked to the queue size (Anime and Manga are two literal lists). | screens.rs:354-397 + `kind_tabs` |
| 13 | No width handling (the row has one fixed layout). | screens.rs:362-375 |
| 14 | No keyboard support (only the default GTK focus). | — |
| 15 | No notification-facing behaviour (pause and resume from the notification). | — |

What the mockup does right and should be kept: the group header and row widget structure, the `list_row` helper with a leading drag handle icon, the trailing `menu_button` (`view-more-symbolic`), the `fab` helper with an icon and label, and the `boxed_list` containers.

---

## 7. View model contract (`src/download_queue_model.rs`)

```rust
pub struct DownloadQueueModel {
    manager: DownloadManager,
    groups: RefCell<Vec<GroupView>>,        // derived from snapshot()
    tx: async_channel::Sender<Msg>,         // to the view
}
pub struct GroupView { pub source_id: i64, pub name: String, pub rows: Vec<RowView> }
pub struct RowView {
    pub chapter_id: i64, pub manga_id: i64, pub manga_title: String, pub chapter_name: String,
    pub status: DownloadState, pub downloaded: usize, pub total: usize, pub progress_sum: i64,
}
pub enum Msg { Rebuild, Progress(i64 /*chapter*/), Status(i64), Running(bool), Empty(bool) }
```

- `Rebuild` on `QueueChanged`; `Progress` at most every 50 ms per chapter (coalesced in the actor); `Status` on each transition.
- Actions call `DownloadManager` (`reorder`, `start_now`, `cancel`, `cancel_manga`, `start`, `pause`, `clear`). Each action is a single command; the view waits for `QueueChanged` before redrawing (no optimistic update, which the Kotlin also avoids).
- `Running(bool)` drives the FAB label. `Empty(bool)` drives the empty state.

---

## 8. Test plan for this screen (UI)

1. Empty queue shows "No downloads" and hides the sort and cancel-all actions.
2. A queue with two sources shows two headers with counts 3 and 2 when 3 and 2 chapters are queued.
3. Dragging a row to another group is refused (drop indicator absent).
4. Move to top hides on the first row; move to bottom hides on the last row.
5. Cancel all for this series removes exactly the rows with the same manga id.
6. The FAB reads "Resume" when the downloader is paused and "Pause" when it runs, after a click, without any polling.
7. Narrow width (below 480 px) puts progress text under the title; no horizontal scrollbar at 360 px width.
