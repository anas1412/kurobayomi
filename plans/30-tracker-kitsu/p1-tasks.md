# Area 30-tracker-kitsu: implementation tasks

Scope: Kitsu tracker. Every scope file (11) and every observed behaviour in `p1-behavior.md` maps to at least one task below. Priority: all tracking tasks are P1 (tracking parity). P3 tasks state the replacement for Android-only code. Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/30-tracker-kitsu/p1-tasks.md`.

Common source prefix used below: `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/` (written in full on each line).

### T-30a-001 — Kitsu tracker identity and capability flags
- Priority: P1
- Effort: S
- Depends: area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt
- Acceptance:
  - `Kitsu` reports id 3, name "Kitsu", kinds manga and anime, deletable for both kinds, completion status 2, rereading and rewatching status absent.
  - `supports_reading_dates` and `supports_private_tracking` are both true (the track sheet shows date rows and the private button for Kitsu only).
  - `is_logged_in()` is true only when a username and a user id are both non-empty.

### T-30a-002 — Status constants, status lists and labels
- Priority: P1
- Effort: S
- Depends: T-30a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt
- Acceptance:
  - Constants are READING 1, WATCHING 11, COMPLETED 2, ON_HOLD 3, DROPPED 4, PLAN_TO_READ 5, PLAN_TO_WATCH 15.
  - `status_list(Manga)` is `[1,2,3,4,5]` and `status_list(Anime)` is `[11,15,2,3,4]`, in that order.
  - `status_label` returns "Currently reading", "Want to read", "Currently watching", "Want to watch", "Completed", "On hold", "Dropped" for the matching kind, and None for any other value (test KT-10).

### T-30a-003 — Score list, index mapping and display
- Priority: P1
- Effort: S
- Depends: T-30a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuUtils.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuListSearch.kt
- Acceptance:
  - `score_list()` has exactly 20 entries `"0","1","1.5","2","2.5","3","3.5","4","4.5","5","5.5","6","6.5","7","7.5","8","8.5","9","9.5","10"` (tests KT-04).
  - `index_to_score(0)` is 0.0; index n > 0 gives (n+1)/2.0 (KT-05); an unknown label maps to 0.0.
  - Wire score: 0.0 → None, 0.5 → "1", 1.0 → "2", 10.0 → "20" (KT-06). Read: rating None → 0.0, rating r → r/2.0 (KT-07).
  - `display_score` yields "0", "7", "7.5", "10" (KT-08).

### T-30a-004 — OAuth token model and expiry rule
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuOAuth.kt
- Acceptance:
  - Struct has access_token, token_type, created_at (i64 seconds), expires_in (i64), refresh_token (Option<String>), decoded from the JSON keys in the source.
  - `is_expired(now)` is `now > created_at + expires_in - 3600` (boundary test KT-20).
  - Unknown JSON keys (e.g. `scope`) are ignored.

### T-30a-005 — Token persistence and restore
- Priority: P1
- Effort: M
- Depends: T-30a-004, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuInterceptor.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - Token is stored as JSON in a secret store (Secret Service via `keyring`), falling back to the settings key `track_token_3` when Secret Service is unavailable.
  - Restore treats "", "null" and malformed JSON as no token (KT-21).
  - Clearing the token deletes the stored value; the string "null" is never written (Proposal, flagged against Aniyomi E-09).

### T-30a-006 — Authenticated request pipeline and refresh-on-expiry
- Priority: P1
- Effort: M
- Depends: T-30a-004, T-30a-005, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt
- Acceptance:
  - Every authed request carries `Authorization: Bearer <access_token>`, `Accept` and `Content-Type` `application/vnd.api+json`, and the Kurobayomi User-Agent (KT-90); login and Algolia requests carry no bearer (KT-91, KT-92).
  - No token → `NotAuthenticated` with no request sent (KT-33). Expired token → refresh POST with form order `grant_type, refresh_token, client_id, client_secret` before the request (KT-26).
  - Refresh response without `refresh_token` keeps the old one (KT-27). Refresh 4xx → `ReauthRequired` without sending the request (KT-28, flagged). Network error on refresh → stale token used, server 401 surfaces as `HTTP error 401` (KT-29).
  - A token with no refresh token is fatal only when expired (KT-30, flagged against E-02).

### T-30a-007 — Password-grant login, user-id resolution, logout and credentials
- Priority: P1
- Effort: M
- Depends: T-30a-005, T-30a-006, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuUser.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt
- Acceptance:
  - Login POSTs `https://kitsu.app/api/oauth/token` as form fields in the exact order `username, password, grant_type=password, client_id, client_secret` (KT-22), then GETs `users?filter[self]=true` and reads `data[0].id` (KT-23).
  - The user's password is never persisted; the user id is stored under its own key `track_user_id_3`; username stored as typed (KT-23, flagged against E-16).
  - If the users call fails or returns no data, the token is rolled back and no credentials are written (KT-24, E-11/E-12).
  - Logout clears token, user id and username without a network call (KT-32).
  - `dto/KitsuUser.kt` user id decodes from a JSON string (test KT-85).

### T-30a-008 — Kitsu login and logout dialogs
- Priority: P1
- Effort: M
- Depends: T-30a-007, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt
- Acceptance:
  - Login dialog title "Log in to Kitsu", username label "Email address", password label "Password", confirm "Login" (shows "Loading…" while processing), disabled until both fields are non-blank (E-13 in p1-edge-cases-and-tests.md).
  - On success toast/notice "Logged in" and dialog closes; on failure the error text (e.g. "HTTP error 401") is shown and both fields are marked in error (E-10).
  - A failed login does not clear an existing valid session (Proposal; Aniyomi clears it, E-14, flagged).
  - Logout dialog: title "Log out from Kitsu?", buttons "Cancel" and "Log out" (destructive); success notice "You are now logged out".

### T-30a-009 — Add library entry (POST library-entries)
- Priority: P1
- Effort: M
- Depends: T-30a-006, T-30a-015, T-30a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuAddEntry.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuUtils.kt
- Acceptance:
  - Request body matches fixture `add_manga_expected.json` / `add_anime_expected.json` byte for byte: attributes status, progress (integer, truncated), private; relationships user (string id, type users) and media (numeric id, type manga or anime) (KT-40, KT-41).
  - Dates and rating are not sent on add.
  - Response `data.id` becomes the track's remote_id (library-entry id); decoding accepts string or number ids (KT-85).

### T-30a-010 — Update library entry (PATCH) with null semantics
- Priority: P1
- Effort: M
- Depends: T-30a-006, T-30a-015, T-30a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuUtils.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuDateHelper.kt
- Acceptance:
  - PATCH `library-entries/<library id>` body equals `update_manga_expected.json` (KT-47): `data.type`, `data.id`, attributes status, progress, ratingTwenty (string or JSON null), startedAt, finishedAt (ISO string or JSON null), private.
  - Score 0 sends `ratingTwenty: null` (KT-48); zero dates send null (E-34); progress 12.7 sends 12 (KT-49).
  - A PATCH with remote_id 0 is refused locally with `NotFound` and sends nothing (KT-50, flagged).
  - A 404 response returns `HTTP error 404` and the local track is unchanged (KT-51).

### T-30a-011 — Remove library entry (DELETE)
- Priority: P1
- Effort: S
- Depends: T-30a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt
- Acceptance:
  - `DELETE library-entries/<library id>` with `Content-Type: application/vnd.api+json` and an empty body (KT-57).
  - Unbound track (remote_id 0 or never bound) is refused locally (KT-58, flagged; E-21/E-82). The `bound` flag is defined in p1-topic-remote-id-lifecycle.md.

### T-30a-012 — Find library entry by media id
- Priority: P1
- Effort: S
- Depends: T-30a-006, T-30a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuListSearch.kt
- Acceptance:
  - `GET library-entries?filter[manga_id]=<id>&filter[user_id]=<user>&include=manga` (anime: filter[anime_id], include=anime), with literal brackets in the query (KT-61).
  - Empty data → `None`, so bind takes the add path.
  - Data non-empty but included empty → error (Proposal, flagged against E-23; KT-62).

### T-30a-013 — Get library entry by entry id (refresh source)
- Priority: P1
- Effort: S
- Depends: T-30a-006, T-30a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuListSearch.kt
- Acceptance:
  - `GET library-entries?filter[id]=<library id>&include=manga` (anime: include=anime).
  - Not found → `NotFound("manga")` for manga and `NotFound("anime")` for anime (KT-60, E-24/E-25).

### T-30a-014 — JSON:API DTO decoding
- Priority: P1
- Effort: M
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuListSearch.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuAddEntry.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuSearchItemCover.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuUser.kt
- Acceptance:
  - Every DTO field and nullability from `p1-data-and-config.md` section 2 is represented; `included` defaults to empty when absent (KT-87).
  - Ids accept JSON number or numeric string (KT-85); unknown keys ignored (KT-86); included item without `status` fails decoding (KT-88).
  - Progress decodes as integer and is read back as f64.

### T-30a-015 — Status mapping between Kitsu and local codes
- Priority: P1
- Effort: S
- Depends: T-30a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuUtils.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuListSearch.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt
- Acceptance:
  - Wire ↔ local tables match behaviour section 2.4: current/completed/on_hold/dropped/planned with the kind-specific READING/WATCHING and PLAN_TO_READ/PLAN_TO_WATCH (KT-01).
  - Cross-kind values are rejected (manga with WATCHING, anime with PLAN_TO_READ, and the other cross-kind pairs) (KT-02).
  - Unknown wire status → `UnknownStatus` and bind/refresh fail (KT-03, KT-65).

### T-30a-016 — Date wire format and parse rules
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuDateHelper.kt
- Acceptance:
  - Wire format `%Y-%m-%dT%H:%M:%S%.3fZ` in UTC; epoch 0 → null on write (KT-12); round trip keeps milliseconds (KT-13); empty string and date-only are `BadDate` (KT-14).
  - Output is identical under two different host time zones (KT-11). Behaviour change against Aniyomi, which writes device-local wall time with a literal Z (flagged defect; decision needed before release, see p1-behavior.md section 14).
  - Parse accepts values with or without milliseconds (KT-15, flagged against E-29).

### T-30a-017 — Algolia search (manga and anime)
- Priority: P1
- Effort: M
- Depends: T-30a-006, T-30a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuSearch.kt
- Acceptance:
  - Key fetched from `algolia-keys/media/` with bearer; search POST to the exact Algolia URL with headers `X-Algolia-Application-Id: AWQO5J657S`, `X-Algolia-API-Key`, and JSON content type `application/json; charset=utf-8` (KT-70).
  - `params` = `query=<URLEncoder-compatible encoding>` followed by the verbatim filter constant for the kind (KT-71, KT-78).
  - Hits with subtype `novel` are dropped; null subtype is kept (KT-72).
  - Searching while logged out fails with `NotAuthenticated` and sends no Algolia request (KT-75).

### T-30a-018 — Search hit mapping to track-search items
- Priority: P1
- Effort: S
- Depends: T-30a-017, T-30a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuSearch.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt
- Acceptance:
  - Fields per behaviour section 12: total from chapterCount or episodeCount (null → 0), tracking URL `https://kitsu.app/manga/<id>` or `/anime/<id>`, score = averageRating or -1.0, publishing_status "Publishing" when endDate is null else "Finished", start_date `yyyy-MM-dd` (KT-73, KT-80).
  - Missing required hit fields skip that hit and log it (Proposal; Aniyomi fails the whole search, KT-79, E-57).
  - Start date display uses the host zone (Aniyomi behaviour, E-56); the zone choice is recorded as a decision.

### T-30a-019 — Bind (register) algorithm
- Priority: P1
- Effort: M
- Depends: T-30a-012, T-30a-009, T-30a-010, T-30a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt
- Acceptance:
  - Found: personal fields copied (progress, score, status, started, finished; private NOT copied), remote_id becomes the library id, status forced to READING/WATCHING when has_progress unless COMPLETED, then PATCH (KT-42, KT-43, KT-44, KT-45).
  - Not found: status READING/WATCHING when has_progress else PLAN_TO_READ/PLAN_TO_WATCH, score 0, then POST (KT-46).
  - tracking_url of the caller's track is unchanged by bind (KT-66).
  - Errors propagate to the caller unchanged.

### T-30a-020 — Update on read or watch event
- Priority: P1
- Effort: M
- Depends: T-30a-010, T-30a-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt
- Acceptance:
  - Only when progress_event is true and status is not COMPLETED: progress truncated to integer equal to a positive total → COMPLETED and finished date now (KT-52); otherwise READING/WATCHING and started date now when progress is exactly 1.0 (KT-54).
  - Total 0 never completes (KT-53). DROPPED and ON_HOLD become READING on a read event (KT-55).
  - progress_event false: status unchanged, only PATCH (KT-56).

### T-30a-021 — Refresh from remote
- Priority: P1
- Effort: S
- Depends: T-30a-013, T-30a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt
- Acceptance:
  - Copies progress, score, status, started/finished dates and private (copy_remote_private true) and sets total from the remote count (null → 0) (KT-59, E-43, E-44).
  - Title, tracking URL and remote id are not changed.
  - Missing remote entry → `NotFound` (KT-60).

### T-30a-022 — Remote delete with local unregister
- Priority: P1
- Effort: S
- Depends: T-30a-011, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt
- Acceptance:
  - Removal dialog shows the "Also remove from Kitsu" checkbox only for Kitsu; OK removes the local row first, then calls DELETE when the box was ticked (E-80, E-81).
  - A failed remote delete is shown to the user (Proposal; Aniyomi logs only, E-80) and does not restore the local row.

### T-30a-023 — Private-tracking and reading-date support flags
- Priority: P1
- Effort: S
- Depends: T-30a-001, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt
- Acceptance:
  - Track search offers "Track privately" for Kitsu; the value is sent as `private` on add and on PATCH.
  - Date rows (start, finish) are shown for Kitsu only when a start/finish date is non-zero (mirrors `supportsReadingDates` use in presentation/track).
  - Turning private on does not change status or progress.

### T-30a-024 — Tracker error model and user-visible messages
- Priority: P1
- Effort: S
- Depends: T-30a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuListSearch.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuUtils.kt
- Acceptance:
  - Texts equal Aniyomi where they reach the UI: "Not authenticated with Kitsu", "HTTP error <code>", "Unknown status", "Missing User data from Kitsu", "Missing Manga data from Kitsu".
  - Anime-specific fixes: "Could not find anime" and "Missing Anime data from Kitsu" (E-25, E-27, flagged).
  - Each message has a stable key in area:27-tracking-core so it can be localised later.

### T-30a-025 — HTTP client configuration for Kitsu
- Priority: P1
- Effort: S
- Depends: area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuInterceptor.kt
- Acceptance:
  - One reqwest client with connect timeout 30 s, read timeout 30 s, total timeout 120 s, rustls with system roots (KT-93, E-75).
  - Authed requests send `Cache-Control: no-cache` (flagged against E-74); response bodies of PATCH and DELETE are dropped without leaking the connection (E-73).
  - User-Agent `Kurobayomi v<version> (io.github.blackbox.Kurobayomi)`.

### T-30a-026 — Concurrency model and GTK bridge
- Priority: P1
- Effort: M
- Depends: T-30a-006, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuInterceptor.kt
- Acceptance:
  - All Kitsu I/O runs on one tokio runtime; the GTK thread receives results through a bounded channel and `glib::spawn_future_local` and never blocks.
  - Keyring calls run on the blocking pool (never on the GTK thread).
  - A per-library-entry async lock serialises bind and update on the same entry (KT-95, flagged).
  - Cancellation of a bind mid-flight is documented: the request completes and the local state is updated (E-72).

### T-30a-027 — Refresh single-flight and logout-wins guard
- Priority: P1
- Effort: S
- Depends: T-30a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuInterceptor.kt
- Acceptance:
  - Twenty concurrent bearer requests with an expired token produce exactly one refresh POST (KT-31, E-18).
  - A logout during an in-flight refresh discards the refresh result (generation counter; KT-97).
  - Before release, verify on the live service whether refresh tokens rotate (p1-edge-cases-and-tests.md section 6, step 6); record the answer here as Evidence.

### T-30a-028 — Algolia key caching and rotation recovery
- Priority: P2
- Effort: S
- Depends: T-30a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt
- Acceptance:
  - The Algolia key is fetched once per session and reused for searches (KT-76, flagged against E-51).
  - An Algolia 403 clears the cached key, refetches once and retries once; a second 403 returns `HTTP error 403` (KT-77).

### T-30a-029 — Tracking URL builder and the library-id link defect
- Priority: P2
- Effort: S
- Depends: T-30a-018, T-30a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/dto/KitsuListSearch.kt
- Acceptance:
  - Base URLs are `https://kitsu.app/manga/` and `https://kitsu.app/anime/`.
  - Any track derived from a list response uses the media id for its tracking URL (Proposal; Aniyomi uses the library id, E-58). Test KT-66 covers the bind path.
  - The defect is recorded in the migration notes so that existing stored URLs can be repaired from the media id when they are next refreshed.

### T-30a-030 — Logo and brand colour
- Priority: P2
- Effort: S
- Depends: T-30a-001, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt
- Acceptance:
  - Brand colour is `#332532` (rgb 51, 37, 50) and is used as the Kitsu tile colour.
  - The Kitsu logo image (`ic_tracker_kitsu`) is not bundled; the tile shows the letter "K" on that colour. Decision recorded as Proposal, since the logo is third-party artwork.

### T-30a-031 — Session-lost recovery state (ReauthRequired)
- Priority: P2
- Effort: S
- Depends: T-30a-006, T-30a-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuInterceptor.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt
- Acceptance:
  - When `ReauthRequired` is returned, the Kitsu row shows "Kitsu session expired. Log in again." (Proposal text) with an action that opens the login dialog.
  - Session state is listed in p1-topic-auth-session.md section 2; no automatic re-login (the password is not stored).

### T-30a-032 — Import of Kitsu rows and credentials from an Aniyomi backup
- Priority: P2
- Effort: S
- Depends: area:22-backup-restore, T-30a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt
- Acceptance:
  - Track rows with tracker id 3 are imported with remote_id kept; the bound flag is set for rows that have a remote id (p1-topic-remote-id-lifecycle.md section 4).
  - Credentials from the backup are NOT imported (the Aniyomi "password" slot holds a user id, not a password); the user must log in once after import. Decision recorded.
  - `track_token_3` from a backup, if present and valid, is accepted as the session token (fallback key).

### T-30a-033 — Test suite (wiremock fixtures for every endpoint and mapping table)
- Priority: P1
- Effort: M
- Depends: T-30a-006, T-30a-009, T-30a-010, T-30a-012, T-30a-013, T-30a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuDateHelper.kt
- Acceptance:
  - Every test id KT-01..KT-99 in p1-edge-cases-and-tests.md is implemented or explicitly marked as skipped with a reason.
  - Fixtures listed in p1-edge-cases-and-tests.md section 4 exist under `src/track/kitsu/tests/fixtures/`.
  - One-time live checks in section 6 of that file are run against a throwaway account and their results recorded as Evidence on T-30a-010, T-30a-016, T-30a-025, T-30a-027.

### T-30a-034 — Android-to-Rust replacement map for scope code
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuInterceptor.kt
- Acceptance:
  - Each Android or Kotlin API in the scope maps to a Rust replacement: `android.graphics.Color`→ constant, `androidx.core.net.toUri`→`url::Url`, kotlinx.serialization→serde, okhttp3→reqwest, injekt `injectLazy`→constructor injection, moko `StringResource`→key strings, `withIOContext`→tokio, `SimpleDateFormat`→chrono, `URLEncoder`→`url::form_urlencoded` (table in p1-rust-design.md section 12).
  - No code path depends on an Android class at runtime in the Rust port.

### T-30a-035 — Score display independent of the device locale
- Priority: P3
- Effort: S
- Depends: T-30a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt
- Acceptance:
  - `DecimalFormat("0.#")` (default locale; replaced by Rust `f64` Display) gives "7.5" on every locale (KT-09). Behaviour change for locales that use a comma; recorded as a deliberate replacement.
  - Stored score values are unchanged; only display and the score list labels change.
