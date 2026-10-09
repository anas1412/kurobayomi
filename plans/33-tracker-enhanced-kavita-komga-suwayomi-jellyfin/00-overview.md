# area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin: overview

Area lead overview for the four "enhanced" trackers (Kavita, Komga, Suwayomi, Jellyfin). Every repo-relative path is relative to `/home/blackbox/aniyomi-src` (the read-only Aniyomi clone). Labels: **OBSERVED** = what the Kotlin does. **Proposal:** = what Kurobayomi should do.

Area id: `33-tracker-enhanced-kavita-komga-suwayomi-jellyfin`. Part 1 of 1. No other area references its files (`p1-behavior.md` section 10).

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

An enhanced tracker never shows a search dialog. It binds automatically to entries that come from one specific extension source, and it talks to the same self-hosted server that the source already talks to. Four trackers do this:

| Tracker | Tracker id | Kind | Bound source (Kotlin class name) | Server state it writes |
|---|---|---|---|---|
| Kavita | 8 (`KAVITA = 8L`, `TrackerManager.kt:22`) | manga | `eu.kanade.tachiyomi.extension.all.kavita.Kavita` (`Kavita.kt:99`) | chapter progress via `mark-chapter-until-as-read` |
| Komga | 6 (`Komga(6L)`, `TrackerManager.kt:32`) | manga | `eu.kanade.tachiyomi.extension.all.komga.Komga` (`Komga.kt:97`) | read progress via Mihon-specific v2 endpoints |
| Suwayomi | 9 (`TrackerManager.kt:35`) | manga | `eu.kanade.tachiyomi.extension.all.tachidesk.Tachidesk` (`Suwayomi.kt:89`) | chapter read flags via `PUT .../chapter/{index}` |
| Jellyfin | 102 (`JELLYFIN = 102L`, `TrackerManager.kt:24`) | anime | `eu.kanade.tachiyomi.animeextension.all.jellyfin.Jellyfin` (`Jellyfin.kt:86`) | "played" flags via `POST /Users/{u}/PlayedItems/{id}` |

The trackers keep no credentials of their own. Kavita and Jellyfin read API keys from the source's preferences. Suwayomi reads server address and Basic auth from its source's preferences. Komga has no credentials at all and relies on cookies that the Kotlin never sets itself (defect D-19). The tracker's own "login" is a dummy pair `"user"`/`"pass"` that only makes `isLoggedIn` true (`BaseTracker.kt:32-34`, `Kavita.kt:95-97`).

None of the 14 in-scope files renders UI. The settings rows that expose these trackers live in `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt` (area:27-tracking-core owns the screen).

### 1.2 Why it matters for a usable Kurobayomi

- These trackers are the only tracker path that needs no user search. A user who self-hosts Kavita, Komga, Suwayomi or Jellyfin gets progress sync for free once the matching source is installed.
- They are **not** needed to read manga or watch anime. They are P1 parity, not P0. The P0 set is owned by the reader, player, library and source areas.
- Two of the four (Kavita and Jellyfin) are where the Kotlin has the most defects that affect data: status is never persisted (D-02), refresh wipes personal fields (D-03), and Jellyfin marks only one episode per sync (D-11). The Rust port fixes these deliberately, so the port behaves differently from Aniyomi on purpose. Those differences go into release notes (T-33a-111).

### 1.3 Scope boundary

In scope (14 files, 1,494 LOC, from `p1-inventory.md`): the four tracker classes, their API classes, interceptors, DTOs and the Kavita OAuth model.

Out of scope and owned elsewhere: the base `Tracker` contract, persistence, retry queue and settings screen (area:27-tracking-core); the shared OkHttp client, DoH and user-agent (area:12-network-stack); the source-id algorithm and source registry (area:15-source-api-manga, area:16-source-api-anime, area:17-source-app-layer); the `source_<id>` preference store (area:02-preferences-system); the `manga_sync` and `anime_sync` tables (area:05-data-repos-entries-items, area:06-data-repos-history-track-source-misc).

---

## 2. Module map

### 2.1 Kotlin packages (OBSERVED)

| Package (under `app/src/main/java/eu/kanade/tachiyomi/data/track/`) | Files (LOC) | Responsibility |
|---|---|---|
| `jellyfin/` | `Jellyfin.kt` (105), `JellyfinApi.kt` (168), `JellyfinInterceptor.kt` (73), `dto/JFItem.kt` (22) | Anime tracker. Parses the tracking URL (`/Users/{u}/Items/{i}` plus fragment), reads item and episode state, posts "played" flags, injects `api_key` into every request from the Jellyfin source's preferences. |
| `kavita/` | `Kavita.kt` (148), `KavitaApi.kt` (193), `KavitaInterceptor.kt` (26), `KavitaModels.kt` (86) | Manga tracker. Obtains JWTs from up to three Kavita sources (`APIURL`, `APIKEY`), computes total chapters from volumes, reads latest chapter, marks chapters read. |
| `komga/` | `Komga.kt` (115), `KomgaApi.kt` (114), `KomgaModels.kt` (107) | Manga tracker. Series and readlist dispatch, v2 read-progress endpoints, status from book counts. No auth header. |
| `suwayomi/` | `Suwayomi.kt` (111), `SuwayomiApi.kt` (126), `SuwayomiModels.kt` (100) | Manga tracker for the Tachidesk server. Basic auth from source preferences, chapter index lookup, `PUT .../chapter/{index}`. |

### 2.2 Shared Kotlin code the scope depends on (OBSERVED, owned elsewhere)

| Path | Role |
|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt` | Credential state (`isLoggedIn`, `saveCredentials`, `logout`) |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedMangaTracker.kt` | Interface for Kavita, Komga, Suwayomi (`accept`, `getAcceptedSources`, `loginNoop`, `match`, `isTrackFrom`, `migrateTrack`) |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt` | Same interface for Jellyfin |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt` | Tracker ids and registry |
| `app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt` | `bindEnhancedTrackers` (`:81-105`) |
| `app/src/main/java/eu/kanade/domain/track/manga/interactor/SyncChapterProgressWithTrack.kt` | Enhanced sync rule (`:21-50`) |
| `app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt` | Per-chapter auto-track (`:25-59`) |
| `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/ConfigurableSource.kt` | `sourcePreferences()` = `source_<id>` file (`:20-26`) |
| `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt` | Shared client: timeouts 30 s / 30 s / 2 min, 5 MiB cache, DoH selection |

### 2.3 Proposed Rust crate and module map (Proposal)

Crate `kuro-track` (no GTK, `gio` or `libsecret` dependency, per area:27). Source identity lives in `kuro-sources`.

```
crates/kuro-track/src/trackers/
  mod.rs                      registry: the four trackers, built from TrackerDeps
  enhanced.rs                 EnhancedError, EntryRef, accepts(), is_track_from(), migrate_remote_url()
  http.rs                     tracker_client(): system DNS, no HTTP cache
  sources.rs                  SourceDirectory and SourcePrefs traits
  kavita/{mod,api,auth,dto}.rs
  komga/{mod,api,dto}.rs
  suwayomi/{mod,api,dto}.rs
  jellyfin/{mod,api,auth,dto,url}.rs
crates/kuro-sources/src/identity.rs   generate_source_id(name, lang, version_id) -> i64
crates/kuro-track/tests/enhanced_*.rs fixture tests (wiremock)
crates/kuro-track/tests/fixtures/     JSON fixtures (list in p1-edge-cases-and-tests.md section 9)
```

Source of truth: `p1-rust-design.md` section 1. Crates and their Android replacements are in its section 2 and section 11.

---

## 3. Reading guide

All documents in this folder, in the order a new engineer should read them:

1. [p1-inventory.md](p1-inventory.md): the 14 Kotlin files with LOC, declared names, port decision and task ids, plus the shared files the scope relies on.
2. [p1-topic-enhanced-tracker-framework.md](p1-topic-enhanced-tracker-framework.md): how enhanced trackers bind, sync and die. Lifecycle diagram, dead status branch (D-02), DNS and cache policy, migration semantics.
3. [p1-topic-kavita-tracker.md](p1-topic-kavita-tracker.md): the three things that are easy to get wrong in Kavita: JWT slots, the volume-based chapter count, and the latest-chapter number.
4. [p1-topic-jellyfin-progress-model.md](p1-topic-jellyfin-progress-model.md): the Jellyfin progress model. Tracking URL with fragment, the continuous rule, single-episode marking (D-11), UTC date (D-10).
5. [p1-behavior.md](p1-behavior.md): the full behaviour spec. Identity table, every endpoint algorithm for the four trackers, defects D-01 to D-32, message keys, timings and request counts.
6. [p1-data-and-config.md](p1-data-and-config.md): DTO field tables, status enum values, the source-id algorithm with worked vectors, preference keys, endpoint table, Rust type mapping.
7. [p1-rust-design.md](p1-rust-design.md): the Rust layout, types, traits, function bodies for each API call, error mapping, constants, Android-to-Linux mapping, pitfalls P-1 to P-12, and the open decisions in section 10.
8. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge cases E-01 to E-116 with expected Kotlin and Rust results, the test name for each, the fixture list, and manual release checks.
9. [p1-tasks.md](p1-tasks.md): 81 implementation tasks (`T-33a-001` to `T-33a-111`), lintable with `tasks.py lint`.
10. [00-overview.md](00-overview.md): this file.

---

## 4. Key design decisions and open questions

### 4.1 Decisions already taken in the design (Proposal unless marked parity)

| # | Topic | Kotlin today | Decision in the design | Where |
|---|---|---|---|---|
| D1 | Status source | Local status computed in `update()` is discarded by the caller (D-02). Status is effectively server-driven. | Status stays server-driven. `update` writes progress and returns the refreshed track. The caller persists that track's `status`, `last_chapter_read` and `total_chapters`. | `p1-rust-design.md` 3.5, pitfall P-1 |
| D2 | Refresh | Copies `score = -1.0`, dates `0`, `private = false` from the search default (D-03). | Refresh copies only `status`, `last_*_read`, `total_*`. Keeps score, dates and private. | `p1-rust-design.md` 3.5, pitfall P-2 |
| D3 | DNS | Three of four set `dns(Dns.SYSTEM)`; Kavita `getNewToken` uses the DoH client (D-04). | System resolver for all four trackers, always. | `p1-rust-design.md` 3.10 |
| D4 | HTTP cache | Tracker clients inherit a 5 MiB OkHttp cache, so GETs can be stale (P-5). | No HTTP cache in the Rust port. | `p1-rust-design.md` pitfall P-5 |
| D5 | Kavita auth failure | One 401 or 500 aborts loading all three slots (D-05). | Each slot is independent. A failing slot is logged and skipped. A 401 on an API call clears that slot and re-authenticates once (D-07). | `p1-rust-design.md` 3.5 |
| D6 | Missing source | `sourceManager.get(id) as ConfigurableSource` throws NPE or ClassCastException (D-05, D-17). | `SourceDirectory::source_prefs` returns `Option`. `None` skips the slot. | `p1-rust-design.md` 3.2 |
| D7 | Suwayomi settings | Server URL and credentials read once per process (D-08). | Read the three preference keys on every call. | `p1-rust-design.md` 3.8 |
| D8 | Suwayomi migration | `isTrackFrom` ignores `remoteUrl` (D-09). | Compare `remoteUrl == entry.url`, as the other three do. | `p1-rust-design.md` 3.8 |
| D9 | Jellyfin date | `DatePlayed` in device time zone with a literal `Z` (D-10). | Format UTC `yyyy-MM-ddTHH:mm:ss.sssZ` with the `time` crate. | `p1-rust-design.md` 3.9 |
| D10 | Jellyfin marking | One episode per sync; a continuous prefix is not marked (D-11). | Mark every unplayed episode with index `<= target + 0.001`, each as its own POST, 4 at a time. Strict parity is one const (`JELLYFIN_MARK_RANGE`). | `p1-rust-design.md` 3.9, section 8 |
| D11 | Jellyfin movie | Posts "played" for any target, including `0.0` (D-32). | Post only when `last_episode_seen >= 1.0`. | `p1-rust-design.md` 3.9 |
| D12 | Jellyfin key transport | `api_key` query parameter on every URL (P-8). | Header `X-Emby-Token`. Query fallback only if a test server rejects the header. | `p1-rust-design.md` 3.9, pitfall P-8 |
| D13 | Jellyfin key lookup | Unsynchronised cache, caches empty keys (D-15, D-16). | No cache. Prefs read per call. Empty key is `SourceNotConfigured`. | `p1-rust-design.md` 3.9 |
| D14 | Enhanced login state | Dummy `"user"`/`"pass"` credentials. | Boolean flag `enhanced_tracker_enabled_<id>`, default false. | `p1-data-and-config.md` 5.2, T-33a-008 |
| D15 | Match input | Title-free: matches by source URL, but the 27 contract takes a title. | `match_entry(&EntryRef)` carries the URL. This changes the 27 trait (T-33a-006). | `p1-rust-design.md` 3.1 |
| D16 | Source identity | Kotlin class names. | Identity string `"{name}/{lang}"`, for example `kavita/all`, `komga/all`, `tachidesk/en`, `jellyfin/all`. | `p1-rust-design.md` section 4 |
| D17 | Interceptors | OkHttp chain adds headers by side effect. | Explicit request builders that take an auth context. No `Interceptor` type is ported (T-33a-023). | `p1-rust-design.md` 11 |
| D18 | Kavita search | `TODO()` throws `NotImplementedError`, an `Error`, which `catch (e: Exception)` does not catch (D-21). | `EnhancedError::SearchUnsupported`. | `p1-rust-design.md` 3.3 |

Parity decisions kept on purpose:

- Kavita `total_chapters` heuristic (volume count vs max chapter number, `p1-behavior.md` 2.6). Kept as the Kotlin heuristic.
- Komga empty series `0,0,0` is UNREAD (D-20). Kept and documented.
- Jellyfin `didWatchEpisode` is ignored (D-31). Kept and documented.
- Jellyfin `equalsTo` tolerance `0.001` (`JellyfinApi.kt:162`). Kept.

### 4.2 Open questions that need the user's call

Each question has a recommendation. Question 1 is blocking for one task.

**Q1. Komga authentication (blocking for T-33a-062).** The Kotlin sends no `Authorization` header and relies on a shared cookie jar that Kurobayomi does not have (`NetworkHelper.kt:18`, `:22`, D-19).
- Option (a): ask for Komga URL, username and password in the tracker login dialog. Store the password in `SecretStore` (area:27). Send Basic auth.
- Option (b): copy cookies from a browser session. Not recommended.
- **Recommendation: (a).** The rest of the Komga tracker is independent, so T-33a-060 and T-33a-061 can proceed with (a) stubbed.

**Q2. Do you run any of these four servers?** Each tracker needs a live self-hosted server that the user administers, plus the matching extension installed. If the answer is no for a tracker, that tracker can move to P2 or be dropped.
- **Recommendation:** keep P1 only for trackers you run. Otherwise demote the whole tracker group to P2 and keep only the shared framework tasks (T-33a-001 to T-33a-009) at P1, since they are used by area:27's bind path.

**Q3. Jellyfin multi-episode marking (D-11).** Kotlin marks one episode per sync. Strict parity leaves earlier episodes unplayed on the server, so the next refresh resets local progress to UNSEEN.
- **Recommendation:** range marking (`JELLYFIN_MARK_RANGE = true`). Confirm the behaviour change is acceptable.

**Q4. Refresh keeps personal fields (D-03).** The Kotlin overwrites local score and dates with defaults. These trackers have no score list and no date support (`BaseTracker.kt:23-25`), so the visible effect is limited to backups.
- **Recommendation:** keep local values on refresh.

**Q5. Dummy credentials replaced by a flag (D14).** Backups from Aniyomi carry `pref_mangasync_username_<id>` = `"user"`. Import must still map that to logged-in state.
- **Recommendation:** flag, plus an import rule for the legacy `"user"` value. Owned jointly with area:21-backup-format-and-create and area:22-backup-restore.

**Q6. Kavita volume encoding (E-113).** Upstream Kavita encodes loose-leaf volumes as `volume * 10000` in chapter numbers. The Kotlin does not decode it, so `last_chapter_read` and `total_chapters` can be far too large for volume-only series.
- **Recommendation:** keep raw values for the first release (parity). Decoding is a P2 follow-up (T-33a-108).

**Q7. Kavita JWT renewal on 401 (D-07, E-116).** Kotlin never renews a token. The design re-authenticates once on 401. The Kavita server's real behaviour on an expired JWT has not been verified (it could be a redirect).
- **Recommendation:** implement the retry-once rule. Confirm on a live server (T-33a-109).

**Q8. Kavita and Jellyfin credentials in query strings.** Kotlin puts `apiKey` and `api_key` in URLs and logs.
- **Recommendation:** Jellyfin header (`X-Emby-Token`) with query fallback. Kavita `apiKey` stays in the authenticate query (the Kavita endpoint requires it), but is redacted in logs.

**Q9. Suwayomi tile colour (D-27).** The Kotlin logo colour is marked `// TODO`.
- **Recommendation:** sample the colour from the logo asset (P2, T-33a-084). Use `#FF2323` until then.

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What is needed | Tasks that depend on it |
|---|---|---|
| area:27-tracking-core | `Tracker` / `MangaTracker` / `AnimeTracker` traits, `TrackError`, `BaseTracker` credential state, `SecretStore` (Komga password), retry queue, `bind_enhanced` caller, settings screen, `spawn` for non-cancellable writes | T-27a-001, T-27a-003, T-27a-005, T-27a-006, T-27a-012, T-27a-020, T-27a-024, T-27a-033, T-27a-034, T-27a-035, T-27a-036, T-27a-044 |
| area:12-network-stack | Shared `reqwest` client, connect/read/call timeouts (30 s, 30 s, 120 s), default user-agent preference (Suwayomi), DoH bypass | T-33a-001, T-33a-004, T-33a-061, T-33a-091 |
| area:15-source-api-manga, area:16-source-api-anime | `generate_source_id` algorithm (MD5, first 8 bytes big-endian, `& i64::MAX`, lowercased name) | T-33a-002 |
| area:17-source-app-layer | Installed-source lookup returning `Option` (`SourceDirectory`), source identity strings (`kavita/all`, etc.) | T-33a-002, T-33a-003, T-33a-009 |
| area:02-preferences-system | `source_<id>` per-source preference store (`SourcePrefs::get_string`) for `APIURL`, `APIKEY`, `user_id`, `api_key`, and the three Suwayomi keys | T-33a-003, T-33a-020, T-33a-047, T-33a-090 |
| area:20-extension-strategy-redesign | Ownership of the external extension preference keys listed above (written by the extensions, read by the trackers) | T-33a-047, T-33a-090 |
| area:05-data-repos-entries-items, area:06-data-repos-history-track-source-misc | `manga_sync` and `anime_sync` repositories, `UNIQUE (manga_id, sync_id)`, REAL and INTEGER column mapping | Used by 27 persistence for every write in this area |
| area:21-backup-format-and-create, area:22-backup-restore | Backup export and import of tracker rows and the legacy `pref_mangasync_username_<id>` values | T-33a-008 (acceptance) |
| area:49b-android-res-values-drawables | Inventory of the four `ic_tracker_*.webp` drawables that become bundle assets | T-33a-100 |
| area:51-i18n | String table for the six new `track.enhanced.*` message keys and the existing `unread`, `reading`, `completed`, `unseen`, `watching`, `enhanced_*` keys | T-33a-005, T-33a-022, T-33a-111 |

### 5.2 What others need from this area

| Area | What they need | Task |
|---|---|---|
| area:27-tracking-core | `EnhancedMangaTracker::match_entry(&EntryRef)` replaces the title-based signature in the 27 trait, and `bind_enhanced` builds `EntryRef { id, source_id, url, title }`. This is a required edit to area 27 and needs its owner's sign-off. | T-33a-006 |
| area:27-tracking-core | Acceptance of `track.enhanced.*` message keys (the design proposes `MsgKey::TrackerError { tracker, detail: key }`) | T-33a-005 |
| area:51-i18n | Six new keys: `track.enhanced.unauthorized`, `track.enhanced.server_error`, `track.enhanced.login_required`, `track.enhanced.search_unsupported`, `track.enhanced.missing_fragment`, `track.enhanced.chapter_missing` (`p1-rust-design.md` 7.2) | T-33a-005 |
| area:21, area:22 | Confirmation that the `enhanced_tracker_enabled_<id>` flag is restored from backups (or the legacy `"user"` value is mapped) | T-33a-008 |

Nothing else in the repository references this area's files.

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then the rows of `TASKS.csv` whose `file` starts with `33-tracker-enhanced-kavita-komga-suwayomi-jellyfin/`. The export ran at 21:48 on 2026-10-08. All 81 rows are in `p1-tasks.md`.

### 6.1 Counts

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 0 | 0 | 0 | 0 | 0 |
| P1 | 51 | 19 | 0 | 0 | 70 |
| P2 | 7 | 0 | 0 | 0 | 7 |
| P3 | 4 | 0 | 0 | 0 | 4 |
| **Total** | **62** | **19** | **0** | **0** | **81** |

Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint p1-tasks.md` prints `ok`. Every `Depends:` token resolves to an existing task id (checked against `TASKS.csv`), and every area token is a real folder id.

### 6.2 P0 critical path

None. This area has no P0 tasks. The trackers are not required to read or watch.

### 6.3 P1 critical path (in order)

The tracker chains can run in parallel once the framework is in place.

1. **Framework.** T-33a-001 (module skeleton, needs T-27a-001) → T-33a-002 (source id) and T-33a-003 (`SourceDirectory`) → T-33a-004 (system DNS, no cache) → T-33a-005 (`EnhancedError`) → T-33a-006 (`EntryRef` signature, needs area 27 sign-off) → T-33a-007 (accept, is-track-from, migrate) → T-33a-008 (login flag) → T-33a-009 (settings rows).
2. **Kavita (longest manga chain).** T-33a-050 (URL helpers) → T-33a-051 (DTOs) → T-33a-052 (authenticate) → T-33a-047 (three-slot auth, needs 052) → T-33a-053 (volume walk) and T-33a-054 (latest chapter) → T-33a-055 (series read) → T-33a-056 (mark progress) → T-33a-057 (wire and auth tests). T-33a-048 (headers) runs after T-33a-047.
3. **Jellyfin (longest anime chain).** T-33a-010 (DTOs) → T-33a-011 (URL parser) → T-33a-014 (episode query, prefix kept) → T-33a-015 (continuous rule) → T-33a-016 (range marking, UTC date) → T-33a-037 (wire and rule tests). T-33a-020 and T-33a-021 (auth and headers) feed T-33a-016.
4. **Komga.** T-33a-061 (client) → T-33a-062 (**auth decision, Q1**) → T-33a-070 (dispatch and DTOs) → T-33a-071 (v2 progress) → T-33a-072 (payloads) → T-33a-074 (wire tests). T-33a-073 (status) feeds T-33a-063.
5. **Suwayomi.** T-33a-090 (settings per call) → T-33a-091 (headers, Basic auth) → T-33a-095 (DTOs) → T-33a-092 (track search) → T-33a-093 (update progress) → T-33a-094 (wire tests).

### 6.4 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Komga auth decision not taken (Q1) | T-33a-062 cannot close; Komga is unusable | Take Q1 first. Option (a) is recommended. |
| No live servers to test against | Expected results for E-114 (Komga readlists), E-116 (Kavita JWT expiry), E-115 (Jellyfin producer URL form) are inferred from source and upstream code | Manual release checks in `p1-edge-cases-and-tests.md` section 11 and T-33a-110. |
| Jellyfin tracking URL shape is produced by an external extension | If the producer's fragment form differs from the consumer contract, every Jellyfin bind fails | T-33a-013 fixture contract with the extension owner. |
| Deliberate behaviour changes (D-02, D-03, D-11, D-24) | Users comparing with Aniyomi see different status, progress and personal fields | T-33a-111 release notes. |
| Area 27 signature change (T-33a-006) | Blocks every enhanced tracker bind path | Get sign-off before T-33a-006; keep title-based match as a shim until then. |
| Kavita `*10000` volume encoding (E-113) | Wrong `last_chapter_read` for volume-only series | Q6 decision; T-33a-108. |
| Defect in the Rust design code (see 7.2 below) | Reverse-proxy prefixes dropped for Jellyfin episode queries | Fix the code block before T-33a-014 starts. |

---

## 7. Definition of done for this area in Kurobayomi

This area is done when all of the following hold on a CachyOS desktop with a live server for each tracker the user runs.

### 7.1 Observable behaviours

1. **Binding.** Adding a manga from an installed Kavita, Komga or Suwayomi source to the library creates one tracking row with the right `tracker_id` (8, 6 or 9), `remote_url` equal to the source's entry URL, and the progress that the server reports. No search dialog appears. If the tracker is not logged in, nothing is bound.
2. **Binding failures are visible.** A Kavita authentication failure for one slot does not block the other two slots. A failed bind logs a WARN with the same wording as the Kotlin (`p1-behavior.md` section 7) and does not silently return nothing.
3. **Settings.** Settings > Tracking > Enhanced trackers shows only trackers whose source is installed. Uninstalled ones are named in the info row "Available but source not installed: %s". Login and logout toggle the flag. No secret is written for Kavita, Jellyfin or Suwayomi.
4. **Reading progress syncs to Kavita.** After reading chapter 12 of a Kavita series, the server shows chapters up to 12 as read. The local `last_chapter_read` equals the server's latest chapter value. Status is the server's (UNREAD, READING or COMPLETED).
5. **Reading progress syncs to Komga.** Series progress uses `/api/v2/series/{id}/read-progress/tachiyomi`. Readlist progress uses the readlist path. A stored v2 URL parses as v2.
6. **Reading progress syncs to Suwayomi.** Reading chapter 5 sends `PUT .../chapter/{index of chapter 5}` with `markPrevRead=true&read=true`. A missing chapter number shows the message "Chapter 5 is not on the server" through `track.enhanced.chapter_missing`, not an English exception.
7. **Watching syncs to Jellyfin.** Watching episodes 1 to 3 of a season marks episodes 1, 2 and 3 as played on the server. `DatePlayed` is UTC. The next refresh shows `last_episode_seen = 3` and WATCHING. Watching a movie marks it played.
8. **Refresh keeps personal fields.** After a refresh, local score, start and finish dates, and private flag are unchanged.
9. **Migration.** Migrating an entry from one Suwayomi source to another matches the track by URL and copies `remote_url` only to an accepted source.
10. **Missing fragment.** A Jellyfin tracking URL with no fragment fails with "Tracking link is incomplete", not a crash.
11. **Test suite.** `cargo test -p kuro-track` passes the wire tests for E-01 to E-102 (excluding the manual items) and the identity tests E-103 to E-112. The Jellyfin module has at least 24 tests (T-33a-037).
12. **Logs.** No log line contains an API key, a JWT or a Basic auth password. Logged URLs have their query string removed.

### 7.2 Known defect in the design text to fix before implementation

`p1-rust-design.md` section 3.9, in the `episodes(...)` code block, builds the base with `url.origin().ascii_serialization()`. That drops a reverse-proxy path prefix such as `/jf`. This contradicts the D-13 fix, E-81 (`/jf/Shows/...` expected) and T-33a-014's acceptance criterion. The fix is to build the base from the tracking URL's own scheme, host and path up to the `Users` segment, found by name (as the design text already says for D-13). A second gap in the same section: `mark_played` uses `self.base_url`, which is not a field of the declared `JellyfinApi` struct. The field must be added.

These are design-text defects found while writing this overview. They were not edited in place, because the area's design doc belongs to the area owner.

### 7.3 Lint note outside this area

`40-ui-browse-manga/p1-tasks.md` (not in this folder) has four `unknown area` tokens (`area:41-ui-browse-manga-part-2`). They were not changed, because they belong to area 40.
