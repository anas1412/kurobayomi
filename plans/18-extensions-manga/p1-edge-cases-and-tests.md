# Area 18 — Manga extensions: edge cases, failure modes, races, tests

Behaviour sections are in `p1-behavior.md`. Rust design sections are in `p1-rust-design.md`.

Upstream tests: the scope lists no `*Test.kt` file, so there are no upstream tests to port from this scope. Verified with `areas.py files 18-extensions-manga 1`, which returns only the 19 main-source files.

Each case gives: the input or condition, the Aniyomi result (OBSERVED), the Kurobayomi result (Proposal), and the task that implements it.

---

## 1. Loader and discovery

| ID | Condition | Aniyomi result | Kurobayomi result | Task |
|---|---|---|---|---|
| L1 | Signed package without `tachiyomi.extension.class` metadata | `!!` throws NPE, the batch throws, app init fails (behaviour 3.8 step 9) | `manifest` validation fails: `LoadError::Manifest("missing sources/class")`; other extensions still load | T-18a-006, T-18a-008 |
| L2 | Private file name `X.ext` but manifest `pkg` is `Y` | Parsed as `Y`; `singleOrNull` by name may find none | `LoadError::Manifest("file name does not match pkg")`; not loaded | T-18a-005 |
| L3 | Two private files with the same package | Possible only with a mismatched name (L2); `singleOrNull` returns null | Impossible: one file per `pkg` by path | T-18a-005 |
| L4 | `versionName` = `""` | `Missing versionName` → Error | Manifest rejects (regex `^\d+\.\d+\.\d+$`) | T-18a-002 |
| L5 | `versionName` = `"1.4"` (no patch) | Lib prefix `"1"` → 1.0 → Error | Manifest rejects (regex) | T-18a-002 |
| L6 | `versionName` = `"1.4.0"` | Accepted | Accepted | T-18a-002 |
| L7 | `versionName` = `"1.5.99"` | Accepted (1.5) | Accepted | T-18a-002 |
| L8 | `versionName` = `"1.6.0"` | Rejected | Rejected | T-18a-002 |
| L9 | `versionName` = `"1.10.0"` | Rejected (1.1) | Rejected (minor 10 > 5) | T-18a-002 |
| L10 | Unsigned package | `Package $pkgName isn't signed` → Error | `LoadError::NotSigned` | T-18a-012 |
| L11 | Signed, key not in repos, not in trust list | `Untrusted` | `Untrusted` | T-18a-012 |
| L12 | Untrusted and NSFW | `Untrusted` (trust before NSFW) | `Untrusted` | T-18a-012 |
| L13 | Trusted and NSFW, setting off | Error | Error (`NsfwDisabled`) | T-18a-010 |
| L14 | NSFW setting toggled at runtime | Not applied until restart (`lazy`) | Applied on next load (no restart) | T-18a-010 |
| L15 | One of several classes fails to instantiate | Whole extension Error | Whole extension Error (kept) | T-18a-006 |
| L16 | Class fails with `LinkageError` | Retried with `PathClassLoader`; then Error if still failing | Not applicable (wasm has no linkage); a wasm instantiate error is `Error` | T-18a-006 |
| L17 | Factory returns zero sources | `lang = ""`, extension installed with no sources | Same | T-18a-009 |
| L18 | Catalogue sources in 3 languages | `lang = "all"` | Same | T-18a-009 |
| L19 | Non-catalogue sources only | `lang = ""` | Same | T-18a-009 |
| L20 | `tachiyomi.extension.factory` missing | `pkgFactory = null` | `factory = None` | T-18a-006 |
| L21 | Label `"Tachiyomi: X"` | `name = "X"` | `name = "X"` | T-18a-003 |
| L22 | Label `" Tachiyomi: X"` (leading space) | `substringAfter` misses the prefix → name is the whole label | Trim, then strip → `"X"` (deliberate fix) | T-18a-003 |
| L23 | Label without prefix `"Example"` | `"Example"` | `"Example"` | T-18a-003 |
| L24 | Zero-byte `.kuroext` in store | Parse fails → null → skipped | `LoadError::NotAnExtension`; listed in logs only | T-18a-005 |
| L25 | `.kuroext` file where zip is corrupt | n/a | `LoadError::Manifest("zip")` | T-18a-004 |
| L26 | Icon entry missing | `loadIcon` falls back to the default icon | Generic glyph | T-18a-011 |
| L27 | Icon larger than 256 KiB | n/a | Not extracted; glyph | T-18a-011 |
| L28 | Store directory missing at start | `listFiles()` returns null → empty | Created, empty | T-18a-005 |
| L29 | Leftover `*.part` files after crash | Not swept (Kotlin temp files `temp_<id>` deleted only on normal path) | Startup sweep deletes `*.part` and cache `download/*.part` | T-18a-034 |

## 2. Install and commit

| ID | Condition | Aniyomi result | Kurobayomi result | Task |
|---|---|---|---|---|
| I1 | New version code lower than installed | Refused, "Downgrading is not allowed." | `LoadError::Downgrade`, Error step | T-18a-037 |
| I2 | Same version code | Accepted (reinstall) | Accepted | T-18a-037 |
| I3 | Signer differs from installed signer | Refused "signature is not matched" | `LoadError::SignerMismatch`, Error | T-18a-037 |
| I4 | Signer set is a superset of installed (multi-signer) | Accepted (`containsAll`) | Not applicable: one signer. Documented | T-18a-037 |
| I5 | Download body larger than 64 MiB | DownloadManager has no cap | Error `TooLarge`; `.part` removed | T-18a-034 |
| I6 | SHA-256 of body differs from index | Not checked (no hash in index) | Error `DigestMismatch`; nothing written | T-18a-034 |
| I7 | Signature does not verify | Not checked | Error `BadSignature`; nothing written | T-18a-034 |
| I8 | Non-canonical ed25519 signature (malleable S) | n/a | Rejected by `verify_strict` | T-18a-034 |
| I9 | `ed25519Pub` digest differs from repo fingerprint | Not applicable | Error before verify | T-18a-034 |
| I10 | Network drops mid-download | DownloadManager retries internally, opaque | Error after one retry (Proposal, 1 retry with 2 s delay), `.part` removed | T-18a-034 |
| I11 | Disk full during commit | `copyTo` throws, Error, `target.delete()` | Error; `.part` removed; old file kept (atomic rename not reached) | T-18a-034 |
| I12 | Install of a package whose `pkg` differs from index entry | Not checked | Error `Manifest("pkg mismatch")` | T-18a-034 |
| I13 | Zip contains path traversal entry (`../x`) | n/a | Only named entries are read; others ignored | T-18a-004 |
| I14 | Zip entry sizes lie (declared small, actual big) | n/a | Reads are bounded with `take(limit + 1)`; over limit → Error | T-18a-004 |
| I15 | `source.wasm` larger than 32 MiB | n/a | Error `Manifest("wasm too large")` | T-18a-004 |
| I16 | Cancel while queued | Entry removed, step Idle | Same, step Idle | T-18a-036 |
| I17 | Cancel while downloading | DownloadManager removed; step NOT set (observed hang risk) | Token cancelled; `.part` removed; step Idle | T-18a-036 |
| I18 | Cancel while installing (commit) | Shizuku: refused; PackageInstaller: abandon, queue may stick | Commit is short; cancel ignored after commit starts; step Installed if it completes | T-18a-036 |
| I19 | Two installs of the same pkg from two split rows | Two downloads, two installs | One job (keyed by pkg), second request joins | T-18a-034 |
| I20 | Install request for a package with no available entry (Update) | Empty flow, no step | No step; log warn | T-18a-036 |
| I21 | Uninstall while installing | Not handled | Cancel the job, wait for the commit lock, then remove | T-18a-033 |
| I22 | Remove private file that does not exist | `delete()` returns false, ignored | Ignored | T-18a-033 |
| I23 | Remove while the package is loaded (sources live) | Sources keep running until restart | Sources dropped from the store; in-flight guest call finishes | T-18a-033 |

## 3. Index and repos

| ID | Condition | Aniyomi result | Kurobayomi result | Task |
|---|---|---|---|---|
| X1 | Index root is an object, not an array | Parse error, repo empty | Repo empty, logged | T-18a-020 |
| X2 | One entry missing `pkg` | Whole repo empty (behaviour 4.3) | That entry dropped; others kept (deliberate) | T-18a-021 |
| X3 | `code` is a string `"13"` | Whole repo empty | Entry dropped (deliberate) | T-18a-021 |
| X4 | `version` `"abc.1"` (non-numeric prefix) | `toDouble()` throws, whole repo empty | Entry dropped (deliberate) | T-18a-021 |
| X5 | `version` `"14"` (no dot) | Parsed as 14.0 then filtered | Entry dropped by manifest/lib gate | T-18a-021 |
| X6 | `sources` absent | Empty list (`explicitNulls = false`) | Empty list | T-18a-021 |
| X7 | `nsfw` is `2` | `isNsfw = false` | `nsfw = false` (only 1 is true) | T-18a-021 |
| X8 | Unknown extra keys | Ignored | Ignored (no `deny_unknown_fields`) | T-18a-021 |
| X9 | Repo returns 404 | Repo empty, others kept | Same | T-18a-020 |
| X10 | Every repo fails | `findExtensions` returns empty; no toast (per-repo errors swallowed) | `FetchFailed` event with a toast, only when zero repos answered (deliberate) | T-18a-022 |
| X11 | Repo table empty | Empty result, available map emptied | Same; installed unchanged (behaviour 6 step 1) | T-18a-022 |
| X12 | Repo base URL with trailing `/` | `//index.min.json` | Trailing `/` removed on add (deliberate) | T-18a-016 |
| X13 | Same `pkg` in two repos | Available map: last wins; reconcile: first wins | First repo in table order wins everywhere | T-18a-024 |
| X14 | Installed package disappears from every repo | `isObsolete = true` | Same | T-18a-024 |
| X15 | Obsolete package reappears | Stays obsolete (defect) | `isObsolete` reset to false | T-18a-024 |
| X16 | Repo returns a new `signingKeyFingerprint` | Ignored (refresh upsert skipped) | Same | T-18a-015 |
| X17 | Repo fingerprint is `NOFINGERPRINT-n` | Replaced on first refresh | Same | T-18a-015 |
| X18 | Refresh upsert violates UNIQUE(fingerprint) | Exception cancels sibling refreshes and fails the check | Isolated per repo; error logged; other repos refreshed | T-18a-015 |
| X19 | Two repos have the same fingerprint | Second insert throws `SaveExtensionRepoException` | Same, surfaced as `ExtError::RepoSave` with the same text | T-18a-016 |
| X20 | Replace repo whose new base URL already exists | `replace` throws `SQLiteException`, not mapped | Not implemented; UI does delete + upsert in a transaction | T-18a-017 |
| X21 | Delete repo that has installed extensions | Installed stay; next index fetch marks them obsolete | Same | T-18a-017 |
| X22 | `findAll` order | Unspecified SQL order | `ORDER BY rowid` (insert order) | T-18a-014 |
| X23 | Index entry `ed25519` missing for a `wasm` entry | n/a | Listed, install blocked with `NotSigned` | T-18a-021 |

## 4. Update check and counts

| ID | Condition | Aniyomi result | Kurobayomi result | Task |
|---|---|---|---|---|
| U1 | App launch (fromAvailable false) | Full fetch, gate not applied | Same | T-18a-026 |
| U2 | Browse open (fromAvailable true) within 24 h | Gated, returns null | Gated | T-18a-026 |
| U3 | Clock moved backwards | Gate blocks until the clock passes the stored time | Elapsed negative → not gated (deliberate) | T-18a-026 |
| U4 | Update check finds updates | Notification; count not written by this call | Notification and count written | T-18a-026 |
| U5 | Notification with `hide_notification_content` | Title only | Title only | T-18a-040 |
| U6 | Count becomes 0 | Notification dismissed | Same | T-18a-040 |
| U7 | Anime and manga notifications | Same id -401, overwrite each other | Manga id `extension-updates`; anime id separate (area:16-source-api-anime) | T-18a-040 |
| U8 | Update check while offline | Repo refresh throws → caller logs | Refresh failures are per repo; check uses cached `available` | T-18a-026 |
| U9 | `has_update` when only `libVersion` increases | True (OR rule) | True | T-18a-024 |
| U10 | `has_update` when a newer version has a lower `versionCode` | False | False | T-18a-024 |

## 5. Trust

| ID | Condition | Aniyomi result | Kurobayomi result | Task |
|---|---|---|---|---|
| T1 | Repo fingerprint equals signer | Trusted | Trusted | T-18a-012 |
| T2 | Trust version 13, then version 14 installed | Version 14 untrusted | Same | T-18a-012 |
| T3 | Trust `a.b`, package `a.b.c` also trusted | `startsWith("a.b:")` does not match `a.b.c:` | Same rule with the colon | T-18a-012 |
| T4 | Revoke all trust | Keys deleted; repo trust remains | Same | T-18a-012 |
| T5 | Trust a package not in untrusted map | No-op | No-op | T-18a-013 |
| T6 | Trust then reload fails (signer changed) | Untrusted again, not registered | Same | T-18a-013 |
| T7 | Trust key stored in preferences, app restarts | Persisted | Persisted in area:02-preferences-system app-state store | T-18a-012 |

## 6. Views and sorting

| ID | Condition | Aniyomi result | Kurobayomi result | Task |
|---|---|---|---|---|
| V1 | Installed list with one obsolete and two normal | Obsolete first (key false), then name | Same | T-18a-030 |
| V2 | Names `"abc"`, `"ABD"`, `"abb"` | Case-insensitive order | Same (`to_lowercase`) | T-18a-030 |
| V3 | Unicode names with mixed case (`"Ä"` vs `"a"`) | `CASE_INSENSITIVE_ORDER` uses per-char uppercase compare | `to_lowercase` compare; may differ for some characters (documented) | T-18a-030 |
| V4 | Available extension with 3 sources, 2 languages enabled | 2 rows with suffixed pkg | Same | T-18a-030 |
| V5 | Available extension with no sources and lang not enabled | Hidden | Hidden | T-18a-030 |
| V6 | Installed package also in available | Not shown in available | Same | T-18a-030 |
| V7 | Untrusted package also in available | Not shown in available | Same | T-18a-030 |
| V8 | NSFW available, setting off | Hidden | Hidden | T-18a-030 |
| V9 | NSFW untrusted, setting off | Shown (no NSFW filter on untrusted) | Shown (same) | T-18a-030 |
| V10 | Language list order | Enabled first, then `LocaleHelper.comparator` (`all` first) | Same | T-18a-031 |
| V11 | `labelAsName` for 2 sources, same name, different languages | False (language label) | Same | T-18a-032 |
| V12 | `labelAsName` for 2 sources, different names | True | Same | T-18a-032 |
| V13 | Source disabled | `hidden_catalogues` contains its id | Same | T-18a-032 |
| V14 | Two packages share a source id | `getExtensionPackage` returns the first in map order | First in pkg order (BTreeMap) | T-18a-029 |

## 7. First-run locale

| ID | Condition | Result (Kotlin = Rust) | Task |
|---|---|---|---|
| F1 | Device `zh`, sources `zh-Hans`, `zh-Hant`, `en` | enabled = `{all, en, zh, zh-Hans, zh-Hant}` | T-18a-023 |
| F2 | Device `en`, sources `en-GB`, `ja` | enabled = `{all, en, en-GB}` | T-18a-023 |
| F3 | Device `en`, no matching sub-languages | enabled = `{all, en}` | T-18a-023 |
| F4 | Setting already saved once | Nothing added | T-18a-023 |
| F5 | Empty available list on first run | Nothing done; flag stays false | T-18a-023 |
| F6 | `LANG=C` or unset | Device language `c` → not a real code; Kotlin would use the JVM default; Rust treats `C`, `POSIX` as `en` (deliberate) | T-18a-023 |

## 8. Races and concurrency

| ID | Race | Risk | Rust mitigation | Task |
|---|---|---|---|---|
| R1 | Two `FindAvailable` in flight | Last finisher overwrites the list with an older result | Single-flight: a second request while one runs joins it | T-18a-022 |
| R2 | Install commit while uninstall runs | File removed under the commit | Commit lock shared with uninstall | T-18a-033 |
| R3 | Trust while the same package reloads | Two loads race | Load per pkg serialised by a per-pkg mutex | T-18a-013 |
| R4 | `Step` events after `Removed` | Map resurrects a removed key | Reducer ignores `Step` for keys not in `steps` after `Removed` | T-18a-028 |
| R5 | Guest call while source is being replaced | Two stores | `Arc<Mutex<Store>>` per source, replaced atomically in the store map | T-18a-006 |
| R6 | GTK borrows the store across an `await` | Panic | No `await` while holding `RefCell` (rule in design section 14) | T-18a-028 |
| R7 | Download finishes after cancel | File committed after the user cancelled | Token checked before commit; commit is skipped | T-18a-036 |
| R8 | Index refresh during update check | Update check uses a stale list | Update check takes a snapshot after the refresh completes | T-18a-026 |

## 9. Wasm guest failure modes

| ID | Condition | Result | Task |
|---|---|---|---|
| W1 | Guest traps (panic, unreachable) | The op returns `SourceError` with the trap text; the source stays loaded | T-18a-006 |
| W2 | Guest infinite loop | Epoch deadline (30 s, 300 ticks of 100 ms) traps the call | T-18a-006 |
| W3 | Guest allocates beyond 256 MiB | `StoreLimits` denies growth; trap | T-18a-006 |
| W4 | Guest requests a host not in `hosts` | Host returns `Err("host not allowed: <host>")` | T-18a-006 |
| W5 | Guest requests `http://` in a host allowed for `https` only | Allowed if the host matches; scheme is not restricted by the manifest (documented) | T-18a-006 |
| W6 | Guest returns invalid JSON | `SourceError::Parse` | T-18a-006 |
| W7 | Guest exports missing `invoke` | Load Error `Instantiate("missing export invoke")` | T-18a-006 |
| W8 | Guest unknown `op` | `Err("unknown op: <op>")` | T-18a-006 |
| W9 | Guest version newer than abi | Load Error via manifest abi check | T-18a-002 |

## 10. Notification and UI edge cases

| ID | Condition | Result | Task |
|---|---|---|---|
| N1 | Update names are very long | Body text truncated by GNOME notification server; no app-side truncation | T-18a-040 |
| N2 | Notification server absent | `GApplication::send_notification` no-op; log once | T-18a-040 |
| N3 | Count 0 at start | No notification | T-18a-040 |
| N4 | Icon path points to deleted file | Generic glyph | T-18a-011 |
| N5 | Row action clicked twice quickly | Second click joins or is ignored (single job per pkg) | T-18a-036 |

---

## 11. Fixtures needed

| Fixture | Contents | Used by |
|---|---|---|
| `fixtures/index/valid.min.json` | Three valid entries (one with `sources`, one with `nsfw: 1`, one with lib 1.5) | X6-X8, V-series |
| `fixtures/index/mixed.min.json` | Valid entries plus one missing `pkg`, one with `code` as string, one with `version: "abc.1"` | X2-X5 |
| `fixtures/index/not-array.json` | `{ "a": 1 }` | X1 |
| `fixtures/repo/repo.json` | `meta` with 64-hex fingerprint | X16-X19 |
| `fixtures/repo/repo-nofp.json` | Repo with a different fingerprint | X16 |
| `fixtures/keys/test-seed.hex` | 32-byte ed25519 test seed (test only, never used in release) | I6-I9, T1 |
| `fixtures/pkg/valid.kuroext` | Signed package with `source.wasm` (test component) | L-series, I-series |
| `fixtures/pkg/unsigned.kuroext` | No `signerPub` | L10 |
| `fixtures/pkg/downgrade.kuroext` | Lower `versionCode` than valid | I1 |
| `fixtures/pkg/other-signer.kuroext` | Different key | I3 |
| `fixtures/pkg/bad-schema.kuroext` | `schema: 2` | Manifest tests |
| `fixtures/pkg/huge-wasm.kuroext` | `source.wasm` > 32 MiB | I15 |
| `fixtures/wasm/echo-source.wasm` | Component exporting `invoke`; returns fixture JSON; calls `http` once | W-series, T-series |
| `fixtures/wasm/trap.wasm` | Component that traps in `invoke` | W1 |
| `fixtures/wasm/spin.wasm` | Component that loops forever | W2 |
| `fixtures/wasm/grow.wasm` | Component that grows memory past 256 MiB | W3 |
| `fixtures/sqlite/schema.sql` | `extension_repos` DDL from data file 9.1 | X-series |
| Locale env table | `LANG` values: `zh_CN.UTF-8`, `en_US.UTF-8`, `C`, `POSIX`, unset | F-series |

---

## 12. Tests to write (Rust)

### 12.1 `kuro-extensions/tests/lib_version.rs`
- `parses_prefix_before_last_dot`: `"1.4.13"` → `(1,4)`, `"1.5.0"` → `(1,5)`.
- `rejects_missing_patch`: `"1.4"` → `None` or out of range (L5).
- `range_inclusive_bounds`: `1.4.0` in, `1.5.99` in, `1.3.9` out, `1.6.0` out (L6-L9).

### 12.2 `kuro-extensions/tests/trust.rs`
- `trust_key_format`: `trust_key("a.b", 13, "ab")` == `"a.b:13:ab"`.
- `trust_removes_only_same_pkg_prefix`: trusting `a.b` keeps `a.b.c:...` (T3).
- `trust_replaces_older_version` (T2).
- `repo_match_trusts_any_version` (T1).
- `revoke_keeps_repo_trust` (T4).

### 12.3 `kuro-extensions/tests/index.rs`
- Fixtures X1-X8 as table tests asserting the per-entry drop rules.
- `duplicate_pkg_first_repo_wins` (X13).
- `classifies_apk_only_and_wasm` (kind rule, 8.1).

### 12.4 `kuro-extensions/tests/reconcile.rs`
- `obsolete_marked_when_missing` (X14).
- `obsolete_reset_when_reappears` (X15).
- `empty_available_keeps_installed_and_zeroes_count` (X11).
- `has_update_by_version_or_lib` (U9, U10).

### 12.5 `kuro-extensions/tests/views.rs`
- `installed_obsolete_first_then_name` (V1, V2).
- `available_split_rows_suffixed` (V4).
- `hides_installed_and_untrusted_from_available` (V6, V7).
- `nsfw_filter_applies_to_available_not_untrusted` (V8, V9).
- `languages_enabled_first_all_first` (V10).
- `label_as_name_rules` (V11, V12).

### 12.6 `kuro-extensions/tests/install.rs`
- `verify_accepts_valid_signature` (I6-I9 happy path).
- `verify_rejects_digest_mismatch` (I6).
- `verify_rejects_malleable_signature` (I8).
- `check_install_downgrade_and_equal` (I1, I2).
- `check_install_signer_mismatch` (I3).
- `commit_is_atomic_and_readonly` (data 5.3): old file survives a failed rename; final mode `0444`.
- `cancel_queued_sets_idle` (I16); `cancel_downloading_removes_part` (I17).
- `same_pkg_joins_active_job` (I19).

### 12.7 `kuro-extensions/tests/repo.rs` (in-memory SQLite)
- `upsert_updates_existing_base_url`.
- `unique_fingerprint_conflict_maps_to_repo_save` (X19).
- `findall_orders_by_rowid` (X22).
- `should_apply_refresh_table` (X16, X17).

### 12.8 `kuro-extensions/tests/loader.rs` (temp dir)
- `scan_skips_bad_files_without_failing_batch` (L24).
- `file_name_must_match_pkg` (L2).
- `startup_sweep_removes_part_files` (L29).

### 12.9 `kuro-source-wasm/tests/sandbox.rs`
- `trap_becomes_source_error` (W1).
- `spin_hits_epoch_deadline` (W2) with a 2 s test deadline.
- `memory_limit_denies_growth` (W3).
- `disallowed_host_is_rejected` (W4).
- `unknown_export_fails_load` (W7).

### 12.10 Locale
- `first_run_enables_sublanguages` table test with F1-F6.

Test runner: `cargo test -p kuro-extensions -p kuro-source-wasm`. No GTK in these tests (the GTK layer is checked by the `run` skill as a smoke test).
