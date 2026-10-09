# Area 27-tracking-core: overview

Area lead overview for the tracker framework in Kurobayomi. Repo-relative paths are relative to the read-only Aniyomi clone at `/home/blackbox/aniyomi-src`. Labels: **OBSERVED** is what the Kotlin does. **Proposal:** is what Kurobayomi should do.

Two parts share this folder. Part 1 (`p1-*`, task IDs `T-27a-NNN`) is the tracker framework, models, interactors, login plumbing and the shared pickers. Part 2 (`p2-*`, task IDs `T-27b-NNN`) is the per-entry track dialogs, the track item types and the OAuth callback activity. Part 2 cites part 1 symbols by name and depends on it through `area:27-tracking-core`.

---

## 1. Purpose and scope

**OBSERVED.** Tracking links a local manga or anime entry to a remote list entry on an external service (AniList, MyAnimeList, Kitsu, Shikimori, Bangumi, MangaUpdates, Simkl, and the enhanced self-hosted Kavita, Komga, Suwayomi and Jellyfin). The user can:

- log in to a tracker (OAuth or username and password),
- bind an entry to a remote title (search, then "Add tracking"),
- push status, progress (episode or chapter), score, start and finish dates, and a private flag to the remote,
- have progress sent automatically when a chapter is read or an episode is watched,
- have offline progress queued and retried,
- refresh remote state into the local app.

**Why it matters for a usable Kurobayomi.** Reading and watching work without tracking, so the area is **P1** (parity), not P0. Users who already track on AniList or MyAnimeList will not switch to an app that drops their progress, so tracking is the first parity feature after the reader and player.

**Scope size.** Part 1: 39 Kotlin files, 4326 LOC (`python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 27-tracking-core 1`). Part 2: 6 files, 1879 LOC (`... 27-tracking-core 2`). Total 45 files, 6205 LOC. Both inventories pass their coverage check (every scope path appears in the table).

**Out of scope here.** Tracker API clients (areas 28 to 33), the track database schema (areas 05 and 06), and the entry screens that call into this area (areas 37 to 41, 43, 45).

---

## 2. Module map

### 2.1 Kotlin packages to responsibilities

| Package (repo-relative) | Responsibility | Files in scope |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/track/` | Tracker contracts (`Tracker`, `BaseTracker`, `AnimeTracker`, `MangaTracker`, `EnhancedAnimeTracker`, `EnhancedMangaTracker`, `DeletableAnimeTracker`, `DeletableMangaTracker`), the registry `TrackerManager` | `Tracker.kt`, `BaseTracker.kt`, `AnimeTracker.kt`, `MangaTracker.kt`, `EnhancedAnimeTracker.kt`, `EnhancedMangaTracker.kt`, `DeletableAnimeTracker.kt`, `DeletableMangaTracker.kt`, `TrackerManager.kt` (part 1) |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/model/` | Search result models `AnimeTrackSearch`, `MangaTrackSearch` (part 1) | 2 files |
| `app/src/main/java/eu/kanade/domain/track/{anime,manga}/interactor/` | Use cases: bind (`AddAnimeTracks`, `AddMangaTracks`), refresh (`RefreshAnimeTracks`, `RefreshMangaTracks`), progress sync (`SyncEpisodeProgressWithTrack`, `SyncChapterProgressWithTrack`), progress triggers (`TrackEpisode`, `TrackChapter`) | 8 files (part 1) |
| `app/src/main/java/eu/kanade/domain/track/{anime,manga}/model/` | Domain-to-legacy mapping (`toDbTrack`, `toDomainTrack`, `copyPersonalFrom`) | 2 files (part 1) |
| `app/src/main/java/eu/kanade/domain/track/{anime,manga}/service/` | WorkManager retry jobs `DelayedAnimeTrackingUpdateJob`, `DelayedMangaTrackingUpdateJob` | 2 files (part 1) |
| `app/src/main/java/eu/kanade/domain/track/{anime,manga}/store/` | Retry queue on SharedPreferences `tracking_queue` (`DelayedAnimeTrackingStore`, `DelayedMangaTrackingStore`) | 2 files (part 1) |
| `app/src/main/java/eu/kanade/domain/track/model/` | `AutoTrackState` enum (`ALWAYS`, `ASK`, `NEVER`) | 1 file (part 1) |
| `app/src/main/java/eu/kanade/domain/track/service/` | `TrackPreferences`: all tracking preference keys, credentials, token, auth-expired flag | 1 file (part 1) |
| `app/src/main/java/eu/kanade/presentation/track/` | Shared Compose pieces: `TrackInfoDialogSelector.kt` (status list, number wheel, score wheel, date picker), `components/TrackLogoIcon.kt`, and the preview provider `components/TrackLogoIconPreviewProvider.kt` | 3 files (part 1) |
| `app/src/main/java/eu/kanade/presentation/track/{anime,manga}/` | Home sheet (`*TrackInfoDialogHome.kt`), remote search screen (`*TrackerSearch.kt`), preview providers | 8 files (part 1) |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt` | Settings > Tracking: switches, auto-update list, login rows, enhanced services, logout | 1 file (part 1) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/entries/{anime,manga}/track/` | Voyager screens that host the dialogs (`AnimeTrackInfoDialog.kt` and `MangaTrackInfoDialog.kt`, 874 lines each, near-identical), `AnimeTrackItem.kt`, `MangaTrackItem.kt` | 4 files (part 2) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/` | OAuth callback: `BaseOAuthLoginActivity.kt` and `TrackLoginActivity.kt` | 2 files (part 2) |
| `domain/src/main/java/tachiyomi/domain/track/{anime,manga}/` | Domain models and interactors `GetAnimeTracks`, `InsertAnimeTrack`, `DeleteAnimeTrack` (read, owned by area 09) | not in scope |
| `data/src/main/java/tachiyomi/data/track/` and `data/src/main/sqldelight*/.../anime_sync.sq`, `manga_sync.sq` | Track repository and tables (owned by areas 05 and 06) | not in scope |

### 2.2 Proposed Rust module map

The two part documents propose slightly different layouts. This is the reconciled map. Where the parts disagree, the choice is listed in section 4.

**Crate `kuro-track`** (no GTK, no OS calls; rule 1 of `plans/addenda/requirements-cross-platform.md`):

```
crates/kuro-track/src/
  lib.rs
  model.rs            AnimeTrack, MangaTrack, TrackRecord, TrackItem, EntryKind     (part 1 section 3, part 2 section 1)
  model/search.rs     AnimeTrackSearch, MangaTrackSearch
  tracker.rs          Tracker, AnimeTracker, MangaTracker, Enhanced*, Deletable*   (part 1 names win)
  base.rs             credential state, login watch channel
  manager.rs          TrackerManager (registry, logged-in list, watch stream)
  policy.rs           AutoTrackState, mark-read decision
  prefs.rs            TrackPrefs over kuro-prefs keys
  remote.rs           setter rules: status, progress, score, dates, private
  bind.rs             bind and bind_enhanced (anime and manga)
  sync.rs             refresh_all, sync_progress
  progress.rs         track_episode, track_chapter (online, offline, queue)
  queue/mod.rs        RetryQueue
  queue/store.rs      SQL on track_retry_queue
  queue/retry.rs      RetryScheduler
  entry_kind.rs       EntryKind                                                    (part 2)
  item.rs             assemble_items() (mapToTrackItem)                            (part 2)
  progress_range.rs   progress_range(), truncation rules                           (part 2)
  score.rs            score index lookup                                           (part 2)
  dates.rs            picker <-> stored conversion and bounds                      (part 2)
  messages.rs         message keys to English text                                 (part 2)
  oauth/mod.rs        route_uri()                                                  (part 2 section 3, part 1 section 5 merged)
  oauth/callback.rs   parse_callback(Uri) -> CallbackOutcome
  oauth/pkce.rs       PkceSession
  oauth/pending.rs    PendingLogin store with TTL and CSRF guard
  error.rs            TrackError (thiserror)
```

**Crate `kuro-platform`** (the only crate that touches OS APIs): traits `SecretStore`, `Opener`, `Connectivity`, `UriSchemeHandler` (part 1 section 5 and 8, part 2 section 8). The Linux implementations are `keyring` (Secret Service), `gio::NetworkMonitor`, `gio::AppInfo::launch_default_for_uri`, and GApplication `open`.

**GUI crate `kurobayomi`** (`src/`):

```
src/track/
  mod.rs          TrackNav (modal stack), entry points, action dispatch to tokio
  sheet.rs        per-entry tracking sheet (replaces src/entry.rs tracking(), lines 496-537)
  search.rs       remote search page (replaces tracker_search(), lines 539-568)
  home_model.rs   Home model: subscribes to rows and logins, refreshes on open
  status.rs       status dialog
  progress.rs     progress dialog (episode or chapter)
  score.rs        score dialog
  dates.rs        start and finish date dialog and remover
  remove.rs       remove-tracking dialog
  login.rs        login feedback view and the manual paste fallback
  settings.rs     Settings > Tracking rows (from the TrackPrefs table)
  logo.rs         48 px logo tile
  toast.rs        in-window toast overlay (no libadwaita)
```

`src/entry.rs` keeps the entry page. Its `dialog_of()` helper is replaced by `TrackNav::push`.

---

## 3. Reading guide

All files are in this folder.

| Document | What it contains |
|---|---|
| [00-overview.md](00-overview.md) | This file: scope, module maps, decisions, dependencies, task summary, definition of done. |
| [p1-inventory.md](p1-inventory.md) | Part 1 file table (39 rows) with port decision, Rust target, priority and task IDs. |
| [p1-behavior.md](p1-behavior.md) | Part 1 behaviour spec in 22 sections: registry, base contract, setter rules, mapping, bind, refresh, sync, triggers, queue, mark-read policy, settings, sheet, pickers, search, remove, OAuth, messages, concurrency, constants, cross-area rules, defects D1 to D20. |
| [p1-data-and-config.md](p1-data-and-config.md) | Part 1 data: tracker ids, preference keys with defaults, credential keys, domain and legacy models, SQL tables, queue storage, enums, sentinels, OAuth endpoints, bit layouts, file paths. |
| [p1-rust-design.md](p1-rust-design.md) | Part 1 Rust design: layout, crates, key traits, async model, secrets and login state, retry scheduler, mockup integration, Android-to-Linux mapping, error policy, pitfalls. |
| [p1-ui-parity.md](p1-ui-parity.md) | Part 1 UI parity: screens, Settings > Tracking, sheet, logo tile, pickers, search, login and logout dialogs, states, keyboard, responsive rules, theme roles, mockup gaps. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Part 1 edge cases per function, races, failure modes, regression tests for D1 to D20, rule and queue tests, UI tests, fixtures, properties, migration tests, manual checklist. |
| [p1-tasks.md](p1-tasks.md) | Part 1 implementation tasks, 82 rows, `T-27a-NNN`. |
| [p1-topic-progress-queue.md](p1-topic-progress-queue.md) | Topic: the offline retry queue and its scheduler (attempt counts, backoff arithmetic, parking, startup, open questions). |
| [p1-topic-tracker-oauth.md](p1-topic-tracker-oauth.md) | Topic: first version of login and OAuth plumbing on Linux (redirect URI options, token storage, logout, auth expiry, open questions). |
| [p2-inventory.md](p2-inventory.md) | Part 2 file table (6 files): track dialogs, track items, OAuth activities. |
| [p2-behavior.md](p2-behavior.md) | Part 2 behaviour: Home sheet, remote search, status, progress, score, dates, remove, OAuth routing, messages, concurrency, constants, observed defects D-1 to D-17. |
| [p2-data-and-config.md](p2-data-and-config.md) | Part 2 data: the domain track row, enumerations, preference keys used by the dialogs, network endpoints of login, parameter table. |
| [p2-rust-design.md](p2-rust-design.md) | Part 2 Rust design: module layout, added dependencies, core types, dialog state machines, threading, errors, platform mapping, mockup integration. |
| [p2-ui-parity.md](p2-ui-parity.md) | Part 2 UI parity: container decision (modal windows), each dialog, login feedback, toasts, keyboard, responsive rules, mockup gap list. |
| [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md) | Part 2 edge cases E-01 to E-74 with expected outcomes, date and progress worked examples, test list, fixtures, failure matrix. |
| [p2-tasks.md](p2-tasks.md) | Part 2 implementation tasks, 41 rows, `T-27b-NNN`. |
| [p2-topic-oauth-callback.md](p2-topic-oauth-callback.md) | Topic: the OAuth callback on Linux (URI scheme, `.desktop` file, GApplication `open`, PKCE, pending attempts, login-CSRF guard, manual paste fallback). |

---

## 4. Key design decisions and open questions

### 4.1 Decisions with a recommendation

| # | Decision | Options | Recommendation |
|---|---|---|---|
| D-A | Redirect URI scheme | (A) keep `aniyomi://<tracker>-auth` with the existing client ids; (B) own scheme `kurobayomi://` with new client ids; (C) loopback `http://127.0.0.1:<port>/callback` | **A** now. It works with the registrations that exist. Revisit B only if the user registers his own developer apps. Source: `p1-topic-tracker-oauth.md` section 3. |
| D-B | Application id and `.desktop` file | `dev.kurobayomi.Kurobayomi` (proposed) | Confirm once, before packaging (`p2-topic-oauth-callback.md` OQ-4). |
| D-C | Where the queue lives | SharedPreferences file `tracking_queue` (source) versus a SQLite table `track_retry_queue` | **SQLite table** with a `kind` column (anime or manga). This fixes defects D1 (row identity), D3 (shared file, no kind namespace) and D6 (non-atomic read-modify-write). Source: `p1-behavior.md` section 22; `p1-topic-progress-queue.md` section 3. |
| D-D | Fix the queue key | `addAnime(track.animeId, …)` in the external player path (source) | Key by track id (`_id`), as fixes D2 requires. |
| D-E | Container for dialogs | Modal `gtk::Window` for status, progress, score, dates and remove. Full page for search. No popovers. | As in `p2-ui-parity.md` section 1. |
| D-F | Secret storage | Service name `kurobayomi.track` in the Secret Service through `keyring`; accounts `tracker/<id>/oauth`, `tracker/<id>/account`, `tracker/<id>/expired` (the part 2 key names, under the part 1 service name) | Use this layout. It is a conflict in the docs: part 1 section 5.1 uses accounts `<id>.token`, `<id>.username`, `<id>.password`. |
| D-G | Secret fallback when no Secret Service runs | (1) part 1 section 5.1: write `$XDG_DATA_HOME/kurobayomi/secrets.json` mode `0600`; (2) part 2 `T-27b-027`: report "Secret storage is unavailable" and store nothing | **(2)**. A plaintext token file is the wrong default for a credential. On KDE the Secret Service is usually provided by KWallet, so the failure should be rare. If the user wants a fallback later, make it an explicit opt-in setting. |
| D-H | Date handling crate | `chrono` (part 2 section 2, needed for `Local`, `Utc`, `NaiveDate`) versus `time` (part 1 section 2 says `time` or `std::time`) | **`chrono`**. `time` needs a feature flag and can fail to read the local offset in a multi-threaded process. `chrono::Local` reads the zone reliably. Update `p1-rust-design.md` section 2 to match. |
| D-I | PKCE randomness | `rand` plus `sha2` plus `base64` (part 1 section 2) versus `getrandom` 0.2 plus `base64` (part 2 section 2) | **`getrandom` 0.2** with `base64` (`URL_SAFE_NO_PAD`) and `sha2` for the S256 challenge. 50 random bytes, as `PkceUtil.kt:9` does. Remove `rand` from the list. |
| D-J | Trait name | `Tracker`, `AnimeTracker`, `MangaTracker` (part 1, the source names) versus `TrackerService` (part 2 section 4) | **`Tracker`**, as part 1 says. Rename the part 2 references. |
| D-K | Priority of the area | P0 (as the part 1 inventory had it for six contract files) versus P1 (as the task files have it) | **P1** for every file and task. Tracking is not needed to read or watch. The inventory was corrected in this pass (section 6.4). |
| D-L | Login denial | Clear the stored token (source, defect D-2) versus keep it and show "<name> login cancelled" | **Keep the token.** Part 2 `T-27b-024` already says so. Confirm with the user (`p1-topic-tracker-oauth.md` OQ 2). |
| D-M | Login timeout | Browser login pending for 5 minutes, then the pending attempt expires | 5 minutes, as proposed in `p1-topic-tracker-oauth.md` OQ 3 and `p2-topic-oauth-callback.md` OQ-3. |

### 4.2 Open questions for the user

These cannot be decided from the source. Each has a default so that implementation can continue.

1. **Embedded client secrets (Bangumi, Shikimori, Simkl).** The source embeds the client secrets (`bangumi/BangumiApi.kt:269`, `shikimori/ShikimoriApi.kt:259`, `simkl/SimklApi.kt:238`). Keep the upstream values, or register Kurobayomi's own OAuth applications? **Default:** keep upstream values for now. This blocks the acceptance of the login task for those three trackers only (`p2-topic-oauth-callback.md` OQ-1).
2. **Does MyAnimeList echo `state`?** Not documented in the source. **Default:** follow section 7.1 of `p2-topic-oauth-callback.md` until the provider documentation is checked (OQ-2).
3. **Login-CSRF guard.** Accept a code only for a login started in this session in the last 5 minutes. **Default:** yes (OQ-3).
4. **Queue expiry for logged-out trackers.** Expire queued items after 30 days, or keep them until removed? `p1-topic-progress-queue.md` section 12.2. **Recommendation:** keep until removed, and show the count, because silent expiry loses reading progress.
5. **Parked-queue notice.** Show "N tracking updates pending" on the next entry open? `p1-topic-progress-queue.md` section 12.1. **Recommendation:** yes. `T-27a-071` is the task.
6. **Enhanced-tracker auto-match on add.** The source matches enhanced trackers after add to library. Keep that behaviour unchanged? **Default:** yes, as `T-27a-033` and `T-27b-013` specify.

---

## 5. Cross-area dependencies

### 5.1 What this area needs from other areas

| Area | Needed for | Tasks |
|---|---|---|
| `area:01-foundation-app-bootstrap` | Paths, `SecretStore`, `Opener`, logging and redaction, threading and cancellation, the app context (DI replacement for Injekt) | `T-27a-020`, `T-27a-067`, `T-27a-068`, `T-27b-020`, `T-27b-027` |
| `area:02-preferences-system` | Preference keys and defaults, `app_date_format`, `UiPreferences` | `T-27a-002`, `T-27a-019`, `T-27a-043`, `T-27b-031`, `T-27b-030` |
| `area:05-data-repos-entries-items` and `area:06-data-repos-history-track-source-misc` | Track repository (`anime_sync`, `manga_sync`), history reads for the start date, the queue store host | `T-27a-015`, `T-27a-079`, `T-27a-014`, `T-27a-025` |
| `area:09-domain-source-track-download-release-misc` | Domain track models and interactors (`GetAnimeTracks`, `InsertAnimeTrack`, `DeleteAnimeTrack`) | `T-27b-001` |
| `area:12-network-stack` | The HTTP client for every tracker, connectivity check through GIO | `T-27a-001`, `T-27a-027`, `T-27a-042` |
| `area:17-source-app-layer` | Source identity for the enhanced `accept()` check (`source::class.qualifiedName` in the source) | `T-27a-024`, `T-27a-077` |
| `area:21-backup-format-and-create`, `area:22-backup-restore` | Track rows in backup, and the rule that tokens are not backed up (`PreferenceBackupCreator.kt:82`) | `T-27a-066` |
| `area:28-tracker-anilist` to `area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin` | Each tracker's `login`, `logout`, `refresh`, `update`, `bind`, `search`, `match`, status and score lists, `authUrl` | `T-27a-021`, `T-27a-023`, `T-27a-062`, `T-27a-072`, `T-27b-022` |
| `area:34-images-covers-cache` | Search result cover images | `T-27a-057` |
| `area:36-ui-shell-navigation-security-webview` | Deep links, GApplication `open`, modal navigation, `.desktop` registration | `T-27a-022`, `T-27b-019`, `T-27b-023` |
| `area:47-settings-screens-general` | Settings rows and dialogs, the manual paste fallback host | `T-27a-043`, `T-27a-044`, `T-27b-029` |
| `area:49-theme-and-design-system` | Tracker brand colours in the cards, theme roles | `T-27a-050`, `T-27a-061` |
| `area:51-i18n` | Message keys | `T-27a-078` |
| `area:53-platform-manifest-intents-workers` | Scheduling replacement for WorkManager, `openInBrowser`, `copyToClipboard` | `T-27a-022` |

### 5.2 What other areas need from this one

| Consumer | What it calls | Tasks |
|---|---|---|
| Entry screens, `area:37-ui-library` to `area:41-ui-browse-anime` (`AnimeScreenModel`, `MangaScreenModel`) | Tracking sheet, add-to-library auto-open, mark-read prompt, derived track counts | `T-27a-039`, `T-27a-041`, `T-27a-048`, `T-27a-070`, `T-27b-004` |
| Reader and player, `area:43-reader-core`, `area:45-player-core` (`ReaderViewModel`, `PlayerViewModel`, `ExternalIntents`) | `TrackChapter`, `TrackEpisode`, auto-update gating | `T-27a-027`, `T-27a-028`, `T-27a-040`, `T-27a-029` |
| Tracker implementations, `area:28` to `area:33` | `Tracker`, `BaseTracker`, `AnimeTracker`, `MangaTracker`, enhanced and deletable traits, `TrackerManager` | `T-27a-001` to `T-27a-013`, `T-27a-012` |
| Backup, `area:21` and `area:22` | Track mapping `toDbTrack` and `toDomainTrack` | `T-27a-014`, `T-27a-016` |
| Settings, `area:47` | `TrackPreferences` keys | `T-27a-019`, `T-27a-043`, `T-27a-044` |
| OAuth activity routing, `area:36` | `route_uri()` and `parse_callback` | `T-27b-021`, `T-27b-022` |

---

## 6. Task summary

### 6.1 Counts

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then the 123 rows of `TASKS.csv` whose `file` starts with `27-tracking-core/`.

| File | Tasks | P1 | P2 | P3 | S | M | L | XL |
|---|---|---|---|---|---|---|---|---|
| `p1-tasks.md` (`T-27a-*`) | 82 | 75 | 5 | 2 | 49 | 29 | 4 | 0 |
| `p2-tasks.md` (`T-27b-*`) | 41 | 37 | 3 | 1 | 27 | 13 | 1 | 0 |
| **Total** | **123** | **112** | **8** | **3** | **76** | **42** | **5** | **0** |

P0: **0 tasks.** This area has no P0 task. The five L tasks are `T-27a-022` (redirect router and pending login), `T-27a-029` (retry scheduler), `T-27a-048` (per-entry sheet), `T-27a-056` (search screen), and `T-27b-010` (start and finish date dialog).

### 6.2 P0 critical path

There is no P0 task, so there is no P0 critical path. The P1 path that must be built first for tracking to work end to end is the longest dependency chain among P1 tasks (9 tasks, after the cycle fixes in section 6.3):

1. `T-27a-014` Domain model and DB mapping (M)
2. `T-27a-017` Search result models (S)
3. `T-27a-005` AnimeTracker trait with default methods (M)
4. `T-27a-010` Date and private setters, and update_remote persistence (M)
5. `T-27a-007` Anime status, completion and progress setters (M)
6. `T-27a-027` Track episode progress, anime (M)
7. `T-27a-026` Queue skip and removal rules (S)
8. `T-27a-029` Retry scheduler for anime and manga (L)
9. `T-27a-030` Manga retry job parity (S)

Before step 1, `area:06` (track repository) and `area:02` (preferences) must be available. The entry-level work (`T-27a-048`, `T-27a-056`) then follows.

### 6.3 Dependency graph fixes made in this pass

The task graph had seven cycles (strongly connected groups found by Tarjan's algorithm over the 123 rows), which would make it impossible to schedule. The three-task group `T-27a-048`, `T-27a-049`, `T-27a-063` needed two edges removed, so eight dependency edges were removed in total. No task IDs changed. Each removed edge is listed below with the reason it is the one to drop.

| Task | Removed dependency | Why it is safe |
|---|---|---|
| `T-27a-001` (Tracker trait) | `T-27a-003` (BaseTracker) | The trait comes before its base implementation. |
| `T-27a-014` (domain model) | `T-27a-017` (search models) | Search models are built on the domain model, not the reverse. |
| `T-27a-013` (deletable traits) | `T-27a-059` (remove flow) | The remove flow calls the deletable traits. |
| `T-27a-048` (per-entry sheet) | `T-27a-063` (private semantics and badge) | The sheet can show the badge once `T-27a-063` lands. |
| `T-27a-049` (overflow menu) | `T-27a-048` (sheet) | The sheet hosts the menu, so the menu cannot wait for the sheet. `T-27a-048` still depends on `T-27a-049`. |
| `T-27a-056` (search screen) | `T-27a-076` (selection rule) | `T-27a-076` is a pure rule that layers onto the screen. It still depends on `T-27a-056`. |
| `T-27b-010` (date dialog) | `T-27b-036` (west-of-UTC fix) | The fix is a change to the dialog, so it depends on the dialog. |
| `T-27b-027` (secret store) | `T-27b-024` (login outcome) | The store is the base. The login outcome uses it. |

After these edits, `python3 _tools/tasks.py lint` prints `ok` for both task files, and a depth-first search finds no back edge in the 123-task graph.

**Self-references kept.** Fourteen part 2 dependencies name `area:27-tracking-core`, meaning part 1. They are valid area ids and they pass lint, so they are kept. Replacing them with exact `T-27a-*` IDs is a worthwhile follow-up.

### 6.4 Other corrections made in this pass

- `p1-inventory.md`: six contract files (rows 5, 12, 36, 37, 38, 39) were tagged P0 in the inventory but P1 in the task file. They are now P1, and the summary line is updated. No task changed.
- `area:` tokens: every `area:` token in both task files names an existing folder under `/home/blackbox/kurobayomi/plans`. No token needed changing. The lint errors reported at `40-ui-browse-manga/p1-tasks.md` lines 37, 83, 158 and 537 (`area:41-ui-browse-manga-part-2`) are outside this area and were not touched.
- `TASKS.csv` was regenerated with `tasks.py export`.

### 6.5 Duplicated scope between part 1 and part 2 (to resolve before implementation)

Part 1 and part 2 both describe the same user-visible pieces. The overlap is the largest risk of double implementation. Each pair below needs one owner, and the other task should be marked as consuming it.

| Piece | Part 1 task(s) | Part 2 task(s) | Proposed owner |
|---|---|---|---|
| Status picker | `T-27a-051` | `T-27b-007` | part 2 host, part 1 list (`T-27a-051` supplies the list only) |
| Progress picker | `T-27a-052` | `T-27b-008` | as above |
| Score picker | `T-27a-053` | `T-27b-009`, `T-27b-035` | as above |
| Date picker and remover | `T-27a-054`, `T-27a-055` | `T-27b-010`, `T-27b-011`, `T-27b-036` | part 2 owns the dialog. Part 1 keeps only the shared conversion rule. |
| Remote search | `T-27a-056`, `T-27a-057`, `T-27a-058`, `T-27a-076` | `T-27b-012` | part 1 owns the widget, part 2 owns the page behaviour and cancellation (`T-27b-012` fixes D-6). |
| Remove tracking | `T-27a-059` | `T-27b-014` | part 2 owns the dialog. Part 1 owns the delete trait and its error contract. |
| Home sheet | `T-27a-048`, `T-27a-049`, `T-27a-050`, `T-27a-063` | `T-27b-003` to `T-27b-006`, `T-27b-015` to `T-27b-017` | part 1 owns the card layout, part 2 owns the row actions and the Home model. |
| Login | `T-27a-022`, `T-27a-023`, `T-27a-045`, `T-27a-046` | `T-27b-019` to `T-27b-024`, `T-27b-029` | part 2 owns the callback and login semantics. Part 1 keeps the credential and logout dialogs. |
| Token storage | `T-27a-020` | `T-27b-027`, `T-27b-028` | part 2 owns the semantics (D-F, D-G above). Part 1 owns the `SecretStore` binding. |
| PKCE | `T-27a-021` | `T-27b-025` | part 2 owns the verifier per attempt. Part 1 owns the URL builder. |
| Messages | `T-27a-078`, `T-27a-060` | `T-27b-030`, `T-27b-039` | one message catalogue, owned by `area:51-i18n`. The two lists must be merged. |
| Queue writes | `T-27a-080` | `T-27b-041` | one mutex per binding. Part 1's version is the reference. |
| Tests | `T-27a-065` | `T-27b-032` | one test crate, `crates/kuro-track/tests/`. |

---

## 7. Risks

1. **Tracker API drift.** Each tracker API is in another area, and the source has observed quirks (for example the AniList token regex that needs a trailing `&`, D18). Mitigation: fixtures recorded per tracker and a manual acceptance checklist (`p1-edge-cases-and-tests.md` section 11).
2. **Secret Service availability.** Under decision D-G, a machine without a Secret Service cannot log in. Mitigation: state the requirement in the login error and in the README.
3. **OAuth redirect on Linux.** The `aniyomi://` scheme needs the `.desktop` file installed and the URI to reach the running instance. The fragment must survive `gio::File::uri()` (`p2-topic-oauth-callback.md` section 3). **Unverified** until the manual test T-28 in `p2-topic-oauth-callback.md` section 11 passes on KDE and on GNOME.
4. **Client secrets are public.** Bangumi, Shikimori and Simkl secrets are in the public source (OQ-1). Those trackers may revoke the upstream app.
5. **Duplicated UI scope.** Section 6.5. If both parts are built, the pickers and login dialogs will exist twice.
6. **Queue correctness.** Replacing the SharedPreferences queue with SQLite (D-C) changes how existing data migrates. The queue is not in backups (`p1-behavior.md` section 21), so the migration can start with an empty queue. Decide whether to copy any existing entries first. Verify with the migration tests (`p1-edge-cases-and-tests.md` section 10).
7. **Track row identity.** Defect D1 means every write changes `_id`. The queue keys on `_id`. The upsert fix must land before the queue, or queued items point at stale rows.
8. **Date and timezone.** The picker and the stored value use different zones (D-10). Tests in `T-27b-036` cover five zones. The tests should cover a day around a daylight-saving change.
9. **No upstream tests.** Every test in both parts is new (`p2-edge-cases-and-tests.md` section 1, `p1-edge-cases-and-tests.md` introduction).
10. **Largest tasks.** `T-27a-022`, `T-27a-029`, `T-27a-048`, `T-27a-056` and `T-27b-010` are L. Split them if they exceed 1.5 weeks.

---

## 8. Definition of done (observable behaviour in Kurobayomi)

The area is done when each item below can be checked in the running app.

**Login**
- Settings > Tracking shows one row per tracker in registry order (`p1-behavior.md` section 1), with the username when logged in.
- Logging in opens the tracker's authorisation page in the default browser. After approval, the app returns to the foreground and shows "<name> logged in". The username and token are stored in the Secret Service and not in any file (`T-27b-027`).
- Denying approval shows "<name> login cancelled" and keeps the previous login (D-L).
- A callback with an unknown host shows an error and does not leave a spinner (D-1).
- A login that is not completed within 5 minutes is abandoned, and a callback for that attempt is rejected.
- Logout clears only that tracker's credentials and in-memory state (`T-27b-028`).
- A missing Secret Service shows "Secret storage is unavailable" and stores nothing (D-G).

**Binding and the entry sheet**
- The entry's Tracking sheet shows one card per logged-in tracker of the entry's kind, and an "Add tracking" card for each unbound tracker (`T-27a-048`).
- "Add tracking" searches the tracker by the entry title, and the selected result binds the entry. The start date and the current progress are sent, as in `AddAnimeTracks` (`p1-behavior.md` section 6).
- Enhanced trackers match automatically on add to library, when the entry's source is accepted (`T-27b-013`).
- The sheet refreshes each logged-in tracker on open. A failed refresh shows one toast per tracker (`T-27b-005`).

**Editing a binding**
- Status, progress, score, start date, finish date and private flag each write to the remote when confirmed. A failed remote write keeps the local value and shows a toast.
- Progress uses the range `0..total`, or `0..10000` when the total is unknown. A fractional progress is kept unless the user changes the number (D-7).
- The score picker starts at the stored score, including index 0 (D-9).
- Date pickers reject future dates. A start after a set finish is rejected, and the reverse. The stored value is local midnight. The picker day is correct in UTC, America/New_York, Asia/Kolkata, Pacific/Auckland and Europe/Berlin (D-10).
- Remove tracking deletes the local binding first. Remote deletion runs only for trackers that implement the deletable trait (MangaUpdates in the current registry). A remote failure is shown as a toast (D-13).

**Progress and the queue**
- Reading a chapter or watching an episode past the threshold sends progress to every logged-in tracker of the entry. The `AutoTrackState` policy is applied to mark-read prompts (`T-27a-039`).
- A failed send is saved in the SQLite queue, keyed by track id and kind. It survives an app restart.
- Retries run after 5, 10 and 20 and 40 minutes, with a maximum of four runs per kick. After the fourth failure the item is parked until the user opens the entry again, and then a notice is shown (`p1-topic-progress-queue.md` sections 6 and 12).
- A stale queue item, one whose episode or chapter is already at or past the value, is removed and not retried (D5).
- Auto-update is gated by the preferences for the reader and the player (`T-27a-040`).

**Settings and search**
- Search with a blank query sends nothing (D11). A slow earlier search cannot overwrite a newer result (D-6).
- Search results show a cover, the title and the summary. A tracker that does not set a title or tracking URL does not crash the search (D9).

**Data and logs**
- Track rows keep their `_id` across writes (D1), and the queue is keyed by track id (D2).
- No token, username or password appears in the logs or in the config file (`T-24` checklist, `p2-topic-oauth-callback.md` section 9). Full callback URIs are never logged, only scheme and host.
- Backup does not contain tokens. After a restore the user logs in again (`area:21`, `area:22`).

**Tests**
- Every defect D1 to D20 (part 1) and D-1 to D-17 (part 2) has a regression test, listed in the edge-case documents.
- `cargo test -p kuro-track` passes and covers the rule tests in `p1-edge-cases-and-tests.md` section 5 and the date and progress tables in `p2-edge-cases-and-tests.md` sections 3 and 4.
- The manual checklist in `p1-edge-cases-and-tests.md` section 11 is completed once on KDE Plasma with a real AniList or MyAnimeList account.

---

## 9. Verification performed for this overview

- Read the headings of all 17 part documents, the full part 1 inventory, the first part of the part 2 inventory, the rust design layout sections of both parts, the defect lists (D1 to D20 and D-1 to D-17), the cross-area sections, and the open-question sections of the three topic documents. Other sections were read only where a conflict or a cross-reference needed checking.
- Confirmed that every scope path appears in both inventories.
- Confirmed that every `area:` token in both task files names a real folder.
- Found and fixed seven dependency cycles and one further cycle in the task graph (section 6.3).
- Confirmed the task graph has no back edges after the fixes.
