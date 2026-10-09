# Area 06 — Data layer repositories: overview

Area id: `06-data-repos-history-track-source-misc`. Lead document for: history, track, updates, sources, releases, custom buttons, handlers, extension repos (anime stores and manga repos).

Scope: 42 Kotlin files, 2,907 LOC (`python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 06-data-repos-history-track-source-misc 1`). All source paths are repo-relative to `/home/blackbox/aniyomi-src`. Plan paths are relative to this folder.

OBSERVED = what the Kotlin does. Proposal = what Kurobayomi should do. Port decisions use the vocabulary Port, Rewrite, Replace-with-crate, Drop.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does in this area

Area 06 is the repository layer between the SQLDelight-generated SQL and the domain interactors. It owns:

1. **Database handlers.** `AndroidMangaDatabaseHandler` and `AndroidAnimeDatabaseHandler` are the only way any repository touches SQL. They run blocks on `Dispatchers.IO`, take over a thread for transactions, and expose reactive queries. (`data/src/main/java/tachiyomi/data/handlers/{manga,anime}/`)
2. **History.** Manga read history with accumulated read time (`time_read`), anime episode history without duration. Covers the list view (one row per series, most recent chapter), search, reset, delete-all and the "latest" lookup behind the continue-reading button.
3. **Updates.** The recent-chapters and recent-episodes feed of favourited entries: `date_fetch > date_added`, `date_upload > now - 3 months`, limit 500.
4. **Tracks.** Tracker bindings per entry and tracker (`manga_sync`, `anime_sync`), with insert-as-replace semantics and a coalesced update used by backup restore.
5. **Sources.** Source list flows, favourite counts per source, the persistent stub table that keeps names of uninstalled sources, and the remote paging algorithm used by Browse (popular, latest, search).
6. **Extension stores (anime) and repos (manga).** Index documents in JSON, protobuf or legacy form, gzip-compressed or not; persistence; refresh; the manga signing-key fingerprint rules.
7. **Player custom buttons.** User Lua snippets for mpv, stored in the anime DB, seeded with one "+85 s" skip button.
8. **Release check.** Fetch of the latest GitHub release, @mention linkification, download asset selection.

### 1.2 Why it matters for a usable Kurobayomi

* **P0 (daily use).** Without the DB core (T-06a-001..005), no other area can persist. Without history (T-06a-010..014) the reader and player cannot resume, and the History and Updates tabs stay empty. Without source repos and `RemotePager` (T-06a-040..043) Browse cannot list titles. Without the stub table, sources of removed extensions have no name.
* **P1 (parity most users rely on).** Track storage, extension stores and repos (how extensions are discovered), custom buttons, restore-support helpers.
* **P2 (long tail).** Release check, by-status update queries.
* **P3 (Android-only, dropped or replaced).** Coroutine transaction plumbing, Paging3 query source, ABI selection, in-app APK install, preview channel.

### 1.3 Not in this area

| Concern | Owner |
|---|---|
| Table definitions, views, seeds, `.sqm` migrations | area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations |
| Domain models and interactors (`MangaHistory`, `GetNextChapters`, `GetApplicationRelease`, `Pins`, `Source`) | area:07-domain-entries-items, area:08-domain-library-category-history-updates, area:09-domain-source-track-download-release-misc |
| Tracker HTTP APIs | area:27-tracking-core and area:28-tracker-anilist through area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin |
| Source objects and `SourceManager` | area:15-source-api-manga, area:16-source-api-anime, area:17-source-app-layer |
| Extension install and loading on Linux | area:18-extensions-manga, area:19-extensions-anime, area:20-extension-strategy-redesign |
| Screens that render this data | area:40-ui-browse-manga, area:41-ui-browse-anime, area:42-ui-updates-history-stats-storage-upcoming, area:47-settings-screens-general, area:48-settings-player-and-scripts, area:46-player-controls-ui |

---

## 2. Module map

### 2.1 Kotlin packages and files to responsibilities

Counts are from the inventory (`p1-inventory.md`). Groups sum to 42 files and 2,907 LOC.

| Kotlin location (repo-relative prefix) | Files | LOC | Responsibility | Port decision and tasks |
|---|---|---|---|---|
| `data/src/main/java/tachiyomi/data/handlers/manga/`, `.../handlers/anime/` | 8 | 781 | SQLDelight facade (`await`, `awaitList`, `awaitOne*`, `subscribeTo*`), transaction thread takeover, Paging3 query source | Rewrite as `kuro_core::db::Db` (T-06a-001..003). Transaction context and `QueryPaging*Source` Drop (T-06a-006, T-06a-007) |
| `data/src/main/java/tachiyomi/data/history/{manga,anime}/` | 4 | 244 | Row mappers and history repositories | Port (T-06a-010..014) |
| `data/src/main/java/tachiyomi/data/updates/{manga,anime}/` | 2 | 158 | Updates feed repositories | Port (T-06a-020, T-06a-021) |
| `data/src/main/java/tachiyomi/data/track/{manga,anime}/` | 4 | 218 | Track mappers and repositories | Port (T-06a-030..032) |
| `data/src/main/java/tachiyomi/data/source/{manga,anime}/` | 6 | 378 | Source flows, stub repositories, paging sources | Rewrite as `RemotePager`, port the rest (T-06a-040..044) |
| `data/src/main/java/mihon/data/extension/anime/{model,repository,service}/` | 6 | 475 | Anime store wire models (JSON and protobuf), legacy repo and index models, persistence, fetch algorithm | Port (T-06a-050..054) |
| `data/src/main/java/mihon/data/repository/manga/` | 1 | 93 | Manga extension repos table | Port (T-06a-055, T-06a-056) |
| `data/src/main/java/tachiyomi/data/custombutton/` | 1 | 84 | Custom button table | Port (T-06a-060..062) |
| `data/src/main/java/tachiyomi/data/release/` | 2 | 90 | GitHub latest-release DTOs and service | Rewrite (T-06a-070..073) |
| `app/src/main/java/eu/kanade/tachiyomi/data/database/models/{manga,anime}/` | 8 | 386 | Legacy mutable structs (`*Track`, `*TrackImpl`, `Chapter`, `ChapterImpl`, `Episode`, `EpisodeImpl`) | Replaced by domain structs and `EpisodeKey`/`ChapterKey` (T-06a-033, T-06a-034) |

Non-scope files read to make this plan exact are listed in `p1-inventory.md` section 2. The most important are `data/src/main/sqldelight/data/history.sq`, `data/src/main/sqldelight/view/updatesView.sq`, `data/src/main/sqldelightanime/dataanime/custom_buttons.sq`, `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt` (lines 71-155, PRAGMAs and handler construction) and `app/src/main/java/mihon/core/migration/migrations/TrustExtensionRepositoryMigration.kt`.

### 2.2 Rust module map (Proposal)

Crate `kuro-core` is GTK-free. The GTK binary is the existing `kurobayomi` crate. Source of truth is `p1-rust-design.md` section 1.

```text
kuro-core/src/
  db/                    Db, Schema, DbError, TableSet, Subscription        T-06a-001..005
    notify.rs            update_hook dirty set and broadcast                T-06a-003
    testkit.rs           open_temp, fixtures (cfg(test) or feature)         T-06a-005
  data/
    mod.rs               pub use crate::db as db;                           (area:05 path)
    history/             manga.rs, anime.rs, recorder.rs                    T-06a-010..014
    updates/             manga.rs, anime.rs                                 T-06a-020, 021
    track/               manga.rs, anime.rs                                 T-06a-030..033
    legacy.rs            ChapterKey, EpisodeKey                             T-06a-034
    source/              manga.rs, anime.rs, stub.rs, pager.rs              T-06a-040..044
    extension/
      store/             wire.rs, mapping.rs, service.rs, repo.rs           T-06a-050..054
      repo/              mod.rs, service.rs, interactors.rs                 T-06a-055, 056
    custom_button/       mod.rs, seed.rs, script.rs                         T-06a-060..062
  release/               mod.rs, github.rs, mentions.rs                     T-06a-070..073
kurobayomi/src/
  data_bridge.rs         owns the AppContext handle; bind_list; spawn_ui    T-06a-080, 081
  screens.rs             history(), updates(), extension screens            T-06a-057, 080
```

Shared types owned elsewhere: `MangaCover` and `AnimeCover` (area:07), `CatalogueSource` and `SourceManager` (area:15-area:17), `HttpClient` (area:12), `Notifier`, `Opener`, `Paths`, `Toast` (kuro-platform and the GTK layer, area:01 and area:36), `Settings` (area:02).

### 2.3 Crates and Android replacements

| Android or Kotlin facility | Rust or Linux replacement | Note |
|---|---|---|
| SQLDelight + `AndroidSqliteDriver` + Requery SQLite | `rusqlite` with `bundled`, `hooks`, `functions` | Pins SQLite >= 3.24 for `ON CONFLICT ... DO UPDATE` |
| Pool of reader connections (implicit in Android driver) | `r2d2` + `r2d2_sqlite`, `max_size = 4`, `min_idle = Some(1)` | Synchronous API inside `spawn_blocking` |
| `Mutex` in transaction machinery | `parking_lot::Mutex<Connection>` | No poisoning |
| Coroutines (`Dispatchers.IO`, `withContext`, `Flow`, `StateFlow`, `combine`) | `tokio::task::spawn_blocking`, `tokio::sync::{watch, broadcast}`, `futures::Stream`, a hand-written `combine_latest` | |
| AndroidX Paging3 | `RemotePager` plus a 25-item prefetch rule in the UI | |
| `java.util.Date` | `i64` epoch milliseconds | |
| kotlinx.serialization JSON, ProtoBuf, okio gzip | `serde_json`, `prost` (derive only, no `protoc`), `flate2` | |
| OkHttp `HttpUrl` | `url::Url` | |
| OkHttp client and `awaitSuccess` | `reqwest` through area:12 `HttpClient` | |
| Regex with lookaround (`@` mention pattern) | `fancy-regex` | The `regex` crate has no lookaround |
| logcat | `tracing` | |
| Injekt singletons | `AppContext` fields (area:01) | |
| `Build.SUPPORTED_ABIS` | `std::env::consts::ARCH` with asset tags `x86_64`, `aarch64` | |
| `MPVLib.command` | libmpv `mpv_command` through the area:45 wrapper | |

---

## 3. Reading guide

Read in this order: the inventory, then the behaviour and data files, then the Rust design, then the tasks.

| Document | One-line description |
|---|---|
| [p1-inventory.md](p1-inventory.md) | One row per scope file: LOC, declarations, port decision, target Rust module, priority, tasks; plus the non-scope files read to make the plan exact |
| [p1-behavior.md](p1-behavior.md) | OBSERVED behaviour of every repository, handler, transaction, paging algorithm, store flow, repo interactor, custom button, release call and dispatcher; message key index |
| [p1-data-and-config.md](p1-data-and-config.md) | Domain models with field types, wire field tags and JSON keys, the legacy-to-domain mapping tables, preference keys, verbatim SQL, network endpoints, on-disk paths |
| [p1-rust-design.md](p1-rust-design.md) | Rust module layout, dependencies, API surface per repository, threading model, error policy, SQL rules, UI wiring into the mockup, Android API mapping, pitfalls |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge-case tables D1-D14, H1-H12, U1-U7, T1-T7, S1-S9, E1-E29, C1-C11, R1-R6; race list; fixtures; test names per module; upstream test mirror; manual checklist |
| [p1-tasks.md](p1-tasks.md) | 41 implementation tasks, T-06a-001 to T-06a-081, in the plan task format |
| [p1-topic-db-handle-and-reactive-queries.md](p1-topic-db-handle-and-reactive-queries.md) | Replacement for the SQLDelight handlers: `Db` API, single IMMEDIATE writer, reader pool, `update_hook` change notification, subscription lifetime |
| [p1-topic-history-and-updates-views.md](p1-topic-history-and-updates-views.md) | How `historyView` picks one row per series, `ORDER BY` decisions, search semantics, upsert worked example, updates window and ordering |
| [p1-topic-extension-store-index.md](p1-topic-extension-store-index.md) | Four index document shapes, `fetch` and `getExtensions` algorithms, store persistence, manga repo rules, screen state machines, deep links |
| [p1-topic-source-paging-and-stubs.md](p1-topic-source-paging-and-stubs.md) | Source contracts this area calls, source list flows, stub table and manager behaviour, `RemotePager` algorithm, error-to-text mapping |
| [p1-topic-custom-buttons.md](p1-topic-custom-buttons.md) | Table, byte-exact seeded button text, placeholder expansion, Lua script assembly and its defects, interactor semantics, restore rules, security note |
| [p1-topic-release-check.md](p1-topic-release-check.md) | GitHub latest-release flow, mention regex, asset selection, what is dropped on Linux, version-comparison defects |

---

## 4. Key design decisions and open questions

### 4.1 Decisions already in the plan

These are Proposals that the plan treats as accepted unless the user objects.

| # | Decision | Where | Reason |
|---|---|---|---|
| 1 | One concrete `Db` struct, no repository or handler trait | `p1-topic-db-handle-and-reactive-queries.md` section 2 | Exactly one implementation exists; tests use real temp-file databases |
| 2 | One writer connection, every write is one IMMEDIATE transaction on a blocking thread; reads use an r2d2 reader pool | Topic db section 3 | Reads never wait for a long library-update write (WAL) |
| 3 | Repository logic is plain `fn(&Connection)`; async wrappers call it | Topic db section 3 | Two repositories compose in one transaction without a context element |
| 4 | Change notification from SQLite `update_hook`, published after commit, coalesced | Topic db section 4 | One mechanism covers cascades without per-repository table lists |
| 5 | `DELETE FROM <table>` is always written with `WHERE 1` | Topic db section 4 item 3 | Avoids the truncate optimisation, which fires no hook |
| 6 | Track insert keeps `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`, so the row id changes on re-insert | `p1-behavior.md` section 4.2 | Parity; the schema is owned by area:03/04 |
| 7 | Timestamps are `i64` milliseconds; no `chrono` type crosses a repository signature (see 4.3 item 2 for the one exception) | Topic db section 5, `p1-rust-design.md` section 8 | Tests pass explicit values |
| 8 | Protobuf and JSON store models share one struct set; protobuf tags and JSON keys identical to Kotlin | `p1-topic-extension-store-index.md` section 2 | Compatibility with existing index documents |
| 9 | Proto3 presence semantics in the store wire models; a missing `contact` fails the store; a missing `resources` skips one extension | Topic extension store section 2 | Third-party encoders omit defaults |
| 10 | Store fetch follows at most 4 requests for `index_v2` chains | Topic extension store section 3 | Kotlin recurses without limit |
| 11 | Concurrency cap 8 for store and source fan-out | `p1-behavior.md` section 11 | Kotlin is unbounded |
| 12 | Each custom button is written to its own Lua file | Topic custom buttons section 4 | One broken script does not disable the others |
| 13 | Release check: stable channel only, arch-tagged assets, no in-app install | Topic release check section 4 | Linux has no APK installer or preview build type |

### 4.2 Open questions for the user

Each question has a recommendation. Plan tasks are written for the recommended option; a different answer changes only the listed tasks.

| # | Question | Recommendation | Tasks affected |
|---|---|---|---|
| Q1 | Keep the Kotlin search semantics (literal parity: `LIKE` with `%` and `_` as wildcards, ASCII-only case folding) or improve them? | Improve: `kuro_fold` plus escaping, switched off with env `HISTORY_SEARCH_PARITY=1` | T-06a-012 |
| Q2 | When a store refresh returns a different `index_url` (a legacy `repo.json` that gained an `index_v2`), keep both rows (Kotlin) or replace the old row? | Replace the old row inside one transaction | T-06a-053 |
| Q3 | Keep the anime-store and manga-repo split with their two wire formats, or unify into one format? | Keep both, since existing index documents must keep working | T-06a-050, T-06a-055 |
| Q4 | Keep the release check at all? | Keep it, informational only, enabled when a build-time `UPDATE_REPOSITORY` constant is set, 3-day throttle | T-06a-070, T-06a-073 |
| Q5 | Confirm dropping the in-app APK download and the preview channel | Confirm. Distro packages are the update path | T-06a-073 |
| Q6 | Restore of custom buttons from a backup runs arbitrary Lua inside mpv. Confirm before restore? | Yes. List the button names in a confirmation when a restore adds buttons | T-06a-061, area:22 |
| Q7 | Anime history has no duration column in Kotlin. Add one now? | No (parity). Adding it needs a schema change in area:04 | T-06a-011 |
| Q8 | Keep `date_upload` as the Updates window column (chapters with `date_upload = 0` never show)? | Keep (parity) | T-06a-020 |
| Q9 | Store and source names that are blank for uninstalled sources: show the numeric id (Kotlin) or a placeholder? | Show the numeric id, as `StubMangaSource.toString()` does | T-06a-041, area:40, area:41 |

### 4.3 Lead review: inconsistencies and gaps found in the area documents

These were found while writing this overview. They are not fixed in the topic files, to keep this pass limited to the overview and task linting. Each needs an edit by the area author or a decision from the user.

1. **LOC mismatch.** `p1-topic-db-handle-and-reactive-queries.md` line 3 says the handler scope is "8 files, 1,157 LOC". The inventory gives 781 LOC (manga 390, anime 391). The 781 figure is correct.
2. **chrono contradiction.** `p1-rust-design.md` section 2 says "No `chrono` in repositories", but section 3.3 declares `pub fn three_months_ago_ms(now: chrono::DateTime<chrono::Local>) -> i64`. Recommendation: keep `chrono` as a `kuro-core` dependency used only by this function (local-calendar month arithmetic needs it), and amend the sentence in section 2.
3. **Verbatim SQL typo.** `p1-data-and-config.md` section 3.5 quotes `signing_key =: signingKey` from `data/src/main/sqldelightanime/dataanime/extension_store.sq:35`. The quote is correct as a citation. The Rust SQL must use `signing_key = ?` and must not copy the typo.
4. **Unverified SQLite behaviour.** The design relies on `update_hook` firing for rows deleted by `ON DELETE CASCADE` (edge case D8, test `cascade_notifies_child_table`). The SQLite documentation does not state this for foreign-key actions explicitly. T-06a-003 must run this test first. If it fails, the fallback is an explicit table list per repository write.
5. **Pending area folders.** The task file references area ids whose folders do not exist yet (see section 6.4). The ids are correct per `areas.py`.

### 4.4 Kotlin defects deliberately not carried over

| Kotlin behaviour | Rust behaviour | Where |
|---|---|---|
| Unbounded recursion on a self-referencing `index_v2` | At most 4 requests, then `StoreError::TooManyIndexHops` | Topic extension store section 3 |
| `getTrackByMangaId` takes a track row id (misnamed) | Named `by_id`, documented as a row-id lookup | `p1-data-and-config.md` section 1.3; T-06a-030 |
| `null as CatalogueSource` throws NPE for an uninstalled manga source | `PagerError::SourceNotInstalled` | Topic source paging section 5 |
| `runBlocking` inside the stub manager's `getOrStub` | Stub table preloaded into memory; lookup is a map read | Topic source paging section 3 |
| Version compare returns true when any component is greater; `IndexOutOfBounds` on a shorter tag | Component-wise compare, zero-padded (area:09 owns the code) | Topic release check section 5 |
| `GithubRelease.body` null fails to decode | `#[serde(default)]` | Topic release check section 1 |
| Concatenated Lua blocks with no separator | Blocks joined with `\n`; one file per button | Topic custom buttons section 4 |
| `refreshAll` writes a changed signing key silently | Logs WARN on a changed `signing_key` (manga repos keep rejecting changes) | Topic extension store section 5 |
| `replaceRepo` crashes on a third repo's `base_url` | `RepoError`, logged, toast `internal_error` | `p1-edge-cases-and-tests.md` E25 |
| Unbounded fan-out in store and source fetch | Cap 8 | `p1-behavior.md` section 11 |

---

## 5. Cross-area dependencies

### 5.1 What area:06 needs from other areas

| Area | What area:06 needs | Used by |
|---|---|---|
| area:01-foundation-app-bootstrap | `AppContext` owns the two `Db` handles and the tokio runtime; `Paths` gives the data and config directories; startup opens both databases off the GTK thread; shutdown awaits outstanding history writes | T-06a-001, T-06a-014 |
| area:02-preferences-system | Keys `__APP_STATE_incognito_mode`, `last_ext_check`, `__APP_STATE_last_app_check`, `__APP_STATE_library_update_last_timestamp` and the search-parity flag | T-06a-014, T-06a-053, T-06a-070 |
| area:03-db-manga-schema-migrations | `Schema` implementation, `history`, `manga_sync`, `sources`, `extension_repos`, `mangas`, `chapters`, views `historyView` and `updatesView`; `user_version` handling | T-06a-001, T-06a-005, T-06a-010, T-06a-020, T-06a-030, T-06a-041, T-06a-055 |
| area:04-db-anime-schema-migrations | Same for the anime DB: `animehistory`, `anime_sync`, `animesources`, `extension_store`, `custom_buttons` with seed row, views `animehistoryView` and `animeupdatesView`; migrations 128, 129 and 137 | T-06a-001, T-06a-011, T-06a-031, T-06a-053, T-06a-060 |
| area:05-data-repos-entries-items | Shares the `Db` type through `kuro_core::data::db`; chapter and episode deletes cascade into history | T-06a-001, T-06a-003 |
| area:07-domain-entries-items | `MangaCover`, `AnimeCover`, `Chapter` and `Episode` domain structs; chapter number rules (`f32` widened to `f64`) | T-06a-010, T-06a-011, T-06a-034 |
| area:08-domain-library-category-history-updates | `GetNextChapters` and `GetNextEpisodes` consume the latest-history repository | T-06a-012 |
| area:09-domain-source-track-download-release-misc | `GetApplicationRelease` (3-day throttle, version compare), `Pins`, `Source` structs, `StubSource` contract | T-06a-070, T-06a-043 |
| area:10-app-domain-layer | `networkToLocalManga` de-duplication; `date_added` and `date_fetch` writers (`UpdateManga`, `SyncChaptersWithSource`) | T-06a-020, T-06a-081 |
| area:11-app-migrations | Migration runner for `TrustExtensionRepositoryMigration` (version 7f) and the preference import | T-06a-054 |
| area:12-network-stack | `HttpClient` with `get_bytes`, `awaitSuccess` equivalent, `HttpException`, DNS and offline classification, `NetworkMonitor` | T-06a-042, T-06a-051, T-06a-056, T-06a-070 |
| area:15-source-api-manga, area:16-source-api-anime | `CatalogueSource` and `AnimeSource` traits, `ItemsPage`, `SourceError`, `FilterList` | T-06a-040, T-06a-042 |
| area:17-source-app-layer | `SourceManager` with `catalogue_sources()`, `get`, `get_or_stub`; stub registration and source rename | T-06a-040, T-06a-041 |
| area:18-extensions-manga, area:19-extensions-anime, area:20-extension-strategy-redesign | `AnimeExtension.Available` shape (`p1-data-and-config.md` section 1.5); meaning of `apk_url` as an artifact URL; the extension manager that calls `findAvailableExtensions` | T-06a-052, T-06a-053 |
| area:21-backup-format-and-create, area:22-backup-restore | Backup creators read the repositories; restorers call `insert_raw`, `update_coalesced`, `remove_resetted`; `Db::notify_all()` after a file swap | T-06a-013, T-06a-032, T-06a-061 |
| area:23-downloads-manga, area:24-downloads-anime | Download folder rename when a stub source's name or language changes | T-06a-041 |
| area:26-library-update-jobs | Writes `chapters.date_fetch` (Updates) and uses `write_blocking` plus the `fn(&Connection)` repository functions | T-06a-020, T-06a-003 |
| area:27-tracking-core | Mutates track structs through `copy_personal_from`; persists through the track repositories | T-06a-030, T-06a-033 |
| area:35-data-misc-updater-notification-export | Notifier for the update prompt; the APK download job is dropped | T-06a-073 |
| area:36-ui-shell-navigation-security-webview | `Toast` helper for `internal_error` and error text; `Opener` for "Download" and "Open on GitHub" | T-06a-073, T-06a-057 |
| area:45-player-core | libmpv `mpv_command` wrapper; `aniyomi.lua` property protocol; `load-script` | T-06a-062 |
| area:53-platform-manifest-intents-workers | URL scheme handlers (`aniyomi://`, `tachiyomi://`) and the single-instance forwarder | T-06a-057 |

### 5.2 What other areas need from area:06

| Consumer | What it uses from area:06 |
|---|---|
| area:42-ui-updates-history-stats-storage-upcoming | `MangaHistoryRepo::watch`, `AnimeHistoryRepo::watch`, `MangaUpdatesRepo::watch_recent`, `AnimeUpdatesRepo::watch_recent`, `reset_by_*`, `delete_all`, `total_read_duration` |
| area:40-ui-browse-manga, area:41-ui-browse-anime | `SourceRepo` flows, `pager(source_id, listing)`, `RemotePager::refresh`, `append`, `has_next`, `sources_with_favorite_count` |
| area:43-reader-core, area:44-reader-viewers | `HistoryRecorder::start` and `flush` |
| area:45-player-core | `HistoryRecorder` for episode progress; `build_player_scripts`, `expand_placeholders`, the favourite button lookup |
| area:46-player-controls-ui | `CustomButtonRepo::watch_all`, the favourite button |
| area:47-settings-screens-general | Extension store and repo screens; `sources_with_non_library_manga` for the clear-database screen |
| area:48-settings-player-and-scripts | `CustomButtonRepo` create, update, delete, reorder, toggle favourite |
| area:27-tracking-core | Track repositories (`by_id`, `by_manga_id`, `insert`, `delete`, watch) |
| area:18-extensions-manga, area:19-extensions-anime | Store and repo rows; `ExtensionStoreRepo::fetch_extensions`; `ExtensionRepoRepo::all` |
| area:21-backup-format-and-create, area:22-backup-restore | Repositories, `insert_raw`, `update_coalesced`, `remove_resetted`, `by_chapter_url`, `by_episode_url` |
| area:26-library-update-jobs | `Db::write_blocking`, `fn(&Connection)` functions for tracks and history inside one transaction |
| area:09-domain-source-track-download-release-misc | `ReleaseClient::latest` |
| area:15-source-api-manga, area:16-source-api-anime, area:17-source-app-layer | `StubSourceRepo` (`upsert`, `get`, `watch_all`) |

---

## 6. Task summary

### 6.1 Counts

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then the 41 rows of `/home/blackbox/kurobayomi/plans/TASKS.csv` whose file starts with `06-data-repos-history-track-source-misc/`.

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 4 | 11 | 2 | 0 | 17 |
| P1 | 5 | 11 | 1 | 0 | 17 |
| P2 | 2 | 1 | 0 | 0 | 3 |
| P3 | 4 | 0 | 0 | 0 | 4 |
| **Total** | **15** | **23** | **3** | **0** | **41** |

Task IDs: T-06a-001 to T-06a-007, T-06a-010 to T-06a-014, T-06a-020, T-06a-021, T-06a-030 to T-06a-034, T-06a-040 to T-06a-044, T-06a-050 to T-06a-057, T-06a-060 to T-06a-062, T-06a-070 to T-06a-073, T-06a-080, T-06a-081. Every `Depends` token that names a task exists.

### 6.2 P0 critical path

The serial chain of the P0 items, in order. Effort bands are upper bounds (L = 1.5 weeks, M = 3 days, S = 1 day).

1. Prerequisites outside this area: area:01 runtime and `AppContext`, area:03 and area:04 schemas and views, area:02 preference keys, area:07 domain structs.
2. T-06a-001 (L): `Db::open`, PRAGMAs, writer and reader pool.
3. T-06a-002 (M): `read` and `write` with transaction semantics. T-06a-004 (S, no dependency) runs in parallel.
4. T-06a-003 (L): `watch`, update-hook notification, subscription lifetime. T-06a-005 (M), the test harness, runs in parallel.
5. T-06a-010 (M) manga history and T-06a-011 (M) anime history in parallel. T-06a-020 (M) updates and T-06a-041 (S) stubs also start here. T-06a-034 (S) runs in parallel once area:07 exists.
6. T-06a-012 (M): history list, search, latest. T-06a-014 (M): `HistoryRecorder`, needs area:02.
7. T-06a-080 (M): History and Updates screens wired to the repositories (needs area:42).

Serial length at the upper bounds: 7 + 3 + 7 + 3 + 3 + 3 = 26 engineer-days.

The source branch runs in parallel once T-06a-003 lands: T-06a-042 (M) with area:15, area:16 and area:12, then T-06a-040 (M) with area:17, then T-06a-043 (S), then T-06a-081 (M) with area:40 and area:41.

### 6.3 Risks

| # | Risk | Mitigation |
|---|---|---|
| R1 | `update_hook` may not report every change (FK cascades, `ON CONFLICT REPLACE` deletions) | Test D8 first in T-06a-003. Explicit table list fallback. `WHERE 1` on delete-all |
| R2 | No public specification exists for the new anime store format; real stores may differ from the Kotlin classes | Fixture tests from the Kotlin field tables (T-06a-050). Manual test against a live store before release |
| R3 | P0 Browse and source paging wait on area:15, area:16 and area:17, which are not yet on disk | Keep the `RemotePager` tests on a fake `CatalogueSource` so the pager is buildable first |
| R4 | Long writes from library updates (area:26) delay other writes | Reads use WAL readers. Keep write closures short; test T-06a-002 (200 ms write does not delay a read by more than 50 ms) |
| R5 | Custom button Lua can run arbitrary code from a backup | Restore confirmation (Q6). mpv `load-script` is the trust boundary; no sandbox is added |
| R6 | A `Subscription` kept alive after its GTK widget is destroyed leaks tasks and updates dead widgets | Store the subscription in the widget's destroy closure (`p1-rust-design.md` pitfall 3). Test D13 |
| R7 | Search semantics change (Q1) | Parity flag `HISTORY_SEARCH_PARITY=1` |
| R8 | Release check silently reports "no update" when the release has no arch-tagged asset | Developer naming rule `kurobayomi-<version>-<arch>` documented in T-06a-072; test R1 |
| R9 | Gzip bombs in store indexes | 32 MiB decompressed cap, `StoreError::TooLarge` (E7) |
| R10 | Empty later pages raise an error row instead of end-of-list (Kotlin parity) | UI in area:40 and area:41 must show the error row with Retry, as specified in `p1-topic-source-paging-and-stubs.md` section 6 |

### 6.4 Plan hygiene (lint results)

* `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint 06-data-repos-history-track-source-misc/p1-tasks.md` prints `ok`. No task IDs were changed. No task text was changed in this pass.
* `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export` reports 12 errors for this area, all of the form "unknown area". They name these ids: `area:15-source-api-manga` and `area:16-source-api-anime` (T-06a-040, T-06a-042), `area:17-source-app-layer` (T-06a-040), `area:47-settings-screens-general` and `area:53-platform-manifest-intents-workers` (T-06a-057), `area:45-player-core` and `area:48-settings-player-and-scripts` (T-06a-062), `area:42-ui-updates-history-stats-storage-upcoming` (T-06a-080), `area:40-ui-browse-manga` and `area:41-ui-browse-anime` (T-06a-081).
* Those ids are the partition ids in `areas.py`. Their folders are not yet in `plans/` (only areas 01 to 13 exist). No token was rewritten, because each one already names the correct partition id, and any other spelling would point at a nonexistent area. The errors clear when those areas are published.
* The `Depends` tokens in this area use only full area ids. Prose in the topic files uses short forms such as `area:12`. Those are not lint-checked.

---

## 7. Definition of done for this area in Kurobayomi

Each item is observable on the running app or by `cargo test -p kuro-core`.

1. **Database.** On an empty data directory, `manga.db` and `anime.db` are created with the schema version from area:03 and area:04. `PRAGMA foreign_keys`, `journal_mode = wal`, `synchronous = 1` and `busy_timeout = 5000` return the expected values on the writer and on a reader. A file with a newer `user_version` is refused and left unchanged.
2. **Reactivity.** A write from one screen updates another open list (History or Updates) without restart, and a rolled-back write updates nothing.
3. **History write.** Reading one chapter for about 10 seconds and leaving the reader makes that series appear in History with the chapter title and `HH:mm` time. The total read time in stats grows by about 10 seconds.
4. **History remove.** Removing one entry reveals the previous chapter of the same series, or removes the series when no earlier chapter remains. "Remove all for this entry" removes the series. "Clear history" shows "History deleted" and leaves the list empty and the total read time at 0.
5. **Anime history.** Watching an episode records `seen_at`. No duration is shown anywhere for anime history.
6. **Continue reading.** The continue-reading action opens the next chapter after the latest history row, and reports "no next chapter" when none exists.
7. **Updates.** The Updates tab lists only chapters of favourited entries with `date_fetch > date_added` and `date_upload` within three months. It is newest first and capped at 500. Toggling read updates the row live. Un-favouriting and re-favouriting hides older chapters.
8. **Browse.** Popular, latest and search listings load page by page. Scrolling within 25 items of the end loads the next page. An empty page shows "No results found" with Retry. An uninstalled source shows "Source not found". Changing the query replaces the listing.
9. **Stubs.** A source that was installed and later removed still shows its stored name with the "uninstalled" treatment. Its favourite count stays visible.
10. **Tracks.** Inserting a track twice for the same entry and tracker leaves one row. Deleting the entry removes its tracks. Fractional progress (for example 12.5) round-trips exactly.
11. **Anime extension stores.** Adding a store URL accepts JSON, protobuf, gzip-compressed and legacy index forms from a local test server. Refresh continues after a failing store and keeps its previous row. The error texts are the ones in `p1-topic-extension-store-index.md` section 3. Removing a store and then listing extensions reflects the change.
12. **Manga extension repos.** Only `https://.../index.min.json` URLs are accepted. A duplicate URL shows "This repo already exists!". A duplicate fingerprint shows the Replace dialog. A refresh adopts a real fingerprint in place of `NOFINGERPRINT-n` and rejects a changed one.
13. **Custom buttons.** A fresh anime database contains exactly the "+85 s" button with the byte-exact seed texts. Create, delete, reorder and favourite keep the invariants (at most one favourite; sort indexes 0..n-1). A broken script does not disable other buttons. The seed is not restored after deletion.
14. **Release check.** A manual check reports "no new updates" or the new version with the release notes (mention links, checksum footer removed). Startup checks run at most once every three days and are silent on failure. No installer or preview channel appears.
15. **Tests.** `cargo test -p kuro-core` contains the test names in `p1-edge-cases-and-tests.md` sections 1 and 4, including `write_err_rolls_back`, `write_panic_does_not_poison`, `nested_write_is_rejected`, `burst_is_coalesced`, `cascade_notifies_child_table`, `delete_all_notifies`, `open_newer_version_fails`, `upsert_accumulates`, `burst_is_coalesced`, `seed_matches_schema_sql` and the mention and asset selection vectors.
16. **Hygiene.** `kuro-core` has no `gtk` import. No `DELETE FROM <table>;` without `WHERE` exists in the crate (a test greps for it).
