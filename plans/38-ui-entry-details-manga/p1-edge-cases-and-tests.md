# Area 38 part 1: edge cases, failure modes, races, upstream tests, new tests

Every ID `E-nn` here is referenced from the behavior spec and rust design. Source paths are repo-relative to `/home/blackbox/aniyomi-src`.

Upstream unit tests: `find app/src/test domain/src/test -name "*Test.kt"` was not exhaustively re-checked in this pass for the scope files. The scope files contain **no** `*Test.kt` files and no `@Preview`-only logic except `MissingItemCountListItem.kt:44-52` (a preview, no assertions). Tests below are therefore new tests; each cites the logic it protects.

---

## 1. Edge cases (UI and state)

### E-01 Header count uses a non-null count
- Observed: both call sites pass `itemCount = chapters.size` (presentation MangaScreen.kt:448 and 692). `ItemHeader` only shows "Chapters" when `itemCount == null` (ItemHeader.kt:43-45), which is unreachable from manga.
- Expected behaviour: zero chapters shows `"0 chapters"` (plural `manga_num_chapters`, `%1$s chapters` other form).
- Rust: never show the bare "Chapters" label for manga; only for the loading placeholder if ever needed.

### E-02 All chapters filtered out
- processed list empty → header shows "0 chapters"; missing count 0 (no warning); FAB hidden (`processed.any { !read }` false); bulk bar unaffected.
- Still counts as `filterActive` (filters set) → filter icon tinted.
- Proposal: show an in-list hint "No chapters match the current filters" (new string, labelled proposal). Aniyomi shows nothing.

### E-03 Unread filter and "Resume" mismatch
- FAB visible iff processed has an unread item (`MangaScreen.kt:346-348`). Label is "Resume" iff unprocessed `state.chapters` has any read item (`:356-358`). With filter "Unread" on and everything read, the FAB is hidden while the label logic is unaffected. Port the two inputs exactly.

### E-04 Selection hidden by filter
- `isAnySelected` (unprocessed) can be true while the processed selection count is 0.
- Toolbar action mode uses processed count `chapters.count { selected }` (presentation MangaScreen.kt:292-294 / 547-549). If that count is 0 the toolbar leaves action mode even though back-handler and pull-refresh still treat the list as selecting.
- Bottom bar visible from processed selection (`EntryBottomActionMenu visible = selected.isNotEmpty()`).
- Rust: define two counters `selected_visible` and `selected_total`. Toolbar action mode = `selected_visible > 0`; back handling and pull-refresh disable = `selected_total > 0`. Document in tests (T-38a list).

### E-05 Blank versus null text
- `description.takeIf { !it.isNullOrBlank() }` (MangaInfoHeader.kt:249-251) — whitespace-only description shows "No description".
- Author/artist: `author?.takeIf { it.isNotBlank() } ?: "Unknown author"`. Whitespace-only author shows "Unknown author" but the click guard is `!author.isNullOrBlank()`.
- Title: `title.ifBlank { "Unknown title" }` (`ifBlank` falls back when `isBlank()` is true, i.e. empty or whitespace-only).
- Scanlator: `takeIf { !it.isNullOrBlank() }` (presentation MangaScreen.kt:807). Whitespace-only scanlator is hidden.
- Rust: `str::trim().is_empty()` for all the above.

### E-06 Blank title interactions
- Blank title: display "Unknown title"; click does nothing (guard `title.isNotBlank()`); long-press does nothing.
- Rust: same guards.

### E-07 Chapter number formatting
- `formatChapterNumber` uses `DecimalFormat("#.###", DecimalFormatSymbols with '.')` (presentation/util/ItemNumberFormatter.kt:6-12). Rules: `#` = optional digit, no grouping (no grouping pattern), max 3 fraction digits, HALF_EVEN rounding (Java default), no minimum fraction digits, so `1.0 → "1"`, `1.5 → "1.5"`.
- Test values (generate expected strings by running Java `DecimalFormat` once into the scratchpad; do not hand-write): `1.0`, `1.5`, `10.25`, `1.2345`, `1.2355`, `0.0005`, `-1.0`, `-0.0`, `1e15`, `NaN`, `Infinity`.
- `-1.0` (unknown number) prints `-1` in the title when display mode is number. Keep this (it is observable Aniyomi behaviour), flagged as a known quirk.

### E-08 Missing count with unknown numbers
- `missingChaptersCount` drops exact `-1.0` (MissingChapters.kt:12-13) but not other negatives. `-0.5` becomes `0` after `toInt()` and is then counted as chapter 0 (`0 > prev+1` false for prev 0). Keep literal behaviour; test it.
- `16.5` is truncated to `16`; two chapters `16` and `16.5` count once (distinct after truncation).

### E-09 Descending separators
- For descending sort the separator logic swaps lower/higher (MangaScreenModel.kt:1168-1170). A gap between `5` and `3` in descending order (list order `5, 3`) yields `count = floor(5) - floor(3) - 1 = 1` (chapter 4 missing). Test both orders.

### E-10 Alphabetical ties
- `sortedWith` is stable. Two chapters with identical names keep their DB order. Rust `sort_by` is stable.

### E-11 Source-order ties
- Equal `sourceOrder` keep DB order (stable).

### E-12 Bottom bar 1-second label reveal race
- `EntryBottomActionMenu.kt:109` declares `var resetJob: Job? = remember { null }`. The `remember` returns the same null on each recomposition, and the local `var` is re-read from the remembered value each time. Result: `resetJob?.cancel()` in `onLongClickItem` does not cancel a previous job reliably. A quick second long-press on the **same** button can have its label hidden by the first job's timer 1 s after the first press (early hide). A long-press on button A followed by button B within 1 s leaves button A's timer running; when it fires it sets `confirm[A] = false` only (B is unaffected). Both are cosmetic.
- Rust: keep one `glib::SourceId`/`JoinHandle` in the view struct; cancel on every new long-press; the timer sets only the current index's visibility to false. Label reveal is 1000 ms (`1.seconds`).
- Also: the tap action fires regardless of the reveal (no confirmation). Keep it (Aniyomi behaviour) and add "undo" only as P2.

### E-13 Library button long-press when not favorite
- `onLongClick = onEditCategory` is null for non-favorite (`MangaInfoHeader.kt:198` and ui MangaScreen.kt:160). No long-press action. Rust: no gesture controller attached.

### E-14 Interval button when not favorite
- Click calls `onEditIntervalClicked?.invoke()` — null for non-favorite → no dialog (MangaInfoHeader.kt:212; ui MangaScreen.kt:161-163). The label still shows the computed next-update days.

### E-15 Downloaded filter with "downloaded only" preference
- `BasePreferences.downloadedOnly()` true → `Manga.downloadedFilter` returns `ENABLED_IS` regardless of stored flags (app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:25). `FilterPage` passes `onDownloadFilterChanged.takeUnless { downloadedOnly }` (ChapterSettingsDialog.kt:105-106), so the row is disabled (`TriStateItem` `onClick == null` → not clickable).
- Test: with the preference on, toggling Downloaded in the sheet does not change flags; list shows only downloaded chapters.

### E-16 Completed manga next-update text
- `expectedNextUpdate` is null when `status == 2` (Manga.kt:17-23). Action row shows "N/A". Interval dialog shows `manga_interval_expected_update_null`.
- Unknown status `0` still returns a date (only `COMPLETED` nulls it).
- Past `nextUpdate` → negative days → coerced to 0 → "Soon" (`manga_interval_expected_update_soon`).

### E-17 `copyMangaUrl` without try/catch
- Observed: `copyMangaUrl` (ui MangaScreen.kt:407-412) calls `source.getMangaUrl(...)` without a guard. If the source throws, the exception is not caught in this function (every other URL helper in the same file wraps it in try/catch, MangaScreen.kt:315-324 and 338-352).
- Rust: guard; on error log and toast "Error" text. Note as a deliberate deviation.

### E-18 Edit cover with unreadable stream
- `contentResolver.openInputStream(data)` null → nothing happens (no snackbar), MangaCoverScreenModel.kt:123. Rust: show `notification_cover_update_failed` so the user is not left without feedback (proposal, labelled).

### E-19 Edit cover picker cancelled
- `GetContent` returns null → `return@rememberLauncherForActivityResult` (ui MangaScreen.kt:264). No state change.

### E-20 Share with null or throwing URL
- `getMangaUrl` returns null on any error (ui MangaScreen.kt:315-324) → share does nothing (no toast).
- Exceptions from building the intent go to `context.toast(e.message)`; `e.message` may be null → the toast call receives null. Rust: show the generic error text when message is absent.

### E-21 Search with fewer than two screens
- `performSearch` returns silently when `navigator.size < 2` (ui MangaScreen.kt:365-367). Global search still pushes (global branch returns earlier, line 360-363). Rust: non-global search with no previous page must do nothing, not panic.

### E-22 Search from a screen that is neither Home nor Browse
- Non-global search does nothing (no pop, no search). E.g. opened from history: clicking the title (global) works; clicking the source name (non-global) does nothing. Rust: keep. Proposal: fall back to global search (labelled new behaviour) — default: keep Aniyomi behaviour.

### E-23 Migrate completion replaces the screen
- `onPopScreen = { navigator.replace(MangaScreen(dialog.newManga.id)) }` (ui MangaScreen.kt:230). The old details page is replaced, not stacked. Rust: `ui::pop()` then `ui::push(new_page)`.

### E-24 Duplicate "Show entry" stacks
- `onOpenManga = { navigator.push(MangaScreen(dialog.duplicate.id)) }` (ui MangaScreen.kt:216) pushes; back returns to the current entry.

### E-25 Tracking button with no logged-in tracker
- Opens tracking settings instead of the sheet (ui MangaScreen.kt:146-148). Count is shown as "Tracking" when `trackingCount == 0` even if trackers exist but are not logged in (the label and the sheet decision use different inputs). Preserve both.

### E-26 Stub source
- `isStubSource` shows a warning icon in error color (MangaInfoHeader.kt:537-546). `isHttpSource` is false for stub (not HttpSource), so WebView, share, copy-URL are hidden.

### E-27 Local manga
- Download indicator disabled (`downloadIndicatorEnabled = !isAnyChapterSelected && !manga.isLocal()`), bulk Download hidden in the toolbar (`onDownloadChapter` null when `isLocalOrStub`), swipe Download still renders with state icon (`downloadState` is DOWNLOADED for local so the swipe shows Delete).
- `toChapterListItems` sets `downloadState = DOWNLOADED` for every chapter of a local manga (MangaScreenModel.kt:536-556), so the bulk bar shows Delete for local chapters. Check that the delete path is allowed for local (part 2).
- Downloaded filter keeps all chapters for local manga (`isLocalManga` in applyFilters).

### E-28 Early-hints HTTP 103
- `fetchMangaFromSource` ignores `HttpException` with code 103 (MangaScreenModel.kt:286-288). Rust: ignore status 103 explicitly (reqwest does not surface 1xx; document that the rule is a no-op in Rust and keep the check in the HTTP layer if it ever surfaces).

### E-29 Long titles
- Chapter title max 1 line with ellipsis (MangaChapterListItem.kt:131-138). Subtitle parts max 1 line each (lines 149-170). Header title (`titleLarge`) wraps. Source name max 1 line.

### E-30 Right-to-left layout
- Not verified in this pass: `SwipeableActionsBox` start/end may be layout-direction aware. Port: physical left swipe = Kotlin `startActions`? Treat start as left-to-right in LTR and mirror in RTL. Mark as unverified; add a test when RTL is enabled.

### E-31 Description expansion state across page open
- `rememberSaveable` survives recomposition and config change but not app restart. Rust: store on the page object; reset on new page. Small layout default = `isFromSource`; large = `true`.

### E-32 Mark previous as read only for one selected
- `takeIf { selected.size == 1 }` (presentation MangaScreen.kt:745-746). Two selected → no button.

### E-33 Bookmark / unbookmark mixed selection
- Bookmark visible iff any not bookmarked; unbookmark visible iff all bookmarked. Mixed selection shows only "Bookmark", which sets all to bookmarked.

### E-34 Mark unread for partially read chapters
- Unread visible iff any `read || lastPageRead > 0` (presentation MangaScreen.kt:743). A chapter with progress but not read can be reset.

### E-35 Progress text bounds
- Shown only when `!read && lastPageRead > 0` (MangaChapterListItem call site: MangaScreen.kt:800). Value is `lastPageRead + 1` (page number, 1-based display).

### E-36 Bookmarked and read
- Bookmark icon shown regardless of read state; title alpha is `DISABLED_ALPHA` when read. Bookmark icon color is primary, not dimmed (the icon is not inside the alpha-applied Text; it is an Icon with explicit tint `colorScheme.primary`). Keep.

### E-37 Error state download
- Click and long-press both START (ChapterDownloadIndicator.kt:235-239). No menu.

### E-38 Downloading with zero progress
- Indeterminate spinner (`downloadState == DOWNLOADING && progress == 0`, ChapterDownloadIndicator.kt:133-134). Once progress > 0, ring with arrow.

### E-39 Progress greater than 100
- `MangaDownload.totalProgress` is the sum over pages (MangaDownload.kt:~24-28); `downloadProgress / 100f` may exceed 1. Compose clamps arc drawing. Rust: clamp fraction to [0,1] (`indicator_view`).

### E-40 Non-Latin alphabetical sort
- `Collator.PRIMARY` with default locale (SortUtil.kt:6-15). Test a Japanese set, an Arabic set, a Cyrillic set, and accented Latin (é vs e must compare equal at PRIMARY).

### E-41 Zero upload date
- `relativeDateTimeText(0)` → `takeIf { dateEpochMillis > 0L }` yields `null` local date-time → `toRelativeString(null)`. Output for null is not read in this pass; test the Rust output against Aniyomi by running the Kotlin function once if feasible, otherwise assert "unknown date" proposal string.

### E-42 Scanlator filter with empty set
- Empty `availableScanlators` → dialog shows "No scanlators found" and only Cancel (ScanlatorFilterDialog.kt:50-53, 105-107).

### E-43 Excluded scanlator no longer available
- `scanlatorFilterActive` ignores stale excluded entries (intersection). The dialog still lists only available ones; stale excluded names are kept in storage (not pruned here).

### E-44 Select all / reset label
- Shows "Select all" when excluded set is empty; shows "Reset" otherwise, even if only one is excluded (ScanlatorFilterDialog.kt:111-119).

### E-45 Interval initial value
- `selectedInterval = if (interval < 0) -interval else 0` (ItemsDialogs.kt:77). Default `0` → wheel shows "Default".

### E-46 Interval in release builds
- `!isReleaseBuildType` gate hides the wheel (ItemsDialogs.kt:126). Release users see only the informational text and an OK that dismisses. Rust: replicate the gate as a debug-build cfg (`cfg!(debug_assertions)`).

### E-47 Interval gate by restriction
- `onValueChanged` null when `manga_outside_release_period` not in restriction set (MangaScreenModel.kt:150-151, ui MangaScreen.kt:291-292). Then the wheel is hidden even in debug builds, because the wheel requires `onValueChanged != null` (ItemsDialogs.kt:126).

### E-48 Set-as-default checkbox
- The "also apply to library" checkbox defaults to unchecked (`optionalChecked = false`, ChapterSettingsDialog.kt:236). Confirm always stores the current manga settings as default; the checkbox only controls `setMangaDefaultChapterFlags.awaitAll()` (MangaScreenModel.kt:951-962).

### E-49 Delete custom cover failure
- `deleteCustomCover` errors → `notification_cover_update_failed` snackbar and error logged (MangaCoverScreenModel.kt:141-143).

### E-50 FAB with partial read
- Label "Resume" if any chapter has `read == true` in the **unfiltered** list. A filter hiding the read chapters still shows "Resume". Keep (E-03).

### E-51 Row interactions
- Non-selection long-press → select this row (`onChapterSelected(item, !selected, true, true)`). Selection mode click toggles. Click on a selected row deselects. Verified in `onChapterItemClick` (presentation MangaScreen.kt:842-853).

### E-52 Linux right-click vs left click
- Ensure the secondary-button gesture does not also trigger the primary click (use `GestureClick` with `button` property and `set_propagation_phase`). Test with a mouse.

### E-53 Selection survives snapshots
- Observed: `toChapterListItems` sets `selected = chapter.id in selectedChapterIds` (MangaScreenModel.kt:563), so a DB or download snapshot does not clear selection.
- Stale ids: `selectedChapterIds` is never pruned when a chapter disappears; `toggleSelection` ignores missing ids (`selectedIndex < 0`, MangaScreenModel.kt:977). Rust: keep the selected set in the model; prune on snapshot to avoid counting ghosts in "Select all".

### E-54 Download state changes during selection
- `updateDownloadState` replaces one item (MangaScreenModel.kt:519-531) and keeps `selected` (copy). Rust: patch by id; do not rebuild the whole model.

### E-55 Chapter removed while selected
- `toggleSelection` returns early when the id is not found (MangaScreenModel.kt:976-978). Bulk actions use the currently selected items at the time of the click.

### E-56 Mark previous as read in descending order
- Uses `filteredChapters` (processed list) with `asReversed()` when `sortDescending()` (MangaScreenModel.kt:741-752, part 2). Test with descending sort and filter on.

### E-57 Separator with empty processed list
- No separators (loop yields nothing); no crash.

### E-58 Large lists
- 10 000 chapters: processed list recomputed on each state change. Rust: recompute only on snapshot or filter/sort change; keep `ListView` virtualization. Target: under 16 ms for filter+sort of 10 000 items (perf test T-38a).

### E-59 Responsive switch with scroll
- Crossing the breakpoint rebuilds layout; preserve scroll offset of the chapter list and the current selection.

### E-60 Interval text rounding
- `until(…, DAYS)` truncates, so 23 h → 0 days → "Soon" (MangaInfoHeader.kt:182). 1 day 23 h → "1 day". Test.

### E-61 Expanded description in large layout
- Large layout always starts expanded (`defaultExpandState = true`, presentation MangaScreen.kt:662); small starts expanded only when `isFromSource`. Test both.

### E-62 Title fade with partial scroll
- Background alpha is 1 when first item is not visible or first item scrolled by > 0px (MangaScreen.kt:305-308). Title alpha is 1 only when first item not visible (line 301-302). Test the four cases.

### E-63 Disabled swipe side
- `Disabled` returns null → no action on that side (MangaChapterListItem.kt:217).

### E-64 Swipe download while downloaded
- Icon Delete; `onSwipe` → `chapterSwipe(item, Download)` → part 2 decides delete (`executeChapterSwipeAction`, MangaScreenModel.kt:614-646).

### E-65 Migrate when not favorite
- Hidden in toolbar and action row (favorite-only). Migrate remains reachable from the duplicate sheet (E-24 family). Test.

### E-66 `fromSource` flag
- Only affects default description expansion in the small layout (presentation MangaScreen.kt:431). Test.

### E-67 Snapshot with manga removed from library
- `toggleFavorite(onRemoved = ...)` path shows "Delete downloaded chapters?" when downloads exist (part 2). This area only triggers it. Test that the removal path leaves the page open and updates the library button.

### E-68 Blank-title search
- Title click guard prevents searching an empty string (`if (title.isNotBlank())`).

### E-69 Author equal to artist
- Artist row hidden when `author == artist` (MangaInfoHeader.kt:477). If both null: `!artist.isNullOrBlank()` false → hidden.

### E-70 Genre list empty or null
- `tags.isNullOrEmpty()` hides the whole tag box (MangaInfoHeader.kt:267-268) including its padding.

### E-71 Very long genre list
- Collapsed mode is a horizontal LazyRow (no wrap); expanded wraps. Rust: horizontal scroll row and wrapping FlowBox; never overflow the window width.

### E-72 Cover placeholder and error
- Placeholder color `0x1F888888` (ItemCover.kt:55), error drawable `cover_error` (ItemCover.kt:35). Rust: placeholder CSS class and the error icon; no spinner.

### E-73 Duplicate check race
- The duplicate dialog only appears for the add path (part 2 `toggleFavorite(checkDuplicate = true)`, MangaScreenModel.kt:314-345). "Add anyway" calls `toggleFavorite(onRemoved = {}, checkDuplicate = false)` (ui MangaScreen.kt:215).

### E-74 Dialog chaining after category change
- After `ChangeCategory` dismiss, if auto-open tracking is on and the change came from category flow, the track sheet opens (ui MangaScreen.kt:181-183). Test with `trackOnAddingToLibrary` on.

---

## 2. Failure modes and races (summary table)

| Scenario | Failure | Mitigation |
|---|---|---|
| Source down on first open | `fetchMangaFromSource` error → snackbar with message; `isRefreshingData` cleared at end | Show toast; keep cached manga; spinner off |
| DB write fails while selecting many | Flag write error; UI keeps optimistic? Kotlin does not; snapshot stays old | Toast error; re-subscription shows real state |
| Page closed during fetch | `fetchAllFromSource` runs in `screenModelScope.launch` → cancelled | Cancel fetch on page drop (tokio abort) |
| Page closed during flag write | `launchNonCancellable` → write completes | Do not abort writes (rust design section 3) |
| Two rapid sort clicks | Each is a separate write; second flips the first's result | Serialize flag writes per manga (single worker queue) |
| Cover edit while cover dialog open | `getManga.subscribe` updates `manga` → dialog re-renders | Subscribe in cover model; re-render image on `cover_last_modified` change |
| Chapter list emits during drag-swipe | Row rebuild cancels the gesture | Patch row widget in place (stable `ListView` model id) |
| Download progress storm | Many snapshots per second | Coalesce to 4 Hz for the progress ring; flags/titles immediate |
| Search navigation with stale navigator | `navigator.items[size-2]` could be a different type | Check type each time; no-op if mismatch (E-22) |

---

## 3. Upstream tests to port

The scope has **no** `*Test.kt` files (`find` over the 19 scope paths returns none). Logic-level tests exist upstream for the domain functions used here; the Rust port must adopt equivalent cases. To find them, run in the source tree (not done in this pass):

```
find domain app -name "*Test.kt" | xargs grep -l "missingChaptersCount\|getChapterSort\|applyFilter\|setFlag\|Chapter" 2>/dev/null
```

Until that is run, the Rust port must still include the cases in section 4, which are derived from the logic read in this pass.

---

## 4. New tests to write (Rust, GTK-free where possible)

Each test name is a required acceptance criterion for the listed task. Expected values are from reading the Kotlin, not from running it, unless marked **[generate]**.

### 4.1 flags.rs
- `flags_default_zero_means_source_desc_name_display`: `ChapterFlags(0)`: `sort_mode == Source`, `sort_descending == true`, `unread == Disabled`, `bookmarked == Disabled`, `display_number == false`.
- `select_sort_new_mode_resets_asc`: from `ChapterFlags(0)`, `select_sort(Number).0 == 0x101`.
- `select_sort_same_mode_flips`: `ChapterFlags(0x101).select_sort(Number).0 == 0x100`; again → `0x101`.
- `with_preserves_other_bits`: `ChapterFlags(0x0010_0000 | 0x80 | 0x300).with(0x2, UNREAD_MASK).0 == 0x0010_0000 | 0x80 | 0x300 | 0x2`.
- `unread_mask_values`: `0x2 → EnabledIs`, `0x4 → EnabledNot`, `0x6 → Disabled` (both bits set is not a Kotlin-defined state; the Kotlin `when` yields `DISABLED` for 0x6 via the else branch: test that 0x6 maps to Disabled).
- `display_number_bit`: `0x0010_0000` → true.
- `tristate_next_cycle`: `D→IS→NOT→D`.
- `tristate_keep_table`: for each of three states and predicate true/false: check.

### 4.2 compare.rs
- `source_desc_true_is_ascending_source_order`: `[sourceOrder 2,1,3]` sorted with `(Source, true)` → `[1,2,3]`.
- `source_desc_false_is_descending`: same with false → `[3,2,1]`.
- `number_desc`: numbers `[1,10,2]` with `(Number,true)` → `[10,2,1]`.
- `upload_desc`: dates similarly.
- `alpha_asc_and_desc_with_fake_collator`: identity collator → lexicographic.
- `equal_keys_keep_order` (stable): `[("a",id1), ("a",id2)]` alpha asc → `[id1,id2]`.
- `number_total_order_nan_safe`: with NaN values, no panic.

### 4.3 chapters.rs
- `process_pipeline_order`: items with mixed read/bookmark/downloaded; assert result for `unread=EnabledIs`, `bookmark=EnabledNot`, `downloaded=EnabledIs`.
- `process_local_keeps_all_for_downloaded`: `is_local=true` and `downloaded=EnabledIs` keeps items with `NotDownloaded` state.
- `missing_count_basic`: numbers `[1,2,4,5,8]` → `missing_chapters_count` = 3 (missing 3, 6, 7). Hand-derived from MissingChapters.kt:6-38; confirm with one Kotlin run before freezing the test.
- `missing_count_ignores_unknown`: `[-1.0, 1.0, 3.0]` → 1 (chapter 2).
- `missing_count_truncates_fractions`: `[1.0, 1.5, 3.0]` → `toInt` gives `[1,1,3]`, distinct `[1,3]` → 1.
- `missing_count_empty` → 0.
- `separators_ascending_first_gap`: list numbers `[3, 4]` asc → first separator count `floor(3) - 1 = 2`; id `"null-<id3>"`.
- `separators_between_items_asc`: `[1, 4]` → separator between 1 and 4: count `floor(4) - floor(1) - 1 = 2`, id `"<id1>-<id4>"`.
- `separators_desc_swaps`: `[4, 1]` desc → between: lower = `after` = item(1), higher = `before` = item(4): count 2, id `"<id1>-<id4>"`.
- `separators_none_for_unknown_number`: chapter with number -1 → `calculate_chapter_gap` returns 0.
- `separators_none_at_end`: last position has no separator (higher is None).
- `separators_empty_list` → rows empty.

### 4.4 numfmt.rs
- `format_integer`: `1.0 → "1"`.
- `format_half`: `1.5 → "1.5"`.
- `format_three_decimals`: `1.2345 → "1.234"` **[generate]** against Java HALF_EVEN on exact double (1.2345 is 1.23449999... so `1.234`); assert after generation.
- `format_negative_one`: `-1.0 → "-1"`.
- `format_neg_zero`: `-0.0 → "0"` **[generate]** (Java prints `-0`; confirm: DecimalFormat prints "-0" for -0.0). **[generate]** — record the Java output and match it exactly, even if odd.
- `format_nan` / `format_inf` **[generate]**.

### 4.5 download.rs
- `indicator_view_table`: each state/progress pair.
- `outcome_table`: each state/gesture pair against section 2.5.
- `progress_clamp`: `progress = 150` → fraction 1.0, arrow_on_fill true.

### 4.6 swipe.rs
- `visual_toggle_read`: `read=false` → `object-select-symbolic`, undo=false; `read=true` → undo=true.
- `visual_download_states`: map table.
- `disabled_returns_none`.

### 4.7 text.rs
- `collapse_two_newlines`: `"a\n\nb"` → `"a\nb"`.
- `collapse_keeps_single`: `"a\nb"` → unchanged.
- `collapse_crlf`: `"a\r\n\r\nb"` → `"a\nb"` (the run `\r\n\r\n` is 4 chars, all matched `[\r\n]{2,}`).
- `trim_end`: trailing spaces and newlines removed; leading kept.
- `placeholder_blank`: `Some("   ")` → "No description".

### 4.8 model.rs
- `filter_active_scanlator_intersection`: excluded `{X}`, available `{Y}` → false; available `{X,Y}` → true.
- `filter_active_manga_flags`: unread set → true.
- `is_any_selected_uses_unfiltered`.
- `selected_visible_vs_total` (E-04).
- `downloaded_only_overrides`: `downloaded_only=true`, flags `NOT_DOWNLOADED` → effective `EnabledIs`.

### 4.9 Interval and time
- `days_until_truncates`: next = now + 36h → 1; next = now + 23h → 0; next = now − 2d → 0.
- `interval_initial_index`: stored `-3` → 3; stored `0` → 0; stored `5` → 0.

### 4.10 Cover model (behavior 8.5)
- `save_success_message`: success → `cover_saved`.
- `save_failure_message`: error → `error_saving_cover`.
- `edit_null_stream_message`: proposal `notification_cover_update_failed` (E-18).
- `delete_custom_failure`: `notification_cover_update_failed`.

### 4.11 Performance
- `perf_10k_process_under_budget`: generate 10 000 items, run `process` + `with_missing_markers`; budget 50 ms in release (CI-skipped in debug).

### 4.12 GTK smoke (manual / script)
- Open manga details with fixture data, toggle every menu item, assert no panic (uses `KUROBAYOMI_SMOKE`).

---

## 5. Fixtures needed

| Fixture | Content | Used by |
|---|---|---|
| `fixtures/manga_basic.json` | one manga, 30 chapters, numbers 1..30 with 5 gaps, mix of read/bookmark/downloaded | pipeline, separators, UI smoke |
| `fixtures/manga_desc_asc.json` | same with descending flag | E-09 |
| `fixtures/manga_unicode_titles.json` | titles in Japanese, Arabic, Cyrillic, accented Latin | E-40 |
| `fixtures/numbers.txt` | list of double literals for formatting | E-07 (generated by Java) |
| `fixtures/local_manga.json` | local manga with all-downloaded chapters | E-27 |
| `fixtures/scanlators.json` | available/excluded sets incl. stale entries | E-42..E-44 |

Fixtures live under `/home/blackbox/kurobayomi/tests/fixtures/details/` (proposal). No real user data.

---

## 6. Open questions (raised, not blocking)

1. `relativeDateTimeText` output for a zero date (E-41) — needs one Kotlin run to pin.
2. Exact Java output for `-0.0`, NaN, infinity (E-07) — needs one Java run.
3. Whether `SwipeableActionsBox` start/end flip under RTL (E-30) — needs a check of the library source.
4. `EntryDownloadDropdownMenu` labels (behavior 7.6) — read its file in the next pass.
