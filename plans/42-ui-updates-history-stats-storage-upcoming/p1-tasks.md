# Area 42 part 1 — Updates, History, Stats, Storage — implementation tasks

Ids run T-42a-001 to T-42a-062. Scope: the 41 part-1 files listed in `p1-inventory.md`. Every task cites the Kotlin sources it implements (repo-relative under `/home/blackbox/aniyomi-src`). Companion documents: `p1-behavior.md` (algorithms, keys, timings), `p1-data-and-config.md` (models, preferences, SQL), `p1-rust-design.md` (crates, signatures, threading), `p1-ui-parity.md` (screens, dialogs, mockup gaps), `p1-edge-cases-and-tests.md` (EC ids, test list), topic files `p1-topic-history-resume-and-remove.md` and `p1-topic-stats-and-storage-aggregation.md`.

Priority guide for this area: P0 = reading and watching history, resume, the shared list/tab/date/format/cache-signal foundations; P1 = updates feed and actions, favorites from history, stats, storage; P2 = long-tail polish; P3 = Android-only drops with the replacement stated.

## Foundation

### T-42a-001 — kuro-domain module skeleton for area 42
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/UpdatesTab.kt, app/src/main/java/eu/kanade/presentation/more/stats/StatsScreenState.kt, app/src/main/java/eu/kanade/presentation/more/storage/StorageScreenState.kt
- Acceptance:
  - `crates/kuro-domain/src/{history,updates,stats,storage,format}/mod.rs` exist and compile with `cargo build -p kuro-domain` with no GTK or rusqlite dependency
  - `EntryKind::{Manga, Anime}` exposes `history_table()` returning `"history"` / `"animehistory"` and `timestamp_col()` returning `"last_read"` / `"last_seen"`
  - constants `UPDATES_WINDOW_MONTHS = 3`, `UPDATES_LIMIT = 500`, `COMPLETED_STATUS = 2`, `ALL_CATEGORY_ID = -1`, `UNCATEGORIZED_ID = 0`, `FAKE_SPINNER_MS = 1000` (only for the legacy flag, see T-42a-033) are defined once

### T-42a-002 — Relative date text (LocalDate.toRelativeString port)
- Priority: P0
- Effort: M
- Depends: T-42a-001, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt, app/src/main/java/eu/kanade/presentation/components/DateText.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - `format::relative_date(date, today, relative, absolute)` returns Today for difference 0, `N days ago` for 1..=6, upcoming for -1..=-7 (`difference < 0` and `>= -7`), and the absolute date for difference `< -7` or `>= 7`
  - `relative = false` always returns the absolute date
  - `app_date_format = ""` produces the locale short date; a non-empty Java pattern is translated (table in p1-rust-design.md section 7) and an unsupported letter is rejected by the settings validator
  - test `format::relative_date_thresholds` from p1-edge-cases-and-tests.md section 5.1 passes

### T-42a-003 — Duration, progress and relative-time formatting
- Priority: P0
- Effort: S
- Depends: T-42a-001
- Sources: app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt, app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesUiItem.kt
- Acceptance:
  - `format::duration_string(ms, none)` reproduces the four rules of behavior 5.4 (days `%dd`, hours `%dh`, minutes `%dm` when `days == 0 || hours == 0`, seconds `%ds` when `days == 0 && hours == 0`), joined with a space, blank gives `none` ("None")
  - `format::progress_mmss(ms)` uses `H:MM:SS` only when `ms > 3_600_000`, else `M:SS` (test vectors in p1-edge-cases-and-tests.md 5.1 item 12, EC-42-U-14)
  - `format::relative_time_span(epoch, now)` returns Never for `epoch <= 0`, JustNow for `now - epoch < 60_000`, else a minute-resolution English string through the i18n table

### T-42a-004 — Byte size formatting (Long.toSize port)
- Priority: P1
- Effort: S
- Depends: T-42a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/StorageUtil.kt
- Acceptance:
  - `storage::size::format_size` uses 1000-based steps with `f32` division and the literals `"%.2f GB"`, `"%.2f MB"`, `"%.2f KB"`, `"{n} B"`
  - test vectors of EC-42-T-10 pass, including `999_999 -> "1000.00 KB"`

### T-42a-005 — Date-grouped list model (insertSeparators port)
- Priority: P0
- Effort: S
- Depends: T-42a-001
- Sources: app/src/main/java/eu/kanade/core/util/CollectionUtils.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt
- Acceptance:
  - `history::group_by_local_date` and `updates::group_by_fetch_date` return `Header(date)` before the first item and whenever the local date changes between neighbours; items with a null date never create a header
  - history groups by `readAt`/`seenAt`, updates by `dateFetch` (never by upload date)
  - test `history::group_by_local_date_inserts_leading_header` passes

### T-42a-006 — Selection engine with range select
- Priority: P0
- Effort: M
- Depends: T-42a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt, app/src/main/java/eu/kanade/core/util/CollectionUtils.kt
- Acceptance:
  - `updates::selection::Selection::toggle` reproduces behavior 2.5: long press on a first selection sets both bounds; a long press outside the bounds selects the gap exclusive of the existing bound; a tap updates bounds only for the bound row (deselect) or outside the bounds (select)
  - `toggle_all` and `invert` reset bounds to -1
  - bounds are recomputed from the selected flags whenever the item list is replaced (EC-42-U-05)
  - tests `updates::selection_*` in p1-edge-cases-and-tests.md section 5.1 pass

### T-42a-007 — Tabbed page shell with per-sub-tab search
- Priority: P0
- Effort: M
- Depends: T-42a-001, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/components/TabbedScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt
- Acceptance:
  - `ui/tabbed.rs` shows a header title, an optional `gtk::SearchEntry` (enabled per sub-tab), a two-button sub-tab strip and a `gtk::Stack` with slide transition
  - search text of sub-tab 0 (anime) and sub-tab 1 (manga) are independent; the entry shows the text of the visible sub-tab
  - tab label texts match behavior 0.2 (History: "Anime"/"Manga", Updates/Stats/Storage: "Anime"/"Manga")

### T-42a-008 — Grouped list primitives (headers, empty, loading)
- Priority: P0
- Effort: S
- Depends: T-42a-005
- Sources: presentation-core/src/main/java/tachiyomi/presentation/core/components/LazyList.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/ListGroupHeader.kt, presentation-core/src/main/java/tachiyomi/presentation/core/screens/EmptyScreen.kt, presentation-core/src/main/java/tachiyomi/presentation/core/screens/LoadingScreen.kt
- Acceptance:
  - one `gtk::ListView` model holds `enum ListRow { Header(String), Item(T) }`; header rows are not selectable and are not counted by range selection
  - a page state enum `Loading | Empty(message_key) | List` renders the matching child of a `gtk::Stack`
  - keys are `(entry_id, unit_id)` for items and the date for headers

### T-42a-009 — Download-cache signal fan-out (broadcast and watch)
- Priority: P0
- Effort: M
- Depends: area:24-downloads-anime, area:23-downloads-manga, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadCache.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt, app/src/main/java/eu/kanade/tachiyomi/ui/storage/CommonStorageScreenModel.kt
- Acceptance:
  - `CacheSignals.changed` is a `tokio::sync::broadcast` channel: two subscribers each receive every change (test EC-42-T-01)
  - `CacheSignals.initializing` is a `tokio::sync::watch` channel readable by a late subscriber without waiting for a new value
  - no call site in this area consumes a `Channel`-based flow for cache changes

### T-42a-010 — Preference bindings for area 42
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system, area:26-library-update-jobs
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - keys and defaults match p1-data-and-config.md section 3 exactly: `library_update_last_timestamp` (0), `library_unread_updates_count` (0), `library_unseen_updates_count` (0), `hidden_categories` (false), `default_category` (-1), `default_anime_category` (-1), `relative_time_v2` (true), `app_date_format` (""), `bottom_rail_nav_style` (MOVE_HISTORY_TO_MORE), `pref_always_use_external_player` (false), `use_external_downloader` (false)
  - a unit test reads each key from an empty store and asserts the default
  - the reads needed by the screens are exposed as `watch` receivers where the Kotlin uses `asState` (last-updated timestamp, counts)

## History

### T-42a-011 — Manga history list query
- Priority: P0
- Effort: M
- Depends: area:03-db-manga-schema-migrations, area:06-data-repos-history-track-source-misc, T-42a-001
- Sources: data/src/main/sqldelight/view/historyView.sq, data/src/main/sqldelight/data/history.sq, data/src/main/java/tachiyomi/data/history/manga/MangaHistoryRepositoryImpl.kt, data/src/main/java/tachiyomi/data/history/manga/MangaHistoryMapper.kt
- Acceptance:
  - `MangaHistoryRepo::list(query)` returns the rows of behavior 1.1: `readAt > 0`, `maxReadAtChapterId = chapterId`, ordered `readAt DESC`
  - search uses `instr(lower(title), lower(?1)) > 0` (no LIKE wildcards); an empty query returns all rows
  - test EC-42-H-01 (one row per manga), EC-42-H-02 (`_` is literal) pass against an in-memory DB with the area:03-db-manga-schema-migrations schema
  - `latest()` returns the newest row with `readAt > 0` (no max-row filter)

### T-42a-012 — Anime history list query
- Priority: P0
- Effort: S
- Depends: area:04-db-anime-schema-migrations, area:06-data-repos-history-track-source-misc, T-42a-011
- Sources: data/src/main/sqldelightanime/view/animehistoryView.sq, data/src/main/sqldelightanime/dataanime/animehistory.sq, data/src/main/java/tachiyomi/data/history/anime/AnimeHistoryRepositoryImpl.kt
- Acceptance:
  - same contract as T-42a-011 with `seenAt`, `maxSeenAtEpisodeId`, `episodeNumber`
  - `latest()` returns the newest `seenAt > 0` row

### T-42a-013 — History view model (manga and anime)
- Priority: P0
- Effort: M
- Depends: T-42a-011, T-42a-012, T-42a-005, T-42a-009
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryScreenModel.kt
- Acceptance:
  - state is `{ list: Option<Vec<HistoryListItem>>, dialog: Option<HistoryDialog>, search: String }`; `list == None` shows loading
  - the query re-runs on `history` (or `animehistory`) table change (50 ms coalesce) and on search change; a query error emits a toast and keeps the last list
  - search change cancels the previous query task (collectLatest semantics)
  - the two view models share one generic implementation parameterized by `EntryKind`

### T-42a-014 — History row widget
- Priority: P0
- Effort: M
- Depends: T-42a-013, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/history/manga/components/MangaHistoryItem.kt, app/src/main/java/eu/kanade/presentation/history/anime/components/AnimeHistoryItem.kt
- Acceptance:
  - row height 96 px minimum; cover is a book-aspect `gtk::Picture` filling the height, opens the entry on click (not the resume)
  - title bold, max 2 lines, ellipsised
  - subtitle is `Ch. {n} - {time}` (`MR.recent_manga_time`) or `Ep. {n} - {time}` (`AYMR.recent_anime_time`) when the unit number is > -1, else time only; time is local short time without date
  - favorite button shown only when the entry is not in the library; delete button always shown; row click resumes (T-42a-016)
  - test EC-42-H-20 (subtitle rules) passes on fixtures

### T-42a-015 — Next-unit algorithm (GetNextChapters and GetNextEpisodes)
- Priority: P0
- Effort: M
- Depends: T-42a-001, area:07-domain-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetNextChapters.kt, domain/src/main/java/tachiyomi/domain/history/anime/interactor/GetNextEpisodes.kt
- Acceptance:
  - `history::next_units_from(ordered, from_id)` with `onlyUnread = false` returns the current unit first when it is unread, else starts at the following unit
  - when `from_id` is missing the result starts at the FIRST unit (fix of EC-42-H-05); a compatibility flag `legacy_second_unit_on_missing` reproduces Aniyomi's second-unit result and is tested
  - the ordering input is `getChapterSort(manga, sortDescending = false)` / `getEpisodeSort(anime, sortDescending = false)` (area:07-domain-entries-items), applied with the scanlator filter (`applyScanlatorFilter = true`)
  - test `history::next_units_*` in p1-edge-cases-and-tests.md section 5.1 passes

### T-42a-016 — Resume action, open unit and snackbars
- Priority: P0
- Effort: M
- Depends: T-42a-015, area:43-reader-core, area:45-player-core, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt
- Acceptance:
  - row click calls `resume(entry_id, unit_id)`: the first element of `next_units_from` opens the reader (manga, `ReaderActivity.newIntent(context, mangaId, chapterId)` equivalent) or the player (anime, with `alwaysUseExternalPlayer` selecting external vs internal)
  - an empty result shows toast `MR.strings.no_next_chapter` ("Next chapter not found") or `AYMR.strings.no_next_episode` ("Next Episode not found!") for 4 s
  - the "resume last" action (`getNextChapter()` / `getNextEpisode()`) uses the newest history row
  - test EC-42-H-06 and EC-42-H-07 pass

### T-42a-017 — Remove history (single row, whole entry, clear all)
- Priority: P0
- Effort: M
- Depends: T-42a-011, T-42a-012
- Sources: domain/src/main/java/tachiyomi/domain/history/manga/interactor/RemoveMangaHistory.kt, domain/src/main/java/tachiyomi/domain/history/anime/interactor/RemoveAnimeHistory.kt, data/src/main/sqldelight/data/history.sq, data/src/main/sqldelightanime/dataanime/animehistory.sq
- Acceptance:
  - remove one: `UPDATE history SET last_read = 0 WHERE _id = ?` (row kept, `time_read` kept, row disappears from the list)
  - remove entry: `resetHistoryByMangaId` semantics (rust design 4.6)
  - clear all: `DELETE FROM history` for manga and `DELETE FROM animehistory` for anime; manga read duration becomes 0 afterwards (EC-42-H-10)
  - each write runs in one transaction on the writer thread; a failure produces a toast and leaves the list unchanged (EC-42-H-09)

### T-42a-018 — History delete dialogs
- Priority: P1
- Effort: S
- Depends: T-42a-017, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/history/HistoryDialogs.kt, spec/app.json
- Acceptance:
  - the remove dialog shows title "Remove", the kind-specific description (manga: "This will remove the read date of this chapter. Are you sure?"; anime: "This will remove the watch date of this episode. Are you sure?"), a checkbox unchecked by default with the kind-specific reset label, buttons "Remove" and "Cancel"
  - confirm with the checkbox calls the entry reset; without it calls the row reset
  - the spec dialog ids `history_remove_manga` and `history_remove_anime` in `spec/app.json` are bound to these texts; the checkbox is present in the rendered dialog (mockup gap, p1-ui-parity.md 2.4)

### T-42a-019 — Favorite from history (add to library)
- Priority: P1
- Effort: L
- Depends: T-42a-013, area:08-domain-library-category-history-updates, area:05-data-repos-entries-items, area:27-tracking-core, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryTab.kt
- Acceptance:
  - duplicate check first: a duplicate library entry opens the duplicate dialog (add anyway / open existing / migrate)
  - default category: `default_category == -1` with categories → change-category dialog (favorite not yet set); `0` or no categories → favorite with no category; a user category id → favorite plus that category
  - the change-category dialog is pre-checked from the entry's current categories and confirm sets categories and favorite
  - tracker binding (`bindEnhancedTrackers`) runs on the add path (EC-42-H-14); the confirm path also binds (fix of EC-42-H-17b)
  - "edit categories" opens the matching categories page (manga and anime; fix of EC-42-H-18)

### T-42a-020 — Migrate dialog wiring from history
- Priority: P1
- Effort: S
- Depends: T-42a-019, area:40-ui-browse-manga, area:41-ui-browse-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryTab.kt
- Acceptance:
  - manga migrate dialog: title click opens the old entry; on finish the stack replaces with the NEW entry (`MangaScreen(newManga.id)`)
  - anime migrate dialog: season-select opens `MigrateSeasonSelectScreen(old, new)`; on finish the stack replaces with the OLD entry (`AnimeScreen(oldAnime.id)`), as observed (EC-42-H-17)
  - a product decision note is recorded in p1-edge-cases-and-tests.md section 9 item 1

### T-42a-021 — History tab shell and reselect behavior
- Priority: P0
- Effort: S
- Depends: T-42a-007, T-42a-016, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryTab.kt
- Acceptance:
  - sidebar entry "History" with nav index 5 (MOVE_HISTORY_TO_MORE), 3 (MOVE_BROWSE_TO_MORE) or 2 (other), per behavior 0.1
  - reselect resumes the VISIBLE sub-tab (fix of EC-42-H-08): manga page → manga resume; anime page → anime resume
  - the manga resume channel is wired (no dead path)
  - `fromMore` back action: if the previous navigation item is Home, open the Anime library tab, else pop

### T-42a-022 — History empty, loading and no-results states
- Priority: P0
- Effort: S
- Depends: T-42a-008, T-42a-013
- Sources: app/src/main/java/eu/kanade/presentation/history/manga/MangaHistoryScreen.kt, app/src/main/java/eu/kanade/presentation/history/anime/AnimeHistoryScreen.kt
- Acceptance:
  - loading (list None) shows a centred spinner
  - empty with empty query shows "Nothing read recently" (manga) / "Nothing watched recently" (anime)
  - empty with a non-empty query shows "No results found"
  - the empty message keys are `MR.strings.information_no_recent_manga`, `AYMR.strings.information_no_recent_anime`, `MR.strings.no_results_found`

### T-42a-023 — History search behavior
- Priority: P1
- Effort: S
- Depends: T-42a-007, T-42a-011, T-42a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt, app/src/main/java/eu/kanade/presentation/components/TabbedScreen.kt
- Acceptance:
  - search matches the title by substring, case-insensitively for ASCII (EC-42-H-03 decision recorded)
  - the query is debounced 250 ms (proposal; Aniyomi has none); the previous in-flight query is cancelled
  - `updateSearchQuery` and `state.searchQuery` are not implemented (dead path, T-42a-061)

### T-42a-024 — Clear history with confirmation and snackbar
- Priority: P1
- Effort: S
- Depends: T-42a-017, T-42a-007
- Sources: app/src/main/java/eu/kanade/presentation/history/HistoryDialogs.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/anime/AnimeHistoryTab.kt
- Acceptance:
  - header button (icon `edit-clear-all-symbolic`, tooltip "Clear history") opens a dialog titled "Remove everything" with text "Are you sure? All history will be lost." and buttons "OK" / "Cancel"
  - OK clears only the visible kind and shows "History deleted" for 4 s
  - a failure shows the internal-error toast and no "History deleted" message (EC-42-H-09)

## Updates

### T-42a-025 — Updates tab shell
- Priority: P1
- Effort: S
- Depends: T-42a-007, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/UpdatesTab.kt
- Acceptance:
  - sidebar entry "Updates" with nav index 5 (MOVE_UPDATES_TO_MORE), 2 (MOVE_HISTORY_TO_MORE or MOVE_BROWSE_TO_MORE) or 1 (MOVE_MANGA_TO_MORE) per behavior 0.1
  - two sub-tabs (anime first); reselect opens the Downloads page (area:23-downloads-manga, area:24-downloads-anime)
  - no search field on this tab

### T-42a-026 — Updates last-updated header and date groups
- Priority: P1
- Effort: S
- Depends: T-42a-003, T-42a-005, area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesUiItem.kt, app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesUiItem.kt, app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt
- Acceptance:
  - the first row reads "Library last updated: {relative}" (`MR.strings.updates_last_update_info`) and italic
  - relative text is recomputed every 30 s while visible (proposal; Aniyomi computes once per composition)
  - date headers group by `dateFetch` local date

### T-42a-027 — Manga updates row
- Priority: P1
- Effort: M
- Depends: T-42a-026, T-42a-029, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesUiItem.kt, app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesScreen.kt
- Acceptance:
  - row height 56 px; cover square with 6 px vertical padding; title one line (grey when read); unread dot 8 px primary when unread; bookmark icon when bookmarked; chapter name one line
  - progress text `Page: N` with N = `lastPageRead + 1`, shown only when unread and `lastPageRead > 0`
  - cover click and download indicator are active only outside selection mode

### T-42a-028 — Anime updates row
- Priority: P1
- Effort: M
- Depends: T-42a-003, T-42a-026, T-42a-029
- Sources: app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesUiItem.kt, app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesScreen.kt
- Acceptance:
  - same layout as T-42a-027 plus fillermark icon (tertiary colour, label icon) after the bookmark icon
  - watch progress `"{formatProgress(lastSecondSeen)}/{formatProgress(totalSeconds)}"` (`AYMR.strings.episode_progress`) shown only when unseen and `lastSecondSeen > 0`
  - values are milliseconds (EC-42-U-16)

### T-42a-029 — Update row download state
- Priority: P1
- Effort: M
- Depends: area:23-downloads-manga, area:24-downloads-anime, T-42a-009
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt
- Acceptance:
  - state precedence: active queue entry status, else DOWNLOADED when the disk check passes (name, scanlator, entry title, source), else NOT_DOWNLOADED
  - progress is the queue progress or 0
  - status and progress events update only the matching row (by unit id); unknown ids are ignored
  - enum ordinals `NOT_DOWNLOADED=0, QUEUE=1, DOWNLOADING=2, DOWNLOADED=3, ERROR=4` are preserved
  - disk checks are cached per (source, title, name, scanlator) and invalidated on cache signal (rust design 10, pitfall 8)

### T-42a-030 — Bulk bookmark, fillermark and read-state actions
- Priority: P1
- Effort: M
- Depends: T-42a-006, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt
- Acceptance:
  - bookmark: filter rows whose flag already equals the target, then one batch update (`ChapterUpdate` / `EpisodeUpdate`)
  - fillermark (anime only) same pattern
  - mark read/seen: `SetReadStatus` / `SetSeenStatus` with the rows' chapters or episodes, selection cleared on success (EC-42-U-11)
  - all actions end by clearing selection

### T-42a-031 — Bulk download actions
- Priority: P1
- Effort: M
- Depends: area:23-downloads-manga, area:24-downloads-anime, T-42a-029
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt
- Acceptance:
  - START groups rows by entry, skips entries not in the library or whose source is not installed, enqueues the rest; if any row is ERROR it also starts the queue
  - START_NOW and CANCEL require exactly one row; CANCEL removes the queue entry through the queue API (no in-place status mutation, EC-42-U-07)
  - DELETE groups by entry and calls the download manager delete with the installed source
  - the grouped work runs as a detached task (non-cancellable after start)

### T-42a-032 — Bulk delete confirmation dialog
- Priority: P1
- Effort: S
- Depends: T-42a-031, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/updates/UpdatesDialog.kt, spec/app.json
- Acceptance:
  - text "Are you sure you want to delete the selected chapters?" (manga) or "...episodes?" (anime), buttons "OK" and "Cancel", no title
  - spec ids `updates_delete_chapters` and `updates_delete_episodes` bound to the same text
  - the bulk bar delete always confirms; the per-row menu delete does not (parity, EC-42-U-08, decision in p1-edge-cases-and-tests.md section 9)

### T-42a-033 — Library refresh from updates
- Priority: P1
- Effort: S
- Depends: area:26-library-update-jobs, T-42a-010
- Sources: app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesScreen.kt, app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt
- Acceptance:
  - header button `view-refresh-symbolic` "Update library" starts the library job; while running the button is a spinner until the job's completion signal (replaces the fake 1 s spinner, EC-42-U-10)
  - a refused start (already running) shows a toast and no spinner (EC-42-U-09)
  - disabled while selection is active

### T-42a-034 — Updates selection-mode page behavior
- Priority: P1
- Effort: M
- Depends: T-42a-006, T-42a-030
- Sources: app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesScreen.kt, app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesScreen.kt
- Acceptance:
  - entering selection mode: long press a row; tap toggles while selection mode is active; Ctrl+click toggles, Shift+click range selects (proposal)
  - Escape clears the selection before any navigation (EC-42-U-23)
  - in selection mode the cover tap and the download indicator are disabled
  - pull or header refresh is disabled

### T-42a-035 — Anime updates play externally or internally
- Priority: P1
- Effort: S
- Depends: T-42a-028, area:45-player-core, T-42a-010
- Sources: app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - with exactly one row selected the bar shows "Play externally" when `alwaysUseExternalPlayer == false`, else "Play internally"; both call the open-episode action with `alt = true` (EC-42-U-20)
  - the open-episode action is provided by area:45-player-core

### T-42a-036 — Updates bottom action bar
- Priority: P1
- Effort: M
- Depends: T-42a-030, T-42a-031, T-42a-032
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt, app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesScreen.kt, app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesScreen.kt
- Acceptance:
  - visible when selection is non-empty; each action appears only when its predicate of behavior 2.6 holds (bookmark, remove bookmark, fillermark, remove fillermark, mark viewed/read, mark unviewed/unread, download, delete, play externally, play internally)
  - labels use the keys of p1-ui-parity.md section 6; icons are the GTK symbolic equivalents of the Material icons listed in the same section
  - a count label "N selected" and a close button (clears selection) sit at the left (proposal)

### T-42a-037 — Anime show-qualities dialog trigger
- Priority: P2
- Effort: S
- Depends: T-42a-031, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt
- Acceptance:
  - long press on a NOT_DOWNLOADED anime row opens the quality chooser with `episodeTitle`, `episodeId`, `animeId`, `sourceId`
  - the chooser content is owned by area:24-downloads-anime; this task only routes the event

### T-42a-038 — Updates loading, empty and error states
- Priority: P1
- Effort: S
- Depends: T-42a-013, T-42a-008
- Sources: app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt
- Acceptance:
  - loading until the first emission; empty shows "No recent updates" (`MR.strings.information_no_recent`)
  - a query error sets loading to false and shows an error row with a retry (fix of EC-42-U-03)
  - the list is limited to 500 rows and shows a footer "Showing latest 500" when the count equals 500 (EC-42-U-02, proposal)

## Stats

### T-42a-039 — Stats data model
- Priority: P1
- Effort: S
- Depends: T-42a-001
- Sources: app/src/main/java/eu/kanade/presentation/more/stats/data/StatsData.kt, app/src/main/java/eu/kanade/presentation/more/stats/StatsScreenState.kt
- Acceptance:
  - Rust types `MangaStats`, `AnimeStats`, `Overview`, `Titles`, `Counts`, `TrackerStats` with the fields and types of p1-data-and-config.md section 1.7
  - `mean_score: Option<f64>` (None = NaN)
  - `StatsState::{Loading, Manga, Anime}`

### T-42a-040 — Manga statistics aggregation
- Priority: P1
- Effort: M
- Depends: T-42a-039, area:08-domain-library-category-history-updates, T-42a-042, T-42a-044
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/stats/manga/MangaStatsScreenModel.kt
- Acceptance:
  - in-library count, completed count (status 2 and zero unread, empty completed entries counted), started count, local count, chapters total/read, downloads total, global-update count (include/exclude categories and restriction tokens), tracked count, tracker count
  - read duration is `SUM(history.time_read)` over all history (EC-42-S-01)
  - test vectors of p1-edge-cases-and-tests.md sections 5.1 and 6.3 pass

### T-42a-041 — Anime statistics aggregation
- Priority: P1
- Effort: M
- Depends: T-42a-039, area:08-domain-library-category-history-updates, T-42a-042, T-42a-044
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/stats/anime/AnimeStatsScreenModel.kt
- Acceptance:
  - watch time sums `total_ms` when seen else `last_ms`, over library anime only (EC-42-S-03)
  - episode totals and seen totals, started (seen > 0), completed (status 2 and zero unseen), local, global-update count
  - single aggregate SQL statement (rust design 4.4) with a parity test against the N+1 reference implementation

### T-42a-042 — Tracker mean score
- Priority: P1
- Effort: M
- Depends: area:27-tracking-core, area:28-tracker-anilist, area:29-tracker-myanimelist, area:30-tracker-kitsu, area:31-tracker-shikimori-bangumi, area:32-tracker-mangaupdates-simkl
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/stats/manga/MangaStatsScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/stats/anime/AnimeStatsScreenModel.kt
- Acceptance:
  - only logged-in trackers of the matching kind count; tracks with score `> 0.0` are scored
  - mean is the average over entries of each entry's average of 10-point scores (EC-42-S-08); no scored tracks gives None
  - a missing tracker service is a per-section error, not a panic (EC-42-S-06)
  - display `"%.2f ★"` with `.` decimal, else "N/A"

### T-42a-043 — Statistics page UI
- Priority: P1
- Effort: M
- Depends: T-42a-039, T-42a-007, T-42a-008
- Sources: app/src/main/java/eu/kanade/presentation/more/stats/AnimeStatsScreenContent.kt, app/src/main/java/eu/kanade/presentation/more/stats/MangaStatsScreenContent.kt, app/src/main/java/eu/kanade/presentation/more/stats/components/StatsItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/stats/StatsTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/stats/anime/AnimeStatsTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/stats/manga/MangaStatsTab.kt
- Acceptance:
  - four cards in order: Overview, Entries, Episodes/Chapters, Trackers, each a heading and a row of equal tiles
  - captions and values exactly as p1-ui-parity.md section 4.2; "None" for zero duration
  - spinner while Loading; per-section "N/A" on error
  - mockup literal arrays and the string-replace hacks of `src/screens.rs` are removed (p1-ui-parity.md 4.5)

### T-42a-044 — Stats performance and error isolation
- Priority: P1
- Effort: M
- Depends: T-42a-040, T-42a-041
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/stats/anime/AnimeStatsScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/stats/manga/MangaStatsScreenModel.kt
- Acceptance:
  - one aggregate query per kind instead of per-entry queries (EC-42-S-03 and rust design 4.4)
  - each section is computed in its own task; a failure is logged with `tracing` and shown as N/A (EC-42-S-06)
  - the library of 2,000 entries computes in under 1 s on the test fixture (perf test, ignored on CI)

### T-42a-045 — Stats download count
- Priority: P1
- Effort: S
- Depends: T-42a-040, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - downloaded count is the total number of chapter (episode) directories across all source directories of the cache, as in Aniyomi (EC-42-S-12); the label says "Downloaded"
  - the value comes from the cache total, not from a library query

## Storage

### T-42a-046 — Storage data model and colour
- Priority: P1
- Effort: S
- Depends: T-42a-001
- Sources: app/src/main/java/eu/kanade/presentation/more/storage/StorageItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/storage/CommonStorageScreenModel.kt
- Acceptance:
  - `StorageRow { entry_id, title, size_bytes, thumbnail_url, entries_count, color, category_id }`
  - colour is a deterministic function of the entry id, three bytes, alpha 255 (EC-42-T-06)

### T-42a-047 — Storage aggregation
- Priority: P1
- Effort: M
- Depends: T-42a-046, T-42a-009, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/storage/CommonStorageScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/storage/manga/MangaStorageScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/storage/anime/AnimeStorageScreenModel.kt, app/src/main/java/eu/kanade/presentation/more/storage/StorageScreenState.kt
- Acceptance:
  - rows deduplicated by entry id, filtered by the selected category (All keeps only visible categories), sorted by size descending with a stable sort (EC-42-T-05)
  - the hidden-categories setting is read at open (EC-42-T-02 decision recorded)
  - rebuilt once per emission, not per row (EC-42-T-12)
  - test `storage::build_snapshot_*` passes

### T-42a-048 — Storage category selector
- Priority: P1
- Effort: S
- Depends: T-42a-047
- Sources: app/src/main/java/eu/kanade/presentation/more/storage/SelectStorageCategory.kt
- Acceptance:
  - `gtk::DropDown` labelled "Category" (`AYMR.strings.label_category`) with options from the category list; the first option is "All" (id -1); id 0 shows "Default"
  - selected index follows the selected id; changing the selection updates the snapshot

### T-42a-049 — Donut chart
- Priority: P2
- Effort: M
- Depends: T-42a-047, T-42a-004
- Sources: app/src/main/java/eu/kanade/presentation/more/storage/CumulativeStorage.kt
- Acceptance:
  - `donut_arcs` returns sweeps summing to 180 degrees for a non-zero total, none for zero, skipping zero-size arcs (EC-42-T-08 decision)
  - drawing: half ring, 15 px stroke, round caps, rotated 180 degrees, colours per row
  - centre-top text is `format_size(total)`, "0 B" when empty
  - test `storage::donut_*` passes

### T-42a-050 — Storage entry row
- Priority: P1
- Effort: S
- Depends: T-42a-046, T-42a-004, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/more/storage/StorageItem.kt
- Acceptance:
  - 48 px high square cover; bold title one line; second line has a 12 px colour dot, size text, separator dot and count text (`MR.plurals.manga_num_chapters` or `AYMR.plurals.anime_num_episodes`)
  - trailing delete button (tooltip "Delete") opens the confirm dialog

### T-42a-051 — Storage delete
- Priority: P1
- Effort: S
- Depends: T-42a-050, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/storage/manga/MangaStorageScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/storage/anime/AnimeStorageScreenModel.kt, app/src/main/java/eu/kanade/presentation/more/storage/StorageItem.kt
- Acceptance:
  - dialog title "Delete downloaded chapters?" / "Delete downloaded episodes?", text `Are you sure you wish to delete "{title}"?`, buttons "OK" / "Cancel"
  - OK deletes the entry's downloads with its installed source; a missing source shows a toast and deletes nothing (EC-42-T-15)
  - the list refreshes from the cache signal (no optimistic removal)
  - UI copy for local-source entries is reviewed (EC-42-T-14, decision recorded)

### T-42a-052 — Storage page shell and layout
- Priority: P1
- Effort: S
- Depends: T-42a-007, T-42a-048, T-42a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/storage/StorageTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/storage/anime/AnimeStorageTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/storage/manga/MangaStorageTab.kt, app/src/main/java/eu/kanade/presentation/more/storage/StorageScreenContent.kt
- Acceptance:
  - sidebar "Storage" (index 8), two sub-tabs (anime, manga)
  - single column: dropdown, donut, rows; row spacing `medium`
  - loading spinner; a success with no entries shows dropdown and "0 B" only (decision on helper text EC-42-T-03)

### T-42a-053 — Storage two-column layout
- Priority: P2
- Effort: S
- Depends: T-42a-052
- Sources: app/src/main/java/eu/kanade/presentation/more/storage/StorageScreenContent.kt, app/src/main/java/eu/kanade/presentation/util/WindowSize.kt
- Acceptance:
  - width at least 720 px: left column (dropdown and donut) at 2/5, right column (rows) at 3/5, extra padding after the left column
  - narrower: single column with the chart first

### T-42a-054 — Storage readiness without a stuck loading state
- Priority: P1
- Effort: S
- Depends: T-42a-009, T-42a-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/storage/CommonStorageScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadCache.kt
- Acceptance:
  - the first snapshot is produced from `initializing == false` (watch) plus a snapshot on subscribe, so the page never stays loading after the cache is ready (EC-42-T-01)
  - while the cache is initializing the page shows "Scanning downloads…" (proposal text, key added in T-42a-062)

## Shared and tests

### T-42a-055 — Android tablet detection replaced by width breakpoint (P3 drop)
- Priority: P3
- Effort: S
- Depends: T-42a-053
- Sources: app/src/main/java/eu/kanade/presentation/util/WindowSize.kt, app/src/main/java/eu/kanade/presentation/util/FastScrollAnimateItem.kt
- Acceptance:
  - `isTabletUi()` has no port; the 720 px breakpoint of T-42a-053 replaces it
  - the fast-scroll animation helper is not ported (GTK list has no equivalent need)

### T-42a-056 — Voyager navigation and animated tab icons (P3 drop)
- Priority: P3
- Effort: S
- Depends: T-42a-021, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/storage/StorageTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/stats/StatsTab.kt
- Acceptance:
  - Voyager `Screen`, `TabOptions` and `AnimatedImageVector` are not ported; the navigation is area:36-ui-shell-navigation-security-webview's `gtk::Stack`
  - sidebar icons are static symbolic icons; the animation is lost (user-visible: no icon animation on tab selection)

### T-42a-057 — Compose preview providers (P3 drop, fixtures kept)
- Priority: P3
- Effort: S
- Depends: T-42a-060
- Sources: app/src/main/java/eu/kanade/presentation/history/anime/AnimeHistoryScreenModelStateProvider.kt, app/src/main/java/eu/kanade/presentation/history/manga/MangaHistoryScreenModelStateProvider.kt, app/src/main/java/eu/kanade/presentation/history/manga/components/MangaHistoryWithRelationsProvider.kt, app/src/main/java/eu/kanade/presentation/history/anime/components/AnimeHistoryWithRelationsProvider.kt, app/src/main/java/eu/kanade/presentation/history/HistoryDialogs.kt, app/src/main/java/eu/kanade/presentation/more/storage/StorageScreenContent.kt
- Acceptance:
  - the Compose previews are not ported; their data is re-created as deterministic fixtures in T-42a-060 (p1-edge-cases-and-tests.md section 6)

### T-42a-058 — Haptic feedback on long press (P3 drop)
- Priority: P3
- Effort: S
- Depends: T-42a-034
- Sources: app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesUiItem.kt, app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesUiItem.kt
- Acceptance:
  - no haptic call; the long-press gesture (600 ms GTK default) still selects the row

### T-42a-059 — MainActivity ready hand-off (P3 drop)
- Priority: P3
- Effort: S
- Depends: T-42a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/UpdatesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/stats/StatsTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/storage/StorageTab.kt
- Acceptance:
  - no splash hand-off is implemented; the window is shown when the GTK application activates

### T-42a-060 — Fixtures and DB-backed tests for area 42
- Priority: P1
- Effort: M
- Depends: T-42a-011, T-42a-012, T-42a-040, T-42a-041, T-42a-047
- Sources: app/src/main/java/eu/kanade/presentation/history/manga/components/MangaHistoryWithRelationsProvider.kt, app/src/main/java/eu/kanade/presentation/history/manga/MangaHistoryScreenModelStateProvider.kt, app/src/main/java/eu/kanade/presentation/more/storage/StorageScreenContent.kt
- Acceptance:
  - fixtures of p1-edge-cases-and-tests.md section 6 exist in `crates/kuro-data/tests/fixtures/area42.rs` with fixed values (no random data)
  - all 48 tests of sections 5.1 to 5.4 are present; the non-GTK tests run in `cargo test -p kuro-domain -p kuro-data`

### T-42a-061 — Dead-code drops (P3)
- Priority: P3
- Effort: S
- Depends: T-42a-013
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/UpdatesTab.kt, app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/manga/MangaHistoryScreenModel.kt
- Acceptance:
  - `TAB_ANIME`/`TAB_MANGA` constants have no port; `onInvertSelection` in the anime updates screen has no port (invert stays in the model); `updateSearchQuery` and `State.searchQuery` have no port
  - list of drops is recorded in p1-rust-design.md section 9

### T-42a-062 — Dialog copy and i18n keys for area 42
- Priority: P1
- Effort: S
- Depends: area:51-i18n, T-42a-018, T-42a-032
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml, spec/app.json
- Acceptance:
  - every key listed in p1-ui-parity.md section 6 resolves in the Rust i18n table with the English value quoted there
  - new keys added for this area ("Scanning downloads…", "Showing latest 500", "Library update already running", "Source not installed; files not deleted", "No downloads in this category") are marked as additions in the table and are not copied into Aniyomi's files
