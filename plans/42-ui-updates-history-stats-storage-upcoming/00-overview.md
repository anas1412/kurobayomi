# Area 42: Updates, History, Stats, Storage, Upcoming (overview)

Area lead document. Repo root for every Aniyomi path below is `/home/blackbox/aniyomi-src` (read-only, Apache-2.0). Kurobayomi target is `/home/blackbox/kurobayomi`. Labels: **OBSERVED** = what the Kotlin does; **Proposal:** = what Kurobayomi should do.

Area id: `42-ui-updates-history-stats-storage-upcoming`. Refer to other areas as `area:<id>` with the full id (for example `area:23-downloads-manga`).

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

Five top-level screens that answer "what did I read or watch, what is new, what do I have, how much, what is coming":

| Screen | Entry point (Kotlin) | What it shows | Data source |
|---|---|---|---|
| History | `app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt` | Last-read/last-watched unit per entry, date-grouped, with resume-on-click, remove, clear-all, add-to-library | `history` and `animehistory` tables through `historyView` and `animehistoryView` |
| Updates | `app/src/main/java/eu/kanade/tachiyomi/ui/updates/UpdatesTab.kt` | New chapters and episodes of library favorites in the last 3 months, date-grouped, with selection, bulk read/bookmark/download/delete | `updatesView` and `animeupdatesView` |
| Statistics | `app/src/main/java/eu/kanade/tachiyomi/ui/stats/StatsTab.kt` | Counts, durations, tracker mean score, global-update count, per-kind | Library rows, history sums, tracks, download cache |
| Storage | `app/src/main/java/eu/kanade/tachiyomi/ui/storage/StorageTab.kt` | Downloaded size per entry, half-donut chart, category filter, per-entry delete | Library rows plus download cache |
| Upcoming | `app/src/main/java/mihon/feature/upcoming/` | Calendar month plus list of favorites with a known next release, grouped by day | `getUpcomingManga` / `getUpcomingAnime` queries |

### 1.2 Why it matters for a usable Kurobayomi (Proposal)

- History is the daily entry point for reading and watching. Without it, the P0 "resume" flow (open the next unit of the last entry) cannot be used. Half of the P0 tasks of this area are History.
- Updates is how a library user learns what to read next. It is also where bulk download and delete happen, so it carries data-loss risk (section 4, decision D-6).
- Stats and Storage are P1: parity features that answer "how much disk am I using" and "what am I tracking".
- Upcoming is P2. Desktop users rarely need a calendar, but the spec exists and the mockup already links to it.

### 1.3 Scope accounting

- **Part 1** (History, Stats, Storage, the Updates tab shell and Updates presentation): 41 Kotlin files, 4,730 LOC. Inventory: [p1-inventory.md](p1-inventory.md).
- **Part 2** (Updates models, selection engine, actions, and the whole Upcoming feature): 18 Kotlin files, 2,368 LOC. Inventory: [p2-inventory.md](p2-inventory.md).
- Total: 59 files, 7,098 LOC.

Resolved gap: the Part 2 inventory says History, Stats and Storage are "not in this part's file list" and asks for an owner. That is correct for Part 2 only. Part 1 covers them. **Area 42 owns all five screens.** The Part 2 note is stale and should be read that way.

Out of scope for this area (owned elsewhere, cited only): entry screens (`area:38-ui-entry-details-manga`, `area:39-ui-entry-details-anime`), reader and player (`area:43-reader-core`, `area:45-player-core`), download manager (`area:23-downloads-manga`, `area:24-downloads-anime`, `area:25-downloads-shared`), library update job (`area:26-library-update-jobs`), navigation and window shell (`area:36-ui-shell-navigation-security-webview`), cover cache (`area:34-images-covers-cache`).

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities (OBSERVED)

| Package (under `app/src/main/java/`) | Files (part) | Responsibility |
|---|---|---|
| `eu/kanade/presentation/history/` (+ `anime/`, `manga/`, `components/`) | P1 | History list screens, row composables, delete dialogs, preview fixtures |
| `eu/kanade/presentation/updates/` (+ `anime/`, `manga/`) | P1 (presentation), P2 (cited) | Updates page, rows, bottom action bar, delete dialog |
| `eu/kanade/presentation/more/stats/` (+ `components/`, `data/`) | P1 | Stats page content, `StatTile`, typed stats payloads |
| `eu/kanade/presentation/more/storage/` | P1 | Storage page content, `CumulativeStorage` half-donut, `StorageItem` row, category select |
| `eu/kanade/tachiyomi/ui/history/` (+ `anime/`, `manga/`) | P1 | `HistoriesTab`, `AnimeHistoryScreenModel`, `MangaHistoryScreenModel`, tab wiring, resume and remove logic |
| `eu/kanade/tachiyomi/ui/updates/` (+ `anime/`, `manga/`) | P1 (tab), P2 (models) | `UpdatesTab`, `AnimeUpdatesScreenModel`, `MangaUpdatesScreenModel` (selection, download dispatch, actions) |
| `eu/kanade/tachiyomi/ui/stats/` (+ `anime/`, `manga/`) | P1 | `StatsTab`, `AnimeStatsScreenModel`, `MangaStatsScreenModel` (snapshot computation) |
| `eu/kanade/tachiyomi/ui/storage/` (+ `anime/`, `manga/`) | P1 | `StorageTab`, `CommonStorageScreenModel<T>` (aggregation), per-kind models |
| `mihon/feature/upcoming/` (+ `anime/`, `manga/`, `components/calendar/`) | P2 | Upcoming screens, screen models, calendar grid, day cell, dot, header |
| `tachiyomi/domain/history/{manga,anime}/` | cited | Models `MangaHistoryWithRelations`, `AnimeHistoryWithRelations`; interactors `GetMangaHistory`, `RemoveMangaHistory`, `GetNextChapters`, `GetNextEpisodes` |
| `tachiyomi/domain/updates/{manga,anime}/` | cited | Models `MangaUpdatesWithRelations`, `AnimeUpdatesWithRelations`; interactors `GetMangaUpdates`, `GetAnimeUpdates` |
| `mihon/domain/upcoming/{manga,anime}/` | cited | Interactors `GetUpcomingManga`, `GetUpcomingAnime` |
| `tachiyomi/data/history/`, `tachiyomi/data/updates/` | cited | Repositories; SQLDelight queries in `data/src/main/sqldelight/` and `data/src/main/sqldelightanime/` |
| `eu/kanade/tachiyomi/util/lang/DateExtensions.kt`, `eu/kanade/presentation/components/DateText.kt`, `eu/kanade/presentation/util/TimeUtils.kt`, `eu/kanade/tachiyomi/util/StorageUtil.kt`, `eu/kanade/core/util/CollectionUtils.kt` | P1, P2 | Shared formatting and grouping helpers used by all five screens |

### 2.2 Proposed Rust module map (Proposal)

Two Rust layouts exist in the part docs and they disagree. Decision D-2 (section 4) picks one. The table below is the adopted layout.

| Rust location | Contents | Source of the design |
|---|---|---|
| `crates/kuro-domain/src/history/` | `HistoryRow`, next-unit algorithm (`GetNextChapters` / `GetNextEpisodes` port), date grouping | [p1-rust-design.md](p1-rust-design.md) section 1 |
| `crates/kuro-domain/src/updates/` | `UpdateRow`, `UpdateItem<T>`, `DownloadSnapshot`, `group_by_day`, `selection.rs` (range engine), `actions.rs` (planning) | [p1-rust-design.md](p1-rust-design.md) section 1, [p2-rust-design.md](p2-rust-design.md) section 3 |
| `crates/kuro-domain/src/upcoming/` | `Entry`, `UpcomingRow`, `build_upcoming`, `calendar.rs` (week start, grid math) | [p2-rust-design.md](p2-rust-design.md) section 1 |
| `crates/kuro-domain/src/stats/` | `StatsSnapshot`, `MangaStats`, `AnimeStats`, `mean.rs`, `global_update.rs` | [p1-rust-design.md](p1-rust-design.md) section 1 |
| `crates/kuro-domain/src/storage/` | `StorageRow`, `StorageSnapshot`, `color.rs`, `size.rs` | [p1-rust-design.md](p1-rust-design.md) section 1 |
| `crates/kuro-domain/src/format/` | `relative_date`, `relative_time_span`, `duration_string`, `progress_mmss`, `format_size`. One implementation, used by this area and by `area:38-ui-entry-details-manga` / `area:39-ui-entry-details-anime` | Replaces `src/time_fmt.rs` in part 2 (decision D-3) |
| `crates/kuro-data/src/` | `history.rs`, `updates.rs`, `stats_queries.rs`, `storage_queries.rs`, `upcoming_queries.rs`. rusqlite, read queries plus write helpers | [p1-rust-design.md](p1-rust-design.md) section 4, [p2-data-and-config.md](p2-data-and-config.md) section 3 |
| `src/ui/history/` | `tab.rs` (HistoriesTab), `list.rs` (date-grouped ListView), `row.rs` (96 px), `dialogs.rs` | [p1-rust-design.md](p1-rust-design.md) section 1 |
| `src/ui/updates/` | `tab.rs`, `manga.rs`, `anime.rs` (page builders), `list.rs`, `row.rs` (56 px), `bottom_bar.rs`, `dialogs.rs`, `toast` use | [p1-rust-design.md](p1-rust-design.md), [p2-rust-design.md](p2-rust-design.md) section 1 |
| `src/ui/upcoming/` | `page.rs`, `manga.rs`, `anime.rs`, `calendar_ui.rs` (gtk::Grid month view), `item_row.rs` | [p2-rust-design.md](p2-rust-design.md) section 1 |
| `src/ui/stats/` | `tab.rs`, `tile.rs` (StatTile), `card.rs` (SectionCard) | [p1-rust-design.md](p1-rust-design.md) section 1 |
| `src/ui/storage/` | `tab.rs`, `donut.rs` (cairo DrawingArea), `row.rs`, `category.rs` (DropDown) | [p1-rust-design.md](p1-rust-design.md) section 1 |
| `src/ui/shared/` | `selection.rs` (gesture to selection adapter), `relative.rs` (preference glue) | [p1-rust-design.md](p1-rust-design.md) section 1 |

Dependency direction (Proposal): `kuro-domain` has no GTK and no rusqlite. `kuro-data` depends on `kuro-domain` and `rusqlite`. The root or `kuro-ui` crate depends on both. Every module under `kuro-domain` is unit-testable without a display.

Crates used by this area (Proposal): `gtk4 = 0.11` (feature `v4_16`), `rusqlite` (bundled), `chrono` (feature `clock`), `tokio` (IO runtime), `async-channel` (GTK delivery), `thiserror`, `tracing` and `tracing-subscriber`. No new crate for the calendar grid or the donut chart; cairo comes through gtk4. Source: [p2-rust-design.md](p2-rust-design.md) section 2 and [p1-rust-design.md](p1-rust-design.md) section 8.

---

## 3. Reading guide

All links are relative to this folder.

| Document | What it contains |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Part 1 file table: 41 files with LOC, every declared name, purpose, port decision (Port, Rewrite, Replace-with-crate, Drop), target module and task ids. |
| [p1-topic-history-resume-and-remove.md](p1-topic-history-resume-and-remove.md) | Mental model for history rows: one row per entry, reset versus hard delete, resume algorithm and its known defects, add-to-library asymmetries. Read this before the behaviour file. |
| [p1-topic-stats-and-storage-aggregation.md](p1-topic-stats-and-storage-aggregation.md) | Which population each statistic uses (library, all history, download cache), counting rules, mean-of-means, global-update approximation, size formatting, failure behaviour. |
| [p1-behavior.md](p1-behavior.md) | Full behaviour specification for History (section 1), Updates feed presentation (section 2), Updates tab (section 3), history wrapper (section 4), Stats (section 5), Storage (section 6), shared helpers (section 7), concurrency (section 8), interactions (section 9), Android-only drops (section 10). |
| [p1-data-and-config.md](p1-data-and-config.md) | Domain model fields and types, enums and stored values, preference keys with defaults and getter locations, tables and views touched, write semantics, constants with exact values, concurrency facts. |
| [p1-rust-design.md](p1-rust-design.md) | Part 1 Rust design: crate and module layout, threading, key types, SQL plan, error handling, mockup migration steps, date formatting, Android to Linux mapping, dropped behaviour, pitfalls. |
| [p1-ui-parity.md](p1-ui-parity.md) | Widget-level parity for History, Updates, Stats and Storage: anatomy, states, dialogs, gestures, keyboard, accessibility, Linux platform notes, text inventory. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases EC-42-H, EC-42-U, EC-42-S, EC-42-T with the required Kurobayomi behaviour; test list; fixtures; failure-mode matrix; race conditions; product decisions. |
| [p1-tasks.md](p1-tasks.md) | 62 tasks, T-42a-001 to T-42a-062 (P0 history foundations, P1 updates, stats, storage, P2 polish, P3 drops). |
| [p2-inventory.md](p2-inventory.md) | Part 2 file table: 18 Updates and Upcoming files, declared names, port decisions, target modules, shared dependencies outside the scope with their owning areas. |
| [p2-behavior.md](p2-behavior.md) | Full behaviour contract for the Updates load pipeline (section 1), date grouping (2), download actions (3), read, seen, bookmark, fillermark and delete (4), selection engine (5), date formatting (6), Upcoming lists and calendar (7, 8), layout breakpoints (9), library update trigger (10), tab wiring (11), row presentation (12), navigation (13), Android-only decisions (14). |
| [p2-data-and-config.md](p2-data-and-config.md) | Updates and Upcoming models, enum values, `updatesView` and `animeupdatesView` SQL, Upcoming queries, preference keys, new Rust settings keys, localisation keys, data flow. |
| [p2-rust-design.md](p2-rust-design.md) | Part 2 Rust design: module tree, feed types, store, selection engine port, grouping, Upcoming model and store, calendar math, relative date, actions, threading, GTK widget choices, keyboard, public API for other areas. |
| [p2-ui-parity.md](p2-ui-parity.md) | Updates and Upcoming widget contracts, states, snackbars, pull-to-refresh replacement, responsive rules, keyboard and focus, icon mapping, typography tokens, mockup gap analysis. |
| [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md) | Edge cases for the updates feed and upcoming; pure-function test vectors (selection, calendar, relative date, grouping, progress format, last-updated); race conditions; failure modes; test list; acceptance. |
| [p2-tasks.md](p2-tasks.md) | 46 tasks, T-42b-001 to T-42b-046 (Updates implementation, Upcoming, tests, drops). |
| [../TASKS.csv](../TASKS.csv) | Machine-readable export of every task in the plans folder. Area 42 rows are the 108 rows whose file starts with this folder name. |

---

## 4. Key design decisions and open questions

Each item lists the decision, the recommendation, and whether it needs the user's call. Items marked **User call** block implementation of the affected task.

### 4.1 Decisions where the part docs already agree

| ID | Decision | Recommendation | Evidence |
|---|---|---|---|
| D-1 | Ownership of History, Stats, Storage | Area 42 owns all five screens. Part 2's ownership note is stale. | section 1.3 above |
| D-4 | History row model | One row per entry: the unit with the maximum `last_read` / `last_seen`. Rows with timestamp 0 are hidden. Remove writes 0 and keeps the row. | [p1-topic-history-resume-and-remove.md](p1-topic-history-resume-and-remove.md) section 1; [p1-behavior.md](p1-behavior.md) 1.1 |
| D-5 | Local-time date grouping | Use the local zone for all headers (`chrono::Local`), never UTC. | [p2-rust-design.md](p2-rust-design.md) section 10 pitfall 1 |
| D-7 | Search matching | Substring with `instr(lower(title), lower(?))`, not `LIKE`. `%` and `_` become literal. Debounce 250 ms (the Kotlin has no debounce). | [p1-behavior.md](p1-behavior.md) 1.1 and 1.2; EC-42-H-02 |
| D-8 | Millisecond units | Every timestamp and duration in this area is milliseconds, including the anime columns named `total_seconds` and `last_second_seen`. Rust names: `total_ms`, `last_ms`. | [p1-data-and-config.md](p1-data-and-config.md) section 1.2; [p2-rust-design.md](p2-rust-design.md) section 10 pitfall 2 |
| D-9 | Stats are a snapshot | Compute once per page open. Each section is computed independently; a failing section shows "N/A" and is logged. Kotlin leaves the page on Loading forever instead. | [p1-topic-stats-and-storage-aggregation.md](p1-topic-stats-and-storage-aggregation.md) section 7 |
| D-10 | Download cache fan-out | Use `tokio::sync::broadcast` for cache changes and `tokio::sync::watch` for the initializing flag. Never a single-consumer channel. Fixes the stuck Storage page. | [p1-rust-design.md](p1-rust-design.md) section 10 pitfall 1 and 2; task T-42a-009 |
| D-11 | Library update trigger | "Update library" button plus F5. Show a real spinner for the job's real duration. Remove the Kotlin fake 1-second spinner. | [p1-behavior.md](p1-behavior.md) 2.9; [p2-behavior.md](p2-behavior.md) section 10 |
| D-12 | Selection gestures | Long press selects a range from the anchor. Ctrl toggles, Shift extends, Escape clears. Escape goes to selection first, then to navigation. | [p2-rust-design.md](p2-rust-design.md) section 7; [p1-rust-design.md](p1-rust-design.md) section 10 pitfall 11 |
| D-13 | Haptics and Android splash | Dropped. No user-visible loss on desktop. | [p1-rust-design.md](p1-rust-design.md) section 9 |
| D-14 | Storage size units | Decimal (1000), two decimals, `.` as separator in every locale. | [p1-behavior.md](p1-behavior.md) 6.7; EC-42-T-10 |

### 4.2 Decisions where the part docs conflict (Proposal: resolve as stated)

| ID | Conflict | Recommendation |
|---|---|---|
| D-2 | Module layout. Part 1 uses `crates/kuro-domain`, `kuro-data`, `src/ui/...`. Part 2 uses a flat `src/updates/`, `src/upcoming/`, `src/time_fmt.rs`. | Adopt the part 1 workspace layout (section 2.2). Map part 2 names onto it. Part 2 file paths in [p2-rust-design.md](p2-rust-design.md) section 1 are superseded. |
| D-3 | Relative time formatting. Part 1 (`format::relative_time_span`, [p1-rust-design.md](p1-rust-design.md) section 7) says `< 60 min` "N minutes ago", `< 24 h` "N hours ago", `< 48 h` "Yesterday", `< 7 d` "N days ago". Part 2 ([p2-behavior.md](p2-behavior.md) 6.5, [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md) 3.7) says under 60 s "Just now", then "N minutes ago", "N hours ago", "N days ago", with singular forms in the test table. | One function in `kuro-domain::format`. Rules: under 60 s "Just now"; under 60 min "N minute(s) ago"; under 24 h "N hour(s) ago"; 24 to under 48 h "Yesterday"; under 7 d "N day(s) ago"; otherwise absolute date. Singular and plural as separate i18n keys. Update the part 2 test vectors to match, in the same change. |
| D-15 | Row list widget. Part 1 says `gtk::ListView` with `SignalListItemFactory`. Part 2 pitfall 7 says `gtk::ListBox` is fine up to about 2000 rows. | Use `gtk::ListView` everywhere. It is virtualised, and headers become a non-selectable row type. Part 2 pitfall 7 is superseded. |
| D-16 | Transient messages. Part 1 says `gtk::Overlay` plus a toast label. Part 2 says a `gtk::Revealer` at the bottom. | One shared toast component, owned by T-42b-039, queued, 4 s duration (Kotlin `SnackbarDuration.Short` is 4000 ms per [p2-ui-parity.md](p2-ui-parity.md) 1.7). Part 1 tasks consume it. |

### 4.3 Open questions that need the user's call

| ID | Question | Evidence | Recommendation |
|---|---|---|---|
| **D-6** (User call) | Updates download indicator "Delete" on a downloaded row runs without confirmation (`ChapterDownloadIndicator` long-press menu, `DELETE` goes straight to `deleteChapters`). | [p1-behavior.md](p1-behavior.md) 2.7; EC-42-U-08 | Keep parity, but add a confirm dialog on the per-row path. Bulk delete already confirms. Data loss is irreversible. |
| **D-17** (User call) | Migrate from history: manga replaces the stack with the new entry's screen, anime with the old entry's screen. | [p1-behavior.md](p1-behavior.md) 1.10; EC-42-H-17 | Use the new entry for both kinds. Confirm with user. |
| **D-18** (User call) | Clear all history is a hard delete (`DELETE FROM history`), which also erases `time_read` and so drops "Read duration" to 0. | [p1-behavior.md](p1-behavior.md) 1.9 and 5.2 | Keep the hard delete (it is what the user asked for) and keep the confirm text "All history will be lost." Show a toast on failure (Kotlin is silent). |
| **D-19** (User call) | Next-unit defect. When the history unit is not in the sorted list, Kotlin opens the second unit. | EC-42-H-05; [p1-topic-history-resume-and-remove.md](p1-topic-history-resume-and-remove.md) section 3 | Fix: open the first unit. Record as a deliberate deviation. |
| **D-20** (User call) | Reselect on the History tab always resumes the anime list, even on the manga sub-tab. The manga resume channel is never sent to. | EC-42-H-08; [p1-behavior.md](p1-behavior.md) 1.7 | Fix: reselect acts on the visible sub-tab. |
| **D-21** (User call) | Storage empty state. Kotlin shows only the category select and the chart, with no message when the list is empty. | EC-42-T-03; [p1-behavior.md](p1-behavior.md) 6.6 | Add "No downloads for this category" text. Low risk. |
| **D-22** (User call) | Local-source delete copy. The dialog "Delete downloaded chapters?" is misleading for local entries, which are files on disk the user owns. | EC-42-T-14 | Use distinct copy for local entries. Confirm wording. |
| **D-23** (User call) | Download count. Stats count every directory in the download cache, including orphans, not just library entries. | EC-42-S-12; [p1-topic-stats-and-storage-aggregation.md](p1-topic-stats-and-storage-aggregation.md) section 1 | Keep the Kotlin population and label it "Downloaded". Offer "in library" as a later option. |
| **D-24** (User call) | Week start for Upcoming. Kotlin uses the locale. Proposal table: Sunday for US, CA, JP, IL, BR, IN, KR, PH, MX, TW, ZA, SA, HK; Saturday for AF, IR; Monday otherwise. | [p2-rust-design.md](p2-rust-design.md) section 8.2; [p2-data-and-config.md](p2-data-and-config.md) section 4.1 | Add setting `upcoming.week_start` = `auto` \| `monday` \| `sunday`, default `auto`. |
| **D-25** (User call) | Updates window and limit. Kotlin hard-codes 3 months and 500 rows. | [p1-data-and-config.md](p1-data-and-config.md) section 5 | Keep 3 months and 500 as defaults. Expose the window as `updates.window_months` (default 3) only if the user wants it. |
| **D-26** (User call) | Duplicate Updates tasks across the two parts (section 6.3). | section 6.3 | Pick one owner per capability before implementation starts. Recommendation is in section 6.3. |

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What it provides to area 42 | Used by |
|---|---|---|
| `area:01-foundation-app-bootstrap` | Crate skeleton, `Services` struct (constructor injection replaces Injekt), tokio runtime | T-42a-001 |
| `area:02-preferences-system` | Typed preference store and `watch`-style state; keys `relative_time_v2`, `app_date_format`, `bottom_rail_nav_style`, `library_update_last_timestamp`, `library_unread_updates_count`, `library_unseen_updates_count`, `default_category`, `default_anime_category`, `hidden_categories`, `library_update_categories`, `animelib_update_categories` (and `_exclude` variants), `library_update_manga_restriction`, `pref_always_use_external_player`, `use_external_downloader` | T-42a-002, T-42a-010, T-42a-039 |
| `area:03-db-manga-schema-migrations`, `area:04-db-anime-schema-migrations` | Tables `history`, `animehistory`, `chapters`, `episodes`, `mangas`, `animes`, views `historyView`, `animehistoryView`, `updatesView`, `animeupdatesView`, the `UNIQUE(chapter_id)` and `UNIQUE(episode_id)` constraints | T-42a-011, T-42a-012 |
| `area:05-data-repos-entries-items`, `area:07-domain-entries-items` | `GetManga`, `GetAnime`, `UpdateManga.awaitUpdateFavorite`, `GetChaptersByMangaId`, `GetEpisodesByAnimeId`, `getChapterSort`, `getEpisodeSort`, `SetReadStatus`, `SetSeenStatus`, `UpdateChapter`, `UpdateEpisode` | T-42a-015, T-42a-019, T-42b-011, T-42b-012 |
| `area:06-data-repos-history-track-source-misc` | History and updates repositories, `subscribeToList` reactivity, `getReadDuration` | T-42a-011, T-42a-012 |
| `area:08-domain-library-category-history-updates` | Categories (`GetMangaCategories`, `SetMangaCategories`, `Category`, `UNCATEGORIZED_ID = 0`), library rows (`GetLibraryManga`, `GetLibraryAnime`) | T-42a-001, T-42a-017, T-42a-047 |
| `area:14-source-local-and-metadata` | `isLocal()` for local entries, local source folder size | T-42a-040, T-42a-041, T-42a-047 |
| `area:17-source-app-layer` | `MangaSourceManager.get`, `getOrStub` | T-42a-019, T-42b-010 |
| `area:23-downloads-manga`, `area:24-downloads-anime`, `area:25-downloads-shared` | `getQueuedDownloadOrNull`, `isChapterDownloaded`, `isEpisodeDownloaded`, `statusFlow`, `progressFlow`, `queueState`, `cache.changes`, `cache.isInitializing`, `getDownloadCount`, `getDownloadSize`, `deleteChapters`, `deleteEpisodes`, `startDownloads`, `startDownloadNow`, `cancelQueuedDownloads` | T-42a-009, T-42a-029, T-42a-031, T-42a-051, T-42b-008, T-42b-009, T-42b-010 |
| `area:26-library-update-jobs` | `startNow(app): Boolean`, `lastUpdatedTimestamp`, new-update counters, the auto-update restriction tokens | T-42a-033, T-42b-015, T-42b-016, T-42b-041 |
| `area:27-tracking-core`, `area:28-tracker-anilist` to `area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin` | `GetMangaTracks`, `GetAnimeTracks`, `TrackerManager.loggedInTrackers()`, `get10PointScore`, `bindEnhancedTrackers` | T-42a-040, T-42a-041, T-42a-042, T-42a-019 |
| `area:34-images-covers-cache` | Cover loading for `gtk::Picture`, `MangaCover` / `AnimeCover` model | T-42a-014, T-42a-050, T-42b-022 |
| `area:36-ui-shell-navigation-security-webview` | `gtk::Stack` navigation, `NavStyle` enum, badge counts on the sidebar, Escape routing, `HomeScreen.openTab` equivalent | T-42a-007, T-42a-021, T-42b-037 |
| `area:37-ui-library` | Library predicate used by the storage and stats population | T-42a-047 |
| `area:38-ui-entry-details-manga`, `area:39-ui-entry-details-anime` | Entry screens (`MangaScreen`, `AnimeScreen`), duplicate and change-category dialogs, migration dialogs, `EntryBottomActionMenu`, download indicators | T-42a-019, T-42a-020, T-42b-024, T-42b-034 |
| `area:40-ui-browse-manga`, `area:41-ui-browse-anime` | Migration screen (`MigrateSeasonSelectScreen`) reached from history | T-42a-020 |
| `area:43-reader-core`, `area:44-reader-viewers` | `reader::open(manga_id, chapter_id)` (replaces `ReaderActivity.newIntent`) | T-42a-016, T-42b-018 |
| `area:45-player-core`, `area:46-player-controls-ui` | `player::open(entry, unit, external)` (replaces `MainActivity.startPlayerActivity`) | T-42a-016, T-42a-035, T-42b-018 |
| `area:51-i18n` | Message keys (all `MR.strings.*` and `AYMR.strings.*` keys listed in the part docs) | T-42a-062 |
| `area:52-util-app` | `formatChapterNumber`, `formatEpisodeNumber`, the shared `DateExtensions` utilities | T-42a-003, T-42a-016 |
| `area:35-data-misc-updater-notification-export` | Library-update notifications (not reimplemented here) | none in this area |

### 5.2 What other areas need from this area

| Consumer | What it needs from area 42 |
|---|---|
| `area:36-ui-shell-navigation-security-webview` | Updates and history badge reads (`library_unread_updates_count`, `library_unseen_updates_count`), the Updates and History page builders, `NavStyle` |
| `area:38-ui-entry-details-manga`, `area:39-ui-entry-details-anime` | `kuro-domain::format::relative_date` and `relative_time_span` (the release-date text on entry pages uses the same helpers; [p2-behavior.md](p2-behavior.md) 6.4 notes the `LocalDateTime` variant is used by other areas), the selection engine if those screens adopt range selection |
| `area:43-reader-core`, `area:44-reader-viewers` | Writes history rows (`history.upsert`, `time_read` accumulation), so history display depends on them. Area 42 only reads. |
| `area:45-player-core` | Writes `animehistory.last_seen`. Area 42 only reads. |
| `area:21-backup-format-and-create`, `area:22-backup-restore` | History and updates fields in backup payloads (`last_read`, `time_read`, `last_second_seen`). Area 42 defines what these fields mean. |
| `area:26-library-update-jobs` | Read-only reference for the restriction tokens. Area 42 does not write to the job. |

---

## 6. Task summary

### 6.1 Counts

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then the rows of `TASKS.csv` whose file starts with `42-ui-updates-history-stats-storage-upcoming/`. Lint: both `p1-tasks.md` and `p2-tasks.md` print `ok`. Export of the full plan folder: 3,801 tasks, 0 errors.

| Source file | Tasks | Id range |
|---|---|---|
| [p1-tasks.md](p1-tasks.md) | 62 | T-42a-001 to T-42a-062 |
| [p2-tasks.md](p2-tasks.md) | 46 | T-42b-001 to T-42b-046 |
| **Total** | **108** | |

| Priority | Count | Effort S | Effort M | Effort L | Effort XL |
|---|---|---|---|---|---|
| P0 | 18 | 8 | 10 | 0 | 0 |
| P1 | 65 | 36 | 28 | 1 | 0 |
| P2 | 17 | 12 | 5 | 0 | 0 |
| P3 | 8 | 8 | 0 | 0 | 0 |
| **Total** | **108** | **64** | **43** | **1** | **0** |

The only L task is T-42a-019 (favorite from history). No task is XL. Area 42 has no task above 1.5 weeks.

### 6.2 P0 critical path (in order)

P0 means "needed so Kurobayomi is usable daily for reading history and resuming". The P0 set is the history foundation and the tab shell. It does not include Updates, Stats or Storage.

| Step | Task | Effort | Depends on | Notes |
|---|---|---|---|---|
| 1 | T-42a-001 kuro-domain module skeleton | S | area:01, area:08 | Everything else waits for it |
| 2 | T-42a-011 Manga history list query | M | area:03, area:06, T-42a-001 | `historyView` filter `readAt > 0 AND maxReadAtChapterId = chapterId` |
| 3 | T-42a-012 Anime history list query | S | area:04, area:06, T-42a-011 | Mirrors step 2 |
| 4 | T-42a-005 Date-grouped list model | S | T-42a-001 | Runs in parallel with steps 2 and 3 |
| 5 | T-42a-009 Download-cache signal fan-out | M | area:23, area:24, area:25 | Runs in parallel; see risk R-2 |
| 6 | T-42a-013 History view model | M | T-42a-011, T-42a-012, T-42a-005, T-42a-009 | Gate for rows and states |
| 7 | T-42a-014 History row widget (96 px) | M | T-42a-013, area:34 | |
| 8 | T-42a-008 Grouped list primitives | S | T-42a-005 | Parallel with step 6 |
| 9 | T-42a-022 Empty, loading and no-results states | S | T-42a-008, T-42a-013 | |
| 10 | T-42a-015 Next-unit algorithm | M | T-42a-001, area:07 | Runs in parallel with steps 2 to 9 |
| 11 | T-42a-016 Resume action, open unit and snackbars | M | T-42a-015, area:43, area:45, area:36 | Depends on other areas' reader and player |
| 12 | T-42a-007 Tabbed page shell with per-sub-tab search | M | T-42a-001, area:36 | |
| 13 | T-42a-021 History tab shell and reselect behaviour | S | T-42a-007, T-42a-016, area:36 | Last P0 gate |
| 14 | T-42a-017 Remove history (single row, whole entry, clear all) | M | T-42a-011, T-42a-012 | Parallel with step 13 |

The longest chain is steps 1, 2, 3, 6, 7, 9. Steps 10, 11 and 13 form a second chain that crosses into the reader and player areas. Steps 2 to 9 cannot start until area:03, area:04 and area:06 have their schema and repositories, so the P0 date is set by those areas.

P0 totals: 10 M and 8 S tasks. Remaining P0 tasks not listed above: T-42a-002 (relative date text, M), T-42a-003 (duration and progress formatting, S), T-42a-006 (selection engine, M), T-42a-010 (preference bindings, S). These are all needed before the rows show correct headers and times.

### 6.3 Duplicates between the two parts (must be resolved)

Part 1 and part 2 both plan the Updates page. The part 2 document was written against the Kotlin, and the part 1 inventory lists the same Updates presentation files. The task lists overlap:

| Capability | Part 1 task(s) | Part 2 task(s) | Recommended single owner |
|---|---|---|---|
| Selection engine (range select) | T-42a-006 (P0, M) | T-42b-006 (P1, M) | T-42b-006 owns the algorithm. T-42a-006 becomes the shared gesture adapter or is deleted. |
| Date grouping | T-42a-005 (P0, S) | T-42b-005 (P1, S) | T-42a-005 owns the generic list primitive. T-42b-005 consumes it. |
| Relative date text | T-42a-002 (P0, M) | T-42b-017 (P1, S) | T-42a-002 owns it. T-42b-017 is a duplicate. |
| Last-updated header | T-42a-026 (P1, S) | T-42b-016 (P1, S) | T-42b-016 owns it (part 2 has the exact rule). |
| Updates tab shell | T-42a-025 (P1, S) | T-42b-037 (P1, S) | T-42b-037 owns it. |
| Updates rows | T-42a-027, T-42a-028 (P1, M each) | T-42b-022 (P1, M) | T-42b-022 owns it. |
| Download state of a row | T-42a-029 (P1, M) | T-42b-008 (P1, M) | T-42b-008 owns it. |
| Bulk download | T-42a-031 (P1, M) | T-42b-009, T-42b-010 (P1) | T-42b-009 and T-42b-010 own it. |
| Bulk read, bookmark, fillermark | T-42a-030 (P1, M) | T-42b-011, T-42b-012, T-42b-013 (P1) | T-42b-011 to T-42b-013 own it. |
| Bulk delete confirm | T-42a-032 (P1, S) | T-42b-014 (P1, S) | T-42b-014 owns it. |
| Library refresh from Updates | T-42a-033 (P1, S) | T-42b-015 (P1, S), T-42b-041 (P1, S) | T-42b-015 and T-42b-041 own it. |
| Selection-mode page behaviour | T-42a-034 (P1, M) | T-42b-007, T-42b-021 (P1) | T-42b-007 and T-42b-021 own it. |
| Anime play externally or internally | T-42a-035 (P1, S) | T-42b-018 (P1, M) | T-42b-018 owns it. |
| Bottom action bar | T-42a-036 (P1, M) | T-42b-024 (P1, M) | T-42b-024 owns it. |
| Anime quality options | T-42a-037 (P2, S) | T-42b-019 (P2, S) | T-42b-019 owns it. |
| Updates states (loading, empty, error) | T-42a-038 (P1, S) | T-42b-023 (P1, S) | T-42b-023 owns it. |
| Tests | T-42a-060 (P1, M) | T-42b-043, T-42b-044 (P1, M) | Both parts keep their own tests; test ids must not collide. |
| Drop decisions | T-42a-055 to T-42a-059, T-42a-061 (P3) | T-42b-042 (P3) | Keep all; they describe different Android features. |

Proposal: the part 2 tasks own the Updates implementation, because the part 2 behaviour contract ([p2-behavior.md](p2-behavior.md)) is the more exact of the two. The part 1 Updates tasks T-42a-025 to T-42a-038 should be re-scoped or closed as covered. This audit compared task titles only, not acceptance criteria line by line, so each pair needs a check before one side is closed. Task ids are not changed here. Priority changes are also not made here (see R-3). This is decision D-26.

### 6.4 Priority observations

- **T-42a-006 (selection engine) is P0 but only Updates uses it.** History rows have no selection (the row gestures are click, favorite and delete, [p1-behavior.md](p1-behavior.md) 1.4). Proposal: P1.
- **T-42a-013 depends on T-42a-009 (download-cache fan-out).** History rows show no download state. The P0 chain therefore runs through `area:23`, `area:24` and `area:25` for no history feature. Proposal: remove the edge from history; keep T-42a-009 as a dependency of Storage (T-42a-047) only.
- **T-42a-019 (favorite from history) is L and P1.** It is the largest single task in the area. It touches categories, trackers and the enhanced-tracker bind. Proposal: keep, but split at the category decision and the migrate dialog if a single engineer cannot finish it in one pass.

---

## 7. Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R-1 | P0 blocked by other areas: schema and repositories (`area:03`, `area:04`, `area:06`), chapter sort (`area:07`), shell (`area:36`), reader and player (`area:43`, `area:45`), covers (`area:34`), downloads signals (`area:23` to `area:25`). | High | High. History cannot ship without them. | Build the pure `kuro-domain` pieces first (steps 1, 4, 10). Stub the reader and player behind the `open` function signature (T-42a-016). |
| R-2 | Single-consumer channel fan-out. Storage and Updates both consume `downloadCache.changes`. Storage can stay on Loading forever if the signal was consumed before subscription. | Medium | High (silent stuck page) | T-42a-009 broadcast plus watch. Test EC-42-T-01 (two subscribers each receive one change). |
| R-3 | Duplicated and overlapping tasks across the two parts (section 6.3), plus a P0 task that does not need to be P0 (T-42a-006). | High | Medium (double implementation, scope creep) | Resolve D-26 before implementation. |
| R-4 | Conflicting specs on time formatting, module layout, list widget and toast (section 4.2). | High | Medium (rework) | Resolve D-2, D-3, D-15, D-16 first. |
| R-5 | Data loss: clear-all is a hard delete (`time_read` erased). Per-row delete from the download indicator has no confirmation. Storage delete is irreversible. | Medium | High | D-6 and D-18. Keep the existing confirm texts. Add a confirm where D-6 says so. |
| R-6 | Stats and Storage cost on large libraries. Kotlin issues one query per entry (stats N+1) and one disk walk per row per emission (storage). | Medium | Medium (slow page, not wrong numbers) | One aggregate SQL per kind ([p1-rust-design.md](p1-rust-design.md) section 4.4). Size cache keyed by download-cache generation. Parity test against the N+1 reference during migration (T-42a-044). |
| R-7 | Intentional deviations change visible results: history resume (D-19), reselect (D-20), relative time wording (D-3), size formatting separator (D-14). Users comparing with Aniyomi will see differences. | High | Low | Each deviation is listed in the edge-case file with a test. Keep the list. |
| R-8 | Compose preview fixtures do not run in Rust. The six history screen states and the row fixtures must be rebuilt as Rust test data. | Low | Low | T-42a-060 rebuilds them from `app/src/main/java/eu/kanade/presentation/history/manga/MangaHistoryScreenModelStateProvider.kt` and the `WithRelationsProvider` files ([p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 6). |
| R-9 | Unverified Kotlin behaviour. Part 1 notes two facts with "verify" wording: `historyView` usage in `animehistorystatsView` ([p1-data-and-config.md](p1-data-and-config.md) section 4.1), and the storage `Channel` readiness race ([p1-behavior.md](p1-behavior.md) 6.8). | Low | Medium | Verify both with a test before T-42a-054 is closed. |

---

## 8. Definition of done for area 42 in Kurobayomi

Each item is observable in the running app.

### 8.1 History

1. The History tab shows two sub-tabs, Anime first, then Manga. Their order matches [p1-behavior.md](p1-behavior.md) 0.1.
2. Each entry appears once, as its latest read or watched unit. Read units do not appear as separate rows.
3. Rows are sorted newest first. Each date has one header, using the local calendar date. Headers read "Today", "Yesterday", "N days ago" or an absolute date, under the thresholds in D-3.
4. Each row shows the cover, title (up to two lines), and "Ch. N - HH:MM" or "Ep. N - HH:MM" (time only, locale short format). A row without a number shows only the time.
5. Clicking a row opens the next unread unit (manga, reader) or the next unseen unit (anime, player), or shows "Next chapter not found" or "Next Episode not found!" when none is left.
6. Reselect of the History tab resumes the sub-tab that is visible (D-20).
7. Search filters the visible sub-tab by a literal substring of the title, case-insensitively for ASCII. `%` and `_` are plain characters. An empty result shows "No results found" when a query is set, otherwise "Nothing read recently" or "Nothing watched recently".
8. The per-row delete asks for a confirmation with the "Reset all" checkbox. Unchecked removes only that row. Checked removes every row of the entry. Both keep `time_read`, so read-duration stats do not change.
9. "Clear history" asks "Are you sure? All history will be lost." and, on confirm, removes all rows of that kind and shows "History deleted". On failure it shows the internal-error toast.
10. "Add to library" from history respects the default category rule (`default_category` / `default_anime_category`) and the duplicate-check order in [p1-behavior.md](p1-behavior.md) 1.10.

### 8.2 Updates

11. The Updates tab shows two sub-tabs, anime first. The header shows "Library last updated: <relative time>", or "Never", or "Just now".
12. The list shows chapters and episodes of library favorites whose upload date is within the last 3 months and whose fetch date is after the entry was added to the library. At most 500 rows, newest fetch date first, grouped by fetch date ([p1-behavior.md](p1-behavior.md) 2.1).
13. Each row shows the cover, title, unread dot, bookmark and fillermark icons where set, the unit name, and progress ("Page: N" for manga; "m:ss" or "h:mm:ss" for anime, as in `formatProgress`).
14. Long press starts a range selection. Ctrl toggles and Shift extends. Escape clears the selection before navigating back. The bottom action bar shows only the actions whose predicates hold (section 2.6 of [p1-behavior.md](p1-behavior.md)).
15. Bulk actions take effect, clear the selection, and show the right confirm dialog for delete. The per-row download indicator follows the state table in [p1-behavior.md](p1-behavior.md) 2.7.
16. "Update library" starts the library job once. The spinner runs for the real duration of the job. A press while the job is already running starts nothing and shows no spinner (single-flight guard, T-42b-041; [p1-behavior.md](p1-behavior.md) 2.9).
17. The new-updates badge resets to 0 when the page is opened and when it is closed.
18. Anime rows open the player (internal or external per `pref_always_use_external_player`). Manga rows open the reader.

### 8.3 Upcoming

19. The Upcoming page shows a month calendar with a dot count per day (max 3 dots) and a list grouped by day with per-day counts. Days in the past are dimmed; today has a ring.
20. Changing month with the header buttons animates a slide of 200 ms ([p2-behavior.md](p2-behavior.md) 8.2 and `MONTH_YEAR_CHANGE_ANIMATION_DURATION`). Day-jump from the calendar scrolls the list to that day's header.
21. Layout is single-column below 720 px wide and two-panel at or above it (`ui.two_panel_min_width` default 720, [p2-data-and-config.md](p2-data-and-config.md) section 4.1).
22. Entries with `status = COMPLETED (2)` and entries not in favorites never appear. Anime entries with status 7 appear.

### 8.4 Stats

23. Two pages, anime and manga. Each shows Overview, Entries, Episodes or Chapters, and Trackers sections, with the tiles listed in [p1-behavior.md](p1-behavior.md) 5.1.
24. Values match the expected fixture results in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 6.3.
25. Mean score shows `"%.2f ★"` with two decimals and a dot, or "N/A" when there are no scored tracks.
26. A section that fails to compute shows "N/A" and logs the error. The page does not stay on Loading.

### 8.5 Storage

27. Two pages, anime and manga. The category dropdown lists "All", then "Default" for id 0, then the category names. The list resets to "All" on page open.
28. The half-donut shows the total size and one arc per entry, largest first, in a stable colour derived from the entry id.
29. Rows show the cover, title, colour dot, size in decimal units (`B`, `KB`, `MB`, `GB`, two decimals above B), and the count of chapters or episodes.
30. Delete asks "Delete downloaded chapters?" (or episodes) with the entry title in the body. On confirm, the downloads are removed and the row disappears when the cache changes.
31. The page never stays on Loading because of a missed cache signal (R-2).

### 8.6 Tests that must pass (headless, `kuro-domain`)

32. Unit tests named in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 5.1 and [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md) section 6 pass. The vectors in [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md) section 3 are the acceptance values.
33. SQL tests in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 5.2 pass against an in-memory database with the area 03 and 04 schema applied.

---

## 9. Items not in this area (explicit)

- Library update notifications: `area:35-data-misc-updater-notification-export`.
- Chapter and episode release-date text on entry pages: `area:38-ui-entry-details-manga`, `area:39-ui-entry-details-anime` (they use `LocalDateTime.toRelativeString`, [p2-behavior.md](p2-behavior.md) 6.4).
- Download manager internals: `area:23-downloads-manga`, `area:24-downloads-anime`, `area:25-downloads-shared`.
- Backup format of history and updates: `area:21-backup-format-and-create`, `area:22-backup-restore`.
