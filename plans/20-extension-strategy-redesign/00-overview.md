# 20-extension-strategy-redesign: overview

Status: area lead summary, written 2026-10-08. Reads the nine part docs in this folder and spot-checks the Kotlin source at `/home/blackbox/aniyomi-src` (repo-relative paths). OBSERVED = what the Kotlin does. Proposal: = what Kurobayomi should do.

## 1. Purpose and scope

Aniyomi loads content sources from Android APK files. Each extension is a signed package that contains Kotlin code implementing `MangaSource`/`AnimeSource` (or their `HttpSource`/`ParsedHttpSource` bases). The app finds them through `PackageManager` or a private `<filesDir>/exts/<pkg>.ext` file, checks their version, signature and trust, instantiates their classes through a `ChildFirstPathClassLoader`, and then uses them for browse, search, details, chapters, pages (manga) and hosters, videos, episodes, seasons (anime).

Why it matters for a usable Kurobayomi: without a source mechanism, the library, reader and player have nothing to read or play. Every library row, chapter row, history row and backup entry stores a 64-bit source id (`HttpSource.generateId`). That id must survive the move, or every migrated library entry is orphaned.

This area decides:
- how Kurobayomi loads sources (built-in Rust, declarative TOML scrapers, WASM components);
- how the registry tracks installed, available and untrusted packages, and how it installs, updates and trusts them;
- how an Aniyomi library maps onto Kurobayomi (frozen source ids, stub sources for missing sources, backup id resolution).

Out of scope here (owned elsewhere): the HTTP client and its rate limiter (area:12-network-stack), the per-store wire formats and persistence of extension repos (area:06-data-repos-history-track-source-misc, area:18-extensions-manga, area:19-extensions-anime), the extension screens (area:40-ui-browse-manga, area:41-ui-browse-anime), backups (area:21-backup-format-and-create, area:22-backup-restore), the player (area:45-player-core), and the reader (area:43-reader-core).

## 2. Module map

### 2.1 Kotlin module map (observed)

The inventory in [p1-inventory.md](p1-inventory.md) lists 84 files, 8,106 LOC. Port decisions: 59 Rewrite, 12 Replace-with-crate, 13 Drop. The 84 files split as: extension tree 25, `source/manga` and `source/anime` 4, `source-api/src/commonMain` 51 (source 18, animesource 26, torrentutils 3, util 4), `source-api/src/androidMain` 4. Priorities: 49 P0, 16 P1, 6 P2, 13 P3.

| Package (repo-relative directory) | Files | Responsibility | Port decision |
|---|---:|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/extension/` | 2 | `ExtensionUpdateNotifier.kt` (update notification), `InstallStep.kt` (install states) | Notifier -> `kb-notify`; InstallStep rewritten |
| `extension/manga/util/` and `extension/anime/util/` | 10 | `MangaExtensionLoader.kt`, `AnimeExtensionLoader.kt` (discovery, load checks, instantiate); `MangaExtensionInstaller.kt`, `AnimeExtensionInstaller.kt` (download and install); `MangaExtensionInstallReceiver.kt`, `AnimeExtensionInstallReceiver.kt` (package broadcasts); `MangaExtensionInstallService.kt`, `AnimeExtensionInstallService.kt` (foreground install service); `AnimeExtensionInstallActivity.kt`, `MangaExtensionInstallActivity.kt` (install UI) | Loader replaced by the WASM and built-in loader; installer rewritten; receiver replaced by inotify watch (T-20a-040); service and activity dropped |
| `extension/manga/api/` and `extension/anime/api/` | 2 | `MangaExtensionApi.kt`, `AnimeExtensionApi.kt`: repo index fetch, update check | Rewrite in `kb-ext::repo` |
| `extension/manga/installer/` and `extension/anime/installer/` | 6 | `InstallerManga.kt`, `InstallerAnime.kt` (queue and download), `PackageInstallerInstallerManga.kt`, `PackageInstallerInstallerAnime.kt`, `ShizukuInstallerManga.kt`, `ShizukuInstallerAnime.kt` | Queue rewritten (one-permit semaphore); PackageInstaller and Shizuku dropped (T-20a-093) |
| `extension/manga/model/` and `extension/anime/model/` | 3 | `MangaExtension.kt` (sealed Installed/Available/Untrusted), `*LoadResult.kt` | Rewrite in `kb-ext::model` |
| `extension/manga/` and `extension/anime/` managers | 2 | `MangaExtensionManager.kt`, `AnimeExtensionManager.kt`: installed, available, untrusted maps, update flags | Rewrite as `kb-ext::registry` |
| `app/src/main/java/eu/kanade/tachiyomi/source/manga/` and `source/anime/` | 4 | `AndroidMangaSourceManager.kt`, `AndroidAnimeSourceManager.kt`, `MangaSourceExtensions.kt`, `AnimeSourceExtensions.kt`: live map, stubs, helper labels | Rewrite as `kb-source::manager`, `kb-source::ext` |
| `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/` | 6 (+9 in `model/`, 3 in `online/`; 18 in total) | `MangaSource`, `CatalogueSource`, `ConfigurableSource`, `UnmeteredSource`, `SourceFactory`, `Filter`, `FilterList`, `SManga`, `SChapter`, `Page`, `MangasPage`, `UpdateStrategy`, `HttpSource`, `ParsedHttpSource`, `ResolvableSource` | Rewrite as `kb-source` traits and models; `ParsedHttpSource` replaced by `kb-source-scrape` |
| `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/` | 6 (+16 in `model/`, 3 in `online/`, 1 in `utils/`) | `AnimeSource`, `AnimeCatalogueSource`, `ConfigurableAnimeSource`, `AnimeFilter`, `SAnime`, `SEpisode`, `Hoster`, `Video`, `Track`, `TimeStamp`, `ThumbnailInfo`, `AnimeRelation`, `AnimesPage`, `FetchType`, `AnimeHttpSource`, `ParsedAnimeHttpSource`, `ResolvableAnimeSource`, `Preferences.kt`, `HttpServer.kt` | Rewrite as `kb-source` anime types; `HttpServer` dropped |
| `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/torrentutils/` | 3 | `TorrentUtils.kt`, `TorrentInfo`, `TorrentFile` | Replace with `kb-torrent` (P2) |
| `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/` | 4 | `JsonExtensions.kt`, `JsoupExtensions.kt`, `RxExtension.kt`, `VideoInfo.kt` | serde_json and scraper helpers; Rx dropped |
| `source-api/src/androidMain/` | 4 | `AndroidManifest.xml`, `PreferenceScreen` typealiases, `RxExtension` actual | Dropped (P3) |
| `app/src/main/java/eu/kanade/domain/extension/{manga,anime}/interactor/` | 2 (referenced by T-20a-060; not in the 84-file inventory) | `TrustMangaExtension.kt`, `TrustAnimeExtension.kt`: repo key set and per-version trust rows | Rewrite as `kb-ext::trust` |

Out-of-scope but referenced: the store and repo services under `data/src/main/java/mihon/data/extension/{manga,anime}/` (for example `data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt`).

### 2.2 Rust module map (proposal)

From [p1-rust-design.md](p1-rust-design.md) R2, plus `kb-notify` from the inventory.

```
kurobayomi/crates/
  kb-source/            traits, models, filters, id derivation; no I/O
    manga.rs            MangaSource, SManga, SChapter, Page, MangasPage, UpdateStrategy
    anime.rs            AnimeSource, SAnime, SEpisode, Hoster, Video, TimeStamp, Track
    filter.rs           Filter, TriState, SortSelection, FilterList
    id.rs               source_id(name, lang, version_id)
    http.rs             HttpSourceBase helper on reqwest
    error.rs            SourceError
    manager.rs          SourceManager: get, get_or_stub; SourceRef::Live | Stub
    settings.rs         Configurable trait; source_settings table access
  kb-ext/               registry, loader, install, trust, repo fetch
    registry.rs         ExtensionRegistry (watch channels + arc-swap source table)
    loader.rs           scan <data>/extensions, select, verify, instantiate
    manifest.rs         manifest.toml parse and validation
    install.rs          download, sha256, atomic rename, InstallStep, one-permit semaphore
    repo.rs             index.min.json fetch and parse
    trust.rs            repo key set plus per-version trust rows
    update.rs           has_update, is_obsolete, 1-day throttle
    watch.rs            notify (inotify) watcher, 300 ms debounce
    model.rs            Installed, Available, Untrusted, LoadResult, InstallStep serde
  kb-ext-wasm/          wasmtime host, WIT world, limits
    wit/source.wit      kurobayomi:source@1.0.0
    host.rs             http and settings imports
    limits.rs           fuel, epoch, memory caps
  kb-source-scrape/     TOML scraper definition and evaluator (scraper crate)
  kb-sources-builtin/   compiled-in Rust sources (none ported yet)
  kb-notify/            notify-rust wrapper for the update notification
  kb-torrent/           torrent backend client (P2)
```

Crate names are proposals. `src/` stays the GTK application and becomes a workspace member.

## 3. Reading guide

- [p1-inventory.md](p1-inventory.md): the 84 Kotlin files in scope, each with LOC, declared names, port decision, target Rust module and task IDs.
- [p1-rust-design.md](p1-rust-design.md): the decision (built-in, then TOML scrapers, then WASM), crate layout, Rust types and traits, errors, registry and install flow, WIT contract, WASM limits, threading, anime playback details, Android-to-Linux mapping, pitfalls.
- [p1-behavior.md](p1-behavior.md): observed behaviour of loading (B1), the registry (B2), first-run language enablement (B3), repo fetch (B4), install (B5), trust (B6), source manager and stubs (B7), helper labels (B8), id derivation (B9), anime specifics (B10), edge cases (B11).
- [p1-data-and-config.md](p1-data-and-config.md): exact source, extension and repo index field models with defaults (D1-D4), preference keys (D5), files on disk (D6), proposed on-disk layout (D7), `manifest.toml` (D8), DB tables (D9), repo list (D10), notification strings (D11), anime player options (D12).
- [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge cases E1-E10 with expected Kurobayomi behaviour, the test list E12, fixtures E13, manual checks E14.
- [p1-tasks.md](p1-tasks.md): the 28 implementation tasks (T-20a-*).
- [p1-topic-option-matrix.md](p1-topic-option-matrix.md): scored comparison of WASM, Lua, JS, TOML scraper, JVM bridge, sidecar and static Rust, with the phased plan.
- [p1-topic-wasm-component-design.md](p1-topic-wasm-component-design.md): WASM package layout, manifest validation order, host functions, limits, versioning, authoring.
- [p1-topic-source-id-and-backups.md](p1-topic-source-id-and-backups.md): frozen id derivation, stub rows, backup import rule, renamed and versioned sources, open question on Unicode lowercasing.

## 4. Key design decisions and open questions

### 4.1 Decisions already in the docs (accepted by this overview)

1. Three source kinds, in this order: built-in Rust sources, declarative TOML scrapers, WASM components. Rejected for the primary path: JVM bridge, Lua, JS as primary, sidecar processes. Source: [p1-topic-option-matrix.md](p1-topic-option-matrix.md).
2. Aniyomi APK extensions are not loaded. There is no APK, dex, PackageInstaller, Shizuku or NanoHTTPD code path. The library survives because ids are preserved, not because binaries are reused. Reason: the Aniyomi index has 3 entries (last commit 2024-07-11) and keiyoushi has 407 `src/en` directories, all written against Android APIs.
3. Source ids are frozen: `md5("<name.lowercase()>/<lang>/<versionId>")`, first 8 bytes big-endian, AND `i64::MAX`. Default `versionId = 1`. Verified in the Kotlin: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:87-90` and `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:101-` (`generateId`), with `versionId = 1` at `HttpSource.kt:45`.
4. Missing sources become stub rows `(id, lang, name)`. Stubs keep the entry, its chapters and history, disable reader, search and refresh, and use message key `source_not_installed`. Installing the source later re-activates the same id with no migration.
5. Trust is two-tier, as in Aniyomi: a package is trusted if its signing key is in a repo key set, or if `"<pkg>:<version_code>:<key_id>"` is in the trusted rows. Downgrades and key changes are refused at install.
6. Install is one at a time (one-permit semaphore), downloads into `<data>/extensions/.tmp/`, verifies sha256 when the repo gives one, then renames into `<data>/extensions/<pkg>/`. Cancel deletes the temp file.
7. Anime playback uses libmpv. `ffmpegStreamArgs` and `ffmpegVideoArgs` are dropped. Only `mpvArgs` is kept. `createHttpServer` is dropped.

### 4.2 Open questions that need the user's call

Each has a recommendation. Nothing here is decided until the user answers.

| # | Question | Recommendation |
|---|---|---|
| Q1 | Accept that existing Aniyomi APK extensions stop working, and only library data (ids, titles, chapters, history) carries over? | Yes. The rewrite is per source in any case. Users keep the library and see stubs for sources not yet ported. |
| Q2 | Which sources to port first? The author has to name them. | Export an Aniyomi backup, count library entries per `source` id, and port the top sources by entry count. Sources with zero library entries wait. |
| Q3 | Is WASM needed for the first usable release? Current tasks mark T-20a-001 (WASM loader) as P0. | No. Phase 0 needs only built-in sources. Recommend moving T-20a-001 to P1 (or splitting it so the built-in loader is P0 and the wasmtime host is P1). This is a priority change, so it needs the user's call. See 4.3. |
| Q4 | Which repositories ship in the default config? The Aniyomi repo list was not read (D10 says so). | Ship none. The user adds a repo URL and its signing key. The observed Aniyomi repo has 3 APK entries that Kurobayomi cannot run. |
| Q5 | Where do repo rows live: a SQLite `extension_repos` table (areas 03 and 18) or `repos.toml` (proposal in [p1-data-and-config.md](p1-data-and-config.md) D10)? | SQLite `extension_repos`. One storage location. Drop `repos.toml`. |
| Q6 | Keep Unicode lowercasing in `source_id` (Kotlin `lowercase()` is Unicode, root-locale) or switch to ASCII? | Keep Unicode (`str::to_lowercase`). Existing non-ASCII ids require it. Add the Turkish dotted-I and non-ASCII golden vectors (E12). |
| Q7 | Do WASM packages need their own signature (ed25519, B6 proposal), or only the repo `sha256` plus the repo signing key? | Repo sha256 plus signing key, the same two-tier rule as Aniyomi. Defer ed25519 until a second repo exists. |
| Q8 | Add a new manifest field `allowed_hosts`, which bounds the WASM `http` import? | Yes, required for WASM sources. Without it, any extension can reach any host. |
| Q9 | Support torrent-based anime sources (T-20a-070, P2)? Candidate backends are librqbit or the rqbit HTTP API. | Defer. Decide when a torrent-only source the user needs exists. |
| Q10 | Drop `ffmpegStreamArgs`, `ffmpegVideoArgs` and the local HTTP server? | Yes. Rare in practice. User loses source-specified ffmpeg tweaks and sources that depend on the local proxy. |
| Q11 | Support Aniyomi's protobuf store and gzip-compressed store formats, and the legacy `repo.json` with `indexV2`? | JSON `index.min.json` first. Protobuf is P2 (T-19a-014, owned by area:19-extensions-anime). Gzip is cheap to add. Reject `repo.json`-only repos with a clear message. |
| Q12 | Show the observed Aniyomi APK entries in the UI as "unsupported format" (E14), or hide them? | Show them as unsupported. Hiding hides the migration state from the user. |

### 4.3 Inconsistencies found while reading (fix before coding)

1. Phase plan versus priorities. [p1-topic-option-matrix.md](p1-topic-option-matrix.md) puts the WASM host in Phase 2, but T-20a-001 (WASM loader) is P0, Effort L. The P0 definition in the project brief needs only built-in sources for usable reading and watching.
2. P0 depends on P1. T-20a-002 (registry, P0) depends on T-20a-020 (repo fetch, P1) and T-20a-060 (trust, P1). The registry can ship with empty repos, so the P0 chain should not require them.
3. API version rule mismatch. T-20a-001 acceptance says "not in `1.x`" is rejected. [p1-topic-wasm-component-design.md](p1-topic-wasm-component-design.md) says major must be `1` and minor must be `0`, and newer minors are rejected with `UnsupportedApi`. Pick one. Recommend the topic doc rule.
4. Missing task for the built-in sources themselves. No task creates `kb-sources-builtin` or ports a named source. Phase 0 cannot deliver a usable app without this. Add one task per source (or one per site), with the source list from Q2.
5. Missing task for the WASM guest template and fixtures. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) E13 requires `kb-ext-template` and `echo.wasm` in CI. No task owns them.
6. Anime library filter is resolved. [p1-behavior.md](p1-behavior.md) B4 marks the anime lib filter as UNVERIFIED. Read now: the anime store conversion has no lib-version filter (`data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt:103` sets `libVersion = extension.extensionLib.toDouble()`, and `NetworkLegacyAnimeExtension.kt:34` parses the version string). The support check happens only at load, `app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt:265` with `SUPPORTED_LIB_VERSIONS = listOf(14.0, 16.0, 17.0)` at line 53. So unsupported anime entries appear in the available list and fail at load. Kurobayomi should filter them out at repo fetch (T-20a-020 already says so).
7. Store wire formats not in this area's docs. `AnimeExtensionStoreService.kt` decodes three formats: a JSON array (`0x5B`), a JSON object with `indexV2` (`0x7B`), a protobuf (`else`), and it also gunzips the body. [p1-data-and-config.md](p1-data-and-config.md) D4 mentions only the JSON array. Covered by T-06a-050 and T-19a-014 in other areas; add a cross-reference to them in D4.
8. Duplicate task areas. Several tasks in other areas cover the same work as this area (see section 5). Cross-area owners must pick one.

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | Needed for | Status in its plan |
|---|---|---|
| area:12-network-stack | HTTP client, per-host rate limiter, UA default, timeouts. Used by T-20a-001, T-20a-011. | Covered (T-12a-023 extension client builder, T-12a-029 extension-facing API). |
| area:02-preferences-system | Per-source preferences (T-20a-005), `enabled_languages`, trust and update preferences. | Not checked against its task list. |
| area:03-db-manga-schema-migrations | `extension_repos`, stub rows, library source columns. Used by T-20a-094. | Covered (T-03a-017). |
| area:04-db-anime-schema-migrations | `extension_store` for anime (T-04a-019). | Covered. |
| area:06-data-repos-history-track-source-misc | Store wire formats and repository persistence (T-06a-050 to T-06a-057). Used by T-20a-020. | Covered; overlaps T-20a-020 (see 5.3). |
| area:08-domain-library-category-history-updates | Library update strategy `ALWAYS_UPDATE` and `ONLY_FETCH_ONCE` (T-20a-098), update notification (T-20a-050). | Not checked against its task list. |
| area:09-domain-source-track-download-release-misc | Aggregation across stores with failure isolation (T-09a-033). | Covered. |
| area:13-core-util-storage-archive-torrent | Path helpers, XDG directories, archive helpers used by the installer. | Not checked against its task list. |
| area:14-source-local-and-metadata | Local source with id `0` (T-20a-003 acceptance). | Not checked against its task list. |
| area:17-source-app-layer | Source list queries, extension-change loop (T-17a-006, T-17a-010). Used by T-20a-003. | Covered; overlaps (see 5.3). |
| area:22-backup-restore | Restore of source ids and stub rows (T-22a-026, T-22a-027). Used by T-20a-094. | Covered. |
| area:26-library-update-jobs | Library refresh that calls `ALWAYS_UPDATE` and `ONLY_FETCH_ONCE` (T-20a-098). | Not checked against its task list. |
| area:36-ui-shell-navigation-security-webview | Startup update checks and tab badges (T-36a-007, T-36a-022). Consumes `has_update` counts. | Covered. |
| area:43-reader-core, area:44-reader-viewers | Source `pages` and `image_url` results. | Not checked against their task lists. |
| area:45-player-core, area:46-player-controls-ui | Anime `Video`, headers, subtitle and audio tracks, `TimeStamp` skip markers, `mpvArgs`. | Not checked against their task lists. |
| area:51-i18n | Message keys: `source_not_installed` (T-20a-095), `ext_install_checksum_mismatch` (B5), `extension_api_error` (E3.2), "no hosters" (E9.1). | Keys still need adding to the catalog. |

### 5.2 What others need from this area

| Area | Needs from this area |
|---|---|
| area:15-source-api-manga | Only the manga trait overlap in 5.3 (T-15a-013 vs T-20a-010). |
| area:16-source-api-anime | Anime trait and models (T-20a-007, T-20a-010, T-20a-097). |
| area:18-extensions-manga, area:19-extensions-anime | Registry (T-20a-002), install pipeline (T-20a-030), trust (T-20a-060), update flags (T-20a-002). |
| area:21-backup-format-and-create | `source_id` and the stub row shape to write backups. |
| area:22-backup-restore | `source_id` derivation (T-20a-011) and stub upsert (T-20a-003) to restore library rows. |
| area:40-ui-browse-manga, area:41-ui-browse-anime | Registry snapshots, `InstallStep`, trust dialog data, update counts (T-20a-002, T-20a-030, T-20a-060). |
| area:47-settings-screens-general | Repo rows and trust revocation (T-20a-099, T-20a-060). |
| area:43-reader-core, area:45-player-core | `Source` trait calls and `SourceError` variants. |

### 5.3 Overlaps that must be resolved by the owners

| This area | Other area | Overlap | Recommendation |
|---|---|---|---|
| T-20a-010 (core Source trait) | T-15a-013 (MangaSource and CatalogueSource traits with panic isolation, area:15) | Same trait, two tasks | Area:15 owns the manga trait file. This area owns the anime trait and the shared model. |
| T-20a-003 (source manager, stubs) | T-17a-010 (source list queries), T-17a-006 (extension-change loop), area:17 | Same manager | Area:17 owns queries, this area owns stub creation and the id lookup. |
| T-20a-002 (registry) | T-18a-001 (model types, load result), T-19a-001 (anime model types) | Same model | This area owns `kb-ext::model`. Areas 18 and 19 import it. |
| T-20a-020 (repo index fetch) | T-06a-052 (store getExtensions), T-06a-053 (repository), T-19a-018 (list branches) | Same fetch | Area:06 owns decoding and persistence. This area owns the update semantics and the supported-API filter. |
| T-20a-060 (trust) | T-10a-024 (TrustExtension, area:10) | Same interactor | This area owns the rule. Area:10 owns the domain interactor shell. |
| Repo storage (D9, D10) | T-03a-017 and T-18a-014 (`extension_repos`) | Table name and TOML file | Decision Q5. |
| T-20a-094 (backup id resolution) | T-22a-026, T-22a-027 | Same restore path | Area:22 owns the restore. This area owns the id rule. |

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint 20-extension-strategy-redesign/p1-tasks.md` prints `ok`. `tasks.py export` rewrote `plans/TASKS.csv` with 3,673 tasks across all areas; 28 belong to this area (file prefix `20-extension-strategy-redesign/`).

Area totals:

| Priority | S | M | L | XL | Total |
|---|---:|---:|---:|---:|---:|
| P0 | 3 | 10 | 1 | 0 | 14 |
| P1 | 5 | 3 | 2 | 0 | 10 |
| P2 | 1 | 1 | 0 | 0 | 2 |
| P3 | 2 | 0 | 0 | 0 | 2 |
| Total | 11 | 14 | 3 | 0 | 28 |

P0 tasks: T-20a-001 (L), T-20a-002, T-20a-003, T-20a-005, T-20a-006, T-20a-007, T-20a-010, T-20a-011, T-20a-094, T-20a-096, T-20a-097 (M), T-20a-004, T-20a-012, T-20a-095 (S).

Rough effort, using the upper bounds (S = 1 day, M = 3 days, L = 7.5 days): P0 about 40 engineer-days upper bound (T-20a-001 alone is 7.5). P1 about 29 days. The real per-source porting cost is outside this count; see the risks.

### 6.1 P0 critical path, in order

Dependencies are from the `Depends:` lines in [p1-tasks.md](p1-tasks.md). Chain length upper bound about 30 days if run serially.

1. T-20a-010 (core Source trait, M). No dependencies. Start here.
2. T-20a-011 (HTTP base and `source_id`, M). Needs area:12-network-stack. Golden vectors must be produced from the Kotlin first (E12 item 1). Highest correctness risk in this area.
3. In parallel after step 1: T-20a-006 (filters, M), T-20a-007 (models, M), T-20a-012 (factory, S), T-20a-005 (settings, M, needs area:02-preferences-system).
4. T-20a-001 (loader, L). Needs steps 1 and 2 and area:12-network-stack. Blocked on Q3.
5. Pulled in from P1: T-20a-099 (repo list, S), T-20a-020 (repo fetch, M), T-20a-060 (trust, M). These are needed by T-20a-002 as written; see inconsistency 4.3.2.
6. T-20a-002 (registry, M). Needs steps 4 and 5.
7. T-20a-003 (source manager and stubs, M). Needs step 6 and area:17-source-app-layer.
8. In parallel after step 7: T-20a-004 (helpers, S), T-20a-095 (missing source behaviour, S), T-20a-094 (backup id resolution, M, needs area:03-db-manga-schema-migrations).
9. Anime branch, in parallel with steps 4 to 8: T-20a-096 (hoster and video, M) and T-20a-097 (episode and season update, M), both need T-20a-007.

### 6.2 Risks

1. No source to play with. Phase 0 cannot be usable unless at least one built-in source is ported. No task owns this (inconsistency 4.3.4). Mitigation: add a task per named source.
2. Wrong source id derivation. Every backup link depends on T-20a-011 matching Kotlin byte for byte. The golden vector run has not been done (E12 item 1). Mitigation: produce the vectors first, before any other task.
3. Porting cost is the real schedule. Keiyoushi's 407 `src/en` directories give no reuse; each source is a rewrite against a live site. Expect the per-source work to dwarf every task in this area.
4. WASM toolchain churn. wasmtime, wit-bindgen and cargo-component change often. Pin versions and add `cargo component` to CI before T-20a-001.
5. Scraper drift. Site markup changes break TOML scrapers and built-in sources alike. Mitigation: recorded HTML fixtures and a test per source.
6. Duplicate ownership across areas (5.3) can cause two implementations of the same trait or table.
7. Anime depends on libmpv. If libmpv is missing at runtime, anime playback is unavailable. Covered by area:45-player-core.

## 7. Definition of done for this area

Observable in the running Kurobayomi build:

1. `cargo test -p kb-source -p kb-ext` passes. It includes the E12 tests, and the `source_id` golden vectors were produced by running the Kotlin `generateId` (not hand-written).
2. Importing an Aniyomi backup: a library entry whose `source` id matches a built-in source opens that source's details and chapter list without remapping.
3. A backup entry with an id that matches nothing shows the source name and language stored in the backup, labelled "not installed". Reader, search and refresh are disabled, and the message key `source_not_installed` is shown.
4. Installing a WASM package whose `component.wasm` fails the sha256 check fails with the checksum message, and `<data>/extensions/<pkg>/` contains no new files.
5. An install that lowers `version_code`, or whose `key_id` differs from the installed one, is refused, with the reason shown.
6. With two repos where one returns HTTP 500, the other repo's entries are still listed, and no toast is shown during background refresh.
7. The three observed Aniyomi index entries (fixture `crates/kb-ext/tests/fixtures/aniyomi-index.min.json`) appear as unsupported APK entries. The app does not crash.
8. When a repo lists a higher `version_code` for an installed package, the Extensions badge shows the count, and the count returns to zero when the update is installed.
9. A WASM guest in an infinite loop is stopped within 5 seconds with an extension timeout error, and other sources keep working.
10. A WASM guest fetching a host not in its `allowed_hosts` gets a network error that says "not in allowed_hosts".
11. The built binary contains no APK, PackageInstaller, Shizuku or NanoHTTPD code paths. Check with `grep` on the source tree for `PackageInstaller`, `Shizuku`, `NanoHTTPD`, `DownloadManager`, and expect no matches.
12. One scraper source defined in TOML returns the same popular list, chapter list and page list as a recorded Kotlin `ParsedHttpSource` run over the same fixture HTML.
