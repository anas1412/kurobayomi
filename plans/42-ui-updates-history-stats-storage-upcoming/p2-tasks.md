# Area 42 (part 2): implementation tasks, Updates and Upcoming

Task ids are `T-42b-NNN`. Priorities: P1 = Updates parity features used daily; P2 = Upcoming and polish; P3 = Android-only drop decisions. Scope references: `p2-behavior.md` (B§), `p2-ui-parity.md` (U§), `p2-data-and-config.md` (D§), `p2-rust-design.md` (R§), `p2-edge-cases-and-tests.md` (E-).

Scope note: History, Stats and Storage are not in this area's file list. No task here covers them. Owner must be assigned (see `p2-inventory.md`).

### T-42b-001 — Updates data: manga recent-updates query and row mapping
- Priority: P1
- Effort: M
- Depends: area:05-data-repos-entries-items, area:03-db-manga-schema-migrations, T-42b-002
- Sources: data/src/main/sqldelight/view/updatesView.sq, data/src/main/java/tachiyomi/data/updates/manga/MangaUpdatesRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/updates/manga/model/MangaUpdatesWithRelations.kt
- Acceptance:
  - Query returns rows where `mangas.favorite = 1`, `chapters.date_fetch > mangas.date_added`, `chapters.date_upload > :after`, ordered `date_fetch DESC`, limit 500 (D§3.1, D§3.2 Proposal SQL).
  - Unit test E-U1, E-U2, E-U3, E-U4, E-U5 pass against the fixture in `tests/fixtures/updates_upcoming.sql`.

### T-42b-002 — Updates domain: subscription with 3-month window and limit 500
- Priority: P1
- Effort: S
- Depends: area:07-domain-entries-items, area:08-domain-library-category-history-updates
- Sources: domain/src/main/java/tachiyomi/domain/updates/manga/interactor/GetMangaUpdates.kt, domain/src/main/java/tachiyomi/domain/updates/anime/interactor/GetAnimeUpdates.kt, domain/src/main/java/tachiyomi/domain/updates/manga/repository/MangaUpdatesRepository.kt
- Acceptance:
  - Window cut-off = now minus 3 calendar months, computed once per page construction (B§1.2; R§3.11).
  - Feed emits a new list on chapter/episode/favorite changes and on download-cache or queue changes (B§1.2 combine).

### T-42b-003 — Manga updates page state model
- Priority: P1
- Effort: M
- Depends: T-42b-001, T-42b-002, area:23-downloads-manga, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt
- Acceptance:
  - State `{isLoading=true initially, items, dialog}`; `isLoading` becomes false on first emission (B§1.1, B§1.5).
  - A query error shows the toast "InternalError: Check crash logs for further information" and keeps the feed alive (E-U20, E-U21; deviation from Kotlin recorded).
  - Row download state is derived as active-queue status, else DOWNLOADED if on disk (by name), else NOT_DOWNLOADED (B§1.3).

### T-42b-004 — Anime updates page state model
- Priority: P1
- Effort: M
- Depends: T-42b-001, T-42b-002, area:24-downloads-anime, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt, data/src/main/sqldelightanime/view/animeupdatesView.sq, data/src/main/java/tachiyomi/data/updates/anime/AnimeUpdatesRepositoryImpl.kt
- Acceptance:
  - Same state rules as manga (T-42b-003) using `animeupdatesView`, `episodes`, `AnimeDownload.State` (B§1, D§3.2).
  - `useExternalDownloader` read once at construction (B§1.1).

### T-42b-005 — Date header grouping for the updates feed
- Priority: P1
- Effort: S
- Depends: T-42b-003, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/core/util/CollectionUtils.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt
- Acceptance:
  - Header emitted before the first item of each local date of `dateFetch`, including the first item (B§2; R§3.4).
  - Tests E-U16, E-U17 pass (DST and midnight splits).

### T-42b-006 — Selection engine (exact port of anchor arithmetic)
- Priority: P1
- Effort: M
- Depends: T-42b-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt
- Acceptance:
  - `Selection::toggle` reproduces every row of the vectors in `p2-edge-cases-and-tests.md` section 3.1 and 3.2, including the quirk rows (B§5.2).
  - Selection ids survive feed refresh; anchors are not recomputed on refresh (B§5.4).

### T-42b-007 — Select all, invert, and selection-mode derivation
- Priority: P1
- Effort: S
- Depends: T-42b-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesTab.kt
- Acceptance:
  - `set_all(true|false)` and `invert` reset anchors to -1 and update the id set (B§5.3, E-U54).
  - Selection mode is true iff at least one visible row is selected (E-U47).

### T-42b-008 — Live download-state patching in the feed
- Priority: P1
- Effort: M
- Depends: T-42b-003, T-42b-004, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt
- Acceptance:
  - Status and progress events patch the single row whose chapter/episode id matches; missing rows are a no-op (E-U22; B§1.4).
  - Progress patch is in-place, no DB re-query (R§3.10).
  - `DownloadState` numeric values are 0..4 (B§3.4).

### T-42b-009 — Download action dispatch (START, START_NOW, CANCEL, DELETE, SHOW_QUALITIES)
- Priority: P1
- Effort: M
- Depends: T-42b-008, T-42b-010, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt, app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt
- Acceptance:
  - START enqueues grouped by manga in first-seen order, skipping missing manga and missing source (E-U25, E-U26); calls restart when any row is ERROR (E-U24).
  - START_NOW, CANCEL, SHOW_QUALITIES require exactly one row; multi-select leaves selection intact (E-U49; deviation recorded as Kotlin quirk).
  - CANCEL on a non-queued row is a no-op; cancel emits NotDownloaded through the download module and does not mutate shared state (E-U27, E-U28).

### T-42b-010 — Enqueue and delete planners with missing-source handling
- Priority: P1
- Effort: S
- Depends: area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt
- Acceptance:
  - `plan_enqueue` and the delete path skip a group silently when the manga/anime or its source is missing; a warning is logged and a toast "Source not installed, downloads were not deleted" is shown on delete (E-U30, deviation recorded).
  - Delete passes the full selection (not only downloaded rows) to the download manager (E-U29).

### T-42b-011 — Mark read and mark unread (chapters)
- Priority: P1
- Effort: S
- Depends: T-42b-001, area:05-data-repos-entries-items, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt
- Acceptance:
  - Mark read writes only unread rows; mark unread writes rows that are read or have progress and resets `lastPageRead` to 0 (E-U31, E-U32; B§4.1).
  - With `removeAfterMarkedAsRead` on, downloads of newly-read chapters are removed, grouped per manga (E-U33).
  - Selection is cleared immediately after the action is started (B§4.1).

### T-42b-012 — Mark seen and unseen (episodes), with fillermark and bookmark
- Priority: P1
- Effort: M
- Depends: T-42b-011, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/domain/items/episode/interactor/SetSeenStatus.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt
- Acceptance:
  - Seen/unseen follows the same filter rules as chapters, writing `lastSecondSeen = 0` on unseen (B§4.1).
  - Fillermark and bookmark batches write only rows whose flag differs (E-U34; B§4.2).

### T-42b-013 — Bookmark toggle for manga chapters
- Priority: P1
- Effort: S
- Depends: T-42b-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt
- Acceptance:
  - Bookmark action writes only rows with the other flag value; the bottom bar shows "Bookmark" when any row is not bookmarked and "Remove bookmark" when all are (B§4.2, B§12.4).

### T-42b-014 — Delete downloads confirmation dialog
- Priority: P1
- Effort: S
- Depends: T-42b-010
- Sources: app/src/main/java/eu/kanade/presentation/updates/UpdatesDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesTab.kt
- Acceptance:
  - Dialog text is "Are you sure you want to delete the selected chapters?" for manga and "Are you sure you want to delete the selected episodes?" for anime; buttons "OK" and "Cancel"; Cancel is the default button (U§1.6.1).
  - Confirm runs delete then closes; Escape and outside click cancel.

### T-42b-015 — Library update trigger from the updates page
- Priority: P1
- Effort: S
- Depends: area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesScreenModel.kt
- Acceptance:
  - Trigger returns false when a run is active; the page shows "An update is already running", otherwise "Updating library" (B§10.1, E-U37).
  - Second click while running does not start a second run (E-U38).
  - Pull-to-refresh and the Refresh button call the same trigger; the 1 s spinner is shown only when started (B§10.2).

### T-42b-016 — Last-updated line and relative time formatting
- Priority: P1
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Values: "Never" for 0, "Just now" under 60 s, relative string otherwise; first row reads "Library last updated: <value>" in italic (B§6.5, B§10.3, E-U41, E-U42).
  - Unit tests for the three cases in `p2-edge-cases-and-tests.md` section 3.7 pass.

### T-42b-017 — Relative date text for headers (shared with Upcoming)
- Priority: P1
- Effort: S
- Depends: area:02-preferences-system, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt, app/src/main/java/eu/kanade/presentation/components/DateText.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - `time_fmt::relative_date` matches the boundary table in `p2-edge-cases-and-tests.md` section 3.4 (E-U14).
  - Respects `relative_time_v2` (default true) and `app_date_format` (default empty = locale short) (D§4).

### T-42b-018 — Open update: reader for manga, player for anime with routing rule
- Priority: P1
- Effort: M
- Depends: T-42b-004, area:24-downloads-anime, area:36-ui-shell-navigation-security-webview, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - Manga row tap opens the reader at the chapter (B§12.5 manga path).
  - Anime row tap computes `extPlayer = alwaysUseExternalPlayer != altPlayer` with `altPlayer = false`; the four-case table E-U35 passes.
  - Bottom "Play externally" and "Play internally" use `altPlayer = true`.

### T-42b-019 — Anime episode quality options from the updates page
- Priority: P2
- Effort: S
- Depends: T-42b-009, area:39-ui-entry-details-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesTab.kt
- Acceptance:
  - SHOW_QUALITIES with exactly one row opens the episode options sheet with `useExternalDownloader`, `episodeTitle`, `episodeId`, `animeId`, `sourceId` (B§3.1, U§1.6.2).

### T-42b-020 — Reset new-updates counter on enter and leave
- Priority: P1
- Effort: S
- Depends: area:26-library-update-jobs, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesTab.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - `library_unread_updates_count` and `library_unseen_updates_count` are set to 0 on page enter and on leave (B§10.4, E-U43).
  - The Updates nav badge reflects the change (area:36-ui-shell-navigation-security-webview).

### T-42b-021 — Updates toolbar actions and selection toolbar
- Priority: P1
- Effort: S
- Depends: T-42b-007, T-42b-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesTab.kt
- Acceptance:
  - Normal toolbar: "View Upcoming Updates" (calendar icon) and "Update library" (refresh icon), in that order (U§1.2).
  - Selecting toolbar: "Select all" and "Select inverse" (U§1.2).

### T-42b-022 — Updates list rows (56 px) with indicators, progress and gestures
- Priority: P1
- Effort: M
- Depends: T-42b-005, T-42b-008, area:34-images-covers-cache, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesUiItem.kt, app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesUiItem.kt
- Acceptance:
  - Row height 56 px, cover 40 px wide with 6 px vertical inset; alpha 0.38 on read or seen rows (U§1.4).
  - Indicators: unread dot, bookmark, fillermark (anime only); progress text only when unread and positive (E-U10).
  - Manga progress "Page: N" with N = lastPageRead + 1; anime "cur/total" with `format_progress` (E-U11, E-U12, E-U13).
  - Tap and Ctrl/Shift/long-press gestures as in U§1.4 and R§7.

### T-42b-023 — Updates page states: loading, empty, populated, error
- Priority: P1
- Effort: S
- Depends: T-42b-022
- Sources: app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesScreen.kt, app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesScreen.kt
- Acceptance:
  - Loading shows a centred spinner; empty shows "No recent updates" (E-U19); populated shows the last-updated row, headers, and rows (U§1.3).
  - Error shows the internal-error toast without clearing rows.

### T-42b-024 — Bottom action bar for updates selection
- Priority: P1
- Effort: M
- Depends: T-42b-009, T-42b-011, T-42b-012, T-42b-013, T-42b-014
- Sources: app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesScreen.kt, app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesScreen.kt
- Acceptance:
  - Each action appears only when its predicate holds (B§12.4): bookmark/remove bookmark, fillermark/remove fillermark (anime), mark viewed/unviewed, download, delete (when any downloaded), play externally/internally (anime, single selection).
  - The bar is shown only while selecting and appears with a 200 ms reveal.

### T-42b-025 — Upcoming query and entry model (manga and anime)
- Priority: P2
- Effort: M
- Depends: area:05-data-repos-entries-items, area:07-domain-entries-items
- Sources: domain/src/main/java/mihon/domain/upcoming/manga/interactor/GetUpcomingManga.kt, domain/src/main/java/mihon/domain/upcoming/anime/interactor/GetUpcomingAnime.kt, data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelightanime/dataanime/animes.sq, data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt, data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt
- Acceptance:
  - Manga status set {1, 4}; anime status set {1, 4, 7} (B§7.1, D§2).
  - `next_update >= local midnight ms`, `favorite = 1`, `ORDER BY next_update ASC` (B§7.2, E-C1..E-C5).
  - `expectedNextUpdate` is null when status is COMPLETED (D§1.6).

### T-42b-026 — Upcoming grouping, per-day counts and header index
- Priority: P2
- Effort: M
- Depends: T-42b-025
- Sources: mihon/feature/upcoming/manga/UpcomingMangaScreenModel.kt, mihon/feature/upcoming/anime/UpcomingAnimeScreenModel.kt, mihon/feature/upcoming/manga/UpcomingMangaUIModel.kt, mihon/feature/upcoming/anime/UpcomingAnimeUIModel.kt, app/src/main/java/eu/kanade/core/util/CollectionUtils.kt
- Acceptance:
  - Output `Header(date, count), Item...` per group in ascending order; `events` map and `header_index` built (B§7.3, B§7.4; R§3.5).
  - Test vector 3.5 in `p2-edge-cases-and-tests.md` passes; dots cap at 3 (E-C7).

### T-42b-027 — Calendar grid math and month header with slide transition
- Priority: P2
- Effort: M
- Depends: T-42b-026
- Sources: mihon/feature/upcoming/components/calendar/Calendar.kt, mihon/feature/upcoming/components/calendar/CalendarHeader.kt
- Acceptance:
  - Week start from locale per R§8.2; leading blanks match vectors 3.3 (E-C12, E-C13).
  - Month title "MMMM yyyy", prev/next buttons with labels "Previous Month" and "Next Month" (U§2.4).
  - Month change animates 200 ms; forward slides from below, backward from above (B§8.2, U§2.6).
  - Month navigation does not filter the list (E-C10); year boundary correct (E-C11).

### T-42b-028 — Calendar day cell and event dots
- Priority: P2
- Effort: S
- Depends: T-42b-027
- Sources: mihon/feature/upcoming/components/calendar/CalendarDay.kt, mihon/feature/upcoming/components/calendar/CalendarIndicator.kt
- Acceptance:
  - Today ring 1 px; past days at alpha 0.38 (E-C16); dots min(count, 3) at alpha 0.3, 0.6, 0.9 with 56/12 px diameter (B§8.3, B§8.4).
  - Day cell hit area is circular (U§2.2).

### T-42b-029 — Day-jump scroll and calendar-to-list wiring
- Priority: P2
- Effort: S
- Depends: T-42b-026, T-42b-028
- Sources: mihon/feature/upcoming/manga/UpcomingMangaScreenContent.kt, mihon/feature/upcoming/anime/UpcomingAnimeScreenContent.kt
- Acceptance:
  - Click on a day scrolls to `header_index[date]` with offset 1 on single-column layout and 0 on two-panel layout (B§7.6, E-C8).
  - Clicking a day with no header does nothing (E-C9).

### T-42b-030 — Upcoming list rows (96 px) and date headers with count badge
- Priority: P2
- Effort: S
- Depends: T-42b-026, area:34-images-covers-cache
- Sources: mihon/feature/upcoming/manga/components/UpcomingItem.kt, mihon/feature/upcoming/anime/components/UpcomingItem.kt, mihon/feature/upcoming/manga/UpcomingMangaScreenContent.kt, mihon/feature/upcoming/anime/UpcomingAnimeScreenContent.kt
- Acceptance:
  - Row height 96 px, book-ratio cover, title up to 2 lines with ellipsis, click opens entry (U§2.5).
  - Date header uses relative text from T-42b-017 and a badge with the count (U§2.5).

### T-42b-031 — Single-column and two-panel layout breakpoints
- Priority: P2
- Effort: S
- Depends: T-42b-027, T-42b-030
- Sources: mihon/feature/upcoming/manga/UpcomingMangaScreenContent.kt, mihon/feature/upcoming/anime/UpcomingAnimeScreenContent.kt, mihon/feature/upcoming/components/calendar/Calendar.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt, app/src/main/java/mihon/core/designsystem/utils/WindowSize.kt
- Acceptance:
  - Two panels at content width >= 720 px; single column below (B§9 proposal).
  - Calendar grid max width 360 px when content width is in (600, 840] (B§8.1, B§9).
  - Paned divider is user-resizable (R§5).

### T-42b-032 — Upcoming help link
- Priority: P2
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt, mihon/feature/upcoming/manga/UpcomingMangaScreenContent.kt
- Acceptance:
  - Help button (tooltip "Upcoming Guide") opens `https://aniyomi.org/docs/faq/updates/upcoming` in the default browser via `gtk::UriLauncher` (D§6, U§2.8).

### T-42b-033 — Upcoming page navigation and toolbar
- Priority: P2
- Effort: S
- Depends: T-42b-027, area:36-ui-shell-navigation-security-webview
- Sources: mihon/feature/upcoming/manga/UpcomingMangaScreen.kt, mihon/feature/upcoming/anime/UpcomingAnimeScreen.kt, mihon/feature/upcoming/manga/UpcomingMangaScreenContent.kt, mihon/feature/upcoming/anime/UpcomingAnimeScreenContent.kt
- Acceptance:
  - Title "Upcoming", back arrow pops the page, Escape pops the page (U§2.1).
  - No empty-state text and no snackbar on this page (U§2.1, U§3).

### T-42b-034 — Upcoming item tap opens entry screen
- Priority: P2
- Effort: S
- Depends: T-42b-030, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime
- Sources: mihon/feature/upcoming/manga/UpcomingMangaScreen.kt, mihon/feature/upcoming/anime/UpcomingAnimeScreen.kt
- Acceptance:
  - Tap on a manga row pushes the manga entry screen by id; anime row pushes the anime entry screen (B§13).

### T-42b-035 — Upcoming tie-break and midnight refresh
- Priority: P2
- Effort: S
- Depends: T-42b-025
- Sources: mihon/feature/upcoming/manga/UpcomingMangaScreenModel.kt, mihon/feature/upcoming/anime/UpcomingAnimeScreenModel.kt
- Acceptance:
  - Entries with equal `next_update` sort by id ascending (Proposal; E-C6).
  - The list re-queries when the page is shown after local midnight (Proposal; E-C18).

### T-42b-036 — Replace the mockup updates screen with the live pages
- Priority: P1
- Effort: M
- Depends: T-42b-022, T-42b-023, T-42b-024
- Sources: src/screens.rs, src/ui.rs, spec/app.json, src/updates/mod.rs, src/updates/manga.rs, src/updates/anime.rs
- Acceptance:
  - `fn updates()` in `src/screens.rs` returns the live pages; hard-coded rows and "5 minutes ago" are removed (U§4.1).
  - Spec ids `updates`, `updates_delete_episodes`, `updates_delete_chapters` remain in use.

### T-42b-037 — Anime and manga tab wiring and navigate-up behaviour
- Priority: P1
- Effort: S
- Depends: T-42b-036, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/anime/AnimeUpdatesTab.kt
- Acceptance:
  - Back arrow only when opened from More; with the Home screen as last item, back opens the Anime library tab (B§11).
  - Tab titles "Manga" and "Anime" from the aniyomi i18n keys (D§7).

### T-42b-038 — Keyboard and focus model for updates and upcoming
- Priority: P2
- Effort: M
- Depends: T-42b-036, T-42b-033
- Sources: src/updates/manga.rs, src/updates/anime.rs, src/upcoming/page.rs
- Acceptance:
  - Ctrl+A select all, Ctrl+I invert, Escape clears selection, Shift+click range, Ctrl+click toggle, Enter opens (R§7, U§1.11).
  - Calendar arrow keys and PgUp/PgDn (Proposal, U§2.7).

### T-42b-039 — Toast component for the updates and upcoming pages
- Priority: P1
- Effort: S
- Depends: T-42b-036
- Sources: src/ui.rs, src/updates/manga.rs
- Acceptance:
  - Toast shows the texts "Updating library", "An update is already running", and "InternalError: Check crash logs for further information" for 4 s, with a queue of up to 3 (U§1.7).
  - Events buffered while the page is hidden are shown on next display (E-U39).

### T-42b-040 — Feed consistency and performance (change coalescing, index for patches)
- Priority: P1
- Effort: M
- Depends: T-42b-036, T-42b-008
- Sources: src/updates/feed.rs, src/updates/selection.rs
- Acceptance:
  - At most one DB query in flight per feed; signals during a query coalesce into one follow-up (R§3.10, R2 in the race section of the edge-case file).
  - Progress patch lookup uses an index keyed by id; 500 rows update at 10 Hz without visible lag (F6).
  - Writes run to completion when the page is dropped (R4).

### T-42b-041 — Library-update single-flight guard shared with the page
- Priority: P1
- Effort: S
- Depends: T-42b-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - `trigger_library_update(kind) -> bool` is backed by an atomic single-flight flag; false while a run is active (R§6, E-U38).
  - Error when the scheduler cannot start shows "Could not start library update" (F5, Proposal).

### T-42b-042 — Android-only behaviours: drop decisions and replacements
- Priority: P3
- Effort: S
- Depends: T-42b-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesTab.kt, app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesUiItem.kt, app/src/main/java/eu/kanade/presentation/util/FastScrollAnimateItem.kt
- Acceptance:
  - Haptic feedback dropped; pull-to-refresh replaced by Refresh button and F5; BackHandler replaced by Escape; `MainActivity.ready` gate replaced by show-after-data; `animateItem` dropped; `FastScrollLazyColumn` replaced by GTK scrollbar (B§14).
  - Each drop is listed in the release notes with the user-facing loss in B§14.

### T-42b-043 — Tests: model, selection, grouping, calendar and time formatting
- Priority: P1
- Effort: M
- Depends: T-42b-006, T-42b-005, T-42b-026, T-42b-027, T-42b-016, T-42b-017
- Sources: src/updates/selection.rs, src/updates/model.rs, src/upcoming/model.rs, src/upcoming/calendar.rs, src/time_fmt.rs
- Acceptance:
  - `cargo test` runs every pure-function vector in `p2-edge-cases-and-tests.md` section 3 and passes.
  - No GTK import in these modules.

### T-42b-044 — Tests: SQL behaviour and set-read semantics on an in-memory DB
- Priority: P1
- Effort: M
- Depends: T-42b-001, T-42b-011, area:03-db-manga-schema-migrations
- Sources: tests/fixtures/updates_upcoming.sql, src/updates/store.rs, src/upcoming/store.rs
- Acceptance:
  - E-U1..E-U5 and E-C1..E-C5 pass on the fixture (section 7 of the edge-case file).
  - E-U33 (remove-after-read only for newly read) passes with a fake download module.

### T-42b-045 — Week start and locale handling for calendar
- Priority: P2
- Effort: S
- Depends: T-42b-027
- Sources: src/upcoming/calendar.rs, src/settings.rs
- Acceptance:
  - `week_start` returns Sunday for US, CA, JP, IL, BR, IN, KR, PH, MX, TW, ZA, SA, HK and Saturday for AF, IR, otherwise Monday (R§8.2).
  - Setting `upcoming.week_start` = auto | monday | sunday overrides the locale (D§4.1, Proposal).

### T-42b-046 — Optional Home key returns to the current month (off by default)
- Priority: P3
- Effort: S
- Depends: T-42b-027
- Sources: mihon/feature/upcoming/components/calendar/CalendarHeader.kt
- Acceptance:
  - Aniyomi has no Today button (B§8.2); this task is an opt-in Proposal behind a setting, off by default, so parity holds.
