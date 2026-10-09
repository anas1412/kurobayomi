# 19-extensions-anime: edge cases, failure modes, races, and tests

Paths are repo-relative to `/home/blackbox/aniyomi-src` for Kotlin references, and to `/home/blackbox/kurobayomi` for Rust references. Behaviour IDs such as "behavior 7.8" refer to `p1-behavior.md`.

**Upstream tests in scope:** none. No `*Test.kt` or `*Spec.kt` file exists under any scope path. The scope has no unit tests for the loader, the store service, the installer, or the trust code. The only test file in the wider repo that touches extensions is `app/src/test/java/mihon/core/migration/MigratorTest.kt`, which belongs to area:11-app-migrations. Every test below is new and is written for Kurobayomi.

**Method note:** the defects marked "static" were derived by reading the code. They were not executed. Each one has a test in section 4 that confirms or refutes it on the Kurobayomi implementation (no Kotlin run is required).

---

## 1. Edge-case register

Columns: ID, area of the spec, scenario, what Aniyomi does (with source), what Kurobayomi does, test ID.

### 1.1 Loader and package validation

| ID | Scenario | Aniyomi (observed) | Kurobayomi | Test |
|---|---|---|---|---|
| E-LDR-01 | Package lacks `tachiyomi.animeextension.class` metadata | `getString(...)!!` throws NPE outside any try. The whole `loadExtensions` batch aborts (LDR:305). Static: any one bad package can crash startup. | Only that package becomes `LoadError::Manifest`. Others load. | TST-19-031 |
| E-LDR-02 | Lib version from metadata `aniyomix.extensionLib = 16` | Accepted as 16.0 (LDR:260-264). | Not applicable. ABI is a tuple in `manifest.abi`. | TST-19-060 |
| E-LDR-03 | No lib metadata, `versionName = "16.3.1"` | Parsed as `"16.3"` → 16.3, rejected (LDR:264-265). | Not applicable. | TST-19-061 |
| E-LDR-04 | No lib metadata, `versionName = "16.0"` | Accepted as 16.0 (`"16"`). | Not applicable. | TST-19-061 |
| E-LDR-05 | `versionName` empty | Error before lib check (LDR:254-257). | `MissingVersionName` before ABI check. | TST-19-032 |
| E-LDR-06 | Unsigned package (no signing info) | `Error` (LDR:272-275). | Installer refuses before copying (section 6.4). | TST-19-033 |
| E-LDR-07 | Signed, not trusted, NSFW | `Untrusted` (trust check at LDR:276 runs before NSFW check at LDR:289). | Same order. `Untrusted`. | TST-19-034 |
| E-LDR-08 | Signed, trusted, NSFW, `show_nsfw_source = false` | `Error` (LDR:291-294). Setting read once per process (LDR:36-38). | `Error` with `NsfwNotAllowed`. The setting is read on every scan. | TST-19-035 |
| E-LDR-09 | Metadata `aniyomix.contentWarning = 1` (SAFE) | Installed `isNsfw = true` because `> 0` (LDR:289). | `nsfw` comes from the manifest. Store listing (E-STO-08) says SAFE is not NSFW. Kurobayomi uses one rule: NSFW iff `>= Mixed` everywhere, documented in data section 2.1. | TST-19-036 |
| E-LDR-10 | Zero sources in the package | `lang = ""` (LDR:349-353). | `lang` from manifest. Installer rejects a manifest whose `lang` differs from the source derivation (section 5 step 7 of design). | TST-19-037 |
| E-LDR-11 | Source constructor throws | `Error` for the whole extension (LDR:342-345). No partial load. | Not applicable at discovery. The source fails on first use with `SourceError`. | TST-19-038 |
| E-LDR-12 | Two packages with the same pkg in the scan | One is picked by version (LDR:163-167). | Installed directory holds one file per pkg, so duplicates cannot occur. | TST-19-039 |
| E-LDR-13 | Manifest `name` starts with `Aniyomi: ` | Stripped (LDR:250). | Rejected (manifest rule 4 in design section 4.1). | TST-19-040 |
| E-LDR-14 | Package name with uppercase letters | Accepted by Aniyomi. | Rejected (`[a-z0-9_.]`). | TST-19-041 |
| E-LDR-15 | `abi` `"1.10"` vs `"1.1"` | Not applicable (Aniyomi Double). Static: `1.10` as Double equals `1.1`. | Tuple: `(1,10) > (1,1)`. | TST-19-042 |
| E-LDR-16 | `manifest.abi` major and minor do not match `versionName` prefix | Not checked in Aniyomi. | Rejected (design section 4.1 rule 7). | TST-19-043 |
| E-LDR-17 | Zip entry `../evil` or absolute path | Not applicable (APK). | Rejected (zip slip). | TST-19-044 |
| E-LDR-18 | `manifest.json` larger than 64 KiB | Not applicable. | Rejected. | TST-19-045 |
| E-LDR-19 | `source.wasm` larger than 16 MiB uncompressed | Not applicable. | Rejected. | TST-19-046 |
| E-LDR-20 | Icon larger than 262144 bytes | Not applicable. | Icon ignored, default icon used. | TST-19-047 |
| E-LDR-21 | `torrent` flag true | Stored; shown as torrent icon in the Browse list (outside scope). Torrent sources need the torrent backend (area:13-core-util-storage-archive-torrent, area:24-downloads-anime). | Stored. The torrent badge is shown. Playback gating is area:24-downloads-anime's decision. | TST-19-048 |

### 1.2 Store protocol and parsing

| ID | Scenario | Aniyomi (observed) | Kurobayomi | Test |
|---|---|---|---|---|
| E-STO-01 | Store URL is `index.min.json` but body is a JSON array | Legacy list path. Fetch `repo.json` (SSVC:35-42). | Same. | TST-19-010 |
| E-STO-02 | Body is a JSON array but URL does not end with `/index.min.json` | `IllegalArgumentException("Provided legacy store url is not valid")` (SSVC:36-38), shown raw in the dialog (behavior 12.1). | Same fixed string on the page (UI K-3). | TST-19-011 |
| E-STO-03 | `repo.json` with `index_v2` pointing to another store | Recursive `fetch(indexV2)` (SSVC:53-55). | Same, with depth limit 4. | TST-19-012 |
| E-STO-04 | `index_v2` cycle A → B → A | Unbounded recursion until `StackOverflowError`, which is an `Error` and not caught by `catch (e: Exception)` (SSVC:62). Static: crashes the process. | `StoreError::RedirectLoop` at depth 5. | TST-19-013 |
| E-STO-05 | Body is gzip (`1f 8b`) | Decompressed for store and list GETs (SSVC:32, 74, 86). Not for the legacy repo or legacy list (SSVC:40-42, 98). | Same rule. | TST-19-014, TST-19-015 |
| E-STO-06 | Body starts with a byte other than `[` or `{` | Protobuf decode (SSVC:50, 78, 90). | Same. A decode error becomes `StoreError::Decode`. | TST-19-016 |
| E-STO-07 | `extensionLib` is `"abc"` | `toDouble()` throws `NumberFormatException`. The whole list fails (WIRE:110). | Same parse, same whole-store failure. The error text is `This extension store could not be read` (UI). | TST-19-017 |
| E-STO-08 | `contentWarning` is `"SAFE"` in the store | `isNsfw = SAFE >= MIXED` is false (WIRE:114). | Same. | TST-19-018 |
| E-STO-09 | `contentWarning` is `"FOO"` | JSON decode error for the whole store. | Same (custom `Deserialize` error). | TST-19-019 |
| E-STO-10 | `contentWarning` is `"CONTENT_WARNING_MIXED"` | Accepted (`@JsonNames`). | Accepted (prefix stripped). | TST-19-020 |
| E-STO-11 | `extensionList` absent and `extensionListUrl` absent, not legacy | `extensionList!!` throws NPE (SSVC:92). Store fails. | `StoreError::Invalid("extensionList is missing")`. Same effect. | TST-19-021 |
| E-STO-12 | `extensionListUrl` set | List fetched from that URL, and the store's own `extensionList` is ignored (SSVC:72-82). | Same. | TST-19-022 |
| E-STO-13 | Store `Source.homeUrl` missing | `""` (WIRE:60). | Same (serde default). | TST-19-023 |
| E-STO-14 | Source id larger than 2^53 | Parsed as Long (exact). | Parsed as `i64` (exact). Compared as integers. | TST-19-024 |
| E-STO-15 | Legacy list entry with `sources: null` or missing | Synthetic source `id = 0`, `name = name`, `baseUrl = ""` (LEG:40-48). | Same. | TST-19-025 |
| E-STO-16 | Legacy `name` without `Aniyomi: ` prefix | Kept (`substringAfter` returns whole string, LEG:30). | Same. Prefix stripped only when present. | TST-19-026 |
| E-STO-17 | Non-legacy entry name starts with `Aniyomi: ` | Not stripped (WIRE:106). | Stripped for display (design 3.3 inconsistency fix). | TST-19-027 |
| E-STO-18 | Index body above 16 MiB | Accepted (no limit in Aniyomi). | `StoreError::TooLarge`. | TST-19-028 |
| E-STO-19 | Gzip bomb (small input, huge output) | Accepted. | Output cap 64 MiB, then `TooLarge`. | TST-19-029 |
| E-STO-20 | Store `signingKey` `"NO_SIGNING_KEY"` | Never matches a digest (TRUST:15-17). | Same. | TST-19-030 |
| E-STO-21 | Legacy store row created by migration: `isLegacy=false` with a `repo.json` URL | `getExtensions` branch 2 fetches `repo.json` as a store and fails (SREPO:23-36 plus SSVC:84-93). | Migration path not implemented. Backup restore of such a row re-fetches and fails the same way, reported as a store failure. | TST-19-049 |
| E-STO-22 | Backup restore row with `isLegacy` missing | `isLegacy = true`, so the store is read as legacy (BRESTORE:68). | Same default, documented. Restore shows a note: legacy store restored. | TST-19-050 |
| E-STO-23 | Protobuf store where `extension_list` absent but `extensionListUrl` set | Branch 1 is used (SSVC:72). | Same. | TST-19-051 |
| E-STO-24 | JSON with unknown keys | Ignored (`ignoreUnknownKeys`). | Ignored (serde default). | TST-19-052 |
| E-STO-25 | JSON with `"discord": null` | `null` (explicit). | `None`. | TST-19-053 |
| E-STO-26 | JSON key missing for a required field `resources.iconUrl` | Decode error, store fails (AppModule Json, no default). | Same. | TST-19-054 |
| E-STO-27 | Protobuf field 7 holds value 9 | Decode error for `ContentWarning` (no matching `@ProtoNumber`). | `prost` `try_from` error; whole store fails. | TST-19-055 |

### 1.3 Repository and aggregation

| ID | Scenario | Aniyomi (observed) | Kurobayomi | Test |
|---|---|---|---|---|
| E-REP-01 | One store fails during `fetchExtensions` | Other stores still listed. Failed store logged only (SREPO:74-78). | Same, plus a per-store message returned to the page (K-6). | TST-19-056 |
| E-REP-02 | Refresh cancelled while stores are being fetched | `CancellationException` caught by `catch (e: Exception)` and returned as `emptyList()` (SREPO:84-87). The caller writes an empty available map (MGR:158). **Static defect: cancellation wipes the Browse list.** | The worker returns `Cancelled`. The main thread keeps the previous list. | TST-19-057 |
| E-REP-03 | Two stores list the same package | Available map keeps the last (`associateBy`, MGR:158). Reconciliation keeps the first (`find`, MGR:210). | First match everywhere (design 7.4). Both sides agree. | TST-19-058 |
| E-REP-04 | Remove a store that supplied an installed extension | The row is deleted. The installed extension is marked obsolete at the next refresh (MGR:212-214). Its trust is lost at the next restart. | Same. | TST-19-059 |
| E-REP-05 | Store list has zero rows | `setupAvailableAnimeExtensionsSourcesDataMap` returns early and keeps the old stub map (MGR:119). The stub map is stale. | Stub map is rebuilt from the current list every time (no stale map). | TST-19-062 |
| E-REP-06 | `findAvailableExtensions` returns empty | `animeext_updates_count` set to 0 and installed map not touched (MGR:201-204). | Same, except the update count is recomputed from the installed map. | TST-19-063 |

### 1.4 Reconciliation, browse, languages

| ID | Scenario | Aniyomi (observed) | Kurobayomi | Test |
|---|---|---|---|---|
| E-REC-01 | Extension disappears, then reappears | `isObsolete` stays `true` until restart (MGR:215-228). **Static defect.** | Reset to `false` when found (design 7.4). | TST-19-064 |
| E-REC-02 | Installed extension is obsolete and has an update flag | Counted in `animeext_updates_count` (MGR:383). | Not counted (design 7.4 step 3). | TST-19-065 |
| E-REC-03 | Sort of installed list | Obsolete first, then name (BYTYPE:26-30). | Same. | TST-19-066 |
| E-REC-04 | Available extension with zero sources | Dropped from the Browse list (`flatMap` yields nothing, BYTYPE:41-50). **Static defect.** | One row with `source = None` (design 7.6 step 3). | TST-19-067 |
| E-REC-05 | Available extension with two sources in enabled languages | Two rows keyed `"<pkg>-<id>"`. Downloads and steps keyed by row key, so they do not share progress (BYTYPE:47, AnimeExtensionsScreenModel.kt:173). **Static defect.** | Two display rows, one step key by bare `pkg` (design 6.6). | TST-19-068 |
| E-REC-06 | Untrusted extension with an NSFW flag | Shown in Untrusted list without NSFW filter (BYTYPE:32-33). | Same (kept on purpose). | TST-19-069 |
| E-REC-07 | `show_nsfw_source` changed while browsing | Browse subscription keeps the value read at subscribe time (BYTYPE:16). | Browse rebuilds on setting change. | TST-19-070 |
| E-REC-08 | Language list with only zero-source extensions | Empty (BYLANG:18-25). | Same. | TST-19-071 |
| E-REC-09 | Device language `zh`, available `zh-Hans`, `zh-Hant`, `en` | First run: enabled = `{all, en, zh, zh-Hans, zh-Hant}` (MGR:185-190). | Same, applied once (flag persisted). | TST-19-072 |
| E-REC-10 | Setting already saved once | Sub-language enablement skipped (MGR:82). | Same. | TST-19-073 |
| E-REC-11 | Source with `name` shared by two sources in a multi-source extension | `labelAsName` false, language label (BYSRC:26). | Same. | TST-19-074 |

### 1.5 Installer and downloads

| ID | Scenario | Aniyomi (observed) | Kurobayomi | Test |
|---|---|---|---|---|
| E-INS-01 | Cancel while downloading | No `Idle` emitted. Poll ends when the DM row disappears. Merged flow never completes (INST:109-119, 134). **Static defect: UI row stuck in Downloading.** | Cancel token fires, step becomes `Idle`, partial file deleted. | TST-19-075 |
| E-INS-02 | Stale collector cancelled after a newer download started for the same pkg | Its `onCompletion` calls `deleteDownload(pkg)` and removes the newer download (INST:116-117, 248-257). **Static defect.** | Downloads keyed by a token id, not by pkg. A stale collector cannot remove a newer download. | TST-19-076 |
| E-INS-03 | PRIVATE path | Never sets `Installing` (INST:170-198). | Sets `Installing` during verify and place. | TST-19-077 |
| E-INS-04 | Temp file cannot be deleted before copy | `Error` set and return. Temp file left in cache (INST:174-178). | Partial file removed on any failure path. | TST-19-078 |
| E-INS-05 | DownloadManager row gone during poll (for example removed by the user) | Poll ends silently. Flow hangs (INST:134, 109). **Static defect.** | No external download manager. Download future always ends with `Ok`, `Cancelled`, or `Error`. | TST-19-079 |
| E-INS-06 | Downloaded APK not found after completion | `Error` (INST:300-307). | Not applicable. | TST-19-080 |
| E-INS-07 | Install of a lower version | Refused with `Installed extension version is higher. Downgrading is not allowed.` (LDR:77-82). | Same message, same rule. | TST-19-081 |
| E-INS-08 | Install of an equal version | Allowed (reinstall). | Allowed. | TST-19-082 |
| E-INS-09 | Install of a package with a different signer | Refused with `Installed extension signature is not matched.` (LDR:90-93). Not refused on first install (no current package). | Refused on upgrade (signer continuity). First install is refused by the verifier when the signer is not the store's. | TST-19-083 |
| E-INS-10 | Install of an unsigned package | Copied (first install), then never loads (E-LDR-06). | Refused before copy. | TST-19-033 |
| E-INS-11 | Uninstall of a package that is not installed | File delete and `notifyRemoved` (INST:227-230). | File delete returns `Ok` if absent. Event emitted. | TST-19-084 |
| E-INS-12 | Two installs of the same pkg, one from split row | Both start downloads (E-REC-05). | One download per pkg. A second request for the same pkg while one runs returns the existing entry. | TST-19-085 |
| E-INS-13 | Queue cancel of an entry in the middle of the queue | Removed from queue, `Idle` emitted (IQ:128-140). | Same. | TST-19-086 |
| E-INS-14 | Service destroyed with queued entries | Queued entries set to `Error`. Active entry's step left unchanged (IQ:114-119). **Static defect.** | Active entry cancelled and set to `Idle`. Queued set to `Error`. | TST-19-087 |
| E-INS-15 | Power loss during place | Not handled (Aniyomi copies in place). | `.part` + `rename`. Old file remains until rename. `.part` removed at startup. | TST-19-088 |
| E-INS-16 | Disk full during download | `DownloadManager` error becomes `Error` through the broadcast (INST:300-307). | `DownloadError::Io` becomes `Error`. Partial file removed. | TST-19-089 |
| E-INS-17 | Download over 64 MiB | Not limited. | `DownloadError::TooLarge`. | TST-19-090 |
| E-INS-18 | Concurrent place of two packages | Not serialised (each entry on its own path). | Serialised by one mutex (design 6.2). | TST-19-091 |

### 1.6 Trust

| ID | Scenario | Aniyomi (observed) | Kurobayomi | Test |
|---|---|---|---|---|
| E-TRU-01 | Trust version N then version N+1 | Entries for the package replaced (TRUST:21-26). | Same. | TST-19-092 |
| E-TRU-02 | Trust with the reload returning `Untrusted` | Nothing registered (MGR:298-300). Package in neither map. | Same, and a log line. | TST-19-093 |
| E-TRU-03 | `revokeAll` | Removes per-version entries, keeps store-level trust (TRUST:29-31). | Same. | TST-19-094 |
| E-TRU-04 | Repo-level trust: store `signingKey` equals package signer | Trusted for every version (TRUST:15-17). | Same. | TST-19-095 |
| E-TRU-05 | Trusted package whose store is removed | Trust lost at next load (store row gone). | Same. | TST-19-096 |
| E-TRU-06 | Trusted extension with an available update | `hasUpdate` not set until next refresh (MGR:308-310). | `hasUpdate` set at trust time (after the next reconciliation). | TST-19-097 |
| E-TRU-07 | `trusted_extensions` value from an Aniyomi backup | Format `"<pkg>:<code>:<hash>"`, hash is APK cert digest. Never matches a Kurobayomi digest. | Entries kept in the set but never match; harmless. | TST-19-098 |
| E-TRU-08 | Signature with small-order public key | Not applicable. | Rejected by `verify_strict` and the digest rule (design 14.5). | TST-19-099 |

### 1.7 Update API

| ID | Scenario | Aniyomi (observed) | Kurobayomi | Test |
|---|---|---|---|---|
| E-UPD-01 | Throttle with `fromAvailableExtensionList = false` | No throttle. Every app start performs a full fetch (API:39-43). | Throttle always applies (24 h, monotonic + stored last time). | TST-19-100 |
| E-UPD-02 | `checkForUpdates` network cost | Two full store fetches (API:46 and API:51). | One refresh per run (design 7.5). | TST-19-101 |
| E-UPD-03 | Shared `last_ext_check` key with manga | Same raw key (API:27, MangaExtensionApi.kt:39). | `anime_ext_last_check`. | TST-19-102 |
| E-UPD-04 | Update notification id | `-401` shared with manga, the newer one overwrites (ExtensionUpdateNotifier.kt). | GIO notification id `anime-ext-updates`. | TST-19-103 |
| E-UPD-05 | `hide_notification_content` true | Content text and BigText omitted (ExtensionUpdateNotifier.kt:31-35). | Body omitted. | TST-19-104 |
| E-UPD-06 | Store refresh button | Refreshes store rows only, Browse not updated (SMODEL:97-105). | Full refresh (UI K-5). | TST-19-105 |
| E-UPD-07 | Update count when last check had no update | Dismiss notification at zero (MGR:385-387). | Withdraw at zero. | TST-19-106 |

### 1.8 Icons, lookup, misc

| ID | Scenario | Aniyomi (observed) | Kurobayomi | Test |
|---|---|---|---|---|
| E-ICO-01 | Package removed after icon map built | `!!` NPE (MGR:108-111). | `None` (no icon). | TST-19-107 |
| E-ICO-02 | `source_for_id` for an uninstalled source | `StubAnimeSource` whose calls throw `AnimeSourceNotInstalledException` (STUB:25-57). | `SourceHandle::Stub`, calls return `SourceError::NotInstalled`. | TST-19-108 |
| E-ICO-03 | `toString()` of stub with blank name | Returns the id (STUB:59-60). | Display uses the id. | TST-19-109 |

### 1.9 Backup and restore

| ID | Scenario | Aniyomi (observed) | Kurobayomi | Test |
|---|---|---|---|---|
| E-BCK-01 | Backup field numbers | 1 indexUrl, 2 name, 3 badgeLabel, 4 contactWebsite, 5 signingKey, 6 contactDiscord, 7 isLegacy, 8 extensionListUrl (BMODEL:24-34). | Same tags (data section 9.1). | TST-19-110 |
| E-BCK-02 | Restore with `badgeLabel` null | `name` (BRESTORE:64). | Same. | TST-19-111 |
| E-BCK-03 | Restore with duplicate index URL | Upsert updates the row (SQ:27-40). | Same. | TST-19-112 |
| E-BCK-04 | Restore failure for one store | Error text `Error Adding Anime Repo: <name> : <message>`, others continue (BRESTORE / BackupRestorer.kt:244-245). | Same text, English. | TST-19-113 |
| E-BCK-05 | Backup with `extensionStores` option off | Empty list (BackupCreator.kt:199-201). | Same. | TST-19-114 |

### 1.10 UI (stores page, see `p1-ui-parity.md`)

| ID | Scenario | Aniyomi (observed) | Kurobayomi | Test |
|---|---|---|---|---|
| E-UI-01 | Add a duplicate URL | Confirm disabled, supporting text `This extension store already exists` (AnimeExtensionStoresDialogs.kt:37-42, 76). | Same. | TST-19-115 |
| E-UI-02 | URL with leading or trailing spaces | Not trimmed. Duplicate check uses the untrimmed text (dialogs:39-40). Fetch uses the untrimmed URL. | Trimmed before check and fetch (new decision, UI section 6, K-11 proposal). | TST-19-116 |
| E-UI-03 | Deep link with an already stored URL | `alreadyExists = true` (SMODEL:122). | Same. | TST-19-117 |
| E-UI-04 | Deep link with a legacy `index.min.json` URL of a stored `repo.json` store | Not detected as existing (stored URL is the rewritten one). | Detected: the redirect target is compared after resolution (design 3.2). | TST-19-118 |
| E-UI-05 | Confirm pressed twice quickly | `processing = true` after the first press (SMODEL:60-68). Second press allowed until state is updated. | The button is disabled synchronously before the job starts. | TST-19-119 |
| E-UI-06 | Dialog open while store list changes | Dialog stays, state copied (SMODEL:41-48). | Same. | TST-19-120 |
| E-UI-07 | Empty store list | `EmptyScreen` text (components/AnimeExtensionStoresScreen.kt:60-66). | Same text. | TST-19-121 |

---

## 2. Failure modes (grouped)

### 2.1 Network
- DNS failure, TLS failure, timeout: `StoreError::Http` or `Io`. Store row unchanged. Per-store message on the page (K-6).
- HTTP 404 on `repo.json` in the legacy path: `StoreError::Http(404)` for the whole add. Add dialog shows `Failed to add extension store`.
- HTTP 200 with an HTML error page: decode fails (first byte `<`, which is not `[` or `{`, so protobuf decode runs and fails). `StoreError::Decode`. The store is not added.
- Slow store: the job continues in the background. Other stores are not blocked (4 concurrent fetches, design 10).

### 2.2 Filesystem
- Package directory missing at startup: created on first install. Scan returns an empty list.
- Package directory not writable: install fails with `PlaceError::Io`. Message logged, step `Error`.
- Read-only file cannot be overwritten in place: replacement uses `rename` (design 14.8).
- `.part` left by a crash: deleted at startup (data section 5.3).

### 2.3 Crypto
- Digest mismatch: refused before the signature check.
- Signature does not verify: refused.
- Key not 32 bytes or invalid point: refused as `BadKey`.
- Store fingerprint mismatch: refused as `UntrustedSigner`, even when the signature is valid.

### 2.4 Process
- Process killed mid-install: `.part` remains and is deleted on next start. No partial package is ever visible under the final name.
- Worker panic: the job result is `Error`, the main thread is not affected (design 12).
- GTK main loop blocked: must not happen. Verified by the rule that the worker never calls `block_on` on the main thread (design 10).

---

## 3. Race conditions

| ID | Race | Aniyomi behaviour | Kurobayomi rule |
|---|---|---|---|
| R-01 | Install step update arrives after the entry was removed from the download map | `updateInstallStep` is a silent no-op for unknown ids (INST:239-241). | Step events carry the pkg. Unknown pkg is ignored. |
| R-02 | Refresh job and install job both write the installed map | Installed map is a `StateFlow`; last write wins (MGR:206-232). | Only the main thread writes state. Events are applied in order. |
| R-03 | Two refreshes overlap | Both write the available map; last to finish wins (MGR:158). | A generation counter. Only the latest generation's result is applied. |
| R-04 | Trust and load overlap | Trust writes prefs then loads (MGR:294-300). A concurrent load might read the old set. | Trust writes the set, then runs the load. The load reads the set after the write (same thread). |
| R-05 | Store add and refresh overlap | Both `upsert` the same row. Last wins. | Serialised through the repository mutex (one writer at a time). |
| R-06 | Cancel and completion arrive together | Undefined: `cancelQueue` may find the entry already removed and return (IQ:129-130). | Completion checks the token. A completed entry ignores a later cancel and keeps `Installed`. |
| R-07 | Re-install while previous install is in place | Concurrent `deleteDownload` (E-INS-02). | Keyed by pkg and install generation. |
| R-08 | Store removed during refresh | `upsert` after delete re-creates the row (SQ:27-40). | The refresh checks that the row still exists before `upsert`. |
| R-09 | Package file replaced while loaded | Aniyomi has no watcher; the next load reads the new file. | No watcher. The loaded copy is kept until restart (design 5). |

---

## 4. Test cases (new, Kurobayomi)

Test IDs are referenced from `p1-tasks.md` (acceptance criteria). Each test names its fixture (section 5) and the module it targets (design section 1).

### 4.1 Store decoding and mapping (`store::decode`, `store::map`, `wire`)

| ID | Name | Input | Expected |
|---|---|---|---|
| TST-19-010 | legacy list via index.min.json | `legacy_list.json` served at `https://example.org/index.min.json`, `repo.json` served from the same host | Fetch returns `StoreRow { is_legacy: true, index_url: "https://x/repo.json" }` |
| TST-19-011 | legacy list URL check | body is a JSON array at URL `https://x/y.json` | `StoreError::Invalid("Provided legacy store url is not valid")` |
| TST-19-012 | index_v2 redirect | repo.json with `index_v2` → store JSON | Final `StoreRow` has the store's name, `index_url` is the redirected URL |
| TST-19-013 | redirect cycle and limit | A → B → A (fixtures `cycle_a.json`, `cycle_b.json`); chain of 5 documents (4 redirects) ending in a store; chain of 6 documents | Cycle and 6-document chain give `StoreError::RedirectLoop`, no stack overflow. The 5-document chain succeeds |
| TST-19-014 | gzip store | `store.json.gz` | Same result as plain `store.json` |
| TST-19-015 | gzip extension list | `list.json.gz` via `extensionListUrl` | Same as plain |
| TST-19-016 | protobuf store | `store.pb` (golden bytes, data section 3.4) | Same fields as `store.json` |
| TST-19-017 | extensionLib not numeric | `extensionLib: "abc"` | Whole store `StoreError::Decode` |
| TST-19-018 | SAFE is not NSFW | `contentWarning: "SAFE"` | `nsfw == false` |
| TST-19-019 | unknown contentWarning | `"FOO"` | JSON decode error, whole store fails |
| TST-19-020 | prefixed contentWarning | `"CONTENT_WARNING_MIXED"` | `nsfw == true` |
| TST-19-021 | no extensionList, not legacy | store JSON without `extensionList` and without `extensionListUrl` | `StoreError::Invalid` |
| TST-19-022 | extensionListUrl wins | store with both `extensionList` and `extensionListUrl` | Extensions come from the URL, not the inline list |
| TST-19-023 | homeUrl default | source without `homeUrl` | `base_url == ""` |
| TST-19-024 | 64-bit source id | id `9007199254740993` | Exact integer round trip |
| TST-19-025 | legacy sources null | legacy entry with `sources` absent | One synthetic source id 0 |
| TST-19-026 | legacy name prefix | `name: "Plain"` | `name == "Plain"` |
| TST-19-027 | store name prefix | `name: "Aniyomi: X"` in store JSON | Display name `X`, raw name kept |
| TST-19-028 | index too large | 17 MiB body | `StoreError::TooLarge` |
| TST-19-029 | gzip bomb | 200 MiB of zeros gzipped to 200 KiB | `StoreError::TooLarge` after 64 MiB |
| TST-19-030 | NO_SIGNING_KEY never trusted | row with `signing_key = "NO_SIGNING_KEY"` and a valid package | `is_trusted == false` |
| TST-19-049 | migrated legacy row | row `is_legacy=false`, `index_url` ending in `repo.json` | `StoreError` for list fetch, reported to page |
| TST-19-050 | restore without isLegacy | backup entry with no `isLegacy` | Row has `is_legacy = true` |
| TST-19-051 | protobuf list via URL | `extensionListUrl` serving protobuf | Extensions decoded |
| TST-19-052 | unknown JSON keys | extra key `"foo": 1` at each level | Ignored |
| TST-19-053 | explicit null discord | `"discord": null` | `None` |
| TST-19-054 | missing required resources.iconUrl | removed key | Decode error |
| TST-19-055 | protobuf enum out of range | field 7 value 9 | Decode error |
| TST-19-056 | one store fails | two stores, one returns 500 | List contains the good store, failure reported for the other |
| TST-19-057 | cancelled refresh | cancel token during fetch | Previous list kept, no empty list written |
| TST-19-058 | duplicate pkg across stores | two stores list `pkg` with versions 2 and 3 | Available shows version 2 (first match) in both maps |
| TST-19-059 | remove store obsoletes | remove the store that supplied installed `pkg` | Installed `pkg` `is_obsolete == true` after refresh |
| TST-19-062 | empty list keeps no stale stub | empty list after non-empty | Stub map equals the empty list |
| TST-19-063 | empty available | `findAvailable` returns empty | Update count 0, installed map unchanged |

### 4.2 Package container, manifest, verification (`package::*`, `verify`)

| ID | Name | Input | Expected |
|---|---|---|---|
| TST-19-031 | bad package does not stop others | scan of two packages, one with `manifest.json` missing | One `Error`, one `Success` |
| TST-19-032 | empty versionName | manifest `versionName: ""` | `MissingVersionName` |
| TST-19-033 | unsigned install refused | index entry without `ed25519` | Installer refuses, nothing written |
| TST-19-034 | untrusted NSFW reports Untrusted | valid signed NSFW package, trust empty | `LoadResult::Untrusted` |
| TST-19-035 | NSFW blocked by setting | trusted NSFW package, `show_nsfw = false` | `LoadError::NsfwNotAllowed` |
| TST-19-036 | NSFW rule unified | manifest `nsfw: false`, store `contentWarning: SAFE` | Both say not NSFW |
| TST-19-037 | lang mismatch | manifest `lang: "en"`, sources all `ja` | Installer rejects the package |
| TST-19-038 | source fails on first use | source runtime error | `SourceError`, package stays `Success` |
| TST-19-039 | one file per pkg | scan with a directory holding one file per pkg | No duplicates |
| TST-19-040 | manifest name prefix | `name: "Aniyomi: X"` | Manifest rejected |
| TST-19-041 | uppercase pkg | `pkg: "Eu.Example"` | Manifest rejected |
| TST-19-042 | abi tuple order | `abi "1.10"` vs `"1.1"` | `(1,10) > (1,1)` |
| TST-19-043 | abi vs version mismatch | `abi "1.0"`, `versionName "2.0.1"` | Rejected |
| TST-19-044 | zip slip | entry `../x` | Rejected, no file outside the temp dir |
| TST-19-045 | big manifest | 70 KiB `manifest.json` | Rejected |
| TST-19-046 | big wasm | 17 MiB `source.wasm` uncompressed | Rejected |
| TST-19-047 | big icon | 300 KiB icon | Icon dropped, package loads |
| TST-19-048 | torrent flag | `torrent: true` | `Installed.torrent == true` |
| TST-19-060 | aniyomi lib from metadata | fixture `aniyomi_meta_16.json` | `aniyomi_lib == Some(16.0)` (ApkOnly listing only) |
| TST-19-061 | lib from version name | `"16.0"` and `"16.3.1"` | `16.0` accepted, `16.3` rejected (ApkOnly listing only) |
| TST-19-064 | obsolete reset | disappear then reappear | `is_obsolete == false` after reappear |
| TST-19-065 | obsolete update not counted | obsolete with `has_update` | Count excludes it |
| TST-19-066 | installed order | obsolete and normal | Obsolete first |
| TST-19-067 | zero-source row | available with zero sources | One row, `source = None` |
| TST-19-068 | split keys | two sources, same enabled language | Two rows, row keys differ, install key is `pkg` |
| TST-19-069 | untrusted no NSFW filter | untrusted NSFW | Shown |
| TST-19-070 | nsfw live | toggle setting | Browse rebuilt |
| TST-19-071 | empty language list | only zero-source extensions | Empty list |
| TST-19-072 | first run languages | device `zh`, store languages `zh-Hans`, `zh-Hant`, `en` | Enabled set as in behavior 5.6 |
| TST-19-073 | second run languages | setting already saved | Set unchanged |
| TST-19-074 | label rule | multi-source one name | Language label |
| TST-19-075 | cancel during download | cancel mid-stream | Step `Idle`, `.part` deleted, no hang |
| TST-19-076 | stale cancel | start A, start B for same pkg, cancel A | B still running |
| TST-19-077 | Installing set | normal install | Sequence contains `Installing` |
| TST-19-078 | temp cleanup | place fails | No temp file left |
| TST-19-079 | no external row | (none; the download API has no external row) | Covered by TST-19-075 |
| TST-19-080 | (not applicable on Linux) | | Recorded as not applicable |
| TST-19-081 | downgrade refused | install version 2 over 3 | Refused, message in log |
| TST-19-082 | equal version reinstall | same version | Allowed |
| TST-19-083 | signer change refused on upgrade | new signer | Refused |
| TST-19-084 | uninstall absent | uninstall pkg not present | `Ok`, event emitted |
| TST-19-085 | duplicate install request | two requests same pkg | One download |
| TST-19-086 | queue cancel middle | three queued, cancel second | Second `Idle`, others run |
| TST-19-087 | shutdown | queued and active | Queued `Error`, active `Idle` |
| TST-19-088 | crash during place | kill after `.part` written | `.part` removed at start, old file kept |
| TST-19-089 | disk full | write returns ENOSPC | `Error`, partial removed |
| TST-19-090 | download too large | 65 MiB body | `TooLarge`, `Error` |
| TST-19-091 | serial place | two installs | No interleaving (mutex) |
| TST-19-092 | trust replace | trust N, then N+1 | Only N+1 entry |
| TST-19-093 | trust reload untrusted | trust with signature change | Not registered |
| TST-19-094 | revokeAll | after trust and repo trust | Per-version gone, repo rule still works |
| TST-19-095 | repo-level trust | store key equals signer | Trusted for every version |
| TST-19-096 | trust after store removal | remove the store | Untrusted on next load |
| TST-19-097 | trust then update flag | trusted with newer store version | `has_update` after reconciliation |
| TST-19-098 | Aniyomi trust entries | entry with APK digest | Not matched, no crash |
| TST-19-099 | small-order key | all-zero public key | `BadKey` |
| TST-19-100 | throttle applies | two app starts within 24 h | Second start does not fetch |
| TST-19-101 | single fetch per run | count HTTP requests for one run | One request per store |
| TST-19-102 | key name | read config | `anime_ext_last_check` used, manga key untouched |
| TST-19-103 | notification id | show and withdraw | id `anime-ext-updates` |
| TST-19-104 | hide content | setting on | Body omitted |
| TST-19-105 | refresh updates browse | press refresh | Available list updated |
| TST-19-106 | withdraw at zero | count drops to 0 | Withdraw called |
| TST-19-107 | icon for vanished package | icon request after removal | `None`, no panic |
| TST-19-108 | stub source calls | call fetch on stub | `SourceError::NotInstalled` |
| TST-19-109 | stub display | blank name | Display id |
| TST-19-110 | backup tags | round trip | Tags 1..8 as in data 9.1 |
| TST-19-111 | badgeLabel default | null badge | Name used |
| TST-19-112 | upsert duplicate | restore same index twice | One row |
| TST-19-113 | restore error text | one row fails | Error text format |
| TST-19-114 | option off | `extensionStores = false` | Nothing written |
| TST-19-115 | duplicate in dialog | URL already stored | Confirm disabled, message |
| TST-19-116 | trim | `" https://x/repo.json "` | Trimmed, duplicate check on trimmed |
| TST-19-117 | deep link duplicate | stored URL in deep link | `alreadyExists` |
| TST-19-118 | deep link redirect duplicate | link to `index.min.json` of stored `repo.json` store | Detected |
| TST-19-119 | double confirm | two clicks | One job |
| TST-19-120 | dialog during change | list changes while open | Dialog stays |
| TST-19-121 | empty label | no stores | Empty text |

Note on TST-19-079 and TST-19-080: they describe Android-only failure paths. They are kept in the list so that the test IDs stay stable. They are marked "not applicable" in the tasks file (T-19a-057).

### 4.3 Non-functional tests
| ID | Name | Method | Pass rule |
|---|---|---|---|
| TST-19-200 | No GTK on worker | `cargo test` for `store`, `package`, `verify`, `browse`, `manager` modules with no display | Passes headless |
| TST-19-201 | Scan does not block main thread | Time `scan` with 50 packages on a worker | Main thread receives events; no `block_on` on the main thread (grep check) |
| TST-19-202 | Fuzz manifest | `cargo fuzz` target on `Manifest` parser, 60 s | No panic |
| TST-19-203 | Fuzz store JSON | `cargo fuzz` target on `StoreJson` and `PbStore` decode | No panic |
| TST-19-204 | Memory bound | decode 16 MiB store | RSS increase below 128 MiB |

---

## 5. Fixtures

All fixtures live in `tests/fixtures/anime_ext/` (Rust integration tests). Generated fixtures are produced by a test helper (`tests/support/build_kuroext.rs`) so that they are reproducible.

| Fixture | Content | Used by |
|---|---|---|
| `store_ok.json` | the sample store of data section 3.1 (one extension, one source) | TST-19-012, 018, 023, 024, 063 |
| `store_zero_sources.json` | one extension with `sources: []` and one with two sources of the same language | lang rule of T-19a-017 (`"all"` for zero sources) |
| `store_nullables.json` | store with `discord: null`, no `extensionList` (`extensionListUrl` set) | TST-19-022, 053 |
| `store_bad_extlib.json` | `extensionLib: "abc"` | TST-19-017 |
| `store_bad_cw.json` | `contentWarning: "FOO"` | TST-19-019 |
| `store_prefixed_cw.json` | `contentWarning: "CONTENT_WARNING_MIXED"` | TST-19-020 |
| `store_missing_list.json` | not legacy, no list and no URL | TST-19-021 |
| `store_unknown_keys.json` | extra keys | TST-19-052 |
| `store_missing_icon.json` | `resources.iconUrl` removed | TST-19-054 |
| `store_store_prefixed.json` | `name: "Aniyomi: X"` | TST-19-027 |
| `store.pb` | protobuf encoding of `store_ok.json`. Generated by `prost` in the test helper. If a JVM is available, a one-time golden file from kotlinx `ProtoBuf` is also stored as `store_kotlinx.pb` and must decode to the same values | TST-19-016 |
| `ext_list.pb` | protobuf `ExtensionList` | TST-19-051 |
| `ext_list.json.gz` | gzip of `ext_list.json` | TST-19-015 |
| `store.json.gz` | gzip of `store_ok.json` | TST-19-014 |
| `legacy_repo.json` | repo document with `index_v2` pointing to `store_ok.json` URL | TST-19-012 |
| `legacy_repo_noindex.json` | repo document without `index_v2` | TST-19-010 |
| `legacy_list.json` | JSON array of two legacy entries, one with `sources: null` | TST-19-010, 025 |
| `cycle_a.json`, `cycle_b.json` | repo documents pointing at each other | TST-19-013 |
| `chain_5.json`, `chain_6.json` | chains of five and six repo documents ending in `store_ok.json` | TST-19-013 (depth limit: 5 succeeds, 6 fails) |
| `big_17mib.json` | 17 MiB valid JSON | TST-19-028 |
| `bomb.json.gz` | 200 MiB zeros gzipped | TST-19-029 |
| `kuroext_ok.kuroext` | valid package built by the test helper, signed with the fixture key | TST-19-034, 036, 048, 081-083 |
| `kuroext_bad_sha.kuroext` | one byte changed | digest mismatch |
| `kuroext_bad_sig.kuroext` | signature replaced | BadSignature |
| `kuroext_wrong_signer.kuroext` | signed with another key | UntrustedSigner |
| `kuroext_pk_short.json` | index entry with 31-byte `publicKey` | BadKey |
| `kuroext_manifest_*.json` | one file per manifest rule (schema, kind, pkg, name prefix, versionName, abi mismatch, lang mismatch, too big) | TST-19-040..047 |
| `kuroext_zipslip.kuroext` | entry `../x` | TST-19-044 |
| `kuroext_big_wasm.kuroext` | 17 MiB uncompressed `source.wasm` | TST-19-046 |
| `keys/fixture.pub`, `keys/fixture.sec` | ed25519 test key pair, generated once and checked in | verification tests |
| `keys/small_order.pub` | all-zero 32-byte key | TST-19-099 |
| `aniyomi_meta_16.json` | description of APK metadata values for the ApkOnly listing tests | TST-19-060, 061 |

The fixture private key is a test secret. It is not used for any real store.

---

## 6. Coverage map

| Spec section | Tests |
|---|---|
| behavior 2 (loader) | TST-19-031..048, 060, 061 |
| behavior 3 (private install file) | TST-19-033, 081..083 |
| behavior 4 (receiver semantics) | TST-19-084, 085 (through the events) |
| behavior 5 (manager) | TST-19-058, 059, 062..066, 092..097 |
| behavior 6 (trust) | TST-19-030, 092..099 |
| behavior 7 (store protocol) | TST-19-010..030, 049..055 |
| behavior 8 (installer) | TST-19-075..080, 084..091 |
| behavior 9 (queue) | TST-19-086, 087 |
| behavior 10 (variants) | Not tested. Dropped on Linux. Recorded in design section 9 |
| behavior 11 (update API) | TST-19-100..106 |
| behavior 12 (store actions) | TST-19-115..121 |
| behavior 13 (browse) | TST-19-066..074 |
| data 9 (backup) | TST-19-110..114 |

---

## 7. Known gaps (must be stated in the plan)

- No test can confirm the behaviour of real Aniyomi extension APKs on Linux. They are not runnable (ApkOnly). This is a product decision, not a test gap.
- The WASM anime ABI (area:16-source-api-anime) is not frozen. The `abi` set in design section 4.5 (`[1.0]`) is pinned by T-19a-002 and confirmed by T-19a-070 before any package fixture depends on it. The tests use `1.0`.
- Protobuf golden bytes generated by kotlinx need a JVM. If none is available, the test relies on `prost` round-trip only, and the field-tag table in data section 3.1 is the reference.
- GTK UI behaviour (dialogs, focus, banner) is checked by a manual smoke run, not by unit tests (design section 15).
