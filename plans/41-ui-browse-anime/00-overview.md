# Area 41: Anime browse, sources, global search, extensions tab, migration (overview)

Area lead document. Read this first, then the part documents in the reading guide. All Kotlin paths are repo-relative to `/home/blackbox/aniyomi-src` (Apache-2.0, read-only). Rust targets are under the `kurobayomi` crate, `src/` tree.

Scope split:

- **Part 1** (`p1-*`): extensions tab, extension details, source preferences, the presentation of the sources list, browse and global search screens, source filter presentation, and the migration flags, confirm dialog, engine, source picker, favorites, search, and seasons. 36 Kotlin files, 4972 LOC (per `p1-inventory.md` header).
- **Part 2** (`p2-*`): the sources list model, source visibility model, browse one source (paging, favorites, duplicates, categories, filters), the source filter sheet model, global search model, and the migration sources tab. 11 Kotlin files, 1617 LOC (per `p2-inventory.md` header).

Together: 47 Kotlin files, about 6589 LOC.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

Aniyomi's anime Browse tab is the way a user finds and adds anime. It has four surfaces:

1. **Sources**: the list of installed anime sources, grouped by "Last used", "Pinned", and one section per language. Each row opens that source's Popular or Latest listing.
2. **Browse one source**: a paged grid or list of the source's entries, with Popular, Latest, text search, source-defined filters, genre search, and add-to-library (favorite) with category selection and duplicate detection.
3. **Global search**: a fan-out search of every pinned (or every enabled) source at once, shown as one block per source.
4. **Extensions**: install, update, uninstall, trust, and language filter of anime extensions, plus an extension details page. Migration (moving a library entry from one source to another, including seasons) is started from a duplicate dialog or the migrate source picker.

### 1.2 Why it matters for a usable Kurobayomi

Without this area a user cannot get anime into the library, cannot see which sources are installed or enabled, and cannot move an entry to a better source. Every P0 flow for anime (library, details, player) depends on an entry being added from a source. The P0 tasks in this area are therefore the entry points to the anime half of the app, not polish.

### 1.3 What this area does not own

- Manga browse: area:40-ui-browse-manga. Shared components (`ExtensionHeader`, `ExtensionTrustDialog`, `NsfwWarningDialog`, `BaseBrowseItem`, `BrowseBadges`, `BrowseSourceLoadingItem`, `GlobalSearchResultItem`, `GlobalSearchErrorResultItem`) live there and are consumed here.
- Extension install engine and manager: area:19-extensions-anime.
- Source API and source manager: area:16-source-api-anime, area:17-source-app-layer.
- Anime entry details page: area:39-ui-entry-details-anime.

---

## 2. Module map

### 2.1 Kotlin packages (OBSERVED)

| Package (under `app/src/main/java/`) | Responsibility | Part |
|---|---|---|
| `eu/kanade/presentation/browse/anime/` | Compose screens: `AnimeExtensionsScreen.kt`, `AnimeExtensionDetailsScreen.kt`, `AnimeExtensionFilterScreen.kt`, `AnimeSourcesFilterScreen.kt`, and the sources, browse, and global search screens | p1 (presentation); p2 owns the flows behind the browse ones |
| `eu/kanade/presentation/browse/anime/components/` | `GlobalAnimeSearchCardRow.kt`, `GlobalAnimeSearchToolbar.kt`, `BrowseAnimeSourceToolbar.kt`, display-mode and migrate dialogs | p1 |
| `eu/kanade/tachiyomi/ui/browse/anime/extension/` and `.../extension/details/` | `AnimeExtensionsTab.kt`, `AnimeExtensionsScreenModel.kt`, filter model, details model | p1 |
| `eu/kanade/tachiyomi/ui/browse/anime/source/` | `AnimeSourcesTab.kt`, `AnimeSourcesScreenModel.kt`, `AnimeSourcesFilterScreenModel.kt`, `AnimeSourcesFilterScreen.kt` (Voyager wrapper) | p2 |
| `eu/kanade/tachiyomi/ui/browse/anime/source/browse/` | `BrowseAnimeSourceScreen.kt`, `BrowseAnimeSourceScreenModel.kt` (listing, paging, favorites, duplicates, categories) | p2 |
| `eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/` | `AnimeSearchScreenModel.kt` (fan-out), `GlobalAnimeSearchScreen.kt`, `GlobalAnimeSearchScreenModel.kt` | p2 |
| `eu/kanade/tachiyomi/ui/browse/anime/migration/` | `anime/` (`MigrateAnimeScreen`, `MigrateAnimeDialog`, season picker), `search/` (migrate search and per-source search), `sources/` (source picker tab) | p1 (dialog, engine, favorites, search, seasons) and p2 (`sources/`) |
| `eu/kanade/domain/source/service/SourcePreferences.kt`, `domain/.../library/service/LibraryPreferences.kt`, `eu/kanade/domain/base/BasePreferences.kt` | Preference keys read here | p1 and p2 |
| `domain/src/main/java/tachiyomi/domain/source/anime/model/` (`AnimeSource.kt`, `StubAnimeSource.kt`, `Pin.kt`) | Domain source model, stub source, pin bits | p2 |
| `domain/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeExtension.kt` | Extension domain model (area:19) | p1 consumer |
| `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt` | Filter tree the source defines (area:16) | p2 consumer |

### 2.2 Rust module map (PROPOSAL, reconciles p1-rust-design.md section 2 and p2-rust-design.md section 2)

The two part documents each propose a tree and they do not agree (see section 4, decision D-A). This overview proposes one tree. Owners of each file are the task IDs in the part task files.

```
src/browse/anime/
  mod.rs              re-exports; page constructors used by src/entry.rs
  model.rs            pure shared types: Pin, LibraryDisplayMode, FetchType, Listing, SourceRef,
                      InstallStep. No GTK imports.
  prefs.rs            typed accessors over area:02 preference keys (p1-data-and-config.md section 2,
                      p2-data-and-config.md section 3). One accessor module only.
  ext/                (part 1) types.rs, query.rs, group.rs, model.rs, filter_model.rs,
                      details_model.rs, icons.rs
  sprefs.rs           (part 1) source preferences page, native widgets
  sources.rs          (part 2) Sources tab: SourcesModel + view
  source_filter.rs    (part 2) Visibility page: model + view
  source_browse.rs    (part 2) Browse page: model + view (grid, list, toolbar, chips, dialogs)
  pager.rs            (part 2) SourcePager; replaces androidx.paging Pager
  library_add.rs      (part 2) favorite toggle, duplicate lookup, category resolution, picker model
  source_filters.rs   (part 2) filter sheet view and FilterList mutation rules
  global_search.rs    (part 2) fan-out model + view
  widgets.rs          (part 2) BaseAnimeSourceItem, AnimeSourceIcon, InLibraryBadge,
                      BrowseSourceLoadingItem, comfortable grid item
  migration/
    mod.rs
    flags.rs          MigrateFlags bitmap (p1-data-and-config.md section 5.1)
    engine.rs         migrate_anime(), ordered steps (p1-behavior.md section 12)
    dialog.rs         confirm dialog (p1-behavior.md section 11)
    favorites.rs      favorites of one source (p1 behavior 14)
    search.rs         migrate search and per-source search (p1 behavior 15, 16)
    seasons.rs        season picker (p1 behavior 17)
    source_picker.rs  source picker: model and view, sort persisted (p1 behavior 13 and p2 behavior 8)
src/widgets/
  banner.rs           in-window banner (replaces Android Toast and Snackbar)
  cover_grid.rs       shared GridView factory (also used by area:37 and area:40)
```

Crates owned by other areas and consumed here (NOT created by this area): `src/domain/anime_source.rs` and `src/domain/anime_filter.rs` (area:16), `src/data/anime_repo.rs` (area:05, area:07), `src/data/anime_source_repo.rs` (area:17), `src/settings/` (area:02).

Crates this area adds (from p1-rust-design.md section 9 and p2-rust-design.md section 5): `async-channel` (tokio to GTK hop), `zip` (read the icon from an extension bundle), `image` with `png`, `jpeg`, `webp` (decode icons), `thiserror`, `icu_displaynames` (locale display names), `futures` (stream), and optionally `async-trait` (or hand-written `Pin<Box<dyn Future>>`) and `icu_collator` (only if sort order fails in testing, see Q-3). `gtk4` 0.11 with feature `v4_16` is already present. No new crate for clipboard (`gdk::Clipboard`), URL opening (`gtk::UriLauncher`), dialogs (`gtk::AlertDialog`), or the snackbar (a `gtk::Revealer`). Version numbers are pinned at implementation time from crates.io, not in this plan.

### 2.3 Entry points in the existing mockup (OBSERVED from `src/entry.rs`)

| Function | Line | Current state | Replaced by |
|---|---|---|---|
| `browse()` | 244 | Scaffold; calls the functions below | Keep the scaffold; call `browse::anime::*` |
| `sources(anime: bool)` | 283 | Hard-coded sections `Last used`, `Pinned`, `English`, `Other` | `sources.rs` view |
| `migrate_sources(anime: bool)` | 360 | Placeholder | `migration/source_picker.rs` |
| `browse_source(id, migrating)` | 380 | Cover-grid placeholder | `source_browse.rs` |
| `global_search(id, migrating)` | 432 | Placeholder sections | `global_search.rs` |

`src/ui.rs` (`push` line 34, `pop` line 40, `open(id)` line 88, `show_sheet` line 391, `show_dialog` line 406) stays the navigation mechanism. The spec JSON in `spec/` is kept only for static dialog copy.

---

## 3. Reading guide

Read in this order: this overview, then `p1-behavior.md` and `p2-behavior.md` (what the Kotlin does), then the `*-rust-design.md` files, then tasks.

| Document | What it contains |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Table of the 36 part-1 Kotlin files with LOC, declared names, port decision, Rust target, priority, and task IDs |
| [p1-behavior.md](p1-behavior.md) | Observed behavior of the extensions tab, extension details, source preferences, sources list, browse and global search UI, source filter UI, and all migration screens; sections 26 holds the Android drops table (D1 to D18) |
| [p1-data-and-config.md](p1-data-and-config.md) | Preference keys, domain model fields, enums, migration bit layout, network endpoints, DB touchpoints, cover sizes, pins, and fixtures |
| [p1-rust-design.md](p1-rust-design.md) | Part-1 module tree, core types, pure logic signatures, migration engine design, views, Android-to-Linux mapping, crates, responsive layout |
| [p1-ui-parity.md](p1-ui-parity.md) | Screen-by-screen parity spec (S1 to S13, D1 to D6), snackbars, gestures, keyboard map, responsive rules, gap list against the mockup |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases E1 to E21 and beyond, failure modes, race conditions, test plan, fixtures, manual test script |
| [p1-topic-worked-examples.md](p1-topic-worked-examples.md) | Test vectors A to H: grouping, migration flags, episode transfer, source picker sort, global search visibility, install step map, migrate outcome matrix, query parsing |
| [p1-tasks.md](p1-tasks.md) | 51 implementation tasks, IDs T-41a-001 to T-41a-053 (with gaps in numbering) |
| [p2-inventory.md](p2-inventory.md) | Table of the 11 part-2 Kotlin files with LOC, declared names, port decision, Rust target, and task IDs |
| [p2-behavior.md](p2-behavior.md) | Features A to F: sources list, visibility page, browse one source, filter sheet, global search, migration sources tab; shared helpers, concurrency, constants, drops table (section 12) |
| [p2-data-and-config.md](p2-data-and-config.md) | Part-2 domain models, pin bits, stub source, `AnimeUpdate`, filter model, browse and search state, preference keys, DB queries, user-visible strings, constants |
| [p2-rust-design.md](p2-rust-design.md) | Decisions D-1 to D-8, part-2 module tree, pure model types, pager, global search fan-out, threading, crates, dialogs, error policy, open questions |
| [p2-ui-parity.md](p2-ui-parity.md) | Parity spec for the sources tab, visibility page, browse page, filter sheet, global search, and migration sources tab; keyboard and pointer map; gap list |
| [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md) | Upstream tests, edge cases per feature, race conditions, fixtures, new test list (T-41b-047 to T-41b-054), manual GTK checklist, open decisions that change tests |
| [p2-topic-sources-list.md](p2-topic-sources-list.md) | Deep dive: six-step sources pipeline, pins, language order (D-1), visibility page order, header text table, failure path |
| [p2-topic-browse-listing.md](p2-topic-browse-listing.md) | Deep dive: listing state machine, pager rules, network-to-library mapping, navigate-up rule, favorite decision tree, category write order, hide-in-library |
| [p2-topic-global-search.md](p2-topic-global-search.md) | Deep dive: source selection, fan-out algorithm, ordering, progress, chips, single-result auto-open, failure text, result rows |
| [p2-topic-source-filters.md](p2-topic-source-filters.md) | Deep dive: filter model, rendering table, pure mutation functions, genre search contract, sheet state, equality, test vectors |
| [p1-tasks.md](p1-tasks.md) and [p2-tasks.md](p2-tasks.md) | Task files (both lint ok with `tasks.py lint`) |

Note: `p1-topic-worked-examples.md` is the only test-vector file for part 1; part 2 test vectors are inside the `p2-topic-*` deep dives.

---

## 4. Key design decisions and open questions

### 4.1 Architecture decisions (PROPOSAL, adopted by both part documents unless noted)

| ID | Decision | Recommendation and reason |
|---|---|---|
| A-1 | Pure logic (query, grouping, sort, pin bits, migration flags, migrate engine, mutation rules) has no GTK types | Adopt. Makes the important logic unit-testable without a display. |
| A-2 | One owner per page state; GTK widgets render a snapshot | Adopt (p1-rust-design.md section 1, principle 2). |
| A-3 | Network, DB and filesystem run on tokio (area:01); results come to GTK through `async-channel` | Adopt. Never block the GTK main loop. |
| A-4 | Typed page builders replace the mockup's string-id navigation (`ui::open("browse_source")`) | Adopt. Spec JSON stays for static text. |
| A-5 | Global search uses one process-wide semaphore of 5 permits (decision D-6), not a thread pool per screen | Adopt. Kotlin creates `Executors.newFixedThreadPool(5)` per model (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:46`) and never shuts it down. |
| A-6 | Android-only mechanics are dropped or replaced with a named Linux API (table in section 6) | Adopt. |
| A-7 | Snackbar and toast become one in-window banner (`gtk::Revealer`, 4 s) | Adopt. |

### 4.2 Behavior changes from Kotlin (each needs the user's sign-off; default in the table is the recommendation)

| ID | Change | Kotlin behavior it replaces | Recommendation |
|---|---|---|---|
| D-1 | Language sections in the Sources list use display-name order with "all" first, same rule as the Visibility page | List uses raw code order; Visibility uses display order (p2-topic-sources-list.md section 4) | Adopt. Only non-English locales see a change. |
| D-2 | Empty first page shows the "No results found" empty state with Retry, Open in WebView, Help (no error snackbar) | Snackbar plus retry | Adopt. Same text and actions. |
| D-3 | Favorite toggle writes only `favorite`, `date_added`, `cover_last_modified`, `background_last_modified` | Rewrites every column from a possibly stale snapshot (E-BR-02) | Adopt. Fixes a data-loss race. |
| D-4 | `TriState` unknown integer maps to `Ignore` with a logged warning; Sort with no selection is index 0 ascending | Crash (E-FD-01, E-FD-03) | Adopt. |
| D-5 | Results from a superseded global search query are dropped (generation guard) | Stale writes possible (E-GS-03) | Adopt. |
| D-6 | One shared semaphore of 5 permits | Thread pool per screen (E-GS-08) | Adopt. No visible change. |
| D-7 | Listing equality is by value (`query` plus filter snapshot) | Reload on unrelated state change (E-BR-06) | Adopt. |
| D-8 | Non-favorite title refresh on browse | Kotlin keeps DB title | Keep Kotlin behavior (see Q-1). |

### 4.3 Open questions for the user

| ID | Question | Recommendation (default used if no answer) |
|---|---|---|
| Q1 (p1-behavior.md section 25) | Share the display-mode catalogue (`pref_display_mode_catalogue`) between manga and anime browse? | Yes (parity). |
| Q2 (p1-behavior.md section 25) | Keep "Delete downloaded" pre-checked in the migrate dialog? | Parity default is pre-checked. Recommendation: default OFF, because it deletes data. |
| Q-1 (p2-rust-design.md section 12) | Refresh the displayed title of non-favorite rows when the source title changes? | Keep Kotlin behavior. Revisit with area:05. |
| Q-2 | Show the browse "Filter" chip only when filters are non-default? | Keep Kotlin: show whenever the source defines any filter. |
| Q-3 | Need ICU collation for the migration tab, or is Unicode-lowercase byte order enough? | Byte order. Add `icu_collator` only if non-Latin names sort badly in testing. |
| Q-4 | Global search default mode `PinnedOnly`? | Keep. |
| Q-5 | Filter sheet: apply edits to the shared list immediately or only on Filter? | Keep immediate (Kotlin). |

### 4.4 Plan conflicts found while writing this overview (resolved here; the lead must confirm)

| ID | Conflict | Resolution in this overview |
|---|---|---|
| D-A | Part 1 proposes `browse/anime/ext/`, `source/`, `global/`, `migration/` subfolders. Part 2 proposes flat `sources.rs`, `source_browse.rs`, `global_search.rs`, `source_filter.rs` and the others listed in its section 2. Both claim `browse/anime/model.rs` with different contents. | Use the tree in section 2.2. `model.rs` holds shared enums and listing types only. |
| D-B | Migration source picker is in two places: T-41a-036 (part 1) and T-41b-038 to T-41b-040 (part 2, including sort in T-41b-039). Both describe the sort. | T-41b-039 owns the sort and persistence. T-41a-036 owns the view only. Recorded in the task file dependencies. |
| D-C | Visibility page: T-41a-031 (view) and T-41b-008, T-41b-009 (model and wiring). | Split by layer: T-41a-031 depends on T-41b-008, so the model is first. |
| D-D | Global search: part 1 (T-41a-027 to T-41a-030) and part 2 (T-41b-032 to T-41b-037). | Part 2 owns the fan-out model. Part 1 renders it. T-41a-027 now depends on T-41b-033. |
| D-E | Dependency cycles in part 1 (T-41a-005 and T-41a-007; T-41a-020, T-41a-022, T-41a-024; T-41a-027 and T-41a-030; T-41a-035 and T-41a-041; T-41a-040 and T-41a-050; T-41a-023 and T-41a-046) | Fixed in `p1-tasks.md`. Each cycle had one edge removed: the component-to-page edge, or the drop-task-to-implementation edge. No task IDs changed. Re-check with the lint command and a reachability check. |
| D-F | Part 1 cites area:41 (its own area) as a dependency in five tasks (T-41a-027, T-41a-031, T-41a-036, T-41a-038, T-41a-039) | Replaced with the specific part-2 task IDs: T-41b-033 for 027, 038 and 039; T-41b-008 for 031; T-41b-039 for 036. |
| D-G | Shared manga components used in T-41a-012, T-41a-020, T-41a-030 had no dependency on area:40-ui-browse-manga | Added `area:40-ui-browse-manga` to those three tasks. |
| D-H | Some citations use short paths (for example `components/GlobalAnimeSearchCardRow.kt` in `p1-inventory.md`, and `presentation/browse/anime/AnimeSourcesFilterScreen.kt` in `p1-inventory.md`) | Not fixed here. The rule is full repo-relative paths. The part-1 author should expand them. |

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others (dependency tokens in area:41 tasks, counted from TASKS.csv)

| Area | Tokens | What it must provide |
|---|---|---|
| area:02-preferences-system (14) | `SourcePreferences`, `LibraryPreferences`, `BasePreferences` keys; typed store for `source_languages`, `hidden_anime_catalogues`, `pinned_anime_catalogues`, sort keys, display modes, migrate flags | Typed preference store, `TriState`, `Pin` bit storage |
| area:19-extensions-anime (12) | `AnimeExtensionManager` flows (installed, available, untrusted), install, update, cancel, uninstall, trust, `InstallStep`, `AnimeExtension` model | Extension engine and states |
| area:17-source-app-layer (8) | `AnimeSourceManager.get`, `getOrStub`, `getAll`; `StubAnimeSource`; `AnimeSource.visualName` | Source registry and stub source |
| area:16-source-api-anime (8) | `AnimeSource` interface, `AnimeHttpSource.getHomeUrl()`, `getSearchAnime`, `getFilterList`, `FetchType`, `AnimesPage` | Source API |
| area:36-ui-shell-navigation-security-webview (8) | Navigator push, pop, replace; `WebViewScreen(url, title, sourceId)`; Browse tab host | Navigation and WebView |
| area:34-images-covers-cache (5) | Anime cover cache and background cache; icon and cover sizes | Covers and icons on disk |
| area:39-ui-entry-details-anime (3) | `AnimeScreen(animeId, fromSource)` | Destination page for taps |
| area:24-downloads-anime (3) | `deleteAnime` on migrate; download count per anime | Download side effects |
| area:08-domain-library-category-history-updates (3) | `GetAnimeCategories`, `SetAnimeCategories` | Categories on favorite |
| area:05, area:07 (2 each) | `GetAnime`, `NetworkToLocalAnime`, `GetAnimeFavorites`, `UpdateAnime`, `GetEpisodesByAnimeId`, `UpdateEpisode`, `Anime`, `Episode` | Entry repos and domain |
| area:27-tracking-core (2) | `EnhancedAnimeTracker`, `TrackerManager`, `InsertAnimeTrack`, `GetAnimeTracks` | Tracks copied on migrate |
| area:37-ui-library (2) | Grid and display-mode components; library empty-state entry to global search | Shared grid |
| area:13-core-util-storage-archive-torrent (2) | `LocaleHelper` display names; language helpers | Locale names |
| area:12-network-stack (2) | `NetworkHelper.cookieJar` for clear cookies | Cookie jar |
| area:01, area:04, area:09, area:10 (1 each) | Runtime and banner primitive; anime schema; pin and source misc; `UpdateAnimeFromRemote` | Runtime, schema, misc |
| area:40-ui-browse-manga (added, D-G) | `ExtensionHeader`, `ExtensionTrustDialog`, `NsfwWarningDialog`, `BaseBrowseItem`, `InLibraryBadge`, `BrowseSourceLoadingItem`, global search result items | Shared browse components |

### 5.2 What others need from this area

| Consumer | What it uses | Source |
|---|---|---|
| area:36 (shell, Browse tab) | `browse::anime` page constructors; Browse tab re-select opens global search | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/BrowseTab.kt:52-54` |
| area:36 (deep links) | Global search with query and extension filter (`INTENT_ANIMESEARCH`), deep-link fallback to global search | `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:514-520` |
| area:39 (entry details) | Text search and genre search back into browse (`performSearch`, `performGenreSearch`); global search from related titles | `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt:227, 478` |
| area:37 (library) | Empty-state global search with the library query | `app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt:221` |
| area:40 (manga browse) | Shared grid factory and display-mode catalogue (Q1) | `widgets/cover_grid.rs` (T-41a-006) |
| area:06 | Browse list and pager wired into the mockup (T-06a-081) | T-41b-012, T-41a-005 |
| area:42 | Migrate dialog opened from history (T-42a-020) | `migration/dialog.rs` |
| area:34 | Custom cover and background copied on migrate (T-34a-051) | `migration/engine.rs` |
| area:35 | Global search hint and search provider replaced (T-35a-029) | T-41a-027 |
| area:36 | Home tab host (T-36a-004) | Page constructors in `browse/anime/mod.rs` |
| area:37 | Global search row in library (T-37a-022) | `global_search.rs` |
| area:39 | Search and genre-search routing (T-39b-050) | `source_browse.rs` |
| area:47 | Hide-in-library filter and 18+ source gate on browse lists (T-47a-060, T-47a-061) | T-41b-014 |

---

## 6. Task summary

Source of counts: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then rows whose file starts with `41-ui-browse-anime/` in `TASKS.csv`. Linted with `tasks.py lint`: both task files print `ok`. Area dependency tokens were checked against real area folders.

### 6.1 Counts (110 tasks: 51 in part 1, 59 in part 2)

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 29 | 14 | 1 | 0 | 44 |
| P1 | 33 | 17 | 3 | 0 | 53 |
| P2 | 5 | 0 | 0 | 0 | 5 |
| P3 | 8 | 0 | 0 | 0 | 8 |
| **Total** | **75** | **31** | **4** | **0** | **110** |

Effort is about 75 S (up to 1 day each), 31 M (up to 3 days), 4 L (up to 1.5 weeks). No XL. Upper bound at the effort ceilings: 75 x 1 day + 31 x 3 days + 4 x 7.5 days = 198 engineer-days (about 9 engineer-months), excluding area:19 and area:16 work. The effort ceilings are the task-format definitions, so this is a bound, not an estimate.

### 6.2 P0 critical path (in order)

Four P0 chains run in parallel. The browse chain is the longest.

**Chain 1 (browse one source, longest):**
1. T-41a-008 Stub source "not installed" page (S)
2. T-41b-010 Browse gating and missing-source page (S)
3. T-41b-011 Listing model and navigation sentinels (S)
4. T-41b-012 SourcePager (M)
5. T-41b-013 Network-to-library upsert (M)
6. T-41b-020 `change_favorite` side effects and narrow write (M)
7. T-41b-053 Integration tests: favorites, duplicates, categories (M). Also needs T-41b-022 (P1, category resolution).

**Chain 2 (browse page UI):**
1. T-41a-004 Source and extension icon loading (M)
2. T-41a-006 Browse grids and list (M)
3. T-41a-007 Browse toolbar (S)
4. T-41a-005 Browse-source page state machine and typed navigation (M)
5. T-41b-018 Body states and append-error snackbar wiring (M)

**Chain 3 (extensions tab):**
1. T-41a-017 Language display names, comparator, default languages (S)
2. T-41a-016 Extension list model state and preference wiring (S)
3. T-41a-009 Extension grouping, query filter, debounce model (L)
4. T-41a-015 Install, update, cancel and retry lifecycle (M)
5. T-41a-010 Extensions list UI (M)
6. T-41a-011 Extensions tab wiring (S)
7. T-41a-049 Unit and integration tests for the part (M)

**Chain 4 (sources list):**
1. T-41a-042 Source enable, hidden, pinned id sets (S)
2. T-41a-043 Pin bits and pinned ordering (S)
3. T-41b-001 Enabled-source selection model (S)
4. T-41b-002 Section grouping and order (S)
5. T-41b-003 Sources tab model state (S)
6. T-41a-001 Sources list screen (M)
7. T-41a-002 Source options popover (S)
8. T-41b-004 and T-41b-005 failure path and header actions (S each)

T-41a-045 (banner primitive) is a P0 leaf that many of these need. Start it on day 1.

### 6.3 Priority reasoning

- P0 (44): every page needed to add an anime entry from a working source and to see installed sources. This matches the P0 definition: library, details, reader, player, progress, DB, essential settings, source mechanism. Here the scope is "browse and add", so the P0 set covers sources list, browse one source, favorites, extensions list and install, and the pure models under them.
- P1 (53): global search, filter sheet, visibility page, trust flow, update all, migration (all screens and engine), language filter, details page.
- P2 (5): clipboard actions (T-41a-047), help and local-source guide (T-41a-048), repo link (T-41a-022), single-result auto-open (T-41b-037), narrow-window rules (T-41b-046).
- P3 (8): replacement or drop decisions (T-41a-024, T-41a-025, T-41a-046, T-41b-055 to T-41b-059). They state the replacement or drop; they are not implementation.

### 6.4 Risks

1. **Upstream blocking.** Chains 1 to 4 need area:16, area:17, area:19 and area:02 to deliver first. Their P0 tasks are the real critical path. Mitigation: build the pure models (T-41b-001, T-41b-011, T-41a-009) against fixtures first (T-41b-047) so UI work is not blocked.
2. **Layout conflict** (D-A). Two module trees will produce two sets of files. Mitigation: confirm the section 2.2 tree before any code lands.
3. **Duplicate ownership** (D-B, D-C, D-D). Two engineers may build the same sort or the same search. Mitigation: owners in section 4.4.
4. **Behavior changes** (D-1 to D-7). Changing list order and empty-state flow is visible. Mitigation: get sign-off before implementing.
5. **Destructive default in migrate** (Q2). "Delete downloaded" pre-checked deletes data on migrate. Mitigation: default OFF, confirm.
6. **Extension trust.** Installing an untrusted extension runs third-party code. The trust dialog (T-41a-012) must gate every install of an untrusted extension. Mitigation: trust cases in the part-1 test plan (`p1-edge-cases-and-tests.md` section 4).
7. **Global search load.** Five permits across all sources can hit rate limits or slow sources. Mitigation: per-source error rows with the failure text in `p2-topic-global-search.md` section 8.
8. **Migration atomicity.** Engine steps touch favorites, categories, episodes, tracks and downloads. A failed step must not leave a half-migrated entry. Mitigation: T-41a-034 uses one transaction, tested by T-41a-049.
9. **Doc quality.** Short citations (D-H) break the full-path rule and make review slower.
10. **Mockup drift.** The mockup's `sources()` and `browse_source()` functions are placeholders with hard-coded sections (section 2.3). Mitigation: replace them in the P0 chains, not by patching.

---

## 7. Definition of done for this area (observable in Kurobayomi)

Each item is checked on the GTK build against a real installed anime source.

1. **Sources tab.** Shows sections in this order: "Last used" (if any), "Pinned" (if any), then one section per language in display-name order with "all" first (D-1). Clicking a row opens that source's Popular listing. The "Latest" button opens Latest. Long press (or right click) opens the options popover with pin, unpin, and disable.
2. **Source failure.** A source that cannot load shows the failure path (T-41b-004) instead of an empty list. Nothing crashes.
3. **Browse one source.** Popular, Latest, text search, and the Filter chip work. Grid, comfortable and compact modes, cover-only, and list are switchable and the choice is stored in preferences (T-41a-006, T-41b-017). Scrolling loads the next page. An append error shows a snackbar; a first-page error shows the empty state with Retry, Open in WebView, and Help (D-2).
4. **Favorites.** Adding an entry writes only the four columns of D-3. Adding uses the default category when `default_anime_category` names a user category, and otherwise opens the category picker, preselected from the entry's current categories (T-41b-022; `p2-topic-browse-listing.md` section 5.1). A duplicate of an existing library entry shows the duplicate dialog with Show anime and Migrate existing entry. Removing a favorite asks for confirmation (T-41b-019).
5. **Global search.** Searches pinned sources by default (Q-4). At most 5 source searches run at once across the whole app (D-6). Results from an older query never replace newer ones (D-5). A single result auto-opens its entry (T-41b-037).
6. **Visibility page.** Language switches hide or show their sources; disabling a language does not change the checkbox state of its sources (T-41a-031).
7. **Extensions tab.** Groups: pending updates with Update all, installed-untrusted, then per language. Install, update, cancel, retry and uninstall work and show the install step text. Untrusted extensions require the trust dialog before they run. Refresh works from the button and F5.
8. **Extension details.** Shows version, language, and 18+ info. Enable all, disable all, clear cookies, per-source switches, and the incognito switch work.
9. **Migration.** The confirm dialog shows the flags of `p1-data-and-config.md` section 5.1 with the defaults of Q2. The source picker sorts and remembers its sort (T-41b-039). Favorites of one source can be migrated one by one. Migration runs as one transaction: if one step fails, the old entry is unchanged.
10. **Keyboard.** Every action in section 7 of `p2-ui-parity.md` and section 21 of `p1-ui-parity.md` works from the keyboard.
11. **No Android-only mechanics.** No Toast, no Intent, no haptics, no permission banner. Every item in the drops table (`p1-behavior.md` section 26, `p2-behavior.md` section 12) is either replaced by the stated Linux API or dropped as written.
12. **Tests.** The part-1 test plan (`p1-edge-cases-and-tests.md` section 4) and the part-2 test list (`p2-edge-cases-and-tests.md` section 9, T-41b-047 to T-41b-054) pass.

---

## 8. Lint and fix log for this overview

- `tasks.py lint` on `p1-tasks.md` and `p2-tasks.md`: ok after the edits in section 4.4.
- Task IDs unchanged. Only `- Depends:` lines changed in `p1-tasks.md`.
- Changes: five self-area tokens replaced (D-F); cycle edges removed (D-E); `area:40-ui-browse-manga` added to three tasks (D-G); empty depends set to `none` on four tasks.
- Global graph check: no dependency cycle inside area:41 after the edits. Cycles in other areas (for example T-14a-014, T-14a-035, T-14a-036) were not touched.
