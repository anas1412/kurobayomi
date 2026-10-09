# 45-player-core (part 2 of 2) — Implementation tasks

Task format per `/home/blackbox/kurobayomi/plans/_tools/tasks.py`. IDs `T-45b-NNN`. Sources are repo-relative to `/home/blackbox/aniyomi-src` unless they start with `kurobayomi/` (then relative to `/home/blackbox/kurobayomi`). Behavior references `§n` point to `p2-behavior.md`; `E-nn` to `p2-edge-cases-and-tests.md`; `D§n` to `p2-data-and-config.md`; `R§n` to `p2-rust-design.md`; `U§n` to `p2-ui-parity.md`.

Dependency tokens: `T-45b-NNN` for in-area tasks, `area:<id>` for other areas.

## A. Core state and session

### T-45b-001 — Player domain types (Video, Hoster, HosterState, chapters, thumbnails)
- Priority: P0
- Effort: M
- Depends: area:16-source-api-anime, area:17-source-app-layer
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Hoster.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/ThumbnailInfo.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/QualitySheet.kt
- Acceptance:
  - `src/player/types.rs` defines `Video` with every field and default in D§1.1 and `VideoState` with variants `Queue, LoadVideo, Ready, Error`.
  - `Hoster`, `HosterState` (`Idle/Loading/Error/Ready`), `ChapterType` (`Opening, Ending, Recap, MixedOp, Other`), `TimeStamp`, `ThumbnailInfo`, `TileInfo` match D§1.2-1.4; `HosterState::with_video` replaces one slot in both lists (QualitySheet.kt:62-72).
  - Unit test: `Video::is_lazy()` is true only for the literal `"null"` (E-32 handling).

### T-45b-002 — Player state store and change notification
- Priority: P0
- Effort: L
- Depends: T-45b-001, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `src/player/model.rs` holds every field of R§5 (`PlayerModel`), all initial values from PlayerViewModel.kt:180-326 (e.g. `paused_state = Some(false)`, `selected_sub = (-1,-1)`, `selected_audio = -1`, `controls_shown = !hide_controls`).
  - Every mutation goes through `Store::update`; a subscriber receives one `Change` per changed field group; a subscriber that mutates the store is rejected (queued via `glib::idle_add_local_once`).
  - Test: setting `sheet` to non-None clears `panel` and `dialog` (exclusivity, behavior §10).

### T-45b-003 — Session init and episode resolution
- Priority: P0
- Effort: M
- Depends: T-45b-002, T-45b-004, T-45b-005, T-45b-007, T-45b-030, area:05-data-repos-entries-items, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - `session::init` implements behavior §2.2 step by step, including the fast path when anime and episode ids are unchanged (E-01) and the mpv user-data writes `user-data/current-anime/{anime-title,intro-length,category}` (D§7.2).
  - Error keys: "No episode loaded", "Hoster selected from empty list", "Hoster list is empty" (`PlayerError::Keyed`, behavior §18); anime missing returns the E-02 state.
  - Test with fakes: anime missing → `AnimeMissing`; hoster list empty → `NoHosters`.

### T-45b-004 — Playlist build and episode sort (incl. SOURCE inversion)
- Priority: P0
- Effort: S
- Depends: area:07-domain-entries-items, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt
- Acceptance:
  - `playlist::build` sorts with `episode_sort(anime, sort_descending = false)` (PlayerViewModel.kt:1340) and keeps the SOURCE inversion (behavior §2.6) — test T11 in E edge doc.
  - `pref_downloaded_only` (`__APP_STATE_pref_downloaded_only`, default false) filters to downloaded episodes.
  - Invalid sorting value returns the same "Invalid episode sorting method" error class.

### T-45b-005 — Playlist filter (episode_flags predicate, selected-kept rule)
- Priority: P0
- Effort: M
- Depends: T-45b-004, area:07-domain-entries-items, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt
- Acceptance:
  - `playlist::filter` implements the eight-clause OR predicate of behavior §2.4 with masks from D§3.1 (`0x6, 0x18, 0x60, 0x180`).
  - The selected episode is always present; it is appended last when filtered out.
  - Tests T6-T10 (E doc §4.2) pass; T10 pins clause semantics.

### T-45b-006 — Adjacent episodes, prev/next and messages
- Priority: P0
- Effort: S
- Depends: T-45b-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `get_adjacent_episode_id` returns -1 at the ends and -1 for index -1 cases exactly as behavior §2.7.
  - Prev with no previous shows "no_prev_episode"; next with none shows "Next Episode not found!" (behavior §18); neither changes the episode.

### T-45b-066 — Session serialization and cancel-on-select
- Priority: P0
- Effort: M
- Depends: T-45b-002, T-45b-013, T-45b-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - At most one `load_video` is in flight at a time (session `tokio::sync::Mutex`); a user selection cancels the previous load token (E-38).
  - Clicking a video while `is_loading_episode` is true is ignored unless it is a deliberate re-selection (E-40); test T68.

### T-45b-030 — Process-restart snapshot and shutdown save
- Priority: P0
- Effort: S
- Depends: T-45b-026, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `session_file.rs` writes `$XDG_STATE_HOME/kurobayomi/player-session.json` with `episode_id`, `episode_position_ms`, `quality_index` (R§13), debounced to 5 s, and restores them only when the episode id matches.
  - On window close the final progress save runs with a 2 s deadline and the runtime shuts down with a 3 s timeout (E-29).

## B. Hosters, videos and loading

### T-45b-007 — Hoster list per episode kind (downloaded / HTTP / local)
- Priority: P0
- Effort: L
- Depends: T-45b-001, T-45b-008, T-45b-063, T-45b-064, area:16-source-api-anime, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt
- Acceptance:
  - `loader::episode::get_hosters` dispatches in the order downloaded → HTTP → local, else error "source not supported" (behavior §3.1).
  - `is_download` uses a disk check without cache (`skipCache = true`, behavior §3.2).
  - HTTP path calls hoster list (sorted) when the source has hosters, else the legacy video list (`to_hoster_list`).

### T-45b-008 — Source capability: has_hoster_list and legacy video path
- Priority: P0
- Effort: S
- Depends: T-45b-001, area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt
- Acceptance:
  - `AnimeSourceApi::has_hoster_list` replaces the reflection walk (EpisodeLoader.kt:60-78); every compiled-in source returns the same answer the reflection gave (test table per source class).
  - Legacy path is marked `// compat: ext-lib<1.6` and is reachable only when `has_hoster_list` is false.

### T-45b-009 — Video list, lazy URL resolution and sorting
- Priority: P0
- Effort: M
- Depends: T-45b-001, T-45b-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt
- Acceptance:
  - `get_videos` implements behavior §3.6; `parse_video_urls` resolves only videos whose URL is the lazy sentinel (§3.7).
  - Sorting is delegated to the source (`sortVideos`) for HTTP sources only.
  - Test: a non-lazy URL is not resolved (no call to `video_url`).

### T-45b-010 — select_best_video
- Priority: P0
- Effort: S
- Depends: T-45b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt
- Acceptance:
  - Preferred pass then first non-empty pass, each over Ready hosters in order, states READY or QUEUE only (behavior §3.9).
  - Tests T1-T5 (E doc §4.1) pass.

### T-45b-011 — get_best_video race for the external-player path
- Priority: P1
- Effort: M
- Depends: T-45b-010, T-45b-012, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - Hosters load in parallel (`JoinSet`); the first preferred resolved non-empty video wins and cancels the rest (behavior §3.11 step 2a).
  - Fallback loop returns the first resolvable video from `select_best_video` or `None`; caller shows "Video list is empty" on None.

### T-45b-012 — Resolve video (getResolvedVideo)
- Priority: P0
- Effort: S
- Depends: T-45b-001, area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt
- Acceptance:
  - HTTP sources resolve only when `initialized == false`; resolve errors become `None`, cancellation is propagated (behavior §3.10).
  - Returned video has `initialized = true`.

### T-45b-013 — load_hosters concurrency and automatic selection
- Priority: P0
- Effort: L
- Depends: T-45b-007, T-45b-010, T-45b-014, T-45b-066
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Initial hoster states: lazy → Idle, null video list → Loading, else Ready with all QUEUE (behavior §4.1 step 3a).
  - Saved (hoster, video) selection is tried first; otherwise first preferred video; otherwise best; no videos → "No available videos" (behavior §4.1 step 3c).
  - Cancel resets all hosters to Idle (T67).

### T-45b-014 — load_video state machine
- Priority: P0
- Effort: M
- Depends: T-45b-012, T-45b-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Implemented as a loop (not recursion) with the rules of behavior §4.2 steps 1-8, including the "still loading → false" branch and the "something already playing → restore old selection" branch.
  - Terminates: each failure marks one slot ERROR (E-37).
  - Test T64-T66 pass.

### T-45b-015 — Video and hoster click handling
- Priority: P0
- Effort: M
- Depends: T-45b-014, T-45b-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `onVideoClicked` ignores ERROR videos and closes the QualityTracks sheet after success (behavior §4.3).
  - `onHosterClicked`: Ready toggles the accordion, Idle loads with force, Loading/Error are no-ops in parity mode (E-08 retry is the documented improvement and is behind flag `hoster_retry`).

### T-45b-063 — Local source hoster
- Priority: P1
- Effort: S
- Depends: T-45b-007, area:14-source-local-and-metadata
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt
- Acceptance:
  - Resolves `<anime dir>/<episode file>` from the local source base directory; produces `Video(videoUrl = file uri, videoTitle = "Local source: <episode.url>")` (behavior §3.5).
  - Missing file → empty list → "Hoster list is empty" (E-34).

### T-45b-064 — Downloaded episode hoster
- Priority: P0
- Effort: S
- Depends: T-45b-007, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt
- Acceptance:
  - `build_video(source, anime, episode)` result is wrapped as a single hoster; any error yields an empty list (behavior §3.4).
  - A downloaded episode plays with no network call.

## C. Tracks and chapters

### T-45b-016 — Track list load and default track selection
- Priority: P0
- Effort: M
- Depends: T-45b-017, T-45b-073
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Reads `track-list/count` and per-track `type/lang/title/id` (D§7.1); audio list starts with `VideoTrack(-1, "Off", None)`; subtitles do not (E-31 default).
  - Preferred track applied via `TrackSelect`; subtitle fallback is the first subtitle track; audio fallback is index 1 (behavior §5.2).
  - An mpv NPE-equivalent (handle gone) clears lists (E-42).

### T-45b-017 — TrackSelect (language, whitelist, blacklist)
- Priority: P0
- Effort: M
- Depends: T-45b-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/TrackSelect.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/SubtitlePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt
- Acceptance:
  - Preference strings are trimmed before empty filtering (E-21 fix); language match uses whole-token anchored matching (E-22 fix).
  - Locale display names via `icu_displaynames`; fallback to the tag when unavailable; system locale used when language prefs are empty.
  - Tests T14-T21 pass.

### T-45b-018 — Chapter list load, current chapter and selection
- Priority: P0
- Effort: S
- Depends: T-45b-001, T-45b-073
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `chapter-list/count` (missing → zero, E-41), titles and times read; list sorted by start (behavior §6.1).
  - `update_chapter(0)` after chapter-list; `select_chapter` seeks exactly (`absolute`).
  - `current_chapter` is the last chapter with start ≤ position (behavior §6.8 getCurrentChapter).

### T-45b-019 — Merge external chapters with mpv chapters
- Priority: P1
- Effort: M
- Depends: T-45b-018, T-45b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/ChapterUtils.kt
- Acceptance:
  - `chapters::merge` implements behavior §6.4 steps 1-8 exactly; tests T22-T28 pass.
  - External start colour is ARGB `0xFFD8BBDF`; `ChapterType::Other` uses unspecified colour.

### T-45b-069 — Chapter type labels and skip label text
- Priority: P1
- Effort: S
- Depends: T-45b-061
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/ChapterUtils.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `ChapterType → message key` table matches behavior §6.7 (`Opening`, `Ending`, `Recap`, `MixedOp` → "Mixed opening").
  - `Other` returns no label; skip label uses `player_skip_action` with the type label.

### T-45b-020 — AniSkip client
- Priority: P1
- Effort: M
- Depends: T-45b-001, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/AniSkipApi.kt
- Acceptance:
  - GET `https://api.aniskip.com/v2/skip-times/{malId}/{ep}?types[]=ed&types[]=mixed-ed&types[]=mixed-op&types[]=op&types[]=recap&episodeLength={len}` through the shared client (E-05).
  - Decodes the live response shape (D§1.5); unknown `skipType` values map to Other and keep the interval (E-04); any error → None.
  - Test T36-T38 pass with the recorded fixtures.

### T-45b-021 — AniSkip tracker selection and AniList→MAL lookup
- Priority: P1
- Effort: S
- Depends: T-45b-020, area:27-tracking-core, area:28-tracker-anilist, area:29-tracker-myanimelist
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/AniSkipApi.kt
- Acceptance:
  - Chooses the first track whose tracker yields a non-zero MAL id (MAL remote id, or AniList lookup) and queries AniSkip once (E-03 fix).
  - AniList lookup parses `data.Media.idMal` with serde; `null` or failure → no AniSkip call (T39, T40).
  - Duration is whole seconds from the player; missing duration → no call.

### T-45b-022 — Skip-intro state machine
- Priority: P1
- Effort: L
- Depends: T-45b-018, T-45b-021, T-45b-047, T-45b-048, T-45b-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - `skip::SkipIntro` implements behavior §6.8: netflix countdown (default 5), auto skip, manual label, cancel on tap, reset on non-intro chapter, reset on chapter change (E-09).
  - Decrements only on whole-second ticks, not on sync calls (E-10); tests T29-T35 pass.
  - Visible strings match U§1.2.

## D. Seeking, gestures and controls

### T-45b-023 — Seek primitives and double-tap accumulator
- Priority: P0
- Effort: M
- Depends: T-45b-073, T-45b-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `seek_by`/`seek_to` use exact flag strings of behavior §8.2 (`relative`, `relative+exact`, `absolute`, `absolute+keyframes`).
  - `seek_to` out of range is ignored; queued when duration is 0 (E-11).
  - Back-tap accumulator adds (E-19 fix); tests T41-T44 pass.

### T-45b-024 — Double-tap and single-action gesture dispatch
- Priority: P0
- Effort: S
- Depends: T-45b-023, T-45b-047, T-45b-067
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - Left/center/right dispatch tables of behavior §8.3 are implemented; center has no-op Seek/Switch.
  - Test T45 covers every enum value on each side.

### T-45b-025 — Seek preview thumbnails
- Priority: P1
- Effort: M
- Depends: T-45b-001, T-45b-012, area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Tile LRU capacity 3; first 2 tile maps preloaded on episode load (behavior §4.7).
  - Fetch throttle 2 s with trailing-edge fire (E-20); tile results tagged with episode generation (race 5).
  - Crop clamped; invalid tile indexes dropped (E-28, E-30).

### T-45b-051 — Controls visibility state machine and exclusivity
- Priority: P0
- Effort: M
- Depends: T-45b-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - `show_controls` refuses while a sheet, panel or dialog is open; `show_sheet/panel/dialog` clear the other two (behavior §10).
  - Defaults: controls shown unless `player_hide_controls`; seek bar shown likewise.

### T-45b-052 — Lock controls
- Priority: P1
- Effort: S
- Depends: T-45b-051
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `lock_controls`/`unlock_controls` toggle `controls_locked`; while locked only the unlock affordance is visible (U§1.5).

### T-45b-057 — Keyboard and MPRIS input mapping
- Priority: P1
- Effort: M
- Depends: T-45b-024, T-45b-038, T-45b-074
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Media previous/play-pause/next (`pref_media_*`) exposed over MPRIS with the three configured actions.
  - Keyboard map per U§2: arrows ±skip length, Space pause, Up/Down volume; documented conflicts with KDE shortcuts; all bindings are configurable.

### T-45b-058 — Orientation cycling dropped; fullscreen replaces it
- Priority: P3
- Effort: S
- Depends: T-45b-059
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - `cycle_screen_rotations` has no Linux implementation; the `pref_default_player_orientation_type_key` value is kept in the store but unused (drop reason: Android requested-orientation API).
  - The fullscreen toggle (T-45b-059) is the replacement; the README-style changelog line states the loss: "rotation lock is no longer available".

### T-45b-059 — Fullscreen preference
- Priority: P1
- Effort: S
- Depends: T-45b-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - `player_fullscreen` (default true) controls whether starting playback enters fullscreen (`gtk::Window::fullscreen`); toggling mid-playback works with the same key.

## E. Progress, history, completion, downloads

### T-45b-026 — Progress save and resume
- Priority: P0
- Effort: M
- Depends: T-45b-028, T-45b-073, T-45b-048, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, data/src/main/sqldelightanime/dataanime/episodes.sq
- Acceptance:
  - `on_second` implements behavior §7.1 steps 1-9 (ms math, `progress_preference` default 0.85).
  - Save writes only changed fields (E-46); DB columns and triggers match D§8.1.
  - Resume from `episode_position` when `pref_preserve_watching_position` is true (default false → start at 0 per Kotlin's use of `episodePosition` restore; document).

### T-45b-027 — Completion side effects and duplicate marking
- Priority: P0
- Effort: M
- Depends: T-45b-026, T-45b-029, T-45b-056, area:27-tracking-core, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Completion runs once per episode (E-14), sets `seen` in memory before the save.
  - `mark_duplicate_seen_episode_seen` containing `existing_episode` marks unseen recognised episodes with the same number in the filtered playlist (behavior §7.2, E-50).
  - Tracker update only when not incognito-global, has trackers, and `pref_auto_update_manga_sync_key` (default true); test the incognito matrix (E-17).

### T-45b-028 — History upsert and incognito gating
- Priority: P0
- Effort: S
- Depends: T-45b-056, area:06-data-repos-history-track-source-misc
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, data/src/main/sqldelightanime/dataanime/animehistory.sq
- Acceptance:
  - Upsert `ON CONFLICT(episode_id) DO UPDATE SET last_seen = :seenAt` (D§8.2) with `seenAt = now`.
  - Writes coalesced to every 5 s plus pause/close (E-47); not written in incognito (T56/T57).

### T-45b-029 — Download-ahead and delete-after-read
- Priority: P1
- Effort: M
- Depends: T-45b-026, area:24-downloads-anime, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - Download-ahead starts past 0.35 of duration only when current and next are downloaded, takes `auto_download_while_watching` next episodes, once per episode (E-13); tests T60, T61.
  - Delete-after-read uses `remove_after_read_slots` (default -1) with the index rule of behavior §7.6; tests T59.

### T-45b-040 — Bookmark and fillermark
- Priority: P1
- Effort: S
- Depends: T-45b-002, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `bookmark_episode`/`fillermark_episode` update the DB and the in-memory episode together (E-15); test T62.
  - Null episode id → no-op with a log line (instead of Kotlin `!!` crash).

## F. Volume, brightness, aspect, decoder and audio/video prefs

### T-45b-031 — Volume control with mpv boost
- Priority: P0
- Effort: M
- Depends: T-45b-048, T-45b-073, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt
- Acceptance:
  - `change_volume_by` implements behavior §9.1 with the E-06 rule; boost cap default 30 (`pref_audio_volume_boost_cap`); `volume-max = 100 + cap`.
  - System volume via a `SystemVolume` trait backed by PipeWire (`wpctl`) with PulseAudio (`pactl`) fallback; tests T46-T49.

### T-45b-032 — Brightness via mpv filter
- Priority: P0
- Effort: S
- Depends: T-45b-046, T-45b-073
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - State clamped to -0.75..1; mapped to mpv `brightness` in -100..100 (behavior §9.2 proposal).
  - Replaces `Settings.System.SCREEN_BRIGHTNESS`; drop note: no window brightness API on Linux, so the Android window backlight behavior is lost.

### T-45b-033 — Aspect modes
- Priority: P0
- Effort: S
- Depends: T-45b-048, T-45b-073
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - Table of behavior §9.3: Fit (`panscan 0`, override -1), Crop (`panscan 1`, override -1), Stretch (monitor ratio, `panscan 0`).
  - Persists `pref_player_aspect_state`; the pill text is one of the three AYMR video mode strings; tests T50-T52.

### T-45b-038 — Playback speed and presets
- Priority: P0
- Effort: S
- Depends: T-45b-048, T-45b-073
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - `playback_speed` initialised from `pref_player_speed` (default 1.0); presets from `default_speed_presets` (12 values, D§4.5); default preset list used when the stored set is absent.
  - Speed changes write mpv `speed` through the observer round trip.

### T-45b-044 — Subtitle style and delay to mpv options
- Priority: P0
- Effort: M
- Depends: T-45b-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/SubtitlePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt
- Acceptance:
  - Every subtitle pref in D§4.6 is applied as an mpv `sub-*` option with the Kotlin-equivalent value (ARGB conversion for colours, border style values `outline-and-shadow`, `opaque-box`, `background-box`).
  - Subtitle delay and secondary delay are in milliseconds; defaults 0.

### T-45b-045 — Audio prefs to mpv
- Priority: P1
- Effort: S
- Depends: T-45b-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt
- Acceptance:
  - `audio-channels` values (`auto-safe`, `auto`, `mono`, `stereo`) and the `af` reverse-stereo pan filter are applied per D§2; pitch correction default true; audio delay default 0.
  - `AudioChannels` default is `AutoSafe` (`value = "auto"`).

### T-45b-046 — Decoder prefs and a single video-filter chain
- Priority: P1
- Effort: M
- Depends: T-45b-048, T-45b-068
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/DecoderPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - `vo` gpu or gpu-next, `hwdec` auto or no from `pref_try_hwdec`, debanding (`gradfun=radius=12` or `deband=yes`), `format=yuv420p`, and video filters are all combined into one `vf` chain (E-26 fix).
  - Linux `hwdec` values (VA-API, NVDEC copy modes) mapped per D§2; Android `mediacodec*` values dropped with the note in behavior §9.6.

### T-45b-068 — Decoder display and switch
- Priority: P1
- Effort: S
- Depends: T-45b-046
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt
- Acceptance:
  - `get_decoder` reads the active hwdec; unknown values map to Auto (E-07); `update_decoder` writes `hwdec`.
  - The More sheet shows the decoder title.

## G. Preferences and settings

### T-45b-047 — Gesture preferences catalogue
- Priority: P0
- Effort: S
- Depends: T-45b-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt
- Acceptance:
  - All 13 keys of D§4.4 with defaults (e.g. `pref_default_intro_length` 85, `pref_skip_length_preference` 10, `pref_left_double_tap` Seek).
  - Test T74 asserts every default.

### T-45b-048 — Player preferences catalogue (typed accessors)
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AdvancedPlayerPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/DecoderPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/SubtitlePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt
- Acceptance:
  - `src/settings/player*.rs` expose every key in D§4 with its exact default; enum-valued prefs stored by name (`getEnum` semantics).
  - Test T74 table asserts defaults; T75, T76 assert preset count and enum strings.

### T-45b-034 — Advanced mpv prefs (user scripts, mpv.conf, input.conf)
- Priority: P1
- Effort: M
- Depends: T-45b-048, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AdvancedPlayerPreferences.kt
- Acceptance:
  - `mpv_scripts` (default false) gates loading of user scripts; `pref_mpv_conf` and `pref_mpv_input` (default "") are applied at mpv creation (conf) and as input bindings (input); invalid lines are logged, not fatal.
  - `pref_player_statistics_page` (default 0) is stored for the statistics page.

### T-45b-049 — External player preferences
- Priority: P1
- Effort: S
- Depends: T-45b-048, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - `pref_always_use_external_player` (default false) and `external_player_preference` (default "") are stored; the launch itself lives in part 1 (`ExternalIntents`) and uses `xdg-open` or the configured command.

### T-45b-050 — Picture-in-picture as a mini player
- Priority: P2
- Effort: L
- Depends: T-45b-074, T-45b-048, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - Mini window re-parents the render GLArea and keeps the same `Mpv` handle; trigger button and `pref_pip_on_exit` (default false) supported; `pref_enable_pip` (default true) shows the button.
  - Episode-change pill shown when `pref_pip_episode_toasts` (default true); `pip_replace_with_previous` (default false) switches the next action to previous.
  - Drop note: Android PiP windowing; desktop "always on top" depends on compositor (documented).

### T-45b-053 — Remember brightness and volume
- Priority: P2
- Effort: S
- Depends: T-45b-031, T-45b-032, T-45b-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - `pref_remember_brightness` and `pref_remember_volume` (both default false) store `player_brightness_value` and `player_volume_value` (default -1.0) and restore them on session start when the value is ≥ 0.

### T-45b-054 — Display preferences
- Priority: P2
- Effort: S
- Depends: T-45b-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - `pref_panel_opacity` (60), `pref_reduce_motion` (false), `pref_display_vol_as_per` (true), `pref_player_time_to_disappear` (4000 ms) and `pref_show_system_status_bar` (false) are stored and applied by the UI.
  - Status-bar setting is a documented no-op on Linux (drop reason: no system status bar).

### T-45b-055 — Auto-play next
- Priority: P1
- Effort: S
- Depends: T-45b-048, T-45b-060
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt
- Acceptance:
  - `pref_auto_play_enabled` (default false) toggled through `set_auto_play`, which shows the on/off pill and persists the value.

### T-45b-056 — Incognito gating
- Priority: P1
- Effort: S
- Depends: area:06-data-repos-history-track-source-misc, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - Per-source incognito state (`GetAnimeIncognitoState`) recomputed on every episode switch (E-16) and the global `__APP_STATE_incognito_mode` used for tracker sync (E-17); tests T56, T57.

### T-45b-061 — Message keys and localisation
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Every key in behavior §18 is present in the Rust i18n table with the base text; the hard-coded "Skip Intro: " prefix is replaced by a localised template (E-level note in §18).
  - Non-English base strings are carried verbatim and flagged for translation review.

### T-45b-060 — One-shot player messages (pills)
- Priority: P1
- Effort: S
- Depends: T-45b-002, T-45b-061
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - A single pill slot; a newer message replaces the visible one and restarts its timer (U§1.1 rule).
  - Save, share and set-art results are delivered as events over an unbounded channel (E-23).

### T-45b-067 — Custom key codes for gesture "Custom"
- Priority: P1
- Effort: S
- Depends: T-45b-024, T-45b-073
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `keypress` commands send `0x10001`, `0x10002`, `0x10003` for left, center, right; media keys use `0x10004-0x10006` (D§7.3).

## H. Lua bridge, custom buttons, mpv user data

### T-45b-035 — Lua user-data dispatcher
- Priority: P1
- Effort: L
- Depends: T-45b-002, T-45b-073, T-45b-051, T-45b-023, T-45b-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt, app/src/main/assets/aniyomi.lua
- Acceptance:
  - Implements every row of behavior §12, including the empty-value guard and the post-dispatch clear of `user-data/aniyomi/<name>`.
  - Malformed payloads are ignored with a log entry, never a panic (E-53); tests T69-T73.

### T-45b-036 — Custom buttons (fetch state, primary button, execute)
- Priority: P1
- Effort: M
- Depends: T-45b-002, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/PlayerSettingsCustomButtonScreenModel.kt
- Acceptance:
  - `custom_buttons` Loading/Success/Error states; primary button = first favourite; title defaults to its name unless a Lua title was set (behavior §1).
  - `execute` sends `script-message call_button_<id>`, long press `call_button_<id>_long` (behavior §13).

### T-45b-071 — Lua user-data writes on load
- Priority: P1
- Effort: S
- Depends: T-45b-003, T-45b-041
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Writes `user-data/current-anime/anime-title`, `intro-length`, `category` (comma-space joined), and `episode-number` (Double) on load (D§7.2).

### T-45b-072 — Local audio and subtitle file add
- Priority: P1
- Effort: S
- Depends: T-45b-042, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - File chooser returns a local path; `audio-add`/`sub-add` receive `path, "cached"` with the file name as title (behavior §5.4).
  - Content-URI branches are replaced (drop note: Android SAF).

### T-45b-042 — External audio and subtitle add (command side)
- Priority: P1
- Effort: S
- Depends: T-45b-072, T-45b-073
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `add_audio` and `add_subtitle` issue exactly the commands of D§7.3; the track list reloads on the next `track-list` event.

### T-45b-043 — Subtitle primary/secondary selection
- Priority: P1
- Effort: S
- Depends: T-45b-016, T-45b-073
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `select_sub` implements the four-case rule of behavior §5.6 and writes `sid` and `secondary-sid`; test covers promotion of secondary to primary.

## I. Screenshots and per-anime settings

### T-45b-039 — Screenshot capture, save, share and set-as-art
- Priority: P1
- Effort: M
- Depends: T-45b-073, area:13-core-util-storage-archive-torrent, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - Capture via `screenshot-to-file` with subtitle flag; temp file renamed to `mpv_screenshot.png` with error propagation (E-52).
  - Save to `Pictures/<title>/<name> - <time>` with the 250-byte cap; share clears the cache dir first; set-as-art maps to cover, background, thumbnail (behavior §14).
  - Same-second collision handled (E-51).

### T-45b-041 — Per-anime skip intro length
- Priority: P1
- Effort: S
- Depends: area:07-domain-entries-items, T-45b-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt
- Acceptance:
  - Reads bits 0-7 and disable bit 56 (D§3.2); `0` sets disable; value clamped to 0..255 before write (E-27 fix).
  - Non-favourite anime: value applies to the session only (E-24); test T77-T78.

## J. Playback chrome and state bindings (UI-facing, state side)

### T-45b-073 — mpv observer set and whole-second event channel
- Priority: P0
- Effort: M
- Depends: area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - `mpv.rs` observes exactly the properties and formats of R§8 (`time-pos` as I64, `track-list` and `chapter-list` as None, `user-data/aniyomi` as Str).
  - Events cross to the GTK thread through an unbounded `async_channel`; no blocking on the libmpv thread.
  - Test with fake events: a `time-pos` event fires only on integer change.

### T-45b-074 — mpv render context on GtkGLArea
- Priority: P0
- Effort: L
- Depends: T-45b-073, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - Render context created on `realize`, freed on `unrealize`, `render` called from the GLArea render signal on the main thread (R§14 pitfall 3).
  - Options set before file load in order: context, options, file (R§14 pitfall 4).

### T-45b-075 — Player surface and overlay bindings (replaces mockup state)
- Priority: P0
- Effort: L
- Depends: T-45b-002, T-45b-074, T-45b-051, T-45b-022, T-45b-023
- Sources: kurobayomi/src/player.rs, kurobayomi/src/ui.rs
- Acceptance:
  - The mockup's placeholder surface, constant duration 1420, static chapter/skip labels and local aspect/speed state are replaced by bindings to `PlayerModel` (U§4 gap list).
  - Play/pause icon follows `paused`; prev/next sensitivity follows `has_prev`/`has_next`; lock state follows `controls_locked`.

### T-45b-076 — Skip button and chapter button UI
- Priority: P1
- Effort: S
- Depends: T-45b-022, T-45b-018, T-45b-075
- Sources: kurobayomi/src/player.rs
- Acceptance:
  - Skip button visibility and label follow `skip_intro_text`; tapping cancels the countdown when it is running (U§1.2).
  - Chapter button shows `"<mm:ss> • <name>"` and hides when the chapter list is empty (U§1.3).

### T-45b-077 — Seek bar, scrub and thumbnail UI
- Priority: P1
- Effort: M
- Depends: T-45b-025, T-45b-023, T-45b-075
- Sources: kurobayomi/src/player.rs
- Acceptance:
  - Scrub shows the thumbnail when available; commit on release calls `seek_to`; chapter marks replace the hard-coded 85 and 1330 (U§1.4).
  - Duration label toggles total/remaining following `invertDuration`.

### T-45b-078 — Sheets, panels and quality/hoster UI binding
- Priority: P0
- Effort: L
- Depends: T-45b-013, T-45b-015, T-45b-016, T-45b-075
- Sources: kurobayomi/src/player.rs, kurobayomi/src/ui.rs
- Acceptance:
  - Hoster rows render Idle, Loading, Ready (expandable), Error exactly as U§1.7; Error rows are not clickable.
  - Audio and subtitle lists bound to `sub_tracks`/`audio_tracks` with the selected markers (U§1.8).

### T-45b-079 — Volume, brightness sliders and seek overlay text
- Priority: P1
- Effort: M
- Depends: T-45b-031, T-45b-032, T-45b-060, T-45b-075
- Sources: kurobayomi/src/player.rs
- Acceptance:
  - Volume slider appears on change; boost range shown above 100 up to 100+cap; brightness slider shows -0.75..1 as percent (U§1.12).
  - `seek_text` overlay and double-tap amount overlay follow the rules of U§1.4.

### T-45b-080 — IntegerPicker dialog
- Priority: P1
- Effort: S
- Depends: T-45b-035, T-45b-075
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Dialog built from `IntegerPicker` fields (title, name format, min, max, step, default), writes the mpv property on change, and closes via `dialog = None` (U§1.6).
  - Out-of-range defaults are clamped on open (U§6).

### T-45b-081 — Episode list dialog binding
- Priority: P1
- Effort: S
- Depends: T-45b-005, T-45b-075
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Dialog lists the filtered playlist with the current episode highlighted and the selected episode always present (U§1.17); row tap switches episodes through `loadEpisode`.

### T-45b-065 — Drop Android IME keyboard handling
- Priority: P3
- Effort: S
- Depends: T-45b-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `software_keyboard` Lua commands are accepted and ignored on Linux with a debug log; the drop reason is recorded: the desktop has no soft keyboard and custom entry fields use GTK entries.

## K. Tests and fixtures

### T-45b-062 — Player core test suite and fixtures
- Priority: P0
- Effort: M
- Depends: T-45b-010, T-45b-017, T-45b-019, T-45b-022, T-45b-005, T-45b-023, T-45b-026, T-45b-027
- Sources: kurobayomi/src/player/mod.rs, kurobayomi/spec/reader_player.json
- Acceptance:
  - In-memory fakes exist for `AnimeSourceApi`, `PlayerDeps`, `Mpv`, `SystemVolume`, clock; fixtures listed in E doc §5 are committed under `tests/fixtures/player/`.
  - Every numbered test in E doc §4 is present and passing (79 tests, T1-T79).
  - `cargo test` passes without network access (AniSkip uses recorded fixtures).

## L. Summary of drops (kept here so every behavior has a task)

### T-45b-083 — Document Android-only drops and the user-visible losses
- Priority: P3
- Effort: S
- Depends: T-45b-058, T-45b-065, T-45b-032, T-45b-053, T-45b-054
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt
- Acceptance:
  - The user-facing changelog lists each loss: rotation lock, Android status bar setting (no-op), window brightness (replaced by mpv filter), software keyboard, Android PiP semantics, content URI picker.
  - Each loss names its replacement (or "none") and is linked to the task that implements it.
