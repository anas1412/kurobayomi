# 25-downloads-shared — implementation tasks (part 1)

Every scope file is covered:
- DeleteEpisodeDownload.kt → T-25a-010, 013, 015, 016
- DeleteChapterDownload.kt → T-25a-011, 013, 015, 016
- DownloadsTab.kt → T-25a-021 … 034
- DownloadPreferences.kt (all 23 accessors and 6 category keys) → T-25a-001 … 006

Edge cases E-01 … E-38 (p1-edge-cases-and-tests.md) are each assigned to a task in its acceptance criteria.

---

### T-25a-001 — Typed download preference store with exact keys and defaults
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt
- Acceptance:
  - `DownloadPrefs` exposes one typed accessor per Kotlin accessor (23 in total, names in snake_case) and a `keys` module with the exact key strings from DownloadPreferences.kt (for example `pref_download_only_over_wifi_key`, `download_slots`).
  - A unit test asserts every key string and default value in p1-data-and-config.md section 3 (table rows 1-23).
  - Reads and writes go through the area:02 store; no value is cached in the struct.
  - `Pref<T>` provides `get`, `set`, and `watch` (tokio watch receiver); a set in one place is visible to a watcher.

### T-25a-002 — Storage and transport preferences
- Priority: P1
- Effort: S
- Depends: T-25a-001, area:23-downloads-manga, area:24-downloads-anime
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt
- Acceptance:
  - `download_only_over_wifi` (`pref_download_only_over_wifi_key`) defaults to `true`; `use_external_downloader` defaults to `false`; `external_downloader_selection` defaults to `""`; `save_chapter_as_cbz` defaults to `true`; `split_tall_images` defaults to `true`.
  - The engine (area:23/24) reads these through the accessors only (no duplicated constants).
  - Key strings are unchanged so backups restore (area:22).

### T-25a-003 — Concurrency and throughput preferences (download_slots, download_speed_limit)
- Priority: P1
- Effort: S
- Depends: T-25a-001, area:23-downloads-manga, area:24-downloads-anime
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - `download_slots` (Int, default `1`) is stored; the UI offers exactly 1 to 5 (SettingsDownloadScreen.kt:103 `(1..5)`).
  - Decision recorded (E-12): the engine honours it as parallel download count clamped to 1..5, OR the setting is hidden. The Kotlin tree has no reader; a test asserts whichever decision was taken.
  - `download_speed_limit` (Int KB/s, default `0`) is converted with `value * 1024` bytes/s (MangaDownloader.kt:525); `0` means unlimited (zero semantics confirmed in area:23 and tested there).

### T-25a-004 — Auto-download and new-item policy preferences
- Priority: P1
- Effort: M
- Depends: T-25a-001, T-25a-006, area:23-downloads-manga, area:24-downloads-anime
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, domain/src/main/java/mihon/domain/items/chapter/interactor/FilterChaptersForDownload.kt, domain/src/main/java/mihon/domain/items/episode/interactor/FilterEpisodesForDownload.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `download_new` (default `false`), `download_new_episode` (default `false`), `auto_download_while_reading` (Int, default `0`), `auto_download_while_watching` (Int, default `0`), `download_new_unread_chapters_only` (default `false`), `download_new_unread_episodes_only` (default `false`) are stored with those defaults.
  - Filter inputs (new-item flags with category include/exclude from T-25a-006) are passed to FilterChaptersForDownload / FilterEpisodesForDownload unchanged.
  - Unit test: flags off → no automatic enqueue.

### T-25a-005 — Remove-after-read preferences
- Priority: P1
- Effort: S
- Depends: T-25a-001
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - `remove_after_read_slots` (Int, default `-1`), `pref_remove_after_marked_as_read_key` (default `false`), `pref_remove_bookmarked` (default `false`), `pref_download_fillermarked` (default `false`) are stored with those defaults.
  - Semantics documented in the accessor's doc comment: `-1` disables slots; `pref_remove_bookmarked=false` protects bookmarked items.
  - Unit test: default values; toggling `pref_remove_bookmarked` changes the delete filter result (T-E19).

### T-25a-006 — Category-set preferences and the restore key list
- Priority: P1
- Effort: M
- Depends: T-25a-001, area:08-domain-library-category-history-updates, area:21-backup-format-and-create
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt, app/src/main/java/mihon/core/migration/migrations/CategoryPreferencesCleanupMigration.kt
- Acceptance:
  - Six string-set keys store category ids as strings: `remove_exclude_categories`, `remove_exclude_anime_categories`, `download_new_categories`, `download_new_anime_categories`, `download_new_categories_exclude`, `download_new_anime_categories_exclude`; default empty set.
  - `CATEGORY_KEYS` equals `categoryPreferenceKeys` in the same order (DownloadPreferences.kt:121-128).
  - `parse_category_ids` returns `DeleteError::BadCategoryId` for non-numeric values (E-05); the delete path aborts that call and logs; no panic.
  - Backup restore merges these six keys with the library category keys (PreferenceRestorer.kt:136).

### T-25a-010 — Port DeleteEpisodeDownload.awaitAll
- Priority: P1
- Effort: S
- Depends: T-25a-012, T-25a-013, T-25a-015, T-25a-016
- Sources: app/src/main/java/eu/kanade/domain/download/anime/interactor/DeleteEpisodeDownload.kt, app/src/main/java/eu/kanade/domain/DomainModule.kt
- Acceptance:
  - `DeleteEpisodeDownload::await_items(entry, candidates, names)` resolves the source first and returns `Ok(0)` when it is unknown (E-02).
  - Calls the shared pipeline (T-25a-013) with `EntryKind::Anime`.
  - Registered once in the app's DI (equivalent of DomainModule.kt:339 `addFactory`) with constructor injection.

### T-25a-011 — Port DeleteChapterDownload.awaitAll
- Priority: P1
- Effort: S
- Depends: T-25a-010
- Sources: app/src/main/java/eu/kanade/domain/download/manga/interactor/DeleteChapterDownload.kt, app/src/main/java/eu/kanade/domain/DomainModule.kt
- Acceptance:
  - Same contract as T-25a-010 with `EntryKind::Manga` and the chapter model.
  - Registered like DomainModule.kt:351.
  - Unit test: unknown source → `Ok(0)` and no file touched.

### T-25a-012 — Pure delete filter (select_for_delete)
- Priority: P1
- Effort: S
- Depends: T-25a-005, T-25a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt
- Acceptance:
  - Implements the four ordered rules of p1-behavior.md section 1.4 (categories fallback `[0]`, protected watched/read in excluded categories, bookmark rule).
  - Tests T-E01, T-E04, T-E18, T-E18b, T-E19 pass for both kinds.
  - Input empty → output empty (E-01).

### T-25a-013 — Delete execution pipeline (queue removal, dirs, cache, empty cascade, per-series lock)
- Priority: P1
- Effort: M
- Depends: T-25a-012, T-25a-017, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - Order of operations is the one in p1-behavior.md 1.5: filter, remove from queue (pause if running, restart or stop after), remove item dirs, cache update, cascade empty series dir, cascade empty source dir and `cache.remove_source`.
  - `remove_from_queue` sets DOWNLOADING and QUEUE items to NOT_DOWNLOADED (E-17), test T-E17.
  - Cascade only on `Some(true)` (E-20, T-E20); missing item dir is skipped (E-21, T-E21).
  - Per-series async mutex keyed by `(kind, entry id)` serialises steps 5 to 9 (E-22, T-E22).
  - Idempotent on partially removed trees (E-23, T-E23).
  - Removing an entry from the library removes its queued items too (AnimeDownloader.kt:900 behaviour).

### T-25a-014 — Remove-after-marked-as-read hook (consumer contract)
- Priority: P1
- Effort: S
- Depends: T-25a-005, T-25a-010, T-25a-011, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/domain/items/episode/interactor/SetSeenStatus.kt, app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt
- Acceptance:
  - Delete is invoked only when `seen`/`read` is true AND the preference is on (SetSeenStatus.kt:49, SetReadStatus.kt:49).
  - Only the items actually changed by this call are passed (E-06, T-E06); partially watched items are included (T-E06b).
  - Grouping by parent id: one delete call per series (SetSeenStatus.kt:50-57).
  - Result enum values unchanged: Success, NoEpisodes/NoChapters, InternalError.

### T-25a-015 — Unknown-source delete is a logged no-op
- Priority: P1
- Effort: S
- Depends: T-25a-013
- Sources: app/src/main/java/eu/kanade/domain/download/anime/interactor/DeleteEpisodeDownload.kt, app/src/main/java/eu/kanade/domain/download/manga/interactor/DeleteChapterDownload.kt
- Acceptance:
  - No file removed, no queue change, returns `Ok(0)` (E-02).
  - A `tracing::warn!` names the entry id and source id.
  - Test: installed set without the source → warning captured, files untouched.

### T-25a-016 — Await semantics: caller waits for removal, UI stays responsive
- Priority: P1
- Effort: S
- Depends: T-25a-013
- Sources: app/src/main/java/eu/kanade/domain/download/anime/interactor/DeleteEpisodeDownload.kt, app/src/main/java/eu/kanade/domain/download/manga/interactor/DeleteChapterDownload.kt
- Acceptance:
  - `await_items` returns after removal finishes (E-03 deviation); the file system work runs on `spawn_blocking`.
  - GTK callers use `glib::spawn_future_local` and never block the main loop (test: a 200 ms fake removal does not stall a main-loop tick).
  - Cancellation: the removal future is not dropped mid-way (`tokio::spawn` the work and await its JoinHandle).

### T-25a-017 — QueueEngine adapter: start, stop, pause_and_stop, clear, reorder, remove
- Priority: P1
- Effort: M
- Depends: area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - `start`: no-op when running; start the job if it is running, otherwise launch it (E-30, T-E30).
  - Anime `stop` = stop; manga `pause_and_stop` = pause then stop (E-29, T-E29); `clear` = clear then stop for both (E-16, T-E16).
  - `reorder(ids)` treats ids as ordering (E-15, T-E15): unknown ids ignored, live items not listed appended in current order, empty list is a no-op (does not clear).
  - All commands are non-blocking sends to the engine task.

### T-25a-020 — Pure QueueModel (sections, stable sort + reverse, move, series moves, removals)
- Priority: P1
- Effort: M
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreenModel.kt
- Acceptance:
  - `sections()` groups by source in first-appearance order (p1-behavior.md 4.1).
  - `sort_within_sections` = stable sort then reverse when requested (E-07, T-E07); never moves items across sections (E-08, T-E08).
  - `move_to_top/bottom` act inside the item's own section; `move_series` uses a flat partition (E-09, T-E09).
  - `remove` and `remove_series` return removed ids; no match → empty (E-11, T-E11).
  - Cross-section flat order is preserved on a drop (T-E34).
  - Two sources with the same name are two sections (E-37, T-E37).

### T-25a-021 — Downloads page shell in the app navigation
- Priority: P1
- Effort: M
- Depends: T-25a-020, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt
- Acceptance:
  - Page registered as the "Download queue" destination at sidebar position 6 (TabOptions.index = 6u in DownloadsTab.kt:85).
  - Header title "Download queue" (MR.strings.label_download_queue) with one line and ellipsis.
  - Title count pill shows the anime count only, and only when > 0 (E-26, T-E26); font 14 sp equivalent; never shows manga count.
  - Pager: two pages, Anime first, Manga second, labels "Anime" and "Manga" (AYMR strings).
  - Back action pops the page (navigateUp).

### T-25a-022 — Tab badges and badge rule
- Priority: P1
- Effort: S
- Depends: T-25a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Tabs.kt
- Acceptance:
  - Each tab shows its page's total item count as a badge (sum of sub-item counts per section).
  - Badge hidden at 0 (proposal, E-14; Kotlin shows "0"). Test T-E14 (new).
  - Badge updates on every snapshot.

### T-25a-023 — FAB start/pause: label, icon, visibility, scroll collapse
- Priority: P1
- Effort: M
- Depends: T-25a-017, T-25a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt
- Acceptance:
  - Visible iff the current page's list is non-empty (E-27, T-E27); crossfade 200 ms.
  - Labels: anime running "Stop" / stopped "Continue"; manga running "Pause" / stopped "Resume" (E-28, T-E28).
  - Icons: running → pause symbolic, stopped → play symbolic.
  - Click: anime running → stop; manga running → pause_and_stop; stopped → start (both kinds).
  - Collapse to icon-only when content scrolls up (dy > 0), expand when dy <= 0; starts expanded (DownloadsTab.kt:111-132).
  - Running state updates from the engine watch within one frame.

### T-25a-024 — Sort menu (two submenus, four leaves)
- Priority: P1
- Effort: M
- Depends: T-25a-020, T-25a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt
- Acceptance:
  - Menu: "By upload date" → "Newest" (reverse=true, upload date desc), "Oldest" (reverse=false); "By episode number" (anime) / "By chapter number" (manga) → "Ascending" (reverse=false), "Descending" (reverse=true).
  - Hidden when the list is empty (DownloadsTab.kt:299, 383; E-35).
  - Sort applies per section and is sent to the engine as the new order (E-08).
  - Menu closes after an action.

### T-25a-025 — Overflow "Cancel all" (clear and stop)
- Priority: P1
- Effort: S
- Depends: T-25a-017, T-25a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt
- Acceptance:
  - Shown only when the page's list is non-empty.
  - No confirmation dialog (matches Kotlin).
  - Runs `clear` (clear queue then stop); test T-E16.

### T-25a-026 — Queue list: sections per source, rows, empty state, keyboard
- Priority: P1
- Effort: L
- Depends: T-25a-020, T-25a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHolder.kt
- Acceptance:
  - One section per source with header "Source name (count)" (proposal, Kotlin shows name; count visible as badge).
  - Row title = episode name (anime) / chapter name (manga); subtitle = anime title / manga title (AnimeDownloadHolder.kt:41,44; MangaDownloadHolder.kt:38,41).
  - Empty page: "No downloads" (MR.strings.information_no_downloads) with an icon; no FAB, no sort, no cancel-all (T-E27 related).
  - Alt+Up / Alt+Down move the focused row (Proposal); Delete cancels the focused row (Proposal).
  - Tab order reaches every row action.

### T-25a-027 — Row menu with exact labels and visibility
- Priority: P1
- Effort: M
- Depends: T-25a-026, T-25a-031
- Sources: app/src/main/res/menu/download_single.xml, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHolder.kt
- Acceptance:
  - Six items in order with labels: "Move to top", "Move series to top", "Move to bottom", "Move series to bottom", "Cancel", "Cancel all for this series".
  - "Move to top" shown only when flat index > 0; "Move to bottom" only when flat index < last (E-10, T-E10).
  - Menu rebuilt on open so visibility reflects the current snapshot.

### T-25a-028 — Drag reorder across sections
- Priority: P2
- Effort: M
- Depends: T-25a-026, T-25a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadAdapter.kt
- Acceptance:
  - Drag handle starts a drag; drop computes the flat order in on-screen order and sends it (`reorder`).
  - Cross-section drops are allowed and produce the flat order; the regroup on next snapshot can move the item back into its source section (E-34, T-E34). Visual cue about this is a Proposal.
  - No reorder call when the drop position equals the original position.

### T-25a-029 — Row progress: formats, updates, throttle, timeout, errors
- Priority: P1
- Effort: M
- Depends: T-25a-017, T-25a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreenModel.kt
- Acceptance:
  - Anime text: "Downloading…" (MR.strings.update_check_notification_download_in_progress) at progress 0, else "NN%" (AYMR.strings.episode_download_progress); bar indeterminate at 0.
  - Manga text: "done/total" (for example "12/30"), empty when pages unknown; bar max = pages*100 (MangaDownloadHolder.kt:43-72).
  - Progress coalesced at most one repaint per 50 ms per row (E-32, T-E32).
  - Manga pages-unknown poll stops after 600 ticks of 50 ms (30 s) with a warning (E-13, Proposal).
  - ERROR stops progress updates (E-31, T-E31); DOWNLOADED cancels progress then does one final update.
  - Lookup miss is a no-op (E-33, T-E33).

### T-25a-030 — Empty state and the no-downloads message
- Priority: P1
- Effort: S
- Depends: T-25a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreen.kt
- Acceptance:
  - Shows "No downloads" when the page list is empty (both pages independently).
  - Initial state before the first snapshot is the same empty state (no spinner, Kotlin behaviour; a Proposal to add a spinner is rejected).

### T-25a-031 — Move-series and cancel-series actions
- Priority: P1
- Effort: S
- Depends: T-25a-020, T-25a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueScreenModel.kt
- Acceptance:
  - "Move series to top" = selected series first, then others, as flat list (T-E09); "Move series to bottom" = others then selected.
  - "Cancel all for this series" cancels every item of the same parent id in the current page; empty match → no call (E-11).
  - Series key = parent id (anime.id / manga.id; manga uses download.manga.id).

### T-25a-032 — One queue model per kind, owned by the page
- Priority: P2
- Effort: S
- Depends: T-25a-020, T-25a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadQueueTab.kt
- Acceptance:
  - Exactly one model per kind lives in the page (Kotlin creates two; E-36).
  - Test: opening the page twice does not double subscriptions (subscriber count stays 1 per kind).

### T-25a-033 — Replace the mockup download page with the real queue page
- Priority: P1
- Effort: M
- Depends: T-25a-021, T-25a-026, T-25a-023, T-25a-024, T-25a-025
- Sources: src/screens.rs, spec/app.json
- Acceptance:
  - `src/screens.rs::downloads()` (line 354) no longer builds "Source A"/"Source B" fake rows; it builds the page from the real `QueueModel`.
  - The fake per-row "45%" and "12/30" values are gone; progress comes from the engine.
  - `spec/app.json` screen `download_queue` lists the toolbar Sort action, the overflow "Cancel all", and tabs "Anime" and "Manga", matching section 7 of p1-rust-design.md.
  - `download_button()` (line 134) is not used by this page; its behaviour is owned by the entry screens.

### T-25a-034 — Android-only items: replace or drop with recorded decision
- Priority: P3
- Effort: S
- Depends: T-25a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt
- Acceptance:
  - Animated tab icon `anim_history_enter` (AnimatedImageVector, DownloadsTab.kt:84) is replaced by a static symbolic icon; the animation is dropped and recorded here (P3 drop, reason: GTK sidebar uses static icons; user loses the enter animation only).
  - Compose `isSystemInDarkTheme()` pill alpha (DownloadsTab.kt:146) is replaced by GTK theme CSS (T-25a-039).
  - `NestedScrollConnection` is replaced by the scroll controller in T-25a-023.
  - `HorizontalPager` is replaced by `gtk::Notebook`; a swipe gesture is optional (T-25a-037).

### T-25a-035 — Test suite for the pure modules and fakes
- Priority: P1
- Effort: M
- Depends: T-25a-012, T-25a-020, T-25a-013, T-25a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadQueueScreenModel.kt
- Acceptance:
  - Every test id in p1-edge-cases-and-tests.md section 5 exists and passes (`cargo test download::`).
  - Fixtures F-01 to F-05 exist as test helpers.
  - `cargo clippy --all-targets -- -D warnings` clean.

### T-25a-036 — Remove-after-read slots consumer rule
- Priority: P1
- Effort: S
- Depends: T-25a-005, area:43-reader-core, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - `-1` disables; `n >= 0` targets index `current - n` with `getOrNull` (E-24).
  - Tests: n=0 → current; n=-1 → none; n=5 with index 4 → none.
  - Shared helper lives in `download::policy` and is used by reader and player.

### T-25a-037 — Narrow-width and swipe behaviour
- Priority: P2
- Effort: S
- Depends: T-25a-021, T-25a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt
- Acceptance:
  - At content width below 480 px the FAB shows icon-only (Proposal).
  - Title ellipsises; tab labels stay on one line.
  - Touchpad horizontal swipe switches pages (GestureSwipe, P2 optional).

### T-25a-038 — Accessibility labels
- Priority: P2
- Effort: S
- Depends: T-25a-022, T-25a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt
- Acceptance:
  - Tab accessible name "Anime, N downloads" (Proposal).
  - Row buttons have accessible names matching their menu labels.
  - FAB has its text label as accessible name.

### T-25a-039 — Theme tokens for the count pill, badges and FAB
- Priority: P2
- Effort: S
- Depends: T-25a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Tabs.kt
- Acceptance:
  - Pill background `alpha(@theme_fg_color, 0.08)` (light and dark use the GTK theme; no separate dark code path).
  - Pill font size matches 14 px title and 10 px tab badge.
  - Works with the user's GTK theme (Tokyo Night per user memory) without overrides.

### T-25a-040 — Animations: FAB fade and label collapse
- Priority: P2
- Effort: S
- Depends: T-25a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt
- Acceptance:
  - FAB visibility change uses a crossfade `gtk::Revealer` 200 ms (DownloadsTab.kt:174-175 fadeIn/fadeOut).
  - Label collapse uses a 150 ms slide (Proposal).
  - Reduced-motion preference from the GTK settings disables both.
</content>
