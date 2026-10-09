# Area 29-tracker-myanimelist: MyAnimeList tracker (overview)

Area lead document. Read this first, then the docs in the reading guide. All source paths are repo-relative to `/home/blackbox/aniyomi-src` (Apache-2.0, read-only). Labels: `OBSERVED` = what the Kotlin does, `Proposal:` = what Kurobayomi should do.

## 1. Purpose and scope

**What Aniyomi does here.** The MyAnimeList (MAL) tracker is one of the remote tracking services. When a user reads a chapter or watches an episode, the app pushes progress, status, score, and start/finish dates to MAL. It also binds a local title to a MAL entry (search, `id:` lookup, `my:` list search), refreshes state from MAL, and removes the remote entry on request. Login is OAuth2 with PKCE.

**Scope in this area** (11 Kotlin files, 1014 LOC, see [p1-inventory.md](p1-inventory.md)):
- `MyAnimeList.kt` (tracker facade, status and score model, bind, update, refresh, login, logout).
- `MyAnimeListApi.kt` (all HTTP calls to MAL OAuth and MAL API v2, PKCE challenge, DTO mapping, date parsing).
- `MyAnimeListInterceptor.kt` (Bearer header, token refresh under a lock, "login expired" flag).
- `MyAnimeListUtils.kt` (local status to wire string and back).
- `dto/*.kt` (eight files, serde-style response models).

**Why it matters for a usable Kurobayomi.** MAL is the most-used tracker among anime and manga users on the Aniyomi side, and it is the only tracker that the player's AniSkip lookup and the AniChart airing lookup key on (MAL id, see section 5). Without MAL, a user with an existing MAL list cannot keep it in sync from Kurobayomi. Tracking is P1 parity (see [area:27-tracking-core](../27-tracking-core/p1-rust-design.md) rules). No MAL task is P0: the reader and player work without tracking.

**Out of scope here.** The generic tracker interface, the interactors that call trackers (`TrackChapter`, `TrackEpisode`, `AddMangaTracks`, `AddAnimeTracks`, `RefreshMangaTracks`, `RefreshAnimeTracks`), the delayed-tracking store, and the Settings tracking screen belong to [area:27-tracking-core](../27-tracking-core). The shared HTTP client belongs to [area:12-network-stack](../12-network-stack). Preferences belong to [area:02-preferences-system](../02-preferences-system).

## 2. Module map

### 2.1 Kotlin packages touched by MAL

| Package or path (repo-relative) | Responsibility | In this area? |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/` | Tracker facade, API, interceptor, utils | Yes (4 files) |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/dto/` | Request and response models (`MALAnime`, `MALList`, `MALManga`, `MALOAuth`, `MALSearch`, `MALUser`, `MALUserListSearch`) | Yes (7 files) |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/` | `Tracker`, `BaseTracker`, `MangaTracker`, `AnimeTracker`, `TrackerManager` (`val myAnimeList = MyAnimeList(1L)`) | Referenced |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/model/` | `MangaTrackSearch`, `AnimeTrackSearch` search holders | Referenced |
| `app/src/main/java/eu/kanade/tachiyomi/data/database/models/` | DB-facing track interfaces (`MangaTrack`, `AnimeTrack`) | Referenced |
| `app/src/main/java/eu/kanade/domain/track/` (`manga/interactor`, `anime/interactor`, `service`) | Callers (`TrackChapter`, `TrackEpisode`, `Add*Tracks`, `Refresh*Tracks`), `TrackPreferences` | Referenced (area:27) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/` | `TrackLoginActivity` (deep link `aniyomi://myanimelist-auth`), `BaseOAuthLoginActivity` | Android-only, dropped (T-29a-044, T-29a-045) |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt` | Login row and logout dialog | UI parity doc |
| `app/src/main/java/eu/kanade/tachiyomi/ui/entries/{anime,manga}/track/` | Track info dialogs with delete option | UI parity doc |
| `app/src/main/java/eu/kanade/tachiyomi/util/` | `PkceUtil.kt` (verifier), `AniChartApi.kt` (MAL id to AniList id) | Referenced |
| `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` | AniSkip lookup by MAL id (`:2044`) | Referenced |
| `core/common/src/main/java/eu/kanade/tachiyomi/network/` | `NetworkHelper.kt`, `Requests.kt`, `OkHttpExtensions.kt` | Referenced (area:12) |
| `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:175` | Shared `Json` with `ignoreUnknownKeys = true` | Referenced |

### 2.2 Proposed Rust module map

Crates follow [01-foundation-app-bootstrap](../01-foundation-app-bootstrap) and [area:27-tracking-core](../27-tracking-core). Full layout is in [p1-rust-design.md](p1-rust-design.md) section 1.

| Kotlin | Rust (proposal) | Notes |
|---|---|---|
| `MyAnimeList.kt` | `kuro-track::mal::tracker` (`MalTracker`, implements the area:27 `TrackerService`) | Status, score, bind, update, refresh, delete, login state |
| `MyAnimeListApi.kt` | `kuro-track::mal::api` (`MalApi`), `mal::pkce`, `mal::search`, `mal::dates` | reqwest + serde, same URLs and form fields |
| `MyAnimeListInterceptor.kt` | `kuro-track::mal::auth` (`MalAuth`, `TokenStore` trait) | Single-flight refresh behind `tokio::sync::Mutex` |
| `MyAnimeListUtils.kt` | `kuro-track::mal::status` | Table-driven pure functions |
| `dto/*.kt` | `kuro-track::mal::dto`, `mal::token` | serde structs |
| `PkceUtil.kt` | `mal::pkce` | Verifier and challenge |
| `TrackLoginActivity` / `BaseOAuthLoginActivity` | `mal::redirect` (loopback listener, paste fallback) and GTK login dialog in `src/tracking/` | Replaced, see section 4 |
| Credentials and token JSON in `Preference` | `kuro-platform::secret` (secret store) plus config file in `$XDG_CONFIG_HOME/kurobayomi/tracking/myanimelist.toml` | Secret store chosen by proposal |
| `openInBrowser(url, forceDefaultBrowser = true)` | `kuro-platform::browser::open_url` (gio or `gtk::UriLauncher`) | |

Rule (proposal): `kuro-track::mal` has no GTK type and no `cfg(target_os)`. Token storage and browser launch are injected as traits so tests run without a desktop.

## 3. Reading guide

Read in this order: behaviour first, then data, then Rust, then tasks.

1. [p1-behavior.md](p1-behavior.md): the full behaviour spec. Constants, status and score model, date model, OAuth steps, HTTP catalogue, tracker operations, search routing, errors, side effects, concurrency, timings, defects D-01 to D-23, Android-only items.
2. [p1-data-and-config.md](p1-data-and-config.md): status codes and wire enums, DTO field lists, DB and domain fields, preference keys and defaults, on-disk files, endpoint list, limits, example config file.
3. [p1-inventory.md](p1-inventory.md): the 11 scope files with LOC, declared names, port decisions, task ids, plus the list of out-of-scope files that call MAL code.
4. [p1-topic-oauth-login.md](p1-topic-oauth-login.md): OAuth state machine, login and refresh sequences, expiry arithmetic, token storage, client id decision, logout, test matrix.
5. [p1-topic-search-and-list.md](p1-topic-search-and-list.md): search routing (`id:`, `my:`, default), text fan-out, the duplicate `q` bug, the anime `my:` endpoint bug, paging, result shapes.
6. [p1-topic-progress-sync.md](p1-topic-progress-sync.md): what is synced, progress-driven status transitions, bind and refresh, dates, score, partial failures, test matrix.
7. [p1-ui-parity.md](p1-ui-parity.md): the MAL-visible UI: Settings row, logout dialog, login return, delete dialog, status labels, score picker, search cards, toasts, keyboard and responsive notes, mockup gaps.
8. [p1-rust-design.md](p1-rust-design.md): Rust module layout, crates, type signatures (status, score, trait shape, client, auth, login flow), async model, OAuth client config decision, error enum, mockup integration, Android to Linux mapping, pitfalls.
9. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge-case table (E-AUTH, E-STAT, E-DATE, E-SEARCH, E-NET, UI), race conditions, failure modes, synthetic fixtures, unit and integration test list, GTK test notes, manual test list, known gaps.
10. [p1-tasks.md](p1-tasks.md): the 59 implementation tasks T-29a-001 to T-29a-059 (priority, effort, depends, sources, acceptance).
11. [00-overview.md](00-overview.md): this file.

## 4. Key design decisions and open questions

Each item needs the user's call unless marked "settled". The recommendation is the proposal in the cited doc.

| # | Question | Options | Recommendation | Where |
|---|---|---|---|---|
| Q1 | OAuth client id. Kotlin embeds `686b980ff4240fccce7f6a654cea07ce` (`MyAnimeListApi.kt:383`), registered to the Aniyomi project. | (a) reuse it; (b) user registers own MAL app and sets `client_id` or `KUROBAYOMI_MAL_CLIENT_ID` | (b). Reusing another project's registration is not acceptable for a separate app. Without an id, the login row says "Set a MyAnimeList client id to log in" and login does not start. The user must register at `https://myanimelist.net/apiconfig`. | [p1-rust-design.md](p1-rust-design.md) section 5, T-29a-040 |
| Q2 | OAuth redirect. MAL is not confirmed to accept `http://localhost:<port>`. | (a) loopback listener on `127.0.0.1:8765` at `/myanimelist-auth`; (b) custom URI scheme; (c) paste-only | (a) with (c) as fallback. Verify MAL accepts `localhost` with a port during T-29a-039 and T-29a-058. Changing later means re-registering the client. | [p1-topic-oauth-login.md](p1-topic-oauth-login.md) section 6, T-29a-039 |
| Q3 | Score on the wire. Kotlin sends `"8.0"` (`MyAnimeListApi.kt:186`). | (a) keep decimal string; (b) integer | (b) after one live PUT confirms integer is accepted (D-11). | T-29a-020, D-11 |
| Q4 | Anime `my:` search calls `/users/@me/mangalist` (D-01, `MyAnimeListApi.kt:305-321`, `:325`). | (a) keep upstream bug; (b) `/users/@me/animelist` | (b), verified against a live account. | [p1-topic-search-and-list.md](p1-topic-search-and-list.md) section 6, T-29a-015 |
| Q5 | Login failure and cancel. Errors are swallowed (D-03). Cancelled consent calls `logout()` (D-04). | Keep upstream or show errors and keep the working login | Show the error text as a toast. On a missing `code`, keep the login and show a message. | T-29a-006, T-29a-051 |
| Q6 | Refresh retry after a non-401 failure. No backoff upstream (D-06). | Retry per request or 30 s backoff | 30 s backoff. | T-29a-054 |
| Q7 | Pending verifier lost on restart (D-20). | In-memory only or persisted until redirect | Persist until redirect is handled. P2. | T-29a-038 |
| Q8 | Expiry margin for short-lived tokens (D-19). | Fixed 60 s or `min(60, expires_in / 2)` | `min(60, expires_in / 2)`. | T-29a-007 |
| Q9 | Partial dates (D-08). Upstream throws. | Throw or accept `YYYY`, `YYYY-MM` | Accept both, day 1 and January 1. | T-29a-019 |
| Q10 | `my:` results carry no list status (D-15). | Show defaults or fetch `my_list_status` per result | Fetch, after verifying the `fields` parameter. | T-29a-014, T-29a-049 |
| Q11 | Search fan-out (D-13). Upstream is unbounded. | Unbounded or bounded | Bounded `buffered(4)` to keep input order. | T-29a-037 |
| Q12 | Token storage. Android stores credentials as private preferences. | Secret Service (D-Bus) with a `0600` file fallback, or config file | Secret store for token and password, config file for `client_id` and the username. Backups never include the secret store. | T-29a-004, T-29a-046 |
| Q13 | Login and session across backup restore. A restored backup has the MAL row but no login (see [area:22-backup-restore](../22-backup-restore)). | Carry credentials in backups or not | Not carried, matching upstream. The user logs in again. | T-29a-046 |
| Q14 | Auth-expired flag (D-05). Only a new login clears it. | Keep or allow re-login without logout | Keep the flag, show "Log in again", allow re-login without logout. | T-29a-027, T-29a-047 |

**Settled by the source (no user call):** tracker id `1`; status and score tables; the 250-item list page size; the 64-character title query limit; the 60 s token margin (subject to Q8); no private tracking for MAL.

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What MAL needs | Tasks that depend on it |
|---|---|---|
| [area:01-foundation-app-bootstrap](../01-foundation-app-bootstrap) | Cargo workspace, `time`, `tracing`, tokio multi-thread runtime, GResource build step that converts `ic_tracker_mal.webp` to PNG | T-29a-001, T-29a-030 |
| [area:02-preferences-system](../02-preferences-system) | Preference store, enum get (`autoUpdateTrackOnMarkRead` uses `getEnum`) | T-29a-004, T-29a-057 |
| [area:03-db-manga-schema-migrations](../03-db-manga-schema-migrations) and [area:04-db-anime-schema-migrations](../04-db-anime-schema-migrations) | `manga_sync` and `anime_sync` tables with `sync_id = 1` for MAL | T-29a-019, T-29a-023 |
| [area:05-data-repos-entries-items](../05-data-repos-entries-items) | Row types `MangaTrackRow`, `AnimeTrackRow` | Track DB mapping |
| [area:12-network-stack](../12-network-stack) | HTTP client builder (DoH, user agent), request helpers, network-state helper. MAL builds its own client with no cookie jar and no HTTP cache (see `p1-rust-design.md` section 4). Open item E-NET-7: MAL uses system DNS unless area:12 provides a resolver | T-29a-009, T-29a-034, T-29a-036 |
| [area:22-backup-restore](../22-backup-restore) | Backup excludes the secret store; restored row shows logged out | T-29a-046, the restore fixture in T-29a-027 |
| [area:26-library-update-jobs](../26-library-update-jobs) | Delayed tracking jobs (`DelayedMangaTrackingUpdateJob`, `DelayedAnimeTrackingUpdateJob`) become tokio tasks | T-29a-056 and offline paths |
| [area:27-tracking-core](../27-tracking-core) | `TrackerService` trait shape, `TrackItem`, `TrackerManager`, `TrackChapter`/`TrackEpisode` callers, `setRemote*` defaults, tracking preferences, delayed store, Settings tracking screen, bind and refresh interactors | T-29a-001, T-29a-024, T-29a-025, T-29a-026, T-29a-029, T-29a-048, T-29a-049, T-29a-057, T-29a-059 |

Assumption to confirm with area:27: the trait shape in `p1-rust-design.md` section 3.3 is the one area:27 will provide. If area:27 changes names, `p1-rust-design.md` section 3.3 is updated in the same change.

### 5.2 What others need from this area

| Consumer | What it needs | Source |
|---|---|---|
| [area:27-tracking-core](../27-tracking-core) | `MalTracker` implementing the trait; MAL tracker id `1`; `supportsReadingDates = true`; delete support | T-29a-048 |
| [area:39-ui-entry-details-anime](../39-ui-entry-details-anime) (AniChart airing lookup, per `p1-behavior.md` section 12) | MAL id as `remoteId` for the AniChart `Media(idMal:...)` query | T-29a-032 |
| Player area (AniSkip lookup, `PlayerViewModel.kt:2044`; proposed owner [area:45-player-core](../45-player-core)) | MAL id as `remoteId` | T-29a-033 |
| Settings (proposed owner [area:47-settings-screens-general](../47-settings-screens-general)) | Login row states, logout dialog, auth-expired state | T-29a-029, T-29a-047, T-29a-051 |
| Track info dialogs ([area:38-ui-entry-details-manga](../38-ui-entry-details-manga), [area:39-ui-entry-details-anime](../39-ui-entry-details-anime)) | Delete dialog option "Also remove from MyAnimeList" | T-29a-031, T-29a-056 |
| Tracking sheet state | Shared in-memory tracker state | T-29a-059 |

Owners marked "proposed" are inferred from the names of the Kotlin files and are not yet confirmed in the plan index. The area leads of those areas confirm them.

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, rows with `file` starting `29-tracker-myanimelist/` in `/home/blackbox/kurobayomi/plans/TASKS.csv`.

**Counts: 59 tasks (T-29a-001 to T-29a-059).**

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 0 | 0 | 0 | 0 | 0 |
| P1 | 33 | 15 | 0 | 0 | 48 |
| P2 | 8 | 0 | 0 | 0 | 8 |
| P3 | 3 | 0 | 0 | 0 | 3 |
| **Total** | **44** | **15** | **0** | **0** | **59** |

P2 tasks: T-29a-030 (logo), T-29a-032 and T-29a-033 (AniChart and AniSkip consumers), T-29a-038 (persist verifier), T-29a-053 (401 recovery), T-29a-054 (refresh backoff), T-29a-055 (per-title PUT serialisation), T-29a-056 (skip PUT after local delete).
P3 tasks (drops): T-29a-044 (`TrackLoginActivity` intent filter), T-29a-045 (`BaseOAuthLoginActivity` loading screen), T-29a-046 (backup does not carry login).

**Critical path (P1, in order).** No MAL task is P0, so this is the chain to a working login and a progress push. Each step lists its blockers.

1. T-29a-040: user registers a MAL app and supplies `client_id` (no dependency, user action).
2. T-29a-001: module skeleton and constants (needs area:27 trait names).
3. T-29a-005 (PKCE and authorize URL) and T-29a-007 (token model and expiry), in parallel.
4. T-29a-039: loopback redirect (after T-29a-005).
5. T-29a-004: credential and token persistence (after T-29a-001 and area:02).
6. T-29a-009: authenticated request path (after T-29a-008 and area:12); T-29a-008 needs T-29a-004 and T-29a-007.
7. T-29a-006: login completion (needs T-29a-004, T-29a-005, T-29a-007, T-29a-009, T-29a-010, T-29a-039).
8. T-29a-010: current user request (after T-29a-009).
9. T-29a-019 (dates), T-29a-002 (status), T-29a-003 (score), then T-29a-020 and T-29a-021 (PUT list entry, manga and anime).
10. T-29a-023 (progress-driven transitions), T-29a-024 (`setRemote*` defaults).
11. T-29a-016, T-29a-017, T-29a-018 (details and list fetch), then T-29a-025 (bind), T-29a-026 (refresh).
12. T-29a-048: `MalTracker` implements the area:27 trait. This is the integration point, and it needs T-29a-006, T-29a-016, T-29a-017, T-29a-022, T-29a-023, T-29a-025, T-29a-026.
13. T-29a-029 (Settings row) and T-29a-058 (live verification against a real MAL account, needs T-29a-039, T-29a-040, T-29a-041).

**Risks.**

- MAL API documentation could not be read during planning (see [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 8). Every MAL behaviour claim comes from the Kotlin source. The live recordings in T-29a-041 and T-29a-058 are the gate for decisions D-01, D-09, D-11, D-12, D-19, and the redirect acceptance (Q2).
- Client id registration (Q1) is a manual user step. Login cannot be tested end to end without it.
- The redirect port and scheme (Q2) cannot be changed without re-registering the MAL app.
- Upstream has no tests for the MAL code. Every test in the plan is new, and the fixtures are synthetic until recorded (T-29a-041).
- Proposed fixes change visible behaviour (D-03, D-04, D-10, D-11, D-15). Each one needs a user-visible toast or doc note so the change is not silent.
- Area:27 trait shape is assumed. A change there forces edits in `p1-rust-design.md` section 3.
- N+1 detail requests (D-13) can hit MAL rate limits on large `my:` lists. The rate limit values are not known from the source. The bounded fetch (Q11) reduces the risk but does not remove it.
- The anime `my:` search (D-01) is broken upstream, so there is no working reference behaviour to copy until Q4 is decided.

## 7. Definition of done for this area

Each item is observable by the user or by a test named in the task file. A test that needs a real MAL account is marked (live).

1. Settings > Trackers shows a MyAnimeList row. With no client id configured, the row says "Set a MyAnimeList client id to log in" and tapping it does not open a browser.
2. With a client id configured, tapping the row opens the MAL authorize page in the default browser. The row shows "Log in" while the loopback listener waits, with a Cancel button and a "Paste redirect URL" fallback. It times out after 5 minutes (per [p1-ui-parity.md](p1-ui-parity.md) section 1).
3. After a successful login, the row shows the MAL username and is checked. A failed login shows the error text in a toast and leaves the row logged out (D-03). Cancelling a re-login keeps the working login (D-04).
4. Logging out asks for confirmation with "Log out from MyAnimeList?" and removes the token and password from the secret store. The config file keeps only `client_id` and the username. No `"null"` value is written (D-07).
5. Restarting Kurobayomi keeps the login (token and username load from the secret store). Expired access tokens refresh once per expiry window under a single-flight lock (T-29a-008, test with the fake server in T-29a-043).
6. A 401 from the refresh endpoint shows "MAL: Login has expired" on the API actions and "Log in again" in the Settings row. Re-login works without logout (D-05).
7. Searching a title shows MAL results with cover, title, and type, and the query is truncated to 64 characters. `id:<n>` opens that exact entry. `my:<title>` searches the user's own list, and for anime it uses the anime list endpoint (D-01 fixed, live).
8. Binding a local manga or anime to MAL writes the MAL id as `remoteId`, sets status and progress by the bind rules in [p1-topic-progress-sync.md](p1-topic-progress-sync.md) section 2.3 (a local read forces READING even when remote is ON_HOLD or DROPPED, D-16), and the local row is written only after the MAL PUT succeeds.
9. Reading a chapter or watching an episode sends one PUT with the new progress and the status rules of [p1-topic-progress-sync.md](p1-topic-progress-sync.md) section 2.1: Completed is terminal, Completed needs a known total greater than 0, the start date is set only at exactly 1.0 progress, and the finish date is the current time in milliseconds. The PUT is serialised per title (T-29a-055).
10. Changing the score in the tracking sheet sends the score as an integer from 0 to 10 once Q3 is settled (until then the Kotlin form `"8.0"` is kept). A remote score of 0 is shown as unscored. An unknown score string is rejected with an error and no request is sent (D-12).
11. Refreshing a tracked title pulls status, progress, score, and dates from MAL. A title missing on MAL is re-added and the user sees the toast "Re-added to MyAnimeList" (D-10).
12. Removing a tracked title asks "Also remove from MyAnimeList" in the delete dialog. Choosing it sends DELETE and removes the local row. Choosing it off leaves MAL unchanged.
13. With the network offline, a progress update is queued by the area:27 delayed store and sent when the network returns (test in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) manual list step 13).
14. The AniChart airing lookup and the AniSkip skip-times lookup use the MAL id stored on the track (T-29a-032, T-29a-033).
15. The Kurobayomi build contains no client id from the Aniyomi project. A grep of the built binary and the source for `686b980ff4240fccce7f6a654cea07ce` returns nothing.
16. All unit, fixture, and fake-server tests in T-29a-041, T-29a-042, and T-29a-043 pass in CI with no network. The live checklist T-29a-058 is run by the user once against a test MAL account and its results are recorded.
