# Area 18 — Manga extensions: implementation tasks

Format: `### T-18a-NNN — Title`, then Priority, Effort, Depends, Sources, Acceptance. Sources are repo-relative to `/home/blackbox/aniyomi-src`. Behaviour references are to `p1-behavior.md`, design to `p1-rust-design.md`, data to `p1-data-and-config.md`, edge cases to `p1-edge-cases-and-tests.md`.

Priorities: P0 is needed for daily use (browse, install, load, run third-party sources). P1 is parity most users rely on (updates, repos, notifications, tests). P2 is long tail. P3 is dropped, with the replacement or the drop decision stated.

---

## Foundations

### T-18a-001 — Extension model types and load result
- Priority: P0
- Effort: S
- Depends: area:15-source-api-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/model/MangaExtension.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/model/MangaLoadResult.kt, app/src/main/java/eu/kanade/domain/extension/manga/model/MangaExtensions.kt
- Acceptance:
  - `kuro-extensions::model` defines `InstalledExtension`, `AvailableExtension`, `UntrustedExtension`, `SourceInfo`, `PackageKind`, `LoadResult` with every field listed in `p1-data-and-config.md` section 1.
  - `LibVersion` is a `(major, minor)` tuple with `MIN = 1.4` and `MAX = 1.5`; a unit test asserts `1.4.0` and `1.5.99` are in range and `1.3.9` and `1.6.0` are not.
  - `cargo test -p kuro-extensions model` passes.

### T-18a-002 — Lib version and ABI gate
- Priority: P0
- Effort: S
- Depends: T-18a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt
- Acceptance:
  - `LibVersion::from_version_name` takes the prefix before the last `.` and parses `major.minor`; `"1.4.13"` gives `(1,4)`, `"1.10.0"` gives `(1,10)`, `"14"` gives `None`.
  - Index and manifest entries outside `1.4..=1.5` are rejected (edge cases L5 to L9).
  - Manifest `versionName` must match `^\d+\.\d+\.\d+$`; `"1.4"` is rejected.

### T-18a-003 — Display-name normalisation
- Priority: P0
- Effort: S
- Depends: T-18a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt
- Acceptance:
  - Name is trimmed, then `"Tachiyomi: "` is stripped when present; a name without the prefix is kept whole (edge cases L21 to L23).
  - Applied to index entries and to manifests at validation time, so the store never holds the prefix.

### T-18a-004 — `.kuroext` zip reader with size limits
- Priority: P0
- Effort: M
- Depends: T-18a-001, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt
- Acceptance:
  - Reads only `manifest.json`, `source.wasm` and optional `icon.png` by exact name using `zip` with `deflate` only; no extraction to disk.
  - Limits: manifest 64 KiB, wasm 32 MiB, icon 256 KiB; each read is bounded with `take(limit + 1)`; an over-limit entry gives `LoadError::Manifest`.
  - A corrupt zip gives `LoadError::Manifest("zip")` and never panics (edge cases L25, I13, I14, I15).

### T-18a-005 — PackageStore: scan, file naming, startup sweep
- Priority: P0
- Effort: M
- Depends: T-18a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt
- Acceptance:
  - Store directory is `$XDG_DATA_HOME/kurobayomi/extensions`, created when missing; one file per package named `<pkg>.kuroext` (behaviour 3.2 single store).
  - A file whose manifest `pkg` does not match its file stem is reported as an error and not loaded (edge case L2).
  - Startup deletes stray `*.part` files in the store and in the download cache (edge case L29).
  - `scan` results are sorted by file name; a bad file never fails the batch.

### T-18a-006 — WASM source host (`kuro-source-wasm`)
- Priority: P0
- Effort: L
- Depends: T-18a-004, area:15-source-api-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt
- Acceptance:
  - `instantiate(engine, bytes, manifest)` compiles the component with `wasmtime` (component model), links the `host` interface from the WIT in design section 2.2, and returns `Vec<Arc<dyn CatalogueSource>>` from the `create-sources` op (or the single source for `MangaSource` packages).
  - Each guest call runs in `spawn_blocking`, is serialised per source by a mutex, has an epoch deadline of 300 ticks at 100 ms (30 s), and a memory limit of 256 MiB (`StoreLimits`).
  - The `http` import rejects hosts not in the manifest `hosts` array with `Err("host not allowed: <host>")` (edge cases W1 to W8).
  - Load failure of one package yields `LoadResult::Error` and never stops the other packages (all-or-nothing per package, behaviour 3.8 step 9).

### T-18a-007 — Source factory expansion
- Priority: P1
- Effort: S
- Depends: T-18a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt
- Acceptance:
  - A factory package (manifest `factory` set) returns the sources from `create-sources`; the list order is preserved and duplicate source ids within the package are rejected with `LoadError::Manifest`.
  - A non-factory package with an empty `sources` array loads with zero sources (edge case L17).

### T-18a-008 — Per-package load isolation
- Priority: P0
- Effort: S
- Depends: T-18a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt
- Acceptance:
  - Each package loads in its own task; a panic or error in one leaves the others loaded (Kotlin's missing-metadata NPE case, edge case L1, is handled as `LoadError::Manifest`).
  - The loader never calls a blocking API on the GTK thread; the startup result arrives as one `ExtEvent::Loaded` (design section 4).

### T-18a-009 — Installed language derivation
- Priority: P0
- Effort: S
- Depends: T-18a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt
- Acceptance:
  - Language is `""` for zero catalogue sources, the single language for one, and `"all"` for several (edge cases L17 to L19).
  - Unit test uses fixture lists of 0, 1, and 3 languages.

### T-18a-010 — NSFW gate per load
- Priority: P1
- Effort: S
- Depends: T-18a-006, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Reads `show_nsfw_source` (default `true`) on each load, so a change applies without restart (deliberate change from behaviour 3.1; edge case L14).
  - A trusted NSFW package with the setting off gives `LoadResult::Error(NsfwDisabled)`; an untrusted NSFW package is reported as `Untrusted` first (edge case L12, L13).

### T-18a-011 — Icon extraction and fallback
- Priority: P2
- Effort: S
- Depends: T-18a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt
- Acceptance:
  - `icon.png` is written to `$XDG_CACHE_HOME/kurobayomi/extensions/icons/<pkg>.png` once per package version; the UI shows a generic glyph when missing (edge cases L26, L27, N4).
  - The per-source icon lookup (`getAppIconForSource`) returns no panic when the package has vanished (behaviour 2.4 defect).

---

## Trust

### T-18a-012 — Trust rules and fingerprints
- Priority: P0
- Effort: M
- Depends: T-18a-001, area:02-preferences-system, area:03-db-manga-schema-migrations
- Sources: app/src/main/java/eu/kanade/domain/extension/manga/interactor/TrustMangaExtension.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/lang/Hash.kt
- Acceptance:
  - `Fingerprint` is SHA-256 of the 32-byte ed25519 public key, lowercase hex; `parse_hex` rejects uppercase.
  - `is_trusted` is true when any repo `signing_key_fingerprint` equals the signer, or when `"<pkg>:<versionCode>:<digest>"` is in the app-state set `trusted_extensions`.
  - `trust` removes every entry starting with `"<pkg>:"` (colon included) and inserts the new key; `revoke_all` clears the set but keeps repo trust (edge cases T1 to T4, T7).
  - Settings > Advanced "Revoke trusted unknown extensions" calls `revoke_all` (caller in area:17-source-app-layer).

### T-18a-013 — Trust action and reload
- Priority: P0
- Effort: S
- Depends: T-18a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt
- Acceptance:
  - `Command::Trust` is a no-op when the package is not in the untrusted set (behaviour 2.9 step 1).
  - On success the package is reloaded with the per-package mutex held and emitted as `Installed`; if the reload is still untrusted nothing is emitted (edge cases T5, T6, R3).

### T-18a-047 — Signature and digest verification
- Priority: P0
- Effort: M
- Depends: T-18a-004
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/lang/Hash.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt
- Acceptance:
  - `verify_package` checks, in order: SHA-256 of the downloaded bytes equals the index value; the fingerprint of `ed25519Pub` equals the repo fingerprint; `ed25519-dalek` `verify_strict` accepts the signature over the raw digest (design section 6.3).
  - Fixture tests cover a valid signature, a digest mismatch, a wrong key, and a malleable signature (edge cases I6 to I9).
  - Installed `signerPub` is written only by the installer after a successful verify (design section 9.4).

---

## Repos

### T-18a-014 — `extension_repos` table and repository
- Priority: P0
- Effort: M
- Depends: area:03-db-manga-schema-migrations
- Sources: data/src/main/sqldelight/data/extension_repos.sq, data/src/main/java/mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt
- Acceptance:
  - Table DDL and column types match data file section 9.1 exactly.
  - Upsert uses `ON CONFLICT(base_url) DO UPDATE SET ... = excluded.*`; `replace` is not implemented (data 9.3).
  - `all()` orders by `rowid` (edge case X22); a fingerprint UNIQUE violation maps to `ExtError::RepoSave` with text `Error Saving Repository to Database`.

### T-18a-015 — Repo refresh with fingerprint rule and isolation
- Priority: P0
- Effort: M
- Depends: T-18a-014, area:09-domain-source-track-download-release-misc
- Sources: domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/UpdateMangaExtensionRepo.kt, domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoService.kt
- Acceptance:
  - `GET <base>/repo.json` (timeout 15 s); upsert only when `should_apply_refresh` (stored fingerprint starts with `NOFINGERPRINT` or equals the fetched one) (behaviour 9.2).
  - Each repo is refreshed in its own task; an error in one never cancels the others (edge case X18).
  - A failed fetch leaves the stored row unchanged and is logged with `Failed to fetch repo details`.

### T-18a-016 — Add repo with validation
- Priority: P1
- Effort: S
- Depends: T-18a-015
- Sources: domain/src/main/java/mihon/domain/extensionrepo/exception/SaveExtensionRepoException.kt, data/src/main/java/mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt
- Acceptance:
  - The base URL is trimmed and a trailing `/` removed before save (edge case X12, deliberate change); only `http://` or `https://` URLs are accepted.
  - The add flow fetches `repo.json` first; a duplicate base URL or duplicate fingerprint shows the existing store error, not a crash (edge case X19).

### T-18a-017 — Delete and replace repo
- Priority: P1
- Effort: S
- Depends: T-18a-014
- Sources: domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/ReplaceMangaExtensionRepo.kt, data/src/main/java/mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt
- Acceptance:
  - Delete removes the row only; installed packages stay and become obsolete at the next index fetch (edge case X21).
  - Replace is implemented as delete plus upsert in one transaction; a conflict rolls back and leaves both rows (edge case X20).

### T-18a-018 — Repo count for the Browse screen
- Priority: P2
- Effort: S
- Depends: T-18a-014
- Sources: domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/GetMangaExtensionRepoCount.kt
- Acceptance:
  - `count()` is exposed through the store and updates on insert and delete events.

### T-18a-019 — Import repo URLs from an Aniyomi backup
- Priority: P2
- Effort: S
- Depends: T-18a-014, area:11-app-migrations
- Sources: app/src/main/java/mihon/core/migration/migrations/TrustExtensionRepositoryMigration.kt
- Acceptance:
  - An importer takes a list of base URLs and writes rows named `Repo #<n>` with fingerprint `NOFINGERPRINT-<n>` (1-based), then refreshes them (data section 9.4).
  - Duplicates are skipped; the `Tachiyomi` trust keys are not imported (task T-18a-043).

---

## Index and update API

### T-18a-020 — Index fetch per repo
- Priority: P0
- Effort: M
- Depends: T-18a-014, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt
- Acceptance:
  - `GET <base>/index.min.json` per repo, all repos in parallel, each with a 15 s total timeout; non-2xx and parse failures give an empty list for that repo only (behaviour 4.2).
  - A repo returning a JSON object instead of an array gives an empty list and a log line (edge case X1).

### T-18a-021 — Index entry classification
- Priority: P0
- Effort: M
- Depends: T-18a-002, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt, app/src/main/java/eu/kanade/di/AppModule.kt
- Acceptance:
  - Wire fields and optionality match data file section 7.1; unknown keys are ignored; `sources` may be absent (edge cases X6 to X8).
  - A bad entry is dropped alone (X2 to X5, deliberate change); `kind` is `Wasm` when `wasm` and `sha256` are present, else `ApkOnly` (behaviour 16.2).
  - `iconUrl` is `<repo>/icon/<pkg>.png`; `apkName` is kept for the `ApkOnly` label.

### T-18a-022 — Find available: single-flight, replace, toast
- Priority: P0
- Effort: S
- Depends: T-18a-021, T-18a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt
- Acceptance:
  - Concurrent `FindAvailable` calls share one run (edge case R1); the result replaces the available map (last-wins is not used; see T-18a-024).
  - `FetchFailed` is emitted, which shows `Failed to fetch available extensions`, only when every configured repo failed and no cached list exists (edge case X10, deliberate change).
  - An empty repo table empties the available map and leaves installed rows unchanged (edge case X11).

### T-18a-023 — First-run sub-language enablement
- Priority: P2
- Effort: S
- Depends: T-18a-022, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - Runs once when `source_languages` has never been saved; sets `{all, en, <device>}` plus source languages that start with the device code and differ from it (edge cases F1 to F5).
  - Device language is read from `LC_MESSAGES`, then `LANG`; `C` and `POSIX` count as `en` (edge case F6, deliberate).

### T-18a-024 — Reconciliation: obsolete, hasUpdate, repo URL, duplicates
- Priority: P1
- Effort: M
- Depends: T-18a-022
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt
- Acceptance:
  - Installed packages missing from every repo are `is_obsolete = true`; a package found again resets it to `false` (edge cases X14, X15, deliberate change).
  - `has_update = available.version_code > installed.version_code || available.lib > installed.lib`; `repo_url` is set from the matching entry (edge cases U9, U10).
  - Duplicate `pkg` across repos: first repo in table order wins in both the available map and the lookup (edge case X13, deliberate change).

### T-18a-025 — Updates count persistence and badge
- Priority: P1
- Effort: S
- Depends: T-18a-024, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - `updates_count` counts installed packages with `has_update` (obsolete included, behaviour 2.12) and is written to `ext_updates_count` on every change.
  - The sidebar Browse badge reads the manga part of the count (design section 13.3).

### T-18a-026 — Update check API
- Priority: P1
- Effort: M
- Depends: T-18a-015, T-18a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt
- Acceptance:
  - Gate applies only when `from_available` is true: blocked when `now < last_ext_check + 86_400_000`; a clock set backwards does not block (edge case U3, deliberate change). Launch calls are not gated (edge case U1).
  - Repo refresh runs first; the installed list comes from the store snapshot taken after refresh (edge case R8).
  - Returns installed names with `has_update`, emits `UpdatesCount`, and calls `prompt` with the names; `last_ext_check` is written only on a full fetch.

### T-18a-027 — Update check triggers
- Priority: P1
- Effort: S
- Depends: T-18a-026, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - Runs once after the window is shown (replaces the `LaunchedEffect(Unit)` at MainActivity.kt:397-402), off the GTK thread.
  - A failure is logged and does not show a toast (behaviour 13).
  - Manual refresh from the Browse screen runs `FindAvailable { force: true }` (T-18a-022).

---

## State and views

### T-18a-028 — ExtensionStore, events and channel bridge
- Priority: P0
- Effort: M
- Depends: T-18a-001, area:10-app-domain-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallReceiver.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt
- Acceptance:
  - `ExtensionStore` holds `BTreeMap`s for installed, untrusted, available, stubs, steps, and `updates_count`; `apply(ExtEvent) -> ChangeSet` implements the reducer rules of design section 5.4.
  - Events cross to the GTK thread through `async_channel`; the store is only mutated on the GTK thread; no `RefCell` borrow is held across an `await` (edge case R6).
  - `Step` events for a key removed by `Removed` are ignored (edge case R4).

### T-18a-029 — Source-id index and stub map
- Priority: P0
- Effort: S
- Depends: T-18a-028, area:15-source-api-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt
- Acceptance:
  - `source_package(id)` returns the first installed package in pkg order that has the source (edge case V14).
  - Stub map is rebuilt from the available list only when that list is non-empty (behaviour 2.5).

### T-18a-030 — Browse buckets (`by_type`)
- Priority: P0
- Effort: M
- Depends: T-18a-028
- Sources: app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionsByType.kt, app/src/main/java/eu/kanade/domain/extension/manga/model/MangaExtensions.kt
- Acceptance:
  - Installed: obsolete first, then name case-insensitive; partitioned into updates and installed (edge case V1, V2).
  - Available: excludes installed, untrusted, and NSFW when the setting is off; one row per enabled-language source with pkg `"<pkg>-<sourceId>"` (edge cases V4 to V8).
  - `show_nsfw` is read on every call, not once at subscription (deliberate change).

### T-18a-031 — Language list
- Priority: P1
- Effort: S
- Depends: T-18a-030
- Sources: app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionLanguages.kt
- Acceptance:
  - Distinct languages from available sources (or the extension language when it has none), enabled first, then `LocaleHelper` order with `all` first (edge case V10).

### T-18a-032 — Source enable list
- Priority: P0
- Effort: S
- Depends: T-18a-029, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionSources.kt
- Acceptance:
  - Enabled = source id not in `hidden_catalogues`; `label_as_name` follows the three-case rule (edge cases V11 to V13).
  - Toggling a source updates the list without reopening the screen.

### T-18a-033 — Uninstall (remove installed package)
- Priority: P0
- Effort: S
- Depends: T-18a-005, T-18a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstaller.kt
- Acceptance:
  - Cancels any active job for the package, waits for the commit lock, deletes the file, and emits `Removed`; the reducer removes the package from installed and untrusted (edge cases I21, I22, R2).
  - Sources of the removed package are dropped from the store; an in-flight guest call finishes (edge case I23).

---

## Installer

### T-18a-034 — Download, verify and commit pipeline
- Priority: P0
- Effort: L
- Depends: T-18a-004, T-18a-012, T-18a-047, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstaller.kt
- Acceptance:
  - Streams `<repo>/<wasm>` with `reqwest` into `$XDG_CACHE_HOME/kurobayomi/extensions/download/<pkg>-<versionName>.kuroext.part` with a 10 s connect and 120 s total timeout; body capped at 64 MiB.
  - Steps emitted: `Pending`, `Downloading` (with byte progress), `Installing` at commit, `Installed` or `Error`; the state map is keyed by package (edge case I19).
  - Digest and signature checks run before any write to the store; a failure removes the `.part` file and emits `Error` (edge cases I5 to I9, I12).
  - One retry after 2 s on a network error (edge case I10); commit copies to `<data>/<pkg>.kuroext.part`, fsyncs, renames, and sets mode `0444` (design section 9.5).

### T-18a-035 — Install step type
- Priority: P0
- Effort: S
- Depends: T-18a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt
- Acceptance:
  - `InstallStep` variants are `Idle, Pending, Downloading, Installing, Installed, Error` in that order; `is_completed` is true for `Idle`, `Installed`, `Error` only (behaviour 1.3).
  - Unit test asserts all six values and the completion set.

### T-18a-036 — Install queue, cancel and update requests
- Priority: P0
- Effort: M
- Depends: T-18a-034, T-18a-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/InstallerManga.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstaller.kt
- Acceptance:
  - Downloads run with at most 3 concurrent permits; commits are serial (design section 4).
  - Cancel of a queued job sets `Idle` at once; cancel during download removes the `.part` file and sets `Idle` (edge cases I16, I17, deliberate change).
  - Cancel during commit is ignored once commit starts (edge case I18).
  - Update for a package without an available entry emits no step and logs a warning (edge case I20).
  - Duplicate requests for one package join the existing job (edge case N5).

### T-18a-037 — Install rules: downgrade, equal version, signer continuity
- Priority: P0
- Effort: M
- Depends: T-18a-005, T-18a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt
- Acceptance:
  - Lower version code than installed → `Downgrade`; equal → accepted; differing signer → `SignerMismatch` (edge cases I1 to I3).
  - Checks run before the download is committed, so a refused install never touches the installed file (design section 9.1).
  - Log texts equal the Kotlin strings in data section 13.

### T-18a-038 — Drop Android installers and install services (P3)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/PackageInstallerInstallerManga.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/ShizukuInstallerManga.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallActivity.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallService.kt
- Acceptance:
  - Decision recorded: PackageInstaller, Shizuku, Legacy Activity and the install foreground service are not ported. Replacement is the single in-process installer (T-18a-034 and T-18a-036).
  - User loss stated in behaviour section 15: no system installer prompt; packages from Aniyomi APKs cannot run at all (see T-18a-006 and design section 2.1).

### T-18a-039 — Drop the installer preference and MIUI workaround (P3)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallActivity.kt
- Acceptance:
  - The `extension_installer` key is not created; an existing value from an imported backup is ignored.
  - No MIUI detection, no 1 s result-ignore window, no `ext_installer_*` strings are used.

### T-18a-040 — Update notification and dismissal
- Priority: P1
- Effort: S
- Depends: T-18a-026, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/ExtensionUpdateNotifier.kt
- Acceptance:
  - `gio::Notification` id `extension-updates`; title `Extension update available` for one and `%d extension updates available` for more; body is the names joined by `", "` unless `hide_notification_content` is true (edge cases U5, N1).
  - Default action opens the Browse tab; dismissed when the count is 0 (edge case U6); no notification when count is 0 (edge case N3).
  - Anime uses its own id (edge case U7); a missing notification server is a logged no-op (edge case N2).

### T-18a-041 — Install progress notification (P3)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallService.kt
- Acceptance:
  - Decision recorded: no system-tray "Installing extension…" notification. Progress is shown per row (ui-parity section 3). User loses the tray entry only.

### T-18a-042 — Drop shared (system-installed) extension discovery (P3)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt
- Acceptance:
  - Decision recorded: no PackageManager scan, no shared-vs-private selection, no system uninstall intent. User loses extensions installed by other Android apps; the Kurobayomi store is the only source.

### T-18a-043 — Trust keys in backups (decision)
- Priority: P2
- Effort: S
- Depends: T-18a-012, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/domain/extension/manga/interactor/TrustMangaExtension.kt
- Acceptance:
  - Backups do not include trust keys, because a key is a digest of an APK certificate in Aniyomi and has no meaning for Kurobayomi packages.
  - After restore every package is untrusted until the user trusts it or its repo fingerprint matches; this is documented in the restore screen help text (area:22-backup-restore).

---

## Tests and data

### T-18a-044 — Core test suite for extensions
- Priority: P1
- Effort: M
- Depends: T-18a-002, T-18a-012, T-18a-021, T-18a-024, T-18a-030, T-18a-037
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt
- Acceptance:
  - Test files listed in edge-cases section 12 exist under `kuro-extensions/tests/` and `kuro-source-wasm/tests/`.
  - `cargo test -p kuro-extensions -p kuro-source-wasm` passes with no GTK display.
  - Every edge-case row marked with a task id in `p1-edge-cases-and-tests.md` has at least one test or a recorded reason.

### T-18a-045 — Fixtures for tests
- Priority: P1
- Effort: S
- Depends: T-18a-044
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt
- Acceptance:
  - Fixture files from edge-cases section 11 are committed under `kuro-extensions/tests/fixtures/`, including the test ed25519 seed, which is marked test-only in its file name and a comment.
  - The wasm test components are built from source in the repo and their size is under 1 MiB each.

### T-18a-046 — Error taxonomy and log texts
- Priority: P1
- Effort: S
- Depends: T-18a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstaller.kt
- Acceptance:
  - `ExtError` and `LoadError` variants exist as in design section 12; each `LoadError` display string equals the Kotlin log text in data section 13.
  - Every error reaches the log with `tracing`; only `FetchFailed` reaches the UI as a toast.

### T-18a-048 — Manifest validation
- Priority: P0
- Effort: M
- Depends: T-18a-004, T-18a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt
- Acceptance:
  - Every field rule of data file section 7.3 is enforced: `schema == 1`, `pkg` pattern and length, `name` 1 to 128 scalars, `versionName` regex, `versionCode` range, `abi` in `1.4`/`1.5`, `lang` length, `hosts` array, `signerPub` 32 bytes when present.
  - A `pkg` that differs from the index entry is rejected at install (edge case I12).

### T-18a-049 — Non-blocking startup load
- Priority: P0
- Effort: S
- Depends: T-18a-005, T-18a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt
- Acceptance:
  - Startup loading runs on the tokio runtime; the window appears before loading finishes; the Browse list fills on `Loaded`.
  - No `block_on` on the GTK thread (replaces `runBlocking` at MangaExtensionLoader.kt:177).

---

## Views, details and UI data

### T-18a-050 — Orphaned state data for the UI
- Priority: P1
- Effort: S
- Depends: T-18a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt
- Acceptance:
  - Rows expose `is_obsolete` so the UI can show the `ORPHANED` tag and the details banner text (ui-parity sections 2 and 9).

### T-18a-051 — Extension details data
- Priority: P1
- Effort: S
- Depends: T-18a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/model/MangaExtension.kt
- Acceptance:
  - Details expose version, language, NSFW flag, package name, repo URL, and the list of sources with enabled state (behaviour section 11.3).

### T-18a-052 — Open repository website
- Priority: P2
- Effort: S
- Depends: T-18a-051
- Sources: domain/src/main/java/mihon/domain/extensionrepo/model/ExtensionRepo.kt
- Acceptance:
  - The details screen opens the repo `website` through `gio::AppInfo::launch_default_for_uri` (ui-parity section 6 for the WebView replacement).
  - The button is hidden when the repo has no website.

### T-18a-054 — Load problems kept for display
- Priority: P2
- Effort: S
- Depends: T-18a-008, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt
- Acceptance:
  - The store keeps each package's last `LoadError` (or untrusted reason) in a map keyed by package file name, cleared when the package is reloaded successfully.
  - A problem list is available to the details screen (area:17-source-app-layer renders it), with the Kotlin log text from data section 13 as the message.

### T-18a-053 — Update all command
- Priority: P1
- Effort: S
- Depends: T-18a-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt
- Acceptance:
  - `Update all` enqueues one update per row in `updates`, in the store's order; rows already installing are skipped (ui-parity section 2.2).
