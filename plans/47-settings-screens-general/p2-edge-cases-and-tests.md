# 47-settings-screens-general — part 2 edge cases, failure modes, tests

Upstream tests: **none**. Searching `aniyomi-src` for `*Test*.kt` that reference `BackupOptions`, `RestoreOptions`, the anime/manga repo screen models, `CreateMangaExtensionRepo`, `LocaleHelper` or `BackupFileValidator` returns nothing. Every test below is new.

---

## 1. Edge cases by file

### 1.1 `SettingsScreen.kt`
- E1. `destination` = unknown integer (e.g. 7): falls to `else` (Main / Appearance). Rust: `SettingsDestination::from_i32(7) -> None` -> default page.
- E2. Two-pane toggles while a sub-page is open: the detail pane's page must survive (keep the page stack when width changes). Kotlin recreates the navigator on recomposition; Rust must not reset.
- E3. `canPop` false on the inner navigator with destination set: back goes to the outer navigator (closes settings). Test in section 4.
- E4. Width exactly 720 px: two-pane (>=). Width 719: narrow.
- E5. Narrow -> wide while on Restore page: Restore stays open in the right pane; the left pane shows Main.

### 1.2 `AppLanguageScreen.kt`
- E6. Locale tag with script or region (`zh-Hans`, `nb-NO`): display name via ICU; when unknown to ICU, fall back to the tag string (not empty). Kotlin `Locale.forLanguageTag` returns `und` display for unknown tags; Rust must not show "undefined".
- E7. Legacy `in` tag: ICU maps to Indonesian; show "Indonesian" (matches Java behaviour for `in`). Test with fixture.
- E8. `getLocalizedDisplayName` returns empty when displayName empty: such tags are dropped from the list (Kotlin `if (displayName.isNotEmpty())`).
- E9. Current tag not in list (e.g. stored `fr-CA` but catalog has `fr`): no row checked. Proposal: map to nearest parent (`fr`) when selecting; for display, check nothing and do not crash.
- E10. Duplicate tags in catalog: dedupe.
- E11. Sort stability: rows with equal native names keep tag order (`sort_by` is stable in Rust).
- E12. Applying the same language twice: no toast on no-op (proposal; Kotlin does a no-op effect).

### 1.3 Anime extension stores
- E13. Add with empty string: button disabled (Kotlin `state.text.isNotEmpty()`); Enter must not submit.
- E14. Add duplicate exact string: disabled. Duplicate after trailing slash difference (`https://x/index.min.json` vs `https://x/index.min.json/`): NOT detected (exact string). Documented; proposal: normalise with `url` crate before the duplicate check and also before insert. Test T-EC-14.
- E15. Add a legacy URL (`https://x/repo.json`, not ending `/index.min.json`): fetch returns JSON `{` or `[` -> legacy branch. For `[` without `/index.min.json` suffix: `IllegalArgumentException("Provided legacy store url is not valid")` -> dialog shows that exact English text. Keep text (it is the raw Kotlin message).
- E16. Network timeout: `throwable.message` = `"timeout"`-like text from OkHttp/reqwest; shown raw. Proposal: wrap with user text plus detail: "Could not fetch the store: <detail>". (Behaviour change: flagged.)
- E17. Deep link while Loading: dropped in Kotlin. Rust: queued (test T-RACE-01).
- E18. Deep link for a store already present: Confirm shows the "already exists" helper and disables Add. Test.
- E19. Delete while the store is being refreshed: DB row removed, refresh upsert re-inserts it (race). Proposal: refresh task checks row existence before upsert and skips missing rows. Test T-RACE-03.
- E20. Two adds in flight (double click before `processing` shows): Kotlin sets `processing=true` on first call; second click is disabled. Rust: disable on first click synchronously (before spawning). Test.
- E21. Removing the last store: list shows empty state, FAB still visible.
- E22. Discord link present but empty string: Kotlin shows button (`!= null`); openInBrowser with empty -> no-op. Rust: treat empty as absent? Parity says show. Decision: show only when non-empty (small deviation, flagged; test T-EC-22).
- E23. Website empty string (`contactWebsite NOT NULL` allows ""): globe button still shown; opening does nothing. Proposal: disable button when empty.
- E24. Open-in-browser failure (no handler): the failure path of Kotlin `openInBrowser` is owned by area:36-ui-shell-navigation-security-webview. Rust: a `gtk::UriLauncher` error shows toast "Could not open the link".
- E25. Name very long: ellipsize.

### 1.4 Manga extension repos
- E26. Input `https://example.org/index.min.json/` (trailing slash): `toHttpUrlOrNull` keeps trailing slash; regex requires `/index.min.json$` -> InvalidUrl. Test T-EC-26.
- E27. Input with uppercase scheme `HTTPS://x/index.min.json`: OkHttp normalises scheme to lowercase -> accepted. Rust `url` crate also lowercases scheme. Test.
- E28. Input `https://x/index.min.json?x=1`: regex fails (query) -> InvalidUrl. Keep.
- E29. Input `http://x/index.min.json`: regex needs `https://` -> InvalidUrl. Keep (security: HTTPS only for manga).
- E30. Host unreachable: `fetchRepoDetails` returns null -> InvalidUrl -> toast "Invalid repo URL". Proposal: distinguish network error (new key "Could not reach the repo") behind a flag; parity default is the Kotlin message.
- E31. Repo with same fingerprint but different URL: Conflict dialog; Replace -> old row deleted and new inserted (area:18). Cancel -> nothing changes. Test T-EC-31.
- E32. Repo with same URL already present: `RepoAlreadyExists` toast. The add dialog's pre-check compares index URL to base URLs, so it does not catch this; the domain does. Test T-EC-32.
- E33. Create returns `Error` (DB write failure): silent in Kotlin. Rust: toast "Could not add the repo" (proposal; parity deviation, flagged).
- E34. Deep link `Confirm` for an already-present repo: Kotlin shows the confirm dialog anyway; Add -> RepoAlreadyExists toast. Keep parity; test.
- E35. Event delivered when no page is listening (page closed mid-add): result dropped silently. Acceptable; test that the DB still has the row.
- E36. Fingerprint `UNIQUE` on `signing_key_fingerprint`: two repos with the same empty fingerprint? Empty fingerprint string is allowed by `NOT NULL` and would collide with another empty one. Proposal: reject empty fingerprint in fetch (area:18), test T-EC-36 at the page level (treated as invalid).

### 1.5 Create backup
- E37. Only "Extensions" checked: Create disabled (`canCreate` ignores extensions). Test T-EC-37.
- E38. "Library entries" off with "Chapters" previously on: chapters disabled but still true in the options. Create proceeds with chapters=true in the bit array; area:21 must ignore chapters when libraryEntries=false. Test that the array is passed unchanged (T-EC-38), and the behaviour in area:21 is documented there.
- E39. Picker cancelled: no state change, no toast.
- E40. Picker returns a URI without write permission (Android); not applicable on Linux. Rust: check writable parent dir before enqueue; toast "Could not write to that location" if not.
- E41. Second Create click while the first picker is open (Android: picker modal). Rust: disable Create while the dialog is open.
- E42. Filename: `BackupCreator.getFilename()` uses `Locale.ENGLISH` and local time zone: test that the Rust version uses local time and the fixed `yyyy-MM-dd_HH-mm` pattern.
- E43. Backup name collision (same minute): the picker asks to replace; no code path. Accept.
- E44. Busy job check: manual job running -> toast, no dialog. Test T-EC-44.

### 1.6 Restore backup
- E45. File is not a backup (random bytes): decoder throws; `InvalidRestore`; Full error text. Test with fixture `random.bin`.
- E46. Truncated gzip: `EOFException` wrapped. Test fixture `truncated.tachibk`.
- E47. Gzip with protobuf that is not a `Backup` message: decode error or empty message. Empty message -> valid, zero sources, zero trackers -> options enabled, restore would do nothing. Proposal: reject `Backup` with no content as invalid (test T-EC-47).
- E48. All sources installed, all trackers logged in: no error card (clean path). Fixture `complete.tachibk`.
- E49. Missing sources only, numeric id names: display `getOrStub(id).toString()`; with non-numeric names display raw name. Fixture `missing-numeric.tachibk`.
- E50. Same missing source in both manga and anime lists with the same name: listed twice (once per list). Kotlin behaviour; test T-EC-50.
- E51. Tracker sync id unknown to the app: skipped (mapNotNull). Test.
- E52. Restore with every option unchecked: Restore disabled. Test.
- E53. Restore started while another restore runs: KEEP drops silently in Kotlin. Rust: toast (T-EC-53).
- E54. File deleted between validation and restore: job fails in area:22; page already closed. Log only.
- E55. Very large backup (100k entries): validation holds whole file in memory in Kotlin. Rust: stream the decode for validation (proposal; area:22 decides). Test fixture size limit note.
- E56. Validation on worker: UI shows Validating; user can close page before validation completes: cancel the task (drop the receiver). Test.

### 1.7 Storage
- E57. No external storage mounted: empty list (Kotlin); Rust: the data-dir mount is always present (unless statvfs fails).
- E58. `statvfs` error: no bar, text "Storage size unavailable".
- E59. `total == 0` (pseudo fs): no bar. Test with fixture stat.
- E60. Available > total (rare with reserved blocks misreporting): clamp used fraction at 0. Test.
- E61. Path with spaces or unicode: display the raw path. Test with fixture mount path.
- E62. Mount point listed twice (bind mount): dedupe by `st_dev`. Test with mountinfo fixture.
- E63. Stat hang on unresponsive FUSE: timeout 2 s. Test with a fake statvfs that sleeps.
- E64. Formatting zero bytes: "0 B". Test table in section 3.

### 1.8 Debug info / worker info / schema
- E65. Job with `nextScheduleTimeMillis` in the past: shown as-is (Kotlin), no "overdue" marker.
- E66. Job with 0 tags: "Tags:" line followed by nothing. Test: exact text.
- E67. Many jobs (>200): the dump grows; Rust: no pagination (Kotlin none). Proposal: cap at 500 entries with "... N more" line (flagged, test).
- E68. Clock change: `chrono::Local` offset recomputed on each render. Fine.
- E69. Build time missing (`KUROBAYOMI_BUILD_TIME` unset in a dev build): show "unknown" (Kotlin shows raw string on parse failure). Test.
- E70. `pkg-config` missing for webkitgtk: build still succeeds, row shows "not built with WebKitGTK". Test with env override.
- E71. Schema page with a huge proto: scroll only. No edge.

### 1.9 Shared
- E72. Copy with blank content: no-op (Kotlin `isBlank`), no toast. Test.
- E73. Toast while another toast is visible: replace. Test.
- E74. Truncate center at 50: content of 50 chars or fewer unchanged; longer keeps first and last parts with an ellipsis. Exact algorithm: area:13 `truncateCenter`. Test with 49, 50, 51 chars.
- E75. Clipboard unavailable (no display, e.g. headless test): no crash; log.

---

## 2. Race conditions

| ID | Where | Race | Kotlin behaviour | Rust behaviour |
|---|---|---|---|---|
| RACE-01 | Anime and manga deep link vs first DB emission | Deep link applied while state is Loading | Dropped silently (`updateSuccessState` / `showDialog` no-op) | Queued in `Option<PendingDeepLink>` and applied on first `Loaded` |
| RACE-02 | Manga model `init` emission | DB emission replaces `Success` and clears open dialog | Dialog closed | Keep dialog unless its target row was deleted |
| RACE-03 | Anime refresh vs delete | `refreshAll` upserts a store the user just deleted | Store reappears | Refresh checks existence before upsert and skips missing rows |
| RACE-04 | Double-click Add | Two `createRepo` calls | Second call also sets processing; both run; second fails with already-exists (anime: "already exists" text) | Button disabled synchronously on first click; second click ignored |
| RACE-05 | Restore start vs running restore | KEEP drops second job | silent | toast `backup_in_progress`, no enqueue |
| RACE-06 | Create busy check vs start | Check passes, another job starts before enqueue | KEEP drops silently | Registry lock held across check and enqueue |
| RACE-07 | Validation vs page close | User closes restore page during validation | Synchronous, no race | Cancel validation task on page drop; ignore late results |
| RACE-08 | Page closed while add in flight | Result has no listener | dropped | DB still updated; no UI update needed |
| RACE-09 | Storage refresh vs unmount | Root disappears between mountinfo read and statvfs | `-1` values | statvfs error -> "unavailable" caption |
| RACE-10 | Language change vs restart | User picks language, then picks another before restart | n/a (applies immediately) | Last write wins in settings table; restart applies the last value |

---

## 3. Numeric and text fixtures (expected outputs)

### 3.1 Size formatting (Rust `fmt_size::format`, SI, proposal)
| Input bytes | Expected |
|---|---|
| 0 | `0 B` |
| 999 | `999 B` |
| 1000 | `1.0 kB` |
| 1500 | `1.5 kB` |
| 999_999 | `1.0 MB` (999.999 kB rounds to 1000 kB, which rolls over to the next unit) |
| 12_300_000_000 | `12 GB` (>= 10 units: integer) |
| 12_345_678_901 | `12 GB` |
| 64_000_000_000 | `64 GB` |
| 9_900_000 | `9.9 MB` |

Rule (proposal): value = bytes / 1000^k (k chosen so value < 1000); if value < 10 print one decimal (round half even, trim trailing `.0`? No: keep `.0`, e.g. `1.0 kB`); else print integer rounded half up; if the printed integer reaches 1000, move to the next unit and reprint. Labels `B`, `kB`, `MB`, `GB`, `TB`. Note `kB` lowercase k matches Android's SI label.

### 3.2 Used fraction
| total | available | expected |
|---|---|---|
| 100 | 25 | 0.75 |
| 100 | 100 | 0.0 |
| 100 | 0 | 1.0 |
| 100 | 150 | 0.0 (clamped) |
| 0 | 0 | None (no bar) |
| -1 | -1 | None (no bar) |
| 100 | -1 | None (no bar) |

Kotlin for comparison: total=-1, available=-1 gives `0.0`, total=-1, available=50 gives `51.0`. Parity is not desired; these are bugs.

### 3.3 Missing list sorting
Input manga sources (names): `["Zeta", "alpha", "Alpha", "1234"]` where `"1234"` maps to a stub named "Stub(1234)" (stub display name from area:17). Expected: lowercase-insensitive? Kotlin `sorted()` on String is code-point order: `["Alpha", "Stub(1234)", "Zeta", "alpha"]` (uppercase first). Rust must use the same code-point order (`str::cmp`), not locale order. Test.

### 3.4 Repo URL check (manga, area:18 regex, reproduced)
| Input | Expected |
|---|---|
| `https://example.org/index.min.json` | accepted, base `https://example.org` |
| `https://example.org/a/b/index.min.json` | accepted, base `https://example.org/a/b` |
| `https://example.org/index.min.json/` | rejected |
| `http://example.org/index.min.json` | rejected |
| `https://example.org/repo.json` | rejected |
| `HTTPS://Example.org/index.min.json` | accepted, base `https://example.org` (scheme and host lowercased) |
| `https://example.org/index.min.json?x=1` | rejected |
| `not a url` | rejected |
| `` (empty) | rejected |

### 3.5 Worker dump text
Input one job: id `1a2b3c`, tags `["LibraryUpdate"]`, state `SUCCEEDED`, no next run, attempts 0. Expected dump (per Kotlin format):
```
Id: 1a2b3c
Tags:
 - LibraryUpdate
State: SUCCEEDED

```
(with the trailing blank line). Enqueued job with next run `2026-10-08 14:00` and attempts 2: adds `Next scheduled run: 10/8/26 2:00 PM` (date/time per `app_date_format` = "" en-US) and `Attempt #3`.

Empty section: `-` + newline.

### 3.6 Backup filename
Input time `2026-10-08 09:05:07` local: `"<applicationId>_2026-10-08_09-05.tachibk"` (seconds dropped).

### 3.7 Language list order
Catalog `["en", "de", "ar", "ja", "zh-CN"]`, system tag `""`. Native names: `English`, `Deutsch`, `العربية`, `日本語`, `中文（简体）`. Expected order: `Default`, then sorted by native name by code point: `Deutsch`, `English`, `العربية`, `中文（简体）`, `日本語`. Test asserts this order (native-name code-point order, documented as the rule).

---

## 4. Test plan (new tests to write)

Pure unit tests (`cargo test`, no GTK):

| ID | Module | Test |
|---|---|---|
| T-UT-01 | `backup/options.rs` | `to_array`/`from_array` round trip for all 4096 combinations; index order equals p2-data-and-config §10 |
| T-UT-02 | `backup/options.rs` | `can_create` truth table over the six contributing fields, with extensions/private toggled both ways |
| T-UT-03 | `backup/options.rs` | `RestoreOptions::can_restore` true iff any of 7; defaults match table |
| T-UT-04 | `backup/options.rs` | `enabled` predicates: chapters/tracking/history/readEntries follow libraryEntries; privateSettings follows appSettings||sourceSettings |
| T-UT-05 | `backup/validator.rs` | sources: manga names sorted, dedupe, numeric ids mapped via stub; anime appended after manga, not re-sorted |
| T-UT-06 | `backup/validator.rs` | trackers: unknown ids skipped; logged-in skipped; output sorted and deduped |
| T-UT-07 | `backup/validator.rs` | duplicate names across manga and anime both appear |
| T-UT-08 | `backup/restore.rs` reducer | states: Validating -> Invalid | Missing | Valid; Restore enabled matrix |
| T-UT-09 | `backup/restore.rs` error text | invalid text contains headers and full chain; missing text has list formatting `- A\n- B` |
| T-UT-10 | `repos/manga.rs` reducer | Loaded keeps open dialog (RACE-02) |
| T-UT-11 | `repos/manga.rs` reducer | Create result mapping: each `MangaCreateOutcome` to message/dialog |
| T-UT-12 | `repos/anime.rs` reducer | deep link queued while Loading and applied on Loaded (RACE-01) |
| T-UT-13 | `repos/anime.rs` reducer | Add: processing set synchronously; success dismisses; failure stores text (RACE-04) |
| T-UT-14 | `repos/anime.rs` | `already_exists` uses exact string match (E14 documented behaviour) |
| T-UT-15 | `repos/manga.rs` | URL check table §3.4 |
| T-UT-16 | `storage.rs` | `storage_roots` from fixture mountinfo: filters fs types, dedupes by dev |
| T-UT-17 | `storage.rs` | `used_fraction` table §3.2 |
| T-UT-18 | `util/fmt_size.rs` | table §3.1 |
| T-UT-19 | `jobs/registry.rs` | Keep policy: second enqueue of same name returns first id and does not run (RACE-05/06) |
| T-UT-20 | `debug/workers.rs` | dump text §3.5 |
| T-UT-21 | `app_language.rs` | list order §3.7; default row first; duplicate tags removed; empty display dropped (E8, E10) |
| T-UT-22 | `util/clipboard.rs` | `truncate_center` at 49/50/51 chars (delegates to area:13 function; test the wrapper) (E74) |
| T-UT-23 | `shell.rs` | `two_pane(719)=false`, `two_pane(720)=true` (E4) |
| T-UT-24 | `shell.rs` | back: inner page pops before closing (E3) |
| T-UT-25 | `debug/info.rs` | build time fallback "unknown" (E69); webkit row fallback (E70) |
| T-UT-26 | `schema.rs` | proto field numbers match `backup.proto` used by the decoder (pitfall 12) |

Integration tests (`tests/settings_*.rs`, with fake services, no display needed for the reducers; GTK smoke tests behind feature `gtk-tests` and `xvfb-run` in CI):

| ID | Test |
|---|---|
| T-IT-01 | Anime add against a local HTTP fixture server: JSON index, gzip index, legacy `[` array with `/index.min.json`, invalid body -> error text |
| T-IT-02 | Manga add against fixture: valid `repo.json`, malformed JSON -> InvalidUrl, duplicate fingerprint -> Conflict -> Replace |
| T-IT-03 | Restore validate fixtures: `complete.tachibk`, `missing-numeric.tachibk`, `random.bin`, `truncated.tachibk` (E45, E46, E48, E49) |
| T-IT-04 | Create backup end-to-end to a temp dir (with a stub backup writer from area:21): file name pattern, options array passed through |
| T-IT-05 | Refresh while delete (RACE-03) with a fake store that delays the fetch |

GTK smoke tests (manual or xvfb):
- GS-01 Settings shell opens each destination; back closes shell.
- GS-02 Anime page empty state appears with zero stores; Add dialog opens; duplicate disables Add.
- GS-03 Manga repo Add with invalid URL shows toast.
- GS-04 Create backup: Create disabled with only Extensions checked.
- GS-05 Restore page with invalid file shows error card and no options card.
- GS-06 Storage card shows one bar per mount in fixture mountinfo (real `/` in CI).
- GS-07 Language page shows Default first; selecting shows restart toast.

---

## 5. Fixtures to create

Directory `tests/fixtures/settings/` (not in scope for this area's writes; the implementer creates):

| File | Content | Used by |
|---|---|---|
| `backup/complete.tachibk` | gzip+protobuf `Backup`, one manga and one anime source, both installed in test catalog, trackers logged in | T-IT-03, E48 |
| `backup/missing-numeric.tachibk` | sources with numeric ids not in catalog, one tracker not logged in | T-IT-03, E49 |
| `backup/empty-message.tachibk` | gzip of zero-length protobuf | E47 |
| `backup/truncated.tachibk` | first 100 bytes of a valid file | E46 |
| `backup/random.bin` | 1 KiB of seeded random bytes | E45 |
| `index/anime-index.json` | JSON store with extensionList | T-IT-01 |
| `index/anime-index.json.gz` | gzip of the same | T-IT-01 |
| `index/legacy-array.json` | JSON array of legacy extension entries (top-level `[`), URL ends in `/index.min.json` | T-IT-01, E15 |
| `index/manga-repo.json` | `{"meta": {name, shortName, website, signingKeyFingerprint}}` | T-IT-02 |
| `index/manga-repo-conflict.json` | same fingerprint as above, different base | T-IT-02, E31 |
| `proc/mountinfo-sample` | 8 lines: `/`, `/home`, `/boot` (vfat), `/run/media/x/USB` (exfat), `tmpfs`, `overlay`, one bind mount duplicate | T-UT-16 |
| `stat/statvfs-table.txt` | rows of `(path, f_bavail, f_blocks, f_frsize)` for the fake statvfs | T-UT-17, GS-06 |

---

## 6. Failure modes and expected behaviour

| Failure | Where | Expected |
|---|---|---|
| Index decode error | anime add | dialog error text = Kotlin message (raw), Add re-enabled |
| Repo details JSON error | manga add | toast "Invalid repo URL" |
| DB write error on insert | manga add | toast "Could not add the repo" (deviation, flagged) |
| DB write error on delete | either | log; list unchanged (no dialog) |
| Backup decode error | restore validate | invalid card, Restore disabled |
| Backup job failure | create job (area:21) | notification (area:22/21); page already closed |
| Restore job failure | area:22 | notification |
| `statvfs` failure | storage | caption "Storage size unavailable" |
| Clipboard unavailable | copy | log, no toast |
| No portal / no chooser | create or restore | toast "No file chooser available" |
| Language catalog missing | language page | default row only |
| Job registry poisoned lock | worker info | show "-" for all sections, log |

---

## 7. Coverage checklist

- Every row in `p2-behavior.md` section 3 (anime) has at least one test (T-UT-12/13/14, GS-02).
- Every row in section 4 (manga) has at least one test (T-UT-10/11/15, T-IT-02, GS-03).
- Create and restore rules (sections 5, 6): T-UT-01..09, T-IT-03/04, GS-04/05.
- Storage (section 7): T-UT-16..18, GS-06.
- Debug (section 8, 9, 10): T-UT-20, T-UT-25, T-UT-26.
- Language (section 2): T-UT-21, GS-07.
- Shell (section 1): T-UT-23/24, GS-01.
