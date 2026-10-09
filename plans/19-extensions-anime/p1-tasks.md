# 19-extensions-anime: implementation tasks (part 1 of 1)

Scope: anime extension loader, installer, trust, update API, and anime extension stores (37 files, 3457 LOC, see `p1-inventory.md`). Target: Kurobayomi Rust crate, GTK 4.16 through gtk4-rs 0.11, no libadwaita.

Read first: `p1-rust-design.md` (layout and types), `p1-behavior.md` (what the Kotlin does), `p1-data-and-config.md` (formats, keys, SQL), `p1-ui-parity.md` (page and dialogs), `p1-edge-cases-and-tests.md` (test IDs TST-19-NNN), and the topics `p1-topic-store-index-protocol.md`, `p1-topic-install-and-trust.md`, `p1-topic-browse-and-reconciliation.md`.

Priority rule used: P0 = needed for Kurobayomi to be usable daily with at least one working anime extension (store, package install, load, trust, browse list, source lookup). P1 = parity features most users rely on (stores UI, updates, backup of stores, cancel and queue, uninstall, languages and source toggles). P2 = long tail (protobuf and legacy formats, torrent gating detail, UI polish). P3 = Android-only mechanisms, each with a drop or replacement statement.

Cross-area dependencies are cited as `area:<id>`. The package container, the ABI, and the WASM host are owned by area:18-extensions-manga and area:16-source-api-anime. The database connection is owned by area:04-db-anime-schema-migrations. Preferences are owned by area:02-preferences-system.

---

## A. Model, ABI, container, loader

### T-19a-001 — Anime extension model types
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: domain/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeExtension.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeLoadResult.kt, app/src/main/java/eu/kanade/domain/extension/anime/model/AnimeExtensions.kt, domain/src/main/java/mihon/domain/extension/anime/model/AnimeExtensionStore.kt, app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionSources.kt
- Acceptance:
  - `src/extensions/anime/model.rs` defines `Installed`, `Available`, `AvailableRow`, `Untrusted`, `Kind` (`Wasm`, `Unsigned`, `ApkOnly`), `LoadResult`, `InstallStep` (order `Idle, Pending, Downloading, Installing, Installed, Error`), `Buckets`, `AnimeSourceRef`, `StoreRef` and `SignerDigest` with the fields listed in p1-rust-design.md section 2.
  - `InstallStep::is_completed` is true exactly for `Idle`, `Installed`, `Error` (unit test).
  - `AnimeExtensionStore` fields (`index_url`, `name`, `badge_label`, `signing_key`, `contact_website`, `contact_discord`, `is_legacy`, `extension_list_url`) exist as `StoreRow` with the types of p1-data-and-config.md section 1.6.

### T-19a-002 — AbiVersion and the supported ABI set
- Priority: P0
- Effort: S
- Depends: T-19a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt
- Acceptance:
  - `AbiVersion::parse` accepts exactly `<u32>.<u32>` and returns `None` for `"1"`, `"1.2.3"`, `"-1.0"`, `" 1.0"`, `"a.b"`.
  - `"1.10" > "1.1"` (tuple comparison; test TST-19-042).
  - `SUPPORTED_ABI` is `[1.0]` and is a single const slice; a test asserts the set and fails when it changes without a fixture update.
  - The Aniyomi lib set `[14.0, 16.0, 17.0]` is recorded as a const used only to label `ApkOnly` rows (no load path).

### T-19a-003 — Aniyomi lib value for ApkOnly listings
- Priority: P1
- Effort: S
- Depends: T-19a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkLegacyAnimeExtension.kt
- Acceptance:
  - Store listing parses `extensionLib` with `str::parse::<f64>`; a non-numeric value fails the whole store (test TST-19-017).
  - Legacy listing computes the lib as the prefix before the last dot of `version`, parsed as f64 (`"16.3"` gives 16.3; `"16"` gives 16.0). Test TST-19-061.
  - The value is stored in `Available.aniyomi_lib` only; `abi` stays `None` for ApkOnly rows.
  - The loader's metadata rule (metadata integer first, versionName fallback) is implemented in one function, `aniyomi_lib_from_metadata`, used by tests only (TST-19-060).

### T-19a-004 — `.kuroext` container reader and manifest validation
- Priority: P0
- Effort: M
- Depends: T-19a-002, area:18-extensions-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeLoadResult.kt
- Acceptance:
  - `package/manifest.rs` implements the 11 rules of p1-rust-design.md section 4.1 in order; the first failure wins and returns `LoadError::Manifest` with the rule name.
  - Tests TST-19-040 (name prefix), TST-19-041 (uppercase pkg), TST-19-043 (abi versus versionName), TST-19-045 (manifest over 64 KiB) pass.
  - Zip reading rejects entries named `..`-containing, absolute, or containing backslash (TST-19-044), and rejects `source.wasm` above 16 MiB uncompressed (TST-19-046); icon above 262144 bytes is dropped, not fatal (TST-19-047).
  - `manifest.json` is read with `deny_unknown_fields` (the store reader stays lenient).

### T-19a-005 — Package directory, scan, read-only files, signer sidecar
- Priority: P0
- Effort: S
- Depends: T-19a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt
- Acceptance:
  - Root is `$XDG_DATA_HOME/kurobayomi/extensions/anime` through the `Paths` trait (area:01-foundation-app-bootstrap); a test with a temporary `XDG_DATA_HOME` checks the path.
  - `scan` lists regular files named `<pkg>.kuroext` only; other names are ignored; a writable file is set read-only (`0444`) before it is parsed (test TST-19-039).
  - Each installed package has a `<pkg>.signer` sidecar with the 64-character digest. A package without a sidecar is `LoadError::NotSigned`.
  - Startup deletes leftover `*.kuroext.part` files in the download cache and in the package directory (data section 5.3).

### T-19a-006 — Loader pipeline with ordered checks and per-package isolation
- Priority: P0
- Effort: M
- Depends: T-19a-004, T-19a-005, T-19a-009, T-19a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt
- Acceptance:
  - `load_package` runs the steps of p1-rust-design.md section 5 in order: manifest, version name, ABI, sidecar, trust, NSFW, build `Installed`. An untrusted NSFW package returns `Untrusted` (test TST-19-034).
  - Log texts for each failure equal the Kotlin texts in p1-data-and-config.md section 11 (`Missing versionName for extension $extName`, `Package $pkgName isn't signed`, `Extension $pkgName isn't trusted`, `NSFW extension $pkgName not allowed`).
  - One bad package never stops the scan (test TST-19-031).
  - Discovery never runs package code: no WASM instance and no class loading during `load_package` (checked by a test that puts a non-WASM `source.wasm` in the package and still gets `Success`).
  - `Installed.lang` is the manifest `lang`. The installer rejects a package whose `lang` differs from the derivation of its sources (test TST-19-037).

### T-19a-007 — NSFW gate read on every scan
- Priority: P1
- Effort: S
- Depends: T-19a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt
- Acceptance:
  - The `show_nsfw_source` value is read at each `scan`, not cached for the process (Aniyomi caches it at LDR:36-38). Test TST-19-035 changes the value between two scans and gets different results.
  - NSFW is `manifest.nsfw` everywhere; the store `contentWarning >= Mixed` rule and the loader rule are the same function (test TST-19-036).

### T-19a-008 — Torrent flag on installed and available packages
- Priority: P2
- Effort: S
- Depends: T-19a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt, domain/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeExtension.kt
- Acceptance:
  - `torrent` is read from the manifest and from the store `isTorrent`; the value is carried into `Installed.torrent` and `Available.torrent` (test TST-19-048).
  - The Browse row shows a torrent marker when the flag is set (UI hook only; the marker is drawn by area:41-ui-browse-anime).

---

## B. Trust and verification

### T-19a-009 — Trust rule and trust storage
- Priority: P0
- Effort: S
- Depends: T-19a-001, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/extension/anime/interactor/TrustAnimeExtension.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - `TrustStore::is_trusted` returns true when any store `signing_key` equals any signer digest (repo rule) or when `"<pkg>:<versionCode>:<digest>"` is in the stored set (per-version rule). Tests TST-19-095 and TST-19-030 pass.
  - The stored set key is `app-state` `trusted_extensions` (area:02-preferences-system). The entry format is unchanged.
  - Store row `signing_key = "NO_SIGNING_KEY"` never matches a real digest (TST-19-030).

### T-19a-010 — Trust mutations, revoke, and the settings action
- Priority: P1
- Effort: S
- Depends: T-19a-009
- Sources: app/src/main/java/eu/kanade/domain/extension/anime/interactor/TrustAnimeExtension.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - `trust(pkg, version_code, signer)` removes earlier entries for the same package and adds the new one (TST-19-092).
  - `revoke_all` removes only per-version entries; repo trust still works afterwards (TST-19-094).
  - The settings row `Revoke trusted unknown extensions` calls the anime revoke, the manga revoke (area:18-extensions-manga), and shows `Requires app restart to take effect`.

### T-19a-011 — Package verification: digest, publicKey, ed25519, store signer
- Priority: P0
- Effort: M
- Depends: T-19a-004, T-19a-009, T-19a-013
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/lang/Hash.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt
- Acceptance:
  - `verify_download` runs in this order: SHA-256 equals `kuroext.sha256`; `publicKey` is 32 bytes; `hex(sha256(publicKey))` equals the store `signing_key`; `verify_strict` of `kuroext.ed25519` over the raw 32-byte digest. Each failure has its own `VerifyError` variant (p1-rust-design.md section 4.2).
  - Tests with `kuroext_bad_sha`, `kuroext_bad_sig`, `kuroext_wrong_signer`, `kuroext_pk_short`, and the all-zero small-order key give `DigestMismatch`, `BadSignature`, `UntrustedSigner`, `BadKey`, `BadKey` (TST-19-099).
  - Signatures are computed over the raw 32-byte digest, not over the file (test: changing the file while keeping the digest fails the digest check first).

### T-19a-012 — Index addition: `resources.kuroext` block
- Priority: P0
- Effort: S
- Depends: T-19a-013
- Sources: data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt
- Acceptance:
  - The Rust wire struct `ResourcesJson` has `kuroext: Option<KuroextJson>` with `url`, `sha256`, `ed25519`, `publicKey` (data section 8.2). Aniyomi ignores the field (test: the same document decodes in the Kotlin-compatible reader with unknown keys ignored).
  - `publicKey` is a documented addition to area:18-extensions-manga's field list; the area:18-extensions-manga owner is told through the area:18-extensions-manga plan (cross-check in T-19a-013).
  - Validation: `sha256` 64 lowercase hex; `ed25519` 64 bytes base64; `publicKey` 32 bytes base64; `url` absolute or resolved against the index URL. A malformed `sha256` or `url` gives `ApkOnly`; a missing or malformed `ed25519` or `publicKey` gives `Unsigned`.

---

## C. Store protocol

### T-19a-013 — Store JSON fetch and detection (JSON only)
- Priority: P0
- Effort: M
- Depends: T-19a-001, area:12-network-stack
- Sources: data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt, data/src/main/java/mihon/data/extension/anime/model/BaseNetworkAnimeExtensionStore.kt
- Acceptance:
  - `fetch_store` reads a store with a first byte `{` and returns a `StoreRow` with `is_legacy = false` (fixture `store_ok.json`, checked by TST-19-023 and TST-19-024; the same store behind a redirect is TST-19-012).
  - Required fields missing make the store fail (TST-19-054); unknown keys are ignored (TST-19-052); `"discord": null` gives `None` (TST-19-053).
  - Requests use the shared reqwest client from area:12-network-stack with a 30 s timeout and `Cache-Control: no-cache` on refresh requests (data section 6).
  - The `resources.kuroext` reader is the one from T-19a-012 (no second parser).

### T-19a-014 — Protobuf store and extension list decode
- Priority: P2
- Effort: M
- Depends: T-19a-013
- Sources: data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt, data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt
- Acceptance:
  - `prost` messages in `wire.rs` use the tags of p1-data-and-config.md section 3.1 (store 1 to 4, 101, 102; list 1; extension 1 to 9; resources 1, 2; source 1 to 6).
  - A protobuf store decodes to the same `StoreRow` as the JSON sample (TST-19-016); `ContentWarning` value 9 fails the store (TST-19-055).
  - The first-byte dispatch (`{` JSON, else protobuf) is shared by the store and list paths (topic section 4).

### T-19a-015 — Gzip for store and list bodies
- Priority: P1
- Effort: S
- Depends: T-19a-013
- Sources: data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt
- Acceptance:
  - Bodies starting with `1f 8b` are gunzipped for store and list GETs (TST-19-014, TST-19-015), and not for the legacy repo or legacy list (documented in a test that asserts a gzip legacy list fails to parse).
  - Gunzip output is capped at 64 MiB (TST-19-029). Bodies above 16 MiB are `TooLarge` (TST-19-028).

### T-19a-016 — Legacy repo and legacy list support with a redirect depth limit
- Priority: P2
- Effort: M
- Depends: T-19a-013
- Sources: data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkLegacyAnimeExtensionRepo.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkLegacyAnimeExtension.kt
- Acceptance:
  - A `[` body at a URL ending `/index.min.json` fetches `repo.json` and stores it with `is_legacy = true` (TST-19-010). The same body at another URL gives the fixed message `Provided legacy store url is not valid` (TST-19-011).
  - `repo.json` with `index_v2` redirects; five documents followed by a store succeed; six documents or a cycle give `RedirectLoop` (TST-19-012, TST-19-013).
  - Legacy `badgeLabel` is `shortName ?? name`; `signingKey` is `signingKeyFingerprint`; `extensionListUrl` is `None` (LEGREPO:85-97).
  - The legacy list maps names with the `Aniyomi: ` prefix stripped only when present (TST-19-026) and gives one synthetic source id 0 when `sources` is null or empty (TST-19-025).

### T-19a-017 — Store entry to Available mapping
- Priority: P0
- Effort: S
- Depends: T-19a-002, T-19a-003, T-19a-012, T-19a-013
- Sources: data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkLegacyAnimeExtension.kt
- Acceptance:
  - `map.rs` implements the table of p1-rust-design.md section 3.3 exactly: `lang` is one distinct language or `"all"` (zero sources gives `"all"`, fixture `store_zero_sources.json`); `nsfw` is `content_warning >= Mixed` (TST-19-018).
  - `pkg`, `apk_url`, `icon_url`, `version_code`, `version_name`, `torrent`, `sources` (`id` as i64, `base_url` from `homeUrl`) match the sample store (TST-19-024, TST-19-023).
  - Display name strips `Aniyomi: ` in both paths; the raw name is kept (TST-19-027).
  - A non-legacy store name keeps its prefix in the raw field (E-STO-17).

### T-19a-018 — Extension list branches (extensionListUrl, store list, legacy list)
- Priority: P1
- Effort: S
- Depends: T-19a-013, T-19a-015, T-19a-016
- Sources: data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt
- Acceptance:
  - Branch order is: `extensionListUrl` present, then non-legacy store list, then legacy list (topic section 3). Test TST-19-022 checks that the URL wins over the inline list.
  - A non-legacy store without `extensionList` and without `extensionListUrl` fails with `Invalid("extensionList is missing")` (TST-19-021).
  - `extensionListUrl` lists are fetched through the same gunzip and detection code (TST-19-051 for protobuf).

---

## D. Store persistence and aggregation

### T-19a-019 — `extension_store` table access (rusqlite)
- Priority: P1
- Effort: S
- Depends: area:04-db-anime-schema-migrations, T-19a-001
- Sources: data/src/main/sqldelightanime/dataanime/extension_store.sq, domain/src/main/java/mihon/domain/extension/anime/repository/AnimeExtensionStoreRepository.kt, data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt, domain/src/main/java/mihon/domain/extension/anime/interactor/GetAnimeExtensionStores.kt, domain/src/main/java/mihon/domain/extension/anime/interactor/GetAnimeExtensionStoreCountAsFlow.kt
- Acceptance:
  - `repo.rs` provides `get`, `list` (`ORDER BY rowid`), `count`, `upsert` (`INSERT ... ON CONFLICT(index_url) DO UPDATE`), `delete`, and a change notification to the main thread (replaces the `Flow`s of `getAllAsFlow` and `getCountAsFlow`).
  - The table name follows the decision of p1-data-and-config.md section 7.3 (one name, recorded in the area:04-db-anime-schema-migrations plan).
  - Upsert keeps the row's `index_url` and updates the other seven columns (test: upsert twice with changed name gives one row with the new name).
  - `is_legacy` accepts only 0 and 1 (pitfall 13 in the design).

### T-19a-020 — Store insert and refresh semantics
- Priority: P1
- Effort: S
- Depends: T-19a-019, T-19a-013
- Sources: data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt
- Acceptance:
  - `insert(url)` stores the **final** URL after redirects and legacy rewrites (TST-19-012), and fails without writing when fetch fails.
  - `refresh_all` fetches every stored row in rowid order and upserts the result; a failing row is logged with the text `Failed to refresh extension store '<name> (<index_url>)'` and left unchanged (TST-19-056).
  - A row removed while a refresh is running is not re-created (race R-08, test TST-19-059 companion).

### T-19a-021 — Aggregate Available list with per-store results and cancel safety
- Priority: P0
- Effort: M
- Depends: T-19a-017, T-19a-018, T-19a-020
- Sources: data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt
- Acceptance:
  - All stores are fetched with at most 4 concurrent requests; the result is `(Vec<Available>, Vec<StoreFailure>)`, so a failed store is reported (TST-19-056).
  - A cancelled refresh returns `Cancelled` and does not replace the previous available list (TST-19-057). The manager keeps the old list.
  - Duplicate package names: the list keeps first-match order for reconciliation and last-match for the map (rule of topic section 2 and design 7.4 chose first match everywhere; test TST-19-058).
  - Empty result with no stores is an empty list and does not error.

### T-19a-022 — Legacy migration rows and backup import of legacy repos
- Priority: P2
- Effort: S
- Depends: T-19a-019
- Sources: data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - `insert_from_legacy(index_url, name)` writes `badge_label = name`, `signing_key = "NO_SIGNING_KEY"`, `contact_website = index_url`, `is_legacy = false`, as Aniyomi does (SREPO:23-36). It is only reached from a backup import of an old `anime_extension_repos` value (data section 4).
  - The import of `anime_extension_repos` runs once and is not run on normal start (matches area:11-app-migrations's finding that version 7 never runs on current installs).
  - Test TST-19-049: the migrated row fails at fetch with a store-level error reported to the page.

---

## E. Manager, reconciliation, browse

### T-19a-023 — Manager state, events, and single writer on the main thread
- Priority: P1
- Effort: M
- Depends: T-19a-001, T-19a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt
- Acceptance:
  - `AnimeExtensions` holds `Rc<RefCell<State>>` with `BTreeMap`s for installed, untrusted, and available (design 7.1). Only the GTK main thread mutates it (test: the type is `!Send`, a compile-time check with `static_assertions::assert_not_impl_any!`).
  - Events (`Loaded`, `StoresChanged`, `AvailableRefreshed`, `Step`, `Installed`, `Removed`, `Trusted`, `UpdatesFound`) are applied in arrival order by one `spawn_local` loop.
  - A generation counter drops results of superseded refresh jobs (race R-03).
  - Subscribers (the stores page, the browse list, the settings row) are notified after each applied event.

### T-19a-024 — Startup load and initialized flag
- Priority: P0
- Effort: S
- Depends: T-19a-006, T-19a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt
- Acceptance:
  - Startup runs `scan` on the worker and sends `Loaded(Vec<LoadResult>)`. Success goes to `installed`, untrusted goes to `untrusted`, errors are logged and dropped (MGR:130-142).
  - `initialized` becomes true after the first `Loaded`. Browse shows a spinner before that.
  - Test TST-19-201: the main thread does not block on the scan (no `block_on` on the GTK thread; checked by a grep test and a timing test).

### T-19a-025 — Reconciliation of installed packages with the available list
- Priority: P0
- Effort: M
- Depends: T-19a-023, T-19a-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt
- Acceptance:
  - Implements topic section 2 exactly: first match; `has_update` by version code or abi tuple; `store` attached; obsolete set when no store lists the package.
  - Obsolete is reset to false when the package is found again (test TST-19-064; fixes the sticky defect).
  - An empty available list sets the update count to 0 and leaves the installed list unchanged (TST-19-063).
  - Removing a store that supplied an installed package makes it obsolete on the next refresh (TST-19-059).

### T-19a-026 — Update count derived in memory
- Priority: P1
- Effort: S
- Depends: T-19a-025
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - `update_count` counts installed packages with `has_update && !is_obsolete` (TST-19-065) and is recomputed after every state change.
  - The count is not persisted; the Home badge (area:36-ui-shell-navigation-security-webview) reads the in-memory value.
  - The notification is withdrawn when the count reaches zero (TST-19-106).

### T-19a-027 — Browse buckets
- Priority: P1
- Effort: M
- Depends: T-19a-025, T-19a-001
- Sources: app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionsByType.kt
- Acceptance:
  - `buckets()` returns updates, installed (obsolete first, then name), untrusted (name, no NSFW filter), and available rows (filter by not installed, not untrusted, NSFW gate; flat-map by enabled source languages) as topic section 4 specifies (TST-19-066, TST-19-069).
  - An available extension with zero sources is kept as one row with `source = None` (TST-19-067). This is a deliberate change from Aniyomi (E-REC-04).
  - Split rows have row keys `<pkg>-<sourceId>` and the same bare `pkg` for installs (TST-19-068).
  - The NSFW setting is read on each rebuild (TST-19-070).

### T-19a-028 — Language list, default set, and first-run enablement
- Priority: P1
- Effort: S
- Depends: T-19a-027
- Sources: app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionLanguages.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt
- Acceptance:
  - Language list is distinct source languages, sorted by enabled-first then the comparator with `"all"` first (TST-19-071 for empty).
  - Default enabled set is `{all, en, device language}` (LocaleHelper.kt:72-74).
  - First run with device `zh` and store languages `zh-Hans`, `zh-Hant`, `en` gives `{all, en, zh, zh-Hans, zh-Hant}` (TST-19-072), and the second run changes nothing (TST-19-073).

### T-19a-029 — Source toggles and the label rule
- Priority: P1
- Effort: S
- Depends: T-19a-027
- Sources: app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionSources.kt
- Acceptance:
  - `enabled` is `source.id.to_string()` not in `hidden_anime_catalogues`; ids are kept as i64 (no float round trip; TST-19-024).
  - `labelAsName` is `multi && !(all names equal)` (TST-19-074).
  - Toggling a source rebuilds the browse list.

### T-19a-030 — Source id lookup without NPE
- Priority: P0
- Effort: S
- Depends: T-19a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt
- Acceptance:
  - `source_for_id` returns the installed handle, else a stub handle for an available source, else `None` (topic section 7).
  - Calls on a stub return `SourceError::NotInstalled` (TST-19-108); a stub with a blank name displays its id (TST-19-109).
  - Icon lookup for a removed package returns `None` (TST-19-107) where Aniyomi throws NPE (MGR:108-111).

---

## F. Installer

### T-19a-031 — Download step: streamed GET to a partial file
- Priority: P0
- Effort: M
- Depends: T-19a-023, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt
- Acceptance:
  - `download` writes `<pkg>-<versionCode>.kuroext.part` under `$XDG_CACHE_HOME/kurobayomi/ext-download`, sends progress at most every 250 ms, and checks the cancel token between chunks.
  - Non-2xx responses and I/O errors delete the partial file and return `DownloadError`.
  - Bodies above 64 MiB return `TooLarge` and delete the partial file (TST-19-090).
  - Test TST-19-089: a write error returns `Io` and removes the partial file.

### T-19a-032 — Install pipeline state machine
- Priority: P0
- Effort: M
- Depends: T-19a-031, T-19a-011, T-19a-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt, app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt
- Acceptance:
  - Steps follow design section 6.1: `Pending`, `Downloading`, `Installing` (verify and place), then `Installed` or `Error`. The `Installing` step is always emitted (fixes E-INS-03, TST-19-077).
  - Steps are keyed by bare `pkg` (TST-19-068 companion); a request for a `pkg` already downloading returns the running entry (TST-19-085).
  - A failure at any stage leaves no partial file (TST-19-078) and emits `Error`.
  - `updateExtension` for a package with no available entry emits nothing and returns (Aniyomi empty flow, MGR:254-257), and the page shows a message `No store lists this extension any more`.

### T-19a-033 — Placement: downgrade, signer continuity, atomic rename
- Priority: P0
- Effort: M
- Depends: T-19a-005, T-19a-011
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt
- Acceptance:
  - `place` refuses a lower version (TST-19-081), accepts an equal version (TST-19-082), refuses a different signer on upgrade (TST-19-083), and refuses a first install whose signer is not the store's (TST-19-033).
  - Writes `<pkg>.kuroext.part` next to the target, `fsync`, `rename`, sets `0444`, writes `<pkg>.signer`, and `fsync`s the directory.
  - A crash between write and rename leaves the old file (TST-19-088).
  - Placement is serialised by one mutex (TST-19-091).
  - Log texts equal the Kotlin texts (`Installed extension version is higher. Downgrading is not allowed.`, `Installed extension signature is not matched.`).

### T-19a-034 — Install queue with serial placement
- Priority: P1
- Effort: M
- Depends: T-19a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/InstallerAnime.kt
- Acceptance:
  - Entries run FIFO; downloads run in parallel; placement is serialised (TST-19-091).
  - Cancel of a queued entry removes it and emits `Idle` (TST-19-086).
  - Shutdown sets queued entries to `Error` and cancels the active entry to `Idle` (TST-19-087; fixes E-INS-14).

### T-19a-035 — Cancel install without a stuck row
- Priority: P1
- Effort: S
- Depends: T-19a-032, T-19a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/InstallerAnime.kt
- Acceptance:
  - Cancel during download emits `Idle`, removes the partial file, and the row leaves `Downloading` (TST-19-075; fixes E-INS-01).
  - A stale cancel does not remove a newer download for the same pkg (TST-19-076; fixes E-INS-02).
  - Cancel after `Installed` is a no-op and keeps `Installed`.

### T-19a-036 — Uninstall
- Priority: P1
- Effort: S
- Depends: T-19a-033
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt
- Acceptance:
  - `uninstall(pkg)` deletes `<pkg>.kuroext` and `<pkg>.signer` and emits `Removed` (TST-19-084 for an absent file returns `Ok`).
  - The manager removes the pkg from the installed and untrusted maps and clears its step (behavior 5.11).
  - Uninstall of an untrusted package deletes its file and removes it from the untrusted list.

### T-19a-037 — Install route: single kuroext path, preference dropped
- Priority: P3
- Effort: S
- Depends: T-19a-032
- Sources: app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt
- Acceptance:
  - Drop: the `extension_installer` preference (values `LEGACY`, `PACKAGEINSTALLER`, `SHIZUKU`, `PRIVATE`) and its settings row are not ported. Replacement: the single kuroext install path (T-19a-032). User loses: the choice of system installer. Nothing else.
  - A backup that holds `extension_installer` is read and the value is ignored (no error).
  - Test: the installer selection function has one input and one output.

### T-19a-038 — Drop: Android PackageInstaller session installer
- Priority: P3
- Effort: S
- Depends: T-19a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/PackageInstallerInstallerAnime.kt
- Acceptance:
  - Dropped with reason: Android `PackageInstaller` session API, no equivalent on Linux. User loses: nothing on Linux; Aniyomi APK extensions are not runnable anyway (ApkOnly).
  - The file is not ported. A grep check in CI finds no `PackageInstaller` identifiers in `src/`.

### T-19a-039 — Drop: Shizuku installer
- Priority: P3
- Effort: S
- Depends: T-19a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/ShizukuInstallerAnime.kt
- Acceptance:
  - Dropped with reason: Android Shizuku and `pm install-*` shell commands. User loses: nothing on Linux.
  - Settings text `Install and start Shizuku` is not shown.

### T-19a-040 — Drop: Legacy install activity and MIUI workaround
- Priority: P3
- Effort: S
- Depends: T-19a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallActivity.kt
- Acceptance:
  - Dropped with reason: `ACTION_INSTALL_PACKAGE` with `startActivityForResult`, MIUI-only timing workaround (1 s window). User loses: nothing on Linux.
  - Result code mapping (`RESULT_OK` → `Installed`, `RESULT_CANCELED` → `Idle`, other → `Error`) is covered by the state machine test (TST-19-077).

### T-19a-041 — Drop: foreground install service and notification -402
- Priority: P3
- Effort: S
- Depends: T-19a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallService.kt
- Acceptance:
  - Dropped with reason: Android foreground service with `shortService` type and notification id `-402`. Replacement: in-page progress and the install step (T-19a-032). User loses: the system-tray notification `Installing extension…` during install.
  - The string `ext_install_service_notif` is not shown in Kurobayomi.

### T-19a-042 — Drop: DownloadManager, its polling, and its completion broadcast
- Priority: P3
- Effort: S
- Depends: T-19a-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt
- Acceptance:
  - Dropped with reason: `DownloadManager` (enqueue, query, remove) and `ACTION_DOWNLOAD_COMPLETE` broadcast. Replacement: streamed download returning a future (T-19a-031). User loses: the Android download notification and the OS download list.
  - The one-second poll (INST:147) is not ported; progress is pushed.

### T-19a-043 — Replace Android package broadcasts with direct events
- Priority: P3
- Effort: S
- Depends: T-19a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallReceiver.kt
- Acceptance:
  - Dropped with reason: `ACTION_PACKAGE_ADDED`, `_REPLACED`, `_REMOVED` and self broadcasts. Replacement: `Event::Installed`, `Event::Removed` sent by the installer (T-19a-032, T-19a-036).
  - `EXTRA_REPLACING` handling is not needed (no system package events).
  - The receiver's listener semantics are kept in the manager (behavior 5.11).

### T-19a-044 — Drop: dynamic class loading and source factories
- Priority: P3
- Effort: S
- Depends: T-19a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt
- Acceptance:
  - Dropped with reason: `ChildFirstPathClassLoader`, `PathClassLoader`, `Class.forName`, `newInstance`, and `AnimeSourceFactory.createSources()`. Replacement: manifest `sources[]` and a WASM instance on first use (area:16-source-api-anime, area:18-extensions-manga). User loses: installing Aniyomi APK extensions (ApkOnly).
  - The source list in the manifest is the only source of truth at discovery (test TST-19-038 for a failing source).

### T-19a-045 — Drop: PackageManager, signatures, base paths, FileProvider, Android feature checks
- Priority: P3
- Effort: S
- Depends: T-19a-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt
- Acceptance:
  - Dropped with reason: `getInstalledPackages`, `getPackageArchiveInfo`, `getPackageInfo`, `PackageInfoCompat`, `fixBasePaths`, `getUriCompat` (FileProvider), `isPackageInstalled`, `hasMiuiPackageInstaller`, `isShizukuInstalled`. Replacement: directory scan and manifest read (T-19a-005, T-19a-004). User loses: packages installed by other Android apps.
  - `getUriSize` becomes `std::fs::metadata(...).len()` (Port).

### T-19a-046 — Extension icons without NPE
- Priority: P1
- Effort: S
- Depends: T-19a-033, T-19a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt
- Acceptance:
  - Icon source order: `icon.png` in the package, then cache under `ext-icons/<pkg>.png` fetched from the store `iconUrl` (the iconUrl fetch goes through area:34-images-covers-cache).
  - Missing icon gives a generic icon; never a panic (TST-19-107).
  - Icon cache is `RefCell`-owned by the main thread (the Kotlin `iconMap` is not thread-safe, MGR:66).

---

## G. Store actions and stores page

### T-19a-047 — Store actions: add, delete, refresh (service layer)
- Priority: P1
- Effort: S
- Depends: T-19a-020, T-19a-021
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt, domain/src/main/java/mihon/domain/extension/anime/interactor/AddAnimeExtensionStore.kt, domain/src/main/java/mihon/domain/extension/anime/interactor/RemoveAnimeExtensionStore.kt, domain/src/main/java/mihon/domain/extension/anime/interactor/UpdateAnimeExtensionStores.kt
- Acceptance:
  - `add_store(url)` trims the URL (K-11), fetches it, stores the final URL, then starts a full refresh; a duplicate is refused before fetch (TST-19-115, TST-19-116, TST-19-118).
  - `delete_store(url)` deletes the row and then starts a refresh (SMODEL:110-115).
  - `refresh_all()` runs the full refresh: store documents and extension lists (K-5; TST-19-105).
  - Errors are returned as `StoreError` variants, mapped to the fixed strings of p1-ui-parity.md section 10 (TST-19-113 for restore text is in T-19a-054).

### T-19a-048 — Stores page: list, empty state, header actions, row actions
- Priority: P1
- Effort: M
- Depends: T-19a-047
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresContent.kt
- Acceptance:
  - The page is reached from Settings (`settings_anime_extension_stores`) and from the Browse tab. The mock `src/settings.rs:13` fixed cards are replaced (p1-ui-parity.md section 9).
  - Empty state shows `You haven't added an extension store yet`. Header has `Extension stores` title, a refresh button (tooltip `Refresh`), and an `Add` button.
  - Each row has name, index URL subtitle (K-8), and buttons in this order: open website (hidden when website empty, K-7), discord (only when present), copy, delete. Tooltips match p1-ui-parity.md section 7.
  - Rows keep the store order of the table; a list update keeps the scroll position.

### T-19a-049 — Create, Confirm, and Delete dialogs
- Priority: P1
- Effort: S
- Depends: T-19a-048
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresDialogs.kt
- Acceptance:
  - Create dialog: title, label `Store index URL`, URI input purpose, supporting text rule (duplicate text, `*required`, or error), `Add` enabled only when text is non-empty, not a duplicate, not processing (TST-19-115). `Processing…` while processing. Focus on open.
  - Confirm dialog: read-only URL, body `Do you wish to add the extension store below?`, duplicate and error rules, `Add` enabled only when not duplicate and not processing.
  - Delete dialog: body `Do you wish to remove the extension store "<name>" (<url>)?` with `OK` and `Cancel`.
  - Errors show the fixed strings of p1-ui-parity.md section 10. Raw exception text is only logged (K-3).

### T-19a-050 — Deep link: aniyomi://add-repo and aniyomi://extension-store
- Priority: P1
- Effort: S
- Depends: T-19a-049
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt, app/src/main/AndroidManifest.xml
- Acceptance:
  - `gio::Application` with `HANDLES_OPEN` receives `aniyomi://add-repo?url=https%3A%2F%2Fexample.org%2Frepo.json` and `aniyomi://extension-store?url=https%3A%2F%2Fexample.org%2Frepo.json`, reads the `url` query value, and opens the Confirm dialog (TST-19-117).
  - Any other scheme or host is ignored with a banner `This link is not an extension store address` (K-9).
  - A link whose target is already stored shows the duplicate text (TST-19-117, TST-19-118).
  - The `.desktop` file declares `MimeType=x-scheme-handler/aniyomi;` (area:36-ui-shell-navigation-security-webview installs it).

### T-19a-051 — Trust and uninstall actions for the Browse UI
- Priority: P1
- Effort: S
- Depends: T-19a-010, T-19a-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt
- Acceptance:
  - `trust(untrusted)` writes the per-version entry, reloads the package, and moves it to installed. Test TST-19-093 (reload untrusted keeps it out of both maps).
  - After a trust reload, reconciliation runs so that `has_update` is set (TST-19-097; fixes E-TRU-06).
  - `uninstall(pkg)` delegates to T-19a-036.
  - Untrusted dialog text is the strings of p1-data-and-config.md section 12 (`Untrusted extension`, the message, `Trust`, `Uninstall`). The dialog itself is drawn by area:41-ui-browse-anime.

### T-19a-052 — Update check on start and once a day
- Priority: P1
- Effort: S
- Depends: T-19a-021, T-19a-025
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/api/AnimeExtensionApi.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - One refresh per run: store documents fetched once and used for the list (E-UPD-02; TST-19-101).
  - A 24 h throttle applies to the scheduled check (TST-19-100). The stored key is `anime_ext_last_check` (TST-19-102), not `last_ext_check`.
  - The scheduled check uses a monotonic timer; the stored last time is wall clock only.
  - A failed refresh logs and keeps the previous data.

### T-19a-053 — Update notification
- Priority: P1
- Effort: S
- Depends: T-19a-052, T-19a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/api/AnimeExtensionApi.kt
- Acceptance:
  - `UpdatesFound(names)` shows a GIO notification with id `anime-ext-updates`, title `Extension update available` (one) or `<n> extension updates available` (plural), body of names unless `hide_notification_content` is true (TST-19-103, TST-19-104).
  - The notification is shown only when the count rises or the names change (proposal in topic 3).
  - The notification is withdrawn at zero (TST-19-106). The id does not collide with manga (E-UPD-04).

### T-19a-054 — Backup and restore of anime stores
- Priority: P1
- Effort: S
- Depends: T-19a-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeExtensionStoresBackupCreator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtensionStore.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeExtensionStoreRestorer.kt
- Acceptance:
  - Backup entries use protobuf tags 1 (indexUrl), 2 (name), 3 (badgeLabel), 4 (contactWebsite), 5 (signingKey), 6 (contactDiscord), 7 (isLegacy), 8 (extensionListUrl) (TST-19-110).
  - Restore uses `badgeLabel ?? name` and `isLegacy ?? true` (TST-19-111; E-STO-22). An existing `index_url` is upserted (TST-19-112).
  - One failed row adds `Error Adding Anime Repo: <name> : <message>` and the others continue (TST-19-113).
  - The option `extensionStores` off writes nothing (TST-19-114). The backup format itself is area:21-backup-format-and-create; the restore flow is area:22-backup-restore.

### T-19a-055 — Store trust and removal consequences
- Priority: P1
- Effort: S
- Depends: T-19a-009, T-19a-019
- Sources: app/src/main/java/eu/kanade/domain/extension/anime/interactor/TrustAnimeExtension.kt, data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt
- Acceptance:
  - Trust by store is read from the table at each load, so removing a store removes its repo trust at the next load (TST-19-096).
  - Removing a store makes its packages obsolete at the next refresh (TST-19-059); they stay installed and runnable until the next load.
  - No trust entries are deleted when a store is removed (the per-version entries remain; they match again if the store returns).

### T-19a-056 — Settings entry and removal of the mock rows
- Priority: P1
- Effort: S
- Depends: T-19a-048
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsBrowseScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreen.kt
- Acceptance:
  - Settings → Browse → Anime extension stores opens the page (T-19a-048). The mock rows `Store A` and `Store B` in `src/settings.rs:13` are gone.
  - `spec/settings.json` item for `settings_anime_extension_stores` no longer carries placeholder rows.

### T-19a-057 — Refresh and per-store failure banners
- Priority: P1
- Effort: S
- Depends: T-19a-047, T-19a-048
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt
- Acceptance:
  - The refresh button starts the full refresh (TST-19-105), shows a spinner in the header while running, and does not block the page.
  - After refresh, one banner per failed store: `Could not update store "<name>"` (K-6; TST-19-056).
  - The Browse list updates after refresh (the Aniyomi refresh did not, E-UPD-06).

### T-19a-058 — Clipboard, open-in-browser, discord actions
- Priority: P1
- Effort: S
- Depends: T-19a-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresContent.kt
- Acceptance:
  - Copy writes the index URL to the GDK clipboard and shows the banner `Copied to clipboard: <URL truncated to 50 characters with ...>` on every platform (K-4, `truncateCenter` rule).
  - Open website and discord use `gtk::UriLauncher`; a launch error shows a banner with the error text.
  - Blank content is not copied (`copyToClipboard` rule).

---

## H. Logging, concurrency, tests, and remaining P2 items

### T-19a-059 — Concurrency model and runtime wiring
- Priority: P1
- Effort: S
- Depends: T-19a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt
- Acceptance:
  - A tokio runtime with 2 worker threads and an `async_channel` from workers to the main thread (design section 10).
  - Background tasks own clones only; no `Rc` crosses threads (compile-time check).
  - Refresh uses 4 concurrent store fetches (topic section 7).
  - No `block_on` on the GTK thread (grep check in TST-19-201).

### T-19a-060 — Logging and error mapping
- Priority: P1
- Effort: S
- Depends: T-19a-006, T-19a-013, T-19a-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt, data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt
- Acceptance:
  - Every log text in p1-data-and-config.md section 11 is emitted verbatim as a `tracing` event message, with fields `pkg`, `store`, `version_code`.
  - Each `StoreError`, `VerifyError`, `DownloadError`, `PlaceError`, `LoadError` variant maps to one fixed UI string (p1-ui-parity.md section 10); a test asserts that every variant has a mapping.

### T-19a-061 — Tests: store decoding, mapping, container, verification, loader
- Priority: P0
- Effort: M
- Depends: T-19a-004, T-19a-011, T-19a-013, T-19a-017, T-19a-006
- Sources: data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt
- Acceptance:
  - Tests TST-19-010 to TST-19-030 and TST-19-031 to TST-19-048 exist and pass, using the fixtures of p1-edge-cases-and-tests.md section 5.
  - `tests/support/build_kuroext.rs` builds signed fixture packages from a test key pair; the key pair is checked in under `tests/fixtures/anime_ext/keys/`.
  - `cargo test` passes without a display (TST-19-200).

### T-19a-062 — Tests: install, queue, cancel, trust
- Priority: P1
- Effort: M
- Depends: T-19a-032, T-19a-034, T-19a-035, T-19a-036, T-19a-009, T-19a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/InstallerAnime.kt, app/src/main/java/eu/kanade/domain/extension/anime/interactor/TrustAnimeExtension.kt
- Acceptance:
  - Tests TST-19-049 to TST-19-050 and TST-19-075 to TST-19-099 exist and pass. Install tests use a local HTTP fixture server.
  - Tests TST-19-079 and TST-19-080 are recorded as "not applicable on Linux" in the test report (no external download manager).
  - Each fixed defect has a test that failed on the Kotlin reading (E-INS-01, E-INS-02, E-INS-03, E-INS-14).

### T-19a-063 — Tests: update checks, browse, reconciliation, UI logic
- Priority: P1
- Effort: M
- Depends: T-19a-025, T-19a-027, T-19a-028, T-19a-029, T-19a-052, T-19a-053, T-19a-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/api/AnimeExtensionApi.kt, app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionsByType.kt
- Acceptance:
  - Tests TST-19-058 to TST-19-074 and TST-19-100 to TST-19-121 exist and pass (headless: buckets, reconciliation, dialog state machine, deep link parsing).
  - The stores page dialog state machine is tested without GTK by a pure `DialogState` type.

### T-19a-064 — Preferences and state keys for anime extensions
- Priority: P2
- Effort: S
- Depends: T-19a-009, T-19a-026
- Sources: app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Keys and defaults of p1-data-and-config.md section 4 are registered with area:02-preferences-system: `source_languages`, `show_nsfw_source` (true), `hidden_anime_catalogues` (empty), `trusted_extensions` (app state, empty), `anime_ext_last_check` (0), `hide_notification_content` (false, shared).
  - `animeext_updates_count` is not registered (derived in memory).
  - Aniyomi's `last_ext_check` is never written by Kurobayomi (E-UPD-03).

### T-19a-065 — Legacy `anime_extension_repos` import and backup trust entries
- Priority: P2
- Effort: S
- Depends: T-19a-022, T-19a-009
- Sources: app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - A backup holding `anime_extension_repos` is imported once through T-19a-022.
  - Trust entries from an Aniyomi backup are kept in the set and never match (E-TRU-07; TST-19-098).

### T-19a-066 — Show "not runnable" packages on demand
- Priority: P2
- Effort: S
- Depends: T-19a-027
- Sources: domain/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeExtension.kt, data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt
- Acceptance:
  - `ApkOnly` and `Unsigned` rows are hidden by default; a store row shows `N entries not runnable on Linux` with the count of hidden rows (topic section 5).
  - A preference `show_not_runnable` (default false) shows them with badges `Not runnable on Linux` and `Unsigned package` and install disabled.

### T-19a-067 — Keyboard, responsive layout, and tooltips for the stores page
- Priority: P2
- Effort: S
- Depends: T-19a-048
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresContent.kt
- Acceptance:
  - Shortcuts of p1-ui-parity.md section 7 work: `Ctrl+N` opens Add, `F5` refreshes, `Escape` cancels dialogs, `Enter` submits an enabled dialog.
  - The list is capped at 720 px and centred; long URLs ellipsise with the full value in the tooltip.
  - Every icon-only button has a tooltip equal to the Kotlin `contentDescription` (or `Discord` for discord).

### T-19a-068 — Protobuf golden test and fuzz targets
- Priority: P2
- Effort: S
- Depends: T-19a-014
- Sources: data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt
- Acceptance:
  - A protobuf golden fixture decodes to the expected values (TST-19-016). If a JVM is available, the golden bytes come from kotlinx `ProtoBuf`; otherwise from `prost` with the tag table.
  - Fuzz targets for the manifest parser and the store decoders run 60 s without a panic (TST-19-202, TST-19-203).
  - Decoding a 16 MiB store stays under 128 MiB RSS (TST-19-204).

### T-19a-069 — Anime package and store end-to-end smoke test
- Priority: P0
- Effort: S
- Depends: T-19a-021, T-19a-032, T-19a-024, T-19a-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt, data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt
- Acceptance:
  - Script: start a local HTTP server with a store and one signed `.kuroext`; add the store; install the package; restart the app; the package still loads as installed and is trusted by its store; the source id resolves to the installed handle.
  - The same run with the package's store removed shows the package as obsolete after refresh.
  - The run is recorded in the project's `run` skill notes (a smoke run, not a unit test).

### T-19a-070 — Notes for the area:18-extensions-manga and area:16-source-api-anime owners (cross-area contract)
- Priority: P1
- Effort: S
- Depends: area:18-extensions-manga, area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt
- Acceptance:
  - The `publicKey` field (T-19a-012), the `Unsigned` kind name (T-19a-013 reconciliation), and the first-match rule (T-19a-025) are written into the area:18-extensions-manga plan as proposed amendments, with the test IDs.
  - The anime manifest `kind = "anime"` and the `abi` set (T-19a-002) are confirmed or amended by area:16-source-api-anime before T-19a-004 starts.
  - The shared `.kuroext` reader is used by both areas (one module, not two copies).

### T-19a-071 — Verify: no Android broadcast or intent code remains for extensions
- Priority: P3
- Effort: S
- Depends: T-19a-043
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallReceiver.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt
- Acceptance:
  - Dropped with reason: the anime receiver and its self-broadcast actions (`ACTION_EXTENSION_ADDED`, `_REPLACED`, `_REMOVED`) have no equivalent. Replacement: T-19a-043. User loses: nothing.
  - No Kurobayomi code emits an Android-style intent for extensions (grep check).

### T-19a-072 — Drop: `hasReadme`, `hasChangelog` metadata and `pkgFactory`
- Priority: P3
- Effort: S
- Depends: T-19a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeLoadResult.kt
- Acceptance:
  - Dropped with reason: the metadata keys `tachiyomi.animeextension.hasReadme` and `hasChangelog` are declared and never read in Aniyomi (LDR:44-45); `tachiyomi.animeextension.factory` is only used for `AnimeSourceFactory`, which is dropped (T-19a-044). User loses: nothing.

### T-19a-073 — Drop: `isShared` (system-installed) extension location
- Priority: P3
- Effort: S
- Depends: T-19a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeLoadResult.kt
- Acceptance:
  - Dropped with reason: the shared (system package) location and `selectExtensionPackage` between shared and private (LDR:136-168, 378-392). Replacement: one directory, one file per package. User loses: packages installed by other Android apps (not runnable on Linux anyway).
  - `isShared` is not a field of `Installed`.

### T-19a-074 — Drop: `ACTION_UNINSTALL_PACKAGE` for system packages
- Priority: P3
- Effort: S
- Depends: T-19a-036
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt
- Acceptance:
  - Dropped with reason: `ACTION_UNINSTALL_PACKAGE` and `isPackageInstalled` branch (INST:221-226). Replacement: T-19a-036. User loses: removal of system-installed extensions.

### T-19a-075 — Drop: Android notification plumbing (channel, foreground, text formats)
- Priority: P3
- Effort: S
- Depends: T-19a-053
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallService.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/api/AnimeExtensionApi.kt
- Acceptance:
  - Dropped with reason: Android `NotificationCompat`, `NotificationChannel` `ext_apk_update_channel`, `setProgress(100, 100, true)`, `setShowWhen`, `ic_ani` small icon. Replacement: GIO notification (T-19a-053) and in-page progress (T-19a-048). User loses: the channel settings screen entry on Android (no channel model in GIO).
