# 24-downloads-anime: UI parity (download queue)

Scope files that render UI (all under `app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/`):
`AnimeDownloadAdapter.kt`, `AnimeDownloadHeaderHolder.kt`, `AnimeDownloadHeaderItem.kt`, `AnimeDownloadHolder.kt`, `AnimeDownloadItem.kt`, `AnimeDownloadQueueScreen.kt`, `AnimeDownloadQueueScreenModel.kt`, `AnimeDownloadQueueTab.kt`.

Resources referenced: layouts `app/src/main/res/layout/download_item.xml`, `download_header.xml`, `download_list.xml`; menu `app/src/main/res/menu/download_single.xml`.

Spec cross-check: `spec/app.json` id `download_queue` (the mockup's extracted spec) describes the whole page, including items this scope does not own (sort dropdown, Cancel-all overflow, FAB, tab badges). Those are listed in §7 with their owning area.

---

## 1. Page anatomy (as implemented in this scope)

### 1.1 Tab (`AnimeDownloadQueueTab.kt:352-379`)

`Screen.animeDownloadTab(nestedScrollConnection)` returns a `TabContent`:
- `titleRes = AYMR.strings.label_anime` (the Anime tab label; the Manga tab is the manga package).
- `searchEnabled = false`: no search field.
- `numberTitle = downloadList.sumOf { it.subItems.size }`: total count of queued downloads on this tab, shown as a count next to the title.
- `navigateUp = navigator::pop`: back arrow pops the navigator.
- `content` renders `AnimeDownloadQueueScreen` with the given `contentPadding`.

`downloadCount` is `derivedStateOf` over `downloadList`, so it recomputes only when the list changes.

### 1.2 Screen (`AnimeDownloadQueueScreen.kt:270-335`)

Compose `Scaffold { ... }`:

1. If `downloadList` is empty: `EmptyScreen(stringRes = MR.strings.information_no_downloads, modifier = Modifier.padding(contentPadding))` then return. Text: "No downloads".
2. Else: compute four `Int` paddings (left, top, right, bottom) from `contentPadding` in pixels (`roundToInt`).
3. `Box(Modifier.nestedScroll(nestedScrollConnection))` containing `AndroidView` (RecyclerView, `DownloadListBinding`):
   - `factory`: inflate `download_list.xml` (`RecyclerView` `match_parent`, `clipToPadding="false"`), create `AnimeDownloadAdapter(screenModel.listener)`, set `isHandleDragEnabled = true`, `LinearLayoutManager(context)`, nested scrolling enabled, and two `launchUI` collectors: `getDownloadStatusFlow().collect(onStatusChange)` and `getDownloadProgressFlow().collect(onUpdateDownloadedPages)`.
   - `update`: `updatePadding(left, top, right, bottom)` on the root, then `adapter.updateDataSet(downloadList)`.

The RecyclerView has `clipToPadding=false`, so the list scrolls under the top app bar and the bottom bar.

### 1.3 Groups (`AnimeDownloadQueueScreenModel.kt:112-126`)

`state` = `queueState.map { downloads -> downloads.groupBy { it.source }.map { (source, list) -> AnimeDownloadHeaderItem(source.id, source.name, list.size) with items AnimeDownloadItem(it, header) } }`.

- Group order: first appearance of each source in the queue list.
- Item order within group: queue order (`addSubItems(0, ...)` inserts them at the front in list order; the effective order is the list order).
- Header title shown: `"${item.name} (${item.size})"` (`AnimeDownloadHeaderHolder.kt:47`). Header `name` is `source.name` (not `toString()`), so no language code.

### 1.4 Header row (`AnimeDownloadHeaderHolder.kt`, `download_header.xml`)

Layout: `MaterialCardView` (`marginTop 8dp`, `cardElevation 0`, background `?android:attr/colorBackground`, foreground `@color/draggable_card_foreground`). Inside a horizontal `LinearLayout`:
- `title`: `TextAppearance.Tachiyomi.SectionHeader`, `weight 1`, `paddingHorizontal 16dp`, `paddingVertical 8dp`, one-line not enforced, centred vertically.
- `reorder`: drag handle `ic_drag_handle_24dp`, tint `?android:attr/textColorHint`, padding `10dp` horizontal and `8dp` vertical, `marginEnd 4dp`, placed at the END of the row.

Behaviour:
- `setDragHandleView(binding.reorder)`: only the handle starts a drag.
- Drag start (`ACTION_STATE_DRAG`): `container.isDragged = true` (card foreground tint) and `collapseAll()` (all groups collapse while dragging a group).
- Release: `isDragged = false`, `expandAll()`, then `downloadItemListener.onItemReleased(position)`.

Header items: `isExpanded = true`, `isHidden = false`, `isSelectable = false` (`AnimeDownloadHeaderItem.kt:123-127`). A header cannot be selected.

### 1.5 Item row (`AnimeDownloadHolder.kt`, `download_item.xml`)

Layout (`download_item.xml`):
- `MaterialCardView` (`match_parent` width, `wrap_content` height, same card styling as header).
- `ConstraintLayout` with `paddingVertical 4dp`:
  - `reorder` (start): `ImageView` `ic_drag_handle_24dp`, `padding 16dp`, constrained to parent top/bottom/start. The drag handle of an item.
  - `manga_full_title` (anime title): `TextAppearance textAppearanceBodyMedium`, `maxLines 1`, `ellipsize end`, start after handle, end before `download_progress_text`.
  - `chapter_title` (episode name): `textAppearanceBodyMedium`, `textSize 12sp`, `marginTop 2dp`, `maxLines 1`, `ellipsize end`, below the title, end before `menu`.
  - `download_progress` (`LinearProgressIndicator`): `marginTop 8dp`, `marginBottom 4dp`, below chapter title, from handle to menu, `width 0dp`, bottom-constrained to parent.
  - `download_progress_text`: `textSize 12sp`, `textColor ?android:textColorSecondary`, `maxLines 1`, aligned to the title's top and bottom, placed before `menu`.
  - `menu` (`ImageButton`): `ic_overflow_24dp`, tint `?attr/colorOnBackground`, `paddingHorizontal 10dp`, `paddingVertical 16dp`, `contentDescription @string/action_menu`, end-constrained.

Bind (`AnimeDownloadHolder.kt:166-185`):
- `chapterTitle.text = episode.name`; `mangaFullTitle.text = anime.title`.
- If `video == null`: progress `0`, `max = 1`, `downloadProgressText.text = ""`.
- Else: `max = 100`, `notifyProgress()`, `notifyDownloadedPages()`.

`notifyProgress()` (`:190-200`): if `max == 1`, set 100. If `progress == 0`: indeterminate. Else determinate `setProgressCompat(progress, true)` (animated).

`notifyDownloadedPages()` (`:205-211`): text = `"Downloading…"` (MR `update_check_notification_download_in_progress`) if `progress == 0`, else `"N%"` (AYMR `episode_download_progress`, `%1$d%%`).

Observed states of an item row:

| Download state | `video` | Progress bar | Progress text | Title line |
|---|---|---|---|---|
| QUEUE | null | 0 of max 1 (empty bar) | "" | anime title, episode name |
| QUEUE (after resolution) | set | indeterminate (progress 0) | "Downloading…" | same |
| DOWNLOADING, resolving hosters | null | empty bar | "" | same |
| DOWNLOADING, progress 0 | set | indeterminate | "Downloading…" | same |
| DOWNLOADING, progress 1–99 | set | determinate N | "N%" | same |
| DOWNLOADED | set | 100 | "100%" | removed from list |
| ERROR | set or null | last value | last value, see note | remains in list |

Note: an item in ERROR keeps its last progress and text, and is not visually flagged. The Kotlin UI shows no error icon or colour (observed). Desktop should add an error glyph (see §4).

### 1.6 Item menu (`AnimeDownloadHolder.kt:226-238`, `download_single.xml`)

Popup built with `view.popupMenu(menuRes = R.menu.download_single, initMenu, onMenuItemClick)`. Entries in order:

| Order | Item id | Label (MR key) | Visible when |
|---|---|---|---|
| 1 | `move_to_top` | "Move to top" (`action_move_to_top`) | `bindingAdapterPosition > 1` |
| 2 | `move_to_top_series` | "Move series to top" (`action_move_to_top_all_for_series`) | always |
| 3 | `move_to_bottom` | "Move to bottom" (`action_move_to_bottom`) | `bindingAdapterPosition != adapter.itemCount - 1` |
| 4 | `move_to_bottom_series` | "Move series to bottom" (`action_move_to_bottom_all_for_series`) | always |
| 5 | `cancel_download` | "Cancel" (`action_cancel`) | always |
| 6 | `cancel_series` | "Cancel all for this series" (`cancel_all_for_series`) | always |

Visibility quirks (observed):
- `bindingAdapterPosition` counts header rows. The first item overall sits at position 1, so "Move to top" is hidden only for that first item. The first item of every later group is shown, even though it is already at the top of its group. Choosing it is a no-op. Proposal: hide when the item is the first in its own group.
- "Move to bottom" is hidden only for the very last row in the whole list, not the last in its group. Proposal: hide when last in its group.

Menu actions (`AnimeDownloadQueueScreenModel.kt:62-109`):
- `move_to_top` / `move_to_bottom`: remove the item from its header's sub-items, insert at index 0 (top) or append (bottom), then collect all downloads across headers in header order and call `reorder`. Items keep their group. A header-only reorder.
- `move_to_top_series` / `move_to_bottom_series`: take the flat list of current items, partition into `selected` = same anime id as the clicked item and `others`. Top: `selected + others`. Bottom: `others + selected`. Relative order inside each part is kept. Then `reorder`.
- `cancel_download`: `cancel(listOf(item.download))`.
- `cancel_series`: all current items with the same anime id; if non-empty, `cancel(all)`.

Note the series moves are across groups (they ignore source groups), while the single moves stay inside the group. Observed, and this is intentional in the Kotlin: a series is pulled out of the group logic. Keep it.

### 1.7 Drag and drop (`AnimeDownloadAdapter.kt:18-21`, `AnimeDownloadHolder.kt:213-217`, `AnimeDownloadQueueScreenModel.kt:46-54`)

- `FlexibleAdapter` with `isHandleDragEnabled = true`: the drag handle starts a drag, not a long-press on the whole row (`setDragHandleView(binding.reorder)`).
- `shouldMove(from, to)` returns true only if `getHeaderOf(getItem(from)) == getHeaderOf(getItem(to))`: an item cannot be dragged into another group.
- Drag start on an item: `container.isDragged = true`.
- On release (`onItemReleased`): `isDragged = false`, then `listener.onItemReleased(position)`, which builds the flat list of all downloads in header order and calls `reorder(downloads)`.

Header drag: drags a whole group. Its release collects all groups in current header order and calls `reorder`.

Item drag: `AnimeDownloadItem.isDraggable()` = true.

### 1.8 Empty, loading and error states

- Empty (no downloads): `EmptyScreen` with "No downloads" (`MR.information_no_downloads`).
- Loading: none. The state starts as `emptyList()` (`AnimeDownloadQueueScreenModel.kt:25`), so the first frame shows "No downloads" until the flow emits. Proposal: show a spinner during the first load.
- Error: none in this page. Errors surface as notifications (behaviour §10) and per-row state only.

### 1.9 Status and progress updates (`AnimeDownloadQueueScreenModel.kt:185-241`)

- `onStatusChange(download)` (from `statusFlow`):
  - DOWNLOADING: update progress and text.
  - DOWNLOADED: cancel the progress job, update.
  - ERROR: cancel the progress job.
  - other: nothing.
- `onUpdateDownloadedPages(download)` (from `progressFlow`): updates text and bar on the holder.
- `getHolder(download)` finds the bound holder by `findViewHolderForItemId(download.episode.id)`. Rows are identified by episode id (the item id).
- Updates are imperative (`notifyProgress` on the holder), not a rebind. Proposal: drive the row from a `watch` receiver (see `p1-rust-design.md` §10).

### 1.10 Sorting helper (`AnimeDownloadQueueScreenModel.kt:162-178`)

`reorderQueue(selector, reverse = false)`: for each header, sort sub-items by `selector` (`AnimeDownloadItem -> R: Comparable`), reverse if requested, then reorder the flat list. No call site inside the 18 scope files; the sort dropdown lives in `ui/download/DownloadsTab.kt` (area:25-downloads-shared). The spec's sort options ("By upload date" newest/oldest, "By episode number" ascending/descending) are satisfied by a selector on `episode.dateUpload` or `episode.episodeNumber`. Keep this function as the single sort entry point (T-24a-045).

### 1.11 Toolbar and count

Title "Anime" with count pill from `numberTitle` (§1.1). The Kotlin count is the total number of queued downloads (all states, including ERROR).

---

## 2. Screen-model lifecycle (`AnimeDownloadQueueScreenModel.kt`)

- Created with `rememberScreenModel { AnimeDownloadQueueScreenModel() }` (`AnimeDownloadQueueTab.kt:358`).
- `init`: `screenModelScope.launch { queueState.map(...).collect { _state.update { it } } }`.
- `onDispose()`: cancels all progress jobs, clears them, sets `adapter = null`.
- `isDownloaderRunning`: `AnimeDownloadManager.isDownloaderRunning` (WorkManager RUNNING flow) as `StateFlow` with `WhileSubscribed(5000)`. This drives the Stop/Continue FAB in the spec (§7). In this scope the flow is exposed but no composable reads it.
- `startDownloads()`, `pauseDownloads()`, `clearQueue()`: manager pass-throughs.

---

## 3. Keyboard, mouse and touch equivalents for Linux

| Android gesture | Desktop equivalent (proposal) |
|---|---|
| Drag by handle | Press and drag the handle (left button) within the group. Visual drop indicator line between rows. |
| Long-press row to open menu | Right-click the row, or press `Menu` / `Shift+F10` with the row focused. |
| Overflow button `menu` | Button at row end (`view-more-symbolic`), click opens the same menu. |
| Tap row | Activate row (Enter or click): nothing in the queue (no playback from the queue in Kotlin). Proposal: open the entry (same as "Show anime" action). |
| Keyboard reorder (not in Kotlin) | `Ctrl+Up` / `Ctrl+Down` moves the focused row within its group. Proposal, for accessibility. |
| Cancel | `Delete` key on a focused row cancels that download (proposal; no confirmation, same as the menu). |
| Stop/Continue FAB | Header-bar button or footer button, activated by click or `Ctrl+P` (proposal, bound to the same action). |
| Pull-to-refresh | Not present in Kotlin for this page; not added. |

Focus order: the list is a `ListBox`, so Up/Down move focus between rows and headers are skipped (they are not focusable, matching `isSelectable = false`).

---

## 4. Desktop-specific additions (proposals, not in Kotlin)

- Error glyph: rows in ERROR show a `dialog-warning-symbolic` icon before the progress text, with a tooltip containing the last error message (the Kotlin shows nothing).
- Progress bar colour: use the accent colour for DOWNLOADING and the error colour for ERROR rows. Keep the GTK theme, no hard-coded colour (per the user's global theme preference: Tokyo Night everywhere, handled by the theme task area).
- Row tooltip: full anime title and episode name when ellipsised.
- Status bar at the bottom of the page: "N queued · M running · K failed" (proposal; the Kotlin has the count in the title pill only).

---

## 5. Responsive behaviour

- Kotlin: the same list layout on phone and tablet. `contentPadding` accounts for system bars and the bottom navigation.
- Desktop: the page fills the content area. Rows are full width. Title and episode name ellipsize at the end. The progress bar keeps a fixed 120 px minimum width so the bar stays visible at narrow widths (proposal).
- Below 600 px content width, hide the progress text column and show it under the episode name instead (proposal).
- Scroll: the list scrolls vertically only. No horizontal scroll.

---

## 6. Animations and transitions

- Header collapse on group drag (`collapseAll`) and expand on release (`expandAll`): this is Kotlin's visual feedback for drag. Desktop: keep groups expanded, and show a drop marker instead. Document as a deliberate change.
- Progress bar animation: `setProgressCompat(progress, animated = true)`. Desktop: GTK `ProgressBar` animates implicitly on `set_fraction`; no extra work.
- Indeterminate bar: Material indeterminate ring. Desktop: `ProgressBar.pulse()` every 100 ms when progress is 0.
- Tab content change: the mockup `kind_tabs` already switches pages. No change.

---

## 7. What the spec adds that this scope does not implement

From `spec/app.json` id `download_queue` (notes field) and owned by area:25-downloads-shared (`DownloadsTab.kt`), not by this scope:
- Top app bar: back arrow, title "Download queue", count pill of queued items in the current tab.
- Sort icon: dropdown "By upload date" (Newest / Oldest) and "By episode number" (manga: "By chapter number") with Ascending / Descending. Visible only when the queue is non-empty.
- Overflow: "Cancel all" (only when non-empty).
- Tab row: "Anime" | "Manga", each tab with a count badge.
- Extended FAB, bottom-end, only when the list is non-empty, collapses on scroll: anime tab "Stop" (Pause icon) when running, "Continue" (Play icon) when stopped; manga tab "Pause" / "Resume". Empty state text "No downloads".

These are required for parity. They are wired to this scope's manager API (`startDownloads`, `pauseDownloads`, `clearQueue`, `reorderQueue`). Tasks T-24a-044 and T-24a-045 cover the anime-side pieces. The chrome itself belongs to area:25-downloads-shared.

---

## 8. Gap analysis: current mockup versus Kotlin

Mockup function: `src/screens.rs:354` `fn downloads() -> Option<Built>`, dispatched from `src/screens.rs:25` (`"download_queue" => downloads()`). Built with `kind_tabs`, `heading`, `boxed_list`, `list_row`, `menu_button`, `gtk::ProgressBar` (helpers in `src/ui.rs`).

| Aspect | Kotlin (this scope or spec) | Mockup today | Gap |
|---|---|---|---|
| Data source | live `queueState` | hard-coded groups "Source A (3)", "Source B (2)" with `title_n(i)` rows | Replace with manager snapshot |
| Group headers | "source name (count)", drag handle at end, collapse on drag | "Source A (3)" heading text, no handle | Headers from groups; handle; DnD |
| Row title | anime title (line 1), episode name (line 2) | `title_n(i)` and `unit` (`Episode N` / `Chapter N`) | Anime title and real episode name |
| Progress | bar: indeterminate at 0, determinate otherwise | `set_fraction(0.45)` fixed | Live value, pulse at 0 |
| Progress text | "Downloading…" at 0, "N%" otherwise | "45%" (anime) or "12/30" (manga) | Use the two message keys; manga text is not applicable to this area |
| Row menu | six entries, visibility rules, actions | six labels, no visibility rules, no actions (`\|_\| {}` closure) | Actions and rules (§1.6) |
| Reorder | drag within group, series moves | none | Implement (§1.7) |
| Cancel | per row and per series | none | Wire to `Command::Cancel` / `CancelAnime` |
| Empty state | "No downloads" | none | Add `ui::status` with the message |
| FAB Stop/Continue | spec, anime tab | none | Add (§7, T-24a-044) |
| Sort | spec dropdown, `reorderQueue` | none | Add (§7, T-24a-045) |
| Error state | last text kept, no flag (Kotlin) | n/a | Add glyph and tooltip (proposal, §4) |
| Live updates | status and progress flows | none | Row `watch` receivers (rust design §10) |
| Count | sum of queued items in the tab title | none | Add count pill |
| Notification | ID -203 progress, -204 error | none | Notifier (T-24a-026, T-24a-027) |

Items the mockup already has that match Kotlin: group-header and row layout skeleton, the six menu labels, the `list-drag-handle-symbolic` icon (Kotlin uses `ic_drag_handle_24dp`), the `view-more-symbolic` overflow icon (Kotlin uses `ic_overflow_24dp`).

---

## 9. Strings used by this page (keys and English text)

| Key | Namespace | English text |
|---|---|---|
| `label_anime` | AYMR | Anime (tab title) |
| `information_no_downloads` | MR | No downloads |
| `update_check_notification_download_in_progress` | MR | Downloading… |
| `episode_download_progress` | AYMR | %1$d%% |
| `action_move_to_top` | MR | Move to top |
| `action_move_to_top_all_for_series` | MR | Move series to top |
| `action_move_to_bottom` | MR | Move to bottom |
| `action_move_to_bottom_all_for_series` | MR | Move series to bottom |
| `action_cancel` | MR | Cancel |
| `cancel_all_for_series` | MR | Cancel all for this series |
| `action_menu` | MR | (content description of the overflow button) |
| `download_queue` title, `Stop`, `Continue`, `Pause`, `Resume`, `Cancel all`, `Sort` | spec / area:25-downloads-shared | per spec notes |

Strings from notifications are in `p1-behavior.md` §14.

---

## 10. Accessibility

- Every row has a tooltip with title and episode name (proposal) and an accessible name equal to "Title, episode, status, progress".
- The menu button has accessible name "Actions for <episode name>" (Kotlin uses the generic `action_menu`).
- Progress bars expose their value through GTK accessibility (`ProgressBar` is accessible by default).
- Headers are announced as group headings (`gtk::AccessibleRole::Heading`) (proposal).
- Colour is never the only indicator: ERROR and DOWNLOADED have icons and text (§4).
