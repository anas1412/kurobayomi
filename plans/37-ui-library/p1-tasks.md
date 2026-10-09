# 37-ui-library — Implementation tasks

Priority meanings (from the plan contract): P0 needed for daily use; P1 parity most users rely on; P2 long tail and polish; P3 Android-only with a drop or replacement decision.

Effort: S ≤ 1 day, M ≤ 3 days, L ≤ 1.5 weeks.

Test ids (T2-…, T4-…) refer to p1-edge-cases-and-tests.md. Topic deep dives: p1-topic-sort-filter-search.md, p1-topic-category-membership.md, p1-topic-selection-and-bulk-actions.md.

Sources are repo-relative to `/home/blackbox/aniyomi-src/`.

---

## Foundations: model, codecs, preferences, data

### T-37a-001 — Library row and category model types
- Priority: P0
- Effort: S
- Depends: area:05-data-repos-entries-items, area:08-domain-library-category-history-updates
- Sources: domain/src/main/java/tachiyomi/domain/library/anime/LibraryAnime.kt, domain/src/main/java/tachiyomi/domain/library/manga/LibraryManga.kt, domain/src/main/java/tachiyomi/domain/category/model/Category.kt, domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt
- Acceptance:
  - One `LibraryRow` type carries every field in p1-data-and-config.md D-1.1 and D-1.2 for both kinds, with `unseen() = total - seen`, `has_started()`, `has_bookmarks()`.
  - `CategoryRecord::visual_name()` returns "Default" for id 0 and the name otherwise.
  - Display mode enum has exactly four variants (T2-CODEC-01).

### T-37a-002 — Sort flag codec for anime and manga
- Priority: P0
- Effort: S
- Depends: T-37a-001
- Sources: domain/src/main/java/tachiyomi/domain/library/anime/model/AnimeLibrarySortMode.kt, domain/src/main/java/tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt, domain/src/main/java/tachiyomi/domain/library/model/Flag.kt, domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt
- Acceptance:
  - Bit layout matches p1-data-and-config.md D-4: type mask 0b00111100, direction 0b01000000.
  - All five upstream LibraryFlagsTest assertions are ported (T2-CODEC-01 to T2-CODEC-05).
  - Flags 0 decode to Descending; manga flags 48 decode to Alphabetical; both recorded as tests.
  - Token codec (D-2.3, D-2.4) round-trips (T2-CODEC-06, T2-CODEC-07).
  - Decision on the Default category seed (flags 0 → Z→A) is written into the migration notes.

### T-37a-003 — TriState codec and predicate application
- Priority: P0
- Effort: S
- Depends: T-37a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt, domain/src/main/java/tachiyomi/domain/entries/TriState.kt, core/common/src/main/java/tachiyomi/core/common/preference/CheckboxState.kt
- Acceptance:
  - Cycle DISABLED → ENABLED_IS → ENABLED_NOT → DISABLED (T2-FILTER-01).
  - `apply` truth table matches the upstream `applyFilter` (T2-FILTER-02).
  - Stored names are `DISABLED`, `ENABLED_IS`, `ENABLED_NOT`.
  - CheckboxState cycles for State (None↔Checked) and TriState (Exclude→None→Include→Exclude) are implemented and tested.

### T-37a-004 — Typed library preferences with exact keys and defaults
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system, T-37a-001
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt
- Acceptance:
  - Every key in p1-data-and-config.md D-3 exists with the listed default on a fresh store.
  - Anime and manga keys are separate where D-3 says [A] or [M]; shared keys are the ones in D-6.
  - Each setter emits a change event that the library pipeline subscribes to.
  - App-state keys use the `__APP_STATE_` prefix.

### T-37a-005 — Library row queries over animelibView and libraryView
- Priority: P0
- Effort: M
- Depends: area:04-db-anime-schema-migrations, area:03-db-manga-schema-migrations, T-37a-001
- Sources: data/src/main/sqldelightanime/view/animelibView.sq, data/src/main/sqldelight/view/libraryView.sq, data/src/main/sqldelightanime/view/episodestatsView.sq, data/src/main/sqldelightanime/view/animehistorystatsView.sq, data/src/main/sqldelightanime/view/animeseasonstatsView.sq, data/src/main/sqldelight/data/excluded_scanlators.sq
- Acceptance:
  - Queries return one row per category link for favorites only (T4-DB-01).
  - Season-parent counts (`fetch_type = 0`) and manga scanlator exclusion match the upstream SQL.
  - Column aliases match D-7 so the Rust mapper reads anime `totalCount`, `seenCount`, `latestUpload`, `episodeFetchedAt`, `lastSeen` and manga `totalChapters`, `readCount`, `latestUpload`, `chapterFetchedAt`, `lastRead`.

### T-37a-006 — Badge decoration (download, unseen, local, language)
- Priority: P0
- Effort: S
- Depends: T-37a-004, area:14-source-local-and-metadata, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryItem.kt
- Acceptance:
  - Each badge value is zeroed or blanked when its preference is off (B-3 step 2).
  - Download filter still consults the cache when the badge is off (T2-FILTER-03).
  - T2-PIPE-01 passes.

### T-37a-007 — Category grouping and display-category rule
- Priority: P0
- Effort: S
- Depends: T-37a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt
- Acceptance:
  - Entries group by category id in DB row order (T2-PIPE-02).
  - An entry in two categories appears on both pages and counts once in the distinct count (T2-PIPE-04).
  - The Default page is hidden when entries exist and none is uncategorized (T2-PIPE-13).
  - Entries only in hidden categories are absent (T2-PIPE-06).

### T-37a-008 — Filter engine
- Priority: P0
- Effort: M
- Depends: T-37a-003, T-37a-004, T-37a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt
- Acceptance:
  - Six predicates plus the tracker predicate combined with AND (T2-FILTER-03 to T2-FILTER-07).
  - Downloaded-only forces the downloaded filter to Is (B-4.2 item 1).
  - Interval filter is evaluated only when restriction `manga_outside_release_period` is set (T2-FILTER-06).
  - Tracker predicate is O(1) per row after precompute (T2-TRACK-01 to T2-TRACK-05).

### T-37a-009 — Sort engine comparators
- Priority: P0
- Effort: M
- Depends: T-37a-002, T-37a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 256-340), app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt (lines 256-340)
- Acceptance:
  - Every anime and manga type compares as in B-5.3; zero-count rows sort last in both directions (T2-SORT-03, T2-SORT-04).
  - Alphabetical tiebreak is always ascending (T2-SORT-09).
  - Collation uses icu_collator with the LANG locale over lowercased titles (T2-SORT-01).
  - Manga has no AiringTime (D-2.4).

### T-37a-010 — Tracker-mean sort key
- Priority: P1
- Effort: S
- Depends: T-37a-009, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 264-276), app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt
- Acceptance:
  - Mean of logged-in tracker 10-point scores; entries without a logged-in score use -1.0 (T2-SORT-06).
  - Tracker option appears in the sort list only when a tracker is logged in (B-12.2).

### T-37a-011 — Random sort with per-kind seed
- Priority: P1
- Effort: S
- Depends: T-37a-004, T-37a-009
- Sources: domain/src/main/java/tachiyomi/domain/category/anime/interactor/SetSortModeForAnimeCategory.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetSortModeForMangaCategory.kt
- Acceptance:
  - Seed changes only when Random is chosen (T2-SORT-05, T2-SORT-07).
  - Same permutation for the same seed and input; no tiebreak.
  - Sequence parity with Kotlin is explicitly not required (decision recorded).

### T-37a-012 — Search query parser and matcher
- Priority: P0
- Effort: M
- Depends: T-37a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryItem.kt
- Acceptance:
  - `id:` prefix, whole-string match on title/author/artist/description, comma terms, negation, source-name contains, genre exact (T2-SEARCH-01 to T2-SEARCH-09).
  - E-03 (empty matches all), E-04 (lone hyphen), E-05 (commas) behave as documented.
  - Search hint shown when the query starts with "-" (UI copy in p1-ui-parity.md U-1.1).

### T-37a-013 — Library pipeline with debounce, generations and off-thread compute
- Priority: P0
- Effort: L
- Depends: T-37a-006, T-37a-007, T-37a-008, T-37a-009, T-37a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 103-176), app/src/main/java/eu/kanade/presentation/components/AppBar.kt (line 65)
- Acceptance:
  - Search debounce is 250 ms (SEARCH_DEBOUNCE_MILLIS).
  - Stale results are discarded by generation counter (R-01 in the edge-case file).
  - Pipeline runs on the DB worker, never on the GTK thread.
  - T4-UI-01 passes with fixtures.

### T-37a-014 — hasActiveFilters derivation
- Priority: P0
- Effort: S
- Depends: T-37a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 153-175)
- Acceptance:
  - True when any of the six filters or any tracker filter is not Disabled.
  - Includes the downloaded-only override (T-37a-056 links here) (T2-PIPE-07).

### T-37a-015 — Toolbar title, count and pill rules
- Priority: P0
- Effort: S
- Depends: T-37a-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 792-810), app/src/main/java/eu/kanade/presentation/library/components/LibraryToolbar.kt
- Acceptance:
  - Every combination of tabs shown, counts shown, and searching yields the title and count in B-7.3.
  - Out-of-range page index uses the default title with no count.

### T-37a-016 — Category tab strip with counts
- Priority: P0
- Effort: M
- Depends: T-37a-013
- Sources: app/src/main/java/eu/kanade/presentation/library/components/LibraryTabs.kt, app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibraryContent.kt
- Acceptance:
  - Tabs shown only when `showPageTabs && categories.size > 1` where showPageTabs includes search (E-19).
  - Per-tab count when `show_number_of_items` is on or a search is active.
  - Horizontal scroll when tabs overflow; Ctrl+PageUp and Ctrl+PageDown switch pages.

### T-37a-017 — Paged category view with lazy window and persisted index
- Priority: P0
- Effort: M
- Depends: T-37a-016, T-37a-004
- Sources: app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibraryPager.kt, app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibraryContent.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt
- Acceptance:
  - Only current ±1 pages are built (B-7.1).
  - Page change persists `last_used_category` (app state).
  - Index is clamped on every rebuild (E-17, E-26; T2-PIPE-08).

### T-37a-018 — Compact grid cards
- Priority: P0
- Effort: M
- Depends: T-37a-017, T-37a-024, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt, app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibraryCompactGrid.kt, app/src/main/java/eu/kanade/presentation/library/components/LazyLibraryGrid.kt, app/src/main/java/eu/kanade/presentation/library/manga/MangaLibraryCompactGrid.kt
- Acceptance:
  - Cover 2:3, 4 px spacing, adaptive 128 px when columns is 0, fixed when N (T2-UI-01).
  - Title scrim and two-line title; selected style per U-1.4.
  - Badges top-left (downloads, unread) and top-right (language or local).

### T-37a-019 — Cover-only grid mode
- Priority: P0
- Effort: S
- Depends: T-37a-018
- Sources: app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibraryCompactGrid.kt, app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibraryPager.kt
- Acceptance:
  - No title overlay; continue button uses the large size (32 px) at bottom-right.

### T-37a-020 — Comfortable grid mode
- Priority: P0
- Effort: S
- Depends: T-37a-018
- Sources: app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibraryComfortableGrid.kt, app/src/main/java/eu/kanade/presentation/library/manga/MangaLibraryComfortableGrid.kt, app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt
- Acceptance:
  - Title below the cover, 12 px, two lines, padding 4 px.

### T-37a-021 — List mode with list-size row height
- Priority: P1
- Effort: M
- Depends: T-37a-017, T-37a-024
- Sources: app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibraryList.kt, app/src/main/java/eu/kanade/presentation/library/manga/MangaLibraryList.kt, app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt
- Acceptance:
  - Row height is page height / list size minus 3 / list size, clamped to at least 48 px; list size 0 gives 76 px (T2-UI-02, E-23).
  - Continue button after the badges.

### T-37a-022 — Global search row
- Priority: P0
- Effort: S
- Depends: T-37a-012, T-37a-017, area:40-ui-browse-manga, area:41-ui-browse-anime
- Sources: app/src/main/java/eu/kanade/presentation/library/components/GlobalSearchItem.kt, app/src/main/java/eu/kanade/presentation/library/components/LazyLibraryGrid.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt
- Acceptance:
  - Shown only when query is non-empty; label is `Search for "%1$s" globally`.
  - Click opens the global search screen with the query (the anime screen is GlobalAnimeSearchScreen, owned by area:41-ui-browse-anime; the manga screen is GlobalMangaSearchScreen, owned by area:40-ui-browse-manga).

### T-37a-023 — Library empty, no-results, filter and category-empty states
- Priority: P0
- Effort: S
- Depends: T-37a-013, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibraryPager.kt (LibraryPagerEmptyScreen), app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt (lines 181-194)
- Acceptance:
  - Messages match B-9 and U-1.6.
  - Library-empty screen only when no search, no filter, and no visible entries (E-49).
  - Zero pages never build a Stack (T2-PIPE-12).

### T-37a-024 — Badge widgets
- Priority: P1
- Effort: S
- Depends: T-37a-006
- Sources: app/src/main/java/eu/kanade/presentation/library/components/LibraryBadges.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/Badges.kt
- Acceptance:
  - Download and unread use count text; local uses the folder icon; language is uppercased (U-5).
  - Badges hidden when count is 0 or value blank.

### T-37a-025 — Selection model
- Priority: P0
- Effort: M
- Depends: T-37a-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 610-689), app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt
- Acceptance:
  - Toggle, range (same category, by id), select all, invert, clear (T2-SEL-01 to T2-SEL-08).
  - Selection survives page and filter changes; the header reports how many selected entries are hidden (E-10).

### T-37a-026 — Selection header and action bar
- Priority: P0
- Effort: M
- Depends: T-37a-025
- Sources: app/src/main/java/eu/kanade/presentation/library/components/LibraryToolbar.kt (lines 45-50, 130-157), app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt (line 292)
- Acceptance:
  - Header shows count, select all, invert, cancel; Escape cancels.
  - Action bar shows change category, mark, download, delete; download hidden when any selected entry is local.
  - Bar hidden when selection is empty.

### T-37a-027 — Mark viewed and read for selection
- Priority: P0
- Effort: S
- Depends: T-37a-025, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 505-519), app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt (lines 490-504)
- Acceptance:
  - Writes for every selected entry; selection cleared after writes finish (E-35).
  - Partial failure toast "N of M updated" (T2-BULK-02).

### T-37a-028 — Download next N and download all unviewed for selection
- Priority: P1
- Effort: M
- Depends: T-37a-025, area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 466-503), app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt (lines 456-500), app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt
- Acceptance:
  - Amounts 1, 5, 10, 25, and all (T2-BULK-01, T2-BULK-04).
  - Queued and downloaded are skipped before the take (E-36).
  - Menu labels: "Next chapter/episode", "Next 5 chapters/episodes", "Next 10 …", "Next 25 …", and "Unread"/"Unwatched" (unviewed), per kind.

### T-37a-029 — Change categories dialog
- Priority: P0
- Effort: M
- Depends: T-37a-025, T-37a-030, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 695-718), app/src/main/java/eu/kanade/presentation/category/components/CategoryDialogs.kt (lines 197-303)
- Acceptance:
  - Common checked, mixed as Exclude, others unchecked; Default never offered (T2-CAT-01, T2-CAT-03, T2-CAT-04).
  - Empty-category case shows the message and "Edit categories" which opens the categories screen.
  - Tri-state cycle and the include/exclude output match B-13.2.

### T-37a-030 — Membership delta apply that preserves hidden links
- Priority: P0
- Effort: S
- Depends: T-37a-005, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 562-578), data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt (lines 95-102)
- Acceptance:
  - New set is computed from all links, not only visible ones (E-07 fix; T2-CAT-02).
  - Mixed categories left unchanged (T2-CAT-03).
  - Replace-all runs in one transaction (T4-DB-02).

### T-37a-031 — Delete-from-library dialog and action
- Priority: P1
- Effort: M
- Depends: T-37a-025, area:05-data-repos-entries-items, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/presentation/library/DeleteLibraryEntryDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 528-553)
- Acceptance:
  - Two independent checkboxes; confirm disabled until at least one is checked (T2-DEL-01, T2-DEL-02).
  - Downloaded checkbox hidden for local entries; local files never deleted (T2-DEL-04).
  - Entries de-duplicated by id before the action.

### T-37a-032 — Cache invalidation and timestamp bump on library removal
- Priority: P1
- Effort: S
- Depends: T-37a-031, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt
- Acceptance:
  - Cover and background cache entries deleted (T2-DEL-03).
  - `cover_last_modified` persisted with the removal (E-27 fix).

### T-37a-033 — Continue button with visibility fix
- Priority: P0
- Effort: M
- Depends: T-37a-008, T-37a-018, area:07-domain-entries-items, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 450-452), app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt (lines 436-438), app/src/main/java/eu/kanade/tachiyomi/util/episode/EpsiodeGetNextUnseen.kt, app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt (lines 206-212)
- Acceptance:
  - Visibility depends on the raw unseen count, not the unread badge (E-02; T2-SEL-09).
  - Opens the next item after entry-level filters; external player when `pref_always_use_external_player` is on.
  - Toast "Next chapter not found" when nothing is found, for both kinds (E-25; T2-CONT-02).
  - Click debounced 300 ms per entry (R-11).

### T-37a-034 — Open random entry from current page
- Priority: P1
- Effort: S
- Depends: T-37a-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 596-604), app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt (lines 148-158)
- Acceptance:
  - Random pick from the filtered and searched current page; toast "No entries found in this category" when empty.
  - Index guard against out-of-range (E-17).

### T-37a-035 — Library and category update triggers
- Priority: P1
- Effort: S
- Depends: area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt (lines 500-521), app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt (lines 105-112)
- Acceptance:
  - Whole-library and current-category triggers from the header menu and F5.
  - Already-running state shows "An update is already running" (T2-UPD-02).
  - Category trigger shows "Updating category".

### T-37a-036 — Refresh affordance without pull gesture
- Priority: P2
- Effort: S
- Depends: T-37a-035
- Sources: app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibraryContent.kt (lines 85-96)
- Acceptance:
  - F5 updates the current category; no spinner-only feedback; toast shows the outcome (E-33).

### T-37a-037 — Filter settings tab
- Priority: P0
- Effort: M
- Depends: T-37a-004, T-37a-008, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibrarySettingsDialog.kt (lines 79-160), app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibrarySettingsScreenModel.kt (lines 38-46)
- Acceptance:
  - Row order and labels per B-12.1 and U-3.1 for each kind.
  - Tri-state cycles on click; downloaded row locked to Is when downloaded-only is on.
  - Tracker rows: none for 0 trackers, one row for 1, heading and rows for many.

### T-37a-038 — Sort settings tab
- Priority: P0
- Effort: M
- Depends: T-37a-009, T-37a-011, T-37a-040
- Sources: app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibrarySettingsDialog.kt (lines 163-225), app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibrarySettingsScreenModel.kt (lines 52-60)
- Acceptance:
  - Option list per kind (B-12.2); TrackerMean only with a tracker; AiringTime anime-only.
  - Same row flips direction; other row keeps direction (B-5.8).
  - Random row reshuffles with a new seed.

### T-37a-039 — Display settings tab
- Priority: P0
- Effort: M
- Depends: T-37a-004, T-37a-018, T-37a-019, T-37a-020, T-37a-021
- Sources: app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibrarySettingsDialog.kt (lines 227-318), app/src/main/java/eu/kanade/presentation/library/manga/MangaLibrarySettingsDialog.kt (lines 227-318)
- Acceptance:
  - Four mode chips; slider 0 to 10 with "Auto" at 0; label switches between list size and items per row.
  - Five overlay toggles and two tab toggles write immediately.
  - Decision on one columns preference for desktop recorded (E-24).

### T-37a-040 — Per-category sort persistence rules
- Priority: P0
- Effort: S
- Depends: T-37a-002, area:08-domain-library-category-history-updates
- Sources: domain/src/main/java/tachiyomi/domain/category/anime/interactor/SetSortModeForAnimeCategory.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetSortModeForMangaCategory.kt
- Acceptance:
  - Categorized display on: only the target category changes (T2-SORT-10).
  - Categorized display off: every category and the global pref get the same value (E-18).
  - Decision on restoring per-category sorts when the setting turns off is recorded.

### T-37a-041 — Category management screens
- Priority: P1
- Effort: L
- Depends: T-37a-042, T-37a-043, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/presentation/category/AnimeCategoryScreen.kt, app/src/main/java/eu/kanade/presentation/category/MangaCategoryScreen.kt, app/src/main/java/eu/kanade/presentation/category/components/CategoryListItem.kt, app/src/main/java/eu/kanade/presentation/category/components/CategoryFloatingActionButton.kt, app/src/main/java/eu/kanade/tachiyomi/ui/category/CategoriesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/category/anime/AnimeCategoryTab.kt
- Acceptance:
  - Anime and Manga sub-pages; manga sub-page opened from the manga "Edit categories" flow.
  - Drag reorder persists; hide and show toggles with icon and dim; rename and delete per row.
  - Empty state text and add button per U-2.2.

### T-37a-042 — Category dialogs with name validation
- Priority: P1
- Effort: S
- Depends: T-37a-041
- Sources: app/src/main/java/eu/kanade/presentation/category/components/CategoryDialogs.kt (lines 40-194), app/src/main/java/eu/kanade/tachiyomi/ui/category/anime/AnimeCategoryScreenModel.kt
- Acceptance:
  - Add and OK disabled per rules (T2-CATSCR-01).
  - Dialog survives a categories refresh (E-29; T2-CATSCR-02).
  - Duplicate check covers hidden categories too (decision on E-40 recorded; T2-CAT-07).

### T-37a-043 — Category domain operations
- Priority: P1
- Effort: M
- Depends: area:08-domain-library-category-history-updates, T-37a-001
- Sources: domain/src/main/java/tachiyomi/domain/category/anime/interactor/CreateAnimeCategoryWithName.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/RenameAnimeCategory.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/HideAnimeCategory.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/DeleteAnimeCategory.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/ReorderAnimeCategory.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/GetVisibleAnimeCategories.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/DeleteMangaCategory.kt
- Acceptance:
  - Behaviour per B-15.5 and D-11 (pref cleanup, order re-index, default reset) (T2-CAT-05, T2-CAT-06).
  - Reorder serialised by a mutex; one shared result type (E-42).
  - T4-DB-03 passes.

### T-37a-044 — Manga continue reading and chapter-next semantics
- Priority: P0
- Effort: S
- Depends: T-37a-033, area:03-db-manga-schema-migrations, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt (lines 206-230), app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterGetNextUnread.kt
- Acceptance:
  - Scanlator-excluded chapters never chosen (T2-CONT-03).
  - Opens the reader route for the chapter; toast when none.

### T-37a-045 — Navigation style aware manga tab position and back behaviour
- Priority: P1
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt (lines 67-125), app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt
- Acceptance:
  - Manga page placed under More when NavStyle is MOVE_MANGA_TO_MORE; back returns to the More parent.
  - The upstream jump-to-anime-library behaviour (B-14.11) is replaced by a return to the origin tab (decision recorded).

### T-37a-046 — Keyboard and mouse equivalents
- Priority: P0
- Effort: S
- Depends: T-37a-025, T-37a-016
- Sources: app/src/main/java/eu/kanade/presentation/library/components/LibraryToolbar.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt (lines 278-283)
- Acceptance:
  - Matrix in p1-topic-selection-and-bulk-actions.md §7 implemented.
  - Escape order: clear selection first, then clear search (T4-UI-01).

### T-37a-047 — Hidden-category and empty-visible-category edge handling
- Priority: P1
- Effort: S
- Depends: T-37a-007, T-37a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 407-415)
- Acceptance:
  - T2-PIPE-06, T2-PIPE-12, T2-PIPE-13 pass; no panic with zero visible categories and an active search.

### T-37a-048 — Season-parent counts and labels for anime entries
- Priority: P1
- Effort: S
- Depends: T-37a-005, area:16-source-api-anime
- Sources: data/src/main/sqldelightanime/view/animeseasonstatsView.sq, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt
- Acceptance:
  - Season-parent counts drive unseen and started (T2-PIPE-11).
  - Badge tooltip reads "unseen seasons" for those entries.

### T-37a-049 — Decide anime and manga shared preferences
- Priority: P1
- Effort: S
- Depends: T-37a-004
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Decision table in p1-data-and-config.md D-6 is ratified: display mode, badges, tabs, counts, continue button, hidden categories, interval, restriction, downloaded-only are shared; sort, seeds, filters, columns, last-used category are split.
  - Migration path written for users with existing Aniyomi backups.

### T-37a-050 — Global update message wording
- Priority: P2
- Effort: S
- Depends: T-37a-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt (lines 105-112), app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt
- Acceptance:
  - Global update says "Updating library"; category update keeps "Updating category" (T2-UPD-01).

### T-37a-051 — Test suite for library pure modules and fixtures
- Priority: P0
- Effort: M
- Depends: T-37a-002, T-37a-003, T-37a-008, T-37a-009, T-37a-012, T-37a-025, T-37a-030
- Sources: domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt
- Acceptance:
  - Every T2-* id in p1-edge-cases-and-tests.md §T-2 has a passing test.
  - Fixtures live in tests/fixtures/library (§T-3).
  - `cargo test --lib library::` passes without a display.

### T-37a-052 — Drop Android-only UI glue
- Priority: P3
- Effort: S
- Depends: T-37a-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt (lines 97, 285-293), app/src/main/java/eu/kanade/tachiyomi/ui/category/CategoriesTab.kt (lines 76-78)
- Acceptance:
  - Dropped: haptic feedback, animated vector tab icons, the splash ready flag, pull-to-refresh physics.
  - Drop note lists what the user loses: nothing functional; only tactile and motion feedback.

### T-37a-053 — Replace the Android URI handler for the getting-started link
- Priority: P3
- Effort: S
- Depends: T-37a-023
- Sources: app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibraryPager.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt (lines 181-194)
- Acceptance:
  - Opens through gtk::UriLauncher (or the default browser).
  - Drop note: the Android intent path is removed; the link still opens.

### T-37a-054 — Precompute tracker lookups and remove the O(n²) filter
- Priority: P1
- Effort: S
- Depends: T-37a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 230-241)
- Acceptance:
  - No per-row map rebuild; the pipeline over 10,000 fixture rows completes within 50 ms on the reference machine (proposal target).

### T-37a-055 — Crash-proof indexing and stale anchors
- Priority: P0
- Effort: S
- Depends: T-37a-025, T-37a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 596-689), app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt (line 144)
- Acceptance:
  - T2-SEL-04 passes (stale anchor adds only the clicked row).
  - No direct indexing of user-driven indices anywhere in library modules (E-09, E-17).

### T-37a-056 — hasActiveFilters includes the downloaded-only override
- Priority: P1
- Effort: S
- Depends: T-37a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 153-175)
- Acceptance:
  - T2-PIPE-07 passes; filter icon tinted when the downloaded filter is forced.

### T-37a-057 — Interval-custom filter gating for the desktop build
- Priority: P2
- Effort: S
- Depends: T-37a-008, T-37a-037
- Sources: app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibrarySettingsDialog.kt (lines 120-128)
- Acceptance:
  - Row hidden on desktop; filter still evaluated when the restriction is on (E-45; T2-FILTER-06).

### T-37a-058 — TrackerMean NaN decision and fix
- Priority: P2
- Effort: S
- Depends: T-37a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 264-276)
- Acceptance:
  - T2-SORT-06 passes; entries with only logged-out tracks sort as -1.0 (E-14 decision recorded).

### T-37a-059 — Selection snapshot refresh after count changes
- Priority: P1
- Effort: S
- Depends: T-37a-025, T-37a-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt (lines 610-659)
- Acceptance:
  - Snapshots refreshed on each recompute; ids that left the library are dropped; counts shown are live (race R-02).

### T-37a-060 — Category link repository methods
- Priority: P0
- Effort: S
- Depends: area:08-domain-library-category-history-updates, T-37a-005
- Sources: data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt, data/src/main/java/tachiyomi/data/category/anime/AnimeCategoryRepositoryImpl.kt
- Acceptance:
  - Read of all links (incl. hidden) and replace-all write in one transaction (T4-DB-02).
  - Visible-only read kept for the dialog's display (D-5.2).

### T-37a-061 — Library page shell wiring
- Priority: P0
- Effort: M
- Depends: area:36-ui-shell-navigation-security-webview, T-37a-013, T-37a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt
- Acceptance:
  - Sidebar index 0 (anime) and 1 or 5 (manga) per nav style.
  - Re-selecting the sidebar item opens the settings dialog (B-1.2).
  - External search injection sets the query (B-1.3).
  - Sidebar hidden while in selection mode (proposal, B-1.4).

### T-37a-062 — In-window toast component
- Priority: P1
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt (lines 103-112, 150-158)
- Acceptance:
  - One toast at a time, auto-hide after 4 s, newer replaces older.
  - Messages verbatim from U-9 and U-1.8.

### T-37a-063 — Library settings persistence to the Rust settings store
- Priority: P1
- Effort: S
- Depends: T-37a-004, area:02-preferences-system
- Sources: src/settings.rs
- Acceptance:
  - Typed LibraryPrefs reads and writes through the existing settings store, with defaults from D-3 when missing.
  - Corrupt or unknown values fall back to defaults without a crash.
