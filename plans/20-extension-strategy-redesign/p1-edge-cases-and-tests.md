# 20-extension-strategy-redesign: edge cases, failure modes and tests

## E1. Loading and selection

| # | Case | Aniyomi behaviour (observed) | Kurobayomi expected |
|---|---|---|---|
| E1.1 | Same pkg installed shared and private | higher version code wins; tie goes to shared (MangaExtensionLoader.kt `selectExtensionPackage`) | same rule for two directories, tie to repo copy |
| E1.2 | Private file version lower than shared | shared wins | same |
| E1.3 | `versionName` missing | `Error` (warn log) | `Error(MissingVersion)` |
| E1.4 | `versionName` "1.4.x" vs "1.5.x" vs "1.6.x" | manga accepts [1.4, 1.5] | api `1.x` accepted only for WIT minor versions the host implements; others `Error(UnsupportedApi)` |
| E1.5 | Anime lib 15 | rejected (set is 14, 16, 17) | not applicable; api version is WIT-based |
| E1.6 | Unsigned package | `Error` | `Error(Unsigned)` when `key_id` missing and not dev mode |
| E1.7 | Signature change on update | refused at install (`containsAll` fails) | refused: `key_id` differs from installed row, `Error(KeyChanged)` |
| E1.8 | Downgrade | refused at install | refused: `version_code` lower than installed, `Error(Downgrade)` |
| E1.9 | Class listed in metadata does not exist | `Error` for the whole package | `Error` for the whole package, reason `MissingEntry` |
| E1.10 | One source of a factory throws | whole package `Error` (try block wraps `createSources`) | same |
| E1.11 | NSFW package with setting off | `Error` (not shown) | filtered out before registry, not shown in untrusted list |
| E1.12 | Extension directory with `manifest.toml` but no `component.wasm` | n/a | `Error(MissingComponent)` |
| E1.13 | Corrupted `component.wasm` (sha256 mismatch) | n/a | `Error(ChecksumMismatch)`, package stays installed in the UI with the error badge, sources unavailable |

## E2. Trust

| # | Case | Expected |
|---|---|---|
| E2.1 | Repo key matches package key | trusted, loads |
| E2.2 | No repo key, but `trusted_ext:<pkg>` row has same version code and key | trusted |
| E2.3 | Same pkg, newer version code, same key but trusted row for older version | trusted only if the row version code is updated; the rule in B6 stores exact version code, so an update requires the repo key to match (matches Aniyomi) |
| E2.4 | `trust()` called twice | second call replaces the first row for that pkg (no duplicates) |
| E2.5 | `revoke_all()` | deletes every `trusted_ext:*` row; repo-key trust stays |
| E2.6 | Repo removed | packages from that repo move to untrusted on next load |

## E3. Repository fetch

| # | Case | Expected |
|---|---|---|
| E3.1 | One repo 404 | empty list for that repo, other repos unaffected, error logged, no toast in background refresh |
| E3.2 | Foreground refresh error (`findAvailableExtensions`) | toast key `extension_api_error` (Aniyomi `MR.strings.extension_api_error`) |
| E3.3 | Malformed JSON | repo treated as failed (empty list) |
| E3.4 | `sources` null | `sources = []` (Kotlin `sources?.map(...).orEmpty()`) |
| E3.5 | `sources[].id` as JSON number larger than 2^53 | must be string in real indexes; a number is accepted but parsed exactly by serde as `i64` (test with 4222017068256633289 both forms) |
| E3.6 | Icon 404 | icon placeholder, no error |
| E3.7 | `last_ext_check` within 24 h | background check returns `None` (no network); a foreground refresh always fetches |
| E3.8 | Live index has 3 entries (observed 2026-10-08) | UI shows 3 anime entries; manga list empty is correct, not a bug |

## E4. Install

| # | Case | Expected |
|---|---|---|
| E4.1 | Second install of same pkg while first runs | first cancelled, temp file removed (Aniyomi `downloadAndInstall` deletes old download) |
| E4.2 | Network drop mid-download | `Error`, `.part` removed, no partial `component.wasm` |
| E4.3 | Disk full on rename | `Error`, old version still installed |
| E4.4 | Cancel during `Installing` | not cancellable after rename started; cancel before rename only |
| E4.5 | Two installs queued | serial by the semaphore, order FIFO |
| E4.6 | App quits mid-install | on start, `.tmp/` is cleared, no install resumes |
| E4.7 | Update of an extension that is currently in use | in-flight requests keep the old `Arc` instance; registry swaps on success only |

## E5. Source manager and stubs

| # | Case | Expected |
|---|---|---|
| E5.1 | Library entry whose source is uninstalled | stub shown, reader disabled, message `source_not_installed` |
| E5.2 | Stub row upsert when name changes | row updated; downloaded folder renamed (Aniyomi `downloadManager.renameSource`) |
| E5.3 | Source id collision (two sources with same name and lang but different version) | id differs by `versionId`; no collision expected. Unit test with `versionId` 1 vs 2 |
| E5.4 | Local source id 0 | always present |
| E5.5 | `get_or_stub` racing with registry reload | returns stub then live on next call; no panic (Kotlin uses ConcurrentHashMap) |

## E6. Backup and id resolution

| # | Case | Expected |
|---|---|---|
| E6.1 | Aniyomi backup source id matches a built-in Rust source id | entry resolves; test vector from B9 |
| E6.2 | Backup id matches nothing installed | stub; entry retains title and url |
| E6.3 | Source renamed upstream (name change) | id changes; stub keeps the old id; user must reinstall; document this in the migration UI |
| E6.4 | Backup from Tachiyomi (manga) restored in anime-only build | manga entries become stubs |

## E7. Declarative scrapers

| # | Case | Expected |
|---|---|---|
| E7.1 | Selector matches nothing for `title` | `title` = `""` (Kotlin `selectText` returns default null, then `SManga.title` lateinit would crash; Rust returns `MissingSelector`, not empty) |
| E7.2 | `selectInt` on "abc" | `Parse` error (Kotlin `toInt()` throws NumberFormatException) |
| E7.3 | `attr` missing on element | empty string (Jsoup `attr` returns empty) |
| E7.4 | Relative href | resolved against page URL (`Response.asJsoup` uses `request.url`) |

## E8. WASM sandbox

| # | Case | Expected |
|---|---|---|
| E8.1 | Infinite loop | epoch deadline 5 s then trap, `Error(Extension("timeout"))` |
| E8.2 | Memory bomb | trap at 256 MiB |
| E8.3 | Undeclared host fetch | `network` error with `not in allowed_hosts` |
| E8.4 | Panic inside guest | trap, `Extension("panic")` |
| E8.5 | Response of 20 MiB | rejected at 16 MiB cap, `Parse("response too large")` |

## E9. Anime

| # | Case | Expected |
|---|---|---|
| E9.1 | Hoster list empty | `Error` shown as "no hosters" (message key to be defined in area:51-i18n) |
| E9.2 | Lazy hoster selected | `videos(hoster)` called once; result stored in `Hoster.videos` |
| E9.3 | `NO_HOSTER_LIST` | treated as flat list |
| E9.4 | `resolve_video` returns None | video skipped, next video tried |
| E9.5 | Timestamp with end < start | kept as-is, UI clamps to 0 length |
| E9.6 | Subtitle track URL 404 | libmpv logs, playback continues |

## E10. Races

- Registry reload during install: install writes into `<pkg>/` only after success; the watcher then reloads. Debounce 300 ms prevents double reload.
- Two repo refreshes at once: the second waits on the first (`tokio::sync::Mutex`), prevents duplicate writes of `last_ext_check`.
- Source instance used after its package is removed: `Arc` keeps it alive until the in-flight call ends; the source table is swapped out.

## E11. Upstream tests to port

Upstream repo: `/home/blackbox/aniyomi-src`. No `*Test.kt` file exists in the scope list (`areas.py files` prints none). Verified by search: no test directory under `source-api/src` or in the scope paths. Therefore the test list is new, built from the observed rules above.

## E12. New tests to write (Rust)

1. `source_id_matches_aniyomi`: golden values must be produced by running the Kotlin `generateId` derivation once (not yet done; the Kotlin is the reference). Candidate vector: `("Google Drive","all",1)`; the observed index shows `4222017068256633289` for that source, which matches only if that extension uses default `versionId` and that name. The implementer must confirm before relying on it. Vector 2: non-ASCII name `"Ünïcode Örg"`. Vector 3: `versionId` 2 gives a different id.
2. `genres_split`: `"Action, , Drama, Action"` -> `["Action","Drama"]`.
3. `triState_values`: 0, 1, 2 constants.
4. `filter_equality`: name and state only.
5. `manifest_api_range`: `1.4` accepted, `1.5` accepted, `1.6` rejected (mirror of LIB range rule, adapted).
6. `selection_shared_vs_private`: table test of E1.1 and E1.2.
7. `install_checksum_mismatch_leaves_no_file`.
8. `install_downgrade_refused`.
9. `repo_index_string_ids`: parse the observed three-entry index (fixture file `crates/kb-ext/tests/fixtures/aniyomi-index.min.json`, copied from the observed JSON).
10. `repo_one_failure_isolated`: two repos, one returns 500.
11. `update_flags`: `has_update` when available `version_code` higher or `api` higher; `is_obsolete` when not in any repo.
12. `throttle_one_day`: with `last_check` 23 h ago, background check returns `None`.
13. `stub_roundtrip`: unknown id -> stub -> installed -> live, same id.
14. `wasm_timeout_traps`, `wasm_memory_cap`, `wasm_undeclared_host`.
15. `scraper_missing_selector_error`, `scraper_int_parse_error`.
16. `hoster_no_list_flat`.

## E13. Fixtures

- `aniyomi-index.min.json` (3 anime entries, observed 2026-10-08).
- A WASM test component `kb-ext-wasm/tests/fixtures/echo.wasm` built from a small Rust crate that implements `manga-source` returning fixed pages. Build is part of CI.
- A repo fixture directory with `index.min.json`, `apk/`, `icon/` for offline tests.
- A keypair fixture for trust tests; test keys only, never production keys.

## E14. Manual checks before release

- Load the 3 observed anime entries through the scraper path is not possible (they are APKs). Expected: the UI shows them as "not supported: APK extension" (`Error(Unsupported format)`), not a crash. This is the honest migration state.
- Build Kurobayomi against an Aniyomi backup file from a real device and confirm stub count equals entries whose source is not built in.
