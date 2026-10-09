# area:28-tracker-anilist — AniList tracker: overview

Repo root for every source path in this area: `/home/blackbox/aniyomi-src` (read-only, Apache-2.0). All paths below are repo-relative. Labels: **OBSERVED** = what the Kotlin does (with `path:line`), **Proposal:** = what Kurobayomi should do.

Area lead summary: the AniList tracker is 13 Kotlin files (1 510 LOC, `python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 28-tracker-anilist 1`). The generic tracker framework (`Tracker`, `MangaTracker`, `AnimeTracker`, `setRemote*`, `updateRemote`, the delayed queue, interactors) belongs to area:27-tracking-core; this area covers only what AniList adds.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here

- AniList (GraphQL endpoint `API_URL = "https://graphql.anilist.co/"`, `CLIENT_ID = "5338"`, `BASE_URL = "https://anilist.co/api/v2/"`, `BASE_MANGA_URL = "https://anilist.co/manga/"`, `BASE_ANIME_URL = "https://anilist.co/anime/"`, all at `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt:570-574`) is tracker id `2` (`ANILIST = 2L`, constructed in `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:28`).
- Login is OAuth implicit-style: the browser opens `AnilistApi.authUrl()`, AniList redirects to `aniyomi://anilist-auth#access_token=<token>&`, and `TrackLoginActivity` extracts the token. The token is stored in preferences (`track_token_2`), with the Viewer id as the "username" and the token as "password" (see [topic doc](p1-topic-auth-and-token-lifecycle.md) §1).
- Once logged in, the app: searches AniList media; binds a local manga or anime to a remote media list entry; pushes progress (chapters read, episodes watched), status, score, start/finish dates; refreshes remote state; removes remote entries; and exposes AniList-only extras (private tracking, AniSkip MAL-id lookup, AniChart next-airing info).
- Score format is read from the user's AniList account (`mediaListOptions.scoreFormat`) and drives the score picker (POINT_100, POINT_10_DECIMAL, POINT_10, POINT_5, POINT_3).

### 1.2 Why it matters for a usable Kurobayomi

- AniList is the tracker most Aniyomi users keep enabled; without it a daily-use Kurobayomi loses progress sync for the most common account type.
- It is the only tracker whose auth path has a real defect (the expiry arithmetic never expires, see §4 below and [behaviour doc](p1-behavior.md) §4.6). Rewriting it carefully is cheaper than porting the bug.
- It is also the reference implementation for the GraphQL transport (rate limiter, error envelope) that MyAnimeList, Kitsu and Shikimori will reuse via area:27-tracking-core.

### 1.3 Scope boundaries

- In scope: the 13 files under `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/` plus the AniList-visible touch points listed in §2.3.
- Out of scope (owned elsewhere): the tracker framework and track interactors (area:27-tracking-core); the sync_ tables schema (area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations); the settings screens (area:47-settings-screens-general); backup fields (area:21-backup-format-and-create, area:22-backup-restore).

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities (AniList area)

| Kotlin package / file (repo-relative) | LOC | Responsibility |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt` | 375 | Tracker facade. Constants `READING`, `WATCHING`, `COMPLETED`, `ON_HOLD`, `DROPPED`, `PLAN_TO_READ`, `PLAN_TO_WATCH`, `REREADING`, `REWATCHING`, `POINT_100/10/10_DECIMAL/5/3`. Status and score mapping, bind/update/refresh/delete entry points, login/logout, OAuth persistence. |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt` | 589 | All GraphQL operations (`AddManga`, `UpdateManga`, `DeleteManga`, `AddAnime`, `UpdateAnime`, `DeleteAnime`, `Search` x2, find x2, `Viewer`), `createOAuth`, `authUrl()`, the `authClient` with `rateLimit(permits = 85, period = 1.minutes)`. |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistInterceptor.kt` | 61 | Adds `Authorization: Bearer` and `User-Agent`; lazy-loads the OAuth record; expiry check (buggy, see §4). |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistUtils.kt` | 59 | Pure mapping: local status int → AniList `MediaListStatus` string; local 100-point score → user-format score string. |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALAddEntry.kt` | 20 | `SaveMediaListEntry` response (`ALAddEntryResult` / `ALAddEntryData` / `ALAddEntryEntry`). |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALAnime.kt` | 83 | Anime search model `ALAnime` and user-list model `ALUserAnime`, with `toTrack()` mappings. |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALFuzzyDate.kt` | 21 | AniList fuzzy date (year, month, day) → epoch millis; 0 on failure. |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALManga.kt` | 82 | Manga search model `ALManga` and user-list model `ALUserManga`, with `toTrack()` mappings. |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALOAuth.kt` | 17 | Persisted token record (`access_token`, `token_type`, `expires`, `expires_in`) and `isExpired()`. |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALSearch.kt` | 20 | Search envelope: `data.Page.media`. |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALSearchItem.kt` | 99 | Shared media node (titles, cover, format, staff, studios, dates, description) for search and user lists. |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALUser.kt` | 26 | `Viewer` response: `id`, `mediaListOptions.scoreFormat`. |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALUserList.kt` | 58 | `mediaList` response for one (user, media) lookup. |

Outside the `anilist` package but part of the AniList behaviour:

| Package / file (repo-relative) | Responsibility |
|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt` | Base class; `isLoggedIn` = username and password both non-empty (`BaseTracker.kt:32-34`). |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt`, `AnimeTracker.kt`, `DeletableMangaTracker.kt`, `DeletableAnimeTracker.kt` | Interfaces that `Anilist` implements. Owned by area:27-tracking-core. |
| `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt` | Preference accessors used by this area: `anilistScoreType()` (stored key `anilist_score_type`, read by `AnilistUtils.kt`) and the auth-expired flag (stored key `pref_tracker_auth_expired_2`, set by `setCredentials` at `TrackPreferences.kt:32`). |
| `app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt`, `app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt` | Progress-driven `refresh` then `update`; queues to the delayed store on failure. Owned by area:27-tracking-core. |
| `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt` | OAuth redirect landing (`anilist-auth`). |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt` | Login/logout row (`:171-180`). |

### 2.2 Proposed Rust module map

Single binary crate for now (matches the mockup, [design doc](p1-rust-design.md) §1). The tree is:

```
src/tracker/
  mod.rs                  pub use anilist::AniList; the TrackerService trait (area:27)
  model.rs                Track, TrackSearch (shared with area:27)
  anilist/
    mod.rs                AniList struct: public API used by UI and by sync jobs
    status.rs             MangaStatus, AnimeStatus, MediaListStatus strings
    score.rs              ScoreFormat, index <-> stored, display, API score
    auth.rs               OAuthRecord, AuthState, redirect parsing, expiry
    api.rs                reqwest client, RateLimiter (85 / 60 s), error mapping
    queries.rs            const &str GraphQL documents
    dto.rs                serde structs
    convert.rs            DTO -> TrackSearch / Track
    service.rs            update / bind / refresh / delete logic
    html.rs               description HTML -> plain text (ammonia, html-escape)
    date.rs               FuzzyDate <-> epoch ms (chrono, local zone)
    error.rs              AniListError (thiserror)
```

Naming conflict to resolve: the inventory's "Target Rust module" column uses `kurobayomi-core::tracker::anilist::...`, which assumes a workspace split that does not exist yet (`Cargo.toml` is a single package, `kurobayomi` 0.1.0). The tasks use the `src/tracker/` path. **Proposal:** use `src/tracker/anilist/` now; when area:27 splits a `kurobayomi-core` crate, the tree moves intact because nothing in it depends on GTK. Treat `kurobayomi-core::...` in the inventory as a future path.

### 2.3 AniList touch points outside the tracker code

| Touch point (repo-relative) | Behaviour that depends on AniList | Owning task / area |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:2045` | `AniSkipApi().getMalIdFromAL(track.remoteId)` | T-28a-034; consumer area:45-player-core |
| `app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt` (called from `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt`) | Direct GraphQL `Media(id:)` / `Media(idMal:)` for airing data | T-28a-035 (P2); consumer area:39-ui-entry-details-anime |
| `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt` | Redirect handler for `anilist-auth` | T-28a-040 |
| `app/src/main/AndroidManifest.xml:250-260` | Intent filter scheme `aniyomi`, host `anilist-auth` | T-28a-040, T-28a-041 |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt:171-180` | Login opens `AnilistApi.authUrl()`; logout dialog | T-28a-047 |

---

## 3. Reading guide

All docs are in this folder. Read in this order: inventory, topic, behaviour, data, design, edge cases, UI parity, tasks.

1. [p1-inventory.md](p1-inventory.md): the 13 scope files, declared names, port decision and task ids per file.
2. [p1-topic-auth-and-token-lifecycle.md](p1-topic-auth-and-token-lifecycle.md): stored state, the login/logout/expiry state machine, and the auth decision table.
3. [p1-behavior.md](p1-behavior.md): the full observed behaviour (status, score, OAuth, transport, mapping, bind/update/refresh/delete, strings, timings) with `path:line` citations.
4. [p1-data-and-config.md](p1-data-and-config.md): persisted OAuth record, preference keys and defaults, constants, exact GraphQL query text and variables, DTO field types, DB tables `manga_sync` and `anime_sync`, dates and identifiers.
5. [p1-rust-design.md](p1-rust-design.md): Rust module layout, public types, error model, score picker design, auth lifecycle, testing seam, i18n, async model, crates, Android-to-Linux mapping, mockup integration, pitfalls.
6. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): 70 edge cases (E-01 to E-70, each tagged BUG, QUIRK or RISK), failure modes per operation, race conditions, fixtures, upstream tests to port, new Rust tests.
7. [p1-ui-parity.md](p1-ui-parity.md): every AniList-visible surface (settings row, redirect landing, info card, status/progress/score/date pickers, search rows, remove dialog, toasts, keyboard, responsive, empty and error states) and what the mockup lacks.
8. [p1-tasks.md](p1-tasks.md): 58 implementation tasks T-28a-001 to T-28a-058, lint-clean (`python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/28-tracker-anilist/p1-tasks.md` prints `ok`).

---

## 4. Key design decisions and open questions

Each item states the Kotlin behaviour, the proposal, and the recommendation for the user's call.

| # | Question | Kotlin today | Recommendation (default in the plan) |
|---|---|---|---|
| D1 | Where is the access token stored? | Plaintext in preference `track_token_2` (`ALOAuth` JSON). | OS Secret Service via `keyring` (service `kurobayomi`, user `anilist-token`), fallback to a 0600 file with a visible warning (T-28a-046). **Caveat:** T-28a-046 is P1 but login (T-28a-011) is P0, so until T-28a-046 lands the P0 build stores the token in plaintext settings. Recommend promoting T-28a-046 to P0 if the user wants the token off disk in cleartext from day one. |
| D2 | Token lifetime and expiry. | `expires = now_ms + 31 536 000 000`, then the setter applies `*1000 - 60 000`, so `isExpired()` never fires (E-03, `INT:18-21`, `API:516`). | Store an absolute expiry in seconds: `expires_in` from the redirect if present, else 365 days (31 536 000 s) from login. Compare in seconds. Remove the `*1000` setter. |
| D3 | What happens on HTTP 401? | `HttpException(401)` toast; state stays logged in (E-49). | Move to `Expired`, keep the viewer id, clear the token, show the re-login row. Do not open the browser automatically. |
| D4 | Are login failures visible? | `login(token)` catches `Throwable` and calls `logout()` silently (E-01, `ANI:353-355`). | Report the error and show a toast with the reason (`anilist_err_login_failed`). Deliberate deviation from Kotlin. |
| D5 | Does search require login? | Yes: `search` and `searchAnime` go through `authClient` (`AnilistApi.kt`; see [data doc](p1-data-and-config.md) §4.7 and §4.8). | Keep login required until a live check shows unauthenticated `Page(media(...))` works. The docs site was unreachable during research, so this is **not verified**. Open question for the user: is a live check acceptable? |
| D6 | Redirect parsing. | Regex `(?:access_token=)(.*?)(?:&)` needs a trailing `&` (E-02, `TrackLoginActivity.kt:20-30`). | Parse the fragment as form data; no delimiter requirement. |
| D7 | Score picker storage. | Picker stores the display index; `indexOf == -1` can produce a negative score (picker bug). | Store index, never a negative value; `index_to_score` is total over the index range ([design doc](p1-rust-design.md) §5). Widget choice (ListBox for POINT_3/POINT_5, SpinButton for others) is a proposal; area:27 UI parity confirms. |
| D8 | Viewer id type. | String, parsed with `toInt()` at each call (E-06). | `i64`, parsed once at login. |
| D9 | Logout storage. | Writes the literal string `"null"` to `track_token_2` (E-05). | Delete the row. Track rows stay on logout (E-48 quirk kept: local history is not deleted). |
| D10 | Crate layout: single crate or `kurobayomi-core` split? | Not applicable. | Single crate `src/tracker/` now; split later with area:27 (§2.2). Needs the user's call only if a workspace split is wanted sooner. |
| D11 | Rate limit enforcement. | `rateLimit(permits = 85, period = 1.minutes)` (`API:42`). | Keep 85 per 60 s rolling, implemented with `tokio::time::Instant` behind a `Clock` trait for tests. |

Open questions that need the user (none blocks the P0 build):

1. Confirm D1 (keyring with 0600 fallback) and whether T-28a-046 should move to P0.
2. Confirm D5: whether a live check against AniList may be run from this machine during implementation (requires network; no account credentials are stored in the plan).
3. Confirm D4: silent logout on login failure is a bug to fix, not a behaviour to keep.

---

## 5. Cross-area dependencies

### 5.1 This area needs

| Need | From | Why |
|---|---|---|
| `TrackerService` trait, `Track`, `TrackSearch` models, bind/update/refresh/delete framework, delayed queue, progress-driven interactors | area:27-tracking-core | AniList implements the trait; T-28a-001 depends on area:27. |
| Track rows persistence (`manga_sync`, `anime_sync` table shape) and a per-track write serialisation point | area:27-tracking-core (T-28a-039 depends on it); tables defined by area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations | [data doc](p1-data-and-config.md) §6. |
| Settings store for viewer id, score format, expiry, and the "auth expired" flag | area:02-preferences-system | T-28a-011, T-28a-046 depend on it; `pref_tracker_auth_expired_2` is the existing flag. |
| Backup and restore of AniList track fields | area:21-backup-format-and-create, area:22-backup-restore | T-28a-049. |
| Redirect scheme `kurobayomi://anilist-auth` registration, `.desktop` handler, argv/`GApplication::open` routing | area:53-platform-manifest-intents-workers (scheme and intent handling); area:36-ui-shell-navigation-security-webview (opening the browser) | T-28a-040, T-28a-041. |
| Shared HTTP client and network status | area:12-network-stack | The design uses `reqwest` directly; if area:12 provides a shared client, reuse it instead of building one (T-28a-014). |
| Settings screen host for the AniList row (to confirm which settings area hosts Tracking) | area:47-settings-screens-general | T-28a-047, T-28a-048. |
| Toast/snackbar helper (to confirm owner) | area:36-ui-shell-navigation-security-webview | T-28a-045. |

### 5.2 Other areas need from this one

| Consumer | Needs from AniList | Notes |
|---|---|---|
| area:27-tracking-core | Concrete `AniList` implementation of the tracker trait; `TrackAuthExpired` semantics (`pref_tracker_auth_expired_2`); the AniList score conversions as the reference for other trackers | Framework calls `bind`, `update`, `refresh`, `delete` (T-28a-025, T-28a-021, T-28a-026, T-28a-028). |
| area:29-tracker-myanimelist, area:30-tracker-kitsu, area:31-tracker-shikimori-bangumi | Transport pattern (`api.rs` rate limiter and error envelope mapping), the expiry-in-seconds fix | Reuse, do not copy the expiry bug. |
| area:45-player-core, area:46-player-controls-ui | AniSkip MAL id lookup via AniList (T-28a-034) | Player only needs `get_mal_id(remote_id)`. |
| area:39-ui-entry-details-anime | AniChart next-airing lookup (T-28a-035, P2) | Optional; skip cleanly when not logged in or offline. |
| area:38-ui-entry-details-manga, area:39-ui-entry-details-anime | Track info card and pickers (T-28a-031, T-28a-053 to T-28a-056) | Visible surface defined in [UI parity doc](p1-ui-parity.md) §3 to §7. |

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then rows with `file` starting `28-tracker-anilist/` in `/home/blackbox/kurobayomi/plans/TASKS.csv`. Total 58 tasks.

### 6.1 Counts

| Priority | Total | S | M | L | XL |
|---|---|---|---|---|---|
| P0 | 35 | 16 | 19 | 0 | 0 |
| P1 | 16 | 12 | 4 | 0 | 0 |
| P2 | 6 | 5 | 1 | 0 | 0 |
| P3 | 1 | 1 | 0 | 0 | 0 |
| **All** | **58** | **34** | **24** | **0** | **0** |

Lint: `p1-tasks.md` lints `ok`. The four lint errors in the global export are in `40-ui-browse-manga/p1-tasks.md` (`area:41-ui-browse-manga-part-2` does not exist); they are outside this area and were not changed.

### 6.2 P0 critical path (in dependency order)

1. T-28a-001 Tracker module skeleton and dependencies (depends on area:27).
2. T-28a-002 Tracker identity and capability flags; T-28a-003 Typed status enums; T-28a-029 Description HTML to plain text; T-28a-037 Serde DTOs; T-28a-023 Fuzzy dates (all depend on 001).
3. T-28a-004 Status option lists; T-28a-007 Local status to AniList strings (depends on 003); T-28a-005 Score formats (depends on 002); T-28a-006 Score conversions (depends on 005).
4. T-28a-008 Authorize URL and redirect parsing (depends on 002).
5. T-28a-009 Token expiry in seconds (depends on 008); T-28a-010 Auth header and not-authenticated gate (depends on 009).
6. T-28a-014 GraphQL transport with 85-per-minute limiter (depends on 010); T-28a-015 Error envelope and HTTP status mapping (depends on 014).
7. T-28a-011 Login (depends on 010 and 037); T-28a-012 Logout semantics (depends on 011).
8. T-28a-016 and T-28a-017 search queries (depend on 015, 018); T-28a-018 Search DTO conversion (depends on 037, 023, 029).
9. T-28a-019 Find entry queries (depends on 037, 007, 006, 023); T-28a-020 Add entry (depends on 019, 015); T-28a-021 Update entry (depends on 015, 023, 007); T-28a-024 Progress-driven transitions (depends on 003, 007).
10. T-28a-026 Refresh (depends on 019); T-28a-027 Lazy library id resolution (depends on 019, 021); T-28a-028 Remote delete with the MediaList id (depends on 022, 019).
11. T-28a-025 Bind for manga and anime (depends on 019, 020, 021, 024); T-28a-039 Track row persistence and per-track write serialisation (depends on 025, 027, area:27).
12. T-28a-040 Redirect handler (depends on 008, 011); T-28a-047 Settings row (depends on 011, 040).
13. T-28a-053 Status picker (depends on 004, 025); T-28a-054 Score picker (depends on 006); T-28a-057 Search screen replacing the mock (depends on 016, 017, 025); T-28a-031 Info panel (depends on 002, 026, 053, 057); T-28a-038 Unit and behaviour test suite (depends on 037, 014, 024, 025, 026, 027, 028).

Items 10 to 13 can run in parallel once 019 and 025 exist.

### 6.3 Risks

1. **Plaintext token until keyring lands.** T-28a-046 (keyring) is P1 but login is P0 (see D1). Mitigation: promote T-28a-046 or treat the P0 login as a dev-only build until it lands.
2. **AniList schema drift.** The GraphQL schema is not versioned in the repo and the docs site was unreachable during research. Mitigation: pin the query text from [data doc](p1-data-and-config.md) §4, record fixtures, and run one optional live smoke test per release with a throwaway account.
3. **Search-without-login is unverified** (D5). If it works, the login gate can be relaxed; if not, search needs a token before the user can bind anything.
4. **Expiry fix changes behaviour.** Kotlin never expires tokens; Kurobayomi will show "log in again" after 365 days or the redirect's `expires_in`. Mitigation: tests T-28a-009 cover both cases and the restart path with no network call.
5. **Intentional deviations from Kotlin** (E-01, E-02, E-03, E-05, E-49). Each needs a fixture; a parity mismatch is expected and must be listed in the test report rather than silently matched.
6. **Redirect on Linux.** Scheme handling depends on an installed `.desktop` file with `MimeType=x-scheme-handler/kurobayomi;`. A dev build run from `cargo run` does not receive the redirect. Mitigation: T-28a-040 acceptance covers the installed build; a paste-the-URL fallback is a proposal for the settings row.
7. **Dependency on area:27 for persistence.** T-28a-025 and T-28a-039 cannot finish until the framework's track row and write-queue exist; a stub is acceptable only for tests.
8. **Dead code in Kotlin.** The POINT_5 and POINT_3 branches of `toApiScore` are unreachable from `displayScore` (see [behaviour doc](p1-behavior.md) §3). The Rust port must not copy the dead branch as a live path; the picker-index design in [design doc](p1-rust-design.md) §5 covers all formats.

---

## 7. Definition of done for Kurobayomi (AniList)

Each item is observable by a user or by a test in the listed task.

1. Settings, Tracking: the AniList row shows "Log in" when logged out and "Log out" when logged in; clicking "Log in" opens the browser at the AniList authorize URL (T-28a-047, T-28a-008).
2. After approving in the browser, Kurobayomi receives the `kurobayomi://anilist-auth` redirect in the installed build, stores the token, fetches the Viewer, and shows the logged-in state. A failed login shows a toast with the reason and leaves the row logged out (T-28a-011, T-28a-040, T-28a-045).
3. Restart with a stored unexpired token shows logged in with zero HTTP calls; a stored expired token shows "Session expired, log in again" with zero HTTP calls (T-28a-009).
4. A 401 from AniList moves the row to the expired state and clears the token (T-28a-009, T-28a-048).
5. Logout removes the token and leaves no literal `"null"` in storage; track rows are kept (T-28a-012, T-28a-044).
6. Tracker search returns results for a query and shows titles, covers, format, and author/studio lines per [UI parity doc](p1-ui-parity.md) §8; selecting a result and confirming binds it (T-28a-016, T-28a-017, T-28a-057, T-28a-025).
7. Status picker, progress picker, score picker (format-specific: POINT_100, POINT_10_DECIMAL, POINT_10, POINT_5, POINT_3 each show the correct widget and labels), and date picker write the expected AniList values (T-28a-053 to T-28a-056).
8. Reading a chapter or watching an episode pushes progress to AniList through the framework; a failed push is queued and retried, and the queue survives restart (T-28a-021, T-28a-042 via area:27).
9. Private tracking sends the entry without a public status change (T-28a-036).
10. Refresh pulls the remote status, progress, score, and dates into the local track row (T-28a-026).
11. Remove on a bound entry deletes the remote MediaList entry using the MediaList id, not the local row id (T-28a-028).
12. The client never exceeds 85 requests per rolling 60 s (T-28a-014 test with a fake clock).
13. GraphQL `errors` in a 200 response are surfaced as errors, never ignored (T-28a-015).
14. Backup and restore round-trips AniList track fields (T-28a-049).
15. The AniSkip MAL id lookup works for a bound anime and returns nothing when not logged in (T-28a-034).
16. The test suite in T-28a-038 passes in CI with fixtures only (no network).
