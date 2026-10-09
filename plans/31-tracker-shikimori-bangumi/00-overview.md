# area:31-tracker-shikimori-bangumi: Shikimori and Bangumi trackers, overview

Repo root for every source path in this area: `/home/blackbox/aniyomi-src` (read-only, Apache-2.0). All paths below are repo-relative. Labels: **OBSERVED** = what the Kotlin does (with `path:line`), **Proposal:** = what Kurobayomi should do.

Area lead summary: the two trackers are 17 Kotlin files, 1,501 LOC (Bangumi 754, Shikimori 747). They are plain tracker implementations with no UI of their own. The work is a faithful Rust port of their wire behaviour, token handling and status mapping, plus a fixed set of observed defects (D1 to D22 in [p1-behavior.md](p1-behavior.md) section 8). The area has **no P0 tasks**: a tracker is not needed to read manga or watch anime, so both trackers are P1 parity with two P2 improvements and one P3 drop list. Shared tracking framework (`Tracker`, registry, login UI, `TrackError`, secret store) belongs to [area:27-tracking-core](../27-tracking-core/00-overview.md) and is consumed here, not re-planned.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here

**OBSERVED.** Aniyomi lets a user link a manga or anime entry to an entry on an external tracking service and keep status, score and progress in sync. Shikimori (`https://shikimori.one`, registry id 4) and Bangumi (`https://bgm.tv` and `https://api.bgm.tv`, registry id 5) are two of eleven trackers in `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:39-42`. Each tracker implements `MangaTracker` and `AnimeTracker`. Shikimori also implements `DeletableMangaTracker` and `DeletableAnimeTracker`; Bangumi does not implement delete.

Operations per tracker (from [p1-behavior.md](p1-behavior.md) sections 2 and 3):
- OAuth login by authorisation code (the only login path for both, `ShikimoriApi.authUrl()` and `BangumiApi.authUrl()`).
- Catalogue search for manga and anime.
- Bind: find or create the remote entry, copy personal fields onto the local track.
- Add, update (progress, status, score), refresh (pull remote state), delete (Shikimori only).
- Status list, status labels, score list.
- Logout (deletes stored credentials).

### 1.2 Why it matters for a usable Kurobayomi

Trackers are P1 parity: most Aniyomi users who track at all use one of the big services, and Shikimori and Bangumi are the two that matter for Japanese and Chinese catalogues. They are not needed for daily reading or watching, so they do not block P0. Their correctness matters for two reasons:

1. The tracker ids 4 and 5 and the status numbers are persisted in `manga_sync` / `anime_sync` rows and in backups. Changing them breaks restores. Proposal: keep them forever (see section 5).
2. Several Kotlin defects can silently corrupt data, for example D17 (id spaces mixed), D22 (remote entry wipes local dates), D8 (unclamped score diverges). The Rust port must not copy them.

### 1.3 Scope boundaries

In scope: the 17 files listed in [p1-inventory.md](p1-inventory.md), the Rust `kuro-track` modules `shikimori/` and `bangumi/`, and the tests for them.

Out of scope (owned elsewhere, consumed here):
- Tracker framework: `BaseTracker`, `MangaTracker`, `AnimeTracker`, `Deletable*`, `TrackerManager`, login activities, `TrackPreferences`, `AddMangaTracks` / `AddAnimeTracks` interactors, remove-tracking dialogs, `ExceptionFormatter`. Owner: [area:27-tracking-core](../27-tracking-core/00-overview.md).
- HTTP client, `awaitSuccess`, `HttpException`, default cache control. Owner: [area:12-network-stack](../12-network-stack/00-overview.md).
- Track table schema (`manga_sync` / `anime_sync`). Owners: [area:03-db-manga-schema-migrations](../03-db-manga-schema-migrations/00-overview.md) and [area:04-db-anime-schema-migrations](../04-db-anime-schema-migrations/00-overview.md).
- Backup encoding of track messages. Owners: [area:21-backup-format-and-create](../21-backup-format-and-create/00-overview.md) and [area:22-backup-restore](../22-backup-restore/00-overview.md).

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities (this area)

| Package (repo-relative directory) | Files | Responsibility |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/` | `Shikimori.kt` (251), `ShikimoriApi.kt` (277), `ShikimoriInterceptor.kt` (49), `ShikimoriUtils.kt` (34) | Tracker class (status, score, bind, update, refresh, delete, login, logout); HTTP calls to `shikimori.one/api`; Bearer and User-Agent header and refresh; status mapping |
| `.../data/track/shikimori/dto/` | `SMAddEntryResponse.kt` (8), `SMEntry.kt` (57), `SMOAuth.kt` (21), `SMUser.kt` (8), `SMUserListEntry.kt` (42) | Wire DTOs for user_rates, catalogue entries, OAuth token, whoami |
| `.../data/track/bangumi/` | `Bangumi.kt` (233), `BangumiApi.kt` (297), `BangumiInterceptor.kt` (63), `BangumiUtils.kt` (22) | Tracker class; HTTP calls to `api.bgm.tv` and `bgm.tv`; Bearer and User-Agent header and refresh; collection type mapping |
| `.../data/track/bangumi/dto/` | `BGMCollectionResponse.kt` (33), `BGMOAuth.kt` (25), `BGMSearch.kt` (72), `BGMUser.kt` (9) | Wire DTOs for collection, OAuth token, search, user |

Top-level declarations: 27 (6 classes, 6 extension functions, 1 function, 14 data classes), cross-checked in [p1-inventory.md](p1-inventory.md).

### 2.2 Proposed Rust module map

Crate `kuro-track`, inside `crates/kuro-track/src/` (layout in [p1-rust-design.md](p1-rust-design.md) section 1):

```
crates/kuro-track/src/
  shikimori/
    mod.rs     ShikimoriTracker: Tracker, MangaTracker, AnimeTracker, Deletable*   (Shikimori.kt)
    api.rs     ShikimoriApi: every request in p1-data-and-config.md 7.1            (ShikimoriApi.kt)
    auth.rs    ShikimoriSession: bearer, single-flight refresh, persistence         (ShikimoriInterceptor.kt)
    status.rs  ShikimoriStatus enum, API strings, labels                           (ShikimoriUtils.kt)
    dto.rs     serde structs for S1..S11                                           (dto/*.kt)
    search.rs  SMEntry -> search result mapping                                    (dto/SMEntry.kt)
    tests/     fixtures and wiremock tests
  bangumi/
    mod.rs     BangumiTracker: Tracker, MangaTracker, AnimeTracker                 (Bangumi.kt)
    api.rs     BangumiApi: every request in p1-data-and-config.md 7.2              (BangumiApi.kt)
    auth.rs    BangumiSession: bearer, single-flight refresh, persistence          (BangumiInterceptor.kt)
    status.rs  BangumiStatus enum, collection types, labels                        (BangumiUtils.kt)
    dto.rs     serde structs for B1..B7                                            (dto/*.kt)
    search.rs  BGMSubject -> search result mapping, summary composition            (dto/BGMSearch.kt)
    tests/
  oauth.rs     shared authorise URL builder, redirect parsing (area 27)
  error.rs     shared TrackError (area 27 owns the enum; variants added by this area)
```

Rules (**Proposal:**):
- `kuro-track` does not depend on GTK, `gio` or `keyring`. Both trackers receive `Arc<dyn SecretStore>` and an HTTP client by constructor injection (replaces injekt, see [p1-rust-design.md](p1-rust-design.md) section 9).
- Two separate status enums with explicit `i64` discriminants. The numbers collide: `1` is `READING` for Shikimori and `PLAN_TO_READ` for Bangumi ([p1-rust-design.md](p1-rust-design.md) section 12, item 2).
- Interceptor logic becomes explicit function calls (`session.bearer()`), so ordering is visible and D16 is fixed by construction.

Crate choices: `reqwest` 0.12 (`json`, `rustls-tls`, `gzip`), `serde` and `serde_json` (with `preserve_order`), `tokio` (`sync`, `time`), `async-trait`, `thiserror`, `url`, `time`, `tracing`, `wiremock` (dev). Not used: `once_cell`, `chrono`, `async-std`.

---

## 3. Reading guide

Read in this order: inventory, behaviour, data, Rust design, tasks, edge cases. The two topic documents are reference material for the auth and remote-API facts.

| Document | What it contains |
|---|---|
| [p1-inventory.md](p1-inventory.md) | One row per in-scope file: LOC, declared names, port decision (Rewrite, Port, Replace-with-crate, Drop), target Rust module, priority, task ids. Lists the out-of-scope files this area depends on. |
| [p1-behavior.md](p1-behavior.md) | Behaviour spec for both trackers: endpoints, status mapping, score, login, token persistence, interceptor, logout, search, bind, write path, delete, refresh, status labels, timings and limits. Section 8 holds defects D1 to D22 and the D-OPEN decisions; section 9 holds implementer pitfalls. |
| [p1-data-and-config.md](p1-data-and-config.md) | Wire DTO shapes for both services, the in-app track models, status and score tables, preference keys, SQL tables touched, backup encoding (summary), network endpoints with request bodies, OAuth parameters, logo assets, and the Linux storage mapping. |
| [p1-rust-design.md](p1-rust-design.md) | Rust layout, types and trait impls, copy rules, DTOs and leniency rules, auth and secrets, HTTP request builders, assets, async model, Android-to-Linux mapping, error model, mockup integration, pitfalls. |
| [p1-tasks.md](p1-tasks.md) | The 49 implementation tasks `T-31a-001` to `T-31a-049`, each with priority, effort, dependencies, sources and acceptance criteria. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases per tracker and shared, failure modes, race conditions, the test plan (`TS-*` ids), fixtures and open questions OQ-1 to OQ-8. |
| [p1-topic-remote-api-verification.md](p1-topic-remote-api-verification.md) | Every remote-server fact the plan depends on, with its source (`Verified (source)`, `Verified (doc)`, `Not verified`). Section 5 lists what needs a live account. Section 6 (live results) is empty: nothing has been verified live yet. |
| [p1-topic-token-lifecycle.md](p1-topic-token-lifecycle.md) | OAuth token states, timelines (Shikimori 86400 s, refresh threshold 82800 s; Bangumi 604800 s, threshold 601200 s), refresh under concurrency, failure table, persistence and restore, logout, security notes, test hooks. |

---

## 4. Key design decisions and open questions

Decisions that need the user (from [p1-behavior.md](p1-behavior.md) section 8.1 and [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 8):

| ID | Question | Recommendation |
|---|---|---|
| D-OPEN-1 | Whose OAuth client is used. The Shikimori and Bangumi client id and secret are in the Aniyomi source (`ShikimoriApi.kt:258-259`, `BangumiApi.kt:268-269`). A desktop app cannot hide a client secret that the token exchange needs. | (b): register Kurobayomi's own applications at `https://shikimori.one/oauth/applications` and `https://bgm.tv/dev/app`, and read `client_id` and `client_secret` from the user's config file. Until then, rows are disabled (OQ-8). The Aniyomi constants are not copied. This area cannot register applications. |
| D-OPEN-2 | Secret-store names. | Accept area:27's names: service `kurobayomi.track`, accounts `<id>.token`, `<id>.password`, `<id>.username`. Already in use across this area. |
| D-OPEN-3 | Bangumi anime progress: keep `ep_status` (parity, D7) or use the episode endpoint (spec-correct). | Parity first (T-31a-030 writes `ep_status`); episode endpoint as P2 task T-31a-044. Show Bangumi anime progress as "unverified" until T-31a-044 lands. |
| OQ-1 | A search element with a missing `chapters` fails the whole Kotlin search. Skip the element or fail? | Skip the element with a warning, so one broken entry does not hide the rest. |
| OQ-2 | Delete returns 404: already deleted, or error? | Success. The goal of delete is that the entry is gone. |
| OQ-3 | Ship the copied Shikimori and Bangumi logos (`ic_tracker_*.webp`)? | Ask the providers, or use neutral initial tiles like the mockup. T-31a-047 (P2). |
| OQ-4 | Refresh loop guard when `expires_in` is 0 or the clock is wrong. | Yes: refresh at most once per 60 seconds. The Kotlin has no guard. |
| OQ-5 | Anonymous search with the public catalogue. | Yes, the endpoints are public and carry no user data. Implemented as P2 (T-31a-045, D9). |
| OQ-6 | Does Shikimori revoke the old refresh token on refresh? | Assume yes and persist the whole new response. Verify live in T-31a-046. |
| OQ-7 | Bangumi refresh response may omit `user_id`. | Already handled as `Option` (EC-B-12). Verify live in T-31a-046. |
| OQ-8 | No OAuth client configured: hide tracker rows or show them disabled? | Show them disabled with an explanation. |

Design decisions that are not open (**Proposal:**, recorded in the docs):
- Tokens go to the secret store, never to a preference or database row. The Kotlin also writes the access token to the password preference (D11) and writes the string `"null"` on logout (D10). Rust deletes the entry instead.
- One refresh at a time per tracker, through a `tokio::sync::Mutex` with a re-read after the lock (D2, [p1-topic-token-lifecycle.md](p1-topic-token-lifecycle.md) section 3).
- Refresh failure with 400 or 401 raises `TrackError::AuthExpired` and sets `track.N.auth_expired` (D1). The Kotlin ignores it.
- Clamp score to `0..=10` on write, negatives written as `0`, for both trackers (D8). The Kotlin clamps only on Bangumi.
- Remote `remote_id` is the catalogue id and `library_id` is the user-rate id, on every path (D17).
- Remote dates are not copied into local tracks (D22). Start date is restored from history only when the local value is `0`, as bind does now.
- Shikimori's `private` is never sent (never supported). Bangumi's remote `private` is never read back (D14); the local value is the source of truth.
- Login errors are returned to the caller and shown as a toast. The Kotlin swallows them (`catch (e: Throwable) { logout() }`). This is a deliberate change ([p1-rust-design.md](p1-rust-design.md) section 12, item 12).
- Keep tracker ids 4 and 5 and the stored numeric statuses forever. Backups and `sync_id` depend on them.

---

## 5. Cross-area dependencies

### 5.1 This area needs

| From | What | Where it is used |
|---|---|---|
| [area:27-tracking-core](../27-tracking-core/00-overview.md) | `Tracker` / `MangaTracker` / `AnimeTracker` / `Deletable*` traits; `TrackerManager` registry (`TrackerManager.kt:39-42`); `TrackError` enum; `SecretStore` (service `kurobayomi.track`); `track.N.auth_expired` flag; OAuth redirect scheme and login UI; `MsgKey`; `ScoreList` (`SCORE_LIST_0_10`); `Opener` trait for browser launch; `Clock` seam; bind and sync interactors | Almost every task. Task rows marked `area:27-tracking-core` in [p1-tasks.md](p1-tasks.md). |
| [area:12-network-stack](../12-network-stack/00-overview.md) | Shared `reqwest` client (`kuro-net`), cache policy, `awaitSuccess`-equivalent status handling, `HttpException` text | T-31a-040 (headers and cache policy), T-31a-008, T-31a-027 |
| [area:03-db-manga-schema-migrations](../03-db-manga-schema-migrations/00-overview.md) and [area:04-db-anime-schema-migrations](../04-db-anime-schema-migrations/00-overview.md) | `manga_sync` and `anime_sync` table shape (`sync_id` = 4 or 5) | Stored track row written by `updateRemote` |
| [area:05-data-repos-entries-items](../05-data-repos-entries-items/00-overview.md) and [area:06-data-repos-history-track-source-misc](../06-data-repos-history-track-source-misc/00-overview.md) | Track repository (`rusqlite`) | Persisting track rows, reading history for start-date restore |
| [area:21-backup-format-and-create](../21-backup-format-and-create/00-overview.md) and [area:22-backup-restore](../22-backup-restore/00-overview.md) | Backup protobuf track fields, tracker id and status number preservation | T-31a-048 |
| [area:51-i18n](../51-i18n/00-overview.md) | Localised labels for statuses and error details | T-31a-018, T-31a-036, T-31a-041 |

### 5.2 Other areas need from this one

- [area:27-tracking-core](../27-tracking-core/00-overview.md) consumes both trackers through the registry rows and the login UI (T-31a-037).
- Backup and restore ([area:21-backup-format-and-create](../21-backup-format-and-create/00-overview.md), [area:22-backup-restore](../22-backup-restore/00-overview.md)) need tracker ids 4 and 5 and the status numbers, which this area keeps unchanged (T-31a-048).
- Entry details and track dialogs (UI areas, for example [area:38-ui-entry-details-manga](../38-ui-entry-details-manga/00-overview.md) and [area:39-ui-entry-details-anime](../39-ui-entry-details-anime/00-overview.md)) show the search result fields and the status and score lists. This is an inference from the framework, not a cross-check against those docs; confirm when those areas are reviewed.

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, rows whose file starts with `31-tracker-shikimori-bangumi/` (all in [p1-tasks.md](p1-tasks.md)).

### 6.1 Counts

Total: 49 tasks (`T-31a-001` to `T-31a-049`).

| Priority | Effort S | Effort M | Effort L | Effort XL | Total |
|---|---|---|---|---|---|
| P0 | 0 | 0 | 0 | 0 | 0 |
| P1 | 32 | 13 | 0 | 0 | 45 |
| P2 | 2 | 1 | 0 | 0 | 3 |
| P3 | 1 | 0 | 0 | 0 | 1 |
| **Total** | **35** | **14** | **0** | **0** | **49** |

P2 tasks: T-31a-044 (Bangumi anime progress via episode endpoints, M), T-31a-045 (anonymous search, S), T-31a-047 (logo assets, S). P3: T-31a-049 (Android-only plumbing and replacements, S).

### 6.2 P1 critical path (in dependency order)

The effort-weighted longest chain is T-31a-043 → T-31a-004 → T-31a-005 → T-31a-027 → T-31a-029 → T-31a-032 → T-31a-036 → T-31a-046.

1. T-31a-043 lenient decoding rules (depends on area:27).
2. T-31a-004 Shikimori wire DTOs.
3. T-31a-005 Shikimori token type, expiry and single-flight session.
4. T-31a-027 Bangumi request authentication, refresh and User-Agent.
5. T-31a-029 Bangumi collection lookup.
6. T-31a-032 Bangumi bind.
7. T-31a-036 Bangumi test suite.
8. T-31a-046 live verification (needs the user's test accounts; not runnable by an agent).

Parallel gates that must be decided early: T-31a-039 (OAuth client configuration, D-OPEN-1, no dependencies) gates T-31a-006 and T-31a-026 (login). T-31a-002 and T-31a-021 (status enums, no dependencies) gate most mapping work.

There is no P0 critical path. Nothing in this area blocks daily reading or watching.

### 6.3 Risks

- **Unverified remote facts.** Every "Verified" fact in [p1-topic-remote-api-verification.md](p1-topic-remote-api-verification.md) is from source or docs, not a live account. Section 6 of that file is empty. Refresh-token rotation (OQ-6, OQ-7), the Bangumi `ep_status` behaviour (D7) and the whoami shape (S-V17) are unconfirmed.
- **OAuth client ownership.** Login cannot work until D-OPEN-1 is answered and the user registers an application. The refresh rule (T-31a-005) and login (T-31a-006) are blocked on that configuration in practice.
- **Silent data loss in the Kotlin.** D22 (remote entry erases local dates), D17 (id spaces mixed) and D8 (unclamped score) are silent. Tests must cover them (see [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md)).
- **Token exposure.** The Kotlin stores the access token in a plain preference (D11) and the client secret is in source. Rust stores tokens in the secret store only; the client secret is config, not code.
- **Shared-framework coupling.** Thirteen tasks depend on area:27-tracking-core directly. If area:27 changes the `Tracker` trait or `TrackError` shape, this area's task list changes with it.
- **Priority labels.** This area uses P2 for behaviour changes on purpose (anime progress endpoint, anonymous search, logos). The global rule says P2 is long-tail polish. Flag for the orchestrator to decide whether T-31a-044 should be P1.
- **Dependency cycle fixed.** `T-31a-001` depended on `T-31a-003`, and `T-31a-003` depends on `T-31a-001`. The edge `T-31a-001 → T-31a-003` was removed from `T-31a-001` (its acceptance criteria do not use the score model). Task ids unchanged. The tasks lint passes.
- **Logo licensing.** The Shikimori and Bangumi logos belong to the providers (OQ-3). Shipping them is a legal question, not a code one.

---

## 7. Definition of done for Kurobayomi (this area)

Each item is observable from the running app or from a test named in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md).

1. **Registry and rows.** The tracking screen lists Shikimori and Bangumi as normal rows in registry order (after Kitsu and before Komga). Logo tile uses `LogoRef`. Login row is disabled with an explanation when no OAuth client is configured (OQ-8).
2. **Login.** Clicking login opens the authorisation URL in the default browser. After redirect, the username (Shikimori: numeric id from `/api/users/whoami`) appears. The token exists only in the secret store, under `4.token` or `5.token`. No plain-text token in any preference file or database row.
3. **Logout.** Deletes the secret entries for that tracker. No `"null"` string is ever written. Local track rows are kept.
4. **Token refresh.** A token within 3600 s of expiry is refreshed once. Concurrent requests trigger one refresh. A rejected refresh shows the auth-expired message with a re-login action and does not send the request with the old token.
5. **Search.** Logged-in search returns manga or anime results with the fields in [p1-rust-design.md](p1-rust-design.md) section 11. Logged-out search fails with `NotAuthenticated` until T-31a-045 is done, then works.
6. **Bind.** Binding a local entry to a remote entry sets `remote_id` to the catalogue id, `library_id` to the user-rate id (Shikimori), and copies status and score. Local start and finish dates are not overwritten. Start date is restored from history only when it is `0`.
7. **Status and score.** Shikimori picker, in order (ids 1, 2, 3, 4, 5, 6): manga "Reading, Completed, On hold, Dropped, Plan to read, Rereading"; anime "Watching, Completed, On hold, Dropped, Plan to watch, Rewatching" (labels from [p1-behavior.md](p1-behavior.md) section 2.14). Bangumi picker, in order (ids 3, 2, 4, 5, 1): manga "Reading, Completed, On hold, Dropped, Plan to read"; anime "Watching, Completed, On hold, Dropped, Plan to watch" (section 3.13; no rereading label, and rereading id is -1). Score picker is 0 to 10. A write never sends a score outside 0 to 10.
8. **Delete.** Removing tracking on Shikimori removes the remote user-rate row when the option is ticked. Bangumi does not offer the option. A delete with no library id returns `NoLibraryId` and sends no request.
9. **Errors.** Every failure shows a toast with a stable key (`track.err.auth_expired`, `NotAuthenticated`, `TooManyEntries`, `NotFound`, `UnknownStatus`, `NoLibraryId`, `NotConfigured`, `Decode`, `HttpStatus`). Unknown status returns an error, never a panic (D4).
10. **Persistence compatibility.** A backup made by Aniyomi restores tracker rows for ids 4 and 5 with the same status numbers. A backup made by Kurobayomi restores in Aniyomi (T-31a-048).
11. **Tests pass.** `cargo test -p kuro-track` runs the `TS-*` tests in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) with wiremock fixtures. Expiry tests use the `Clock` seam.
12. **Live check recorded.** [p1-topic-remote-api-verification.md](p1-topic-remote-api-verification.md) section 6 holds a dated result for each item in its section 5 (T-31a-046). Until then this area is not released.
13. **Anime progress is honest.** Bangumi anime progress is marked "unverified" in the UI until T-31a-044 (episode endpoint) is verified live.
