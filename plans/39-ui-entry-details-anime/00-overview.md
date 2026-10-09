# 39-ui-entry-details-anime: area overview (anime entry details screen)

Area lead overview. Scope: the Aniyomi anime details page (`AnimeScreen`), its ScreenModel (`AnimeScreenModel`), the episode and season lists, the settings sheets, the quality window, the images dialog, the tracking sheet and the related and airing sections. All repo paths are relative to `/home/blackbox/aniyomi-src` (read-only, Apache-2.0).

Part 1 (presentation, owned by one author) covers 13 files, 4,977 lines. Part 2 (screen model and logic, owned by the other author) covers 7 files, 3,242 lines. Together that is 20 Kotlin files, about 8,200 lines.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here

The anime details page is the hub for one anime entry. It shows:

- Header: blurred backdrop, cover, title, author, artist, status, source name.
- Action row: start or continue watching, library toggle, tracking, and a download/overflow menu.
- Description (collapsible), genre chips, and a related-anime section.
- Episode list (or season grid for entries whose `fetchType` is `Seasons`), with filter, sort and display settings.
- Per-episode rows with watch progress, bookmark and filler marks, download state, swipe actions, and a multi-select action bar.
- Next-episode airing countdown.
- Dialogs and sheets: category choice, duplicate-add choice, delete downloads, custom update interval, skip-intro length, tracking sheet, quality/hoster window, images (cover and background) viewer.

Source: `app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt` (phone and tablet layouts), `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt` (route host, dialog host, navigation).

### 1.2 Why it matters for a usable Kurobayomi

Without this screen the library and browse flows have no way to reach an episode. Every anime-watching path (library, browse, search, history, updates) ends here or in the player. The screen is therefore P0 for "watch anime from one working source". Its logic carries most of the state rules the user sees: which episodes are listed, which are marked seen, what "continue" opens, which downloads are offered, and when a refresh changes the list.

The current Kurobayomi mockup (`/home/blackbox/kurobayomi/src/entry.rs`, `anime_entry_details` route in `src/screens.rs:39`) is static and renders anime and manga with the same hard-coded code. This area replaces the anime half with a model-driven page.

### 1.3 Explicit scope limits

- Player internals (playback, controls, hoster loading) are owned by `area:45-player-core` and `area:46-player-controls-ui`. This area only starts playback and reads the hoster list for the quality window.
- Tracker services are owned by `area:27-tracking-core` and the `area:28` to `area:33` tracker areas. This area owns the tracking sheet UI and its state.
- Manga details are owned by `area:38-ui-entry-details-manga`. Shared chrome such as the item header is referenced across both.
- Download manager internals are owned by `area:24-downloads-anime` and `area:25-downloads-shared`.
- Airing data comes from the AniList and Simkl HTTP clients in `app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt`, owned by `area:52-util-app`.

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities

| Package or path (repo-relative) | Responsibility | Files in this area |
|---|---|---|
| `app/src/main/java/eu/kanade/presentation/entries/anime/` | Compose presentation: screen layouts, dialogs, settings sheets, episode quality window, duplicate sheet | `AnimeScreen.kt` (1192), `DuplicateAnimeDialog.kt` (127), `EpisodeOptionsDialogScreen.kt` (696), `EpisodeSettingsDialog.kt` (256), `SeasonSettingsDialog.kt` (340) |
| `app/src/main/java/eu/kanade/presentation/entries/anime/components/` | Header, action row, episode row, download indicator, related rows, season tile, base row | `AnimeInfoHeader.kt` (730), `AnimeSeasonListItem.kt` (129), `BaseAnimeListItem.kt` (63), `EpisodeDownloadIndicator.kt` (251), `RelatedAnimeRows.kt` (219), plus the episode row composables listed in `p1-inventory.md` |
| `app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeImagesDialog.kt` | Images viewer (cover and background pages) | Part 1; the model is `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt` (207, listed under the ScreenModel row below) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/` | Route host and ScreenModel (state, actions, refresh, selection, sort/filter, tracking hooks) | `AnimeScreen.kt` (530), `AnimeScreenModel.kt` (1760), `AnimeSeasonItem.kt` (12), `AnimeImageScreenModel.kt` (207) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/` | Tracking sheet and its child screens (status, episode, score, dates, search, remove) | `AnimeTrackInfoDialog.kt` (874), `AnimeTrackItem.kt` (6) |
| `app/src/main/java/eu/kanade/tachiyomi/util/episode/` | Pure episode helpers: next unseen, downloaded filter | `EpsiodeGetNextUnseen.kt` (33; the spelling is in the repo), `EpisodeFilterDownloaded.kt` (27) |
| `domain/src/main/java/tachiyomi/domain/entries/anime/model/` | `Anime` value and bit-flag getters and setters (`episodeFlags`, `seasonFlags`, `viewerFlags`) | `Anime.kt` |
| `domain/src/main/java/tachiyomi/domain/items/episode/model/` and `.../service/` | `Episode`, `EpisodeUpdate`, `EpisodeSorter.kt`, `MissingEpisodes.kt` | used, not owned |
| `domain/src/main/java/tachiyomi/domain/entries/anime/interactor/` | `SetAnimeEpisodeFlags.kt`, `SetAnimeSeasonFlags.kt`; the default writers are injected into the ScreenModel as `setAnimeDefaultEpisodeFlags` and `setAnimeDefaultSeasonFlags` | used, not owned (flag writes are specified in part 2) |
| `domain/src/main/java/aniyomi/domain/anime/` | `SeasonAnime`, `SeasonDisplayMode` | used |
| `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt` | Library and default episode/season settings keys | used (owned by `area:02-preferences-system`) |
| `app/src/main/java/eu/kanade/domain/entries/anime/interactor/` | `SetAnimeViewerFlags.kt` (skip-intro and airing bits), `SyncRelatedAnimeWithSource.kt` | used |
| `app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt` | Remote refresh of cover, background and details | used |
| `app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt` | AniList GraphQL and Simkl calls for airing time | used (owned by `area:52-util-app`) |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/` | `AnimeTracker.kt`, `Tracker.kt`, `TrackerManager.kt`, enhanced and deletable tracker interfaces | used (owned by `area:27-tracking-core`) |
| `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/` | `AnimeDownloadManager` and download state | used (owned by `area:24-downloads-anime`) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/` | `EpisodeLoader`, `HosterLoader` (hoster list and best-video selection) | used (owned by `area:45-player-core`) |
| `app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt` | Source lookup and `isSourceForTorrents()` | used |
| `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/` | `SAnime`, `Video`, `Hoster`, `FetchType` | used (owned by `area:16-source-api-anime`) |

### 2.2 Proposed Rust module map

Source of truth for the Rust layout is `p2-rust-design.md` section 1 (workspace split). `p1-rust-design.md` section 1 proposes a single-crate layout under `src/anime_details/`. Section 4.1 of this overview records the choice.

Proposal (adopted): a workspace split.

```
kuro-core/src/anime/       no GTK; pure logic and services, testable headless
  flags.rs                 episode/season/viewer bit codec, TriState, SeasonDisplayMode
  episodes.rs              Episode, sort, filter, separators, gap rule, next_unseen, filter_downloaded
  selection.rs             selection anchors and toggle/all/invert
  seasons.rs               SeasonAnime, SeasonItem, season filter/sort/overlay
  entry_model.rs           AnimeEntryModel: State, Dialog, actions over services
  library.rs               favorite toggle, duplicate check, category choice, enhanced bind
  refresh.rs               source refresh (episodes and seasons), error mapping, cover rules
  defaults.rs              set-as-default and apply-to-all
  related.rs               related sync (TTL 7 days, max 10 per group)
  airing.rs                airing consumer contract and AniList/Simkl client glue
  viewer.rs                skip-intro and airing writes
kuro-core/src/tracking/    row, service trait, sheet state, dates
kuro-core/src/downloads/   DownloadService trait (subset used here)
kuro-core/src/settings/    typed accessors for the keys in p2-data-and-config.md section 4.5
kuro-ui/src/anime_details/ GTK4 widgets: page, episode_list, season_grid, dialogs, track_sheet, snackbar
```

Rule from `p2-rust-design.md` section 1: everything that decides state lives in `kuro-core`. The GTK layer renders a snapshot and forwards intents.

Alternative (not adopted): the single-crate layout in `p1-rust-design.md` section 1 (`src/anime_details/{model,flags,format,missing,selection,related_pack,download_action,status,source_name,quality,images,view,dialogs}.rs`). It is shorter but cannot run the logic tests without a display.

Shared Rust dependencies (from `p2-rust-design.md` section 4): `gtk4` 0.11.5 with `v4_16`, `glib`, `gio`, `async-channel`, `tokio` (rt-multi-thread, sync, time, macros), `tokio-util` (CancellationToken), `futures`, `rusqlite` (bundled), `reqwest` (rustls-tls, json, default-features off), `serde`/`serde_json`, `chrono`, `icu_collator`, `thiserror`, `tracing`. No libadwaita.

### 2.3 Reading the Kotlin in the right order

1. `presentation/entries/anime/AnimeScreen.kt` for the shape of the page.
2. `ui/entries/anime/AnimeScreen.kt` for what each callback does.
3. `ui/entries/anime/AnimeScreenModel.kt` for state and actions (1760 lines; read in chunks).
4. `domain/.../entries/anime/model/Anime.kt` for the flag layouts.
5. `util/episode/*` and the domain episode service files for the pure rules.

---

## 3. Reading guide

All files are in this folder. Start with this overview, then read the part 1 and part 2 files that match the work you are doing.

### 3.1 Part 1 (presentation): `p1-*`

- [p1-inventory.md](p1-inventory.md): every part 1 file with LOC, declared names, port decision and task IDs.
- [p1-behavior.md](p1-behavior.md): screen layouts, episode row rules, info box, action row, missing-count algorithm, FAB, formatting, settings sheets, images dialog, related packing, description and tags.
- [p1-data-and-config.md](p1-data-and-config.md): part 1 models, bit-flag layouts, enumerations, preference keys, files on disk, endpoints, tables, and the Rust representations.
- [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): defects D-1 to D-16, edge cases E-1 to E-39, race conditions R-*, and open decisions OD-1 to OD-4.
- [p1-rust-design.md](p1-rust-design.md): the single-crate Rust layout for the presentation side, flags and model types, selection, related packing, missing count, formatting, quality state machine, images model, and the Android to Linux mapping.
- [p1-tasks.md](p1-tasks.md): the T-39a task list (69 tasks, linted).
- [p1-ui-parity.md](p1-ui-parity.md): every visible element of the page and its dialogs, snackbars and toasts (exact English text and message keys), gestures, responsive rules, and the mockup gap list.
- [p1-topic-cover-background-dialog.md](p1-topic-cover-background-dialog.md): the images dialog, `AnimeImageScreenModel` save, share, edit and delete flows, and file layout on disk.
- [p1-topic-episode-list-rows.md](p1-topic-episode-list-rows.md): episode row anatomy, constants, info line, summary, bookmark and download icons, download indicator states, swipe, missing and airing rows, selection visuals.
- [p1-topic-flags-filters-and-seasons.md](p1-topic-flags-filters-and-seasons.md): where the flags live, the write and read rules, episode and season settings summary, the season versus episode fetch type, and set-as-default writes.
- [p1-topic-quality-options-dialog.md](p1-topic-quality-options-dialog.md): the quality window model, `loadVideo`, hoster and video states, best-video selection, defects and the Rust state machine.

### 3.2 Part 2 (screen model and logic): `p2-*`

- [p2-inventory.md](p2-inventory.md): the 7 part 2 files with LOC, declared names, port decision and task IDs.
- [p2-behavior.md](p2-behavior.md): the authoritative logic spec (sections 0 to 24): external contracts, construction and init, state model and invariants, library membership, dialog state machine, source refresh, episode pipeline, sort and filters, settings sheet and set-as-default, selection, episode actions, marks and tracker prompt, downloads, seasons, related anime, airing, continue watching, search and share, message keys, side effects, concurrency map, constants, 24 observed defects (23.1 to 23.24), and cross-area references.
- [p2-data-and-config.md](p2-data-and-config.md): part 2 models, bit-flag layouts (`episodeFlags`, `seasonFlags`, `viewerFlags`), enumerations, preference keys with the Kurobayomi key proposal (4.5), files, endpoints, database columns and the Kurobayomi storage proposal (section 8).
- [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md): upstream tests to port, defect cross-reference, edge cases by feature (3.1 to 3.12), failure modes, races, the new Rust test list (6.1 to 6.9), fixtures and builders, acceptance summary.
- [p2-rust-design.md](p2-rust-design.md): the workspace layout (section 1, adopted), type and trait signatures for flags, episodes, seasons, the entry model and tracking, the async model, crates, the Android to Linux mapping, mockup integration, and pitfalls.
- [p2-tasks.md](p2-tasks.md): the T-39b task list (70 tasks, linted).
- [p2-ui-parity.md](p2-ui-parity.md): dialogs, sheets, snackbars and toasts driven by the host, gestures and inputs, responsive and motion behavior, mockup gap analysis, strings, accessibility.
- [p2-topic-airing-and-related.md](p2-topic-airing-and-related.md): viewer flags codec, skip-intro and airing setters, the airing countdown flow with AniList and Simkl calls, and related-anime sync and presentation.
- [p2-topic-episode-list.md](p2-topic-episode-list.md): the episode list pipeline: sort comparator, filters and their source flags, missing-episode separators and the gap rule, selection, next unseen, next-N and unseen download selection, row state derivation.
- [p2-topic-seasons.md](p2-topic-seasons.md): what a season is, season list pipeline (filter, sort, overlay), opening a season, and the season settings.
- [p2-topic-track-sheet.md](p2-topic-track-sheet.md): the tracking sheet home page, status, episode, score and date selectors, search and bind, tracker removal, auto-track rules, navigation graph, defects and the Rust mapping.

---

## 4. Key design decisions and open questions

### 4.1 Decisions already taken in the plan (recommended; the user can overturn)

1. **Crate layout: workspace split (adopted).** Recommendation: `kuro-core` (no GTK) plus `kuro-ui`. Reason: flags, sort, filter, selection, refresh rules and track validity are the highest-risk logic, and they must run under `cargo test -p kuro-core` on a headless machine. The alternative is the single-crate layout in `p1-rust-design.md` section 1. The two documents disagree; the task files reference both sets of paths (`src/anime_details/...` in p1 tasks, `kuro-core/...` in p2 tasks). Before implementation, pick one. Recommendation: workspace split, and make `p1-rust-design.md` section 1 paths read as `kuro-ui/src/anime_details/...` for the widget files and `kuro-core/src/anime/...` for model files.

2. **Fix defects rather than copy them, except where the user decides (4.2).** Each defect in `p2-behavior.md` section 23 and `p1-edge-cases-and-tests.md` section 1 carries a "Proposal:" line. Recommendation: accept every "fix" proposal. Examples with the biggest user impact:
   - 23.1: non-library entries lose per-entry view settings on every open. Fix: apply library defaults once.
   - 23.2: a single toggle replaces the full episode list with the filtered list. Fix: keep the full list and derive the view.
   - 23.10: anime episode defaults share keys with manga chapter defaults (`default_chapter_display_by_name_or_number`, `default_chapter_sort_by_ascending_or_descending`). Fix: separate anime keys (tasks T-39b-010 and T-39a-062).
   - 23.24: a source that returns zero episodes re-fetches on every open. Fix: a 10-minute guard.

3. **Season default writes the wrong filter (23.9).** `setSeasonSettingsDefault` writes `filterSeasonByDownload` from `seasonUnseenFilterRaw` (`LibraryPreferences.kt:331`). Recommendation: fix to `seasonDownloadedFilterRaw` (task T-39b-011).

4. **Auto-download does not apply to season episodes (23.7).** `fetchEpisodesFromSeasons` discards the result (`AnimeScreenModel.kt:678-683`). Recommendation: keep v1 behavior and treat it as a later feature; the season refresh task is T-39b-022 (P1), so decide its acceptance text there.

5. **Custom update wheel (23.8).** The Kotlin hides it in release builds (`app/src/main/java/eu/kanade/presentation/entries/components/ItemsDialogs.kt:126`). Kurobayomi has no release/debug split. Recommendation: always show it (T-39b-028).

6. **Dismiss-then-act ordering (23.6).** The duplicate dialog confirm path does not clear its own dialog slot. Recommendation: dismiss first, then run the action (T-39b-030, T-39a-047).

7. **Track sheet ordering (23.5).** Replace timing-based "track sheet after category dialog" with an explicit pending flag (T-39b-030).

8. **Track data safety (23.17 to 23.20).** Recommendation: stop writing stale track objects (23.19), reject a score not in the list instead of storing -1 (23.20), and require non-null `currentUrl` for preselection (23.18). These are in T-39b-041 to T-39b-044 scope.

9. **Desktop replacements for Android affordances.** Share uses the GTK clipboard and `gtk::UriLauncher` (T-39b-051, T-39a-054). Image share and save use the image model tasks T-39a-045 and T-39a-046. Recommendation: accept.

### 4.2 Open questions for the user (need a yes or no)

| ID | Question | Recommendation | Where |
|---|---|---|---|
| OD-1 | Missing-episode count: keep the Kotlin rule (count over the filtered list, D-10) or count over the unfiltered list? | Unfiltered. "Missing" describes the source's numbering, not the user's view. Record as a documented deviation. | `p1-edge-cases-and-tests.md` OD-1, D-10 |
| OD-2 | Season/related row packing: keep the Kotlin quirk (`packRows` can return more rows than `ceil(total / 3)`, D-12) or balance rows? | Keep the quirk for v1 parity. It is visual only; revisit after daily use. | `p1-edge-cases-and-tests.md` OD-2, D-12 |
| OD-3 | Confirm before deleting a custom image on Linux? | Yes, with a GTK alert dialog. | `p1-ui-parity.md` 9.8 |
| OD-4 | Episode thumbnails and summaries: on or off by default on desktop? (Kotlin: on; `EPISODE_SHOW_PREVIEWS = 0` means shown.) | On, matching Kotlin. The toggle is T-39b-070. | `p1-edge-cases-and-tests.md` OD-4 |
| Q-5 | Torrent server start before playback (`T-39b-052`, starts a torrent server before playback). Do you watch torrent-sourced anime? | Keep it P2. Skip it for v1 unless you use torrent sources. The task is P2 and is not on the P0 path. | `p2-behavior.md` sections 6.9 and 17.5, `p2-tasks.md` T-39b-052 |
| OD-8 | Animated GIF covers: the Kotlin save path decodes one bitmap and drops the animation (`AnimeImageScreenModel.kt:125`). Keep the static save or save the original bytes? | Save the original bytes for GIF so the saved file keeps its animation. The on-screen display stays static. | `p1-topic-cover-background-dialog.md` section 8 |

None of the decisions in 4.1 block the P0 path. OD-1 and OD-2 affect only the missing-count and related-row output. Q-5 affects only a P2 task. OD-8 affects only the image save path.

### 4.3 Drops with user impact (final unless the user objects)

Every Android-only mechanism is dropped or replaced with a named Linux equivalent (P3 tasks, `p1-tasks.md` and `p2-tasks.md`):

| Task | Dropped item | What the user loses |
|---|---|---|
| T-39a-052 | `ignorePadding` full-bleed Layout trick (`AnimeScreen.kt`) | Nothing visible; GTK margins replace it. |
| T-39a-053 | Haptic feedback and preview composables | No vibration on selection or long-press. |
| T-39a-059 | Compose grid item keys | Nothing visible. |
| T-39a-064 | Fast scroller | Scrolling by mouse wheel, scrollbar and keyboard only. |
| T-39b-066 | `AssistContentScreen` (Android assist URL) | Android assistant cannot read the anime URL. |
| T-39b-067 | Haptic feedback on library add | Nothing visible. |
| T-39b-068 | Compose sheet swipe-to-dismiss and Voyager navigator | Sheets and dialogs close by their close button or Escape, not by a swipe gesture. |
| T-39b-069 | Voyager-specific behavior with no GTK counterpart | Nothing visible; back navigation uses the GTK page stack. |

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Need | Owner | Tasks that depend on it (from the CSV depends column) |
|---|---|---|
| Preference store and typed keys | `area:02-preferences-system` | T-39a-004, T-39a-050, T-39b-010, T-39b-054 |
| Anime DB tables and bit-flag columns, DB writes | `area:04-db-anime-schema-migrations`, `area:05-data-repos-entries-items` | T-39a-005, T-39a-046, T-39b-001, T-39b-006, T-39b-009, T-39b-015, T-39b-025, T-39b-032, T-39b-035 |
| Domain entities, episode sort and missing rules, season model | `area:07-domain-entries-items` | T-39a-001, T-39a-002, T-39a-003, T-39b-001, T-39b-020, T-39b-021, T-39b-028 |
| Library categories, favorite handling | `area:08-domain-library-category-history-updates`, `area:09-domain-source-track-download-release-misc` | T-39b-024, T-39b-026, T-39b-046 |
| Downloads: manager, state, queue, cache | `area:24-downloads-anime`, `area:25-downloads-shared` | T-39a-018, T-39a-041, T-39a-044, T-39b-002, T-39b-013, T-39b-023, T-39b-027, T-39b-052, T-39b-055, T-39b-056, T-39b-057, T-39b-058 |
| Source API and source app layer | `area:16-source-api-anime`, `area:17-source-app-layer` | T-39a-042, T-39a-063, T-39b-021, T-39b-035, T-39b-059 |
| Local and stub source behavior | `area:14-source-local-and-metadata` | T-39b-059 |
| Anime extensions | `area:19-extensions-anime` | T-39a-057 |
| Covers and backgrounds cache | `area:34-images-covers-cache` | T-39a-011, T-39a-023, T-39a-045, T-39a-046, T-39a-065, T-39b-021, T-39b-024, T-39b-031, T-39b-070 |
| Tracking: sheet contracts, tracker services, enhanced tracker | `area:27-tracking-core`, `area:28-tracker-anilist`, `area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin` | T-39a-026, T-39b-036, T-39b-038, T-39b-039, T-39b-044, T-39b-045, T-39b-047 |
| Library update jobs (interval) | `area:26-library-update-jobs` | T-39a-026, T-39a-027, T-39b-028 |
| Navigation, WebView, shell, intents | `area:36-ui-shell-navigation-security-webview`, `area:53-platform-manifest-intents-workers` | T-39a-038, T-39a-047, T-39a-069, T-39b-033, T-39b-039, T-39b-050, T-39b-051 |
| Library grid visuals and browse search targets | `area:37-ui-library`, `area:41-ui-browse-anime` | T-39a-030, T-39a-032, T-39a-061, T-39a-068, T-39b-026, T-39b-050 |
| Manga details (shared item header, entry page chrome) | `area:38-ui-entry-details-manga` | T-39a-066, T-39a-075 |
| Player routing, downloaded playlist filter, controls | `area:45-player-core`, `area:46-player-controls-ui` | T-39b-017 (P0), T-39b-060, T-39b-029 |
| Torrent storage and archive helpers | `area:13-core-util-storage-archive-torrent` | T-39b-052 |
| AniList and Simkl HTTP clients (airing) | `area:52-util-app` | T-39b-036 |
| Theme and design system | `area:49-theme-and-design-system` | T-39b-034 |

Note on area 45: `p1-inventory.md` says the player area is "not yet planned under plans/". `plans/45-player-core` now exists. T-39b-017 (P0) depends on it. The quality window's hoster loading (`EpisodeLoader`, `HosterLoader` in `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/`) is named in `p1-inventory.md` but is not in the CSV depends column of T-39a-042 to T-39a-044. Check the player tasks for those contracts before implementing the quality window.

### 5.2 What others need from this area

| Consumer | What it needs | Provided by |
|---|---|---|
| `area:37-ui-library` | "Continue watching" next-unseen target and the anime open route | T-39b-018, T-39b-017 |
| `area:41-ui-browse-anime` | Entry open route, library add core and duplicate check | T-39b-024, T-39b-025, T-39b-026 |
| `area:42-ui-updates-history-stats-storage-upcoming` | Mark-seen semantics that history and updates display | T-39b-013, T-39b-014 |
| `area:45-player-core` | Episode list order, downloaded-only filter for the playlist, and the open-episode routing rule | T-39b-004, T-39b-060, T-39b-017 |
| `area:24-downloads-anime` | Per-episode download actions and the cancel and delete rules | T-39b-055 to T-39b-058 |
| `area:27-tracking-core` | Track sheet state and the track-after-add and mark-seen prompt rules | T-39b-038 to T-39b-049 |
| `area:08-domain-library-category-history-updates` | Category-choice dialog content and remove-from-library downloads prompt | T-39b-026, T-39b-027 |
| `area:34-images-covers-cache` | Images dialog save, share, edit and delete flows | T-39b-031, T-39a-045, T-39a-046 |

### 5.3 Dependency tokens

All `area:` tokens in this folder's tasks files use real folder names (checked against `/home/blackbox/kurobayomi/plans/`). One bad token was fixed in this pass (see section 6.4).

---

## 6. Task summary

### 6.1 Counts

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export` and the rows of `/home/blackbox/kurobayomi/plans/TASKS.csv` whose file starts with `39-ui-entry-details-anime/`. Task IDs: part 1 uses `T-39a-*` (69 tasks), part 2 uses `T-39b-*` (70 tasks).

| Priority | S | M | L | Total |
|---|---|---|---|---|
| P0 | 26 | 18 | 3 | 47 |
| P1 | 43 | 25 | 1 | 69 |
| P2 | 11 | 4 | 0 | 15 |
| P3 | 8 | 0 | 0 | 8 |
| **Total** | **88** | **47** | **4** | **139** |

Effort meaning (from the brief): S at most 1 day, M at most 3 days, L at most 1.5 weeks. There are no XL tasks in this area.

Lint: `p1-tasks.md` and `p2-tasks.md` both print `ok`.

### 6.2 P0 critical path

The P0 set is what lets the user open an anime, watch an episode, and keep progress. The chain below is ordered by dependency. Effort in parentheses.

1. Foundations, no area-39 dependencies except DB and domain: `T-39a-050` preference keys (S), `T-39a-001` episode flags codec (S), `T-39a-002` season flags codec (S), `T-39a-004` TriState mapping (S), `T-39a-005` flag write helper (S, needs `area:04`), `T-39b-019` snackbar and message-key layer (M), `T-39b-061` threading model (M).
2. Episode ordering and filters: `T-39b-006` episode flag codec and SetAnimeEpisodeFlags semantics (M), `T-39b-003` sort engine (S), `T-39b-005` separators and gap rule (S), `T-39b-004` filter engine (S).
3. Refresh and entry model (the long pole): `T-39b-021` source refresh pipeline with the empty-list guard (L), `T-39b-001` entry model with load sequence and first-open refresh decision (L).
4. Episode list model and selection: `T-39b-002` episode list item model and download-state derivation (M, needs `area:24`), `T-39b-016` selection model (M), `T-39b-018` continue watching next unseen (S).
5. Page binding: `T-39b-063` replace the static mockup with the model-driven page (L), `T-39b-064` episode and season list binding (M).
6. Row and chrome widgets: `T-39a-010` episode row simple layout (M), `T-39a-023` info box (M), `T-39a-026` action row (M), `T-39a-037` FAB rule (S), `T-39a-039` bottom action bar predicates (M), `T-39a-007` episode settings window (M), `T-39a-035` wide and narrow layouts (M).
7. Wiring: `T-39a-074` wire the anime details screen into the entry page (M). Also `T-39b-017` episode open routing (S, depends on `area:45-player-core`).
8. Tests that gate the P0 release: `T-39a-051` upstream and unit tests for part 1 (M), `T-39b-062` logic test suite (M).

Longest serial chain by effort: `T-39b-021` (L) to `T-39b-001` (L) to `T-39b-002` (M) to `T-39b-016` (M) to `T-39b-063` (L) to `T-39b-064` (M) to `T-39a-074` (M). That is three L and four M, roughly 3 to 6 weeks of serial work at the brief's upper bounds, before parallel work on the widgets.

Parallel lanes that can start at once: the flag and sort codecs (step 1 and 2), the widget work for the row and info box once `T-39a-001`, `T-39a-022` and `T-39a-025` land, and the dialog work once `T-39b-030` (dialog state machine) lands.

### 6.3 Priority P1, P2, P3 highlights

- P1 includes the quality window state machine (`T-39a-042`, L), the set-as-default dialog (`T-39a-009`) and the season set-as-default logic (`T-39b-011`), the tracking sheet and bind rules (`T-39b-038` to `T-39b-049`; `T-39b-045` is P2), the download actions (`T-39b-055` to `T-39b-058`), and the related rows (`T-39a-030`, `T-39a-031`). The episode set-as-default logic (`T-39b-010`, key-collision fix) is P0.
- P2 includes skip-intro length (`T-39b-029`), season display modes (`T-39b-034`), related sync (`T-39b-035`), airing countdown (`T-39b-036`, `T-39b-037`), torrent start (`T-39b-052`), and episode preview toggles (`T-39b-070`).
- P3 is the drop list in section 4.3.

### 6.4 Plan hygiene fixes made in this pass

1. `p2-behavior.md` section 24 had `area:20 onward` (not a real area id) for the player. Replaced with `area:45-player-core, area:46-player-controls-ui`.
2. `p1-tasks.md` had eight dependency cycles (lint does not check cycles). Each was broken by removing the back-edge, keeping the foundation-first direction (base layer before overlay, model before consumer):
   - `T-39a-042` and `T-39a-043`: removed `T-39a-043` from the quality state machine's depends.
   - `T-39a-042` and `T-39a-060`: removed `T-39a-042` from the error types' depends (now `none`).
   - `T-39a-010` and `T-39a-014`: removed `T-39a-014` from the simple episode row's depends.
   - `T-39a-016` and `T-39a-039`: removed `T-39a-039` from the selection state's depends.
   - `T-39a-015` and `T-39a-066`: removed `T-39a-066` from the missing-count rule's depends (now `none`).
   - `T-39a-011` and `T-39a-012`: removed `T-39a-012` from the rich episode row's depends.
   - `T-39a-030` and `T-39a-031`: removed `T-39a-031` from the related packing's depends.
   - `T-39a-032` and `T-39a-033`: removed `T-39a-033` from the season tiles' depends.
   After the fix, the dependency graph reachable from this area has no cycles, and `tasks.py lint` still prints `ok` for both files. Task IDs are unchanged.
3. `TASKS.csv` was regenerated with `tasks.py export` after these edits.

### 6.5 Overlaps to watch during implementation

These pairs describe the same behavior from two sides. Implement the logic once in `kuro-core` and the widget once in `kuro-ui`:

- `T-39a-007` (episode settings window UI) and `T-39b-010` (episode settings sheet actions and set-as-default logic).
- `T-39a-008` (season settings window UI) and `T-39b-011` (season settings sheet actions).
- `T-39a-074` (wire the anime details screen into the entry page) and `T-39b-063` (replace the static mockup with the model-driven page). Both touch `src/entry.rs`. `T-39b-063` must land first.
- `T-39a-051` and `T-39b-062` (test suites). Put pure-logic tests in `kuro-core/tests/` and widget tests (if any) in `kuro-ui`.
- `T-39a-022` (formatTime) and `T-39a-021` (airing countdown text). Shared `format` helpers belong in one module.

### 6.6 Risks

1. **Layout disagreement between the two Rust designs** (section 4.1). If not settled first, the part 1 and part 2 tasks write files to different places.
2. **Player dependency on the P0 path.** `T-39b-017` (episode open routing) is P0 and depends on `area:45-player-core`. If the player area slips, the user cannot start an episode from this page. Mitigation: a stub route that opens the internal player with a fixed hoster list, behind the same interface.
3. **Source refresh correctness.** `T-39b-021` is an L task that rewrites the refresh rules (manual versus automatic, the empty-list guard, cover and background rules, `updateAnimeFromRemote`). The Kotlin defects 23.7, 23.24 and the `!!` at 23.4 are all in this path. Mitigation: port tests from `p2-edge-cases-and-tests.md` section 1 before rewriting.
4. **Data loss from stale writes.** Track sheet child screens write whole track objects (23.19). Mitigation: `T-39b-041` to `T-39b-044` must send only changed fields and must reject unknown score strings.
5. **Missing-count and packing deviations** (OD-1, OD-2) change visible output. Confirm before writing parity tests for them.
6. **Airing data defects** (`p2-topic-airing-and-related.md` sections 2.7 and 2.8, `removeAiredSimkl` and the offset defect). Owner tasks are T-39b-036 and T-39b-037 (both P2). Mitigation: build a fixture from a real Simkl response before marking T-39b-037 done. The HTTP client is owned by `area:52-util-app`.
7. **Dangling reference, fixed.** `p2-behavior.md` section 24 cited a non-existent `p2-rust-design.md` section 8 for the player. The row now names the player areas directly (section 6.4, item 1).
8. **Mockup replacement scope.** The mockup `src/entry.rs` (585 lines) renders manga and anime with the same code. Replacing the anime half must not break the manga half owned by `area:38-ui-entry-details-manga`. Mitigation: `T-39b-063` changes only the anime branch.

---

## 7. Definition of done for this area in Kurobayomi

The area is done when each behavior below can be observed in the running app on Linux (GTK 4.16+, no libadwaita), and each has a test in `kuro-core/tests/` or in the widget smoke test where the behavior is visual.

### 7.1 Opening and header

1. Opening an anime from the library or browse shows the title, author, artist, status, and source name. A blank title shows "Unknown title" and disables search (`p1-edge-cases-and-tests.md` E-31).
2. The cover and blurred backdrop load from the cache. A missing cover shows the placeholder. A stub source shows the warning icon (E-28).
3. The description collapses to the Kotlin trimming rule and expands with More and Less. Genre chips open a genre search. Long-press on title, author, artist or source copies or searches (T-39a-024).
4. The action row enables and disables buttons by the rules in `p1-behavior.md` section 7. Library toggle opens the category choice for a new favorite (T-39b-026). Duplicate add shows the duplicate sheet (T-39b-025).

### 7.2 Episode list

5. The episode list shows episodes in the order given by the stored sort key and direction. The four sort keys and both directions are in `p2-topic-episode-list.md` section 2.
6. Filters apply as tri-states in the order given by `p2-topic-episode-list.md` section 3.1 (unseen, downloaded, bookmarked, fillermarked, and the rest of that list). In Downloaded-only mode the Downloaded filter is forced on (E-22).
7. Missing-episode separators appear where the numbering has a gap, with the count rule from `p2-topic-episode-list.md` section 4.
8. Each row shows the title (by name or number according to the display mode), the information line, the watch progress string, the bookmark and filler marks, and the download indicator in one of its five states.
9. Long-press and click select; a range selection works from an anchor; select all and invert work (T-39b-016). The bottom action bar shows only the actions allowed for the selection (T-39a-039).
10. Mark seen and unseen update progress. Marking seen removes the download only when the stored setting says so (T-39b-013). Mark previous as seen applies only to a single selection (E-2).
11. Continue watching opens the first unseen episode in display order, after filters and sort (T-39b-018, `p2-topic-episode-list.md` section 6).

### 7.3 Settings and defaults

12. The episode settings window has three tabs, Filter, Sort and Display, and each change is saved to the entry (T-39a-007). "Set as default" writes the anime default keys, and those keys do not collide with manga chapter keys (23.10).
13. Non-library entries keep their view settings within a session and do not lose them on every open (23.1).
14. The season settings window applies the season filter, sort and display modes. Season default writes the downloaded filter from the correct key (23.9).

### 7.4 Seasons

15. An entry with `fetchType` Seasons shows a season grid. Grid columns follow the season grid column rule (E-18 in `p1-edge-cases-and-tests.md`, task T-39a-034). Four display modes work (T-39b-034).
16. Opening a season shows its episodes with the season's own sort and filters.

### 7.5 Refresh

17. Opening an entry refreshes from the source when the first-open rule says so (`p2-behavior.md` section 6.6). A manual refresh replaces the list and reports the error messages in section 6.3 of that document.
18. A source returning zero episodes does not trigger a re-fetch on every open (23.24). The guard is 10 minutes.
19. Torrent server start failure does not crash the refresh (23.4); the entry is still usable.

### 7.6 Downloads

20. Per-episode download actions offer start, start now, cancel, delete and the qualities window, according to the state (T-39b-056). Delete asks for confirmation (T-39b-058).
21. The quality window lists hosters in parallel, auto-selects the best video by the rule in `p1-topic-quality-options-dialog.md` section 8, and does not crash on a missing hoster (D-5). Copy link, download, and play work.
22. Download progress updates the rows live (T-39b-057).

### 7.7 Tracking

23. The tracking sheet lists logged-in trackers with their bound and unbound states. Bind by search, status, episode, score, start and finish dates, and remove (with optional remote delete) all work and write only the changed fields (23.19).
24. Mark seen shows the tracker prompt according to the ALWAYS, ASK or NEVER setting (T-39b-047). The "Trackers updated" toast appears only when a tracker accepted the update (23.12).

### 7.8 Airing and related

25. The airing row shows the countdown to the next episode from the stored airing time (`airingAt` in epoch seconds, multiplied by 1000 for the countdown; `p2-topic-airing-and-related.md` section 2). A past airing time is handled by the countdown rule in T-39a-021 and T-39a-020.
26. Related anime appear in the rows described in section 18 of `p1-behavior.md`. Tapping one opens its entry details (T-39a-031).

### 7.9 Images, share, copy

27. The images dialog shows the cover and background pages. Save, share, edit and delete custom image each work and show the message keys in `p1-ui-parity.md` section 10. Delete asks for confirmation (OD-3).
28. Share and copy link use the GTK clipboard and `gtk::UriLauncher`. A copy failure shows a toast and does not crash (23.15).

### 7.10 Dropped features

29. The Android-only items in section 4.3 are absent, and nothing in the UI refers to them.

### 7.11 Tests

30. `cargo test -p kuro-core` passes with the tests listed in `p2-edge-cases-and-tests.md` section 6 (flags, episodes, selection, seasons, entry model, library, refresh, marks, downloads, tracking, airing and related).
31. Each defect in section 23 of `p2-behavior.md` marked "fix" has a regression test that fails on the Kotlin rule and passes on the Rust rule, where the Kotlin rule is reproducible.
