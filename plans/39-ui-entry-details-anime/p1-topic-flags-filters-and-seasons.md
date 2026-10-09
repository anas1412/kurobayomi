# Topic: entry flags, filters, sorts, display modes and seasons (anime)

Deep dive for the settings windows (episode and season) and the flag words they write. Paths are repo-relative to `/home/blackbox/aniyomi-src`. Full masks are in p1-data-and-config.md section 2; this file explains how they are used.

## 1. Where the flags live

- `Anime.episodeFlags` (column `episode_flags`): filters, sort, display, previews, summaries for episode lists.
- `Anime.seasonFlags` (column `season_flags`): filters, sort, grid display, overlays and grid-title mode for season lists.
- `Anime.viewerFlags` (column `viewer`): skip-intro length and airing data. Read-only in this area (section 6).
- All three are `Long`. Mutation is always "read the word, change masked bits, write the word".

A flag word is never partially written. The Kotlin writes the whole `Anime` through the update interactor (part 2). The Rust write is one `UPDATE animes SET episode_flags = ?1 WHERE _id = ?2` (or `season_flags`) inside a transaction with the bump of `last_modified_at` if the existing code path does so (check area 04 before implementing).

## 2. Write rule

Every dialog control sends `(mask, value)`:

```
new_flags = (old_flags & !mask) | value
```

`value` must be inside `mask`. Examples (from `EpisodeSettingsDialog.kt` and `SeasonSettingsDialog.kt`):

| Control | Mask | Value sent | Notes |
|---|---|---|---|
| Episode unseen "is" | `EPISODE_UNSEEN_MASK` 0x6 | `EPISODE_SHOW_UNSEEN` 0x2 | tri-state "is" |
| Episode unseen "not" | 0x6 | `EPISODE_SHOW_SEEN` 0x4 | tri-state "not" |
| Episode unseen off | 0x6 | 0x0 | tri-state cleared |
| Episode sort by number | `EPISODE_SORTING_MASK` 0x600 | 0x200 | |
| Episode sort direction asc | `EPISODE_SORT_DIR_MASK` 0x1 | 0x1 | direction toggled by the sort row (part 2) |
| Episode previews hide (checkbox currently checked) | `EPISODE_PREVIEWS_MASK` 0x800 | 0x800 | click sends the "not" bit |
| Episode previews show (checkbox currently unchecked) | 0x800 | 0x0 | click sends the zero bit |
| Season started "not" | `SEASON_STARTED_MASK` 0x60 | `SEASON_SHOW_NOT_STARTED` 0x40 | |
| Season sort unseen count | `SEASON_SORT_MASK` 0xE000 | 0x8000 | |
| Season grid mode List | `SEASON_GRID_DISPLAY_MODE_MASK` 0x30000 | 3 << 16 = 0x30000 | value is the enum number shifted |
| Season grid size 4 | `SEASON_GRID_DISPLAY_SIZE_MASK` 0x3C0000 | 4 << 18 = 0x100000 | |
| Season overlay downloaded on | `SEASON_OVERLAY_DOWNLOADED_MASK` 0x400000 | 0x400000 | set bit means on |
| Season grid title "Season number" | `SEASON_DISPLAY_MODE_MASK` 0x8000000 | 0x8000000 | |

Worked example (episode unseen "is" applied to `flags = 0x203`, which is sort by episode number (0x200) with ascending direction (0x1)):
- `0x203 & !0x6 = 0x201`; `0x201 | 0x2 = 0x203`. The result is `0x203` with bit 1 set (unseen is). The sort bits are preserved.

Worked example (season started "not" on `flags = 0x0`): `0x0 & !0x60 = 0x0`; `0x0 | 0x40 = 0x40`.

Tri-state clearing: setting `mask` with `value = 0` clears the filter. The Kotlin dialogs always send the mask and the value together, so a clear is `(mask, 0)`.

## 3. Read rule (model side)

- Tri-state read: `masked = flags & mask`; `masked == is_bit` gives `ENABLED_IS`, `masked == not_bit` gives `ENABLED_NOT`, else `DISABLED` (`Anime.kt:unseenFilter` family).
- Boolean "shown" flags with zero meaning "shown" (previews, summaries): `shown = (flags & mask) == 0`.
- Boolean "on" overlays (set bit means on): `on = (flags & bit) == bit`.
- Enum fields read by shifting: `(flags & mask) >> shift`.
- Sorting: `flags & SORT_MASK` value, direction from `flags & SORT_DIR_MASK` (0 = descending for episodes and seasons).

Default word for a new anime: built from the `default_episode_*` and `default_season_*` preferences (p1-data-and-config.md section 4) through the same write rule, starting from 0. Example: `default_episode_sort_by_ascending_or_descending = EPISODE_SORT_ASC (1)` gives episode direction bit 1.

## 4. Episode settings (summary)

Tabs: Filter (4 tri-states), Sort (4 modes), Display (2 radios + 2 checkboxes). Detailed rows are in p1-behavior.md section 15. The Downloaded row is insensitive when `pref_downloaded_only` is true; the read returns `ENABLED_IS` in that mode regardless of stored bits (`app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:9-16`).

Applied effect (part 2, `app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt:44-54`): `List<EpisodeList.Item>.applyFilters(anime)` runs four sequential `filter` steps (unseen, bookmarked, fillermarked, downloaded) and then sorts with `getEpisodeSort(anime)`. Each step uses `applyFilter(tri) { predicate }` from `domain/src/main/java/tachiyomi/domain/entries/TriState.kt:5-9`: `DISABLED` keeps every row, `ENABLED_IS` keeps rows where the predicate is true, `ENABLED_NOT` keeps rows where it is false. The steps combine with AND.

The downloaded predicate is `it.isDownloaded || isLocalAnime` (local anime always pass when the downloaded filter is on).

Proposal: keep the AND semantics and the local-anime rule exactly. The "any filter active" check (`episodesFiltered()`, `app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:33-38`) is an OR over the four filters and drives only the toolbar's filter badge.

## 5. Season settings (summary)

Tabs: Filter (6 tri-states), Sort (7 modes), Display (chips, size slider, five overlays, grid-title radios). Detailed rows: p1-behavior.md section 16.

Grid display modes (`SeasonDisplayMode`): 0 CompactGrid, 1 ComfortableGrid, 2 CoverOnlyGrid, 3 List. The chip list order is the same as the enum order in the dialog (Compact, Comfortable, Cover-only, List).

Grid size: 0 is Auto (adaptive 128 px columns in grid modes; natural row height in list mode); 1..10 are fixed counts in the slider. The model can store up to 15 (4 bits); values above 10 can only come from a manual DB edit.

Overlay defaults for a new anime (from preferences, p1-data-and-config.md section 4): downloaded off, unseen on, local on, language off, continue-watching on. Existing anime use stored bits only (defect D-6 and E-7 in p1-edge-cases-and-tests.md).

Grid title mode: `SEASON_DISPLAY_MODE_SOURCE` shows the source title; `SEASON_DISPLAY_MODE_NUMBER` shows "Season {n}" (AYMR `display_mode_season`, number through `format_episode_number`).

## 6. Viewer flags (read-only here)

- Skip-intro length (bits 0..7): displayed by the "Change intro length" action in the toolbar (outside this scope). This area only reads it.
- Next episode and airing time (bits 8..23 and 24..55): drive the airing row (p1-behavior.md section 3.8). The writer is the AniChart tracker path (`app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt`), which is not in this scope.
- Intro disable (bit 56): read by the player (not in this scope).

The airing row needs `Airing { episode_number, at_ms }`. `at_ms` is computed by the ScreenModel from `nextAiringEpisode.second` (part 2) and the unit of that value must be confirmed there before the Rust port writes it (open item OD-5 to add).

## 7. Season versus episode fetch type

`Anime.fetchType` (`FetchType.Seasons` = 0, `Episodes` = 1; SQL default 1):
- `Seasons`: the entry page shows the season grid (`sharedSeasons`), the season settings window is used, the episode flags are ignored for display, and `ItemHeader` counts seasons.
- `Episodes`: the entry page shows the episode list; the episode settings window is used; `ItemHeader` counts episodes; the airing row is available.
- The toolbar filter button and the `ItemHeader` click both open `episode_settings` or `season_settings` (mockup spec: `anime_entry_details.sheets`).

Proposal: store the chosen settings window in the screen state so the same button opens the right one after a fetch type change.

## 8. Set-as-default writes

`LibraryPreferences.setEpisodeSettingsDefault(anime)` and `setSeasonSettingsDefault(anime)` copy the current word's bits into the default keys. The episode path has the D-1 defect (chapter keys). The season path writes `default_season_*` keys directly.

The checkbox "Also apply to all anime in my library" applies the same bits to every library anime (part 2). Proposal: apply in one DB transaction and show one snackbar with the count.

## 9. Compatibility notes

- Bits are stable across versions; do not renumber. New features take unused bits (episode: 13..19 and 21..63; season: 28..63; viewer: 57..63).
- Store as signed 64-bit `INTEGER`. The highest used bit is 56 (viewer), so signed storage is safe.
- Backups (area 21/22) carry the same integers; the Rust reader must accept the Kotlin values unchanged.

## 10. Test checklist (links)

- Flag golden values and masks: T-F1, T-F2, T-F9.
- Season enum range: T-F8.
- Overlay creation defaults: T-F5.
- Downloaded-only override: T-F4.
- Zero-means-shown booleans: T-F6, T-F7.
- Sort indicator on the active row only: T-D1.
- Set as default key split: T-M1 (cross-reference) and T-PR1.
