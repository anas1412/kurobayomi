# 38 part 2: edge cases, failure modes, races, upstream tests, and new tests

Each entry has an ID used by the task list and the behavior document. "Upstream" is the Kotlin behavior; "Port" is what the Rust implementation must do. Where Port differs from upstream, the entry is a proposal that needs the user's decision (see p2-ui-parity.md section 9).

---

## 1. Edge cases and failure modes

### 1.1 Loading and refresh

**E-LOAD-1 Missing manga row stays Loading forever.**
- Upstream: `awaitManga` (GetMangaWithChapters.kt:24-26 -> `mangaRepository.getMangaById`) throws if the row is gone; the exception ends the `launchIO` block; state stays `Loading` (MSM:215-239).
- Port: `LoadManga` returns `Ok(None)` -> `LoadState::NotFound` with a message and a Back action. Never stay in Loading on error.
- Task: T-38b-003.

**E-LOAD-2 First paint uses pre-default flags.**
- Upstream: `setMangaDefaultChapterFlags.await(manga)` (MSM:220-222) runs after the manga was read, and `chapters` is computed before it. `Success.manga` holds the old flags until the next DB emission.
- Port: apply defaults first (or re-read the manga after the defaults write) before building the initial `Ready`.
- Task: T-38b-003, T-38b-014.

**E-LOAD-3 Initial fetch decision uses the pre-fetch manga.**
- Upstream: `needRefreshInfo = !manga.initialized` is read once (MSM:224). A failing source leaves `initialized = false`, so the next open fetches again. Correct, but a source that keeps failing causes a fetch on every open.
- Port: keep; add a backoff (not implemented upstream) is a Proposal and is NOT required by parity. Log only.
- Task: T-38b-004.

**E-LOAD-4 Refresh while a previous refresh runs.**
- Upstream: `fetchAllFromSource` does not guard against concurrent calls; two overlapping calls each set `isRefreshingData = false` at the end, so the spinner can hide while the other call still runs (MSM:258-268).
- Port: a single in-flight refresh per manga (`Option<JoinHandle>` in the model); a second request while running is ignored and returns the same completion.
- Task: T-38b-004.

**E-FETCH-1 HTTP 103 is not an error.**
- Upstream: `HttpException` code 103 (Early Hints) is ignored (MSM:284).
- Port: `SourceError::Http { code: 103 }` is not shown; no banner.
- Task: T-38b-004.

**E-FETCH-2 Only manual refresh auto-downloads new chapters.**
- Upstream: `fetchChaptersFromSource(manualFetch)` calls `downloadNewChapters` only when `manualFetch` (MSM:582-584). Initial load (`manualFetch = false`) never downloads, even if `download_new` is on.
- Port: same. Document in the settings help (Proposal text) so users know that opening a manga does not download.
- Task: T-38b-019.

**E-FETCH-3 Empty source list on local manga is valid.**
- Upstream: `NoChaptersException` is thrown only when `!source.isLocal()` (SyncChaptersWithSource.kt:17-19).
- Port: same rule in the core (`Sources::is_local`).
- Task: T-38b-004.

**E-FETCH-4 Fetch failure reloads the manga from the DB.**
- Upstream: on any chapter-fetch failure, `mangaRepository.getMangaById` replaces `manga` in state (MSM:597-598) to clear a half-applied update. The reload is not conditional.
- Port: same. If the reload fails, keep the previous manga and log.
- Task: T-38b-004.

### 1.2 Favorite and categories

**E-FAV-1 Duplicate check ignores the source and the case of non-ASCII titles.**
- Upstream: SQL `LOWER(title) = :title` (mangas.sq:121). SQLite `LOWER` only folds ASCII; the Kotlin side passes `title.lowercase()` (Unicode). A title with non-ASCII capitals (for example "Ä") can be missed in SQL when the stored title has the capital letter.
- Port: compare `title.to_lowercase()` in Rust on the candidate list, or store `title_key` (lowercased) and index it. Keep the "any source" rule (D-9).
- Task: T-38b-011.

**E-FAV-2 Duplicate dialog blocks but does not set state.**
- Upstream: when a duplicate is found, nothing is written; `dialog = DuplicateManga` (MSM:334-345). Dismiss leaves the manga non-favorite.
- Port: same. The add flow is restartable from the dialog.
- Task: T-38b-011.

**E-FAV-3 Category branch C leaves the manga non-favorite until confirmed.**
- Upstream: `ChooseCategory` does not set `favorite`; confirm sets it (MSM:367-370, 455-462). Dismissing the dialog keeps the manga out of the library, with no message.
- Port: same. Add the explicit banner "Not added to library" (Proposal) only if the user dismisses the category dialog from the add flow.
- Task: T-38b-012.

**E-FAV-4 Categories and favorite are two unordered writes.**
- Upstream: `moveMangaToCategory` and `awaitUpdateFavorite` run in two independent `launchIO` coroutines (MSM:455-478).
- Port: one transaction (`MangaStore::add_to_library`). A crash in between cannot leave a favorite with the wrong categories.
- Task: T-38b-012.

**E-FAV-5 Default category points at a deleted category.**
- Upstream: `default_category` stores an id that no longer exists; `defaultCategory == null`, `defaultCategoryId != 0`, categories may be non-empty -> branch C (choose), silently.
- Port: same branch; also log at info.
- Task: T-38b-011.

**E-FAV-6 Default category set to `0` (Uncategorized) with no categories.**
- Upstream: branch B (`defaultCategoryId == 0L || categories.isEmpty()`). Membership is cleared (empty list).
- Port: same.
- Task: T-38b-011.

**E-FAV-7 Removing from library: cover deletion happens before the favorite write is checked.**
- Upstream: `awaitUpdateFavorite(false)` must return true first (MSM:324-330), then `removeCovers()` (deletes files). Order is correct.
- Port: keep this order. Deleting cover files for a manga that stays in library would be a data-loss bug.
- Task: T-38b-013.

**E-CAT-1 `getMangaCategoryIds` KDoc says default id, returns empty.**
- Upstream: KDoc (MSM:444-449) says "if the manga is not in a category, returns the default id", code returns `map { id }` (empty list).
- Port: empty list. The category dialog shows no checked item. Matches the code.
- Task: T-38b-012.

### 1.3 Chapter list engine

**E-SRC-1 SOURCE sort direction is inverted in the comparator.**
- Upstream: `getChapterSort` SOURCE branch with `sortDescending = true` compares `c1.sourceOrder.compareTo(c2.sourceOrder)` (ascending). Because `sourceOrder` is the index in the source list (newest first), "descending" shows newest first.
- Port: the table in p2-behavior 6.4 is the spec. A regression test must check [2, 0, 1] -> [0, 1, 2] for default settings.
- Task: T-38b-002.

**E-SORT-1 Collation differs between Java Collator PRIMARY and ICU PRIMARY.**
- Upstream: `Collator.getInstance(Locale.getDefault())`, strength PRIMARY (SortUtil.kt:1-15).
- Port: ICU collator at Primary strength with the system locale. Known differences: punctuation and symbols (Java ignores some; ICU "variable" weighting defaults to non-ignorable), and some CJK. Test fixtures list the strings where the two are expected to differ; the Rust result is the reference after the port.
- Decision needed: none; record in the release notes that alphabetical order of symbol-prefixed titles may differ slightly.
- Task: T-38b-002.

**E-SEP-1 Missing-count separator id uses "null".**
- Upstream: `"${lowerChapter?.id}-${higherChapter.id}"` -> `"null-123"` for the top separator (MSM:1184-1186).
- Port: `Separator::Missing { after: Option<Id>, before: Id }`, stable key `(after, before)`; the string "null" is not used.
- Task: T-38b-045.

**E-GAP-1 Unrecognised chapter numbers in separators.**
- Upstream: `calculateChapterGap` returns 0 when either number is negative (MissingChapters.kt:40-44). The top-separator branch uses `floor(number).toInt() - 1` without that check, and the `coerceAtLeast(0)` keeps a `-1.0` first item at 0.
- Port: same arithmetic. Test `[-1.0, 5.0]` ascending -> no separator at all: the top separator is computed from `-1.0` (floor(-1) - 1 = -2, clamped to 0), and the gap between `-1.0` and `5.0` is 0 because one side is unrecognised.
- Task: T-38b-045.

**E-GAP-2 Fractional numbers and gaps.**
- Upstream: `floor(higher) - floor(lower) - 1`: `10.5` and `10.0` give -1 (no separator); `10.0` and `8.5` give 1.
- Port: same formula. Test vectors in p2-behavior 11.
- Task: T-38b-045.

**E-NEXT-1 "Next unread" in descending order returns the OLDEST unread.**
- Upstream: `findLast` on the sorted list in descending mode (ChapterGetNextUnread.kt:41-46).
- Port: same. Test: NUMBER desc [5r, 4u, 3u, 2r, 1r] -> 3.
- Task: T-38b-005.

**E-NEXT-2 Next unread uses view filters even when `skip_filtered` is false.**
- Upstream: `getNextUnread` always calls the filtered `applyFilters` (ChapterGetNextUnread.kt:52-59). `skip_filtered` only affects the download/unread list (MSM:653-655).
- Port: same split. Document in settings help.
- Task: T-38b-005.

**E-DL-3 ERROR chapters are excluded from "next N" and "unviewed".**
- Upstream: `getUnreadChapters` keeps only `NOT_DOWNLOADED` (MSM:656). A chapter whose last download failed (ERROR) is not re-queued by "next N"; only the single-chapter swipe/START path retries it.
- Port: same (parity). Proposal, not in the default: include ERROR in "next N" so a failed chapter is retried. This is a behavior change and needs the user's approval; it is not in the plan default.
- Task: T-38b-016.

**E-DL-4 Next-N order is oldest first in both sort directions.**
- Upstream: double sort/reverse (MSM:660-664).
- Port: same. Test: NUMBER desc {1,2,3,7} NEXT_1 -> [1].
- Task: T-38b-016.

**E-PREV-1 "Mark previous as read" uses value equality on Chapter.**
- Upstream: `prevChapters.indexOf(pointer)` (MSM:745) with data-class equality over all fields. A stale pointer (for example after a read toggle) returns -1 and nothing happens.
- Port: compare by id; if the id is not in the view, show nothing (same visible result), but a fresh pointer is always found.
- Task: T-38b-017.

**E-SEL-1 toggleSelection overwrites the raw list with the processed list.**
- Upstream: MSM:977-1031. After the first single-chapter toggle, `chapters` contains only visible rows until the next DB emission.
- Effects: select-all after one tap selects fewer rows; `getUnreadChapters` with `skip_filtered = false` uses a filtered list.
- Port: the raw list is never replaced by the view. Selection is a set of ids.
- Task: T-38b-008.

**E-SEL-2 Selection side effects inside `MutableStateFlow.update`.**
- Upstream: `toggleSelection` mutates `selectedChapterIds` and `selectedPositions` inside the `update` lambda (MSM:977-1031). `update` may re-run the lambda on CAS contention, so the range logic can run twice.
- Port: pure reducer, run once per message (p2-rust-design.md 4.1, 11.1).
- Task: T-38b-008.

**E-SEL-3 Anchor -1 is never replaced on the low side.**
- Upstream: with `[-1, -1]`, a first tap sets only `[1]`; `selectedIndex < selectedPositions[0]` is never true for `-1`. A later long press before the tapped row gets an empty range.
- Port: anchors are optional ids. First tap sets both anchors. This changes range results only in that corner case.
- This is a deliberate deviation from upstream and is in the plan default (a clear bug fix). It changes range results only in that corner case.
- Task: T-38b-008.

**E-SEL-4 Select-all and invert act on hidden rows.**
- Upstream: raw list (MSM:1035-1057). Bulk actions take the selected rows from the list (MangaScreen, part 1).
- Port: keep for parity (decision D-2).
- Task: T-38b-008.

**E-SEL-5 Selection clears after mark, bookmark, and download even when nothing changed.**
- Upstream: `toggleAllSelection(false)` runs before the empty check (MSM:755-756) and after bookmarks (MSM:838-839).
- Port: same ordering.
- Task: T-38b-008.

**E-FLAG-1 Quick successive filter changes lose the first change.**
- Upstream: each setter writes `manga.chapterFlags` captured from `successState` (SetMangaChapterFlags.kt:11-70). Two calls before the DB emission both start from the old flags.
- Port: `MangaStore::set_flag_bits` does the masked update in SQL (`UPDATE mangas SET chapter_flags = (chapter_flags & ~?mask) | (?flag & ?mask) WHERE _id = ?id`). Concurrent writes compose.
- Test: two tasks set different masks on one row; both survive.
- Task: T-38b-009.

**E-FLAG-2 Sort flip uses captured direction.**
- Upstream: `awaitSetSortingModeOrFlipOrder` reads `manga.sortDescending()` from the captured manga (SetMangaChapterFlags.kt:47-70). Rapid double taps can write the same direction twice.
- Port: the flip is computed in SQL (`chapter_flags ^ 1` when the sorting bits equal the requested value) or inside the transaction after reading the row.
- Task: T-38b-009.

**E-FLAG-3 Downloaded filter is forced on when "downloaded only" is set.**
- Upstream: `Manga.downloadedFilter` returns `ENABLED_IS` regardless of the stored flags while `downloadedOnly` is on (Manga.kt:25). The stored flags are unchanged; the UI shows the filter as on.
- Port: same. The stored bits are kept so turning downloaded-only off restores the user's choice.
- Task: T-38b-010.

**E-FILT-1 `filterActive` ignores forced downloaded filter in one place.**
- Upstream: `Success.filterActive` uses `manga.chaptersFiltered()` which uses the forced `downloadedFilter` (so it is true while downloaded-only is on). `scanlatorFilterActive` is separate.
- Port: same definition.
- Task: T-38b-010.

**E-DEF-1 Anime and manga share the default sort-direction key.**
- Upstream: `default_chapter_sort_by_ascending_or_descending` is used by both `sortEpisodeByAscendingOrDescending` (LibraryPreferences.kt:215-218) and `sortChapterByAscendingOrDescending` (220-223).
- Port: keep the manga key for manga; use `default_episode_sort_by_ascending_or_descending` for anime in the Rust settings table; on import from an Aniyomi backup, copy the value to both (documented).
- Decision needed (D-7).
- Task: T-38b-014.

**E-DEF-2 `setChapterSettingsDefault` writes the raw sorting value for the "source_or_number" key.**
- Upstream: `sorting` (0x000, 0x100, 0x200 or 0x300) is stored under `default_chapter_sort_by_source_or_number`, a key whose name implies two values (LibraryPreferences.kt:253).
- Port: store the full 0..=0x300 value; the key name is kept for backup compatibility; document.
- Task: T-38b-014.

**E-DEF-3 Apply-to-existing writes every favorite.**
- Upstream: `setMangaDefaultChapterFlags.awaitAll()` iterates all favorites (SetMangaDefaultChapterFlags.kt:31-35) and overwrites their flags.
- Port: same, inside one transaction; show the count after.
- Task: T-38b-014.

**E-DEF-4 Non-favorite open overwrites flags.**
- Upstream: MSM:220-222. Decision D-1.
- Port: keep, but document in the library settings help; implement as the same write.
- Task: T-38b-014.

**E-SCAN-1 NULL scanlators are never excluded.**
- Upstream: `C.scanlator = ES.scanlator` is NULL for NULL scanlator (chapters.sq:56-66).
- Port: same SQL. Test: exclude "X"; chapter with NULL scanlator remains.
- Task: T-38b-015.

**E-SCAN-2 Blank scanlators are absent from the available list but not from the chapter data.**
- Upstream: `GetAvailableScanlators` maps `ifBlank { null }` (GetAvailableScanlators.kt:8-14). `Chapter.copyFromSChapter` already turns blank into null (Chapter.kt:25). Old rows could hold "".
- Port: normalize blank to null on read as well.
- Task: T-38b-015.

**E-SCAN-3 Excluded set and available set can disagree.**
- Upstream: `scanlatorFilterActive = excluded ∩ available nonEmpty` (MSM:1192-1193). An excluded scanlator that no longer exists keeps `filterActive` false.
- Port: same.
- Task: T-38b-015.

**E-SCAN-4 Excluded write is a full diff in one transaction.**
- Upstream: `SetExcludedScanlators.await` (SetExcludedScanlators.kt:8-27). Without a unique key, a race can insert a duplicate row.
- Port: `PRIMARY KEY (manga_id, scanlator)` in the Rust schema (area:03 migration) plus `INSERT OR IGNORE`.
- Task: T-38b-015.

**E-DUP-2 Duplicate rows in `excluded_scanlators` are harmless upstream** (set semantics in Kotlin) and are removed by the diff in Port.
- Task: T-38b-015.

**E-VIEW-1 Views with the downloaded filter depend on the download cache.**
- Upstream: `isChapterDownloaded` uses the cache (`skipCache = false`, MangaDownloadCache.kt:136-...). Cache updates arrive through `downloadCache.changes` (MSM:177), which re-runs the combine.
- Port: `Downloads::is_chapter_downloaded` with cache; cache change events re-run `ChaptersLoaded` for the manga.
- Task: T-38b-010.

**E-VIEW-2 Local manga is always "downloaded" for filters.**
- Upstream: `isLocal` treats every row as downloaded for the filter and the state (MSM:541-550, ChapterFilter.kt:32).
- Port: same.
- Task: T-38b-010.

**E-CHG-1 Re-marking read can re-open the tracker prompt.**
- Upstream: prompt decision uses all passed chapters, not only changed ones (MSM:769-773). See D-8.
- Port: default keep (parity) with the decision flagged; if the user accepts D-8, use changed ids only.
- Task: T-38b-040.

**E-CHG-2 `maxOf` over chapters with -1.0 numbers.**
- Upstream: `chapters.maxOf { chapterNumber }` (MSM:770). If all passed chapters are unrecognised (-1.0), the max is -1.0 and `maxChapterNumber > track.lastChapterRead` is false (lastChapterRead >= 0 in practice). Edge: a track with lastChapterRead of -1 can prompt for -1.
- Port: skip unrecognised numbers when computing the max; if none, no prompt.
- Task: T-38b-040.

**E-CHG-3 `toInt()` truncation in messages.**
- Upstream: `maxChapterNumber.toInt()` in S5 and T2 (MSM:779, 786): 12.5 -> "12".
- Port: same truncation for display; the tracker update sends the Double (`TrackChapter`).
- Task: T-38b-040.

**E-WRITE-1 Read write with no matching rows still runs tracking.**
- Upstream: see D-8.
- Task: T-38b-040.

**E-REMOVE-1 Remove-after-read deletes downloads of chapters just marked read.**
- Upstream: `deleteDownload.awaitAll` on the chapters that were actually changed (SetReadStatus.kt:49-57). Unchanged chapters are not deleted (correct).
- Port: same: delete only the changed set.
- Task: T-38b-020.

**E-DIALOG-1 Dialog snapshots.**
- Upstream: `DeleteChapters(chapters)` and `ChangeCategory(...)` store copies at open time (MSM:1099, 389-393). If the list changes while open, the dialog still acts on the snapshot.
- Port: same for DeleteChapters (ids snapshot). For ChangeCategory, reload categories on confirm (Proposal) so a category deleted meanwhile is not written.
- Task: T-38b-012.

**E-DIALOG-2 `dialog` is `null` after a dialog is replaced.**
- Upstream: opening a new dialog replaces the current value without closing (no stack). E.g. `DuplicateManga` and then a click elsewhere.
- Port: one dialog slot, same.
- Task: T-38b-021.

**E-DL-1 CANCEL with several selected rows does nothing.**
- Upstream: `items.singleOrNull()` (MSM:711-714).
- Port: same behavior (documented). Proposal in UI: hide "cancel" for multi-select.
- Task: T-38b-017.

**E-DL-2 START_NOW with several rows does nothing and skips the add-to-library prompt.**
- Upstream: MSM:673-675 returns before the prompt code.
- Port: same.
- Task: T-38b-017.

**E-DL-5 Queue error retry.**
- Upstream: START with any ERROR item calls `startDownloads()` (MSM:703-705).
- Port: same.
- Task: T-38b-016.

**E-DL-6 Download progress event rate.**
- Upstream: per-page progress with `debounce(50)` in MangaDownload.progressFlow (MangaDownload.kt:59). Each event rebuilds the whole chapter list through `updateDownloadState` (index-based, cheap).
- Port: forwarded at most 10 Hz per manga, one row update per event.
- Port: each event updates one row in place through its index (no full list rebuild); the full view is rebuilt only on DB change or filter/sort change.
- Task: T-38b-028.

**E-CANCEL-1 Cancel an active download sets NOT_DOWNLOADED locally.**
- Upstream: `updateDownloadState(activeDownload.apply { status = NOT_DOWNLOADED })` (MSM:735-739). The UI shows NOT_DOWNLOADED while the downloader may still be running for a short time.
- Port: the cancel command is sent; the UI shows NOT_DOWNLOADED immediately (same), and a subsequent event from the downloader overrides it.
- Task: T-38b-017.

**E-DELETE-1 Delete a chapter download while it is queued.**
- Upstream: `deleteChapters` removes from queue first (MangaDownloadManager.kt:264-289).
- Port: same.
- Task: T-38b-020.

**E-DELETE-2 Manga directory removed when empty.**
- Upstream: `deleteChapters` deletes the manga directory when it has no files left (MangaDownloadManager.kt:264-289), then `deleteManga(removeQueued = false)` may remove the source directory when empty.
- Port: same, through area:23.
- Task: T-38b-020.

**E-LIB-1 Toggling favorite off while downloads exist shows the banner only after the write.**
- Upstream: `onRemoved` runs after the write (MSM:296-307), so the banner appears when the manga is no longer in the library.
- Port: same ordering.
- Task: T-38b-013.

### 1.4 Tracking

**E-TRK-1 Private toggle without a track.**
- Upstream: `togglePrivate(item)` uses `item.track!!` (MTID:262-266). A row with no track crashes if the toggle is reachable.
- Port: the control is not shown when the track is None (p2-ui-parity 4.1). Core rejects the action with an error.
- Task: T-38b-033.

**E-TRK-2 Enhanced auto-match hides the real error.**
- Upstream: any exception -> "No match found" (MTID:232-236). The logcat line is missing for this path.
- Port: log the error; show "No match found" only when `match_manga` returns None; show the error text otherwise (decision D-6).
- Task: T-38b-034.

**E-TRK-3 TrackChapter duplicated condition.**
- Upstream: `TrackChapter.await` checks the same condition twice (TrackChapter.kt:31-35). Harmless.
- Port: single check `service is none || !logged_in || chapter <= last_read` -> skip.
- Task: T-38b-040.

**E-TRK-4 Remote delete is fire-and-forget.**
- Upstream: `deleteMangaFromService` catches and logs (MTID:860-868), no user message.
- Port: decision D-5 (banner on failure, Proposal).
- Task: T-38b-036.

**E-TRK-5 Tracking count is stale right after adding.**
- Upstream: `autoOpenTrack` reads `trackingCount` which comes from an async flow (MSM:158-159, 1063-1090).
- Port: compute `tracking_count` synchronously from the store right before deciding `auto_open_tracking`.
- Task: T-38b-029.

**E-TRK-6 Refresh failure text uses a name captured at display time.**
- Upstream: `track!!.name` at the time of toast (MSM:807-813). If the tracker logs out between the refresh and the toast, the name is still correct because the object is held; in Port, capture the name in the failure record.
- Task: T-38b-029.

**E-TRK-7 Status/score/chapter/date writes ignore the local DB until the tracker answers.**
- Upstream: MangaTracker.setRemote* write remote and then the local row (area:27). The selectors close immediately (MTID:307-309, 327, 364, 392, 425, 445, 576-578).
- Port: same: close, then write detached; failures show a banner (Proposal).
- Task: T-38b-031.

**E-TRK-8 Search cancellation is swallowed.**
- Upstream: `catch (e: Throwable)` in `trackingSearch` catches coroutine cancellation (MTID:739-746).
- Port: cancellation is not an error; the search future is aborted without a banner.
- Task: T-38b-035.

**E-TRK-9 Search result identity.**
- Upstream: `MangaTrackSearch.equals` uses `manga_id`, `tracker_id`, `remote_id` only (app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt).
- Port: `TrackSearchResult` implements Eq/Hash on `(tracker_id, remote_id)` plus `manga_id` context; the `private` flag is excluded from identity.
- Task: T-38b-035.

**E-DATE-1 Local midnight vs UTC millis.**
- Upstream: the picker works in UTC midnight millis; stored values are converted from UTC to the system zone (MTID:562-583). A DST change at midnight can shift a date by one hour.
- Port: store `local date at 00:00` in the local zone; if midnight does not exist (rare DST gap), use the earliest valid time.
- Task: T-38b-032.

**E-DATE-2 Year-only validation.**
- Upstream: `isSelectableYear` checks year bounds only (MTID:495-519); `isSelectableDate` checks the day.
- Port: the same two functions; the calendar view uses the day check, the year grid uses the year check.
- Task: T-38b-032.

**E-DATE-3 Remove date writes 0.**
- Upstream: `setRemoteStartDate(track, 0)` (MTID:667-675).
- Port: same. `0` means unset in every tracker mapping (area:27 owns the wire mapping).
- Task: T-38b-033.

**E-INT-1 Interval 0 means automatic.**
- Upstream: `-0 = 0`, not negative, so `toMangaUpdate` recalculates (MangaFetchInterval.kt:22).
- Port: same.
- Task: T-38b-022.

### 1.5 Platform and UI

**E-UI-1 Snackbar action after the screen is gone.**
- Upstream: `launch { showSnackbar(...) }` uses the model scope; a closed screen cancels it, so the action is lost. The DB writes behind a snackbar action (`toggleFavorite`, `deleteDownloads`) run on user action only.
- Port: the banner belongs to the window, not the page. The action runs if the page is still alive; otherwise the action is dropped and nothing is written. Decision: keep this (safer than acting on a page that was closed).
- Task: T-38b-024.

**E-UI-2 Toast on a closed window.**
- Port: toasts are window-scoped, dropped when the window closes.
- Task: T-38b-024.

**E-UI-3 Keyboard focus in the chapter list during selection.**
- Port: Space and Shift+Space act on the focused row only (p2-ui-parity section 6).
- Task: T-38b-023.

**E-THREAD-1 Model used off the GTK main thread.**
- Port: enforced by type (`Rc<RefCell<DetailModel>>` is not `Send`); worker tasks send `Msg` only.
- Task: T-38b-039.

**E-PATH-1 Chapter directory names from title/scanlator must be NTFS-safe.**
- Port: all names derived from chapter or manga titles go through the addendum's sanitizer (area:23 owns the function). This part only calls `is_chapter_downloaded` with raw names.
- Task: T-38b-010.

**E-DB-1 Chapter and manga version counters.**
- Upstream: triggers increment `chapters.version` and `mangas.version` on update when not syncing (chapters.sq around line 40-50).
- Port: keep the counters so backup/sync (area:21, area:22) keep working; the masked flag update in E-FLAG-1 must also bump `mangas.version`.
- Task: T-38b-009.

**E-I18N-1 Plurals and formatting.**
- Upstream: `%d` and `%s` placeholders (no plural forms in this scope).
- Port: `format!` with typed arguments (strings.rs).
- Task: T-38b-037.

---

## 2. Upstream tests to port

Source: `domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt` (read in full). Port to `crates/kuro-chapters/src/tests/missing.rs`.

| Upstream test name | Input | Expected | Rust test |
|---|---|---|---|
| `missingChaptersCount returns 0 when empty list` | `[]` | 0 | `missing_count_empty` |
| `missingChaptersCount returns 0 when all unknown item numbers` | `[-1, -1, -1]` | 0 | `missing_count_all_unknown` |
| `missingChaptersCount handles repeated base item numbers` | `[1, 1, 1.1, 1.5, 1.6, 1.99]` | 0 | `missing_count_repeated_base` |
| `missingChaptersCount returns number of missing items` | `[-1, 1, 2, 2.2, 4, 6, 10, 10]` | 5 | `missing_count_basic` |
| `calculateChapterGap returns difference` | (10, 9) 0; (10, 8) 1; (10, 8.5) 1; (10, 1.1) 8 | as listed | `gap_difference` |
| `calculateChapterGap returns 0 if either are not valid chapter numbers` | (-1, 10) 0; (99, -1) 0 | 0 | `gap_unrecognised` |
| `calculateEpisodeGap returns difference` | same vectors, episode version | same | not ported (anime; area:16/19) |

Note: the upstream test for `calculateChapterGap` passes `Chapter` objects through `Chapter.create().copy(chapterNumber = ...)`. The Rust fixture helper is `Chapter::with_number(n)`.

No upstream tests exist for: `getChapterSort`, `applyFilters`, `toggleSelection`, `getNextUnread`, `removeDuplicates`, `filterDownloadedChapters`, `SetMangaChapterFlags`, `SetReadStatus`, `FilterChaptersForDownload`, `SetExcludedScanlators`, track date constraints. These are new tests (section 3).

Other existing upstream tests in neighbouring areas that touch this part: `domain/src/test/java/tachiyomi/domain/entries/manga/interactor/MangaFetchIntervalTest.kt` (area:07), `domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt` (area:07). Not ported here.

---

## 3. New tests to write (Rust)

Test naming: `crates/<crate>/src/tests/*.rs` or `tests/` integration tests. All pure tests need no display and no DB.

### 3.1 Flags (`kuro-chapters`)

| Test | Assertion |
|---|---|
| `flag_masks_are_disjoint` | sort dir 0x1, unread 0x6, downloaded 0x18, bookmarked 0x60, sorting 0x300, display 0x100000 are pairwise disjoint |
| `with_masked_keeps_other_bits` | `ChapterFlags(0x1A).with_masked(0x8, 0x18).0 == 0x0A` (from p2-behavior 11) |
| `with_masked_preserves_unknown_bits` | value with bit 0x400 set keeps it after any setter |
| `unread_triple_mapping` | 0x0 Disabled, 0x2 EnabledIs, 0x4 EnabledNot, 0x6 Disabled (else branch) |
| `downloaded_forced_when_downloaded_only` | any flags + downloaded_only=true -> EnabledIs |
| `default_flags_are_source_desc` | `ChapterFlags(0)`: sorting Source, descending true, all Disabled, display Name |
| `filter_active_matches_chapters_filtered` | matrix of 27 tri-state combinations x downloaded_only flag |

### 3.2 Sort (`kuro-chapters`)

Fixed collator in tests: a `TestCollate` that compares ASCII case-insensitively (primary) for determinism; plus an `IcuCollate` test under a fixed locale `en-US` (E-SORT-1).

| Test | Input | Expected |
|---|---|---|
| `source_desc_is_newest_first` | sourceOrder [2, 0, 1], flags default | sourceOrder [0, 1, 2] (E-SRC-1) |
| `source_asc_is_oldest_first` | same input, SORT_ASC | [2, 1, 0] |
| `number_desc` | numbers [1, 10, 2.5] | [10, 2.5, 1] |
| `number_asc` | same | [1, 2.5, 10] |
| `number_unrecognised_first_asc` | [-1, 3] asc | [-1, 3] |
| `number_unrecognised_last_desc` | [-1, 3] desc | [3, -1] |
| `upload_date_desc` | dates [5, 9, 1] desc | [9, 5, 1] |
| `alphabet_primary_case` | "b", "A", "a" asc | "A", "a", "b" (equal primaries keep stable order) |
| `sort_is_stable` | equal numbers, different ids | original order kept |

### 3.3 View (`kuro-chapters`)

| Test | Assertion |
|---|---|
| `view_unread_is_only_unread` | EnabledIs keeps !read |
| `view_not_read_is_read_only` | EnabledNot keeps read |
| `view_downloaded_keeps_local_rows` | is_local=true keeps all rows with downloaded filter EnabledIs |
| `view_downloaded_queue_is_not_downloaded` | a QUEUE row is dropped by EnabledIs (E-VIEW-1 analog) |
| `view_all_27_combinations_match_reference` | generated table from the three `keep` functions (reference implementation written as nested ifs) |
| `view_filters_compose` | unread + bookmarked + downloaded together |

### 3.4 Missing and separators

| Test | Assertion |
|---|---|
| `separators_asc_top` | [5, 6] asc -> separator at index 0 with count 4 and after None |
| `separators_asc_between` | [1, 2, 4] asc -> separator at index 2 (before 4) count 1 |
| `separators_desc_bottom` | [6, 5, 2] desc -> no top; between 5 and 2 count 2 (chapters 3, 4); bottom count 1 (chapter 1) |
| `separators_desc_fraction_no_gap` | [10.5, 10] desc -> none |
| `separators_unrecognised_no_separator` | [-1, 5] asc -> no separators (top: floor(-1)-1 clamped to 0; between: gap 0 because -1 is unrecognised) |
| `separator_id_uses_none_not_null_string` | id type is Option, formatted key "none-7" |

### 3.5 Selection (`kuro-chapters`)

View ids [1..=10] in view order.

| Test | Assertion |
|---|---|
| `tap_toggle_on_off` | toggle true then false restores empty set |
| `long_press_first_sets_anchors` | first long press selects one, anchors = it |
| `long_press_range_forward` | anchors at 3; long press 7 -> ids 3..=7 |
| `long_press_range_backward` | anchors at 6; long press 2 -> ids 2..=6 |
| `long_press_inside_range_only_self` | anchors 3 and 7; long press 5 -> only 5 selected |
| `tap_deselect_anchor_moves_to_next_selected` | anchor 3 deselected -> first anchor becomes next selected |
| `set_all_uses_raw_ids` | raw includes hidden ids; set_all(true) selects hidden |
| `invert_flips_raw` | invert over raw ids |
| `toggle_no_change_returns_false` | selecting an already selected id returns false |
| `toggle_missing_id_is_noop` | id not in view -> no change (upstream `indexOfFirst < 0`) |
| `anchors_survive_filter_change` | after view changes (ids removed), anchor resolves or falls back, no panic |
| `no_panic_on_empty_view` | empty view returns false |

### 3.6 Next unread, dedupe, download pick

| Test | Assertion |
|---|---|
| `next_unread_asc_first` | asc list [1r, 2u, 3u] -> 2 |
| `next_unread_desc_last` | desc view [3u, 2r, 1u] -> `1` (findLast) |
| `next_unread_none` | all read -> None |
| `dedupe_prefers_current_id` | two chapters number 5: current id wins |
| `dedupe_prefers_scanlator` | no id match, same scanlator wins |
| `dedupe_first_fallback` | neither -> first in input order |
| `dedupe_keeps_first_seen_group_order` | output order by first appearance of each number |
| `download_pick_excludes_error_and_queue` | ERROR and QUEUE excluded from unread_not_downloaded |
| `next_n_oldest_first_desc` | NUMBER desc {1,2,3,7} -> NEXT_1 [1], NEXT_5 [1,2,3,7] |
| `unviewed_is_unsorted_view_order` | keeps input order |

### 3.7 Favorite routing (`kuro-core`)

| Test | default_category | categories | Route |
|---|---|---|---|
| `route_default_category_exists` | 5 | [5, 7] | AddDefaultCategory(Some(5)) |
| `route_default_uncategorized` | 0 | [5] | AddDefaultCategory(None) |
| `route_no_categories` | 9 | [] | AddDefaultCategory(None) |
| `route_default_deleted_choose` | 9 | [5, 7] | ChooseCategory |
| `route_default_unset_choose` | -1 | [5] | ChooseCategory |

### 3.8 Track constraints (`kuro-core`)

| Test | Assertion |
|---|---|
| `start_date_not_after_finish` | finish = Mar 10; start Mar 11 -> not selectable; Mar 10 selectable |
| `finish_date_not_before_start` | start = Mar 10; finish Mar 9 -> not selectable |
| `no_future_dates` | today Jun 1; Jun 2 not selectable; year 2027 not selectable when today is 2026 |
| `year_bounds_match_day_bounds` | year checks use the same bound logic |
| `chapter_picker_max_default_10000` | total 0 -> max 10000 |
| `chapter_picker_max_from_total` | total 24 -> max 24 |
| `chapter_picker_initial_truncates` | last read 12.9 -> selected 12 (toInt) |
| `can_remove_date_only_when_set` | start 0 -> false |
| `initial_date_falls_back_to_now` | start 0 -> now |
| `local_midnight_roundtrip` | converting a picked date to ms and back gives the same date in local zone (with a DST test zone) |

### 3.9 Tracker row availability (`kuro-core`)

| Test | Assertion |
|---|---|
| `no_track_hides_private_toggle` | track None -> no private action (E-TRK-1) |
| `no_remote_url_hides_browser` | blank url -> no open/copy |
| `enhanced_source_filter` | tracker.accepts_source false -> row absent |
| `map_track_items_one_row_per_logged_in_manga_tracker` | counts |

### 3.10 Model reducer (`kuro-core`)

| Test | Assertion |
|---|---|
| `mark_read_clears_selection_even_when_empty` | E-SEL-5 |
| `mark_read_filters_already_read` | only unread ids produce writes |
| `mark_unread_resets_last_page` | ChapterUpdate.last_page_read = Some(0) |
| `mark_read_no_tracker_when_never` | auto=Never -> no Track cmd |
| `mark_read_always_emits_track` | auto=Always and behind -> Track cmd + toast message |
| `mark_read_ask_emits_banner_with_action` | auto=Ask -> banner with action Track |
| `add_prompt_once_per_session` | has_prompted_add prevents second prompt |
| `start_now_multi_is_noop` | E-DL-2 |
| `cancel_multi_is_noop` | E-DL-1 |
| `filter_changes_compose` | two Msg::FilterUnread/Downloaded produce two masked writes, not a full overwrite (E-FLAG-1) |
| `pending_after_category_flag_one_shot` | consumed once |

### 3.11 Database (`kuro-db`, integration with SQLite)

| Test | Assertion |
|---|---|
| `masked_flag_write_is_atomic` | two concurrent `set_flag_bits` with different masks both persist |
| `masked_flag_matches_kotlin_setflag` | table of 64 random (old, flag, mask) triples: SQL result == Rust `with_masked` == Kotlin formula |
| `excluded_null_scanlator_not_removed` | E-SCAN-1 |
| `excluded_diff_transaction` | set {A,B} then {B,C}: rows = {B,C} |
| `duplicate_check_unicode_case` | title "Ärger" vs "ärger" detected (E-FAV-1 fix) |
| `duplicate_check_excludes_self` | `_id != :id` |
| `add_to_library_is_one_transaction` | fault injection after categories write -> favorite not set |
| `mark_read_bumps_chapter_version` | version increments (E-DB-1) |

### 3.12 Source mapping (`kuro-core`)

| Test | Assertion |
|---|---|
| `scanlator_blank_becomes_none` | "  " -> None; "A " -> Some("A") (trim after ifBlank) |
| `chapter_number_unknown_is_minus_one` | from SChapter default -1.0 |
| `http_103_is_ignored` | no banner |
| `no_chapters_non_local_error` | NoChapters for non-local; local manga accepts empty |

### 3.13 Snapshot / golden tests

Golden files for the view order of a fixture manga (50 chapters with mixed numbers, a missing range, one fraction, two scanlators, one unrecognised) under each sort and filter combination. The golden JSON is produced by the Rust reference implementation after review against the Kotlin logic by hand (no JVM available in the port environment). Stored under `crates/kuro-chapters/tests/golden/`.

---

## 4. Fixtures needed

| Fixture | Content | Used by |
|---|---|---|
| `fixtures/chapters_mixed.json` | 50 chapters: numbers 1..=40 with 13, 14 missing, 20.5, -1 for one, two scanlators ("Alpha", "Beta") and one null scanlator, dates across 10 days, read/bookmark mix | sort, view, separators, golden |
| `fixtures/manga_flags.json` | 27 flag combinations | view |
| `fixtures/track_dates.json` | tracks with start 0/finish 0, start only, finish only, both | track constraints |
| `fixtures/collate_pairs.json` | pairs where ICU and Java PRIMARY may differ (symbols, Unicode) | E-SORT-1 |
| `fixtures/fake_downloads.rs` | in-memory `Downloads` impl with scripted events | reducer, download intents |
| `fixtures/fake_trackers.rs` | `MangaTracker` fake with statuses, scores, failing search | tracking models |
| `fixtures/sqlite_schema.sql` | the Rust schema for mangas, chapters, excluded_scanlators, manga_sync, categories, mangas_categories (generated by area:03) | kuro-db tests |
| `fixtures/clock.rs` | fixed `Clock` with date 2026-06-01 and a DST test zone | date tests |

---

## 5. Test execution rules

- `cargo test -p kuro-chapters` and `cargo test -p kuro-core` must pass without a display server or a tokio runtime (reducer tests are synchronous).
- `cargo test -p kuro-db` may use an in-memory SQLite database.
- No test may read the system locale. Collation tests set the locale explicitly.
- Golden files are regenerated only with an explicit `UPDATE_GOLDEN=1` env var and reviewed in the diff.
- Add the Windows check (`cargo check --target x86_64-pc-windows-gnu`) to CI for these crates per the cross-platform addendum.
