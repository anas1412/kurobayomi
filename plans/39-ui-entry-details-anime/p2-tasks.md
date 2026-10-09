# 39-ui-entry-details-anime (part 2): implementation tasks

Format: see `plans/_tools/tasks.py`. Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/39-ui-entry-details-anime/p2-tasks.md`.

Test IDs (`T-FLG-*`, `T-EPS-*`, `T-SEL-*`, `T-SEA-*`, `T-MDL-*`, `T-DEF-*`, `T-LIB-*`, `T-REF-*`, `T-MRK-*`, `T-DL-*`, `T-TRK-*`, `T-AIR-*`, `T-REL-*`, `T-NAV-*`, `T-UI-*`, `V-*`) are defined in `p2-edge-cases-and-tests.md`.

Priority rules used here (from the project brief): P0 = needed to read and watch daily from one working source; P1 = parity features most users rely on; P2 = long tail; P3 = Android-only, with the replacement or drop decision stated.

Kurobayomi paths referenced in acceptance criteria are under `/home/blackbox/kurobayomi/` (`src/entry.rs`, `src/screens.rs`, `src/ui.rs`, `spec/entries.json`), and new crate paths are as in `p2-rust-design.md` section 1.

---

### T-39b-001 — Entry model: load sequence, state machine and first-open refresh decision
- Priority: P0
- Effort: L
- Depends: area:07-domain-entries-items, area:05-data-repos-entries-items, T-39b-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - `AnimeEntryModel::load` performs the sequence of behavior 2.3 steps 1-11 in order; `Loading` ignores dispatches (`T-MDL-5`).
  - Refresh flags follow `needRefreshInfo = !initialized`, `needRefreshEpisode = episodes.is_empty() && Episodes`, `needRefreshSeason = seasons.is_empty() && Seasons` (`T-MDL-2`).
  - `is_refreshing` is true until the refresh completes, also on error (`T-MDL-3`).
  - Kurobayomi opens a fixture entry from a DB file and shows the first Success state within one frame of data availability.

### T-39b-002 — Episode list item model and download-state derivation
- Priority: P0
- Effort: M
- Depends: T-39b-001, area:24-downloads-anime, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - `EpisodeItem` carries episode, `DownloadState` (values 0..4 as in `AnimeDownload.State`), progress, and selected flag; a queued download beats an on-disk copy (`T-DL-6`).
  - Local anime: every item is DOWNLOADED and no download lookup runs (behavior 13.5, `T-EPS-DL-1`).
  - `isDownloaded` is true only for DOWNLOADED.

### T-39b-003 — Episode sort engine (four keys, both directions, source-order rule)
- Priority: P0
- Effort: S
- Depends: none
- Sources: domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt
- Acceptance:
  - `episode_comparator(key, desc)` matches the table in topic episode-list section 2.2 for all 8 combinations (`T-EPS-SORT-1`).
  - SOURCE + desc sorts by ascending `source_order` (`T-EPS-SORT-2`); the default flags word 0 gives this order.
  - Alphabetical uses ICU PRIMARY collation with the user locale (`T-EPS-SORT-3`).

### T-39b-004 — Episode filter engine (tri-state filters, downloaded-only override, local rule)
- Priority: P0
- Effort: S
- Depends: T-39b-003, T-39b-006
- Sources: app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt, domain/src/main/java/tachiyomi/domain/entries/TriState.kt, app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt
- Acceptance:
  - Filter order and semantics as behavior 8 and topic episode-list section 3 (`T-EPS-FILT-1`).
  - Global downloaded-only forces `EnabledIs` on the downloaded filter without changing the stored flag (`T-EPS-FILT-2`, E-FILTER-5).
  - Local anime passes every item through the downloaded filter (E-FILTER-4).

### T-39b-005 — Missing-episode separators and gap rule
- Priority: P0
- Effort: S
- Depends: T-39b-003
- Sources: domain/src/main/java/tachiyomi/domain/items/episode/service/MissingEpisodes.kt, app/src/main/java/eu/kanade/core/util/CollectionUtils.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - `gap_numbers` and `episode_gap` pass the two upstream tests in `p2-edge-cases-and-tests.md` section 1.1 (`T-EPS-GAP-1`, `T-EPS-GAP-2`).
  - `with_separators` reproduces `insertSeparators` call order: leading `(null, first)`, adjacent pairs, trailing `(last, null)` (`T-EPS-SEP-1`).
  - Ids follow `"<lower>-<higher>"` with `null` for an absent lower (E-LIST-9).

### T-39b-006 — Episode flag codec and SetAnimeEpisodeFlags semantics
- Priority: P0
- Effort: M
- Depends: area:04-db-anime-schema-migrations, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/interactor/SetAnimeEpisodeFlags.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt
- Acceptance:
  - Every mask and value of `episodeFlags` (data file section 2.1) is a constant; `set_flag` equals Kotlin `setFlag` (`T-FLG-1`, `T-FLG-2`, `T-FLG-3`).
  - Same-key sort selection flips the direction; a new key sets ascending (`T-FLG-4`, E-FILTER-6, E-FILTER-7).
  - Unknown high bits are preserved on write (E-STORE-1).

### T-39b-007 — Season flag codec and SetAnimeSeasonFlags semantics
- Priority: P1
- Effort: M
- Depends: T-39b-006
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/interactor/SetAnimeSeasonFlags.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt
- Acceptance:
  - Every season mask and value (topic seasons section 2) is a constant; each filter, the grid mode (bits 16-17), grid size (bits 18-21), five overlay bits, and display mode bit 27 round-trip (`T-FLG-5`..`T-FLG-8`).
  - `SeasonDisplayMode` rejects 4..7 with an error value, not a panic (E-SEA-7).

### T-39b-008 — Season filter, sort and overlay pipeline
- Priority: P1
- Effort: M
- Depends: T-39b-007, T-39b-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, domain/src/main/java/tachiyomi/domain/items/season/service/SeasonSorter.kt, domain/src/main/java/aniyomi/domain/anime/SeasonAnime.kt
- Acceptance:
  - `process_seasons` matches topic seasons section 3.1 exactly, including unclamped filter counts and the always-ascending alphabetical tie-break (`T-SEA-1`, `T-SEA-2`, `T-SEA-3`).
  - `show_continue` requires `unseen > 0` and `fetchType == Episodes` (`T-SEA-4`, E-SEA-5).
  - `seen` for total 0 is true (`T-SEA-5`).

### T-39b-009 — Viewer flags codec (skip intro and airing bits)
- Priority: P0
- Effort: S
- Depends: area:04-db-anime-schema-migrations
- Sources: app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt
- Acceptance:
  - Masks `0xFF`, `0x00FF_FF00`, `0x0FFF_FFFF_F000_000`, `1 << 56` pinned by `V-1`.
  - Round trips and independent writes pass `V-2`; skip-intro 0 sets the disable bit (`V-3`).
  - Each write is one atomic SQL update, not a read-modify-write (E-AIR-1).

### T-39b-010 — Episode settings sheet actions and set-as-default (key-collision fix)
- Priority: P0
- Effort: M
- Depends: T-39b-006, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, domain/src/main/java/tachiyomi/domain/items/episode/interactor/SetAnimeDefaultEpisodeFlags.kt
- Acceptance:
  - Each setter writes the mapping in behavior section 9 (table with masks and values) and does not change the list directly (no optimistic write).
  - `set_as_default(apply_to_existing)` writes the Kurobayomi keys of data file section 4.5, never the manga chapter keys (`T-DEF-4`), and with apply-all updates every favorite (E-DEF).
  - Snackbar "Updated default episode settings" shows once per successful save.

### T-39b-011 — Season settings sheet actions and set-as-default (downloaded-filter fix)
- Priority: P1
- Effort: M
- Depends: T-39b-007, T-39b-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, domain/src/main/java/tachiyomi/domain/items/season/interactor/SetAnimeDefaultSeasonFlags.kt
- Acceptance:
  - Each season setter writes the mapping in topic seasons section 4.
  - Set-as-default writes the downloaded filter from the downloaded value, fixing behavior 23.9 (`T-DEF-3`, `T-FLG-9`).
  - Snackbar "Updated default season settings" on save.

### T-39b-012 — Apply library defaults once for non-library entries (no per-open overwrite)
- Priority: P0
- Effort: S
- Depends: T-39b-010, T-39b-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, domain/src/main/java/tachiyomi/domain/items/episode/interactor/SetAnimeDefaultEpisodeFlags.kt, domain/src/main/java/tachiyomi/domain/items/season/interactor/SetAnimeDefaultSeasonFlags.kt
- Acceptance:
  - Opening a non-library entry writes no defaults to its row (`T-DEF-1`); behavior 23.1 is fixed.
  - Defaults for a non-library entry are applied at the moment the user adds it to the library, and for the session through an in-memory override while it is open.
  - Documented in the UI: "Settings are kept for this session".

### T-39b-013 — Mark seen and unseen (SetSeenStatus semantics and download removal)
- Priority: P0
- Effort: M
- Depends: T-39b-002, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/domain/items/episode/interactor/SetSeenStatus.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Filtering (`seen` true: `!seen`; false: `seen || last_second_seen > 0`) and the empty-list `NoEpisodes` result match behavior 1.2 (`T-MRK-1`, `T-MRK-2`).
  - Unseen resets `last_second_seen` to 0; seen keeps it (E-MRK-3).
  - With `pref_remove_after_marked_as_read_key` on, seen deletes downloads for the affected entries (`T-MRK-3`).

### T-39b-014 — Mark previous episodes as seen
- Priority: P0
- Effort: S
- Depends: T-39b-013, T-39b-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Uses the visible (filtered, sorted) view and the reversed order for descending display (behavior 11.5), passing `T-MRK-4`.
  - A pointer not in the view marks nothing.

### T-39b-015 — Bookmark and fillermark bulk toggles
- Priority: P0
- Effort: S
- Depends: area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Only episodes whose flag differs are written (behavior 11.2).
  - Selection is cleared after the call (E-SEL-10).
  - Swipe actions ToggleBookmark and ToggleFillermark use the same functions (`T-SEL-*` regression).

### T-39b-016 — Episode selection model (toggle, long-press range, select all, invert)
- Priority: P0
- Effort: M
- Depends: T-39b-002, T-39b-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - `Selection::toggle_one` reproduces topic episode-list section 5.2 for every branch (`T-SEL-1`, `T-SEL-2`, `T-SEL-3`, E-SEL-1..5, E-SEL-9).
  - Select all and invert act on the full list (`T-SEL-4`, E-SEL-6, E-SEL-7).
  - Ids and flags stay consistent (`T-SEL-5`, property test).
  - The model never writes the filtered view into the full list (`T-SEL-6`, fix 23.2) and the transform is pure (`T-SEL-7`).

### T-39b-017 — Episode open routing (internal or external, alt inverts the default)
- Priority: P0
- Effort: S
- Depends: area:45-player-core, T-39b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - Tap opens with `external = always_external XOR alt` (behavior 17.4).
  - Continue-watching and season continue use `always_external` without XOR.
  - Torrent server start is attempted only for torrent sources, before the player opens (T-39b-052).

### T-39b-018 — Continue watching: next unseen (both overloads)
- Priority: P0
- Effort: S
- Depends: T-39b-004, T-39b-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/episode/EpsiodeGetNextUnseen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - `next_unseen` = first unseen in display order when ascending, last when descending (`T-EPS-NEXT-1`, `T-EPS-NEXT-2`).
  - No unseen episode: the button is disabled and no navigation happens.

### T-39b-019 — Snackbar, toast and message-key layer
- Priority: P0
- Effort: M
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - `UserMessage` enum carries every key in behavior section 19 with its English text; all 16 keys render correctly with `%d` and `%s` substitution.
  - Snack: one at a time (E-UI-3), Short = 4 s, dismiss and action only where the table in UI parity section 4 says so (`T-UI-1`).
  - Toast: 2 s, no action.

### T-39b-020 — Refresh error mapping
- Priority: P0
- Effort: S
- Depends: T-39b-019, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - `NoEpisodesException` -> "No episodes found"; `NoSeasonsException` -> "No seasons found"; others -> formatted message (`T-REF-1`, `T-REF-2`).
  - `CancellationException` is not shown (E-CONC-1 family).

### T-39b-021 — Source refresh pipeline for episodes (manual and automatic) with the empty-list guard
- Priority: P0
- Effort: L
- Depends: area:16-source-api-anime, area:17-source-app-layer, area:07-domain-entries-items, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt, app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt
- Acceptance:
  - Episode path of behavior 6.1-6.2: details and episodes fetched per flags; `new_episodes` returned for auto-download (`T-REF-4`).
  - Cover and background rules of behavior 6.5 (`T-REF-5`).
  - Zero-episode refresh is limited to once per 10 minutes per entry (`T-REF-3`, E-REF-3, fix 23.24).
  - Manual refresh runs `fetchAllFromSource(manual=true)`; open-time refresh runs `manual=false` and never auto-downloads (E-REF-5).

### T-39b-022 — Season refresh pipeline and per-season episode fetch
- Priority: P1
- Effort: M
- Depends: T-39b-021, T-39b-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt
- Acceptance:
  - Season list refresh with all seasons returned (behavior 6.4); episode fetch per season only for Episodes children that are not completed or never updated (`T-REF-7`).
  - `UnmeteredSource` fetches in parallel with at most 8 at once; metered sources sequentially with cancellation between items (E-SEA-9, E-SEA-10).
  - Child episodes are stored and not downloaded (`T-REF-6`, fix 23.7 kept and documented).

### T-39b-023 — Auto-download of new episodes after manual refresh
- Priority: P1
- Effort: S
- Depends: T-39b-021, area:24-downloads-anime
- Sources: domain/src/main/java/mihon/domain/items/episode/interactor/FilterEpisodesForDownload.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Eligibility rules (favorite, categories default `[0]`, include and exclude lists) pass the logic of FilterEpisodesForDownload.kt:52-67 in tests.
  - Unseen-only filter by episode number (E-REF-4).
  - Empty result queues nothing.

### T-39b-024 — Library add and remove core (favorite flag, cover removal, prompt)
- Priority: P0
- Effort: S
- Depends: area:08-domain-library-category-history-updates, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Add sets favorite true; remove sets false and deletes the cover when `removeCovers` changes the row (behavior 4.3).
  - When remove finds downloads, the delete prompt runs; Delete removes downloads, dismiss keeps them (`T-LIB-4`, E-LIB-7, E-LIB-8).
  - Remove with a failed favorite write does nothing (E-LIB-9).

### T-39b-025 — Duplicate-library check dialog
- Priority: P1
- Effort: M
- Depends: T-39b-024, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt, data/src/main/sqldelightanime/dataanime/animes.sq
- Acceptance:
  - Duplicate query is `favorite = 1 AND LOWER(title) = :title AND _id != :id` and returns the first (`T-LIB-2`).
  - Dialog offers Show anime, Migrate existing entry, Add anyway; "Add anyway" closes the dialog before adding (`T-LIB-3`, fix 23.6).

### T-39b-026 — Category choice on add (default category, no-category and choose branches)
- Priority: P1
- Effort: M
- Depends: area:08-domain-library-category-history-updates, area:37-ui-library
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - The three branches call services in the order of behavior 4.3 (`T-LIB-1`).
  - Choose branch sets the pending-category flag; favorite is set only on confirm (`moveAnimeToCategoriesAndAddToLibrary`) (E-LIB-5).
  - Category write is awaited before favorite on the choose branch (proposal in E-LIB-6); failure keeps favorite false.

### T-39b-027 — Remove-from-library downloads prompt wiring
- Priority: P1
- Effort: S
- Depends: T-39b-024, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Prompt only when `getDownloadCount(anime) > 0` (behavior 4.2).
  - Delete calls `deleteAnime(anime, source, removeQueued = true)` (E-LIB-7).

### T-39b-028 — Custom update interval dialog and expected update
- Priority: P1
- Effort: S
- Depends: area:26-library-update-jobs, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/components/ItemsDialogs.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchInterval.kt
- Acceptance:
  - Index 0 stores 0 (smart); index N stores -N (`T-REF-8`, E-REF-8, E-REF-9).
  - The wheel is always enabled in Kurobayomi (fix 23.8); the gating condition of `isUpdateIntervalEnabled` is kept as a setting, default on.
  - Expected update text follows `AnimeFetchInterval` (area 07).

### T-39b-029 — Skip intro length dialog
- Priority: P2
- Effort: S
- Depends: T-39b-009, area:46-player-controls-ui
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt, app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt
- Acceptance:
  - Initial value rule of E-AIR-2 and range 0..255.
  - OK writes and closes; the change is visible in the player's skip button (area 46).

### T-39b-030 — Dialog state machine (single slot, dismiss, pending track sheet)
- Priority: P1
- Effort: M
- Depends: T-39b-001, T-39b-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - The `Dialog` enum has the 11 variants of behavior 5.1; one slot; the last request wins (`T-MDL-4`, E-UI-2).
  - Dismiss clears the slot; the track sheet re-opens after the category dialog only when the pending flag and the auto-open setting are both true (`T-LIB-5`, fix 23.5).

### T-39b-031 — Full images dialog routing
- Priority: P1
- Effort: S
- Depends: area:34-images-covers-cache, T-39b-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - Cover tap opens the images dialog; the dialog body is provided by the part 1 image model; the routing creates the model with the anime id.
  - Edit calls `EDIT` (file picker), Delete calls `DELETE` (`EditCoverAction`).

### T-39b-032 — Season list item model and season card data
- Priority: P1
- Effort: M
- Depends: T-39b-008, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeSeasonItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Card data: season anime, download count (true value), unseen count, local flag, language, continue flag; hidden overlays are -1 or empty (topic seasons section 5).
  - Download counts refresh when the download queue or cache changes (proposal in topic seasons section 5).

### T-39b-033 — Season navigation
- Priority: P1
- Effort: S
- Depends: T-39b-032, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - Card tap pushes the season's entry page with `fromSource = false` (AnimeScreen.kt:212-214).
  - Continue button opens the season's next unseen episode directly (AnimeScreen.kt:215-222).

### T-39b-034 — Season display modes, grid size and grid binding
- Priority: P2
- Effort: M
- Depends: T-39b-007, area:49-theme-and-design-system
- Sources: domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Four display modes (values 0..3) and grid size 0..15 bind to a `gtk::GridView` min-columns rule (UI parity section 6).
  - Season display-number mode shows "Season N" titles (topic seasons section 2 bit 27).

### T-39b-035 — Related anime sync and display groups
- Priority: P2
- Effort: M
- Depends: area:16-source-api-anime, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncRelatedAnimeWithSource.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - TTL 7 days, max 10 per group, dedupe, self-exclusion, empty groups dropped (`T-REL-1`..`T-REL-4`).
  - Trigger on open (no force) and on manual refresh (force) (behavior 15).
  - Missing source returns a typed result (`T-REL-5`).

### T-39b-036 — Airing countdown: consumer contract and Rust port of loadAiringTime
- Priority: P2
- Effort: M
- Depends: area:52-util-app, T-39b-009, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Decision table of topic airing section 2.3 (`T-AIR-1`); AniList `null` gives (1, 0) (`T-AIR-2`); values parse as `serde_json` (`T-AIR-3`).
  - Simkl calendar parsed as JSON array with earliest upcoming entry per id; fixture from the live schema (`T-AIR-7`, `T-AIR-8`).
  - One shared `reqwest::Client` (`T-AIR-10`); MAL lookup checks HTTP status (`T-AIR-9`).
  - Live check: the countdown row shows for a tracked AniList entry with an upcoming episode.

### T-39b-037 — Airing data overwrite fix and countdown display model
- Priority: P2
- Effort: S
- Depends: T-39b-036, T-39b-038
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Manual refresh uses the populated track items, not an empty list (`T-AIR-4`, fix 23.3, A-1).
  - Completed entries keep stored airing data on automatic updates (A-6).
  - Countdown is `airing_at * 1000 - now`, not clamped; the row is hidden when <= 0 (`T-AIR-5`, E-AIR-10).

### T-39b-038 — Tracking summary state (count, logged-in flag, accepted trackers)
- Priority: P1
- Effort: M
- Depends: area:27-tracking-core, area:28-tracker-anilist
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt
- Acceptance:
  - Count and logged-in flag from the same filter as the home sheet (`T-TRK-12`, D-10).
  - Track items populated from the same observer (fix 23.3 prerequisite).
  - Tracking button shows "Track" when count is 0 and "N trackers" otherwise (UI parity section 2).

### T-39b-039 — Tracking home sheet (bound and unbound rows, refresh on open, private toggle)
- Priority: P1
- Effort: M
- Depends: T-39b-038, area:27-tracking-core, area:53-platform-manifest-intents-workers
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackItem.kt
- Acceptance:
  - Rows per logged-in anime tracker that accepts the source (behavior 2.8); unbound rows show "Add tracking" (`T-TRK-10`).
  - Refresh on open runs `RefreshAnimeTracks` and toasts per failure with the service name (`T-TRK-13`-family, E-TRK-14).
  - Private toggle writes remotely; on failure a toast and no local change (E-TRK-15).
  - Copy link and Open in browser are available when the remote URL is non-blank.

### T-39b-040 — Track status selector
- Priority: P1
- Effort: S
- Depends: T-39b-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt
- Acceptance:
  - Selection starts at the stored status; OK writes it with the completion rule (`T-TRK-1`, E-TRK-5).
  - The write re-reads the local row first (`T-TRK-11`, D-1).

### T-39b-041 — Track episode selector (range rule and progress rules)
- Priority: P1
- Effort: S
- Depends: T-39b-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt
- Acceptance:
  - Range `0..=total` when total > 0, else `0..=10000` (`T-TRK-4`).
  - Setting progress applies the watching and completion rules, stamping the finish date with the current time when completed (`T-TRK-1`, E-TRK-6).

### T-39b-042 — Track score selector
- Priority: P1
- Effort: S
- Depends: T-39b-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt
- Acceptance:
  - Initial value from `displayScore`; OK writes the index-based score (`T-TRK-3`).
  - A string not in the list is rejected with an error, not stored as -1 (E-TRK-7, D-4).

### T-39b-043 — Track start and finish date selectors and remover
- Priority: P1
- Effort: M
- Depends: T-39b-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt
- Acceptance:
  - Validity rules of topic track section 6.1 with `NaiveDate` (`T-TRK-2`, E-TRK-2, E-TRK-3, E-TRK-4).
  - Stored value is local midnight; picker and stored value round-trip (topic track 6.3).
  - Remove button only when the date is set; confirm removes and returns to the home sheet (`T-TRK-*`, popUntil rule).

### T-39b-044 — Track search and bind
- Priority: P1
- Effort: M
- Depends: T-39b-039, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt
- Acceptance:
  - Search runs on open when the query is non-blank; loading state; zero results shows "No results found" (E-TRK-8).
  - Preselect only with a non-null current URL (`T-TRK-5`, D-3); a result without URL is an error row (`T-TRK-6`, D-8).
  - Bind follows the leading-run rule and start-date rule of topic track 7.4 (`T-TRK-14`).
  - Private checkbox only when the tracker supports private tracking (E-TRK-11).

### T-39b-045 — Enhanced tracker registration on search
- Priority: P2
- Effort: S
- Depends: T-39b-044, area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt
- Acceptance:
  - Enhanced trackers call `match` and register the result; no match shows "No match found" (`MR error_no_match`).
  - Only accepted enhanced trackers appear (behavior 2.8).

### T-39b-046 — Remove tracking (local delete and optional remote delete)
- Priority: P1
- Effort: S
- Depends: T-39b-039, area:09-domain-source-track-download-release-misc
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/DeletableAnimeTracker.kt
- Acceptance:
  - Local delete always; remote delete only when the tracker is deletable and the checkbox is set (E-TRK-12).
  - Remote runs before the local row is deleted; both errors are reported (`T-TRK-13`, D-11).

### T-39b-047 — Auto-track prompt on mark seen (ALWAYS, ASK, NEVER)
- Priority: P1
- Effort: M
- Depends: T-39b-013, T-39b-038, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt, app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt
- Acceptance:
  - Prompt conditions and branches of behavior 12, tests `T-MRK-5` (covers E-MRK-7 to E-MRK-10).
  - Success toast only when at least one tracker accepted (`T-MRK-6`, fix 23.12).
  - Offline updates are queued by the delayed-update store (area 27).

### T-39b-048 — Track on add to library (auto-open the track sheet)
- Priority: P1
- Effort: S
- Depends: T-39b-026, T-39b-038
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - Auto-open only when the setting `track.on_add_open_sheet` is true (default true) and at least one logged-in tracker exists (`T-LIB-5`).

### T-39b-049 — Tracking refresh on open and tracker error toasts
- Priority: P1
- Effort: S
- Depends: T-39b-039, T-39b-019
- Sources: app/src/main/java/eu/kanade/domain/track/anime/interactor/RefreshAnimeTracks.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Each failing tracker gives one toast "<name> error: <message>" (`MR track_error`).
  - One failing tracker does not stop the others (E-TRK-14).
  - Refresh runs once per open of the sheet (single-flight per track id, RC-10).

### T-39b-050 — Search and genre-search routing
- Priority: P1
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview, area:41-ui-browse-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - Library search from the library page, source search from a Browse page, and genre search from a Browse page with an HTTP source behave as behavior 18.1 and 18.2 (`T-NAV-1`, `T-NAV-3`).
  - Any other previous screen falls back to global search (`T-NAV-2`, fix 23.14).

### T-39b-051 — Share, copy link and open in browser (desktop replacements)
- Priority: P2
- Effort: S
- Depends: area:53-platform-manifest-intents-workers, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - Share copies the entry URL to the GDK clipboard and shows "Link copied" (UI parity section 4); no system share sheet is used.
  - Open in browser uses `gtk::UriLauncher`; copy-URL errors show a toast and copy nothing (`T-NAV-4`, fix 23.15).
  - Visible only for HTTP sources.

### T-39b-052 — Torrent server start before playback (child process, not an Android service)
- Priority: P2
- Effort: M
- Depends: area:13-core-util-storage-archive-torrent, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt
- Acceptance:
  - Start is attempted only when the torrent setting is on and the source is a torrent source; waits up to 11 one-second polls (behavior 22).
  - Missing extension or missing server binary returns Ok with a toast, never a panic (`T-TRK-9`, fix 23.4).
  - Tracker list is applied after the wait regardless of its result.

### T-39b-053 — Episode swipe actions (per-anime start and end, desktop context menu)
- Priority: P1
- Effort: S
- Depends: T-39b-013, T-39b-015, T-39b-056
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Actions ToggleSeen, ToggleBookmark, ToggleFillermark, Download map as behavior 11.1; Disabled never reaches the model (`T-DL-1`).
  - Setting names are by effect (fix 23.9); defaults keep Bookmark on the start edge and Seen on the end edge.
  - Desktop: right-click menu carries the same actions with keyboard shortcuts of UI parity section 5.

### T-39b-054 — Downloaded-only global override for the downloaded filter
- Priority: P1
- Effort: S
- Depends: T-39b-004, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - When the app-state flag `downloaded_only` is on, the downloaded filter reads EnabledIs for episodes and seasons; stored flags unchanged (`T-EPS-FILT-2`, E-FILTER-5).
  - The UI shows the filter as active.

### T-39b-055 — Download next N and unseen actions
- Priority: P1
- Effort: M
- Depends: T-39b-002, T-39b-003, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt
- Acceptance:
  - NEXT_1, NEXT_5, NEXT_10, NEXT_25 take the first N of the sorted unseen not-downloaded list; UNVIEWED takes all of them (behavior 13.2).
  - "Next N" uses the full episode list, not the filtered view (`T-DL-2`, fix 23.11).
  - Empty list starts nothing.

### T-39b-056 — Episode download actions (start, start now, cancel, delete, qualities)
- Priority: P1
- Effort: M
- Depends: T-39b-002, area:24-downloads-anime, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt
- Acceptance:
  - Each action of behavior 13.1 with the multi-select rules (`T-DL-3`).
  - Cancel returns a new value, does not mutate the shared download (`T-DL-4`, fix 23.13).
  - First download of a non-library entry prompts "Add anime to library?" once per session (`T-DL-5`).

### T-39b-057 — Download status and progress observers
- Priority: P1
- Effort: S
- Depends: T-39b-002, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Status and progress events update only the matching item in place (E-DL-7, RC-6).
  - Season download counts also refresh on these events (T-39b-032 proposal).

### T-39b-058 — Delete episode downloads (confirm and action)
- Priority: P1
- Effort: S
- Depends: T-39b-056, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Confirm dialog "Are you sure you want to delete the selected episodes?" (AYMR confirm_delete_episodes); Confirm clears selection then deletes (behavior 11.3).
  - Local anime: no delete action (E-DL-9).

### T-39b-059 — Local and stub source behavior
- Priority: P1
- Effort: S
- Depends: area:14-source-local-and-metadata, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Local and stub sources hide download actions, web and share controls, and related rows (`isLocalOrStub`, `supportsRelatedAnime` false).
  - Local anime treats all episodes as downloaded (E-FILTER-4).

### T-39b-060 — Downloaded-episode filter for the player playlist
- Priority: P1
- Effort: S
- Depends: area:45-player-core, T-39b-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/episode/EpisodeFilterDownloaded.kt
- Acceptance:
  - Returns all episodes for local anime and only downloaded ones otherwise (`T-EPS-DL-1`, E-DL-10).
  - Used by the player playlist with the same cache rule.

### T-39b-061 — Threading model, executors and cancellation
- Priority: P0
- Effort: M
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - GTK thread owns the model; tokio runtime for I/O; one DB actor thread (`p2-rust-design.md` section 3.1).
  - No RefCell borrow across await: `clippy::await_holding_refcell_ref` is denied in `kuro-core` (checklist item 4 of `p2-edge-cases-and-tests.md` section 9).
  - Page close does not cancel `launchNonCancellable`-equivalent writes; their UI updates are dropped (RC-8).
  - Refresh single-flight per anime id (E-CONC-1).

### T-39b-062 — Unit and integration test suite for the logic layer
- Priority: P0
- Effort: M
- Depends: T-39b-003, T-39b-005, T-39b-006, T-39b-016
- Sources: domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt
- Acceptance:
  - All tests of `p2-edge-cases-and-tests.md` sections 6.1-6.8 that are P0 pass with `cargo test -p kuro-core`.
  - Upstream episode-gap cases ported verbatim (`T-EPS-GAP-1`, `T-EPS-GAP-2`).
  - Fixtures of section 7 are checked in under `kuro-core/tests/fixtures/`.

### T-39b-063 — Replace the static anime details mockup with the model-driven page
- Priority: P0
- Effort: L
- Depends: T-39b-001, T-39b-016, T-39b-019, T-39b-061
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - `src/entry.rs` anime branch (lines 81-240 for `anime == true`) is replaced by `kuro-ui::anime_details::build`; the manga branch keeps working (area 38).
  - Episode list, selection bar, filter sheet, download menu, library button and FAB are bound to the model (UI parity sections 2 and 8).
  - Acceptance test: open a fixture anime, filter to unseen, mark one episode seen, add to library; the UI and DB reflect each step with no GTK warnings.

### T-39b-064 — Episode and season list binding (ListStore, splice, row widgets)
- Priority: P0
- Effort: M
- Depends: T-39b-063, T-39b-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Bulk updates use `gio::ListStore::splice` (one call per list change, pitfall 3).
  - Row widgets rebind on item change without losing focus (`T-UI-2`).
  - Season grid min-columns follows the size setting (`T-UI-3`).

### T-39b-065 — Retire spec-driven anime dialogs and sheets
- Priority: P2
- Effort: S
- Depends: T-39b-063, T-39b-030
- Sources: spec/entries.json, src/spec.rs
- Acceptance:
  - The `anime_entry_details` dialogs and sheets and the anime `tracking_sheet` / `tracker_search` entries are removed from the spec after the typed dialogs are in place; the shared manga entries stay until area 38 migrates.
  - `spec_is_consistent` still passes for all remaining screens.

### T-39b-066 — Drop: Android assist URL (AssistContentScreen)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - Dropped: the `AssistContentScreen` contract and `onProvideAssistUrl` have no Linux equivalent. User loses: the entry URL offered to the Android assistant. Replacement: the copy-link action (T-39b-051).

### T-39b-067 — Drop: haptic feedback on library add
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - Dropped: `HapticFeedbackType.LongPress` on add. User loses: a tactile pulse on phones. Replacement: none; the button state change is the feedback.

### T-39b-068 — Drop: Compose sheet swipe-to-dismiss and Voyager navigator
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - Dropped: `NavigatorAdaptiveSheet` swipe-dismiss and Voyager `Screen`/`Navigator`. User loses: swipe-to-close on the track sheet. Replacement: Escape, close button, and the GTK page stack (`ui::push`, `ui::pop`).

### T-39b-069 — Drop: Voyager-specific behaviors that have no GTK counterpart
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt
- Acceptance:
  - Dropped: `rememberScreenModel` retention across configuration changes. User loses: nothing visible on desktop (window resize keeps state in the GTK object). Replacement: the entry model is held by the page object.

### T-39b-070 — Episode preview thumbnails and summaries toggles
- Priority: P2
- Effort: S
- Depends: T-39b-010, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt
- Acceptance:
  - Two per-entry switches write bits 11 and 12 (`T-FLG-1`); the rich row layout appears only when at least one is on and the source provides the data (part 1 rule).
  - Defaults (both shown) follow the library settings (`T-DEF-4`).
