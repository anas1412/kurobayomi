# Area 39 part 1: implementation tasks (anime entry details)

Format: see `plans/_tools/tasks.py`. Every task lists its Kotlin sources (repo-relative to `/home/blackbox/aniyomi-src`). Behaviour details are in p1-behavior.md; data in p1-data-and-config.md; design in p1-rust-design.md; UI in p1-ui-parity.md; defects and tests in p1-edge-cases-and-tests.md.

Shared sources used in many tasks (not repeated in full):
- `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt` (flag masks and getters)
- `app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt` (screen layouts, bottom menu, FAB, time format)
- `app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeEpisodeListItem.kt` (episode row)
- `app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt` (info box, action row, description, tags)

## Flags, model and preferences

### T-39a-001 — Episode flags codec (EpisodeFlags)
- Priority: P0
- Effort: S
- Depends: area:07-domain-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt
- Acceptance:
  - `EpisodeFlags` exposes every mask and value in p1-data-and-config.md section 2.1 with identical values (golden test T-F2).
  - `sort_descending`, `sorting`, `unseen`, `downloaded`, `bookmarked`, `fillermarked`, `show_previews`, `show_summaries`, `display_by_number` match the Kotlin getters for every golden value.
  - `with(mask, value)` changes only the masked bits (property test T-F9).

### T-39a-002 — Season flags codec and SeasonDisplayMode
- Priority: P0
- Effort: S
- Depends: area:07-domain-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt
- Acceptance:
  - `SeasonFlags` exposes every constant in p1-data-and-config.md section 2.2 (values identical to Anime.kt:257-308).
  - `grid_mode` returns `Err` for an out-of-range value (T-F8) with the message "Invalid sorting value"; `from_i64(0..=3)` maps to CompactGrid, ComfortableGrid, CoverOnlyGrid, List.
  - Overlay bits read as "set means on" (T-F5).

### T-39a-003 — Viewer flags codec (intro length and airing)
- Priority: P1
- Effort: S
- Depends: area:07-domain-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt
- Acceptance:
  - `skip_intro_length`, `next_episode_to_air` (bits 8-23), `next_episode_airing_at` (bits 24-55), `skip_intro_disabled` (bit 56) match the Kotlin getters.
  - Test T-F1 asserts the three masks are disjoint.

### T-39a-004 — TriState mapping and downloaded-only override
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt, app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - Raw masked value maps to `EnabledIs`, `EnabledNot` or `Disabled` exactly as `Anime.unseenFilter` etc. do (T-F2).
  - When `pref_downloaded_only` is true, the Downloaded filter reads `EnabledIs` and its control is insensitive (T-F4).

### T-39a-005 — Flag write helper and persistence
- Priority: P0
- Effort: S
- Depends: T-39a-001, T-39a-002, area:04-db-anime-schema-migrations
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/EpisodeSettingsDialog.kt, app/src/main/java/eu/kanade/presentation/entries/anime/SeasonSettingsDialog.kt, data/src/main/sqldelightanime/dataanime/animes.sq
- Acceptance:
  - Dialog callbacks send `(mask, value)` pairs; the write computes `(flags & !mask) | value` and persists `episode_flags` or `season_flags` in one update.
  - A changed flag re-renders the list without a network call (manual check).
  - Stored value is `INTEGER` 64-bit (no truncation for bit 27 or bit 56).

### T-39a-006 — Season overlay and grid accessors with creation defaults
- Priority: P1
- Effort: S
- Depends: T-39a-002, T-39a-050
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/SeasonSettingsDialog.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Existing anime read stored bits only; no dialog fallback values are used for existing anime (D-6, E-7).
  - New anime get overlay and grid bits from `default_season_*` preferences at creation (T-F5 creation path).

### T-39a-050 — Preference keys and defaults used by this screen
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - Every key in p1-data-and-config.md section 4 exists with the listed default (golden test T-PR1).
  - Swipe accessors name the stored keys exactly: `pref_episode_swipe_end_action` (default ToggleSeen) and `pref_episode_swipe_start_action` (default ToggleBookmark) (T-PR2).
  - The `pref_downloaded_only` key is read through the app-state key wrapper used by Kotlin.

### T-39a-062 — Split episode default keys from manga chapter keys (D-1)
- Priority: P1
- Effort: S
- Depends: T-39a-050
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Episode display and sort-direction defaults use `default_episode_display_by_name_or_number` and `default_episode_sort_by_ascending_or_descending`.
  - Saving episode defaults no longer changes manga chapter defaults (test: set episode default, read chapter default unchanged).
  - First-run migration copies chapter values to the episode keys only when the episode keys are absent and an episode default was saved.

### T-39a-063 — Source display name rules
- Priority: P0
- Effort: S
- Depends: area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt
- Acceptance:
  - `display_source_name` follows p1-behavior.md section 6.3 and passes the table test T-SN1.
  - `toString` form is `"{name} ({LANG})"` with uppercase language code.

### T-39a-060 — Error types and no-panic conversions (D-5)
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/EpisodeOptionsDialogScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt
- Acceptance:
  - No `!!` or unchecked cast remains in the port; each former `!!` returns a typed error shown in the UI (T-Q7).
  - `DetailsError` variants from p1-rust-design.md section 10 exist and log with `tracing::error!`.

## Formatting

### T-39a-022 — formatTime for progress and durations
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - `format_time_ms` matches the boundaries in test T-N3 (`3_600_000` gives `60:00`; `3_600_001` gives `1:00:00`).
  - Output is ASCII (T-N2).

### T-39a-021 — Airing countdown text
- Priority: P1
- Effort: S
- Depends: T-39a-022
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - `format_airing` returns `Airing in %02dd %02dh %02dm %02ds` with the literal English text (T-N4, T-N5).

### T-39a-048 — Episode number formatting
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/util/ItemNumberFormatter.kt
- Acceptance:
  - `format_episode_number(1.0) == "1"`, `12.5 -> "12.5"`, `3.14159 -> "3.142"`, `100.0 -> "100"` (T-N1).
  - Output is ASCII and never has a trailing dot or zero (T-N2).

### T-39a-066 — Item header (count, plural, missing count, filter button)
- Priority: P0
- Effort: S
- Depends: T-39a-015, area:38-ui-entry-details-manga
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/ItemHeader.kt, app/src/main/java/eu/kanade/presentation/entries/components/MissingItemCountListItem.kt
- Acceptance:
  - Count uses `anime_num_episodes` or `anime_num_seasons` plurals with the English text from i18n-aniyomi plurals.xml.
  - Missing text uses `missing_items` plural; hidden when the count is 0.
  - Header is disabled while selecting; tapping it opens the settings window.

## Episode list

### T-39a-007 — Episode settings window (Filter, Sort, Display)
- Priority: P0
- Effort: M
- Depends: T-39a-001, T-39a-004, T-39a-005, area:39-ui-entry-details-anime
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/EpisodeSettingsDialog.kt
- Acceptance:
  - Filter tab has four tri-state rows in order Downloaded, Unseen, Bookmarked, Fillermarked (p1-behavior.md 15.1).
  - Sort tab has four rows with direction shown only on the active row; values match p1-behavior.md 15.2 table.
  - Display tab has two radios and two checkboxes that send the mask values from 15.3.
  - Opened from the header and the filter button; closes on Escape.

### T-39a-008 — Season settings window (Filter, Sort, Display)
- Priority: P1
- Effort: M
- Depends: T-39a-002, T-39a-005, T-39a-006, area:39-ui-entry-details-anime
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/SeasonSettingsDialog.kt
- Acceptance:
  - Filter tab has six tri-state rows in order Downloaded, Unseen, Started, Completed, Bookmarked, Fillermarked (p1-ui-parity.md 9.2).
  - Sort tab has seven rows with the values in p1-behavior.md 16.2 (SEASON_SORT_* masks, 0xE000).
  - Display tab: four grid-mode chips, size slider 0..10 with "Auto" at 0 (label "List size" in list mode, "Items per row" otherwise), five overlay checkboxes with defaults only as stored bits, two grid-title radios (T-F5, T-F6 logic).

### T-39a-009 — Set as default dialog and default writes
- Priority: P1
- Effort: S
- Depends: T-39a-050, T-39a-062, T-39a-006
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/EpisodeSettingsDialog.kt, app/src/main/java/eu/kanade/presentation/entries/anime/SeasonSettingsDialog.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Title, body text and the checkbox "Also apply to all anime in my library" match p1-ui-parity.md 9.3.
  - Confirming with the checkbox off writes only the default keys; with it on, also applies to every anime in the library (part 2 engine).

### T-39a-010 — Episode row, simple layout
- Priority: P0
- Effort: M
- Depends: T-39a-001, T-39a-013, T-39a-022, T-39a-019
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeEpisodeListItem.kt, app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Row padding start 16, top 12, end 8, bottom 12; title one line; seen rows at 0.38 alpha (p1-ui-parity.md 7.1).
  - Click opens the episode when nothing is selected (`alt = false`), toggles when selecting.
  - Renders 3000 rows without a visible stall (T-P1).

### T-39a-011 — Episode row, rich layout (thumbnail and summary)
- Priority: P1
- Effort: M
- Depends: T-39a-010, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeEpisodeListItem.kt
- Acceptance:
  - Rich layout chosen when `preview_url` or `summary` is non-blank, with the title line count 2 when a thumbnail exists.
  - Thumbnail width is `min(40% of window width, 250)`; thumbnails and summaries only when the flags allow them (E-24).

### T-39a-012 — Summary expansion keyed by episode id
- Priority: P1
- Effort: S
- Depends: T-39a-011
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeEpisodeListItem.kt
- Acceptance:
  - Summary shows 3 lines collapsed, 10 px font, 11 px line height; click toggles only when no episode is selected.
  - Expansion survives list recycling (T-R3).

### T-39a-013 — Watch progress string
- Priority: P0
- Effort: S
- Depends: T-39a-022
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Shown only when `!seen && last_second_seen > 0`; text "%1$s/%2$s" of `format_time_ms(last_second_seen)` and `format_time_ms(total_seconds)` (both milliseconds).
  - `total_seconds == 0` renders "m:ss/0:00" without special case (E note in p1-behavior 3.4).

### T-39a-014 — Filler label and bookmark indicator
- Priority: P0
- Effort: S
- Depends: T-39a-010
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeEpisodeListItem.kt
- Acceptance:
  - Filler shows the tag icon and the text "Filler" in the tertiary colour; simple-row title gap becomes 0 when filler.
  - Bookmark shows the filled bookmark icon in primary colour, only when bookmarked.

### T-39a-019 — Episode title display mode
- Priority: P0
- Effort: S
- Depends: T-39a-001
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - When `EPISODE_DISPLAY_NUMBER` is set the title is "Episode {n}" (AYMR `display_mode_episode` with `format_episode_number`), otherwise the episode name.

### T-39a-015 — Missing-count separator row and count rule
- Priority: P1
- Effort: S
- Depends: none
- Sources: domain/src/main/java/tachiyomi/domain/items/episode/service/MissingEpisodes.kt, domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt, app/src/main/java/eu/kanade/presentation/entries/components/MissingItemCountListItem.kt
- Acceptance:
  - `missing_entries_count` matches T-M1..T-M6 (including the four upstream cases).
  - Decision OD-1 (D-10) recorded in settings: header count computed on the filtered list (Kotlin parity) unless changed.
  - Separator is full width and keyed `missing-count-{id}`.

### T-39a-016 — Selection state and gestures
- Priority: P0
- Effort: M
- Depends: T-39a-010
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Long press selects that row (`fromLongPress = true`); click toggles while selecting; click opens otherwise (p1-behavior.md section 2).
  - Escape or back clears the selection first, then navigates up (T-S5..T-S7).
  - Action bar count and cancel, select all and invert behave as the toolbar callbacks.

### T-39a-017 — Swipe actions
- Priority: P1
- Effort: M
- Depends: T-39a-010, T-39a-050
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeEpisodeListItem.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Five action types map to the icons and undo states in p1-behavior.md 3.6 (download icon depends on state).
  - Threshold 56 px; background `primaryContainer`; swipe active while selecting (T-W1).
  - Same five actions available from the row context menu.

### T-39a-018 — Download indicator state machine
- Priority: P1
- Effort: M
- Depends: T-39a-010, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt
- Acceptance:
  - `download_click_action` matches the full matrix in T-DL1 (not-downloaded, queued, downloading, downloaded, error).
  - Indeterminate ring for queued and downloading at 0 progress; determinate ring otherwise; arrow colour flips at 50%.
  - Disabled (insensitive) while selecting and for local anime.

### T-39a-020 — Airing countdown row
- Priority: P1
- Effort: S
- Depends: T-39a-021, T-39a-003, T-39a-035
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt, app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeEpisodeListItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Shown only when airing time > 0, `show_next_episode_airing_time` is true and status != COMPLETED (T-A3).
  - Remaining time re-derived from the absolute target on each tick; reflects a refreshed airing time (D-8, T-A2).
  - The glib timer is removed on destroy (T-A4).

### T-39a-023 — Info box
- Priority: P0
- Effort: M
- Depends: T-39a-025, T-39a-063, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt
- Acceptance:
  - Backdrop (opacity 0.2, gradient, blur substitute), cover (100 px small; 65% large), title, author, artist, status, source per p1-behavior.md section 6.
  - "Unknown title" and "Unknown author" placeholders; artist hidden when equal to author (T-I8, T-I9).
  - Stub source shows the warning icon in the error colour.

### T-39a-024 — Copy and search gestures on title, author, artist, source
- Priority: P1
- Effort: S
- Depends: T-39a-023
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt
- Acceptance:
  - Click title runs global search; click author and artist run global search; click source runs this-source search.
  - Long press copies the text to the clipboard (none when blank).

### T-39a-025 — Status mapping
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt
- Acceptance:
  - Values 0..7 map to the labels and icons in p1-behavior.md 6.3; other values map to "Unknown" and the Block icon (T-St1).

### T-39a-026 — Action row
- Priority: P0
- Effort: M
- Depends: T-39a-027, area:27-tracking-core, area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt
- Acceptance:
  - Library button label and icon switch with favorite state; long press opens the category editor when available.
  - Interval button primary when `fetch_interval < 0`; tracking button shown only when a tracking callback exists; WebView long press only when available.
  - Inactive colour is the foreground at 0.38 alpha.

### T-39a-027 — Next update label
- Priority: P1
- Effort: S
- Depends: area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt
- Acceptance:
  - Null gives "N/A"; 0 days gives "Soon"; N days gives "1 day" or "N days" (plural) using `now.until(next, DAYS)` truncation, coerced at 0 (E-8).

### T-39a-028 — Description collapse and trimming
- Priority: P0
- Effort: S
- Depends: T-39a-023
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt
- Acceptance:
  - Collapsed 3 lines, expanded all; "More" and "Less" toggle; default expanded is `is_from_source` on small layout and always on large (p1-behavior 19.1).
  - Regex `[\r\n]{2,}` becomes one newline; blank becomes "No description" (T-D2, T-D3).

### T-39a-029 — Genre chips and menu
- Priority: P1
- Effort: S
- Depends: T-39a-028
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt
- Acceptance:
  - Chips come from `anime.genre`; collapsed horizontal scroll, expanded wrapping.
  - Chip popover offers "Search" (global search of the tag) and "Copy to clipboard".

### T-39a-030 — Related rows packing and rendering
- Priority: P1
- Effort: M
- Depends: area:37-ui-library
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/RelatedAnimeRows.kt
- Acceptance:
  - `pack_rows` reproduces the Kotlin output including the quirk (T-R1, T-R5); guard for empty input (T-R2).
  - Groups of size 1 to 3 in 3-slot rows with alignment rules; groups of 4 or more in horizontal scrollers with 96 px items.

### T-39a-031 — Related item display modes
- Priority: P1
- Effort: S
- Depends: T-39a-030
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/RelatedAnimeRows.kt
- Acceptance:
  - Four `LibraryDisplayMode` values render comfortable, compact, or cover-only items; favorites use 0.34 cover alpha and an in-library badge.

### T-39a-032 — Season tiles in four display modes
- Priority: P1
- Effort: M
- Depends: T-39a-002, area:37-ui-library
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeSeasonListItem.kt
- Acceptance:
  - Title switches between source title and "Season {n}" (`SEASON_DISPLAY_MODE_NUMBER`); CoverOnly hides the title.
  - List mode uses full-width rows with all badges in one slot.

### T-39a-033 — Season continue overlay and badges
- Priority: P1
- Effort: S
- Depends: T-39a-032
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeSeasonListItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeSeasonItem.kt
- Acceptance:
  - Continue overlay only when the callback exists and `show_continue_overlay` is true (T-G4).
  - Downloads and unseen badges use the `-1` not-computed default; language badge uses `is_local` and `source_language`.

### T-39a-034 — Season grid columns
- Priority: P1
- Effort: S
- Depends: T-39a-032
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Grid size 0 gives `max(1, width / 128)` columns; size n gives n columns (T-G1); list mode spans full width (T-G2).
  - Columns recompute on resize.

### T-39a-035 — Wide and narrow layouts, large-layout fixes
- Priority: P0
- Effort: M
- Depends: T-39a-023, T-39a-010, T-39a-037
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Two panes at width >= 900 px (info and actions left, list right), single column below.
  - Airing row spans full width in both (D-7, T-L5).
  - No horizontal page scroll at 360 px wide.

### T-39a-036 — Title and background fade on scroll (narrow layout)
- Priority: P2
- Effort: S
- Depends: T-39a-035
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Title opacity 1 when first item is not visible; header background 1 when not first item or scrolled (p1-behavior 1.2).

### T-39a-037 — FAB rule and label
- Priority: P0
- Effort: S
- Depends: T-39a-016
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Visible when some unfiltered-list episode is unseen and nothing is selected; label "Resume" when any episode is seen, else "Start" (D-9, T-F3, T-L4).
  - Collapses to icon-only when the list scrolls forward (`shouldExpandFAB` rule).

### T-39a-038 — Refresh replacement
- Priority: P1
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Refresh button in the header and F5 shortcut trigger `onRefresh`; disabled while selecting; spinner while `isRefreshingData`.

### T-39a-039 — Bottom action bar predicates
- Priority: P0
- Effort: M
- Depends: T-39a-016, T-39a-018
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt, app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt
- Acceptance:
  - `bottom_actions` returns exactly the rows of p1-behavior.md section 1.4 for each selection (T-S1..T-S4, T-S8).
  - Bar hidden when selection empty; full width in narrow layout, half width in wide layout.

### T-39a-040 — Mark previous as viewed
- Priority: P1
- Effort: S
- Depends: T-39a-039
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Offered only when exactly one episode is selected (T-S2); applies to the selected episode.

### T-39a-041 — Delete downloaded episodes action
- Priority: P1
- Effort: S
- Depends: T-39a-039, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Offered when any selected is downloaded; deletes all selected ids (E-4); confirmation names the count (Proposal).

### T-39a-047 — Duplicate-add choice window
- Priority: P1
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/DuplicateAnimeDialog.kt
- Acceptance:
  - Three rows (Show anime, Migrate existing entry, Add anyway) and Cancel with the exact texts in p1-ui-parity.md 9.5.
  - Each row dismisses first, then acts (T-DU1).

### T-39a-049 — Keyboard and mouse map
- Priority: P1
- Effort: S
- Depends: T-39a-016, T-39a-039
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Shortcuts from p1-ui-parity.md section 11 are registered on the window and listed in the shortcuts overlay.
  - Escape never clears the selection while a dialog is open.

### T-39a-057 — Settings item only for configurable sources
- Priority: P1
- Effort: S
- Depends: area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Menu item "Settings" appears only when the source implements the configurable interface; it opens the source preferences screen for that source id.

### T-39a-069 — Toasts and snackbars for this area
- Priority: P1
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeImagesDialog.kt
- Acceptance:
  - Toast and snackbar texts match p1-ui-parity.md section 10; snackbars carry a dismiss action.
  - Revealer toast auto-hides after 4 s (Proposal).

## Quality and download window

### T-39a-042 — Quality window state machine
- Priority: P1
- Effort: L
- Depends: T-39a-060, area:16-source-api-anime, T-39a-039
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/EpisodeOptionsDialogScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt
- Acceptance:
  - Loads hoster list, then every hoster in parallel, with `Loading`, `Ready`, `Error`, `Idle` transitions per p1-topic-quality-options-dialog.md.
  - Cancellation on close sends `Idle` to unfinished hosters and no later events (T-Q11).
  - Failure shows toast "No available videos" once and closes (T-Q10).

### T-39a-043 — Preferred video selection and load loop
- Priority: P1
- Effort: M
- Depends: T-39a-042
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt, app/src/main/java/eu/kanade/presentation/entries/anime/EpisodeOptionsDialogScreen.kt
- Acceptance:
  - `select_best_video` passes T-Q1..T-Q5 (two passes; READY or QUEUE usable).
  - `load_video` runs as a loop with restore-on-failure semantics (T-Q6); the first preferred hoster wins the race (T-Q12).

### T-39a-044 — Video actions: copy, download, play
- Priority: P1
- Effort: M
- Depends: T-39a-043, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/EpisodeOptionsDialogScreen.kt
- Acceptance:
  - Actions in order Copy, Use internal downloader, Use external downloader, Play externally, Play internally; all close the window except Copy (T-Q8, T-Q9).
  - "Copied video quality link to clipboard" toast after copy; a video that needs the local HTTP server starts it first (player API) when the flag says so.

### T-39a-045 — Images window (pager, zoom, buttons)
- Priority: P1
- Effort: M
- Depends: T-39a-046, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeImagesDialog.kt
- Acceptance:
  - Two pages (cover, background); switch arrow and labels per T-I10; Edit shows Edit and Delete menu only for custom images (T-I11).
  - Zoom with gesture and Ctrl+scroll; Escape or background click closes.
  - Delete asks for confirmation on Linux (OD-3).

### T-39a-046 — Image model: save, share, edit, delete
- Priority: P1
- Effort: M
- Depends: T-39a-065, area:34-images-covers-cache, area:04-db-anime-schema-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/data/saver/ImageSaver.kt, app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt
- Acceptance:
  - Save writes `cover.<ext>` or `background.<ext>` to the Pictures location; share writes to a temp file; edit and delete bump the matching last-modified value and refresh the view (R-7).
  - Success and failure messages from p1-ui-parity.md section 10.

### T-39a-058 — Image model defect fixes (D-2, D-3, D-4, D-13, D-14, D-15)
- Priority: P1
- Effort: S
- Depends: T-39a-046
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/data/saver/ImageSaver.kt
- Acceptance:
  - Missing image gives an error message, not success (T-I2); background requests use the background URL (T-I3); directory names sanitised (T-I4).
  - Edit stream open failure shows the update-failed message (T-I5); log before snackbar (T-I6); empty share shows the share error (T-I7).

### T-39a-065 — Cache file naming compatibility
- Priority: P1
- Effort: S
- Depends: area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeCoverCache.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeBackgroundCache.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - Custom cover path is `animecovers/custom/<md5(anime_id)>` and background `animebackgrounds/custom/<md5(anime_id)>` under the app data directory (p1-data-and-config.md section 5).
  - A file produced by the Android app is found without renaming (fixture test).

## Shell wiring

### T-39a-074 — Wire the anime details screen into the entry page
- Priority: P0
- Effort: M
- Depends: T-39a-035, T-39a-007, T-39a-010, T-39a-023, T-39a-026, T-39a-039
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt, src/entry.rs
- Acceptance:
  - The anime branch of the entry page is built by the new module; the manga branch is unchanged (T-39a-074 check: manga page layout unchanged in a snapshot).
  - Mockup placeholders (fixed episodes, fixed strings, fixed genres) are gone from the anime path.

### T-39a-075 — Toolbar items for anime entries
- Priority: P1
- Effort: S
- Depends: T-39a-038, area:38-ui-entry-details-manga
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Menu items Share, Refresh, Edit categories, Migrate, Change intro length, Settings (configurable only) are wired to real actions; Download menu offers Next 1, 5, 10, 25 and Unwatched.

### T-39a-061 — Shared 76 dp anime row for the migration screen
- Priority: P2
- Effort: S
- Depends: area:37-ui-library
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/BaseAnimeListItem.kt
- Acceptance:
  - Row height 76, vertical padding 3, horizontal padding 16; cover, title and actions slots; used by migration screens.

### T-39a-054 — Share and clipboard replacement
- Priority: P2
- Effort: S
- Depends: T-39a-046
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt, app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt
- Acceptance:
  - Share opens `gtk::FileLauncher` or the default app; clipboard uses `gdk::Clipboard::set_text`.

### T-39a-055 — Desktop-only selection additions
- Priority: P2
- Effort: S
- Depends: T-39a-016
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Shift+click selects the range from the anchor; Ctrl+I inverts; both documented as desktop additions.

### T-39a-068 — List-mode row height from size slider
- Priority: P2
- Effort: S
- Depends: area:37-ui-library, T-39a-032
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeSeasonListItem.kt
- Acceptance:
  - Row height follows the Kotlin `entries` and container-height rule (defined in the shared list component); the size slider changes the visible rows.

## Drops (Android-only or superseded)

### T-39a-052 — Drop the ignorePadding layout trick
- Priority: P3
- Effort: S
- Depends: T-39a-035
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - Replaced by GTK margins on each row; visible width of info, action row, description and rows matches the 14 px gutter (manual check).

### T-39a-053 — Drop haptic feedback and preview composables
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeEpisodeListItem.kt, app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - No haptic calls; `AnimeEpisodeListItemPreview` is not ported. Decision recorded: Linux has no haptic API used by this app.

### T-39a-059 — Drop Compose grid item keys
- Priority: P3
- Effort: S
- Depends: T-39a-035
- Sources: app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt, app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - `EntryScreenItem` keys are replaced by GTK model identity (`EpisodeRowObject` and season anime id); no enum is ported.

### T-39a-064 — Drop the fast scroller
- Priority: P3
- Effort: S
- Depends: T-39a-035
- Sources: app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt
- Acceptance:
  - GTK overlay scrollbars are used; no custom fast scroller is built. Decision recorded.

## Tests and verification

### T-39a-051 — Port upstream tests and write unit tests for this area
- Priority: P0
- Effort: M
- Depends: T-39a-001, T-39a-002, T-39a-015, T-39a-022, T-39a-048, T-39a-030, T-39a-039, T-39a-043
- Sources: domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt
- Acceptance:
  - All tests named in p1-edge-cases-and-tests.md section 5 exist and pass with `cargo test` (flags, format, missing, related packing, selection, download, quality model, image model).
  - Fixtures F-1..F-9 are checked in under `tests/fixtures/anime_details/`.
