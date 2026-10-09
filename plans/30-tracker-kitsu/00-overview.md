# Area 30-tracker-kitsu: Kitsu tracker overview

Area lead overview. Repo root for every Aniyomi path: `/home/blackbox/aniyomi-src` (repo-relative paths below). Rust target: `/home/blackbox/kurobayomi` (crate `kurobayomi`). Priority P1 (tracking parity).

---

## 1. Purpose and scope

Kitsu is one of the online tracking services that Aniyomi can sync reading and watching progress to. This area covers only the Kitsu implementation: the OAuth password-grant login, the JSON:API library-entry CRUD, the Algolia-backed title search, status and score mapping, and the wire date format. The generic tracker layer (interfaces, `TrackerManager`, bind/update orchestration, track sheet UI, delayed retries) belongs to area:27-tracking-core.

Why it matters for a usable Kurobayomi: Kitsu is the only tracker in this area, and it is a P1 parity feature. Kurobayomi is usable without it (P0 is reading, watching, local DB, and a source), so this area has no P0 work. A user who syncs to Kitsu gets remote progress, status, score, and dates, and can search and bind titles from the track sheet.

Scope: 11 Kotlin files (`app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/` and its `dto/` subpackage). No scope file renders UI. The Kitsu login and logout dialogs live in `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt` (outside scope, specified in T-30a-008). No Kotlin unit tests exist for this package; the test cases are derived from source (`p1-edge-cases-and-tests.md`).

Out of scope for this area: AniList, MyAnimeList, Shikimori, Bangumi, MangaUpdates, Simkl, and the enhanced trackers (other areas 28, 29, 31 to 33).

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities

| Package or file (repo-relative) | Responsibility | Owner |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt` | Tracker facade: ids, status and score tables, bind, update, refresh, delete, login, logout, token save and restore | this area |
| `.../data/track/kitsu/KitsuApi.kt` | HTTP client for every endpoint (OAuth, JSON:API, Algolia), two OkHttp clients (`client`, `authClient`), constants | this area |
| `.../data/track/kitsu/KitsuInterceptor.kt` | OkHttp interceptor: bearer injection, header overrides, refresh-on-expiry | this area |
| `.../data/track/kitsu/KitsuUtils.kt` | Local status and score to wire values (`toApiStatus`, `toApiScore`) | this area |
| `.../data/track/kitsu/KitsuDateHelper.kt` | Epoch millis to and from the Kitsu timestamp string | this area |
| `.../data/track/kitsu/dto/*.kt` (11 files) | Serializable request and response models: `KitsuOAuth`, `KitsuAddEntry`, `KitsuListSearch`, `KitsuSearch`, `KitsuSearchItemCover`, `KitsuUser` | this area |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt`, `Tracker.kt`, `MangaTracker.kt`, `AnimeTracker.kt`, `TrackerManager.kt` | Tracker interfaces, default setters (`setRemote*`, `updateRemote`), `isLoggedIn`, `saveCredentials`, `logout`, `KITSU = 3L` | area:27-tracking-core |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/model/MangaTrackSearch.kt`, `AnimeTrackSearch.kt` | Search result objects (`cover_url`, `summary`, `publishing_status`, and so on) | area:27-tracking-core |
| `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt` | Keys `pref_mangasync_username_3`, `pref_mangasync_password_3`, `pref_tracker_auth_expired_3`, `track_token_3` | area:02-preferences-system |
| `app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt`, `.../anime/interactor/AddAnimeTracks.kt` | Calls `bind` when a title is added, then sets progress and start date | area:27-tracking-core |
| `app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt` | Calls `refresh` then `update(track, true)` on a read event; failures go to the delayed store | area:27-tracking-core |
| `app/src/main/java/eu/kanade/domain/track/manga/interactor/SyncChapterProgressWithTrack.kt` | No-op for Kitsu (not an enhanced tracker) | area:27-tracking-core |
| `app/src/main/java/eu/kanade/tachiyomi/data/database/models/{manga,anime}/*Track*.kt`, `app/src/main/java/eu/kanade/domain/track/{manga,anime}/model/*Track.kt` | Data and domain track models and mappers | area:06, area:09 |
| `data/src/main/sqldelight/data/manga_sync.sq`, `data/src/main/sqldelightanime/dataanime/anime_sync.sq` | Storage of tracks (`remote_id`, `remote_url`, dates, `private`) | area:03, area:04 |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt` | Kitsu login and logout dialogs, remove-tracking dialog | settings area (to confirm, likely area:47-settings-screens-general) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/entries/{manga,anime}/track/*TrackInfoDialog.kt` | Remove-tracking dialog with "Also remove from Kitsu" checkbox | area:38, area:39 |
| `core/common/src/main/java/eu/kanade/tachiyomi/network/{NetworkHelper,Requests,OkHttpExtensions,HttpException}.kt` | Shared client, timeouts, cache, `GET`/`POST` helpers, `awaitSuccess`, `HttpException` | area:12-network-stack |
| `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt` | Shared `Json` instance (`ignoreUnknownKeys = true`, `explicitNulls = false`) | area:01-foundation-app-bootstrap |
| `i18n/src/commonMain/moko-resources/base/strings.xml`, `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` | Message keys used by the dialogs | area:51-i18n |

### 2.2 Proposed Rust module map

Full signatures are in `p1-rust-design.md` sections 1 to 9. Kitsu subtree (owned here):

```
src/track/kitsu/mod.rs        Kitsu facade: ids, status/score tables, bind/update/refresh/delete/search, login/logout
src/track/kitsu/error.rs      KitsuError (thiserror) with user-visible texts
src/track/kitsu/auth.rs       OAuth, AuthSession (token + refresh mutex), SecretStore (keyring, settings fallback)
src/track/kitsu/api.rs        REST calls: library entries, users, OAuth token
src/track/kitsu/search.rs     Algolia key (cached), Algolia query, hit to SearchHit
src/track/kitsu/dto.rs        serde structs for bodies and responses
src/track/kitsu/mapping.rs    status tables, score list, index and display, wire score
src/track/kitsu/dates.rs      wire timestamp in UTC
src/track/kitsu/tests/        wiremock tests and fixtures
```

Shared types owned by area:27 at `src/track/mod.rs`: `Kind`, `TrackRecord`, `SearchHit`, `TrackerRow`.

---

## 3. Reading guide

- [p1-inventory.md](p1-inventory.md): the 11 scope files with LOC, every declared name, port decision, target module, and the task ids for each file.
- [p1-behavior.md](p1-behavior.md): the full behaviour spec (OBSERVED vs Proposal): identity, status and score tables, auth lifecycle, request pipeline, library-entry operations, bind, update, refresh, delete, search, mapping, dates, concurrency, timings, edge-case list, interaction map.
- [p1-data-and-config.md](p1-data-and-config.md): OAuth and DTO field tables, every endpoint with exact URL, body and response, the Algolia constants, preference keys, message keys, token and credential storage, the `manga_sync` and `anime_sync` columns, enumerations, date format, Rust data model.
- [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): the edge-case table (E-01 to E-74), the upstream test list, the new Rust test list (KT ids), fixtures, race-condition tests, one-time live checks against the service, and the failure-mode checklist.
- [p1-rust-design.md](p1-rust-design.md): Rust module layout, public signatures, error enum, auth and refresh algorithm, mapping functions, API layer, search, facade logic, dates, concurrency and timeouts, crate choices, Android-to-Rust table, integration with the Kurobayomi mockup, testing hooks, pitfalls.
- [p1-tasks.md](p1-tasks.md): 35 implementation tasks (T-30a-001 to T-30a-035) with priority, effort, dependencies, sources and acceptance criteria.
- [p1-topic-auth-session.md](p1-topic-auth-session.md): deep dive on the session state machine (LoggedOut, LoggedIn, Expired, Reauth, PartialLogin), transitions, concurrency rules, and the user-visible states.
- [p1-topic-remote-id-lifecycle.md](p1-topic-remote-id-lifecycle.md): deep dive on the three Kitsu id namespaces (media, library-entry, user) and how `remote_id` changes across bind, refresh, update and delete; the proposed `bound` flag.

`p1-ui-parity.md` is not written: no scope file renders UI. The UI consequences are in `p1-rust-design.md` section 13 and in the tasks.

---

## 4. Key design decisions and open questions

Each item needs the user's call. The recommendation is what the area lead proposes.

1. **Date wire format (defect in Aniyomi).** `KitsuDateHelper.kt:9-10` uses `SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSS'Z'")` with the device default zone, so local wall-clock time is labelled `Z`. Kitsu reads it as UTC, which shifts stored dates by the device's UTC offset. Recommendation: write and read UTC (`T-30a-016`). Consequence: entries that Aniyomi already wrote keep a shifted value until refreshed. Decide before release whether a one-time repair pass is wanted.
2. **Store the Kitsu password?** Aniyomi never stores it (the "password" slot holds the user id). Recommendation: never store it. After a backup import the user logs in again (`T-30a-032`).
3. **Token storage.** Recommendation: Secret Service through `keyring` (KWallet provides it on Plasma), with the settings table as fallback under `track_token_3`. The fallback is a plain SQLite value protected only by file mode 0600. Decide whether that fallback is acceptable or whether the app should refuse to store a token without a keyring.
4. **Refresh-token rotation.** Unknown from source. Recommendation: implement single-flight refresh anyway (`T-30a-027`) and run the live check in `p1-edge-cases-and-tests.md` section 6, step 6, before enabling Kitsu by default.
5. **Refresh 4xx behaviour.** Aniyomi sends the request with the expired token and gets 401. Recommendation: return `ReauthRequired` on a 4xx refresh and show "Kitsu session expired. Log in again." (`T-30a-006`, `T-30a-031`). Changes the message the user sees.
6. **Login error UX.** Aniyomi shows the error as a toast and marks both fields red. Recommendation: inline error label in the login window, plus the area:27 toast for other failures. A failed login must not clear an existing valid session (`T-30a-008`).
7. **Legacy tracking URLs.** Search results and list-derived tracks can carry a library-id URL (`KitsuListSearch.kt` `tracking_url = mangaUrl(remote_id)`). Recommendation: rebuild from the media id on the next refresh and on import (`T-30a-029`).
8. **Anime error texts.** Aniyomi says "Could not find manga" (`KitsuApi.kt:388`) and "Missing Manga data from Kitsu" (`KitsuListSearch.kt:52`) on the anime path. Recommendation: fix to "Could not find anime" and "Missing Anime data from Kitsu" (`T-30a-024`).
9. **Score display locale.** `DecimalFormat("0.#")` (`Kitsu.kt:98,107,112`) follows the device locale. Recommendation: always `.` (Rust `f64` Display) (`T-30a-035`).
10. **Start-date display zone (search).** Aniyomi formats `startDate` in the host zone, so a UTC-midnight date can show as the previous day. Recommendation: format search dates in UTC (`T-30a-018`).
11. **Search average rating.** Algolia `averageRating` is not divided by 2 and its scale is not confirmed by source. Recommendation: label it "Average rating" in the UI and never feed it into `toApiScore` (`p1-behavior.md` section 3.5). Verify the scale with live check 2.
12. **Logo.** Recommendation: a letter tile "K" on `#332532`, no bundled third-party artwork (`T-30a-030`).
13. **Ship Kitsu in v1 at all?** Recommendation: yes, as P1. It is small (35 tasks, about 35 to 60 engineer-days upper bound) and users who track on Kitsu depend on it.

---

## 5. Cross-area dependencies

### 5.1 This area needs from others

| Area | What is needed | Tasks |
|---|---|---|
| area:27-tracking-core | Tracker trait and registry; `TrackerManager.KITSU = 3`; shared `Kind`, `TrackRecord`, `SearchHit`, `TrackerRow`; the bound-flag decision on `TrackRecord`; delayed-update store used by `TrackChapter`; track sheet and remove dialog; toast and notification plumbing; stable keys for messages | T-30a-001, 008, 019, 020, 022, 023, 024, 030 |
| area:12-network-stack | Shared `reqwest` client, user agent, timeouts, rustls native roots | T-30a-006, 025 |
| area:02-preferences-system | Settings table for `track_user_id_3`, username, and the `track_token_3` fallback; file mode 0600 for the database | T-30a-005, 007 |
| area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations | `manga_sync` and `anime_sync` columns; new `bound` column; `CHECK (score >= 0 AND score <= 10)`; `private` as 0 or 1 | T-30a-011, 032 |
| area:06-data-repos-history-track-source-misc | Track repositories: insert, update, delete, get by manga id | T-30a-019, 022 |
| area:09-domain-source-track-download-release-misc | Domain track models and mappers | T-30a-019, 021 |
| area:01-foundation-app-bootstrap | One tokio runtime, GTK bridge via `glib::spawn_future_local`, logging | T-30a-026 |
| area:22-backup-restore | Tracker rows with id 3 in backups; token fallback key | T-30a-032 |
| area:26-library-update-jobs | Retry job for failed delayed track updates | (behaviour section 8.2, consumed by area:27) |
| area:51-i18n | Stable message keys for the dialog texts and error texts | T-30a-008, 024 |
| settings area (likely area:47-settings-screens-general, to confirm) | Tracking settings row that hosts the Kitsu login and logout dialogs | T-30a-008 |
| area:38-ui-entry-details-manga, area:39-ui-entry-details-anime | Track sheet Kitsu row; date rows; "Track privately" button; remove dialog checkbox | T-30a-022, 023 |
| area:49-theme-and-design-system | Brand colour token `#332532` | T-30a-030 |

### 5.2 Others need from this area

| Area | What they need | Where |
|---|---|---|
| area:27-tracking-core | `Kitsu` implementation of the tracker interface: id 3, status lists and labels, score list and index mapping, `display_score`, login and logout, search, bind, update, refresh, delete, capability flags | `p1-rust-design.md` section 2 |
| area:22-backup-restore | Import rules for tracker id 3: keep `remote_id`, set `bound`, do not import credentials, accept `track_token_3` | T-30a-032 |
| area:03, area:04 | `bound` column (proposal), score CHECK, wire-to-column mapping | `p1-topic-remote-id-lifecycle.md` section 4, `p1-data-and-config.md` section 8 |
| area:38, area:39 | Score display rule, date-row rule, status labels, remove-checkbox rule | T-30a-022, 023 |
| area:51-i18n | Message key list | `p1-data-and-config.md` section 6 |

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, read from `/home/blackbox/kurobayomi/plans/TASKS.csv` (rows where file starts with `30-tracker-kitsu/`).

Total 35 tasks.

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 0 | 0 | 0 | 0 | 0 |
| P1 | 16 | 12 | 0 | 0 | 28 |
| P2 | 5 | 0 | 0 | 0 | 5 |
| P3 | 2 | 0 | 0 | 0 | 2 |
| Total | 23 | 12 | 0 | 0 | 35 |

Effort bound (S up to 1 day, M up to 3 days): 23 + 36 = 59 engineer-days upper bound, roughly 35 to 60 days.

P0 critical path: none. This area has no P0 tasks. The gate for all P1 work is area:27-tracking-core, because T-30a-001 depends on the tracker interface.

P1 critical chain, in order (the longest dependency chain to the test suite):
1. T-30a-004 OAuth model and expiry (no dependencies)
2. T-30a-005 token persistence (needs area:02)
3. T-30a-006 authenticated pipeline and refresh (needs area:12)
4. T-30a-010 PATCH with null semantics (needs T-30a-015 and T-30a-016; T-30a-015 needs T-30a-002, which needs T-30a-001 from area:27)
5. T-30a-019 bind (needs T-30a-012, T-30a-009, T-30a-010, T-30a-003)
6. T-30a-020 update on read event (needs T-30a-010 and T-30a-015)
7. T-30a-033 test suite (needs T-30a-006, 009, 010, 012, 013, 017)

Parallel branches: T-30a-014 (DTOs, no dependencies), T-30a-016 (dates, no dependencies), T-30a-017 to T-30a-018 (Algolia search), T-30a-007 to T-30a-008 (login and dialogs, after area:27 UI exists), T-30a-026 to T-30a-027 (concurrency).

Risks:
1. Undocumented server behaviour: refresh-token rotation, whether `PATCH` accepts numeric `data.id` (T-30a-033 step 4), cacheability of authed GETs, `expires_in` values, Algolia key rotation. All of these need the one-time live checks in `p1-edge-cases-and-tests.md` section 6 before release.
2. Kitsu API and client credentials: `CLIENT_ID` and `CLIENT_SECRET` are compiled into Aniyomi and are reproduced in the plan. If Kitsu revokes them, login and refresh stop working for every user. There is no fallback in the design.
3. Date migration: entries written by Aniyomi with the local-time defect will show a shifted date until refreshed (open question 1).
4. Credential loss: users lose the session on backup restore and must log in again. The password is never stored, so no silent re-login is possible.
5. Keyring availability: no Secret Service on headless or non-KDE sessions. The settings fallback stores the token in the database.
6. No upstream tests exist for this package; test cases are derived from source, so the behaviour in the port is only as good as the derived cases and the live checks.
7. Concurrency: Aniyomi has no refresh serialisation. The port adds single-flight; the behaviour of a server that rotates refresh tokens under concurrent use is still unknown.
8. Locale and zone: score display and search dates change with the host locale and zone in Aniyomi; the port fixes both and must document the change.

Document inconsistencies found by the area lead (to resolve in the owning docs):
- `p1-rust-design.md` section 2 `TrackRecord` has no `bound` field, but T-30a-011, T-30a-032 and `p1-topic-remote-id-lifecycle.md` section 4 depend on it. Add `pub bound: bool` to the struct.
- Algolia key refetch: `p1-behavior.md` section 19 says "one refetch on 401/403"; T-30a-028 and `p1-rust-design.md` section 7 say 403 only. Pick one and align.
- T-30a-033 says "every test id KT-01..KT-99". The edge-case file defines 81 KT ids with gaps (for example KT-16 to KT-19 and KT-34 to KT-39 are absent). Replace the range with "every KT id defined in `p1-edge-cases-and-tests.md` section 3".
- `p1-behavior.md` section 11.1 says the Algolia page size is not confirmed. Add it to the live checks.

Lint and dependency check: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/30-tracker-kitsu/p1-tasks.md` prints `ok`. Every `area:` token in this area names an existing area directory. The export reports four unknown-area errors in `/home/blackbox/kurobayomi/plans/40-ui-browse-manga/p1-tasks.md` (`area:41-ui-browse-manga-part-2`, lines 37, 83, 158, 537). Those are outside this area and were not changed.

Spot-checks against source (confirmed): `KitsuDateHelper.kt:9-10` (literal `Z`, default zone); `Kitsu.kt:98,107,112` (`DecimalFormat("0.#")`); `KitsuApi.kt:367,388` ("Could not find manga" on both paths); `KitsuListSearch.kt:17,18,51,52` (the anime path reports "Missing Manga data"); `dto/KitsuOAuth.kt:20` (expiry predicate with `- 3600`); `KitsuInterceptor.kt:25` (`refreshToken!!` before the expiry check); `TrackerManager.kt:21` (`KITSU = 3L`); `TrackPreferences.kt:15,20,35` (key names).

---

## 7. Definition of done (observable in Kurobayomi)

1. Settings, Tracking: a Kitsu row shows "Log in" when logged out and "Log out" when logged in. Logging in with an email and password shows "Logged in" and closes the dialog. A wrong password shows "HTTP error 401" inline and keeps both fields marked. The "Login" button is disabled until both fields are non-blank and shows "Loading..." while the request runs.
2. Logout asks "Log out from Kitsu?" with "Cancel" and "Log out". Confirming shows "You are now logged out", makes no network call, and the Kitsu row shows as logged out after a restart.
3. Persistence: the session survives an application restart. The Kitsu password is found nowhere on disk (keyring or database). With no Secret Service running, the token is stored in the settings fallback and the session still survives a restart.
4. Search: a manga search for "one piece" from a logged-in session returns hits with no `novel` subtype. Searching while logged out shows "Not authenticated with Kitsu" and sends no Algolia request.
5. Bind: adding a manga with 5 read chapters to Kitsu creates or updates a library entry with status "current" and progress 5 (verified on kitsu.app in a throwaway account). An existing remote entry's progress, score, status, and dates are copied to the local track, and the private flag stays local.
6. Read event: reading chapter 24 of a 24-chapter title sets status "completed" and a finished date on both the local track and the remote entry. Chapter 1 sets a started date.
7. Refresh: 20 requests issued at the same time with an expired token cause exactly one refresh POST.
8. Score: a 7.5 score is sent as `ratingTwenty` "15", reads back as 7.5, and displays "7.5" under a German locale. A 0 score sends `ratingTwenty` null.
9. Dates: a start date chosen on the device is sent as a UTC wire timestamp. The same operation gives identical wire output with the host zone set to UTC and to Asia/Tokyo.
10. Delete: the remove dialog removes the local track first. With "Also remove from Kitsu" ticked, a DELETE request is sent. A failed remote delete is shown to the user and the local track stays removed.
11. Backup import: tracker id 3 rows appear in the track sheet as bound. Credentials are not imported; the Kitsu row shows as logged out until the user logs in.
12. Every KT test defined in `p1-edge-cases-and-tests.md` section 3 passes or is marked skipped with a reason. The live checks in section 6 are run once against a throwaway account, and their results are recorded as Evidence on T-30a-010, T-30a-016, T-30a-025 and T-30a-027.
13. No dependency on an Android class remains in the Rust port (T-30a-034).
