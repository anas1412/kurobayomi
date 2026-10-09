# Area 17 — App source layer: overview

Area lead overview. Scope: the 27 Kotlin files listed in [p1-inventory.md](p1-inventory.md). All paths are repo-relative to `/home/blackbox/aniyomi-src`.

## 1. Purpose and scope

**What Aniyomi does here.** The source layer sits between extension loading (area:18-extensions-manga, area:19-extensions-anime, area:20-extension-strategy-redesign) and every screen that shows or fetches entries. It answers six questions:

1. Which sources exist right now: live sources from installed extensions, the local source (id `0`, lang `"other"`), and persisted **stub** rows for sources whose extension was uninstalled (`app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt`, `.../source/anime/AndroidAnimeSourceManager.kt`).
2. Which sources the Browse tab shows: language filter, disabled set, name sort, pinned flag, and a duplicated "last used" row (`app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt`, anime twin).
3. How sources are grouped by language and in what order (`GetLanguagesWith*Sources.kt`).
4. Toggles: enable/disable, pin, language, incognito per extension package, data-saver exclusion (manga only).
5. Migration source list ordering with favourite counts (`GetMangaSourcesWithFavoriteCount.kt`, `SetMigrateSorting.kt`).
6. Remote refresh of one anime: fetch metadata and episode or season lists from the source, write them, and sync (`app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt`).

**Why it matters for a usable Kurobayomi.** Without this area the Browse tab has no real data, library entries whose source was uninstalled show a numeric id instead of a name, the language sections cannot be built, and no library or reader refresh can write metadata or episode lists. Of the 50 tasks, 33 are P0.

**Scope limits.** Source trait implementations and HTTP (area:15-source-api-manga, area:16-source-api-anime), extension loading (areas 18 to 20), schema and migrations (area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations), the image data-saver pipeline (area:34-images-covers-cache), and the screens (areas 40 and 41 for browse, 37 to 39 for library and details) are owned elsewhere. There are no `*Test.kt` files in the 27 files, so the acceptance vectors are derived from the Kotlin (see [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md)).

## 2. Module map

### 2.1 Kotlin packages to responsibilities

| Package (repo-relative) | Files | LOC | Responsibility |
|---|---|---|---|
| `app/src/main/java/eu/kanade/domain/source/manga/interactor/` | 8 | 265 | Manga enabled list, language groups, incognito state and toggle, favourite-count list, data-saver exclusion toggle, source enable and pin toggles |
| `app/src/main/java/eu/kanade/domain/source/anime/interactor/` | 7 | 235 | Anime twins of the above (no data-saver toggle) |
| `app/src/main/java/eu/kanade/domain/source/interactor/` | 2 | 39 | `SetMigrateSorting` (mode and direction), `ToggleLanguage` |
| `app/src/main/java/eu/kanade/domain/source/manga/model/`, `.../anime/model/` | 2 | 32 | Compose `icon` extension properties (replace-with-crate, see section 2.2) |
| `app/src/main/java/eu/kanade/domain/source/service/` | 1 | 125 | `SourcePreferences`: typed view over the settings store, all source-level keys, `DataSaver` enum |
| `app/src/main/java/eu/kanade/tachiyomi/source/manga/` | 2 | 158 | `AndroidMangaSourceManager` (live map, stub map, registration), `MangaSourceExtensions` (naming, stub conversion, preference key, icon) |
| `app/src/main/java/eu/kanade/tachiyomi/source/anime/` | 2 | 167 | Anime twins, plus `isSourceForTorrents` |
| `app/src/main/java/mihon/domain/source/interactor/` | 1 | 215 | `UpdateAnimeFromRemote` (episode and season refresh, metadata write, cover and background cache rules) |
| `app/src/main/java/mihon/domain/source/models/` | 2 | 17 | `RemoteAnimeEpisodeUpdate`, `RemoteAnimeSeasonUpdate` result data classes |

Total: 27 files, 1,253 LOC. Port decisions: 21 Port, 4 Rewrite, 2 Replace-with-crate, no file-level Drop (drops are per feature; see section 4).

### 2.2 Proposed Rust module map

Crates follow the workspace names used by the other areas: `kuro-domain` (pure types), `kuro-core` (shared traits, here `SourceGate`), `kuro-sources` (this area), `kurobayomi` (GTK 4 UI).

```
kuro-domain/src/source/   pin.rs (PinFlags), model.rs (SourceEntry, StubSource, SourceKind)
kuro-core/src/sources.rs  SourceGate (watch receiver of readiness), SourceId = i64
kuro-sources/src/
  registry.rs      SourceRegistry (live map, stub map, readiness, generation watch)
  identity.rs      SourceIdentity trait over area 15/16 traits
  stubs.rs         StubStore trait + rusqlite store for `sources` and `animesources`
  naming.rs        visual_name, name_for_info, stub display, preference key
  prefs.rs         SourcePrefs typed view and key constants
  locale.rs        default enabled languages, display names, collator (ICU4X)
  enabled.rs       enabled_sources(), flip_disabled, set_disabled, toggle_pin, toggle_language
  languages.rs     group_by_language()
  incognito.rs     is_incognito()
  datasaver.rs     DataSaver enum, toggle_data_saver_excluded
  migration.rs     MigrationMode, MigrationDirection, sort_migration_sources()
  icons.rs         icon_for_source() -> gdk::Texture (GTK only, UI crate)
  torrent.rs       is_source_for_torrents()
  remote_update.rs RemoteUpdater, cover/background/title rules, apply_remote_metadata()
  error.rs         SourceError, RemoteError
```

Dropped Kotlin types: `ImageBitmap` (Compose) becomes `gdk::Texture`, and `Drawable` becomes the package icon file (see [p1-rust-design.md](p1-rust-design.md) section 11).

## 3. Reading guide

| Document | One-line description |
|---|---|
| [p1-inventory.md](p1-inventory.md) | The 27 files with LOC, every declared name, port decision and task ids |
| [p1-behavior.md](p1-behavior.md) | Exact Kotlin behaviour with `path:line` citations, and the Rust proposal for each rule |
| [p1-data-and-config.md](p1-data-and-config.md) | Every preference key with type and default, pin bit layout, domain models, SQL, serialization, constants |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Failure-mode table, test vectors (enabled list, migration, remote update), concurrency tests, fixtures |
| [p1-rust-design.md](p1-rust-design.md) | Crate and module layout, Rust types and signatures, SQL, pure functions, threading model, mockup integration |
| [p1-tasks.md](p1-tasks.md) | The 50 implementation tasks `T-17a-001` to `T-17a-050` in the house task format |
| [p1-topic-registry-and-stubs.md](p1-topic-registry-and-stubs.md) | Deep dive on the live registry, stub lifecycle, the state machine per source id, and the `getOrStub` lookup chain |
| [p1-topic-enable-pin-language-incognito.md](p1-topic-enable-pin-language-incognito.md) | Deep dive on the five persistent sets, enabled list, language groups, toggles, incognito |
| [p1-topic-migration-sorting-and-favorites.md](p1-topic-migration-sorting-and-favorites.md) | Deep dive on the migration source list, comparator, tie rules, and four test vectors |
| [p1-topic-remote-entry-update.md](p1-topic-remote-entry-update.md) | Deep dive on `UpdateAnimeFromRemote`: algorithm, cover and background decision tables, `AnimeUpdate` field rules |

No UI parity document is written for this area (see [p1-inventory.md](p1-inventory.md), "UI in scope"). The consumer screens belong to areas 37 to 41.

## 4. Key design decisions and open questions

### 4.1 Decided in the area docs (keep unless the user objects)

| # | Decision | Where |
|---|---|---|
| D1 | One `SourceEntry` and one `StubSource` type for manga and anime; `SourceKind` picks the stub table (`sources` or `animesources`). Anime rows always have `data_saver_excluded = false`. | [p1-rust-design.md](p1-rust-design.md) 2.2 to 2.4 |
| D2 | Memory-first registry: both stub tables are loaded at start; `get`, `get_or_stub`, and every list query do no I/O. | [p1-rust-design.md](p1-rust-design.md) 3.4 to 3.5, 10 |
| D3 | Pin flags are a 2-bit set (`PINNED_BIT 0b01`, `ACTUAL_BIT 0b10`). The Kotlin XOR `minus` becomes AND-NOT, so an unpinned last-used row has `0b00`, not `0b10`. No consumer reads the difference. | [p1-rust-design.md](p1-rust-design.md) 2.1 |
| D4 | `StubSource` derives `PartialEq`. Kotlin's stub classes have no `equals`, so the "unchanged" check is always false and every snapshot rewrites rows and calls rename. | [p1-behavior.md](p1-behavior.md) 1.5 |
| D5 | The stub list shows every persisted stub. Kotlin's `subscribeAllManga` collector writes into a copy that is discarded, so the stub list only shows stubs touched this session. | [p1-behavior.md](p1-behavior.md) 1.3 |
| D6 | A failed metadata write stops the episode or season sync and returns `RemoteError::Db`. Kotlin drops the `Boolean` and runs the sync anyway. | [p1-behavior.md](p1-behavior.md) 10, [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) E-S4 |
| D7 | Cancellation is `RemoteError::Cancelled`, never logged as an error and never returned as a generic failure. Kotlin's `catch (e: Exception)` swallows it. | [p1-behavior.md](p1-behavior.md) 12.1 |
| D8 | Language group order is enabled first, then `"all"` first, then localized display name, then raw code (tie-break). Kotlin's `TreeMap` silently drops a second group whose display name is equal. | [p1-behavior.md](p1-behavior.md) 5 |
| D9 | Stored keys are byte-identical to Aniyomi (`hidden_catalogues` for the disabled set, `pinned_catalogues`, `source_<id>` preference namespace, decimal id strings). Backup import can then map 1:1. | [p1-data-and-config.md](p1-data-and-config.md) 1, 7 |
| D10 | Download folder rename is injected after construction through `OnceLock<Arc<dyn SourceRenamer>>`, which breaks the Kotlin `injectLazy` cycle. Renames before attach are skipped and logged. | [p1-rust-design.md](p1-rust-design.md) 7 |
| D11 | Missing owners never panic: torrent check returns `false`, icon load returns `None`, `as CatalogueSource` becomes `SourceError::NotCatalogue`. | [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 1 |
| D12 | Dropped (P3): the commented-out SY keys (`eh_enable_source_blacklist`, `sources_tab_categories`, `sources_tab_categories_filter`, `sources_tab_source_categories`). They have no readers. | T-17a-039 |

### 4.2 Open questions for the user

| # | Question | Options | Recommendation |
|---|---|---|---|
| Q1 | Locale-aware display names and collation: ICU4X (`icu` crate, `icu_displaynames`, collator) or a hand-written table? | ICU4X adds compiled data to the binary. A table is wrong for many languages and has no collation. | ICU4X, accept the size cost, and say so in the release notes. |
| Q2 | Data-saver modes `BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT` send image requests to third-party image services. | (a) keep all four, (b) keep `NONE` only and drop the others. | Keep the enum and key so the stored value parses. Default `NONE`. Show a privacy note next to the modes in the settings screen (area:47-settings-screens-general or area:48). Do not enable any third-party mode by default. |
| Q3 | Are `__APP_STATE_*` keys (last-used source, global incognito, trusted extensions, search-filter toggle) included in backups? Owned by area:02. | Include all, exclude all, or split. | Exclude `__APP_STATE_*` from backups by default because they are device state. Trusted-extension state is re-verified by areas 18 to 20 on restore instead of being trusted from the file. |
| Q4 | Local source language: area:09 documents `lang ""`, but the code uses `"other"` (`source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:72`, `.../anime/LocalAnimeSource.kt:68`). | Follow the code or the area:09 doc. | Follow the code (`"other"`) and ask area:09 to correct its doc. The visual name becomes `Local manga source (OTHER)`, which is what Aniyomi shows. |
| Q5 | Shape of the local source constructor used by `SourceSystem` (area:14). | A `local_source(paths)` function returning `Arc<dyn SourceIdentity>`. | Ask area:14 to expose exactly that. |
| Q6 | Stub list visibility (D5) changes what users see on the source-management screen. | Show all persisted stubs, or keep the Kotlin session-only list. | Show all persisted stubs. It is the only way to find and clean up uninstalled sources. |

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | Needed | Used by tasks |
|---|---|---|
| area:01-foundation-app-bootstrap | Tokio runtime handle, XDG paths, tracing setup, constructor wiring, the start-up hook | T-17a-011, T-17a-036, T-17a-041 |
| area:02-preferences-system | Typed settings store with `watch` per key, `getStringSet` / `getEnum` / `getLong` semantics (unknown enum falls back to default), `__APP_STATE_` prefix, `incognito_mode`, backup rules for app state | T-17a-015, T-17a-021 |
| area:03-db-manga-schema-migrations | Pool for the manga database, `sources` table (migration `16.sqm`), `mangas` favourite-count query | T-17a-003, T-17a-024 |
| area:04-db-anime-schema-migrations | Pool for the anime database, `animesources` table, `animes` favourite-count query | T-17a-004 |
| area:05-data-repos-entries-items | `AnimeRepo::update` returning `bool`, anime-by-id read | T-17a-027 |
| area:07-domain-entries-items | Episode and season repositories (ordered by source order), `SyncEpisodesWithSource`, `SyncSeasonsWithSource`, `NoEpisodesException`, `NoSeasonsException`, `Anime` and `Episode` types | T-17a-027, T-17a-029, T-17a-032 |
| area:09-domain-source-track-download-release-misc | Domain source types as the reference model for the port; corrected local-source language (Q4) | T-17a-001, T-17a-026 |
| area:13-core-util-storage-archive-torrent | Collator (primary strength), locale utility, cover and background cache paths and deletion (`hashKeyForDisk`) | T-17a-028, T-17a-033, T-17a-045 |
| area:14-source-local-and-metadata | Local manga and anime source objects (id `0`, lang `"other"`) | T-17a-009 |
| area:15-source-api-manga | `CatalogueSource`, `HttpSource`, `SAnime`, `SEpisode`, source-side `getAnimeEpisodeUpdate` contract | T-17a-005, T-17a-012, T-17a-013 |
| area:16-source-api-anime | Anime source trait, `getAnimeEpisodeUpdate`, `getAnimeSeasonUpdate`, `UpdateStrategy`, status constants | T-17a-027, T-17a-029 |
| area:18-extensions-manga, area:19-extensions-anime, area:20-extension-strategy-redesign | Implementation of the `ExtensionCatalog` trait (`installed` watch, `index_source`, `package_for_source`, `icon_path_for_package`, `is_torrent`) | T-17a-006, T-17a-008, T-17a-025 |
| area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared | Implementation of `SourceRenamer` (download folder rename, `TMP_DIR_SUFFIX` handling) | T-17a-007 |

### 5.2 What others need from this area

| Consumer | Needs from area 17 | Task |
|---|---|---|
| area:18 to 20 | The `ExtensionCatalog` trait shape in [p1-rust-design.md](p1-rust-design.md) 2.7. If their names differ, keep this shape. | T-17a-006 |
| area:21-backup-format-and-create, area:22-backup-restore | Exact key names, `source_<id>` namespace, decimal id strings, enum names | T-17a-015, T-17a-026 |
| area:23 to 25 | `SourceRenamer` contract and the rename trigger (only from stub registration) | T-17a-007 |
| area:26-library-update-jobs | `RemoteUpdater::episodes` and `seasons`, `FetchFlags`, `RemoteError` | T-17a-027 to T-17a-031 |
| area:34-images-covers-cache | Data-saver settings and per-source exclusion set | T-17a-022 |
| area:37-ui-library, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime | `get_or_stub`, `ResolvedSource`, `visual_name`, `key()`, `is_used_last`, stub display | T-17a-010, T-17a-014, T-17a-037 |
| area:40-ui-browse-manga, area:41-ui-browse-anime | `enabled_sources`, `group_by_language`, `flip_disabled`, `set_disabled`, `toggle_pin`, `ready_watch`, generation watch | T-17a-016, T-17a-017, T-17a-019, T-17a-011 |
| area:43-reader-core, area:45-player-core | `get_or_stub` for the source of an entry, `catalogue` for page or episode fetch with typed errors | T-17a-013, T-17a-037 |
| area:47-settings-screens-general, area:48-settings-player-and-scripts | Typed `SourcePrefs` for source settings and data-saver modes | T-17a-015, T-17a-022 |

## 6. Task summary

### 6.1 Counts (from `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, rows whose file starts with `17-source-app-layer/`)

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 25 | 7 | 1 | 0 | 33 |
| P1 | 7 | 3 | 0 | 0 | 10 |
| P2 | 3 | 0 | 0 | 0 | 3 |
| P3 | 4 | 0 | 0 | 0 | 4 |
| **Total** | **39** | **10** | **1** | **0** | **50** |

Task file lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint 17-source-app-layer/p1-tasks.md` prints `ok`. The dependency graph has no cycles (checked over the 50 tasks).

### 6.2 P0 critical path, in order

1. **Domain types and pin bits.** T-17a-001 (`SourceEntry`, `StubSource`, `SourceKind`), then T-17a-002 (`PinFlags`). No I/O; everything else depends on them.
2. **Stores and settings, in parallel.** T-17a-003 (manga `sources` store) and T-17a-004 (anime `animesources` store); T-17a-015 (typed core keys, needs area:02).
3. **Registry core.** T-17a-005 (live map, lookups; needs area:15) then T-17a-006 (snapshot loop and readiness) then T-17a-011 (`SourceGate` watch).
4. **Registration and fallback.** T-17a-007 (`register_stub`: equality, write, rename) then T-17a-008 (stored row, then index data, then fallback) then T-17a-036 (start-up order) then T-17a-037 (memory-only `get_or_stub`).
5. **Browse list.** T-17a-016 (enabled pipeline; needs 015 and 002) then T-17a-035 (last-used duplicate) then T-17a-042 (tests). In parallel: T-17a-019 (flips and pins) and T-17a-020 (language toggle).
6. **Language groups.** T-17a-045 (locale display names and collator; needs area:13) then T-17a-018 (default languages) then T-17a-017 (grouping; needs 015 and 045) then T-17a-049 (tests).
7. **Remote refresh** (the only L task). T-17a-032 (result models) and T-17a-030 (errors) and T-17a-028 (cover and background rules; needs area:13) and T-17a-031 (fetch flags), then T-17a-027 (orchestration; needs area:05 and area:07), then T-17a-043 (rule tests).

Steps 5 to 7 can run in parallel once step 4 is done. The UI can show the Browse list as soon as steps 1 to 5 are done.

### 6.3 Risks

| Risk | Effect | Mitigation |
|---|---|---|
| Cross-area trait shapes move (area:15/16, area:18-20) | `SourceIdentity` and `ExtensionCatalog` change, and the registry and tests need rework | Keep the trait shapes in [p1-rust-design.md](p1-rust-design.md) 2.6 and 2.7 as the contract; add a fake `ExtensionCatalog` (T-17a-038) so this area can be built and tested alone |
| Area:16 plan is marked "not yet written" in [p1-inventory.md](p1-inventory.md) | Anime source trait may differ from the assumed shape | Confirm the shape in area:16 before T-17a-012 and T-17a-027 start |
| Stub-list fix (D5) and stub-equality fix (D4) change visible behaviour | Users see more stubs and fewer rewrites than before | Document both in the Rust docs (T-17a-034, T-17a-007) |
| ICU4X binary size and data (Q1) | Larger binary; a build-time dependency | Measure after T-17a-045; fall back to a table only if the size is unacceptable |
| `to_lowercase` vs Java `CASE_INSENSITIVE_ORDER` | A few code points sort differently (for example German `ß`) | Accepted difference, pinned by vector E-F11 in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) |
| Rename during start-up before `set_renamer` | A rename is skipped and logged; the download folder keeps its old name | Attach the renamer before the first stub registration and log orphan folders (T-17a-048, P2) |
| Stale `anime` object passed to the episode sync | Scheduling fields (`fetchInterval`, `nextUpdate`) read from pre-update values | Keep the Kotlin behaviour on purpose and document it in T-17a-027 |
| Two concurrent refreshes of one anime | Duplicate episode rows or cache deletes | Per-anime mutex (T-17a-047, P1) |

## 7. Definition of done for this area in Kurobayomi

Each item is an observable behaviour the user can check in the running app.

1. **Browse list.** The Browse tab shows the enabled sources from installed extensions and the local source. Order: last-used row first (with its duplicate), then pinned sources, then language sections (enabled languages first). Within a section, disabled sources come last and names sort case-insensitively.
2. **Toggles persist.** Disabling a source, pinning a source, or unticking a language is visible immediately and still applied after a restart. Unticking a language hides its sources without changing their disabled or pinned state.
3. **Uninstall keeps the name.** After uninstalling an extension, a library entry from one of its sources still shows the stored name (not the numeric id), and opening that source's fetch shows a "not installed" message.
4. **Loading state.** Source lists show a spinner until the first extension snapshot is applied, then show the list.
5. **Migration list.** The migrate source list shows favourite counts. Alphabetical and total sorting, ascending and descending, match the four vectors in [p1-topic-migration-sorting-and-favorites.md](p1-topic-migration-sorting-and-favorites.md) section 6. The local source does not appear.
6. **Incognito.** The per-extension incognito switch hides history writes for that extension's sources. The global incognito switch applies to every source, including the local source.
7. **Refresh offline.** Refreshing an anime with the network off shows one error and leaves the library entry unchanged.
8. **Refresh online.** A refresh writes metadata (cover and background timestamps follow the rules in the remote-update topic), syncs the episode list, and shows new episodes. A refresh of an entry with no episodes from a non-local source fails with the "no episodes" message.
9. **Responsiveness.** No GTK-thread call in this area performs I/O. Opening the Browse tab with 10,000 unknown source ids in the library does not freeze the UI (test E-C6).
10. **Backup interop.** A backup written by this app restores the source settings to the same keys Aniyomi uses (`hidden_catalogues`, `pinned_catalogues`, `source_languages`, `data_saver_excluded`, and the rest in [p1-data-and-config.md](p1-data-and-config.md) section 1).
11. **Data saver setting.** The data-saver mode and per-source exclusion are stored, shown, and read back. The image pipeline behaviour belongs to area:34.

## 8. Known inconsistencies in the area docs

These were found while writing this overview. They are small, and each should be fixed in the named file during the next pass.

1. [p1-rust-design.md](p1-rust-design.md) 3.1 names the type `SourceRegistry<K>`, but the struct that follows is not generic (`kind: SourceKind` field instead).
2. [p1-rust-design.md](p1-rust-design.md) 5.1 gives `set_disabled` no return value. [p1-topic-enable-pin-language-incognito.md](p1-topic-enable-pin-language-incognito.md) section 9 and T-17a-019 say every toggle returns the new membership. Resolve in favour of the topic: the bulk form returns the count of changed ids.
3. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) E-C6 says "under 5 ms per call on average" for 10,000 calls, while T-17a-037 says "under 50 ms total". Both cannot hold. Use the total (50 ms).
4. [p1-inventory.md](p1-inventory.md) says the area:16 plan is "not yet written", but `plans/16-source-api-anime/` exists. Check its contents before relying on that statement.
5. Short tokens `area:NN` appear in prose in several docs. The task file uses full ids only, and lint passes. The prose tokens are unambiguous prefixes, so they are left as they are.

## 9. Changes made in this pass

- [p1-tasks.md](p1-tasks.md): removed the dependency cycle T-17a-015 to T-17a-020 (T-17a-015 no longer depends on T-17a-020; the typed accessors do not need the toggle).
- [p1-tasks.md](p1-tasks.md): removed the dependency cycle T-17a-027 to T-17a-028 (T-17a-028 depends only on area:13, because the cover and background rules are the pure inputs of T-17a-027).
- No task IDs were changed. Lint prints `ok`.
