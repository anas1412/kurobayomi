# Area 09 — Domain: source, track, download, release, storage, backup, custom buttons, extension repos and stores

Area lead overview. Paths are repo-relative to the Aniyomi clone at `/home/blackbox/aniyomi-src`. Every Kotlin claim in the area docs carries a `path:line` citation. Labels: **OBSERVED** = what the Kotlin does; **Proposal:** = what Kurobayomi should do.

## 1. Purpose and scope

**What this area is.** The Kotlin `domain` module code that sits between the UI and the data layer for:

- **Sources**: identity of a source (`Source`, `AnimeSource`), pins (bit flags), stub placeholders for uninstalled sources, browse routing (Popular / Latest / Search), non-library listings (counts per source, deletable anime, orphaned ids), and the remote paging contract.
- **Tracks**: per-entry tracker links (manga and anime), read/write interactors.
- **Download policy**: the 23 download preference keys, the auto-download filter for new chapters and episodes, remove-after-read slot math, and the speed and Wi-Fi values. The downloader execution itself (threads, files, queue) belongs to area:23-downloads-manga, area:24-downloads-anime and area:25-downloads-shared.
- **Extension repositories (manga)** and **extension stores (anime)**: where extension indexes come from, how they are added, refreshed, fetched and deleted.
- **Custom buttons**: user Lua-backed buttons in the anime player, with placeholder substitution and an ordered list.
- **Release check**: the "is there a newer app version" check against GitHub.
- **Storage**: the base directory preference and the folder tree (`downloads`, `local`, `localanime`, `autobackup`, `mpv-config/*`).
- **Backup preferences**: interval, last automatic backup time, default flags. The backup format and restore live in area:21-backup-format-and-create and area:22-backup-restore.

**Why it matters for a usable Kurobayomi.** The P0 subset is what lets a user see installed sources, see uninstalled-source placeholders without crashing, pick a browse mode, and have a download and storage configuration that the reader and downloader can read. Without the source registry and the storage tree, no source can be browsed and nothing can be saved to disk. Parity features (tracking, extension repos, auto-download, backup prefs) are P1.

**Scope size.** 73 Kotlin files, 1999 LOC, all in the Gradle module `domain` (packages `mihon.domain.*` and `tachiyomi.domain.*`). No UI files. The screens that consume this code belong to other areas (see [p1-topic-consumer-screens.md](p1-topic-consumer-screens.md)).

**Out of scope here.** Data-layer SQL (owned by area:05-data-repos-entries-items and area:06-data-repos-history-track-source-misc, restated only where the domain contract depends on it), the Kotlin source-api types (area:15-source-api-manga, area:16-source-api-anime), extension loading (area:18-extensions-manga, area:19-extensions-anime), and the downloader.

## 2. Module map

### 2.1 Kotlin packages to responsibilities

Each row points to its declaration list in [p1-inventory.md](p1-inventory.md).

| Kotlin package (under `domain/src/main/java/`) | Responsibility | Inventory section |
|---|---|---|
| `mihon/domain/extension/anime/` | Anime extension stores: add, refresh, fetch across stores, insert from preference, remove, and the store model and repository | [§1](p1-inventory.md) |
| `mihon/domain/extensionrepo/` (manga) | Manga extension repositories: add with URL validation, replace, refresh, delete, get, count, and the `repo.json` service | [§2](p1-inventory.md) |
| `mihon/domain/items/` | Auto-download filters for chapters and episodes (`FilterChaptersForDownload`, `FilterEpisodesForDownload`) | [§3](p1-inventory.md) |
| `mihon/domain/upcoming/` | Upcoming tab feeds: two interactors, status sets | [§4](p1-inventory.md) |
| `tachiyomi/domain/backup/service/` | `BackupPreferences`, `PreferenceValues` (flag tags) | [§5](p1-inventory.md) |
| `tachiyomi/domain/custombuttons/` | Custom button model, repository, create, update, delete, reorder, favourite toggle, read, exception types | [§6](p1-inventory.md) |
| `tachiyomi/domain/download/service/` | `DownloadPreferences` (all download keys) | [§7](p1-inventory.md) |
| `tachiyomi/domain/release/` | Release check interactor, `Release` model, `ReleaseService` | [§8](p1-inventory.md) |
| `tachiyomi/domain/source/anime/` | Anime source model, pins, stubs, listings, remote paging, repository and manager | [§9](p1-inventory.md) |
| `tachiyomi/domain/source/manga/` | Manga source model, pins, stubs, listings, remote paging, repository | [§10](p1-inventory.md) |
| `tachiyomi/domain/storage/service/` | `StorageManager`, `StoragePreferences` | [§11](p1-inventory.md) |
| `tachiyomi/domain/track/anime/` | Anime track model, read and write interactors, repository | [§12](p1-inventory.md) |
| `tachiyomi/domain/track/manga/` | Manga track model, read and write interactors, repository | [§13](p1-inventory.md) |

The Android build and manifest files for the `domain` module are in scope only to be dropped (T-09a-055).

### 2.2 Proposed Rust module map

Target: crate `kuro-domain` inside the Rust workspace. **No module in `kuro-domain` imports `gtk`, `glib`, `gio` or `adw`.** Platform paths come only from `storage::layout`, via the `directories` crate. Full signatures are in [p1-rust-design.md](p1-rust-design.md) sections 1 and 4.

```
crates/kuro-domain/src/
  error.rs          DomainError, RepoStoreError, CustomButtonError
  source/           model, pin (Pins(u8)), stub, browse (BrowseQuery), registry, repo, non_library, upcoming
  track/            model (MangaTrack, AnimeTrack), repo (TrackRepo per kind), service (log-and-default)
  download/         prefs (DownloadPrefs), filter (generic over EntryKind), policy (slot math, speed)
  extension_repo/   model, dto, service (repo.json), repo, create, refresh, write
  extension_store/  model, repo, wire (format sniffing), add / refresh / fetch / read
  custom_buttons/   model (placeholder substitution), repo, ops (mutex-guarded create/update/delete/reorder/favourite)
  release/          model, check (throttle + compare), github (latest release + asset choice)
  storage/          layout (folder names, derived paths), manager (watch channel, lazy creation)
  backup/           prefs (interval, last timestamp, flags), flags (BackupFlag tags)
  upcoming/         status sets
```

Sibling crates used, owned elsewhere: `kuro-prefs` (area:02-preferences-system), `kuro-db` (area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations), `kuro-net` (area:12-network-stack), `kuro-core` (area:01-foundation-app-bootstrap).

## 3. Reading guide

All files are in this folder. Read in the order shown.

1. [00-overview.md](00-overview.md) — this document: scope, module map, decisions, dependencies, task summary, definition of done.
2. [p1-inventory.md](p1-inventory.md) — every one of the 73 scope files, grouped by package, with its declarations and port decision.
3. [p1-behavior.md](p1-behavior.md) — the algorithms and contracts (OBSERVED with `file:line`, plus Proposals): source model, tracks, download filter, extension repos and stores, custom buttons, storage, release check.
4. [p1-data-and-config.md](p1-data-and-config.md) — models, field types, enum and bit layouts, the SQL fragments the domain depends on, all preference keys with defaults, on-disk folder tree, network endpoints, JSON field names, constants.
5. [p1-rust-design.md](p1-rust-design.md) — crate layout, shared types, error enums, traits and signatures, async model, Android-to-Linux mapping, crate list, mockup integration, pitfalls.
6. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) — quirk register (Q-…), edge cases (E-…), upstream tests to port and new tests (TC-…), fixtures, failure-mode matrix, race conditions.
7. [p1-tasks.md](p1-tasks.md) — the 61 implementation tasks T-09a-001 to T-09a-061 with priority, effort, dependencies, sources and acceptance criteria.
8. [p1-topic-source-model-pins-stubs.md](p1-topic-source-model-pins-stubs.md) — deep dive: source identity, pins, stub sources, browse routing.
9. [p1-topic-sources-pins-stubs.md](p1-topic-sources-pins-stubs.md) — deep dive: the same ground as item 8 from the enabled-source list, remote browsing and the 24 source-domain files. Overlaps item 8; see section 4 Q-OV-3.
10. [p1-topic-download-policy.md](p1-topic-download-policy.md) — deep dive: which new items auto-download, which old items are removed, throttling, network gating.
11. [p1-topic-extension-repos-and-stores.md](p1-topic-extension-repos-and-stores.md) — deep dive: manga repos and anime stores, add, refresh, fingerprint, wire formats, legacy migration.
12. [p1-topic-custom-buttons.md](p1-topic-custom-buttons.md) — deep dive: custom player buttons end to end, from the Lua snippets to the database rows.
13. [p1-topic-release-check.md](p1-topic-release-check.md) — deep dive: GitHub latest-release check, throttle, version comparison, the four Kotlin comparison defects.
14. [p1-topic-storage-layout.md](p1-topic-storage-layout.md) — deep dive: base directory, folder tree, `StorageManager` lifecycle.
15. [p1-topic-consumer-screens.md](p1-topic-consumer-screens.md) — which screens in other areas consume this code, and what the mockup lacks.

## 4. Key design decisions and open questions

Each item needs a call from the user. The recommendation is the default if the user says nothing.

| ID | Question | Options | Recommendation | Why |
|---|---|---|---|---|
| Q-OV-1 | Which GitHub repository does the release check query? | (a) Keep the Kotlin behaviour and query the upstream project's releases; (b) point it at the Kurobayomi repository; (c) disable the check | (b) with a settings switch that defaults on; if no Kurobayomi release exists, the check returns "no release" and does nothing | `ReleaseServiceImpl.kt:21` builds `https://api.github.com/repos/${arguments.repository}/releases/latest`. Querying Aniyomi's repo would announce the wrong app. |
| Q-OV-2 | Keep Aniyomi's on-disk folder names `local` (manga) and `localanime` (anime)? | (a) keep; (b) rename to parallel names | (a) keep | Q-STO-1. Users importing or sharing an Aniyomi folder keep working. Costs nothing. |
| Q-OV-3 | Two overlapping deep dives exist (`p1-topic-source-model-pins-stubs.md` and `p1-topic-sources-pins-stubs.md`). | (a) keep both; (b) merge into one | (b) at the next doc pass; this overview does not delete either | Same source files, two write-ups; a reader can get conflicting wording. |
| Q-OV-4 | Support the protobuf anime store body? | (a) defer until area:19-extensions-anime specifies the schema; (b) implement now | (a) defer | T-09a-031 returns `Err(Invalid(...))` for protobuf bodies until area:19 supplies the schema. |
| Q-OV-5 | Is there any Aniyomi install whose preference-based anime store entries must be imported? | (a) yes, keep T-09a-035; (b) no, drop T-09a-035 | (b) drop unless the user has such data | Kurobayomi is a new application. T-09a-035 is P2 and has no network. |
| Q-OV-6 | The `backupFlags` preference has no caller outside the domain module (Q-BK-2). Keep it? | (a) keep key for format parity, no UI; (b) drop | (a) keep the key; area:21-backup-format-and-create decides whether to show it | The flag tags are part of the backup format. Dropping the key would break format parity. |
| Q-OV-7 | Auto-download for anime episodes: same rules as manga chapters? | (a) same rules; (b) separate rules | (a) same rules, with the episode-specific names from [p1-behavior.md](p1-behavior.md) §5 | The Kotlin code is parallel (`FilterEpisodesForDownload`). Two rule sets would double the test matrix. |
| Q-OV-8 | Fingerprint change during a repo refresh: silent skip (Kotlin) or user-visible warning? | (a) silent; (b) warning | (b) warning, logged with `tracing::warn!` and surfaced by the UI | behavior 6.3 says the Kotlin ignores it silently. A user who sees a changed fingerprint should know. |
| Q-OV-9 | Custom buttons in the player: keep at P2 or raise? | (a) P2; (b) P1 | (a) P2 | They depend on the mpv Lua runtime (area:45-player-core, area:48-settings-player-and-scripts). Reading and writing buttons does not block daily reading or watching. |
| Q-OV-10 | SY data-saver flag (`isExcludedFromDataSaver`) | (a) drop; (b) keep | (a) drop | T-09a-056. The data saver is an image feature (area:34-images-covers-cache) and the flag comes from a fork. Loss: a source cannot be excluded from the image data saver. |
| Q-OV-11 | Extension repos and stores run third-party code. Trust model? | Owned by area:18-extensions-manga, area:19-extensions-anime and area:20-extension-strategy-redesign | Those areas decide; this area stores only the index URL and fingerprint | Recorded so that no one assumes the domain code validates extension code. |

Decisions already made by the area docs (not up for debate here, but reviewable):

- **Pins are bit flags**, with the Kotlin quirks made explicit. `contains(Unpinned)` is true in the Kotlin code (Q-PIN-1). `without` is AND-NOT, not XOR (Q-PIN-2). The Rust code adds `is_pinned()` for real checks (T-09a-003).
- **Browse modes are an enum** (`BrowseQuery`), not sentinel strings (Q-BR-1, T-09a-006).
- **Track upsert keeps the row id** (Q-TRK-2, T-09a-012).
- **Repo refresh uses a `JoinSet` with per-task results**, so one failing repo does not cancel its siblings (Q-REPO-2, T-09a-028).
- **Release versions use `semver`**, fixing four comparison defects (Q-REL-1 to Q-REL-4, T-09a-043).
- **Storage base directory encoding is path-safe** (T-09a-047).
- **The storage tree is created explicitly at startup** by `ensure_tree()`, because the Kotlin `drop(1)` skips the initial value (Q-STO-2, T-09a-048).

## 5. Cross-area dependencies

### 5.1 What this area needs from other areas

| Area | What this area needs | Tasks affected |
|---|---|---|
| area:01-foundation-app-bootstrap | Workspace skeleton, the runtime, `tracing` setup | T-09a-001 |
| area:02-preferences-system | Typed preference store `kuro-prefs` and the Download, Backup and Storage preference keys | T-09a-014, T-09a-047, T-09a-052 |
| area:03-db-manga-schema-migrations | Manga database handle, `extension_repos` and `manga_sync` tables | T-09a-013, T-09a-025, T-09a-058 |
| area:04-db-anime-schema-migrations | Anime database handle, `extension_store`, `custom_buttons`, `anime_sync`, stub tables | T-09a-005, T-09a-008, T-09a-034, T-09a-037, T-09a-041 |
| area:05-data-repos-entries-items | Non-library listing queries and upcoming data | T-09a-007, T-09a-009 |
| area:06-data-repos-history-track-source-misc | Repository implementations for tracks, sources, extension repos and stores, custom buttons | T-09a-004, T-09a-011, T-09a-060 |
| area:07-domain-entries-items | `Chapter.isRecognizedNumber`, `Episode.isRecognizedNumber`, chapter and episode queries used by the filters | T-09a-015, T-09a-016 |
| area:08-domain-library-category-history-updates | Category ids and `Category.UNCATEGORIZED_ID` used by the category include and exclude table; the `kuro-domain` base | T-09a-001, T-09a-018 |
| area:12-network-stack | HTTP client, GET, status check, JSON parsing, gzip sniffing support | T-09a-027, T-09a-031, T-09a-033, T-09a-042 |
| area:13-core-util-storage-archive-torrent | `PreferenceStore`, `FolderProvider`, `DiskUtil` (the last is dropped here, T-09a-050) | T-09a-047, T-09a-050 |
| area:17-source-app-layer | Real installed-source instances that feed `SourceRegistry` | T-09a-004 |
| area:18-extensions-manga and area:19-extensions-anime | Extension loading, protobuf store schema, trust model | T-09a-031, T-09a-011 |
| area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared | Download execution, Wi-Fi network check, slot throttling, CBZ writer | T-09a-014, T-09a-021, T-09a-023 |
| area:26-library-update-jobs | Calls the auto-download filters after discovering new items | T-09a-015, T-09a-016 |
| area:27-tracking-core | Tracker logic and the `manga_sync` / `anime_sync` update SQL | T-09a-010 to T-09a-013 |
| area:21-backup-format-and-create, area:22-backup-restore | Backup create and restore, and the flag UI | T-09a-052, T-09a-053 |
| area:45-player-core, area:48-settings-player-and-scripts | mpv Lua host that runs custom button snippets | T-09a-041 |
| area:34-images-covers-cache | Image data saver (the dropped SY flag) | T-09a-056 |

### 5.2 What other areas need from this area

- **Source identity and pins** (`Source`, `AnimeSource`, `Pins`) for area:37-ui-library, area:40-ui-browse-manga, area:41-ui-browse-anime, area:17-source-app-layer.
- **`SourceRegistry` and `StubSource`** for every screen that lists or opens a source. An uninstalled source must still render by name.
- **`BrowseQuery`** and the `PageSource` trait for area:40 and area:41 browse screens.
- **`DownloadPrefs` (23 keys) and the filter outputs** for area:23-downloads-manga, area:24-downloads-anime and area:26-library-update-jobs.
- **`StorageManager`** (base directory, `downloads`, `local`, `localanime`, `autobackup`, `mpv-config` paths, change notification) for every download, local-source and backup path.
- **Track models and `TrackRepo`** for area:27-tracking-core and the tracker areas 28 to 33.
- **Extension repo and store models and services** for area:18, area:19 and area:20.
- **Custom button model and placeholder substitution** for area:45-player-core and area:46-player-controls-ui.
- **Release check** for area:35-data-misc-updater-notification-export and the About screen in area:47-settings-screens-general.
- **`BackupPrefs` and flag tags** for area:21 and area:22.

## 6. Task summary

Source: [p1-tasks.md](p1-tasks.md), exported to `plans/TASKS.csv` with `python3 plans/_tools/tasks.py export`. This area has 61 rows (T-09a-001 to T-09a-061).

### 6.1 Counts

| Priority | Count | Effort S | Effort M | Effort L | Effort XL |
|---|---|---|---|---|---|
| P0 | 11 | 7 | 4 | 0 | 0 |
| P1 | 30 | 19 | 11 | 0 | 0 |
| P2 | 14 | 10 | 4 | 0 | 0 |
| P3 | 6 | 6 | 0 | 0 | 0 |
| **Total** | **61** | **42** | **19** | **0** | **0** |

Lint: `python3 plans/_tools/tasks.py lint` on [p1-tasks.md](p1-tasks.md) prints `ok`. The lint run found no errors. All `Depends:` area tokens name existing plan folders (checked in this pass).

### 6.2 P0 tasks

| ID | Title | Effort | Depends |
|---|---|---|---|
| T-09a-001 | kuro-domain module skeleton | S | area:01, area:08 |
| T-09a-002 | Source value model: visual name, key, last-used key | S | T-09a-001 |
| T-09a-004 | Source registry: installed sources, stubs, init state, listing traits | M | T-09a-002, area:06 |
| T-09a-005 | Stub sources: uninstalled-source placeholders and error behaviour | M | T-09a-002 |
| T-09a-006 | Browse routing as an enum (Popular, Latest, Search) | S | T-09a-004 |
| T-09a-060 | Remote page trait and paging contract for browse lists | M | T-09a-004, area:06 |
| T-09a-014 | Download preferences: the 23 keys with exact defaults | S | T-09a-001, area:02 |
| T-09a-021 | Download slots, speed limit and Wi-Fi-only values | S | T-09a-014 |
| T-09a-047 | Base storage directory preference, path-safe encoding | S | T-09a-001, area:02 |
| T-09a-048 | Storage tree: creation on change, lazy creation, startup check | M | T-09a-047 |
| T-09a-049 | Storage change notification (replaces the Kotlin shared flow) | S | T-09a-048 |

### 6.3 P0 critical path, in order

Three chains run in parallel after T-09a-001.

1. **Source chain:** T-09a-001 → T-09a-002 → T-09a-004 → then T-09a-005, T-09a-006 and T-09a-060 in parallel. Estimated 1 + 1 + 3 + 3 days at upper bounds for the first three in sequence; T-09a-005, T-09a-006 and T-09a-060 can overlap.
2. **Download-policy chain:** T-09a-001 → T-09a-014 → T-09a-021. Four days or fewer at upper bounds.
3. **Storage chain:** T-09a-001 → T-09a-047 → T-09a-048 → T-09a-049. Five days or fewer at upper bounds.

The longest single path is the source chain. Its end date sets the earliest date at which Kurobayomi can list and open sources through the domain layer. Real network sources still need area:17 to area:19 before a chapter can be read.

Note: T-09a-003 (pins) is P1 but the "Pinned" section of the source list depends on it for real data. Schedule it right after T-09a-004 if the source list ships in the first release.

### 6.4 Risks

1. **The P0 contract is not an end-to-end source.** T-09a-004 gives a registry, but a working source requires area:17-source-app-layer, area:18-extensions-manga and area:19-extensions-anime, plus network from area:12. Plan the first end-to-end milestone with those areas, not with this one alone.
2. **Release check target (Q-OV-1).** Left as-is, it would look up the upstream project. Must be decided before T-09a-042 is built.
3. **Concurrency semantics change.** Kotlin coroutine cancellation (sibling cancellation in `coroutineScope`, `withNonCancellableContext`) maps to `JoinSet` and spawned tasks. A port that copies the structure without the per-task error handling reproduces Q-REPO-2. Tests TC-… in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) §4 cover this.
4. **Quirks preserved vs fixed.** Some quirks are kept for parity (Q-DL-1 unread-only by chapter number, Q-CB-1 substring placeholder, Q-REL-5 no `lastChecked` on null release, Q-STO-1 folder names). Others are fixed (Q-PIN-2, Q-BR-1, Q-REL-1 to Q-REL-4). A reviewer must check each against the quirk register.
5. **Protobuf store schema is missing.** Anime stores that serve protobuf bodies fail until area:19 supplies the schema (Q-OV-4).
6. **Duplicate deep dives.** Two topic files overlap (Q-OV-3). Readers may get conflicting wording until one is removed.
7. **Third-party code.** Extension repos and stores ship code that runs on the user's machine. This area does not verify it (Q-OV-11).
8. **Android-only drops lose behaviour.** SAF (`UniFile.fromUri`), NoMedia marker, external downloader intent, Serializable and the SY data-saver flag are dropped (T-09a-050, T-09a-022, T-09a-054, T-09a-056). Users who depended on SAF folders (for example an SD card through the Android storage framework) have no Linux equivalent here; they must use a local path.

## 7. Definition of done for this area in Kurobayomi

Each item is an observable behaviour in the running app or a test that the area owns.

1. **Sources list.** The source list shows sections "Last used", "Pinned", language groups and the local source row, with real `Source` values from `SourceRegistry`, not the `Source A` placeholders in `src/entry.rs:283-300`. Last-used and pinned come from `Pins` and the used-last key.
2. **Uninstalled source.** An uninstalled source still appears in the list with its name and language as `"{name} ({LANG})"`, or its numeric id if both are blank. Browsing it shows a `SourceNotInstalled` error, not a crash (T-09a-005; TC-SRC-03, TC-SRC-04).
3. **Browse modes.** Popular, Latest and Search are selected by `BrowseQuery`. Typing the old sentinel text searches for that text; it does not switch modes (Q-BR-1).
4. **Source counts.** The migrate-source screen shows a count per source from the non-library listing (T-09a-007).
5. **Download settings.** The downloads settings screen reads and writes the 23 keys with the defaults in [p1-data-and-config.md](p1-data-and-config.md) §4.1. A fresh install shows the defaults. The Wi-Fi-only switch defaults to on.
6. **Download slots and speed.** The configured number of concurrent download slots and the speed limit are applied by the downloader. The Wi-Fi-only check uses a `gio::NetworkMonitor` query and is verified with a fake monitor in tests.
7. **Auto-download.** Enabling auto-download for a category downloads new unread chapters and episodes in that category only, using the include and exclude table in [p1-behavior.md](p1-behavior.md) §5.2. Q-DL-1 and Q-DL-2 are handled as stated in the tasks.
8. **Remove-after-read.** After a chapter or episode is read, it is removed according to the slot arithmetic in T-09a-019, and never removes a bookmarked or excluded-category item.
9. **Tracks.** Adding, updating and removing a tracker link on a manga or anime entry persists the link. Re-adding a link keeps the row id (T-09a-012). A list read error shows "no tracks" only when the log-and-default policy says so, and the error is logged.
10. **Extension repos (manga).** Adding a repo URL validates it, classifies a duplicate as `RepoAlreadyExists`, saves the fingerprint, and refreshes the repo. A failing repo does not stop the other repos from refreshing. A changed fingerprint produces a visible warning (Q-OV-8).
11. **Extension stores (anime).** Adding a store accepts the JSON wire formats; protobuf reports the deferred error until area:19 lands. Refresh runs stores in order with per-store error logging, and aggregated extension lists come from all stores in parallel with failure isolation.
12. **Custom buttons.** Creating, editing, deleting, reordering and favouriting buttons persists them in the `custom_buttons` table, with unique sort indexes after deletion and no duplicate ids after concurrent creates (Q-CB-2). The player receives the rendered `on_startup` snippet with placeholders replaced as in [p1-behavior.md](p1-behavior.md) §8.1.
13. **Release check.** With the check enabled, the app queries the configured repository at most once every three days (`Limit checks to once every 3 days at most`, `GetApplicationRelease.kt:22`), compares versions with `semver` (for example `1.0.0` is newer than `0.9.9`, and `1.2.1` is newer than `1.2`), and shows a notice only when a newer version exists.
14. **Storage.** The base directory can be changed. The change creates `autobackup`, `downloads`, `local`, `localanime` and `mpv-config/{fonts,scripts,script-opts,shaders}` at once. Startup creates the tree explicitly. The change is announced to all subscribers through a `watch` channel. An unreachable base directory produces `StorageUnavailable`, not a silent `null` (Q-STO-3).
15. **Backup preferences.** The backup interval, last automatic backup time and default flags persist and round-trip through the backup area's create and restore flows (area:21, area:22).
16. **Dropped features are visibly absent.** External downloader settings, the SY data-saver flag and the Android storage (SAF) picker do not appear. Each dropped feature has a line in the task that dropped it, stating the loss.
17. **Tests.** The area's tests (TC-… in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) §4) pass on an in-memory database, a fake HTTP server and temporary directories (T-09a-058). The release-check cases from `GetApplicationReleaseTest` are ported (T-09a-057).
