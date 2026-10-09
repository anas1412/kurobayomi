# Area 27-tracking-core, part 1: implementation tasks

IDs `T-27a-NNN`. Paths are repo-relative to the Aniyomi clone. Behaviour: `p1-behavior.md`. Data: `p1-data-and-config.md`. Design: `p1-rust-design.md`. UI: `p1-ui-parity.md`. Tests: `p1-edge-cases-and-tests.md` (`TT-27-…`). Topics: `p1-topic-progress-queue.md`, `p1-topic-tracker-oauth.md`.

Priority: no task here is P0. Tracking is parity (P1) because reading and watching work without it. P2 is polish, P3 is an Android-only drop with a stated replacement. Effort: S up to 1 day, M up to 3 days, L up to 1.5 weeks.

Tracker services themselves (AniList, MyAnimeList, Kitsu, Shikimori, Bangumi, MangaUpdates, Simkl, and the enhanced Kavita, Komga, Suwayomi, Jellyfin) are not in this file. Their tasks are in areas 28 to 33. This file depends on them through `area:` tokens.

---

## A. Tracker contracts and registry

### T-27a-001 — Tracker trait and capability surface
- Priority: P1
- Effort: M
- Depends: area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt
- Acceptance:
  - `kuro-track::tracker::Tracker` declares `id`, `name`, `logo`, `logo_color`, `supports_reading_dates` (default false), `supports_private_tracking` (default false), `completion_status`, `score_list`, `login`, `logout`, `is_logged_in`, `login_state`, `save_credentials`, `username`, `password`.
  - `as_anime`, `as_manga`, `as_enhanced_anime`, `as_enhanced_manga`, `as_deletable_anime`, `as_deletable_manga` return `Option` and never panic. A manga-only fake returns `None` from `as_anime`.
  - `Arc<dyn Tracker>` compiles (trait is object safe via `async-trait`).

### T-27a-002 — TrackerManager registry
- Priority: P1
- Effort: S
- Depends: T-27a-001, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt
- Acceptance:
  - The registry holds exactly 11 trackers with ids 1, 2, 3, 4, 5, 6, 7, 8, 9, 101, 102 (`p1-data-and-config.md` section 1).
  - `all()` returns registry order: MyAnimeList, AniList, Kitsu, Shikimori, Bangumi, Komga, MangaUpdates, Kavita, Suwayomi, Simkl, Jellyfin.
  - `get(999)` returns `None`. `logged_in()` filters by `is_logged_in` and keeps registry order.
  - `logged_in_stream()` emits a new list within one update after a login or logout.

### T-27a-003 — BaseTracker credential state and login watch
- Priority: P1
- Effort: M
- Depends: T-27a-020, T-27a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - `is_logged_in` is true only when username and password are both non-empty (TT-27-100 family in section 1.2 of the edge-case file).
  - `logout` clears username, password, the token and the auth-expired flag, and publishes `false`.
  - `save_credentials` writes username, then password, then clears auth-expired, then publishes `true`. A failure between writes leaves the tracker logged out on the next start.
  - `login_state()` emits only when the value changes.

### T-27a-004 — Login state reaches the settings rows
- Priority: P1
- Effort: S
- Depends: T-27a-003, T-27a-002
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - A login completed from the browser redirect updates the matching settings row's check mark while the settings page stays open.
  - The row's check mark matches `is_logged_in` on every open of the page.

### T-27a-005 — AnimeTracker trait with default methods
- Priority: P1
- Effort: M
- Depends: T-27a-001, T-27a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt
- Acceptance:
  - Default `index_to_score(i) = i as f64`, default `score_10(t) = t.score`.
  - Required methods: `status_list`, `watching_status`, `rewatching_status`, `status_label`, `display_score`, `update(track, did_watch_episode)`, `bind(track, has_seen)`, `search`, `refresh`.
  - A fake tracker in `kuro-testkit` implements every required method and records calls (`p1-edge-cases-and-tests.md` section 8).

### T-27a-006 — MangaTracker trait with default methods
- Priority: P1
- Effort: M
- Depends: T-27a-001, T-27a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt
- Acceptance:
  - Same defaults as T-27a-005 with `reading_status`, `rereading_status`, `update(track, did_read_chapter)`, `bind(track, has_read)`, `search`.
  - Compiles against the same fake as T-27a-005 with a manga variant.

### T-27a-007 — Anime status, completion and progress setters
- Priority: P1
- Effort: M
- Depends: T-27a-005, T-27a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt
- Acceptance:
  - `set_anime_status` to completion with total 12 sets last to 12; with total 0 leaves last unchanged (TT-27-100).
  - `set_last_episode_seen(1)` on a planned entry with last 0 sets watching; episode 0 does not (TT-27-101).
  - Setting episode 12 with total 12 sets completed and `finish_date` to the injected clock value; episode 11.9 is truncated to 11 and does not complete (TT-27-102, TT-27-103).
  - Rewatching status is not changed by the watching rule.

### T-27a-008 — Manga status and chapter setters
- Priority: P1
- Effort: S
- Depends: T-27a-006, T-27a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt
- Acceptance:
  - Same rules as T-27a-007 with `reading_status`, `rereading_status`, `last_chapter_read`, `total_chapters`, `finish_date` (TT-27-100 and TT-27-102 manga variants).

### T-27a-009 — Score setter and index mapping
- Priority: P1
- Effort: S
- Depends: T-27a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt
- Acceptance:
  - A score string at index 0 stores `0.0`; index 2 stores `2.0` (TT-27-104).
  - A string not in the list stores `-1.0` (source behaviour) and emits `UnknownScore` in the Rust API, which the caller shows as "Score not in list" (a documented change; `p1-behavior.md` section 3.4).
  - Manga has the same behaviour (test duplicated).

### T-27a-010 — Date and private setters, and update_remote persistence
- Priority: P1
- Effort: M
- Depends: T-27a-005, T-27a-006, T-27a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt
- Acceptance:
  - `set_start_date(0)` clears the start date; `set_finish_date(t)` stores `t` unchanged (TT-27-105).
  - `set_private(b)` stores the flag and calls the tracker's `update`.
  - Order is remote `update` first, then local upsert (TT-27-106). A remote failure writes nothing locally and returns the error.
  - The error returned to the caller carries the raw message (`MsgKey::Raw`).

### T-27a-011 — Register entry point
- Priority: P1
- Effort: S
- Depends: T-27a-031, T-27a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt
- Acceptance:
  - `register` sets the entry id on the search result before binding.
  - A bind failure is shown as a toast with the raw message and is not logged at ERROR (matches source: no log, `p1-behavior.md` section 6.4).

### T-27a-012 — Enhanced tracker traits
- Priority: P1
- Effort: M
- Depends: T-27a-005, T-27a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedMangaTracker.kt
- Acceptance:
  - `accepts(source_id)` is true only for listed source ids (string compare, replaces the Kotlin class-name check).
  - `match_entry(title, entry_id)` returns `Ok(None)` for no match and never a partially filled result.
  - `is_track_from` and `migrate_track` exist with the source semantics (migration is used by area 17 and 34).

### T-27a-013 — Deletable tracker traits
- Priority: P1
- Effort: S
- Depends: T-27a-005, T-27a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/DeletableAnimeTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/DeletableMangaTracker.kt
- Acceptance:
  - Only trackers that implement the capability expose `as_deletable_*` (MangaUpdates in the current registry).
  - A remote delete failure is returned to the caller so the remove flow can keep the local row (TT-27-D16).

### T-27a-014 — Domain model and DB mapping
- Priority: P1
- Effort: M
- Depends: area:06-data-repos-history-track-source-misc
- Sources: app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt, app/src/main/java/eu/kanade/domain/track/manga/model/MangaTrack.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt
- Acceptance:
  - Round trip `domain -> db -> domain` is identity for a track with `id = Some(_)` (TT-27-304).
  - A `None` id maps with `id_required = false` to a track with `id: None`, and with `id_required = true` to an error.
  - Field names and types match `p1-data-and-config.md` sections 4.1 and 4.2 exactly.

### T-27a-015 — Track row identity and upsert
- Priority: P1
- Effort: M
- Depends: T-27a-014, area:06-data-repos-history-track-source-misc
- Sources: data/src/main/sqldelightanime/dataanime/anime_sync.sq, data/src/main/sqldelight/data/manga_sync.sq
- Acceptance:
  - Upsert on `(anime_id, sync_id)` keeps `_id` unchanged (TT-27-D01).
  - `RETURNING _id` yields the stable id; the same for manga.
  - The unique constraint and foreign key with cascade are unchanged from the source.

### T-27a-016 — copy_personal_from semantics
- Priority: P1
- Effort: S
- Depends: T-27a-014
- Sources: app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt, app/src/main/java/eu/kanade/domain/track/manga/model/MangaTrack.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt
- Acceptance:
  - Domain form copies progress, score, status, start and finish dates and `private` (TT-27-303 covers the legacy flag).
  - Legacy form with `copy_remote_private = false` leaves `private` unchanged.

### T-27a-017 — Search result models
- Priority: P1
- Effort: S
- Depends: T-27a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/model/AnimeTrackSearch.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt
- Acceptance:
  - `score` is `Option<f64>`; the source sentinel `-1.0` is not stored in Rust.
  - `title` and `tracking_url` are required fields, not lateinit (TT-27-D09).
  - Equality uses `(anime_id, tracker_id, remote_id)`; list keys use `(tracker_id, remote_id)` (TT-27-D10).

---

## B. Policy, preferences and login

### T-27a-018 — AutoTrackState and mark-read decision
- Priority: P1
- Effort: S
- Depends: T-27a-019
- Sources: app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt
- Acceptance:
  - Serialised names are `ALWAYS`, `ASK`, `NEVER`; unknown stored text yields `Always`.
  - `decide(read, tracker_behind, state)` returns `Skip`, `Prompt`, or `Apply` exactly as TT-27-113 and `p1-behavior.md` section 11.2 describe.

### T-27a-019 — Tracking preference keys and defaults
- Priority: P1
- Effort: M
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Keys and defaults match `p1-data-and-config.md` section 2: `pref_auto_update_manga_sync_key` true, `track_on_adding_to_library` true, `show_next_episode_airing_time` true, `pref_auto_update_manga_on_mark_read` ALWAYS, `anilist_score_type` "POINT_10".
  - Per-tracker keys are generated only through the private-key helper, so the stored key is prefixed `__PRIVATE_`.
  - Keys are read through the typed preference API; a unit test asserts each default.

### T-27a-020 — Credential and token storage in the secret store
- Priority: P1
- Effort: M
- Depends: area:01-foundation-app-bootstrap, T-27a-019
- Sources: app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt
- Acceptance:
  - Username, password and token JSON are stored in the Secret Service through `SecretStore` under `kurobayomi.track` accounts `<id>.username`, `<id>.password`, `<id>.token`.
  - When no Secret Service is available, a `0600` file under the data directory is used and a single warning is logged.
  - No secret appears in the preference file or in logs (test: grep of the test data directory finds no token).

### T-27a-021 — OAuth request builder and PKCE
- Priority: P1
- Effort: M
- Depends: T-27a-020, area:28-tracker-anilist, area:29-tracker-myanimelist, area:31-tracker-shikimori-bangumi, area:32-tracker-mangaupdates-simkl
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeListApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt
- Acceptance:
  - Each authorise URL matches the golden string in `p1-data-and-config.md` section 10 with the same query parameter order.
  - `state` is 32 random bytes, base64url without padding, generated per request.
  - MyAnimeList PKCE verifier has 43 to 128 unreserved characters and the challenge equals the verifier (plain method, as in the source).

### T-27a-022 — Redirect router and pending login
- Priority: P1
- Effort: L
- Depends: T-27a-021, area:53-platform-manifest-intents-workers, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt
- Acceptance:
  - A `gio::Application` with `HANDLES_OPEN` receives `aniyomi://<tracker>-auth` URIs and calls `handle_redirect`.
  - Parsing follows the table in `p1-topic-tracker-oauth.md` section 5 for every row.
  - An unknown host shows "Unexpected login response" and does not hang (TT-27-D17).
  - A pending login older than 5 minutes is rejected with "Login expired, try again".
  - A mismatched `state` is rejected and the old credentials are kept (race R8 in `p1-edge-cases-and-tests.md` section 2).

### T-27a-023 — AniList implicit token parsing
- Priority: P1
- Effort: S
- Depends: T-27a-022
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt
- Acceptance:
  - A fragment ending in `access_token=abc` yields token `abc` (TT-27-D18).
  - A fragment with `error=access_denied` keeps the previous login and shows "Login cancelled".

### T-27a-024 — Enhanced services discovery and info text
- Priority: P1
- Effort: S
- Depends: T-27a-012, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - A tracker is listed as available when any installed source's qualified id is in its accepted list; otherwise it is named in the not-installed text.
  - The text joins names with ", " and appends to the base sentence after two newlines.
  - The "Enhanced trackers" group is always rendered, even with no enhanced tracker.

---

## C. Progress, bind, refresh and sync

### T-27a-025 — Retry queue store
- Priority: P1
- Effort: M
- Depends: area:06-data-repos-history-track-source-misc, T-27a-015
- Sources: app/src/main/java/eu/kanade/domain/track/anime/store/DelayedAnimeTrackingStore.kt, app/src/main/java/eu/kanade/domain/track/manga/store/DelayedMangaTrackingStore.kt
- Acceptance:
  - Table `track_retry_queue` with primary key `(kind, entry_id, tracker_id)` and `progress REAL` exactly as `p1-data-and-config.md` section 7.2.
  - `add` is one `INSERT … ON CONFLICT DO UPDATE SET progress = MAX(progress, excluded.progress)` statement (TT-27-D06 with 100 concurrent adds).
  - Anime and manga entries with the same numbers stay separate (TT-27-D03).
  - Progress round trips `12.1` exactly (TT-27-D07).

### T-27a-026 — Queue skip and removal rules
- Priority: P1
- Effort: S
- Depends: T-27a-025, T-27a-027
- Sources: app/src/main/java/eu/kanade/domain/track/anime/service/DelayedAnimeTrackingUpdateJob.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt
- Acceptance:
  - An item whose binding no longer exists is removed on the next run (source behaviour).
  - An item whose remote progress is at or past the queued value is removed (TT-27-D05, a change from the source).
  - Removal is conditional on the value processed, so a newer `add` during the run is kept (race R2 in `p1-edge-cases-and-tests.md` section 2).

### T-27a-027 — Track episode progress (anime)
- Priority: P1
- Effort: M
- Depends: T-27a-007, T-27a-025, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt
- Acceptance:
  - Trackers that are logged out or already at or past the episode are skipped without a request (TT-27-111 negative case).
  - Online path: `refresh`, copy with the episode number, `update(track, true)`, upsert, remove the queue item.
  - Offline path (no connectivity): queue the item and schedule the retry job.
  - An online error is queued too (change from the source, `p1-behavior.md` section 9.1); the test documents the change.

### T-27a-028 — Track chapter progress (manga)
- Priority: P1
- Effort: M
- Depends: T-27a-008, T-27a-025
- Sources: app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt
- Acceptance:
  - Same flow as T-27a-027 for chapters; any failure queues the item (TT-27-112).
  - The nested duplicate condition of the source is written once.
  - Errors are returned as an outcome per tracker, not rethrown.

### T-27a-029 — Retry scheduler for anime and manga
- Priority: P1
- Effort: L
- Depends: T-27a-025, T-27a-026, T-27a-068
- Sources: app/src/main/java/eu/kanade/domain/track/anime/service/DelayedAnimeTrackingUpdateJob.kt, app/src/main/java/eu/kanade/domain/track/manga/service/DelayedMangaTrackingUpdateJob.kt
- Acceptance:
  - `MAX_ATTEMPTS = 4`, `INITIAL_BACKOFF = 5 min`, `MAX_BACKOFF = 5 h`, backoff doubles per attempt (TT-27-121, TT-27-122).
  - `kick()` resets attempts and wakes the loop (TT-27-123, TT-27-126).
  - Runs only when `is_online()` is true (TT-27-124); a queue survives restart (TT-27-125).
  - Exactly one scheduler loop exists (TT-27-127).

### T-27a-030 — Manga retry job parity
- Priority: P1
- Effort: S
- Depends: T-27a-029
- Sources: app/src/main/java/eu/kanade/domain/track/manga/service/DelayedMangaTrackingUpdateJob.kt
- Acceptance:
  - The manga scheduler uses the same constants as anime (`p1-data-and-config.md` section 2 and behaviour section 20).
  - It drains chapter progress only and never anime items (TT-27-D03).

### T-27a-031 — Bind anime
- Priority: P1
- Effort: M
- Depends: T-27a-005, T-27a-010, T-27a-037, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt
- Acceptance:
  - Steps follow `p1-behavior.md` section 6.1 in order (TT-27-110 for the start date).
  - The remote episode number is truncated with `as i32` (documented).
  - A history row with null `seenAt` is ignored when choosing the first date (TT-27-D20).
  - Errors from the tracker propagate to `register`.
  - After the setters run, the local write uses the latest values, not a stale copy (TT-27-D19).

### T-27a-032 — Bind manga
- Priority: P1
- Effort: M
- Depends: T-27a-006, T-27a-010, T-27a-038, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt
- Acceptance:
  - Same as T-27a-031 with `has_read`, chapter numbers and read history dates.
  - Test: an entry with read chapters 1 to 3 binds with the remote read progress set to 3.

### T-27a-033 — Enhanced bind for anime
- Priority: P1
- Effort: M
- Depends: T-27a-012, T-27a-031
- Sources: app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt
- Acceptance:
  - Only logged-in enhanced trackers that accept the source are matched.
  - A matched result is bound, upserted and synced with the enhanced rule.
  - A failure for one tracker is logged at WARN with the entry title and tracker; the next tracker still runs.
  - A tracker already bound to the entry is not bound twice (a change from the source, `p1-behavior.md` section 6.2).

### T-27a-034 — Enhanced bind for manga
- Priority: P1
- Effort: S
- Depends: T-27a-012, T-27a-032
- Sources: app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt
- Acceptance:
  - Same as T-27a-033 with the manga kind.

### T-27a-035 — Refresh anime
- Priority: P1
- Effort: S
- Depends: T-27a-005, T-27a-037
- Sources: app/src/main/java/eu/kanade/domain/track/anime/interactor/RefreshAnimeTracks.kt
- Acceptance:
  - Only logged-in trackers are refreshed; logged-out bindings are skipped without a failure entry.
  - Each refresh runs concurrently; one failure does not cancel the others.
  - The result is the list of `(tracker, error)` failures.

### T-27a-036 — Refresh manga
- Priority: P1
- Effort: S
- Depends: T-27a-006, T-27a-038
- Sources: app/src/main/java/eu/kanade/domain/track/manga/interactor/RefreshMangaTracks.kt
- Acceptance:
  - Same as T-27a-035 for manga.

### T-27a-037 — Sync episode progress with enhanced trackers
- Priority: P1
- Effort: M
- Depends: T-27a-005, T-27a-015
- Sources: app/src/main/java/eu/kanade/domain/track/anime/interactor/SyncEpisodeProgressWithTrack.kt
- Acceptance:
  - Non-enhanced trackers: no call, no write (TT-27-107).
  - Continuous-watching rule and max rule: TT-27-108 and TT-27-109.
  - Order: remote update, then local episode updates, then binding upsert.

### T-27a-038 — Sync chapter progress
- Priority: P1
- Effort: S
- Depends: T-27a-006, T-27a-015
- Sources: app/src/main/java/eu/kanade/domain/track/manga/interactor/SyncChapterProgressWithTrack.kt
- Acceptance:
  - Same as T-27a-037 for chapters (manga tests duplicated).

### T-27a-039 — Mark read or watched prompt flow
- Priority: P1
- Effort: M
- Depends: T-27a-018, T-27a-035, T-27a-027, T-27a-028, T-27a-060
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Nothing happens when marking unread, when no logged-in tracker exists, or when the policy is `NEVER` (TT-27-113).
  - `ALWAYS` applies the update and shows "Trackers updated to chapter N" or "… episode N" for three seconds.
  - `ASK` shows a banner "Update trackers to chapter N?" (or episode) with an OK action and a dismiss action for four seconds, and sends the update only on OK, with no success toast.
  - No prompt when the selection max is not greater than every tracker's progress.

### T-27a-040 — Auto-update gating for reader and player
- Priority: P1
- Effort: S
- Depends: T-27a-019, T-27a-027, T-27a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt
- Acceptance:
  - Reader and player skip tracking in incognito mode and when `pref_auto_update_manga_sync_key` is false (`p1-behavior.md` section 9.3).
  - The external player queues by `(entry_id, tracker_id)` and never by the entry id (TT-27-D02).
  - The reader passes the chapter number through an `f64` path (TT-27-D07).

### T-27a-041 — Auto-open track sheet after add to library
- Priority: P1
- Effort: S
- Depends: T-27a-070, T-27a-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - The sheet opens only when the preference is on and the entry has at least one supported binding (TT-27-115).
  - The settings label stays "Open track menu on adding to library" and the help text names the condition (documented in `p1-behavior.md` section 11.4).

### T-27a-042 — Connectivity check through GIO
- Priority: P1
- Effort: S
- Depends: area:12-network-stack, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkExtensions.kt
- Acceptance:
  - `is_online()` returns the value of `gio::NetworkMonitor::default().is_network_available()`.
  - `wait_online()` resolves within one second of the network becoming available (tested with a fake monitor).
  - The source's transport-type check is not ported; the difference is documented.

---

## D. Settings and sign-in UI

### T-27a-043 — Settings switches and mark-read list
- Priority: P1
- Effort: S
- Depends: T-27a-019, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Four rows in order with the labels of `p1-behavior.md` section 12.2, with the trailing space of "airing time " trimmed for display.
  - Defaults are on, on, on, and Always. Changes persist immediately.
  - The list has three options in order: Always, Always ask, Never (TT-27-214).

### T-27a-044 — Settings tracker login rows
- Priority: P1
- Effort: M
- Depends: T-27a-002, T-27a-004, T-27a-022, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Rows in the order MyAnimeList, AniList, Kitsu, MangaUpdates, Shikimori, Simkl, Bangumi, then the info text.
  - Logged-in row: check mark visible; click opens the logout dialog (TT-27-215).
  - OAuth rows open the authorise URL in the default browser; credential rows open the login dialog.

### T-27a-045 — Credential login dialog
- Priority: P1
- Effort: M
- Depends: T-27a-003, T-27a-020
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Labels "Email address" (Kitsu) or "Username" (MangaUpdates), "Password"; title "Log in to <tracker>".
  - OK is disabled until both fields are non-blank (TT-27-216); shows "Loading…" while the request runs.
  - Failure marks both fields, shows the message, keeps the typed values, and logs out the tracker (source rule).
  - A failure with no message shows "Login failed", not "null" (TT-27-D14).

### T-27a-046 — Logout dialog
- Priority: P1
- Effort: S
- Depends: T-27a-003
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Title "Log out from <tracker>?"; Cancel is the default focus (TT-27-217).
  - Log out clears credentials, shows "You are now logged out", and removes the check mark.

### T-27a-047 — Drop the external tracking guide link
- Priority: P3
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - The settings page has no help button that opens `aniyomi.org`.
  - The decision and the lost link are recorded in the release notes (no replacement link exists in Kurobayomi yet).

### T-27a-048 — Per-entry tracking sheet
- Priority: P1
- Effort: L
- Depends: T-27a-002, T-27a-049, T-27a-050, T-27a-051, T-27a-052, T-27a-053, T-27a-054, T-27a-064, T-27a-073, T-27a-079
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHome.kt, app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackInfoDialogHome.kt
- Acceptance:
  - One card per logged-in tracker of the entry's kind, in registry order; an "Add tracking" card for trackers without a binding (TT-27-200).
  - Progress text "12" or "12 / 24" (TT-27-202); placeholders at 50 percent alpha (TT-27-201).
  - Score cell only when the tracker has a score list; start and finish dates only when the tracker supports reading dates (TT-27-203, TT-27-204).
  - Private badge shown only when private (TT-27-205).
  - Header title opens search; logo opens the browser; both have the copy actions.

### T-27a-049 — Overflow menu and copy actions
- Priority: P1
- Effort: S
- Depends: T-27a-059
- Sources: app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackInfoDialogHome.kt, app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHome.kt
- Acceptance:
  - Menu order Open in browser, Copy link, Track publicly or privately (only when supported), Remove (TT-27-206, TT-27-207).
  - Copy shows the toast "Copied to clipboard:\n<truncated>" (TT-27-218).

### T-27a-050 — Logo tile
- Priority: P1
- Effort: S
- Depends: T-27a-072, area:49-theme-and-design-system
- Sources: app/src/main/java/eu/kanade/presentation/track/components/TrackLogoIcon.kt
- Acceptance:
  - 48 by 48 px tile with the tracker colour background and 4 px padding.
  - Click and long-press are separate callbacks; without a click handler the tile is not interactive.
  - Missing logo asset shows the tracker's first letter.

### T-27a-051 — Status selector
- Priority: P1
- Effort: S
- Depends: T-27a-005, T-27a-006
- Sources: app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt
- Acceptance:
  - Lists the tracker's status list in order with the tracker's labels; the current status is selected at open.
  - OK sets the status through the remote setter (T-27a-007 or T-27a-008); Cancel changes nothing.
  - Up, Down, Space and Enter work on the list.

### T-27a-052 — Number selector (chapters and episodes)
- Priority: P1
- Effort: S
- Depends: T-27a-007, T-27a-008
- Sources: app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt
- Acceptance:
  - Range `0..=total` when total is known, otherwise `0..=10000` (TT-27-211).
  - Starts at the current progress; OK calls the progress setter.
  - Title "Chapters" for manga, "Episodes" for anime.

### T-27a-053 — Score selector
- Priority: P1
- Effort: S
- Depends: T-27a-009
- Sources: app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt
- Acceptance:
  - Starts at the index of the current score, including index 0 (TT-27-212, defect D8 fixed).
  - Score cell is hidden when the score list is empty.
  - OK calls the score setter with the chosen string.

### T-27a-054 — Date selector
- Priority: P1
- Effort: M
- Depends: T-27a-010, T-27a-064
- Sources: app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt
- Acceptance:
  - Future days are not selectable; start is not after finish; end is not before start (TT-27-213).
  - The stored value uses the same UTC conversion pair as the source (`p1-behavior.md` section 14.5), and a round trip returns the picked day.
  - Remove button appears only when a date is set and clears it with value 0.

### T-27a-055 — Date removal confirmation
- Priority: P1
- Effort: S
- Depends: T-27a-054
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - Title "Remove date?"; text names the tracker and the start or finish date.
  - Remove sets the date to 0 and returns to the sheet; Cancel returns to the picker.

### T-27a-056 — Track search screen
- Priority: P1
- Effort: L
- Depends: T-27a-005, T-27a-006, T-27a-017
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt, app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackerSearch.kt
- Acceptance:
  - Search field prefilled with the title; the search runs once on open when the title is not blank.
  - Loading, results, no results, and error states match `p1-ui-parity.md` section 6.2.
  - A blank query is not sent (TT-27-D11).
  - Enter and the search key both run the query.

### T-27a-057 — Search result card
- Priority: P1
- Effort: M
- Depends: T-27a-056, T-27a-017, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt, app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackerSearch.kt
- Acceptance:
  - Cover 96 px tall; title two lines; authors and artists joined with ", "; detail lines hidden when empty.
  - Type and status capitalised after lower-casing; score line only when score is set.
  - Long-press menu: Copy to clipboard, Open in browser; the menu closes after both (TT-27-D15).

### T-27a-058 — Search bottom bar and private tracking
- Priority: P1
- Effort: S
- Depends: T-27a-056, T-27a-063
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt, app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackerSearch.kt
- Acceptance:
  - Bar appears only when a card is selected (TT-27-209).
  - "Track" registers publicly; the private button appears only for trackers that support private tracking (TT-27-210) and registers with `private = true`.

### T-27a-059 — Remove tracking flow
- Priority: P1
- Effort: M
- Depends: T-27a-013, T-27a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/DeletableAnimeTracker.kt
- Acceptance:
  - Title "Remove <tracker> tracking?"; checkbox "Also remove from <tracker>" only for deletable trackers.
  - Remote delete runs first when checked; if it fails the local binding is kept and "Remote removal failed: <message>" is shown (TT-27-D16, a change from the source).
  - Without the checkbox, only the local binding is deleted.

### T-27a-060 — Error and status messages
- Priority: P1
- Effort: S
- Depends: T-27a-078
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - Every `MsgKey` variant maps to a key listed in `p1-behavior.md` section 18.
  - `track_error` formats "%1$s error: %2$s" with the tracker name and message.
  - The copy toast truncates the content to 50 characters with "..." in the middle.

### T-27a-061 — Compose preview fixtures
- Priority: P3
- Effort: S
- Depends: area:49-theme-and-design-system
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHomePreviewProvider.kt, app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearchPreviewProvider.kt, app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackInfoDialogHomePreviewProvider.kt, app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackerSearchPreviewProvider.kt, app/src/main/java/eu/kanade/presentation/track/components/TrackLogoIconPreviewProvider.kt
- Acceptance:
  - Compose preview annotations are not ported.
  - The sample tracks and results (`p1-edge-cases-and-tests.md` section 8) exist in `kuro-testkit` for logic tests and for the GTK snapshot tests.

---

## E. Support, mockup and polish

### T-27a-062 — AniList score type preference
- Priority: P1
- Effort: S
- Depends: T-27a-019, area:28-tracker-anilist
- Sources: app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - Key `anilist_score_type` with default `POINT_10` is readable by the AniList tracker.
  - Unknown stored values fall back to `POINT_10` (test with a bad value).

### T-27a-063 — Private tracking semantics and badge
- Priority: P1
- Effort: S
- Depends: T-27a-014, T-27a-049
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHome.kt
- Acceptance:
  - Badge with eye-off icon (14 px) at offset x minus 5 px when `private` is true.
  - Menu label "Track publicly" when private, "Track privately" when public; hidden for trackers without private support.

### T-27a-064 — Reading-date support gating
- Priority: P1
- Effort: S
- Depends: T-27a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt
- Acceptance:
  - `supports_reading_dates` defaults to false (source `BaseTracker.kt:23`).
  - Date cells are absent for trackers that return false; start and finish cells use the placeholders otherwise.

### T-27a-065 — Test suite for tracking rules
- Priority: P1
- Effort: M
- Depends: T-27a-007, T-27a-025, T-27a-031, T-27a-068
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt, app/src/main/java/eu/kanade/domain/track/anime/store/DelayedAnimeTrackingStore.kt
- Acceptance:
  - Tests TT-27-100 to TT-27-127 and TT-27-300 to TT-27-305 exist and pass.
  - Defect tests TT-27-D01 to TT-27-D20 exist; each fails against the behaviour quoted in `p1-behavior.md` section 22 and passes on Kurobayomi.
  - `cargo test -p kuro-track` finishes without network access.

### T-27a-066 — Backup interplay
- Priority: P1
- Effort: S
- Depends: T-27a-019, area:21-backup-format-and-create, area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeBackupCreator.kt
- Acceptance:
  - Tracker bindings are included in backup and restore (TT-27-400, TT-27-401).
  - Passwords and tokens are not included (TT-27-402); the restore flow leaves the tracker logged out.
  - Preference import maps `ASK`, `ALWAYS`, `NEVER` and unknown values as TT-27-403 and TT-27-404 describe.
  - The retry queue is not part of backup (TT-27-405).

### T-27a-067 — Logging levels and redaction
- Priority: P1
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt
- Acceptance:
  - Levels: `error` for remote update failure, `warn` for sync and enhanced-match failures, `info` for progress failures, `debug` for queue operations (`p1-rust-design.md` section 9).
  - Tokens, passwords and redirect query strings never appear in log output (test: captured log has no token).

### T-27a-068 — Threading and cancellation model
- Priority: P1
- Effort: M
- Depends: area:01-foundation-app-bootstrap, T-27a-002
- Sources: app/src/main/java/eu/kanade/domain/track/anime/interactor/RefreshAnimeTracks.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt, app/src/main/java/eu/kanade/domain/track/anime/service/DelayedAnimeTrackingUpdateJob.kt
- Acceptance:
  - Write paths run as detached tokio tasks; closing the page does not abort them (test: drop the handle, verify the write completes).
  - GTK widgets are updated only on the main thread through a channel.
  - Refresh runs to completion once started.

### T-27a-069 — Mockup wiring
- Priority: P1
- Effort: M
- Depends: T-27a-048, T-27a-056
- Sources: src/entry.rs, spec/entries.json
- Acceptance:
  - `tracking()` and `tracker_search()` no longer contain hard-coded tracker names, values or results.
  - The status, chapters, episodes, score, start_date, finish_date, remove_date and remove_tracking dialogs take their lists from the tracker and the database.
  - Navigation through `open()` and `dialog_of()` keeps working.

### T-27a-070 — Track count and logged-in derivation for entry screens
- Priority: P1
- Effort: S
- Depends: T-27a-002, T-27a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - `has_logged_in` is true when at least one logged-in tracker supports the entry's source.
  - `tracking_count` counts only bindings of supported logged-in trackers (TT-27-114).

### T-27a-071 — Pending queue indicator and parked toast
- Priority: P2
- Effort: S
- Depends: T-27a-025, T-27a-029
- Sources: app/src/main/java/eu/kanade/domain/track/anime/service/DelayedAnimeTrackingUpdateJob.kt
- Acceptance:
  - The per-entry sheet shows a small pending count when queue items exist for the entry.
  - When the scheduler parks items, the next entry open shows one toast with the count (`p1-topic-progress-queue.md` section 12).

### T-27a-072 — Tracker brand assets and colours
- Priority: P2
- Effort: S
- Depends: area:28-tracker-anilist, area:29-tracker-myanimelist, area:30-tracker-kitsu, area:31-tracker-shikimori-bangumi, area:32-tracker-mangaupdates-simkl, area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt
- Acceptance:
  - Each of the 11 trackers has a logo asset and a brand colour (`getLogo`, `getLogoColor`).
  - A missing asset uses the letter fallback in T-27a-050.

### T-27a-073 — Date format reuse
- Priority: P2
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHome.kt, app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackInfoDialogHome.kt
- Acceptance:
  - Dates in the sheet use the user's date format preference.
  - Dates convert from epoch millis with the system time zone (`toLocalDate`).

### T-27a-074 — Auth-expired state and warning
- Priority: P1
- Effort: S
- Depends: T-27a-020, area:29-tracker-myanimelist
- Sources: app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeList.kt
- Acceptance:
  - HTTP 401 from a tracker sets the flag; the settings row shows a warning mark.
  - A successful login clears the flag (TT-27-215 neighbour test).

### T-27a-075 — Enhanced registration from the sheet
- Priority: P1
- Effort: S
- Depends: T-27a-033, T-27a-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - "Add tracking" on an enhanced tracker runs match and register; no match shows "No match found" (`MR.strings.error_no_match`).
  - Other trackers open the search screen.

### T-27a-076 — Search selection and current-URL rule
- Priority: P1
- Effort: S
- Depends: T-27a-056
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt
- Acceptance:
  - A result is auto-selected only when `current_url` is set and matches its `tracking_url` (search rows in section 1.11 of `p1-edge-cases-and-tests.md`).
  - Starting a new query clears the selection and shows loading.

### T-27a-077 — Enhanced services refresh on source install
- Priority: P2
- Effort: S
- Depends: T-27a-024, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Installing a source that an enhanced tracker accepts makes its row appear without restarting the app.

### T-27a-078 — Message key table
- Priority: P1
- Effort: S
- Depends: area:51-i18n
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - The `MsgKey` enum in `kuro-track` lists the keys in `p1-behavior.md` section 18.
  - A build check fails when a referenced key is missing from the string resources.

### T-27a-079 — Track repository adapters
- Priority: P1
- Effort: M
- Depends: area:05-data-repos-entries-items, area:06-data-repos-history-track-source-misc, T-27a-015
- Sources: data/src/main/java/tachiyomi/data/track/anime/AnimeTrackRepositoryImpl.kt, data/src/main/java/tachiyomi/data/track/manga/MangaTrackRepositoryImpl.kt
- Acceptance:
  - `get_by_id`, `list_for_entry`, `list_all`, `upsert`, `upsert_all`, `delete`, and `watch_for_entry` exist for both kinds.
  - Round-trip mapping tests cover every column (`p1-data-and-config.md` section 6).

### T-27a-080 — Per-binding mutex and conditional queue delete
- Priority: P1
- Effort: S
- Depends: T-27a-068, T-27a-025
- Sources: app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt
- Acceptance:
  - Two concurrent progress updates for one binding run one after the other (race R1 in `p1-edge-cases-and-tests.md` section 2).
  - A queue item added during a run is not removed by that run (race R2 in `p1-edge-cases-and-tests.md` section 2).

### T-27a-081 — Bounded parallelism for refresh
- Priority: P2
- Effort: S
- Depends: T-27a-035, T-27a-036
- Sources: app/src/main/java/eu/kanade/domain/track/anime/interactor/RefreshAnimeTracks.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/RefreshMangaTracks.kt
- Acceptance:
  - At most four refresh requests run per tracker at the same time.
  - A failure in one request is reported and does not stop the others.

### T-27a-082 — Per-entry sheet subscription
- Priority: P1
- Effort: S
- Depends: T-27a-079, T-27a-048
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHome.kt
- Acceptance:
  - The sheet updates within one frame of a binding change in the database.
  - Repeated identical values do not re-render the sheet (distinct until changed).
