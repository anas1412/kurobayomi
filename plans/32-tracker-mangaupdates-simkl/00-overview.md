# Area 32: MangaUpdates + Simkl trackers (part 1), overview

Area lead overview. Every path is repo-relative to `/home/blackbox/aniyomi-src` (read-only, Apache-2.0) unless it starts with `plans/` or `crates/`, which are Kurobayomi paths under `/home/blackbox/kurobayomi`. Labels used throughout: **OBSERVED** = what the Kotlin does; **PROPOSAL** = what the Rust port does; **Proposal:** marks a new design choice in this file.

Scope: 22 Kotlin files, 1,111 LOC (MangaUpdates 498, Simkl 613). The full file list is in [p1-inventory.md](p1-inventory.md).

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here

Aniyomi's tracking layer lets a user link a local manga or anime entry to a record on an external service, then keeps status, progress, score and dates in sync. This area covers two of the many services:

- **MangaUpdates** (`MangaUpdates`, id `7`): a manga-only tracker. Remote state is a "list" (Reading, Wish, Complete, Unfinished, On hold) plus a chapter number and a 0.0 to 10.0 rating in steps of 0.1.
- **Simkl** (`Simkl`, id `101`): an anime tracker that covers anime, TV and movies. Remote state is a status (Watching, Completed, On hold, Not interesting, Plan to watch), an episode count, and an integer rating 0 to 10. Login is OAuth through the browser.

Both services are full trackers: they support search, bind (link an entry to a remote record), push progress, pull remote state (refresh), and log in and out. MangaUpdates also supports remote delete. Simkl does not.

### 1.2 Why it matters for a usable Kurobayomi

Trackers are a parity feature (P1 in the project rules), not a P0 blocker. Kurobayomi can read manga and watch anime without them. Users who already track on MangaUpdates or Simkl expect their progress to flow there after each chapter or episode, and expect a bind of an existing series to show their real remote status. A wrong write here destroys remote data the user cares about. Two observed Kotlin defects can do that:

- D-MU-3: a failed rating fetch is treated as score 0.0, and the next update sends DELETE on the user's real rating.
- D-SK-5: Simkl progress is sent as remove then add with no transaction, so a failed add leaves the title with empty server history.

So this area's priority is correctness of remote writes, not feature count.

### 1.3 Explicit non-goals (from the source)

- No MyAnimeList, AniList, Kitsu or Shikimori logic (other areas 28 to 31).
- No Kavita, Komga, Suwayomi or Jellyfin (area 33).
- No Simkl delete (the Kotlin Simkl class does not implement `DeletableAnimeTracker`, see [p1-behavior.md](p1-behavior.md) section 3.16).
- No background sync loop. Background sync is the library update job, which calls the generic tracker flow (area 26).

---

## 2. Module map

### 2.1 Kotlin packages in scope

| Package (repo-relative directory) | Files | Responsibility |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/` | `MangaUpdates.kt` (126 LOC), `MangaUpdatesApi.kt` (197), `MangaUpdatesInterceptor.kt` (31) | Tracker entry point (status and score tables, bind, update, refresh, delete, search, login); HTTP surface to `https://api.mangaupdates.com`; bearer-token interceptor |
| `.../data/track/mangaupdates/dto/` | 10 files: `MUContext.kt`, `MUImage.kt`, `MUListItem.kt`, `MULoginResponse.kt`, `MURating.kt`, `MURecord.kt`, `MUSearch.kt` (2 classes), `MUSeries.kt`, `MUStatus.kt`, `MUUrl.kt` | Wire DTOs and the two mappings into the track model (`MUListItem.copyTo`, `MURecord.toTrackSearch`) |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/` | `Simkl.kt` (150), `SimklApi.kt` (255), `SimklInterceptor.kt` (35), `SimklUtils.kt` (21) | Tracker entry point (status table, bind, update, refresh, login by OAuth code, logout, token persistence); HTTP surface to `https://api.simkl.com`; bearer and API-key interceptor; status-to-wire mapping |
| `.../data/track/simkl/dto/` | 5 files: `SimklOAuth.kt`, `SimklSearch.kt` (2 classes), `SimklSyncItem.kt` (4 classes), `SimklSyncWatched.kt`, `SimklUser.kt` (2 classes) | Wire DTOs and mappings (`SimklSyncItem.toAnimeTrack`, `SimklSearchResult.toTrackSearch`) |

Each DTO file is a `kotlinx.serialization` data class with no logic beyond the mapping functions listed above.

### 2.2 Kotlin code outside scope that the behaviour depends on

These are read for exactness, not ported here. They are listed in [p1-inventory.md](p1-inventory.md) under "External files":

- Shared contract: `app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt`, `Tracker.kt`, `MangaTracker.kt`, `AnimeTracker.kt`, `DeletableMangaTracker.kt`, `DeletableAnimeTracker.kt`, `TrackerManager.kt`.
- Preferences: `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`.
- Models: `app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt`, `.../anime/AnimeTrack.kt`, domain models under `app/src/main/java/eu/kanade/domain/track/`.
- Login UI: `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt`.
- Manifest: `app/src/main/AndroidManifest.xml` (intent filter for scheme `aniyomi`, host `simkl-auth`).
- Network helpers: `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt`, `core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt`, `core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt`.

### 2.3 Proposed Rust module map

Crate `kuro-track` owns the trackers. Layout is from [p1-rust-design.md](p1-rust-design.md) section 1, with the shared traits from area 27.

```
crates/kuro-track/src/trackers/
  mod.rs                     registry constructor all_trackers(ctx)
  mangaupdates/
    mod.rs                   MangaUpdatesTracker (Tracker, MangaTracker, DeletableMangaTracker)
    api.rs                   MuApi: endpoints M1..M9
    auth.rs                  MuAuth: token holder and request headers (replaces MangaUpdatesInterceptor)
    dto.rs                   wire structs
    search.rs                MuRecord -> MangaTrackSearch
    scores.rs                92-entry score table and index mapping
    status.rs                list ids, labels, status order
  simkl/
    mod.rs                   SimklTracker (Tracker, AnimeTracker)
    api.rs                   SimklApi: endpoints S2..S11
    auth.rs                  SimklAuth: OAuth token holder and headers (replaces SimklInterceptor)
    oauth.rs                 authorise URL, redirect parsing, code exchange glue
    dto.rs                   wire structs
    status.rs                status ids, labels, wire strings
    sync.rs                  find, add, update, progress, rating sequences
    search.rs                SimklSearchResult -> AnimeTrackSearch; tracking_url normalisation
    scores.rs                integer 0..10 score list
crates/kuro-track/src/creds.rs       CredentialStore (secret store plus prefs)
crates/kuro-track/src/http.rs        read_json helper (awaitSuccess and parseAs equivalent)
crates/kuro-track/src/error.rs       TrackError (shared variants, see rust design section 9)
```

GUI glue (crate root `src/`, per area 27):

```
src/track/ui/login_mangaupdates.rs   credential dialog (MU)
src/track/ui/login_simkl.rs          browser step and paste-code fallback (SK)
```

Mapping of the two services' Kotlin files to Rust modules is in the inventory table (column "Target Rust module").

---

## 3. Reading guide

| Document | What it holds |
|---|---|
| [p1-inventory.md](p1-inventory.md) | The 22 in-scope files with LOC, declarations, port decision and task IDs per file. Also the list of external files the behaviour depends on. |
| [p1-behavior.md](p1-behavior.md) | The behaviour spec. Sections 1 (shared contract), 2 (MangaUpdates), 3 (Simkl), 4 (message keys and literal strings), 5 (side effects), 6 (concurrency and timings), 7 (cross-area), 8 (defects D-MU-1..10, D-SK-1..11 with proposals), 9 (dead code), 10 (Android-to-Linux notes that change behaviour). |
| [p1-data-and-config.md](p1-data-and-config.md) | Data shapes: track models, preference keys, enums and value tables, all HTTP endpoints with request and response JSON, database tables touched (`manga_sync`, `anime_sync`), files on disk, JSON rules, Rust type sketches, value domains, and secrets. |
| [p1-rust-design.md](p1-rust-design.md) | The Rust design: layout, crates, identity and models, the MangaUpdates and Simkl designs (types and algorithms), OAuth and deep link on Linux, secrets, async model, error handling, mockup integration, Android-to-Linux mapping, pitfalls, and open decisions O-1 to O-8. |
| [p1-ui-parity.md](p1-ui-parity.md) | UI parity for the Settings > Tracking rows, MU login dialog, Simkl browser login and paste fallback, logout dialog, tracker search, search item, status and score pickers, remove dialog, states, keyboard and mouse, responsive behaviour, and the gap list against the mockup. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases (E-MU-*, E-SK-*, E-X-*), defect regression tests, the unit, HTTP and live test list (TST-1 to TST-78), and fixtures to record. |
| [p1-tasks.md](p1-tasks.md) | 55 implementable tasks (T-32a-001 to T-32a-072, see section 6). |

---

## 4. Key design decisions and open questions

### 4.1 Decisions already made in the docs (kept)

1. **Keep the wire contract byte-compatible with Aniyomi.** Request bodies keep key order (`serde_json` with `preserve_order`). Storage keys keep their meaning (uid as username, session token as password for MU; user id as username, access token as password for SK). Source: [p1-rust-design.md](p1-rust-design.md) section 2 and [p1-behavior.md](p1-behavior.md) section 1.2.
2. **Tracker ids are stable numbers**: `MangaUpdates = 7`, `Simkl = 101`. They appear in `sync_id` columns and must not change. Source: [p1-behavior.md](p1-behavior.md) 1.1.
3. **Tokens go to the secret store, usernames stay in prefs.** MU session token and SK access token move to Secret Service through `keyring`. Usernames stay in the preference store. Old values are migrated once and deleted. Source: [p1-rust-design.md](p1-rust-design.md) section 7, task T-32a-065.
4. **Fix the observed defects, do not reproduce them.** Each defect in [p1-behavior.md](p1-behavior.md) section 8 has a proposal. Fixes are required by the edge-case and regression list in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 5.
5. **Async, no blocking.** `runBlocking` (SK `getCurrentUser`) becomes an `async fn` (fix D-SK-9). Results reach the GTK loop over `async_channel` (area 27).
6. **Simkl client secret stays a compile-time constant.** The secret ships in the binary today. For a desktop client it is public-equivalent. Source: [p1-behavior.md](p1-behavior.md) section 10.

### 4.2 Open questions that need the user's call or a live check

Each has the recommendation the design already records. Items marked "live" need one run against the real service with a throwaway account; the docs could not verify them offline.

| ID | Question | Recommendation | Why it matters | Needs |
|---|---|---|---|---|
| O-1 | MangaUpdates "not on list": HTTP 404 on `GET /v1/lists/series/{id}` (assumed), or 200 with an empty body? | Treat 404 as "not on list" and any other error as an error (fix D-MU-2). Confirm live with TST-74. | A wrong branch causes either a duplicate add or a lost bind. | live |
| O-2 | `TrackerTrait::index_to_score` return type: keep `f64` with a clamp, or `Result<f64, TrackError>`? | `Result<f64, TrackError>` (preferred), so an out-of-range index is an error and never a panic (fix D-MU-7). Affects area 27. | Shared trait signature. | user (signature change in area 27) |
| O-3 | Where is the MU "remote rating known" flag stored, so a 0.0 score sends DELETE only when a rating exists? | Store a tri-state (`Unknown`, `None`, `Some`) in `RatingFetch` for the session, and persist the last known state in `manga_sync` if it must survive restarts. Decide the column name with area 03. | Avoids deleting real ratings (D-MU-3). A schema column is a small change, so it needs sign-off. | user |
| O-4 | Stored movie path for Simkl: `/movies/<id>` (proposed) or the Kotlin's `/movie/<id>` form (never actually stored)? | `/movies/<id>`, because the Kotlin's stored form for movies found through the movies list is `/movies/<id>`. Confirm with one live search and one live bind (TST-76). | Determines whether progress and status go to the movies or shows list later. | live |
| O-5 | Simkl progress order: keep remove then add (Kotlin), or add then trim? | Keep the Kotlin order for now, as the default. Test the history endpoint semantics live first (TST-77). Add a retry of the add once, and surface the error if it still fails, as the fallback in [p1-behavior.md](p1-behavior.md) D-SK-5. | Data loss on partial failure (D-SK-5, H severity). | live, then user |
| O-6 | Simkl redirect: keep `aniyomi://simkl-auth` (needs a desktop scheme handler), or ask Simkl for a loopback redirect? | Keep the custom scheme. Register `x-scheme-handler/aniyomi` in the `.desktop` file (area 01), and always offer the paste fallback because the handler may be absent when running from a build directory. | Whether browser login works without a manual paste step. | user (Simkl app registration is outside this repo) |
| O-7 | Simkl client secret: keep in the binary, or move to a build-time env var? | Keep it (matches source). It is public-equivalent for a desktop client. | Build config only. | user, low stakes |
| O-8 | Live response shapes for Simkl `/sync/all-items`, `/search/*` and MU `/v1/*` were not re-verified offline. | DTOs follow the Kotlin exactly. Record real fixtures on the first live run (TST-74..TST-78) and re-run the fixture tests. | If a key differs, parsing fails on the first real call. | live |

### 4.3 Design issues found while writing this overview (not in the source docs)

These are gaps in the existing docs. None is a design change, but each needs a fix before implementation:

1. **Defect IDs are not cited consistently in the task file.** Only seven of the 21 defects appear by ID in [p1-tasks.md](p1-tasks.md): D-MU-1 (T-32a-014), D-MU-2 (T-32a-015), D-MU-4 (T-32a-016), D-MU-5 (T-32a-021), D-MU-9 (T-32a-024), D-SK-1 (T-32a-039), D-SK-4 (T-32a-046). D-MU-3 (fixed by T-32a-018) and D-SK-2 (fixed by T-32a-038) are covered by title only. The other twelve are not cited by ID in any task: D-MU-6, D-MU-7, D-MU-8, D-MU-10, D-SK-3, D-SK-5, D-SK-6, D-SK-7, D-SK-8, D-SK-9, D-SK-10, D-SK-11. T-32a-042 has the D-SK-5 title ("history remove then add") but no acceptance line for the fix. Each defect needs an acceptance line that names its ID, so the regression tests in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 5 trace to a task. Not changed here, because the task IDs and scope are the authors' call.
2. **Dependency cycles were found and removed.** The task file had three dependency problems that were fixed while linting (task IDs unchanged):
   - T-32a-017 depended on T-32a-022, and T-32a-022 depended on T-32a-017. The removed edge was 017 on 022, because 022 is the tracker-level rule built on the 017 API call.
   - T-32a-024 and T-32a-068 depended on each other. The removed edge was 024 on 068, because 068 refines the dialog that 024 creates.
   - T-32a-033 depended on T-32a-036 (search). Login completion does not need search. The edge was removed.
3. **Area 40 has four unknown `area:` tokens**, outside this area (`area:41-ui-browse-manga-part-2` at `plans/40-ui-browse-manga/p1-tasks.md` lines 37, 83, 158 and 537). Not changed here, because that file belongs to area 40. The export step reports them on every run (`errors=4`). This area's own tokens (`area:02-preferences-system`, `area:05-data-repos-entries-items`, `area:12-network-stack`, `area:27-tracking-core`) are all real area folder ids.
4. **Lint prints no count.** The inventory says the lint step prints the task count. `tasks.py lint` prints `ok` only. The count in section 6 below comes from `TASKS.csv`.

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | Needed | Used by |
|---|---|---|
| area:27-tracking-core | `Tracker`, `MangaTracker`, `AnimeTracker`, `DeletableMangaTracker` traits; `TrackerId`; `MangaTrack`, `AnimeTrack`, `MangaTrackSearch`, `AnimeTrackSearch`; `ScoreList`; `LogoRef`; the generic setter flow (`setRemoteMangaStatus`, `setRemoteLastChapterRead`, `setRemoteScore`); `SecretStore` (`kuro-platform`); login-state watch channel; `TrackError` base; tracking settings rows | T-32a-001, 002, 024, 064, 071, 072 and most others |
| area:02-preferences-system | Preference store for usernames, `track_login_name_7` (new), and the one-time migration of `pref_mangasync_*` and `track_token_101` | T-32a-002, 065 |
| area:05-data-repos-entries-items | Track row insert and update (`InsertMangaTrack`, `InsertAnimeTrack`), and the `coalesce(:libraryId, library_id)` update rule | T-32a-004 |
| area:12-network-stack | Shared HTTP client, timeouts (connect 10 s, total 30 s proposal), JSON helpers, `awaitSuccess` equivalent | T-32a-011, 060 |
| area:13-core-util-storage-archive-torrent | HTML decoding (`htmlDecode` replacement, `html-escape` and `ammonia`) | T-32a-023 |
| area:01-foundation-app-bootstrap | `.desktop` file with `MimeType=x-scheme-handler/aniyomi;`, so the browser can hand the OAuth redirect back to the app | T-32a-031 |
| area:53-platform-manifest-intents-workers | Linux replacement for the Android intent filter (`TrackLoginActivity` routing). No task in this area names this area as a dependency; the replacement is recorded in T-32a-031 and T-32a-062. | T-32a-031, T-32a-062 (no dependency token) |
| area:34-images-covers-cache | Poster and cover download and cache for search results | T-32a-067 (inference: this task needs the images cache; the docs do not name the area) |
| area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations | No schema change for these two services, except possibly one column for O-3 (`manga_sync`) | O-3 |

### 5.2 What others need from this area

| Area | Needs from area 32 | Where |
|---|---|---|
| area:26-library-update-jobs | Background progress push goes through the generic tracker flow, which calls `update` on `MangaUpdatesTracker` and `SimklTracker` | [p1-behavior.md](p1-behavior.md) section 5.3 |
| area:21-backup-format-and-create, area:22-backup-restore | Backups store track rows (`tracking_url`, `remote_id`, `status`, `score`, `last_episode_seen` and similar). Restore does not re-login and does not call the services. Tokens are not backed up (to confirm in those areas). | [p1-behavior.md](p1-behavior.md) section 7 |
| area:27-tracking-core | The two concrete trackers implement the shared traits; the registry `all_trackers(ctx)` lists them | [p1-rust-design.md](p1-rust-design.md) section 1 |
| UI areas (tracking sheet and search screen, in the entry-details areas 38 and 39, to confirm) | Status list order (MU `[0, 2, 4, 3, 1]`, SK `[1, 2, 3, 4, 5]`), score list (MU 92 entries, SK 0 to 10), the search and bind actions, and the "Track privately" button hidden for these two services | [p1-ui-parity.md](p1-ui-parity.md) sections 5, 7, 12 |

---

## 6. Task summary

Source: `TASKS.csv`, rows whose file starts with `32-tracker-mangaupdates-simkl/` (55 rows, all in [p1-tasks.md](p1-tasks.md)).

### 6.1 Counts

| Priority | Count | Effort | Count |
|---|---|---|---|
| P0 | 0 | S | 39 |
| P1 | 50 | M | 16 |
| P2 | 4 (T-32a-025, 047, 051, 067) | L | 0 |
| P3 | 1 (T-32a-062) | XL | 0 |
| **Total** | **55** | **Total** | **55** |

There are no P0 tasks in this area. Trackers are parity (P1), so Kurobayomi can be used daily without them.

### 6.2 P0 critical path

None, because the area has no P0 tasks.

### 6.3 P1 critical path (longest dependency chain inside the area, prerequisite first)

Eight tasks:

1. T-32a-001 (tracker id constants and module skeleton; needs area 27)
2. T-32a-002 (shared credential and login-state behaviour; needs T-32a-001 and area 27)
3. T-32a-034 (Simkl token persistence; needs T-32a-002 and T-32a-065, the secret store migration that needs area 02)
4. T-32a-035 (Simkl request headers and missing-token behaviour)
5. T-32a-039 (Simkl find in list, with the TV fix D-SK-1)
6. T-32a-044 (Simkl bind)
7. T-32a-061 (unit and fixture test suite for both trackers)
8. T-32a-069 (live verification tests and fixture recording)

The MangaUpdates chain is shorter: T-32a-001, 011, 016, 015, 020, 019, 070. The GUI tasks (T-32a-064 and T-32a-071) depend on the Simkl chain through T-32a-048 and T-32a-036.

### 6.4 Task list by priority (IDs from `TASKS.csv`)

- P1 (50): T-32a-001, 002, 003, 004, 010, 011, 012, 013, 014, 015, 016, 017, 018, 019, 020, 021, 022, 023, 024, 026, 030, 031, 032, 033, 034, 035, 036, 037, 038, 039, 040, 041, 042, 043, 044, 045, 046, 048, 049, 050, 060, 061, 064, 065, 066, 068, 069, 070, 071, 072.
- P2 (4): T-32a-025 (MangaUpdates logo), 047 (Simkl score display and labels), 051 (Simkl logo), 067 (poster and cover loading).
- P3 (1): T-32a-062 (Android-only plumbing: replacement decisions).

Groups in the task file: A shared (001 to 004), B MangaUpdates (010 to 026), C Simkl (030 to 049), D cross-cutting (050 to 072).

### 6.5 Risks

1. **Silent loss of remote data.** D-MU-3 and D-SK-5 can delete a rating or empty a history. Mitigation: tasks T-32a-018 and T-32a-042, with live test TST-77 before release. Open decision O-5 must be resolved before T-32a-042 ships.
2. **Wrong "not on list" branch.** O-1 is unverified offline. Mitigation: live test TST-74 and a regression test that an error from M1 never adds a series.
3. **Unverified response shapes.** O-8. All DTOs follow the Kotlin, but the real JSON was not seen. Mitigation: fixture recording in T-32a-069.
4. **OAuth on Linux depends on a desktop handler or a paste step.** O-6. Without the handler, the user must paste the redirect URL. This is a worse experience but not a data risk.
5. **Secret Service may be absent (KDE without KWallet bridge, minimal WMs).** Mitigation: fallback to prefs with a visible warning (E-X-5, TST-64). The warning must not be silent.
6. **Shared trait change (O-2).** Changing `index_to_score` affects area 27 and possibly other trackers. Decide before T-32a-003 is written.
7. **Cancellation between two writes.** A window close can leave Simkl history removed without re-adding (E-X-2). Mitigation: spawn the pair on the runtime (TST-61).
8. **Simkl client secret in the binary.** Low risk for a desktop client, but it is in every build. Accepted (O-7).

---

## 7. Definition of done for this area

Each item is an observable behaviour in the running Kurobayomi app. Items marked "live" also need a run against the real service.

1. **Settings > Tracking shows two rows**, MangaUpdates and Simkl, each with its logo, its login state ("Logged in" or "Not logged in"), and a login or logout action.
2. **MangaUpdates login** with a valid username and password closes the dialog and shows the toast "login success". The stored secret is the session token. The username shown in the dialog is the one the user last typed, not the numeric uid. A wrong password keeps the dialog open with an error text, and never shows the literal `null`. (live)
3. **Simkl login** opens the system browser at the authorise URL `https://simkl.com/oauth/authorize?response_type=code&client_id=...&redirect_uri=aniyomi%3A%2F%2Fsimkl-auth`. After the redirect, the row shows logged in. A paste of the redirect URL or the bare code also logs in. An empty or invalid paste leaves the row logged out and shows no crash. (live)
4. **Logout** from either row asks for confirmation, then clears the in-memory token immediately (no authenticated request succeeds after logout in the same session, fixing D-MU-1) and deletes the stored secret. The Simkl token key is deleted, not rewritten with `null`.
5. **Tracker search** for manga returns MangaUpdates results with the title, the cover, a type that is empty (not `null`) when unknown, and an open-in-browser action. For anime, Simkl returns results from three searches (anime, tv, movie) concatenated, and one failing request fails the whole search with an error shown in the UI.
6. **Bind a manga to MangaUpdates** with no existing list entry adds it to Wish (no read chapters) or Reading (with read chapters). Bind when the entry exists copies the remote status, chapter and rating into the local track, and never adds a duplicate. A failed lookup with a non-404 error shows an error and sends no add request (fix D-MU-2). (live)
7. **Progress update to MangaUpdates** after reading chapter 12.9 sends `status.chapter = 12`. A completed series keeps status Complete. Score -1 sends no rating request. A failed rating fetch never deletes the remote rating (fix D-MU-3). (live)
8. **Bind an anime to Simkl** with no existing list entry sets status Watching (if episodes are seen locally) or Plan to watch, adds it to the Simkl list (movies to the movies list, shows to the shows list), and sends no progress. A TV show already on the list binds without error (fix D-SK-1). (live)
9. **Progress update to Simkl** for a show sends history remove then history add with episodes 1..N, then add-to-list, then rating. For a movie it sends add-to-list and rating only. If the add step fails, the app retries once and then shows the error (fix D-SK-5, O-5). (live)
10. **Refresh** of a MangaUpdates or Simkl track updates status, progress, score and total episodes from the remote, keeps the title and URL, and shows an error without changing the local track if the list lookup fails.
11. **Remove tracking** from the remove dialog for MangaUpdates removes the local row, and also deletes the series from the MangaUpdates list when "Also remove from my MangaUpdates list" is ticked (default unchecked). For Simkl there is no checkbox, and the dialog states "This only removes the link in Kurobayomi. Your Simkl list is not changed." ([p1-ui-parity.md](p1-ui-parity.md) section 8).
12. **Score pickers** show MangaUpdates 92 entries (`-`, `1.0` to `10.0`) and Simkl 0 to 10. A MangaUpdates track with score 0.0 shows `-` (fix D-MU-8).
13. **No secret is written to a log or a toast.** A search for the session token, the access token and the OAuth code in the log output finds nothing (TST-58). Logged HTTP requests show method and path only.
14. **Old Aniyomi values migrate once.** After importing an Aniyomi preference file, the first start moves `pref_mangasync_password_7`, `pref_mangasync_password_101` and `track_token_101` (with value `null` treated as absent) into the secret store, deletes the old keys, and a second start does nothing (TST-65).
15. **If no Secret Service is running**, the tracking row shows "Tokens stored unencrypted" and the login still works (E-X-5, TST-64).
16. **Every task in [p1-tasks.md](p1-tasks.md) passes its acceptance criteria**, and `cargo test -p kuro-track` passes with the non-ignored set (TST-1 to TST-73 as applicable). The ignored live set is run once before release and its fixtures are recorded (TST-74 to TST-78).
