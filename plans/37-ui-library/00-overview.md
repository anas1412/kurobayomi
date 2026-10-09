# 37-ui-library — Overview (Library screens, anime and manga)

Lead document for area:37-ui-library. Repo-relative paths are relative to `/home/blackbox/aniyomi-src/` (Apache-2.0, read-only). Target: Kurobayomi, Rust, gtk4-rs 0.11 (GTK 4.16+), no libadwaita, rusqlite, tokio. Labels: **OBSERVED** = what the Kotlin does (cited path:line). **Proposal:** = what Kurobayomi should do.

---

## 1. Purpose and scope

The library is the home screen of Aniyomi for both kinds of entry: manga (`Kind::Manga`) and anime (`Kind::Anime`). It shows the entries the user has added (`favorite` / library membership), split into user categories, and lets the user:

- switch between category pages through a tab strip with counts;
- change how items look (compact grid, cover-only grid, comfortable grid, list);
- filter (six tri-state predicates plus a tracker predicate), sort (per category or global), and search (`id:`, genre, source, author, title, negation);
- select entries (single, range, select all, invert) and run bulk actions (mark seen/read, download next N, change categories, remove from library, open random entry);
- manage categories (create, rename, delete, hide, reorder), including the Change Categories dialog;
- trigger library and category updates from the toolbar;
- continue reading or watching from the Continue button on each cover.

Why it matters for a usable Kurobayomi: the library is the first screen after startup and the entry point to the reader (area:43-reader-core, area:44-reader-viewers) and the player (area:45-player-core). Without it there is no daily workflow. Its P0 tasks (36 of 63) are the minimum for daily use: pipeline, grids, tabs, pages, continue, selection, change categories, settings dialog.

Out of scope here (owned elsewhere): the library SQL views and schema (area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations), the repositories (area:05-data-repos-entries-items), the preference store (area:02-preferences-system), cover loading (area:34-images-covers-cache), the update job (area:26-library-update-jobs), the entry details screens (area:38-ui-entry-details-manga, area:39-ui-entry-details-anime), browse and global search screens (area:40-ui-browse-manga, area:41-ui-browse-anime), the shell and sidebar tab (area:36-ui-shell-navigation-security-webview).

Scope size: 38 files, 6,211 LOC (see [p1-inventory.md](p1-inventory.md)).

---

## 2. Module map

### 2.1 Kotlin packages (what each one is responsible for)

| Package / path (repo-relative) | Responsibility | Key files and LOC |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/` | Anime library state, tab, settings state | `AnimeLibraryScreenModel.kt` 812; `AnimeLibraryTab.kt` 308; `AnimeLibrarySettingsScreenModel.kt` 61; `AnimeLibraryItem.kt` 59 |
| `app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/` | Manga library state, tab, settings state (same pipeline, manga names) | `MangaLibraryScreenModel.kt` 797; `MangaLibraryTab.kt` 329; `MangaLibrarySettingsScreenModel.kt` 61; `MangaLibraryItem.kt` 59 |
| `app/src/main/java/eu/kanade/presentation/library/anime/` and `.../manga/` | Compose pagers, grids, list, settings dialog, empty states (per kind) | `AnimeLibraryPager.kt` 159; `AnimeLibraryContent.kt` 119; `AnimeLibraryList.kt` 84; `AnimeLibraryCompactGrid.kt` 74; `AnimeLibraryComfortableGrid.kt` 73; `AnimeLibrarySettingsDialog.kt` 318 (and manga equivalents) |
| `app/src/main/java/eu/kanade/presentation/library/components/` | Shared library widgets: toolbar, tabs, grid container, badges, global search row | `LibraryToolbar.kt` 163; `LibraryTabs.kt` 52; `LazyLibraryGrid.kt` 46; `LibraryBadges.kt` 61; `GlobalSearchItem.kt` 26 |
| `app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt` | Shared cell widgets: cover, title overlay, selection outline, continue button, metrics | 415 |
| `app/src/main/java/eu/kanade/presentation/library/DeleteLibraryEntryDialog.kt` | Remove-from-library dialog | 81 |
| `app/src/main/java/eu/kanade/presentation/category/` (+ `components/`) | Category list screens, create/rename/delete/change-categories dialogs, FAB, row | `CategoryDialogs.kt` 303; `CategoryListItem.kt` 86; `AnimeCategoryScreen.kt` 113; `MangaCategoryScreen.kt` 113; `CategoryExtensions.kt` 21 |
| `app/src/main/java/eu/kanade/tachiyomi/ui/category/` (`anime/`, `manga/`) | Category screen models (state machine for create, rename, delete, reorder, hide) and tab wrappers | `AnimeCategoryScreenModel.kt` 161; `MangaCategoryScreenModel.kt` 161; `CategoriesTab.kt` 93; `AnimeCategoryTab.kt` 74 |
| `domain/src/main/java/tachiyomi/domain/library/anime/` and `.../manga/` | Library row models (`LibraryAnime`, `LibraryManga`), sort modes and codecs | `LibraryAnime.kt` (`:5-25`); `LibraryManga.kt` (`:5-24`); `AnimeLibrarySortMode.kt`; `MangaLibrarySortMode.kt` |
| `domain/src/main/java/tachiyomi/domain/library/model/` | Shared flag machinery and display mode | `Flag.kt` (`Flag`, `Mask`, `FlagWithMask`); `LibraryDisplayMode.kt` |
| `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt` | Typed library preference keys and defaults | 428 |
| `domain/src/main/java/tachiyomi/domain/category/model/Category.kt` | Category record (`:5-18`) | 14 |
| `core/common/src/main/java/tachiyomi/core/common/preference/` | `TriState`, `CheckboxState<T>` | small |
| `data/src/main/sqldelightanime/` and `data/src/main/sqldelight/` | SQL views and tables read by the library (`animelibView.sq`, `libraryView.sq`, `categories.sq`, `animes_categories`, `mangas_categories`) | owned by area:03 / 04 / 05 |

### 2.2 Proposed Rust module map

From [p1-rust-design.md](p1-rust-design.md) R-1. Rule: `library::*` and `category::*` contain no GTK types and are unit-testable without a display. Only `ui::*` imports `gtk4`.

```
src/
  library/
    mod.rs      Kind, LibraryRow, LibraryState, LibraryCommand
    kind.rs     enum Kind { Anime, Manga } with labels and keys
    model.rs    LibraryRow, Counts, CategoryRef, visual_name()
    flags.rs    sort codec tables (anime/manga), TriState, bit merge, token serialiser
    prefs.rs    typed LibraryPrefs over src/settings.rs
    filter.rs   FilterSet, apply_filters()
    sort.rs     SortSpec, comparator(), random_shuffle()
    search.rs   Query::parse(), LibraryRow::matches()
    select.rs   Selection (ordered Vec<i64> + index), toggle / range / select_all / invert
    pipeline.rs build_map(): rows + categories + prefs -> Vec<PageData>, pure
    db.rs       rusqlite queries on animelibView / libraryView, category rows, writes
    actions.rs  mark_read, download_next, set_categories_delta, remove_from_library, random_pick
    worker.rs   single DB thread, request/response over async-channel
  category/
    mod.rs, model.rs (CategoryRecord { id, name, order, flags, hidden }), ops.rs
  ui/library/   mod.rs (LibraryView), toolbar.rs, tabs.rs, pages.rs, grid.rs, list.rs,
                card.rs, settings.rs, delete_dialog.rs, change_category_dialog.rs, empty.rs
  ui/category/  mod.rs (CategoriesView), list.rs (ListBox with drag), dialogs.rs
```

Key types (from R-2): `LibraryRow` is one unified type for both kinds (`Kind` discriminates labels and the few kind-specific fields such as `AiringTime`, which is anime only). Wrapper types from Kotlin (`AnimeLibraryItem`, `MangaLibraryItem`) are not ported as separate types; badges live on `LibraryRow` (see [p1-inventory.md](p1-inventory.md) row for `AnimeLibraryItem.kt`).

Kotlin-to-Rust mapping for the library scope is in [p1-rust-design.md](p1-rust-design.md) R-12 (Compose `LazyVerticalGrid` to `gtk::GridView`, `HorizontalPager` to `gtk::Stack`, `combinedClickable` to `GestureClick` plus `GestureLongPress`, `icu_collator` for `Collator`, `rand::StdRng` for `kotlin.random.Random`).

---

## 3. Reading guide

All files are in this folder.

| Document | What it contains |
|---|---|
| [p1-inventory.md](p1-inventory.md) | One row per scope file (38 files): path, LOC, every declaration, purpose, port decision, target Rust module, priority, task ids. |
| [p1-behavior.md](p1-behavior.md) | The behaviour spec (B-0 to B-21): tab entry, the three init pipelines, filters, sort comparators, search, category tabs and toolbar title, item rendering, empty states, selection, bulk actions, settings dialog, change categories, category management, update triggers, concurrency, timings and limits. |
| [p1-data-and-config.md](p1-data-and-config.md) | Models (D-1), enum and codec tables with test vectors (D-2), preference keys with defaults (D-3), sort flag bit layout (D-4), category rows (D-5), shared preference map (D-6), SQL views and tables (D-7), files and caches (D-8), constants (D-10), category-delete cascade (D-11). |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases and defects E-01 to E-49 with upstream citation, Rust rule and test id; upstream tests to port (T-1); new unit tests (T-2); fixtures (T-3); integration and UI tests (T-4); race conditions (T-5). |
| [p1-rust-design.md](p1-rust-design.md) | Rust design: module layout (R-1), core types (R-2), flags and TriState (R-3), pure pipeline (R-4), sort (R-5), selection (R-6), search (R-7), database (R-8), category ops (R-9), background model (R-10), UI composition (R-11), Android to Linux mapping (R-12), error handling (R-13), pitfalls (R-14), crates (R-15), test hooks (R-16). |
| [p1-tasks.md](p1-tasks.md) | 63 implementation tasks T-37a-001 to T-37a-063 with priority, effort, dependencies, sources and acceptance criteria. |
| [p1-ui-parity.md](p1-ui-parity.md) | What must be visible and how it behaves on the desktop: library page (U-1), category screens (U-2), settings dialog (U-3), selection and gestures (U-4), badges (U-5), typography (U-6), responsive behaviour (U-7), animations (U-8), English strings with keys (U-9), gaps in the current mockup (U-10). |
| [p1-topic-sort-filter-search.md](p1-topic-sort-filter-search.md) | Deep dive: order of operations (must match), worked examples for sort and tracker mean, filter truth table, tracker filter, search grammar, random, decision table. |
| [p1-topic-category-membership.md](p1-topic-category-membership.md) | Deep dive: data model, membership sets, dialog preselection, tri-state cycle, apply semantics, category operations, reorder algorithm, edge-case summary. |
| [p1-topic-selection-and-bulk-actions.md](p1-topic-selection-and-bulk-actions.md) | Deep dive: selection state machine, transitions, range algorithm, select all and invert, bulk actions and their effects, interaction with search and filters, keyboard and mouse matrix, snapshot refresh, tests. |

---

## 4. Key design decisions and open questions

The first seven need the user's call. Each has a recommendation. Items already settled by a proposal in [p1-behavior.md](p1-behavior.md) B-21 are listed at the end.

| # | Question | Options | Recommendation |
|---|---|---|---|
| Q1 | Right-click on a library cover: range toggle or context menu? The UI parity doc (U-4) maps right-click to range toggle. area:34-images-covers-cache states that right-click opens the same menu long-press opens on Android. The two areas conflict. | (a) right-click = range toggle (U-4); (b) right-click = context menu (area:34); (c) right-click = context menu, Shift+click = range, long-press = range (as on Android) | (c). GTK users expect right-click menus. Ctrl+click toggles, Shift+click ranges. Update U-4 and area:34 to match. |
| Q2 | Which preference keys are shared between anime and manga? Upstream reads the same keys in both libraries (D-6). | Adopt the D-6 table as is, or split more or less | Adopt D-6 (T-37a-049). Shared: display mode, badges, tabs, counts, continue button, hidden categories, categorized display, interval filter, update restriction, downloaded-only. Split: sort, sort seeds, all filters, columns, last-used category. |
| Q3 | Keep the Aniyomi sort bit layout in the category `flags` column (D-4)? Backups carry these flags (area:21-backup-format-and-create). | (a) bit-exact Aniyomi layout; (b) new Rust-native encoding with a converter | (a). The layout is six bits, costs nothing, and keeps Aniyomi backups importable without conversion. Keep the typed `"ALPHABETICAL,ASCENDING"` string in settings for the same reason. |
| Q4 | Interval-custom filter (`filterFnIntervalCustom`) on desktop. Upstream only lets release builds hide it (E-45). | (a) show the row; (b) hide the row and still evaluate the filter when the restriction is on | (b). Matches T-37a-057 and E-45. |
| Q5 | Pull-to-refresh on desktop (U-1.9, T-37a-036, P2). | (a) drop; (b) refresh button and F5 | (b). Drop the pull gesture. |
| Q6 | Default category is not offered in the Change Categories dialog (E-12). Entries with no category are implicitly in Default. | (a) keep upstream; (b) offer Default as a checkbox | (a). Keeps the membership model identical to upstream; removing all categories already puts the entry in Default. |
| Q7 | TrackerMean for entries whose tracks are all logged out (E-14, T-37a-058). Upstream returns NaN for them, which sorts unpredictably. | (a) keep NaN; (b) map to `-1.0` | (b). Matches T-37a-058. |

Settled by proposal (see [p1-behavior.md](p1-behavior.md) B-21 and the tasks): global update message names the target "Updating library" (T-37a-050); downloaded filter stays forced when downloaded-only mode is on and is reflected in `hasActiveFilters` (T-37a-056); the Continue button no longer depends on the unread badge (E-02, T-37a-033); hidden categories are kept when entries move between categories (E-07, T-37a-030); range selection compares by id, not index (E-09, T-37a-055); random sort uses one seed per kind (E-21, T-37a-011).

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What is needed | Tasks that need it |
|---|---|---|
| area:02-preferences-system | Typed preference store; keys, types and defaults; `AnimeLibrarySort`, `MangaLibrarySort`, `LibraryDisplayMode`, `TriState` codecs (its T-02a-007, 008, 009); `state.downloaded_only` (`__APP_STATE_pref_downloaded_only`, not backed up) | T-37a-004, T-37a-063, T-37a-049 |
| area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations | `libraryView` and `animelibView` columns, joins and `ORDER BY` | T-37a-005, T-37a-044 |
| area:05-data-repos-entries-items | Library query repositories, `UpdateAnime` for remove-from-library | T-37a-001, T-37a-031 |
| area:07-domain-entries-items | `SetSeenStatus`, `SetReadStatus`, next-chapter and next-episode queries | T-37a-027, T-37a-033 |
| area:08-domain-library-category-history-updates | Category domain operations and category link repository methods | T-37a-029, T-37a-030, T-37a-043, T-37a-060 |
| area:14-source-local-and-metadata | `isLocal()` for the local badge | T-37a-006 |
| area:16-source-api-anime | Season-parent counts (anime) | T-37a-048 |
| area:17-source-app-layer | Source name and language for badges and search (`getOrStub`, `lang`) | T-37a-006 |
| area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared | Download cache for download badges, download-next-N, delete-from-library cleanup | T-37a-028, T-37a-031, T-37a-044 |
| area:26-library-update-jobs | `start_now(kind, Some(category))` and "already running" result | T-37a-035 |
| area:27-tracking-core | Tracker logins and per-entry tracks for the tracker filter and TrackerMean sort | T-37a-010, T-37a-037 |
| area:34-images-covers-cache | Cover cache for cards; cache invalidation on library removal | T-37a-018, T-37a-032 |
| area:36-ui-shell-navigation-security-webview | Library page shell, sidebar tab, `NavStyle` (manga tab position), in-window toast component | T-37a-045, T-37a-061, T-37a-062 |
| area:40-ui-browse-manga, area:41-ui-browse-anime | Global search screens (opened from the "Search for X globally" row) | T-37a-022 |

### 5.2 What others need from this area

- **area:36-ui-shell-navigation-security-webview**: the library page as a route with a search entry point (`AnimeLibraryTab.search`, `MangaLibraryTab.search`).
- **area:38-ui-entry-details-manga**, **area:39-ui-entry-details-anime**: the Change Categories dialog (`ChangeCategoryDialog`) is reused from the entry details screen; the dialog is owned here (T-37a-029).
- **area:41-ui-browse-anime**, **area:42-ui-updates-history-stats-storage-upcoming**: `LibraryRow` and the grid cell widgets (`EntryComfortableGridItem` pattern, T-37a-020); area:42 needs the library predicate, which must be the same as the one used here (see 5.3).
- **area:47-settings-screens-general**: the library settings page reads and writes the same typed preferences through the settings store (T-37a-063).
- **area:43-reader-core**, **area:45-player-core**: the Continue button opens the reader or player for the next unread chapter or episode (T-37a-044 and T-37a-033).

### 5.3 Contracts that must match across areas

- **Library membership predicate.** area:42 (see its correctness note) says Kotlin's library list is not `favorite = 1`. It is the rows returned by the library query, one row per (entry, category) pair. area:37, area:42 and area:05 must use the same predicate. Recommendation: the Rust `library_entries` view is the single definition; area:37 reads it.
- **Right-click semantics.** Q1 above; area:34 and area:37 must agree.
- **Shared preference keys.** Q2 above; area:02 owns the keys, area:37 owns the decision table.

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, filtered to rows with file starting `37-ui-library/` in [TASKS.csv](../TASKS.csv). Lint: `ok` for [p1-tasks.md](p1-tasks.md).

Total: 63 tasks.

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 18 | 17 | 1 | 0 | 36 |
| P1 | 16 | 4 | 1 | 0 | 21 |
| P2 | 4 | 0 | 0 | 0 | 4 |
| P3 | 2 | 0 | 0 | 0 | 2 |
| **Total** | **40** | **21** | **2** | **0** | **63** |

Upper-bound effort (S = 1 day, M = 3 days, L = 7.5 days, from the effort definitions): P0 about 76.5 engineer-days, P1 about 35.5, P2 about 4, P3 about 2. Total about 118 engineer-days (about 24 weeks) for one engineer. These are caps, not estimates.

### 6.1 P0 critical path (in order)

Dependencies followed are within-area task dependencies. Cross-area dependencies are listed in 5.1.

1. **T-37a-001** Library row and category model types (S). Needs area:05, area:08.
2. **T-37a-004** Typed library preferences with exact keys and defaults (M). Needs area:02.
3. **T-37a-006** Badge decoration (S). Needs area:14, area:17.
4. **T-37a-008** Filter engine (M).
5. **T-37a-009** Sort engine comparators (M).
6. **T-37a-013** Library pipeline with debounce, generations and off-thread compute (L). Needs T-37a-012 (search) and T-37a-007.
7. **T-37a-016** Category tab strip with counts (M).
8. **T-37a-017** Paged category view with lazy window and persisted index (M).
9. **T-37a-018** Compact grid cards (M). Needs area:34.
10. **T-37a-033** Continue button with visibility fix (M). Needs area:07.
11. **T-37a-044** Manga continue reading and chapter-next semantics (S). Needs area:03, area:23.

Weight of this chain: about 31.5 engineer-days upper bound. Parallel to it: T-37a-005 (queries, needs area:03/04), T-37a-025 (selection model), T-37a-029 (change categories), T-37a-037 to T-37a-040 (settings tabs), T-37a-051 (test suite), T-37a-055 (crash-proof indexing), T-37a-060 (category link methods), T-37a-061 (shell wiring).

### 6.2 Risks

1. **Cross-area predicate drift.** The library predicate and the category link model are defined in several areas (5.3). If area:42 or area:05 drift, Counts and Stats disagree. Mitigation: one `library_entries` definition and a test that compares counts across screens.
2. **Pipeline cost on large libraries.** Upstream filters are O(n²) for tracker lookups (T-37a-054). The pipeline must stay O(n log n) and run off the GTK thread (R-14 pitfalls 3 and 4). T-37a-013 is L and carries the most risk.
3. **Right-click conflict** between area:34 and area:37 (Q1). Resolve before implementing T-37a-018.
4. **Flag compatibility** with Aniyomi backups (Q3). Changing the bit layout breaks backup restore (area:22-backup-restore).
5. **rusqlite on one thread.** `rusqlite::Connection` is not `Sync` (R-14 pitfall 5). Every library query goes through the worker (`library/worker.rs`). A slow query blocks the library page. Mitigation: the pipeline reads the database in one batch per refresh.
6. **GTK virtualization.** `GridView` needs size requests (R-14 pitfall 8). Without them the adaptive column math breaks.
7. **Random sort parity.** Kotlin `Random` order cannot be reproduced exactly with `StdRng`. Parity of the sequence is not required (R-12), so the Random order for a given seed differs from Aniyomi.
8. **Mockup gaps.** The mockup (`src/screens.rs` `library()`, lines 154-272) has no data layer. Every item in U-10 must be replaced, not patched.

---

## 7. Definition of done for this area (observable)

Each item can be checked by hand against a library of at least 500 entries, and each pure rule is also covered by `cargo test --lib library::` with no display.

1. The library shows an anime page and a manga page, switched from the sidebar. Each page shows category tabs with counts. The category tab index is remembered between launches.
2. The Display tab switches among compact grid, cover-only grid, comfortable grid and list. The choice persists and applies to both kinds (D-6).
3. The Filter tab applies the six tri-state predicates and the tracker predicate. Exclude works. The filter indicator is lit when a filter is active, including when downloaded-only mode forces the downloaded filter.
4. The Sort tab sets every anime sort type and every manga sort type, toggles direction, and writes to the selected category only when "categorized display" is on. Sorting by Random is stable until Random is chosen again.
5. Search follows the grammar in [p1-topic-sort-filter-search.md](p1-topic-sort-filter-search.md) section 6. `id:<n>` matches the row id. Otherwise the whole query matches title, author, artist or description (case-insensitive substring). If that fails, every comma-separated term must match the source name (substring) or a genre (exact, case-insensitive). A leading `-` negates a term. An empty query matches everything, and a bare `-` does not crash (E-03, E-04).
6. Selection: click opens the entry when nothing is selected and toggles when something is selected. Ctrl+click toggles, Shift+click selects a range within the same category, Escape clears the selection, Ctrl+A selects all and Ctrl+I inverts. Right-click follows the decision in Q1.
7. The action bar runs mark seen or read, download next N, change categories and remove from library. Remove from library shows two checkboxes and clears the cover cache entries for the removed items (T-37a-031, T-37a-032).
8. Change Categories shows a tri-state checkbox per category (not Default, E-12). Applying changes memberships and keeps hidden-category links (E-07).
9. The Continue button shows on a cover when the entry has an unread chapter or an unseen episode, regardless of whether the unread badge is shown. It opens the reader or player at the next item.
10. Category management: create, rename, delete, hide and reorder work. Names are not trimmed, and uniqueness is case-sensitive (E-40). Deleting a category clears its preference references (D-11).
11. Empty, no-results, filter-empty and category-empty states show the upstream messages from [p1-ui-parity.md](p1-ui-parity.md) U-9.
12. Toolbar update starts a library or category update. A second start while one runs shows "An update is already running". Global update reads "Updating library" (T-37a-050).
13. Opening a random entry picks from the current page's visible entries.
14. Pure modules (`library::flags`, `filter`, `sort`, `search`, `select`, `pipeline`) pass the test vectors in [p1-data-and-config.md](p1-data-and-config.md) section D-12 and the tests in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section T-2.
15. No library interaction blocks the GTK thread for more than one frame on a 5,000-entry fixture. Measured with `tracing` spans around the pipeline.
