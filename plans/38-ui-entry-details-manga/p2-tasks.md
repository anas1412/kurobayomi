# 38 part 2: implementation tasks (manga details ScreenModel, chapter engine, tracking sheets)

Task format per `plans/_tools/tasks.py`. Paths in `Sources:` are repo-relative to `/home/blackbox/aniyomi-src`. Every task cites the behavior section in `p2-behavior.md` in its acceptance, and the edge IDs in `p2-edge-cases-and-tests.md`.

P0 = needed to read manga daily with the details page (load, chapter list, sort/filter, selection, read/bookmark, downloads, favorite, fetch, errors, threading). P1 = parity features (tracking, categories presets, migration triggers, scanlator filter, auto-track). P2 = polish. P3 = Android-only, dropped or replaced.

### T-38b-001 — ChapterFlags bit layout and TriState mapping
- Priority: P0
- Effort: S
- Depends: area:07-domain-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt, domain/src/main/java/tachiyomi/domain/entries/TriState.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt
- Acceptance:
  - `ChapterFlags` in `kuro-chapters::flags` exposes every constant in p2-behavior 1.1 with the exact hex values; masks are pairwise disjoint (test `flag_masks_are_disjoint`).
  - `with_masked(flag, mask)` equals `(old & !mask) | (flag & mask)`; `ChapterFlags(0x1A).with_masked(0x8, 0x18).0 == 0x0A`.
  - `unread()`, `bookmarked()`, `downloaded(downloaded_only)` return the TriState rules of p2-behavior 1.2 and 1.3, including `downloaded_only = true` forcing `EnabledIs`.
  - `sorting()` maps 0x000/0x100/0x200/0x300 and treats other values as Source (documented; upstream throws).
  - Default `ChapterFlags(0)` is Source sorting, descending, no filters, name display.

### T-38b-002 — Chapter sort comparator and primary collation
- Priority: P0
- Effort: M
- Depends: T-38b-001
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt, core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt, domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt
- Acceptance:
  - `compare(sorting, descending, a, b, collate)` reproduces the 8-row table of p2-behavior 6.4 exactly, including the inverted SOURCE branch (E-SRC-1): `[2,0,1]` with defaults sorts to `[0,1,2]`.
  - NUMBER uses `f64::total_cmp`; `-1.0` sorts first ascending and last descending.
  - ALPHABET uses an ICU collator at Primary strength with the system locale (fallback `und`); test fixture `collate_pairs.json` lists strings where Java PRIMARY and ICU may differ (E-SORT-1), and the chosen behavior is recorded.
  - `sort_chapters` is stable.
  - Unit tests `source_desc_is_newest_first`, `number_desc`, `number_unrecognised_first_asc`, `upload_date_desc`, `alphabet_primary_case`, `sort_is_stable` pass.

### T-38b-003 — Details load pipeline (Loading, NotFound, defaults before first paint)
- Priority: P0
- Effort: M
- Depends: T-38b-001, T-38b-014, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetMangaWithChapters.kt, domain/src/main/java/tachiyomi/domain/items/chapter/interactor/SetMangaDefaultChapterFlags.kt
- Acceptance:
  - Load order: manga, then (for non-favorites) defaults applied, then chapters, then `Ready` (p2-behavior 3.1; E-LOAD-2).
  - A missing manga row yields `LoadState::NotFound` with a Back action, never an endless Loading (E-LOAD-1).
  - `needs_info = !manga.initialized`, `needs_chapters = chapters.is_empty()` drive the initial fetch; both are read once.
  - Initial state carries `is_refreshing = needs_info || needs_chapters`, `dialog = None`, `has_prompted_add = false`.
  - Chapter rows are built with download state and selection from the models (behavior 6.1).

### T-38b-004 — Source fetch: manga details, chapter list, manual refresh, error mapping
- Priority: P0
- Effort: M
- Depends: T-38b-003, T-38b-025, area:15-source-api-manga, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt, app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt, app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt
- Acceptance:
  - Details and chapter parts run concurrently; `fetch_all(manual)` sets refreshing true then false (behavior 3.2-3.4).
  - Manual refresh default is `manual = true`; initial load uses `manual = false`.
  - HTTP 103 is silently ignored (E-FETCH-1); `NotInstalled` shows `source_not_installed`; empty non-local list yields `no_chapters_error` text (E-FETCH-3).
  - On chapter-fetch failure the manga is re-read from the store and replaces the cached manga (E-FETCH-4).
  - Only one refresh runs per manga; a second request is ignored (E-LOAD-4).
  - Errors are shown as banners (task 024); no panic on any error.

### T-38b-005 — Next unread chapter (continue reading)
- Priority: P0
- Effort: S
- Depends: T-38b-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterGetNextUnread.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt
- Acceptance:
  - `next_unread(view, descending)` returns the first unread in view order, or the last unread when descending (`findLast`); returns `None` when all read (E-NEXT-1).
  - Operates on the view after view filters are applied, even when `skip_filtered` is false (E-NEXT-2).
  - Both overloads (`List<Chapter>` with download manager and `List<ChapterList.Item>`) produce the same result for the same input.
  - Test `next_unread_desc_last` returns `1` for view `[3u, 2r, 1u]`.

### T-38b-006 — Reader duplicate removal (one chapter per number)
- Priority: P1
- Effort: S
- Depends: T-38b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterRemoveDuplicates.kt
- Acceptance:
  - Groups by `chapter_number` in first-seen order; per group picks id == current, else same scanlator, else first (ChapterRemoveDuplicates.kt:8-14).
  - Grouping uses exact `f64` bits, so `0.0` and `-0.0` are separate groups (matches `Double.equals`).
  - Tests `dedupe_prefers_current_id`, `dedupe_prefers_scanlator`, `dedupe_first_fallback`, `dedupe_keeps_first_seen_group_order` pass.

### T-38b-007 — Downloaded-only chapter list helper
- Priority: P1
- Effort: S
- Depends: T-38b-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterFilterDownloaded.kt
- Acceptance:
  - Returns the input unchanged for local manga; otherwise keeps chapters where `Downloads::is_chapter_downloaded(name, scanlator, manga_title, source_id)` (cache-backed) is true.
  - Used by reader skip-downloaded paths (area:36) with the same signature shape.

### T-38b-008 — Chapter selection engine (set, anchors, tap, long-press range, select all, invert)
- Priority: P0
- Effort: M
- Depends: T-38b-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/core/util/CollectionUtils.kt
- Acceptance:
  - `Selection::toggle` implements p2-behavior 6.8 steps 1-9 for the three gestures; anchors are chapter ids, resolved against the current view.
  - Selection never replaces the raw chapter list with the view (E-SEL-1); `set_all` and `invert` act on raw ids (E-SEL-4).
  - Toggle is a pure reducer step, run once per message (E-SEL-2).
  - Tap-select with no anchor sets both anchors (E-SEL-3 deviation, recorded).
  - Mark, bookmark, download actions clear the selection even when nothing changed (E-SEL-5).
  - Tests `tap_toggle_on_off`, `long_press_range_forward`, `long_press_range_backward`, `long_press_inside_range_only_self`, `set_all_uses_raw_ids`, `invert_flips_raw`, `toggle_missing_id_is_noop`, `anchors_survive_filter_change` pass.

### T-38b-009 — Masked chapter-flag writes, version bump, and no lost updates
- Priority: P0
- Effort: M
- Depends: T-38b-001, area:05-data-repos-entries-items, area:03-db-manga-schema-migrations
- Sources: domain/src/main/java/tachiyomi/domain/entries/manga/interactor/SetMangaChapterFlags.kt, data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelight/data/chapters.sq
- Acceptance:
  - `MangaStore::set_flag_bits(id, flag, mask)` runs one SQL `UPDATE` with `(chapter_flags & ~mask) | (flag & mask)` (E-FLAG-1); two concurrent calls with different masks both persist.
  - Sort flip (`awaitSetSortingModeOrFlipOrder`) is computed inside the same transaction from the stored row (E-FLAG-2); the first sort selection writes direction ASC.
  - Every flag write increments `mangas.version` (E-DB-1), matching the trigger behavior in chapters.sq.
  - Test `masked_flag_matches_kotlin_setflag` passes for 64 generated triples.

### T-38b-010 — Chapter view pipeline (unread, bookmarked, downloaded, sort) and local-manga rules
- Priority: P0
- Effort: M
- Depends: T-38b-001, T-38b-002, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt, domain/src/main/java/tachiyomi/domain/entries/TriState.kt
- Acceptance:
  - `apply_view` applies in order: unread, bookmarked, downloaded (`is_downloaded || is_local`), then sort (p2-behavior 6.3); returns indices.
  - Downloaded filter uses `DownloadState::Downloaded` only (QUEUE/DOWNLOADING/ERROR are not downloaded) (behavior 6.3 step 5).
  - Local manga: every row is `Downloaded`, no download actions (E-VIEW-2).
  - `downloaded_only` forces the downloaded filter to `EnabledIs` without changing stored bits (E-FLAG-3); `filter_active` includes it (E-FILT-1).
  - The 27 tri-state combinations match a reference implementation (test `view_all_27_combinations_match_reference`).
  - Cache-backed download checks are re-run when the download cache changes (E-VIEW-1). Filenames passed to the cache are never built with raw path separators (E-PATH-1, area:23 sanitizer).

### T-38b-011 — Favorite add/remove with duplicate detection and default-category routing
- Priority: P0
- Effort: M
- Depends: T-38b-012, T-38b-013, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetDuplicateLibraryManga.kt, data/src/main/sqldelight/data/mangas.sq, app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt
- Acceptance:
  - `route_add(default_category, categories)` returns the five cases of p2-rust-design 4.4 and the table in p2-edge-cases 3.7 (E-FAV-5, E-FAV-6).
  - Duplicate check: `LOWER(title)` semantics fixed in Rust (E-FAV-1); "any source" rule kept (D-9); no write on duplicate (E-FAV-2).
  - Remove: `favorite = false` and `date_added = 0` first; cover deletion only after the write succeeds (E-FAV-7, task 013).
  - Add, branch A and B: favorite and category write happen in one transaction (task 012).
  - Add, branch C: favorite is not written until categories are confirmed (E-FAV-3).
  - `bind_enhanced` runs in all three add branches (behavior 4.2 step 6).
  - Tests `route_default_category_exists`, `route_default_uncategorized`, `route_no_categories`, `route_default_deleted_choose`, `route_default_unset_choose`, `duplicate_check_unicode_case` pass.

### T-38b-012 — Change-category dialog and one-transaction confirm
- Priority: P0
- Effort: S
- Depends: T-38b-021, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/GetMangaCategories.kt, domain/src/main/java/tachiyomi/domain/category/model/Category.kt
- Acceptance:
  - Dialog lists user categories only (`id != 0`), with initial checked state from `categories_of(manga)` (behavior 4.3).
  - Empty membership yields an empty list, not the default id (E-CAT-1).
  - Confirm calls `add_to_library(manga_id, ids)` (add flow) or `set_categories` (already favorite) inside one transaction (E-FAV-4).
  - Checkboxes are two-state (`CheckboxState.State`); only checked categories are written (UI parity 3.1).
  - Dismissing the add-flow dialog leaves the manga non-favorite and triggers the one-shot "open tracker" intent only when `auto_open_tracking` (E-CAT-3 in behavior 4.2 note).
  - Snapshot semantics for the open dialog (E-DIALOG-1); categories are reloaded on confirm.

### T-38b-013 — Remove from library side effects and download-deletion prompt
- Priority: P1
- Effort: S
- Depends: T-38b-011, T-38b-024, area:34-images-covers-cache, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt
- Acceptance:
  - Order: favorite write succeeds, then cover cache removal (`removeCovers`), then `cover_last_modified` update when the copy differs, then the callback (behavior 4.2, E-FAV-7, E-LIB-1).
  - If the manga has downloads (`download_count > 0`), show banner `delete_downloads_for_manga` with action `action_delete`; action runs `delete_manga(remove_queued = true)`.
  - Local manga never has its cover removed.

### T-38b-014 — Chapter settings defaults: set as default, apply to all favorites, non-favorite reset, key separation
- Priority: P1
- Effort: M
- Depends: T-38b-009, area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, domain/src/main/java/tachiyomi/domain/items/chapter/interactor/SetMangaDefaultChapterFlags.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt
- Acceptance:
  - `set_chapter_defaults(current manga)` writes exactly the six keys of behavior 6.13 with the exact key names of p2-data-and-config 4.1.
  - Apply-to-existing writes every favorite in one transaction and reports the count (E-DEF-3).
  - Non-favorite open applies defaults (kept per decision D-1); the behavior is documented in the library settings help.
  - Anime and manga default sort-direction keys are separate in the Rust settings table; backup import copies the value to both (E-DEF-1, decision D-7).
  - Snackbar `chapter_settings_updated` after saving (behavior 6.13).
  - Reset-to-default applies the saved defaults to the current manga only.

### T-38b-015 — Scanlator exclusions (data, available list, filter state, UI sheet wiring)
- Priority: P1
- Effort: M
- Depends: T-38b-010, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetExcludedScanlators.kt, app/src/main/java/eu/kanade/domain/entries/manga/interactor/GetExcludedScanlators.kt, app/src/main/java/eu/kanade/domain/items/chapter/interactor/GetAvailableScanlators.kt, data/src/main/sqldelight/data/excluded_scanlators.sq, data/src/main/sqldelight/data/chapters.sq
- Acceptance:
  - Chapter query with `apply_scanlator_filter = true` removes excluded scanlators; NULL scanlators are never excluded (E-SCAN-1; test `excluded_null_scanlator_not_removed`).
  - Available scanlators: distinct non-blank values, blank normalized to None on read (E-SCAN-2).
  - `scanlator_filter_active = excluded ∩ available nonempty`; `filter_active` ORs the chapter filters (behavior 7.1).
  - Save is a diff in one transaction; the table gains `PRIMARY KEY (manga_id, scanlator)` in the Rust schema, with `INSERT OR IGNORE` (E-SCAN-4).
  - Live updates from `watch_available` and `watch_excluded` re-run the view.
  - Test `excluded_diff_transaction` passes.

### T-38b-016 — Download sheet actions: next N, unviewed, error retry
- Priority: P1
- Effort: M
- Depends: T-38b-010, T-38b-017, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt
- Acceptance:
  - `NEXT_1_ITEM/5/10/25` take the first N of `unread_sorted_oldest_first` (NUMBER desc `{1,2,3,7}` gives NEXT_5 `[1,2,3,7]`; E-DL-4).
  - `UNVIEWED_ITEMS` uses unsorted view order (behavior 6.9.5).
  - "Unread and not downloaded" is exact `NotDownloaded`, so ERROR, QUEUE, DOWNLOADING, DOWNLOADED are excluded (E-DL-3; Proposal to include ERROR is NOT in the default).
  - START with any ERROR row calls `start_all()` after enqueue (E-DL-5).
  - Empty set does nothing.

### T-38b-017 — Chapter row actions: download state mapping, START_NOW, CANCEL, swipe, mark previous
- Priority: P0
- Effort: M
- Depends: T-38b-008, T-38b-018, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Indicator action mapping: NOT_DOWNLOADED or ERROR -> START_NOW; QUEUE or DOWNLOADING -> CANCEL; DOWNLOADED -> DELETE (behavior 6.9.1).
  - START_NOW with several rows does nothing and skips the add prompt (E-DL-2); CANCEL with several rows does nothing (E-DL-1).
  - Cancel sets the local state to NOT_DOWNLOADED immediately; a later downloader event overrides it (E-CANCEL-1).
  - Swipe actions: start edge key `pref_chapter_swipe_start_action` (default ToggleBookmark), end edge key `pref_chapter_swipe_end_action` (default ToggleRead); mapping by direction, not key name (behavior 2.1).
  - `Disabled` swipe never dispatches; no panic.
  - Mark previous as read: compares chapters by id (E-PREV-1); marks chapters strictly before the pointer in chronological order (behavior 6.9.8; test vector: view `[9,8,7,6]`, pointer 7 marks `[6]`).
  - First enqueue for a non-library manga triggers the add prompt once per session (behavior 6.9.3; the prompt wiring is task 017).

### T-38b-018 — Mark read/unread and bookmark writes
- Priority: P0
- Effort: S
- Depends: T-38b-008, T-38b-024, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, domain/src/main/java/tachiyomi/domain/items/chapter/model/ChapterUpdate.kt
- Acceptance:
  - Read: only chapters with `!read` are written; unread: only chapters with `read || last_page_read > 0`, and `last_page_read` is reset to 0 (behavior 6.9.9a).
  - Bookmark: only rows whose bookmark differs are written (behavior 6.10).
  - Selection is cleared first, even when the list is empty (E-SEL-5).
  - `remove_download_after_read` deletes downloads only for chapters that actually changed (E-REMOVE-1, task 020).
  - Writes run detached (`launchNonCancellable` semantics, task 039); a page close does not cancel them.
  - The tracker prompt that follows a read is task 040, not this task.

### T-38b-019 — Auto-download new chapters on manual refresh
- Priority: P1
- Effort: M
- Depends: T-38b-004, area:23-downloads-manga, area:07-domain-entries-items
- Sources: domain/src/main/java/mihon/domain/items/chapter/interactor/FilterChaptersForDownload.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt
- Acceptance:
  - Runs only when the refresh was manual (E-FETCH-2); initial load never downloads.
  - Filter rules of behavior 6.12: empty input, `download_new` off, non-favorite, category include/exclude, and `download_new_unread_chapters_only` (exclude numbers that have a read chapter).
  - Category rule: no categories -> category id 0; excluded wins over included; empty include list means "all" (behavior 6.12 `shouldDownloadNewChapters`).
  - Keys: `download_new`, `download_new_categories`, `download_new_categories_exclude`, `download_new_unread_chapters_only` (p2-data-and-config 4.3).
  - Enqueue clears the selection (behavior 6.9.4).

### T-38b-020 — Delete downloads and remove-after-read
- Priority: P1
- Effort: S
- Depends: T-38b-018, area:23-downloads-manga, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - `delete_chapters(chapters)` removes queued entries first, then chapter directories, then cache entries, then the manga directory if empty (E-DELETE-1, E-DELETE-2; behavior 6.11).
  - Errors are logged; no crash; selection is not cleared by this task (the dialog task clears it).
  - `pref_remove_after_marked_as_read_key` (default false) triggers deletion after read (behavior 6.9.9a).

### T-38b-021 — Dialog routing: single slot and typed dialog kinds
- Priority: P0
- Effort: S
- Depends: T-38b-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt
- Acceptance:
  - One optional dialog value; opening replaces the current dialog (E-DIALOG-2); `dismiss` sets None.
  - Kinds: ChangeCategory, DeleteChapters(snapshot ids), DuplicateManga, Migrate, SetFetchInterval, ChapterSettings, Tracking, FullCover (behavior 1.9).
  - The one-shot "open tracking after category dialog" intent is consumed exactly once (behavior 4.2 note).

### T-38b-022 — Custom fetch interval
- Priority: P1
- Effort: S
- Depends: T-38b-021, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/MangaFetchInterval.kt
- Acceptance:
  - Days value `n` is stored as `-n` in `fetch_interval` (behavior 4.5).
  - `0` resets to automatic (E-INT-1); the dialog offers an explicit reset (UI parity 3.4).
  - After a successful write the manga is re-read and the state updated.
  - Test: stored `-7` for 7 days; `0` restores automatic calculation path.

### T-38b-023 — Keyboard and pointer selection in the chapter list
- Priority: P0
- Effort: M
- Depends: T-38b-008, T-38b-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt
- Acceptance:
  - Long press (touch) and right-click (mouse) enter selection; Shift+click and Shift+Space give range; Ctrl+click and Space give tap toggle (UI parity 6).
  - Ctrl+A selects all raw rows; Ctrl+I inverts; Escape clears selection; Escape closes dialogs.
  - Focus stays on the row after a toggle; focus is never moved to a hidden row.
  - Manual test script: select 3 rows with Shift, deselect the middle one, confirm the anchors move as in behavior 6.8 step 7.

### T-38b-024 — Window-scoped banners and toasts (replaces Compose SnackbarHostState and Android Toast)
- Priority: P0
- Effort: M
- Depends: T-38b-037, T-38b-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt
- Acceptance:
  - One visible banner at a time, queue of at most 3 (UI parity 5); Short 4000 ms; Indefinite until dismissed.
  - Action runs only if the page is still alive; otherwise dropped, nothing written (E-UI-1).
  - Toasts (T1-T3 in UI parity 5) use the same component, non-focusable, window-scoped (E-UI-2).
  - Every snackbar and toast text in p2-behavior 9.3 renders with its exact key and arguments.
  - Test with the fake clock: a Short banner disappears at 4000 ms; an Indefinite one stays.

### T-38b-025 — Exception formatting and user-message mapping
- Priority: P0
- Effort: S
- Depends: T-38b-037, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt, domain/src/main/java/tachiyomi/domain/items/chapter/model/NoChaptersException.kt
- Acceptance:
  - `DetailError::user_message` reproduces ExceptionFormatter.kt:16-38 for HTTP, offline, unknown host, no results, not installed, and generic exceptions (`"Exception"`/`"IOException"` give the message; others `"Class: message"`).
  - Chapter-fetch `NoChapters` uses `no_chapters_error` text (behavior 3.4), not `no_results_found`.
  - Unit tests cover all branches.

### T-38b-026 — Chapter settings sheet wiring (filters, sort, display, defaults)
- Priority: P0
- Effort: S
- Depends: T-38b-009, T-38b-010, T-38b-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Setters: unread, downloaded, bookmarked tri-state (mapping in behavior 6.5); display mode; sort (same key flips direction, new key starts ASC) (behavior 6.6).
  - Each setter writes through the masked write (task 009) and the view updates from the store watch, not an optimistic local copy (E-FLAG-1).
  - Sheet is the only place that writes these defaults-related setters; the defaults actions are in task 014.

### T-38b-027 — Full-cover dialog trigger
- Priority: P2
- Effort: S
- Depends: T-38b-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt
- Acceptance:
  - `show_cover` opens FullCover; dismiss returns to the details page; Escape closes.
  - Cover image itself is area:34; this task only routes the dialog.

### T-38b-028 — Chapter list model and live updates (download events, progress throttle, in-place row updates)
- Priority: P0
- Effort: M
- Depends: T-38b-010, T-38b-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt
- Acceptance:
  - Download status events update one row by chapter id (behavior 6.2); events before `Ready` are dropped as upstream does.
  - Progress events are throttled to 10 Hz per manga; status changes are not throttled (E-DL-6).
  - Rows are keyed by chapter id; the list model is a `gio::ListStore` with one `items_changed` per changed row, not a rebuild.
  - Empty, loading and refreshing states follow UI parity 1.
  - Local manga rows never show queue state.

### T-38b-029 — Tracking counts and auto-open of the track sheet after add
- Priority: P1
- Effort: S
- Depends: T-38b-030, T-38b-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt
- Acceptance:
  - `tracking_count` counts tracks whose tracker is logged in and accepts the source; `has_logged_in` is true when any supported tracker is logged in (behavior 5.1).
  - `auto_open_tracking` is computed from a synchronous store read at decision time (E-TRK-5).
  - Refresh failure toast names the service captured at failure time (E-TRK-6).
  - `track_on_adding_to_library` (default true) gates auto-open.

### T-38b-030 — Tracking home list model (per-tracker rows and row action availability)
- Priority: P1
- Effort: M
- Depends: T-38b-029, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackItem.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt
- Acceptance:
  - `map_track_items` produces one row per logged-in manga tracker that accepts the manga's source, each with its linked track or None (behavior 8.1 `mapToTrackItem`).
  - Row actions follow availability rules of UI parity 4.1; no action dereferences a missing track (E-TRK-1).
  - Refresh-on-open runs once and reports failures as banners (behavior 5.2).
  - Test `map_track_items_one_row_per_logged_in_manga_tracker` and `enhanced_source_filter` pass.

### T-38b-031 — Remote tracker setters, detached writes, and failure reporting
- Priority: P1
- Effort: M
- Depends: T-38b-030, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt
- Acceptance:
  - Status, last chapter read, score, start date, finish date, private: each closes its picker and then runs a detached write (behavior 8.2-8.5, E-TRK-7).
  - Write failures show a banner naming the service (Proposal, behavior change against silent upstream in the remover only).
  - Remote update sends the same wire shape as `setRemote*` (area:27 owns the mapping).
  - Toggle private never runs without a track (E-TRK-1, task 033).

### T-38b-032 — Start and finish date pickers with validity rules
- Priority: P1
- Effort: M
- Depends: T-38b-031, T-req-platform-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt, app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt
- Acceptance:
  - `date_selectable` and `year_selectable` implement behavior 8.5 exactly (no future dates; start <= finish; finish >= start).
  - Picker initial value: stored date or now (`initial_date_ms`).
  - Stored value is local midnight epoch ms; DST gap uses the earliest valid instant (E-DATE-1).
  - Invalid day shows an inline error and disables Save; no silent clamp (UI parity 4.5).
  - Tests `start_date_not_after_finish`, `finish_date_not_before_start`, `no_future_dates`, `year_bounds_match_day_bounds`, `local_midnight_roundtrip` pass.

### T-38b-033 — Track row rules: private toggle, remove date, unlink guards
- Priority: P1
- Effort: S
- Depends: T-38b-030, T-38b-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt
- Acceptance:
  - Private toggle is only available when the track exists and `supports_private()` (E-TRK-1; test `no_track_hides_private_toggle`).
  - Remove date is only offered when the date is set; removal writes `0` and closes back to the home (E-DATE-3, behavior 8.6).
  - Remove-date confirm text uses the exact keys of behavior 9.3.

### T-38b-034 — Enhanced tracker auto-match on "new search"
- Priority: P1
- Effort: S
- Depends: T-38b-030, T-38b-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedMangaTracker.kt
- Acceptance:
  - For enhanced trackers, "new search" runs `match_manga` then `register` without UI (behavior 8.1).
  - No match shows `error_no_match` ("No match found").
  - A thrown error is logged with its real text; the banner still shows "No match found" by default (decision D-6, parity), the real text is shown only when the user accepts D-6.

### T-38b-035 — Tracker search screen state machine and result identity
- Priority: P1
- Effort: M
- Depends: T-38b-030, T-38b-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt
- Acceptance:
  - States Idle, Loading, Done(results, selected), Failed(text) (p2-rust-design 5.3).
  - Auto-search when the initial query is not blank; preselect the result whose URL equals the current link.
  - Confirm writes the private flag onto the selected result and registers it detached, then closes.
  - `TrackSearchResult` equality uses `(tracker_id, remote_id)`; private excluded (E-TRK-9).
  - Cancellation is not shown as an error (E-TRK-8).
  - Private option visible only when `supports_private()`.

### T-38b-036 — Tracker removal: local unlink and optional remote delete
- Priority: P1
- Effort: S
- Depends: T-38b-030, T-38b-024, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt, domain/src/main/java/tachiyomi/domain/track/manga/interactor/DeleteMangaTrack.kt
- Acceptance:
  - Confirm always unlinks locally (`DeleteMangaTrack`); the remote checkbox appears only for deletable trackers and defaults to unchecked.
  - Remote delete runs detached after the local unlink; on failure a banner names the service (decision D-5, Proposal).
  - Title, text, and button labels follow behavior 8.8 and 9.3.

### T-38b-037 — User-facing string keys and formatting module
- Priority: P0
- Effort: S
- Depends: T-req-platform-001
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - Every key in p2-behavior 9.3 exists in `kuro-core::strings` with the exact English text and typed placeholders (no runtime printf).
  - A check script compares the key list with the Kotlin base strings (names only) and fails on any missing key.
  - `%1$s`/`%d` placeholders are replaced by typed arguments; `track_error(name, msg)` yields "name error: msg".

### T-38b-038 — Opener and clipboard for tracker links
- Priority: P1
- Effort: S
- Depends: T-req-platform-001, T-38b-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - Open in browser uses `Opener::open_url` from `kuro-platform`; no direct `xdg-open` call in this crate (addendum rule 1).
  - Copy link writes the URL to the GTK clipboard in the UI layer (not in core); clipboard label is the URL.
  - Both are hidden when the URL is blank (behavior 8.1).

### T-38b-039 — Threading model: reducer on the GTK thread, commands on tokio, detached writes
- Priority: P0
- Effort: M
- Depends: T-req-platform-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt
- Acceptance:
  - `DetailModel::update` is synchronous and never awaits; commands run on a tokio runtime and report back through an `async_channel` drained on the GTK main context.
  - Page dispose cancels page-scoped commands via a `CancellationToken`; writes marked non-cancellable keep running (behavior 10).
  - The model type is not `Send`; the compiler enforces that only `Msg` crosses threads.
  - Progress forwarding and watch subscriptions stop on page dispose.
  - Test: a dispose during a fetch cancels the fetch and still completes a queued write.

### T-38b-040 — Auto-track after mark read (ALWAYS and ASK)
- Priority: P1
- Effort: M
- Depends: T-38b-018, T-38b-031, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt, app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt, domain/src/main/java/tachiyomi/domain/track/manga/interactor/GetMangaTracks.kt
- Acceptance:
  - Runs only for read=true, when `has_logged_in` and `auto_track != Never` (behavior 6.9.9b).
  - Refreshes trackers first (failures show `track_error` toast).
  - Decision uses the max chapter number of the passed chapters; unrecognised (-1) numbers are skipped (E-CHG-2); if no track is behind, no prompt.
  - ALWAYS: pushes `last_chapter_read` and shows `trackers_updated_summary_manga` with the number truncated (E-CHG-3). ASK: banner `confirm_tracker_update` with action OK, pushes only on OK.
  - Re-marking already-read chapters: prompt only when a chapter changed (decision D-8; default keeps the upstream behavior and this is flagged to the user).
  - `TrackChapter` duplicate condition collapsed to one check (E-TRK-3). Failures store a delayed update (area:27) and do not show a banner.

### T-38b-041 — Port upstream unit tests and add the Rust test suite for this part
- Priority: P0
- Effort: M
- Depends: T-38b-001, T-38b-002, T-38b-005, T-38b-006, T-38b-008, T-38b-010, T-38b-011, T-38b-045
- Sources: domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt
- Acceptance:
  - All six `MissingItemsTest` cases for `missingChaptersCount` and `calculateChapterGap` pass as Rust tests (p2-edge-cases section 2).
  - The test suites of p2-edge-cases section 3 (flags, sort, view, separators, selection, next unread, dedupe, download pick, favorite routing, track constraints, track row rules, reducer, database, source mapping) exist and pass.
  - Golden view files for the mixed fixture are generated once and reviewed; regeneration only with `UPDATE_GOLDEN=1`.
  - `cargo test -p kuro-chapters -p kuro-core` passes with no display and no tokio runtime for pure tests.

### T-38b-042 — Android-only plumbing: dropped or replaced (decision record)
- Priority: P3
- Effort: S
- Depends: T-38b-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt, domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt
- Acceptance:
  - Dropped: Voyager `StateScreenModel` and `rememberScreenModel` (replaced by the reducer, task 039); Compose `State`/`@Immutable` annotations; Injekt field injection (replaced by constructor injection in `AppContext`); `Serializable` on `Manga` and `MangaTrack` (Android intent transport; Rust uses plain structs); `Lifecycle` flows (replaced by widget mapping/unmapping).
  - User impact recorded: none visible; no state survives app restart except the DB and preferences, same as upstream.
  - Each dropped item has a one-line reason in the implementation notes.

### T-38b-043 — Track pickers: status, chapter, score
- Priority: P1
- Effort: M
- Depends: T-38b-030, T-38b-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt
- Acceptance:
  - Status: options from `status_list()` in order with labels from `status_label`; initial = current status; confirm writes, cancel closes (behavior 8.2).
  - Chapter: integer picker 0..=max where max = `total_chapters` if > 0 else 10000; initial = `last_chapter_read` truncated (behavior 8.3; tests `chapter_picker_max_default_10000`, `chapter_picker_initial_truncates`).
  - Score: options from `score_list()`; initial = `display_score(track)`; writes the string (behavior 8.4).
  - Chapter picker uses a spin button, not a 10000-row list (UI parity 4.3).

### T-38b-044 — Duplicate and migrate dialog triggers
- Priority: P1
- Effort: S
- Depends: T-38b-011, T-38b-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt, app/src/main/java/eu/kanade/presentation/entries/manga/DuplicateMangaDialog.kt
- Acceptance:
  - Duplicate found on add opens `DuplicateManga(manga, duplicate)`; nothing is written until the user chooses (E-FAV-2).
  - `show_migrate(duplicate)` opens `Migrate(new = current, old = duplicate)`; field naming matches behavior 6.14.
  - Dialog content is part 1 (MangaScreen.kt); this task covers the state and triggers only.

### T-38b-045 — Missing-chapter separators and gap arithmetic
- Priority: P1
- Effort: M
- Depends: T-38b-002, T-38b-010
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt, app/src/main/java/eu/kanade/core/util/CollectionUtils.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt
- Acceptance:
  - `gap` and `gap_chapters` match MissingChapters.kt:40-49 (0 when either number is negative; floor arithmetic).
  - `separators(view, descending)` reproduces `insertSeparators` plus the top/bottom and between rules of behavior 6.7 (test vectors: asc `[5,6]` top count 4; asc `[1,2,4]` count 1 before 4; desc `[6,5,2]` count 2 between and 1 at bottom; desc `[10.5,10]` none; `[-1,5]` none).
  - Separator key uses `Option` for the lower chapter, never the string "null" (E-SEP-1).
  - `missing_count` (for the port of `missingChaptersCount`) matches the upstream test vectors.

---

Total tasks in this file: 45 (P0: 21, P1: 22, P2: 1, P3: 1). IDs run T-38b-001 to T-38b-045 with no gaps.
