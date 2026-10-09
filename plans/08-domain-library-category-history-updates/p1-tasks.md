# Area 08 — Domain: library, category, history, updates — implementation tasks (part 1 of 1)

All paths are repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`). Ids run T-08a-001 to T-08a-044. Companion documents: `p1-behavior.md` (algorithms), `p1-data-and-config.md` (models, bit layouts, 87 preference keys, SQL), `p1-rust-design.md` (crate layout, signatures), `p1-edge-cases-and-tests.md` (test vectors), topic files `p1-topic-flag-algebra-and-sort-modes.md`, `p1-topic-library-pipeline.md`, `p1-topic-category-lifecycle.md`, `p1-topic-history-and-next-item.md`, `p1-topic-updates-feed.md`, `p1-topic-library-preferences-quirks.md`.

Coverage map: every one of the 58 scope files is referenced by at least one task (see `p1-inventory.md` for the file-to-task columns). Cross-area dependencies use `area:<id>`; this area never re-implements what an `area:` dependency owns (schema, repositories of entries/chapters, preference store, update jobs, downloads, trackers, reader/player, UI widgets).

Priority guide for this area: P0 = needed to browse the library, keep progress and add entries to categories; P1 = category management, history/updates tabs, sort/filter settings, backup interplay; P2 = polish and rarely used preferences; P3 = Android-only drops.

## Foundation (module, flags, codecs)

### T-08a-001 — kuro-domain crate skeleton, EntryKind, error type
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/domain/DomainModule.kt, domain/src/main/java/tachiyomi/domain/category/model/Category.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - workspace crate `crates/kuro-domain` builds with `cargo build -p kuro-domain` and has no dependency on `gtk4`, `glib` or any `cfg(target_os)`
  - modules `kind`, `error`, `category`, `history`, `library`, `updates` exist as in `p1-rust-design.md` section 1 and `EntryKind::{Manga, Anime}` exposes `membership_table()`, `entry_fk()`, `history_table()`, `item_fk()`, `timestamp_col()` with the exact identifiers `mangas_categories/animes_categories`, `manga_id/anime_id`, `history/animehistory`, `chapter_id/episode_id`, `last_read/last_seen`
  - `DomainError` has the variants `Db`, `InvalidName`, `DuplicateName`, `NotFound`, `SystemCategory`, `Join` with the English messages of `p1-rust-design.md` section 5
  - consts `SYSTEM_CATEGORY_ID = 0`, `UPDATES_LIMIT = 500`, `UPDATES_WINDOW_MONTHS = 3`, `SEARCH_DEBOUNCE_MS = 250` are defined once and referenced elsewhere

### T-08a-002 — Flag algebra (merge/contains) with LibraryFlagsTest port
- Priority: P0
- Effort: S
- Depends: T-08a-001
- Sources: domain/src/main/java/tachiyomi/domain/library/model/Flag.kt, domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt
- Acceptance:
  - `merge(value, flag, mask) == (value & !mask) | (flag & mask)` and `contains(value, flag, mask) == (flag == (value & mask))` are implemented with explicit parentheses
  - the five tests of `LibraryFlagsTest` are ported to `crates/kuro-domain/tests/library_flags.rs` and pass, including `0b01011100` for `LastRead|Asc` merged with `DateAdded|Asc` and `0b01000000` for the default sort
  - property test: `merge` is idempotent and leaves bits outside the mask untouched for random `i64` inputs

### T-08a-003 — LibrarySort type, flag layout and string codec (manga and anime)
- Priority: P0
- Effort: M
- Depends: T-08a-002
- Sources: domain/src/main/java/tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt, domain/src/main/java/tachiyomi/domain/library/anime/model/AnimeLibrarySortMode.kt
- Acceptance:
  - `SortType` has the 11 variants of `p1-data-and-config.md` section 3.1 with flags 0x00,0x04,0x08,0x0C,0x10,0x14,0x18,0x1C,0x20,0x30 (anime only),0x3C and `Direction` Ascending = 0x40 / Descending = 0
  - `LibrarySort::from_flags(kind, Some(f))` returns Alphabetical for the unassigned type patterns (manga 0x24,0x28,0x2C,0x30,0x34,0x38; anime 0x24,0x28,0x2C,0x34,0x38) and `from_flags(kind, None) == DEFAULT (Alphabetical, Ascending)`
  - `serialize/deserialize` match section 5 of `p1-topic-flag-algebra-and-sort-modes.md`: `"LAST_READ"` (no comma) gives DEFAULT, `"LAST_SEEN,ASCENDING"` parsed as manga gives Alphabetical Ascending, unknown direction token gives Descending, tokens are exact uppercase
  - the manga tokens `LAST_READ, LAST_MANGA_UPDATE, UNREAD_COUNT, TOTAL_CHAPTERS, LATEST_CHAPTER, CHAPTER_FETCH_DATE` and the anime tokens `LAST_SEEN, LAST_ANIME_UPDATE, UNSEEN_COUNT, TOTAL_EPISODES, LATEST_EPISODE, EPISODE_FETCH_DATE, AIRING_TIME` round-trip
  - `apply_to(category_flags)` replaces only bits 0x7C (test: `0x81 -> 0xDD` for DateAdded Ascending)
  - `Category::sort(kind)` equals `Category?.sort` including the null case

### T-08a-004 — LibraryDisplayMode enum and codec
- Priority: P0
- Effort: S
- Depends: T-08a-001
- Sources: domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt
- Acceptance:
  - variants `CompactGrid, ComfortableGrid, List, CoverOnlyGrid` with `ALL` in that order and `DEFAULT = CompactGrid`
  - stored strings `COMPACT_GRID`, `COMFORTABLE_GRID`, `COVER_ONLY_GRID`, `LIST`; any other input (including lower case and empty) deserialises to `CompactGrid` without error
  - the registry codec for key `pref_display_mode_library` uses these functions

### T-08a-005 — Library row model and library repository (libraryView / animelibView)
- Priority: P0
- Effort: M
- Depends: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations, area:05-data-repos-entries-items, T-08a-001
- Sources: domain/src/main/java/tachiyomi/domain/library/manga/LibraryManga.kt, domain/src/main/java/tachiyomi/domain/library/anime/LibraryAnime.kt, data/src/main/sqldelight/view/libraryView.sq, data/src/main/sqldelightanime/view/animelibView.sq
- Acceptance:
  - `LibraryRepo::all(kind)` returns one `LibraryRow` per (favourite entry, category) and one row with `category = 0` for entries without categories (fixture: entry in 3 categories yields 3 rows)
  - derived values `pending = total - done`, `has_started = done > 0`, `has_bookmarks = bookmarks > 0` match `LibraryManga`/`LibraryAnime`; manga `fillermarks` is always 0
  - manga counts exclude chapters of excluded scanlators (fixture with `excluded_scanlators`)
  - NULL aggregates are coalesced to 0 so no mapper can fail (replaces the Kotlin NPE-retry loop); a DB error produces `LibraryState::Error`
  - library size shown anywhere is counted with distinct entry ids, never with row counts (test: 3-category entry counts once)

## Category

### T-08a-006 — Category model, CategoryUpdate, system category seed
- Priority: P0
- Effort: S
- Depends: T-08a-001
- Sources: domain/src/main/java/tachiyomi/domain/category/model/Category.kt, domain/src/main/java/tachiyomi/domain/category/model/CategoryUpdate.kt, data/src/main/sqldelight/data/categories.sq
- Acceptance:
  - `Category { id, name, order, flags, hidden }` and `CategoryUpdate { id, name?, order?, flags?, hidden? }` with `is_system()` = `id == 0`
  - a fresh database contains the system row `(0, "", -1, <flags>)` and a raw `DELETE FROM categories WHERE _id = 0` aborts with `System category can't be deleted` (trigger test)
  - first-run initialisation writes `flags = 64` (Alphabetical Ascending) into the system row instead of upstream's 0; an existing database with flags 0 is left untouched (documented deviation Q28)
  - display name of the system category is the localized "Default" (`MR.strings.label_default`) through one helper `Category::display_name`

### T-08a-007 — Category repository: reads, visible filter, per-entry lookup, reactive watch
- Priority: P0
- Effort: M
- Depends: T-08a-006, area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations, T-08a-032
- Sources: domain/src/main/java/tachiyomi/domain/category/manga/repository/MangaCategoryRepository.kt, domain/src/main/java/tachiyomi/domain/category/anime/repository/AnimeCategoryRepository.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/GetMangaCategories.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/GetAnimeCategories.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/GetVisibleMangaCategories.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/GetVisibleAnimeCategories.kt
- Acceptance:
  - `all`, `visible`, `get`, `by_entry(visible_only)` return the column mapping `_id->id, sort->order, hidden==1` and are ordered `ORDER BY sort, _id` (by-entry variants add the same ORDER BY)
  - `watch_all/watch_visible` emit a new snapshot within 100 ms after a category insert/update/delete or a membership change (test with in-memory DB)
  - manga and anime use separate databases: creating a category in one never appears in the other
  - `hidden = 1` categories are excluded from `visible` and from `by_entry(visible_only = true)`

### T-08a-008 — Category writes: create, rename, hide toggle, partial update (with validation)
- Priority: P1
- Effort: M
- Depends: T-08a-007, T-08a-012, T-08a-026
- Sources: domain/src/main/java/tachiyomi/domain/category/manga/interactor/CreateMangaCategoryWithName.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/CreateAnimeCategoryWithName.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/RenameMangaCategory.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/RenameAnimeCategory.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/HideMangaCategory.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/HideAnimeCategory.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/UpdateMangaCategory.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/UpdateAnimeCategory.kt
- Acceptance:
  - `create("X")` on a fresh database gives `order = 0`; the next two give 1 and 2; flags equal the current global sort flag (default 0x40; with global `LastRead Descending` -> 0x04)
  - names are trimmed; `""` and whitespace-only give `InvalidName`; a name equal (case-sensitive, after trim) to another existing category (hidden ones included) gives `DuplicateName`; `"a"` and `"A"` may coexist
  - `rename` of a category to its own current name is a successful no-op; `toggle_hidden` flips `hidden` and survives a restart
  - `update_partial` with all optional fields `None` changes nothing; with `name` only changes only the name (SQL `COALESCE` semantics)
  - failures return `DomainError::Db` and are logged with `tracing::error!`; the UI shows "InternalError: Check crash logs for further information"

### T-08a-009 — Category reorder with id-based move and per-kind lock
- Priority: P1
- Effort: S
- Depends: T-08a-007
- Sources: domain/src/main/java/tachiyomi/domain/category/manga/interactor/ReorderMangaCategory.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/ReorderAnimeCategory.kt
- Acceptance:
  - `move_before(id, before)` renumbers all non-system categories to `0..n-1` in one transaction; the system category's `sort` is never written
  - fixture [A, B, C] move C before A gives orders C=0, A=1, B=2; `before = None` moves to the end; unknown id or the system id returns `NotFound` (Kotlin `Unchanged`)
  - two concurrent moves are serialized by a `tokio::sync::Mutex` per kind and the final state equals applying them in lock order
  - the same lock is taken by delete (T-08a-010); a stress test with 50 concurrent moves leaves orders a permutation of `0..n-1`
  - the drag handle in the categories screen calls `move_before(dragged, next_sibling_id)` once on drop, not once per crossed row

### T-08a-010 — Delete category with order renumber and preference cleanup (bug-fixed)
- Priority: P1
- Effort: M
- Depends: T-08a-007, T-08a-026, area:09-domain-source-track-download-release-misc
- Sources: domain/src/main/java/tachiyomi/domain/category/manga/interactor/DeleteMangaCategory.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/DeleteAnimeCategory.kt
- Acceptance:
  - deleting a user category removes its membership rows by cascade, renumbers every remaining category (system included) to its list index and clears `default_category`/`default_anime_category` only if equal to the deleted id
  - the id string is removed from all five sets per kind: `library_update_categories`, `library_update_categories_exclude`, `remove_exclude_categories`, `download_new_categories`, `download_new_categories_exclude` (anime: `animelib_update_categories`, `animelib_update_categories_exclude`, `remove_exclude_anime_categories`, `download_new_anime_categories`, `download_new_anime_categories_exclude`) — regression test for upstream Q8 where manga's exclude set was skipped
  - the three SQL steps (delete, renumber) run in one transaction; deleting id 0 returns `SystemCategory` and changes nothing
  - sequence test reproduces the `sort` values of the table in `p1-topic-category-lifecycle.md` section 1 (system row becomes 0 after the first delete)
  - after the call an entry that lost its only category appears in the Default category (library view coalesce)

### T-08a-011 — Entry category membership: set_entry_categories
- Priority: P0
- Effort: S
- Depends: T-08a-007, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetMangaCategories.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/SetAnimeCategories.kt
- Acceptance:
  - `set_entry_categories(kind, entry, ids)` deletes all membership rows of the entry and inserts the de-duplicated ids in one transaction; id 0 and non-existent ids are dropped; an empty list means "Default"
  - `[2,2,0,99]` with only category 2 existing yields exactly one row `(entry, 2)`
  - the version-bump trigger (`insert_manga_category_update_version`) is exercised: `version` increments once per inserted row unless `is_syncing = 1`
  - errors are returned as `Result` and logged; callers in the UI ignore them like upstream

### T-08a-012 — Per-category sort setter, reset flags, initial flags, global display mode
- Priority: P0
- Effort: M
- Depends: T-08a-003, T-08a-007, T-08a-026
- Sources: domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetSortModeForMangaCategory.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/SetSortModeForAnimeCategory.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/ResetMangaCategoryFlags.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/ResetAnimeCategoryFlags.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetMangaDisplayMode.kt, domain/src/main/java/tachiyomi/domain/category/anime/interactor/SetAnimeDisplayMode.kt
- Acceptance:
  - `set_sort_mode(category?, type, dir)` with `categorized_display = true` and a category updates only that category's flags (`flags + type + dir` via `apply_to`); with it false (or no category) it stores the global sort preference and writes the computed flags to every category
  - choosing Random generates and stores a new 32-bit seed in `library_random_manga_sort_seed` / `library_random_anime_sort_seed` on every call (also when Random was already selected); a fake RNG test asserts one draw per call
  - `reset_category_flags(kind)` writes `global.type + global.direction` to all categories and is invoked for **both** kinds when `categorized_display` is switched off (regression test for Q9)
  - `set_display_mode(mode)` writes the single shared key `pref_display_mode_library`
  - `initial_flags(kind)` = `global.type.flag | global.direction.flag` is used by T-08a-008

### T-08a-013 — Library page model: group by category, Default-category visibility rule, hidden categories
- Priority: P0
- Effort: M
- Depends: T-08a-005, T-08a-007, T-08a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt
- Acceptance:
  - pages are the visible categories in `sort` order; the Default page is dropped iff the library has at least one row and none with `category = 0`; with an empty library every visible category (Default included) is kept
  - entries in several categories appear once per page; entries whose categories are all hidden appear on no page
  - `LibraryItem` carries the badge fields exactly as in behaviour 2.3 (download count only if the download badge is on, pending count only if the unread badge is on, local flag only if the local badge is on, language only if the language badge is on; the anime unseen badge reads `display_unread_badge`)
  - toolbar title/count function `toolbar_title(default_title, default_category_title, page)` passes the four-combination test of `p1-edge-cases-and-tests.md` 2.2
  - `is_library_empty` is based on the distinct entry count over all displayed pages

### T-08a-014 — Library filter engine (TriState filters, tracker filters, downloaded-only)
- Priority: P0
- Effort: M
- Depends: T-08a-013, T-08a-026, area:09-domain-source-track-download-release-misc
- Note: Tracker framework area:27-tracking-core is not yet authored; the tracker-side inputs are mapped to area:09 (domain track models) for now.
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt, domain/src/main/java/tachiyomi/domain/entries/TriState.kt, core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt
- Acceptance:
  - `TriState` has `DISABLED, ENABLED_IS, ENABLED_NOT`, `next()` cycles DISABLED -> IS -> NOT -> DISABLED, stored by name; `apply_filter` follows `DISABLED -> true`, `IS -> p`, `NOT -> !p`
  - filters run in the short-circuit order downloaded, unread/unseen, started, bookmarked, completed, interval-custom, tracking; the fixture of `p1-edge-cases-and-tests.md` 2.2 gives the listed id sets
  - tracking filter uses only logged-in trackers: both maps empty or all DISABLED means no effect; excluded wins over included; an entry with no tracks is excluded when any IS tracker filter is set
  - downloaded filter treats local-source entries (`source == 0`) as downloaded; "downloaded only" (`pref_downloaded_only`) forces `ENABLED_IS` and the toggle reports disabled
  - `has_active_filters` follows behaviour 2.4 with the Q21 fix (interval-custom counted only when `manga_outside_release_period` is in the item restrictions)
  - tracker filter keys are the dynamic family `pref_filter_library_tracked_{id}_v2` / `pref_filter_animelib_tracked_{id}_v2`

### T-08a-015 — Library sort engine (comparators, collation, random seed, tracker mean, airing time)
- Priority: P0
- Effort: L
- Depends: T-08a-003, T-08a-013, area:09-domain-source-track-download-release-misc
- Note: Tracker framework area:27-tracking-core is not yet authored; tracker mean inputs are mapped to area:09 (domain track models) for now.
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt, core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetSortModeForMangaCategory.kt
- Acceptance:
  - each page is sorted with the sort decoded from its own category flags; ties on the main key are broken by lowercase-title collation ascending even when the direction is descending (comparator order: type, then reversed if descending, then alphabetical)
  - alphabetical uses `icu_collator` at primary strength for the user's locale; fixture titles `Alpha, beta, Émile` sort as `Alpha, beta, Émile` ascending
  - UnreadCount/UnseenCount place zero-count entries last in both directions (fixture of `p1-edge-cases-and-tests.md` 2.2); AiringTime sorts by pending count first, then by `next_episode_airing_at` with 0 last in both directions
  - the airing time is decoded as `(viewer_flags & 0x0FFFFFFFF000000) >> 24`
  - TrackerMean uses -1.0 for unscored or untracked entries and treats NaN as -1.0
  - Random shuffles with a ChaCha8 RNG seeded from the stored seed: same seed gives the same order, a new seed a different order; the sort is stable (`slice::sort_by`)
  - alphabetical keys (lowercase + collation key) are computed once per item, not inside the comparator; 20,000 items sort in under 100 ms in a release-profile benchmark

### T-08a-016 — Library search grammar and debounce contract
- Priority: P0
- Effort: S
- Depends: T-08a-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryItem.kt, app/src/main/java/eu/kanade/presentation/components/AppBar.kt
- Acceptance:
  - `matches(query, item)` implements: `id:<n>` prefix (case-insensitive prefix, `toLongOrNull` semantics, `id:abc` and `id:` match nothing); otherwise true if title, author, artist or description contains the query (case-insensitive), or every comma-separated trimmed sub-constraint satisfies `negatable(source name contains || any genre equals, case-insensitive)`
  - a leading `-` inverts a sub-constraint after `trimStart`; an empty or blank query matches everything
  - the UI search entry delays emission by 250 ms (`SEARCH_DEBOUNCE_MS`), tabs are shown while a search is active, and per-category match counts are shown even when `display_number_of_items` is off
  - empty-page messages follow behaviour 2.6 step 5 with the exact texts "No results found", "No match found", "Category is empty", "Your library is empty"
  - the global-search affordance appears on empty search results and opens global search with the same query

### T-08a-017 — Bulk change of categories for a selection (common/mix tri-state)
- Priority: P1
- Effort: S
- Depends: T-08a-007, T-08a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt, app/src/main/java/eu/kanade/presentation/category/components/CategoryDialogs.kt, core/common/src/main/java/tachiyomi/core/common/preference/CheckboxState.kt
- Acceptance:
  - `bulk_dialog_state(entries)` returns per category (system excluded) `Checked` if in all entries, `Mixed` if in some, `None` otherwise; clicking cycles `Checked<->None` for plain states and `None -> Include -> Exclude -> None` for mixed ones
  - confirm sends `include` (Checked/Include) and `exclude` (None) id lists; mixed (Exclude) ids are untouched; apply computes `(current - exclude) + include` from **all** categories of each entry including hidden ones (Q26 fix; test: entry in hidden H keeps H) and writes via `set_entry_categories`
  - the dialog with zero categories shows "You don't have any categories yet." with a single "Edit categories" action; dialog title "Set categories", buttons Edit / Cancel / OK
  - the operation is non-cancellable (completes if the window closes)

## History

### T-08a-018 — History models and repository (queries, view, reactive list)
- Priority: P0
- Effort: M
- Depends: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations, area:06-data-repos-history-track-source-misc, T-08a-032
- Sources: domain/src/main/java/tachiyomi/domain/history/manga/model/MangaHistory.kt, domain/src/main/java/tachiyomi/domain/history/manga/model/MangaHistoryUpdate.kt, domain/src/main/java/tachiyomi/domain/history/manga/model/MangaHistoryWithRelations.kt, domain/src/main/java/tachiyomi/domain/history/manga/repository/MangaHistoryRepository.kt, domain/src/main/java/tachiyomi/domain/history/anime/model/AnimeHistory.kt, domain/src/main/java/tachiyomi/domain/history/anime/model/AnimeHistoryUpdate.kt, domain/src/main/java/tachiyomi/domain/history/anime/model/AnimeHistoryWithRelations.kt, domain/src/main/java/tachiyomi/domain/history/anime/repository/AnimeHistoryRepository.kt
- Acceptance:
  - `HistoryRepo` exposes `upsert`, `list(query)` (watchable), `latest()`, `by_entry(id)`, `reset_one(history_id)`, `reset_entry(entry_id)`, `clear_all() -> bool`, `total_duration()` (manga only) with the SQL of `p1-data-and-config.md` section 4
  - `list` returns one row per entry (its row with the max timestamp), only rows with timestamp > 0, ordered newest first; the 4-row fixture of `p1-topic-history-and-next-item.md` 2 gives `[M.c2, N.n1]`, after resetting row 2 gives `[N.n1, M.c1]`
  - `latest()` returns the newest row with timestamp > 0 or `None`
  - the query text is escaped for `LIKE` (`\`, `%`, `_` with `ESCAPE '\'`) and matched against `lower(title)` of the lowercased query
  - `HistoryWithRelations.cover` carries `(entry id, source id, favourite, url, cover_last_modified)`; the models `create()` defaults are `-1, -1, null` and `-1` for duration
  - errors of reset/upsert are logged and swallowed by the callers, `clear_all` returns `false` on error

### T-08a-019 — History recording: upsert semantics, session timer, throttle, incognito
- Priority: P0
- Effort: M
- Depends: T-08a-018
- Note: Call sites need the reader and player areas area:43-reader-core and area:45-player-core, which are not yet authored in plans/.
- Sources: domain/src/main/java/tachiyomi/domain/history/manga/interactor/UpsertMangaHistory.kt, domain/src/main/java/tachiyomi/domain/history/anime/interactor/UpsertAnimeHistory.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - manga upsert: two upserts for one chapter `(1000, 5000)` then `(2000, 3000)` leave one row with `last_read = 2000`, `time_read = 8000`; anime upsert keeps one row with the latest `last_seen`
  - `HistoryRecorder` (manga): `start()` on chapter open/window focus, `flush()` on chapter change/window blur/close adds `now - start` (0 if no start; clamped to 0..12 h), then clears the start; `flush` is a no-op in incognito mode
  - anime recorder: `tick()` is called every playback second but writes at most one upsert per 5 s of playback plus unconditional writes on pause, seek end, episode change, stop and window close
  - flushes run as detached tasks so closing the window cannot cancel them, and shutdown waits up to 2 s for pending flushes
  - a `FOREIGN KEY` failure (chapter deleted meanwhile) is logged and swallowed

### T-08a-020 — History list view model: date headers, resume, search
- Priority: P1
- Effort: M
- Depends: T-08a-018, T-08a-022, T-08a-025
- Sources: domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetMangaHistory.kt, domain/src/main/java/tachiyomi/domain/history/anime/interactor/GetAnimeHistory.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryTab.kt, app/src/main/java/eu/kanade/presentation/history/manga/MangaHistoryScreen.kt, app/src/main/java/eu/kanade/presentation/history/manga/components/MangaHistoryItem.kt
- Acceptance:
  - items are grouped with a date header (local date of the timestamp) before the first item and at each date change; header label is "Today", "Yesterday", "N days ago" (2..6), else the short date (or the user's `app_date_format`) when `relative_time_v2` is on, always the absolute date when off
  - row text is `Ch. {n} - {time}` (anime `Ep. {n} - {time}`) with the number formatted `#.###` and `.` decimal separator, and just `{time}` when the number is not greater than -1; time uses the locale short time
  - row click resumes via `next_items(entry, item, only_pending = false).first()`; none gives the toast "Next chapter not found" / "Next Episode not found!"; the heart button is shown only when the entry is not in the library and runs the add-to-library flow of T-08a-030 including the duplicate dialog
  - the remove dialog "Remove" with text "This will remove the read date of this chapter. Are you sure?" (anime: "...watch date of this episode...") and the checkbox "Reset all chapters for this manga" (anime: "Reset all episodes for this anime") calls `reset_entry` when ticked else `reset_one`
  - empty state texts "Nothing read recently" / "Nothing watched recently", with a non-empty query "No results found"; a flow error shows the internal-error toast
  - the search query is applied without a debounce in the domain and the UI cancels stale queries (latest wins)

### T-08a-021 — History removal: reset one, reset entry, clear all
- Priority: P1
- Effort: S
- Depends: T-08a-018
- Sources: domain/src/main/java/tachiyomi/domain/history/manga/interactor/RemoveMangaHistory.kt, domain/src/main/java/tachiyomi/domain/history/anime/interactor/RemoveAnimeHistory.kt, app/src/main/java/eu/kanade/presentation/history/HistoryDialogs.kt
- Acceptance:
  - `reset_one` and `reset_entry` set the timestamp to 0 and keep the rows and `time_read` (total duration unchanged), hiding them from the list; `clear_all` hard-deletes all rows and returns true on success
  - the clear-all confirmation shows title "Remove everything", text "Are you sure? All history will be lost." and OK/Cancel; on success the toast "History deleted" is shown, on failure nothing
  - resetting the newest row of an entry reveals its previous row in the list; resetting every row removes the entry from the list
  - the "Clear database" maintenance action `remove_reset_history()` (delete rows with timestamp 0) is exposed for area:47

### T-08a-022 — Next chapter / next episode computation
- Priority: P0
- Effort: M
- Depends: T-08a-018, area:07-domain-entries-items, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetNextChapters.kt, domain/src/main/java/tachiyomi/domain/history/anime/interactor/GetNextEpisodes.kt, domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt, domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt
- Acceptance:
  - `next_items(kind, entry, from: Option<item>, only_pending)` returns the ascending reading order of the entry's own sort type (`SOURCE`: source order descending index; `NUMBER`, `UPLOAD_DATE`, `ALPHABET` ascending with the collator) filtered by `!done` when `only_pending`
  - the eight example rows of `p1-topic-history-and-next-item.md` section 4 produce exactly the listed results, including `unknown from-id` skipping the first item when `only_pending = false`
  - manga applies the excluded-scanlator filter, anime has none; unknown entry id or repository error returns an empty list (logged)
  - `next_after_last_history(only_pending)` uses `HistoryRepo::latest()` and returns `[]` when there is no history
  - callers: library "download next N", reader/player download-ahead and history resume use this function only

### T-08a-023 — Total read duration for statistics
- Priority: P2
- Effort: S
- Depends: T-08a-018
- Sources: domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetTotalReadDuration.kt
- Acceptance:
  - `total_read_duration_ms()` = `COALESCE(SUM(time_read), 0)` over all manga history rows, including soft-reset rows and excluding hard-deleted ones
  - fixture rows `0, 5000, 7000` plus one reset row of 3000 gives 15000 (reset rows still count); after `clear_all` it is 0
  - the statistics screen (area:42) formats it; no anime equivalent is exposed (anime history has no duration)

## Updates

### T-08a-024 — Updates repository (feed query, window, limit, status filter)
- Priority: P1
- Effort: M
- Depends: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations, area:06-data-repos-history-track-source-misc, T-08a-032
- Sources: domain/src/main/java/tachiyomi/domain/updates/manga/repository/MangaUpdatesRepository.kt, domain/src/main/java/tachiyomi/domain/updates/anime/repository/AnimeUpdatesRepository.kt, domain/src/main/java/tachiyomi/domain/updates/manga/interactor/GetMangaUpdates.kt, domain/src/main/java/tachiyomi/domain/updates/anime/interactor/GetAnimeUpdates.kt, domain/src/main/java/tachiyomi/domain/updates/manga/model/MangaUpdatesWithRelations.kt, domain/src/main/java/tachiyomi/domain/updates/anime/model/AnimeUpdatesWithRelations.kt, data/src/main/sqldelight/view/updatesView.sq, data/src/main/sqldelightanime/view/animeupdatesView.sq
- Acceptance:
  - `recent(kind, after)` and `by_status(kind, done, after)` return only favourite entries' items with `date_fetch > date_added` and `date_upload > after`, ordered `date_fetch DESC, item id DESC`, capped at 500
  - the fixture of `p1-edge-cases-and-tests.md` 2.5 yields `[d, b]`, `[b]` for `done = false`, `[d]` for `done = true`; re-adding the entry with a later `date_added` hides older items
  - anime rows additionally carry `fillermark`, `last_second_seen`, `total_seconds`; manga rows carry `last_page_read`
  - excluded scanlators are not filtered in the feed (documented, tested)
  - the watchable variants re-emit after a chapter/episode insert or a read-flag change

### T-08a-025 — Updates tab state: date grouping, counters, last-updated, window helper
- Priority: P1
- Effort: M
- Depends: T-08a-024, T-08a-026
- Note: The Update library action needs the library update jobs of area:26-library-update-jobs, which are not yet authored in plans/.
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/UpdatesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt
- Acceptance:
  - `three_months_ago(now)` clamps the day of month (31 May 2025 gives 28 Feb 2025; 31 May 2024 gives 29 Feb 2024) and is computed once when the tab model is created
  - items are grouped by local date of `date_fetch` with the same header rule as history; the first row shows "Library last updated: {relative}" from `library_update_last_timestamp` ("Never" for 0, "Just now" below one minute, otherwise a minutes/hours/days phrase)
  - `reset_new_counts(kind)` sets `library_unread_updates_count` / `library_unseen_updates_count` to 0 when the Updates tab is shown and again when it is left; the navigation badge shows `anime + manga` when `library_show_updates_count` is on and is hidden at 0
  - the "Update library" action shows "Updating library" or "An update is already running" depending on whether area:26 started a job
  - selection helper reproduces the first/last-position range selection of `toggleSelection` (long-press between ends selects the span)
  - bookmark action touches only items whose flag differs; anime also supports fillermark

## Preferences and cross-cutting rules

### T-08a-026 — LibraryPrefs section: 87 accessors with exact keys, defaults and quirks
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system, T-08a-003, T-08a-004
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/di/PreferenceModule.kt
- Acceptance:
  - `LibraryPrefs` defines all 87 accessors of `p1-data-and-config.md` section 5 with the exact key strings, kinds and defaults; the three app-state keys are `library_update_last_timestamp`, `last_used_anime_category`, `last_used_category`
  - a registry test parses the table of section 5 and compares key, default, kind and namespace for every row
  - shared keys: `display_chapter_by_name_or_number` and `display_episode_by_name_or_number` use `default_chapter_display_by_name_or_number`; the two direction accessors use `default_chapter_sort_by_ascending_or_descending`
  - swipe accessors keep the Kotlin keys but are named by effect (`swipe_left_chapter` -> `pref_chapter_swipe_end_action` default ToggleRead; `swipe_right_chapter` -> `pref_chapter_swipe_start_action` default ToggleBookmark; episodes ToggleSeen / ToggleBookmark)
  - unknown enum strings (TriState, swipe actions, display mode, sort strings) fall back to the default without deleting the stored value
  - the constants `DEVICE_*`, `ENTRY_*`, `MARK_DUPLICATE_*` and the six category-preference keys are exported (`category_preference_keys()` returns the six library keys)

### T-08a-027 — Default chapter/episode/season flag defaults ("Set as default", apply to existing)
- Priority: P1
- Effort: M
- Depends: T-08a-026, area:07-domain-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, domain/src/main/java/tachiyomi/domain/items/chapter/interactor/SetMangaDefaultChapterFlags.kt, domain/src/main/java/tachiyomi/domain/items/episode/interactor/SetAnimeDefaultEpisodeFlags.kt, domain/src/main/java/tachiyomi/domain/items/season/interactor/SetAnimeDefaultSeasonFlags.kt
- Acceptance:
  - `set_chapter_settings_default(manga)` copies unread filter, downloaded filter, bookmarked filter, sorting, display mode and direction (`CHAPTER_SORT_DESC`/`ASC` from `sortDescending()`); the episode variant additionally copies fillermarked filter, show-previews and show-summaries raw bits; the season variant copies the 16 components listed in `p1-data-and-config.md` section 3.3
  - regression for Q3: the season downloaded filter default is taken from the **downloaded** bits, not the unseen bits
  - `apply_defaults_to_entry` writes the default prefs into an entry's flags (used when adding to the library) and `apply_defaults_to_all_favourites` does so for every favourite, non-cancellable
  - the "Updated default chapter settings" confirmation is surfaced by the caller
  - `SeasonDisplayMode` converts 0..3 to CompactGrid/ComfortableGrid/CoverOnlyGrid/List and any other value to CompactGrid (Q11 fix)

### T-08a-028 — Library update restrictions and global update candidate selection
- Priority: P1
- Effort: M
- Depends: T-08a-026, T-08a-005
- Note: Update candidate selection feeds area:26-library-update-jobs, which is not yet authored in plans/.
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/ui/stats/manga/MangaStatsScreenModel.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt
- Acceptance:
  - `update_candidates(rows, prefs, only_category)` implements include/exclude semantics of behaviour 1.11 (single-category refresh ignores both lists; excluded wins; distinct by entry id; empty include set means all)
  - `skip_reason(entry, restrictions, fetch_window_upper)` returns the five reasons with the exact English texts "Skipped because series does not require updates", "Skipped because series is complete", "Skipped because there are unread chapters", "Skipped because no chapters are read", "Skipped because no release was expected today" in the order: update strategy, completed, has unread, not started, outside release period
  - the same function counts "global update items" for the statistics screen (entries not skipped)
  - settings options: interval choices 0/12/24/48/72/168 hours with labels Off, Every 12 hours, Daily, Every 2 days, Every 3 days, Weekly; device restrictions Only on Wi-Fi, Only on unmetered network, When charging with defaults `{wifi}`; the three device checks are mapped on Linux to NetworkManager connectivity/metered state and UPower AC state through `kuro-platform` (area:26 consumes them)
  - the include/exclude dialog text for manga/anime matches the two `pref_*_library_update_categories_details` strings

### T-08a-029 — Swipe action enums and duplicate-marking constants
- Priority: P2
- Effort: S
- Depends: T-08a-026
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaChapterListItem.kt
- Acceptance:
  - `ChapterSwipeAction {ToggleRead, ToggleBookmark, Download, Disabled}` and `EpisodeSwipeAction {ToggleSeen, ToggleBookmark, ToggleFillermark, Download, Disabled}` are stored by name with the defaults of T-08a-026
  - the settings list entries and labels are "Disabled", "Bookmark chapter", "Mark as read", "Download" (episodes: "Bookmark episode", "Fillermark episode", "Mark as watched")
  - the multi-select preferences hold `existing`/`new` (chapters) and `existing_episode`/`new_episode` (episodes) with labels "After reading a chapter", "After fetching new chapter", "After watching an episode", "After fetching new episode"
  - on desktop the swipe actions are available as chapter-row context-menu items and as optional horizontal drag gestures on touch screens (threshold 56 px)

### T-08a-030 — Add-to-library default category resolution (shared)
- Priority: P0
- Effort: S
- Depends: T-08a-007, T-08a-011, T-08a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt
- Acceptance:
  - `resolve_add_target(kind)` returns `Categories([id])`, `Categories([])` (Default) or `Ask` per the table in `p1-edge-cases-and-tests.md` 2.3 (`(-1,[A,B])->Ask`, `(-1,[])->[]`, `(0,[A])->[]`, `(B,[A,B])->[B]`, `(999,[A,B])->Ask`)
  - the favourite flag is set first and categories second; a failed favourite update aborts before categories are touched
  - the settings list preference offers "Always ask" (-1) plus every category (Default shown as "Default") for manga ("Default manga category") and anime ("Default anime category")
  - library, browse, history, details and migration entry points all call this one function

### T-08a-031 — Active category persistence, random entry, continue button
- Priority: P1
- Effort: S
- Depends: T-08a-013, T-08a-022, T-08a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt, app/src/main/java/eu/kanade/presentation/library/manga/MangaLibraryContent.kt, app/src/main/java/eu/kanade/presentation/library/components/LibraryTabs.kt, app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterGetNextUnread.kt, app/src/main/java/eu/kanade/tachiyomi/util/episode/EpsiodeGetNextUnseen.kt
- Acceptance:
  - the selected page index is written to `last_used_category` / `last_used_anime_category` on every page change and clamped to `len-1` when the category list shrinks (0 when empty)
  - "Open random entry" picks uniformly from the visible items of the current category; none shows "No entries found in this category"
  - the continue button is shown only when `display_continue_reading_button` is on; it opens the first pending item in the entry's reading order (descending-sorted entries take the last pending item of the list), applying the entry's own chapter filters; none shows "Next chapter not found" / "Next Episode not found!"
  - "Update category" and "Update library" actions show "Updating category" / "Updating library" or "An update is already running"; refresh by F5/pull shows the busy indicator for 1 s only if started

### T-08a-032 — Change bus integration and reactive plumbing (watch channels)
- Priority: P0
- Effort: M
- Depends: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations, T-08a-001
- Sources: data/src/main/java/tachiyomi/data/category/manga/MangaCategoryRepositoryImpl.kt, data/src/main/java/tachiyomi/data/history/manga/MangaHistoryRepositoryImpl.kt, data/src/main/java/tachiyomi/data/updates/manga/MangaUpdatesRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetLibraryManga.kt
- Acceptance:
  - a generic helper `watch_query(db, tables, query_fn) -> watch::Receiver<Arc<T>>` re-runs the query when any listed table changes, coalescing events within 30 ms, and always publishes the initial value
  - table lists: categories `{categories}`, by-entry `{categories, mangas_categories|animes_categories}`, history list `{history|animehistory, chapters|episodes, mangas|animes}`, updates `{chapters|episodes, mangas|animes}`, library `{mangas|animes, chapters|episodes, history|animehistory, mangas_categories|animes_categories, excluded_scanlators}`
  - a query error is published as an error state and the watcher keeps running (next change retries); consumers that need every event use `mpsc` (documented pitfall 8)
  - a UI helper `bind_watch` runs the callback on the GTK main loop via `glib::spawn_future_local` and never holds the watch borrow across an await

## Integration, backup, polish, drops

### T-08a-033 — Wire library, categories and category dialogs into the mockup
- Priority: P0
- Effort: L
- Depends: T-08a-013, T-08a-014, T-08a-015, T-08a-016, T-08a-012, T-08a-008, T-08a-009, T-08a-010, T-08a-017, T-08a-030
- Note: Library UI area:37-ui-library is not yet authored in plans/; this task wires the existing mockup in the meantime.
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt, app/src/main/java/eu/kanade/presentation/library/manga/MangaLibrarySettingsDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/category/CategoriesTab.kt, app/src/main/java/eu/kanade/presentation/category/MangaCategoryScreen.kt, app/src/main/java/eu/kanade/presentation/category/components/CategoryListItem.kt, app/src/main/java/eu/kanade/presentation/category/components/CategoryDialogs.kt
- Acceptance:
  - `src/screens.rs` `libraries()`/`library()` no longer use `SAMPLE`/`title_n`: tabs come from the service pages, cards from real items with the unread (accent) and download (osd) badges, the continue button, selection by Ctrl/Shift click and long-press, the bottom action bar bound to bulk actions
  - the filter sheet `manga_library_settings`/`anime_library_settings` tri-state, sort (with the tracker row only when a tracker is logged in, Random with refresh icon, re-tap toggles direction), display chips, columns slider (0 = Auto, 0..10) and badge/tab switches read and write `LibraryPrefs` and the sort setter
  - `categories()` shows a drag-reorderable list from `CategoryService` with rename/hide/delete buttons, an Add button, the empty text "You have no categories. Tap the plus button to create one for organizing your library.", the rename/create dialogs with the duplicate-name error and the delete dialog "Do you wish to delete the category \"%s\"?"
  - every `.rs` page keeps using `spec/app.json` for titles and dialog texts; `cargo test` (the `spec_is_consistent` test) still passes
  - the app is usable end to end with a fixture database: create categories, move an entry between them, change sort per category, filter and search

### T-08a-034 — Wire history and updates tabs into the mockup
- Priority: P1
- Effort: M
- Depends: T-08a-020, T-08a-021, T-08a-025, T-08a-019
- Note: History and updates UI area:42-ui-updates-history-stats-storage-upcoming is not yet authored in plans/.
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt, app/src/main/java/eu/kanade/presentation/history/anime/AnimeHistoryScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesTab.kt, app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesUiItem.kt
- Acceptance:
  - `history()` and `updates()` in `src/screens.rs` list real rows with date headers, per-row actions and the dialogs `history_remove_anime`, `history_remove_manga`, `history_remove_everything`, `updates_delete_episodes`, `updates_delete_chapters`
  - selection mode in updates exposes select all / invert and the bottom actions; the Updates tab badge in the sidebar follows the counters
  - opening a history row or an update starts the reader/player of areas 43/45 with the entry and item ids; reading something creates a history row visible immediately in the History tab (end-to-end check)

### T-08a-035 — Category data in backups, preference id remapping, startup cleanup
- Priority: P1
- Effort: S
- Depends: T-08a-007, T-08a-026
- Note: Backup formats and restore areas area:21-backup-format-and-create and area:22-backup-restore are not yet authored in plans/.
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/MangaCategoriesBackupCreator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/MangaCategoriesRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeCategoriesRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt, app/src/main/java/mihon/core/migration/migrations/CategoryPreferencesCleanupMigration.kt
- Acceptance:
  - export lists non-system categories only with `name`, `order`, `flags` (hidden is not exported)
  - restore maps categories by exact name, appends new ones in backup order with consecutive `order` values starting at `max(existing order) + 1` (0 when the maximum does not exist), then sets `categorized_display = (distinct flags among all categories) > 1`
  - category-id preference sets of the six library and six download keys are remapped through backup id -> name -> new id (both kinds tried) and appended to the current sets
  - a startup cleanup removes ids from the 10 category-id preference sets (and resets the default category preferences) that exist in neither database (E19)

### T-08a-036 — Hidden categories consumers and the hide preference
- Priority: P2
- Effort: S
- Depends: T-08a-007, T-08a-026
- Note: Storage screen consumer belongs to area:42-ui-updates-history-stats-storage-upcoming, which is not yet authored in plans/.
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/category/manga/MangaCategoryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/storage/CommonStorageScreenModel.kt, app/src/main/java/eu/kanade/presentation/category/components/CategoryListItem.kt
- Acceptance:
  - the Categories screen always lists hidden categories (dimmed, closed-eye icon) so they can be unhidden; the preference `hidden_categories` ("Hide hidden categories from categories and storage screen") keeps its key and affects the Storage screen filter only
  - the Storage screen's "All" selection includes only entries from visible categories, and a specific category shows its own entries; items are distinct by entry id and sorted by size descending
  - hide toggle icon: eye when the category is hidden, crossed eye when visible, tooltip "Hide"

### T-08a-037 — Auto-clear item cache on cold start preference
- Priority: P2
- Effort: S
- Depends: T-08a-026
- Note: Cache clearing depends on the image pipeline area:34-images-covers-cache, which is not yet authored in plans/.
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - when `auto_clear_chapter_cache` is true, the page/chapter cache directory of area:34/43 is emptied once on application start (not on window re-open), asynchronously after the main window is shown
  - the preference defaults to false and its setting label stays "Clear chapter cache on app launch"
  - a failure to delete files is logged and ignored

### T-08a-038 — Drop: home-screen widgets fed by GetXUpdates (replaced by tray/desktop notification)
- Priority: P3
- Effort: S
- Depends: T-08a-024
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/AnimeWidgetManager.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/BaseAnimeUpdatesGridGlanceWidget.kt
- Acceptance:
  - decision recorded: Android Glance widgets (updates grid of up to rows x columns unread covers from the last 3 months, one per manga) are not ported; the user loses the launcher widget
  - replacement: the Updates tab and the library-update desktop notification of area:26 show the same information; `UpdatesRepo::by_status(kind, false, three_months_ago)` remains available for a future KDE Plasma widget or tray menu
  - no code path calls `GetXUpdates.subscribe(false, after)` for a widget

### T-08a-039 — Customized update frequency filter (interval-custom) gating
- Priority: P2
- Effort: S
- Depends: T-08a-014, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/presentation/library/manga/MangaLibrarySettingsDialog.kt, app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibrarySettingsDialog.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - the filter `pref_filter_library_interval_custom` ("Customized update frequency") predicate `fetch_interval < 0` is implemented and active whenever its state is not DISABLED and `manga_outside_release_period` is in the restrictions
  - the toggle row is hidden in release builds (upstream `!isReleaseBuildType` gate) and visible in debug/dev builds and when the environment variable `KUROBAYOMI_DEV=1` is set; documented in the settings help
  - a backup-restored non-DISABLED value still filters and the filter icon reflects it (Q21 rule)

### T-08a-040 — Test suite, fixtures and manual compatibility checklist
- Priority: P1
- Effort: M
- Depends: T-08a-002, T-08a-003, T-08a-015, T-08a-018, T-08a-024, T-08a-026
- Sources: domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt
- Acceptance:
  - all vectors of `p1-edge-cases-and-tests.md` section 2 exist as Rust tests; fixtures `library_small.sql`, `history_basic.sql`, `updates_window.sql` and `golden/sort_orders.json` are committed under `crates/kuro-domain/tests/`
  - property tests of section 5 run in CI with 256 cases each
  - `cargo test -p kuro-domain` is green headless (no display, no network)
  - a manual checklist (documented in the test README) compares, on a real Aniyomi backup supplied by the user, category names/order, decoded sort per category, hidden defaults and history ordering against the Android app's screens

### T-08a-041 — Library item decoration data (badges) and counters contract
- Priority: P1
- Effort: M
- Depends: T-08a-013, area:09-domain-source-track-download-release-misc, area:10-app-domain-layer
- Note: Download runtime areas area:23-downloads-manga and area:24-downloads-anime are not yet authored; the domain-side download index maps to area:09 and source lookup maps to area:10 (area:17 folds into area:10).
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryItem.kt, app/src/main/java/eu/kanade/presentation/library/components/LibraryBadges.kt, app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt
- Acceptance:
  - `DownloadIndex::count`, `SourceLookup::lang` and `is_local` are implemented as thin adapters over areas 23/24 and 17 and are only called when the matching badge preference is on
  - an invalidation tick from the download index re-runs the pipeline within 30 ms of a download completing or being deleted
  - badge rendering: pending-count badge hidden at 0, download badge hidden at 0, "local" badge for source id 0, language badge shows the language code; defaults off/on as in section 2.2 of `p1-behavior.md`
  - the item shape keeps defaults `download_count = -1`, `pending = -1`, `is_local = false`, `lang = ""` when badges are disabled so renderers can detect "not computed"

### T-08a-042 — Drop: Android-only domain plumbing (Injekt wiring, Serializable, logcat, Compose state)
- Priority: P3
- Effort: S
- Depends: T-08a-001
- Sources: app/src/main/java/eu/kanade/domain/DomainModule.kt, domain/src/main/java/tachiyomi/domain/category/model/Category.kt, domain/src/main/java/tachiyomi/domain/category/manga/interactor/DeleteMangaCategory.kt
- Acceptance:
  - decision recorded: `app/src/main/java/eu/kanade/domain/DomainModule.kt` factories (one `addFactory { ... }` line per interactor and per repository for both kinds), `java.io.Serializable` on `Category`, `logcat`, `Result` sealed classes with `InternalError(Throwable)` and `Flow` interactor facades are replaced by one `Domain` struct (services built in `kuro-core::AppContext`), plain structs, `tracing`, `Result<_, DomainError>` and `watch` receivers
  - the user loses nothing functionally; the interactor class names disappear from the Rust API in favour of service methods (mapping table in `p1-inventory.md` tasks column)
  - no `Injekt`-style global service locator exists in `kuro-domain`

### T-08a-043 — Columns and display adaptation for desktop (portrait/landscape keys)
- Priority: P2
- Effort: S
- Depends: T-08a-004, T-08a-026
- Note: Library UI area:37-ui-library is not yet authored in plans/; the column logic is independent of it.
- Sources: app/src/main/java/eu/kanade/presentation/library/manga/MangaLibraryPager.kt, app/src/main/java/eu/kanade/presentation/library/manga/MangaLibrarySettingsDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt
- Acceptance:
  - the four column preferences (`pref_library_columns_portrait_key`, `pref_library_columns_landscape_key`, `pref_animelib_columns_portrait_key`, `pref_animelib_columns_landscape_key`) keep default 0 (Auto) and range 0..10; the slider shows "Auto" at 0, label "Items per row" for grids and "List size" for List mode
  - Auto computes `max(2, floor(width / 160 px))` columns; windows narrower than 900 px use the portrait key and wider ones the landscape key
  - display modes render: List -> list rows, CompactGrid -> grid with title overlay, CoverOnlyGrid -> grid without title, ComfortableGrid -> grid with title below the cover; per-page display mode is the global preference

### T-08a-044 — Library settings sheet behaviours (sort click semantics, tabs, Random)
- Priority: P1
- Effort: S
- Depends: T-08a-012, T-08a-003
- Note: Settings sheet UI area:37-ui-library is not yet authored in plans/; the sort logic is independent of it.
- Sources: app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibrarySettingsDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibrarySettingsScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibrarySettingsScreenModel.kt
- Acceptance:
  - sort options appear in the order of behaviour 2.9 (manga 10 incl. tracker score only when a tracker is logged in; anime 11 with Airing time before Random)
  - clicking the current type flips the direction, clicking another type keeps the current direction, clicking Random sets Ascending and re-rolls the seed (refresh icon shown when Random is active)
  - the sheet has tabs Filter / Sort / Display and closes itself if the active category disappears
  - toggling a filter row calls `TriState::next()` on the preference; tracker rows use `toggle_tracker(id)`
