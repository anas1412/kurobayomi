# Area 18 — Manga extensions: overview

Area id: `18-extensions-manga`. Scope: manga source extensions, from discovery and loading through repo index, install, trust, updates, the Browse data model, and notifications. Anime extensions are area:19-extensions-anime. The Kotlin code is read-only at `/home/blackbox/aniyomi-src`; every path below is repo-relative to that directory.

Status: planning. The inventory, behaviour, data, design, edge-case, UI-parity and task documents are complete for this area. Open decisions are in section 4.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here

Aniyomi (a Tachiyomi/Mihon fork) runs third-party manga sources as Android APK packages. The area covers:

1. **Discovery and loading.** At startup it finds extension packages, checks each one (lib version, signature, trust, NSFW gate), loads its source classes and exposes the result as installed extensions.
2. **Repos and index.** The user adds extension repositories (base URLs). Each repo serves `repo.json` (metadata with a signing-key fingerprint) and `index.min.json` (the list of packages). The app fetches these, filters them by lib version, and shows them as available.
3. **Install, update, uninstall.** Downloads an APK, verifies it, installs it through one of four installer paths, reports progress per package, and removes packages.
4. **Trust.** A package whose signer is not trusted (no repo key match and no per-version user trust) is listed as Untrusted until the user trusts that exact version.
5. **Update check.** Once per day (`1.days`), compares installed versions with the index and posts the "extension updates available" notification and the badge count.
6. **Browse data.** Four buckets (updates, installed, available, untrusted), the language filter, and per-source enable state.

Source of truth for the observed behaviour: `app/src/main/java/eu/kanade/tachiyomi/extension/manga/` and `app/src/main/java/eu/kanade/domain/extension/manga/` (see section 2.1).

### 1.2 Why it matters for a usable Kurobayomi

A manga reader with no sources shows an empty library. Every series, chapter list and page image comes from a source. This area is the only way third-party sources reach the app, and the only place where trust decisions are made for them. Without it, Kurobayomi can run only built-in sources (if any exist; see open question D1 in section 4).

### 1.3 What changes on Linux (headline)

Android APK extensions cannot run on Linux. The design therefore replaces the runtime, not the code:

- Decided (design section 2): a Kurobayomi extension is a `.kuroext` zip file holding `manifest.json`, `source.wasm` (WebAssembly component, run by `wasmtime`) and optional `icon.png`. Its network access goes only through a host HTTP bridge with a per-package host allow-list.
- Consequence: **no existing Aniyomi APK extension works on Kurobayomi.** The index still lists them, marked `Not runnable on Linux` (behaviour section 16.2). The user loses every installed or repo-listed Aniyomi APK source (behaviour section 15, first row). This is the largest user-visible loss in the area and is open question D1.
- Replacement trust model: the APK certificate SHA-256 becomes the SHA-256 of an ed25519 public key that signs the package. Repo fingerprints keep the same format (`sha256` hex lowercase), so the repo rule carries over (`p1-behavior.md` section 16.4 and 16.5).

Full list of dropped mechanisms with the user loss for each: [p1-behavior.md](p1-behavior.md) section 15, and the dropped UI surfaces in [p1-ui-parity.md](p1-ui-parity.md) section 6.

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities (OBSERVED)

Total: 19 files, 2,387 LOC (inventory [p1-inventory.md](p1-inventory.md)). The package totals below add up to 2,387.

| Package (repo-relative) | Files | LOC | Responsibility |
|---|---|---|---|
| `app/src/main/java/eu/kanade/domain/extension/manga/interactor/` | `GetMangaExtensionLanguages.kt`, `GetMangaExtensionSources.kt`, `GetMangaExtensionsByType.kt`, `TrustMangaExtension.kt` | 161 | Browse views: language list, per-extension source list, the four buckets, the trust decision and trust actions. |
| `app/src/main/java/eu/kanade/domain/extension/manga/model/` | `MangaExtensions.kt` | 10 | Result bundle of the Browse view (`updates`, `installed`, `available`, `untrusted`). |
| `app/src/main/java/eu/kanade/tachiyomi/extension/` | `ExtensionUpdateNotifier.kt`, `InstallStep.kt` | 64 | Update notification (shared with anime); install step enum (`Idle, Pending, Downloading, Installing, Installed, Error`). |
| `app/src/main/java/eu/kanade/tachiyomi/extension/manga/` | `MangaExtensionManager.kt` | 392 | Central state: installed, available and untrusted maps; init; find, install, update, uninstall, trust; update counts; stub-source map. |
| `app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/` | `MangaExtensionApi.kt` | 171 | Index fetch per repo, lib-version filter, mapping to Available, daily update check. |
| `app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/` | `InstallerManga.kt`, `PackageInstallerInstallerManga.kt`, `ShizukuInstallerManga.kt` | 426 | Android install backends and the serial install queue base class. |
| `app/src/main/java/eu/kanade/tachiyomi/extension/manga/model/` | `MangaExtension.kt`, `MangaLoadResult.kt` | 81 | Extension value model (`Installed`, `Available`, `Untrusted`) and load outcome (`Success`, `Untrusted`, `Error`). |
| `app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/` | `MangaExtensionInstallReceiver.kt`, `MangaExtensionInstallService.kt`, `MangaExtensionInstaller.kt`, `MangaExtensionLoader.kt`, `MangaExtensionInstallActivity.kt` | 1,082 | Package-change broadcasts, install foreground service, DownloadManager-based installer, the loader (discovery, selection, validation, class loading, private file install), the system install Activity. |

Files outside this area that this area uses are listed in [p1-inventory.md](p1-inventory.md) under "Scope notes": repo DTO and repo interactors (`mihon/domain/extensionrepo/manga/`), the `extension_repos` table (`data/src/main/sqldelight/data/extension_repos.sq`), preference keys, `NetworkHelper`, and the source API.

### 2.2 Proposed Rust crates and modules (Proposal)

Source: [p1-rust-design.md](p1-rust-design.md) section 3. Workspace root is the existing `kurobayomi` package (`gtk4 0.11.5` with feature `v4_16`, edition 2024).

```
Cargo.toml                       (workspace; members: crates/kuro-extensions, crates/kuro-source-wasm)
crates/
  kuro-extensions/               no GTK, no wasmtime; tokio, reqwest, rusqlite, zip, ed25519-dalek, sha2
    src/model.rs                 InstalledExtension, AvailableExtension, UntrustedExtension, SourceInfo, PackageKind, LoadResult, LibVersion
    src/manifest.rs              PackageManifest, validation (name prefix, versionName regex, pkg rule)
    src/index.rs                 index.min.json wire types, classification (Wasm / ApkOnly)
    src/repo.rs                  RepoRepository trait, RepoRow, rusqlite implementation
    src/repo_refresh.rs          repo.json fetch, should_apply_refresh (NOFINGERPRINT rule)
    src/trust.rs                 Fingerprint, trust_key, TrustStore (is_trusted, trust, revoke_all)
    src/store.rs                 ExtensionStore (BTreeMaps), reducer for ExtEvent, reconcile, updates count
    src/loader.rs                PackageStore (scan, check_install, commit, remove), load_one
    src/views.rs                 by_type, languages, source_items
    src/install/mod.rs           Installer: queue, download, verify, commit, cancel
    src/install/step.rs          InstallStep
    src/update.rs                check_for_updates (daily gate)
    src/events.rs                ExtEvent, Command
    src/error.rs                 ExtError, LoadError
  kuro-source-wasm/              wasmtime host; depends on kuro-source (area:15) and kuro-extensions::model
    wit/kuro-source.wit          package kuro:source@1.4.0; world source (import host; export invoke)
    src/lib.rs                   WasmSource, instantiate()
    src/bridge.rs                host http, pref-get, pref-set, log; host allow-list
    src/limits.rs                StoreLimits (256 MiB), epoch ticker (100 ms)
src/                             existing GTK app
    extensions_glue.rs           (new) ExtEvent to UI on the GTK thread; sidebar badge; notification adapter (gio)
    entry.rs                     extensions(anime, store, svc) replaces the mockup rows for the manga branch
```

Dependency direction (no cycles): `kurobayomi` → `kuro-extensions`; `kuro-source-wasm` → `kuro-source` (area:15) and `kuro-extensions::model`. The app passes `kuro_source_wasm::instantiate` into `kuro-extensions` as a closure (`SourceLoader`), so the loader crate never links wasmtime.

Threading (Proposal, design section 4): `ExtensionStore` lives in `Rc<RefCell<…>>` on the GTK main thread. Network, file and wasm work runs on a tokio runtime with two worker threads. Events cross with `async_channel`; commands go the other way with a `tokio::sync::mpsc::UnboundedSender`.

---

## 3. Reading guide

Read in this order: this overview, then the inventory, then the topic pages, then the reference documents, then the tasks.

| Document | What it contains |
|---|---|
| [00-overview.md](00-overview.md) | This page: scope, module map, decisions, dependencies, task summary, definition of done. |
| [p1-inventory.md](p1-inventory.md) | All 19 Kotlin files with LOC, every top-level declaration, the port decision (Port, Rewrite, Replace-with-crate, Drop) and the task ids for each file. Scope notes list the out-of-area dependencies. |
| [p1-topic-loader-and-trust.md](p1-topic-loader-and-trust.md) | Deep dive: the ordered load pipeline as a decision table, copy selection and install rules, signer and trust model, repo fingerprint lifecycle, failure modes, concurrency. |
| [p1-topic-installer-pipeline.md](p1-topic-installer-pipeline.md) | Deep dive: the Kotlin download-and-install pipeline, the Kurobayomi install state machine and transition table, cancel rules, queue and concurrency limits, timings, filesystem effects, step emission order. |
| [p1-behavior.md](p1-behavior.md) | Behaviour spec (19 sections): model and lifecycle, manager state and operations, loader rules with constants, index fetch and repo API, install receiver, status reconciliation, installer and queue, update check API, repo refresh, trust, browse views, screen-model callers, app-launch trigger, notifications, dropped mechanisms, Linux package model, threading, cross-area interactions, user-visible message keys. |
| [p1-data-and-config.md](p1-data-and-config.md) | Data: extension models, APK metadata keys and package flags, preferences with keys and defaults, files on disk (Aniyomi and Kurobayomi), network endpoints, wire schemas (`index.min.json`, `repo.json`, `.kuroext` manifest, index extension), constants table, database table `extension_repos` with named queries, bit layouts and numeric enums, Rust type mapping, log texts. |
| [p1-rust-design.md](p1-rust-design.md) | Rust design: WASM decision with options, WIT interface, crate layout, threading, key types and signatures (model, step, events, store reducer, loader wiring, service commands), icons, trust and signature verification, repo storage, index reconciliation, update check, loader and install, browse views, Android-to-Linux mapping, error types, integration with the mockup, pitfalls, dependency table, test seams. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases by family: loader L1 to L29, install I1 to I23, index and repos X1 to X23, update U1 to U10, trust T1 to T7, views V1 to V14, locale F1 to F6, races R1 to R8, wasm guest W1 to W9, notifications and UI N1 to N5; fixtures; test file list with what each test pins. |
| [p1-ui-parity.md](p1-ui-parity.md) | UI parity for the Browse extensions screen: rows, sections, search, overflow, pull to refresh, warning banner, install states, update notification, trust dialog, dropped surfaces, keyboard and mouse equivalents, responsive behaviour, gap list against the current mockup (`src/entry.rs:311-357`). |
| [p1-tasks.md](p1-tasks.md) | 54 implementation tasks, `T-18a-001` to `T-18a-054`, in eight sections: Foundations, Trust, Repos, Index and update API, State and views, Installer, Tests and data, Views, details and UI data. |

---

## 4. Key design decisions and open questions

Each item states the decision in use now, the open choice, and the recommendation. Items marked **User call** need the user's answer before implementation starts on that path.

### 4.1 Decided in the plan (not open)

| ID | Decision | Where |
|---|---|---|
| D0a | Sources run as WASM components (`wasmtime`, component model), not APK, JNI, native `.so` or subprocess. | [p1-rust-design.md](p1-rust-design.md) section 2.1 |
| D0b | Package format is a `.kuroext` zip with `manifest.json`, `source.wasm`, optional `icon.png`. Read in memory, never extracted. | [p1-data-and-config.md](p1-data-and-config.md) section 7.3 |
| D0c | Signatures are ed25519; the fingerprint is SHA-256 of the 32-byte public key, lowercase hex. Verification uses `verify_strict`. | [p1-rust-design.md](p1-rust-design.md) section 6 |
| D0d | Trust keys (`pkg:versionCode:digest`) and the repo fingerprint rule are ported unchanged. Trust keys are not carried into backups. | [p1-tasks.md](p1-tasks.md) T-18a-043 |
| D0e | Install queue is a single FIFO consumer, one commit at a time, with cancel for queued and active jobs. Downloads are capped at 3 concurrent permits (Kotlin has no cap). | [p1-topic-installer-pipeline.md](p1-topic-installer-pipeline.md) section 4 |
| D0f | Update check runs at most once per 24 h (`lastExtCheck` + `1.days`). The gate applies when the check runs from the available list (`fromAvailableExtensions`); the design keeps this rule as `from_available` in `check_for_updates`. | [p1-behavior.md](p1-behavior.md) section 8 and section 13 |
| D0g | Every Android installer, the foreground install service, the install Activity, the MIUI workaround and the `extension_installer` preference are dropped. | [p1-behavior.md](p1-behavior.md) section 15 |
| D0h | Load order: lib gate, signed, trusted, NSFW gate, instantiate. An untrusted NSFW package is `Untrusted`, not `Error`. | [p1-topic-loader-and-trust.md](p1-topic-loader-and-trust.md) section 2 |

### 4.2 Open questions for the user

**D1. Do sources ship as WASM only, or does the first release also need built-in sources? (User call)**
- Facts: no Aniyomi APK source runs on Linux (D0a). The plan has no task that produces a `.kuroext` package, signs it, writes an index entry, or provides a guest SDK for `wasm32-wasip2` authors. Design section 2.1 names the guest SDK as the cost of the WASM option, but no task in [p1-tasks.md](p1-tasks.md) covers it.
- Without a source, P0 ("at least one working source", the bar in the project brief) cannot be met through this area.
- Options: (a) WASM host plus one first-party source written as a `.kuroext` by the user; (b) built-in Rust sources in the app for the first release, WASM later; (c) both.
- Recommendation: (c). Build the WASM host as planned, and in the same milestone write one first-party source as `.kuroext` so the loop (repo → index → verify → install → load → browse) is exercised end to end. Keep built-in sources as the fallback for daily use if the WASM host slips. This needs a new guest-SDK task and a publisher-tool task (section 5.3); neither exists in [p1-tasks.md](p1-tasks.md) today.

**D2. Keep ed25519 signing for Kurobayomi repos, or trust by digest only? (User call)**
- Option A (plan as written): ed25519 signatures; repo fingerprint is a hash of the signing public key, so a repo can be trusted once and all its packages follow.
- Option B: SHA-256 digest only (no signatures); trust is per package-version only.
- Recommendation: A. The repo rule (T-18a-012) needs a key to fingerprint. Option B would remove the repo branch and make every update need a new trust decision.

**D3. Show APK-only index entries as "Not runnable on Linux", or hide them? (User call)**
- Recommendation: show, with the badge text from [p1-behavior.md](p1-behavior.md) section 16.2. The user sees what the repo offers and why it cannot be installed. Hiding them would make a repo look empty.

**D4. Duplicate package names across repos: first repo wins, or last repo wins?**
- Kotlin uses `associateBy` (last wins) for the available map and `find` (first wins) for reconciliation, so the two disagree ([p1-rust-design.md](p1-rust-design.md) section 8.2).
- Recommendation: first repo in table order wins, for both. One rule, no mismatch.

**D5. Per-load NSFW setting versus once per process.**
- Kotlin reads `show_nsfw_source` once per process. Rust reads it on every load ([p1-rust-design.md](p1-rust-design.md) section 9.2, behaviour 3.1).
- Recommendation: per load, as designed. The cost is one preference read per package.

**D6. Compile cache for WASM modules in v1.**
- Plan: no cache in v1 (design section 9.3). Each start compiles each `source.wasm` with Cranelift.
- Recommendation: keep no cache. Add `Module::serialize` into `$XDG_CACHE_HOME/kurobayomi/extensions/compiled/` when startup exceeds 2 s with 10 packages.

**D7. Sandbox limits for guests.**
- Plan: 256 MiB memory per instance, 30 s per call (300 epoch ticks of 100 ms), `hosts` allow-list for HTTP, no file or process access (T-18a-006, [p1-rust-design.md](p1-rust-design.md) section 14 item 2).
- Recommendation: keep as written. These are the numbers the sandbox tests pin.

### 4.3 Deliberate deviations from Kotlin (recorded so they are not reverted by accident)

| Deviation | Kotlin | Kurobayomi | Reference |
|---|---|---|---|
| Invalid lib version string drops only that entry | `toDouble()` throws and the whole repo is dropped | Entry dropped with a warning | [p1-rust-design.md](p1-rust-design.md) 8.1 |
| Required-field failure drops the entry, not the repo | Entry-level failure can fail the repo | Entry dropped | 8.1 step 3 |
| Cancel in Downloading or Installing sets `Idle` at once | Step unchanged | `Idle` | [p1-topic-installer-pipeline.md](p1-topic-installer-pipeline.md) 2.2 |
| Every route emits `Installing` | Private route skips it | Always emitted | 2.2 |
| One download per package | Two split rows, two downloads | Second request reuses the active job | design 9.6 |
| Update check updates the badge count too | Count not written | Same call emits the count | design 8.4 |
| Repo refresh failures are isolated | One failure can cancel siblings | `join_all` with isolation | design 8.5 |
| Loader library-version comparison by tuple | `Double` | `(major, minor)` | design 5.1 |

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What is needed | Used by tasks |
|---|---|---|
| area:01-foundation-app-bootstrap | `GApplication` with an application id for `gio::Notification`; startup hook for the update check and for the non-blocking load; the app's `main.rs` sidebar badge (`src/main.rs:153`). | T-18a-027, T-18a-040, T-18a-049 |
| area:02-preferences-system | Keys `source_languages`, `show_nsfw_source`, `hidden_catalogues`, `ext_updates_count`, `trusted_extensions` (app-state prefix), `last_ext_check`, `extension_installer` (dropped), `extension_repos` (legacy). Typed reads and writes. | T-18a-010, T-18a-012, T-18a-023, T-18a-025, T-18a-032 |
| area:03-db-manga-schema-migrations | The `extension_repos` table and a database handle for `rusqlite` calls inside `spawn_blocking`. | T-18a-012, T-18a-014 |
| area:09-domain-source-track-download-release-misc | Repo interactors (`UpdateMangaExtensionRepo`, `GetMangaExtensionRepo`) and `ExtensionRepo` model used as the reference for repo validation. | T-18a-015 |
| area:10-app-domain-layer | Named as the dependency of T-18a-028 (store, events and channel bridge). The task text does not state which contract it needs; confirm with the area:10 author before T-18a-028 starts. | T-18a-028 |
| area:11-app-migrations | Legacy migrations `ExternalRepoMigration` and `TrustExtensionRepositoryMigration` (repo preference moved to the table with `NOFINGERPRINT-<n>` fingerprints and the `Repo #<n>` title). | T-18a-019 |
| area:12-network-stack | HTTP client (`reqwest` 0.12, rustls, user agent, timeouts), the JSON config (`ignoreUnknownKeys`, `explicitNulls = false` for the wire models). | T-18a-020, T-18a-021, T-18a-034 |
| area:13-core-util-storage-archive-torrent | Hash helpers, read-only file copy, and the zip reader crate choice (`zip` 2, `deflate` only). | T-18a-004, T-18a-043 |
| area:15-source-api-manga | `MangaSource`, `CatalogueSource`, source factory and `create-sources`, `SourceError`, the serde models (`SManga`, `SChapter`, `Page`, `MangasPage`, `FilterList`), and the trait the WASM host implements. | T-18a-001, T-18a-006, T-18a-029 |
| area:17-source-app-layer | Renders the Browse tab, the trust dialog, the details screen and the Problems entry; owns the wording of the `UNTRUSTED` and `ORPHANED` tags, the language header labels (`LocaleHelper`), and the image cache for APK-only icons. | T-18a-022, T-18a-031, T-18a-054 |
| area:22-backup-restore | Restore help text states that every package is untrusted after restore (T-18a-043). | T-18a-043 |

### 5.2 What others need from this area

| Consumer | What it gets | Where |
|---|---|---|
| area:17-source-app-layer | `ExtensionStore`, `by_type`, `languages`, `source_items`, the `ExtEvent` stream, the `Command` set (install, cancel, update, trust, revoke, uninstall, add and delete repo, find available), the details data. | [p1-rust-design.md](p1-rust-design.md) sections 5.3, 5.6, 10 |
| area:15-source-api-manga | Its traits are implemented by `kuro-source-wasm`. Its JSON field names are reused in the WIT `invoke` payloads. | design 2.2 |
| area:16-source-api-anime and area:19-extensions-anime | The `UpdateNotifier` trait and the `ExtEvent` channel pattern; anime keeps its own notification id (`p1-behavior.md` section 15, anime variant row). | design 3.2 |
| area:21-backup-format-and-create and area:22-backup-restore | The decision to exclude trust keys (T-18a-043). | [p1-tasks.md](p1-tasks.md) |
| area:34-images-covers-cache | Extension icons are files in `$XDG_CACHE_HOME/kurobayomi/extensions/icons/<pkg>.png`; APK-only entries use the repo `iconUrl` through the image cache. The design text (5.7) attributes that cache to area:17; the cache itself is area:34, so confirm the owner. | design 5.7 |

### 5.3 Gaps between this area and the rest of the plan

- **Guest SDK and publisher tool are not tasked.** Authoring a `.kuroext` (compile to `wasm32-wasip2`, write the manifest, sign with ed25519, emit the index entry with `ed25519Pub`, `sha256`, `wasm`, `ed25519`) has no task in this area. Needed for D1. Owner to decide: this area (new tasks) or a tooling area.
- **Index entry for Kurobayomi** (`ed25519Pub`, `ed25519`, `wasm`, `sha256`) is specified in [p1-data-and-config.md](p1-data-and-config.md) section 7.5 but no task checks that a repo served by the publisher tool matches it end to end.
- **Source enable and language filter** write to area:02 keys; the UI write path is area:17's and is not tasked here.

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, rows whose file starts with `18-extensions-manga/` in `TASKS.csv`. Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint 18-extensions-manga/p1-tasks.md` prints `ok`.

### 6.1 Counts

Total: 54 tasks (`T-18a-001` to `T-18a-054`).

| Priority | Tasks | Effort S | Effort M | Effort L | Effort XL |
|---|---|---|---|---|---|
| P0 | 27 | 12 | 13 | 2 | 0 |
| P1 | 16 | 13 | 3 | 0 | 0 |
| P2 | 7 | 7 | 0 | 0 | 0 |
| P3 | 4 | 4 | 0 | 0 | 0 |
| **Total** | **54** | **36** | **16** | **2** | **0** |

P0 tasks: T-18a-001, 002, 003, 004, 005, 006, 008, 009, 012, 013, 014, 015, 020, 021, 022, 028, 029, 030, 032, 033, 034, 035, 036, 037, 047, 048, 049.

P3 tasks: T-18a-038, 039, 041, 042 (decision and drop records; no code).

### 6.2 P0 critical path

Arrows mean "must finish before". Tasks on one line can run in parallel. The two L tasks (T-18a-006 and T-18a-034) are the schedule risk.

1. Foundations: T-18a-001 (model), T-18a-035 (install step enum), T-18a-002 (lib gate) and T-18a-003 (name normalisation) run in parallel after T-001.
2. Package file: T-18a-004 (`.kuroext` reader; needs area:13) → T-18a-048 (manifest validation; also needs T-002) → T-18a-005 (PackageStore).
3. Store: T-18a-028 (store, events, bridge; needs area:10) → T-18a-029 (source-id index and stub map; needs area:15) → T-18a-032 (source enable list; needs area:02). T-18a-028 also → T-18a-030 (Browse buckets).
4. Repos and index: area:03 → T-18a-014 (table and repository) → T-18a-015 (refresh with fingerprint rule; needs area:09) and T-18a-020 (index fetch; needs area:12) → T-18a-021 (classification; also needs T-002 and area:12) → T-18a-022 (find available; also needs T-028).
5. Trust: T-18a-001 with area:02 and area:03 → T-18a-012 (trust rules and fingerprints) → T-18a-013 (trust action and reload).
6. Load: T-18a-004 → T-18a-006 (WASM host, L; needs area:15) → T-18a-008 (per-package isolation) and T-18a-009 (language derivation). T-18a-049 (non-blocking startup load) needs T-005 and T-028.
7. Install: T-18a-004 → T-18a-047 (signature and digest verification) → T-18a-034 (download, verify, commit, L; also needs T-012 and area:12) → T-18a-036 (queue, cancel; also needs T-035) → T-18a-037 (install rules; also needs T-005). T-18a-033 (uninstall) needs T-005 and T-028.

Longest chain: T-18a-001 → T-18a-004 → T-18a-047 → T-18a-034 (L) → T-18a-036 → T-18a-037. Second longest: T-18a-001 → T-18a-004 → T-18a-006 (L) → T-18a-008 → usable load.

### 6.3 Risks

| Risk | Effect | Mitigation |
|---|---|---|
| No Aniyomi APK source runs (D0a). | The library is empty until a `.kuroext` source exists. P0 usability depends on D1. | Decide D1 first. Write one first-party source in the same milestone. |
| Guest SDK and publisher tool not tasked (5.3). | Nothing can produce a valid `.kuroext`, so the install path cannot be tested with real packages. | Add tasks before T-18a-034 is started. |
| `wasmtime` is the least-known dependency. Version and feature set are "latest stable at implementation time" (design section 15). | Build time, binary size and API churn. | Pin the version in T-18a-006 acceptance; epoch and store-limit tests (W1 to W9) run under `cargo test -p kuro-source-wasm`. |
| WIT ABI `kuro:source@1.4.0` is frozen by this area, but its JSON payloads follow area:15 models. | Changes in area:15 break the ABI. | Freeze the ABI with a version gate (`abi` in the manifest) and a golden JSON fixture per op. |
| Two L tasks in the P0 chain (T-006, T-034). | Schedule slip. | Start T-006 and T-034 early, in parallel; split if either passes 1.5 weeks. |
| Trust and signature mistakes (malleable signatures, prefix hazard). | Unsafe installs or unintended trust removal. | `verify_strict` only; prefix test T3 (`a.b` versus `a.b.c`); fixture tests I6 to I9 in T-18a-047. |
| Concurrency between reload, trust write and install commit. | Stale state in Browse. | Per-package mutex (edge case R3); commit lock; store touched only on the GTK thread. |
| Users expect Android extensions to keep working. | Support questions and lost trust. | Index keeps listing APK entries with the not-runnable badge (D3); the explanation is UI copy owned by area:17 (design section 14, pitfall 1 names it). |

---

## 7. Definition of done for this area

Observable behaviour in the running Kurobayomi. Each line is checked by the named test or by a manual step with the `run` skill.

1. **Add a repo.** Settings or Browse: adding a base URL fetches `<base>/repo.json`, stores the fingerprint, and lists the repo. A second repo with the same fingerprint is refused (edge case X19). A repo whose fingerprint is `NOFINGERPRINT-<n>` takes the real fingerprint on the next refresh (edge cases X16 to X18).
2. **Browse buckets.** The Browse tab shows the sections in this order: `Updates pending` (with `Update all`), `Installed` (obsolete rows first, then by lowercase name), `Untrusted`, then available rows grouped by language. NSFW rows are hidden unless `show_nsfw_source` is on. Verified by views tests V1 to V14.
3. **Install.** Clicking Install on an available row shows the steps in order: `Pending`, `Downloading`, `Installing`, `Installed`. The row moves to Installed with its version and language label. Verified by the event-order fixture in [p1-topic-installer-pipeline.md](p1-topic-installer-pipeline.md) section 6.1.
4. **Failed install.** A package whose digest, signature or fingerprint does not match ends in `Error`, leaves no `.part` file, and the row stays Available. Verified by install tests I6 to I9.
5. **Cancel.** Cancel during download sets `Idle`, removes the `.part` file, and leaves no row change. Verified by I18.
6. **Untrusted package.** A valid package from an unknown signer appears under Untrusted with the `UNTRUSTED` tag. The Trust dialog shows `Untrusted extension` and the two body lines from [p1-ui-parity.md](p1-ui-parity.md) section 5. After Trust, it moves to Installed and its sources appear in the source list. Trusting version 13 does not trust version 14 (T2).
7. **Restart.** After a restart, installed packages load with no network. Untrusted packages stay untrusted unless their repo fingerprint matches. A stray `.part` file is removed at startup (L29).
8. **Uninstall.** The confirm dialog shows `Do you really want to remove "<name>" extension?`. Confirming deletes the file and removes the row.
9. **Downgrade and signer change.** Installing a lower version code shows `Installed extension version is higher. Downgrading is not allowed.`; a different signer shows `Installed extension signature is not matched.`. The installed package is unchanged.
10. **APK-only entries.** Listed with `Not runnable on Linux`. The Install button is disabled for them.
11. **Update check.** Launching the app runs the check at most once per 24 h. When updates exist, one notification with `Extension update available` (one update) or `%d extension updates available` (more) is posted, and the sidebar badge shows the same count. When the count is zero, the notification is withdrawn.
12. **Non-blocking startup.** The window opens before packages finish loading; the Installed list fills in when loading completes.
13. **No Android-only UI.** No install prompt, no foreground notification `Installing extension…`, no installer preference.
14. **Tests.** `cargo test -p kuro-extensions` and `cargo test -p kuro-source-wasm` pass. They cover the lib-version table (T-18a-044), trust prefix (T3), repo refresh rule (`should_apply_refresh`), index classification, reconciliation, views, install rules, and the sandbox (W1 to W9).

---

## 8. Document QA performed while writing this overview

- Lint: `p1-tasks.md` lints `ok`.
- Short `area:NN` tokens in all nine documents were expanded to full area ids (for example `area:17` became `area:17-source-app-layer`). The one prose reference to the restore help text now points to `area:22-backup-restore` (it was `area:13`).
- Inventory summary counts corrected: the "Rewrite" line listed nine files but names four (rows 8, 10, 17, 18). The table counts are Port 9, Rewrite 4, Replace-with-crate 2, Drop 4 = 19.
- Spot checks against the Kotlin source passed: `LIB_VERSION_MIN = 1.4` and `LIB_VERSION_MAX = 1.5` (`app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt:56-57`); `"Tachiyomi: "` strip (`MangaExtensionLoader.kt:253`); `1.days` update gate (`app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt:75`); `extension_installer` key (`app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt:19`).
- Not verified here: the individual Kotlin line numbers cited in the topic documents beyond the spot checks above.
