# Area 09 — edge cases, failure modes, quirks and tests

Companion to `p1-behavior.md` (algorithms, with file:line citations) and `p1-tasks.md` (task ids). IDs: `Q-…` = a quirk present in the Kotlin code that the Rust port must either preserve deliberately or fix as written in the Proposal; `E-…` = an edge case the Rust code must handle; `TC-…` = a test to write.

## 1. Quirk register (OBSERVED in Kotlin)

| ID | Where | Behaviour | Port decision | Task |
|---|---|---|---|---|
| Q-PIN-1 | `source/*/model/Pin.kt:19` (`contains`) | `pin.code and code == pin.code` → `Pins.unpinned.contains(Pin.Unpinned)` is `true` | Preserve operator; add `is_pinned()` for real checks | T-09a-003 |
| Q-PIN-2 | Pin.kt:23 (`minus` = XOR) | Removing an absent flag adds it | Fix: `without` = AND-NOT | T-09a-003 |
| Q-PIN-3 | Pin.kt:36-38 (`unaryMinus` ORs) | Builder "minus" adds | Not ported (no caller) | T-09a-003 |
| Q-BR-1 | `GetRemoteManga.kt:11-17`, `GetRemoteAnime.kt:11-17` | Sentinel strings share the search-query channel; typing the sentinel text searches nothing and shows popular | Fix: `BrowseQuery` enum from the UI | T-09a-006 |
| Q-TRK-1 | `MangaTrackRepositoryImpl.kt:14-16`; `GetMangaTracks.kt:13-20` | `awaitOne(id)` queries `_id` (row id), not the manga id | Rename to `by_row_id`; document | T-09a-011 |
| Q-TRK-2 | `manga_sync.sq:18`, `anime_sync.sq:18` (`UNIQUE … ON CONFLICT REPLACE`) | Re-inserting a (entry, tracker) pair replaces the row and changes `_id` | Proposal: upsert keeping `_id` | T-09a-012 |
| Q-TRK-3 | `GetMangaTracks.kt:22-29` | List read errors return `emptyList()` silently; the UI cannot tell "no tracks" from "DB error" | Keep log-and-default for parity; surface a `last_error` flag in the Rust service (Proposal) | T-09a-059 |
| Q-DL-1 | `FilterChaptersForDownload.kt:41-43` | Unread-only compares chapter **numbers**; a re-numbered already-read chapter is suppressed | Keep (parity), documented | T-09a-017 |
| Q-DL-2 | `FilterChaptersForDownload.kt:55-56` (`.map { it.toLong() }`) | Non-numeric category strings throw | Proposal: skip non-numeric | T-09a-018 |
| Q-REPO-1 | `CreateMangaExtensionRepo.kt:27-40` | Only `SaveExtensionRepoException` is caught; other exceptions escape `await` | Proposal: map all DB errors to `Error` | T-09a-026 |
| Q-REPO-2 | `UpdateMangaExtensionRepo.kt:15-19` | `coroutineScope { async … }` cancels siblings when one repo's upsert throws | Fix with JoinSet + per-task result | T-09a-028 |
| Q-REPO-3 | `extension_repos.sq:31-41` | Upsert with a fingerprint that another row already owns raises a UNIQUE error → `SaveExtensionRepoException` during refresh | Proposal: treat as "fingerprint changed", skip | T-09a-028 |
| Q-STORE-1 | `AnimeExtensionStoreService.kt:36-39`, `AnimeExtensionStoreRepositoryImpl.kt:54-67` | Legacy `index.min.json` input is stored under the rewritten `repo.json` key → possible duplicate row | Proposal: key by user URL, store resolved URL separately | T-09a-031 |
| Q-CB-1 | `CustomButton.kt:12-25` | `$id` replacement is substring-based (`$identity` → `<id>entity`) | Keep for parity; document | T-09a-036 |
| Q-CB-2 | `CreateCustomButton.kt:17-18` | No mutex: concurrent creates can reuse the same `max+1` | Take the reorder mutex | T-09a-038 |
| Q-CB-3 | `UpdateCustomButton.kt:14-15` | Single-update errors are not logged | Log at error level | T-09a-038 |
| Q-CB-4 | `ReorderCustomButton.kt:27-28` | Out-of-range `newIndex` → `IndexOutOfBoundsException` caught as `InternalError` | Validate and return `InvalidIndex` (Proposal) | T-09a-039 |
| Q-CB-5 | `ToggleFavoriteCustomButton.kt:17-44` | Toggling favourite off leaves zero favourites; toggling an unknown id sets all to false and returns Success | Return Unchanged for unknown id; define primary fallback | T-09a-040 |
| Q-STO-1 | `StorageManager.kt:39-40`, `StorageManager.kt:99-103` | Manga local folder is `local`, anime is `localanime`; names are not parallel | Keep names (disk compatibility) | T-09a-048 |
| Q-STO-2 | `StorageManager.kt:31-54` | `drop(1)` skips the initial value: tree not created at start | Ensure tree at start explicitly | T-09a-048 |
| Q-STO-3 | `StorageManager.kt:56-59` | Unreachable base → `null` silently | Typed `StorageUnavailable` error | T-09a-051 |
| Q-BK-1 | `PreferenceValues.kt:3-9` | Backup flags look like bit masks but are compared as strings | Keep tags; never OR them | T-09a-053 |
| Q-BK-2 | `BackupPreferences.kt:14-17` | `backupFlags` has no caller outside the domain module | Keep key for parity; wire UI in area 21 | T-09a-053 |
| Q-REL-1 | `GetApplicationRelease.kt:68-72` | Not lexicographic: `old 1.0.0`, `new 0.9.9` → update | Fix with semver | T-09a-043 |
| Q-REL-2 | same | Extra segments in the new tag ignored (`old 1.2`, `new 1.2.1` → no update) | Fix with semver | T-09a-043 |
| Q-REL-3 | same | `newSemVer[index]` out of range when new has fewer segments → uncaught IOOBE | Fix with semver | T-09a-043 |
| Q-REL-4 | `GetApplicationRelease.kt:54-59` | Preview `toInt()` throws for dotted tags | Parse as u64, error → no update | T-09a-044 |
| Q-REL-5 | `GetApplicationRelease.kt:31` | Null release does not update `lastChecked` (retry each launch) | Keep (parity) | T-09a-042 |
| Q-SRC-1 | `StubMangaSource.kt:26-27` | `toString` falls back to id when name/lang blank | Keep `display_name()` | T-09a-005 |
| Q-SRC-2 | `Source.kt:22-27` | `key` is a property lambda, recomputed per call; `isUsedLast` makes a different key | Keep as method | T-09a-002 |

## 2. Edge cases to handle (E-…)

### 2.1 Source and browse
- **E-SRC-01** Source id `0` or negative ids from local sources: `key()` produces `"0"`; never used as "unset" sentinel.
- **E-SRC-02** `lang` empty → `visual_name` = name only (no parentheses). `lang` = `"en"` → `"Name (EN)"`.
- **E-SRC-03** Stub with blank name → `display_name()` shows the numeric id; never an empty label.
- **E-SRC-04** Source uninstalled while a browse page is loading: `load()` returns `SourceNotInstalled`; the UI shows the stub message, not a crash.
- **E-SRC-05** `get_or_stub(key)` for an unknown key with no stub row → `SourceHandle::Stub` with `lang=""`, `name=""` → `display_name()` = id.
- **E-SRC-06** `is_used_last` and the normal row of the same source appear together; keys differ (`"<id>-lastused"` vs `"<id>"`); list rendering must use `key()`.
- **E-SRC-07** Pins: an unpinned source with `Pins::PINNED` after a pin/unpin cycle must return to `UNPINNED` exactly (no residual bits). TC-PIN-04.
- **E-SRC-08** Source list order: pinned first, then used-last, then by name — the ordering rule is owned by the source-list screen (area 17); the domain only exposes flags.
- **E-SRC-09** Non-library listing with `count = 0` rows: still shown only if the source has orphaned ids (Proposal: hide zero-count, zero-orphan rows).
- **E-SRC-10** Remote page `next = None` ends paging; a `load` returning an empty page with `next = Some(n)` must not loop forever (Proposal: stop after one empty page).

### 2.2 Tracks
- **E-TRK-01** Track with `libraryId = None`: serialise as SQL NULL; reading back must yield `None`, not 0.
- **E-TRK-02** `lastChapterRead = 12.5` round-trips exactly (REAL). Rust `f64` identical.
- **E-TRK-03** Write failure in `insert` (log-and-default): the UI must still learn about the failure. Proposal: `TrackService::insert` returns `Result` and the existing swallow behaviour is the explicit wrapper used only for parity call sites. TC-TRK-05.
- **E-TRK-04** `delete` of a non-existent pair: `DELETE` affects 0 rows and is not an error. TC-TRK-03.
- **E-TRK-05** Deleting an entry cascades tracks (`ON DELETE CASCADE`); with `PRAGMA foreign_keys = ON` (area 03) this must be enabled on every connection, or stale tracks remain.
- **E-TRK-06** `insert_all` with duplicate pairs in one batch: last one wins (ON CONFLICT REPLACE) — the Proposal upsert gives the same result. TC-TRK-06.
- **E-TRK-07** `score` is a `REAL` but tracker scales differ (0–10 vs 0–100); the domain does not normalise; tracker code (area 27) owns conversion.
- **E-TRK-08** `startDate`/`finishDate` `0` means unset (observed convention in tracker code; area 27).

### 2.3 Download policy and filter
- **E-DL-01** `numberOfDownloads` outside 1..5 (manual edit of the preference file): clamp to 1..5 on read.
- **E-DL-02** `downloadSpeedLimit` negative → treat as 0 (unlimited). Proposal; Kotlin would pass a negative to the throttler.
- **E-DL-03** `removeAfterReadSlots = 4` with only 3 chapters before the current one → `getOrNull` returns null → nothing deleted. TC-DL-07.
- **E-DL-04** `removeAfterReadSlots` < -1 (e.g. -2 from a hand-edited file): Kotlin computes `index - (-2) = index + 2` (targets a *later* chapter). Proposal: treat any negative value as disabled.
- **E-DL-05** Category sets containing `UNCATEGORIZED_ID` (`"0"`): valid; `0` means the default category.
- **E-DL-06** Entry in no category: uses `[0]`. Entry in categories `[3, 5]` with excluded `{5}` → false.
- **E-DL-07** `download_new` enabled, `favorite = false` entries never download. TC-DL-02.
- **E-DL-08** New items list empty → return empty without DB calls (perf).
- **E-DL-09** Number-keyed unread-only with decimal chapter numbers (`10.5`): compare via `f32::to_bits` for exact equality; `10.5` and `10.50` are the same float.
- **E-DL-10** `Float` NaN number: `NaN >= 0.0` is false → unrecognised; never in read set; always kept when unread-only is on.
- **E-DL-11** Speed limit conversion overflow: `i32::MAX * 1024` overflows `i32`; use `i64` (Kotlin `toLong()` first). TC-DL-09.

### 2.4 Extension repos (manga)
- **E-REPO-01** Index URL with uppercase host (`HTTPS://EXAMPLE.COM/…`): normalised by `url::Url`; accepted.
- **E-REPO-02** `http://` scheme: rejected by the regex (InvalidUrl). TC-REPO-02.
- **E-REPO-03** URL with query string `…/index.min.json?x=1`: regex fails (anchored `$`) → InvalidUrl.
- **E-REPO-04** Repo server returns HTML (captive portal) with status 200: JSON decode error → `fetch_repo_details` returns None → InvalidUrl (not Error). TC-REPO-04.
- **E-REPO-05** Same repo added twice: second insert → SQLite PK violation → `RepoAlreadyExists`. TC-REPO-03.
- **E-REPO-06** Refresh: one repo's server down → skip that repo, refresh others, return Ok. TC-REPO-08 (fix for Q-REPO-2).
- **E-REPO-07** Refresh: fingerprint changed upstream → skip update silently, log warning, expose a flag `fingerprint_changed` to UI (Proposal).
- **E-REPO-08** Refresh of `NOFINGERPRINT…` repo: updates fingerprint to the real one (TOFU upgrade). TC-REPO-06.
- **E-REPO-09** Replace on DuplicateFingerprint: changes base_url of the old row; the old base URL no longer exists in the table afterwards. TC-REPO-05.
- **E-REPO-10** Delete a repo that is referenced by installed extensions (area 18): repo row goes, extensions stay; UI must warn (Proposal).

### 2.5 Extension stores (anime)
- **E-STO-01** Add with a `[`-leading body and URL not ending in `/index.min.json`: `IllegalArgumentException("Provided legacy store url is not valid")` → failure with that message.
- **E-STO-02** Add with `indexV2` set: one recursive fetch; second-level `indexV2` ignored in Rust (Proposal, E-STO-07).
- **E-STO-03** Gzip body with protobuf payload: gunzip first, then sniff. TC-STO-02.
- **E-STO-04** Store with `extension_list_url = None` and `is_legacy = false` and `extensionList == null` → NPE in Kotlin (`!!`); Rust returns `Invalid("extension list missing")`.
- **E-STO-05** Refresh: sequential; a slow store delays the rest. Proposal: per-store timeout from `kuro-net` (area 12).
- **E-STO-06** `fetch_extensions` with zero stores → empty list without network.
- **E-STO-07** Recursion guard: `indexV2` cycle → Rust depth-1 limit returns `Invalid("indexV2 chain too long")`.
- **E-STO-08** `insert_from_preference` migration with an existing row: upsert overwrites metadata with placeholders (`NO_SIGNING_KEY`). Proposal: do not overwrite a row whose `signing_key != "NO_SIGNING_KEY"`.
- **E-STO-09** Remove a store while its refresh is running: the refresh's upsert re-creates the row (race). Proposal: refresh re-checks existence before upsert.

### 2.6 Custom buttons
- **E-CB-01** Create with the table empty: sortIndex 0.
- **E-CB-02** Delete the last remaining button: compaction batch is empty → no-op Success (Kotlin calls `updatePartialCustomButtons(emptyList())`, which opens an empty transaction). Proposal: skip the transaction.
- **E-CB-03** Reorder to the same index: remove and re-add produce the same list; updates still written (cheap). Proposal: skip writes when unchanged.
- **E-CB-04** Reorder with `newIndex = n - 1`: moves to end (valid). `newIndex = n`: error (Q-CB-4).
- **E-CB-05** Toggle favourite on the only favourite: all false afterwards; the player falls back to lowest sortIndex (Proposal).
- **E-CB-06** Button content containing a literal `$id` that the user wants verbatim: cannot escape; document in the editor help text (Proposal).
- **E-CB-07** Seed row missing (user deleted id 1, then migration reruns): `INSERT OR IGNORE` re-inserts id 1 only if absent; no duplicate.
- **E-CB-08** Lua snippet with `$isPrimary` replaced by `false` for non-primary buttons: the `if false then` block is valid Lua. TC-CB-03.
- **E-CB-09** Name empty string: allowed by the DB (`TEXT NOT NULL`, empty is not NULL). Proposal: reject empty name in UI and in `CreateCustomButton` (`Invalid("name")`).

### 2.7 Storage
- **E-STO-10** Base directory deleted while the app runs: accessors create it again (`create_dir_all`) if the parent exists; if the parent is gone, `create_dir_all` creates the whole path (may be on an unmounted drive → mount point becomes a plain folder). Proposal: check `is_dir()` of the base and refuse to create when the base's *mount* is missing (`st_dev` check).
- **E-STO-11** Base directory on a read-only filesystem: `create_dir_all` fails → `DomainError::Io` (add a variant `Io(#[from] std::io::Error)`); UI shows error.
- **E-STO-12** Base path with non-UTF-8 bytes: store as `PathBuf` (not `String`); preference holds a `String` via lossy encode is forbidden — use `OsString` bytes base64 (Proposal, T-09a-047).
- **E-STO-13** Changing base to the same path: `distinct` suppresses the change (Kotlin `distinctUntilChanged`); Rust `watch` only notifies on change via `send_if_modified`.
- **E-STO-14** Concurrent `set_base` calls: serialised by a mutex in `StorageManager` (Proposal).

### 2.8 Backup preferences
- **E-BK-01** `backup_interval = 0` means disabled; reading must not treat 0 as "every 0 hours" (division or timer). TC-BK-01.
- **E-BK-02** Negative interval: treat as disabled.
- **E-BK-03** `backup_flags` with unknown tags (e.g. `"80"`): ignored on read; preserved on write? Proposal: drop unknown tags on write.
- **E-BK-04** `lastAutoBackupTimestamp` in the future (clock change): throttle comparisons must use `max(0, now - last)`.

### 2.9 Release check
- **E-REL-01** Throttle boundary: exactly 3 days after last check → allowed (`isBefore` is strict). TC-REL-05.
- **E-REL-02** `forceCheck = true` bypasses throttle and still updates `lastChecked` (Kotlin: yes, after a successful fetch).
- **E-REL-03** GitHub returns 403 (rate limit): `awaitSuccess` throws → `DomainError::Net`; check returns Err; `lastChecked` not updated (retry next launch). TC-REL-06.
- **E-REL-04** Release with no matching asset → `NoNewUpdate`, not an error (Kotlin `?: return null`). TC-REL-07.
- **E-REL-05** Mention regex on `@-user` at string start and after punctuation (`\B`): `(@alice)` linkified, `email@host` not (preceded by a word char). TC-REL-08.
- **E-REL-06** Release notes (`body`) can be `null` in GitHub JSON; Kotlin `info: String` non-null would throw. Proposal: `Option<String>` → empty string.
- **E-REL-07** Preview build tags `r2000` with commitCount `2000` → not newer (strict `>`). TC-REL-02.
- **E-REL-08** Tag `v1.0.0-rc1`: after stripping non-digits/dots → `1.0.0.1` (Kotlin) which is 4 segments. Proposal: strip pre-release suffix first, then compare; pre-release < release by semver rules.

### 2.10 Upcoming
- **E-UP-01** Status `UPCOMING=7` for manga (not defined): manga set excludes 7 (observed). A manga with status 7 is never upcoming.
- **E-UP-02** Status `UNKNOWN=0` is excluded from upcoming (observed).

### 2.11 Pins and sources (cross)
- **E-PIN-01** `Pins::from_bits(0b100)` masks to `0b00`. Non-zero bits above bit 1 are impossible in app code.

## 3. Upstream tests to port

Only one upstream test exists in the scope's neighbourhood: `domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt` (lines 17-140). The four tests below port one-to-one:

| Port ID | Kotlin name (verbatim) | Inputs | Expected | Lines |
|---|---|---|---|---|
| TC-REL-P1 | `When has update but is preview expect new update` | prefs get → 0; release `r2000`; `isPreview=true`, `commitCount=1000`, `versionName=""` | `NewUpdate(release)` | 34-59 |
| TC-REL-P2 | `When has update expect new update` | prefs get → 0; release `v2.0.0`; `isPreview=false`, `commitCount=0`, `versionName="v1.0.0"` | `NewUpdate(release)` | 61-87 |
| TC-REL-P3 | `When has no update expect no new update` | prefs get → 0; release `v1.0.0`; `isPreview=false`, `versionName="v2.0.0"` | `NoNewUpdate` | 89-113 |
| TC-REL-P4 | `When now is before three days expect no new update` | prefs get → `Instant.now().toEpochMilli()`; release `v1.0.0`; `versionName="v2.0.0"` | `NoNewUpdate` and the service is **not called** (`coVerify(exactly = 0)`) | 115-140 |

Each Kotlin test mocks `ReleaseService` and `PreferenceStore`; the Rust port uses a fake `ReleaseSource` (`async_trait` impl with a call counter) and an in-memory `kuro_prefs::Store`. The Kotlin `preference.set(any())` stub is replaced by asserting `last_checked` changed in TC-REL-P1..P3 (Proposal).

## 4. New tests to write (TC-…)

Rust `#[test]` / `#[tokio::test]` in `crates/kuro-domain/tests/`. Deterministic: inject the clock (`fn() -> SystemTime`).

### 4.1 Pins
- **TC-PIN-01** `Pins::PINNED.contains(Pin::Pinned) && contains(Pin::Actual)`.
- **TC-PIN-02** `Pins::UNPINNED.contains(Pin::Pinned) == false`.
- **TC-PIN-03** `UNPINNED.with(Pinned).with(Actual) == PINNED`.
- **TC-PIN-04** `PINNED.without(Pinned).without(Actual) == UNPINNED` (E-SRC-07).
- **TC-PIN-05** Quirk preservation: `UNPINNED.contains(Pin::Unpinned) == true` (Q-PIN-1) — documents the behaviour.
- **TC-PIN-06** `from_bits(0xFF) == PINNED` (masking).

### 4.2 Source model and stubs
- **TC-SRC-01** `visual_name` table: `("Foo","en") → "Foo (EN)"`, `("Foo","") → "Foo"`, `("Foo","pt-BR") → "Foo (PT-BR)"`.
- **TC-SRC-02** `key()` for `is_used_last` true/false.
- **TC-SRC-03** `StubSource::display_name` for blank name/lang → id string (Q-SRC-1).
- **TC-SRC-04** Every stub operation returns `SourceNotInstalled` (fixture: the manga and anime traits).
- **TC-SRC-05** `from_legacy` maps the four sentinel strings and passes any other text as `Search` (Q-BR-1 guard).

### 4.3 Tracks
- **TC-TRK-01** Insert then `by_entry` returns the track with all 13 fields equal (round-trip).
- **TC-TRK-02** `by_row_id` after insert returns it; after re-insert of the same pair with the Proposal upsert the row id is unchanged (Q-TRK-2 fix). With the Kotlin replace semantics the id changes (test marks the expected difference).
- **TC-TRK-03** Delete of an absent pair → Ok, no rows changed.
- **TC-TRK-04** `subscribe` / `watch` emits after insert and after delete (not on read).
- **TC-TRK-05** Log-and-default wrapper returns `None`/empty on a forced DB error and logs once (use `tracing-test`-style capture or a counting subscriber).
- **TC-TRK-06** `insert_all` with duplicates (same pair twice): exactly one row remains; the second values win.
- **TC-TRK-07** Group by entry: `GetTracksPerEntry` returns a map with one key per entry that has tracks; entries without tracks are absent.

### 4.4 Download filter
- **TC-DL-01** Table test of `should_download_new` covering the five steps of behavior 5.2 (all 8 combinations of included empty/excluded empty/category membership).
- **TC-DL-02** `favorite=false` → false regardless of prefs (E-DL-07).
- **TC-DL-03** `download_new=false` → `filter_new` returns empty.
- **TC-DL-04** Unread-only: existing read chapter 5 suppresses new chapter 5; unread chapter 5 does not suppress.
- **TC-DL-05** Unread-only with unrecognised (-1) read chapter does not suppress anything.
- **TC-DL-06** Unread-only with decimal chapter `10.5` read suppresses new `10.5`; `10.0` not suppressed.
- **TC-DL-07** `remove_after_read` target: slots `0` → current; `1` → previous; `4` with 3 previous → none (E-DL-03); `-1` → no-op.
- **TC-DL-08** Negative slots other than -1 are treated as disabled (E-DL-04 Proposal).
- **TC-DL-09** Speed limit: `download_speed_limit = i32::MAX` → 2_147_483_647 × 1024 as i64 without overflow.
- **TC-DL-10** Category set parse: `{"1","x","3"}` → `{1,3}` (Q-DL-2 Proposal).
- **TC-DL-11** Defaults test: every accessor default equals the table in `p1-data-and-config.md` section 4.1 (23 assertions).

### 4.5 Extension repos
- **TC-REPO-01** `validate_index_url` accepts `https://example.com/a/index.min.json` and returns base `https://example.com/a`.
- **TC-REPO-02** Rejects `http://…`, `…/index.json`, `…/index.min.json?x`.
- **TC-REPO-03** Add the same repo twice → `RepoAlreadyExists`.
- **TC-REPO-04** Server returns HTML 200 → `InvalidUrl`.
- **TC-REPO-05** Add repo whose fingerprint equals an existing repo with another base URL → `DuplicateFingerprint{old, new}`; then `replace` → old base URL gone, new present.
- **TC-REPO-06** Refresh: `NOFINGERPRINT-x` repo updates to the real fingerprint; a repo with changed fingerprint is not updated.
- **TC-REPO-07** `DeleteMangaExtensionRepo` for absent base URL → Ok.
- **TC-REPO-08** Refresh with one failing server and one OK → the OK repo is updated, the call returns Ok (Q-REPO-2 fix).
- **TC-REPO-09** `count` watch emits after insert and delete.

### 4.6 Extension stores (anime)
- **TC-STO-01** `sniff` on `[` returns `LegacyList`; on `{` with legacy shape returns `LegacyRepo`; on `{` current shape returns `CurrentJson`; on `0x0A` returns `Proto`.
- **TC-STO-02** Gzip magic: a gzipped JSON body round-trips through `gunzip_if_needed`; a plain body is returned unchanged.
- **TC-STO-03** `[` body with URL not ending `/index.min.json` → error with the exact Kotlin message.
- **TC-STO-04** `indexV2` recursion depth limit (E-STO-07).
- **TC-STO-05** `refresh_all` continues after one store fails; both upserts attempted (wiremock: one 500, one 200).
- **TC-STO-06** `fetch_extensions` returns the union of the successful stores and drops failing ones.
- **TC-STO-07** `insert_from_preference` placeholders: `badge_label == name`, `signing_key == "NO_SIGNING_KEY"`, `contact_website == index_url`, `contact_discord == None`, `is_legacy == false`, `extension_list_url == None`.
- **TC-STO-08** Remove a store: row gone; a second remove is Ok.

### 4.7 Custom buttons
- **TC-CB-01** `render` replaces `$id` and `$isPrimary` for primary and non-primary (table).
- **TC-CB-02** Quirk: `$identity` → `"<id>entity"` (Q-CB-1 documented).
- **TC-CB-03** Lua snippet with `$isPrimary` false → the `if false then ... end` block is syntactically valid (run through `mlua` in the test or a parse check; optional).
- **TC-CB-04** `create` three times → sortIndex `0,1,2`; after `delete(1)` → `0,1` (compacted).
- **TC-CB-05** `reorder(button 0 → index 2)` on `[A,B,C]` → `[B,C,A]` with sortIndex `0,1,2`.
- **TC-CB-06** `reorder` with unknown id → `Unchanged`, no writes (assert zero update calls).
- **TC-CB-07** `reorder` with `newIndex = 3` on 3 items → `InvalidIndex` (Q-CB-4 fix).
- **TC-CB-08** `toggle_favorite(B)` on `[A*,B,C]` → `[ ,B*, ]`; toggling `B` again → none favourite.
- **TC-CB-09** `toggle_favorite` unknown id → `Unchanged`.
- **TC-CB-10** Seed migration twice → exactly one row with `_id = 1` (E-CB-07).
- **TC-CB-11** Concurrent creates (two tasks) → distinct sortIndex values (Q-CB-2 fix).

### 4.8 Storage
- **TC-STO-09** `set_base(tmp)` creates `autobackup, local, localanime, downloads, mpv-config/{fonts,scripts,script-opts,shaders}` (exact names; assert with `read_dir`).
- **TC-STO-10** `open(configured)` does not create the tree (Q-STO-2 documented; test asserts absence until `ensure_tree` or an accessor).
- **TC-STO-11** Accessor with `base = None` → `StorageUnavailable`.
- **TC-STO-12** `set_base` same path → no notification (E-STO-13).
- **TC-STO-13** Non-UTF-8 base path round-trips through the preference (E-STO-12).
- **TC-STO-14** Read-only base → `Io` error from accessor (E-STO-11) (skipped in CI as root).

### 4.9 Backup prefs
- **TC-BK-01** Interval 0 → disabled (E-BK-01); reading default 12 when absent.
- **TC-BK-02** Flags default `{Categories, Chapters, History, Track}`; tags `"1","2","4","8"`.
- **TC-BK-03** `BackupFlag::from_tag("10") == Some(Settings)`; `from_tag("3") == None`.
- **TC-BK-04** Unknown tag in stored set is dropped on write (E-BK-03).

### 4.10 Release
- **TC-REL-01** `is_new_version` table: `(release, "v2.0.0", "v1.0.0") → true`; `("1.0.0","0.9.9") → false` (Q-REL-1 fix); `("1.2.1","1.2") → true` (Q-REL-2 fix: new tag with extra segment is newer); `("1.2","1.2.3") → false` (Q-REL-3 fix, no panic).
- **TC-REL-02** Preview: `("r2000", 2000) → false`; `("r2001", 2000) → true`; `("r1.2", 1000) → error → no update`.
- **TC-REL-03** Pre-release `v1.0.0-rc1` vs `v1.0.0` → `1.0.0` is newer (E-REL-08).
- **TC-REL-04** `choose_asset` for `x86_64` picks `…-linux-x86_64…`; for `aarch64` picks the aarch64 asset; none → `None`.
- **TC-REL-05** Throttle boundary: exactly 3 days → checks again (E-REL-01).
- **TC-REL-06** GitHub 403 → Err, `last_checked` unchanged (E-REL-03).
- **TC-REL-07** No matching asset → `NoNewUpdate`, `last_checked` unchanged (Q-REL-5).
- **TC-REL-08** `linkify_mentions("hi @alice and (@bob) x@y")` → `hi [@alice](https://github.com/alice) and ([@bob](https://github.com/bob)) x@y`.
- **TC-REL-09** `info: null` JSON → empty string (E-REL-06).

### 4.11 Upcoming
- **TC-UP-01** Manga statuses `[1,4]` only; anime `[1,4,7]`; `2` and `5` excluded (E-UP-01, E-UP-02).

## 5. Fixtures

| Fixture | Content | Used by |
|---|---|---|
| `fixtures/repo.json` | `{"meta":{"name":"Test Repo","shortName":null,"website":"https://example.com","signingKeyFingerprint":"abc123"}}` | TC-REPO-*, TC-REPO-06 |
| `fixtures/repo_nofingerprint.json` | same with `signingKeyFingerprint` = `NOFINGERPRINT-1` | TC-REPO-06 |
| `fixtures/store_current.json` | `NetworkAnimeExtensionStore` JSON with `extensionList` (area 19 shape) | TC-STO-01, TC-STO-06 |
| `fixtures/store_legacy_list.json` | JSON array starting with `[` (legacy extension list) | TC-STO-01, TC-STO-03 |
| `fixtures/store_legacy_repo.json` | `{ "name":…, "indexV2": "https://…/index.min.json" }` | TC-STO-04 |
| `fixtures/store_gzipped.json.gz` | gzip of `store_current.json` | TC-STO-02 |
| `fixtures/github_latest.json` | GitHub `releases/latest` sample: `tag_name`, `body` with `@alice`, `html_url`, `assets` with `kuro-1.2.3-linux-x86_64.AppImage` and `-linux-aarch64` | TC-REL-04, TC-REL-08 |
| `fixtures/github_latest_null_body.json` | `body: null` | TC-REL-09 |
| SQLite | in-memory DB from `kuro-db` migrations (areas 03/04) | all repo tests |
| HTTP | `wiremock` (dev-dependency, added by T-09a-058) | TC-REPO-*, TC-STO-*, TC-REL-* |
| Clock | injected `fn() -> SystemTime` | TC-REL-05, TC-REL-06 |
| Temp dir | `tempfile` (dev-dependency) | TC-STO-* |

## 6. Failure-mode matrix

| Failure | Where | Result | UI effect |
|---|---|---|---|
| Network down during repo add | `fetch_repo_details` | None → InvalidUrl | "Invalid repository URL" |
| Network down during store add | `service.fetch` | Err → `insert` Err | error toast with reason |
| Network down during refresh-all | per store | logged, others continue | none (silent) |
| DB error on track write | `TrackRepo::insert` | logged; default returned | Proposal: toast via `Result` path |
| DB error on custom-button write | ops | `InternalError` | toast |
| Storage base missing | accessors | `StorageUnavailable` | settings banner |
| GitHub rate limit | `latest` | Err | none (retry next launch) |
| Corrupt gzip | `gunzip_if_needed` | Err (Io) | store add/refresh error |
| Unknown protobuf | `sniff` → Proto | Err until area 19 | error toast |
| Fingerprint changed | refresh | skipped + warn | Proposal: badge on repo row |

## 7. Race conditions

- **RC-01** Two concurrent custom-button creates → duplicate sortIndex (Q-CB-2). Fixed by the mutex.
- **RC-02** Reorder while delete is compacting → interleaved sortIndex writes. Fixed by sharing the same mutex for delete (Proposal: make delete take the mutex too).
- **RC-03** Store removed during refresh → re-created row (E-STO-09). Proposal re-check.
- **RC-04** Track insert while the UI holds an old row id (Q-TRK-2). Fixed by upsert.
- **RC-05** Base directory changed while a download writes into the old base: downloader holds the old `PathBuf`; new downloads go to new base; old files stay. Proposal: document; the downloader resolves the base per job.
- **RC-06** Favourite toggle (off path) is not under the mutex (Q-CB-5): two concurrent toggles can both clear; harmless. Proposal: take the mutex on all toggle paths.
- **RC-07** Refresh-all sequential + user add during refresh: no conflict (different keys).

## 8. Performance notes

- `filter_new` is O(n + m) with a hash set of read numbers; Kotlin re-queries all chapters per call (`getChaptersByMangaId`). Proposal: pass the already-loaded chapter list from the library-update job to avoid a second query.
- Refresh-all for N repos issues N HTTP requests in parallel (Kotlin) — Rust caps concurrency at 4 with a `Semaphore` (Proposal; avoids hammering GitHub-hosted repos).
- Reorder rewrites all rows (O(n)); acceptable (n is small).
