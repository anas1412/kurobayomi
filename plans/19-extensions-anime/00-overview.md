# 19-extensions-anime: overview

Area id: `19-extensions-anime`. Kotlin root: `/home/blackbox/aniyomi-src` (every path below is repo-relative to it). Rust target: `/home/blackbox/kurobayomi` (single crate `kurobayomi`, edition 2024, `gtk4` 0.11 with feature `v4_16`). Parts: 1 of 1.

Status of this document: the area plan is complete (10 documents, 75 tasks). Section 4 lists the decisions that need the user's answer. Nothing in this file is decided until the user answers those questions.

---

## 1. Purpose and scope

**What Aniyomi does here.** The anime extension system discovers installed anime extension packages, checks each one (container, library version, signer, trust), loads its sources, and keeps three lists: installed, untrusted, and available. Available entries come from anime *stores* (remote index documents in JSON, protobuf, or legacy form). The system also downloads, verifies, places, updates, and uninstalls packages, runs an update check once a day, and backs up the configured stores and trust decisions. The Stores page (add, delete, refresh, deep link) is the user-facing part.

Scope of the Kotlin code: 37 files, 3457 LOC (see [p1-inventory.md](p1-inventory.md)). Breakdown by Kotlin area:

| Kotlin area (repo-relative folder) | Files | LOC |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/extension/anime/` (manager, API, loader, installer, installer variants, load result) | 11 | 1978 |
| `domain/src/main/java/eu/kanade/tachiyomi/extension/anime/model/` (extension model) | 1 | 79 |
| `app/src/main/java/eu/kanade/domain/extension/anime/` (browse interactors, trust interactor, buckets) | 5 | 164 |
| `domain/src/main/java/mihon/domain/extension/anime/` (store model, repository interface, five store interactors) | 7 | 94 |
| `data/src/main/java/mihon/data/extension/anime/` plus `data/src/main/sqldelightanime/dataanime/extension_store.sq` (store service, repository, wire models, SQL) | 5 | 420 |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/` (Stores page, screen model, components, dialogs) | 5 | 650 |
| Backup creator, backup model, backup restorer for stores | 3 | 72 |

**Why it matters for a usable Kurobayomi.** A user cannot get a new anime source into Kurobayomi without this area. The builtin registry in area:16-source-api-anime starts empty (task T-16a-038, see [area:16-source-api-anime](../16-source-api-anime/00-overview.md)). So the install path for `.kuroext` packages, the trust decision, and the store list are the only route to a playable source that is not compiled in. The Browse tab (area:41-ui-browse-anime) reads the buckets this area builds.

**In scope.** Models; `.kuroext` package reading and verification; loader ordering; trust storage and rules; store protocol (JSON, protobuf, legacy list and legacy repo); store persistence; Available aggregation; reconciliation; browse buckets, languages, source toggles; installer download, placement, queue, cancel, uninstall; update check and notification; store actions; stores page and its three dialogs; deep link; backup of stores and trust entries; logging keys and user-visible strings for this scope.

**Out of scope here.** Anime source API and video pipeline (area:16-source-api-anime). Manga extensions and the shared `.kuroext` container for manga (area:18-extensions-manga). Choice of the extension mechanism (area:20-extension-strategy-redesign). HTTP client (area:12-network-stack). Browse screen (area:41-ui-browse-anime). Backup file format (area:21-backup-format-and-create, area:22-backup-restore). Notification channel plumbing (area:35-data-misc-updater-notification-export). Shell, deep link registration and `.desktop` file (area:36-ui-shell-navigation-security-webview).

**What the user loses on Linux.** Every Aniyomi extension that is an Android APK. The APK loader, the Android package installer, Shizuku, and the Android broadcast system are dropped (section 9 of [p1-rust-design.md](p1-rust-design.md)). APK entries stay visible in the store lists, marked "Not runnable on Linux", with install disabled (design decision in [p1-data-and-config.md](p1-data-and-config.md) section 8.3). This is the largest user-visible change in the whole area. See open question Q1.

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities

| Package or file (repo-relative) | Responsibility | Port decision summary |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt` (397 LOC) | Central state owner: installed, untrusted, available maps; reconciliation; install, update, uninstall, trust entry points; icon lookup; update count | Rewrite. One owner on the GTK main thread. Events replace broadcasts. Sticky-obsolete defect fixed. |
| `app/src/main/java/eu/kanade/tachiyomi/extension/anime/api/AnimeExtensionApi.kt` (80 LOC) | Store listing fetch and once-per-run update check with notification | Rewrite. Throttle kept (24 h), key renamed to `anime_ext_last_check`. |
| `app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt` (442 LOC) | Package discovery, shared versus private selection, ordered checks, source instantiation, private file install | Rewrite. Manifest-only discovery. Ordered checks ported. Class loading and PackageManager dropped. |
| `app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt` (327 LOC) | Download, dispatch by installer preference, cancel, uninstall, step updates | Rewrite (download, placement). Drop (DownloadManager, receiver, Android dispatch). |
| `app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/InstallerAnime.kt` (170 LOC) | Serial install queue with one active entry and cancel semantics | Rewrite. FIFO plus token cancel. Cancel emits Idle (fixes stuck row). |
| `app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/PackageInstallerInstallerAnime.kt` (117 LOC) | Android PackageInstaller session installer | Drop. |
| `app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/ShizukuInstallerAnime.kt` (124 LOC) | Shizuku and `pm install-*` installer | Drop. |
| `app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallReceiver.kt` (147 LOC) | Package add, replace, remove broadcasts | Drop. Replaced by direct events. |
| `app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallService.kt` (88 LOC) | Foreground install service, notification -402 | Drop. In-page progress instead. |
| `app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallActivity.kt` (79 LOC) | System installer Activity and MIUI workaround | Drop. |
| `app/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeLoadResult.kt` (7 LOC) | `Success`, `Untrusted`, `Error` load result | Port (enum). |
| `domain/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeExtension.kt` (79 LOC) | `Installed`, `Available` (with nested `AnimeSource`), `Untrusted` | Rewrite as structs with explicit kind and Option fields. |
| `app/src/main/java/eu/kanade/domain/extension/anime/interactor/TrustAnimeExtension.kt` (32 LOC) | Trust rule and `trusted_extensions` writer | Port. Digest source changes from APK certificate to ed25519 publisher key. |
| `app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionsByType.kt` (57 LOC) | Browse buckets with NSFW filter and sorting | Rewrite as pure function. |
| `app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionSources.kt` (37 LOC) | Per-source enabled state and label rule | Port. |
| `app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionLanguages.kt` (28 LOC) | Language filter list | Port. ICU-free comparator from area:02. |
| `app/src/main/java/eu/kanade/domain/extension/anime/model/AnimeExtensions.kt` (10 LOC) | Browse bucket value type | Port (struct). |
| `domain/src/main/java/mihon/domain/extension/anime/model/AnimeExtensionStore.kt` (16 LOC) | Store domain model | Port (`StoreRow`). |
| `domain/src/main/java/mihon/domain/extension/anime/repository/AnimeExtensionStoreRepository.kt` (23 LOC) | Repository contract | Rewrite as one concrete repository type. |
| `domain/src/main/java/mihon/domain/extension/anime/interactor/` (Add 11, GetCount 9, Get 13, Remove 11, Update 11 LOC) | Store use cases | Port. |
| `data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt` (122 LOC) | HTTP fetch, format detection, legacy redirects, extension list branches | Rewrite. reqwest, flate2, serde, prost. Redirect depth limit added. |
| `data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt` (127 LOC) | Store table access and aggregate fetch | Rewrite on rusqlite with per-store results. |
| `data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt` (120 LOC) and `BaseNetworkAnimeExtensionStore.kt` (7 LOC) | Wire model for store documents and its mapping | Rewrite. serde for JSON, prost for protobuf, same tags. |
| `data/src/main/sqldelightanime/dataanime/extension_store.sq` (44 LOC) | `extension_store` table and named queries | Port (SQL kept as constants). SQLDelight dropped. |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreen.kt` (85 LOC) and `AnimeExtensionStoresScreenModel.kt` (166 LOC) | Stores page entry, deep-link handling, dialog routing, store actions | Rewrite as GTK page and service calls. |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/` (`AnimeExtensionStoresScreen.kt` 80, `AnimeExtensionStoresContent.kt` 127, `AnimeExtensionStoresDialogs.kt` 192 LOC) | Scaffold, list rows, three dialogs | Rewrite as GTK widgets. |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeExtensionStoresBackupCreator.kt` (17 LOC), `.../data/backup/models/BackupExtensionStore.kt` (30 LOC), `.../data/backup/restore/restorers/AnimeExtensionStoreRestorer.kt` (25 LOC) | Store backup export, model (protobuf tags 1 to 8), restore | Port. |

### 2.2 Proposed Rust module map (Kurobayomi)

Source: [p1-rust-design.md](p1-rust-design.md) section 1. Everything lives under `src/extensions/anime/`.

```
src/extensions/anime/
  mod.rs          facade: AnimeExtensions service handle, re-exports
  model.rs        Installed, Available, Untrusted, Kind, LoadResult, InstallStep, StoreRef, StoreRow, Buckets
  abi.rs          AbiVersion (major, minor), SUPPORTED_ABI, parse rules
  wire.rs         serde structs (store JSON, legacy JSON), prost structs (protobuf)
  store/
    mod.rs        IndexFetcher: fetch(), extension list, redirects, gzip
    decode.rs     first-byte format detection, gzip sniffing
    map.rs        wire -> Available mapping (lang, nsfw, abi, sources, kuroext block)
    repo.rs       rusqlite access (get, list, count, upsert, delete), SQL constants
    refresh.rs    refresh_all, fetch_all_available, add_store
  package/
    mod.rs        KuroextPackage: open zip, read manifest, validate
    manifest.rs   Manifest struct and validation rules
    verify.rs     sha256 of file, ed25519 of digest, signer digest
  loader.rs       discover installed dir, select, load -> LoadResult
  trust.rs        trust rule, trust entry format, revoke
  install/
    mod.rs        InstallService: queue, steps, cancel
    download.rs   reqwest streaming download to .part, progress, cancel token
    place.rs      downgrade and signer-continuity checks, fsync, atomic rename
  uninstall.rs    remove installed file, emit event
  manager.rs      AnimeExtensionManager: state maps, events, reconciliation, store actions
  browse.rs       buckets, sorting, NSFW filter, split rows, languages, source toggles
  updates.rs      update check, update count, notification
  icons.rs        icon cache paths
  backup.rs       BackupExtensionStore <-> StoreRow mapping
  deeplink.rs     aniyomi://add-repo and aniyomi://extension-store
src/extensions/ui/anime_stores.rs        stores page
src/extensions/ui/anime_store_dialogs.rs create, confirm (deep link), delete dialogs
src/extensions/ui/anime_trust.rs         untrusted dialog
```

Crate choices are in section 13 of [p1-rust-design.md](p1-rust-design.md): `tokio`, `reqwest`, `serde`/`serde_json`, `prost`, `flate2`, `zip`, `sha2`, `ed25519-dalek` (with `verify_strict`), `base64`, `hex`, `url`, `rusqlite`, `async-channel`, `tokio-util`, `thiserror`, `tracing`, `gtk4` 0.11, and `wasmtime` (decided in area:18 and area:20, not in this area). Dropped: `async-trait`, `anyhow`, `directories`.

Android-to-Linux mapping for every touched API: [p1-rust-design.md](p1-rust-design.md) section 9. Threading model (GTK main thread owns state; tokio workers send events over `async_channel`): section 10.

---

## 3. Reading guide

Read in this order: this overview, then [p1-inventory.md](p1-inventory.md) (what each file does and where it goes), then [p1-behavior.md](p1-behavior.md) (what the Kotlin does), then [p1-rust-design.md](p1-rust-design.md) (what Rust does), then [p1-tasks.md](p1-tasks.md).

| Document | What it contains |
|---|---|
| [p1-inventory.md](p1-inventory.md) | The 37 in-scope files with LOC, every declaration, port decision, target Rust module, priority and task IDs. Also the callee and caller files read for exactness, and the tests in scope (none). |
| [p1-behavior.md](p1-behavior.md) | Observed behavior of the Kotlin code, section by section: model and lifecycle, loader checks in order, private install file, install receiver semantics, manager operations, trust, store protocol, installer, install queue, dropped installer variants, update check API, store actions, browse. Abbreviations for file references are in its section 0. |
| [p1-data-and-config.md](p1-data-and-config.md) | Data models, wire formats (JSON and protobuf field tags), preference keys, files on disk (Aniyomi and Kurobayomi), network endpoints, SQL, the Kurobayomi `manifest.json` and index extension, backup model, exact constants table (section 10), logging keys (section 11), user-visible strings (section 12), fields not carried over (section 13). |
| [p1-rust-design.md](p1-rust-design.md) | Rust module layout, core types, store client, package container and verification, trust, loader, install pipeline and queue, manager, reconciliation, update checks, Android-to-Linux mapping, threading, UI integration, error handling, crates, pitfalls, test hooks. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge-case register (E-LDR, E-STO, E-REC, E-REP, E-INS, E-TRU, E-UPD, E-UI, E-BCK and others), failure modes, race conditions, the test-case list (TST-19-NNN, 117 IDs), fixtures, coverage map, and known gaps. |
| [p1-tasks.md](p1-tasks.md) | 75 implementation tasks (T-19a-001 to T-19a-075) in sections A to H, each with priority, effort, dependencies, sources and acceptance criteria. |
| [p1-topic-store-index-protocol.md](p1-topic-store-index-protocol.md) | Deep dive: the four store document shapes, fetch sequence, detection rules, mapping rules, error texts, limits. |
| [p1-topic-install-and-trust.md](p1-topic-install-and-trust.md) | Deep dive: install pipeline overview, downloads, verification order, placement, package kinds, queue and cancel, uninstall, trust, failure matrix. |
| [p1-topic-browse-and-reconciliation.md](p1-topic-browse-and-reconciliation.md) | Deep dive: state maps, reconciliation rule, update count, browse buckets, languages, source toggles, lookup by source id, ordering, user-visible effects of actions. |
| [p1-ui-parity.md](p1-ui-parity.md) | Stores page and dialogs, field by field, with Kotlin file line references; decisions K-1 to K-11; deep link; keyboard and responsive notes; mockup gap list; strings. |

---

## 4. Key design decisions and open questions

### 4.1 Decisions already taken in the plan (accepted by this overview)

| # | Decision | Where |
|---|---|---|
| D1 | Only `.kuroext` packages are installable. Aniyomi APK entries are listed and marked "Not runnable on Linux". | [p1-data-and-config.md](p1-data-and-config.md) 8.3 |
| D2 | Installed packages live in `$XDG_DATA_HOME/kurobayomi/extensions/anime/<pkg>.kuroext`, mode `0444`, written by `.part` then fsync then rename. | [p1-data-and-config.md](p1-data-and-config.md) 5.2, 5.3 |
| D3 | Verification order: sha256 of file, publicKey decodes to 32 bytes, SHA-256 of publicKey equals store `signingKey`, ed25519 `verify_strict` over the raw 32-byte digest, manifest validation, manifest `pkg` equals store `packageName`. | [p1-topic-install-and-trust.md](p1-topic-install-and-trust.md) 3 |
| D4 | Trust key is `"<pkg>:<versionCode>:<signer digest>"`. Revoke-all is kept. | [p1-topic-install-and-trust.md](p1-topic-install-and-trust.md) 8 |
| D5 | Signer continuity is equality with the current signer. Key rotation is not supported. A rotated key needs a new package name. | [p1-topic-install-and-trust.md](p1-topic-install-and-trust.md) 4 |
| D6 | Store parsing keeps all four shapes (JSON store, protobuf store, legacy list, legacy repo) so existing store URLs still parse. Legacy repo support is P2. | [p1-topic-store-index-protocol.md](p1-topic-store-index-protocol.md) 1 |
| D7 | Store redirect depth limit: 4. A fifth redirect is `RedirectLoop`. Aniyomi has no limit (E-STO-04). | [p1-topic-store-index-protocol.md](p1-topic-store-index-protocol.md) 7 |
| D8 | NSFW is `contentWarning >= MIXED` on both the store path and the loader path. The loader path was looser in Aniyomi. | [p1-topic-store-index-protocol.md](p1-topic-store-index-protocol.md) 5 |
| D9 | Reconciliation resets `isObsolete` when a package is found again. Zero-source extensions stay in the Browse list. | [p1-topic-browse-and-reconciliation.md](p1-topic-browse-and-reconciliation.md) 2, 4 |
| D10 | Update count is in memory only and excludes obsolete packages. | [p1-topic-browse-and-reconciliation.md](p1-topic-browse-and-reconciliation.md) 3 |
| D11 | The update check runs on start and at most once per 24 h (`86,400,000` ms gate). A monotonic `Instant` drives the schedule and a wall clock is stored only for the last-check time. | [p1-data-and-config.md](p1-data-and-config.md) 10; [p1-rust-design.md](p1-rust-design.md) section 14, pitfall 15 |
| D12 | Stores page: header buttons `Add` and `Refresh` replace the FAB (K-1). Refresh runs the full refresh, not only store rows (K-5). Per-store failures show one banner each (K-6). | [p1-ui-parity.md](p1-ui-parity.md) 6 |
| D13 | Language comparator is ICU-free, with a small ISO 639-1 table and `"all"` first. | [p1-rust-design.md](p1-rust-design.md) 9 |
| D14 | Store refresh runs at most 4 fetches at once. Downloads are not capped. Placement is serialised. | [p1-rust-design.md](p1-rust-design.md) 10 |

### 4.2 Open questions for the user

Each question has a recommendation. Nothing below is settled until the user answers.

**Q1. Accept that no existing Aniyomi APK extension runs on Linux?**
- Facts: every Aniyomi anime extension is an APK. Kurobayomi cannot load APKs (design section 9). Those entries stay listed as "Not runnable on Linux".
- Options: (a) accept, and rely on `.kuroext` sources; (b) run APKs through an Android compatibility layer (out of scope, not planned anywhere in the plans).
- Recommendation: (a). Record the loss in the user-facing release notes.

**Q2. Is the `.kuroext` install path needed for the first usable release?**
- Facts: area:20-extension-strategy-redesign section 4 (Q3) recommends that phase 0 uses built-in sources only and that WASM is not needed first. area:16-source-api-anime Q1 recommends one first-party source written as `.kuroext` so the loop (repo, index, verify, install, load, browse) is exercised end to end.
- Recommendation: keep the loader, verification, and placement tasks (T-19a-004, 005, 006, 011, 033, 031, 032) in P0 only if the user wants third-party installs in the first release. If the first release ships built-in and first-party sources only, move the store-browsing chain (T-19a-013 to T-19a-021) and the installer UI to P1, and keep a file-based install (`.kuroext` from disk) as P0.

**Q3. Package format: which format is authoritative?**
- Facts: this area and area:18-extensions-manga both use a `.kuroext` zip with `manifest.json`. area:20-extension-strategy-redesign section D8 uses a directory package with `manifest.toml`.
- Recommendation: `.kuroext` zip with `manifest.json`. Two areas already agree on it. area:20 D7 and D8 must be rewritten to match.

**Q4. ABI version string: `1.0` or `1.4`?**
- Facts: this area accepts `["1.0"]` (data section 8.1, rust design 4.5, tasks T-19a-002). area:16-source-api-anime and area:20 name the WIT world `kurobayomi:source@1.0.0`. area:18-extensions-manga section 7.3 uses `"abi": "1.4"` in its manifest example.
- Recommendation: `1.0`, matching the WIT package version. area:18 must change its example and its accepted set.

**Q5. Where is the signer digest stored after install?**
- Facts: this area writes a sidecar `<pkg>.signer` next to the installed file (inventory row for `place.rs`; behavior 3). area:18-extensions-manga section 7.3 writes `signerPub` into the installed copy's `manifest.json`.
- Recommendation: sidecar. Rewriting the manifest changes the installed bytes, so the file no longer matches the sha256 that was verified. area:18 must drop the "installer overwrites `signerPub`" rule.

**Q6. Maximum size of `source.wasm`: 16 MiB or 32 MiB?**
- Facts: this area caps `source.wasm` at 16 MiB (rust design pitfall 6). area:18-extensions-manga section 7.3 caps it at 32 MiB.
- Recommendation: 32 MiB for both kinds, since the container is shared. Change pitfall 6.

**Q7. Naming of the unsigned state: `Unsigned` or `Untrusted`?**
- Facts: this area lists `kuroext` entries with missing `ed25519` or `publicKey` as `Unsigned` (data 8.2). area:18-extensions-manga names the same case `Untrusted` (section 16.2), and this area uses `Untrusted` for valid but not yet trusted signers.
- Recommendation: `Unsigned` for missing or malformed signature data, and `Untrusted` only for a valid signature from a signer the user has not trusted. Two distinct states. area:18 must align.

**Q8. Priority inversion in the P0 chain.**
- Facts: nine tasks that the P0 chain depends on are P1 or P2: T-19a-003 (P1), T-19a-015 (P1), T-19a-016 (P2), T-19a-018 (P1), T-19a-019 (P1), T-19a-020 (P1), T-19a-023 (P1), T-19a-047 (P1), T-19a-048 (P1). T-19a-069 (P0 smoke test) depends on T-19a-048.
- Recommendation: promote T-19a-003, 015, 018, 019, 020, 023 and 047 to P0. Make T-19a-016 P1 and remove it from the dependency of T-19a-018 by making the legacy branch return `Err(LegacyUnsupported)` until T-19a-016 lands. Keep T-19a-048 at P1 and move T-19a-069 to P1, or promote T-19a-048 if the smoke test stays in P0.

**Q9. Legacy store URLs.**
- Facts: the legacy path (`/index.min.json` plus `/repo.json`) is the format of older Aniyomi repos. Its lists are APK-only (see Q1).
- Recommendation: keep parsing so that users' saved URLs are not broken, as P2 (T-19a-016). Do not spend effort on legacy list install.

**Q10. Store fetch cap (4 concurrent) and redirect depth (4).**
- Facts: both are Proposal values, not Aniyomi behavior.
- Recommendation: keep both. They are simple to change later.

**Q11. Downloads: no size cap beyond 64 MiB?**
- Facts: this area caps downloads at 64 MiB (Proposal). A `.kuroext` package is at most 16 MiB of WASM plus manifest and icon.
- Recommendation: keep 64 MiB as one cap for the download. Size of each entry is checked after download.

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What is needed | Tasks in this area that need it |
|---|---|---|
| [area:01-foundation-app-bootstrap](../01-foundation-app-bootstrap/00-overview.md) | `Paths` trait for `$XDG_DATA_HOME`, `$XDG_CACHE_HOME`, `$XDG_CONFIG_HOME` roots (`plans/addenda/requirements-cross-platform.md`). | T-19a-005, T-19a-031, T-19a-059 |
| [area:02-preferences-system](../02-preferences-system/00-overview.md) | `state.toml` store for trust, store list keys, enabled languages, last-check key; ICU-free language comparator; `LANG` and `LC_MESSAGES` parsing. | T-19a-001, T-19a-009, T-19a-028, T-19a-064 |
| [area:04-db-anime-schema-migrations](../04-db-anime-schema-migrations/00-overview.md) | Connection handling for `rusqlite`; the `extension_store` table migration. | T-19a-019, T-19a-022 |
| [area:11-app-migrations](../11-app-migrations/00-overview.md) | Migration version 7 inserts legacy repos (`TrustExtensionRepositoryMigration.kt`). | T-19a-022 |
| [area:12-network-stack](../12-network-stack/00-overview.md) | Shared `reqwest` client, default headers, cache policy, timeouts. | T-19a-013, T-19a-031 |
| [area:13-core-util-storage-archive-torrent](../13-core-util-storage-archive-torrent/00-overview.md) | SHA-256 helper, atomic file helpers, read-only file helper. | T-19a-011, T-19a-033, T-19a-045 |
| [area:16-source-api-anime](../16-source-api-anime/00-overview.md) | ABI set and WIT world (`kurobayomi:source@1.0.0`); `AnimeSource` id and name; stub source type; builtin registry. | T-19a-002, T-19a-030, T-19a-070 |
| [area:18-extensions-manga](../18-extensions-manga/00-overview.md) | The shared `.kuroext` container reader and manifest rules; the verification rules that are shared by manga. | T-19a-004, T-19a-011, T-19a-070 |
| [area:20-extension-strategy-redesign](../20-extension-strategy-redesign/00-overview.md) | The decision on the extension mechanism. Decides whether the loader and install chain (Q2 above) is P0 at all. | T-19a-004 to T-19a-006 and T-19a-031 to T-19a-033 (see Q2) |
| [area:35-data-misc-updater-notification-export](../35-data-misc-updater-notification-export/) | Notification channel and `gio::Notification` plumbing. | T-19a-053, T-19a-075 |
| [area:36-ui-shell-navigation-security-webview](../36-ui-shell-navigation-security-webview/) | Deep link registration (`x-scheme-handler/aniyomi`), main-window start, update check at start, `.desktop` file. | T-19a-050, T-19a-052 |
| [area:47-settings-screens-general](../47-settings-screens-general/) | Settings entry to the stores page and the revoke-trust row. | T-19a-010, T-19a-056 |

### 5.2 What others need from this area

| Area | What it uses from 19 | Tasks in this area it depends on |
|---|---|---|
| [area:41-ui-browse-anime](../41-ui-browse-anime/) | Browse buckets, language list, source toggles, install and update actions, trust dialog. The Browse P0 tasks T-41a-004, 009, 011, 012, 013, 015 and 016 depend on this area, so this area is on the Browse critical path. | T-19a-027, T-19a-028, T-19a-029, T-19a-051 |
| [area:16-source-api-anime](../16-source-api-anime/00-overview.md) | Source id lookup for installed and stub handles, and the loaded source instances. | T-19a-030, T-19a-006 |
| [area:21-backup-format-and-create](../21-backup-format-and-create/00-overview.md) (T-21a-017) and [area:22-backup-restore](../22-backup-restore/00-overview.md) (T-22a-026) | `BackupExtensionStore` protobuf model with tags 1 to 8; restore mapping and defaults; trust entries. | T-19a-054, T-19a-065 |
| [area:36-ui-shell-navigation-security-webview](../36-ui-shell-navigation-security-webview/) (T-36a-013, 014, 022) | Deep-link handler (`deeplink.rs`), the Confirm dialog, and the update check on start. | T-19a-050, T-19a-052 |
| [area:10-app-domain-layer](../10-app-domain-layer/00-overview.md) (T-10a-023), [area:39-ui-entry-details-anime](../39-ui-entry-details-anime/) (T-39a-057), [area:52-util-app](../52-util-app/) (T-52a-019) | Installed and available package data for the anime entry screen and shared utilities. | T-19a-001, T-19a-046 |
| [area:47-settings-screens-general](../47-settings-screens-general/) (T-47a-062, T-47a-084, T-47b-009, T-47b-011) | Settings rows that open the stores page and manage trust. Two of these are P0. | T-19a-010, T-19a-056 |
| [area:35-data-misc-updater-notification-export](../35-data-misc-updater-notification-export/) (T-35a-085) | Notification plumbing. | T-19a-053 |

### 5.3 Cross-area conflicts that must be resolved

These are listed in section 4.2 (Q3 to Q7). Summary: format (`manifest.toml` versus `manifest.json`), ABI string (`1.0` versus `1.4`), signer storage (sidecar versus embedded), wasm size cap (16 versus 32 MiB), and the unsigned state name.

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then the rows of `TASKS.csv` whose file starts with `19-extensions-anime/`. The file [p1-tasks.md](p1-tasks.md) lints with `ok`.

**Totals: 75 tasks.** Priority: P0 19, P1 33, P2 9, P3 14. Effort: S 57, M 18, L 0, XL 0.

| Priority | S | M | Total |
|---|---|---|---|
| P0 | 9 | 10 | 19 |
| P1 | 27 | 6 | 33 |
| P2 | 7 | 2 | 9 |
| P3 | 14 | 0 | 14 |
| **Total** | **57** | **18** | **75** |

**P0 tasks (19).** T-19a-001, 002, 004, 005, 006, 009, 011, 012, 013, 017, 021, 024, 025, 030, 031, 032, 033, 061, 069.

**P0 critical path, in order.** Effort is the upper bound (S about 1 day, M about 3 days), so the sum is 21 working days. Realistic is 2 to 3 weeks with one person.

1. T-19a-001 Anime extension model types (S). Needs area:02.
2. T-19a-002 AbiVersion and the supported ABI set (S).
3. T-19a-004 `.kuroext` container reader and manifest validation (M). Needs area:18.
4. T-19a-011 Package verification: digest, publicKey, ed25519, store signer (M). Also needs T-19a-009 (trust, S) and T-19a-013 (store JSON, M, needs area:12).
5. T-19a-006 Loader pipeline with ordered checks (M).
6. T-19a-023 Manager state, events, single writer (M). P1 today (Q8).
7. T-19a-031 Download step (M). Needs area:12.
8. T-19a-032 Install pipeline state machine (M). Also needs T-19a-033 (placement, M).
9. T-19a-069 End-to-end smoke test (S). Needs T-19a-021, 024, 032, 048.

Parallel track for the store: T-19a-013, 017 (S), 018 (S, P1 today), 019 (S, P1 today), 020 (S, P1 today), 021 (M), 025 (M, reconciliation). T-19a-021 needs T-19a-018, 020 and 017.

**P1 tasks (33)** cover: lib and NSFW and torrent metadata (T-19a-003, 007), trust mutations (T-19a-010), gzip and store list branches (T-19a-015, 018), store table and insert semantics (T-19a-019, 020), manager concurrency and update count (T-19a-023, 026), browse buckets, languages and source toggles (T-19a-027, 028, 029), install queue, cancel and uninstall (T-19a-034, 035, 036), icons (T-19a-046), store actions, stores page, dialogs and deep link (T-19a-047 to 051, 055 to 058), update check and notification (T-19a-052, 053), backup of stores (T-19a-054), runtime wiring and error mapping (T-19a-059, 060), tests (T-19a-062, 063), and the cross-area notes (T-19a-070).

**P2 tasks (9)**: torrent flag (T-19a-008), protobuf store and list decode (T-19a-014), legacy repo and legacy list (T-19a-016), legacy migration rows (T-19a-022), preference and state keys (T-19a-064), legacy `anime_extension_repos` import and backup trust entries (T-19a-065), show "not runnable" packages on demand (T-19a-066), keyboard, responsive layout and tooltips (T-19a-067), protobuf golden and fuzz tests (T-19a-068).

**P3 tasks (14)** are drops. Each states the replacement or the drop decision: T-19a-038 (PackageInstaller), 039 (Shizuku), 040 (system install Activity), 041 (foreground service), 042 (DownloadManager), 043 (broadcast replaced by events), 044 (class loading), 045 (PackageManager, signatures, FileProvider), 071 (verify no Android broadcast or intent code remains), 072 (`hasReadme`, `hasChangelog`, `pkgFactory`), 073 (`isShared` location), 074 (`ACTION_UNINSTALL_PACKAGE` for system packages), 075 (notification channel, foreground, text formats).

**Tests.** 117 test IDs (TST-19-NNN) in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md). Pure functions (store decode and map, manifest, verify, loader, buckets, reconciliation) are tested without GTK. GTK UI is covered by a smoke run only.

### 6.1 Risks

1. **Catalog loss (product).** Users keep none of their existing Aniyomi APK extensions. Mitigation: first-party `.kuroext` sources (area:16 Q1) and a clear "Not runnable on Linux" label. Owner decision: Q1.
2. **Cross-area format drift.** Q3 to Q7 show four conflicts. Until they are resolved, verification and loader tests can be written against the wrong fixture. Mitigation: resolve before T-19a-004 starts.
3. **Priority inversion (Q8).** The P0 chain pulls in P1 and P2 tasks. Mitigation: promote or cut the links before scheduling.
4. **Schedule.** The P0 chain is serial (section 6, critical path). A single slip in T-19a-004 or T-19a-006 slips the whole P0 chain.
5. **Security of third-party code.** A `.kuroext` package carries WASM code that runs with network access (area:18 and area:20). Mitigations in this area: sha256 and ed25519 `verify_strict` before placement; signer must match the store `signingKey`; trust is per version; zip-slip and bomb limits (design pitfall 6); no extraction to disk. Residual risk: a user who trusts a malicious publisher runs its code. The trust dialog text (section 11.3 of [p1-rust-design.md](p1-rust-design.md)) is the only warning.
6. **Atomic placement across filesystems.** `.part` must be in the same directory as the target, or `rename` fails with `EXDEV` (design pitfall 7). Covered by T-19a-033.
7. **No real Aniyomi store fixtures.** The protobuf golden bytes need a JVM to generate. Without one, the test relies on prost round-trip plus the field-tag table (edge-case file, section 7).
8. **Threading.** `AnimeExtensions` holds `Rc` and is `!Send` (design pitfall 1). A mistake here causes compile errors, not runtime bugs, so the risk is low.
9. **Wasmtime cost.** Shared with area:18 and area:20. Pin versions (area:20 risk 4).

### 6.2 Known issues in the plan text

Fixed in this pass:

- Short-form area tokens in prose (`area:18`, `area:02` and similar) were replaced with full ids in all ten documents. Dependency tokens in [p1-tasks.md](p1-tasks.md) already used full ids.
- Three stale task cross-references were corrected: the ABI pin now cites T-19a-002 (in [p1-data-and-config.md](p1-data-and-config.md) 8.1 and [p1-rust-design.md](p1-rust-design.md) 4.5), the protobuf golden test cites T-19a-068 (pitfall 4), and the redirect-loop test cites T-19a-016 and T-19a-061 (pitfall 11).

Open in the plan text (owners to fix):
- The UI file location is inconsistent. [p1-rust-design.md](p1-rust-design.md) section 1 places the stores page at `src/extensions/ui/anime_stores.rs`, while the [p1-inventory.md](p1-inventory.md) rows for the stores page and dialogs say `src/extensions/anime/ui/`. Section 2.2 above follows the design file.
- The area:18 manifest example uses `"abi": "1.4"` and an installer-written `signerPub` (see Q4 and Q5).
- T-19a-069 (P0) depends on T-19a-048 (P1). See Q8.

---

## 7. Definition of done for this area

Each item is an observable behavior on Linux in the Kurobayomi window.

1. **Add a store.** On the Stores page, `Add` opens a dialog with the label `Store index URL`. Entering a valid store URL and pressing `Add` shows the store in the list with its name, its badge label, and the index URL as a subtitle. The dialog closes.
2. **Duplicate refused.** Adding a URL that is already stored shows `This extension store already exists` and keeps `Add` disabled.
3. **Bad URL shape.** A deep link whose URL is not `http` or `https` shows the banner `This link is not an extension store address` (K-9).
4. **Refresh.** `Refresh` reloads every store document and every extension list. A store that fails shows one banner `Could not update store "<name>"`. The other stores still update.
5. **Empty state.** With no stores, the page shows `You haven't added an extension store yet`.
6. **Install a signed package.** Clicking `Install` on an available `.kuroext` entry shows the states `Downloading` and then `Installing`, then the row moves to Installed. The file exists at `$XDG_DATA_HOME/kurobayomi/extensions/anime/<pkg>.kuroext`, mode `0444`.
7. **Tampered package refused.** A package whose sha256 does not match the store entry ends the install in the Error step. Nothing is placed in the extensions directory. No `.part` file remains.
8. **Signer mismatch refused.** A package whose publicKey does not hash to the store `signingKey` is refused before the signature check.
9. **Downgrade refused.** Installing a lower `versionCode` over an installed package is refused. Reinstalling the same version is allowed.
10. **Untrusted package.** A valid package from an untrusted signer appears under Untrusted. `Trust` moves it to Installed after a reload and runs reconciliation. `Uninstall` removes it.
11. **Not runnable.** An Aniyomi APK entry shows the badge `Not runnable on Linux` and a disabled install action.
12. **Cancel.** Cancelling a download during the Downloading step returns the row to Idle. No row stays stuck.
13. **Queue.** Two installs started together run in order. Only one placement runs at a time.
14. **Update count.** With an installed package older than the store entry, the count is 1 and the package sorts under Updates. An obsolete package (no longer listed) sorts first and is not counted.
15. **Browse buckets.** Installed, Updates, Available and Untrusted lists match the rules in [p1-topic-browse-and-reconciliation.md](p1-topic-browse-and-reconciliation.md) section 4. Language and source toggles filter the Available list.
16. **Update check.** On start, the app checks the stores at most once per 24 hours. A new update count above the previous one shows one notification. The count reaching zero withdraws it.
17. **Restart.** After restart, installed packages, trust decisions and stores are unchanged. A stray `.part` file in the cache directory is removed at startup.
18. **Uninstall.** Uninstalling removes the installed file. The package returns to Available if a store still lists it.
19. **Backup round trip.** Exporting a backup with stores and trust entries, then restoring it on a clean profile, restores the same stores (with `isLegacy` defaulting to true when absent) and the same trust entries.
20. **Deep link.** Opening `aniyomi://add-repo?url=<encoded URL>` or `aniyomi://extension-store?url=<encoded URL>` opens the Confirm dialog with the URL read-only.
21. **Clipboard and browser.** `Copy to clipboard` copies the index URL and shows the banner `Copied to clipboard:` with the URL on the next line (K-4, `copied_to_clipboard` in [p1-data-and-config.md](p1-data-and-config.md) section 12). The website button opens the store website in the default browser; it is hidden when the website is empty (K-7).
22. **No Android code.** No Android package, intent, broadcast, or notification ID from the Kotlin code remains in the Rust crate (T-19a-071).
