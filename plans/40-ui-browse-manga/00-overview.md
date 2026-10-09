# Area 40: Manga browse (sources, browse, global search, filters, extensions screen, migration screens)

Area lead overview. Read this first, then the docs in section 3.

All repo paths are relative to `/home/blackbox/aniyomi-src` (read-only Aniyomi clone, Apache-2.0). Plan paths are relative to `/home/blackbox/kurobayomi/plans/`.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does in this area

The Browse tab is the manga entry point for finding content. It has these user-visible parts:

- **Browse tab host**: six sub-pages behind one tab, with a tab-switch channel and a hoisted extension search (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/BrowseTab.kt`).
- **Sources page**: installed manga sources grouped by language, with pin, Latest, options dialog (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt`).
- **Show/hide sources**: language switches and per-source checkboxes (`.../ui/browse/manga/source/MangaSourcesFilterScreenModel.kt`).
- **Browse one source**: paged remote listing (Popular, Latest, search, genre search), filter tree, display modes, favorite toggles, duplicate and migrate hand-off (`.../ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt`).
- **Global search**: parallel search across enabled sources, result ordering, single-result auto-open (`.../ui/browse/manga/source/globalsearch/MangaSearchScreenModel.kt`).
- **Extensions tab**: grouped extension list, install / update / uninstall / trust, language filter, details page, per-source preferences (`.../ui/browse/manga/extension/**`).
- **Migration**: source picker, favorites of a source, per-source and global migrate search, flag bitmap, confirm dialog, and the migration engine that moves library state from one manga to another (`.../ui/browse/manga/migration/**`).

### 1.2 Why it matters for a usable Kurobayomi

Without this area the user cannot discover manga from any source, cannot add a manga to the library from a source, and cannot install or enable a source. Reading (area:43-reader-core) and the library (area:37-ui-library) are reachable only through the entry points defined here. Migration is the only way to move a library entry to a different source without losing progress, categories and tracks. The P0 subset of this area is therefore the path "pick a source, browse it, open a manga, add it to the library", plus the extensions screen that makes a source exist in the first place.

### 1.3 Scope boundary

In scope:

- Part 1 (presentation and page wiring): 40 Kotlin files, 4984 LOC. Inventory: [p1-inventory.md](p1-inventory.md).
- Part 2 (models, state machines, algorithms, data writes): 12 Kotlin files, 1749 LOC. Inventory: [p2-inventory.md](p2-inventory.md).
- Total: 52 Kotlin files, 6733 LOC. Both counts come from `_tools/areas.py files 40-ui-browse-manga 1` and `... 2`.

Out of scope, owned elsewhere (see section 5): the anime browse (area:41-ui-browse-anime), the source plumbing (area:17-source-app-layer), extension packaging and install (area:18-extensions-manga, area:20-extension-strategy-redesign), the library screens (area:37-ui-library), and the shell (area:36-ui-shell-navigation-security-webview).

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities

| Kotlin package (repo-relative) | Responsibility | Owner doc |
|---|---|---|
| `app/src/main/java/eu/kanade/presentation/browse/` | Shared row shell, in-library badge, loading row, global search result rows, remove-entry dialog | p1 |
| `app/src/main/java/eu/kanade/presentation/browse/manga/` | Compose screens: browse source, sources list, sources filter, global search, extensions list, extension details, extension filter, migrate screens | p1 |
| `app/src/main/java/eu/kanade/presentation/browse/manga/components/` | Grids (comfortable, compact), list, toolbars, source and extension icons, base source row | p1 |
| `app/src/main/java/eu/kanade/tachiyomi/ui/browse/` | `BrowseTab` host | p1 |
| `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/` | Extensions tab wrapper, extensions model, language filter model | p1 |
| `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/` | Extension details model and screen, source preferences host | p1 |
| `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/` | Flag bitmap, favorites of a source, migrate search, migrate dialog and engine | p1 |
| `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/sources/` | Migrate source picker model and tab (sort mode and direction, favorite counts) | p2 |
| `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/` | Sources page model, sources filter model, sources tab | p2 |
| `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/` | Browse-one-source screen and model, filter tree dialog | p2 |
| `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/` | Global search base model, global search model, global search wrapper | p2 |
| `app/src/main/java/eu/kanade/domain/source/manga/interactor/` | Used, not owned: `GetEnabledMangaSources.kt` (enabled sources, pins, last used), toggle interactors | area:17 |

### 2.2 Proposed Rust module map

Root: `kurobayomi::browse`. The GTK pages are thin; the logic lives in GTK-free modules. Authoritative trees: [p1-rust-design.md](p1-rust-design.md) section 2 and [p2-rust-design.md](p2-rust-design.md) section 2.

```
src/browse/
  mod.rs                       SourceId, MangaId, Pins, BrowseHandle
  tab.rs                       Browse tab page: Stack of six sub-pages (GTK)
  widgets.rs                   base row, in-library badge, loading row, empty/error state (GTK)
  dialogs.rs                   remove-entry, untrusted extension, NSFW, uninstall-private (GTK)
  icons.rs                     source icon resolution, extension icon loader
  display_name.rs              language display names, comparator (pure, ICU4X)
  source_list.rs               Sources page (GTK)            uses model::source_list
  source_filter.rs             Show/hide sources page (GTK)  uses model::source_filter
  source_browse.rs             Browse-one-source page (GTK)  uses model::browse_source, paging, filter_tree
  global_search.rs             Global search page (GTK)      uses model::global_search
  extensions/{mod,model,install,list,filter,details,source_prefs}.rs
  migration/{mod,flags,engine,source_picker,favorites,search,dialog}.rs
  prefs.rs                     BrowsePrefs typed view over browse config keys
  model/                       (part 2, pure, no GTK, no tokio)
    source_row.rs, source_list.rs, source_filter.rs, migrate_sources.rs,
    collate.rs, listing.rs, filter_tree.rs, paging.rs, browse_source.rs,
    favorites.rs, global_search.rs, migrate_search.rs
  services.rs                  traits + tokio glue (part 2)
```

Dependency rules (from the two designs):

- Pure modules import only `std`, `icu_collator`, `thiserror` and `super::{SourceId, MangaId, Pins}`.
- `services.rs` imports tokio and the traits. GTK pages import both.
- The GTK main thread owns every model value. Background tasks send typed events over `async_channel`; `glib::spawn_future_local` applies them.

### 2.3 Crate choices for this area

| Need | Crate | Source of decision |
|---|---|---|
| Widgets | `gtk4` 0.11 | target |
| Background I/O | `tokio` 1 (`rt-multi-thread`, `sync`, `time`, `macros`) | target |
| Event channel | `async-channel` 2 | p1-rust-design section 8 |
| Cancellation | `tokio-util` 0.7 (`CancellationToken`) | p1 and p2 section 8 |
| Trait objects over async | `async-trait` 0.1 | p1 section 8 |
| Errors | `thiserror` 1 | p1 section 8 |
| Language display names | `icu_displaynames` (p1 says `1.x`) | p1-rust-design section 8, see open question Q5 |
| Collation (primary strength) | `icu_collator` and `icu_locale` (p2 says `2.x`) | p2-rust-design section 8, see open question Q5 |
| Logging | `tracing` 0.1 | p2 section 8 |

Rejected by the designs: `libadwaita` (forbidden by the target), `relm4` (extra component framework), `diesel` (rusqlite already chosen), `indexmap` and `sorted-vec` (a `Vec` with a stable sort is enough for up to a few hundred sources).

---

## 3. Reading guide

Read in this order: this overview, then `p1-behavior.md` and `p2-behavior.md` for the rules, then the rest as needed.

| Document | One-line description |
|---|---|
| [00-overview.md](00-overview.md) | This file: scope, module map, decisions, dependencies, task summary, definition of done. |
| [p1-inventory.md](p1-inventory.md) | Table of the 40 part-1 Kotlin files: LOC, declared names, port decision, Rust target, priority, task IDs. |
| [p2-inventory.md](p2-inventory.md) | Table of the 12 part-2 Kotlin files with the same columns, plus per-file risk notes. |
| [p1-behavior.md](p1-behavior.md) | Observed behavior of the presentation layer, sections 1 to 19: browse shell, grids, sources list, extensions, install state machine, details, global search, migration screens, engine, drops, message keys. |
| [p2-behavior.md](p2-behavior.md) | Observed behavior of the models, sections 0 to 18: shared rules, sources page, filter page, browse-one-source, paging, filter dialog, global search algorithm, migrate ordering, deviations D-BR/D-GS/D-MG/D-DUP/D-SS/D-FAV, drops. |
| [p1-ui-parity.md](p1-ui-parity.md) | Screen-by-screen UI parity spec (S1 to S18), keyboard and mouse map, responsive sizing, mockup gap list G1 to G21, UI deviations D-UI-1 to D-UI-10. |
| [p2-ui-parity.md](p2-ui-parity.md) | UI parity for the part-2 screens: states per screen, dialog decision table, desktop keyboard map, conflicts with part 1 and corrections. |
| [p1-data-and-config.md](p1-data-and-config.md) | Data models, preference keys, extension package files, network endpoints, DB tables touched by migration, `browse.toml` keys, icon and size constants. |
| [p2-data-and-config.md](p2-data-and-config.md) | Models, enum stored values, bit layouts, on-disk files, preference keys, `browse.toml` keys, endpoints, DB tables, data-model invariants. |
| [p1-rust-design.md](p1-rust-design.md) | Rust design for the presentation side: principles, module tree, core types, traits, threading, GTK widget mapping, crates, Android-to-Linux mapping, pitfalls. |
| [p2-rust-design.md](p2-rust-design.md) | Rust design for the models: principles, module tree, core types, Kotlin-to-Rust mapping, cancellation, error handling, crates, pitfalls. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases E-40-nn, failure modes, races, and the part-1 test plan with coverage check. |
| [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md) | Upstream tests (none), edge cases by area, races, failure modes, logging sites, fixtures, the part-2 test list, test data policy. |
| [p1-tasks.md](p1-tasks.md) | Implementation tasks `T-40a-001` to `T-40a-054` (54 tasks). |
| [p2-tasks.md](p2-tasks.md) | Implementation tasks `T-40b-001` to `T-40b-087` (57 tasks). |
| [p2-topic-browse-listing-and-filters.md](p2-topic-browse-listing-and-filters.md) | Topic: listing transitions, navigate-up sequence, demand-driven paging, filter tree semantics, dialog state machine, test vectors. |
| [p2-topic-global-search-engine.md](p2-topic-global-search-engine.md) | Topic: global search state, transitions, ordering worked examples, concurrency design, why a `Vec` and not a map, test vectors. |

---

## 4. Key design decisions and open questions

### 4.1 Decisions already made in the area docs (accepted here)

| ID | Decision | Where | Why |
|---|---|---|---|
| A1 | Models are GTK-free, tokio-free, pure functions or plain structs under `browse/model/`. | p2-rust-design section 1 | Unit tests without a display |
| A2 | Single writer: the GTK main thread owns every model value; background tasks send events. | p2-rust-design section 1, principle 2; D-GS-02 | Removes the lost-update race in `MangaSearchScreenModel.kt:192-197` |
| A3 | Generation counters decide whether a result is stale. No equality on filter trees. | p2-rust-design section 1, principle 3 | `FilterList.equals` is always false in Kotlin |
| A4 | Global search results are a stable `Vec` sorted by the same key, not a map keyed by a comparator. | p2-rust-design section 1, principle 4; D-GS-01; `MangaSearchScreenModel.kt:186` | A comparator-keyed `TreeMap` drops sources whose keys collide |
| A5 | Migrate ordering is origin source first, then pinned, then `"name (lang)"` lowercased. | D-MG-01, p1-behavior section 13 | The base sort in Kotlin discards the subclass order |
| A6 | Failed favorite writes show `internal_error` and roll the row back. | D-FAV-02 | Errors must not diverge silently |
| A7 | Long-running work is bounded: a semaphore of 5 permits replaces the upstream thread pool of 5 that is never shut down. | D-GS-03 | No thread leak |
| A8 | Browse tab re-select opens global search of the visible kind (manga or anime). | D-UI-1 | Upstream comment at `BrowseTab.kt:51` marks it unfinished |
| A9 | Empty browse source with no error shows `empty_screen` text, not a blank grid. | D-UI-2 | A blank page reads as broken on desktop |
| A10 | Migration failure shows a toast `internal_error`; the dialog stays open. | D-UI-4, D-MG-02 | Silent failure is hard to diagnose |
| A11 | Migration DB steps run in one SQLite transaction; file operations run after commit. | p1-tasks T-40a-041 (acceptance) | Atomicity; E-40-85, E-40-86 |
| A12 | Extension icons and source icons come from files inside the extension bundle (decoded at the requested size), not from Android `PackageManager`. | p1-inventory D7, p1-data-and-config section 6 | No Android package manager on Linux |
| A13 | The source preference page is a native GTK page built from the source's declarative preference list. The Fragment and reflection host is dropped. | p1-inventory row `MangaSourcePreferencesScreen.kt`, D4 | Reflection hack has no Linux equivalent |
| A14 | Collation: primary strength, case-insensitive, with a fixed fallback if ICU data is missing. | p2-rust-design section 8 | Matches Kotlin `Collator` `PRIMARY` |

### 4.2 Open questions that need the user's call

Each question has a recommendation. Anything not answered takes the recommendation by default.

| # | Question | Options | Recommendation |
|---|---|---|---|
| Q1 | Chip filters in global search: rerun the search on chip change (what the code does, `MangaSearchScreenModel.kt:115-118`) or apply only on the next search (what part 1 documents as D-UI-10 "same (kept)")? | (a) rerun, (b) next search | (a) rerun. It matches upstream runtime behavior. Correct D-UI-10 in [p1-ui-parity.md](p1-ui-parity.md) section 14 to say so. p2-ui-parity section 9 item 1 already states the correction. |
| Q2 | Drop the data-saver feature (source menu row, `DataSaver` enum, excluded-source list). Upstream rewrites image URLs through third-party proxies (`bandwidth-hero`, `wsrv.nl`, `resmush.it`, `SourcePreferences.kt:118-123`). | (a) drop, (b) keep and route through area:12 | (a) drop. The user loses image proxying for sources that rely on it. This is an SY-fork feature; the user should confirm. Tasks: T-40b-082 (P3), T-40a-042 (P3). |
| Q3 | Migration always copies tracks (upstream hides this). | (a) keep the disclosure line under the checkboxes (D-UI-9), (b) hide it | (a) keep the disclosure line. It is one line of text. |
| Q4 | Extension "install" on Linux. The state machine (`InstallStep` six values) and install UI are specified here, but the install mechanism (what a package is, where it is stored, how it is trusted) is owned by area:20-extension-strategy-redesign. | Decision by area 20 | Area 20 decides; this area keeps `InstallStep` and the cancel and reference-count rules unchanged. Blocks T-40a-018, and through it T-40a-017 and T-40a-019, until area 20 is settled. |
| Q5 | ICU4X version. Part 1 pins `icu_displaynames` at `1.x`. Part 2 uses `icu_collator` and `icu_locale` at `2.x` "same release train". Mixing releases will not compile together. | (a) all 2.x, (b) all 1.x | (a) all 2.x. Pin all three at implementation. Edit p1-rust-design section 8 to match. |
| Q6 | Which SQLite crate version (rusqlite `0.31`+ with `bundled` is the placeholder). | Decision by area 03 | Area 03 decides. Nothing in this area depends on the version. |
| Q7 | Approve the deviation list: part 1 D-UI-1 to D-UI-10 and part 2 D-BR-01 to D-BR-03, D-GS-01 to D-GS-03, D-MG-01, D-MG-02, D-DUP-01, D-SS-01, D-FAV-01, D-FAV-02, D-CH-01. | (a) approve all, (b) reject some | (a) approve all. Each one removes a crash, a silent drop or an unclear state. Reject any the user wants to match upstream exactly. |
| Q8 | Kurobayomi has no Android app-info screen and no "install unknown apps" permission. Is an "Open extension folder" action wanted? | (a) add as P2 (T-40a-031 covers the drop), (b) drop | (b) drop. It is a P2 option in [p1-inventory.md](p1-inventory.md) D1 and can be added later. |

### 4.3 Inconsistencies found while writing this overview

These were corrected in the task files and docs during the lead pass. Recorded here so a reviewer can verify.

| Item | Before | After |
|---|---|---|
| Invalid dependency token `area:41-ui-browse-manga-part-2` (not an area id) | 4 tasks in p1-tasks.md (T-40a-003, T-40a-007, T-40a-014, T-40a-050) | Replaced with real IDs: T-40a-003 depends on T-40b-030, T-40b-032, T-40b-040; T-40a-007 on T-40b-010, T-40b-011; T-40a-014 on T-40b-067; T-40a-050 on T-40b-065. Prose references in p1-behavior.md, p1-inventory.md and p1-tasks.md now name part 2 by file. |
| Self-referencing dependency `area:40-ui-browse-manga` inside area 40 | T-40b-004, T-40b-010, T-40b-046, T-40b-067, T-40b-087 | Replaced with real task IDs or area ids (T-40b-004 on area:36 and area:01; T-40b-010 on T-40a-033; T-40b-046 on T-40a-015; T-40b-067 on T-40a-033; T-40b-087 on T-40a-010). |
| Prose reference to `area:41` for migration | p2-behavior.md section 15 | Points to part 1 (`p1-behavior.md` sections 14 and 15). |
| Dependency cycles inside area 40 | 9 tasks were in cycles (T-40a-016/041, T-40a-017/018, T-40a-017/025, T-40a-021/022, T-40a-024/038) | Removed the back edges: T-40a-018 no longer depends on T-40a-017; T-40a-021 no longer on T-40a-022; T-40a-024 no longer on T-40a-038; T-40a-016 no longer on T-40a-041; T-40a-025 no longer on T-40a-017. |
| Part 2 totals | p2-inventory.md totals: "P0 24, P1 25, P2 3, P3 5" | Matches the TASKS.csv export for `p2-tasks.md` (see section 6.1). |

Tasks lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint` passes on both files (`ok`). `tasks.py export` reports `errors=0` across all 3801 tasks.

### 4.4 Risks behind the decisions

- **Migration engine correctness (T-40a-016, T-40a-041).** Several DB writes (chapters, categories, tracks, favorite flip) plus file operations in one flow. A partial failure leaves a library entry with two favorites or none. Mitigation: one transaction, favorite flip ordering (E-40-86), and a test that injects failures at each step (p1-edge-cases-and-tests section 4).
- **Global search ordering (T-40b-060, T-40b-061).** Comparator-keyed maps silently drop results (`MangaSearchScreenModel.kt:186`). Mitigation: `Vec` plus stable sort (A4), with the collision test vector in [p2-topic-global-search-engine.md](p2-topic-global-search-engine.md) section 6.
- **Extension install (T-40a-018) depends on area:20.** If area 20 changes the package format, icons (T-40a-006, T-40a-027) and the install state machine change with it.
- **ICU data size and availability.** `icu_collator` and `icu_displaynames` add data. Mitigation: fallback to the BCP-47 tag and a fixed primary comparison (Q5, p2-rust-design section 8).
- **Compose has no direct sticky header in GTK.** The migrate sort row is a fixed `gtk::Box` above the list (p1-rust-design section 7). It does not scroll, which matches the visible behavior but not the mechanism.
- **Paging.** Kotlin pages with `LazyPagingItems`. Rust uses demand-driven paging with a generation counter (p2-topic-browse-listing-and-filters section 4). A bug here shows as duplicated or missing rows; the test vectors in that topic cover it.
- **Scale.** The largest Kotlin files in scope are `app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt` (546 LOC, part 1) and `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt` (370 LOC, part 2). Both are split across the Rust module tree above.

---

## 5. Cross-area dependencies

### 5.1 What this area needs from other areas

Every `area:` token in this area's task files, with the tasks that carry it (from TASKS.csv, current export).

| Area | Tasks in area 40 that depend on it |
|---|---|
| area:01-foundation-app-bootstrap | T-40b-004 |
| area:02-preferences-system | T-40a-002, T-40a-023, T-40a-032, T-40a-033, T-40b-001, T-40b-005, T-40b-039 |
| area:03-db-manga-schema-migrations | T-40a-016, T-40a-041 |
| area:05-data-repos-entries-items | T-40a-016, T-40b-033, T-40b-070 |
| area:07-domain-entries-items | T-40a-016 |
| area:08-domain-library-category-history-updates | T-40b-041, T-40b-042, T-40b-045 |
| area:12-network-stack | T-40a-022 |
| area:14-source-local-and-metadata | T-40a-006 |
| area:17-source-app-layer | T-40a-022, T-40a-023, T-40b-001, T-40b-003 |
| area:18-extensions-manga | T-40a-017, T-40a-018, T-40a-020, T-40a-021, T-40a-027, T-40a-046, T-40a-047 |
| area:20-extension-strategy-redesign | T-40a-017 (open question Q4) |
| area:23-downloads-manga | T-40a-015 |
| area:27-tracking-core | T-40a-016 |
| area:34-images-covers-cache | T-40a-004, T-40a-015, T-40a-027, T-40b-041 |
| area:36-ui-shell-navigation-security-webview | T-40a-001, T-40b-004, T-40b-065, T-40b-073, T-40b-083 |
| area:37-ui-library | T-40a-004 |

Dependencies inside this area are written as task IDs (`T-40a-*`, `T-40b-*`). Part 1 depends on part 2 by ID only where a specific model is needed (for example T-40a-003 on T-40b-032).

### 5.2 What other areas need from this area

Tasks in other areas whose `Depends` lists `area:40-ui-browse-manga` (current export, 16 tasks). Each row names the part of this area they use.

| Task | Title | Uses from area 40 |
|---|---|---|
| T-06a-081 | Wire source browse list and pager of the mockup | Browse one source (part 1 `browse/source_browse.rs`) |
| T-15a-039 | Filter sheet renderer for all eight filter kinds | Filter sheet (T-40b-050) |
| T-15a-041 | Source settings screen renderer | Source preference page (T-40a-023) |
| T-34a-051 | Migration copy of custom cover and background | Migration engine (T-40a-016) |
| T-35a-029 | Drop or replace `searchable.xml` (global search hint and search provider) | Global search entry points |
| T-36a-004 | Home tab host and start screen | Browse tab host (T-40a-001) |
| T-37a-022 | Global search row | Global search result rows (T-40a-011) |
| T-41a-012 | Untrusted extension trust dialog and flow | Trust dialog (T-40a-019) |
| T-41a-020 | Extension details page | Extension details page (T-40a-021) |
| T-41a-030 | Global search source header, origin marker and subtitle | Global search result headers (T-40a-011) |
| T-42a-020 | Migrate dialog wiring from history | Migrate dialog (T-40a-015) |
| T-47a-060 | Hide entries already in library (browse lists) | Hide-in-library filter (T-40b-034) |
| T-47a-061 | Show NSFW (18+) sources, auth-gated | Sources list visibility (T-40a-009) |

Note: the list is a snapshot of TASKS.csv taken during the lead pass. Re-run `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export` to refresh it.

---

### 6.1 Counts

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then the rows of `TASKS.csv` whose file starts with `40-ui-browse-manga/`.

Total: **111 tasks** (`p1-tasks.md` 54, `p2-tasks.md` 57). Lint: `ok` for both files.

| Priority | S | M | L | Total |
|---|---|---|---|---|
| P0 | 21 | 16 | 2 | **39** |
| P1 | 32 | 18 | 5 | **55** |
| P2 | 8 | 0 | 0 | **8** |
| P3 | 8 | 1 | 0 | **9** |
| Total | 69 | 35 | 7 | **111** |

Per file: `p1-tasks.md` has P0 15, P1 30, P2 5, P3 4. `p2-tasks.md` has P0 24, P1 25, P2 3, P3 5.

Effort definitions follow the project brief (S <= 1 day, M <= 3 days, L <= 1.5 weeks). No task is XL.

### 6.2 P0 critical path

Dependencies below are the ones written in the task files. External dependencies are named in parentheses where they gate a step.

1. **Foundations (external):** area:02 preferences, area:17 source registry, area:36 shell, area:01 runtime.
2. **Shared rows and labels:** T-40a-033 (display names, language order) → T-40a-005 (base row, source row) → T-40a-030 (loading, empty, error primitives).
3. **Browse one source (reading path):**
   - T-40b-004 (generation-guarded loop and cancellation) → T-40b-032 (paged remote listing), which also needs T-40b-030 (listing model).
   - T-40a-004 (grids, favorite alpha, in-library badge) needs T-40a-005 and T-40a-006 (source icons).
   - T-40a-003 (browse content states and dispatch) needs T-40a-030, T-40a-004, T-40b-030, T-40b-032 and T-40b-040 (stub page and loading gate).
   - T-40a-002 (browse header bar and display-mode menu) needs T-40a-033 and T-40a-003.
4. **Sources page:** T-40b-001 (source value type) → T-40b-010 (grouping, needs T-40a-033) → T-40b-011 (pin and disable) → T-40b-012 (options dialog actions). T-40a-007 (installed list) needs T-40b-010 and T-40b-011.
5. **Filter tree:** T-40b-031 (filter tree model, no dependencies) → T-40b-051 (TriState) and T-40b-053 (widgets). T-40b-050 (filter sheet) needs T-40b-031 and T-40b-036 (navigate-up rule).
6. **Favorite and add to library:** T-40b-033 (map remote manga to local rows) → T-40b-041 (favorite toggle with partial writes) → T-40b-044 (long-click decision) and T-40b-047 (remove-from-library confirm).
7. **Extensions screen (longest P0-only chain):** T-40a-018 (install, update, uninstall, cancel state machine, needs area:18) → T-40a-017 (extensions tab list, needs T-40a-018, T-40a-024, T-40a-025 and area:20) → T-40a-034 (route table and mockup replacement).

The longest chain by effort weight in the whole area (S=1, M=2, L=4) includes P1 migration work: T-40a-033 → T-40a-024 → T-40a-038 → T-40a-016 → T-40a-015 → T-40a-014 → T-40a-054, weight 15. It does not block daily reading.

### 6.3 Risks for the task list

- **P0 size.** 39 P0 tasks is a large first milestone. Items 3 to 7 above are the minimum path for reading and adding from one source. Cut scope only after the user decides (Q7 covers the deviations, not priorities).
- **Two L-effort tasks at P0.** T-40a-017 and T-40a-018. Both are candidates for splitting if they overrun (list vs install state machine).
- **Migration risk** is concentrated in T-40a-016 and T-40a-041 (section 4.4).
- **Drop tasks.** The part-2 drop tasks T-40b-080, T-40b-081, T-40b-082 and the part-1 drop T-40a-042 are P3 and must not block P0.

---

## 7. Definition of done for this area in Kurobayomi

Each item is an observable behavior on a real desktop build (Wayland and X11), checked by the acceptance script T-40b-086.

1. **Browse tab.** The Browse tab shows the six sub-pages specified in [p1-ui-parity.md](p1-ui-parity.md) section 2. Re-selecting the tab on a manga page opens manga global search, and on an anime page opens anime global search (A8).
2. **Sources list.** The list shows real installed manga sources grouped by language, with headers in this order: last used, pinned, then language codes by code point, with an empty language last. A source shows a pin control, a Latest button only when the source supports Latest, and an options dialog with pin, disable, and the help or web view actions for local and other sources. The list shows a spinner until the source registry is initialized (D-SS-01), never an empty flash.
3. **Show and hide sources.** Language switches and per-source checkboxes persist across restarts in the `hidden_catalogues` preference, and hidden sources disappear from the Sources list and from global search.
4. **Browse one source.** Popular and Latest listings page on demand. Empty results show `empty_screen`. An error shows a bottom toast with `action_retry` and does not auto-hide. Display modes (comfortable grid, list, compact grid, cover-only grid) persist across restarts (`display_mode` in `browse.toml`, default `COMPACT_GRID`). The filter sheet applies the filter kinds with the tri-state cycle, and reset clears the filter state.
5. **Favorite from a browse row.** Clicking the favorite control adds the manga to the library and shows the category chooser (T-40b-045). A failed write shows `internal_error` and rolls the row back (A6). Long-click opens the duplicate or migrate decision.
6. **Global search.** Typing a query searches all enabled sources in parallel. Results appear per source as they arrive, and the progress bar tracks completed sources. The "has results" toggle and the source filter (pinned, all, one extension) apply. Results are ordered by the same key as upstream, with no source dropped on a key collision (A4). A single result auto-opens the manga page.
7. **Extensions.** The Extensions tab lists updates, installed, untrusted, and available extensions grouped by language. Install, update, cancel, and uninstall run through the six `InstallStep` states. An untrusted extension asks for trust before install. Update-all runs sequentially. The language filter enables and disables languages. The details page toggles sources, enables or disables all, clears cookies, and toggles incognito.
8. **Source preferences.** A source with declared preferences shows a native GTK page built from those declarations (A13). A source without preferences shows no settings action.
9. **Migration.** From the source picker, the user picks a source sorted by name or favorite count, opens its favorites, and searches globally or per source with the origin source first. The confirm dialog offers the flags for chapters, categories, custom cover, and delete downloaded. Migrating moves chapters, categories, tracks, and the favorite flag in one transaction; file operations run after commit. A missing manga shows an error page, not a crash.
10. **No crashes on malformed data.** A source that returns an unknown `TriState` value, a missing source, or a missing manga never panics; each case has a logged warning and a visible state (D-BR-02, D-SS-01, E-40-63).
11. **Keyboard and mouse.** Every page works without a pointer. The map is in [p1-ui-parity.md](p1-ui-parity.md) section 9 and [p2-ui-parity.md](p2-ui-parity.md) section 4. Right-click opens the row context menu (G18).
12. **Tests.** The pure-logic unit tests (T-40a-038, T-40b-084) and the in-memory SQLite integration tests (T-40b-085) pass. The migration atomicity tests show that an injected failure at each DB step leaves the library unchanged.
13. **Drops are visible and documented.** Data saver, the Android app-info button, the install-permission banner, the haptic feedback, and the assist-content URL do not appear in the UI, and the drop list in [p1-behavior.md](p1-behavior.md) section 16 and [p2-behavior.md](p2-behavior.md) section 15 matches what the user sees.
