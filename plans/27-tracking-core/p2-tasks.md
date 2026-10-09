# area:27-tracking-core part 2 — implementation tasks

Format: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint <file>` must print `ok`.
Source paths are repo-relative to `/home/blackbox/aniyomi-src` unless marked `(kurobayomi)`, which is `/home/blackbox/kurobayomi`.
Behavior references: B§n = `p2-behavior.md`; D§n = `p2-data-and-config.md`; R§n = `p2-rust-design.md`; U§n = `p2-ui-parity.md`; E-nn = `p2-edge-cases-and-tests.md`; T-nn (lowercase test ids in edge file) = test rows there; OAuth topic = `p2-topic-oauth-callback.md`.
Dependencies on part 1 (`p1-*.md` in this folder) are written as `area:27-tracking-core`.

### T-27b-001 — TrackRecord model and field mapping from the domain track rows
- Priority: P1
- Effort: S
- Depends: area:09-domain-source-track-download-release-misc, area:27-tracking-core
- Sources: domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt, domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt, app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt, app/src/main/java/eu/kanade/domain/track/manga/model/MangaTrack.kt
- Acceptance:
  - `kuro-track::record::TrackRecord` has exactly the fields listed in R§3 with the types and defaults of D§1 (`id` -1 when not persisted, `start_date_ms`/`finish_date_ms` 0 when unset, `library_id` Option).
  - Conversion from the stored row to `TrackRecord` and back keeps every field equal for the anime and manga shapes (round-trip unit test with all fields set).
  - Mapping matches `toDbTrack`: `remote_url` is written to the tracking URL field, `progress` maps to `last_episode_seen` or `last_chapter_read` by `EntryKind`.

### T-27b-002 — EntryKind and TrackItem types (anime and manga merged)
- Priority: P1
- Effort: S
- Depends: T-27b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackItem.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackItem.kt
- Acceptance:
  - One `TrackItem { tracker: Arc<dyn TrackerService>, record: Option<TrackRecord> }` type replaces both Kotlin data classes (R§3); there is no separate anime and manga item type.
  - `EntryKind { Anime, Manga }` returns the progress label key (`Episodes` for Anime, `Chapters` for Manga) per U§4.
  - A test constructs an item with and without a record and checks the `None` record is used for the "Add tracking" row (U§2.4).

### T-27b-003 — Assemble tracking rows for an entry (mapToTrackItem)
- Priority: P1
- Effort: M
- Depends: T-27b-002, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt
- Acceptance:
  - `assemble_items` (R§3) returns one item per logged-in tracker of the kind, in `TrackerManager` order (MyAnimeList 1, AniList 2, Kitsu 3, Shikimori 4, Bangumi 5, Komga 6, MangaUpdates 7, Kavita 8, Suwayomi 9, Simkl 101, Jellyfin 102), B§3.4 steps 1-5.
  - An enhanced tracker is dropped when its `accepts` is false for the entry's source, and dropped for a stub source (E-06, T-08).
  - Rows are bound by `tracker_id` only; a row for a logged-out tracker is not shown but is not deleted (E-03).

### T-27b-004 — Home model: subscribe to rows and logins, emit distinct lists
- Priority: P1
- Effort: M
- Depends: T-27b-003, T-27b-040
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt
- Acceptance:
  - The Home model re-assembles on every store change and on every login or logout (fixes D-4, B§3.2, E-01).
  - The emitted list replaces the previous one only when `(tracker.id(), record)` pairs differ (`distinctUntilChanged` equivalent, B§3.2).
  - A store error is logged and the last good list stays visible (B§3.2 `catch`).

### T-27b-005 — Refresh every logged-in tracker on Home open; toast each failure
- Priority: P1
- Effort: M
- Depends: T-27b-004, T-27b-039, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/domain/track/anime/interactor/RefreshAnimeTracks.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - Opening Home starts one refresh per logged-in tracker that has a row for the entry; trackers that are logged out are skipped (RefreshAnimeTracks.kt:29).
  - Each failure produces exactly one toast with text `<Tracker name> error: <message>` (B§3.3, E-72); a null message gives `<name> error: ` with nothing after the colon.
  - One tracker's failure does not cancel the others (per-tracker futures, R§6).
  - A progress bar of 2 px shows at the top of the sheet while any refresh runs (U§2.6, Proposal).

### T-27b-006 — Home row action gating
- Priority: P1
- Effort: S
- Depends: T-27b-004
- Sources: app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHome.kt, app/src/main/java/eu/kanade/presentation/track/manga/MangaTrackInfoDialogHome.kt
- Acceptance:
  - With no record, the row shows only "Add tracking" and no status, progress, score, date, private, or menu actions (B§11, U§2.1).
  - Score text is hidden when score is exactly 0.0 (B§11); start and finish dates show only when the tracker supports reading dates and the stored value is not 0 (B§11, D§5.4 and BaseTracker default false).
  - The private toggle is shown only when `supports_private()` is true (AniList, Bangumi, Kitsu).

### T-27b-007 — Status dialog: radio list, OK writes remotely
- Priority: P1
- Effort: M
- Depends: T-27b-004, T-27b-020, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt
- Acceptance:
  - The options are the tracker's status list in the tracker's order, labels from the tracker (B§5); the initial radio is the stored status (B§5).
  - Cancel and Escape write nothing; OK closes the dialog immediately and runs `set_status` detached (B§5, B§14).
  - Choosing the completion status with a known total sets progress to the total in the same write (B§5 via part 1 rule).

### T-27b-008 — Progress dialog: episode or chapter number with range 0..total or 0..10000
- Priority: P1
- Effort: M
- Depends: T-27b-004, T-27b-020, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt, app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt
- Acceptance:
  - `progress_range(total)` returns `0..=total` when total > 0 and `0..=10000` otherwise (B§6, T-03).
  - Title is "Episodes" for Anime and "Chapters" for Manga (U§4).
  - The spin control clamps to the range, Page Up/Down move by 10, Home is 0, End is the maximum (U§4).

### T-27b-009 — Score dialog and score index mapping
- Priority: P1
- Effort: S
- Depends: T-27b-004, T-27b-020, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt
- Acceptance:
  - The dialog lists `score_options()` in order; the initial selection is `display_score(record)` (B§7).
  - A label not in the list returns `TrackError::UnknownScore` and leaves the record unchanged (fixes D-8, T-13).
  - Unset score uses the tracker's display string, and the Home row hides a score of 0.0 (B§7, E-21).

### T-27b-010 — Start and finish date dialog with picker, bounds, and UTC-to-local conversion
- Priority: P1
- Effort: L
- Depends: T-27b-004, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt, app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt
- Acceptance:
  - Days after today are not selectable; start may not be after a set finish date; finish may not be before a set start date (B§8.3, T-06, E-23, E-24).
  - Stored value is the local midnight of the chosen calendar day; the picker value is the UTC midnight of that day (B§8.4, E-25, section 3 table of E-nn).
  - Confirm with no day selected is not possible (the OK button is disabled); no unwrap of a missing date (fixes D-11).
  - The "Remove" button appears only when a date is set (B§8.1) and opens the remover (task 011).

### T-27b-011 — Date remover confirmation and pop to Home
- Priority: P1
- Effort: S
- Depends: T-27b-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - Title "Remove date?", body "This will remove your previously selected start date from <name>" or the finish-date text, buttons "Cancel" and "Remove" (B§9, U§6.2).
  - Confirm writes 0 for the date (remote first, then local, B§9) and closes both the remover and the picker, returning to Home (`pop_until_home`, B§9).
  - Cancel closes only the remover.

### T-27b-012 — Remote search page: auto-search, loading, results, empty, error, pre-select, confirm
- Priority: P1
- Effort: M
- Depends: T-27b-018, T-27b-020, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackerSearch.kt, src/entry.rs (kurobayomi)
- Acceptance:
  - A non-blank initial query runs a search when the page opens (B§4.1); a blank query does not call the tracker (U§8.2 Idle, fixes E-31).
  - Only the latest search generation writes results; earlier responses are dropped (fixes D-6, T-17).
  - States: loading spinner, "No results found", error text with "Unknown error" fallback, plus Retry (U§8.2).
  - Pre-select only when the current URL is non-empty and equals a result's URL (E-34, E-35, T-18). The bottom bar with Track and Track privately shows only when a result is selected (U§8.1).
  - Track writes `private` from the button used and registers the result for the entry (B§4.3); with no selection, nothing happens.

### T-27b-013 — Enhanced tracker auto-match on "Add tracking"
- Priority: P1
- Effort: S
- Depends: T-27b-003, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt
- Acceptance:
  - For an enhanced tracker, "Add tracking" runs match then register without opening the search page (B§3.9).
  - No match shows the toast "No match found" (E-07); a missing entry returns silently (E-09).
  - A register failure shows the tracker's message prefixed with the tracker name (E-08).

### T-27b-014 — Remove tracking dialog: local delete and optional remote delete
- Priority: P1
- Effort: S
- Depends: T-27b-004, T-27b-041, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, domain/src/main/java/tachiyomi/domain/track/anime/interactor/DeleteAnimeTrack.kt
- Acceptance:
  - Title "Remove <name> tracking?", body "This will remove the tracking locally.", checkbox "Also remove from <name>" only for deletable trackers (AniList, Kitsu, MyAnimeList, Shikimori, MangaUpdates for manga) (B§10.1, B§10.3).
  - Local delete runs first, then the remote delete if the checkbox is set (E-41, T-19); remote and local errors each show a toast (fixes D-13).
  - Confirm button label is "Remove" in the destructive style (fixes D-14; parity divergence noted in U§7).

### T-27b-015 — Open tracker page in browser
- Priority: P1
- Effort: S
- Depends: T-27b-004, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - Opens `remote_url` through `kuro_platform::Opener::open_url` (R§8), only when the URL is not blank (B§3.6).
  - A blank URL shows the toast "No link stored for this entry" (E-43, Proposal) and calls no opener.
  - No `xdg-open` call appears outside `kuro-platform` (addendum rule 1).

### T-27b-016 — Copy tracker link to clipboard with truncated confirmation
- Priority: P1
- Effort: S
- Depends: T-27b-004, T-27b-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - Clipboard text is the full URL; a blank URL does nothing (E-44).
  - Toast "Copied to clipboard:\n<url truncated to 50 characters>" with center truncation and `...` (E-45, T-22), always shown on Linux.
  - Long press or the menu "Copy link" triggers the same action (U§2.2, U§2.3).

### T-27b-017 — Toggle private tracking from the row menu
- Priority: P1
- Effort: S
- Depends: T-27b-006, T-27b-041
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt
- Acceptance:
  - The menu label is "Track publicly" when private and "Track privately" otherwise (U§2.3).
  - Remote write first; the local row changes only after the remote call succeeds; a failure leaves the row and toasts (E-46, T-23).
  - Only shown for trackers with private support (T-27b-006).

### T-27b-018 — Modal navigation stack and typed errors for dialogs
- Priority: P1
- Effort: M
- Depends: T-27b-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, src/ui.rs (kurobayomi), src/entry.rs (kurobayomi)
- Acceptance:
  - `TrackNav` push, pop and `pop_until_home` (R§5.2) reproduce the Kotlin navigator semantics used in B§8.4 and B§9.
  - Every former `!!` on tracker lookup, row track, or date value returns a typed `TrackError` and shows a toast instead of panicking (fixes D-15, E-05, B§17).
  - `src/entry.rs::dialog_of` is replaced by `TrackNav::push` and no tracking code remains in `src/entry.rs` beyond the entry point call.

### T-27b-019 — Deliver aniyomi:// callbacks to the running app (GApplication open) and the .desktop registration
- Priority: P1
- Effort: M
- Depends: T-27b-021, area:36-ui-shell-navigation-security-webview, area:01-foundation-app-bootstrap
- Sources: app/src/main/AndroidManifest.xml, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt
- Acceptance:
  - Application flags include HANDLES_OPEN; the `open` signal creates the main window if missing, presents it, and routes each URI through `parse_callback` (OAuth topic sections 3-4).
  - A second process started with a URI forwards it to the first and exits (single instance; T-28 checklist item 4).
  - The desktop file declares `MimeType=x-scheme-handler/aniyomi;` and the app id matches the GApplication id (OAuth topic section 3).

### T-27b-020 — Detached job runner for non-cancellable remote writes
- Priority: P1
- Effort: M
- Depends: T-27b-001, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - Writes (status, progress, score, dates, private, delete, register) are spawned on the tokio runtime with no abort handle kept; closing the dialog does not cancel them (E-67, T-20).
  - Results return to the GTK main thread by channel only; no GTK object crosses threads (R§6, R§11).
  - Blocking rusqlite calls run through `spawn_blocking`; tracker network calls stay async (R§6).

### T-27b-021 — Parse AniList implicit-grant fragment token
- Priority: P1
- Effort: S
- Depends: area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt
- Acceptance:
  - `access_token=` is matched anchored at the fragment start or after `&`; the value is the raw substring up to the next `&` or the end, not percent-decoded (E-50, E-51, E-55, T-25).
  - A fragment with no token yields `Denied` and no logout (fixes D-2 for AniList).
  - `expires_in` is parsed when present (E-66, T-30).

### T-27b-022 — Route authorization-code callbacks for Bangumi, MyAnimeList, Shikimori, Simkl
- Priority: P1
- Effort: M
- Depends: T-27b-021, area:27-tracking-core, area:31-tracker-shikimori-bangumi, area:32-tracker-mangaupdates-simkl, area:29-tracker-myanimelist
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeListApi.kt
- Acceptance:
  - Host to tracker mapping is exactly `anilist-auth`, `bangumi-auth`, `myanimelist-auth`, `shikimori-auth`, `simkl-auth` (B§12.3); any other host is `UnknownHost` (T-01).
  - `code` is the first occurrence, percent-decoded with `percent_decode_str` so `+` stays `+` (E-52, E-54, T-26); empty value is `Malformed` with no network call (E-53).
  - Missing code or an `error=` parameter is `Denied`, with no logout (fixes D-2).
  - Client secrets come from configuration; a missing secret disables that tracker's login with a toast (E-63, T-29, OQ-1).

### T-27b-023 — Visible feedback for unknown or malformed callbacks (no hanging spinner)
- Priority: P1
- Effort: S
- Depends: T-27b-019, T-27b-022
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt
- Acceptance:
  - Unknown host or malformed URI shows the toast "Unrecognized login link" and returns to the normal window (fixes D-1, E-48).
  - No separate loading window is created for any callback (OAuth topic section 4).
  - The toast text is the MsgKey in T-27b-030, not a hard-coded string.

### T-27b-024 — Login outcome semantics: keep state on failure and denial, show results
- Priority: P1
- Effort: M
- Depends: T-27b-027, T-27b-028, T-27b-022
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeList.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/Bangumi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt
- Acceptance:
  - Denied or cancelled leaves the stored token and username unchanged and shows "<name> login cancelled" (OAuth topic section 6; fixes D-2).
  - Exchange or identity failure removes only what this attempt wrote and shows "<name> login failed" (fixes D-3 silent logout; E-65 end state).
  - Success stores token JSON, username and auth-expired false, and shows "<name> logged in" (B§12.6, D§3).

### T-27b-025 — PKCE verifier per login attempt for MyAnimeList
- Priority: P1
- Effort: S
- Depends: T-27b-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/PkceUtil.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeListApi.kt
- Acceptance:
  - Verifier is 50 random bytes as URL-safe base64 without padding, 67 characters, drawn from `getrandom` (D§6, T-02).
  - The authorize URL carries the verifier as `code_challenge` with no `code_challenge_method` (D§5.1).
  - The verifier is kept in `PendingLogins` and never in a process-wide static (fixes E-56 and E-57 mechanics).

### T-27b-026 — Pending login attempts with TTL and login-CSRF guard (reuses part 1 PendingLogin)
- Priority: P1
- Effort: S
- Depends: T-27b-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeListApi.kt
- Acceptance:
  - A pending attempt (part 1 `PendingLogin`, with `state` and `started_at`) exists per tracker for 5 minutes after its authorize URL is opened; a code callback without one returns `NotAllowed` and makes no network call (OAuth topic section 7.3, OQ-3).
  - A new attempt for the same tracker replaces the old one (last attempt wins, E-56).
  - After an app restart no pending attempt exists and MAL callbacks give "Login expired, start again" (E-57, T-27).

### T-27b-027 — Store tracker tokens in the platform secret store, not preferences
- Priority: P1
- Effort: M
- Depends: area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt
- Acceptance:
  - Token JSON, username and auth-expired flag are read and written through `kuro_platform::SecretStore` under keys `tracker/<id>/oauth`, `tracker/<id>/account` and `tracker/<id>/expired` (D§3.1, R§8).
  - When no secret service is available, login reports "Secret storage is unavailable" and stores nothing in files (R§8).
  - No token value appears in logs or in the config file (OAuth topic section 9; test by grepping the log output in T-24).

### T-27b-028 — Logout semantics: clear only tracker credentials and in-memory auth
- Priority: P1
- Effort: S
- Depends: T-27b-027
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeList.kt
- Acceptance:
  - Logout deletes the token, username, expired flag, and clears the in-memory interceptor auth (B§12.6); local track rows are kept (E-03).
  - Logout is idempotent (E-59, T-24).
  - Settings shows the tracker as logged out immediately after logout (store notification, T-27b-040).

### T-27b-029 — Manual paste fallback for the redirect link
- Priority: P1
- Effort: S
- Depends: T-27b-022, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt
- Acceptance:
  - A settings action "Finish login by pasting the link" opens a dialog with one text field; the text goes through `parse_callback` (OAuth topic section 8).
  - The same CSRF guard applies to pasted links (OAuth topic section 7.3).
  - Empty or unrecognized text shows the same toasts as the routed path.

### T-27b-030 — Message catalog for every key used by this area
- Priority: P1
- Effort: S
- Depends: area:02-preferences-system
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - `MsgKey` enum has one variant per key in B§13 with the exact English text (ErrorNoMatch "No match found", TrackError "%1$s error: %2$s", the remove texts, ActionCancel "Cancel", ActionRemove "Remove", ActionOk "OK", Copied "Copied to clipboard:\n%1$s", and the rest of B§13).
  - Proposal keys added with their texts: `NoTrackingLink` "No link stored for this entry", `TrackerLoggedIn` "<name> logged in", `TrackerLoginFailed` "<name> login failed", `TrackerLoginCancelled` "<name> login cancelled", `UnrecognizedLoginLink` "Unrecognized login link", `TrackerNotConfigured` "<name> is not configured", `LoginExpired` "Login expired, start again", `NoTrackingData` "No tracking data for this tracker".
  - A test checks every `%s` placeholder receives exactly one argument (T-22 covers formatting).

### T-27b-031 — Date display format from app_date_format
- Priority: P1
- Effort: S
- Depends: area:02-preferences-system, T-27b-004
- Sources: app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - Empty `app_date_format` gives the locale short date (`FormatStyle.SHORT` equivalent, D§3).
  - A non-empty value is used as the date pattern for Home date rows (D§3, U§2.1).
  - The preference is read once when the Home opens (B§3.1).

### T-27b-032 — Unit and integration tests for the track dialog logic and login parser
- Priority: P1
- Effort: M
- Depends: T-27b-003, T-27b-010, T-27b-022, T-27b-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt, app/src/main/java/eu/kanade/test/DummyTracker.kt, app/src/main/java/eu/kanade/tachiyomi/util/PkceUtil.kt
- Acceptance:
  - Tests T-01 to T-27 and T-29 to T-35 from `p2-edge-cases-and-tests.md` section 6 exist and pass in `cargo test`, except the manual ones (T-28, T-31).
  - The completion and progress rules in section 5 of the edge file are covered by table tests (T-10, T-11).
  - Fixture values match `DummyTracker` defaults (1..6 statuses, completion 2, scores 0..10) (edge file section 1).

### T-27b-033 — Drop the Android activity lifecycle and main-activity relaunch
- Priority: P3
- Effort: S
- Depends: T-27b-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt
- Acceptance:
  - Decision recorded: `BaseOAuthLoginActivity`, `LoadingScreen`, `finish()` and the `FLAG_ACTIVITY_CLEAR_TOP` relaunch are not ported (R§8 last row).
  - The main window is presented on callback instead (T-27b-019); no code path imitates the activity stack.
  - Injekt `injectLazy` in the activity is replaced by the app context (area:01-foundation-app-bootstrap).

### T-27b-034 — Correct the anime refresh log text (divergence from Kotlin)
- Priority: P2
- Effort: S
- Depends: T-27b-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - The anime log line says `animeId=` and the manga line says `mangaId=`, each followed by the tracker id and name (fixes D-5).
  - The log never includes a token or a full URL (OAuth topic section 9).

### T-27b-035 — Score wheel starts at the stored score even at index 0
- Priority: P2
- Effort: S
- Depends: T-27b-009
- Sources: app/src/main/java/eu/kanade/presentation/track/TrackInfoDialogSelector.kt
- Acceptance:
  - The initial dropdown index is the index of the current score when it is found, including 0 (fixes D-9, E-20, T-14).
  - When not found, the first item is selected (U§5).

### T-27b-036 — Fix the picker day when the device zone is west of UTC
- Priority: P1
- Effort: S
- Depends: T-27b-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt
- Acceptance:
  - The picker value is read as a UTC calendar day, not converted to the local zone first (fixes D-10, E-25).
  - For New York at 09:00 on 2026-10-08, the day 2026-10-09 is rejected as a future date (edge file section 3).
  - Tests with zones UTC, Europe/Berlin, Asia/Kolkata, Pacific/Auckland and America/New_York pass (T-16).

### T-27b-037 — Keep fractional progress unless the user changes the number
- Priority: P1
- Effort: S
- Depends: T-27b-008
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - When the stored progress is 12.9 and the user presses OK without changing the value, no remote write happens (fixes D-7, E-17, T-12).
  - When the user changes the value, the write uses the chosen integer.

### T-27b-038 — AniList token lifetime from expires_in when present
- Priority: P2
- Effort: S
- Depends: T-27b-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt
- Acceptance:
  - When the fragment includes `expires_in`, the stored expiry is now plus that many seconds (fixes D-12, E-66, T-30).
  - When absent, the 31 536 000-second default is used, matching the Kotlin value.

### T-27b-039 — Toast surface for the track windows
- Priority: P1
- Effort: S
- Depends: T-27b-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt, src/ui.rs (kurobayomi)
- Acceptance:
  - A toast overlay at the bottom of the active window shows one message at a time per call, auto-hides after 3500 ms, and can be dismissed by click (U§10).
  - Toasts raised after a dialog closes appear on the main window (T-33).
  - No libadwaita toast type is used (addendum, R§2).

### T-27b-040 — Store change notifications for logins and track rows
- Priority: P1
- Effort: S
- Depends: area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt, domain/src/main/java/tachiyomi/domain/track/anime/interactor/GetAnimeTracks.kt
- Acceptance:
  - A `TrackChanged { entry_id }` and a `LoginChanged { tracker_id }` event are emitted after every committed write (store write, login, logout).
  - Subscribers receive events on the GTK main thread through the channel (R§6).
  - Events are sent after the database transaction commits, so a subscriber that re-reads sees the new row.

### T-27b-041 — Serialize remote writes per entry and tracker
- Priority: P1
- Effort: S
- Depends: T-27b-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt
- Acceptance:
  - Two writes for the same `(entry_id, tracker_id)` run one after the other in submission order (R§6, E-42, E-62, T-20).
  - Writes for different rows run concurrently.
  - Remove after a queued status write runs after that write (E-42).
